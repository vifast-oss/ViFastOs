<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-414 — UEFI boot `x86_64` (OVMF) + `BootSource::Uefi` (`BootInfo` v1, D2-260)

Nhánh `task/kt414-uefi`, base `16a03de2` (merge `main`), commit `741fda11` (merge). Agent `kt414`,
worktree `/private/tmp/vifast-wt/2`, `CARGO_TARGET_DIR=…/target-task/slot-2` (cache ấm). Ngân sách
3 h (W4, D2-259/D2-260). Phần `aarch64` tách sang **KT-414b** (không làm trong task này).

## 1. Vì sao

`task/kt414-uefi` (`7974bde1`) đã có boot stub UEFI x86_64 chạy được dưới OVMF (`GetMemoryMap` →
kiểm fail-closed → `ExitBootServices`; 1 ca xanh + 7 ca âm + 2 đỏ-trước), nhưng **chưa** nối vào
kernel: `BootInfo` v1 chỉ có `BootSource::{Dtb, Acpi}`, nên stub phải in dòng "nháp"
`source=uefi-memmap` và bản ghi bàn giao thiếu địa chỉ RSDP/DTB. **D2-260 mục 4** duyệt: thêm
`BootSource::Uefi` (`name() => "uefi"`) **giữ nguyên `BOOTINFO_VERSION = 1`**, `validate()`
fail-closed **theo nguồn**, bổ sung `rsdp_pa`/`dtb_pa` trong `UefiHandoffV1` + ghi bất biến
reserved/copy trang bàn giao, và nối `--firmware uefi` cho `x86_64` (0 đổi `-kernel`).

## 2. Đã làm

### 2.1 `v2/kernel/vifast-plat-common/src/bootinfo.rs` — `BootSource::Uefi`

- Thêm biến thể `Uefi` + `name() => "uefi"`; `BOOTINFO_VERSION` **giữ 1** (thêm biến thể enum
  không đổi ngữ nghĩa trường nào của struct).
- `validate()` **fail-closed theo nguồn**: `Dtb`/`Acpi` giữ nguyên 100 % điều kiện cũ
  (`n_harts`, `boot_hart` ∈ harts, PLIC/GIC/APIC theo arch, `uart`, `pcie_ecam`); `Uefi` chỉ đòi
  `n_ram > 0` + **mọi** vùng RAM hợp lệ (`size > 0`, không tràn địa chỉ) — ACPI/DTB chưa parse ở
  bước stub `GetMemoryMap`.
- Doc module ghi rõ nguồn thứ ba (D2-260).

### 2.2 `v2/boot/uefi-x86_64/` — bản ghi bàn giao + `BootInfo` thật

- `src/handoff.rs`: `UefiHandoffV1` thêm `rsdp_pa`/`dtb_pa` (0 = firmware không khai) ⇒ 336 → **352 B**;
  `verify()` fail-closed thêm: `arch`/`source`, `n_ram > 0` (`missing-memory`), từng vùng RAM
  (`bad-region`), căn RSDP 16 / DTB 8 (`bad-rsdp`/`bad-dtb`); `reserved_pages(pa)` +
  `HANDOFF_RESERVED_INVARIANT` ghi **bất biến reserved/copy** trang bàn giao khi nhảy `e_entry`.
- `src/efi.rs`: GUID ACPI 2.0/1.0/DTB + `find_config_table` (hàm thuần, test host khoá).
- `src/console.rs`: `line_bytes` + `ConOutWriter` (`fmt::Write` → `ConOut`) để in
  `BootInfo::write_log` **nguyên văn** (không chép chuỗi định dạng ở boot stub).
- `src/lib.rs`: `bootinfo_from_memmap` — dựng `BootInfo` nguồn `Uefi` (dùng chung ảnh + test host).
- `src/main.rs`: đọc `EFI_CONFIGURATION_TABLE` → `rsdp_pa`/`dtb_pa`; dựng `BootInfo` +
  `validate()` (fail-closed `missing-memory` nếu không còn RAM usable) rồi in `# bootinfo:` bằng
  `write_log` của kernel; bản ghi bàn giao in thêm `rsdp_pa`/`dtb_pa` + dòng
  `# uefi: handoff reserved … invariant=kernel-must-reserve-or-copy`.
- `src/memmap.rs` + `build.rs`: thêm ca tiêm lỗi `no-usable` (mọi mô tả usable → reserved: bảng
  nhớ **hợp lệ theo UEFI** nhưng `BootInfo::validate` phải fail-closed `missing-memory`) +
  `UefiError::BootInfo(&'static str)`.
- `run.sh`: kỳ vọng ca xanh thêm `# bootinfo: v=1 arch=x86_64 source=uefi …`, `cfgtable`,
  `handoff reserved`; thêm ca âm `no-usable` + đỏ-trước `naive-no-usable` (1 xanh + 8 âm + 3 đỏ-trước).
- `tests/vifast_boot_uefi.rs`: 18 → **23 test** (Uefi fail-closed theo nguồn, Dtb/Acpi vẫn chặt,
  GUID bảng cấu hình, trường bàn giao mới, `no-usable`).
- `README.md`: cập nhật §1–§5 theo số đo thật.

### 2.3 `v2/test/boot/run-x86_64.sh` — `--firmware uefi` (0 đổi `-kernel`)

Thêm nhánh `--firmware uefi` (mặc định **không đổi**): chạy bộ ca UEFI rồi kiểm trên log thật
`# bootinfo: … source=uefi`, `rsdp_pa`/`dtb_pa`, bất biến reserved/copy, ca âm `no-usable`
fail-closed. Lead nối cờ vào `v2/bin/**` (D2-253/254 — sub-agent không sửa `v2/bin/`).

## 3. Bằng chứng

### 3.1 UEFI selftest (QEMU/OVMF, TCG) — `bash run.sh --all-cases` rc=0

```
PASS positive (rc=33): memmap=ok, handoff verify=ok, ExitBootServices OK, after-ebs=ok
PASS neg-desc-size (rc=65): fail-closed err=map-desc-size, KHÔNG ExitBootServices
PASS neg-desc-version (rc=65): fail-closed err=map-desc-version, KHÔNG ExitBootServices
PASS neg-map-size (rc=65): fail-closed err=map-size, KHÔNG ExitBootServices
PASS neg-map-empty (rc=65): fail-closed err=map-empty, KHÔNG ExitBootServices
PASS neg-bad-type (rc=65): fail-closed err=map-bad-type, KHÔNG ExitBootServices
PASS neg-bad-page (rc=65): fail-closed err=map-bad-region, KHÔNG ExitBootServices
PASS neg-bad-attr (rc=65): fail-closed err=map-bad-attribute, KHÔNG ExitBootServices
PASS neg-no-usable (rc=65): fail-closed err=missing-memory, KHÔNG ExitBootServices
RED-PROOF naive-desc-size: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
RED-PROOF naive-bad-attr: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
RED-PROOF naive-no-usable: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
SELFTEST: XANH (1 xanh + 8 âm + 3 đỏ-trước)
```

Bằng chứng `BootSource::Uefi` trên log **thật** (`logs/positive/serial.log`, rc=33):

```
# uefi: cfgtable n=10 rsdp_pa=0xf77e000 dtb_pa=0x0
# bootinfo: v=1 arch=x86_64 source=uefi n_harts=1 boot_hart=0
# bootinfo: ram n=8 bytes=0xf9c3000
# bootinfo: harts=0 timebase=none
# bootinfo: plic=none gic=none gic_cpu=none apic=none uart=none ecam=none
# bootinfo: acpi=no pm1a=none
# uefi: handoff magic=0x3148545341464956 version=1 va=0xde1f000 size=352 n_ram=8 rsdp_pa=0xf77e000 dtb_pa=0x0 checksum=0x00001207
# uefi: handoff verify=ok checksum=ok
# uefi: handoff reserved pa=0xde1f000 bytes=352 pages=1 invariant=kernel-must-reserve-or-copy
```

Ca âm **bảng nhớ hỏng** (`logs/neg-no-usable/serial.log`, rc=65) — bảng nhớ **hợp lệ theo UEFI**
nhưng `BootInfo::validate` fail-closed:

```
# uefi: fault=no-usable injected=1 (bang nho bi lam hong co chu dich - ca am)
# uefi: memmap=ok count=100 desc_size=48 desc_version=1 map_size=0x12c0 sum=0x7f7c
# uefi: memmap: n_usable=0 usable_bytes=0x0 highest_phys=0x10000000000 … n_reserved=88 …
# uefi: fail-closed err=missing-memory detail=- code=0x20
# uefi: memmap=FAIL err=missing-memory exitbootservices=not-called
# uefi: exit=0x20 (QEMU isa-debug-exit ⇒ rc 65)
```

### 3.2 T0 — `./v2/bin/v2 t0` (rc=1 — **chỉ** vì 13 lỗi clippy có sẵn của `vifast-core`)

- `vifast-boot-uefi-x86_64`: clippy `-D warnings` **PASS**, `cargo test` **23/23 PASS**.
- `vifast-plat-common`: `cargo test` **PASS** (32 test `bootinfo.rs` + toàn bộ suite khác);
  clippy `-D warnings` **ĐỎ do 13 lỗi có sẵn trong `vifast-core`** (D2-260 đã ghi nhận, dọn ở
  **KT-405**) — không phải mã của task. `cargo clippy --no-deps -p vifast-plat-common` **PASS**.
- `v2 t0` tổng: **FAIL** vì lỗi có sẵn đó (xem §4).

### 3.3 `--firmware uefi` (đường boot thứ ba, 0 đổi `-kernel`) — rc=0

```
# firmware=uefi (OVMF) — boot stub v2/boot/uefi-x86_64; đường -kernel KHÔNG đổi
… SELFTEST: XANH (1 xanh + 8 âm + 3 đỏ-trước)
ok 1 - firmware uefi: 1 xanh + 8 âm + 3 đỏ-trước (log …/uefi-selftest.log)
ok 2 - firmware uefi: '# bootinfo: … source=uefi' + rsdp_pa/dtb_pa + reserved/copy + ca âm fail-closed
```

### 3.4 T1 — `./v2/bin/v2 gate --dev` (rc=1 — **chỉ** 1 bước quy trình)

- `v2 check: PASS` (HEAD `741fda11`, 44 bước, **110,8 s**).
- Lượt `gate --dev` đầy đủ (sau khi T2 của Lead nhả khoá máy đo lúc 13:46, chạy 13:51–14:02,
  **657,8 s**): **ĐỎ đúng 1 bước** `selftest pre-measure (KT-95)` rc=7 `tree-dirty` — worktree
  còn tệp chưa commit (quy trình, không phải lỗi mã; Lead chạy T1 trên cây sạch sau commit).
  **Mọi bước khác OK**, gồm `QEMU x86_64 (K5' bước 1)` (`# 37/37 kiểm sạch (boot=1, tests=47,
  exc=6)`) và `QEMU aarch64 (K4' bước 3)`.
- Ghi chú môi trường: lượt đầu (13:36) bị `job_kill` theo chỉ đạo Lead khi T2 giữ khoá; lượt
  chạy lại **không** export `CARGO_TARGET_DIR` — các runner QEMU (`run-x86_64.sh`,
  `run-aarch64.sh`) hardcode `v2/kernel/target/…` nên nếu export biến đó thì ảnh build ra chỗ
  khác và bước QEMU đỏ oan "không thấy ảnh" (đã gặp và xác định là môi trường, không phải mã).
- T1 trên **cây sạch** do Lead chạy sau commit.

## 4. Còn nợ

- **Nhảy `e_entry`** (nạp ELF kernel từ ESP + truyền con trỏ bản ghi + giữ trang bàn giao
  reserved/copy) — kernel chưa mở API nhận bản ghi; bất biến đã ghi ở `handoff.rs`/README §4.1.
- **Parse ACPI** (`RSDP` → `MADT`/`FADT`/`MCFG`) để điền `apic`/`uart`/`pcie_ecam` — `rsdp_pa` đã
  có trong bản ghi nên không phải dò.
- **aarch64** → **KT-414b** (target `aarch64-unknown-uefi` chưa cài; không cài mạng trong task).
- `RUSTUP_TOOLCHAIN=stable` cho ảnh `.efi` (toolchain ghim `1.100.0` chưa có component
  `x86_64-unknown-uefi`) — nợ hạ tầng của KT-414b.
- `vifast-core` có **13 lỗi clippy `-D warnings` có sẵn** (D2-260 đã ghi nhận, dọn ở **KT-405**)
  ⇒ `v2 t0` báo đỏ ở bước clippy `vifast-plat-common` dù mã của task sạch.
