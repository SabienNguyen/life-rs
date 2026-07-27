//! Reading a world without changing it.
//!
//! Everything here takes `&World` and never `&mut`. Keeping the omniscient view strictly
//! read-only is what stops it becoming a god-mode editor by accident, and it is what
//! lets any query be run at any moment without perturbing the run being observed.

pub mod balance;
pub mod dossier;
#[cfg(test)]
mod balance_tests;
#[cfg(test)]
mod dossier_tests;

pub use balance::{Balance, Shares, measure};
pub use dossier::{
    Attribution, Dossier, Kin, Reasoning, Whereabouts, ancestry, descendants, dossier, life, why,
};
