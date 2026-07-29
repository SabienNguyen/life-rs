//! Does piling into one quarter cost anything the model computes?
//!
//! §30.5 concludes that nothing local is scarce, so sorting has nothing to balance it. That
//! was reasoned from `build_for`'s comment rather than measured. The economy has a
//! diminishing return in it — `work::make` is Cobb–Douglas, so land per hand falls as hands
//! are added — and if that reaches `Place::prosperity` then a crowded quarter is poorer per
//! head and there *is* a cost, just not one `appeal` consults.
//!
//! This prints households against output per head, year by year, for the quarter that wins.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x221);
    let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);

    let places: Vec<_> = world.places.ids().collect();
    println!(
        "year | {}",
        places
            .iter()
            .filter_map(|id| world.places.get(*id))
            .map(|p| format!("{:>20}", p.name))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for year in 0..90 {
        world.run_for(Duration::from_years(1));
        if year % 6 != 0 {
            continue;
        }
        let row: Vec<String> = places
            .iter()
            .map(|id| {
                let households = world.society.households_in(*id).count();
                match world.places.get(*id) {
                    // Households, what a head gets out of the ground, and what §14 makes of
                    // the people in it. The first two are the economy; the third is the
                    // positive feedback that has nothing pulling against it.
                    Some(p) => format!(
                        "{households:>3}h {:>5.3}pros {:>5.3}aff",
                        p.prosperity, p.env.affluence
                    ),
                    None => format!("{:>20}", "—"),
                }
            })
            .collect();
        println!("{:>4} | {}", year, row.join(" "));
    }
}
