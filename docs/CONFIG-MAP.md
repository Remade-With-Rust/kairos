# Config map — every `config*` knob of FreeRTOS-Kernel V11.3.1, classified

Written 2026-09-09 from the 94 `config*` identifiers `FreeRTOS.h` reads
(`3a22924e`; the two grep artefacts `configuration` and `configured` dropped).
`FreeRTOSConfig.h` becomes a type in Kairos (mission plan §3.2): each knob is
one of

- **const** — an associated constant on `rusty_rtos_core::Config` (geometry;
  the kernel is generic over it, so a wrong value is a compile error);
- **type** — an associated type on `Config`;
- **feature** — a cargo feature on `rusty_rtos_kernel` (a subsystem's presence);
- **hook** — a method on the `Hooks` trait (default no-op);
- **port** — the port crate's business (`rusty_rtos_port-*`), never the core's;
- **derived** — computed from another knob, not settable;
- **never** — not remade, with the reason.

The `default` profile (`rusty_rtos_core::config::DefaultConfig`) mirrors
`examples/template_configuration/FreeRTOSConfig.h` value for value where a
value exists. `parameterizing-a-constant` governs every knob's tests.

| knob | class | Kairos form | note |
|---|---|---|---|
| `configTICK_RATE_HZ` | const | `Config::TICK_RATE_HZ` (100) | |
| `configTICK_TYPE_WIDTH_IN_BITS` | type | `Config::Tick = Bits32` (`Bits16` / `Bits64`) | the one most likely to invert a test |
| `configUSE_16_BIT_TICKS` | derived | `Bits16` | the pre-V10.5 spelling |
| `configINITIAL_TICK_COUNT` | const | `Config::INITIAL_TICK_COUNT` (0) | tests set it near the wrap |
| `configMAX_PRIORITIES` | const | `Config::MAX_PRIORITIES` (5) | `Priority::new` is bounded by it |
| `configMINIMAL_STACK_SIZE` | const | `Config::MINIMAL_STACK_SIZE` (128 words) | |
| `configMAX_TASK_NAME_LEN` | const | `Config::MAX_TASK_NAME_LEN` (16) | |
| `configSTACK_DEPTH_TYPE` | type | `usize` | fixed; no need for a narrower type in Rust |
| `configMESSAGE_BUFFER_LENGTH_TYPE` | type | `usize` | fixed |
| `configRUN_TIME_COUNTER_TYPE` | type | `u64` | fixed (`stats` feature) |
| `configTLS_BLOCK_TYPE` | never | — | C-runtime TLS; a Rust task owns its state |
| `configUSE_PREEMPTION` | const | `Config::USE_PREEMPTION` (true) | |
| `configUSE_TIME_SLICING` | const | `Config::USE_TIME_SLICING` (false in the template; the Posix demo uses true) | |
| `configIDLE_SHOULD_YIELD` | const | `Config::IDLE_SHOULD_YIELD` (true) | |
| `configUSE_PORT_OPTIMISED_TASK_SELECTION` | port | the port's ready-list selection | the core selects by priority bitmap either way |
| `configUSE_TICKLESS_IDLE` | feature | `tickless-idle` | K8 |
| `configEXPECTED_IDLE_TIME_BEFORE_SLEEP` | const | `Config::EXPECTED_IDLE_TIME_BEFORE_SLEEP` | with `tickless-idle` |
| `configPRE_SLEEP_PROCESSING` | hook | `Hooks::pre_sleep` | |
| `configPOST_SLEEP_PROCESSING` | hook | `Hooks::post_sleep` | |
| `configPRE_SUPPRESS_TICKS_AND_SLEEP_PROCESSING` | hook | `Hooks::pre_suppress_ticks` | |
| `configUSE_IDLE_HOOK` | hook | `Hooks::idle` | always compiled; default no-op |
| `configUSE_PASSIVE_IDLE_HOOK` | hook | `Hooks::passive_idle` | SMP, K8 |
| `configUSE_TICK_HOOK` | hook | `Hooks::tick` | |
| `configUSE_MALLOC_FAILED_HOOK` | hook | `Hooks::malloc_failed` | |
| `configUSE_DAEMON_TASK_STARTUP_HOOK` | hook | `Hooks::daemon_startup` | |
| `configUSE_SB_COMPLETED_CALLBACK` | feature | `stream-buffer-callbacks` | |
| `configCHECK_FOR_STACK_OVERFLOW` | const | `Config::CHECK_FOR_STACK_OVERFLOW` (2) | method 1 and 2 as the C kernel |
| `configCHECK_HANDLER_INSTALLATION` | port | the port asserts its vectors | |
| `configRECORD_STACK_HIGH_ADDRESS` | feature | `stats` | |
| `configUSE_TASK_NOTIFICATIONS` | feature | `task-notifications` (default on) | |
| `configTASK_NOTIFICATION_ARRAY_ENTRIES` | const | `Config::NOTIFICATION_ARRAY_ENTRIES` (1) | |
| `configNUM_THREAD_LOCAL_STORAGE_POINTERS` | const | `Config::NUM_TLS_POINTERS` (0) | |
| `configUSE_MUTEXES` | feature | `mutexes` | |
| `configUSE_RECURSIVE_MUTEXES` | feature | `recursive-mutexes` | |
| `configUSE_COUNTING_SEMAPHORES` | feature | `counting-semaphores` | |
| `configUSE_QUEUE_SETS` | feature | `queue-sets` | |
| `configQUEUE_REGISTRY_SIZE` | const | `Config::QUEUE_REGISTRY_SIZE` (0) | the oracle harness sets it > 0 so queues have names in the trace |
| `configUSE_TIMERS` | feature | `timers` | |
| `configTIMER_TASK_PRIORITY` | const | `Config::TIMER_TASK_PRIORITY` (MAX - 1) | validated < `MAX_PRIORITIES` |
| `configTIMER_QUEUE_LENGTH` | const | `Config::TIMER_QUEUE_LENGTH` (10) | |
| `configTIMER_TASK_STACK_DEPTH` | const | `Config::TIMER_TASK_STACK_DEPTH` (128) | |
| `configUSE_EVENT_GROUPS` | feature | `event-groups` | |
| `configUSE_STREAM_BUFFERS` | feature | `stream-buffers` | |
| `configUSE_TRACE_FACILITY` | feature | `trace` | the `Trace` seam is always present; this adds the numbering fields |
| `configUSE_STATS_FORMATTING_FUNCTIONS` | feature | `stats` | `vTaskList`-style text |
| `configGENERATE_RUN_TIME_STATS` | feature | `stats` | |
| `configSTATS_BUFFER_MAX_LENGTH` | const | `Config::STATS_BUFFER_MAX_LENGTH` | with `stats` |
| `configUSE_APPLICATION_TASK_TAG` | feature | `task-tags` | |
| `configUSE_POSIX_ERRNO` | never | — | Rust errors are values (`rusty_rtos_core::Error`) |
| `configSUPPORT_STATIC_ALLOCATION` | feature | `static-alloc` (default on) | the arena IS static allocation |
| `configSUPPORT_DYNAMIC_ALLOCATION` | feature | `dynamic-alloc` | through the `Heap` seam |
| `configKERNEL_PROVIDED_STATIC_MEMORY` | derived | the kernel owns the idle and timer task memory | |
| `configAPPLICATION_ALLOCATED_HEAP` | port | `rusty_rtos_heap`'s region is caller-owned | |
| `configENABLE_HEAP_PROTECTOR` | feature | `heap-protector` on `rusty_rtos_heap` | canaries on |
| `configUSE_MINI_LIST_ITEM` | never | — | the list is index-linked; there is no item struct to slim |
| `configUSE_LIST_DATA_INTEGRITY_CHECK_BYTES` | never | — | generational handles refuse a stale item; nothing to poison |
| `configENABLE_BACKWARD_COMPATIBILITY` | never | — | the old names are the C ABI's job (`rusty_rtos-capi`) |
| `configUSE_ALTERNATIVE_API` | never | — | removed upstream |
| `configUSE_CO_ROUTINES` | never | — | co-routines are deprecated upstream |
| `configMAX_CO_ROUTINE_PRIORITIES` | never | — | co-routines |
| `configUSE_NEWLIB_REENTRANT` | never | — | a C runtime shim |
| `configUSE_PICOLIBC_TLS` | never | — | a C runtime shim |
| `configUSE_C_RUNTIME_TLS_SUPPORT` | never | — | a C runtime shim |
| `configINIT_TLS_BLOCK` | never | — | a C runtime shim |
| `configSET_TLS_BLOCK` | never | — | a C runtime shim |
| `configDEINIT_TLS_BLOCK` | never | — | a C runtime shim |
| `configINCLUDE_FREERTOS_TASK_C_ADDITIONS_H` | never | — | a C include hook |
| `configRUN_ADDITIONAL_TESTS` | never | — | an upstream test hook |
| `configASSERT` | never | — | a Rust API returns `Result`; internal invariants are `debug_assert!` under `kernel-debug-checks` |
| `configASSERT_DEFINED` | derived | — | |
| `configPRECONDITION` | never | — | see `configASSERT` |
| `configPRECONDITION_DEFINED` | derived | — | |
| `configPRINTF` | never | — | the trace sink prints; the kernel never does |
| `configCONTROL_INFINITE_LOOP` | never | — | a test hook for the C proofs; Kani harnesses use their own bounds |
| `configMAX` / `configMIN` | never | — | `core::cmp` |
| `configNUMBER_OF_CORES` | const | `Config::NUMBER_OF_CORES` (1) | > 1 is the `smp` feature, K8 |
| `configRUN_MULTIPLE_PRIORITIES` | feature | `smp` | K8 |
| `configUSE_CORE_AFFINITY` | feature | `smp` | K8 |
| `configTASK_DEFAULT_CORE_AFFINITY` | const | `Config::DEFAULT_CORE_AFFINITY` | K8 |
| `configIDLE_AFFINITY` | const | `Config::IDLE_AFFINITY` | K8 |
| `configUSE_TASK_PREEMPTION_DISABLE` | feature | `preemption-disable` | K8 |
| `configUSE_TASK_FPU_SUPPORT` | port | Cortex-M4F/M7 lazy stacking | |
| `configENABLE_FPU` | port | ARMv8-M | K8 |
| `configENABLE_MVE` | port | ARMv8.1-M | K8 |
| `configENABLE_MPU` | port | `rusty_rtos_mpu` | K8 |
| `configENABLE_TRUSTZONE` | port | ARMv8-M NTZ first | K8 |
| `configENABLE_PAC` | port | ARMv8.1-M | K8 |
| `configENABLE_BTI` | port | ARMv8.1-M | K8 |
| `configRUN_FREERTOS_SECURE_ONLY` | port | ARMv8-M | K8 |
| `configUSE_MPU_WRAPPERS_V1` | never | — | v2 wrappers only |
| `configENABLE_ACCESS_CONTROL_LIST` | feature | `mpu-acl` on `rusty_rtos_mpu` | K8 |
| `configINCLUDE_APPLICATION_DEFINED_PRIVILEGED_FUNCTIONS` | never | — | an MPU-wrapper C include hook |

Kairos adds five knobs the C kernel has no name for, because its objects
live in arenas instead of a heap: `Config::MAX_TASKS`, `MAX_QUEUES`,
`MAX_TIMERS`, `MAX_EVENT_GROUPS`, `MAX_STREAM_BUFFERS` (the arena
capacities; a create beyond them is `Error::NoMemory`, exactly what
`pvPortMalloc` returning `NULL` is in C).

Coverage: 94 knobs classified — 26 const, 3 type, 17 feature, 9 hook, 13 port,
5 derived, 21 never. The `default` profile reproduces the template
configuration's values; the oracle harness uses the `Posix_GCC` demo's values
(`docs/../oracle/harness/FreeRTOSConfig.h`), which the sim port's
`PosixDemoConfig` mirrors.
