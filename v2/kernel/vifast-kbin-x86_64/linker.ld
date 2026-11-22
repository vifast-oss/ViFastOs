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
        /* **KT-276** — trang trampoline SMP: đặt ở địa chỉ **64 KiB-aligned** bằng location
           counter **trong** `.text` (không phải output section riêng, không phải `.p2align 16`
           trong asm). Vì sao: một section/input-section có `sh_addralign = 64 KiB` làm **segment**
           `.text` nhận `align 2**16` ⇒ file offset của segment thành `0x10000`, và **multiboot2
           header** (24 byte đầu `.text`) rơi ra ngoài 32 KiB đầu tệp — vi phạm đặc tả multiboot2.
           `. = ALIGN(64K)` chỉ đệm **bên trong** section nên segment vẫn `align 2**12` và header
           ở file offset `0x1000`.

           Địa chỉ 64 KiB-aligned là điều kiện để `offset label` **16-bit** trong đoạn real mode
           bằng đúng offset trong trang (địa chỉ link `0x110000 + off` cắt còn 16 bit = `off`),
           nên BSP **copy** nguyên trang xuống `0x8000` lúc chạy mà không phải patch địa chỉ.

           Vì sao **không** để linker đặt thẳng ở `0x8000`: đo được QEMU `-kernel` (PVH) có nạp
           segment thấp, nhưng **SeaBIOS chạy sau đó** và ghi đè vùng này — `u32 @ 0x8000` đọc
           trong `kmain` ra `0`. Copy lúc chạy là đường duy nhất còn đúng. */
        . = ALIGN(64K);
        __trampoline_start = .;
        KEEP(*(.trampoline))
        __trampoline_end = .;
        __text_end = .;
    }

    /* `ASSERT` là lỗi **liên kết** nếu trang trampoline không đúng 4 KiB / không 64 KiB-aligned —
       không phải lỗi lúc chạy. */
    ASSERT(__trampoline_end - __trampoline_start == 0x1000,
           "KT-276: trang trampoline phải đúng 4 KiB (một trang SIPI)")
    ASSERT(__trampoline_start % 0x10000 == 0,
           "KT-276: trampoline phải 64 KiB-aligned (offset 16-bit trong real mode)")

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    /* PVH note: section `SHT_NOTE` riêng ⇒ linker sinh `PT_NOTE` cho QEMU. */
    .note.Xen : ALIGN(4) {
        KEEP(*(.note.Xen))
    }

    .data : ALIGN(4K) {
        /* **KT-340a** — ranh giới đoạn cho W^X: `[__text_end, __data_start)` là `.rodata`+note
           (R, NX); `[__data_start, __kernel_end)` là RW NX. Symbol để crate arch **đọc** thay vì
           hardcode địa chỉ. */
        __data_start = .;
        *(.data .data.*)
        /* KT-261: GOT **phải** được giữ. `stub_addr` (crate plat) tham chiếu stub asm qua
           `extern "C"`, và rustc phát sinh tham chiếu GOT-gián-tiếp cho symbol ngoài crate ⇒
           bỏ `.got` đi thì mọi entry thành địa chỉ 0 (đo được: `movq 0x0(%rip)` / `callq *0x0`). */
        *(.got)
        *(.got.plt)
    }

    /* **KT-270** — trang mã **ring 3** (K5' bước 4): section riêng, căn 4 KiB, để ảnh map
       **cùng PA** ở một VA user (`USER_CODE_VA`) với `US = 1`. `KEEP` chống `--gc-sections`
       xoá (không ai tham chiếu từng instruction). Phải vừa **một** Frame 4 KiB — `ASSERT` dưới
       đây là lỗi **liên kết** nếu vượt, không phải lỗi lúc chạy. Nằm **ngoài** `.text` nên
       không đổi `# conf image:` (bản băm `.text` của KT-265). */
    .user : ALIGN(4K) {
        __user_start = .;
        KEEP(*(.user))
        __user_end = .;
    }
    ASSERT(__user_end - __user_start <= 0x1000,
           "KT-270: section .user phải vừa một trang 4 KiB (USER_CODE_VA map một Frame)")

    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* **KT-276** — arena stack **per-hart**: mỗi hart một record `guard 4 KiB | stack 64 KiB`
       (`HART_STRIDE = 0x1_1000`, `MAX_HARTS = 8` ⇒ `0x8_8000` = 544 KiB), khớp TUYỆT ĐỐI với
       `vifast_plat_x86_64::smp` (ảnh PANIC nếu `__smp_stacks_end - __smp_stacks_start` lệch).

       Vì sao NOLOAD: không nằm trong `filesz` của LOAD segment nhưng vẫn chiếm địa chỉ trong
       `[__text_start, __kernel_end)` — identity map 1 GiB của long mode phủ cả khoảng này nên
       stack dùng được ngay sau khi bật `CR0.PG` (bài học KT-46 bên riscv64).

       Hart 0 dùng **slot 0**: `__stack_top = __smp_stacks_start + HART_STRIDE` nên `_start`
       (`mov esp, offset __stack_top`) **không đổi**. `__smp_stacks_start` là gốc arena cho
       trampoline (hart 1) và cho `smp::HartStacks` (kiểm bố cục lúc chạy). */
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

    /* **KT-340b** — bảng trang **boot** (8 × 4 KiB) + **arena bảng trang động** (32 × 4 KiB) ra
       khỏi `.bss`: output section **NOLOAD** đặt ngay sau `.smp_stacks`, vẫn trong
       `[__text_start, __kernel_end)` nên identity map `0..2 MiB` của `_start` phủ tới (điều kiện
       để `_start` 32-bit ghi entry bằng địa chỉ tuyệt đối và `wx::enable` thay `PD[0]`).

       Vì sao NOLOAD: `KT-340c` sẽ link `vifast-core::Machine` (184.792 B) vào `.data/.bss`; trần
       I7 là 262.144 B. Tám bảng trang (32 KiB) là byte chết trong `.bss` — đưa ra NOLOAD đúng
       loại section mà D2-243/D2-244 cho phép (khuôn `.smp_stacks` của KT-276). `_start` **zero**
       cả dải `[__bootpt_start, __bootpt_end)` vì `.bss` zero hộ không phủ section khác.

       Arena 32 frame là nguồn frame của `VSpace` động (`KT-340b`): `PageTable` cấp bảng trang
       **riêng** cho từng tác vụ ring 3 từ đây, không `static` nào trong `.bss`. */
    .bootpt (NOLOAD) : ALIGN(4K) {
        __bootpt_start = .;
        KEEP(*(.bootpt))
        KEEP(*(.bootpt.*))
        . = ALIGN(4K);
        __bootpt_end = .;
    }

    /* **KT-340b** — stack cố định ra khỏi `.bss`: bốn stack IST1 (`#DF` per-hart, KT-278) + trang
       stack user của ring 3 (KT-270). Cùng lý do `.smp_stacks` (KT-276) và cùng lý do D2-243:
       mỗi hart đã có một arena `.smp_stacks` NOLOAD báo riêng, đây là các stack **của riêng**
       ảnh boot — không phải writable data. `.bss` còn lại `__syscall_stack` 32 KiB (giữ có chủ
       đích: D2-244 chỉ cần `.data+.bss < 64 KiB`; xem `main.rs`). */
    .bootstacks (NOLOAD) : ALIGN(4K) {
        __bootstacks_start = .;
        KEEP(*(.bootstacks))
        KEEP(*(.bootstacks.*))
        . = ALIGN(4K);
        __bootstacks_end = .;
    }

    /* **KT-341** — `vifast-core::Machine` (184.792 B) ra khỏi `.data+.bss`: output section
       **NOLOAD** riêng, vẫn trong `[__text_start, __kernel_end)` nên `# core: … in_image=1` và
       identity map phủ tới. `core_bridge::init` zero tường minh cả vùng (NOLOAD không được `.bss`
       zero hộ), nên không dựa vào "QEMU tình cờ trả 0". Đột biến `mut-machine-bss` bỏ thuộc tính
       `link_section` của static ⇒ 184.792 B quay lại `.bss` ⇒ budget ảnh sản phẩm ĐỎ. */
    .corespace (NOLOAD) : ALIGN(4K) {
        __corespace_start = .;
        KEEP(*(.corespace))
        KEEP(*(.corespace.*))
        . = ALIGN(4K);
        __corespace_end = .;
    }

    __kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note.GNU-stack)
    }
}
