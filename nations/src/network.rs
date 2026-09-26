//! A ledger nobody keeps, as a world of nations comes to use one.
//!
//! `chain` is the ledger; this is the world deciding to keep one, and then keeping it. Every
//! year, until one exists, the houses of the world's market towns are asked the three questions
//! `commerce::payments` states — is there a keeper they all trust, are there enough of them,
//! and would checking it cost less than their distrust does — and the first year the answers
//! are no, yes and yes, the largest of them found a chain together: a genesis they all sign,
//! with stake in proportion to their business abroad — except that no country's houses hold the
//! two thirds that would let them finalise a block alone.
//!
//! After that, nothing about it is decided here that the chain does not also check. Once the
//! world has a currency everybody invoices in, the house that issues it registers a stable
//! token on the chain, with a house in another country vouching for its reserve. Twelve times a
//! year a block is cut: the attestor states the reserve, the issuer mints what houses have paid
//! it for, houses — every sizeable state's market house, for its own state's trade — pay each
//! other what their merchants owe abroad, and whatever a house holds beyond what it needs on
//! hand it redeems. Every one of those is a signed transaction the chain
//! refuses if it does not add up, in a block the validators commit only if they can re-derive
//! it. A validator whose town is starving is not at its post; one that simply has a bad day
//! misses a round; and a chain that loses a third of its stake stops until they come back.
//!
//! What the chain changes in the world is the one number it was founded for: what it costs a
//! country to be paid abroad. That is the wedge on every border in the market tree, so a chain
//! that works widens every market it touches.

use std::collections::BTreeMap;

use chain::{
    Action, Address, Asset, COIN, Chain, Digest, Genesis, SigningKey, Swap, TOKEN_UNIT, Transaction, Vote,
};
use commerce::payments::{self, Candidate};
use commerce::production::Sector;
use sim_core::Domain;

use crate::{Event, Nations, WORKING};

/// Blocks a year: one a month of the world's time. Coarse, and deliberately — the mechanism is
/// real and the resolution is coarse, as everywhere else in this workspace.
pub const BLOCKS_A_YEAR: u64 = 12;

/// A twelfth of a Julian year, in seconds.
const MONTH: u64 = 2_629_800;

/// How many rounds a height is given before the block is left for next month.
const MOST_ROUNDS: u32 = 8;

/// Coin created at genesis, staked by the founders.
const GENESIS_COINS: u128 = 10_000;

/// The least any founder stakes, so every founder is a validator from the first block.
const FOUNDING_STAKE: u128 = 100;

/// The typical payment abroad, in years of the world's income per head: a merchant's
/// consignment. What a validator's checking is paid out of, per payment.
pub const PAYMENT_SIZE: f64 = 3.0;

/// The chance a validator misses a round for nothing to do with the world: a courier who did
/// not arrive, a fire in the counting house.
const DOWNTIME: f64 = 0.01;

/// A validator whose town is this short of food is not at its post.
const TOO_HUNGRY: f64 = 0.3;

/// The chance, each time a validator signs a block, that its house signs that height twice. A
/// house keeps two clerks at its keys, so that one of them falling ill never costs it a round;
/// once in a long while both are at their posts, and the one who has not seen the proposal in
/// time signs for no block. Two machines holding one key is how validators on real chains are
/// slashed, far more often than by plotting. Chosen, not derived: about once in two and a half
/// thousand years of a house validating.
const TWO_CLERKS: f64 = 1.0 / 30_000.0;

/// Where the draws for it start, among the world's commercial streams.
const CLERKS: u64 = 0xc1e2_0000;

/// The share of a year's settlements a house keeps in tokens rather than redeeming.
const FLOAT: f64 = 0.1;

/// A country whose share of what a chain carries reaches this takes a seat among its
/// validators, rather than leave its payments to be checked by others.
const SEAT_AT: f64 = 0.05;

/// The most of a chain's stake the houses of one country may hold: short of the two thirds that
/// would let them finalise a block with nobody else signing. Nobody abroad joins a ledger one
/// country could keep on its own, so the founders settle it between them — the largest country
/// takes less stake than its business would give it, and every block needs somebody abroad.
pub const ONE_COUNTRY_AT_MOST: f64 = 0.6;

/// How much of a house's standing on a chain one year's business sets: its share of what the
/// chain carries, averaged over about five years. Read afresh each year, a small state that
/// merged into its neighbour and split off again took a seat and gave it up thirteen times in
/// three centuries, which is the oscillation §31.1 warns a decision read off a moving quantity
/// always has.
const STANDING_MEMORY: f64 = 0.2;

/// A validator whose standing falls below this share of a chain's business takes its stake
/// back: a house that hardly trades abroad any more has no reason to check other houses'
/// payments, and its stake is better sold to one that does. A fifth of what it takes to join,
/// so the two do not chase each other.
const LEAVE_BELOW: f64 = SEAT_AT / 5.0;

/// A state that makes this share of its country's product has its own market house on a chain,
/// paying and being paid for its own trade abroad. A smaller one's trade goes through the
/// capital's house.
const OWN_HOUSE: f64 = 0.05;

/// The least share of the trade between two countries that two houses settle with each other
/// directly. Less than that, and the paying house sends it to the largest house on the other
/// side, as a small bank pays through a large one abroad rather than keep an account with
/// every small one.
const DIRECT: f64 = 0.02;

/// How unevenly a year's payments between two houses fall across its months: orders come in
/// lumps, not a twelfth at a time. The spread of a month's payments about the year's mean, in
/// logs.
const LUMPY: f64 = 0.4;

/// Where the draws for those lumps start, among the world's commercial streams.
const LUMPS: u64 = 1 << 48;

/// The stable token on a chain.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub id: u32,
    pub symbol: String,
    /// The currency one token stands for.
    pub currency: usize,
    /// The house that mints and redeems it, and the house that vouches for its reserve.
    pub issuer: usize,
    pub attestor: usize,
    /// What the issuer holds against it, in units of its currency.
    pub reserves: f64,
}

/// A chain, and the houses that use it.
pub struct Network {
    pub name: String,
    pub founded: u64,
    pub chain: Chain,
    /// Every house with an account, by town, with the key it signs with.
    keys: BTreeMap<usize, SigningKey>,
    /// The keys a house stakes and validates with once the chain has jailed its own, newest
    /// last: a house caught signing twice keeps its account — its coin, its tokens, whatever it
    /// issues or vouches for — and if it validates again, does so with a key kept for nothing
    /// else, as an operator on a real chain comes back under a new consensus key.
    stakers: BTreeMap<usize, Vec<SigningKey>>,
    pub founders: Vec<usize>,
    pub token: Option<Token>,
    /// Share of each pair of countries' payments settled here, by country keys.
    shares: BTreeMap<(usize, usize), f64>,
    /// What a payment settled here costs, as a share of it.
    pub cost: f64,
    /// Units of the token's currency one coin fetches.
    pub coin_price: f64,
    /// Settled here in the last year, in years of food, and what that paid in fees.
    pub carried: f64,
    pub fees: f64,
    pub stalls: u64,
    pub refused: u64,
    /// Why transactions were refused, by the ledger's own reason.
    pub refusals: BTreeMap<String, u64>,
    pub blocks_this_year: u64,
    /// What each house paid and was paid on the chain last year, in years of food.
    pub business: BTreeMap<usize, f64>,
    /// The same, so far this year.
    working: BTreeMap<usize, f64>,
    /// Each house's share of the business here, averaged over the years (`STANDING_MEMORY`).
    pub standing: BTreeMap<usize, f64>,
    /// The books as they stood when this year began, with the height they are for: what any
    /// node could hand one joining late, which would check them against that height's header
    /// and replay the year from them (`chain::join`).
    pub books: Option<(u64, Vec<Vec<u8>>)>,
}

impl Network {
    /// The share of a pair of countries' payments settled here.
    pub fn share_of(&self, pair: (usize, usize)) -> f64 {
        self.shares.get(&pair).copied().unwrap_or(0.0)
    }

    pub fn address_of(&self, town: usize) -> Option<Address> {
        self.keys.get(&town).map(|k| Address::of(&k.public()))
    }

    pub fn houses(&self) -> impl Iterator<Item = usize> + '_ {
        self.keys.keys().copied()
    }

    /// Every key a house has signed with: its own, then any it has staked with.
    fn keys_of(&self, town: usize) -> impl Iterator<Item = &SigningKey> + '_ {
        self.keys
            .get(&town)
            .into_iter()
            .chain(self.stakers.get(&town).into_iter().flatten())
    }

    /// The key a house stakes and validates with: its own, until the chain jails it, and after
    /// that the newest it keeps for staking.
    fn staking_key(&self, town: usize) -> Option<&SigningKey> {
        self.stakers
            .get(&town)
            .and_then(|keys| keys.last())
            .or_else(|| self.keys.get(&town))
    }

    /// Every address a house holds: its own first, then any it has staked with, oldest first.
    pub fn addresses_of(&self, town: usize) -> Vec<Address> {
        self.keys_of(town).map(|k| Address::of(&k.public())).collect()
    }

    /// The address a house's stake is bonded from.
    pub fn staking_address(&self, town: usize) -> Option<Address> {
        self.staking_key(town).map(|k| Address::of(&k.public()))
    }

    /// The towns whose houses validate now.
    pub fn validators(&self) -> Vec<usize> {
        self.keys
            .keys()
            .copied()
            .filter(|t| self.keys_of(*t).any(|k| self.chain.rotation.find(&k.public()).is_some()))
            .collect()
    }

    /// The town whose house holds an address, if any does — by its own key or one it stakes with.
    pub fn town_of(&self, address: &Address) -> Option<usize> {
        self.keys
            .keys()
            .copied()
            .find(|t| self.keys_of(*t).any(|k| Address::of(&k.public()) == *address))
    }
}

/// The key a town's house signs with on a chain: derived from the world's seed, so a world
/// founded twice signs the same history twice.
fn key_for(nations: &Nations, network: usize, town: usize) -> SigningKey {
    key_of_generation(nations, network, town, 0)
}

/// The same for the keys a house stakes with after its own is jailed, the first of them
/// generation one.
fn key_of_generation(nations: &Nations, network: usize, town: usize, generation: u64) -> SigningKey {
    let mut rng = nations.seed.stream(
        Domain::Commerce,
        0x04e1_0000 + network as u64,
        town as u64 | generation << 32,
    );
    let mut seed = [0u8; 32];
    for chunk in seed.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    SigningKey::from_seed(seed)
}

/// A year of every ledger: founding one if it has become worth founding, then twelve blocks of
/// each.
pub(crate) fn year(nations: &mut Nations) {
    if nations.networks.is_empty() && nations.chains_are_possible {
        consider_founding(nations);
    }
    for at in 0..nations.networks.len() {
        keep(nations, at);
    }
}

/// The houses that would found a chain: the ones that would carry their country's trade on it —
/// every state's own market house where the state is large enough to have one, and the
/// capital's for the rest, as `carriers` settles it once there is a chain — each with the part of
/// its country's trade it would carry. A small state's hub founding a chain its trade would never
/// run through left again within a few years, nine founders of fifteen on 0x11.
fn candidates(nations: &Nations) -> (Vec<usize>, Vec<Candidate>) {
    let mut towns = Vec::new();
    let mut found = Vec::new();
    for (c, country) in nations.countries.iter().enumerate() {
        if country.product <= 0.0 {
            continue;
        }
        let mut carried: BTreeMap<usize, f64> = BTreeMap::new();
        for (hub, share) in state_shares(nations, c) {
            let house = if share >= OWN_HOUSE { hub } else { country.capital };
            *carried.entry(house).or_insert(0.0) += share;
        }
        for (house, share) in carried {
            let town = &nations.towns[house];
            towns.push(house);
            found.push(Candidate {
                country: c,
                reckoners: town.workers()[Sector::Reckoning as usize],
                technique: town.technique[Sector::Reckoning as usize],
                volume: country.exports * share,
            });
        }
    }
    (towns, found)
}

/// What a year's income per worker is, across the world: what a validator's clerks cost.
fn wage(nations: &Nations) -> f64 {
    let people = nations.people();
    if people <= 0.0 {
        return 1.0;
    }
    let product: f64 = nations.towns.iter().map(|t| t.product()).sum();
    product / (people * WORKING)
}

fn payment_size(nations: &Nations) -> f64 {
    let people = nations.people().max(1.0);
    let product: f64 = nations.towns.iter().map(|t| t.product()).sum();
    PAYMENT_SIZE * (product / people).max(1.0)
}

/// How far one house trusts another to keep a ledger: as their countries trust each other, or,
/// within one country, as far as distance lets people who share everything else.
fn house_trust(nations: &Nations, trust: &[Vec<f64>], a: usize, b: usize) -> f64 {
    let (ca, cb) = (nations.towns[a].country, nations.towns[b].country);
    if ca != cb {
        return trust[ca][cb];
    }
    payments::trust(payments::trust_ceiling(true, nations.distance_km(a, b)), 1.0)
}

/// Founders' stakes, in coin, with no country's houses holding more than `ONE_COUNTRY_AT_MOST`
/// of the whole: a country over it has its houses' stakes scaled down to it, and what it gives
/// up goes to the others in proportion to what they hold, until none is over. `founders` is
/// each founder's country and the stake its business would have given it.
pub(crate) fn no_country_keeps_it(founders: &[(usize, f64)]) -> Vec<f64> {
    let mut stakes: Vec<f64> = founders.iter().map(|(_, s)| s.max(0.0)).collect();
    let total: f64 = stakes.iter().sum();
    let countries: std::collections::BTreeSet<usize> = founders.iter().map(|(c, _)| *c).collect();
    if total <= 0.0 || countries.len() < 2 {
        return stakes;
    }
    let most = ONE_COUNTRY_AT_MOST * total;
    let mut capped = std::collections::BTreeSet::new();
    // Each pass caps at least one more country or finds none over, so it ends.
    for _ in 0..countries.len() {
        let held = |c: usize, stakes: &[f64]| -> f64 {
            founders
                .iter()
                .zip(stakes)
                .filter(|((k, _), _)| *k == c)
                .map(|(_, s)| s)
                .sum()
        };
        let over: Vec<usize> = countries
            .iter()
            .copied()
            .filter(|c| held(*c, &stakes) > most * (1.0 + 1e-12))
            .collect();
        if over.is_empty() {
            break;
        }
        let mut freed = 0.0;
        for c in &over {
            let scale = most / held(*c, &stakes);
            for ((k, _), stake) in founders.iter().zip(stakes.iter_mut()) {
                if k == c {
                    freed += *stake * (1.0 - scale);
                    *stake *= scale;
                }
            }
            capped.insert(*c);
        }
        let others: f64 = founders
            .iter()
            .zip(&stakes)
            .filter(|((k, _), _)| !capped.contains(k))
            .map(|(_, s)| s)
            .sum();
        if others <= 0.0 {
            break;
        }
        for ((k, _), stake) in founders.iter().zip(stakes.iter_mut()) {
            if !capped.contains(k) {
                *stake += freed * *stake / others;
            }
        }
    }
    stakes
}

fn consider_founding(nations: &mut Nations) {
    // A world of one country has no border to pay across, and a house everybody can pay through.
    if nations.countries.len() < 2 {
        nations.not_yet = Some(payments::NotYet::OneCountry);
        return;
    }
    // A chain carries a stable token, and a stable token needs a currency to stand for.
    let Some(reserve) = nations.reserve else {
        nations.not_yet = Some(payments::NotYet::TooFew);
        return;
    };
    let (towns, candidates) = candidates(nations);
    if candidates.len() < payments::FEWEST_FOUNDERS {
        nations.not_yet = Some(payments::NotYet::TooFew);
        return;
    }
    let trust = nations.country_trust();
    let volume: f64 = candidates.iter().map(|c| c.volume).sum();
    if volume <= 0.0 {
        nations.not_yet = Some(payments::NotYet::TooFew);
        return;
    }
    let bank: f64 = candidates
        .iter()
        .map(|c| c.volume * nations.countries[c.country].pay_cost)
        .sum::<f64>()
        / volume;
    let validators = candidates.len().min(payments::MOST_FOUNDERS);
    let pairs = nations.payments.len().max(1) as f64;
    let checks = BLOCKS_A_YEAR as f64 * (2.0 * pairs + 3.0 * validators as f64 + validators as f64);
    let wage = wage(nations);
    let size = payment_size(nations);
    let decision = payments::worth_founding(
        &candidates,
        &|a, b| house_trust(nations, &trust, towns[a], towns[b]),
        checks,
        bank,
        size,
        wage,
    );
    let founding = match decision {
        Ok(founding) => founding,
        Err(why) => {
            nations.not_yet = Some(why);
            return;
        }
    };
    nations.not_yet = None;
    let techniques: Vec<f64> = founding
        .founders
        .iter()
        .map(|i| candidates[*i].technique)
        .collect();
    let chain_cost = payments::chain_cost(size, &techniques, wage, 1);

    let network = nations.networks.len();
    let founders: Vec<usize> = founding.founders.iter().map(|i| towns[*i]).collect();
    let keys: Vec<SigningKey> = founders.iter().map(|t| key_for(nations, network, *t)).collect();
    let founding_volume: f64 = founding.founders.iter().map(|i| candidates[*i].volume).sum();
    let spare = GENESIS_COINS - FOUNDING_STAKE * founders.len() as u128;
    let stakes = no_country_keeps_it(
        &founding
            .founders
            .iter()
            .map(|i| {
                let share = candidates[*i].volume / founding_volume.max(1e-9);
                (
                    candidates[*i].country,
                    FOUNDING_STAKE as f64 + spare as f64 * share,
                )
            })
            .collect::<Vec<_>>(),
    );
    // Beside its stake, each founder holds coin to pay fees with: as much as fee-paying makes
    // up of what the coin is worth. At a founding, when checking is still dear, that is about
    // as much again as the stake — a tenth, which is what founders once kept, ran out in the
    // first month and left houses refused for want of a fee for two years.
    let for_fees = payments::held_for_fees(chain_cost).min(0.9);
    let liquid = for_fees / (1.0 - for_fees);
    let allocations: Vec<(chain::PublicKey, u128, u128)> = stakes
        .iter()
        .zip(&keys)
        .map(|(coins, key)| {
            let stake = (coins * COIN as f64) as u128;
            (key.public(), (stake as f64 * liquid) as u128, stake)
        })
        .collect();
    let genesis_coins: u128 = allocations.iter().map(|(_, coin, stake)| coin + stake).sum::<u128>() / COIN;
    // Named for the town whose house did the most business among its founders, as a country is
    // named for its largest place.
    let name = format!("{} Ledger", nations.towns[founders[0]].name);
    let mut params = chain::state::monthly(&name);
    params.max_txs = 4_000;
    let genesis = Genesis {
        params,
        time: nations.year * BLOCKS_A_YEAR * MONTH,
        allocations,
    };
    let Ok(chain) = Chain::found(genesis, &keys) else {
        return;
    };
    let reserve_level = nations.currencies[reserve].level;
    let volume_in_currency = founding_volume * reserve_level;
    let coin_price = payments::coin_value(volume_in_currency, 0.0, genesis_coins as f64);
    nations.networks.push(Network {
        name,
        founded: nations.year,
        chain,
        keys: founders.iter().copied().zip(keys).collect(),
        stakers: BTreeMap::new(),
        founders,
        token: None,
        shares: BTreeMap::new(),
        cost: chain_cost,
        coin_price: coin_price.max(1e-9),
        carried: 0.0,
        fees: 0.0,
        stalls: 0,
        refused: 0,
        refusals: BTreeMap::new(),
        blocks_this_year: 0,
        business: BTreeMap::new(),
        working: BTreeMap::new(),
        // Every founder starts with the share of the business that made it one.
        standing: founding
            .founders
            .iter()
            .map(|i| (towns[*i], candidates[*i].volume / founding_volume.max(1e-9)))
            .collect(),
        books: None,
    });
    nations.history.push(Event::Founded {
        year: nations.year,
        network,
    });
}

/// A year of one chain.
fn keep(nations: &mut Nations, at: usize) {
    let Some(reserve) = nations.reserve else {
        return;
    };
    let network = &mut nations.networks[at];
    network.books = Some((network.chain.height(), network.chain.ledger.snapshot()));
    let wage = wage(nations);
    let size = payment_size(nations);
    // What the chain counts in is its token's currency — the one the world invoiced in when the
    // token was issued — whatever it invoices in since: the token stands for that currency and
    // no other, so a payment abroad becomes so many of its units at that currency's price.
    let counted_in = nations.networks[at].token.as_ref().map_or(reserve, |t| t.currency);
    let reserve_level = nations.currencies[counted_in].level.max(1e-9);

    // Every country that trades abroad keeps an account through its capital's house, and so
    // does every state of it large enough to have a market house of its own.
    for c in 0..nations.countries.len() {
        if nations.countries[c].exports <= 0.0 {
            continue;
        }
        let mut houses = vec![nations.countries[c].capital];
        houses.extend(
            state_shares(nations, c)
                .into_iter()
                .filter(|(_, share)| *share >= OWN_HOUSE)
                .map(|(hub, _)| hub),
        );
        for town in houses {
            if !nations.networks[at].keys.contains_key(&town) {
                let key = key_for(nations, at, town);
                nations.networks[at].keys.insert(town, key);
            }
        }
    }

    // What settling here costs now: every validator checks every payment, each at its own
    // reckoning.
    let techniques = validator_techniques(nations, at);
    let cost = payments::chain_cost(size, &techniques, wage, 1);
    nations.networks[at].cost = cost;

    // How much of each pair's business moves here: towards the share of the cost it saves.
    let trust = nations.country_trust();
    let pairs: Vec<((usize, usize), f64)> = nations.payments.iter().map(|(p, v)| (*p, *v)).collect();
    for (pair, _) in &pairs {
        let (Some(a), Some(b)) = (
            nations.countries.iter().position(|c| c.key == pair.0),
            nations.countries.iter().position(|c| c.key == pair.1),
        ) else {
            continue;
        };
        let both_here = nations.networks[at].keys.contains_key(&nations.countries[a].capital)
            && nations.networks[at].keys.contains_key(&nations.countries[b].capital);
        if !both_here {
            continue;
        }
        let bank = payments::cheapest_route(&trust, a, b, nations.fx(a, b)).0;
        let now = nations.networks[at].share_of(*pair);
        let next = payments::adoption(now, bank, cost);
        nations.networks[at].shares.insert(*pair, next);
    }

    // What the coin is worth this year: the stake worth holding against what the chain is
    // expected to carry, and the coin people must hold to pay what checking it will cost. Priced
    // on what is expected rather than on last year, or a chain that carried nothing last year
    // would price its coin at nothing and its fees at more coin than exists.
    let per_payment: f64 = validator_techniques(nations, at)
        .iter()
        .map(|t| payments::check_cost(*t))
        .sum::<f64>()
        * wage;
    let expected: f64 = pairs
        .iter()
        .map(|(pair, volume)| nations.networks[at].share_of(*pair) * volume)
        .sum();
    let expected_fees = expected / size.max(1e-9) * per_payment;
    let coins = nations.networks[at].chain.ledger.coin_supply as f64 / COIN as f64;
    let price = payments::coin_value(expected * reserve_level, expected_fees * reserve_level, coins);
    if price > 0.0 {
        nations.networks[at].coin_price = price;
    }

    issue_token(nations, at, reserve);
    tend_stakes(nations, at);
    leave_idle(nations, at);
    take_seats(nations, at);

    // Twelve months.
    let mut carried = 0.0;
    let mut fees = 0.0;
    nations.networks[at].blocks_this_year = 0;
    for month in 0..BLOCKS_A_YEAR {
        mend(nations, at);
        let (value, paid) = settle_month(nations, at, &pairs, month, size, wage, reserve_level);
        carried += value;
        fees += paid;
        let time = (nations.year * BLOCKS_A_YEAR + month + 1) * MONTH;
        let answering = who_answers(nations, at, month);
        let network = &nations.networks[at];
        let keys: BTreeMap<Address, SigningKey> = network
            .keys
            .values()
            .chain(network.stakers.values().flatten())
            .map(|k| (Address::of(&k.public()), k.clone()))
            .collect();
        let outcome = nations.networks[at].chain.step(
            time,
            &keys,
            &|address, round| answering.get(&(*address, round)).copied().unwrap_or(false),
            MOST_ROUNDS,
        );
        match outcome {
            Some(committed) => {
                nations.networks[at].blocks_this_year += 1;
                for (town, first, second) in signed_twice(nations, at, committed.height, month) {
                    accuse(nations, at, town, first, second);
                }
            }
            None => {
                let network = &mut nations.networks[at];
                network.stalls += 1;
                let height = network.chain.height();
                nations.history.push(Event::Stalled {
                    year: nations.year,
                    network: at,
                    height,
                });
            }
        }
    }
    let network = &mut nations.networks[at];
    network.carried = carried;
    network.fees = fees;
    network.refused = network.chain.refused;
    network.business = std::mem::take(&mut network.working);
    let total: f64 = network.business.values().sum();
    if total > 0.0 {
        let houses: Vec<usize> = network.houses().collect();
        for town in houses {
            let share = network.business.get(&town).copied().unwrap_or(0.0) / total;
            let standing = network.standing.entry(town).or_insert(0.0);
            *standing += STANDING_MEMORY * (share - *standing);
        }
    }
}

/// The reckoning technique of each validator's town.
fn validator_techniques(nations: &Nations, at: usize) -> Vec<f64> {
    nations.networks[at]
        .validators()
        .iter()
        .map(|t| nations.towns[*t].technique[Sector::Reckoning as usize])
        .collect()
}

/// Who is at their post, round by round, this month: nobody from a starving town, and
/// otherwise everybody but the unlucky.
fn who_answers(nations: &Nations, at: usize, month: u64) -> BTreeMap<(Address, u32), bool> {
    let network = &nations.networks[at];
    let mut rng = nations.seed.stream(
        Domain::Commerce,
        0xa115_0000 + at as u64,
        nations.year * BLOCKS_A_YEAR + month,
    );
    let mut answers = BTreeMap::new();
    // Houses' own keys first and staking keys after, so a world with none of the second draws
    // exactly what it drew before there were any.
    let staking = network
        .stakers
        .iter()
        .filter_map(|(town, keys)| Some((town, keys.last()?)));
    for (town, key) in network.keys.iter().chain(staking) {
        let address = Address::of(&key.public());
        let fed = nations.towns[*town].hunger < TOO_HUNGRY;
        for round in 0..MOST_ROUNDS {
            answers.insert((address, round), fed && !rng.chance(DOWNTIME));
        }
    }
    answers
}

/// A house that signs a transaction: its key, and the nonce it is up to.
fn sign(network: &Network, town: usize, fee: u128, action: Action) -> Option<Transaction> {
    sign_with(network, network.keys.get(&town)?, fee, action)
}

/// The same with a particular key, such as the one a house stakes with.
fn sign_with(network: &Network, key: &SigningKey, fee: u128, action: Action) -> Option<Transaction> {
    let nonce = network.chain.next_nonce(&Address::of(&key.public()));
    Some(Transaction::signed(key, network.chain.id, nonce, fee, action))
}

fn submit(network: &mut Network, tx: Option<Transaction>) -> bool {
    let Some(tx) = tx else {
        *network.refusals.entry("no account".to_string()).or_insert(0) += 1;
        return false;
    };
    let what = tx.action.label();
    match network.chain.submit(tx) {
        Ok(_) => true,
        Err(why) => {
            let reason = format!("{what}: {why:?}");
            let reason = reason.split(['{', '(']).next().unwrap_or("").trim().to_string();
            *network.refusals.entry(reason).or_insert(0) += 1;
            false
        }
    }
}

/// Once the world has a currency everybody invoices in, the house that issues it registers a
/// token standing for it, and a house of another country agrees to vouch for its reserve.
fn issue_token(nations: &mut Nations, at: usize, reserve: usize) {
    if nations.networks[at].token.is_some() {
        return;
    }
    let issuer = nations.issuers[reserve];
    let Some(issuer_country) = nations.countries.iter().position(|c| c.towns.contains(&issuer)) else {
        return;
    };
    // The issuer needs an account; the attestor is the house abroad that trusts it most.
    if !nations.networks[at].keys.contains_key(&issuer) {
        let key = key_for(nations, at, issuer);
        nations.networks[at].keys.insert(issuer, key);
    }
    let trust = nations.country_trust();
    let attestor = nations.networks[at]
        .keys
        .keys()
        .copied()
        .filter(|t| nations.towns[*t].country != issuer_country)
        .max_by(|a, b| {
            trust[nations.towns[*a].country][issuer_country]
                .total_cmp(&trust[nations.towns[*b].country][issuer_country])
                .then(b.cmp(a))
        });
    let Some(attestor) = attestor else {
        return;
    };
    fund(nations, at, issuer, 5 * COIN);
    fund(nations, at, attestor, 5 * COIN);
    let network = &mut nations.networks[at];
    let min_fee = network.chain.params().min_fee;
    let symbol = format!("{}T", nations.currencies[reserve].symbol);
    let Some(attestor_address) = network.address_of(attestor) else {
        return;
    };
    let tx = sign(
        network,
        issuer,
        min_fee,
        Action::Issue {
            symbol: symbol.clone(),
            peg: nations.currencies[reserve].symbol.clone(),
            attestor: attestor_address,
        },
    );
    if !submit(network, tx) {
        network.refused += 1;
        return;
    }
    let id = network.chain.pending_tokens() as u32 - 1;
    network.token = Some(Token {
        id,
        symbol: symbol.clone(),
        currency: reserve,
        issuer,
        attestor,
        reserves: 0.0,
    });
    nations.history.push(Event::Issued {
        year: nations.year,
        network: at,
        symbol,
    });
}

/// What coin costs on a chain, in its stable token: which token, and how many of its base units
/// one base unit of coin fetches.
#[derive(Clone, Copy)]
struct Price {
    token: u32,
    per_unit: f64,
}

impl Price {
    fn now(network: &Network) -> Option<Price> {
        let token = network.token.as_ref()?;
        Some(Price {
            token: token.id,
            per_unit: network.coin_price * TOKEN_UNIT as f64 / COIN as f64,
        })
    }

    /// Tokens for so much coin, rounded down, so a seller is never paid more than the price.
    fn of(&self, coin: u128) -> u128 {
        (coin as f64 * self.per_unit).max(0.0) as u128
    }

    /// Coin so many tokens buy, rounded down.
    fn buys(&self, tokens: u128) -> u128 {
        if self.per_unit <= 0.0 {
            return 0;
        }
        (tokens as f64 / self.per_unit).max(0.0) as u128
    }
}

/// A house short of coin buys it from the houses that have most to spare, largest first, until
/// it has enough or nobody has any left to sell. Nobody sells what they need themselves: what
/// every house needs this month is `keeping`.
///
/// Where there is a stable token and the buyer holds it, the buyer pays in it, in a swap both
/// of them sign — the seller's coin and the buyer's tokens change hands at once or not at all.
/// What tokens do not cover is paid for over the counter, off the chain, which is how a house
/// with nothing on the chain yet gets its first coin. Returns what each seller was paid in
/// tokens.
fn fund(nations: &mut Nations, at: usize, town: usize, wanted: u128) -> Vec<(usize, u128)> {
    let price = Price::now(&nations.networks[at]);
    fund_keeping(nations, at, town, wanted, &BTreeMap::new(), price, 0)
}

/// `owing` is what the buyer must still pay others in tokens this month, which it never spends
/// on coin.
fn fund_keeping(
    nations: &mut Nations,
    at: usize,
    town: usize,
    wanted: u128,
    keeping: &BTreeMap<usize, u128>,
    price: Option<Price>,
    owing: u128,
) -> Vec<(usize, u128)> {
    let mut paid = Vec::new();
    let network = &mut nations.networks[at];
    let Some(address) = network.address_of(town) else {
        return paid;
    };
    let min_fee = network.chain.params().min_fee;
    let mut sellers: Vec<(usize, u128)> = network
        .keys
        .keys()
        .copied()
        .filter(|t| *t != town)
        .filter_map(|t| {
            let a = network.address_of(t)?;
            Some((t, network.chain.pending_balance(&a, Asset::Coin)))
        })
        .collect();
    sellers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (seller, balance) in sellers {
        let have = network.chain.pending_balance(&address, Asset::Coin);
        if have >= wanted {
            break;
        }
        // A seller keeps a coin and a fee for itself, and whatever it needs this month.
        let own = keeping.get(&seller).copied().unwrap_or(0);
        let spare = balance.saturating_sub(min_fee + COIN + own);
        let amount = (wanted - have).min(spare);
        if amount == 0 {
            continue;
        }
        // As much as the buyer's tokens pay for, in a swap; the rest over the counter.
        let mut swapped = 0;
        if let Some(price) = price
            && let Some(seller_address) = network.address_of(seller)
            && let Some(buyer_key) = network.keys.get(&town)
        {
            let tokens = network
                .chain
                .pending_balance(&address, Asset::Token(price.token))
                .saturating_sub(owing);
            let coin = amount.min(price.buys(tokens));
            let cost = price.of(coin);
            if coin > 0 && cost > 0 {
                let agreed = Swap::agreed(
                    buyer_key,
                    network.chain.id,
                    seller_address,
                    network.chain.next_nonce(&address),
                    (Asset::Coin, coin),
                    (Asset::Token(price.token), cost),
                );
                let tx = sign(network, seller, min_fee, Action::Swap(Box::new(agreed)));
                if submit(network, tx) {
                    swapped = coin;
                    paid.push((seller, cost));
                } else {
                    network.refused += 1;
                }
            }
        }
        if amount > swapped {
            let tx = sign(
                network,
                seller,
                min_fee,
                Action::Pay {
                    to: address,
                    asset: Asset::Coin,
                    amount: amount - swapped,
                },
            );
            if !submit(network, tx) {
                network.refused += 1;
            }
        }
    }
    paid
}

/// A house whose business on a chain has grown large enough takes a seat among its validators:
/// it buys stake from those who have it and bonds it — within what keeps its country's houses
/// short of `ONE_COUNTRY_AT_MOST` of the stake, and only if it checks as fast as the others.
fn take_seats(nations: &mut Nations, at: usize) {
    let most = nations.networks[at].chain.params().max_validators;
    let standing = nations.networks[at].standing.clone();
    let best = best_technique(nations, at);
    let houses: Vec<usize> = nations.networks[at].houses().collect();
    for town in houses {
        let validators = nations.networks[at].validators();
        if validators.len() >= most {
            return;
        }
        if validators.contains(&town) {
            continue;
        }
        let share = standing.get(&town).copied().unwrap_or(0.0);
        if share < SEAT_AT || !fast_enough(nations, town, best) {
            continue;
        }
        let wanted = (GENESIS_COINS as f64 * share).max(20.0);
        let (ours, all) = pending_country_power(nations, at, nations.towns[town].country);
        let room = (ONE_COUNTRY_AT_MOST * all as f64 - ours as f64) / (1.0 - ONE_COUNTRY_AT_MOST);
        if room < 20.0 {
            continue;
        }
        // A seat nobody would sell it the coin for is a seat it does not take this year.
        if bond(nations, at, town, wanted.min(room) as u128 * COIN, false) {
            record_seat(nations, at, town);
        }
    }
}

/// Write down that a house took a seat — under a key kept for staking, if its own was jailed.
fn record_seat(nations: &mut Nations, at: usize, town: usize) {
    let network = &nations.networks[at];
    let new_key = network.staking_address(town) != network.address_of(town);
    nations.history.push(Event::Joined {
        year: nations.year,
        network: at,
        town,
        new_key,
    });
}

/// The fastest any of a chain's validators checks.
fn best_technique(nations: &Nations, at: usize) -> f64 {
    validator_techniques(nations, at).into_iter().fold(0.0f64, f64::max)
}

/// A seat is only worth taking by a house that checks as fast as the others: a slow one would
/// make every payment on the chain dearer.
fn fast_enough(nations: &Nations, town: usize, best: f64) -> bool {
    nations.towns[town].technique[Sector::Reckoning as usize] >= payments::CAPABLE * best
}

/// Whether the key a house stakes with has been jailed, as things will stand once everything
/// sent so far has gone through.
fn jailed(network: &Network, town: usize) -> bool {
    network
        .staking_address(town)
        .and_then(|a| network.chain.pending_account(&a))
        .is_some_and(|a| a.jailed)
}

/// A house buys stake from those who have it and bonds it; whether it bonded anything. With
/// `partly`, it bonds as much as it could buy; without, all of `stake` or nothing. A house whose
/// staking key the chain has jailed takes a new one first, and hands it the coin to bond.
fn bond(nations: &mut Nations, at: usize, town: usize, stake: u128, partly: bool) -> bool {
    if jailed(&nations.networks[at], town) {
        let generation = nations.networks[at].stakers.get(&town).map_or(0, Vec::len) as u64 + 1;
        let key = key_of_generation(nations, at, town, generation);
        nations.networks[at].stakers.entry(town).or_default().push(key);
    }
    fund(nations, at, town, stake + COIN);
    let network = &mut nations.networks[at];
    let min_fee = network.chain.params().min_fee;
    let (Some(own), Some(key)) = (network.address_of(town), network.staking_key(town).cloned()) else {
        return false;
    };
    let staking = Address::of(&key.public());
    // Handing the stake to a staking key is one more transaction, and one more fee.
    let fees = if staking == own { min_fee } else { 2 * min_fee };
    let held = network.chain.pending_balance(&own, Asset::Coin);
    let stake = if partly {
        stake.min(held.saturating_sub(fees + COIN)) / COIN * COIN
    } else if held >= stake + fees {
        stake
    } else {
        0
    };
    if stake == 0 {
        return false;
    }
    if staking != own {
        let handed = Action::Pay {
            to: staking,
            asset: Asset::Coin,
            amount: stake + min_fee,
        };
        let tx = sign(network, town, min_fee, handed);
        if !submit(network, tx) {
            network.refused += 1;
            return false;
        }
    }
    let tx = sign_with(network, &key, min_fee, Action::Bond { amount: stake });
    if submit(network, tx) {
        true
    } else {
        network.refused += 1;
        false
    }
}

/// The validators whose houses signed a height twice: every vote in the block's commit has a
/// `TWO_CLERKS` chance of a second from the same key, for no block. Each such house, with the
/// vote the chain kept and the one it never used.
fn signed_twice(nations: &Nations, at: usize, height: u64, month: u64) -> Vec<(usize, Vote, Vote)> {
    let network = &nations.networks[at];
    let Some(block) = network.chain.blocks.get(height as usize) else {
        return Vec::new();
    };
    let mut rng = nations.seed.stream(
        Domain::Commerce,
        CLERKS + at as u64,
        nations.year * BLOCKS_A_YEAR + month,
    );
    let mut caught = Vec::new();
    for vote in &block.commit.votes {
        if !rng.chance(TWO_CLERKS) {
            continue;
        }
        let Some(town) = network.town_of(&Address::of(&vote.validator)) else {
            continue;
        };
        let Some(key) = network.keys_of(town).find(|k| k.public() == vote.validator) else {
            continue;
        };
        let stray = Vote::signed(key, network.chain.id, vote.height, vote.round, Digest::default());
        caught.push((town, vote.clone(), stray));
    }
    caught
}

/// Every validator hears every vote, so two from one key at one height are seen by all of them.
/// The one that proposes next shows both to the chain — or, if that is the offender, the one
/// with most stake after it — and the chain burns a twentieth of the offender's stake and jails
/// it for good (`chain::state`). What it burns is read off the account as the evidence will find
/// it, and written into the world's history.
fn accuse(nations: &mut Nations, at: usize, town: usize, first: Vote, second: Vote) {
    let network = &nations.networks[at];
    let offender = Address::of(&first.validator);
    let params = network.chain.params();
    let (min_fee, permille) = (params.min_fee, params.slash_permille as u128);
    let Some(account) = network.chain.pending_account(&offender) else {
        return;
    };
    if account.jailed {
        return;
    }
    let burned = account.bonded * permille / 1000
        + account
            .unbonding
            .iter()
            .map(|(_, amount)| amount * permille / 1000)
            .sum::<u128>();
    let (rotation, next) = network.chain.rotation.at_round(0);
    let mut others: Vec<&chain::consensus::Validator> = rotation.members.iter().collect();
    others.sort_by(|a, b| b.power.cmp(&a.power).then(a.address.cmp(&b.address)));
    let Some(by) = std::iter::once(&rotation.members[next])
        .chain(others)
        .filter(|v| v.address != offender)
        .filter(|v| network.chain.pending_balance(&v.address, Asset::Coin) >= min_fee)
        .find_map(|v| network.town_of(&v.address))
    else {
        return;
    };
    let height = first.height;
    let network = &mut nations.networks[at];
    let evidence = Action::Evidence {
        first: Box::new(first),
        second: Box::new(second),
    };
    let tx = sign(network, by, min_fee, evidence);
    if submit(network, tx) {
        nations.history.push(Event::Slashed {
            year: nations.year,
            network: at,
            town,
            by,
            height,
            burned,
        });
    } else {
        network.refused += 1;
    }
}

/// The votes the validators due to sign the next height would give a block — all of them, or
/// only those of one country — for tests that show what a country cannot do by itself.
#[cfg(test)]
pub(crate) fn votes_for(nations: &Nations, at: usize, block: Digest, of_country: Option<usize>) -> Vec<Vote> {
    let network = &nations.networks[at];
    let height = network.chain.height() + 1;
    network
        .chain
        .rotation
        .members
        .iter()
        .filter_map(|v| {
            let town = network.town_of(&v.address)?;
            if of_country.is_some_and(|c| nations.towns[town].country != c) {
                return None;
            }
            let key = network.keys_of(town).find(|k| k.public() == v.key)?;
            Some(Vote::signed(key, network.chain.id, height, 0, block))
        })
        .collect()
}

/// Everybody due to sign the next height: the town whose house each is, the key it signs with
/// and its power — for tests that have them sign what they should not.
#[cfg(test)]
pub(crate) fn signers(nations: &Nations, at: usize) -> Vec<(usize, SigningKey, u64)> {
    let network = &nations.networks[at];
    network
        .chain
        .rotation
        .members
        .iter()
        .filter_map(|v| {
            let town = network.town_of(&v.address)?;
            let key = network.keys_of(town).find(|k| k.public() == v.key)?;
            Some((town, key.clone(), v.power))
        })
        .collect()
}

/// Every house's own key, by town — for tests that need somebody with coin to send something.
#[cfg(test)]
pub(crate) fn house_keys(nations: &Nations, at: usize) -> Vec<(usize, SigningKey)> {
    let network = &nations.networks[at];
    network.keys.iter().map(|(town, key)| (*town, key.clone())).collect()
}

/// Make a house sign the latest block it signed a second time, for no block, as `signed_twice`
/// does by chance — for tests, which cannot wait on chance.
#[cfg(test)]
pub(crate) fn sign_twice(nations: &mut Nations, at: usize, town: usize) {
    let network = &nations.networks[at];
    let key = network.staking_key(town).expect("a house has a key").clone();
    let vote = network
        .chain
        .blocks
        .iter()
        .rev()
        .find_map(|b| b.commit.votes.iter().find(|v| v.validator == key.public()).cloned())
        .expect("it has signed a block");
    let stray = Vote::signed(&key, network.chain.id, vote.height, vote.round, Digest::default());
    accuse(nations, at, town, vote, stray);
}

/// A key a house can no longer validate with gives up what it holds. A jailed key takes back
/// what is left of its stake, through the same twelve-block wait as any house leaving; and a key
/// kept for staking hands its house whatever coin it has beyond the fees it may still need —
/// its rewards, and its stake as that is released.
fn tend_stakes(nations: &mut Nations, at: usize) {
    let network = &mut nations.networks[at];
    let min_fee = network.chain.params().min_fee;
    let towns: Vec<usize> = network.houses().collect();
    for town in towns {
        let Some(own) = network.address_of(town) else {
            continue;
        };
        let current = network.staking_address(town);
        let keys: Vec<SigningKey> = network.keys_of(town).cloned().collect();
        for key in keys {
            let address = Address::of(&key.public());
            let Some(account) = network.chain.pending_account(&address).cloned() else {
                continue;
            };
            if account.jailed && account.bonded > 0 && account.coin >= min_fee {
                let unbond = Action::Unbond {
                    amount: account.bonded,
                };
                let tx = sign_with(network, &key, min_fee, unbond);
                if !submit(network, tx) {
                    network.refused += 1;
                }
            }
            if address == own {
                continue;
            }
            let keep = if Some(address) == current && !account.jailed {
                COIN + min_fee
            } else {
                min_fee
            };
            let spare = network
                .chain
                .pending_balance(&address, Asset::Coin)
                .saturating_sub(keep + min_fee);
            if spare > 0 {
                let home = Action::Pay {
                    to: own,
                    asset: Asset::Coin,
                    amount: spare,
                };
                let tx = sign_with(network, &key, min_fee, home);
                if !submit(network, tx) {
                    network.refused += 1;
                }
            }
        }
    }
}

/// A chain that has lost a validator to jail may be left with one country's houses holding more
/// than `ONE_COUNTRY_AT_MOST` of the stake, or with fewer than four validators. Then the houses
/// best placed to mend it bond what it takes, whatever their standing, until neither is so:
/// while a country is over its share, the best-placed house of another country — which may
/// validate already — bonds what brings it back; while there are fewer than four, the
/// best-placed house not validating whose country has room under its share takes a seat. What
/// nobody can buy the coin for, the country over its share gives up: its largest validator
/// unbonds the rest. With nothing to mend, as in almost every month, it does nothing.
fn mend(nations: &mut Nations, at: usize) {
    let most = nations.networks[at].chain.params().max_validators;
    let min_bond = nations.networks[at].chain.params().min_bond.max(COIN) / COIN;
    for _ in 0..most {
        if let Some((country, ours, all)) = over_its_share(nations, at) {
            // What the others must add for this country to hold its three fifths again.
            let needed = (ours as f64 / ONE_COUNTRY_AT_MOST - all as f64).ceil() as u128 + 1;
            let Some(town) = best_placed(nations, at, |t| nations.towns[t].country != country, true) else {
                break;
            };
            let seated = pending_towns(&nations.networks[at]).contains(&town);
            if !bond(nations, at, town, needed.max(min_bond) * COIN, true) {
                break;
            }
            // Bonding what it could buy may still leave it short of a seat.
            if !seated && pending_towns(&nations.networks[at]).contains(&town) {
                record_seat(nations, at, town);
            }
        } else if nations.networks[at].chain.pending_validators().len() < payments::FEWEST_FOUNDERS.min(most) {
            let room = |t: usize| {
                let (ours, all) = pending_country_power(nations, at, nations.towns[t].country);
                (ONE_COUNTRY_AT_MOST * all as f64 - ours as f64) / (1.0 - ONE_COUNTRY_AT_MOST)
            };
            let Some(town) = best_placed(nations, at, |t| room(t) >= 20.0, false) else {
                break;
            };
            let share = nations.networks[at].standing.get(&town).copied().unwrap_or(0.0);
            let wanted = (GENESIS_COINS as f64 * share).max(20.0).min(room(town)) as u128;
            if !bond(nations, at, town, wanted.max(min_bond) * COIN, false) {
                break;
            }
            record_seat(nations, at, town);
        } else {
            return;
        }
    }
    // With nobody else holding stake, giving some up would change nothing.
    let Some((country, ours, all)) = over_its_share(nations, at).filter(|(_, ours, all)| ours < all) else {
        return;
    };
    let excess = ((ours as f64 - ONE_COUNTRY_AT_MOST * all as f64) / (1.0 - ONE_COUNTRY_AT_MOST)).ceil() as u128;
    let network = &nations.networks[at];
    let largest = network
        .chain
        .pending_validators()
        .into_iter()
        .filter(|(address, _, _)| {
            network
                .town_of(address)
                .is_some_and(|t| nations.towns[t].country == country)
        })
        .max_by(|a, b| a.2.cmp(&b.2).then(b.0.cmp(&a.0)));
    let Some((address, _, power)) = largest else {
        return;
    };
    // Signed by the key the stake is bonded under, which is not the house's own if that was
    // jailed.
    let Some(town) = network.town_of(&address) else {
        return;
    };
    let Some(key) = network.keys_of(town).find(|k| Address::of(&k.public()) == address).cloned() else {
        return;
    };
    let amount = excess.min((power as u128).saturating_sub(min_bond)) * COIN;
    let network = &mut nations.networks[at];
    let min_fee = network.chain.params().min_fee;
    if amount == 0 || network.chain.pending_balance(&address, Asset::Coin) < min_fee {
        return;
    }
    let tx = sign_with(network, &key, min_fee, Action::Unbond { amount });
    if !submit(network, tx) {
        network.refused += 1;
    }
}

/// The towns whose houses will validate once everything sent so far has gone through.
fn pending_towns(network: &Network) -> Vec<usize> {
    network
        .chain
        .pending_validators()
        .iter()
        .filter_map(|(address, _, _)| network.town_of(address))
        .collect()
}

/// The country whose houses hold more than `ONE_COUNTRY_AT_MOST` of the stake, once everything
/// sent so far has gone through, with its power and everybody's. Power is whole coins, so a
/// founding that gave a country exactly its three fifths can leave it a thousandth over, and that
/// is not what this is for.
fn over_its_share(nations: &Nations, at: usize) -> Option<(usize, u64, u64)> {
    (0..nations.countries.len()).find_map(|c| {
        let (ours, all) = pending_country_power(nations, at, c);
        (ours as f64 > (ONE_COUNTRY_AT_MOST + 1e-3) * all as f64).then_some((c, ours, all))
    })
}

/// The house with the best standing that is `wanted` — one that will validate already only if
/// `validating` allows it — preferring one that checks fast enough. A house whose key was jailed
/// counts like any other: it would bond with a new one (`bond`). Mending a
/// chain is not taking a seat by choice: when the only houses that can keep a country from
/// holding a chain alone check slower than the rest, a slower chain is the price of one nobody
/// keeps, as it was at the founding, which waited for the slowest country.
fn best_placed(
    nations: &Nations,
    at: usize,
    wanted: impl Fn(usize) -> bool,
    validating: bool,
) -> Option<usize> {
    let network = &nations.networks[at];
    let seated = pending_towns(network);
    let best = best_technique(nations, at);
    network
        .houses()
        .filter(|t| validating || !seated.contains(t))
        .filter(|t| wanted(*t))
        .max_by(|a, b| {
            let key = |t: &usize| {
                (
                    fast_enough(nations, *t, best),
                    network.standing.get(t).copied().unwrap_or(0.0),
                )
            };
            let (ka, kb) = (key(a), key(b));
            ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1)).then(b.cmp(a))
        })
}

/// A validator whose standing has fallen below `LEAVE_BELOW` takes its stake back — unless the
/// chain would be left with fewer than four validators, or with one country's houses holding
/// more of the stake than `ONE_COUNTRY_AT_MOST`.
fn leave_idle(nations: &mut Nations, at: usize) {
    let network = &nations.networks[at];
    let idle: Vec<usize> = network
        .validators()
        .into_iter()
        .filter(|town| network.standing.get(town).copied().unwrap_or(0.0) < LEAVE_BELOW)
        .collect();
    for town in idle {
        let network = &nations.networks[at];
        let Some(address) = network.staking_address(town) else {
            continue;
        };
        let pending = network.chain.pending_validators();
        let Some(&(_, _, leaving)) = pending.iter().find(|(a, _, _)| *a == address) else {
            continue;
        };
        if pending.len() <= payments::FEWEST_FOUNDERS {
            continue;
        }
        let left = pending.iter().map(|(_, _, power)| power).sum::<u64>() - leaving;
        let too_much = (0..nations.countries.len()).any(|c| {
            let (ours, _) = pending_country_power(nations, at, c);
            let ours = if nations.towns[town].country == c { ours - leaving } else { ours };
            ours as f64 > ONE_COUNTRY_AT_MOST * left as f64
        });
        if too_much {
            continue;
        }
        let bonded = network.chain.ledger.account(&address).map(|a| a.bonded).unwrap_or(0);
        let network = &mut nations.networks[at];
        let min_fee = network.chain.params().min_fee;
        if bonded == 0 || network.chain.pending_balance(&address, Asset::Coin) < min_fee {
            continue;
        }
        let key = network.staking_key(town).cloned();
        let tx = key.and_then(|k| sign_with(network, &k, min_fee, Action::Unbond { amount: bonded }));
        if submit(network, tx) {
            nations.history.push(Event::Left {
                year: nations.year,
                network: at,
                town,
            });
        } else {
            network.refused += 1;
        }
    }
}

/// What each state of a country makes, as a share of what the country makes, by the state's
/// hub.
fn state_shares(nations: &Nations, country: usize) -> Vec<(usize, f64)> {
    let made: Vec<(usize, f64)> = nations.countries[country]
        .states
        .iter()
        .map(|s| {
            let state = &nations.states[*s];
            let product: f64 = state.towns.iter().map(|t| nations.towns[*t].product()).sum();
            (state.hub, product)
        })
        .collect();
    let total: f64 = made.iter().map(|(_, p)| p).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    made.into_iter().map(|(hub, p)| (hub, p / total)).collect()
}

/// The houses that carry a country's payments abroad on a chain, with the share each carries:
/// every state's trade through its own market house if that house keeps an account here, and
/// through the capital's if not.
fn carriers(nations: &Nations, at: usize, country: usize) -> Vec<(usize, f64)> {
    let capital = nations.countries[country].capital;
    let network = &nations.networks[at];
    let mut carried: BTreeMap<usize, f64> = BTreeMap::new();
    for (hub, share) in state_shares(nations, country) {
        let house = if share >= OWN_HOUSE && network.keys.contains_key(&hub) {
            hub
        } else {
            capital
        };
        *carried.entry(house).or_insert(0.0) += share;
    }
    if carried.is_empty() {
        carried.insert(capital, 1.0);
    }
    carried.into_iter().collect()
}

/// How one house's payments to another fall across this year's months: twelve factors that
/// average one, drawn afresh each year for each pair of houses and each way round — so what a
/// house pays another in a month is not what it is paid back, though over the year it is.
fn lumps(nations: &Nations, at: usize, from: usize, to: usize) -> [f64; BLOCKS_A_YEAR as usize] {
    let entity = LUMPS | (at as u64) << 40 | (from as u64) << 20 | to as u64;
    let mut rng = nations.seed.stream(Domain::Commerce, entity, nations.year);
    let mut factors = [0.0; BLOCKS_A_YEAR as usize];
    for factor in factors.iter_mut() {
        *factor = (LUMPY * rng.normal()).exp();
    }
    let mean = factors.iter().sum::<f64>() / factors.len() as f64;
    for factor in factors.iter_mut() {
        *factor /= mean;
    }
    factors
}

/// The voting power of one country's validators on a chain, and of all of them.
fn country_power(nations: &Nations, at: usize, country: usize) -> (u64, u64) {
    let network = &nations.networks[at];
    let members = &network.chain.rotation.members;
    let ours = members
        .iter()
        .filter(|v| {
            network
                .town_of(&v.address)
                .is_some_and(|t| nations.towns[t].country == country)
        })
        .map(|v| v.power)
        .sum();
    (ours, members.iter().map(|v| v.power).sum())
}

/// The same once everything already sent this year has gone through — what a house deciding to
/// join or leave has to reckon with, or two houses of one country each finding room under the cap
/// in the same year take it twice.
fn pending_country_power(nations: &Nations, at: usize, country: usize) -> (u64, u64) {
    let network = &nations.networks[at];
    let pending = network.chain.pending_validators();
    let ours = pending
        .iter()
        .filter(|(address, _, _)| {
            network
                .town_of(address)
                .is_some_and(|t| nations.towns[t].country == country)
        })
        .map(|(_, _, power)| power)
        .sum();
    (ours, pending.iter().map(|(_, _, power)| power).sum())
}

/// The largest share of a chain's voting power the validators of any one country hold.
pub fn largest_country_share(nations: &Nations, at: usize) -> Option<(usize, f64)> {
    (0..nations.countries.len())
        .map(|c| {
            let (ours, all) = country_power(nations, at, c);
            (c, ours as f64 / all.max(1) as f64)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
}

/// One month of settlement: the attestor states the reserve, the issuer mints what houses have
/// paid it for, houses pay each other, and anything held beyond need is redeemed. Returns what
/// was settled, in years of food, and the fees paid, in years of food.
fn settle_month(
    nations: &mut Nations,
    at: usize,
    pairs: &[((usize, usize), f64)],
    month: u64,
    size: f64,
    wage: f64,
    reserve_level: f64,
) -> (f64, f64) {
    let Some(token) = nations.networks[at].token.clone() else {
        return (0.0, 0.0);
    };
    let country_of = |key: usize| nations.countries.iter().position(|c| c.key == key);
    // What each house pays each other house this month, in base units of the token.
    let to_units = |food: f64| (food * reserve_level * TOKEN_UNIT as f64).max(0.0) as u128;
    let mut owed: Vec<(usize, usize, u128, f64)> = Vec::new();
    for (pair, volume) in pairs {
        let share = nations.networks[at].share_of(*pair);
        if share <= 0.0 {
            continue;
        }
        let (Some(a), Some(b)) = (country_of(pair.0), country_of(pair.1)) else {
            continue;
        };
        // Half each way over the year: what one pays for the other's wares, the other pays for
        // its food. Split between the houses on each side by the trade each carries.
        let each_way = share * volume / 2.0;
        for (payer, payee) in [(a, b), (b, a)] {
            let paying = carriers(nations, at, payer);
            let paid = carriers(nations, at, payee);
            let largest = paid
                .iter()
                .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
                .map(|(t, _)| *t)
                .expect("a country always has a house to be paid through");
            let mut between: BTreeMap<(usize, usize), f64> = BTreeMap::new();
            for (from, mine) in &paying {
                for (to, theirs) in &paid {
                    let to = if mine * theirs >= DIRECT { *to } else { largest };
                    *between.entry((*from, to)).or_insert(0.0) += mine * theirs;
                }
            }
            for ((from, to), part) in between {
                let lump = lumps(nations, at, from, to)[month as usize];
                let food = each_way * part * lump / BLOCKS_A_YEAR as f64;
                let units = to_units(food);
                if units > 0 {
                    owed.push((from, to, units, food));
                }
            }
        }
    }
    if owed.is_empty() {
        return (0.0, 0.0);
    }

    // What checking one payment costs every validator between them, paid in coin.
    let per_payment: f64 = validator_techniques(nations, at)
        .iter()
        .map(|t| payments::check_cost(*t))
        .sum::<f64>()
        * wage;
    let coin_price = nations.networks[at].coin_price.max(1e-12);
    let min_fee = nations.networks[at].chain.params().min_fee;
    let fee_for = |food: f64| -> u128 {
        let payments_in_it = (food / size.max(1e-9)).max(1.0);
        let in_food = payments_in_it * per_payment;
        let in_coin = in_food * reserve_level / coin_price;
        ((in_coin * COIN as f64) as u128).max(min_fee)
    };

    // Every house needs coin for its fees this month: for what it pays, and a fee's worth to
    // hand back what it is paid.
    let mut coin_needed: BTreeMap<usize, u128> = BTreeMap::new();
    for (from, to, _, food) in &owed {
        *coin_needed.entry(*from).or_insert(0) += fee_for(*food) + 4 * min_fee;
        *coin_needed.entry(*to).or_insert(0) += 2 * min_fee;
    }
    // And the issuer a fee for every house it might mint for, and the attestor one for its word.
    let payers = owed.iter().map(|(from, ..)| *from).collect::<std::collections::BTreeSet<_>>();
    *coin_needed.entry(token.issuer).or_insert(0) += (payers.len() as u128 + 2) * min_fee;
    *coin_needed.entry(token.attestor).or_insert(0) += 2 * min_fee;
    let keeping: BTreeMap<usize, u128> = coin_needed
        .iter()
        .map(|(t, need)| (*t, need + COIN / 100))
        .collect();
    let price = Price::now(&nations.networks[at]);
    // The two houses that must sign before there are any new tokens this month — the attestor
    // and the issuer — pay those fees with coin bought the month before, so they buy two months'
    // worth when the others buy one. Only when that is short, as in a chain's first month, do they
    // buy before the mints, with whatever tokens they hold and over the counter for the rest.
    let first = [token.attestor, token.issuer];
    let target = |town: usize| {
        let need = keeping.get(&town).copied().unwrap_or(0);
        if first.contains(&town) { 2 * need } else { need }
    };
    let mut incoming: BTreeMap<usize, u128> = BTreeMap::new();
    for town in first {
        let need = keeping.get(&town).copied().unwrap_or(0);
        let have = nations.networks[at]
            .address_of(town)
            .map(|a| nations.networks[at].chain.pending_balance(&a, Asset::Coin))
            .unwrap_or(0);
        if have < need {
            for (seller, tokens) in fund_keeping(nations, at, town, need, &keeping, price, 0) {
                *incoming.entry(seller).or_insert(0) += tokens;
            }
        }
    }

    // Tokens: what each house must hold to pay what it owes before it is paid, and to buy the
    // coin its fees will take.
    let mut outgoing: BTreeMap<usize, u128> = BTreeMap::new();
    for (from, to, units, _) in &owed {
        *outgoing.entry(*from).or_insert(0) += units;
        *incoming.entry(*to).or_insert(0) += units;
    }
    let owing = outgoing.clone();
    if let Some(price) = price {
        let network = &nations.networks[at];
        for town in keeping.keys() {
            let Some(address) = network.address_of(*town) else {
                continue;
            };
            // The attestor and issuer pay their own fees between now and buying, so their tokens
            // are planned with a month's fees to spare.
            let spare = if first.contains(town) { keeping[town] } else { 0 };
            let short = (target(*town) + spare)
                .saturating_sub(network.chain.pending_balance(&address, Asset::Coin));
            if short > 0 {
                *outgoing.entry(*town).or_insert(0) += price.of(short) + TOKEN_UNIT;
            }
        }
    }
    let network = &mut nations.networks[at];
    let mut mints: Vec<(usize, u128)> = Vec::new();
    for (town, out) in &outgoing {
        let Some(address) = network.address_of(*town) else {
            continue;
        };
        let held = network.chain.pending_balance(&address, Asset::Token(token.id));
        if *out > held {
            mints.push((*town, out - held));
        }
    }
    let minted: u128 = mints.iter().map(|(_, m)| m).sum();
    let supply = network.chain.pending_token(token.id).map(|t| t.supply).unwrap_or(0);
    // The attestor states what the issuer now holds: everything outstanding, plus what houses
    // have just paid it for the tokens it is about to mint. It is the issuer's real reserve —
    // the model's issuer keeps every unit it is paid.
    let reserves = supply + minted;
    let attest = sign(
        network,
        token.attestor,
        min_fee,
        Action::Attest {
            token: token.id,
            reserves,
        },
    );
    if !submit(network, attest) {
        network.refused += 1;
    }
    for (town, amount) in &mints {
        let Some(to) = network.address_of(*town) else {
            continue;
        };
        let tx = sign(
            network,
            token.issuer,
            min_fee,
            Action::Mint {
                token: token.id,
                to,
                amount: *amount,
            },
        );
        if !submit(network, tx) {
            network.refused += 1;
        }
    }
    // Houses buy the coin their fees will take from the houses with most to spare.
    for town in keeping.keys() {
        let owes = owing.get(town).copied().unwrap_or(0);
        for (seller, tokens) in fund_keeping(nations, at, *town, target(*town), &keeping, price, owes) {
            *incoming.entry(seller).or_insert(0) += tokens;
        }
    }
    let network = &mut nations.networks[at];
    let mut settled = 0.0;
    let mut fees = 0.0;
    for (from, to, units, food) in &owed {
        let Some(to_address) = network.address_of(*to) else {
            continue;
        };
        let fee = fee_for(*food);
        let tx = sign(
            network,
            *from,
            fee,
            Action::Pay {
                to: to_address,
                asset: Asset::Token(token.id),
                amount: *units,
            },
        );
        if submit(network, tx) {
            settled += 2.0 * food;
            fees += fee as f64 / COIN as f64 * coin_price / reserve_level;
            *network.working.entry(*from).or_insert(0.0) += food;
            *network.working.entry(*to).or_insert(0.0) += food;
        } else {
            network.refused += 1;
        }
    }
    // Whatever a house holds beyond a float of its business — the larger of what it paid and
    // what it was paid this month — it hands back for currency.
    let mut redeemed = false;
    let houses: Vec<usize> = network.houses().collect();
    for town in &houses {
        let Some(address) = network.address_of(*town) else {
            continue;
        };
        let held = network.chain.pending_balance(&address, Asset::Token(token.id));
        // Nobody sends what it cannot pay the fee for; a house with no coin to hand redeems
        // another month.
        if held == 0 || network.chain.pending_balance(&address, Asset::Coin) < min_fee {
            continue;
        }
        let business = incoming
            .get(town)
            .copied()
            .unwrap_or(0)
            .max(outgoing.get(town).copied().unwrap_or(0));
        let keep = (FLOAT * business as f64 * BLOCKS_A_YEAR as f64) as u128;
        if held > keep {
            let tx = sign(
                network,
                *town,
                min_fee,
                Action::Redeem {
                    token: token.id,
                    amount: held - keep,
                },
            );
            if submit(network, tx) {
                redeemed = true;
            } else {
                network.refused += 1;
            }
        }
    }
    // The issuer pays out what was redeemed, so what it holds is what is still outstanding —
    // and when anything was, the attestor says so again, so that the reserve the chain shows at
    // the end of a month is the one there is rather than the one there was at its start.
    let outstanding = network
        .chain
        .pending_token(token.id)
        .map(|t| t.supply)
        .unwrap_or(0);
    if redeemed {
        let tx = sign(
            network,
            token.attestor,
            min_fee,
            Action::Attest {
                token: token.id,
                reserves: outstanding,
            },
        );
        if !submit(network, tx) {
            network.refused += 1;
        }
    }
    if let Some(t) = network.token.as_mut() {
        t.reserves = outstanding as f64 / TOKEN_UNIT as f64;
    }
    (settled / 2.0, fees)
}
