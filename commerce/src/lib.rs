//! What places do for each other once they can reach each other, and what they come to use
//! to keep count.
//!
//! `economy` is what a place makes and what that does to the people in it. This is the next
//! question up: what *many* places do once goods can travel between them — which is where
//! every economy that ever grew got its growth from, and the question §48.2 found this world
//! had never once asked. Regions here have been containers, never parties.
//!
//! The mechanisms, each small and each stated as a comparison rather than a schedule:
//!
//! - `production` — four sectors (farming, making, serving, reckoning) on the same ground the
//!   people-level world stands on, constant returns in land and hands so a town is its villages.
//! - `market` — Stone–Geary demand (everybody eats first), and spatial market clearing on a
//!   tree: a town trades with its state, a state with its country, a country with the world,
//!   and every link has a wedge.
//! - `money` — Menger: a medium is accepted because others accept it, which is bistable, so
//!   barter persists until trade is thick enough and then gives way within a generation; then
//!   coinage where a market is big enough to pay for a mint; then currencies with price levels.
//! - `growth` — births that answer income and then stop answering it, and ideas that come from
//!   people with time to think, with diminishing returns to how many of them there are.
//! - `payments` — trust built by trading and capped by distance, correspondent routes through
//!   whoever is trusted, and the three conditions under which parties who distrust each other
//!   will found a ledger none of them keeps.
//!
//! Nothing here knows about a planet, a seed or a year. `nations` puts it on one.

pub mod growth;
pub mod market;
pub mod money;
pub mod payments;
pub mod production;

pub use market::{Offer, Tastes, Tree};
pub use money::{Acceptance, Currency, Medium};
pub use production::{Endowment, Output, Sector};
