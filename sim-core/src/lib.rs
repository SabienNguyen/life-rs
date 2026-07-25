//! Foundations for the simulation: handles, seeded randomness, simulated time, the
//! scale ladder, scheduling, and the event log.
//!
//! This crate depends on nothing else in the workspace, and everything else depends on
//! it. Four things live here because getting them wrong later would mean rewriting
//! every system built on top:
//!
//! - **[`Id`] and [`Arena`]** — entities are addressed by handle, never by reference,
//!   so a family can be a cycle and any entity can be reached at any time.
//! - **[`WorldSeed`] and [`Rng`]** — a new world is genuinely new, and a given world
//!   stays reproducible, which is what allows deep history to be recomputed instead of
//!   stored.
//! - **[`TimeScale`]** — the ladder from a 15-minute agent tick to a megayear of plate
//!   motion. A million years is not reachable by ticking faster.
//! - **[`Scheduler`]** — nothing is polled; dormant entities cost nothing.

pub mod chronicle;
pub mod id;
pub mod rng;
pub mod schedule;
pub mod time;

pub use chronicle::{Chronicle, Record, Salience};
pub use id::{Arena, Id};
pub use rng::{Domain, Rng, WorldSeed};
pub use schedule::Scheduler;
pub use time::{Duration, Time, TimeScale};
