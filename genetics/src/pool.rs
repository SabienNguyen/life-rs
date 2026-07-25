//! Founder populations, and the one constraint placed on them.
//!
//! Founders draw from population-specific allele frequencies, which produces genuine
//! population structure: regional resemblance, inherited appearance, and an ancestry
//! that is *read off the genome* rather than stored as a label.
//!
//! **Founder pools differ only at physical loci. Behavioural loci draw from a single
//! shared pool with identical frequencies in every population.**
//!
//! Two reasons pointing the same way. It is what the science supports — between-group
//! variance in behavioural traits is not what the genetics shows. And the alternative
//! would hardcode a racial determinism into the engine, which would be both false and
//! repellent. Group differences in outcomes can still arise, through environment,
//! opportunity, and history; those are the interesting mechanisms anyway, because they
//! are the ones that can change.
//!
//! The constraint is enforced here in construction rather than left to callers, and
//! asserted in the tests, so it cannot quietly rot.

use crate::architecture::Architecture;
use crate::genome::{Genome, Haplotype, N_LOCI};
use sim_core::Rng;

/// The allele frequencies a founding population draws from.
#[derive(Clone, Debug, PartialEq)]
pub struct FounderPool {
    frequencies: Vec<f64>,
}

/// The frequency every behavioural locus takes, in every population, always.
pub const SHARED_BEHAVIOURAL_FREQUENCY: f64 = 0.5;

impl FounderPool {
    /// A population with its own physical variation.
    ///
    /// `divergence` is how far its physical allele frequencies may drift from even —
    /// 0 makes it indistinguishable from any other pool, 0.4 makes it visibly distinct.
    pub fn diverged(architecture: &Architecture, divergence: f64, rng: &mut Rng) -> FounderPool {
        let spread = divergence.clamp(0.0, 0.45);
        let frequencies = (0..N_LOCI)
            .map(|locus| {
                if architecture.is_behavioural_locus(locus) {
                    SHARED_BEHAVIOURAL_FREQUENCY
                } else {
                    rng.range_f64(0.5 - spread, 0.5 + spread)
                }
            })
            .collect();
        FounderPool { frequencies }
    }

    /// The undifferentiated pool: every locus even. What a single-population world uses.
    pub fn uniform() -> FounderPool {
        FounderPool {
            frequencies: vec![SHARED_BEHAVIOURAL_FREQUENCY; N_LOCI],
        }
    }

    pub fn frequency(&self, locus: usize) -> f64 {
        self.frequencies[locus]
    }

    /// Draw a founder's genome.
    pub fn draw(&self, rng: &mut Rng) -> Genome {
        Genome {
            maternal: self.haplotype(rng),
            paternal: self.haplotype(rng),
        }
    }

    fn haplotype(&self, rng: &mut Rng) -> Haplotype {
        let mut h = Haplotype::EMPTY;
        for (locus, frequency) in self.frequencies.iter().enumerate() {
            if rng.chance(*frequency) {
                h.set(locus, true);
            }
        }
        h
    }
}

impl Default for FounderPool {
    fn default() -> Self {
        FounderPool::uniform()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture::Trait;
    use sim_core::{Domain, WorldSeed};

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0xf0_01d).stream(Domain::Genetics, n, 0)
    }

    fn mean_trait(arch: &Architecture, pool: &FounderPool, of: Trait, n: u64) -> f32 {
        let total: f32 = (0..n)
            .map(|i| arch.genetic_value(&pool.draw(&mut rng(i)), of))
            .sum();
        total / n as f32
    }

    #[test]
    fn behavioural_loci_are_identical_across_pools() {
        let arch = Architecture::standard();
        let a = FounderPool::diverged(&arch, 0.4, &mut rng(1));
        let b = FounderPool::diverged(&arch, 0.4, &mut rng(2));

        for locus in 0..N_LOCI {
            if arch.is_behavioural_locus(locus) {
                assert_eq!(
                    a.frequency(locus),
                    b.frequency(locus),
                    "behavioural locus {locus} differs between populations"
                );
                assert_eq!(a.frequency(locus), SHARED_BEHAVIOURAL_FREQUENCY);
            }
        }
    }

    #[test]
    fn populations_can_differ_physically() {
        let arch = Architecture::standard();
        let a = FounderPool::diverged(&arch, 0.4, &mut rng(3));
        let b = FounderPool::diverged(&arch, 0.4, &mut rng(4));

        let physical_differences = (0..N_LOCI)
            .filter(|&l| !arch.is_behavioural_locus(l) && a.frequency(l) != b.frequency(l))
            .count();
        assert!(
            physical_differences > 10,
            "populations should have real physical structure, found {physical_differences}"
        );
    }

    #[test]
    fn no_behavioural_difference_between_populations() {
        // The guardrail, as an assertion rather than a comment. Two maximally diverged
        // populations must be statistically indistinguishable on every behavioural
        // trait; if this ever fails, the engine has started encoding something false.
        let arch = Architecture::standard();
        let a = FounderPool::diverged(&arch, 0.45, &mut rng(5));
        let b = FounderPool::diverged(&arch, 0.45, &mut rng(6));

        for t in Trait::ALL.into_iter().filter(|t| t.is_behavioural()) {
            let (ma, mb) = (
                mean_trait(&arch, &a, t, 1_500),
                mean_trait(&arch, &b, t, 1_500),
            );
            assert!(
                (ma - mb).abs() < 0.12,
                "{t:?} differs between populations: {ma:.3} vs {mb:.3}"
            );
        }
    }

    #[test]
    fn physical_traits_may_differ_between_populations() {
        // The other half: the guardrail must not be achieved by flattening everything.
        // Appearance and physiology are exactly what population structure is for.
        let arch = Architecture::standard();
        let mut biggest = 0.0f32;
        for seed in 0..8 {
            let a = FounderPool::diverged(&arch, 0.45, &mut rng(100 + seed * 2));
            let b = FounderPool::diverged(&arch, 0.45, &mut rng(101 + seed * 2));
            for t in Trait::ALL.into_iter().filter(|t| !t.is_behavioural()) {
                let difference =
                    (mean_trait(&arch, &a, t, 600) - mean_trait(&arch, &b, t, 600)).abs();
                biggest = biggest.max(difference);
            }
        }
        assert!(
            biggest > 0.15,
            "physical traits should vary between populations, largest gap was {biggest:.3}"
        );
    }

    #[test]
    fn the_uniform_pool_is_even_everywhere() {
        let pool = FounderPool::uniform();
        for locus in 0..N_LOCI {
            assert_eq!(pool.frequency(locus), 0.5);
        }
    }

    #[test]
    fn drawing_is_reproducible() {
        let pool = FounderPool::uniform();
        assert_eq!(pool.draw(&mut rng(7)), pool.draw(&mut rng(7)));
        assert_ne!(pool.draw(&mut rng(7)), pool.draw(&mut rng(8)));
    }
}
