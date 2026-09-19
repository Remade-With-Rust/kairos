# Kairos

[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/remade-with-rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

*Greek: the opportune moment, the right time — the kernel that decides which
task runs now.*

Kairos is the Remade-With-Rust programme that rebuilds the **FreeRTOS
portfolio** — the kernel, its ports and heaps, and the LTS libraries that sit
on it — in memory-safe Rust, as a family of independent packages that expose
the API a FreeRTOS developer already knows, prove every scheduling decision
against the C kernel's own trace, and run on the chips the house already
ships to (the Janus ESP32 family, Cortex-M, RISC-V).

The plan is [docs/plans/rtos-mission.md](docs/plans/rtos-mission.md) — one
document: where we are, the strategy, what is finished, what remains, the
decisions that bind. Read it first; everything below is a summary of it.

## The one-line test

> Could this ship as-is inside a firmware a maker soldered, with no C in our
> crates, every scheduling decision replayable from a trace a stranger can diff
> against the C kernel's, and would a FreeRTOS developer with a board and this
> folder reach a blinking task in one evening using the names they already know?

## The family

| Package | Layer | What it remakes | Status |
|---|---|---|---|
| [`rusty_rtos_core`](https://github.com/Remade-With-Rust/rusty_rtos_core) | 0 · foundation | the shared vocabulary: ticks, priorities, generational handles, one `Copy` error, the `Config` trait, the Port / Heap / Trace / Hooks seams | **K0 passed** — 34 tests, 8 bare-metal rungs, Miri |
| [`rusty_rtos_kernel`](https://github.com/Remade-With-Rust/rusty_rtos_kernel) | 1 · function | `tasks.c`, `queue.c`, `timers.c`, `event_groups.c`, `stream_buffer.c` | **K1 passed** — 9/9 scenarios, 8,408,764 lines identical at 100k ticks; **K2 14/18** |
| [`rusty_rtos_port`](https://github.com/Remade-With-Rust/rusty_rtos_port) | 1 · function | `portable/`: sim, Posix, Cortex-M, RISC-V, Xtensa | **sim port done**; **K3 part done** — Cortex-M switches under QEMU, 100/100 resumptions |
| [`rusty_rtos_heap`](https://github.com/Remade-With-Rust/rusty_rtos_heap) | 1 · function | `portable/MemMang/heap_1..5.c` | **K4 passed** — 20,000 operations agree with `heap_4.c` |
| [`rusty_rtos_backoff`](https://github.com/Remade-With-Rust/rusty_rtos_backoff) | 1 · function | backoffAlgorithm | **done** — 192/192 calls agree |
| [`rusty_rtos_json`](https://github.com/Remade-With-Rust/rusty_rtos_json) | 1 · function | coreJSON | **done**, both halves — JSONTestSuite at 100 % |
| [`rusty_rtos_sntp`](https://github.com/Remade-With-Rust/rusty_rtos_sntp) | 1 · function | coreSNTP | **done**, both halves — 160 trace lines agree |
| [`rusty_rtos_mqtt`](https://github.com/Remade-With-Rust/rusty_rtos_mqtt) | 1 · function | coreMQTT | **complete** — **218 of 218 functions**, 100 % |
| [`rusty_rtos_demo`](https://github.com/Remade-With-Rust/rusty_rtos_demo) | 2 · process | `Demo/Common/Minimal` — the conformance corpus | **K6 passed** — 25/25 unmodified C demo files, QEMU M3 and host |
| [`rusty_rtos-capi`](https://github.com/Remade-With-Rust/rusty_rtos-capi) | 2 · process | the C ABI: `xTaskCreate` and friends as `extern "C"` | **K6 passed** — both halves, two ports, three platforms |
| `rusty_rtos_{tcp, http, pkcs11, cellular, fat, posix, cli, mpu}` | 1 | the rest of the LTS and the Labs | planned (K7, K9) |

Every package has the same shape: a `no_std` (+ `alloc`), `forbid(unsafe)`
**core** that compiles for Cortex-M and RISC-V bare metal and is tested on the
host; a **facade** you depend on; WRAP crates (a port, a backend, the C ABI)
that hold the family's only fenced `unsafe`; per-chip **firmware** examples as
their own cargo projects; a plan, a ledger and a hardening table.

## This folder is an umbrella, not a workspace

Each package directory is **its own git repository and cargo workspace**,
deployable on its own to a public or private GitHub repo. The umbrella holds
the plan, the fleet manifest, the fleet tool and the C oracle harness.

```text
KAIROS.toml          the fleet manifest: every package, its kind, status, intended visibility,
                     crates, no_std crates, targets, the siblings it uses, its plan
ORACLES.md           every pinned reference (FreeRTOS-Kernel V11.3.1, the 202604.01 LTS set), the
                     sim contract and the trace format
tools/kairos/        the fleet tool (Rust): status · check · new · patches · harden · deploy · secrets · oracle · conform;
                     on the house stack: rusty_alloc (seam), thoth (glyphs), rusty_json_turbo (--json), rusty_zstd (traces)
tools/house-gate/    the house-stack compile gate: each house crate at its pin, no_std on the Kairos targets, under deny.toml
oracle/harness/      the deterministic-tick patch and trace hooks for the C kernel (tracked);
                     oracle/FreeRTOS-Kernel and oracle/FreeRTOS are fetched checkouts (ignored)
.cargo/config.toml   [net] only; each repo's own (generated, gitignored) .cargo/config.toml
                     patches the siblings IT uses to the local checkouts — `kairos patches`
docs/                plans/rtos-mission.md, the one plan; API-MAP.md and CONFIG-MAP.md, the contracts;
                     LEDGER.md, the family-level numbers (the oracle trace, the fleet gate);
                     HOUSE-STACK.md, every house crate's readiness for this family, compile-gated;
                     plans/build-me-bare.md, the queue of house crates Kairos wants on bare metal
rusty_rtos_*/        the packages (each: .git, Cargo.toml, crates/, firmware/, docs/plans/<name>.md,
                     docs/LEDGER.md, docs/plans/use-protection-please.md, ci)
```

```sh
cargo build --release --manifest-path tools/kairos/Cargo.toml
tools/kairos/target/release/kairos status --ci              # what exists, what is a repo, CI's verdict
tools/kairos/target/release/kairos check --fmt --clippy --test --deny --harden
                                                            # host + Cortex-M + RISC-V no_std gates, every package
tools/kairos/target/release/kairos check --xtensa           # + each no_std crate on xtensa-esp32s3-none-elf (local only)
tools/kairos/target/release/kairos patches                  # each repo's umbrella-only sibling `paths` override (lockfile stays standalone)
tools/kairos/target/release/kairos harden --all             # render every README's hardening block from its plan file
tools/kairos/target/release/kairos new rusty_rtos_tcp       # stamp a new package from the template
tools/kairos/target/release/kairos deploy rusty_rtos_core --private   # commit, create the GitHub repo, push
KAIROS_GIT_TOKEN=<pat> tools/kairos/target/release/kairos secrets     # the sibling-fetch token into every package's CI
tools/kairos/target/release/kairos oracle fetch             # clone the pinned FreeRTOS commits (ORACLES.md) into oracle/
tools/kairos/target/release/kairos oracle patch             # the deterministic-tick edits to the Posix port (sim contract v1)
tools/kairos/target/release/kairos oracle build dynamic     # gcc under WSL (Windows) or cc (Linux); no CMake
tools/kairos/target/release/kairos oracle trace dynamic     # run twice, refuse a differing trace, store oracle/traces/dynamic.trace.zst
tools/kairos/target/release/kairos oracle cat dynamic       # the stored trace, decompressed, for a diff
tools/kairos/target/release/kairos conform dynamic          # the K1 gate: the Rust kernel's trace against the C kernel's
tools/kairos/target/release/kairos conform --all --exits    # every scenario, with the critical-section-exit column
tools/kairos/target/release/kairos status --json            # the fleet table as JSON (the house serde_json)
```

`deploy` needs exactly one of `--public` / `--private` and refuses a mismatch
with the manifest's intended visibility. `deploy --umbrella` pushes this folder
itself. Deploy **`rusty_rtos_core` first**: the other packages consume it by git
URL, and cargo must be able to fetch that URL before the local `paths`
override can take over (the override is applied after resolution, so the
committed Cargo.lock keeps the git source and `--locked` passes everywhere).

**Visibility policy:** everything starts **private** under `Remade-With-Rust`.
A repo flips public only at the market-ready bar (plan §2.11), and the owner
runs the flip. While a sibling is private, cargo fetches it through the git CLI
(`net.git-fetch-with-cli` in `.cargo/config.toml`) and a package's CI needs the
`KAIROS_GIT_TOKEN` secret (a fine-grained PAT, Contents: Read on every
`rusty_rtos_*` repo, fanned out by `kairos secrets`).

## What is Rust here, and what is not

The whole family, the fleet tool (which installs `rusty_alloc` through a seam
and prints the house `rusty_symbols` glyphs) and both scripts Janus kept in
Python (the sibling-patch generator and the hardening-table renderer) are Rust.
The C in this folder is the **oracle** — FreeRTOS itself, built by `gcc`/`clang`
under WSL as a dev-only reference the way openh264 and libzstd are for the
codecs — and it is never a dependency of anything that ships.

## Principles carried from the house

- Pure Rust, memory-safe by construction; a `*-sys` crate is a decision stated
  out loud, never a default; `cargo-deny` enforces the licence and `*-sys`
  posture in every repo.
- API first: every capability is an op callable by a test, a CLI and an agent
  before it has a button or a sketch verb.
- The C kernel's trace is the oracle; the scalar path is the oracle; counters
  before clocks; no claim without a ledger row and its method line; the README
  copies the plan and never upgrades it.
- Every parser that takes bytes from a wire, a store or a bus has a no-panic
  test; every package has a hardening table rendered from its plan file.

## Status (2026-09-19)

Every number below is in [`docs/LEDGER.md`](docs/LEDGER.md) with the command
that produced it, and every one is a diff against the C, not a self-assessment.

| Milestone | |
|---|---|
| **K0 family** | passed 2026-09-09 |
| **K1 scheduler on sim** | passed 2026-09-09 — nine of nine scenarios, **8,408,764 lines identical** at 100,000 ticks |
| **K2 IPC + timers** | 14 of 18 passed 2026-09-09 — 12,808,722 lines identical; Kani verifies seven harnesses (3,965 checks) |
| **K2.1–2.3 the Rust, async and static faces** | passed 2026-09-10 |
| **K3 silicon + QEMU** | part done 2026-09-10 — Cortex-M switches under QEMU, 100/100 resumptions over 200 switches, poison-proven |
| **K4 heaps** | **passed 2026-09-10** — 20,000 operations agree with `heap_4.c` on the offset chosen, the free bytes and the minimum ever free |
| **K6 C ABI** | **both halves pass 2026-09-11** — 25/25 unmodified C demo files on QEMU M3 and on the host, Windows threads and Linux pthreads |
| **K7 libraries** | `backoff`, `json` and `sntp` done; **`mqtt` complete — 218 of coreMQTT's 218 functions** |
| **K5 Janus joint, K8 SMP / MPU / 1.0** | open |

It has run on a chip: K3 is QEMU on Cortex-M and RISC-V, and K6 links the
unmodified C demos against the Rust ABI.

## Part of Remade With Rust

Kairos is **[Remade With Rust](https://github.com/remade-with-rust)**'s FreeRTOS
programme, and one of the pieces **[Mata Network](https://www.mata.network)**
builds on: a smart device that joins the Mata cloud runs a real-time kernel
underneath whatever it is doing, and this is that kernel — memory-safe, with
the API a FreeRTOS developer already knows, and every scheduling decision
diffable against the C kernel's own trace.

The family:
[`rusty_rtos_core`](https://crates.io/crates/rusty_rtos_core) (the shared vocabulary),
[`rusty_rtos_kernel`](https://crates.io/crates/rusty_rtos_kernel) (the scheduler),
[`rusty_rtos_port`](https://crates.io/crates/rusty_rtos_port) (the architecture seam),
[`rusty_rtos_heap`](https://crates.io/crates/rusty_rtos_heap) (the allocators),
[`rusty_rtos_json`](https://github.com/Remade-With-Rust/rusty_rtos_json) (coreJSON),
[`rusty_rtos_sntp`](https://github.com/Remade-With-Rust/rusty_rtos_sntp) (coreSNTP),
[`rusty_rtos_mqtt`](https://github.com/Remade-With-Rust/rusty_rtos_mqtt) (coreMQTT),
[`rusty_rtos_backoff`](https://github.com/Remade-With-Rust/rusty_rtos_backoff) (backoffAlgorithm),
[`rusty_rtos-capi`](https://github.com/Remade-With-Rust/rusty_rtos-capi) (the C ABI) and
[`rusty_rtos_demo`](https://github.com/Remade-With-Rust/rusty_rtos_demo) (the conformance corpus).
The last six are on GitHub and not yet on crates.io. Also check out
the rest of **[github.com/remade-with-rust](https://github.com/remade-with-rust)**.

## About Mata Network

<!-- ORG BOILERPLATE — keep identical across repos -->

[Mata Network](https://www.mata.network) builds sovereign, self-hostable
infrastructure. **Remade With Rust** is our open-source home for the
permissively-licensed building blocks that work depends on.

<!-- /ORG BOILERPLATE -->

## License

MIT OR Apache-2.0, at your option, for every package and for this folder.
FreeRTOS is MIT-licensed by Amazon.com, Inc. or its affiliates; Kairos remakes
its API and behaviour from the published sources and credits it in every README.
