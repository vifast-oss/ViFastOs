# KT-242 — K4' **bước 3**: thiết kế (design-only)

**Ngày**: 2026-11-20 · **Agent 2 (luồng L1/kế hoạch)** · nhánh `task/kt242-l1` · HEAD trước task: `21cc75cd`
**Hàng QUEUE**: `KT-242` (phần "chuẩn bị K4' bước 3") · **Trạng thái**: **CHƯA MỞ** triển khai
**Phạm vi**: **chỉ thiết kế**. Không viết mã kernel, không sửa `spec/`, không sửa
`docs/v2/ViFastOs-v2-thiet-ke.md`, không tuyên bố K4' đạt.

> **Đọc kèm**: `v2/agent/JOURNAL/kt160-aarch64.md` (bước 1), `kt164-shared.md` (bước 1b),
> `kt167-a64step2.md` §5 (danh sách còn thiếu — nguồn gốc của tài liệu này), `kt175-a64review.md`
> (review đối kháng bước 2), `v2/agent/reports/K5-X86-READINESS.md` (khuôn "audit sẵn sàng").

---

## 0. Bước 3 là gì — và **không** là gì

`kt167-a64step2.md` §5 ghi đúng bốn nợ còn lại của K4':

1. **Chưa boot**: không `_start`, không linker script, không vector table, không `eret`.
2. **Chưa nối thanh ghi vào đường boot**: chưa gọi `write_tcr_el1`/`write_mair_el1`/
   `write_ttbr0_el1`/`write_sctlr_el1` theo trình tự chuẩn (TCR → MAIR → TTBR → SCTLR → ISB).
3. **Chưa dựng `VSpace` thật**: `PageTable` (bước 1) và `Ttbr` (bước 2) chưa được nối; chưa có
   `TLBI` sau `unmap`.
4. **Chưa kiểm trên QEMU/máy thật**.

Bước 3 **là** bốn việc trên, cộng phần tối thiểu để chúng **đo được**: một crate ảnh
`vifast-kbin-aarch64`, một harness QEMU aarch64, và một bộ TAP chạy trên ảnh đó.

Bước 3 **không là**:

- **Không** phải "K4' đạt". Tiêu chí K4' theo thiết kế là "**như K3**"
  (`docs/v2/ViFastOs-v2-thiet-ke.md:880`): TAP đầy đủ trên smp 1|2|4, soak boot **0 lỗi / N =
  1.000** trên máy `ps`, cộng các mốc phụ thuộc. Bước 3 chỉ là **đường boot + MMU + VSpace**;
  soak 1.000 boot là bước sau.
- **Không** mở UEFI/AAVMF. Thiết kế đặt UEFI **sau** K5' (`docs/v2/ViFastOs-v2-thiet-ke.md:882`);
  bước 3 dùng `-kernel` trực tiếp, đúng khuôn riscv64 hiện tại (`v2/test/boot/run.sh:103-104`).
- **Không** mở VT-x/EL2 (ảo hoá) — thuộc H1, không thuộc K4'.
- **Không** đụng `vifast-core` bằng `cfg(target_arch)` (AGENTS §4.8; D2-147 cấm tường minh).

---

## 1. Điều kiện vào (entry conditions)

| # | Điều kiện | Hiện trạng | Ai chốt |
|---|---|---|---|
| E1 | Bước 1 + 1b + 2 **DONE** | **ĐẠT** — KT-160/164/167 DONE, KT-175 review ĐẠT CÓ ĐIỀU KIỆN, KT-185 vá C1/C2/C4/C5 | đã có |
| E2 | `PageTableFormat`/`Mmu` ở crate **dùng chung** | **ĐẠT** — `v2/lib/vifast-pte/src/mmu.rs`, ba impl (Sv39, AArch64, x86_64) | đã có |
| E3 | Toolchain có target `aarch64-unknown-none` | **ĐẠT** — `v2/rust-toolchain.toml:11-12` | đã có |
| E4 | `qemu-system-aarch64` trên máy đo | **ĐẠT** — `/opt/homebrew/bin/qemu-system-aarch64` (kiểm 2026-11-20) | đã có |
| E5 | **A0 mở bước 3** kèm ngân sách + phạm vi | **CHƯA** — `STATE.md:48` ghi "CHƯA MỞ: K4' bước 3" | **A0** |
| E6 | Chốt **đường boot** = `-kernel` trực tiếp (không AAVMF/UEFI) | **CHƯA thành văn** — suy ra từ `docs/v2/ViFastOs-v2-thiet-ke.md:882` + D2-10, nhưng chưa có dòng D2 nào cho aarch64 | **A0** (xem §6 Q1) |
| E7 | `spec/abi.toml` `[calling.aarch64]` mở | **CHƯA** — `spec/abi.toml:62-64` còn `reserved = true` | **A0** (xem §6 Q2) — **chỉ chặn B3.7** |
| E8 | `spec/kernel.toml` (`KernelConfig`) tồn tại | **CHƯA** — `spec/` chỉ có `abi.toml` + `v1-parity.toml` | **A0** (xem §6 Q3) — chặn "như K3", **không** chặn bước 3 |
| E9 | Không chạy song song với cổng/soak khác | quy tắc vận hành | lead |

**Kết luận E**: bước 3 **chưa thể mở** — thiếu E5 (duyệt) và E6 (chốt đường boot). E7/E8 chặn
**phần sau** của bước 3, không chặn B3.1–B3.6.

---

## 2. Phần việc (work items)

Tám bước, mỗi bước **≤ 3 h** (AGENTS §10.4), có tiêu chí **đo được** và **ngân sách** riêng.
Thứ tự là thứ tự **phụ thuộc**, không phải thứ tự ưu tiên.

### B3.1 — Crate ảnh `vifast-kbin-aarch64` + linker script + `_start` (3 h)

- **Việc**: crate mới `v2/kernel/vifast-kbin-aarch64` theo khuôn `vifast-kbin-riscv64`
  (`v2/kernel/vifast-kbin-riscv64/linker.ld`, `build.rs`, `src/`); `linker.ld` với
  `OUTPUT_ARCH(aarch64)`, `ENTRY(_start)`, nạp tại `0x4008_0000` (QEMU `virt` `-kernel`);
  `_start` đặt `sp` vào `.stack` (NOLOAD, có trang bảo vệ như riscv64), gọi `kmain`, dừng.
- **Cần thêm**: driver **PL011 UART** tối thiểu (`write_byte`) — chưa có trong `v2/`; đặt ở
  `vifast-plat-aarch64` sau `#[cfg(target_arch = "aarch64")]` + stub host (khuôn
  `vifast-plat-riscv64/src/uart_console.rs`).
- **Đo được**: `cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-aarch64 --target
  aarch64-unknown-none` xanh; `llvm-objdump -h` cho `.text` bắt đầu đúng `0x4008_0000`;
  `qemu-system-aarch64 -M virt -cpu max -kernel <elf> -nographic` in đúng **một** dòng nhận
  dạng rồi dừng sạch (không panic, không treo).
- **Không làm**: MMU, vector table, U-mode.

### B3.2 — Vector table + `VBAR_EL1` + handler đồng bộ (2,5 h)

- **Việc**: bảng 16 vector × `0x80` (`VBAR_EL1` đòi căn `0x800`), mỗi ô là một stub asm đẩy
  `ESR_EL1`/`FAR_EL1`/`ELR_EL1`/`SPSR_EL1` vào một khung rồi gọi handler Rust; handler in
  nguyên nhân rồi dừng. Dùng `Vbar`/`Esr`/`FarEl1`/`ElrEl1`/`Spsr` của bước 2.
- **Đo được**: (i) test host khẳng định **offset tuyệt đối** từng ô (`0x000`, `0x080`, …,
  `0x780`) và cờ `VBAR_BASE_MASK`; (ii) trên QEMU, một `brk #0` cố ý in đúng `EC` của
  `BRK` và `ELR` trỏ đúng chỗ.
- **Bất biến**: bảng vector **không** được nằm trong `.bss` (phải ở `.text`/`.rodata` đã nạp).

### B3.3 — Bật MMU: chuỗi TCR → MAIR → TTBR0 → SCTLR → ISB, identity map (3 h)

- **Việc**: dựng `PageTable<Aarch64, TableMemory>` (bước 1) cho **identity map** vùng kernel +
  UART + GIC; nạp `TCR_EL1_KERNEL`/`MAIR_EL1_KERNEL` (bước 2), `TTBR0_EL1`, rồi
  `SCTLR_EL1.M = 1`, `ISB`. Thứ tự **bắt buộc** đúng như `kt167-a64step2.md` §5 ghi.
- **Đo được**: sau khi bật, đọc lại một marker qua VA **không** đổi giá trị; `AT S1E1R` (hoặc
  `translate` của walker so với phần cứng) cho cùng PA; tắt MMU **không** cần nữa.
- **Đột biến bắt buộc**: (i) bỏ `ISB` sau `SCTLR` ⇒ phải đỏ (hoặc treo có kiểm soát — ghi rõ
  trong log); (ii) đảo `TCR.T0SZ` 25→24 ⇒ đỏ; (iii) `MAIR` slot 1 sai ⇒ đỏ.

### B3.4 — Nửa cao (`TTBR1_EL1`) + physmap (3 h)

- **Việc**: kernel chạy ở nửa cao qua `TTBR1_EL1`, `TTBR0_EL1` để trống cho user; cửa sổ
  physmap đọc/ghi PA bất kỳ. Đây là chỗ **trait `PageTableFormat` hiện chưa đủ**: thiếu thao
  tác copy nửa cao/physmap (chính `K5-X86-READINESS.md` §2.1 đã chỉ ra cho x86; aarch64 vướng
  **cùng** lỗ hổng).
- **Đo được**: kernel in được địa chỉ VA nửa cao của `_start`; đọc một PA đã biết qua physmap
  cho đúng giá trị đã ghi trước đó bằng identity map.
- **Quyết định kỹ thuật cần chốt** (xem §6 Q4): thêm thao tác vào `PageTableFormat` (ảnh hưởng
  **cả ba** impl + thân test generic) **hay** làm trong crate arch như riscv64 đang làm
  (`v2/kernel/vifast-plat-riscv64/src/mmu.rs:279`).

### B3.5 — `VSpace` thật cho aarch64 (3 h)

- **Việc**: nối `PageTable` + `Ttbr` thành `VSpace` dùng được: `vspace_create`/`vspace_map_obj`/
  `vspace_unmap`/`vspace_walk`/`vspace_activate`. Vướng đã biết: object layer
  `v2/kernel/vifast-plat-common/src/obj/vspace.rs` **gắn Sv39** (import `walk_shape`,
  `PTE_LEVELS`) — cùng nợ M17 của audit K5'.
- **Đo được**: map một Frame 4 KiB vào VSpace, `walk` trả đúng PA; `unmap` rồi `walk` trả lỗi;
  đọc qua VA sau `unmap` sinh fault **đúng** `FaultKind::Vm`.
- **Phụ thuộc**: B3.3, B3.4, và quyết định §6 Q4.

### B3.6 — `TLBI` sau `unmap` + shootdown đa hart (2,5 h)

- **Việc**: gọi `tlb::vae1` (bước 2) sau mỗi `unmap`, kèm `dsb ish` + `isb`; shootdown đa hart
  qua SGI (GICv3) theo giao thức `TlbFlushRequest` đã có ở
  `v2/kernel/vifast-plat-common/src/tlb.rs:314`.
- **Đo được**: nạp entry TLB trên hart 0 và hart 1, `unmap` trên hart 0, rồi hart 1 đọc VA cũ
  ⇒ **fault** (không dùng entry cũ). Đây là bất biến "TLB-safety" mà sim đã kiểm 100k seed
  (`tlb_violations() == 0`) — nay kiểm trên phần cứng thật.
- **Đột biến bắt buộc**: bỏ `vae1` ⇒ phải đỏ (hart 1 vẫn đọc được giá trị cũ).

### B3.7 — U-mode + `eret` + `svc` (**BLOCKED** — cần E7)

- **Việc**: `SPSR_EL1`/`ELR_EL1` + `eret` xuống EL0; handler `svc`; quy ước gọi
  `[calling.aarch64]`.
- **Chặn**: `spec/abi.toml:62-64` còn `reserved = true`. **Không** được tự đặt quy ước rồi
  coi là ABI (D2-04, D2-22). Cần A0 mở `spec/` theo khuôn KT-05 (một lần, chỉ chép
  `[calling.aarch64]`).
- **Đo được (khi mở)**: một tác vụ EL0 gọi `DebugPutStr` rồi `Exit(0)`; trace khớp sim.

### B3.8 — Harness QEMU aarch64 + TAP + conformance (3 h)

- **Việc**: `v2/test/boot/run-aarch64.sh` theo khuôn `run.sh` (đang chỉ riscv64:
  `v2/test/boot/run.sh:44`, `:103-104`); TAP trên aarch64; so trace sim ↔ QEMU aarch64 bằng
  `vifast-traceformat` (đã có, dùng chung với kernel từ KT-55).
- **Đo được**: TAP **N/N** trên smp 1|2|4; `trace-compare` khớp **từng dòng**; soak boot ngắn
  (≥ 50 boot, 0 lỗi) — **soak 1.000 là bước sau**, không phải bước 3.
- **Không làm**: `-cpu max` (đó là K5'), KPTI, VT-x.

---

## 3. Bất biến (invariants) — không được vi phạm trong **mọi** bước

| # | Bất biến | Nguồn | Cách kiểm |
|---|---|---|---|
| I1 | **Không** `cfg(target_arch)` trong `vifast-core`/runtime | AGENTS §4.8, D2-147 | `grep -rn "cfg(target_arch)"` trên `vifast-core`/`vifast-plat-common`/`vifast-pte` phải **rỗng** |
| I2 | `unsafe` **chỉ** trong `vifast-plat-aarch64::asm` (và crate arch mới) | D2-17 | `#![deny(unsafe_code)]` ở gốc, `#![allow(unsafe_code)]` **chỉ** module asm |
| I3 | Đường riscv64 **không đổi**: `gate --fast` xanh, TAP không đổi số | AGENTS §7 | `./v2/bin/v2 gate --fast` trước/sau mỗi bước |
| I4 | `PageTableFormat`/`Mmu` **không** đổi chữ ký mà không sửa **cả ba** impl + thân test generic | KT-164 | `cargo test -p vifast-plat-common --test mmu_shared` |
| I5 | Trace **giống nhau** giữa sim và mọi arch (cùng `.scn` ⇒ cùng dòng) | thiết kế §9 | `trace-compare` |
| I6 | Không thêm crate ngoài allowlist giấy phép | D2-21, QD §9.3 | `v2 check` |
| I7 | `.data + .bss ≤ 256 KiB` (tiêu chí K0.5) | thiết kế §10.1 | `v2/test/boot/budget.sh` (cần bản aarch64) |
| I8 | Không sửa `spec/` ngoài lần mở `[calling.aarch64]` được duyệt | AGENTS §4.1 | review diff |
| I9 | Mỗi bước có **đột biến làm đỏ** trước khi tuyên bố xanh | D2-05, QD | log đột biến trong JOURNAL |

---

## 4. Test (theo tầng)

| Tầng | Nội dung | Lệnh | Trạng thái |
|---|---|---|---|
| T-host | **47** test host hiện có (24 `host.rs` + 19 `sysreg.rs` + 4 `tlb_barrier.rs`, đo 2026-11-20) + test mới cho vector table, chuỗi nạp thanh ghi, `TLBI` | `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64` | có sẵn, mở rộng |
| T-build | Build target bare-metal | `cargo build … --target aarch64-unknown-none` | có sẵn |
| T-objdump | Địa chỉ nạp, thứ tự ô vector, mã `TLBI` | `llvm-objdump -d/-h` | có sẵn (KT-175 đã dùng) |
| T-boot | Boot QEMU aarch64, in marker, dừng sạch | `v2/test/boot/run-aarch64.sh` | **mới** (B3.8) |
| T-mmu | Bật MMU, identity map, nửa cao, physmap | trong `kmain` + TAP | **mới** (B3.3/B3.4) |
| T-tlb | Shootdown đa hart | TAP | **mới** (B3.6) |
| T-tap | TAP đầy đủ smp 1\|2\|4 | `v2/test/boot/ktest.sh` (bản aarch64) | **mới** (B3.8) |
| T-conf | `trace-compare` sim ↔ QEMU aarch64 | `v2/test/boot/` | **mới** (B3.8) |
| T-mut | Đột biến bắt buộc ở B3.2/B3.3/B3.6 | thủ công, log JOURNAL | **mới** |

**Ngưỡng**: giữ nguyên ngưỡng hiện hành của K3 ("như K3"). **Không** nới bất kỳ ngưỡng nào
(AGENTS §4.4). Nếu bước 3 không đạt ngưỡng K3 thì **không** tuyên bố K4' đạt — đúng tiền lệ
KT-160/167.

---

## 5. Rủi ro

| # | Rủi ro | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| R1 | `PageTableFormat` thiếu thao tác nửa cao/physmap ⇒ phải sửa trait giữa chừng | Vỡ **cả ba** impl + test generic | Chốt §6 Q4 **trước** B3.4; nếu sửa trait thì làm **một** commit riêng, chạy `mmu_shared` cho cả ba |
| R2 | Object layer `VSpace` gắn Sv39 | Chặn B3.5 | Tổng quát hoá qua `Mmu` (đúng nợ M17) — **một** lần, không nhân bản |
| R3 | `[calling.aarch64]` chưa mở ⇒ B3.7 bế tắc | Không có U-mode | Làm B3.1–B3.6 trước; B3.7 chờ A0 |
| R4 | QEMU `virt` aarch64 khác riscv64 về UART/GIC địa chỉ | Sai driver | Lấy địa chỉ từ DTB QEMU, **không** viết tay; ghi vào `resources.rs` bản aarch64 |
| R5 | Bật MMU sai thứ tự ⇒ treo không log | Khó chẩn đoán | In marker qua UART **trước và sau** mỗi bước của chuỗi TCR→…→ISB |
| R6 | `-kernel` nạp ELF aarch64: QEMU đòi `Image` hoặc ELF tuỳ phiên bản | Chặn B3.1 | Kiểm bằng `qemu-system-aarch64 --version` trước; nếu cần thì thêm bước `objcopy` sang `Image` |

---

## 6. Cần quyết định ABI/spec mới — **blocker cho A0**

> Đây là **blocker**, không phải đề xuất. Bước 3 **không** mở được cho tới khi Q1 và Q2 có dòng
> quyết định trong `v2/agent/DECISIONS.md` (và `spec/` chỉ được sửa bởi task được ghi rõ).

**Q1 — Đường boot của K4' bước 3: `-kernel` trực tiếp hay AAVMF/UEFI?**
Đề nghị: **`-kernel` trực tiếp**, đúng khuôn riscv64 (`v2/test/boot/run.sh:103-104`) và đúng
`docs/v2/ViFastOs-v2-thiet-ke.md:882` (UEFI **sau** K5'). Cần một dòng D2 vì hiện **chưa** có
quyết định nào nói về đường boot aarch64 — E6.

**Q2 — Mở `spec/abi.toml` `[calling.aarch64]` (một lần, khuôn KT-05)?**
Đề nghị: **chỉ mở khi bước 3 tới B3.7**, và khi mở thì chép **nguyên văn** quy ước đã kiểm bằng
asm thật, không đặt trước. Đây là **blocker cứng** cho B3.7 (D2-04/D2-22: không tự đặt ABI).

**Q3 — `spec/kernel.toml` (`KernelConfig`) có thuộc bước 3 không?**
Đề nghị: **không**. `KernelConfig` (`MAX_HARTS`, trace ring, hộp thư, KPTI —
`docs/v2/ViFastOs-v2-thiet-ke.md:508`) là điều kiện của "**như K3**", tức bước **sau** bước 3.
Bước 3 dùng hằng số hiện có của `vifast-plat-common`.

**Q4 — Nửa cao/physmap: thêm vào `PageTableFormat` hay làm trong crate arch?**
Đây là quyết định **kiến trúc**, ảnh hưởng cả ba arch. Hai lựa chọn:
(a) thêm thao tác vào trait `vifast-pte` ⇒ ba impl + thân test generic phải sửa **cùng** commit
(đúng bài học KT-164: "một thân test generic chạy cả hai định dạng");
(b) làm trong crate arch như riscv64 (`v2/kernel/vifast-plat-riscv64/src/mmu.rs:279`) ⇒ nhanh
nhưng **nhân bản** logic — đúng lớp lỗi "hai bản trùng" mà KT-164 vừa đóng.
Đề nghị: **(a)**, và làm ở **một** task riêng **trước** B3.4.

**Q5 — `MAX_HARTS` cho QEMU `virt` aarch64 (GICv3) là bao nhiêu?**
Đề nghị: giữ **8** như `spec/abi.toml`/`vifast-plat-common` (`MAX_HARTS`), không đổi. Cần xác
nhận vì `-smp` của QEMU aarch64 `virt` hỗ trợ tới 8 hart dễ dàng.

**Q6 — Bước 3 có gồm U-mode không, hay dừng ở kernel boot + MMU + VSpace?**
Đề nghị: **gồm**, nhưng B3.7 là bước **cuối** và **phụ thuộc Q2**. Nếu A0 muốn bước 3 nhỏ hơn
thì cắt B3.7 + B3.8 thành bước 4 — khi đó bước 3 kết thúc ở "kernel chạy nửa cao, MMU bật,
VSpace map/unmap đúng, shootdown đúng".

---

## 7. Ngân sách

| Bước | Giờ | Phụ thuộc |
|---|---|---|
| B3.1 crate ảnh + linker + `_start` + PL011 | 3 | E5, E6 |
| B3.2 vector table + handler | 2,5 | B3.1 |
| B3.3 bật MMU + identity map | 3 | B3.2 |
| B3.4 nửa cao + physmap | 3 | B3.3, **Q4** |
| B3.5 `VSpace` thật | 3 | B3.4, **Q4** |
| B3.6 `TLBI` + shootdown | 2,5 | B3.5 |
| B3.7 U-mode + `eret` + `svc` | 3 | **Q2** (blocker) |
| B3.8 harness + TAP + conformance | 3 | B3.6 |
| **Tổng** | **23 h** [ƯL] | — |

So với K5' ([ƯL] ~24 h, `K5-X86-READINESS.md:195`): cùng bậc, vì aarch64 **không** có
GDT/IDT/TSS/LAPIC/IOAPIC nhưng **có** nửa cao + physmap + GICv3.

---

## 8. Việc **không** làm trong tài liệu này

- Không viết một dòng mã kernel nào.
- Không sửa `spec/`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `v2/agent/QUEUE.md`, `v2/agent/STATE.md`.
- Không tuyên bố K4' đạt, không tuyên bố bước 3 đã mở.
- Không đặt trước quy ước gọi aarch64 (đó là Q2, thuộc A0).
