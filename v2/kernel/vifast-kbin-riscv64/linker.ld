/* SPDX-License-Identifier: MIT OR Apache-2.0 */
/* Ảnh boot K0.5: OpenSBI (fw_dynamic) nạp trực tiếp vào 0x8020_0000 (thiết kế §5.1). */
OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    . = 0x80200000;

    .text : ALIGN(4K) {
        KEEP(*(.text.entry))
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* Stack boot 64 KiB, NOLOAD — đo RIÊNG, không cộng vào trần .data+.bss (256 KiB [TK]). */
    .stack (NOLOAD) : ALIGN(16) {
        __stack_bottom = .;
        . += 0x10000;
        __stack_top = .;
        __kernel_end = .;
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}
