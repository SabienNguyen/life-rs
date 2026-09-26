//! The world at the scale of nations, told: a history in lines, the countries and their money,
//! and the ledger nobody keeps — and the same as data for a page.

use chain::{Action, Address, Asset, COIN, TOKEN_UNIT};
use nations::{Event, Nations, Network};

/// What checking a chain from its genesis found, and how long it took.
pub struct Checked {
    pub ok: Result<(), String>,
    pub seconds: f64,
    pub blocks: usize,
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
            Checked {
                ok,
                seconds: started.elapsed().as_secs_f64(),
                blocks: network.chain.blocks.len(),
            }
        })
        .collect()
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
        Event::Joined { network, town, .. } => format!(
            "{} takes a seat among the {}'s validators",
            town_name(world, *town),
            world.networks[*network].name
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
        "  {:<14} {:>8} {:>7} {:>6} {:>6}  {:<22} {:>14} {:>9} {:>9}",
        "country", "people", "income", "towns", "states", "money", "a year's food", "inflation", "pay abroad"
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
            "  {:<14} {:>8} {:>7.1} {:>6} {:>6}  {:<22} {:>14} {:>9} {:>9}",
            country.name,
            amount(country.people),
            country.product / country.people.max(1.0),
            country.towns.len(),
            country.states.len(),
            money,
            price,
            inflation,
            percent(country.pay_cost)
        ));
    }
    out.push(String::new());

    out.push("── states ──".to_string());
    for country in &world.countries {
        let states: Vec<String> = country
            .states
            .iter()
            .map(|s| {
                let state = &world.states[*s];
                let people: f64 = state.towns.iter().map(|t| world.towns[*t].people).sum();
                let towns = state.towns.len();
                format!(
                    "{} ({towns} {}, {})",
                    town_name(world, state.hub),
                    if towns == 1 { "town" } else { "towns" },
                    amount(people)
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
        out.push(format!(
            "  the most stake any one country's houses hold is {}'s, {} — short of the two thirds a block needs, so every block is signed abroad too",
            world.countries[country].name,
            percent(share)
        ));
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

    out.push(format!("  {:<14} {:<14} {:>11}  {:>6}", "validator", "country", "coin staked", "power"));
    let mut validators: Vec<(usize, u64)> = network
        .validators()
        .into_iter()
        .filter_map(|t| {
            let key = network.address_of(t)?;
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

    let tip = chain.tip();
    let (year, month) = when(tip.header.time);
    out.push(format!(
        "  the latest block, #{}, year {year} month {month}: proposed by {}, round {}, {} transactions, {} signatures",
        tip.header.height,
        holder(world, network, &tip.header.proposer),
        tip.header.round,
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
    }
    out.push(String::new());
    out
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
                grouped(amount / COIN)
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
        Action::Bond { amount } => format!("{from} stakes {} coin", grouped(amount / COIN)),
        Action::Unbond { amount } => format!("{from} unstakes {} coin", grouped(amount / COIN)),
        Action::Evidence { .. } => format!("{from} shows two votes signed by one validator at one height"),
        Action::Swap(swap) => {
            let other = holder(world, network, &Address::of(&swap.counterparty));
            let side = |(asset, amount): (Asset, u128)| match asset {
                Asset::Coin => format!("{} coin", grouped(amount / COIN)),
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
            "{{\"name\":{},\"capital\":{},\"people\":{},\"product\":{},\"currency\":{},\"payCost\":{},\"exports\":{},\"towns\":{},\"states\":{}}}",
            quoted(&c.name),
            c.capital,
            num(c.people),
            num(c.product),
            c.currency.map(|x| x.to_string()).unwrap_or_else(|| "null".to_string()),
            num(c.pay_cost),
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
            "[{},{},{},{},{},{},{},{},{}]",
            r.year,
            num(r.people),
            num(r.income),
            num(r.hunger),
            num(r.traded),
            num(r.shares[0]),
            num(r.monetised),
            num(r.on_chain),
            r.blocks
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
        format!(
            "[{},{},{},{},{},{}]",
            b.header.height,
            b.header.time / 2_629_800,
            b.header.round,
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
            "{{\"height\":{},\"month\":{},\"round\":{},\"proposer\":{},\"hash\":{},\"parent\":{},\"txRoot\":{},\"stateRoot\":{},\"validators\":{},\"lastCommit\":{},\"txs\":{txs},\"votes\":{votes}}}",
            b.header.height,
            b.header.time / 2_629_800,
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
    let accounts = list(network.houses().filter_map(|t| {
        let address = network.address_of(t)?;
        let coin = chain.balance(&address, Asset::Coin);
        let tokens = network
            .token
            .as_ref()
            .map(|k| chain.balance(&address, Asset::Token(k.id)))
            .unwrap_or(0);
        let bonded = chain.ledger.account(&address).map(|a| a.bonded).unwrap_or(0);
        Some(format!(
            "{{\"town\":{t},\"address\":{},\"coin\":{},\"staked\":{},\"tokens\":{}}}",
            quoted(&address.to_string()),
            quoted(&grouped(coin / COIN)),
            quoted(&grouped(bonded / COIN)),
            quoted(&grouped(tokens / TOKEN_UNIT))
        ))
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
    let verified = match checked {
        Some(c) => format!(
            "{{\"ok\":{},\"why\":{},\"seconds\":{},\"blocks\":{}}}",
            c.ok.is_ok(),
            quoted(c.ok.as_ref().err().map(|s| s.as_str()).unwrap_or("")),
            num(c.seconds),
            c.blocks
        ),
        None => "null".to_string(),
    };
    let largest = match nations::network::largest_country_share(world, at) {
        Some((country, share)) => format!("{{\"country\":{country},\"share\":{}}}", num(share)),
        None => "null".to_string(),
    };
    format!(
        "{{\"name\":{},\"id\":{},\"founded\":{},\"founders\":{},\"largestCountry\":{largest},\"height\":{},\"stalls\":{},\"refused\":{},\"roundsLost\":{},\"coin\":{{\"supply\":{},\"genesis\":{},\"issued\":{},\"burned\":{},\"price\":{}}},\"carried\":{},\"fees\":{},\"cost\":{},\"token\":{token},\"validators\":{validators},\"accounts\":{accounts},\"blocks\":{summary},\"recent\":{full},\"verified\":{verified}}}",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_the_way_people_read_them() {
        assert_eq!(amount(1_234_567.0), "1.2M");
        assert_eq!(amount(9_876_543_210.0), "9.88B");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(grouped(12), "12");
        assert_eq!(when(2_629_800), (0, 1));
        assert_eq!(when(2_629_800 * 13), (1, 1));
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
