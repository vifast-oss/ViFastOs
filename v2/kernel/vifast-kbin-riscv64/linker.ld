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
       image ⇒ không thể set NONE riêng. */
    . = ALIGN(2M);

    /* Trang bảo vệ 4 KiB NGAY DƯỚI đáy stack. MMU (KT-46) cưỡng chế bằng entry **NONE**
       (không map) ở cả identity, nửa cao và physmap. NOLOAD nên không tính vào budget. */
    .stack_guard (NOLOAD) : ALIGN(4K) {
        __stack_guard_start = .;
        . += 0x1000;
        __stack_guard_end = .;
    }

    /* Stack boot 64 KiB, NOLOAD — đo RIÊNG, không cộng vào trần 256 KiB (D2-72 Q5).
       Stack mọc xuống (`sp = __stack_top`) nên chạm đáy = vào trang bảo vệ. */
    .stack (NOLOAD) : ALIGN(16) {
        __stack_bottom = .;
        . += 0x10000;
        __stack_top = .;
    }

    /* KT-45: trap stack RIÊNG, NOLOAD — `trap_entry` làm `csrrw sp, sscratch, sp` (sp = đỉnh
       vùng này) rồi `addi sp, sp, -288` để trapframe 288 B nằm TRỌN trong vùng, nên trap không
       bao giờ ghi đè stack của code bị ngắt. Một hart ở KT-45; stack theo hart là KT-49.
       LƯU Ý KT-46: `.trap_stack`/`.stack` là NOLOAD nên **không nằm trong `filesz`** của LOAD
       (LOAD `memsz` vẫn phủ tới `__kernel_end`), nhưng page table kernel **vẫn phải map tường
       minh** cả hai vùng (và arena bắt đầu từ `__kernel_end`, sau vùng này).
       NOLOAD nên KHÔNG cộng vào trần `.data+.bss` (D2-72 Q5); `budget.sh` in riêng như `.stack`. */
    .trap_stack (NOLOAD) : ALIGN(16) {
        __trap_stack_bottom = .;
        . += 0x4000; /* 16 KiB: trapframe 288 B + handler Rust + in log */
        __trap_stack_top = .;
        __kernel_end = .;
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}
