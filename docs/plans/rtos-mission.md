# Kairos — the plan (FreeRTOS remade in Rust)

**One document.** Written 2026-09-09, before a line of code exists, so that a
session starting cold can read where we are, what does not change, what is
finished and what remains. It mirrors the Janus family plan
(`C:\Users\talmo\coding\janus\docs\plans\janus.md`) section for section and
borrows its build architecture whole, because that architecture has already
shipped ten independent packages and paid for its walls. Each package will
carry its own plan in its own repository (`<package>/docs/plans/<package>.md`)
and its own ledger (`docs/LEDGER.md`); those obey this one. What is finished is
§4, what remains §5, the strategy that does not change §2, the decisions that
bind §8.

**Kairos** (Greek: the opportune moment, the right time) is the working title
for the family; the folder is `F:\coding\rusty_RTOS`, the packages are
`rusty_rtos_*`. The name is an open decision (§5.5); nothing below depends on it.

Re-check §1 before use: it is a snapshot.

---

## 0. Mission and promise

**Remake the FreeRTOS portfolio — the kernel, its ports, its heaps, and the
LTS libraries that sit on it (TCP/IP, MQTT, HTTP, JSON, SNTP, backoff, PKCS#11,
cellular) — as independent memory-safe Rust packages that expose the API a
FreeRTOS developer already knows, prove every scheduling decision against the
C kernel's own trace, and run on the chips the house already ships to, so that
the last C layer between a sensor and the MATA mesh is gone.**

FreeRTOS is the most deployed real-time kernel on earth: MIT-licensed, MISRA
C:2012, 60-odd ports, an LTS line Amazon maintains, and the operating system
underneath every ESP-IDF firmware — which means underneath every Janus Track A
device today. It is also 365 KB of `tasks.c`, a hand-maintained intrusive list,
`configASSERT`, and a heap that hands you `NULL`. The Remade-With-Rust
portfolio owns everything above it (codecs, identity, storage, the mesh) and
everything beside it (esp-hal, the Janus function packages). It does not own
the scheduler. Kairos is the scheduler.

Three things change when it exists:

1. **Safety by construction in the kernel.** The scheduler, the IPC and the
   timers are `forbid(unsafe)` Rust over an arena of indices; the only `unsafe`
   in the family is the context switch in each port and the C ABI, fenced and
   inventoried.
2. **A FreeRTOS program is a Rust program.** `xTaskCreate` becomes
   `Task::spawn`, `xQueueSend` becomes `Queue::send`, the names and the
   semantics stay; a C caller relinks against the C ABI and the unmodified
   standard demo tasks pass.
3. **The Janus device stack has a Rust kernel.** Track B firmwares (`no_std`,
   esp-hal) run on `rusty_rtos` instead of `esp-rtos`, with the Wi-Fi/BLE blob
   hosted through the `esp-radio-rtos-driver` seam; the home computer's device
   fleet stops depending on a C kernel it cannot audit.

The one-line test for every change: *could this ship as-is inside a firmware a
maker soldered, with no C in our crates, every scheduling decision replayable
from a trace a stranger can diff against the C kernel's, and would a FreeRTOS
developer with a board and this folder reach a blinking task in one evening
using the names they already know?*

---

## 1. Where we are (2026-09-09)

Nothing exists. `F:\coding\rusty_RTOS` holds this file and is not yet a git
repository. No package is stamped, no oracle is built, no number is measured.
The umbrella tooling it will copy exists and works in Janus (the `janus` fleet
tool, 839 lines; `gen-sibling-patches.py`, 173 lines; the package template).

| package | layer | status (`KAIROS.toml`) | proven on the host | on a chip / emulator |
|---|---|---|---|---|
| `rusty_rtos_core` | 0 | K0 types (private) | 34 tests, 8 bare-metal rungs, Miri on the unit tests, deny + audit clean (2026-09-09) | — |
| `rusty_rtos_kernel` | 1 | **K1 scheduler, passed** (private) | all nine K1 scenarios trace-identical to the C kernel for 100,000 ticks, counters equal (2026-09-09); Miri green over the corpus; fleet gate on 8 rungs | — |
| `rusty_rtos_port` | 1 | **K1 sim port** (private) | the deterministic sim port implements the sim contract v1; Miri green; fleet gate (2026-09-09) | — |
| `rusty_rtos_heap` | 1 | scaffold (private) | fleet gate (2026-09-09) | — |
| `rusty_rtos_json` | 1 | scaffold (private) | fleet gate (2026-09-09) | — |
| `rusty_rtos_backoff` | 1 | scaffold (private, standard tier) | fleet gate (2026-09-09) | — |
| `rusty_rtos_mqtt` | 1 | planned | — | — |
| `rusty_rtos_http` | 1 | planned | — | — |
| `rusty_rtos_sntp` | 1 | planned | — | — |
| `rusty_rtos_tcp` | 1 | planned | — | — |
| `rusty_rtos_pkcs11`, `_cellular`, `_fat`, `_posix`, `_cli`, `_mpu` | 1 | planned (after 1.0 of the kernel) | — | — |
| `rusty_rtos_demo` | 2 | **K1 corpus, 9 of 9** (private, standard tier) | `dynamic`, `PollQ`, `BlockQ`, `semtest`, `countsem`, `recmutex`, `blocktim`, `QPeek`, `GenQTest` remade and trace-identical; the runner, the trace sink and `kairos conform` (2026-09-09) | — |
| `rusty_rtos-capi` | 2 | scaffold (private) | fleet gate (2026-09-09) | — |
| `kairos` (fleet tool) | 2 | K0 (in the umbrella `Remade-With-Rust/kairos`, private) | status / check / new / patches / harden / deploy / secrets / oracle all exercised in K0; `oracle trace dynamic` reproducible | — |

**Reference, pinned for this plan:** FreeRTOS-Kernel **V11.3.1** (released
2026-08-21) as the kernel oracle; the **FreeRTOS 202604.01-LTS** set (kernel
V11.3.1, FreeRTOS-Plus-TCP, coreMQTT, coreMQTT-Agent, coreHTTP, coreJSON,
coreSNTP, corePKCS11, backoffAlgorithm, FreeRTOS-Cellular-Interface) as the
library oracles. The exact commit of each is fixed in K0 and recorded in the
umbrella `ORACLES.md`; a number measured against an unpinned checkout is not a
number.

**This machine (read 2026-09-09):** Rust 1.98.0 stable; the `esp` toolchain
(Xtensa via `-Z build-std=core`); targets `riscv32imac/imafc/imc-unknown-none-elf`,
wasm32, the desktop triples; `thumbv7em-none-eabihf`, `thumbv8m.main-none-eabihf`
and — since 2026-09-10 — **`thumbv7m-none-eabi`** (the Cortex-M3 of the QEMU
cell, which has no FPU and is not `thumbv7em`); `clang`, `cmake`, `ninja`
present, **no `gcc`, no `arm-none-eabi-gcc`**; **QEMU 11.1.0 present since
2026-09-10** (`qemu-system-arm`, `-riscv32`, `-xtensa`; `lm3s6965evb` and
`virt` machines confirmed, and an M3 binary run through it exits QEMU with a
code, so a cell can gate); cargo-deny, audit, vet, geiger, fuzz, mutants,
careful, miri present; **cargo-kani 0.67 under WSL only**, **no nextest, no
probe-rs**; espflash 4.6, espup, esp-generate, ldproxy. Boards from
Janus: a XIAO ESP32-S3 Sense and an AI-Thinker ESP32-CAM in hand; ESP32-C6
devkits on the purchase list. No Cortex-M board.

---

## 2. Strategy — what does not change

### 2.1 The portfolio map — FreeRTOS repository → Kairos package

Every FreeRTOS repository is classified the way Janus classified Espressif's:
**REMAKE** (pure Rust, same job, same names), **WRAP** (a thin fenced boundary
over silicon or a blob; we own the API, not the hardware), **NEVER** (out of
scope by design, with the reason). The org's LTS manifest is the canonical list.

| FreeRTOS repository | Kairos package | REMAKE | WRAP | NEVER |
|---|---|---|---|---|
| **FreeRTOS-Kernel** `tasks.c`, `queue.c`, `timers.c`, `event_groups.c`, `stream_buffer.c`, `list.c` | **`rusty_rtos_kernel`** | the scheduler (fixed-priority preemptive, round-robin time slicing, `configUSE_PORT_OPTIMISED_TASK_SELECTION`), task notifications (array), TLS pointers, run-time stats, idle + hooks, queues, semaphores, mutexes with priority inheritance, recursive mutexes, queue sets, the queue registry, software timers + the daemon task, event groups, stream and message buffers, static and dynamic allocation, the trace facility, SMP (`configNUMBER_OF_CORES > 1`, core affinity) | — | `croutine.c` (co-routines: deprecated upstream), the `configUSE_ALTERNATIVE_API`, newlib/picolibc reentrancy shims |
| **FreeRTOS-Kernel** `portable/GCC/*`, `portable/ThirdParty/GCC/*` | **`rusty_rtos_port`** (one crate per architecture) | the port *logic* (tick, yield, critical sections, stack layout, idle sleep) | the context switch and vector entry per arch (`PendSV`/`SysTick` on Cortex-M; `mtvec`/CLINT on RISC-V; window-register save/restore on Xtensa) in fenced inline asm; startup via `cortex-m-rt` / `riscv-rt` / `esp-hal`; the Posix host port | the 50-odd legacy ports (ARM7/9, AVR, 8051, PIC, ColdFire, H8, MSP430, NiosII, MicroBlaze, PPC, RX, RL78, TriCore, oWatcom, IAR/Keil/Tasking variants); Cortex-A/AArch64 until a consumer exists |
| **FreeRTOS-Kernel** `portable/MemMang/heap_1..5.c` | **`rusty_rtos_heap`** | `heap_1` (bump), `heap_4` (first-fit with coalescing), `heap_5` (multi-region) and the `configENABLE_HEAP_PROTECTOR` canaries; `pvPortMalloc`/`vPortFree` semantics; the malloc-failed hook | `heap_3` (wraps the platform allocator) as a seam over `rusty_alloc` small-metal / `esp-alloc` / the IDF heap | `heap_2` (deprecated upstream; a `heap_4` alias with the difference documented) |
| **FreeRTOS-Kernel** `mpu_wrappers.h`, `mpu_prototypes.h`, ARMv8-M ports | `rusty_rtos_mpu` | privileged/unprivileged tasks, the system-call shim, access-control lists (`configENABLE_ACCESS_CONTROL_LIST`) | the MPU/SAU/TrustZone registers per arch | — (after kernel 1.0) |
| **FreeRTOS-Plus-TCP** | `rusty_rtos_tcp` | the FreeRTOS-shaped socket API (`FreeRTOS_socket`, `_bind`, `_send`, `_recv`, DHCP/DNS/ARP client surface) and the network-interface seam | the packet engine: **smoltcp** (pure Rust, `no_std`, heap-free) as the engine behind the API — a decision item (§5.5); MAC drivers via esp-hal / `esp-emac` | a second TCP/IP stack written from scratch while smoltcp exists |
| **coreMQTT**, **coreMQTT-Agent** | `rusty_rtos_mqtt` | MQTT 3.1.1 + 5.0 client (serializer, deserializer, keep-alive, QoS 0-2 state), the agent task for connection sharing, over a transport seam | — | — |
| **coreHTTP** | `rusty_rtos_http` | the HTTP/1.1 client subset (request build, response parse, range GET) over the same transport seam | — | a server; HTTP/2 |
| **coreJSON** | `rusty_rtos_json` | the strict ECMA-404 validator and the query-by-path search, zero allocation | — | a serde replacement |
| **coreSNTP** | `rusty_rtos_sntp` | the SNTPv4 client (or a `no_std` leaf of the house `rusty_time`, §5.5) | — | NTS on a chip in v1 |
| **backoffAlgorithm** | `rusty_rtos_backoff` | exponential backoff with jitter (one file; kept as its own package for the 1:1 map, stamped from the template in an hour) | — | — |
| **corePKCS11** | `rusty_rtos_pkcs11` | the PKCS#11 subset over RustCrypto for drop-in C consumers | the device key through the mID seam (`rusty_esp_mid` on a chip) | a full PKCS#11 |
| **FreeRTOS-Cellular-Interface** (+ BG96 / SARA-R4 / HL7802 references) | `rusty_rtos_cellular` | the 3GPP TS 27.007 AT layer, the module ports as data + tables, the socket adapter | the UART | — |
| **Lab-Project-FreeRTOS-FAT** | `rusty_rtos_fat` | FAT12/16/32 over a block-device seam | — | exFAT; NTFS |
| **Lab-Project-FreeRTOS-POSIX** | `rusty_rtos_posix` | pthreads / mqueue / semaphore / clock over the kernel, for the C ABI consumers | — | a POSIX-complete OS |
| **FreeRTOS-Plus-CLI** | `rusty_rtos_cli` | the command registry and line parser | — | — |
| **FreeRTOS-Plus-Trace** (Percepio recorder) | — | the trace *seam* is in core (§2.4); a `rusty_rtos_trace` sink that writes the oracle format and a `defmt`/`rerun` sink | — | the Percepio recorder and Tracealyzer format (proprietary) |
| **AWS IoT libraries** (Device Shadow, Jobs, OTA, Defender, Fleet Provisioning) | — | — | — | cloud-of-record by design; the house answers are the MATA mesh (`rusty_esp_iroh`), mID adoption, and `janus/ota/1` |
| **FreeRTOS-Kernel-Book**, **FreeRTOS-Website-Content**, **CMSIS-Packs**, **freertos-gdb** | — | the API docs are generated from the Rust docs; a `kairos` gdb/`probe-rs` helper later | — | — |
| **FreeRTOS** (the classic distribution: `Demo/`, `Test/`) | **`rusty_rtos_demo`** (Layer 2) | the 34 standard demo task files of `Demo/Common/Minimal` as the conformance corpus; the `Posix_GCC`, `CORTEX_LM3S6965_GCC_QEMU`, `RISC-V_RV32_QEMU_VIRT_GCC` demos as the emulator cells; the CBMC and VeriFast proof lists as the Kani/loom harness list; the CMock unit tests as the unit-test vocabulary | — | the 200+ board-specific demo projects |

### 2.2 Three layers, one dependency direction

```text
Layer 2  process      kairos (fleet tool) · rusty_rtos_demo · rusty_rtos-capi · firmware/ demos
                              │ depends on ▼
Layer 1  functions    kernel · port · heap · mpu · json · backoff · mqtt · http · sntp · tcp · pkcs11 · cellular · fat · posix · cli
                              │ depends on ▼
Layer 0  foundation   rusty_rtos_core
```

Inside Layer 1 the graph is a DAG with a small fixed set of cross edges, every
one "consumes a trait":

```text
kernel ──► port        (the scheduler drives a Port; it never names an arch)
kernel ──► heap        (dynamic allocation goes through the Heap seam)
mpu ────► kernel       (a privileged wrapper over the kernel's ops)
mqtt/http/sntp/tcp ─► kernel  (a task, a queue, a timer, a semaphore — through the facade)
mqtt/http/sntp ─────► tcp     (through the transport seam only; a test uses a loopback)
cellular ──► tcp       (the socket adapter)
```

`json`, `backoff`, `fat`, `cli` depend on core alone. If a fourth kind of edge
is wanted, it is a missing type in core. `rusty_rtos_demo` reaches every
function package; `kairos` reaches none.

### 2.3 The shape of every package (fixed — copied from Janus)

```text
<package>/
  Cargo.toml                 workspace; [workspace.package] lockstep version; resolver 3
  crates/<package>           facade: re-exports + prelude — what you depend on
  crates/<package>-core      no_std (+ alloc as a feature); forbid(unsafe); types, traits, algorithms
  crates/<package>-<wrap>    the WRAP crate(s): a port (-cortex-m, -riscv, -xtensa, -posix, -sim),
                             a backend (-esp-radio), a C ABI — the ONLY place a fenced unsafe block lives
  firmware/                  per-chip example projects, separate cargo projects, excluded from the workspace
  docs/plans/<package>.md    the package plan
  docs/LEDGER.md             every number, with its method line
  .github/workflows/ci.yml   host matrix (ubuntu/windows/macos: fmt, clippy -D warnings, test)
                             + no_std rungs (thumbv7em-none-eabihf, thumbv8m.main-none-eabihf,
                               riscv32imac-unknown-none-elf, riscv32imafc-unknown-none-elf;
                               --no-default-features, and --features alloc)
                             + the sim-port conformance run (rusty_rtos_demo on the sim port, host)
  deny.toml                  permissive-only; *-sys, ring, aws-lc-sys, libc-backed crates denied;
                             wildcards = deny; allow-git for siblings
  SECURITY.md, UNSAFE.md     from the first commit (the -port crates inventory every block)
```

Rules the shape encodes (Janus §2.2, unchanged, one addition):

1. **The core is arch-agnostic.** `-core` never names `cortex-m`, `riscv`,
   `esp-hal`, ESP-IDF, an allocator, or a std-only API. Its tests run on the
   host. It compiles for every listed bare-metal target with and without
   `alloc`; CI proves it on every push.
2. **`unsafe` lives in the WRAP crates only**, denied crate-wide, opened per
   block with a `// SAFETY:` invariant, and only at a context switch, a vector
   table, a register, an FFI boundary, or the C ABI. `undocumented_unsafe_blocks
   = "deny"` everywhere; every block is a row in the crate's `UNSAFE.md`.
3. **No library sets `#[global_allocator]`.** A firmware binary installs the
   allocator through the `rusty_rtos_heap` seam (or the Janus `rusty_esp_alloc`
   seam); `rusty_rtos_heap`'s `heap_4` is itself a heap, never a global allocator.
4. **Firmware projects are separate cargo projects** (target, linker script,
   toolchain per chip). n0 and Janus both arrived here.
5. **Every claim has a kill test or a ledger row.** The README's status copies
   the plan; it never upgrades it.
6. **(New) Every package ships a `sim` configuration.** Anything that can run
   on the deterministic sim port runs there in CI, on every push, on every
   host OS. Hardware and emulator rows are additive, never the only gate.

Edition 2024, MSRV 1.85, `unsafe_code = "deny"` at the workspace, `forbid` in
every core, `missing_docs = "warn"`, MIT OR Apache-2.0 (FreeRTOS is MIT; the
API names are not copyrightable but the credit is written in every README).

### 2.4 The oracle: the C kernel's trace is the bitstream

An RTOS has no byte-identical output to gate on — until you make one. FreeRTOS
ships a trace facility: ~120 `trace*` macros (`traceTASK_SWITCHED_IN`,
`traceQUEUE_SEND`, `traceTIMER_EXPIRED`, `traceEVENT_GROUP_WAIT_BITS`, …)
that fire at every scheduling and IPC decision. Under a **deterministic tick**
— the harness advances time, no real timer, single-threaded, cooperative
switch — the C kernel's behaviour is a pure function of the program and the
tick sequence, and the trace macros print it as a sequence of symbols.

That sequence is our bitstream. The discipline is `codec-bringup-decoder`
transposed:

- **The reference C kernel IS the oracle — instrument it, don't just diff
  outcomes.** Build FreeRTOS-Kernel V11.3.1 on its Posix port with `clang` on
  this machine, patch the port so the tick is delivered by an explicit call
  from the demo harness instead of a signal, define every `trace*` macro to
  print `(tick, event, task, object, arg)` to stderr. Keep the patch and the
  captured traces in-tree (`rusty_rtos_demo/oracle/`); the C binary is
  ephemeral.
- **Compare the FULL state, brick by brick.** Our kernel implements the same
  trace seam (`rusty_rtos_core::Trace`, a trait with a no-op default) and a
  sink that prints the same line format. A demo scenario is conformant when
  the two traces diff clean for N ticks. Never advance past a divergence; the
  first differing line names the brick.
- **Force determinism on both sides.** The sim port (`rusty_rtos_port-sim`) is
  single-threaded by construction: `tick()` and `yield_now()` are harness calls
  and a context switch is a function return. The Posix port of the C oracle is
  patched to the same contract. A trace whose line count varies between runs
  is a broken harness, not a finding.
- **The standard demo tasks are the corpus.** `Demo/Common/Minimal` holds 34
  self-checking task sets (`BlockQ`, `GenQTest`, `QPeek`, `QueueOverwrite`,
  `QueueSet`, `semtest`, `countsem`, `recmutex`, `TaskNotify`, `TimerDemo`,
  `EventGroupsDemo`, `StreamBufferDemo`, `MessageBufferDemo`, `IntQueue`,
  `IntSemTest`, `AbortDelay`, `blocktim`, `death`, `dynamic`,
  `StaticAllocation`, …). Each is remade in Rust in `rusty_rtos_demo` with the
  same task structure, priorities and timings; each runs against the oracle
  trace on the sim port and self-checks on every other port.
- **Where the trace is silent, count.** Context switches, ticks, queue
  operations, timer expiries, ISR entries are deterministic counters on both
  sides; a count that disagrees voids the comparison before any duration is
  read (`codec-measurement` §4).
- **The proofs are the property list.** `FreeRTOS/Test/CBMC/proofs/{Queue,Task}`
  and `Test/VeriFast/{list,queue}` state the invariants upstream proves;
  they become Kani harnesses on our core (queue send/receive never panics or
  reads out of bounds for any length and item size; the ready list is a
  permutation; a mutex holder's priority is the max of its waiters) and loom
  models for the SMP ready-list protocol.

The second oracle is silicon and emulators: the same demo tasks on QEMU
(`CORTEX_LM3S6965_GCC_QEMU`, `RISC-V_RV32_QEMU_VIRT_GCC` are FreeRTOS's own
emulator demos) and on the Janus boards, self-checking with the same "check
task" the C demos use, with cycle counts from the same counters the C build
reads (DWT `CYCCNT`, `mcycle`, `ccount`).

### 2.5 The vocabulary and the config discipline (`rusty_rtos_core`)

The one decision every package inherits: **handles are indices, never
pointers.** FreeRTOS threads `ListItem_t` through every TCB and queue; that
intrusive list is the unsafe heart of the C kernel and the reason its proofs
are hard. Our core holds every kernel object in a fixed-capacity arena
(`heapless`-style, capacity from the config) or an `alloc` pool, and every
list is an index-linked list inside that arena. A `TaskHandle` is a
generational index; a stale handle is an `Error::Gone`, not a use-after-free.
The cost is measured in K1 against the C list (counters, then cycles) and
revisited only with a ledger row.

| Type / trait | Shape | Why |
|---|---|---|
| `Tick<W>` | monotonic tick count, width a const generic (16/32/64) mirroring `configTICK_TYPE_WIDTH_IN_BITS` | the one constant most likely to invert a test; see `parameterizing-a-constant` |
| `Priority` | `u8` newtype bounded by `Config::MAX_PRIORITIES` at construction | a parse-constructor; no runtime range check downstream |
| `TaskHandle`, `QueueHandle`, `TimerHandle`, `EventGroupHandle`, `StreamBufferHandle` | generational indices into the arena | no pointers in the core |
| `Error` | one `Copy` enum (`Timeout`, `Full`, `Empty`, `NoMemory`, `Gone`, `InvalidPriority`, `InIsr`, `SchedulerSuspended`, `Unsupported`) mapping onto FreeRTOS's `pdFAIL`/`errQUEUE_*` codes | crosses crate boundaries without conversion; the C ABI maps it back |
| `Config` (a trait with associated consts) | `MAX_PRIORITIES`, `TICK_RATE_HZ`, `MINIMAL_STACK_SIZE`, `MAX_TASK_NAME_LEN`, `TIMER_QUEUE_LENGTH`, `TIMER_TASK_PRIORITY`, `NOTIFICATION_ARRAY_ENTRIES`, `NUM_TLS_POINTERS`, `NUMBER_OF_CORES`, `USE_PREEMPTION`, `USE_TIME_SLICING`, `IDLE_SHOULD_YIELD`, … | `FreeRTOSConfig.h` remade as a type the kernel is generic over; every knob is either a const (geometry), a cargo feature (a subsystem's presence), a hook (a trait method), or NEVER — the classification table is K0's deliverable (94 `config*` identifiers in `FreeRTOS.h` on 2026-09-09) |
| `Port` (trait) | `init_stack`, `start_first_task`, `yield_now`, `enter_critical`/`exit_critical` (nesting count), `tick_source`, `idle` (sleep, tickless), `in_isr`, `core_id` | the only way the kernel touches a CPU |
| `Heap` (trait) | `alloc`/`free` with the FreeRTOS semantics (`pvPortMalloc` returns `None`, never panics) | the only way the kernel allocates; static allocation needs none |
| `Trace` (trait) | one method per upstream `trace*` macro, default no-op | the oracle instrument, zero cost when unused |
| `Hooks` (trait) | idle, tick, malloc-failed, stack-overflow, daemon-startup, pre/post-sleep | the application hooks as a type, not link-time symbols |
| `Isr` token | a zero-sized proof that the caller is in an ISR, selecting the `*FromISR` variants | the from-ISR/not-from-ISR split as a type, not a naming convention |

`FORMAT_VERSION` discipline (Janus C4) applies to the trace line format and
the C ABI struct layouts from the first release: readers move first, writers
one release later, a new tag is a new version.

### 2.6 Two kinds of unsafe, and the census that keeps them honest

1. **Structural unsafe** — the context switch, the first-task start, the vector
   table, the stack initialisation, MPU/SAU registers. Lives only in
   `rusty_rtos_port-*`, one module per port, every block SAFETY-commented and
   listed in `UNSAFE.md`, gated by the sim-port trace (the port's logic) and by
   the emulator/board demo run (the asm). Kani proves what it can (stack
   layout arithmetic); Miri runs the core and the sim port.
2. **Interface unsafe** — the C ABI (`rusty_rtos-capi`): `extern "C"`
   functions taking raw pointers from C callers. Every pointer is validated
   against the arena (a handle that is not ours is a refused call, never a
   deref), sizes are overflow-checked, and the crate is `deny(unsafe_code)` with
   the FFI module opened.

The family-wide unsafe surface is published as one table in this plan (Janus
did it as "twenty documented blocks in three files"); `cargo geiger` runs per
release and the count may not rise without a decision-log row.

### 2.7 Ports and the two tracks — the Janus joint

| Track | Runtime today | What Kairos does there | When |
|---|---|---|---|
| **Janus A** — `std` on ESP-IDF | Espressif's IDF-FreeRTOS fork (C, SMP) under esp-idf-svc | **Nothing in v1.** IDF's drivers are C code written against IDF's FreeRTOS API; replacing the kernel underneath them is a C-ABI project (§5.1 item 11) once `rusty_rtos-capi` passes the unmodified C demo corpus. Until then Track A keeps its C kernel and the plan says so. | K8+ |
| **Janus B** — `no_std` + esp-hal | `esp-rtos` 0.4 (or Embassy) hosts the esp-radio blob's scheduler needs through `esp-radio-rtos-driver` 0.4.1 | **`rusty_rtos` becomes the Track B kernel.** `rusty_rtos_port-riscv` (C6/C61/H2/P4) and `-xtensa` (ESP32/S3) implement the `esp-radio-rtos-driver` interface so the Wi-Fi/BLE blob runs on our scheduler; the first Janus firmware moved over is `rusty_esp_signal/firmware/c6-mesh-node`. | K5 |
| **Host** | — | `-posix` (pthreads + signals, like `portable/ThirdParty/GCC/Posix`) for running a firmware's logic on a laptop; `-sim` for the oracle and CI. | K1 |
| **Cortex-M** | — | `-cortex-m` over `cortex-m-rt`: M0/M0+ (no `ldrex`, PRIMASK-only), M3, M4F/M7 (lazy FPU stacking), M23/M33/M55/M85 (ARMv8-M, TrustZone NTZ first). QEMU `lm3s6965evb` (M3) is the first cell; `mps2-an385` (M3) and `mps2-an505` (M33) follow. | K3 |
| **RISC-V** | — | `-riscv` over `riscv-rt`: RV32I + CLINT (QEMU `virt`), then esp-hal's interrupt controller on the C6. | K3 |
| **Xtensa** | — | `-xtensa` for ESP32 (LX6) and S3 (LX7) on the `esp` toolchain, `xtensa-esp32s3-none-elf`; local gate only (`kairos check --xtensa`), CI has no esp toolchain. | K5 |
| **AArch64 / Cortex-A** | — | not in v1 (no consumer). | — |

Companion-set rule from Janus: an esp-hal 1.1 set and a 1.2 set are never
mixed in one firmware; the port crate names no chip feature (the firmware
does); `cargo check -p rusty_rtos_port-xtensa` alone is expected to fail —
the firmware is the compile gate.

### 2.8 Claims discipline

An external oracle before any self-metric (the C kernel's trace, the C demo
tasks' own check task, QEMU, the board's cycle counter, `cbmc`'s property
list). **Counters before clocks:** context switches, ticks, queue ops, ISR
entries, bytes allocated are the primary evidence; cycle counts confirm; work-
count parity between arms is checked before any ratio. Every number lives in
a `docs/LEDGER.md` with its method line (pinned? interleaved? CPU or wall?
how many pairs? what was the null-arm floor? which QEMU version? which
board, which clock?). The README says "sim only", "QEMU only" or "builds, not
flashed" until a row exists. A speed claim against C is taken on the same
emulator or board with the same counter, both binaries built by us from
pinned sources, ABBA-interleaved, with a null arm — `codec-measurement`
governs. **No claim is ever "faster than FreeRTOS" without the row; parity is
the honest headline until the ledger says otherwise** (rusty_alloc's lesson).

Every parser that takes bytes from a wire, a store or a bus (JSON, MQTT,
HTTP, SNTP, AT responses, FAT, TCP segments, the C ABI's structs) has a
`tests/no_panic.rs` from the day it exists; the kernel's public API has a
"no-panic on any handle, any tick, any priority" property test.

### 2.9 The matrix rule

A supported combination is a **cell**: a port × a chip or emulator × a config
profile (`minimal`, `default`, `smp`, `mpu`). A cell is `Verified` only when
`rusty_rtos_demo`'s check task has run its kill test on that hardware or
emulator with ledger numbers; `Host` when the firmware builds and every part
runs on the sim; `Planned` when only the port exists. The `kairos` tool
refuses to mark a cell above what the ledger supports, and a README may not
list a cell the manifest does not carry.

### 2.10 Security doctrine on the kernel

The kernel *is* the trusted computing base of every firmware above it; it is a
**critical-path** unit in `use-protection-please` terms and every package's
hardening table starts in the first commit (`docs/plans/use-protection-please.md`,
rendered into the README by the vendored script, checked in CI). Specifically:
no `unwrap()` reachable from any API; every `FromISR` variant is a separate
typed entry (an ISR-only token) so the C-world foot-gun of calling the wrong
variant does not compile; stack overflow detection (`configCHECK_FOR_STACK_
OVERFLOW` 1 and 2) is on by default in the `default` profile; the heap
protector canaries are on by default; the MPU package is the privilege story
and ships after 1.0; the C ABI validates every handle and length; secrets never
enter the kernel (PKCS#11 and mID keep them); `cargo deny` denies `*-sys`,
`ring`, `aws-lc-sys`, `libc`-linked crates in every repo; the family unsafe
census (§2.6) is a release gate; Miri, Kani harnesses for the proof list, loom
for SMP, `cargo fuzz` targets for every parser, and `cargo mutants` on the
scheduler core are wired before the first public flip.

### 2.11 Visibility and the market-ready bar

Everything starts private under `Remade-With-Rust`. A repo flips public only
when (Janus §2.10, verbatim): its siblings are public first (core, then
kernel/port/heap, then the libraries, then demo/capi); released pins only (no
`git =`, no branch); its emulator and board half is measured; its hardening
row is complete and `cargo deny` is green in CI; CI is green without a token;
the README equals the plan with the dual licence and the FreeRTOS credit; a
version tag, `release-plz` and the org's `portfolio-check` are in the flipping
commit and the crates.io name is reserved; and **the owner runs the flip**.
`kairos deploy` refuses without `--public|--private`.

### 2.12 Non-goals

No rewrite of Espressif's radio blob, RF calibration or ROM; no replacement of
IDF-FreeRTOS under Track A in v1 (the C ABI is the bridge, and it is K8+);
no co-routines; no `configUSE_ALTERNATIVE_API`; no 16-bit or 8-bit ports; no
Cortex-A/AArch64 in v1; no POSIX-complete OS; no Linux; no Percepio recorder
or Tracealyzer format; no AWS IoT libraries (the mesh, mID and `janus/ota/1`
are the house answers); no second TCP/IP engine beside smoltcp unless the
K7 measurement says the API cannot be met on it; no "no C on the device"
claim while a radio blob is linked; no performance claim without a ledger row;
no agent acting on a fleet without a capability and an approval.

---

## 3. The product surface

### 3.1 What a firmware author writes

The API is FreeRTOS-shaped: same nouns, same semantics, Rust ownership. A
developer who knows `xTaskCreate` reads `Task::spawn` without a manual.

```rust
use rusty_rtos::prelude::*;

type Cfg = rusty_rtos::config::Default;          // or a custom `impl Config`
static KERNEL: Kernel<Cfg, PortC6> = Kernel::new();

#[entry]
fn main() -> ! {
    let k = KERNEL.init(PortC6::take(peripherals));       // the WRAP crate hands over the CPU
    let q: Queue<Reading, 8> = k.queue();                  // static capacity, no heap
    k.task("sensor", Priority::new(3)?, 1024)
        .spawn(move |cx| loop {                            // cx: TaskContext — delay, notify, yield
            let r = read_sensor();
            q.send(r, Timeout::ticks(10)).ok();
            cx.delay(Duration::millis(100));
        })?;
    k.task("radio", Priority::new(2)?, 2048)
        .spawn(move |cx| loop { if let Ok(r) = q.receive(Timeout::Forever) { push(r); } })?;
    k.timer("blink", Duration::millis(250), Periodic, |_| toggle_led())?.start();
    k.start()                                              // never returns; the idle task runs
}
```

The names map one to one (`xTaskCreate` → `task().spawn`, `vTaskDelay` →
`cx.delay`, `xQueueSendFromISR` → `q.send_from_isr(isr_token, …)`,
`xTaskNotifyGive` → `handle.notify_give()`, `xEventGroupWaitBits` →
`group.wait(bits, Wait::All, Clear::Yes, timeout)`, `xStreamBufferSend` →
`sb.send(&bytes, timeout)`). The full mapping table — roughly 430 public
declarations and API macros across `task.h`, `queue.h`, `semphr.h`,
`timers.h`, `event_groups.h`, `stream_buffer.h`, `message_buffer.h` on
2026-09-09 — is the API contract, written in K0 as
`docs/API-MAP.md` with a status column (`done` / `planned` / `never`, with the
reason) that CI checks against the facade's exports.

### 3.2 The config

`FreeRTOSConfig.h` becomes a type. Geometry knobs are associated consts on a
`Config` impl; subsystems are cargo features on the facade (`mutexes`,
`recursive-mutexes`, `counting-semaphores`, `queue-sets`, `timers`,
`event-groups`, `stream-buffers`, `task-notifications`, `tls`, `stats`,
`trace`, `tickless-idle`, `smp`, `mpu`, `static-alloc` on by default,
`dynamic-alloc`); hooks are a trait; a handful are NEVER and documented. A
`kairos config --from FreeRTOSConfig.h` verb translates an existing header
into a Rust `Config` impl and lists what it dropped, so a migration is a
diff, not a reading exercise. Every knob obeys `parameterizing-a-constant`:
tests are written in the unit the code counts, and each knob has one test
that asserts its default so a change is a visible diff.

### 3.3 The verbs (one binary, the same flag matrix as `janus`)

| verb | job |
|---|---|
| `kairos status [--ci]` | what exists on disk, what is a repo, what has a remote, CI's verdict |
| `kairos check [PKG..] [--clippy] [--test] [--sim] [--qemu] [--xtensa]` | host workspaces; every `no_std` crate on every listed target; the demo corpus on the sim port; the QEMU cells; the Xtensa rungs locally |
| `kairos new <name>` | stamp a package from `tools/kairos/templates/function` and register it in `KAIROS.toml` |
| `kairos deploy <pkg> --public\|--private` | commit, create the GitHub repo with the visibility you name, push; refuses a manifest mismatch |
| `kairos secrets` | fan `KAIROS_GIT_TOKEN` to every sibling-consuming repo through `gh secret set` on stdin |
| `kairos oracle build\|trace <demo>` | build the pinned C kernel + Posix port with the trace patch; capture a deterministic trace for a demo scenario |
| `kairos conform [--cell C]` | run the demo corpus against the oracle traces (sim) or the check task (QEMU/board) and update the matrix |
| `kairos config --from FreeRTOSConfig.h` | translate a C config header into a `Config` impl |

`--json` on every verb; exit codes carry the verdict (2 refused, 3 does not
fit, 4 panicked, 5 the expected line never came, 6 a trace diverged, with the
line). The tool is `tools/kairos` (crate `kairos-cli`, binary `kairos`,
`publish = false` because `kairos` is taken on crates.io — the same shape as
`janus-cli`), copied from `tools/janus` and renamed; generalising the two into
one fleet tool is an upstream item for Janus (§5.4).

### 3.4 The C ABI — the drop-in story

`rusty_rtos-capi` exports the stable FreeRTOS symbols (`xTaskCreate`,
`vTaskDelay`, `xQueueCreate`, `xQueueSend`, `xSemaphoreTake`,
`xTimerCreate`, `xEventGroupWaitBits`, `xStreamBufferSend`, the `*FromISR`
family, `pvPortMalloc`/`vPortFree`, `vTaskStartScheduler`, …) as a
`staticlib`/`cdylib`, with `FreeRTOS.h`-compatible headers generated by
`cbindgen`, so a C program relinks. The gate is the strongest one this family
has: **the unmodified C standard demo tasks from `Demo/Common/Minimal`,
compiled with clang, link against `rusty_rtos-capi` on the Posix port and
their check task passes.** That is rusty_alloc's `mi_*` ABI + `LD_PRELOAD`
corpus and rusty_zstd's `capi`, transposed. It is also the road to Track A.

### 3.5 Deploy paths (reuse, never fork)

Bench: `espino run --board … --project firmware/…` flashes and monitors any
Kairos firmware on an ESP32 exactly as it does a Janus one — the board
records, the partition tables, the truthful monitor and the symbolizer are
espino's; Kairos adds nothing there. QEMU cells run from `kairos check
--qemu`. Cortex-M boards use `probe-rs` (an owner-only install) or the
board's own flasher; a `kairos flash` verb exists only if espino's `Port`/
`Flasher` seams cannot be reused, which is a decision to record, not a default.

---

## 4. Finished work

**K0 — the family exists (2026-09-09).** Eight private repositories under
`Remade-With-Rust` (`rusty_rtos_core`, `_kernel`, `_port`, `_heap`, `_json`,
`_backoff`, `_demo`, `rusty_rtos-capi`), each a clean clone that builds alone
and passes the fleet gate on the developer box (`kairos check --fmt --clippy
--test --deny`: 8 bare-metal rungs each); `rusty_rtos_core` carries the real
types (§2.5) with 34 host tests; the instrumented C oracle builds under WSL and
`kairos oracle trace dynamic` produces a 24,403-line trace that is
byte-identical across two runs, stored as `oracle/traces/dynamic.trace.zst`
(umbrella `docs/LEDGER.md`). Two items are
owner steps and stay open: CI runs on GitHub have not started because the
org's Actions billing is failing ("recent account payments have failed or
your spending limit needs to be increased"), and `KAIROS_GIT_TOKEN` is not on
the box, so the seven dependents' CI cannot fetch the private core until
`kairos secrets` is run. The kill test is therefore **passed locally, CI
pending the owner**.

**K1 — the scheduler agrees with the C kernel (2026-09-09). Passed.** All
nine scenarios of the conformance corpus produce traces **identical to the C
kernel's for 100,000 ticks each** — 8,408,764 lines, the counters included
(umbrella `docs/LEDGER.md`). One command is the whole gate:

```sh
kairos conform --all --ticks 100000
```

`dynamic`, `PollQ`, `BlockQ`, `semtest`, `countsem`, `recmutex`, `blocktim`,
`QPeek`, `GenQTest`. Between them they exercise dynamic priorities and
suspension, polled and blocking queues, binary and counting semaphores,
mutexes with priority inheritance and their recursive variant, block times
to the tick, peeking and the order four priorities wake in, both ends of a
queue, and `xTaskAbortDelay` dragging a blocked task off a mutex so its
holder has to disinherit down to whoever is still waiting rather than to its
own base priority.

What exists behind it: `rusty_rtos_port`'s deterministic sim port (sim
contract v1); `rusty_rtos_kernel`'s scheduler — ready lists, both delayed
lists, pending-ready and suspended, the tick, the context switch, queues
with their two event lists, semaphores, mutexes with inheritance and
disinheritance-after-timeout, and `vTaskDelay` / `vTaskSuspend` /
`vTaskResume` / `vTaskPrioritySet` / `vTaskSuspendAll` / `xTaskResumeAll` /
`xTaskAbortDelay` following the C control flow statement for statement;
`rusty_rtos_demo`'s runner, trace sink and 34 demo tasks as resumable state
machines; and `kairos conform`, the gate. Miri is green on the core, the
kernel, the port, and over all nine scenarios.

The arena-and-list cost row is taken too, and it is the one number that did
not come out where the plan hoped: **2.08×** the C list, against a 1.25×
revisit line. §5.2 item 2 is therefore open, with the three ways out and the
diagnosis in the ledger.

Not done in K1: the Kani harnesses for the CBMC proof list (K2's), and
anything on silicon (K3's).

**K0 checklist** (the family exists — the kill test is a clean clone of any
package building alone with green CI):

- [x] `git init` the umbrella; `KAIROS.toml`; `.cargo/config.toml` with `[net]
      git-fetch-with-cli = true` only; `.gitignore` ignoring `/rusty_rtos_*/`,
      `/kairos-*/`, `**/target/`; `tools/kairos` copied from `tools/janus`
      (manifest name, token name, template path changed; `--sim`, `--qemu`,
      `oracle`, `conform`, `config` verbs added over time). The two Janus
      Python scripts are Rust verbs here: `kairos patches` (the sibling
      overrides, from `KAIROS.toml`'s `uses`) and `kairos harden`
      (the hardening-table renderer); the tool runs on the house allocator
      seam and prints `rusty_symbols` glyphs.
- [x] `ORACLES.md`: the pinned commits of FreeRTOS-Kernel V11.3.1 and the
      202604.01-LTS libraries; the QEMU version; how each is fetched.
- [x] `docs/API-MAP.md`: the 339-row API contract with a status column;
      `docs/CONFIG-MAP.md`: the 94 `config*` knobs classified const / feature /
      hook / never.
- [x] `rusty_rtos_core` with real types and tests (§2.5), checked on
      `thumbv7em-none-eabihf`, `thumbv8m.main-none-eabihf`, `riscv32imac`,
      `riscv32imafc`, with and without `alloc`; `rustup target add` the two
      thumb targets on this box.
- [x] Packages stamped (kernel, port, heap, demo, capi, json, backoff — the
      rest at their phase), each its own git repo; all deployed **private**;
      `rusty_rtos_core` first (2026-09-09; CI runs blocked on the org's
      Actions billing and on `kairos secrets` — owner steps).
- [x] The instrumented C oracle builds on this machine (`gcc` under WSL —
      the Posix port needs pthreads and signals, which the native `clang`
      MSVC target lacks; no CMake, one compiler line the tool owns),
      deterministic tick patch applied by `kairos oracle patch`, producing a
      reproducible trace for the `dynamic` demo (`kairos oracle trace
      dynamic` runs it twice and refuses a differing pair).
- [x] Every repo carries `deny.toml`, `SECURITY.md`, the hardening plan file
      and the rendered README table, and a `docs/LEDGER.md` with its first
      row (the build fact); the umbrella's `docs/LEDGER.md` carries the oracle
      row.

---

## 5. Remaining work

### 5.1 The next bricks, in order (host work unless marked)

1. **K0** — the family exists (§4).
2. **K1 — a scheduler on the sim.** `rusty_rtos_kernel-core`: the arena, the
   ready lists, priorities, preemption, time slicing, `delay` / `delay_until`,
   suspend/resume, task notifications, TLS, idle + hooks; `rusty_rtos_port-sim`;
   the `Trace` sink in the oracle's line format. Demo corpus subset:
   `dynamic`, `death`, `blocktim`, `AbortDelay`, `TaskNotify`,
   `TaskNotifyArray`, `flash`, `integer`, `flop`. **Kill test:** each
   scenario's trace diffs clean against the C oracle for 100 000 ticks;
   context-switch and tick counters equal on both sides; Miri runs the core
   suite; the K1 ledger row records the arena-list vs C-list cost as counts
   (instructions retired on the host under callgrind, or `--emit asm` counts
   of the ready-list ops) — the §2.5 decision's first number.
3. **K2 — the IPC and the timers.** Queues (copy semantics, `overwrite`,
   `peek`, registry), semaphores (binary, counting), mutexes with priority
   inheritance and the recursive variant, queue sets, event groups, software
   timers + the daemon task, stream and message buffers, the `FromISR`
   variants with the ISR token and the "higher priority task woken" return.
   Corpus: `BlockQ`, `GenQTest`, `PollQ`, `QPeek`, `QueueOverwrite`,
   `QueueSet`, `QueueSetPolling`, `semtest`, `countsem`, `recmutex`,
   `IntSemTest`, `TimerDemo`, `EventGroupsDemo`,
   `StreamBufferDemo`, `StreamBufferInterrupt`, `MessageBufferDemo`,
   `MessageBufferAMP`. `IntQueue` is **not** in the corpus: it needs nested
   interrupts at different priorities, which a signal-driven host port does
   not have, and upstream's own Posix demo does not build it either.
   **Kill test:** all traces clean for 100 000 ticks; the
   Kani harnesses for the CBMC `Queue` and `Task` proof list pass; the
   no-panic property test over every API with random handles/ticks passes;
   `cargo mutants` on the scheduler core reports its score in the ledger.
4. **K2.1 — the Rust face.** The C-shaped API is what the corpus proves;
   it is not what a Rust application should have to hold. This kiln puts a
   safe, ownership-typed API on the *same proven kernel*, as a face beside
   `rusty_rtos-capi`'s C face, so that the conformance corpus keeps gating
   the scheduler underneath both. Three things, in this order, each one
   additive and none of them touching the scheduler:

   1. **IPC that moves values.** `Queue<T, N>` transfers ownership instead
      of copying bytes; `Mutex<T>` *owns* what it protects, so reaching the
      data without the guard does not compile. This is the delta the C
      cannot have: a FreeRTOS mutex is an advisory token that protects
      nothing, and `MessageBufferAMP` — which passes a *pointer* through a
      message buffer — is the monument to what that costs.
   2. **The ISR surface as a capability.** A from-ISR call takes a token
      obtainable only in interrupt context, so calling the wrong half is a
      missing argument rather than a `configASSERT` at best and silent
      corruption at worst.
   3. **Compile-time topology.** Tasks and queues named by type rather than
      by runtime handle: no create-failure path, exact `.bss` by
      construction, and — measured in K2, not hoped for — the Kani
      harnesses converge. Ten of the thirty-five blow up today, and the two
      causes were measured on 2026-09-10 rather than assumed:

      * Five need a **started kernel**, and that is not a topology problem
        at all. `start_scheduler` is concrete — CBMC cannot afford it with
        no symbolic input — so naming things by type does nothing for it.
      * Five pass a **symbolic handle** into a call that must then be
        explored for every arena slot it could name. Moving the check to
        the door was tried and does **not** help; the cost is the handle
        meeting the arena, wherever the check sits.

      So this item is an **addition to the API, not a change inside the
      kernel**: a static face that declares its tasks and queues, on the
      same kernel the corpus proves. The dynamic face keeps taking a handle
      that may name nothing, because that is its contract; the static one
      never mints a bad one, which is what makes those five harnesses
      unwritable rather than slow.

   **Kill test:** every corpus scenario still trace-identical for 100 000
   ticks with the Rust face compiled in — the face costs **zero** extra
   critical-section exits, and the ledger says so by naming the scenario
   whose exits were counted on both sides; a `typed` demo arm re-implements
   one scenario against the Rust face and passes its own
   `xAre...StillRunning()`; the misuse a C caller can commit and a Rust
   caller cannot is a `trybuild` row per bug class (unguarded shared data,
   from-ISR from a task, item-size mismatch); the Kani harness count rises
   with the topology work and the number is in the ledger.

   **What this kiln deliberately does not do:** `async` task bodies. The
   compiler would write the continuations K2 wrote by hand six times over
   (`WaitFrame`, `stream_resume`, `owed_exits`, `OwedTrace`, the AMP
   handler's `stage`, every body's `pc`), and that is the most attractive
   thing on this list — but it is also where the identity drifts: tasks as
   futures is nearer Embassy-with-priorities than FreeRTOS, and the corpus
   may no longer gate it. It is K2.2, behind a one-scenario prototype whose
   job is to answer whether the await points land exactly where the
   `Wait::Blocked` points are today. If they do, we keep both and the
   corpus keeps working. If they do not, that is a decision to take with a
   number in hand.

5. **K2.2 — `async` task bodies.** Answered: see the phase table and the
   ledger. The law it found is the one K2 met six times — a suspension
   point belongs after every kernel call, not only the blocking ones — and
   `async` obeys it once told. Storage — the difference between one
   scenario and an executor — went the same day: a future of unnameable
   type lives in a task slot as a `Pin<&mut dyn Future>` the caller pins
   with `core::pin::pin!` and lends, which costs no allocator, no `unsafe`
   and nothing unstable, and which forces the kernel to be the caller's
   rather than the runner's. `async` is a body kind beside the hand-written
   ones now, and all 18 arms still trace-identical says it took nothing
   with it.

6. **K3 — silicon and an emulator** (bench + QEMU). `-cortex-m` on QEMU
   `lm3s6965evb` (FreeRTOS's own QEMU demo is the C arm of the A/B);
   `-riscv` on QEMU `virt` and then on an ESP32-C6 devkit through esp-hal
   (stable Rust, no espup). **Kill test:** the check task of the full corpus
   passes for one hour on each cell; context-switch cycles, tick-ISR cycles
   and ISR-to-task latency measured with the same counter on our binary and
   the C demo built from the pinned source, ABBA, null arm, in the ledger;
   flash and RAM decomposed per `footprint-decomposition` beside the C
   build's map file. Cells `M3-qemu`, `RV32-qemu`, `C6` flip to `Verified`.
7. **K4 — the heaps and the allocator.** `heap_1`, `heap_4`, `heap_5` with
   the protector; `heap_3` as the seam over `rusty_alloc` small-metal
   (2.0.4, `prim::fixed::Region<N>`) and `esp-alloc`; static allocation
   first-class (`StaticAllocation` demo). **Kill test:** the `heap_4`
   fragmentation behaviour matches C on a recorded alloc/free trace (same
   free-block walk, same coalescing — a differential test like rusty_alloc's
   G2); the `StaticAllocation` demo passes on every cell; the RAM table per
   profile is in the ledger.
8. **K5 — the Janus joint** (bench). `rusty_rtos_port-riscv` and `-xtensa`
   implement `esp-radio-rtos-driver` 0.4.1; `rusty_esp_signal/firmware/c6-mesh-node`
   is rebuilt on `rusty_rtos` in a sibling firmware dir; then the XIAO S3
   sketch firmware on the Xtensa port. **Kill test:** Janus S1 (two C6s,
   ESP-NOW authenticated link, 1 000 frames each way, counters) passes on
   the Kairos kernel with the same numbers as on `esp-rtos`, recorded in
   both ledgers; the Xtensa cell flips to `Verified` on the XIAO.
9. **K6 — the C ABI and the unmodified corpus.** `rusty_rtos-capi` +
   generated headers. **Kill test:** the 34 C demo task files from
   `Demo/Common/Minimal`, compiled unmodified with clang, link against the
   capi on the Posix port and the check task passes; the same on QEMU M3
   with `arm-none-eabi-gcc`/clang (an owner-only install).
10. **K7 — the libraries.** In this order, each its own package with its own
   external oracle: `json` (JSONTestSuite + coreJSON's own vectors,
   no-panic fuzz), `backoff` (the vectors from the C unit tests),
   `mqtt` (a real broker — Mosquitto — plus coreMQTT's CMock vectors),
   `http` (a real server plus coreHTTP's vectors), `sntp` (against
   `rusty_time`'s server, decision §5.5), `tcp` (the +TCP API over smoltcp;
   interop against the host stack; `iperf`-style throughput rows on the C6).
   **Kill test:** an MQTT session over our TCP on a C6 to a broker for one
   hour with zero lost keep-alives, and the JSON suite at 100 %.
11. **K8 — SMP, MPU, tickless idle, stats, hardening, 1.0.** `configNUMBER_OF_CORES
   > 1` on the ESP32-S3 (two cores) and QEMU; loom models for the SMP ready
   list; `rusty_rtos_mpu` on M33 (`mps2-an505`); tickless idle with the
   pre/post-sleep hooks; run-time stats and the trace facility as products;
   every hardening row `Completed` or waived; the public flips in §2.11 order.
   **Kill test:** the SMP corpus (`FreeRTOS-SMP-Demos`) traces clean; a
   privileged/unprivileged demo on M33 refuses a bad access; the family
   unsafe census in this plan; 1.0.0 tags.
12. **K9 — the rest of the portfolio** (`pkcs11`, `cellular`, `fat`, `posix`,
    `cli`), each a package with its oracle, after a consumer asks.
11. **Track A** — IDF-FreeRTOS replaced under esp-idf-svc through the C ABI:
    a decision item, opened only after K6 and only with Janus's owner.

### 5.2 Board and emulator rows — every claim only silicon or an emulator can make

Rules (Janus §5.2): a row moves to the repo's `docs/LEDGER.md` with its
method line the day it is measured; the README flips from "sim only" /
"builds, not flashed" to the number; a number that disagrees with a host
assumption changes the assumption and the decision log says so.

**Bench prerequisites** (one-time): QEMU (`qemu-system-arm`, `qemu-system-riscv32`)
installed; `rustup target add thumbv7m-none-eabi thumbv7em-none-eabihf
thumbv8m.main-none-eabihf`; a Cortex-M board with a debug probe (an STM32
Nucleo-F411 or nRF52840-DK) and `probe-rs`; two ESP32-C6-DevKitC-1 (already on
the Janus list); the XIAO ESP32-S3 Sense (in hand); the esp toolchain (in
hand); `clang` (in hand); `arm-none-eabi-gcc` or clang's baremetal target for
the C arm of the QEMU A/B.

| # | cell | unlocks | have |
|---|---|---|---|
| 1 | Posix port on this laptop | K1–K2 traces (the C arm runs here too) | **yes** |
| 2 | QEMU `lm3s6965evb` (Cortex-M3) | K3 Cortex-M numbers vs the C demo | install |
| 3 | QEMU `virt` RV32 | K3 RISC-V numbers vs the C demo | install |
| 4 | ESP32-C6-DevKitC-1 ×2 | K3 silicon, K5 the Janus joint (S1) | on the Janus purchase list |
| 5 | XIAO ESP32-S3 Sense | K5 Xtensa, K8 SMP (two cores) | **in hand** |
| 6 | QEMU `mps2-an505` (Cortex-M33) | K8 MPU / TrustZone | install |
| 7 | a Cortex-M4F board + probe | K3 on real Cortex-M silicon; FPU lazy stacking | buy |

**Rows** — [x] K1 arena-list cost vs C list (host counts): 46.45 vs 22.32
instructions per list operation, 2.08×, callgrind under WSL2
(`bench/list-cost/run.sh`, 2026-09-09) · [ ] K3 context
switch cycles M3-qemu, RV32-qemu, C6 · [ ] K3 tick ISR cycles · [ ] K3
ISR-to-task latency · [ ] K3 flash + RAM per profile vs the C map · [ ] K4
`heap_4` differential trace · [ ] K5 S1 on Kairos vs esp-rtos · [ ] K5 Xtensa
context switch on the S3 · [ ] K6 unmodified C corpus on Posix, then QEMU ·
[ ] K7 MQTT one-hour session on the C6 · [ ] K7 TCP throughput on the C6 ·
[ ] K8 SMP corpus on the S3 · [ ] K8 MPU refusal on M33 · [ ] power per
firmware on the USB meter (idle, tickless idle, station) — the Janus row,
reused.

### 5.3 Owner-only steps

| step | unblocks |
|---|---|
| Decide the family name (Kairos or otherwise) and the crates.io reservations (`rusty_rtos`, `rusty_rtos_core` first) | the manifest's `[kairos]` table; `cargo add` by name at the first flip |
| Mint a fine-grained PAT (Contents: Read on every `rusty_rtos_*`, and on `rusty_esp_core`, `rusty_esp_signal` for K5) and run `kairos secrets` | CI in every sibling-consuming repo |
| Install QEMU and a Cortex-M target; buy a Cortex-M board and probe | K3 and K8 cells |
| Approve the `esp-radio-rtos-driver` adapter landing in a Janus firmware (K5) and the S1 re-run | the Janus joint |
| Decide the +TCP engine (§5.5) and the SNTP source (§5.5) | K7 |
| Run each public flip (§2.11) | market |

### 5.4 The upstream queue

| item | where | state | when it lands |
|---|---|---|---|
| Generalise the fleet tool (`janus` and `kairos` are one binary reading `<NAME>.toml`) | janus `tools/janus` | not filed | Kairos ships with a copy until then |
| `esp-radio-rtos-driver` adapter on a non-esp-rtos scheduler | esp-rs (docs / a reference impl) | not filed | K5 opens it if the trait's contract is under-documented |
| `rusty_alloc` `prim::fixed` on Cortex-M (small-metal was measured on the S3 only) | rusty_alloc | not filed | K4's `heap_3` seam on the M3 cell |
| `rusty_time` `no_std` leaf for the SNTP client: `rusty_time-core` 0.1.10 carries the `no-std` category but has no `#![no_std]` (it needs `std` on `thumbv7em` / `riscv32imac`, measured 2026-09-09 in `tools/house-gate`) | rusty_time | drafted (`docs/upstream/rusty_time-no-std-leaf.md`), not filed | decides §5.5 item 4; until it lands, `rusty_rtos_sntp` is a coreSNTP remake |
| ~~`rusty_zstd` 0.2.3 `no_std + alloc` uses `core::sync::atomic::AtomicU64`~~ | rusty_zstd | **resolved upstream in 0.2.5**, never filed; re-measured 2026-09-09 (`gate-zstd` exit 0 on both bare-metal targets), house-gate pin moved to `=0.2.5` | on-chip compression (OTA, trace capture) is unblocked; the fleet tool still links 0.2.3 on the host, which is a pin to align, not a blocker |
| `rusty_erasure-core` 0.4.0 imports `AtomicU64` in its `no_std` build; same 32-bit gap | rusty_erasure | drafted (`docs/upstream/rusty_erasure-atomic-u64.md`), not filed | nothing in Kairos v1 needs it; recorded so the ladder is honest |
| smoltcp: any API gap the +TCP surface needs (e.g. socket-set sizing from a `Config`) | smoltcp | not filed | K7 |
| FreeRTOS upstream: a deterministic-tick option for the Posix port (our oracle patch) | FreeRTOS-Kernel | not filed | the patch stays in-tree either way |

### 5.5 Open decisions

1. **The name.** Kairos is the recommendation (the opportune moment; the
   Greek sibling of Janus's naming); the crates.io name `kairos` is taken by an
   unrelated crate, which only affects the tool (`kairos-cli`, never
   published). Alternatives: keep `rusty_rtos` for everything.
2. **Arena lists vs intrusive lists** (§2.5). Decided for the arena; the K1
   ledger row was the revisit condition, and **it has fired**: 46.45
   instructions per list operation against `list.c`'s 22.32, or 2.08×,
   where 1.25× was the line. The row also says where the cost is, and it is
   not the index arithmetic — it is a branch and a bounds check on every
   link access, several of them re-validating a handle the same call already
   validated. So the choice is three ways, not two: reaffirm the decision
   with this price written down; spend a K2 task on the two changes the
   ledger names (end markers in the item array, validate once per call) and
   re-run `bench/list-cost/run.sh`; or open the fenced `-core-unsafe` twin
   gated by the same nine traces. The first two keep `forbid(unsafe)`.
3. **The +TCP engine.** smoltcp as the engine with the FreeRTOS-shaped API
   above it (recommended: pure Rust, `no_std`, heap-free, already the engine
   under embassy-net), versus a from-scratch remake for a 1:1 code map.
   Decided at K7 with a measurement: if the +TCP API cannot be met on
   smoltcp's socket model, the remake is opened as a package plan.
4. **SNTP.** A `no_std` leaf of the house `rusty_time` (chrony remake) versus
   a coreSNTP remake. Recommended: the leaf, if `rusty_time` can expose one
   without `std`; else the remake (it is small). **Measured 2026-09-09:**
   `rusty_time-core` 0.1.10 is `std`-only on the bare-metal targets (the
   umbrella's `tools/house-gate`), so the remake is the default until the
   upstream leaf exists (`docs/upstream/rusty_time-no-std-leaf.md`).
5. **Tick width and `TickType_t` at the C ABI.** The ABI must pick one width
   per build (a `capi` feature); the Rust API is generic.
6. **A self-hosted runner** for the Xtensa gate and the QEMU cells (the
   laptop, the Janus Pi hub when it exists, or neither until boards).
7. **Whether `rusty_rtos_demo` also carries an `embassy` comparison arm** so
   the "why not Embassy" question has a number: an async executor is not an
   RTOS, but a maker will ask; a ledger row beats an argument.
   **Answered in part 2026-09-10:** K2.1 settles the shape of the question.
   The family is to be *both* the RTOS you can trust — the C-shaped kernel
   the corpus proves — and the one the others would want to be: a Rust face
   over that same kernel where the type system holds the invariants the C
   only asserts. Embassy is async and cooperative; RTIC is stack-resource
   policy with no blocking and no dynamic tasks; FreeRTOS is preemptive
   priority with inheritance, in C. **Preemptive priority scheduling with
   ownership-typed IPC and compile-time topology, in safe Rust, is empty
   ground**, and this family is the only one with the proven preemptive
   core already standing. The comparison arm is still wanted, and K2.2 is
   where it gets its number.

---

## 6. Phases and kill tests

| phase | kill test | state |
|---|---|---|
| **K0 family** | a clean clone of any package builds alone and its CI is green; the C oracle produces an identical trace twice | **passed locally 2026-09-09** (8 repos, fleet gate green on the box; `dynamic` trace identical twice); CI green pending the owner's Actions billing and `kairos secrets` |
| **K1 scheduler on sim** | nine demo scenarios trace-identical to the C kernel for 100 000 ticks; counters equal; Miri green; the arena-list cost row | **passed 2026-09-09** — nine of nine, 8,408,764 lines identical at 100 000 ticks, ticks/yields/exits equal on every one; Miri green over the whole corpus; the cost row taken and **2.08×**, which fired §2.5's revisit condition — cut to **1.533×** on 2026-09-10 by reading each node once per call, still above the 1.25× bar, so the decision stays open (`docs/LEDGER.md`) |
| **K2 IPC + timers** | the remaining demo scenarios trace-identical; Kani harnesses for the CBMC proof list pass; no-panic property test; mutants score ledgered | **14 of 18 passed 2026-09-09.** The corpus is sixteen scenarios in all (the other two are K1's) and 12,808,722 lines identical at 100 000 ticks. In the kernel: the from-ISR surface, queue sets, real queue lock counts, task notifications, stream and message buffers with their own arena, the software timers with their daemon, event groups, and the `sbSEND_COMPLETED` seam. The no-panic gate passes; Kani verifies seven harnesses (3,965 checks) and does not converge over a started kernel; `cargo mutants` with the corpus as the oracle catches 36 of 42 viable mutants in the scheduler core. Of the four not passed, **every one is blocked above the kernel** — `IntQueue` needs nested interrupts this port does not have; `MessageBufferDemo` and `StreamBufferDemo` starve sim contract v1 (a task that polls without entering a critical section stops the clock); `QueueSet` seeds its PRNG from a stack address. All of them and their evidence are ledger rows |
| **K2.1 the Rust face** | every corpus scenario still trace-identical at 100 000 ticks with the Rust face compiled in, at **zero** extra critical-section exits; a `typed` demo arm passes its own check; one `trybuild` row per bug class the C API cannot refuse; the Kani count after the topology work in the ledger | **passed 2026-09-10.** `PollQ-typed` is `PollQ` written against `Queue<u16, 10>` and diffed against **`PollQ`'s own C oracle trace**: 117,417 lines identical at 100 000 ticks, `exits=105608` on both sides, and the same pinned digest offline. Six bug classes the C cannot refuse are compile errors — item size, item type, a failed send whose value cannot be dropped, the from-ISR half from a task, the task half from an interrupt, and shared data touched without the lock — each a `compile_fail` doctest paired with the working line it is one character from, so none can pass for the wrong reason (`compile_fail` rather than `trybuild`: it is built in, and the house does not add a dependency for what the toolchain already does). Kani is 25 of 35, the three new ones being the face's own properties at 1–3 s each where the raw kernel's nearest equivalent does not converge in 700 s. **Re-taken 2026-09-10 and it holds** — but the suite had stopped compiling when `Raw` grew for `Mutex<T>` and nothing noticed, because `#[cfg(kani)]` is invisible to every other gate; `kairos check --kani` now closes that, and the sweep needs `--exact` or Kani's substring filter runs three harnesses inside one budget. **Still owed:** the topology work, which the same day's measurements redirect — the arena indirection is not worth removing for speed, the started-kernel harnesses are not a topology problem, and the symbolic-handle ones need the *static* face of §5.2 item 3 rather than anything inside the kernel (`docs/LEDGER.md`) |
| **K2.2 async task bodies** | one scenario re-written as `async fn` whose await points land exactly on today's `Wait::Blocked` points, and the corpus still gates it — or a ledger row saying why it cannot and what that costs | **passed 2026-09-10, with its own premise corrected.** The await points must *not* go where the `Wait::Blocked` points are: `PollQ` blocks nowhere, so a body that awaits only there never suspends and `poll` never returns — measured, it hangs at 200 ticks, and the runaway guard cannot fire because the loop is inside a single poll. They go after **every** kernel call, which is where a `pc` arm ends and for the same reason. With that, `PollQ-async` is identical to `PollQ`'s own C oracle trace: 117,417 lines at 100 000 ticks, `exits=105608`, and the same digest offline. **Storage, settled the same day:** `Body::Async` holds a `Pin<&mut dyn Future>`, the caller pins with `core::pin::pin!` and lends it — no allocator, no `unsafe`, nothing unstable. It forced the kernel and the statics out of the runner and into `RefCell`s the caller holds, because a future cannot borrow a field of the struct that polls it; all 18 arms are still identical at 100 000 ticks, which is what says the refactor was safe. `async` is now a body kind beside the others, not a prototype |
| **K2.3 the static face** | a declaration whose geometry is the declaration: exact `.bss`, no create-failure path an application can reach, and the symbolic-handle proofs unwritable against it | **passed 2026-09-10.** `system! { mod app use Cfg; tasks {..} queues {..} }` writes `TASKS`, `QUEUES`, `SLOTS`, `ITEMS` and `LISTS` from the declaration. As a counter rather than an adjective: **2,088 bytes against 13,600 hand-sized, 6.5×** (`size_of`, exact, compile-time), and a test that fills every declared slot and asserts the next send is refused, which fails if `SLOTS` was rounded either way. Every create-failure path is closed by construction and the one that is an application mistake — a priority the config has no ready list for — is a `const` assertion, so a build failure rather than a `configASSERT` on the bench. Two more `compile_fail` doctests with their controls. **Still owed:** timers and event groups are not declarable yet, and no corpus arm runs on it — it is proven by unit tests and `size_of`, not by the C oracle |
| **K3 silicon + QEMU** | the full corpus check task passes one hour on M3-qemu, RV32-qemu and a C6; context switch / tick / latency cycle rows vs the C demo **from the C6, not from QEMU**; flash + RAM decomposition | **part done 2026-09-10, and its measurement half re-specified.** **`rusty_rtos_port-cortex-m` exists and switches**: two tasks with real stacks under QEMU, 100/100 resumptions over 200 switches, each re-checking every word of its own stack and a callee-saved register — and the test is poison-proven, since deleting `stmdb r0!, {r4-r11}` reports a corrupted witness at word 0. It is the first crate in the family to lift the `unsafe_code` deny, per item, with `UNSAFE.md` carrying every one. **And the kernel now drives it**: `mps2-an385-qemu-kernel` has `PendSV` call `Kernel::switch_context` and `SysTick` call `increment_tick`, so the same fixed-priority scheduler the corpus proves runs real tasks on ARMv7-M — 40 high-priority laps against 402 ticks and a 10-tick `delay`, which is the number only a priority scheduler with a working block produces, and 0 early wakes. Poison-proven at 1 lap against 120.  **The corpus runs on a Cortex-M3 AND on RV32**: `rusty_rtos_demo/firmware/mps2-an385-qemu-corpus` and `.../riscv32-qemu-corpus`, **all 17 scenarios on each** — every counter, the FNV-1a/64 digest and the byte count matching the host's pins, which are themselves diffed against `oracle/traces/*`. So the corpus is byte-identical to C FreeRTOS on **three** architectures: the host, ARMv7-M and RV32. Both cells and the host test read one pin table (`rusty_rtos_demo_core::pins`), so a cell cannot silently disagree with the host about what the C kernel said, and both are poison-proven at one `exits` off. **And the one-hour clause is met on the host**: `tests/soak.rs`, via `kairos check rusty_rtos_demo --soak`, runs all 17 for **3,600,000 ticks** — an hour at the oracle's own `TICK_RATE_HZ` of 1000 — and every scenario's check task still reports running, with `ticks >= 3,600,000` asserted so a scenario that stopped early cannot pass. It claims **liveness, not conformance**: there are no C pins at 3.6M ticks, so it asks what the C demo asks. Poison-proven by sizing the step limit for 2,000 ticks. **The hour is met on M3-qemu and RV32-qemu too**, behind `--features soak` on each cell — a feature and not a second binary, so the gate's plain `cargo run --release` keeps meaning the 2000-tick conformance run. 17/17 on each, exit 0. And the three sets of counters were diffed by machine: **host, Cortex-M3 and RV32 agree on ticks, yields and exits for all 17 scenarios at 3,600,000 ticks** — cross-architecture determinism at 1800x the pinned length, though our builds against each other rather than against C, since no C pin exists at that length. They are background runs rather than gates (hours per architecture). **Only the C6 clause remains, blocked on the same hardware B4b needs.** So this is agreement with **C FreeRTOS to the byte on ARM**, and it needed **no context-switching port**: a scenario is a state machine, so a task needs no stack of its own. That the corpus is portable is a consequence of the K2 design — locals live in the TCB rather than on a C stack — rather than a trick. The M3 cell exists and gates — `rusty_rtos_core/firmware/mps2-an385-qemu-region`, 9/9, QEMU exit 0 — on `mps2-an385` rather than `lm3s6965evb`, because `MIN_REGION` (64 KiB) is the LM3S6965's entire SRAM. But **QEMU can supply no cycle, latency or work counter**: DWT is unimplemented there, SysTick's deltas are host wall time and *shrink* as the work grows, and under `-icount` the clock does not advance at all — measured six ways, clock source pinned, in that firmware's README. So the QEMU cells carry **correctness** (and an exit code, so they gate) while the **cycle rows come from the C6**, which is a Kairos target with a real `mcycle`. Flash and RAM decomposition are unaffected — properties of the binary, not of execution — and **that third of the kill test is done**: the M3 cell decomposes to `.bss` 198,672 with remainder 0, of which the declared `Region` is 196,608, so the seam costs **2,080 bytes of RAM and 22.5 KiB of flash** beyond the heap a firmware declares (`docs/LEDGER.md`) |
| **K4 heaps** | `heap_4` differential trace matches C; `StaticAllocation` on every cell; RAM table per profile | open |
| **K5 the Janus joint** | Janus S1 on a Kairos kernel with the same numbers as on esp-rtos; the XIAO S3 Xtensa cell `Verified` | open |
| **K6 C ABI** | the unmodified C standard demo tasks link against `rusty_rtos-capi` and pass on Posix, then on QEMU M3 | open |
| **K7 libraries** | MQTT over our TCP on a C6 to a broker for one hour, zero lost keep-alives; JSON suite 100 %; no-panic fuzz on every parser | open |
| **K8 SMP / MPU / 1.0** | SMP demos trace clean on two cores; MPU refusal on M33; every hardening row complete; 1.0.0 tags; flips in order | open |

The sequence is fixed: **sim → IPC → silicon → heaps → Janus → C ABI →
libraries → SMP**. Nothing on a chip before the trace is clean on the sim;
nothing in a library before the kernel it needs has a `Verified` cell.

---

## 7. Numeric targets — the exit bars

Standing targets, revisited only with a ledger argument. "C" means the
FreeRTOS-Kernel V11.3.1 demo built by us from the pinned source, on the same
emulator or board, measured with the same counter, ABBA, null arm.

| regime | target vs C | when |
|---|---|---|
| **Context switch** (`taskYIELD` between two ready tasks, cycles) | ≤ 1.25× at K3; ≤ 1.10× at K8; parity is the stretch | K3 / K8 |
| **Tick ISR** (cycles, N ready tasks, no wake) | ≤ 1.25× at K3; ≤ 1.10× at K8 | K3 / K8 |
| **ISR-to-task latency** (`xQueueSendFromISR` to the receiver running, cycles) | ≤ 1.25× at K3 | K3 |
| **Queue send/receive** (16-byte item, cycles) | ≤ 1.25× at K3 | K3 |
| **RAM** (TCB + queue + timer control blocks; `minimal` profile static footprint) | ≤ 1.20× the C map at K4; every byte decomposed | K4 |
| **Flash** (the `flash` demo, `minimal` profile, `opt-level = "z"`, LTO) | ≤ 1.30× C at K3; ≤ 1.15× at K8 | K3 / K8 |
| **API coverage** | 100 % of the `docs/API-MAP.md` rows not marked `never`, CI-checked | K2 (kernel), K7 (libraries) |
| **Config coverage** | every one of the 94 `config*` knobs classified and the `default` profile reproducing the C `template_configuration` behaviour | K0 |
| **Conformance** | 34/34 demo task sets trace-identical on sim; check task green on every `Verified` cell | K2 / K3 |
| **Unsafe surface** | zero in Layer 0 and every `-core`; every port block inventoried; count published per release, non-increasing without a decision row | every release |

A number in our favour gets the arm-duration and work-parity checks first
(`codec-measurement` §5); a number against us is a finding, not a caveat.

---

## 8. Decision log — the rows that bind

| date | decision |
|---|---|
| 2026-09-09 | The family mirrors the Janus build architecture whole: an umbrella that is not a workspace; `KAIROS.toml`; a fleet tool copied from `tools/janus`; one independent repo per package, siblings by git URL with a version; per-repo generated sibling overrides; everything private first; `deploy` needs an explicit visibility. |
| 2026-09-09 | The taxonomy: `rusty_rtos_{core, kernel, port, heap, mpu, tcp, mqtt, http, json, sntp, backoff, pkcs11, cellular, fat, posix, cli}` + `rusty_rtos_demo` + `rusty_rtos-capi` + `kairos`. One package per FreeRTOS LTS repository; Labs after 1.0; AWS libraries never. |
| 2026-09-09 | **The C kernel's trace is the oracle.** FreeRTOS-Kernel V11.3.1 on its Posix port with a deterministic tick, every `trace*` macro printing a line, is the reference; our kernel's `Trace` seam prints the same line; a demo scenario is conformant when the traces diff clean. The standard demo tasks are the corpus; the CBMC/VeriFast proofs are the Kani/loom list. |
| 2026-09-09 | **Handles are indices, never pointers.** Every kernel object lives in an arena; lists are index-linked; the core is `forbid(unsafe)`. The revisit condition is a K1 ledger row above 1.25× the C list cost. |
| 2026-09-09 | **The revisit condition fired.** The K1 row is 2.08× (46.45 vs 22.32 instructions per list operation, callgrind, `bench/list-cost/run.sh`). The decision above is therefore open, as §5.2 item 2 sets out; it is the owner's to close and nothing downstream assumes either answer. |
| 2026-09-09 | **A switched-out call's tail belongs to its task, not to the clock.** A thread stops at the switch; a stackless call runs to the end of the frame the scheduler abandoned. The port stops counting that tail's critical-section exits as sim time and tallies them (`Port::begin_unwind` / `end_unwind`); the kernel replays the tally when the task next runs. Counting rather than discarding is load-bearing: a tail can open a section of its own, and `xQueueReceive`'s timeout path does. |
| 2026-09-09 | **The C heap's cost is a config fact, not a kernel one.** Every `heap_N.c` suspends the scheduler around `malloc`, so creating a kernel object after the scheduler starts costs one more outermost exit on the C side. `Config::DYNAMIC_ALLOCATION` says whether a configuration mirrors such a kernel: true for the Posix demo, false for silicon, where this kernel allocates nothing and the exit is not there to spend. |
| 2026-09-09 | `unsafe` lives only in `rusty_rtos_port-*` (context switch, vectors, registers) and `rusty_rtos-capi` (validated FFI), fenced per block, inventoried in `UNSAFE.md`, counted per release. |
| 2026-09-09 | The config is a type: geometry as associated consts, subsystems as cargo features, hooks as a trait, ISR-ness as a token; every knob classified in `docs/CONFIG-MAP.md`; `parameterizing-a-constant` governs every test. |
| 2026-09-09 | Ports in v1: sim, posix, cortex-m (M0/M3/M4F/M7/M33 NTZ), riscv (RV32 + esp-hal C6/P4), xtensa (ESP32/S3, local gate). The legacy ports and AArch64 are never / later. |
| 2026-09-09 | Track B is where Kairos meets Janus: the port crates implement `esp-radio-rtos-driver` so the radio blob runs on our scheduler; Track A keeps IDF-FreeRTOS until the C ABI passes the unmodified C corpus, and only then with the Janus owner's say. |
| 2026-09-09 | Heaps: `heap_1`, `heap_4`, `heap_5` remade with the protector; `heap_3` is the seam to `rusty_alloc` small-metal / `esp-alloc`; `heap_2` is an alias with the difference documented; no library sets the global allocator. |
| 2026-09-09 | +TCP is an API over the smoltcp engine unless a K7 measurement says the API cannot be met there; a from-scratch stack is opened only by that row. |
| 2026-09-09 | Claims discipline is Janus's: external oracle first, counters before clocks, ledger rows with method lines, README copies the plan, "sim only" / "builds, not flashed" are legal statuses, parity is the headline until the ledger says otherwise. Every parser has a no-panic test from the day it exists. |
| 2026-09-09 | Edition 2024, MSRV 1.85, `unsafe_code = "deny"` at every workspace, `forbid` in every core, `undocumented_unsafe_blocks = "deny"`, MIT OR Apache-2.0 with the FreeRTOS MIT credit in every README. |
| 2026-09-09 | The plan is one document (this one); package plans stay in their repositories; every binding decision gets a dated row here. |
| 2026-09-09 | **Sim contract v1 delivers the tick at every 16th outermost critical-section exit**, plus one per idle-hook pass — not at every 16th yield. The first `dynamic` run under the yield rule never finished: SUSP_RX polls a queue without blocking and never yields, so no tick could arrive (8 GB of trace before it was killed). A critical-section exit is where a hardware tick that arrived during the section fires, so it is also the more faithful model. Known limit: a task that computes without touching the kernel (`flop`, `integer`) starves; those scenarios need contract v2 (a checkpoint rule). |
| 2026-09-09 | Trace subjects: tasks and timers by their own names; queues, event groups and stream buffers by a per-kind creation ordinal (`q1`, `g1`, `s1`) — `Queue_t` has no name and addresses are nondeterministic. `Event` therefore carries `EventGroupCreate` and `StreamBufferCreate` (42 events), since the creation line is what assigns the name. |
| 2026-09-09 | `PosixDemoConfig` has 64-bit ticks: the Posix port types `TickType_t` as `unsigned long` and the oracle prints `portMAX_DELAY` as 2^64-1. Firmware profiles stay 32-bit; the kernel is generic over `Config`. |
| 2026-09-09 | The classic `FreeRTOS/FreeRTOS` repository is pinned to `main` @ `f4fcc3b2` (2026-08-26), sparse-checked-out to `Demo/Common/{Minimal,include}`, `Demo/Posix_GCC`, `Test/{CBMC,VeriFast}`: no release tag covers the demo corpus, and the pin is what makes it a number. |
| 2026-09-09 | **Sibling overrides are cargo `paths` overrides, not `[patch]` tables.** Measured on `rusty_rtos_kernel`: a `[patch]` rewrites `Cargo.lock` (the patched crate loses its git `source`, unused rows become `[[patch.unused]]`), so the lockfile committed from inside the umbrella fails `--locked` in a standalone clone and vice versa. A `paths` override is applied after resolution: the committed lockfile keeps `git+…#<commit>`, `cargo metadata --locked` passes in both places, the local checkout is what compiles, and an unmatched entry is ignored. `kairos patches` reads each package's manifests and lists only the crates the graph names. Janus's wall 5 (unused patch rows) is thereby closed rather than avoided. |
| 2026-09-09 | The umbrella repository is `Remade-With-Rust/kairos` (the manifest's `umbrella` field, mirroring Janus's `janus`), whatever the local folder is called; it holds the plan, the manifest, the tool, the oracle harness and the stored traces, and never a package. |
| 2026-09-09 | **A task on the sim is a resumable state machine, not a stack.** A context switch is a stack swap and a stack swap is `unsafe`, which the family forbids outside `rusty_rtos_port-<arch>`. So `rusty_rtos_kernel` decides *which* task runs and `rusty_rtos_demo`'s runner acts on the decision by stepping that task's body one C statement at a time. Both kernels then make the same kernel calls in the same order, which is all the trace records. The cost is that a scenario must be remade as a state machine rather than linked; the benefit is a scheduler with no `unsafe` at all, and a trace that is identical to the C kernel's. |
| 2026-09-09 | **Critical nesting is per task, and its exits are deferred, not lost.** Three facts about the C Posix port that a stackless kernel has to state explicitly, each found by a trace diff: a task's first switch-in resets the nesting (`prvWaitForStart`); nesting is saved and restored across a switch (`prvSwitchThread`), so an abandoned frame's exits are paid when the task runs again; and the tick handler's nesting bump is a raw `++`/`--`, never `vPortExitCritical`, so it must not create an owed exit. On the sim these are not bookkeeping — an outermost critical-section exit *is* the clock, so getting them wrong moves every tick. |
| 2026-09-09 | **`KAIROS_TRACE_EXITS` is part of the conformance toolkit.** Both the C harness and the Rust sink can add a ` #<exits>` column, and `kairos conform --exits` compares it. The events agreed for 1,598 lines after the sim-time accounting had already drifted, so the event diff names the symptom and the column names the cause. Off by default: a trace with the column is not the contract's format. |
| 2026-09-09 | **Sibling patching stays a `[patch]` table, not a `paths` override.** A `paths` override leaves `Cargo.lock` in its standalone form, which is why it was tried; cargo refuses to let one alter a package's dependency list, and every Kairos sibling is a facade over a `-core` crate, so the facade's own dependency is altered the moment both are overridden ("in the future this message will become a hard error"). The rule that keeps `[patch]` honest: the committed lockfile is the standalone one, `--locked` is a standalone check, and `kairos patches` emits a row only for a crate the graph actually names — reaching through a facade to its core, which patching the facade alone would silently leave on the published version. |
| 2026-09-09 | **The house stack is validated by a compile gate, not a reading.** `tools/house-gate` in the umbrella checks every house crate Kairos could consume, at its pin, `no_std` on `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` and on the host, under the Kairos `deny.toml`. Verdicts of 2026-09-09 in `docs/HOUSE-STACK.md`: ready on bare metal — `rusty_alloc-api` 2.0.4 (with `--cfg ra_single_threaded --cfg ra_small_profile`), `rusty_symbols` 0.1.0, `thoth` v0.3.0 (git tag, `default-features = false`), `rusty_json_turbo` 0.1.0 (git; lib `serde_json`); `rusty_zstd` 0.2.5 (0.2.3 failed on `AtomicU64`; re-measured the same day at 0.2.5, which passes on both bare-metal targets, and the pin moved); host-only today — `rusty_erasure-core` 0.4.0 (`AtomicU64`), `rusty_time-core` 0.1.10 and `rusty_xml` 0.8.1 (`std`); out of scope for an RTOS — SpaceDB, FFAI, remade_ffmpeg_rs, rusty_maps (their closures resolve clean of C and of banned crates). Four crates.io names are imposters and are banned by name in every `deny.toml`: `rusty_time`, `rff`, `thoth`, `spacedb`. |
| 2026-09-09 | `rusty_rtos_json` is the coreJSON API (zero-allocation validator + `JSON_Search`); `rusty_json_turbo` (the house serde_json) is the typed layer behind a `serde` feature under `alloc`. One job, one parser each; the json package plan carries the row. |
| 2026-09-09 | The allocator seam has both halves: `rusty_rtos_alloc` with `std` (default) for hosted deliverables, and `--no-default-features` under `--cfg ra_single_threaded --cfg ra_small_profile` for firmware, checked on the four bare-metal targets by CI and by `kairos check` (`cfgs` in `KAIROS.toml`). The fixed `Region` and `heap_3` wiring stay K4's. |
| 2026-09-09 | **The fleet tool consumes the house stack it validates**: `rusty_alloc` through the seam, `thoth` v0.3.0 for the status glyphs, `rusty_json_turbo` for `status --json`, `rusty_zstd` for the stored oracle traces (`<scenario>.trace.zst`, level 19, decompressed and compared before it is kept, `oracle cat` to read). Every stored trace is therefore a zstd frame from the day the corpus exists (K1 diffs against `kairos oracle cat`). What Kairos wants on bare metal and cannot have yet is `docs/plans/build-me-bare.md`. |
| 2026-09-09 | The oracle is a dev-only C dependency built by the fleet tool from source with the system compiler under WSL; it never enters a package's build graph, and `kairos oracle patch` restores the pinned `port.c` before applying its six exact-anchor edits, so the checkout is never in an unknown state. |

---

## 9. Appendix — the ground truth a session needs

**FreeRTOS, read 2026-09-09.** Org `github.com/FreeRTOS`, 44 active
repositories. Kernel latest release **V11.3.1** (2026-08-21); the current LTS
**202604.01** carries the same V11.3.1 (an earlier draft of this plan said 202406.00 / V11.1.0; corrected 2026-09-09 from `FreeRTOS-LTS` @ `0b25dc50`). Kernel tree: `tasks.c` 365 KB,
`queue.c` 130 KB, `stream_buffer.c` 77 KB, `timers.c` 57 KB,
`event_groups.c` 36 KB, `croutine.c` 17 KB, `list.c` 10 KB;
`include/`: `task.h` 171 KB, `FreeRTOS.h` 107 KB, `queue.h` 74 KB,
`timers.h` 66 KB, `stream_buffer.h` 59 KB, `semphr.h` 50 KB,
`message_buffer.h` 45 KB, `event_groups.h` 36 KB, `mpu_prototypes.h` 33 KB,
`list.h` 25 KB, `mpu_wrappers.h` 19 KB, `atomic.h` 13 KB, `portable.h` 12 KB.
`portable/`: 26 compiler families; `GCC/` has 60 port directories (ARM_CM0
… ARM_CM85_NTZ, ARM_CR5/CR82/CRx, ARM_CA9/CA53/AARCH64, RISC-V, MicroBlaze,
NiosII, PPC, RX, RL78, AVR, MSP430, …); `ThirdParty/GCC/` has ARC, ARM_TFM,
ATmega, **Posix**, RISC-V, RP2040, **Xtensa_ESP32**; `MemMang/heap_1..5.c`;
a `template/` port. 94 `config*` identifiers in `FreeRTOS.h`. Roughly 430
public declarations and API macros across the seven public headers.
`FreeRTOS/Test/`: **CBMC** (proofs for Queue and Task), **CMock** unit tests,
**VeriFast** (list, queue), Target. `FreeRTOS/Demo/Common/Minimal/`: 34
standard demo task files. Emulator demos: `Posix_GCC`, `WIN32-MSVC`,
`CORTEX_LM3S6965_GCC_QEMU`, `CORTEX_MPS2_QEMU_IAR_GCC`,
`CORTEX_MPU_M3_MPS2_QEMU_GCC`, `RISC-V_RV32_QEMU_VIRT_GCC`,
`RISC-V-Qemu-virt_GCC`, `RISC-V-Qemu-sifive_e-Eclipse-GCC`,
`CORTEX_A9_Zynq_ZC702_Vitis_QEMU`, `RISC-V_Renode_Emulator_SoftConsole`.
Kernel CI: formatting, spell-check, link-verifier, manifest-verifier, CMock
unit tests with coverage, Coverity MISRA, kernel-demos. MISRA C:2012 with
listed deviations (`MISRA.md`).

**The Rust landscape, pinned 2026-09-09 (re-check with `cargo search`):**
embassy-executor 0.10.0, embassy-sync 0.8.0, embassy-time 0.5.1, embassy-net
0.9.1, smoltcp 0.14.0, rtic 2.3.1, esp-rtos 0.4.0, esp-radio 1.0.0-beta.0,
**esp-radio-rtos-driver 0.4.1**, esp-hal 1.2.1, esp-hal-embassy 0.9.1,
esp-alloc 0.11.0, esp-println 0.18.0, esp-backtrace 0.20.0,
esp-bootloader-esp-idf 0.6.0, esp-storage 0.10.0, esp-idf-svc 0.52.1,
esp-idf-sys 0.37.2, cortex-m 0.7.9, cortex-m-rt 0.7.6, riscv 0.16.1,
riscv-rt 0.18.0, heapless 0.9.3, critical-section 1.2.0, portable-atomic
1.15.0, defmt 1.1.1, embedded-alloc 0.7.0, freertos-rust 0.2.0 (C bindings —
a comparison arm, never a dependency), rusty_alloc 2.0.4, rusty_zstd 0.2.5 (the fleet tool still links 0.2.3),
mid-verify 0.1.0, spacedb-sdk 0.6.0, loom 0.7.2, kani-verifier 0.67.0,
proptest 1.11.0, cargo-mutants 27.1.0, cargo-deny 0.20.2, cargo-vet 0.10.2,
cargo-geiger 0.13.0, cargo-audit 0.22.2. Neither Embassy (an async executor)
nor RTIC (stack-resource policy) nor esp-rtos (Espressif-only, esp-radio's
host) nor Hubris/Tock (microkernels with their own build systems) is a
portable, FreeRTOS-shaped, preemptive priority kernel in pure Rust; that is
the gap this family fills, and each of them is a comparison arm in
`rusty_rtos_demo`, never the bar.

**Where things are on this machine:** the Janus umbrella and its tool at
`C:\Users\talmo\coding\janus` (not F:); this umbrella at `F:\coding\rusty_RTOS`;
the esp toolchain env in `C:\Users\talmo\export-esp.ps1`; keep build dirs
under short paths (`C:/kairos-t`); `df` before a heavy build; delete only your
own target directories.

**Standing rules for whoever works here next:** never flip a repo public
without the owner; `kairos deploy` needs `--public|--private`; Janus repos are
touched only through their own plans and only for the K5 joint; commits end
with the co-author line; secrets are read from the environment or stdin and
never printed; no number without its method line; no plan edit without a
decision-log row when it binds.

**Skills that govern this work** (all in `~/.claude/skills`):
`building-the-new-internet` (the stack, the scaffold, the deploy seams),
`rusty-esp-embedded` (the chip side, the umbrella pattern, the walls),
`codec-measurement` (what makes a number admissible — required before any
number here), `codec-bringup-decoder` (the trace-oracle discipline, transposed),
`parameterizing-a-constant` (every config knob), `footprint-decomposition`
(RAM/flash tables), `use-protection-please` (the hardening tables and the
1.0.0 gate), `rusty-curiosity` (when a trace diverges where it should not).
