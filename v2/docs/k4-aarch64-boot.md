# K4' bước 3 (phần đầu) — ViFastOs **boot thật trên AArch64** (QEMU `virt`)

Trạng thái: **B3.1–B3.9 + B3.7 (U-mode) DONE** — Agent 1 (toàn quyền A0), 2026-11-21.
**Soak 1.000 + `budget-aarch64.sh` DONE** (KT-265, §8) — điều kiện "như K3" đã đủ **phần soak**.
**U-mode/EL0 + `svc` DONE** (KT-269, §9) — `[calling.aarch64]` đã mở (D2-206).
**Không** tuyên bố K4' đạt: tiêu chí K4' là "**như K3**" (TAP đầy đủ smp 1|2|4 + soak 1.000) —
xem §7 "Còn thiếu".

Nguồn sự thật: code + log trong `v2/agent/JOURNAL/kt247-*`, `kt248-*`, `kt249-*`, `kt250-*`,
`kt253-*`, `kt254-*`, `kt259-*`, `kt262-*`, `kt265-*`, `kt269-*`.
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
bash v2/test/boot/run-aarch64.sh      # build + boot + TAP + 10 ca âm
```

- Dùng `qemu_guard.sh` (process group + timeout + dọn con) — **`perl -e 'alarm N; exec @ARGV'`
  KHÔNG giết được QEMU** (qemu bỏ qua `SIGALRM`).
- **13/13 kiểm sạch** (KT-262 + KT-265 + KT-269): ELF (`.text` @ `0x4008_0000`, `.text.vectors`
  là TEXT, 16 stub đúng offset), boot (**TAP 18/18** + marker đúng 1 lần + **8 dòng `# smp:`** +
  **3 dòng `# tlbi:`** + **`# umode: el0 ran svc=0 ret=<len>`** + **`# conf image:` khớp ELF** —
  đúng một lần mỗi dòng), **11 ca âm** (marker sai, thiếu `ok`, có `not ok`, plan sai, log rỗng,
  thiếu dòng hart 1 online, IPI in hai lần, **thiếu dòng `# tlbi:`**, **thiếu ack shootdown**,
  **hash ảnh lệch ELF**, **thiếu dòng `# umode:`**) đều **ĐỎ đúng**.
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
6. **W^X chưa đầy đủ**: vùng ảnh map `KERNEL_RWX` một khối; tách `.text` RX / `.data` RW thuộc B3.5.
7. **`budget.sh` bản aarch64**: **DONE** (KT-265) — `v2/test/boot/budget-aarch64.sh`, trần
   262.144 B, `.smp_stacks` báo riêng (§8).
8. **16 ô vector dùng chung một stub**: IRQ/FIQ chưa bật (`DAIF` mask đủ); khi bật ngắt phải tách
   handler theo ô.
9. **Shootdown theo dải VA**: chỉ một VA mỗi lần (`vae1is`); `vmalle1is`/`aside1is` có hằng
   encoding nhưng chưa dùng. **Không lock**: một shootdown tại một thời điểm (chưa preemption).
10. **`TLBI` trên QEMU TCG**: `vae1is` flush TLB **mọi** CPU (`tlb_flush_page_by_mmuidx_all_cpus_synced`),
    nên đột biến "bỏ `tlbi` ở hart phụ" **không** đỏ trên QEMU — xem `kt262-mutation.log` §5 và
    đối chứng dương ở §5.1. **Không** được gọi là "đã kiểm trên phần cứng".
11. **`# build:` (KT-86) chưa có trên ảnh aarch64**: cổng danh tính bản dựng của riscv64 chưa áp
    cho aarch64; bù bằng `# conf image:` (D2-80) + sha256 + `git HEAD` trong header soak. **Không**
    gọi là tương đương KT-86.

## 8. Soak 1.000 + ngân sách aarch64 (KT-265)

```bash
bash v2/test/boot/soak-aarch64.sh 1000 3     # cổng: N=1.000, 3 luồng, 20 s/boot, không retry
bash v2/test/boot/soak-aarch64.sh --smoke 3 3 # N < 1.000: LUÔN exit 1 (không phải bằng chứng)
bash v2/test/boot/soak-aarch64.sh --selftest  # 1 ca dương + 13 ca âm, không mở QEMU
bash v2/test/boot/budget-aarch64.sh <ảnh> product
bash v2/test/boot/budget-aarch64.sh --selftest
```

**Bất biến mỗi boot** (cùng khuôn `soak.sh` riscv64, **không nới**): đúng 1 `TAP version 13`,
đúng 1 `1..15`, đúng 15 dòng `ok` (đủ 1..15), 0 `not ok`, đúng 1 marker
`# viFastOs aarch64: boot ok`, đúng 1 `# conf image:` **khớp ELF**, `# smp: online mask=0xf …
harts=4 mpidr_ok=1` + bằng chứng từng hart + ba dòng `# tlbi:`, 0 `PANIC`, quá 20 s ⇒ đỏ.

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
