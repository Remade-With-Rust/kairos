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
| **rusty_alloc** (`rusty_alloc-api`) | `=2.1.0`, `default-features = false`; bare metal adds `RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile"` | host; **all four bare-metal targets** (plain and `small-metal`) | `rusty_rtos_alloc`: the `std` half runs the fleet tool today and will run the demo runner and the C-ABI harness; the `small-metal` half re-exports the whole firmware recipe — `Region`, `good_region_size`, `region_for`, and since 2.1.0 `REGION_ALIGN`, `PrimError` and its four `FERR_*` codes, which is what makes the documented `HEAP.give()?` writable | **ready** on host and all four chips (re-measured 2026-09-10 at 2.1.0), and **it has now run on a board**: `firmware/esp32s3-devkit-region`, 9/9 on an ESP32-S3, 2 MiB cycled through a 192 KiB region with every allocation proven inside it. That is an S3, not a Kairos part, so `build-me-bare` **B3 is closed but B4 is not** — B4 wants the Cortex-M3 cycle count |
| **thoth** | git tag `v0.3.0`, `version = "0.3.0"`, `default-features = false` | host; **both bare-metal targets** | the fleet tool's status glyphs (`thoth::status::{OK, CROSS}`) — switched from the split `rusty_symbols` on 2026-09-09 | **ready** |
| **rusty_json_turbo** | git, `version = "0.1.0"`, lib name `serde_json`; `no_std + alloc` | host; **both bare-metal targets** | `kairos status --json`; `rusty_rtos_json`'s `serde` feature for typed (de)serialization under `alloc` (K7) — the zero-allocation coreJSON validator stays its own engine, one job per parser | **ready** |
| **rusty_zstd** | `=0.2.5` (`no_std + alloc` via `default-features = false, features = ["alloc"]`); the fleet tool still links `=0.2.3` on the host | host; **both bare-metal targets** — 0.2.5 fixed the `AtomicU64` census counters that failed at 0.2.3 | every stored oracle trace is a zstd frame (`oracle/traces/<scenario>.trace.zst`, level 19, nine of them, decompressed and compared before they are kept; `kairos oracle cat` reads them); on-chip OTA and trace capture are now unblocked | **ready** on host and chip (re-measured 2026-09-09 at 0.2.5: `gate-zstd` exit 0 on `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`). Aligning the fleet tool's pin to 0.2.5 is a follow-up, not a blocker |
| **rusty_time** (`rusty_time-core`) | `=0.2.0` (crates.io: `-core`, `-api`, `-clock`); bare metal via `default-features = false` | host; **both bare-metal targets** — 0.2.0 added the `no_std` leaf that 0.1.10 lacked | `rusty_rtos_sntp` (K7) **WRAPS the leaf** rather than remaking coreSNTP: `NtpPacket::{parse, write, to_bytes, client_request}`, `NtpTimestamp`, `NtpShort`, `offset_delay`, `extension_fields`. The leaf needs no `alloc`, so the package needs no allocator seam | **ready** on host and chip (measured 2026-09-09 at 0.2.0: `gate-time` exit 0 on `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`; upstream also ran it on an ESP32-S3, 30/30) |
| **rusty_erasure** (`rusty_erasure-core`) | `=0.4.1` (was `=0.4.0`) | host; **both bare-metal targets** — 0.4.1 fixed the `AtomicU64` census counter that failed at 0.4.0 | nothing in Kairos v1 — erasure shards are SpaceDB placement, above the RTOS | **ready** on host and chip (measured 2026-09-10 at 0.4.1: `gate-erasure` exit 0 on both, a genuine FAIL→OK). `build-me-bare` **B5 closed**; the org's `no-std` label is now true for it |
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
| ~~`rusty_erasure-core` 0.4.0~~ | ~~one `AtomicU64` census counter~~ | — | **closed 2026-09-10 at 0.4.1** |
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
  `rusty_rtos_sntp` (K7) **wraps** `rusty_time-core`'s `no_std` leaf (0.2.0)
  rather than remaking coreSNTP -- it keeps the crate as its host-side oracle
  too, but the chip now runs the house codec itself.
- `plans/build-me-bare.md` queues the bare-metal bricks; `upstream/` holds
  the three issue drafts; `tools/house-gate` re-takes every verdict here in
  one command (`no_std` gates, `host/`, `hostgit/`).
