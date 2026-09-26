//! Who holds what, after everything so far.
//!
//! The ledger is a pure function of its genesis and the blocks applied to it: nothing in here
//! reads a clock, a random number or anything outside the transaction in hand. That is what
//! makes every node that applies the same blocks arrive at the same ledger — and what lets a
//! block commit to the ledger it produces, as a single Merkle root, so that "these balances
//! are what the chain says" is a claim anybody can check rather than one anybody has to be
//! trusted for.
//!
//! ## Three conservation laws
//!
//! The tests hold the ledger to these after every block, and they are what a chain *is* in
//! the accounting sense:
//!
//! 1. **Coin is only made by issuance and only destroyed by slashing.** Everything in every
//!    account — liquid, bonded and on its way out — adds up to what genesis created, plus what
//!    has been issued, less what has been burned.
//! 2. **A token's supply is exactly what its holders hold.**
//! 3. **No token is ever minted beyond the reserve its attestor last vouched for.** The
//!    reserve can *fall* below the supply — an attestor stating a shortfall is the chain
//!    working, not failing — but then nothing more can be minted until it is made good.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::Writer;
use crate::consensus::Vote;
use crate::merkle;
use crate::tx::{Action, Address, Asset, COIN, Transaction};
use crate::{Digest, PublicKey};

/// The rules a chain is founded with. Part of what its id is a hash of, so they cannot be
/// changed without making it a different chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    pub name: String,
    /// Coin created per block at first, in base units.
    pub issuance: u128,
    /// Blocks after which issuance halves. A fixed eventual supply, as bitcoin's, rather than
    /// a perpetual inflation — so that what the coin is worth is a question about demand and
    /// not about a schedule.
    pub halving: u64,
    /// The least fee any transaction may pay, in base units: what stops a free ledger being
    /// filled with nothing.
    pub min_fee: u128,
    /// The most transactions in one block.
    pub max_txs: usize,
    /// The most validators at once.
    pub max_validators: usize,
    /// The least stake worth counting as a validator, in base units.
    pub min_bond: u128,
    /// How long unbonded stake stays at risk before it is released, in blocks.
    ///
    /// Without it a validator could sign two blocks and unbond in the same breath, and be gone
    /// with their stake before anybody had shown the two votes to the chain.
    pub unbonding_blocks: u64,
    /// The share of a validator's stake destroyed for signing two blocks at one height, in
    /// parts per thousand.
    pub slash_permille: u64,
}

impl Params {
    pub(crate) fn encode_into(&self, w: &mut Writer) {
        w.text(&self.name)
            .u128(self.issuance)
            .u64(self.halving)
            .u128(self.min_fee)
            .u64(self.max_txs as u64)
            .u64(self.max_validators as u64)
            .u128(self.min_bond)
            .u64(self.unbonding_blocks)
            .u64(self.slash_permille);
    }
}

/// One address's holdings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Account {
    /// How many transactions it has signed. The next must carry exactly this.
    pub nonce: u64,
    /// Coin it can spend.
    pub coin: u128,
    /// Coin it has staked.
    pub bonded: u128,
    /// Coin on its way out of stake: (the height it is released at, how much).
    pub unbonding: Vec<(u64, u128)>,
    /// Stable tokens, by token id. Never holds a zero.
    pub tokens: BTreeMap<u32, u128>,
    /// The key that signs for it, once it has signed anything.
    pub key: Option<PublicKey>,
    /// Caught signing two blocks at once. A jailed account never validates again.
    pub jailed: bool,
}

impl Account {
    fn at_stake(&self) -> u128 {
        self.bonded + self.unbonding.iter().map(|(_, a)| a).sum::<u128>()
    }

    fn encode(&self, address: &Address) -> Vec<u8> {
        let mut w = Writer::tagged("life-rs/chain/account/1");
        w.fixed(&address.0)
            .u64(self.nonce)
            .u128(self.coin)
            .u128(self.bonded)
            .u32(self.unbonding.len() as u32);
        for (release, amount) in &self.unbonding {
            w.u64(*release).u128(*amount);
        }
        w.u32(self.tokens.len() as u32);
        for (token, amount) in &self.tokens {
            w.u32(*token).u128(*amount);
        }
        match &self.key {
            Some(key) => w.u8(1).fixed(&key.0),
            None => w.u8(0),
        };
        w.u8(self.jailed as u8);
        w.finish()
    }
}

/// A stable token: a promise, and the ledger's record of what backs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub symbol: String,
    /// The currency one unit stands for.
    pub peg: String,
    pub issuer: Address,
    pub attestor: Address,
    /// Held by everybody, in base units.
    pub supply: u128,
    /// The reserve as last attested, in base units of the peg.
    pub reserves: u128,
    /// Everything ever minted and ever redeemed, for reading.
    pub minted: u128,
    pub redeemed: u128,
}

impl Token {
    fn encode(&self, id: u32) -> Vec<u8> {
        Writer::tagged("life-rs/chain/token/1")
            .u32(id)
            .text(&self.symbol)
            .text(&self.peg)
            .fixed(&self.issuer.0)
            .fixed(&self.attestor.0)
            .u128(self.supply)
            .u128(self.reserves)
            .u128(self.minted)
            .u128(self.redeemed)
            .finish()
    }

    /// Reserve per unit in circulation. One is fully backed; under one is a promise the issuer
    /// could not keep if everybody asked at once.
    pub fn backing(&self) -> f64 {
        if self.supply == 0 {
            1.0
        } else {
            self.reserves as f64 / self.supply as f64
        }
    }
}

/// Why a transaction was not applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    WrongChain,
    BadSignature,
    /// The account has moved on, or has not got there yet.
    WrongNonce { expected: u64, got: u64 },
    FeeTooLow,
    CannotAfford,
    NothingToMove,
    UnknownToken,
    NotTheIssuer,
    NotTheAttestor,
    /// An issuer vouching for itself is not a reserve anybody has checked.
    SelfAttested,
    BadSymbol,
    /// Minting past what the attestor last said was held.
    BeyondReserves { supply: u128, reserves: u128, asked: u128 },
    NotBonded,
    Jailed,
    BadEvidence,
    AlreadyPunished,
    TooLate,
    /// A swap with oneself, or of an asset for itself.
    NoExchange,
    /// The counterparty's agreement was given at a nonce its account has moved past, or has
    /// not reached.
    StaleConsent { expected: u64, got: u64 },
    /// The counterparty cannot deliver its side.
    CounterpartyCannotAfford,
}

/// Everything, as of the last block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    pub chain: Digest,
    pub params: Params,
    pub accounts: BTreeMap<Address, Account>,
    pub tokens: Vec<Token>,
    /// The height of the last block applied. Genesis is height zero.
    pub height: u64,
    /// All coin in existence, in every state.
    pub coin_supply: u128,
    pub genesis_coin: u128,
    pub issued: u128,
    pub slashed: u128,
    pub fees_paid: u128,
    /// Offences already punished, so the same two votes cannot be shown twice.
    punished: BTreeSet<(Address, u64, u32)>,
}

impl Ledger {
    /// The ledger a chain starts from: some addresses with coin, some of it staked.
    pub fn genesis(chain: Digest, params: Params, allocations: &[(PublicKey, u128, u128)]) -> Ledger {
        let mut accounts: BTreeMap<Address, Account> = BTreeMap::new();
        let mut total = 0;
        for (key, liquid, bonded) in allocations {
            let account = accounts.entry(Address::of(key)).or_default();
            account.coin += liquid;
            account.bonded += bonded;
            account.key = Some(*key);
            total += liquid + bonded;
        }
        Ledger {
            chain,
            params,
            accounts,
            tokens: Vec::new(),
            height: 0,
            coin_supply: total,
            genesis_coin: total,
            issued: 0,
            slashed: 0,
            fees_paid: 0,
            punished: BTreeSet::new(),
        }
    }

    pub fn account(&self, address: &Address) -> Option<&Account> {
        self.accounts.get(address)
    }

    pub fn nonce_of(&self, address: &Address) -> u64 {
        self.accounts.get(address).map(|a| a.nonce).unwrap_or(0)
    }

    pub fn balance(&self, address: &Address, asset: Asset) -> u128 {
        let Some(account) = self.accounts.get(address) else {
            return 0;
        };
        match asset {
            Asset::Coin => account.coin,
            Asset::Token(id) => account.tokens.get(&id).copied().unwrap_or(0),
        }
    }

    /// Coin created by the block at a height: halving on schedule, and nothing once it has
    /// halved to zero.
    pub fn issuance_at(&self, height: u64) -> u128 {
        let halvings = height / self.params.halving.max(1);
        if halvings >= 128 {
            0
        } else {
            self.params.issuance >> halvings
        }
    }

    /// Whether a transaction would apply, without applying it. Its signature is taken as
    /// already checked — that is the one check that does not depend on the ledger, so a node
    /// does it once when a transaction arrives rather than every time it is looked at.
    pub fn admits(&self, tx: &Transaction) -> Result<(), Refusal> {
        self.clone().apply(tx, Address::default())
    }

    /// Apply one transaction whose signature has been checked, paying its fee to `proposer`.
    ///
    /// All or nothing: every check is made before anything changes, so a refused transaction
    /// leaves the ledger exactly as it found it.
    pub fn apply(&mut self, tx: &Transaction, proposer: Address) -> Result<(), Refusal> {
        if tx.chain != self.chain {
            return Err(Refusal::WrongChain);
        }
        let sender = tx.sender();
        let account = self.accounts.get(&sender).cloned().unwrap_or_default();
        if tx.nonce != account.nonce {
            return Err(Refusal::WrongNonce {
                expected: account.nonce,
                got: tx.nonce,
            });
        }
        if tx.fee < self.params.min_fee {
            return Err(Refusal::FeeTooLow);
        }
        if account.coin < tx.fee {
            return Err(Refusal::CannotAfford);
        }
        let spendable = account.coin - tx.fee;

        // Check what the action needs.
        match &tx.action {
            Action::Pay { asset, amount, .. } => {
                if *amount == 0 {
                    return Err(Refusal::NothingToMove);
                }
                match asset {
                    Asset::Coin => {
                        if spendable < *amount {
                            return Err(Refusal::CannotAfford);
                        }
                    }
                    Asset::Token(id) => {
                        if self.tokens.get(*id as usize).is_none() {
                            return Err(Refusal::UnknownToken);
                        }
                        if account.tokens.get(id).copied().unwrap_or(0) < *amount {
                            return Err(Refusal::CannotAfford);
                        }
                    }
                }
            }
            Action::Issue {
                symbol,
                peg,
                attestor,
            } => {
                let fine = |s: &str| {
                    !s.is_empty() && s.len() <= 12 && s.chars().all(|c| c.is_ascii_alphanumeric())
                };
                if !fine(symbol) || !fine(peg) {
                    return Err(Refusal::BadSymbol);
                }
                if *attestor == sender {
                    return Err(Refusal::SelfAttested);
                }
            }
            Action::Attest { token, .. } => {
                let known = self.tokens.get(*token as usize).ok_or(Refusal::UnknownToken)?;
                if known.attestor != sender {
                    return Err(Refusal::NotTheAttestor);
                }
            }
            Action::Mint { token, amount, .. } => {
                let known = self.tokens.get(*token as usize).ok_or(Refusal::UnknownToken)?;
                if known.issuer != sender {
                    return Err(Refusal::NotTheIssuer);
                }
                if *amount == 0 {
                    return Err(Refusal::NothingToMove);
                }
                if known.supply.saturating_add(*amount) > known.reserves {
                    return Err(Refusal::BeyondReserves {
                        supply: known.supply,
                        reserves: known.reserves,
                        asked: *amount,
                    });
                }
            }
            Action::Redeem { token, amount } => {
                if self.tokens.get(*token as usize).is_none() {
                    return Err(Refusal::UnknownToken);
                }
                if *amount == 0 {
                    return Err(Refusal::NothingToMove);
                }
                if account.tokens.get(token).copied().unwrap_or(0) < *amount {
                    return Err(Refusal::CannotAfford);
                }
            }
            Action::Bond { amount } => {
                if *amount == 0 {
                    return Err(Refusal::NothingToMove);
                }
                if account.jailed {
                    return Err(Refusal::Jailed);
                }
                if spendable < *amount {
                    return Err(Refusal::CannotAfford);
                }
            }
            Action::Unbond { amount } => {
                if *amount == 0 {
                    return Err(Refusal::NothingToMove);
                }
                if account.bonded < *amount {
                    return Err(Refusal::NotBonded);
                }
            }
            Action::Evidence { first, second } => {
                self.judge(first, second)?;
            }
            Action::Swap(swap) => {
                let other = Address::of(&swap.counterparty);
                if other == sender || swap.give.0 == swap.get.0 {
                    return Err(Refusal::NoExchange);
                }
                if swap.give.1 == 0 || swap.get.1 == 0 {
                    return Err(Refusal::NothingToMove);
                }
                for (asset, _) in [swap.give, swap.get] {
                    if let Asset::Token(id) = asset
                        && self.tokens.get(id as usize).is_none()
                    {
                        return Err(Refusal::UnknownToken);
                    }
                }
                let theirs = self.accounts.get(&other).cloned().unwrap_or_default();
                if theirs.nonce != swap.counterparty_nonce {
                    return Err(Refusal::StaleConsent {
                        expected: theirs.nonce,
                        got: swap.counterparty_nonce,
                    });
                }
                let holds = |account: &Account, asset: Asset, spendable_coin: u128| match asset {
                    Asset::Coin => spendable_coin,
                    Asset::Token(id) => account.tokens.get(&id).copied().unwrap_or(0),
                };
                if holds(&account, swap.give.0, spendable) < swap.give.1 {
                    return Err(Refusal::CannotAfford);
                }
                if holds(&theirs, swap.get.0, theirs.coin) < swap.get.1 {
                    return Err(Refusal::CounterpartyCannotAfford);
                }
            }
        }

        // Everything checks. Now change things.
        let fee = tx.fee;
        {
            let account = self.accounts.entry(sender).or_default();
            account.nonce += 1;
            account.coin -= fee;
            account.key.get_or_insert(tx.signer);
        }
        self.accounts.entry(proposer).or_default().coin += fee;
        self.fees_paid += fee;

        match &tx.action {
            Action::Pay { to, asset, amount } => match asset {
                Asset::Coin => {
                    self.accounts.get_mut(&sender).expect("sender exists").coin -= amount;
                    self.accounts.entry(*to).or_default().coin += amount;
                }
                Asset::Token(id) => {
                    self.take_tokens(&sender, *id, *amount);
                    *self.accounts.entry(*to).or_default().tokens.entry(*id).or_insert(0) += amount;
                }
            },
            Action::Issue {
                symbol,
                peg,
                attestor,
            } => {
                self.tokens.push(Token {
                    symbol: symbol.clone(),
                    peg: peg.clone(),
                    issuer: sender,
                    attestor: *attestor,
                    supply: 0,
                    reserves: 0,
                    minted: 0,
                    redeemed: 0,
                });
            }
            Action::Attest { token, reserves } => {
                self.tokens[*token as usize].reserves = *reserves;
            }
            Action::Mint { token, to, amount } => {
                let known = &mut self.tokens[*token as usize];
                known.supply += amount;
                known.minted += amount;
                *self
                    .accounts
                    .entry(*to)
                    .or_default()
                    .tokens
                    .entry(*token)
                    .or_insert(0) += amount;
            }
            Action::Redeem { token, amount } => {
                self.take_tokens(&sender, *token, *amount);
                let known = &mut self.tokens[*token as usize];
                known.supply -= amount;
                known.redeemed += amount;
            }
            Action::Bond { amount } => {
                let account = self.accounts.get_mut(&sender).expect("sender exists");
                account.coin -= amount;
                account.bonded += amount;
            }
            Action::Unbond { amount } => {
                let release = self.height + 1 + self.params.unbonding_blocks;
                let account = self.accounts.get_mut(&sender).expect("sender exists");
                account.bonded -= amount;
                account.unbonding.push((release, *amount));
            }
            Action::Swap(swap) => {
                let other = Address::of(&swap.counterparty);
                {
                    let theirs = self.accounts.entry(other).or_default();
                    theirs.nonce += 1;
                    theirs.key.get_or_insert(swap.counterparty);
                }
                self.move_asset(&sender, &other, swap.give);
                self.move_asset(&other, &sender, swap.get);
            }
            Action::Evidence { first, .. } => {
                let offender = Address::of(&first.validator);
                self.punished.insert((offender, first.height, first.round));
                let permille = self.params.slash_permille;
                let account = self.accounts.get_mut(&offender).expect("judged to exist");
                let mut burned = 0;
                let cut = |stake: &mut u128| {
                    let taken = *stake * permille as u128 / 1000;
                    *stake -= taken;
                    taken
                };
                burned += cut(&mut account.bonded);
                for (_, amount) in &mut account.unbonding {
                    burned += cut(amount);
                }
                account.jailed = true;
                self.coin_supply -= burned;
                self.slashed += burned;
            }
        }
        Ok(())
    }

    /// Move an amount of one asset between two accounts that have been checked to hold it.
    fn move_asset(&mut self, from: &Address, to: &Address, (asset, amount): (Asset, u128)) {
        match asset {
            Asset::Coin => {
                self.accounts.get_mut(from).expect("holder exists").coin -= amount;
                self.accounts.entry(*to).or_default().coin += amount;
            }
            Asset::Token(id) => {
                self.take_tokens(from, id, amount);
                *self.accounts.entry(*to).or_default().tokens.entry(id).or_insert(0) += amount;
            }
        }
    }

    fn take_tokens(&mut self, from: &Address, token: u32, amount: u128) {
        let account = self.accounts.get_mut(from).expect("holder exists");
        let held = account.tokens.get_mut(&token).expect("checked above");
        *held -= amount;
        if *held == 0 {
            account.tokens.remove(&token);
        }
    }

    /// Whether two votes prove a validator signed two blocks at once, and whether that is
    /// still something the chain can punish.
    fn judge(&self, first: &Vote, second: &Vote) -> Result<(), Refusal> {
        let same_slot = first.chain == self.chain
            && second.chain == self.chain
            && first.validator == second.validator
            && first.height == second.height
            && first.round == second.round
            && first.block != second.block;
        if !same_slot || !first.signature_holds() || !second.signature_holds() {
            return Err(Refusal::BadEvidence);
        }
        let offender = Address::of(&first.validator);
        if self.punished.contains(&(offender, first.height, first.round)) {
            return Err(Refusal::AlreadyPunished);
        }
        // Stake that has been released can no longer be reached; that is what the unbonding
        // period is for, and evidence older than it is evidence against nothing.
        if first.height + self.params.unbonding_blocks < self.height {
            return Err(Refusal::TooLate);
        }
        match self.accounts.get(&offender) {
            Some(account) if account.at_stake() > 0 => Ok(()),
            _ => Err(Refusal::NotBonded),
        }
    }

    /// The end of a block: issue this height's coin to the validators who kept the chain, in
    /// proportion to their power, and release any stake whose unbonding has run its course.
    pub fn close(&mut self, height: u64, proposer: Address, validators: &[(Address, u64)]) {
        let reward = self.issuance_at(height);
        let total: u64 = validators.iter().map(|(_, p)| p).sum();
        if reward > 0 && total > 0 {
            let mut paid = 0;
            for (address, power) in validators {
                let share = reward * *power as u128 / total as u128;
                self.accounts.entry(*address).or_default().coin += share;
                paid += share;
            }
            // Whatever the division left over goes to whoever proposed, so issuance is exact.
            self.accounts.entry(proposer).or_default().coin += reward - paid;
            self.coin_supply += reward;
            self.issued += reward;
        }
        for account in self.accounts.values_mut() {
            if account.unbonding.is_empty() {
                continue;
            }
            let (due, waiting): (Vec<_>, Vec<_>) =
                account.unbonding.iter().partition(|(release, _)| *release <= height);
            account.coin += due.iter().map(|(_, a)| a).sum::<u128>();
            account.unbonding = waiting;
        }
        self.height = height;
    }

    /// Who may validate after this block, most stake first: bonded at least the minimum, with
    /// a known key, not jailed. Power is whole coins staked.
    pub fn validators(&self) -> Vec<(Address, PublicKey, u64)> {
        let mut eligible: Vec<(Address, PublicKey, u64)> = self
            .accounts
            .iter()
            .filter(|(_, a)| !a.jailed && a.bonded >= self.params.min_bond.max(COIN))
            .filter_map(|(address, a)| {
                a.key
                    .map(|key| (*address, key, (a.bonded / COIN).min(u64::MAX as u128) as u64))
            })
            .collect();
        eligible.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        eligible.truncate(self.params.max_validators);
        eligible
    }

    /// The leaves the state root is built over, in order: every account by address, every
    /// token by id, and the totals.
    fn leaves(&self) -> Vec<Vec<u8>> {
        let mut leaves: Vec<Vec<u8>> = self
            .accounts
            .iter()
            .map(|(address, account)| account.encode(address))
            .collect();
        for (id, token) in self.tokens.iter().enumerate() {
            leaves.push(token.encode(id as u32));
        }
        let mut totals = Writer::tagged("life-rs/chain/totals/1");
        totals
            .u64(self.height)
            .u128(self.coin_supply)
            .u128(self.genesis_coin)
            .u128(self.issued)
            .u128(self.slashed)
            .u128(self.fees_paid)
            .u32(self.punished.len() as u32);
        for (address, height, round) in &self.punished {
            totals.fixed(&address.0).u64(*height).u32(*round);
        }
        leaves.push(totals.finish());
        leaves
    }

    /// Thirty-two bytes that change if anything in the ledger does.
    pub fn root(&self) -> Digest {
        let hashed: Vec<Digest> = self.leaves().iter().map(|l| merkle::leaf(l)).collect();
        merkle::root(&hashed)
    }

    /// Proof that an account holds what it holds, checkable against a block's state root by
    /// somebody who has only the header — which is how a holder of a stable token can show
    /// their balance to anybody without that person keeping a copy of the chain.
    pub fn prove(&self, address: &Address) -> Option<AccountProof> {
        let index = self.accounts.keys().position(|a| a == address)?;
        let leaves: Vec<Digest> = self.leaves().iter().map(|l| merkle::leaf(l)).collect();
        Some(AccountProof {
            address: *address,
            account: self.accounts[address].clone(),
            index,
            size: leaves.len(),
            path: merkle::proof(&leaves, index),
        })
    }

    /// The three conservation laws, checked. `None` when they hold; otherwise which broke.
    pub fn broken_law(&self) -> Option<String> {
        let held: u128 = self
            .accounts
            .values()
            .map(|a| a.coin + a.at_stake())
            .sum();
        if held != self.coin_supply {
            return Some(format!("coin held {held} is not the supply {}", self.coin_supply));
        }
        if self.genesis_coin + self.issued - self.slashed != self.coin_supply {
            return Some("coin appeared or vanished outside issuance and slashing".to_string());
        }
        for (id, token) in self.tokens.iter().enumerate() {
            let held: u128 = self
                .accounts
                .values()
                .map(|a| a.tokens.get(&(id as u32)).copied().unwrap_or(0))
                .sum();
            if held != token.supply {
                return Some(format!("{} held {held} is not its supply {}", token.symbol, token.supply));
            }
            if token.minted - token.redeemed != token.supply {
                return Some(format!("{} minted less redeemed is not its supply", token.symbol));
            }
        }
        None
    }
}

/// An account, and the path from it to a state root.
#[derive(Clone, Debug)]
pub struct AccountProof {
    pub address: Address,
    pub account: Account,
    pub index: usize,
    pub size: usize,
    pub path: Vec<Digest>,
}

impl AccountProof {
    pub fn holds_under(&self, root: &Digest) -> bool {
        let leaf = merkle::leaf(&self.account.encode(&self.address));
        merkle::verify(root, &leaf, self.index, self.size, &self.path)
    }
}

/// Parameters for a chain whose blocks come once a month of world time.
pub fn monthly(name: &str) -> Params {
    Params {
        name: name.to_string(),
        // Fifty coins a block, halving every four years of blocks: twenty-four hundred coins
        // in the first four years and a little under forty-eight hundred ever.
        issuance: 50 * COIN,
        halving: 48,
        min_fee: COIN / 10_000,
        max_txs: 400,
        max_validators: 21,
        min_bond: 10 * COIN,
        unbonding_blocks: 12,
        slash_permille: 50,
    }
}
