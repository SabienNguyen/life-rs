//! The bands §15 commits to, checked over three worlds instead of one.
//!
//! `targets` says a measurement outside one of these *"is a finding, not necessarily a fault
//! — but it should be looked at rather than shrugged off"*, and only two of the seven were
//! ever looked at by a test. `neither_cause_decides_everything` checks the *claim* — that the
//! world is about both causes and that chance keeps a real share — and explains at length why
//! it deliberately avoids the calibration bands. Elasticity, sibling correlation, mobility and
//! the upbringing gap were computed, printed by `--balance`, and asserted nowhere.
//!
//! Over three seeds rather than one, and that is the point rather than a detail. Measured at
//! 160 founders and 120 years:
//!
//! | seed | genes | upbringing | luck | elasticity | siblings | mobility | gap |
//! |---|---|---|---|---|---|---|---|
//! | 0x11 | 0.50 | 0.42 | 0.40 | 0.69 | 0.47 | 0.69 | 1.13 |
//! | 0x21 | 0.45 | 0.23 | 0.51 | 0.56 | 0.36 | 0.70 | 0.72 |
//! | 0x221 | 0.33 | 0.20 | 0.59 | 0.41 | 0.19 | 0.61 | 0.61 |
//!
//! Four of the seven leave their band on *some* seed, and each of those four is comfortably
//! inside it on another. A world of a few hundred lives is a small sample and its statistics
//! wander; a single seed reading "within target" is close to no evidence, which is what the
//! `--balance` sheet has been showing all along. Averaged over the three, exactly two are out
//! — luck and intergenerational elasticity — which is the pair `neither_cause_decides_everything`
//! already names from its own measurements. Those two are quarantined below rather than
//! having their bands widened.
//!
//! **And it depends on the size of the world as much as on the seed.** The same three seeds at
//! 110 founders and 110 years, which is a third of the running time, give a different sheet:
//!
//! | seed | genes | upbringing | luck | elasticity | siblings | mobility | gap |
//! |---|---|---|---|---|---|---|---|
//! | 0x11 | 0.32 | 0.15 | 0.62 | 0.31 | 0.27 | 0.76 | 0.39 |
//! | 0x21 | 0.43 | 0.12 | 0.57 | 0.43 | 0.22 | 0.78 | 0.36 |
//! | 0x221 | 0.30 | 0.24 | 0.59 | 0.50 | 0.26 | 0.75 | 1.01 |
//!
//! The upbringing share averages 0.17 there, *below* its floor, and elasticity comes back
//! inside its band. Which makes sense — a world of a hundred and ten people for a hundred and
//! ten years has barely had time for its neighbourhoods to become different places, so there
//! is less shared environment to find, and less of it to be handed down. It also means §15's
//! validation is contingent on the fixture in a way nothing had said. The bigger world is the
//! one to check against, because the bands come from human populations that have had
//! somewhere to differ, but the cheaper sheet is kept here so the dependence is on the record
//! rather than being rediscovered. `print_the_sheet` prints either.

use crate::balance::{Balance, measure, targets};
use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

/// The worlds these are measured over.
///
/// Three by default, and `BALANCE_SEEDS=n` for more — which is not a convenience. The
/// shared-environment share read 0.253, 0.260, 0.240, 0.213, 0.197 and 0.170 across one
/// session's changes, and at least two of those moves had **no mechanism behind them**: a
/// de-duplication worth a hundredth of warmth moved it by seven hundredths, because it shifted
/// the trajectory and this is measured over three worlds (§38.2). `vitals` widened to eight
/// worlds for exactly this reason after `biggest` and `empty` swung twenty points on nineteen
/// robberies (§35.8); this is the same problem in the more expensive instrument.
///
/// Each world is a hundred and sixty founders for a hundred and twenty years and costs about
/// three minutes, which is why the default is what it is and why raising it is a decision
/// about the suite's runtime rather than a free improvement.
const ALL_SEEDS: [u128; 8] = [0x11, 0x21, 0x221, 0x31, 0x41, 0x5ee, 0x77, 0x8a];

fn seeds() -> &'static [u128] {
    static HOW_MANY: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| {
        std::env::var("BALANCE_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3)
            .clamp(1, ALL_SEEDS.len())
    });
    &ALL_SEEDS[..*HOW_MANY]
}

/// Built once and shared. Three worlds of a hundred and sixty founders run for a hundred and
/// twenty years is most of a minute each, and both tests below want the same three.
fn balances() -> &'static [(u128, Balance)] {
    static MEASURED: std::sync::LazyLock<Vec<(u128, Balance)>> = std::sync::LazyLock::new(|| {
        seeds()
            .iter()
            .copied()
            .map(|seed| {
                let mut world = World::genesis(WorldSeed::from_u128(seed), 160);
                world.record_only(Salience::Pivotal);
                world.set_detail_budget(100_000);
                // `ACTS=0` switches §35's vocabulary off, so that a band which moves after a
                // change to it can be attributed rather than argued about. Reading it here
                // costs nothing and is the only way this sheet can answer "was it that".
                world.acts_are_possible =
                    std::env::var("ACTS").map(|v| v != "0").unwrap_or(true);
                world.run_for(Duration::from_years(120));
                (seed, measure(&world))
            })
            .collect()
    });
    &MEASURED
}

fn mean(of: impl Iterator<Item = f32>) -> f32 {
    let values: Vec<f32> = of.collect();
    values.iter().sum::<f32>() / values.len().max(1) as f32
}

#[test]
#[ignore]
fn print_the_sheet() {
    for (seed, b) in balances() {
        let s = b.shares.unwrap();
        println!(
            "SHEET {seed:x}: n={} genes {:.2} env {:.2} luck {:.2} elas {:.2} sib {:.2} mob {:.2} gap {:.2}",
            b.sample,
            s.genes + s.entangled,
            s.environment + s.entangled,
            s.luck,
            b.elasticity.unwrap_or(f32::NAN),
            b.sibling_correlation.unwrap_or(f32::NAN),
            b.mobility.unwrap_or(f32::NAN),
            b.upbringing_gap.unwrap_or(f32::NAN),
        );
    }
}

#[test]
fn the_bands_the_design_meets_stay_met() {
    let measured = balances();
    for (seed, balance) in measured {
        assert!(
            balance.sample > 100,
            "seed {seed:x}: only {} lives to measure over",
            balance.sample
        );
    }
    let shares = |pick: fn(&crate::balance::Shares) -> f32| {
        mean(measured.iter().map(|(_, b)| pick(&b.shares.expect("shares"))))
    };
    // Entangled counted towards both causes, for the reason given where this is printed: the
    // twin and adoption studies the bands come from cannot separate it either, so comparing
    // only the separated part marks the model wrong for being more careful than the
    // measurement it is checked against.
    let genes = shares(|s| s.genes + s.entangled);
    let environment = shares(|s| s.environment + s.entangled);
    let elasticity = mean(measured.iter().filter_map(|(_, b)| b.elasticity));
    let siblings = mean(measured.iter().filter_map(|(_, b)| b.sibling_correlation));
    let mobility = mean(measured.iter().filter_map(|(_, b)| b.mobility));
    let gap = mean(measured.iter().filter_map(|(_, b)| b.upbringing_gap));

    for (name, value, band) in [
        ("genes", genes, targets::GENES),
        ("upbringing", environment, targets::ENVIRONMENT),
        ("siblings", siblings, targets::SIBLING_CORRELATION),
        ("mobility", mobility, targets::MOBILITY),
        ("upbringing gap", gap, targets::UPBRINGING_GAP),
    ] {
        assert!(
            band.contains(&value),
            "{name} averages {value:.2} across {} worlds, outside {band:?}",
            seeds().len()
        );
    }
    // Not asserted against its band, because it does not meet it — see below. Asserted to
    // exist, so that a change which stops producing parent-child pairs at all fails here
    // rather than silently reporting nothing.
    assert!(elasticity.is_finite(), "no intergenerational slope at all");
}

#[test]
fn the_two_known_excursions_do_not_get_worse() {
    // **Luck** runs above `targets::LUCK` — 0.50 averaged, against a ceiling of 0.45 — and
    // **elasticity** above `ELASTICITY`, 0.55 against 0.50. Both predate §30: seed 0x221 read
    // luck 0.53 before that work and 0.60 after.
    //
    // They are one finding from two sides. Luck is a residual — whatever neither the genome
    // nor the household explains — so everything the model does not yet contain lands in it,
    // and §19.1's list of approximations is long. Elasticity is high for the opposite reason:
    // what *is* modelled about inheritance is concentrated in a few strong channels, so the
    // parts of a life that are handed down are handed down harder than they should be. §21
    // records the tension that makes the two hard to fix together.
    //
    // The bounds here are what the two measure with room to move, so a change that makes
    // either worse fails. They are not targets and must not be read as any.
    let measured = balances();
    let luck = mean(
        measured
            .iter()
            .map(|(_, b)| b.shares.expect("shares").luck),
    );
    let elasticity = mean(measured.iter().filter_map(|(_, b)| b.elasticity));
    assert!(
        luck < 0.65,
        "luck averages {luck:.2}, further out than the recorded excursion"
    );
    assert!(
        elasticity < 0.75,
        "elasticity averages {elasticity:.2}, further out than the recorded excursion"
    );
}
