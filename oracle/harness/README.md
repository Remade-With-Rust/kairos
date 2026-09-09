# oracle/harness — the instrumented C kernel

This directory is the only C in the umbrella that is ours, and it is a
**dev-only oracle** (the way openh264 and libzstd are for the codecs): it
never ships, and nothing in `rusty_rtos_*` depends on it.

| file | what |
|---|---|
| `FreeRTOSConfig.h` | the `Posix_GCC` demo's configuration plus the trace macros of the Kairos trace contract (v1) |
| `kairos_trace.h/.c` | the line printer: `<tick> <EVENT> [<subject>] [<arg>...]` on stderr; objects named by creation ordinal per kind |
| `main.c` | the scenario runner: starts a standard demo task set, a CHECK task ends the run at `max_ticks` and prints `KAIROS_RESULT` |

The pinned kernel and demo checkouts live beside this directory
(`oracle/FreeRTOS-Kernel`, `oracle/FreeRTOS`; gitignored; `ORACLES.md` has
the commits). The Posix port is patched in place by `kairos oracle patch` —
exact-anchor textual edits, each asserted to match once, marked `KAIROS` —
to remove the timer thread, fire a tick on every 16th outermost critical-section
exit, and add `vPortKairosTick()` for the idle hook (the sim contract).

```sh
kairos oracle fetch            # clone the pinned commits (refuses a mismatch)
kairos oracle patch            # the deterministic-tick patch, idempotent
kairos oracle build dynamic    # gcc under WSL (Windows) or cc (Linux)
kairos oracle trace dynamic --ticks 2000   # run twice, refuse a differing trace, store oracle/traces/dynamic.trace.zst (rusty_zstd, round-tripped)
kairos oracle cat dynamic                  # the stored trace, decompressed to stdout (diff it against the kernel's)
```
