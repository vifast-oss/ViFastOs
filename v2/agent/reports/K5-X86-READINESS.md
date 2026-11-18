# K5' — Audit sẵn sàng x86_64 (KT-169)

**Task**: KT-169 — audit sẵn sàng cho mốc **K5'** (x86_64). Đây là **báo cáo audit**, không phải
kế hoạch đã duyệt: **không viết mã sản phẩm**, không sửa `spec/`, không sửa thiết kế, không
tuyên bố K5' đạt.
**Ngày**: 2026-11-19 (vòng hai đêm).
**Nền**: `25306b3f`, nhánh `night2/L12-x86audit`, worktree `/private/tmp/night2-L12-x86audit`.
**Kết luận ngắn**: **CHƯA SẴN SÀNG**. Nền dùng chung (trait bảng trang, `vifast-core`, một phần
`vifast-plat-common`) đã đủ để mở **bước dữ liệu** của K5' theo đúng khuôn K4' bước một
(D2-147); nhưng **chưa có** crate `vifast-plat-x86_64`, chưa có đường boot, chưa có linker
script, chưa có harness QEMU x86_64, và **chưa có** phần cứng x86 nào trong `v2/` (LAPIC,
IOAPIC, MSI, GDT/IDT/TSS, syscall entry, VT-x, KPTI). Danh sách đầy đủ ở §3.

**Ba chặn lớn nhất** (chi tiết ở §3): (i) crate `vifast-plat-x86_64` **không tồn tại** — workspace
kernel chỉ có riscv64/aarch64/common; (ii) **đường boot chưa chốt** — thiết kế đặt UEFI là mốc
**sau** K5' (`docs/v2/ViFastOs-v2-thiet-ke.md:882`), trong khi K5' cần một đường boot để đo; (iii)
**trait bảng trang chưa đủ** cho x86 — thiếu thao tác copy nửa cao/physmap mà chính thiết kế
(`docs/v2/ViFastOs-v2-thiet-ke.md:358`) nói phải nằm trong `PageTableFormat`.

## 1. Nguồn đã đọc và phạm vi

Đọc theo thứ tự nguồn sự thật của `AGENTS.md` §1:

| Nguồn | Vị trí | Dùng cho |
|---|---|---|
| Thiết kế v2 (đọc-only) | `docs/v2/ViFastOs-v2-thiet-ke.md:539-545` (bảng ba arch), `docs/v2/ViFastOs-v2-thiet-ke.md:881` (dòng K5'), `docs/v2/ViFastOs-v2-thiet-ke.md:882` (dòng UEFI), `docs/v2/ViFastOs-v2-thiet-ke.md:358` (nửa cao/physmap), `docs/v2/ViFastOs-v2-thiet-ke.md:372` (ngắt), `docs/v2/ViFastOs-v2-thiet-ke.md:391` (FPU), `docs/v2/ViFastOs-v2-thiet-ke.md:397` (KPTI), `docs/v2/ViFastOs-v2-thiet-ke.md:400` (SMEP/SMAP), `docs/v2/ViFastOs-v2-thiet-ke.md:435` (VT-x), `docs/v2/ViFastOs-v2-thiet-ke.md:492` (đóng băng ABI), `docs/v2/ViFastOs-v2-thiet-ke.md:508` (`KernelConfig`), `docs/v2/ViFastOs-v2-thiet-ke.md:516-519` (boot/UEFI), `docs/v2/ViFastOs-v2-thiet-ke.md:686-687` (bản đồ crate), `docs/v2/ViFastOs-v2-thiet-ke.md:1030` (D2-10), `docs/v2/ViFastOs-v2-thiet-ke.md:1101` (rủi ro cắt còn một arch), `docs/v2/ViFastOs-v2-thiet-ke.md:1116` (x86 AP→BSP + PCID), `docs/v2/ViFastOs-v2-thiet-ke.md:1132` (Q9) | Phạm vi K5', tiêu chí, ràng buộc |
| Quyết định v2 | `v2/agent/DECISIONS.md:39` (D2-10), `v2/agent/DECISIONS.md:269` (D2-147 — K4' bước một), `v2/agent/DECISIONS.md:273` (D2-149 — N1 ở lại K5') | Cách tách bước, tiền lệ K4' |
| ABI | `spec/abi.toml:50-68` — `[calling.riscv64]` đóng băng, `[calling.aarch64]`/`[calling.x86_64]` chỉ giữ chỗ | Quy ước gọi x86_64 còn **reserved** |
| Toolchain | `v2/rust-toolchain.toml:11-12` — đã có target `x86_64-unknown-none` | Build target sẵn |
| Mã nguồn | `v2/kernel/`, `v2/lib/vifast-pte/`, `v2/kernel/vifast-core/`, `v2/test/boot/` | Hiện trạng |
| Tham khảo v1 (đọc-only) | `crates/vifast-platform-x86_64/`, `crates/vifast-kernel-x86_64-uefi/`, `crates/vifast-acpi/` | Thuật toán/bit để tham khảo, **không** phải dependency |

**Phạm vi K5' theo thiết kế**: dòng K5' ghi "plat-x86_64 (+ KPTI tuỳ chọn)" với tiêu chí
"như K3 + `-cpu max`" (`docs/v2/ViFastOs-v2-thiet-ke.md:881`). "Như K3" nghĩa là lặp lại tiêu chí
của K3: TAP đầy đủ trên smp 1|2|4, soak boot không lỗi trên máy `ps`, và các mốc phụ thuộc
(K3.5/K6) chỉ chạy lại khi K5' xong. **VT-x** (`docs/v2/ViFastOs-v2-thiet-ke.md:435`,
`docs/v2/ViFastOs-v2-thiet-ke.md:546`) thuộc H1, **không** nằm trong tiêu chí K5'; **UEFI**
(`docs/v2/ViFastOs-v2-thiet-ke.md:882`) là mốc riêng **sau** K5'. Báo cáo này giữ đúng ranh giới
đó.

## 2. Đã có — dùng lại được

### 2.1 Trait và kiểu dùng chung (`vifast-pte`)

Đây là tài sản quan trọng nhất: từ KT-164, `PageTableFormat`/`Mmu` nằm ở crate **dùng chung**
`vifast-pte`, không còn định nghĩa cục bộ trong crate arch.

| Tài sản | Vị trí | Dùng lại được gì | Vướng cho x86 |
|---|---|---|---|
| `PageTableFormat` | `v2/lib/vifast-pte/src/mmu.rs:253` | hình học (`LEVELS`, `PAGE_BITS`, `INDEX_BITS`, `VA_BITS`, `PA_BITS`, `PAGE_BYTES`, `ENTRIES`), `index`, `is_canonical`, `encode_table`, `encode_leaf`, `decode`, `perms_valid` | **không** có thao tác copy nửa cao/physmap, `protect`, `map_with` attrs; `Descriptor::Block` không có nghĩa với x86 (x86 dùng bit `PS`, không có block entry) |
| `Mmu` | `v2/lib/vifast-pte/src/mmu.rs:297` | `root`/`map`/`unmap`/`translate` — đủ cho bước dữ liệu | chỉ map **lá mức cuối**; không map hai MiB/một GiB; không đổi quyền sau map |
| `Perms`/`MemAttr`/`Shareability` | `v2/lib/vifast-pte/src/mmu.rs:178` (`Descriptor`), `v2/lib/vifast-pte/src/mmu.rs:210` (`MapError`), `v2/lib/vifast-pte/src/mmu.rs:239` (`Translation`), `v2/lib/vifast-pte/src/mmu.rs:318` (`is_canonical_bits`) | x86 dùng lại nguyên: `Perms` ánh xạ `R/W/X/US`; `MemAttr::Device/Normal` ánh xạ `PCD/PWT/PAT`; `Shareability` bỏ qua | `Shareability` không có nghĩa với x86 (không có `SH`); cần quy ước "bỏ qua" thay vì lỗi |
| Hằng hình học | `v2/lib/vifast-pte/src/lib.rs:55` (`PAGE_SIZE_BITS`), `v2/lib/vifast-pte/src/lib.rs:58` (`PTE_LEVELS`), `v2/lib/vifast-pte/src/lib.rs:63` (`VA_BITS`), `v2/lib/vifast-pte/src/lib.rs:66` (`ASID_BITS`), `v2/lib/vifast-pte/src/lib.rs:75` (`MAX_FRAMES_PER_VSPACE`), `v2/lib/vifast-pte/src/lib.rs:81` (`MAX_MAP_DEPTH`) | `PAGE_SIZE_BITS` (bốn KiB/hai MiB/một GiB) và `MAX_FRAMES_PER_VSPACE` dùng được | `PTE_LEVELS` (ba cấp) và `VA_BITS` (ba mươi chín bit) là **hình Sv39** — x86 bốn cấp phải dùng `Self::LEVELS` của trait, **không** dùng hằng crate-root; `ASID_BITS` (tám bit) hẹp hơn PCID (mười hai bit) |
| `Pte`/`PteArray`/`WalkShape` | `v2/lib/vifast-pte/src/pte.rs:14`, `v2/lib/vifast-pte/src/pte.rs:60`, `v2/lib/vifast-pte/src/walk.rs:34` | **không** dùng cho x86 (bit layout Sv39) — chỉ dùng làm khuôn | x86 phải viết `Pte`/`walk_shape` riêng (bit `P/RW/US/PWT/PCD/A/D/PS/PAT/NX`) |

**Kết luận mục này**: trait đã đủ để x86 **cài** một format mới và chạy **cùng thân test generic**
như Sv39/AArch64 (bằng chứng: `v2/kernel/vifast-plat-common/tests/mmu_shared.rs:156` —
`kiem_tung_dinh_dang<F: PageTableFormat>` chạy cho cả hai định dạng). Nhưng trait **chưa đủ** cho
phần kernel thật: thiếu copy nửa cao, thiếu `protect`, thiếu map block.

### 2.2 `vifast-core` — arch-agnostic

`vifast-core` **dùng lại được nguyên** cho x86_64:

- `#![no_std]` + `#![forbid(unsafe_code)]` — `v2/kernel/vifast-core/src/lib.rs:47-48`.
- Không `cfg(target_arch)`, không asm, không `unsafe` — kiểm bằng máy: `grep -rn "cfg(target_arch)"`
  trên `vifast-core`/`vifast-plat-common`/`vifast-pte` trả **không** dòng nào (khối
  `scope=kt169-audit.log` ở §6).
- Chỉ phụ thuộc `vifast-abi` — `v2/kernel/vifast-core/Cargo.toml:10-13`.
- Toàn bộ object/TCB/IPC/scheduler/MCS/notification nằm ở đây; phần arch chỉ cài hook
  `TlbEnv` (`v2/kernel/vifast-core/src/tlb.rs:84`) và đọc/ghi thanh ghi syscall.

**Vướng duy nhất**: `SyscallOutcome::Return` có doc ghi `sepc` cộng bốn
(`v2/kernel/vifast-core/src/syscall.rs:143`) — x86 arch phải đọc đây là "IP cộng độ dài
instruction" (`syscall`/`sysret` khác `int 0x80`), không phải sửa core. Đây là **nợ tài liệu**,
không phải nợ mã.

### 2.3 `vifast-plat-common` — cái gì **thật sự** arch-agnostic

Crate này là **nửa riscv, nửa dùng chung**. Bảng dưới phân loại từng module; cột "dùng lại" nói
rõ mức.

| Module | Vị trí | Thật sự arch-agnostic? | Dùng lại cho x86 |
|---|---|---|---|
| `PanicHandler`, `PlatformWriter` | `v2/kernel/vifast-plat-common/src/lib.rs:231`, `v2/kernel/vifast-plat-common/src/lib.rs:204` | **Có** — chỉ dựa trên `Platform` | Nguyên |
| `HartMask`, bố cục arena theo hart | `v2/kernel/vifast-plat-common/src/smp.rs:140`, `v2/kernel/vifast-plat-common/src/smp.rs:31` (`MAX_HARTS`) | **Có** (bitmask + hằng số), trừ phần SBI HSM | Nguyên phần `HartMask`/arena; thay `SBI_EXT_HSM` (`v2/kernel/vifast-plat-common/src/smp.rs:269`) bằng INIT-SIPI |
| Hộp thư IPI coalesce theo kind | `v2/kernel/vifast-plat-common/src/ipi.rs:233` (`IpiMailbox`) | **Có** — ngữ nghĩa hộp thư không phụ thuộc arch | Nguyên; thay `SBI_EXT_IPI` (`v2/kernel/vifast-plat-common/src/ipi.rs:49`) bằng LAPIC ICR |
| ASID + giao thức flush/shootdown | `v2/kernel/vifast-plat-common/src/tlb.rs:113` (`AsidAllocator`), `v2/kernel/vifast-plat-common/src/tlb.rs:255` (`FlushScope`), `v2/kernel/vifast-plat-common/src/tlb.rs:314` (`TlbFlushRequest`) | **Gần như** — giao thức dùng chung, độ rộng ASID lấy từ `vifast_pte::ASID_BITS` | Nguyên giao thức; **phải** parameterize độ rộng cho PCID (mười hai bit) hoặc chấp nhận tập con tám bit |
| `PhysMem` | `v2/kernel/vifast-plat-common/src/obj/mod.rs:327` | **Có** — chỉ đọc/ghi tám B vật lý | Nguyên (arch cài bằng physmap) |
| `Event`, `TrapAction`, bộ đếm trap | `v2/kernel/vifast-plat-common/src/trap.rs:346` | **Có** — enum sự kiện trung lập | Nguyên; `decode_trap`/`TrapFrame` thì **không** |
| `FaultKind` | `v2/kernel/vifast-plat-common/src/fault.rs:133` | **Có** — enum ABI | Nguyên; `TrapFacts`/`classify_trap` thì **không** |
| `UserTask` + `dispatch` | `v2/kernel/vifast-plat-common/src/user.rs:179`, `v2/kernel/vifast-plat-common/src/user.rs:422` | **Một phần** — state machine + gọi core dùng chung | Nguyên state machine; quy ước `a7/a0/a1` (`v2/kernel/vifast-plat-common/src/user.rs:21`) là riscv |
| `Platform` trait | `v2/kernel/vifast-plat-common/src/lib.rs:89` | **Không** — method `set_trap_vector` (`v2/kernel/vifast-plat-common/src/lib.rs:117`), `enable_irq` (`v2/kernel/vifast-plat-common/src/lib.rs:125`), `set_timer_deadline` (`v2/kernel/vifast-plat-common/src/lib.rs:135`), `enable_timer` (`v2/kernel/vifast-plat-common/src/lib.rs:142`) mang ngữ nghĩa PLIC/SBI | Impl lại theo nghĩa x86 (IDT/LAPIC/IOAPIC); **không** đổi chữ ký riscv |
| `TrapFrame` | `v2/kernel/vifast-plat-common/src/trap.rs:148` | **Không** — `x0..x31` + `sepc/sstatus/scause/stval` | Viết frame x86 riêng (GPR + vector + error code + iret frame) |
| `PageTableBuilder`/`Sv39` | `v2/kernel/vifast-plat-common/src/mmu.rs:300`, `v2/kernel/vifast-plat-common/src/mmu.rs:474`, `v2/kernel/vifast-plat-common/src/mmu.rs:481` | **Không** — Sv39 | Chỉ dùng làm khuôn |
| PLIC | `v2/kernel/vifast-plat-common/src/plic.rs` | **Không** — PLIC | Thay bằng IOAPIC/MSI |
| Timer SBI | `v2/kernel/vifast-plat-common/src/timer.rs:50` (`TIMEBASE_HZ`), `v2/kernel/vifast-plat-common/src/timer.rs:52` (`TICK_HZ`) | **Một phần** — phân loại tick/deadline dùng chung; timebase SBI là riscv | Nguyên phần phân loại; thay nguồn bằng TSC/LAPIC |
| Bảng vùng MMIO QEMU `virt` | `v2/kernel/vifast-plat-common/src/resources.rs:84` (`REGIONS`) | **Không** — địa chỉ QEMU `virt` | Thay bằng bảng vùng lấy từ ACPI/MCFG |
| `TrapFacts` | `v2/kernel/vifast-plat-common/src/fault.rs:230` | **Không** — `scause/stval/sepc` | Viết `TrapFacts` x86 (vector, error code, CR2) |
| Object layer VSpace | `v2/kernel/vifast-plat-common/src/obj/vspace.rs:1`, `v2/kernel/vifast-plat-common/src/obj/vspace.rs:7` | **Không** — import `walk_shape`/`PTE_LEVELS` và PTE Sv39 | Tổng quát hoá qua `Mmu` hoặc nhân bản — xem M17 ở §3 |

### 2.4 Khuôn crate plat mới (`vifast-plat-aarch64`)

`vifast-plat-aarch64` là **khuôn chính xác** cho bước dữ liệu của `vifast-plat-x86_64`:

- `v2/kernel/vifast-plat-aarch64/src/format.rs:247` — `impl PageTableFormat for Aarch64` (chỉ
  impl, không định nghĩa lại trait).
- `v2/kernel/vifast-plat-aarch64/src/mmu.rs:42` — `TableMemory` (nguồn frame trừu tượng),
  `v2/kernel/vifast-plat-aarch64/src/mmu.rs:131` — `PageTable<F, M>`,
  `v2/kernel/vifast-plat-aarch64/src/mmu.rs:168` — `map`,
  `v2/kernel/vifast-plat-aarch64/src/mmu.rs:254` — `unmap`,
  `v2/kernel/vifast-plat-aarch64/src/mmu.rs:325` — `translate`.
- `v2/kernel/vifast-plat-aarch64/tests/host.rs` — test host đầy đủ (bit literal viết tay, quyền,
  walk bốn mức, high-half, 48-bit VA, không rò frame).
- `v2/kernel/vifast-plat-aarch64/Cargo.toml:1-8` — crate **thuần dữ liệu**: không asm, không
  `cfg(target_arch)`, không `unsafe`, không dependency ngoài.

**Nợ cần chuyển**: `TableMemory` đang là trait **riêng của crate aarch64**
(`v2/kernel/vifast-plat-aarch64/src/mmu.rs:42`). Nếu x86 dùng lại khuôn này thì nên **chuyển
`TableMemory` vào `vifast-pte`** để ba format dùng chung một nguồn frame — nếu không, x86 sẽ
định nghĩa bản thứ hai (đúng lớp lỗi "hai bản trùng" mà KT-164 vừa đóng).

### 2.5 Khuôn asm/CSR/MMIO (`vifast-plat-riscv64`)

`vifast-plat-riscv64` là khuôn **cấu trúc** cho crate arch x86 (không phải nội dung):

- `v2/kernel/vifast-plat-riscv64/src/lib.rs:151` — `global_asm!` chứa
  `_start`/`_start_secondary`/`trap_entry`; `v2/kernel/vifast-plat-riscv64/src/lib.rs:215` —
  `trap_entry`; `v2/kernel/vifast-plat-riscv64/src/lib.rs:329` — `sret`.
- `v2/kernel/vifast-plat-riscv64/src/mmu.rs:279` — dựng bảng rồi `csrw satp` + `sfence.vma`
  (hai bước, không nửa vời).
- `v2/kernel/vifast-plat-riscv64/src/timer.rs` — SBI TIME;
  `v2/kernel/vifast-plat-riscv64/src/plic.rs:60` — MMIO `read_volatile`/`write_volatile`;
  `v2/kernel/vifast-plat-riscv64/src/user.rs:284` — `sret` xuống U-mode;
  `v2/kernel/vifast-plat-riscv64/src/smp.rs` — SBI HSM;
  `v2/kernel/vifast-plat-riscv64/src/ipi.rs` — SBI IPI;
  `v2/kernel/vifast-plat-riscv64/src/tlb.rs` — `sfence.vma`.
- Cấu trúc module/feature (`Cargo.toml`, `#[cfg(feature = ...)]`) và cách cài `Platform`/
  `PhysMem` dùng lại được nguyên xi.

### 2.6 Toolchain và tham khảo v1

- **Toolchain**: `v2/rust-toolchain.toml:11-12` đã khai target `x86_64-unknown-none` — không cần
  thêm target.
- **v1 (đọc-only, đóng băng)**: `crates/vifast-platform-x86_64/src/platform.rs:1` có GDT/TSS/IDT,
  ring3, PIC/PIT, user paging; `crates/vifast-platform-x86_64/src/platform.rs:59` PML4;
  `crates/vifast-platform-x86_64/src/platform.rs:472` GDT theo hart;
  `crates/vifast-platform-x86_64/src/platform.rs:475` IDT;
  `crates/vifast-platform-x86_64/src/platform.rs:496` TSS;
  `crates/vifast-platform-x86_64/src/platform.rs:759` `enable_pcid`;
  `crates/vifast-platform-x86_64/src/platform.rs:1219` `configure_lapic`;
  `crates/vifast-platform-x86_64/src/platform.rs:1266` `reload_cr3`.
  `crates/vifast-kernel-x86_64-uefi/src/main.rs:1` là entry UEFI;
  `crates/vifast-kernel-x86_64-uefi/src/user.S:34` dùng `int 0x80`;
  `crates/vifast-acpi/src/lib.rs` có parser ACPI/MADT.
  **Giới hạn của v1**: không có IOAPIC/MSI/VT-x/KPTI; chỉ một AP
  (`crates/vifast-platform-x86_64/src/platform.rs:29`); dùng PIC/PIT chứ không phải LAPIC timer
  cho tick. Vì vậy v1 là **tham khảo**, không phải bản chép được.

## 3. Còn thiếu cho K5'

Bảng dưới có **mười tám mục**; mỗi mục ghi hiện trạng bằng `file:dòng` (thường là "không có" —
chứng minh bằng khối `scope=kt169-audit.log` ở §6). Thứ tự không phải thứ tự thi công (thứ tự
thi công ở §4).

| # | Hạng mục | Hiện trạng (`file:dòng`) | Ảnh hưởng K5' | Ghi chú |
|---|---|---|---|---|
| M1 | Crate `vifast-plat-x86_64` | `v2/kernel/Cargo.toml:7-13` — members chỉ có `vifast-core`, `vifast-kbin-riscv64`, `vifast-plat-aarch64`, `vifast-plat-common`, `vifast-plat-riscv64`; không có thư mục `v2/kernel/vifast-plat-x86_64/` | Chặn **mọi** bước | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:686` |
| M2 | Paging bốn mức + PCID/INVPCID | `v2/lib/vifast-pte/src/lib.rs:58` (`PTE_LEVELS` ba cấp); `v2/kernel/vifast-plat-common/src/mmu.rs:481` (Sv39); `v2/kernel/vifast-plat-aarch64/src/format.rs:247` (AArch64); **không** có PML4/PDPT/PD/PT trong `v2/` | Chặn boot có MMU | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:541` |
| M3 | Copy nửa cao/physmap trong trait | `v2/lib/vifast-pte/src/mmu.rs:253` — trait **không** có thao tác copy; riscv làm trong arch `v2/kernel/vifast-plat-riscv64/src/mmu.rs:279` | Chặn VSpace user | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:358` nói logic nằm trong `PageTableFormat` |
| M4 | LAPIC/x2APIC + timer TSC-deadline | `v2/kernel/vifast-plat-common/src/lib.rs:135` (`set_timer_deadline` là SBI TIME); `v2/kernel/vifast-plat-common/src/timer.rs:50` (`TIMEBASE_HZ`); **không** có LAPIC trong `v2/` | Chặn tick/preempt | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:543` |
| M5 | IOAPIC + MSI/MSI-X | `v2/kernel/vifast-plat-common/src/plic.rs:1` (PLIC); `v2/kernel/vifast-plat-common/src/resources.rs:84` (vùng QEMU `virt`); **không** có IOAPIC/MSI trong `v2/` | Chặn ngắt ngoài | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:372`, `docs/v2/ViFastOs-v2-thiet-ke.md:542` |
| M6 | GDT/IDT/TSS + chuyển mức đặc quyền | `v2/kernel/vifast-plat-common/src/trap.rs:148` (trapframe riscv); **không** có GDT/IDT/TSS trong `v2/` | Chặn U-mode | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:545` |
| M7 | Syscall entry + quy ước thanh ghi | `spec/abi.toml:66-68` — `[calling.x86_64]` còn `reserved = true`; `v2/kernel/vifast-plat-common/src/user.rs:21` (a7/a0/a1); `v2/kernel/vifast-core/src/syscall.rs:143` (`sepc` cộng bốn) | Chặn U-mode/ABI | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:492`; cần Agent 0 mở `spec/` |
| M8 | VT-x | **Không** có mã; thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:435`, `docs/v2/ViFastOs-v2-thiet-ke.md:546` | **Không** chặn K5' "như K3"; chặn H1 | Tách khỏi K5' |
| M9 | KPTI | **Không** có mã; thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:397`; **không** có `spec/kernel.toml` (thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:508`) | Tuỳ chọn; chặn tuyên bố bảo mật x86 | Cần quyết định `KernelConfig` |
| M10 | `vifast-boot` UEFI | **Không** có `v2/boot/`; thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:687`, `docs/v2/ViFastOs-v2-thiet-ke.md:516`, `docs/v2/ViFastOs-v2-thiet-ke.md:882` | Chặn đường boot chuẩn | Có thể tạm dùng `-kernel` (xem §4) |
| M11 | Linker script x86_64 | `v2/kernel/vifast-kbin-riscv64/linker.ld:5` (`OUTPUT_ARCH(riscv)`), `v2/kernel/vifast-kbin-riscv64/linker.ld:10` (`0x80200000`); **không** có linker x86 | Chặn ảnh boot | Cần linker riêng |
| M12 | Harness QEMU `-cpu max` | `v2/test/boot/run.sh:44` (`qemu-system-riscv64`), `v2/test/boot/run.sh:103-104` (`-machine virt -cpu rv64`); **không** có runner x86 | Chặn đo | Cần runner x86 |
| M13 | ACPI/MADT | **Không** có `v2/lib/vifast-acpi`; v1 `crates/vifast-acpi/src/lib.rs` (đọc-only) | Chặn LAPIC/IOAPIC/ECAM | Port từ v1 |
| M14 | FPU/XSAVE | **Không** có mã; thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:391` | Chặn user SSE | Bước M8 |
| M15 | SMEP/SMAP | **Không** có mã; thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:400` | Bảo mật | Bước M8 |
| M16 | Độ rộng ASID/PCID | `v2/lib/vifast-pte/src/lib.rs:66` (`ASID_BITS` tám bit); `v2/kernel/vifast-plat-common/src/tlb.rs:113` (`AsidAllocator`) | PCID mười hai bit không dùng hết | Parameterize hoặc chấp nhận tập con |
| M17 | Object layer VSpace gắn Sv39 | `v2/kernel/vifast-plat-common/src/obj/vspace.rs:1` (doc "PTE Sv39"), `v2/kernel/vifast-plat-common/src/obj/vspace.rs:7` (import `walk_shape`, `PTE_LEVELS`) | Chặn VSpace x86 | Tổng quát hoá qua `Mmu` hoặc nhân bản |
| M18 | `Platform` trait hình riscv | `v2/kernel/vifast-plat-common/src/lib.rs:89` (trait), `v2/kernel/vifast-plat-common/src/lib.rs:117`, `v2/kernel/vifast-plat-common/src/lib.rs:125`, `v2/kernel/vifast-plat-common/src/lib.rs:135`, `v2/kernel/vifast-plat-common/src/lib.rs:142` (method) | Không chặn, nhưng phải reinterpret | Thêm method có default, không đổi chữ ký riscv |

**Đọc bảng này đúng cách**: M1–M7 và M11–M13 là **chặn cứng** (không có thì không boot được);
M10 (UEFI) chỉ chặn **nếu** chọn đường UEFI — nếu chọn `-kernel` thì M10 lùi về M9; M8/M9/M14/M15
là **hoãn có chủ đích** (K5' "như K3" không đòi); M16–M18 là **nợ kiến trúc** phải quyết trước
khi viết dòng đầu tiên, nếu không sẽ phải sửa lại trait/object layer giữa chừng.

## 4. Thứ tự thi công đề xuất

Mỗi bước ≤ **ba giờ** (AGENTS §10.4), có tiêu chí **đo được** và **ngân sách** riêng. Tổng
[ƯL] khoảng **hai mươi tư giờ** — K5' lớn hơn K4' vì có phần cứng x86 thật (LAPIC/IOAPIC/GDT/
IDT/TSS) mà K4' bước một chưa chạm.

### Bước M1 — Chốt phạm vi và đường boot (nửa giờ)

- **Việc**: ghi một quyết định ngắn (D2 mới, Agent 0 duyệt) chốt ba thứ: (i) K5' = "như K3",
  **không** gồm VT-x/KPTI/UEFI/PCIe-MSIX; (ii) đường boot tạm của K5' là QEMU `-kernel` với ELF
  (multiboot) **hay** OVMF/UEFI sớm; (iii) ai mở `spec/abi.toml` `[calling.x86_64]` và khi nào.
- **Tiêu chí đo**: văn bản quyết định có đủ ba mục "trong/ngoài/hoãn", có `file:dòng` cho từng
  mục, và lead duyệt bằng một dòng `APPROVED`.
- **Ngân sách**: nửa giờ.
- **Vì sao trước**: nếu chọn UEFI sớm thì bước M3 phải làm `vifast-boot` trước; nếu chọn
  `-kernel` thì M3 nhẹ hơn và UEFI lùi về sau K5'. Đây là quyết định **rẽ nhánh**, không phải
  chi tiết.

### Bước M2 — `vifast-plat-x86_64` phần dữ liệu (ba giờ)

- **Việc**: tạo crate mới trong workspace kernel; `format.rs` (bốn mức, bốn KiB, VA canonical ba
  mươi chín bit, PA bốn mươi tám bit; descriptor `P/RW/US/PWT/PCD/A/D/PS/PAT/NX`; `impl
  PageTableFormat`); `mmu.rs` (`PageTable<F, TableMemory>` map/unmap/translate/walk/destroy,
  **không** asm, **không** `unsafe`); test host.
- **Tiêu chí đo**: `cargo test -p vifast-plat-x86_64` xanh; thân test generic
  `kiem_tung_dinh_dang::<X86_64>()` chạy **cùng** thân với Sv39/AArch64 (mở rộng
  `v2/kernel/vifast-plat-common/tests/mmu_shared.rs`); test bit literal viết tay theo Intel SDM;
  `translate` từ chối bảng-mức-lá; không rò frame; `cargo clippy` sạch; L0 thêm
  `-p vifast-plat-x86_64`.
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M1.

### Bước M3 — Entry + GDT/IDT/TSS + serial (ba giờ)

- **Việc**: `_start`/entry theo đường boot đã chốt; GDT (kernel/user code/data + TSS); IDT đủ
  vector ngoại lệ; TSS (`RSP0`); serial (UART); `Platform::put_str`/`halt`; `PanicHandler`.
- **Tiêu chí đo**: boot QEMU x86_64 tới `DebugPutStr` + `DebugHalt`; TAP một test; **năm mươi**
  boot không lỗi; ảnh nằm trong ngân sách `.data+.bss` hiện hành; không `unsafe` ngoài crate arch.
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M2.

### Bước M4 — Paging thật + long mode + nửa cao/physmap (ba giờ)

- **Việc**: dựng PML4/PDPT/PD/PT; map kernel image + physmap + MMIO; bật `CR0.PG`/`CR4.PAE`/
  `EFER.LME`; nạp `CR3`; `NX`; guard page; cài `Mmu` trên bảng thật; cài `TlbEnv`
  (`INVLPG`/`CR3` reload).
- **Tiêu chí đo**: boot với paging; walk phần mềm đối chiếu phần cứng; guard page fault **thật**;
  **năm mươi** boot; `-cpu max` và `-cpu qemu64` cùng xanh (nếu chọn được hai cấu hình).
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M3.

### Bước M5 — Timer LAPIC + IPI + SMP (ba giờ)

- **Việc**: LAPIC/x2APIC (MSR); TSC-deadline (hoặc LAPIC timer); IPI qua ICR; INIT-SIPI
  trampoline cho AP; arena theo hart; `HartMask`.
- **Tiêu chí đo**: tick **một trăm** Hz; IPI round-trip; boot smp 1|2|4; **năm mươi** boot mỗi
  cấu hình; không mất tick (đếm `late`/`max_delta` như riscv).
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M4.

### Bước M6 — Ngắt ngoài: IOAPIC + MSI/MSI-X + PCI ECAM (ba giờ)

- **Việc**: IOAPIC redirection table; MSI/MSI-X capability; PCI ECAM (MCFG từ ACPI); UART IRQ.
- **Tiêu chí đo**: UART IRQ qua IOAPIC; một MSI line; không IRQ giả; **năm mươi** boot.
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M5.

### Bước M7 — U-mode + syscall entry + ABI x86_64 (ba giờ)

- **Việc**: `syscall`/`sysret` hoặc `int 0x80`; MSR `STAR/LSTAR/FMASK`; `swapgs`; bảng trang
  user; mở `spec/abi.toml` `[calling.x86_64]` (Agent 0); `UserTask`/`dispatch`.
- **Tiêu chí đo**: `init` ELF chạy; **mười sáu** syscall; fault cách ly; `DebugGetInfo` khớp
  golden; **năm mươi** boot.
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M6 + Agent 0 mở `spec/`.

### Bước M8 — Conformance + soak + KPTI/SMEP/SMAP/FPU (ba giờ)

- **Việc**: chạy conformance ba arch; KPTI tuỳ chọn; SMEP/SMAP; XSAVE; soak.
- **Tiêu chí đo**: TAP **một trăm phần trăm** trên smp 1|2|4; soak **không lỗi** trên **một
  nghìn** boot; KPTI bật/tắt đều boot; SMEP fault test; XSAVE context.
- **Ngân sách**: ba giờ.
- **Phụ thuộc**: M7.

### Bước M9 — UEFI `vifast-boot` (mốc riêng, **sau** K5')

- **Việc**: ứng dụng UEFI cho ba arch; đọc ACPI/DTB; dựng bảng trang ban đầu; `BootDesc`.
- **Tiêu chí đo**: boot ba arch qua UEFI, **hoặc** quyết định giữ OpenSBI cho riscv64
  (`docs/v2/ViFastOs-v2-thiet-ke.md:882`).
- **Ngân sách**: ba giờ (tách task riêng).
- **Phụ thuộc**: K5' xong.

## 5. Rủi ro và cách giảm

| # | Rủi ro | Vì sao thật | Cách giảm |
|---|---|---|---|
| R1 | Đường boot chưa chốt (UEFI vs `-kernel`) làm trễ M3 | Thiết kế đặt UEFI **sau** K5' nhưng K5' cần boot để đo | M1 chốt; chọn `-kernel`/multiboot tạm; UEFI là M9 |
| R2 | Trait `PageTableFormat` thiếu copy nửa cao/protect/map block | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:358` nói logic copy nằm trong trait, hiện **không** có | M2 thêm method (có default) + test generic ba format; không sửa hành vi Sv39/AArch64 |
| R3 | Object layer VSpace gắn Sv39 | `v2/kernel/vifast-plat-common/src/obj/vspace.rs:7` import thẳng `walk_shape`/`PTE_LEVELS` | M2 chỉ làm format; M4 dùng `Mmu`; tổng quát hoá `obj/vspace.rs` là task riêng (ghi nợ) |
| R4 | ASID tám bit vs PCID mười hai bit | `vifast_pte::ASID_BITS` và `AsidAllocator` gắn tám bit | Dùng tập con tám bit trước; parameterize sau; ghi nợ rõ |
| R5 | `Platform` trait hình riscv | Method mang ngữ nghĩa PLIC/SBI | Giữ chữ ký; thêm method mới có default; x86 impl theo nghĩa IDT/LAPIC/IOAPIC |
| R6 | QEMU TCG không bật đủ PCID/SMEP/SMAP ⇒ xanh giả | v1 từng ghi "QEMU TCG hiện không bật PCID" (`docs/v1/overnight-log.md:48-49`) | `-cpu max`; kiểm CPUID; ca âm tắt feature; ghi rõ cấu hình trong log |
| R7 | ACPI/MADT chưa có ⇒ LAPIC base/IOAPIC/ECAM sai | `v2/lib/vifast-acpi` không tồn tại | Port từ `crates/vifast-acpi` (đọc-only) ở M5/M6; test host parser trước |
| R8 | FPU/XSAVE chưa có ⇒ user SSE hỏng | Không có mã trong `v2/` | M8; nếu user Rust dùng SSE thì kéo lên M7 |
| R9 | KPTI làm chậm/khó, dễ sinh lỗi | Thiết kế chỉ nói "tuỳ chọn" | Mặc định tắt trên QEMU TCG; bật khi CPUID Meltdown; đo trước/sau |
| R10 | Ngân sách K5' lớn hơn K4' | Nhiều phần cứng hơn (LAPIC/IOAPIC/GDT/IDT/TSS) | Tách đúng tám bước ≤ ba giờ; mỗi bước có tiêu chí riêng; không gộp |

## 6. Bằng chứng

Nền và trạng thái worktree:

```text scope=kt169-audit.log
$ git rev-parse --short HEAD
25306b3f
$ git branch --show-current
night2/L12-x86audit
```

Không có crate/thư mục x86_64, không có `v2/boot/`, không có `vifast-acpi` trong `v2/lib/`:

```text scope=kt169-audit.log
$ test -d v2/kernel/vifast-plat-x86_64; echo "plat-x86_64-dir-exit=$?"
plat-x86_64-dir-exit=1
$ test -d v2/boot; echo "boot-dir-exit=$?"
boot-dir-exit=1
$ test -d v2/lib/vifast-acpi; echo "acpi-dir-exit=$?"
acpi-dir-exit=1
```

Không có mã x86 nào trong `v2/kernel` + `v2/lib`, và `vifast-core`/`vifast-plat-common`/
`vifast-pte` không có `cfg(target_arch)`:

```text scope=kt169-audit.log
$ grep -n "vifast-plat-x86_64" v2/kernel/Cargo.toml | wc -l
       0
$ grep -rnE "\b(PML4|IOAPIC|LAPIC|GDT|IDT|TSS|XSAVE|SMEP|SMAP|KPTI)\b" v2/kernel v2/lib --include=*.rs | wc -l
       0
$ grep -rn "cfg(target_arch)" v2/kernel/vifast-core/src v2/kernel/vifast-plat-common/src v2/lib/vifast-pte/src | wc -l
       0
```

Trait dùng chung và hai impl hiện có (chưa có impl x86):

```text scope=kt169-audit.log
$ grep -n "pub trait PageTableFormat\|pub trait Mmu" v2/lib/vifast-pte/src/mmu.rs
253:pub trait PageTableFormat {
297:pub trait Mmu {
$ grep -n "impl PageTableFormat" v2/kernel/vifast-plat-common/src/mmu.rs v2/kernel/vifast-plat-aarch64/src/format.rs
v2/kernel/vifast-plat-common/src/mmu.rs:481:impl PageTableFormat for Sv39 {
v2/kernel/vifast-plat-aarch64/src/format.rs:51://! §358/§1101). Tệp này **chỉ còn impl** ([`Aarch64`] + `impl PageTableFormat for Aarch64`),
v2/kernel/vifast-plat-aarch64/src/format.rs:247:impl PageTableFormat for Aarch64 {
```

Trait chưa có thao tác copy nửa cao; object layer VSpace vẫn import thẳng hình Sv39:

```text scope=kt169-audit.log
$ grep -n "copy_high\|high_half" v2/lib/vifast-pte/src/mmu.rs | wc -l
       0
$ grep -n "walk_shape\|PTE_LEVELS" v2/kernel/vifast-plat-common/src/obj/vspace.rs | head -2
7:use vifast_pte::{walk_shape, PAGE_SIZE_BITS, PTE_LEVELS};
73:        let mut reserved = [OBJ_NONE; PTE_LEVELS - 1];
```

ABI x86_64 còn `reserved`; harness QEMU chỉ có riscv64; linker chỉ có riscv:

```text scope=kt169-audit.log
$ sed -n "66,68p" spec/abi.toml
[calling.x86_64]
reserved = true
note = "Mở ở K5' theo D2-10; chỉ giữ số, không đóng băng chữ ký"
$ grep -n "qemu-system-riscv64" v2/test/boot/run.sh | head -1
44:QEMU="$(command -v qemu-system-riscv64 || true)"
$ grep -n "OUTPUT_ARCH" v2/kernel/vifast-kbin-riscv64/linker.ld
5:OUTPUT_ARCH(riscv)
```

Toolchain đã có target x86_64; v1 có tham khảo (đọc-only):

```text scope=kt169-audit.log
$ grep -n "x86_64-unknown-none" v2/rust-toolchain.toml
12:    "x86_64-unknown-none",
$ grep -n "int 0x80" crates/vifast-kernel-x86_64-uefi/src/user.S | head -1
34:    int 0x80
$ grep -n "enable_pcid\|configure_lapic\|reload_cr3" crates/vifast-platform-x86_64/src/platform.rs | head -3
759:unsafe fn enable_pcid() {
964:    unsafe { reload_cr3() };
1219:unsafe fn configure_lapic() -> u32 {
```

Cổng số liệu (chạy trên chính cây có báo cáo này):

```text scope=kt169-verify.log
# verify-numbers: ok=5 fail=0 wall_ms=18075 law_wall_ms={1:1119,2:2987,3:104,7:43,8:13576}
```

Cổng `check` (chạy trên chính cây có báo cáo này):

```text scope=kt169-check.log
v2 check: PASS (L1 chạy trong gate --fast)
```

## 7. Việc audit này **không** làm

- **Không** viết mã sản phẩm, **không** tạo crate `vifast-plat-x86_64`, **không** sửa `spec/`,
  **không** sửa `docs/v2/ViFastOs-v2-thiet-ke.md`, **không** sửa `docs/v1/`.
- **Không** tuyên bố K5' đạt, không tuyên bố "sẵn sàng thi công ngay": bước M1 (chốt phạm vi +
  đường boot) và việc Agent 0 mở `[calling.x86_64]` là **điều kiện tiên quyết**.
- **Không** đo gì trên QEMU x86_64 (task không cần QEMU); mọi số trong báo cáo là **số thiết kế
  `[TK]`** hoặc trích từ log đã commit trong khối `scope=`.
- **Không** đánh giá chi tiết v1: v1 chỉ được trích làm **tham khảo**, trạng thái đóng băng
  (`AGENTS.md` §5) không đổi.

## 8. Đề nghị cho lead

- **Duyệt M1 trước** (nửa giờ): chốt phạm vi K5' + đường boot + người mở `[calling.x86_64]`.
- **Mở task M2 ngay sau M1** (ba giờ): crate dữ liệu `vifast-plat-x86_64` — đây là bước **không
  cần QEMU**, đo được bằng `cargo test`, đúng khuôn K4' bước một (D2-147).
- **Ghi nợ có tên** cho M16–M18 (ASID/PCID, object layer VSpace, `Platform` trait) — nếu không
  ghi trước, chúng sẽ thành "sửa trait giữa chừng" ở M4/M7.
- **Không** gộp K5' với UEFI/VT-x: giữ UEFI là M9 và VT-x là H1, đúng D2-10 và
  `docs/v2/ViFastOs-v2-thiet-ke.md:882`.
