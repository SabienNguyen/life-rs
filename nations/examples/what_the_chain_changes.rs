//! What a world comes to, and what its chain changes in it.
//!
//! `who_builds_a_ledger` measures the chains. This measures the worlds around them, and asks the
//! question a chain has to answer: what would have been different without it? Each world is run
//! three times from its seed — as it is, with `chains_are_possible` off, and with
//! `borders_are_free` on — and the share of what it makes that crosses a border, its income, what
//! a head consumes counted at one price for a ware everywhere, and the same counting what its
//! wares are worth for coming from more than one country are set side by side, with what the
//! chain is worth to a head of the smallest country. Then the famine ablation §49.3 quotes:
//! 0xbeef's first hundred and fifty years with and without `trade_is_possible`.
//!
//! Prints §49.3's and §49.6.1's tables as they stand in the design document. `SEEDS` is a
//! comma-separated list of hex seeds and `YEARS` how long to run each.

use nations::{Event, Nations};
use sim_core::WorldSeed;

fn main() {
    let seeds: Vec<u128> = std::env::var("SEEDS")
        .unwrap_or_else(|_| "0x11,0x21,0x221,0xbeef,0x5eed,0x7,0x1234,0x2b,0xc0ffee".to_string())
        .split(',')
        .filter_map(|s| u128::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok())
        .collect();
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(700);
    let famines = |w: &Nations| {
        w.history
            .iter()
            .filter(|e| matches!(e, Event::Famine { .. }))
            .count()
    };

    let mut changes = Vec::new();
    println!(
        "| seed | towns | countries | first money | first coin | trap opens | people | income | farmers | traded | famines |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for seed in seeds {
        let run = |free: bool, chains: bool| {
            let mut world = Nations::found(WorldSeed::from_u128(seed));
            world.borders_are_free = free;
            world.chains_are_possible = chains;
            world.run(years);
            world
        };
        let world = run(false, true);
        let (Some(now), Some(then)) = (world.readings.last(), world.readings.get(10)) else {
            continue;
        };
        let year_of = |wanted: fn(&Event) -> bool| {
            world
                .history
                .iter()
                .find(|e| wanted(e))
                .map(|e| e.year().to_string())
                .unwrap_or_else(|| "—".to_string())
        };
        let traded = if world.countries.len() > 1 {
            format!("{:.1}%", 100.0 * now.traded)
        } else {
            "—".to_string()
        };
        println!(
            "| {seed:#x} | {} | {} | {} | {} | {} | {:.1}B | {:.0} | {:.0}% → {:.0}% | {traded} | {} |",
            world.towns.len(),
            world.countries.len(),
            year_of(|e| matches!(e, Event::Monetised { .. })),
            year_of(|e| matches!(e, Event::Coined { .. })),
            year_of(|e| matches!(e, Event::TrapOpened { .. })),
            now.people / 1e9,
            now.income,
            100.0 * then.shares[0],
            100.0 * now.shares[0],
            famines(&world),
        );
        if world.countries.len() > 1 {
            let (unchained, freed) = (run(false, false), run(true, true));
            let (Some(without), Some(free)) = (unchained.readings.last(), freed.readings.last()) else {
                continue;
            };
            // The smallest country, and the same country without the chain: the one with its key.
            let smallest = world.countries.last().and_then(|c| {
                let other = unchained.countries.iter().find(|o| o.key == c.key)?;
                Some(format!(
                    "{} +{:.1}%",
                    c.name,
                    100.0 * (world.lives_on(c) / unchained.lives_on(other) - 1.0)
                ))
            });
            let of_the_way = (now.traded - without.traded) / (free.traded - without.traded);
            changes.push(format!(
                "| {seed:#x} | {:.2}% | {:.2}% | {:.2}% | {:.0}% | {:.1} / {:.1} / {:.1} | {:.1} / {:.1} / {:.1} | {:.1} / {:.1} / {:.1} | {} |",
                100.0 * without.traded,
                100.0 * now.traded,
                100.0 * free.traded,
                100.0 * of_the_way,
                without.income,
                now.income,
                free.income,
                without.consumed,
                now.consumed,
                free.consumed,
                without.with_variety,
                now.with_variety,
                free.with_variety,
                smallest.unwrap_or_else(|| "—".to_string()),
            ));
        }
    }
    println!();
    println!(
        "| seed | traded abroad, no chain | with the chain | borders free | of the way | income: no chain / chain / free | consumed at one price | counting variety | smallest country, for the chain |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for row in changes {
        println!("{row}");
    }
    println!();
    for trade in [true, false] {
        let mut world = Nations::found(WorldSeed::from_u128(0xbeef));
        world.trade_is_possible = trade;
        world.run(150);
        let people = world.readings.last().map(|r| r.people).unwrap_or(0.0);
        println!(
            "0xbeef, 150 years, trade {}: {} famines, {:.1}M people",
            if trade { "possible" } else { "impossible" },
            famines(&world),
            people / 1e6
        );
    }
}
