//! What a chain claims to be, checked as claims rather than as functions.

use std::collections::BTreeMap;

use crate::state::monthly;
use crate::*;

/// A world's worth of keys: four founding validators with equal stake, and some people who
/// hold nothing yet.
struct Fixture {
    chain: Chain,
    validators: Vec<SigningKey>,
    people: Vec<SigningKey>,
    keys: BTreeMap<Address, SigningKey>,
    time: u64,
}

const MONTH: u64 = 30 * 86_400;

fn found_with(stakes: &[u64]) -> Fixture {
    let validators: Vec<SigningKey> = (0..stakes.len())
        .map(|i| SigningKey::from_seed([10 + i as u8; 32]))
        .collect();
    let people: Vec<SigningKey> = (0..4).map(|i| SigningKey::from_seed([50 + i; 32])).collect();
    let mut allocations: Vec<(PublicKey, u64, u64)> = validators
        .iter()
        .zip(stakes)
        .map(|(k, stake)| (k.public(), 100 * COIN, stake * COIN))
        .collect();
    // The first person starts with some coin, so there is something to spend.
    allocations.push((people[0].public(), 1_000 * COIN, 0));
    let genesis = Genesis {
        params: monthly("test"),
        time: 0,
        allocations,
    };
    let chain = Chain::found(genesis, &validators).expect("founders sign their own genesis");
    let keys = validators
        .iter()
        .map(|k| (Address::of(&k.public()), k.clone()))
        .collect();
    Fixture {
        chain,
        validators,
        people,
        keys,
        time: 0,
    }
}

impl Fixture {
    fn everybody(&mut self) -> Committed {
        self.time += MONTH;
        self.chain
            .step(self.time, &self.keys, &|_, _| true, 8)
            .expect("everybody answering commits a block")
    }

    fn pay(&mut self, from: usize, to: Address, asset: Asset, amount: u64) -> Result<Digest, Refusal> {
        let key = self.people[from].clone();
        let nonce = self.chain.next_nonce(&Address::of(&key.public()));
        let tx = Transaction::signed(
            &key,
            self.chain.id,
            nonce,
            self.chain.params().min_fee,
            Action::Pay { to, asset, amount },
        );
        self.chain.submit(tx)
    }

    fn address(&self, person: usize) -> Address {
        Address::of(&self.people[person].public())
    }
}

#[test]
fn a_chain_needs_more_than_two_thirds_of_its_founders_to_begin() {
    let keys: Vec<SigningKey> = (0..3).map(|i| SigningKey::from_seed([i + 1; 32])).collect();
    let genesis = Genesis {
        params: monthly("consent"),
        time: 0,
        allocations: keys.iter().map(|k| (k.public(), 0, 10 * COIN)).collect(),
    };
    assert!(
        matches!(
            Chain::found(genesis.clone(), &keys[..2]),
            Err(Invalid::Commit(consensus::NoQuorum::TooLittlePower { .. }))
        ),
        "two of three founders is exactly two thirds, which is not a quorum"
    );
    assert!(Chain::found(genesis, &keys).is_ok());
}

#[test]
fn a_payment_moves_what_it_says_and_the_laws_hold() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let (alice, bob) = (world.address(0), world.address(1));
    world.pay(0, bob, Asset::Coin, 30 * COIN).expect("alice can afford it");
    let block = world.everybody();
    assert_eq!(block.txs, 1);
    assert_eq!(world.chain.balance(&bob, Asset::Coin), 30 * COIN);
    let fee = world.chain.params().min_fee;
    assert_eq!(world.chain.balance(&alice, Asset::Coin), 970 * COIN - fee);
    assert_eq!(world.chain.ledger.broken_law(), None);
    assert_eq!(world.chain.ledger.fees_paid, fee);
}

/// The two ways of spending a coin twice — sending the same payment again, and writing a
/// second payment against money the first has already taken — and both are refused.
#[test]
fn nothing_is_spent_twice() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let bob = world.address(1);
    let carol = world.address(2);
    let key = world.people[0].clone();
    let first = Transaction::signed(
        &key,
        world.chain.id,
        0,
        COIN / 10_000,
        Action::Pay {
            to: bob,
            asset: Asset::Coin,
            amount: 900 * COIN,
        },
    );
    world.chain.submit(first.clone()).expect("the first spend is fine");
    assert!(
        matches!(world.chain.submit(first.clone()), Err(Refusal::WrongNonce { .. })),
        "the same payment again is refused while it waits"
    );
    // A second payment of most of the same money, correctly sequenced, cannot be afforded
    // once the first is counted.
    assert_eq!(
        world.pay(0, carol, Asset::Coin, 900 * COIN),
        Err(Refusal::CannotAfford)
    );
    world.everybody();
    assert!(
        matches!(world.chain.submit(first), Err(Refusal::WrongNonce { .. })),
        "and refused after it has gone through"
    );
    assert_eq!(world.chain.balance(&bob, Asset::Coin), 900 * COIN);
    assert_eq!(world.chain.balance(&carol, Asset::Coin), 0);
}

#[test]
fn a_stable_token_is_only_minted_against_an_attested_reserve() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let issuer = world.people[0].clone();
    let attestor = world.people[1].clone();
    let holder = world.address(2);
    let id = world.chain.id;
    let fee = world.chain.params().min_fee;
    // The attestor needs coin for fees.
    world.pay(0, Address::of(&attestor.public()), Asset::Coin, 10 * COIN).unwrap();
    let submit = |world: &mut Fixture, key: &SigningKey, action: Action| {
        let nonce = world.chain.next_nonce(&Address::of(&key.public()));
        world
            .chain
            .submit(Transaction::signed(key, id, nonce, fee, action))
    };
    // Nobody may vouch for their own reserve.
    assert_eq!(
        submit(
            &mut world,
            &issuer,
            Action::Issue {
                symbol: "TIL".into(),
                peg: "tilmark".into(),
                attestor: Address::of(&issuer.public()),
            }
        ),
        Err(Refusal::SelfAttested)
    );
    submit(
        &mut world,
        &issuer,
        Action::Issue {
            symbol: "TIL".into(),
            peg: "tilmark".into(),
            attestor: Address::of(&attestor.public()),
        },
    )
    .unwrap();
    world.everybody();

    // Nothing attested, nothing minted.
    let mint = |amount| Action::Mint {
        token: 0,
        to: holder,
        amount,
    };
    assert!(matches!(
        submit(&mut world, &issuer, mint(5 * TOKEN_UNIT)),
        Err(Refusal::BeyondReserves { .. })
    ));
    // The issuer cannot attest for itself, and the attestor cannot mint.
    assert_eq!(
        submit(&mut world, &issuer, Action::Attest { token: 0, reserves: 100 * TOKEN_UNIT }),
        Err(Refusal::NotTheAttestor)
    );
    submit(&mut world, &attestor, Action::Attest { token: 0, reserves: 100 * TOKEN_UNIT })
        .unwrap();
    assert_eq!(
        submit(&mut world, &attestor, mint(5 * TOKEN_UNIT)),
        Err(Refusal::NotTheIssuer)
    );
    submit(&mut world, &issuer, mint(60 * TOKEN_UNIT)).unwrap();
    assert!(
        matches!(
            submit(&mut world, &issuer, mint(41 * TOKEN_UNIT)),
            Err(Refusal::BeyondReserves { .. })
        ),
        "sixty minted and forty-one more would pass a hundred"
    );
    submit(&mut world, &issuer, mint(40 * TOKEN_UNIT)).unwrap();
    world.everybody();
    assert_eq!(world.chain.balance(&holder, Asset::Token(0)), 100 * TOKEN_UNIT);

    // The holder pays some on, and hands some back. Handing back destroys it.
    world.pay(2, world.address(3), Asset::Token(0), 30 * TOKEN_UNIT).unwrap_err(); // no coin for the fee
    world.pay(0, holder, Asset::Coin, COIN).unwrap();
    world.everybody();
    world.pay(2, world.address(3), Asset::Token(0), 30 * TOKEN_UNIT).unwrap();
    let redeemer = world.people[2].clone();
    submit(&mut world, &redeemer, Action::Redeem { token: 0, amount: 20 * TOKEN_UNIT }).unwrap();
    world.everybody();
    let token = world.chain.token(0).unwrap().clone();
    assert_eq!(token.supply, 80 * TOKEN_UNIT);
    assert_eq!(token.redeemed, 20 * TOKEN_UNIT);
    assert_eq!(world.chain.balance(&holder, Asset::Token(0)), 50 * TOKEN_UNIT);
    assert_eq!(world.chain.balance(&world.address(3), Asset::Token(0)), 30 * TOKEN_UNIT);
    assert_eq!(world.chain.ledger.broken_law(), None);

    // A shortfall can be stated — that is the attestor's whole job — and it stops minting.
    submit(&mut world, &attestor, Action::Attest { token: 0, reserves: 70 * TOKEN_UNIT }).unwrap();
    world.everybody();
    assert!(world.chain.token(0).unwrap().backing() < 1.0);
    assert!(matches!(
        submit(&mut world, &issuer, mint(TOKEN_UNIT)),
        Err(Refusal::BeyondReserves { .. })
    ));
}

/// Change one byte of one old block and replaying from genesis finds it, at that block.
#[test]
fn history_cannot_be_rewritten_quietly() {
    let mut world = found_with(&[25, 25, 25, 25]);
    for month in 0..6 {
        world.pay(0, world.address(1 + month % 3), Asset::Coin, (month as u64 + 1) * COIN).unwrap();
        world.everybody();
    }
    assert_eq!(world.chain.verify(), Ok(()));
    let history = world.chain.blocks.clone();
    let genesis = world.chain.genesis.clone();

    // A payment made larger after the fact. Its id changes with it, so the header's
    // transaction root no longer matches — the first line of defence.
    let mut forged = history.clone();
    if let Action::Pay { amount, .. } = &mut forged[3].txs[0].action {
        *amount += 1;
    }
    assert_eq!(
        Chain::replay(&genesis, &forged).map(|_| ()),
        Err((3, Invalid::WrongTxRoot))
    );
    // Patch the root to match and the second line holds: the signer never signed that.
    forged[3].header.txs = Block::root_of(&forged[3].txs);
    assert_eq!(
        Chain::replay(&genesis, &forged).map(|_| ()),
        Err((3, Invalid::BadTxSignature { index: 0 }))
    );

    // A payment removed: the header's root no longer matches.
    let mut removed = history.clone();
    removed[4].txs.clear();
    assert_eq!(
        Chain::replay(&genesis, &removed).map(|_| ()),
        Err((4, Invalid::WrongTxRoot))
    );

    // Removed, with the header patched to match: now the validators' signatures are over a
    // different header.
    let mut patched = history.clone();
    patched[4].txs.clear();
    patched[4].header.txs = Block::root_of(&[]);
    patched[4].header.tx_count = 0;
    assert!(matches!(
        Chain::replay(&genesis, &patched),
        Err((4, Invalid::WrongStateRoot))
    ));

    // Patch the state root as well, and the block is internally consistent — and nobody
    // signed it.
    let mut resealed = patched.clone();
    let mut ledger = Chain::replay(&genesis, &history[..4]).unwrap();
    ledger.close(4, resealed[4].header.proposer, &{
        let rotation = world.chain.rotation.clone();
        rotation.members.iter().map(|v| (v.address, v.power)).collect::<Vec<_>>()
    });
    resealed[4].header.state = ledger.root();
    assert_eq!(
        Chain::replay(&genesis, &resealed).map(|_| ()),
        Err((4, Invalid::Commit(consensus::NoQuorum::StrayVote))),
        "every vote in the commit is for the header as it was"
    );

    // And a block dropped out of the middle breaks the parent link.
    let mut skipped = history.clone();
    skipped.remove(2);
    assert!(matches!(
        Chain::replay(&genesis, &skipped),
        Err((3, Invalid::NotNext { .. }))
    ));
}

#[test]
fn the_chain_keeps_going_with_a_quarter_absent() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let absent = Address::of(&world.validators[0].public());
    let mut rounds_lost = 0;
    for month in 1..=24 {
        world.time = month * MONTH;
        let before = world.chain.rounds_failed;
        world
            .chain
            .step(world.time, &world.keys, &|v, _| *v != absent, 8)
            .expect("three of four is more than two thirds");
        rounds_lost += world.chain.rounds_failed - before;
    }
    assert_eq!(world.chain.height(), 24);
    // The absent validator was due to propose about a quarter of the time, and each time the
    // round failed and passed to somebody else.
    assert!((3..=10).contains(&rounds_lost), "{rounds_lost} rounds lost in 24 heights");
    assert_eq!(world.chain.verify(), Ok(()));
}

#[test]
fn the_chain_stops_rather_than_split_when_half_are_absent() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let gone: Vec<Address> = world.validators[..2]
        .iter()
        .map(|k| Address::of(&k.public()))
        .collect();
    world.time += MONTH;
    let outcome = world
        .chain
        .step(world.time, &world.keys, &|v, _| !gone.contains(v), 8);
    assert_eq!(outcome, None, "half the stake cannot finalise anything");
    assert_eq!(world.chain.height(), 0);
    // They come back, and it carries on from where it stopped.
    world.everybody();
    assert_eq!(world.chain.height(), 1);
}

/// A proposer who puts a bad transaction in a block gets no signatures for it: the honest
/// validators re-derive the block before they sign and find the lie.
#[test]
fn a_block_that_breaks_a_rule_is_never_signed() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let good = world.chain.propose(0, MONTH);
    assert!(world.chain.validate(&good).is_ok());

    // Slip in a payment its signer cannot afford, and redo the header so it all matches.
    let pauper = world.people[3].clone();
    let theft = Transaction::signed(
        &pauper,
        world.chain.id,
        0,
        COIN / 10_000,
        Action::Pay {
            to: world.address(1),
            asset: Asset::Coin,
            amount: 5 * COIN,
        },
    );
    let mut bad = good.clone();
    bad.txs.push(theft);
    bad.header.txs = Block::root_of(&bad.txs);
    bad.header.tx_count = bad.txs.len() as u32;
    assert!(matches!(
        world.chain.validate(&bad),
        Err(Invalid::BadTx {
            why: Refusal::CannotAfford,
            ..
        })
    ));

    // And even signed by everybody, `accept` refuses it, because accepting re-derives it too.
    let hash = bad.hash();
    bad.commit = Commit {
        round: 0,
        votes: world
            .validators
            .iter()
            .map(|k| Vote::signed(k, world.chain.id, 1, 0, hash))
            .collect(),
    };
    assert!(world.chain.accept(bad).is_err());
    assert_eq!(world.chain.height(), 0);
}

/// Two signatures for two different blocks at one height is the only way a BFT chain forks,
/// and it is what stake is for: the evidence is on the chain and the stake is gone.
#[test]
fn a_validator_who_signs_twice_loses_stake_and_its_seat() {
    let mut world = found_with(&[25, 25, 25, 25]);
    world.everybody();
    let cheat = world.validators[2].clone();
    let cheat_address = Address::of(&cheat.public());
    let id = world.chain.id;
    let one = Vote::signed(&cheat, id, 2, 0, Digest::of(b"one block"));
    let two = Vote::signed(&cheat, id, 2, 0, Digest::of(b"another block"));
    let staked = world.chain.ledger.account(&cheat_address).unwrap().bonded;
    let supply = world.chain.ledger.coin_supply;

    let accuser = world.people[0].clone();
    let evidence = |nonce| {
        Transaction::signed(
            &accuser,
            id,
            nonce,
            COIN / 10_000,
            Action::Evidence {
                first: Box::new(one.clone()),
                second: Box::new(two.clone()),
            },
        )
    };
    world.chain.submit(evidence(0)).unwrap();
    world.everybody();

    let after = world.chain.ledger.account(&cheat_address).unwrap();
    assert!(after.jailed);
    let burned = staked - after.bonded;
    assert_eq!(burned, staked * 50 / 1000, "five per cent of the stake");
    assert_eq!(world.chain.ledger.slashed, burned);
    assert!(
        world.chain.ledger.coin_supply < supply + world.chain.ledger.issuance_at(2),
        "the burn came out of the supply"
    );
    assert!(
        world.chain.rotation.find(&cheat.public()).is_none(),
        "a jailed validator does not validate"
    );
    // The same offence cannot be shown twice.
    assert_eq!(world.chain.submit(evidence(1)), Err(Refusal::AlreadyPunished));
    // Two votes for the *same* block are not an offence.
    let same = Transaction::signed(
        &accuser,
        id,
        1,
        COIN / 10_000,
        Action::Evidence {
            first: Box::new(one.clone()),
            second: Box::new(one.clone()),
        },
    );
    assert_eq!(world.chain.submit(same), Err(Refusal::BadEvidence));
    assert_eq!(world.chain.ledger.broken_law(), None);
    assert_eq!(world.chain.verify(), Ok(()));
}

#[test]
fn stake_decides_who_proposes() {
    let mut world = found_with(&[10, 20, 30, 40]);
    let mut proposed: BTreeMap<Address, u32> = BTreeMap::new();
    for _ in 0..200 {
        let block = world.everybody();
        *proposed.entry(block.proposer).or_default() += 1;
    }
    for (i, key) in world.validators.iter().enumerate() {
        let count = proposed.get(&Address::of(&key.public())).copied().unwrap_or(0);
        let share = count as f64 / 200.0;
        // Stakes 10, 20, 30, 40 of 100 — but rewards compound the stake of whoever is paid,
        // so the proportions drift a little over two hundred blocks.
        let expected = (i + 1) as f64 / 10.0;
        assert!((share - expected).abs() < 0.04, "validator {i} proposed {share:.3}, stake {expected}");
    }
}

#[test]
fn issuance_halves_until_there_is_none() {
    let world = found_with(&[25, 25, 25, 25]);
    let ledger = &world.chain.ledger;
    assert_eq!(ledger.issuance_at(1), 50 * COIN);
    assert_eq!(ledger.issuance_at(48), 25 * COIN);
    assert_eq!(ledger.issuance_at(96), 12 * COIN + COIN / 2);
    let ever: u64 = (1..48 * 64).map(|h| ledger.issuance_at(h)).sum();
    assert!(ever < 48 * 100 * COIN, "a halving schedule converges: {ever}");
    assert_eq!(ledger.issuance_at(48 * 64), 0);
}

/// A holder can show anybody their balance with a proof against a block header, and that
/// person needs nothing but the header — which is what makes a stable token something you can
/// be paid in by a stranger.
#[test]
fn a_balance_can_be_shown_with_nothing_but_a_header() {
    let mut world = found_with(&[25, 25, 25, 25]);
    world.pay(0, world.address(1), Asset::Coin, 7 * COIN).unwrap();
    world.everybody();
    let root = world.chain.tip().header.state;
    let proof = world.chain.ledger.prove(&world.address(1)).expect("bob has an account");
    assert!(proof.holds_under(&root));
    assert_eq!(proof.account.coin, 7 * COIN);
    let mut inflated = proof.clone();
    inflated.account.coin += COIN;
    assert!(!inflated.holds_under(&root), "a balance claimed larger does not prove");
}

#[test]
fn unbonded_stake_waits_before_it_can_be_spent() {
    let mut world = found_with(&[25, 25, 25, 25]);
    let validator = world.validators[1].clone();
    let address = Address::of(&validator.public());
    let before = world.chain.balance(&address, Asset::Coin);
    world
        .chain
        .submit(Transaction::signed(
            &validator,
            world.chain.id,
            0,
            COIN / 10_000,
            Action::Unbond { amount: 5 * COIN },
        ))
        .unwrap();
    world.everybody();
    let account = world.chain.ledger.account(&address).unwrap().clone();
    assert_eq!(account.bonded, 20 * COIN);
    assert_eq!(account.unbonding.len(), 1);
    let waiting = world.chain.params().unbonding_blocks;
    for _ in 0..waiting {
        assert_eq!(world.chain.ledger.account(&address).unwrap().unbonding.len(), 1);
        world.everybody();
    }
    let released = world.chain.ledger.account(&address).unwrap();
    assert!(released.unbonding.is_empty());
    assert!(released.coin >= before + 5 * COIN - COIN / 10_000);
    assert_eq!(world.chain.ledger.broken_law(), None);
}

#[test]
fn a_chain_replays_to_exactly_the_ledger_it_holds() {
    let mut world = found_with(&[5, 30, 30, 35]);
    // Somebody with enough to stake, eventually.
    world.pay(0, world.address(1), Asset::Coin, 200 * COIN).unwrap();
    for month in 0..30u64 {
        let to = world.address(1 + (month % 3) as usize);
        world.pay(0, to, Asset::Coin, COIN + month).unwrap();
        if month % 7 == 3 {
            // Somebody new starts validating partway through.
            let joiner = world.people[1].clone();
            let nonce = world.chain.next_nonce(&Address::of(&joiner.public()));
            let _ = world.chain.submit(Transaction::signed(
                &joiner,
                world.chain.id,
                nonce,
                COIN / 10_000,
                Action::Bond { amount: 10 * COIN },
            ));
            world.keys.insert(Address::of(&joiner.public()), joiner);
        }
        world.everybody();
        assert_eq!(world.chain.ledger.broken_law(), None, "month {month}");
    }
    assert_eq!(world.chain.verify(), Ok(()));
    assert!(
        world.chain.rotation.find(&world.people[1].public()).is_some(),
        "a person who bonded enough became a validator"
    );
}
