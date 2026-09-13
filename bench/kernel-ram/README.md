# `kernel-ram` — what a kernel costs in RAM, ours against theirs

```
  C FreeRTOS   static   1704 B, and it does NOT move when a task is added
               per task 84 B TCB + 512 B stack + a heap header
                        = 596 B minimum, from the heap, on demand

  Kairos       static   6304 B of arenas at 8 tasks / 8 queues / 16 timers
               per task 207 B, and that is the WHOLE cost
               heap     0 B. There is no on-demand allocation to size.
```

Run it:

```sh
sh bench/kernel-ram/run.sh        # needs clang, llvm-nm, llvm-size, cargo
```

## The finding: two floors, and they are different functions

This is the shape that matters, not the totals:

```
FreeRTOS:  RAM = 1704 + tasks x (84 + stack + header)
                        + queues x (72 + storage) + timers x 40 + groups x 28
                 ^ static                          ^ all of it from the heap

Kairos:    RAM = arenas(TASKS, QUEUES, TIMERS, GROUPS, BUFFERS, BYTES)
                 ^ all of it static; the heap term is ZERO
```

They are not one function tuned two ways. FreeRTOS's static cost is small and
**does not move when a task is added**, because the TCB comes from the heap at
`xTaskCreate` time. Kairos declares arenas, so its static cost is the whole
budget and every dimension moves it — and there is nothing left to allocate
at run time.

Which is better depends entirely on what you are optimising. A system that
must know its worst case at link time reads the Kairos column and is done.
A system that creates a few tasks and wants the smallest possible image
reads the FreeRTOS column.

## Per-unit cost, measured as a slope

Each figure is the difference between two geometries that differ in exactly
one dimension — never a total divided by a count.

| dimension | bytes | |
|---|---:|---|
| a task | 207 | TCB + its two list items, and **no stack** |
| a queue | 56 | |
| a timer | 88 | |
| an event group | 20 | |
| a stream buffer | 48 | |
| a notify slot | 8 | one `u64` |
| a buffer byte | 1 | one byte |

`ITEMS` and `LISTS` are derived from the other dimensions by `items_for` /
`lists_for`, exactly as a real configuration derives them — so a task is
charged for its two list items, which is where part of the 207 goes.

## The per-task comparison, and its counterweight

At the config's own `configMINIMAL_STACK_SIZE` of 128 words, a C task costs
**596 B** against our **207 B** — 2.9×. The gap is the stack: a blocking
Kairos call keeps its locals in the TCB (the `WaitFrame`), so a task can be
suspended mid-call without owning a stack. That is the same design fact that
let the conformance corpus run on four architectures before any
context-switch port existed.

**The counterweight, stated plainly:** this is the *kernel* cost. An
application still needs somewhere for its own state. A Kairos task holds it
in a struct sized to what it actually uses; a C task holds it on a stack
reserved for the worst case. The saving is real, but it comes from **exact
sizing**, not from the state ceasing to exist — a task whose state genuinely
needs 512 B pays 512 B in either kernel. Quoting 2.9× without this sentence
would be quoting a stack-sizing policy as a kernel result.

## Method

* **Both arms are measured on the target.** `size_of` on the host is a
  different number — a host `usize` is eight bytes and rv32's is four — so a
  host test would measure the wrong machine. The C arm is compiled for
  `rv32imac`; the Rust arm's figures are the **lengths of arrays** in an rv32
  staticlib, read back with `llvm-nm -S`. Making the size the symbol's size
  means there is no value to decode and nothing to run.
* **The configurations are matched** on the knobs that size the arrays: 5
  priorities, 16-byte task names, a 10-deep timer queue, one notification
  entry. Matching them is what makes the totals comparable rather than
  merely adjacent.
* **Struct sizes come from clang's own record layouts**, not from a header
  read by eye — `TCB_t` and `Queue_t` are private to their translation units.
* **The libc headers in `c/stub/` are declaration-only.** clang has no
  bare-metal riscv32 sysroot on this box. Declarations emit no code, so the
  object sizes are unaffected: calls to `memcpy` and friends stay external
  relocations exactly as they would with a real libc.

## Identity checks

Two slopes are true by construction, and the script checks them:

```
      ok    a notification slot is one u64 (8)
      ok    a stream-buffer byte is one byte (1)
      ok    the C TCB still carries its two list items (84)
```

An instrument that cannot reproduce a number that is true by construction has
not earned belief in the numbers that are not. These are the reason the other
five slopes can be quoted.

## A packaging note

`rs/.cargo/config.toml` is **committed**, unlike the per-repo tables
`kairos patches` writes. It has to be: `rusty_rtos_kernel-core` reaches
`rusty_rtos_core` by git URL, so without a patch cargo builds *both* — the
local one for this cell and the published one for the kernel — and the kernel
compiles against a sibling missing whatever the working tree just added to it.
That is not hypothetical: the first build of this cell failed on
`Port::COMMITS_SWITCH`, a constant that exists here and not in the published
crate. The per-repo tables are gitignored because their repos must also
resolve standalone from a fresh clone; this cell never does, since it is a
measurement *of* the umbrella working tree.

## What this does NOT claim

Flash. The C arm's `text` column is printed (13,990 B for the five kernel
modules at `-Os`) but **not compared**, because object-file `.text` includes
functions a linker would garbage-collect and the two arms expose different
API surfaces. A flash comparison needs both arms linked from a matched root
set, which needs `rusty_rtos-capi` to stop being a scaffold.
