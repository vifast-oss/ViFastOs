# K4' bước 3 (phần đầu) — ViFastOs **boot thật trên AArch64** (QEMU `virt`)

Trạng thái: **B3.1–B3.5 + B3.8 + B3.9 DONE** — Agent 1 (toàn quyền A0), 2026-11-21.
**Không** tuyên bố K4' đạt: tiêu chí K4' là "**như K3**" (TAP đầy đủ smp 1|2|4 + soak 1.000) —
xem §7 "Còn thiếu".

Nguồn sự thật: code + log trong `v2/agent/JOURNAL/kt247-*`, `kt248-*`, `kt249-*`, `kt250-*`.
Quyết định: **D2-195** (mở bước 3, phạm vi), **D2-199** (Q4: nửa cao làm trong crate arch).

## 1. Phạm vi

| Bước | Nội dung | Task | Trạng thái |
|---|---|---|---|
| B3.1 | crate ảnh `vifast-kbin-aarch64` + `linker.ld` + `_start` + PL011 + PSCI halt | KT-247 | **DONE** |
| B3.2 | bảng vector 16 ô + `VBAR_EL1` + handler đồng bộ | KT-248 | **DONE** |
| B3.3 | bật MMU (TCR→MAIR→TTBR0→SCTLR→ISB) + identity map | KT-249 | **DONE** |
| B3.4 | nửa cao (`TTBR1_EL1`) + physmap | KT-253 | mở (D2-199) |
| B3.5 | `VSpace` thật | KT-254 | mở |
| B3.6 | `TLBI` + shootdown đa hart | — | chưa mở |
| B3.7 | U-mode + `eret` + `svc` | — | **chặn** (Q2: `spec/abi.toml [calling.aarch64]` chưa mở) |
| B3.8 | runner QEMU + TAP + ca âm | KT-250 | **DONE (phần đầu)** |
| B3.9 | **smp 2**: PSCI `CPU_ON` + GICv3 `SGI 0` (IPI) | KT-259 | **DONE** (2 hart; **chưa** smp 4) |

**Không** làm trong bước này: UEFI/AAVMF (thiết kế đặt **sau** K5'), EL2/ảo hoá (thuộc H1),
`cfg(target_arch)` trong `vifast-core`/`vifast-plat-common` (AGENTS §4.8).

## 2. Đường boot

```text
qemu-system-aarch64 -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 2 \
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

## 6. Runner + TAP

```bash
bash v2/test/boot/run-aarch64.sh      # build + boot + TAP + 5 ca âm
```

- Dùng `qemu_guard.sh` (process group + timeout + dọn con) — **`perl -e 'alarm N; exec @ARGV'`
  KHÔNG giết được QEMU** (qemu bỏ qua `SIGALRM`).
- **9/9 kiểm sạch** (KT-259): ELF (`.text` @ `0x4008_0000`, `.text.vectors` là TEXT, 16 stub đúng
  offset), boot (**TAP 12/12** + marker đúng 1 lần + **4 dòng `# smp:`** đúng một lần mỗi dòng),
  **7 ca âm** (marker sai, thiếu `ok`, có `not ok`, plan sai, log rỗng, **thiếu dòng hart 1
  online**, **IPI in hai lần**) đều **ĐỎ đúng**.
- Nối vào cổng: bước **56** `QEMU aarch64 (K4' bước 3)` — chạy ở **cả T1 và T2**.

## 7. Còn thiếu (ghi rõ, không giấu)

1. **smp > 2**: KT-259 mới làm **2 hart** (`_start_secondary` + PSCI `CPU_ON` + GICv3 `SGI 0`).
   **Chưa** smp 4, **chưa** timer per-hart, **chưa** preemption, **chưa** TLBI shootdown đa hart
   (B3.6) — nên `unmap` hiện chỉ làm mới TLB **cục bộ**.
2. **Soak 1.000** boot: chưa chạy (thuộc "như K3").
3. **U-mode** (B3.7): chặn bởi Q2 — `spec/abi.toml [calling.aarch64]` **chưa mở**.
4. **Nửa cao/physmap** (B3.4) và **`VSpace`** (B3.5): đang mở (KT-253/KT-254, D2-199).
5. **DTB**: `_start` nhận con trỏ trong `x0` nhưng **chưa đọc**; địa chỉ thiết bị đang là hằng của
   `resources.rs` (memory map QEMU `virt`).
6. **W^X chưa đầy đủ**: vùng ảnh map `KERNEL_RWX` một khối; tách `.text` RX / `.data` RW thuộc B3.5.
7. **`budget.sh` bản aarch64**: chưa có; `.data+.bss` đo bằng `llvm-objdump`.
8. **16 ô vector dùng chung một stub**: IRQ/FIQ chưa bật (`DAIF` mask đủ); khi bật ngắt phải tách
   handler theo ô.
