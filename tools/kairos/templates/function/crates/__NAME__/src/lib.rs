#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! `__NAME__` — __DESCRIPTION__
//!
//! This is the facade: it re-exports the `no_std` core. Depend on this crate;
//! reach into the sub-crates only when you are building a port or a backend.
//!
//! Part of Kairos (Remade With Rust). Plan: `docs/plans/__NAME__.md`.

pub use __NAME_IDENT___core::*;

/// The names a firmware wants in scope.
pub mod prelude {
    pub use __NAME_IDENT___core::prelude::*;
}
