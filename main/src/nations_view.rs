//! The world at the scale of nations, told: a history in lines, the countries and their money,
//! and the ledger nobody keeps — and the same as data for a page.

use chain::{Action, Address, Asset, COIN, TOKEN_UNIT};
use nations::{Event, Nations, Network};

/// What checking a chain from its genesis found, and how long it took — replaying every block,
/// following its headers as a light client does, and joining it late from somebody's books.
pub struct Checked {
    pub ok: Result<(), String>,
    pub seconds: f64,
    pub blocks: usize,
    pub light: Result<u64, String>,
    pub light_seconds: f64,
    /// `None` for a chain nobody has kept books of yet.
    pub joined: Option<Result<Joined, String>>,
    pub join_seconds: f64,
}

/// A node that joined late: the height of the books it was handed, what they held, and how many
/// blocks it replayed after them to reach the chain's ledger.
pub struct Joined {
    pub at: u64,
    pub accounts: usize,
    pub tokens: usize,
    pub replayed: usize,
}

/// The same world run again from its seed with no chain allowed, as far as the world has gone:
/// what it would have traded across its borders and earned without one. `None` for a world that
/// has founded no chain, which has nothing to compare. Cheap — it is the chain that costs time.
pub fn without_a_chain(world: &Nations) -> Option<Nations> {
    if world.networks.is_empty() {
        return None;
    }
    let mut other = Nations::found(world.seed);
    other.chains_are_possible = false;
    other.borders_are_free = world.borders_are_free;
    other.trade_is_possible = world.trade_is_possible;
    other.run(world.year);
    Some(other)
}


/// Replay every chain a world keeps from its genesis, with nothing taken on trust.
pub fn check(world: &Nations) -> Vec<Checked> {
    world
        .networks
        .iter()
        .map(|network| {
            let started = std::time::Instant::now();
            let ok = network
                .chain
                .verify()
                .map_err(|(height, why)| format!("block {height}: {why:?}"));
            let seconds = started.elapsed().as_secs_f64();
            let started = std::time::Instant::now();
            let light = chain::light::follow(&network.chain.genesis, &network.chain.light_blocks())
                .map_err(|(height, why)| format!("block {height}: {why:?}"));
            let light_seconds = started.elapsed().as_secs_f64();
            let started = std::time::Instant::now();
            let joined = network.books.as_ref().map(|(at, books)| join(network, *at, books));
            Checked {
                ok,
                seconds,
                blocks: network.chain.blocks.len(),
                light,
                light_seconds,
                joined,
                join_seconds: started.elapsed().as_secs_f64(),
            }
        })
        .collect()
}

/// Join a chain as a node arriving late would: its headers followed from the genesis to the
/// height of the books it is handed, the books checked against that header's state root, and
/// only the blocks since replayed — and whether that arrives where the chain is.
fn join(network: &Network, at: u64, books: &[Vec<u8>]) -> Result<Joined, String> {
    let chain = &network.chain;
    let headers: Vec<chain::LightBlock> = (0..=at).filter_map(|h| chain.light_block(h)).collect();
    let since = chain.blocks.get(at as usize + 1..).unwrap_or(&[]);
    match chain::join(&chain.genesis, &headers, books, since) {
        Ok(ledger) if ledger == chain.ledger => Ok(Joined {
            at,
            accounts: ledger.accounts.len(),
            tokens: ledger.tokens.len(),
            replayed: since.len(),
        }),
        Ok(_) => Err("it arrived at other books than the chain's".to_string()),
        Err((height, why)) => Err(format!("block {height}: {why:?}")),
    }
}

/// A number of people or years of food, the way a person reads one.
pub fn amount(value: f64) -> String {
    let v = value.abs();
    let sign = if value < 0.0 { "-" } else { "" };
    if v >= 1e12 {
        format!("{sign}{:.2}T", v / 1e12)
    } else if v >= 1e9 {
        format!("{sign}{:.2}B", v / 1e9)
    } else if v >= 1e6 {
        format!("{sign}{:.1}M", v / 1e6)
    } else if v >= 1e4 {
        format!("{sign}{:.0}k", v / 1e3)
    } else {
        format!("{sign}{v:.0}")
    }
}

/// Income a head across some towns, in years of food, weighed by who lives where — the measure
/// the world's own reading takes, so a country's and a state's add up to the world's.
fn income_of(world: &Nations, towns: &[usize]) -> f64 {
    let people: f64 = towns.iter().map(|t| world.towns[*t].people).sum();
    towns
        .iter()
        .map(|t| world.towns[*t].people * world.towns[*t].income)
        .sum::<f64>()
        / people.max(1.0)
}

/// A number of bytes, the way a person reads one.
pub fn size(bytes: usize) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.0} kB", b as f64 / (1 << 10) as f64),
        b => format!("{b} bytes"),
    }
}

/// Check a chain file with nothing but the file, as somebody would who was handed it: read it
/// strictly — and ask that it is the one way there is to write what it holds — then replay every
/// block from its genesis, every signature and root again, and say what the ledger comes to.
/// The lines to print, and whether it holds.
pub fn verify_file(bytes: &[u8]) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let (genesis, blocks) = match chain::file::read(bytes) {
        Ok(read) => read,
        Err(e) => {
            out.push(match e.block {
                Some(at) => format!("  it does not read as a chain: block {at} is not a block ({:?})", e.why),
                None => format!("  it does not read as a chain ({:?})", e.why),
            });
            return (out, false);
        }
    };
    let Some(last) = blocks.last() else {
        out.push("  it holds a genesis and no blocks, not even the first".to_string());
        return (out, false);
    };
    let (year, month) = when(last.header.time);
    out.push(format!(
        "  the {}: genesis {}…, {} blocks, the last at height {}, year {year} month {month}",
        genesis.params.name,
        genesis.id().short(),
        blocks.len(),
        last.header.height
    ));
    if chain::file::write(&genesis, &blocks) != bytes {
        out.push("  it reads, but is not the one way of writing what it holds".to_string());
        return (out, false);
    }
    let started = std::time::Instant::now();
    match chain::Chain::replay(&genesis, &blocks) {
        Ok(ledger) => {
            out.push(format!(
                "  replayed from its genesis: every signature and root checked, in {:.1}s — it holds",
                started.elapsed().as_secs_f64()
            ));
            out.push(format!(
                "  coin: {} in existence ({} at genesis, {} issued since, {} burned); {} validators, {} accounts",
                grouped(ledger.coin_supply / COIN),
                grouped(ledger.genesis_coin / COIN),
                grouped(ledger.issued / COIN),
                coin(ledger.slashed),
                ledger.validators().len(),
                ledger.accounts.len()
            ));
            for token in &ledger.tokens {
                out.push(format!(
                    "  {}: {} in circulation, {} held in reserve as its attestor last said",
                    token.symbol,
                    grouped(token.supply / TOKEN_UNIT),
                    grouped(token.reserves / TOKEN_UNIT)
                ));
            }
            let carried: Vec<String> = tally_blocks(&blocks)
                .into_iter()
                .map(|(what, n)| format!("{} {what}", grouped(n as u128)))
                .collect();
            if !carried.is_empty() {
                out.push(format!("  it carries {}", carried.join(", ")));
            }
            out.push(format!("  its last state root: {}", chain::hex(&last.header.state.0)));
            (out, true)
        }
        Err((height, why)) => {
            out.push(format!("  it does not replay: at height {height}, {why:?}"));
            (out, false)
        }
    }
}

/// A whole number with thousands separated, for base units and prices.
fn grouped(value: u128) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Coin, from base units: whole coin where there is plenty, three figures where there is less —
/// in a rich world one coin is worth millions, and a swap buys a fraction of one.
fn coin(amount: u128) -> String {
    if amount >= 100 * COIN || amount == 0 {
        return grouped(amount / COIN);
    }
    let value = amount as f64 / COIN as f64;
    let decimals = (2 - value.log10().floor() as i64).clamp(0, 8) as usize;
    format!("{value:.decimals$}")
}

fn percent(value: f64) -> String {
    format!("{:.1}%", 100.0 * value)
}

fn town_name(world: &Nations, town: usize) -> &str {
    world.towns.get(town).map(|t| t.name.as_str()).unwrap_or("?")
}

fn country_of_town(world: &Nations, town: usize) -> &str {
    world
        .towns
        .get(town)
        .and_then(|t| world.countries.get(t.country))
        .map(|c| c.name.as_str())
        .unwrap_or("?")
}

/// Who holds an address on a chain, by the town whose house it is.
fn holder(world: &Nations, network: &Network, address: &Address) -> String {
    network
        .town_of(address)
        .map(|t| town_name(world, t).to_string())
        .unwrap_or_else(|| address.short())
}

/// A line of the world's history, or `None` for something too routine to list one by one.
pub fn describe(world: &Nations, event: &Event, first_money: bool) -> Option<String> {
    let line = match event {
        Event::Monetised { town, medium, .. } => {
            if !first_money {
                return None;
            }
            format!(
                "{}'s trade comes to run on {} — the first money anywhere",
                town_name(world, *town),
                medium.label()
            )
        }
        // Struck by a town, not a country: countries are drawn again as the world changes and
        // a currency keeps the name it was struck under, as real ones do.
        Event::Coined { currency, .. } => {
            let c = &world.currencies[*currency];
            format!(
                "{} strikes the {} ({}), made of {}",
                town_name(world, world.issuers[*currency]),
                c.name,
                c.symbol,
                c.medium.label()
            )
        }
        Event::Reserve { currency, .. } => {
            format!(
                "the world comes to invoice in the {}",
                world.currencies[*currency].name
            )
        }
        Event::Famine { .. } => return None,
        Event::TrapOpened { .. } => {
            "income per head passes twice what it takes to eat, and stays there: the trap opens"
                .to_string()
        }
        Event::Founded { network, .. } => {
            let n = &world.networks[*network];
            let mut countries: Vec<&str> = n.founders.iter().map(|t| country_of_town(world, *t)).collect();
            countries.sort_unstable();
            countries.dedup();
            let houses: Vec<&str> = n.founders.iter().map(|t| town_name(world, *t)).collect();
            format!(
                "{} houses in {} countries found the {}: {}",
                n.founders.len(),
                countries.len(),
                n.name,
                houses.join(", ")
            )
        }
        Event::Issued { network, symbol, .. } => {
            let n = &world.networks[*network];
            match n.token.as_ref() {
                Some(token) => format!(
                    "{}'s house issues {} on the {}, standing for the {}, its reserve vouched for by {}",
                    town_name(world, token.issuer),
                    symbol,
                    n.name,
                    world.currencies[token.currency].name,
                    town_name(world, token.attestor)
                ),
                None => format!("{symbol} is issued on the {}", n.name),
            }
        }
        Event::Joined {
            network,
            town,
            new_key,
            ..
        } => format!(
            "{} takes a seat among the {}'s validators{}",
            town_name(world, *town),
            world.networks[*network].name,
            if *new_key { " again, under a key it keeps for staking" } else { "" }
        ),
        Event::Left { network, town, .. } => format!(
            "{} leaves the {}'s validators and takes its stake back, its own payments on the chain having dwindled",
            town_name(world, *town),
            world.networks[*network].name
        ),
        Event::Slashed {
            network,
            town,
            by,
            height,
            burned,
            ..
        } => format!(
            "{}'s house signs block {} of the {} twice — its second clerk, not having seen the block, signs for none — and {} shows the chain both: {} coin of its stake is burned and that key never validates again",
            town_name(world, *town),
            height,
            world.networks[*network].name,
            town_name(world, *by),
            coin(*burned)
        ),
        Event::Stalled { network, height, .. } => format!(
            "the {} stalls at height {}: more than a third of its stake is absent",
            world.networks[*network].name, height
        ),
    };
    Some(line)
}

/// The world's history, with the routine left out and famines counted by century.
pub fn history(world: &Nations) -> Vec<String> {
    let mut lines = Vec::new();
    let mut first_money = true;
    for event in &world.history {
        let is_money = matches!(event, Event::Monetised { .. });
        if let Some(text) = describe(world, event, first_money && is_money) {
            lines.push(format!("  year {:>4}  {}", event.year(), text));
        }
        if is_money {
            first_money = false;
        }
    }
    let famines: Vec<u64> = world
        .history
        .iter()
        .filter_map(|e| match e {
            Event::Famine { year, .. } => Some(*year),
            _ => None,
        })
        .collect();
    if !famines.is_empty() {
        let mut by_century: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
        for year in &famines {
            *by_century.entry(year / 100).or_insert(0) += 1;
        }
        let parts: Vec<String> = by_century
            .iter()
            .map(|(c, n)| match c {
                0 => format!("{n} in the first century"),
                c => format!("{n} in the {}s", c * 100),
            })
            .collect();
        lines.push(format!("  and {} famines — {}", famines.len(), parts.join(", ")));
    }
    lines
}

/// The whole report, for a terminal.
pub fn report(world: &Nations, checked: &[Checked]) -> Vec<String> {
    let mut out = Vec::new();
    let reading = world.readings.last();
    out.push(format!(
        "  {} towns on {} cells of the planet, grouped into {} states in {} {}",
        world.towns.len(),
        world.surface.planet.grid().len(),
        world.states.len(),
        world.countries.len(),
        if world.countries.len() == 1 { "country" } else { "countries" }
    ));
    if let Some(r) = reading {
        out.push(format!(
            "  {} people, making {:.1} times what it takes to feed them; {} of it is traded across a border",
            amount(r.people),
            r.income,
            percent(r.traded)
        ));
    }
    out.push(String::new());

    out.push("── what happened ──".to_string());
    out.extend(history(world));
    out.push(String::new());

    out.push("── the world, by century ──".to_string());
    out.push(format!(
        "  {:>5} {:>9} {:>7} {:>8} {:>8} {:>7} {:>10} {:>9}",
        "year", "people", "income", "hungry", "farmers", "traded", "currencies", "on chain"
    ));
    for r in world.readings.iter().filter(|r| r.year % 100 == 0 || r.year == world.year) {
        out.push(format!(
            "  {:>5} {:>9} {:>7.2} {:>8} {:>8} {:>7} {:>10} {:>9}",
            r.year,
            amount(r.people),
            r.income,
            percent(r.hunger),
            percent(r.shares[0]),
            percent(r.traded),
            r.currencies,
            if r.blocks > 0 { percent(r.on_chain) } else { "—".to_string() }
        ));
    }
    out.push(String::new());

    out.push("── countries ──".to_string());
    out.push(format!(
        "  {:<14} {:>8} {:>7} {:>6} {:>6}  {:<22} {:>14} {:>9} {:>10} {:>9}",
        "country", "people", "income", "towns", "states", "money", "a year's food", "inflation", "pay abroad", "variety"
    ));
    for country in &world.countries {
        let (money, price, inflation) = match country.currency {
            Some(id) => {
                let c = &world.currencies[id];
                (
                    format!("{} ({})", c.name, c.symbol),
                    format!("{} {}", grouped(c.level.round().max(0.0) as u128), c.symbol),
                    format!("{:+.1}%", 100.0 * c.inflation),
                )
            }
            None => ("barter and metal".to_string(), "—".to_string(), "—".to_string()),
        };
        out.push(format!(
            "  {:<14} {:>8} {:>7.1} {:>6} {:>6}  {:<22} {:>14} {:>9} {:>10} {:>9}",
            country.name,
            amount(country.people),
            income_of(world, &country.towns),
            country.towns.len(),
            country.states.len(),
            money,
            price,
            inflation,
            percent(country.pay_cost),
            format!("+{}", percent(country.variety - 1.0))
        ));
    }
    if world.countries.len() > 1 {
        out.push(
            "  variety: what a head's wares are worth for coming from more than one country, beyond the same spent at home"
                .to_string(),
        );
    }
    out.push(String::new());

    out.push("── states ──".to_string());
    out.push("  each a market area, by its hub: towns, people, and income a head in years of food".to_string());
    for country in &world.countries {
        let states: Vec<String> = country
            .states
            .iter()
            .map(|s| {
                let state = &world.states[*s];
                let people: f64 = state.towns.iter().map(|t| world.towns[*t].people).sum();
                let income = income_of(world, &state.towns);
                let towns = state.towns.len();
                format!(
                    "{} ({towns} {}, {}, {:.0}×)",
                    town_name(world, state.hub),
                    if towns == 1 { "town" } else { "towns" },
                    amount(people),
                    income
                )
            })
            .collect();
        out.push(format!("  {}: {}", country.name, states.join(" · ")));
    }
    out.push(String::new());

    if world.networks.is_empty() {
        out.push("── no ledger nobody keeps ──".to_string());
        out.push(format!(
            "  {}",
            match world.not_yet {
                _ if !world.chains_are_possible => "this run allows none: it is the world run --without chain".to_string(),
                Some(commerce::payments::NotYet::OneCountry) =>
                    "one country: there is a house everybody can pay through, and no border to pay across".to_string(),
                Some(commerce::payments::NotYet::TooFew) =>
                    "fewer than four houses do business abroad".to_string(),
                Some(commerce::payments::NotYet::CannotCheck) =>
                    "no four houses can yet check a chain as fast as it would have to be checked".to_string(),
                Some(commerce::payments::NotYet::NotWorthIt) =>
                    "checking a chain would still cost more than the distrust it would save".to_string(),
                Some(commerce::payments::NotYet::TrustedKeeper(_)) =>
                    "one house is trusted by all the rest, so they keep their books with it".to_string(),
                None => "nobody has asked yet".to_string(),
            }
        ));
    }
    for (at, network) in world.networks.iter().enumerate() {
        out.extend(ledger(world, at, network, checked.get(at)));
    }
    out
}

fn ledger(world: &Nations, at: usize, network: &Network, checked: Option<&Checked>) -> Vec<String> {
    let mut out = Vec::new();
    let chain = &network.chain;
    let ledger = &chain.ledger;
    out.push(format!("── the {} ──", network.name));
    let founders: Vec<&str> = network.founders.iter().map(|t| town_name(world, *t)).collect();
    out.push(format!("  founded in year {} by {}", network.founded, founders.join(", ")));
    out.push(format!(
        "  height {} · {} validators · {} rounds lost · {} stalls · {} transactions refused",
        chain.height(),
        network.validators().len(),
        chain.rounds_failed,
        network.stalls,
        network.refused
    ));
    if let Some((country, share)) = nations::network::largest_country_share(world, at) {
        // What the cap does not stop: a third of the stake away leaves a height short of the
        // two thirds, so whoever holds that much can stop the chain, if not finalise anything.
        let members = &chain.rotation.members;
        let total: u128 = members.iter().map(|v| v.power as u128).sum();
        let of = |country: usize| -> u128 {
            members
                .iter()
                .filter(|v| network.town_of(&v.address).is_some_and(|t| world.towns[t].country == country))
                .map(|v| v.power as u128)
                .sum()
        };
        let stops = if 3 * of(country) >= total {
            ", but a third is enough to stop one: were its houses all to stay away, the chain would wait for them"
        } else {
            ", and short of the third it would take to stop one"
        };
        out.push(format!(
            "  the most stake any one country's houses hold is {}'s, {} — short of the two thirds a block needs, so every block is signed abroad too{stops}",
            world.countries[country].name,
            percent(share)
        ));
        if let Some(biggest) = members.iter().max_by_key(|v| (v.power, std::cmp::Reverse(v.address)))
            && 3 * biggest.power as u128 >= total
        {
            // Held to three tenths wherever its country's share allows: over it, it is the only
            // house its country has to hold that share with.
            let alone = network.town_of(&biggest.address).is_some_and(|town| {
                let country = world.towns[town].country;
                network.houses().filter(|t| world.towns[*t].country == country).count() == 1
            });
            out.push(format!(
                "  one house, {}'s, holds {} of the stake by itself — enough to stop the chain alone by staying away{}",
                holder(world, network, &biggest.address),
                percent(biggest.power as f64 / total.max(1) as f64),
                if alone { ", being the only house its country has to hold its share" } else { "" }
            ));
        }
    }
    let reserve = network.token.as_ref().map(|t| world.currencies[t.currency].symbol.clone());
    out.push(format!(
        "  coin: {} in existence ({} at genesis, {} issued since, {} burned), one worth {} {}",
        grouped(ledger.coin_supply / COIN),
        grouped(ledger.genesis_coin / COIN),
        grouped(ledger.issued / COIN),
        grouped(ledger.slashed / COIN),
        grouped(network.coin_price.round().max(0.0) as u128),
        reserve.clone().unwrap_or_default()
    ));
    // What forging would take: two final blocks at one height need more than a third of the
    // stake to sign both, and the evidence burns a twentieth of whatever did.
    if let Some(token) = network.token.as_ref() {
        let third = chain.rotation.total_power() as f64 / 3.0;
        let level = world.currencies[token.currency].level.max(1e-9);
        out.push(format!(
            "  to finalise two blocks at one height, validators with more than a third of the stake — {} coin, worth {} years of food — would have to sign both, and the evidence would burn a twentieth of it",
            grouped(third.ceil() as u128),
            amount(third * network.coin_price / level)
        ));
    }
    if let Some(token) = network.token.as_ref()
        && let Some(on_chain) = chain.token(token.id)
    {
        out.push(format!(
            "  {}: {} in circulation, {} held in reserve and attested by {} — backed {:.4}",
            on_chain.symbol,
            grouped(on_chain.supply / TOKEN_UNIT),
            grouped(on_chain.reserves / TOKEN_UNIT),
            town_name(world, token.attestor),
            on_chain.backing()
        ));
        out.push(format!(
            "  {} minted and {} redeemed since it was issued, all of it by {}",
            grouped(on_chain.minted / TOKEN_UNIT),
            grouped(on_chain.redeemed / TOKEN_UNIT),
            town_name(world, token.issuer)
        ));
    }
    let share = world.readings.last().map(|r| r.on_chain).unwrap_or(0.0);
    out.push(format!(
        "  last year it settled {} years of food — {} of all payments abroad — for {} in fees",
        amount(network.carried),
        percent(share),
        amount(network.fees)
    ));
    if let Some(houses) = world.cost_through_houses() {
        let now = world.countries.iter().map(|c| c.pay_cost).sum::<f64>() / world.countries.len() as f64;
        out.push(format!(
            "  paying abroad costs {} of the payment now, against {} through houses alone",
            percent(now),
            percent(houses)
        ));
    }
    if let Some(unchained) = without_a_chain(world)
        && let (Some(without), Some(with)) = (unchained.readings.last(), world.readings.last())
    {
        out.push(format!(
            "  run again with no chain allowed, the same world trades {} of what it makes across its borders rather than {}, and a head makes {:.1} times subsistence rather than {:.1}",
            percent(without.traded),
            percent(with.traded),
            without.income,
            with.income
        ));
        // The smallest country, as the same country in both worlds: the one with the same key.
        let smallest = world.countries.last().and_then(|c| {
            let other = unchained.countries.iter().find(|o| o.key == c.key)?;
            Some((c, world.lives_on(c) / unchained.lives_on(other).max(1e-9) - 1.0))
        });
        out.push(format!(
            "  counting what wares are worth for coming from more than one country, a head lives on {} {} for the chain{}",
            percent((with.with_variety / without.with_variety.max(1e-9) - 1.0).abs()),
            if with.with_variety >= without.with_variety { "more" } else { "less" },
            match smallest {
                Some((c, gain)) => format!(
                    " — in {}, the smallest country, {} {}",
                    c.name,
                    percent(gain.abs()),
                    if gain >= 0.0 { "more" } else { "less" }
                ),
                None => String::new(),
            }
        ));
    }

    out.push(format!("  {:<14} {:<14} {:>11}  {:>6}", "validator", "country", "coin staked", "power"));
    let mut validators: Vec<(usize, u64)> = network
        .validators()
        .into_iter()
        .filter_map(|t| {
            let key = network.staking_address(t)?;
            let power = chain
                .rotation
                .members
                .iter()
                .find(|v| v.address == key)
                .map(|v| v.power)?;
            Some((t, power))
        })
        .collect();
    validators.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: u64 = validators.iter().map(|(_, p)| p).sum();
    for (town, power) in validators.iter().take(12) {
        out.push(format!(
            "  {:<14} {:<14} {:>11}  {:>6}",
            town_name(world, *town),
            country_of_town(world, *town),
            grouped(*power as u128),
            percent(*power as f64 / total.max(1) as f64)
        ));
    }
    if validators.len() > 12 {
        out.push(format!("  … and {} more", validators.len() - 12));
    }

    let history: Vec<String> = tally(network)
        .into_iter()
        .map(|(kind, n)| format!("{} {kind}", grouped(n as u128)))
        .collect();
    out.push(format!("  since its genesis it has carried {}", history.join(", ")));

    let tip = chain.tip();
    let (year, month) = when(tip.header.time);
    // A block put forward in one round can be final in a later one, when a round between failed
    // after somebody had signed it (`Chain::step`).
    let rounds = if tip.commit.round == tip.header.round {
        format!("round {}", tip.commit.round)
    } else {
        format!("put forward in round {} and final in round {}", tip.header.round, tip.commit.round)
    };
    out.push(format!(
        "  the latest block, #{}, year {year} month {month}: proposed by {}, {rounds}, {} transactions, {} signatures",
        tip.header.height,
        holder(world, network, &tip.header.proposer),
        tip.txs.len(),
        tip.commit.votes.len()
    ));
    out.push(format!("    hash        {}", tip.hash()));
    out.push(format!("    parent      {}", tip.header.parent));
    out.push(format!("    state root  {}", tip.header.state));
    for tx in tip.txs.iter().take(8) {
        out.push(format!("    {}", transaction(world, network, tx)));
    }
    if tip.txs.len() > 8 {
        out.push(format!("    … and {} more", tip.txs.len() - 8));
    }
    if let Some(checked) = checked {
        out.push(match &checked.ok {
            Ok(()) => format!(
                "  replayed from genesis: {} blocks, every signature and root checked, in {:.1}s — it holds",
                checked.blocks, checked.seconds
            ),
            Err(why) => format!("  replayed from genesis: FAILED at {why}"),
        });
        out.push(match &checked.light {
            Ok(height) => format!(
                "  followed as a light client, by headers, commits and validator sets alone: to height {height} in {:.1}s, never a transaction",
                checked.light_seconds
            ),
            Err(why) => format!("  followed as a light client: FAILED at {why}"),
        });
        match &checked.joined {
            Some(Ok(joined)) => out.push(format!(
                "  joined late, as a new node would: {} headers followed from the genesis, the books as they stood at #{} — {} accounts and {} {} — checked against its state root, and the {} blocks since replayed, in {:.1}s; it arrives at the ledger the chain holds, to the byte",
                grouped(joined.at as u128 + 1),
                joined.at,
                joined.accounts,
                joined.tokens,
                if joined.tokens == 1 { "token" } else { "tokens" },
                joined.replayed,
                checked.join_seconds
            )),
            Some(Err(why)) => out.push(format!("  joined late: FAILED at {why}")),
            None => {}
        }
    }
    out.push(String::new());
    out
}

/// Every transaction a chain has carried since its genesis, by what it did: payments in its
/// token, swaps of coin for it, mints, redemptions, attestations, and everything else.
fn tally(network: &Network) -> Vec<(&'static str, usize)> {
    tally_blocks(&network.chain.blocks)
}

/// The same for any blocks — a chain read from a file, with no world behind it.
fn tally_blocks(blocks: &[chain::Block]) -> Vec<(&'static str, usize)> {
    let mut counts: std::collections::BTreeMap<(&'static str, &'static str), usize> =
        std::collections::BTreeMap::new();
    for block in blocks {
        for tx in &block.txs {
            let kind = match &tx.action {
                Action::Pay {
                    asset: Asset::Token(_),
                    ..
                } => ("token payment", "token payments"),
                Action::Pay { .. } => ("coin transfer", "coin transfers"),
                Action::Swap(_) => ("swap of coin for tokens", "swaps of coin for tokens"),
                Action::Mint { .. } => ("mint", "mints"),
                Action::Redeem { .. } => ("redemption", "redemptions"),
                Action::Attest { .. } => ("attestation", "attestations"),
                Action::Bond { .. } => ("stake bonded", "stakes bonded"),
                Action::Unbond { .. } => ("stake unbonded", "stakes unbonded"),
                Action::Issue { .. } => ("token issued", "tokens issued"),
                Action::Evidence { .. } => ("piece of evidence", "pieces of evidence"),
            };
            *counts.entry(kind).or_insert(0) += 1;
        }
    }
    let mut counts: Vec<(&'static str, usize)> = counts
        .into_iter()
        .map(|((one, many), n)| (if n == 1 { one } else { many }, n))
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    counts
}

/// The year and month a block's time falls in.
fn when(seconds: u64) -> (u64, u64) {
    const MONTH: u64 = 2_629_800;
    let months = seconds / MONTH;
    ((months.saturating_sub(1)) / 12, (months.saturating_sub(1)) % 12 + 1)
}

fn transaction(world: &Nations, network: &Network, tx: &chain::Transaction) -> String {
    let from = holder(world, network, &tx.sender());
    let token = network
        .token
        .as_ref()
        .map(|t| t.symbol.clone())
        .unwrap_or_else(|| "token".to_string());
    match &tx.action {
        Action::Pay { to, asset, amount } => match asset {
            Asset::Coin => format!(
                "{from} pays {} {} coin",
                holder(world, network, to),
                coin(*amount)
            ),
            Asset::Token(_) => format!(
                "{from} pays {} {} {token}",
                holder(world, network, to),
                grouped(amount / TOKEN_UNIT)
            ),
        },
        Action::Attest { reserves, .. } => {
            format!("{from} attests a reserve of {} for {token}", grouped(reserves / TOKEN_UNIT))
        }
        Action::Mint { to, amount, .. } => format!(
            "{from} mints {} {token} for {}",
            grouped(amount / TOKEN_UNIT),
            holder(world, network, to)
        ),
        Action::Redeem { amount, .. } => {
            format!("{from} redeems {} {token}", grouped(amount / TOKEN_UNIT))
        }
        Action::Issue { symbol, peg, .. } => format!("{from} registers {symbol}, standing for {peg}"),
        Action::Bond { amount } => format!("{from} stakes {} coin", coin(*amount)),
        Action::Unbond { amount } => format!("{from} unstakes {} coin", coin(*amount)),
        Action::Evidence { first, .. } => format!(
            "{from} shows that {} signed block {} twice, once for no block",
            holder(world, network, &Address::of(&first.validator)),
            first.height
        ),
        Action::Swap(swap) => {
            let other = holder(world, network, &Address::of(&swap.counterparty));
            let side = |(asset, amount): (Asset, u128)| match asset {
                Asset::Coin => format!("{} coin", coin(amount)),
                Asset::Token(_) => format!("{} {token}", grouped(amount / TOKEN_UNIT)),
            };
            format!("{from} sells {other} {} for {}, both legs at once", side(swap.give), side(swap.get))
        }
    }
}

// ---- the same, as data for a page ----------------------------------------------------

fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn num(value: f64) -> String {
    if !value.is_finite() {
        return "null".to_string();
    }
    let text = if value.abs() >= 1000.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.4}")
    };
    let trimmed = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    };
    match trimmed.as_str() {
        "" | "-" | "-0" => "0".to_string(),
        _ => trimmed,
    }
}

fn list(items: impl Iterator<Item = String>) -> String {
    format!("[{}]", items.collect::<Vec<_>>().join(","))
}

/// How many of the latest blocks go out in full, transactions and signatures and all. The rest
/// go out as one line each.
const BLOCKS_IN_FULL: usize = 48;

/// Everything a page needs, as JSON.
pub fn snapshot(world: &Nations, checked: &[Checked]) -> String {
    let mut fields: Vec<String> = Vec::new();
    fields.push(format!("\"seed\":{}", quoted(&world.seed.to_string())));
    fields.push(format!("\"year\":{}", world.year));
    let towns = list(world.towns.iter().map(|t| {
        format!(
            "{{\"name\":{},\"lat\":{},\"lon\":{},\"people\":{},\"income\":{},\"country\":{},\"state\":{},\"coastal\":{},\"biome\":{},\"shares\":{},\"technique\":{},\"money\":{},\"hunger\":{}}}",
            quoted(&t.name),
            num(t.terrain.latitude as f64),
            num(t.terrain.longitude as f64),
            num(t.people),
            num(t.income),
            t.country,
            t.state,
            t.coastal,
            quoted(t.terrain.biome),
            list(t.shares.iter().map(|s| num(*s))),
            list(t.technique.iter().map(|s| num(*s))),
            match t.monetised {
                Some((year, medium)) => format!("{{\"year\":{year},\"medium\":{}}}", quoted(medium.label())),
                None => "null".to_string(),
            },
            num(t.hunger)
        )
    }));
    fields.push(format!("\"towns\":{towns}"));
    // The ground under them, a cell at a time, so the map can show where the land is.
    let grid = world.surface.planet.grid();
    let cells = list(grid.cells().map(|cell| {
        let at = grid.position(cell);
        format!(
            "[{:.1},{:.1},{}]",
            at.latitude().to_degrees(),
            at.longitude().to_degrees(),
            u8::from(world.surface.planet.is_land(cell))
        )
    }));
    fields.push(format!("\"cells\":{cells}"));
    let states = list(world.states.iter().map(|s| {
        format!(
            "{{\"hub\":{},\"country\":{},\"towns\":{}}}",
            s.hub,
            s.country,
            list(s.towns.iter().map(|t| t.to_string()))
        )
    }));
    fields.push(format!("\"states\":{states}"));
    let countries = list(world.countries.iter().map(|c| {
        format!(
            "{{\"name\":{},\"capital\":{},\"people\":{},\"product\":{},\"income\":{},\"currency\":{},\"payCost\":{},\"variety\":{},\"exports\":{},\"towns\":{},\"states\":{}}}",
            quoted(&c.name),
            c.capital,
            num(c.people),
            num(c.product),
            num(income_of(world, &c.towns)),
            c.currency.map(|x| x.to_string()).unwrap_or_else(|| "null".to_string()),
            num(c.pay_cost),
            num(c.variety),
            num(c.exports),
            list(c.towns.iter().map(|t| t.to_string())),
            list(c.states.iter().map(|s| s.to_string()))
        )
    }));
    fields.push(format!("\"countries\":{countries}"));
    let currencies = list(world.currencies.iter().enumerate().map(|(id, c)| {
        format!(
            "{{\"name\":{},\"symbol\":{},\"medium\":{},\"minted\":{},\"level\":{},\"inflation\":{},\"managed\":{},\"issuer\":{},\"reserve\":{}}}",
            quoted(&c.name),
            quoted(&c.symbol),
            quoted(c.medium.label()),
            c.minted,
            num(c.level),
            num(c.inflation),
            c.managed,
            world.issuers[id],
            world.reserve == Some(id)
        )
    }));
    fields.push(format!("\"currencies\":{currencies}"));
    let readings = list(world.readings.iter().map(|r| {
        format!(
            "[{},{},{},{},{},{},{},{},{},{}]",
            r.year,
            num(r.people),
            num(r.income),
            num(r.hunger),
            num(r.traded),
            num(r.shares[0]),
            num(r.monetised),
            num(r.on_chain),
            r.blocks,
            num(r.with_variety)
        )
    }));
    fields.push(format!("\"readings\":{readings}"));
    let mut first_money = true;
    let events = list(world.history.iter().filter_map(|e| {
        let is_money = matches!(e, Event::Monetised { .. });
        let line = describe(world, e, first_money && is_money);
        if is_money {
            first_money = false;
        }
        line.map(|text| format!("{{\"year\":{},\"text\":{}}}", e.year(), quoted(&text)))
    }));
    fields.push(format!("\"events\":{events}"));
    let chains = list(
        world
            .networks
            .iter()
            .enumerate()
            .map(|(at, n)| network_json(world, at, n, checked.get(at))),
    );
    fields.push(format!("\"chains\":{chains}"));
    fields.push(format!(
        "\"withoutChain\":{}",
        without_a_chain(world)
            .and_then(|w| w.readings.last().cloned())
            .map(|r| format!(
                "{{\"traded\":{},\"income\":{},\"withVariety\":{}}}",
                num(r.traded),
                num(r.income),
                num(r.with_variety)
            ))
            .unwrap_or_else(|| "null".to_string())
    ));
    fields.push(format!(
        "\"throughHouses\":{}",
        world.cost_through_houses().map(num).unwrap_or_else(|| "null".to_string())
    ));
    fields.push(format!(
        "\"notYet\":{}",
        world
            .not_yet
            .map(|w| quoted(&format!("{w:?}")))
            .unwrap_or_else(|| "null".to_string())
    ));
    fields.push(format!(
        "\"without\":{}",
        list(
            [
                (!world.chains_are_possible, "chain"),
                (world.borders_are_free, "borders"),
                (!world.trade_is_possible, "trade"),
            ]
            .into_iter()
            .filter(|(off, _)| *off)
            .map(|(_, what)| quoted(what))
        )
    ));
    format!("{{{}}}", fields.join(",\n"))
}

fn network_json(world: &Nations, at: usize, network: &Network, checked: Option<&Checked>) -> String {
    let chain = &network.chain;
    let address_town = |a: &Address| {
        network
            .town_of(a)
            .map(|t| t.to_string())
            .unwrap_or_else(|| "null".to_string())
    };
    let summary = list(chain.blocks.iter().map(|b| {
        // The validator a block's evidence convicts, if it carries any.
        let convicted = b
            .txs
            .iter()
            .find_map(|tx| match &tx.action {
                Action::Evidence { first, .. } => Some(address_town(&Address::of(&first.validator))),
                _ => None,
            })
            .unwrap_or_else(|| "null".to_string());
        format!(
            "[{},{},{},{},{},{},{convicted}]",
            b.header.height,
            b.header.time / 2_629_800,
            b.commit.round,
            address_town(&b.header.proposer),
            b.txs.len(),
            b.commit.votes.len()
        )
    }));
    let start = chain.blocks.len().saturating_sub(BLOCKS_IN_FULL);
    let full = list(chain.blocks[start..].iter().map(|b| {
        let txs = list(b.txs.iter().map(|tx| {
            // Where a token payment went, and how much, for drawing it on the map.
            let (to, amount) = match &tx.action {
                Action::Pay {
                    to,
                    asset: Asset::Token(_),
                    amount,
                } => (address_town(to), num(*amount as f64 / TOKEN_UNIT as f64)),
                _ => ("null".to_string(), "0".to_string()),
            };
            format!(
                "{{\"id\":{},\"from\":{},\"to\":{to},\"amount\":{amount},\"nonce\":{},\"fee\":{},\"what\":{},\"text\":{}}}",
                quoted(&tx.id().short()),
                address_town(&tx.sender()),
                tx.nonce,
                quoted(&grouped(tx.fee)),
                quoted(tx.action.label()),
                quoted(&transaction(world, network, tx))
            )
        }));
        let votes = list(b.commit.votes.iter().map(|v| {
            format!(
                "{{\"by\":{},\"signature\":{}}}",
                address_town(&Address::of(&v.validator)),
                quoted(&chain::hex(&v.signature.0[..12]))
            )
        }));
        format!(
            "{{\"height\":{},\"month\":{},\"round\":{},\"proposedIn\":{},\"proposer\":{},\"hash\":{},\"parent\":{},\"txRoot\":{},\"stateRoot\":{},\"validators\":{},\"lastCommit\":{},\"txs\":{txs},\"votes\":{votes}}}",
            b.header.height,
            b.header.time / 2_629_800,
            b.commit.round,
            b.header.round,
            address_town(&b.header.proposer),
            quoted(&b.hash().to_string()),
            quoted(&b.header.parent.to_string()),
            quoted(&b.header.txs.to_string()),
            quoted(&b.header.state.to_string()),
            quoted(&b.header.validators.to_string()),
            quoted(&b.header.last_commit.to_string())
        )
    }));
    let validators = list(chain.rotation.members.iter().map(|v| {
        format!(
            "{{\"town\":{},\"power\":{},\"address\":{}}}",
            address_town(&v.address),
            v.power,
            quoted(&v.address.to_string())
        )
    }));
    // Every house's own account, and after it any key it has staked with since its own was
    // jailed.
    let held_by: Vec<(usize, Address, bool)> = network
        .houses()
        .flat_map(|t| {
            network
                .addresses_of(t)
                .into_iter()
                .enumerate()
                .map(move |(i, a)| (t, a, i > 0))
        })
        .collect();
    let accounts = list(held_by.into_iter().map(|(t, address, staking)| {
        let held = chain.balance(&address, Asset::Coin);
        let tokens = network
            .token
            .as_ref()
            .map(|k| chain.balance(&address, Asset::Token(k.id)))
            .unwrap_or(0);
        let account = chain.ledger.account(&address);
        let bonded = account.map(|a| a.bonded).unwrap_or(0);
        let jailed = account.is_some_and(|a| a.jailed);
        format!(
            "{{\"town\":{t},\"address\":{},\"coin\":{},\"staked\":{},\"tokens\":{},\"jailed\":{jailed},\"staking\":{staking}}}",
            quoted(&address.to_string()),
            quoted(&coin(held)),
            quoted(&coin(bonded)),
            quoted(&grouped(tokens / TOKEN_UNIT))
        )
    }));
    let token = match network.token.as_ref() {
        Some(t) => match chain.token(t.id) {
            Some(on) => format!(
                "{{\"symbol\":{},\"peg\":{},\"issuer\":{},\"attestor\":{},\"supply\":{},\"reserves\":{},\"minted\":{},\"redeemed\":{},\"backing\":{}}}",
                quoted(&on.symbol),
                quoted(&on.peg),
                t.issuer,
                t.attestor,
                quoted(&grouped(on.supply / TOKEN_UNIT)),
                quoted(&grouped(on.reserves / TOKEN_UNIT)),
                quoted(&grouped(on.minted / TOKEN_UNIT)),
                quoted(&grouped(on.redeemed / TOKEN_UNIT)),
                num(on.backing())
            ),
            None => "null".to_string(),
        },
        None => "null".to_string(),
    };
    let ledger = &chain.ledger;
    let proof = check_it_yourself(network);
    let light = light_window(network);
    let history = list(
        tally(network)
            .into_iter()
            .map(|(kind, n)| format!("[{},{n}]", quoted(kind))),
    );
    let verified = match checked {
        Some(c) => format!(
            "{{\"ok\":{},\"why\":{},\"seconds\":{},\"blocks\":{},\"light\":{},\"lightSeconds\":{},\"joined\":{}}}",
            c.ok.is_ok(),
            quoted(c.ok.as_ref().err().map(|s| s.as_str()).unwrap_or("")),
            num(c.seconds),
            c.blocks,
            c.light.is_ok(),
            num(c.light_seconds),
            match &c.joined {
                Some(Ok(j)) => format!(
                    "{{\"at\":{},\"accounts\":{},\"replayed\":{},\"seconds\":{}}}",
                    j.at,
                    j.accounts,
                    j.replayed,
                    num(c.join_seconds)
                ),
                _ => "null".to_string(),
            }
        ),
        None => "null".to_string(),
    };
    let largest = match nations::network::largest_country_share(world, at) {
        Some((country, share)) => format!("{{\"country\":{country},\"share\":{}}}", num(share)),
        None => "null".to_string(),
    };
    format!(
        "{{\"name\":{},\"id\":{},\"founded\":{},\"founders\":{},\"largestCountry\":{largest},\"height\":{},\"stalls\":{},\"refused\":{},\"roundsLost\":{},\"coin\":{{\"supply\":{},\"genesis\":{},\"issued\":{},\"burned\":{},\"price\":{}}},\"carried\":{},\"fees\":{},\"cost\":{},\"token\":{token},\"validators\":{validators},\"accounts\":{accounts},\"blocks\":{summary},\"recent\":{full},\"proof\":{proof},\"light\":{light},\"history\":{history},\"verified\":{verified}}}",
        quoted(&network.name),
        quoted(&chain.id.to_string()),
        network.founded,
        list(network.founders.iter().map(|t| t.to_string())),
        chain.height(),
        network.stalls,
        network.refused,
        chain.rounds_failed,
        quoted(&grouped(ledger.coin_supply / COIN)),
        quoted(&grouped(ledger.genesis_coin / COIN)),
        quoted(&grouped(ledger.issued / COIN)),
        quoted(&grouped(ledger.slashed / COIN)),
        num(network.coin_price),
        num(network.carried),
        num(network.fees),
        num(network.cost)
    )
}

/// The latest block of a chain as bytes somebody can check without taking this program's word
/// for anything: the header, which hashes to the block's name and holds its state root; every
/// vote's signed bytes, key and signature; and for every house, its account's bytes in the
/// state tree with the path from them to that root.
struct Checkable {
    height: u64,
    header: Vec<u8>,
    hash: chain::Digest,
    state_root: chain::Digest,
    tx_root: chain::Digest,
    /// The validators that signed it — the set the header names — with the stake of each.
    validators_hash: chain::Digest,
    signers: Vec<(Address, chain::PublicKey, u64)>,
    /// Every transaction in the block, as the bytes whose hash is its id.
    txs: Vec<Vec<u8>>,
    /// Signed bytes, key, signature.
    votes: Vec<(Vec<u8>, chain::PublicKey, chain::Signature)>,
    accounts: Vec<Proved>,
    /// The whole of the books the state root is a root of — every account, every token and the
    /// totals — with how many accounts and tokens that is.
    books: Vec<Vec<u8>>,
    book_accounts: usize,
    book_tokens: usize,
}

/// One house's account as a leaf of the state tree, and the path from it to the root.
struct Proved {
    town: usize,
    leaf: Vec<u8>,
    index: usize,
    size: usize,
    path: Vec<chain::Digest>,
}

fn checkable(network: &Network) -> Checkable {
    let chain = &network.chain;
    let tip = chain.tip();
    Checkable {
        height: tip.header.height,
        header: tip.header.encode(),
        hash: tip.hash(),
        state_root: tip.header.state,
        tx_root: tip.header.txs,
        validators_hash: tip.header.validators,
        signers: chain
            .signers
            .members
            .iter()
            .map(|v| (v.address, v.key, v.power))
            .collect(),
        txs: tip.txs.iter().map(|tx| tx.encode()).collect(),
        votes: tip
            .commit
            .votes
            .iter()
            .map(|v| (v.payload(), v.validator, v.signature))
            .collect(),
        accounts: network
            .houses()
            .filter_map(|town| {
                let address = network.address_of(town)?;
                let proof = chain.ledger.prove(&address)?;
                Some(Proved {
                    town,
                    leaf: proof.account.encode(&address),
                    index: proof.index,
                    size: proof.size,
                    path: proof.path,
                })
            })
            .collect(),
        books: chain.ledger.snapshot(),
        book_accounts: chain.ledger.accounts.len(),
        book_tokens: chain.ledger.tokens.len(),
    }
}

/// How many of the latest blocks the page follows by their headers alone.
const LIGHT_WINDOW: u64 = 48;

/// The latest blocks as a light client is handed them (`chain::light`), for the page to follow
/// from the first of them: each header's bytes, and the round and signatures of the commit that
/// made it final — what each validator signed, the page works out for itself from the header —
/// with the validator sets that signed them, each given once, from the height it began.
fn light_blocks(network: &Network) -> Vec<chain::LightBlock> {
    let tip = network.chain.height();
    (tip.saturating_sub(LIGHT_WINDOW - 1)..=tip)
        .filter_map(|h| network.chain.light_block(h))
        .collect()
}

/// `light_blocks`, for the page.
fn light_window(network: &Network) -> String {
    let blocks = light_blocks(network);
    let mut sets: Vec<(u64, &chain::ValidatorSet)> = Vec::new();
    for b in &blocks {
        if sets.last().is_none_or(|(_, set)| set.hash() != b.header.validators) {
            sets.push((b.header.height, &b.validators));
        }
    }
    let sets = list(sets.iter().map(|(from, set)| {
        let members = list(set.members.iter().map(|v| {
            format!(
                "{{\"address\":{},\"key\":{},\"power\":{}}}",
                quoted(&v.address.to_string()),
                quoted(&chain::hex(&v.key.0)),
                v.power
            )
        }));
        format!("{{\"from\":{from},\"members\":{members}}}")
    }));
    let blocks = list(blocks.iter().map(|b| {
        let votes = list(b.commit.votes.iter().map(|v| {
            format!("[{},{}]", quoted(&chain::hex(&v.validator.0)), quoted(&chain::hex(&v.signature.0)))
        }));
        format!(
            "{{\"header\":{},\"round\":{},\"votes\":{votes}}}",
            quoted(&chain::hex(&b.header.encode())),
            b.commit.round
        )
    }));
    format!("{{\"sets\":{sets},\"blocks\":{blocks}}}")
}

/// `checkable`, for the page.
fn check_it_yourself(network: &Network) -> String {
    let it = checkable(network);
    let votes = list(it.votes.iter().map(|(payload, key, signature)| {
        format!(
            "{{\"payload\":{},\"key\":{},\"signature\":{}}}",
            quoted(&chain::hex(payload)),
            quoted(&chain::hex(&key.0)),
            quoted(&chain::hex(&signature.0))
        )
    }));
    let accounts = list(it.accounts.iter().map(|a| {
        format!(
            "{{\"town\":{},\"leaf\":{},\"index\":{},\"size\":{},\"path\":{}}}",
            a.town,
            quoted(&chain::hex(&a.leaf)),
            a.index,
            a.size,
            list(a.path.iter().map(|d| quoted(&d.to_string())))
        )
    }));
    let txs = list(it.txs.iter().map(|bytes| quoted(&chain::hex(bytes))));
    let books = format!(
        "{{\"leaves\":{},\"accounts\":{},\"tokens\":{}}}",
        list(it.books.iter().map(|leaf| quoted(&chain::hex(leaf)))),
        it.book_accounts,
        it.book_tokens
    );
    let signers = list(it.signers.iter().map(|(address, key, power)| {
        format!(
            "{{\"address\":{},\"key\":{},\"power\":{power}}}",
            quoted(&address.to_string()),
            quoted(&chain::hex(&key.0))
        )
    }));
    format!(
        "{{\"height\":{},\"header\":{},\"hash\":{},\"stateRoot\":{},\"txRoot\":{},\"validatorsHash\":{},\"signers\":{signers},\"txs\":{txs},\"votes\":{votes},\"accounts\":{accounts},\"books\":{books}}}",
        it.height,
        quoted(&chain::hex(&it.header)),
        quoted(&it.hash.to_string()),
        quoted(&it.state_root.to_string()),
        quoted(&it.tx_root.to_string()),
        quoted(&it.validators_hash.to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_the_way_people_read_them() {
        assert_eq!(amount(1_234_567.0), "1.2M");
        assert_eq!(amount(9_876_543_210.0), "9.88B");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(grouped(12), "12");
        // A swap in a rich world buys part of a coin, and says which part.
        assert_eq!(coin(39_500_000), "0.395");
        assert_eq!(coin(550_000_000), "5.50");
        assert_eq!(coin(4_000 * COIN), "4,000");
        assert_eq!(coin(0), "0");
        assert_eq!(when(2_629_800), (0, 1));
        assert_eq!(when(2_629_800 * 13), (1, 1));
    }

    /// What the page hands a browser to check the latest block is what the chain committed to:
    /// the header hashes to the block's name and holds the state root, every vote is a real
    /// signature over bytes naming the block, and every house's leaf and path lead to the root.
    /// The page checks the same things again with the browser's own SHA-256 and Ed25519.
    #[test]
    fn a_browser_is_given_what_it_needs_to_check_the_latest_block() {
        let mut world = Nations::found(sim_core::WorldSeed::from_u128(0x11));
        while world.networks.is_empty() && world.year < 800 {
            world.year();
        }
        world.run(2);
        let it = checkable(&world.networks[0]);
        // The headers the page follows are ones a light client follows, from the first of them.
        let window = light_blocks(&world.networks[0]);
        assert_eq!(window.len() as u64, LIGHT_WINDOW.min(world.networks[0].chain.height() + 1));
        assert_eq!(
            chain::light::follow_from(&window[0], &window[1..]),
            Ok(world.networks[0].chain.height())
        );
        assert_eq!(chain::Digest::of(&it.header), it.hash);
        let holds_root = it.header.windows(32).any(|w| w == it.state_root.0);
        assert!(holds_root, "the header carries its state root");
        assert!(!it.votes.is_empty());
        assert_eq!(world.networks[0].chain.signers.hash(), it.validators_hash);
        assert_eq!(it.signers.len(), world.networks[0].chain.signers.members.len());
        for (payload, key, signature) in &it.votes {
            assert!(key.verify(payload, signature));
            assert!(payload.windows(32).any(|w| w == it.hash.0), "a vote names its block");
        }
        // The transactions: each hashes to its id, and the ids to the header's root.
        let ids: Vec<chain::Digest> = it.txs.iter().map(|t| chain::merkle::leaf(&chain::Digest::of(t).0)).collect();
        assert_eq!(chain::merkle::root(&ids), it.tx_root);
        assert!(it.header.windows(32).any(|w| w == it.tx_root.0));
        let tip = world.networks[0].chain.tip();
        assert!(tip.txs.iter().all(|tx| tx.signature_holds()));
        assert_eq!(it.txs.len(), tip.txs.len());
        assert!(it.accounts.len() > 4);
        for a in &it.accounts {
            assert!(
                chain::merkle::verify(&it.state_root, &chain::merkle::leaf(&a.leaf), a.index, a.size, &a.path),
                "{}'s account",
                world.towns[a.town].name
            );
        }
        // And the whole of the books hashes up to that same root: every account, not only the
        // houses', each token and the totals.
        let books: Vec<chain::Digest> = it.books.iter().map(|l| chain::merkle::leaf(l)).collect();
        assert_eq!(chain::merkle::root(&books), it.state_root);
        assert_eq!(it.books.len(), it.book_accounts + it.book_tokens + 1);
        let page = check_it_yourself(&world.networks[0]);
        assert!(page.contains(&chain::hex(&it.header)));
        // And the whole chain, by its headers alone, as the report follows it, and joined late
        // from the books the year began with.
        let checked = check(&world);
        assert_eq!(checked[0].ok, Ok(()));
        assert_eq!(checked[0].light, Ok(world.networks[0].chain.height()));
        let joined = checked[0].joined.as_ref().expect("the world kept books").as_ref().unwrap();
        assert_eq!(joined.replayed as u64, world.networks[0].chain.height() - joined.at);
    }

    /// A chain written to a file is checked by somebody holding nothing else, and the same file
    /// with one bit changed, or its last byte gone, is not.
    #[test]
    fn a_chain_file_checks_on_its_own() {
        let mut world = Nations::found(sim_core::WorldSeed::from_u128(0x11));
        while world.networks.is_empty() && world.year < 800 {
            world.year();
        }
        world.run(2);
        let bytes = world.networks[0].chain.export();
        let (lines, holds) = verify_file(&bytes);
        assert!(holds, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("it holds")));
        // What it carries, read off the file alone, is what the world's own report says.
        let carried = tally(&world.networks[0])
            .into_iter()
            .map(|(what, n)| format!("{} {what}", grouped(n as u128)))
            .collect::<Vec<_>>()
            .join(", ");
        assert!(lines.iter().any(|l| l.ends_with(&carried)), "{lines:?}");
        let mut changed = bytes.clone();
        changed[bytes.len() / 2] ^= 1;
        let (lines, holds) = verify_file(&changed);
        assert!(!holds, "{lines:?}");
        assert!(!verify_file(&bytes[..bytes.len() - 1]).1);
    }

    #[test]
    fn a_young_world_reports_and_exports() {
        let mut world = Nations::found(sim_core::WorldSeed::from_u128(0x21));
        world.run(120);
        let lines = report(&world, &check(&world));
        assert!(lines.iter().any(|l| l.contains("── countries ──")));
        let data = snapshot(&world, &check(&world));
        assert!(data.starts_with('{') && data.ends_with('}'));
        // No number the page cannot read: Rust writes the non-finite ones as `NaN` and `inf`,
        // and JSON has neither. (Looked for where a value starts, since "rainforest" is fine.)
        for bad in ["NaN", "inf", "-inf"] {
            for before in [':', ',', '['] {
                assert!(!data.contains(&format!("{before}{bad}")), "{before}{bad} in the data");
            }
        }
        let (open, close) = (data.matches('{').count(), data.matches('}').count());
        assert_eq!(open, close);
    }
}
