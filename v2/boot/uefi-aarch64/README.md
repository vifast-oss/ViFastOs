# `v2/boot/uefi-aarch64` — boot stub UEFI aarch64 (KT-414b, P1)

Ứng dụng UEFI `PE/COFF` tối giản (`efi_main`) cho **aarch64**: chụp `GetMemoryMap` **ngay trước**
`ExitBootServices`, **kiểm bảng nhớ fail-closed có tên**, dựng **`BootInfo` v1 nguồn `Uefi`**
(D2-260) + bản ghi bàn giao `UefiHandoffV1` (`rsdp_pa`/`dtb_pa`, bất biến reserved/copy), in bằng
chứng `# uefi:`/`# bootinfo:` ra `ConOut`, rồi thoát dịch vụ boot và báo kết quả cho runner bằng
**ARM semihosting** (`SYS_EXIT_EXTENDED`). **Không** đụng đường `-kernel` hiện có: đây là nhánh
boot **thứ ba**, chọn tường minh khi chạy (`run.sh` / `run-aarch64.sh --firmware uefi`), không có
mặc định nào đổi.

Đặc tả nền: [`v2/docs/u1-uefi-readiness.md` §4](../../docs/u1-uefi-readiness.md) (KT-364), kiểu
`BootInfo` v1: [`v2/kernel/vifast-plat-common/src/bootinfo.rs`](../../kernel/vifast-plat-common/src/bootinfo.rs) (KT-402).
Bản `x86_64`: [`v2/boot/uefi-x86_64`](../uefi-x86_64/README.md) (KT-414).

## 1. Chạy

```bash
cd v2/boot/uefi-aarch64
bash run.sh --all-cases    # 1 ca xanh + 8 ca âm + 3 chứng minh đỏ-trước  (~2,2 phút, TCG)
bash run.sh --positive     # chỉ ca xanh
bash run.sh --fault bad-page   # một ca âm; tên: desc-size desc-version map-size map-empty
                               #                 bad-type bad-page bad-attr no-usable
bash run.sh --naive-fault desc-size   # bản CHƯA VÁ + bảng nhớ hỏng ⇒ vẫn xanh (đỏ-trước)
```

Đường boot **thứ ba** (không đổi `-kernel`): `bash v2/test/boot/run-aarch64.sh --firmware uefi`
chạy bộ ca trên rồi kiểm thêm trên log thật `# bootinfo: v=1 arch=aarch64 source=uefi …`,
`rsdp_pa`/`dtb_pa` + bất biến reserved/copy, `after-ebs=ok` trên semihosting, và ca âm `no-usable`
fail-closed.

Kết quả: `logs/<tag>/{serial.log,qemu.out,rc,vars.fd}`, `run.sh` trả 0 ⇔ mọi kỳ vọng đúng.
Test host (không cần QEMU): `cargo test` → **27** test.

Môi trường đo trên máy này (Apple Silicon, 12 core): QEMU 11.1.3 (aarch64 **TCG**), AAVMF
`/opt/homebrew/share/qemu/edk2-aarch64-code.fd` (rev `0x00020046`, edk2-stable202408) +
`edk2-arm-vars.fd` (bản sao ghi được mỗi lần chạy). Không cần mạng, không cần root, không cần
`mtools`. Toolchain **ghim `1.100.0`** (target `aarch64-unknown-uefi` đã cài — tiền đề hạ tầng
của task); `run.sh` **fail-closed** (`need_target`) nếu thiếu rust-std, **không** rơi về `stable`.

### Vì sao `-semihosting-config` (không phải `PSCI SYSTEM_OFF`)

`PSCI SYSTEM_OFF` tắt máy nhưng **luôn** cho `rc = 0` ⇒ runner không phân biệt được "xanh" với
"fail-closed" nếu chỉ nhìn mã thoát. Semihosting `SYS_EXIT_EXTENDED` mang **mã thoát của ta** ra
tiến trình QEMU, nên giữ được **cùng thang `rc`** với `v2/boot/uefi-x86_64` (`isa-debug-exit`):
**33** xanh, **65** fail-closed bảng nhớ, **67** lỗi `ExitBootServices`. Đo được: `SYS_EXIT_EXTENDED`
với block `{0x20026, code}` cho `rc = code` **nguyên văn** (không phải `(code<<1)|1`), nên
`semihost::qemu_rc` nhân đôi + 1.

Sau `ExitBootServices` `ConOut` của firmware đã chết ⇒ bằng chứng `after-ebs=ok` đi qua
`SYS_WRITE0`, QEMU ghi ra **stderr** ⇒ nằm trong `logs/<tag>/qemu.out` (tương đương `debug.log`
của `isa-debugcon` bên x86_64).

### Vì sao AAVMF tự boot (không cần `startup.nsh`)

AAVMF (edk2-stable202408) tự tạo boot option `Boot0001 "UEFI Misc Device"` cho đĩa virtio-blk có
`\EFI\BOOT\BOOTAA64.EFI` và nạp thẳng (`BdsDxe: starting Boot0001 "UEFI Misc Device"`). ESP vẫn
chứa `startup.nsh` làm **dự phòng** cho firmware rơi vào UEFI Shell (khuôn OVMF của KT-414).

## 2. Kết quả đo (bằng chứng thật, không viết tay số)

### 2.1 Ca xanh — `logs/positive/` (rc = **33**)

```
# uefi: app=vifast-boot-aarch64 v=1 target=aarch64-unknown-uefi
# uefi: firmware rev=0x00020046 tables=8 naive=0 fault=none
# uefi: memmap=query desc_size=48 desc_version=1 map_size=0x600 key=0x32e
# uefi: memmap=ok count=30 desc_size=48 desc_version=1 map_size=0x5a0 sum=0x2e00
# uefi: memmap: n_usable=20 usable_bytes=0xfa7d000 highest_phys=0x50000000 n_runtime=7 n_acpi=1 n_reserved=0 attr_odd=0 types=0xafa
# uefi: cfgtable n=8 rsdp_pa=0x4cb43018 dtb_pa=0x0
# uefi: dtb pa=0x0 check=none (firmware khong khai FDT)
# bootinfo: v=1 arch=aarch64 source=uefi n_harts=1 boot_hart=0
# bootinfo: ram n=5 bytes=0xfa7d000
# bootinfo: ram[0]=0x40000000+0xc770000
# bootinfo: ram[1]=0x4cb40000+0x1000
# bootinfo: ram[2]=0x4cb44000+0x32dc000
# bootinfo: ram[3]=0x4feb0000+0x10000
# bootinfo: ram[4]=0x4ffe0000+0x20000
# bootinfo: harts=0 timebase=none
# bootinfo: plic=none gic=none gic_cpu=none apic=none uart=none ecam=none
# bootinfo: acpi=no pm1a=none
# uefi: handoff magic=0x3148545341464956 version=1 arch=2 va=0x4c76e000 size=352 n_ram=5 rsdp_pa=0x4cb43018 dtb_pa=0x0 checksum=0x00000d10
# uefi: handoff verify=ok checksum=ok
# uefi: handoff reserved pa=0x4c76e000 bytes=352 pages=1 invariant=kernel-must-reserve-or-copy
# uefi: exitbootservices=attempt try=0 previous_key=0x333
```

`# bootinfo:` ở trên là **nguyên văn** `BootInfo::write_log` của kernel (`arch=aarch64
source=uefi` — D2-260), không phải chuỗi chép ở boot stub: `bootinfo.rs` được `#[path]` include từ
`v2/kernel/vifast-plat-common/src/`. `rsdp_pa=0x4cb43018` là **RSDP thật** lấy từ
`EFI_CONFIGURATION_TABLE` (GUID ACPI 2.0; chữ ký `RSD PTR ` đọc lại đúng tại địa chỉ đó);
`dtb_pa=0` vì AAVMF **không** cấp FDT config table (nó cấp ACPI) — **không** phải lỗi.

`qemu.out` (semihosting, **chỉ** ghi được sau `ExitBootServices` — chứng minh đã thoát thật):

```
# uefi: exitbootservices=ok
# uefi: after-ebs=ok
```

Ghi chú đo được trên AAVMF này: `DescriptorSize = 48` (không phải 40 — firmware thêm 8 B đệm mỗi
mô tả; bên đọc **luôn** dùng stride), `Attribute = 0x8` cho mọi mô tả (đúng một bit cacheability
`WB` — khác OVMF trả `0xF`), `n_usable=20` gộp còn **5** vùng RAM. Số mô tả (`count`) và
`memmap_key` **đổi giữa các lần chạy** (firmware cấp phát khác nhau) — runner chỉ đòi hình dạng
dòng, không đòi hằng số; `usable_bytes`/`highest_phys`/`ram n` thì ổn định.

### 2.2 Ca âm — `logs/neg-<fault>/` (rc = **65**)

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
semihosting có `fail-closed`. Ví dụ `neg-no-usable` (đo được):

```
# uefi: fault=no-usable injected=1 (bang nho bi lam hong co chu dich - ca am)
# uefi: memmap=ok count=32 desc_size=48 desc_version=1 map_size=0x600 sum=0x3109
# uefi: memmap: n_usable=0 usable_bytes=0x0 highest_phys=0x50000000 n_runtime=7 n_acpi=1 n_reserved=22 attr_odd=0 types=0xa61
# uefi: cfgtable n=8 rsdp_pa=0x4cb43018 dtb_pa=0x0
# uefi: fail-closed err=missing-memory detail=- code=0x20
# uefi: memmap=FAIL err=missing-memory exitbootservices=not-called
# uefi: exit=0x20 (semihosting SYS_EXIT_EXTENDED ??? rc 65)
```

### 2.3 Đỏ-trước — `logs/naive-<fault>/` (rc = **33**)

`VIFAST_UEFI_NAIVE=1` tái hiện **hành vi trước khi vá**: bỏ toàn bộ phép kiểm, vẫn
`ExitBootServices`. Với cùng bảng nhớ hỏng, bản naive **xanh** (và `after-ebs=ok`) ⇒ ca âm ở
§2.2 thật sự bắt được lỗi, không phải "xanh giả". Ba ca đại diện: `desc-size` (hỏng siêu dữ liệu),
`bad-attr` (hỏng thuộc tính), `no-usable` (hỏng tầng `BootInfo`).

### 2.4 Tổng hợp `bash run.sh --all-cases` (đầu ra cuối, `run.sh` rc = 0)

```
PASS positive (rc=33): memmap=ok, handoff verify=ok, ExitBootServices OK, after-ebs=ok (semihosting)
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

## 3. Cấu trúc — **tái dùng, không chép**

| Tệp | Nội dung |
|---|---|
| `src/main.rs` | `efi_main`; thứ tự: banner → `GetMemoryMap` → tiêm lỗi (harness) → kiểm → `EFI_CONFIGURATION_TABLE` (RSDP/DTB) → kiểm header DTB → `BootInfo` v1 `source=uefi` → bàn giao → làm mới key → `ExitBootServices` → semihosting |
| `src/lib.rs` | `#![no_std]`; `#[path]` include **nguyên văn** `efi.rs`/`memmap.rs`/`handoff.rs`/`console.rs` của `v2/boot/uefi-x86_64` + `bootinfo.rs` của kernel; `bootinfo_from_memmap` (arch `Aarch64`); `dtb_header_ok` |
| `src/semihost.rs` | ARM semihosting: `SYS_WRITE0` (0x04), `SYS_EXIT_EXTENDED` (0x20), `ADP_Stopped_ApplicationExit` (0x20026), `qemu_rc` |
| `tests/vifast_boot_uefi_aarch64.rs` | 27 test host: offset ABI (cùng nguồn `efi.rs`), `BootArch::Aarch64` + `BootSource::Uefi`, 8 ca tiêm lỗi, `handoff` **tổng quát hoá arch** (`ARCH_AARCH64`, `build_for`/`verify_for`, `arch` có răng), căn RSDP theo arch, `dtb_header_ok`, hằng số semihosting |
| `build.rs` | danh sách đóng `VIFAST_UEFI_FAULT` / `VIFAST_UEFI_NAIVE` (sai tên ⇒ build đỏ) |

Bốn mô-đun `efi`/`memmap`/`handoff`/`console` **không** có bản sao: `src/lib.rs` trỏ thẳng vào
`v2/boot/uefi-x86_64/src/` bằng `#[path]`. Nhờ vậy hai arch **không thể lệch** bộ ca âm, tên lỗi,
bố cục `repr(C)` hay định dạng dòng log — sửa một nơi là cả hai crate cùng đổi (và test host của
**cả hai** khoá lại). `bootinfo` cũng vậy: `#[path]` vào nguồn sự thật của kernel + shim
`crate::smp::MAX_HARTS = 8`; test `bootinfo_is_included_from_kernel_not_copied` đọc `include_str!`
của `smp.rs` nên lệch số hart là đỏ.

Phần **riêng** của aarch64 chỉ là `main.rs` (luồng arch-specific: semihosting, DTB) và
`semihost.rs` — đúng ranh giới: `console.rs` dùng chung chỉ in `ConOut` **trước** `ExitBootServices`,
còn cơ chế thoát/bằng chứng **sau** đó khác nhau giữa `isa-debug-exit` (x86_64) và semihosting
(aarch64).

## 4. Bàn giao `BootInfo` v1 — `BootSource::Uefi` (D2-260)

Bản ghi `UefiHandoffV1` (**352 B**, `repr(C)`, magic `"VIFASTH1"`, checksum cộng dồn) — **cùng**
struct với x86_64, chỉ khác `arch = 2` (`ARCH_AARCH64`):

| Trường | Giá trị (đo được) |
|---|---|
| `magic`, `version` | `0x3148545341464956` (`"VIFASTH1"`), `1` |
| `arch`, `source` | **`2`** (aarch64), `2` (UEFI memmap — ánh xạ `BootSource::Uefi`) |
| `size_bytes`, `checksum` | 352, checksum cộng dồn byte (trường checksum = 0 khi tính) |
| `desc_size`, `desc_version`, `memmap_count`, `memmap_key` | 48, 1, 30, key của lần `GetMemoryMap` cuối |
| `highest_phys`, `usable_bytes`, `memmap_sum` | `0x50000000`, `0xfa7d000`, `0x2e00` |
| **`rsdp_pa`**, **`dtb_pa`** | **`0x4cb43018`**, **`0`** (AAVMF cấp ACPI, không cấp FDT) |
| `ram[16]` | 5 vùng usable đã gộp vùng kề |

`verify_for(ARCH_AARCH64)` (kernel đọc **và** test host) từ chối: magic/version/size sai,
`arch` khác `ARCH_AARCH64` (`bad-arch`), `source` sai, `n_ram == 0` (`missing-memory`), vùng RAM
`size == 0`/tràn địa chỉ (`bad-region`), `rsdp_pa` không căn **4** (`bad-rsdp` — xem §4.2),
`dtb_pa` không căn 8 (`bad-dtb`), checksum sai.

### 4.1 Bất biến **trang bàn giao** (reserved/copy) — bắt buộc khi nhảy `e_entry`

Ảnh `.efi` nằm trong vùng `EfiLoaderCode/Data` — sau `ExitBootServices` vùng đó **được nhả** cho
kernel. Vì vậy trước khi nhảy `e_entry`, bên nhận **phải** hoặc:

(a) đánh dấu `[pa, pa + size_bytes)` là **reserved** trong bảng nhớ bàn giao (không cấp cho
    frame allocator), hoặc
(b) **chép** bản ghi ra trang riêng **trước** khi cấp phát RAM.

`pa` là con trỏ bản ghi loader truyền (aarch64: `x0`), `size_bytes` là trường trong bản ghi,
`UefiHandoffV1::reserved_pages(pa)` tính số trang 4 KiB cần giữ. Stub in bất biến này ở dòng
`# uefi: handoff reserved pa=0x… bytes=352 pages=1 invariant=kernel-must-reserve-or-copy` (đo
được: `pa=0x4c76e000`, 1 trang) để runner đối chiếu. **KT-414b chưa nhảy `e_entry`** (kernel chưa
mở API nhận bản ghi) — bất biến được ghi ở đây + trong `handoff.rs` để lần nối dây không bỏ sót.

### 4.2 Hai phát hiện **chỉ aarch64 mới lộ** (đã vá trong task này)

1. **`ACPI_20_TABLE_GUID` sai byte-order** trong `v2/boot/uefi-x86_64/src/efi.rs` (mô-đun dùng
   chung): bản KT-414 khai `e8 71 68 88 …` (Data1 `0x8868_71e8`) trong khi `Data1 = 0x8868_e871`
   ⇒ LE là `71 e8 68 88`. Trên OVMF lỗi **bị che** vì firmware cũng cấp bảng ACPI 1.0 (fallback
   đúng) nên `rsdp_pa` vẫn khác 0; trên **AAVMF** chỉ có bảng ACPI 2.0 ⇒ `rsdp_pa=0` (đo được
   trước khi vá). Đã sửa + khoá từng byte bằng test host `config_table_guids_match_spec_bytes`
   (crate x86_64) và `config_table_lookup_by_guid` (crate này).
2. **Căn RSDP không như spec**: ACPI 6.x §5.2.2.1 đòi RSDP căn **16** byte, nhưng edk2 đặt RSDP
   2.0 **ngay sau** RSDP 1.0 (20 byte) trong cùng một khối ⇒ chỉ căn **4**. Đo được: OVMF bảng
   ACPI 1.0 → `0xf77e000`, bảng ACPI 2.0 → **`0xf77e014`**; AAVMF bảng ACPI 2.0 → **`0x4cb43018`**
   (edk2 cấp bằng pool). Cả hai đọc lại đúng chữ ký `RSD PTR `. Vì vậy `handoff::RSDP_ALIGN = 4`
   (hiệu chỉnh từ firmware thật, không phải spec lý tưởng) — đòi 16 là false-red trên **cả hai**
   firmware; KT-414 chỉ "qua" được vì rơi về bảng ACPI 1.0 (xem phát hiện 1). Test host
   `rsdp_alignment_is_measured_not_spec_ideal` khoá cả hai địa chỉ đo được + ca không căn 4.

### 4.3 Rủi ro cho kernel khi nối dây (ghi lại, **chưa** thuộc task này)

- **Trang chứa RSDP nằm trong RAM usable**: `rsdp_pa=0x4cb43018` rơi đúng vào vùng
  `ram[1]=0x4cb40000+0x1000` mà `GetMemoryMap` khai **usable**. Kernel **phải** giữ trang đó (hoặc
  copy 36 B RSDP) **trước** khi cấp phát frame — cùng lớp bất biến với trang bàn giao §4.1. Nếu
  không, frame allocator có thể ghi đè RSDP rồi `parse_acpi` đọc rác.
- **`dtb_pa=0` trên AAVMF**: kernel không được giả định "aarch64 ⇒ có DTB". Nguồn cấu hình phải
  chọn theo dữ liệu (`dtb_pa != 0` ⇒ FDT, ngược lại ⇒ ACPI qua `rsdp_pa`), đúng thứ tự ưu tiên đã
  ghi ở `u1-uefi-readiness.md` §4.3.
- **`gic`/`uart`/`pcie_ecam` = `None`** trong `BootInfo` bước stub là **đúng** (chưa parse
  ACPI/DTB); kernel phải parse từ `rsdp_pa` **sau** khi đã có bảng nhớ.

## 5. Tầng kiểm của task

| Tầng | Lệnh | Kết quả |
|---|---|---|
| T0 (host) | `cargo test` trong `v2/boot/uefi-aarch64` | **27/27 PASS** |
| T0 (host, không hồi quy) | `cargo test` trong `v2/boot/uefi-x86_64` | **24/24 PASS** (23 của KT-414 + 1 khoá GUID) |
| T0 (QEMU) | `bash run.sh --all-cases` | **XANH** 1 xanh + 8 âm + 3 đỏ-trước — **2m12s** |
| T0 (đường thứ ba) | `bash v2/test/boot/run-aarch64.sh --firmware uefi` | **XANH** 2/2 — `# bootinfo: … arch=aarch64 source=uefi` + `rsdp_pa`/`dtb_pa` + reserved/copy + `after-ebs=ok` + ca âm `no-usable` fail-closed — **1m46s** |
| T0 (không hồi quy x86_64) | `bash v2/boot/uefi-x86_64/run.sh --all-cases` | **XANH** 1 xanh + 8 âm + 3 đỏ-trước — **1m45s** (sau khi sửa mô-đun chung) |
| T0 (clippy bin) | `cargo clippy --target aarch64-unknown-uefi --features uefi-image -- -D warnings` | **PASS** (T0 bỏ bin vì `required-features`, nên chạy thêm tay) |
| `v2 check` | `./v2/bin/v2 check` | **PASS** — 122,8 s |
| T1 | `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` | **PASS** — `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')`, `TỔNG gate --dev (T1) 652793 ms`, wall 27m6s |
| T2 | `./v2/bin/v2 gate --fast` | **Lead** chạy trên ứng viên tích hợp |

## 6. Ghi chú kỹ thuật đáng nhớ (đã trả giá trong task)

- **`hlt #0xF000` + `-semihosting-config enable=on,target=native`**: `SYS_EXIT_EXTENDED` (0x20) với
  block `{0x20026, code}` cho `rc = code`; `SYS_WRITE0` (0x04) ghi ra **stderr** của QEMU (không
  phải `-serial`). Không có semihosting ⇒ `hlt` trap vào firmware đã chết ⇒ treo; watchdog
  `rc=124` **không** phải kết quả hợp lệ.
- **AAVMF cần `-cpu cortex-a57`** (mặc định `virt` là `cortex-a15` 32-bit) và **pflash** cho
  code+vars; ESP phải là **virtio-blk-pci** (`-drive if=none,file=fat:rw:… -device
  virtio-blk-pci`) — `virt` không có IDE/AHCI như `q35` của x86_64.
- **`ConOut->OutputString` cấp phát pool bên trong firmware** ⇒ in giữa `GetMemoryMap` và
  `ExitBootServices` làm `MapKey` cũ ngay lập tức. Vì vậy **không in gì** giữa lần `GetMemoryMap`
  cuối và `ExitBootServices` (cùng kết luận với KT-414).
- **`-drive file=fat:ro:…` bị QEMU từ chối** (`Block node is read-only`); dùng `fat:rw` (QEMU ghi
  ra overlay qcow tạm, không đụng thư mục nguồn).
- Một lượt boot AAVMF ~8–10 s (TCG). `run.sh` có watchdog `VIFAST_UEFI_TIMEOUT` (mặc định 180 s).
