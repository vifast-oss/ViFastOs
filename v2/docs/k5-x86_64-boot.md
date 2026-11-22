# K5' — ViFastOs **boot thật trên x86_64** (QEMU, long mode)

Trạng thái: **K5' bước 1 DONE** (KT-257) + **K5' bước 2 DONE** (KT-261) + **K5' bước 3 DONE**
(KT-266) + **K5' bước 4 DONE** (KT-270: U-mode ring 3 + `[calling.x86_64]`) + **K5' bước 5 DONE**
(KT-274: PCI ECAM/legacy + virtio-net + ARP) + **K5' bước 6 DONE** (KT-276: SMP 2 hart) +
**K5' bước 7 DONE** (KT-278: SMP **4 hart** + **IPI** + **TSS per-hart**) + **K5' bước 8 DONE**
(KT-286: **preemption** — hai tác vụ kernel + timer chuyển ngữ cảnh) + **K5' bước 9 DONE**
(KT-288: **virtio-blk** — BAR leaf riêng + virtqueue + đọc **sector 0** của đĩa QEMU thật) —
Agent 1 (toàn quyền A0), 2026-11-21. Quyết định: **D2-201** (bước 1), **D2-203** (bước 2),
**D2-204** (bước 3), **D2-207** (bước 4), **D2-208** (bước 5), **D2-209** (bước 6),
**D2-210** (bước 7), **D2-212** (bước 8), **D2-213** (bước 9).
**Không** tuyên bố K5' đạt: K5' đầy đủ là "như K3 + `-cpu max`" (soak 600 boot, `syscall`/`sysret`,
virtio-gpu, MSI-X, DHCP).

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
| **`spec/abi.toml [calling.x86_64]`** mở (đối xứng riscv64/aarch64) | **DONE** (KT-270) |
| **U-mode ring 3**: trang mã/stack `US = 1`, `iretq` xuống CPL3, `int 0x80` (gate `DPL = 3`) → handler → `iretq` về ring 3, `Exit` kết thúc | **DONE** (KT-270) |
| **PCI**: ECAM (q35, `MCFG` từ ACPI) + legacy `0xCF8`/`0xCFC` (pc); quét bus/dev/func, class, BAR, capability list | **DONE** (KT-274) |
| **virtio-net**: 2 split virtqueue, MAC từ `virtio_net_config`, ARP request → **reply thật** của slirp | **DONE** (KT-274) |
| **SMP 2 hart**: trampoline real mode → long mode ở `0x8000`, `INIT`+`SIPI`+`SIPI` qua `ICR`, arena stack per-hart (guard 4 KiB + stack 64 KiB), LAPIC riêng cho hart phụ | **DONE** (KT-276) |
| **SMP 4 hart**: `INIT`+`SIPI`+`SIPI` **tuần tự** cho hart 1..3, chờ `online mask=0xf`, bốn stack **khác slot** | **DONE** (KT-278) |
| **IPI**: vector riêng `0x30` + stub riêng, `ICR` fixed-delivery (`low=0x4030`), hart 1 nhận + đếm trong handler, in `# smp: ipi recv=1 intid=0x30` | **DONE** (KT-278) |
| **TSS per-hart**: mỗi hart một TSS 104 byte + **GDT riêng** + `ltr` trên AP; `IST1` riêng cho `#DF`; kiểm **địa chỉ** (`distinct=Ok(())`) | **DONE** (KT-278) |
| **virtio-blk**: `VirtioKind::Blk` (0x1001/0x1042), BAR firmware gán **rời** BAR net (`bar_distinct=1`), split virtqueue 8 descriptor, đọc **sector 0** của đĩa QEMU thật | **DONE** (KT-288) |
| **tác vụ ring 3 trong scheduler**: `schedule()` chọn nó, tick preempt nó, nó chạy lại ở ring 3 (`# sched: umode task ran n=… ring=3 preempted=… resumed=…` + `# sched: umode progress first=… last=… delta=…`) | **DONE** (KT-304, siết lại KT-308) |
| runner + TAP **26** + **16 ca âm** | **DONE** (KT-257/261/266/270/274/276/278/286/288/304) |
| `syscall`/`sysret`, virtio-gpu trên x86_64, MSI-X, hotplug, DHCP | **chưa** (K5' đầy đủ) |
| `-cpu max` (KT-323: `V2_X64_CPU`, mặc định `qemu64`) | **ĐÃ ĐO** — TAP 30/30 trên cả hai model, 26 ca âm + 1 ca dương, smoke 60/60 |
| soak **600 boot** x86_64 (KT-324) | **ĐẠT** — 600/600, `qemu_foreign=0` hai mẫu, `invalid=0`, `rc_124=0` |

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
1..17
# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x000000000010b000 efer=0x0000000000000d00 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x10b000
# viFastOs x86_64: mmio map 0x80000000..0x100000000 pages=1024 pcd=1
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
# umode: iretq entry=0x4000000000 sp=0x4000002000 rflags=0x202 cs=0x1b ss=0x23 code_va=0x4000000000 code_pa=0x109000 code_bytes=71 stack_va=0x4000001000 stack_pa=0x113000 syscall_stack=0x11c000
ok 12 - umode: ring3 entered
# umode: int80 nr=12 cs=0x1b rip=0x4000000025 user=1
ring3 says hi
ok 13 - umode: syscall handled
# umode: ring3 ran nr=12 ret=14
# umode: int80 ret error=0 value=14 printed=14 ptr=0x4000000039 len=14
ok 14 - umode: returned to ring3
# umode: exit code=0 nr=14
# umode: done
# pci: ecam base=0xb0000000 buses=0..255 functions=6
# pci: dev 00:00.0 vendor=0x8086 device=0x29c0 class=0x0600 bridge hdr=0x00
# pci: dev 00:01.0 vendor=0x1234 device=0x1111 class=0x0300 display hdr=0x00
# pci: dev 00:02.0 vendor=0x1af4 device=0x1000 class=0x0200 network hdr=0x00
# pci: dev 00:1f.0 vendor=0x8086 device=0x2918 class=0x0601 bridge hdr=0x80
# pci: dev 00:1f.2 vendor=0x8086 device=0x2922 class=0x0106 storage hdr=0x80
# pci: dev 00:1f.3 vendor=0x8086 device=0x2930 class=0x0c05 other hdr=0x80
ok 15 - pci: ecam
# pci: net 00:02.0 bar=4 kind=mem64 base=0xfe000000 size=0x4000 command=0x0107 caps=4
# net: mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020
ok 16 - net: mac
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=1705 errors=0 timeouts=0
ok 17 - net: arp
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
bash v2/test/boot/run-x86_64.sh     # build + boot + 18 kiểm + 19 ca âm
```

- **18/18 kiểm sạch** (1 boot + 1 boot ảnh marker sai + 1 boot **không mạng** + 1 boot **không
  đĩa**).
- **19 ca âm ĐỎ**: (a) `EXPECT` sai, (b) **ảnh thật** build `--features wrong-marker` in `boot BAD`
  (log QEMU thật, không sửa tay), (c) mất marker, (d) log rỗng, (e) marker 2 lần,
  (f) **mất dòng `# exc:`** (KT-261), (g) **sai vector** (KT-261), (h) **mất dòng `# timer:`**
  (KT-266), (i) **mất dòng `# apic:`** (KT-266), (j) **mất dòng `# umode:`** (KT-270),
  (k) **không có mạng** (KT-274): boot **thật** không có `-device virtio-net-pci` ⇒ ảnh in
  `# net: no device` rồi TAP 16 đỏ; (l) mất dòng `# smp:` (KT-276), (m) mất dòng
  `# smp: ipi recv=` (KT-278), (n) mất dòng `# sched:` (KT-286); (o) **không có đĩa** (KT-288):
  boot **thật** không có `-drive`/`-device virtio-blk-pci` ⇒ ảnh in `# blk: no device` rồi TAP 25
  đỏ; (p) **tác vụ ring 3 không chạy lại** (KT-304): log tốt bị sửa `resumed=0` ⇒ ĐỎ;
  (q) **preempt không ở ring 3** (KT-308): sửa `cs=0x1b` → `cs=0x8` ⇒ ĐỎ; (r) **bộ đếm giảm**
  (KT-308): sửa `# sched: umode progress` thành `first=100 last=50 delta=-50` ⇒ ĐỎ;
  (s) **tác vụ chưa chạy** (KT-308): sửa `n=<n>` → `n=0` ⇒ ĐỎ. Ba ca
  (k)/(o) đòi **cả** dòng evidence **và** `check_log` đỏ.
- Máy của runner: **`-M q35`** + `-netdev user,id=n0 -device virtio-net-pci,netdev=n0` +
  `-drive file=<tmp>,if=none,id=d0,format=raw -device virtio-blk-pci,drive=d0` (một chỗ quyết
  định: `QEMU_MACHINE`/`QEMU_NET`/`QEMU_DISK`). `-M pc` **không** có ECAM ⇒ TAP 15 đỏ (xem §11.2).
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

1. ~~**SMP**~~ — **DONE 4 hart + IPI + TSS per-hart** (KT-276 bước 6: 2 hart; KT-278 bước 7: 4
   hart, IPI fixed-delivery, TSS/GDT/IST1 per-hart). Còn: scheduler/preemption (đã xong ở KT-286..KT-318), `-cpu max` (KT-323).
2. **IPC từ ring 3**, nhiều process, preemption — bước 4 chỉ có **một** syscall thật
   (`DebugPutStr`) + `Exit` (xem §10.6).
3. **`syscall`/`sysret`** — bước 4 dùng `int 0x80` (lý do ở §10.2); nâng lên `syscall`/`sysret`
   cần đổi thứ tự GDT (user data ở `CS - 8`) + per-CPU/`swapgs`.
4. ~~**PCI/ECAM**, virtio trên x86~~ — **DONE** (KT-274: ECAM + legacy + virtio-net + ARP;
   KT-288: virtio-blk đọc sector 0). Còn: virtio-gpu trên x86_64, MSI-X, hotplug, DHCP.
5. **`-cpu max`**, KPTI (tuỳ chọn), soak 1.000.
6. **Nợ N5 của v1** (x86 AP→BSP + PCID) — mở ở K5' đầy đủ, không phải bước này.

## 10. U-mode (ring 3) + `[calling.x86_64]` — K5' bước 4 (KT-270, D2-207)

### 10.1 `spec/abi.toml [calling.x86_64]` — đã mở

`reserved = true` đã bỏ; mục này **đối xứng từng trường** với `[calling.riscv64]` (cùng vai trò,
chỉ đổi tên thanh ghi + cơ chế chuyển), cộng ba trường **bổ sung tường minh** như aarch64
(`trap`/`stack`/`clobbered` — riscv64 đóng băng nên không được thêm vào mục ấy):

| Trường | riscv64 | aarch64 | x86_64 |
|---|---|---|---|
| `syscall_number` | `a7` | `x8` | **`rax`** |
| `args` | `a0`, `a1` | `x0`, `x1` | **`rdi`, `rsi`** |
| `message_info` | `a2` | `x2` | **`rdx`** |
| `mr_registers` | `a3..a6` | `x3..x6` | **`r10`, `r8`, `r9`, `r12`** |
| `return_error` / `return_value` | `a0` / `a1` | `x0` / `x1` | **`rax` / `rdx`** |
| `trap` | — (đóng băng) | `svc #0` | **`int 0x80`** |
| `stack` | — (đóng băng) | SP_EL0 user / SP_EL1 kernel | **RSP user / TSS `RSP0`** |
| `clobbered` | — (đóng băng) | `x0`, `x1`, `nzcv` | **`rax`, `rdx`** |
| `preserved` | `sp, gp, tp, s0..s11, fs0..fs11` | `sp, x19..x28, x29, d8..d15` | **`rsp, rbx, rbp, r12..r15`** |

`vifast-gen` **tự nhận** arch đã mở (điều kiện: có `syscall_number`) — không phải sửa `gen.py`:
`CALLING_ARCHES` thành `["aarch64", "riscv64", "x86_64"]` và sinh `X86_64_*` vào
`v2/lib/vifast-abi/src/generated.rs` + bảng trong `docs/abi.md`. Ảnh dùng hằng **sinh** cho số
hiệu syscall (`SYS_DEBUG_PUT_STR`, `SYS_EXIT`) — `global_asm!` nhận `const` operand nên stub
ring 3 đổi theo spec, không có số 12/14 viết tay (D2-11). `check_abi.py` **siết** tương ứng: kiểm
**từng trường** của x86_64 + **đối xứng cấu trúc** (`set(x64) == set(riscv64) | {trap,stack,
clobbered}`) + cross-check `generated.rs` có `CALLING_ARCHES` ba arch và `X86_64_*` đúng giá trị.

### 10.2 Vì sao `int 0x80` chứ không `syscall`/`sysret`

Đo và đọc đặc tả trước khi chọn:

- **`sysret`** nạp `CS = STAR[63:48] + 16`, `SS = STAR[63:48] + 8` (SDM Vol 2B "SYSRET"). GDT của
  KT-261 đặt **user code `0x18` trước user data `0x20`** ⇒ muốn `CS = 0x1B` thì `STAR[63:48] =
  0x0B` ⇒ `SS = 0x13` = kernel data | RPL 3 ⇒ `#GP` khi `sysret` về CPL3. Muốn dùng `sysret` phải
  **đảo thứ tự** hai descriptor user (đổi hợp đồng GDT + test của KT-261) — ngoài phạm vi bước này.
- **`syscall`** khắc phục được phần `CS`/`SS` (lấy từ `STAR[47:32]`), nhưng **không đổi stack**:
  `syscall` nhảy tới `LSTAR` với `RSP` của ring 3, nên entry stub phải tự đổi sang stack kernel
  (`swapgs` + per-CPU, hoặc một static `RSP` + dựng lại frame `iretq` bằng tay) — thêm một tầng
  trạng thái chưa có (chưa có `GS` base/`swapgs`, chưa có per-CPU).
- **`int 0x80`** dùng đúng đường đã có: IDT vector `0x80` (gate `DPL = 3`), CPU tự đổi sang
  `TSS.RSP0` và đẩy `SS/RSP/RFLAGS/CS/RIP`; `iretq` khôi phục nguyên trạng ring 3. Đủ cho tiêu chí
  "vào ring 3 + một syscall được xử lý + trả về".

⇒ `trap = "int 0x80"` trong spec, ghi rõ lý do; nâng lên `syscall`/`sysret` là bước sau (§9.3).

### 10.3 Đường chạy

```text
kmain (CPL0, stack boot)
  ├─ `_start` dựng cây bảng trang user: PML4[0] (US=1) → PDPT[256] → __pd_user → __pt_user
  │    ├─ lá 0 = __user_start | P|US        (mã ring 3: đọc + exec, W^X)
  │    └─ lá 1 = __user_stack | P|RW|US|NX  (stack ring 3: đọc + ghi, không exec)
  ├─ TSS.RSP0 ← __syscall_stack_top (stack kernel riêng cho đường vào từ CPL3)
  ├─ iretq: SS=0x23, RSP=0x4000_0000_2000, RFLAGS=0x202 (IF=1), CS=0x1B, RIP=0x4000_0000_0000
  ↓
ring 3: ghi/đọc stack user → `int 0x80` DebugPutStr(ptr, len)
  ↑ stub `vifast_isr_syscall` (crate plat): đẩy frame, gọi `syscall_dispatch`
    → in chuỗi user qua COM1 → trả (rax=0, rdx=14) → `iretq` về ring 3
  ↓
ring 3: `int 0x80` Exit(0)
  ↑ handler: TAP 14 (ring 3 **đã chạy tiếp** sau `int 0x80` đầu) → marker + halt
```

**Bốn điểm kỹ thuật đo được, không suy đoán:**

1. **VA user phải chọn đúng cấp bảng trang.** Lần chạy đầu chọn `0x6000_0000_0000` và ghi
   `PDPT[96]` — sai: `0x6000_0000_0000` = **96 TiB** ⇒ `PML4[192]`, không phải `PML4[0]`; kết quả
   `# exc: vec=14 err=0x14 rip=0x600000000000 cs=0x1b` (instruction fetch, `P=0`). Chọn lại
   `0x0000_0040_0000_0000` = **256 GiB** ⇒ `PML4[0]`, `PDPT[256]` — đúng.
2. **Thứ tự `push` quyết định offset frame.** Stub đẩy `rax … r15`; `r15` đẩy **cuối** nên ở
   offset **0**, `rax` ở offset **112**. Lần chạy đầu đọc `nr` ở offset 0 ⇒ ra `r15` = địa chỉ
   entry (`nr=274877906944`), TAP 12 vẫn xanh nhưng `nr` sai. Test host
   `syscall_frame_offsets_literal` chốt **từng** offset.
3. **`int 0x80` lưu RIP của lệnh KẾ TIẾP** (SDM Vol 2B "INT n") — khác fault. Vì vậy handler
   **không** cộng RIP; đột biến tương đương là **lùi** RIP (§10.5 (iii)).
4. **Kernel đọc được trang user** vì `CR4.SMAP = 0` (chưa bật SMAP) — nếu bật, lần đọc chuỗi đầu
   fault và handler in `# exc: vec=14` thay vì im lặng.

### 10.4 Đo được (log `boot-1.log`, TAP 12/13/14)

```text
# umode: iretq entry=0x4000000000 sp=0x4000002000 rflags=0x202 cs=0x1b ss=0x23 code_va=0x4000000000 code_pa=0x106000 code_bytes=71 stack_va=0x4000001000 stack_pa=0x10f000 syscall_stack=0x114000
ok 12 - umode: ring3 entered
# umode: int80 nr=12 cs=0x1b rip=0x4000000025 user=1
ring3 says hi
ok 13 - umode: syscall handled
# umode: ring3 ran nr=12 ret=14
# umode: int80 ret error=0 value=14 printed=14 ptr=0x4000000039 len=14
ok 14 - umode: returned to ring3
# umode: exit code=0 nr=14
# umode: done
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

| Phép đo | Kết quả |
|---|---|
| trang mã ring 3 | `code_bytes=71`, một Frame 4 KiB, `US = 1`, chỉ đọc + exec |
| trang stack ring 3 | `stack_pa=0x10f000`, `US = 1`, RW, `NX = 1` |
| `iretq` | `cs=0x1b`, `ss=0x23`, `rflags=0x202` (`IF = 1`) |
| `int 0x80` đầu | `nr=12` (DebugPutStr), `cs=0x1b` (**từ CPL3**), `rip=0x4000_0000_0025` (= lệnh kế) |
| trả kết quả | `rax=0` (error), `rdx=14` (value) — đúng `[calling.x86_64]` |
| ring 3 chạy tiếp | `int 0x80` thứ hai `nr=14` (Exit) ⇒ TAP 14 |
| runner | **12/12** kiểm sạch, TAP **14/14**, **10 ca âm** ĐỎ đúng |

### 10.5 Đột biến bắt buộc (ghi trung thực **tầng nào** đỏ)

| # | Đột biến | Kết quả |
|---|---|---|
| (i) | `iretq` frame để `CS = 0x08` / `SS = 0x10` (ring 0) | **ĐỎ** — `not ok 12 - umode: ring3 entered`, `# TAP 12 FAIL — dừng (không in marker)`, `qg_boot rc=124` (treo có kiểm soát). Handler thấy `cs = 0x08` ⇒ `is_user_cs` sai ngay ở tầng dữ liệu (test host `kernel_cs_is_not_user` cũng ĐỎ nếu đổi hằng). |
| (ii) | trang mã ring 3 map `P` **không** `US` (`or eax, 1`) | **ĐỎ** — `# exc: vec=14 err=0x15 rip=0x4000000000 cs=0x1b rflags=0x10202` + `# exc: cr2=0x4000000000` (instruction fetch từ CPL3 trên trang supervisor: `P=1`, `U/S=1`, `I/D=1`), **không** có TAP 12, **không** marker, `rc=124`. |
| (iii) | handler **không** tiến `rip` | **Không phải đột biến trên x86_64**: `int 0x80` đã đặt RIP = lệnh **kế tiếp**, nên "không tăng" là hành vi **đúng** (xem §10.3 điểm 3). Đột biến tương đương đã chạy: **lùi** RIP 2 byte (`sub qword ptr [rsp+136], 2`) ⇒ `int 0x80` lặp vô hạn; log bị chặn ở **2.271 B** (trần `MAX_USER_PRINT = 256` + chỉ in dòng "không phải" cho 2 lần đầu), `qg_boot rc=124` (timeout 5 s) — **ĐỎ, treo có kiểm soát**, không có TAP 14/marker. |

**Không** được gọi (i)/(ii) là "đã kiểm trên phần cứng": cả hai đo trên QEMU TCG.

### 10.6 Còn thiếu của bước 4 (ghi rõ)

1. **Một** syscall thật (`DebugPutStr`) + `Exit`; **không** IPC từ ring 3, **không** nhiều process.
2. **Không** preemption/context switch từ ring 3: `RFLAGS.IF = 1` nên timer (vector 32) vẫn vào
   handler và `iretq` về ring 3, nhưng **không** có scheduler — chỉ là ngắt trong suốt.
3. `Exit` **chưa** đi qua `fault_ep`/`FaultKind::Exit` (chưa có procsrv) — nó in marker rồi tắt
   máy trong handler.
4. **Không** có `Tcb`/`VSpace` object cho tiến trình ring 3: trang mã/stack được map trực tiếp vào
   bảng trang của ảnh, chưa qua cap/Untyped (nợ M17 của object layer).
5. **Không** `syscall`/`sysret` (lý do §10.2); **không** tuyên bố K5' đạt — tiêu chí K5' vẫn là
   "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot).

## 11. PCI + virtio-net + ARP — K5' bước 5 (KT-274, D2-208)

### 11.1 Hai đường truy cập cấu hình, chọn theo **máy**

| Máy | ECAM | Đường ảnh dùng | Bằng chứng |
|---|---|---|---|
| `-M q35` | `pcie-mmcfg-mmio` @ **`0xB000_0000`**, 256 MiB (bus 0..255) — bảng ACPI `MCFG` mô tả | **ECAM** | `# pci: ecam base=0xb0000000 buses=0..255 functions=6` |
| `-M pc` (i440fx) | **không có** (không có `MCFG`) | **legacy `0xCF8`/`0xCFC`** | `# pci: legacy base=0x0 buses=0..0 functions=6` |

- Địa chỉ ECAM **không** viết tay: `find_mcfg` đi `RSDP` → `RSDT`/`XSDT` → `MCFG`, **kiểm checksum**
  từng bảng (ACPI §5.2.6). Chỉ khi không có `MCFG` (hoặc ECAM đọc ra `0xffff`) mới rơi xuống legacy.
- **`MCFG` có 8 byte `Reserved` trước allocation structure** (ACPI 6.4 §5.2.6.2) ⇒ `base_address` ở
  offset **44**, không phải 36. Đo được: bảng thật của SeaBIOS dài `0x3C` = 44 + 16; đọc sai offset
  cho `base=0, bus_end=0xB0` (đúng là `0xB000_0000`, `0xFF`) — lỗi đã bắt được bằng log QEMU.
- `RSDP` của SeaBIOS nằm ở **`0xF52E0`** (bản 1.0, `revision = 0`, `rsdt` trỏ vào vùng
  `0x07FE2xxx`), và các bảng ACPI **không** căn 8 byte (`RSDT` @ `0x07FE2345`) — vì vậy mọi phép
  đọc bảng đi theo **địa chỉ trong bảng mục lục**, không suy ra từ căn lề.
- **Không gán lại BAR**: SeaBIOS đã enumerate và cấp BAR trước khi ảnh chạy (đo được trên q35 **và**
  `-M pc`: virtio-net `BAR4 = 0xFE00_0000`, cỡ `0x4000`). Ảnh chỉ **đọc** BAR rồi **kiểm nó nằm
  trong cửa sổ MMIO đã map** (`boot::mmio_mapped`) — ngoài cửa sổ ⇒ `BarUnassigned` chặng
  `bar-unmapped` (fail-closed). Khác riscv64 (`virt` không có firmware enumerate ⇒ driver tự gán).
- Cỡ BAR vẫn **đo** bằng mask `0xffff_ffff` (PCI §6.2.5.1) rồi khôi phục nguyên giá trị firmware.

### 11.2 `-M pc`: legacy chạy **thật**, nhưng TAP `pci: ecam` đỏ **có chủ đích**

Đo được (cùng ảnh, `-M pc`, có `-device virtio-net-pci`):

```text
# pci: legacy base=0x0 buses=0..0 functions=6
# pci: dev 00:03.0 vendor=0x1af4 device=0x1000 class=0x0200 network hdr=0x00
not ok 15 - pci: ecam
```

Đường legacy **không** chỉ enumerate: khi tạm nới điều kiện TAP 15 (bản đo tay, **không** commit),
cùng ảnh trên `-M pc` chạy hết `# pci: net 00:03.0 bar=4 kind=mem64 base=0xfe000000 size=0x4000
command=0x0107 caps=4`, `# net: mac=52:54:00:12:34:56`, `# net: arp gw=10.0.2.2
mac=52:55:0a:00:02:02 sent=1 recv=1` — tức BAR/capability/virtio-net/ARP đều đúng qua
`0xCF8`/`0xCFC`. Bản **commit** giữ TAP 15 **đòi ECAM** vì tên ca là `pci: ecam`: một TAP xanh trên
đường legacy sẽ là dòng log nói sai sự thật. Hợp đồng của runner là **q35**; runner còn kiểm **từ
log** (`^# pci: ecam base=…`) nên ECAM không thể "xanh giả".

### 11.3 virtio-net: MAC thật + ARP reply thật

- Driver ở `vifast-plat-x86_64::virtio_net` — **dùng lại** `vifast_virtio::net` (feature,
  `virtio_net_config`, `virtio_net_hdr`, split queue) và `vifast_net` (Ethernet/ARP); **không** viết
  lại codec nào (D2-110A/B).
- Feature đàm phán: `VIRTIO_F_VERSION_1` + `VIRTIO_NET_F_MAC` (đo được `driver=0x0000000100000020`).
  **Không** `MRG_RXBUF`/`STATUS` — quyết định ở `negotiated_features` (host test).
- MAC đọc từ `virtio_net_config` **sau** khi đọc `config_generation` hai lần (spec §2.4.4: lệch ⇒
  `Err("config_generation")`, không dùng MAC đọc nửa vời). MAC toàn 0/broadcast ⇒ `Err("mac")`.
- ARP: `10.0.2.15` (địa chỉ chuẩn của slirp) hỏi `10.0.2.2` (gateway) ⇒ slirp trả
  `52:55:0a:00:02:02`. Kiểm `oper = reply`, `spa = 10.0.2.2`, `tpa = 10.0.2.15`, `tha = MAC của ta`
  — **không** kiểm `ar_tip == target_ip` (bẫy KT-246: trong reply, `ar_tip` là địa chỉ **người
  hỏi**).
- **Không** DHCP (ngoài phạm vi D2-208) ⇒ địa chỉ tĩnh theo dải slirp.

### 11.4 Đồng hồ cho bước mạng: PIT **kênh 2**, không cần ngắt

Bước PCI/net chạy **sau** `cli` (và sau `Exit` của ring 3) nên `TICK_COUNT` (PIT kênh 0 → IOAPIC →
LAPIC) **không** tăng. Đồng hồ là PIT **kênh 2** — đã được `measure_timer` lập trình mode 2 divisor
`0xFFFF` và **không** phát ngắt: mỗi lần đọc là một ảnh 16 bit, `PitClock` cộng dồn chênh lệch (xử
lý bọc) rồi đổi sang µs bằng `1_193_182 Hz`. Timeout ARP 3 s; đo được `elapsed_us=1705` cho lần
thành công (không phải hằng — số thật của lần chạy).

### 11.5 Đánh đổi đã ghi nhận (KT-274)

1. **MMIO map thêm 2..3 GiB** (`PDPT[2]` → `__pd_mmio_low`, 512 lá 2 MiB, `PCD = 1`): ECAM của q35
   ở `0xB000_0000` nằm **dưới** 3 GiB nên ngoài cả identity map 1 GiB lẫn vùng 3..4 GiB của KT-266.
   Dòng evidence đổi thành `mmio map 0x80000000..0x100000000 pages=1024 pcd=1`.
2. **Stack syscall 16 KiB → 32 KiB**: sau `Exit`, handler chạy tiếp phần PCI/net trên stack này;
   `find_mcfg` dùng hai bộ đệm bảng ACPI 4 KiB **lồng nhau**.
3. **`virtio_net.rs` có `#![allow(unsafe_code)]`** (MMIO + vùng DMA `UnsafeCell`), đúng danh sách
   đóng D2-17 và **y hệt** khuôn `vifast-plat-riscv64/src/virtio_net.rs`. Crate vẫn
   `#![deny(unsafe_code)]`; chỉ `asm.rs` + `virtio_net.rs` được `allow`.
4. **TAP 15..17 nối tiếp sau TAP 14** (không đánh số lại 12/13/14 của U-mode): handler `Exit` gọi
   `net::run` — chưa có procsrv/scheduler nên đây là chỗ duy nhất kernel còn chạy được sau `Exit`.
5. **Không** đàm phán `VIRTIO_NET_F_STATUS` ⇒ "không có mạng" phát hiện bằng **timeout ARP**
   (fail-closed), không bằng bit link.

### 11.6 Còn thiếu của bước 5 (ghi rõ)

1. **Không** virtio-gpu trên x86_64 (virtio-blk **đã có** từ KT-288), **không** MSI-X, **không**
   hotplug, **không** DHCP.
2. **Không** ngắt cho virtio (poll `used`), **không** nhiều frame đồng thời (queue depth 1 thực tế).
3. **Không** `syscall`/`sysret` — vẫn là nợ của K5' đầy đủ. (`-cpu max` **đã đo** ở KT-323.) SMP **4 hart +
   IPI + TSS per-hart** đã có từ KT-278 (bước 7).
4. **Không** tuyên bố K5' đạt: tiêu chí K5' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot).

## 12. SMP 4 hart + IPI + TSS per-hart — K5' bước 7 (KT-278, D2-210)

Bước 6 (KT-276) làm **2 hart**; bước 7 mở lên **4 hart**, thêm **IPI** fixed-delivery và **TSS
per-hart** (bỏ nợ "AP dùng chung GDT tạm 3 entry + IST1 của BSP").

### 12.1 Đường đánh thức hart phụ (ba hart, **tuần tự**)

```text
  BSP (kmain → umode::run → net::run → smp::run)
    │ 1. copy trang trampoline (link 64 KiB-aligned trong `.text`) xuống 0x8000
    │ 2. bật bit online của hart 0 (BSP luôn online — nó đang chạy đây)
    │ 3. với hart = 1, 2, 3 **tuần tự**:
    │      a. ghi boot block (PML4, stack của hart ấy, hart id, entry, IDTR, CR0/CR4/EFER,
    │         LAPIC base, LAPIC ID kỳ vọng, magic)
    │      b. ICR: INIT (level assert) → 10 ms → SIPI vector 0x08 → 200 µs → SIPI
    │      c. chờ `SMP_ONLINE_MASK` bit hart (trần 2 s, đo bằng PIT kênh 2)
    │ 4. TAP 18: bốn hart online (mask 0xf, LAPIC ID đúng, stack **khác slot**)
    │ 5. TAP 19: TSS per-hart — bốn địa chỉ rời nhau, `tr=0x28`, descriptor busy
    │ 6. IPI: ICR fixed-delivery vector 0x30 → hart 1; chờ `SMP_IPI_COUNT == 1` (trần 100 ms)
    │    ⇒ TAP 20
    │ 7. `SMP_CMD[h] = CMD_REPORT` **tuần tự** từng hart phụ, chờ `SMP_ALIVE_MASK` bit h
    │ 8. TAP 21 (cả bốn hart báo alive) → marker + halt
    ↓
  AP (SIPI ⇒ 0x8000, real mode)
    ├─ 16-bit: cli/cld, DS=ES=SS=CS, lgdt GDT tạm, CR4.PAE, CR3, EFER.LME|NXE, CR0.PG|PE
    ├─ ljmp 0x08:0x8046 ⇒ 64-bit: DS/ES/SS=0x10, rsp = boot.stack_top, lidt boot.idt
    ├─ call boot.entry = `kmain_secondary(hart)`
    └─ kmain_secondary: kiểm slot stack + LAPIC_ID → **TSS riêng + GDT riêng + `ltr`** →
       bật LAPIC (SVR/TPR/LINT) → in `# smp: hart=N online` → bật bit online → LAPIC timer
       (vector 32) + `sti` → vòng `hlt`: thấy `CMD_REPORT` ⇒ in `# smp: hart=N alive` + bit
       alive; thấy IPI (vector 0x30) ⇒ handler in `# smp: ipi recv=<n> intid=0x30` + `EOI`.
```

### 12.2 Vì sao boot block **dùng lại một chỗ** cho ba hart phụ

Trang trampoline chỉ có **một** boot block (`0x8900`). Ba hart phụ được đánh thức **tuần tự**:
BSP ghi boot block cho hart `h`, gửi `INIT`+`SIPI`, rồi **chờ hart ấy online** trước khi ghi boot
block cho hart kế. Hart phụ đọc **mọi** trường của boot block (kể cả `BOOT_LAPIC_ID`) **trước**
khi bật bit online, nên sau khi thấy bit ấy BSP ghi đè là an toàn — không cần ba bản boot block,
và không có hai hart phụ nào chạy trampoline cùng lúc (stack 16-bit tạm `0xFC00` cũng chỉ một
người dùng).

### 12.3 Vì sao trampoline **copy xuống** thay vì để linker đặt ở `0x8000`

Đo được: QEMU `-kernel` (PVH) **có** nạp segment ở `0x8000`, nhưng **SeaBIOS chạy sau đó** và ghi
đè vùng thấp — đọc lại `u32 @ 0x8000` trong `kmain` ra `0`. Vì vậy trampoline được **link ở địa
chỉ 64 KiB-aligned** (`. = ALIGN(64K)` **trong** `.text`, không phải output section riêng) rồi BSP
**copy** xuống `0x8000` lúc chạy.

- **64 KiB-aligned** là điều kiện để `offset label` **16-bit** trong đoạn real mode bằng đúng
  offset trong trang (địa chỉ link `0x110000 + off` cắt còn 16 bit = `off`); mọi địa chỉ tuyệt đối
  trong đoạn 64-bit dùng hằng `0x8000 + off` nên **đúng sau khi copy**.
- **Trong `.text`** (không phải output section `ALIGN(64K)`): một section có `sh_addralign = 64 KiB`
  làm **segment** nhận `align 2**16` ⇒ file offset của segment thành `0x10000`, và **multiboot2
  header** (24 byte đầu `.text`) rơi ra ngoài 32 KiB đầu tệp — vi phạm đặc tả multiboot2. `. =
  ALIGN(64K)` chỉ đệm **bên trong** section nên segment vẫn `align 2**12`, header ở file offset
  `0x1000` (runner kiểm cả hai: `__trampoline_start % 0x10000 == 0` và trang đúng 4 KiB).

### 12.4 Vì sao **không** có đoạn 32-bit protected mode

Từ real mode có thể đặt thẳng `CR4.PAE`/`CR3`/`EFER.LME` (đều là lệnh CPL0 hợp lệ trong real
mode), rồi `CR0.PG|PE` và `ljmp` vào đoạn code 64-bit — CPU vào thẳng IA-32e mode. Nhờ vậy GDT
tạm chỉ cần **3 entry** (null + code 64-bit + data), không cần descriptor code 32-bit, và selector
`0x08`/`0x10` **trùng** GDT thật của BSP nên bảng IDT dùng chung (`selector 0x08`) vẫn hợp lệ
trên hart phụ.

### 12.5 Bố cục trang trampoline (`0x8000`, một trang 4 KiB)

| Offset | Nội dung |
|---|---|
| `0x000` | code 16-bit real mode (cli/cld, segment, `lgdt`, `CR4/CR3/EFER/CR0`, `ljmp`) |
| `0x046` | code 64-bit long mode (segment, `rsp`, `lidt`, `call kmain_secondary`) |
| `0x800` | GDT tạm 3 entry (null, code 64-bit `0x08`, data `0x10`) |
| `0x880` | `GDTR` 6 byte (limit 23, base `0x8800`) — real mode |
| `0x900` | boot block (BSP ghi): `PML4`, `stack_top`, `hart`, `entry`, `IDTR` (limit+base liền), `CR0`, `CR4`, `EFER`, `LAPIC base`, `LAPIC ID`, magic `"SMP2"` |

**Địa chỉ hoá 16-bit**: `mov eax, [0x900]` trong `.code16` biên dịch thành `A1` + **moffs16** (địa
chỉ 16-bit) — không phải moffs32 — nên 4 byte sau bị CPU hiểu là lệnh khác. Trampoline dùng
`mov bx, 0x900; mov eax, [bx]` (ModRM 16-bit, `DS:bx`) là dạng **tường minh** và đúng ở mọi
assembler. **Địa chỉ tuyệt đối ở 64-bit**: `DS`/`SS` có base 0 nên `[0x908]` là linear `0x908` —
phải cộng `0x8000`; đo được lần chạy đầu: quên cộng ⇒ `rsp = 0`, `call` đẩy return address xuống
`0xFFFF_FFFF_FFFF_FFF8` ⇒ `#PF` rồi `#DF` (AP chết).

### 12.6 Arena stack per-hart

Mỗi hart một record `guard 4 KiB | stack 64 KiB` (`HART_STRIDE = 0x1_1000`), `MAX_HARTS = 8` ⇒
`0x8_8000` (544 KiB) NOLOAD do `linker.ld` cấp (`__smp_stacks_start`). Hart 0 dùng **slot 0**
(`__stack_top = __smp_stacks_start + HART_STRIDE` nên `_start` không đổi). Ảnh kiểm **lúc chạy**
`__smp_stacks_end - __smp_stacks_start == HART_AREA_BYTES` và `check_layout` (record rời nhau, căn
trang); mỗi hart phụ kiểm `sp` của chính nó nằm trong **slot của mình** (`slot_of`), và BSP kiểm
**bốn** slot đôi một khác nhau.

**Guard là địa chỉ logic, chưa unmap phần cứng**: identity map dùng lá 2 MiB nên guard 4 KiB vẫn
được map; phát hiện tràn dựa vào `check_layout`/`slot_of` (đúng câu chữ "phát hiện bằng kiểm địa
chỉ"). Unmap guard cần bảng PT 4 KiB cho vùng arena — việc của bước sau.

### 12.7 LAPIC per-hart + vòng `hlt`

Mỗi hart phụ bật LAPIC **của nó**: `IA32_APIC_BASE` (enable), đọc `LAPIC_ID` và **so** với giá trị
BSP đã ghi vào boot block (lệch ⇒ PANIC), `SVR` (enable + spurious 255), `TPR = 0`, mask
`LINT0/1`. Sau đó cấu hình **LAPIC timer của hart ấy** (`LVT_TIMER` vector 32 periodic, chia 1,
count `100_000`) rồi `sti` và vào vòng `hlt` — timer là **nguồn đánh thức** để kiểm `SMP_CMD`,
**không** phải nguồn tick của hệ (PIT của BSP vẫn là nguồn tick).

Vector 32 dùng **chung** stub/handler với BSP; handler phân biệt tick của ai bằng `LAPIC_ID` đọc
qua MMIO (mỗi hart thấy ID của chính nó) nên `TICK_COUNT`/`EOI_COUNT` của BSP **không** bị hart
phụ làm bẩn — nếu không, `timer: eoi` sẽ đỏ oan.

### 12.8 IPI — vector riêng `0x30`, `ICR` fixed-delivery

| Thành phần | Giá trị | Nguồn |
|---|---|---|
| Vector IPI | `0x30` (`exc::VEC_IPI`) | ngoài dải exception `0..31`, khác timer 32 / syscall `0x80` / spurious 255 / sentinel 254 |
| Stub | `STUB_IPI = 18` (`vifast_isr_ipi`) | `exc::stub_slot(0x30)`; `STUB_COUNT` 18 → **19** |
| `ICR` thấp | `0x4030` = vector `0x30` \| `Level = 1` (bit 14) \| `Trigger = 0` (bit 15) \| delivery mode `000` (fixed) | SDM Vol 3A Hình 10-27 |
| `ICR` cao | `0x0100_0000` (destination = `APIC ID 1`) | SDM §10.6.1 |
| Trần chờ | **KT-301**: trần **số vòng** `SMP_SPIN_LIMIT = 100_000_000` (điều kiện dừng **chính**, chờ biến đếm `SMP_IPI_COUNT`); `IPI_TIMEOUT_US = 100_000` (100 ms, PIT kênh 2) **giữ nguyên** nhưng chỉ còn là **chốt an toàn** hỏi sau khi trần vòng cạn | ngân sách của ảnh + §16 |

**Vì sao probe `isr_unknown` dời từ `0x30` sang `0x31`**: KT-261 dùng `int 0x30` để chứng minh
"vector lạ vẫn thấy được". Từ KT-278 `0x30` là vector IPI **có stub riêng**, nên `int 0x30` sẽ
chạy vào handler IPI và TAP 7 mất bằng chứng. `exc::VEC_UNKNOWN_PROBE = 0x31` giữ nguyên ý nghĩa
của ca ấy (vẫn rơi về `isr_unknown`, vẫn in `vec=254`).

**Quy tắc in**: BSP in dòng `# smp: ipi send …` **trước** khi ghi `ICR` (hart đích in
`# smp: ipi recv=1 intid=0x30` ngay khi nhận). Đo được ở lần chạy đầu khi in **sau**:
`# smp: ipi send dest=1 vector=0x30 low# smp: ipi r=0x4030 …` — hai dòng xen ký tự trên COM1 và
runner ĐỎ. Đây đúng bài học KT-259 ("in trước, đếm sau").

### 12.9 TSS per-hart (bỏ nợ của bước 6)

| Thành phần | Bước 6 (KT-276) | Bước 7 (KT-278) |
|---|---|---|
| TSS | **một** vùng 104 byte dùng chung | **mỗi hart một** vùng (`[[AtomicU32; 26]; MAX_HARTS]`) |
| GDT | hart phụ dùng **GDT tạm 3 entry** của trampoline | **mỗi hart một bảng 7 entry** (`[[AtomicU64; 7]; MAX_HARTS]`) + `GDTR` riêng |
| `ltr` trên AP | **không** | **có** — descriptor TSS trỏ TSS của chính hart ấy |
| `IST1` cho `#DF` | dùng chung `IST1` của BSP | **mỗi hart một stack IST1** 4 KiB (`__ist1_top + hart × 0x1000`) |

Hart phụ **đọc lại** base TSS từ **descriptor nó vừa nạp** (`gdt::tss_base_of`) rồi công bố; BSP
kiểm **địa chỉ**: `tss::check_distinct` đòi bốn vùng **rời nhau** (không trùng, không chồng lấn) và
bốn stack `IST1` cũng rời nhau. Đó là "phát hiện bằng kiểm địa chỉ" của đột biến (iii) — không
phải "hy vọng nó không xảy ra".

Bố cục GDT **giống hệt nhau** ở mọi hart nên selector `0x08`/`0x10`/`0x28` có **cùng nghĩa** trên
mọi hart — điều kiện để bảng IDT **dùng chung** (mọi ô `selector = 0x08`) vẫn hợp lệ sau khi hart
phụ `lgdt` bảng riêng của nó.

### 12.10 Đo được (log QEMU nguyên văn, `-smp 4`)

```text
# smp: arena pa=0x12e000 bytes=557056 expect=557056 layout=Ok(()) ok=1
# smp: trampoline base=0x8000 bytes=4096 src=0x110000 magic=0x00000000534d5032 ok=1
# smp: boot block pml4=0x116000 stack=0x150000 hart=1 entry=0x1065ab idt=0x12c098/0xfff cr0=0x80000011 cr4=0x20 efer=0x900 lapic=0xfee00000 bsp_id=0 dest=1
# smp: icr init dest=1 low=0xc500 high=0x01000000 sipi vector=0x08 low=0x0608
# smp: hart=1 online
# smp: hart=1 lapic id=1 base=0xfee00000 svr=0x1ff tpr=0 sp=0x14feb0 slot=1 stack=0x140000-0x150000
# smp: hart=1 tss base=0x12d10c tr=0x28 type=0x8b ist1=0x11d000 rsp0=0x150000
# smp: cpu_on hart=1 online=1 waited_us=4482 lapic_id=1 sp=0x14feb0 slot=Some(1) init_ok=1 sipi_ok=1/1
# smp: icr init dest=2 low=0xc500 high=0x02000000 sipi vector=0x08 low=0x0608
# smp: hart=2 online
# smp: hart=2 lapic id=2 base=0xfee00000 svr=0x1ff tpr=0 sp=0x160eb0 slot=2 stack=0x151000-0x161000
# smp: hart=2 tss base=0x12d174 tr=0x28 type=0x8b ist1=0x11e000 rsp0=0x161000
# smp: cpu_on hart=2 online=1 waited_us=2672 lapic_id=2 sp=0x160eb0 slot=Some(2) init_ok=1 sipi_ok=1/1
# smp: icr init dest=3 low=0xc500 high=0x03000000 sipi vector=0x08 low=0x0608
# smp: hart=3 online
# smp: hart=3 lapic id=3 base=0xfee00000 svr=0x1ff tpr=0 sp=0x171eb0 slot=3 stack=0x162000-0x172000
# smp: hart=3 tss base=0x12d1dc tr=0x28 type=0x8b ist1=0x11f000 rsp0=0x172000
# smp: cpu_on hart=3 online=1 waited_us=2515 lapic_id=3 sp=0x171eb0 slot=Some(3) init_ok=1 sipi_ok=1/1
# smp: online mask=0xf expect=0xf harts=4 booted=3 slots=[Some(0), Some(1), Some(2), Some(3)]
ok 18 - smp: 4 harts online
# smp: tss bases=[0x12d0a4, 0x12d10c, 0x12d174, 0x12d1dc] ist1=[0x11c000, 0x11d000, 0x11e000, 0x11f000] distinct=Ok(()) ist1_distinct=Ok(()) tr=0x28 type=0x8b ok=1
ok 19 - smp: tss per-hart
# smp: ipi send dest=1 vector=0x30 low=0x4030 high=0x01000000
# smp: ipi recv=1 intid=0x30
# smp: ipi sent=1 count=1 waited_us=629 ok=1
ok 20 - smp: ipi delivered
# smp: hart=1 alive
# smp: report hart=1 done=1 waited_us=8812 alive=1
# smp: hart=2 alive
# smp: report hart=2 done=1 waited_us=608 alive=1
# smp: hart=3 alive
# smp: report hart=3 done=1 waited_us=18883 alive=1
# smp: hart=0 alive
# smp: online mask=0xf alive=0xf expect=0xf harts=4 ap_ticks=10 waited_online_us=9669 waited_alive_us=28303
ok 21 - smp: all harts report
```

### 12.11 Đột biến bắt buộc (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **không gửi SIPI** | `--features no-sipi` (chỉ `INIT`) | hart phụ không online ⇒ `not ok 18`, không marker; QEMU treo tới hết ngân sách runner (ĐỎ) |
| (ii) **trampoline sai địa chỉ** | `--features bad-trampoline` (SIPI vector `0x09` ⇒ `0x9000`) | hart phụ chạy vào vùng 0, không online ⇒ `not ok 18`, không marker; BSP hết trần 2 s rồi `cli; hlt`, QEMU không tự thoát (treo **có kiểm soát**, runner timeout 20 s giết) |
| (iii) **stack per-hart chồng nhau** | bỏ guard khỏi `.smp_stacks` của `linker.ld` | `arena bytes=524288 expect=557056 ok=0` ⇒ `PANIC: smp: bố cục arena stack không khớp linker`, không marker (ĐỎ). Test host `layout_rejects_overlap_without_guard` chốt `check_layout` trả `Overlap { a: 0, b: 1 }` |
| (iv) **không gửi IPI** | `--features no-ipi` | `# smp: ipi sent=0 count=0` ⇒ `not ok 20`, không marker (ĐỎ) — TAP 18/19 vẫn xanh, nên ca này đo **đúng** phần IPI |
| (v) **IPI tới hart không tồn tại** | `--features bad-ipi-dest` (dest = `APIC ID 7`, `-smp 4` chỉ có 0..3) | `# smp: ipi sent=1 count=0 waited_us=18446744073709551615` ⇒ `not ok 20`, không marker. **Không treo**: BSP hết **trần số vòng** (KT-301; trước KT-301 là trần 100 ms) rồi **đi tiếp** và in `not ok 20` (ĐỎ) |
| (vi) **TSS per-hart trùng địa chỉ** | `--features same-tss` (descriptor TSS của AP trỏ TSS của hart 0) | `# smp: tss bases=[0x12d0a4, 0x12d0a4, 0x12d0a4, 0x12d0a4] … distinct=Err(Duplicate { a: 0, b: 1 }) … ok=0` ⇒ `not ok 19`, không marker (ĐỎ) — phát hiện bằng **kiểm địa chỉ**, không fault |

### 12.12 Còn thiếu của bước 7 (ghi rõ)

1. **Không** scheduler/preemption, **không** IPI broadcast/shootdown `TLBI` (chưa có `TLBI` vì
   chưa có `unmap` trên x86_64), **không** `GS`/`FS` base per-hart.
2. **Không** `syscall`/`sysret` — vẫn là nợ của K5' đầy đủ. (`-cpu max` **đã đo** ở KT-323.)
3. **Không** tuyên bố K5' đạt: tiêu chí K5' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot).

## 13. KT-286 — preemption: hai tác vụ kernel + timer chuyển ngữ cảnh

Trạng thái: **DONE** (KT-286, 2026-11-21). Quyết định: **D2-212**.

### 13.1 Phạm vi

| Việc | Trạng thái |
|---|---|
| **Hai** tác vụ kernel, mỗi tác vụ **stack riêng** 16 KiB + frame ngắt riêng | **DONE** |
| Timer vector 32 gọi `schedule()`: lưu ngữ cảnh, chọn tác vụ kế, nạp ngữ cảnh, `iretq` | **DONE** |
| Đo: `# sched: switch a→b n=…`, `# sched: switch b→a n=…`, `# sched: ticks=… tasks=2` | **DONE** |
| TAP 22/23 + runner **14 ca âm** | **DONE** |
| U-mode task, nhiều hart schedule, priority, sleep/wake | **chưa** (bước sau) |

### 13.2 Cơ chế: đổi `rsp` trước `iretq`

Stub ngắt chung đã là một context switch hoàn chỉnh: CPU đẩy `rflags/cs/rip`, stub đẩy `vector`
+ mã lỗi + 15 GPR, gọi `exc_dispatch`, rồi `pop`/`iretq`. Chỉ cần **đổi `rsp`** sang frame của
tác vụ kế là chuyển được ngữ cảnh — không cần asm riêng cho tác vụ, không lưu/khôi phục thanh
ghi bằng tay:

1. `exc_dispatch` nhận thêm tham số thứ 6 `frame_rsp` (stub truyền `rbx` trong `r9`).
2. Nhánh timer gọi `sched::on_tick(frame_rsp)`: lưu `rsp` của tác vụ đang chạy, chọn tác vụ kế,
   trả `(rsp, rip)` của frame đích.
3. Handler trả `rsp` đích trong **giá trị trả** (`rdx`); stub `mov rsp, rdx`, rồi
   `pop`/`iretq` trên frame mới.

> **KT-298 sửa bước 3**: KT-286 đặt `rsp` đích vào ô `static VIFAST_SCHED_NEW_RSP` — ô ấy
> **dùng chung mọi hart** và là gốc của boot `#GP` storm 410/600. Nay bàn giao đi qua **thanh
> ghi** `rax`/`rdx`; xem §15.

### 13.3 Frame khởi tạo **22** qword — vì sao không phải 20

Intel SDM nói `IRET` chỉ pop `SS:RSP` khi **đổi mức đặc quyền** (`CS.RPL != CPL`); frame cùng
CPL đáng lẽ chỉ 20 qword. Nhưng **QEMU 11.1.3** (`target/i386/tcg/seg_helper.c`,
`helper_ret_protected`) dùng điều kiện:

```text
rpl == cpl && (!(HF_CS64) || (HF_CS64 && !is_iret))
```

Trong **64-bit mode** (`HF_CS64 = 1`) với **`iret`** (`is_iret = 1`) điều kiện này **luôn sai**
⇒ QEMU **luôn** pop `SS:RSP`, kể cả khi `CS.RPL == CPL`. Đo được: frame 20 qword làm `iretq` lấy
`rsp` từ ô ngay sau frame (0) ⇒ tác vụ chạy với `rsp = 0` ⇒ timer đẩy frame vào `0xffff…fff8`
⇒ `#PF` rồi `#DF` (`# exc: vec=8 err=0x0 rip=<task_a> cs=0x8`).

Vì vậy frame khởi tạo mang **thêm** hai ô `rsp`/`ss`:

- **QEMU** (pop 5): `rsp = stack_top - 8`, `ss = 0x10` — đúng trạng thái kernel.
- **CPU thật** (pop 3, đúng SDM): hai ô ấy **không** được đọc; `rsp` sau `iretq` =
  `frame + 160` = `stack_top - 24` — vẫn nằm trong stack, chỉ hụt 16 byte (vô hại).

Frame 22 qword vì vậy đúng trên **cả hai**, không phải workaround chỉ cho QEMU.

### 13.4 Đo được (log `boot-1.log`, TAP 22/23)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: ticks=7 tasks=2
# sched: runs a=300242 b=363551 switches=6 ab=1 ba=2 a_rsp=0x12e158 b_rsp=0x132158
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
```

| Phép đo | Kết quả |
|---|---|
| stack tác vụ | `a_rsp=0x12e158`, `b_rsp=0x132158` — hai vùng `.bss` rời nhau, mỗi vùng 16 KiB |
| chuyển ngữ cảnh | `switches=6` (A→B, B→A, … ba vòng), `ab=1`, `ba=2` — **cả hai** chiều |
| tác vụ chạy thật | `runs a=300242 b=363551` — bộ đếm do **chính tác vụ** tăng, không phải hằng |
| tick | `ticks=7` — timer PIT thật sự ngắt trong lúc scheduler bật |
| nhường lại `main` | sau 6 lần chuyển, scheduler quay về `main` ⇒ in tổng kết + marker |

### 13.5 Đột biến bắt buộc (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **không gọi `schedule()`** | `--features no-preempt` (sau tick đầu không chuyển nữa) | chỉ tác vụ A chạy, `main` không được nhường lại ⇒ không `# sched: ticks=…`, không marker; QEMU treo tới hết ngân sách runner (ĐỎ) |
| (ii) **nạp `rsp` hỏng** | `--features bad-rsp` (nạp `rsp` tác vụ kế = địa chỉ chưa map `0x7000_0000_0000`) | KT-286: stub `pop` từ địa chỉ ấy ⇒ `#PF` rồi `#DF` ⇒ `# exc: vec=8`, không marker (ĐỎ). **KT-298**: phép kiểm frame bắt **sớm** ⇒ `# sched: BAD frame slot=2 rsp=0x700000000000` + `halt`, không marker (ĐỎ, có chẩn đoán) — xem §15.5 |
| (iii) **không `EOI`** | `--features no-eoi` (bỏ `lapic_eoi()` trong handler timer) | LAPIC giữ ISR bit của vector 32 ⇒ timer **tắt sau tick đầu** ⇒ `not ok 10 - timer: ticks`, không marker (ĐỎ) |

### 13.6 Còn thiếu của bước 8 (ghi rõ)

1. **Không** U-mode task (tác vụ vẫn ở CPL0) — **KT-304 đã làm** (xem §17: tác vụ ring 3 được
   `schedule()` chọn, tick preempt, chạy lại ở ring 3); **không** nhiều hart schedule (chỉ BSP),
   **không** priority, **không** sleep/wake — đúng phạm vi D2-212.
2. **Không** tuyên bố K5' đạt: tiêu chí K5' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot).

## 14. KT-288 — virtio-blk: BAR leaf riêng + virtqueue + đọc **sector 0**

Trạng thái: **DONE** (KT-288, 2026-11-21). Quyết định: **D2-213**.

### 14.1 Phạm vi

| Việc | Trạng thái |
|---|---|
| `pci::VirtioKind::Blk` (0x1001/0x1042) + BAR firmware gán + bốn capability | **DONE** |
| Split virtqueue 8 descriptor trong **một** trang DMA 4 KiB (header 16 B + data 512 B + status 1 B) | **DONE** |
| Đọc **sector 0** của đĩa QEMU thật (`-drive file=…,if=none,id=d0,format=raw -device virtio-blk-pci,drive=d0`) | **DONE** |
| Bằng chứng: `# blk: device …` + `# blk: sector0 hex=<32 hex> … magic=1` | **DONE** |
| TAP 25/26 (**KT-304** dời từ 24/25) + runner **19 ca âm** (thêm ca "không có đĩa", "tác vụ ring 3 không chạy lại", "preempt không ở ring 3", "bộ đếm giảm", "tác vụ chưa chạy (n=0)") | **DONE** |
| virtio-gpu trên x86_64, MSI-X, hotplug | **chưa** (bước sau) |

### 14.2 Khác khuôn aarch64 (KT-273) ở đúng ba điểm, đều do kiến trúc

1. **DMA bằng VA = PA**: x86_64 chạy identity map 1 GiB nên `addr_of!(trang DMA)` **là** PA —
   không có alias nửa cao, không cần `high_to_pa` như aarch64.
2. **BAR do firmware gán**: SeaBIOS đã enumerate và gán BAR trước khi ảnh chạy, nên driver **đọc**
   rồi kiểm BAR nằm trong cửa sổ MMIO đã map — **không** ghi đè địa chỉ firmware đã cấp (xem đầu
   `v2/kernel/vifast-plat-x86_64/src/pci.rs`). Vì vậy "leaf riêng" trên x86_64 **không** phải một
   hằng địa chỉ như aarch64 mà là **bất biến kiểm được**: `virtio_blk::bar_disjoint(blk_base,
   blk_size, net_base, net_size)` — BAR của blk phải **rời** BAR của virtio-net. Hàm thuần này có
   test host cho cả ca âm (trùng leaf, chồng một phần, BAR 0, cỡ 0, tràn 64 bit).
3. **Đồng hồ là tham số**, không phải singleton: bước này chạy sau `cli` nên không dùng được tick
   PIT→IOAPIC→LAPIC; đồng hồ là PIT **kênh 2** (`PitClock`, cùng §11.4) do ảnh sở hữu. Mọi hàm
   chờ nhận `now: &mut dyn FnMut() -> u64` (µs) — driver **không** giữ trạng thái thời gian.

### 14.3 Đo được (log `boot-1.log`, TAP 25/26 — **KT-304** dời từ 24/25)

```text
# pci: blk 00:03.0 vendor=0x1af4 device=0x1001 bar=4 kind=mem64 base=0xfe004000 size=0x4000 command=0x0107 caps=4
# blk: device vendor=0x1af4 device=0x1001 bar=4 base=0xfe004000 size=0x4000 net_base=0xfe000000 bar_distinct=1 capacity=8 blk_size=512 size_max=0 seg_max=254 queue=8 max=256 status=0x0f features=0x0000010130007e54 driver=0x0000000100000200 flush=1
ok 24 - blk: device
# blk: sector0 hex=5649464153544f532d4b543238382d53 ascii="VIFASTOS-KT288-S" magic=1 capacity=8 status=0x0f dma=0x133000 desc=0x133000 avail=0x133080 used=0x133098 requests=1 completions=1 errors=0 timeouts=0
ok 25 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| thiết bị | `vendor=0x1af4 device=0x1001` — virtio-blk transitional, bus `00:03.0` |
| BAR **leaf riêng** | blk `0xfe004000` vs net `0xfe000000` ⇒ `bar_distinct=1` (hai leaf 16 KiB rời nhau) |
| dung lượng | `capacity=8` — **đọc từ `virtio_blk_config`**, khớp đĩa tạm 8 sector của runner |
| trạng thái | `status=0x0f` = `ACK\|DRIVER\|FEATURES_OK\|DRIVER_OK` (đọc lại sau DRIVER_OK) |
| feature | `driver=0x0000000100000200` = `VIRTIO_F_VERSION_1` (bit 32) + `VIRTIO_BLK_F_FLUSH` (bit 9) |
| sector 0 | `hex=5649464153544f532d4b543238382d53` = `VIFASTOS-KT288-S` — **khớp từng ký tự** với 16 byte đầu đĩa tạm (`od -An -tx1 -N16`) |
| chữ ký | `magic=1` — 8 byte đầu khớp `SECTOR0_MAGIC = b"VIFASTOS"` |
| request | `requests=1 completions=1 errors=0 timeouts=0` — một request `IN`, hoàn tất thật |

### 14.4 Đột biến bắt buộc (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **không có đĩa** | runner boot **thật** không `-drive`/`-device virtio-blk-pci` (ca âm (o)) | `# blk: no device` + `not ok 25 - blk: device` + `# TAP 25 FAIL — dừng (không in marker)`; QEMU treo tới hết ngân sách (ĐỎ) |
| (ii) **chữ ký sai** | `--features bad-blk-magic` (ảnh in `magic=0` dù hex vẫn đúng) | `# blk: sector0 … magic=0 …` + `not ok 26 - blk: sector0` + `# TAP 26 FAIL` (ĐỎ) |
| (iii) **BAR sai** | `--features bad-blk-bar` (BAR = địa chỉ chưa map `0x7000_0000_0000`) | `# exc: vec=14 err=0x2 rip=0x10084f cs=0x8 rflags=0x10046` + `# exc: cr2=0x700000000014` ⇒ halt, không marker (ĐỎ) |

### 14.5 Còn thiếu của bước 9 (ghi rõ)

1. **Không** virtio-gpu trên x86_64, **không** MSI-X, **không** hotplug — đúng phạm vi D2-213.
2. **Không** ngắt cho virtio (poll `used`), **không** nhiều request đồng thời (queue depth 1 thực
   tế), **không** ghi/flush trên đường sản phẩm (hàm có, chỉ đọc sector 0 được đo).
3. **Không** tuyên bố K5' đạt: tiêu chí K5' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot).

## 15. KT-298 — sửa `#GP` storm trong preemption (boot 410/600 của KT-293)

Trạng thái: **DONE** (KT-298, 2026-11-21). Căn cứ: KT-293 soak 600 ⇒ **590/600**, một boot
(410) bão `#GP` (`# exc: vec=13 err=0x0 rip=0x10890` lặp vô hạn, log 159 KB nhiễu nhị phân).

### 15.1 Triệu chứng nguyên văn (log `boot-410.log` của KT-293)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: ticks=7 tasks=2
# sched: runs a=# exc: vec=13 err=0x0 rip=0x10890
# exc: v1000ec=2
# exc: vec=
# exc: vec=# exc: vec=
… (lặp tới hết ngân sách 20 s; 159 KB, phần lớn là **byte của `.text`** bị đẩy ra COM1)
```

Ba dữ kiện định vị lỗi:

1. `ticks=7` + `switch a→b n=1` + `switch b→a n=2` ⇒ **scheduler chạy đúng** tới lần chuyển
   cuối; hỏng xảy ra **sau** khi `main` được nhường lại.
2. `rip=0x10890` — đối chiếu ELF của **đúng** ảnh ấy (`build=983d01f865b19220 text=69632`, còn
   trong worktree KT-293): `0x10890` = `66 ff cb` = `decw %bx`, một lệnh **không thể** sinh
   `#GP`. Vậy `rip` ấy **không** phải lệnh gây lỗi — frame đã bị đọc từ chỗ khác.
3. Byte trên COM1 là **mã máy của `.text`** (`48 8d 35 …`, `48 8d 7c 24 18`) ⇒ `Console::write_str`
   nhận một `&str` trỏ vào `.text` với độ dài lớn ⇒ **stack của đường in đã bị ghi đè**, không
   phải chỉ xen ký tự.

### 15.2 Gốc rễ: ô bàn giao `rsp` **dùng chung mọi hart**

KT-286 đặt `rsp` của frame đích vào `static VIFAST_SCHED_NEW_RSP`; stub chung đọc ô ấy **sau**
khi handler trả về. Ô ấy **không** có chủ: hart phụ (1/2/3) nhận tick PIT trong khoảng giữa lúc
BSP ghi ô và lúc stub BSP đọc ô sẽ đọc **cùng** ô ấy, thấy khác `0`, rồi `mov rsp, <frame của
BSP>` + `iretq` vào **cùng** một ngữ cảnh trên **cùng** một stack. Hai hart chạy chung một stack
⇒ khung in bị ghi đè ⇒ byte rác ra COM1 ⇒ `#GP`/`#PF` lung tung.

Cửa sổ rất hẹp (vài chục lệnh) nên tần suất **1/600** — đúng cỡ đã đo.

### 15.3 Tái hiện: **kéo dài cửa sổ bàn giao** (thí nghiệm, không phải feature)

Soak thường **không** tái hiện được: `--smoke 200 3` cho **198/200**, hai boot đỏ đều là
`not ok 10 - timer: ticks` (KT-297), **không** có `#GP`. Vì vậy cửa sổ được **kéo dài** bằng một
**bản vá tạm** (không commit): trong nhánh timer của `exc_dispatch`, sau khi quyết định chuyển,
chèn

```rust
let mut spins = 0u32;
while spins < 5_000_000 { core::hint::spin_loop(); spins += 1; }
```

| Ảnh | Bản vá tạm | Kết quả (3 boot) |
|---|---|---|
| **trước** KT-298 (ô `static`) | bật | **ĐỎ 3/3** — treo sau `# sched: switch b→a n=2`: **không** `# sched: ticks=…`, **không** marker, `rc_124` |
| **sau** KT-298 (thanh ghi) | bật | scheduler **hoàn tất** (`ticks=7 switches=6`), **không** `BAD frame`, **không** `#GP`; TAP 22 đỏ vì spin dài **bỏ đói** tác vụ (`runs a=0 b=0`) — tác dụng phụ của bản vá, **không** phải lỗi bàn giao |

**Đọc kết quả cho đúng**: bản vá chứng minh ô `static` **quan sát được từ hart khác** (boot
hỏng hẳn), và chứng minh bàn giao qua thanh ghi **không** còn phụ thuộc thời điểm (scheduler
chạy đúng trình tự dù cửa sổ dài 5 triệu vòng). Nó **không** phải ca hồi quy tự động: spin dài
tự nó phá TAP 22, nên bản vá **không** được giữ trong ảnh.

**Vì sao không giữ feature**: một feature mà sau khi sửa vẫn làm boot đỏ (vì bỏ đói tác vụ) sẽ
bị đọc thành "bản sửa không hiệu quả". Bằng chứng nằm ở đây + `JOURNAL/kt298-x86_64-gp.md`.

**Vì sao không tái hiện được ở lần chuyển cuối**: đo `ap_ticks` (11 → 40 trong lúc scheduler
bật) xác nhận hart phụ **có** nhận tick, nhưng kéo dài cửa sổ **chỉ ở lần chuyển cuối** (50
triệu vòng) vẫn **xanh 6/6** trên ảnh trước KT-298 — cửa sổ ấy không trúng tick hart phụ trong
6 lần đo. Đây là lý do bản vá phải quét **mọi** lần chuyển mới đỏ được.

### 15.4 Bản sửa

1. **Bàn giao qua thanh ghi** — `exc_dispatch` trả `FrameReturn` (`#[repr(C)]` hai `u64` ⇒ SysV
   trả ở `RAX`/`RDX`): `rax` = `rip` để `iretq` về, `rdx` = `rsp` frame tác vụ kế (`0` = giữ
   frame hiện tại). Stub: `test rdx, rdx` → `mov rsp, rdx`. `rax`/`rdx` là **của hart đang chạy**
   ⇒ không có trạng thái chia sẻ, không cần khoá, không cần `cli` (ngắt đã tắt bởi interrupt
   gate `0x8E`). Ô `static VIFAST_SCHED_NEW_RSP` **bị xoá**.
2. **Kiểm frame trước `iretq`** — `sched::on_tick` gọi `frame_ok(rsp, slot)` trước mỗi lần
   chuyển; hàm này kiểm hai lớp:
   - **nội dung** — `psched::switch_frame_ok(rip, cs, rflags, rsp)`: `rip` khác 0 và canonical,
     `cs == 0x08`, `rflags` có `IF`, `rsp` khác 0/căn 8/canonical (hàm **thuần**, 7 ca host);
   - **vị trí** — `rsp` phải nằm trong stack **của chính slot** (A/B: 16 KiB do ảnh khai).
     `main` **không** có cận: sau `umode::run` toàn bộ phần còn lại chạy trên **stack syscall**
     (`__syscall_stack_top`, 32 KiB), không phải `__stack_top` — lấy stack boot làm cận là
     **dương tính giả** (đã bắt được ngay lượt boot đầu của bản sửa: frame `main` ở `0x128970`,
     dưới `__stack_bottom` `0x139000`).
   Không qua ⇒ `# sched: BAD frame slot=… rsp=0x…` (+ `fields …` khi `rsp` đọc được) rồi `halt`.
3. **Chẩn đoán cho lần sau** — nhánh exception **không** được chờ in thêm dòng thứ hai
   `# exc: frame rsp=0x… ss=0x… task_rsp=0x…` trước `halt`. Dòng này **không** khớp
   `^# exc: vec=` nên **không** đổi bất biến đếm-exception của `soak-x86_64.sh`/`run-x86_64.sh`.

### 15.5 Đột biến (đều ĐỎ, có chẩn đoán)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **cửa sổ bàn giao dài** | bản vá tạm (không commit) — xem §15.3 | ảnh **trước** KT-298: treo sau `# sched: switch b→a n=2`, không marker (ĐỎ 3/3). Ảnh **sau** KT-298: scheduler hoàn tất, không `BAD frame`, không `#GP` |
| (ii) **`rsp` chưa map** | `--features bad-rsp` | `# sched: BAD frame slot=2 rsp=0x700000000000` + `halt`, không marker (ĐỎ). KT-286 ghi `#PF`/`#DF`; nay phép kiểm bắt **trước** `iretq` nên đỏ **có chẩn đoán** — bất biến runner không đổi ("không marker ⇒ ĐỎ") |
| (iii) **frame thiếu `rsp`/`ss`** | (đã biết từ KT-286) frame 20 qword | `#PF` rồi `#DF` — không đổi |

### 15.6 Đo được (log boot sau sửa)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: ticks=7 tasks=2
# sched: runs a=332143 b=312291 switches=6 ab=1 ba=2 a_rsp=0x12e158 b_rsp=0x132158
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
```

`a_rsp`/`b_rsp` **không đổi** so với KT-286 ⇒ bố cục frame và stack tác vụ giữ nguyên; chỉ
đường bàn giao `rsp` đổi.

### 15.7 Còn thiếu / rủi ro

1. **Không** chứng minh được boot 410 là **đúng** đường này (1/600, không tái hiện được bằng
   soak thường: `--smoke 200 3` cho **198/200**, hai boot đỏ đều là `not ok 10 - timer: ticks`
   của KT-297, **không** có `#GP`). Bằng chứng là: (a) ô `static` **quan sát được từ hart khác**
   — chứng minh bằng bản vá kéo dài cửa sổ (§15.3), và (b) ba dữ kiện định vị ở §15.1 khớp với
   "hai hart chung một stack". **Chưa** loại trừ được một nguyên nhân khác chưa thấy.
2. **Không** sửa `Console` thành có khoá: hai hart cùng in vẫn xen ký tự. Đó là **triệu chứng**
   từng thấy, không phải gốc; thêm khoá sẽ che mất lớp lỗi này thay vì sửa nó.
3. **Không** tuyên bố K5' đạt.

## 16. KT-301 — `wait_ipi`/`wait_report`: chờ theo **biến đếm**, không theo **thời gian**

Trạng thái: **DONE** (KT-301, 2026-11-21). Quyết định: **D2-201** (K5' bước 1), **D2-216** (vòng 15:
sửa ba lỗi thật soak bắt được), **D2-198** (uỷ quyền commit nhánh task), **D2-05** (test đỏ trước),
**D2-07** (không retry).

> **D2-218 không tồn tại** trong `v2/agent/DECISIONS.md` tại base `a78e67a9` (mục cuối là **D2-217**).
> Nhiệm vụ yêu cầu đọc D2-218 ⇒ đã đọc D2-201/D2-216 và **ghi lại** việc thiếu mục này thay vì tự
> suy ra nội dung.

**Không** tuyên bố K5' đạt.

### 16.1 Triệu chứng nguyên văn (boot 422/600 của KT-298)

```text
# smp: ipi send dest=1 vector=0x30 low=0x4030 high=0x01000000
# smp: ipi sent=1 count=0 waited_us=18446744073709551615 ok=0
not ok 20 - smp: ipi delivered
# TAP 20 FAIL — dừng (không in marker)
# smp: ipi recv=1 intid=0x30
```

`wait_ipi` hết trần **100 ms** **trước** khi IPI tới; dòng `# smp: ipi recv=1` in **sau** dòng đỏ.
Máy lúc đo: `qemu_foreign=3 loadavg_1m=6.95` (ba QEMU của sub-agent khác). Cùng dấu hiệu xuất hiện
ở KT-297 soak 600 (boot 27/600: `# smp: ipi sent=1 count=0 waited_us=18446744073709551615`).

### 16.2 Gốc rễ: trần **thời gian tuyệt đối** là sai **đại lượng**

`wait_ipi`/`wait_mask` cũ dừng khi **PIT kênh 2** trôi quá `IPI_TIMEOUT_US` (100 ms) /
`ONLINE_TIMEOUT_US` (2 s). PIT đếm theo **thời gian tường**; còn điều kiện thật cần chờ là **hart
đích chạy được handler**. Dưới tải ngoài, host deschedule vCPU hart phụ > 100 ms thì IPI **không
thể** tới trong hạn — phép chờ sai **đại lượng**, không phải ngưỡng quá chặt. Vì vậy **không** nới
ngưỡng thời gian (100 ms **giữ nguyên**); đổi **cách chờ**.

### 16.3 Bản sửa

| Thành phần | Trước KT-301 | Sau KT-301 |
|---|---|---|
| Điều kiện dừng **chính** | thời gian PIT > 100 ms (IPI) / 2 s (report) | **biến đếm**: `SMP_IPI_COUNT >= 1` / bit `SMP_ALIVE_MASK` của hart |
| Trần | thời gian tuyệt đối | **số vòng** `SMP_SPIN_LIMIT = 100_000_000` (`vifast-plat-x86_64::smp`) |
| Thời gian | điều kiện dừng | **chốt an toàn**: chỉ hỏi **sau** khi trần vòng cạn; `IPI_TIMEOUT_US` (100 ms) và `ONLINE_TIMEOUT_US` (2 s) **không đổi** |
| Vòng chờ | `pit_now` **mỗi** vòng (3 cổng I/O) + `pause` mỗi vòng | `load` mỗi vòng; `pause` mỗi 64 vòng; nạp ảnh PIT mỗi 65.536 vòng |

Một helper `wait_counter(clock, latch_us, ready)` dùng cho **cả** `wait_ipi` và `wait_mask` (report
và online). Chốt an toàn thời gian có thêm trần `SMP_SPIN_ROUNDS = 4` vòng: nếu trần vòng cạn
**trước** khi `latch_us` trôi qua (vòng chạy nhanh bất thường) thì chờ thêm, tối đa 4 vòng; PIT
hỏng (elapsed luôn 0) cũng dừng sau 4 vòng ⇒ **không treo**.

**Số đo chọn trần** (QEMU 11.1.3 TCG, máy 12 core, đo 2 lần): 2.000.000 vòng `load` + `pause` mỗi
64 vòng ≈ **51 ms** ⇒ **~39 triệu vòng/s** ⇒ 100.000.000 vòng ≈ **2,5 s**, dư ~25× so với trần cũ
100 ms. Đo đối chứng: `pause` **mỗi** vòng đắt ~0,5 µs (2.000.000 vòng ≈ 1,05 s) nên **không** dùng
mỗi vòng; không `pause` thì 2.000.000 vòng ≈ 25 ms nhưng vòng chờ chiếm vCPU — `pause` mỗi 64 vòng
là điểm cân bằng.

### 16.4 Kiểm **đỏ-trước** (D2-05): `--features slow-ipi`

Feature **chỉ để kiểm** (không phải sản phẩm): hart đích **nhận** IPI nhưng trì hoãn việc tăng bộ
đếm **150 ms** (đo bằng PIT kênh 2) — mô phỏng **tất định** độ trễ lập lịch host của boot 422.
150 ms **lớn hơn** trần cũ 100 ms và **nhỏ hơn** trần số vòng mới ⇒ cùng một ảnh cho hai kết quả.

| Ảnh | Log nguyên văn | Kết quả |
|---|---|---|
| **trước** KT-301 (trần 100 ms) | `# smp: ipi sent=1 count=0 waited_us=18446744073709551615 ok=0` + `not ok 20 - smp: ipi delivered` + `# TAP 20 FAIL — dừng` rồi mới `# smp: ipi recv=1 intid=0x30` | **ĐỎ** — tái hiện **đúng** dấu hiệu boot 422 |
| **sau** KT-301 (chờ biến đếm) | `# smp: ipi recv=1 intid=0x30` + `# smp: ipi sent=1 count=1 waited_us=152117 ok=1` + `ok 20 - smp: ipi delivered` | **XANH** — chờ 152 ms, vượt trần cũ 100 ms mà vẫn đúng |

### 16.5 Đột biến (đều ĐỎ, **không treo**)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **không gửi IPI** | `--features no-ipi` | `# smp: ipi sent=0 count=0 waited_us=18446744073709551615 ok=0` ⇒ `not ok 20`, không marker (ĐỎ) |
| (ii) **hart phụ không tăng bộ đếm** | `--features no-ipi-count` (hart đích **nhận** và in `# smp: ipi recv=1` nhưng **không** tăng `SMP_IPI_COUNT`) | `# smp: ipi recv=1 intid=0x30` + `# smp: ipi sent=1 count=0 waited_us=18446744073709551615 ok=0` ⇒ `not ok 20` (ĐỎ). **Không treo**: log đạt `not ok 20` trong **< 10 s** (guard 10 s), tức trần **số vòng** hữu hạn đã cắt |
| (iii) **IPI tới hart không tồn tại** | `--features bad-ipi-dest` (dest = `APIC ID 7`) | `# smp: ipi sent=1 count=0 waited_us=18446744073709551615 ok=0` ⇒ `not ok 20` (ĐỎ), không treo |

Đột biến (ii) là ca **trực tiếp** của thay đổi này: bộ đếm **không** tăng thì **không** có đường
nào khác làm `wait_ipi` trả `Some` — điều kiện dừng đúng là biến đếm, không phải thời gian.

### 16.6 Đo được

Soak **600 boot / 3 luồng** trên ảnh KT-301: **600/600**, `rc_124=0`, `qemu_own=0`,
`qemu_foreign=0`, `verdict=OK`, wall **786 s**, ảnh không đổi giữa soak
(`BUILD_ID=37238e1ad66748d4`, `git=a78e67a9`). Trước KT-301 (KT-298, ảnh **trước** bản sửa timer
của KT-297): 597/600 — 2× TAP 10 `timer: ticks` (KT-297 đã sửa) + 1× TAP 20 IPI (KT-301 đã sửa).

Phân bố độ chờ đo được trên 600 boot (PIT kênh 2, µs): IPI p50 **622**, p99 **5.705**, max
**37.617**; report p50 **25.098**, p99 **141.256**, max **180.549**. p99 của **report** vượt trần
thời gian cũ 100 ms ⇒ độ trễ lập lịch của hart phụ **thường xuyên** vượt 100 ms dưới tải soak —
đúng lý do phải đổi **đại lượng** chờ.

### 16.7 Còn thiếu / rủi ro

1. **Trần số vòng** là proxy của thời gian: máy **chậm** thì chờ **lâu hơn** (đúng ý), nhưng nếu
   host deschedule BSP quá lâu thì một boot có thể chạm timeout 20 s của runner. Đo được: trần
   100.000.000 vòng ≈ 2,5 s danh định; dưới tải soak 3 luồng vẫn < 20 s.
2. **Không** sửa `Console` thành có khoá: hai hart cùng in vẫn xen ký tự (triệu chứng cũ, không
   phải gốc của KT-301).
3. **Không** tuyên bố K5' đạt: K5' còn các mục ngoài soak (xem §12.12).

## 17. KT-304 — tác vụ **ring 3** trong scheduler (K5')

Trạng thái: **DONE** (KT-304, 2026-11-21). Quyết định: **D2-201** (K5' bước 1), **D2-212**
(KT-286 preemption), **D2-216** (KT-297/298), **D2-219** (KT-301), **D2-198** (uỷ quyền commit
nhánh task), **D2-05** (test đỏ trước), **D2-07** (không retry).

> **D2-219 không tồn tại** trong `v2/agent/DECISIONS.md` tại base `58a1f867` (mục cuối là
> **D2-218**). Nhiệm vụ yêu cầu đọc D2-219 ⇒ đã đọc D2-201/D2-212/D2-216 và **ghi lại** việc
> thiếu mục này thay vì tự suy ra nội dung.

**Không** tuyên bố K5' đạt.

### 17.1 Khảo sát trước: ba câu hỏi của task

**(1) `switch_frame_ok` (KT-298) có chấp nhận frame ring 3 không? — KHÔNG.** Hàm kiểm
`cs == KERNEL_CS` (`0x08`) ở điều kiện (2), nên frame `cs = 0x1B` bị từ chối **ngay**. Hàm cũng
**không nhận `ss`** (frame ring 3 cần `ss = 0x23`; frame kernel thật có ô `ss` **ngoài** frame vì
CPU chỉ đẩy 20 qword khi cùng CPL), và điều kiện `rsp` cũ (canonical/căn 8) **không đủ** — `rsp`
của frame ring 3 là **user rsp**, phải nằm trong trang stack user. `frame_ok` của ảnh cũng chỉ
nhận ba slot `MAIN/A/B`; `slot_stack` trả `None` cho slot khác.

**Đính chính selector**: task viết "`cs=0x23`/`ss=0x1b`" — trong code **ngược lại**:
`USER_CS = 0x1B` (code), `USER_SS = 0x23` (data) — xem `vifast-plat-x86_64::umode` và
`tests/umode.rs::user_selectors_literal`. Bản làm theo **code**.

**(2) `TSS.rsp0` có được nạp khi vào ring 3 không? — có, nhưng chỉ cho đường KT-270.**
`umode::run` gọi `tss::set_rsp0(0, __syscall_stack_top)` ngay trước `enter_user`. `sched::run`
thì **không** đụng `RSP0`: nếu `schedule()` `iretq` xuống ring 3 mà không nạp lại, tick timer từ
CPL3 sẽ đẩy frame vào **stack syscall** — đúng stack `main` đang chạy sau `Exit` — và ghi đè
frame của `main` đã lưu ở `SCHED_RSP[TASK_MAIN]` (đúng lớp lỗi boot 410 của KT-298). Vì vậy tác
vụ ring 3 có **stack kernel riêng** 16 KiB và `RSP0` được nạp **đỉnh stack ấy** trước lần `iretq`
đầu tiên.

**(3) Frame `iretq` ring 3 khác frame kernel ở đâu?** Cùng 22 qword/offset, nhưng `cs = 0x1B`,
`ss = 0x23`, `rsp` = **user rsp** (`USER_STACK_TOP - 16`), và `rsp` khởi tạo = `kernel_stack_top -
176` **không slack 8**: `iretq` đổi CPL **luôn** pop 5 qword (SDM Vol 3A §5.5), nên sau `iretq`
kernel `rsp = kernel_stack_top`; timer ngắt từ CPL3 đẩy 5 qword rồi stub đẩy 17 qword ⇒ frame mới
rơi **đúng** chỗ frame khởi tạo.

### 17.2 Cơ chế

| Thành phần | Thay đổi |
|---|---|
| `vifast-plat-x86_64::sched` | thêm `TASK_U` (slot 3), `TASK_SEQUENCE` (9 bước), `init_user_frame_words`, `switch_frame_ok(rip, cs, rflags, rsp, ss)` có **nhánh ring 3** |
| `vifast-kbin-x86_64::umode` | stub `.user` thứ hai `__user_sched_entry` (tăng bộ đếm `USER_COUNTER_VA` trong vòng lặp), `USER_TASK_RSP`, `USER_COUNTER_VA`, `sched_entry_va()` |
| `vifast-kbin-x86_64::sched` | `TASK_U_STACK` 16 KiB, `build_user_frame`, `slot_stack(TASK_U)`, `frame_ok` đọc thêm `ss`, `on_tick` đo preempt/resume, `run` nạp `TSS.RSP0` + zero bộ đếm + in bằng chứng + TAP 24 |
| `vifast-kbin-x86_64::main` | `TAP_TESTS = 26`; bỏ `.p2align 12` trong `.user` (stub thứ hai có thể được phát **trước** khối của `main.rs`; căn 4 KiB giữa section sẽ đệm tới 4096 và làm `ASSERT` của linker đỏ — section đã được linker script `ALIGN(4K)`) |
| `vifast-kbin-x86_64::net` | blk dời `tap_first + 9` → `+ 10` (TAP 25/26) |

Thứ tự `TASK_SEQUENCE`: `A→B→A→B→A→B→U→A→U→main` (9 lần chuyển, `MAX_SWITCHES = 9`). U bị
preempt **hai** lần (`U→A` bước 7, `U→main` bước 9) và được chạy lại **một** lần (`A→U` bước 8).
Bảng giữ **cả hai** chiều `A→B`/`B→A` (TAP 23 của KT-286) — không nới phép kiểm cũ.

### 17.3 Đo được (log QEMU nguyên văn)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: ticks=10 tasks=3
# sched: runs a=283832 b=157811 switches=9 ab=1 ba=2 a_rsp=0x12e168 b_rsp=0x132168 u_rsp=0x136170
# sched: umode task ran n=5015211 ring=3 preempted=2 resumed=1
# sched: umode counts first=4279886 last=5015211 cs=0x1b kernel_stack=0x136220
# sched: umode progress first=4279886 last=5015211 delta=735325
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - blk: device
ok 26 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| tác vụ ring 3 chạy thật | `n=5015211` — bộ đếm do **chính tác vụ** tăng trong vòng lặp `inc [rcx]` ở CPL3 |
| đang ở ring 3 khi bị preempt | `cs=0x1b` — `cs` của frame do CPU đẩy khi timer ngắt từ CPL3, ở **cả hai** lần preempt |
| preempt | `preempted=2` — rời U hai lần (`U→A`, `U→main`) |
| chạy lại | `resumed=1` — `schedule()` chọn U lại sau preempt; `first=4279886 > 0` (đã chạy trước lần preempt đầu) và `last=5015211 >= first` (bộ đếm **không giảm**) |
| stack kernel riêng | `u_rsp=0x136170`, `kernel_stack=0x136220` — frame nằm trong `TASK_U_STACK` 16 KiB, **không** đụng stack syscall của `main` |
| TAP | 26/26 xanh; runner 18/18 kiểm sạch + **19 ca âm** ĐỎ |

### 17.4 Đột biến bắt buộc (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **không nạp `TSS.RSP0`** | `--features no-rsp0` (nạp `RSP0 = 0`) | tick đầu tiên từ CPL3 đẩy frame vào địa chỉ 0 ⇒ `# exc: vec=8 err=0x0 rip=0x400000005a cs=0x1b rflags=0x206` + `# exc: frame rsp=0x11bf50 ss=0x23 task_rsp=0x4000001ff0` ⇒ halt, **không marker** (ĐỎ) |
| (ii) **`switch_frame_ok` từ chối `cs=0x1B`** | `--features reject-user-cs` | `# sched: BAD frame slot=3 rsp=0x136170` + `# sched: BAD frame fields rip=0x4000000050 cs=0x1b rflags=0x202 task_rsp=0x4000001ff0 ss=0x23` ⇒ halt, **không marker** (ĐỎ) |

### 17.5 Còn thiếu / rủi ro

1. **Một** tác vụ ring 3, **không** IPC giữa các tác vụ ring 3, **không** nhiều hart schedule
   (chỉ BSP), **không** priority, **không** sleep/wake — đúng phạm vi task.
2. Tác vụ ring 3 **không** gọi syscall (không có số hiệu mới — `spec/` không mở trong task này);
   bằng chứng "chạy lại" là **bộ đếm user** kernel đọc trực tiếp, không phải in từ ring 3.
3. `TASK_SEQUENCE` là **bảng cố định** (không phải runqueue): đủ cho phép đo "chọn → preempt →
   chạy lại", **không** phải scheduler sản phẩm.
4. **Không** tuyên bố K5' đạt: K5' còn "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot) và các mục
   ngoài soak (§12.12).

### 17.6 KT-308 — sửa **giả định sai** của phép kiểm TAP 24 (soak 600 bắt được)

Soak x86_64 600 boot trên ảnh KT-304 (KT-308, 2026-11-21) ra **599/600**: boot 331 in

```text
# sched: umode task ran n=275791 ring=3 preempted=2 resumed=1
# sched: umode counts first=275791 last=275791 cs=0x1b kernel_stack=0x136220
not ok 24 - sched: umode task
```

TAP 24 của KT-304 đòi `last > first` ("bộ đếm **tăng** giữa hai lần preempt"). Đó là **giả định
sai**: tick là **PIT 100 Hz** (10 ms); nếu vCPU bị host deschedule ≥ 10 ms đúng lúc `iretq` xuống
CPL3 thì tick kế đã **pending** ⇒ CPU nhận ngắt **trước** lệnh user đầu tiên ⇒ 0 vòng. Phân bố
`last - first` trên 600 boot: **min = 0** (chỉ boot 331), p1 = 58.490, p50 = 1.238.069,
p99 = 6.457.271; **13/600** boot < 100.000 ⇒ biên là **dải liên tục**, không phải lỗi logic của
đường resume (`resumed=1`, `cs=0x1b` ở cả hai lần, không exception).

Bản sửa đầu tiên thay `last > first` bằng `first > 0` — **cũng sai**: đo lại 600 boot trên máy
sạch ra **599/600**, boot 231 in

```text
# sched: umode task ran n=1194730 ring=3 preempted=2 resumed=1
# sched: umode counts first=0 last=1194730 cs=0x1b kernel_stack=0x136220
not ok 24 - sched: umode task
```

Tác vụ bị preempt lần đầu **trước khi thi hành lệnh nào** (`first=0`), rồi được resume và chạy
**1.194.730 vòng** (`last=1194730`). Boot 331 (`first>0`, `last==first`) và boot 231 (`first=0`,
`last>first`) là **hai ảnh ngược của cùng một race** ⇒ **không** cửa sổ nào được đòi có tiến bộ.

**Sửa cuối** (không nới ngưỡng nào khác): TAP 24 = `n > 0` (chạy **ít nhất một** cửa sổ) ∧
`resumed ≥ 1` ∧ `cs = 0x1B` ở **cả hai** lần preempt ∧ `last >= first` (bộ đếm **không giảm**);
thêm dòng `# sched: umode progress first=… last=… delta=…` (delta **có dấu**); runner đòi
`last >= first` + `delta >= 0` và thêm ba ca âm (q) `cs=0x08`, (r) `last < first`, (s) `n=0`.
Bằng chứng "chạy lại sau resume" là **tổng hợp** (qua 1.200 boot: boot nào tác vụ cũng chạy ít
nhất một cửa sổ, `n > 0`), **không** phải một cửa sổ cụ thể. Bằng chứng đo lại 600 boot trên máy
sạch: xem `v2/agent/JOURNAL/kt308-x86_64-soak2.md`.

## 18. KT-310 — **nhiều hart schedule**: hàng đợi chạy toàn cục + khoá

Trạng thái: **DONE** (KT-310, 2026-11-22). Quyết định: **D2-201** (K5' bước 1), **D2-212**
(KT-286 preemption), **D2-216** (KT-297/298), **D2-221** (nhiệm vụ KT-310), **D2-198** (uỷ quyền
commit nhánh task), **D2-05** (test đỏ trước), **D2-07** (không retry).

**Không** tuyên bố K5' đạt.

### 18.1 Khảo sát trước: ba câu hỏi của task

**(1) `sched.rs` có runqueue toàn cục hay per-hart? — KHÔNG có runqueue.** `vifast-plat-x86_64::
sched` là **bảng cố định** `TASK_SEQUENCE` (`[B, A, B, A, B, U, A, U, MAIN]`) + hàm thuần
`next_task`; `vifast-kbin-x86_64::sched` giữ trạng thái **toàn cục** (`SCHED_RSP[slot]`,
`SCHED_CURRENT`, `SCHED_SWITCHES`, …) — **một** bản, **không** per-hart, **không** khoá. Vì vậy
`schedule()` hiện **chỉ đúng khi một hart gọi**: hai hart cùng gọi sẽ cùng tăng `SCHED_SWITCHES`
và cùng đọc/ghi `SCHED_RSP` ⇒ hai hart `iretq` vào **cùng** một frame trên **cùng** một stack —
đúng lớp lỗi boot 410/600 của KT-293, nhưng ở tầng **chọn tác vụ** (KT-298 mới sửa tầng **bàn
giao `rsp`**).

**(2) `TASK_SEQUENCE` là bảng cố định — hart phụ đọc nó thế nào? — KHÔNG đọc được.**
`exc_dispatch` so `LAPIC_ID` với `smp::bsp_lapic_id()`: nhánh BSP mới gọi `sched::on_tick`, nhánh
hart phụ chỉ `smp::note_ap_tick()` + `lapic_eoi()`. Cho hart phụ vào **nguyên trạng** thì
`SCHED_SWITCHES` (toàn cục) bị hai hart cùng tăng ⇒ hai hart chọn **cùng** bước của bảng.

**(3) `TSS.rsp0` per-hart đã có (KT-278) — hart phụ `iretq` xuống ring 3 cần gì thêm?**
`tss::init(hart, …)` đã nạp `RSP0` = đỉnh slot stack **của hart ấy** và `gdt::install(hart, …)`
nạp **GDT riêng** có đủ `USER_CS`/`USER_SS`. Còn thiếu **hai** thứ: (a) `RSP0` của hart phụ đang
trỏ stack `hlt` của chính nó, **không** phải `TASK_U_STACK` — phải nạp lại **khi hart ấy chọn
`TASK_U`**, nếu không tick từ CPL3 ghi đè ngữ cảnh hart phụ (lớp lỗi `no-rsp0` của KT-304, ở hart
khác); (b) `frame_ok` chỉ nhận bốn slot `MAIN/A/B/U`, nên frame **của hart phụ** (do CPU đẩy trên
stack `hlt` của nó) **không** được đưa qua `frame_ok` — cùng lý do `TASK_MAIN` trả `None` ở
`slot_stack`: frame do CPU đẩy tại chỗ ngắt là hợp lệ **theo định nghĩa**.

### 18.2 Phương án: **(A) hàng đợi toàn cục + khoá**

Chọn **(A)** — nhỏ hơn (B), đúng khuyến nghị của task, và **tự nhiên** phát hiện double-pick:
hàng đợi nói "rảnh" mà bảng ownership nói "có chủ" ⇒ dừng có chẩn đoán. (B) (per-hart runqueue +
chuyển tác vụ qua IPI) cần thêm cơ chế **migration** — ngoài phạm vi ("**không** migration").

Phạm vi **hẹp có chủ đích**: pha nhiều hart chạy **sau** chuỗi cố định (TAP 22/23/24 **không** đổi
nghĩa, **không** nới ngưỡng nào); **hai** hart (BSP + hart 1); hàng đợi là **ba** tác vụ **đã có**
(`A`, `B`, `U`) nên có **chuyển** tác vụ thật mà **không** thêm tác vụ mới; **trần cứng**
`MH_HARD_ROUNDS = 48` nên boot **không** treo khi một hart im lặng (khuôn KT-301).

### 18.3 Cơ chế

| Thành phần | Thay đổi |
|---|---|
| `vifast-plat-x86_64::sched` | `MH_HARTS=2`, `MH_TASKS=[A,B,U]`, `MH_QUEUE_SLOTS=8`, `MH_MAX_ROUNDS=6`, `MH_HARD_ROUNDS=48`, `MH_FREE=u64::MAX`; hàm thuần `task_name`, `mh_all_picked`, `mh_end`, `mh_ok` |
| `vifast-plat-x86_64/tests/runqueue.rs` | **mới** — 6 test host (đỏ-trước): hằng, tên tác vụ, FIFO **có khoá** (không hart nào chia tác vụ), FIFO **không khoá** (double-pick **phải** bị phát hiện), `mh_end` (chờ hart kia + trần cứng), `mh_ok` (bốn điều kiện) |
| `vifast-kbin-x86_64::sched` | hàng đợi vòng `MH_QUEUE`/`MH_HEAD`/`MH_TAIL`, bảng `MH_OWNER`, khoá spin `MH_LOCK`, `MH_CURRENT`/`MH_PICKS`/`MH_FIRST`/`MH_FIRST_N`/`MH_ROUNDS`/`MH_DOUBLE`; `mh_lock`/`mh_unlock`/`mh_push`/`mh_pop`/`mh_tick`/`mh_start`/`on_tick_ap`; nhánh `MH_ACTIVE` trong `on_tick`; TAP 25 |
| `vifast-kbin-x86_64::smp` | `hart_of_lapic(id)` — tra `SMP_HART_LAPIC_ID` (dữ liệu **do chính hart phụ** công bố trước khi bật bit online) |
| `vifast-kbin-x86_64::main` | nhánh hart phụ của `exc_dispatch` gọi `sched::on_tick_ap(frame_rsp)`; `TAP_TESTS` 26 → **27** |
| `vifast-kbin-x86_64::net` | blk dời `tap_first + 10` → `+ 11` (TAP 26/27) |
| `Cargo.toml` | feature đột biến `bsp-only-sched`, `no-sched-lock` |

**Bốn bất biến của pha** (mỗi cái là một lớp lỗi thật): (1) **một chủ** — `MH_OWNER[task]` +
`compare_exchange`, hàng đợi chỉ chứa tác vụ **rảnh**; (2) **không double-pick** — `mh_pop` trả
`Err(task)` ⇒ `# sched: BAD double-pick` + halt; (3) **chờ hart kia** — `mh_end` đòi **mọi** hart
đã chọn ≥ 1 trước khi kết thúc (trừ trần cứng); (4) **trần cứng** — `MH_HARD_ROUNDS` ⇒ kết thúc dù
thiếu bằng chứng (TAP đỏ, **không** treo).

**Vì sao dòng của hart 0 in ở tổng kết, không in trong handler:** hart phụ in dòng của nó **trước**
khi tăng `MH_PICKS[1]` ("in trước, đếm sau" — khuôn KT-259/KT-278) và BSP chỉ tới phần tổng kết
**sau** khi pha kết thúc ⇒ **không** có hai dòng xen ký tự trên COM1. Nếu cả hai hart cùng in
trong handler thì hai dòng có thể xen nhau và regex của runner đỏ **oan**.

### 18.4 Đo được (log QEMU nguyên văn, `-smp 4`)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: hart=1 picked=a n=1
# sched: ticks=10 tasks=3
# sched: runs a=1245744 b=1471487 switches=9 ab=1 ba=2 a_rsp=0x12e178 b_rsp=0x132178 u_rsp=0x136180
# sched: umode task ran n=24270228 ring=3 preempted=2 resumed=1
# sched: umode counts first=12915741 last=24232588 cs=0x1b kernel_stack=0x136230
# sched: umode progress first=12915741 last=24232588 delta=11316847
# sched: hart=0 picked=b n=2
# sched: multi-hart harts=2 rounds=7 picks=[3, 4] first=[b, a] diff=2 double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
ok 26 - blk: device
ok 27 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| hart phụ `schedule()` | `# sched: hart=1 picked=a n=1` — hart 1 chọn **A** ở vòng 1 |
| BSP `schedule()` trên **cùng** hàng đợi | `# sched: hart=0 picked=b n=2` — BSP chọn **B** ở vòng 2 |
| hart phụ chọn **khác** tác vụ BSP đang chạy | `diff=2` (**KT-313**; `first=[b, a]` chỉ còn là **chẩn đoán**) |
| chuyển tác vụ thật trong pha | `rounds=7`, `picks=[3, 4]` — mỗi hart chạm **cả ba** tác vụ (kể cả `U` ring 3 trên hart 1) |
| không double-pick | `double=0` |
| TAP | **27/27** xanh; runner 19/19 kiểm sạch + **20 ca âm** ĐỎ |

### 18.5 Đột biến bắt buộc (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **hart phụ không schedule** | `--features bsp-only-sched` | `# sched: hart=0 picked=a n=1` + `# sched: multi-hart harts=2 rounds=48 picks=[48, 0] first=[a, ?] diff=0 double=0` ⇒ `not ok 25 - sched: multi-hart` + `# TAP 25 FAIL — dừng (không in marker)` (ĐỎ, **không** treo: `rounds=48` = trần cứng) |
| (ii) **bỏ khoá** | `--features no-sched-lock` (khoá no-op + cửa sổ **30 ms** bằng PIT kênh 2) | `# sched: hart=1 picked=a n=1` + `# sched: BAD double-pick hart=0 task=a n=1` ⇒ halt, **không marker** (ĐỎ) |

### 18.6 Còn thiếu / rủi ro

1. **Hai** hart (BSP + hart 1), **không** phải bốn: hart 2/3 vẫn chỉ `hlt` — đúng phạm vi task.
2. **Không** load balancing, **không** priority, **không** migration chủ động, **không** sleep/wake.
   Hàng đợi là FIFO **cố định** ba tác vụ, **không** phải scheduler sản phẩm.
3. Hart phụ **không** quay về `hlt` sau pha: nó ở lại trong tác vụ cuối (tick sau chỉ `resume`).
   Không ảnh hưởng boot (sau TAP 21 hart phụ không còn việc gì), nhưng là **nợ** nếu sau này cần
   hart phụ làm việc khác — khi đó phải thêm slot "ngữ cảnh riêng của hart phụ" và cho `frame_ok`
   nhận slot ấy.
4. Pha nhiều hart chạy **sau** chuỗi cố định nên nó **không** chứng minh "hai hart cùng chạy tác
   vụ trong lúc tác vụ ring 3 đang chạy" — chỉ chứng minh "hai hart cùng `schedule()` trên **một**
   hàng đợi". Đó là mốc thật của task, không hơn.
5. **Không** tuyên bố K5' đạt: K5' còn "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot) và các mục
   ngoài soak (§12.12).

## 19. KT-315 + KT-313 — **bốn** hart schedule, và bất biến `diff >= 1` thay `first[0] != first[i]`

Trạng thái: **DONE** (KT-315, 2026-11-22; bất biến của phép kiểm sửa bởi **KT-313**, áp cho bốn
hart bởi **KT-317**). Quyết định: **D2-221** (nhiệm vụ KT-315), **D2-222** (KT-313), **D2-212**,
**D2-216**, **D2-198**, **D2-05**, **D2-07**. Nhật ký đầy đủ:
[`../agent/JOURNAL/kt315-x86_64-fourhart.md`](../agent/JOURNAL/kt315-x86_64-fourhart.md),
[`../agent/JOURNAL/kt317-merge313.md`](../agent/JOURNAL/kt317-merge313.md) (hợp nhất KT-313 vào
KT-315).

**Không** tuyên bố K5' đạt.

### 19.1 Khảo sát trước: mở lên bốn hart cần gì

**(1) Ba tác vụ + bốn hart ⇒ hart thứ tư đói vĩnh viễn.** Hàng đợi FIFO toàn cục; mỗi hart trả
tác vụ cũ rồi lấy tác vụ mới **trong cùng vùng tới hạn** ⇒ với `N` tác vụ và `N` hart, mỗi hart
luôn lấy lại **chính nó**. Với ba tác vụ, hart thứ tư `mh_pop` trả `Ok(None)` **mãi mãi** ⇒
`picks[3] = 0` ⇒ TAP 25 đỏ. **Phải thêm tác vụ `C`** (`TASK_C = 4`), không nới ngưỡng, không hạ
phạm vi.

**(2) Nhịp BSP chậm hơn hart phụ 10× ⇒ cần cổng khởi động.** PIT của BSP 100 Hz (10 ms); LAPIC
timer hart phụ ~1 ms. Không có cổng thì hart phụ hút sạch hàng đợi trước tick đầu của BSP. Cổng:
hart phụ chờ `MH_PICKS[0] != 0`.

**(3) Cổng khởi động không đủ ⇒ cần cổng giữ của BSP.** Đo được (boot 4/5 của lượt runner đầu):
hart 1 bị host deschedule, BSP đã **trả** `a` ở tick 2 ⇒ hart 1 chọn `a` ⇒ `first=[a, a, b, u]` —
hợp lệ nhưng làm phép kiểm "hart phụ chọn khác BSP" đỏ **oan**. BSP **giữ** tác vụ đầu của nó tới
khi mọi hart đã chọn lần đầu, tối đa `MH_HOLD_TICKS = 4` tick của chính nó (≈ 40 ms); hết hạn thì
trả và chọn lại mỗi tick ⇒ `MH_ROUNDS` tăng đều ⇒ trần cứng chạm ⇒ **không treo**.

**(4) `TSS.rsp0` per-hart (KT-278) đã đủ cho bốn hart `iretq` xuống ring 3** — không phải sửa:
`tss::set_rsp0(hart, …)` đã chạy cho **mọi** hart khi hart ấy chọn `TASK_U`. Đo được: hart 2 chọn
`u` và boot không có `# exc:` nào.

**(5) `frame_ok` không tranh slot giữa các hart** — frame lưu **theo tác vụ** (`SCHED_RSP[slot]`),
không theo hart; ownership bảo đảm không đồng thời. Cái thiếu là **slot thứ năm** cho `TASK_C`
(`frame_ok` + `slot_stack`).

### 19.2 Cơ chế

| Thành phần | Thay đổi |
|---|---|
| `vifast-plat-x86_64::sched` | `TASK_COUNT` 3 → **4**, `TASK_C = 4`, `TASK_SLOTS` 4 → **5**; `MH_HARTS` 2 → **4**; `MH_TASKS` 3 → **4**; `MH_QUEUE_SLOTS` 8 → **16**; `MH_MAX_ROUNDS` 6 → **8**; `MH_HARD_ROUNDS` **giữ 48**; `MH_HOLD_TICKS = 4`; `mh_harts_picked`, `mh_hold`; `mh_ok(rounds, picks, double, diff)` / `mh_end(rounds, picks, diff)` — điều kiện 4 là **`diff >= 1`** (**KT-313**, bỏ tham số `first`); `task_name` thêm `"c"` |
| `vifast-kbin-x86_64::sched` | `TASK_C_STACK` + `TASK_C_RUNS` + `task_c`; `slot_stack`/`frame_ok` nhận `TASK_C`; `MH_BSP_TICKS`; `MH_DIFF` (**KT-313**, đếm cho **cả ba** hart phụ 1/2/3 — **KT-317**); `mh_picks_now`/`mh_finish`; cổng khởi động + cổng giữ; dòng tổng hợp `harts=<số đo> … picks=[4] first=[4] diff=<n> double=0`; `tasks=4`; `runs … c=… c_rsp=…` |
| `Cargo.toml` | feature đột biến `two-hart-sched` |
| runner + soak | mẫu 4 hart, `tasks=4`, `diff ≥ 1` + `double=0` + `harts=4`, **bỏ** điều kiện `first`; ca âm (u) "chỉ 2 hart" + ca âm (v) `diff=0` ⇒ **22** ca âm; soak selftest **40** ca (2 dương + 38 âm) |

`harts=` là **số đo** (số hart có `picks ≥ 1`), không phải hằng `MH_HARTS` — nhờ vậy đột biến
"chỉ 2 hart" in `harts=2` và runner **đỏ**.

`first=[…]` **vẫn in** nhưng **không** còn là điều kiện: nó là **chẩn đoán** để thấy race
`first=[a, a, …]` (xem §19.3). Bất biến của pha là `diff >= 1` — **tồn tại** một lượt hart **phụ**
chọn tác vụ **khác** tác vụ BSP **đang chạy**.

### 19.3 KT-313 — soak 600 bắt **bất biến sai** của TAP 25 (`first[0] != first[i]`)

**Đo được** (soak 600 trên ảnh KT-310, `build=33773cfe64a48b7b`): **593/600**, 7 boot đỏ, **không**
retry (D2-07). Phân loại **từ log nguyên văn**:

| Lớp | Boot | Số | Nguyên nhân |
|---|---|---:|---|
| (A) `first=[a, a]` | 98, 211, 269, 304, 431 | **5** | **bất biến sai** của TAP 25 — `double=0`, scheduler đúng |
| (B) log xen ký tự + `# exc:`/`PANIC` | 64, 326 | **2** | **lỗi thật** khác (COM1 không khoá) — KT-316 |

Cả 5 boot (A) in **cùng một khuôn** (nguyên văn boot 98):

```text
# sched: hart=1 picked=a n=1
# sched: hart=0 picked=a n=4
# sched: multi-hart harts=2 rounds=6 picks=[2, 4] first=[a, a] double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
not ok 25 - sched: multi-hart
# TAP 25 FAIL — dừng (không in marker)
```

`double=0` (không double-pick), cả hai hart đều chọn, `rounds=6` — **mọi** điều kiện **thật** đều
đạt; chỉ `first[0] != first[1]` sai.

**Vì sao điều kiện ấy sai** (không phải lỗi scheduler): hàng đợi là FIFO **ba** tác vụ `[a, b, u]`;
mỗi hart **đẩy tác vụ cũ về trước** rồi mới pop. Nếu hart phụ tick trước và kịp **ba** lượt trước
lượt đầu của BSP (BSP còn ở `main`), thì `a` đã quay lại **đầu** hàng đợi đúng lúc BSP pop lần đầu:

```text
R1 h1: pop a (FIRST[1]=a)   R2 h1: push a, pop b   R3 h1: push b, pop u
R4 h0: pop a (FIRST[0]=a)   ← a quay lại đầu hàng đợi
R5 h1: push u, pop b        R6 h0: push a, pop u   ⇒ picks=[2, 4] first=[a, a]
```

Đúng khớp `picks=[2, 4]` của **cả 5** boot. Phân bố `first=` trên 392 log đọc được:
`a,b` 274 · `b,a` 84 · `u,a` 30 · **`a,a` 5** ⇒ ~1,3 %. Đây **cùng lớp lỗi** với `last > first`
của KT-308: một điều kiện **phụ thuộc thời điểm**, không được thiết kế bảo đảm.

**Sửa: `diff >= 1` thay `first[0] != first[i]`.**

| Thành phần | Thay đổi |
|---|---|
| `vifast-plat-x86_64::sched` | `mh_ok(rounds, picks, double, diff)` — **bỏ** tham số `first`, điều kiện 4 = `diff >= 1`; `mh_end(rounds, picks, diff)` — kết thúc bình thường **đòi** `diff >= 1` (trần cứng `MH_HARD_ROUNDS = 48` giữ nguyên) |
| `vifast-kbin-x86_64::sched` | `MH_DIFF` — đếm lượt hart **phụ** chọn tác vụ **khác** tác vụ BSP **đang chạy** (đọc `MH_CURRENT[0]` **trong** vùng tới hạn; cả hai phải là tác vụ thật); in `diff=<n>` trong `# sched: multi-hart`; `first=[…]` **vẫn in** để **chẩn đoán** nhưng **không** còn là điều kiện. **KT-317** đếm cho **cả ba** hart phụ (1, 2, 3), không chỉ hart 1 |
| `vifast-plat-x86_64/tests/runqueue.rs` | 10 test host; thêm `kt313_first_collision_is_not_a_failure` — **tái hiện đúng** chuỗi R1..R8 với **bốn** hart và đòi `mh_ok` **XANH** với `picks=[1, 5, 1, 1] first=[a, a, u, c] diff=3` |
| `v2/test/boot/soak-x86_64.sh` | đòi `diff ≥ 1` + `double=0` + `harts=4`; **bỏ** `first[0] != first[i]`; selftest **40 ca** (2 dương + 38 âm), gồm ca dương hồi quy `picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1` ⇒ **XANH** |
| `v2/test/boot/run-x86_64.sh` | như trên; ca âm mới (v) `diff=0` ⇒ ĐỎ; ca dương hồi quy KT-313 |

**Vì sao `diff >= 1` là bất biến thật:** sau lượt chọn **đầu** của BSP, BSP **sở hữu** một tác vụ
`X`; lượt chọn kế của hart phụ **không thể** là `X` (ownership) ⇒ `diff` tăng. `mh_end` **kéo dài**
pha tới khi `diff >= 1` (trần cứng 48 vòng vẫn chặn treo) nên điều kiện **không** phụ thuộc việc
hart nào tick trước. Đây là **sửa phép kiểm**, **không** nới ngưỡng nào (N=600, 20 s/boot, 3 luồng,
0 lỗi/N giữ nguyên).

**Bằng chứng bản sửa có tác dụng** (đo trên chính 600 log của lần soak thứ hai): `first=[a, a]`
xuất hiện **8/600** boot — **cả 8 đều `ok 25` XANH** (boot 17, 177, 229, 311, 346, 368, 520, 528);
`diff` phân bố **1..6** (`2` 255 · `3` 234 · `4` 75 · `1` 28 · `5` 5 · `6` 3) ⇒ **600/600** boot
`diff ≥ 1`; `double=0` **600/600**. Tức race **vẫn còn** (8/600 boot có cửa sổ đầu trùng — **nhiều
hơn** 5/600 của lần 1 vì máy tải nặng hơn) nhưng **không** còn là điều kiện đỏ — đúng thiết kế.

**Lớp (B) — lỗi thật, ghi nhận, không sửa ở đây.** Boot 64 và 326 có log **xen ký tự COM1** giữa
hai hart, kèm `# exc: vec=13 … rip=0x3000000000000`, `# exc: vec=1`, `# exc: vec=6`,
`PANIC: smp: arena … không căn 4 KiB`, `# sched: BAD frame …`. Comment cũ ở `sched.rs` khẳng định
"không có hai dòng xen ký tự" là **sai** khi có hai hart `schedule()` thật. Việc **khoá COM1** là
task riêng (**KT-316**) — mục này chỉ ghi nhận.

**Bài học — lần thứ ba trong tuần một phép kiểm có giả định phụ thuộc thời điểm:**

| Lần | Điều kiện sai | Vì sao sai | Sửa |
|---|---|---|---|
| KT-308 lần 1 | `last > first` (TAP 24) | PIT 100 Hz; host deschedule ≥ 10 ms đúng lúc `iretq` ⇒ tick kế pending ⇒ 0 vòng ở cửa sổ sau | `n > 0` ∧ `resumed ≥ 1` ∧ `cs=0x1B` cả hai lần ∧ `last >= first` |
| KT-308 lần 1 (bản sửa đầu) | `first > 0` | **ảnh ngược** của cùng race: preempt trước lệnh user đầu ⇒ `first = 0` | (bỏ — không cửa sổ nào được đòi tiến bộ) |
| **KT-313** | `first[0] != first[i]` (TAP 25) | FIFO **ba** tác vụ quay vòng; hart phụ kịp **ba** lượt trước lượt đầu của BSP ⇒ tác vụ đầu trùng | `diff >= 1` (hart phụ chọn **khác** tác vụ BSP **đang chạy**) |

> **Bất biến của một phép kiểm nhiều hart KHÔNG được phụ thuộc thứ tự/thời điểm giữa các hart**,
> trừ khi có **cơ chế bảo đảm** thứ tự ấy (hàng rào, khoá, hoặc pha kéo dài tới khi bằng chứng tồn
> tại). Cụ thể: **không** đòi "hart A chọn trước hart B", "lần đầu của A khác lần đầu của B",
> "cửa sổ thứ k có tiến bộ". Chỉ đòi những gì **cơ chế** bảo đảm: `double == 0` (ownership),
> mỗi hart đã chọn ≥ 1, và **tồn tại** một lượt hai hart chạy **khác** tác vụ (`diff >= 1`).
> Cách nhận ra: nếu điều kiện có thể **đảo** khi chỉ đổi **thứ tự tick** (không đổi code) thì nó
> là giả định thời điểm, **không** phải bất biến.

### 19.4 Đo được (log QEMU nguyên văn, `-smp 4`)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: hart=3 picked=b n=2
# sched: hart=1 picked=u n=3
# sched: hart=2 picked=c n=4
# sched: ticks=10 tasks=4
# sched: runs a=296300 b=228876 c=67113 switches=9 ab=1 ba=2 a_rsp=0x12e198 b_rsp=0x132198 u_rsp=0x13a1a0 c_rsp=0x136198
# sched: umode task ran n=11685558 ring=3 preempted=2 resumed=1
# sched: umode counts first=3665460 last=4989088 cs=0x1b kernel_stack=0x13a250
# sched: umode progress first=3665460 last=4989088 delta=1323628
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=10 picks=[3, 2, 2, 3] first=[a, u, c, b] diff=7 double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
ok 26 - blk: device
ok 27 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| **bốn** hart `schedule()` | bốn dòng `# sched: hart=0|1|2|3 picked=…` |
| hart phụ chọn **khác** tác vụ BSP đang chạy | `diff=7` (**KT-313**; `first=[a, u, c, b]` chỉ còn là **chẩn đoán**) |
| mọi hart chọn ≥ 1 | `picks=[3, 2, 2, 3]`, `harts=4` |
| tác vụ thứ tư **chạy thật** | `c=67113` vòng |
| không double-pick | `double=0` |
| TAP | **27/27** xanh; runner 20/20 kiểm sạch + **22 ca âm** ĐỎ (+ 1 ca dương KT-313) |

### 19.5 Đột biến (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **chỉ 2 hart** | `--features two-hart-sched` | `# sched: multi-hart harts=2 rounds=50 picks=[22, 28, 0, 0] first=[a, b, ?, ?] diff=28 double=0` ⇒ `not ok 25` + `# TAP 25 FAIL — dừng (không in marker)` (ĐỎ, **không** treo) |
| (ii) **bỏ khoá** | `--features no-sched-lock` | `# sched: BAD double-pick hart=1 task=b n=8` + `# sched: BAD double-pick hart=3 task=b n=8` + `# sched: multi-hart harts=3 rounds=48 picks=[42, 3, 0, 3] first=[a, u, ?, b] diff=6 double=3` ⇒ `not ok 25`, không marker (ĐỎ) |
| (iii) **`diff=0`** | ca âm của runner/soak (sửa log tốt) | `# multi-hart diff=0 — hart phụ chưa chọn tác vụ **khác** tác vụ BSP đang chạy` ⇒ ĐỎ — chứng minh bất biến **mới** có răng, không phải "bỏ phép kiểm" |

### 19.6 Còn thiếu / rủi ro

1. Bốn hart schedule **trong pha nhiều hart**, **không** phải "bốn hart cùng chạy tác vụ trong lúc
   tác vụ ring 3 đang chạy": pha vẫn chạy **sau** chuỗi cố định.
2. **Không** load balancing, **không** priority, **không** migration chủ động, **không** sleep/wake.
3. Hart phụ **không** quay về `hlt` sau pha (nợ KT-310, giữ nguyên) — nay là **ba** hart phụ.
4. **Cổng giữ** dựa trên tick của BSP (PIT 100 Hz): host deschedule hart phụ > 40 ms thì BSP trả
   tác vụ trước. Từ **KT-313** điều này **không** còn làm TAP 25 đỏ (điều kiện là `diff >= 1`,
   không phải `first`), nhưng nó làm `diff` **chậm** đạt hơn — trần cứng 48 vòng vẫn chặn treo.
5. `diff >= 1` chứng minh "**tồn tại** một lượt hart phụ chạy khác tác vụ BSP", **không** chứng
   minh "bốn hart chạy song song **lâu dài**" — đúng mức bằng chứng của pha (8–15 vòng), không hơn.
6. **Không** tuyên bố K5' đạt: K5' còn "như K3" (TAP đầy đủ smp 1|2|4 + soak **600** boot, D2-149)
   và các mục ngoài soak (§12.12). `--smoke 60` **không** phải bằng chứng cổng K5'.
