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

**One full scheduling round costs 8,313 ns — 1,995 cycles at 240 MHz — which
is 88 parts per million of a P-256 signature.** That is K5a's measurement
clause, answered on the XIAO ESP32-S3.

`rusty_rtos_kernel/firmware/xiao-s3-signing`. The workload is not ours:
`p256 = "0.13"` (RustCrypto), the same crate and major version
`rusty_esp_mid-core` signs with, over the same fixed 32-byte prehash.

| fact | value | method |
|---|---|---|
| a scheduling round | `per_round_ns=8313 per_round_cycles=1995` | a queue send, a queue receive and two context switches, timed on their own over 20,000 rounds |
| switches actually taken | `switches=40000 per_round=2` | counted, not assumed |
| as a share of one signature | **88 ppm** (8,313 ns of 94,349,000 ns) | a small measured number over a large measured one |
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
baseline?"* with a number: 88 ppm, on the same part, same crate, same clock.

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
regression bound, generous against the 88 ppm measured, because the bound is
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

**A cross-check against the sibling cell, which is worth as much as either
number.** `xiao-s3-signing` measured a scheduling round — queue send, receive,
two switches — at **1,995 cycles** by amortising 20,000 of them through a 1 µs
clock. These rows predict roughly `2 x 623 + (949 - 623) ~ 1,570` for the same
shape: same order, ~20% apart, by two different instruments on two different
arrangements. A factor-of-several disagreement would have meant one of them was
measuring something else.

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
