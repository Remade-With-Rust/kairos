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

The conformance corpus is **sixteen scenarios**, every one of them
trace-identical to the C kernel for 100,000 ticks: **12,808,722 lines**,
counters included. Seven are new since K1, and with them the interrupt half
of the kernel, the software timers, the event groups and the send-completed
seam.

Counted against K2's own list of eighteen (mission plan section 6), that is
**fourteen passed** — the other two of the sixteen, `dynamic` and
`blocktim`, belong to K1's list. One of the eighteen, `IntQueue`, is out of
scope for this port. **Three remain**, and every one of them is blocked
above the kernel rather than by it: `QueueSet`, `StreamBufferDemo` and
`MessageBufferDemo`.

| scenario | lines identical at 100,000 ticks | what it is for |
|---|---|---|
| `QueueOverwrite` | 1,300,381 | a queue of one, written from a task and from the tick |
| `QueueSetPolling` | 1,373,605 | a queue set polled with no block time, written by an interrupt |
| `IntSemTest` | 132,892 | a counting semaphore filled from an interrupt; a *mutex* given from one |
| `StreamBufferInterrupt` | 113,299 | a string streamed from the tick, read one byte at a time |
| `TimerDemo` | 156,491 | twenty-one software timers, the daemon task, and four callbacks — including two an interrupt starts and stops |
| `EventGroupsDemo` | 1,202,786 | four tasks on one event group: selective bits, bit combinations and a four-way rendezvous, plus an interrupt setting bits through the daemon |
| `MessageBufferAMP` | 120,513 | two "cores" through three message buffers, with `sbSEND_COMPLETED` replaced so a send wakes its reader the long way round |

The nine from K1 are unchanged, which is the other half of the result:
turning the tick hook on and adding the whole `FromISR` surface, the timers
and the event groups moved no existing trace by a line.

| fact | value | method |
|---|---|---|
| the gate | `kairos conform --all --ticks 100000` | as K1's, now over sixteen scenarios, from two oracle binaries |
| offline regression | all sixteen pinned by counters, line count, byte count and an FNV-1a/64 digest of the C kernel's own trace file | `rusty_rtos_demo`'s `tests/conformance.rs` |
| still to do | `QueueSet`, `StreamBufferDemo`, `MessageBufferDemo` | none of the three needs new kernel: every subsystem they use is in and proved by another scenario. The rows below say what each is actually blocked on |
| **`MessageBufferDemo` and `StreamBufferDemo` cannot run under sim contract v1** | measured 2026-09-09 | both create a non-blocking sender and receiver at the idle priority, and `xStreamBufferSend` with a zero block time takes **no critical section at all** when the buffer is full: no exit, so no tick, so no time slice, so the sender spins for ever and the receiver never runs. The C oracle hangs on `MessageBufferDemo` at one tick and prints nothing — `timeout 20 ./oracle/build/corpus MessageBufferDemo 1` exits 124 with an empty trace. This is the failure mode `ORACLES.md` already records for `flop` and `integer`: **a task that polls without entering a critical section stops the clock.** Both are contract-v2 scenarios, and the contract is the thing that has to change, not the kernel |
| the two buffer demos also need `vStreamBufferDelete` with reclamation | **built** | both `MessageBufferDemo` and `StreamBufferDemo` create a buffer and delete it again **on every loop of their echo server** — the C leans on `pvPortMalloc` / `vPortFree` handing back the same block each time. This kernel's byte arena is a bump allocator with no free, so it would run out in a few hundred ticks. It needs a free list over `BYTES` (a contained piece of work, and the only place in the kernel that will have one) before either scenario can run |
| `QueueSet` cannot be made trace-identical as it stands | needs a decision | it chooses which of its three queues to write with a PRNG that `prvQueueSetSendingTask` seeds from **the address of one of its own stack locals** (`prvSRand( ( size_t ) &ulTaskTxValue )`). That is reproducible on the C side — two oracle runs are byte-identical — and unknowable to any second implementation, and the seed decides every write for the rest of the run. Either the sim contract fixes the seed on both sides, the way it already fixes the tick, or `QueueSet` joins `IntQueue` as out of scope. `QueueSetPolling` already covers the queue-set API; what `QueueSet` adds is contention between three of them and the overwrite-into-a-set corner |
| `MessageBufferAMP` needed its own oracle binary | **built, and the scenario passes** | it works by overriding `sbSEND_COMPLETED` in `FreeRTOSConfig.h`, which is a global macro: with it defined every other stream-buffer scenario changes behaviour, and `vGenerateCoreBInterrupt` would reach a control buffer that only exists once the AMP demo has started. `kairos oracle build` now emits two binaries from the same sources, differing in one `-DKAIROS_AMP=1`, and each scenario says which it comes out of |
| **a hook that makes several kernel calls needs a program counter, exactly as a task body does** | found by `MessageBufferAMP`, 2026-09-09 | the replaced `sbSEND_COMPLETED` makes three calls in a row — post the handle to a control buffer, read it back, notify — and the C can afford to because it has a stack: a tick that lands inside one of them parks the thread and the rest runs when the task has the CPU back. Ours cannot park; the call returns and the rest ran under the *next* task's name, one exit early. It was trace-identical for 3,063 lines and then diverged at the first tick that landed inside the handler. The fix is the same shape as a task body's: the handler asks after each call whether the current task changed, and what is left is owed and paid on the first step after the sending task is switched back in. This is the general answer for any `TickHook` method that makes more than one kernel call |
| K2's no-panic gate | **passed** | `rusty_rtos_kernel`'s `tests/no_panic.rs`: 64 independent kernels, 4,000 arbitrary calls each — a quarter of a million over the whole public surface, with handles from other arenas, handles from nowhere, stale handles, indices past the configured end, tick counts at both extremes and lengths larger than the arenas. The generator is a seeded xorshift rather than a property-testing crate, so it needs no dependency and a failure reproduces from the seed it names. It also asserts the calls *landed*: the run must trace more than 100,000 lines, and it traces 131,802. Raising that floor fails the test, so the gate is not vacuous |
| K2's Kani gate | **22 of 32 harnesses verify — 32,145 checks, 0 failures** | `rusty_rtos_kernel/proofs.sh` in WSL (Kani 0.67.0 has no Windows build), harnesses in `src/proofs.rs` named after the `FreeRTOS/Test/CBMC/proofs/{Queue,Task}` directories. Swept one at a time with a 150 s budget each, because one blow-up in a combined run says nothing about the others; most finish in under ten seconds. **`--exact` is not optional** and this row was first taken without it: Kani filters harnesses by substring, so `--harness queue_generic_send` also runs `queue_generic_send_from_isr` and `queue_generic_send_stale_handle` — three harnesses inside one budget, reported as a timeout on the first. The ten that do not converge split cleanly in two, and both halves are measurements rather than guesses — see the two rows below |
| the six that need a *started* kernel | `start_scheduler` is the wall | `task_get_scheduler_state`, `task_get_current_task_handle`, `task_switch_context`, `task_start_scheduler`, `task_increment_tick`, `task_delay`. Every ingredient of the setup is cheap on its own — `Kernel::new` 0.8 s, one task 3.0 s, three tasks 10.2 s, a task and a queue 3.2 s — but `start_scheduler` with **no user task at all** does not finish in 500 s, and what it adds over the cheap cases is the first switch. Every other harness runs on a kernel that has its tasks but has not started, which is why they finish: the call bodies under proof are the same either side of `vTaskStartScheduler`, and what is given up is the block-and-switch tail of a blocking call |
| the four that pass a symbolic handle into a kernel call | the space is not the cost | `queue_generic_send_stale_handle`, `task_priority_set_stale_handle`, `task_priority_set`, `queue_take_and_give_mutex_recursive`. The obvious economy was tried and refuted: bounding the symbolic handle's index and generation to the handful of values next to the arena — which is where a real disagreement lives, and what the C proofs do with their pointers — changed nothing, all four still timed out, one of them at 700 s. So it is not the size of the space; it is that a call which walks the arena and the lists with a symbolic handle has to be explored for every slot that handle could name |
| K2's mutants gate | **36 of 42 viable mutants caught (86%) with the corpus as the oracle** | `cargo mutants --in-place --file crates/rusty_rtos_kernel-core/src/kernel.rs --test-package rusty_rtos_demo-core --shard 1/8 --timeout 240 -- --manifest-path ../rusty_rtos_demo/Cargo.toml --release`, run from the kernel repo with the demo's `[patch]` table in scope so the mutated kernel is the one the corpus runs. 44 of the shard's 47 mutants completed before the tool wedged: 36 caught, 6 missed, 2 unviable. The mechanism was checked by hand first — a mutation planted in `queue_send_list` fails `every_scenario_reproduces_the_c_kernels_trace_and_counters` in 79 s — because a mutants run that silently tests the wrong binary reports the same shape as one that tests nothing |
| the six that survived are one finding and one equivalent mutant | measured 2026-09-09 | five of the six are the same line: `if self.running && C::USE_PREEMPTION && self.current_priority() < priority` in `prvAddNewTaskToReadyList`, the yield a task creation owes when the new task outranks the running one. **No scenario in the corpus creates a task after the scheduler has started** — every one of the fifteen creates all of its tasks in its `start`, and `Runner::start_common` creates `CHECK` and then starts the scheduler — so the arm is never reached and any mutation of it survives. The corpus's own answer to that was taken to be `death.c`, the standard demo whose whole subject is creating and deleting tasks at runtime. **`death.c` landed on 2026-09-10 and this row is only partly discharged: it kills two of the six mutations of that line and four survive**, because `vCreateTasks` creates its tasks at its OWN priority, so the arm is reached but never true. Closing this needs a scenario that creates a task that OUTRANKS the running one — see the K5a section. Reachable is not killed. The sixth, `>` to `>=` in `taskRECORD_READY_PRIORITY`, assigns the same value either way and is an equivalent mutant, not a gap |
| what the kernel's *own* test suite pins | **13 of 342 (4%)** | the same file, mutated the same way, judged by `rusty_rtos_kernel`'s own `cargo test`: 372 mutants in 4 minutes, 13 caught, 328 missed, 30 unviable, 1 timeout. That is not a failure of the suite — `tests/no_panic.rs` was built to prove no call panics, and it proves exactly that and nothing about behaviour. It is the number that says why the corpus has to be the oracle, and why the two repositories being separate cargo workspaces is worth the trouble it costs to bridge |

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

### What the timers and the event groups cost

Five more mechanisms, each one found the same way:

5. **A refused send may not consume the ring slot its message went in.**
   The timer command queue carries *indices* into a ring of messages, so
   the ring has to be told what the queue decided. Staging the message
   before the send let a send the queue refused overwrite a message whose
   index was still queued — and `TimerDemo`'s first test is exactly that: it
   starts as many timers as the queue holds and then requires the next to
   fail. The staged write is now conditional on the queue having room, and
   only an accepted send advances the pointer.
6. **A trace macro on the line *after* a kernel call belongs to the
   resumed frame.** `traceTIMER_COMMAND_SEND` follows `xQueueSendToBack`,
   and `traceEVENT_GROUP_WAIT_BITS_END` follows `xTaskResumeAll`. When
   either call hands the CPU to a higher-priority task, the C's line does
   not run until the sender has it back. `OwedTrace` — built in K2 for the
   two queue-failure lines — now carries both.
7. **A hook that runs on the daemon task may not be handed a copy of
   itself.** `TickHook::tick` runs in interrupt context, where nothing it
   calls can produce another tick, so a copy is safe. A *timer callback*
   runs on the daemon task, and `pvTimerGetTimerID` on its first line takes
   a critical section whose exit can be the one that produces a tick — so
   the tick hook runs inside it and writes its own state back, and the
   outer copy then goes over the top. It cost `vTimerPeriodicISRTests` one
   `uxTick++` and made it fire the same arm twice, a tick apart.
   `TickHook::timer` and `TickHook::pended` now take the kernel and reach
   the state through it, one short read-modify-write at a time, the way the
   C reaches its `static`s.
8. **`vPortFree` costs an exit too.** Fact 5 above says `pvPortMalloc` does;
   the same wrapper is around the free, and `vEventGroupDelete` frees. The
   demo deletes and remakes its event group every cycle, so the missing exit
   showed up on the second `EVENT_GROUP_CREATE`.
9. **`xEventGroupSync` has its own two trace points, and the harness hooks
   neither.** It is `xEventGroupSetBits` and `xEventGroupWaitBits` in one,
   but it traces `traceEVENT_GROUP_SYNC_BLOCK` / `_SYNC_END`, not the
   wait-bits pair. The only line a sync leaves in the trace is the set-bits
   one from inside it.

## K2.1 — the Rust face, in progress (2026-09-10)

The C-shaped API is what the corpus proves; it is not what a Rust
application should have to hold. K2.1 puts a safe, ownership-typed face on
the *same kernel*, so that both faces are gated by the same 12.8 million
lines. The first primitive is done and the headline is the cost.

| fact | value | method |
|---|---|---|
| **the Rust face costs nothing** | **zero extra critical-section exits** | `PollQ-typed` is `PollQ` written against `Queue<u16, 10>` — a queue that *moves* `u16`s — and it is diffed against **`PollQ`'s own C oracle trace**, not a new one. 117,417 lines identical at 100,000 ticks, `ticks=100051 yields=2056 exits=105608`, every number `PollQ`'s to the digit. `kairos conform PollQ-typed` strips the `-typed` suffix to choose the oracle, so there is no second C arm to disagree with |
| the same claim, without a C toolchain | the two pinned rows carry the **same digest** | `tests/conformance.rs` pins `PollQ-typed` at `0xf50b_bbbd_22ec_16d1`, 68,195 bytes — character for character `PollQ`'s own pin. The verdict line is the one thing allowed to differ, because it names the scenario; its counters are compared like every other line, so a face that cost one exit would still be caught |
| how it can be free | the kernel's queue carries a **slot index**, not the item | the pattern the timer command queue already uses. A typed send is one `queue_send` and a typed receive is one `queue_receive` — the same calls in the same order. The only extra work is a non-counting room check (`Raw::raw_queue_has_room`), which is the kernel reading its own queue rather than a task asking it a question, so it takes no critical section |
| the other two primitives, the same claim | **exactly the calls the C would make, and no others** | `PollQ-typed` can only prove what a corpus scenario exercises, which is the queue. Six unit tests in `typed.rs` count the raw calls for the rest: a typed send is one `queue_send` and asks nothing else; a *refused* send still makes the call, because the C does and the trace would otherwise lose a `QUEUE_SEND_FAILED` line; a from-ISR send takes the from-ISR path and never the task one; `Mutex::with` is one `xSemaphoreTake` and one `xSemaphoreGive`; and a mutex that cannot be taken gives nothing back and never runs the closure |
| the slot discipline is not optional | a refused send may not claim or write a slot | the same rule, and the same reason, as the timer ring: writing first and sending after is exactly the bug that cost `TimerDemo` its first divergence, where a refused command overwrote a message whose index was still queued |

### What the type system now refuses that the C cannot

Four `compile_fail` doctests on `Queue`, each paired with the working line
it is one character away from — because a "this must not compile" test that
fails for the wrong reason is worse than none. The control arm carries the
same `#![deny(unused_must_use)]` the third refusal relies on, so the
attribute cannot be what makes that one fail.

| the bug | in C | here |
|---|---|---|
| the item size disagrees with the item | `xQueueCreate( n, sizeof( uint16_t ) )` then a send of something else copies the wrong number of bytes | `Queue<u16, N>::send` takes a `u16`; a `u32` does not compile |
| a receive reads the item as the wrong type | the receive fills a buffer you nominate and nothing checks the type | a `u16` comes out because nothing else could have gone in — no `try_from`, no clamp |
| a failed send is ignored and the value lost | `xQueueSend`'s `BaseType_t` is as ignorable as any other return | `Sent<T>` is `#[must_use]` and **has no variant that drops the value** — the failure branch cannot be written without saying what happened to it |
| a queue is sent a pointer to a stack local | legal, and `MessageBufferAMP` in this corpus does it on purpose | the value *moves*; there is no pointer to outlive |
| `xQueueSendFromISR` called from a task | in scope everywhere; takes the wrong lock | `send_from_isr` wants an `Isr`, and a task holds only the kernel |
| `xQueueSend` called from an interrupt | in scope everywhere; may try to block, which corrupts the scheduler | `send` wants the kernel, and an interrupt holds only an `Isr` |
| shared data touched without taking the mutex | a FreeRTOS mutex protects *nothing* — it is a counting semaphore with inheritance and a name, and the data it is said to guard is an unrelated variable somewhere else | `Mutex<T>` owns `T`; there is no `get`, no `lock` that returns it, and no reachable field. `Mutex::with` takes the lock, runs the closure, gives it back |

The two halves of every FreeRTOS call live on two different types here, and
there is exactly one runtime check — at the boundary where `Isr::with`
grants the capability, which is the `configASSERT( xPortIsInsideInterrupt()
)` the C fires on the ports that can tell and stays silent about on the
ones that cannot. Everything downstream of that boundary is the type
system. The shape is `critical_section::with`'s, for the same reason: a
capability that must not outlive its context is borrowed inside a closure
rather than returned.

### The typed face is cheap to verify, and that is the topology argument

| fact | value | method |
|---|---|---|
| Kani harnesses | **25 of 35 verify**, up from 22 of 32 | the three new ones are the Rust face's own properties, over a `Raw` fake with symbolic occupancy: **a value handed to `send` is never lost** (it is in the queue or it comes back *equal to what went in*), what comes out is what went in, and a refused send does not disturb a value whose index is already queued |
| what they cost | **1 s, 2 s and 1 s** | 120, 164 and 166 checks. The raw kernel's nearest equivalent, `queue_generic_send_stale_handle`, does not converge in **700 s** |
| why the difference is the design and not the tool | the state is not there to explore | four of the ten harnesses that do not converge fail because a *symbolic handle* reaches a kernel call and the model checker must then explore every arena slot it could name. Against the typed face those four **cannot be written**: a `Queue<T, N>` is minted by `create` and there is no constructor that invents one. That is compile-time topology stated as something checkable — not "the proof got faster" but "the state the proof was exploring does not exist" |

### Still open in K2.1, and now pointed somewhere

The topology work proper. What that sentence used to say — "so the arena
indirection goes and the six harnesses that need a started kernel get cheap
too" — turned out to name three unrelated things, and *The topology work,
measured* takes them apart. What survives of it:

* The **arena indirection** is not worth removing for speed: `Arena::resolve`
  does not appear in a profile at all. The list was the thing, and the list
  is done at 1.533×.
* The **started-kernel** harnesses have nothing to do with types. CBMC
  cannot afford `start_scheduler` even with no symbolic input, so the fix is
  to stop it executing that prefix, not to name anything by type.
* The **symbolic-handle** harnesses are the real topology item, and a token
  inside the dynamic kernel does not fix them — measured, not argued. The
  dynamic face must keep accepting a handle that may name nothing, because
  that is its contract. So the payoff needs the **static** face of plan
  §5.2 item 3: declared tasks and queues, no create-failure path, exact
  `.bss` by construction, and the four harnesses unwritable because there is
  no constructor that invents a handle. That is the shape of the remaining
  work, and it is an addition to the API rather than a change inside the
  kernel.

## K2.2 — `async` task bodies, answered (2026-09-10)

K2 wrote six continuations by hand — `WaitFrame`, `stream_resume`,
`owed_exits`, `OwedTrace`, the AMP handler's `stage`, and every body's `pc`
— because a kernel with no stacks cannot park a call in the middle. Rust
has a compiler that writes continuations. K2.2 asked one question: **do the
await points land exactly where the `Wait::Blocked` points are today?**

**No.** And the counterfactual was run rather than reasoned about.

| fact | value | method |
|---|---|---|
| awaiting only where the kernel blocks | **hangs — it does not merely diverge** | `PollQ` is the scenario whose whole subject is *not* blocking: a send and a receive at `pollqNO_DELAY`, and a `uxQueueMessagesWaiting` to choose between them. Not one of its calls can answer `Blocked`, so not one `.await` suspends and `poll` never returns. The task runs on past `vTaskDelay` — past the call that took it off the ready list — and keeps sending as a task the scheduler believes is asleep. `kairos-sim PollQ-async 200` does not terminate, and the runaway guard never fires because the loop is *inside a single poll* |
| awaiting after **every** kernel call | **identical, exits included** | `PollQ-async` against `PollQ`'s own C oracle trace: 117,417 lines at 100,000 ticks, `ticks=100051 yields=2056 exits=105608` — `PollQ`'s numbers to the digit, and the same as `PollQ-typed`'s |
| the same claim offline | the **same digest** as `PollQ` | `tests/conformance.rs::the_async_arm_reproduces_pollqs_trace_exactly`. It cannot sit in `PINS` because its bodies are futures the caller has to pin, which is one line more than the table's `start` shape holds — everything else about it is the other seventeen's, and it asserts every pinned number of `PollQ`'s and the FNV-1a/64 of the trace |
| what it costs in the trace | **nothing** | one extra poll per kernel call, and a poll is not a kernel call. The step counter roughly doubles; ticks, yields, exits and lines do not move |

### The finding

`async` writes the continuations, and it writes them correctly. But **where
the suspension points go is the scheduler's business, not the compiler's**,
and deriving them from the kernel's blocking behaviour is precisely wrong.
A `pc` arm ends after *every* kernel call, not only the blocking ones,
because a tick can land at any critical-section exit and switch the task
away — whatever the C would have run next belongs to a frame the scheduler
has abandoned. An `.await` has to be worth exactly one of those arms.

That is the same law K2 met six times from six directions, arriving a
seventh way. It is not a fact about `async`; it is a fact about this
scheduler, and `async` obeys it once told.

### The storage question, settled

An `async fn`'s future has no nameable type, so it cannot be a field of the
runner. It does not have to be. `Body::Async` holds a
`Pin<&mut dyn Future<Output = ()>>`; the caller pins the future as a local
with [`core::pin::pin!`] and lends it; the runner polls it like any other
body. **No allocator, no `unsafe`, nothing unstable** — a safe macro and an
unsized coercion, both stable.

| fact | value | method |
|---|---|---|
| `async` is a body kind, not a prototype | `PollQ-async` runs through the same `Runner`, `step_once` and verdict as the other seventeen | there is no bespoke driver left; the arm is a `start` function like any other, taking two pinned futures |
| it costs no allocator | `pin!`, not `Box::pin` | `tests/conformance.rs::the_async_arm_reproduces_pollqs_trace_exactly` pins on the stack, and so does `kairos-sim` |
| what it forced | **the kernel and the statics are the caller's, not the runner's** | a future borrows the kernel to make its calls, and a future that borrowed a field of the struct polling it would be self-referential and cannot be written. So the runner borrows a `RefCell` of each instead. The borrow can never conflict — the runner polls one body at a time and holds no borrow of its own while polling an `async` one, which is the one rule `step_once` has to keep |
| and it moved nothing | all 18 arms still identical at 100 000 ticks | the refactor touched the file that drives the whole corpus, which is exactly why the corpus is the thing that says it was safe |

The cost is one `RefCell` borrow per step — a counter and a branch — and it
is inside the measurement: the arms' exits did not move, because a borrow
is not a critical section.

## The topology work, measured (2026-09-10)

K1 left the arena-and-list cost row open at **2.08×** `list.c` and K2.1 left
"the topology work proper: tasks and queues named by type rather than by
runtime handle *inside* the kernel, so the arena indirection goes and the
six harnesses that need a started kernel get cheap too". That one sentence
turned out to name three unrelated problems. Taking them apart is most of
what this session produced, and two of the fixes it had already written
down are refuted.

### The list is 26% cheaper, and it was the re-reading

The cost row named the cause: "a branch plus a bounds check on every link
access, several of them re-validating a handle the same call already
validated". Measured, the second half was the whole of it. `links()`
returned all three fields of a node as a tuple, so a caller that wanted
`before.next` and then `next.value` paid two bounds checks and two
end-marker tests to perform two loads.

| fact | value | method |
|---|---|---|
| the list against the C it remakes | **34.22 instructions per operation, 1.533×** — from 46.45 and 2.081× | `bench/list-cost/run.sh` unchanged: callgrind, three run lengths, the cost is the slope, and both arms still print checksum `7de9075f4deb23e5` so they are doing the same work |
| what changed | each node a call touches is read **once** | `next_and_value` and `prev_of` replace `links`, taking only the fields the caller needs; `link_between` takes `after` rather than re-reading `before.next`, which both callers already know; the sorted walk reads one node per step instead of two; `remove` answers with the length it just decremented rather than re-reading the end marker; `next_round_robin` reuses the `ends[list].next` it already took for the wrap |
| what it cost the corpus | **nothing** | 18 scenarios identical to the C kernel at 100 000 ticks, every arm's ticks, yields, exits and lines the same to the digit. This is the scheduler's own data structure, so that is the gate that matters |

**Refuted: end markers in the item array.** The other fix the K1 row named
— give the end markers the same type as items, the way `list.c`'s embedded
`MiniListItem_t` already is, so following a link never asks which kind it
is — makes it **worse**, measured two ways:

| shape | instructions/op |
|---|---|
| one array read per node touched (kept) | **34.22** |
| the same, plus uniform `Node` ends reached by a two-way branch | 42.82 |
| the same, reached by a slice select | 48.85 |

An end marker needs `pxIndex` and `uxNumberOfItems`, which an item does
not, so making it a `Node` costs it eight bytes and pushes those two into
a third array. That is worth more than the branch it removes, and the
slice select is worse again because it materialises two fat pointers where
the branch materialised none. The row is closed as done at 1.533×, not at
the 1.25× the plan hoped for; whether 1.533× is worth revisiting is the
owner's call, and the remaining gap is bounds checks that `forbid(unsafe)`
does not allow us to skip.

### The proof gate had been broken, and nothing could have noticed

`proofs.rs` is `#[cfg(kani)]`. `cargo check`, `cargo clippy` and
`cargo test` therefore never compile a line of it. When K2.1 grew the `Raw`
trait by three methods for `Mutex<T>`, the symbolic fake in `proofs.rs` was
not updated, and **the entire proof suite stopped compiling** while every
gate in the tree stayed green.

    error[E0046]: not all trait items implemented,
                  missing: `raw_mutex_create`, `raw_mutex_take`, `raw_mutex_give`

So K2.1's "Kani is 25 of 35" was a number taken before that commit and
carried forward. It happens to be right — see below — but it was not being
re-taken, and could not have been.

| fact | value | method |
|---|---|---|
| the gate that closes it | **`kairos check --kani`** | `cargo kani --only-codegen`, which builds every harness and verifies none: seconds, and it catches exactly the half that rots. Under WSL on Windows, through the same `host_shell` the oracle uses |
| that it actually catches it | **proven by planting the same break** | renaming one method of the fake makes `check --kani` fail with `the rusty_rtos_kernel-core proof harnesses do not compile`; restoring it passes. A gate nobody has seen fail is not a gate |
| the fake now models a mutex | rather than stubbing it | a mutex is a binary semaphore here as in the C, and the face under proof may assume only what `Raw` promises — a stub that always succeeds would verify `Mutex::with` on the one path it never has to be right about |

### The sweep itself was measuring the wrong thing

Kani's `--harness` is a **substring** filter. `--harness queue_generic_send`
also runs `queue_generic_send_from_isr` and
`queue_generic_send_stale_handle` — three harnesses inside one time budget,
which reads as a timeout on the first. Every harness whose name is a prefix
of another was mis-measured. With `--exact`, `queue_generic_send` verifies
in **8 seconds**.

Re-taken with `--exact`, one harness at a time, 150 s each:

**25 pass, 10 time out, 0 fail, 0 error, of 35** — which reproduces K2.1's
figure exactly, so that number survives its method being corrected. The ten
that do not converge, and what actually stops each:

| cause | harnesses | what would fix it |
|---|---|---|
| a **started kernel** | `task_increment_tick`, `task_get_scheduler_state`, `task_get_current_task_handle`, `task_switch_context`, `task_start_scheduler` | nothing to do with handles or types. `start_scheduler` is a *concrete* computation — CBMC passes 2 GB on it with no symbolic input at all — so the fix is to stop the model checker executing it, by stubbing or by const-evaluating the initial state. That moves a proof obligation rather than discharging it, which is why it is not done here |
| a **symbolic handle** | `queue_generic_send_stale_handle`, `task_priority_set_stale_handle`, and `task_priority_set`, `task_delay`, `queue_take_and_give_mutex_recursive` | see below |

### Refuted: validating at the door does not make those proofs converge

`queue_send_generic` and `queue_take` set up the caller's wait frame and
entered a critical section **before** resolving the handle, so a call with
a handle that names nothing had already touched the kernel by the time it
was refused. The hypothesis was that this is what the model checker must
carry through every slot a symbolic handle could name, and that moving the
check to the door — where the C's `configASSERT( pxQueue )` is — would make
the proof trivial.

It does not. With the handle resolved as the first statement,
`queue_generic_send_stale_handle` still does not converge in **300 s**. The
cost is not *where* the check is; it is that a symbolic handle meeting an
arena forces the checker to reason about every slot's generation, and that
is true wherever the check sits. Three lines tested it, so the 180-site
refactor it would have justified was not written.

**What that leaves.** K2.1's own finding already had the answer and this
confirms it from the other side: against the typed face those harnesses
*cannot be written*, because a `Queue<T, N>` is minted by `create` and
there is no constructor that invents one. The proof payoff of "named by
type" therefore needs the **static** face the plan describes in section 5.2
item 3 — declared tasks and queues, no create-failure path, exact `.bss` by
construction — and **not** a resolved-handle token inside the dynamic
kernel, which must keep taking a handle that may name nothing because that
is its contract. That is a redirection of the remaining K2.1 work, on
evidence rather than on taste.

One piece of it was worth keeping on its own: the wait frame is now set
*after* the resolve rather than before, so a refused call leaves the caller
exactly as it found it. It costs nothing — the resolve was happening anyway
— and the corpus is unmoved at 100 000 ticks.

### Where the kernel's instructions actually go

Taken before any of the above, because the K2.1 sentence claimed the arena
indirection was worth removing for speed:

| fact | value | method |
|---|---|---|
| `Arena::resolve` in the profile | **does not appear** | callgrind on `kairos-sim BlockQ 20000`. It is inlined into its callers and none of them is large enough to surface it |
| what the sim actually spends on | **~47% formatting the trace** | `core::fmt::write` 14.6%, `write_str` 10.5%, `pad_integral` 8.4%, `_fmt_inner` 7.9%, `LineTrace::event` 5.5%. The kernel's own functions total ~15%, the largest being `queue_send_generic` at 3.25% |

That is a fact about the harness, not the kernel — firmware has no trace —
but it does say the arena was not the thing to attack for instructions, and
that the list was. Which is how the list came to be attacked instead.

## K2.3 — the static face (2026-09-10)

The topology measurements pointed the remaining K2.1 work at an **addition
to the API rather than a change inside the kernel**: the dynamic face must
keep accepting a handle that may name nothing, because that is its
contract and the corpus proves it. This is the face that never mints a bad
one.

```rust
system! {
    mod blinky use PosixDemoConfig;
    tasks { consumer: 2, producer: 2 }
    queues { data: u16; 10 }
}
```

### Exact `.bss` by construction, as a number

`TASKS` is the declared count plus the kernel's own two; `QUEUES` is the
declared count; `SLOTS` is the sum of the declared lengths; `ITEMS` and
`LISTS` are `items_for` / `lists_for` over the same numbers. Nothing is
rounded up because nothing is guessed.

| fact | value | method |
|---|---|---|
| a declared kernel against a hand-sized one | **2,088 bytes against 13,600 — 6.5×** | `size_of`, which is exact, decided at compile time and identical on every machine. "Hand-sized" is the demo's own geometry, which is what you must use when you cannot declare: `TASKS = 24`, `QUEUES = 12`, `SLOTS = 128` because it holds eighteen scenarios at once |
| the slot count is exact, and the exactness is load-bearing | every declared slot fills and there is not one spare | `every_declared_slot_is_usable_and_there_is_not_one_spare` fills both declared queues to their declared lengths and asserts the next send of each is refused. A geometry that had been rounded up would pass a weaker test; this one fails if `SLOTS` is 12 or 14 |

### No create-failure path an application can reach

| how a create fails | why it cannot here |
|---|---|
| the task arena is full | `TASKS` **is** the declared count plus the kernel's two |
| the queue arena is full | `QUEUES` **is** the declared count |
| the item slots are exhausted | `SLOTS` **is** the sum of the declared lengths |
| a list is missing | `LISTS` is `lists_for` over the same numbers |
| the priority is out of range | a `const` assertion per task — a **build failure**, where the C gets a `configASSERT` on the bench the day that task first runs |

`System::build` still answers `Result`, because the kernel calls it makes
do and this crate may not panic. The difference is that the `Err` arm is
now unreachable *by construction* rather than merely unlikely, and the
table says which construction closes each one.

### What it refuses, with its controls

Two `compile_fail` doctests, each paired with the working line it is one
character from, on the house's rule that a "must not compile" test which
fails for the wrong reason is worse than none: a priority the config has
no ready list for, and a queue told to carry what it was not declared for.
Eight `compile_fail` rows in the crate now, all with controls.

**And there is no third way in.** `System` has no public constructor but
`build`, and `Queue<T, N>` none but `create`. That is what makes the
symbolic-handle proofs *unwritable* against this face rather than merely
slow — the state they explore has no way to come into being.

## Ten kernel bricks, and five refutations (2026-09-10)

### The instrument

`bench/kernel-ir/run.sh` — callgrind Ir per function over BlockQ, GenQTest
and TimerDemo at 20 000 ticks. **Not a clock**: deterministic to the
instruction, so there is no noise floor, no interleaving, no null arm and
no z-score, and a difference of one is a difference of one. What it cannot
see is cache behaviour, so a change that trades instructions for locality
has to be judged elsewhere and none here was.

Work parity is the strongest available anywhere in this repo: the gate is
`conform --all`, which demands a trace **identical to the C kernel's for
all eighteen scenarios**. Two runs that produce the same trace did the
same work by construction, which is what makes an Ir delta attributable.
Every brick below was gated at 3 000 ticks and each batch at 100 000.

Denominator, stated because it is easy to quote wrongly: the sim spends
roughly half its instructions **formatting the trace**, which firmware
does not have. The kernel's own rows are 94.4M of the 190.5M measured.

### The ten

| # | brick | Ir |
|---|---|---|
| 1 | `copy_data_from_queue` takes the snapshot its caller already holds | **−282,206** |
| 2 | a peek writes `read_from` back no more — `xQueuePeek` "saves and restores" it, which is to say it leaves it as it found it | *(in 1)* |
| 3 | `copy_data_to_queue` folds the message count into whichever resolve its branch is already making | **−149,437** |
| 4 | the yield test asks the cheap local before re-reading the list | **−68,464** |
| 5 | **`begin_wait` where the C sets it** — after finding the queue unusable *and* the block time non-zero, not at the top of every call | **−3,427,915** |
| 6 | `end_wait` does not wipe a frame that was never set | } |
| 7 | `insert_keeping_value` — the event lists stop reading an item's value out to hand it straight back | } **−600,203** |
| 8 | no time-slice test when the tick already required a switch | } |
| 9 | `insert` already stores the value `set_value` was storing | } |
| 10 | `remove` already answers for an item in no list, so `container` need not ask first | } **−38,065** |

**−4,566,290 Ir in total: 2.34% of every row measured, 4.84% of the
kernel's own.** Brick 5 is three quarters of it, and it is the one that
came from reading `queue.c` rather than from reading our own code:
FreeRTOS calls `vTaskInternalSetTimeOutState` only when it is about to
block, guarded by `xEntryTimeSet`, and this kernel was setting a six-field
wait frame on *every* queue call and having `end_wait` wipe it on the way
out.

### The five refutations, which cost the same to find and are worth as much

| refuted | measured |
|---|---|
| threading the caller's snapshot into `copy_data_to_queue` as well as the reader | −245,389 in the helper, **+211,717 in `queue_send_generic`** — a `&Queue` argument forces the caller's local to be addressable and it stops living in registers |
| dropping the `is_empty` pre-check before `remove_from_event_list` (seven sites) | **+492,233** — the callee does answer `false` for an empty list, but the pre-check is an *inlined read* and removing it means making a *call* on every empty list |
| `add_task_to_ready_list` returning the priority it resolved, so `remove_from_event_list` need not resolve the same TCB again | **+120,499** — the tail stops being a tail call and grows a `Result` branch |
| merging `is_empty` and `next_round_robin` in `taskSELECT_HIGHEST_PRIORITY_TASK` | **+1,293,066** — walking down the empty priorities now runs the whole round-robin body instead of a one-field test |
| merging `unlock_queue`'s two resolves of the same queue | **exactly 0** — LLVM had already CSE'd two identical pure reads with no mutation between them |

**One law came out of four of those five**, and it is worth more than any
single brick: *removing a redundant read wins only when the read costs
more than the check that avoids it.* Three separate attempts replaced an
inlined one-field test with a call and all three lost. The redundancy was
real every time; removing it was not.

The fifth is the other standing warning — **check whether the compiler has
already done it**. Two identical resolves of the same slot, with nothing
between them, are one resolve in the emitted code whatever the source says.

### The instrument had a bug, and it was found the same way

The first `diff` reported **+1,758,490** for bricks 6–10 against a true
**−600,203**. Renaming `insert` to `insert_inner` and letting its body
inline into two callers meant the old row vanished and its work reappeared
under three other names, and a total taken over "rows present in both
files" silently dropped it. The fix is to sum *every* row on each side —
the two recordings cover the same program on the same workload, so the
grand totals are comparable even when the rows are not. `diff.py` now
prints both and labels which is the verdict.

## K3 — the first cells, and what an emulator can and cannot say (2026-09-10)

### The M3 cell exists and gates

`rusty_rtos_core/firmware/mps2-an385-qemu-region`: the `small-metal` seam
on a **Cortex-M3**, 9/9, and **QEMU exits 0** because the guest calls
`debug::exit(EXIT_SUCCESS)`. No hardware. That exit code is what makes a
cell a gate rather than something a person watches.

| fact | value | method |
|---|---|---|
| the seam runs on a Kairos target | 9/9, identical numbers to the S3 row | `cargo run --release`; `good_region_size(220 KiB)` -> 196,608 and 63/63 reclaimed to the same address, exactly as on silicon — the geometry is a property of the configuration, not the part |
| the cell had to change machine | `mps2-an385`, not `lm3s6965evb` | `MIN_REGION` is 65,536 bytes and the LM3S6965 has 65,536 bytes of SRAM **in total**: the smallest region the allocator accepts is the whole chip. Check the part can hold the thing before naming the part |

### The corpus runs on a Cortex-M3, byte-identical to C

`rusty_rtos_demo/firmware/mps2-an385-qemu-corpus`. This is K3's main
clause, and the headline is that it needed **no context-switching port**.

| fact | value | method |
|---|---|---|
| six scenarios on ARMv7-M | **every counter, the digest and the byte count match the host's pins** | dynamic, PollQ, BlockQ, semtest, GenQTest, TimerDemo at 2000 ticks. The pins are the C kernel's — `tests/conformance.rs` diffs them against `oracle/traces/*` — so this is agreement with **C FreeRTOS to the byte on ARM**, not self-consistency |
| why no port was needed | a scenario is a **state machine** | one `step` per C statement with a `pc`, so a task needs no stack of its own and the kernel needs no context switch to run it. `rusty_rtos_demo-core` is `no_std` with **no `alloc`** and builds for `thumbv7m-none-eabi` unchanged |
| and that is a consequence, not a trick | the kernel keeps a blocking call's locals in the **TCB**, not on a C stack | which is what lets a call return and be re-entered. **The property that made the corpus provable is the one that makes it portable** — K2's stackless design paying a second time |
| it can fail | changing `dynamic`'s pinned `exits` by one is caught | `exits` is sim time itself, so anything that changed *when* the scheduler ran moves it long before it moves a digest. The poison reports `exits 21346 want 21347` with every other field matching, and the cell exits non-zero |

Six of the seventeen pinned scenarios, chosen to cover different kernel
paths. The other eleven are host-side only because they are pinned there,
not because anything stops them running here.

### And on RV32: the same seventeen, on a second architecture

`rusty_rtos_demo/firmware/riscv32-qemu-corpus`, on
`riscv32imac-unknown-none-elf` under `qemu-system-riscv32 -machine virt`.
K3 asks for the corpus on M3-qemu **and** RV32-qemu; this is that half.

| fact | value | method |
|---|---|---|
| the whole corpus on RISC-V | **all 17 scenarios** match the C kernel's ticks, yields, exits, lines, digest and byte count | 5.4 s, QEMU exit 0. The pins are the C kernel's — the host test diffs them against `oracle/traces/*` — so this is agreement with **C FreeRTOS on RISC-V**, not self-consistency |
| and the M3 cell now runs all 17 too | up from a chosen six | the subset was never a limit: the whole corpus is 5.6 s under QEMU. So the corpus is byte-identical to C on **three** architectures — the host, ARMv7-M and RV32 |
| it needed no RISC-V port | a scenario is a **state machine** | the same reason the M3 cell needed no ARM one. **The K2 stackless design has now paid for itself on a second architecture** — a blocking call's locals live in the TCB, so a task needs no stack and the kernel needs no context switch to run one |
| it can fail | moving `dynamic`'s pinned `exits` by one gives FAIL with **every other field matching** and exit 1 | `exits` is sim time itself, so it moves before a digest does |

**One table, three readers.** The pins, and the FNV-1a/64 sink, now live in
`rusty_rtos_demo_core::pins`; the host test and both cells read them. They
were three hand-written copies of seventeen rows of hex, which is a drift
waiting to happen — and **a cell that silently disagrees with the host is
worse than no cell**, because it reports PASS against numbers nobody is
comparing. A pin that moves, moves once, and the poison above proves all
three see it.

**A gate that only worked in one terminal.** Adding the second cell
exposed it: `kairos check --qemu` inherits whatever `PATH` the operator's
shell had, so on a clean `PATH` **both** cells reported "the cell failed,
exit 101" when the truth was that QEMU was not on it — a tooling failure
wearing a test failure's clothes, which is the most expensive kind. The
gate now resolves each cell's emulator (`KAIROS_QEMU_DIR`, then `PATH`,
then where installers put it), prepends it for the child, and if it cannot
be found says so as *"the cell did NOT run, so this is not a verdict about
the code"*. Verified by running the gate with QEMU off `PATH` entirely.

### An hour: the corpus survives one, and one scenario owns the clock

K3 asks that "the full corpus check task passes **one hour**". The sim runs
`PosixDemoConfig`, whose `TICK_RATE_HZ` is 1000 — the same value as the
oracle's `FreeRTOSConfig.h`, which is why the corpus can be byte-identical
to C at all — so one tick is one millisecond and **an hour is 3,600,000
ticks**, not a round number chosen to be quick.

| fact | value | method |
|---|---|---|
| every scenario survives an hour | **17 / 17**, ~125 s | `crates/rusty_rtos_demo-core/tests/soak.rs`, via `kairos check rusty_rtos_demo --soak`. Three checks per scenario, and the third is what stops it being theatre: `pass` (the scenario's own check task), `!runaway`, **and `ticks >= 3,600,000`** — a scenario that ended at tick 5 would report `pass` perfectly happily, having never been asked to survive anything |
| what it claims | **liveness, not conformance** | there are no C pins at 3,600,000 ticks and getting them means an hour-long instrumented C run per scenario. So this does what the C demo itself does — asks each scenario's own checker — and claims that and nothing more. Conformance is `tests/conformance.rs` and the two QEMU cells, at 2000 ticks against the C trace |
| it can fail | sizing the step limit for 2,000 ticks instead of an hour | `semtest FAIL ... ran away: the step limit stopped it at tick 52219; stopped early at tick 52219 of 3600000`, and the test fails. Only `semtest` trips it, which is itself the finding below |

Both QEMU cells carry the same hour behind `--features soak` — a feature
and not a second binary, because `kairos check --qemu` runs a plain
`cargo run --release` there and a second bin target would make that
ambiguous. Roughly three hours each, so they are background runs, not
gates.

**And the hour runs on both QEMU cells too — where it turned into something
stronger.** `mps2-an385-qemu-corpus --features soak` and
`riscv32-qemu-corpus --features soak`: **17/17 still running after 3,600,000
ticks** on each, exit 0. That was the liveness clause. But the counters they
printed were then diffed against the host soak's, by machine rather than by
eye:

```text
rows: host=17 m3=17 rv32=17
IDENTICAL across all THREE: host, Cortex-M3, RV32
             -- 17 scenarios, ticks+yields+exits, at 3,600,000 ticks
```

**Every counter agrees, on every scenario, on three architectures, at 1,800x
the pinned length.** The conformance cells prove agreement with the C kernel
at 2,000 ticks; this says the x86-64, ARMv7-M and RV32 builds stay in lockstep
for a simulated hour.

**Say what it is not.** This is our two builds agreeing with each other, not
with C — there are no C pins at 3,600,000 ticks, which is why the soak claims
liveness. It is a **cross-architecture determinism** result, and a strong one:
a divergence anywhere in an hour of scheduling, blocking, timer and queue
traffic would move `exits`, which is sim time itself. It is recorded as a
ledger row rather than a gate, because re-checking it costs a QEMU run of
hours per architecture.

**K3's one-hour clause is now met on two of its three targets** — M3-qemu and
RV32-qemu — with the third, a C6, blocked on the same hardware B4b needs.

**Where the two minutes go, and why the obvious reading is wrong.**
`semtest` takes **92 of the ~125 seconds**; everything else takes between
0.3 and 4.1 — while doing *fewer* yields and *fewer* critical-section exits
than `GenQTest`, which finishes in 4.1. Wall time and work count disagree,
so a counter had to settle it, and `Verdict::steps` did:

| scenario | steps per exit | ns per step |
|---|---:|---:|
| `semtest` | **345.6** | 6.3 |
| `GenQTest` | 1.0 | 68.4 |
| `countsem` | 1.5 | 35.2 |
| `dynamic` | 1.7 | 45.6 |

semtest runs 345 state-machine steps per unit of sim time where the others
run one or two; its steps are individually *cheap*; and the ratio is 345.6
at 200,000 ticks and **345.6 at 400,000** — exactly constant, so structural
rather than a leak. That is `semtest.c`'s polling pair, which takes its
semaphore with a **zero block time**: the same shape the ledger already
records for `dynamic`'s SUSP_RX, and the reason the sim contract counts
critical-section exits rather than yields in the first place. **It is
faithful to C, so it is a cost and not a defect** — and the first
hypothesis, that a 22x wall-time outlier meant an algorithmic defect, was
refuted by measuring the scaling: 0.98 and 1.06 across a 4x tick range, for
semtest and its neighbours alike. Linear, with a large constant.

### And the kernel drives it: the scheduler on a Cortex-M3

`rusty_rtos_port/firmware/mps2-an385-qemu-kernel`. The port switches; the
**kernel chooses**. Same fixed-priority preemptive scheduler the corpus
proves against C FreeRTOS, on ARMv7-M.

```text
ticks                 402
switches              81
high-priority laps    40   (expected about 40)
low-priority laps     38273
woke early            0
RESULT: PASS        QEMU exit code: 0
```

| joint | how |
|---|---|
| who runs next | `PendSV` -> `Kernel::switch_context` -> `CURRENT_SP_SLOT` |
| when to switch | `SysTick` -> `Kernel::increment_tick` -> pend a PendSV if it says so |
| how a task blocks | `Kernel::delay`, then a yield; it resumes on the line after |

**The lap count is the check, not "both tasks ran".** A round robin makes
both tasks run. **40 laps against 402 ticks and a 10-tick delay** is what
only a priority scheduler with a working `delay` produces — too many means
`delay` never blocked, too few that it blocked and was never woken. And
`woke early == 0` is the task reading the tick either side of its own
delay. Deleting the one line that asks the kernel anything gives **1 lap
against an expected 120**, two failures and exit 1.

**Two costs worth more than the feature.**

* **Every task the kernel creates needs a stack, including the two it
  creates for itself.** `start_scheduler` makes `IDLE` and `Tmr Svc`
  unconditionally, and `Tmr Svc` sits *above* both application tasks. The
  first run locked up — `can't escalate 3 to HardFault`, `PC=0` — because
  the scheduler correctly chose the highest-priority ready task and that
  task had no stack.
* **A cell that hangs is worse than one that fails.** The high task ends
  the run; a broken joint means it never runs again and nothing ends
  anything, so `kairos check --qemu` would wait forever. The task that
  always runs now owns a deadline. The poison above *hung* before that
  existed — which is how the gap was found.

### The Cortex-M port exists: a real context switch, proven

`rusty_rtos_port-cortex-m`, the first crate in the family to lift the
workspace's `unsafe_code` deny — per item, with `#[expect]`, so an unused
fence is itself a warning.

| fact | value | method |
|---|---|---|
| all five things a `port.c` is | critical sections, yield, tick, stack init, the PendSV switch | PRIMASK with a nesting count (`uxCriticalNesting`); PendSV pending for the yield; SysTick for the tick; an architectural exception frame for a new task |
| two real tasks, real stacks | **100 / 100 resumptions over 200 switches**, exit 0 | `firmware/mps2-an385-qemu-switch`. Each task re-checks **every word of a 256-word witness array on its own stack** after every switch, plus a value the compiler must keep in a callee-saved register — `r4-r11` are exactly what the hardware does *not* stack, so they are what the asm must save |
| **the test can fail** | deleting `stmdb r0!, {{r4-r11}}` gives `corrupted witness word index 0` and exit 1 | that one instruction is "stop saving the callee-saved registers". A fenced `unsafe` whose test has never been made to fail is a fence around nothing; the diff is in `UNSAFE.md` |
| the stack is the port's, not the kernel's | `CURRENT_SP_SLOT` holds the **address of** the running task's SP word | a Kairos TCB has no stack pointer, because the kernel keeps a blocking call's locals in the TCB. FreeRTOS gets the same shape by putting the SP first in its TCB ("THIS MUST BE THE FIRST MEMBER") |

**The bug it cost, which is worth more than the feature.** The first
version started nothing: `main` printed and hung. A stacked PC has **bit 0
clear**, which is what the architecture wants of an exception frame — but
`start_first_task` enters the task with `bx`, and `bx` reads bit 0 as
"switch to ARM state". An M-profile core has no ARM state, so it took a
UsageFault, which presents as *a task that simply never runs*. One `orr`.

**And a constraint met from the other side.** The port's counters are
`AtomicU32`, not `AtomicU64`, because **ARMv7-M has no 64-bit atomics** —
the same fact that cost `rusty_zstd` and `rusty_erasure-core` their
bare-metal builds and produced two whole `build-me-bare` bricks. Writing a
port is where you stop reading that as someone else's bug.

It is **not the scheduler**: choosing the next task is the kernel's job,
and wiring `Kernel::switch_context` into `set_scheduler` is the next
increment. The cell installs a round robin so the switch can be judged
alone.

### QEMU cannot supply a cycle, a latency, or a work count

Measured six ways, over 1000 / 2000 / 4000 iterations of one loop, with
the SysTick clock source pinned to `Core` so a misconfigured peripheral
could not be mistaken for a result:

| probe | 1000 | 2000 | 4000 |
|---|---:|---:|---:|
| DWT `CYCCNT` | 0 | 0 | 0 |
| SysTick, plain, run 1 | 688 | 493 | 492 |
| SysTick, plain, run 2 | 667 | 490 | 317 |
| SysTick, plain, run 3 | 810 | 585 | 395 |
| SysTick, `-icount shift=0` | 1 | 0 | 0 |
| SysTick, `-icount ...,sleep=off` | 1 | 0 | 0 |

DWT is unimplemented there. SysTick's deltas **shrink as the work grows**
— the opposite of a work counter — because they track host wall time while
TCG's translation cache warms, and they differ run to run. Under `-icount`
the virtual clock does not advance for it at all. TCG plugin support is
compiled in but the Windows build ships no plugin library, so `libinsn` is
out too.

**So the division of labour is fixed on evidence:** QEMU cells carry
correctness and gate on an exit code; **cycle and latency rows need
silicon** — the ESP32-C6 is a Kairos target with a real `mcycle`. This is
the same law the kernel's own speed work already runs on, arriving from
the other side: *an emulator can tell you whether something is right, and
cannot tell you what it costs.*

### And silicon can: the first cycle numbers the project has had

`rusty_rtos_core/firmware/esp32s3-devkit-alloc-cycles`, on an ESP32-S3
DevKit over USB. Xtensa **`CCOUNT`** is a real per-cycle counter, which is
what the section above says QEMU has none of. Cycles for one `alloc` +
one `free` through the `small-metal` seam: best of 32 rounds of 256 ops,
8 warm-up rounds, an identical empty round measured the same way and
subtracted (16 cycles/op of harness), a checksum so removed work is
visible, and `region_contains` proving the blocks came from the declared
region.

| size (bytes) | cycles per alloc+free |
|---:|---:|
| 16 / 32 / 48 / 64 | 115 / 125 / 146 / 157 |
| 96, 128 | **209** |
| 160, 192, 224, 256, 288, 320, 384, 512 | **314** |
| 768, 1024, 2048 | **271** |

**A step function, and the 768–2048 step is 43 cycles CHEAPER than the
160–512 step below it** — a larger allocation costing 14% less than a
smaller one, monotonically on either side of the boundary.

**The finding is the control, not the table.** Sizes 3x apart produce
totals identical *to the cycle* (160 through 512 all read 84,498), yet
those are eight distinct bins — so the plateaus are not the size-class
geometry, and `alloc.rs` says why no geometric model should be expected
to fit: "a tight alloc/free loop frees into `local_free`, so the queue
front's `free` list is ALWAYS dry when the next allocation arrives". This
harness *is* that loop, so its cost is set by the slow collect, whose
frequency is a property of free-list **state**. That makes "is this a
function of size, or of history?" a question the table cannot answer
about itself.

So the cell measures every size **twice, ascending and descending**, and
fails if a size does not reproduce its own total. **All seventeen sizes
up to 2048 reproduce to the cycle** — the cost is a function of size, and
the step-down is real rather than an artefact of sweep order. Three
models were written down and all three are refuted by the data: bin
geometry, collect frequency (`FIXED_PAGE`/`good_size` is monotone in size
and the cost is not), and a per-page cost amortised over blocks (fits
16…256 at ~0.875 cycles/byte, then breaks at 512). The mechanism is
inside a house dependency; the reproduction is written up for the owner
at `kairos-upstream/drafts/rusty_alloc-size-class-inversion.md`, in the private `Remade-With-Rust/kairos-upstream`.

**One row is not quoted.** 4,096 is `FIXED_PAGE`, so it cannot come from
a page and takes the dedicated path; it read **789, 861 and 933**
cycles/op in three runs differing only in which sizes ran before it. Not
a function of size, so the cell declares it an expected asymmetry — the
way the house gate declares its expected failure — and the check keeps
its teeth for the other seventeen.

**It is ONE ARM, and does not close `build-me-bare` B4b.** B4b wants this
against the C `heap_4` on a Kairos target — the C6's `mcycle`. An arm is
not an A/B. It is recorded because the family had no timing number from
any silicon at all, and because the control makes it admissible.

### The A/B: rusty_alloc against FreeRTOS heap_4, on silicon

`rusty_rtos_core/firmware/esp32s3-devkit-alloc-ab`. **The half of
`build-me-bare` B4b that was missing was never the board — it was the
second allocator.** `heap_4.c` is portable C over a static byte array with
no port dependency, so `build.rs` compiles it **verbatim** out of the
oracle checkout (the same `FreeRTOS-Kernel` the conformance traces come
from) with `xtensa-esp32s3-elf-gcc`, and runs it under the identical
CCOUNT harness.

| request | rusty_alloc | heap_4 | |
|---|---:|---:|---|
| 16 B | 99 | 236 | **2.38x faster** |
| 32 B | 109 | 236 | **2.17x faster** |
| 64 B | 141 | 236 | **1.67x faster** |
| 128 B | 193 | 236 | 1.22x faster |
| 256–512 B | 298 | 236 | **1.27x slower** |
| 1024–2048 B | 255 | 236 | 1.08x slower |

**The null A/B is zero.** The Rust arm run against *itself*, presented to
the harness as two different allocators, came back **37,963 vs 37,963** —
byte-identical. The resolution floor is 0 cycles, so every difference above
it counts. Checksums are compared across arms, so work parity is checked
rather than assumed; the arms are interleaved ABBA; the 11 cycles/op null
arm is subtracted; and the shims make `heap_4`'s `vTaskSuspendAll` a no-op
because the Rust arm is `ra_single_threaded` and takes no lock either — a C
arm paying for a lock the Rust arm does not pay for would be measuring the
lock.

**A flat `heap_4` number is its best case, and the other half says so.**
236 at every size is the tell: this workload keeps one block live, so the
free list is one entry and first-fit answers in one step. 512-byte requests
against a free list of 16-byte holes:

| holes | rusty_alloc | heap_4 |
|---:|---:|---:|
| 0 | 297 | 235 |
| 8 | 297 | **466** |
| 32 | 297 | **1,114** |
| 128 | 297 | **3,706** |

Flat against linear — **~27 cycles per free-list entry walked** (+28.9,
+27.5, +27.1) — and `heap_4` has lost its advantage by **eight holes**.
Different floor functions, not one function tuned differently:
`heap_4 = c0 + 27 x entries walked`, `rusty_alloc = f(size)` independent of
heap state.

**And the first fragmentation probe was refuted.** Fragmenting with holes
of the *same* size as the request made `heap_4` **faster** (236 -> 218),
because first-fit stops at the first block that fits. A long list costs
nothing until the allocator must walk *past* it; the holes have to be
smaller than the request. Recorded so nobody re-runs it.

### And the losing range is ROUTING, proven by one byte

The only range `rusty_alloc` loses is 256–2048, and that is not a gradual
effect. The plateaus in the single-arm cell are not the bin geometry — they
span eight bins — but they land exactly on `rusty_alloc`'s **page-kind
boundaries** under `ra_small_profile`, the configuration every Kairos
firmware builds with: `SMALL_OBJ_SIZE_MAX = SLICE/8 = 512` and
`MEDIUM_OBJ_SIZE_MAX = 4*SLICE/8 = 2048`.

A prediction that can be wrong by one byte was tested that way:

| size | total | cycles/op | route |
|---:|---:|---:|---|
| 511 | 84,498 | 314 | small |
| 512 | 84,498 | 314 | small |
| **513** | **73,490** | **271** | **medium — steps here** |
| 2,047 | 73,490 | 271 | medium |
| 2,048 | 73,490 | 271 | medium |
| **2,049** | **242,979** | **933** | **own large span — and here** |

**One byte moves it 43 cycles at 512 and 662 cycles at 2,048**, totals are
byte-identical within each route, and nothing steps anywhere else. So the
answer to "gating or routing" is **routing**: size picks the page kind, and
the small-page route costs 16% more per operation than the medium one above
it. Closing that gap puts 256–512 at roughly 255 and turns the one losing
range into a win.

**Four quantitative models were written down first and all four are
refuted** — bin geometry, collect frequency, a per-page cost amortised over
blocks (fits 16…256 at ~0.875 cycles/byte, breaks at 512), and
blocks-per-page within a route (256 and 512 share a page size, have 16 and
8 blocks, and cost the same 314). Recorded so nobody re-runs them.

**What this cannot say, and the run that can.** On a 32-bit target two
constants both equal 512 — `SMALL_SIZE_MAX` (`SMALL_WSIZE_MAX * INTPTR_SIZE`
= 128 x 4, the top of the `direct[]` table) and `SMALL_OBJ_SIZE_MAX`
(`SLICE/8`, the top of the small-page range). They coincide, so this cannot
name the router. On **64-bit they differ** — 1,024 against 512 — so the same
sweep on a host build under `ra_small_profile` settles it, and needs no
hardware. It matters because it decides whether the fix is free or a
footprint trade. Written up at
`kairos-upstream/drafts/rusty_alloc-size-class-inversion.md`.

**Not quoted, and said so.** `heap_4`'s byte charge is measured — request +
8 exactly, from `xPortGetFreeHeapSize()` either side of one allocation — and
`rusty_alloc`'s has no counterpart: the seam exposes no per-allocation
usable size, and `region_stats()` answers over region *extents* so it does
not move for a small allocation (the same dead check already caught in this
project's own S3 reclamation test). Reaching around the seam to
`rusty_alloc::alloc::usable_size` is the one thing the seam exists to
prevent, so it is recorded as a **seam gap** rather than turned into half a
comparison.

**It does not close B4b.** The kill test names the ESP32-C6 —
`riscv32imac`, a Kairos target with a real `mcycle`. Xtensa is not one of
the family's four targets. This closes the **comparison** and leaves the
**target** clause open, exactly as B4a substituted `mps2-an385` for the
`lm3s6965evb` the plan named and recorded why.

### Chasing the router: a host that refuses to reproduce, and a gate born from it

The one-byte experiment says the cost is routed. The obvious next question is
*by which constant*, because on a 32-bit target two of them equal 512:
`SMALL_SIZE_MAX` (`SMALL_WSIZE_MAX * INTPTR_SIZE` = 128 x 4, the top of the
`direct[]` table) and `SMALL_OBJ_SIZE_MAX` (`SLICE/8`, the top of the
small-page range). On 64-bit they come apart — 1,024 against 512 — so the same
sweep on a host names its own cause.

`tools/alloc-route-probe` is that sweep: one self-contained binary, the same
`=2.1.0` and the same two cfgs, pinned to one core at High priority,
`rdtsc`, best of 400 rounds with a null arm subtracted. It prints the
constants it compiled against rather than restating them.

| fact | value | method |
|---|---|---|
| the host does **not** reproduce the step | no step at 512, none at 1,024; the only step is 2,048 -> 2,049 (11 -> 91 cycles/op) | **and the conclusion drawn from this was WRONG — see the correction below.** What it licenses is "this host cannot discriminate"; what was written was "so it is not the `direct[]` table" |
| `MEDIUM_OBJ_SIZE_MAX` is confirmed twice | it routes on **both** platforms | the one boundary not in question |
| and the host runs **24x faster** | 11–13 cycles/op against the device's 271–314 | not a core-speed ratio — a different path. The host hits a fast path the device does not |
| what was blamed | the **prim** | wrong, and §"the correction" below says how the error was made |

**A hypothesis, labelled as one — and since settled, against it.** The guess
was that the device takes `malloc_generic` where the host does not. Measured
on the device (below): **`generic` is 1.0000 per op on BOTH routes**. The slow
path is universal here and is not the difference. The claim that "nothing on
the seam exposes a slow-path counter" was also wrong: `alloc::stats()` has
carried it all along — it is in `alloc`, not `prim::fixed`, which is why a
seam re-exporting the fixed-region API in one `pub use` could not see it. The
seam now re-exports `stats` and `usable_size`, and both gaps are closed.

### The correction: it is the POINTER WIDTH, not the prim

**The allocator's maintainers took the report, reproduced it, and
re-attributed it** (`rusty_alloc/docs/plans/fixed-prim-small-step.md` §8). The
report was right that there is a real step, right that it is routing, and
right about where it hurts. It was **wrong about the cause**, and the way it
was wrong is worth more than the fix.

`SMALL_SIZE_MAX` is not a profile constant. It is
`SMALL_WSIZE_MAX * INTPTR_SIZE` — **1,024 on a 64-bit host and 512 on a
32-bit chip**, where it lands on the same byte as `SMALL_OBJ_SIZE_MAX`. So the
host sweep above was run on **a machine on which the suspect is not at the
scene**, and "there is no step at 1,024, therefore it is not `direct[]`" does
not follow. Re-run with pointer width as the only variable, same crate, same
cfgs, the OS prim in **both** arms:

| arm | 256 vs 264 (control) | **512 vs 513** | 2048 vs 2049 |
|---|---:|---:|---:|
| x86-64 | −0.9% | **+0.1%** | −81.9% |
| **i686** | −0.4% | **+8.6%** | −80.6% |

`prim::fixed` is exonerated: the step appears on the OS prim as soon as the
pointer is 32 bits. The run that would finish it was not "inside the crate"
as the report claimed — it was `cargo build --target i686`.

**This is the three-probe rule's own failure mode** (`codec-measurement` §11):
*never refute a lever on one measurement, and vary the axis that could flip
the answer*. The axis here was pointer width, and it was held fixed.

### FIXED in rusty_alloc 2.2.0, and the root cause was deeper than the sweep

The periodic-collect reading above was the *proximate* mechanism. The root
cause is one literal. `page_extend` bounds its batch at 4 KiB of payload as
`span_shift = 4 + slice_count.trailing_zeros()`, and the `4` is
`SEGMENT_SLICE_SIZE / 4096` — **correct only at the shipped 64 KiB slice.**
Under `ra_small_profile` the slice is 4 KiB, so the bound was **256 bytes**;
for a 512-byte class the batch computes to 0, `.max(1)` clamps it to **ONE**,
and every page carried `capacity == 1`. With no second block for the fast
path to find, `malloc_generic` ran on **100% of allocations**.

**That is why this project measured `generic` at exactly 1.0000 per op on
both routes and wrote it down as "the slow path is universal here."** It was
the symptom, recorded faithfully and read as a property.

Verified on the S3 at `=2.2.0`, per 10,240 alloc+free pairs:

| size | `generic` 2.1.0 → 2.2.0 | per op | `retired` 2.1.0 → 2.2.0 |
|---:|---|---:|---|
| 256 | 10,240 → **640** | 1.0000 → **0.0625** | 20 → **1** |
| 512 | 10,240 → **1,280** | 1.0000 → **0.1250** | 21 → **4** |
| 513 | 10,240 → 10,240 | 1.0000 | 0 → 0 |
| 1024 | 10,240 → 10,240 | 1.0000 | 0 → 0 |

512 lands on **0.1250 generic/op — their host prediction to the digit.**

Timed, cycles per alloc+free: **256 goes 314 → 118, 512 goes 314 → 131.** The
one-byte step **inverts**: 512→513 was 314→271 (513 cheaper by 14%) and is now
131→266, so 512 is cheaper by **51%**. The palindrome control still holds.

**And the range Kairos lost is now won.** Against `heap_4` on the same part:

| request | 2.1.0 | 2.2.0 | ratio before → after |
|---|---:|---:|---|
| 64 B | 141 | **91** | 1.67x → **2.58x** |
| 256 B | 298 | **101** | **0.79x (loss)** → **2.33x** |
| 512 B | 298 | **114** | **0.79x (loss)** → **2.06x** |
| 1024 B | 255 | 249 | 0.93x → 0.94x |

Fragmented — 512-byte requests against 128 16-byte holes — it is **32.51x**.

**One thing did NOT improve, and it is only visible here.** The bin-peek route
(513–2048 on a 32-bit target) still enters the generic path on every
operation. That is `alloc.rs`'s documented behaviour — a tight alloc/free loop
frees into `local_free`, so the queue front's free list is always dry and the
peek can never hit — and it churns nothing, so it is a cost and not a defect.
It is worth reporting because **on a 64-bit host those same sizes are on the
`direct[]` route** (`SMALL_SIZE_MAX` = 1,024 there), so the bin route below
1 KiB cannot be isolated on the box the maintainers have. 1024 and 2048
remain the only sizes where `heap_4` is ahead, by 6%.

**The check was inverted, not deleted.** It asserted that the direct route
retires ~20 pages per 10,240 ops — true, and the defect. A check written to
confirm a mechanism becomes a check that defends it; it now fails if the fast
path stops hitting or the churn comes back. Regression controls: the seam is
green on all four bare-metal targets and the M3 QEMU cell is still 9/9.

### The mechanism, counted on 32-bit silicon

Their §8.2 counted it on a host; §8.5 asked for the device, noting the counter
half "is deterministic and needs no quiet box at all". It is now in
`esp32s3-devkit-alloc-cycles` — no clock, no best-of-N, just `stats()` either
side of the boundary, on the 32-bit part their host arms could not be:

| size | route | `generic`/op | pages_fresh | retired |
|---:|---|---:|---:|---:|
| 256 | `direct[]` | 1.0000 | 21 | 20 |
| 512 | `direct[]` | 1.0000 | 21 | **21** |
| 513 | bin peek | 1.0000 | 1 | **0** |
| 1024 | bin peek | 1.0000 | 1 | **0** |

10,240 / 512 = 20, and the direct route carves 21 — the initial page plus one
per periodic sweep. That is `GENERIC_COLLECT_DEFAULT`, 512 at the small
profile: in a loop holding one block live the page is empty at every sweep, so
every sweep costs a carve, an extend and a retire. The bin route carves its
first page and then never churns. It corroborates their host figure (195 per
100,000 = one per 513) on hardware they do not have.

**And the first version of this check was wrong in the house's favourite
way.** It keyed on `pages_fresh` and demanded **zero** from the bin route —
but every route must carve a first page for a class it has not served before,
so it reported FAIL against correct behaviour. Churn is the discriminator, not
carving; the check now keys on `pages_retired`, which is 21 against 0.

**It is a deliberate trade, and the real defect was that a firmware could not
move it.** Short sweeps buy back a starvation the small profile had at 10,000
and cost page churn; which is right depends on the workload, and a bench that
keeps one block live cannot decay, so it sees only the cost. Upstream shipped
`--cfg ra_generic_collect="64" | "4096" | "65536"`, plus
`prim::fixed::shape_of(size)` carrying `direct_route` so the next consumer
meets the boundary in a `const` assertion instead of on silicon. **What is
still open is how much of the 16% the churn accounts for** — their timing arm
was withdrawn as inadmissible (its control flipped from +8.0% to +0.6%), and
the device is the right box for it.

**The probe that could not answer it, deleted rather than shipped.** The
footprint route — a small page is one slice (4 KiB), a medium four (16 KiB),
so a 513-byte allocation should claim 4x the region — is invisible from
outside: `region_stats()` answers over region **extents**, moving when a whole
segment is claimed and not when a page is. It read 0 for all six sizes. **Third
check-that-cannot-fail this project has caught**, after B3's grep and the S3
reclamation test.

### The kernel allocates nothing — and the check that seemed to prove it did not

The Kairos-side lever was going to be "re-size any kernel allocation that
lands in the expensive band". There are none: **the kernel allocates nothing,
in any configuration** — no `Box`, no `Vec`, and not one `cfg(feature =
"alloc")` in `rusty_rtos_kernel-core`. So the routing cliff cannot reach
Kairos internally at all; it reaches only an application's allocations,
through K4's `heap_3` seam. That is now written into
`rusty_rtos_heap/docs/plans/rusty_rtos_heap.md` as a design input, along with
what that package must **not** do about it (round requests up to cross the
boundary: 16% for up to 25% more memory, and it stops being a win the day the
allocator is fixed).

**But the check that appeared to prove it was worthless, and that is the
lesson.** "It compiles with `--no-default-features` on four bare targets" says
nothing about allocation: `extern crate alloc` resolves from the sysroot on a
bare-metal target whatever the feature flags say. A `Box::new` added to the
kernel compiled **clean through all eight `no_std` rungs**. The claim has to be
read off the object file.

| fact | value | method |
|---|---|---|
| the kernel references the allocator zero times | **0** `__rust_alloc` / `__rust_dealloc` symbols | `cargo build -p rusty_rtos_kernel-core --no-default-features --target thumbv7em-none-eabihf`, then `llvm-nm` the rlib |
| and the check can fail | a `Box::new` turns 0 into **2** (`__rust_alloc`, `__rust_alloc_zeroed`, both undefined) | poison-proven both directions |
| it is a gate, not a note | `no_alloc_crates` in `KAIROS.toml`; runs with the ordinary `kairos check`, not behind a flag | fast, and exactly the kind of property that rots silently. A missing `llvm-nm` reports "the check did NOT run, so this is not a verdict about the code" rather than passing |

**And the report is filed.** `rusty_alloc`'s own repo now carries
`docs/plans/fixed-prim-small-step.md` in the house plan format — the step, the
host refutation, the four dead models, the two seam gaps, the three
reproducers, and the `heap_4` A/B that says why 16% is worth their time.

### Flash and RAM, decomposed — the part of K3 a cell CAN carry

A footprint is a property of the linked binary, not of execution.
`llvm-size -A` and `llvm-nm -S` on the linked artifact:

| line | bytes | class |
|---|---:|---|
| the declared `Region` | 196,608 | **structure** — it is the declaration |
| `rusty_alloc`'s `FIRST_HEAP_BOX` | 1,752 | structure |
| everything else in `.bss` | 312 | — |
| **= `.bss`** | **198,672** | remainder **0** |
| `.data` | 4 | |
| alignment gap, **in no section** | 12 | defect-class in general; harmless here |
| **static RAM** | **198,688** | |
| flash (`.vector_table` + `.text` + `.rodata` + init) | **22,992** | |

**The number to quote: 2,080 bytes of RAM and 22.5 KiB of flash** is what
the allocator seam costs a firmware beyond the heap it declares. The
region is 98.96% of `.bss`, which is an identity rather than a
correlation, and it says the only lever on this firmware's RAM is the
argument to `good_region_size`.

The 12-byte gap is printed because it belongs to no section and would
vanish from a table built from `size -A` alone. It is nothing on a 4 MiB
part; on a fixed small map that is precisely how a saving gets overstated.

## A use-after-free in the oracle's own tracing, found by adding a scenario (2026-09-10)

Two K1 corpus scenarios the plan lists were never built and never recorded as
dropped — `TaskNotify` and `AbortDelay` — and the kernel implements what they
cover (five notification methods, `abort_delay`) with **zero conformance
coverage**. Registering `TaskNotify` in the oracle harness took three small
edits and immediately produced a **SIGSEGV between 300 and 600 ticks**.

| fact | value | method |
|---|---|---|
| what crashed | `traceTIMER_COMMAND_SEND` reading `( xTimer )->pcTimerName` | `timers.c:486`, reached from `xTimerDelete` at `TaskNotify.c:473` |
| what it really was | a **heap-use-after-free**, on three distinct threads | ASan: allocated by the task in `xTimerCreate`, **freed by the timer daemon** in `prvProcessReceivedCommands` -> `vPortFree`, read back by the task |
| why | the hook fires **after** `xQueueSendToBack` | the daemon runs at `configTIMER_TASK_PRIORITY`, above the task, so a `tmrCOMMAND_DELETE` send preempts straight into the free. Upstream never sees it because the default macro ignores its arguments |
| the fix | never dereference a timer handle outside creation | the name is recorded at `traceTIMER_CREATE` and looked up by **pointer value**, which stays a valid key after the free |
| the fix is a fix, not a change | **`TimerDemo`'s stored trace is byte-identical** — no git diff on the `.zst` — and `kairos conform TimerDemo` still reports 3,092 lines identical | a pinned scenario over the same hook is the control this needed, and it already existed |

**The first read of the evidence was wrong, and cheaply so.** Under gdb the
struct looked *intact* — `pxCallbackFunction` and `xTimerPeriodInTicks` were
correct — with only `pcTimerName` and one list field garbage. That reads as
"my macro has the wrong offset". It was a glibc tcache header (fd pointer +
key) written into the first 16 bytes of the freed chunk. **A partial clobber
looks like a misread struct**; ASan turned a guess into three stacks in one
run. Written up for the owner at
`kairos-upstream/drafts/freertos-timer-trace-uaf.md`.

### And `TaskNotify` still cannot be an oracle — for an unrelated reason

With the crash fixed it runs clean and passes its own check
(`ticks=2000 yields=220 exits=2803`), and then fails the reproducibility gate:

```text
run:   3435 lines; ... yields=218 exits=2769
run.2: 3374 lines; ... yields=206 exits=2725
error: the trace is NOT deterministic
```

`TaskNotify.c:124` is `uxNextRand = ( uint32_t ) prvRand;` — a PRNG seeded
from a **function address** — and lines 557/570 use it to set the notifying
timer's period. So with PIE/ASLR the demo differs between two runs of the same
binary, **in C, before any Rust remake is attempted**. Same shape as
`QueueSet` (seeded from the address of a stack local), and the same owner
decision: patch the seed to a constant — the `oracle patch` verb already makes
six exact-anchor edits to `port.c`, so the mechanism exists — or leave the
scenario out. It stays registered in the oracle's scenario table so the
finding is one command to reproduce; `conform --all` keeps its own separate
list of eighteen and is unaffected.

## AbortDelay: three missing kernel APIs, and a contract ambiguity (2026-09-10)

`AbortDelay` was in the plan's **K1** corpus subset, never built, never
recorded as dropped — while `abort_delay` and the notification API it
exercises had **zero** conformance coverage. It is the only scenario whose
subject is unblocking a task early, and it is thorough: eight blocking
surfaces, each blocked three times — time out, be aborted, time out again.
The third is the one that matters, because an abort that corrupted the
task's delayed-list membership would still pass the first two.

| fact | value | method |
|---|---|---|
| the C oracle is reproducible | **2,549 lines, byte-identical across two runs** | unlike `TaskNotify`, which seeds a PRNG from a function address |
| the remake agrees | **1,945 lines byte-identical**, then one ordinal difference (below) | `kairos conform AbortDelay --ticks 2000` |
| three real APIs were missing | `ulTaskNotifyTake`, `xTaskGetHandle`, `vQueueDelete` | each found by the diff, each a core FreeRTOS call |
| the corpus is unmoved | 17 scenarios still byte-identical; no-alloc gate still 0 symbols | the additions touch no existing path |

### The lesson the diff taught twice: exits are the clock

`TASK_NOTIFY_TAKE` and `TASK_NOTIFY_TAKE_BLOCK` were declared in the K0
trace vocabulary and **emitted by nothing** — the kernel waited only on
notification *bits*, never on a *count*. `notify_take` fills that in.
`xTaskNotifyGive` needed no method: the C defines it as
`xTaskGenericNotify(.., eIncrement, ..)`, so `NotifyAction::Increment`
already was it.

The other two were found the same way, and the mistake was mine both times:

* **`xTaskGetHandle` emits no trace event and costs one critical-section
  exit**, because it walks the lists inside `vTaskSuspendAll` /
  `xTaskResumeAll`. The remake held the handle from creation instead, on
  the reasoning that an event-less call cannot move the trace. Every event
  agreed and the exit column was **one short** at the first block.
* **`vQueueDelete` takes no critical section itself** — and ends in
  `vPortFree`, which `heap_3` wraps in suspend/resume exactly as it wraps
  malloc. So a delete costs one exit while tracing nothing. The sibling
  `event_group_delete` already carried that note; the queue had no delete
  at all, which also meant a scenario creating one per cycle could not run
  long without exhausting the arena.

**An event-less call is not a free call.** `exits` is sim time itself, so
anything that takes a critical section moves the clock whether or not it
says so.

### And a fourth defect, in the reading of the C

Two of the eight helpers block the *aborted* attempt with `portMAX_DELAY`,
not `xMaxBlockTime` — `prvTestAbortingTaskNotifyWait` and
`prvTestAbortingSemaphoreTake`. An infinite wait is the stronger test:
nothing but the abort can end it, where a timeout would prove nothing. It
also sends the task to the **suspended** list rather than a delayed one,
which emits no `MOVED_TASK_TO_DELAYED_LIST` — and that extra line in our
trace is how the difference was found. Assuming all eight helpers shared
one shape cost 1,800 lines of agreement.

### The 1,946th line is a CONTRACT ambiguity, not a kernel divergence

The C harness keys a queue's trace ordinal on the object's **malloc
address** (`prvOrdinalPerKind` searches a table of pointers). A freed object
leaves its entry behind, so a successor gets a **new** ordinal unless the
allocator hands back the same block. Our arena reuses the freed **index**.

The two agree whenever the recreated object is the same size — every
scenario in the corpus that deletes, `EventGroupsDemo` included — and part
when it is not. `AbortDelay` deletes a binary semaphore and creates a
1-item queue; `malloc` returns a different block, so the C says `q3` where
our index says `q2`.

**A running count was tried and reverted.** It makes `AbortDelay` identical
for all 2,549 lines and **breaks `EventGroupsDemo`** (707,007 bytes against
a pinned 702,507), whose C ordinals reuse precisely because its addresses
do. Neither rule is right, because the subject identity in the contract is
an allocator address — reproducible for a given libc, and not a property of
the kernel at all. The index rule is kept, being the one that matches
seventeen scenarios, and `trace.rs` records why. **Making the C's ordinal a
pure creation count would settle it and requires re-pinning every scenario:
an owner decision, alongside `QueueSet` and `TaskNotify`.**

> **Taken 2026-09-20, and the estimate was wrong in our favour.** It did not
> require re-pinning every scenario -- it required re-pinning **one**. The C
> rule only differs from a creation count where an address was REUSED, and
> across the whole corpus that happens in `EventGroupsDemo` alone. See
> *AbortDelay conforms* at the end of this ledger.

## K4 — the heaps (2026-09-10)

### `heap_4` diffed against C, operation for operation

`rusty_rtos_heap-core`'s `Heap4` is `heap_4.c`'s algorithm transcribed, not
re-designed: address-ordered first fit, split only when the remainder is
**strictly** larger than twice the header, coalesce with the block before
and the block after on free, the allocated bit in the top of the size word.

| fact | value | method |
|---|---|---|
| 20,000 operations agree | the **offset** first fit chose, the **free bytes** remaining, the **minimum ever** free — every time | `heap4_differential`; the C arm is `heap_4.c` compiled **verbatim** from the pinned kernel by `oracle/run.sh`, and its trace is checked in so the diff needs no C toolchain |
| offsets, not pointers | a pointer is not comparable across two programs | the C driver sets `configAPPLICATION_ALLOCATED_HEAP` so `ucHeap` is its own aligned array and prints offsets into it; this side's offset 0 is that base |
| and `forbid(unsafe)` | headers written in-band exactly where the C writes them | the arithmetic, the split points and the coalescing tests are the same arithmetic |

**It passed first time, which is a weaker result than it sounds.** The first
workload — 32 slots of at most 300 bytes — leaves a steady state of ~2.6 KB
in 8 KB and **never refused a single request**, so it was agreement about
the easy half of an allocator.
`the_workload_reaches_the_branches_that_matter` caught that; 48 slots of at
most 600 bytes is what it takes, and the agreement above is on **that**
workload: **2,087 refusals, 784 distinct offsets, minimum-ever free 1,232 of
8,192**.

### The RAM table is an identity, not a total

`total = arena + bookkeeping`, remainder **0** on every row, and bookkeeping
is a **fixed 56 bytes** that does not grow with the arena. Per profile means
per **pointer width**: `heap_4`'s header is one pointer plus one `size_t`,
so 8 bytes on every Kairos target and 16 on the oracle's host, and the
minimum block and split threshold move with it.

**A host test cannot measure a target**, so the same identity is a `const`
assertion in `heap4.rs` that the compiler evaluates for whichever target is
being built — `kairos check` proves it on ARMv7-M, ARMv8-M, `riscv32imac`
and `riscv32imafc`. The table's `header/op` and `min blk` columns are the
target's (const parameters); `total` and `book` are the host's, and the test
says so rather than letting a reader assume otherwise.

### `StaticAllocation`, in the form this design makes meaningful

The C demo shows a system **can** be built with
`configSUPPORT_DYNAMIC_ALLOCATION 0`. Here it cannot be built any other way:
`rusty_rtos_kernel-core` and `rusty_rtos_demo-core` are both gated
allocation-free on the linked **rlib**, poison-proven both directions, and a
bare-metal cell that declares no global allocator cannot link an allocation
at all — also poison-proven.

**A symbol scan of the linked ELF was built, found unsound, and removed.** A
cell poisoned with a real `#[global_allocator]` and a live `Box::new` still
carried **zero** allocator symbols, because LTO inlines the shim away: the
check reported "static allocation only" about a cell that had a heap. The
rlib is the instrument — there the allocator is an **undefined** symbol and
cannot be optimised out — and the linked binary is not. Sixth
check-that-cannot-fail this project has caught, and the fifth of them mine.

### 41.6% fewer instructions, with the differential as the gate

The differential is the ideal gate for speed work: any change must leave
20,000 operations byte-identical to the C.

| step | Ir | per op | |
|---|---:|---:|---|
| baseline | 4,674,633 | 233.7 | |
| the walk reads each header **once** | 3,145,762 | 157.3 | **−32.7%** |
| the insert reads each node **once** | 2,730,871 | 136.5 | **−13.2%** |
| the same move in `alloc`'s tail | 2,776,099 | 138.8 | **+1.7% — REFUTED** |

**−1,943,762 Ir, −41.6%**, checksum identical at every step.

**The profile said where to look, and it was not where anyone would have
guessed: 30% of the program was `core::num::uint_macros`** — the saturating
and checked arithmetic written to satisfy the workspace's
no-bare-arithmetic lint — with another 13% in `copy_from_slice` moving eight
bytes at a time. Reading a block's 16-byte header once per visit instead of
as two separately bounds-checked 8-byte reads removed both: `slice/mod.rs`
and `ptr/mod.rs` left the profile entirely, because `first_chunk` compiles
to a direct load.

**The third application of the same move LOST**, and is kept in the source
as a comment rather than deleted: *removing a redundant read wins only when
the read costs more than the check that avoids it*. In a walk the read is
per node and the branch is not; in `alloc`'s tail the reads are once per
call and folding them introduced a merge the unconditional stores did not
need. The law this project keeps relearning, relearned in its own allocator.

**And the harness was wrong before any of this.** Its first version ran
`cargo build --target x86_64-unknown-linux-gnu` with stderr discarded; that
**succeeded**, writing to a target-specific directory, while the script went
on to profile the stale binary left in `target/release`. It reported an
instruction count **identical to the digit** across a real source change —
which is exactly what a stale binary looks like, and exactly what
`codec-measurement` §10 says to check for. `run.sh` now fails loudly if any
source is newer than the binary it is about to measure.

## K5a — task deletion, and the three costs no scenario could see (2026-09-10)

`vTaskDelete` was declared in the trace enum and emitted **zero times**.
`SchedulerImplementation::schedule_task_deletion` needs it, `death.c` needs
it, and it was expected to close a measured mutant-coverage gap — one
feature, three payoffs, which is why it led K5a. **The third payoff is only
a third delivered, and the measurement below says so.**

The gate is `death.c` remade as a state machine: **4,370 lines byte-identical
to the C kernel at 4,000 ticks**, counters included, and byte-identical again
on Cortex-M3 and RV32 under QEMU. It is the nineteenth scenario in `conform`
and the eighteenth pinned.

| fact | value | method |
|---|---|---|
| the gate | `kairos conform death` | 4,370 lines, `ticks=4000 yields=55 exits=3890 lines=4369`, identical on both arms |
| offline pin | digest `0xec10_62cf_1c36_07ea`, 129,014 bytes | FNV-1a/64 **of the C kernel's own trace**. The method was validated by recomputing `dynamic`'s existing pin from its oracle file first and matching it to the digit, then applied to `death` |
| two more architectures | Cortex-M3 and RV32, same digest and byte count | `kairos check rusty_rtos_demo --qemu`, `no_std`, no alloc, no per-task stack |
| poison-proving | 8 of 10 deliberate defects caught | below |
| one hour of simulated time | `death` survives: `ticks=3600057 yields=57580 exits=3533911` in 0.3 s | `kairos check rusty_rtos_demo --soak`, 18/18. ~1,800 create/kill/self-kill cycles, which is also the evidence that the arena RECLAIMS slots: a deletion that leaked one would exhaust `MAX_TASKS` within seconds |

### Why `death` and not a unit test

`vTaskDelete` has two paths that are not variations on each other. Deleting
another task frees its TCB inside the call, after the critical section;
deleting **yourself** cannot, because the caller is still running out of that
TCB, so the C parks it on `xTasksWaitingTermination` and the idle task frees
it in `prvCheckTasksWaitingTermination`. `vSuicidalTask` takes both paths
back to back. A kernel that got the deferred path wrong passes the other
eighteen scenarios, because **none of them deletes anything**.

### Three costs the whole corpus was blind to

The sim port counts outermost exits **only once the scheduler is running**
(`exits_are_not_counted_before_the_scheduler_runs`), and every scenario
before `death` creates all of its tasks before `vTaskStartScheduler`. So
three real costs had never been charged, and could not have been:

| cost | exits | where the C spends it |
|---|---|---|
| `xTaskCreate`'s two allocations | 2 | `pvPortMallocStack` then `pvPortMalloc`, each `vTaskSuspendAll`/`xTaskResumeAll` on heap_3 |
| `pxPortInitialiseStack` | 1 | the Posix port wraps `pthread_create` in `vPortEnterCritical`/`vPortExitCritical`. A **port** fact, not a kernel one — hence its own `Config::PORT_STACK_INIT_CRITICAL`, default `false`, because a silicon port lays out a register frame and spends nothing |
| `prvDeleteTCB`'s two frees | 2 | `vPortFreeStack` then `vPortFree`, **outside** the critical section |

A delete therefore costs one exit for its own section plus two for the
frees, and the trace shows exactly that: `TASK_DELETE SUICID1 #2136` then
`TASK_DELETE SUICID2 #2139`, on both arms.

### Two findings that only a mid-call preemption could produce

**The reap cannot happen at the switch.** The first design reaped the
self-deleted task at the top of `vTaskSwitchContext`. The C's own trace
refutes it: it prints `TASK_SWITCHED_OUT SUICID2` *after* SUICID2 deleted
itself, because the TCB is still alive until idle runs. Reaping at the
switch would have read the name out of freed storage and emitted an empty
one. Found by reading the C trace, not by the corpus — no scenario could
have caught it.

**`xTaskCreate` can be preempted between its allocations and its trace
event.** A tick landing on any of those three exits stops the C creator dead:
`prvAddNewTaskToReadyList` has not run, so the new task is on no ready list
and `traceTASK_CREATE` has not fired. Our create completed in one
uninterruptible Rust call and put both events on the wrong side of the
switch. The fix reuses the kernel's existing `OwedTrace` mechanism — the
line an abandoned frame had not reached yet — with one new variant that
carries a ready-list insertion rather than only a trace line.

**A reused task index inherits the dead task's debt.** Our arena hands back
the same index; the per-index bookkeeping (owed exits, owed yield, owed
trace) is keyed on the index, not the TCB. A task deleted while it still
owed the tail of a call bequeathed that debt to its successor, which then
paid an exit it never incurred. This surfaced 1,100 lines *after* the first
create/kill/self-kill cycle had matched perfectly. The fix is one line —
mark the index unstarted — because `hand_over` already empties all three on
a first switch-in. Clearing them here as well **also worked and was worse**:
two places that must agree, and a gate that cannot catch either being
dropped because the other still covers it. That redundancy was found by the
poison run, not by review.

### Poison-proving: 8 of 10

Every gate must be made to fail on purpose. Caught: the delete's two free
exits; the idle reap disabled; the self-delete not deferred; the index not
marked unstarted; reaping at the switch; the port's stack-init exit; the
create tail not deferred; only one of the two create allocations charged.

**Not caught, with reasons rather than excuses:**

- **`prvResetNextTaskUnblockTime` after a delete.** Provably unobservable,
  not merely uncovered: removing a task from the delayed list can only make
  the true next-unblock time *later*, so a stale value is always *earlier*,
  and an early value costs `xTaskIncrementTick` a scan that finds nothing
  and self-heals. No trace line can move. No scenario can cover this.
- **Removing the deleted task's event-list item.** `death` deletes a task
  sitting on the *delayed* list, never on an event list, so that branch is
  never reached. This one is a real coverage gap and closing it needs a
  scenario that deletes a task blocked on a queue or semaphore.

### The mutant gap is NOT closed, and `death.c` cannot close it

K2's mutants row says five of its six survivors are one line —
`if self.running && C::USE_PREEMPTION && self.current_priority() < priority`
in `prvAddNewTaskToReadyList` — and names `death.c` as the corpus's answer,
because no scenario then created a task after the scheduler had started.
`death` now does. So the claim was testable, and it was tested rather than
repeated: the line was mutated six ways by hand, the way `cargo mutants`
would, and each run gated on `kairos conform death`.

| mutation | `death` |
|---|---|
| condition → `true` | **killed** |
| `<` → `<=` | **killed** |
| condition → `false` | survives |
| body removed (no yield) | survives |
| `<` → `>` | survives |
| `<` → `!=` | survives |

**Two of six.** The reason is precise and was predictable from the
scenario's own source: `vCreateTasks` creates both suicidal tasks at
`uxTaskPriorityGet( NULL )` — *its own* priority. The new task therefore
never outranks the running one, so the guarded `taskYIELD_IF_USING_
_PREEMPTION` is reached and evaluated but is never TRUE. Mutations that
make it fire spuriously are caught instantly; mutations that suppress a
yield which never happens are invisible.

Reachable is not the same as killed, and "the corpus creates tasks at
runtime now" is not the same as "the arm is exercised". **What actually
closes this gap is a scenario that creates a HIGHER-priority task while the
scheduler is running**, which `death.c` never does and no scenario in the
corpus does.

### CLOSED (2026-09-19), by a unit test rather than a scenario

No corpus scenario can do it, and none can be added without a C original
to diff against -- but the arm does not need a *scenario*, it needs a
*caller*. `system::tests::creating_a_higher_priority_task_asks_for_a_switch_and_a_lower_one_does_not`
starts the scheduler, creates one task below the running priority and one
above, and asserts on `CountingPort::count_yield` that the kernel asks for
a switch in the second case and not the first.

Both halves are required, and that was established by planting all four
survivors by hand and re-running:

| mutation | killed by |
|---|---|
| condition → `false` | the HIGHER half |
| body removed (no yield) | the HIGHER half |
| `<` → `>` | the HIGHER half |
| `<` → `!=` | the LOWER half |

Four for four. The LOWER half reads like a formality and is the only thing
that kills `!=` -- a guard that fires whenever the priorities merely
*differ* is wrong in a way only a lower-priority create can show.

K2's mutants row therefore reads **40 of 42** rather than 36, with the two
remaining survivors elsewhere in the file. The general point stands and is
worth keeping: a corpus diffed against a C oracle can only exercise what
its C scenarios do, and where that stops, a unit test with a counting port
is the instrument -- not a bigger corpus.

### An inconclusive experiment, recorded so it is not re-run

The first poison — charging `create_task` two extra exits — left the corpus
green, which looked like a hole in the gate. It was not: the build was
fresh (a deliberate `panic!` in the same function diverged at line 1) and
the scenario was `dynamic`, whose tasks are all created **before** the
scheduler starts, where the port counts no exits at all. The poison landed
in an uncounted region. A poison that cannot fire is not evidence about the
gate.

## K5a — the corpus on ESP32-S3 silicon, and a plan row corrected (2026-09-10)

**The corpus is byte-identical to C FreeRTOS on a PART, not an emulator.**
`rusty_rtos_demo/firmware/xiao-s3-corpus`, XIAO ESP32-S3 (esp32s3 rev v0.2,
8 MB flash): all **18 scenarios**, every counter, the FNV-1a/64 digest and
the byte count matching the host's pins — which are themselves diffed
against the C kernel's traces.

| fact | value | method |
|---|---|---|
| the cell | `RESULT: PASS -- 18 scenarios byte-identical to the C kernel on ESP32-S3 SILICON` | `cargo +esp run --release`, flashed over `espflash`, captured from reset |
| app size | 172,704 bytes | `espflash` |
| poison-proven | `dynamic`'s `exits` 21346 → 21347, rebuilt and reflashed, reports `FAIL ... exits 21346 want 21347` | the same one-exit poison the two QEMU cells use |
| architectures now | host, ARMv7-M, RV32, **Xtensa LX7** — the last on silicon | four |

### The finding: Xtensa needed no port at all

The K5a plan row assumed the first Xtensa deliverable had to be a context
switch. **It does not, and the corpus proves it by running without one.** A
scenario is a state machine driven by `Runner`, one `step` per C statement
with a `pc`, so a task keeps its locals in the TCB and owns no stack;
`rusty_rtos_demo-core` is `no_std`, no `alloc`, and builds for
`xtensa-esp32s3-none-elf` unchanged. A context switch is what you need to
run tasks that own stacks. It is not what you need to prove this kernel
schedules identically to C FreeRTOS on this part.

This is the third time the stackless K2 design has paid for itself on a new
architecture, and the first time it has done so on hardware.

### A false alarm of my own, and what it cost to catch

Working from this box, `rusty_esp_mid/firmware/` is empty, no `rusty_esp_*`
firmware cell has a single file, and I recorded that K5a's named target —
`rusty_esp_mid/firmware/xiao-s3-keys` — "does not exist", along with the
P-256 baseline the plan says it measured.

**That was wrong, and wrong in exactly the way this ledger keeps warning
about: I checked the local box and concluded about the world.** The `.git`
directories under `/f/coding/janus/*` are EMPTY — the tree is a scaffold,
not a checkout. Asking the actual repository settles it in one call:

```
$ gh api repos/Remade-With-Rust/rusty_esp_mid/contents/firmware/xiao-s3-keys
.cargo  Cargo.lock  Cargo.toml  rust-toolchain.toml  src
```

and its own ledger carries the baseline, measured 2026-09-06:

> *M1, the other half: what a P-256 signature costs an ESP32-S3.* A
> signature is **95 ms** and a verification is **151 ms** at full clock.
> Method line: `board=xiao-esp32s3-sense opt-level=3 lto=fat
> metric=in-process-us`.

So **K5a's measurement clause is NOT blocked**: the firmware exists and the
baseline is real and published. What is missing is only a local checkout.

The transferable rule is the one the campaign already had and I did not
apply to myself: *an absence observed in one place is not an absence.* The
check that would have prevented it is the same one that proved `dynamic`'s
digest method before trusting it on `death` — confirm the instrument against
something whose answer you already know. An empty directory tree is not an
instrument.

### The pin drift, now settled with evidence

The plan flagged "esp-hal 1.2.0 vs 1.2.1" as a thing to settle first. It is
real and it is exactly where the plan said:

| | esp-hal |
|---|---|
| `rusty_esp_mid/firmware/xiao-s3-keys` | `=1.2.0` |
| every Kairos S3 cell, including `xiao-s3-corpus` | `=1.2.1` |

Both also pin `esp-bootloader-esp-idf =0.6.0`, `esp-println =0.18.0` and
`esp-backtrace =0.20.0` identically, so the drift is one crate wide. Joining
the two in one firmware means one of them moves, and §2.7's rule is that one
seam crate owns that pin rather than two.

## K5a — what the kernel costs a real Janus workload (2026-09-11)

**One full scheduling round costs 3,724 ns — 893 cycles at 240 MHz — which
is 39 parts per million of a P-256 signature.** That is K5a's measurement
clause, answered on the XIAO ESP32-S3.

> **These figures replaced 8,313 ns / 1,995 cycles / 88 ppm on 2026-09-21**,
> when this cell was found to be measuring through the shadowed-`NoTrace`
> defect (see the 2026-09-21 rows at the end of this ledger). No kernel code
> changed. The clause's conclusion is unchanged and stronger.

`rusty_rtos_kernel/firmware/xiao-s3-signing`. The workload is not ours:
`p256 = "0.13"` (RustCrypto), the same crate and major version
`rusty_esp_mid-core` signs with, over the same fixed 32-byte prehash.

| fact | value | method |
|---|---|---|
| a scheduling round | `per_round_ns=3724 per_round_cycles=893` | a queue send, a queue receive and two context switches, timed on their own over 20,000 rounds |
| switches actually taken | `switches=40000 per_round=2` | counted, not assumed |
| as a share of one signature | **39 ppm** (3,724 ns of 94,390,000 ns) | a small measured number over a large measured one |
| our P-256 cost on this part | sign 94.3 ms, verify 149.4 ms (minima) | 100 of each, min and median |
| the Janus baseline it echoes | sign 95 ms, verify 151 ms | `rusty_esp_mid` M1, 2026-09-06 — **cross-binary** (they pin esp-hal `=1.2.0`, we `=1.2.1`), so a reference and never the result |
| work parity | `bare=100s/100v/100ok scheduled=100s/100v/100ok` | printed every run |

### The headline is a direct measurement because the obvious one does not work

The natural experiment — run the workload with the kernel and without, and
subtract — **failed, and failed loudly enough to be useful**: the scheduled
arm came out FASTER than the bare one. Two ~24.5-second batches cannot
resolve a few microseconds. This is `codec-measurement` §5's "never take a
differential of two same-sized numbers", met in the wild.

So the kernel is timed on its own — the identical send/yield/receive/yield
sequence, with the signature removed. The bare-vs-scheduled arms are still
run, ABBA over four rounds, but as the **work-parity** check rather than the
result, and the batch delta is printed with an explicit note that it is not
the overhead figure.

### Three guards, two of which fired

- **Work parity caught a 75x phantom.** The first run had the scheduled arm
  finishing 75x faster — because it had completed **zero** operations.
  `TIMER_TASK_PRIORITY` was 3, above the workers at 2, and this firmware
  steps only the workers' bodies, so the daemon never blocked and starved
  them. Without the work count that would have been a spectacular and
  entirely false result.
- **Asymmetric timed regions.** The bare arm signed once *inside* its batch
  window and the scheduled arm did not: 101 signatures timed against 100,
  ~94 ms, enough on its own to make the arm doing MORE work look faster.
- **Switch counting.** A yield returning to the same task is cheaper than a
  switch, so a loop that never changed task would report a small number and
  look fine. Two switches per round, counted.

### Clock resolution, stated

`esp_hal::time::Instant` resolves to 1 µs; a scheduling round is far below
that. Timing rounds individually would print zeroes and call it proof, so
20,000 rounds are timed as one 166 ms batch — five orders of magnitude above
the quantum — and divided. The method line prints the resolution.

### What this does and does not settle

It settles the K5a question *"does the kernel cost anything against the P-256
baseline?"* with a number: **39 ppm**, on the same part, same crate, same clock. (It read 88 ppm until 2026-09-21, when the cell was found to be measuring through the shadowed-`NoTrace` defect; the answer moved in the same direction as the question.)

It is **not** `xiao-s3-keys` itself running on a Kairos kernel. That joining
act lives in a Janus repository and is the owner's; this is the same workload
measured on our side of the fence.

## K5b — the Xtensa context switch, and why it is needed at all (2026-09-11)

**`rusty_rtos_port-xtensa` switches contexts on an ESP32-S3**: 100 and 99
resumptions, zero faults, yielding from three call frames deep every time —
`rusty_rtos_port/firmware/xiao-s3-switch`.

### First, the finding that makes it necessary

`esp-radio-rtos-driver` **0.4.1** (the plan said 0.4.1 and the box had a
stale 0.3.0; `cargo search` settles it — 0.4.1 is current) declares five
implementation traits, not four. `SchedulerImplementation::task_create` is
the one that decides the architecture:

```text
/// This function is used to create threads.
/// It should allocate the stack.
fn task_create(&self, name: &str, task: extern "C" fn(*mut c_void),
               param: *mut c_void, priority: u32, core_id: Option<u32>,
               task_stack_size: usize) -> ThreadPtr;
```

A C function pointer, a stack size, `schedule_task_deletion` documented as
"the thread stack can be free'ed", and blocking semaphore waits the radio
blob performs from inside its own call frames. **A stackless kernel cannot
satisfy this interface**, and no amount of finishing the rest of the traits
changes that.

So the corpus and the radio joint want opposite things, and both answers are
now measured rather than assumed:

| | needs a context switch? |
|---|---|
| the conformance corpus on this chip | **no** — `xiao-s3-corpus`, 18/18 byte-identical to C, no port at all |
| the `esp-radio-rtos-driver` joint | **yes** — the interface hands us stacks and C function pointers |

### The design, and the one that failed first

The switch runs **inside a software interrupt**. `xtensa-lx-rt`'s interrupt
entry has by then spilled every register window and written the whole machine
into a `Context`, so the switch is two struct copies: trap frame out to the
outgoing slot, incoming slot in over the trap frame. The exception exit
restores it.

The first design did it the obvious way — from task context, spilling the
windows by hand with `xtensa-lx-rt`'s own `SPILL_REGISTERS` sequence. **It
started a task, ran it, printed from it, and hung** the moment that task
nested calls deeply enough to need the register file back. It is recorded
because a shallow probe passed it, and because the fix was not "try harder at
the spill" but "do it where the context is already saved" — which is what
FreeRTOS's Xtensa port and Espressif's `esp-rtos` both do.

That is why the test yields from **three frames deep**. A switch that
mishandles the window file corrupts the *outer* frames specifically, so a
test that only yielded from the task body would have certified the broken
version.

### Poison-proving, including one that did not fire

| poison | result |
|---|---|
| the outgoing context is not saved (the analogue of deleting `stmdb r0!, {r4-r11}` from the ARM port) | **hangs, never reaches a verdict** — it cannot return to `main`, whose state was never written down |
| `A12` clobbered in the resumed context | **not detected; still passes** |

The second is recorded because it is a fact about the *test*, not a defect:
the witnesses the compiler generated are stack-resident, so this cell proves
**stack and frame integrity across a switch**, not that every individual
register is preserved. A hundred laps of nested calls is strong evidence the
restore is complete; it is not the same as asserting it.

### Versions, settled while doing this

`xtensa-lx-rt` is a `links` crate, so its version must match whatever
`esp-hal` resolves — `=1.2.1` pulls `0.23`, and pinning `0.20` is a hard
build error rather than a duplicate. The port pins `0.23` for that reason.

## The board gate: `kairos check --board` (2026-09-11)

Four hardware cells landed for K5, and `kairos check --qemu` skips every one
of them — it discovers cells whose runner is a `qemu-system-*`, and theirs is
`espflash`. This repository's own doctrine, printed in the tool's help, is
that **"a test nothing runs is a test that has stopped existing"**. Four
ungated cells is that failure, freshly created.

`--board` closes it. It discovers cells the same way `--qemu` does — by
reading the runner out of the cell's own `.cargo/config.toml`, so the list
cannot fall behind the directory — builds each with the toolchain the cell
pins, flashes it, and reads its verdict.

```
$ kairos check rusty_rtos_kernel rusty_rtos_port rusty_rtos_demo --board
$ espflash flash --monitor   (xiao-s3-signing)
  RESULT: PASS -- kernel overhead measured, work parity held,
$ espflash flash --monitor   (xiao-s3-switch)
  RESULT: PASS -- 100 and 99 resumptions, every witness
$ espflash flash --monitor   (xiao-s3-corpus)
  RESULT: PASS -- 18 scenarios byte-identical to the C kernel
```

**The verdict is a printed line, not an exit code, and silence is failure.**
An emulator cell ends in `debug::exit` and hands cargo the guest's verdict; a
board has no such channel, so the contract is that a cell prints `RESULT:
PASS` or `RESULT: FAIL`. A cell that prints neither within five minutes
fails — which is not a technicality, because **a hang is exactly how a broken
context switch presents**, and the first Xtensa port hung precisely that way.
A gate treating silence as success would have certified it.

`xiao-s3-signing` gained a verdict of its own to be gateable: work parity
held, two real switches per round, and the kernel's share under 1000 ppm — a
regression bound, generous against the 88 ppm then measured (39 ppm after the 2026-09-21 correction, so more generous still), because the bound is
not the result.

### The trap that cost the first run

The cell build removes `RUSTUP_TOOLCHAIN` from the child's environment rather
than setting it. When `kairos` is itself run through `cargo run`, cargo
exports the toolchain that built *it*; the child cargo obeys the environment
over the cell's `rust-toolchain.toml`, and the Xtensa cell gets built with an
x86 compiler. The symptom is a wall of `'esp32s3' is not a recognized
processor for this target`, which reads like a broken toolchain install and
is nothing of the kind.

## K5b — the radio seam: it type-checks, and it cannot link yet (2026-09-11)

All five `esp-radio-rtos-driver` 0.4.1 traits are implemented against the
Kairos kernel and build for `xtensa-esp32s3-none-elf`:
`rusty_rtos_port/firmware/xiao-s3-radio`. **The open question — can a kernel
of this shape satisfy the interface at all — is answered yes.**

### What the compile proves, and what `nm` says it does not

`nm` on the built ELF finds **zero `esp_rtos_*` symbols out of 2,295**. LTO
drops the `#[no_mangle]` registration shims because nothing references them,
and nothing references them because `esp-radio` is not linked.

So the compile is a **type-check**, not a link proof, and this row says so
because the ELF does. Checking instead of assuming is the only reason the
distinction is here at all — the natural thing to write after a green build
would have been "the seam is wired", and it is not.

### The blocker is a companion set, not a design

| | esp-hal | esp-radio-rtos-driver |
|---|---|---|
| `esp-radio 1.0.0-beta.0` — the only published radio | `~1.1.0` | **0.3.0** |
| every Kairos S3 cell | `=1.2.1` | — |
| this adapter, and the plan | — | **0.4.1** |

`xtensa-lx-rt` is a `links` crate; `esp-radio` wants `^0.22` and
`esp-hal 1.2.1` wants `^0.23`, so cargo refuses the pair outright.

**This corrects an earlier row.** The 0.4.1-versus-0.3.0 question was
"settled" on 2026-09-11 by observing that 0.4.1 is current on crates.io.
True of the *driver*, misleading about the *stack*: the published radio
consumes 0.3.0, so the 0.3.0 on this box was never stale — it was the
matching one. Two versions of the same name are not a newer and an older
until you check what consumes them.

### The design, for the record

| trait | how |
|---|---|
| `SchedulerImplementation` | `Kernel` calls; `task_create` allocates a stack and builds a `Context`; the switch is the port's |
| `SemaphoreImplementation` | the kernel's counting semaphores and mutexes |
| `QueueImplementation` | a heap ring guarded by two counting semaphores |
| `WaitQueueImplementation` | a semaphore plus a waiter count, so `notify` broadcasts |
| `TimerImplementation` | the cell's own table, serviced on `delay` and a microsecond clock |

Two findings worth keeping:

**A stacked task CAN block on this kernel.** `Wait::Blocked` is documented as
*"leave the program counter where it is and make the same call again when the
task next runs"* — a **retry** protocol, which serves both shapes: a corpus
task returns to its runner, a radio task loops and yields and the switch
resumes it inside the same call. The join that looked impossible is a loop.

**Queues could not be mapped.** The driver's `create(capacity, item_size)`
returns a pointer and takes runtime sizes; the kernel's queues are
`[u64; SLOTS]` behind arena handles fixed at compile time. So the payload is
the adapter's, and the kernel supplies only the blocking. That is what forced
the heap, which the owner chose on 2026-09-11.

**This is the one Kairos cell that allocates on purpose.** The gate's
"allocates nothing (0 allocator symbols)" covers the kernel and the corpus
and does not cover this adapter.

### Also open

`RadioConfig::TICK_RATE_HZ` is 1000, so sub-millisecond sleeps round **up** —
early is a wrong answer, late is a slow one. Whether the radio tolerates it
is a hardware question. And the XIAO was off the serial bus, so nothing here
has been flashed.

## K5b — the seam runs on silicon, and finds a KERNEL assumption (2026-09-11)

`rusty_rtos_port/firmware/xiao-s3-radio` now boots the Kairos kernel on a
XIAO ESP32-S3 with a real port, creates tasks **through the driver's own
`task_create`** — C function pointer, heap stack — and switches into them.

**It reports FAIL, and the failure is the finding.**

| | |
|---|---|
| heap | `heap_usable=65536`, `rusty_rtos_alloc` |
| tasks created through the trait | two, at real heap addresses |
| `workers_entered` | **1** — a worker's first instruction DID execute on its heap stack |
| switches | `entries=5 swaps=2` — real context switches happened |
| laps | **0 and 0** — no worker completed a single hand-off |

So the port is sound (the `xiao-s3-switch` cell proves it separately, 100/99
resumptions) and the seam type-checks. What does not work is the join.

### The kernel commits the switch itself, and a stacked port cannot allow that

`Kernel::port_yield` is:

```rust
pub(crate) fn port_yield(&mut self) {
    self.enter_critical();
    self.port.count_yield();
    self.switch_context();   // <-- moves `self.current` HERE
    self.exit_critical();
}
```

On a **stackless** kernel that is correct and complete: changing which task
the runner steps next *is* the switch, because no task owns a stack. Every
architecture the corpus runs on relies on it.

On a **stacked** port it is a decision committed before it can be enacted.
`XtensaPort::yield_now` can only raise the switching interrupt; the CPU
swaps later, in the handler. Between the two, task code keeps running while
`Kernel::current` already names somebody else — so a blocking call made in
that window blocks **the wrong task**.

The instruments caught it saying exactly that:

```
RADIO after_create  current=3  ready=[1,1,1,0,2,0,0,0]   <- main is index 0
RADIO main_take0=false current=1 ready=[0,0,0,0,0,0,0,0] <- every list empty
```

`main` is index 0 and is the code on the CPU; the kernel's current is 3.
`main`'s `take` therefore parked task 3. Repeat, and every task is blocked
and no ready list has anything in it — a state the C kernel asserts against
(`configASSERT( uxTopPriority )`), reached here because the two notions of
"current" drifted.

### What this is NOT

Not the port: a worker entered and ran on its heap stack, and the switch cell
passes 100/99 from three frames deep. Not the adapter's trait
implementations: they compile and the semaphores are valid
(`a_ok=true b_ok=true done_ok=true`). Not the allocator.

### What it needs

A kernel-level decision, not a cell fix. For a stacked port the commit has
to happen **inside the switching exception** — the decision and the
enactment together, the way `mps2-an385-qemu-kernel` has `PendSV` call
`Kernel::switch_context` and read the slot in the same handler. Either
`port_yield` stops calling `switch_context` when the port is a stacked one,
or the port gets a way to say "defer the commit to me".

Until then the cell stands, reports FAIL, and says why. **The three other
board cells are unaffected and still pass** — corpus 18/18, signing, and the
switch — re-run on 2026-09-11 after the board came back.

## The commit point: a kernel/port contract fixed, and measured on both (2026-09-11)

**`Port::COMMITS_SWITCH` splits deciding a switch from committing it.** The
radio seam now runs on silicon — 50 and 50 hand-offs through the driver's own
traits, zero faults — and the ARM witness reports the window it was built to
find is closed.

### The defect

`Kernel::port_yield` called `switch_context`, which moves `current`. On a
**stackless** kernel that IS the switch: no task owns a stack, so changing
which task the runner steps next is the whole of it, and every architecture
the corpus runs on depends on it.

A **stacked** port cannot switch there. `yield_now` can only raise its
switching exception; the registers move later. Between the two, code runs as
a task the kernel has already moved on from — and a blocking call in that gap
parks the wrong task.

### Both ports were affected, and the numbers say so

| | before | after |
|---|---|---|
| **Xtensa** (`xiao-s3-radio`, silicon) | `laps 0/0`, every ready list empty | **50 and 50**, faults 0, 102 swaps |
| **ARMv7-M** (`mps2-an385-qemu-preempt`) | window opened **199** of 200 rounds, consumer 199/200 | window opened **0**, consumer **200/200** |

ARM was the surprise. The hypothesis was that ARM shared the defect latently;
the witness showed the window opening on essentially every round, with the
harmful sub-case — blocking inside it — landing once in 200 and being rescued
by the call's own timeout. **Rare is not safe**, and with the const set the
count is 0 by construction and the consumer stops losing its last token.

### The shape, and why it is that shape

A `const` on `Port`, not a runtime flag:

* **The corpus is the wedge**, so the stackless path had to be the code that
  runs today, not a re-derivation of it. A const monomorphises — the branch
  is not compiled — and **19/19 stayed byte-identical** through every step,
  which is the proof rather than the hope.
* **The port must not keep its own view of "current".** The first Xtensa
  attempt reconciled in the handler with a `RUNNING` static: a scheduling
  primitive re-implemented in a second place, which ADR 0003 is about. It is
  deleted. The exception calls `switch_context` once — decide and enact
  together.

### Law 3 arrived first, and paid immediately

`switch_context` had **four** silent `return`s. Now `Kernel::stalls()` counts
them and `first_stall()` names the first, and `tests/conformance.rs` asserts
zero across all 18 scenarios — an invariant that cannot move a trace byte,
since a stall is a path the corpus never takes. Poison-proven: a stall
recorded on every switch fails the test with `4702 time(s), first:
NoReadyTask`.

It was written first because the state it reports — every ready list empty,
kernel quietly carrying on — is what cost a multi-flash hardware hunt the day
before.

### Two ordering bugs the fix exposed

**No tick.** The radio cell had no timer, so `increment_tick` never ran and no
timeout could expire. Invisible until the fix, because blocking had not
previously worked at all; the first run afterwards hung. A 1 kHz systimer now
drives it.

**The slot must exist before the task can be scheduled.** `task_create`
registered its context *after* `create_task` returned — and `create_task` can
preempt, so the exception fired in between, found no context for the task the
scheduler had just chosen, declined, and reinstated the very drift the fix
removes. Creation and registration now happen under one interrupt mask.

### Poison-proof

Setting `COMMITS_SWITCH` back to `false` on Xtensa reproduces the original
failure exactly: `current=3`, `laps 0/0`, FAIL. The const is load-bearing and
the pass is its consequence.

### Gates after the change

`conform --all` 19/19 byte-identical · host tests 3/3 · three ARM QEMU cells
PASS · four board cells PASS (corpus 18/18, signing, switch, radio).

## The RISC-V port, and QEMU CAN supply a work counter (2026-09-11)

**`rusty_rtos_port-riscv` exists and switches**: 100 and 99 resumptions,
zero faults, from three call frames deep — `riscv32-qemu-switch`. That is
the third port, after Cortex-M and Xtensa, and it leaves the family with
every architecture the plan names except an untested C6.

### What RISC-V actually costs, and it is not what the plan assumed

The plan calls Xtensa the long pole because of register windows. RISC-V has
none, so by that measure it should be trivial. It is not trivial for a
different reason:

| | who saves the callee-saved registers |
|---|---|
| Cortex-M | hardware stacks eight; the port adds `r4-r11` |
| Xtensa | the exception entry spills the whole window file |
| **RISC-V** | **nobody — a trap saves NOTHING, so the port saves all fourteen** |

And a calling-convention trap that cost one hang: a fresh task must resume
through a **trampoline**, because the switch restores callee-saved registers
while a `extern "C"` entry reads its arguments from caller-saved `a0`/`a1`.

### The finding: `-icount` turns QEMU into a work counter

The mission plan says cycle rows come from silicon because *"QEMU can supply
no cycle, latency or work counter"* — measured six ways on the Cortex-M cell,
where DWT is unimplemented. **That reasoning is correct for ARM and does not
carry to RV32.** `mcycle` and `minstret` are architectural CSRs.

Measured rather than assumed, three consecutive runs each:

| | `minstret_total` |
|---|---|
| plain QEMU | 1,932,185 / 2,107,644 / 2,320,843 — a **20% spread** |
| **`-icount shift=0`** | 199,102 / 199,102 / 199,102 — **identical to the instruction** |

Plain, QEMU tracks host time and the counter is a number rather than a
measurement. Under `-icount` it is exactly reproducible, so the cell's runner
sets it and the cell is deterministic by construction.

**Stated narrowly**, because this is the kind of claim that gets over-quoted:
an emulator's cycle count is still not a chip's, and no absolute timing row
should come from here. What a deterministic retired-instruction count IS good
for is comparing two builds of the same workload — a **work count**, which
this family prefers to a clock anyway.

So K3 splits: **absolute cycle rows still need the C6**; comparative work
rows can come from RV32 QEMU today, reproducibly, with no hardware.

### The number reported is a ROUND TRIP

`per_round_trip instructions=995`. The first version labelled it "per switch"
and reported ~10,000 instructions for fourteen loads and fourteen stores — a
figure 250x too large, which is the instrument asking for help rather than a
slow switch. The bracket spans everything the other task does between the
switch out and the switch back. Measuring the bare swap needs the counter
read inside the assembly, and that is a separate job.

### Poison-proof

Deleting the twelve `s0`-`s11` stores makes the cell **hang** without
reaching a verdict — corrupting the callee-saved set destroys control flow,
so nothing survives to report a fault. A failure, and the gate treats it as
one, but a hang rather than a diagnosis. Recorded as such.

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
| Miri | green on `rusty_rtos_core`, `_kernel` and `_port` unit tests, and on all **sixteen** corpus scenarios | `cargo +nightly miri test --workspace` in each; the corpus run is `rusty_rtos_demo`'s determinism test, which drops to 20 ticks under `cfg!(miri)` so the interpreter can finish — 891.8 s for the sixteen, twice each (2026-09-09) |
| `cargo audit` on `rusty_rtos_core` | 0 advisories over 17 locked crates | advisory-db of 2026-09-09 (1243 advisories) |

## The arena-and-list cost row (2026-09-09, K1)

> **Superseded on 2026-09-10.** The row below is the measurement as first
> taken. It has since been cut to **34.22 instructions per operation,
> 1.533x**, by reading each node once per call; the second fix named at the
> foot of this section — end markers in the item array — was tried and is
> **refuted**. See *The topology work, measured* above. The method, the
> harness and the correctness gate described here are unchanged, which is
> the only reason the two numbers are comparable.

Mission plan §2.5 chose index-linked lists over pointer-linked ones so the
core could be `forbid(unsafe)`, and wrote down the price of being wrong: a
K1 ledger row above **1.25×** the C list cost reopens the decision. Here is
the row as first taken. **It was 2.08×, and the revisit condition fired.**
It is 1.533× now, which is still above 1.25×, so the decision stays open
and stays the owner's.

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


## K3 — the measurement half, two rows closed (2026-09-11)

K3's kill test has three clauses. This is what moved on the two that were not
blocked on hardware, and what the third is doing.

### The context switch, ours against theirs, on RV32

`bench/switch-cost/run.sh`. **A Kairos cooperative switch on RV32 is 30
retired instructions against FreeRTOS's 83** — 2.77× — with both arms counted
from their own toolchains and neither number typed in by hand.

| | save | restore | total |
|---|---:|---:|---:|
| FreeRTOS `portASM.S` (ecall trap) | 42 | 41 | **83** |
| Kairos `kairos_riscv_switch` | 14 | 16 | **30** |

The number is **retired instructions, not cycles**, and the plan's reason for
putting cycle rows on the C6 has not changed. What makes it exact rather than
sampled is that **both paths are straight line** — no loop, no data-dependent
branch — so the count in the block is the count retired by it. The script
checks that rather than assuming it: a conditional branch appearing where one
should not fails the run, because the moment one exists a static count stops
being a dynamic one.

**The 2.8× is structural, and naming the structure matters more than the
ratio.** `portYIELD()` on their RISC-V port is `ecall` (`portmacro.h:94`), so
a yield takes the same trap an interrupt does and saves everything an
interrupt could have clobbered: all 28 GPRs, `mstatus`, `mepc`, the
critical-nesting count. Kairos yields at a *call site*, where the C ABI has
already declared the caller-saved half dead, so its frame is `ra`, `sp` and
`s0`–`s11`. That is a different function, not the same function tuned — and
their design buys one handler serving both yields and interrupts.

**The bill comes due under preemption, and is stated rather than hidden.** A
preemptive Kairos switch also pays `riscv-rt`'s trap entry, 37 instructions,
so the arms converge to **67 against 83 — and 67 is a LOWER bound**, because
`_start_trap_rust` spills callee-saved registers of its own that this does not
count. The preemptive RV32 cell does not exist yet; when it does, that row
gets a measurement instead of a bound. So the claim is narrow on purpose: on a
cooperative yield — which is what `taskYIELD` and every blocking call do — our
port moves 2.8× less register traffic. Not "our context switch is 2.8× faster".

**Poison-proven on the instrument, not just the result.** Assembling the same
probe with `-D__riscv_32e` drops `x16`–`x31` from both macros and must remove
exactly sixteen instructions per side: 42 → 26 and 41 → 25, both exact. The
probe reads their code rather than reporting a constant.

Two method notes worth keeping. The oracle's macros are expanded **alone in
their own sections** (`bench/switch-cost/c/freertos_probe.S`) rather than
carved out of one of their four handlers by address range — a section boundary
is chosen by the assembler, an address range would be chosen by eye. And
FPU/VPU are off in the C arm because the Kairos port has no FPU save either;
charging the C arm for a feature we do not implement would not be a comparison.

### The RAM floor, ours against theirs, on RV32

`bench/kernel-ram/run.sh`. The useful output is not a total, it is that **the
two floors are different FUNCTIONS**:

```
FreeRTOS:  RAM = 1704 + tasks x (84 + stack + header)
                      + queues x (72 + storage) + timers x 40 + groups x 28
                 ^ static                        ^ all of it from the heap

Kairos:    RAM = arenas(TASKS, QUEUES, TIMERS, GROUPS, BUFFERS, BYTES)
                 ^ all of it static; the heap term is ZERO
```

FreeRTOS's static cost is 1,704 B and **does not move when a task is added**,
because the TCB comes from the heap at `xTaskCreate`. Kairos declares arenas,
so its static cost is the whole budget, every dimension moves it, and nothing
is left to allocate at run time. Which one is better depends on what is being
optimised, and saying so is the finding: a system that must know its worst
case at link time reads our column and is done; a system that creates a few
tasks and wants the smallest image reads theirs.

Per-unit, each a **slope between two geometries differing in exactly one
dimension** — never a total divided by a count:

| dimension | bytes | |
|---|---:|---|
| a task | 207 | TCB + its two list items, and **no stack** |
| a queue | 56 | |
| a timer | 88 | |
| an event group | 20 | |
| a stream buffer | 48 | |
| a notify slot | 8 | one `u64` |
| a buffer byte | 1 | one byte |

At the config's own `configMINIMAL_STACK_SIZE` of 128 words a C task costs
**596 B against our 207 B**, 2.9×. The gap is the stack: a blocking Kairos
call keeps its locals in the TCB (the `WaitFrame`), so a task can be suspended
mid-call without owning a stack — the same design fact that let the corpus run
on four architectures before any context-switch port existed.

**The counterweight is part of the row, not a footnote.** This is the *kernel*
cost. An application still needs somewhere for its own state: ours in a struct
sized to what it uses, theirs on a stack reserved for the worst case. The
saving comes from **exact sizing**, not from the state ceasing to exist — a
task whose state genuinely needs 512 B pays 512 B in either kernel. Quoting
2.9× without that sentence would be quoting a stack-sizing policy as a kernel
result.

Both arms are measured **on the target**, because `size_of` on the host is a
different number: a host `usize` is eight bytes and rv32's is four. The Rust
figures are the *lengths* of arrays in an rv32 staticlib, read back with
`llvm-nm -S`, so the size is the symbol's size and there is nothing to decode
and nothing to run. Two slopes are true by construction — a notification slot
is a `u64`, a stream-buffer byte is a byte — and the script checks both; they
are why the other five can be quoted.

**Flash is NOT compared, and the reason is a real blocker.** The C kernel's
five modules are 13,990 B of `.text` at `-Os`, but object-file `.text`
includes functions a linker would garbage-collect, and the two arms expose
different API surfaces. A flash row needs both arms linked from a matched root
set — which needs `rusty_rtos-capi` to stop being a scaffold, since it is the
crate that would present FreeRTOS's own symbol names over our kernel.

### The RISC-V port now PREEMPTS — the defect, the fix, and what it costs

`rusty_rtos_port/firmware/riscv32-qemu-preempt` was built to turn the switch
row's preemptive **lower bound** into a measurement. It first found a defect,
and then the defect was fixed, so this row ends with a number rather than a
blocker.

```
PREEMPT switches=201 want=200
PREEMPT work_a=224541 work_b=224515
PREEMPT faults=0
RESULT: PASS -- 201 preemptive switches between two tasks that
        never yield, 224541 and 224515 laps of work, zero faults.
```

Two tasks that never yield, sharing the hart almost exactly evenly, every
callee-saved register and stack witness intact across 201 switches.

**The defect.** `switch_context` resumes a task by restoring `ra` and
**returning** (`ret`). That is exactly right at a call site — it is what makes
a cooperative yield 30 instructions, and what `riscv32-qemu-switch` proves with
100/99 resumptions three frames deep. Inside a trap it is wrong: a trap is left
with `mret`, and a task entered any other way keeps running in trap context. On
the first run the cell reported `switched=1` and then silence: the first
preemptive switch was also the last.

**The fix**, deliberately kept as a *second* switch rather than a flag on the
first:

| | resumes by | swaps | instructions |
|---|---|---|---:|
| `switch_context` | `ret` | `ra`, `sp`, `s0`–`s11` | **30** |
| `switch_context_trap` | the task's own `mret` | the same fourteen plus `mepc` and `mstatus` | **37** |

plus `new_task_context_preemptive`, which builds a fresh task so its first
switch leaves through `mret`: `mepc` is the entry point, `mstatus` carries
`MPIE`, and the trampoline it returns to only moves the two arguments into
place and `mret`s. Keeping them apart means **a yield still pays only for what
a yield needs** — the cooperative row is unchanged and still pinned at 30, and
`riscv32-qemu-switch` still passes 100/99.

**Poison-proving refuted the first explanation, which is the part worth
keeping.** Three lines were removed one at a time:

| removed | result |
|---|---|
| the `mepc` restore | **hangs** |
| `mret` in the trampoline (→ `ret`) | **hangs** |
| the `mstatus` restore | **still passes** |

So `mepc` and the `mret` are load bearing, and the first write-up — which said
`mstatus` was "the field whose absence made preemption impossible" — **was
wrong**. The trap entry has already copied the live `MIE` into `MPIE`, so
`mret` re-enables interrupts without our help. `mstatus` is still saved and
restored, because a task preempted inside a critical section must come back
with that section in force; but this cell does not demonstrate that, and the
port's comment now says so instead of claiming a proof it does not have. Had
the poison run been skipped, a false mechanism would have shipped attached to a
true result.

**The measurement the cell was built for:**

```
    Kairos kairos_riscv_switch_trap      37
    + riscv-rt default_start_trap        37
    = a preemptive switch                74   vs FreeRTOS 83   1.12x
```

Straight line, no conditional branches, so a static count is a retired count —
and `bench/switch-cost` now pins it alongside the others.

**Both rows are true and they say different things.** A *yield* is 2.77×
cheaper on our side, because FreeRTOS routes yields through the interrupt trap
(`portYIELD()` is `ecall`) and we do not. A *preemptive switch* is **near
parity**, because there both kernels must write down what an interrupt could
have clobbered. Quoting only the first would be quoting the easy half — which
is exactly what the row said before the preemptive path existed to check it
against.

### The ARM control, which changes how the RISC-V switch row reads

The RISC-V row above says a Kairos cooperative switch is 2.77× cheaper. On its
own that reads as a win. **It is not one, and the Cortex-M3 arm is what shows
that** — added to `bench/switch-cost/run.sh` on 2026-09-11:

| Cortex-M3, both arms the PendSV handler | instructions |
|---|---:|
| FreeRTOS `xPortPendSVHandler` | **19** |
| Kairos `PendSV` | **19** |

Exact parity. On ARM both kernels reach the switch the same way — their
`portYIELD()` pends `PendSV` and so does ours — so the shapes match and so do
the counts. The RISC-V gap is therefore about **where a yield is taken**, not
about one kernel moving registers more cheaply: their `portYIELD()` is `ecall`,
which takes the interrupt trap and must save everything an interrupt could
have clobbered.

Read together, the two rows say: *the two kernels cost the same to switch; the
RISC-V port differs because FreeRTOS routes yields through a trap there and we
do not.* Read alone, the RISC-V row claims a win we did not earn, and that is
why the ARM arm was built.

Two method notes. Neither ARM arm is straight line, and they spend their guards
differently — ours has four `cbz` for the null-slot cases, theirs four
instructions of `basepri` masking, same count and different work. And the ARM
handlers are counted **up to `bx lr`**, because what follows in the block is
the literal pool and alignment padding; counting those read 21 where the answer
is 19.

**Counts do not compare across architectures** and the script says so: one
`stmdb` moves eight registers where RISC-V needs eight `sw`. Only
ours-against-theirs within an architecture means anything.

**Xtensa is absent, and the blocker is specific rather than assumed.** The
oracle does carry an Xtensa port
(`portable/ThirdParty/GCC/Xtensa_ESP32/portasm.S`) and the box does have
`xtensa-esp-elf-gcc`. What is missing is that the port is **not
self-contained**: `portasm.S` includes `sdkconfig.h` and `esp_idf_version.h`,
and its headers reach on into `soc/spinlock.h`, `esp_timer.h`, `hal/cpu_hal.h`
and `esp32s3/rom/ets_sys.h`. Those are ESP-IDF's, and `sdkconfig.h` is
generated. Stubbing them would be worse than not measuring, because the port's
`#if`s key off `CONFIG_FREERTOS_*` — a stubbed config emits *different*
assembly from a real build, and the number would be of our own construction.

### The flash half of the footprint row, and two instrument faults

`bench/kernel-flash/run.sh`. **Kairos is 1.15× the C kernel's flash** —
16,008 bytes against 13,924 (15,950 until 2026-09-11; see the note below), both on rv32 at `-Os`, both dead-code-eliminated
by `ld.lld --gc-sections` from the same root set. 2,026 bytes more.

That is a loss, and a modest one. It completes the K3 footprint clause, whose
RAM half is above.

**The root set is the corpus, and that is the whole argument.**
`docs/API-MAP.md` lists 341 FreeRTOS entry points, nearly all still `planned`
here. Linking the C kernel against all of them and Kairos against the forty it
implements would price a feature gap rather than a kernel. The operations
rooted are the ones the corpus exercises — the one place the two are known to
do the same work — and both arms name them with `-u`, so the roots are the
kernels' own symbols and neither arm is charged for a harness.

Where the C arm's 13,924 bytes are: `tasks.c` 6,408 · `queue.c` 3,032 ·
`stream_buffer.c` 1,474 · `timers.c` 1,122 · `event_groups.c` 778 · `heap_4.c`
646 · `portASM.S` 194 · `port.c` 144 · `list.c` 126. `heap_4.c` is included
because FreeRTOS genuinely needs a heap to create a task where Kairos
allocates from arenas inside the kernel; excluding it would charge Kairos for
a facility and let the C arm have it free.

**Two instrument faults, both caught, and they leaned opposite ways.**

1. **31,524 bytes, against us.** The first Rust probe called every operation
   from one function. The optimiser inlined most of the kernel into it, the map
   attributed **13,314 bytes to "the probe"**, and the total came out at 2.3×.
   One call site is not how a kernel is used. Giving each operation its own
   `extern "C"` entry point — which is what the C arm has — took the Rust arm
   from 31,524 to 16,556. *A number roughly twice what it should be is the
   instrument asking for help in whichever direction it leans*, and the
   temptation to bank the unflattering one and move on is the same failure as
   banking a flattering one.
2. **596 bytes, in our favour.** The C arm was first pinned with a table of
   function addresses, and that table's own code counted as kernel.
3. **150 bytes, against us.** The Rust root set globbed `^kairos_`, which also
   caught the port's own assembly symbols, while the C arm's roots pull in
   neither of *its* context switches. Invisible until the preemptive switch
   moved the total; fixing it took the pin **down**, 16,064 → 15,950. The
   switch is measured exactly and on both sides by `bench/switch-cost`.

**A tick-width asymmetry that could NOT be matched, so it is reported.** Kairos
uses a `u64` tick; the FreeRTOS RISC-V port does
`typedef portUBASE_TYPE TickType_t` (`portmacro.h:68`), so on rv32 the tick is
32-bit and **`configTICK_TYPE_WIDTH_IN_BITS` is not honoured by that port at
all**. Setting it to 64 was tried, because matching the configuration is what
these benches do: the type stayed `unsigned int` and clang reported
`eventUNBLOCKED_DUE_TO_BIT_SET` truncating to 0 — a broken build, not a matched
one. So part of the 2,026-byte gap is a tick that never wraps, including 492
bytes of `compiler_builtins` 64-bit helpers already excluded from the total.
`compiler_builtins` comes out because the C arm's `memcpy` and 64-bit helpers
are unresolved and uncounted too.

**Also corrected while matching configurations:** `INCLUDE_eTaskGetState`,
`INCLUDE_xTaskAbortDelay` and `INCLUDE_xTaskGetHandle` were off in the template
config though Kairos implements all three and the corpus exercises them. Turning
them on grows the C kernel's `tasks.c` from 7,134 to 7,786 bytes and leaves the
TCB at 84 — `ucDelayAborted` packs into padding that was already there — so it
moved the flash row and not the RAM one.

**Instrument check.** A linker that discards nothing measures nothing, so each
arm is linked twice, with and without `--gc-sections`, and the run fails if the
two agree. It removes 4,660 bytes from the C arm and 368,372 from the Rust one
(a staticlib carries every codegen unit until the linker prunes it).

### Four defects in the gate itself, found by running it (2026-09-11)

`kairos check --board` is how the hardware claims in this ledger are re-run
rather than re-read. It was run to confirm K5a still held after the day's port
changes, and it failed six times — none of them because the kernel was wrong.
All three have the same shape: **the gate named the code for something that was
not the code.**

| said | was |
|---|---|
| "a broken context switch presents exactly this way" | `Access is denied` on the serial port |
| "Cargo.lock is not the standalone one" | the previous gate run wrote it |
| "The cell HUNG" | the cell was working, and takes 196 s |

**It passes**: `check: 5 package(s) passed`, five board cells PASS, and the
three lockfiles intact both before and after — so a run no longer breaks the
next one.

**One flake is NOT resolved, and is recorded rather than waved past.** A later
run had `xiao-s3-signing` produce one of its four rounds and then emit
non-matching output until the 900-second hard stop. The same binary gives 8/8
arms and a PASS standalone, repeatedly, and gave 8/8 in the run that passed. So
the cell is sound and something about the gate context destabilises the longest
one — most likely board state after the previous cell's monitor is killed, but
that is a hypothesis and has not been tested. The next diagnostic is obvious:
timestamp each line the monitor receives, so the stall point is visible instead
of inferred.

**1. `stderr` was discarded, so a busy serial port was reported as a hung
cell.** `run_board_cell` spawned `espflash` with `stderr(Stdio::null())`. These
cells never exit — they print a verdict and spin — so the monitor has to be
killed, and on Windows the COM port is not reliably free by the time the next
cell asks for it. The next cell then got `Failed to open serial port ...
Access is denied`, the gate saw only silence, and it said:

> The cell HUNG, which is a failure and not a missing feature — **a broken
> context switch presents exactly this way.**

That sentence is well meant and, here, confidently wrong: it points at the
context switch when the fault is a serial port. It cost a session one wrong
statement about K5a before the pattern gave it away — **the hangs rotated**.
`xiao-s3-switch` hung in one run and passed in the next; both "hung" cells
passed when run singly. A cell that fails for a different reason each run is
not a broken cell.

Fixed: stderr is captured and quoted in every failure; a port-busy failure is
named as one instead of blamed on the cell; one retry after a settle; a short
delay after killing each monitor; and espflash exiting early is noticed rather
than waited out for the full 300 s.

**2. The lockfile restore ran too early, so the gate broke its own next run.**
`check` snapshots the standalone `Cargo.lock` before running cargo and restores
it after, because the umbrella's `[patch]` rewrites it (H-07).
`standalone_lockfile`'s doc promises *"the working tree is always left in the
state that should be committed"*. It was not: `--board`, `--qemu`, the no-alloc
rlib builds and `--soak` all run cargo **after** the restore, each rewriting the
lockfile the restore had just put back. So a `check --board` left a lockfile
that the **next** `check` reported as an H-07 failure.

Fixed with a second, idempotent restore at the end of the package iteration.

**3. A flat timeout priced duration when it meant to price silence.**
`BOARD_TIMEOUT` was 300 seconds for every cell. `xiao-s3-signing` legitimately
spends **196 of them in timed batches** — measured: nine batches, plus the
kernel-only phase, plus flashing and key setup — because it amortises 20,000
rounds, a 1 us clock being unable to resolve a microsecond any other way. So it
passed one run and missed the next, a coin flip on a cell that was working.

It was caught only because defect 1 had already been fixed: the message now
carried `espflash said: Flashing has completed!`, which rules out the port and
points at the cell's duration. Before that it read as another anonymous hang.

Raising the number would have been the wrong repair — it papers over the flake
and makes every genuine hang cost ten minutes. The gate's own word is *hung*,
and a cell printing `SIGN` lines throughout is not hung however long it runs.
So the deadline now measures **silence**: `BOARD_SILENCE` of 120 s, reset by any
output, with `BOARD_TIMEOUT` kept as a 900-second absolute stop. A genuinely
hung cell prints nothing and is caught in two minutes rather than five; a slow
one is never punished for being slow. The message says which of the two
happened.

**4. `conform` rewrote lockfiles and never restored them.** `check` has
snapshotted and restored around its cargo calls for a while.
`kairos conform` builds `kairos-sim` inside `rusty_rtos_demo` with the same
`[patch]` table in scope and did not, which was invisible because `conform` is
normally run alone. It surfaced when a `conform --all` run *between* two
`check --board` runs left the demo's lockfile rewritten, and the second check
reported an H-07 failure caused by the conform in between — the same
break-the-next-run shape as defect 2, in a different command. Fixed with the
same snapshot-and-restore, and verified: 5 git sources before, 5 after, working
tree clean.

**And one that was not the gate's fault, but is worth knowing.**
`standalone_lockfile` only snapshots when the lockfile is *already* clean, so
the protection exists only if you start clean. This session had damaged three
lockfiles with bench builds before running the gate, which is why the first run
flagged two of them. **The gate is self-protecting; it is not self-healing.**

The general lesson is the one this ledger keeps relearning from the other
direction: a gate that fails for environmental reasons trains people to ignore
it, and is worse than no gate. Every one of these failures was real in the
sense that something was wrong — and none of them was wrong in the thing the
message named.

### K3's cycle rows, measured on silicon (2026-09-11)

`rusty_rtos_kernel/firmware/xiao-s3-cycles`, on a XIAO ESP32-S3 over USB.
The clause names three rows — context switch, tick, latency — and these are
them, in **cycles on a real part**:

| row | cycles | at 240 MHz |
|---|---:|---:|
| tick, two ready tasks | **131** | 545 ns |
| tick, one ready + one delayed | **129** | 537 ns |
| switch | **623** | 2,595 ns |
| ISR-API wake → the task holds the value | **949** | 3,954 ns |

Stable across four consecutive flashes (tick 131 every time; switch 621–623).

**Why this can exist where the QEMU cells' cannot.** Xtensa's `ccount` is a
real cycle counter with **one cycle of resolution**. `esp_hal::time::Instant`
has one microsecond — 240 cycles, larger than every number above — which is why
the sibling `xiao-s3-signing` had to amortise 20,000 rounds to say anything.
The plan's reason for putting cycle rows on a part rather than QEMU is intact;
this is the part.

**The instrument measures itself first.** An empty `ccount` pair costs **1
cycle**, measured the same way and subtracted from every row. A row that did
not exceed that tax would be reported as *below resolution* rather than given a
number. Medians with min and max, never means — a chip's interrupts add time
and never remove it, and the `max` column is exactly those: 3,901 on the idle
tick against 201 on the delayed one, same work, fewer interruptions landing in
the window.

**The refuted check is the most interesting line in the cell.** It originally
asserted `tick_delayed >= tick`, on the obvious reasoning that a non-empty
delayed list can only add work. The board said **129 against 131** — the
assumption was the thing that was wrong, so the check was replaced and the
number kept. Blocking a task takes it *off the ready list*, so the tick stops
making a time-slice round-robin decision between two runnable tasks at one
priority; that saving exceeds the cost of looking at one delayed entry whose
wake time is far away. The two figures are therefore not "idle vs loaded" but
"two ready tasks" vs "one ready and one delayed", and ready-list population
dominates.

**A cross-check against the sibling cell — and on 2026-09-21 it turned out
not to be one.** `xiao-s3-signing` measured a scheduling round — queue send,
receive, two switches — at **1,995 cycles**, against these rows' prediction of
`2 x 623 + (949 - 623) ~ 1,570`: same order, ~20% apart, which read as two
instruments agreeing.

**They were two measurements of the same defect.** The sibling hand-rolls the
same shadowed `NoTrace`. Both were corrected and re-run on the board: the
round is now **893 cycles** and the prediction is
`2 x 166 + (430 - 166) ~ 596`. Two instruments agreeing is worth as much as
either alone ONLY when they are independent, and a common-mode defect makes
them one instrument wearing two hats.

**What is still open, and it is half the clause.** K3 asks for these rows
*against the C demo*, and there is **no C arm**. Building FreeRTOS for the S3
needs the ESP-IDF header tree and a generated `sdkconfig.h`, and its Xtensa
port's `#if`s key off `CONFIG_FREERTOS_*` — a stubbed build would not be the
kernel anyone runs, so the comparison number would be of our own construction.
The same wall blocks the Xtensa arm of `bench/switch-cost`. These are our
numbers on silicon; the comparison is not done.

**The ISR row is deliberately named "ISR-API", and the qualification is load
bearing.** No interrupt is taken: `queue_send_from_isr` is the API an ISR would
call, invoked inline. So the row is the *kernel's share* of a wake and excludes
the vector entry and exit a real interrupt pays either side of it. The clause
names "latency"; this is not the whole of it, and the name says so rather than
letting the number be quoted as something it is not.

Also not claimed: a register-file swap. Kairos tasks are stackless, so `switch`
here is the scheduler choosing and committing the next task. The register cost
is `bench/switch-cost`'s row — 30 instructions on RV32, 19 on ARM.

### The packaging defect bit again, and this time a compiler caught it

The first build of the RAM probe failed on `Port::COMMITS_SWITCH` — a constant
that exists in this working tree and not in the published `rusty_rtos_core`.
The cell named its siblings by path, but `rusty_rtos_kernel-core` reaches
`rusty_rtos_core` by **git URL**, so cargo built *both*: the local one for the
cell and the published one for the kernel. This is the same hazard the plan
already records for `firmware/*` (`kairos patches` scans only `crates/*`), and
it is worth noting that here it surfaced as a **compile error only because
this session had just added a constant**. Had the local edits been to a
function body rather than a trait surface, it would have built green against
the wrong code and every number above would have been wrong.
`bench/kernel-ram/rs/.cargo/config.toml` is committed for exactly that reason.

### Clause 1, the one-hour soak: the host closes it, the emulators are slow, and one claim was withdrawn on the way

An earlier version of this section claimed the **emulator** hour was 18/18.
**That claim was wrong and is withdrawn.** It rested on `death` passing at
3,600,000 ticks plus a 17/17 row this session had not re-run. Re-running is
what found the problem, and then what explained it.

**The host closes the clause outright.** `cargo test -p rusty_rtos_demo-core
--test soak -- --ignored`, 2026-09-11: **18/18 at 3,600,000 ticks**, every
scenario's check task still reporting running, `ticks >= 3,600,000` asserted so
a scenario that stopped early cannot pass.

**The emulators are not failing; they are slow, and `semtest` is where the time
is.** The host run makes that unmissable:

| | host time at 3,600,000 ticks |
|---|---:|
| `semtest` | **106.2 s** |
| every other scenario, together | 30.2 s |
| total | 136.4 s |

`semtest` is **78% of the corpus's work at this length**, and the reason is
structural and already documented in the soak test's own notes: it runs **345
state-machine steps per unit of sim time** against a corpus average of 6.3,
because `semtest.c`'s guarded loop counts to `0xfff` between one semaphore take
and the next. On an emulator that is tens of minutes to hours by itself.

So the three full-corpus emulator runs that "stopped at `semtest`" were stopped
**in** it, not **by** it. What was read as a failure at scenario four was the
scenario that takes longer than the other seventeen combined, reached in order.

What is established on the emulators, all 2026-09-11:

| | result |
|---|---|
| `death` at 3,600,000 ticks, Cortex-M3 | `ok ticks=3600057 yields=57580 exits=3533911 lines=3973943` |
| `death` at 3,600,000 ticks, RV32 | **identical, field for field** |
| `semtest` at 100,000 / 400,000 ticks, RV32 | ok, counters scaling linearly |
| `semtest` at 1,600,000 ticks, RV32 | still running past 25 minutes |
| `semtest` at 3,600,000 ticks, RV32 | no verdict inside the time a session can hold a process |
| the pinned 2,000-tick gate, both cells | 18/18, unaffected throughout |

**What stays open, stated narrowly.** The full eighteen at 3,600,000 ticks has
been *observed* end to end on the host and not on an emulator. That is a
duration problem rather than a correctness one — but it is not the same
sentence as "the emulator hour passes", and it will not be written as one
until a run finishes. The plan already classed these as background runs rather
than gates, at hours per architecture; this session measured what "hours"
means and it is mostly one scenario.

**The record that prompted the withdrawal.** A previous session recorded 17/17
at 3,600,000 ticks per emulator. Nothing here contradicts it — that run
evidently had the wall-clock this one did not — but it could not be reproduced
inside a session, so the claim it supports should carry that caveat.

**Tooling that came out of it.** Both corpus cells now take two compile-time
overrides, and the second exists only because the first was not enough to
diagnose anything:

* `KAIROS_SOAK_ONLY=<name>` runs one scenario instead of eighteen. Re-running
  seventeen that pass to reach one that has not been tried is waste; this made
  `death` a 24-second command and isolated `semtest` in a single run. A name
  not in the table FAILs and exits 1, because a filter matching nothing
  otherwise produces a run with zero failures — the shape of a gate that tests
  nothing.
* `KAIROS_SOAK_TICKS=<n>` moves the length. The soak asks one question — is
  this still running after an hour — and when no answer comes back, that
  question gives no purchase on *why*. A length that can be moved turns silence
  into a bisection, which is how the 100k/400k/1.6M points above exist, and how
  "`semtest` is broken" was ruled out before it reached a ledger row as a fact.

**And the hour now exists on SILICON, which the clause asks for and the C6 was
only ever one way to get.** `rusty_rtos_demo/firmware/xiao-s3-corpus`, run live
on a XIAO ESP32-S3 on 2026-09-11:

| on the S3, over USB, this session | result |
|---|---|
| the pinned corpus | **18/18 byte-identical to the C kernel**, `death` included |
| `death` at 3,600,000 ticks | `ok ticks=3600057 yields=57580 exits=3533911 lines=3973943 bytes=139919939` |

That second row is **identical field for field to the Cortex-M3 and RV32 runs**
— so three architectures, one of them a real part rather than an emulator,
agree on every counter and the FNV-1a/64 digest at **1,800× the pinned length**.

It is worth being exact about what that does and does not settle. The clause
names a C6, and no C6 is in hand; this is an S3. But the thing the C6 was in
the row *for* — an hour of the corpus on silicon rather than under emulation —
is now measured, on the part that is actually on the desk. What a C6 would add
is a second chip and a second architecture-of-record, not the first silicon
hour. The S3 cell also gained `KAIROS_SOAK_ONLY` and `KAIROS_SOAK_TICKS`, so
re-running one scenario's hour there is a single command rather than an
afternoon.

**And the emulator hour is now CLOSED on RV32 — all 18, measured end to end.**
`bench/soak-each/run.sh riscv32-qemu-corpus`, 2026-09-11: **18/18 at 3,600,000
ticks, in 58 minutes**, every scenario's check task still reporting running.

The split is the whole trick, and it costs nothing: the cell builds a **fresh
kernel per scenario** (`Runner::kernel_for` inside the loop), so eighteen runs
of one scenario is the same computation as one run of eighteen, in the same
order, at the same lengths. Nothing is shared across iterations for splitting
them to lose. What it costs is a rebuild per scenario — about twenty seconds,
against the forty-six minutes `semtest` alone takes.

And the timing confirms the diagnosis exactly:

| | of the 58-minute run |
|---|---:|
| `semtest` | **2,769 s (46 min) — 79%** |
| the other seventeen together | 719 s |

The host had put `semtest` at 78% of its own 136-second run. **79% against 78%,
two machines three orders of magnitude apart in speed** — which is what a
structural cost looks like, and it is why the three earlier full-corpus runs
appeared to "stop at scenario four". They were stopped *in* `semtest`, not by
it.

**And Cortex-M3 followed**: `bench/soak-each/run.sh mps2-an385-qemu-corpus`, **18/18 at 3,600,000 ticks in 75 minutes**. So the emulator hour is closed on BOTH architectures, and with the host and the S3's `death` row it is the same answer on four machines.

**Still blocked regardless:** the C6 third of the clause, on the same hardware
B4b needs.



## v0.1.0 — the first crates.io release (2026-09-16)

**`rusty_rtos_core` 0.1.0 is published, and its repository is public.** 22
files, 41.9 KiB compressed. It is the bottom of the stack — no dependencies at
all — which is why it went first and why nothing above it could go before it.

### What the release actually required, and none of it was code

| blocker | what it was | how it was closed |
|---|---|---|
| **24 git-sourced dependencies** | the umbrella's development pattern is git URLs plus a `[patch]` table. **`cargo publish` refuses a git dependency outright**, and §2.11 says "released pins only" for the same reason | every sibling now names a version only; `kairos patches` emits ONE `[patch.crates-io]` table instead of one per git URL |
| **the names were unclaimed** | proven, not assumed: a dry run answered `no matching package named rusty_rtos_core found — location searched: crates.io index` | `rusty_rtos_core` is now taken; the rest are queued |
| **licence texts were not in the packages** | the `license` field was set but `LICENSE-MIT` / `LICENSE-APACHE` live at each repo root, outside the crate directory cargo packages | copied into all 20 publishable crate directories; the package went 20 files to 22 |
| **four crates had no README** | the port backends — `-cortex-m`, `-riscv`, `-xtensa`, `-host` — each publish separately and each would have shipped a blank crates.io page | written, and `readme` / `documentation` declared in their manifests |
| **relative documentation links** | `[docs/LEDGER.md](docs/LEDGER.md)` resolves against **crates.io**, not GitHub, so every doc link on the crate page would have 404'd | 17 links across eight READMEs made absolute |

### The H-07 lockfile artifact is gone, structurally

Every standalone `Cargo.lock` now carries **zero** `source = "git+..."` lines
and resolves `rusty_rtos_core` from the registry, with a checksum. The gate row
that fired on every run for weeks fired because a lockfile written with the
umbrella's `[patch]` in scope recorded git sources a fresh clone could not use.
With no git dependencies there is nothing for the patch to rewrite that way.

Worth noting the ordering this forced: a dependent's standalone lockfile could
not be generated **until `rusty_rtos_core` was actually on crates.io**, because
a version-only dependency has nowhere else to resolve from. Publish order here
is not a preference, it is the only order that works.

### The repo is public, and §2.11 scored honestly

`Remade-With-Rust/rusty_rtos_core` flipped **PUBLIC** on 2026-09-16, after the
release work was committed and tagged `v0.1.0` — in that order, because the
published tarball was AHEAD of the repo. Publishing from a working tree with
`--allow-dirty` puts source in the crate that GitHub does not have, and a public
repo that disagrees with the crate it claims to be the source of is worse than
a private one.

**The flip added no exposure.** The source was already on crates.io, which is
permanent and world-readable; what it fixed was three dead links — the crate
page's Repository link, and every `docs/` link in the README, all of which are
absolute by necessity and all of which were 404ing. Verified after the flip:
`docs/LEDGER.md`, `docs/plans/rusty_rtos_core.md` and
`docs/plans/use-protection-please.md` all resolve.

Scored against §2.11's own bar rather than waved through:

| condition | |
|---|---|
| siblings public first | ✓ core is first; nothing precedes it |
| released pins only, no `git =` | ✓ |
| emulator and board half measured | ✓ through the corpus above it |
| `cargo deny` green in CI | ✓ |
| **CI green without a token** | ✓ **closed by this release** — the `KAIROS_GIT_TOKEN` rewrite existed to fetch private siblings by git URL, and there is nothing private to fetch now |
| README equals the plan, dual licence, FreeRTOS credit | ✓ |
| version tag in the flipping commit | ✓ `v0.1.0` |
| crates.io name reserved | ✓ |
| **hardening row complete** | ✗ **50%**, waived for 0.x with a reason per gate |
| **`release-plz` and `portfolio-check` in the flipping commit** | ✗ neither exists yet |

Two conditions unmet, both recorded rather than quietly skipped. The bar in
§2.11 is written for **1.0.0**; this is 0.1.0, and 0.x is the version number
that tells a consumer the API is not stable. That is a reason, not an excuse,
and those two rows are what has to close before the next flip.

### H-07 had become a check that could only fail

Worth its own note, because it is the failure mode a gate is most likely to
have and least likely to admit to. `standalone_lockfile` decided a lockfile was
the standalone one by checking it **contained**
`git+https://github.com/<org>/<sibling>`. That was correct while the siblings
were git dependencies. Removing those — which `cargo publish` requires — made
the string impossible, so the function answered `None` every time and the gate
reported all six dependent packages as broken on every run.

A gate that always fails is indistinguishable from a gate that is telling you
something, and this one had been shouting for weeks.

The property it should test never changed, only its signature. A standalone
lockfile is one a **fresh clone** can use, which means every sibling resolves
from the registry; under `[patch.crates-io]` a sibling is a path override, and
cargo records a path dependency as a `[[package]]` with **no `source` line at
all**. So the tell is a sibling entry carrying no source, and that tell is the
same whichever way the dependency is written.

With it fixed, `kairos check --fmt --clippy --test --deny` reports
**12 package(s) passed** — green end to end, which it had not been for weeks.

### The READMEs were rebuilt on the `rusty_mp3` architecture

All eight repository READMEs and eight inner-crate READMEs now share one
section order, taken from [`rusty_mp3`](https://crates.io/crates/rusty_mp3):
badges, a positioning paragraph, the halves **with their known gaps stated in
the opening rather than buried**, an evidence section with a regeneration
command, a runnable example, performance **with its method**, a portability
matrix, the family, the org boilerplate between markers, and the licence.

Two things that pass for detail and are not. The reference states the
measurement method beside every number — pinned, CPU time, ABBA-interleaved, N
pairs, the null-arm floor — and quotes **per-item** tables rather than a mean,
"because a mean hides the thing that matters". Both are carried over: the
list-cost row quotes its callgrind slope and the checksum both arms print, and
the switch-cost rows quote the ARM control (19 vs 19, exact parity) that makes
the RISC-V number readable rather than flattering.

The stale status sections went with it. The kernel's README had said "K2 in
progress — 14 of its 18 scenarios agree" and the port's had said the silicon
ports were future work; both were months out of date and would have been the
first thing a reader saw.

### Hardening: 42% to 50%, and the waivers are on the record

| gate | closed by |
|---|---|
| H-11 unsafe inventory | `cargo geiger`: **0/0** across the whole tree, reported `:)` — no `unsafe`, `forbid(unsafe_code)` declared. The compiler enforces it, which is stronger than the survey |
| H-12 SBOM | CycloneDX per published crate, in `sbom/` — deliberately NOT inside the crate directories, where an SBOM is stale the moment a dependency moves |
| H-13 git deps pinned | there are none left; `deny.toml`'s `allow-git` list is now empty, and the file's own comment says an allowance nothing uses is a warning on every run |
| H-14 dependency freshness | `dependabot.yml`, weekly, PR-only, siblings ignored because a sibling's version is decided by a release rather than a bot |

The rest are **waived for 0.x with a reason each**, in every plan's "v0.1.0
release decision" section — threat model, `cargo vet`, fuzzing beyond the
no-panic gate, and formal verification. The distinction that section draws: an
Incomplete gate listed there is a decision, one not listed is an omission.

### Still to publish, and one thing that has to be settled first

`rusty_rtos_alloc`, then the kernel, port and heap crates — each dry-run clean
(`tools/publish-0.1.0.sh`, dry run by default) and each blocked only on running
the command. **Three of the four port backends (`-host`, `-riscv`, `-xtensa`)
are UNTRACKED in git**: they exist on one machine and have never been
committed. They cannot be published from a state nobody else can reproduce,
and that is a release blocker rather than a tidiness one.

Held back deliberately: `rusty_rtos-capi` until its seam is a library crate
rather than a `#[path]` include (today the publishable crate is a 15-line
`lib.rs` and the 87 symbols are not in it), and `rusty_rtos_json` /
`rusty_rtos_backoff` until they have code — both are 27-line scaffolds, and
publishing them would claim a name with nothing behind it.

## K6 — the C ABI, first cells (2026-09-11)

`rusty_rtos-capi/firmware/mps2-an385-qemu-capi` links **unmodified C demo
task files** from the pinned `oracle/` checkout against the Kairos kernel,
through `extern "C"` symbols, on QEMU `mps2-an385`.

Nothing about the C is patched, wrapped or regenerated. It is compiled by
`build.rs` straight out of `oracle/FreeRTOS/FreeRTOS/Demo/Common/Minimal/`
against the oracle's own `FreeRTOS.h`, `task.h` and `queue.h`. The only file
this cell hands the C side is a `FreeRTOSConfig.h`, which every FreeRTOS
application supplies.

### The inversion

Those files are the corpus's **oracle**. Nineteen of them were remade as
Rust state machines and diffed line-by-line against their traces on four
architectures. Here the same sources are the **client**: we proved we behave
like them, and now they run on us.

### The specification was derived, not chosen

The demo files were compiled unmodified and `llvm-nm -u` was asked what they
wanted. That list **is** the requirement, and it is much smaller than the API
map suggests:

| | |
|---|---:|
| `docs/API-MAP.md` entry points | 341 |
| union of undefined symbols across 33 demo files | **108** |
| of those, `__aeabi_*` compiler float helpers | 11 |
| board drivers (`vParTestSetLED`, the serial four) | 6 |
| **the actual kernel surface** | **~84** |

`PollQ.c` alone needs eight. Ordering the files by how many NEW symbols each
one costs turned "implement 341 things" into a sequence where the first
cell needed eight and the next six needed seven more — and eight demo files
need **zero** beyond what `PollQ` already required.

**33 of 34 demo files compile.** The one that does not is `IntQueue.c`,
which includes a board-specific `IntQueueTimer.h` that each demo *project*
supplies — not an ABI gap.

**Update 2026-09-12: the six board drivers in that table are now supplied,
and they are still not ABI.** `seam/board.rs` implements
`vParTestInitialise`/`SetLED`/`ToggleLED` and the serial four as a demo
*project* would, with the serial port as a software LOOPBACK because
`comtest.c` asks for one in words. They are deliberately absent from
`symbols.rs` and from the generated header — FreeRTOS does not export them —
so `seam/board_gate.c` audits their signatures against the oracle's own
`partest.h` and `serial.h` instead, the same trick the header gate uses.

That table's last row also moved, downward, for a good reason: the kernel
surface is now **87** rather than 89, because `strcmp` and `strncmp` were
being claimed as ABI when they exist only because a bare-metal cell has no
libc. The deriver stops at the seam's own `tiny libc` banner, which also
keeps a three-argument `sprintf` stand-in out of a header that would then
conflict with the real `<stdio.h>`.

The run itself is in `rusty_rtos-capi/docs/LEDGER.md`: **26 demo files**, 25
of them reporting a verdict of their own, on QEMU M3 and on a host under
Windows threads and Linux pthreads.

### A C task can block because the port gives it a stack

The Kairos kernel is stackless: a blocking call returns `Wait::Blocked`,
meaning "call again when this task next runs". A C task cannot do that —
`vTaskDelay` must return *later*, with its locals intact.

It gets a real stack from `rusty_rtos_port-cortex-m`, and `PendSV` asks
`Kernel::switch_context` who is next. That is the same joint
`mps2-an385-qemu-kernel` proves; this cell puts a C function on top of it.
`Wait::Blocked` then becomes a retry loop around a `pend_switch`, which is
exactly what the enum's own doc comment describes.

`pvPortMalloc` and `vPortFree` are **K4's `heap_4` remake**, not a new
allocator: the one already diffed against `heap_4.c` over 20,000 operations.
That needed one small addition to `Heap4` — `address_of` / `offset_of`,
bounds-checked both ways — which is the offset-to-pointer "binding" its own
module docs anticipated but had left to whoever owned the storage.

### Four defects, and the C found all four

The `configASSERT` hook is wired to print and exit rather than being
compiled out, on the reasoning that **a demo tripping one is the C telling
us the ABI lied to it**, and that is the most valuable thing this cell can
produce.

1. **A null handle encoded as a non-null pointer.** Handles cross the seam
   as `to_raw() + 1`, so that a valid handle is never NULL. A *null* kernel
   handle then came out as `0 + 1 = 1`. `GenQTest.c:564` checks
   `xSemaphoreGetMutexHolderFromISR( xMutex ) != NULL` after giving a mutex
   back; an unheld mutex answering `1` failed its assertion. The `+ 1` must
   keep valid handles non-null without manufacturing one from the absence of
   a handle.
2. **A discarded return value.** `Kernel::notify` answers `bool` —
   with `eSetValueWithoutOverwrite` a notification that is already pending
   makes it fail. The seam returned `pdPASS` for `Ok(_)`, so a notification
   that never landed reported success. `TaskNotify.c:204` asserts precisely
   that case.
3. **An operation silently turned into a different one.** `xQueueSend`,
   `xQueueSendToFront` and `xQueueOverwrite` are all macros over
   `xQueueGenericSend`, distinguished by `xCopyPosition`. Handling two of
   the three sent every overwrite to the back of the queue instead, with no
   error anywhere. Both the task and ISR forms had the hole.
4. **The timer daemon starved the demos.** Parked in a spin loop at
   `TIMER_TASK_PRIORITY`, it starved every task below it: of nineteen tasks
   across six demo files, only the two at its own priority ever ran. This is
   the **same shape** as the `xiao-s3-signing` finding already in this
   ledger, where the scheduled arm completed zero operations while reporting
   a batch 75x faster than bare.

### And one that was not a defect at all

Six demo files passed **individually** and four of six failed **together**,
with context switches collapsing from 3,057 to 121. That is correct
fixed-priority scheduling punishing a bad priority assignment: tasks that
never block starve everything beneath them.

The priorities were mine, invented. `Demo/Posix_GCC/main_full.c` has the
authoritative ones — `PollQ` and `semtest` at `tskIDLE_PRIORITY + 1`,
`BlockQ` at `+ 2`, `integer` at idle, the check task at
`configMAX_PRIORITIES - 2`. Using theirs fixed it outright. These files are
designed to run together at *those* numbers.

Separating that from an ABI bug needed a per-demo filter
(`KAIROS_CAPI_ONLY=<name>`), for the same reason `KAIROS_SOAK_ONLY` exists:
individually-pass/together-fail is a different diagnosis from a broken
symbol, and without the filter the two are indistinguishable.

### Two sizing traps, both silent by construction

Both presented as the demos doing nothing, and neither raised an error:

* **The queue slot pool.** `PollQ` asks for a queue of ten; the pool was
  eight. `xQueueCreate` returned NULL and `PollQ.c` skipped its task
  creation inside `if( xPolledQueue != NULL )` — which is correct FreeRTOS
  behaviour and terrible to debug. The seam now says out loud when the
  kernel refuses a queue.
* **The stream-buffer arena** was `1` buffer of `8` bytes, a placeholder
  from when only `PollQ` was linked, and silently refused every
  `xStreamBufferGenericCreate` once the buffer demos arrived.

### Status

Not finished, and the remaining work has changed character: the surface is
essentially complete, and what is left is **semantic conformance**, one C
assertion at a time, in edge cases the corpus's nineteen scenarios do not
reach. Several of those are likely to be kernel findings rather than seam
findings, which is a good outcome and a different kind of work.

## Tickless idle — two architectures, and two defects only hardware found (2026-09-19)

Opt-in and **off unless asked for**: a build that does not set
`Config::USE_TICKLESS_IDLE` passes the 19-scenario differential byte for byte.

### The prize, sized before anything was built

| fact | value | method |
|---|---|---|
| sleepable share of the corpus | **50.3 %** of 38,011 ticks; idle 63.6 % | `kairos power idle`, over the 18 pinned oracle traces |
| and it is **bimodal**, which is the finding | 9 scenarios 65–99.9 % sleepable, 9 scenarios **0 %** | they idle up to 52.6 % but never for two consecutive ticks. Tickless is a property of the workload, not a tuning knob |
| the projection is invariant | **18 of 18** | `kairos power diff`: strip every heartbeat line, project again, compare. Demonstrated, not asserted |

### The mechanism, on QEMU and on silicon

| fact | value | method |
|---|---|---|
| ARMv7-M, QEMU | **401 SysTick interrupts -> 0**, digest `94f229d7a5bd8f77` both arms | `cargo run --release` and `--features tickless` in `rusty_rtos_port/firmware/mps2-an385-qemu-tickless`. One pinned digest serves both arms, so a single run is a kill test |
| **XIAO ESP32-S3, silicon** | **400 alarm interrupts -> 0**, digest `ebb908b74bccb99e` both arms | same two commands in `firmware/xiao-s3-tickless`, flashed over `espflash`. Board: rev v0.2, 8 MB flash, 40 MHz crystal, MAC 68:ee:8f:51:74:64 |
| and the kernel runs from a tick on Xtensa at all | 400 ticks, 63 switches, 0 stalls | the control arm of that cell. A repo first: every other kernel-driving cell is Cortex-M under emulation |
| poison | deleting the pended-last-tick line in `Kernel::step_tick` gives **421 ticks** — every lap one late — with the digest **unchanged** | the order digest cannot see a wake that is in the right sequence but late. That is why there is also a tick band, and the band is what fails |

### The energy question, answered negative (M3c)

Measured against a **fair** baseline — a control that halts the core in
`waiti`, as FreeRTOS's idle task does. An earlier version busy-spun, which is
valid for counting wakeups and useless for counting current.

| fact | value | method |
|---|---|---|
| core duty cycle | control **0.6 %** · tickless **0.9 %** | active 2,506 us vs 3,722 us over a ~399,600 us run; time-halted accumulated off the free-running SYSTIMER. Three runs each way, reproducing to ~2 us |
| so tickless costs ~48 % **more** core-active time | and 400 wakeups still became 0 | a `waiti` control is already **99.4 % halted**, so 0.6 % is the ceiling for any idle optimisation on this workload. Each sleep replaces ~20 interrupts at ~6.6 us with one suspend/reprogram/restore cycle at ~182 us |
| verdict | **tickless is a lever on sleep DEPTH, not sleep COUNT** | a fitted sleep-length policy is pruned on arithmetic: no policy beats a 0.6 % ceiling. The next brick is `Rtc::sleep_light` |

### Two defects, neither findable by building

| defect | size | how it was found |
|---|---|---|
| **`waiti 0` destroys the caller's critical section** | the scheduler silently stopped working: **20 logical ticks where the arithmetic says 400** | it SETS `PS.INTLEVEL` to zero and leaves it there — the wake returns through `RFI`, restoring the PS `waiti` installed. Everything after the sleep ran with interrupts open on a kernel the idle task held `&mut` to. ARM's `wfi` leaves PRIMASK alone, so this is Xtensa-only. Fix: re-raise the mask the instant `waiti` returns |
| **the tickless clock ran 0.99 % slow** | **38.7 seconds in an hour** | the tick was a free-running 1 ms period and the sleep restarted it *at the wake*, discarding whatever fraction had elapsed while the worker ran (~180 us a lap), compounding. 91 % of the 4,294 us wall gap is exactly the measured active time; a simulation of the old logic predicts +0.965 % against +0.99 % measured. Fix: drive the tick as a one-shot on an **absolute grid**. After: a 16 us gap, 0.0 % drift |

**Why no test caught the second one, which is worth more than the bug.** Every
check counted **logical** ticks, and both arms produce exactly 400 of those
however badly the timer is driven. The schedule digest matched too, because no
event was ever out of order.

> **A gate that only compares the system to itself cannot catch the system's
> shared reference drifting.**

Both arms agreed with each other and both were wrong about the wall clock.
There is now a check on wall-time-per-logical-tick against the free-running
counter, bounded at 0.3 % — the only check in that cell comparing the kernel to
anything outside itself. The hazard was already written down, in prose, in this
repository, on the other port: the ARM cell implements boundary-sleeping
deliberately and says why. It was simply not carried across. **A law the
tooling does not enforce is a law you will break again.**

## The instruments, measured against each other (2026-09-19)

A campaign that had hammered `kernel-ir` to a standstill went looking for what
was left. What it found was not a seam — it was that **the instrument used to
gate kernel work barely measures the kernel.**

### Every instruction counter in the tree, and what it is really counting

| instrument | Ir | Ir/call | what dominates it |
|---|---:|---:|---|
| `kernel-ir` | 119,462,960 | 3,143/tick | **the harness** — see below |
| `search-ir` | 21,361,060 | 356 | the query walk (halved this session) |
| `json-ir` | 15,424,053 | 243 | validation |
| `mqtt-ir` | 12,998,136 | 81 | topic matching — **at its floor** |
| `khot-ir` | 8,212,073 | 86/op | **the kernel, cleanly** |
| `hdr-ir` | 7,450,102 | 11 | 68% harness (found earlier) |
| `kobj-ir` · `kipc-ir` · `ksched-ir` | 9,112,873 | — | the kernel, cleanly |
| `heap4-ir` · `deser-ir` · `sntp-ir` | 4,569,552 | — | — |

### ★ `kernel-ir` is 46% trace formatter and about 7% kernel

| component of `kernel-ir` | Ir | share |
|---|---:|---:|
| `LineTrace<Digest>::event` (6 census rows) | 55,482,249 | **46.4%** |
| `str::from_utf8` — 97,547 calls | 6,162,016 | 5.2% |
| the runner | 4,746,959 | 4.0% |
| `memcpy` — 102,689 calls | 3,109,775 | 2.6% |
| **the kernel** (`switch_context`, `increment_tick`, `queue`, `kernel.rs`) | **8,222,510** | **6.9%** |

**A 10% kernel improvement moves `kernel-ir` by 0.7%**, which is inside the
layout noise that a rebuild produces. That is why the vein felt exhausted: the
earlier kernel campaign was conducted through a lens that divided its results
by roughly fourteen.

`khot-ir` is the contrast and the cure. It uses `CountTrace`, its harness is
**2.66%** of its total, and the rest is kernel and port. The four `k*-ir`
instruments were already built that way; nothing had compared them to
`kernel-ir` to notice the difference.

**Standing rule from this:** `kernel-ir` is a REGRESSION GATE — it proves a
change did not move the corpus — and is not an optimisation instrument. Kernel
speed work reads `khot-ir`, `ksched-ir`, `kipc-ir` and `kobj-ir`, where the
kernel is most of the number.

### Three things inside that 46%, each worth knowing

| finding | number | what it is |
|---|---|---|
| **`Name::as_str` re-validates UTF-8** | 97,547 calls, 6,162,016 Ir (5.2%) | `Name::new` truncates on a character boundary from a `&str`, so the bytes are valid **by construction** — and every read re-checks them. `&[u8] -> &str` has no cheaper safe path, so this is the **measured price of `forbid(unsafe)`** in the traced kernel, not a defect |
| **the tick hook is copied twice per tick** | 76,026 memcpys of **144 bytes** | `TickHook: Copy` and `run_tick_hook` copies it out and back — deliberately, so it cannot alias while running in ISR context. The 144 bytes are the demo's `TickIsr`, an enum sized by its fattest variant. A real firmware's hook is small, so this is harness cost that scales with the hook |
| **`Kind::carries_data` is already optimal** | 336,006 Ir (4.09% of `khot-ir`) | the enum's variant ORDER is load-bearing and documented as such, so the test is one comparison. The share is call frequency, not cost. Recorded so nobody re-opens it |

### What was looked at and pruned, with the numbers

Four candidates were sized before any code was written, and none survived:

| candidate | why not |
|---|---|
| a skip gate on MQTT topic matching | `mqtt-ir` is **81 Ir/call** and rejects `finance` against `sport/…` on byte 0. A prefilter would add a pass to a path already at its floor |
| hoisting the JSON absence proof across query parts | **1 pair in 200** |
| an array-index bound for JSON, `idx > commas + 1` | sound, and **1 pair in 200** |
| a rarest-byte anchor for the JSON scan | 47% fewer candidates, but choosing the byte needs the document histogram it would save |

And one was built and **refuted by measurement**: restructuring the JSON scan
to a stateful `position` loop measured **+18.9%** — the second time the
census's `slice/iter` and `ptr/non_null` rows have been mistaken for removable
overhead when they were the scanning itself.

> **A census attributes cost to a line; it does not say the cost is
> removable.** Two attempts on the same rows, one −1.5% and one +18.9%.


## TaskNotify, and what its two kernel fixes cost (2026-09-19)

The twentieth conformance scenario. `TaskNotify.c` -> 806 Rust lines, **3,519
lines identical to the C kernel, exits included**
(`ticks=2001 yields=227 exits=2845 lines=3518`), pinned in `pins.rs` with a
digest taken from the C kernel's own trace. `conform --all` is 20 identical.
It agreed for 565 lines on the first run.

It closes three of H2's undifferentiated APIs -- `notify_value_clear`,
`timer_change_period`, `timer_delete` -- and needed two changes to the kernel.

**A timer delete never paid for its free.** `prvProcessReceivedCommands` ends
`tmrCOMMAND_DELETE` in `vPortFree( pxTimer )`, and every `heap_N.c` wraps free
in `vTaskSuspendAll` / `xTaskResumeAll` exactly as it wraps malloc. One
outermost exit, spent with no trace line to show for it. `queue_delete` and
`event_group_delete` had both been told; the timer command was the third
object with a free and **nothing in the corpus had ever deleted a timer**. It
presented as the other two did: every event agreed and the exit column was one
short from the first delete on, ours `#473` against the oracle's `#474`.

**`xTaskNotifyAndQuery` needs one critical section, not two.** It hands back
the value it is about to replace from inside the notify's own section. The
capi shim reads `notify_value` first and then notifies, which yields the same
two numbers and costs a second section -- and on the sim an exit is a tick
opportunity, so the extra one moves the tick. Hence `notify_and_query` /
`notify_and_query_from_isr`. `TaskNotify.c` calls it eleven times.

### The price, on every instrument

Both arms are real commits, built and counted under callgrind:

| instrument | before | after | delta |
|---|---:|---:|---:|
| `kdelay-ir` | 4,024,115 | 4,024,119 | +4 |
| `kdelay-deep` | 4,368,150 | 4,368,154 | +4 |
| `ksched-ir` | 1,790,783 | 1,790,783 | 0 |
| `khot-ir` | 14,800,492 | 14,800,490 | -2 |
| `kipc-ir` | 5,423,490 | 5,423,490 | 0 |
| `kobj-ir` | 5,616,763 | 5,616,763 | 0 |

Checksums identical down every column: the work is unchanged. Every delta is
at or inside the measured kernel-arm noise floor of 4.

**And the largest of them has an accidental null-arm control.** The first
attempt at this table had a harness bug -- `git checkout <commit> -- <paths>`
writes the INDEX, so the restore handed the second arm the first arm's code --
and that run, with BOTH arms on identical source, produced `kdelay-ir` +4 and
`kdelay-deep` +4, the same two numbers. A delta a null arm reproduces exactly
is not a delta. **Verdict: flat. The correctness fix is free.**

Two lessons kept: that git trap belongs to one-off scripts, since `ksweep.sh`
swaps a variant FILE and never touches the index; and both sweeps documented
their own invocation as `wsl -e sh ...`, a non-login shell with no cargo on
PATH, which printed a full table of `BUILD FAIL` and **exited 0**. Both now
fail the run when a cell does not build.


## AbortDelay conforms: the contract named objects after the allocator (2026-09-20)

The last scenario outside the gate is inside it. **2,549 lines, and the sim's
trace is now BYTE-IDENTICAL to the C kernel's.** `conform --all` is 21
scenarios, `BLOCKED` is empty, and both QEMU cells report 20 pinned scenarios
byte-identical on Cortex-M3 and on RV32.

### It was never a kernel disagreement, and that is measurable

Of 2,549 lines, 8 differed. **All 8 differed only in a queue ordinal**, `q2`
against `q3`; every event, tick, argument and ordering already matched. The
kernel had agreed all along.

### Why no fix on one side could work

The contract names unnamed objects by a per-kind ordinal. The C harness
computed it as *position among the same-kind pointers ever seen*, which makes
it a function of what `malloc` did; the Rust sink read it off the arena slot.
Those rules disagree in **both directions at once**:

| scenario | what happens | C says | arena says |
|---|---|---|---|
| `EventGroupsDemo` | same-sized group deleted and recreated, same block back | `g2` every time | `g2` — agrees |
| `AbortDelay` | binary semaphore deleted, 1-item queue created, different block | `q3` | `q2` — disagrees |

So a running count on our side alone fixed `AbortDelay` and broke
`EventGroupsDemo` (707,007 bytes against a pinned 702,507). That was recorded
in 2026-09-10 as a dead end. It was the right measurement and the wrong
conclusion: **no rule on one side can satisfy both, because the disagreement
is about an allocator the two kernels do not share.**

### The fix, and why it is better evidence rather than a workaround

Both sides now number by a monotonic per-kind creation counter: the n-th
object of a kind ever created is `<kind>n`, and an ordinal is never reused.

Identity now depends only on **creation order** — which is a thing the
differential already proves identical, line by line — instead of on a heap
layout that was never checking anything about the kernel under test. The old
rule could not be computed by a kernel that does not have a C heap, so it was
testing the allocator and calling it a kernel property.

### The cost, measured rather than estimated

The 2026-09-10 entry predicted "re-pinning every scenario". Measured: **one**.

| what moved | why |
|---|---|
| `EventGroupsDemo` digest and byte count | 120 creates that all said `g2` now say `g1`..`g119` |
| `AbortDelay` — **our side only** | the C trace is byte-for-byte what it always was |
| the other 19 scenarios | nothing: their creates never reused an ordinal, so the two rules already agreed |

`EventGroupsDemo`'s `ticks`, `yields`, `exits` and `lines` are **unchanged** —
only its digest and byte count moved. A naming change that moved a scheduling
counter would have been a naming change that was not one.

The C rule only differs from a creation count where an address was reused, and
that is why the blast radius was one scenario rather than twenty. Checking
which traces actually move, rather than assuming all of them do, is what
turned an "owner decision" into an afternoon.

### Closed with it

* **H5** — `xTaskAbortDelay` is now inside the gate, with `xTaskGetHandle`,
  `vTaskDelayUntil`, all three object deletes and both notify paths, all of
  which that scenario exercises.
* `kairos conform AbortDelay` used to answer *unknown scenario* while the
  `BLOCKED` note told the reader to run exactly that. Both are gone.
* `TaskNotify` had no committed `.trace.zst`. It has one now; the corpus
  pins 19 C traces and the set is complete.


## H6: the safe version of the delayed-list append is the fast one (2026-09-20)

`ListsOf::tail_value` handed its caller a three-part promise written only in
prose, and the kernel's delayed-list append was the one caller keeping it.
Closed by moving the PATTERN into the list as `insert_sorted`, which turned
out to be **a win on every instrument that touches it**.

| instrument | call site | `insert_sorted` | |
|---|---:|---:|---:|
| `kdelay-ir` | 4,024,119 | 3,995,313 | **-0.72%** |
| `kdelay-deep` | 4,368,168 | 4,344,745 | **-0.54%** |
| `khot-ir` | 14,800,504 | 14,736,516 | **-0.43%** |
| `ksched-ir` | 1,790,797 | 1,790,797 | flat |

Two real trees, callgrind, checksums identical down every column, and
`conform --all` still 21 scenarios identical to the C kernel.

### The first mechanism was wrong, and the bisect is why we know

The obvious closure keeps `insert_end` and TESTS the cursor, because
`insert_end` links before the cursor and therefore appends only while the
cursor sits at the marker. That version cost **+0.09% / +0.04% / +0.19%**.

A three-arm bisect separated the two effects:

| arm | `kdelay-ir` | `kdelay-deep` | `khot-ir` |
|---|---:|---:|---:|
| call site (before) | 4,024,119 | 4,368,168 | 14,800,504 |
| pattern folded in, no cursor test | 4,015,111 | 4,359,142 | 14,800,492 |
| + cursor test | 4,027,720 | 4,369,972 | 14,828,503 |

The fold was already a win; the whole cost was the check. So the check went,
and with it the reason for one: `insert_sorted` links between the tail and
the end marker directly, which appends whatever the cursor is doing.
**Removing the question beat checking it, on every arm.** That is worth
keeping as a shape -- when a runtime check is guarding an invariant, ask
first whether a different mechanism makes the invariant unreachable.

### Where the speed came from

Not from the safety. From what the old call site did twice: it read
`end(list)` once for `tail_value` and again inside `insert_end`, and it
called `set_value` to write a value that `link_between` writes for free on
the way in. One read, one write, no cursor.

### What is left, and what it costs to check

The list must already be sorted. That one cannot go without the `sorted`
flag this repository already measured as a 12% stage win turned 5% system
loss, so it stays the caller's -- but it is now CHECKABLE: `is_sorted` walks
the list and answers a `Result`.

A `debug_assert!` was the obvious alternative and is ruled out by this
crate's own contract: `tests/no_panic.rs` states that nothing in it panics
on any input a caller can construct, and fuzzes the list under
`catch_unwind` to prove it. The predicate is the better tool regardless --
it runs in a firmware self-check, not only in a debug build.

Five tests pin the result, including the hazard: `insert_sorted` on an
unsorted list DOES misplace, pinned as a hazard rather than fixed. One of
them fails if anyone ever rebuilds the append on `insert_end`.


## H3: 22 tests for the three files that had none, each pinning the C (2026-09-20)

`stream.rs`, `timer.rs` and `events.rs` had **no unit test at all**, and
most of their APIs are the ones no conformance scenario reaches either.
They now have 27 tests between them where they had none: 10, 6 and 6 new,
and the kernel-core suite goes from 27 to 49.

`kernel.rs` and `queue.rs` still have none, and that stays a decision
rather than a gap: the corpus is their oracle, deliberately.

### The rule these were written under

**Pin the C's contract, quoted, not this kernel's behaviour.** A test
written the other way round proves only that the code still does what it
did. Every one of the 22 carries the `stream_buffer.c`, `timers.c` or
`event_groups.c` lines it is pinning, so a reader can check the test rather
than trust it.

That rule earned itself twice: the audit found the TEST wrong and the
kernel right, both times.

### The find worth keeping: a zero-length message is a no-op

`prvWriteMessageToBuffer` writes the length header and keeps the new
position in `xNextHead`, a **local**:

```c
if( message buffer ) {
    if( xSpace >= xRequiredSpace ) {
        xNextHead = prvWriteBytesToBuffer( ..., sbBYTES_TO_STORE_MESSAGE_LENGTH, xNextHead );
    }
}
if( xDataLengthBytes != ( size_t ) 0 ) {
    pxStreamBuffer->xHead = prvWriteBytesToBuffer( ..., xNextHead );
}
return xDataLengthBytes;
```

`pxStreamBuffer->xHead` is only committed inside the `!= 0` branch, which a
zero-length message never takes. So the header bytes go into the array, the
head never moves, the buffer stays empty, the call returns 0, and the next
send writes over them. **A zero-length send is indistinguishable from a
send that failed**, and no amount of polling reveals the difference.

The first version of that test asserted the opposite. The kernel already
matched the C.

### The other contracts now pinned, none of which a scenario isolates

* a MESSAGE buffer reports `isFull` **with free bytes still in it** — the C
  compares spaces against the length HEADER (`<=`), not against zero, so a
  caller expecting `spaces_available() == 0` is wrong by up to a header;
* the timer API is a **command queue**: `xTimerStart` returning pdPASS
  means "the daemon has been told", `xTimerIsTimerActive` keeps saying
  false until it runs, and `xTimerGetPeriod` answers the OLD period after a
  successful `xTimerChangePeriod`. A setter and its getter that disagree;
* `xTimerGetExpiryTime` is an **unguarded** `listGET_LIST_ITEM_VALUE` — on
  a dormant timer it answers whatever the item was last left holding, and
  the return type cannot say so. Only `xTimerIsTimerActive` can;
* `vTimerSetReloadMode` is the odd one out: not a queued command, so it is
  immediate — and it reschedules nothing, so a running timer keeps the
  expiry it has;
* `xEventGroupSetBits` can answer a value **without the bit it just set**,
  because a waiter matched inside the call and its clear-on-exit ran before
  the snapshot. A control test with a non-clearing waiter pins that the
  surprise is about clear-on-exit, not about waking;
* `xEventGroupClearBits` answers the value from **before** the clear, so
  the return still contains what was cleared;
* `xStreamBufferNextMessageLengthBytes` answers 0 on a stream buffer
  whatever it holds — "the question does not apply", which the return type
  cannot distinguish from "no message waiting";
* `xStreamBufferReset` keeps the trigger level (the C passes the buffer's
  own back into `prvInitialiseNewStreamBuffer`) and refuses while a task is
  blocked on the buffer.

### Cost

None. The harness moved to `pub(crate)` on a `#[cfg(test)]` module, so the
release build is unchanged by construction and no instrument was re-run on
that account. `conform --all` is 21 scenarios identical, clippy clean.


## H4: the mutation survey, seven files to nine, and two broken instruments (2026-09-20)

`cargo mutants` had been run over **one file of nine**. It has now been run
over eight, plus a crate outside the list, and 48 tests were written against
what survived.

| target | oracle | mutants | caught | missed | viable killed |
|---|---|---:|---:|---:|---:|
| `list.rs` + `arena.rs` | core's own suite | 140 | 94 | 12 | **88.7%** |
| `queue.rs` | **the corpus** | 255 | 125 | 55 | **69.4%** |
| `name.rs` | kernel's suite | 9 | 6 | 3 | 66% |
| `events.rs` | kernel's suite | 120 | 67 | 38 | 64% |
| `timer.rs` | kernel's suite | 192 | 78 | 51 | 60% |
| `stream.rs` | kernel's suite | 163 | 84 | 65 | 56% |
| `typed.rs` | kernel's suite | 39 | 9 | 8 | 52% |
| `system.rs` | — | **0** | — | — | *nothing to mutate* |
| `port-core` (`SimPort`) | port's suite | 58 | 36 | 22 | 62% |

`list.rs` and `arena.rs` are CLOSED: all twelve survivors are cfg-disabled
or equivalent, so nothing reachable and non-equivalent remains alive.

**`queue.rs`'s 69.4% is the number that justifies the architecture.** It is
the largest file in the kernel and carries the whole IPC surface, it has no
unit tests by design, and the corpus catches more than two thirds of its
mutants — against the ~4% the kernel's own suite manages on `kernel.rs`.
Keeping the two repositories as separate cargo workspaces and bridging them
costs real trouble, and this is the measurement that says the trouble buys
something.

### What the survivors taught, and it was the same thing three times

**Blocking a task and never resuming it tests the blocking, not the
waking.** `finish_wait` had twelve survivors and `finish_sync` sixteen
because every test stopped at the wake-up: a waiter was blocked, a set woke
it, and nothing ever ran it again — so the code deciding what a RESUMED call
answers never executed. A full round trip, then a real two-task rendezvous,
took `events.rs` from 41% to 64%.

The contract that fell out is worth more than the score: a woken participant
is told **what satisfied it**, not what happens to be in the group when it
reaches the CPU. Those differ by exactly one other task's clear-on-exit.

**A guard nothing approaches is a guard nothing is testing** — and the same
guard can be reachable in one function and equivalent in another.
`is_sorted` and `insert_inner` both carry `if guard > N`; `>` and `>=`
disagree only at exactly `N`. `is_sorted` walks every item, so a full list
drives its guard there and the pair dies. `insert_inner` stops one node
short of the marker, so its guard reaches at most `N - 1` and nothing
constructible tells the two apart.

**21% of the first survivors were a surface nobody had touched** — the
`_from_isr` and pended calls. Testing them found that
`xEventGroupSetBitsFromISR` sets no bits at all: its whole body is
`xTimerPendFunctionCallFromISR`, so the answer means "the callback was
queued" and the group is untouched until the daemon runs.

### Two instrument defects, and both produce plausible numbers

**cfg-disabled code is scored MISSED, not unviable.** `end_index`, one of
the two `is_marker_of` bodies, and the whole of `port-cortex-m` and
`port-riscv` are gated to targets this host does not build. cargo-mutants
mutates the source anyway, the build succeeds because the function is not
compiled, the tests pass, and it reads as a survivor. That is six of core's
twelve and **134 of the port's 308**. The two arch ports scored 0 caught
for that reason and because they have no tests at all; measuring them means
mutating on thumbv7m and riscv32 through the QEMU cells.

**★ Concurrent `--in-place` runs across path-patched siblings corrupt each
other.** `rusty_rtos_core` is patched into both the kernel and the port, so
mutating core while either runs means they build against a mutated core and
a mutant is scored CAUGHT for the wrong reason. Measured on one population:

| run | caught | missed | unviable |
|---|---:|---:|---:|
| clean, before the new tests | 163 | 220 | **92** |
| contaminated | 209 | 119 | 152 |
| clean, after the new tests | 205 | 178 | **92** |

The two clean runs agree exactly on unviable; the contaminated one shows 60
more, because a mutated core broke COMPILATION. That shrank the viable
denominator and overstated the kill rate by about ten points. The honest
delta for those tests is 42.6% -> 53.5%, not the 63.7% the bad run implied.

cargo-mutants refuses outright in the worst case — starting a run while a
dependency was mutated gave *"cargo test failed in an unmutated tree, so no
mutants were tested"* and it produced nothing rather than producing
fiction. **Never mutate two repos that are path-patched into one another at
the same time.**

### The corpus bridge did not work, and now proves itself

Judging `queue.rs` and `kernel.rs` means building the DEMO against the
mutated kernel, and the recorded recipe does not do that today:

* the demo declares `rusty_rtos_kernel = { version = "0.1.0" }`, a
  **registry** dependency, patched to the local checkout only by its own
  `.cargo/config.toml`;
* cargo reads config from the **invocation** directory, which for that
  recipe is the kernel repo, whose table `kairos patches` fills with only
  the siblings that repo depends on;
* v0.1.0 **is published**, so the registry resolution succeeds silently.

Checked rather than argued: `cargo tree` from the demo resolves the kernel
to a PATH, the same query from the kernel repo resolves it to the registry.
Whether the 2026-09-09 run was affected cannot be settled from here — v0.1.0
was published a week after it — so that row stands and the METHOD changed.

`tools/mutants-corpus.sh` borrows the demo's own patch rows (sibling-relative,
so correct verbatim) and **proves the bridge before measuring**: it plants a
poison that cannot fail to fire, requires the gate to FAIL, removes it,
requires the gate to PASS, and aborts on either surprise. It refuses to
start on a dirty tree and restores everything on exit.

It took three corrections of its own, each of which would have produced
confident wrong numbers: it must run under **Git Bash** (cargo-mutants is
not in the WSL distro, and WSL git lacks `core.autocrlf` so a clean tree
reads as twelve modified files); and its first real run **exited 0 with 55
survivors**, because the EXIT trap's status was becoming the script's — the
same "reports success having measured nothing" defect `bench/sweep.sh`
carried.


## `kernel.rs`: two oracles, 92.5%, and why neither number works alone (2026-09-20)

`kernel.rs` was the last file in H4's nine, carried as "42 viable mutants,
40 caught, two survivors outstanding and unlocated" from a shard of 47. It
has now been run twice over its whole surface — once judged by the
CONFORMANCE CORPUS and once by the kernel's OWN unit suite — and the two
disagree about 239 mutants.

| | mutants |
|---|---:|
| survived the corpus only | 71 |
| survived the unit suite only | 179 |
| **survived both — the real gap** | **31** |

Together: **384 of 415 viable, 92.5%.** The corpus alone reads 75.4%, the
unit suite alone about a third.

**Neither number is usable on its own, and the reason is structural.** The
corpus runs `PosixDemoConfig`, which leaves `USE_TICKLESS_IDLE` at its
default of false — so it cannot execute the tickless path at all, and 23
of its survivors were sitting in `expected_idle_time`, `step_tick` and
`idle_suppress_ticks`. The unit suite has `TicklessConfig` and
`SleepyPort` and reaches exactly there, and very little else. Quoting the
corpus alone overstates the hole by 71; quoting the pair without
intersecting understates the coverage by seventeen points.

So: a mutant surviving ONE oracle is not a gap. One surviving BOTH is.

### Twenty-six tests, 80 -> 31

The gap fell 80 -> 65 -> 41 -> 31. Two of those tests are worth repeating.

**`delay_until`'s tick-overflow arms held nine of the original 80.** The
corpus runs 2,000 ticks and never overflows; no unit test went near it.
Reaching them needs no simulation at all, because `previous_wake` is a
CALLER variable — a wake time just below the maximum with a period that
carries it over the top exercises both halves of

    if( xConstTickCount < *pxPreviousWakeTime ) {
        if( ( xTimeToWake < *pxPreviousWakeTime ) && ( xTimeToWake > xConstTickCount ) )

The non-overflow half matters too, and its second case is the one a naive
test misses: a task that OVERRAN its period must not sleep another whole
one — it returns false, and still advances the wake time, or it never
catches up.

**`notify_value`'s survivors were `Ok(0)` and `Ok(1)`,** so the test pins
it with `0xabcd_1234`. A test asserting any small value would have passed
against a constant. Choosing the assertion to defeat the specific mutant is
what makes a test kill rather than merely pass.

The tickless cluster is closed: all five `expected_idle_time` survivors
are dead. What remains is led by `set_priority` (4), `delay_until` (4) and
a priority-inheritance cluster of six. That cluster is the interesting
lead — `recmutex` and `GenQTest` hammer inheritance in the corpus, so
survivors there point at the disinherit-after-TIMEOUT path, which no
scenario reaches.

### Three mutants HANG the kernel, and the "disk fault" was them

`increment_tick` (`&` to `|`), `unwind_pended_ticks` (`>` to `>=`) and
`tick_count` (to `0`) stop time rather than corrupting it. Each runs to the
harness timeout; the killed test leaves the build directory held, and the
next write fails with *"The device is not ready. (os error 21)"*.

That was read as a transient drive fault at first, and it was not. The tell
was free: repeated runs stopped at IDENTICAL counts — 151 of 474 twice,
then 29 twice — and a flaky disk does not do that. Excluding them one at a
time took the run from 151 to 388 to complete. **They are detections, not
gaps**: a mutant that stops an RTOS ticking is caught by any observer.

### Two ways to get a good-looking number that is not one

**A truncated run understates the gap.** An intersection computed from a
unit pass that died at 29 of 454 printed "real gap: 4", down from 65 — a
sixteenfold overstatement of progress, and it looks exactly like success.
The only guard is to check the run COMPLETED before believing its
survivor list, which is the null-arm law wearing different clothes.

**Long runs on this box fault intermittently**, unrelated to the hanging
mutants. Shard them: four short runs completed where three full ones did
not, and a fault then costs one shard instead of the measurement.

## The Cortex-M port, measured on a Cortex-M3 (2026-09-20)

`rusty_rtos_port-cortex-m` had **no tests**, and its host mutation score was
meaningless: the source is arch-gated, so on x86-64 every mutation landed in
code that is not compiled, the build succeeded, the tests passed, and all 87
mutants were scored MISSED. The same artefact accounted for 47 in the riscv
crate and 134 of the port workspace's 308 survivors.

It now has a real number, taken by mutating the port and booting the real
thing in QEMU:

| | mutants | caught | missed | killed |
|---|---:|---:|---:|---:|
| `port-cortex-m`, judged by 3 QEMU cells | 84 | 13 | 71 | **15.5%** |

### What the cells DO catch, and it is the right list

`init_stack`'s arithmetic and its masks (7), `write_reg` removed,
`yield_now` removed, `pend_switch` removed, `start_tick` removed, and
`tick` removed. That is: the exception frame a task is started with, the
fact that registers are written at all, the PendSV request, SysTick
starting, and the tick running. Break any of those and the cells say so.

### What they do NOT, which is the finding

The 71 survivors are almost all BIT-LEVEL: `&` to `^`, `&` to `|`, `==` to
`!=` on register masks and comparisons — 34 of them in the `Port` impl, 24
in the tick and tickless code, 13 in the register and trap helpers. Twenty
are in `suppress_ticks_and_sleep` alone.

**So the cells prove the port WORKS; they do not prove its register
handling is exact.** A mask that clears one bit too many leaves the switch
still switching and the tick still ticking, and three passing cells say
nothing about it. For a port that is meant to run on silicon that is the
useful thing to know, and it is not knowable from "20 scenarios
byte-identical on Cortex-M3", which is what the corpus cells already
proved.

### Two instrument defects found on the way, both fixed

**A broken port HANGS rather than fails.** Pointing `ICSR.PENDSVSET` at the
wrong bit leaves PendSV unrequested, so nothing reaches the semihosting
exit and QEMU runs for ever: a clean port passes in 3.2s, the poisoned one
ran 600s until it was killed from outside. The cell test therefore imposes
its own deadline and launches QEMU as a direct child it can kill -- `cargo
run` leaves QEMU orphaned on Windows. Nearly every CAUGHT mutant here is
caught by that timeout rather than by an assertion.

**An unsharded run reported 81 of 87 UNVIABLE, and none of them were.** The
log said `Failure(-1073741502)` -- 0xC0000142, STATUS_DLL_INIT_FAILED:
Windows refusing to start a process. The mutants it hit plainly compile
(`replace init_stack -> usize with 0` on a `usize` function). It is
cumulative -- the first six gave real verdicts and everything after failed
-- because each mutant spawns cargo, rustc, a NESTED cargo build for the
cell, and QEMU. Sharded into ten, the same 84 mutants produced **zero**
process failures and zero unviable.

That report would have passed for an ordinary one. The tell was that
"unviable" is a claim about COMPILATION, and the mutants it was claimed
for obviously compile.

## `StreamBufferDemo` — the 22nd scenario, and three kernel defects it found (2026-09-20)

**The headline: the corpus is 22 scenarios, all byte-identical to the C
kernel.** `StreamBufferDemo` is the biggest single addition — 20,737 lines
at the gate's 2,000 ticks — and it closes six of H2's APIs at a stroke.

| fact | value | method |
|---|---|---|
| lines identical, 2,000 ticks | 20,737 | `kairos conform StreamBufferDemo` |
| counters | `ticks=2000 yields=3023 exits=21953 lines=20736` | the scenario's own `KAIROS_RESULT`, both arms |
| **lines identical, 100,000 ticks** | **1,225,431** | `kairos conform StreamBufferDemo --ticks 100000` |
| counters there | `ticks=100008 yields=184513 exits=1303089 lines=1225430` | as above |
| oracle reproduces itself | byte-identical across two runs | `kairos oracle trace` runs it twice and refuses otherwise |
| corpus after | 22 scenarios identical | `kairos conform --all` |
| H2 APIs closed | 6 — `Reset`, `IsEmpty`, `IsFull`, `BytesAvailable`, `SpacesAvailable`, `ReceiveFromISR` | all called from `prvSingleTaskTests`; H2 goes 19 → 13 |

### The C side had to be unblocked first, and that is H9

The scenario would not run at all: it froze at `ticks=1`, and because
`kairos_trace.c` gives stderr a 1 MiB buffer flushed on exit, the symptom
read as *"0 lines, KAIROS_RESULT missing"* — a scenario that produces
nothing, not one that never ends. A control run of a known-good scenario
through the same command separated the two in one step.

The cause is structural and is recorded as `docs/HOLES.md` H9:
`prvNonBlockingReceiverTask` polls at `tskIDLE_PRIORITY` with
`sbDONT_BLOCK`, and that path takes **no critical section when the buffer
is empty** — so under sim contract v1 it takes no time, never yields, and
starves the clock. `kairos oracle patch` now drops it and its partner, the
same anchored-edit mechanism that already pins `TaskNotify`'s PRNG.

### Three kernel defects, each found by a one-exit drift

Every one of these was invisible to the other 21 scenarios and to the unit
suites. The `--exits` column found each in one run.

| # | defect | how it showed |
|---|---|---|
| 1 | a stream call preempted at its sampling exit **skipped the block entirely** and returned 0 from a call told to wait 350 ticks | events diverged at line 41 |
| 2 | `stream_buffer_delete` never paid for its `vPortFree` | one exit short at line 164 |
| 3 | the send never paid for `vTaskSetTimeOutState` | one exit short at line 474 |

Defect 1 is the serious one: the C evaluates `if( xBytesAvailable <=
xBytesToStoreMessageLength )` **below** the critical-section exit, against
the sample taken inside it, so a task preempted there still blocks. The
sim fell through to the read and answered `Ready(0)`. A caller that asked
to wait and was told "nothing, immediately" is a wrong answer, not a
timing one.

Defect 2 is the timer service's `Delete` arm again, in another file: the
create paid for its `pvPortMalloc` and the delete paid for nothing.

Three more followed from modelling the C's control flow properly rather
than its effect: `xTaskCheckForTimeOut` costs an exit but only on the path
where the wait actually blocked; `vTaskSetTimeOutState` must be paid for
once per call and not again on re-entry; and a `traceSTREAM_BUFFER_CREATE`
or `traceSTREAM_BUFFER_RECEIVE` whose task was switched away belongs to
that task's **next** run, not to whoever took the CPU.

### The rule that keeps being the answer

`uxTaskPriorityGet` takes a critical section and the kernel's own field
read does not — `priority_of` vs `task_priority_get`. The port used the
wrong one, and read it twice where the C reads it once. Both were single
exits, and single exits are sixteenths of a tick.

### What the workaround costs, measured before it was chosen

The dropped pair is the demo's only coverage of interleaved non-blocking
send/receive. It covers **none** of the six APIs the scenario exists to
close: all six are called from `prvSingleTaskTests`, lines 256–572, and
none from the pair. That was checked before the patch was written, not
after.

## The StreamBufferDemo instruction campaign — sixteen probes, five kept (2026-09-20)

**Headline: `StreamBufferDemo` costs 157,842,265 instructions at 20,000
ticks and now costs 138,869,663 — −18,972,602, or −12.02%.** Every one of
the twenty-two scenarios still produces a trace byte-identical to the C
kernel's, which is what makes the deltas attributable at all.

### Method

`bench/sb-ir/run.sh`, callgrind Ir, **two instruments over different
corpus shapes**: `sbd` = `StreamBufferDemo` (the whole stream-buffer face)
and `sbi` = `StreamBufferInterrupt` + `MessageBufferAMP` (one byte per tick
against a trigger level, and the length-prefix path `sbd` never takes).
Kernel-wide probes were also run against a third shape, `BlockQ` +
`GenQTest` + `TimerDemo`. Not a clock: deterministic to the instruction, so
no interleaving, no null arm, no z-score. Work parity is each scenario's
own `KAIROS_RESULT` line, printed beside every number, plus `kairos conform
--all`.

**The instrument had a defect on its first probe and it mattered.** The
bench summed only rows matching `rusty_rtos_*`, which excludes
`core::str::from_utf8`, `memcpy` and the inlined `BufWriter`. A change that
moved work across that boundary read **+19.9M** on the rows when the
program total was **+42.1M**. The verdict is now the program total, and the
script says so.

Measured this session: a rebuild from identical source reproduces **exactly
0** on both arms. Deltas below ~25 are that artifact.

### The five that landed

| # | change | `StreamBufferDemo` | ships? |
|---|---|---|---|
| 1 | `LineTrace::head` and `::num` inlined | **−8,888,722** | sim + cells |
| 2 | `resume_pending` split hot/cold | **−3,005,870** | **kernel** |
| 3 | `resume_all` inlined | −1,715,178 | **kernel** |
| 4 | `Stderr::write_str` inlined | −2,718,840 | sim |
| 5 | `Name::as_str` validates a 16-byte window | −2,644,036 | **kernel** |

Wins 2–5 improved **all six** scenarios measured. Win 4 was worth −9.29% on
`StreamBufferInterrupt` and −7.37% on `TimerDemo`; the sim's entire I/O path
was behind a call. Across the campaign `StreamBufferInterrupt` fell 14.95%
and `MessageBufferAMP` 14.07%.

Win 5 is the one worth remembering. `run_utf8_validation` reaches its
word-at-a-time path only when two `usize`s remain, so an eleven-byte task
name is checked a byte at a time — 86 instructions a call, 113,645 calls.
Validating the **whole 32-byte array** dropped that to 53 but LOST on four
of six scenarios. Validating the **smallest window that reaches the fast
path, sixteen bytes**, won on all six. The mechanism was right and the size
was wrong, and only the second instrument showed it.

### The eleven that did not, with their numbers

| probe | verdict |
|---|---|
| buffer the whole line, one `write_str` instead of eight | **+42,101,840** — `copy_from_slice` per field is a `memcpy` call, and the `from_utf8` on the way out costs more than the `BufWriter` bookkeeping it saves |
| replace two provably-exact `try_from`s; drop a 64-byte struct snapshot | **0** — LLVM had already done both |
| divide in 32 bits when the value fits | +1,406,818 — +2.75 a call, exactly a compare and a branch; LLVM's 64-bit divide-by-constant was already as cheap |
| move `num`'s scratch array into the sink so it is not re-zeroed | +1,022,407 |
| cache the tick's digits (12.1 lines share each tick) | +3,087,534 on `sbd`, and a loss on all six |
| `inline` on `end_line` and `line_tick_name_str` | −22, the rebuild artifact — already inlined |
| `Name::as_str` over the full 32 bytes | wins 2 of 6, loses 4 — a stage win and a system loss |
| `inline` on `send_completed` / `receive_completed` | +323,801 |
| `inline` on `exit_critical` | −210,292 on `sbd`, **+200,912 `BlockQ`, +280,540 `GenQTest`**, net +271,962 |
| remove `#[inline(never)]` from `streambuffer::Body::step` | −253,476 on `sbd` and **+585,000 on two scenarios that never run the file** — it lands in the dispatch every scenario shares |
| `inline` on `switch_context` | loses on all six |

Two of those agree on one thing and it is the campaign's most transferable
finding: **state in the struct is not free here.** The scratch array and the
tick cache both replaced arithmetic with a field reached through `&mut
self`, at a site inlined into every arm of `event`, and both lost. Where
this code looks like it is recomputing something, it is usually cheaper
than remembering it.

### The opposite-sign pair, and why the textbook fix was recorded not taken

`num` inlined ALONE moves the two instruments in opposite directions:
−6,412,337 on `StreamBufferDemo`, **+52,517** and **+36,810** on the other
two. That is the signature of one body serving call sites that want
different things, and the answer is to split the body from the symbol. It
was built — an `inline(always)` body that `head` takes in line, behind an
`inline(never)` handle — and measured at −3,939,614 / −280,875 / −284,090.
It works. It does not dominate: inlining both wins more than twice as much
on the scenario this bench exists for and still costs no arm anything. So
both are in line and the split is written down in `trace.rs` instead.

(Without the `inline(never)`, the split measured byte-for-byte identical to
inlining everywhere — the thin handle was itself inlined and took the body
with it. An attribute that changes nothing is a fact about the compiler's
existing choice.)

## Sim contract v2 — the blind-call tick (2026-09-20)

**H9 is closed, and it cost two re-pinned scenarios.**

### The rule

Time passes at kernel-visible points. v1 had two — the idle hook, and every
16th outermost critical-section exit. v2 adds the one that was missing:

> the return of a kernel call that took neither, priced at exactly one
> empty critical section.

Pricing it as an empty section is what makes it cheap and safe. It is not a
second clock — the count, the every-16th rule, the running-task test and
the switch are the paths v1 already proved, reached through
`vPortEnterCritical(); vPortExitCritical();` on the C side and
`self.enter_critical(); self.exit_critical();` on the Rust side. `exits`
keeps its meaning, widened from "outermost critical-section exits" to
"kernel-visible points".

### Why it is only two functions

Every other object closes the hole by accident — `uxQueueMessagesWaiting`,
`eTaskGetState` and `uxTaskPriorityGet` all take a section. Only the stream
buffer's zero-wait and query paths can return blind. So the bracket goes on
`xStreamBufferSend` and `xStreamBufferReceive` and nowhere else: four of
FreeRTOS's own `traceENTER_`/`traceRETURN_` hooks in `FreeRTOSConfig.h`, two
`vPortKairos*` entry points in the port patch, and one wrapper each in
`Kernel`.

A call that BLOCKED is not blind, and nothing special is needed to say so:
the thread stops inside it, other tasks run, and the counter has moved by
the time the return hook is reached. Comparing the count is the whole test,
on both sides.

### What it cost

| scenario | under v1 | under v2 |
|---|---|---|
| `StreamBufferDemo` | **would not run at all** | `exits=28002 lines=20927` |
| `MessageBufferAMP` | `exits=2072 lines=2430` | `exits=2090 lines=2445` |
| the other twenty | — | unchanged, byte for byte |

`StreamBufferInterrupt` being in the unchanged column is the check that the
widening was as narrow as intended: its reader always blocks, so it never
makes a blind call.

### What it bought

`kairos oracle patch` no longer edits `StreamBufferDemo.c`. The workaround
that dropped `prvNonBlockingSenderTask` and `prvNonBlockingReceiverTask` is
gone, both tasks are ported, and the demo's own check passes with all three
of its clauses — including `ulNonBlockingRxCounter`, which is the evidence
that the non-blocking receiver is genuinely making progress rather than
merely not hanging. The scenario is the upstream one again.

| gate | result | method |
|---|---|---|
| `conform StreamBufferDemo` | 20,928 lines identical, **first try** | trace compared line for line, counters included |
| `conform StreamBufferDemo --ticks 100000` | **1,155,782 lines identical** | as above |
| `conform --all` | 22 scenarios identical | the corpus |
| `tests/conformance.rs` | 3 passed | the pinned digests, two rows re-pinned |

### The one still out of reach

`xTaskGetTickCount` is blind on this port too (`portTICK_TYPE_IS_ATOMIC` is
1), so a task that busy-polled it alone would still stop the clock. Nothing
in the corpus does. The bracket is additive — two more hooks and a re-pin of
whichever scenarios call it — and it is named in `docs/HOLES.md` so the next
demo that needs it finds the answer rather than the symptom.

## The K7 packages on a Cortex-M3, and a gate that could not see them (2026-09-21)

Six K7 packages were each proved against their pinned C by a differential.
Every one of those differentials ran on **x86-64**, against a C program from
the same tarball. `tools/vertical/k7-battery` asks the question they could
not: are the answers the same at **half the pointer width**?

One battery, compiled from one source for two architectures, both asserting
against one pin table measured on the host. All six packages and the roll-up
digest are byte-identical on ARMv7-M:

```
      ok    tcp      0x69b93ded7cdb3812
      ok    http     0xb3f6599939dba776
      ok    mqtt     0x93444f0d9da263cb
      ok    json     0x3862c7de11afd42b
      ok    sntp     0x17475012129a67d7
      ok    backoff  0x47a6bfbda06c3a57
      ok    ALL      0xf6797db42bd95cc0
```

`rusty_rtos_tcp/docs/LEDGER.md` slice 11 has the full argument: why the
digests are comparable across widths at all (a `usize` is folded as eight
big-endian bytes, so a difference means a different VALUE rather than a
different pointer size), the 60-byte ELF delta proving the part executes the
battery rather than printing a constant its compiler folded, and the two
poisons proving the gate can fail.

**The chip was never an owner action for this question.** `qemu-system-arm`
11.1.0 with nine Cortex-M machines and `thumbv7m-none-eabi` were already
installed, and four kernel cells already boot on `mps2-an385`. One command
settled what the plan had listed as "buy a board". That is the fourth time in
this campaign that an obstacle dissolved the moment it was measured instead
of assumed -- after `/dev/net/tun` in WSL, the blocking seam, and rumqttd.

### The gate could not see the new crate, and said so itself

`kairos check` discovers cargo projects one level under `tools/`. Its own
comment says a hand-written list is how a project gets forgotten -- and the
same hole existed one level down: `tools/vertical/k7-battery` declares its
own `[workspace]`, so the `cargo check` run in `tools/vertical` stops at that
boundary. Nothing would ever have built it.

`tool_crates` now recurses, with two rules that keep the gate honest:

| rule | why |
|---|---|
| below the top level, take a directory only if its manifest declares `[workspace]` | ordinary members are already built by their parent, and some cannot build alone |
| skip a crate whose `.cargo/config.toml` pins a `target` | that is a FIRMWARE CELL. It needs an emulator this gate does not promise, so it belongs to `tools/vertical/chip.sh`, exactly as a `run.sh` directory owns its own policy |

Running it immediately found two defects in the change itself, which is the
argument for running a gate rather than reasoning about it:

* it descended into `tools/kairos/templates/function`, the SCAFFOLD `kairos
  new` copies, whose manifests are full of `__NAME__` placeholders and cannot
  compile by design. A template is source for a future project, not a
  project. **And it left something behind**: before failing, cargo wrote a
  `Cargo.lock` in the scaffold naming `__NAME__`. `kairos new` copies that
  directory, so every package created afterwards would have started with a
  lockfile naming a package that does not exist, and H-07 would have failed
  it. Deleted. The lesson is that the gate going red and the gate leaving
  damage are different events, and the second one was in a directory nobody
  was looking at.
* it labelled nested crates by their own directory name, so
  `tools/vertical/k7-battery` printed as `tools/k7-battery` -- a path that
  does not exist. The whole reason that line names the tool is that two bare
  `cargo check` lines look identical, and naming one WRONGLY is worse than
  not naming it.

Poisoned before being believed: a type error added to `k7-battery` fails
`kairos check` with `tools/vertical/k7-battery: cargo check --all-targets
failed`. **18 packages pass**, up from 17.

### And one defect in a rig the gate caught for free

`tools/vertical/src/bin/throughput.rs` had been added without a
`required-features = ["tap"]` stanza, so the umbrella gate tried to compile a
binary that cannot exist without a TAP device -- `TunTapInterface` and
`Instant::now` are both behind `smoltcp/std`. Its two older siblings declare
the stanza; the third was simply forgotten. There is nothing that infers it.

## K7's kill test, against the broker it names (2026-09-21)

> *"a Kairos device publishing over our TCP to the Home Computer's `rumqttd`,
> one hour, zero lost keep-alives"*

Two hours were run in parallel, on separate TAP devices and separate subnets.
Both passed.

| | mosquitto 2.0.22 | **rumqttd 0.20.0** |
|---|---|---|
| held | 3,600s | 3,600s |
| keep-alive periods, at 2s | ~1,800 | ~1,800 |
| PINGRESPs seen | 1,793 | 1,790 |
| protocol loops | 178,247 | 178,263 |
| **failures** | **0** | **0** |
| unexpected application events | 0 | 0 |
| minutes sampled | 59 | 59 |
| **quiet minutes** | **0** | **0** |
| fewest bytes in any one minute | 58 | 58 |

rumqttd is reached on its **v5** listener (port 1884). This client speaks
MQTT 5, and rumqttd's v4 and v5 listeners are different ports -- sending a v5
CONNECT at a v4 listener is a protocol-version refusal, not a stack problem,
and a rig should get that right before it blames anybody.

### The PINGRESP count is NOT the evidence, and it matters which one is

1,790 of ~1,800 looks like ten lost keep-alives and is not. The counting
window closes while the last few exchanges are still in flight, and the
per-minute sampler covers 59 whole minutes plus a partial 60th: 59 x 30 =
1,770, and the rest is the tail.

**The evidence is that the broker never dropped us.** Both brokers reap a
client 3.0 seconds after a missed keep-alive, and both held the connection
for the full 3,600 seconds. A keep-alive late enough to matter would have
ended the run. The broker's own timeout is the oracle here, exactly as the C
kernel's trace is the oracle for the scheduler -- a third party deciding,
rather than us scoring ourselves.

`quiet-minutes=0` is the shape figure that supports it: **no single minute
of either hour was silent**, and the leanest minute still carried 29 of its
30 responses.

### What was substituted, and is not any more

Both substitutions this campaign made are gone. The broker is the named one,
and the duration is the specified one. Neither was ever hidden -- the scripts
printed their own substitutions on every run -- but printing a substitution
is not the same as not needing one.

The rumqttd substitution should never have been made at all. It rested on a
note recorded here as measured fact: *"`rumqttd` 0.20.0 does not build on
this toolchain"*. That was **wrong**. `cargo install rumqttd --locked` fails
with E0521 in its `metrics` dependency; the same command without `--locked`
resolves a working `metrics` and installs. The broken thing was the published
LOCKFILE.

That is the fourth obstacle in this campaign to dissolve on being measured
rather than assumed -- after "interop cannot be faked on a workstation" (WSL
runs as root with `/dev/net/tun`), "blocking needs the kernel" (it needs a
two-method seam), and "a chip is an owner action" (QEMU was already
installed). Each was written down as a measured fact. Each was wrong, and
each was cheap to check.

## An H-07 violation that was already committed (2026-09-21)

Verifying two clauses of K7's kill test meant running `cargo test` in
`rusty_rtos_json`, `rusty_rtos_sntp`, `rusty_rtos_mqtt` and
`rusty_rtos_backoff`. That dirtied four `Cargo.lock`s, which is the known
trap: cargo run inside a package while the umbrella's `[patch]` table is
active rewrites the sibling's entry and drops its `source` and `checksum`.

Restoring them is routine. What was not routine is that **two of the four had
nothing to restore.**

| package | `git status Cargo.lock` | `rusty_rtos_core` had a `source`? |
|---|---|---|
| `rusty_rtos_json` | ` M` | yes, once restored |
| `rusty_rtos_sntp` | ` M` | yes, once restored |
| **`rusty_rtos_mqtt`** | **clean** | **no** |
| **`rusty_rtos_backoff`** | **clean** | **no** |

A clean working tree whose lockfile is still wrong means the patched form was
**committed**. A fresh clone of either package could not resolve
`rusty_rtos_core` at all -- which is the entire thing hardening gate H-07
exists to prevent.

Both regenerated with the documented procedure and now carry
`source = "registry+..."` and a checksum. The fix is uncommitted; committing
it is the owner's, and it must be committed BEFORE anything else invokes
cargo in those two packages, or the patched form goes straight back.

### Why the gate did not catch it earlier

H-07 reads the lockfile on disk. On disk it was wrong, so the gate WOULD have
said so -- on any run that reached those packages with the file in that
state. What hid it is that the same `kairos check` run that validates a
package also restores its lockfile afterwards, so a green run leaves the
working tree looking correct whatever was committed. The state that matters
is the one in git, and nothing was comparing the two.

**That is the lesson worth keeping: a gate that repairs what it inspects can
report green forever on a repository that is broken for everyone else.** The
distinguishing check is one command -- `git status` on the lockfile beside
`grep -c 'source = '` on it -- and disagreement between those two is the
signal. Two packages sat in that state.

## The list beats `list.c`: 35.84 -> 19.80 instructions per operation (2026-09-21)

K1 left the arena-and-list cost row open at **2.08x** the C list, with the
mission plan's revisit condition at 1.25x. A later pass took it to 1.533x.
This takes it to **0.887x**: fewer instructions per list operation than the
`list.c` it remakes, at `-O2` and at `-O3`.

| arm | instr/op | vs C `-O2` |
|---|---|---|
| `rusty_rtos_core::list`, at the start of this session | 35.84 | 1.606x |
| C `list.c`, gcc 15.2 `-O2` | 22.32 | 1.000x |
| C `list.c`, gcc 15.2 `-O3` | 21.82 | 0.978x |
| **`rusty_rtos_core::list`, now** | **19.80** | **0.887x** |

Instrument: `bench/list-cost/run.sh` — callgrind, three run lengths so the
cost of a round is the SLOPE and process start-up cancels exactly, both arms
gated on an identical checksum of every value every operation returned.

### The baseline had moved, and that mattered

The ledger's figure was 34.22; this session measured **35.84** before
touching anything. WSL carries rustc 1.97.1 and the umbrella's Windows
toolchain is 1.98.0, so the two arms of that comparison were different
compilers. Every number here is same-session, same-invocation, against
35.84. A refutation expires when its baseline moves, and so does a win.

### What was actually wrong: two arrays and a tagged link

`list.rs` kept `items: [Node; N]` and `ends: [End; L]`, and encoded a link as
"below `END_BASE` means an item, at or above means a marker". Every traversal
therefore paid a test to learn which array it was about to read, and then a
bounds check on whichever one it was.

C FreeRTOS does not do this. `xListEnd` is a `ListItem_t` embedded in
`List_t`, so `vListInsert` follows `pxNext` and never asks whether it has
arrived. **The marker being a list item is not an implementation detail of
the C; it is why the C is fast.**

The fix is to do the same: one array, markers at the top, each carrying
`V::MAX` so the ordered walk stops on them by the comparison it was making
anyway.

### Probed before it was built

Four throwaway binaries in `bench/list-cost/rs/src/bin/`, each gated on the
same checksum, sized the prize before a line of `list.rs` changed:

| probe | instr/op | what it says |
|---|---|---|
| unified array + `% SLOTS`, exact size | 27.37 | **REFUTED**: LLVM lowers a constant modulo to a multiply-shift that costs more than the branch it replaces |
| unified array, bounds-checked indexing | 20.70 | the split alone was worth **15.14** |
| unified array + power-of-two mask | **16.55** | the mask is worth another **4.15** |

The floor said the direction could clear 20 with room, which is what made
the API change worth proposing. The modulo probe is the one worth keeping in
mind: it is the version that needs no power-of-two rounding, and it is worse
than either alternative.

### The mask, and why `N` changed meaning

Every internal link is now followed as `nodes[link as usize & (N - 1)]`. LLVM
can prove `x & (N - 1) < N`, so the bounds check folds away — no `unsafe`, no
`get_unchecked`, a panic that is unreachable rather than suppressed. The
census confirms it: `core/src/slice/index.rs` was **19.63%** of the arm
before and does not appear at all after.

That requires `N` to be a power of two, so `N` is now the SLOT count rather
than the item count, and `slots_for(items, lists)` computes it. The kernel
gains `list_slots_for(tasks, timers, lists)`; `items_for` keeps its own
honest meaning — two list items per task plus one per timer — because
renaming a constant to mean something else is how one starts lying.

### The single biggest line: a guard that was dead code

The ordered-insert walk carried a step counter that returned
`InvalidArgument` if it ran longer than the arena. Removing it took the arm
from **23.07 to 19.80 — 3.27 instructions per operation**, far more than
predicted, and it is what crossed the 20 line.

It was dead code, for three reasons that are now a test rather than an
argument (`the_marker_is_what_terminates_the_ordered_walk`):

1. a marker's value cannot be written — every path to a `value` write goes
   through `item_mut`, which refuses any id at or above `CAPACITY`;
2. the walk only runs for values strictly below `MAX_VALUE`, because
   `MAX_VALUE` takes the append branch above it;
3. so the marker breaks the loop within `len + 1` steps.

**Both poisons were run.** Letting a caller reach a marker (`item_mut`
bounded by `N` instead of `CAPACITY`) fails the test cleanly. Giving the
marker `ZERO` instead of `MAX` does **not** fail it — it HANGS, exit 124
under a timeout, because an ordered walk with no terminator is an infinite
loop.

That is a real cost of removing a guard, so the guard did not leave
entirely: it is `#[cfg(debug_assertions)]` now. Tests run in debug and fail
cleanly; the kernel ships in release and pays nothing — measured
byte-identical at 19.80 either way. C FreeRTOS has no such counter at all,
so this is strictly better than the oracle rather than merely equal to it.

### Gates

| gate | result |
|---|---|
| `rusty_rtos_core` tests | 58 pass, including the new marker test |
| `rusty_rtos_kernel` tests | 85 + 15 pass |
| `kairos conform --all` | **22 scenarios identical to the C kernel** |
| `kairos conform --all --ticks 100000` | **22 scenarios identical**, over a million lines each |
| bench checksum gate | both arms agree on every returned value, every run |

The conformance corpus is the gate that matters: the list sits under the
whole scheduler, and 22 scenarios reproducing the C kernel's trace line for
line at 100,000 ticks is what says a rewrite of it changed no behaviour.

### The price, stated plainly

RAM. A marker is now a 16-byte `Node` rather than an 8-byte `End`, and the
array is rounded up to a power of two.

| | before | after | delta |
|---|---|---|---|
| list arena, corpus geometry (80 items, 39 lists -> 128 slots) | 1,592 B | 2,204 B | **+612 B (+38%)** |
| whole kernel, same geometry | 13,068 B | 13,680 B | **+612 B (+4.7%)** |

Two things soften it and neither erases it. The rounding is not waste: the
spare slots are **item capacity** — this geometry can declare 9 more tasks'
worth of list items for nothing. And the alternative measured above (exact
size, no mask) is 20.70 instr/op, so the rounding buys 4.15 instructions per
operation for 288 of those 612 bytes. Whether that trade is right on the
smallest targets is the owner's call, and the numbers to make it are here.

### 18.80, and the floor that says where this stops (2026-09-21)

`remove` was clearing the unlinked node's `prev` and `next` to `NONE` as well
as its `container`. `uxListRemove` clears only `pxContainer`, and the two
extra stores were hygiene rather than safety: nothing can follow a removed
node's links, because every path to them tests `container != NO_LIST` first.

Deleting them: **19.80 -> 18.80 instructions per operation**, a clean 1.00,
which is 40 stores per round. 22 scenarios still identical to the C kernel.

| arm | instr/op | vs C `-O2` |
|---|---|---|
| session start | 35.84 | 1.606x |
| C `list.c` `-O2` | 22.32 | 1.000x |
| **now** | **18.80** | **0.842x** |

### Why not below 15

Asked for, probed, and **refuted with a number.** `bench/list-cost/rs/src/bin/probe.rs`
is a bare index-linked list doing this exact workload with the same checksum
and **no safety at all** — no `Result`, no handle validation, no
`Busy`/`NotActive`, no `Option`. Re-measured in the same session:

| arm | per round | instr/op |
|---|---:|---:|
| zero-safety probe | 661.86 | **16.55** |
| ours, full safety | 751.87 | 18.80 |
| **below 15 would be** | **< 600** | **< 15** |

So 15 instructions per operation is **62 instructions per round BELOW an
implementation that does no checking whatsoever.** It is not a matter of
removing more safety; there is not enough safety left to remove.

The decomposition says the same thing from the other side. Of 751.87:

| | per round | note |
|---|---:|---|
| the BENCH's own body | 189.0 | 5 loops of 8, 8 xorshifts, 24 checksum mixes |
| node addressing | ~171 | ~168 accesses at ~1 instruction each — the mask folds into the addressing mode |
| everything else the list does | ~392 | the walk, the field writes, the cursor and length |

The bench body is 4.7 instr/op and cannot be reduced without changing the
measurement rather than the code — and it is already far cheaper than the C
arm's own body (189 against 387). Reaching 15 would require the list's share
to fall from 563 to 411 per round, a 27% cut of code that is now at
structural parity with `list.c`: the same six writes per insert, the same
four per removal, the same walk.

**What IS still available, and its price.** The 90-per-round gap to the probe
is entirely `Result` and `Option` plumbing at call sites, the one-time handle
validation, and the `Busy` check. An infallible hot path for pre-validated
handles would recover most of it — about **1.6 instr/op, landing near 17.2** —
at the cost of a second, unchecked-by-construction entry point on a core
crate's public surface. That is an API decision, not a measurement, so it is
recorded here rather than taken.

A refuted target with a number beside it is worth more than an open one: the
next session should not re-run this probe, it should read 16.55 and know.

### An instrument defect found on the way

`bench/kernel-ram` and `bench/kernel-flash` carry a committed `[patch]` table
whose comment says the cell "measures the kernel IN THIS CHECKOUT". It
patched the three git URLs — and `rusty_rtos_kernel` names `rusty_rtos_core`
as `{ version = "0.1.0" }`, a **crates-io** dependency, which no git-URL
patch reaches. Both benches have been building the local kernel against the
PUBLISHED core.

It surfaces only when the two have diverged, which is precisely when the
measurement matters, and it reads as a compile error inside the kernel
rather than as a wiring fault in the bench. `[patch.crates-io]` added to
both.

## The allocator above 512 bytes, and a premise that had gone stale (2026-09-21)

### First, the correction

This ledger and the mission notes both carried "rusty_alloc is 2.4x faster at
16 B and **1.27x slower at 256-512**". Re-measured on the part today, twice,
identical to the cycle:

| request | `rusty_alloc` | `heap_4` | ratio |
|---:|---:|---:|---:|
| 256 | 101 | 235 | **2.33x faster** |
| 512 | 114 | 235 | **2.06x faster** |

**The 256-512 regression does not exist any more.** The `direct_route` /
`shape_of` work that this project reported upstream shipped in `rusty_alloc`
2.2.0, the A/B cell locks 2.2.0, and the figure in these notes was never
re-taken afterwards. A number kept without its date is a number that goes
quietly wrong.

### The cliff is at 512, to the byte

A sweep one byte either side, same run:

| request | cycles/op |
|---:|---:|
| 504 | 114 |
| **512** | **114** |
| **513** | **249** |
| 520 | 249 |
| 640 | 249 |

`SMALL_SIZE_MAX = SMALL_WSIZE_MAX * INTPTR_SIZE`, and `SMALL_WSIZE_MAX` is
mimalloc's fixed 128 — so the `direct[]` fast-path table covers **1 KiB on a
64-bit host and 512 bytes on a 32-bit chip**. One byte over it is +135
cycles, because the request falls off the end of the table and takes the
generic path.

Every Kairos target is 32-bit. This is the same pointer-width trap as the
`prim::fixed` small-step investigation, and it has the same property: **a
host sweep cannot see it**, because on a 64-bit host the bound is 1,024 and
the sizes in question sit below it.

### Refuted first

`--cfg ra_generic_collect="65536"` — the page-churn lever that fixed the
EARLIER regression — was tried and produced a **byte-identical** table. That
diagnosis explains nothing here; it had already done its work upstream.

### The knob, prototyped and measured

`SMALL_WSIZE_MAX` made into a cfg, exactly as `GENERIC_COLLECT_DEFAULT`
already is, and for the reason that one's doc gives: a bare-metal consumer
could measure a bound was costing it and had no way to move it. Everything
else already derives from it, so nothing else was touched.

With `ra_small_wsize="512"` on the same part, same harness, null A/B delta 0:

| request | before | after | ratio before -> after |
|---:|---:|---:|---|
| 16 | 88 | 90 | 2.67x -> 2.61x |
| 256 | 101 | 102 | 2.33x -> 2.30x |
| 512 | 114 | 115 | 2.06x -> 2.04x |
| **513** | 249 | **122** | 0.98x -> **1.99x** |
| **640** | 249 | **122** | 0.94x -> **1.93x** |
| **1024** | 249 | **140** | 0.94x -> **1.68x** |
| **2048** | 251 | **190** | 0.94x -> **1.24x** |

**Both bands where we lost to `heap_4` now win.** The price is stated rather
than buried: every size at or below 512 gets about **2 cycles slower (~2%)**,
the cost of a table four times the size — 2,052 bytes per heap against 516 on
a 32-bit target.

`cargo test -p rusty_alloc` passes with the knob off and on. Two tests needed
the per-arm treatment their own neighbour prescribes for `ra_segment_size`:
pin each arm rather than the default's numbers, so the mimalloc oracle value
stays pinned for the default build. The Kani proof
`direct_table_index_is_always_in_range` is written against the derived
constants, not literals, so it re-verifies unchanged.

### Why 1024 and 2048 stop short of 2x

There is a SECOND boundary, and it is not the same one:

```rust
pub const SMALL_OBJ_SIZE_MAX: usize = SMALL_PAGE_SIZE / 8;   // 4 KiB / 8 = 512
```

Above 512 bytes an object comes from a **medium page** whatever the direct
table says. That is the residual: the curve keeps climbing with size (122 at
640, 140 at 1024, 190 at 2048) instead of flattening at 122.

Reaching it wants a bigger slice, which means a bigger segment —
`ra_segment_size="256k"` would give an 8 KiB slice and a 1 KiB
`SMALL_OBJ_SIZE_MAX`. **That was not measurable here and is recorded as
untested, not as refuted:** the A/B gives each allocator 64 KiB for parity,
a 256 KiB segment does not fit in that, and two 256 KiB arms do not fit the
part's SRAM at all. The blocker is the board, not the allocator.

### The build profile, priced — and why 50% at 256-512 B is out of reach (2026-09-21)

The harness's own method line names the asymmetry: the C arm is `-O2`, the
Rust arm is **`opt-level = "s"` with `overflow-checks = true`**. One is built
for speed, the other for size and with arithmetic checks C does not make.
Measured on the part, three configurations, same harness, null A/B delta 0:

| configuration | 16 | 128 | **256** | **512** | 1024 |
|---|---:|---:|---:|---:|---:|
| `opt-level="s"`, checks on — **what ships** | 88 | 94 | **101** | **114** | 249 |
| `opt-level=3`, checks on — **H-05 intact** | 80 | 85 | **90** | **101** | 220 |
| `opt-level=3`, checks off — the ceiling | 67 | 72 | **78** | **88** | 207 |

So in the 256-512 band:

* **`opt-level=3` alone is worth 11%** (101 -> 90, 114 -> 101), taking the
  ratio against `heap_4` from 2.33x/2.06x to **2.62x/2.34x**. It honours
  hardening gate H-05 and is a pure size-for-speed trade a firmware may make.
* **Turning overflow checks off adds another 12%**, and that is the measured
  price of H-05 at these sizes. Recorded, not taken: the rule is the owner's.
* **Both together are 23%.**

**A 50% improvement at 256-512 B is refuted.** With every lever pulled the
allocator costs **67 cycles at 16 bytes** — its floor at any size. 256 bytes
costs 78 there, so the entire size-dependent component at 256 is 11 cycles.
Removing all of it would give 67, which is 34% below the shipped 101, not
50%. The target is below the allocator's own floor, and no size-specific fix
can reach it; halving the figure would mean halving a mature upstream
allocator's whole fast path.

The cell is left at `opt-level = "s"`, which is what the firmware cells use
and therefore what the benchmark ought to measure. The 11% is available to
any firmware that would rather have the speed than the size.

### The 50% target, chased to the instruction

The refutation above rested on a curve — "67 cycles at 16 bytes is the floor"
— and a curve does not say WHY. Before letting it stand, the path every
measured call takes was read end to end. Three hypotheses, each a plausible
source of avoidable work, each refuted by reading the code rather than by
argument:

| hypothesis | verdict |
|---|---|
| `Layout` forces an alignment-aware path the C's one-argument `pvPortMalloc` never pays | **refuted** — `RustyAlloc::alloc` routes `align <= 8` to the size-only `malloc(size)`, and the cell allocates at 8 |
| `malloc_small` is a cheaper entry the seam is missing | **refuted** — it is literally `pub fn malloc_small(size) { malloc(size) }` |
| the per-allocation heap lookup costs TLS on Xtensa | **refuted** — on `no_std` the `ra_thread_local!` macro expands to a plain `static SingleThreadCell`, so it is one load |

What the fast path actually is, in full: one static load, a `size <=
SMALL_SIZE_MAX` test, a `div_ceil` by the word size, a `direct[w]` load, a
`page_pop` (load the free-list head, store its successor), a null test, and
the return. **Seven operations.** Free is its mirror.

There is no 50% in seven operations. Halving 88 cycles would mean removing
~44 from roughly 35 instructions, and none of the three candidate sources of
slack exists. The only lever that moved anything was the build profile, and
it is worth 11% shippable and 23% with H-05 broken.

One instrument note while reading it: the null arm subtracts the loop, the
`black_box` and the two checksum adds, but NOT the one-byte write and read
into the returned block — those need a real pointer. So a few of the 88
cycles are the workload touching its own memory, not the allocator. It makes
the allocator look slightly worse than it is, applies to both arms, and does
not move the conclusion.

### The instruction count, which is what finally settles it

Everything above bounded the 50% target with cycles and with reading. The
missing quantity was the one an exact instrument can give: **how many
instructions an alloc/free pair actually is.** Counted with callgrind by the
slope method (`rusty_alloc/bench/fastpath-ir`, three lengths so start-up and
allocator init cancel; linearity within 0.004%, and exactly 0 at 256):

| request | Ir per alloc+free pair | cycles on the S3 |
|---:|---:|---:|
| 16 | **48.31** | 88 |
| 256 | **53.57** | 101 |
| 512 | **59.14** | 114 |

**The entire fast path — both halves — is about 54 instructions at 256
bytes.** And the SIZE-dependent component is about **11 instructions**: that
is the whole difference between 16 bytes and 512, a 32x range.

That is the bound, stated exactly. A size-targeted optimisation can address
at most those ~11 instructions, or **20%**. Reaching 50% would mean deleting
27 instructions from a 54-instruction path — half of the allocator's entire
fast path, alloc and free together, not the part that varies with size.

It also matches the silicon from the other direction: `opt-level=3` measured
11%, every lever together 23%, and the arithmetic here says the ceiling for
anything size-specific is 20%.

**One caveat, because it limits what this number proves.** The count is
x86-64; the cycles are Xtensa. The two ISAs do not retire the same
instructions for the same work, so the naive ratio (~1.9 cycles per
instruction) is not a CPI for the part and is not quoted as one. What DOES
carry across is the shape: 54 instructions total, ~11 of them size-dependent,
in a path whose six-deep dependent load chain is the same on both.

### And a reason not to chase the last of it on THIS instrument

The A/B harness says of itself: *"the harness holds exactly ONE allocation
live at a time."* That is deliberate and it is what makes heap size not part
of the measured path.

`rusty_alloc/src/options.rs` says, of the knob nearest this band:

> **Do not raise it because a benchmark that keeps one block live got
> faster** — that workload cannot decay.

The allocator's own authors wrote that warning about tuning to exactly the
workload this cell runs. A one-live-block, same-size-repeatedly loop rewards
caching the last page and the last freed block, and neither helps a real RTOS
mix of TCBs, queue items and timers arriving and leaving at different sizes.

So the remaining ~11 size-dependent instructions are not merely a 20% ceiling
— they are 20% that would have to be won by optimising for a pattern the
upstream project explicitly says not to optimise for. The measured refusal
and the methodological one point the same way, which is the strongest form a
"no" comes in.

What the harness IS good for stands unchanged: it is a fair, checksum-gated,
null-armed comparison against `heap_4` on silicon, and by it we are **2.3x to
2.7x faster at every size at or below 512 bytes**.

### The answer was a different strategy, not a faster allocator

The 50% target is unreachable for the general allocator — 54 instructions per
pair, 11 of them size-dependent. But the question "make 256-512 byte
allocation faster" has an answer the general allocator cannot give, and an
RTOS is exactly where it applies: **the sizes are known at compile time.**
TCBs, queue items, timer records. A fixed-size pool serves one block size
from a pre-sized arena, so there is no size class to compute, no bin, no page
lookup — `alloc` is a pop and `free` is a push.

Measured, host, callgrind, slope method (`rusty_alloc/bench/fastpath-ir`):

| arm | 256 B | 512 B |
|---|---:|---:|
| general (`RustyAlloc` via `GlobalAlloc`) | 53.57 Ir | 59.14 Ir |
| **fixed-size pool** | **12.00 Ir** | **12.00 Ir** |
| | **-77.6%** | **-79.7%** |

Flat in size, which the general path is not — there is nothing size-dependent
left to be dependent on. The bounds checks are still in: this is what a SAFE
pool costs, not an unchecked one.

**What it costs to have.** A pool answers a narrower question: one size,
capacity pre-sized, no sharing with other sizes. It is not a replacement for
the allocator, it is the right tool for the fraction of RTOS allocation that
is fixed-shape — and that fraction is most of it.

**The silicon figure is NOT quoted as the headline.** The cell reads 8 c/op
against the general path's 101, which would be -92%. It is not believed: the
chip-side pool loop pops and pushes the same slot each iteration, which LLVM
can partly fold, where the general allocator is opaque to it. The host
instruction count compares two implementations the compiler treats alike, so
it is the defensible number. An impossible-looking figure gets checked, not
banked.

### The headline numbers are the RECYCLING case, and that matters

Splitting the pair (blocks HELD live, so neither half hides in the other)
turned up something the paired loop cannot show:

| request | alloc c/op | free c/op | pair | paired-loop figure |
|---:|---:|---:|---:|---:|
| 16 | 46 | 54 | 100 | 88 |
| 256 | 114 | 77 | **191** | 101 |
| 512 | 173 | 100 | **273** | 114 |
| 1024 | 267 | 67 | 334 | 249 |

**Holding blocks live costs roughly twice what recycling one does** at 256 and
512 bytes, because every allocation carves a fresh block instead of handing
back the same hot one. The A/B cell's headline table is the best case, and it
says so about itself ("the harness holds exactly ONE allocation live at a
time") — but the size of the gap was not known until now.

That does not invalidate the A/B: both arms run the same loop, so the ratio
stands. It does mean the ABSOLUTE cycle figures are a floor, and anything
sized from them should be sized from the split instead. `heap_4` has not yet
been measured in the held-live shape, so no ratio is claimed there.

### The method line was asserted, not derived

`println!("Rust arm  rusty_alloc small-metal, opt-level=s + LTO")` — a string
literal. Building the cell at any other optimisation level produced a report
that still said `opt-level=s`. The first ceiling probe above would have
printed exactly that while running at `opt-level=3`.

That is the precise failure a printed method line exists to prevent, and it
is the same shape as the two other stale claims this session found: a memory
note pinning an allocator figure upstream had already fixed, and a bench
whose `[patch]` table reached the git URL but not the registry. **A check
that reports on something other than what actually happened.**

Now derived: `build.rs` passes `OPT_LEVEL` through as an env var and reads
`overflow-checks` out of the manifest — Cargo hands a build script neither
`overflow-checks` nor a stable `cfg!(overflow_checks)`, so the manifest is
the setting's own source of truth. The line now reads
`opt-level=s + LTO, overflow-checks=true`, and it changes when the build
changes.

### One instrument caveat worth carrying

The harness reports 0 cycles of resolution from its null arm, and that is
true of the Rust arm against itself. But `heap_4`'s flat cost read **235 in
one build and 236 in another**, where the only difference was the *Rust*
arm's optimisation level. Code layout. A single-cycle difference across
rebuilds is not a result here, whatever the null arm says.

### Where it lives

The knob is an **uncommitted prototype** in the `rusty_alloc` working tree,
with a `[patch.crates-io]` in the A/B cell pointing at it — both marked as
such, because published 2.2.0 has no such cfg and a Kairos cell cannot depend
on a sibling repo's working tree. The proposal, with every number above, is
`kairos-upstream/drafts/rusty_alloc-small-wsize-knob.md`; filing it is the
owner's.

### An instrument note

The A/B harness earns its keep again: it prints its own method line, a null
arm it subtracts, a **null A/B that reads 0 cycles of resolution**, and a
work-parity checksum both allocators must agree on. Every figure here is the
best of 32 interleaved rounds, and both configurations were run twice and
were identical to the cycle. One run failed with `Access is denied` on COM4
and was not a contended port — the S3's USB CDC re-enumerates after a reset
and is briefly unavailable. Retrying is correct; concluding anything from it
is not.

## heap_4's byte arena: the word-array probe, built and REFUTED (2026-09-21)

### First, the target was misread

"`heap_4` 256-512 B" means **our** `heap4.rs` — a Kairos remake with its own
instruction-count bench (`bench/heap4-ir`) and its own byte-exact
differential — the same kind of thing "our `list.c`" meant. A long detour
measured `rusty_alloc` against the *C* heap_4 on silicon instead. Recorded
because the detour produced real results (they are above) but answered a
question nobody asked.

**Baseline, this session:** 127.74 Ir per operation over the differential's
own 20,000 operations, of which **69.74 is `heap4.rs`**.

### Where it goes

| Ir/op | line |
|---:|---|
| 13.54 | `if (raw & !ALLOCATED_BIT) >= size as u64 \|\| next == NONE` — the first-fit walk |
| 5.33 | `(u64::from_ne_bytes(*next), u64::from_ne_bytes(*size))` — the header read |
| 20.02 | `core/src/num/uint_macros.rs` — integer helpers, inlined |
| 3.00 | `core/src/slice/index.rs` — bounds checks |

The arena is `[u8; N]`, so every header access reassembles two `u64`s out of
bytes: a checked `usize::try_from`, a `get(base..)`, a `first_chunk::<16>`,
two more chunk `Option`s, then `from_ne_bytes`. **Eight operations to read
what C reads with two loads** — because a `forbid(unsafe)` crate cannot
reinterpret bytes as a struct.

### The obvious fix was already refuted, in the source, twice

Folding the alloc path's four reads of `chosen` and carrying `taken` in a
local is written up at the site with its numbers: `2,730,871 -> 2,776,099`
(+1.7%), and re-tested on a moved shape, `2,709,563 -> 2,745,401`. A patch
to do exactly that was written here and stopped by its own anchor assertion
before it could be applied. **The recorded-refutation habit paid for itself:
the comment is why no fourth measurement was spent on it.**

### So the arena itself was tried: `[u64; N]` instead of `[u8; N]`

This code never touches payload — the only two uses of `store` outside the
header functions take a pointer for the protector — so the arena can be
words and a header field can be an indexed load. `N` became the WORD count,
`words_for` computed it, `Self::BYTES` replaced the nine places `N` meant
bytes, and `read_word`/`write_word` stopped fetching a 16-byte chunk to
reach 8 of it.

**It was carried all the way to green**: the whole suite passes, including
`our_heap4_matches_the_c_kernels_operation_for_operation` — the byte-exact
differential against the C driver — so the change was behaviour-preserving.
Four stale-literal breakages were fixed on the way, each the same shape the
list campaign hit: a constant that had been a byte count and silently became
a word count (`ram_table`'s `row!`, the protector's out-of-arena offset, the
fixed-overhead `const` assertion, heap5's region bound).

Then it was measured:

| | before | after | |
|---|---:|---:|---|
| `uint_macros.rs` (the byte assembly) | 20.02 | **17.24** | **-2.8**, as predicted |
| `slice/index.rs` (bounds checks) | 3.00 | **17.51** | **+14.5** |
| `heap4.rs` | 69.74 | 71.33 | +1.6 |
| **TOTAL** | **127.74** | **142.96** | **+11.9%** |

**REVERTED.** The byte-assembly saving is real and arrived exactly where
predicted. It is swamped: indexing a `[u64]` from a byte offset shifts right
to get a word index and the addressing shifts left again, and LLVM cannot
fold that round trip through the bounds check between them. Five times more
was paid in checking than was saved in conversion.

That is the third refutation this file has earned, and it is the same law
the other two state: **removing a redundant read wins only when the read
costs more than the check that avoids it.** Here the read got cheaper and
the check got dearer, which is the same trade seen from the other side.

### What the probe BOUNDS, which is the useful part

The word arena failed, but it priced the idea it belonged to. The byte
assembly it removed was worth **-2.8 Ir/op** (`uint_macros` 20.02 -> 17.24),
and that saving arrived exactly where predicted. Everything else was the
bounds-check machinery going the other way.

So the untested variant above — one shared bounds path per header instead of
one per word — has a **ceiling of 2.8 Ir/op**, because that is the whole
prize the representation change was ever competing for. Against a 127.74
total that is **2.2%**. It is worth trying for 2%; it is not a route to 50%,
and nobody should spend a day on it expecting one.

### Why 50% is not available here at all

`heap_4` is first-fit over an address-ordered free list, and the census says
its cost IS the walk: **13.54 Ir/op on the walk's own compare**, plus the
header read at each step.

The walk's length is not ours to change. `heap4_differential` replays 20,000
operations against a trace from the C driver and requires the same block to
be chosen every time, so the free list's order and the first-fit rule are
both pinned. A rover, a size-bucketed list, a best-fit — every classic way to
shorten a first-fit walk changes WHICH block is returned, and the differential
exists precisely to catch that.

What is left after the walk is the per-step and per-call overhead, and the
three attempts on it now have numbers: the alloc-path fold **+1.7%** (twice),
and the word arena **+11.9%**. The representation lever is bounded at 2.2%.

That was first written as "the only 50%-sized lever in this file is the
walk, and the differential is what makes it immovable." **That premise was
wrong, and the arithmetic says so.**

| | Ir | share |
|---|---:|---:|
| total, the differential's own 20,000 operations | 2,554,892 | 100% |
| the first-fit walk's compare | 270,800 | **10.6%** |
| + EVERY header read, counted as walk (an upper bound; some are elsewhere) | 377,400 | **14.8%** |
| what a 50% cut would have to remove | 1,277,446 | 50% |

**Deleting the entire walk — a perfect O(1) index that returned the identical
block, differential intact — buys at most 14.8%.** The walk was never the
50% lever; it is a seventh of the cost. The rest is spread across the header
writes, the coalescing, the length and sentinel bookkeeping, and 20.02 Ir/op
of `core`'s integer helpers, none of which is individually near a half.

So the refutation does not rest on the differential pinning anything. It
rests on a decomposition: **there is no 50% in this file to find.** The
largest identifiable lever is a seventh, the three attempted ones measured
+1.7%, +11.9% and a 2.2% ceiling, and the size band named in the ask has no
pathology in it. Something would have to be made to cost less that nobody has
yet identified as costing anything — which is a reason to keep measuring, not
a route anyone can be pointed down today.

### The band itself was measured, and there is nothing special about it

"256-512 B" was taken literally and tested: the bench restricted to
`256..=512` over a 64 KiB arena, so essentially every allocation succeeds
(10,014 of them) and the run is not measuring a failing allocator.

| workload | `heap4.rs` | total |
|---|---:|---:|
| mixed, 1..600, 8 KiB arena | 69.74 Ir/op | 127.74 |
| **256..=512, 64 KiB arena** | **70.36 Ir/op** | 133.04 |

**The band costs what every other size costs.** There is no pathology at
256-512 to find and fix — the request size barely moves the per-operation
cost, because first-fit's work is the walk and the walk is a function of the
free list's shape, not of the size asked for.

That closes the last reading of the target. The size range in the ask does
not name a defect; it names a range, and the range is ordinary.

### What is still open

`slice/index.rs` at 17.51 Ir/op says the bound, not the arithmetic, is what
a word arena costs. A variant that keeps ONE bounds path per header — a
`get(w..).first_chunk::<2>()` shared by `read_word` and `write_word` rather
than a `get` each — was not measured, and is the obvious next probe for
anyone returning to this. It is recorded as untested, not as refuted.

## `Pool`: 55.2% in the 256-512 band, by answering a narrower question (2026-09-21)

Four attempts to make `heap4.rs` itself 50% faster are recorded above, and
the decomposition says why none of them could be: the largest identifiable
lever is the first-fit walk at **14.8%**, and a 50% cut needs 1,277,446 of
2,554,892 Ir removed. **There is no 50% in that file.**

There is one in the *question*. `heap_4` answers "any size, any order,
coalescing", and its cost is that generality — the band the ask names costs
70.36 Ir/op against 69.74 for a mixed 1-600 workload, because first-fit's
work is the free list's shape and not the size asked for.

**Most RTOS allocation does not ask the general question.** TCBs, queue
items, timer records and event blocks are one size, known when the system is
declared — which is what this package's charter already calls *static
allocation first-class*. `crates/rusty_rtos_heap-core/src/pool.rs` serves one
size from a pre-sized arena: no size to compare, no block to split, no
neighbour to coalesce, no list to walk. `alloc` is a pop and `free` is a
push.

### Measured, with work parity as the gate

`bench/pool-ir` is `bench/heap4-ir` with one thing changed — which allocator
answers. Same LCG, same seed, same 48-slot pattern, same 20,000 operations,
same alternation of take and give back.

| | allocations | frees | Ir total | Ir/op |
|---|---:|---:|---:|---:|
| `heap4.rs`, restricted to 256..=512, 64 KiB arena | 10,014 | 9,986 | 2,660,831 | **133.04** |
| **`Pool<512, 48>`, same workload** | **10,014** | **9,986** | **1,191,428** | **59.57** |

**Identical allocation and free counts** — 10,014 and 9,986 on both sides —
so the two counts are a comparison and not two numbers. **55.2% fewer
instructions.**

512 is the unflattering end of the band to measure at: a pool pays for the
size it was declared with whatever is asked, so the top of the range is its
worst case, and the general path's best relative showing.

### What it costs to have, stated rather than buried

One block size. A capacity fixed at compile time. No coalescing, and no
borrowing from a neighbour that has room. A pool cannot serve a request it
was not sized for. That is the whole trade: it is faster **because** the
question is narrower, not because the allocator is cleverer.

It is not a replacement for `heap_4` and does not touch it — `heap4.rs` is
untouched and its byte-exact differential still passes. It is the right tool
for the fraction of RTOS allocation that is fixed-shape, and that fraction
is most of it.

### The handle is the protector

`Slot` carries a generation and freeing bumps it, so a handle to a block that
has since been freed names a generation that no longer exists. In a crate
that cannot reach for `unsafe`, the handle is the only place a use-after-free
can be caught — the same discipline `Heap4`'s `Block` uses.

**Poisoned before it was believed.** Removing the generation bump does not
fail the double-free test — the `live` flag catches that on its own — it
fails `a_stale_handle_cannot_read_the_block_that_replaced_it`, which is the
generation's actual job: a block freed and immediately reallocated, with the
old handle still in hand. That is the test that had to fail, and it did.

### Gates

| gate | result |
|---|---|
| `pool` unit tests | 7 pass |
| the package's whole suite | 10 binaries green, including `heap4_differential` |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `unsafe` in `pool.rs` | none — the two matches are the words in its own doc comments |
| work parity with `heap4-ir` | 10,014 allocations and 9,986 frees on both arms |

## The ladder: the pool's win GROWS with size (2026-09-21)

`bench/bands-ir` runs the same workload up a ladder of size bands, one
binary, one 256 KiB arena, the band chosen at run time — so every rung sits
on the same geometry and the rungs can be read against each other. The
allocation and free counts are printed on every rung for the usual reason:
**10,014 and 9,986 on every rung and in both modes**, so the counts are a
comparison rather than a collection of numbers.

Taken as a SLOPE (20,000 against 40,000 operations), which cancels process
start-up and — the reason it matters here — the pool zeroing its arena:

| band | `heap4.rs` | `Pool` | win |
|---|---:|---:|---:|
| 256..512 | 121.89 | **44.00** | **63.9%** |
| 1024..2048 | 128.83 | **44.00** | **65.8%** |
| 4096..8192 | 131.32 | **44.00** | **66.5%** |

**The pool is 44.00 Ir/op at 512, 1024, 2048 and 4096 — the same number, to
the instruction.** That is what an allocator with no size-dependent work
looks like, and it is the property being bought: `alloc` is a pop and `free`
is a push whatever the block is.

`heap4.rs` climbs gently instead — 121.89 to 131.32 across a 16x range, +7.7%
— so **the advantage widens as the blocks get bigger**, from 63.9% at the
bottom of the ladder to 66.5% at the top. The higher bands are the better
case for a pool, not the worse one.

### Two instrument notes, both of which changed a number

**The slope supersedes the earlier total-count figures.** The first pool
measurement read 133.04 against 59.57 (55.2%) by counting whole processes.
Counting a process charges one-time work to the operations, and `Pool::new()`
zeroes its arena: the per-op cost appeared to CREEP with block size, 59.75 to
68.36, which a size-independent allocator has no business doing. The
arithmetic named it — 8.61 Ir/op x 20,000 is 172,200 for 172,032 extra bytes,
**one instruction per byte** — and the slope removed it. In a `static` pool,
which is how an RTOS would hold one, that zeroing is `.bss` and costs nothing
at all.

**And the first slope read 0.00 Ir/op, which is impossible.** The `pool` mode
takes its block size where `heap4` takes a range, so the operation count was
landing in the wrong argument position and both runs were the same length.
A zero slope is the instrument asking for help, not a result; the fix was one
positional argument.

## Hammering the higher bands: two refutations and a census (2026-09-21)

### There is no allocation volume in Kairos to pool

The pool is 66% faster per operation. Before wiring it anywhere, the obvious
question — **how often does anything actually allocate?** — was answered by
instrumenting `pvPortMalloc` in the C ABI seam with a power-of-two size
census and running the real workload: **25 unmodified C demo files, 49,922
kernel switches, all passing.**

```
pvPortMalloc size census (power-of-two buckets):
       5..8             2
      17..32            8
          total       10
```

**Ten allocations.** None in the 256-4096 band the pool serves.

The reason is structural: **the Kairos kernel is static.** `xTaskCreate`,
`xQueueCreate` and `xTimerCreate` take from the heap in C FreeRTOS; here they
come from declared arenas. Only a demo's own explicit allocation reaches
`pvPortMalloc`.

So the arithmetic that should have come first: 10 allocations x ~80 Ir saved
is **~800 instructions**, against a run doing ~50,000 context switches. The
pool remains correct, measured and useful to an application that allocates
fixed sizes through the heap seam — it has no hot caller inside Kairos, and
that is a fact about this kernel's design rather than about the pool.

### And the higher bands' extra cost is first-fit, not occupancy

`heap4.rs` climbs 121.89 -> 131.32 Ir/op from the 256..512 band to
4096..8192. The line census says where: the alloc walk's compare **+3.19**
and the free path's `while next < insert` **+1.16**.

The obvious explanation was arena pressure — 48 live blocks of 4,096 fill 75%
of a 256 KiB arena where 48 of 512 fill 9% — so a fuller arena, a longer free
list, a longer walk. **Refuted.** Re-run in a 2 MiB arena, holding the block
count and the pattern fixed so occupancy falls to 9% at the top rung:

| band | 256 KiB arena | 2 MiB arena |
|---|---:|---:|
| 256..512 | 121.57 | 121.89 |
| 1024..2048 | 128.53 | 128.83 |
| 4096..8192 | 131.02 | 131.77 |

An **eight-fold** change in arena size moves nothing. The free list has the
same shape either way, because the same pattern allocates and frees the same
48 slots.

What is left is the algorithm: the walk's condition is `size >= wanted`, so a
**larger request rejects more blocks and first-fit walks further.** That is
`heap_4` behaving exactly as `heap_4` does, and the differential requires it
to. There is no win in the higher bands to take.

### And the kernel's own hot path, looked at while there

`bench/kernel-ir`, BlockQ at 20,000 ticks, 121.59M Ir total. The harness owns
most of it — the trace formatter 48.69M (40%) and the scenario's `step`
18.98M — which is what the bench's own method line warns about. The kernel
starts at `switch_context`, 7.91M.

One thing did stand out: **`__memcpy_avx_unaligned_erms`, 6.72M (5.5%)**. Its
callers, which is the census that matters:

| caller | calls | Ir/call |
|---|---:|---:|
| the trace formatter's `event` | 380,954 | 12.9 |
| `Kernel::increment_tick` | 40,044 | **30** |
| `TickIsr::tick` | 20,022 | **30** |

**1.8M instructions of memcpy in the tick path**, two per tick from
`increment_tick` and one from `tick`, at 30 Ir each. `Trace::event` takes its
payload BY VALUE and `Event<'_>` is **40 bytes** — which is exactly what a
30-instruction copy looks like.

**Not taken, and the reason is the scope rule rather than the size.** A
shipped kernel traces with `NoTrace`, whose `event` has an empty body, so
LLVM drops the construction and the copy entirely: this cost exists only in
traced builds — the conformance corpus and the demos. Against that, taking it
by reference is a public trait signature change across **19 implementors and
46 call sites** for about 1.5% of a test run.

Recorded with its number so the trade is there if the corpus ever becomes the
thing worth speeding up: `Event` is 40 bytes, it is copied three times per
tick, and `&Event<'_>` is the fix.

## heap_4, -7.6% on the targets Kairos actually ships (2026-09-21)

Four attempts on `heap4.rs` are recorded above, all refuted, and a
decomposition saying there was no large lever left. **Every one of those
measurements was taken on a 64-bit host, and every Kairos target is
32-bit.**

Rebuilt for `i686-unknown-linux-gnu` — same source, same workload, same
checksum — the picture is a different one:

| | 64-bit host | **32-bit** |
|---|---:|---:|
| total | 127.74 Ir/op | **265.41** |
| `heap4.rs` | 69.74 | 146.63 |
| `core/num/uint_macros.rs` | 20.02 | 40.73 |
| **`core/convert/num.rs`** — the `u64`/`usize` conversions | **1.79** | **38.46 (14.5%)** |

`header_of`, `set_header` and `write_word` each began with
`usize::try_from(offset).unwrap_or(usize::MAX)`, and the walk reaches them
once per step. Where `usize` is 64 bits that narrowing is a no-op and the
compiler deletes it; where it is 32 bits it is a real check, on the hottest
path in the file.

### The fix, and the measurement that proves it

The three sites now call one `Self::index_of(offset)`:

| | baseline | after | |
|---|---:|---:|---|
| 64-bit | 2,554,892 | 2,554,892 | **byte-identical** — nothing given up on the host |
| **32-bit** | 5,308,232 | **4,903,077** | **-405,155 Ir, -7.63%** (265.41 -> 245.15 Ir/op) |

Checksum `44726496` and `allocations 8967 frees 8946` on every arm, so the
two counts are a comparison. `heap4_differential` — 20,000 operations
byte-exact against the C driver's trace — passes, along with the package's
other 9 test binaries, clippy and fmt.

### Why the cast is exact, and why that is a test rather than a comment

Every offset this module forms is bounded by the arena: it comes from a
`Block`, whose `offset` is a `u32`, or from arithmetic over such offsets and
sizes the arena already bounds. `N` is a `usize`, so an in-range offset fits
one on any target. `index_of` carries a `debug_assert` for that invariant —
live through every test run, including the differential's 20,000 operations,
where it never fired — and each caller's `.get(..)` is the second line.

### The lesson, which this project has now paid for three times

The first probe of this change measured **byte-identical** and would have
been recorded as a refutation. It was run on x86-64, where the thing it
removes costs nothing.

That is the same trap as `prim::fixed`'s small-step investigation, where a
64-bit sweep "refuted" `SMALL_SIZE_MAX` because the boundary is 1,024 there
and 512 on a 32-bit chip; and the same as `rusty_alloc`'s `direct[]` route,
which is 512 bytes on the part and 1,024 on the host. **A refutation is only
as good as the machine it was taken on, and for a `usize`-shaped cost the
host is the machine where the suspect is not at the scene.**

Every earlier figure in this file for `heap4.rs` is a host figure and is
labelled as one. The 32-bit column is the one that describes shipped code.

## The list at the TARGET's width: 0.711x the C (2026-09-21)

`heap4.rs` had a 7.6% win that measured byte-identical on the host, so the
same question was put to the list campaign, whose every figure was also a
64-bit one — and whose central decision, a power-of-two mask instead of a
bounds check, is exactly a `usize`-shaped tradeoff.

`bench/list-cost/run.sh` now builds and counts BOTH widths, with all four
arms checksum-gated:

| arm | per op |
|---|---:|
| C `list.c` `-O2`, 64-bit | 22.32 |
| C `list.c` `-O3`, 64-bit | 21.82 |
| **ours, 64-bit** | **18.80** (0.842x) |
| C `list.c` `-O2`, **32-bit** | **33.42** |
| **ours, 32-bit** | **23.75** (**0.711x**) |

**On the width Kairos ships, the list is 29% fewer instructions than the C** —
a wider margin than the host reported, not a narrower one. Both arms cost
more at 32 bits, and the C costs much more: +50% against our +26%. i686 has
half the general-purpose registers, and a pointer-walking list spills where
an index-linked one does not.

So the campaign's conclusion holds and understates itself. The decision it
turned on — §5.2 item 2, arena lists against intrusive ones — is reaffirmed
more strongly at the width that matters.

### What this changes about the instrument

Both widths are now reported by default, and the two new arms are gated on
the same checksum as the other two. A bench that only ever runs 64-bit cannot
report on the code that ships, and this project has now found two costs that
are invisible there — one worth 7.6% and one that moved a ratio from 0.842x
to 0.711x. `gcc-multilib` and `rustup target add i686-unknown-linux-gnu`
are the only requirements; the script says so and degrades to the 64-bit
arms when they are missing rather than failing.

## The geometry was a parameter the header ignored -- a defect, then a 17% win (2026-09-21)

Three things were asked for: re-test heap_4's host-measured refutations at 32
bits, attack `uint_macros`, and audit every bench for host-only blindness.
The second found a correctness defect, so that is reported first.

### 1. The word arena, re-tested -- REFUTED HARDER, and the open note closed

The `[u64; N]` arena was refuted at +11.9% on the host, with a note that the
representation might pay differently at 32 bits. Rather than redo a
1,000-line refactor to find out, `bands-ir hdrbytes|hdrwords` reads the same
headers both ways. **The byte array is the little-endian image of the word
array, so both arms must print the same checksum** -- they do, on all four
arms (`6551217753363456`).

| Ir per header read | bytes | words | delta |
|---|---:|---:|---:|
| 64-bit host | 10.00 | 10.00 | **+0.00** |
| 32-bit target | 21.00 | 21.00 | **+0.00** |

**Exactly zero, at both widths.** The representation was never the cost. The
+11.9% was entirely the extra bounds paths the refactor introduced, which
also closes the "keep ONE bounds path per header" note recorded as untested:
there is nothing to win there, because there is nothing being lost.

*(The first cut of this probe filled the byte array with a uniform `0x5A`, so
`from_ne_bytes` folded at compile time and both arms measured nothing. It
reported a tidy 10.00/10.00 too. A probe that cannot fail is not a probe.)*

### 2. `u64` on a 32-bit machine, priced

`heap4.rs` carried 83 `u64`s against 9 `u32`s. On the allocator's own
operation mix -- compare, add, subtract, mask, shift:

| one mixed step | u64 | u32 | u64 tax |
|---|---:|---:|---:|
| 64-bit host | 12.50 | 9.62 | +2.88 |
| 32-bit target | 18.50 | 11.00 | **+7.50** |

**2.6x more expensive on the machine that runs it.** *(The first cut charged
the `u32` arm for per-iteration conversions a real narrowing would not
perform, and measured it as DEARER at both widths. Each arm is now carried
end to end in its own width, with the wide checksum asserted to truncate to
the narrow one.)*

### 3. ** THE DEFECT: `LINK` was a parameter the header ignored**

`LINK` is `sizeof( BlockLink_t )` and `STRUCT_SIZE = align_up(LINK, ALIGN)`
is what `alloc` adds to a block's offset to reach the caller's payload. But
`set_header` wrote **two `u64`s -- sixteen bytes -- whatever `LINK` said.**

So at `LINK = 8`:

```
the payload starts at 8 but the header this block owns runs to 16;
the caller's first write lands on the size word at 8
```

The payload was handed out **starting on top of the block's own length**. The
caller's first write destroys it.

This is not a hypothetical geometry. `LINK = 8` is one pointer plus one
`size_t` -- **`sizeof( BlockLink_t )` on every 32-bit target FreeRTOS runs
on** -- and it is the geometry `heap4.rs`'s own test module and
`tests/protector.rs` are declared with. It was invisible because nothing in
the tree writes through an allocated pointer: the crate is `forbid(unsafe)`,
so no test can.

The comment above the constant stated the right law and then broke it:
*"Modelled at 64 bits because the oracle's `size_t` is 64 bits. The bit's
position is part of the geometry, not of the host."*

**Fixed properly rather than forbidden.** `WORD`, `NARROW`, `ALLOCATED_BIT`
and `NONE` are now associated constants derived from the geometry, and the
accessors branch on `NARROW` -- a `const`, so one arm survives
monomorphisation and the branch costs nothing. `GEOMETRY_FITS` refuses any
`LINK`/`ALIGN` pair that is neither 8 nor 16 **at compile time**.

Two tests: `the_payload_begins_after_the_header_it_follows` is the defect as
an assertion, and `the_narrow_header_survives_a_full_cycle` churns 24 blocks
and requires every byte back and the arena coalesced to one block.

### 4. The fix is also the win

| heap_4, 256..512 band | LINK=16 | LINK=8 | |
|---|---:|---:|---|
| 64-bit host | 119.16 | 119.38 | +0.18% |
| **32-bit target** | 230.65 | **191.46** | **-16.99%** |

Work parity exact: **10,014 allocations and 9,986 frees on all four arms.**
The header also halves, 16 bytes to 8, on every block.

**A host-only bench would have reported +0.18% and no reason to do this.**
That is the fourth time this session a real target win was invisible on the
host, and the first where the host number was the WRONG SIGN.

The wide path, which the byte-exact differential runs at, is unmoved in
behaviour -- `our_heap4_matches_the_c_kernels_operation_for_operation`
passes, checksum `44726496` on both widths -- and moved slightly in cost:
host 2,554,892 -> 2,517,317 (**-1.47%**), 32-bit 4,903,045 -> 4,915,227
(**+0.25%**). Recorded rather than rounded away.

### 5. The bench audit -- and the instrument that lied first

The first audit read every `run.sh` and reported **all 18 benches host-only.**
It was wrong: `ls "$d"*.sh | head -1` sorts `census.sh` before `run.sh`, so
it read the wrong file. The corrected answer is **4 of 18** have a 32-bit arm.

Building each IR bench both ways gives the census that matters -- a ratio far
from the pack is where a width-shaped cost hides:

| bench | 32/64 ratio |
|---|---:|
| `pool-ir` | **1.08x** |
| `kipc-ir` / `kobj-ir` | 1.24x / 1.25x |
| `kdelay-ir` / `khot-ir` / `ksched-ir` | 1.33x / 1.36x / 1.36x |
| `bands-ir` | 1.70x |
| **`heap4-ir`** | **1.95x** |

**The census named the defect independently.** `heap4-ir` is the outlier by a
wide margin, and it is where both the defect and the win were. `pool-ir` at
1.08x is nearly width-neutral because `Pool` indexes with `u16` -- which also
re-prices the pool against the general allocator: **55.2% fewer instructions
on the host, 73.9% at 32 bits.**

### 6. Five benches were broken, by me, and the gate could not see them

Every `rusty_rtos_kernel/bench/k*-ir` failed to compile -- on the host, not
just cross -- because they pass a literal `ITEMS` of 80, which stopped being
valid when the list migration made `N` a power-of-two SLOT count. They now
derive it: `{ list_slots_for(24, 32, 41) }`.

They are nested workspaces, so `kairos check`'s 18 packages never reached
them. **That is the second time this session a nested workspace hid a
breakage I caused** -- the first was `hosted/capi-host`.

## `MessageBufferDemo`: the 21st scenario, and a dead `#ifndef` worth 2,407 lines (2026-09-21)

The last of the four K2 scenarios that was buildable. `IntQueue` is out of
scope and `QueueSet` needs an owner decision; this one needed only work.

### It was never kernel work

Every API the scenario calls already existed and was already proved by
another scenario — checked BEFORE a line was written, because a missing one
would have made this kernel work rather than demo work. The byte arena's
reclamation was already built too: `take_bytes` serves first fit from what
deleted buffers returned, which is what lets the echo servers create and
delete a message buffer on **every** loop, as the C does to prove it leaks
nothing.

What was missing was the scenario: a `messagebuffer.rs` twin of the C's
~660 live lines, and its registration in five places.

### Roughly a third of the C file is dead code

`configSUPPORT_STATIC_ALLOCATION` is 0 and `configRUN_ADDITIONAL_TESTS` is
undefined, so 310 of the 971 lines never compile in. Establishing that first
meant not remaking the sender/receiver pair, the coherence actors, or the
static-allocation half of the still-running check.

### The C's own comments are stale, and the const is not

`mbBYTES_TO_STORE_MESSAGE_LENGTH` is `sizeof( configMESSAGE_BUFFER_LENGTH_TYPE )`,
which is `size_t` — **eight** bytes on the oracle's machine. The file's
comments are written as though it were four ("a maximum of 5 6 bytes items
can be added"). Taking the comment would have put three messages where five
were expected and diverged immediately. `LENGTH_BYTES` is read from
`PosixDemoConfig`, which already carried the right value.

### The divergence: a `#ifndef` block that cannot run

First conformance run agreed for **2,407 lines** and then put a tick two
events early. With `KAIROS_TRACE_EXITS=1` both sides agreed to the
instruction — exit #4108 — and then the C spent **1** charged exit where we
spent **4**.

The cause is four lines of C that never execute:

```c
xReturned = xMessageBufferSend( ..., mbMESSAGE_BUFFER_LENGTH_BYTES, mbDONT_BLOCK );
#ifndef configMESSAGE_BUFFER_LENGTH_TYPE
    ... three more sends ...
#endif
```

`FreeRTOS.h:2889` defaults `configMESSAGE_BUFFER_LENGTH_TYPE` to `size_t`
when the application has not set it, and `MessageBufferDemo.c` includes
`FreeRTOS.h` long before that `#ifndef` is read. **The macro is always
defined, so the block is unreachable** — one send there, not four.

Under sim contract v2 each of the three extra sends is a *blind call*: a
zero-wait send of a message too large to fit takes no critical section, and
`vPortKairosApiReturn` charges exactly one exit for that. Three extra blind
charges moved the tick, and nothing else about the scenario was wrong.

**The lesson is about reading C, not about the kernel**: a `#ifndef` on a
config macro tests whether the APPLICATION set it, and a kernel header that
supplies a default makes the guarded arm dead. Checking which of
`IsEmpty`/`SpacesAvailable`/`NextLengthBytes` take a critical section
(none do) is what narrowed it to the sends.

### Result

| | |
|---|---|
| `conform MessageBufferDemo` | **19,588 lines identical**, second attempt |
| `conform MessageBufferDemo --ticks 100000` | **1,017,915 lines identical** |
| the C's own verdict | pass, ticks=2000 yields=2560 exits=28097 |
| `conform --all` | **23 scenarios identical**, up from 22 |
| pinned | digest `0x5cb3_12f2_6ca3_18f9`, 620,657 bytes, all 22 pins pass |

The C oracle needs 1,000 ticks before its own check passes — the echo
servers spend the first 250 blocked on data that cannot arrive yet, by
design — so at the 200 ticks the trace command defaults to it reports
`fail`. That is the scenario working, not failing, and it is why the trace
is taken at 2,000.

## `QueueSet` and `IntQueue`: the last two, and two kernel defects they found (2026-09-21)

Both of K2's remaining scenarios. Neither was blocked on kernel FEATURES --
and both turned out to be blocked on kernel BEHAVIOUR that no other scenario
reached.

### `QueueSet`: the seed was the whole blocker

Upstream seeds its generator from the address of one of the sending task's
own stack locals, and that seed decides which of the three queues every write
goes to for the rest of the run. Reproducible on the C side, unknowable to a
second implementation, and not stable across builds.

`kairos oracle patch` now pins it to a constant. **This is not a new
decision**: `TaskNotify.c` already carried exactly this edit, for exactly
this defect, and the machinery to apply it was already there. A constant
changes nothing the demo tests -- which queue a write picks is arbitrary by
design, and the property under test is that ALL THREE get used, which
`xAreQueueSetTasksStillRunning` checks directly.

| | |
|---|---|
| `conform QueueSet` | **6,281 lines identical**, second attempt |
| `conform QueueSet --ticks 100000` | **318,143 lines identical** |
| pinned | digest `0x2f8a_570d_f721_e2bc`, 167,059 bytes |

### `IntQueue`: the file did not compile, and now it does

`IntQueue.c` includes `IntQueueTimer.h`, which no demo directory supplies
because every BOARD project writes its own. That is why it was out of the
corpus: **the file did not compile, so nothing downstream of it could be
judged**. `oracle/harness/IntQueueTimer.h` is this harness's, and 34 of 34
demo files now compile where it was 33.

**What it proves, and what it does not.** Every queue access the demo makes
-- two interrupt handlers, six tasks at three priorities, suspend/resume
sequencing, a duplicate-and-missing audit over a 200-entry log -- is
trace-identical. The property the demo was WRITTEN for is not: its own
comment says "the interrupts are prioritised such to ensure that nesting
occurs", and this port has one interrupt source and no nesting. Nothing in
the C detects the difference, because `xAreIntQueueTasksStillRunning` checks
only that the four counted tasks are cycling -- which is exactly why it is
written down here and in the header rather than left to be inferred from a
passing trace.

| | |
|---|---|
| `conform IntQueue` | **44,285 lines identical** |
| `conform IntQueue --ticks 100000` | holds **362,065 lines**, then diverges at tick 16,260 -- OPEN, below |
| pinned | digest `0x595e_402b_5576_5d65`, 1,279,335 bytes |

### KERNEL DEFECT 1: a queue call preempted at its sampling exit

The first `IntQueue` run agreed for **135 lines** and then recorded a
blocking send under the wrong task. The interleaved trace located it exactly:

```text
125 4 TASK_SWITCHED_IN H1QTx
    # body=FirstHigherFull pc=4        <- enters queue_send
126 4 TASK_INCREMENT_TICK 4            <- the tick fires INSIDE the send
134 5 TASK_SWITCHED_OUT H1QTx          <- the tick switches away
135 5 TASK_SWITCHED_IN H1QRx
136 5 BLOCKING_ON_QUEUE_SEND q1        <- the same call, still running
137 5 MOVED_TASK_TO_DELAYED_LIST H1QRx <- naming whoever is current NOW
```

`xQueueGenericSend` exits its sampling section before suspending the
scheduler to block, and **that exit can release a tick which switches the
caller away**. The C's thread stops inside the exit; everything below it runs
when the task is scheduled again, and so names the right task. Our kernel ran
straight on.

The cure already existed for the other half of the API: `stream_resume`, a
per-TCB marker `xStreamBufferSend` uses for precisely this case. `queue_send`
and `queue_receive` had **no equivalent**. Both now split at the sampling
exit and re-enter below it through `queue_resume`.

135 -> 2,325 lines on the send fix, then the receive fix took it to all
44,285. **No regression anywhere**: BlockQ 26,949, semtest 30,100, recmutex
27,739 and GenQTest 25,127 are unmoved, and they are the scenarios that live
on that path.

### KERNEL DEFECT 2: a refusal that charged the clock

`QueueSet` agreed for 30 lines and put a tick one event early.
`xQueueRemoveFromSet` tests both of its refusals **outside** the critical
section and only enters it to do the removal -- and `xQueueAddToSet`, three
functions up, puts its whole body including both refusals **inside** one
section. The asymmetry is upstream's, ours took the section unconditionally,
and `QueueSet`'s setup makes a deliberately-failing remove to prove a queue
cannot be removed from a set it is not in. One exit, one tick, thirty lines.

*(Checked before it was "fixed": the first guess was that `xQueueAddToSet`
had the same shape. It does not -- reading the C is what stopped a
non-defect being patched.)*

### Also added

`queue_is_full_from_isr` and `queue_select_from_set_from_isr`, both `&self`
or section-free as their C originals are, and both previously unreachable
because no scenario called them.

`KAIROS_TRACE_UNBUFFERED` drops the trace sink's capacity to one byte so a
diagnostic `eprintln!` interleaves with the trace in the right ORDER. That
ordering is the whole reason defect 1 was found rather than guessed at: it
showed the blocking bookkeeping happening with **no body step between it and
the tick**, which named the kernel instead of the scenario.

### Result

**`conform --all`: 25 scenarios identical to the C kernel**, up from 23.

### Open: `IntQueue` beyond 16,260 ticks

At tick 16,260 the C defers a waiting task's wake to the end of the interrupt
-- the `cTxLock` path, where a task holds the queue locked inside a blocking
receive -- and ours wakes on the first send because the lock is not held at
that instant. The deferral mechanism is implemented on both sides; what
differs is whether the lock window overlaps the tick, which is a timing
question a level below the one defect 1 fixed. Recorded as open rather than
pinned away: the scenario is pinned at the corpus standard of 2,000 ticks,
where it is exact.

## `IntQueue` at 100,000 ticks: a queue call has MORE THAN ONE resume point (2026-09-21)

The open item from earlier today. `IntQueue` was exact at the corpus standard
and held 362,065 lines at 100,000 before diverging at tick 16,260.

### The first reading of it was wrong

It looked like a `cTxLock` deferral difference -- the C waking a task at the
END of the interrupt where we woke it on the first send. That was a real
difference but it was DOWNSTREAM. Diffing on the exit column instead of the
event text found the actual first divergence 13 lines earlier, and it is not
an event difference at all:

```text
362051 | 16260 TASK_SWITCHED_IN L1QRx #258118      | 16260 TASK_SWITCHED_IN L1QRx #258118
362052 | 16260 QUEUE_RECEIVE_FAILED q2 #258121     | 16260 QUEUE_RECEIVE_FAILED q2 #258120
```

Same event, same tick, **one extra critical-section exit**. Everything above
is identical to the instruction.

### What the census said

`QUEUE_RECEIVE_FAILED` happens **16 times in 100,000 ticks**, and the cost of
each one -- exits from the line before -- separates cleanly:

| | cost 8 | cost 3 | cost 2 | cost 1 |
|---|---:|---:|---:|---:|
| ours | 10 | 5 | 1 | 0 |
| oracle | 10 | 0 | 4 | 2 |

**The ten expensive ones agree exactly; every cheap one costs us one more.**
A cheap failing receive is one the caller RESUMED into, and paying one extra
there means resuming at a coarser point than the C does.

### The mechanism, from the trace rather than from reading code

```text
361964 16256 TASK_SWITCHED_IN L1QRx #258061
361965 16256 TASK_INCREMENT_TICK 16256 #258064   <- the tick, inside the call
361966..74   the ISR
361975 16256 TASK_INCREMENT_TICK 16256 #258067   <- the SAME tick number, AGAIN
361976 16257 TASK_SWITCHED_OUT L1QRx #258067
```

**A tick number printed twice is the signature of a PENDED tick.** The
scheduler was suspended -- L1QRx was inside the `vTaskSuspendAll()` window of
its blocking receive -- so `xTaskIncrementTick` pended it, and
`xTaskResumeAll()` replayed it. Replaying it switched L1QRx away, with
`prvIsQueueEmpty` still to run.

So the preemption was **not** at the sampling exit. This morning's fix gave
the queue path one resume point, below that exit. `xTaskResumeAll` in the
timed-out branch is a second one, and there was nothing to name it with:
`queue_resume` was a `bool`.

### The fix

`QueueResume` is now a three-valued resume POINT -- `No`, `BelowSample`,
`BelowTimedOutResume` -- and the timed-out tail is split into
`queue_take_timed_out` so a call can re-enter there. The send path keeps one
point on purpose: its timed-out branch has nothing after `resume_all` that
costs an exit, so being switched away there changes no count.

| | |
|---|---|
| `conform IntQueue` | 44,285 lines identical |
| `conform IntQueue --ticks 100000` | **2,220,518 lines identical** |

**No regression**, and the check was made at 100,000 rather than 2,000
because this is the queue blocking path: BlockQ 1,344,460, semtest
1,503,634, recmutex 1,385,862, GenQTest 1,253,579, QPeek 486,871 and
QueueSet 318,143 lines identical. `conform --all` remains 25 scenarios.

### The transferable part

**A "one more exit" difference is not an arithmetic error, it is a resume
point.** The kernel's own `stream_resume` / `stream_timed` pair already said
so -- two one-shot markers for two different exits inside one stream-buffer
call -- and the queue path was given one marker for a call that has two.
Anywhere a call can be preempted at more than one section exit, the marker
has to say WHICH, and a `bool` cannot.

**And diff on the instrument, not the text.** The event stream agreed for
another thirteen lines after the counts stopped agreeing; reading the events
alone pointed at a `cTxLock` deferral that was a consequence, not a cause.

## `ApiSweep`: the oracle does not have to be a DEMO, and two poisons that prove it works (2026-09-21)

`docs/HOLES.md` H2 listed public kernel APIs that no corpus scenario reaches,
so nobody had asked whether the Rust answers what the C answers. Six of them
have a direct C twin and **no upstream demo calls any of them** — checked
against `FreeRTOS/Demo/Common/Minimal`, not assumed.

So there was nothing to port, and the hole's own note said closing them
"means writing a scenario rather than porting one" — which had been read as
*too expensive to do*. It is not, because of one thing:

> **The oracle does not have to be an upstream DEMO. It has to be the C
> KERNEL.**

`oracle/harness/ApiSweep.c` is the only scenario in this corpus whose C was
written here rather than compiled verbatim from FreeRTOS, and both files say
so at the top. It drives those six against **real FreeRTOS**, and the Rust
twin is diffed against it like any other scenario.

| | |
|---|---|
| `conform ApiSweep` | **4,898 lines identical, first attempt** |
| `conform ApiSweep --ticks 100000` | **243,101 lines identical** |
| `conform --all` | **26 scenarios**, up from 25 |
| pinned | digest `0x2438_48b2_612f_ff91`, 143,106 bytes |

### ★ The trace does not move, and that is the interesting part

Five of the six emit no trace event at all. It would be easy to conclude a
trace cannot judge them. It judges them **twice**, and the two halves catch
different things:

1. **The clock.** Every one takes a critical section, and an outermost exit
   is a sixteenth of a tick here. An API that takes a different NUMBER of
   sections than the C moves every event after it.
2. **The values.** The C side checks what each call answered and latches a
   failure into its own `xAreApiSweepTasksStillRunning`.

**Both were poisoned before either was believed**, because a differential
that has never failed is not evidence:

| poison | result |
|---|---|
| `timer_period` returns `period + 1` | `ours: KAIROS_RESULT ApiSweep **fail**` / `oracle: ... **pass**` |
| `set_trigger_level` accepts a level the buffer cannot hold | the same |

In **both** cases the trace was **byte-identical for all 4,897 lines** —
same ticks, same yields, same 3,505 exits — and only the verdict line
differed. So:

- the clock alone would have passed both defects;
- the value check alone is what caught them;
- and a scenario written without that second half would have shipped a wrong
  `xTimerGetPeriod` green.

The second poison is the one worth keeping. `xStreamBufferSetTriggerLevel`
promises two things — accept a level the buffer can hold, **refuse one it
cannot** — and the refusal is the half a port is likely to get wrong,
because it is the half no happy path exercises. It is checked here
explicitly, and removing the check is what the poison did.

### What closed and what could not

H2 closed at eleven, and closing it meant separating two claims it had
conflated. *Coverage* ("called by no unit test") was largely **stale** —
ten of the eleven had call sites; only `queue_send_to_front_from_isr` had
none. *Conformance* ("never compared to FreeRTOS") stood, and split five
ways: six closed by this scenario; `with_tick_hook` a **false positive** in
the hole's own list, since `Kernel::new` IS `Self::with_tick_hook(...)`;
`notify_value` proven under another name, its field reached by
`xTaskNotifyAndQuery` and `ulTaskNotifyValueClear` which `TaskNotify`
already compares; and `task_at`, `ready_cursor`, `ready_items` **have no C
twin at all** — asking a differential about them is a category error.

**"No evidence from the C differential" read as one defect and was five
different things.**

## K5b's blocker was stale, and the A/B says the item still is not met (2026-09-21)

K5's §6.1 checklist says of its two open items: *"Neither K5b item can be
moved by work in this repository."* One of them could.

### The recorded blocker, re-checked

> the only published radio pins `esp-hal ~1.1.0` + driver **0.3.0**, every
> Kairos S3 cell pins `esp-hal =1.2.1`, and `xtensa-lx-rt` is a `links` crate
> so cargo refuses the pair

That was measured on 2026-09-11 against `esp-radio 1.0.0-beta.0`. **A version
claim about a public registry is the most perishable kind of fact a plan can
hold**, and this one had gone off: `esp-radio` is now `1.0.0-beta.1` and
`esp-radio-rtos-driver` is `0.4.2`.

Resolved against our own cell, unchanged except for the radio lines:

```text
error: failed to select a version for `esp-radio-rtos-driver`
    ... required by package `esp-radio v1.0.0-beta.1`
versions that meet the requirements `^0.4.2` are: 0.4.2
  previously selected package `esp-radio-rtos-driver v0.4.1`
```

**No `esp-hal` complaint at all** — the whole recorded cause is gone, and the
only conflict left was our own `=0.4.1` pin. Bumping it to `=0.4.2`:

```text
Locking 193 packages to latest compatible versions
  Adding esp-hal v1.2.1 (available: v1.2.2)
```

`esp-hal 1.2.1` — our exact pin — beside `esp-radio 1.0.0-beta.1`. It builds,
exit 0.

### And then the A/B said the item is still not met

It would have been easy to tick the box there. The checklist says **LINKED**,
so the question is whether esp-radio is in the binary, and the honest test is
an A/B on a clean build with the artifacts compared byte for byte:

| | bytes | symbols |
|---|---:|---:|
| without `esp-radio` | 276,820 | 2,416 |
| with `esp-radio` | 276,820 | 2,416 |

The hashes differ and 64 symbols differ each way — **and every one of those
64 is one of our own names whose crate-disambiguator hash moved between
compilation sessions.** No esp-radio code is in the binary at all. LTO drops
it because nothing in the cell calls it, which is exactly what the original
note said and is still the live reason.

**Same size, different hash was the tell.** Two binaries that differ in
content but agree to the byte in length are almost never differing in code.

### What actually changed

The item moved from *blocked on a decision nobody could take* to *unblocked
work that needs a Wi-Fi controller path and the board to verify*. That is a
real change in a plan row that said the half could not be moved from here —
and it was found by re-running the claim rather than re-reading it.

## K8's hardening half, measured — and the tables were not reproducible (2026-09-21)

K8 asks for "every hardening row complete". Nobody had measured how far that
was, so the first move was to run the gate rather than read the READMEs.

### The fleet position

| package | v1.0.0 gates | overall |
|---|---:|---:|
| `rusty_rtos_demo` | 10/12 | 63% |
| `rusty_rtos_core` | 10/16 | 50% |
| **`rusty_rtos_kernel`** | **13/16** | **58%** |
| `rusty_rtos_port` | 10/16 | 42% |
| `rusty_rtos_backoff` | 7/12 | 46% |
| `rusty_rtos_heap`, `rusty_rtos-capi`, `rusty_rtos_json` | 7/16 | 31% |
| `rusty_rtos_mqtt`, `rusty_rtos_http`, `rusty_rtos_sntp`, `rusty_rtos_tcp` | 6/16 | 22% |

**K8's hardening clause is a long way out**, and now it is a number instead
of an impression.

### ★ The tables did not regenerate from their plans

`kairos harden --all` **updated ten of twelve READMEs** — which should have
been a no-op. It was not a no-op because it was a REGRESSION:

```text
- **Audited** 2026-09-16 (v0.1.0 release pass) · **v1.0.0 gates** 10/17 · [checklist](https://github.com/...)
+ **Audited** 2026-09-09 (survey)              · **v1.0.0 gates** 10/16 · [checklist](docs/plans/...)
```

Every README in the fleet claims an audit on **2026-09-16 (v0.1.0 release
pass)**, and **no plan in the repository records that pass** — the last stamp
in every plan is 2026-09-09, and the `**Audited**` metadata line the tool
actually reads still said 2026-09-09 too. The line was hand-edited into the
READMEs at release time and the source was never updated.

Three consequences, and the third is the one that matters:

1. The published status is **not reproducible** — regenerating it produces
   different numbers and a different date.
2. Running the tool **silently reverts** the published tables, which is how
   this was found: the regeneration was assumed to be a formality.
3. **A hardening status that cannot be regenerated is not evidence, whatever
   number it shows.** The whole point of generating the table from a plan is
   that the plan is the auditable artefact; a hand-edited README is an
   assertion.

The ten regressed READMEs were reverted rather than committed. **Which side is
true is an owner question** — whether a real re-audit happened on 2026-09-16
— and inventing an answer in either direction would be worse than recording
the disagreement. The kernel's is fixed at source and now regenerates.

### The one unit moved: the kernel, 10/16 → 13/16

`rusty_rtos_kernel/docs/threat-model.md` closes **H-01**, and with it two more
that were waiting on it: **H-20** (secrets) and **H-41** (residual risks).

The register is the part worth reading, because writing it is what forces the
admissions:

- **no privilege separation between tasks** — that is what an RTOS is, and
  the MPU package is what would change it;
- **Kani proves the data structures, not the running kernel** — measured, not
  asserted: lists, arenas and names converge in 2–4 s and pass, and anything
  that creates a task does not converge in 240 s;
- **the corpus is a floor** — trace-identity proves agreement on what the
  demos do, and two scenarios added this year found two kernel defects
  precisely because they reached what the demos did not.

Each row carries **the condition that closes it**, because a waiver without
one is a decision nobody revisits.

### ★★ And the library units' plans are the KERNEL's plan, unedited

The 22–31% figures are worse than low — **they are measuring the wrong unit.**
Every library package's `use-protection-please.md` was scaffolded from the
kernel's and never specialised, so:

| package | what its H-26 row says | what the package actually ships |
|---|---|---|
| `rusty_rtos_json` | *"no parser yet"* | a JSON parser, `jsontestsuite.rs`, `no_panic.rs` |
| `rusty_rtos_mqtt` | *"no parser yet"* | `connect`, `connack`, `disconnect`, `header` parsers |
| `rusty_rtos_http` | *"no parser yet"* | `headers`, `readheader`, `range` parsers |
| `rusty_rtos_tcp` | *"no parser yet"* | `address`, `checksum` parsers — and **fifteen defects found in the pinned C** |
| `rusty_rtos_heap` | *"no parser yet"* | an allocator whose input is a length from a caller |

Their H-01 evidence rows still describe *"a kernel with no network stack, no
filesystem and no dynamic loading"* — a sentence about the kernel, sitting in
the plan of a package whose entire job is to parse bytes off a wire.

**So the fleet percentages understate and misdescribe at the same time.** Some
gates are open that the evidence would close — the fuzz and no-panic suites
K7 built are exactly H-26's subject — and the threat sketches name the wrong
adversary for the unit they are in. A parser unit's highest-value attack path
is crafted input; the kernel's is a wrong scheduling decision. Copying one
into the other produces a plan that cannot be audited against.

**The lesson is the scaffold's, not the packages'.** `kairos new` gives a unit
a hardening plan so the gate exists from the first commit, which is right —
but a scaffolded plan is a TEMPLATE, and a template that is never specialised
reads exactly like a completed one. Nothing in the tooling distinguishes
"this row was considered and is genuinely N/A" from "this row is still the
kernel's".

## K3's remaining measurement half: three rows were already done, and a fourth is refuted (2026-09-21)

K3's row ends with *"What remains of the measurement half: tick-ISR and
ISR-to-task latency rows, absolute cycles, and the flash comparison."*
**All four were already done, and two of them are described as done earlier in
the same row.** A row long enough to contradict itself is a row nobody reads
to the end.

| the summary said remained | actually |
|---|---|
| tick-ISR row | **131 cycles** on silicon, 2026-09-11 |
| ISR-to-task latency | **949 cycles**, same cell, same day |
| absolute cycles | all three rows are cycles on a 240 MHz part |
| flash comparison | **15,950 vs 13,924, 1.15x** — and the row itself says it "did NOT need `rusty_rtos-capi`, which an earlier note assumed" |

### What actually remains, and an attempt at it

Two things: the **C6** clause of the hour (hardware, and the silicon hour is
already met on an S3), and **the C arm of the cycle rows** — the clause says
*vs the C demo*, and the silicon rows have no FreeRTOS arm beside them.

A host **work row** was built to stand in for that comparison: count
`xTaskIncrementTick` and `vTaskSwitchContext` in the C oracle under callgrind
against `increment_tick` and `switch_context` in our sim, on the same
scenario, divided by the number of calls.

**The work-parity anchor was perfect**, which is why the attempt got as far as
it did — both kernels call the tick and the switch **exactly the same number
of times** on the same scenario:

| scenario | tick calls | switch calls |
|---|---:|---:|
| PollQ | 2,001 / 2,001 | 81 / 81 |
| BlockQ | 2,810 / 2,810 | 4,826 / 4,826 |
| semtest | 2,000 / 2,000 | 3,296 / 3,296 |

And the ratios looked stable across four scenarios — tick 1.49x, 1.51x,
1.50x, 1.30x; switch 2.03x, 2.35x, 2.51x, 2.37x. Against K3's bars (tick and
switch both ≤ 1.25x) that would have been a finding against us, and it would
have been written up as one.

### ★ It is an artefact of FACTORING, not a measure of tick cost

The basis is self cost, and self cost is only comparable if both
implementations put the same share of the work in the function itself:

| arm | tick self | tick inclusive | self as % of inclusive |
|---|---:|---:|---:|
| C | 93,530 | 3,441,619 | **2.7%** |
| Kairos | 139,690 | 1,062,313 | **13.1%** |

**A self-cost comparison here compares 2.7% of one thing against 13.1% of
another.** The 1.5x measures how differently the two kernels factor a tick,
not what a tick costs.

Inclusive does not rescue it either, in the other direction: the C's tick
inclusive is **68% of the entire run**, because on the Posix port the tick
drags in the thread switch and the trace write — neither of which is kernel
work, and neither of which our runner arranges the same way.

**REFUTED, and recorded so nobody builds it again.** There is no defensible
host work row for a tick between these two kernels. The instrument is not the
problem; the boundary is. A tick has no agreed edge in either codebase.

**And that is precisely why the clause puts the cycle rows on a part.** On
silicon you bracket the code path between two reads of a cycle counter, and
the question "where does this function's work end" never arises — the
bracket answers it. The plan's reasoning survives the attempt to work around
it, which is the useful outcome.

*(One instrument fault caught on the way, by the parity gate rather than by
reading: the call-count parser missed callgrind cost lines beginning `+`,
`-` or `*` — relative subpositions — and reported the C arm as **zero**
calls. A ratio would have been printed against a zero denominator had the
script not refused to print one when the counts disagree.)*

## The C arm of K3's cycle rows was blocked on a chip; it is blocked on an SDK (2026-09-21)

The row above refutes a host stand-in for *"cycle rows vs the C demo"* and
concludes the comparison belongs on a part. The next question is which part,
and the plan has said C6 since it was written.

**Re-checking it moved the item off the hardware list.** Three probes, and
two of them refuted what I expected to find:

| checked | expected | found |
|---|---|---|
| does the pinned oracle have an Xtensa port? | no — ESP maintains its own | **yes**, `portable/ThirdParty/GCC/Xtensa_ESP32`, in the upstream tree |
| does that port support the **S3** (LX7), or only the original ESP32 (LX6)? | LX6 only | **S3 named explicitly** in four files |
| is there an S3 C compiler on this box? | no | **`xtensa-esp32s3-elf-gcc`, installed** |

The S3 references are not incidental — each is a chip branch that pulls a
different ROM or clock header:

```
include/FreeRTOSConfig_arch.h:81   #elif CONFIG_IDF_TARGET_ESP32S3
port.c:73                          #if   CONFIG_IDF_TARGET_ESP32S3
port_common.c:28                   #elif CONFIG_IDF_TARGET_ESP32S3
xtensa_init.c:56                   #elif CONFIG_IDF_TARGET_ESP32S3
```

So the C arm can be built for **the part already on COM4** — the same XIAO
ESP32-S3 that produced tick 131 / switch 623 / wake 949.

### ★ And it would be the RIGHT C, which an IDF build would not be

This matters more than the convenience. **ESP-IDF bundles a FORK of
FreeRTOS**, not the upstream kernel. Our whole conformance claim is against
`oracle/FreeRTOS-Kernel` at **V11.3.1** (`3a22924`), which is where the 26
byte-identical scenarios come from. A cycle row measured against IDF's fork
would compare Kairos to a kernel it has **never been proved equivalent to** —
the same class of mistake as the self-vs-inclusive basis above, one level up.

Building the oracle's own port keeps the comparison on the kernel we conform
to, with IDF supplying only the platform.

### The actual blocker, named

The port includes **eighteen ESP-IDF headers**:

```
sdkconfig.h  esp_idf_version.h  esp_attr.h  esp_crosscore_int.h
esp_debug_helpers.h  esp_err.h  esp_freertos_hooks.h  esp_heap_caps.h
esp_heap_caps_init.h  esp_int_wdt.h  esp_intr_alloc.h  esp_log.h
esp_panic.h  esp_rom_sys.h  esp_system.h  esp_task.h  esp_task_wdt.h
esp_timer.h
```

It cannot compile without the SDK supplying startup, clock init, interrupt
allocation, the watchdogs and the flash bootloader. **So the item is an SDK
install (~2 GB), not a hardware purchase** — left undone here deliberately,
because a 2 GB SDK is an environment change, and the owner decides those.

**Net effect on the plan:** the C6 is no longer the blocker for the
*comparison*; it remains the blocker for the *second architecture of record*.
Those were one line and are now two, and only one of them needs money.

## K3's "vs the C demo" row exists, on rv32, in retired instructions (2026-09-21)

The row above refutes a host stand-in for this comparison and concludes the
boundary has to be stated rather than inferred from a call graph. **This is
that comparison, built the way the refutation said to build it.**

`bench/tick-work/run.sh`. Two arms, one machine, one instrument, one script:

| | |
|---|---|
| **C** | FreeRTOS **V11.3.1** out of the pinned `oracle/` checkout, **unmodified**, with the oracle's own first-party `portable/GCC/RISC-V` port, linked bare-metal for QEMU `virt` |
| **Kairos** | `rusty_rtos_kernel/firmware/riscv32-qemu-tick-work`, same machine, same instrument, matched config |

The instrument is `minstret` under `-icount shift=0` — an architectural CSR,
not optional debug hardware, and exactly reproducible where a clock is not.
So this is a **work** row on the same basis as `bench/switch-cost`. **Cycles
still come from a part**, and that has not changed.

### The rows

```
retired instructions per call, rv32imac, -O2 both arms, no LTO either
row                FreeRTOS     Kairos    ratio
tick_idle                15         56    3.73x
tick_delayed             15         56    3.73x
switch_select            27         79    2.93x
```

**Two of the three are against us.** They are findings, not caveats.

### ★ But quoting `switch_select` alone is quoting a third of the answer

A switch is selection **plus** the register file, and the two kernels put
their weight in opposite halves. `bench/switch-cost` already prices the other
half from the same oracle — cooperative 30 against 83, preemptive 74 against
83 — because FreeRTOS routes yields through the `ecall` trap and saves 28
GPRs, where a Kairos yield happens at a call site whose caller-saved half the
ABI has already declared dead.

Added up:

| whole switch | FreeRTOS | Kairos | |
|---|---:|---:|---:|
| cooperative | 27 + 83 = **110** | 79 + 30 = **109** | **0.99x — parity** |
| preemptive | 27 + 83 = **110** | 79 + 74 = **153** | **1.39x against us** |

Two true statements that say different things, which is why both are here.

### The two gates, and what each catches

**PARITY** — identical anchors or no comparison:
`ANCHOR samples=512 tick_calls=1024 switch_calls=512 tick_count=1024`, and
`tick_count` is read back from the kernel so a tick that took a
short-circuit path cannot pass. That is not hypothetical:
`xTaskIncrementTick` opens with `if( uxSchedulerSuspended == 0 )` and a
suspended call merely does `++xPendedTicks` — about the right size to look
like a plausible answer.

**POISON** — every arm built twice, the measured call made once per bracket
and then twice, and every row must move by one call's worth:

```
                     C x1     C x2  Rust x1  Rust x2
tick_idle              15       30       56      112
tick_delayed           15       30       56      112
switch_select          27       52       79      155
```

A bracket measuring the loop instead of the callee does not move. Without
this the numbers are plausible and unproven.

### ★★ The defect the gap found: a hand-rolled twin of a sink the crate ships

The first Kairos `switch_select` read **305**, an 11.3x. A number that far
past prediction gets the same suspicion as one far short of it, so the
boundary was checked before anything was published — and the check found a
**harness** defect, not a kernel one.

`rusty_rtos_core::trace` **already ships** a `NoTrace` carrying
`const WANTS_NAMES: bool = false`, which is exactly what lets the kernel skip
the task-name lookup and its UTF-8 validation. The cell had hand-rolled its
own `NoTrace`, shadowing it and inheriting the trait default of **`true`** —
so every switch built a 16-byte name and handed it to a sink that dropped it.

Using the one that already existed: **305 -> 79, a 3.86x on the row, with no
kernel change at all.**

**This is not confined to one cell.** Thirteen sites in the repo impl `Trace`
by hand; eleven of them never set `WANTS_NAMES`, and every one whose body was
read is a no-op or count-only sink that never looks at a name:

| site | sink |
|---|---|
| `rusty_rtos_kernel/firmware/xiao-s3-cycles` | no-op — **and its published silicon switch row was measured through it** |
| `rusty_rtos_kernel/firmware/xiao-s3-signing` | no-op |
| `rusty_rtos_port/firmware/mps2-an385-qemu-kernel` | no-op |
| `rusty_rtos_port/firmware/mps2-an385-qemu-preempt` | no-op |
| `rusty_rtos_port/firmware/host-kernel` | no-op |
| `rusty_rtos_port/firmware/xiao-s3-radio` | no-op |
| `rusty_rtos-capi/firmware/mps2-an385-qemu-capi` | no-op |
| `rusty_rtos-capi/hosted/capi-host` | no-op |
| `rusty_rtos_kernel-core/src/proofs.rs` | no-op |
| `rusty_rtos_kernel-core/src/system.rs` (test) | no-op |
| `rusty_rtos_kernel-core/tests/no_panic.rs` | counts only |

Only the two tickless cells set it. `xiao-s3-cycles` is fixed and
re-measured on the board; **the other ten are surveyed and left for the
owner**, since they sit in four packages and this session was asked for the
rv32 row.

The general shape is the one the house skills already name: the good
implementation existed, was tested, and the hot path called a worse twin that
someone wrote because they did not know it was there.

### Four instrument faults caught on the way

1. **A run with no output read as a hang.** QEMU here is a native Windows
   binary, so `-serial file:/tmp/...` is not path-translated the way `-kernel`
   is; the file is never created and the program looks dead. `run.sh` now
   asks MSYS what Windows calls the directory (`pwd -W`).
2. **Piped stdout is block-buffered**, so a run that hangs loses everything it
   printed before hanging — which is exactly the output that says where it
   hung. Serial goes to a file now.
3. **The oracle's RISC-V port needs the application to install `mtvec`.**
   Nothing in the port does it, so the first `taskYIELD()` traps to address 0.
   Two instructions in `start.S`, and until they were there the scheduler
   started and never came back.
4. **The kernel provides `vApplicationGetIdleTaskMemory` itself** in V11.3.1
   (`configKERNEL_PROVIDED_STATIC_MEMORY`); supplying one is a duplicate
   symbol, not a missing one.

## The silicon cycle rows were measured through the harness defect, and are corrected (2026-09-21)

The rv32 work row above found that eleven sites hand-roll a `NoTrace` that
shadows the one `rusty_rtos_core::trace` ships and inherits
`WANTS_NAMES = true`. **`xiao-s3-cycles` is one of them, and its rows are
K3's silicon cycle rows.** So they were re-measured on the board.

| row, ESP32-S3 at 240 MHz | through the defect | corrected | |
|---|---:|---:|---:|
| tick (nothing delayed) | 131 | **54** | 2.43x |
| tick (one task delayed) | 129 | **55** | 2.35x |
| switch | 623 | **166** | 3.75x |
| ISR-API wake to the task holding the value | 949 | **430** | 2.21x |

**No kernel code changed.** The cell stopped building a 16-byte task name and
UTF-8 validating it for a sink whose body is `{}`. In wall terms the tick went
from ~545 ns to ~225 ns and the switch from ~2,595 ns to ~691 ns.

### Why this correction is believable, given it is entirely in our favour

A result this large in your own favour earns more scrutiny than one against
you, not less. Four things carry it:

1. **The mechanism is named and was found before the number** — the boundary
   check was run because 11.3x on rv32 was implausible, not because a nicer
   number was wanted.
2. **Cross-architecture agreement.** The same one-line fix moved the rv32
   selection row **305 -> 79 retired instructions (3.86x)** and this cell's
   switch **623 -> 166 Xtensa cycles (3.75x)**. Two instruments sharing no
   code, on two architectures, agreeing on the size of the defect.
3. **It reproduces.** Two reflash-and-reset cycles returned 54 / 55 / 166 /
   430 identically, and the cell's own seven self-checks pass.
4. **The rv32 arm is poison-proven** — doubling the measured call doubles the
   row on both arms — so the bracket is known to enclose the callee.

### ★ It also WITHDREW a finding, which is the part worth keeping

This cell's README carried "the refuted check, which is the most interesting
line here": the delayed tick measured CHEAPER than the idle one (129 against
131), the assertion that said otherwise was replaced, and a paragraph
explained the inversion in terms of ready-list population dominating the
delayed-list check.

Corrected, it is **55 against 54** — the delayed tick costing one cycle more,
the obvious way round. **The inversion was a 2-cycle wobble inside a ~77-cycle
overhead that should never have been in the measurement.** The explanation was
a good story about an artefact.

The cell now prints whichever sentence its own numbers support instead of
carrying the conclusion in prose, because the prose was wrong for ten days and
was confident throughout.

### And a cross-check that turned out not to be one

The README claimed agreement with the sibling cell `xiao-s3-signing`: that
cell measures a scheduling round at **1,995 cycles**, and the old rows
predicted `2 x 623 + (949 - 623) ~= 1570` — about 20% apart, which read as two
instruments agreeing.

**They were two measurements of the same defect.** `xiao-s3-signing`
hand-rolls the same shadowed `NoTrace`. Against the corrected rows the
prediction is `2 x 166 + (430 - 166) ~= 596`. Two instruments agreeing is
worth as much as either alone only when they are independent, and a
common-mode defect makes them one instrument wearing two hats. The sibling has
been fixed and a re-measurement is running; until it reports, this README
carries a prediction rather than a cross-check.

## The sibling cell fell too, and the "cross-check" was common-mode (2026-09-21)

`xiao-s3-signing` hand-rolls the same shadowed `NoTrace` as `xiao-s3-cycles`,
so it was fixed and re-run on the board. It is K5a's measurement clause.

| | through the defect | corrected | |
|---|---:|---:|---:|
| one scheduling round | 8,313 ns / **1,995 cycles** | 3,724 ns / **893 cycles** | **2.23x** |
| as a share of one P-256 signature | 88 ppm | **39 ppm** | |

Work parity held across the correction — `bare=100s/100v/100ok
scheduled=100s/100v/100ok`, and `switches=40000 per_round=2`, counted rather
than assumed, so the round really is two switches either way.

**Three cells, three instruments, one defect.** The fix moved
`riscv32-qemu-tick-work` **3.86x** (retired instructions, rv32),
`xiao-s3-cycles` **2.2-3.8x** (Xtensa cycles), and this cell **2.23x**
(amortised microseconds). Three arrangements sharing no measurement code
agreeing that the overhead was real is what makes this a defect rather than a
story.

### The honest state of the cross-check

The cycles cell predicted the signing round as `switch + wake`:

| | prediction | measured | apart |
|---|---:|---:|---:|
| through the defect | 1,570 | 1,995 | 1.27x |
| corrected | 596 | 893 | **1.50x** |

**The agreement got looser, not tighter, and that is recorded rather than
smoothed.** The model is crude — a round is a queue send, a queue receive and
two switches, and `switch + wake` does not account for the receive-side call
— so 297 cycles of slop is unsurprising. What it is NOT is independent
confirmation, and the README that claimed it was has been corrected.

### K5a's conclusion is unchanged and stronger

The clause asks what a Kairos kernel costs a real Janus workload. It was 88
ppm of a signature; it is **39 ppm**. A scheduler that was already noise
against P-256 is half as much noise. Nothing about the finding turned on the
old number being right, which is the only comfortable thing about having
carried it for ten days.

## The release gate found StreamBufferDemo diverging on BOTH emulators (2026-09-21)

Run before pushing 0.2.0, not after. `rusty_rtos_demo/firmware/*-qemu-corpus`
on Cortex-M3 and on RV32:

```
RESULT: FAIL -- 1 of 25 scenarios diverged
```

**Identically on both architectures**, which already rules out anything
architecture-specific. The scenario is `StreamBufferDemo`, and the failure
report is unusually narrow:

```
StreamBufferDemo       FAIL
           ticks      2000 want     2000
           yields     2424 want     2424
           exits     28002 want    28002
           lines     20927 want    20927
           bytes    676856 want   676662
           digest 0xafab98d0be27e7d8
           want   0xf1592634a152db89
```

**Every counter matches and the line count matches.** Same number of events,
in the same order, at the same sim times. Only the trace TEXT differs, by
**194 bytes across 20,927 lines** — about one extra character on one line in a
hundred.

### It is NOT today's work

Proven rather than assumed. The three files this session touched in
`rusty_rtos_kernel-core` (`proofs.rs`, `system.rs`, `tests/no_panic.rs`, each
gaining `const WANTS_NAMES: bool = false`) were stashed and the cell re-run:
**byte-identical failure**, 676,856 against 676,662 with the same digest. Two
of the three are `#[cfg(test)]` or Kani-only in any case.

### What it points at

The host passes. `kairos conform --all` is **22 scenarios identical to the C
kernel**, exit 0, and that includes `StreamBufferDemo` against the C oracle
itself. The cells compare against the HOST's pins. So the disagreement is
**host against target**, not either against C:

| | |
|---|---|
| host, 64-bit | 676,662 bytes — agrees with the C kernel |
| M3 and RV32, 32-bit | 676,856 bytes — same lines, same order |

A trace that is longer on the NARROWER machine, at one character per hundred
lines, is a value in the trace text whose digit count depends on
`size_of::<usize>()`. A stream buffer's free space is computed from its
capacity minus a header, the header is four bytes smaller on a 32-bit target,
so the printed space is four larger — and every time that crosses a power of
ten it costs a character.

**That is the pointer-width detector firing for the fifth time on this
project**, and the first time it has been caught by a conformance cell rather
than by a benchmark ratio.

### Status

**Open, recorded, not hidden.** It is a trace-text defect and not a scheduling
one — the schedule is provably identical, which is what the counters are for.
The published crates' own gates pass (`kairos check`, 4 packages), the host
conformance passes, and 0.2.0 ships with this named in the demo's Known gaps
rather than smoothed out of the README.

The claim that had to change: the corpus READMEs said 18/18 on each emulator,
measured when the corpus was 18 scenarios. It is now 25, and the honest
number is **24 of 25**.

## H7: the whole Kani table at a 240 s bound — 12 of 34, and two dead hypotheses (2026-09-21)

The cheapest-first sweep finished: every harness in
`rusty_rtos_kernel-core/src/proofs.rs` run with `BOUND=240`.

**12 SUCCESSFUL, 22 TIMEOUT, 34 total.**

| converges | seconds | | times out at 240 s |
|---|---:|---|---|
| `lists_take_any_argument` | 4 | | every `task_*` harness — all eleven |
| `insert_end_keeps_the_item_value` | 2 | | `queue_generic_create`, `_reset`, `_send`, `_peek`, `_receive` |
| `an_arena_only_resolves_its_own_handles` | 3 | | `queue_messages_waiting_and_spaces_available` |
| `a_fresh_kernel_is_not_running` | 3 | | `queue_create_counting_semaphore` |
| `a_name_is_truncated_not_overrun` | **80** | | `queue_create_mutex_and_get_holder` |
| `typed_send_never_loses_the_value` | 2 | | `queue_take_and_give_mutex_recursive` |
| `typed_receive_gives_back_what_was_sent` | 2 | | `queue_generic_send_stale_handle` |
| `typed_a_refused_send_does_not_disturb_a_queued_value` | 2 | | `a_queue_starts_empty`, `a_created_task_is_counted`, `three_tasks_are_three` |
| `queue_receive_from_isr` | 7 | | |
| `queue_generic_send_from_isr` | 13 | | |
| `queue_semaphore_take_and_give_from_isr` | 12 | | |
| `queue_unlock_leaves_the_queue_usable` | 11 | | |

### ★ Two explanations were offered and both are REFUTED by the table

**"Harnesses that touch kernel task state do not converge."** Stated out loud
mid-sweep, from the first thirteen rows. **Wrong.** Four queue harnesses build
a kernel with `ready()`, create a queue, and finish in 7-13 seconds.

**"The boundary is BLOCKING — the FromISR paths never block, so they have
nothing to reason about."** It survives the FromISR trio and dies on the
fifth row: `queue_unlock_leaves_the_queue_usable` calls the blocking send and
converges in 11 seconds, while `queue_generic_create` calls nothing that
blocks and times out.

A cleaner-looking pair kills it outright. `queue_receive_from_isr` (7 s) and
`queue_peek` (timeout) have the SAME setup — `ready()`, `queue_create(2)`, an
assertion on `queue_messages_waiting`. Only the operation differs, and the
difference is not blocking.

### What this does establish

- A **reproducible convergence table** at a stated bound, which is what H7
  asked for and did not have.
- The cost is **not** uniform in "kernel-ness": the spread inside the queue
  family alone is 7 seconds to past 240.
- `a_name_is_truncated_not_overrun` at **80 s** is the one row that converges
  slowly rather than either quickly or not at all, so it is where a bound
  sweep would actually show a curve.

### What comes next, and what NOT to do

Do not guess the mechanism from harness bodies — that has now failed twice in
one sitting, and a wrong refutation is permanent where a wrong keep is not.
The next probe is **Kani's own per-property statistics** (VCC counts and
solver time per harness), which this sweep discarded by keeping only the
verdict and the wall clock. That is one flag and one re-run, and it replaces
speculation with the solver's own account of where it went.

## K3's one-hour clause was closed against 18 scenarios; the corpus is 25 (2026-09-21)

`bench/soak-each/run.sh` carries the scenario list **as a hardcoded string**.
It holds eighteen names. The corpus the cells actually run holds
twenty-five.

```
in the corpus, never soaked on either emulator:
  AbortDelay  ApiSweep  IntQueue  MessageBufferDemo
  QueueSet    StreamBufferDemo    TaskNotify
```

So "the emulator hour is CLOSED on both" — recorded here on 2026-09-11 with
RV32 18/18 in 58 minutes and M3 18/18 in 75 — was true of the corpus as it
then stood and has been quietly false since the seventh scenario joined.
**Nothing failed. The clause simply stopped covering what it claimed to
cover**, which is the more dangerous of the two, because a failure is loud and
a shrinking denominator is not.

### The defect is the hand-maintained list, not the seven scenarios

A list that must be edited whenever the corpus grows will eventually not be,
and no gate was comparing the two. The script now **derives the corpus from
the cell** — the ordinary 2,000-tick run prints one line per scenario — and
fails if its own list and that set disagree, in either direction:

```
FAIL: this script's scenario list has drifted from the corpus.
  in the corpus, never soaked: ...
  soaked, no longer in the corpus: ...
```

Both directions matter. A name that leaves the corpus and stays in the list
is a run that proves nothing about code that exists; a name that joins the
corpus and never reaches the list is the case that actually happened.

### How this was found

Not by reading the script. The 0.2.0 release gate re-ran both corpus cells,
which reported **25** scenarios where the READMEs claimed 18, and the arithmetic
did not work. The same re-run also found `StreamBufferDemo` diverging on both
emulators.

**Two findings out of one command that nobody had run in a while**, which is
the argument for running the gate before a release rather than trusting the
last recorded number.

### Status

The seven are being soaked on RV32 now, at 3,600,000 ticks each, one scenario
per run. Until they pass, K3's hour is **18 of 25 on each emulator**, and the
plan says so rather than carrying the older, rounder claim.

## ★ The soak harness counted a FAIL as a pass (2026-09-21)

Found immediately after the scenario-list drift above, by soaking three of the
seven scenarios that had never had the hour. The run printed:

```
AbortDelay             FAIL  [10s]
ApiSweep               ok    ticks=3600011 ...
QueueSet               ok    ticks=3600049 ...

RESULT: FAIL -- 3 passed, 0 without a verdict (of 18)
```

**Three passed, with a FAIL on screen.** The verdict test was:

```sh
out=$(... | grep -E "^$s " || true)
if [ -n "$out" ]; then pass=$((pass + 1)); else ... fi
```

It asked whether the cell printed **a line beginning with the scenario's
name**. `AbortDelay  FAIL` is such a line. So the harness could not tell `ok`
from `FAIL`, and every failure in its history would have been tallied as a
pass.

**A harness that cannot tell ok from FAIL is not a weaker gate; it is not a
gate.** The plan already warns that "a gate treating silence as success would
have certified it" — this is the sharper version, treating an explicit failure
as success.

### What it costs the record

The recorded "RV32 18/18 in 58 minutes, M3 18/18 in 75" proves that each of
eighteen scenarios **emitted a line**. It does not prove that each said `ok`.
Those runs may well have been genuinely 18/18 — nothing here shows otherwise —
but the evidence is weaker than the number implied, and the number is the only
thing anyone reads.

### Fixed, and shown to work

The verdict is now the word, and the denominator is counted rather than
written down (it was hardcoded at 18 in three places while the list had grown
to 25). Same three scenarios, before and after:

| | before | after |
|---|---|---|
| summary | `3 passed, 0 without a verdict (of 18)` | `2 of 3 passed, 1 did not` |
| exit code | **0** | **1** |

### And a real result underneath it

`AbortDelay` genuinely fails the hour, and not by hanging: it reports
`pass=false runaway=false ticks 3600009 of 3600000`. It reaches the full
3,600,000 ticks and its own check task declines to say it is passing. That is
consistent with `AbortDelay` being the corpus's known-divergent scenario — the
C harness keys a queue's trace ordinal on its malloc address — but the hour is
a LIVENESS claim, so this is a second, separate failure of that scenario and
it is now visible instead of counted as a pass.

`ApiSweep` and `QueueSet` pass the hour on RV32: 8,749,100 and 11,447,794
trace lines at 3,600,000 ticks.

## The seven unsoaked scenarios, measured on RV32: six pass, AbortDelay does not (2026-09-21)

The hour, at 3,600,000 ticks, one scenario per run, with the corrected
harness:

| scenario | verdict | ticks | trace lines |
|---|---|---:|---:|
| `ApiSweep` | ok | 3,600,011 | 8,749,100 |
| `QueueSet` | ok | 3,600,049 | 11,447,794 |
| `TaskNotify` | ok | 3,600,094 | 6,019,524 |
| `MessageBufferDemo` | ok | 3,600,000 | 36,672,178 |
| `StreamBufferDemo` | ok | 3,600,000 | 41,689,251 |
| `IntQueue` | ok | 3,600,068 | 79,912,747 |
| `AbortDelay` | **FAIL** | 3,600,009 | `pass=false runaway=false` |

`IntQueue` alone writes **2.59 GB** of trace in 78 seconds.

### ★ StreamBufferDemo passes the hour while failing conformance, and that is EVIDENCE

Earlier today the same scenario was found diverging on both emulators at
2,000 ticks: every counter and the line count matching, 194 bytes of trace
text differing. The reading offered was that this is a value whose digit
count depends on `size_of::<usize>()`, reaching the trace text — a 32-bit
target defect, not a scheduling one.

**The hour tests that reading and it survives.** The hour is a LIVENESS
claim: it asks whether each scenario's own check task still reports running
after an hour of simulated time. A wrong schedule would be expected to show
up there. `StreamBufferDemo` runs the full 3,600,000 ticks and passes, across
41.7 million trace lines.

Two claims about the same scenario, failing one and passing the other, is
what a text defect looks like and is not what a scheduling defect looks like.

### AbortDelay fails the hour, separately from its known conformance gap

`pass=false runaway=false ticks 3600009 of 3600000`. It reaches the full
length — it does not hang, and it does not run away — and its own check task
declines to report passing. `AbortDelay` is already the corpus's known
divergent scenario for a CONTRACT reason (the C harness keys a queue's trace
ordinal on its malloc address), but that is a conformance gap. This is a
second, independent failure of the same scenario on the liveness claim, and
it was previously invisible because the harness counted it as a pass.

### What the hour's denominator honestly is now

**RV32: 6 of the 7 previously-unsoaked scenarios pass, and AbortDelay fails.**

The other eighteen are NOT re-stated here. They were measured with the
harness that could not tell `ok` from `FAIL`, so "18/18 in 58 minutes" proves
eighteen lines were printed. They may all have said ok. Nothing here suggests
otherwise, and nothing here establishes it either — so they are being re-run
with the corrected harness rather than carried forward on the old evidence.

M3 has had none of the seven.

## The kill-test rows' narrative, moved out of the plan (2026-09-21)

`docs/plans/rtos-mission.md` had grown to 40,536 words with **nine table
cells holding 35% of it**. The K3 row alone was 22,974 characters in a single
markdown cell — long enough that its own closing summary sat stale for eleven
days while contradicting its own body, because nobody reads to the end of a
cell that long.

The plan now carries each closed row's verdict and a pointer here. **Nothing
was deleted**: every row's full text at the time of the move is below,
verbatim, so the plan's history is recoverable from the ledger as well as
from git.

### K0 family

**The kill test.** a clean clone of any package builds alone and its CI is green; the C oracle produces an identical trace twice

**The state, as the plan carried it:** **passed locally 2026-09-09** (8 repos, fleet gate green on the box; `dynamic` trace identical twice); CI green pending the owner's Actions billing and `kairos secrets`

### K1 scheduler on sim

**The kill test.** nine demo scenarios trace-identical to the C kernel for 100 000 ticks; counters equal; Miri green; the arena-list cost row

**The state, as the plan carried it:** **passed 2026-09-09** — nine of nine, 8,408,764 lines identical at 100 000 ticks, ticks/yields/exits equal on every one; Miri green over the whole corpus; the cost row taken and **2.08×**, which fired §2.5's revisit condition — cut to **1.533×** on 2026-09-10 by reading each node once per call, still above the 1.25× bar, so the decision stays open (`docs/LEDGER.md`)

### K2 IPC + timers

**The kill test.** the remaining demo scenarios trace-identical; Kani harnesses for the CBMC proof list pass; no-panic property test; mutants score ledgered

**The state, as the plan carried it:** **18 of 18 run; 14 on 2026-09-09, the two buffer demos and the last two on 2026-09-21.** The corpus is sixteen scenarios in all (the other two are K1's) and 12,808,722 lines identical at 100 000 ticks. In the kernel: the from-ISR surface, queue sets, real queue lock counts, task notifications, stream and message buffers with their own arena, the software timers with their daemon, event groups, and the `sbSEND_COMPLETED` seam. The no-panic gate passes; Kani verifies seven harnesses (3,965 checks) and does not converge over a started kernel; `cargo mutants` with the corpus as the oracle catches 36 of 42 viable mutants in the scheduler core. Of the four once not passed, **all four now run**, and none needed a kernel FEATURE. `StreamBufferDemo` (1,155,782 lines at 100 000 ticks) and `MessageBufferDemo` (1,017,915 lines) starved sim contract v1; **contract v2 fixed that at the contract rather than in the kernel**. `QueueSet` (318,143 lines at 100 000 ticks) was blocked only by a PRNG seeded from the address of a stack local, now pinned by `kairos oracle patch` exactly as `TaskNotify.c` already was. `IntQueue` did not COMPILE -- it includes a board-specific `IntQueueTimer.h` -- and with the harness supplying one it is exact at the corpus standard (44,285 lines at 2 000 ticks) and holds 362,065 lines at 100 000. `IntQueue` is now exact at 100 000 ticks too -- **2,220,518 lines identical** -- after a second kernel defect: a queue call has MORE THAN ONE critical-section exit it can be preempted at, and the resume marker was a `bool`. One caveat remains and is a ledger row rather than buried: this port has no nested interrupts, so the nesting `IntQueue` was WRITTEN for is not exercised, and the demo's own check cannot detect that. The pair cost two KERNEL defects no other scenario reached -- a queue call preempted at its sampling exit, and a refusal that charged the clock. `conform --all` is now 25 scenarios. All of them and their evidence are ledger rows

### K2.1 the Rust face

**The kill test.** every corpus scenario still trace-identical at 100 000 ticks with the Rust face compiled in, at **zero** extra critical-section exits; a `typed` demo arm passes its own check; one `trybuild` row per bug class the C API cannot refuse; the Kani count after the topology work in the ledger

**The state, as the plan carried it:** **passed 2026-09-10.** `PollQ-typed` is `PollQ` written against `Queue<u16, 10>` and diffed against **`PollQ`'s own C oracle trace**: 117,417 lines identical at 100 000 ticks, `exits=105608` on both sides, and the same pinned digest offline. Six bug classes the C cannot refuse are compile errors — item size, item type, a failed send whose value cannot be dropped, the from-ISR half from a task, the task half from an interrupt, and shared data touched without the lock — each a `compile_fail` doctest paired with the working line it is one character from, so none can pass for the wrong reason (`compile_fail` rather than `trybuild`: it is built in, and the house does not add a dependency for what the toolchain already does). Kani is 25 of 35, the three new ones being the face's own properties at 1–3 s each where the raw kernel's nearest equivalent does not converge in 700 s. **Re-taken 2026-09-10 and it holds** — but the suite had stopped compiling when `Raw` grew for `Mutex<T>` and nothing noticed, because `#[cfg(kani)]` is invisible to every other gate; `kairos check --kani` now closes that, and the sweep needs `--exact` or Kani's substring filter runs three harnesses inside one budget. **Still owed:** the topology work, which the same day's measurements redirect — the arena indirection is not worth removing for speed, the started-kernel harnesses are not a topology problem, and the symbolic-handle ones need the *static* face of §5.2 item 3 rather than anything inside the kernel (`docs/LEDGER.md`)

### K2.2 async task bodies

**The kill test.** one scenario re-written as `async fn` whose await points land exactly on today's `Wait::Blocked` points, and the corpus still gates it — or a ledger row saying why it cannot and what that costs

**The state, as the plan carried it:** **passed 2026-09-10, with its own premise corrected.** The await points must *not* go where the `Wait::Blocked` points are: `PollQ` blocks nowhere, so a body that awaits only there never suspends and `poll` never returns — measured, it hangs at 200 ticks, and the runaway guard cannot fire because the loop is inside a single poll. They go after **every** kernel call, which is where a `pc` arm ends and for the same reason. With that, `PollQ-async` is identical to `PollQ`'s own C oracle trace: 117,417 lines at 100 000 ticks, `exits=105608`, and the same digest offline. **Storage, settled the same day:** `Body::Async` holds a `Pin<&mut dyn Future>`, the caller pins with `core::pin::pin!` and lends it — no allocator, no `unsafe`, nothing unstable. It forced the kernel and the statics out of the runner and into `RefCell`s the caller holds, because a future cannot borrow a field of the struct that polls it; all 18 arms are still identical at 100 000 ticks, which is what says the refactor was safe. `async` is now a body kind beside the others, not a prototype

### K2.3 the static face

**The kill test.** a declaration whose geometry is the declaration: exact `.bss`, no create-failure path an application can reach, and the symbolic-handle proofs unwritable against it

**The state, as the plan carried it:** **passed 2026-09-10.** `system! { mod app use Cfg; tasks {..} queues {..} }` writes `TASKS`, `QUEUES`, `SLOTS`, `ITEMS` and `LISTS` from the declaration. As a counter rather than an adjective: **2,088 bytes against 13,600 hand-sized, 6.5×** (`size_of`, exact, compile-time), and a test that fills every declared slot and asserts the next send is refused, which fails if `SLOTS` was rounded either way. Every create-failure path is closed by construction and the one that is an application mistake — a priority the config has no ready list for — is a `const` assertion, so a build failure rather than a `configASSERT` on the bench. Two more `compile_fail` doctests with their controls. **Still owed:** timers and event groups are not declarable yet, and no corpus arm runs on it — it is proven by unit tests and `size_of`, not by the C oracle

### K4 heaps

**The kill test.** `heap_4` differential trace matches C; `StaticAllocation` on every cell; RAM table per profile

**The state, as the plan carried it:** **PASSED 2026-09-10.** **The differential matches**: `heap_4.c`'s algorithm transcribed over offsets rather than pointers — address-ordered first fit, split only when the remainder is strictly larger than twice the header, coalesce with the block before and after — and diffed against `heap_4.c` compiled **verbatim** from the pinned kernel. **20,000 operations agree on the offset first fit chose, the free bytes remaining and the minimum ever free.** Offsets, because a pointer is not comparable across two programs and an offset into a known aligned base is; the C driver sets `configAPPLICATION_ALLOCATED_HEAP` so `ucHeap` is its own aligned array. The C arm is generated once and checked in, so the diff runs with no C toolchain. It passed first time, and is recorded with its weakness: the first workload **never refused a single request**, so it was agreement about the easy half of an allocator; `the_workload_reaches_the_branches_that_matter` caught that and the quoted agreement is on the harder one (2,087 refusals, 784 distinct offsets, minimum-ever free 1,232 of 8,192). **The RAM table is an identity, not a total**: `total = arena + bookkeeping`, remainder 0 on every row, bookkeeping a fixed 56 bytes that does not grow with the arena — and because a host test cannot measure a target, the same identity is a `const` assertion the compiler evaluates on all four bare-metal rungs. Per profile means per pointer width: the header is 8 bytes on every Kairos target and 16 on the oracle's host, and the minimum block moves with it. **`StaticAllocation` is met in the form Kairos's design makes meaningful, which is stronger than the C demo's**: the C shows a system *can* be built with `configSUPPORT_DYNAMIC_ALLOCATION 0`; here it cannot be built any other way. `rusty_rtos_kernel-core` and `rusty_rtos_demo-core` are both gated as allocation-free on the linked **rlib**, poison-proven both directions, and a bare-metal cell that declares no global allocator cannot link an allocation at all — also poison-proven. **A symbol scan of the linked ELF was tried and REMOVED**: a cell poisoned with a real `#[global_allocator]` and a live `Box::new` still carried zero allocator symbols, because LTO inlines the shim away. The rlib is the instrument; the binary is not

### K6 C ABI

**The kill test.** the unmodified C standard demo tasks link against `rusty_rtos-capi` and pass on Posix, then on QEMU M3

**The state, as the plan carried it:** **BOTH HALVES PASS as of 2026-09-11, with the order inverted.** The unmodified demo files pass individually AND together on two ports and THREE platforms: **25 files each**, QEMU M3 25/25, the host cell 25/25 on Windows threads and 25/25 on Linux pthreads (3 runs of 3 each), plus `MessageBufferAMP` 1/1 in a binary of its own and `flash.c` running with no checker to grade it. Every verdict is the demo file's own checker, and the default run is one full 10,000-tick check period at 1 kHz, 60 tasks, 25 queues. Nothing about the C is patched: the files come straight out of pinned `oracle/` and compile against the oracle's own `FreeRTOS.h`/`task.h`/`queue.h`; the only file either cell hands the C is a `FreeRTOSConfig.h`. **The check strength is itself a finding.** The demos ship a check task that asks every checker every 10,000 ticks (`main_full.c`: `xCycleFrequency = pdMS_TO_TICKS( 10000UL )`) and latches failures; asking ONCE at the end asks only "did this counter ever move". Sampling at the demos' own cadence is strictly stronger, found SEVEN more defects, and **all three are now clean under it.** The DEFAULT run is now one full check period rather than 3,000 ticks, because a default shorter than the period the checkers are written against does not merely ask the weak question -- it produces false failures, and one arrived the moment `flop.c` joined the set. The host was 19/21 and the cause is FIXED: `HostPort` never declared `Port::COMMITS_SWITCH`, so the kernel treated itself as stackless, moved `current` inside `port_yield`, and the port switched AGAIN on the way out — TWO selections per yield, which on a ready list of two is the same task for ever. Found by per-task turn counts (`QProdB2`, priority 0, on 107,299 turns beside `SetB`, priority 0, on **124**), reproduced in `rusty_rtos_port/firmware/host-kernel` (13.5M laps against ZERO, now 6.08M against 6.07M), after refuting arena exhaustion, a global stall, `integer.c` hogging, and `xTaskResumeAll` — and after exonerating the list and the kernel with tests now in the tree. It also retired `reconcile()`, a workaround written earlier the same session for the identity divergence this defect caused. **The last gap was the emulator, and it was PROVED so rather than assumed.** `StreamBufferDemo` on M3 had 2 of ~290 trigger-test receives block 6 ticks against a 5-tick request (byte and tick histograms identical), exactly 2 whether the run was 30,000 or 60,000 ticks, with a STICKY checker so two events poisoned the rest. Same kernel, same seam, same demo, and the host did it ZERO times — so what differed was not code but how much CPU a tick gets. Sweeping that one quantity: 20 MHz -> 2, 40 MHz -> 2, **80 MHz -> 0**, at which point the byte histogram is `[_, _, 58, 58, 58, 115, 0, 0, 0, 0]`, IDENTICAL to the host's. That identity is the evidence, not the pass: 60 tasks plus a tick hook running every demo's ISR half every tick is ~330 cycles per task per tick at 20,000, and the emulated part simply could not finish. The cell's `FreeRTOSConfig.h` and its SysTick reload are two ends of one number in two languages, so both carry that table as a comment. The demo ships `configSTREAM_BUFFER_TRIGGER_LEVEL_TEST_MARGIN` to widen the assertion; no FreeRTOS project sets it and neither do we — one `#define` would have turned the run green in a minute and hidden the measurement instead of making it. **The order inverted because Posix needed a host port with REAL STACKS and there was none** — the three real ports are bare-metal and the sim port is stackless, and an unmodified C task blocks, so it needs a stack. `rusty_rtos_port-host` closed it: one OS thread per task, a single run permit, a tick thread that freezes whoever holds it. Its own kill test is `rusty_rtos_port/firmware/host-kernel`, where a task that NEVER yields is still taken off the CPU — 40 of 40 expected laps, against **1 of 120** with `KAIROS_HOST_NO_PREEMPT=1`, which is the poison test. **The surface was DERIVED, not chosen** — `llvm-nm -u` on the unmodified files: 108 symbols across 33 files, not 341, of which 11 are `__aeabi_*` and 6 board drivers. It is now **87** — it grew to 89 and then LOST two, because `strcmp` and `strncmp` were being claimed as ABI when they exist only because a bare-metal cell has no libc; the deriver now stops at the seam's `tiny libc` banner, which also keeps our three-argument `sprintf` stand-in out of a header that would then conflict with the real `<stdio.h>`. The one it grew is itself a finding: the derived surface depends on the PORT HEADER, because `ARM_CM3` expands `portYIELD()` into an SCB write while `MSVC-MingW` expands it into `vPortGenerateSimulatedInterrupt`. **One seam serves both cells** (`seam/abi.rs`, compiled twice, not copied), naming no chip and no host: every platform-shaped thing is one of six names the cell supplies. `BaseType_t` is 32 bits in one cell and 64 in the other and neither the seam nor a demo file needed a line changed. **The header is generated and the COMPILER audits it**: `tools/derive_symbols.py` re-derives the table from the seam (`--check` reports drift), and `capi/header_gate.c` compiles the generated header beside the ORACLE's real ones in one translation unit — a disagreement is "conflicting types" (it found four) and a declaration with no definition is an undefined reference (poison-tested). That completeness half was **vacuous on its first attempt**: `--gc-sections` discarded the table, and once reachable the compiler folded the walk to a constant; `volatile` is what makes it real. **Seventeen defects, and the C, a compiler or the port's own kill test found every one.** A port critical section that ENABLED interrupts instead of restoring them (a task schedulable before it had a stack, faulting to `pc = 0` 360 ticks later in a DIFFERENT task); a queue set with no item size whose handles were never translated; deferred `FromISR` work routed nowhere because both cells used `NoTickHook`; a null handle encoded as non-null; a discarded `notify()` bool; `xQueueOverwrite` silently sent to the BACK; the timer daemon starving 17 of 19 tasks; a C function pointer kept in an `AtomicU32` (fine on M3, truncated on a 64-bit host); demo entry points declared `u32` against the C's `UBaseType_t`; a self-deleted task never reclaimed because neither idle task ran `prvCheckTasksWaitingTermination`; a timer daemon that was HALF a daemon (`prvProcessReceivedCommands` without `prvProcessTimerOrBlockTask`, so `xTimerStart` worked and nothing ever expired); a KERNEL gap — `vTaskSuspend` not clearing `taskWAITING_NOTIFICATION`, so a suspended task reported as blocked for ever (`TaskNotify.c:498`); and the two halves of ONE configuration disagreeing, `configTIMER_QUEUE_LENGTH 10` in the C against `2` in the Rust, on the queue that carries every deferred `FromISR` call. **And a defect the diagnosis itself had**: three probes came back clean because there was no `HardFault` handler, so a faulting C task presented as the whole system stopping. Seven permanent diagnostics stayed. **The host cell now runs on Linux too, on pthreads** — clean at the default length and under sustained checking (25/25 as of the 2026-09-12 expansion), `preemptive=true`. Unix has no call that stops another thread from outside, so the target freezes ITSELF in a `SIGUSR1` handler and parks in `sigsuspend`; `freeze` does not answer until it observes the target parked (`pthread_kill` returns when a signal is queued, not handled, and answering there would put two tasks on the CPU at once), and `SIGUSR2` is blocked for the whole handler so a thaw in the gap arrives pending rather than being lost. The port's own kill test passes 11 checks on Linux with 0 failed freezes, and FAILS 5 with `KAIROS_HOST_NO_PREEMPT=1`. It also surfaced an upstream conflict: `MSVC-MingW/portmacro.h` expands `portYIELD_FROM_ISR` to `return x`, `MessageBufferAMP.c` calls it from a `void` handler, and GCC 14 makes that an error — both files upstream's, in a combination upstream never builds. **Expanded to 26 files 2026-09-12, and the five new ones cost four more defects** -- `pcTimerGetName` returning an empty string against an assertion that compares it; the timer daemon sleeping on the delayed list instead of WAITING on its own queue, so a posted command could not preempt it, which also exposed that the kernel had no way to express the block time (`process_one_timer_command` now takes one, and two kernel tests with a switch-counting port pin both halves); `MESSAGE_LENGTH_BYTES` disagreeing with the C by four bytes on a 64-bit host, the same two-halves class as `configTIMER_QUEUE_LENGTH`, now DERIVED from the target; and the harness's own default run being shorter than the checkers' period. **Which files to add was decided by the linker**: the undefined symbols of all 33 compiled files were diffed against the 89 exported at the time, and nine of the twelve not running needed nothing from the kernel at all -- C library functions and a demo project's board drivers. **`all 34` is not reachable in one binary and that is a property of the demo files**: `flop.c`/`sp_flop.c` and `comtest.c`/`comtest_strings.c` are mutually exclusive pairs defining the same symbols, `MessageBufferAMP.c`'s `sbSEND_COMPLETED` is process-wide (its own binary now, as the oracle does it), `flash_timer.c` starts timers in the window `TimerDemo.c` deliberately fills, and `flash.c`/`flash_timer.c` ship NO CHECKER so they can never be graded. **What is NOT done**: `IntQueue.c`, which needs a board-specific header a demo *project* supplies, and three items put as owner decisions in `rusty_rtos-capi/docs/plans/rusty_rtos-capi.md` §4b -- co-routines (a second scheduler, for two of the lowest-value files, which FreeRTOS itself calls legacy), static allocation (which collides with this kernel's compile-time arenas and is a K4 design question, not a task), and `IntQueue` (a PORT item: nested interrupts, belonging with K8). **Re-weighted upward 2026-09-10.** This is the commercial wedge, not the sixth of eight: FreeRTOS's moat is not its code (MIT, given away deliberately) but the installed base and the C API everyone already writes against, and the ABI is what collapses the switching cost that moat is made of. It is also the only thing that unlocks **Janus Track A**. **Dependency still standing:** Espressif's IDF-FreeRTOS is an SMP *fork* — `xTaskCreatePinnedToCore` and friends — so passing the vanilla C demo corpus is necessary and **not sufficient** for Track A. K6 and K8 together unlock it

### K7 libraries

**The kill test.** MQTT over our TCP for one hour, zero lost keep-alives; JSON suite 100 %; no-panic fuzz on every parser — **but to OUR broker, on OUR box**

**The state, as the plan carried it:** **PASSED 2026-09-21, all three clauses, and the target was changed 2026-09-10.** The hour: `rumqttd` 0.20.0 on its v5 listener over our own TCP on a TAP device — **3,600s held, 178,263 protocol loops, 1,790 PINGRESPs, 0 failures, 0 quiet minutes**, with a second hour against mosquitto 2.0.22 in parallel. The evidence is NOT the PINGRESP count (1,790 of ~1,800 is the counting window closing on exchanges in flight): it is that both brokers reap a client 3.0s after a missed keep-alive and **neither dropped us across the full hour** — a third party deciding, the same shape as the C kernel's trace deciding for the scheduler. JSON: **318/318 vs coreJSON, 95/95 `y_`, 188/188 `n_`**. No-panic fuzz: it was NOT met and nobody had said so — json/sntp/mqtt had suites, and `rusty_rtos_http` and `rusty_rtos_tcp`, the two newest and the two with the most parsers, had none. Both now do (9 and 10 tests). **Read a milestone's clauses before scoring it.** All six packages are built and the six run **byte-identically on a Cortex-M3** (`tools/vertical/k7-battery`). The original row said "to a broker", which is the FreeRTOS-LTS set's own framing: those libraries exist to reach AWS IoT Core, and §2.2 already marks the AWS IoT libraries **NEVER**.** The original row says "to a broker", which is the FreeRTOS-LTS library set's own framing: those libraries exist to reach AWS IoT Core, and §2.2 already marks the AWS IoT libraries **NEVER**. The Home Computer's Phase 2 IoT Hub milestone runs **`rumqttd`**, a Rust-native MQTT broker, on the box. So the kill test should close the vertical rather than leave it hanging at a generic cloud endpoint: **a Kairos device publishing over our TCP to the Home Computer's `rumqttd`, one hour, zero lost keep-alives** — sensor → kernel → mesh → a hub that cannot read it. That also frees the row from the C6: `rumqttd` does not care which chip publishes to it, so the S3 in hand can carry it. Bias the library set toward what the mesh needs (MQTT, JSON, SNTP, backoff) and away from anything cloud-of-record shaped. **Scored clause by clause 2026-09-21, because this row has THREE and only the first was ever being tracked:** (1) *one hour, zero lost keep-alives* — **MET 2026-09-21, against `rumqttd` itself.** rumqttd 0.20.0 on its v5 listener (port 1884; this client speaks MQTT 5 and a v5 CONNECT at a v4 listener is a protocol refusal, not a stack problem), over our own TCP on a TAP device: **3,600s held, 178,263 protocol loops, 1,790 PINGRESPs, 0 failures, 0 application events, 0 quiet minutes**, leanest minute 29 of 30 responses. A second hour against mosquitto 2.0.22 ran in parallel on its own TAP and subnet and also passed (178,247 loops, 1,793 PINGRESPs, 0 failures). **The PINGRESP count is not the evidence and it matters which one is**: 1,790 of ~1,800 is the counting window closing on exchanges still in flight, not ten lost keep-alives. The evidence is that BOTH brokers reap a client 3.0 seconds after a missed keep-alive and NEITHER dropped us across the full hour — a third party deciding, the same shape as the C kernel's trace deciding for the scheduler. Both substitutions this campaign made are now gone, and the rumqttd one should never have been made; (2) *JSON suite 100 %* — **MET**: 318/318 files agree with coreJSON, 95/95 JSONTestSuite `y_` accepted, 188/188 `n_` rejected, corpus pinned at `nst/JSONTestSuite@1ef36fa` with the file counts asserted so a corpus that moved fails rather than re-scores; (3) *no-panic fuzz on every parser* — **was NOT met, and nothing had said so.** `rusty_rtos_json`, `rusty_rtos_sntp` and `rusty_rtos_mqtt` each had a suite; `rusty_rtos_http` and `rusty_rtos_tcp`, the two newest and the two with the most parsers between them, had none. Both now have one: **9 tests** for HTTP and **10** for +TCP, and both were poisoned before they were believed. The HTTP suite makes `recv` **lie** — a negative count, and a count LARGER than the buffer it was handed, which a broken driver really can return — and that is the test that caught the poison where `feed` indexes instead of using `.get()`. The +TCP suite goes after the two surfaces a corpus structurally cannot reach: `BitConfig` and `StreamBuffer` are driven by SIZES, not bytes, so a recorded script only ever exercises the sizes it happens to contain. **Its second poison did not fire on the first attempt, and that was a defect in the TEST**: the wrap test drained the buffer every lap, so it was empty whenever `get_ptr` was called and the trim at the end of the array was never exercised. Reading back only half leaves a residue, and the poison then reports `get_ptr reported start 4 + run 2 past an array of 5` — the CWE-787 shape exactly. The assertion was too weak as well (`run <= room` cannot catch a run that overruns from its START; the contract is `start + run <= room`), and trying to poison it is what showed that

### K3 silicon + QEMU — the row as the plan carried it before the 2026-09-21 trim

**The kill test.** the full corpus check task passes one hour on M3-qemu, RV32-qemu and a C6; context switch / tick / latency cycle rows vs the C demo **from the C6, not from QEMU**; flash + RAM decomposition

**The state:** **STATE OF PLAY, 2026-09-21 — read this first; the rest of the row is the evidence.** This row is long enough that its own closing summary went stale for eleven days while contradicting its body, so the state is stated here instead. **Done:** the corpus is byte-identical to C FreeRTOS on **four** targets (host, ARMv7-M, RV32, and a XIAO ESP32-S3 — a real part); the one-hour clause is **CLOSED on the host and on silicon, and on the two emulators it covers 18 of the corpus's 25 scenarios** — RV32 18/18 in 58 min and M3 18/18 in 75 min, both true, both measured against the corpus as it stood on 2026-09-11. **Re-checked 2026-09-21: `bench/soak-each/run.sh` hardcodes its scenario list and seven scenarios have joined the corpus since** (`AbortDelay`, `ApiSweep`, `IntQueue`, `MessageBufferDemo`, `QueueSet`, `StreamBufferDemo`, `TaskNotify`), so the clause stopped covering what it claimed without anything failing. The script now derives the corpus from the cell and fails on drift in either direction. **The seven were soaked on RV32 the same day and six pass** — `ApiSweep`, `QueueSet`, `TaskNotify`, `MessageBufferDemo`, `StreamBufferDemo` and `IntQueue`, the last writing 79.9M trace lines and 2.59 GB — while **`AbortDelay` FAILS the hour** at `pass=false runaway=false ticks 3600009 of 3600000`: it reaches the full length and its check task declines to pass, which is a second failure independent of its known conformance gap. **A SECOND defect was found doing it, and it is the worse of the two**: the harness counted any line beginning with the scenario's name as a pass, so `AbortDelay  FAIL` was tallied among the passes and the run reported '3 passed' with a FAIL on screen. Fixed — the verdict is now the word and the denominator is counted rather than hardcoded at 18 in three places — and the same three scenarios went from '3 passed, exit 0' to '2 of 3 passed, exit 1'. **The consequence for the record is that the earlier 18/18 proves eighteen lines were PRINTED, not that eighteen said ok**, so the eighteen are being re-run under the corrected harness rather than carried forward. M3 has had none of the seven. One thing the hour settled for free: `StreamBufferDemo` passes it across 41.7M lines while failing conformance at 2,000 ticks, and liveness passing where conformance fails is what a trace-TEXT defect looks like rather than a scheduling one. flash **and** RAM are decomposed and compared against the C map (15,950 B vs 13,924, 1.15x; per task 207 B vs 596 B); and **all three cycle rows exist on silicon** — tick **54**, switch **166**, ISR-wake-to-task **430** cycles, **re-measured on the board 2026-09-21 after a harness defect was found; the numbers this row used to carry (131 / 623 / 949) were measured through it**. **Open, and only these two:** a **C6**, for a second chip and a second architecture-of-record — and the **C arm** of the cycle rows, which re-checking on 2026-09-21 moved off the hardware list entirely: the oracle carries its own S3-capable Xtensa port and the S3 compiler is installed, so that arm is blocked on an **ESP-IDF install**, not on a purchase. **Refuted on 2026-09-21 and recorded so it is not rebuilt:** a host callgrind stand-in for the C arm. Details at the end of this row and in `docs/LEDGER.md`. **(Earlier label, kept for the record: part done 2026-09-10, and its measurement half re-specified.)** **`rusty_rtos_port-cortex-m` exists and switches**: two tasks with real stacks under QEMU, 100/100 resumptions over 200 switches, each re-checking every word of its own stack and a callee-saved register — and the test is poison-proven, since deleting `stmdb r0!, {r4-r11}` reports a corrupted witness at word 0. It is the first crate in the family to lift the `unsafe_code` deny, per item, with `UNSAFE.md` carrying every one. **And the kernel now drives it**: `mps2-an385-qemu-kernel` has `PendSV` call `Kernel::switch_context` and `SysTick` call `increment_tick`, so the same fixed-priority scheduler the corpus proves runs real tasks on ARMv7-M — 40 high-priority laps against 402 ticks and a 10-tick `delay`, which is the number only a priority scheduler with a working block produces, and 0 early wakes. Poison-proven at 1 lap against 120.  **The corpus runs on a Cortex-M3 AND on RV32**: `rusty_rtos_demo/firmware/mps2-an385-qemu-corpus` and `.../riscv32-qemu-corpus`, **all 18 scenarios on each** (`death` joined 2026-09-10) — every counter, the FNV-1a/64 digest and the byte count matching the host's pins, which are themselves diffed against `oracle/traces/*`. So the corpus is byte-identical to C FreeRTOS on **three** architectures: the host, ARMv7-M and RV32. Both cells and the host test read one pin table (`rusty_rtos_demo_core::pins`), so a cell cannot silently disagree with the host about what the C kernel said, and both are poison-proven at one `exits` off. **And the one-hour clause is met on the host**: `tests/soak.rs`, via `kairos check rusty_rtos_demo --soak`, runs all 18 for **3,600,000 ticks** — an hour at the oracle's own `TICK_RATE_HZ` of 1000 — and every scenario's check task still reports running, with `ticks >= 3,600,000` asserted so a scenario that stopped early cannot pass. It claims **liveness, not conformance**: there are no C pins at 3.6M ticks, so it asks what the C demo asks. Poison-proven by sizing the step limit for 2,000 ticks. **The hour is met on M3-qemu and RV32-qemu too**, behind `--features soak` on each cell — a feature and not a second binary, so the gate's plain `cargo run --release` keeps meaning the 2000-tick conformance run. a previous session recorded 17/17 on each, exit 0. **A claim that the EMULATOR hour was 18/18 was made and withdrawn on 2026-09-11**, and what replaced it is better: the host closes the clause and the emulators turn out to be slow rather than failing. **The host runs all 18 at 3,600,000 ticks** (`cargo test -p rusty_rtos_demo-core --test soak -- --ignored`), and the timing there explains everything the emulators did: **`semtest` alone is 106.2 s of the 136.4 s total — 78% of the corpus's work at this length** — because it runs 345 state-machine steps per unit of sim time against a corpus average of 6.3 (`semtest.c`'s guarded loop counts to `0xfff` between one semaphore take and the next). So the three full-corpus emulator runs that appeared to 'stop at scenario four' were stopped **in** `semtest`, not **by** it: it takes longer than the other seventeen combined. On the emulators what is established is `death` at 3,600,000 ticks on both, identical field for field (`ticks=3600057 yields=57580 exits=3533911 lines=3973943`), and `semtest` passing at 100,000 and 400,000 with counters scaling linearly, still running past 25 minutes at 1,600,000. **And the hour now exists on SILICON**, which is what the C6 was in this row for: on a XIAO ESP32-S3 over USB, 2026-09-11, the pinned corpus runs **18/18 byte-identical to the C kernel** and `death` at 3,600,000 ticks reports `ticks=3600057 yields=57580 exits=3533911 lines=3973943` — **identical field for field to the M3 and RV32 runs**, so three architectures, one of them a real part, agree on every counter at 1800x the pinned length. A C6 would add a second chip and a second architecture-of-record, not the first silicon hour. **And the emulator hour is now CLOSED on RV32**: `bench/soak-each/run.sh riscv32-qemu-corpus`, **18/18 at 3,600,000 ticks in 58 minutes**. The split costs nothing, because the cell builds a fresh kernel per scenario, so eighteen runs of one is the same computation as one run of eighteen. The timing confirms the diagnosis to the point: **`semtest` is 2,769 s of the 58 minutes — 79%**, against the host's 78% of its own 136-second run. Two machines three orders of magnitude apart agreeing on the proportion is what a structural cost looks like. **M3-qemu followed: 18/18 in 75 minutes**, so the emulator hour is CLOSED on both. Two compile-time overrides came out of the work and both now exist in each corpus cell: `KAIROS_SOAK_ONLY=<name>` runs one scenario rather than eighteen (which made `death` a 24-second command instead of an ~8-hour one, and isolated `semtest` in a single run; an unknown name FAILs and exits 1, because a filter matching nothing otherwise yields a run with zero failures), and `KAIROS_SOAK_TICKS=<n>` moves the length — which is what turned silence into a bisection, and ruled out '`semtest` is broken' before it reached a ledger row as a fact. The pinned 2,000-tick conformance gate is unaffected and still 18/18 on both cells. And the three sets of counters were diffed by machine: **the host agrees with itself on all 18 at 3,600,000 ticks (2026-09-11), and an earlier session had host, Cortex-M3 and RV32 agreeing on 17 at that length**; `death` now agrees across M3 and RV32 there too — cross-architecture determinism at 1800x the pinned length, though our builds against each other rather than against C, since no C pin exists at that length. The full eighteen remain a background run rather than a gate (~8 hours per architecture), but a SINGLE scenario at that length is now a 24-second command, which is what closed the `death` gap. **Only the C6 clause remains, blocked on the same hardware B4b needs.** So this is agreement with **C FreeRTOS to the byte on ARM**, and it needed **no context-switching port**: a scenario is a state machine, so a task needs no stack of its own. That the corpus is portable is a consequence of the K2 design — locals live in the TCB rather than on a C stack — rather than a trick. The M3 cell exists and gates — `rusty_rtos_core/firmware/mps2-an385-qemu-region`, 9/9, QEMU exit 0 — on `mps2-an385` rather than `lm3s6965evb`, because `MIN_REGION` (64 KiB) is the LM3S6965's entire SRAM. But **QEMU can supply no cycle, latency or work counter**: DWT is unimplemented there, SysTick's deltas are host wall time and *shrink* as the work grows, and under `-icount` the clock does not advance at all — measured six ways, clock source pinned, in that firmware's README. So the QEMU cells carry **correctness** (and an exit code, so they gate) while the **cycle rows come from the C6**, which is a Kairos target with a real `mcycle`. Flash and RAM decomposition are unaffected — properties of the binary, not of execution — and **that third of the kill test is done**: the M3 cell decomposes to `.bss` 198,672 with remainder 0, of which the declared `Region` is 196,608, so the seam costs **2,080 bytes of RAM and 22.5 KiB of flash** beyond the heap a firmware declares (`docs/LEDGER.md`). **The measurement half advanced twice on 2026-09-11, and neither number needed the C6.** (1) **The context-switch row now exists as a WORK row**: `bench/switch-cost/run.sh` counts **30 retired instructions for a Kairos cooperative switch on RV32 against FreeRTOS's 83** (2.77x), both arms counted from their own toolchains — theirs assembled by clang from the pinned `oracle/` checkout with the context macros expanded ALONE in their own sections so no address range is chosen by eye, ours disassembled out of the linked firmware. It is exact rather than sampled because **both paths are straight line**, which the script CHECKS rather than assumes: a conditional branch where none is expected fails the run, since a static count is a retired count only while there is nothing to skip. The gap is **structural, not tuning** — their `portYIELD()` is `ecall` (`portmacro.h:94`), so a yield takes the interrupt trap and saves all 28 GPRs plus `mstatus`, `mepc` and the nesting count, where a Kairos yield happens at a call site whose caller-saved half the C ABI has already declared dead. **And the preemptive row is now MEASURED, after the defect it exposed was fixed**: `rusty_rtos_port/firmware/riscv32-qemu-preempt` runs **201 preemptive switches between two tasks that never yield** (224,541 and 224,515 laps, zero faults), and a preemptive switch costs **74 against FreeRTOS's 83 — 1.12x, near parity**. Getting there needed a port fix: `switch_context` resumes with `ret`, which is right at a call site and wrong inside a trap, so the first preemptive switch was also the last. The port now carries a SECOND switch, `switch_context_trap` (37 instructions: the same fourteen registers plus `mepc` and `mstatus`), and `new_task_context_preemptive`, which builds a fresh task to leave its first trap through `mret`. They are kept apart so **a yield still pays only for what a yield needs** — the cooperative 30 is unchanged and `riscv32-qemu-switch` still passes 100/99. **Poison-proving refuted the first explanation**: removing the `mepc` restore hangs, removing the `mret` hangs, and removing the `mstatus` restore **still passes** — the trap entry has already copied `MIE` into `MPIE`. The port's comment was corrected rather than the number. **So both rows are true and say different things**: a yield is 2.77x cheaper on our side because FreeRTOS routes yields through the trap, and a preemptive switch is near parity because there both kernels save what an interrupt could clobber. Quoting only the first is quoting the easy half. Poison-proven on the instrument: the same probe under `-D__riscv_32e` must lose exactly sixteen instructions a side, and does (42->26, 41->25). This is a **work row and not a cycle row** — the reason cycle rows come from the C6 has not changed. (2) **The 'vs the C map' comparison now exists for RAM**: `bench/kernel-ram/run.sh`, both arms on rv32 at matched config (5 priorities, 16-byte names, 10-deep timer queue), finds the two floors are **different FUNCTIONS** — FreeRTOS is `1704 B static + per-object heap` and does not move when a task is added, Kairos is `arenas(...)` with a heap term of ZERO. Per task: **596 B for C** (84 B TCB + 512 B stack at their own `configMINIMAL_STACK_SIZE` + a heap header) **against 207 B for us**, 2.9x, and the gap is the stack a stackless task does not own. Every Rust figure is a **slope between two geometries differing in one dimension**, measured ON THE TARGET as the length of an array in an rv32 staticlib (`size_of` on the host is a different number), and two slopes true by construction — a notify slot is a `u64`, a buffer byte is a byte — are checked as the instrument's own gate. The counterweight is carried in the row rather than footnoted: this is the KERNEL cost, an application still needs somewhere for its state, and the saving is from **exact sizing** rather than the state ceasing to exist. **And FLASH is compared too (2026-09-11)**: `bench/kernel-flash/run.sh` links BOTH arms with `ld.lld --gc-sections` from the same root set — the operations the corpus proves byte-identical, named with `-u` so neither arm is charged for a harness — and gets **15,950 bytes against 13,924, 1.15x**, 2,026 bytes more. This did NOT need `rusty_rtos-capi`, which an earlier note assumed: it needed a defined root set, not matching symbol names. Three instrument faults were caught getting there, two against us and one for: a single-call-site probe that let the optimiser inline the kernel into it and read 2.3x; a 596-byte root table counted as C kernel; and a root glob that pulled our port's own switch assembly in while the C arm's roots pull in neither of its — that last one only became visible when the preemptive switch moved the total, and fixing it took the pin DOWN. A linker that discards nothing measures nothing, so each arm is linked twice, with and without `--gc-sections`, and the run fails if they agree. **Also caught here**: the RAM probe's first build failed on `Port::COMMITS_SWITCH` because `rusty_rtos_kernel-core` reaches `rusty_rtos_core` by git URL, so cargo built the local sibling AND the published one — the `firmware/*` packaging hazard again, and it only surfaced because the session had just added a trait constant; a change to a function body would have built green against the wrong code. **What remains of the measurement half, RE-CHECKED 2026-09-21 — the sentence that used to be here was stale in four ways and contradicted this row's own body.** It said tick-ISR, ISR-to-task latency, absolute cycles and the flash comparison all remained. **All four are done.** The three cycle rows were measured on silicon on 2026-09-11 (`rusty_rtos_kernel/firmware/xiao-s3-cycles`, XIAO ESP32-S3 over USB): **tick 54 cycles** (225 ns at 240 MHz), **switch 166**, **ISR-API wake to the task holding the value 430** — in cycles, absolute, on a real part, with the instrument's own 1-cycle tax measured and subtracted and medians rather than means. **Those four figures were 131 / 129 / 623 / 949 until 2026-09-21, and they were wrong** — not the kernel, the CELL. It hand-rolled its own `NoTrace` instead of the one `rusty_rtos_core::trace` already ships, so it inherited the trait default `WANTS_NAMES = true` and every traced event built a 16-byte task name, validated it as UTF-8, and handed it to a sink whose body is `{}`. Deleting the twin moved every row — **tick 2.43x, switch 3.75x, ISR wake 2.21x cheaper** — with no kernel change of any kind, and the corrected run reproduces to the cycle across reflashes. It is corroborated by a different instrument on a different architecture: the same fix moved the rv32 selection row **305 -> 79 retired instructions, 3.86x**, against this cell's 3.75x in Xtensa cycles. **A correction this large, and entirely in our favour, is the kind that gets checked hardest**, which is why it is quoted with its mechanism, its poison proof and its cross-architecture agreement rather than on its own. It also WITHDREW a finding: this cell used to report the delayed tick as cheaper than the idle one (129 against 131) and explained why at length; corrected, it is 55 against 54 — the obvious way round, and the inversion was noise inside a number that should not have been there. Flash was compared on the same day: **15,950 bytes against 13,924, 1.15x**, and it did NOT need the capi surface, which this row already says four sentences earlier. **What actually remains is two things.** (1) The **C6 clause** of the hour, which is hardware — though the silicon hour itself is already met on an S3, so a C6 adds a second chip and a second architecture-of-record rather than the first silicon hour. (2) **The C ARM.** **A WORK row against the C now EXISTS, built 2026-09-21** — `bench/tick-work/run.sh`, and it is the comparison this clause asks for on the one basis where QEMU can carry it. Two arms on one machine under one instrument: **FreeRTOS V11.3.1 out of the pinned oracle, unmodified**, with the oracle's own first-party `portable/GCC/RISC-V` port linked bare-metal for QEMU `virt`, against `rusty_rtos_kernel/firmware/riscv32-qemu-tick-work` at matched config. The instrument is `minstret` under `-icount shift=0` — architectural, and exactly reproducible where a clock is not — so it is a **work** row on `bench/switch-cost`'s basis and the cycle rows still come from a part. **tick 15 against 56 (3.73x, against us); scheduler selection 27 against 79 (2.93x, against us).** But a switch is selection PLUS the register file, and `bench/switch-cost` prices that half from the same oracle, so the whole switch is **110 against 109 cooperative — parity — and 110 against 153 preemptive, 1.39x against us**. Three numbers, two of them against us, all of them recorded. Gated twice: identical work-parity anchors (with `tick_count` read back, so the `uxSchedulerSuspended` early-out cannot pass as a tick), and a POISON build of every arm that makes the measured call twice per bracket and requires every row to move. **And the row found a defect worth more than the row**: `rusty_rtos_core::trace` ships a `NoTrace` with `WANTS_NAMES = false`, and eleven sites hand-roll a twin that shadows it and inherits the default `true`, so every switch builds a 16-byte task name for a sink that drops it — **305 to 79 on this row from using the one that already existed**, no kernel change. `xiao-s3-cycles` was measured through that defect too. **What remains of this clause is therefore CYCLES on a part with a C arm beside them**, not the comparison itself. (2b) **The C ARM of the CYCLE rows.** The rows above say what THIS kernel costs; the clause says *vs the C demo*, and there is no ESP-IDF FreeRTOS arm on the same part. **A host work row was built to stand in for it and is REFUTED** — see `docs/LEDGER.md`: the C's `xTaskIncrementTick` self cost is **2.7%** of its inclusive and ours is **13.1%**, so a self-cost comparison compares 2.7% of one thing against 13.1% of another, and the C's inclusive encloses the port's thread switch and the trace write where ours does not. The two kernels partition a tick's work differently, which is exactly why this clause puts the cycle rows on a part: there you bracket the code path with a counter instead of deciding where a function's work ends. **And re-checking what the C arm would REQUIRE moved it off the hardware list, same day.** The assumption was that it waits on the C6. It does not. Three facts, each checked rather than recalled: the pinned oracle **carries its own Xtensa port** — `oracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Xtensa_ESP32`, in the upstream tree, not a fork of it — and that port **names the S3 specifically** (`FreeRTOSConfig_arch.h:81`, `port.c:73`, `port_common.c:28`, `xtensa_init.c:56` all branch on `CONFIG_IDF_TARGET_ESP32S3`); **the S3 C compiler is already installed on this box** (`xtensa-esp32s3-elf-gcc`); and **the part is already on the bench** — the same XIAO ESP32-S3 on COM4 that produced the cycle rows above. So the C arm is buildable on the part we HAVE, and it would be built from **the oracle's own V11.3.1 source, the very kernel the corpus proves us byte-identical to** — which is the point, because ESP-IDF's bundled FreeRTOS is a FORK, and a cycle row against a fork compares us to a kernel we have never conformed against. **The one blocker is ESP-IDF itself**, as a platform layer rather than as a kernel: the port includes eighteen IDF headers — `sdkconfig.h`, `esp_idf_version.h`, `esp_intr_alloc.h`, `esp_heap_caps.h`, `esp_timer.h`, `esp_panic.h`, `esp_task_wdt.h` and eleven more — so it cannot compile without the SDK supplying startup, clocks, interrupt allocation and the flash bootloader. **That is an SDK install (~2 GB, owner's call), not a hardware purchase.** It is recorded here unbought rather than done, because installing a 2 GB SDK is an environment change this session did not have a mandate for. **What the C6 is still for is the OTHER half**: a second chip and a second architecture-of-record for the hour — and on a C6 the same arrangement gets better, since the oracle's `portable/GCC/RISC-V` port is first-party rather than ThirdParty


### K5 the Janus joint — the row as the plan carried it before the 2026-09-21 trim

**The kill test.** ~~Janus S1 on a Kairos kernel with the same numbers as on esp-rtos; the XIAO S3 Xtensa cell `Verified`~~ — **RE-SPECIFIED 2026-09-10, see `master-plan.md`.** The kill test as written pairs two halves that are blocked differently, and one of them cannot be unblocked by buying hardware. **K5a — PART DONE 2026-09-10, and its target CONFIRMED (after a false alarm of ours).** The row's target `rusty_esp_mid/firmware/xiao-s3-keys` **exists and its baseline is real**: `gh api repos/Remade-With-Rust/rusty_esp_mid/contents/firmware/xiao-s3-keys` lists a full cargo cell, and that repo's own ledger carries the M1 row of 2026-09-06 — **a P-256 signature is 95 ms and a verification 151 ms** on a `xiao-esp32s3-sense`, `opt-level=3 lto=fat metric=in-process-us`. A session working only from the local box read `rusty_esp_mid/firmware/` as empty and briefly recorded the opposite; the `/f/coding/janus/*` tree is a SCAFFOLD with empty `.git` directories, not a checkout, and an absence observed in one place is not an absence (`docs/LEDGER.md`). So **K5a's measurement clause is not blocked** — what is missing is a local checkout, and the joining act itself (a Kairos kernel inside a Janus firmware) is the owner's, since a Kairos session does not modify Janus repositories. **Settled while checking:** the esp-hal drift this row warns about is real and one crate wide — `xiao-s3-keys` pins `=1.2.0`, every Kairos S3 cell pins `=1.2.1`, while `esp-bootloader-esp-idf`, `esp-println` and `esp-backtrace` already agree. **The measurement clause is now ANSWERED (2026-09-11):** `rusty_rtos_kernel/firmware/xiao-s3-signing` runs the same workload — `p256 0.13`, the crate `rusty_esp_mid-core` signs with — on the same part, and times the kernel directly: **a scheduling round (queue send, queue receive, two context switches) costs 3,724 ns / 893 cycles, which is 39 ppm of a P-256 signature** (**re-measured 2026-09-21** — it read 8,313 ns / 1,995 cycles / 88 ppm until the cell was found to be hand-rolling a `NoTrace` that shadowed the crate's own and inherited `WANTS_NAMES = true`, so every traced event built a task name for a sink that drops it; no kernel change, and the clause's conclusion is unchanged and stronger); our sign/verify come out at 94.3/149.4 ms against their published 95/151 ms. The differencing experiment this row implies was tried first and FAILED — two 24.5-second batches cannot resolve microseconds, and it produced a scheduled arm faster than bare — so the kernel is timed on its own and the arms serve work parity instead. What remains of the clause is only the joining act itself, which is the owner's. **What was also doable here, and is done:** the conformance corpus runs on the XIAO S3 — `rusty_rtos_demo/firmware/xiao-s3-corpus`, all 18 scenarios byte-identical to the C kernel on **silicon**, poison-proven at one `exits` off. That makes four architectures (host, ARMv7-M, RV32, Xtensa LX7) and the first on a part rather than an emulator, and it proves the prerequisite any Janus workload on the S3 needs: the kernel runs there. **It also refutes this row's build assumption** — Xtensa needed *no port at all*, because a scenario is a state machine and a task owns no stack, so the context switch is what you need to host tasks with stacks, not what you need to prove conformance on a part. **K5b (blocked twice):** the `esp-radio-rtos-driver` joint and Janus S1

**The state:** **open, and split.** Three structural findings changed it. (1) **Every Janus firmware using `esp-rtos`/`esp-radio` is a C6 firmware** and no C6 is in hand; the only two Track B firmwares on the S3 (`xiao-s3-probe`, `xiao-s3-keys`) have **no scheduler at all**, so a kernel there is additive rather than a replacement. (2) **Janus S1 is unchecked in Janus's own plan** — the esp-rtos numbers this row says to match do not exist, so buying C6s does not unblock it; Janus must run S1 on esp-rtos first and that baseline must be taken BEFORE any kernel swap (§2.8). (3) The driver's `SchedulerImplementation::schedule_task_deletion` needs `vTaskDelete`, **which the kernel did not have** — the same gap that blocked `death.c`, so task deletion landed first and paid three times. **DONE 2026-09-10**: `Kernel::task_delete` (both paths) and `check_tasks_waiting_termination`, gated by `death.c` remade as a state machine — 4,370 lines byte-identical to the C kernel at 4,000 ticks, and byte-identical again on Cortex-M3 and RV32. It charged three costs the corpus had been structurally blind to, because every earlier scenario creates its tasks before the scheduler starts and the port counts no exits there: `xTaskCreate`'s two allocations, `pxPortInitialiseStack`'s critical section (a port fact, now `Config::PORT_STACK_INIT_CRITICAL`), and `prvDeleteTCB`'s two frees. Poison-proven 8 of 10, with the two misses reasoned rather than waved past (`docs/LEDGER.md`). **Build order inverted:** the first Xtensa deliverable is a cell running a real Janus workload with the smallest port that supports it, not a complete port looking for a consumer — the failure mode named in MATA's own review is that "the deep work keeps eating the surface work". Also to settle first: the plan names `esp-radio-rtos-driver` **0.4.1** and the box has **0.3.0**; and Janus pins esp-hal **1.2.0** where the Kairos S3 cells pin **1.2.1**, which is the companion-set drift §2.7 warns about — one seam crate owns those pins, never two

## K7's six library rows, moved out of the plan (2026-09-21)

Section 5.1 of the mission plan is called "The next bricks, in order" and every one of its six entries was finished, three of them carrying their whole result write-up inline (15,000, 4,200 and 22,000 characters). A section that lists only finished work cannot answer the question it exists to answer.

The rows as the plan carried them:

### 1. `rusty_rtos_backoff`

**Status.** ✅ **done 2026-09-16: 192/192 calls agree**

**The oracle, and what the kill test can be.** `backoffAlgorithm`'s own unit-test vectors, plus its PRNG driven identically so the sequences are comparable — the same trick the heap differentials use, because `rand()` is implementation-defined and would make the arms incomparable by construction

**Why here.** the smallest thing that proves the K7 harness. No I/O, no transport, no clock

### 2. `rusty_rtos_json`

**Status.** ✅ **done 2026-09-16, both halves**

**The oracle, and what the kill test can be.** **JSONTestSuite at 100 %** (95/95 `y_`, 188/188 `n_`) **and 318/318 agreement with `core_json.c`**, which are kept as two tests because they are two claims — they coincide only because coreJSON was measured against the corpus *first*. Then the query half: **2,124 queries and 348 iterations agree with the C**, as a 3,089-line trace. Plus the no-panic gate over the whole public surface: **73,005 deterministic documents**, including a LIVENESS assert, because a query cursor that stops advancing hangs rather than failing

**Why here.** self-contained, and the first one with a real external corpus rather than a vendor's vectors

### 3. `rusty_rtos_sntp`

**Status.** ✅ **done 2026-09-16, both halves**

**The oracle, and what the kill test can be.** **160 trace lines agree with `core_sntp_serializer.c`** across 75 calls, including both sides of the 2036 era wrap and the half-era tie from both directions. The trace carries its own inputs, so there is no second copy of the C's table to drift. Then the client: **549 more lines across 23 scenarios, compared CALLBACK FOR CALLBACK** — every clock read, every datagram, every rotation, and the context state after each one

**Why here.** its hardest defect is DATED — an era comparison that is wrong is correct until 2036 and silently wrong afterwards, which no amount of testing-against-today would find. **Two defects in the C came out of it**, both drafted for filing (§5.4)

### 4. `rusty_rtos_mqtt`

**Status.** ⚡ **COMPLETE 2026-09-19 — ALL 218 of coreMQTT's 218 FUNCTIONS, 100 %, counted from the pinned source by `oracle/coverage.py --check`; every file finished, and what is left is not coreMQTT but coreMQTT's CMock vectors and a real broker**

**The oracle, and what the kill test can be.** **413 trace lines across 24 scenarios agree with `core_mqtt_state.c`**, compared OPERATION FOR OPERATION with both record arrays checked after each one — their order is the resend order, so a status-only comparison would bless a transcription that reordered a session's backlog. Then the **fixed header**: **6,291,456 calls agree** — every one of the 256 type bytes against every remaining-length pattern at every claimed length, compared by per-status counts and an FNV-1a digest, with all eight poisons firing first time. Then the **MQTT 5 property primitives**: 104 trace lines plus a 6,480-call sweep, comparing the CURSOR and the BUDGET after every read, because a refused string has already consumed its length bytes. Then the **fixed-header writers**: 51 lines plus a 1,536-call exhaustive sweep of the CONNECT flags byte, the one byte that packs six independent decisions. Then the **packet-size calculators** that feed those writers: 53 lines reaching MQTT's 268,435,455 limit at the EXACT value each check tests, plus the first cross-slice test in K7 — each calculator's answer fed into the matching writer, because two arms that each agree with the C can still disagree with each other. Then the **acknowledgement deserializers**, the first INCOMING slice and so the first whose whole input a broker chooses: 57 lines plus three 256-value sweeps whose ACCEPTED SETS are printed rather than hashed — and reading them found **three divergences from MQTT 5.0**, including a conformant UNSUBACK a stock client refuses (§5.4). Then the **CONNACK**, the packet that sets every connection-wide limit: 54 lines plus six more sweeps, the property identifier swept FIVE TIMES because each one introduces a value of a different width — and this time both tables are exactly §3.2.2.2 and §3.2.2.3, which is a result and is asserted as one. Then the **incoming PUBLISH**, the only packet carrying application data: 54 lines, TWO flag sweeps (QoS decides whether a packet id is present, so one body cannot sweep the nibble) and six property sweeps, with the payload arithmetic pinned by an IDENTITY rather than a bound — **two more divergences from MQTT 5.0** came out of it (§5.4). Then the **DISCONNECT in BOTH DIRECTIONS**, which the previous slice's "every packet a broker can send" had missed by one: 58 lines, the reason code swept 256 times per direction because one table answers differently each way, and ten property sweeps because the two directions have different property tables — the outgoing set matched the specification exactly, which is what made the incoming set's **missing `0x9F`** visible (§5.4). Then the **CONNECT**, the packet that starts a session: 30 lines compared BYTE FOR BYTE plus a 32-combination digest of the whole packet, because every field is length-prefixed and a packet with two of them swapped still parses — four ordering poisons are caught that a parse-level comparison could not see, and the five 16-bit field checks needed cases built for them because nothing in the corpus went near 65,535. Then the **outgoing PUBLISH**, across all THREE of coreMQTT's serializers: 25 lines, with the trace carrying the PREFIX relationship between them, because three separate differentials could each pass while the nesting broke — and four cases that break the C's own stated API contract refuted an assumption in my own comment on their first run. Then **SUBSCRIBE, UNSUBSCRIBE, the acknowledgements and PINGREQ**, which complete the outgoing codec: 45 lines, all 36 subscription-option bytes swept and asserted distinct, and the ack reason codes swept PER TYPE — which showed coreMQTT validating them CORRECTLY on the way out and incorrectly on the way in, sharpening the §5.4 report from "write a table" to "use the one three thousand lines up". Then the **transport reader**, the one function handed a CALLBACK rather than a buffer: 20 lines compared CALL FOR CALL, because two of its orderings — the type checked before the length, and the multiplier guard checked before each read — are invisible in the status and plain in the log. Then the **six outgoing property validators** — which property may go in which packet: 94 lines, of which **36 are sweeps** over all 256 identifiers at six value shapes each, printed rather than hashed, and together they are the whole map. Three more divergences came out of them, all in the PUBLISH table and all cases where the SAME library gets the rule right elsewhere (§5.4). And a lesson the sweeps could not teach: the driver was written from them, produced 49 cases and passed first time — transcribing the C added seven more cases, and **three of the nine poisons are caught only by those seven**, measured by rerunning them against the 49-case trace. A sweep says what a table ACCEPTS; only reading the C says what it REMEMBERS. Then the **last of `core_mqtt_serializer.c`** — the connection context, the two constructors and the outgoing PUBLISH's parameter validator, with the file's SECOND reader of the fixed header driven alongside the callback-driven one: 56 lines, and the instrument was to run each pair of implementations over the same input and print both answers. `truncated-sweep differ=168` — for every packet type a client may receive, a type byte with no length behind it yet is `MQTTNeedMoreBytes` from the buffered reader and `MQTTBadResponse` from the callback-driven one, whose own doxygen shows a NON-BLOCKING loop ending in an assert. Three more findings (§5.4), and the honest half of the slice: three of the C's refusals are a POINTER AND A LENGTH THAT MUST AGREE, which a slice carries as one, so those cases were removed from the driver rather than faked — a trace should ask only what both arms can answer. Then the **MQTT 5 property builders**, all of `core_mqtt_prop_serializer.c`: 73 lines, and **the first place in K7 where the transcription could not follow the C**. `addPropUtf8` sizes a property as its two length bytes plus its body and forgets the identifier byte it then writes, so a buffer of exactly `propertyLength + 2` gets `MQTTSuccess` and ONE BYTE PAST ITS END — a CWE-787 in six public adders (§5.4). This arm writes through a `&mut [u8]` under `forbid(unsafe)` and cannot do it, so the overflow surfaced as a differential FAILURE and the only question left was which arm was right. Two poisons then could not fire — sizing a four-byte property as 4, and sizing a string property the C's way — because the size check is advisory here and the SLICE BOUND is the guarantee: two independent bounds on every write, the second not code that can be got wrong. Then the **MQTT 5 property reader**, all of `core_mqtt_prop_deserializer.c`: 55 lines, twenty-two getters each demanding ONE identifier across 6,144 calls, and two tables over one alphabet that — for the first time in this library — AGREE, which is asserted rather than assumed because a differential that only ever reported disagreement would not be believed when it reported agreement. Then **topic matching**, the first of `core_mqtt.c`: 109 lines, and the sweep goes up a dimension because matching takes two STRINGS — every string over `{a, /, +}` to length three as a printed 39x39 GRID, plus 2,414,916 digested pairs. A filter whose last level is `+` stops matching a topic whose last level is empty once an earlier `+` has been used, so a client subscribed to `+/+` silently never receives a message published to `a/` (§5.4) — and it is a COLUMN PATTERN in the matrix, not a case anyone had to suspect. The same sweep caught our own arm naming PUBLISH by its whole byte where the C names it by its nibble. Then the **client context**, the last of `core_mqtt.c` that needs no transport: 41 lines, and `MQTT_Subscribe` validates BEFORE it looks at the connection, so an unconnected context separates a malformed list from a well-formed one. Two defects, both about a list (§5.4) — and a new member of an old family, THE TYPE AS A REFUSAL: a QoS of 3 fits `MQTTQoS_t` and not `QoS`, so the type refuses before any validator runs, which makes two of the C's checks unreachable here rather than transcribed. Then the **send plumbing**, which is the harness the rest of `core_mqtt.c` needs: a SCRIPTED transport whose every call is logged and a SCRIPTED clock, 28 lines. What a transport is OFFERED on each call is as much of the behaviour as how many bytes arrive — `2:1,1:1` is a sender that advanced and `2:1,2:1` is one that sent the first byte twice, and both put two bytes on the wire. Two poisons here do not FAIL, they HANG: a saturating elapsed-time subtraction makes a client that has been up 49.7 days never time out. So the poison runner now bounds every run and kills the process tree, and the wrap is pinned by a unit test the differential cannot express. Then the **outgoing packets** — SUBSCRIBE, UNSUBSCRIBE and PUBLISH, built WITHOUT COPYING the caller's filters or payload: 59 lines, and a packet turns out to be one stream and several GATHERS whose split is not symmetric. A SUBSCRIBE spends three vectors on a topic and an UNSUBSCRIBE two, against a four-vector array whose count is never reset before the filter loop, so a SUBSCRIBE's first gather carries NO filter and an UNSUBSCRIBE's carries one — `v2/5:5,v3/6:6` against `v4/10:10`, and the same bytes either way. Three deliberate breakages of that arithmetic changed no line of the trace, because a gather boundary is INVISIBLE to a transport offered one vector at a time; implementing `writev`, which the C hands the outstanding vectors AND their count, made all three fail — and that was not an optimisation, because `sendMessageVector` had been claimed as remade with its `writev` branch missing. Two more poisons still do not fire and neither is a workload gap: at `MQTT_SUB_UNSUB_MAX_VECTORS = 4` a SUBSCRIBE's room is one vector and the cheapest filter costs two, so the arithmetic cannot be wrong in a way that shows — which is a property of the NUMBER, and a unit test states it in terms of the three constants. Reading that guard closely found a CWE-787: `4U - 3U` is unsigned, so a maximum below 3 makes it `SIZE_MAX`, always true, and the loop writes past a stack array (§5.4). And reading the two senders side by side found two defects in the PREVIOUS slice — `sendMessageVector` compares elapsed time with `>` and reads the clock only when it will loop again, where `sendBuffer` thirty lines away compares with `>=` and reads it every turn — which this crate had transcribed the same way twice and the trace had never asked about. Then **opening a connection** — `MQTT_Connect`, the CONNACK and the two session handlers: 39 lines, and the FIRST function in the library that both sends and receives, so both directions are scripted and both logged. A CONNECT with no properties of its own still carries FIVE: coreMQTT invents a Maximum Packet Size set to the network buffer's size, and the application supplied none of it. Two timeouts, and only one resets — the CONNACK's header is retried against either a clock or a RETRY COUNT depending on whether the caller passed one, and the body's ten-millisecond poll restarts on every byte, so it bounds the GAP and not the packet; one byte every nine milliseconds never times out and one every ten fails on the first gap. `handleCleanSession` clears the stored PUBLISHes, ZEROES the outgoing record array, and then asks `MQTT_PubrelToResend` — which reads that array — what PUBRELs to clear, so a clean session leaks one storage entry per in-flight QoS 2 publish (§5.4); the resumed case re-sends exactly that record, which is what makes it a defect rather than an empty array. And a deduplication that was TRIED AND REVERTED: `ServerSettings` and `ServerLimits` have the same nine fields and are not the same type, because their ZERO means different things — an absent Maximum QoS is 2 in a session and 0 in a packet. Then the **receive loop**, the last of `core_mqtt.c`: 52 lines, and a process loop has THREE inputs — the transport, the clock, and the APPLICATION CALLBACK, which decides whether the packet was accepted, what reason code the acknowledgement carries and whether it carries properties, so the callback is scripted too and every packet it is handed is logged. Two findings came out of that log rather than out of any status. An acknowledgement with properties and NO reason code is never sent: the C's sentinel for "the application set none" is 0xFF and the reason-code validator has no case for it, so one Reason String added for diagnostics costs the whole PUBACK and the handshake stalls (§5.4). And the callback is handed an UNINITIALISED `pReasonCode` for a PUBLISH and a PINGRESP — two of the four places that build a `MQTTDeserializedInfo_t` fill in three of its four members — which showed as four different large numbers for the same input (§5.4). A poison then found an infidelity in OUR arm: "the property buffer is never emptied" could not fire, because this crate started a fresh builder on every callback where the C's lives in the context and its index SURVIVES; giving the builder a `resume` made the reset load-bearing and the poison fired. Then the **last two functions**, and the only two in the library with NO observable behaviour: `logConnackResponse` and `logAckResponse` spend their whole bodies calling `LogError` and `LogDebug`, which the shipped config defines as nothing, so on a stock build they are switches that do not do anything. Held back for several slices under the rule that a remake producing no log line has not remade a logger — and then remade as tables that RETURN their strings, which is the only honest shape for a crate with no logger and strictly more useful, because the C's message is unreachable unless the application defines the macro. Their oracle is the pinned SOURCE TEXT rather than a run, because turning the logging on would mean building the pin with a configuration it does not ship: `oracle/logtable.py` parses both switches into a checked-in trace and `tests/logtable.rs` sweeps all 256 codes in each direction against it, with the instrument named for what it is. Two of the C's messages are WRONG — a publish acknowledgement's 0x80 is Unspecified Error and reads "Connection rate exceeded", which is a CONNACK's 0x9F — and both are transcribed, because a transcription is not a correction and pinning the defect means an upstream fix shows up as a disagreement. Then coreMQTT's CMock vectors, then a real broker — `rumqttd` per the 2026-09-10 retarget, NOT AWS IoT Core, which §2.2 marks NEVER

**Why here.** transport-agnostic by design: it takes send/recv callbacks, so it can be proven over the HOST's TCP long before `rusty_rtos_tcp` exists

### 5. `rusty_rtos_http`

**Status.** ⚡ **COMPLETE 2026-09-20 — ALL NINE of coreHTTP's nine public functions, across 7 gated slices, with 20 mechanised poisons (`oracle/poison.py`) every one of which fails the differential; what is left is not coreHTTP but a real server**

**The oracle, and what the kill test can be.** The request side is compared BYTE FOR BYTE, because it delegates nothing. The header trace dumps the whole buffer PAST the reported length, which is what pinned coreHTTP's two partial-write behaviours: `httpHeaderStrncpy` copies up to the offending character before failing, and the partial write lands on the header terminator, so a rejected field leaves `\r\n` as `X\n` while `headersLen` still calls the block complete. The send slice is compared CALL FOR CALL over a scripted transport and a scripted clock. Two instrument lessons came out of it: a `grep -vE "^\s+\*"` meant to drop comment lines also dropped `*pBuffer = '-';`, and the C arm refuted the defect that filtered read implied before it was written up; and poisoning the retry boundary HUNG the differential rather than failing it, so both arms' scripted transports now print `send RUNAWAY` past a margin and the same poison fails in 0.00s. **llhttp is NEVER remade here** (the package plan, §2.1) — coreHTTP delegates every byte of a RESPONSE to it, so the response side's differential will compare ANSWERS, not internals, and must say so every time it is quoted. Then the **response reader**: `ReadHeader` in 20 cases, and the first claim in this package WEAKER than byte for byte, deliberately — it compares the answer AND the offset, because a lookup that found the right number of bytes in the wrong place would pass a length comparison. Then **receive and parse**, the streaming response path and the largest answer-for-answer claim here: 34 cases / 109 lines, and the trace carries the header and body OFFSETS as well as the body BYTES. It carries the bytes because a poison put them there — removing the chunked body's compacting move entirely left the same offset and the same length with the wrong content, and the differential PASSED. Four of the answers could not have been guessed from reading the C: a header with an EMPTY value still counts; a chunked response's LAST trailer never does, because a complete header is flushed by the one that follows it and nothing follows the last; a buffer that fills mid-header reports a content length of ZERO though its `Content-Length` header was whole; and llhttp does not set the status code until the reason phrase has at least one byte. And a stale comment found by measurement: `httpParserOnHeadersCompleteCallback` states that `\r\n\r\n`, `\r\n\n`, `\n\r\n` and `\n\n` all end a header block, and against the pinned llhttp only the first does — `\r\n\n` answers `InvalidCharacter` and a lone `\n` answers `ParserInternalError`, an arm the C's own switch believes unreachable. The comment describes `http-parser`, which llhttp replaced. Then **the connection**, `HTTPClient_Send`: 19 cases / 90 lines carrying calls, request BYTES and response ANSWERS at once, plus the CLOCK's call count, which is there because two loops read it and which one is retrying cannot be seen in the status. That count is also what makes the library's own substituted clock visible from outside, and this slice is the ONLY thing in the package that reaches it: an application that supplies no clock does not get one that reads zero, it gets a client whose first empty `send` is a failure and whose first empty `recv` ends the response. `Clock::Zero` had been carried unexercised since the send slice and is now pinned from both directions. One honest non-finding: Rust keeps the C's distinction between no body and a body that is present and empty, and the trace proves NOTHING can see it — `client/empty-body` and `client/post-no-length` build byte-identical request blocks, because the `Content-Length` follows the LENGTH and an empty send reaches the transport zero times. Of the C's eleven parameter checks, seven ask whether a pointer is NULL and an eighth asks whether a pointer and a length disagree; none can be written here, so three remain and those three are transcribed. Remaining: a real server

**Why here.** same shape as mqtt and strictly less protocol

### 6. `rusty_rtos_tcp`

**Status.** ⚡ **COMPLETE ON THIS BOX 2026-09-20 — ten slices, the engine question answered, and INTEROP PROVEN against stacks that are not ours. What is left needs a deployment or a chip, not a decision. The four byte-exact slices are the bit-stream codec, the stream buffer, the checksum and the address text codecs; 143 cases across 612 trace lines, plus 5,128 swept calls; 44 mechanised poisons, every one of which fails the differential; and FIFTEEN defects found in the pinned C: two CWE-787 buffer overflows, an unbounded write by API design, an overflow that rounds the largest value DOWN TO ZERO, and eleven cases across THREE different parsers of malformed text being accepted as a DIFFERENT valid address**

**The oracle, and what the kill test can be.** the +TCP API over smoltcp; interop against the host stack; `iperf`-style rows. **The number is the point: it is nearly three times coreMQTT's 13,066 function lines, and coreMQTT took about fifteen slices.** Anyone planning K7's end should plan this as its own mission rather than a package

**Why here.** **What the first slice did, and why it was that one.** `FreeRTOS_BitConfig.c` is 354 lines of pure bytes and arithmetic — no network, no timer, no socket — and it is what DHCPv6 and Router Advertisement parsing are built out of, so it is the smallest piece of +TCP whose correctness is decidable on this box, the way `backoffAlgorithm` was for K7 as a whole. Every call is followed by the WHOLE state, because a reader that returns the right value and leaves the cursor in the wrong place passes a value comparison and is a different reader. `vBitConfig_write_uc` bounds its write with `uxIndex <= ( uxSize - uxNeeded )`, both `size_t`: when the length exceeds the buffer that subtraction WRAPS, the test passes, and the memcpy runs off the end. Measured with a GUARDED allocator — `pvPortMalloc` over-allocates and fills the slack with a pattern, so the trace carries how many bytes were destroyed rather than merely crashing — it wrote **6 bytes past a 4-byte buffer, 10 past an empty one, and reported no error at all**, leaving the cursor past the end where no caller can see it. Same family as the `MQTT_SUB_UNSUB_MAX_VECTORS` finding: an unsigned subtraction used as a bound. This arm cannot reproduce it, so the two disagree on exactly three lines and the test carries a table of both arms' text for each — if upstream fixes it the table stops matching and the test says so, where a tolerance would have gone quiet. And the poisons earned their keep on their first outing: `a write resumes after an error has latched` PASSED, because nothing in the corpus attempted a post-error write that would otherwise have fit, so two cases were added and both poisons then fired. **The scope note stands and is now written into the package plan: the 38,629 lines are the size of what is NOT being transcribed.** The 2026-09-09 decision makes +TCP an API over an existing engine, so this package owes K7 three things — the 39-function API surface measured against that engine, the ~1,000 lines that are pure bytes and are the same whoever runs the state machine, and interop. The byte-exact parts are being built FIRST because no outcome of the engine measurement can throw them away, and **no claim is made about whether the +TCP API can be met over smoltcp until the per-function table exists**. Then the **stream buffer**, `FreeRTOS_Stream_Buffer.c` — the ring every TCP socket holds two of, and not an ordinary one: FOUR cursors, not two. `uxMid` iterates inside the VALID items so bytes can go to the peer and be acknowledged later, and `uxFront` iterates inside the FREE space so a segment arriving OUT OF ORDER can be written ahead of the head without the head moving over the hole in front of it. That is why the trace prints all four cursors after every call instead of the count each returned: writing four bytes at offset four leaves the head at 0 and the readable size at 0 and moves only the front, so an implementation that copied the right bytes and left the front behind returns the same count, prints the same array, and loses the segment the moment the gap is filled. 24 cases / 166 lines, plus a SWEEP of `xStreamBufferLessThenEqual` — the modular comparison that decides whether the front moved — over every `(tail, left, right)` triple at two lengths, 4,608 calls, because a comparison that is wrong only near the wrap is one that is wrong only under load. 14 poisons, all firing. Then the **checksum**, `usGenerateChecksum`, and the axis it is swept over is ALIGNMENT rather than content. The first thing that function does is read the buffer's ADDRESS (`((uintptr_t) p) & 3`) and take one of several different paths through the data — byte-swapping the running sum on odd alignments, summing four 32-bit words at a time in the middle, reading through a union of three pointer types. So the ANSWER must not depend on where the buffer sits while the CODE does nothing but. A corpus can never find a bug of that shape, because a corpus takes whatever address the allocator gave it; so the driver starts from a 4-byte-aligned base and runs every case at all four offsets, and the Rust arm is the NAIVE RFC 1071 fold with no alignment concept at all. Four genuinely different paths through optimised C against one obvious fold, at every length from 0 to 64: 18 cases and 520 swept calls, **zero disagreements**. Its poison found the input the corpus could not reach — `the carry is folded only once` passed, because the first fold carries again only when its low half lands on exactly `0xffff` and 520 pseudo-random calls never did; `ffff + ffff + 0001` was then CONSTRUCTED rather than searched for, and reading the C to place it also proved that its two unconditional folds and this arm's `while` agree on every possible `u32`, not merely on the corpus. Linking that slice needed no stubs either: `FreeRTOS_IP_Utils.c`'s other functions reference 24 symbols from the rest of the stack, and rather than write 24 prototypes to get subtly wrong the file is compiled one section per function and the linker drops what nothing reaches — safe here for a checkable reason, since `usGenerateChecksum`'s body contains no function calls at all. Then **addressing**, the `pton`/`ntop` pair for IPv4 and IPv6 — where a stack meets text a person or a config file wrote, and the one slice in the package whose Rust arm deliberately does NOT transcribe the C. `FreeRTOS_inet_pton4` does not refuse an empty octet after the first, because its emptiness check is `pcIPAddress == pcSource` and that can only ever be true while parsing octet zero: `1.2.3.` is accepted as `1.2.3.0` and `1..3.4` as `1.0.3.4`. `FreeRTOS_inet_pton6` is worse, and for a nameable reason: `prv_inet_pton6_set_zeros` ends by forcing `xTargetIndex` to sixteen, and the function's last act is `if( xTargetIndex == 16 ) xResult = 1;` with nothing guarding either line on whether the parse succeeded — so `:::` is accepted as `::`, `1::2::3` as `1::2`, `::1x` as `::`, `1:2:3:4:5:6:7:8:9` as `1:2:3:4:5:6:7:8` with the ninth group silently dropped, and `1:2:3:4:5:6:7` as `1:2:3:4:5:6:7:0` with an eighth silently invented. Eight inputs, no error reported for any of them, each one a device talking to a DIFFERENT host than it was told to. The formatters are off by one in opposite directions: `inet_ntop6` writes its NUL terminator outside every size check, so a guarded buffer measured it succeeding at size 3 for `::1` with ONE BYTE past the end while FAILING at sizes 4 and 5 that are large enough — not monotonic in the buffer size — and `inet_ntop4` refuses an 8-byte buffer for `1.2.3.4`, which is seven characters and a terminator, exactly enough. Reproducing any of that in a memory-safe stack and calling it fidelity would be the wrong trade, so **this arm is strict and the 25 disagreeing lines are GENERATED into a checked-in table by `oracle/divergences.py`**, with a test asserting the set is neither larger nor smaller — a hand-written expectation is a place to record what you wished had happened, and two of the eleven defects were found by the generator rather than by reading. The poisons earned their keep three more times: one found DEAD CODE (an IPv6 trailing-text check nothing could reach, since the loop refuses any byte it cannot use and therefore only ever exits at the end of the string), and two could not fire because each range is guarded TWICE, by an explicit bound and by the `try_from` that narrows the accumulator — the same shape as `bitconfig`'s slice bound, recorded in `oracle/poison.py` as poisons written down rather than run. Then **the API measurement**, which is the decision this plan put to K7 on 2026-09-09 and the gate on everything after it. All **78** public entry points — not the 39 this plan first wrote, which came from a regex that only matched certain return types — censused out of the pinned V4.4.1 headers and given a verdict against smoltcp 0.12 (0.14 needs rustc 1.91; our MSRV is 1.85). **8 cannot be met as promised, and 7 of those 8 are ONE CONTRACT**: the zero-copy handout that gives an application a raw pointer INTO the stack's own buffer pool to keep across calls — `GetUDPPayloadBuffer` and its `_Multi` twin, both `Release*PayloadBuffer`, and `get_rx_buf`/`get_tx_base`/`get_tx_head`. smoltcp takes a CLOSURE over the buffer instead (`F: FnOnce(&mut [u8]) -> (usize, R)`, read in the source rather than assumed), which is the same saving with no pointer outliving the call — and is what a `forbid(unsafe)` crate could offer whatever engine sat underneath. The eighth is `FreeRTOS_mss`: smoltcp 0.12 keeps `remote_mss` private with no getter, a one-line ask now filed in the upstream table. **So the API is meetable and the from-scratch stack stays shut**, at the cost of one honest note: the zero-copy family gets a different SIGNATURE, so a Kairos firmware cannot be a line-for-line port of a +TCP one there. The measurement is a JOIN and not a document — `api.census` extracted from the headers, `api.verdicts` a row per entry, and a script that refuses unless the two cover each other exactly, because a measurement that can silently omit the one function that does not fit is not a measurement. All three refusals were provoked before the result was believed, the same way a differential is poisoned, and both scripts now run in the package's CI so neither can drift from the C or from each other. Then **the entry points no engine can supply** — the 28 the measurement puts in the `ours` column, of which this slice takes every one with behaviour. `FreeRTOS_EUI48_pton` is the THIRD parser this package has measured and has BOTH of slice 4's faults: `01:02:03:04:05:06junk` parses, and so does `01:02:03:04:05:06:07` — as the first six bytes, with the seventh silently dropped, because the separator is checked after the byte is stored and after the have-we-got-six test — while a FAILING parse has already written to the caller's buffer (`01:02:zz:04:05:06` returns failure with two bytes in it, `01:02:03:04:05:` with five). Three parsers, three times the same pair of faults: a pattern in the library rather than three accidents. `FreeRTOS_EUI48_ntop` then turns out to take **no buffer size at all** — its four arguments are the source, the target, a case character and a separator — so it writes eighteen bytes into whatever pointer it is handed, an unbounded write by API design; a poison found that the differential could not test the bound because the C has nothing to compare it against, and a unit test stands in. And `FreeRTOS_round_up` computes `(a + d - 1U) / d`, which OVERFLOWS BEFORE IT DIVIDES: `round_up(0xFFFFFFFF, 4)` answers **0**, so rounding the largest value up gives the smallest and a size calculation would ask for nothing. Both rounding helpers also `configASSERT( d != 0U )` and then guard the division anyway, so on a shipped build with assertions off a zero divisor is silent and defined — measured with a COUNTING assert hook rather than one that ends the run, which is what let the trace record both the complaint and the answer. What AGREED is stated as a result too: `FreeRTOS_add_int32` and `multiply_int32` are correct saturating arithmetic across 1,458 calls over 27 edge values squared, and every formatter agrees on every well-formed input — the defects are all on the way IN, which is where they would be. 19 divergences recorded, 7 poisons firing, and the package now stands at **seven slices, 38 tests, 741 pinned lines, 64 mechanised poisons and 44 recorded divergences, every one of them a defect in the pinned C**. And then **the socket surface itself**, which the measurement had made plannable and which smoltcp's `Loopback` device made PROVABLE on this box with no hardware and no peer — a spike confirmed a full TCP exchange in one process before the plan leaned on it. The surface is built: a `Stack` owning the interface, the socket table and the driver; one `Socket` handle across TCP and UDP, which is what `Socket_t` is; `socket`/`bind`/`listen`/`connect`/`send`/`recv`/`sendto`/`recvfrom`/`shutdown`/`closesocket`, and the thirteen `direct` accessors. It **allocates nothing** — the socket table, every socket's buffers and the interface all borrow from the caller, so the whole thing builds for `thumbv7em-none-eabihf` with no allocator, and CI builds `--no-default-features` to keep that claim true. Two departures are deliberate and both are VISIBLE in the pinned trace rather than in a footnote: the zero-copy family becomes `send_with`/`recv_with`, a closure that borrows the buffer for exactly the length of the call — nothing to release, no pointer outliving it, and the reason a +TCP application cannot be ported line for line at those seven call sites; and the surface DOES NOT BLOCK, answering `WouldBlock` where +TCP would have waited, because the adapter between a poll-driven engine and a blocking call wants the Kairos kernel's notify and delay and is therefore a cross-package integration rather than an engine question. The evidence is weaker than every other slice's and says so: +TCP's socket functions will not run without its whole IP task and a driver, so there is no C arm, and a 42-line behavioural trace is pinned instead — a REGRESSION PIN, not an oracle, which means nothing without the thirteen poisons that make it fail. Six of those thirteen SURVIVED the first run, every one a misuse the scenario never performed (binding a stream socket, listening on a datagram one, sending after a close, `send_to` on TCP, and nothing ever looking at the socket table); six cases and an `open_sockets` accessor later, all thirteen fire. The package now stands at **seven slices, 38 tests, 741 pinned lines, 64 mechanised poisons and 44 recorded divergences**, and Then **blocking with a timeout**, which the socket slice had left out and called a cross-package integration — one step too cautious. It wants the kernel's notify and delay, yes, but through a SEAM, not a dependency, which is what this house does everywhere: `rusty_rtos_http` names a `Transport` and a `Clock`, not a socket. So `Host` is a trait with two methods and **`rusty_rtos_tcp` names `rusty_rtos_kernel` nowhere**. There is ONE loop, because two drift: coreMQTT's `sendMessageVector` compares elapsed time with `>` and `sendBuffer` thirty lines away with `>=`, and that exact drift is a poison here. The trace pins a closed port answering `NotConnected` after ONE round trip against a silent host timing out after twenty — different failures a client must not confuse — and back-pressure, where a full pipe makes a blocking send WAIT rather than answer `Ok(0)` and spin. Six of the thirteen socket poisons and two of the seven blocking ones survived their first run, every one a path the scenario never walked; all twenty fire now. The poison runner is also BOUNDED now, because a poison that breaks a timeout does not fail, it HANGS — the same lesson `rusty_rtos_mqtt` learned on its send plumbing. Then **the vertical**, in `tools/vertical` in the umbrella rather than in either package, because both are unpublished and a dependency between them would break H-07's fresh-clone rule: `HTTPClient_Send` completing over these sockets, and coreMQTT's CONNECT going out in its VECTORED pieces (12, 1, 5, 2, 15 bytes) through a real socket without the test having been written to exercise that path. And then **INTEROP**, which the slice before had recorded as impossible here — an assumption that took one command to refute: WSL runs as root with `/dev/net/tun`, so a TAP device can be made and a FOREIGN stack put on the far side. Against **the Linux kernel's own TCP** carrying a stock `python3 -m http.server`: `status=200 headers=5 content-length=27`, five real headers no hand-written responder would have thought to send. Against **mosquitto 2.0.22**, a third-party C broker: CONNACK accepted, a publish away, and the connection held **two minutes across sixty keep-alive periods, 5,939 loops, ZERO failures, 62 two-byte PINGRESPs** — and mosquitto reaps a client three seconds late, so surviving a hundred and twenty is the broker itself stating our keep-alives were on time. The first run of that reported `pingresp=0` and the number meant nothing: `process_loop` SWALLOWS a PINGRESP, which is its documented difference from `receive_loop`, so the handler cannot see one. The binary now says where the evidence actually is and refuses if nothing arrives after the CONNACK, because being CONNECTED is not being KEPT ALIVE. **What was left of K7 was a deployment and a chip — and the chip was NOT one.** `qemu-system-arm` 11.1.0 with nine Cortex-M machines and `thumbv7m-none-eabi` were already installed on this box, and four kernel cells already boot on `mps2-an385`; one command answered what this plan had listed as an owner action alongside buying a board. **The question a part can answer is narrower than interop and sharper: every differential in K7 ran on x86-64, so none of them tested whether the same code gives the same answers at HALF THE POINTER WIDTH.** `tools/vertical/k7-battery` is one battery over all six K7 packages, compiled from one source for two architectures, both arms asserting against ONE pin table measured on the host — and all six digests plus the roll-up are **byte-identical on a Cortex-M3**. The TCP section is not a token gesture at the package: the checksum at every prefix length including the odd ones, all three text parsers and all three formatters, `BitConfig` written and read back with the error latch provoked, the stream buffer driven past the end of its array so the wrap is exercised, and `round_up(0xFFFFFFFF, 2)` — the C overflow that rounds the largest value to zero — which the part has to REFUSE too, and does. The one design decision that makes any of it mean something: a `usize` is folded as eight **big-endian** bytes rather than its native bytes, because otherwise the two arms would differ on pointer width alone, every run would "fail", and the instrument would be measuring `size_of::<usize>()` instead of the code. Two things were checked before the PASS was believed. **Does the part execute it, or print a constant its compiler folded?** — the battery is a pure function of literals, so LLVM may evaluate the whole thing at compile time, and it would do so with the TARGET semantics, so the number would still be RIGHT and the part would be reporting its compiler's arithmetic. Making all 44 input sites opaque with `core::hint::black_box` grew the ELF by **60 bytes**; folding six parsers back into an image costs kilobytes, so the code was already there and running. **And can the gate fail?** — two poisons, both on the part: one pin altered gives `FAIL tcp` and exit 1, and one INPUT byte altered (`0x45`→`0x46`) makes the part recompute a DIFFERENT digest, which is only possible if it is genuinely running the checksum over the data. The cell holds no expected values of its own, so it cannot be re-pinned into agreeing with itself. What the chip does NOT cover is the `std`-only surface — the blocking loop and the smoltcp engine — which is why the interop rigs are not replaced by it. The **`iperf`-style rows** are done too: **tx 30.89 MiB/s, rx 46.07 MiB/s**, best of 3 over 256 MiB per rep, `CLOCK_MONOTONIC`, work parity checked EVERY rep (the peer counts what it saw and the script refuses to report a rate unless both ends agree), and the timer stopping when the send buffer DRAINS rather than when the copy returns. The spreads are 5.0% and 0.7% and do not overlap, so rx being about half again faster is a real repeatable asymmetry rather than noise — recorded as a baseline, not explained, because nothing here has measured why. A userspace poll loop over a TAP device on a workstation is not a NIC on a part and bounds nothing about embedded performance, which the script prints on every run. Three defects in that rig were found and fixed before any number was believed: a first run so short that process launch dominated (4 MiB in 0.089s is not a number), a hardcoded local port that reused the whole 4-tuple, a readiness probe that CONSUMED the single `accept()` it was checking for, and a shared output file whose fork/truncate race let one rep read the previous rep's `listening` line. Both substitutions were printed by the scripts themselves rather than left for a reader to find — and **both are now gone.** The rumqttd one should never have been made: this plan recorded "`rumqttd` 0.20.0 does not build on this toolchain, measured" and that was WRONG. `--locked` pins a `metrics` that fails with E0521; without `--locked` it installs and runs. A wrong refutation is the expensive kind, because nothing revisits it and it reads authoritative for having a version number in it — and this one would have told the next person that the broker the kill test NAMES was unavailable. | by far the largest, and the only one that needs a part for its headline number

## The eighteen re-run under the corrected harness: all pass, and RV32's hour is 24 of 25 (2026-09-21)

The earlier "18/18" was measured by a harness that counted any line beginning
with a scenario's name as a pass, so it proved eighteen lines were printed.
Re-run under the corrected one, which requires the word `ok`:

**All eighteen pass.** Every row reports `ticks=3,600,000` or more, with its
own check task still running. The heaviest: `semtest` 54,129,625 lines,
`QueueSetPolling` 49,449,190, `recmutex` 49,890,470, `BlockQ` 48,395,537.

### RV32's hour, whole and honest

| | |
|---|---|
| the original eighteen | **18/18**, re-verified under the corrected harness |
| the seven that had never been soaked | **6 pass**, `AbortDelay` fails |
| **RV32 total** | **24 of 25** |

`AbortDelay` is the single failure: `pass=false runaway=false ticks 3600009 of
3600000`. It reaches the full length and its check task declines to pass.

**M3 has had none of this.** Its eighteen were measured with the broken
harness and it has never seen the seven.

### ★ A timing discrepancy, recorded and NOT explained

This run took **8 minutes**. The record for the same eighteen on the same cell
says **58 minutes**, with `semtest` alone at 2,769 s against the 65 s it took
today — a factor of roughly 42.

The length is not in doubt: every row prints its tick count and every one is
at or past 3,600,000, so these are full-length runs and not a shortened build
that a stale `option_env!("KAIROS_SOAK_TICKS")` might have produced. That was
the first thing checked, because it is the failure that would look exactly
like this.

Beyond that, **the cause is unknown and is left unknown here.** Candidates not
tested: a warm build cache against a cold one, other load on the box during
the earlier run, or a real change in the kernel's trace cost between the two
dates. Guessing between them would put a third unverified mechanism into this
ledger in one day, after two were already refuted. The honest record is that
both numbers exist, the newer one is reproducible from the command above, and
whichever is quoted should carry its date.

## AbortDelay's hour failure, bisected: it passes to 220,000 ticks and fails at 221,000 (2026-09-21)

"`AbortDelay` fails the hour" is not a debuggable statement. `KAIROS_SOAK_TICKS`
makes it one, because the length is a compile-time knob and a bisection is
eleven runs of a few seconds each.

```
100,000  ok        500,000  FAIL
200,000  ok        300,000  FAIL
210,000  ok        250,000  FAIL
220,000  ok        230,000  FAIL
                   224,000  FAIL
                   223,000  FAIL
                   222,000  FAIL
                   221,000  FAIL
```

**The boundary is between 220,000 and 221,000 ticks**, on RV32, reproducible.

### What the verdict means, precisely

The cell reports `pass=false runaway=false`. `runaway=false` matters: the
scenario is not looping without end and it is not hanging — it reaches the
full requested length every time, including at 3,600,009 ticks.

`pass` comes from `State::still_running`, which is our transcription of the
C's `xAreAbortDelayTestTasksStillRunning`, and it returns false for exactly
three reasons:

1. `controlling_cycles` has not moved since the previous check,
2. `blocking_cycles` has not moved since the previous check,
3. `error` is set.

**Which of the three it is has NOT been determined**, and is not guessed at
here. The next probe is to print the three separately at the failing length,
which is a one-line change to the cell and a single run — cheap enough that
speculating first would be the expensive option.

### Why this is worth having even unfinished

A 1,000-tick bracket is a different kind of object from "fails after an hour".
The failing window is now small enough to trace in full: the scenario emits
about 1.27 lines per tick at this length, so the last few thousand ticks
before the boundary are a few thousand lines, readable end to end and diffable
against the same window at 220,000 where it passes.

It also bounds the blast radius. `AbortDelay` is already the corpus's known
divergent scenario for a CONTRACT reason at the pinned length — the C harness
keys a queue's trace ordinal on its malloc address — and that is a conformance
gap at 2,000 ticks. This is a separate liveness failure two orders of
magnitude further out, and the two should not be assumed to share a cause
just because they share a scenario.

## Both emulator hours, on the whole 25-scenario corpus: 24 of 25, and they agree exactly (2026-09-21)

M3 followed RV32 through the corrected harness, on the full corpus rather
than the eighteen the hardcoded list used to cover.

| | RV32 | Cortex-M3 |
|---|---|---|
| scenarios attempted | 25 | 25 |
| passed the hour | **24** | **24** |
| failed | `AbortDelay` | `AbortDelay` |
| wall clock | 8 min | 13 min |

**The same scenario fails on both, and only that one.** Two architectures
agreeing to the scenario is what says this is a kernel-or-corpus property
rather than anything about either target — the same reasoning that made
`StreamBufferDemo`'s identical divergence on both emulators worth trusting.

Heaviest row on both: `IntQueue`, 79,912,747 trace lines and 2.59 GB, in 75
seconds on M3 against 78 on RV32.

**So K3's "an hour each on M3-qemu and RV32-qemu" is now measured on the
whole corpus, under a harness that can report a failure, and the answer is 24
of 25 on each.** It was previously recorded as closed at 18/18 on each, which
was a smaller corpus counted by a harness that could not tell `ok` from
`FAIL`. The remaining clause is the C6, which is hardware.

## Both C6 cells exist and build, so the clause waits on hardware alone (2026-09-21)

K3's last two clauses need an ESP32-C6: the third hour, and the cycle rows
"from the C6, not from QEMU". Neither can be measured here — no C6 has ever
been on this bench.

**But "needs hardware" was hiding a second cost.** There was **no C6 cell of
any kind** in the family — 20 firmware cells, none for a C6. So the day a
board arrived, the work would not have been "plug it in": it would have been
write two cells, discover the toolchain differences, and only then measure.

Both cells now exist and compile:

| cell | target | builds |
|---|---|---|
| `rusty_rtos_kernel/firmware/esp32c6-cycles` | `riscv32imac-unknown-none-elf` | ✅ 0 errors |
| `rusty_rtos_demo/firmware/esp32c6-corpus` | same | ✅ 0 errors, **and with `--features soak`**, which is the hour itself |

**Neither has ever been flashed and neither carries a number.** Both say so
in their own banner, because a cell that builds is not a cell that has run
and the distance between those is where claims go wrong.

### What the build actually proves

Not much on its own — so it was checked rather than assumed:

- the cycles binary is a **209 KB rv32 ELF** carrying **20 `mcycle` reads**
  in its disassembly, so the instrument survives to the artifact and is not
  optimised out;
- the corpus binary is **450 KB** and places `.flash.appdesc` at 0x42000020,
  which is the segment espflash refuses an image for lacking;
- the corpus builds **with and without `soak`**, so the hour is one flag away
  rather than an unexplored path.

### Two things learned writing them, both cheap now and expensive later

**`-nostartfiles` is Xtensa-only.** Every Xtensa cell passes it; `rust-lld`
rejects it outright on RISC-V, where `riscv-rt` supplies the startup.
Copying an Xtensa cell's `.cargo/config.toml` verbatim fails at link with
`unknown argument '-nostartfiles'`. `-Tlinkall.x` is still required on both.

**The C6 builds on STABLE.** No esp toolchain, no `build-std`, because RV32
is an upstream rustc target where Xtensa is not. Every Xtensa cell in this
family needs `cargo +esp`. So a C6 cell is the only silicon cell CI could
ever *build*, even though CI can never *run* it — which is an argument for
the C6 beyond the plan's own.

### What remains, precisely

A board. Then `cargo run --release` in each, and the numbers go in this
ledger with a date. The C arm of the cycle rows still needs ESP-IDF as a
platform layer — and on a C6 that arrangement is better than on the S3,
because the oracle's own `portable/GCC/RISC-V` port is **first-party** where
its Xtensa port is ThirdParty.

## ★ AbortDelay is NOT a liveness failure — it catches a fault, and I had it wrong (2026-09-21)

The hour's single failure was recorded here twice as a liveness problem: "its
check task declines to pass", filed beside `runaway=false` as though the
question were whether the scenario kept running. **That reading is wrong**,
and one probe settles it.

`State::still_running` returns false for three reasons — controlling cycles
stalled, blocking cycles stalled, or `error` set — and the cell only ever
reported the answer, never which. `tests/abortdelay_hour.rs` asks:

```
     ticks    pass   controlling      blocking   error
    220000    true           868           868   false
    221000   false           872           872    true
   3600000   false         14904         14904    true
```

**Both cycle counters climb the whole way** — 868 at the passing length, 872
just past the boundary, **14,904** at the full hour. Neither task ever stops
being scheduled. What fails is the third condition: **`error` is set**, which
is `prvCheckExpectedTimeIsWithinAnAcceptableMargin` — the scenario itself
catching a block that came back outside its allowable margin.

### Why the distinction matters

A stalled counter is a scheduling problem: something stopped running. An
`error` is a **timing-correctness** problem: everything ran, and a blocked-for
time was wrong. They have different causes, different fixes, and only one of
them is about liveness. The hour's claim — "every check task still reports
running" — is in fact SATISFIED by this scenario; it is the scenario's own
assertion that fails.

So `AbortDelay` fails the corpus at the pinned length for a **contract**
reason (the C harness keys a queue's trace ordinal on its malloc address) and
fails the hour for a **timing** reason, and the two still should not be
assumed to share a cause — but neither of them is the liveness failure this
ledger called it.

### What is now known, and what is not

**Known.** The fault first appears between 220,000 and 221,000 ticks, at
roughly the 869th to 872nd cycle of the test — so it is not present from the
start and is not a wrap of anything at a round power of two. It reproduces on
the host, on RV32 and on Cortex-M3.

**Not known, and not guessed.** `outside_margin` fails in two directions —
`blocked < expected` or `blocked > expected + ALLOWABLE_MARGIN` — and the
comment in the source says which one is interesting: *"A blocked-for time
that is too short is the interesting direction: it is what an abort firing
early would look like."* Which direction this is has NOT been measured. That
is the next probe, and it is one more field on the same test.

### The lesson worth keeping

The bisection was good work and produced a wrong description, because a
bracket tells you WHEN and says nothing about WHAT. Three runs answered a
question eleven runs of bisection could not, and the difference is that these
asked the program what it thought rather than narrowing where it changed its
mind.
