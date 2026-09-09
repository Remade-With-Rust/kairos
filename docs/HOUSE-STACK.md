# The house stack in Kairos — ready to use, crate by crate

The `building-the-new-internet` requirements name a fixed stack: reach for the
house crate first, no C anywhere in the build, the allocator through a seam,
compress-then-encrypt, Deputy for the supply chain, SpaceDB for storage, and
the ladder (memorysafety.org → RustCrypto → Remade-With-Rust → Oxi* →
crates.io) before anything else. This page answers, for every house crate the
requirements name, whether Kairos is **ready to use it**: the pin that works,
proven by a compile at that pin under this family's `deny.toml`, and the
place it enters — today, on the host, and on a Cortex-M4F / RV32 part.

"Ready" here has one meaning: *the crate compiles at a released pin, under
the Kairos policy, on every target where Kairos would call it, and the call
site is named.* Where the target is a 32-bit bare-metal part and the crate
does not build there yet, the crate is ready on the host and the bare-metal
work is a dated brick in [`plans/build-me-bare.md`](plans/build-me-bare.md).
Every verdict was taken by `tools/house-gate` (rustc 1.98.0, 2026-09-09;
`thumbv7em-none-eabihf`, `riscv32imac-unknown-none-elf`, the host), not by
reading a README.

The one-line test the skill sets — *could this ship to a user who assumes
their data is theirs alone, onto a machine you do not own, with no C
toolchain anywhere in the build?* — holds for every Kairos package: the only
C in the umbrella is the FreeRTOS oracle, a dev-only comparison arm that no
package depends on (`ORACLES.md`).

## All eleven, and the two the requirements add

| house crate | the pin that works | proven where | Kairos uses it | verdict |
|---|---|---|---|---|
| **rusty_alloc** (`rusty_alloc-api`) | `=2.0.4`, `default-features = false`; bare metal adds `RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile"` | host; **all four bare-metal targets** (plain and `small-metal`) | `rusty_rtos_alloc`: the `std` half runs the fleet tool today and will run the demo runner and the C-ABI harness; the `small-metal` half re-exports `Region` / `good_region_size` / `region_for` for firmware (`heap_3`, K4) | **ready** on host and chip; the board number is K4's ledger row (`build-me-bare` B3, B4) |
| **thoth** | git tag `v0.3.0`, `version = "0.3.0"`, `default-features = false` | host; **both bare-metal targets** | the fleet tool's status glyphs (`thoth::status::{OK, CROSS}`) — switched from the split `rusty_symbols` on 2026-09-09 | **ready** |
| **rusty_json_turbo** | git, `version = "0.1.0"`, lib name `serde_json`; `no_std + alloc` | host; **both bare-metal targets** | `kairos status --json`; `rusty_rtos_json`'s `serde` feature for typed (de)serialization under `alloc` (K7) — the zero-allocation coreJSON validator stays its own engine, one job per parser | **ready** |
| **rusty_zstd** | `=0.2.3` (`std`); `no_std + alloc` via `default-features = false, features = ["alloc"]` | host: yes; 32-bit bare metal: **not yet** (eight `AtomicU64` census counters) | every stored oracle trace is a zstd frame (`oracle/traces/<scenario>.trace.zst`, level 19, 757,457 → 50,346 bytes for `dynamic`, decompressed and compared before it is kept; `kairos oracle cat` reads it); on-chip OTA and trace capture after the fix | **ready on the host**, in use; chip: `build-me-bare` B1 |
| **rusty_time** (`rusty_time-core`) | `=0.1.10` (crates.io: `-core`, `-api`, `-clock`) | host: yes; bare metal: **not yet** (`std`-only) | the host-side **oracle** for `rusty_rtos_sntp` (K7): the remake's packet codec diffs against `NtpPacket::{parse, write}`; when the `no_std` leaf lands the package wraps it instead | **ready on the host** as an oracle; chip: `build-me-bare` B2 |
| **rusty_erasure** (`rusty_erasure-core`) | `=0.4.0` | host: yes; bare metal: **not yet** (one `AtomicU64` census counter) | nothing in Kairos v1 — erasure shards are SpaceDB placement, above the RTOS | **ready on the host**, no call site; chip: `build-me-bare` B5 |
| **rusty_xml** | `=0.8.1` | host: yes; bare metal: no (`std`-only) | nothing — the FreeRTOS portfolio has no XML | **ready on the host**, no call site, none foreseen |
| **SpaceDB** (`spacedb-sdk`) | `=0.6.0`, `default-features = false` for a library | host: compiles (21 s with FFAI); closure clean of C, `ring`, `aws-lc-sys` | nothing in v1: a kernel persists no user data; the ledgers and traces are files in git by design; `rusty_rtos_fat` (K9) is where SpaceDB's `ShardStore`/`Transport` seams meet a chip | **ready on the host**, call site deferred to K9 by design |
| **FFAI** (`ffai-core`) | `=0.7.1` (rust 1.95) | host: compiles (candle-core in the closure); clean of C | nothing — AI runs above the RTOS; on-edge vision is Janus's | **ready on the host**, no call site by design |
| **remade_ffmpeg_rs** (`remade-ffmpeg`, lib `rff`) | git, `version = "0.2.1"`, `default-features = false` (skips `h264-asm`) | host: compiles (305 packages, 54 s); never the crates.io `rff` | nothing — media is `rusty_esp_video`'s, above the kernel | **ready on the host**, no call site by design |
| **rusty_maps** (`rmap-core`) | private git, `version = "0.0.0"`, `default-features = false` (it is `no_std + alloc` capable); `CARGO_NET_GIT_FETCH_WITH_CLI=true` for the credentials | host: compiles | nothing — a maps UI | **ready on the host**, no call site |
| **Deputy** | `deputy 0.4.0` on the box | `deputy discover` reads every package's `Cargo.lock` (core: 15 pinned crates) | the gate step before a public flip (market-ready bar) | **ready**; `acquire` / `scan` / `gate` need the owner's `DEPUTY_PASSPHRASE` and vault |
| **rusty_symbols** (the split of thoth) | `=0.1.0`, `default-features = false` | host; both bare-metal targets | was the tool's glyph source until thoth replaced it; stays proven in the gate | **ready** |

Settled without a crate: **mID** (identity) has no place in a kernel;
**RustCrypto / rustls** enter with `rusty_rtos_tcp` and the NTS work (K7);
the **Oxi\*** replacements have nothing to replace here; **Dioxus** is not a
firmware framework.

## What "not yet on bare metal" means, precisely

Four crates carry the `no-std` category on crates.io and do not build on a
Cortex-M4F or an RV32 at their current pins. None of them is a Kairos
blocker today, and the ones Kairos wants there are bricks with kill tests:

| crate | why it fails on 32-bit bare metal | wanted by Kairos on a chip? | brick |
|---|---|---|---|
| `rusty_zstd` 0.2.3 | eight `AtomicU64` **census counters** (instrumentation, not the codec) | yes: OTA, on-chip trace capture | B1 |
| `rusty_time-core` 0.1.10 | no `#![no_std]`; the packet codec (`ntp.rs`) needs only a `std::error::Error` impl and `f64` division | yes: the SNTP leaf | B2 |
| `rusty_erasure-core` 0.4.0 | one `AtomicU64` census counter | no (v1) | B5 |
| `rusty_xml` 0.8.1 | `std`-only sax/tree | no, ever | — |

Issue drafts with the reproduction and the fix: [`upstream/`](upstream/).
Filing them is the owner's act.

## The four imposters

Four crates.io names look like house crates and are not. Every Kairos
`deny.toml` (template and all eight packages) bans them by name, and the
compile gate proved the ban fires:

| crates.io name | what it actually is | the house crate |
|---|---|---|
| `rusty_time` 1.1.0 | cleancut's timer crate | `rusty_time-core` / `-api` / `-clock` |
| `rff` 0.3.0 | a fuzzy finder | `remade_ffmpeg_rs` by git URL (package `remade-ffmpeg`, lib `rff`) |
| `thoth` 0.1.10 | a GraphQL client | the house `thoth` by git tag `v0.3.0` (what the tool uses), or `rusty_symbols` / `rusty_tokens` / `rusty_a11y` |
| `spacedb` 0.1.4 | spacesprotocol's | `spacedb-sdk` |

## Rules the gate taught

- **Every git dependency carries a `version`** beside its URL, or
  `wildcards = "deny"` fails the build. Sibling deps already do; house git
  deps (`thoth`, `rusty_json_turbo`, `remade-ffmpeg`, `rmap-core`) do the same.
- **`allow-git` lists the exact URL form the manifest uses** (no `.git`
  suffix when the manifest has none) and only URLs the graph contains; an
  unused entry is a warning on every run. A package's policy names the
  sibling only; a house git dep adds its URL the day it is added.
- **A library opts out of the allocator**: `thoth` and `rusty_symbols` pull
  `rusty_alloc` by default; `default-features = false` in every library,
  the deliverable opts in through `rusty_rtos_alloc`.
- **`no_std` on a label is not `no_std` on a Cortex-M4.** The gate, not the
  category, is the claim.
- **Licences in the house graph:** MIT, Apache-2.0, Unlicense (`memchr`),
  Unicode-3.0 (`unicode-ident`) — all allowed by the Kairos policy;
  `cargo deny check` passes over the whole no_std graph.

## What changed in the family because of this page (2026-09-09)

- The fleet tool now runs on four house crates: `rusty_alloc` (the seam),
  `thoth` (glyphs), `rusty_json_turbo` (`status --json`), `rusty_zstd`
  (stored traces, round-tripped).
- `rusty_rtos_alloc` gained the `small-metal` feature (the firmware half)
  and a CI rung on the four bare-metal targets under rusty_alloc's two cfgs.
- `KAIROS.toml` gained `cfgs` per package and `kairos check` a feature-aware
  `no_std` ladder (`core`, then `alloc` and `small-metal` where declared).
- Every `deny.toml` bans the four imposters.
- `rusty_rtos_json`'s plan records its relation to `rusty_json_turbo`;
  `rusty_rtos_sntp` (K7) takes `rusty_time-core` as its oracle.
- `plans/build-me-bare.md` queues the bare-metal bricks; `upstream/` holds
  the three issue drafts; `tools/house-gate` re-takes every verdict here in
  one command (`no_std` gates, `host/`, `hostgit/`).
