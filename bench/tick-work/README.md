# `tick-work` — K3's tick and switch rows, against the C, on rv32

```
retired instructions per call, rv32imac, -O2 both arms, no LTO either
row                FreeRTOS     Kairos    ratio
tick_idle                15         56    3.73x
tick_delayed             15         56    3.73x
switch_select            27         79    2.93x

A SWITCH IS TWO HALVES, and this bench measures one of them.
  bench/switch-cost prices the register half from the same oracle:
  cooperative 30 (Kairos) against 83 (FreeRTOS), preemptive 74 against 83.
  whole cooperative switch: FreeRTOS 27+83 = 110, Kairos 79+30 = 109  (0.99x)
  whole preemptive switch:  FreeRTOS 27+83 = 110, Kairos 79+74 = 153  (1.39x)
```

Run it:

```sh
sh bench/tick-work/run.sh
```

Both arms are built and run by that one script, in one invocation, because a
paired measurement taken two different ways is not paired.

## What the two arms are

| | |
|---|---|
| **C** | FreeRTOS **V11.3.1** from the pinned `oracle/` checkout, **unmodified**, with the oracle's own first-party `portable/GCC/RISC-V` port, on QEMU `virt` |
| **Kairos** | `rusty_rtos_kernel/firmware/riscv32-qemu-tick-work`, same machine, same instrument |

The C arm is the kernel the corpus proves us byte-identical to — not a fork,
not a rewrite, not a stub. That is the whole reason to build it: a cycle row
against ESP-IDF's *fork* of FreeRTOS would compare Kairos to a kernel it has
never been proved equivalent to.

## Why retired instructions, and not cycles

QEMU is not a clock — measured six ways on the Cortex-M cell, where DWT is
unimplemented, SysTick's deltas are host wall time, and under `-icount` the
clock does not advance at all.

But `minstret` is an **architectural CSR**, not optional debug hardware, and
under `-icount shift=0` it is exactly reproducible: the sibling cell
`riscv32-qemu-switch` read **199,902 three times running** with it, and swung
20% without it.

So this is a **work** row on the same basis as `bench/switch-cost`, and it sits
beside those rows. **Cycles still come from a part** —
`rusty_rtos_kernel/firmware/xiao-s3-cycles` — and that has not changed.

## ★ Why a bracket, and not a profiler

A host callgrind comparison of these two kernels was built and **refuted** on
2026-09-21 (`docs/LEDGER.md`). FreeRTOS's `xTaskIncrementTick` self cost is
**2.7%** of its inclusive and ours is **13.1%**, so a self-cost ratio measures
how differently two codebases *factor* a tick, not what a tick costs.

**Work parity was perfect and did not save it.** Both arms called the tick
2,001 times and the switch 81 times on the same scenario, and the ratios looked
stable across four scenarios. The anchors prove both arms did the same WORK;
they say nothing about whether the boundary encloses the same THING.

Two CSR reads state the boundary explicitly. That is the one change that makes
these arms comparable, and it is why this bench exists.

## The two gates

**PARITY.** Both arms print an anchor line and the run fails unless they are
identical:

```
ANCHOR samples=512 tick_calls=1024 switch_calls=512 tick_count=1024
```

`tick_count` is read from the kernel afterwards, so a tick that took a
short-circuit path instead of the real one cannot pass. That matters here:
`xTaskIncrementTick` opens with `if( uxSchedulerSuspended == 0 )` and a
suspended call merely does `++xPendedTicks` — about the right size to look like
a plausible answer.

**POISON.** Every arm is built twice, with the measured call made once per
bracket and then twice, and each row must move by about one call:

```
                     C x1     C x2  Rust x1  Rust x2
tick_idle              15       30       56      112
tick_delayed           15       30       56      112
switch_select          27       52       79      155
```

A bracket that measures the loop rather than the callee does not move. Without
this the numbers are plausible and unproven.

## What is matched, deliberately

| | |
|---|---|
| config | 5 priorities, 16-byte names, 100 Hz tick, 10-deep timer queue — the **shared** `bench/kernel-ram/c/FreeRTOSConfig.h`, included rather than copied |
| optimisation | `-O2`, **no LTO**, both arms. The C arm compiles per translation unit, so fat LTO on the Rust side would let it inline across a boundary C cannot cross, and the row would measure the build |
| ready tasks | two at the measured priority on both sides — a switch with one ready task is not a switch |
| samples | 512, median, instrument tax measured and subtracted |

Three config deltas from the shared file are declared in
`c/FreeRTOSConfig.h`, each with its reason: no timer interrupt
(`configMTIME_BASE_ADDRESS 0`, so nothing but this program drives the tick), a
heap big enough for four real stacks, and the stack-overflow check turned
**off** — that check compares a 20-byte pattern on every switch and a stackless
kernel cannot have that cost, so leaving it on would charge C for work our
design makes impossible. A number in our favour is the kind that gets checked
first here.

## Reading the result honestly

Two of these three rows are **against us**, and they are findings, not caveats.

- **The tick is 3.73x.** Ours does the same steps in the same order — the code
  is written against the C and says so — and costs 41 more instructions.
- **`switch_select` is 2.93x**, but quoting it alone is quoting a third of the
  answer. A switch is selection *plus* the register file, and the two kernels
  put their weight in opposite halves: FreeRTOS routes yields through the
  `ecall` trap and saves 28 GPRs (83), where a Kairos yield happens at a call
  site whose caller-saved half the ABI has already declared dead (30). **Added
  up, a whole cooperative switch is 109 against 110 — parity.**
- **A whole preemptive switch is 1.39x against us** (153 against 110), because
  there both kernels must save what an interrupt could clobber and our
  selection half is the expensive one.

`tick_idle` and `tick_delayed` are identical on both arms, which is correct
rather than suspicious: a delayed task that is not yet due changes nothing but
a comparison against `xNextTaskUnblockTime`.

**The two halves of that 109 come from different instruments, and only one of
them could have carried the defect below.** This bench counts `minstret` at
runtime; `bench/switch-cost` counts instructions **statically**, out of
`llvm-objdump` on the linked firmware, and fails the run if either path is not
straight line — a static count is a retired count only when there is nothing
to skip. No kernel trace is reachable from a disassembly, so the 30 / 74 / 83
figures cannot be inflated by a shadowed `NoTrace`, and the parity claim does
not rest on the half that was wrong.

## ★ The defect this bench found

The first Kairos `switch_select` read **305**. The gap was large enough to
demand the boundary check before publishing, and the check found a **harness**
defect rather than a kernel one.

`rusty_rtos_core::trace` already ships a `NoTrace` with
`const WANTS_NAMES: bool = false`, which is what lets the kernel skip the task
name lookup and its UTF-8 validation. This cell had **hand-rolled its own
`NoTrace`** that shadowed it and inherited the trait default of `true`, so
every switch built a 16-byte name and handed it to a sink that dropped it.

Using the one the crate already provides: **305 to 79, a 3.86x** on this row,
with no kernel change at all.

**Eleven sites in the repo hand-roll the same twin** and ten of them still miss
the const — including `xiao-s3-cycles`, whose published silicon switch row was
inflated by it. See `docs/LEDGER.md`.
