//! A chain: where it started, every block since, and the ledger they add up to.
//!
//! One `Chain` is every honest node at once. They would all hold the same blocks and compute
//! the same ledger — that is what being honest means here — so the simulation keeps one copy
//! and lets each validator's *answer* differ: present or absent in a round, and, in the tests,
//! willing or not to sign something it should not. What no part of this does is take a
//! shortcut through a rule. A block is appended only after it has been re-derived in full and
//! a quorum of real signatures over its hash has been checked, and `Chain::replay` does all of
//! that again from genesis with nothing cached.

use std::collections::{BTreeMap, BTreeSet};

use crate::block::{Block, Header};
use crate::codec::{Malformed, Reader, Writer};
use crate::consensus::{Commit, NoQuorum, ValidatorSet, Vote};
use crate::light::LightBlock;
use crate::state::{Account, Ledger, MAX_SUPPLY, Params, Refusal, Token};
use crate::tx::{Address, Asset, Transaction};
use crate::{Digest, PublicKey, SigningKey};

/// How a chain begins: its rules, when, and who holds the first coin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Genesis {
    pub params: Params,
    pub time: u64,
    /// A key, the coin it holds liquid, and the coin it has staked.
    pub allocations: Vec<(PublicKey, u128, u128)>,
}

impl Genesis {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::tagged("life-rs/chain/genesis/1");
        self.params.encode_into(&mut w);
        w.u64(self.time).u32(self.allocations.len() as u32);
        for (key, liquid, bonded) in &self.allocations {
            w.fixed(&key.0).u128(*liquid).u128(*bonded);
        }
        w.finish()
    }

    /// `encode`, backwards.
    pub fn decode(bytes: &[u8]) -> Result<Genesis, Malformed> {
        let mut r = Reader::tagged(bytes, "life-rs/chain/genesis/1")?;
        let params = Params::decode_from(&mut r)?;
        let time = r.u64()?;
        let n = r.count(32 + 16 + 16)?;
        let mut allocations = Vec::with_capacity(n);
        for _ in 0..n {
            allocations.push((PublicKey(r.fixed()?), r.u128()?, r.u128()?));
        }
        r.done()?;
        Ok(Genesis {
            params,
            time,
            allocations,
        })
    }

    /// Whether this can begin a chain at all: a share of stake to burn for signing twice that
    /// is a share, and no more coin — what the genesis hands out, and every block's issuance
    /// until it halves to nothing — than `MAX_SUPPLY`. A chain whose genesis passes this can
    /// never overflow a balance, a stake or its supply, whatever its blocks do; one that does
    /// not is refused before a ledger is built from it, by founding, replaying or following.
    pub fn check(&self) -> Result<(), &'static str> {
        if self.params.slash_permille > 1000 {
            return Err("it would burn more than the stake");
        }
        let handed_out = self
            .allocations
            .iter()
            .try_fold(0u128, |sum, (_, liquid, bonded)| sum.checked_add(*liquid)?.checked_add(*bonded));
        // Issuance halves every `halving` blocks, so all of it is under twice one run of it.
        let issued = self
            .params
            .issuance
            .checked_mul(self.params.halving.max(1) as u128)
            .and_then(|run| run.checked_mul(2));
        match (handed_out, issued) {
            (Some(a), Some(b)) if a.checked_add(b).is_some_and(|all| all <= MAX_SUPPLY) => Ok(()),
            _ => Err("it could make more coin than a chain can count"),
        }
    }

    /// The chain's id, which every transaction and vote on it carries.
    pub fn id(&self) -> Digest {
        Digest::of(&self.encode())
    }
}

/// Why a block is not the next block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    WrongChain,
    NotNext { expected: u64, got: u64 },
    WrongParent,
    WrongLastCommit,
    TimeGoesBackwards,
    WrongProposer,
    WrongValidators,
    TooManyTxs,
    WrongTxRoot,
    BadTxSignature { index: usize },
    BadTx { index: usize, why: Refusal },
    WrongStateRoot,
    Commit(NoQuorum),
    NotGenesis,
    /// A genesis that cannot begin a chain (`Genesis::check`).
    BadGenesis(&'static str),
    /// A block claiming more rounds than `MOST_ROUNDS` at its height.
    TooManyRounds,
    /// A commit from a round before the one its block was put forward in.
    SignedBeforeProposed,
}

/// The most rounds one height may take. A round is one proposer's chance, and a height that
/// needs this many has had every validator absent a thousand times over; the bound is there so
/// a block claiming more cannot make whoever checks it turn the rotation for ever.
pub const MOST_ROUNDS: u32 = 1 << 16;

/// What became of one height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Committed {
    pub height: u64,
    /// The round that committed it — later than the one it was put forward in, if a round
    /// between failed after somebody had signed it.
    pub round: u32,
    pub txs: usize,
    pub proposer: Address,
    /// The share of the power that signed, in parts per thousand.
    pub signed_permille: u64,
}

#[derive(Clone)]
pub struct Chain {
    pub genesis: Genesis,
    pub id: Digest,
    /// Every committed block, genesis first.
    pub blocks: Vec<Block>,
    /// The ledger after the last block.
    pub ledger: Ledger,
    /// Who must sign the next height, and where each stands in the rotation.
    pub rotation: ValidatorSet,
    /// Who signed the last block: the set its header names, and the one a light client holding
    /// that header checks its commit against.
    pub signers: ValidatorSet,
    /// Every set that has signed a block, from the height it first did, with its hash: what a
    /// node hands a light client beside each header.
    sets: Vec<(u64, Digest, ValidatorSet)>,
    /// Transactions waiting for a block, in the order they arrived.
    pool: Vec<Transaction>,
    /// The ledger with the pool applied, which is what a new arrival is checked against — so a
    /// second payment that the first has already made unaffordable is refused at the door.
    pending: Ledger,
    /// Transactions whose signatures this node has already checked.
    checked: BTreeSet<Digest>,
    /// The block validators have signed at the next height in a round that failed, and the
    /// ledger and rotation it leads to: the only block they will sign there, and so the only one
    /// any later round may put forward — this month or, if the height stalls, a later one.
    signed: Option<(Block, Ledger, ValidatorSet)>,
    /// Rounds that ended without a block: an absent proposer, or too few answering.
    pub rounds_failed: u64,
    /// Transactions refused on arrival.
    pub refused: u64,
}

impl Chain {
    /// Begin a chain, if enough of its founders sign its first block.
    pub fn found(genesis: Genesis, founders: &[SigningKey]) -> Result<Chain, Invalid> {
        genesis.check().map_err(Invalid::BadGenesis)?;
        let id = genesis.id();
        let ledger = Ledger::genesis(id, genesis.params.clone(), &genesis.allocations);
        let rotation = ValidatorSet::after(&ValidatorSet::default(), &ledger.validators());
        let header = genesis_header(&genesis, id, &ledger, &rotation);
        let hash = header.hash();
        let votes: Vec<Vote> = founders
            .iter()
            .filter(|key| rotation.find(&key.public()).is_some())
            .map(|key| Vote::signed(key, id, 0, 0, hash))
            .collect();
        let commit = Commit { round: 0, votes };
        rotation
            .check(&commit, id, 0, hash)
            .map_err(Invalid::Commit)?;
        Ok(Chain {
            id,
            blocks: vec![Block {
                header,
                txs: Vec::new(),
                commit,
            }],
            pending: ledger.clone(),
            ledger,
            signers: rotation.clone(),
            sets: vec![(0, rotation.hash(), rotation.clone())],
            rotation,
            genesis,
            pool: Vec::new(),
            checked: BTreeSet::new(),
            signed: None,
            rounds_failed: 0,
            refused: 0,
        })
    }

    pub fn height(&self) -> u64 {
        self.tip().header.height
    }

    pub fn tip(&self) -> &Block {
        self.blocks.last().expect("a chain has at least its genesis")
    }

    pub fn params(&self) -> &Params {
        &self.ledger.params
    }

    pub fn balance(&self, address: &Address, asset: Asset) -> u128 {
        self.ledger.balance(address, asset)
    }

    pub fn token(&self, id: u32) -> Option<&Token> {
        self.ledger.tokens.get(id as usize)
    }

    /// The nonce the next transaction from this address should carry, counting what is
    /// already waiting in the pool.
    pub fn next_nonce(&self, address: &Address) -> u64 {
        self.pending.nonce_of(address)
    }

    /// Who will validate once everything waiting has gone through: address, key and power.
    pub fn pending_validators(&self) -> Vec<(Address, PublicKey, u64)> {
        self.pending.validators()
    }

    /// What an address will hold once everything waiting has gone through.
    pub fn pending_balance(&self, address: &Address, asset: Asset) -> u128 {
        self.pending.balance(address, asset)
    }

    /// An account as it will be once everything waiting has gone through.
    pub fn pending_account(&self, address: &Address) -> Option<&Account> {
        self.pending.account(address)
    }

    pub fn waiting(&self) -> usize {
        self.pool.len()
    }

    /// A token as it will stand once everything waiting has gone through — including one
    /// registered by a transaction still in the pool.
    pub fn pending_token(&self, id: u32) -> Option<&Token> {
        self.pending.tokens.get(id as usize)
    }

    /// How many tokens there will be once everything waiting has gone through.
    pub fn pending_tokens(&self) -> usize {
        self.pending.tokens.len()
    }

    /// Hand a transaction to the chain. It is checked now — signature, then whether it would
    /// apply after everything already waiting — and held until a block takes it.
    pub fn submit(&mut self, tx: Transaction) -> Result<Digest, Refusal> {
        let outcome = self.admit(&tx);
        match outcome {
            Ok(()) => {
                let id = tx.id();
                self.checked.insert(id);
                self.pool.push(tx);
                Ok(id)
            }
            Err(why) => {
                self.refused += 1;
                Err(why)
            }
        }
    }

    fn admit(&mut self, tx: &Transaction) -> Result<(), Refusal> {
        if tx.chain != self.id {
            return Err(Refusal::WrongChain);
        }
        if !self.checked.contains(&tx.id()) && !tx.signature_holds() {
            return Err(Refusal::BadSignature);
        }
        self.pending.apply(tx, Address::default())
    }

    /// The block a proposer would cut at a round: as many waiting transactions as will apply,
    /// in the order they arrived, and the ledger that results.
    pub fn propose(&self, round: u32, time: u64) -> Block {
        let (rotation, at) = self.rotation.at_round(round);
        let proposer = rotation.members[at].address;
        let mut ledger = self.ledger.clone();
        let mut txs = Vec::new();
        for tx in &self.pool {
            if txs.len() >= self.params().max_txs {
                break;
            }
            // `apply` changes nothing unless it succeeds, so a transaction that no longer fits
            // is simply left out.
            if ledger.apply(tx, proposer).is_ok() {
                txs.push(tx.clone());
            }
        }
        let height = self.height() + 1;
        ledger.close(height, proposer, &powers(&self.rotation));
        let next = ValidatorSet::after(&rotation, &ledger.validators());
        let header = Header {
            chain: self.id,
            height,
            round,
            time,
            parent: self.tip().hash(),
            last_commit: self.tip().commit.hash(),
            txs: Block::root_of(&txs),
            tx_count: txs.len() as u32,
            state: ledger.root(),
            validators: self.rotation.hash(),
            next_validators: next.hash(),
            proposer,
        };
        Block {
            header,
            txs,
            commit: Commit::default(),
        }
    }

    /// Everything about a block except its signatures: what an honest validator checks before
    /// it will sign. Returns the ledger and the rotation it leads to.
    pub fn validate(&self, block: &Block) -> Result<(Ledger, ValidatorSet), Invalid> {
        next_state(
            &self.ledger,
            &self.rotation,
            self.tip(),
            self.id,
            block,
            Some(&self.checked),
        )
    }

    /// Append a block whose commit is a quorum.
    pub fn accept(&mut self, block: Block) -> Result<(), Invalid> {
        let (ledger, rotation) = self.validate(&block)?;
        made_final(&self.rotation, &block.commit, &block.header)?;
        self.install(block, ledger, rotation);
        Ok(())
    }

    fn install(&mut self, block: Block, ledger: Ledger, rotation: ValidatorSet) {
        let included: BTreeSet<Digest> = block.txs.iter().map(|t| t.id()).collect();
        let height = block.header.height;
        let named = block.header.validators;
        self.signed = None;
        self.blocks.push(block);
        self.ledger = ledger;
        self.signers = std::mem::replace(&mut self.rotation, rotation);
        if self.sets.last().map(|(_, hash, _)| *hash) != Some(named) {
            self.sets.push((height, named, self.signers.clone()));
        }
        // What is still waiting is re-checked against the new ledger; anything the block made
        // impossible — a second spend of the same coin, a nonce already used — falls out.
        let waiting = std::mem::take(&mut self.pool);
        self.pending = self.ledger.clone();
        for tx in waiting {
            let id = tx.id();
            if included.contains(&id) {
                self.checked.remove(&id);
                continue;
            }
            if self.pending.apply(&tx, Address::default()).is_ok() {
                self.pool.push(tx);
            } else {
                self.checked.remove(&id);
            }
        }
    }

    /// One height, however many rounds it takes.
    ///
    /// Each round the rotation names a proposer. If they are not answering, the round fails.
    /// Otherwise they put a block forward, it is validated, and every validator answering this
    /// round signs it; with more than two thirds of the power signed it is final. `answering` is
    /// the world's say in who is at their post — this function has none.
    ///
    /// A validator signs one block a height, whatever the round. So a round that fails after
    /// anybody has signed binds the rest of the height — the rounds after it here and, if the
    /// height stalls, every call after this one until it commits: every later proposer puts
    /// *that* block forward again rather than cutting one of their own, since nobody who signed
    /// it would sign another. It is what makes a fork, in however many rounds, the work of
    /// validators who signed two blocks at one height — which is what `Action::Evidence`
    /// punishes.
    ///
    /// Returns `None` if no round in `max_rounds` commits, which happens exactly when more than
    /// a third of the power is absent: a BFT chain stops rather than risk being wrong.
    pub fn step(
        &mut self,
        time: u64,
        keys: &BTreeMap<Address, SigningKey>,
        answering: &dyn Fn(&Address, u32) -> bool,
        max_rounds: u32,
    ) -> Option<Committed> {
        // Nobody left to propose: a chain whose every validator has gone never commits again.
        if self.rotation.is_empty() {
            return None;
        }
        for round in 0..max_rounds.min(MOST_ROUNDS) {
            let (rotation, at) = self.rotation.at_round(round);
            let proposer = rotation.members[at].address;
            if !answering(&proposer, round) || !keys.contains_key(&proposer) {
                self.rounds_failed += 1;
                continue;
            }
            let (block, ledger, next) = match self.signed.take() {
                Some(bound) => bound,
                None => {
                    let block = self.propose(round, time);
                    let Ok((ledger, next)) = self.validate(&block) else {
                        self.rounds_failed += 1;
                        continue;
                    };
                    (block, ledger, next)
                }
            };
            let hash = block.hash();
            let votes: Vec<Vote> = self
                .rotation
                .members
                .iter()
                .filter(|v| answering(&v.address, round))
                .filter_map(|v| keys.get(&v.address))
                .map(|key| Vote::signed(key, self.id, block.header.height, round, hash))
                .collect();
            let commit = Commit { round, votes };
            let Ok(power) = self.rotation.check(&commit, self.id, block.header.height, hash) else {
                self.rounds_failed += 1;
                // The proposer signed it if nobody else did, so from here on it is the block.
                self.signed = Some((block, ledger, next));
                continue;
            };
            let committed = Committed {
                height: block.header.height,
                round,
                txs: block.txs.len(),
                proposer: block.header.proposer,
                signed_permille: power * 1000 / self.rotation.total_power().max(1),
            };
            let mut block = block;
            block.commit = commit;
            self.install(block, ledger, next);
            return Some(committed);
        }
        None
    }

    /// Check every block from genesis with nothing taken on trust: every signature on every
    /// transaction and every vote, every root recomputed, every rule re-applied. Returns the
    /// ledger it arrives at, or the height of the first block that fails and why.
    pub fn replay(genesis: &Genesis, blocks: &[Block]) -> Result<Ledger, (u64, Invalid)> {
        genesis.check().map_err(|why| (0, Invalid::BadGenesis(why)))?;
        let id = genesis.id();
        let mut ledger = Ledger::genesis(id, genesis.params.clone(), &genesis.allocations);
        let mut rotation = ValidatorSet::after(&ValidatorSet::default(), &ledger.validators());
        let Some(first) = blocks.first() else {
            return Err((0, Invalid::NotGenesis));
        };
        if first.header != genesis_header(genesis, id, &ledger, &rotation) || !first.txs.is_empty() {
            return Err((0, Invalid::NotGenesis));
        }
        made_final(&rotation, &first.commit, &first.header).map_err(|e| (0, e))?;
        for pair in blocks.windows(2) {
            let (tip, block) = (&pair[0], &pair[1]);
            let height = block.header.height;
            let (next_ledger, next_rotation) =
                next_state(&ledger, &rotation, tip, id, block, None).map_err(|e| (height, e))?;
            made_final(&rotation, &block.commit, &block.header).map_err(|e| (height, e))?;
            ledger = next_ledger;
            rotation = next_rotation;
        }
        Ok(ledger)
    }

    /// `replay` on this chain's own history, and whether it arrives where this node is.
    pub fn verify(&self) -> Result<(), (u64, Invalid)> {
        let ledger = Chain::replay(&self.genesis, &self.blocks)?;
        if ledger != self.ledger {
            return Err((self.height(), Invalid::WrongStateRoot));
        }
        Ok(())
    }

    /// What a light client is handed for one height: its header, the commit that made it final,
    /// and the set that signed it.
    pub fn light_block(&self, height: u64) -> Option<LightBlock> {
        let block = self.blocks.get(height as usize)?;
        let (_, _, validators) = self.sets.iter().rev().find(|(from, _, _)| *from <= height)?;
        Some(LightBlock {
            header: block.header.clone(),
            commit: block.commit.clone(),
            validators: validators.clone(),
        })
    }

    /// Light blocks for every height, genesis first.
    pub fn light_blocks(&self) -> Vec<LightBlock> {
        (0..=self.height()).filter_map(|h| self.light_block(h)).collect()
    }

    /// The whole chain as a file: its genesis and every block (`file`).
    pub fn export(&self) -> Vec<u8> {
        crate::file::write(&self.genesis, &self.blocks)
    }

    /// Where a transaction is: the height of the block holding it.
    pub fn find(&self, id: &Digest) -> Option<u64> {
        self.blocks
            .iter()
            .find(|b| b.txs.iter().any(|t| t.id() == *id))
            .map(|b| b.header.height)
    }
}

/// Whether a commit makes a header final by a set: more than two thirds of its power signed
/// exactly this block, at the header's chain and height, in a round no earlier than the one
/// the block was put forward in. Returns the power that signed.
pub(crate) fn made_final(set: &ValidatorSet, commit: &Commit, header: &Header) -> Result<u64, Invalid> {
    if commit.round < header.round {
        return Err(Invalid::SignedBeforeProposed);
    }
    set.check(commit, header.chain, header.height, header.hash())
        .map_err(Invalid::Commit)
}

fn powers(rotation: &ValidatorSet) -> Vec<(Address, u64)> {
    rotation.members.iter().map(|v| (v.address, v.power)).collect()
}

/// The first header of the chain a genesis begins, which anybody holding the genesis can build.
pub(crate) fn genesis_header_of(genesis: &Genesis) -> Header {
    let id = genesis.id();
    let ledger = Ledger::genesis(id, genesis.params.clone(), &genesis.allocations);
    let rotation = ValidatorSet::after(&ValidatorSet::default(), &ledger.validators());
    genesis_header(genesis, id, &ledger, &rotation)
}

fn genesis_header(genesis: &Genesis, id: Digest, ledger: &Ledger, rotation: &ValidatorSet) -> Header {
    Header {
        chain: id,
        height: 0,
        round: 0,
        time: genesis.time,
        parent: Digest::ZERO,
        last_commit: Digest::ZERO,
        txs: Block::root_of(&[]),
        tx_count: 0,
        state: ledger.root(),
        validators: rotation.hash(),
        next_validators: rotation.hash(),
        proposer: Address::default(),
    }
}

/// The rule, stated once and used both by a live node and by a replay: what a block must be to
/// follow `tip`, and what it leads to.
fn next_state(
    ledger: &Ledger,
    rotation: &ValidatorSet,
    tip: &Block,
    id: Digest,
    block: &Block,
    checked: Option<&BTreeSet<Digest>>,
) -> Result<(Ledger, ValidatorSet), Invalid> {
    let header = &block.header;
    if header.chain != id {
        return Err(Invalid::WrongChain);
    }
    let expected = tip.header.height + 1;
    if header.height != expected {
        return Err(Invalid::NotNext {
            expected,
            got: header.height,
        });
    }
    if header.parent != tip.hash() {
        return Err(Invalid::WrongParent);
    }
    if header.last_commit != tip.commit.hash() {
        return Err(Invalid::WrongLastCommit);
    }
    if header.time <= tip.header.time {
        return Err(Invalid::TimeGoesBackwards);
    }
    if header.validators != rotation.hash() {
        return Err(Invalid::WrongValidators);
    }
    if header.round > MOST_ROUNDS {
        return Err(Invalid::TooManyRounds);
    }
    let (turned, at) = rotation.at_round(header.round);
    if rotation.is_empty() || header.proposer != turned.members[at].address {
        return Err(Invalid::WrongProposer);
    }
    if block.txs.len() > ledger.params.max_txs {
        return Err(Invalid::TooManyTxs);
    }
    if header.tx_count as usize != block.txs.len() || header.txs != Block::root_of(&block.txs) {
        return Err(Invalid::WrongTxRoot);
    }
    let mut next = ledger.clone();
    for (index, tx) in block.txs.iter().enumerate() {
        let known = checked.is_some_and(|c| c.contains(&tx.id()));
        if !known && !tx.signature_holds() {
            return Err(Invalid::BadTxSignature { index });
        }
        next.apply(tx, header.proposer)
            .map_err(|why| Invalid::BadTx { index, why })?;
    }
    next.close(header.height, header.proposer, &powers(rotation));
    if header.state != next.root() {
        return Err(Invalid::WrongStateRoot);
    }
    let next_rotation = ValidatorSet::after(&turned, &next.validators());
    if header.next_validators != next_rotation.hash() {
        return Err(Invalid::WrongValidators);
    }
    Ok((next, next_rotation))
}
