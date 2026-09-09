# Kairos umbrella — the ledger

Every number the umbrella claims, with the run that produced it. Package
numbers live in each package's own `docs/LEDGER.md`; this file carries the
family-level facts: the oracle, the fleet gate, the conformance corpus.

Method discipline (mission plan §2.7): an external oracle before a
self-metric; counters before clocks; the method line names the machine, the
pinning and the arm order for anything timed. Nothing here is timed yet.

## Conformance — the Rust kernel against the C kernel (2026-09-09, K1)

The headline: **`rusty_rtos_kernel` reproduces the C kernel's trace exactly**
for the `dynamic` scenario, for 100,000 ticks, including the counters.

| fact | value | method |
|---|---|---|
| `dynamic`, 100,000 ticks | **1,219,231 lines identical**, the verdict line included | `kairos conform dynamic --ticks 100000`: runs `rusty_rtos_demo`'s sim (release) and the instrumented C oracle, compares line for line, and fails at the first difference. Not a sample and not a summary — every line |
| counters at 100,000 ticks | ticks 100000, yields 179588, exits 1066689, lines 1219230 — identical on both sides | the harness's `KAIROS_RESULT` line, from the patched port's counters on the C side and `SimPort`'s on ours |
| `dynamic`, 2000 ticks | 24,403 lines identical | `kairos conform dynamic` (the default length); the same run stored as `oracle/traces/dynamic.trace.zst` |
| counters at 2000 ticks | ticks 2000, yields 3589, exits 21346, lines 24402 | as above; pinned as a test in `rusty_rtos_demo` (`tests/conformance.rs`) with an FNV-1a/64 digest of the trace, so drift fails without a C toolchain |
| corpus covered | 1 scenario of 9 (`dynamic`) | the other eight are written as they are ported; the gate is the same command |

**What "identical" means here.** The trace is one line per scheduling
decision — task create, switch in and out, tick, delay, suspend, resume,
priority set, queue send and receive, and the list moves each causes. Both
kernels emit it from their own `trace*` seam, and the diff includes the
tick each line carries. Sim time on both sides is a count of outermost
critical-section exits (`ORACLES.md`, sim contract v1), so agreeing on the
trace means agreeing on *when* as well as *what*.

**What it does not mean.** Nothing here has run on a chip, nothing is
timed, and the Rust kernel does not switch stacks — on the sim a task is a
resumable state machine and the runner drives it, which is what lets the
kernel be `forbid(unsafe)`. The arena-and-list cost row K1 also asks for is
not taken yet; the port's context switch is K3's.

### Three things the C port does that a stackless kernel has to say out loud

Each cost a diff, and each is now a named mechanism rather than a fudge:

1. **A task's first switch-in resets the critical nesting count.**
   `prvWaitForStart` in the Posix port is one line: `uxCriticalNesting = 0`.
   A real port hands a new task a fresh stack, so the outgoing task's open
   sections are not on it.
2. **Nesting is per task, saved and restored across a switch**
   (`uxSavedCriticalNesting` in `prvSwitchThread`), so the exits that unwind
   an abandoned frame are *deferred* to when that task runs again, not lost
   and not taken early. The kernel records them (`owed_exits`) and pays them
   in `resume_pending`.
3. **The tick handler's nesting bump is not a critical section.** It is a
   raw `uxCriticalNesting++`/`--`, never `vPortExitCritical`, so it must not
   create an owed exit. Modelling it as one invented a tick at every switch
   the tick itself caused.

The debugging tool that found all three: `KAIROS_TRACE_EXITS=1` on both
sides adds a ` #<exits>` column to every trace line, and `kairos conform
--exits` compares them. The events agreed for 1,598 lines after the
accounting had already drifted, so the column is the diagnosis and the event
is only the symptom.

## The oracle (2026-09-09)

| fact | value | method |
|---|---|---|
| C oracle builds on this machine | yes | `kairos oracle build dynamic`: `cc` (gcc 15.2) under WSL2 Ubuntu on the Windows 11 host, `-O0 -g -Wall`, no CMake; sources = 6 kernel files + Posix `port.c` (patched, 6 exact-anchor edits) + `wait_for_event.c` + `heap_3.c` + `oracle/harness/*.c` + `Demo/Common/Minimal/dynamic.c`; FreeRTOS-Kernel `V11.3.1` @ `3a22924e`, classic repo @ `f4fcc3b2` (`ORACLES.md`) |
| `dynamic` trace is reproducible | byte-identical across two consecutive runs | `kairos oracle trace dynamic --ticks 2000`: the verb runs the binary twice and refuses a differing pair; stored as `oracle/traces/dynamic.trace.zst` (rusty_zstd 0.2.3, level 19: 757,457 → 50,346 bytes, decompressed and compared before it is kept; `kairos oracle cat dynamic` prints it) |
| `dynamic` trace, 2000 ticks | 24,403 lines (24,402 events + the `KAIROS_RESULT` line) | sim contract v1 (`ORACLES.md`): idle-hook tick + a tick every 16th outermost critical-section exit; the demo's own `xAreDynamicPriorityTasksStillRunning()` returned pass |
| counters at the end of the run | ticks 2000, yields 3589, critical exits 21,346 | the `KAIROS_RESULT` line the harness prints from the patched port's counters |
| event mix (from `kairos oracle cat dynamic`) | TASK_SWITCHED_IN 4703, TASK_SWITCHED_OUT 4702, MOVED_TASK_TO_READY_STATE 4032, TASK_PRIORITY_SET 3952, QUEUE_RECEIVE_FAILED 3933, TASK_INCREMENT_TICK 2887, TASK_DELAY 52, MOVED_TASK_TO_DELAYED_LIST 52, TASK_SUSPEND 23, TASK_RESUME 22, QUEUE_SEND 16, QUEUE_RECEIVE 16, TASK_CREATE 8, QUEUE_CREATE 2, TASK_DELAY_UNTIL 1, STARTING_SCHEDULER 1 | `awk '{print $2}' oracle/traces/dynamic.trace \| sort \| uniq -c` |
| TASK_INCREMENT_TICK lines vs ticks | 2887 lines for 2000 ticks | ticks delivered while the scheduler is suspended are pended and replayed by `xTaskResumeAll`, which fires the macro again; the trace records the kernel's view, and so must ours |

The first run of the unpatched contract (a tick on every 16th *yield*) never
finished: `dynamic`'s SUSP_RX task polls a queue without blocking and never
yields, so no tick could arrive and the trace grew past 8 GB before it was
killed. That is why v1 counts critical-section exits (decision log, mission
plan §8), and why the `trace` verb now runs under `timeout 300` and
`ulimit -f 1048576`.

## The fleet gate (2026-09-09)

| fact | value | method |
|---|---|---|
| `rusty_rtos_core` passes the fleet gate | yes | `kairos check rusty_rtos_core --fmt --clippy --test --deny`: fmt, clippy `-D warnings` under the workspace lint policy, 34 host tests, `cargo deny check` (advisories, bans, licenses, sources all ok), then `cargo check` on `thumbv7em-none-eabihf`, `thumbv8m.main-none-eabihf`, `riscv32imac-unknown-none-elf`, `riscv32imafc-unknown-none-elf` with and without `alloc` (8 rungs) |
| `rusty_rtos_kernel`, `_port`, `_demo` pass the fleet gate | yes | the same command; each also checked on the four bare-metal targets, `no_std` and `no_std + alloc` |
| Miri | green on `rusty_rtos_kernel` and `rusty_rtos_port` unit tests | `cargo +nightly miri test --lib`, miri 0.1.0 of 2026-09-08 |
| `cargo audit` on `rusty_rtos_core` | 0 advisories over 17 locked crates | advisory-db of 2026-09-09 (1243 advisories) |
