//! Which worlds come to keep a ledger nobody keeps, when, and how it holds up.
//!
//! A chain is not written into a world; it is founded when houses in different countries,
//! none of whom the rest would trust with the books, find that checking one costs less than
//! the distrust it saves. So whether and when it happens is a measurement, and this is the
//! instrument: one line per world on when money came, when growth did, when a chain was
//! founded and by whom, the most of its stake one country's houses and one house hold — at the
//! end, and at any height from the second — what paying abroad cost with it and without it, and
//! how many of its validators were caught signing twice — and then the chain replayed from its
//! genesis, to say that what was kept can still be checked.
//!
//! `SEEDS` is a comma-separated list of hex seeds, `YEARS` how long to run each.

use std::collections::BTreeSet;
use std::time::Instant;

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

    println!(
        "{:>9} {:>5} {:>4} {:>6} {:>5} {:>7} {:>13} {:>5} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7} {:>6} {:>7} {:>6} {:>6}  replayed",
        "seed", "towns", "ctry", "money", "trap", "founded", "founders", "vals", "height",
        "stake", "house", "ever", "houses", "now", "chain", "refused", "stalls", "jailed"
    );
    for seed in seeds {
        let started = Instant::now();
        let mut world = Nations::found(WorldSeed::from_u128(seed));
        world.run(years);
        let ran = started.elapsed().as_secs_f64();
        let year_of = |wanted: fn(&Event) -> bool| {
            world
                .history
                .iter()
                .find(|e| wanted(e))
                .map(|e| e.year().to_string())
                .unwrap_or_else(|| "—".to_string())
        };
        let money = year_of(|e| matches!(e, Event::Monetised { .. }));
        let trap = year_of(|e| matches!(e, Event::TrapOpened { .. }));
        let houses = world
            .cost_through_houses()
            .map(|c| format!("{:.1}%", 100.0 * c))
            .unwrap_or_else(|| "—".to_string());
        let now = if world.countries.len() > 1 {
            let mean = world.countries.iter().map(|c| c.pay_cost).sum::<f64>()
                / world.countries.len() as f64;
            format!("{:.1}%", 100.0 * mean)
        } else {
            "—".to_string()
        };
        let share = world.readings.last().map(|r| r.on_chain).unwrap_or(0.0);
        match world.networks.first() {
            Some(network) => {
                let countries: BTreeSet<usize> =
                    network.founders.iter().map(|t| world.towns[*t].country).collect();
                // The most of the stake any one country's validators hold.
                let stake = nations::network::largest_country_share(&world, 0)
                    .map(|(_, share)| format!("{:.0}%", 100.0 * share))
                    .unwrap_or_default();
                // And of any one house; then the most of either at any height from the second,
                // the first whose signers a founding's mending can have changed.
                let held = |set: &chain::ValidatorSet| -> (f64, f64) {
                    let total = set.total_power().max(1) as f64;
                    let mut countries = vec![0u64; world.countries.len()];
                    let mut houses = std::collections::BTreeMap::new();
                    for v in &set.members {
                        if let Some(town) = network.town_of(&v.address) {
                            countries[world.towns[town].country] += v.power;
                            *houses.entry(town).or_insert(0u64) += v.power;
                        }
                    }
                    let most = |n: Option<u64>| n.unwrap_or(0) as f64 / total;
                    (most(countries.into_iter().max()), most(houses.into_values().max()))
                };
                let house = format!("{:.0}%", 100.0 * held(&network.chain.rotation).1);
                let ever = (2..=network.chain.height())
                    .filter_map(|h| network.chain.light_block(h))
                    .map(|b| held(&b.validators))
                    .fold((0.0f64, 0.0f64), |a, b| (a.0.max(b.0), a.1.max(b.1)));
                let ever = format!("{:.0}/{:.0}", 100.0 * ever.0, 100.0 * ever.1);
                let checking = Instant::now();
                let verdict = match network.chain.verify() {
                    Ok(()) => format!("ok in {:.1}s", checking.elapsed().as_secs_f64()),
                    Err((height, why)) => format!("FAILED at {height}: {why:?}"),
                };
                let jailed = world
                    .history
                    .iter()
                    .filter(|e| matches!(e, Event::Slashed { .. }))
                    .count();
                println!(
                    "{:>9} {:>5} {:>4} {:>6} {:>5} {:>7} {:>13} {:>5} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7} {:>5.0}% {:>7} {:>6} {:>6}  {verdict} ({ran:.1}s to run)",
                    format!("{seed:#x}"),
                    world.towns.len(),
                    world.countries.len(),
                    money,
                    trap,
                    network.founded,
                    format!("{} in {}", network.founders.len(), countries.len()),
                    network.validators().len(),
                    network.chain.height(),
                    stake,
                    house,
                    ever,
                    houses,
                    now,
                    100.0 * share,
                    network.refusals.values().sum::<u64>(),
                    network.stalls,
                    jailed,
                );
            }
            None => println!(
                "{:>9} {:>5} {:>4} {:>6} {:>5} {:>7} {:>13} {:>5} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7} {:>6} {:>7} {:>6} {:>6}  no chain: {:?} ({ran:.1}s to run)",
                format!("{seed:#x}"),
                world.towns.len(),
                world.countries.len(),
                money,
                trap,
                "—",
                "—",
                "—",
                "—",
                "—",
                "—",
                "—",
                houses,
                now,
                "—",
                "—",
                "—",
                "—",
                world.not_yet,
            ),
        }
    }
}
