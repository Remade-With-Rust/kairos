# oracle/harness-smp -- the C kernel on TWO cores

FreeRTOS-Kernel V11.3.1 built with `configNUMBER_OF_CORES 2`, running the
standard demo tasks through `../harness/main.c` on a deterministic two-core
sim port. It is the oracle for the two-core corpus
(`rusty_rtos_demo --features smp`); the contract both sides obey is in the
umbrella `ORACLES.md` ("The two-core sim contract").

| file | what |
|---|---|
| `portmacro.h`, `port.c` | the two-core port: one pthread per task, one token, the turn rule, the task lock, ticks between turns |
| `gen_api_hooks.py` | every `traceENTER_*` / `traceRETURN_*` in the pinned `FreeRTOS.h` -> the port's call-boundary hooks (`api_hooks.h`, generated) |
| `gen_config.py` | `../harness/FreeRTOSConfig.h` + two cores + the hooks (`FreeRTOSConfig.h`, generated) |
| `build.sh` | regenerates both and builds `oracle/build/smp/corpus` (WSL) |

```sh
sh oracle/harness-smp/build.sh
oracle/build/smp/corpus semtest 20000 2> semtest.trace
KAIROS_SMP_DEBUG=1 oracle/build/smp/corpus semtest 50   # + one line per turn, and why it ended
```

Dev-only, like `../harness`: nothing ships from here.
