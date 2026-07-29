//! Where the slack is, and how much of it there is.
//!
//! §29 makes an advance need somebody with a year they did not spend staying alive, and
//! measures that as a place's `prosperity - want`. A world that is poor everywhere has none of
//! it; so, less obviously, does a world that is comfortable everywhere but only just. What
//! produces advances is surplus *and* heads to have ideas in, and those two can be in
//! different places.
//!
//! This prints, per place, how much spare there is and how many adults are standing in it.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x221);
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);

    let places: Vec<_> = world.places.ids().collect();
    println!("year | per place: adults × spare      | total thinking capacity");
    for year in 0..years {
        world.run_for(Duration::from_years(1));
        if year % 20 != 0 {
            continue;
        }
        let mut capacity = 0.0;
        let cells: Vec<String> = places
            .iter()
            .map(|id| {
                let adults = world
                    .society
                    .households_in(*id)
                    .flat_map(|(_, h)| h.members.iter())
                    .filter(|m| world.people.get(**m).is_some_and(|p| p.is_alive()))
                    .count();
                let spare = world
                    .places
                    .get(*id)
                    .map(|p| (p.prosperity - p.want).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                // The same product `work_things_out` effectively rolls: heads times how idle
                // each of them is. TIME_TO_THINK is 0.5, so spare above a half is full idle.
                capacity += adults as f32 * (spare / 0.5).min(1.0);
                format!("{adults:>4}×{spare:>5.2}")
            })
            .collect();
        println!("{:>4} | {} | {capacity:>8.0}", year, cells.join(" "));
    }
}
