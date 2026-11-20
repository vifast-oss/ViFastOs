# KT-259 — K4' B3.9: **hai hart** trên aarch64 (PSCI `CPU_ON` + GICv3 `SGI 0`)

Trạng thái: **DONE** (nhánh `task/kt259-a64smp`, base `main` @ `4478662a`), 2026-11-21.
Sub-agent `a64smp`. Quyết định: **D2-202** (nhiệm vụ), **D2-195** (phạm vi bước 3), **D2-199**
(nửa cao trong crate arch), **D2-198** (uỷ quyền commit trên nhánh task).

**Không** tuyên bố K4' đạt: tiêu chí K4' là "**như K3**" (TAP đầy đủ smp 1|2|4 + soak 1.000).
KT-259 chỉ làm **smp 2** + IPI; **không** smp 4, **không** timer per-hart, **không** preemption,
**không** U-mode.

## 1. Phạm vi đã làm

| Việc | Kết quả |
|---|---|
| `_start_secondary` (asm) | **DONE** — `MPIDR_EL1` → hart id, stack per-hart, `CPACR_EL1`, gọi `kmain_secondary` |
| PSCI `CPU_ON` (`hvc #0`, `0xC400_0003`) | **DONE** — hart 1 online, `rc=0` |
| MMU cho hart phụ | **DONE** — nạp lại **đúng** `TCR/MAIR/TTBR0/TTBR1/SCTLR` của hart 0, dùng lại `MMU_ENABLE_SEQUENCE` |
| GICv3 (distributor + redistributor + CPU interface) | **DONE** — `ARE_NS`, `GICR_WAKER`, `GICR_ISENABLER0`, `ICC_SRE/PMR/IGRPEN1` |
| IPI `SGI 0` hart 0 → hart 1 | **DONE** — `ICC_SGI1R_EL1`, nhận ở ô vector **IRQ EL1h** (slot 5), `ICC_IAR1/EOIR1` |
| TAP mới | **DONE** — 3 ca (10/11/12), tổng **12** |
| Runner | **DONE** — `-smp 2`, `-M virt,gic-version=3`, `EXPECT_TESTS=12`, 5 ca âm cũ + **2 ca âm mới** |

## 2. Số đo QEMU (đo trước khi viết code — không đoán)

| Câu hỏi | Cách đo | Kết quả |
|---|---|---|
| QEMU bản nào? | `qemu-system-aarch64 --version` | **11.1.3** |
| `virt` mặc định GICv2 hay v3? | `-M virt,dumpdtb` + `dtc` | **GICv2** (`arm,cortex-a15-gic`) ⇒ **phải** `-M virt,gic-version=3` |
| GICv3 nằm ở đâu? | `-M virt,gic-version=3,dumpdtb` + `dtc` | `intc@8000000` `reg = <0 0x08000000 0 0x10000 0 0x080a0000 0 0xf60000>` ⇒ GICD `0x0800_0000`, GICR `0x080A_0000`, mỗi hart `0x2_0000` |
| PSCI conduit? | DTB `/psci` | `method = "hvc"`, `cpu_on = 0xC4000003` |
| Hart id trong DTB? | `/cpus/cpu@N/reg` | `0`, `1` ⇒ `target_cpu = 1` cho hart 1 |
| `MPIDR_EL1` hart 1? | ảnh in `# smp: hart=1 mpidr=…` | `0x0000_0000_8000_0001` (`Aff0 = 1`, `MT = 1`) |
| `GICR_WAKER` lúc reset? | ảnh in `waker=0x…` | **`0x0000_0000`** — QEMU để redistributor **thức sẵn** (xem §6 đột biến i) |

## 3. Thiết kế

### 3.1 Arena stack per-hart (`linker.ld` + `vifast_plat_aarch64::smp`)

```text
__smp_stacks_start
  slot 0 (hart 0): guard 4 KiB | stack 64 KiB     ← __stack_bottom/__stack_top trỏ vào đây
  slot 1 (hart 1): guard 4 KiB | stack 64 KiB
  …
  slot 7 (hart 7): guard 4 KiB | stack 64 KiB
__smp_stacks_end                                  ← 8 × 0x1_1000 = 0x8_8000 (544 KiB, NOLOAD)
```

- **Hart 0 cũng dùng slot 0**: `linker.ld` đặt `__stack_top = __smp_stacks_start + 0x1_1000` nên
  `_start` và bước nhảy nửa cao của KT-253 **không đổi một dòng nào**; mọi hart có **cùng một
  quy tắc** địa chỉ (không nhánh riêng cho hart 0).
- Guard nằm **ngay dưới** đáy stack của **chính hart đó** ⇒ tràn stack chạm guard của mình.
- `smp::check_layout` là phép kiểm **thuần số học** (record rời nhau, căn trang, không tràn);
  ảnh gọi nó lúc chạy và so `__smp_stacks_end - __smp_stacks_start == HART_AREA_BYTES` — lệch
  linker/crate là **PANIC**, không đoán.
- NOLOAD **không** cộng vào trần `.data + .bss ≤ 256 KiB` (I7); `__kernel_end` lớn thêm 544 KiB
  nhưng vẫn trong **cùng** một bảng mức 3 (2 MiB) nên **không** tốn frame trang.

### 3.2 `_start_secondary` (asm, `main.rs`)

```text
mrs x19, mpidr_el1 ; and x19, x19, #0xff      ← hart id THẬT (không tin x0/context_id)
adrp/add x9, __smp_stacks_start
movz/movk x10, HART_STRIDE
madd x9, x19, x10, x9 ; add x9, x9, x10 ; mov sp, x9   ← đỉnh slot của hart này
msr cpacr_el1, … ; isb                        ← FP/SIMD trước mọi mã Rust (bài học KT-254)
mov x1, x0 ; mov x0, x19 ; bl kmain_secondary ← x0 = hart (MPIDR), x1 = context_id (PSCI)
```

Vì PSCI đánh thức hart phụ với **MMU tắt**, mọi địa chỉ trong đoạn này là **PA** — và ảnh liên
kết tại PA (`0x4008_0000`) nên `adrp` cho đúng PA. `SMP_ENTRY_PA` được đọc **trước** khi hart 0
nhảy nửa cao (sau đó `adrp` cho alias, không phải PA).

### 3.3 `kmain_secondary` (Rust)

1. `MPIDR` → `hart_index` (từ chối topology không phẳng), `ctx == hart`, và **`sp` thuộc slot của
   chính mình** — đây là chỗ đột biến (iii) lộ ra;
2. nạp `TCR/MAIR/TTBR0/TTBR1/SCTLR` đã công bố, theo đúng `MMU_ENABLE_SEQUENCE` (**một** định
   nghĩa chuỗi cho cả hai hart), kiểm `SCTLR_EL1.M = 1`;
3. `VBAR_EL1` = VA **thấp** của bảng vector (hart 1 chạy VA thấp);
4. GICv3 **của chính nó**: `wake_redistributor` (kiểm `GICR_TYPER.Affinity_Value` khớp hart) →
   `enable_sgi` → `init_cpu_interface`;
5. in `# smp: hart=1 online` + dòng bằng chứng, đặt `SMP_ONLINE`, `dsb sy`/`sev`, rồi **mở** IRQ;
6. chờ IPI (handler IRQ đếm), chờ yêu cầu báo alive, in `# smp: hart=1 alive`;
7. `wfi` vô hạn (hart 1 **không** được tắt máy — chỉ hart 0 `SYSTEM_OFF`).

### 3.4 GICv3 + IPI

```text
hart 0: GICD_CTLR ← ARE_NS | EnableGrp1NS (chờ RWP=0)
        GICR_WAKER(0) ← clear ProcessorSleep (chờ ChildrenAsleep=0)
        GICR_IGROUPR0/ISENABLER0 bit 0 (SGI 0), ICC_SRE=1, ICC_PMR=0xF0, ICC_IGRPEN1=1
hart 1: (như trên, cho redistributor của chính nó) + DAIF.I = 0
IPI   : hart 0 ghi ICC_SGI1R_EL1 = sgi1r(0, TargetList=1<<Aff0(1)=0b10, Aff1..3=0, IRM=0)
        hart 1 nhận ở ô vector 5 (IRQ EL1h): ICC_IAR1_EL1 → in/đếm → ICC_EOIR1_EL1
```

`ICC_SGI1R_EL1` được test host khẳng định **tuyệt đối** (`0x2` cho hart 1, `0x4003_0302_0001`
cho tổ hợp Aff1..3 khác 0) — không lấy hằng của crate để so.

### 3.5 Bàn giao giữa hai hart (không lock, không `unsafe`)

`AtomicU64` + `Release`/`Acquire` + `dsb sy`/`sev`. **Quy tắc in: in trước, đặt cờ sau** — lần
chạy đầu của KT-259 đặt cờ trước khi in và hai hart in **xen ký tự** vào cùng PL011:

```text
# smp: # smp: ipiipi  send=recv=1 intiOkd=0
```

Runner so **nguyên văn** từng dòng `# smp:` nên lỗi ấy làm boot đỏ — đúng như phải. Đã sửa bằng
cách đảo thứ tự trong handler IRQ (`in` → `store`).

## 4. Đo được

### 4.1 TAP (ảnh, `v2/agent/JOURNAL/kt259-boot.log`)

```text
ok 10 - smp: hart 1 online
ok 11 - smp: ipi 0->1
ok 12 - smp: both harts report
```

Dòng bằng chứng (nguyên văn):

```text
# smp: arena pa=0x400b3000 alias=0xffffffc080033000 bytes=557056 expect=557056 layout=Ok(()) ok=1
# gic: dist ctlr=0x00000052 typer=0x037a0008 lines=288 are_ns=1 grp1ns=1
# gic: rd hart=0 typer=0x0000000001000001 aff=0x0 waker=0x00000000 asleep=0 sre=0x7 sgi=0
# smp: boot regs tcr=0x00000015b5193519 mair=0x000000000000ff00 ttbr0=0x10000400a8000 ttbr1=0x400a3000 sctlr=0x0000000000c50839
# smp: cpu_on begin hart=1 entry=0x40080044 ctx=1 fn=0xc4000003
# smp: hart=1 online
# smp: hart=1 mpidr=0x0000000080000001 ctx=1 sp=0x400d4df0 slot=1 stack=0x400c5000-0x400d5000 vbar=0x40088000 sctlr=0x0000000000c50839 gic_typer=0x0000000101000111 aff=0x1 waker=0x00000000 asleep=0 sre=0x7
# smp: cpu_on rc=0 (success) online=1 waited=167469 hart1_mpidr=0x0000000080000001 ctx=1 sp=0x400d4df0 slot1=Some(1) hart0_sp_pa=0x400c3d30 slot0=Some(0)
# smp: ipi recv=1 intid=0
# smp: ipi send=Ok(()) count=1 waited=14270 sgi=0 target=1
# smp: hart=1 alive
# smp: hart=0 alive
# smp: report hart0=1 hart1=1 waited=9432 online=1 ipi=1
```

Đọc được từ log: hart 1 tự đọc `MPIDR = 0x8000_0001` (không phải `x3`), `sp` của nó nằm trong
**slot 1** (`0x400c5000-0x400d5000`), `sp` hart 0 quy về PA nằm trong **slot 0** — hai hart
**khác slot**; `GICR_TYPER.Affinity_Value` của redistributor hart 1 = `0x1` (đúng hart);
`ICC_SRE_EL1 = 0x7` (SRE=1); IPI tới **một** lần với `intid=0`.

### 4.2 Runner (`bash v2/test/boot/run-aarch64.sh`)

```text
ok 1 - elf: .text tại 0x40080000, .text.vectors TEXT (không BSS), 16 stub đúng offset
ok 2 - boot 1: TAP 12/12 + marker 1 lần
ok 3..9 - 7 ca âm (5 ca cũ + 2 ca mới cho dòng `# smp:`)
# ca âm: 7/7 ĐỎ đúng như phải
# 9/9 kiểm sạch (boot=1, tests=12)
```

Hai ca âm **mới** (bắt buộc, chạy mặc định): (f) thiếu `# smp: hart=1 online` ⇒ ĐỎ;
(g) `# smp: ipi recv=1` xuất hiện **hai** lần ⇒ ĐỎ. Nhờ vậy "ảnh chỉ chạy hart 0 mà vẫn in
`ok 10`" không thể PASS.

### 4.3 Test host

`cargo test -p vifast-plat-aarch64`: **136** ca (trước KT-259: 112) — **+24**:
`tests/smp.rs` **13** (PSCI id/mã trả, MPIDR decode, arena/stride/offset/slot, overlap, căn trang,
`MmuBootRegs`) + `tests/gic.rs` **11** (offset GICD/GICR, bit `CTLR`/`WAKER`/`SRE`, base
redistributor, encoding `ICC_SGI1R_EL1`, dải spurious).

## 5. Đột biến (log: `kt259-mutation.log`)

| # | Đột biến | Tầng host | Tầng QEMU |
|---|---|---|---|
| (i) | bỏ clear `GICR_WAKER.ProcessorSleep` của hart 1 | — | **KHÔNG quan sát được** — `waker=0x6`/`asleep=1` mà SGI **vẫn** tới (`ok 11`) |
| (i') | bỏ `GICR_ISENABLER0` bit 0 của hart 1 (thay thế, cùng chuỗi init) | — | **ĐỎ** — `not ok 11`, `# smp: hart=1 ipi timeout spins=50000000`, runner 8/9 |
| (ii) | `CPU_ON` với `entry + 4` (bỏ `mrs mpidr_el1`) | — | **ĐỎ** — `PANIC: smp: hart từ MPIDR=1 nhưng asm truyền 0`; **treo có kiểm soát** (PANIC in rồi dừng), runner 8/9 |
| (iii) | `HART_STRIDE = HART_STACK_BYTES` (bỏ guard) | **ĐỎ** — 5 ca (`stack_geometry`, `stack_offsets_per_hart`, `stacks_are_disjoint`, `stack_slot_of_address`, `stack_overlap_without_guard_is_rejected`) | **ĐỎ** — `layout=Err(Overlap { a: 0, b: 1 })`, `PANIC: smp: bố cục arena stack không khớp linker` |

**Ghi trung thực**: đột biến (i) **đúng như đề bài** (bỏ `GICR_WAKER` clear) **không** làm đỏ
trên QEMU 11.1.3 — QEMU không chặn chuyển SGI theo `ProcessorSleep` (đo: `waker=0x6`,
`asleep=1`, IPI vẫn nhận). Vì vậy tôi thêm **(i')** — cùng chuỗi init, cùng mục đích "hart 1 không
nhận IPI" — và nó **đỏ thật**. Không được gọi (i) là "đã kiểm trên phần cứng".

## 6. Còn thiếu (ghi rõ, không giấu)

1. **smp 4**: chưa. Arena đã đủ 8 slot và `CPU_ON` đã tổng quát theo hart id, nhưng TAP/runner
   mới đo 2 hart; smp 4 cần thêm ca và (quan trọng hơn) **TLBI shootdown** (B3.6) mới đúng nghĩa.
2. **Timer per-hart / preemption**: chưa — chưa có `CNTP`/`CNTV` per-hart, chưa có scheduler.
3. **TLBI shootdown đa hart** (B3.6): chưa. Hiện `unmap` chỉ `tlbi vae1` **cục bộ**; nếu hart 1
   từng đọc VA ấy thì TLB của nó **không** được làm mới — đây là **nợ thật**, không phải "đã xong".
4. **Hai address space**: hart 1 chạy **VA thấp**, hart 0 chạy **VA nửa cao**, nhưng **cùng** bảng
   trang `VSpace`. Chưa có "mỗi hart một `VSpace`/ASID".
5. **U-mode** (B3.7): vẫn chặn bởi Q2 (`spec/abi.toml [calling.aarch64]` chưa mở).
6. **`GICR_WAKER` không cưỡng chế được trên QEMU**: bước wake vẫn giữ (đúng đặc tả, cần cho phần
   cứng thật) nhưng **không** có bằng chứng QEMU cho nó — đã ghi ở §5.
7. **`-smp 2` + `gic-version=3` là thay đổi môi trường boot**: mọi số đo cũ của KT-247..KT-254
   (một hart, GICv2) **không** còn so sánh trực tiếp được; các ca 1..9 vẫn xanh trên môi trường mới.

## 7. Rủi ro

- **Chờ có trần** (`SMP_WAIT_LIMIT = 50e6` vòng): nếu QEMU chậm hơn nhiều (máy bận, TCG nặng),
  hart 1 có thể vượt trần ⇒ `not ok 10` **giả**. Đo được: ~1,7×10^5 vòng (online), ~1,4×10^4 (IPI),
  ~9,4×10^3 (alive) — trần rộng hơn **hai bậc độ lớn**, nhưng đây là ngưỡng **thời gian**, không
  phải ngưỡng đúng/sai.
- **In xen dòng**: chỉ tránh được nhờ quy tắc "in trước, đặt cờ sau". Mọi dòng in mới của hart 1
  **phải** theo quy tắc ấy; không có lock PL011.
- **`GICR` map 1 MiB** trong `VSpace` (256 trang) — cùng bảng mức 3 với GICD nên không tốn frame,
  nhưng nếu sau này map dải khác vắt qua ranh giới 2 MiB thì sẽ cần frame mới (arena 32 frame,
  đang dùng 14).

## 8. Tệp

| Tệp | Việc |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/smp.rs` | **mới** — PSCI/MPIDR/arena stack/`MmuBootRegs` (dữ liệu, host-testable) |
| `v2/kernel/vifast-plat-aarch64/src/gic.rs` | **mới** — GICv3: offset/bit/encoding + init MMIO |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | +13 hàm asm (`mpidr`, `psci_cpu_on`, `ICC_*`, `daifclr`, `sev`, `sp`) + stub host |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | +`pub mod gic; pub mod smp;` |
| `v2/kernel/vifast-plat-aarch64/tests/smp.rs` | **mới** — 13 ca host |
| `v2/kernel/vifast-plat-aarch64/tests/gic.rs` | **mới** — 11 ca host |
| `v2/kernel/vifast-kbin-aarch64/linker.ld` | `.stack`/`.stack_guard` → `.smp_stacks` (8 record, NOLOAD) |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | `_start_secondary`, IRQ slot 5, map `GICR`, gọi `smpboot` |
| `v2/kernel/vifast-kbin-aarch64/src/smpboot.rs` | **mới** — trạng thái SMP + `smp_bringup` + `kmain_secondary` (tách vì luật R1) |
| `v2/test/boot/run-aarch64.sh` | `-smp 2`, `-M virt,gic-version=3`, `EXPECT_TESTS=12`, `check_smp`, 2 ca âm mới |
| `v2/docs/k4-aarch64-boot.md` | §1/§7 cập nhật B3.9 |
