//! What everything alive has in common.
//!
//! People, wolves, and oak trees share needs, aging, and death. Only humans get
//! personality, society, and narrative on top. Keeping the substrate in its own crate
//! is what stops the animal and plant work from re-implementing a second, subtly
//! different version of the same biology later.

pub mod needs;
pub mod vitals;

pub use needs::{Need, Needs};
pub use vitals::{Age, Health, LifeStage, Mortality};
