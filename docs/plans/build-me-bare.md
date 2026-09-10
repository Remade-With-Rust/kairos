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

## 1. Where we are (2026-09-09, re-measured 2026-09-10)

| crate | pin measured | Kairos wants it for | on `thumbv7em` / `riscv32imac` today | what fails | size of the fix |
|---|---|---|---|---|---|
| `rusty_zstd` | `=0.2.5` (was `=0.2.3`) | on-chip OTA payloads and trace capture (K6+); the fleet tool already uses it on the host | **PASS at 0.2.5** (2026-09-09) | was: eight `core::sync::atomic::AtomicU64` **census counters** (`bit.rs` `RELOAD_CALLS`, `RELOAD_REFILLS`; `compressed.rs` `DEC_LIT32`, `DEC_MATCH32`, `DEC_LIT16`, `DEC_LIT64`, `DEC_MATCH16`, `DEC_BAND[..]`). Fixed upstream | none left |
| `rusty_time-core` | `=0.2.0` (was `=0.1.10`) | `rusty_rtos_sntp` (K7): the NTP packet codec and offset arithmetic as the house leaf instead of a coreSNTP remake | **PASS at 0.2.0** (2026-09-09) | was: no `#![no_std]`; `ntp.rs` needed only `std::error::Error` (one impl, line 159) and `f64` division — no `libm`. Fixed upstream, and the impl became `core::error::Error` rather than being gated away | none left |
| `rusty_alloc` (`rusty_rtos_alloc` `small-metal`) | `=2.1.0` (was `=2.0.4`) | the firmware's Rust global allocator behind `heap_3` (K4) | **OK to compile** on all four targets under `--cfg ra_single_threaded --cfg ra_small_profile`; **no board has run it on a Kairos part** — an ESP32-S3 has (section 4, 9/9), which is not one | nothing: 2.1.0 added `REGION_ALIGN` and the public `PrimError` the seam was missing, and the seam re-exports them | a board day (K4) — **B3 closed 2026-09-10** |
| `rusty_erasure-core` | `=0.4.1` (was `=0.4.0`) | nothing in v1 (the mesh is above the RTOS); recorded because its `no-std` label was wrong on 32-bit parts | **PASS at 0.4.1** (2026-09-10) | was: `kernel.rs:10` imported `AtomicU64` for `SCALAR_CENSUS_BYTES` and a `census: &'static AtomicU64` field — the census counter again. Fixed upstream | none left |
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
| B2 | ~~`rusty_time-core`: `no_std` leaf~~ — **closed 2026-09-09, fixed upstream in 0.2.0.** `gate-time` at `=0.2.0` exits 0 on both targets; the draft was never filed and is now moot. Original scope: — `cfg_attr(not(std), no_std)`, `ntp.rs` unconditional, `filter`/`discipline`/`select`/`server`/`client`/`config`/`refclock`/`vclock` behind `std`; a CI rung with `--no-default-features`; release 0.2.0 | rusty_time (draft: `docs/upstream/rusty_time-no-std-leaf.md`) | `gate-time` OK on both targets at `=0.2.0`; `NtpPacket::parse` of a 48-byte request round-trips `to_bytes` in a `no_std` test — **both met upstream** (the round trip is `client_request_round_trips_in_48_bytes`, run on the host `no_std` arm AND on the S3); only the registry pin is outstanding | `rusty_rtos_sntp` as a wrapper over the leaf (mission plan §5.5 item 4 flips to "the leaf"); until then the coreSNTP remake |
| B3 | ~~`rusty_alloc` 2.0.5: `REGION_ALIGN` and a public region error type~~ — **closed 2026-09-10, released as 2.1.0** (not the 2.0.5 this plan named). The seam pins `=2.1.0` and re-exports `REGION_ALIGN`, `PrimError` and the four `FERR_*` codes beside the region API it already had | rusty_alloc | met — see section 6 | the seam's docs and the firmware template: the documented `HEAP.give()?` is now **writable**, which it was not while the error type had no name |
| B4 | The seam on a board: `Region<{ good_region_size(...) }>` given once in the M3-QEMU cell and on the C6, `heap_3` over it, the K4 allocation-latency row against the C `heap_4` | Kairos (K4; family plan) | the mission plan's K4 kill test | `rusty_alloc` "measured on Cortex-M" (the mission plan's upstream row closes) |
| B5 | ~~`rusty_erasure-core`: the census counter behind `target_has_atomic = "64"`~~ — **closed 2026-09-10, fixed upstream in 0.4.1**, which is the release this plan asked for by name. `gate-erasure` at `=0.4.1` exits 0 on both targets, a genuine **FAIL → OK**: the rung was still failing at `=0.4.0` in the same session, minutes earlier | rusty_erasure (draft: `docs/upstream/rusty_erasure-atomic-u64.md`, resolved) | met | nothing in Kairos v1; the org's `no-std` label is now true |

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

### B2 closed -- `rusty_time-core` has its `no_std` leaf (released 0.2.0)

**The kill test passes**, at a registry pin, which is the only thing that counts:

```text
cd tools/house-gate
cargo check -p gate-time --target thumbv7em-none-eabihf         # exit 0
cargo check -p gate-time --target riscv32imac-unknown-none-elf  # exit 0

Cargo.lock: rusty_time-core 0.2.0
  source   = registry+https://github.com/rust-lang/crates.io-index
  checksum = d019985f57b6328019a09a08644e6f6e4172a700885d966f834a976f72b9beb4
```

A released pin from the registry, with a checksum. No fork, no `[patch]`, no
vendored copy: strategy item 1 held. The pin moved in the same commit as this
verdict and `HOUSE-STACK.md`'s row: strategy item 2 held.

**Proof the gate can fail, which is the part that makes exit 0 mean anything.**
Both poisons were run against the fixed tree:

```text
remove #![cfg_attr(not(feature = "std"), no_std)]
  -> error[E0463]: can't find crate for `std`      <- appendix section 8, exactly
restore `impl std::error::Error for ParseError`
  -> error[E0433]: cannot find module or crate `std` in this scope
restored
  -> exit 0
```

**The plan's estimate was right this time, and its prescription was improved
on in one place.** Item 4 of the strategy asked for the `std::error::Error` impl
to go behind `feature = "std"`. Upstream used `core::error::Error` instead: the
two have been the same trait since Rust 1.81, so a hosted caller sees no change
at all, and a `no_std` caller *keeps* the impl rather than losing it. Gating it
would have cost `rusty_rtos_sntp` the ability to use `?` on a `ParseError`.

**The leaf is smaller than the plan assumed, in the direction that helps.** The
draft said "`no_std`, no `alloc`, no clock, no socket" as an aspiration; it is
literally true. `ntp.rs` allocates nothing, so `rusty_rtos_sntp` needs no
allocator seam to consume it -- unlike `rusty_zstd`, which needs `alloc`.

Also landed upstream, so it cannot rot:

- `default = ["std"]`; `filter`, `select`, `discipline`, `client`, `server`,
  `config`, `refclock` and `vclock` behind it. Every existing dependant takes
  default features and is unaffected (`cargo semver-checks` vs 0.1.10: 196
  checks, no semver update required).
- CI rungs on **both** Kairos targets with `--no-default-features`, plus the
  leaf's own suite run against the same `no_std` code path on the host
  (`cargo test -p rusty_time-core --no-default-features`) and clippy on that arm.
  The compile rung proves it builds; the test rung proves it is right.
- The three benches are `required-features = ["std"]` -- they drive the
  server/filter/client paths and cannot compile without it. Found by the
  `--all-targets` clippy rung, not by hand.

### And it RAN, on the same desk as B1

Strategy item 5 again. Same ESP32-S3, revision v0.2, 8 MB flash:

```text
=== rusty_time-core leaf on ESP32-S3 (xtensa, no_std, NO alloc) ===
[1] client request                    5 checks   ok
[2] server response (literal image)  14 checks   ok
[3] offset / delay (RFC 5905 section 8)   5 checks   ok
      offset 0.09999999997671694 s   delay 0.05000000004656613 s
      2 s across the 2036 era wrap measured as 2 s
[4] extension fields and rejections    6 checks   ok

checks passed 30 / 30
RESULT: PASS -- the rusty_time-core leaf ran on the board
```

Three things make this more than a smoke test, and they are the three a Kairos
reviewer should check for in any future board row:

- **The server response is a hand-written 48-byte wire image, not our own
  `to_bytes` output.** Parsing what the codec wrote would only prove the codec
  is self-consistent. A literal image pins the RFC 5905 field offsets and
  big-endian order independently of the writer, and the timestamps use
  distinguishable patterns (`0x3333...`, `0x5555...`, `0x7777...`) so a
  field-order slip cannot pass.
- **The arithmetic is soft-float on this part.** Xtensa LX7 has a
  single-precision FPU and no `f64` unit, so every division in `offset_delay`
  and `seconds_since` is a compiler-generated soft-float call. That is the half
  a host `cargo test` cannot stand in for, and it is why the era-wrap and
  sub-nanosecond cases were put on the board rather than left in the suite.
- **No heap is linked.** `build-std = ["core"]`, no `alloc`, no `esp-alloc`.

**Two numbers on that board look wrong and are not.** The 1 ns step reads
0.931 ns because `from_unix` truncates nanoseconds into the 32-bit NTP fraction
(`(1 << 32) / 1e9` is 4.29, stored as 4); root delay reads 0.0149993... for the
same reason in 16.16. Both are the wire format's resolution showing through, and
the checks are toleranced at one NTP tick (2^-32 s) accordingly -- tighter would
be testing the test. **A board row that quotes a suspiciously round number is
the one to distrust.**

**This is the S3, not a Kairos target** -- the same caveat B1 carries. It makes
the leaf real on silicon; the Cortex-M row is still owed and still rides K4.

### B3 closed -- the seam re-exports the whole recipe (released 2.1.0)

`rusty_alloc` 2.1.0 has both things the seam was missing, and the reason
they matter is in the crate docs rather than in a compile error: the
documented firmware recipe ends `let usable = HEAP.give()?;`, and `give`
answers `Result<usize, PrimError>`. **While `PrimError` had no name a
firmware could reach, that `?` could not be written.** The seam now
re-exports it and the four `FERR_*` codes it can carry — `PrimError` is a
`u32`, so the codes are the difference between "the region was refused"
and knowing which way.

```text
RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile" \
  cargo check -p rusty_rtos_alloc --no-default-features --features small-metal \
  --target thumbv7em-none-eabihf          # exit 0, and on the other three
```

**The re-export list is itself a gate, and it was made to fail** before the
pass was believed — section 7's rule from B2:

```text
add REGION_NOT_A_THING to the list
  -> error[E0432]: unresolved import `rusty_alloc::prim::fixed::REGION_NOT_A_THING`
restored
  -> exit 0
```

`VERSION` is asserted in the crate's own test (`the_pin_is_what_the_manifest_says`),
so the manifest pin and the printed constant cannot drift apart.

**B3's old kill test could not fail, and that is the finding worth keeping.**
It was `grep -c "arrives with the next" ... lib.rs` printing 0 — and it
printed 0 on the unfixed tree too, because the comment read "`REGION_ALIGN`
and a public error type arrive with / the next rusty_alloc release": wrong
verb form, and split across a line break so no single-line grep could match
it either way. It is replaced with the compile check above. **A kill test
that greps for prose is a kill test that rots with the prose**; every other
row in section 6 runs a compiler, and this one now does too.

### B5 closed -- `rusty_erasure-core` builds on both Kairos targets (released 0.4.1)

The release this plan asked for by name, and the cleanest evidence in the
document: the rung was **failing at `=0.4.0` in the same session, minutes
before** the pin moved, so this is a measured FAIL → OK rather than a pass
taken on a tree that was never seen to fail.

```text
cd tools/house-gate
cargo check -p gate-erasure --target thumbv7em-none-eabihf         # exit 0
cargo check -p gate-erasure --target riscv32imac-unknown-none-elf  # exit 0

Cargo.lock: rusty_erasure-core 0.4.1
  source   = registry+https://github.com/rust-lang/crates.io-index
  checksum = 61c237be51374ef0b897e4a9205966b9941ce89d063e6e5d63bf96d8611580dc
```

Registry pin, checksum, no fork and no `[patch]`: strategy item 1 held for
the third time. Nothing in Kairos v1 links it — the value is that the org's
`no-std` label is now true for the crate rather than aspirational.

### Two gaps in the gate itself, found by running all of it

**The `host/` and `hostgit/` rungs did not run at all.** Both are packages
inside `tools/house-gate/`, whose workspace declares
`members = ["gate-*"]` — which does not match them. Cargo therefore walked
up, found a workspace they were not members of, and refused:

```text
error: current package believes it's in a workspace when it's not
```

Each now carries its own empty `[workspace]` table, which is what the
README always said they were. They pass in 26 s and 40 s. **A rung nobody
runs is not a gate**, and this one had a README row claiming a pass it
could not have produced — the same species as B3's grep, one layer up.
`host/` was also still pinning `rusty_time-core = "=0.1.10"` and
`rusty_erasure = "=0.4.0"`, the two pre-fix versions, because nothing had
been able to compile it since those bricks closed.

**`host` fails `cargo deny check advisories`, and the gate workspace does
not.** Three `unmaintained` advisories, no vulnerabilities:

| advisory | crate | arrives through |
|---|---|---|
| RUSTSEC-2023-0089 | `atomic-polyfill` 1.0.3 | `heapless` 0.7 -> `postcard` -> `spacedb-access`/`-meter` -> **spacedb-sdk** |
| RUSTSEC-2026-0215 | `smallstr` 0.3.1 | `yrs` -> `spacedb-crdt` -> **spacedb-sdk** |
| RUSTSEC-2024-0436 | `paste` 1.0.15 | `gemm` -> `candle-core` -> **ffai-core** |

None is in a bare-metal graph, which is exactly why `cargo deny check` in
the gate workspace is clean and this one is not — the split is doing its
job. Two are one bump: `heapless` 0.8 replaced `atomic-polyfill` with
**`portable-atomic`**, the same crate strategy item 3 already names as the
house's answer for atomics a target lacks. Not Kairos's to schedule; owner
steps for `spacedb-sdk` and `ffai-core`.

### And the seam RAN, which is the first time it has

Strategy item 5 draws the line at "'bare' means a ledger row from a board",
and until today the allocator seam had none — `HOUSE-STACK.md` said of it,
in as many words, "**no board has run it**". Now one has:

```text
=== rusty_rtos_alloc small-metal seam on ESP32-S3 (xtensa, no_std + alloc) ===
rusty_alloc      2.1.0
REGION_ALIGN     16 bytes
MIN_REGION       65536 bytes
budget asked     225280 bytes
region reserved  196608 bytes        <- three whole 64 KiB segments
give() -> 196608 bytes usable
second give -> FERR_REGISTERED
64 rounds of 32768 bytes = 2097152 total
  served from the region: 64/64
  landed on the first block's address again: 63/63
checks passed 9 / 9
RESULT: PASS -- the seam gave, served and reclaimed on the board
```

ESP32-S3 revision v0.2, MAC `68:ee:8f:51:74:64` — the same desk and the
same part as B1 and B2. `firmware/esp32s3-devkit-region` in
`rusty_rtos_core`, following that repo's `firmware/README.md` convention.

`good_region_size(220 KiB)` answering **196,608** is the seam's own crate
docs — "three whole 64 KiB segments" — reproduced on silicon rather than
asserted, and the second `give` answering `FERR_REGISTERED` exercises the
error surface B3 landed hours earlier.

**Two checks carry this row; the rest is arithmetic a host could do.**

- **`region_contains` on every allocation's address.** "The allocation
  succeeded" proves nothing: a global allocator quietly falling back to
  something else prints exactly that. Each address is tested against the
  base and length the region registered when it was given.
- **64 rounds of a 32 KiB block against a 192 KiB region** — two megabytes
  through a region holding a tenth of it, so a heap reclaiming nothing
  would be exhausted on the sixth round. All 64 served, and 63 of 63 land
  on the first block's address.

**The first version of that second check could not fail, and saying so is
the point.** It read `region_stats()` and asserted
`free_after >= free_before`; it passed with `65536 == 65536`, because that
field counts extents the allocator has not been handed yet, so dropping a
`Box` never moves it. That is the same defect as B3's grep — a green check
that was green by construction — found on the same day, in my own work,
one section later. It is replaced by the rounds above and the counter is
printed but labelled.

**This is the S3, not a Kairos target**, the same caveat B1 and B2 carry,
and it claims **no timing**: the cycle count is K4's and belongs on a
Kairos part beside the C `heap_4`. **B4 is untouched by this row.** What
changes is that the seam is no longer un-run: the fixed-region backend,
`give`, the error codes and reclamation are all real on 32-bit silicon.

Also measured getting there, and worth having written down: the seam
**compiles for `xtensa-esp32s3-none-elf`** under the two cfgs with
`-Z build-std=core,alloc`. The house's ESP notes say "rusty_alloc has no
Xtensa port by default", which is true of its default segment path and not
of `prim::fixed`.

## 5. Remaining work and owner steps

- ~~**Owner:** file B1~~ **done 2026-09-09**: fixed upstream in its own
  repository, released as `rusty_zstd` 0.2.5, pin moved here, both gate rungs
  green. The `docs/upstream/` draft was never needed.
- ~~**Owner:** file B2~~ **done 2026-09-09**: fixed upstream in its own
  repository, released as `rusty_time-core` 0.2.0 (breaking, so a minor bump
  and not the 0.1.11 this plan asked for), pin moved here, both gate rungs
  green. The `docs/upstream/` draft was never needed.
- ~~**Owner:** file B5~~ **done 2026-09-10**: fixed upstream and released as
  `rusty_erasure-core` 0.4.1 — the version this plan named — pin moved here,
  both rungs green from the registry. The `docs/upstream/` draft was never
  needed, which is now three for three.
- ~~**Owner:** B3~~ **done 2026-09-10**: released as `rusty_alloc` **2.1.0**,
  not the 2.0.5 this plan asked for. Pin moved, the seam's re-export list
  completed, all four targets green, and the list poison-tested.
- **Kairos:** B4 is the ONLY open brick, and no upstream release can close
  it. It rides K4, and K4 rides K3's QEMU cell — `qemu-system-arm` is not
  installed on this box, which is the first concrete step. Note that B4
  needs **no hardware**: its kill test is a cycle count from the M3 QEMU
  cell, so the blocker is the cell, not a part. The S3 row above is as
  close as an Xtensa board can get and is explicitly not it.
- **Kairos: B2's consequence is now live.** `rusty_rtos_sntp`'s package plan
  opens with "WRAP the leaf" instead of "REMAKE coreSNTP", and the mission
  plan's §5.5 item 4 is decided by this ledger row. The wrapper needs **no
  allocator seam**: the leaf allocates nothing, so it is a thinner dependency
  than `rusty_zstd` (which needs `alloc`). That is the next Kairos step here.

## 6. Kill tests

| brick | a stranger runs | passes when |
|---|---|---|
| B1 | `cd tools/house-gate && cargo check -p gate-zstd --target thumbv7em-none-eabihf` | **met**: exit 0 at `=0.2.5`, and on `riscv32imac-unknown-none-elf` too |
| B2 | `cd tools/house-gate && cargo check -p gate-time --target riscv32imac-unknown-none-elf` | **met**: exit 0 at `=0.2.0`, and on `thumbv7em-none-eabihf` too |
| B3 | `cd rusty_rtos_core && RUSTFLAGS="--cfg ra_single_threaded --cfg ra_small_profile" cargo check -p rusty_rtos_alloc --no-default-features --features small-metal --target thumbv7em-none-eabihf` | **met**: exit 0 at `=2.1.0` with `REGION_ALIGN` and `PrimError` in the re-export list, and on all four targets |
| B4 | the mission plan's K4 row | a cycle count from the M3 cell in `rusty_rtos_heap/docs/LEDGER.md` |
| B5 | `cd tools/house-gate && cargo check -p gate-erasure --target thumbv7em-none-eabihf` | **met**: exit 0 at `=0.4.1` from the registry with a checksum, and on `riscv32imac-unknown-none-elf` too |
| **the gate itself** | `sh tools/house-gate/run.sh` | exit 0. This row exists because the four above can all pass while a rung nobody runs is broken: on 2026-09-10 `host/` and `hostgit/` had been unbuildable for a day while the README recorded times for them. The script **discovers** `gate-*` off the filesystem rather than listing them — the README's loop named seven of the eight that exist — and it declares `gate-xml` as an EXPECTED failure, so a rung that starts passing when it should not is red too. Poison-tested both ways: exit 1 when an expectation is wrong, exit 0 when it is right |

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
| 2026-09-09 | **B2 built, not closed.** The leaf, both CI rungs and a 30/30 board row are upstream on `rusty_time`; the pin here stays `=0.1.10` until 0.2.0 is on crates.io, because strategy item 2 says the verdict IS the released pin. A reproduction that needs a path override is a measurement, not a verdict -- and saying so is the difference between this row and a fork. **Discharged the same day: 0.2.0 published, pin moved, row closed.** |
| 2026-09-09 | Strategy item 4 asked for the `std::error::Error` impl to go **behind** `feature = "std"`; upstream used **`core::error::Error`** instead. Same trait since Rust 1.81, so hosted callers see nothing, and `no_std` callers KEEP the impl instead of losing it -- `rusty_rtos_sntp` can use `?` on a `ParseError`. **When a plan prescribes a gate, check whether the thing can simply move to `core`.** |
| 2026-09-09 | The leaf needs **no `alloc`**, which the draft hoped for and did not verify. So `rusty_rtos_sntp` wraps it with no allocator seam at all -- a thinner dependency than `rusty_zstd`, and the reason the S3 firmware for it links no heap. |
| 2026-09-09 | **A gate is not a gate until it has been made to fail.** Both B2 poisons were run (drop the `cfg_attr` -> the appendix's exact `E0463`; restore `std::error::Error` -> `E0433`), then restored to exit 0. Every future row in this plan should carry its poison, not just its pass -- `gate-*` crates are three lines long and a green three-line crate is exactly the shape that can be green for the wrong reason. |
| 2026-09-09 | The B2 board row parses a **hand-written 48-byte wire image**, not the codec's own `to_bytes` output, and runs the offset arithmetic where `f64` is soft-float. Parsing your own output tests self-consistency; parsing a literal image tests the RFC field offsets. **A board row that only round-trips its own encoder has not tested the wire.** |
| 2026-09-09 | **The B2 release is 0.2.0, not the 0.1.11 this plan asked for.** Gating the `std`-only modules narrows what `default-features = false` returns, and under Cargo's 0.x rules `0.1.10` and `0.1.11` are COMPATIBLE -- so a consumer on `rusty_time-core = "0.1"` with `default-features = false` (which is exactly what `gate-time` writes) would have been handed the narrower crate by the next `cargo update`, with no version change to notice. A minor bump makes the one breaking arm an explicit choice. **When a fix adds a feature gate, ask which spelling of the dependency gets NARROWER, not just which gets wider.** |
| 2026-09-10 | **Audited the whole document by running it, and found the appendix contradicting the plan**: section 8 still showed `gate-erasure`'s `AtomicU64` error as a live failure while sections 1, 3 and 6 all recorded B5 closed. Every other fixed row had been marked `[HISTORICAL]` and that one was missed. **When a brick closes, grep the whole file for its failure text, not just its row.** |
| 2026-09-10 | **The gate got a kill test of its own** (`tools/house-gate/run.sh`, exit 0). Per-brick kill tests cannot catch a rung nobody runs, which is what `host/` and `hostgit/` were for a day. The script discovers `gate-*` off the filesystem -- the README's hand-written loop named seven of eight -- and treats `gate-xml` as an EXPECTED failure, so a rung that starts passing when it should not is red as well. Poison-tested in both directions. |
| 2026-09-10 | **The allocator seam ran on a board for the first time** (ESP32-S3, 9/9). `HOUSE-STACK.md` had said "no board has run it" since it was written. The row is deliberately the S3 caveat again -- it does NOT touch B4, which wants the Cortex-M3 cycle count -- but the fixed-region backend, `give`, the `FERR_*` codes and reclamation are now real on 32-bit silicon. Also measured: the seam **does** compile for `xtensa-esp32s3-none-elf`; the house note "rusty_alloc has no Xtensa port" is true of its default segment path, not of `prim::fixed`. |
| 2026-09-10 | **My own first version of that board row's reclamation check could not fail** -- `region_stats().free` before and after a drop, which reads 65536 both sides because that field counts extents not yet handed to the allocator. Replaced with 64 rounds of 32 KiB through a 192 KiB region (2 MiB total; a heap reclaiming nothing dies on the sixth), 64/64 served and 63/63 at the same address. **Same defect as B3's grep, same day, one section later.** When a check passes, ask what would have made it fail before believing it. |
| 2026-09-10 | **B4 needs no hardware.** Its kill test is a cycle count from the M3 QEMU cell, so the blocker is the cell (K3) and `qemu-system-arm`, not a part. Plugging in a board -- any board -- cannot close it, and the S3 is as close as Xtensa gets. |
| 2026-09-10 | **B3 and B5 closed, both by an upstream release, both verified by running the gate rather than by reading a changelog.** B5 is the plan's cleanest evidence: `gate-erasure` was measured FAILING at `=0.4.0` and passing at `=0.4.1` minutes apart in one session. B3 came out as **2.1.0**, not the 2.0.5 named here — check what the crate actually released, not what the plan asked for. |
| 2026-09-10 | **B3's kill test could not fail.** `grep -c "arrives with the next"` printed 0 on the UNFIXED tree, because the comment said "arrive with / the next release" — wrong verb form and split over a line break. Replaced with a compile check. **A kill test that greps for prose rots with the prose**; make every row run a compiler. |
| 2026-09-10 | **Two of the gate's own rungs had never run.** `host/` and `hostgit/` are packages inside a workspace whose `members = ["gate-*"]` does not match them, so cargo refused to build either — while the README recorded times for both. They now carry their own `[workspace]` tables, and `host/` was still pinning the two PRE-FIX versions because nothing had compiled it since. **Run every rung the README lists before believing any of them**, and re-run all three workspaces when a pin moves. |
| 2026-09-10 | `host` fails `cargo deny check advisories` on three `unmaintained` crates (`atomic-polyfill`, `smallstr` via **spacedb-sdk**; `paste` via **ffai-core**) and the gate workspace is clean, which is the host/bare-metal split doing its job. Two are one `heapless` 0.8 bump away, because 0.8 replaced `atomic-polyfill` with `portable-atomic` — the crate strategy item 3 already names. Owner steps for those two crates, not Kairos's. |
| 2026-09-09 | **B2 closed.** Fixed in `rusty_time` itself and released as `rusty_time-core` 0.2.0; the pin here moved in the same act and both rungs are green from the registry. Two bricks in one day (B1, B2) both closed by fixing the crate rather than working around it, and in both cases the `docs/upstream/` draft turned out never to be needed -- **the draft is insurance, not the plan.** Remaining: B3 and B5 need releases, B4 rides K4. |

## 8. Appendix — the exact failures

```text
gate-zstd    @ thumbv7em-none-eabihf:      [HISTORICAL -- fixed in 0.2.5, now exit 0]
gate-zstd    @ riscv32imac-unknown-none-elf: same [HISTORICAL -- now exit 0]
             the "x4 sites reported, 8 statics" above was rustc's CAP, not the count:
             204 errors, 427 uses, 16 files.
gate-erasure @ thumbv7em-none-eabihf:      [HISTORICAL -- fixed in 0.4.1, now exit 0]
             was: error[E0432]: unresolved import `core::sync::atomic::AtomicU64`  (kernel.rs:10)
gate-time    @ thumbv7em-none-eabihf:      [HISTORICAL -- fixed in 0.2.0, now exit 0]
             was: error[E0463]: can't find crate for `std` — `rusty_time_core`
             does not declare `#![no_std]`. This exact error is what POISONING
             the fix reproduces, which is how the gate was shown able to fail.
gate-xml     @ thumbv7em-none-eabihf:      error[E0463]: can't find crate for `std` — `rusty_xml_sax` / `rusty_xml_tree`
rusty_rtos_alloc --no-default-features without the cfgs: rusty_alloc's compile_error! ("assumes a SINGLE THREAD ... opt in with --cfg ra_single_threaded") — by design, not a bug
```

The gate: `tools/house-gate/README.md`. The verdict table: `docs/HOUSE-STACK.md`.
