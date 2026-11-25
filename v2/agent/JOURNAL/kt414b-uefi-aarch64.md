<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-414b — UEFI boot `aarch64` (AAVMF) + chuẩn hoá target UEFI trên `rustc 1.100.0` (D2-260 mục 4)

Nhánh `task/kt414b-uefi-aarch64`, base `main` @ `19f0a4c3`. Agent `kt414b`, worktree
`/private/tmp/vifast-wt/4`, `CARGO_TARGET_DIR=/Users/mps/dvchd/ViFastOs/target-task/slot-4`.
Ngân sách 3 h (W4). Phần `x86_64` đã xong ở **KT-414** (`task/kt414-uefi`, T2 75/75 @ `26287331`).

## 0. Kế hoạch (5–10 dòng)

1. **Đỏ-trước (tầng runner)**: thêm khối `--firmware uefi` cho `v2/test/boot/run-aarch64.sh`
   (khối **riêng**, không định dạng lại tệp — KT-402b cũng sửa tệp này) rồi chạy ⇒ phải ĐỎ vì
   chưa có `v2/boot/uefi-aarch64/run.sh`.
2. **Đỏ-trước (tầng host)**: viết `tests/` của crate mới đòi `handoff::ARCH_AARCH64` +
   `build_for`/`verify_for` (tổng quát hoá mô-đun **chung** `handoff` của KT-414) ⇒ `cargo test`
   ĐỎ vì API chưa có.
3. Dựng `v2/boot/uefi-aarch64`: `#[path]` include **nguyên văn** `efi.rs`/`memmap.rs`/`handoff.rs`/
   `console.rs` của `v2/boot/uefi-x86_64` + `bootinfo.rs` của kernel (không chép bản sao) +
   `semihost.rs` (SYS_WRITE0/SYS_EXIT_EXTENDED) + `main.rs` (arch-specific).
4. `run.sh` của crate: build `aarch64-unknown-uefi` bằng **toolchain ghim 1.100.0** (bỏ fallback
   `stable`), ESP `EFI/BOOT/BOOTAA64.EFI`, QEMU `virt` + AAVMF + `-semihosting-config`, 1 ca xanh
   + 8 ca âm + 3 đỏ-trước (naive).
5. Nối `--firmware uefi` cho aarch64 (0 đổi `-kernel`), kiểm `# bootinfo: … arch=aarch64
   source=uefi`, `rsdp_pa`/`dtb_pa`, bất biến reserved/copy, `after-ebs=ok` (semihosting), ca âm
   `no-usable` fail-closed.
6. Bỏ fallback `RUSTUP_TOOLCHAIN=stable` ở `v2/boot/uefi-x86_64/run.sh` + README; chạy lại
   selftest x86_64 để chứng minh **không hồi quy**.
7. `./v2/bin/v2 t0` → `./v2/bin/v2 check` → **T1** `gate --dev` qua khoá máy.

## 1. Đỏ-trước (log nguyên văn)

### 1.1 Tầng runner — `--firmware uefi` chưa có stub

```
$ bash v2/test/boot/run-aarch64.sh --firmware uefi ; echo rc=$?
run-aarch64.sh: không thấy boot stub UEFI aarch64: /private/tmp/vifast-wt/4/v2/boot/uefi-aarch64/run.sh
rc=1
```

Đỏ **đúng lý do**: yêu cầu "boot aarch64 qua AAVMF" chưa có artefact nào; khối `--firmware` là
phần **kiểm** của yêu cầu, nên nó phải đỏ trước khi có `run.sh` của crate.

### 1.2 Tầng host — mô-đun chung `handoff` chỉ biết x86_64

```
$ cd v2/boot/uefi-aarch64 && cargo test
error[E0432]: unresolved import `vifast_boot_uefi_aarch64::handoff::ARCH_AARCH64`
error[E0599]: no associated function or constant named `build_for` found for struct `UefiHandoffV1`
error[E0599]: no method named `verify_for` found for struct `UefiHandoffV1` in the current scope
error[E0599]: no method named `verify_for` found for struct `UefiHandoffV1` in the current scope
error[E0599]: no method named `verify_for` found for struct `UefiHandoffV1` in the current scope
error: could not compile `vifast-boot-uefi-aarch64` (test "vifast_boot_uefi_aarch64") due to 5 previous errors
```

Đỏ **đúng lý do**: `handoff.rs` (mô-đun **dùng chung** theo yêu cầu task) hard-code
`ARCH_X86_64` trong `build()`/`verify()` ⇒ boot stub aarch64 **không thể** ghi bản ghi đúng arch
nếu không chép tệp. Test viết trước buộc phải **tổng quát hoá** mô-đun chung, không chép bản sao.

### 1.3 Đỏ-trước của chính phép kiểm fail-closed (mạnh nhất — chạy lại được mọi lúc)

`VIFAST_UEFI_NAIVE=1` tái hiện hành vi **trước khi vá** (bỏ kiểm bảng nhớ) + bảng nhớ bị tiêm
lỗi ⇒ **vẫn xanh** (`rc=33`, `after-ebs=ok`) ⇒ ca âm ở §3.2 thật sự bắt được lỗi:

```
RED-PROOF naive-desc-size: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33) ⇒ ca âm bắt được lỗi thật
RED-PROOF naive-bad-attr: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33) ⇒ ca âm bắt được lỗi thật
RED-PROOF naive-no-usable: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=33) ⇒ ca âm bắt được lỗi thật
```

## 2. Đã làm

### 2.1 `v2/boot/uefi-aarch64/` (mới) — tái dùng, không chép

- `src/lib.rs`: `#![no_std]`; `#[path]` include **nguyên văn** bốn mô-đun của
  `v2/boot/uefi-x86_64` (`efi.rs`, `memmap.rs`, `handoff.rs`, `console.rs`) + `bootinfo.rs` của
  kernel (`v2/kernel/vifast-plat-common/src/`) + shim `crate::smp::MAX_HARTS = 8`;
  `bootinfo_from_memmap` (arch `Aarch64`, source `Uefi`); `dtb_header_ok` (phần thuần của phép
  kiểm DTB).
- `src/semihost.rs`: ARM semihosting — `SYS_WRITE0` (0x04), `SYS_EXIT_EXTENDED` (0x20),
  `ADP_Stopped_ApplicationExit` (0x20026), `qemu_rc(val) = (val<<1)|1` (giữ **cùng thang rc** với
  `isa-debug-exit` của x86_64: 33/65/67).
- `src/main.rs`: luồng arch-specific — banner, `GetMemoryMap`, tiêm lỗi, kiểm fail-closed,
  `EFI_CONFIGURATION_TABLE` → `rsdp_pa`/`dtb_pa`, kiểm header DTB khi firmware khai, `BootInfo` v1
  `source=uefi`, bản ghi bàn giao `build_for(ARCH_AARCH64, …)` + `verify_for(ARCH_AARCH64)`, làm
  mới `MapKey` → `ExitBootServices`, rồi `after-ebs=ok` + thoát qua semihosting.
- `build.rs`: danh sách đóng `VIFAST_UEFI_FAULT`/`VIFAST_UEFI_NAIVE` (cùng bộ 8 lỗi với x86_64).
- `run.sh`: build `aarch64-unknown-uefi` bằng **toolchain ghim `1.100.0`**, `need_target`
  fail-closed nếu thiếu rust-std (TỪ CHỐI ĐO), ESP `EFI/BOOT/BOOTAA64.EFI` + `startup.nsh` dự
  phòng, QEMU `virt -cpu cortex-a57 -m 256` + AAVMF pflash + virtio-blk-pci + `-semihosting-config
  enable=on,target=native`; 1 xanh + 8 âm + 3 đỏ-trước; tôn trọng `CARGO_TARGET_DIR`.
- `tests/vifast_boot_uefi_aarch64.rs`: **27 test host** (bố cục ABI, `BootArch::Aarch64` +
  `BootSource::Uefi`, 8 ca tiêm lỗi, `handoff` tổng quát hoá arch, căn RSDP đo được, `dtb_header_ok`,
  hằng số semihosting).
- `README.md`: đặc tả + bằng chứng đo + **hai phát hiện** + rủi ro cho kernel khi nối dây.

### 2.2 Mô-đun **chung** `v2/boot/uefi-x86_64/src/handoff.rs` — tổng quát hoá arch

- Thêm `ARCH_RISCV64 = 1`, **`ARCH_AARCH64 = 2`**, `ARCH_X86_64 = 3` (ánh xạ theo **thứ tự**
  `BootArch` của kernel).
- `build_for(arch, …)` + `verify_for(arch)`; `build()`/`verify()` **giữ nguyên nghĩa x86_64** nên
  23 test của KT-414 không đổi hành vi. `verify()` nhận arch **đã biết** (x86_64/aarch64), arch lạ
  ⇒ `bad-arch`; `verify_for(arch)` **ghim** arch (bản ghi aarch64 bị `verify_for(ARCH_X86_64)` từ
  chối) ⇒ trường `arch` **có răng**, không phải trang trí.
- `RSDP_ALIGN = 4` thay cho "căn 16" (xem §2.4).

### 2.3 `v2/test/boot/run-aarch64.sh` — khối `--firmware uefi` **riêng**

Thêm **một khối độc lập** ngay sau `mkdir -p "$LOGDIR"`: hàm `run_firmware_uefi()` + nhánh
`case "${1:-}" in --firmware) … esac`. **Không** định dạng lại tệp, **không** đổi dòng nào khác
(để Lead merge với KT-402b — agent đó cũng sửa tệp này để thêm `-dtb`). Khối kiểm: selftest xanh
(1+8+3), `# uefi: app=vifast-boot-aarch64 …`, `# bootinfo: v=1 arch=aarch64 source=uefi …`,
`rsdp_pa`/`dtb_pa`, bất biến reserved/copy, `after-ebs=ok` trên **semihosting** (`qemu.out`), ca âm
`no-usable` fail-closed + **không** gọi `ExitBootServices`. Đường `-kernel` **không đổi một byte**.

### 2.4 Hai lỗi của mô-đun dùng chung — **chỉ aarch64 mới lộ** (đã vá)

1. **`ACPI_20_TABLE_GUID` sai byte-order** (`efi.rs`): KT-414 khai `e8 71 68 88 …` (Data1
   `0x8868_71e8`) trong khi `Data1 = 0x8868_e871` ⇒ LE là `71 e8 68 88`. Trên OVMF lỗi **bị che**
   vì firmware cũng cấp bảng ACPI 1.0 (fallback đúng) nên `rsdp_pa` vẫn khác 0; trên **AAVMF**
   (chỉ cấp ACPI 2.0) `rsdp_pa=0` — đo được trước khi vá. Đã sửa + khoá từng byte bằng test
   `config_table_guids_match_spec_bytes` (crate x86_64, 24 test) và `config_table_lookup_by_guid`
   (crate aarch64).
2. **Ngưỡng căn RSDP `16` → `RSDP_ALIGN = 4`**: ACPI 6.x §5.2.2.1 đòi 16, nhưng edk2 đặt RSDP 2.0
   **ngay sau** RSDP 1.0 (20 byte) trong cùng khối ⇒ chỉ căn 4. Đo được: OVMF bảng ACPI 1.0 →
   `0xf77e000`, bảng ACPI 2.0 → **`0xf77e014`**; AAVMF → **`0x4cb43018`**; cả hai đọc lại đúng chữ
   ký `RSD PTR `. Ngưỡng 16 chỉ "qua" được nhờ fallback ACPI 1.0; sau khi sửa GUID nó thành
   **false-red** trên cả hai firmware. Test `handoff_rejects_tampering_with_name` đổi giá trị xấu
   sang **không căn 4** (`0x7f00_0002`) để phép kiểm "con trỏ rác" còn răng.

### 2.5 Bỏ fallback `RUSTUP_TOOLCHAIN=stable` (D2-260 mục 4)

- `v2/boot/uefi-x86_64/run.sh`: `TOOLCHAIN` mặc định `stable` → **`1.100.0`**; thêm `need_target`
  fail-closed (thiếu rust-std ⇒ TỪ CHỐI ĐO, không lặng lẽ rơi về toolchain khác); `TDIR_ROOT` tôn
  trọng `CARGO_TARGET_DIR` của môi trường (máy đo dùng target dùng chung của slot).
- `v2/boot/uefi-x86_64/README.md`: mục "Vì sao `RUSTUP_TOOLCHAIN=stable`" → "Toolchain: ghim
  `1.100.0` (nợ của KT-414 đã trả ở KT-414b)"; cập nhật 23 → **24** test; ghi hai lỗi §2.4.
- `v2/boot/uefi-aarch64/run.sh`: mặc định `1.100.0` + `need_target` ngay từ đầu.

### 2.6 `cargo fmt` trên **cả hai** crate boot

Hai crate `v2/boot/*` **không** thuộc workspace `v2/` nên `cargo fmt --all` của `v2 check` không
chạm tới; tệp của KT-414 vì vậy **chưa** được rustfmt (import chưa sắp, chữ ký hàm chưa xuống
dòng). Đã chạy `cargo fmt` trong **cả hai** crate ⇒ diff trên `console.rs`/`memmap.rs`/`efi.rs`/
`main.rs` của KT-414 là **thuần định dạng** (đã xem diff: chỉ sắp import + xuống dòng), không đổi
ngữ nghĩa. `cargo fmt --all -- --check` xanh ở cả hai.

## 3. Bằng chứng (log nguyên văn)

### 3.1 Ca xanh aarch64 — `v2/boot/uefi-aarch64/logs/positive/` (rc = **33**)

```
# uefi: app=vifast-boot-aarch64 v=1 target=aarch64-unknown-uefi
# uefi: firmware rev=0x00020046 tables=8 naive=0 fault=none
# uefi: memmap=query desc_size=48 desc_version=1 map_size=0x5a0 key=0x333
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

`qemu.out` (semihosting — **chỉ** ghi được sau `ExitBootServices`):

```
# uefi: exitbootservices=ok
# uefi: after-ebs=ok
```

`# bootinfo:` là **nguyên văn** `BootInfo::write_log` của kernel (`arch=aarch64 source=uefi`), không
phải chuỗi chép ở boot stub. `rsdp_pa=0x4cb43018` là RSDP thật (chữ ký `RSD PTR ` đọc lại đúng);
`dtb_pa=0` vì AAVMF cấp ACPI, **không** cấp FDT config table — không phải lỗi.

### 3.2 Ca âm aarch64 — `logs/neg-<fault>/` (rc = **65**)

```
PASS neg-desc-size (rc=65): fail-closed err=map-desc-size, KHÔNG ExitBootServices
PASS neg-desc-version (rc=65): fail-closed err=map-desc-version, KHÔNG ExitBootServices
PASS neg-map-size (rc=65): fail-closed err=map-size, KHÔNG ExitBootServices
PASS neg-map-empty (rc=65): fail-closed err=map-empty, KHÔNG ExitBootServices
PASS neg-bad-type (rc=65): fail-closed err=map-bad-type, KHÔNG ExitBootServices
PASS neg-bad-page (rc=65): fail-closed err=map-bad-region, KHÔNG ExitBootServices
PASS neg-bad-attr (rc=65): fail-closed err=map-bad-attribute, KHÔNG ExitBootServices
PASS neg-no-usable (rc=65): fail-closed err=missing-memory, KHÔNG ExitBootServices
```

`neg-no-usable` (ca âm của tầng `BootInfo` theo nguồn `Uefi`, D2-260):

```
# uefi: fault=no-usable injected=1 (bang nho bi lam hong co chu dich - ca am)
# uefi: memmap=ok count=30 desc_size=48 desc_version=1 map_size=0x5a0 sum=0x2e00
# uefi: memmap: n_usable=0 usable_bytes=0x0 highest_phys=0x50000000 n_runtime=7 n_acpi=1 n_reserved=20 attr_odd=0 types=0xa61
# uefi: fail-closed err=missing-memory detail=- code=0x20
# uefi: memmap=FAIL err=missing-memory exitbootservices=not-called
```

### 3.3 Tổng hợp selftest

Chạy trên **cây cuối** (sau `cargo fmt` + sửa doc — không đổi mã):

```
$ bash v2/boot/uefi-aarch64/run.sh --all-cases
SELFTEST: XANH (1 xanh + 8 âm + 3 đỏ-trước)          # 2m00,6s
$ bash v2/test/boot/run-aarch64.sh --firmware uefi
ok 1 - firmware uefi aarch64: 1 xanh + 8 âm + 3 đỏ-trước (log …/uefi-selftest.log)
ok 2 - firmware uefi aarch64: '# bootinfo: … arch=aarch64 source=uefi' + rsdp_pa/dtb_pa + reserved/copy + after-ebs + ca âm fail-closed
# 1m41,3s
$ bash v2/boot/uefi-x86_64/run.sh --all-cases        # KHÔNG hồi quy sau khi sửa mô-đun chung
SELFTEST: XANH (1 xanh + 8 âm + 3 đỏ-trước)          # 1m45,4s
```

Sau khi sửa GUID, **cả hai** arch đọc đúng RSDP của bảng ACPI 2.0:
`x86_64: rsdp_pa=0xf77e014` (trước: `0xf77e000` nhờ fallback ACPI 1.0), `aarch64: rsdp_pa=0x4cb43018`.

### 3.4 Host test

```
$ cd v2/boot/uefi-aarch64 && cargo test     → test result: ok. 27 passed; 0 failed
$ cd v2/boot/uefi-x86_64 && cargo test      → test result: ok. 24 passed; 0 failed   (23 của KT-414 + 1 khoá GUID)
$ cargo clippy --target aarch64-unknown-uefi --features uefi-image -- -D warnings   → Finished (0 cảnh báo)
$ cargo clippy --target x86_64-unknown-uefi  --features uefi-image -- -D warnings   → Finished (0 cảnh báo)
```

## 4. T0 / T1

| Tầng | Lệnh | Kết quả |
|---|---|---|
| T0 | `./v2/bin/v2 t0` | **PASS** (2 crate: `vifast-boot-uefi-aarch64` + `vifast-boot-uefi-x86_64` — fmt + clippy `-D warnings` + `cargo test`) |
| T0 (bin `.efi`) | `cargo clippy --target {aarch64,x86_64}-unknown-uefi --features uefi-image -- -D warnings` | **PASS** cả hai — T0 bỏ bin vì `required-features = ["uefi-image"]`, nên chạy thêm tay để `main.rs` cũng qua clippy |
| `v2 check` | `./v2/bin/v2 check` | **PASS** — 122,8 s (fmt + clippy + luật R1..R5 + E1 + toàn bộ selftest) |
| **T1** | `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` | **PASS** — dòng nguyên văn: `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')`; `# step-time-total 652793 ms TỔNG gate --dev (T1)`; wall **27m6s** (gồm chờ khoá máy) |
| T2 | `./v2/bin/v2 gate --fast` | **Lead** chạy trên ứng viên tích hợp (task **không** chạy T2 theo chỉ đạo) |

`v2 check`/T1 xanh ⇒ R3 (SPDX) và R5 (`unsafe` chỉ trong allowlist `boot/`) đã quét
`v2/boot/uefi-aarch64/` (thư mục `boot` nằm trong `SCAN_DIRS`).

## 5. Việc còn lại / rủi ro / cần Lead quyết

1. **Nhảy `e_entry`** (nạp ELF kernel từ ESP, truyền bản ghi ở `x0`) **chưa** làm — ngoài phạm vi
   KT-414/KT-414b (kernel chưa mở API nhận bản ghi). Bất biến reserved/copy đã ghi + in.
2. **Trang chứa RSDP nằm trong RAM usable** (đo được: `rsdp_pa=0x4cb43018` rơi vào
   `ram[1]=0x4cb40000+0x1000`): kernel **phải** giữ trang đó (hoặc copy 36 B RSDP) **trước** khi
   cấp phát frame — cùng lớp bất biến với trang bàn giao. Ghi ở README §4.3; **cần** một task nối
   dây (KT-424?) xử lý.
3. **`dtb_pa=0` trên AAVMF**: kernel không được giả định "aarch64 ⇒ có DTB"; chọn nguồn theo dữ
   liệu (`dtb_pa != 0` ⇒ FDT, ngược lại ⇒ ACPI qua `rsdp_pa`).
4. **Nhánh kiểm DTB chưa chạy trên QEMU** (AAVMF không cấp FDT): phần **thuần** `dtb_header_ok`
   được test host khoá; nhánh đọc PA chỉ chạy trên firmware cấp FDT. Ghi rõ ở README §4.3.
5. **`v2/rust-toolchain.toml` không liệt kê target UEFI** — cố ý: thêm vào sẽ khiến rustup đòi cài
   (mạng) mỗi lần vào cây. Tiền đề hạ tầng (đã có trên máy đo) là `rustup target add … --toolchain
   1.100.0`; `run.sh` fail-closed nếu thiếu. **Lead quyết** có ghi tiền đề này vào tài liệu máy đo
   hay không.
6. **`v2/test/boot/run-aarch64.sh` bị hai task cùng sửa** (KT-402b thêm `-dtb`, task này thêm khối
   `--firmware`): khối của task này **độc lập**, nằm ngay sau `mkdir -p "$LOGDIR"`, không đổi dòng
   nào khác ⇒ merge tay dễ. **Lead kiểm** khi gộp.
