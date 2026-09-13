# `switch-cost` — what a context switch costs, ours against theirs

```
RISC-V -- a COOPERATIVE yield, the whole port cost:
                                      save  restore  total
  FreeRTOS portASM.S (ecall trap)       42       41     83
  Kairos kairos_riscv_switch            14       16     30
                                              ratio  2.77x

  Under PREEMPTION, measured:
    Kairos kairos_riscv_switch_trap      37
    + riscv-rt default_start_trap        37
    = a preemptive switch                74   vs FreeRTOS 83   1.12x

ARM Cortex-M3 -- both arms are the PendSV handler, trap to trap:
  FreeRTOS xPortPendSVHandler            19
  Kairos PendSV                          19
```

**Read all three together.** A yield is 2.77× cheaper on our side; a preemptive
switch is near parity; ARM is exact parity. The first number alone is the easy
half.

Run it:

```sh
sh bench/switch-cost/run.sh        # needs clang and llvm-objdump
```

## Read these two rows together, or you will read the first one wrong

The RISC-V row alone says we move 2.8× less register traffic. The ARM row is
its **control**, and it says **parity** — 19 instructions each, exactly.

That is the real finding. On ARM, both kernels reach the switch the same way:
`portYIELD()` pends `PendSV` on their side and ours does the same, so the
shapes match and so do the counts. On RISC-V they differ because
**`portYIELD()` is `ecall`** (`portmacro.h:94`) — a yield there takes the same
trap an interrupt does, and must save everything an interrupt could have
clobbered: all 28 GPRs, `mstatus`, `mepc`, the critical-nesting count. Kairos
yields at a **call site**, where the C ABI has already declared the
caller-saved half dead, so its frame is `ra`, `sp` and `s0`–`s11`.

So the RISC-V gap is about **where a yield is taken**, not about one kernel
moving registers more cheaply than the other. With the ARM row beside it that
reads as a design difference; without it, it reads as a win we did not earn.

## The number is work, not time

**Retired instructions**, not cycles. The mission plan puts cycle rows on real
parts because QEMU cannot supply a trustworthy cycle count, and nothing here
changes that. This family prefers counters to clocks anyway.

The RISC-V counts are exact rather than sampled because **both paths are
straight line** — no loop, no data-dependent branch — so the count *in* the
block is the count retired *by* it. The script does not take that on trust: it
counts conditional branches and fails if one appears where it should not.

Neither ARM arm is straight line, and the script says so rather than pretending
otherwise. They spend their guards differently: ours has four `cbz` for the
null-slot cases, theirs four instructions of `basepri` masking. Same count,
different work.

**Counts do not compare across architectures.** One `stmdb` moves eight
registers where RISC-V needs eight `sw`. Only ours-against-theirs *within* an
architecture means anything, which is why there is no "ARM is 4× cheaper than
RISC-V" line here.

## What preemption costs us on RISC-V

```
    Kairos kairos_riscv_switch_trap      37
    + riscv-rt default_start_trap        37
    = a preemptive switch                74   vs FreeRTOS 83   1.12x
```

**Measured, not bounded** — as of 2026-09-11 the preemptive path exists and
works. `rusty_rtos_port/firmware/riscv32-qemu-preempt` runs 201 switches
between two tasks that never yield, zero faults, and this row is counted from
the firmware it builds. Straight line, no conditional branches, so again a
static count is a retired count.

It got there by fixing a real defect. `switch_context` resumes a task with
`ret`, which is right at a call site and wrong inside a trap — a task entered
that way keeps running in trap context, so the first preemptive switch was also
the last. The port now has a second switch, `switch_context_trap`, which swaps
`mepc` and `mstatus` as well, and a `new_task_context_preemptive` that builds a
fresh task to leave its first trap through `mret`. The two are kept apart so a
yield still pays only for what a yield needs, which is why the 30 above is
unchanged.

**Both rows are true and they say different things.** A *yield* is 2.77× cheaper
on our side, because FreeRTOS routes yields through the interrupt trap
(`portYIELD()` is `ecall`) and we do not. A *preemptive switch* is **near
parity**, because there both kernels must write down what an interrupt could
have clobbered — 37 against their equivalent, plus a trap entry each.

Quoting only the first would be quoting the easy half, and the earlier version
of this file did exactly that before the preemptive path existed to check it
against.

## How each arm is counted

Neither number is typed in by hand, and neither is read off a source listing.

| | counted from |
|---|---|
| FreeRTOS RISC-V | their `portContext.h` macros, assembled by clang for `rv32imac` |
| FreeRTOS ARM | their `ARM_CM3/port.c`, compiled by clang for `thumbv7m` |
| Kairos RISC-V | the linked `riscv32-qemu-switch` firmware, disassembled |
| Kairos ARM | the linked `mps2-an385-qemu-kernel` firmware, disassembled |

The RISC-V macros are expanded **alone in their own sections**
(`c/freertos_probe.S`) rather than carved out of one of their four handlers by
address range. A section boundary is chosen by the assembler; an address range
would be chosen by eye, and that is a thumb on the scale.

ARM handlers are counted up to their `bx lr`, because everything after it in
the block is the literal pool and alignment padding — data, not instructions.
Counting those would have read 21 where the answer is 19.

FPU and VPU are off in the C arms because the Kairos ports have no FPU save
either.

## Poison-proving

Assembling the *same* RISC-V probe with `-D__riscv_32e`, which drops
`x16`–`x31` from both macros, must remove exactly sixteen instructions from
each side:

| | save | restore |
|---|---:|---:|
| as shipped | 42 | 41 |
| `__riscv_32e` | **26** | **25** |

42 − 16 = 26 and 41 − 16 = 25. The probe tracks their macro to the
instruction, so it is reading their code rather than reporting a constant.

## Why the counts are pinned

`run.sh` fails if any of the six numbers moves. A register joining a `Context`,
or the oracle checkout changing under us, should be a failed gate and not a
quietly different README.

## What is missing

**Xtensa.** The oracle carries an `Xtensa_ESP32` port
(`portable/ThirdParty/GCC/Xtensa_ESP32/portasm.S`), so an oracle exists, but
clang has no Xtensa target on this box and the port's headers reach into
ESP-IDF. Counting it by eye is exactly what the rest of this bench refuses to
do, so it is left out rather than estimated.

Cycles, silicon, and anything about the ESP32-C6.
