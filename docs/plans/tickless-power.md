# Tickless power — mission plan

## 0 · Mission and promise

> Give a Kairos device the power saving of tickless idle, **and the diff proving
> it did not move the schedule.**

**The one-line test for every change:** *could a firmware engineer turn this on,
watch their battery current drop, and run one command that proves nothing about
task scheduling changed?*

## 1 · Where we are (2026-09-19)

| piece | state |
|---|---|
| `Config::EXPECTED_IDLE_TIME_BEFORE_SLEEP` | exists, `rusty_rtos_core/config.rs` |
| `Config::USE_TICKLESS_IDLE` | **done, M2** — off by default |
| `Port::suppress_ticks_and_sleep` | **done, M2** — returns the ticks it slept; 0 declines |
| `Hooks::pre_suppress_ticks` | signature only — the kernel has no `Hooks` seam, so the veto is **not wired**. A port declines by returning 0 |
| `Hooks::pre_sleep` / `post_sleep` | signatures only — they belong in a port implementation, as they do in the C |
| `trace::Scheduling` + `is_suppressible` | **done, M1** — `rusty_rtos_core`, 3 tests, poison-proved |
| `kairos power idle` | **done, M0** — ceiling probe over the stored traces |
| `kairos power diff` | **done, M1** — projection over the stored traces, 18/18 invariant |
| kernel-side suppression | **done, M2** — `expected_idle_time`, `step_tick`, `idle_suppress_ticks` |
| a port that sleeps | **done, M3a** — `rusty_rtos_port-cortex-m` really reprograms SysTick. The **sim port does not**, see M2a |
| a cell that runs it | **done, M3a** — `mps2-an385-qemu-tickless`, 401 wakeups → 0 |
| on-board energy measurement | **not built** — needs hardware, and on the S3 needs a tick-driven kernel cell first (M3b) |

## 2 · Strategy that does not change

- **The default is FreeRTOS, byte-identical.** Tickless is opt-in and off unless
  asked for. A build that does not enable it passes the existing 18-scenario
  differential unchanged.
- **One dependency direction.** `rusty_rtos_power` → `rusty_rtos_core`. Never the
  reverse. The kernel never learns what a battery is.
- **REMAKE / WRAP / NEW, stated per piece.** The *mechanism* (suppress, sleep,
  wind the tick count forward) is REMAKE — FreeRTOS specifies it and it gets a
  differential. The *policy* (which depth, how much margin) is NEW and is the
  only part no differential covers.
- **Two gates, named separately, never quoted as one another.** Mechanism →
  trace projection. Policy → energy on named hardware.
- **The matrix rule.** Every number states chip, clock, workload and arm.
- **Non-goals:** SMP, DVFS, peripheral power domains, and anything that changes
  *when* a task runs.

## 3 · The product surface

```rust
use rusty_rtos_power::{Tickless, Depth};

let hooks = Tickless::new()
    .max_depth(Depth::Standby)   // deepest state that preserves what you need
    .margin_ticks(2);            // wake-up latency budget
```

Two verbs, both shipped:

```
kairos power idle [SCENARIO] [--min-sleep N]
kairos power diff [SCENARIO] [--show]
```

## 4 · Finished work

### M0 — the ceiling probe (2026-09-19)

`kairos power idle`, over the 18 pinned oracle traces:

```
ALL     38,011 ticks    idle 24,193 (63.6%)    sleepable 19,116 (50.3%)
```

**The corpus is bimodal, and that is the finding.**

| | scenarios | sleepable |
|---|---|---|
| sleepable | `death` 99.9%, `StreamBufferInterrupt` 99.4%, `MessageBufferAMP` 99.1%, `AbortDelay` 99.0%, `PollQ` 98.8%, `blocktim` 98.7%, `IntSemTest` 98.1%, `TimerDemo` 97.0%, `QPeek` 65.7% | windows up to **100 ticks** |
| not | `BlockQ`, `EventGroupsDemo`, `GenQTest`, `QueueOverwrite`, `QueueSetPolling`, `countsem`, `dynamic`, `recmutex`, `semtest` | **0** — they idle, up to 52.6%, but never for two consecutive ticks |

So **tickless idle is a property of the workload, not a tuning knob.** Half of
this corpus can sleep through half of all its ticks; the other half cannot sleep
at all, however good the policy. That is a product insight before it is an
engineering one: a customer needs to know which half they are in, and `power
idle` answers it before they change a line of firmware.

Note the corpus is a *conformance stress test* — it exists to exercise the
kernel. A real product idles far more, so 50.3% is a floor, not a forecast.

### M1 — the projection (2026-09-19)

`rusty_rtos_core::trace::Scheduling<T>` wraps any sink and passes on every event
except the three `is_suppressible` names: `TaskIncrementTick`,
`LowPowerIdleBegin`, `LowPowerIdleEnd`. Everything else — every switch, list
move, queue, event-group and timer operation, with its tick stamp — goes through
untouched.

`kairos power diff` applies the same rule to a trace file and **demonstrates**
the invariance rather than asserting it: strip every heartbeat line, project
again, compare. **18 of 18 scenarios invariant.**

Eight tests across the two crates. Two are the claim and its poison:

- the projection does not move when the heartbeat goes;
- it does move when a context switch goes.

Poison-proved on both sides by widening the suppressible set to include
`TASK_SWITCHED_IN` — two tests fail in each crate.

Additive: kernel-ir unchanged at 119,461,182.

### M2 — the mechanism (2026-09-19)

Three pieces, each the C's:

* `expected_idle_time` = `prvGetExpectedIdleTime`;
* `step_tick` = `vTaskStepTick`, leaving the **last** tick pended rather than
  stepped so `increment_tick` wakes the delayed task through the same code
  that would have woken it — the detail the whole invariance rests on;
* `idle_suppress_ticks` = the `configUSE_TICKLESS_IDLE` block of
  `prvIdleTask`, double-sample and all.

Five tests. The fifth is the claim, and it is M1 pointed at a kernel that
really suppressed its ticks rather than at a trace file standing in for one:
two kernels identical but for the config, the same script, **traces that
differ and schedules that do not**.

Poison-proved twice — stepping the last tick instead of pending it fails the
invariance test, and trusting a port that oversleeps by 4x fails the clamp.

Off by default, so nothing moved: the 18-scenario differential is unchanged,
khot-ir / ksched-ir / kipc-ir byte-identical, kernel-ir +2,184 (0.002%) of
layout drift.

**Two gaps named rather than hidden.** The application veto
(`configPRE_SUPPRESS_TICKS_AND_SLEEP_PROCESSING`) is not wired, because the
kernel holds no `Hooks`; a port declines by returning zero instead. And
`system!` sizes `QUEUES` with no spare, so `System::build` and a started
scheduler cannot both have the timer daemon's slot — the tickless tests
create their one task by hand and say why.

### M3a — the mechanism on silicon (2026-09-19)

`vPortSuppressTicksAndSleep` for ARMv7-M, and the cell that runs it. This is
the first time in the mission a timer was actually reprogrammed: everything
before it either read stored traces or asked a test double to return a number.

`mps2-an385-qemu-tickless`, one feature flag the only difference between arms:

| | control | tickless |
|---|---:|---:|
| logical ticks | 401 | 400 |
| **SysTick wakeups** | **401** | **0** |
| projected events | 193 | 193 |
| context switches | 62 | 62 |
| **schedule digest** | `94f229d7a5bd8f77` | `94f229d7a5bd8f77` |

**Wakeups are a proxy, named as one.** QEMU has no power model. They are
exactly countable, deterministic, and what idle energy is proportional to on
a part that idles in `wfi` — so they carry the *mechanism* claim, and the
energy claim still needs a board and a shunt.

#### ★ The finding: the simulator's gate does not transfer to silicon

`kairos power diff` compares whole projected **lines**, tick stamp included,
and reports 18/18. That is sound for the oracle traces, where time is
critical-section exits and a tick stamp is a count of work done. **It is not
sound on a Cortex-M3**, where a tick stamp also records how long the work
took — and the two arms do not take the same time, because one spends the
idle windows in `wfi` instead of servicing four hundred interrupts.

Editing the cell twice while building it moved the logical tick count each
time, in *both* arms — control 402, 401, 400; tickless 400, 400, 401 — and
moved a tick-stamped digest with it. The order digest never moved.

> **A quantity the instrument's own cost can move is not a schedule.**

So the hardware gate is **order plus a timing band**, not a line diff. Both
halves are needed and the poison proves it: deleting the line in
`Kernel::step_tick` that leaves the last tick *pended* gave 421 ticks — every
lap one tick late — with the order digest **unchanged**, event for event. The
order digest cannot see a wake that is in the right sequence but late; the
band can.

`kairos power diff` keeps its own guarantee over stored traces. Section 8's
standing rule is unchanged and now merely the weaker of two reasons.

#### What M3a did not do

It did not choose a policy — the sleep is `expected_idle_time` less nothing,
the fixed policy the C ships. And it is not the XIAO; see M3b.

Additive: port crate 7/7, kernel 39/39, `mps2-an385-qemu-preempt` still
200/200 with the window closed.

### M3b — the XIAO ESP32-S3 (2026-09-19)

Flashed and run on a XIAO ESP32-S3 rev v0.2 (8 MB flash, 40 MHz crystal, MAC
68:ee:8f:51:74:64). Both claims hold:

| | control | tickless |
|---|---:|---:|
| logical ticks | 400 | 400 |
| **alarm wakeups** | **400** | **0** |
| projected events | 195 | 195 |
| context switches | 63 | 63 |
| **schedule digest** | `ebb908b74bccb99e` | `ebb908b74bccb99e` |

**Claim 1 is a repo first:** the Kairos kernel driven by a *tick interrupt*
on an Xtensa part. Every other kernel-driving cell is Cortex-M under QEMU,
and the two existing XIAO cells prove the port's switch and stop there. That
is the K3 prerequisite, met inside this mission.

**Claim 2:** every one of those interrupts suppressed, schedule unmoved.
Diagnostics: 20 sleeps, 400 ticks asked, 400 slept, last window 20,014 us
against a 20,000 us request — elapsed time read off the free-running counter
rather than assumed, which is what makes the mechanism survive swapping
`waiti` for a light sleep in M3c.

#### ★ The bug the board found, which building never would have

The first tickless run **failed**: 20 logical ticks where the arithmetic says
400. The cause is an Xtensa fact with no ARM counterpart —

> **`waiti 0` does not lower `PS.INTLEVEL` for the duration of the sleep. It
> SETS it to zero and leaves it there.** The waking interrupt returns through
> `RFI`, which restores the PS `waiti` installed, so the caller's critical
> section is gone from the wake onward.

Everything `idle_suppress_ticks` did after the sleep — `step_tick`,
`resume_all`, two trace events — therefore ran with interrupts open on a
kernel the idle task held `&mut` to. It did not crash. The scheduler quietly
stopped working and the worker never blocked. One line fixes it: re-raise the
mask the instant `waiti` returns. ARM needs none, because `wfi` leaves
PRIMASK alone.

**Two changes were made and only one mattered.** The switch handler was also
taught to decline while sleeping, on the theory `Software0` could be taken in
the open window; its counter reads **zero** across the run. Defence in depth,
not the cure, and labelled so in the source.

This is the mission's clearest argument for hardware: M3a's QEMU cell, the
kernel's unit tests, the section sizes and the disassembly all passed while
this defect sat in the code.

### M3c — the energy question, answered against a fair baseline (2026-09-19)

M3b counted wakeups. Wakeups are not energy, and the honest measurement needed
two things M3b did not have: a control arm that **halts the core** the way
FreeRTOS's idle task does, and a measure of how long the core actually ran.

Both arms now halt in `waiti` and accumulate time-halted off the free-running
SYSTIMER. **The headline inverts.**

| | control (`waiti` per tick) | tickless |
|---|---:|---:|
| wall | 399,666 us | 403,960 us |
| halted | 397,010 us | 400,322 us |
| **core active** | **2,656 us** | **3,638 us** |
| **duty cycle** | **0.6 %** | **0.9 %** |
| alarm wakeups | 400 | 0 |

Four hundred wakeups became zero and the part worked **37 % harder**. Three
repeats each way — control 2,656 / 2,656 / 2,656 us, tickless 3,638 / 3,638 /
3,639 us — so this is a deterministic instrument, not noise.

#### The ceiling argument, which is the real result

A `waiti` control is **already 99.4 % halted**. That is the ceiling for *any*
idle optimisation on this workload: a tickless implementation costing
literally nothing could remove at most 2,656 us from a 399,666 us run.

This one is not free. Each sleep replaces ~20 tick interrupts at ~6.6 us
(~133 us) with one suspend / reprogram / sleep / measure / restore / resume
cycle costing ~182 us — **+49 us per sleep**, twenty times over.

The reason is that `waiti` is a *shallow* halt: the core clock stops, nothing
else does, so re-entering costs one interrupt entry. Tickless pays for itself
only when a wakeup is EXPENSIVE.

> **Tickless idle is a lever on sleep DEPTH, not sleep COUNT. Removing wakeups
> is worth nothing until a wakeup is worth something.**

#### What this prunes, and what it opens

**M4 — the fitted sleep-length policy — is PRUNED, on arithmetic.** Tuning
*when* to sleep cannot help when the sleep is the wrong *depth*, and no policy
beats a 0.6 % ceiling. This is precisely the go/no-go M3 was declared to be on
2026-09-19, answered "no".

What it opens is **sleep depth**: `Rtc::sleep_light`, which gates clocks and
drops power domains. There a wake costs hundreds of microseconds and real
charge, and 400 to 0 becomes the whole game. The mechanism is already built
to survive the swap — elapsed time is read off the counter rather than
trusted, which is exactly what a sleep that reports nothing demands.

#### On predicting first

The prediction on record before the run was that the difference would be
"small". It was neither small nor in the predicted direction. A confirmed
guess would have taught nothing; this one produced the ceiling argument that
closed the mission.

## 5 · Remaining work

| brick | what |
|---|---|
| **M2a** | wire `idle_suppress_ticks` into the sim's `prvIdleTask` and make the sim port sleep. **Needs an `ORACLES.md` decision first**: the sim's time is critical-section exits, not wall time, so "sleeping" is a sim-contract change |
| **M5** | ship: opt-in package, provenance, README row naming which gate covers which half. The README row is **done** — it states the wakeup win AND the duty-cycle loss |
| **M6 (new, replaces M4)** | sleep DEPTH: `Rtc::sleep_light` behind the same measured-elapsed mechanism, with the same three numbers plus a current shunt. This is where the prize actually is |
| ~~**M4**~~ | ~~fitted sleep-length policy~~ — **PRUNED by M3c on arithmetic.** A `waiti` idle is already 99.4 % halted, so no policy can win more than 0.6 %. Reopen only if M6 makes a wake expensive enough for the policy to matter |

**Owner-only:** M2a needs an `ORACLES.md` decision on what a sim sleep means.
M3 needs a board (ESP32-S3 DevKit is cheapest — it closed `build-me-bare` B3 —
but Cortex-M3 is the Kairos-native target) and a current shunt.

## 6 · Phases and kill tests

| # | brick | kill test a stranger can run |
|---|---|---|
| M0 | ceiling probe | `kairos power idle` reports a non-zero sleepable share |
| M1 | projection | `kairos power diff` reports 18/18 invariant; widening the suppressible set fails tests |
| M2 | mechanism | the existing 18-scenario differential is unchanged with tickless off |
| M3a | mechanism on silicon | `cargo run --release` and `--features tickless` in `mps2-an385-qemu-tickless` both PASS; wakeups collapse; one pinned order digest serves both arms |
| M3b | the S3 joint and tickless on it | `cargo run --release` and `--features tickless` in `xiao-s3-tickless` both PASS on a XIAO; wakeups collapse; one pinned digest serves both arms |
| M3c | the energy question | **answered, negative** — against a `Port::idle` baseline the duty cycle went 0.6 % -> 0.9 %; the ceiling is 0.6 % and tickless cannot beat it on a shallow halt |
| M6 | sleep depth | a measured current drop on a named board with `Rtc::sleep_light`, digest and tick band clean |
| M4 | fitted policy | beats M3 **on a holdout board it was not fitted on** |
| M5 | ship | `cargo add` plus three lines reduces measured current on a stranger's board |

## 7 · Decision log

| date | decision |
|---|---|
| 2026-09-19 | Policy is a separate opt-in package; `rusty_rtos_core` never depends on it |
| 2026-09-19 | Two gates named separately; the differential never covers the policy |
| 2026-09-19 | Exactly three events are suppressible; a fourth is a decision-log row, not a code edit |
| 2026-09-19 | M3 is the go/no-go for M4 — a fixed policy within noise of optimal ends the mission |
| 2026-09-19 | M0 measured 50.3% sleepable over the corpus, bimodal 9/9. Mission proceeds |
| 2026-09-19 | A port reports the ticks it slept and the kernel winds the clock; a port that oversleeps is clamped, not trusted, because this kernel may not panic |
| 2026-09-19 | The application veto stays unwired until the kernel has a `Hooks` seam; declining by returning zero is the supported route |
| 2026-09-19 | Making the SIM port sleep is a sim-contract change and belongs to `ORACLES.md`, not to a code edit — M2a, owner-only |
| 2026-09-19 | On silicon a tick stamp is a wall-clock reading, so the hardware gate is the event ORDER plus a TIMING BAND. `power diff`'s line comparison stays the gate for stored traces and is not quoted for hardware |
| 2026-09-19 | Wakeup count is the proxy under QEMU, and the word "proxy" ships with every number. Energy needs a board |
| 2026-09-19 | M3 splits: M3a is the mechanism on a Cortex-M cell, done. M3b is the XIAO, M3c is the meter |
| 2026-09-19 | The K3 prerequisite is met inside this mission rather than deferred: `xiao-s3-tickless` drives the Kernel from `SYSTIMER` alarm 0. Its control arm IS the K3 claim, so one flashing answers both |
| 2026-09-19 | A cell that has never executed says so in its module header, its README banner and its commit. `xiao-s3-tickless` said so until it was flashed, and the claim was replaced by measurements the same day |
| 2026-09-19 | `waiti 0` leaves `PS.INTLEVEL` at zero after the wake, so an Xtensa port must re-raise its own mask; `wfi` does not, so ARM must not. The two sleeps are not interchangeable and neither is their suppression |
| 2026-09-19 | When a fix ships as two changes, the one that did nothing is labelled as such. The `Software0` decline fired zero times and is recorded as defence in depth, not as part of the cure |
| 2026-09-19 | M3c may not use `xiao-s3-tickless`'s control arm as its energy baseline: that arm busy-spins, because `idle_suppress_ticks` returns before reaching `Port::idle`. Valid for counting wakeups, invalid for counting current. **Fixed same day** — the control now halts, and the fixed measurement is M3c |
| 2026-09-19 | **M3c answers the go/no-go NO.** Against a fair `waiti` baseline tickless raised the duty cycle 0.6 % -> 0.9 %. M4 (fitted policy) is pruned on arithmetic: 99.4 % already halted leaves nothing for a policy to win |
| 2026-09-19 | The mission's remaining value is sleep DEPTH, not sleep count. M6 replaces M4 |
| 2026-09-19 | The README states the duty-cycle LOSS beside the wakeup win. A feature whose headline number is real and whose benefit is conditional says both, or the headline is a lie by omission |
| 2026-09-19 | The Xtensa sleep lives in the CELL, not in `rusty_rtos_port-xtensa`: SysTick is a core peripheral the ARM port owns, `SYSTIMER` is a chip peripheral belonging to `esp-hal`, and the Xtensa port is HAL-free. A second ESP part promotes the newtype into `rusty_rtos_port-esp` |
| 2026-09-19 | An Xtensa port measures its own sleep with the free-running counter rather than trusting the sleep, because `waiti 0` unmasks and `Rtc::sleep_light` reports nothing. That mechanism is what lets M3c swap in a deeper sleep without re-proving anything |
| 2026-09-19 | The tickless cell pins ONE order digest for BOTH arms, so a single run gates and the cross-arm claim is carried by a number rather than by a promise to run a diff |

## 8 · Appendix: ground truth

- **Traces:** `oracle/traces/<scenario>.trace`, written by `kairos oracle trace`.
  18 scenarios, 263,543 lines, 38,013 ticks.
- **Suppressible set:** `TASK_INCREMENT_TICK` (15.2% of corpus lines, 40,008),
  `LOW_POWER_IDLE_BEGIN`, `LOW_POWER_IDLE_END`. Named once, in
  `rusty_rtos_core::trace::is_suppressible`; the CLI mirrors the spellings.
- **Sleep floor:** `--min-sleep` defaults to 2 ticks, matching
  `configEXPECTED_IDLE_TIME_BEFORE_SLEEP`.
- **Idle task name:** `IDLE`, as `prvIdleTask` sets it. `power idle` finds
  windows by tracking `TASK_SWITCHED_IN`.
- **Standing rule:** a raw trace byte-diff cannot gate a tickless port, and the
  projection is the only thing that may be quoted as proving the schedule held.
  **On hardware, tighten it further** (M3a): the projection's *tick stamps* are
  a wall-clock reading there and move with the instrument's own cost, so what
  may be quoted is the projection's ORDER, plus a separate bound on the clock.
- **The tickless cells:** `rusty_rtos_port/firmware/mps2-an385-qemu-tickless`,
  pinned order digest `94f229d7a5bd8f77`, measured on qemu-system-arm 11.1.0,
  `-cpu cortex-m3 -machine mps2-an385`, release profile. And
  `rusty_rtos_port/firmware/xiao-s3-tickless`, **built, never flashed**, whose
  digest is deliberately unpinned until a board has said what it is.
- **`waiti 0` is not `wfi`.** It sets `PS.INTLEVEL` to zero, so it unmasks
  going in and the tick really is taken; ARM's `wfi` wakes on an interrupt
  left pending under PRIMASK. The suppression is a flag read at the top of
  the handler on Xtensa and a `PENDSTCLR` on ARM, and the two are not
  interchangeable.
