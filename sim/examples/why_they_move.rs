//! Whether households that leave somewhere stay left, and where they all end up.
//!
//! Migration and churn are the same thing to any aggregate — a hundred moves each way is the
//! same net flow as none — so this follows individual households through the years instead,
//! and counts how often one goes back where it was two moves ago. It also prints how
//! concentrated the world ends up and how far its quarters diverge, because those are the two
//! things a fix for churn is most likely to break. §30 has what it found.
//!
//!     cargo run --release --example why_they_move

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    println!(
        "{:>6} {:>8} {:>8} {:>8} {:>7} {:>7}",
        "seed", "biggest", "empty", "spread", "moves", "back"
    );
    let (mut biggest, mut empty, mut spread, mut moves, mut back) = (0.0, 0.0, 0.0, 0usize, 0usize);
    let seeds = [0x11u128, 0x21, 0x31, 0x221];
    for seed in seeds {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        // Year by year, so households can be followed. A person's own record of moves is
        // not the same question: people join and leave households when they pair off and
        // when they grow up, so somebody can go A, B, A without any household ever having
        // gone back — that is a marriage, not churn. The households are what decide.
        let mut where_they_were: std::collections::BTreeMap<u64, Vec<u64>> = Default::default();
        for _ in 0..years {
            world.run_for(Duration::from_years(1));
            for (id, household) in world.society.households() {
                if let Some(place) = household.place {
                    let path = where_they_were.entry(id.to_bits()).or_default();
                    if path.last() != Some(&place.to_bits()) {
                        path.push(place.to_bits());
                    }
                }
            }
        }
        let (mut hm, mut hb) = (0usize, 0usize);
        for steps in where_they_were.values() {
            hm += steps.len().saturating_sub(1);
            hb += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }

        let counts: Vec<usize> = world
            .places
            .ids()
            .map(|id| world.society.households_in(id).count())
            .collect();
        let total: usize = counts.iter().sum();
        // How concentrated: the largest quarter's share, and how many stand empty.
        let top = *counts.iter().max().unwrap_or(&0) as f32 / total.max(1) as f32;
        let bare = counts.iter().filter(|c| **c == 0).count() as f32 / counts.len().max(1) as f32;

        // How sorted: the spread of what the inhabited quarters are worth. Zero means
        // crowding has flattened the world, which is a failure in the other direction.
        let lived_in: Vec<f32> = world
            .places
            .ids()
            .filter(|id| world.society.households_in(*id).count() > 0)
            .filter_map(|id| world.places.get(id).map(|p| p.env.affluence))
            .collect();
        let mean = lived_in.iter().sum::<f32>() / lived_in.len().max(1) as f32;
        let sd = (lived_in.iter().map(|a| (a - mean).powi(2)).sum::<f32>()
            / lived_in.len().max(1) as f32)
            .sqrt();

        let mut path: std::collections::BTreeMap<u64, Vec<u64>> = Default::default();
        for record in world.chronicle.iter() {
            if let sim::Happening::PersonMoves { person, to } = record.kind {
                path.entry(person.to_bits()).or_default().push(to.to_bits());
            }
        }
        let (mut m, mut b) = (0usize, 0usize);
        for steps in path.values() {
            m += steps.len();
            b += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }

        println!(
            "{seed:>6x} {top:>8.2} {bare:>8.2} {sd:>8.3} {m:>7} {b:>7}   households {hm} moves {hb} back"
        );
        biggest += top;
        empty += bare;
        spread += sd;
        moves += m;
        back += b;
    }
    let n = seeds.len() as f32;
    println!(
        "\nbiggest {:.2}  empty {:.2}  spread {:.3}  {moves} moves, {:.0}% straight back",
        biggest / n,
        empty / n,
        spread / n,
        100.0 * back as f32 / moves.max(1) as f32
    );
}
