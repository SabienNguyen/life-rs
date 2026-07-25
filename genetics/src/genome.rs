//! Genomes, and how they are passed on.
//!
//! Not molecular. Simulating base pairs costs a great deal and changes nothing anyone
//! can observe; what produces trait variation is many small-effect loci, so that is the
//! layer modelled here.
//!
//! Loci are biallelic, which lets a haplotype be a bitset — 32 bytes, so a diploid
//! genome is 64. At that size a million people's genomes fit in 64 MB and inheritance
//! is a handful of word operations.

use sim_core::Rng;

/// How many loci a genome carries. Enough for polygenic traits to go normal by the
/// central limit theorem, small enough to stay cheap.
pub const N_LOCI: usize = 256;

const N_WORDS: usize = N_LOCI / 64;

/// One inherited copy of the genome.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Haplotype {
    words: [u64; N_WORDS],
}

impl Haplotype {
    pub const EMPTY: Haplotype = Haplotype {
        words: [0; N_WORDS],
    };

    /// Draw from a uniform allele frequency across every locus.
    pub fn sample(rng: &mut Rng, frequency: f64) -> Haplotype {
        let mut words = [0u64; N_WORDS];
        for (i, word) in words.iter_mut().enumerate() {
            for bit in 0..64 {
                let locus = i * 64 + bit;
                if locus < N_LOCI && rng.chance(frequency) {
                    *word |= 1 << bit;
                }
            }
        }
        Haplotype { words }
    }

    pub fn get(&self, locus: usize) -> bool {
        debug_assert!(locus < N_LOCI);
        self.words[locus / 64] & (1 << (locus % 64)) != 0
    }

    pub fn set(&mut self, locus: usize, value: bool) {
        debug_assert!(locus < N_LOCI);
        let (word, bit) = (locus / 64, locus % 64);
        if value {
            self.words[word] |= 1 << bit;
        } else {
            self.words[word] &= !(1 << bit);
        }
    }

    /// How many loci carry the allele. Used to check drift and frequency.
    pub fn count(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    /// Loci where two haplotypes differ — the raw material of genetic distance.
    pub fn differences(&self, other: &Haplotype) -> u32 {
        self.words
            .iter()
            .zip(&other.words)
            .map(|(a, b)| (a ^ b).count_ones())
            .sum()
    }

    /// Take bits from `self` where the mask is set, from `other` where it is not.
    fn blend(&self, other: &Haplotype, mask: &[u64; N_WORDS]) -> Haplotype {
        let mut words = [0u64; N_WORDS];
        for i in 0..N_WORDS {
            words[i] = (self.words[i] & mask[i]) | (other.words[i] & !mask[i]);
        }
        Haplotype { words }
    }
}

impl std::fmt::Debug for Haplotype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Haplotype({} of {N_LOCI})", self.count())
    }
}

/// A diploid genome: one haplotype from each parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Genome {
    pub maternal: Haplotype,
    pub paternal: Haplotype,
}

impl Genome {
    /// A founder's genome, drawn from a population's allele frequencies.
    pub fn founder(rng: &mut Rng, frequency: f64) -> Genome {
        Genome {
            maternal: Haplotype::sample(rng, frequency),
            paternal: Haplotype::sample(rng, frequency),
        }
    }

    /// How many copies of the allele are carried at this locus: 0, 1, or 2.
    pub fn dosage(&self, locus: usize) -> u8 {
        u8::from(self.maternal.get(locus)) + u8::from(self.paternal.get(locus))
    }

    pub fn is_heterozygous(&self, locus: usize) -> bool {
        self.dosage(locus) == 1
    }

    /// Proportion of loci at which two genomes carry different dosages. A crude
    /// genetic distance — enough for the isolation-and-divergence test that decides
    /// speciation later on.
    pub fn distance(&self, other: &Genome) -> f32 {
        let differing = (0..N_LOCI)
            .filter(|&locus| self.dosage(locus) != other.dosage(locus))
            .count();
        differing as f32 / N_LOCI as f32
    }
}

/// The number of crossovers per gamete. Real chromosomes average one or two per arm;
/// too few and whole genomes travel together, too many and linkage disappears.
const MEAN_CROSSOVERS: f64 = 2.5;

/// Chance per locus of a copying error. Low enough that mutation is a slow source of
/// novelty rather than noise, high enough that isolated populations drift apart.
const MUTATION_RATE: f64 = 0.0005;

/// Produce one gamete: a haplotype recombined from the parent's two copies.
pub fn meiosis(parent: &Genome, rng: &mut Rng) -> Haplotype {
    let mask = crossover_mask(rng);

    // Which copy the gamete starts reading from is itself a coin flip; without it,
    // every gamete would begin with the maternal allele at locus zero.
    let mut gamete = if rng.coin() {
        parent.maternal.blend(&parent.paternal, &mask)
    } else {
        parent.paternal.blend(&parent.maternal, &mask)
    };

    for locus in 0..N_LOCI {
        if rng.chance(MUTATION_RATE) {
            gamete.set(locus, !gamete.get(locus));
        }
    }
    gamete
}

/// A child's genome, from its parents and one seed.
///
/// Pure: the same parents and the same seed always give the same child. That is what
/// lets a genome be stored as a reference to its parents plus a seed and reconstructed
/// on demand rather than kept.
pub fn conceive(mother: &Genome, father: &Genome, recomb_seed: u64) -> Genome {
    let mut rng = Rng::from_u64(recomb_seed);
    Genome {
        maternal: meiosis(mother, &mut rng),
        paternal: meiosis(father, &mut rng),
    }
}

/// Where a genome came from. Store this, not the genome, once memory matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ancestry {
    /// `None` for a founder, whose genome comes from the world seed instead.
    pub parents: Option<(u64, u64)>,
    pub recomb_seed: u64,
}

impl Ancestry {
    pub fn founder(seed: u64) -> Ancestry {
        Ancestry {
            parents: None,
            recomb_seed: seed,
        }
    }

    pub fn of(mother: u64, father: u64, recomb_seed: u64) -> Ancestry {
        Ancestry {
            parents: Some((mother, father)),
            recomb_seed,
        }
    }

    pub fn is_founder(&self) -> bool {
        self.parents.is_none()
    }
}

fn crossover_mask(rng: &mut Rng) -> [u64; N_WORDS] {
    let count = poisson(rng, MEAN_CROSSOVERS).min(N_LOCI as u32);
    let mut points: Vec<usize> = (0..count)
        .map(|_| rng.range_u64(1, N_LOCI as u64 - 1) as usize)
        .collect();
    points.sort_unstable();

    let mut mask = [0u64; N_WORDS];
    let mut taking = true;
    let mut next = 0;
    for locus in 0..N_LOCI {
        while next < points.len() && points[next] == locus {
            taking = !taking;
            next += 1;
        }
        if taking {
            mask[locus / 64] |= 1 << (locus % 64);
        }
    }
    mask
}

/// Knuth's method. Fine at these means, and it keeps the crossover count a draw rather
/// than a constant — otherwise every gamete recombines identically.
fn poisson(rng: &mut Rng, mean: f64) -> u32 {
    let limit = (-mean).exp();
    let mut count = 0;
    let mut product = rng.unit_f64();
    while product > limit && count < 64 {
        count += 1;
        product *= rng.unit_f64();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0x6e_11e).stream(Domain::Genetics, n, 0)
    }

    fn founder(n: u64) -> Genome {
        Genome::founder(&mut rng(n), 0.5)
    }

    #[test]
    fn a_genome_is_small() {
        assert_eq!(size_of::<Genome>(), 64, "64 bytes is the whole point");
    }

    #[test]
    fn haplotypes_read_back_what_was_written() {
        let mut h = Haplotype::EMPTY;
        assert_eq!(h.count(), 0);
        h.set(0, true);
        h.set(255, true);
        h.set(130, true);
        assert!(h.get(0) && h.get(255) && h.get(130));
        assert!(!h.get(1) && !h.get(129));
        assert_eq!(h.count(), 3);

        h.set(130, false);
        assert!(!h.get(130));
        assert_eq!(h.count(), 2);
    }

    #[test]
    fn founders_carry_roughly_the_stated_frequency() {
        let mut carried = 0;
        let genomes = 200;
        for i in 0..genomes {
            let g = founder(i);
            carried += g.maternal.count() + g.paternal.count();
        }
        let frequency = carried as f64 / (genomes * 2 * N_LOCI as u64) as f64;
        assert!((frequency - 0.5).abs() < 0.02, "frequency was {frequency}");
    }

    #[test]
    fn dosage_counts_both_copies() {
        let mut g = Genome {
            maternal: Haplotype::EMPTY,
            paternal: Haplotype::EMPTY,
        };
        assert_eq!(g.dosage(5), 0);
        g.maternal.set(5, true);
        assert_eq!(g.dosage(5), 1);
        assert!(g.is_heterozygous(5));
        g.paternal.set(5, true);
        assert_eq!(g.dosage(5), 2);
        assert!(!g.is_heterozygous(5));
    }

    #[test]
    fn conception_is_pure() {
        // The property the whole storage scheme rests on: same parents, same seed,
        // same child, forever.
        let (mother, father) = (founder(1), founder(2));
        assert_eq!(
            conceive(&mother, &father, 42),
            conceive(&mother, &father, 42)
        );
        assert_ne!(
            conceive(&mother, &father, 42),
            conceive(&mother, &father, 43)
        );
    }

    #[test]
    fn a_child_inherits_one_copy_from_each_parent() {
        let (mother, father) = (founder(3), founder(4));
        let child = conceive(&mother, &father, 7);

        // Every maternal allele must exist in the mother at that locus — barring the
        // rare mutation, which is why this counts rather than asserts outright.
        let impossible = (0..N_LOCI)
            .filter(|&l| {
                let from_mother = child.maternal.get(l);
                from_mother != mother.maternal.get(l) && from_mother != mother.paternal.get(l)
            })
            .count();
        assert!(
            impossible < 5,
            "{impossible} maternal alleles came from nowhere"
        );
    }

    #[test]
    fn siblings_share_about_half_their_variation() {
        // Same parents, different draws. This is what makes siblings resemble each
        // other without being copies, and it falls out of meiosis rather than being
        // imposed.
        //
        // Calibrated against the unrelated baseline measured here rather than against a
        // number worked out on paper: under Hardy-Weinberg two strangers differ in
        // dosage at ~5/8 of loci, which is easy to misremember as 3/8.
        let pairs = 300;
        let mean =
            |f: &dyn Fn(u64) -> f32| (0..pairs).map(|i| f(i) as f64).sum::<f64>() / pairs as f64;

        let (mother, father) = (founder(10), founder(11));
        let siblings = mean(&|i| {
            conceive(&mother, &father, i * 2).distance(&conceive(&mother, &father, i * 2 + 1))
        });
        let strangers = mean(&|i| founder(1_000 + i * 2).distance(&founder(1_001 + i * 2)));

        assert!(
            siblings < strangers * 0.75,
            "siblings ({siblings:.3}) should be much closer than strangers ({strangers:.3})"
        );
        assert!(siblings > 0.15, "but not clones: {siblings:.3}");
    }

    #[test]
    fn siblings_are_closer_than_strangers() {
        let (mother, father) = (founder(20), founder(21));
        let sib_a = conceive(&mother, &father, 1);
        let sib_b = conceive(&mother, &father, 2);
        let stranger = founder(22);

        let mut closer = 0;
        for i in 0..200 {
            let a = conceive(&mother, &father, i * 2);
            let b = conceive(&mother, &father, i * 2 + 1);
            if a.distance(&b) < a.distance(&stranger) {
                closer += 1;
            }
        }
        assert!(
            closer > 180,
            "siblings resembled strangers {} times",
            200 - closer
        );
        assert!(sib_a.distance(&sib_b) < sib_a.distance(&stranger));
    }

    #[test]
    fn recombination_actually_shuffles() {
        // Without crossovers a child would inherit one parental haplotype wholesale,
        // and siblings would come in only four varieties.
        let (mother, father) = (founder(30), founder(31));
        let children: std::collections::HashSet<Genome> =
            (0..64).map(|i| conceive(&mother, &father, i)).collect();
        assert!(
            children.len() > 50,
            "only {} distinct children",
            children.len()
        );
    }

    #[test]
    fn a_gamete_can_start_from_either_parental_copy() {
        // Guards a subtle bug: crossover points never land on locus zero, so if the
        // blend always began with the maternal copy, the first locus could never be
        // inherited from the paternal one.
        //
        // Has to be checked at a locus where the mother actually carries two different
        // alleles — at a homozygous locus both copies agree and the flip is invisible.
        let mother = (40..200)
            .map(founder)
            .find(|g| g.is_heterozygous(0))
            .expect("some founder differs at locus zero");
        let father = founder(41);

        let mut from_maternal = 0;
        for i in 0..200 {
            if conceive(&mother, &father, i).maternal.get(0) == mother.maternal.get(0) {
                from_maternal += 1;
            }
        }
        assert!(
            (60..140).contains(&from_maternal),
            "locus zero came from the maternal copy {from_maternal}/200 times"
        );
    }

    #[test]
    fn mutation_is_rare_but_real() {
        let identical = Genome {
            maternal: Haplotype::EMPTY,
            paternal: Haplotype::EMPTY,
        };
        let mut mutations = 0;
        let trials = 200;
        for i in 0..trials {
            // Both parents are all-zero, so any set bit in a child is a mutation.
            let child = conceive(&identical, &identical, i);
            mutations += child.maternal.count() + child.paternal.count();
        }
        let per_locus = mutations as f64 / (trials * 2 * N_LOCI as u64) as f64;
        assert!(mutations > 0, "no novelty at all");
        assert!(
            (per_locus - MUTATION_RATE).abs() < MUTATION_RATE,
            "mutation rate {per_locus} is not near {MUTATION_RATE}"
        );
    }

    #[test]
    fn distance_is_zero_to_self_and_positive_to_others() {
        let a = founder(50);
        assert_eq!(a.distance(&a), 0.0);
        assert!(a.distance(&founder(51)) > 0.0);
    }

    #[test]
    fn ancestry_distinguishes_founders_from_descendants() {
        assert!(Ancestry::founder(1).is_founder());
        let descended = Ancestry::of(7, 8, 9);
        assert!(!descended.is_founder());
        assert_eq!(descended.parents, Some((7, 8)));
    }
}
