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
| a port that sleeps | **done, M2 in tests** — `SleepyPort`. The **sim port does not**, see M3 |
| on-board energy measurement | **not built** — needs hardware |

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

## 5 · Remaining work

| brick | what |
|---|---|
| **M2a** | wire `idle_suppress_ticks` into the sim's `prvIdleTask` and make the sim port sleep. **Needs an `ORACLES.md` decision first**: the sim's time is critical-section exits, not wall time, so "sleeping" is a sim-contract change |
| **M3** | the fixed policy on a chip — sleep to the next wake less a fixed margin. **The go/no-go for M4** |
| **M4** | only if M3 leaves a gap: observe-only harvest, then a threshold or a fit. Decide which *at the ceiling step* |
| **M5** | ship: opt-in package, provenance, README row naming which gate covers which half |

**Owner-only:** M2 needs a board choice (ESP32-S3 DevKit is cheapest — it closed
`build-me-bare` B3 — but Cortex-M3 is the Kairos-native target). M3 needs a
current shunt.

## 6 · Phases and kill tests

| # | brick | kill test a stranger can run |
|---|---|---|
| M0 | ceiling probe | `kairos power idle` reports a non-zero sleepable share |
| M1 | projection | `kairos power diff` reports 18/18 invariant; widening the suppressible set fails tests |
| M2 | mechanism | the existing 18-scenario differential is unchanged with tickless off |
| M3 | fixed policy | measured energy drop on a named board, with `power diff` clean |
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
