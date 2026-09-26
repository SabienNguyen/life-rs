//! A ledger nobody keeps, as a world of nations comes to use one.
//!
//! `chain` is the ledger; this is the world deciding to keep one, and then keeping it. Every
//! year, until one exists, the houses of the world's market towns are asked the three questions
//! `commerce::payments` states — is there a keeper they all trust, are there enough of them,
//! and would checking it cost less than their distrust does — and the first year the answers
//! are no, yes and yes, the largest of them found a chain together: a genesis they all sign,
//! with stake in proportion to their business abroad.
//!
//! After that, nothing about it is decided here that the chain does not also check. Once the
//! world has a currency everybody invoices in, the house that issues it registers a stable
//! token on the chain, with a house in another country vouching for its reserve. Twelve times a
//! year a block is cut: the attestor states the reserve, the issuer mints what houses have paid
//! it for, houses pay each other what their merchants owe abroad, and whatever a house holds
//! beyond what it needs on hand it redeems. Every one of those is a signed transaction the chain
//! refuses if it does not add up, in a block the validators commit only if they can re-derive
//! it. A validator whose town is starving is not at its post; one that simply has a bad day
//! misses a round; and a chain that loses a third of its stake stops until they come back.
//!
//! What the chain changes in the world is the one number it was founded for: what it costs a
//! country to be paid abroad. That is the wedge on every border in the market tree, so a chain
//! that works widens every market it touches.

use std::collections::BTreeMap;

use chain::{Action, Address, Asset, COIN, Chain, Genesis, SigningKey, TOKEN_UNIT, Transaction};
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

/// The share of a year's settlements a house keeps in tokens rather than redeeming.
const FLOAT: f64 = 0.1;

/// A country whose share of what a chain carries reaches this takes a seat among its
/// validators, rather than leave its payments to be checked by others.
const SEAT_AT: f64 = 0.05;

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

    /// The towns whose houses validate now.
    pub fn validators(&self) -> Vec<usize> {
        self.keys
            .iter()
            .filter(|(_, k)| self.chain.rotation.find(&k.public()).is_some())
            .map(|(t, _)| *t)
            .collect()
    }

    /// The town whose house holds an address, if any does.
    pub fn town_of(&self, address: &Address) -> Option<usize> {
        self.keys
            .iter()
            .find(|(_, k)| Address::of(&k.public()) == *address)
            .map(|(t, _)| *t)
    }
}

/// The key a town's house signs with on a chain: derived from the world's seed, so a world
/// founded twice signs the same history twice.
fn key_for(nations: &Nations, network: usize, town: usize) -> SigningKey {
    let mut rng = nations
        .seed
        .stream(Domain::Commerce, 0x04e1_0000 + network as u64, town as u64);
    let mut seed = [0u8; 32];
    for chunk in seed.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    SigningKey::from_seed(seed)
}

/// A year of every ledger: founding one if it has become worth founding, then twelve blocks of
/// each.
pub(crate) fn year(nations: &mut Nations) {
    if nations.networks.is_empty() {
        consider_founding(nations);
    }
    for at in 0..nations.networks.len() {
        keep(nations, at);
    }
}

/// Who could found a chain: every state's hub, with the business abroad of its state.
fn candidates(nations: &Nations) -> (Vec<usize>, Vec<Candidate>) {
    let mut towns = Vec::new();
    let mut found = Vec::new();
    for state in &nations.states {
        let country = &nations.countries[state.country];
        if country.product <= 0.0 {
            continue;
        }
        let product: f64 = state.towns.iter().map(|t| nations.towns[*t].product()).sum();
        let volume = country.exports * product / country.product;
        let hub = &nations.towns[state.hub];
        towns.push(state.hub);
        found.push(Candidate {
            country: state.country,
            reckoners: hub.workers()[Sector::Reckoning as usize],
            technique: hub.technique[Sector::Reckoning as usize],
            volume,
        });
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
    let allocations: Vec<(chain::PublicKey, u128, u128)> = founding
        .founders
        .iter()
        .zip(&keys)
        .map(|(i, key)| {
            let share = candidates[*i].volume / founding_volume.max(1e-9);
            let stake = FOUNDING_STAKE * COIN + (spare as f64 * share * COIN as f64) as u128;
            // A tenth again, liquid, to pay fees with until the chain pays them back.
            (key.public(), stake / 10, stake)
        })
        .collect();
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
    let coin_price = payments::coin_value(volume_in_currency, 0.0, GENESIS_COINS as f64);
    nations.networks.push(Network {
        name,
        founded: nations.year,
        chain,
        keys: founders.iter().copied().zip(keys).collect(),
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
    let wage = wage(nations);
    let size = payment_size(nations);
    let reserve_level = nations.currencies[reserve].level.max(1e-9);

    // Every country that trades abroad keeps an account through its capital's house.
    for c in 0..nations.countries.len() {
        let capital = nations.countries[c].capital;
        if !nations.networks[at].keys.contains_key(&capital) && nations.countries[c].exports > 0.0 {
            let key = key_for(nations, at, capital);
            nations.networks[at].keys.insert(capital, key);
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
    take_seats(nations, at);

    // Twelve months.
    let mut carried = 0.0;
    let mut fees = 0.0;
    nations.networks[at].blocks_this_year = 0;
    for month in 0..BLOCKS_A_YEAR {
        let (value, paid) = settle_month(nations, at, &pairs, size, wage, reserve_level);
        carried += value;
        fees += paid;
        let time = (nations.year * BLOCKS_A_YEAR + month + 1) * MONTH;
        let answering = who_answers(nations, at, month);
        let keys: BTreeMap<Address, SigningKey> = nations.networks[at]
            .keys
            .values()
            .map(|k| (Address::of(&k.public()), k.clone()))
            .collect();
        let network = &mut nations.networks[at];
        let outcome = network.chain.step(
            time,
            &keys,
            &|address, round| answering.get(&(*address, round)).copied().unwrap_or(false),
            MOST_ROUNDS,
        );
        match outcome {
            Some(_) => network.blocks_this_year += 1,
            None => {
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
    for (town, key) in &network.keys {
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
    let key = network.keys.get(&town)?;
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

/// A house short of coin buys it from the houses that have most to spare, largest first, until
/// it has enough or nobody has any left to sell. Nobody sells what they need themselves: what
/// every house needs this month is `keeping`.
fn fund(nations: &mut Nations, at: usize, town: usize, wanted: u128) {
    fund_keeping(nations, at, town, wanted, &BTreeMap::new());
}

fn fund_keeping(
    nations: &mut Nations,
    at: usize,
    town: usize,
    wanted: u128,
    keeping: &BTreeMap<usize, u128>,
) {
    let network = &mut nations.networks[at];
    let Some(address) = network.address_of(town) else {
        return;
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
            return;
        }
        // A seller keeps a coin and a fee for itself, and whatever it needs this month.
        let own = keeping.get(&seller).copied().unwrap_or(0);
        let spare = balance.saturating_sub(min_fee + COIN + own);
        let amount = (wanted - have).min(spare);
        if amount == 0 {
            continue;
        }
        let tx = sign(
            network,
            seller,
            min_fee,
            Action::Pay {
                to: address,
                asset: Asset::Coin,
                amount,
            },
        );
        if !submit(network, tx) {
            network.refused += 1;
        }
    }
}

/// A country whose business on a chain has grown large enough takes a seat among its
/// validators: it buys stake from those who have it and bonds it.
fn take_seats(nations: &mut Nations, at: usize) {
    let validators = nations.networks[at].validators();
    let most = nations.networks[at].chain.params().max_validators;
    if validators.len() >= most {
        return;
    }
    let total: f64 = nations.payments.values().sum();
    if total <= 0.0 {
        return;
    }
    let best = validator_techniques(nations, at)
        .into_iter()
        .fold(0.0f64, f64::max);
    for c in 0..nations.countries.len() {
        let capital = nations.countries[c].capital;
        if validators.contains(&capital) || !nations.networks[at].keys.contains_key(&capital) {
            continue;
        }
        // A seat is only worth taking by a house that checks as fast as the others: a slow one
        // would make every payment on the chain dearer.
        if nations.towns[capital].technique[Sector::Reckoning as usize] < payments::CAPABLE * best {
            continue;
        }
        let key = nations.countries[c].key;
        let mine: f64 = nations
            .payments
            .iter()
            .filter(|((a, b), _)| *a == key || *b == key)
            .map(|((a, b), v)| v * nations.networks[at].share_of((*a, *b)))
            .sum();
        if mine / total < SEAT_AT {
            continue;
        }
        let stake = (GENESIS_COINS as f64 * mine / total).max(20.0) as u128 * COIN;
        fund(nations, at, capital, stake + COIN);
        let network = &mut nations.networks[at];
        let min_fee = network.chain.params().min_fee;
        // A seat nobody would sell it the coin for is a seat it does not take this year.
        let Some(address) = network.address_of(capital) else {
            continue;
        };
        if network.chain.pending_balance(&address, Asset::Coin) < stake + min_fee {
            continue;
        }
        let tx = sign(network, capital, min_fee, Action::Bond { amount: stake });
        if submit(network, tx) {
            nations.history.push(Event::Joined {
                year: nations.year,
                network: at,
                town: capital,
            });
        } else {
            network.refused += 1;
        }
    }
}

/// One month of settlement: the attestor states the reserve, the issuer mints what houses have
/// paid it for, houses pay each other, and anything held beyond need is redeemed. Returns what
/// was settled, in years of food, and the fees paid, in years of food.
fn settle_month(
    nations: &mut Nations,
    at: usize,
    pairs: &[((usize, usize), f64)],
    size: f64,
    wage: f64,
    reserve_level: f64,
) -> (f64, f64) {
    let Some(token) = nations.networks[at].token.clone() else {
        return (0.0, 0.0);
    };
    let capital_of = |key: usize| {
        nations
            .countries
            .iter()
            .find(|c| c.key == key)
            .map(|c| c.capital)
    };
    // What each house pays each other house this month, in base units of the token.
    let to_units = |food: f64| (food * reserve_level * TOKEN_UNIT as f64).max(0.0) as u128;
    let mut owed: Vec<(usize, usize, u128, f64)> = Vec::new();
    for (pair, volume) in pairs {
        let share = nations.networks[at].share_of(*pair);
        if share <= 0.0 {
            continue;
        }
        let (Some(a), Some(b)) = (capital_of(pair.0), capital_of(pair.1)) else {
            continue;
        };
        // Half each way: what one pays for the other's wares, the other pays for its food.
        let each_way = share * volume / BLOCKS_A_YEAR as f64 / 2.0;
        let units = to_units(each_way);
        if units == 0 {
            continue;
        }
        owed.push((a, b, units, each_way));
        owed.push((b, a, units, each_way));
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
    let keeping: BTreeMap<usize, u128> = coin_needed
        .iter()
        .map(|(t, need)| (*t, need + COIN / 100))
        .collect();
    for (town, need) in &keeping {
        fund_keeping(nations, at, *town, *need, &keeping);
    }

    // Tokens: what each house must hold to pay what it owes before it is paid.
    let mut outgoing: BTreeMap<usize, u128> = BTreeMap::new();
    let mut incoming: BTreeMap<usize, u128> = BTreeMap::new();
    for (from, to, units, _) in &owed {
        *outgoing.entry(*from).or_insert(0) += units;
        *incoming.entry(*to).or_insert(0) += units;
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
        } else {
            network.refused += 1;
        }
    }
    // Whatever a house holds beyond a float of its business, it hands back for currency.
    for (town, came_in) in &incoming {
        let Some(address) = network.address_of(*town) else {
            continue;
        };
        let held = network.chain.pending_balance(&address, Asset::Token(token.id));
        let keep = (FLOAT * *came_in as f64 * BLOCKS_A_YEAR as f64) as u128;
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
            if !submit(network, tx) {
                network.refused += 1;
            }
        }
    }
    // The issuer pays out what was redeemed, so what it holds is what is still outstanding.
    let outstanding = network
        .chain
        .pending_token(token.id)
        .map(|t| t.supply)
        .unwrap_or(0);
    if let Some(t) = network.token.as_mut() {
        t.reserves = outstanding as f64 / TOKEN_UNIT as f64;
    }
    (settled / 2.0, fees)
}
