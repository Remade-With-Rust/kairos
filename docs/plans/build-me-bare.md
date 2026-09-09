# Build me bare — everything Kairos wants on bare metal that is not there today

Written 2026-09-09. Companion to `rtos-mission.md` (the family plan) and
`../HOUSE-STACK.md` (the compile-gated verdicts this plan acts on). Family
work stays in the family plan; this document is the queue of **house crates
that Kairos wants on a Cortex-M4F / RV32 part and that do not build there
today**, plus the one Kairos seam that compiles for bare metal but has not
run on a board. Every row was measured, not read: `tools/house-gate`,
rustc 1.98.0, `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`.

## 0. Mission and promise

**Every house crate Kairos consumes on a chip builds `no_std` on the Kairos
targets at a released pin, proven by a gate rung, never by a fork.**

The one-line test for every change here: *does `cargo check -p gate-<crate>
--target thumbv7em-none-eabihf` in `tools/house-gate` go from FAIL to OK at
a pin that `cargo info` can see?* A fix that lives on a branch, a `[patch]`,
or a vendored copy is not done.

## 1. Where we are (2026-09-09)

| crate | pin measured | Kairos wants it for | on `thumbv7em` / `riscv32imac` today | what fails | size of the fix |
|---|---|---|---|---|---|
| `rusty_zstd` | `=0.2.5` (was `=0.2.3`) | on-chip OTA payloads and trace capture (K6+); the fleet tool already uses it on the host | **PASS at 0.2.5** (2026-09-09) | was: eight `core::sync::atomic::AtomicU64` **census counters** (`bit.rs` `RELOAD_CALLS`, `RELOAD_REFILLS`; `compressed.rs` `DEC_LIT32`, `DEC_MATCH32`, `DEC_LIT16`, `DEC_LIT64`, `DEC_MATCH16`, `DEC_BAND[..]`). Fixed upstream | none left |
| `rusty_time-core` | `=0.1.10` | `rusty_rtos_sntp` (K7): the NTP packet codec and offset arithmetic as the house leaf instead of a coreSNTP remake | **FAIL** | no `#![no_std]`; `ntp.rs` itself needs only `std::error::Error` (one impl, line 159) and `f64` division — no `libm`; the rest of the crate (`filter`, `discipline`, `server`, `config`) uses `Vec`/`String` and float math and stays `std` | small for the leaf, large for the whole crate — so: the leaf |
| `rusty_alloc` (`rusty_rtos_alloc` `small-metal`) | `=2.0.4` | the firmware's Rust global allocator behind `heap_3` (K4) | **OK to compile** under `--cfg ra_single_threaded --cfg ra_small_profile`; **no board has run it** on Cortex-M or RV32 (the S3 is the only measured part) | nothing at compile time; `REGION_ALIGN` and a public `PrimError` are not in 2.0.4, so the seam re-exports 2.0.4's surface only | a board day (K4) + one upstream release |
| `rusty_erasure-core` | `=0.4.0` | nothing in v1 (the mesh is above the RTOS); recorded because its `no-std` label is wrong on 32-bit parts | **FAIL** | `kernel.rs:10` imports `AtomicU64` for `SCALAR_CENSUS_BYTES` and a `census: &'static AtomicU64` field — a census counter again | small |
| `rusty_xml` | `=0.8.1` | nothing: no XML in the FreeRTOS portfolio | **FAIL** (`std`-only `-sax`, `-tree`) | not a Kairos want; listed so nobody reads the `no-std` category as a fact | not ours to schedule |

Ready today and not in this plan: `rusty_alloc-api` (compiles, both halves),
`rusty_symbols`, `thoth` v0.3.0, `rusty_json_turbo` (`no_std + alloc`) — all
proven on both targets by the gate.

## 2. Strategy that does not change

1. **Fixes land upstream, in the crate's own repository, through its own
   plan.** Kairos never carries a `[patch]`, a `paths` override, a fork or a
   vendored copy of a house crate. The standing rule for Janus repos ("touched
   only through their own plans") is the rule for every house crate.
2. **The pin moves in one commit with the verdict.** When a release lands,
   `tools/house-gate/Cargo.toml` moves the pin, the gate rung turns OK, and
   `HOUSE-STACK.md` records the new verdict — one commit, one row.
3. **A census counter is not a reason to lose a target.** The pattern that
   breaks both `rusty_zstd` and `rusty_erasure-core` is the codec-skill
   census (`skill-codec-analyzer`): 64-bit `static` counters bumped on hot
   paths. On a target without 64-bit atomics the counter either becomes
   `portable_atomic::AtomicU64` (a critical-section fallback; pure Rust;
   already in the house graph through `rusty_alloc`'s `std` feature) or
   compiles out behind `#[cfg(target_has_atomic = "64")]`. The second is the
   house's own "measure before you claim" ethic applied to itself: a census
   nobody can read on that target should not cost the target.
4. **A leaf, not a lift.** `rusty_time-core` does not need to become `no_std`
   whole; `rusty_rtos_sntp` needs `ntp.rs` (`NtpPacket::{parse, write,
   to_bytes, client_request}`, `NtpTimestamp`, `NtpShort`, `offset_delay`,
   `extension_fields`). The upstream change is `#![cfg_attr(not(feature =
   "std"), no_std)]` on the crate, the `std::error::Error` impl behind
   `feature = "std"`, and the other modules behind the same feature.
5. **"Ready" means compiled at a released pin under the Kairos policy on the
   Kairos targets; "bare" means a ledger row from a board.** The mission
   plan's K3 puts the first Kairos code on QEMU and a C6; K4 puts the seam's
   `Region` on the M3 cell. Those rows are the only thing that upgrades a
   "compiles" here to "runs".
6. **Filing is the owner's act.** Issue and PR drafts are prepared in
   `docs/upstream/` with the reproduction and the fix; the owner files them.
   Nothing in this plan is executed against a public house repository by a
   session on its own.

## 3. The bricks, in order

| # | brick | where the work is | kill test | unblocks |
|---|---|---|---|---|
| B1 | ~~`rusty_zstd`: the eight census counters behind `target_has_atomic = "64"`~~ — **closed 2026-09-09, fixed upstream in 0.2.5.** `gate-zstd` at `=0.2.5` exits 0 on `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`; the draft was never filed and is now moot | rusty_zstd (draft: `docs/upstream/rusty_zstd-atomic-u64.md`, resolved) | met | on-chip compression is unblocked; `rusty_rtos_demo`'s trace capture on a board (K3) can store compressed |
| B2 | `rusty_time-core`: `no_std` leaf — `cfg_attr(not(std), no_std)`, `ntp.rs` unconditional, `filter`/`discipline`/`select`/`server`/`client`/`config`/`refclock`/`vclock` behind `std`; a CI rung with `--no-default-features`; release 0.1.11 | rusty_time (draft: `docs/upstream/rusty_time-no-std-leaf.md`) | `gate-time` OK on both targets at `=0.1.11`; `NtpPacket::parse` of a 48-byte request round-trips `to_bytes` in a `no_std` test | `rusty_rtos_sntp` as a wrapper over the leaf (mission plan §5.5 item 4 flips to "the leaf"); until then the coreSNTP remake |
| B3 | `rusty_alloc` 2.0.5: `REGION_ALIGN` and a public region error type, so `rusty_rtos_alloc::small_metal` re-exports the whole recipe the README documents | rusty_alloc (a doc-only ask; the code is on `main` already) | the seam's re-export list has no "arrives with the next release" comment | the seam's docs and the firmware template |
| B4 | The seam on a board: `Region<{ good_region_size(...) }>` given once in the M3-QEMU cell and on the C6, `heap_3` over it, the K4 allocation-latency row against the C `heap_4` | Kairos (K4; family plan) | the mission plan's K4 kill test | `rusty_alloc` "measured on Cortex-M" (the mission plan's upstream row closes) |
| B5 | `rusty_erasure-core`: the census counter behind `target_has_atomic = "64"`; a CI rung; release 0.4.1 | rusty_erasure (draft: `docs/upstream/rusty_erasure-atomic-u64.md`) | `gate-erasure` OK on both targets | nothing in Kairos v1; the org's `no-std` label becomes true |

Not a brick: `rusty_xml`. No Kairos package will ever parse XML; the gate
keeps the row so the category is not mistaken for a fact.

## 4. Finished work

### B1 closed -- `rusty_zstd` builds on both Kairos targets (released 0.2.5)

**The kill test passes**, which is the only thing that counts here:

```text
cd tools/house-gate
cargo check -p gate-zstd --target thumbv7em-none-eabihf         # exit 0
cargo check -p gate-zstd --target riscv32imac-unknown-none-elf  # exit 0

Cargo.lock: rusty_zstd 0.2.5
  source   = registry+https://github.com/rust-lang/crates.io-index
  checksum = cc2318a1441e10cf436f0bf98615216b0a5410b6f546548b50faa170c7b4a1a6
```

A released pin from the registry, with a checksum. No fork, no `[patch]`, no
vendored copy: strategy item 1 held.

**The plan under-counted the work, and was wrong in a useful way.** It said
"eight statics ... small: the counters are the only sites", read off a rustc
report that caps its output. The real figure is **204 errors** from **427
fully-qualified uses plus six imports across sixteen files**. The conclusion
survived and got stronger: at that scale a hundred `#[cfg]`s would have been
unmaintainable, so upstream took the other option this plan offered and
routed every counter through ONE seam, `census64`.

- Where 64-bit atomics exist it is `pub use core::sync::atomic::AtomicU64`,
  the same type rather than a wrapper. Proved, not asserted: the codec's own
  asm board is identical in all thirty-two columns and its byte-identity
  gates are unchanged, so the hosted build paid nothing for our target.
- Where they do not, a zero-sized stub, and the statics leave BSS entirely.

Upstream chose the stub over `portable-atomic` for the reason strategy item 3
implies: that crate needs a critical-section implementation on a core with no
64-bit atomic instruction, and a library enabling it would conscript every
downstream firmware's interrupt policy for a diagnostic counter. The seam is
one type wide, so `rusty_rtos_*` can supply `portable_atomic::AtomicU64` if a
Kairos build ever wants the census on-chip.

**A zero from a census counter on these parts means "not measurable on this
target", never "measured zero"**: `rusty_zstd::census64::CENSUS_LIVE` is
`false` there and says so.

Also landed upstream: a CI rung on both targets (`--no-default-features
--features alloc`) so it cannot rot, and the README's platform table.

### And it RAN, which strategy item 5 distinguishes from compiling

"'Bare' means a ledger row from a board." Here is one, on the ESP32-S3 on
this desk. Xtensa LX7 is also 32-bit and also has no 64-bit atomics, so it is
the same stub configuration:

```text
census64::CENSUS_LIVE = false
source            8260 bytes
L1  8260 ->  468 bytes  (17.65x)  round trip OK
L3  8260 ->  426 bytes  (19.39x)  round trip OK
L5  8260 ->  263 bytes  (31.41x)  round trip OK
RESULT: PASS -- compressed and decompressed on the board
```

Three levels because they are three different match finders (Fast, DFast,
Greedy); each output was decompressed and compared byte for byte on the part.
192 KiB heap, `no_std + alloc`. The firmware lives upstream at
`bare-metal/esp32s3`, hand-run because it needs Espressif's Rust fork which
no CI has. It claims no timing and no heap floor.

**This is the S3, not a Kairos target.** It makes the stub configuration real
on silicon; B4's Cortex-M row is still owed and still rides K4.

## 5. Remaining work and owner steps

- ~~**Owner:** file B1~~ **done 2026-09-09**: fixed upstream in its own
  repository, released as `rusty_zstd` 0.2.5, pin moved here, both gate rungs
  green. The `docs/upstream/` draft was never needed.
- **Owner:** file B2 and B5 from the drafts (or hand them to the crates' own
  plans); cut the releases; move the pins here in one commit each.
- **Owner:** B3 is a release of what is already on `rusty_alloc`'s `main`.
- **Kairos:** B4 rides K4; nothing to do until K3 has a QEMU cell.
- **Kairos:** when B2 lands, `rusty_rtos_sntp`'s package plan opens with
  "WRAP the leaf" instead of "REMAKE coreSNTP", and the mission plan's §5.5
  item 4 is decided by that ledger row.

## 6. Kill tests

| brick | a stranger runs | passes when |
|---|---|---|
| B1 | `cd tools/house-gate && cargo check -p gate-zstd --target thumbv7em-none-eabihf` | **met**: exit 0 at `=0.2.5`, and on `riscv32imac-unknown-none-elf` too |
| B2 | `cd tools/house-gate && cargo check -p gate-time --target riscv32imac-unknown-none-elf` | exit 0 at a crates.io pin |
| B3 | `grep -c "arrives with the next" rusty_rtos_core/crates/rusty_rtos_alloc/src/lib.rs` | prints 0 |
| B4 | the mission plan's K4 row | a cycle count from the M3 cell in `rusty_rtos_heap/docs/LEDGER.md` |
| B5 | `cd tools/house-gate && cargo check -p gate-erasure --target thumbv7em-none-eabihf` | exit 0 at a crates.io pin |

## 7. Decision log

| date | decision |
|---|---|
| 2026-09-09 | This plan exists because a `no-std` category on crates.io is not a claim: three house crates carry it and fail on a Cortex-M4F. The gate is the claim; this plan is the queue from FAIL to OK. |
| 2026-09-09 | Census counters are the whole reason two codecs lose 32-bit bare metal; the fix is to gate or port the counters, never to drop the census (the codec skills depend on it) and never to fork the crate. |
| 2026-09-09 | `rusty_time` is consumed as a leaf (`ntp.rs`), not lifted whole; the SNTP package wraps the leaf when it exists and remakes coreSNTP until then. |
| 2026-09-09 | Nothing here is executed against a public house repository by a session; drafts in `docs/upstream/` are the hand-off, filing is the owner's. |
| 2026-09-09 | **B1 closed.** Fixed in `rusty_zstd` itself and released as 0.2.5; the pin here moved in the same act and both rungs are green. The plan's estimate ("eight statics, small") was low by two orders of magnitude -- 427 uses across sixteen files -- because it was read off a rustc report that CAPS its output. **Count the sites before sizing a fix from an error list.** The conclusion held regardless, and at that scale the one-seam option was the only maintainable one. |
| 2026-09-09 | The stub was chosen over `portable-atomic` so a codec does not conscript a downstream firmware's interrupt policy for the sake of a diagnostic counter. The seam is one type wide if Kairos ever wants the census on-chip. |
| 2026-09-09 | B1 also produced the plan's first board row (an ESP32-S3 round trip), which is strategy item 5's "bare" for the STUB configuration -- but not for a Kairos part. B4 still owes the Cortex-M row. |

## 8. Appendix — the exact failures

```text
gate-zstd    @ thumbv7em-none-eabihf:      [HISTORICAL -- fixed in 0.2.5, now exit 0]
gate-zstd    @ riscv32imac-unknown-none-elf: same [HISTORICAL -- now exit 0]
             the "x4 sites reported, 8 statics" above was rustc's CAP, not the count:
             204 errors, 427 uses, 16 files.
gate-erasure @ thumbv7em-none-eabihf:      error[E0432]: unresolved import `core::sync::atomic::AtomicU64`  (kernel.rs:10)
gate-time    @ thumbv7em-none-eabihf:      error[E0463]: can't find crate for `std` — `rusty_time_core` does not declare `#![no_std]`
gate-xml     @ thumbv7em-none-eabihf:      error[E0463]: can't find crate for `std` — `rusty_xml_sax` / `rusty_xml_tree`
rusty_rtos_alloc --no-default-features without the cfgs: rusty_alloc's compile_error! ("assumes a SINGLE THREAD ... opt in with --cfg ra_single_threaded") — by design, not a bug
```

The gate: `tools/house-gate/README.md`. The verdict table: `docs/HOUSE-STACK.md`.
