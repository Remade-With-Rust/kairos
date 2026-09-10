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

**And the hour runs on ARM too — where it turned into something stronger.**
`mps2-an385-qemu-corpus --features soak`: **17/17 still running after
3,600,000 ticks on a Cortex-M3 under QEMU**, exit 0. That was the liveness
clause. But the counters it printed were then diffed against the host soak's,
by machine rather than by eye:

```text
host rows: 17   M3 rows: 17
IDENTICAL -- all 17 scenarios, ticks+yields+exits, host vs Cortex-M3,
             at 3,600,000 ticks
```

**Every counter agrees, on every scenario, at 1,800x the pinned length.** The
conformance cells prove agreement with the C kernel at 2,000 ticks; this says
the ARM build and the host build stay in lockstep for a simulated hour.

**Say what it is not.** This is our two builds agreeing with each other, not
with C — there are no C pins at 3,600,000 ticks, which is why the soak claims
liveness. It is a **cross-architecture determinism** result, and a strong one:
a divergence anywhere in an hour of scheduling, blocking, timer and queue
traffic would move `exits`, which is sim time itself. It is recorded as a
ledger row rather than a gate, because re-checking it costs a three-hour QEMU
run.

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
at `docs/upstream/rusty_alloc-size-class-inversion.md`.

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
`docs/upstream/rusty_alloc-size-class-inversion.md`.

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
| the host does **not** reproduce the step | no step at 512, none at 1,024; the only step is 2,048 -> 2,049 (11 -> 91 cycles/op) | so the lower step is **not** the `direct[]` table, whose top is 1,024 here, and not a generic geometry effect |
| `MEDIUM_OBJ_SIZE_MAX` is confirmed twice | it routes on **both** platforms | the one boundary not in question |
| and the host runs **24x faster** | 11–13 cycles/op against the device's 271–314 | not a core-speed ratio — a different path. The host hits a fast path the device does not |
| what actually differs | the **prim** | a firmware gets `prim::fixed`, a host `prim::windows`/`prim::unix` — and the prim is selected by target OS, not by a feature, so it **cannot be swapped on a host** to isolate it. That run has to happen inside the crate |

**A hypothesis, labelled as one.** `alloc.rs` records that "a tight alloc/free
loop frees into `local_free`, so the queue front's `free` list is ALWAYS dry
when the next allocation arrives" — and this harness *is* that loop. If the
device takes `malloc_generic` on most operations while the host does not, the
24x gap and the step are the same fact seen twice. Unconfirmed: nothing on the
seam exposes a slow-path counter.

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
