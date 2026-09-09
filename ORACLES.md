# Oracles — what every Kairos number is measured against

A number measured against an unpinned checkout is not a number. Every oracle
below is pinned to a commit; `kairos oracle fetch` clones exactly these, into
`oracle/` (gitignored), and refuses to run against anything else. Re-pinning
is a decision-log row in `docs/plans/rtos-mission.md`.

## The kernel

| oracle | tag | commit | role |
|---|---|---|---|
| `FreeRTOS/FreeRTOS-Kernel` | **V11.3.1** (2026-08-21) | `3a22924e0a9ddbbc8b0758881c33b3422a5cc20d` | the reference kernel: its trace macros, on its Posix port, under the deterministic-tick patch in `oracle/harness/`, are the bitstream every `rusty_rtos_kernel` scenario diffs against |
| `FreeRTOS/FreeRTOS` (classic distribution) | `main` on 2026-08-26 (no release tag covers the demo corpus) | `f4fcc3b228643144727e9257ba12db1cb632b6e6` | `FreeRTOS/Demo/Common/Minimal` (34 standard demo task files, the conformance corpus), `Demo/Posix_GCC` (the config the oracle harness copies), `Test/CBMC` and `Test/VeriFast` (the proof list for Kani and loom) — sparse-checked-out, never the 200 board demos |

`V11.3.1` is also the kernel the current LTS pins (below), so the kernel oracle
and the library oracles are one tag.

## The libraries (FreeRTOS 202604.01-LTS, `FreeRTOS/FreeRTOS-LTS` @ `0b25dc50bae4cb971c7a459b109e52ab2f01a6b8`)

| oracle | tag | commit | Kairos package |
|---|---|---|---|
| `FreeRTOS/FreeRTOS-Plus-TCP` | V4.4.1 | `c12361095aca68aeed858f45d14395fbffa92c0d` | `rusty_rtos_tcp` (K7) |
| `FreeRTOS/coreMQTT` | v5.0.2 | `04845c6a8e5f9cf2d232f1c6e80baeb81302e690` | `rusty_rtos_mqtt` (K7) |
| `FreeRTOS/coreMQTT-Agent` | v2.0.0 | `2338d00db41196e4a1fce4bc3ab32f907e77f871` | `rusty_rtos_mqtt` (K7) |
| `FreeRTOS/coreHTTP` | v3.1.3 | `3c4a5838658cd6d0ff8fb7c3a14e30baafcbcd28` | `rusty_rtos_http` (K7) |
| `FreeRTOS/coreJSON` | v3.3.1 | `cffa492da18c890181d64462f8af63992a69d3b0` | `rusty_rtos_json` (K7) |
| `FreeRTOS/coreSNTP` | v2.0.0 | `50f5f96f4c33b14c0358f404ff4ff2a29d422ad7` | `rusty_rtos_sntp` (K7, or a `rusty_time` leaf) |
| `FreeRTOS/corePKCS11` | v3.6.4 | `ccc78afee1716436cca832dd3d9388ead2ba05b0` | `rusty_rtos_pkcs11` (K9) |
| `FreeRTOS/backoffAlgorithm` | v1.4.2 | `14f4c88b33dd554be30a00a312c88d3986d457d0` | `rusty_rtos_backoff` (K7) |
| `FreeRTOS/FreeRTOS-Cellular-Interface` | v1.4.2 | `5d76e3b3099c3693f98ecc6e15bc62cca140f13f` | `rusty_rtos_cellular` (K9) |

The LTS also carries the AWS IoT libraries (SigV4, Device Shadow, Defender,
Jobs, Fleet Provisioning, MQTT file streams). They are NEVER in Kairos (mission
plan §2.1): cloud-of-record by design.

## Emulators and silicon (the second oracle)

| cell | what runs there | tool | state on this machine (2026-09-09) |
|---|---|---|---|
| Posix host | the C kernel's Posix port + the harness; our `-posix` port | `gcc 15.2` / `clang 21` under WSL2 Ubuntu (`wsl -d Ubuntu`); `clang 22` natively (MSVC target, no pthreads — not used for the oracle) | present |
| QEMU `lm3s6965evb` (Cortex-M3) | `CORTEX_LM3S6965_GCC_QEMU`; our `-cortex-m` | `qemu-system-arm` | **not installed** (owner-only step) |
| QEMU `virt` RV32 | `RISC-V_RV32_QEMU_VIRT_GCC`; our `-riscv` | `qemu-system-riscv32` | **not installed** |
| ESP32-C6-DevKitC-1 | our `-riscv` on esp-hal; the Janus S1 joint | `espflash 4.5`, `espino` | on the Janus purchase list |
| XIAO ESP32-S3 Sense | our `-xtensa`; SMP on two cores | esp toolchain, `espflash` | in hand |

`rusty-arm-vm` (a Rust Cortex-M emulator on crates.io, `rusty-arm-kernel
0.1.0`) is a candidate *comparison* arm for the Cortex-M cells once QEMU has
produced the first number; it is not an oracle until it agrees with QEMU on the
C demo.

## The sim contract (v1, K0) — how a deterministic tick is defined

Both the patched C oracle and `rusty_rtos_port-sim` obey exactly this, so their
traces are comparable:

1. There is no timer thread and no signal. Time advances only through
   `kairos_tick()`, which does what the Posix port's `SIGALRM` handler did:
   increment the tick and switch context if the kernel asks.
2. **A tick is delivered from the idle hook** (`configUSE_IDLE_HOOK = 1`): each
   time the idle task runs its hook, one tick is delivered before it yields.
3. **A tick is delivered on every 16th outermost critical-section exit** on a
   task thread after the scheduler started (`vPortExitCritical` reaching
   nesting 0; every kernel API and every yield passes through one). This is
   where a hardware tick that arrived during the section fires, so a task
   that polls without blocking (`dynamic`'s SUSP_RX, `PollQ`) still sees
   time move. A task that computes without touching the kernel at all
   (`flop`, `integer`) does not: those scenarios need contract v2.
4. **A CHECK task** (name `CHECK`, priority 5, `configMINIMAL_STACK_SIZE`)
   is created after the scenario's tasks; it delays 100 ticks at a time and
   ends the run when the tick count reaches `max_ticks`, printing
   `KAIROS_RESULT <scenario> pass|fail ticks=<n> yields=<n> exits=<n>
   lines=<n>` as the last line of the trace.
5. Everything else is the kernel's own behaviour under `configUSE_PREEMPTION =
   1`, `configUSE_TIME_SLICING = 1`, and the `Posix_GCC` demo's
   `FreeRTOSConfig.h` values (copied into the harness verbatim, with the trace
   macros added).

The contract is version 1; changing it is a `FORMAT_VERSION` bump for the trace
format (mission plan §2.5) and re-captures every stored trace.

## Trace line format (v1)

One line per event on stderr of the oracle binary and of our sim sink:

```
<tick> <EVENT> <task-or-object-name> [<arg>...]
```

`tick` is the kernel's tick count at the event (read before the increment
in `TASK_INCREMENT_TICK`, so that line carries the tick being left). Tasks
and timers are named by their own names (`IDLE`, `Tmr Svc`, the scenario's);
queues, event groups and stream buffers by a creation ordinal per kind
(`q1`, `g1`, `s1`, first-seen order), so no address ever reaches a line. The
event set for K1/K2 is the 42 macros in `oracle/harness/FreeRTOSConfig.h`,
mirrored one for one by `rusty_rtos_core::trace::Event`; the other ~470 (the
`traceENTER_*` / `traceRETURN_*` pairs) are NOT part of the contract.

### The debug column

With `KAIROS_TRACE_EXITS` set in the environment, **both** sides append
` #<outermost critical-section exits>` to every line, and `kairos conform
--exits` compares that too. It is off by default because a line with the
column is not the contract's format — but it is how a divergence is
actually diagnosed. Sim time is a count of those exits (rule 3), so a
kernel can emit the right events for a long while after it has started
disagreeing about *when*: on the first `dynamic` diff the events matched
for 1,598 lines past the point where the accounting had drifted.
