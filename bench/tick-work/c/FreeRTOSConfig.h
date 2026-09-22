/* The C arm's config for K3's tick/switch WORK rows on rv32.
 *
 * The config is the SHARED one that `bench/kernel-ram` and
 * `bench/kernel-flash` already use, included rather than copied, so a
 * comparison cannot drift from the other benches by someone editing one of
 * two files. Only the deltas this measurement REQUIRES are set here, and
 * each says why.
 */
#ifndef KAIROS_TICK_WORK_CONFIG_H
#define KAIROS_TICK_WORK_CONFIG_H

#include "../../kernel-ram/c/FreeRTOSConfig.h"

/* DELTA 1 -- no timer interrupt.
 *
 * With both of these 0, `port.c` does not define vPortSetupTimerInterrupt()
 * and never sets the mtimer enable bit, so NOTHING drives the tick but this
 * program. That is the whole point: the Rust arm calls `increment_tick()`
 * directly in a loop, and an ISR firing inside the bracket would be measured
 * as kernel work. The application supplies the empty vPortSetupTimerInterrupt
 * the port documents as the alternative. */
#undef configMTIME_BASE_ADDRESS
#define configMTIME_BASE_ADDRESS       0
#undef configMTIMECMP_BASE_ADDRESS
#define configMTIMECMP_BASE_ADDRESS    0

/* DELTA 2 -- a heap big enough for four real tasks with real stacks.
 *
 * The shared config's 4 KiB sizes a static-analysis build that never runs.
 * This one runs, and a stackless kernel's opposite number needs somewhere to
 * put four stacks. It does not touch what is being measured. */
#undef configTOTAL_HEAP_SIZE
#define configTOTAL_HEAP_SIZE          ( 48U * 1024U )

/* DELTA 3 -- the stack-overflow check is a KNOB, not a constant.
 *
 * The shared config sets configCHECK_FOR_STACK_OVERFLOW to 2, which makes
 * every vTaskSwitchContext() compare a 20-byte pattern in the outgoing
 * task's stack. A stackless kernel CANNOT have that cost, so leaving it on
 * would charge C for work our design makes impossible -- a number in our
 * favour, which is the kind that gets checked first here, not last.
 *
 * So the default is OFF, and run.sh builds the C arm BOTH ways and reports
 * both. The difference is the price of the check, stated rather than
 * hidden inside a ratio. */
#undef configCHECK_FOR_STACK_OVERFLOW
#ifdef KAIROS_STACK_CHECK
#define configCHECK_FOR_STACK_OVERFLOW    KAIROS_STACK_CHECK
#else
#define configCHECK_FOR_STACK_OVERFLOW    0
#endif

#endif /* KAIROS_TICK_WORK_CONFIG_H */
