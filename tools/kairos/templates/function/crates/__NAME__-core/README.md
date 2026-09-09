# __NAME__-core

The pure `no_std` core of [`__NAME__`](https://crates.io/crates/__NAME__):
types, traits and algorithms with no CPU, no allocator and no operating system.
`forbid(unsafe)`. Tests run on the host; the crate compiles for Cortex-M and
RISC-V bare metal with `--no-default-features`.

Feature ladder: `std` ⊃ `alloc` ⊃ core-only.

Part of Kairos (Remade With Rust). Plan: `docs/plans/__NAME__.md` in the repo.
