/* SPDX-License-Identifier: MIT OR Apache-2.0 */
/* Ảnh boot aarch64 K4' bước 3 (KT-247) — QEMU `virt` nạp ELF trực tiếp qua `-kernel`.
 *
 * Vì sao `0x4008_0000`: RAM của QEMU `virt` (aarch64) bắt đầu ở `0x4000_0000` và 128 MiB đầu
 * (tới `0x4800_0000`) là RAM phẳng; `0x4008_0000` (512 KiB vào RAM) chừa chỗ cho các cấu trúc
 * QEMU/firmware đặt ở đầu RAM và **trùng** địa chỉ nạp mà `run.sh` riscv64 dùng cho OpenSBI —
 * không có lý do kỹ thuật để lệch, và giữ cùng mốc giúp so log hai arch.
 *
 * `.stack` + guard là **NOLOAD**: không nằm trong `filesz` của LOAD segment, nhưng vẫn chiếm địa
 * chỉ trong `[__kernel_start, __kernel_end)` — MMU của KT-249 map cả khoảng này nên stack dùng
 * được sau khi bật (bài học KT-46 bên riscv64: NOLOAD **phải** nằm trong vùng được map).
 */
OUTPUT_ARCH(aarch64)
ENTRY(_start)

SECTIONS
{
    . = 0x40080000;

    .text : ALIGN(4K) {
        __text_start = .;
        KEEP(*(.text.entry))
        *(.text .text.*)
        __text_end = .;
    }

    /* KT-248: bảng vector 16 ô × 0x80, cả bảng căn 0x800 (`VBAR_EL1` chỉ giữ bit [63:11]).
       **Output section riêng** (không nhét vào `.text`) để `objdump -h` chứng minh được nó là
       `TEXT` chứ **không** phải `BSS`: `.bss` là NOLOAD nên bảng nằm trong `.bss` sẽ **không**
       có trong ảnh đã nạp. `KEEP` chống `--gc-sections` xoá (không ai tham chiếu từng ô).
       `__vector_base` do **linker** cấp; asm của ảnh không định nghĩa lại. */
    .text.vectors : ALIGN(0x800) {
        __vector_base = .;
        KEEP(*(.vectors))
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* **KT-259** — arena stack **per-hart**: mỗi hart một record `guard 4 KiB | stack 64 KiB`
       (`HART_STRIDE = 0x1_1000`, `MAX_HARTS = 8` ⇒ `0x8_8000` = 544 KiB), khớp TUYỆT ĐỐI với
       `vifast_plat_aarch64::smp` (ảnh PANIC nếu `__smp_stacks_end - __smp_stacks_start` lệch).

       Vì sao NOLOAD: cùng lý do `.stack` cũ — không nằm trong `filesz` của LOAD segment nhưng
       vẫn chiếm địa chỉ trong `[__kernel_start, __kernel_end)` nên MMU của KT-249 map cả khoảng
       này và stack dùng được sau khi bật (bài học KT-46 bên riscv64: NOLOAD **phải** nằm trong
       vùng được map).

       Hart 0 dùng **slot 0**: `__stack_bottom`/`__stack_top` trỏ vào slot ấy nên `_start` và
       bước nhảy nửa cao của KT-253 **không đổi**. `__smp_stacks_start` là gốc arena cho
       `_start_secondary` (hart 1..7) và cho `smp::HartStacks` (kiểm bố cục lúc chạy). */
    .smp_stacks (NOLOAD) : ALIGN(4K) {
        __smp_stacks_start = .;
        /* slot 0 = hart 0: guard | stack */
        . += 0x1000;
        __stack_bottom = .;
        . += 0x10000;
        __stack_top = .;
        /* slot 1..7 — cùng khuôn record, chỉ cấp địa chỉ (NOLOAD). */
        . += 7 * 0x11000;
        __smp_stacks_end = .;
    }

    __kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.ARM.exidx*)
        *(.ARM.attributes)
    }
}
