//! Everything a change to this world can quietly break, in one run.
//!
//! Three mechanisms were built and reverted in one night — §26.11, §27.10, and a household
//! head — and every one of them was caught by the same test, eight minutes into a full suite,
//! after the change had already been committed. Each time the question was the same: *what did
//! that do to the world?* and answering it meant remembering which of six scattered examples
//! and two test modules to run.
//!
//! This is that question, asked once. It is not a test and asserts nothing — the suite is
//! where claims live. It is for the minute after a change, before deciding whether the change
//! is worth measuring properly.
//!
//!     cargo run --release --example vitals
//!
//! Where the world stands, measured rather than remembered — three seeds, 120 founders, 90
//! years, which is what the defaults produce:
//!
//!     living      667
//!     churn         9%   82 of 929 moves went straight back. Over 10% is pathological (§30.4)
//!     biggest    0.55    share of households in one quarter. 1.00 is the collapse (§30.5)
//!     empty      0.33    quarters with nobody in them
//!     spread     0.11    how far apart the inhabited quarters are. §14.4 needs this above 0
//!     short      0.00    the hungriest quarter's shortfall. Should be small; zero at this
//!                        size is expected, since §21's ceiling wants a crowded world
//!     trades           farm 318  hew 13  smith 6  cook 48  keep 19 — thin but not empty
//!
//! Seventy-eight seconds, against eight minutes for the suite that would otherwise tell you.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    let (mut moves, mut back) = (0usize, 0usize);
    let (mut biggest, mut empty, mut spread, mut short) = (0.0, 0.0, 0.0, 0.0);
    let mut trades = [0usize; 5];
    let mut living = 0;

    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));
        living += world.living();

        // Churn: households going back where they were two moves ago. The single most
        // sensitive number here — every mechanism reverted so far moved this one first.
        let mut path: std::collections::BTreeMap<u64, Vec<u64>> = Default::default();
        for record in world.chronicle.iter() {
            if let sim::Happening::PersonMoves { person, to } = record.kind {
                path.entry(person.to_bits()).or_default().push(to.to_bits());
            }
        }
        for steps in path.values() {
            moves += steps.len();
            back += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }

        // Where everybody ended up, and whether the quarters still differ.
        let counts: Vec<usize> = world
            .places
            .ids()
            .map(|id| world.society.households_in(id).count())
            .collect();
        let total: usize = counts.iter().sum();
        biggest += *counts.iter().max().unwrap_or(&0) as f32 / total.max(1) as f32;
        empty += counts.iter().filter(|c| **c == 0).count() as f32 / counts.len().max(1) as f32;

        let lived_in: Vec<f32> = world
            .places
            .ids()
            .filter(|id| world.society.households_in(*id).count() > 0)
            .filter_map(|id| world.places.get(id).map(|p| p.env.affluence))
            .collect();
        let mean = lived_in.iter().sum::<f32>() / lived_in.len().max(1) as f32;
        spread += (lived_in.iter().map(|a| (a - mean).powi(2)).sum::<f32>()
            / lived_in.len().max(1) as f32)
            .sqrt();

        short += world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).count() > 0)
            .map(|(_, p)| p.want)
            .fold(0.0_f32, f32::max);

        for (_, person) in world.people.iter() {
            if person.is_alive() && person.has_matured() {
                trades[person.trade() as usize] += 1;
            }
        }
    }

    let n = SEEDS.len() as f32;
    println!("{} seeds, {founders} founders, {years} years\n", SEEDS.len());
    println!("  living     {:>6}   across all three", living);
    println!(
        "  churn      {:>5.0}%   {back} of {moves} moves went straight back",
        100.0 * back as f32 / moves.max(1) as f32
    );
    println!("  biggest    {:>6.2}   share of households in one quarter", biggest / n);
    println!("  empty      {:>6.2}   quarters with nobody in them", empty / n);
    println!("  spread     {:>6.2}   how far apart the inhabited quarters are", spread / n);
    println!("  short      {:>6.2}   the hungriest quarter's shortfall per head", short / n);
    println!(
        "\n  trades     {}",
        ["farm", "hew", "smith", "cook", "keep"]
            .iter()
            .enumerate()
            .map(|(at, name)| format!("{name} {}", trades[at]))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!("\n  (§15's bands need `cargo test -p observer` — they cost six minutes.)");
}
