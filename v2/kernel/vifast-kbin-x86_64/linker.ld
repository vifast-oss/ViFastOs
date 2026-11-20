/* SPDX-License-Identifier: MIT OR Apache-2.0 */
/* Ảnh boot x86_64 K5' bước 1 (KT-257) — QEMU `-kernel` nạp ELF trực tiếp.
 *
 * # Vì sao `0x0010_0000` (1 MiB)
 *
 * Vùng dưới 1 MiB của PC chứa IVT/BDA/EBDA, option ROM và framebuffer VGA — không dùng làm chỗ
 * nạp ảnh. 1 MiB là mốc chuẩn của kernel 32-bit (và là `BOOT_LOAD_ADDR` trong
 * `vifast_plat_x86_64::boot`), nằm trong 128 MiB RAM của QEMU `-m 128M`.
 *
 * # Vì sao `.multiboot` đứng **đầu** `.text`
 *
 * Multiboot2 đòi header nằm trong **32 KiB đầu của tệp** và căn 8 byte. `.text` là section đầu
 * tiên ⇒ file offset `0x1000` (4 KiB) — thoả cả hai. Header do asm phát ra, linker chỉ `KEEP`.
 *
 * # Vì sao `.note.Xen` là section **riêng**
 *
 * QEMU 11.1.3 `-kernel` cho x86 **không** đọc multiboot2 (xem `vifast_plat_x86_64::boot`): nó
 * đọc note `"Xen"` type 18 trong một **`PT_NOTE` segment** để lấy địa chỉ entry 32-bit. Section
 * phải có type `SHT_NOTE` (asm khai `@note`) để linker sinh `PT_NOTE`; nhét nó vào `.rodata`
 * thường thì QEMU **không thấy** và báo "without PVH ELF Note".
 *
 * # `.stack` là NOLOAD
 *
 * Stack 64 KiB không nằm trong `filesz` của LOAD segment (không tốn chỗ trong ảnh) nhưng vẫn
 * chiếm địa chỉ trong `[__text_start, __kernel_end)` — identity map 1 GiB của long mode phủ cả
 * khoảng này nên stack dùng được ngay sau khi bật `CR0.PG` (bài học KT-46 bên riscv64).
 */
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

SECTIONS
{
    . = 0x00100000;

    .text : ALIGN(4K) {
        __text_start = .;
        /* Multiboot2 header — 24 byte, căn 8, phải ở đầu tệp. */
        KEEP(*(.multiboot))
        /* `_start` 32-bit: `ENTRY(_start)` giữ nó, nhưng đặt tường minh để `objdump -f` và
           `_start` không phụ thuộc thứ tự section của rustc. */
        KEEP(*(.text.entry))
        *(.text .text.*)
        __text_end = .;
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    /* PVH note: section `SHT_NOTE` riêng ⇒ linker sinh `PT_NOTE` cho QEMU. */
    .note.Xen : ALIGN(4) {
        KEEP(*(.note.Xen))
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

    /* Stack 64 KiB, NOLOAD: `_start` đặt `esp`/`rsp` = `__stack_top` trước mọi lệnh dùng stack. */
    .stack (NOLOAD) : ALIGN(16) {
        __stack_bottom = .;
        . += 0x10000;
        __stack_top = .;
    }

    __kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note.GNU-stack)
        *(.got)
        *(.got.plt)
    }
}
