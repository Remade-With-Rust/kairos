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
| **coreSNTP** | `rusty_rtos_sntp` | the SNTPv4 packet codec, its clock arithmetic and the polling client over a UDP transport ✅ | — | NTS on a chip in v1; the full NTPv4 engine, which is the house `rusty_time`'s job |
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
| **Host** | — | **`-host`, BUILT 2026-09-11**: one OS thread per task with a single run permit, the shape `portable/ThirdParty/GCC/Posix` and MSVC-MinGW both use. It exists because a C task needs a REAL STACK and a stackless kernel has none to give — without it K6 could not run on a laptop at all. Preemption is a platform backend and BOTH are written: `SuspendThread` on Windows, and on Unix `pthread_kill` with a handler that parks the target in `sigsuspend` — because Unix has no call that stops another thread from outside, so the thread has to freeze itself. `PREEMPTIVE` reports the answer for a platform that is neither. `-sim` for the oracle and CI. | K1 / K6 |
| **Cortex-M** | — | `-cortex-m` over `cortex-m-rt`: M0/M0+ (no `ldrex`, PRIMASK-only), M3, M4F/M7 (lazy FPU stacking), M23/M33/M55/M85 (ARMv8-M, TrustZone NTZ first). QEMU `lm3s6965evb` (M3) is the first cell; `mps2-an385` (M3) and `mps2-an505` (M33) follow. | K3 |
| **RISC-V** | — | `-riscv` over `riscv-rt`: RV32I + CLINT (QEMU `virt`), then esp-hal's interrupt controller on the C6. | K3 |
| **RISC-V** | **`rusty_rtos_port-riscv` exists and switches (2026-09-11)** — 100/99 resumptions from three frames deep on QEMU `virt`, zero faults. A RISC-V trap saves NOTHING, so the port writes down all fourteen callee-saved registers itself, and a fresh task resumes through a trampoline because the C ABI passes arguments in caller-saved registers the switch does not restore. **And QEMU RV32 CAN supply a work counter**: `minstret` under `-icount shift=0` is reproducible to the instruction (199,102 three times; 20% spread without it), so comparative work rows no longer need hardware — absolute cycle rows still do. | `-riscv` for the C6/C61/H2/P4 family; `CLINT_MSIP` is a constant a board overrides. | K3/K5 |
| **Xtensa** | **`rusty_rtos_port-xtensa` exists and switches (2026-09-11)** — the switch runs in a software interrupt so `xtensa-lx-rt`'s exception entry spills the register windows, which is how FreeRTOS and `esp-rtos` do it and is what a hand-written spill from task context failed to do. 100/99 resumptions from three call frames deep, zero faults, on a XIAO S3. | `-xtensa` for ESP32 (LX6) and S3 (LX7) on the `esp` toolchain, `xtensa-esp32s3-none-elf`; local gate only, CI has no esp toolchain. | K5 |
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

**Status (2026-09-16): the first flip has happened.**
`rusty_rtos_core` is on crates.io at **0.1.0** and its repository is **public**,
tagged `v0.1.0`. Two of the conditions above were NOT met and are recorded
rather than skipped: the hardening row is 50% rather than complete (waived for
0.x, with a reason per gate in that package's plan), and neither `release-plz`
nor the org's `portfolio-check` exists yet. This bar is written for 1.0.0;
0.x is the version number that says the API is not stable, which is the reason
those two were acceptable here and will not be at 1.0. One condition the
release CLOSED for the whole family: "CI green without a token" — the
`KAIROS_GIT_TOKEN` rewrite existed to fetch private siblings by git URL, and
with every sibling resolving from crates.io there is nothing private to fetch.
Everything else is queued behind `tools/publish-0.1.0.sh`, and three of the
four port backends are still UNTRACKED in git, which blocks publishing them.
See `docs/LEDGER.md`.

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

The arena-and-list cost row is taken too. It was the one number that did not
come out where the plan hoped — **2.08×** the C list against a 1.25× revisit
line — and it is now the one that came out BETTER than the plan hoped:
**0.842×**, i.e. fewer instructions per list operation than the `list.c` it
remakes. §5.2 item 2 is CLOSED and the arena is reaffirmed on a measurement
rather than on preference.

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
### K7's build order, and why it is not the order this plan first wrote (2026-09-16)

The list above reads *json, backoff, mqtt, http, sntp, tcp*. **Start with
`backoff`**, and the reason is the one K4 paid for twice: `heap_1` was built
before `heap_5` to learn whether the differential harness generalised beyond
the allocator it was written for, and it did, and the harness needed nothing.
K7 has the same question and no answer yet — nothing in this family has ever
been diffed against a C *library* rather than the kernel.

`backoffAlgorithm` is the smallest thing that can answer it. It is a pure
function with no I/O, no time source and no transport, its behaviour is two
lines of arithmetic plus a PRNG, and its oracle is its own unit-test vectors.
If the K7 harness needs changing, it is far better to learn that on a crate
that can be read in one sitting than on coreMQTT.

**Step 0, and it blocks every one of them: the oracles are pinned but not
fetchable.** `ORACLES.md` carries the exact commits — coreJSON v3.3.1 at
`cffa492`, backoffAlgorithm v1.4.2 at `14f4c88`, coreMQTT v5.0.2, coreHTTP
v3.1.3, coreSNTP v2.0.0, FreeRTOS-Plus-TCP V4.4.1, all under
`FreeRTOS/FreeRTOS-LTS` at `0b25dc50` — and `kairos oracle fetch` clones only
`FreeRTOS-Kernel` and the classic `FreeRTOS` repository. Teaching it the
library pins is the first commit of K7 and it is shared by all six.

| # | package | oracle, and what the kill test can actually be | why here |
|---|---|---|---|
| 1 | **`rusty_rtos_backoff`** ✅ done 2026-09-16 | `backoffAlgorithm`'s own unit-test vectors, plus its PRNG driven identically so the sequences are comparable — the same trick the heap differentials use, because `rand()` is implementation-defined and would make the arms incomparable by construction | the smallest thing that proves the K7 harness. No I/O, no transport, no clock |
| 2 | **`rusty_rtos_json`** ✅ done 2026-09-16 | **JSONTestSuite at 100 %** (95/95 `y_`, 188/188 `n_`) **and 318/318 agreement with `core_json.c`**, which are kept as two tests because they are two claims — they coincide only because coreJSON was measured against the corpus *first*. Then the query half: **2,124 queries and 348 iterations agree with the C**, as a 3,089-line trace. Plus the no-panic gate over the whole public surface: **73,005 deterministic documents**, including a LIVENESS assert, because a query cursor that stops advancing hangs rather than failing | self-contained, and the first one with a real external corpus rather than a vendor's vectors |
| 3 | **`rusty_rtos_sntp`** ✅ done 2026-09-16 | **160 trace lines agree with `core_sntp_serializer.c`** across 75 calls, including both sides of the 2036 era wrap and the half-era tie from both directions. The trace carries its own inputs, so there is no second copy of the C's table to drift. Then the client: **549 more lines across 23 scenarios, compared CALLBACK FOR CALLBACK** — every clock read, every datagram, every rotation, and the context state after each one | its hardest defect is DATED — an era comparison that is wrong is correct until 2036 and silently wrong afterwards, which no amount of testing-against-today would find. **Two defects in the C came out of it**, both drafted for filing (§5.4) |
| 4 | **`rusty_rtos_mqtt`** ✅ complete 2026-09-19 | **coreMQTT v5.0.2**, compared OPERATION FOR OPERATION with both record arrays checked after every call — their order is the resend order, so a status-only comparison would bless a transcription that reordered them. Full record: **`docs/LEDGER.md`** and the package's own ledger. | transport-agnostic by design: it takes send/recv callbacks, so it can be proven over the HOST's TCP long before `rusty_rtos_tcp` exists. Full record: **`docs/LEDGER.md`** and the package's own ledger. |
| 5 | **`rusty_rtos_http`** ✅ complete 2026-09-20 | **coreHTTP v3.1.3**. The request side is compared BYTE FOR BYTE, because it delegates nothing; the header trace dumps the whole buffer PAST the reported length, which is what pins coreHTTP's two partial-write behaviours. Full record: **`docs/LEDGER.md`** and the package's own ledger. | same shape as mqtt and strictly less protocol, so it costs little once the mqtt harness stands. Full record: **`docs/LEDGER.md`** and the package's own ledger. |
| 6 | **`rusty_rtos_tcp`** ✅ complete 2026-09-20 | the +TCP API over smoltcp; interop against the host stack; `iperf`-style rows. **The number is the point: it is nearly three times coreMQTT's 13,066 function lines, and coreMQTT took about fifteen slices.** Anyone planning K7's end should plan this as its own mission rather than a package | the largest of the six and the only one carrying a transport. Everything above it can be proved without a socket, so it comes last — and it is the one whose kill test needs a second machine. Full record: **`docs/LEDGER.md`** and the package's own ledger. |

**All six are finished — see §4.** This table is kept for the *decision* it records: which oracle each package is proved against, and why they were taken in that order. The results that used to sit in it are in `docs/LEDGER.md` and in each package's own ledger.

**The `json` decision is already taken and should not be reopened.** This
package is the coreJSON *API* — the zero-allocation validator and
`JSON_Search` over a byte buffer, `forbid(unsafe)`, **no `alloc`** — because
serde_json's model (an `alloc`-backed `Value`, boxed errors) cannot provide
it. Typed (de)serialization under `alloc` goes through the house's
`rusty_json_turbo` behind a `serde` feature, **never a second parser for that
job**. That is `rusty_rtos_json`'s decision log, 2026-09-09, and
`HOUSE-STACK.md` carries the pin and the gate. Two jobs, one crate each.

**What each package must NOT do**, learned from K4 and K6 and worth stating
once for all six:

* **No differential whose workload cannot fail.** heap_4's first one never
  refused a request and agreed about the easy half of an allocator; heap_1's
  guard is inverted for the same reason. Every K7 package needs its own
  version of "the workload reached the branch that matters" — for a parser
  that means the REJECTIONS, so JSONTestSuite's `n_` files carry more weight
  than its `y_` files.
* **No claim of a kind the oracle cannot support.** `heap_3` could not be
  diffed against `heap_3.c` because both sides would be third-party
  allocators, so its claim is weaker and says so. If a K7 package hits the
  same wall — and `sntp` against a live server may — the weaker claim is
  stated, not dressed up.
* **Poison-prove every pass.** Three K4 differentials passed first time and
  all three were only trustworthy after a deliberate one-character break made
  them fail.
* **Read the pinned source, do not remember it.** K4 corrected two plan
  assumptions this way — `heap_1`'s `vPortFree` is an assertion and not a
  no-op, and `heap_5` asserts region order rather than sorting. Both were
  written from memory into a plan and both were wrong.

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
| 5 | XIAO ESP32-S3 Sense | K5 Xtensa, K8 SMP (two cores) | **in hand, and the corpus runs on it** — 18/18 byte-identical to C, 2026-09-10 |
| 6 | QEMU `mps2-an505` (Cortex-M33) | K8 MPU / TrustZone | install |
| 7 | a Cortex-M4F board + probe | K3 on real Cortex-M silicon; FPU lazy stacking | buy |

**Rows** — [x] K1 arena-list cost vs C list (host counts): **18.80 vs 22.32
instructions per list operation, 0.842×** — callgrind under WSL2,
`bench/list-cost/run.sh`, both arms gated on an identical checksum
(2026-09-21; it read 46.45 / 2.08× on 2026-09-09 and 35.84 / 1.606× when
this session re-baselined it, WSL carrying a different rustc from the
umbrella's) · [x] K3 context switch WORK
row on RV32: 30 retired instructions vs FreeRTOS's 83, both from their own
toolchains, straight-line-checked and poison-proven (`bench/switch-cost/run.sh`,
2026-09-11) · [x] K3 context switch WORK row on ARM too:
19 vs 19, PARITY, which is the RISC-V row's control (`bench/switch-cost/run.sh`,
2026-09-11) · [x] K3 the CYCLE rows, on S3 SILICON: switch 623,
tick 131/129, ISR-API wake 949 cycles (the kernel's share of a wake -- no
interrupt is taken, so NOT the full ISR-to-task latency the clause names), ccount at 1-cycle resolution, instrument
tax measured and subtracted (`xiao-s3-cycles`, 2026-09-11) · [ ] those rows
AGAINST THE C DEMO — blocked: FreeRTOS on the S3 needs ESP-IDF headers and a
generated sdkconfig.h, and its port's #ifs key off CONFIG_FREERTOS_* · [x] K3 RAM per profile vs
the C map: two floor FUNCTIONS, 596 B vs 207 B per task (`bench/kernel-ram/run.sh`,
2026-09-11) · [x] K3 FLASH vs the C map: 15,950 vs 13,924 bytes,
1.15x, both gc-sectioned from the corpus root set (`bench/kernel-flash/run.sh`,
2026-09-11) · [x] K3 the one-hour soak on SILICON:
`death` at 3,600,000 ticks on a XIAO S3, counters identical to M3 and RV32
(2026-09-11) · [x] K3 the full eighteen at that length on BOTH emulators:
RV32 18/18 in 58 min, M3 18/18 in 75 min, `semtest` 79% of it (`bench/soak-each/run.sh`,
2026-09-11) · [ ] the same on a part · [x] K3 the RISC-V PREEMPTIVE path, DEFECT FOUND
AND FIXED: `switch_context` resumed with `ret`, so the first preemptive switch
was also the last; the port gained `switch_context_trap` +
`new_task_context_preemptive`, and the cell now runs 201 switches between two
tasks that never yield, zero faults (`riscv32-qemu-preempt`, 2026-09-11) ·
[x] K3 the preemptive switch MEASURED: 74 vs FreeRTOS 83, 1.12x, near parity
(`bench/switch-cost/run.sh`, 2026-09-11) · [x] K4 `heap_4`
differential trace -- and since 2026-09-16 all four heaps: `heap_1` (2,000 ops,
1,973 refusals), `heap_4` (20,000 ops + a generational protector), `heap_5`
(20,000 ops over three regions with real gaps), each poison-proven, plus
`heap_3` over the global allocator whose claim is deliberately a WEAKER kind --
the accounting reconciles, because a differential against `heap_3.c` would
compare two third-party allocators and call it conformance · [x] K5 the four hardware claims RE-RUNNABLE,
not merely re-readable: `kairos check --board` (2026-09-11) · [ ] K5 S1 on
Kairos vs esp-rtos · [x] K5 the
corpus on S3 SILICON, 18/18 byte-identical to C (2026-09-10) · [x] K5 Xtensa
context switch on the S3 — 100/99 resumptions from three frames deep, zero
faults, poison-proven (2026-09-11); needed to host tasks that own stacks, NOT
needed for the corpus, which runs there without one · [x] K6 unmodified C corpus: **26 demo files**, 25 of them with a verdict of their own, together on QEMU M3 and on a host (Windows threads and Linux pthreads), all clean under the demos' own 10,000-tick cadence with latching checkers; the 26th needs a binary of its own and 2 of the 34 can never be graded (2026-09-12) ·
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
| ~~Decide the +TCP engine (§5.5)~~ — **answered 2026-09-20 by measurement, not by preference.** All 78 public +TCP entry points were censused from the pinned V4.4.1 headers and given a verdict against smoltcp 0.12: **8 cannot be met as promised, and 7 of those 8 are one thing** — the zero-copy handout that gives an application a raw pointer INTO the stack's buffer pool to keep across calls. smoltcp takes a CLOSURE over the buffer instead, which is the same saving without a pointer outliving the call, and is what a `forbid(unsafe)` crate could offer anyway. The eighth is `FreeRTOS_mss`, a private `remote_mss` with no getter. **The API is meetable; no row opens the from-scratch stack.** `rusty_rtos_tcp/docs/api-measurement.md`, generated by a JOIN that fails if any entry point lacks a verdict or any verdict names a function that does not exist | — |
| ~~the SNTP source (§5.5)~~ — **decided 2026-09-16**: a coreSNTP remake, because `rusty_time-core` is `f64` where coreSNTP is integer-only. Measured, see §5.4 | — |
| Run each public flip (§2.11) | market |

### 5.4 The upstream queue

| item | where | state | when it lands |
|---|---|---|---|
| Generalise the fleet tool (`janus` and `kairos` are one binary reading `<NAME>.toml`) | janus `tools/janus` | not filed | Kairos ships with a copy until then |
| `esp-radio-rtos-driver` adapter on a non-esp-rtos scheduler | esp-rs (docs) | **drafted 2026-09-11, awaiting the owner** — `kairos-upstream/drafts/esp-radio-rtos-driver-stack-contract.md` | The condition fired: the contract IS under-documented. `SchedulerImplementation` can only be implemented by a scheduler that gives each task its own call stack — the requirement is spread across `task_create` ("It should allocate the stack"), `schedule_task_deletion` ("The thread stack can be free'ed") and a blocking `Semaphore::take`, and is never stated as a requirement. A stackless scheduler reads the trait as satisfiable and finds out on silicon. We found out by writing the Xtensa switch |
| ~~`rusty_alloc` `prim::fixed` on Cortex-M~~ — **CLOSED, and this row was stale.** `rusty_rtos_core/firmware/mps2-an385-qemu-region` has run it on a Cortex-M3 since 2026-09-10 and still does: **9/9**, `give()` → 196,608 usable, a second `give()` refused with `FERR_REGISTERED`, a `Box` and a collection served and every allocation proven inside the region by `region_contains`. `build-me-bare` B4a closed it; this row was never updated, and it was quoted as a blocker for K4's `heap_3` on 2026-09-16 before being re-run. **What is actually outstanding is B4b**: a cycle row from a Kairos target, which QEMU cannot give — that cell measured DWT CYCCNT at **0** and SysTick deltas that SHRINK as the loop grows (848/548/353 for 1k/2k/4k iterations), because they track host wall time while TCG's translation cache warms rather than guest work. The Xtensa arm of the comparison is done on silicon; the riscv32 arm needs a C6, which is hardware and not code | rusty_alloc | **correctness: met on Cortex-M.** The cycle row needs a part | K4's `heap_3` decision |
| ~~`rusty_time` `no_std` leaf for the SNTP client~~ — **CLOSED, and this row was stale.** It described 0.1.10 as carrying the `no-std` category without `#![no_std]`, measured 2026-09-09. **`rusty_time-core` 0.2.0 was published 2026-09-10**, the day after, and builds clean on `thumbv7em-none-eabihf` with no default features and no transitive dependencies — re-measured 2026-09-16 before `rusty_rtos_sntp` was started. The draft never needed filing. **`rusty_rtos_sntp` is still a coreSNTP remake, for a different reason**: `rusty_time-core`'s time arithmetic is `f64` (`NtpTimestamp::seconds_since`, `NtpShort::to_seconds`), and coreSNTP's clock offset is integer milliseconds with specific truncation, computed on the receive path of every poll on parts with no FPU. Sharing its codec alone would buy ~40 lines of byte shuffling and cost a dependency whose arithmetic this package cannot use | rusty_time | **closed by upstream, unfiled** | §5.5 item 4 is decided |
| ~~`rusty_zstd` 0.2.3 `no_std + alloc` uses `core::sync::atomic::AtomicU64`~~ | rusty_zstd | **resolved upstream in 0.2.5**, never filed; re-measured 2026-09-09 (`gate-zstd` exit 0 on both bare-metal targets), house-gate pin moved to `=0.2.5` | on-chip compression (OTA, trace capture) is unblocked; the fleet tool still links 0.2.3 on the host, which is a pin to align, not a blocker |
| `rusty_erasure-core` 0.4.0 imports `AtomicU64` in its `no_std` build; same 32-bit gap | rusty_erasure | drafted (`kairos-upstream/drafts/rusty_erasure-atomic-u64.md`), not filed | nothing in Kairos v1 needs it; recorded so the ladder is honest |
| **coreSNTP: two clock-related defects in `core_sntp_client.c`** — (1) `calculateElapsedTimeMs` subtracts on a `uint64_t` that may still be zero, so a clock stepping backwards inside one second yields 1.8 x 10^19 ms and a spurious `SntpErrorResponseTimeout`; a backward step is what a host does when coreSNTP itself hands it a negative offset. (2) Both retry loops exit only on a deadline computed from the host's clock, so an oscillating clock makes them spin for ever, and `blockTimeMs` does not bound the wait it documents. Found by the `rusty_rtos_sntp` client differential; the second was found by our own gate HANGING | FreeRTOS/coreSNTP | **drafted 2026-09-16, awaiting the owner** — `kairos-upstream/drafts/coresntp-elapsed-time-underflow.md` | our transcription reproduces both exactly, with a poison proving the differential is sensitive to the first; if a fix lands, it moves with the pin |
| **coreMQTT: three reason-code defects in `core_mqtt_serializer.c`** — (1) `readSubackStatus` is the SUBACK reason-code table and serves UNSUBACK too, so `0x11` "No subscription existed" — legal per MQTT 5.0 §3.11.3 — is refused with `MQTTBadResponse` and the granted-QoS codes an UNSUBACK cannot grant are accepted; `handleSubUnsubAck` never invokes the application callback on a failure, so a client unsubscribing from a filter it is not subscribed to never learns the unsubscribe completed. (2) A SUBACK or UNSUBACK with NO reason codes is accepted, because the count is derived by subtraction and never checked against zero. (3) `0x92` is accepted in a PUBACK and a PUBREC, where §3.4.2.1 and §3.5.2.1 do not list it. Found by PRINTING the accepted set of each 256-value sweep into the checked-in trace rather than hashing it | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-unsuback-reason-code-table.md` | our transcription reproduces all three exactly, and the test that records them reads the trace, so a fix landing under the pin fails the suite and says so |
| **coreMQTT: two unenforced PUBLISH protocol errors in `core_mqtt_serializer.c`** — (1) a zero-length Topic Name with no Topic Alias is accepted, though §3.3.2.3.4 calls it a protocol error, so the APPLICATION is handed a message with no topic and no alias to resolve one; (2) a Subscription Identifier of zero is accepted, though §3.3.2.3.8 calls it a protocol error and both its neighbours in the same switch check their values. Both too permissive, so no conformant traffic is lost. DUP on a QoS 0 PUBLISH is recorded as an observation rather than raised | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-publish-protocol-errors.md` | our transcription reproduces both, asserted from the trace so a fix landing under the pin fails the suite |
| **coreMQTT: a server DISCONNECT reason code missing from the table in `core_mqtt_serializer.c`** — `validateDisconnectResponse` is one switch serving both directions, and `0x9F` "connection rate exceeded" appears in neither arm, so it falls to `default` and is refused. MQTT 5.0 §3.14.2.1 lists it as sent by the Server. `handleIncomingDisconnect` never invokes the application callback on a failure, so the one disconnection whose correct response is "back off and reconnect more slowly" is reported as a malformed packet. The fix is one case label; the enumerator already exists. **Second table in this library to be one entry short**, after `readSubackStatus` | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-disconnect-reason-code.md` | our transcription reproduces it, asserted from the trace so a fix landing under the pin fails the suite |
| **coreMQTT: `MQTT_ValidatePublishProperties` is laxer than the library's own PUBLISH reader** — three defects in the OUTGOING publish property table, and in each one coreMQTT already gets the rule right somewhere else. (1) A Topic Alias of **zero** is accepted, where §3.3.2.3.4 forbids sending one and `deserializePublishProperties` refuses it. (2) A Payload Format Indicator **above 1** is accepted, where `MQTT_ValidateWillProperties` — forty lines up, carrying five of the same seven identifiers — refuses it. (3) **Every property but the Topic Alias may repeat**, because the `used` flag is declared INSIDE the property loop and is false at every property; `MQTT_ValidatePublishAckProperties` is the same loop with its flag one brace higher and dedupes correctly. A broker that enforces §3.3.2.3 answers with a `0x82` DISCONNECT, and the client cannot see why: its own library passed the packet | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-publish-property-validator.md`; `coremqtt-publish-protocol-errors.md` amended the same day with the mirror evidence for its zero Subscription Identifier, which the outgoing SUBSCRIBE validator refuses | our transcription reproduces all three, pinned from BOTH directions in one test, so a fix landing under the pin fails the suite |
| **coreMQTT: one header, two readers, and they disagree** — three defects in the last of `core_mqtt_serializer.c`. (1) `MQTT_GetIncomingPacketTypeAndLength` cannot say "not yet" where its buffered twin `MQTT_ProcessIncomingPacketTypeAndLength` answers `MQTTNeedMoreBytes`; a type byte with no length behind it yet is `MQTTBadResponse`, for all 168 types a client may receive, and that function's own doxygen shows a NON-BLOCKING loop ending in `assert( status == MQTTSuccess )` — so an ordinary TCP segment boundary inside a header closes a healthy connection. (2) `updateContextWithConnectProps` stores four values `MQTT_ValidateConnectProperties` calls protocol errors and KEEPS two of them; a Maximum Packet Size of zero makes nine functions answer `MQTTBadParameter` for ever, three of them deserializers, so the session goes inert in both directions — and the helper is public, documented with a worked example, and callable without the validator. (3) `MQTT_ValidatePublishParams` checks `qos != 0 && maxQos == 0` instead of `qos > maxQos`, so a broker that announced Maximum QoS 1 is sent QoS 2 | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-two-readers-one-header.md` | our transcription reproduces all three, each pinned by a test that reads the trace, so a fix landing under the pin fails the suite |
| **coreMQTT: `addPropUtf8` writes one byte past the caller's buffer (CWE-787)** — the three integer adders in `core_mqtt_prop_serializer.c` each check for the identifier byte PLUS the value; the string adder checks for the two length bytes plus the body and forgets the identifier, then writes it. A buffer of exactly `propertyLength + 2` gets `MQTTSuccess` and one byte past its end, and `currentIndex` is left OUTSIDE the buffer so every later adder works from an out-of-bounds cursor. Six public functions route through it: auth method, auth data, response topic, correlation data, content type and reason string. `MQTTPropAdd_UserProp` and `MQTTPropAdd_SubscriptionId` count the identifier correctly, so this one is alone. The fix is `+ 3U` where it says `+ 2U`, twice. Also: `isValidPropertyInPacketType` allows a Subscription Identifier in a PUBLISH, which its own comment and [MQTT-3.3.4-6] forbid, and which `MQTT_ValidatePublishProperties` refuses | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-addproputf8-off-by-one.md` | the FIRST finding our transcription could not reproduce rather than merely notice: it writes through a slice under `forbid(unsafe)`, so the overflow surfaced as a differential failure. Three trace lines are a bounded, tested exception |
| **coreMQTT: the topic matcher loses an empty last level** — a filter whose last level is `+` stops matching a topic whose last level is empty, once an earlier `+` has been used: `a/+` matches `a/` and `+/+` does not, nor `/`, nor `a//` against `a/+/+`. §4.7.1.3 makes `+` match exactly one level and §4.7.3 makes an empty level a legal one, and coreMQTT gets the specification's OWN example (`sport/+` against `sport/`) right — which is what makes it a defect rather than a design. A client subscribed to `+/+` silently never receives a message published to `a/`, and `MQTT_MatchTopic` returns `MQTTSuccess` with `false`, so nothing surfaces. Cause: `matchWildcards` advances only the FILTER index on a `+`, and the rescue in `matchEndWildcardsSpecialCases` is written in terms of that index. Also `MQTT_Status_strerror` has no case for `MQTTEndOfProperties`, which every property walk ends with | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-topic-filter-empty-last-level.md` | found by PRINTING the 39x39 match matrix rather than hashing it; the defect is a column pattern in it |
| **coreMQTT: a publish acknowledgement with properties and no reason code is never sent** — `handleIncomingPublish` and `handlePublishAcks` route to `sendPublishAcksWithProperty` when the application touched EITHER output, and that function then validates the reason code — which is still `MQTT_INVALID_REASON_CODE` (0xFF). `validatePublishAckReasonCode`'s switch has no case for the sentinel, so it falls to the default and answers `MQTTBadParameter`; nothing reaches the transport, the broker never sees the PUBACK or PUBREC, and the message is redelivered for ever. Adding a Reason String for diagnostics is exactly what the property is for, and MQTT 5.0 §3.4.2.2 allows properties with a success reason code. Two lines fix it: map the sentinel to `MQTT_REASON_PUBACK_SUCCESS` before validating | FreeRTOS/coreMQTT | **drafted 2026-09-19, awaiting the owner** — `kairos-upstream/drafts/coremqtt-ack-properties-without-reason-code.md` | found on the first run of a harness that scripts the APPLICATION CALLBACK as a third input; `qos1-with-property` sends nothing and `qos1-with-reason`, the same publish with a reason code as well, sends the PUBACK |
| **coreMQTT: the callback is handed an uninitialised `pReasonCode` for PUBLISH and PINGRESP (CWE-457)** — `MQTTDeserializedInfo_t` has four members; of the four places that build one, `handleIncomingPublish` and `handleIncomingAck`'s PINGRESP branch declare it with NO initialiser and fill in three. `handleIncomingDisconnect` uses `= { 0 }` and does it right. An application that handles reason codes generically — natural, given that SUBACK, UNSUBACK, DISCONNECT and the publish acknowledgements all carry them — dereferences a wild pointer, and there is no flag in the struct saying which members are valid. One character each fixes it | FreeRTOS/coreMQTT | **drafted 2026-09-19, awaiting the owner** — `kairos-upstream/drafts/coremqtt-uninitialised-reason-code-pointer.md` | the harness's callback log printed FOUR DIFFERENT large numbers for the same input. A field that is not stable across identical runs is not a value; it is a read of something else |
| **coreMQTT: a clean session never clears a stored PUBREL** — `handleCleanSession` clears the stored PUBLISHes, ZEROES `outgoingPublishRecords`, and then asks `MQTT_PubrelToResend` which PUBRELs to clear; that function reads the array that was just erased, so the answer is always none and `clearFunction` is never called for one. Every QoS 2 publish still awaiting its PUBCOMP when a session is replaced leaves one entry in the application's storage, keyed on `SET_INCOMING_PUB_FLAG( packetId )`, that nothing will ever remove. Hoisting both clear loops above both memsets fixes it. The PUBREL loop is also gated on `incomingPublishRecordMaxCount` while the records it reads are the OUTGOING ones, which is worth revisiting at the same time | FreeRTOS/coreMQTT | **drafted 2026-09-19, awaiting the owner** — `kairos-upstream/drafts/coremqtt-clean-session-leaks-stored-pubrels.md` | found because the trace prints the store's call counts AND the keys it was given: `clear=0` on a case that plainly had something to clear. The isolating case was added afterwards, and the resumed case one line below re-sends exactly that record — which is what makes it a defect rather than an empty array |
| **coreMQTT: `MQTT_SUB_UNSUB_MAX_VECTORS` below 3 makes the gather guard always true and writes past a stack array (CWE-787)** — both list senders bound their gather with `ioVectorLength <= ( MQTT_SUB_UNSUB_MAX_VECTORS - CORE_MQTT_SUBSCRIBE_PER_TOPIC_VECTOR_LENGTH )`, and both operands are unsigned. The macro is a documented user-settable config with NO stated minimum and whose whole purpose is to trade RAM for transport calls, so `2` is a plausible choice; `2U - 3U` is `4294967295U`, the guard is always true, and the loop appends past the end of `TransportOutVector_t pIoVector[ MQTT_SUB_UNSUB_MAX_VECTORS ]` — a stack write whose length the subscription list controls. Two preprocessor lines fix it, and neither changes the shipped behaviour | FreeRTOS/coreMQTT | **drafted 2026-09-19, awaiting the owner** — `kairos-upstream/drafts/coremqtt-sub-unsub-max-vectors-underflow.md` | found by asking why a POISON could not fire: deliberately mis-counting the per-topic vector cost changed no trace line, and working out why surfaced the subtraction. In this arm the precondition is two `const` assertions that fail the build |
| **coreMQTT: the two senders disagree about the timeout and about when to read the clock** — `sendMessageVector` compares elapsed time with `>` and reads `getTime` only when it will loop again, including NOT reading it after a transport failure; `sendBuffer`, thirty lines away, compares with `>=` and reads it every turn. So a clock step landing exactly on `MQTT_SEND_TIMEOUT_MS` stops a PINGREQ and not a DISCONNECT, and a completed vector send reads the application's clock callback one time fewer. Either behaviour is defensible; having both is what is not | FreeRTOS/coreMQTT | **drafted 2026-09-19, awaiting the owner** — `kairos-upstream/drafts/coremqtt-two-senders-two-timeouts.md` | our own arm had transcribed BOTH with `sendBuffer`'s shape and a 28-line differential agreed, because nothing asked. The sibling of the subscription-list finding: two things three lines apart that are meant to be the same |
| **coreMQTT: only the last subscription in a list is validated, and a topic filter is searched past its length** — (1) `validateSubscribeUnsubscribeParams` ends in a loop that ASSIGNS its status and never breaks, so a list's verdict is its LAST entry's; the same two subscriptions in both orders give opposite answers, and the loop three lines above it over the same list DOES break. Everything `validateTopicFilter` checks is lost, so the client sends a SUBSCRIBE carrying a filter like `$share//a/b`. (2) `checkWildcardSubscriptions` uses `strchr` where every other function that touches a topic filter uses `topicFilterLength`, so a filter of `"abc"` inside a buffer reading `"abc#"` is refused, and an unterminated one is read past its allocation (CWE-125) | FreeRTOS/coreMQTT | **drafted 2026-09-18, awaiting the owner** — `kairos-upstream/drafts/coremqtt-subscription-list-validation.md` | the first found from a PAIR of cases in both orders; the second is the second finding our arm could not reproduce, so one trace line is a bounded exception |
| **smoltcp: a public getter for `remote_mss`** — the measurement's only gap that is not the zero-copy family, and a one-line addition. `FreeRTOS_mss` reports the peer's MSS and smoltcp 0.12 keeps the field private. Also worth asking: a setter for the socket buffer capacities, which `SO_RCVBUF`/`SO_SNDBUF` need and which are fixed at `Socket::new` today | smoltcp | not filed | K7 |
| FreeRTOS upstream: a deterministic-tick option for the Posix port (our oracle patch) | FreeRTOS-Kernel | not filed | the patch stays in-tree either way |

### 5.5 Open decisions

1. **The name.** Kairos is the recommendation (the opportune moment; the
   Greek sibling of Janus's naming); the crates.io name `kairos` is taken by an
   unrelated crate, which only affects the tool (`kairos-cli`, never
   published). Alternatives: keep `rusty_rtos` for everything.
2. ~~**Arena lists vs intrusive lists**~~ (§2.5). **CLOSED 2026-09-21, and
   the arena WINS.** The revisit condition had fired at 2.08× the C list
   (46.45 instructions per list operation against `list.c`'s 22.32, where
   1.25× was the line), and the second of the three routes was taken — the
   two changes the ledger named, plus the one it had not seen. The arm is
   now **18.80 instructions per operation: 0.842× the C at `-O2` and 0.862×
   at `-O3`**, measured on `bench/list-cost` with both arms gated on an
   identical checksum. No fenced `-core-unsafe` twin was needed and
   `forbid(unsafe)` is intact.

   What did it, in the order the measurements found it: **one array instead
   of two** (15.14 instr/op) — C FreeRTOS embeds `xListEnd` in `List_t` as a
   real `ListItem_t`, and that is not a detail of the C, it is why the C's
   walk needs no end test; **a power-of-two slot count and a masked index**
   (4.15) — LLVM proves `x & (N-1) < N`, so the bounds check folds away with
   no `unsafe`; and **deleting a step counter in the ordered walk** (3.27),
   which was dead code the moment the marker carried `MAX_VALUE`, and which
   now survives only under `debug_assertions` because its absence turns a
   corrupted list from an error into a HANG.

   The price is RAM: the list arena grows 38% (+612 B on the corpus
   geometry, +4.7% of the whole kernel's static footprint), of which about
   half is the power-of-two rounding — and that half is not waste, it is
   spare item capacity. Full decomposition, the refuted probes and the poisons are in
   `docs/LEDGER.md` — including the measured answer to "can it go below
   15": **no.** A bare index-linked list doing the same workload with NO
   safety at all measures **16.55**, so 15 would be 62 instructions per
   round below an implementation that checks nothing. The remaining 90
   per round between us and that probe is `Result`/`Option` plumbing and
   the one handle check, worth about 1.6 instr/op if an infallible entry
   point is ever wanted — an API decision, not a measurement.
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
   upstream leaf exists (`kairos-upstream/drafts/rusty_time-no-std-leaf.md`).
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
| **K0 family** | a clean clone of any package builds alone and its CI is green; the C oracle produces an identical trace twice | **passed locally 2026-09-09** — 8 repos, fleet gate green on the box, `dynamic` trace identical twice. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K1 scheduler on sim** | nine demo scenarios trace-identical to the C kernel for 100 000 ticks; counters equal; Miri green; the arena-list cost row | **passed 2026-09-09** — nine of nine, **8,408,764 lines identical at 100,000 ticks**, ticks/yields/exits equal. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K2 IPC + timers** | the remaining demo scenarios trace-identical; Kani harnesses for the CBMC proof list pass; no-panic property test; mutants score ledgered | **passed — 18 of 18 scenarios run**: 14 on 2026-09-09, the two buffer demos and the last two on 2026-09-21. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K2.1 the Rust face** | every corpus scenario still trace-identical at 100 000 ticks with the Rust face compiled in, at **zero** extra critical-section exits; a `typed` demo arm passes its own check; one `trybuild` row per bug class the C API cannot refuse; the Kani count after the topology work in the ledger | **passed 2026-09-10.** `PollQ-typed` is `PollQ` written against `Queue<u16, 10>` and diffed against the C kernel's own `PollQ` trace. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K2.2 async task bodies** | one scenario re-written as `async fn` whose await points land exactly on today's `Wait::Blocked` points, and the corpus still gates it — or a ledger row saying why it cannot and what that costs | **passed 2026-09-10, with its own premise corrected** — the await points must NOT go where the `Wait::Blocked` returns are. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K2.3 the static face** | a declaration whose geometry is the declaration: exact `.bss`, no create-failure path an application can reach, and the symbolic-handle proofs unwritable against it | **passed 2026-09-10.** `system! { mod app use Cfg; tasks {..} queues {..} }` writes the geometry the declaration implies. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K3 silicon + QEMU** | the full corpus check task passes one hour on M3-qemu, RV32-qemu and a C6; context switch / tick / latency cycle rows vs the C demo **from the C6, not from QEMU**; flash + RAM decomposition | **PART DONE — and the measurement half is done except for one purchase.** **Closed:** the corpus is byte-identical to C FreeRTOS on **four** targets (host, ARMv7-M, RV32 and a XIAO ESP32-S3, the last a real part); the hour is closed on the host and on silicon; flash and RAM are both compared against the C map (**15,950 B against 13,924, 1.15x**; per task **207 B against 596 B**); and **all three cycle rows exist on silicon** — tick **54**, switch **166**, ISR-wake-to-task **430** cycles, re-measured 2026-09-21 after the numbers this row used to carry (131 / 623 / 949) were found to have been measured through a harness defect. **And the C arm exists as a WORK row**: `bench/tick-work/run.sh` runs FreeRTOS **V11.3.1 out of the pinned oracle, unmodified**, against Kairos on one machine under one instrument — tick **15 against 56**, scheduler selection **27 against 79**, both against us and published as findings; but a whole cooperative switch is **110 against 109, parity**, because the two kernels put their weight in opposite halves. **Open, and this is the whole list.** (1) **A C6 — and nothing else.** Both cells now exist and build: `rusty_rtos_demo/firmware/esp32c6-corpus` (the hour, and it builds with `--features soak` too) and `rusty_rtos_kernel/firmware/esp32c6-cycles` (the cycle rows). **Neither has ever been flashed and neither carries a number**, and both say so in their own banner. The point of writing them unrun is that there was no C6 cell of ANY kind in the family, so a board arriving would have meant writing two cells and finding the toolchain differences before measuring anything; now it means `cargo run --release`. Checked rather than assumed: the cycles binary carries 20 `mcycle` reads in its disassembly and the corpus binary places `.flash.appdesc`, so both instruments survive to the artifact. Two findings fell out — `-nostartfiles` is Xtensa-only and `rust-lld` rejects it on RISC-V, and **a C6 cell builds on STABLE** (no esp toolchain, no `build-std`), making it the only silicon cell CI could build. The same purchase unblocks K5's kill test. (2) **Cycles with a C arm beside them**, which needs an **ESP-IDF install (~2 GB)** rather than hardware: the pinned oracle carries its own S3-capable Xtensa port and the S3 compiler is already on the box, so the blocker is the SDK, not the part. Owner's call, recorded unbought. (3) **`AbortDelay` FAILS the hour, bisected to a 1,000-tick bracket**: it passes at **220,000** ticks and fails at **221,000**, on RV32, reproducibly. **and it is NOT a liveness failure, which this row said twice before measuring it.** `tests/abortdelay_hour.rs` prints all three of `still_running`'s conditions: both cycle counters climb the whole way (868 at 220,000 ticks, 872 just past the boundary, **14,904** at the full hour), so neither task ever stops being scheduled. What fires is **`error`** — `prvCheckExpectedTimeIsWithinAnAcceptableMargin`, the scenario catching a block that came back outside its margin. That is a timing-correctness fault, not a scheduling one, and the hour's own claim (every check task still reports running) is in fact SATISFIED here. **And it is now NAMED**: the first failure is `expected 100, blocked 0` at `pc 72` — `prvTestAbortingQueueSend`'s first step, a `queue_send` on the one-deep queue that must block for 100 ticks and time out. It returns **immediately**, so the queue was not full when the test required it. Not a near miss against the 7-tick margin: the call did not block at all. Both tasks cycle **868 times cleanly** and fail on about the 869th, which is the shape of a slow drift rather than a missing case. **And 'the queue was not full' turned out to be MY misreading.** The probe could not read the queue at all: `queue_messages_waiting` is refused — `Gone` at the passing length, `InvalidHandle` at the failing ones — so **the handle does not name a live queue**. More to the point, the scenario cannot notice: `pc 72`'s match arm is `_ => self.checked(..)`, which catches `Err(..)` as well as a completed send, so a **refused** send reaches the margin check exactly as an instant one does and presents as `blocked 0`. The exact boundary is now **220,387 ticks, passing at 220,386** — one tick, at 870 controlling cycles, on the host, consistent with both emulators. **ROOT CAUSE FOUND: `queue_create` is refused after ~870 create/delete cycles.** The scenario creates and deletes its queue every cycle; `unwrap_or_default()` turns the refusal into a default handle; `queue_send` on it returns `Err(InvalidHandle)`; `pc 72`'s `_` arm treats that as a completed send; the margin check sees 0 ticks against 100 and flags an error. **Six layers between cause and symptom**, which is why the first five readings were wrong, three of them mine. **Not yet established**, and it is the one number that matters: whether this is a slot LEAK or an arena sized for fewer queues. A count of live queues across cycles decides it — flat means reclaimed, climbing means a leak. Two lesser defects found on the way and both fixed in the scenario: `unwrap_or_default()` swallowing a fallible create, and `_` as a match arm over a `Result`, which makes a refused call indistinguishable from a completed one and can mask a kernel defect indefinitely. (4) **DONE — both emulator hours are measured on the whole 25-scenario corpus and agree exactly: 24 of 25 on each**, `AbortDelay` the only failure on either, RV32 in 8 minutes and M3 in 13. This replaces a previously-recorded 18/18 on each, which covered a smaller corpus and was counted by a harness that could not tell `ok` from `FAIL`. *(A timing discrepancy is open and deliberately unexplained: 8 minutes against a recorded 58 for the same eighteen. Tick counts rule out a shortened build; beyond that it is not guessed at.)* (5) **`StreamBufferDemo` diverges on both emulators** at the pinned length — every counter and the line count matching, 194 bytes of trace text differing — while **passing** the hour, which is what a trace-text defect looks like rather than a scheduling one. **Refuted and recorded so it is not rebuilt:** a host callgrind stand-in for the C arm. FreeRTOS's tick self cost is 2.7% of its inclusive and ours is 13.1%, so a self-cost ratio measures how two codebases factor a tick, not what a tick costs — and perfect work parity did not save it. Every number, with its method line: **`docs/LEDGER.md`**. |
| **K4 heaps** | `heap_4` differential trace matches C; `StaticAllocation` on every cell; RAM table per profile | **PASSED 2026-09-10.** The `heap_4` differential matches: the C's algorithm transcribed over offsets rather than pointers. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K5 the Janus joint** | ~~Janus S1 on a Kairos kernel with the same numbers as on esp-rtos; the XIAO S3 Xtensa cell `Verified`~~ — **RE-SPECIFIED 2026-09-10, see `master-plan.md`.** The kill test as written pairs two halves that are blocked differently, and one of them cannot be unblocked by buying hardware. **K5a — PART DONE 2026-09-10, and its target CONFIRMED (after a false alarm of ours).** The row's target `rusty_esp_mid/firmware/xiao-s3-keys` **exists and its baseline is real**: `gh api repos/Remade-With-Rust/rusty_esp_mid/contents/firmware/xiao-s3-keys` lists a full cargo cell, and that repo's own ledger carries the M1 row of 2026-09-06 — **a P-256 signature is 95 ms and a verification 151 ms** on a `xiao-esp32s3-sense`, `opt-level=3 lto=fat metric=in-process-us`. A session working only from the local box read `rusty_esp_mid/firmware/` as empty and briefly recorded the opposite; the `/f/coding/janus/*` tree is a SCAFFOLD with empty `.git` directories, not a checkout, and an absence observed in one place is not an absence (`docs/LEDGER.md`). So **K5a's measurement clause is not blocked** — what is missing is a local checkout, and the joining act itself (a Kairos kernel inside a Janus firmware) is the owner's, since a Kairos session does not modify Janus repositories. **Settled while checking:** the esp-hal drift this row warns about is real and one crate wide — `xiao-s3-keys` pins `=1.2.0`, every Kairos S3 cell pins `=1.2.1`, while `esp-bootloader-esp-idf`, `esp-println` and `esp-backtrace` already agree. **The measurement clause is now ANSWERED (2026-09-11):** `rusty_rtos_kernel/firmware/xiao-s3-signing` runs the same workload — `p256 0.13`, the crate `rusty_esp_mid-core` signs with — on the same part, and times the kernel directly: **a scheduling round (queue send, queue receive, two context switches) costs 3,724 ns / 893 cycles, which is 39 ppm of a P-256 signature** (**re-measured 2026-09-21** — it read 8,313 ns / 1,995 cycles / 88 ppm until the cell was found to be hand-rolling a `NoTrace` that shadowed the crate's own and inherited `WANTS_NAMES = true`, so every traced event built a task name for a sink that drops it; no kernel change, and the clause's conclusion is unchanged and stronger); our sign/verify come out at 94.3/149.4 ms against their published 95/151 ms. The differencing experiment this row implies was tried first and FAILED — two 24.5-second batches cannot resolve microseconds, and it produced a scheduled arm faster than bare — so the kernel is timed on its own and the arms serve work parity instead. What remains of the clause is only the joining act itself, which is the owner's. **What was also doable here, and is done:** the conformance corpus runs on the XIAO S3 — `rusty_rtos_demo/firmware/xiao-s3-corpus`, all 18 scenarios byte-identical to the C kernel on **silicon**, poison-proven at one `exits` off. That makes four architectures (host, ARMv7-M, RV32, Xtensa LX7) and the first on a part rather than an emulator, and it proves the prerequisite any Janus workload on the S3 needs: the kernel runs there. **It also refutes this row's build assumption** — Xtensa needed *no port at all*, because a scenario is a state machine and a task owns no stack, so the context switch is what you need to host tasks with stacks, not what you need to prove conformance on a part. **K5b (blocked twice):** the `esp-radio-rtos-driver` joint and Janus S1 | **SPLIT, and the two halves are blocked differently.** **K5a is ANSWERED (2026-09-11, re-measured 2026-09-21):** `rusty_rtos_kernel/firmware/xiao-s3-signing` runs the same P-256 workload `rusty_esp_mid-core` uses, on the same part, and times the kernel directly — a scheduling round (queue send, queue receive, two context switches) costs **3,724 ns / 893 cycles, which is 39 ppm of a P-256 signature**. It read 8,313 ns / 1,995 cycles / 88 ppm until the cell was found to be hand-rolling a `NoTrace` that shadowed the one `rusty_rtos_core` ships; no kernel code changed and the clause's conclusion is unchanged and stronger. Our sign/verify are 94.3 / 149.4 ms against Janus's published 95 / 151 ms. **K5b is blocked twice, and neither is ours alone.** `esp-radio` reaches a scheduler through `esp-radio-rtos-driver`, and the kill test is **Janus S1** — two C6s, ESP-NOW, 1,000 frames each way — which needs (a) **2x C6**, the same purchase K3 wants, and (b) **S1's baseline taken on `esp-rtos` FIRST**, which does not exist yet. Do not schedule the kill test against a baseline nobody has taken. **Two concrete asks from the Janus side** (`docs/plans/janus-rtos.md`, written from the Janus tree 2026-09-20). (1) **Document the priority trap**: `start_scheduler` creates `Tmr Svc` unconditionally at `TIMER_TASK_PRIORITY` and `create_task` makes the highest-priority task current, so a firmware whose `main` sits below the daemon has the kernel believing a stackless task is running — every switch declined as `from == to`, workers starving at `laps 0/0`. It cost Janus a board run. (2) **Lift the radio adapter out of the firmware.** `rusty_rtos_port/firmware/xiao-s3-radio` implements all five `esp-radio-rtos-driver` traits and passes on silicon — further along than this plan says — but it is **1,464 lines inside a firmware directory**, so Janus cannot consume it. As a crate (`rusty_rtos_port-esp-radio`, or a feature on the port crates) the mesh node could take it the day S1's baseline exists. **Also worth knowing:** `esp-radio-rtos-driver` is **0.4.2** on crates.io, not the 0.4.1 this plan names in three places. Every number, with its method line: **`docs/LEDGER.md`**. |
| **K6 C ABI** | the unmodified C standard demo tasks link against `rusty_rtos-capi` and pass on Posix, then on QEMU M3 | **BOTH HALVES PASS as of 2026-09-11, with the order inverted** — the unmodified C demo files pass individually and together. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K7 libraries** | MQTT over our TCP for one hour, zero lost keep-alives; JSON suite 100 %; no-panic fuzz on every parser — **but to OUR broker, on OUR box** | **PASSED 2026-09-21, all three clauses**, and the target was changed 2026-09-10. The hour ran against `rumqttd` 0.20.0. Every number, with its method line, and the full record of how it was reached: **`docs/LEDGER.md`** (and each package's own ledger). |
| **K8 SMP / MPU / 1.0** | SMP demos trace clean on two cores; MPU refusal on M33; every hardening row complete; 1.0.0 tags; flips in order | **open, and the hardening clause is now MEASURED rather than guessed (2026-09-21): no package is above 63% and most sit at 22-31% of their v1.0.0 gates.** `rusty_rtos_kernel` moved 10/16 -> 13/16 by writing `docs/threat-model.md`, which closes H-01 and with it H-20 and H-41. **A fleet defect was found doing it**: every README claims an audit on 2026-09-16 that no plan records, so the tables do not regenerate from their source and `kairos harden` silently reverts them -- a hardening status that cannot be regenerated is not evidence whatever number it shows. Which side is true is an owner question and is recorded rather than guessed. **open — and the REASON for the SMP half changed 2026-09-10.** SMP was scoped as completeness. It is not: the ESP32-S3 is dual-core and **IDF-FreeRTOS is SMP**, so SMP is a hard prerequisite for Janus Track A, which is where most of our own devices run today. It pairs with K6 (see that row) — ABI without SMP does not reach Track A, and SMP without ABI has nothing to link. **And the 1.0 story should lead with the corpus, not with memory safety.** "Memory-safe RTOS" is a claim a well-capitalised incumbent can simply buy — AWS already paid for CBMC proofs on the C kernel. *"Byte-identical scheduling decisions to the kernel you already shipped, proved against its own trace on three architectures, with the diff checked in"* is the claim that answers the only question a buyer with certified firmware asks, which is **what breaks if I swap this**. The corpus is not QA; it is the thing that makes the wedge credible |

### 6.1 K5, as a checklist rather than a paragraph

The K5 row above is prose, and prose cannot be evaluated. It was asked on
2026-09-11 whether K5 was "done" and the row could not answer, because a
kill test written as a narrative has no state. So K5's condition is restated
here as things that are either true or not, each with the command that says
which.

**K5a — the kernel on Xtensa. COMPLETE 2026-09-11.**

- [x] `vTaskDelete`, both paths, gated by `death.c` byte-identical to the C
      kernel — `kairos conform death`
- [x] the conformance corpus on ESP32-S3 **silicon**, 18/18 byte-identical —
      `kairos check rusty_rtos_demo --board`
- [x] the kernel's cost against a real Janus workload, measured: **88 ppm of
      a P-256 signature** — `kairos check rusty_rtos_kernel --board`
- [x] an Xtensa context switch, 100/99 resumptions from three call frames
      deep — `kairos check rusty_rtos_port --board`
- [x] every one of those re-runnable rather than re-readable — `--board`
      exists, and a cell that prints no verdict FAILS
- [x] the upstream finding drafted —
      `kairos-upstream/drafts/esp-radio-rtos-driver-stack-contract.md`

**The whole of K5a is therefore: `kairos conform --all` reports 19/19 and
`kairos check rusty_rtos_kernel rusty_rtos_port rusty_rtos_demo --board`
passes.** Both hold, **re-verified 2026-09-11** after the day's port changes —
19/19, `check: 5 package(s) passed`, and five board cells PASS:
`xiao-s3-cycles`, `xiao-s3-signing`, `xiao-s3-radio`, `xiao-s3-switch`,
`xiao-s3-corpus`.

**That re-verification took six attempts, and the first five failed on the GATE
rather than the kernel.** Three defects, all of them naming the code for
something that was not the code: `stderr` was discarded so a busy serial port
read as "a broken context switch presents exactly this way"; the lockfile
restore ran before the `--board` and `--qemu` builds, so a run left an H-07
failure for the *next* run; and a flat 300-second budget priced duration where
it meant silence, while `xiao-s3-signing` legitimately spends 196 seconds in
timed batches. All three are fixed and the details are in `docs/LEDGER.md`.

The lesson is the one this section already encodes: a claim is worth what its
command is worth. §6.1 replaced prose with a checklist so K5 could be
evaluated; the six attempts showed that **the checklist is only as good as the
gate behind it**, and that a gate which fails for environmental reasons is
worse than no gate, because it teaches the reader to discount it.

**K5b — the radio joint. BLOCKED, and not on us.**

- [x] the five `esp-radio-rtos-driver` traits implemented against our kernel
      — **2026-09-11**, `rusty_rtos_port/firmware/xiao-s3-radio`, building for
      `xtensa-esp32s3-none-elf`. It TYPE-CHECKS; `nm` finds zero `esp_rtos_*`
      symbols of 2,295, because LTO drops them with no consumer linked
- [x] ...and the SEAM working end to end on silicon — **2026-09-11**, 50 and
      50 hand-offs through `task_create` and `Semaphore::take`/`give`, zero
      faults. It first failed, and the failure was a KERNEL assumption:
      `port_yield` committed the switch before a stacked port could enact it.
      `Port::COMMITS_SWITCH` splits deciding from committing; the corpus
      stayed 19/19 byte-identical throughout, and ARM turned out to be
      affected too — its window opened 199 times in 200 rounds and is now 0
      (`mps2-an385-qemu-preempt`, `docs/LEDGER.md`)
- [ ] ...and LINKED against a real `esp-radio` — **the companion-set blocker
      is GONE, re-checked 2026-09-21, and what remains is different and
      smaller.** The old entry said the only published radio pinned `esp-hal
      ~1.1.0` + driver **0.3.0** against every Kairos cell's `esp-hal =1.2.1`,
      with `xtensa-lx-rt` a `links` crate so cargo refused the pair. That was
      true then. Now `esp-radio 1.0.0-beta.1` with `esp-radio-rtos-driver
      0.4.2` **resolves and builds against our exact `esp-hal =1.2.1`** — 193
      packages, exit 0, no `esp-hal` complaint at all. The cell's driver pin
      is bumped to `=0.4.2` accordingly.
      **But adding the dependency links nothing**, measured as a clean A/B
      with the binaries compared byte for byte: 276,820 bytes and 2,416
      symbols either way, the 64 symbols differing each way being our own
      names whose crate-disambiguator hash moved between compilations. LTO
      drops esp-radio because nothing in the cell CALLS it. Meeting this item
      needs a real Wi-Fi controller path and the board to verify it — which is
      **unblocked work rather than a pin decision nobody could take**
- [ ] Janus S1 on a Kairos kernel — needs **2x ESP32-C6** in hand
- [ ] ...and its esp-rtos baseline, which **Janus must take first**; §2.8

Neither K5b item can be moved by work in this repository. K5 is complete in
the half that was ours.

The sequence is fixed: **sim → IPC → silicon → heaps → Janus → C ABI →
libraries → SMP**. Nothing on a chip before the trace is clean on the sim;
nothing in a library before the kernel it needs has a `Verified` cell.

**One open question against that sequence, raised 2026-09-10 and not taken
unilaterally.** K6 (C ABI) and K8 (SMP) are the pair that unlocks Janus
Track A, and Track A is where most of our own devices already run. K6 is also
the wedge against FreeRTOS's real moat — the installed base and the C API,
not the code, which was given away MIT on purpose. That is an argument for
K6 ahead of K7. It is **an owner decision**, recorded here rather than acted
on, because re-ordering a sequence declared fixed is not a session's call.
The strategic reasoning is in `master-plan.md`.

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
| 2026-09-20 | **That measurement is made, and the answer is smoltcp.** 78 entry points, 8 gaps, 7 of them the zero-copy pointer handout and 1 a missing getter. The from-scratch stack stays shut. The replacement for the zero-copy family is a closure API that does the same job, and it is a DIFFERENT SIGNATURE — so the socket slice owes an explicit note wherever a Kairos firmware cannot be a line-for-line port of a +TCP one. |
| 2026-09-16 | **Measure the C against a third-party corpus BEFORE adopting both as one target.** coreJSON scores 100 % on JSONTestSuite, so "agree with the C" and "pass the suite" coincide for `rusty_rtos_json` — but that was measured first, and the two are kept as separate tests so the day they diverge is visible rather than silently resolved. Any K7 package adopting an external corpus does this first. |
| 2026-09-16 | **A vendored corpus is data, and its repository must be told so.** `core.autocrlf` was silently normalising 10 of the 318 JSONTestSuite files, so a Linux checkout would have read different bytes from a Windows one. No verdict was at risk (measured both ways, 318/318 either way), but a corpus whose contents depend on the machine that cloned it is not a fixed denominator. `.gitattributes` marks `oracle/** -text` in any package that vendors one. |
| 2026-09-16 | **A poison that does NOT fire is a finding, not a dropped test.** Making `rusty_rtos_json`'s universal byte reader panic on any read past the end left all 11 tests passing, which proves no scanner reads out of bounds in the first place. Recorded next to the function with the measurement, because the property holds only while every caller is right. The two bounds that ARE load-bearing were poisoned and both fired. |
| 2026-09-17 | **A trace of the output cannot say "and nothing else".** A poison that made an mqtt writer emit one byte BEYOND the length it reports passed the byte-for-byte differential, because a differential only ever looks at the bytes it was told about. A caller packing data after the header would have had it clobbered. "Wrote the right bytes" and "wrote only those bytes" are two claims; the second needs a filled buffer and its own assertion, and every writer in K7 from here on gets one. Third member of the family that started with the fixed header's over-large byte count: **a differential bounds what the C ANSWERS, never what it TOUCHES.** |
| 2026-09-17 | **A differential compares the STATE LEFT BEHIND, not only the answer.** Three K7 slices in a row have turned on it: sntp's client (the context after every action), mqtt's state machine (both record arrays), mqtt's property reader (the cursor and the budget). The last is the sharpest — a refused string has already consumed its length bytes, and a transcription that checked first would look more correct and disagree with the C on every malformed packet. If a module leaves a cursor, a budget or a container behind, diff that. |
| 2026-09-17 | **Provably dead code in the oracle is KEPT, and pinned by arithmetic.** mqtt's property length decoder has an in-loop range check that cannot fire: the multiplier guard bounds the value to exactly one less than the constant it tests. A differential arm does not tidy its oracle, so it stays — and a unit test pins the bound, because it is a relationship between two constants either of which could move. |
| 2026-09-17 | **The lockfile trap has now cost three commits in one session.** `cargo generate-lockfile` with the umbrella's patch moved aside, then commit the lockfile, then run nothing that invokes cargo in between — a `cargo test` puts the patched version straight back. `kairos check`'s H-07 gate catches it every time, so the habit that actually works is running `kairos check <package>` BEFORE the commit rather than after. |
| 2026-09-17 | **Where the input space is small enough to ENUMERATE, enumerate it — and keep named cases beside the digest.** MQTT's fixed header is 256 type bytes x an eight-value length alphabet^4 x six claimed lengths = 6.29 million calls, compared as per-status counts plus an FNV-1a digest of every answer, with 30 named cases printed in full. The digest proves agreement everywhere; the named cases say WHERE when it breaks. All eight poisons fired first time — the first K7 slice where none needed a scenario adding, and the sweep is why. |
| 2026-09-17 | **The first place in K7 where the Rust is strictly SAFER than the C on the same inputs, not merely equivalent.** `MQTT_ProcessIncomingPacketTypeAndLength` takes a pointer and a count and trusts the count, so an `available` larger than the allocation reads past the buffer. Ours uses the count only as an upper bound on a `get`. Worth recording as a category: a differential proves we match the C's ANSWERS, and says nothing about what the C does on inputs that violate its own preconditions. Those are worth a test of their own. |
| 2026-09-17 | **A library too big to remake in one go gets sliced at the SELF-CONTAINED seam, and the slice is named in lines.** coreMQTT is 21,102; `core_mqtt_state.c` is 1,206 and includes nothing but its own header, so it is the only part that is a complete provable unit today. `rusty_rtos_mqtt` says "7.7 % remade" in its README, its plan and its crates.io description, because "coreMQTT remade" would be false. `rusty_rtos_tcp` will need the same treatment. |
| 2026-09-17 | **When state is a container, the differential compares the CONTAINER, not the return values.** MQTT's record arrays are in resend order, which the protocol requires; comparing statuses alone would pass a transcription that reordered a resumed session's backlog. The rule generalises: if the module's job is to remember something, diff what it remembered. |
| 2026-09-17 | **The third "poison that did not fire" of K7, and the third time it split into the same two kinds.** One was a workload gap with a real defect behind it (the ack/QoS check is only observable for a QoS 1 publish handed a PUBCOMP, where the wrong answer is a LEGAL transition); the other was a genuine property, pinned by a unit test. Asking which kind it is, every time, is now the habit. |
| 2026-09-19 | **When a function has no observable behaviour, say what the oracle IS instead of pretending it is a run.** coreMQTT's last two functions log and return nothing, and the shipped config compiles the logging out — so no differential can see them, and turning the logging on would mean building the pin with a configuration it does not ship, which every other arm of this oracle refuses to do. They were held back for six slices rather than claimed on a technicality. The answer was to make the oracle the SOURCE TEXT and say so: a checked-in script parses both switches into a trace, a test sweeps all 256 codes against it in each direction, five poisons on the table all fire, and the test's own module note says in its first paragraph that this is a weaker instrument than a differential and why. A weaker instrument named accurately is worth more than a strong one claimed falsely — and it still caught a defect: `0x80` is Unspecified Error and its message says "Connection rate exceeded". |
| 2026-09-19 | **A poison that cannot fail is sometimes telling you about the ARM, not the trace.** mqtt's receive-loop slice poisoned away the line that empties the acknowledgement-property buffer after a send, and nothing failed — even with a case that sends TWO acknowledgements with properties in one pass. The reason was not a workload gap: this crate's `AckReply` started a FRESH `PropertyBuilder` on every callback, where the C's builder lives in the context and its `currentIndex` survives between acknowledgements. The two agreed only because the C resets, so the reset could not be load-bearing here. The fix was a `resume` constructor, and the poison then fired. The existing taxonomy had four kinds of dead poison — workload gap, genuine property, over-determined, and one that does not change what it names — and this is a fifth: **a poison the transcription has already made unreachable.** |
| 2026-09-19 | **Two test packets that differ only in a field the trace does not record are ONE packet sent twice.** The receive loop handles two packets from a single read, and the case for it used two QoS 0 publishes differing only in their topic — which the callback log does not carry. So a loop that handled the FIRST one twice produced an identical line, and three poisons about the buffer index and the memmove all passed. Giving them different packet identifiers made all three fail at once. The sibling of the twenty-first shape: it is not enough for the inputs to differ, they must differ in something the INSTRUMENT prints. |
| 2026-09-19 | **Two types with the same fields are not the same type if their ZERO means different things.** mqtt's connect slice started by merging `connack::ServerSettings` into `context::ServerLimits` — field for field identical, and the twenty-second shape of the guard says to. It is wrong and it would have been a defect: the CONNACK reader starts every field at zero and reports what was PRESENT beside it, while the session's copy starts at MQTT's defaults, where an absent Maximum QoS is 2 and an absent Retain Available is yes. Assigning one to the other caps every publish at QoS 0 the moment a broker leaves the property out. The C has one struct and no merge because its deserializer writes STRAIGHT INTO the context the initialiser already filled; this arm writes the merge out field by field, gated on the present-bits. The guard tests whether two things can be told APART — and a zero element is a way to tell them apart. |
| 2026-09-19 | **A poison with no caller is not a workload gap.** `recvExact` sets the connection to disconnect-pending when a read fails, but only if it is already CONNECTED — and during `MQTT_Connect` it never is, because the status is not set until the CONNACK has been accepted. So deleting that branch changes no line of a 39-line trace, and no case can be added that would: the branch belongs to the process loop, which does not exist yet. Recorded as such rather than closed, which is the difference between a gap and a boundary. The sibling verdict in the same run: refusing a packet larger than the network buffer is subsumed by the SLICE BOUND, because a slice of 257 bytes from a 256-byte buffer is `None` whether or not the check is there. |
| 2026-09-19 | **A gather boundary is invisible to a transport that is offered one vector at a time — the twenty-seventh shape of the guard.** mqtt's outgoing slice splits a SUBSCRIBE across several calls to the vector sender, and three deliberate breakages of how many filters go in each — mis-counting the per-topic cost either way, and a strict inequality in the guard — changed NO line of a 49-line trace. The vectors are offered one at a time in the same order however they are grouped, so the grouping simply is not in the log. The instrument that can see it is `writev`, which coreMQTT calls with the outstanding vectors AND THEIR COUNT; implementing it made all three poisons fail at once. The general form: when a poison does not fire, ask what INSTRUMENT would see it before assuming the behaviour does not exist — and here the answer was a branch of a function already claimed as remade, which made the claim half true. |
| 2026-09-19 | **A poison that a CONSTANT makes dead is not a workload gap.** Two of twenty-seven still do not fire after that, and no case can make them. At `MQTT_SUB_UNSUB_MAX_VECTORS = 4` a SUBSCRIBE's room is ONE vector and the cheapest filter costs TWO, so the first filter ends the gather whether or not the options byte is counted; and `room + per_topic == MAX` exactly, so a correct loop can never overflow the array. Both are properties of the number, not of the code, and the pin is a unit test written in terms of the three constants rather than a case. The same reading found the C's guard is unsigned — `4U - 3U` — so a user-set maximum below 3 wraps it to SIZE_MAX and the loop writes past a stack array, a CWE-787 reachable by configuration (§5.4). A dead poison is worth the time it takes to explain: this one was worth a finding. |
| 2026-09-19 | **Two functions that differ in one character need a case that turns on that character.** coreMQTT's two senders sit thirty lines apart: `sendMessageVector` compares elapsed time with `>` and reads the clock only when it will go round again, `sendBuffer` compares with `>=` and reads it every turn including after a failure. This crate gave both the second shape, and a 28-line differential agreed — because no case landed elapsed time exactly on the timeout in the VECTOR sender, and nothing recorded the clock-read count. Adding both made `calls=3` against `calls=2` on two cases of the same name. Found by reading the C to write the NEXT slice, not by testing the last one, which is the argument for transcribing a function even when a caller of it already passes. |
| 2026-09-18 | **Some poisons do not fail — they HANG, and a harness must survive the code it is breaking.** mqtt's send slice replaced `calculateElapsedTime`'s wrapping subtraction with a saturating one, which is what a careful reader would write: a clock past 0xFFFFFFFF then reports zero elapsed for ever and the sender never returns. That is the strongest possible catch and the differential cannot see it, because a hang is not a failing assertion. Two of twelve poisons behaved that way. The runner now bounds every run and kills the PROCESS TREE — killing `cargo` alone leaves the test binary holding its own output file, and the next build fails to link — and reports HANGS as its own verdict. The property itself is pinned by a unit test on the arithmetic, which is the only place it can be stated. |
| 2026-09-18 | **A differential case that cannot terminate is not a case.** The same slice wrote a "frozen clock" case: a transport that never accepts and a clock that never advances. coreMQTT's own config header says that if the time function is a no-op then MQTT_SEND_TIMEOUT_MS must be zero, so that combination loops for ever BY CONSTRUCTION. It is a documented precondition, not a defect, and the case came out of the driver with the reason written beside it. The sibling of "a trace should ask only what both arms can answer": a trace should also only ask questions that have answers. |
| 2026-09-18 | **A percentage is a claim, so make it a SCRIPT.** K7's mqtt ledger had divided a hand-kept sum of per-slice line counts by all 15,643 lines of coreMQTT. The numerators used a looser boundary than the denominator — they counted the comment blocks BETWEEN functions — so the two were never on one basis, the figure was inflated against a strict count, and it could never have reached 100 % because 2,577 of those lines are includes and doxygen. `oracle/coverage.py` now reads the pinned source, enumerates every function definition and checks each against a checked-in `REMADE.txt`; `--check` fails if that list names a function upstream does not have. THE HEADLINE IS FUNCTIONS, not lines: a function is the unit that can be transcribed and diffed. Second counting error in this package after the double-counted reader, and the same fix both times. |
| 2026-09-18 | **The TYPE as a refusal is the null-pointer family one level up.** mqtt's client slice met a QoS of 3 and a retain-handling option of 3: `MQTTQoS_t` holds them, Rust's `QoS` does not, so a caller cannot build the call at all and `validateTopicFilter`'s two `> 2` checks are unreachable here rather than transcribed. Those cases came OUT of the driver, like the null pointers and the pointer-and-length pairs before them — a trace should ask only what both arms can answer. The defect the slice was after was then demonstrated with an empty topic filter instead, which both arms can express. |
| 2026-09-18 | **A GRID is to a string algorithm what a printed accepted set is to a table.** Every sweep in K7 walks one byte over 256 values and PRINTS what it accepted, because a digest says a table changed and a printed set says which entry moved. Topic matching takes two strings, so the same idea needs two dimensions: every string over `{a, /, +}` to length three as a 39x39 matrix in the trace. The divergence it found — a trailing `+` losing an empty last level — is a COLUMN PATTERN, not a case anyone had to suspect first. The guard grew a twenty-fifth shape for it: a grid must vary in both directions, since a matcher that refused everything would still give a diagonal (every filter is a legal topic name and matches itself) and one that accepted everything would give a solid block. |
| 2026-09-18 | **An asymmetry in a table is the thing a transcription smooths out, so sweep the whole domain.** `MQTT_GetPacketTypeString` names PUBLISH by its NIBBLE — its low bits are QoS, DUP and RETAIN — and every other type by its WHOLE BYTE, because a PUBREL's reserved bit must be set and `0x60` is malformed rather than a PUBREL. Our first transcription used the whole byte throughout, which is the tidier-looking rule and wrong for sixteen values. The 256-value digest failed on the first run; a table of named representatives would have agreed, because the sixteen wrong answers were all unprinted. |
| 2026-09-18 | **A transcription that CANNOT follow its oracle has found something — and this is what the project is for.** Fifteen mqtt slices reproduced coreMQTT exactly, divergences from MQTT 5.0 included, and eight upstream reports came out of putting two of the library's own answers side by side. The sixteenth could not: `addPropUtf8` reports success while writing one byte past a buffer, and a `&mut [u8]` under `forbid(unsafe)` has no way to do that. The overflow surfaced as a differential FAILURE rather than as a reading, and the only question left was which arm was right. The rule that follows: when a transcription refuses to match, ask whether the C is doing something the language forbids before assuming the transcription is wrong. |
| 2026-09-18 | **An exception must be smaller than the rule, and bounded from both ends.** Three of 73 trace lines could not match, so the comparison became conditional — which is a hole. It is closed by a second test: exactly three lines, each one the C itself marked `OVERFLOW`, each a four-byte buffer accepting a five-byte write, and sixty-odd lines still compared without exception. A differential with an unbounded exception list is a differential that can be quietly widened until it proves nothing. |
| 2026-09-18 | **Two bounds beat one, and the second should not be code.** Two deliberate off-by-one errors in mqtt's property builder — including the EXACT arithmetic that makes the C overflow — changed no answer at all, because the size check is advisory there and the slice bound is the guarantee. Both poisons 'failing' is the strongest statement of the thesis the campaign has produced, and it is now a test rather than a claim: every adder against every buffer size from 1 to 23, asserting the cursor never passes the buffer and a refusal writes nothing. |
| 2026-09-18 | **Before writing a function, grep your own crate for its shape — the law the house has for C code, applied to itself.** mqtt's fifteenth slice wrote a second Rust copy of `MQTT_ProcessIncomingPacketTypeAndLength` and `processRemainingLength`, which the SECOND slice had already remade, and counted their 138 C lines again: 45.2 % should have been 44.3 %. It was caught by the slice's own instrument — driving the two header readers side by side put the duplicate next to the original — and the differential passed UNCHANGED after being re-pointed at the original, which is the proof they were one function. The duplicate is deleted and every number it touched is corrected. **A percentage is a claim, and a claim nobody recomputes is one that drifts.** |
| 2026-09-18 | **A trace should ask only what both arms can answer — a sentinel printed in both columns is a constant compared with itself.** mqtt's last serializer slice met three C refusals with no reachable equivalent in Rust, all one shape: a POINTER AND A LENGTH that must agree and are never checked against each other (`MQTTPropertyBuilder_Init`'s buffer and length; `MQTT_ValidatePublishParams`'s topic name and its length). A slice carries both, so the question cannot be asked. The cases were REMOVED FROM THE DRIVER rather than faked, and the one bound that is real but needs a 256 MB buffer to reach is pinned by a unit test on the arithmetic. The same edit applied to the OUT-PARAMETERS: the C leaves its sentinels alone on a refusal, so the driver now prints them only on success. |
| 2026-09-18 | **Where a library does one job twice, run both over the same input and print both answers.** mqtt's fifteenth slice had two fixed-header readers and a third copy of the CONNECT property table in one differential, and simply printing the pairs side by side produced three upstream findings in one run — including one with real severity, a reader that reports a well-formed packet as malformed whenever a TCP segment boundary lands inside its header. The guard grew a twenty-second shape for it: TWO ARMS THAT NEVER DISAGREE ARE ONE ARM DRIVEN TWICE, so the trace must hold cases where they agree, cases where they part, and a sweep showing the parting is systematic. |
| 2026-09-18 | **A sweep says what a table ACCEPTS; only reading the C says what it REMEMBERS.** mqtt's six outgoing property validators were driven from 36 sweeps — 9,216 calls, every identifier at every value shape — and the driver built from them produced 49 cases that passed first time. Transcribing the C afterwards added seven cases, and **three of the nine poisons are caught only by those seven**: a status passed through from a shared decoder, a bound's inclusivity, and a duplicate flag declared inside a loop rather than outside it. Each spans two properties or two checks, and a sweep sends ONE property at a time at ONE value. Measured by rerunning the poisons against the 49-case trace rather than argued. The sweeps are still what found the three defects; the two instruments answer different questions, and a slice needs both. |
| 2026-09-18 | **Where a function takes a CALLBACK, compare the call sequence — orderings live there that no status can show.** mqtt's transport reader checks the packet type BEFORE reading the length and its multiplier guard BEFORE each read; both mean a malformed stream costs the peer one byte instead of a drained header, and both answer the same status either way. Poisoning each is caught only by the count. Fourth K7 package to use the shape `rusty_rtos_sntp`'s client established, and the first where the finding is about how much of a HOSTILE INPUT gets consumed. |
| 2026-09-18 | **An ADDITION to the C's answer does not belong in the trace.** `MQTT_GetIncomingPacketTypeAndLength` never sets `headerLength`, so a C caller counts the bytes itself; we counted them anyway and return the number. It is pinned by a unit test and kept OUT of the differential, because a trace line one arm invents is not a comparison — it is the same rule that withdrew the CONNECT's null-pointer case, seen from the other side. |
| 2026-09-18 | **When a library validates the same thing twice, DIFF THE TWO VALIDATORS — one of them may be right.** coreMQTT checks publish-acknowledgement reason codes per packet type on the way out (nine for a PUBACK, two for a PUBREL: exactly MQTT 5.0) and against one shared table of ten on the way in. So it refuses to SEND a PUBACK carrying `0x92` and accepts one. The finding was already drafted from the reading side alone; seeing the writing side changed what the fix is, from "write a table" to "use the one three thousand lines up", and is far harder for a maintainer to dismiss. |
| 2026-09-18 | **A poison must change the thing it NAMES, and a miss proves nothing until you have read it back.** "Check the filters before the buffer" was written as a removal of the buffer check rather than a reordering — a different experiment, subsumed by the slices below it, reporting a miss that was not one. Rewritten to reorder, it was caught at once. **Before drawing any conclusion from a poison that did not fire, re-read the poison.** That is now the fourth thing to check, alongside a workload gap, a genuine property, and an over-determined case. |
| 2026-09-18 | **A comment that states a relationship is a CLAIM, and a claim in a comment is one nobody runs.** I wrote that coreMQTT's publish-header serializer reports the size it COMPUTED rather than the bytes it wrote, and added a `debug_assert` beside it saying the first was never smaller. It differs in BOTH directions, and four new cases refuted it on their first run. **Where a comment asserts a relationship, either test it or do not assert it** — the comment was right about the mechanism and wrong about the consequence, which is the most expensive shape of wrong. |
| 2026-09-18 | **Where several functions share one body, run them TOGETHER in the differential.** coreMQTT has three outgoing-PUBLISH serializers over one `serializePublishCommon`, and the property a caller depends on is that the short output is a prefix of the middle and the middle of the long. Three separate differentials could each pass while that broke. The same shape as the packet-size calculators' cross-slice test, and it belongs in the TRACE rather than in a test on one arm. |
| 2026-09-18 | **An API contract the corpus always keeps is an API contract nobody has tested.** The C's comment says calling the size function before the serializers is "part of the API contract", and every case did — so two numbers that differ only when it is broken were always equal, and a poison swapping them passed. **When a comment says callers must do X, add the case where they do not.** |
| 2026-09-18 | **Where every field is length-prefixed, only a BYTE comparison can see an ordering mistake.** Swapping a CONNECT's will topic with its will payload, or its user name with its password, produces a packet that parses cleanly and means something else entirely — the will goes to the wrong topic, the password is sent as the user name. Four such swaps are poisons in mqtt's CONNECT slice and all four are caught by the byte-for-byte trace; a differential comparing PARSED FIELDS would have passed every one. The rule: **compare the bytes wherever the format's own framing would hide the mistake.** |
| 2026-09-18 | **A limit no input in the corpus can approach is decorative, and the fix is cases BUILT for it.** All five of `MQTT_GetConnectPacketSize`'s 16-bit field checks were unreachable from a table whose longest field was four bytes, so a poison on each passed. Seven cases now put one field at a time on 65,535 and 65,536. Same lesson as the packet-size calculators' 268,435,455 boundary — and it had to be learned again in a slice where the limit was four orders of magnitude NEARER and still out of reach, which is the part worth remembering: "close enough to plausibly hit" is not a measurement. |
| 2026-09-18 | **A slice cannot lie about its length, and that deletes a whole class of check.** The C's final CONNECT remaining-length limit is load-bearing against a property builder claiming 268 million bytes while pointing at eight — the function reads `currentIndex` and never touches `pBuffer`. A `&[u8]` has no second number to disagree with itself, so the differential CANNOT reach the check and the input class does not exist. Recorded as a property rather than papered over with a case one arm cannot replay; the same reasoning withdrew the null-pointer case in the same slice. |
| 2026-09-18 | **A correct arm is the best instrument for finding a wrong one.** mqtt's DISCONNECT reason code is validated by ONE table read with a direction flag, so it was swept 256 times in each direction and both accepted sets printed. The outgoing set matched MQTT 5.0 §3.14.2.1 exactly — and that is what turned the incoming set's missing `0x9F` from "plausible" into "the only difference". Where a system does the same job twice under a flag, exercise both and DIFF THE ANSWERS; the half that is right calibrates the half that is not. |
| 2026-09-18 | **A scope claim is checked against the ORACLE's function list, not against how complete the last slice felt.** "Reads every packet a broker can send" went into `rusty_rtos_mqtt`'s README after the PUBLISH slice and was wrong by one packet, because MQTT 5 made the DISCONNECT bidirectional. The overstatement is recorded in the README rather than quietly corrected: a README that overstates once is read sceptically forever, and the correction is the interesting part. Every future scope sentence gets its enumeration checked before it is written. |
| 2026-09-18 | **A case that fails for the WRONG reason is worse than no case, because it looks like coverage.** Two DISCONNECT poisons survived because the case meant to catch each was malformed a SECOND way — a trailing byte, a short section — so the property walk refused it before the check under test could matter. **When a poison does not fire, check whether the case is over-determined before concluding anything about the code.** That is now a third possibility alongside the two the house already asks about (a workload gap, a genuine property). |
| 2026-09-18 | **"It fits" is not an invariant; "the parts reconstruct it" is.** The obvious assertion for a PUBLISH's payload — that it is no larger than the buffer — was MEASURED to be vacuous: the mutation that silently empties the payload passes it. What has teeth is the identity, that the topic-length bytes, the topic, the packet id, the encoded property length, the properties and the payload sum to the remaining length. **Where a parser splits an input into parts, assert the split, not a bound on one part** — and measure the assertion by mutating the thing it is supposed to watch. |
| 2026-09-18 | **A byte that changes the SHAPE OF THE REST of the input needs one sweep per shape, just as one selecting values of different widths does.** QoS is two bits of a PUBLISH's first byte and it decides whether a packet identifier follows the topic — so the sixteen-value flags nibble needs two sweeps, not one. Third entry in the sweep trio (2026-09-17 enumerate, 2026-09-18 print, 2026-09-18 vary the shape), and the one that generalises furthest: a sweep proves something only about the shape it swept. |
| 2026-09-18 | **Three checks can be one finding, and should be written as one.** All three `checkPublishRemainingLength` calls survived poisoning for the same reason — each is the predicate of the bounded slice that follows it, and the C needs them only because it indexes with a length it was handed. Recorded as a single entry rather than three, because three would have implied three investigations. Fourth appearance of the family that began with the size calculators. |
| 2026-09-18 | **A mutation that changes no answer is INERT, not undetected — and the difference belongs at the call site.** Emptying the PUBLISH payload's error path passed every test, because the slice can never fail: its offset plus its length is the remaining length exactly, and an earlier slice already required that much buffer. That is now a comment where the code is, because "this cannot fail" is a fact a reader will otherwise re-derive, or quietly come to rely on without checking. |
| 2026-09-18 | **Enumerating an input is not the same as exercising it: a one-byte sweep must vary the SHAPE of what the byte introduces.** mqtt's CONNACK property identifier selects a value of one of five widths, so a body sized for one is malformed for the other four — both arms refuse for the wrong reason, every line matches, and the sweep has discriminated nothing. Swept five times, once per shape, the accepted sets are the table and say which identifier is which type. The rule sits directly under 2026-09-17's "enumerate where you can" and 2026-09-18's "print the accepted set": all three are about a sweep being able to fail. |
| 2026-09-18 | **A sweep that confirms conformance is a result, and gets asserted the same way as one that does not.** The ack deserializers' tables diverged from MQTT 5.0 in three places; the CONNACK's 22 reason codes and 17 properties are exactly the specification. That is worth an assertion against the CHECKED-IN TRACE rather than a sentence in a README — the event it reports is the pinned oracle drifting, which is the event worth being told about either way. |
| 2026-09-18 | **Where the C's STATUS carries information, a `Result` must not throw it away.** `MQTTServerRefused` means the CONNACK parsed and the broker said no, and the C fills its out-parameters on it because the Reason String explaining the refusal is in the property section. A refusal is therefore `Ok` in the transcription, with `refused()` true. Mapping it to `Err` would have looked tidier and discarded the one thing a refused client needs. The general form: **map the C's statuses by what they CARRY, not by whether they are named like errors.** |
| 2026-09-18 | **A check can be redundant in the ORACLE, not only in the transcription — the third shape of this family.** The size calculators' in-loop check is load-bearing in the C (wrapping arithmetic) and subsumed in ours (saturating); the ack deserializers' property bound is load-bearing in the C (pointer arithmetic) and subsumed in ours (a slice); the CONNACK's three-byte minimum is subsumed in BOTH arms, because `decodeVariableLength` refuses a zero-length buffer on its own. All three are kept for fidelity and pinned by a test that records why removing them is free — and a test of that kind is not supposed to fail when the check is loosened. That IS the finding. |
| 2026-09-18 | **A one-byte decision table is swept and PRINTED, not hashed — and that is how the third K7 package found defects in its own oracle.** mqtt's acknowledgement deserializers make three decisions by switching on a single byte; each is swept over all 256 values and the trace carries the ACCEPTED SET in full. Twelve values out of 256 is small enough for a reader to check against the specification, and reading it turned up three divergences from MQTT 5.0 that a digest would have concealed behind a passing test. The rule generalises: **enumerate where you can (2026-09-17), and where the accepted set is small, PRINT it.** |
| 2026-09-18 | **A divergence from the SPECIFICATION is transcribed, and asserted from the TRACE.** The Rust arm reproduces all three faithfully, because it is a transcription and its oracle is the C, not the standard. The test that records them reads the checked-in trace rather than our own code — so what fails the suite is the pinned oracle changing its mind, which is the event worth being told about. |
| 2026-09-18 | **Where the differential's own input would make the C read out of bounds, the case does not belong in the trace.** `MQTTPacketInfo_t` carries a pointer and a claimed length that an attacker can make disagree, and that IS the attack — but the C's answer then depends on whatever is next in memory, so the line would be a coin toss that happens to be reproducible on one machine. The driver asserts the two are equal; the over-claim is pinned on the Rust side alone. Fifth member of the family: **a differential bounds what the C ANSWERS, never what it TOUCHES** — and it cannot even bound the answer where the C's preconditions are violated. |
| 2026-09-18 | **A check load-bearing in the C can be subsumed in the transcription, for a second distinct reason.** The size calculators' in-loop check guards a wrapping `uint32_t` that we saturate; the ack deserializers' property bound guards pointer arithmetic that we do with a slice. Both are kept and both are pinned by a test that records the equivalence — which means neither test fails when the check is loosened, and that is exactly the content of the finding. |
| 2026-09-18 | **A boundary case must land EXACTLY on the comparison it is meant to exercise.** mqtt's size calculators guard MQTT's 268,435,455 limit, and no plausible subscription list gets within three orders of magnitude of it — so the limit poisons passed on an unreachable check. Adding property-length cases *near* the limit was still not enough: a case that overshoots refuses for the same reason whether the operator is `>=` or `>`. Compute the input backwards from the arithmetic so the check sees the exact value, and name the case for what it tests. |
| 2026-09-18 | **A check that is load-bearing in the C can be redundant in the transcription when the arithmetic differs — keep it, and pin WHY.** The in-loop overflow check in `calculateSubscriptionPacketSize` guards a wrapping `uint32_t`; ours saturates, so the final check refuses the same input regardless. A differential arm does not tidy its oracle, so the check stays, and a unit test records that removing it would be safe here and unsafe there. Fourth non-firing poison in mqtt alone, and the third that was a genuine property rather than a workload gap. |
| 2026-09-18 | **Two slices that each agree with the C still need a test that they agree with EACH OTHER.** A size calculator and the writer it feeds can both pass their differentials while disagreeing about the same packet, because neither arm ever sees the other. K7's first cross-slice test reconciles them directly, and every later pair of slices in a package gets one. |
| 2026-09-18 | **A `Result` cannot reproduce a half-written out-parameter, and the trace should not pretend otherwise.** coreMQTT's size calculators write both out-parameters BEFORE their final limit check, so a failed call leaves them updated and a caller who ignored the status would serialize a length the library had just refused. There is no error-path value in a `Result`, so the driver prints values only on success and the divergence is written down. Fourth member of the family: **a differential bounds what the C ANSWERS, never what it TOUCHES, and never what it LEAVES BEHIND.** |
| 2026-09-16 | **A state machine's differential compares its CALLBACKS, not its return values.** `rusty_rtos_sntp`'s client is five callbacks deep; a transcription that returned the right status while reading the clock a different number of times would be a different library. Every callback is scripted and logged on both sides, and the context state is printed after every action. `rusty_rtos_mqtt` and `rusty_rtos_http` have exactly this shape and do the same. |
| 2026-09-16 | **A liveness bound must be enforced from INSIDE the mock.** The client gate's first version asserted a call count after the API returned, which cannot fire when the call never returns — it hung. Moving the bound into the mock host's own callback turns a spin into a named failure with a count. That is also how the second coreSNTP defect was found, so the rule paid for itself immediately. |
| 2026-09-16 | **A differential's arm never "fixes" its oracle.** Two defects in coreSNTP are reproduced exactly, `wrapping_sub` and all, because an arm that corrects the C is measuring two different libraries. The correction is a decision-log row if the fix lands upstream, never a silent edit — and a poison proves the differential is sensitive to the line, which is what makes the faithfulness checkable rather than asserted. |
| 2026-09-16 | **Re-measure a blocking row before you build around it.** §5.4 said `rusty_time-core` was not `no_std`, measured 2026-09-09, and that until a leaf landed `rusty_rtos_sntp` was a coreSNTP remake. 0.2.0 landed on 2026-09-10 and the row was never updated — the fifth stale row found in this plan, and the fifth that claimed work undone that was in fact done. The decision came out the same way in the end, but for a reason that had to be measured rather than inherited. |
| 2026-09-16 | **A differential's trace should carry its own inputs.** `backoff` kept the case table in both arms; two tables drift, and nothing catches two arms agreeing perfectly about a workload that is not the one the driver documents. `sntp`'s trace holds an `in` line before every answer, packet bytes included, and the Rust arm replays it. Every remaining K7 package does this. |
| 2026-09-16 | **`kairos new`'s template was emitting a `git =` dependency on `rusty_rtos_core`**, which `cargo publish` refuses AND which silently defeats the umbrella's `[patch.crates-io]` rows — a patch aimed at crates-io cannot reach a dependency resolved from a git URL, so a newly scaffolded package builds against the PUBLISHED core while every sibling builds against the local one. The v0.1.0 release pass fixed the existing packages and missed the template, so the defect was waiting for the next `kairos new`. Fixed in the template and in `rusty_rtos_sntp`. |
| 2026-09-16 | **A guard that has never fired is not a guard.** `rusty_rtos_json`'s query driver caps reported values at 32 per document so one corpus file cannot dominate the trace — and no document had ever produced more than five, so the cap was dead code in BOTH arms. That is the thing that would hide the two arms disagreeing about how many values they had produced. The rule generalises to every K7 driver: if the harness has a limit, the workload must reach it. |
| 2026-09-16 | **Some components can HANG rather than fail, and those need a liveness assert, not a panic gate.** `JSON_Iterate` carries a caller-owned cursor; one that stops advancing does not panic and does not answer wrongly, it loops, and a test runner reports that as "still running". Every K7 package with a caller-driven cursor — mqtt's and http's incremental parsers both will — needs a test that the cursor strictly advances, not only that it cannot panic. |
| 2026-09-16 | **Three poisons in `rusty_rtos_json` did not fire, and all three meant the same thing.** The universal byte reader, and two narrowed sub-slices, can all be made to panic on an out-of-range access and no test notices — because nothing reaches out of range. Those bounds are defence in depth rather than load-bearing, which is worth KNOWING and worth keeping. A poison that does not fire is a measurement; deleting the test would throw the measurement away. |
| 2026-09-16 | **A crates.io description is a capability claim.** `rusty_rtos_json`'s said "the query-by-path search", which is not written; corrected rather than left aspirational. The README discipline covers every published string, not just the README. |
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
