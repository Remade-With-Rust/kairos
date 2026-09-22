/* QEMU `virt`, RV32: one DRAM region at 0x8000_0000, same as the Rust cells'
   memory.x, so both arms are linked at the same address with the same shape. */
OUTPUT_ARCH( "riscv" )
ENTRY( _start )

MEMORY
{
    RAM (rwxa) : ORIGIN = 0x80000000, LENGTH = 16M
}

SECTIONS
{
    .init : { KEEP(*(.init)) } > RAM

    .text : {
        *(.text.unlikely .text.unlikely.*)
        *(.text .text.*)
    } > RAM

    .rodata : {
        . = ALIGN(4);
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    } > RAM

    .data : {
        . = ALIGN(8);
        PROVIDE( __global_pointer$ = . + 0x800 );
        *(.data .data.*)
        *(.sdata .sdata.*)
    } > RAM

    .bss (NOLOAD) : {
        . = ALIGN(4);
        _sbss = .;
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(4);
        _ebss = .;
    } > RAM

    . = ALIGN(16);
    . = . + 0x8000;          /* 32 KiB for the startup stack */
    _stack_top = .;

    /DISCARD/ : { *(.eh_frame .eh_frame.*) *(.comment) }
}
