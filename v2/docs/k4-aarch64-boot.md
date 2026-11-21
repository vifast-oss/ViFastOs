# K4' bước 3 (phần đầu) — ViFastOs **boot thật trên AArch64** (QEMU `virt`)

Trạng thái: **B3.1–B3.9 + B3.7 (U-mode) + B3.10 (PCI ECAM + virtio-blk) DONE** — Agent 1
(toàn quyền A0), 2026-11-21.
**Soak 1.000 + `budget-aarch64.sh` DONE** (KT-265, §8) — điều kiện "như K3" đã đủ **phần soak**.
**U-mode/EL0 + `svc` DONE** (KT-269, §9) — `[calling.aarch64]` đã mở (D2-206).
**PCI ECAM + virtio-blk DONE** (KT-273, §10) — đọc được sector 0 của đĩa QEMU.
**virtio-net + DHCP + ARP + ICMP DONE** (KT-275, §11) — MAC thật, DHCP/ARP/ping thật của slirp.
**W^X toàn ảnh + cache maintenance trước DMA DONE** (KT-277, §11) — `.text`/`.rodata` **RX**,
`.data`/`.bss`/stack **RW**, trang guard **không map**; `dc civac`/`dc ivac` trước/sau DMA.
**Không** tuyên bố K4' đạt: tiêu chí K4' là "**như K3**" (TAP đầy đủ smp 1|2|4 + soak 1.000) —
xem §7 "Còn thiếu".

Nguồn sự thật: code + log trong `v2/agent/JOURNAL/kt247-*`, `kt248-*`, `kt249-*`, `kt250-*`,
`kt253-*`, `kt254-*`, `kt259-*`, `kt262-*`, `kt265-*`, `kt269-*`, `kt273-*`.
Quyết định: **D2-195** (mở bước 3, phạm vi), **D2-199** (Q4: nửa cao làm trong crate arch),
**D2-203** (KT-262: `TLBI` shootdown + smp 4), **D2-204** (KT-265: soak 1.000 + budget aarch64),
**D2-206** (KT-269: mở `[calling.aarch64]` + U-mode).

## 1. Phạm vi

| Bước | Nội dung | Task | Trạng thái |
|---|---|---|---|
| B3.1 | crate ảnh `vifast-kbin-aarch64` + `linker.ld` + `_start` + PL011 + PSCI halt | KT-247 | **DONE** |
| B3.2 | bảng vector 16 ô + `VBAR_EL1` + handler đồng bộ | KT-248 | **DONE** |
| B3.3 | bật MMU (TCR→MAIR→TTBR0→SCTLR→ISB) + identity map | KT-249 | **DONE** |
| B3.4 | nửa cao (`TTBR1_EL1`) + physmap | KT-253 | mở (D2-199) |
| B3.5 | `VSpace` thật | KT-254 | mở |
| B3.6 | `TLBI` + shootdown đa hart | KT-262 | **DONE** (smp 4, ack đủ 3 hart phụ) |
| B3.7 | U-mode + `eret` + `svc` | KT-269 | **DONE** (D2-206: `[calling.aarch64]` đã mở) |
| B3.8 | runner QEMU + TAP + ca âm | KT-250 | **DONE (phần đầu)** |
| B3.9 | **smp 2**: PSCI `CPU_ON` + GICv3 `SGI 0` (IPI) | KT-259 | **DONE** (2 hart) |
| B3.9+ | **smp 4** + ba SGI (IPI 0, shootdown 1, lệnh 2) | KT-262 | **DONE** (4 hart) |
| B3.10 | **PCI ECAM** + quét bus/device/function + gán BAR + **virtio-blk** đọc sector 0 | KT-273 | **DONE** (§10) |
| B3.11 | **virtio-net** (hai queue, MAC thật) + **DHCP** + **ARP** + **ICMP echo** | KT-275 | **DONE** (§11) |
| B3.11 | **W^X toàn ảnh** (`.text` RX / `.data` RW, guard không map) + **cache maintenance** trước/sau DMA | KT-277 | **DONE** (§11) |

**Không** làm trong bước này: UEFI/AAVMF (thiết kế đặt **sau** K5'), EL2/ảo hoá (thuộc H1),
`cfg(target_arch)` trong `vifast-core`/`vifast-plat-common` (AGENTS §4.8). **U-mode** (B3.7) chỉ
là **một** syscall (`DebugPutStr`) + `Exit` — **không** IPC từ EL0, **không** nhiều process,
**không** preemption (D2-206).

## 2. Đường boot

```text
qemu-system-aarch64 -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 \
                    -kernel <ELF> -no-reboot
```

**`gic-version=3` là bắt buộc từ KT-259**: QEMU `virt` **mặc định GICv2** (`arm,cortex-a15-gic`,
đo bằng `-M virt,dumpdtb` + `dtc`) — GICv2 **không** có redistributor và **không** có `ICC_*_EL1`
nên IPI `SGI 0` không thể chạy. Với `gic-version=3`, DTB ghi `arm,gic-v3` với GICD `0x0800_0000`
và GICR `0x080A_0000 + hart × 0x2_0000`.

- QEMU `virt` nạp **ELF trực tiếp** qua `-kernel` (không cần `Image`, không cần AAVMF).
- Ảnh link tại **`0x4008_0000`** (RAM `virt` bắt đầu `0x4000_0000`); `ENTRY(_start)`.
- `.stack` **NOLOAD** 64 KiB + `.stack_guard` 4 KiB (khuôn riscv64).
- `_start` (`global_asm!`): đặt `sp`, xoá `.bss`, gọi `kmain`; `x0` (con trỏ DTB) được chuyển vào
  `kmain` nhưng **chưa đọc** (chưa có FDT parser — nợ ghi ở §7).
- Halt: **PSCI `SYSTEM_OFF`** (`hvc #0`, `x0 = 0x8400_0008`). QEMU `virt` đặt PSCI conduit = HVC khi
  khách ở EL1 ⇒ QEMU thoát mã 0. Không có PSCI ⇒ rơi vào `wfi` (runner phân biệt bằng marker).

## 3. PL011 (UART0)

- Địa chỉ `0x0900_0000` (QEMU `virt`), offset/bit trong `vifast-plat-aarch64::pl011`.
- `write_byte` chờ `FR.TXFF` (bit 5) clear, có **trần** `TX_WAIT_LIMIT` (không treo vô hạn).
- `INIT_SEQUENCE` là **dữ liệu** (5 bước) ⇒ test host khẳng định **thứ tự**; `divisors()` là
  `const fn` (làm tròn nửa lên bằng số nguyên) ⇒ test được trên host.
- **Đo được**: QEMU in `# viFastOs aarch64: boot ok` **3/3 lần**, `qg_boot rc=0`.

## 4. Bảng vector + trap đồng bộ

- 16 ô × `0x80`, căn `0x800`, nằm trong **`.text.vectors`** (input section `.vectors` — **không**
  đặt tên `.text.vectors` cho input, vì `*(.text .text.*)` sẽ nuốt mất).
- `VBAR_EL1` = địa chỉ bảng; mỗi ô là stub asm lưu `ESR_EL1`/`FAR_EL1`/`ELR_EL1`/`SPSR_EL1` rồi gọi
  handler Rust.
- **Đo được**: `brk #0` ⇒ `ec=0x3c` (BRK), `elr` khớp `expect`, `trap: other=0`; 8 test host
  khẳng định **offset tuyệt đối** từng ô (`0x000`..`0x780`) + `VBAR_BASE_MASK`.
- **Đột biến**: lệch `0x80` từ ô 3 ở tầng host ⇒ test ĐỎ; lệch `.org` ở asm ⇒ kiểm ELF ĐỎ.
- **KT-262 — stub phải giữ `x9`**: stub đặt chỉ số ô vào `x9`; KT-248 làm `mov x9, #idx` **trước**
  khi cất `x9` ⇒ giá trị gốc **mất**, sau `eret` `x9` = chỉ số ô (4 với trap ở EL1h). Lỗi chỉ lộ
  khi mã sau điểm fault còn giữ `x9` là con trỏ sống (KT-262: `ldar x9, [x9]` với `x9 = 4` ⇒
  `FAR=0x4`). Vá: `str x9, [sp, #-16]!` ở stub, `ldr x9, [sp], #16` sau khi hồi khung.

## 5. MMU

Chuỗi **bắt buộc** (mỗi bước có marker trước/sau — R5 của thiết kế):

```text
TCR_EL1 → MAIR_EL1 → TTBR0_EL1 → SCTLR_EL1.M=1 → ISB
```

Hai điểm dễ sai, đã kiểm bằng test:

1. **`TTBR0_EL1` phải trỏ vào bảng mức 1, không phải gốc mức 0.** Với granule 4 KiB và `T0SZ = 25`
   (39-bit VA), walk bắt đầu ở **mức 1** (DDI 0487 bảng D5-5). `PageTable` dựng cây 4 mức ⇒ đưa gốc
   mức 0 vào `TTBR0` sẽ dịch sai **mọi** địa chỉ. `ttbr0_base()` lấy con của gốc; test
   `ttbr0_is_level1_table` chốt hành vi.
2. **Arena bảng trang không cần `unsafe`**: `#[repr(align(4096))] struct Arena([AtomicU64; N])`,
   đọc/ghi entry bằng `load`/`store(Relaxed)` ⇒ `unsafe` **vẫn chỉ** ở `asm.rs` (D2-17), và
   `BootMemory` test được trên host.

**Đo được**:

| Phép đo | Kết quả |
|---|---|
| `SCTLR_EL1.M` sau `ISB` | `on=1 m=1` (giá trị reset `0xc50839` được **giữ**, chỉ thêm bit `M`) |
| marker qua VA | `before == after`, `same=1` |
| **probe VA `0x5000_0000`** (ngoài RAM 128 MiB) | đọc ra `magic` ⇒ **translation thật**, không chỉ bật bit |
| frame đã dùng | `used=8` / `frames=32` |
| `.data + .bss` | **140 KiB** (trần 256 KiB — bất biến I7) |

**Ba đột biến bắt buộc** (ghi trung thực **tầng nào** quan sát được):

| # | Đột biến | Host | QEMU |
|---|---|---|---|
| (i) | bỏ `ISB` | **ĐỎ** (chuỗi phải đủ 5 bước) | **không quan sát được** — TCG áp dụng `msr sctlr_el1` ngay |
| (ii) | `TCR.T0SZ` 25→24 | **ĐỎ** | **treo có kiểm soát** `rc=124`, dừng đúng bước 4/5 |
| (iii) | `MAIR` slot 1 sai | **ĐỎ** | **không quan sát được** — TCG không mô hình hoá thuộc tính bộ nhớ |

⇒ **Không** được gọi (i)/(iii) là "đã kiểm trên phần cứng".

### 5.1 `TLBI` + shootdown đa hart (B3.6 — KT-262)

`TLBI` chỉ xoá TLB của **PE thực thi nó**; KT-254 vì thế chỉ làm mới TLB **cục bộ** và ghi đó là
**nợ thật**. KT-262 trả nợ bằng `VSpace::unmap_shootdown`:

```text
hart 0 (sửa bảng trang):  dsb ishst → tlbi vae1is → dsb ish → isb
                          → SGI 1 tới MỌI hart phụ → chờ đủ ack (trần 50e6 vòng)
hart phụ (handler SGI 1): tlbi vae1is → dsb ish → isb → ack (AtomicU32)
```

- Chuỗi bước là **mảng hằng** (`shootdown::LOCAL_SEQUENCE`/`REMOTE_SEQUENCE`) ⇒ test host khẳng
  định **thứ tự tuyệt đối**; hart phụ **không** có `dsb ishst` (nó không ghi bảng trang).
- **Ba SGI khác nhau**: `0` IPI (KT-259), `1` shootdown, `2` lệnh. Dùng chung số thì `note_ipi`
  đếm lẫn và dòng `# smp: ipi recv=1` xuất hiện nhiều lần ⇒ runner ĐỎ.
- **Hart phụ chờ bằng `wfe`** (không `wfi`): `sev` đặt event register kể cả khi hart phụ chưa vào
  `wfe` ⇒ không mất đánh thức; spin thì đốt CPU TCG.
- **Fault theo hart** (`faults.rs`): TAP 9 (hart 0) và TAP 13 (hart 1) phải phân biệt được **hart
  nào** đã fault — một ô toàn cục sẽ ghi đè lẫn nhau.

**Đo được** (TAP 13/14/15, log `kt262-boot.log`):

| Phép đo | Kết quả |
|---|---|
| hart 1 **nạp** entry vào TLB (`read1`) | `0x5653_5041_4345_0001` = magic, `match=1` |
| shootdown | `targets=0xe expected=3 acks=3 timeout=0 send_failed=0x0 ok=1` |
| hart 1 đọc **lại** sau `unmap` | `fault=1 va=0x6000_0000 access=Read reason=NotMapped` |
| bốn hart | `online mask=0xf alive=0xf harts=4 mpidr_ok=1` |

**Đột biến** (log `kt262-mutation.log`):

| # | Đột biến | Host | QEMU |
|---|---|---|---|
| (i) | hart phụ không `tlbi vae1is` | **ĐỎ** | **không quan sát được** — QEMU flush TLB mọi CPU cho `*IS` |
| (ii) | bỏ `dsb ish` | **ĐỎ** | **không quan sát được** — TCG không mô hình hoá barrier |
| (iii) | không chờ ack | — | **ĐỎ** — `not ok 14`, `acks=0 expected=3` |

Vì (i) không đỏ, có **đối chứng dương**: bỏ `tlbi vae1is` **của hart 0** (chỉ giữ SGI) ⇒ TAP 13
**vẫn xanh** ⇒ một mình đường SGI đã xoá TLB của hart 1. Đó là bằng chứng QEMU-observable rằng
hart phụ **thật sự** chạy `tlbi` trong handler.

**Hai lỗi thật phát hiện trong lúc làm** (không phải đột biến):

1. **Stub vector xoá `x9`** của mã bị ngắt (từ KT-248): `mov x9, #idx` đứng **trước** `stp x8, x9`
   ⇒ sau `eret` `x9` = chỉ số ô. KT-262 lộ ra (`ldar x9, [x9]` với `x9 = 4` ⇒ `FAR=0x4`). Vá:
   cất `x9` vào khung 16 B trước khi dùng.
2. **`GICR_IPRIORITYR<n>` là thanh ghi theo BYTE**: ghi 32 bit vào `base + 0x400 + sgi` lệch lề với
   `sgi = 1` ⇒ **alignment fault** trên Device memory (`FAR=0x080B_0401`). KT-259 chỉ bật `SGI 0`
   nên chưa lộ. Vá: `asm::mmio_write_u8`.

## 6. Runner + TAP

```bash
bash v2/test/boot/run-aarch64.sh      # build + boot + TAP + 14 ca âm
```

- Dùng `qemu_guard.sh` (process group + timeout + dọn con) — **`perl -e 'alarm N; exec @ARGV'`
  KHÔNG giết được QEMU** (qemu bỏ qua `SIGALRM`).
- **16/16 kiểm sạch** (KT-262 + KT-265 + KT-269 + KT-273 + KT-277): ELF (`.text` @
  `0x4008_0000`, `.text.vectors` là TEXT, 16 stub đúng offset), boot (**TAP 23/23** + marker
  đúng 1 lần + **8 dòng `# smp:`** + **3 dòng `# tlbi:`** + **`# umode: el0 ran svc=0 ret=<len>`**
  + **`# pci: ecam … class=0x060000 … empty=0xffff`** + **`# pci: virtio-blk found …`** +
  **`# blk: sector0 hex=<khớp đĩa tạm> … magic=1`** + **`# wx: … text=RX data=RW ok=1 …
  fault_ec=0x25 …`** + **`# dma: cache-clean ok=1 lines=<n> bytes=<m> …`** + **`# conf image:`
  khớp ELF** — đúng một lần mỗi dòng), **14 ca âm** (marker sai, thiếu `ok`, có `not ok`, plan
  sai, log rỗng, thiếu dòng hart 1 online, IPI in hai lần, **thiếu dòng `# tlbi:`**, **thiếu ack
  shootdown**, **hash ảnh lệch ELF**, **thiếu dòng `# umode:`**, **không có đĩa ⇒ `# blk: no
  device` + TAP đỏ**, **thiếu dòng `# wx:`**, **thiếu dòng `# dma:`**) đều **ĐỎ đúng**.
- Nối vào cổng: bước **56** `QEMU aarch64 (K4' bước 3)` — chạy ở **cả T1 và T2**.

## 7. Còn thiếu (ghi rõ, không giấu)

1. **Timer per-hart / preemption**: chưa — chưa có `CNTP`/`CNTV` per-hart, chưa có scheduler.
   `smp 4` **đã** xong (KT-262).
2. **Soak 1.000** boot: **DONE** (KT-265, §8) — **0/1000 lỗi**, 3 luồng, 20 s/boot, không retry.
3. **U-mode** (B3.7): **DONE** (KT-269, §9) — `[calling.aarch64]` đã mở (D2-206). Phạm vi hẹp:
   **một** syscall (`DebugPutStr`) + `Exit`; **chưa** IPC từ EL0, **chưa** nhiều process,
   **chưa** preemption, **chưa** `FaultKind::Exit` qua `fault_ep` (chưa có procsrv).
4. **Mỗi hart một address space/ASID**: chưa — bốn hart dùng **cùng** bảng trang `VSpace`
   (hart 0 ở VA nửa cao, hart phụ ở VA thấp).
5. **DTB**: `_start` nhận con trỏ trong `x0` nhưng **chưa đọc**; địa chỉ thiết bị đang là hằng của
   `resources.rs` (memory map QEMU `virt`).
6. **W^X toàn ảnh**: **DONE** (KT-277, §11) — `.text`/`.text.vectors`/`.el0`/`.rodata` **RX**,
   `.data`/`.bss`/`.smp_stacks` **RW**, trang guard của stack per-hart **không map**. **Chưa**
   KPTI, **chưa** PAN/UAO (ngoài phạm vi D2-209).
7. **`budget.sh` bản aarch64**: **DONE** (KT-265) — `v2/test/boot/budget-aarch64.sh`, trần
   262.144 B, `.smp_stacks` báo riêng (§8).
8. **16 ô vector dùng chung một stub**: IRQ/FIQ chưa bật (`DAIF` mask đủ); khi bật ngắt phải tách
   handler theo ô.
9. **Shootdown theo dải VA**: chỉ một VA mỗi lần (`vae1is`); `vmalle1is`/`aside1is` có hằng
   encoding nhưng chưa dùng. **Không lock**: một shootdown tại một thời điểm (chưa preemption).
10. **`TLBI` trên QEMU TCG**: `vae1is` flush TLB **mọi** CPU (`tlb_flush_page_by_mmuidx_all_cpus_synced`),
    nên đột biến "bỏ `tlbi` ở hart phụ" **không** đỏ trên QEMU — xem `kt262-mutation.log` §5 và
    đối chứng dương ở §5.1. **Không** được gọi là "đã kiểm trên phần cứng".
11. **PCI**: chỉ **virtio-blk** (KT-273, §10). **Chưa** virtio-net/gpu/input trên aarch64, **chưa**
    MSI-X (dùng poll `used`), **chưa** hotplug, **chưa** PCIe 64-bit MMIO window
    (`0x80_0000_0000` — BAR modern của virtio nằm trong cửa sổ 32-bit nên chưa cần). ECAM map
    **16 MiB = 16 bus** (không phải 256 MiB): 256 MiB cần **129** frame bảng trang, vượt arena
    44 frame và trần `.data+.bss`; QEMU `virt` đặt **mọi** thiết bị trên bus 0 (đo `info pci`).
12. **`# build:` (KT-86) chưa có trên ảnh aarch64**: cổng danh tính bản dựng của riscv64 chưa áp
    cho aarch64; bù bằng `# conf image:` (D2-80) + sha256 + `git HEAD` trong header soak. **Không**
    gọi là tương đương KT-86.

## 8. Soak 1.000 + ngân sách aarch64 (KT-265)

```bash
bash v2/test/boot/soak-aarch64.sh 1000 3     # cổng: N=1.000, 3 luồng, 20 s/boot, không retry
bash v2/test/boot/soak-aarch64.sh --smoke 3 3 # N < 1.000: LUÔN exit 1 (không phải bằng chứng)
bash v2/test/boot/soak-aarch64.sh --selftest  # 1 ca dương + 18 ca âm, không mở QEMU
bash v2/test/boot/budget-aarch64.sh <ảnh> product
bash v2/test/boot/budget-aarch64.sh --selftest
```

**Bất biến mỗi boot** (cùng khuôn `soak.sh` riscv64, **không nới**): đúng 1 `TAP version 13`,
đúng 1 `1..21`, đúng 21 dòng `ok` (đủ 1..21), 0 `not ok`, đúng 1 marker
`# viFastOs aarch64: boot ok`, đúng 1 `# conf image:` **khớp ELF**, `# smp: online mask=0xf …
harts=4 mpidr_ok=1` + bằng chứng từng hart + ba dòng `# tlbi:` + `# umode: el0 ran …` +
`# pci: ecam …` + `# pci: virtio-blk found …` + `# blk: sector0 …` (hex khớp đĩa tạm), 0
`PANIC`, quá 20 s ⇒ đỏ. **KT-273** thêm đĩa tạm + `-nic none` vào `QEMU_ARGS` (cùng cấu hình
runner) — nếu không ảnh in `not ok 20/21` và soak đỏ vì lý do sai.

**Đo được** (log `kt265-soak-1000.log`, `kt265-soak-results.log`):

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot |
| boot đỏ | **0** (`results.txt`: 1000 `ok`, 0 `fail`) |
| wall | 247 s (≈ 4 boot/s) — **thời gian KHÔNG HỢP LỆ**: mẫu `qg_infra after` thấy **QEMU x86_64 của sub-agent KT-266** (`qemu_foreign=1`), không phải QEMU của soak (`qemu_own=0`) |
| guard | `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| ảnh | `# conf image: build=68b890daa9e08f6b text=38072`, sha256 `b7568e13…`; **không** đổi giữa soak |
| `.data+.bss` | **155.872 B** / trần **262.144 B** (59,5 %) |
| `.smp_stacks` (NOLOAD) | **557.056 B** — báo riêng, **không** cộng trần |

**Dòng `# conf image:`** (D2-80, KT-265): `vifast_plat_aarch64::image` băm FNV-1a 64 trên
`[__text_start, __text_end)` **từng byte** (`asm::read_u8`; đọc `u64` sẽ vượt `__text_end` vì
`text=38072` không chia hết 8) và in ngay sau `1..15`. `image_digest.py` băm **cùng** section từ
ELF ⇒ runner/soak đối chiếu được hai phía. `unsafe` **không** tăng: chỉ `asm::read_u8` (D2-17);
`image.rs` không có `unsafe`; host **panic** (fail-loud) và có 5 test host (5 vector FNV chuẩn).

**Nợ P20 (đo được, chưa sửa)**: sha ảnh phụ thuộc **cách viết đường dẫn** build — cùng cây, cùng
lệnh, `--manifest-path v2/kernel/Cargo.toml` (từ `/tmp/kt264-s2`) cho `build=31e033672c0648de`,
còn `--manifest-path /tmp/kt264-s2/v2/kernel/Cargo.toml` (từ `/`) cho `build=68b890daa9e08f6b`
(`.text` lệch 3.707 byte, cùng cỡ). Soak **tự build + tự băm trong cùng một lần chạy** nên nhất
quán; **không** so sha ảnh giữa hai lần chạy khác cách gọi.

## 9. U-mode (EL0) + `svc` — B3.7 (KT-269, D2-206)

### 9.1 `spec/abi.toml [calling.aarch64]` — đã mở

`reserved = true` đã bỏ; mục này **đối xứng từng trường** với `[calling.riscv64]` (cùng vai trò,
chỉ đổi tên thanh ghi + cơ chế chuyển), cộng ba trường **bổ sung tường minh** cho aarch64
(`trap`/`stack`/`clobbered` — riscv64 đóng băng nên không được thêm vào mục ấy):

| Trường | riscv64 | aarch64 |
|---|---|---|
| `syscall_number` | `a7` | **`x8`** |
| `args` | `a0`, `a1` | **`x0`, `x1`** |
| `message_info` | `a2` | **`x2`** |
| `mr_registers` | `a3..a6` | **`x3..x6`** |
| `return_error` / `return_value` | `a0` / `a1` | **`x0` / `x1`** |
| `trap` | — (đóng băng) | **`svc #0`** |
| `stack` | — (đóng băng) | **SP_EL0 user / SP_EL1 kernel** |
| `clobbered` | — (đóng băng) | **`x0`, `x1`, `nzcv`** |
| `preserved` | `sp, gp, tp, s0..s11, fs0..fs11` | **`sp, x19..x28, x29, d8..d15`** |

`vifast-gen` sinh hằng từ `[calling.*]` cho **arch đã mở** (`AARCH64_*`, `RISCV64_*`,
`CALLING_ARCHES`) vào `v2/lib/vifast-abi/src/generated.rs` + bảng trong `docs/abi.md`; ảnh dùng
hằng **sinh** cho số hiệu syscall (`SYS_DEBUG_PUT_STR`, `SYS_EXIT`) — `global_asm!` nhận `const`
operand nên stub EL0 đổi theo spec, không có số 12/14 viết tay (D2-11). `check_abi.py` kiểm
**đối xứng cấu trúc** (cùng tập khoá với riscv64 + đúng ba trường bổ sung) và `x86_64` **vẫn**
`reserved = true` (D2-10).

### 9.2 Đường chạy

```text
high_entry (EL1h, VA nửa cao)
  ├─ map `.el0` (PA trong ảnh) tại EL0_CODE_VA = 0x7000_0000, Perms::USER_RX
  ├─ map trang stack user (PA trong .bss) tại EL0_STACK_VA = 0x7000_1000, Perms::USER_RW
  ├─ vs.activate()  (TTBR0 = bảng VSpace; VA mới chưa có entry TLB cũ)
  ├─ SP_EL0 ← 0x7000_2000, ELR_EL1 ← entry, SPSR_EL1 ← 0x3c0 (EL0t | DAIF mask), ISB, eret
  ↓
EL0: adr x0, msg; mov x1, #12; mov x8, #12 (DebugPutStr); svc #0
  ↑ ô 8 (Lower EL AArch64 Sync), EC=0x15: đọc ESR.ISS (imm16) + x8 (số hiệu) + x0/x1 (ptr/len)
    → in chuỗi user qua PL011 → x0 = 0 (error), x1 = 12 (value) → eret (ELR đã là instruction kế)
  ↓
EL0: mov x8, #14 (Exit); mov x0, #0; svc #0
  ↑ handler: TAP 18 (EL0 **đã chạy tiếp** sau `svc` đầu) → ELR ← el0_done, SPSR ← EL1h
  ↓
el0_done (EL1h): marker + halt
```

**Ba điểm kỹ thuật đo được, không suy đoán:**

1. **`svc` đặt `ELR_EL1` = instruction KẾ TIẾP** (DDI 0487 §D17.2.37 — preferred return address
   của SVC), **khác `brk`** (ELR trỏ vào chính instruction, KT-248 phải `+4`). Đo được:
   `# umode: svc … elr=0x70000010` trong khi `svc` ở `0x7000_000c`. Vì vậy handler **không**
   cộng 4; cộng 4 là bỏ một instruction của EL0.
2. **Kernel đọc được trang user** vì `SCTLR_EL1.SPAN = 1` (PAN không bật khi vào exception) —
   nếu PAN bật, lần đọc chuỗi đầu fault và handler in `# umode: fault` thay vì im lặng.
3. **`Perms::USER_RX` đặt `PXN = 1`** ⇒ nếu `eret` sai chế độ (SPSR để EL1h), EL1 **không**
   thực thi được trang EL0 — đột biến (i) dừng ở instruction abort `EC=0x21` trước cả `svc`.

### 9.3 Đo được (log `boot-1.log`, TAP 16/17/18)

```text
# umode: eret entry=0x70000000 sp=0x70002000 spsr=0x3c0 mode=EL0t code_va=0x70000000 code_pa=0x4008b000 code_bytes=52 stack_va=0x70001000 stack_pa=0x40097000
ok 16 - umode: el0 entered
el0 says hi
ok 17 - umode: svc handled
# umode: el0 ran svc=0 ret=12
# umode: svc nr=12 imm=0 elr=0x70000010 ptr=0x70000028 len=12 printed=12 error=0 ret=12
ok 18 - umode: returned to el0
# umode: exit code=0 nr=14 imm=0
# umode: done
# viFastOs aarch64: boot ok
```

| Phép đo | Kết quả |
|---|---|
| `SPSR_EL1` lúc `eret` | `0x3c0` — `M=0b0000` (EL0t), `DAIF` mask đủ |
| trang mã EL0 | `code_bytes=52`, một Frame 4 KiB, `USER_RX` |
| `svc` đầu | `imm=0`, `nr=12` (DebugPutStr), `elr=0x7000_0010` (= instruction kế) |
| trả kết quả | `x0=0` (error), `x1=12` (value) — đúng `[calling.aarch64]` |
| EL0 chạy tiếp | `svc` thứ hai `nr=14` (Exit) ⇒ TAP 18 |
| runner | **13/13** kiểm sạch, TAP **18/18**, **11 ca âm** ĐỎ đúng |

### 9.4 Đột biến bắt buộc (ghi trung thực **tầng nào** đỏ)

| # | Đột biến | Kết quả |
|---|---|---|
| (i) | `SPSR_EL1` để EL1 (`M=0b0101`) | **ĐỎ** — `# trap: slot=4 off=0x200 ec=0x21 il=1 far=0x70000000 elr=0x70000000 spsr=0x3c5` (instruction abort từ EL1: trang EL0 là `PXN=1`), **không** có TAP 16, **không** marker |
| (ii) | trang EL0 map `KERNEL_RX` (không `user=true`) | **ĐỎ** — `# umode: fault ec=0x20 esr=0x000000008200000f far=0x70000000 elr=0x70000000 spsr=0x3c0 mode=EL0t` (instruction abort **từ EL0**), **không** có TAP 16, **không** marker |
| (iii) | handler **không** tiến `ELR_EL1` | **Không phải đột biến trên AArch64**: `svc` đã đặt ELR = instruction kế, nên "không tăng" là hành vi **đúng** (xem §9.2 điểm 1). Đột biến tương đương đã chạy: **(iii-a)** lùi `ELR` về chính `svc` (`elr - 4`) ⇒ `svc` thứ hai đọc `x0 = 0` (giá trị trả của lần trước) ⇒ `# trap: slot=4 ec=0x25 far=0x0` — **ĐỎ, dừng có kiểm soát**; **(iii-b)** lùi `ELR` về **đầu stub** ⇒ `svc` lặp vô hạn, log bị chặn ở **256 B** (21 dòng `el0 says hi`), `qg_boot rc=124` (timeout 5 s) — **ĐỎ, treo có kiểm soát**, không có marker/TAP 18 |

**Không** được gọi (i)/(ii) là "đã kiểm trên phần cứng": cả hai đo trên QEMU TCG.

### 9.5 Còn thiếu của B3.7 (ghi rõ)

1. **Một** syscall (`DebugPutStr`) + `Exit`; **không** IPC từ EL0, **không** nhiều process.
2. **Không** preemption/timer từ EL0; `DAIF` mask đủ trong `SPSR_EL1` nên EL0 không nhận ngắt.
3. `Exit` **chưa** đi qua `fault_ep`/`FaultKind::Exit` (chưa có procsrv) — nó `eret` về EL1h
   tại `el0_done` để in marker rồi tắt máy.
4. **Không** có `Tcb`/`VSpace` object cho tiến trình EL0: trang mã/stack được map trực tiếp vào
   `VSpace` của ảnh, chưa qua cap/Untyped (nợ M17 của object layer).
5. **Không** tuyên bố K4' đạt — tiêu chí K4' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000).

## 10. PCI ECAM + virtio-blk — B3.10 (KT-273)

### 10.1 ECAM ở đâu — **đo**, không suy đoán

QEMU `virt` aarch64 **không** đặt ECAM ở `0x3f00_0000` (địa chỉ của bản QEMU cũ / của riscv64).
Đo bằng DTB của chính QEMU 11.1.3 trên máy đo:

```bash
qemu-system-aarch64 -M virt,gic-version=3,dumpdtb=/tmp/virt.dtb -cpu cortex-a72 -m 128M -display none
dtc -I dtb -O dts /tmp/virt.dtb | sed -n '/pcie@10000000/,/^};/p'
#   reg    = <0x40 0x10000000 0x00 0x10000000>                        ← ECAM 0x40_1000_0000, 256 MiB
#   ranges = <0x1000000 0x00 0x00 0x00 0x3eff0000 0x00 0x10000        ← PIO  0x3eff_0000, 64 KiB
#             0x2000000 0x00 0x10000000 0x00 0x10000000 0x00 0x2eff0000 ← MMIO32 0x1000_0000
#             0x3000000 0x80 0x00 0x80 0x00 0x80 0x00>                 ← MMIO64 0x80_0000_0000
```

Ba vùng này nằm trong bảng tài nguyên aarch64 (`resources::{PCI_ECAM, PCI_MMIO32, PCI_PIO}`) và
được kiểm **không chồng lấn** ở host test.

### 10.2 Vì sao ECAM phải đi qua **cửa sổ VA riêng**

PA `0x40_1000_0000` có bit 38 = 1 nhưng bit 63..39 = 0 ⇒ **không canonical** với 39-bit VA
(D2-147) ⇒ **không** identity-map được. Cửa sổ **physmap** cũng không cứu được:
`PHYSMAP_BASE + PA` **tràn `u64`** (nửa cao 39-bit chỉ phủ PA `< 2^38` = 256 GiB; ECAM ở
256 GiB + 16 MiB). Vì vậy driver map ECAM vào **cửa sổ VA riêng** `pci::ECAM_VA_BASE =
0x8000_0000` (nối tiếp dãy VA đã dùng: `0x5000_0000` probe, `0x6000_0000` VSpace,
`0x7000_0000` EL0) và đọc `VA = ECAM_VA_BASE + (PA - ECAM_BASE)`. Cửa sổ BAR (`0x1000_0000`,
2 MiB) thì identity-map được — cùng khuôn riscv64.

**Cửa sổ ECAM map 16 MiB = 16 bus**, không phải 256 MiB: ảnh chỉ map trang 4 KiB (không dùng
block descriptor) nên mỗi 2 MiB cần một bảng mức 3 ⇒ 256 MiB cần **129 frame**, vượt arena boot
44 frame và trần `.data+.bss` 256 KiB (I7). QEMU `virt` đặt **mọi** thiết bị trên bus 0 (đo
`info pci`: host bridge dev 0, virtio-blk dev 1 khi `-nic none`), nên 16 bus là 16× dải cần;
`SCAN_BUSES` **suy từ** số byte đã map, không viết tay.

### 10.3 Đo được (log `boot-1.log` của runner)

```text
# pci: map ecam_pa=0x4010000000 ecam_va=0x80000000 ecam_pages=4096 bar_va=0x10000000 bar_pages=512 frames_used=37
# pci: ecam pa=0x4010000000 va=0x80000000 host=1b36:0008 class=0x060000 rev=0x00 empty=0xffff functions=2 buses=16
ok 19 - pci: ecam
# pci: virtio-blk found bus=0 dev=1 func=0 vendor=0x1af4 device=0x1001 class=0x010000 rev=0x00 bar=4 kind=mem64 size=0x4000 assigned=0x10000000 readback=0x10000000 command=0x0006 caps=1 other_bars=0 reserved=6
ok 20 - pci: virtio-blk found
# blk: sector0 hex=5649464153544f532d4b543237332d53 ascii="VIFASTOS-KT273-S" magic=1 capacity=8 status=0x0f dma=0x400c8000 desc=0x400c8000 avail=0x400c8080 used=0x400c8098
ok 21 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| ECAM | PA `0x40_1000_0000` → VA `0x8000_0000`, **4096** trang (16 MiB) |
| host bridge | `1b36:0008`, class **`0x060000`**, rev `0x00`; slot trống trả **`0xffff`** |
| quét | **16** bus × 32 device × 8 function; **2** function có mặt (host bridge + virtio-blk) |
| virtio-blk | `1af4:1001` (transitional), BAR **4** `mem64` cỡ `0x4000`, gán `0x10000000`, **đọc lại khớp** |
| `command` | `0x0006` = `MEMORY | BUS_MASTER` (đọc lại sau khi bật) |
| capability | đủ bốn (`common`/`notify`/`device`/`isr`) cùng BAR 4, `offset+length ≤ bar_size` |
| sector 0 | hex `5649464153544f532d4b543237332d53` = `VIFASTOS-KT273-S`, `magic=1`, `capacity=8` |
| DMA | trang 4 KiB trong `.bss`; descriptor mang **PA** (`high_to_pa`), không phải alias nửa cao |
| frame | `frames_used=37` / arena **44** (KT-273 nâng 32 → 44; 7 dự phòng) |
| `.data+.bss` | **221.392 B** / trần **262.144 B** (84,5 %) |

**Ca âm "không có đĩa"** (runner, boot lần hai **không** `-drive`): ảnh in `# pci: no virtio-blk
stage=nodevice error=NoDevice`, `# blk: no device`, `not ok 20`, `not ok 21`, **không** marker ⇒
`check_log` ĐỎ. Đây là ca âm **thật** (không phải biến đổi log): nó chứng minh driver
**fail-closed** khi thiếu thiết bị.

### 10.4 Bất biến của driver

- Feature đàm phán **duy nhất** `VIRTIO_F_VERSION_1` + `VIRTIO_BLK_F_FLUSH` **nếu** thiết bị có
  (`vifast_virtio::blk::negotiated_features`, test host); gửi `T_FLUSH` khi chưa đàm phán là
  vi phạm spec §5.2.3.
- Thứ tự spec §2.7: descriptor → `avail->ring` → **`dsb sy`** → `avail->idx` → **`dsb sy`** →
  notify → poll `used->idx` → **`dsb sy`** → đọc `used->elem`/status.
- `used->id` phải khớp head descriptor (0); lệch ⇒ `id_mismatch` + lỗi, không im lặng.
- Request dựng bằng `vifast_virtio::blk::build_request` — **cùng** mã test host kiểm, không ghép
  header bằng tay (bài học v1 #2).
- Trần poll bằng `CNTPCT_EL0` (2 s) — bước 3 **chưa** có timer per-hart (nợ §7.1); `CNTFRQ_EL0 = 0`
  ⇒ lùi về trần **số vòng** (`POLL_SPIN_LIMIT`), ghi rõ trong mã.
- Trạng thái driver bằng `Atomic*` (không `UnsafeCell`): crate `#![deny(unsafe_code)]` ngoài
  `asm` (D2-17); trang DMA nằm ở `asm::dma_page` — mô-đun duy nhất được `unsafe`.

### 10.5 Còn thiếu của B3.10 (ghi rõ)

1. **virtio-blk + virtio-net** đã có; **chưa** virtio-gpu/input trên aarch64, chưa MSI-X
   (poll `used`), chưa hotplug. TCP/DNS trên aarch64: xem §11.5.
2. **ECAM 16/256 bus** — lý do frame/trần ở §10.2; bus ≥ 16 **không** quét được (đọc ra ngoài
   cửa sổ đã map ⇒ data abort), không phải "bỏ qua cho nhanh".
3. **Cache maintenance**: **DONE** (KT-277, §11) — `dc civac` từng dòng + `dsb ish`/`dsb sy`
   trước notify, `dc ivac` sau khi thiết bị ghi. **Giới hạn**: QEMU TCG **không** mô hình hoá
   cache nên **hiệu ứng** không quan sát được; tầng quan sát được là bộ đếm `lines=` (§11.3).
4. **Không** dùng ngắt: driver poll; `ISR` chỉ đọc chẩn đoán.
5. **Không** tuyên bố K4' đạt — tiêu chí K4' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000).

## 11. virtio-net + DHCP + ARP + ICMP — B3.11 (KT-275)

### 11.1 Phạm vi

D1 (virtio-net) trước KT-275 **chỉ có trên riscv64** (`d1-net.md`). KT-275 mở **cùng** đường đó
trên aarch64, dùng lại **nguyên** codec `vifast-net` (KT-244) và **không** chép codec nào vào
kernel:

| Tệp | Nội dung |
|---|---|
| `kernel/vifast-plat-aarch64/src/virtio_net.rs` | driver virtio-pci modern: hai queue `rx`/`tx`, MAC thật, `send_frame`/`recv_frame_wait`, `dhcp_acquire`, `arp_resolve`, `ping` |
| `kernel/vifast-kbin-aarch64/src/net.rs` | bốn phép đo + TAP 22/23/24/25 |
| `kernel/vifast-plat-aarch64/tests/virtio_net.rs` | **7 test host** cho phần thuần (bố cục DMA, ARP request/reply, DHCP DISCOVER/parse, ICMP echo, hằng số) |

**Không** làm (đúng phạm vi D2-208 + KT-275): TCP/DNS trên aarch64, virtio-gpu/input trên aarch64,
MSI-X, net server U-mode. **Không** tuyên bố K4' đạt.

### 11.2 Khác khuôn riscv64/x86_64 ở ba điểm (do kiến trúc)

1. **DMA bằng PA**: sau `activate` hart 0 chạy ở alias nửa cao ⇒ caller đổi bằng
   `virtio_net::high_to_pa` (cùng công thức `virtio_blk` KT-273) trước khi ghi descriptor.
2. **Trạng thái bằng `Atomic*`**, không `UnsafeCell`: crate `#![deny(unsafe_code)]` ngoài `asm`
   (D2-17). Trang DMA là **trang thứ hai** trong `asm` (`dma_page_net`) — dùng chung trang với
   virtio-blk thì driver thứ hai ghi đè ring của driver thứ nhất (hỏng im lặng).
3. **Trần thời gian bằng `CNTPCT_EL0`** (2 s poll, 3 s/bước mạng); `CNTFRQ_EL0 = 0` ⇒ `now_us`
   trả tick (ngân sách **ngắn hơn** thật ⇒ fail-closed, không treo).

### 11.3 Đo được (log `boot-1.log` của runner, TAP 22..25)

```text
# pci: map ecam_pa=0x4010000000 ecam_va=0x80000000 ecam_pages=4096 bar_va=0x10000000 bar_pages=512 net_bar_va=0x10400000 net_bar_pages=512 frames_used=38
# pci: ecam pa=0x4010000000 va=0x80000000 host=1b36:0008 class=0x060000 rev=0x00 empty=0xffff functions=3 buses=16
# pci: virtio-blk found bus=0 dev=2 func=0 vendor=0x1af4 device=0x1001 … assigned=0x10000000 readback=0x10000000 …
ok 21 - blk: sector0
# net: device bus=0 dev=1 func=0 vendor=0x1af4 device=0x1000 bar=4 kind=mem64 size=0x4000 assigned=0x10400000 readback=0x10400000 mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020 dma=0x400cd000
ok 22 - net: device
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
ok 23 - net: dhcp
# net: arp gw=52:55:0a:00:02:02
ok 24 - net: arp
# net: ping 10.0.2.2 ok n=3 rtt=0
ok 25 - net: ping
# net stats: tx_frames=6 tx_bytes=860 rx_frames=6 rx_bytes=1466 notifies=13 errors=0 timeouts=0 id_mismatch=0
# viFastOs aarch64: boot ok
## 12. W^X toàn ảnh + cache maintenance trước DMA — B3.12 (KT-277)

### 11.1 Vấn đề và cách tách

Trước KT-277 **mọi** đường map vùng ảnh dùng **một** khối `Perms::KERNEL_RWX` (bảng boot nửa
thấp, alias nửa cao, physmap, và ba đường tương ứng trong `VSpace`) ⇒ `.text` **ghi được** từ
EL1, W^X không có (nợ §7.6 cũ). KT-277 tách theo **symbol linker**, không theo địa chỉ viết tay:

```text
  [__text_start, __data_start)   RX   .text + .text.vectors + .el0 + .rodata
  [__data_start, __kernel_end)   RW   .data + .bss + .smp_stacks (NOLOAD)
```

- `__data_start` là symbol **mới** trong `linker.ld`, đặt ở đầu `.data` (căn 4 KiB).
- `.rodata` nằm trong vùng **RX**: nó là dữ liệu **chỉ đọc** (vtable `core::fmt`, bảng chuỗi…).
  `.data.rel.ro` **không** nằm trong `.rodata` ở ảnh này — linker script gom nó vào `.data`
  (`*(.data .data.*)`) nên nó **RW**, đúng bản chất "dữ liệu" của nó. Đây là điểm đã lường trước
  trong phiếu giao việc; **không** phải sửa gì thêm.
- Trang **guard** của stack per-hart (`MAX_HARTS` record `guard 4 KiB | stack 64 KiB`) **không
  map**: `wx::ImageLayout::rw_ranges` trả các khoảng RW **đã trừ** 8 trang guard. Tràn stack đi
  xuống chạm guard ⇒ fault; KT-277 **không** kiểm fault đó (phạm vi: chỉ cần không map), nhưng
  bất biến "guard không map" được kiểm **hai** tầng: test host (`rw_ranges_exclude_guard`,
  `map_low_perms`) và **số trang** ở runner (§11.3).
- **Một** định nghĩa cho **sáu** đường map (`wx::map_low`/`map_high`/`map_phys` cho bảng boot;
  `wx::map_vspace_low`/`map_vspace_high`/`map_vspace_phys` cho `VSpace`) ⇒ không có đường map thứ
  hai lệch quyền.
- Alias nửa cao dùng **một** gốc (`__text_start`) cho **mọi** khoảng: `map_high_alias_from` thêm
  tham số `origin_pa` thay vì chép vòng lặp map. Nếu mỗi khoảng lấy đầu của chính nó làm gốc,
  alias của `.data` lệch đúng bằng kích thước vùng RX và kernel nhảy vào giữa `.data` sau khi đổi
  `TTBR1` — test host `map_high_alias_origin` bắt lỗi ấy.

### 11.2 Đo được (log `boot-1.log` của runner)

```text
# mmu: map kernel=0x40080000-0x40152000 pages=202 uart=1 gic=32 probe_va=0x50000000 probe_pa=0x40096000
# mmu: high alias=0x40080000-0x40152000 base=0xffffffc080000000 pages=202 physmap_base=0xffffffc000000000 pages_physmap=202
# vspace: map va=0x60000000 pa=0x40098000 walk=0x40098000 match=1 pages_high=202 pages_physmap=202 pages_image=202 pages_mmio=289
# wx: text=RX data=RW ok=1 text_bytes=77824 data_bytes=782336 guard_pages=8 guard_bytes=32768 text_va=0xffffffc080000100 data_va=0xffffffc080013000 text_perms=RX data_perms=RW fault_ec=0x25 fault_access=Write fault_va=0xffffffc080000100 fault_elr=0xffffffc080004cf4 low_fault_ec=0x25 low_fault_va=0x40080100 text_unchanged=1 data_rw=1 data_in_rw=1
ok 22 - wx: text RX (write faults), data RW
# dma: cache-clean ok=1 lines=64 bytes=4096 ops=1 inval_lines=64 inval_bytes=4096 inval_ops=1 poll_inval=1 line=64 ctr=0x000000008444c004
ok 23 - dma: cache maintenance before/after DMA
```

| Phép đo | Kết quả |
|---|---|
| thiết bị | `1af4:1000` (transitional), bus 0 dev 1, BAR **4** `mem64` `0x4000`, gán `0x1040_0000` (**leaf 2**, không đụng leaf blk `0x1000_0000`), đọc lại khớp |
| MAC | `52:54:00:12:34:56` — **đọc từ `virtio_net_config`**, không hardcode |
| DHCP | `ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2` (slirp thật) |
| ARP | `gw=52:55:0a:00:02:02` — reply **thật** của slirp |
| ICMP | `n=3` reply, `rtt=0` ms (đồng hồ ảo của TCG) |
| bộ đếm | `errors=0 timeouts=0 id_mismatch=0` |
| `.data+.bss` | **225.488 B** / trần **262.144 B** (86,0 %) — thêm 4 KiB trang DMA mạng |
| frame table | `frames_used=38` / arena **44** (leaf BAR mạng tốn 1 frame) |

### 11.4 Ca âm (fail-closed)

| Ca | Cách dựng | Kỳ vọng |
|---|---|---|
| **không có mạng** | cùng ảnh + đĩa, **không** `-netdev`/`-device virtio-net-pci` | `# net: no device` + `not ok 22..25`; runner coi là **đạt** khi ảnh đỏ đúng cách |

Runner `run-aarch64.sh`: **15/15** kiểm sạch, TAP **25/25**, **13 ca âm** (12 cũ + ca "không có
mạng"). Soak `soak-aarch64.sh`: `--selftest` **23/23** (1 dương + 22 âm), smoke **20/20** boot.

### 11.5 Còn thiếu của B3.11 (ghi rõ)

1. **TCP/DNS** trên aarch64 — chưa có (riscv64 có từ KT-260/KT-268).
2. **Không** dùng ngắt: driver poll `used`; `ISR` chỉ đọc chẩn đoán. **Không** MSI-X.
3. **Không** cache maintenance (`dc civac`/`dsb`) trước khi bàn giao ring — QEMU TCG không mô
   hình hoá cache nên DMA nhất quán; trên phần cứng thật **chưa** làm, **chưa** đo.
4. **Không** net server U-mode, **không** socket API — toàn bộ stack chạy trong kernel ảnh.
5. **Không** tuyên bố K4' đạt — tiêu chí K4' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000).
| vùng RX | `text_bytes=77824` (76 KiB) = `.text` + `.text.vectors` + `.el0` + `.rodata` |
| vùng RW | `data_bytes=782336` (764 KiB) = `.data` + `.bss` + `.smp_stacks` (NOLOAD) |
| guard | `guard_pages=8`, `guard_bytes=32768` — **không** nằm trong khoảng RW nào |
| số trang map | **202** = `(77824 + 782336 − 32768) / 4096` — khớp **chính xác**; nếu 8 trang guard bị map thì **210** |
| quyền lá (đọc lại **từ bảng trang**) | `.text` = `RX` (read + exec, **không** write), `.data` = `RW` (read + write, **không** exec) |
| ghi vào `.text` từ EL1 (alias nửa cao) | **fault**: `EC=0x25` (data abort same EL), `WnR=1`, `FAR=0xffffffc080000100` |
| ghi vào `.text` từ EL1 (identity nửa thấp) | **fault**: `EC=0x25`, `FAR=0x40080100` |
| `.text` sau lần ghi bị từ chối | **không đổi** (`text_unchanged=1`) |
| ghi vào `.data` | **thành công**, đọc lại đúng (`data_rw=1`, `data_in_rw=1`) |
| cache line | `line=64`, `CTR_EL0=0x000000008444c004` (`DminLine=4` ⇒ `4 << 4 = 64`) |
| `dc civac` trước bàn giao | **64** dòng / **4096** B / **1** lần (`lines × line == bytes`) |
| `dc ivac` sau khi thiết bị ghi | **64** dòng / **4096** B / **1** lần + **1** lần `dc ivac` dòng `used->idx` trong poll |
| runner | **16/16** kiểm sạch, TAP **23/23**, **14 ca âm** ĐỎ đúng |
| `.data+.bss` | **221.416 B** / trần **262.144 B** (84,5 %) — không đổi đáng kể |

### 11.3 Đột biến — ghi trung thực **tầng nào** đỏ

| # | Đột biến | Host | QEMU |
|---|---|---|---|
| (i) | map vùng RX bằng `KERNEL_RWX` (như trước KT-277) | **ĐỎ** — 4 test (`map_low_perms`, `map_high_alias_origin`, `map_phys_perms`, `map_vspace_perms`) | **ĐỎ** — `# wx: … ok=0 … text_perms=RWX … fault_ec=0x00 … text_unchanged=0` ⇒ `not ok 22`; lần ghi **thành công** (8 byte đầu `.text` bị đổi) |
| (ii) | bỏ **bước** `CleanInvalidate` khỏi `HANDOFF_SEQUENCE` | **ĐỎ** — `handoff_sequence_cleans_before_notify` | **ĐỎ** — `# dma: cache-clean ok=0 lines=0 …` ⇒ `not ok 23` |
| (iii) | map **cả** 8 trang guard (bỏ tách guard khỏi khoảng RW) | **ĐỎ** — 5 test (`rw_ranges_exclude_guard`, `map_low_perms`, `map_high_alias_origin`, `map_phys_perms`, `map_vspace_perms`) | **ĐỎ** — `# mmu: map kernel=… pages=210` ⇒ runner: `số trang map vùng ảnh (210) khác (text+data-guard)/4096 = 202` |
| (iv) | bỏ **đúng lệnh** `dc civac` nhưng **giữ** vòng lặp (closure rỗng) | **XANH** | **XANH** — `lines=64`, `ok=1`, boot sạch |

**Về (iv) — giới hạn thật, không giấu**: QEMU TCG **không** mô hình hoá cache, nên **hiệu ứng**
của `dc civac`/`dc ivac` không quan sát được ở tầng QEMU; và tầng host chỉ kiểm **vòng lặp** (số
lệnh + địa chỉ từng dòng, `tests/cache.rs::for_each_line_addresses`) cùng **mã lệnh tuyệt đối**
(`dc_encodings`), **không** kiểm được "closure có gọi `asm::dc_civac` hay không" (asm đặc quyền
panic trên host). Chuỗi bằng chứng vì thế là: mã lệnh đúng (test tuyệt đối) → vòng lặp phát đúng
địa chỉ (test host) → vòng lặp **đã chạy trong ảnh** (`lines=64` do chính vòng lặp đếm). Mắt
"closure gọi asm" là **một dòng mã**, không kiểm được nếu không chạy trên phần cứng — **không**
được gọi là "đã kiểm trên phần cứng".

### 11.4 Còn thiếu của B3.11 (ghi rõ)

1. **KPTI** và **PAN/UAO**: **không** làm (ngoài phạm vi D2-209). `SCTLR_EL1.SPAN = 1` nên kernel
   vẫn đọc được trang user (điều kiện của U-mode KT-269).
2. **Guard chỉ "không map"**, **không** kiểm fault khi tràn stack — đúng phạm vi phiếu giao việc.
3. **Cache maintenance chỉ cho virtio-blk**: virtio-net aarch64 (KT-275) **chưa** merge; khi merge
   phải dùng **cùng** `cache::HANDOFF_SEQUENCE`/`COMPLETE_SEQUENCE`, không viết bản thứ hai.
4. **Không** tuyên bố K4' đạt — tiêu chí K4' vẫn là "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000).
