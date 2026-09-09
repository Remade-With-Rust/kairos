# The house stack in Kairos — what is ready, what is a gap, what does not apply

The `building-the-new-internet` requirements name a fixed stack: reach for the
house crate first, no C anywhere in the build, the allocator through a seam,
compress-then-encrypt, Deputy for the supply chain, SpaceDB for storage, and
the ladder (memorysafety.org → RustCrypto → Remade-With-Rust → Oxi* →
crates.io) before anything else. This page answers, crate by crate, whether
Kairos — an RTOS family that builds `no_std`, `forbid(unsafe)` in its cores,
on Cortex-M and RV32 parts — can consume each one **today**, and where it
enters. Every verdict was taken by a compile gate, not by reading a README:
`tools/house-gate` (this umbrella), rustc 1.98.0, 2026-09-09, targets
`thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` plus the host.

The one-line test the skill sets — *could this ship to a user who assumes
their data is theirs alone, onto a machine you do not own, with no C
toolchain anywhere in the build?* — holds for every Kairos package: the only
C in the umbrella is the FreeRTOS oracle, a dev-only comparison arm that no
package depends on (`ORACLES.md`).

## The verdicts

| house crate | what the house uses it for | the pin that works | on the Kairos targets | where it enters Kairos | verdict |
|---|---|---|---|---|---|
| **rusty_alloc** (`rusty_alloc-api`) | the allocator, through a one-crate seam, declared only by deliverables | `=2.0.4`, `default-features = false`; `RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile"` for `no_std` | **builds on all four bare-metal targets** under the two cfgs; refuses without the first, by design | `rusty_rtos_alloc`: `std` half for the demo runner, the C-ABI harness, the fleet tool (`kairos` runs on it today); `small-metal` half re-exports `Region` / `good_region_size` / `region_for` for firmware, checked by CI and `kairos check` (`cfgs` in `KAIROS.toml`); `heap_3` is the seam's kernel-side face (K4) | **ready** — both halves compile; the board number is K4's ledger row |
| **thoth** | UI chrome: glyphs, tokens, a11y (or the split trio) | git tag `v0.3.0` with `version = "0.3.0"`, `default-features = false` (the default pins `rusty_alloc-api =1.1.6`, a library declaring an allocator dep — exactly what a library must opt out of) | **builds `no_std`** on both targets | Kairos uses the split crate: `rusty_symbols =0.1.0`, `default-features = false`, in the fleet tool's status glyphs; nothing on a chip draws chrome | **ready** (as `rusty_symbols`; thoth itself proven by the gate) |
| **rusty_json_turbo** | the JSON layer under every public API (serde_json, forked and made fast; lib name `serde_json`) | git, `version = "0.1.0"`, `default-features = false`, `features = ["alloc"]`; not on crates.io yet; pushed 2026-09-09 | **builds `no_std + alloc`** on both targets | `rusty_rtos_json` stays the coreJSON API (zero-allocation validator + `JSON_Search`, no `alloc`, `forbid(unsafe)`), which serde_json's `Value`/boxed-error model cannot be; typed (de)serialization under `alloc` goes through `rusty_json_turbo` behind a `serde` feature (K7). The fleet tool takes it the day it prints JSON | **ready**; one job, one parser each (decision log) |
| **rusty_zstd** | compression (compress, then encrypt, separate frames) | `=0.2.3`; `no_std + alloc` via `default-features = false, features = ["alloc"]` | **fails on both 32-bit targets**: four `core::sync::atomic::AtomicU64` sites; host and wasm build | host: the fleet tool for trace storage (`oracle/traces`, tens of MB at K1's 100 000 ticks); chip: OTA payloads and trace capture, after the fix | **host-ready, chip-blocked upstream** — `docs/upstream/rusty_zstd-atomic-u64.md` |
| **rusty_time** (`rusty_time-core`) | the house clock discipline (chrony remake, NTPv4 + NTS) | `=0.1.10` (crates.io: `rusty_time-core` / `-api` / `-clock`) | **fails**: `std`-only despite the `no-std` category | `rusty_rtos_sntp` (K7) wanted a `no_std` leaf (packet codec + offset arithmetic); until it exists, coreSNTP is remade (plan §5.5 item 4) | **not usable on a chip yet** — `docs/upstream/rusty_time-no-std-leaf.md` |
| **rusty_erasure** (`rusty_erasure-core`) | erasure shards for placement (SpaceDB's `ShardStore`) | `=0.4.0` | **fails**: imports `AtomicU64` in its `no_std` build | nothing in Kairos v1; the mesh is above the RTOS | **not applicable in v1**; gap recorded — `docs/upstream/rusty_erasure-atomic-u64.md` |
| **rusty_xml** | XML (libxml2 remake) | `=0.8.1` | **fails**: `std`-only (`rusty_xml-sax`, `-tree`) | nothing: the FreeRTOS portfolio speaks JSON, MQTT, HTTP; no XML anywhere in it | **not applicable** |
| **SpaceDB** (`spacedb-sdk`) | all storage: per-entry, encrypted, own compound key; the four deploy seams | `=0.6.0`, `default-features = false` for a library | host only; closure resolves clean of C, `ring`, `aws-lc-sys` | nothing in v1: a kernel persists no user data; `rusty_rtos_fat` (post-1.0, K9) is where storage on a chip appears and where SpaceDB's `ShardStore`/`Transport` seams would meet it; the umbrella's ledgers and traces are files in git by design (claims discipline) | **not applicable in v1**, resolves clean |
| **FFAI** (`ffai-core`) | AI: OCR, ASR/TTS, detection, VLM on candle | `=0.7.1` (rust 1.95) | host only (candle-core in the closure); clean of C | nothing: AI runs above the RTOS; on-edge vision is a Janus matter (`diana`, `argus` on the S3) | **not applicable** |
| **remade_ffmpeg_rs** (`rff`) | media probe / transcode, by git URL only | git (workspace 0.2.1); never the crates.io `rff` | host only; not compiled here (a large clone, nothing to link) | nothing: media is `rusty_esp_video`'s, above the kernel | **not applicable** |
| **rusty_maps** | maps UI (tiles, MVT) | private git repo, `0.0.0`, rust 1.98; not on crates.io | not gated (UI, private) | nothing | **not applicable**; consumable by git URL with the org token when a hosted UI wants it |
| **Deputy** | supply chain: acquire, scan, promote, gate | `deputy 0.4.0` installed on the box | `deputy discover` reads every package's `Cargo.lock` (core: 15 pinned crates); `acquire`/`scan`/`gate` need `DEPUTY_PASSPHRASE` and a vault | the gate step before a public flip (market-ready bar) | **installed and working**; the vault is the owner's |

Also in the requirements and settled without a crate: **mID** (identity) has
no place in a kernel; **RustCrypto / rustls** enter with `rusty_rtos_tcp` and
the NTS work (K7); the **Oxi\*** replacements have nothing to replace here;
**Dioxus** is not a firmware framework.

## The four imposters

Four crates.io names look like house crates and are not. Every Kairos
`deny.toml` (template and all eight packages) bans them by name, and the
compile gate proved the ban fires:

| crates.io name | what it actually is | the house crate |
|---|---|---|
| `rusty_time` 1.1.0 | cleancut's timer crate | `rusty_time-core` / `-api` / `-clock` |
| `rff` 0.3.0 | a fuzzy finder | `remade_ffmpeg_rs` by git URL (its lib is `rff`) |
| `thoth` 0.1.10 | a GraphQL client | the house `thoth` by git tag `v0.3.0`, or `rusty_symbols` / `rusty_tokens` / `rusty_a11y` |
| `spacedb` 0.1.4 | spacesprotocol's | `spacedb-sdk` |

## Rules the gate taught

- **Every git dependency carries a `version`** beside its URL, or
  `wildcards = "deny"` fails the build. Sibling deps already do; house git
  deps do the same.
- **`allow-git` lists the exact URL form the manifest uses** (no `.git`
  suffix when the manifest has none) and only URLs the graph contains; an
  unused entry is a warning on every run. The policy therefore names the
  sibling only; a house git dep adds its URL the day it is added.
- **A library opts out of the allocator**: `thoth` and `rusty_symbols` pull
  `rusty_alloc` by default; `default-features = false` in every library,
  the deliverable opts in through `rusty_rtos_alloc`.
- **`no_std` on a label is not `no_std` on a Cortex-M4.** Three house crates
  wear the `no-std` category and need `std` or 64-bit atomics. The gate, not
  the category, is the claim.
- **Licences in the house graph:** MIT, Apache-2.0, Unlicense (`memchr`),
  Unicode-3.0 (`unicode-ident`) — all allowed by the Kairos policy;
  `cargo deny check` passes over the whole no_std graph.

## What changed in the family because of this page (2026-09-09)

- `rusty_rtos_alloc` gained the `small-metal` feature (the firmware half)
  and a CI rung on the four bare-metal targets under rusty_alloc's two cfgs.
- `KAIROS.toml` gained `cfgs` per package and `kairos check` a feature-aware
  `no_std` ladder (`core`, then `alloc` and `small-metal` where declared).
- Every `deny.toml` bans the four imposters.
- `rusty_rtos_json`'s plan records its relation to `rusty_json_turbo`.
- Three upstream issue drafts in `docs/upstream/`, listed in the mission
  plan's upstream table; filing them is the owner's call.
- The mission plan's decision log carries the verdicts; `tools/house-gate`
  re-takes them in one command.
