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

## H2 — 13 public APIs have no evidence from the C differential

**Measured.** Of 130 public kernel APIs, these are called by no conformance
scenario, no runner code, no other kernel code, and no unit test:

```
name_of                 task_at                 with_tick_hook
notify_value            queue_remove_from_set   queue_send_to_front_from_isr
ready_cursor            ready_items             timer_expiry_time
timer_period            timer_pend_function_call
stream_buffer_next_message_length               stream_buffer_set_trigger_level
```

`notify_value_clear`, `timer_change_period` and `timer_delete` came off
this list on 2026-09-19 when the `TaskNotify` scenario landed. `notify_value`
stayed on it: the scenario reaches `xTaskNotifyAndQuery` through
`notify_and_query`, which is one critical section, and `notify_value` is
the two-call spelling only the capi shim uses.

**Six more came off on 2026-09-20**, when the `StreamBufferDemo` scenario
landed: `stream_buffer_reset`, `stream_buffer_is_empty`,
`stream_buffer_is_full`, `stream_buffer_bytes_available`,
`stream_buffer_spaces_available` and `stream_buffer_receive_from_isr`.
All six are called from `prvSingleTaskTests`, which walks one buffer's
head past its tail and back and checks the four counts after every move --
so they are now compared against the C kernel on 20,736 lines.

The two that are left are the two `prvSingleTaskTests` does not use.
`stream_buffer_next_message_length` is a message-buffer call and this is a
stream-buffer demo; `stream_buffer_set_trigger_level` is set at create time
by every scenario in the corpus and never changed afterwards. Neither has a
standard demo that reaches it, so closing them means writing a scenario
rather than porting one.

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

## H3 — Four of five kernel-core files have no unit tests at all — CLOSED for the three that needed it, 2026-09-20

**Closed where it mattered.** The entry itself named which three: the
corpus is the deliberate oracle for `kernel.rs` and `queue.rs`, and it was
`timer.rs`, `stream.rs` and `events.rs` that had no unit test AND no
scenario. Those three now have 22 tests between them.

| file | tests before | tests now |
|---|---:|---:|
| `system.rs` | 17 | 17 |
| `typed.rs` | 6 | 6 |
| `name.rs` | 4 | 4 |
| **`stream.rs`** | **0** | **10** |
| **`timer.rs`** | **0** | **6** |
| **`events.rs`** | **0** | **6** |
| `kernel.rs` | 0 | 0 — corpus-oracled, by decision |
| `queue.rs` | 0 | 0 — corpus-oracled, by decision |

**Each test pins the C's contract, quoted, not this kernel's behaviour.**
That distinction is the whole value: a test written the other way round
proves only that the code still does what it did, which is worth having
and is not evidence of conformance. For the H2 APIs no scenario reaches,
a semantic audit against the C source plus a test of what it says is the
only evidence available, and this is that evidence.

The audit found the *tests* wrong twice and the kernel right both times,
which is the outcome to want. The one worth reading is the zero-length
message: `prvWriteMessageToBuffer` writes the length header into
`xNextHead`, a LOCAL, and only commits `pxStreamBuffer->xHead` inside
`if( xDataLengthBytes != 0 )` — so a zero-length message to a message
buffer is a no-op indistinguishable from a send that failed. The first
version of that test asserted the header would show up in
`bytes_available`. It does not, in either kernel.

Contracts now pinned that no scenario isolates, in rough order of how
badly they would bite:

* a MESSAGE buffer reports `isFull` while it still has free bytes — the C
  compares spaces against the length HEADER, not against zero;
* the timer API is a COMMAND QUEUE, so `xTimerStart` returning pdPASS
  means "queued", `xTimerIsTimerActive` keeps saying false until the
  daemon runs, and `xTimerGetPeriod` answers the OLD period after a
  successful `xTimerChangePeriod`;
* `xTimerGetExpiryTime` is an unguarded list-item read — valid only when
  `xTimerIsTimerActive` says so, and the return type cannot tell you;
* `xEventGroupSetBits` can answer a value WITHOUT the bit it just set,
  because a waiter woke inside the call and its clear-on-exit ran first;
* `xEventGroupClearBits` answers the value from BEFORE the clear;
* `xStreamBufferNextMessageLengthBytes` answers 0 on a stream buffer
  whatever it holds, and cannot distinguish that from "no message";
* `xStreamBufferReset` keeps the trigger level and refuses while a task
  waits.

`vTimerSetReloadMode` is pinned as the odd one out: it is the only timer
mutator that is NOT a queued command, so it takes effect at once and
reschedules nothing.

The three share `system.rs`'s config, port and sink, which is now
`pub(crate)` — three copies of a harness is three places for it to drift.
Release builds are unchanged by construction: the module is `#[cfg(test)]`.

*The measurement that opened it is kept below.*

## H3 — the counts as they stood

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

## H4 — The mutant survey covers one file of nine — SEVEN OF NINE NOW MEASURED, 2026-09-20

**Seven of the nine are measured**, and the two that are not are named
below with what blocks them. 42 tests were written against what survived.

| file | mutants | caught | missed | viable killed |
|---|---:|---:|---:|---:|
| `list.rs` + `arena.rs` (core) | 140 | 94 | 12 | **88.7%** |
| `name.rs` | 9 | 6 | 3 | 66% |
| `events.rs` | 120 | 67 | 38 | 64% |
| `timer.rs` | 192 | 78 | 51 | 60% |
| `stream.rs` | 163 | 84 | 65 | 56% |
| `typed.rs` | 39 | 9 | 8 | 52% |
| `system.rs` | **0** | — | — | *nothing to mutate* |
| `queue.rs` (corpus) | 255 | 125 | 55 | **69.4%** |
| `kernel.rs` (corpus) | 454 | 313 | 102 | **75.4%** |
| `kernel.rs` (**both oracles**) | 415 viable | 384 | **31** | **92.5%** |

`list.rs` and `arena.rs` are **closed**: every one of their twelve
survivors is either in code this target does not compile or an equivalent
mutant. Nothing reachable and non-equivalent is left alive in them.

### `system.rs` has nothing to mutate, which is not the same as 0%

Of its 58 `fn` lines, 57 are inside `#[cfg(test)]` and the one before them
is `pub fn build` inside the `macro_rules! system!` body. cargo-mutants
mutates neither test code nor macro bodies, so it generates **zero**
mutants. That line item closes on a technicality, and the technicality is
worth stating: a table that showed `0%` here would mean the opposite.

### What the survivors taught, three times over

**Blocking a task and never resuming it tests the blocking, not the
waking.** `finish_wait` had twelve survivors and `finish_sync` sixteen,
because every test stopped at the wake-up: a waiter was blocked, a set
woke it, and nothing ever ran it again — so the code that decides what a
resumed call ANSWERS never executed. A full round trip, and then a real
two-task rendezvous, took `events.rs` from 41% to 64%.

**A guard nothing approaches is a guard nothing is testing.** `is_sorted`
and `insert_inner` carry the same `if guard > N`. The pair `>`/`>=`
disagree only at exactly `N`, and `is_sorted` walks every item so a full
list drives its guard there — which killed its pair. `insert_inner` stops
one node short of the marker, so its guard reaches at most `N - 1` and
**nothing constructible tells the two apart**. Same expression, reachable
in one function and equivalent in the other.

**21% of the first survivors were a surface nobody had touched**: the
`_from_isr` and pended calls. Nine tests for them found that
`xEventGroupSetBitsFromISR` sets no bits at all — its whole body is
`xTimerPendFunctionCallFromISR`.

### Two instrument defects, both of which produce plausible numbers

**cfg-disabled code is counted MISSED, not unviable.** `end_index` and one
of the two `is_marker_of` bodies are `#[cfg(target_pointer_width = "32")]`.
cargo-mutants mutates the source anyway, the build succeeds because the
function is not compiled, the tests pass, and it is scored as a survivor.
Six of core's twelve are this. Killing them needs an i686 run, which
`bench/sweep.sh` already targets for list-ir.

**★ Concurrent `--in-place` runs across path-patched siblings corrupt each
other.** `rusty_rtos_core` is patched into both the kernel and the port, so
mutating core while those run means they build against a mutated core and a
mutant is scored CAUGHT for the wrong reason. Measured, on the same
population: a contaminated kernel run reported 209 caught / 119 missed /
**152 unviable**, and the clean runs either side of it both reported
**92 unviable** — the extra 60 were compile failures caused by the mutated
core, which shrank the viable denominator and overstated the kill rate by
about ten points.

cargo-mutants catches the worst case itself: starting a run while a
dependency was mutated gave *"cargo test failed in an unmutated tree, so no
mutants were tested"*, and it refused to produce numbers. **The rule:
never mutate two repos that are path-patched into one another at the same
time.**

### `kernel.rs` needs BOTH oracles, and the number only means anything together

Neither oracle is sufficient for this file, and the proof is that they
disagree about 235 mutants:

| | mutants |
|---|---:|
| survived the CORPUS only (the unit suite catches them) | 71 |
| survived the UNIT SUITE only (the corpus catches them) | 179 |
| **survived BOTH — the real gap** | **31** |

Together they kill **384 of 415 viable, 92.5%**, where the corpus alone
manages 75.4% and the unit suite alone about a third. The corpus runs
`PosixDemoConfig`, which leaves `USE_TICKLESS_IDLE` at its default of
false, so it **cannot execute the tickless path at all** — that was 23 of
its survivors. The unit suite has `TicklessConfig` and `SleepyPort` and
reaches exactly there, and almost nothing else.

**A mutant that survives one oracle is not a gap. One that survives both
is.** Quoting either number alone overstates the hole by 60 or understates
the coverage by fifteen points.

Twenty-six tests took the real gap from 80 to 31, and the tickless cluster is
CLOSED: `expected_idle_time`'s five survivors are all dead, and so are
`notify_value`'s. What remains is 31, led by `set_priority` (4),
`delay_until` (4), and a priority-inheritance cluster of six —
`priority_disinherit_after_timeout`, `priority_inherit` and
`wait_inherited`. That last one is the interesting lead: `recmutex` and
`GenQTest` hammer inheritance in the corpus, so mutants surviving there
point at the disinherit-after-TIMEOUT path specifically, which no scenario
reaches.

### Three mutants HANG the kernel, and that is a detection

`increment_tick` (`&` to `|`), `unwind_pended_ticks` (`>` to `>=`) and
`tick_count` (to `0`) all stop time rather than corrupting it. Each runs
to the harness timeout, and — on Windows — the killed test leaves the
build directory held, so cargo-mutants' next write fails with *"The device
is not ready. (os error 21)"*. That is what the fault was; it was not a
failing disk, and the tell was that repeated runs stopped at identical
counts.

They are excluded by name from the runs above and counted as DETECTED: a
mutant that stops an RTOS ticking is caught by any observer.

Long runs on this box also fault intermittently for unrelated reasons.
**Shard them** — four short runs completed where three full ones did not,
and a fault then costs one shard rather than the measurement.

### `queue.rs` was blocked on the corpus bridge, and the bridge was broken

`queue.rs` is the largest file in the kernel and carries the whole IPC
surface. Its oracle has to be the corpus, because the kernel's own
`cargo test` catches 13 of 342 in `kernel.rs` by design — and the recorded
recipe for that does not work today:

* the demo declares `rusty_rtos_kernel = { version = "0.1.0" }`, a
  **registry** dependency, patched to the local checkout only by its own
  `.cargo/config.toml`;
* cargo reads config from the **invocation** directory, which for that
  recipe is the kernel repo, whose table is written by `kairos patches` and
  lists only siblings that repo depends on;
* v0.1.0 **is published**, so the registry resolution succeeds silently.

Checked rather than reasoned: `cargo tree` from the demo resolves the
kernel to a PATH, and the same query from the kernel repo resolves it to
the registry. Run as recorded today, the corpus would test the PUBLISHED
kernel and every mutant would survive.

Whether the 2026-09-09 run was affected cannot be settled from here —
v0.1.0 was not published until a week after it — so that number stands and
the METHOD is what changed. `tools/mutants-corpus.sh` now proves the bridge
before measuring: it plants a poison that cannot fail to fire, requires the
gate to FAIL, removes it, requires the gate to PASS, and aborts on either
surprise.

*The measurement that opened this hole is kept below.*

## H4 — the survey as it stood

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

---

## H8 — `mps2-an385-qemu-kernel` does not compile in this checkout

**Measured 2026-09-20**, while wiring the port's QEMU cells up as a
mutation oracle. Three of the four Cortex-M cells pass; this one fails to
build with 30 errors, all of the shape

```
the trait bound `CortexMPort: Port` is not satisfied
```

which is not a code error. `cargo tree` shows the cell resolving **two**
`rusty_rtos_core v0.1.0`:

| source | reached via |
|---|---|
| `F:\...\rusty_rtos_core\crates\rusty_rtos_core` | `rusty_rtos_port-cortex-m`, by **path** |
| `https://github.com/Remade-With-Rust/rusty_rtos_core#bb56d5c8` | `rusty_rtos_kernel-core`, by **git** |

So `CortexMPort` implements one `Port` and the kernel expects the other —
the same trait name from two different crates.

It is the **only** cell that names its siblings by git URL; the other
three use paths. `[patch.crates-io]` cannot override a git dependency, so
the umbrella's patch table does not unify them, and `kairos patches` would
not write such a row anyway: the port repository does not depend on the
kernel, only this excluded firmware cell does.

**Cost of the gap:** the cell that exercises the KERNEL on the cortex-m
port is the one that cannot run, so the port's mutation oracle is three
cells rather than four — PendSV, SysTick and tickless, but not the kernel
above them.

**Not fixed here**, because the choice is an owner's: either the cell
moves to path dependencies like its three siblings, or the repository
grows a `[patch."https://github.com/..."]` table, and the second changes
what a fresh clone resolves. `firmware/README.md` states the git-URL rule
that this cell alone follows, so the inconsistency is documented in two
directions at once.

### Evidence for the first option, from a different cell (2026-09-21)

`tools/vertical/firmware/mps2-an385-k7` was built on the same machine, the
same QEMU and the same `mps2-an385`, and it depends on **six** sibling
packages -- entirely by path. It compiles, links and runs, and the six
packages it carries all reach one shared `rusty_rtos_core`.

That does not decide H8, which is still an owner's call about what a fresh
clone of `rusty_rtos_port` should resolve. It does remove one uncertainty
from the decision: the path option is not merely the other choice, it is a
configuration that demonstrably works here, at six path dependencies rather
than two.

## H9 — sim contract v1 could not run an always-ready task that makes no kernel call — CLOSED 2026-09-20, by contract v2

**Measured 2026-09-20**, porting `StreamBufferDemo`.

Contract v1 has exactly two tick sources: the idle hook (one tick per idle
pass) and every 16th outermost critical-section exit. Both are
kernel-visible points. So under the contract **a task that spins in user
code takes no time at all**, and if such a task is always ready the whole
run freezes — permanently, not slowly.

`prvNonBlockingReceiverTask` is one. It polls at `tskIDLE_PRIORITY` with
`sbDONT_BLOCK`, and `xStreamBufferReceive`'s zero-wait path takes no
critical section when the buffer is empty (`stream_buffer.c:1143`,
`xBytesAvailable = prvBytesInBuffer( ... )` with no `taskENTER_CRITICAL`).
`corpus StreamBufferDemo 200`, counters read under gdb at three times:

| t | counters |
|---|---|
| 8 s | `ticks=1 yields=4 exits=16` |
| 23 s | `ticks=1 yields=4 exits=16` |
| 38 s | `ticks=1 yields=4 exits=16` |

The deadlock closes on itself: the spinner cannot yield, because rotating a
priority level needs a tick; the idle task never runs, because the spinner
is always ready; and `prvNonBlockingSenderTask` — the one thing that would
put bytes in the buffer and so make the receive path take a critical
section — is at the same priority and never runs either.

Upstream states the dependency: *"The non blocking tasks run continuously
and will interleave with each other"*. That interleaving is preemptive
time-slicing off a hardware tick, which the Posix demo gets from a SIGALRM
thread and Kairos deliberately does not have.

**The symptom is misleading and cost the first hour.** `kairos_trace.c`
gives stderr a 1 MiB buffer flushed only on exit, so a hang reports as
`0 lines; KAIROS_RESULT ? missing` — indistinguishable, at a glance, from a
scenario that runs and emits nothing. A control run of a known-good
scenario through the same command separates the two in one step.

**Worked around, not closed.** `kairos oracle patch` now carries
`patch_stream_buffer`, which drops the two polling tasks and the clause in
`xAreStreamBufferTasksStillRunning` that they fed — the clause has to go
with them, or the check fails for ever on a counter nothing increments.
With the pair gone the same binary runs clean at the gate's budget:

```
ticks=2000 yields=3023 exits=21953 lines=20736   scenario pass
20737 lines, byte-identical across two runs
```

`prvSingleTaskTests`, both echo server/client pairs and the interrupt
trigger-level test all run and self-verify.

**What the workaround costs, measured before choosing it:** the pair is the
demo's only coverage of interleaved non-blocking send/receive, and covers
**none** of the six APIs this scenario exists to close for H2 —
`xStreamBufferReset`, `IsEmpty`, `IsFull`, `BytesAvailable`,
`SpacesAvailable` and `ReceiveFromISR` are all called from
`prvSingleTaskTests` (lines 256–572) and from nowhere in the pair.

**Not fixed here, because widening the contract is an owner's call.** A
tick source that reaches ordinary code — say a tick every Nth kernel API
entry, which the spinner *does* reach — would re-pin all 20 scenarios and
need the identical rule implemented in the Rust sim. That is a contract
v2, not a porting step.

### CLOSED by sim contract v2

**The rule.** Time passes at kernel-visible points. v1 had two: the idle
hook, and every 16th outermost critical-section exit. v2 adds the third
that was missing -- **the return of a kernel call that took neither** -- and
prices it at exactly one empty critical section.

That last part is what makes it cheap and safe. It is not a second clock:
the count, the every-16th rule, the running-task test and the switch are
all the paths v1 already proved, reached through `vPortEnterCritical();
vPortExitCritical();`. `exits` keeps its meaning, widened from "outermost
critical-section exits" to "kernel-visible points".

**Why it is this narrow.** Every other object closes the hole by accident:
`uxQueueMessagesWaiting`, `eTaskGetState` and `uxTaskPriorityGet` all take a
section. Only the stream buffer's zero-wait and query paths can return
blind, so only `xStreamBufferSend` and `xStreamBufferReceive` carry the
bracket -- four of FreeRTOS's own `traceENTER_`/`traceRETURN_` hooks on the C
side, and one wrapper each in `Kernel` on the Rust side.

**What it cost, measured.** Two scenarios re-pinned and no others:

| scenario | under v1 | under v2 |
|---|---|---|
| `StreamBufferDemo` | would not run at all | `exits=28002 lines=20927` |
| `MessageBufferAMP` | `exits=2072 lines=2430` | `exits=2090 lines=2445` |
| the other twenty | unchanged | unchanged, byte for byte |

`StreamBufferInterrupt` is in the unchanged column, which is the useful
check: its reader always blocks, so it never makes a blind call.

**What it bought.** `kairos oracle patch` no longer edits
`StreamBufferDemo.c` -- the workaround that dropped
`prvNonBlockingSenderTask` and `prvNonBlockingReceiverTask` is gone, both
tasks are ported, and the demo's own check passes with all three of its
clauses including `ulNonBlockingRxCounter`. The scenario is the upstream one
again.

| gate | result |
|---|---|
| `conform StreamBufferDemo` | 20,928 lines identical, first try |
| `conform StreamBufferDemo --ticks 100000` | **1,155,782 lines identical** |
| `conform --all` | 22 scenarios identical |

**What is still out of reach, and it is no longer a hole in the corpus.**
`xTaskGetTickCount` is also blind on this port (`portTICK_TYPE_IS_ATOMIC`
is 1), so a task that busy-polled it alone would still stop the clock.
Nothing in the corpus does, and the bracket is additive -- adding it there
is two more hooks and a re-pin of whichever scenarios call it. It is named
here so that the next demo that needs it finds the answer rather than the
symptom.
