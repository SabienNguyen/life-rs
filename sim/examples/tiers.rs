//! Does a thin detail budget starve people a full one would not?
//!
//! §21.1's cardinal sin, measured over seeds rather than one. A single pair of worlds
//! diverges in its families and its migrations within a decade, so one comparison measures
//! that divergence as much as the tiers.

use sim::World;
use sim_core::{Duration, WorldSeed};

fn main() {
    let (mut thin_total, mut ample_total) = (0usize, 0usize);
    for seed in [0x211u128, 0x11, 0x21, 0x31, 0x41, 0x51] {
        let toll = |budget: usize| {
            let mut world = World::genesis(WorldSeed::from_u128(seed), 40);
            world.set_detail_budget(budget);
            world.run_for(Duration::from_years(40));
            let starved = world
                .people
                .iter()
                .filter(|(_, p)| {
                    matches!(p.death(), Some((_, person::Cause::Deprivation)))
                })
                .count();
            (world.living(), starved)
        };
        let ((tl, ts), (al, asv)) = (toll(12), toll(4_000));
        println!("{seed:>4x}: thin {tl:>4} living {ts:>2} starved   ample {al:>4} living {asv:>2} starved");
        thin_total += ts;
        ample_total += asv;
    }
    println!("\nthin {thin_total} starved, ample {ample_total} starved across six seeds");
}
