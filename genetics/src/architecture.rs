//! Which loci build which traits, and how a genome becomes a phenotype.
//!
//! Two properties fall out of this shape, and they are most of the reason to model
//! genetics at all:
//!
//! - **Pleiotropy.** Loci are shared between traits, so trait correlations emerge from
//!   the architecture instead of being hand-tuned onto uncorrelated draws.
//! - **Regression to the mean.** Parents pass on half their alleles, not their
//!   phenotype, so two exceptional parents mostly produce a less exceptional child —
//!   by construction rather than by a fudge factor.

use crate::genome::{Genome, N_LOCI};
use sim_core::Rng;

/// The heritable traits. Personality is five of them; the rest are physical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Trait {
    Openness,
    Conscientiousness,
    Extraversion,
    Agreeableness,
    Neuroticism,
    Stature,
    Constitution,
}

impl Trait {
    pub const ALL: [Trait; 7] = [
        Trait::Openness,
        Trait::Conscientiousness,
        Trait::Extraversion,
        Trait::Agreeableness,
        Trait::Neuroticism,
        Trait::Stature,
        Trait::Constitution,
    ];

    pub const COUNT: usize = Trait::ALL.len();

    pub const fn label(self) -> &'static str {
        match self {
            Trait::Openness => "openness",
            Trait::Conscientiousness => "conscientiousness",
            Trait::Extraversion => "extraversion",
            Trait::Agreeableness => "agreeableness",
            Trait::Neuroticism => "neuroticism",
            Trait::Stature => "stature",
            Trait::Constitution => "constitution",
        }
    }

    /// Whether this trait is behavioural. The distinction is load-bearing: founder
    /// populations may differ at physical loci and must not differ at behavioural ones.
    pub const fn is_behavioural(self) -> bool {
        !matches!(self, Trait::Stature | Trait::Constitution)
    }

    /// Share of variance from genes, and from the shared environment of a household.
    ///
    /// These two numbers decide whether this simulation is a story about inheritance or
    /// about circumstance, so they are stated rather than emergent. The defaults sit
    /// near the behaviour-genetics consensus and deliberately leave a large remainder
    /// for the idiosyncratic — a particular teacher, an illness, a chance meeting.
    pub const fn variance(self) -> (f32, f32) {
        match self {
            Trait::Stature => (0.80, 0.10),
            Trait::Constitution => (0.50, 0.20),
            // The five personality factors.
            _ => (0.40, 0.20),
        }
    }
}

/// How many loci contribute to one trait. Enough that the sum goes normal.
const LOCI_PER_TRAIT: usize = 48;

/// How far apart consecutive traits' locus windows start. Smaller than
/// `LOCI_PER_TRAIT`, so windows overlap and traits share loci.
const WINDOW_STRIDE: usize = 30;

/// Dominance deviation, as a fraction of a locus's additive weight. Small, so
/// inheritance is mostly additive — but non-zero, which is what lets a trait skip a
/// generation and resurface in a grandchild.
const DOMINANCE_FRACTION: f32 = 0.30;

/// The architecture is fixed rather than per-world: it is the species' genetic
/// structure, not a property of one planet. Derived from a constant so it is identical
/// on every machine and in every run.
const ARCHITECTURE_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Copy, Debug)]
struct LocusEffect {
    locus: u16,
    additive: f32,
    dominance: f32,
}

#[derive(Clone, Debug)]
struct TraitSpec {
    effects: Vec<LocusEffect>,
}

/// Which loci build which traits.
#[derive(Clone, Debug)]
pub struct Architecture {
    specs: Vec<TraitSpec>,
}

impl Architecture {
    /// The standard human architecture.
    pub fn standard() -> Architecture {
        let mut rng = Rng::from_u64(ARCHITECTURE_SEED);
        let specs = (0..Trait::COUNT)
            .map(|index| {
                let start = index * WINDOW_STRIDE;
                let mut effects: Vec<LocusEffect> = (0..LOCI_PER_TRAIT)
                    .map(|offset| {
                        // Signs are drawn per (trait, locus), so a shared locus can pull
                        // two traits the same way or opposite ways.
                        let sign = if rng.coin() { 1.0 } else { -1.0 };
                        LocusEffect {
                            locus: ((start + offset) % N_LOCI) as u16,
                            additive: sign,
                            dominance: sign * DOMINANCE_FRACTION * rng.unit_f32(),
                        }
                    })
                    .collect();

                // Scale so the genetic value has unit variance under Hardy-Weinberg at
                // frequency 0.5: per locus, Var = 0.5*a^2 for dosage and 0.25*d^2 for
                // the heterozygote term.
                let raw: f32 = effects
                    .iter()
                    .map(|e| 0.5 * e.additive * e.additive + 0.25 * e.dominance * e.dominance)
                    .sum();
                let scale = 1.0 / raw.sqrt();
                for effect in &mut effects {
                    effect.additive *= scale;
                    effect.dominance *= scale;
                }
                TraitSpec { effects }
            })
            .collect();

        Architecture { specs }
    }

    /// The genetic value of a trait, as a z-score.
    pub fn genetic_value(&self, genome: &Genome, of: Trait) -> f32 {
        self.specs[of as usize]
            .effects
            .iter()
            .map(|effect| {
                let dosage = genome.dosage(effect.locus as usize);
                // Additive part is centred on one copy; the dominance deviation applies
                // only to heterozygotes, which is what makes some alleles recessive.
                let additive = effect.additive * (f32::from(dosage) - 1.0);
                // Centred on the heterozygote rate rather than on zero. A raw
                // "+d if heterozygous" term has a non-zero mean, and with fixed
                // per-locus signs those means do not cancel — the whole trait ends up
                // offset from the population average by a tenth of a deviation or more.
                let heterozygous = if dosage == 1 { 0.5 } else { -0.5 };
                additive + effect.dominance * heterozygous
            })
            .sum()
    }

    /// Combine genes, shared upbringing, and everything else into a phenotype.
    ///
    /// `shared` is the household's contribution and `unique` is the idiosyncratic
    /// remainder; both are z-scores. Weighting by the square roots of the variance
    /// shares is what makes the resulting trait a z-score too.
    pub fn express(&self, genome: &Genome, of: Trait, shared: f32, unique: f32) -> Expression {
        let (h2, c2) = of.variance();
        let e2 = 1.0 - h2 - c2;
        let genetic = self.genetic_value(genome, of);

        Expression {
            genetic: h2.sqrt() * genetic,
            shared: c2.sqrt() * shared,
            unique: e2.max(0.0).sqrt() * unique,
        }
    }

    /// Whether any behavioural trait draws on this locus.
    ///
    /// A locus in both a behavioural and a physical trait counts as behavioural: the
    /// guardrail in [`FounderPool`](crate::pool::FounderPool) is a constraint to be
    /// satisfied, so overlaps resolve toward the stricter reading.
    pub fn is_behavioural_locus(&self, locus: usize) -> bool {
        Trait::ALL
            .into_iter()
            .filter(|t| t.is_behavioural())
            .any(|t| {
                self.specs[t as usize]
                    .effects
                    .iter()
                    .any(|e| e.locus as usize == locus)
            })
    }

    /// How many loci two traits have in common — the source of their correlation.
    pub fn shared_loci(&self, a: Trait, b: Trait) -> usize {
        let of = |t: Trait| {
            self.specs[t as usize]
                .effects
                .iter()
                .map(|e| e.locus)
                .collect::<std::collections::HashSet<_>>()
        };
        of(a).intersection(&of(b)).count()
    }
}

impl Default for Architecture {
    fn default() -> Self {
        Architecture::standard()
    }
}

/// A trait, decomposed into where it came from.
///
/// Kept separated rather than summed, so a dossier can say *why* someone is the way
/// they are — and so the counterfactual "what if they had grown up elsewhere" is a
/// substitution rather than a re-simulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expression {
    pub genetic: f32,
    pub shared: f32,
    pub unique: f32,
}

impl Expression {
    pub fn total(&self) -> f32 {
        self.genetic + self.shared + self.unique
    }

    /// The same person raised somewhere else.
    pub fn if_raised(&self, elsewhere: f32, of: Trait) -> f32 {
        let (_, c2) = of.variance();
        self.genetic + c2.sqrt() * elsewhere + self.unique
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::conceive;
    use sim_core::{Domain, WorldSeed};

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0xa11e1e).stream(Domain::Genetics, n, 0)
    }

    fn founder(n: u64) -> Genome {
        Genome::founder(&mut rng(n), 0.5)
    }

    fn population(count: u64) -> Vec<Genome> {
        (0..count).map(founder).collect()
    }

    fn mean_and_variance(values: &[f32]) -> (f32, f32) {
        let n = values.len() as f32;
        let mean = values.iter().sum::<f32>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        (mean, var)
    }

    fn correlation(xs: &[f32], ys: &[f32]) -> f32 {
        let (mx, vx) = mean_and_variance(xs);
        let (my, vy) = mean_and_variance(ys);
        let cov = xs
            .iter()
            .zip(ys)
            .map(|(x, y)| (x - mx) * (y - my))
            .sum::<f32>()
            / xs.len() as f32;
        cov / (vx.sqrt() * vy.sqrt())
    }

    #[test]
    fn the_architecture_is_the_same_everywhere() {
        let a = Architecture::standard();
        let b = Architecture::standard();
        let genome = founder(1);
        for t in Trait::ALL {
            assert_eq!(a.genetic_value(&genome, t), b.genetic_value(&genome, t));
        }
    }

    #[test]
    fn genetic_values_are_standardised() {
        let arch = Architecture::standard();
        let genomes = population(3_000);
        for t in Trait::ALL {
            let values: Vec<f32> = genomes.iter().map(|g| arch.genetic_value(g, t)).collect();
            let (mean, var) = mean_and_variance(&values);
            assert!(mean.abs() < 0.12, "{t:?} mean {mean}");
            assert!((var - 1.0).abs() < 0.15, "{t:?} variance {var}");
        }
    }

    #[test]
    fn traits_share_loci_and_so_correlate() {
        let arch = Architecture::standard();
        // Adjacent windows overlap by construction.
        let overlap = arch.shared_loci(Trait::Openness, Trait::Conscientiousness);
        assert_eq!(overlap, LOCI_PER_TRAIT - WINDOW_STRIDE);

        // Distant windows do not.
        assert_eq!(arch.shared_loci(Trait::Openness, Trait::Neuroticism), 0);

        // And sharing loci produces a real, modest correlation rather than none.
        let genomes = population(3_000);
        let a: Vec<f32> = genomes
            .iter()
            .map(|g| arch.genetic_value(g, Trait::Openness))
            .collect();
        let b: Vec<f32> = genomes
            .iter()
            .map(|g| arch.genetic_value(g, Trait::Conscientiousness))
            .collect();
        let c: Vec<f32> = genomes
            .iter()
            .map(|g| arch.genetic_value(g, Trait::Neuroticism))
            .collect();

        assert!(
            correlation(&a, &b).abs() > 0.03,
            "overlapping traits should correlate: {}",
            correlation(&a, &b)
        );
        assert!(
            correlation(&a, &c).abs() < 0.06,
            "non-overlapping traits should not: {}",
            correlation(&a, &c)
        );
    }

    #[test]
    fn children_resemble_their_parents() {
        let arch = Architecture::standard();
        let mut midparents = Vec::new();
        let mut children = Vec::new();

        for i in 0..1_500u64 {
            let (mother, father) = (founder(i * 2), founder(i * 2 + 1));
            let child = conceive(&mother, &father, i);
            let t = Trait::Openness;
            midparents
                .push((arch.genetic_value(&mother, t) + arch.genetic_value(&father, t)) / 2.0);
            children.push(arch.genetic_value(&child, t));
        }

        let r = correlation(&midparents, &children);
        // Midparent-offspring correlation is ~sqrt(1/2) for a purely additive trait.
        assert!((0.55..0.85).contains(&r), "midparent correlation {r}");
    }

    #[test]
    fn exceptional_parents_regress_toward_the_mean() {
        // Selection has to be on *phenotype*, not on genetic value. Additive breeding
        // values do not regress at all — a child's expected breeding value is exactly
        // the midparent's. Regression appears because an outlying phenotype is part
        // luck, and luck is not inherited; the offspring keep the genes and lose the
        // rest, which is why the recovered slope is the heritability.
        let arch = Architecture::standard();
        let t = Trait::Extraversion;
        let (h2, _) = t.variance();

        let phenotype = |g: &Genome, seed: u64| {
            let mut r = rng(seed + 500_000);
            arch.express(g, t, r.normal() as f32, r.normal() as f32)
                .total()
        };

        let mut midparents = Vec::new();
        let mut children = Vec::new();
        let mut family = 0u64;

        while children.len() < 400 && family < 80_000 {
            let (mother, father) = (founder(family * 2), founder(family * 2 + 1));
            let midparent =
                (phenotype(&mother, family * 2) + phenotype(&father, family * 2 + 1)) / 2.0;
            family += 1;
            if midparent > 1.0 {
                let child = conceive(&mother, &father, family);
                midparents.push(midparent);
                children.push(phenotype(&child, family + 300_000));
            }
        }

        let parent_mean = midparents.iter().sum::<f32>() / midparents.len() as f32;
        let child_mean = children.iter().sum::<f32>() / children.len() as f32;

        assert!(
            child_mean > 0.15,
            "children of exceptional parents should stay above average: {child_mean:.3}"
        );
        assert!(
            child_mean < parent_mean * 0.8,
            "but should fall well back toward it: {child_mean:.3} vs {parent_mean:.3}"
        );
        // The slope should land near the heritability rather than anywhere below it.
        let slope = child_mean / parent_mean;
        assert!(
            (h2 - 0.25..h2 + 0.25).contains(&slope),
            "regression slope {slope:.2} should be near h2 = {h2}"
        );
    }

    #[test]
    fn siblings_resemble_each_other_more_than_strangers() {
        let arch = Architecture::standard();
        let t = Trait::Agreeableness;
        let (mut first, mut second, mut unrelated) = (Vec::new(), Vec::new(), Vec::new());

        for i in 0..1_200u64 {
            let (mother, father) = (founder(i * 3), founder(i * 3 + 1));
            first.push(arch.genetic_value(&conceive(&mother, &father, i * 2), t));
            second.push(arch.genetic_value(&conceive(&mother, &father, i * 2 + 1), t));
            unrelated.push(arch.genetic_value(&founder(i * 3 + 2), t));
        }

        let sibling_r = correlation(&first, &second);
        let stranger_r = correlation(&first, &unrelated);
        // Full siblings share half their additive variance.
        assert!(
            (0.35..0.65).contains(&sibling_r),
            "sibling correlation {sibling_r}"
        );
        assert!(stranger_r.abs() < 0.1, "stranger correlation {stranger_r}");
    }

    #[test]
    fn variance_shares_sum_to_one_and_leave_room_for_luck() {
        for t in Trait::ALL {
            let (h2, c2) = t.variance();
            assert!(h2 > 0.0 && c2 >= 0.0);
            assert!(h2 + c2 < 1.0, "{t:?} leaves nothing to chance");
        }
    }

    #[test]
    fn expression_weights_each_source_and_stays_standardised() {
        let arch = Architecture::standard();
        let genomes = population(2_000);
        let t = Trait::Neuroticism;

        let totals: Vec<f32> = genomes
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let mut r = rng(i as u64 + 90_000);
                arch.express(g, t, r.normal() as f32, r.normal() as f32)
                    .total()
            })
            .collect();

        let (mean, var) = mean_and_variance(&totals);
        assert!(mean.abs() < 0.12, "mean {mean}");
        assert!(
            (var - 1.0).abs() < 0.2,
            "variance {var} should still be about one"
        );
    }

    #[test]
    fn a_trait_can_be_traced_to_its_causes() {
        let arch = Architecture::standard();
        let expression = arch.express(&founder(1), Trait::Openness, 1.0, -0.5);
        let sum = expression.genetic + expression.shared + expression.unique;
        assert!((expression.total() - sum).abs() < 1e-6);

        // The counterfactual: same person, a worse upbringing.
        let elsewhere = expression.if_raised(-1.0, Trait::Openness);
        assert!(elsewhere < expression.total(), "upbringing should matter");
        // But not infinitely: genes and luck are untouched.
        assert!(
            (elsewhere - expression.total()).abs() < 1.0,
            "and should not be destiny"
        );
    }

    #[test]
    fn behavioural_traits_are_flagged_apart_from_physical_ones() {
        assert!(Trait::Openness.is_behavioural());
        assert!(Trait::Neuroticism.is_behavioural());
        assert!(!Trait::Stature.is_behavioural());
        assert!(!Trait::Constitution.is_behavioural());
    }
}
