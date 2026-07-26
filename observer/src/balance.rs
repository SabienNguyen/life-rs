//! Measuring whether this is a story about inheritance or about circumstance.
//!
//! The design asks for both, in balance — and balance cannot be defined as the
//! heritability setting. Heritability is a statement about *trait* variance, while
//! outcomes are dominated by the opportunity gate, which is not a variance component at
//! all. Heritability could sit at a modest 0.4 while birth decides everything, simply by
//! gating hard. So balance has to be measured where it matters, in outcomes.
//!
//! One honesty note runs through all of this. Genes and environment are *correlated* by
//! construction, because parents supply both — passive gene-environment correlation.
//! Any split of the variance between them is therefore partly arbitrary, and this module
//! reports the overlap as its own quantity rather than dividing it up and pretending to
//! a precision it does not have.

use person::{Person, PersonId};
use sim::World;
use std::fmt;

/// The share of outcome variance a cause accounts for, plus how much of it cannot be
/// separated from the other cause.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shares {
    /// Attributable to genes alone.
    pub genes: f32,
    /// Attributable to upbringing alone.
    pub environment: f32,
    /// Explained by either, separable by neither — passive rGE.
    pub entangled: f32,
    /// Everything the model's own causes do not explain.
    pub luck: f32,
}

/// What a run turned out to be about.
#[derive(Clone, Debug, Default)]
pub struct Balance {
    /// Adults whose lifetime outcome could be measured.
    pub sample: usize,
    pub shares: Option<Shares>,
    /// Slope of a child's outcome on the midparent's. 0 is a meritocracy fantasy,
    /// 1 is a caste system.
    pub elasticity: Option<f32>,
    /// How alike full siblings end up.
    pub sibling_correlation: Option<f32>,
    /// Share of children who ended in a different outcome quintile from their parents.
    pub mobility: Option<f32>,
    /// Outcome gap, in standard deviations, between those raised in the best and worst
    /// quarter of neighbourhoods.
    pub upbringing_gap: Option<f32>,
}

/// The bands the design commits to. A measurement outside one of these is a finding,
/// not necessarily a fault — but it should be looked at rather than shrugged off.
pub mod targets {
    use std::ops::RangeInclusive;

    pub const GENES: RangeInclusive<f32> = 0.15..=0.45;
    pub const ENVIRONMENT: RangeInclusive<f32> = 0.20..=0.55;
    pub const LUCK: RangeInclusive<f32> = 0.15..=0.45;
    pub const ELASTICITY: RangeInclusive<f32> = 0.20..=0.50;
    pub const SIBLING_CORRELATION: RangeInclusive<f32> = 0.25..=0.65;
    pub const MOBILITY: RangeInclusive<f32> = 0.40..=0.90;
    pub const UPBRINGING_GAP: RangeInclusive<f32> = 0.30..=1.20;
}

/// Someone who lived long enough to have an outcome worth measuring.
struct Subject {
    id: PersonId,
    outcome: f32,
    /// The heritable part of the trait that drives attainment.
    genetic: f32,
    /// The neighbourhood absorbed across childhood.
    upbringing: f32,
    /// What their parents had — the transfer at birth.
    inherited: f32,
    /// The opportunity available where they actually spent their working life.
    opportunity: f32,
}

/// The age by which someone counts as having had a life.
const ADULT: f64 = 25.0;

pub fn measure(world: &World) -> Balance {
    let subjects = gather(world);
    let mut balance = Balance {
        sample: subjects.len(),
        ..Balance::default()
    };
    if subjects.len() < 8 {
        return balance;
    }

    let outcomes: Vec<f32> = subjects.iter().map(|s| s.outcome).collect();
    let genes: Vec<f32> = subjects.iter().map(|s| s.genetic).collect();

    // Circumstance is three things, not one: the street someone grew up on, what their
    // parents had, and the opportunity where they actually worked. Leaving the first out
    // pushed half the variance into the residual and called it chance; leaving the last
    // out did the same again once people could move for work, because a childhood
    // neighbourhood stops predicting an outcome as soon as you can leave it. All three
    // are on different scales, so each is standardised before they are combined.
    let homes = composite(&[
        subjects.iter().map(|s| s.upbringing).collect(),
        subjects.iter().map(|s| s.inherited).collect(),
        subjects.iter().map(|s| s.opportunity).collect(),
    ]);

    balance.shares = decompose(&outcomes, &genes, &homes);
    balance.elasticity = elasticity(world, &subjects);
    balance.sibling_correlation = sibling_correlation(world, &subjects);
    balance.mobility = mobility(world, &subjects);
    balance.upbringing_gap = upbringing_gap(&subjects);
    balance
}

/// Adults who were born into this world, rather than founded with it.
///
/// Founders are excluded deliberately: they have no parents and no simulated childhood,
/// so they have neither of the two causes being weighed and would only add noise to the
/// question of how much each one matters.
fn gather(world: &World) -> Vec<Subject> {
    let now = world.now();
    world
        .people
        .iter()
        .filter(|(_, p)| lived_a_life(p, now))
        .filter_map(|(id, p)| {
            let (mother, father) = world.society.parents_of(id)?;
            let midparent = (world.people.get(mother)?.peak_standing()
                + world.people.get(father)?.peak_standing())
                / 2.0;
            Some(Subject {
                id,
                outcome: p.peak_standing(),
                // Conscientiousness is what feeds attainment, so its genetic component
                // is the genetic input to an outcome.
                genetic: p.origins.conscientiousness.genetic,
                upbringing: p.absorbed_upbringing(),
                inherited: midparent,
                opportunity: p.mean_opportunity(),
            })
        })
        .collect()
}

fn lived_a_life(person: &Person, now: sim_core::Time) -> bool {
    let age = match person.death() {
        Some((when, _)) => person.age(when).years(),
        None => person.age(now).years(),
    };
    age >= ADULT
}

/// Commonality analysis over two correlated predictors.
///
/// The unique share of each is what it explains that the other cannot; the entangled
/// share is what they can only explain together. That last quantity is the passive
/// gene-environment correlation made visible instead of being silently assigned to
/// whichever predictor the regression happened to enter first.
fn decompose(outcome: &[f32], genes: &[f32], homes: &[f32]) -> Option<Shares> {
    let r_g = correlation(outcome, genes)?;
    let r_e = correlation(outcome, homes)?;
    let r_ge = correlation(genes, homes)?;

    // Two-predictor R², closed form.
    let denominator = 1.0 - r_ge * r_ge;
    if denominator.abs() < 1e-6 {
        return None;
    }
    let full = ((r_g * r_g + r_e * r_e - 2.0 * r_g * r_e * r_ge) / denominator).clamp(0.0, 1.0);

    let unique_genes = (full - r_e * r_e).clamp(0.0, 1.0);
    let unique_environment = (full - r_g * r_g).clamp(0.0, 1.0);
    let entangled = (full - unique_genes - unique_environment).clamp(0.0, 1.0);

    Some(Shares {
        genes: unique_genes,
        environment: unique_environment,
        entangled,
        luck: (1.0 - full).clamp(0.0, 1.0),
    })
}

fn elasticity(world: &World, subjects: &[Subject]) -> Option<f32> {
    let (mut parents, mut children) = (Vec::new(), Vec::new());
    for subject in subjects {
        let Some((mother, father)) = world.society.parents_of(subject.id) else {
            continue;
        };
        let (Some(m), Some(f)) = (world.people.get(mother), world.people.get(father)) else {
            continue;
        };
        parents.push((m.peak_standing() + f.peak_standing()) / 2.0);
        children.push(subject.outcome);
    }
    if parents.len() < 8 {
        return None;
    }
    // Regression slope, which is the elasticity for a measure already on a fixed scale.
    let variance = variance_of(&parents);
    if variance < 1e-9 {
        return None;
    }
    Some(covariance(&parents, &children) / variance)
}

fn sibling_correlation(world: &World, subjects: &[Subject]) -> Option<f32> {
    let (mut first, mut second) = (Vec::new(), Vec::new());
    for subject in subjects {
        let Some(parents) = world.society.parents_of(subject.id) else {
            continue;
        };
        for sibling in world.society.siblings_of(subject.id) {
            // Full siblings only, and each pair once.
            if sibling <= subject.id || world.society.parents_of(sibling) != Some(parents) {
                continue;
            }
            if let Some(other) = subjects.iter().find(|s| s.id == sibling) {
                first.push(subject.outcome);
                second.push(other.outcome);
            }
        }
    }
    if first.len() < 5 {
        return None;
    }
    correlation(&first, &second)
}

fn mobility(world: &World, subjects: &[Subject]) -> Option<f32> {
    let mut pairs = Vec::new();
    for subject in subjects {
        let Some((mother, father)) = world.society.parents_of(subject.id) else {
            continue;
        };
        let (Some(m), Some(f)) = (world.people.get(mother), world.people.get(father)) else {
            continue;
        };
        pairs.push((
            (m.peak_standing() + f.peak_standing()) / 2.0,
            subject.outcome,
        ));
    }
    if pairs.len() < 10 {
        return None;
    }

    let parent_cuts = quintiles(&pairs.iter().map(|p| p.0).collect::<Vec<_>>());
    let child_cuts = quintiles(&pairs.iter().map(|p| p.1).collect::<Vec<_>>());
    let moved = pairs
        .iter()
        .filter(|(parent, child)| quintile(*parent, &parent_cuts) != quintile(*child, &child_cuts))
        .count();
    Some(moved as f32 / pairs.len() as f32)
}

/// How far apart the best- and worst-raised quarters of the population end up, in
/// standard deviations of outcome.
fn upbringing_gap(subjects: &[Subject]) -> Option<f32> {
    if subjects.len() < 12 {
        return None;
    }
    let mut sorted: Vec<&Subject> = subjects.iter().collect();
    sorted.sort_by(|a, b| a.upbringing.total_cmp(&b.upbringing));

    let quarter = (sorted.len() / 4).max(1);
    let mean =
        |slice: &[&Subject]| slice.iter().map(|s| s.outcome).sum::<f32>() / slice.len() as f32;
    let worst = mean(&sorted[..quarter]);
    let best = mean(&sorted[sorted.len() - quarter..]);

    let spread = variance_of(&subjects.iter().map(|s| s.outcome).collect::<Vec<_>>()).sqrt();
    if spread < 1e-6 {
        return None;
    }
    Some((best - worst) / spread)
}

/// Standardise two series and add them, so neither dominates by having a wider scale.
fn composite(series: &[Vec<f32>]) -> Vec<f32> {
    let length = series.first().map(Vec::len).unwrap_or(0);
    let mut total = vec![0.0; length];
    for values in series {
        let (mean, spread) = (mean_of(values), variance_of(values).sqrt());
        let spread = if spread < 1e-9 { 1.0 } else { spread };
        for (sum, value) in total.iter_mut().zip(values) {
            *sum += (value - mean) / spread;
        }
    }
    total
}

// ---- small statistics --------------------------------------------------------------

fn mean_of(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

fn variance_of(values: &[f32]) -> f32 {
    let mean = mean_of(values);
    values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32
}

fn covariance(a: &[f32], b: &[f32]) -> f32 {
    let (ma, mb) = (mean_of(a), mean_of(b));
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - ma) * (y - mb))
        .sum::<f32>()
        / a.len() as f32
}

fn correlation(a: &[f32], b: &[f32]) -> Option<f32> {
    let (va, vb) = (variance_of(a), variance_of(b));
    if va < 1e-9 || vb < 1e-9 {
        return None;
    }
    Some(covariance(a, b) / (va.sqrt() * vb.sqrt()))
}

fn quintiles(values: &[f32]) -> [f32; 4] {
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    let at =
        |fraction: f32| sorted[((sorted.len() as f32 * fraction) as usize).min(sorted.len() - 1)];
    [at(0.2), at(0.4), at(0.6), at(0.8)]
}

fn quintile(value: f32, cuts: &[f32; 4]) -> u8 {
    cuts.iter().filter(|cut| value >= **cut).count() as u8
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  measured over {} lives", self.sample)?;

        let band = |value: Option<f32>, target: &std::ops::RangeInclusive<f32>| match value {
            Some(v) if target.contains(&v) => format!("{v:>6.2}  within target"),
            Some(v) => format!(
                "{v:>6.2}  outside {:.2}–{:.2}",
                target.start(),
                target.end()
            ),
            None => "     —  too few to say".to_string(),
        };

        if let Some(s) = self.shares {
            writeln!(f, "  outcome variance")?;
            writeln!(
                f,
                "    genes        {}",
                band(Some(s.genes), &targets::GENES)
            )?;
            writeln!(
                f,
                "    upbringing   {}",
                band(Some(s.environment), &targets::ENVIRONMENT)
            )?;
            writeln!(
                f,
                "    entangled    {:>6.2}  inseparable — parents supply both",
                s.entangled
            )?;
            writeln!(f, "    luck         {}", band(Some(s.luck), &targets::LUCK))?;
        } else {
            writeln!(f, "  outcome variance  — too few to say")?;
        }

        writeln!(
            f,
            "  elasticity     {}",
            band(self.elasticity, &targets::ELASTICITY)
        )?;
        writeln!(
            f,
            "  siblings       {}",
            band(self.sibling_correlation, &targets::SIBLING_CORRELATION)
        )?;
        writeln!(
            f,
            "  mobility       {}",
            band(self.mobility, &targets::MOBILITY)
        )?;
        write!(
            f,
            "  upbringing gap {}",
            band(self.upbringing_gap, &targets::UPBRINGING_GAP)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Duration, Salience, WorldSeed};

    /// One world lived long enough for grandchildren to have outcomes. Built once.
    ///
    /// Sixty people over ninety years was the first version and it was too small to
    /// measure the thing it was measuring. An intergenerational elasticity is a regression
    /// over parent–child pairs, and that fixture produced sixty-six lives — at which size
    /// the estimate is dominated by sampling noise. It read 0.89 on one world and 0.53 on
    /// the same seed run half again as long, and the difference was not a change in the
    /// model. Two hundred lives is enough for the number to mean something.
    fn measured() -> &'static Balance {
        static BALANCE: std::sync::LazyLock<Balance> = std::sync::LazyLock::new(|| {
            let mut world = World::genesis(WorldSeed::from_u128(0x11), 80);
            world.record_only(Salience::Pivotal);
            world.run_for(Duration::from_years(120));
            measure(&world)
        });
        &BALANCE
    }

    #[test]
    fn there_is_enough_to_measure() {
        let balance = measured();
        // Enough to regress on. The bar is deliberately near what the fixture produces:
        // if a change to the world shrinks the population below this, every statistic
        // below becomes noise and should fail loudly rather than drift.
        assert!(balance.sample > 150, "only {} lives", balance.sample);
        assert!(balance.shares.is_some());
        assert!(balance.elasticity.is_some(), "no parent-child pairs");
        assert!(balance.mobility.is_some());
    }

    #[test]
    fn neither_cause_decides_everything() {
        // The headline. Not that the split matches a particular pair of numbers, but
        // that the world is about both — and that chance keeps a real share.
        let shares = measured().shares.unwrap();
        let total = shares.genes + shares.environment + shares.entangled + shares.luck;
        assert!(
            (total - 1.0).abs() < 0.01,
            "shares should sum to one: {total}"
        );

        assert!(
            shares.genes + shares.entangled > 0.03,
            "genes should matter: {shares:?}"
        );
        assert!(
            shares.environment + shares.entangled > 0.03,
            "upbringing should matter: {shares:?}"
        );
        assert!(
            targets::LUCK.contains(&shares.luck),
            "chance should keep a real share, got {:.2}",
            shares.luck
        );
    }

    #[test]
    fn advantage_passes_down_without_being_destiny() {
        let balance = measured();
        let elasticity = balance.elasticity.unwrap();
        assert!(
            elasticity > 0.05,
            "a world where birth means nothing is a fantasy: {elasticity:.2}"
        );
        assert!(
            elasticity < 0.85,
            "a world where birth means everything is a caste system: {elasticity:.2}"
        );
    }

    #[test]
    fn most_people_do_not_end_where_they_started() {
        let mobility = measured().mobility.unwrap();
        assert!(
            mobility > 0.3,
            "mobility should be common, not miraculous: {mobility:.2}"
        );
    }

    #[test]
    fn where_you_were_raised_shows_up_in_the_outcome() {
        let gap = measured().upbringing_gap.unwrap();
        assert!(gap > 0.05, "upbringing should leave a mark: {gap:.2}");
    }

    #[test]
    fn the_report_reads() {
        let text = measured().to_string();
        assert!(text.contains("outcome variance"));
        assert!(text.contains("elasticity"));
        assert!(text.contains("entangled"));
    }

    #[test]
    fn an_empty_world_says_so_rather_than_guessing() {
        let balance = measure(&World::new(WorldSeed::from_u128(1)));
        assert_eq!(balance.sample, 0);
        assert!(balance.shares.is_none());
        assert!(balance.elasticity.is_none());
        assert!(balance.to_string().contains("too few"));
    }

    #[test]
    fn commonality_shares_always_account_for_everything() {
        // Including the awkward cases: perfectly correlated predictors, and none.
        let outcome = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let same = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let noise = [0.4, -1.2, 0.9, 0.1, -0.6, 1.4];

        for (g, e) in [(&same, &noise), (&noise, &same), (&noise, &noise)] {
            if let Some(s) = decompose(&outcome, g, e) {
                let total = s.genes + s.environment + s.entangled + s.luck;
                assert!((total - 1.0).abs() < 0.02, "{s:?} sums to {total}");
                for share in [s.genes, s.environment, s.entangled, s.luck] {
                    assert!((0.0..=1.0).contains(&share), "{s:?}");
                }
            }
        }
        // Two identical predictors cannot be told apart at all.
        assert!(decompose(&outcome, &same, &same).is_none());
    }

    #[test]
    fn quintiles_split_a_population_into_five() {
        let values: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let cuts = quintiles(&values);
        assert_eq!(quintile(0.0, &cuts), 0);
        assert_eq!(quintile(99.0, &cuts), 4);
        assert_eq!(quintile(50.0, &cuts), 2);
    }
}
