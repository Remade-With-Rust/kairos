# Flash by primitive — the campaign list

Where the K3 flash row's excess actually lives, from scorecard §18. Every number is
`llvm-nm` byte sizes from the pinned `bench/kernel-flash` link, C's functions paired
against the Kairos functions that do their job (our A4 splits put one C function's
work across a wrapper, an outlined handle and a blocking path, so this must be
grouped by PRIMITIVE, not by symbol).

Baseline at the start of this campaign: **flash 19,302 B, 1.386× C's 13,924.**
Now: **19,490 B, 1.400×** — wins 25 (−316), 26 (−4), 27 (−84), 28 (−230) and 29 (−8), and then
**+830 B of INSTRUMENT CORRECTION**: the two stream-buffer probe ops passed a constant
`0` tick, folding our blocking path out of the measurement. Every earlier number in this
file was taken on that folded arm. See the ledger for 2026-09-25.

## The list, in the order we are taking it

| # | primitive | C | Kairos | excess | ratio | status |
|---|---|---:|---:|---:|---:|---|
| 1 | **queue SEND** | 716 | 2,176 | +1,460 | **3.04×** | **IN PROGRESS** |
| 2 | queue TAKE (recv/peek/sem) | 1,512 | 3,236 | +1,724 | 2.14× | todo |
| 3 | queue CREATE/RESET/UNLOCK | 776 | 1,752 | +976 | 2.26× | **✅ −316 B (win 25)** |
| 4 | event groups | 778 | 1,810 | +1,032 | 2.33× | **✅ −84 B (win 27)** |
| 5 | stream buffers | 1,474 | **2,678** | **+1,204** | **1.82×** | △ −230 B taken, then **+770 B of instrument correction** — the honest ratio is 1.82×, not 1.294× |
| 6 | TICK | 308 | 584 | +276 | 1.90× | **△ −4 B (win 26)** |
| 7 | event lists / delayed | 968 | 1,224 | +256 | 1.26× | todo |
| 8 | LISTS | 126 | 374 | +248 | 2.97× | todo |
| 9 | scheduler | 726 | 892 | +166 | 1.23× | todo |
| 10 | priority inheritance | 514 | 574 | +60 | 1.12× | todo |
| | **the ten** | **7,898** | **14,948** | **+7,050** | 1.89× | |

Not on the list because they are at or better than parity, and must not regress:
task lifecycle 2,696 → 2,646 (**0.98×**), notifications 1,064 → 992 (**0.93×**),
timers 1,092 → 574 (**0.53×**), heap 646 → **0** (static arenas, no allocator).

## Attributing bytes to a module — use the tool, not a grep

`tools/flash-by-module.py <src.rs> <linked.elf> [shim_prefix ...]` sums the linked
`.text` of every symbol the file defines, matching the file's own `fn` names.

**Do not use `llvm-nm --demangle | grep -i <module>`.** Rust mangles a method by the
path of the TYPE its `impl` block is on, not the file it lives in, so every
`impl Kernel<..>` helper in `stream.rs` demangles as `kernel::Kernel<..>::write_message`
with no "stream" in it. That filter hid 3 of 7 stream.rs symbols (472 B) on 2026-09-25
and reported stream buffers as a win against C when they were at 1.33x.

Checked with the tool: the **events** (1,726) and **queue** (6,876) figures below are
sound; `stream.rs` was the only row the grep broke. Re-derive any row you intend to act
on. The tell that this is happening: **a group total that falls by more than the whole
binary does.**

## The gate every change passes

`bench/kernel-flash` (7 pins: byte total + five opcodes), `kairos conform --all`
(26 scenarios, line-identical to the C kernel), core + kernel + port test suites,
`bench/tick-work` rv32 rows, `bench/kernel-ram`. **Flash AND total instructions both
arbitrate** — a change that moves one opcode down while raising the byte total is a
displacement, not a win, and this campaign has already declined seven of those.

## Standing constraints

- `forbid(unsafe)` in the kernel crates. Several levers die here and the ledger
  records which.
- The `exits` counter is on EVERY trace line, so **critical-section structure is
  pinned by the differential** and cannot be restructured.
- rv32 work rows are a separate K-gate: `block_cycle` 983, `queue_roundtrip` 122,
  `send_full` 36, `peek_ok` 41 and the rest. A flash win that moves these is a
  trade, not a win, and needs saying out loud.

## 1 — queue SEND (+1,460, 3.04×) — IN PROGRESS

```
C:     xQueueGenericSend 406 + prvCopyDataToQueue 138 + xQueueGenericSendFromISR 172 = 716
ours:  kairos_queue_send 638 + send_generic_outlined 788 + queue_send_blocking 398
       + kairos_queue_send_from_isr 300 + kairos_queue_overwrite 26
       + kairos_semaphore_give 26                                            = 2,176
```

**~638 B of it is the A4 duplicate**: `queue_send_generic` is `#[inline(always)]`
so the hot wrapper gets its own copy, and `send_generic_outlined` is the
out-of-line handle the four colder wrappers call. Dropping the hint recovers about
1,666 B and costs `send_full` +40, `queue_roundtrip` +66, `block_cycle` +50 — a
K3-flash-versus-K3-work trade, already measured, owner's call. **Everything below is
what can be won WITHOUT taking that trade.**

### Findings

**Where the 788-byte body goes, against C's 406.** Instruction mix of
`send_generic_outlined` (277 instr) versus `xQueueGenericSend` (145):

| class | ours | C | excess |
|---|---:|---:|---:|
| MOV/CONST | 62 | 24 | **+38** |
| LOAD | 60 | 27 | **+33** |
| ADD | 32 | 11 | +21 |
| BRANCH | 41 | 28 | +13 |
| STORE | 28 | 15 | +13 |
| LOGIC | 12 | 1 | +11 |
| SHIFT | 9 | 0 | +9 |
| CMP | 9 | 0 | +9 |
| **CALL** | **8** | **13** | **−5** |
| other (C's inline `csr`) | 0 | 16 | −16 |

**We make FEWER calls than C and more of everything else.** C delegates to
`prvCopyDataToQueue`, `prvUnlockQueue` (x3) and `xTaskResumeAll` (x3); we inline
that work. And the prologues say the same thing:

| | frame | callee-saved |
|---|---:|---:|
| ours | **80 B** | **13** (ra, s0–s11) |
| C | 48 B | 7 (ra, s0–s5) |

Six extra callee-saved registers hold the 48-byte `Queue` snapshot, which exists so
the send does not resolve the handle twice.

### What was tried

| probe | result |
|---|---|
| **Drop `#[inline(always)]` on `queue_send_generic`** (the A4 duplicate) | **−598 B** flash, `mv` −31, `andi` −7, −221 instr — against **+118 rv32 instructions** (`queue_roundtrip` 122→169, `block_cycle` 983→1022, `send_full` 36→62). **5.07 B per instruction**, a worse rate than the `queue_peek` trade declined at 7.3 and the 10.7 this hint was originally kept at. DECLINED, re-priced at the site (the old comment said 1,666 B, measured when it was inlined at five places rather than one). |
| A4-split `copy_data_to_queue`, 3 cold callers out of line | **+72 B**. The cold callers pass CONSTANT arguments, so each inlined copy specialises below C's 138 B while a call costs a 5-argument frame and forces `&snapshot` to memory. |
| the same for `from_isr` alone (bisected) | **+6 B**. No caller wins individually either. |
| narrow the snapshot / let `copy_data_to_queue` re-resolve | not taken — the site records the measurement: re-resolving *"made every send pay for the same four checks twice."* |

### Verdict on SEND

The +1,460 B decomposes as **~638 B of priced A4 duplicate** (declined on the
project's own flash-versus-work rate) plus **~822 B of inline-where-C-calls**, each
piece of which has now been measured rather than assumed. **No untraded win was
available.** The largest item is available whenever flash outranks the work rows, and
its price is now current rather than stale.

### Bonus: the census defect this turn uncovered

`send_generic_outlined` appeared to have ONE caller, which contradicted the whole
reason an out-of-line handle exists. It has **18** — 17 by `j` (tail call) and 1 by
`jal`. Every caller census in this campaign counted `jal`/`jalr` only and so missed
**266 tail calls**. The A3 wins (19–24) stand because they were measured, but their
stated mechanism is withdrawn; see the ledger.

## 2 — queue TAKE (+1,724, 2.14×) — and the decision it forces

```
C:    xQueueSemaphoreTake 484 + xQueueReceive 398 + xQueuePeek 384
      + xQueueReceiveFromISR 166 + vQueueWaitForMessageRestricted 80   = 1,512
ours: queue_take_blocking 734 + take_outlined 668 + kairos_queue_receive 554
      + kairos_queue_peek 468 + queue_take_timed_out 368
      + kairos_queue_receive_from_isr 216 + copy_data_from_queue 192
      + kairos_semaphore_take 36                                       = 3,236
```

### Grouped, the gap is not where it looks

| | entries | shared machinery | from_isr |
|---|---:|---:|---:|
| ours | receive 554 + peek 468 + **semaphore 36** = **1,058** | take_outlined 668 + blocking 734 + timed_out 368 + copy_from 192 = **1,962** | 216 |
| C | receive 398 + peek 384 + **semaphore 484** = 1,266 | 80 | 166 |

**We are 208 B BETTER than C on the entry points**, because C reimplements
`xQueueSemaphoreTake` as a third full copy (484 B) where ours is a 36-byte thunk onto
the shared body. The whole TAKE gap is our **1,962 bytes of shared machinery against
C's 80** — C's blocking loop is inline inside each of its three entries; ours is
factored into `take_outlined` + `queue_take_blocking` + `queue_take_timed_out`.

So for TAKE we pay BOTH: the inline copies in the entries AND the outlined machinery.

### ★ The two queue A4 hints, priced together — this MEETS the K3 target

| | flash | ratio | rv32 work rows |
|---|---:|---:|---|
| pinned (both hints intact) | 19,302 | 1.386× | baseline |
| drop `queue_send_generic` only | 18,704 | 1.343× | +118 instr (5.07 B/instr) |
| drop `queue_take` only | 18,406 | 1.322× | +212 instr (4.23 B/instr) |
| **drop BOTH** | **17,800** | **1.278× ✅** | **+299 instr (5.02 B/instr)** |

Combined row cost: `block_cycle` 983 → 1083, `queue_roundtrip` 122 → 204,
`peek_ok` 41 → 98, `recv_empty` 38 → 73, `send_full` 36 → 62, `group_roundtrip`
71 → 72, `notify_roundtrip` 59 → 57. Every opcode improves: `mv` 763 → 680,
`andi` 136 → 122, `slli` 259 → 238, instructions 6,892 → 6,350.

**This is the decision the campaign has been walking towards. Two `#[inline(always)]`
attributes, one line each, are worth 1,502 bytes — and taking them puts the K3 flash
row UNDER its ≤1.30× target for the first time.**

What the price is, and is not:

- `bench/tick-work` **PASSES either way** — it gates work PARITY with the C arm plus
  the poison check, not absolute row values, and the ANCHOR is identical.
- The six rows are Kairos-only absolutes with **no C comparison**, unlike the flash
  row, which has one and is failing it.
- But they were deliberately optimised and this ledger records wins on `block_cycle`
  and `queue_roundtrip` specifically. Undoing that is a real cost, not a free one.

**LEFT AT THE PINNED STATE**, because trading away a passing gate to fix a failing
one is the owner's call — the same line this campaign held for `codegen-units`,
`queue_peek` and the `Tcb` stride pad. Both reversals are one line in `queue.rs`:

```rust
    #[inline(always)] pub fn queue_send_generic(   // remove -> -598 B, +118 instr
    #[inline(always)] fn queue_take(               // remove -> -896 B, +212 instr
```

**NOT YET CONFORMED.** The 26-scenario differential for the both-dropped state was
started and its build died on a full F: drive (rustc exit 101), so that state is
measured on flash but unvalidated on correctness. Inlining should not move a trace
line, but it has to be shown before the trade is taken.

### ⚠ Correction to §18's grouping (2026-09-25)

`notify_locked` (272 B) was grouped into CREATE/RESET/UNLOCK because its name
contains "lock". It is not queue-lock code at all — it is the task NOTIFICATION
implementation in `kernel.rs`, called by `notify`, `notify_from_isr` and their
`indexed` variants. Moving it to the bucket it belongs in:

| primitive | C | ours | excess | ratio |
|---|---:|---:|---:|---:|
| queue CREATE/RESET/UNLOCK | 776 | **1,752** | **+976** | **2.26×** (was +1,248, 2.61×) |
| notifications | 1,064 | **1,264** | **+200** | **1.19×** (was −72, 0.93×) |

**So notifications is NOT one of the primitives at parity — it is 1.19× behind.**
Totals are unaffected; the 272 B only moved buckets. The four-at-parity claim is now
three: task lifecycle 0.98×, timers 0.53×, heap 0.00×.

> **The law: group a primitive by what the code DOES, never by what its name
> contains.** A substring match put notification code in the queue-lock bucket and
> turned a 1.19× primitive into a reported win.

### Queue sets are correctly gated — checked, not assumed

`configUSE_QUEUE_SETS 0` makes the C preprocessor delete its queue-set code
entirely: **zero queue-set symbols in the C arm.** Ours is gated on
`C::USE_QUEUE_SETS` at all three sites (`queue.rs` 811, 1084, 1726) and folds with
the const false, so there is no dead queue-set code to remove. The hypothesis that
we were carrying 272 B C did not have was wrong — that was `notify_locked`.

---

## 3 — queue CREATE/RESET/UNLOCK (+976, 2.26×) — ✅ WIN 25

Breakdown before the work, `llvm-nm` bytes:

| what | C | Kairos | ratio |
|---|---:|---:|---:|
| **mutex create** | 48 | **464** | **9.7×** ← the outlier |
| unlock | 208 | 596 | 2.87× |
| create + reset | 520 | 1,004 | 1.93× |

### ★ The win: a specialisation written to escape an inline that had a handle already

C's `xQueueCreateMutex` is **48 bytes** because `prvInitialiseMutex` does not
*contain* a send — it **calls** `xQueueGenericSend`. Ours contained one: a private
`prime_mutex` carrying a hand-written copy of the send, specialised on the four
constants its single call site supplied, with a 27-line doc block arguing for it.
`prime_mutex` emitted **zero symbols**, so it was fully inlined and its ~424 B *was*
`new_mutex`'s 464.

The doc block's own justification is where the refutation was sitting in plain
sight: it said inlining the general body here cost **534 bytes**, so a specialised
copy was cheaper. Both halves of that are true and the conclusion still does not
follow — **the choice was never "specialise or inline", it is *which symbol to
call*.** `send_generic_outlined` — the A4 out-of-line handle that already existed
for the cold callers — costs a five-argument frame, not a body.

Calling it and deleting the specialisation:

| pin | before | after | Δ |
|---|---:|---:|---:|
| **flash bytes** | 19,302 | **18,986** | **−316** |
| `mv` | 763 | 755 | −8 |
| `slli` | 259 | 254 | −5 |
| `andi` | 134 | 134 | −2 (from 136) |
| rv32 instructions | 6,892 | 6,793 | −99 |

Every pin down, none up — no displacement. **Ratio 1.386× → 1.360×.**

Gates: `conform --all` **26/26** (the one that matters — `prime_mutex` was crafted
to produce the same trace, so equality of the trace is exactly what had to be
re-proved), kernel **101/0**, core **63/0**, port **28/0**, rv32 **PASS**,
RAM **PASS** (11 checks).

Deleting the now-dead 52 lines measured **byte-identical** — LLVM had already
dropped the uncalled private fn, so the source shrank for free.

### ★★ The law, and it generalises past this kernel

> **A specialisation written to escape an unwanted INLINE is only worth its bytes
> if no out-of-line handle on the general body exists. Check for the handle
> first.**

An A4 split (`rusty-compiler-leverage` §A4) creates exactly such a handle. So the
failure mode is specific and recurrent: **an earlier A4 split hands you the cure,
and the next person to face the same inlining cost writes a specialisation instead
of calling it**, because the handle was introduced for different callers and does
not advertise itself. 316 bytes, and 52 lines of code carrying a wrong argument.

### The A4 re-test this forced — one caller was carrying the whole verdict

Deleting `prime_mutex` took `copy_data_to_queue` from four callers to three, so its
A4 verdict was now measured against a shape that no longer existed (§9 re-test
law). Re-tested at three callers: **+6 B**, versus **+72 B** at four. **~66 of the
original 72 bytes were `prime_mutex`'s own site.** Still a loss, so still reverted,
but it is now a near-tie held by 6 bytes rather than a comfortable refutation — and
that is the point of the law: *an A4 verdict can be carried almost entirely by ONE
caller.*

### Still open on this primitive

* `new_queue` 209 instructions vs C's create+reset 130. `account_for_allocation`'s
  `suspend_all`/`resume_all` is **not** a candidate — it mirrors `pvPortMalloc`'s
  `vTaskSuspendAll`/`xTaskResumeAll` so the `exits` count matches, and conformance
  pins it. (We take the 646 B of `pvPortMalloc`/`vPortFree` to zero in exchange.)
* `unlock_queue` 117 instructions vs `prvUnlockQueue`'s 70; excess LOGIC +8,
  SHIFT +6, LOAD +8 — the four handle resolves where C dereferences a pointer.
  Structural, and the same shape as every other resolve in the kernel.

---

## 6 — TICK (+276, 1.90×) — △ win 26 taken early, out of order

Not reached in list order. It came out of a sweep of the **compiler's own warnings**
while chasing something else, which is where both of that sweep's findings lived.

`switch_delayed_lists` — `taskSWITCH_DELAYED_LISTS()`, the tick-count overflow swap —
carried `#[inline(never)]` **and** a later `#[inline]` at the same time. `never` wins,
so the A3 change had been **dead since the day it was written**, and `#[inline]` alone
would have been declined at that size regardless: A3 needs `always`.

| pin | before | after | Δ |
|---|---:|---:|---:|
| flash bytes | 18,986 | **18,982** | **−4** |
| total rv32 instructions | 6,793 | **6,785** | **−8** |
| `mv` | 755 | 747 | −8 |
| `slli` | 254 | 256 | +2 |
| `andi` | 134 | 136 | +2 |

Both arbiters fall, so a win and not a displacement. `bench/tick-work` **PASS** with
its PARITY anchors identical — nothing hot paid for pulling the cold body in, which
was the `never` rationale's whole concern. Gates: flash 7/7, kernel 101/0, RAM PASS.

The sweep's second finding cost nothing but would have: `trace_failure_or_owe`'s doc
block **and** its `#[inline(always)]` had been spliced above `note_exits`, so that
function held a duplicate attribute and `trace_failure_or_owe` held none. Restoring
it is byte-identical today (LLVM inlines it at this size anyway) — and would have
stopped being byte-identical the moment the body grew, silently.

> **The law: a contradictory attribute pair is a silently reverted change, and
> `rustc` prints it on every build.** Neither defect could fail a pin, because
> neither moved a number. `cargo build 2>&1 | grep -c 'unused attribute'` is now a
> number worth watching: it was 2, it is 0.

Still open on TICK: the +276 proper, which this barely dents.

---

## 4 — event groups (+1,032, 2.33×) — ✅ WIN 27, −84 B

Symbol pairing first, which moved the target: `create` was the worst ratio, not the
biggest absolute gap.

| primitive | C | Kairos | ratio | excess |
|---|---:|---:|---:|---:|
| **create** | 38 | **168** | **4.4×** | +130 |
| clear_bits | 64 | 86 | 1.34× | +22 |
| set_bits | 162 | 590 | 3.64× | +428 |
| sync | 234 | 264 | 1.13× | +30 |
| wait_bits | 280 | 702 | 2.51× | +422 |

`events.rs` had **no inline attributes at all** — the only kernel module the A3/A4
work had never touched.

### The win: `vListInitialise` ported without its reason

`event_group_create` drained the group's waiting list, mirroring
`vListInitialise( &( pxEventBits->xTasksWaitingForBits ) )`. C needs it: its list is
a field of memory `pvPortMalloc` just returned, so it holds garbage. Ours is a slot
in a shared array indexed by the group's arena index, already emptied by
`event_group_delete` before the slot is discarded. **−84 B for a loop that cannot
iterate.** Full account and the poison protocol in the ledger.

### Refuted here: folding `finish_sync` into `finish_wait`

Two tails, one of them a 20-line copy of the other with both flags frozen. Deleting
the copy measured **+34 B** — at two callers LLVM emits a shared symbol instead of
inlining, and one shared body loses to two constant-folded ones. With
`#[inline(always)]` it is **byte-identical**, so the duplication came out for free
but it was never a flash win. Kept in that form.

### ⚠ OPEN, and it is a correctness question rather than a flash one

The same analysis says queues have the hazard event groups were defending against,
with **no defence on either side**:

* `Self::queue_receive_list(queue)` / the send list are indexed by the queue's arena
  index, exactly as a group's list is.
* `queue_delete` gives the storage back, discards the descriptor and calls
  `account_for_allocation` — **it does not touch the two waiting lists.**
* `new_queue` does not drain them either.

So a queue deleted while a task is blocked on it leaves live list entries at that
index, and the next queue to take the slot inherits them. **C does not have this
hazard even though `vQueueDelete` also ignores blocked waiters**, because its create
reinitialises a freshly-malloc'd list and the stale entries simply vanish with the
old allocation. Ours persist.

**Status: unverified.** It needs the queue equivalent of
`a_reused_group_slot_does_not_inherit_the_last_groups_waiters` — block a task on a
queue, delete the queue under it, create another, and look at the new queue's lists.
FreeRTOS documents deleting a queue with blocked waiters as not allowed, so this may
be out of contract; that is an argument for writing the test, not for skipping it.
Not chased inline because it is not a flash question, and `queue.rs` has no unit-test
module to put it in (the queue tests live outside the file).

---

## 5 — stream buffers (+664, 1.45×) — △ −184 B → 1.326×, NOT a win, and the floor is ~1.21×

**Goal for this stretch was to make stream buffers beat C. They do not.** Group
**2,138 → 1,936** against C's 1,474. Full account in the ledger; the decision-relevant
parts:

### ⚠ Two numbers in this file's earlier drafts were wrong

Group totals were taken with `llvm-nm --demangle | grep -i stream`, and **three of the
seven stream.rs symbols contain no "stream" in their demangled name** — the `impl` block
is on `Kernel`, so they mangle under `kernel::Kernel<…>`: `write_message` (212),
`write_bytes` (160), `blind_call` (40). The filter briefly reported **1,418 B / 0.96×,
"a win"**, and **528 instructions vs C's 573**. True figures: **1,954 / 1.326×** and
**686 vs 573**. Attribute by the file's function-name list, never by a symbol substring
— `tools/flash-by-module.py` now does this, so it cannot recur.

### What was won: −184 B, both arbiters down

Bytes −184, total rv32 instructions 6,785 → 6,699 (−86). Gates: flash 7/7, conform
26/26, kernel 102/0, rv32 PASS, RAM PASS.

The largest single item was **one word**: `read_bytes` asked for `&mut self` while only
ever READING `self.bytes`. That is why `read_length_prefix` (which has `&self`) carried
a complete second copy of the ring walk — its own comment said so. The write side had
already been fixed; the read side had not. **Sibling-parity defect, −56 B.**

Also −124 B for two byte-at-a-time fallback loops, each with a per-byte wrap check,
guarded by `len <= ring` — a guard that is always true on both sides. The stub is its
own poison (an unadvanced tail makes the caller re-read), and conform is 26/26.

### The floor, and the one place we DO beat C

| | bytes | what C does with it |
|---|---:|---|
| our byte arena, inlined into `create` | 136 | `pvPortMalloc`: **646 B in heap_4.c**, outside C's 174 |
| `blind_call` | 40 | `vPortKairosApiReturn`: in **port.c**, outside C's 1,474 |
| like-for-like remainder | **1,778** | **1,474** → **1.206×** |

Priced by outlining `take_bytes` (measured, then reverted): create is **140 B**, arena
**150 B**. On C's own accounting:

> **`stream_buffer_create` 140 B vs `xStreamBufferGenericCreate` 174 B — create beats C
> by 34 bytes, with no allocator at all against C's 646.**

The remaining ~300 B is §14's two structural costs: a handle resolve where C
dereferences a pointer, and stackless resume where C blocks inside the call.
`write_message` shows it plainly — 84 instructions vs C's 39, of which `mv` 16 and
`sw` 14 are argument marshalling and spills around two outlined calls, not work.

**To go under 1,474 would mean giving up handle resolution or stackless resume.** That
is the owner's call, not a byte to be found.
