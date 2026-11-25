# `v2/boot/uefi-x86_64` — boot stub UEFI (KT-414, P1)

Ứng dụng UEFI `PE/COFF` tối giản (`efi_main`) cho **x86_64**: chụp `GetMemoryMap` **ngay trước**
`ExitBootServices`, **kiểm bảng nhớ fail-closed có tên**, dựng **`BootInfo` v1 nguồn `Uefi`**
(D2-260) + bản ghi bàn giao `UefiHandoffV1` (`rsdp_pa`/`dtb_pa`, bất biến reserved/copy), in bằng
chứng `# uefi:`/`# bootinfo:` ra `ConOut`, rồi thoát dịch vụ boot và báo kết quả cho runner bằng
mã thoát QEMU. **Không** đụng đường `-kernel` hiện có: đây là nhánh boot **thứ ba**, chọn tường
minh khi chạy (`run.sh` / `run-x86_64.sh --firmware uefi`), không có mặc định nào đổi.

Đặc tả nền: [`v2/docs/u1-uefi-readiness.md` §4](../../docs/u1-uefi-readiness.md) (KT-364), kiểu
`BootInfo` v1: [`v2/kernel/vifast-plat-common/src/bootinfo.rs`](../../kernel/vifast-plat-common/src/bootinfo.rs) (KT-402).

## 1. Chạy

```bash
cd v2/boot/uefi-x86_64
bash run.sh --all-cases    # 1 ca xanh + 8 ca âm + 3 chứng minh đỏ-trước  (~4–5 phút, TCG)
bash run.sh --positive     # chỉ ca xanh
bash run.sh --fault bad-page   # một ca âm; tên: desc-size desc-version map-size map-empty
                               #                 bad-type bad-page bad-attr no-usable
bash run.sh --naive-fault desc-size   # bản CHƯA VÁ + bảng nhớ hỏng ⇒ vẫn xanh (đỏ-trước)
```

Đường boot **thứ ba** (không đổi `-kernel`): `bash v2/test/boot/run-x86_64.sh --firmware uefi`
chạy bộ ca trên rồi kiểm thêm trên log thật `# bootinfo: v=1 arch=x86_64 source=uefi …`,
`rsdp_pa`/`dtb_pa` + bất biến reserved/copy, và ca âm `no-usable` fail-closed. Cờ do Lead nối vào
`v2/bin/**` (D2-253/254).

Kết quả: `logs/<tag>/{serial.log,debug.log,qemu.out,rc,vars.fd}`, `run.sh` trả 0 ⇔ mọi kỳ vọng
đúng. Test host (không cần QEMU): `RUSTUP_TOOLCHAIN=stable cargo test` → 23 test.

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
# uefi: memmap=query desc_size=48 desc_version=1 map_size=0x12c0 key=0x4a0
# uefi: memmap=ok count=100 desc_size=48 desc_version=1 map_size=0x12c0 sum=0x80e8
# uefi: memmap: n_usable=84 usable_bytes=0xf9c3000 highest_phys=0x10000000000 n_runtime=4 n_acpi=7 n_reserved=4 attr_odd=98 types=0xefb
# uefi: cfgtable n=10 rsdp_pa=0xf77e000 dtb_pa=0x0
# bootinfo: v=1 arch=x86_64 source=uefi n_harts=1 boot_hart=0
# bootinfo: ram n=8 bytes=0xf9c3000
# bootinfo: ram[0]=0x0+0xa0000  …
# bootinfo: harts=0 timebase=none
# bootinfo: plic=none gic=none gic_cpu=none apic=none uart=none ecam=none
# bootinfo: acpi=no pm1a=none
# uefi: handoff magic=0x3148545341464956 version=1 va=0xde1f000 size=352 n_ram=8 rsdp_pa=0xf77e000 dtb_pa=0x0 checksum=0x00001207
# uefi: handoff verify=ok checksum=ok
# uefi: handoff reserved pa=0xde1f000 bytes=352 pages=1 invariant=kernel-must-reserve-or-copy
# uefi: exitbootservices=attempt try=0 previous_key=0x4a0
```

`# bootinfo:` ở trên là **nguyên văn** `BootInfo::write_log` của kernel (`source=uefi` — D2-260),
không phải chuỗi chép ở boot stub: `bootinfo.rs` được `#[path]` include từ
`v2/kernel/vifast-plat-common/src/`. `rsdp_pa=0xf77e000` là **RSDP thật** lấy từ
`EFI_CONFIGURATION_TABLE` (GUID ACPI 2.0); `dtb_pa=0` vì `x86_64` không có DTB — **không** phải lỗi.

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
| `no-usable` | mọi mô tả usable → `Type = 0` (reserved) | `missing-memory` (**`BootInfo::validate`** theo nguồn `Uefi`) |

`no-usable` là ca âm **của tầng `BootInfo`** (D2-260): bảng nhớ vẫn **hợp lệ theo UEFI**
(`memmap::validate` xanh, `n_usable=0`) nhưng `BootInfo::validate()` nguồn `Uefi` fail-closed
`missing-memory` ⇒ kernel sẽ không có frame nào để cấp. Bảy ca kia hỏng **bảng nhớ**; ca này hỏng
**"không có RAM"** — lớp lỗi khác, không ca nào thay được ca nào.

Mỗi ca khẳng định thêm: **không** có `handoff verify=ok`, **không** gọi `ExitBootServices`,
`debug.log` có `fail-closed`. Ví dụ:

```
# uefi: fail-closed err=map-bad-region detail=- code=0x20
# uefi: memmap=FAIL err=map-bad-region exitbootservices=not-called
# uefi: exit=0x20 (QEMU isa-debug-exit ⇒ rc 65)
```

`neg-no-usable` (đo được, `logs/neg-no-usable/`):

```
# uefi: fault=no-usable injected=1 (bang nho bi lam hong co chu dich - ca am)
# uefi: memmap=ok count=100 desc_size=48 desc_version=1 map_size=0x12c0 sum=0x7f7c
# uefi: memmap: n_usable=0 usable_bytes=0x0 highest_phys=0x10000000000 … n_reserved=88 …
# uefi: cfgtable n=10 rsdp_pa=0xf77e000 dtb_pa=0x0
# uefi: fail-closed err=missing-memory detail=- code=0x20
# uefi: memmap=FAIL err=missing-memory exitbootservices=not-called
# uefi: exit=0x20 (QEMU isa-debug-exit ⇒ rc 65)
```

### 2.3 Đỏ-trước — `logs/naive-<fault>/` (rc = **33**)

`VIFAST_UEFI_NAIVE=1` tái hiện **hành vi trước khi vá**: bỏ toàn bộ phép kiểm, vẫn
`ExitBootServices`. Với cùng bảng nhớ hỏng, bản naive **xanh** (và `after-ebs=ok`) ⇒ ca âm ở
§2.2 thật sự bắt được lỗi, không phải "xanh giả". Đây là chứng minh đỏ-theo-mutation giữ được
trong harness (chạy lại được mọi lúc), thay cho việc commit một bản hỏng. Ba ca đại diện:
`desc-size` (hỏng siêu dữ liệu), `bad-attr` (hỏng thuộc tính), `no-usable` (hỏng tầng
`BootInfo` — bảng nhớ vẫn hợp lệ theo UEFI).

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
PASS neg-no-usable (rc=65): fail-closed err=missing-memory, KHÔNG ExitBootServices
RED-PROOF naive-desc-size: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
RED-PROOF naive-bad-attr: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
RED-PROOF naive-no-usable: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33)
SELFTEST: XANH (1 xanh + 8 âm + 3 đỏ-trước)
```

## 3. Cấu trúc

| Tệp | Nội dung |
|---|---|
| `src/main.rs` | `efi_main`; thứ tự: banner → `GetMemoryMap` → tiêm lỗi (harness) → kiểm → `EFI_CONFIGURATION_TABLE` (RSDP/DTB) → `BootInfo` v1 `source=uefi` → bàn giao → làm mới key → `ExitBootServices` |
| `src/memmap.rs` | kiểm bảng nhớ **fail-closed có tên** + thống kê + tiêm lỗi (8 ca, gồm `no-usable`); không panic, không đọc ngoài biên |
| `src/handoff.rs` | bản ghi `BootInfo` v1 (`repr(C)`, magic/version/checksum, `rsdp_pa`/`dtb_pa`, bất biến reserved/copy) |
| `src/efi.rs` | định nghĩa `repr(C)` UEFI tối thiểu + GUID ACPI 2.0/1.0/DTB + `find_config_table` (ABI khoá bằng test host) |
| `src/console.rs` | `ConOut->OutputString` (trước EBS), `ConOutWriter` (`fmt::Write` → `ConOut`), cổng debug QEMU `0xE9` (sau EBS), `isa-debug-exit` `0x501` |
| `src/lib.rs` | `#![no_std]`; include **nguyên văn** `bootinfo.rs` của kernel qua `#[path]`; `bootinfo_from_memmap` dùng chung ảnh + test |
| `tests/vifast_boot_uefi.rs` | 23 test host: offset ABI, tên lỗi kernel, `BootSource::Uefi` fail-closed theo nguồn, `Dtb`/`Acpi` vẫn chặt, GUID bảng cấu hình, checksum + trường bàn giao |
| `build.rs` | danh sách đóng `VIFAST_UEFI_FAULT` / `VIFAST_UEFI_NAIVE` (sai tên ⇒ build đỏ) |

`bootinfo` **không** phải bản sao: `#[path = "../../../kernel/vifast-plat-common/src/bootinfo.rs"]`
+ shim `crate::smp::MAX_HARTS = 8`; test `bootinfo_is_included_from_kernel_not_copied` đọc
`include_str!` của `smp.rs` nên lệch số hart là đỏ.

## 4. Bàn giao `BootInfo` v1 — `BootSource::Uefi` (D2-260)

**D2-260 đã duyệt** `BootSource::Uefi` trong `BootInfo` v1 (`BOOTINFO_VERSION` **giữ nguyên 1**,
không nâng v2). Boot stub dựng **thật** `BootInfo` từ bảng nhớ đã kiểm
(`vifast_boot_uefi::bootinfo_from_memmap`) và in `# bootinfo:` bằng **chính**
`BootInfo::write_log` của kernel — không còn dòng `source=uefi-memmap` "nháp".

`BootInfo::validate()` **fail-closed theo nguồn**:

| Nguồn | Điều kiện bắt buộc |
|---|---|
| `Dtb` / `Acpi` | **giữ nguyên 100 %** như trước: `n_ram > 0`, `n_harts > 0`, `boot_hart` ∈ harts, PLIC/GIC/APIC theo arch, `uart`, `pcie_ecam` |
| `Uefi` | `n_ram > 0` + **mọi** vùng RAM hợp lệ (`size > 0`, không tràn địa chỉ); `apic`/`gic`/`uart`/`pcie_ecam` được `None` (ACPI/DTB chưa parse ở bước stub) |

Bản ghi `UefiHandoffV1` (**352 B**, `repr(C)`, magic `"VIFASTH1"`, checksum cộng dồn):

| Trường | Giá trị |
|---|---|
| `magic`, `version` | `0x3148545341464956` (`"VIFASTH1"`), `1` |
| `arch`, `source` | `3` (x86_64), `2` (UEFI memmap — ánh xạ `BootSource::Uefi`) |
| `size_bytes`, `checksum` | 352, checksum cộng dồn byte (trường checksum = 0 khi tính) |
| `desc_size`, `desc_version`, `memmap_count`, `memmap_key` | 48, 1, 100, key của lần `GetMemoryMap` cuối |
| `highest_phys`, `usable_bytes`, `memmap_sum` | địa chỉ cao nhất, tổng RAM usable, checksum mảng thô |
| **`rsdp_pa`**, **`dtb_pa`** | **`0xf77e000`**, **`0`** (đo được trên OVMF này) — 0 = firmware không khai |
| `ram[16]` | 8 vùng usable đã gộp vùng kề (dùng `RamRegion`/`MAX_USABLE_RAM` của kernel) |

`verify()` (kernel đọc **và** test host) từ chối: magic/version/size sai, `arch`/`source` sai,
`n_ram == 0` (`missing-memory`), vùng RAM `size == 0`/tràn địa chỉ (`bad-region`), `rsdp_pa`
không căn 16 (`bad-rsdp`), `dtb_pa` không căn 8 (`bad-dtb`), checksum sai.

### 4.1 Bất biến **trang bàn giao** (reserved/copy) — bắt buộc khi nhảy `e_entry`

Ảnh `.efi` nằm trong vùng `EfiLoaderCode/Data` — sau `ExitBootServices` vùng đó **được nhả** cho
kernel. Vì vậy trước khi nhảy `e_entry`, bên nhận **phải** hoặc:

(a) đánh dấu `[pa, pa + size_bytes)` là **reserved** trong bảng nhớ bàn giao (không cấp cho
    frame allocator), hoặc
(b) **chép** bản ghi ra trang riêng **trước** khi cấp phát RAM.

`pa` là con trỏ bản ghi loader truyền (x86_64: `rdi`; aarch64: `x0`), `size_bytes` là trường
trong bản ghi, `UefiHandoffV1::reserved_pages(pa)` tính số trang 4 KiB cần giữ. Stub in bất biến
này ở dòng `# uefi: handoff reserved pa=0x… bytes=352 pages=1 invariant=kernel-must-reserve-or-copy`
(đo được: `pa=0xde1f000`, 1 trang) để runner đối chiếu. **KT-414 chưa nhảy `e_entry`** (kernel
chưa mở API nhận bản ghi) — bất biến được ghi ở đây + trong `handoff.rs` để lần nối dây không bỏ
sót.

### 4.2 Việc còn lại (không thuộc KT-414)

1. **Nhảy `e_entry`**: nạp ELF kernel từ ESP (parse `PT_LOAD`, cấp `AllocatePages`, `memcpy`),
   truyền con trỏ bản ghi ở `rdi`, rồi `jmp` — kèm bất biến §4.1. Đây là phần **chưa** làm.
2. **Parse ACPI** (`RSDP` → `MADT`/`FADT`/`MCFG`) để điền `apic`/`uart`/`pcie_ecam` **sau** khi
   kernel có bảng nhớ; `rsdp_pa` đã có sẵn trong bản ghi nên không phải dò.
3. **aarch64** (`aarch64-unknown-uefi` + AAVMF + `-semihosting`/`PSCI SYSTEM_OFF`): tách sang
   **KT-414b** (target rust-std chưa cài trên máy này; không cài mạng trong task).

## 5. Tầng kiểm của task

| Tầng | Lệnh | Kết quả |
|---|---|---|
| T0 | `./v2/bin/v2 t0` (fmt + clippy `-D warnings` + `cargo test` trên crate bị chạm) | **ĐỎ một phần, không phải mã task**: `vifast-boot-uefi-x86_64` clippy **PASS** + **23/23 test PASS**; `vifast-plat-common` test **PASS** (32 test `bootinfo.rs` + toàn bộ suite), clippy **ĐỎ do 13 lỗi có sẵn trong `vifast-core`** (D2-260 đã ghi nhận, dọn ở **KT-405**). `cargo clippy --no-deps -p vifast-plat-common` **PASS**. |
| T0 (đỏ-trước) | `bash run.sh --all-cases` | **XANH** 1 xanh + 8 âm + 3 đỏ-trước — ~4,5 phút (OVMF/TCG) |
| T0 (đường thứ ba) | `bash v2/test/boot/run-x86_64.sh --firmware uefi` | **XANH** rc=0 — `# bootinfo: … source=uefi` + `rsdp_pa`/`dtb_pa` + reserved/copy + ca âm `no-usable` fail-closed |
| T1 | `./v2/bin/v2 gate --dev` | **ĐỎ đúng 1 bước, vì quy trình — không phải mã**: `selftest pre-measure (KT-95)` rc=7 `tree-dirty` (worktree còn tệp chưa commit; cây sạch của Lead sẽ xanh). **Mọi bước khác OK**, gồm `QEMU x86_64 (K5' bước 1)` (37/37 kiểm sạch, TAP 47/47) và `QEMU aarch64 (K4' bước 3)`; `v2 check: PASS`; tổng **657,8 s**. Lượt chạy đầu (13:36) bị dừng theo chỉ đạo Lead khi T2 giữ khoá máy đo; lượt này chạy sau khi khoá TRỐNG (13:51). |
| T2 | `./v2/bin/v2 gate --fast` | **Chưa chạy** — Lead chạy trên ứng viên tích hợp (thay đổi chạm `boot/`, không chạm kernel/ABI nên T2 không bắt buộc ngay theo §6.1). |

R3 (SPDX) / R5 (unsafe chỉ trong allowlist `boot/`) đã xanh qua `./v2/bin/v2 rules` (PASS, E1 PASS
1.713 tên tracked). Ảnh `.efi` build **không** có mạng; `run.sh` không đổi `v2/`, `kernel/`,
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
