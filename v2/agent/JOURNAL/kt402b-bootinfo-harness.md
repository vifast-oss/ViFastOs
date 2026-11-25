<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-402b — Đóng nợ harness `KT-402`: `aarch64 -dtb` thật + `x86_64 fw_cfg` + ca âm bảng hỏng 3 arch

Người làm: **agent `kt402b`** (slot 3, worktree `/private/tmp/vifast-wt/3`, nhánh
`task/kt402b-bootinfo-harness` từ `main` @ `19f0a4c3`). Nguồn: `v2/agent/QUEUE.md` dòng KT-402b;
`D2-260` mục 5; `KE-HOACH-KIEN-TRUC.md` dòng KT-402b.

**Ranh giới** (nguyên văn QUEUE): chỉ chạm `v2/kernel/vifast-plat-aarch64/src/bootinfo.rs`,
`v2/kernel/vifast-plat-x86_64/src/bootinfo.rs`, `v2/test/boot/{run-aarch64,run-x86_64,ktest}.sh`.
0 chạm `spec/`, `v2/kernel/vifast-plat-common/`, `v2/kernel/vifast-kbin-*/`, `v2/agent/{STATE,QUEUE,
DECISIONS,BLOCKED}.md`, `v2/agent/reports/STATUS.md`. Không commit (lead commit).

## 0. Kế hoạch (5–10 dòng)

1. **Đo trước**: dựng ảnh ktest 3 arch, boot tay để chụp log **đỏ** hiện trạng (`absent src=dtb`
   trên aarch64; `missing-table:rsdp` trên x86_64 khi `acpi=off`).
2. **aarch64**: `run-aarch64.sh` dựng DTB thật bằng `-machine dumpdtb=…`, **cắt về đúng
   `totalsize`** (QEMU ghi nguyên 1 MiB đệm ⇒ không vừa khoảng trống 512 KiB trước ảnh), rồi
   truyền `-dtb` cho boot ktest ⇒ `detect` chạy thật `parse_fdt` ⇒ `# bootinfo: v=1 arch=aarch64
   source=dtb`. Ca âm: DTB hỏng nạp bằng `-device loader` (QEMU **từ chối** `-dtb` hỏng) ⇒
   `# bootinfo: fail-closed err=…` + halt.
3. **x86_64**: `plat-x86_64/src/bootinfo.rs` thêm nhánh fallback `fw_cfg` (`etc/acpi/tables` +
   `etc/table-loader`) khi không có RSDP; đồng bộ chú thích đầu tệp. `run-x86_64.sh` kiểm
   `# bootinfo: v=1 arch=x86_64 source=acpi` + ca âm `-M q35,acpi=off` (không có bảng ⇒ fail-closed).
4. **riscv64**: `ktest.sh` kiểm `# bootinfo: v=1 arch=riscv64 source=dtb` trên boot ảnh sản phẩm
   của `kt86_rev_gate` (đường duy nhất trong `ktest.sh` boot ảnh `src/main.rs`).
5. Sau mỗi bước: `./v2/bin/v2 t0 <crate>`. Cuối: `./v2/bin/v2 check` + T1 qua khoá máy.

## 1. Bằng chứng ĐỎ trước (chụp trên `main` @ `19f0a4c3`, chưa sửa gì)

### 1.1 aarch64 — runner **không** truyền DTB ⇒ `detect` không bao giờ chạy `parse_fdt`

Boot tay đúng tham số runner (`-M virt,gic-version=3 -cpu cortex-a72 -m 128M -smp 4 -nic none`):

```
$ qemu-system-aarch64 -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 \
    -kernel $CARGO_TARGET_DIR/aarch64-unknown-none/release/vifast-kbin-aarch64 -no-reboot -nic none
TAP version 13
1..46
# bootinfo: absent src=dtb (QEMU ELF direct boot không cài DTB; xem bootinfo.rs)
```

⇒ Yêu cầu "`# bootinfo: v=1 arch=aarch64 source=dtb`" **ĐỎ** (dòng hiện tại là `absent src=dtb`).

### 1.2 x86_64 — nhánh fallback `fw_cfg` **không tồn tại** (không có RSDP ⇒ chết ngay, không thử fw_cfg)

```
$ qemu-system-x86_64 -M q35,acpi=off -cpu qemu64 -m 128M -nographic -no-reboot -smp 4 \
    -kernel $CARGO_TARGET_DIR/x86_64-unknown-none/release/vifast-kbin-x86_64 -nic none
TAP version 13
1..47
# bootinfo: fail-closed err=missing-table:rsdp
```

⇒ Lỗi **có tên** đúng (fail-closed), nhưng tên `rsdp` chứng minh `detect` **chưa từng** đọc
`etc/acpi/tables`: nhánh fallback chưa có. Sau khi vá, cùng boot này phải báo
`missing-table:acpi-tables` (đã thử fw_cfg rồi mới chết).

### 1.3 riscv64 — dòng đã có, runner **chưa** kiểm

```
$ qemu-system-riscv64 -machine virt,aia=none,aclint=on -bios $OPENSBI -kernel \
    $CARGO_TARGET_DIR/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 -nographic -cpu rv64 -m 128M -smp 1
# bootinfo: v=1 arch=riscv64 source=dtb n_harts=1 boot_hart=0
```

⇒ Kernel in đúng, nhưng `ktest.sh` không có phép kiểm nào cho dòng này ⇒ hồi quy sẽ lọt.

## 2. `aarch64` — DTB **thật** qua `-machine dumpdtb` + `-dtb` (D2-260 mục 5)

### 2.1 Đo để chọn cách (hai phát hiện, đều **đo được**, không suy đoán)

1. **`-dtb` với tệp `dumpdtb` nguyên bản KHÔNG có tác dụng**: `dumpdtb` ghi **1 MiB** đệm
   (`totalsize` = `0x0010_0000`) trong khi khoảng trống trước ảnh chỉ **512 KiB**
   (`0x4000_0000..0x4008_0000`) ⇒ QEMU **bỏ qua** `-dtb`. Bằng chứng: boot `-S` rồi
   `pmemsave 0x40000000 0x8000000` (128 MiB) — **0** lần magic `d00dfeed` trong toàn bộ RAM.
2. **Cắt `totalsize` về kích thước thật thì `-dtb` hoạt động**: cùng phép đo trên, magic xuất
   hiện **đúng một** lần ở `0x4000_0000` (= `loader_start`). Cắt bỏ vùng đệm là thao tác hợp lệ
   theo spec FDT (bảng vẫn **nguyên byte** ở phần có nghĩa).

Vì vậy runner: `-machine virt,gic-version=3,dumpdtb=…` (cùng `-cpu`/`-m`/`-smp` với boot) →
`python3` cắt `totalsize` → `-dtb` cho `boot_full`. Log runner:

```
# dựng DTB thật (-machine dumpdtb) rồi cắt totalsize cho -dtb
# dtb: totalsize 1048576 -> 8642 (struct=7968 strings=610 rsvmap=16)
```

### 2.2 Kết quả — `detect` chạy **thật** `parse_fdt`

`/tmp/kt402b/a64run/boot-1.log` (boot ktest thật của runner):

```
# bootinfo: v=1 arch=aarch64 source=dtb n_harts=4 boot_hart=0
# bootinfo: ram n=1 bytes=0x8000000
# bootinfo: ram[0]=0x40000000+0x8000000
# bootinfo: harts=0,1,2,3 timebase=none
# bootinfo: plic=none gic=0x8000000 gic_cpu=0x80a0000 apic=none uart=pl011@0x9000000 ecam=0x4010000000
# bootinfo: acpi=no pm1a=none
```

`n_harts=4`, GIC `0x8000000`/`0x80a0000`, PL011 `0x9000000`, ECAM `0x4010000000` — **tất cả** đọc
từ DTB (không hằng số). Phép kiểm mới `check_bootinfo_dtb` đòi **đúng một** dòng
`source=dtb n_harts=$EXPECT_SMP boot_hart=0` + dòng RAM + dòng GIC/UART/ECAM.

### 2.3 Ca âm — DTB hỏng ⇒ fail-closed

QEMU **từ chối** `-dtb` hỏng trước khi vào khách (`qemu_fdt_getprop: Couldn't get
//#address-cells: FDT_ERR_BADOFFSET`), nên ca âm nạp bảng hỏng bằng `-device loader` (chép **thô**,
không kiểm) — đúng mô hình đe doạ "firmware giao bảng hỏng". Bảng hỏng = `size_struct = 0` (cùng
magic, cùng `totalsize`) ⇒ parser chung trả `BadOffset("fdt-block")`:

```
$ cat /tmp/kt402b/a64run/boot-baddtb.log
TAP version 13
1..46
# bootinfo: fail-closed err=bad-offset
```

QEMU **tự thoát** (PSCI `SYSTEM_OFF` của `debug_halt(9)`) — không treo, không chạy tiếp.

### 2.4 Runner `run-aarch64.sh`: **39/39 kiểm sạch**

```
ok 11 - bootinfo: '# bootinfo: v=1 arch=aarch64 source=dtb n_harts=4 boot_hart=0' (DTB thật qua -dtb)
ok 39 - ca âm (bb): DTB hỏng ⇒ '# bootinfo: fail-closed err=…' + halt (# bootinfo: fail-closed err=bad-offset)
# ca âm: 28/28 ĐỎ đúng như phải
# 39/39 kiểm sạch (boot=1, tests=46)
```

## 3. `x86_64` — nhánh fallback `fw_cfg` (`etc/acpi/tables` + `etc/table-loader`)

### 3.1 Đo trước khi viết mã (ba phát hiện)

1. **Blob `etc/acpi/tables` là 128 KiB** (`-trace enable=fw_cfg_add_file`:
   `etc/acpi/tables (131072 bytes)`, `etc/table-loader (4096 bytes)`) — **không** cấp buffer bằng
   cả blob được (stack kernel 64 KiB). Vì vậy đọc **theo luồng**: header 36 B → `length` → thân
   bảng; bảng không cần thì bỏ qua đúng `length`.
2. **Bảng trong blob là bảng TRƯỚC khi firmware vá**: cả `FACP`/`APIC`/`MCFG` đều có checksum
   **sai** (QEMU đặt `checksum = 0` rồi ghi lệnh `ADD_CHECKSUM` vào `etc/table-loader` để firmware
   tính sau khi vá con trỏ). Đo bằng bản nháp in trạng thái checksum từng bảng: `FACP X / APIC X /
   MCFG X`. Vì vậy fallback **tính lại** checksum đúng công thức ACPI cho từng bảng (đúng bước
   firmware) rồi để parser chung kiểm chữ ký/độ dài/trường — **không** bỏ phép kiểm nào.
3. **Bố cục `etc/table-loader`**: entry **64 B**, `command` u32 ở offset 0, `char file[56]` ở
   offset 4 (đo bằng hex-dump: entry `ALLOCATE` của `etc/acpi/rsdp` ở offset 0, của
   `etc/acpi/tables` ở offset 128). Fallback dùng loader để **kiểm blob đúng là bộ bảng firmware
   được lệnh cài** (`ALLOCATE` cho `etc/acpi/tables`); thiếu ⇒ `missing-table:table-loader`.

### 3.2 Chứng minh fallback **chạy đúng** (không chỉ biên dịch)

Vì QEMU 11.1.3 **luôn** chạy SeaBIOS khi `acpi=on` (và `-bios none` bị từ chối: `could not load
PC BIOS 'none'`; BIOS rỗng thì treo; `-fw_cfg` **không** cho ghi đè tên đã có: `duplicate fw_cfg
file name: etc/acpi/rsdp`), **không** dựng được boot "không RSDP mà vẫn có bảng" bằng tham số QEMU.
Nên đã **tạm** buộc `detect` đi thẳng nhánh fallback (bản vá tạm, **đã revert** — xem §6) rồi boot
thật:

```
# bootinfo: v=1 arch=x86_64 source=acpi n_harts=4 boot_hart=0
# bootinfo: ram n=2 bytes=0x7f7ec00
# bootinfo: ram[0]=0x0+0x9fc00
# bootinfo: ram[1]=0x100000+0x7edf000
# bootinfo: harts=0,1,2,3 timebase=none
# bootinfo: plic=none gic=none gic_cpu=none apic=0xfee00000 uart=ns16550@0x3f8 ecam=0xb0000000
# bootinfo: acpi=yes pm1a=0x604
```

**Trùng từng trường** với đường RSDP (`apic=0xfee00000`, `ecam=0xb0000000`, `pm1a=0x604`,
`ram n=2`) ⇒ fallback cho **cùng** `BootInfo` như firmware đã cài bảng.

### 3.3 Ca âm — không có bảng ACPI ⇒ fail-closed **có tên**

`-M q35,acpi=off` ⇒ QEMU không dựng `etc/acpi/tables`/`etc/acpi/rsdp`, SeaBIOS không cài RSDP:

```
$ cat /tmp/kt402b/x64run/boot-noacpi.log
TAP version 13
1..47
# bootinfo: fail-closed err=missing-table:acpi-tables
```

Tên lỗi `acpi-tables` (không phải `rsdp` của bản trước khi vá) **chứng minh nhánh fallback đã
chạy** rồi mới chết — đúng yêu cầu "không rơi về hằng số QEMU".

### 3.4 Runner `run-x86_64.sh`: **39/39 kiểm sạch**

```
ok 7 - bootinfo: '# bootinfo: v=1 arch=x86_64 source=acpi n_harts=4 boot_hart=0' (MADT/FADT/MCFG thật)
ok 2 - ca âm (ff): không có bảng ACPI ⇒ '# bootinfo: fail-closed err=missing-table:acpi-tables' + halt
# ca âm: 33/33 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 39/39 kiểm sạch (boot=1, tests=47, exc=6)
```

## 4. `riscv64` — kiểm `# bootinfo: v=1 arch=riscv64 source=dtb` trên `ktest.sh`

Kernel **đã** in đúng dòng này (đo trước khi sửa, §1.3) nhưng `ktest.sh` **không** kiểm. Đã thêm
vào `kt86_rev_gate` — chỗ **duy nhất** trong `ktest.sh` boot ảnh `src/main.rs` (ảnh sản phẩm, nơi
`report_bootinfo` được gọi):

```
# bootinfo: v=1 arch=riscv64 source=dtb n_harts=1 boot_hart=0
```

Thiếu/lệch ⇒ `vbrev_refuse no-bootinfo-dtb` ⇒ `exit 3` (TỪ CHỐI ĐO) — cùng lớp với cổng danh tính
bản dựng `KT-86` (ảnh không chứng minh được nguồn bảng boot thì không đo). Regex đã đối chiếu trên
log thật `/tmp/kt402b/rv.log`: **MATCH**.

## 5. T0 / `check` / T1

- `./v2/bin/v2 t0 vifast-plat-x86_64` ⇒ **FAIL**, nhưng **không** do thay đổi của task: 5 lỗi
  `clippy -D warnings` nằm ở `vifast-plat-common` (`bootinfo.rs:679` `is_multiple_of`,
  `bootinfo.rs:1431/1440` `needless_range_loop`, `obj/check.rs:90/91`) — tệp **không** bị chạm
  (`git status --short` chỉ có 4 tệp của task). Đây đúng **lỗ hổng đã biết** ở `D2-260` mục 3:
  `steps_check.sh` chưa quét `v2/kernel/`, việc dọn `clippy` cho các crate `plat-*` được giao cho
  **`KT-409`**. `cargo clippy -p vifast-plat-x86_64 --all-targets` ⇒ **0 error**, **0** cảnh báo
  trong `vifast-plat-x86_64/src/bootinfo.rs`.
- `./v2/bin/v2 check` ⇒ **rc=0** (134 s).
- T1: xem §7.

## 6. Việc **tạm** đã làm rồi **revert** (không nằm trong diff cuối)

Để chứng minh nhánh fallback `fw_cfg` **chạy đúng** (không chỉ biên dịch), đã tạm sửa `detect` cho
đi thẳng `acpi_from_fw_cfg` (bỏ qua RSDP) + in hex `etc/table-loader`/checksum từng bảng ra COM1,
build, boot, đọc log, rồi **khôi phục nguyên trạng** (`cp` bản sạch). Ba lần đo cho ba phát hiện ở
§3.1. `git diff` cuối **không** còn dấu vết nào của bản tạm (đã kiểm bằng `git diff` + build lại
ảnh sạch: log `# bootinfo: v=1 arch=x86_64 source=acpi …` bình thường).

## 7. T1 (`gate --dev`) — chạy qua khoá máy

```
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
...
--- QEMU aarch64 (K4' bước 3): OK          (95,5 s)
ok 11 - bootinfo: '# bootinfo: v=1 arch=aarch64 source=dtb n_harts=4 boot_hart=0' (DTB thật qua -dtb)
ok 39 - ca âm (bb): DTB hỏng ⇒ '# bootinfo: fail-closed err=…' + halt (# bootinfo: fail-closed err=bad-offset)
# 39/39 kiểm sạch (boot=1, tests=46)
--- QEMU x86_64 (K5' bước 1): OK           (246,8 s)
ok 7 - bootinfo: '# bootinfo: v=1 arch=x86_64 source=acpi n_harts=4 boot_hart=0' (MADT/FADT/MCFG thật)
ok 2 - ca âm (ff): không có bảng ACPI ⇒ '# bootinfo: fail-closed err=missing-table:acpi-tables' + halt
# 39/39 kiểm sạch (boot=1, tests=47, exc=6)
...
# step-time-total   583690 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**Hai làn QEMU của chính task: OK.** T1 đỏ ở **đúng một** bước, và bước ấy **không** liên quan mã
của task:

```
--- selftest pre-measure (KT-95)
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
--- selftest pre-measure (KT-95): FAIL
```

`pre-measure.sh:209` là `[ "$DIRTY" = "0" ] || refuse 7 tree-dirty` — **cây nguồn bẩn**. Worktree
agent **bắt buộc** bẩn (luật task: không `git add`/`commit`; lead commit). Bằng chứng đây là nguyên
nhân **duy nhất** và là đặc tính của quy trình, không phải hồi quy:

```
$ cd /Users/mps/dvchd/ViFastOs && bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest: PASS (a/b/c/d/e/f/g)          # cây chính: build-inputs sạch
$ cd /private/tmp/vifast-wt/3 && bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)   # worktree: 4 tệp build-inputs chưa commit
```

⇒ **Lead commit rồi chạy lại T1/T2**: bước này sẽ xanh (không cần sửa gì).

Dòng `not ok - selftest: netsrv ca âm có dòng PANIC 'xong' …` trong log T1 là **có sẵn từ trước**,
không do task: chạy `bash v2/test/boot/net.sh --red-selftest` trên **cây chính** (`19f0a4c3`, sạch)
cũng in **đúng** dòng ấy, và làn vẫn kết luận `--- selftest validator net (KT-245): OK`
(`# net.sh --red-selftest: 34 kiểm, 0 lỗi`).

## 8. Việc còn lại / rủi ro

1. **T1 đỏ vì `tree-dirty`** (mục §7) — lead commit rồi chạy lại; không phải lỗi mã.
2. **`v2 t0 vifast-plat-*` đỏ vì `clippy -D warnings` của `vifast-plat-common`** (5 cảnh báo có sẵn
   ở `bootinfo.rs:679/1431/1440`, `obj/check.rs:90/91`) — đúng lỗ hổng `D2-260` mục 3 đã giao cho
   **`KT-409`** (nối `cargo clippy --manifest-path v2/kernel/Cargo.toml` vào `steps_check.sh`).
   `vifast-plat-x86_64/src/bootinfo.rs` (tệp task sửa) **0** cảnh báo.
3. **Nhánh fallback `fw_cfg` không thể kiểm end-to-end bằng tham số QEMU** (QEMU 11.1.3 luôn chạy
   SeaBIOS khi `acpi=on`; `-bios none` bị từ chối; BIOS rỗng treo; `-fw_cfg` không cho ghi đè tên
   đã có). Đã chứng minh bằng bản vá **tạm** (§6) rồi revert; phép kiểm **thường trực** là ca âm
   `-M q35,acpi=off` (đòi `missing-table:acpi-tables`, tức fallback **đã chạy**). Nếu lead muốn
   phép kiểm dương thường trực cho fallback thì cần một cơ chế QEMU khác (ngoài phạm vi 2 h).
4. **`etc/table-loader`**: dùng để **kiểm** blob đúng là bộ bảng firmware được lệnh cài
   (`ALLOCATE` cho `etc/acpi/tables`), **không** áp các lệnh vá con trỏ (`ADD_POINTER`) vì
   `MADT`/`FADT`/`MCFG` tự chứa; checksum được **tính lại** đúng bước firmware (`ADD_CHECKSUM`).
   Đây là quyết định có chủ ý, ghi rõ ở đầu tệp — nếu lead muốn áp đầy đủ loader thì đó là việc
   của task sau (cần bảng `ADD_POINTER` + `zone` mà QEMU 11.1.3 đóng gói khác mô tả cũ).
5. **`run-aarch64.sh`**: hunk `-dtb "$DTB"` nằm **riêng** trong `boot_full` (1 dòng) để merge dễ
   với `KT-414b`; không định dạng lại tệp, không đổi dòng không liên quan.
6. **`run-x86_64.sh`**: số hiển thị `total=$((total + 30))` → `31` cho ca âm mới (ff). Con số này
   **đã lệch sẵn** trước task (32 ca âm thật + 1 ca dương mà chỉ cộng 30) — chỉ là số in ở dòng
   tổng kết, **không** phải ngưỡng; không sửa thêm để giữ diff nhỏ.
