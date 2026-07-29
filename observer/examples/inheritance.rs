//! How much of a life is handed down, and by which channel.
//!
//! §15's intergenerational elasticity runs 0.55 against a ceiling of 0.50, and §26.10 defers
//! property that outlives its members on exactly that: adding inherited wealth to a world whose
//! IGE is already too high makes it worse. Real societies *have* inherited property and manage
//! 0.3–0.5, so this world being too high *without* it says the channels it already has are too
//! strong — and the honest way to build the missing one is to find which of the existing ones
//! is doing more than its share, not to add a field and hope.
//!
//! This is that question, cheap enough to ablate against: one world, one minute, and the four
//! numbers §15 cares about. `balance_tests` is where the claims live; this is for finding out
//! what a change did.

use observer::balance::{measure, targets};
use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x11);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(160);
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(years));
    let balance = measure(&world);

    let shares = balance.shares.expect("a world this size has shares");
    let band = |value: f32, target: &std::ops::RangeInclusive<f32>| {
        if target.contains(&value) { "  " } else { "<<" }
    };
    let genes = shares.genes + shares.entangled;
    let environment = shares.environment + shares.entangled;
    let elasticity = balance.elasticity.unwrap_or(f32::NAN);
    let siblings = balance.sibling_correlation.unwrap_or(f32::NAN);

    println!("seed {seed:x}, {founders} founders, {years} years, {} lives", balance.sample);
    println!("  genes       {genes:.3} {}", band(genes, &targets::GENES));
    println!("  upbringing  {environment:.3} {}", band(environment, &targets::ENVIRONMENT));
    println!("  luck        {:.3} {}", shares.luck, band(shares.luck, &targets::LUCK));
    println!(
        "  elasticity  {elasticity:.3} {}   <- what a child gets from their parents",
        band(elasticity, &targets::ELASTICITY)
    );
    println!(
        "  siblings    {siblings:.3} {}",
        band(siblings, &targets::SIBLING_CORRELATION)
    );
    println!(
        "  mobility    {:.3} {}",
        balance.mobility.unwrap_or(f32::NAN),
        band(balance.mobility.unwrap_or(f32::NAN), &targets::MOBILITY)
    );
    println!("\n  '<<' marks a number outside the band §15 commits to.");
}
