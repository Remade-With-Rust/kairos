//! The allocator seam: `rusty_alloc` as the fleet tool's global allocator.
//!
//! This crate does not declare the allocator — a library must never do that,
//! because a program may define exactly one `#[global_allocator]` and a
//! library that declares it forces the choice on every consumer. It hands out
//! the type and the pin; `kairos-cli`'s `main.rs` does the declaring:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: kairos_alloc::Alloc = kairos_alloc::Alloc;
//! ```
//!
//! Why rusty_alloc at all for a tool that runs for seconds: the safety
//! posture, not speed. A double free aborts instead of handing one block to
//! two owners, and the whole portfolio runs on one allocator so its behaviour
//! is one thing to understand.

#![no_std]
#![forbid(unsafe_code)]

/// The global allocator type. A deliverable writes
/// `#[global_allocator] static A: Alloc = Alloc;` and nothing else.
pub use rusty_alloc_api::RustyAlloc as Alloc;

/// The pinned allocator version, so the tool can print what it runs rather
/// than what its manifest said.
pub use rusty_alloc_api::VERSION;
