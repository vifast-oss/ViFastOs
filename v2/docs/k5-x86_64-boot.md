# K5' — ViFastOs **boot thật trên x86_64** (QEMU, long mode)

Trạng thái: **K5' bước 1 DONE** (KT-257) + **K5' bước 2 DONE** (KT-261) + **K5' bước 3 DONE**
(KT-266) — Agent 1 (toàn quyền A0), 2026-11-21. Quyết định: **D2-201** (bước 1), **D2-203**
(bước 2), **D2-204** (bước 3).
**Không** tuyên bố K5' đạt: K5' đầy đủ là "như K3 + `-cpu max`" (U-mode/PCI/SMP).

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
| **MMIO map 3..4 GiB** (LAPIC/IOAPIC ngoài identity map 1 GiB) | **DONE** (KT-266) |
| **LAPIC**: `IA32_APIC_BASE`, ID/version, `SVR` (enable + spurious 255), `TPR = 0`, mask `LINT0/1` | **DONE** (KT-266) |
| **IOAPIC**: `IOAPICID`/`IOAPICVER`, **mask toàn bộ** redirection, rồi mở **đúng** IRQ0 cho PIT | **DONE** (KT-266) |
| **LAPIC timer**: `LVT_TIMER` (vector 32, periodic), `TIMER_DIVIDE`, `TIMER_INITIAL_COUNT` + kiểm đếm ngược | **DONE** (KT-266) |
| **Nguồn tick**: PIT kênh 0 mode 2 (100 Hz) → IOAPIC → LAPIC → vector 32; handler đếm tick + `EOI` | **DONE** (KT-266) |
| **IDT** thêm vector 32 (timer) + 255 (spurious); sentinel `isr_unknown` dời 255 → 254 | **DONE** (KT-266) |
| runner + TAP + **9 ca âm** | **DONE** (KT-257/261/266) |
| U-mode, PCI, SMP, `-cpu max` | **chưa** (K5' đầy đủ) |

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

## 5. KT-266 — APIC + timer

### 5.1 MMIO map 3..4 GiB

LAPIC (`0xFEE0_0000`) và IOAPIC (`0xFEC0_0000`) nằm **trên** 3 GiB, ngoài identity map 1 GiB của
KT-257 ⇒ đọc `LAPIC_ID` lần đầu sẽ `#PF`. Ảnh dựng **thêm một** bảng PD (512 × 2 MiB) phủ
`0xC000_0000..0x1_0000_0000`, gắn vào `PDPT[3]`, lá đặt thêm `PCD = 1` (uncached). Identity map
1 GiB **giữ nguyên** — không đổi hợp đồng KT-257.

### 5.2 LAPIC

- Base đọc từ `IA32_APIC_BASE` (MSR `0x1B`, bit 12..51); nếu bit 11 (`EN`) = 0 thì bật lại.
- Đọc `LAPIC_ID` (bit 24..31) và `LAPIC_VERSION` (byte thấp) **trước** khi bật `SVR` — nếu MMIO
  không map thì đây là chỗ fault, không phải "im lặng rồi báo xanh".
- `SVR = 0x1FF` (vector spurious **255** | `APIC_ENABLE` bit 8), `TPR = 0`, mask `LINT0`/`LINT1`.
- **Che 8259 PIC** (`0x21`/`0xA1` = `0xFF`) **trước** `sti`: trên PC, PIC nối vào `LAPIC.LINT0`
  (virtual wire) và BIOS đặt vector IRQ0 = `0x08` — **trùng `#DF`**. Lần chạy đầu KT-266 chết
  đúng như vậy: `# exc: vec=8 err=0x100479 rip=0x8 cs=0x203` (frame lệch vì ngắt vector 8
  **không** đẩy mã lỗi).

### 5.3 IOAPIC

- `IOREGSEL` `0x00` / `IOWIN` `0x10`; đọc `IOAPICID` (bit 24..27) và `IOAPICVER` (version byte
  thấp, số entry = bit 16..23 **+ 1**; QEMU: `ver=0x20 entries=24`).
- **Mask toàn bộ** redirection entry (bit 16 của thanh ghi low), đọc lại từng entry để chứng minh
  mask **thật sự** nằm trong thanh ghi.
- Sau đó mở **đúng một** entry — IRQ0 (pin 2) → vector 32, dest = LAPIC 0 — làm nguồn tick.

### 5.4 Timer: LAPIC timer **cấu hình + kiểm**, PIT **là nguồn tick**

Đo trên QEMU 11.1.3 TCG: LAPIC timer đặt `initial_count = 100_000`, chia 1 (chu kỳ danh định
100 µs theo mô hình QEMU `(count+1) ns`) nhưng **khoảng cách giữa hai ngắt đo được** là
**2,8–38,5 ms** và đổi giữa các lần boot — QEMU chỉ xử lý timer ở ranh giới vòng lặp chính, không
theo từng chu kỳ. Vì vậy chọn **phương án dự phòng đã ghi trong KT-266**:

| Thành phần | Vai trò |
|---|---|
| LAPIC timer (`LVT_TIMER` vector 32 periodic, `TIMER_DIVIDE` = 1, `TIMER_INITIAL_COUNT` = 1_000_000) | **cấu hình đủ ba thanh ghi** + kiểm `CURRENT_COUNT` **giảm** (bằng chứng timer chạy) rồi **mask** |
| PIT kênh 0 **mode 2** (rate generator), divisor `11_932` ⇒ **100 Hz** | **nguồn tick** |
| IOAPIC redirection IRQ0 (pin 2) → vector 32 | đưa tick vào LAPIC |
| LAPIC `SVR`/`EOI` | nhận ngắt + `EOI` mỗi tick |
| PIT **kênh 2** mode 2, divisor `0xFFFF` | **đồng hồ đo** độc lập (không dùng làm tick) |

- Handler vector 32: tăng `TICK_COUNT`, ghi `EOI`, tăng `EOI_COUNT`, `iretq` — **không** in dòng
  `# exc:` (một dòng mỗi tick sẽ nghẽn COM1 và biến phép đo thành đo tốc độ UART).
- Vector 255 (spurious): chỉ đếm, **không** `EOI` (SDM Vol 3A §10.9).
- `hz` **đo** bằng PIT kênh 2 trong lúc chờ ba mốc tick; dưới TCG giá trị đo được **thấp hơn**
  100 Hz danh định (đo được 46–63 Hz) và đổi giữa các lần boot — đó là số thật, không hardcode.
- Ba mốc tick: mỗi mốc chờ `TICK_COUNT` tăng thêm 1 (có trần), đòi **tăng nghiêm ngặt** và mỗi
  chênh lệch ≤ 1_000. Số tick **tuyệt đối** không phải hợp đồng.
- `EOI` == tick: handler ghi EOI **mỗi** tick, không thừa không thiếu.

### 5.5 IDT: vector 32 + 255, sentinel dời 255 → 254

- Thêm stub `vifast_isr_timer` (vector 32) và `vifast_isr_spurious` (vector 255), slot 15/16;
  `STUB_COUNT` 15 → 17.
- `VEC_UNKNOWN` (sentinel của `isr_unknown`) dời **255 → 254**: từ bước 3, 255 là vector **thật**
  của spurious, nên sentinel phải khác nó — nếu không, handler không phân biệt được "ngắt
  spurious" với "vector lạ".

## 6. Đo được

```text
TAP version 13
1..11
# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x0000000000106000 efer=0x0000000000000500 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x106000
# viFastOs x86_64: mmio map 0xc0000000..0x100000000 pages=512 pcd=1
# viFastOs x86_64: gdt base=0x104088 limit=55 tr=0x28 tss=0x10c064 ist1=0x10b000
ok 1 - exc: gdt loaded
# viFastOs x86_64: idt base=0x10b058 limit=4095 entries=256 df_ist=1
ok 2 - exc: idt loaded
# exc: vec=3 err=0x0 rip=0x1002d1 cs=0x8 rflags=0x2
ok 3 - exc: int3
# exc: vec=6 err=0x0 rip=0x1002e0 cs=0x8 rflags=0x10002
ok 4 - exc: ud2
# exc: vec=14 err=0x0 rip=0x1002f0 cs=0x8 rflags=0x10002
# exc: cr2=0x700000000000
ok 5 - exc: page fault
# exc: vec=13 err=0x38 rip=0x100304 cs=0x8 rflags=0x10002
ok 6 - exc: gp
# exc: vec=254 err=0x0 rip=0x100312 cs=0x8 rflags=0x2
ok 7 - exc: unknown
# apic: id=0 version=0x14 svr=0x1ff
ok 8 - apic: lapic id
# apic: ioapic id=0 ver=0x20 entries=24
ok 9 - apic: ioapic
# timer: lapic count0=964000 count1=468000 runs=1 ioapic_route=1 nominal_hz=100 pit=0x2e76->0x29f0
# timer: hz=63 ticks=3 window_pit=56332
ok 10 - timer: ticks
# timer: eoi=3 ticks=3 spurious=0
ok 11 - timer: eoi
# viFastOs x86_64: exc count=5 last_vec=254
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
- `# exc: vec=254` = đường `isr_unknown` (mọi vector không có stub riêng vẫn **thấy được**);
  sentinel dời 255 → 254 vì 255 là vector thật của spurious (KT-266).
- `mmio map 0xc0000000..0x100000000 pages=512 pcd=1` = bảng PD thứ hai cho LAPIC/IOAPIC.
- `# apic: id=0 version=0x14 svr=0x1ff` = `LAPIC_ID`/`LAPIC_VERSION` đọc từ MMIO; `SVR` đọc lại
  có bit 8 (enable) + vector 255.
- `# apic: ioapic id=0 ver=0x20 entries=24` = `IOAPICID`/`IOAPICVER` đọc qua `IOREGSEL`/`IOWIN`.
- `# timer: lapic … runs=1 ioapic_route=1` = `CURRENT_COUNT` **giảm** (LAPIC timer chạy) và
  redirection IRQ0 trỏ vector 32, không mask; `pit=0x2e76->0x29f0` = counter PIT kênh 0 đang đếm.
- `# timer: hz=63 ticks=3 window_pit=56332` = `hz` **đo** bằng PIT kênh 2 trong cửa sổ ba mốc
  tick; `ticks` tăng nghiêm ngặt (3 mốc). Dưới TCG `hz` thấp hơn 100 Hz danh định.
- `# timer: eoi=3 ticks=3 spurious=0` = số `EOI` **bằng** số tick; không có ngắt spurious.
- `rflags=0x10002` ở các fault = bit `RF` (resume flag) do CPU đặt khi vào fault — đúng SDM.
- `objdump -f`: `start address: 0x100018` == symbol `_start`; `.note.Xen` trong `PT_NOTE`.

## 7. Runner + ca âm

```bash
bash v2/test/boot/run-x86_64.sh     # build + boot + 11 kiểm + 9 ca âm
```

- **11/11 kiểm sạch** (chạy lại 3 boot: 11/11).
- **9 ca âm ĐỎ**: (a) `EXPECT` sai, (b) **ảnh thật** build `--features wrong-marker` in `boot BAD`
  (log QEMU thật, không sửa tay), (c) mất marker, (d) log rỗng, (e) marker 2 lần,
  (f) **mất dòng `# exc:`** (KT-261), (g) **sai vector** (KT-261), (h) **mất dòng `# timer:`**
  (KT-266), (i) **mất dòng `# apic:`** (KT-266).
- Nối vào cổng: bước **59** `QEMU x86_64 (K5' bước 1)` — chạy ở **cả T1 và T2** (tên bước giữ
  nguyên theo `v2/bin/v2`; nội dung đã gồm bước 1+2+3).

## 8. Đánh đổi đã ghi nhận (D2-201, D2-203, D2-204)

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
5. **Nguồn tick là PIT, không phải LAPIC timer** (KT-266, D2-204). LAPIC timer vẫn được cấu hình
   đủ ba thanh ghi và **kiểm đếm ngược**, nhưng QEMU TCG giao ngắt của nó không đều (2,8–38,5 ms
   cho chu kỳ danh định 100 µs) ⇒ dùng PIT kênh 0 mode 2 (100 Hz) làm nguồn tick, LAPIC nhận
   ngắt qua IOAPIC và ghi `EOI`. Đây là **phương án dự phòng đã ghi trong KT-266**, không phải
   nới ngưỡng: ca kiểm vẫn đòi tick tăng thật + `EOI` == tick.
6. **Che 8259 PIC** (`0x21`/`0xA1` = `0xFF`) trước `sti`: BIOS đặt vector IRQ0 = `0x08` (trùng
   `#DF`) và PIC nối vào `LAPIC.LINT0` (virtual wire). Không che thì ảnh chết trong `#DF` ngay
   sau `sti` (đo được ở lần chạy đầu KT-266).

## 9. Còn thiếu (K5' đầy đủ)

1. **IPI**, **SMP** (APIC đã có LAPIC/IOAPIC/timer ở bước 3).
2. **U-mode** + quy ước gọi `[calling.x86_64]` (chưa mở trong `spec/abi.toml`) — GDT đã có
   descriptor user và TSS đã có `RSP0`, nhưng chưa có đường vào CPL3.
3. **PCI/ECAM**, virtio trên x86.
4. **`-cpu max`**, KPTI (tuỳ chọn), soak 1.000.
5. **Nợ N5 của v1** (x86 AP→BSP + PCID) — mở ở K5' đầy đủ, không phải bước này.
