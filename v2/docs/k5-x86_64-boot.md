# K5' — ViFastOs **boot thật trên x86_64** (QEMU, long mode)

Trạng thái: **K5' bước 1 DONE** (KT-257) + **K5' bước 2 DONE** (KT-261) — Agent 1 (toàn quyền A0),
2026-11-21. Quyết định: **D2-201** (bước 1), **D2-203** (bước 2).
**Không** tuyên bố K5' đạt: K5' đầy đủ là "như K3 + `-cpu max`" (APIC/timer/U-mode/PCI/SMP).

Nguồn sự thật: code + log trong `v2/agent/JOURNAL/` (KT-257, KT-261) và
`v2/test/boot/run-x86_64.sh`.

## 1. Phạm vi

| Việc | Trạng thái |
|---|---|
| crate ảnh `vifast-kbin-x86_64` + linker + `_start` 32-bit | **DONE** (KT-257) |
| multiboot2 header **và** PVH note (xem §2) | **DONE** (KT-257) |
| long mode (PML4/PDPT/PD, identity map 0..1 GiB) | **DONE** (KT-257) |
| COM1 serial + halt (ACPI shutdown) | **DONE** (KT-257) |
| **GDT 7 entry** (null, kernel code/data, user code/data, TSS 16 byte) + `lgdt`/`ltr` | **DONE** (KT-261) |
| **TSS** 104 byte: `RSP0` + `IST1` cho `#DF` + `I/O map base = 104` | **DONE** (KT-261) |
| **IDT 256 ô × 16 byte**, 14 stub riêng + `isr_unknown`, `lidt` | **DONE** (KT-261) |
| **Handler exception** in `# exc: vec=… err=… rip=… cs=… rflags=…` (+ `cr2` cho `#PF`) | **DONE** (KT-261) |
| **5 exception đo được**: `int3`, `ud2`, `#PF`, `#GP`, `isr_unknown` — resume bằng `iretq` | **DONE** (KT-261) |
| runner + TAP + 7 ca âm | **DONE** (KT-257/261) |
| APIC, timer, U-mode, PCI, SMP, `-cpu max` | **chưa** (K5' đầy đủ) |

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
  512 trang) → `CR4.PAE` → `CR3` → `EFER.LME` → `CR0.PG|PE` → `lgdt` (GDT **tạm** 3 entry) →
  `ljmp` sang 64-bit → `kmain`.
- **`-no-pie`** trong `build.rs`: target `x86_64-unknown-none` mặc định liên kết PIE (đo được: ELF
  `DYNAMIC`, entry `0x1274`) ⇒ không dùng được cho ảnh nạp ở 1 MiB.
- **COM1** `0x3F8`: `write_byte` chờ `LSR.THRE` (bit 5), có trần; init 115200 8N1.
- **Halt**: ACPI shutdown (`outb(0x604, 0x2000)`) ⇒ QEMU thoát mã 0.

## 4. KT-261 — GDT/TSS/IDT + handler exception

### 4.1 GDT 7 entry

| # | Nội dung | Selector | Access | Flags |
|---|---|---|---|---|
| 0 | null (bắt buộc) | — | `0x00` | `0x0` |
| 1 | kernel code 64-bit | `0x08` | `0x9A` | `0x2` (`L=1`, `D/B=0`) |
| 2 | kernel data | `0x10` | `0x92` | `0x0` |
| 3 | user code 64-bit | `0x18` (`0x1B` + RPL 3) | `0xFA` | `0x2` |
| 4 | user data | `0x20` (`0x23` + RPL 3) | `0xF2` | `0x0` |
| 5–6 | **TSS** (descriptor hệ thống 16 byte) | `0x28` | `0x89` | `0x0` |

- Bảng nằm ở `vifast_plat_x86_64::gdt` dưới dạng `[AtomicU64; 7]`: ghi/đọc **an toàn** (không
  `unsafe`), layout `AtomicU64` **đúng bằng** `u64` nên `lgdt` đọc được từng entry.
- `_start` vẫn dùng **GDT tạm** 3 entry để vào long mode (cùng selector, cùng giá trị descriptor);
  `kmain` nạp bảng **thật** 7 entry sau khi đã có Rust để dựng descriptor TSS.
- **`ltr` đánh dấu descriptor TSS là busy**: CPU tự đổi `type` `0x9 → 0xB` (byte `0x89 → 0x8B`,
  SDM Vol 3A §7.2.2). Ảnh **kiểm đúng dạng busy** — đó là bằng chứng `ltr` đã nạp thật, không chỉ
  "đã gọi".

### 4.2 TSS 104 byte

- `RSP0` = đỉnh stack kernel (`__stack_top`), `IST1` = đỉnh stack **riêng** 4 KiB cho `#DF`,
  `I/O map base = 104` (không bitmap ⇒ mọi I/O từ CPL3 đều `#GP`).
- Vùng nhớ là `[AtomicU32; 26]`: `RSP0` ở offset **4** (không căn 8) nên `#[repr(C)]` + `AtomicU64`
  sẽ bị chèn đệm và lệch layout, còn `#[repr(C, packed)]` + atomic là **UB**. Mọi trường TSS đều
  căn **4** ⇒ `[AtomicU32; 26]` phủ đúng layout, mỗi trường 64-bit ghi bằng hai `u32`.
- Bước này **cấu hình** `IST1` và **kiểm** nó (descriptor IDT của vector 8 + trường `IST1` của
  TSS); **không** kích hoạt `#DF` — một `#DF` thật không "thu hồi" được và không cần cho tiêu chí.

### 4.3 IDT 256 ô × 16 byte

- Mọi ô `P=1`; **14 vector** có stub riêng (`#DE` 0, `#DB` 1, `#BP` 3, `#UD` 6, `#DF` 8, `#TS` 10,
  `#NP` 11, `#SS` 12, `#GP` 13, `#PF` 14, `#AC` 17, `#CP` 21, `#VC` 29, `#SX` 30), còn lại trỏ
  `isr_unknown` (đẩy sentinel **255**). Ô chưa điền (`P=0`) sẽ sinh `#GP` **trong lúc xử lý
  exception** ⇒ `#DF` ⇒ treo im lặng, nên điền **cả 256** ô là bắt buộc.
- Stub chia **hai họ** theo tập vector có mã lỗi (SDM Bảng 6-1: 8, 10, 11, 12, 13, 14, 17, 21, 29,
  30): họ `isr_err_*` **không** đẩy gì thêm (CPU đã đẩy mã lỗi), họ `isr_noerr_*` đẩy `0` giả.
  Tập này là **dữ liệu** ở `vifast_plat_x86_64::exc` (có test host) — lệch một vector là frame lệch
  8 byte và handler đọc `rip` từ chỗ của `cs`.
- Stub chung đẩy **15 GPR** (120 byte) rồi gọi `exc_dispatch(vec, err, rip, cs, rflags) -> u64`.
  Handler trả **`rip` mới** ⇒ stub `iretq`; trả **`0`** ⇒ stub `cli; hlt` (halt sạch).
- Handler nhận **giá trị** chứ không nhận con trỏ frame: ảnh **không** được có `unsafe` (R5) nên
  không thể deref con trỏ; stub asm đọc 5 trường của frame và truyền thành 5 tham số SysV.

### 4.4 Resume: `INT3`/`INT n` khác fault

- `INT3` và `INT n` là **software interrupt**: CPU lưu RIP của lệnh **kế tiếp** (SDM Bảng 6-1) ⇒
  handler resume **nguyên** RIP.
- `#UD`/`#PF`/`#GP` là **fault**: CPU lưu RIP của **chính** lệnh gây lỗi ⇒ `#UD` resume `rip + 2`
  (`ud2` = `0F 0B`), còn `#PF`/`#GP` nhảy tới **nhãn resume** do ca kiểm đặt (độ dài lệnh gây lỗi
  không biết chắc).
- Đo được (lần đầu làm sai): resume `int3` bằng `rip + 1` **nhảy qua** `ret` và rơi vào padding
  `nop` cho tới `ud2` kế tiếp — log có `# exc: vec=6` ngay sau `vec=3` và **không** có `ok 3`.

### 4.5 `#GP` sinh **chắc chắn**

Nạp `DS` bằng selector **ngoài** GDT (`0x38` = index 7, bảng có 7 entry ⇒ limit 55) ⇒ `#GP(0x38)`
theo SDM Vol 3A §5.4.1. Cách này **không đổi trạng thái** (DS giữ nguyên khi fault) và **không**
phụ thuộc hành vi QEMU với `mov cr0` (nếu QEMU không fault thì paging bị tắt — thảm hoạ).

## 5. Đo được

```text
TAP version 13
1..7
# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x0000000000105000 efer=0x0000000000000500 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x105000
# viFastOs x86_64: gdt base=0x103088 limit=55 tr=0x28 tss=0x10a044 ist1=0x109000
ok 1 - exc: gdt loaded
# viFastOs x86_64: idt base=0x109038 limit=4095 entries=256 df_ist=1
ok 2 - exc: idt loaded
# exc: vec=3 err=0x0 rip=0x100281 cs=0x8 rflags=0x2
ok 3 - exc: int3
# exc: vec=6 err=0x0 rip=0x100290 cs=0x8 rflags=0x10002
ok 4 - exc: ud2
# exc: vec=14 err=0x0 rip=0x1002a0 cs=0x8 rflags=0x10002
# exc: cr2=0x700000000000
ok 5 - exc: page fault
# exc: vec=13 err=0x38 rip=0x1002b4 cs=0x8 rflags=0x10002
ok 6 - exc: gp
# exc: vec=255 err=0x0 rip=0x1002c2 cs=0x8 rflags=0x2
ok 7 - exc: unknown
# viFastOs x86_64: exc count=5 last_vec=255
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

- `lma=1` **đọc lại** từ `IA32_EFER` sau khi bật (không chỉ tin asm).
- `pvh_magic=0x336ec578` chứng minh giao thức PVH thật.
- `gdt … tr=0x28` = `sgdt`/`str` đọc lại **khớp** bảng; `tss=0x10a044` = descriptor TSS trỏ đúng
  TSS; `ist1=0x109000` = `IST1` đã nạp.
- `idt … limit=4095 entries=256 df_ist=1` = `sidt` đọc lại khớp bảng; `#DF` có `IST1`.
- `# exc: vec=3 … rip=0x100281` = RIP **sau** `int3` (software interrupt) — đúng SDM.
- `# exc: vec=14 … cr2=0x700000000000` = `#PF` với `P=0` (err `0x0`) và `CR2` đúng địa chỉ đã đọc.
- `# exc: vec=13 err=0x38` = `#GP` với mã lỗi là selector ngoài GDT.
- `# exc: vec=255` = đường `isr_unknown` (mọi vector không có stub riêng vẫn **thấy được**).
- `rflags=0x10002` ở các fault = bit `RF` (resume flag) do CPU đặt khi vào fault — đúng SDM.
- `objdump -f`: `start address: 0x100018` == symbol `_start`; `.note.Xen` trong `PT_NOTE`.

## 6. Runner + ca âm

```bash
bash v2/test/boot/run-x86_64.sh     # build + boot + 9 kiểm + 7 ca âm
```

- **9/9 kiểm sạch** (chạy lại 3 boot: 9/9).
- **7 ca âm ĐỎ**: (a) `EXPECT` sai, (b) **ảnh thật** build `--features wrong-marker` in `boot BAD`
  (log QEMU thật, không sửa tay), (c) mất marker, (d) log rỗng, (e) marker 2 lần,
  (f) **mất dòng `# exc:`** (KT-261), (g) **sai vector** (KT-261).
- Nối vào cổng: bước **59** `QEMU x86_64 (K5' bước 1+2)` — chạy ở **cả T1 và T2**.

## 7. Đánh đổi đã ghi nhận (D2-201, D2-203)

1. **`vifast-plat-x86_64`: `forbid(unsafe_code)` → `deny(unsafe_code)`**, chỉ `asm.rs` có
   `#![allow(unsafe_code)]`. Lý do: `extern "C" { fn }` trong Rust **luôn** phải gọi trong `unsafe`
   ⇒ nếu để asm trong crate ảnh thì buộc phải `unsafe` ở đó (vi phạm R5). Đúng D2-17 và **y hệt**
   khuôn `vifast-plat-aarch64` (KT-167). **Không** phải nới luật.
2. **`-no-pie`** là cờ liên kết, không đổi mã nguồn.
3. **`.got` phải được GIỮ trong `linker.ld`** (KT-261). Bản KT-257 `DISCARD` cả `.got`/`.got.plt`
   vì ảnh chưa dùng; KT-261 tham chiếu stub asm qua `extern "C"` từ crate khác, và rustc phát sinh
   tham chiếu **GOT-gián-tiếp** cho symbol ngoài crate ⇒ bỏ `.got` thì mọi entry thành **địa chỉ
   0** (đo được: `movq 0x0(%rip), %r8` và `callq *0x0(%rip)`), `stub_addr` trả rác và IDT trỏ vào
   rác. Đã chuyển `.got`/`.got.plt` vào output section `.data`.
4. **`x86_64-unknown-none` tắt red zone** (đo được: hàm lá vẫn `sub rsp, 0x18`), nên stub chung
   được phép `and rsp, -16` để căn stack trước `call` mà không đạp dữ liệu sống dưới `rsp`.

## 8. Còn thiếu (K5' đầy đủ)

1. **APIC** (LAPIC/IOAPIC), timer, IPI, SMP.
2. **U-mode** + quy ước gọi `[calling.x86_64]` (chưa mở trong `spec/abi.toml`) — GDT đã có
   descriptor user và TSS đã có `RSP0`, nhưng chưa có đường vào CPL3.
3. **PCI/ECAM**, virtio trên x86.
4. **`-cpu max`**, KPTI (tuỳ chọn), soak 1.000.
5. **Nợ N5 của v1** (x86 AP→BSP + PCID) — mở ở K5' đầy đủ, không phải bước này.
