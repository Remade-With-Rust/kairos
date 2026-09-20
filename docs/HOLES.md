# Holes

What is **not** proved, and how that was established. Companion to
`docs/LEDGER.md`, which records what is.

Every row here was measured, not suspected. Where a method has a known
false-positive mode it is named, because a hole list that overstates is as
useless as one that misses — and this one already caught itself overstating
twice (see *Method*, below).

Ordered by how much a reader should care, not by how easy the fix is.

---

## H1 — Three of the four kernel instruments never block a task — CLOSED 2026-09-19

**Closed.** All four now park a task on a real timeout and assert
`Wait::Blocked` on it: `khot-ir` on a starved queue (3) and a semaphore
(2), `kipc-ir` on a notification (2) and a stream buffer (3), `kobj-ir`
on an event-group bit nobody sets (2), `kdelay-ir` on a queue (3). The
assertion is `matches!(.., Ok(Wait::Blocked))` and not `is_ok()`, because
a call that declines to block is also `Ok` — which is how the hole hid.

*The measurement that opened it is kept below; a closed hole that deletes
its own evidence teaches nothing.*

**Measured, as it stood.** Zero-timeout calls per bench, and how many ever
asserted `Wait::Blocked`:

| bench | zero-timeout calls | blocking asserts |
|---|---|---|
| `khot-ir` (queues) | 4 | **0** |
| `kipc-ir` (notifications, stream buffers) | 8 | **0** |
| `kobj-ir` (event groups) | 5 | **0** |
| `ksched-ir` | 0 | **0** |

Every queue, stream-buffer and event-group call in those three passes `0`
as its timeout, so nothing is ever parked. The blocking half of the IPC
machinery — `vTaskPlaceOnEventList`, the dual-list unlink on timeout, the
wake ordering by priority — is measured by **nothing**.

This is the same defect `kdelay-ir` had this morning and it is fixed there
only. That fix mattered: widening `kdelay-ir` to block on an event list
moved its top consumer from `wake_due_tasks` to
`add_current_task_to_delayed_list` and changed the answer on a real
optimisation. Whatever these three would say about the blocking paths, they
are not saying it yet.

**Cost of the gap:** any change to the blocking IPC paths was priced at
zero. **Fix, applied:** the `kdelay-ir` pattern — one call with a real
timeout against an endpoint nobody satisfies.

**This entry was itself stale for a day.** The fix landed in the bench
crates and the hole list was not told, which is the failure mode a hole
list exists to prevent. Closing a hole is two commits, not one.

---

## H2 — 19 public APIs have no evidence from the C differential

**Measured.** Of 130 public kernel APIs, these are called by no conformance
scenario, no runner code, no other kernel code, and no unit test:

```
name_of                 task_at                 with_tick_hook
notify_value            queue_remove_from_set   queue_send_to_front_from_isr
ready_cursor            ready_items             timer_expiry_time
timer_period            timer_pend_function_call
stream_buffer_reset     stream_buffer_is_empty  stream_buffer_is_full
stream_buffer_bytes_available                   stream_buffer_spaces_available
stream_buffer_next_message_length               stream_buffer_receive_from_isr
stream_buffer_set_trigger_level
```

`notify_value_clear`, `timer_change_period` and `timer_delete` came off
this list on 2026-09-19 when the `TaskNotify` scenario landed. `notify_value`
stayed on it: the scenario reaches `xTaskNotifyAndQuery` through
`notify_and_query`, which is one critical section, and `notify_value` is
the two-call spelling only the capi shim uses.

They are **not dead**: `rusty_rtos-capi` calls most of them, so a C program
linking the shim reaches them. `vTimerDelete`, `xQueueRemoveFromSet`,
`xStreamBufferReset`, `ulTaskNotifyValueClear` and the rest answer with Rust
behaviour that **has never been compared to FreeRTOS's**.

The gate's whole claim is "does the Rust kernel decide what the C kernel
decides". For these, nobody has asked.

**Cost of the gap:** a semantic difference here surfaces in somebody's
application, not in CI.

### What closing it takes, scoped (2026-09-19)

The strongest evidence needs a C original to diff against. Four of the
vendored-but-unused demos supply one, and the ratio for a port is about
0.7 Rust lines per C line (`TimerDemo` is 1,226 → 861):

| work | C lines | H2 APIs it closes |
|---|---|---|
| **`TimerDemo` Test7** + one harness line | **107** | `timer_change_period` — **closed by `TaskNotify` instead** |
| ~~`TaskNotify` scenario~~ | 721 | ~~`notify_value_clear`, `timer_delete`, `timer_change_period`~~ — **DONE 2026-09-19, see below** |
| `QueueSet` scenario | 1,160 | `queue_remove_from_set` |
| `StreamBufferDemo` scenario | 1,247 | six: `reset`, `is_empty`, `is_full`, `bytes_available`, `spaces_available`, `receive_from_isr` |

`StreamBufferDemo` is the best coverage per scenario. **Test7 is the best
coverage per hour** — it extends a scenario that already passes, needs no
new plumbing, and is an order of magnitude smaller than any new port.

**Seven have no C demo at all** and no differential is possible for them:
`xTimerGetPeriod`, `xTimerGetExpiryTime`, `xTimerPendFunctionCall`,
`xQueueSendToFrontFromISR`, `xStreamBufferNextMessageLengthBytes`,
`xStreamBufferSetTriggerLevel`, `pcTaskGetName`. Those need a semantic
audit against the C source plus unit tests pinning the contract — weaker
evidence, and the only evidence available.

### `TaskNotify` was blocked, and is not any more (2026-09-19)

Its C side was already wired up -- registered in the harness, with an ISR
hook -- so it looked like only the Rust port was missing. It was not. The
demo **could not conform at all**:

```c
uxNextRand = ( uint32_t ) prvRand;   /* seed = a function's LOAD ADDRESS */
...
xPeriod = prvRand() % xMaxPeriod;    /* and that picks TIMER PERIODS */
```

Timer periods decide when the daemon wakes, which is all over the trace,
and ASLR varies the address per run — so the oracle did not reproduce
*itself*:

| run condition | result |
|---|---|
| 3 runs, ASLR on, as vendored | 3,421 / 3,336 / 3,530 lines |
| 3 runs, `setarch -R` | identical, 3,530 |
| 3 runs, ASLR on, seed pinned | identical, 3,519 |

Pinned in `kairos oracle patch` — not in the file, because
`oracle/FreeRTOS/` is gitignored and an edit there evaporates at the next
fetch. `TaskNotify` is now a porting job like any other.

**Ported 2026-09-19: 3,519 lines identical, exits included** (`ticks=2001
yields=227 exits=2845 lines=3518`), pinned with the other nineteen, and
`conform --all` is 20 scenarios identical. The estimate above was 721 C
lines at ~0.7; it came out at 806 Rust lines, and it agreed with the C
kernel for 565 lines on the first run.

It found one defect, and the defect was the **kernel's**, not the port's:
`prvProcessReceivedCommands` ends `tmrCOMMAND_DELETE` in `vPortFree`,
which every `heap_N.c` wraps in `vTaskSuspendAll` / `xTaskResumeAll` —
one outermost exit, spent with no trace line to show for it.
`queue_delete` and `event_group_delete` had both been told; the timer
command was the third object with a free and nothing in the corpus had
deleted a timer before. It presented as the other two did: every event
agreed and the exit column was one short from the first delete on.

It also needed one new kernel call. `xTaskNotifyAndQuery` hands back the
value it is about to replace **from inside the same critical section**;
the capi shim's read-then-notify gives the same two numbers and costs a
second one, and on the sim an exit is a tick opportunity. Hence
`notify_and_query` / `notify_and_query_from_isr`.

**The middle row is worth more than the scenario.** It says the C oracle is
deterministic on real pthreads: the POSIX port adds no scheduling
non-determinism to the trace. Every conformance number in `LEDGER.md`
depends on that and it was nowhere written down.

### Why `TimerDemo` does not already cover this

`TimerDemo` passes and its C original calls `xTimerChangePeriod` four
times, which looks like a contradiction. It is not, and the answer took a
detour worth recording.

The Rust port implements Test1–Test6; the C has Test7,
`prvTest7_CheckBacklogBehaviour`, which is where those calls live. That
looks like an incomplete port. It is not: Test7 is guarded by
`ucIsBacklogDemoEnabled`, which defaults to `pdFALSE` and is set only by
`vTimerDemoIncludeBacklogTests()` — and `oracle/harness/main.c` calls
`vStartTimerDemoTask()` without ever calling the enabler. **Test7 is
switched off in the C oracle, so the port is faithful and the pass is
honest.** Confirmed by running `TimerDemo` at 100,000 ticks, five times
the usual budget: 156,492 lines identical. A C that reached Test7 would
diverge from a port that has no Test7, and it does not.

So the API is uncovered because the only C code that exercises it is
disabled, not because anybody skipped it. Turning it on is one line in the
harness plus the port — and both APIs Test7 needs already exist
(`xTaskCatchUpTicks` is `Kernel::step_tick`, exposed at `abi.rs:1247`,
whose doc already says it is *"used by the timer tests to jump time
forward"*). That call is itself only ever reached through the tickless
path today, so porting Test7 puts it under the differential for the first
time in the role it was written for.

---

## H3 — Four of five kernel-core files have no unit tests at all

**Measured**, `#[test]` per file:

| file | tests |
|---|---|
| `system.rs` | 16 |
| `typed.rs` | 6 |
| `name.rs` | 4 |
| **`kernel.rs`** | **0** |
| **`queue.rs`** | **0** |
| **`timer.rs`** | **0** |
| **`stream.rs`** | **0** |
| **`events.rs`** | **0** |

The four with none are the four that implement the kernel. This is not as
bad as it looks — the conformance corpus is the oracle for `kernel.rs` and
`queue.rs`, deliberately, and the ledger explains why the corpus has to be
the oracle rather than a unit suite. But it is exactly as bad as it looks
for `timer.rs`, `stream.rs` and `events.rs`, whose APIs are **also** the
bulk of H2: no unit test, and no scenario either.

The ledger's own number for what the kernel's suite pins: **13 of 342
mutants (4%)**.

---

## H4 — The mutant survey covers one file of nine, and two survivors remain

**Measured** (`docs/LEDGER.md`): `cargo mutants` has been run over
`kernel.rs` only, judged by the corpus — 42 viable mutants, **40 now
caught** after the `create_task` preemption guard was closed (2026-09-19),
two survivors outstanding and unlocated.

`queue.rs`, `timer.rs`, `stream.rs`, `events.rs`, `typed.rs`, `system.rs`,
`name.rs` and `rusty_rtos_core`'s `list.rs` and `arena.rs` have **never been
mutation-tested**. `queue.rs` is the largest file in the kernel and carries
the whole IPC surface.

The run also wedged: 44 of 47 mutants in shard 1/8 completed. The other
seven shards have not been run.

---

## H5 — `AbortDelay` is the 20th scenario and it does not run — CLOSED 2026-09-20

**Closed.** `AbortDelay` runs in `conform --all`, **2,549 lines, and the
sim's trace is byte-identical to the C kernel's**. `BLOCKED` is now empty.
`xTaskAbortDelay` is inside the gate, along with `xTaskGetHandle`,
`vTaskDelayUntil`, all three object deletes and both notify paths.

It was a contract question, and provably so: of 2,549 lines, 8 differed and
**all 8 differed only in a queue ordinal**. Both sides now number objects by
a monotonic per-kind creation counter, so identity depends on creation
ORDER rather than on a C allocator the Rust arena does not share. The
2026-09-10 note that "a running count fixes `AbortDelay` and breaks
`EventGroupsDemo`" was a correct measurement of a fix applied to ONE side.
Full write-up in `LEDGER.md`.

*The measurement that opened it is kept below.*

**Measured, as it stood:** 1,945 of 2,549 lines identical, then a divergence
the ledger argues is a *contract* question rather than a kernel one — the C
harness keys a queue's trace ordinal on its malloc address, so a
differently-sized successor to a freed object gets a new ordinal where our
arena reuses the freed index. A running count fixes `AbortDelay` and breaks
`EventGroupsDemo`.

The effect, until 2026-09-20, was that `xTaskAbortDelay` and the three APIs
that scenario exercises were outside the gate.

---

## H6 — `tail_value`'s invariant is a caller's promise with no check — CLOSED 2026-09-20

**Closed, and it made the kernel faster.** The promise was never really
about `tail_value`; it was about the PATTERN the one caller built on it —
read the tail, compare, write the value, append. `ListsOf::insert_sorted`
is that pattern, in the list, so two of the three obligations are gone:

| obligation | before | now |
|---|---|---|
| the comparison must be `>=`, not `>` | caller's, and silent when wrong | in the list, tested |
| the append must actually append | caller's: `insert_end` links before the CURSOR | **structurally absent** — it links between the tail and the marker, so the cursor cannot matter |
| the list must already be sorted | caller's, prose only | caller's, and `ListsOf::is_sorted` checks it |

`is_sorted` answers a `Result` rather than panicking, because this crate's
contract is that nothing in it panics on any input a caller can construct
(`tests/no_panic.rs`). That ruled out the `debug_assert!` this would
otherwise have been — and the predicate is the better tool anyway, since it
works in a firmware self-check and not only in a debug build.

**The first attempt got the mechanism backwards, and the measurement said
so.** Keeping `insert_end` and TESTING the cursor cost +0.09% to +0.19%
across the instruments. Removing the question instead of checking it won on
every arm:

| instrument | call site | `insert_sorted` | |
|---|---:|---:|---:|
| `kdelay-ir` | 4,024,119 | 3,995,313 | **-0.72%** |
| `kdelay-deep` | 4,368,168 | 4,344,745 | **-0.54%** |
| `khot-ir` | 14,800,504 | 14,736,516 | **-0.43%** |
| `ksched-ir` | 1,790,797 | 1,790,797 | flat |

One end read rather than two, the value written by the link rather than by
a separate `set_value`, and no cursor read at all. Checksums identical down
every column; `conform --all` still 21 scenarios identical.

Five tests pin it, including the hazard itself: `insert_sorted` on an
unsorted list DOES misplace, and that is pinned as a hazard rather than
fixed, because fixing it is the `sorted` flag `tail_value` records as a
measured net loss.

*The measurement that opened it is kept below.*

## H6 — `tail_value`'s invariant is a caller's promise with no check (as it stood)

**By construction**, introduced 2026-09-19. `ListsOf::tail_value` is only
meaningful on a sorted list, and the list deliberately does not track
whether it is sorted — a `sorted` flag cost every insert and remove on
every list and turned a 12% stage win into a 5% system loss.

So the delayed-list append is correct because of three facts about *that
list* (it takes only sorted inserts and appends; `>=` preserves the tie
rule; its cursor never leaves its marker). A second caller that adopts
`tail_value` without re-establishing all three gets a silently mis-ordered
list.

The conformance gate **would** catch it — that was tested, by breaking the
append on purpose and watching `dynamic` diverge at line 6137 — so this is
a documentation-and-review hole, not an unguarded one. It is here because
the next person to use `tail_value` will not have run that experiment.

---

## H7 — Kani proves 35 properties; the mission's target was higher

**Measured:** 35 `#[kani::proof]` functions in `proofs.rs`. Recorded
elsewhere as 25 of 35 passing at the time of writing; the gap between
"written" and "passing" has not been re-checked in this session and should
be before anyone quotes it.

---

## Method, and what it gets wrong

H2 was produced by intersecting the public API list against call sites in
the scenarios, the runner, the kernel itself, the unit tests and
`rusty_rtos-capi`. That is a **call graph by grep**, and it has two known
failure modes, both of which fired here before the list was written:

* **Indirect reach.** `tick_from_isr` appears in no scenario and is called
  on every tick — the sim reaches it through `exit_critical` →
  `tick_on_exit`. Any API reached only through an internal caller looks
  unexercised and is not. The list above excludes everything with an
  internal caller for this reason.
* **Constructor spelling.** `Kernel::new` is called everywhere and matches
  no `.new(` pattern. Excluded by hand.

The honest fix is coverage instrumentation, which is not installed on this
machine (`cargo-llvm-cov`, `grcov` and the `llvm-tools` component are all
absent). Until it is, **treat H2 as a list of candidates that survived two
filters, not as proof of absence.** Every entry should be confirmed by
running the thing before work is scheduled against it.

H1, H3, H4 and H5 do not depend on that method: they are counts of what is
in the files and what the tools reported.
