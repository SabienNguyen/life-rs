//! Reading a world without changing it.
//!
//! Everything here takes `&World` and never `&mut`. Keeping the omniscient view strictly
//! read-only is what stops it becoming a god-mode editor by accident, and it is what
//! lets any query be run at any moment without perturbing the run being observed.

pub mod balance;

pub use balance::{Balance, Shares, measure};
