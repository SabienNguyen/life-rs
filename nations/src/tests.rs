//! What the world at the scale of nations claims, checked as claims.

use std::sync::OnceLock;

use commerce::payments;
use sim_core::WorldSeed;

use crate::*;

/// A world that founds a chain, run until it does and a century beyond — long enough for checking
/// to have become cheap, which is when a chain does most of what it is for.
///
/// Shared, because getting there is six centuries of history — cheap at this scale, a few
/// seconds, but not something every test should pay for again.
fn chained() -> &'static Nations {
    static WORLD: OnceLock<Nations> = OnceLock::new();
    WORLD.get_or_init(|| {
        let mut world = Nations::found(WorldSeed::from_u128(0x11));
        while world.networks.is_empty() && world.year < 800 {
            world.year();
        }
        assert!(!world.networks.is_empty(), "seed 0x11 founds a chain within eight centuries");
        world.run(100);
        world
    })
}

/// The first two centuries of a world, before anything much has happened.
fn early() -> &'static Nations {
    static WORLD: OnceLock<Nations> = OnceLock::new();
    WORLD.get_or_init(|| {
        let mut world = Nations::found(WorldSeed::from_u128(0x21));
        world.run(200);
        world
    })
}

#[test]
fn nations_stand_on_the_planet_the_people_level_world_founds() {
    let seed = WorldSeed::from_u128(0x21);
    let nations = early();
    let surface = sim::Surface::genesis(seed);
    let grid = surface.planet.grid();
    assert_eq!(nations.surface.planet.grid().len(), grid.len());
    for cell in grid.cells().step_by(17) {
        assert_eq!(
            nations.surface.planet.height_above_sea_m(cell),
            surface.planet.height_above_sea_m(cell),
            "cell {cell}"
        );
    }
    // And every town stands on dry, habitable ground, one to a cell, none neighbouring another.
    for (a, town) in nations.towns.iter().enumerate() {
        assert!(nations.surface.planet.is_land(town.cell));
        for (b, other) in nations.towns.iter().enumerate() {
            if a != b {
                assert!(!grid.neighbours(town.cell).contains(&other.cell));
            }
        }
    }
}

#[test]
fn the_same_seed_is_the_same_world() {
    let run = || {
        let mut world = Nations::found(WorldSeed::from_u128(0x5eed));
        world.run(80);
        world.readings.clone()
    };
    assert_eq!(run(), run());
}

#[test]
fn every_town_is_in_one_state_and_every_state_is_in_one_country() {
    let world = early();
    let mut seen = vec![0; world.towns.len()];
    for (s, state) in world.states.iter().enumerate() {
        assert!(state.towns.contains(&state.hub), "a hub is in its own state");
        for t in &state.towns {
            seen[*t] += 1;
            assert_eq!(world.towns[*t].state, s);
            assert_eq!(world.towns[*t].country, state.country);
        }
        assert!(world.countries[state.country].states.contains(&s));
    }
    assert!(seen.iter().all(|n| *n == 1), "{seen:?}");
    // States are bigger than towns and smaller than countries — the level is not empty.
    assert!(world.states.len() < world.towns.len());
    assert!(world.states.len() > world.countries.len());
}

/// Money comes out of trade being thick, not out of anybody being rich: the first town to
/// trade mostly in a medium does so centuries before the world's income leaves subsistence.
#[test]
fn money_is_used_long_before_anybody_is_rich() {
    let world = chained();
    let first_money = world
        .history
        .iter()
        .find_map(|e| match e {
            Event::Monetised { year, .. } => Some(*year),
            _ => None,
        })
        .expect("money appears");
    let trap = world
        .history
        .iter()
        .find_map(|e| match e {
            Event::TrapOpened { year } => Some(*year),
            _ => None,
        })
        .expect("the trap opens");
    assert!(first_money + 100 < trap, "money in {first_money}, growth in {trap}");
    assert!(world.currencies.iter().all(|c| c.minted < trap));
}

/// The trap: for its first centuries a world's income stays near what it takes to eat however
/// much it learns, because every gain is eaten by the children it feeds — and then it opens.
#[test]
fn the_trap_holds_and_then_opens() {
    let world = chained();
    let early: Vec<&Reading> = world.readings.iter().filter(|r| (20..150).contains(&r.year)).collect();
    let mean = early.iter().map(|r| r.income).sum::<f64>() / early.len() as f64;
    assert!(mean < 1.6, "income in the first centuries averaged {mean:.2}");
    let people_then = early.first().unwrap().people;
    let people_after = early.last().unwrap().people;
    assert!(people_after > people_then, "the gains went into people");
    let last = world.readings.last().unwrap();
    assert!(last.income > 5.0, "and the trap opened: {:.2}", last.income);
}

/// Structural change: as the world gets richer, fewer of its people farm.
#[test]
fn the_richer_the_world_the_fewer_its_farmers() {
    let world = chained();
    let first = &world.readings[10];
    let last = world.readings.last().unwrap();
    assert!(first.shares[Sector::Farming as usize] > 0.7);
    assert!(last.shares[Sector::Farming as usize] < 0.3);
    assert!(last.shares[Sector::Serving as usize] > first.shares[Sector::Serving as usize]);
}

/// The demographic transition: once people are rich, population stops growing as fast as it
/// did on the way.
#[test]
fn births_slow_once_people_are_rich() {
    let world = chained();
    let growth_over = |from: &Reading, years: u64| {
        let later = world
            .readings
            .iter()
            .find(|r| r.year == from.year + years)
            .unwrap_or(world.readings.last().unwrap());
        (later.people / from.people).powf(1.0 / (later.year - from.year).max(1) as f64) - 1.0
    };
    let fastest = world
        .readings
        .iter()
        .filter(|r| r.year + 30 <= world.year)
        .map(|r| growth_over(r, 30))
        .fold(0.0f64, f64::max);
    let rich = world
        .readings
        .iter()
        .find(|r| r.income > 15.0)
        .expect("the world gets rich");
    let then = growth_over(rich, 20);
    assert!(then < fastest / 2.0, "grew {then:.4} a year once rich against {fastest:.4} at most");
}

/// Markets end famines. The same world with nothing able to move between towns goes hungry far
/// more often, because a bad harvest can no longer be met from somebody else's good one.
#[test]
fn markets_end_famines() {
    let famines = |trade: bool| {
        let mut world = Nations::found(WorldSeed::from_u128(0xbeef));
        world.trade_is_possible = trade;
        world.run(150);
        world
            .history
            .iter()
            .filter(|e| matches!(e, Event::Famine { .. }))
            .count()
    };
    let (with, without) = (famines(true), famines(false));
    assert!(without > 2 * with.max(1), "{without} famines without trade, {with} with");
}

/// A chain is what parties who do not trust each other build: its founders span countries, and
/// none of them is a keeper the rest would simply have used.
#[test]
fn a_chain_is_founded_by_parties_who_distrust_each_other() {
    let world = chained();
    let network = &world.networks[0];
    assert!(network.founders.len() >= payments::FEWEST_FOUNDERS);
    let countries: std::collections::BTreeSet<usize> = network
        .founders
        .iter()
        .map(|t| world.towns[*t].country)
        .collect();
    assert!(countries.len() >= 2, "founders from {} countries", countries.len());
    assert!(
        world.history.iter().any(|e| matches!(e, Event::Founded { .. })),
        "the founding is part of the world's history"
    );
}

/// A chain's validators follow its business: a founder whose state no longer carries its own
/// trade gives its stake back, and a house that has come to do a twentieth of the business takes
/// a seat. Each comes or goes once — read off business averaged over years rather than afresh
/// each year, a small state merging with its neighbour and splitting off again took a seat and
/// gave it up thirteen times in three centuries.
#[test]
fn validators_come_and_go_with_the_business() {
    let world = chained();
    let mut changes: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    let (mut joined, mut left) = (0, 0);
    for event in &world.history {
        match event {
            Event::Joined { town, .. } => {
                joined += 1;
                *changes.entry(*town).or_insert(0) += 1;
            }
            Event::Left { town, .. } => {
                left += 1;
                *changes.entry(*town).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    assert!(joined > 0 && left > 0, "{joined} joined and {left} left");
    assert!(changes.values().all(|n| *n <= 2), "{changes:?}");
    assert!(world.networks[0].validators().len() >= payments::FEWEST_FOUNDERS);
}

/// Nobody keeps it — in the strict sense: no country's houses hold the two thirds of the stake
/// that would let them finalise a block with nobody abroad signing, although one country does
/// most of the business. They were held to three fifths at the founding, and a capital that
/// buys a seat later is held to it too.
#[test]
fn no_country_can_finalise_a_block_on_its_own() {
    let world = chained();
    let (country, share) = network::largest_country_share(world, 0).expect("a chain has validators");
    assert!(
        share <= network::ONE_COUNTRY_AT_MOST + 1e-3,
        "{} holds {:.1}% of the stake",
        world.countries[country].name,
        100.0 * share
    );
    assert!(share < 2.0 / 3.0);
    // Nor at any height before: every set that ever signed a block kept to it.
    let (most, at) = most_one_country_ever_held(world);
    assert!(most <= network::ONE_COUNTRY_AT_MOST + 1e-3, "{most:.3} at height {at}");
}

/// The largest share of a signing set any one country's houses held, at any height of a world's
/// first chain, and the height — countries as the world reads them now.
fn most_one_country_ever_held(world: &Nations) -> (f64, u64) {
    let network = &world.networks[0];
    (0..=network.chain.height())
        .map(|height| {
            let set = network.chain.light_block(height).expect("a block").validators;
            let total: u64 = set.members.iter().map(|v| v.power).sum();
            let mut held = vec![0u64; world.countries.len()];
            for v in &set.members {
                let town = network.town_of(&v.address).expect("every validator is a house");
                held[world.towns[town].country] += v.power;
            }
            (held.iter().copied().max().unwrap_or(0) as f64 / total.max(1) as f64, height)
        })
        .fold((0.0, 0), |best, now| if now.0 > best.0 { now } else { best })
}

/// What the cap is for, tried rather than counted. The country whose houses hold the most stake
/// cuts a block of its own at the next height — the real tip moved on, with a ledger that suits
/// it better — and every one of its validators signs it: not final, because its stake is short of
/// the two thirds. The same block signed by every validator would be; what refuses it is who
/// signed, not what it looks like.
#[test]
fn the_largest_country_cannot_finalise_a_block_alone() {
    let world = chained();
    let network = &world.networks[0];
    let (country, share) = network::largest_country_share(world, 0).expect("validators");
    let tip = network.chain.tip();
    let mut header = tip.header.clone();
    header.height += 1;
    header.parent = tip.hash();
    header.state = chain::Digest::of(b"a ledger that suits us");
    let block = header.hash();
    let (id, height) = (network.chain.id, header.height);
    let alone = chain::Commit {
        round: 0,
        votes: network::votes_for(world, 0, block, Some(country)),
    };
    assert!(!alone.votes.is_empty(), "the country has validators");
    assert!(
        matches!(
            network.chain.rotation.check(&alone, id, height, block),
            Err(chain::consensus::NoQuorum::TooLittlePower { .. })
        ),
        "{} holds {:.1}% and finalised a block by itself",
        world.countries[country].name,
        100.0 * share
    );
    let everybody = chain::Commit {
        round: 0,
        votes: network::votes_for(world, 0, block, None),
    };
    assert!(network.chain.rotation.check(&everybody, id, height, block).is_ok());
}

/// What the report says forging the world's chain would take is what it takes. The validators
/// with most stake — as few as can do it, so that the rest can be split into two sides that each
/// make a quorum with them — sign two blocks at the next height, and each side of the rest signs
/// one, having seen only that: two histories, both final. A light client shown both names exactly
/// those who signed both, with more than a third of the power — never less than the report's
/// figure — and the chain, shown what it names by any house at all, jails them and burns at least
/// a twentieth of that.
#[test]
fn whoever_forks_the_worlds_chain_is_named_and_burned() {
    use chain::{Action, Address, Asset, COIN, Commit, Transaction, Vote};
    use std::collections::BTreeSet;

    let world = chained();
    let network = &world.networks[0];
    let chain = &network.chain;
    let mut rest = network::signers(world, 0);
    rest.sort_by_key(|(_, key, power)| (std::cmp::Reverse(*power), key.public()));
    let total: u64 = rest.iter().map(|(_, _, power)| power).sum();
    assert_eq!(total, chain.rotation.total_power(), "every validator has a house");
    let mut cheats = Vec::new();
    let (one_side, other_side) = loop {
        let power: u64 = cheats.iter().map(|(_, _, power)| power).sum();
        let (mut one, mut other, mut a, mut b) = (Vec::new(), Vec::new(), 0, 0);
        for signer in &rest {
            if a <= b {
                a += signer.2;
                one.push(signer.clone());
            } else {
                b += signer.2;
                other.push(signer.clone());
            }
        }
        if 3 * (power + a.min(b)) > 2 * total {
            break (one, other);
        }
        cheats.push(rest.remove(0));
    };

    let height = chain.height() + 1;
    let time = chain.tip().header.time + 1;
    let signed = |mut block: chain::Block, by: Vec<&(usize, chain::SigningKey, u64)>| {
        let hash = block.hash();
        block.commit = Commit {
            round: 0,
            votes: by.iter().map(|(_, key, _)| Vote::signed(key, chain.id, height, 0, hash)).collect(),
        };
        block
    };
    let (mut left, mut right) = (chain.clone(), chain.clone());
    let one = signed(chain.propose(0, time), cheats.iter().chain(&one_side).collect());
    let two = signed(chain.propose(0, time + 1), cheats.iter().chain(&other_side).collect());
    left.accept(one).expect("one side's block is final");
    right.accept(two).expect("and so is the other's");

    let trusted = chain.light_block(chain.height()).unwrap();
    let fork = chain::fork(
        &trusted,
        &[left.light_block(height).unwrap()],
        &[right.light_block(height).unwrap()],
    )
    .unwrap()
    .expect("two final blocks at one height");
    let named: BTreeSet<_> = fork.culprits.iter().map(|(vote, _)| vote.validator).collect();
    let guilty: BTreeSet<_> = cheats.iter().map(|(_, key, _)| key.public()).collect();
    assert_eq!(named, guilty, "those who signed both, and nobody who signed one");
    assert!(!one_side.is_empty() && !other_side.is_empty(), "honest validators on both sides");
    assert!(3 * fork.power > fork.total);
    assert!(fork.power as f64 >= (fork.total as f64 / 3.0).ceil(), "the report's figure is the least it takes");

    let fee = left.params().min_fee;
    let fees = fee * fork.culprits.len() as u128;
    let (_, accuser) = network::house_keys(world, 0)
        .into_iter()
        .find(|(_, key)| {
            !guilty.contains(&key.public())
                && left.pending_balance(&Address::of(&key.public()), Asset::Coin) >= fees
        })
        .expect("a house with coin for the fees");
    for (first, second) in &fork.culprits {
        let nonce = left.next_nonce(&Address::of(&accuser.public()));
        let evidence = Action::Evidence {
            first: Box::new(first.clone()),
            second: Box::new(second.clone()),
        };
        left.submit(Transaction::signed(&accuser, chain.id, nonce, fee, evidence))
            .expect("what the fork names, the chain takes");
    }
    let keys = network::signers(world, 0)
        .into_iter()
        .map(|(_, key, _)| (Address::of(&key.public()), key))
        .collect();
    left.step(time + 30 * 86_400, &keys, &|_, _| true, 8).expect("everybody at their posts");
    for (_, key, _) in &cheats {
        assert!(left.ledger.account(&Address::of(&key.public())).unwrap().jailed);
    }
    let burned = left.ledger.slashed - chain.ledger.slashed;
    assert!(
        burned >= fork.power as u128 * COIN / 20,
        "burned {} of a stake of {} coin",
        burned / COIN,
        fork.power
    );
    assert_eq!(left.ledger.broken_law(), None);
}

/// The cap keeps any one country from finalising a block alone; it does not keep one from
/// stopping the chain. A third of the stake away leaves a height short of the two thirds, so
/// every country — and every house — holding that much could stop the chain by staying away, and
/// nobody holding less could. It would wait, not split: once they are back it goes on, with the
/// block the others had already signed.
#[test]
fn a_third_of_the_stake_staying_away_stops_the_chain_and_less_does_not() {
    use chain::Address;
    use std::collections::BTreeMap;

    let world = chained();
    let network = &world.networks[0];
    let signers = network::signers(world, 0);
    let total: u64 = signers.iter().map(|(_, _, power)| power).sum();
    let keys: BTreeMap<Address, chain::SigningKey> = signers
        .iter()
        .map(|(_, key, _)| (Address::of(&key.public()), key.clone()))
        .collect();
    let time = network.chain.tip().header.time + 30 * 86_400;
    let country_of = |address: &Address| network.town_of(address).map(|t| world.towns[t].country);
    let mut stoppers = 0;
    for country in 0..world.countries.len() {
        let theirs: u64 = signers
            .iter()
            .filter(|(town, _, _)| world.towns[*town].country == country)
            .map(|(_, _, power)| power)
            .sum();
        let mut chain = network.chain.clone();
        let away = |address: &Address, _| country_of(address) != Some(country);
        let outcome = chain.step(time, &keys, &away, 64);
        assert_eq!(
            outcome.is_none(),
            3 * theirs >= total,
            "{} holds {theirs} of {total}",
            world.countries[country].name
        );
        if outcome.is_none() {
            stoppers += 1;
            let before = chain.tip().header.height;
            chain.step(time + 1, &keys, &|_, _| true, 64).expect("back, and it goes on");
            assert_eq!(chain.height(), before + 1);
            assert_eq!(chain.tip().header.time, time, "with the block the others had signed");
        }
    }
    let (_, largest) = network::largest_country_share(world, 0).expect("validators");
    assert!(largest > 1.0 / 3.0 && stoppers >= 1, "the largest country can stop it");
    for (town, key, power) in &signers {
        let mut chain = network.chain.clone();
        let gone = Address::of(&key.public());
        let outcome = chain.step(time, &keys, &|address, _| *address != gone, 64);
        assert_eq!(outcome.is_none(), 3 * power >= total, "{}'s house", world.towns[*town].name);
    }
}

/// The cap moves stake from the country over it to the others in proportion to what they hold,
/// and leaves a world where nobody is over it alone.
#[test]
fn stake_over_the_cap_goes_to_the_others() {
    let stakes = network::no_country_keeps_it(&[(0, 70.0), (0, 20.0), (1, 6.0), (2, 4.0)]);
    let total: f64 = stakes.iter().sum();
    assert!((total - 100.0).abs() < 1e-9, "no stake is lost: {total}");
    assert!((stakes[0] + stakes[1] - 60.0).abs() < 1e-9);
    assert!((stakes[0] / stakes[1] - 3.5).abs() < 1e-9, "a country's own houses keep their order");
    assert!((stakes[2] / stakes[3] - 1.5).abs() < 1e-9, "the others gain in proportion");
    let fair = [(0, 50.0), (1, 50.0)];
    assert_eq!(network::no_country_keeps_it(&fair), vec![50.0, 50.0]);
}

/// Everything the world has done on its chain can be checked by somebody holding nothing but
/// the genesis: every block, signature and root, from the first.
#[test]
fn the_chain_a_world_keeps_can_be_checked_from_its_genesis() {
    let network = &chained().networks[0];
    assert!(network.chain.height() >= 12 * 100, "a century of monthly blocks");
    assert_eq!(network.chain.verify(), Ok(()));
    assert_eq!(network.chain.ledger.broken_law(), None);
}

/// The stable token is never more than its reserve — the attestor states it and the chain
/// refuses a mint past it — and the world's issuer never lets it fall short.
#[test]
fn the_stable_token_is_backed_one_for_one() {
    let network = &chained().networks[0];
    let token = network.token.as_ref().expect("a token was issued");
    let on_chain = network.chain.token(token.id).expect("and is on the ledger");
    assert!(on_chain.supply > 0, "and used");
    assert!(on_chain.reserves >= on_chain.supply);
    assert!((on_chain.backing() - 1.0).abs() < 1e-9);
    assert!(on_chain.minted > on_chain.supply, "tokens are redeemed as well as minted");
}

/// A chain is used by the houses that founded it, not by capitals alone: every state big enough
/// to have its own market house pays for its own trade abroad. And what two houses pay each
/// other in a month is not the same both ways — orders come in lumps — though over a year what
/// a country buys and sells balances.
#[test]
fn the_houses_that_found_a_chain_are_the_ones_that_use_it() {
    let world = chained();
    let network = &world.networks[0];
    let token = network.token.as_ref().expect("a token was issued").id;
    let mut payers = std::collections::BTreeSet::new();
    let mut flows = std::collections::BTreeMap::new();
    for block in &network.chain.blocks {
        for tx in &block.txs {
            if let chain::Action::Pay {
                to,
                asset: chain::Asset::Token(id),
                amount,
            } = &tx.action
                && *id == token
                && let (Some(from), Some(to)) = (network.town_of(&tx.sender()), network.town_of(to))
            {
                payers.insert(from);
                *flows.entry((block.header.height, from, to)).or_insert(0u128) += amount;
            }
        }
    }
    let capitals: std::collections::BTreeSet<usize> = world.countries.iter().map(|c| c.capital).collect();
    assert!(
        payers.difference(&capitals).count() >= world.countries.len(),
        "{} houses pay, of which {} are capitals",
        payers.len(),
        payers.intersection(&capitals).count()
    );
    let mirrored = flows
        .iter()
        .filter(|((height, a, b), paid)| flows.get(&(*height, *b, *a)) == Some(paid))
        .count();
    assert!(mirrored * 20 < flows.len(), "{mirrored} of {} monthly flows mirrored", flows.len());
}

/// Coin changes hands on the chain against the stable token, both legs at once: every house
/// buys the coin its fees take in a swap it and the seller both sign. Only a house with no
/// tokens yet — the issuer, before there are any — buys its first coin over the counter.
#[test]
fn coin_is_bought_with_tokens_on_the_chain() {
    let network = &chained().networks[0];
    let (mut swaps, mut over_the_counter) = (0, 0);
    for block in &network.chain.blocks {
        for tx in &block.txs {
            match &tx.action {
                chain::Action::Swap(swap) => {
                    assert_eq!(swap.give.0, chain::Asset::Coin, "houses sell coin for tokens");
                    swaps += 1;
                }
                chain::Action::Pay {
                    asset: chain::Asset::Coin,
                    ..
                } => over_the_counter += 1,
                _ => {}
            }
        }
    }
    assert!(swaps > 100, "{swaps} swaps");
    assert!(over_the_counter * 50 < swaps, "{over_the_counter} over the counter against {swaps} swaps");
}

/// A house does not send what it cannot pay the fee for: the founders keep enough coin liquid to
/// pay a chain's fees while checking is dear, the houses that join buy theirs from those with
/// coin to spare, and the issuer and attestor budget for their own. So the chain refuses nothing
/// the world sends it.
#[test]
fn nothing_the_world_sends_its_chain_is_refused() {
    let network = &chained().networks[0];
    assert_eq!(network.refused, 0, "{:?}", network.refusals);
}

/// What a chain is for: paying abroad costs less once there is one.
#[test]
fn a_chain_makes_paying_abroad_cheaper() {
    let world = chained();
    let founded = world.networks[0].founded;
    assert!(world.networks[0].carried > 0.0, "the chain carries payments");
    let now: f64 = world.countries.iter().map(|c| c.pay_cost).sum::<f64>() / world.countries.len() as f64;
    // What routing through houses would cost, with no chain at all, between the same countries.
    let bank = world.cost_through_houses().expect("more than one country");
    assert!(
        now < 0.8 * bank,
        "{now:.3} with the chain against {bank:.3} through houses, founded {founded}"
    );
}

/// What a chain is for, as the world feels it: the same world with no chain trades less across
/// its borders, and one where paying abroad costs nothing at all trades a little more. The chain
/// gets most of the way there. It does not have to move income much — trade abroad is a few per
/// cent of what a world makes — but it has to move trade, or it has changed nothing.
///
/// Written after finding that it had changed nothing: redrawing the countries every year reset
/// what paying abroad cost before the market read it, so the chain lowered a number nobody used.
///
/// And it has to make somebody better off. Counted at one price for a ware it hardly does, but
/// people who buy each country's wares for being that country's are better off for buying more
/// of them, and a small country, which buys most of its wares abroad, most of all.
#[test]
fn a_chain_widens_the_markets_it_touches() {
    let world = chained();
    let otherwise = |free: bool, chains: bool| {
        let mut other = Nations::found(WorldSeed::from_u128(0x11));
        other.borders_are_free = free;
        other.chains_are_possible = chains;
        other.run(world.year);
        other
    };
    let (unchained, freed) = (otherwise(false, false), otherwise(true, true));
    let last = |w: &Nations| w.readings.last().expect("a year has passed").clone();
    let (without, with, free) = (last(&unchained), last(world), last(&freed));
    let (without, with, free) = (without.traded, with.traded, free.traded);
    assert!(with > 1.1 * without, "{with:.4} traded abroad with the chain, {without:.4} without");
    assert!(with <= 1.01 * free, "{with:.4} with the chain, {free:.4} with free borders");
    assert!(with - without > 0.5 * (free - without), "most of the way: {without:.4} → {with:.4} of {free:.4}");

    let (without, with) = (last(&unchained).with_variety, last(world).with_variety);
    assert!(with > 1.003 * without, "a head lives on {with:.2} with the chain, {without:.2} without");
    let smallest = |w: &Nations| w.countries.last().expect("more than one country").variety;
    let (without, with) = (smallest(&unchained), smallest(world));
    assert!(with > 1.05 * without, "the smallest country's wares are worth {with:.3} with the chain, {without:.3} without");
}

/// Two clerks at one key, once, in the worst place: the largest validator of the smaller
/// country, whose jailing leaves the larger one holding more than two thirds of the stake. The
/// house that proposes next shows both signatures; the chain burns a twentieth of the stake and
/// jails the key for good; the house takes back the rest; and its country bonds what brings the
/// larger one back to its three fifths — the house itself, under a key it keeps for staking —
/// with nothing refused and nothing stalled.
#[test]
fn a_house_that_signs_twice_is_caught_and_the_chain_mended() {
    let mut world = Nations::found(WorldSeed::from_u128(0x11));
    while world.networks.is_empty() {
        world.year();
    }
    world.run(20);
    let smaller = world.countries.len() - 1;
    let network = &world.networks[0];
    let offender = network
        .validators()
        .into_iter()
        .filter(|t| world.towns[*t].country == smaller)
        .max_by_key(|t| {
            let address = network.address_of(*t).expect("a validator has an account");
            network.chain.ledger.account(&address).map(|a| a.bonded).unwrap_or(0)
        })
        .expect("the smaller country validates");
    let address = network.address_of(offender).expect("an account");
    let at_stake = network.chain.ledger.account(&address).expect("an account").bonded;
    let slashed = network.chain.ledger.slashed;

    network::sign_twice(&mut world, 0, offender);
    world.run(2);

    let network = &world.networks[0];
    let burned = world
        .history
        .iter()
        .find_map(|e| match e {
            Event::Slashed { town, burned, .. } if *town == offender => Some(*burned),
            _ => None,
        })
        .expect("the offence is in the world's history");
    assert_eq!(burned, at_stake * 50 / 1000, "a twentieth of the stake");
    assert_eq!(network.chain.ledger.slashed - slashed, burned);
    let account = network.chain.ledger.account(&address).expect("an account");
    assert!(account.jailed);
    assert_eq!(account.bonded, 0, "it took back what was left");
    assert!(!network.chain.rotation.members.iter().any(|v| v.address == address));
    // Its country's best-placed house to mend the stake was itself, under a key kept for staking.
    assert!(network.validators().contains(&offender));
    assert_ne!(network.staking_address(offender), Some(address));
    assert_eq!(network.address_of(offender), Some(address), "and its own account is its own");
    assert!(network.validators().len() >= payments::FEWEST_FOUNDERS);
    let (country, share) = network::largest_country_share(&world, 0).expect("validators");
    assert!(
        share <= network::ONE_COUNTRY_AT_MOST + 1e-3,
        "{} holds {:.1}% of the stake",
        world.countries[country].name,
        100.0 * share
    );
    assert_eq!(network.refused, 0, "{:?}", network.refusals);
    assert_eq!(network.stalls, 0);
    assert_eq!(network.chain.verify(), Ok(()));
    let light = chain::light::follow(&network.chain.genesis, &network.chain.light_blocks());
    assert_eq!(light, Ok(network.chain.height()));
}

/// The world that found the gap. 0x5f's smaller country has one house on its chain, Lingquay,
/// and in year 565 that house is caught signing twice. Before a caught house could come back
/// under a key it keeps for staking, nobody in that country could ever bond again, and the other
/// held the whole of the stake from then on; now Lingquay is back the same year, and no country
/// holds more than its three fifths — at the end, or at any height on the way.
#[test]
fn a_country_whose_only_house_is_caught_keeps_its_share() {
    let mut world = Nations::found(WorldSeed::from_u128(0x5f));
    world.run(570);
    let caught = world
        .history
        .iter()
        .find_map(|e| match e {
            Event::Slashed { town, .. } => Some(*town),
            _ => None,
        })
        .expect("a house is caught");
    let network = &world.networks[0];
    let country = world.towns[caught].country;
    let houses = network.houses().filter(|t| world.towns[*t].country == country).count();
    assert_eq!(houses, 1, "the one house its country has on the chain");
    assert!(network.validators().contains(&caught));
    assert_ne!(network.staking_address(caught), network.address_of(caught));
    let (largest, share) = network::largest_country_share(&world, 0).expect("validators");
    assert!(
        share <= network::ONE_COUNTRY_AT_MOST + 1e-3,
        "{} holds {:.1}% of the stake",
        world.countries[largest].name,
        100.0 * share
    );
    // Not even for the block after it was caught: the evidence and the mending go into one.
    let (most, at) = most_one_country_ever_held(&world);
    assert!(most <= network::ONE_COUNTRY_AT_MOST + 1e-3, "{most:.3} at height {at}");
    assert_eq!(network.refused, 0, "{:?}", network.refusals);
}

/// A world that is one country has a house everybody in it can pay through, and never builds a
/// ledger nobody keeps.
#[test]
fn a_world_that_is_one_country_keeps_its_books_in_one_house() {
    let mut world = Nations::found(WorldSeed::from_u128(0x2b));
    world.run(600);
    assert_eq!(world.countries.len(), 1);
    assert!(world.networks.is_empty());
    assert_eq!(world.not_yet, Some(payments::NotYet::OneCountry));
}
