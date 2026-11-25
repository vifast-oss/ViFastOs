# `v2/boot/uefi-x86_64` — boot stub UEFI (KT-414, P1)

Ứng dụng UEFI `PE/COFF` tối giản (`efi_main`) cho **x86_64**: chụp `GetMemoryMap` **ngay trước**
`ExitBootServices`, **kiểm bảng nhớ fail-closed có tên**, in bằng chứng `# uefi: key=value` ra
`ConOut`, dựng bản ghi bàn giao `BootInfo` v1 (nháp), rồi thoát dịch vụ boot và báo kết quả cho
runner bằng mã thoát QEMU. **Không** đụng đường `-kernel` hiện có: đây là nhánh boot **thứ ba**,
chọn tường minh khi chạy (`run.sh`), không có mặc định nào đổi.

Đặc tả nền: [`v2/docs/u1-uefi-readiness.md` §4](../../docs/u1-uefi-readiness.md) (KT-364), kiểu
`BootInfo` v1: [`v2/kernel/vifast-plat-common/src/bootinfo.rs`](../../kernel/vifast-plat-common/src/bootinfo.rs) (KT-402).

## 1. Chạy

```bash
cd v2/boot/uefi-x86_64
bash run.sh --all-cases    # 1 ca xanh + 7 ca âm + 2 chứng minh đỏ-trước  (~3–4 phút, TCG)
bash run.sh --positive     # chỉ ca xanh
bash run.sh --fault bad-page   # một ca âm; tên: desc-size desc-version map-size map-empty
                               #                 bad-type bad-page bad-attr
bash run.sh --naive-fault desc-size   # bản CHƯA VÁ + bảng nhớ hỏng ⇒ vẫn xanh (đỏ-trước)
```

Kết quả: `logs/<tag>/{serial.log,debug.log,qemu.out,rc,vars.fd}`, `run.sh` trả 0 ⇔ mọi kỳ vọng
đúng. Test host (không cần QEMU): `RUSTUP_TOOLCHAIN=stable cargo test` → 18 test.

Môi trường đo trên máy này (Apple Silicon, 12 core): QEMU 11.1.3 (x86_64 **TCG** — bản
`qemu-system-x86_64` của Homebrew không bật HVF), OVMF
`/opt/homebrew/share/qemu/edk2-x86_64-code.fd` (rev `0x00020046`) + `edk2-i386-vars.fd`
(bản sao ghi được mỗi lần chạy). Không cần mạng, không cần root, không cần `mtools`.

### Vì sao có `startup.nsh`

OVMF của QEMU 11 trên máy này vào thẳng **`EFI Internal Shell`** (không tự chạy fallback
`\EFI\BOOT\BOOTX64.EFI` của removable media — đo được: BDS chỉ nạp `Boot0001 "EFI Internal
Shell"`). Vì vậy ESP ảo (`-drive file=fat:rw:…`) chứa thêm `startup.nsh` = `\EFI\BOOT\BOOTX64.EFI`
và Internal Shell tự chạy nó ở `FS0:`. Ghi rõ ở đây để người sau không tưởng là app tự boot.

### Vì sao `RUSTUP_TOOLCHAIN=stable`

Toolchain ghim của dòng v2 là `1.100.0`, nhưng toolchain **cài thêm** tên `1.100.0` trên máy này
**không có** component `x86_64-unknown-uefi` (rust-std); toolchain `stable` thì **có**, và
`rustc +stable --version` = `rustc 1.100.0 (0ef1bc386 2026-11-09)` — **cùng phiên bản, cùng
commit** với ghim. Script **không** cài gì từ mạng (đúng ràng buộc task). **Nợ**: khi máy có
`rustup target add --toolchain 1.100.0 x86_64-unknown-uefi` (cần mạng) thì bỏ biến
`VIFAST_UEFI_TOOLCHAIN`/mặc định `stable` trong `run.sh`; ghi vào báo cáo KT-414.

## 2. Kết quả đo (bằng chứng thật, không viết tay số)

### 2.1 Ca xanh — `logs/positive/` (rc = **33** = `(0x10<<1)|1`)

```
# uefi: app=vifast-boot-x86_64 v=1 target=x86_64-unknown-uefi
# uefi: firmware rev=0x00020046 tables=10 naive=0 fault=none
# uefi: memmap=query desc_size=48 desc_version=1 map_size=0x1260 key=0x…
# uefi: memmap=ok count=99 desc_size=48 desc_version=1 map_size=0x1290 sum=0x8023
# uefi: memmap: n_usable=83 usable_bytes=0xf9c3000 highest_phys=0x10000000000 attr_odd=97 …
# bootinfo: v=1 arch=x86_64 source=uefi-memmap
# bootinfo: ram n=8 bytes=0xf9c3000
# bootinfo: ram[0]=0x0+0xa0000  …
# uefi: handoff magic=0x3148545341464956 version=1 va=0xde23000 size=336 n_ram=8 checksum=0x00000fcb
# uefi: handoff verify=ok checksum=ok
# uefi: exitbootservices=attempt try=0 previous_key=0x…
```

`debug.log` (cổng QEMU `0xE9`, **chỉ** ghi được sau `ExitBootServices` — chứng minh đã thoát thật):

```
# uefi: exitbootservices=ok
# uefi: after-ebs=ok
```

Ghi chú đo được trên OVMF này: `DescriptorSize = 48` (không phải 40 — firmware thêm 8 B đệm mỗi
mô tả; bên đọc **luôn** dùng stride), `Attribute = 0xF` cho mọi mô tả (trái khuyến nghị "đúng một
bit cacheability" của UEFI 2.10 §7.2) ⇒ phép kiểm fail-closed chỉ đòi **≥ 1 bit** với vùng usable,
và đếm `attr_odd` để in bằng chứng. Nếu siết "đúng một bit" sẽ **false-red** trên firmware thật.

### 2.2 Ca âm — `logs/neg-<fault>/` (rc = **65** = `(0x20<<1)|1`)

| Tiêm lỗi | Trường bị phá | Lỗi có tên in ra |
|---|---|---|
| `desc-size` | `DescriptorSize = 39` | `map-desc-size` |
| `desc-version` | `DescriptorVersion = 2` | `map-desc-version` |
| `map-size` | `MapSize % DescriptorSize = 1` | `map-size` |
| `map-empty` | `MapSize = 0` | `map-empty` |
| `bad-type` | `Type = 0x11` (> `EfiMaxMemoryType`) | `map-bad-type` |
| `bad-page` | `NumberOfPages = 0` | `map-bad-region` |
| `bad-attr` | `Attribute = 0` trên vùng usable đầu | `map-bad-attribute` |

Mỗi ca khẳng định thêm: **không** có `handoff verify=ok`, **không** gọi `ExitBootServices`,
`debug.log` có `fail-closed`. Ví dụ:

```
# uefi: fail-closed err=map-bad-region detail=- code=0x20
# uefi: memmap=FAIL err=map-bad-region exitbootservices=not-called
# uefi: exit=0x20 (QEMU isa-debug-exit ⇒ rc 65)
```

### 2.3 Đỏ-trước — `logs/naive-<fault>/` (rc = **33**)

`VIFAST_UEFI_NAIVE=1` tái hiện **hành vi trước khi vá**: bỏ toàn bộ phép kiểm, vẫn
`ExitBootServices`. Với cùng bảng nhớ hỏng, bản naive **xanh** (và `after-ebs=ok`) ⇒ ca âm ở
§2.2 thật sự bắt được lỗi, không phải "xanh giả". Đây là chứng minh đỏ-theo-mutation giữ được
trong harness (chạy lại được mọi lúc), thay cho việc commit một bản hỏng.

### 2.4 Tổng hợp `bash run.sh --all-cases` (đầu ra cuối, `run.sh` rc = 0)

```
PASS positive (rc=33): memmap=ok, handoff verify=ok, ExitBootServices OK, after-ebs=ok
PASS neg-desc-size (rc=65): fail-closed err=map-desc-size, KHÔNG ExitBootServices
PASS neg-desc-version (rc=65): fail-closed err=map-desc-version, KHÔNG ExitBootServices
PASS neg-map-size (rc=65): fail-closed err=map-size, KHÔNG ExitBootServices
PASS neg-map-empty (rc=65): fail-closed err=map-empty, KHÔNG ExitBootServices
PASS neg-bad-type (rc=65): fail-closed err=map-bad-type, KHÔNG ExitBootServices
PASS neg-bad-page (rc=65): fail-closed err=map-bad-region, KHÔNG ExitBootServices
PASS neg-bad-attr (rc=65): fail-closed err=map-bad-attribute, KHÔNG ExitBootServices
RED-PROOF naive-desc-size: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
RED-PROOF naive-bad-attr: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
SELFTEST: XANH (1 xanh + 7 âm + 2 đỏ-trước)
```

## 3. Cấu trúc

| Tệp | Nội dung |
|---|---|
| `src/main.rs` | `efi_main`; thứ tự: banner → `GetMemoryMap` → tiêm lỗi (harness) → kiểm → in bằng chứng → bàn giao → làm mới key → `ExitBootServices` |
| `src/memmap.rs` | kiểm bảng nhớ **fail-closed có tên** + thống kê + tiêm lỗi; không panic, không đọc ngoài biên |
| `src/handoff.rs` | bản ghi `BootInfo` v1 nháp (`repr(C)`, magic/version/checksum) |
| `src/efi.rs` | định nghĩa `repr(C)` UEFI tối thiểu (ABI khoá bằng test host) |
| `src/console.rs` | `ConOut->OutputString` (trước EBS), cổng debug QEMU `0xE9` (sau EBS), `isa-debug-exit` `0x501` |
| `src/lib.rs` | `#![no_std]`; include **nguyên văn** `bootinfo.rs` của kernel qua `#[path]` |
| `tests/vifast_boot_uefi.rs` | 18 test host: offset ABI, tên lỗi kernel, toàn bộ ca âm của `validate`, checksum bàn giao |
| `build.rs` | danh sách đóng `VIFAST_UEFI_FAULT` / `VIFAST_UEFI_NAIVE` (sai tên ⇒ build đỏ) |

`bootinfo` **không** phải bản sao: `#[path = "../../../kernel/vifast-plat-common/src/bootinfo.rs"]`
+ shim `crate::smp::MAX_HARTS = 8`; test `bootinfo_is_included_from_kernel_not_copied` đọc
`include_str!` của `smp.rs` nên lệch số hart là đỏ.

## 4. Bàn giao `BootInfo` v1 — hiện trạng và việc còn lại

Bản ghi `UefiHandoffV1` (336 B, `repr(C)`, magic `"VIFASTH1"`, checksum cộng dồn):

| Trường | Giá trị |
|---|---|
| `magic`, `version` | `0x3148545341464956` (`"VIFASTH1"`), `1` |
| `arch`, `source` | `3` (x86_64), `2` (UEFI memmap — **chưa** có trong `BootSource` của v1) |
| `size_bytes`, `checksum` | 336, checksum cộng dồn byte (trường checksum = 0 khi tính) |
| `desc_size`, `desc_version`, `memmap_count`, `memmap_key` | 48, 1, 99, key của lần `GetMemoryMap` cuối |
| `highest_phys`, `usable_bytes`, `memmap_sum` | địa chỉ cao nhất, tổng RAM usable, checksum mảng thô |
| `ram[16]` | 8 vùng usable đã gộp vùng kề (dùng `RamRegion`/`MAX_USABLE_RAM` của kernel) |

Địa chỉ/kích thước in ở `# uefi: handoff … va=0x… size=336` (PE của OVMF nạp identity-map nên
VA = PA tại thời điểm này). **Cách kernel đọc (khi nối dây)**: loader truyền con trỏ bản ghi
(x86_64: `rdi`; aarch64: `x0`) trước khi nhảy `e_entry`; kernel kiểm `magic`/`version`/
`size_bytes`/`checksum` rồi fail-closed bằng `BootInfoError::{BadMagic, UnsupportedVersion,
BadChecksum}`; `n_ram`/`ram[]` dùng trực tiếp.

**Việc còn lại (chưa làm, cần quyết định/duyệt — không thuộc quyền task này):**

1. `BootSource::Uefi` (hoặc `BootInfo` v2): thêm nguồn thứ ba vào `BootInfo` v1 là **đổi ngữ
   nghĩa** ⇒ cần A0/Lead duyệt (D2-04). Hiện ghi `source=uefi-memmap` ở dòng `# bootinfo:`
   **nháp**, không giả `source=acpi`.
2. Parse ACPI (`RSDP` trong configuration table → `MADT`/`FADT`/`MCFG`) để điền
   `apic/uart/pcie_ecam` nếu muốn dùng **nguyên** `BootInfo::validate()`.
3. Trang chứa bản ghi: ảnh `.efi` nằm trong vùng `EfiLoaderCode/Data` — sau `ExitBootServices`
   vùng đó được nhả. Trước khi nhảy kernel phải đánh dấu trang bản ghi là reserved trong bảng
   nhớ bàn giao (hoặc kernel chép ra trang riêng trước khi dùng bộ nhớ).
4. Nhảy `e_entry` + chọn kernel theo `--firmware uefi` (đường `-kernel` giữ nguyên byte).
5. **aarch64 (`aarch64-unknown-uefi` + AAVMF)**: target rust-std **chưa cài** trên máy này
   (`rustup target list --installed` không có `aarch64-unknown-uefi`); theo ràng buộc "không cài
   mạng" nên **không làm** trong task này. Thư viện đã tách `efi/memmap/handoff/console` (arch
   trung lập trừ 2 hàm asm `debugcon_byte`/`qemu_exit_port` có `cfg(target_arch)`), dựng crate
   `uefi-aarch64` chỉ cần đổi `ARCH_*`, cổng debug và `exit_val` (AAVMF không có `isa-debug-exit`
   ⇒ cần `-semihosting` hoặc `PSCI SYSTEM_OFF` sau EBS).

## 5. Tầng kiểm của task

| Tầng | Lệnh | Kết quả |
|---|---|---|
| T0 | `./v2/bin/v2 t0` (fmt + clippy `-D warnings` + `cargo test` trên crate bị chạm) | **PASS** (`v2 t0: PASS (1 crate…)`, 18 test) — 11 s |
| T0 (đỏ-trước) | `bash run.sh --all-cases` | **XANH** 1+7+2 ca — ~3,5 phút (OVMF/TCG) |
| T1 | `./v2/bin/v2 gate --dev` | **2 lượt, cả 2 chỉ đỏ vì quy trình/harness — không phải mã của crate này** (Lead chốt): lượt 1 (08:57) đỏ `check-selftest-wiring (KT-134)` vì `run.sh` có cờ `--selftest` mà không cổng nào gọi ⇒ **đã sửa bằng (a)**: bỏ hẳn cờ đó (đổi `--all-cases`, hàm `run_all_cases`), checker PASS (`40 script tự kiểm: 40 có cổng, 0 mồ côi`); lượt 2 (09:10–09:21) `v2 check: PASS`, mọi bước xanh, **chỉ** `selftest pre-measure (KT-95)` đỏ rc=7 `tree-dirty` — vì worktree của worker còn tệp chưa commit (quy trình, không phải lỗi mã; cây sạch của Lead sẽ xanh), cộng 1 dòng TAP `not ok` của `selftest netsrv` có sẵn từ trước. Lượt 3 bị Lead dừng để giữ máy sạch; T1 trên cây sạch do Lead chạy. |
| T2 | `./v2/bin/v2 gate --fast` | **Chưa chạy** — Lead chạy trên ứng viên tích hợp (thay đổi chạm `boot/`, không chạm kernel/ABI nên T2 không bắt buộc ngay theo §6.1). |

R3 (SPDX) / R5 (unsafe chỉ trong allowlist `boot/`) đã xanh qua `./v2/bin/v2 rules` (PASS, E1 PASS
1.693 tên tracked). Ảnh `.efi` build **không** có mạng; `run.sh` không đổi `v2/`, `kernel/`,
`plat-*`, `run-common.sh`, `v2/bin/v2`.

## 6. Ghi chú kỹ thuật đáng nhớ (đã trả giá trong task)

- **`ConOut->OutputString` cấp phát pool bên trong firmware** ⇒ in giữa `GetMemoryMap` và
  `ExitBootServices` làm `MapKey` cũ ngay lập tức (đo được: key tăng `0x10` mỗi vòng,
  EBS trả `EFI_INVALID_PARAMETER` 8/8). Sửa: **không in gì** giữa lần `GetMemoryMap` cuối và
  `ExitBootServices`; mọi dòng log ở trước.
- `isa-debug-exit` của QEMU 11 trên `q35`: dùng `iobase` **mặc định `0x501`**; `0x604` **không**
  thoát (đo được). Mã thoát `(val<<1)|1`: xanh `0x10` ⇒ rc 33, lỗi bảng nhớ `0x20` ⇒ 65, lỗi EBS
  `0x21` ⇒ 67.
- `-drive file=fat:ro:…` bị QEMU từ chối (`Block node is read-only`); dùng `fat:rw` (QEMU ghi ra
  overlay qcow tạm, không đụng thư mục nguồn).
- QEMU x86_64 trên máy này không có HVF ⇒ TCG; một lượt boot OVMF ~20 s (kể cả 5 s đếm ngược của
  Internal Shell). `run.sh` có watchdog `VIFAST_UEFI_TIMEOUT` (mặc định 180 s); rc=124 **không**
  phải kết quả hợp lệ.
