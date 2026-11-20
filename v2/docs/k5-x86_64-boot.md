# K5' bước 1 — ViFastOs **boot thật trên x86_64** (QEMU, long mode)

Trạng thái: **K5' bước 1 DONE** — Agent 1 (toàn quyền A0), 2026-11-21. Quyết định: **D2-201**.
**Không** tuyên bố K5' đạt: K5' đầy đủ là "như K3 + `-cpu max`" (IDT/GDT/APIC/timer/U-mode/PCI/SMP).

Nguồn sự thật: code + log trong `v2/agent/JOURNAL/` (KT-257) và `v2/test/boot/run-x86_64.sh`.

## 1. Phạm vi

| Việc | Trạng thái |
|---|---|
| crate ảnh `vifast-kbin-x86_64` + linker + `_start` 32-bit | **DONE** |
| multiboot2 header **và** PVH note (xem §2) | **DONE** |
| long mode (PML4/PDPT/PD, identity map 0..1 GiB) | **DONE** |
| COM1 serial + halt (ACPI shutdown) | **DONE** |
| runner + TAP + 5 ca âm | **DONE** |
| IDT/GDT, APIC, timer, U-mode, PCI, SMP, `-cpu max` | **chưa** (K5' đầy đủ) |

## 2. Phát hiện quan trọng: QEMU x86 `-kernel` **không** nhận multiboot2

Đọc mã QEMU 11.1.3 (`hw/i386/multiboot.c`, `hw/i386/x86-common.c`) + đo thật:

- `-kernel` cho x86 **chỉ** tìm magic multiboot **1** (`0x1BADB002`) và **từ chối thẳng ELF64**
  (`"Cannot load x86-64 image, give a 32bit one."`).
- Magic multiboot2 (`0xE85250D6`) **không** được tìm ở đâu ⇒ ảnh ELF64 + multiboot2 **không thể**
  boot bằng `-kernel`.
- Đường QEMU thật sự nhận ELF64 là **PVH** (`load_elfboot`): note `"Xen"` type 18 trong `PT_NOTE`,
  desc 4 byte = entry 32-bit.

⇒ Ảnh mang **cả hai**: multiboot2 header (đúng đặc tả, để GRUB nạp được) **và** PVH note (đường
`-kernel` chạy thật). `_start` không phụ thuộc giao thức nào.

## 3. Đường boot

```text
qemu-system-x86_64 -m 128M -nographic -kernel <ELF> -no-reboot
```

- Linker: `. = 1M` (1 MiB); multiboot header nằm trong 8 KiB đầu tệp.
- `_start` (32-bit): đặt stack → dựng PML4/PDPT/PD (2 MiB pages, identity map `0x0..0x4000_0000`,
  512 trang) → `CR4.PAE` → `CR3` → `EFER.LME` → `CR0.PG|PE` → `ljmp` sang 64-bit → `kmain`.
- **`-no-pie`** trong `build.rs`: target `x86_64-unknown-none` mặc định liên kết PIE (đo được: ELF
  `DYNAMIC`, entry `0x1274`) ⇒ không dùng được cho ảnh nạp ở 1 MiB.
- **COM1** `0x3F8`: `write_byte` chờ `LSR.THRE` (bit 5), có trần; init 115200 8N1.
- **Halt**: ACPI shutdown (`outb(0x604, 0x2000)`) ⇒ QEMU thoát mã 0.

## 4. Đo được

```text
Booting from ROM..# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x0000000000105000 efer=0x0000000000000500 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x105000
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

- `lma=1` **đọc lại** từ `IA32_EFER` sau khi bật (không chỉ tin asm).
- `pvh_magic=0x336ec578` chứng minh giao thức PVH thật.
- `objdump -f`: `start address: 0x100018` == symbol `_start`; `.note.Xen` trong `PT_NOTE`.

## 5. Runner + ca âm

```bash
bash v2/test/boot/run-x86_64.sh     # build + boot + 7 kiểm + 5 ca âm
```

- **7/7 kiểm sạch** (chạy lại 3 boot: 9/9).
- **5 ca âm ĐỎ**: (a) `EXPECT` sai, (b) **ảnh thật** build `--features wrong-marker` in `boot BAD`
  (log QEMU thật, không sửa tay), (c) mất marker, (d) log rỗng, (e) marker 2 lần.
- Nối vào cổng: bước **59** `QEMU x86_64 (K5' bước 1)` — chạy ở **cả T1 và T2**.

## 6. Đánh đổi đã ghi nhận (D2-201)

1. **`vifast-plat-x86_64`: `forbid(unsafe_code)` → `deny(unsafe_code)`**, chỉ `asm.rs` có
   `#![allow(unsafe_code)]`. Lý do: `extern "C" { fn }` trong Rust **luôn** phải gọi trong `unsafe`
   ⇒ nếu để asm trong crate ảnh thì buộc phải `unsafe` ở đó (vi phạm R5). Đúng D2-17 và **y hệt**
   khuôn `vifast-plat-aarch64` (KT-167). **Không** phải nới luật.
2. **`-no-pie`** là cờ liên kết, không đổi mã nguồn.

## 7. Còn thiếu (K5' đầy đủ)

1. **IDT/GDT/TSS**, xử lý exception, `syscall`/`sysret`.
2. **APIC** (LAPIC/IOAPIC), timer, IPI, SMP.
3. **U-mode** + quy ước gọi `[calling.x86_64]` (chưa mở trong `spec/abi.toml`).
4. **PCI/ECAM**, virtio trên x86.
5. **`-cpu max`**, KPTI (tuỳ chọn), soak 1.000.
6. **Nợ N5 của v1** (x86 AP→BSP + PCID) — mở ở K5' đầy đủ, không phải bước này.
