# Kairos umbrella — the ledger

Every number the umbrella claims, with the run that produced it. Package
numbers live in each package's own `docs/LEDGER.md`; this file carries the
family-level facts: the oracle, the fleet gate, the conformance corpus.

Method discipline (mission plan §2.7): an external oracle before a
self-metric; counters before clocks; the method line names the machine, the
pinning and the arm order for anything timed. Nothing here is timed yet.

## Conformance — the Rust kernel against the C kernel (2026-09-09, K1)

The headline: **`rusty_rtos_kernel` reproduces the C kernel's trace exactly**,
for all nine scenarios of the K1 corpus, for 100,000 ticks each, counters
included. 8,408,764 lines of agreement, and not a sample among them.

| scenario | lines identical at 100,000 ticks | ticks | yields | exits |
|---|---|---|---|---|
| `dynamic` | 1,219,231 | 100000 | 179588 | 1066689 |
| `PollQ` | 117,417 | 100051 | 2056 | 105608 |
| `BlockQ` | 1,344,460 | 100011 | 195466 | 1282272 |
| `semtest` | 1,503,634 | 100000 | 64837 | 1163649 |
| `countsem` | 966,326 | 100000 | 21001 | 1280002 |
| `recmutex` | 1,385,862 | 100000 | 40923 | 1068657 |
| `blocktim` | 131,384 | 100062 | 4505 | 114313 |
| `QPeek` | 486,871 | 100000 | 88964 | 414030 |
| `GenQTest` | 1,253,579 | 100000 | 150435 | 1299809 |

| fact | value | method |
|---|---|---|
| the gate | `kairos conform --all --ticks 100000` | runs `rusty_rtos_demo`'s sim (release) and the instrumented C oracle, compares line for line, and fails at the first difference. Not a sample and not a summary — every line, the verdict line included |
| counters | ticks, yields, exits and line counts identical on both sides, every scenario | the harness's `KAIROS_RESULT` line, from the patched port's counters on the C side and `SimPort`'s on ours |
| the same nine at 2000 ticks | identical, and stored | `kairos oracle trace --all`: each scenario run twice on the C kernel, refused unless byte-identical, kept as `oracle/traces/<scenario>.trace.zst` (rusty_zstd level 19, round trip verified) |
| offline regression | all nine pinned by counters, line count, byte count and an FNV-1a/64 digest **of the C kernel's own trace file** | `rusty_rtos_demo`'s `tests/conformance.rs`; it fails on drift without a C toolchain, which is what CI has. Poisoning one pinned number fails the test, so the gate is not vacuous |
| Miri over the corpus | green: all nine scenarios, twice each, at 20 ticks | `cargo +nightly miri test` in `rusty_rtos_demo`, 13 minutes; this is what puts the arenas and the lists through an interpreter that checks them, rather than 11 unit tests |

**What "identical" means here.** The trace is one line per scheduling
decision — task create, switch in and out, tick, delay, suspend, resume,
priority set, queue send and receive, and the list moves each causes. Both
kernels emit it from their own `trace*` seam, and the diff includes the
tick each line carries. Sim time on both sides is a count of outermost
critical-section exits (`ORACLES.md`, sim contract v1), so agreeing on the
trace means agreeing on *when* as well as *what*.

**What it does not mean.** Nothing here has run on a chip and nothing is
timed. The Rust kernel does not switch stacks — on the sim a task is a
resumable state machine and the runner drives it, which is what lets the
kernel be `forbid(unsafe)`. The port's context switch is K3's.

### Five things the C port does that a stackless kernel has to say out loud

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
4. **A thread stops at the switch; a stackless call does not.** The frame
   the scheduler switched away from is still on the machine and runs to its
   end — closing the sections it had open *and*, on some paths, opening one
   more. `xQueueReceive`'s timeout path calls `prvIsQueueEmpty` after the
   `xTaskResumeAll` that may have switched the caller out, and on the C side
   that section is charged to the task when it next runs. So the port stops
   counting the tail's exits as sim time and *tallies* them
   (`Port::begin_unwind` / `end_unwind`), and the kernel replays the tally
   on resume. Counting rather than discarding is the whole point: an earlier
   version discarded exactly the sections that were open, which was right
   until a tail opened one of its own. It cost `blocktim` a divergence that
   the totals hid completely — ticks, yields, exits and line count all
   agreed at 59,000 ticks while one exit sat 1,377 lines too early.
5. **The heap costs time.** Every `heap_N.c` wraps its `malloc` in
   `vTaskSuspendAll()` / `xTaskResumeAll()`, and `xTaskResumeAll` is a
   critical section. So on the C side creating a queue *after* the scheduler
   has started costs one more outermost exit than creating it before. This
   kernel allocates nothing, so it spends the exit deliberately, under
   `Config::DYNAMIC_ALLOCATION` — true for the Posix demo configuration,
   false for silicon, where the exit is not there to spend. `GenQTest`
   creates a mutex from a running task, and that is where it showed up.

The debugging tool that found all five: `KAIROS_TRACE_EXITS=1` on both
sides adds a ` #<exits>` column to every trace line, and `kairos conform
--exits` compares them. The events agreed for 1,598 lines after the
accounting had already drifted, so the column is the diagnosis and the event
is only the symptom.

## K2 — the IPC, in progress (2026-09-09)

The conformance corpus is **thirteen scenarios**, every one of them
trace-identical to the C kernel for 100,000 ticks: **11,328,945 lines**,
counters included. Four are new since K1, and with them the interrupt half
of the kernel.

Counted against K2's own list of eighteen (mission plan section 6), that is
**eleven passed** — the other two of the thirteen, `dynamic` and
`blocktim`, belong to K1's list. One of the eighteen, `IntQueue`, is out of
scope for this port. **Six remain:** `QueueSet`, `TimerDemo`,
`EventGroupsDemo`, `StreamBufferDemo`, `MessageBufferDemo` and
`MessageBufferAMP`.

| scenario | lines identical at 100,000 ticks | what it is for |
|---|---|---|
| `QueueOverwrite` | 1,300,382 | a queue of one, written from a task and from the tick |
| `QueueSetPolling` | 1,373,606 | a queue set polled with no block time, written by an interrupt |
| `IntSemTest` | 132,893 | a counting semaphore filled from an interrupt; a *mutex* given from one |
| `StreamBufferInterrupt` | 113,300 | a string streamed from the tick, read one byte at a time |

The nine from K1 are unchanged, which is the other half of the result:
turning the tick hook on and adding the whole `FromISR` surface moved no
existing trace by a line.

| fact | value | method |
|---|---|---|
| the gate | `kairos conform --all --ticks 100000` | as K1's, now over thirteen scenarios |
| offline regression | all thirteen pinned by counters, line count, byte count and an FNV-1a/64 digest of the C kernel's own trace file | `rusty_rtos_demo`'s `tests/conformance.rs` |
| still to do | `QueueSet`, `TimerDemo`, `EventGroupsDemo`, `StreamBufferDemo`, `MessageBufferDemo`, `MessageBufferAMP` | `QueueSet` and the two buffer demos need no new kernel — the queue-set and stream-buffer subsystems are in. `TimerDemo` needs software timers and the daemon; `EventGroupsDemo` needs event groups *and* the timers, because `xEventGroupSetBitsFromISR` defers to the daemon's pended function call |
| `QueueSet` cannot be made trace-identical as it stands | needs a decision | it chooses which of its three queues to write with a PRNG that `prvQueueSetSendingTask` seeds from **the address of one of its own stack locals** (`prvSRand( ( size_t ) &ulTaskTxValue )`). That is reproducible on the C side — two oracle runs are byte-identical — and unknowable to any second implementation, and the seed decides every write for the rest of the run. Either the sim contract fixes the seed on both sides, the way it already fixes the tick, or `QueueSet` joins `IntQueue` as out of scope. `QueueSetPolling` already covers the queue-set API; what `QueueSet` adds is contention between three of them and the overwrite-into-a-set corner |
| `MessageBufferAMP` needs its own oracle binary | not built yet | it works by overriding `sbSEND_COMPLETED` in `FreeRTOSConfig.h`, which is a global macro: with it defined, every other stream-buffer scenario changes behaviour, and `vGenerateCoreBInterrupt` would dereference a control buffer that only exists once the AMP demo has started. One binary cannot serve both, so the oracle build needs a per-scenario config |
| K2's other three gates | Kani harnesses for the CBMC proof list, the no-panic property test, a `cargo mutants` score | not started |

**`IntQueue` is not in this corpus and will not be.** Upstream's own Posix
demo does not build it either: it needs a port with nested interrupts of
different priorities, which a signal-driven host port does not have.
`IntSemTest` covers the from-ISR surface that a single-priority interrupt
can reach. That is a scope row, not a pass.

### What the interrupt half cost

Four mechanisms, each found by a diff:

1. **The tick hook has to be a seam the kernel can hand itself to.** Six K2
   scenarios have an interrupt half, and every one of them calls back into
   the kernel. `Hooks::tick` takes `&self` and cannot; `TickHook<K>` takes
   the kernel. It is `Copy` and taken by value because the kernel owns it —
   copy out, run, store back — which is how it borrows the kernel mutably
   without borrowing itself twice.
2. **A `FromISR` call costs nothing on this port.**
   `portSET_INTERRUPT_MASK_FROM_ISR` is an empty function on the Posix
   port: signals are already blocked in a handler. So the from-ISR half
   touches neither the nesting count nor the exit count, and a separate
   `Port` seam says so rather than reusing `enter_critical`.
3. **An interrupt that discards the woken flag still gets the switch.**
   `xTaskGenericNotifyFromISR` sets `xYieldPendings[0]` as well as the
   caller's flag, and `StreamBufferInterrupt` passes NULL for the flag — so
   the switch comes from the pended yield, on the way out of
   `xTaskIncrementTick`. That is why the tick hook runs *before* the
   yield-pending test, and it cost 217 lines of trace to find out.
4. **A preempted call resumes after the exit that preempted it, not
   before.** A stream buffer samples its counters inside a critical
   section; if that section's exit releases a tick that switches the task
   away, the C's thread stops *inside* the exit. Ours returns and is called
   again — and must not re-enter the section, because the C already paid
   for it. The sample the frame took is kept in the TCB, like the queue
   calls' `WaitFrame`, because the frame it belonged to is gone.

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
| Miri | green on `rusty_rtos_core`, `_kernel` and `_port` unit tests, and on all nine corpus scenarios | `cargo +nightly miri test --workspace` in each; the corpus run is `rusty_rtos_demo`'s determinism test, which drops to 20 ticks under `cfg!(miri)` so the interpreter can finish (13 minutes) |
| `cargo audit` on `rusty_rtos_core` | 0 advisories over 17 locked crates | advisory-db of 2026-09-09 (1243 advisories) |

## The arena-and-list cost row (2026-09-09, K1)

Mission plan §2.5 chose index-linked lists over pointer-linked ones so the
core could be `forbid(unsafe)`, and wrote down the price of being wrong: a
K1 ledger row above **1.25×** the C list cost reopens the decision. Here is
the row. **It is 2.08×, and the revisit condition has fired.**

| arm | instructions per list operation | vs C |
|---|---|---|
| `FreeRTOS-Kernel/list.c`, gcc 15.2 `-O2` | **22.32** | 1.00× |
| the same, gcc 15.2 `-O3` | 21.82 | 0.98× |
| `rusty_rtos_core::list`, rustc 1.98 release (opt-level 3, LTO, one codegen unit) | **46.45** | **2.08×** |

**Method** — `bench/list-cost/run.sh`, one command, on WSL2 Ubuntu on this
Windows 11 host. It is not timed: instructions are counted with callgrind,
which is deterministic, so there is no noise floor to argue about and no
reason to interleave the arms.

- **Work-count parity, enforced.** Both arms run the same 40 operations per
  round on 8 items — 8 appends to a ready list, 8 round-robin picks, 8
  removals, 8 ordered inserts into a delayed list, 8 more removals — which
  is the mix `prvAddTaskToReadyList`, `taskSELECT_HIGHEST_PRIORITY_TASK` and
  `prvAddCurrentTaskToDelayedList` actually make. Both sort the same
  pseudo-random values from the same xorshift, spelled the same way.
- **A correctness gate.** Each arm prints a checksum of every value its list
  operations returned. They match (`7de9075f4deb23e5`), and the script stops
  if they ever do not: two instruction counts of two different programs are
  not a comparison.
- **The cost is a slope, not a count.** Each arm is counted at 100k, 200k
  and 300k rounds and the cost of a round is the difference, so process
  start-up, the dynamic loader, libc and the final `printf` cancel exactly
  instead of being estimated away. The second difference is printed as the
  linearity check: −8,756 on 89M for C and −32,213 on 186M for Rust, that
  is 0.01% and 0.017%, so the counts are affine in the round count and the
  slope is a slope.
- The C arm compiles `FreeRTOS-Kernel/list.c` unmodified, from the pinned
  V11.3.1 checkout.

**Where the 24 extra instructions go.** Not to the index arithmetic. Every
link the C follows with one dereference costs us a branch (is this link an
item or a list's end marker?) plus a bounds check plus the load, and there
are about five link accesses in each of `insert_end`, `remove` and
`insert` — several of them re-validating a handle the same call already
validated. Two changes are visible from here, neither of which touches the
decision itself: put the end markers in the same array as the items so the
branch disappears, and validate a handle once per call rather than once per
access.

**What this row does and does not say.** It is a static count of one data
structure on x86-64, not a scheduler benchmark and not a number from a chip:
it says nothing about how often these operations run, about cache behaviour,
or about what a Cortex-M4 makes of the same code. The scheduler-level
figure — what a context switch costs end to end — is K3's, with the first
silicon and an A/B against the C kernel on the same board. **The decision is
the owner's:** the plan's condition has fired, so §2.5's "handles are
indices, never pointers" gets a decision-log row, either reaffirming it with
this price attached or funding the two changes above and re-running this
same script.
