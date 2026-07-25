//! Seeded, splittable randomness.
//!
//! Two properties that sound contradictory and are not:
//!
//! - **Every new world is different.** [`WorldSeed::from_entropy`] draws 128 bits from
//!   the OS. The dynamics are chaotic, so a one-bit difference gives an unrecognizable
//!   planet a few million years later.
//! - **Any given world is reproducible.** Every random draw inside a world comes from a
//!   stream derived from that world's seed. Nothing calls `thread_rng()`.
//!
//! Reproducibility is not a restriction on variety; it is the mechanism that lets the
//! deep past be *recomputed* rather than stored, which is the only way megayears and
//! individual biographies fit in one program.
//!
//! The generator is implemented here rather than pulled from a dependency on purpose:
//! "the same seed produces the same universe, forever" cannot rest on another crate's
//! freedom to change its internals. xoshiro256** and SplitMix64 are both fixed,
//! published algorithms, so a saved seed stays meaningful indefinitely.

use std::fmt;

/// The 128-bit identity of a world. Everything random in that world descends from it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldSeed(u128);

impl WorldSeed {
    /// A fresh world. Different every time.
    pub fn from_entropy() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        WorldSeed(u128::from_le_bytes(bytes))
    }

    pub const fn from_u128(bits: u128) -> Self {
        WorldSeed(bits)
    }

    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// Parse the form printed by `Display`, with or without a `0x` prefix.
    pub fn parse(text: &str) -> Result<Self, std::num::ParseIntError> {
        let text = text.strip_prefix("0x").unwrap_or(text);
        let cleaned: String = text.chars().filter(|c| *c != '_').collect();
        u128::from_str_radix(&cleaned, 16).map(WorldSeed)
    }

    /// Open a stream for one purpose, for one entity, at one epoch.
    ///
    /// Streams are derived, never shared, so adding a system that draws random numbers
    /// cannot perturb what any other system draws. That independence is what keeps
    /// worlds reproducible as the simulation grows.
    pub fn stream(self, domain: Domain, entity: u64, epoch: u64) -> Rng {
        let lo = self.0 as u64;
        let hi = (self.0 >> 64) as u64;
        Rng::from_parts([lo, hi ^ (domain as u64).wrapping_mul(GOLDEN), entity, epoch])
    }
}

impl fmt::Debug for WorldSeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorldSeed({self})")
    }
}

impl fmt::Display for WorldSeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:032x}", self.0)
    }
}

/// What a stream is *for*. Keeps unrelated systems from disturbing each other's draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Domain {
    World = 0,
    Terrain = 1,
    Climate = 2,
    Ocean = 3,
    Vegetation = 4,
    Ecology = 5,
    Evolution = 6,
    Genetics = 7,
    Naming = 8,
    Behavior = 9,
    Demography = 10,
    Weather = 11,
    /// Deliberate luck: accidents, windfalls, chance meetings.
    Chance = 12,
}

const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;

/// SplitMix64 — the standard way to expand a single word into good generator state.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GOLDEN);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// xoshiro256\*\* — fast, small, and good enough for everything here.
///
/// Not cryptographic, and it does not need to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Build from four words, run through SplitMix64 so that even poor input
    /// (zeros, small counters) gives well-distributed state.
    pub fn from_parts(parts: [u64; 4]) -> Self {
        let mut mixer = SplitMix64::new(
            parts[0]
                ^ parts[1].rotate_left(16)
                ^ parts[2].rotate_left(32)
                ^ parts[3].rotate_left(48),
        );
        let mut s = [0u64; 4];
        for (slot, part) in s.iter_mut().zip(parts) {
            *slot = mixer.next_u64() ^ SplitMix64::new(part).next_u64();
        }
        if s == [0; 4] {
            s = [GOLDEN, 1, 2, 3]; // all-zero state is the one forbidden value
        }
        Rng { s }
    }

    pub fn from_u64(seed: u64) -> Self {
        Rng::from_parts([seed, 0, 0, 0])
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A child stream. Use when a system needs randomness for a sub-task without
    /// perturbing the sequence its caller will see.
    pub fn split(&mut self) -> Rng {
        Rng::from_parts([
            self.next_u64(),
            self.next_u64(),
            self.next_u64(),
            self.next_u64(),
        ])
    }

    /// Uniform in `0.0..1.0`. 53 bits of mantissa, the usual construction.
    pub fn unit_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
    }

    pub fn unit_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) * (1.0 / (1u32 << 24) as f32)
    }

    /// Uniform in `low..=high`, free of modulo bias (Lemire's method).
    pub fn range_u64(&mut self, low: u64, high: u64) -> u64 {
        assert!(low <= high, "empty range {low}..={high}");
        let span = high - low;
        if span == u64::MAX {
            return self.next_u64();
        }
        let span = span + 1;
        let threshold = span.wrapping_neg() % span;
        loop {
            let value = self.next_u64();
            let (hi, lo) = widening_mul(value, span);
            if lo >= threshold {
                return low + hi;
            }
        }
    }

    pub fn range_i64(&mut self, low: i64, high: i64) -> i64 {
        assert!(low <= high, "empty range {low}..={high}");
        let span = (high as i128 - low as i128) as u64;
        low.wrapping_add(self.range_u64(0, span) as i64)
    }

    /// Uniform in `low..high`.
    pub fn range_f64(&mut self, low: f64, high: f64) -> f64 {
        low + self.unit_f64() * (high - low)
    }

    pub fn chance(&mut self, probability: f64) -> bool {
        self.unit_f64() < probability
    }

    pub fn coin(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    /// Standard normal, via Box–Muller. Traits are z-scores, so this gets heavy use.
    pub fn normal(&mut self) -> f64 {
        // Guard against log(0); u must be in (0, 1].
        let u = 1.0 - self.unit_f64();
        let v = self.unit_f64();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        items.get(self.range_u64(0, items.len() as u64 - 1) as usize)
    }

    /// Fisher–Yates, so shuffles are reproducible too.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.range_u64(0, i as u64) as usize;
            items.swap(i, j);
        }
    }
}

fn widening_mul(a: u64, b: u64) -> (u64, u64) {
    let wide = (a as u128) * (b as u128);
    ((wide >> 64) as u64, wide as u64)
}

// Interop with the `rand` ecosystem (name generation and similar), without letting
// any of it reach for entropy of its own.
impl rand::RngCore for Rng {
    fn next_u32(&mut self) -> u32 {
        Rng::next_u32(self)
    }

    fn next_u64(&mut self) -> u64 {
        Rng::next_u64(self)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut chunks = dest.chunks_exact_mut(8);
        for chunk in &mut chunks {
            chunk.copy_from_slice(&Rng::next_u64(self).to_le_bytes());
        }
        let tail = chunks.into_remainder();
        if !tail.is_empty() {
            let bytes = Rng::next_u64(self).to_le_bytes();
            tail.copy_from_slice(&bytes[..tail.len()]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let seed = WorldSeed::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);
        let a: Vec<u64> = (0..64)
            .scan(seed.stream(Domain::Terrain, 7, 0), |r, _| {
                Some(r.next_u64())
            })
            .collect();
        let b: Vec<u64> = (0..64)
            .scan(seed.stream(Domain::Terrain, 7, 0), |r, _| {
                Some(r.next_u64())
            })
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn streams_are_independent() {
        let seed = WorldSeed::from_u128(99);
        let draw = |domain, entity, epoch| {
            let mut rng = seed.stream(domain, entity, epoch);
            (0..8).map(|_| rng.next_u64()).collect::<Vec<_>>()
        };

        let base = draw(Domain::Terrain, 1, 0);
        assert_ne!(base, draw(Domain::Climate, 1, 0), "domain must matter");
        assert_ne!(base, draw(Domain::Terrain, 2, 0), "entity must matter");
        assert_ne!(base, draw(Domain::Terrain, 1, 1), "epoch must matter");
    }

    #[test]
    fn neighbouring_seeds_diverge() {
        // Adjacent seeds must not produce correlated worlds.
        let a: Vec<u64> = (0..8)
            .scan(
                WorldSeed::from_u128(1000).stream(Domain::World, 0, 0),
                |r, _| Some(r.next_u64()),
            )
            .collect();
        let b: Vec<u64> = (0..8)
            .scan(
                WorldSeed::from_u128(1001).stream(Domain::World, 0, 0),
                |r, _| Some(r.next_u64()),
            )
            .collect();
        assert!(a.iter().zip(&b).all(|(x, y)| x != y));
    }

    #[test]
    fn fresh_worlds_differ() {
        let a = WorldSeed::from_entropy();
        let b = WorldSeed::from_entropy();
        assert_ne!(a, b, "a new world must not repeat the last one");
    }

    #[test]
    fn seed_round_trips_through_text() {
        let seed = WorldSeed::from_entropy();
        assert_eq!(WorldSeed::parse(&seed.to_string()).unwrap(), seed);
    }

    #[test]
    fn ranges_stay_in_bounds_and_cover() {
        let mut rng = Rng::from_u64(5);
        let mut seen = [false; 6];
        for _ in 0..2_000 {
            let roll = rng.range_u64(1, 6);
            assert!((1..=6).contains(&roll));
            seen[roll as usize - 1] = true;
        }
        assert!(seen.iter().all(|hit| *hit), "every face should come up");
    }

    #[test]
    fn degenerate_range_is_a_constant() {
        let mut rng = Rng::from_u64(7);
        assert_eq!(rng.range_u64(42, 42), 42);
        assert_eq!(rng.range_i64(-3, -3), -3);
    }

    #[test]
    fn negative_ranges_work() {
        let mut rng = Rng::from_u64(11);
        for _ in 0..500 {
            let value = rng.range_i64(-10, 10);
            assert!((-10..=10).contains(&value));
        }
    }

    #[test]
    fn unit_stays_in_the_unit_interval() {
        let mut rng = Rng::from_u64(3);
        for _ in 0..5_000 {
            let f = rng.unit_f64();
            assert!((0.0..1.0).contains(&f));
            let g = rng.unit_f32();
            assert!((0.0..1.0).contains(&g));
        }
    }

    #[test]
    fn normal_has_roughly_the_right_shape() {
        let mut rng = Rng::from_u64(17);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "mean {mean} should be ~0");
        assert!((var - 1.0).abs() < 0.1, "variance {var} should be ~1");
    }

    #[test]
    fn chance_is_calibrated() {
        let mut rng = Rng::from_u64(23);
        let hits = (0..10_000).filter(|_| rng.chance(0.25)).count();
        assert!((2_200..2_800).contains(&hits), "got {hits}/10000");
    }

    #[test]
    fn shuffle_permutes_deterministically() {
        let mut a: Vec<u32> = (0..32).collect();
        let mut b = a.clone();
        Rng::from_u64(1).shuffle(&mut a);
        Rng::from_u64(1).shuffle(&mut b);
        assert_eq!(a, b);

        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            (0..32).collect::<Vec<_>>(),
            "shuffle must not lose items"
        );
        assert_ne!(a, sorted, "and should actually reorder");
    }

    #[test]
    fn split_yields_a_divergent_stream() {
        let mut parent = Rng::from_u64(1);
        let mut child = parent.split();
        let from_child: Vec<u64> = (0..8).map(|_| child.next_u64()).collect();
        let from_parent: Vec<u64> = (0..8).map(|_| parent.next_u64()).collect();
        assert_ne!(from_child, from_parent);
    }

    #[test]
    fn all_zero_state_is_avoided() {
        let mut rng = Rng::from_parts([0, 0, 0, 0]);
        assert_ne!(rng.next_u64(), 0);
        assert_ne!(rng.next_u64(), 0);
    }
}
