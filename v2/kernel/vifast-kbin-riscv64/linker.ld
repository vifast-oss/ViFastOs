/* SPDX-License-Identifier: MIT OR Apache-2.0 */
/* Ảnh K3 (KT-44): OpenSBI `fw_dynamic` nạp trực tiếp vào 0x8020_0000 (D2-72 Q3).
   `.stack` và trang bảo vệ là NOLOAD — báo RIÊNG, không cộng vào trần
   `.data + .sdata + .bss + .sbss <= 262.144 B` (D2-72 Q5). */
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

    /* KT-46: đặt guard/stack/trap_stack/arena sang **2 MiB kế tiếp** (0x8040_0000) để miền
       kernel image `[0x8020_0000, 0x8040_0000)` map được bằng **một leaf 2 MiB** ở nửa cao
       (yêu cầu "kernel nửa cao 2 MiB"), còn guard page vẫn cưỡng chế được ở granularity 4 KiB
       trong vùng NOLOAD phía sau. Nếu bỏ ALIGN(2M) này, guard nằm trong chính leaf 2 MiB của
       image ⇒ không thể set NONE riêng.
       KT-49: vùng này là **MAX_HARTS = 8** record per-hart, mỗi record
         guard 4 KiB | stack 64 KiB | trap stack 16 KiB  (HART_STRIDE = 0x1_5000)
       — khớp TUYỆT ĐỐI với `vifast_plat_common::smp` (mmu::init panic nếu lệch). */
    . = ALIGN(2M);
    __hart_area_base = .;

    /* ---- hart 0: giữ tên symbol cũ (KT-46 dùng `__stack_*`/`__trap_stack_*` cho hart 0). */
    .stack_guard (NOLOAD) : ALIGN(4K) {
        __stack_guard_start = .;
        . += 0x1000;
        __stack_guard_end = .;
    }

    .stack (NOLOAD) : ALIGN(16) {
        __stack_bottom = .;
        . += 0x10000;
        __stack_top = .;
    }

    .trap_stack (NOLOAD) : ALIGN(16) {
        __trap_stack_bottom = .;
        . += 0x4000;
        __trap_stack_top = .;
    }

    /* ---- hart 1..7: cùng khuôn record, chỉ cấp địa chỉ (NOLOAD); guard/stack/trap stack của
       từng hart tính bằng `__hart_area_base + hart*HART_STRIDE` trong Rust. */
    .hart_stacks (NOLOAD) : ALIGN(16) {
        . += 7 * 0x15000;
    }

    __kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}
