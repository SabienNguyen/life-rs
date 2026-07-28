//! Why no raid ever fires: the pairs the world actually offers.
//!
//! §32 says takings never happen and that the remaining gate is the numbers — that among the
//! adjacent cross-country pairs these worlds produce, the would-be raider is never the larger
//! side. That was inferred from the mechanism failing at a rate of 1.0, not measured. This
//! measures it.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let mut world = World::genesis(WorldSeed::from_u128(0x221), 600);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(
        std::env::var("YEARS").ok().and_then(|v| v.parse().ok()).unwrap_or(70),
    ));

    let countries = world.countries();
    println!("{} countries", countries.len());
    let (mut pairs, mut bigger, mut with_estate) = (0, 0, 0);
    for country in &countries {
        for mine in &country.places {
            for other in &countries {
                if other.name == country.name {
                    continue;
                }
                for theirs in &other.places {
                    if !world.within_reach(*mine, *theirs) {
                        continue;
                    }
                    let ours = world.souls_at(*mine).unwrap_or(0);
                    let them = world.souls_at(*theirs).unwrap_or(0);
                    if ours < 1 || them < 1 {
                        continue;
                    }
                    pairs += 1;
                    if ours > them {
                        bigger += 1;
                    }
                    let prize: f32 = world
                        .place_at(*theirs)
                        .map(|id| {
                            world
                                .society
                                .households_in(id)
                                .flat_map(|(_, h)| h.members.iter().copied())
                                .filter_map(|m| world.people.get(m))
                                .filter(|p| p.is_alive())
                                .map(|p| p.estate())
                                .sum()
                        })
                        .unwrap_or(0.0);
                    if prize > 0.0 {
                        with_estate += 1;
                    }
                    println!(
                        "  {} ({ours}) -> {} ({them}){}  prize {prize:.2}",
                        country.name,
                        other.name,
                        if ours > them { "  BIGGER" } else { "" }
                    );
                }
            }
        }
    }
    println!("\n{pairs} adjacent cross-country pairs, {bigger} where the raider is bigger, {with_estate} with anything to take");
}
