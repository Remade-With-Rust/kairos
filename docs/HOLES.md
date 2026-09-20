# Holes

What is **not** proved, and how that was established. Companion to
`docs/LEDGER.md`, which records what is.

Every row here was measured, not suspected. Where a method has a known
false-positive mode it is named, because a hole list that overstates is as
useless as one that misses — and this one already caught itself overstating
twice (see *Method*, below).

Ordered by how much a reader should care, not by how easy the fix is.

---

## H1 — Three of the four kernel instruments never block a task

**Measured.** Zero-timeout calls per bench, and how many ever assert
`Wait::Blocked`:

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

**Cost of the gap:** any change to the blocking IPC paths is currently
priced at zero. **Fix:** the `kdelay-ir` pattern — one call with a real
timeout against an endpoint nobody satisfies.

---

## H2 — 22 public APIs have no evidence from the C differential

**Measured.** Of 130 public kernel APIs, these are called by no conformance
scenario, no runner code, no other kernel code, and no unit test:

```
name_of                 task_at                 with_tick_hook
notify_value            notify_value_clear      queue_remove_from_set
queue_send_to_front_from_isr                    ready_cursor
ready_items             timer_change_period     timer_delete
timer_expiry_time       timer_period            timer_pend_function_call
stream_buffer_reset     stream_buffer_is_empty  stream_buffer_is_full
stream_buffer_bytes_available                   stream_buffer_spaces_available
stream_buffer_next_message_length               stream_buffer_receive_from_isr
stream_buffer_set_trigger_level
```

They are **not dead**: `rusty_rtos-capi` calls most of them, so a C program
linking the shim reaches them. `vTimerDelete`, `xQueueRemoveFromSet`,
`xStreamBufferReset`, `ulTaskNotifyValueClear` and the rest answer with Rust
behaviour that **has never been compared to FreeRTOS's**.

The gate's whole claim is "does the Rust kernel decide what the C kernel
decides". For these, nobody has asked.

**Cost of the gap:** a semantic difference here surfaces in somebody's
application, not in CI. **Fix:** ranked by blast radius — the timer
mutators (`timer_delete`, `timer_change_period`) first, since they alter a
list the tick walks.

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

## H5 — `AbortDelay` is the 20th scenario and it does not run

**Measured:** 1,945 of 2,549 lines identical, then a divergence the ledger
argues is a *contract* question rather than a kernel one — the C harness
keys a queue's trace ordinal on its malloc address, so a differently-sized
successor to a freed object gets a new ordinal where our arena reuses the
freed index. A running count fixes `AbortDelay` and breaks
`EventGroupsDemo`.

Whatever the right answer, the effect today is that `xTaskAbortDelay` and
the three APIs that scenario exercises are outside the gate.

---

## H6 — `tail_value`'s invariant is a caller's promise with no check

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
