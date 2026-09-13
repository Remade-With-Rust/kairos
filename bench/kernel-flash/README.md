# `kernel-flash` — what a kernel costs in flash, ours against theirs

```
the same operations, dead-code-eliminated from the same root set:
                                       .text
  FreeRTOS kernel + RISC-V port        13924
  Kairos kernel + RISC-V port          16008
    (Kairos compiler_builtins)           492   excluded, see below
                                       1.15x

  Kairos costs 2026 bytes more flash for the same operation set.
```

Run it:

```sh
sh bench/kernel-flash/run.sh        # needs clang, ld.lld, llvm-nm, llvm-size, cargo
```

## The headline, stated plainly

**Kairos is 1.15× the C kernel's flash** for the operation set the conformance
corpus proves byte-identical — 2,026 bytes more, on rv32, at `-Os`, both
dead-code-eliminated by the same linker.

That is a loss, and a modest one. It is worth being precise about, because
three earlier numbers from this same measurement were wrong — two against us,
one in our favour — and all three were instrument faults rather than findings.

## The root set is the corpus, and that is the whole argument

`docs/API-MAP.md` lists 341 FreeRTOS entry points, nearly all still `planned`
on our side. Linking the C kernel against all of them and Kairos against the
forty it implements would price a **feature gap**, not a kernel.

The operations rooted here are the ones the corpus exercises — the one place
the two kernels are known to do the same work, eighteen scenarios
byte-identical on four architectures. It is the only root set this
comparison can honestly use. Both arms name them with `-u`, so the roots are
the kernels' own symbols and neither arm is charged for a harness.

## Where the C arm's bytes are

| module | bytes |
|---|---:|
| `tasks.c` | 6,408 |
| `queue.c` | 3,032 |
| `stream_buffer.c` | 1,474 |
| `timers.c` | 1,122 |
| `event_groups.c` | 778 |
| `heap_4.c` | 646 |
| `portASM.S` | 194 |
| `port.c` | 144 |
| `list.c` | 126 |

`heap_4.c` is included because FreeRTOS genuinely needs a heap to create a
task, where Kairos allocates from arenas inside the kernel. Excluding it
would charge Kairos for a facility and let the C arm have it free.

## Three instrument faults, all caught, all worth keeping

**1. The single-call-site blob — 31,524 bytes, against us.** The first Rust
probe called every operation from one function. The optimiser inlined most
of the kernel into it, the map attributed **13,314 bytes to "the probe"**,
and the total came out at 2.3× the C arm. One call site is not how a kernel
is used; the inlining was an artefact of the harness. Giving each operation
its own `extern "C"` entry point — which is what the C arm has — took the
Rust arm from 31,524 to **16,556**. A number roughly twice what it should be
is the instrument asking for help, in whichever direction it leans.

**2. The 596-byte root table — in our favour.** The C arm was first pinned
with a table of function addresses, and that table's own code counted as
kernel. Both arms now root with `-u` instead.

**3. The port's own switch, rooted on one side only — against us, 150 bytes.**
The Rust root set was globbed as `^kairos_`, which also caught the port's
assembly symbols (`kairos_riscv_switch`, and later its preemptive twin), while
the C arm's roots pull in neither of *its* context switches. It stayed
invisible until adding the preemptive switch moved the total, and the fix took
the pin **down** from 16,064 to 15,950. The switch is measured exactly, and on
both sides, by `bench/switch-cost`; counting it here too would count it twice
and only against one arm.

## The tick-width asymmetry, which is NOT a language cost

Kairos uses a `u64` tick. The FreeRTOS RISC-V port does
`typedef portUBASE_TYPE TickType_t` (`portmacro.h:68`), so on rv32 the tick
is 32-bit and **`configTICK_TYPE_WIDTH_IN_BITS` is not honoured by this port
at all**.

Setting it to 64 was tried, because matching the configuration is what the
rest of these benches do. The type stayed `unsigned int` and clang reported
`eventUNBLOCKED_DUE_TO_BIT_SET` truncating to 0 — a broken build, not a
matched one. So the asymmetry is **reported rather than removed**: part of
the 2,026-byte gap is a tick that never wraps, including the 492 bytes of
`compiler_builtins` 64-bit helpers already excluded from the total.

`compiler_builtins` is excluded because the C arm's `memcpy` and 64-bit
helpers are unresolved and uncounted too. Counting ours and not theirs would
not be symmetric.

## Instrument checks

A linker that discards nothing measures nothing, so the script links each
arm **twice** — with and without `--gc-sections` — and fails if the two agree:

```
      ok    --gc-sections removed 4660 bytes from the C arm
      ok    --gc-sections removed 368726 bytes from the Rust arm
```

The Rust figure is large because a staticlib carries every codegen unit until
the linker prunes it; that it prunes 368 KB is the check passing, not a
finding.

Both totals are pinned, so either kernel changing size fails the run.

## What this does NOT claim

That this is a firmware image. It is the kernel and its port, with nothing
above and no C library beneath. It also says nothing about Cortex-M or
Xtensa: the ports differ, and `port.c` + `portASM.S` here are the RISC-V
ones.

2026-09-11: 15,950 -> 16,008, **+58 bytes**, and both causes are K6
findings the C demos produced:

  * `Kernel::suspend` now clears `taskWAITING_NOTIFICATION` on every
    notification slot, which `tasks.c` does and we did not. Without it a
    suspended task reports as BLOCKED for ever, because `eTaskGetState`
    scans those slots before it calls a suspended-list task suspended.
    `TaskNotify.c:498` asserts exactly that inside a timer callback.
  * `Kernel::task_at`, which answers the live handle in an arena slot, so
    a crash report can walk every task without knowing any of their names.

The pin moves because the kernel does, and that is what the pin is for.
