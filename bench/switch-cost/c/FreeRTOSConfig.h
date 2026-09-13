/*
 * The smallest FreeRTOSConfig.h that lets `portContext.h`'s context macros
 * assemble. This build ASSEMBLES the oracle's own port source; it never
 * links or runs it, so only the knobs the macros read matter.
 *
 * FPU and VPU are OFF deliberately, and that is the fair setting: the
 * Kairos RISC-V port has no FPU/VPU save either, so enabling them here
 * would charge the C arm for a feature the Rust arm does not implement.
 * It is also the setting the RV32I_CLINT_no_extensions chip profile ships.
 */
#ifndef FREERTOS_CONFIG_H
#define FREERTOS_CONFIG_H

#define configENABLE_FPU                0
#define configENABLE_VPU                0
#define configMTIME_BASE_ADDRESS        0x0200BFF8
#define configMTIMECMP_BASE_ADDRESS     0x02004000
#define configISR_STACK_SIZE_WORDS      512

#endif /* FREERTOS_CONFIG_H */
