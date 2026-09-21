# Integrating Kairos with Janus — the K5 contract

**Audience: whoever is implementing K5 on the Kairos side.** This is the
Janus half of the joint, written from the Janus tree on 2026-09-20. Every
version and number below was read off a resolved lockfile, a published
manifest or a board, not from a plan.

`janus.md` §2.3 is the rule this document serves: *Janus consumes
`rusty_rtos_port-xtensa` the way it consumes `rusty_alloc` — one seam crate
owns the pins.* Kairos is a **seam** to Janus, never a parent, and Janus must
stay useful with Kairos absent. Nothing below asks you to change that.

---

## 1. What Janus has already done for you

Three things landed on the Janus side on 2026-09-20 so that K5 is a port
problem rather than a dependency problem.

### `rusty_esp_rtos` — the seam exists and compiles against your published ports

`janus/rusty_esp_core/crates/rusty_esp_rtos`. It pins
`rusty_rtos_port-xtensa = "=0.1.0"`, `rusty_rtos_port-riscv = "=0.1.0"` and
`rusty_rtos_core = "=0.1.0"`, target gated, behind an off-by-default `port`
feature. Verified: `--features port` compiles for both
`xtensa-esp32s3-none-elf` and `riscv32imac-unknown-none-elf` against the
crates.io releases.

**You do not need to do anything to be consumed.** When the ports gain the
radio driver, Janus turns on one feature.

### The C6 mesh node moved to the companion set your port requires

`esp-rtos` 0.4 requires `esp-hal ~1.2.0-rc.0`; the mesh node was on `=1.1.2`.
It is now on esp-hal 1.2.0 / esp-radio 1.0.0-beta.1 / esp-rtos 0.4.0, with
esp-alloc 0.11, esp-bootloader-esp-idf 0.6, esp-println 0.18 and
esp-backtrace 0.20 moving with it. It builds.

### A library pin that would have blocked you is gone

`rusty_esp_signal-esp` hard-pinned `esp-hal = "=1.1.2"`. An `=` pin in a
*library* takes the companion-set choice away from every consumer at once,
which is backwards — the rule is that a **firmware** must not mix sets, and
it is the firmware that picks which. It is a range now.

If you find another Janus library doing this, it is a bug on our side; say
so rather than working around it.

---

## 2. The hard joint: `xtensa-lx-rt` is a `links` crate

Your Xtensa port runs its context switch inside a software interrupt so
`xtensa-lx-rt`'s exception entry spills the register windows. That makes
`xtensa-lx-rt` a shared, exclusive dependency: **one version per graph**, and
esp-hal chooses it.

Read on 2026-09-20:

| | `xtensa-lx-rt` |
|---|---|
| `rusty_rtos_port-xtensa` 0.1.0 (pinned to match esp-hal 1.2.1) | 0.23 |
| `janus/rusty_esp_dsp` `xiao-s3-probe` | **0.23.0** |
| `janus/rusty_esp_mid` `xiao-s3-keys` | **0.23.0** |

**The joint already lines up.** If you move the port's pin, this is the table
to re-check first — a mismatch here fails the build outright, which is the
good failure, but it fails it for every Janus S3 firmware at once.

---

## 3. What Kairos still owes K5

`esp-radio` reaches a scheduler through **`esp-radio-rtos-driver`**. The
Xtensa port documents that interface (`lib.rs`, "the interface 0.4.1 that K5b
joins") and does not implement it. Until it does, a Janus firmware with a
radio keeps `esp-rtos`, and the seam's `compat::HOSTS_ESP_RADIO` reads
`false` in code rather than staying silent.

Two notes from moving the firmware onto `esp-rtos` 0.4, both of which are
your integration surface rather than incidental renames:

- **`esp_rtos::start(timer, peripherals.FROM_CPU_INTR0)`.** 0.4 takes the
  FROM_CPU interrupt peripheral directly; 0.3 went through
  `SoftwareInterruptControl::new(peripherals.SW_INTERRUPT)`. That interrupt
  is exactly where your context switch runs, so whatever Kairos offers in
  place of `esp_rtos::start` should take the same thing — a firmware
  swapping kernels should be editing one line, not restructuring init.
- **`esp-radio-rtos-driver` is at 0.4.2 on crates.io**, not the 0.4.1 the
  mission plan names. Worth confirming which you target before writing
  against the older docs.

---

## 4. The kill test, and the thing that blocks it

K5's kill test is **Janus S1**:

> two C6s, ESP-NOW authenticated link, 1 000 frames each way, loss / replay /
> bad-tag counters, a third C6 cannot join — passing on the Kairos kernel
> **with the same numbers as on `esp-rtos`**, recorded in both ledgers.

> ⚠ **The baseline does not exist yet.** `janus.md` says it outright: S1 is
> *"blocked twice: on 2× C6, and on this row being taken on esp-rtos before
> any kernel is swapped under it — the baseline a rusty_RTOS S1 must match
> does not exist until this row does."*

So the sequence is fixed, and it is not negotiable by either side:

1. Janus runs S1 on `esp-rtos`, on two C6s, and records the counters.
2. Kairos implements `esp-radio-rtos-driver` in `-riscv`.
3. The mesh node is rebuilt on `rusty_rtos` in a **sibling firmware
   directory** — the `esp-rtos` build stays as the comparison arm, because a
   before/after with only an "after" is not a comparison.
4. S1 runs again and the numbers are compared.

Step 1 needs hardware that is not on the bench today. **Do not schedule K5's
kill test against a baseline that has not been taken.**

---

## 5. The Xtensa half can proceed without any of that

The second half of K5 — "then the XIAO S3 sketch firmware on the Xtensa
port" — has no such blocker, because the S3 Track B firmwares are
**bare-metal main loops with no scheduler and no radio**. A kernel under them
is *additive*: it is a smoke test of the port, not a migration, and it needs
neither the radio driver nor a second board.

Janus has a measured reference standing for exactly this. `xiao-s3-keys` is
the designated first occupant because it already carries a baseline the
kernel must cost nothing against, and it was **re-measured on silicon on
2026-09-20** so the reference is current rather than months old:

| op | 2026-09-20 median | vs the recorded M1 |
|---|---:|---:|
| sign | **94,783 µs** | −0.16% |
| verify | **151,892 µs** | +0.29% |

`cpu_mhz=240`, `iterations=100`, `rung=core-only`, `verify_ok=100/100`.
Min/median/max within 0.6% on verify and 0.006% on sign.

**Any cost the kernel adds is attributable against these numbers.** Re-run
the firmware unchanged first to confirm the board agrees, then again with the
port under it.

One thing to know before you read a regression into it: the board reset once
mid-run, after `generate` and before `sign`, and completed the full sequence
on the second boot. `generate` is 77 ms of continuous compute with no yield —
the operation most likely to interact with a scheduler tick. It is
**pre-existing bare-metal behaviour recorded before the port**, so if you see
it, it is not yours.

---

## 5b. And the kernel now schedules Janus tasks through the seam

Added 2026-09-20, after the section above was written.

The seam gained a `kernel` rung (`rusty_rtos_kernel-core`), and
`rusty_esp_core/firmware/xiao-s3-kairos-tasks` runs two tasks that
**`Kernel::switch_context` chooses**, on a XIAO S3, through the seam:

```
KT boot main_idx=0 current_idx=0 ready1=Some(1) ready2=Some(2)
KT laps_a=50 laps_b=50 want=50 faults=0
KT entries=103 swaps=103 declined_same=0 declined_no_ctx=0
RESULT: PASS
```

103 entries, 103 real swaps, none declined; hand-off by `suspend`/`resume`;
witnesses checked from three call frames deep. This is the claim
`xiao-s3-switch` deliberately does not make, and it is now made from the
Janus side against your published 0.1.0 crates.

**Two things you may want from this.** First, the priority trap: because
`start_scheduler` creates `Tmr Svc` unconditionally at
`TIMER_TASK_PRIORITY` and `create_task` makes the highest-priority task
current, a firmware whose `main` sits *below* the daemon has the kernel
believing a stackless task is running — `current_idx` names the daemon while
the CPU is on `main`'s stack, every switch is declined as `from == to`, and
the workers starve silently at `laps 0/0`. It cost a board run here and it
is the same failure your radio cell's README records. A line in the
`create_task` or `start_scheduler` docs would save the next person the run.

Second, and larger: **the radio adapter is in a firmware, not a crate.**
`xiao-s3-radio` implements all five `esp-radio-rtos-driver` traits and
passes on silicon — which is further along than the mission plan's "the
ports document the interface" suggests. But Janus cannot consume ~1,450
lines of `adapter.rs` + `kernel.rs` from inside a firmware directory. If
that glue became a crate (`rusty_rtos_port-esp-radio`, or a feature on the
port crates), the Janus mesh node could take it the day S1's baseline
exists, instead of Janus duplicating it and the two copies drifting.

## 6. How to consume Janus, concretely

```toml
# in a Janus firmware's Cargo.toml
rusty_esp_rtos = { version = "0.1", features = ["port"] }
```

```rust
// prove which kernel the binary is actually on, at startup
if let Some(p) = rusty_esp_rtos::port_name() {
    println!("KERNEL port={p}");
}
```

That print is not decoration. A capability you cannot detect is one you must
not claim, and a swap that cannot be observed from the serial log is a swap
nobody can verify from a ledger row.

Janus firmwares are separate cargo projects, never workspace members — each
needs its own target, linker script and (for Xtensa) toolchain. Expect to
build them individually.

---

## 7. What Janus will hold you to

These are the house rules both sides already share; they are here so K5 is
not where they get discovered.

- **A gate that cannot fail is worse than no gate, because it reports
  success.** Poison every gate — make it fail on purpose before quoting it.
- **Same-build A/B wherever a number decides something.** Cross-build
  comparisons on these firmwares move several percent from layout alone; we
  have measured a null arm at p90 2.3% after moving 103 lines between
  functions in one firmware.
- **Work-count parity.** If the kernel changes how many frames, ticks or
  samples a run processes, the comparison is void before it is slow.
- **Both ledgers.** K5's numbers land in the Kairos ledger and in the Janus
  package's ledger, with the method line, or they are not quotable.
