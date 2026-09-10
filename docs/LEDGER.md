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
| K2's Kani gate | **22 of 32 harnesses verify — 32,145 checks, 0 failures** | `cargo kani -p rusty_rtos_kernel-core --harness <name>` in WSL (Kani 0.67.0 has no Windows build), harnesses in `src/proofs.rs` named after the `FreeRTOS/Test/CBMC/proofs/{Queue,Task}` directories. Swept one at a time with a 150 s budget each, because one blow-up in a combined run says nothing about the others; most finish in under ten seconds. The ten that do not converge split cleanly in two, and both halves are measurements rather than guesses — see the two rows below |
| the six that need a *started* kernel | `start_scheduler` is the wall | `task_get_scheduler_state`, `task_get_current_task_handle`, `task_switch_context`, `task_start_scheduler`, `task_increment_tick`, `task_delay`. Every ingredient of the setup is cheap on its own — `Kernel::new` 0.8 s, one task 3.0 s, three tasks 10.2 s, a task and a queue 3.2 s — but `start_scheduler` with **no user task at all** does not finish in 500 s, and what it adds over the cheap cases is the first switch. Every other harness runs on a kernel that has its tasks but has not started, which is why they finish: the call bodies under proof are the same either side of `vTaskStartScheduler`, and what is given up is the block-and-switch tail of a blocking call |
| the four that pass a symbolic handle into a kernel call | the space is not the cost | `queue_generic_send_stale_handle`, `task_priority_set_stale_handle`, `task_priority_set`, `queue_take_and_give_mutex_recursive`. The obvious economy was tried and refuted: bounding the symbolic handle's index and generation to the handful of values next to the arena — which is where a real disagreement lives, and what the C proofs do with their pointers — changed nothing, all four still timed out, one of them at 700 s. So it is not the size of the space; it is that a call which walks the arena and the lists with a symbolic handle has to be explored for every slot that handle could name |
| K2's mutants gate | **36 of 42 viable mutants caught (86%) with the corpus as the oracle** | `cargo mutants --in-place --file crates/rusty_rtos_kernel-core/src/kernel.rs --test-package rusty_rtos_demo-core --shard 1/8 --timeout 240 -- --manifest-path ../rusty_rtos_demo/Cargo.toml --release`, run from the kernel repo with the demo's `[patch]` table in scope so the mutated kernel is the one the corpus runs. 44 of the shard's 47 mutants completed before the tool wedged: 36 caught, 6 missed, 2 unviable. The mechanism was checked by hand first — a mutation planted in `queue_send_list` fails `every_scenario_reproduces_the_c_kernels_trace_and_counters` in 79 s — because a mutants run that silently tests the wrong binary reports the same shape as one that tests nothing |
| the six that survived are one finding and one equivalent mutant | measured 2026-09-09 | five of the six are the same line: `if self.running && C::USE_PREEMPTION && self.current_priority() < priority` in `prvAddNewTaskToReadyList`, the yield a task creation owes when the new task outranks the running one. **No scenario in the corpus creates a task after the scheduler has started** — every one of the fifteen creates all of its tasks in its `start`, and `Runner::start_common` creates `CHECK` and then starts the scheduler — so the arm is never reached and any mutation of it survives. The corpus's own answer to that is `death.c`, the standard demo whose whole subject is creating and deleting tasks at runtime; it is not on K2's list of eighteen, so this is a K3 row. The sixth, `>` to `>=` in `taskRECORD_READY_PRIORITY`, assigns the same value either way and is an equivalent mutant, not a gap |
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

### Still open in K2.1

The topology work proper: tasks and queues named by type rather than by
runtime handle *inside the kernel*, so the arena indirection goes and the
six harnesses that need a started kernel get cheap too. The three above are
the argument for doing it, not the doing of it — they show the effect at
the face, where there is no arena, and the kernel is where the arena is.

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
| the same claim offline | the **same digest** as `PollQ` | `tests/conformance.rs::the_async_arm_reproduces_pollqs_trace_exactly`. It cannot sit in `PINS` because it does not go through `Runner`, but it asserts every pinned number of `PollQ`'s and the FNV-1a/64 of the trace |
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

### What K2.2 does not settle

The **storage** question, which is the one between this prototype and an
executor. An `async fn`'s future is an anonymous type, and `Runner` holds
its bodies in a `[Body; TASKS]` of a `Copy` enum, which no future can join.
`PollQ-async` therefore owns its own kernel and pins its two futures as
locals, sharing the kernel by `&RefCell` because a future cannot hold
`&mut SimKernel` across a suspension and still let the driver hand the
kernel to another task. That is honest for a prototype and it is not an
executor: a general one needs somewhere for a future of unnameable type to
live, which is what Embassy's `TaskStorage` is for. **The trace result does
not depend on that choice** — the suspension points are where the answer
lives — but the choice is still open, and it is the first thing K3 would
have to settle if the async face is to be more than one scenario.

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
