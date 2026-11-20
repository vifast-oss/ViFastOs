# KT-242 — K5' (x86_64): kế hoạch + harness độc lập

**Ngày**: 2026-11-20 · **Agent 2 (luồng L1/kế hoạch)** · nhánh `task/kt242-l1` · HEAD trước task: `21cc75cd`
**Hàng QUEUE**: `KT-242` (phần "chuẩn bị K5'") · **Trạng thái**: **CHƯA MỞ** triển khai
**Phạm vi**: **chỉ kế hoạch**. Không viết mã kernel/nền tảng mới, không sửa `spec/`, không sửa
`docs/v2/ViFastOs-v2-thiet-ke.md`, không tuyên bố K5' đạt.

> **Đọc kèm**: `v2/agent/reports/K5-X86-READINESS.md` (KT-169 — audit sẵn sàng, 18 mục M1–M18),
> `v2/agent/JOURNAL/kt242-k4-buoc3-design.md` (K4' bước 3 — **cùng** một quyết định trait),
> `v2/agent/DECISIONS.md` D2-10, D2-147, D2-149, D2-163.

---

## 0. K5' đang ở đâu (cập nhật so với audit KT-169)

Audit KT-169 (2026-11-19) kết luận **CHƯA SẴN SÀNG** với 18 mục thiếu. Từ đó **KT-186**
(D2-163, commit `4b3b625d`) đã đóng **hai** mục:

| Mục KT-169 | Nội dung | Hiện trạng 2026-11-20 |
|---|---|---|
| M1 | Crate `vifast-plat-x86_64` | **ĐÓNG** — `v2/kernel/vifast-plat-x86_64/` tồn tại, là member của `v2/kernel/Cargo.toml:7-14` |
| M2 | Paging bốn mức + PCID | **ĐÓNG một phần** — `format.rs` (297 dòng) + `mmu.rs` (re-export `PageTable`/`TableMemory` từ `vifast-pte`); **PCID/INVPCID chưa có** (chỉ có hằng `CR3_PCID_MASK`, `CR4_PCIDE`) |
| M7 | Syscall entry + quy ước thanh ghi | **MỞ một phần** — `calling.rs` (173 dòng) có bảng System V + **đề xuất** ánh xạ syscall; `spec/abi.toml:66-68` **vẫn** `reserved = true` |
| M13 | ACPI/MADT | **CHƯA** — không có `v2/lib/vifast-acpi` |
| M3–M6, M8–M12, M14–M18 | — | **CHƯA** (không đổi) |

**Kết luận**: K5' vẫn **chưa sẵn sàng để mở triển khai**, nhưng **bước dữ liệu (M2) đã xong** —
nghĩa là phần "không cần QEMU" của K5' đã cạn. Mọi bước còn lại đều cần **quyết định của A0**
(đường boot, phạm vi) hoặc **mã kernel** (chưa được duyệt).

---

## 1. Phụ thuộc (dependencies)

### 1.1 Phụ thuộc đã đóng

| # | Phụ thuộc | Bằng chứng |
|---|---|---|
| P1 | Trait `PageTableFormat`/`Mmu` ở crate dùng chung | `v2/lib/vifast-pte/src/mmu.rs`; ba impl Sv39/AArch64/x86_64; thân test generic `v2/kernel/vifast-plat-common/tests/mmu_shared.rs` |
| P2 | `vifast-core` arch-agnostic (`no_std`, không `cfg(target_arch)`, không asm) | `v2/kernel/vifast-core/src/lib.rs:47-48`; audit KT-169 §2.2 |
| P3 | Toolchain có `x86_64-unknown-none` | `v2/rust-toolchain.toml:11` |
| P4 | Crate `vifast-plat-x86_64` + **18** test host (1086 dòng `tests/host.rs`) | KT-186 / `4b3b625d`; đo 2026-11-20 |
| P5 | `qemu-system-x86_64` trên máy đo | kiểm 2026-11-20: `/opt/homebrew/bin/qemu-system-x86_64` |

### 1.2 Phụ thuộc **chưa** đóng — chặn cứng

| # | Phụ thuộc | Chặn bước nào | Ai chốt |
|---|---|---|---|
| P6 | **A0 mở K5'** kèm ngân sách + phạm vi ("như K3", **không** VT-x/KPTI/UEFI) | **mọi** bước M3+ | **A0** |
| P7 | **Đường boot** chốt: `-kernel` ELF trực tiếp **hay** OVMF/UEFI sớm | M3 (rẽ nhánh) | **A0** |
| P8 | `spec/abi.toml` `[calling.x86_64]` mở (khuôn KT-05) | M7 | **A0** |
| P9 | **Quyết định trait**: thêm copy nửa cao/physmap/`protect` vào `PageTableFormat` hay làm trong crate arch | M4 | **A0** — **cùng** quyết định với K4' bước 3 (Q4 của `kt242-k4-buoc3-design.md`) |
| P10 | `vifast-acpi` (RSDP/MADT/MCFG) | M5, M6 | task riêng — **làm được trước, không cần duyệt** (xem §2) |
| P11 | `spec/kernel.toml` (`KernelConfig`) | "như K3", **không** chặn M3–M6 | **A0** |

### 1.3 Phụ thuộc **mềm** (không chặn nhưng đổi thiết kế nếu bỏ qua)

| # | Phụ thuộc | Vì sao |
|---|---|---|
| P12 | Object layer `VSpace` đang gắn Sv39 (`v2/kernel/vifast-plat-common/src/obj/vspace.rs:7`) | M4 phải dùng `Mmu`; nếu không tổng quát hoá thì phải nhân bản — đúng lớp lỗi "hai bản trùng" KT-164 vừa đóng |
| P13 | `Platform` trait hình riscv (`v2/kernel/vifast-plat-common/src/lib.rs:89`) | Thêm method **có default**, không đổi chữ ký riscv |
| P14 | `ASID_BITS` tám bit vs PCID mười hai bit (`v2/lib/vifast-pte/src/lib.rs:74`) | Dùng tập con tám bit trước, ghi nợ; hoặc parameterize |
| P15 | `vifast-traceformat` dùng chung sim ↔ kernel | Điều kiện của conformance ba arch |

---

## 2. Harness độc lập — làm được **trước** khi A0 mở K5'

Đây là phần **không** phải mã kernel, **không** đụng `spec/`, **không** cần QEMU chạy được ảnh
x86. Mỗi mục có tiêu chí đo và **không** tuyên bố gì về K5'.

### H1 — `v2/lib/vifast-acpi`: parser RSDP/MADT/MCFG thuần dữ liệu (3 h)

- **Vì sao độc lập**: chỉ đọc **byte** ACPI; không asm, không MMIO, không `unsafe`, không
  `cfg(target_arch)`. Chạy `cargo test` trên host. Đúng khuôn `vifast-pte`/`vifast-objmem`.
- **Việc**: `Rsdp` (checksum, revision, `XSDT`), `Madt` (kiểu entry 0/1/2/9/11 — LAPIC,
  IOAPIC, IOAPIC override, x2APIC, LAPIC NMI), `Mcfg` (ECAM base + segment/bus range).
  **Không** đọc I/O — nhận `&[u8]`.
- **Đo được**: (i) test host trên **fixture thật** lấy từ QEMU (`-M q35 -dumpdtb`/ACPI dump,
  commit fixture + nguồn); (ii) test âm: checksum sai ⇒ `Err`, entry dài quá buffer ⇒ `Err`
  (không panic, không đọc ngoài); (iii) đột biến: đảo một byte checksum ⇒ đỏ.
- **Không làm**: map MMIO, tìm bảng trong RAM thật, LAPIC/IOAPIC driver.
- **Chặn**: không. **Phụ thuộc**: P10.

### H2 — `v2/test/boot/run-x86_64.sh`: runner QEMU x86_64 + `--selftest` (2 h)

- **Vì sao độc lập**: là **harness**, không phải mã sản phẩm. Viết theo khuôn
  `v2/test/boot/run.sh` (đang chỉ riscv64: `:44`, `:103-104`).
- **Việc**: dựng đúng dòng lệnh `qemu-system-x86_64 -M q35 -cpu max -m … -kernel <elf>
  -nographic -serial …`, timeout cứng, thu log, **từ chối** ảnh sai kiến trúc (đọc `e_machine`
  của ELF = `EM_X86_64` = 62), **từ chối** ảnh thiếu `# build:` (theo KT-145).
- **Đo được**: `--selftest` **≥ 6 ca** (dương: ảnh giả đúng arch + đúng dòng `# build:` ⇒ nhận;
  âm: ELF riscv64 ⇒ từ chối, ELF rỗng ⇒ từ chối, thiếu `# build:` ⇒ từ chối, timeout ⇒ đỏ,
  thiếu `qemu-system-x86_64` ⇒ đỏ rõ). **Không** cần ảnh x86 thật để chạy selftest.
- **Không làm**: build ảnh, TAP, conformance.
- **Chặn**: không. **Phụ thuộc**: P5.

### H3 — `v2/test/boot/budget-x86_64.sh`: ngân sách `.data+.bss` (1,5 h)

- **Vì sao độc lập**: chỉ đọc `llvm-objdump -h` của một ELF; không cần ảnh x86 để **viết**.
- **Việc**: cùng ngưỡng K0.5 (`≤ 256 KiB`, thiết kế §10.1) nhưng cho ELF x86_64; **từ chối**
  chạy nếu ELF không phải x86_64.
- **Đo được**: `--selftest` với **đối chứng âm** dùng chính ảnh riscv64 hiện có (phải **từ
  chối** vì sai arch) + một ELF x86_64 tổng hợp (phải **nhận**); đột biến: cộng thêm một
  section giả vượt trần ⇒ đỏ.
- **Chặn**: không.

### H4 — Fixture ACPI từ QEMU (1 h)

- **Vì sao độc lập**: chỉ chạy QEMU để **lấy byte** bảng ACPI (`-M q35`), lưu fixture + script
  tái tạo. Không phải mã kernel.
- **Đo được**: fixture có sha256 ghim; script tái tạo cho **cùng** sha trên cùng phiên bản QEMU;
  ghi rõ phiên bản QEMU trong fixture (khác phiên bản ⇒ có thể khác byte — ghi nợ, không giả vờ
  bất biến).
- **Chặn**: không. **Phụ thuộc**: P5.

### H5 — `check-x86-asm.sh`: kiểm cấu trúc asm x86 (1,5 h)

- **Vì sao độc lập**: kiểm **văn bản/thư viện**, không chạy kernel.
- **Việc**: khi M3 có asm, script kiểm: mọi `asm!` nằm trong crate `vifast-plat-x86_64` (D2-17);
  không `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`/`vifast-pte` (I1 của K4');
  `global_asm!` chỉ ở crate arch.
- **Đo được**: `--selftest` với cây giả (dương + âm); chạy trên cây **hiện tại** phải xanh.
- **Chặn**: không — nhưng **hữu ích ngay** vì nó là bất biến I1/I2 dùng chung K4'/K5'.

**Tổng H1–H5**: **9 h** [ƯL], **không** cần A0 duyệt (không phải mã kernel, không đụng `spec/`),
và **giảm** rủi ro R7 (ACPI) + R1 (đường boot) của audit KT-169 trước khi K5' mở.

---

## 3. Repro cần dựng (trước khi tuyên bố bất cứ điều gì)

| # | Repro | Vì sao cần | Trạng thái |
|---|---|---|---|
| RP1 | **Dòng lệnh QEMU x86_64** đầy đủ, ghim phiên bản QEMU + `-cpu max` + `-M q35` | Không có repro thì "boot được" không kiểm lại được | H2 |
| RP2 | **Ảnh mẫu + đầu ra mong đợi** (`expected.txt` bản x86) | Khuôn `v2/test/boot/expected.txt` đã có cho riscv64 | sau M3 |
| RP3 | **Đối chứng âm**: ảnh cố ý sai (entry sai / thiếu `# build:`) **phải** làm runner đỏ | Chống "runner luôn xanh" | H2 |
| RP4 | **Tất định**: cùng ảnh, N boot, **cùng** đầu ra | Bất biến trace của dự án | sau M3 |
| RP5 | **Fixture ACPI** + script tái tạo + sha256 ghim | Parser test trên byte thật, không phải byte tự nghĩ | H4 |
| RP6 | **Đột biến bắt buộc** cho từng bước M3–M8 (mỗi bước ≥ 1 đột biến làm đỏ) | D2-05, QD | theo bước |
| RP7 | **So trace sim ↔ QEMU x86_64** bằng `vifast-traceformat` | Điều kiện conformance ba arch | sau M7 |
| RP8 | **CPUID/feature check** trong log (`-cpu max` có PCID/SMEP/SMAP hay không) | R6 của audit: QEMU TCG có thể **không** bật PCID ⇒ xanh giả | sau M4 |

---

## 4. Phần **chưa** được duyệt (không làm trong task này, không làm trước khi có dòng D2)

| # | Phần | Trạng thái | Ghi chú |
|---|---|---|---|
| N1 | M3 — entry + GDT/IDT/TSS + serial | **CHƯA DUYỆT** | mã kernel |
| N2 | M4 — paging thật + long mode + nửa cao/physmap | **CHƯA DUYỆT** | mã kernel; phụ thuộc P9 |
| N3 | M5 — LAPIC timer + IPI + SMP | **CHƯA DUYỆT** | mã kernel |
| N4 | M6 — IOAPIC + MSI/MSI-X + PCI ECAM | **CHƯA DUYỆT** | mã kernel |
| N5 | M7 — U-mode + syscall entry + mở `[calling.x86_64]` | **CHƯA DUYỆT** | cần P8; **cấm** tự đặt ABI (D2-04/D2-22) |
| N6 | M8 — conformance + soak + KPTI/SMEP/SMAP/FPU | **CHƯA DUYỆT** | soak 1.000 boot |
| N7 | M9 — UEFI `vifast-boot` | **CHƯA DUYỆT** — mốc **sau** K5' | thiết kế §10.1 |
| N8 | VT-x (H1) | **CHƯA DUYỆT** — **không** thuộc K5' | thiết kế §10.2 |
| N9 | KPTI bật **mặc định** | **CHƯA DUYỆT** — thiết kế ghi "tuỳ chọn" | thiết kế §4 |
| N10 | Đổi chữ ký `Platform` trait / `PageTableFormat` | **CHƯA DUYỆT** | P9, P13 |
| N11 | Sửa `spec/` bất kỳ | **CẤM** trừ lần mở `[calling.x86_64]` được duyệt | AGENTS §4.1 |

---

## 5. Thứ tự đề xuất (khi được mở)

```
H1 ACPI ─┐
H2 runner ─┼─ (làm trước, không cần duyệt)
H3 budget ─┤
H4 fixture ┘
H5 asm-check ┘
                ↓
M1 chốt phạm vi + đường boot (A0)  ← P6, P7
                ↓
M3 entry + GDT/IDT/TSS + serial
                ↓
M4 paging thật + nửa cao/physmap   ← P9 (cùng quyết định với K4' bước 3)
                ↓
M5 LAPIC + IPI + SMP
                ↓
M6 IOAPIC + MSI + ECAM
                ↓
M7 U-mode + syscall                ← P8 (mở spec/)
                ↓
M8 conformance + soak
                ↓
M9 UEFI (mốc riêng, sau K5')
```

**Ghi chú quan trọng**: **P9 là cùng một quyết định** với Q4 của K4' bước 3. Nếu A0 chốt
"thêm copy nửa cao/physmap vào `PageTableFormat`" thì **một** task làm cho **cả ba** impl
(Sv39/AArch64/x86_64) + thân test generic, và **cả** K4' B3.4 **lẫn** K5' M4 dùng lại. Nếu chốt
"làm trong crate arch" thì mỗi arch nhân bản — nhanh hơn nhưng đúng lớp lỗi KT-164 vừa đóng.
**Đề nghị: gộp P9/Q4 thành một quyết định duy nhất, làm trước cả hai milestone.**

---

## 6. Ngân sách

| Nhóm | Giờ [ƯL] | Cần duyệt? |
|---|---|---|
| H1–H5 (harness độc lập) | 9 | **không** |
| M1 (chốt phạm vi) | 0,5 | A0 |
| M3–M8 | 18 | **có** |
| M9 (UEFI, sau K5') | 3 | **có** |
| **Tổng K5'** | **~24** (khớp `K5-X86-READINESS.md:195`) | — |

---

## 7. Việc **không** làm trong tài liệu này

- Không viết mã kernel/nền tảng mới (kể cả H1–H5 — đây chỉ là **kế hoạch**).
- Không sửa `spec/`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `v2/agent/QUEUE.md`, `v2/agent/STATE.md`.
- Không tuyên bố K5' đạt, không tuyên bố K5' đã mở, không đặt trước `[calling.x86_64]`.
