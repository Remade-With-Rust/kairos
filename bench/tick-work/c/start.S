    .section .init, "ax"
    .global _start
    .extern freertos_risc_v_trap_handler
_start:
    .option push
    .option norelax
    la      gp, __global_pointer$
    .option pop
    la      sp, _stack_top
    li      t5, 0x10000000
    li      t6, 65
    sb      t6, 0(t5)
    la      a0, _sbss
    la      a1, _ebss
1:  bgeu    a0, a1, 2f
    sw      zero, 0(a0)
    addi    a0, a0, 4
    j       1b
2:
    la      t0, freertos_risc_v_trap_handler
    csrw    mtvec, t0
    li      t5, 0x10000000
    li      t6, 66
    sb      t6, 0(t5)
    call    main
3:  j       3b
