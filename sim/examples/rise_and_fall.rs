//! A country, from nothing to nothing.
//!
//! A country here is not drawn on a map and nobody declares one. It is the set of places whose
//! people can reach each other *and* share enough of their ways to count as the same — so it
//! can grow by settling a neighbour, split when the ways drift apart, and end by having nobody
//! left in it. This walks a world a decade at a time and prints what each country was, so the
//! rising and the falling can be read rather than asserted.
//!
//!     SEED=5ee YEARS=260 cargo run --release --example rise_and_fall

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x5ee);
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(260);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);

    println!("seed {seed:x}, {founders} founders\n");
    for decade in 0..(years / 10) {
        world.run_for(Duration::from_years(10));
        let year = (decade + 1) * 10;
        let countries = world.countries();
        let line: Vec<String> = countries
            .iter()
            .map(|country| {
                let souls: u32 = country
                    .places
                    .iter()
                    .filter_map(|at| world.souls_at(*at))
                    .sum();
                // What the ground gives a head, averaged over the places that have anybody in
                // them — a country nobody lives in has no answer rather than a zero.
                let lived_in: Vec<f32> = country
                    .places
                    .iter()
                    .filter(|at| world.souls_at(**at).unwrap_or(0) > 0)
                    .filter_map(|at| world.place_at(*at))
                    .filter_map(|id| world.places.get(id))
                    .map(|p| p.fortune)
                    .collect();
                let fortune = if lived_in.is_empty() {
                    f32::NAN
                } else {
                    lived_in.iter().sum::<f32>() / lived_in.len() as f32
                };
                let quarters: Vec<&str> = country
                    .places
                    .iter()
                    .filter(|at| world.souls_at(**at).unwrap_or(0) > 0)
                    .filter_map(|at| world.place_named(*at))
                    .collect();
                format!(
                    "{:<12} {souls:>5} souls  fortune {fortune:>5.2}  {}",
                    country.name,
                    quarters.join(", ")
                )
            })
            .collect();
        println!("year {year:>4}  ({} living)", world.living());
        for entry in line {
            println!("    {entry}");
        }
    }
}
