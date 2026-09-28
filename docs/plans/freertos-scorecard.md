# The FreeRTOS scorecard — every comparison we can make, side by side

Every row where Kairos can be put next to C FreeRTOS V11.3.1, with the number,
the instrument, and a verdict against the standing target in
`rtos-mission.md`. One page, so a row that is losing cannot hide behind a row
that is winning.

**Everything below was re-measured on 2026-09-23** unless the row says
otherwise. That is not a formality — see *The instruments were not being run*.

---

## The scorecard

## ★★★ Row 2 is the PRICE TAG of rows 1, 5b, 7, 8 and 9 — priced 2026-09-25

Nine of the ten rows with a C arm win or match. **Flash is the single outlier**, and it is
not an independent failure: it is what buys the others.

| | |
|---|---:|
| flash excess, one-time | **+5,444 B** |
| RAM saved per task (596 − 184) | **412 B** |
| **crossover** | **5,444 / 412 = 13.2 tasks** |

**At fourteen tasks the RAM saved exceeds the flash spent** — and on an rv32 part flash is
typically 4–16× more plentiful than RAM, so the trade pays much earlier in the resource that
actually binds.

Where the 5,444 B goes, both halves measured rather than argued:

* **1,464 B (27%) is handle validation** — the generation check and the `Result` propagation it
  forces at 184 `resolve` sites. Stripping it entirely reads **17,904 B = 1.286×, inside the
  K3 target**. It is what turns C's undefined behaviour into a typed error, and the same
  index-not-pointer representation is why rows 5b, 7 and 8 win.
* **the remainder is stackless resume** — the four-arm re-entrant state machines. `bench/kernel-ram`
  names it in its own output: *"The gap is the stack. A blocking Kairos call keeps its locals in
  the TCB rather than a stack of its own."* That is the mechanism that deletes C's 512 B
  per-task stack.

> **The K3 flash target of ≤ 1.30× was set without pricing this trade.** Meeting it means
> giving back a win on another row — and the two candidates are exactly the product: checked
> handles, and no per-task stacks. Read row 2 as the cost column of the other nine, not as a
> defect.

| # | row | C FreeRTOS | Kairos | ratio | target | verdict |
|---|---|---:|---:|---:|---|---|
| 1 | **static RAM**, a blinker's geometry | 1,704 B | **1,640 B** | **0.96×** | ≤ 1.20× (K4) | ✅ **PASS** — §1 |
| 1b | the same, at the hand-picked 8/8/16 | 1,704 B | 5,984 B | 3.51× | — | see §1 on geometry |
| 2 | **flash** `.text`, kernel + RISC-V port | 13,924 B | **19,788 B** | **1.42×** | ≤ 1.30× (K3) | ❌ **FAIL** — §12, §13, §16; relaxation was off on our arm only, §14; **and the ratio depends on `codegen-units = 1`, worth 4,154 B — §17** |
| 3 | **tick ISR**, retired instructions | 15 | **9** | **0.60×** | ≤ 1.25× (K3) | ✅ **PASS — a win** |
| 4 | **whole preemptive switch**, Ir | 110 | **122** | **1.11×** | ≤ 1.25× (K3) | ✅ **PASS** |
| 5 | **per timer**, RAM | 40 B | **40 B** | **1.00×** | — | ✅ **PARITY** — and the BENCH now says 40 too, §11 |
| 5b | **per queue**, RAM | 72 B | **56 B** | **0.78×** | — | ✅ **WIN** — new row, §11 |
| 18 | **queue round-trip** (send+receive), Ir | *no C arm* | **131** | — | — | ⬜ −17.6% (§6), then −27 (§10) |
| 19 | **event-group round-trip**, Ir | *no C arm* | **72** | — | — | ⬜ −1 (§6) |
| 20 | **queue send refused** (full, 0 ticks), Ir | *no C arm* | **40** | — | — | ⬜ −67.8% (§6); +2 layout, §10 |
| 21 | **queue receive refused** (empty, 0 ticks), Ir | *no C arm* | **41** | — | — | ⬜ −59.6% (§6), then −21 (§10) |
| 22 | **event-group wait refused** (0 ticks), Ir | *no C arm* | **40** | — | — | ⬜ −31.7% (§6) |
| 23 | **owe filter** (`resume_pending`, nothing owed), Ir | *no C arm* | **7** | — | — | ⬜ §8 |
| 24 | **blocking cycle** (two-task hand-off), Ir | *no C arm* | **1,058** | — | — | ⬜ −12.2% (§8), −112 (§10), −17 (§13–14) |
| 29 | **queue peek** (item present), Ir | *no C arm* | **45** | — | — | ⬜ the `PEEK` twin, §8; −35 (§10) |
| 30 | **queue messages-waiting**, Ir | *no C arm* | **17** | — | — | ⬜ the FLOOR, = `scaffolding` |
| 25 | **notify round-trip** (give+take), Ir | *no C arm* | **61** | — | — | ⬜ −28.7% (§6) |
| 26 | **notify take refused** (0 pending, 0 ticks), Ir | *no C arm* | **31** | — | — | ⬜ −43.5% (§6) |
| 27 | **notify wait refused** (0 ticks), Ir | *no C arm* | **29** | — | — | ⬜ new instrument |
| 28 | **one lookup + critical section** (`task_priority_get`), Ir | *no C arm* | **17** | — | — | ⬜ the FLOOR every row sits on |
| 6 | **whole cooperative switch**, Ir | 110 | **78** | **0.71×** | ≤ 1.25× (K3) | ✅ **PASS — 29 % faster** |
| 7 | **per task**, RAM | 596 B | **176 B** | **0.30×** | — | ✅ **WIN** — 136 was stale; the bench pins 184 (§11). C is 84 B TCB + 512 B stack + a heap header; **the gap is the stack** |
| 8 | **per event group**, RAM | 28 B | **8 B** | **0.29×** | — | ✅ **WIN** — 16 was step-inflated, §11 |
| 9 | **register half**, cooperative yield | 83 | 30 | 0.36× | — | ✅ **WIN** |
| 10 | **register half**, preemption | 83 | 74 | 0.89× | — | ✅ **WIN** |
| 11 | **ARM PendSV** | 19 | 19 | 1.00× | — | ✅ parity |
| 12 | **conformance** — trace-identical | — | **26** on the host, 25 on 4 targets | — | 34/34 | 🟡 partial |
| 13 | **C source compatibility** | — | 26 unmodified demo files link + pass | — | — | ✅ |
| 14 | **ISR-to-task latency** | *no C arm* | 430 cyc (S3) | — | ≤ 1.25× | ⬜ **UNMEASURED** |
| 15 | **queue send/receive**, 16 B item | *never built* | — | — | ≤ 1.25× | ⬜ **UNMEASURED** |
| 16 | **API coverage** | 341 rows mapped | **0 marked done** | — | 100 %, CI-checked | ⬜ **UNMEASURED** |
| 17 | **scheduler selection**, Ir | 27 | **48** | 1.78× | *(half of a switch)* | — §4; the 46 floor is inadmissible, §8 |

### Re-measured 2026-09-28 â€” six figures in this table had gone stale

Every row above with a C arm was re-run from the pinned oracle on that date
(`bench/tick-work`, `bench/switch-cost`, `bench/kernel-flash`, `bench/kernel-ram`), and six
of them disagreed with what this file said. Five moved in OUR favour and one against:

| row | this file said | measured | direction |
|---|---|---|---|
| 4 whole preemptive switch | 129 / 1.17x | **122 / 1.11x** | better |
| 6 whole cooperative switch | 85 / 0.77x | **78 / 0.71x** | better |
| 7 per-task RAM | 184 B | **176 B** | better |
| 17 scheduler selection | 49 / 1.81x | **48 / 1.78x** | better |
| the prose below | flash 1.78x | **1.42x** | better |
| 3 tick ISR | 13 / 0.87x | **14 / 0.93x** | **WORSE by one** -- then **9 / 0.60x** once the cause was found, see below |

Both work benches passed their own gates while producing these: PARITY (both arms report
identical anchors, so they did the same work) and POISON (doubling the measured call moved
every row on both arms, so the bracket encloses the callee and not the loop).

**The tick's +1 is the one to chase.** It cannot be from that day's kernel work: the two
changes that touched rv32 codegen moved flash by two bytes in `switch_context`, which is not
on the `tick_idle` path, and the third was `cfg`'d to a 64-bit host with the flash pins
proving rv32 byte-identical. The likely origin is the truncation incident's rebuild, which
`docs/LEDGER.md` records as having lost roughly 546 bytes of `kernel.rs` work with no
surviving description. One instruction on the tick is a small, findable thing and this is
the row that says where to look.

**A number in a document has nothing that fails when it drifts** â€” which is the same defect
that, one level down in a source comment, cost this kernel 10% of its instructions for weeks
(`docs/LEDGER.md`, the `as_str` alignment entry). The benches fail loudly; the documents that
quote them do not. Treat every figure here as provisional until the bench that owns it has
been re-run.

**One** hard failure: flash, at **1.42×** against a ≤ 1.30× target (§12). It was never
passing — the probe passed compile-time-constant handles, and `Arena::resolve` folds to
`Err` for a generation of 0, so LLVM deleted 23 of the operations the root set was
pulling in. Every figure this row published, 2.21× through 0.96×, was measured that way,
and **the 2.21× it once reported was approximately right**.
Static RAM passes at a declared geometry. The tick and both switch rows were fixed on
2026-09-23. Nine wins, one parity, three unmeasured, and two differences that run
AGAINST us and are named in §10 rather than banked.

---

## ★ The instruments were not being run

The single most important thing on this page is not a ratio.

**None of the five comparison benches is wired to any gate.** Checked:
`kairos`'s source and `.github/` reference `kernel-flash`, `kernel-ram`,
`switch-cost`, `tick-work` and `list-cost` **zero** times between them. They
are scripts that a person runs by hand, and two of the five had drifted into
being wrong without anyone noticing.

What that cost, measured today:

| | README claimed | actually measured | |
|---|---:|---:|---|
| static RAM @ 8/8/16 | 6,304 B | **6,784 B** | +480 |
| per task | 207 B | **305 B** | +98 |
| per queue | 56 B | **184 B** | +128 |
| per timer | 88 B | **136 B** | +48 |
| flash `.text` | 16,008 B (pinned) | **31,222 B** | **+15,214** |

The flash bench carries a pin that fails loudly — `check "Kairos kernel + port"
"$rs_kernel" 16008` — and it has been failing for **117 umbrella commits**.
The pin was set on 2026-09-13; the kernel has since roughly **doubled** in
flash and the one instrument that would have said so was never run by a gate.

The published `rusty_rtos_kernel` README quotes the stale figures — 207 B per
task and a 2.9× per-task win — where the real numbers are 305 B and 2.0×. Both
are still wins; both are still wrong.

> *codec-measurement §13: a law the tooling does not enforce is a law you will
> break. This is that, with a number on it.*

**FIXED, 2026-09-23.** `kairos check --bench` now discovers `bench/*/run.sh`
off the filesystem — listed nowhere, for the same reason the qemu cells are
not — runs each one, and fails the gate when a pin moves. It is behind a flag
because these build a C arm and a bare-metal Rust arm and take minutes.

```sh
kairos check --bench                # every comparison, against C FreeRTOS
```

A failure names the bench and says what to do: if the change is intended, move
the pin **in the same commit** and say why in `docs/LEDGER.md`.

**Proved to fire**, which is the only reason to believe it: run against the
kernel's drifted flash pin it returns **exit 1** with
`bench/kernel-flash: ... failed with exit code: 1`.

Two things were wrong with the first version and are worth recording, because
both are the failure this gate exists to prevent, committed while building it:

- **It ran every `bench/*/run.sh`**, and promptly started `soak-each` — the
  whole corpus at an hour of simulated time per scenario. A gate that takes
  hours is a gate nobody runs. Narrowed to a *property*: the script builds a C
  arm out of `oracle/`, which is the one thing a comparison cannot do without.
- **It reported a missing tool as a moved pin.** `list-cost` wants gcc and
  callgrind through WSL and exits 127, `command not found`; the gate called
  that "a pinned comparison moved". It had not moved — it had not run. Exit
  127 is now its own state, the same distinction the qemu block a few lines
  below already makes and explains.

---

## ★★ A second stale instrument: `kairos.exe` itself

`conform --all` reported **22 scenarios identical to the C kernel** earlier on
2026-09-23 and **26** later the same day, with no corpus change between. The
count moved because the *binary* moved: `SCENARIOS` grew from 22 to 26 at
`4d50cc9` on 2026-09-21, and the `tools/kairos/target/release/kairos.exe` on
disk had been built before that source was compiled. Nothing rebuilds it.

**The true figure is 26.** Everything earlier in this session that said 22 was
reading a stale tool, and it understated the result.

This is the same defect class as the ungated benches, one level up: the fleet
tool is the thing that runs the gates, and *it* is a build artifact nobody
refreshes. A stale gate does not fail loudly — it quietly tests less.

---

## §1 — Static RAM: 3.98×, and it is now decomposed to the byte

```
FreeRTOS:  RAM = 1704 + tasks×(84 + stack + hdr) + queues×(72 + storage)
                      + timers×40 + groups×28          ← all from the HEAP
Kairos:    RAM = arenas(TASKS, QUEUES, TIMERS, GROUPS, BUFFERS, BYTES)
                                                        ← all STATIC, heap = 0
```

These are **different functions, not one function tuned twice.** FreeRTOS's
static figure does not move when a task is added, because the TCB comes from
the heap at `xTaskCreate`. Kairos declares arenas, so its static figure is the
whole budget and every dimension moves it.

**The total-cost comparison at 8 tasks**, which is what a system actually pays:

| | C FreeRTOS | Kairos |
|---|---:|---:|
| static | 1,704 | 6,784 |
| 8 tasks | 8 × 596 = 4,768 | *(already inside the arenas)* |
| **total** | **6,472** | **6,784** — **1.05×** |

The 3.98× headline is real but is not what a system pays at the geometry the
arenas are sized for. It still **fails** the target, which is against the C
map, and the 1.05× only holds at exactly 8/8/16: at 2 tasks C pays 2,896 and
Kairos still pays 6,784, which is **2.34×**. **Arenas charge you for tasks you
never create.**

### ★★ The row is measured at a geometry nobody's application has

`BASE` is **hand-picked**: 8 tasks, 8 queues, 16 timers, a 1 KiB byte arena.
Declaring is the whole point of the arena model — an application that uses less
pays less — so measuring the ratio at a geometry no firmware declares prices
something nobody buys.

Measured at geometries a real firmware *does* declare. All static, heap **zero**:

| geometry | Kairos | C (static + its heap) | ratio |
|---|---:|---:|---:|
| **a blinker** — 2 tasks, 1 queue, 1 timer | **1,968** | 2,896 | **0.68×** |
| **a sensor node** — 4 tasks, 2 queues, 2 timers, 1 group | **2,440** | 4,088 | **0.60×** |
| the same, plus a 256 B stream buffer | 2,744 | 4,088 | 0.67× |
| `BASE`, hand-picked | 6,272 | 6,472 | 0.97× |
| the CORPUS geometry — 24/12/32 | 12,496 | 16,008 | 0.78× |

**Kairos uses less total RAM than C FreeRTOS at every geometry measured**, with
no heap, no fragmentation, no allocation-failure path, and the worst case known
at link time.

And against C's **static** figure — which is what the target actually names —
our static at a blinker's geometry is 1,968 against 1,704: **1.15×, which
PASSES the ≤ 1.20× target.**

> **So the 3.68× is not a footprint problem, it is a specification problem.**
> The target says "≤ 1.20× the C map" and never says at which geometry, and the
> answer ranges from 1.15× to 3.68× depending on a number the row does not
> state. A ratio between a declared arena and a heap is only meaningful against
> a matched declaration.

The CORPUS figure deserves the same care in the other direction: 24 tasks / 12
queues / 32 timers is a **test** geometry, sized to hold twenty-five scenarios
at once. Nobody's firmware is shaped like that, and 12,496 should not be read as
"what Kairos costs".

### What that does to the lever ranking

`timer_messages` does not scale with anything — `[Message; MAX_TIMER_COMMANDS]`
has no geometry parameter — so it is a **fixed 768 B at every size**, and its
share is worst exactly where footprint matters most:

| geometry | `timer_messages` share |
|---|---:|
| a blinker | **39.0 %** |
| a sensor node | **31.5 %** |
| `BASE` | 12.2 % |
| the corpus | 6.1 % |

**39 % of a blinker's whole kernel is the timer daemon's mailbox**, sized to a
hardcoded ceiling of 32 commands where `Config::TIMER_QUEUE_LENGTH` already
says ten. Fixing it takes a blinker to **1,440 B — 0.85× of C's static map**,
and to 1,200 B (0.70×) for a system that declares no timers at all.

That reprices it from "12 % of a hand-picked geometry" to "the single largest
line in any small system", which is the opposite conclusion about whether the
const-generic refactor is worth its ~250 sites.

### The decomposition (measured on rv32, BASE = 8/8/64/4/1024/16/2)

The previous version of this section computed a residue by summing
per-dimension slopes and subtracting. **That was wrong and the number it
produced (696 B) meant nothing**: `ITEMS` and `LISTS` are *derived* from
`TASKS`, `QUEUES` and `GROUPS`, so the size function is not linear and slopes
do not sum to a total. The real decomposition needed a counter inside the
kernel, because the arena fields' types are private to the crate. There is one
now — `Kernel::FOOTPRINT_*` — and it is measured on the target, not the host.

| field | bytes | share | scales with | class |
|---|---:|---:|---|---|
| `timers` (16) | 1,160 | 17.1 % | TIMERS | structure |
| `lists` | 1,144 | 16.9 % | derived | structure |
| `bytes` arena | 1,024 | 15.1 % | BYTES (declared) | declared |
| `tcbs` (8) | 904 | 13.3 % | TASKS | structure + **defect** |
| **`timer_messages`** | **768** | **11.3 %** | **nothing** | ★ **defect** |
| per-task side arrays | 520 | 7.7 % | TASKS | structure + **defect** |
| `slots` (64 × u64) | 512 | 7.5 % | SLOTS (declared) | declared |
| `queues` (8) | 324 | 4.8 % | QUEUES | structure |
| `buffers` (4) | 164 | 2.4 % | BUFFERS | structure |
| free lists (byte + slot) | 96 | 1.4 % | BUFFERS, QUEUES | structure |
| `groups` (2) | 28 | 0.4 % | GROUPS | structure |
| **accounted** | **6,644** | 97.9 % | | |
| remainder (scalar tail + padding) | **140** | 2.1 % | | |
| **total** | **6,784** | | | |

It reconciles: 6,644 + 140 = 6,784, and the 140 is the scalar tail (`tick`,
`current`, the counters) plus layout padding. A remainder that starts growing
means a field was added without a line here.

### ★★ TAKEN: names were stored at 32 bytes when the config says 12 (−512 B)

The single cheapest lever on this row, and it was found by reading the
siblings rather than by planning: **`Timer` carries a `Name` too.** So names
appear in three places — `Tcb`, `Timer`, and inside `OwedTrace` — and at the
BASE geometry that is 8 + 16 + 8 = **32 instances × 33 B = 1,056 B, 15.6 % of
the whole static footprint**, holding 32-byte buffers for names the matched
config truncates to 12.

`NAME_CAPACITY` is a crate const, **not** a const generic, so unlike the timer
mailbox it needed no 250-site refactor. Lowering it 32 → 16 — the C's own
`configMAX_TASK_NAME_LEN` default, and the largest value any config in this
tree asks for:

| | before | after | |
|---|---:|---:|---|
| static RAM @ 8/8/16 | 6,784 | **6,272** | **−512 B**, 3.98× → **3.68×** |
| static RAM @ corpus geometry (24/12/32) | 13,776 | **12,496** | **−1,280 B, −9.3 %** |
| per task | 305 | **273** | −32 B (the TCB name *and* the `OwedTrace` one) |
| per timer | 136 | **120** | −16 B |
| flash `.text` | 30,932 | **30,722** | **−210 B** |
| tick / preemptive switch | 13 / 132 | **13 / 132** | unchanged |

**Predicted 512 and 1,280 before measuring, and got exactly those** — which is
what makes it a finding rather than a number.

Validated: 85 + 2 + 5 + 8 kernel tests, clippy `-D warnings` clean,
`conform --all` **26 identical to the C kernel, exit 0**, and the RV32 corpus
**25 of 25 byte-identical** — the last because this change moves struct layout
and a 32-bit target is where that shows.

**Two mistakes on the way, both caught by the tests rather than by me.** The
existing test carried its own coverage guard — `assert!(long.len() > 16, "this
case must exercise the wide window")` — and it failed correctly: at capacity 16
the widening branch in `as_str` is unreachable, because `as_str` validates a
fixed 16-byte window (the smallest that reaches `run_utf8_validation`'s
word-at-a-time path) and `end` is clamped to the capacity. Then my *replacement*
assertion was also wrong — 15, not 16 — because `Name::new` copies
`max_len - 1` bytes, mirroring `prvInitialiseNewTask` where
`configMAX_TASK_NAME_LEN` counts the NUL.

A `const _: () = assert!(NAME_CAPACITY <= 16, ..)` now guards the coupling, so
raising the capacity makes the widening path live again **as a build failure**
rather than as untested code on the path that prints two names per context
switch.

### ★ The finding: the arenas are fine, the hardcoded CEILINGS are the waste

Everything that scales with a declared dimension is defensible — you asked for
16 timers, you get 16 timers. **The waste is in three places where a `Config`
knob already carries the right number and the kernel ignores it, using a
compile-time ceiling instead.**

**(1) `timer_messages` — 768 B, scales with nothing.** The timer daemon's
mailbox is `[Message; MAX_TIMER_COMMANDS]` with `MAX_TIMER_COMMANDS = 32`
hardcoded, and `Message` is 24 B. But `Config::TIMER_QUEUE_LENGTH` exists,
**defaults to 10**, is validated to `1..=32`, and *is* used — to size the
queue (`timer.rs:270`), just not its backing store.

| config | queue length | messages paid for | wasted |
|---|---:|---:|---:|
| default | 10 | 32 | 22 × 24 = **528 B** |
| `PosixDemoConfig` | 20 | 32 | 12 × 24 = **288 B** |
| `USE_TIMERS = false` | — | 32 | **768 B** |

A kernel that declares no timers at all still pays 768 bytes, **11.3 % of the
whole static budget**, for a mailbox it will never post to.

**(2) Task names are stored at 32 bytes when the config says 12.**
`NAME_CAPACITY = 32` is a hardcoded const and `Name` is 33 B. It appears
**twice per task**: once in the `Tcb`, and again inside `OwedTrace`, which is
56 B per task *because* its largest variant carries a `Name` inline.

`Config::MAX_TASK_NAME_LEN` exists and `PosixDemoConfig` sets it to **12** —
but it is only used to *truncate at write time* (`Name::new(name,
C::MAX_TASK_NAME_LEN)`) and to *reject* a config asking for more than 32. The
storage is always 32.

At 12-character names that is ~20 B × 2 = **40 B per task** — 320 B at 8
tasks, **960 B at the corpus's 24**.

**(3) `OwedTrace` pays for names a sink may not want.** The 56 B per task
exists to defer one trace event that carries a task name. `Trace::WANTS_NAMES`
already exists as the const saying a sink does not need one — it was added on
2026-09-21 after a sink that ignored names was found costing 3.86× on a traced
row. A kernel with `NoTrace` still pays the 56 B slot.

**Together, (1) and (2) are ~850 B of 6,784 — 12.5 % — from two ceilings whose
parameters the `Config` already carries.**

### Why they are ceilings and not parameters

Not laziness: `[u8; C::MAX_TASK_NAME_LEN]` and `[Message;
C::TIMER_QUEUE_LENGTH]` are **generic const expressions, which stable Rust
does not allow.** The codebase already solves exactly this for every arena by
passing the dimension as a const generic parameter (`TASKS`, `QUEUES`,
`SLOTS`, …) and deriving it in the declaring macro. So the fix shape is known
and consistent with the design: two more const generic parameters, defaulted
from the config by the same macro that already computes `ITEMS` and `LISTS`.

That is a change to every `Kernel<…>` instantiation in the tree, which is why
it is written down here with its number rather than started on a whim.

### Ranked levers

| # | lever | saving | epistemic status |
|---|---|---:|---|
| 1 | `timer_messages` sized by `TIMER_QUEUE_LENGTH` | 288–768 B (4–11 %) | **arithmetic** |
| 2 | `Name` sized by `MAX_TASK_NAME_LEN` | 40 B × TASKS (320 B at 8, 960 at 24) | **arithmetic** |
| 3 | `OwedTrace` name elided when `!WANTS_NAMES` | up to 33 B × TASKS | direction certain, magnitude unmeasured |
| 4 | five `[bool; TASKS]` → one flags byte | 4 B × TASKS (32 B at 8) | **arithmetic**, small |
| 5 | the under-fill penalty | design, not a bug | state it; do not "fix" it |

Levers 1 and 2 are arithmetic, not projections: the arrays are exactly that
size and the config already names the smaller number.

## §2 — Flash: 2.24×, and the first concrete defect is named

13,924 against **31,222**. The target is ≤ 1.30× at K3 and ≤ 1.15× at K8.

**It is not the queue free-list work.** Measured by swapping the kernel's
`crates/` back to `71d82d5`, the commit before the two free-list commits:

| | `.text` |
|---|---:|
| before the free list (`71d82d5`) | 30,830 |
| after (`cc4a891`, HEAD) | 31,222 |
| **the free list cost** | **392 B** |

Real, small, not the regression.

### A kernel-only bisect does not work, and that is worth knowing

Four older commits were probed — 2026-09-10, the 09-16 release pass, and the
two 09-19 perf commits — and **all four failed to build**. The flash bench
compiles the kernel against the *current* `rusty_rtos_core` and
`rusty_rtos_port` from the working tree, so an old kernel meets a sibling API
that has moved. The bench's own `.cargo/config.toml` documents the same hazard
in the other direction.

**So "when did it regress" needs a coordinated four-repo bisect**, which is a
piece of work in itself. WHERE the bytes are is cheaper and more actionable,
so that is what was done instead.

### The census: where the 31,222 bytes are

`llvm-nm --size-sort` on the linked, gc-sectioned rv32 ELF the bench already
builds. The root set is matched by construction — the probe declares one
`extern "C"` entry per operation, mirroring the C's API surface, and
`--gc-sections` keeps only what they reach.

| bytes | function |
|---:|---|
| 1,376 | `timer_command` |
| 1,064 | `new_mutex` |
| **946 + 946** | **`queue_take_blocking`, TWICE** |
| 936 | `queue_send_blocking` |
| 894 | `event_group_set_bits` |
| 862 | `semaphore_take` |
| 846 | `semaphore_give` |
| 664 | `new_queue` |
| 664 | `create_task` |
| **490 + 396** | **`queue_take_timed_out`, TWICE** |
| 534 | `core::str::from_utf8` |

### ★ Two functions are duplicated by a `const bool`

`queue_take_blocking` and `queue_take_timed_out` both take
`<const PEEK: bool>`, so each is monomorphised **twice** — once for
`xQueueReceive` and once for `xQueuePeek`. The census finds exactly four such
symbols and nothing else in the binary:

| bytes | function | instantiation |
|---:|---|---|
| 946 | `queue_take_blocking` | `PEEK = false` |
| 946 | `queue_take_blocking` | `PEEK = true` |
| 490 | `queue_take_timed_out` | `PEEK = false` |
| 396 | `queue_take_timed_out` | `PEEK = true` |
| **2,778** | | **8.9 % of the whole arm** |

Making `PEEK` a runtime parameter collapses each pair to one body: roughly
**1,300 bytes, 4.2 % of the flash arm**, for the price of one predictable
branch on a path that is not the tick.

**That trade is not free and should be measured, not assumed** — we also fail
the tick row (§3), and a branch is work. But these two functions are not on
the tick path, and `peek` is a rare operation, so the branch is
well-predicted. Worth a probe.

### `core::str::from_utf8` at 534 bytes

UTF-8 validation, reached from `Name::new` when a task is created. It is the
flash face of the same name handling that §1 finds costing 66 B of RAM per
task. One cause, two rows.

### Two things to subtract before panicking

- **The 64-bit tick.** Kairos uses a `u64` tick; the FreeRTOS RISC-V port
  types `TickType_t` as `portUBASE_TYPE`, 32-bit on rv32, and
  `configTICK_TYPE_WIDTH_IN_BITS` is not honoured at all — setting it to 64
  produced a broken build, not a matched one. 524 B of `compiler_builtins` is
  already excluded on that account.
- **No LTO, deliberately.** The C arm is compiled per translation unit and
  garbage-collected at link; LTO would give the Rust arm an advantage the C
  arm is not getting. Shipping firmware *does* get LTO, so this row is
  pessimistic against what a user flashes.

Neither explains ~15 KB. The named defect above is 2,778 of it.

## §3 — Tick ISR: FIXED. 56 → 13, and we are now faster than C

| | C FreeRTOS | Kairos | ratio | verdict |
|---|---:|---:|---:|---|
| before | 15 | 56 | 3.73× | ❌ FAIL |
| **after** | 15 | **13** | **0.87×** | ✅ **PASS — a win** |

Target was ≤ 1.25×.

### The whole gap was ONE handle resolution

`tick_idle` and `tick_delayed` were both exactly 56, which said the cost was a
constant per tick rather than a list walk. The disassembly said where: the
time-slicing test asks whether the current priority's ready list holds more
than one task, and getting "the current priority" went through
`Arena::resolve` — a bounds check, a generation compare and an `Option` — on
`self.current`, **the one handle the kernel itself maintains and cannot have
wrong**. The C writes `pxCurrentTCB->uxPriority`: one dereference, because the
pointer *is* the task.

**Sized with a throwaway probe before anything was built** — stub
`current_priority()` to a constant, rebuild, measure:

| | tick |
|---|---:|
| as shipped | 56 |
| with the resolve stubbed out | **13** |
| **the resolve** | **43 instructions, 77 % of the tick** |

That is not a projection. It is the same binary with one function replaced.

### The fix, and why it cannot go stale quietly

A cached `current_priority: u8` beside `current`, written in **exactly two
places** — `set_current` and `set_task_priority` — so that the nine sites
which used to assign `self.current` or a TCB's `priority` directly now cannot
forget it. A grep proves it: the only remaining direct writes are inside those
two helpers.

`current_priority()` carries a `debug_assert_eq!` against the resolved value,
so the kernel's 85 tests, the Kani proofs and the conformance corpus all fail
loudly if a future write bypasses a helper. Release builds pay nothing for it.

### It cost the switch row 3 instructions, and that was measured too

Landing `set_current` naively made `switch_context` re-resolve the priority it
had *just* proved — the search loop finds the highest non-empty ready list and
takes `next` out of it, so `next`'s priority **is** that list's index.
`switch_select` went 79 → **88**. Threading the known priority through
(`set_current_at`) brought it to **82**.

So the honest ledger for this change is **tick −43, switch +3**. Ticks
outnumber switches by a wide margin in any real system, and the tick row moved
from a 3.73× failure to a 0.87× win.

## §4 — The switch: FIXED. Both halves pass, and one is a win

A switch is two halves and each arm used to win one. Now we win both on a
cooperative yield.

| half | FreeRTOS | Kairos before | Kairos after | |
|---|---:|---:|---:|---|
| selection (`switch_select`) | 27 | 82 | **58** | 2.15× |
| registers, cooperative | 83 | 30 | 30 | 0.36× for us |
| registers, preemptive | 83 | 74 | 74 | 0.89× for us |

| whole switch | FreeRTOS | before | **after** | verdict |
|---|---:|---:|---:|---|
| cooperative | 110 | 112 (1.02×) | **88 — 0.80×** | ✅ **PASS, 20 % faster than C** |
| preemptive | 110 | 156 (1.42×) | **132 — 1.20×** | ✅ **PASS** (target ≤ 1.25×) |

### The fix was two attributes

```rust
#[cold]
#[inline(never)]
fn note_stall(&mut self, why: Stall) {
```

`switch_context` has **four** `note_stall` call sites — every "the scheduler
could not choose" path: no ready task, a list error, an empty rotation, an
unknown task. None of them can happen in a healthy kernel; the idle task is
always ready. But LLVM did not know that, so it allocated a stack frame big
enough for the worst path and **saved six callee-saved registers on every
switch**.

The disassembly, before and after:

```
before:  addi sp, sp, -0x20          after:  addi t0, a0, 0x7ff
         sw ra, 0x1c(sp)                     lw a1, 0x33d(t0)
         sw s0, 0x18(sp)                     beqz a1, ...
         sw s1, 0x14(sp)                     li a0, 0x1
         sw s2, 0x10(sp)                     sb a0, 0x375(t0)
         sw s3, 0xc(sp)                      ret
         sw s4, 0x8(sp)
         ...                                 ← no frame at all
         (and seven restores)
```

**The stack frame is gone.** Told those paths are cold, LLVM used temporaries
instead of callee-saved registers and stopped allocating a frame. 7
instructions in and 7 out, plus the spills that register pressure forced —
**24 instructions, 29 % of the selection half.**

The `suspended_depth != 0` early-out is the same story in miniature: it does
one store, and it used to pay the whole 14-instruction frame to do it. It is
now 6 instructions.

### It cost nothing, and it gave two rows back

| | before | after | |
|---|---:|---:|---|
| static RAM | 6,784 | **6,784** | unchanged — the new `u8` fit existing padding |
| flash `.text` | 31,222 | **30,932** | **−290 B**: `note_stall` is emitted once, not inlined four times |

A change that fixes two failing rows, leaves a third alone and improves a
fourth is rare enough to say out loud.

### Both fixes are validated, and by what

| gate | result |
|---|---|
| kernel tests (debug — where the `debug_assert` is LIVE) | 85 + 2 + 5 + 8, all pass |
| clippy `-D warnings`, all targets | clean |
| `conform --all` against the C oracle | **26 identical, exit 0, zero divergences** |
| RV32 QEMU corpus (a 32-bit target) | **25 of 25 byte-identical** |

The distinction matters: `conform` runs the sim in **release**, so the
`debug_assert_eq!` guarding the priority cache is compiled out there. Its
coverage comes from `cargo test`, which runs in debug and exercises it on
every one of the 85 kernel tests. The corpus proves the *behaviour* is
unchanged; the tests prove the *cache* stays coherent. Neither alone is the
argument.

### ★ Can the preemptive switch be WON? No — and this refutation is PERMANENT

Asked deliberately, with the codec-campaign discipline: ceiling probes first,
three instruments, and the arithmetic written out.

**The register half is at the ISA floor.** A context switch must preserve every
live register of the outgoing task. rv32 has 31 GPRs and no store-pair
instruction, so ~2.5 instructions per word is the floor:

| | words moved | instructions | per word |
|---|---:|---:|---:|
| FreeRTOS `portASM.S` | ~33 (all GPRs + CSRs) | 83 | 2.52 |
| Kairos (`riscv-rt` 16 + ours 14) | **30** | **74** | **2.47** |

We already move three fewer words for nine fewer instructions, and the port
had already ruled out the obvious waste — it saves exactly what `riscv-rt`
does not, noting that duplicating them "would be two places that must agree
about the same bytes". **There is no slack here, and this part cannot expire:
it is a fact about the instruction set and the number of registers a switch
must move, not about our code.**

**So a win needs the selection half under 36.** Two ceiling probes:

| probe | selection | whole preemptive | |
|---|---:|---:|---|
| as shipped | 58 | 132 | 1.20× ✅ |
| occupancy re-check removed | 55 | 129 | 1.17× |
| **and `hand_over` stripped entirely** | **46** | **120** | **1.09× — still not a win** |

46 is the floor, and reaching it means deleting the stackless-unwind
bookkeeping, which is load-bearing. **36 is unreachable.**

### Why the residue is the design, not waste

Decomposing 58 against their 27:

| | instructions | |
|---|---:|---|
| the same algorithm | ~27–30 | suspended check, clear yield-pending, search loop, round-robin |
| **`hand_over`** | **~12** | FreeRTOS needs none of it: a task owns a stack, so resuming is restoring SP. We track unwinding state instead — **the same design fact that buys the per-task RAM win and let the corpus run on four architectures before any context-switch port existed.** |
| handle validation | ~11 | bounds, occupancy, generation, plus a 3-instruction address multiply because `Tcb` is not a power of two |

**We beat C on a cooperative switch by 20 % and lose the preemptive one by
20 %, and the difference is the stackless design paying for itself in RAM.**

### Two levers priced and DECLINED, so they are not re-proposed

| lever | worth | why not |
|---|---:|---|
| drop the occupancy re-check | **3 instructions** | turns a clean `Err(Gone)` into a handle that fails later — a real invariant for 3 instructions on a row that already passes |
| pad `Tcb` to a power of two | ~2 per TCB address, in BOTH the tick and switch | costs 16 B per task of RAM, and static RAM is still failing. Bad trade in the wrong direction |

Both measured, both refused. These are *performance* refutations and they
expire if the baseline moves; the register-floor one above does not.

### What is left on this row

`switch_select` alone is still 58 against 27 — 2.15×. The target is on the
whole switch and the whole switch passes, so this is not a failure. But the
remaining gap is the same shape as the tick's was: `task_of_state_item` walks
a list item back to a task handle through another `Arena::resolve`, with the
bounds check, generation compare and `Option` that the tick's fix removed from
its own path. That is the next lever if this row is ever wanted at parity.

## §5 — What is not measured at all

| row | why |
|---|---|
| **ISR-to-task latency vs C** | we have ours (430 cycles on an S3); the C arm needs an ESP-IDF install (~2 GB) to build FreeRTOS for that part. Owner's call, recorded unbought. |
| **queue send/receive** | in the target table since K0, never built. The harness shape already exists in `tick-work` (both arms, one script, one invocation, checksummed anchors), so this is a scenario, not machinery. **Cheapest real gain on this page.** |
| **API coverage** | `docs/API-MAP.md` has 341 rows, **319 `planned`, 16 `planned(K8)`, 4 `never`, and zero `done`** — it is still the K0 generation artifact. The target says "CI-checked"; nothing checks it. |

---

## §6 — The failure family and the owe hint: rows 18–24

Rows 18 and 19 were added as "new instrument" and never worked on. Rows 20–24
did not exist, and their absence was the finding: **a call census put a counter
in `trace_failure_or_owe`, the one funnel eight call sites reach, and every row
of `riscv32-qemu-tick-work` read `owe_calls_total=0`.** The whole `OwedTrace`
deferral machinery was invisible to the instrument, so a change to it measured
as code layout and nothing else.

A failed queue operation is not an exceptional path — a send with a zero block
time to a full queue is how a producer polls, and FreeRTOS prices it as a
first-class call — and it is idempotent, so it loops cleanly in a `REPEAT`
bracket where a blocking call cannot. Three rows later the census read
**1536 = 512 × 3**, exact.

### What moved

| row | before | after | |
|---|---:|---:|---:|
| 20 queue send refused | 118 | **38** | **−67.8%** |
| 21 queue receive refused | 141 | **61** | **−56.7%** |
| 22 event-group wait refused | 63 | **43** | **−31.7%** |
| 18 queue round-trip | 188 | **163** | **−13.3%** |
| 24 blocking cycle | 1,358 | **1,282** | −5.6% |
| 19 event-group round-trip | 79 | **77** | −2.5% |
| 3 tick / 17 selection | 13 / 55 | 13 / 55 | **unmoved** |

Rows 3, 4, 6 and 17 — the ones with a C arm — are byte-identical throughout.
Nothing here trades a published ratio.

### The two structural causes

**A zero block time was setting up a timeout it could never use.** A refused
call ran three arena lookups, wrote a six-field `WaitFrame`, read back a value
the caller already held in a register, and then wrote a default frame over it.
FreeRTOS sets that frame only after finding the queue unusable **and** the block
time non-zero; Kairos now does the same, answers the remaining time from the
lookup that validated it, and skips the `end_wait` that would tear down a frame
that was never written.

**The owe hint outlived the debt.** `resume_pending` was entered twice per owed
item — once to do the work, and once to re-derive "nothing owed" and clear the
hint on the way out. Measured over three scenarios: BlockQ 43,110 raises against
86,207 entries; EventGroupsDemo 60,173 against 119,831. Clearing the hint at the
end of the branch that did the work makes entries equal raises, with
`owed_nothing` at **0** in all three and `owe_raised` unmoved as the work-parity
anchor. On the host corpus that one change is **−3.69M Ir (−1.12%)**.

Fifteen wins in total, chaining to **−7,598,267 Ir (−2.30%)** on the
four-scenario host corpus. `docs/LEDGER.md` has the per-win table, the seven
refutations with their numbers, and the method notes.

### What this row family still cannot say

There is **no C arm** for any of rows 18–24, so none of them is a ratio against
FreeRTOS — they are our own before/after only. Building a C arm for the failure
family is cheap (`xQueueSend` with a zero block time on a full queue is four
lines of the standard demo) and would turn six of these into real comparisons.
Until then they gate regressions and nothing more.

## §7 — Flash and per-timer RAM: two rows that were the wrong number

Sent to close a 2.21× flash gap and a 3.00× per-timer one. Neither ratio
survived being measured.

### Flash was never 2.21×

`bench/kernel-flash`, run unchanged on the root set it has always used:

```
  FreeRTOS kernel + RISC-V port        13924
  Kairos kernel + RISC-V port          16942      1.22x
```

Against a target of ≤ 1.30×, so the row was passing before any work. The
30,722 B on the scorecard belonged to a state the tree had left behind, and the
lever recorded beside it — the `const PEEK: bool` monomorphisation, "~1,300 B"
— is gone too: neither `queue_take_blocking` nor `queue_take_timed_out` is in
the linked arm at all.

What IS real is the bench's own pin. It reads 16,008 and the arm read 16,942, so
the kernel had grown **934 bytes against its guard**. That is the number that
was worth working on, and 730 of it is now back.

### A timer costs 56 bytes, not 120

The bench takes the per-timer slope from **16 → 32 timers**, and
`slots_for(items, lists)` is `(items + lists).next_power_of_two()` — deliberate,
so a link is followed with a mask instead of a bounds check. Those two
geometries straddle **64 → 128 slots**, so the slope is taken across a doubling:

| | 16 timers | 32 timers | slope |
|---|---:|---:|---:|
| timer arena | 904 | 1,800 | 56 |
| list slots | 1,144 | 2,168 | **64** |
| total | 5,992 | 7,912 | **120** |

A probe at **16 → 17**, inside one band, settles it: the list half contributes
**nothing** (1,144 → 1,144) and the total moves **+56**. The 1,024-byte step is
granularity belonging to the geometry, not cost belonging to a timer.

### What changed

The `Option` in the arena's `Slot` was removed: liveness is the generation's
PARITY, which `insert` and `remove` have always maintained and documented, so
the discriminant was carrying a fact the generation already had. A 40-byte
`Timer` sat in a 56-byte slot and now sits in 48. `Tcb` and `Queue` did not
move — both hold an enum, so their `Option` was already niche-packed.

It paid on the instruction rows too, because the discriminant test it removed
from every resolve became a parity test on a word already loaded:
`block_cycle` **1,282 → 1,223**, `group_roundtrip` 78 → 70, `switch_select`
55 → 54.

On flash, `timer_create` resolved a task name and called `as_str()` on it
UNGATED — and its sibling gated the resolve but not the `as_str`. Turning even
a default `Name` into a `&str` reaches `core::str::from_utf8`, so a kernel built
with `NoTrace` was linking 534 bytes of UTF-8 validation it could never use.
Gating both, plus comparing name BYTES in `task_get_handle`, takes
`from_utf8` out of the binary entirely.

### Where the rows stand

| row | C | before | after | |
|---|---:|---:|---:|---|
| flash `.text` | 13,924 | 16,942 | **13,986** | **1.00×** |
| per timer, RAM | 40 B | 56 B | **40 B** | **1.00× — parity** |
| scheduler selection | 27 | 55 | **53** | 1.96×, floor 46 |

The flash pin still fails by **204 bytes** (closed in §9). `timer_command` (1,150 B) and
`new_mutex` (1,046 B) are the two largest functions; neither yielded to
outlining, and what is inside them has not been decomposed.

## The order of work

1. ~~**Wire the five benches to a gate.**~~ **DONE** — `kairos check --bench`.
   (§ *The instruments were not being run*)
2. **Static RAM** — reconcile the 696-byte residue, then the 136 B timer.
   (§1)
3. **Flash** — bisect the ~15 KB. (§2)
4. **Tick ISR** — 3.73×, and it also moves the preemptive switch. (§3)
5. **Queue send/receive** — build the missing row; the harness exists. (§5)
6. **Refresh the stale READMEs** once 2 and 3 land, rather than twice.

---

## Method notes

- **Every ratio above is ours ÷ theirs**, so > 1.00 is against us.
- **RAM and flash** are measured on the target (`rv32imac`), never on the
  host: a host `usize` is 8 bytes and rv32's is 4, so a host figure measures
  the wrong machine.
- **Instruction counts are deterministic** (callgrind / `minstret` under
  `-icount shift=0`), so there is no noise floor and no interleaving needed —
  but they are *work*, not *time*, and this page never calls them speed.
- **A refuted approach, recorded so it is not rebuilt:** a host callgrind
  stand-in for the C arm. FreeRTOS's tick self-cost is 2.7 % of its inclusive
  and ours is 13.1 %, so the ratio measures how two codebases *factor* a tick,
  not what a tick costs. Work parity did not save it.
- Every number's run and method line: **`docs/LEDGER.md`**.


## §9 — flash reaches parity (2026-09-24)

`bench/kernel-flash`: **13,986 against 13,924 — 1.00×, 62 bytes.** One change.

Decomposing the linked map per method put `new_mutex` second at **876 bytes**
for a two-line body. `new_queue` is its own 652-byte symbol, so all 876 were
`queue_send_generic` inlined — a send to a queue created on the line above,
with constant `value`, `ticks` and `Position`. The blocking half, the
queue-set announcement and the receiver wake are all structurally unreachable
there, and LLVM cannot prove it because `account_for_allocation`,
`enter_critical` and `trace.event` each take `&mut self` between the
`try_insert` and the `resolve`.

`Kernel::prime_mutex` writes that path directly. `new_mutex` **876 → 482**,
kernel **14,520 → 13,986**.

**It is not free.** Three of seventeen rv32 rows moved up — `queue_roundtrip`
155 → 158, `block_cycle` 1,185 → 1,187, `peek_ok` 79 → 80. Neither
`prime_mutex` nor `copy_data_to_queue` is a symbol in the linked arm and the
call census is unchanged, so the +6 is code layout. Taken because flash is the
scarce resource on the part and this is the row that reaches parity.

Gates: `conform --all` 26 identical, 101 kernel tests, clippy and fmt clean,
`tick-work` PARITY ok 1024/1024.

`timer_command` is now the largest function at **974 B** and has the same
shape — five task-side wrappers each calling it with a constant `Command`,
none of which can be `is_from_isr()`. Not yet decomposed.


## §10 — flash goes below the C kernel, and why the old number was wrong (2026-09-24)

```
  FreeRTOS kernel + RISC-V port        13924
  Kairos kernel + RISC-V port          13318      0.96x
  Kairos costs 606 bytes LESS flash for the same operation set.
```

Two separate things got us here, and only one is a kernel improvement.

**A kernel change, -534 B (§9).** `prvInitialiseMutex`'s give stopped going
through the general `queue_send_generic`; see `Kernel::prime_mutex`.

**An instrument fix, -668 B — and this one was a defect, not a win.**
`bench/kernel-flash` compiles its C arm with `-include
bench/kernel-ram/c/FreeRTOSConfig.h`, and the Rust arm was linking
`PosixDemoConfig`. **They disagreed in nine places.** Measured one at a time:

| the C header says | delta to our arm |
|---|---:|
| `configUSE_QUEUE_SETS 0` | **-570** |
| `configTASK_NOTIFICATION_ARRAY_ENTRIES` 1 (we had 3) | **-62** |
| `configUSE_TIME_SLICING 0` | **-34** |
| `configMAX_PRIORITIES 5` | **+10** |
| five others (`USE_TICK_HOOK`, `CHECK_FOR_STACK_OVERFLOW`, `QUEUE_REGISTRY_SIZE`, `MAX_TASK_NAME_LEN`, `TIMER_QUEUE_LENGTH`) | 0 |

The probe now declares `MatchedConfig` against that header field by field, as
the rv32 instrument always has. Its comment is the rule this bench was
breaking: *"A comparison at two different geometries is not a comparison."*

**Two things a reader is owed, both against us.**

1. `configCHECK_FOR_STACK_OVERFLOW 2` reads flash-NEUTRAL because our kernel
   does not implement stack overflow checking. The C arm compiles a check we
   have no code for. **A feature gap in our favour that no config can close.**
2. The bench's long-standing claim that "part of the gap is a 64-bit tick,
   including the 524 bytes of `compiler_builtins`" was never measured and is
   **false**. `Tick = Bits32` reads 13,288 against 13,318: the u64 tick costs
   **30 bytes**, and its helpers were already inside the `compiler_builtins`
   excluded from both arms — so that 524 was being double-counted. The u64 tick
   stays; the paragraph now prints the measured 30.

### The instruction rows, same day

| row | before | after | |
|---|---:|---:|---|
| `block_cycle` | 1,187 | **1,075** | -112 |
| `peek_ok` | 80 | **45** | -35 |
| `queue_roundtrip` | 158 | **131** | -27 |
| `recv_empty` | 62 | **41** | -21 |

**-193 instructions at no flash cost.** The cause was a live range, not a copy:
`queue_take` was copying an eleven-field descriptor to read five, and its
three-deep cold chain took the whole thirty-six bytes by value to read `kind`
twice. Letting the cold callee resolve `kind` itself — so the hot path never
computes it — is what took the last 35 off `block_cycle` and gave `peek_ok`
back. This closes the ledger's standing open question about the snapshot
costing 2x: the earlier attempts swapped the copy for a borrow, which does not
shorten a live range.


## §11 — every per-unit RAM number was a doubling divided by the unit count (2026-09-24)

`bench/kernel-ram` took each dimension's slope across a **doubling** — 8 to 16
tasks, 8 to 16 queues, 16 to 32 timers, 2 to 4 groups — and `list_slots_for`
ends in `next_power_of_two()`. Every one of those spans crosses a 64 to 128
slot step, so dividing by the unit count charged a share of a one-off step to
every unit.

| dimension | marginal | the bench printed | inflation | C |
|---|---:|---:|---:|---:|
| a task | **136 B** | 264 | 94 % | 84 B TCB + 512 B stack = 596 |
| a queue | **56 B** | 184 | 228 % | 72 B |
| a timer | **40 B** | 104 | 160 % | 40 B |
| an event group | **8 B** | 12 | 50 % | 28 B |

Corrected, the rows read: task **0.23×**, queue **smaller than C**, timer
**parity**, group **3.5× smaller**.

**This had already been found.** §7 diagnosed exactly this for timers in
September, built a 16 → 17 probe inside one band, and published **40 B**. The
bench was never changed, so it went on printing 104 — the number CI checked —
while this scorecard quoted 40. The diagnosis landed in prose and not in the
instrument.

**What landed now**, beyond the numbers: one-unit neighbours for every
dimension, and a **structural counter beside the bytes**. Each geometry exports
its list-slot count and the bench refuses a slope whose count moved:

```
      ok    task slope has no granularity step in it (64 slots)
      ok    queue slope has no granularity step in it (64 slots)
      ok    timer slope has no granularity step in it (64 slots)
      ok    event group slope has no granularity step in it (64 slots)
```

The timer reading exactly 40 — reached here by a different pair of geometries
than §7 used — is the check that the method is right.

**The counterweight, because all four corrections ran our way.** The step is
REAL memory: the list arena genuinely rounds up and crossing a band genuinely
costs it. It is simply not a *per-unit* cost — it is paid once, at the
boundary. The bench now prints both, so sizing a part uses the marginal and
budgeting a geometry uses `FOOTPRINT` at that geometry, which is exact.


## §12 — ★★★ the flash row was never measuring the kernel (2026-09-24)

**This retracts §7's "the 2.21× was stale". It was not stale; it was roughly right.**

`bench/kernel-flash/rs` roots one `extern "C"` entry per operation, and every
one taking a handle passed `Default::default()` = `{index: 0, generation: 0}`.
`Arena::resolve` tests `slot.generation != handle.generation() ||
!live(slot.generation)`; with a constant generation of **0** that is
`g != 0 || g is even` — **true for every `g`, because 0 is even.** LLVM proved
the resolve always fails and deleted the operation behind it.

`kairos_stream_buffer_send` compiled to **two bytes**, and the linker folded
`..._receive` onto the same address because both had become the same early
return.

| folded by | bytes |
|---|---:|
| constant handles (23 ops) | **13,588** |
| constant block times | 1,340 |
| constant control-flow flags | 206 |

The C arm folds none of it: `-u xQueueReceive` roots the real function with
every parameter unknown.

The probe's own doc comment held the corroboration: *"The first version ...
called every operation from a single function ... 13,314 bytes were attributed
to the probe."* That is this 13,588 — splitting per operation is what turned
the handles into constants.

**Honest baseline 28,452. After ten kernel wins: 24,832 — 1.78×, and the row
FAILS its ≤ 1.30× target.**

### The ten wins

| # | change | effect |
|---|---|---|
| 1 | `prime_mutex` (§9) | −534 B |
| 2–4 | `queue_take` live-range campaign (§10) | −193 Ir |
| 5 | `Name::new` zip, not `copy_from_slice` | **−1,704 B** — all `core::fmt` leaves |
| 6 | A4 split on `queue_send_generic` | **−986 B**, no row moved |
| 7 | `const PEEK` → runtime flag | −28 B |
| 8 | A4 split on `resume_all` (21 sites) | **−808 B**, no row moved |
| 9 | loop-invariant resolve in `event_group_set_bits` | −26 B, −3 Ir |
| 10 | two cold event sites → shared `resume_all` | −68 B |

Win 5 is the one worth repeating elsewhere: `copy_from_slice`'s
length-mismatch arm panics with two formatted integers, which links
`pad_integral`, `Display for usize` and `do_count_chars` — **1,052 bytes of
formatter in a `no_std` kernel, for a branch that cannot be taken.**

### The law

**A probe that supplies its own arguments is part of the optimiser's input.**
Rooting a symbol does not keep its body; it keeps whatever survives constant
propagation from the call site you wrote. The tell was printed every run — two
different operations with the same size, and a root of two bytes. **Read a size
census for the entries that are impossibly SMALL, not only the large ones.**


## §13 — phase two: five wins with queue sets off (2026-09-24)

`USE_QUEUE_SETS = false` is what the C header sets, so it is what the matched
config carries. **On the honest instrument it is worth 676 B** (24,832 off vs
25,508 on) — it read 570 on the folded probe.

**24,832 → 24,572, and `block_cycle` 1,075 → 1,067.**

| # | change | effect |
|---|---|---|
| 1 | `ListsOf::next_and_value` — one lookup for a node's next AND value | **−136 B** |
| 2 | four `container(item)?.is_some()` guards before `remove(item)` folded | **−62 B** |
| 3 | `is_null` + `contains` + `resolve` → one resolve, twice | **−28 B** |
| 4 | the lock-cap compares stop being 64-bit on a 32-bit target | **−34 B** |
| 5 | `queue_take_locked`'s two arms share one unlock-and-resume tail | **−6 Ir** |

Win 3 reuses the tautology §12 uncovered: `resolve` fails for the NULL handle
too, so `is_null()` was asking what it answers.

### The rule that came out of it

`list.rs` already carried four campaigns of record, and reading it both saved a
build and explained the win. Its law: the only shape that pays there **deletes
a function body carrying its own branch and its own `Result`** — because
`&mut self` is `noalias` and LLVM has already shared the plain reads.

Three siblings, measured, pin it down exactly:

| pair | what stands between | result |
|---|---|---|
| `next` then `value` | an early-returning `NotActive` test | **−136 B (win)** |
| `head` then `value` | nothing | byte-identical |
| `value` then `set_value` | a `&mut self` WRITE | +8 B |

**What blocks CSE is not `&mut self`, and not even a write. It is an early
return.**

### The control that validates §10

Narrowing the stream-buffer ISR snapshots — the identical change that won −193
instructions on `queue_take` — measured **+4 B**. Every read there precedes the
first `&mut self` call, so the copy was already promoted. That is the live-range
theory's control arm: the win exists only where the value must survive such a
call.

### An instrument this box cannot run

`rusty_rtos_core/bench/list-ir` prices `list.rs` on six arms (x86-64 and i686 ×
three widths) and its docs call the i686 arm "the binding constraint... A
host-only harness would have shipped all three" rejected changes. **All six
SKIPPED** — WSL lacks `i686-unknown-linux-gnu` and `gcc-multilib`. Win 1 is
gated on rv32 flash and the rv32 rows, which is a real 32-bit target rather
than the proxy, but not on the instrument its own crate nominates.


## §14 — ★★★ the flash gap is instruction COUNT, and 8% of it was the toolchain (2026-09-24)

**Prediction, written first:** the gap would be stackless-resume machinery
(~1,900 B), arena resolves (~3,000), and `Result<Wait<T>>` returned through
memory (~2,000). Two of three were wrong and the biggest cause was not in the
kernel.

**Units first.** Both arms encode at the same density — C 2.74 bytes per
instruction, ours 2.78. So this is an instruction-**count** gap, not an encoding
one, and every hypothesis must explain instructions.

### The finding

`auipc`+`jalr` is an 8-byte call pair that the linker collapses to a 4-byte
`jal` when the relocation carries `R_RISCV_RELAX`. **clang emits that by default
(`-mrelax`); rustc does not.**

| | C | Kairos |
|---|---:|---:|
| `auipc` + `jalr` (call) | **0** | **391** |
| `auipc` + `jr` (tail call) | **0** | **66** |

`-C target-feature=+relax`: **24,242 → 22,264 B**, and 8,715 → 8,226
instructions. Symmetric — the C arm rebuilt with `-mno-relax` reads 15,022
against its 13,924, so relaxation is worth −7.3% to it and −8.2% to us:

| | C | Kairos | ratio |
|---|---:|---:|---|
| both relaxed | 13,924 | **22,264** | **1.60×** |
| both unrelaxed | 15,022 | 24,242 | 1.61× |
| *as published* | 13,924 | 24,242 | 1.74× |

**Caveat:** `relax` is an *unstable* rustc target feature. Every Kairos rv32
firmware carries that 8% today and a stable toolchain cannot switch it off.
That is a rustc finding, not a kernel one.

### What the rest is

| class | C | Kairos | ratio |
|---|---:|---:|---|
| ALU | 849 | 1,977 | 2.33× |
| **MOVE** | 344 | 1,027 | **2.99×** |
| LOAD | 1,135 | 1,638 | 1.44× |
| BRANCH | 640 | 960 | 1.50× |
| **`zext.b`** | 2 | 63 | **31.5×** |
| TOTAL | 5,089 | 8,226 | **1.62×** |

Inside ALU: `mul` 10.7×, `srli` 16.3×, `andi` 8.1×, `slli` 4.8×. That is
**index-based access against pointer-based access** — C does one `lw` at a fixed
offset from a register; we compute `base + index * size_of::<Slot<T>>()` first.
MOVE 2.99× is the register pressure that follows, and `zext.b` 31.5× is our
byte-sized fields where C uses word-sized `BaseType_t`. It is diffuse: 1–2 `mul`
and 4–12 `slli` in every arena-touching function, with no hotspot.

**The 12-bit displacement hypothesis (§ the biggest prior finding) is refuted:**
we emit 56 `lui` against the C arm's 187. The field-ordering fix worked.

### Priced and rejected — the structural options, with numbers

| option | flash | instructions | verdict |
|---|---:|---|---|
| shrink `Result<Wait<u64>>` below the 8-byte register limit | **−34** (upper bound) | — | the sret is not the cost |
| outline `copy_data_to_queue` (hold a handle, as C does) | −354 | `queue_roundtrip` **+126** | inline is earned |
| outline the send's blocking half | +34 | `send_full` **+27** | registers didn't drop; the "cold" arm is a measured row |
| power-of-two arena slots, `align(64)` | −298 | −24 | **+1,488 B RAM**, per-timer parity lost, per-group 0.29×→2.29× |
| mask the arena index (the `list.rs` trick) | +444 | +7 | as an `if`: **+3,660 B**, phi defeats the proof |
| `--icf=all`, link `-O2`, both arms | 0 | 0 | nothing |
| `overflow-checks = true` | −12 | — | **H-05 hardening is free here** |

### Two corrections, both against us

The family census credited us the C's `heap_4.c` as "we have no allocator" —
**we have two**, inlined into the constructors. And per-family ratios are
contaminated: the C's `event_groups.c` excludes the task/list machinery it
calls, while our inlined equivalents are charged to the caller. **Only the total
and the per-symbol census are sound.**

### The law

**A cross-language size comparison must diff the TOOLCHAIN before the code.**
This bench argued its root set, its `--gc-sections` control and its
`compiler_builtins` exclusion at length, and underneath it one arm's calls were
relaxed and the other's were not — 8% of the number, invisible to every
per-function census. **Build an opcode histogram of both arms before attributing
a size gap to anything you wrote.**

---

## §16 — the `mv` gap is argument setup, and five wins took flash to 1.50× (2026-09-24)

`mv` read 976 against C's 344 and had been labelled "register pressure". The
per-function attribution (sum-checked against the pinned opcode counts, so a
fabrication could not pass) says otherwise:

|                                |  Rust |    C | ratio |
|--------------------------------|------:|-----:|------:|
| `mv` total                     |   976 |  344 | 2.84× |
| real calls (`jal`/`jalr`)      |   387 |  234 | 1.65× |
| `mv` immediately before a call |   552 |  157 | 3.52× |
| **`mv` per call**              |**1.43**|**0.67**|**2.13×**|

**57% of our `mv` is call-argument setup**, and in the three functions holding
the most it is 109 of 146 moves in the `saved->arg` direction — a value parked in
a callee-saved register and re-supplied as an argument. `queue_take_blocking`
makes **26 calls**; 48 of its 56 `mv` are the setup for them. Not spilling.

### What the gap is made of

Composition of the 445 setup moves in the FINAL binary (853 `mv`, 311 calls,
1.43 per call — the ratio did not budge, because the wins removed whole calls
rather than arguments):

- **183 `mv` are a single `mv a0, sN`** — the `&mut self` receiver, one per call.
  Irreducible in rv32: `a0` is caller-saved, so a function making 26 calls must
  re-materialise the receiver 26 times. The only lever on this class is a lower
  CALL COUNT.
- **82 `mv` are in 12 wide-argument calls** (width ≥ 6). `begin_wait` takes
  `(&mut self, TaskHandle, QueueHandle, u64) -> Result<Option<u64>>` = eight
  argument registers in ilp32, sret included.
- The rest is the width-2..5 middle.

### Twenty-two wins — flash 21,818 → 19,370 (1.60× → **1.39×**)

| # | change | flash | `mv` | instructions |
|---|---|---:|---:|---:|
| | baseline | 21,818 | 976 | 8,042 |
| 1 | `#[inline]` on `Port::exits` | 21,364 | 922 | 7,823 |
| 2 | `#[inline]` on `Arena::why` | 21,246 | 878 | 7,772 |
| 3 | `T::EMITS` gate on `note_exits` | 20,952 | 870 | 7,693 |
| 4 | `ListsOf::unlink` (no `sret`) | 20,918 | 869 | 7,671 |
| 5 | `#[inline]` on `wrap_next` | 20,832 | 853 | 7,622 |
| 6 | arena `FREE` bit replaces generation parity | 20,694 | 851 | 7,563 |
| 7 | `#[inline]` on 8 small `Port` primitives | 20,684 | 850 | 7,559 |
| 8 | `#[inline]` on 3 `const fn` helpers | 20,634 | 837 | 7,522 |
| 9 | `#[inline]` on the 3 derivation `const fn`s | 20,364 | 834 | 7,437 |
| 10 | `#[inline]` on `spaces_available` | 20,350 | 829 | 7,428 |
| 11 | `wait_set` out of the packed flags byte | 20,252 | 829 | 7,405 |
| 12 | `owes_yield` out of it too (the byte held one bit) | 20,204 | 829 | 7,393 |
| 13 | `end_wait` clears two bools, not a 30-byte frame | 20,010 | 829 | 7,318 |
| 14 | `Arena::discard` leaves the value (`needs_drop`-gated) | 19,932 | 828 | 7,292 |
| 15 | `MAX_PRIORITIES.saturating_sub` → `wrapping_sub` in `u32` | 19,882 | 828 | 7,277 |
| | (field reorder from a reverted experiment, layout drift) | 19,880 | 828 | 7,276 |
| 16 | `mask_interrupts` returns the raw `mstatus` bit | 19,878 | 828 | 7,275 |
| 17 | `begin_wait` is infallible — `Result` dropped | 19,784 | 828 | 7,250 |
| 18 | `Error` discriminants start at 1 — `Ok`'s niche moves 255 → 0 | 19,628 | 810 | 7,059 |
| 19–22 | **A3: the seventeen single-caller functions** (4 wins) | **19,370** | **788** | **6,945** |
| | **total** | **−2,448 B** | **−179** | **−926** |

Wins 19–22 are one vein: `rusty-compiler-leverage` A3, *a single-caller function
that is not inlined is paying a frame for nothing*. Every earlier sweep ranked
callees by BODY SIZE; none ranked them by CALLER COUNT, and a single-caller inline
duplicates nothing, so the body-size-versus-call-overhead arithmetic that dismissed
the vein five times never applied to it. Two findings out of it: the **larger**
single-caller functions win while the smaller ones lose (+68 B for three of them),
and **`#[inline]` is a hint LLVM declines** — `#[inline(always)]` on one already-
annotated function was worth −94 B on its own.

rv32 work rows against their pins: `block_cycle` **1003 → 984**, `queue_roundtrip`
125 → 122, `send_full` 38 → 36, `peek_ok` 43 → 41, `notify_take_empty` 31 → 29,
`recv_empty` 39 → 38 — **six rows down 29 instructions, none above its pin**, and
ANCHOR identical to the C arm. `notify_wait_empty`'s earlier 27 → 29 reads 27 again
here, which settles it as layout drift rather than cost.

> The per-win `mv` and instruction columns above are AS MEASURED, on the census
> that then included `compiler_builtins`. The flash column is unaffected (the byte
> total always excluded it). The DELTAS are basis-independent — `compiler_builtins`
> contributed exactly 9 `mv` at both ends of the campaign, so `mv` reads −148
> either way — but the absolute values in those two columns are 9 `mv` and about
> 170 instructions higher than the corrected table below.

**The opcode figures, corrected twice on 2026-09-25 and now apples-to-apples**
(our code only, `compiler_builtins` excluded to match what the byte total covers,
and `zext.b` counted as the `andi` it encodes):

| | baseline | final | C arm |
|---|---:|---:|---:|
| `mv` | 967 | **819** (−148) | 344 |
| `andi` | 174 | **143** (−31) | 31 |
| `slli` | 289 | **265** (−24) | 56 |
| `srli` | 78 | **76** | 8 |
| **`mul`** | **0** | **0** | **3** |
| instructions | 7,871 | **7,104** (−767) | 5,089 |

**`mul` was always zero in our code** — the pin read 1 for the whole campaign and
that multiply is inside `compiler_builtins`. The `Tcb` stride pad achieved its aim
completely: no multiply anywhere in the kernel, against C's three.

Those are the TRUE counts. Until 2026-09-24 the pin read 133 → 108, because
`zext.b` **is** `andi rd, rs, 0xff` (encoding `0ff57593`: OP-IMM, funct3 111,
imm 0x0FF) and llvm-objdump prints the pseudo, so a census keyed on the mnemonic
missed 49 of ours and 2 of C's. The deltas quoted earlier all stand — the wins
removed `andi 0x4`, `0xfb`, `0x2` and the parity `andi 0x1`, which were counted
correctly — but the totals were understated and the ratio with them: **5.1×, not
3.7×**. The gate now folds the pseudo into what it encodes.

Two levers were then measured and declined, both recorded with numbers in the
ledger: widening `ListId` to `u16` (free in RAM, **`andi` −20** — and `srli`
**+18** with flash +40 B, a displacement), and routing `queue_peek` out of line
(**−414 B, `mv` −25**, against `peek_ok` 41 → 98).

Every one measured on all five pinned opcodes plus total instructions plus flash,
so a displacement cannot read as a win. `andi` moved 133 → 135 (+2) across the
set; nothing else moved except where noted in the ledger.

Gates: **conform 26/26**, core **63/0** (the 62 plus a new exhaustive
forged-handle test), kernel **101/0**, port **28/0**.

And the rv32 work instrument improved, which was not the plan. All 17 rows, ANCHOR
identical to the C arm, PARITY and POISON ok: `block_cycle` **1,003 -> 991**,
`notify_take_empty` 31 -> 29, `queue_roundtrip` 125 -> 124, `send_full` 38 -> 37,
`peek_ok` 43 -> 42, the other twelve unchanged. **Five rows down, none up.**
Win 6 removed `Handle::from_raw`'s parity normalisation, which ran at every API
entry point — a static-size lever and a runtime lever coincide when the removed
code sits on the ENTRY PATH.

### The three transferable findings

1. **`lto = false` makes small cross-crate bodies UNINLINABLE, not merely
   un-inlined.** Without `#[inline]` the body is not a candidate at all. Two
   three-instruction functions were sitting behind 45 and 18 calls.

2. **A no-op sink deletes the CALL, not the ARGUMENT.** All 34 sites spelled
   `self.trace.note_exits(self.port.exits())`; with `NoTrace` the body folds away
   but `Port::exits` is an **atomic** load, which LLVM may not delete even
   unused. A tracing-off kernel was running 34 loads of a counter no sink reads.
   Gate at the call site on the sink's capability const.

3. **Static inlining predictions underestimate, systematically.** `Port::exits`
   predicted −48 instructions, measured −219. `wrap_next` predicted a LOSS,
   measured −49. Both misses are the same mechanism: the prediction prices the
   call and misses the folding at the site. **Measure inlining candidates; do not
   rank them.**

4. **★ A `const fn` is NOT automatically folded.** Without `#[inline]` it is a
   SYMBOL, and a call to it with compile-time-constant arguments stays a call.
   `lists_for`, `items_for` and `list_slots_for` are pure arithmetic over
   associated consts and const generics, and three sites were calling them at
   RUNTIME. Three attributes: −270 B, −85 instructions, every pinned opcode down
   or flat. `codegen-units = 1` is not enough — LLVM will not fold a symbol it
   was not asked to inline.

5. **Ten wins, no displacement — and five refutations that moved `mv` alone.**
   Inlining the critical pair (−165 `mv`), `exit_critical` alone (−70),
   `add_task_to_ready_list` (−9), narrowing `Handle` (+9), rounding the list
   count. **A campaign scored on `mv` would have banked −244 `mv` and +3,300 B
   of flash.** All five opcodes plus total instructions plus flash, on every
   probe, is what separated the two sets.

### Why this row will not reach 1.30× by this route

The two large structural levers both measured as losses (full numbers in the
ledger): inlining the critical-section pair is **−165 `mv`** and **+1,866 B**,
and narrowing `Handle` back to four bytes is **+1,160 B** with `mv` going the
wrong way by 9. The remaining small-callee candidates are all 1–4 call sites.

And the `andi` half of the target has a trap in it: 19 of our `andi` are `at()`'s
hand-written `& (N-1)` mask, which **replaces** a compare, a branch and a panic
edge. C's lower count is C doing no bounds checking. Driving that class down
makes the binary bigger, so 19 instructions of the 104-instruction `andi` gap are
`forbid(unsafe)` safety bought at the cheapest price the ISA offers, and should
be reported as such rather than chased.

---

## §17 — ★★★ the row's fairness basis and its build disagree by 4,154 bytes (2026-09-25)

`bench/kernel-flash/rs/Cargo.toml` carried a comment saying `lto` **and one
codegen unit** were "left OFF on purpose ... LTO would give the Rust arm a
whole-program optimisation the C arm is not getting" — above the line
`codegen-units = 1`.

| `codegen-units` | flash | ratio | `mv` | instructions |
|---|---:|---:|---:|---:|
| **1** (pinned) | **19,878** | **1.43×** | **828** | 7,275 |
| 16 (the default) | 24,032 | **1.73×** | 1,079 | 8,788 |

**One codegen unit is worth 4,154 B — 21% of the row — against 1,940 B for every
optimisation win in the campaign put together. The build flag is worth more than
twice the work.**

Same class as §14's linker relaxation, opposite direction: §14 favoured the C arm
and was corrected by matching it; this favours ours and is a decision about what
the row compares, not a bug. A crate is Rust's compilation unit as a `.c` file is
C's — but `rusty_rtos_kernel-core` holds the equivalent of SIX FreeRTOS
translation units, and at one unit they are optimised together while C's six never
are. `codegen-units = 16` does not reproduce C's boundaries either; it splits one
crate wherever rustc likes.

**Left at 1, with the asymmetry and its size written at the setting.** Whoever
settles the K3 row should state which comparison it claims and put that beside the
ratio. Until then, 1.43× is the *whole-crate-optimised* figure and 1.73× is the
per-unit one.

---

## §18 — WHERE the flash goes, primitive by primitive (2026-09-25)

Every measurement before this one was by opcode or by symbol. This is the first
comparison of the two arms by PRIMITIVE, pairing each FreeRTOS function with the
Kairos functions that do its job — because our A4 splits put one C function's work
across a wrapper, an outlined handle and a blocking path.

Mapping covers **98% of C's `.text`** (13,396 of 13,630) and **97% of ours** (19,244
of 19,814); the unmapped remainder is small accessors on both sides.

| primitive | C | Kairos | excess | ratio |
|---|---:|---:|---:|---:|
| **queue TAKE** (recv/peek/sem-take) | 1,512 | 3,236 | **+1,724** | 2.14× |
| **queue SEND** | 716 | 2,176 | **+1,460** | **3.04×** |
| **queue CREATE/RESET/UNLOCK** | 776 | 1,752 | **+976** | 2.26× |
| event groups | 778 | 1,810 | +1,032 | 2.33× |
| stream buffers | 1,474 | 2,138 | +664 | 1.45× |
| TICK | 308 | 584 | +276 | 1.90× |
| event lists / delayed list | 968 | 1,224 | +256 | 1.26× |
| **LISTS** | 126 | 374 | +248 | **2.97×** |
| scheduler (switch / resume-all) | 726 | 892 | +166 | 1.23× |
| priority inheritance | 514 | 574 | +60 | 1.12× |
| task lifecycle | 2,696 | 2,646 | **−50** | **0.98×** |
| notifications | 1,064 | 1,264 | +200 | 1.19× |
| **timers** | 1,092 | 574 | **−518** | **0.53×** |
| **heap vs static arenas** | 646 | **0** | **−646** | — |
| **mapped total** | **13,396** | **19,244** | **+5,848** | **1.44×** |

### The answer in one line

**The queue subsystem is 76% of the entire gap.** SEND + TAKE + CREATE/UNLOCK is
**+4,432 of the +5,848** excess. Everything else together is +1,416, and four
primitives are at or better than parity.

### And 37% of the queue gap is our own deliberate duplication

The A4 splits put the same logic in several places:

```
SEND, two copies:   kairos_queue_send 638  +  send_generic_outlined 788
TAKE, three copies: kairos_queue_receive 554 + kairos_queue_peek 468 + take_outlined 668
```

One copy each would be ~788 and ~668, so **~1,660 bytes are deliberate duplication**
— 37% of the queue gap and 28% of the whole excess. It is priced: dropping the send
hint recovers ~1,666 B and costs `send_full` +40, `queue_roundtrip` +66,
`block_cycle` +50; routing peek out of line recovers 414 B and costs `peek_ok`
41 → 98.

**So the flash row is not uniformly behind C — it is behind on ONE subsystem, and
over a quarter of that is a speed-for-size trade already made on purpose.**

### Where we win, and why it matters for the target

- **No heap at all: −646 B.** C needs `pvPortMalloc` + `vPortFree`; static arenas
  need neither.
- **Timers 0.53×** — half C's size, the largest proportional win anywhere.
- **Notifications 0.93×** and **task lifecycle 0.98×** — genuine parity on two large
  subsystems, which is the proof the 1.44× is not a uniform language tax.
- **LISTS at 2.97×** is the worst ratio after SEND but only +248 B — a bad ratio on a
  small base, so it is the wrong place to spend time despite looking alarming.

### Where to spend next

1. **queue SEND (3.04×, +1,460)** — worst ratio and second-largest excess. Of it,
   ~638 B is the A4 duplicate whose price is known.
2. **queue TAKE (+1,724)** — largest excess; ~1,022 B is duplicate across three
   copies, and the peek third is already priced at −414 B.
3. **queue CREATE/RESET/UNLOCK (2.61×, +1,248)** — `new_queue` 630 and `new_mutex`
   424 against C's `xQueueGenericCreate` 98 + `xQueueGenericReset` 152. Cold paths,
   so flash-only with no runtime cost to trade — **the most attackable of the three.**
4. **event groups (2.33×, +1,032)** — `kairos_event_group_wait_bits` 702 and
   `event_group_set_bits` 560 against 280 and 162.

Everything outside queues and event groups is +1,416 total, so effort spent there
cannot move the row much whatever the ratio looks like.
