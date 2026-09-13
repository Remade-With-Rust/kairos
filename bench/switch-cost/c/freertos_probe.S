/*
 * The oracle's context switch, isolated so it can be counted.
 *
 * This file adds NO instructions of its own. It expands FreeRTOS's two
 * context macros -- unmodified, from the pinned checkout in `oracle/` --
 * each alone in its own section, so a disassembly of that section is
 * exactly the macro and nothing else.
 *
 * Why not just disassemble `portASM.S`? Because there the macros are
 * embedded in four different handlers, and counting them would mean
 * choosing an address range by eye. Choosing the range by eye is how a
 * comparison acquires a thumb on the scale. A section boundary is chosen
 * by the assembler.
 *
 * The symbols the macros reference (pxCurrentTCB, xCriticalNesting,
 * pxCriticalNesting, xISRStackTop) are left undefined on purpose: this is
 * assembled with -c and never linked. An undefined symbol still assembles
 * to the same instruction sequence -- `load_x t0, sym` is auipc+lw either
 * way -- so the count is unaffected.
 */

#include "portContext.h"

    .section .text.kairos_probe_save, "ax"
    .align 2
    .global kairos_probe_freertos_save
    .type kairos_probe_freertos_save, @function
kairos_probe_freertos_save:
    portcontextSAVE_INTERRUPT_CONTEXT
    .size kairos_probe_freertos_save, . - kairos_probe_freertos_save

    .section .text.kairos_probe_restore, "ax"
    .align 2
    .global kairos_probe_freertos_restore
    .type kairos_probe_freertos_restore, @function
kairos_probe_freertos_restore:
    portcontextRESTORE_CONTEXT
    .size kairos_probe_freertos_restore, . - kairos_probe_freertos_restore
