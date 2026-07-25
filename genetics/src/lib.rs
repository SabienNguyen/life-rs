//! Inheritance.
//!
//! A population is an allele-frequency distribution and an individual is a draw from
//! it, so the same machinery serves one person's parentage and a species' evolution —
//! the difference is only which resolution you look at. Phase 2 uses the individual
//! end; the population end is what selection and drift will act on later.

pub mod architecture;
pub mod genome;
pub mod pool;

use std::sync::LazyLock;

pub use architecture::{Architecture, Expression, Trait};

/// The species' genetic architecture.
///
/// Shared process-wide rather than held per world: which loci build which traits is a
/// property of the species, not of a planet, and building it is deterministic anyway.
pub fn standard_architecture() -> &'static Architecture {
    static ARCHITECTURE: LazyLock<Architecture> = LazyLock::new(Architecture::standard);
    &ARCHITECTURE
}
pub use genome::{Ancestry, Genome, Haplotype, N_LOCI, conceive, meiosis};
pub use pool::FounderPool;
