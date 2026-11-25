# KT-418 — SEC-1: FPU/SIMD eager theo TCB + cứng hóa `SMEP`/`SMAP`/`PAN`/`SUM` + `virtio-rng` + fuzz `vifast-net`

- **Task**: KT-418 (P1, Chặng 2 / W7, 3 h) — phụ thuộc KT-401b, KT-415.
- **Nhánh**: `task/kt418-sec1` · **Worktree**: `/private/tmp/vifast-wt/2`
- **Base**: `main` @ `fe79074d` (đã có KT-401b/415).
- **Căn cứ**: `D2-259` mục 2 (FPU/SEC-1), `D2-260` mục 2 (W^X 3 arch → `KT-401b`), `D2-17`
  (unsafe chỉ trong danh sách đóng), `QUEUE.md` dòng 470, `KE-HOACH-KIEN-TRUC.md` dòng 193.
- **Ngân sách**: 3 h. Bắt đầu 2026-11-26 03:39 JST (18:39 UTC, Thấp điểm chính).

## 0. Kế hoạch ngắn (5–10 dòng)

1. **Đo hiện trạng**: `asm/fpu.rs` (x86_64) chỉ có `read_cr0`/`write_cr0`/`read_cr4` — **chưa** có
   `fxsave`/`xsave`, **chưa** có vùng lưu theo TCB. `TcbCore` **không** có trường FPU (đúng: vùng
   lưu phải **ngoài** `TcbCore`). riscv64 **chưa** bật `sstatus.FS` ⇒ lệnh FP ở U-mode trap
   `illegal`; aarch64 đã bật `CPACR_EL1.FPEN` nhưng **chưa** lưu/khôi phục `v0..v31`.
2. **Đỏ-trước (1)**: `v2/kernel/vifast-plat-common/tests/fpu.rs` gọi `FpuTracker`/`FpuArea`
   (chưa có) ⇒ **không biên dịch được**. Sau khi cài: cấp khe theo TCB, tràn ⇒ fail-closed;
   `save` chỉ khi bẩn, `restore` chỉ khi hợp lệ; 2 TCB không lẫn vùng; 4 luồng ⇒ 0 lẫn.
3. **Đỏ-trước (2)**: test QEMU `ktest-user` TAP mới — 2 tác vụ U-mode dấu phẩy động chạy xen kẽ
   qua `Yield`, mỗi tác vụ nạp `f0..f31` giá trị riêng rồi kiểm lại sau khi được resume ⇒ **đỏ**
   khi chưa có lưu/khôi phục (thanh ghi bị tác vụ kia ghi đè).
4. **Cài (a)**: `vifast-plat-common/src/fpu.rs` — `FpuArea` (272 B, căn 16), `FpuTracker<N>`
   (khe theo TCB + cờ `valid` + bộ đếm `saves`/`restores`/`skipped`/`overflow`), thuần an toàn,
   test host.
5. **Cài (b)**: riscv64 — `fpu.rs` asm `fsd`/`fld` `f0..f31` + `fcsr`, bật `sstatus.FS=Initial`
   khi vào U-mode, lưu khi `FS=Dirty` lúc ra, khôi phục khi khe hợp lệ; hook **một cửa** ở
   `user::enter_loaded_with_root`.
6. **Cài (c)**: aarch64 — `stp/ldp q0..q31` + `fpsr`/`fpcr`, hook ở `eret_to_el0`/handler;
   x86_64 — `fxsave64`/`fxrstor64` trong `asm/fpu.rs` + hook ở `umode`.
7. **Ưu tiên 2**: bật `CR4.SMEP`/`SMAP` (x64), `SCTLR.PAN` + `PSTATE.PAN=1` (a64), `sstatus.SUM=0`
   (rv) + ca âm (đột biến tắt bit ⇒ ĐỎ).
8. **Ưu tiên 3**: `virtio-rng` + target fuzz `vifast-net` (nếu còn giờ).
9. `./v2/bin/v2 t0` sau mỗi bước → `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `spec/`; không nới ngưỡng; không `#[ignore]`/`#[should_panic]`;
không `git add/commit`; chỉ T0 + T1; không `cfg(target_arch)` mới trong kernel/runtime.

## 1. Đo hiện trạng (trước khi sửa)

```text
$ wc -l v2/kernel/vifast-plat-x86_64/src/asm/fpu.rs
      62 v2/kernel/vifast-plat-x86_64/src/asm/fpu.rs
$ grep -rn "fpu\|Fpu\|FPU" --include=*.rs v2/kernel/vifast-core/src/
(không có kết quả)          # TcbCore không có trường FPU — đúng yêu cầu "vùng lưu ngoài TcbCore"
$ grep -rn "SSTATUS_FS\|fcsr\|fld\|fsd" --include=*.rs v2/kernel/vifast-plat-riscv64/src/
(không có kết quả)          # riscv64 chưa bật FS, chưa có asm FP
$ grep -rn "CPACR_EL1_FP_SIMD" --include=*.rs v2/kernel/
v2/kernel/vifast-kbin-aarch64/src/main.rs:219: cpacr = const ...CPACR_EL1_FP_SIMD
v2/kernel/vifast-kbin-aarch64/src/main.rs:260: cpacr = const ...CPACR_EL1_FP_SIMD
                            # aarch64 bật FPEN nhưng KHÔNG lưu/khôi phục v0..v31
$ grep -rn "SMEP\|SMAP" --include=*.rs v2/kernel/
(không có kết quả)          # chưa bật SMEP/SMAP
$ grep -rn "PAN\b" --include=*.rs v2/kernel/vifast-plat-aarch64/src/sysreg.rs
SCTLR_EPAN ...              # có hằng EPAN nhưng chưa bật PAN ở PSTATE
```

Đo ảnh (baseline trước khi sửa):

```text
$ bash v2/test/boot/budget.sh <ktest-user> ktest-user
# image=ktest-user ... data+bss=15160 budget=262144 ... corespace=184840
```

Kết luận: cả ba arch **chưa** có ngữ cảnh FPU theo TCB; riscv64 còn **chưa bật** FP cho U-mode
(lệnh FP ở U-mode ⇒ `illegal instruction`). Đây là lỗ hổng đúng phạm vi KT-418.

## 2. Đỏ-trước (1) — `vifast_plat_common::fpu` chưa tồn tại

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test fpu
error[E0432]: unresolved import `vifast_plat_common::fpu`
  --> vifast-plat-common/tests/fpu.rs:21:25
   |
21 | use vifast_plat_common::fpu::{
   |                         ^^^ could not find `fpu` in `vifast_plat_common`
error: could not compile `vifast-plat-common` (test "fpu") due to 1 previous error
```

Đỏ **đúng lý do**: module chưa tồn tại (không phải test sai kỳ vọng).

## 3. Cài (a) — `vifast-plat-common/src/fpu.rs` (bookkeeping thuần, host test)

`FpuArea` (272 B, căn 16) + `FpuTracker<N>` (khe theo TCB, cờ `valid`, bộ đếm
`saves`/`skipped`/`restores`/`fresh`/`overflow`). Tràn khe ⇒ **fail-closed** (không cấp, không
ghi đè, đếm `overflow`). `FPU_SLOTS = 65` = `MAX_TCBS` 64 + 1 (`ObjId` 1-based).

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test fpu
running 8 tests
test bo_dem_save_restore_skip ... ok
test hai_tcb_khong_lan_vung ... ok
test dem_luot_vao_ra_khop ... ok
test khe_theo_tcb_khong_lan ... ok
test tran_khe_fail_closed ... ok
test valid_chi_sau_khi_luu ... ok
test vung_luu_dung_co_va_can_16 ... ok
test bon_luong_khong_lan ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 4. Cài (b) — riscv64: `fs0..fs31` + `fcsr`, `sstatus.FS`, hook **một cửa**

- `v2/kernel/vifast-plat-riscv64/src/fpu.rs`: `fsd`/`fld` 32 thanh ghi + `frcsr`/`fscsr`;
  `read_fs`/`set_fs` trên `sstatus.FS[14:13]`; `restore_current`/`save_current`; `AREAS`
  `[FpuArea; 65]` = **17.680 B `.bss`** (đo dưới), **ngoài** `TcbCore`.
- Hook **một cửa** ở `user::enter_loaded_with_root`: `restore_current()` ngay trước `user_enter`,
  `save_current()` ngay sau khi nó trả về ⇒ mọi ảnh/đường vào U-mode đi qua đúng một lần.
- Đo trước khi sửa: ảnh kernel **0** lệnh FP (`llvm-objdump -d | grep -c 'fld|fsd|fadd|…'` = 0)
  ⇒ kernel không dùng thanh ghi FP, `FS` chỉ do U-mode đổi ⇒ theo dõi bẩn bằng phần cứng đúng.

## 5. Đỏ-trước (2) + xanh — TAP 38 `fpu hai tac vu khong lan thanh ghi`

Chương trình U-mode mới `init/fpu_user.S` (+ `fpu_user.rs`, build.rs sinh `fpu.elf`): nạp
`f0..f31` = `(task << 32) | (0x100 + reg)`, `Yield` 8 lần; sau **mỗi** lần resume **kiểm lại**
toàn bộ 32 thanh ghi rồi **ghi lại** mẫu (ghi ⇒ `FS = Dirty` ⇒ kernel buộc phải lưu). Lệch ⇒
`Exit(1)`. Kernel chạy **hai** tác vụ xen kẽ (`ktest_fpu.rs`, round-robin như `ktest_capa`).

**Đột biến chứng minh TAP có răng** (tắt `restore_current`, build lại, boot):

```text
# fpu: tcb_a=87 tcb_b=89 slot_a=1 slot_b=2 valid_a=1 valid_b=1 saves=11 skipped=113 restores=0 fresh=0 overflow=0 steps=11 code_a=1 code_b=0
not ok 38 - fpu hai tac vu khong lan thanh ghi
```

`restores=0` + `code_a=1` (tác vụ A thấy thanh ghi bị tác vụ B ghi đè) ⇒ **đỏ đúng lý do**.
Khôi phục `restore_current` ⇒ xanh:

```text
$ qemu-system-riscv64 -icount shift=6 -machine virt,aia=none,aclint=on \
    -bios opensbi-riscv64-generic-fw_dynamic.bin -kernel vifast-kbin-ktest-user \
    -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
# fpu: tcb_a=87 tcb_b=89 slot_a=1 slot_b=2 valid_a=1 valid_b=1 saves=19 skipped=112 restores=16 fresh=115 overflow=0 steps=18 code_a=0 code_b=0
ok 38 - fpu hai tac vu khong lan thanh ghi
PANIC: KT-70 u-mode pha syscall, invocation, capalloc, IDL va ring va pager va fpu xong (38 TAP, 0 fail)
```

**4 hart** (`-smp 4`, cùng ảnh): `harts=4` + `ok 38` + cùng bằng chứng `# fpu:` (khe 1/2, 0 tràn)
⇒ 2 tác vụ U-mode dấu phẩy động trên máy 4 hart **không lẫn thanh ghi**.

## 6. Ưu tiên 2 — cứng hóa `SUM`/`SMEP`/`SMAP`/`PAN` + ca âm phần cứng

### 6.1 riscv64 — `sstatus.SUM = 0` (TAP 3 ảnh `ktest-guard`)

- `mmu::SSTATUS_SUM` + `sum_enabled()` + `set_sum(bool)` (chỉ `ktest`); `map_scratch_user_4k`
  (leaf `PTE_USER`) tách từ `map_scratch_4k` bằng thân chung `map_scratch_4k_flags`.
- Ca kiểm: map trang `.bss` 4 KiB **`U=1`** vào root kernel rồi `probe_fault(Load, 0x1000)`:
  - `SUM = 0` (mặc định) ⇒ **load page fault thật** `scause = 13`, `stval = 0x1000`, `SPP = 1`;
  - `SUM = 1` (bật tạm) ⇒ **không** fault ⇒ chứng minh fault trên là do `SUM`, không phải trang
    không map;
  - `SUM` trả về `0` sau phép đo.

```text
$ qemu-system-riscv64 … -kernel vifast-kbin-ktest-guard -smp 1
ok 1 - guard image
ok 2 - guard load fault
# sum: va=0x1000 pa=0x80205000 mapped=1 sum_before=0 fault_scause=13 fault_stval=0x1000 fault_spp=1 sum1_no_fault=1 sum_after=0 ok=1
ok 3 - sum=0 s-mode khong doc trang user
ok 4 - guard store fault
PANIC: guard page fault scause=0xf stval=0x80400008 sepc=0x80200ea6 sstatus=0x8000000200006100
```

### 6.2 x86_64 — `SMEP` bật + ca âm; `SMAP` **hoãn có tên** (TAP 50)

- `asm::cpuid_leaf7_ebx()` (cứu/khôi phục `rbx` — LLVM giữ riêng), `asm::fpu::enable_smep_smap()`
  bật `CR4.SMEP` **chỉ khi** `CPUID.07H:EBX[7]`, đọc lại `CR4` làm bằng chứng.
- `asm::probe_read8`/`probe_exec` (+ nhãn resume) + `sec_probe.rs`: `#PF` thật từ CPL0.
- **Đo được**: model mặc định `qemu64` **không** có SMEP/SMAP (`cpuid_smep=0 cpuid_smap=0`) ⇒
  TAP xanh theo nhánh "không có gì để bật" (`hardened=1`), log ghi rõ `cpuid_*=0`.
- **Chứng minh ca âm thật** bằng `-cpu max` (có SMEP/SMAP):

```text
$ qemu-system-x86_64 -M q35 -cpu max … -kernel vifast-kbin-x86_64
# smep/smap: cpuid_smep=1 cpuid_smap=1 smep=1 smap=0 smap_deferred=1 cr4=0x0000000000100020 …
# smep/smap: va=0x1000000000 … smep_err=0x11 smep_cr2=0x1000000000 smep_ok=1 hardened=1
ok 50 - smep/smap: bat va ca am
```

`err=0x11` = `P=1` + `I/D=1` (instruction fetch bị từ chối) — đúng `SMEP`; `CR2` = đúng VA trang
`US=1`. **Bài học đo được**: bit `U/S` của `#PF` là **chế độ truy cập** (0 = CPL0), **không** phải
`U/S` của trang ⇒ vi phạm `SMEP`/`SMAP` từ CPL0 có `U/S = 0`.

- **`SMAP` chưa bật — nợ có tên**: kernel **đọc/ghi trang `US=1` từ CPL0** ở nhiều đường hợp lệ
  (`umode::run` đọc `VSPACE_PROBE_VA` cho phép đo `INVLPG`, loader ELF ghi trang user, IPC
  buffer). Bật `SMAP` mà không bọc `stac`/`clac` thì boot `-cpu max` **chết** ngay phép đo
  `INVLPG` — đo được: `# exc: vec=14 err=0x1 rip=0x110a14` (đọc `0x1000004000`) rồi halt. Vì vậy
  KT-418 bật **SMEP** (kernel không bao giờ thực thi trang user ⇒ an toàn tuyệt đối) và **giữ
  `SMAP` tắt**, ghi nợ: bọc `stac`/`clac` cho các đường chạm trang user rồi mới bật `CR4.SMAP`.
- **Vá phụ (đo được)**: `exc_arm` phải **xoá `EXPECT_CR2`** — không thì `EXPECT_CR2` còn sót từ
  phép đo cô lập `VSpace` **nuốt** fault của ca mới (`# vspace: BAD fault vec=14 err=0x11
  cr2=0x1000000000 cs=0x8 expect_cr2=0x1000003000`).

### 6.3 aarch64 — `PAN` bật + ca âm data abort (TAP 50)

- `asm::pan_set`/`pan_get` (mã hoá `S3_0_C4_C2_3` — assembler LLVM 1.100 **không** nhận tên
  `pan`), `asm::probe_read64` (một `ldr` 4 byte ⇒ handler resume `ELR + 4`).
- `pan_probe.rs`: `PAN = 1` ⇒ `ldr` từ `EL0_CODE_VA` (trang `USER_RX`) ⇒ **data abort cùng EL**
  (`EC = 0x25`, `FAR` đúng VA, `Access::Read`); `PAN = 0` ⇒ cùng `ldr` **không** fault; `PAN` trả
  về `1`.
- **Vì sao bật được toàn cục** (khác `SMAP` của x86_64): loader ELF aarch64 ghi nội dung qua
  **PA** (identity map, supervisor) rồi mới `map_obj_with` sang VA `USER_*`; trang `.el0` lấy
  thẳng từ ảnh ⇒ kernel **không** đọc/ghi bằng VA user.

## 7. Ưu tiên 3 — fuzz target `vifast-net` (đã xong); `virtio-rng` **chưa** làm

### 7.1 `v2/lib/vifast-net/tests/fuzz.rs` — fuzz tất định, không dependency

- RNG tự viết (xorshift64* — **không** thêm crate ngoài, D2-21); cùng seed ⇒ cùng chuỗi ⇒ tái lập
  được trên mọi máy, **không** `#[ignore]`.
- **20.000** chuỗi (stable) / **200** chuỗi (dưới Miri — cùng lý do `vifast-objmem/tests/fuzz.rs`).
- Ba bất biến trên **mọi** chuỗi byte (kể cả rỗng, 1 byte, toàn `0x00`/`0xFF`, mọi tiền tố):
  1. **không panic** — đường từ chối dịch vụ từ một gói rác;
  2. **round-trip idempotent** — parse thành công ⇒ `encode` lại ⇒ parse lại cho **cùng** kết quả
     (bắt "parser chấp nhận dữ liệu mà encoder không tái tạo được");
  3. **kiểm chặt hơn không bao giờ lỏng hơn** — `parse(buf, true)` thành công ⇒ `parse(buf, false)`
     cũng phải thành công.
- Parser phủ: `EthHeader`, `EthernetFrame`, `ArpPacket`, `Ipv4Header`, `UdpHeader`, `TcpHeader`,
  `IcmpEcho`, `DhcpMessage`, `DnsMessage` (+ `parse_response`).
- **Giới hạn ghi rõ, không nới bất biến**: `Ipv4Header::encode` chỉ ghi 20 byte header (gói có
  option `IHL > 5` không tái tạo được) và `TcpHeader::encode` chỉ hỗ trợ `data_offset = 5` ⇒ phép
  round-trip **bỏ qua riêng** hai trường hợp đó, các parser còn lại vẫn chạy.

### 7.2 Chứng minh test **có răng** (đột biến, đã khôi phục)

Sửa `EthHeader::parse` lệch 1 byte (`buf.len() < ETH_HEADER_LEN - 1`):

```text
thread 'fuzz_buf_dac_biet' panicked at lib/vifast-net/src/eth.rs:83:53:
index out of bounds: the len is 13 but the index is 13
thread 'fuzz_parser_khong_panic_va_round_trip' panicked at lib/vifast-net/src/eth.rs:83:53:
index out of bounds: the len is 13 but the index is 13
test result: FAILED. 0 passed; 2 failed
```

Khôi phục từ `/tmp/kt418-eth.rs.bak`; `git diff --stat v2/lib/vifast-net/src/eth.rs` rỗng.

### 7.3 `virtio-rng` — **chưa làm** (hết ngân sách sau T1)

Không có mã nào cho `virtio-rng` trong cây này (`grep -rn "rng" v2/lib/vifast-virtio/src/` rỗng).
Đây là **nợ chuyển tiếp**, không phải "đã làm một phần": cần thêm device id 4 vào
`vifast-virtio`, driver ring 1 hàng đợi, nguồn entropy cho `vifast-core` (hiện chưa có API), và
TAP riêng trên cả ba arch. Đề xuất tách thành task riêng (ước lượng 2–3 h) thay vì nhét vào KT-418.

## 8. T0 / `v2 check` / T1

### 8.1 T0 — `./v2/bin/v2 t0`

- **Crate host-testable (xanh)**: `./v2/bin/v2 t0 vifast-net vifast-plat-common` — `vifast-net`
  clippy sạch + **11 bộ test** xanh (gồm `tests/fuzz.rs`: 2 ca, 20.000 chuỗi); `vifast-plat-common`
  clippy sạch + 3 bộ test xanh (gồm `tests/fpu.rs`: 8 ca).
- **Crate kernel (T0 host ĐỎ — hạn chế có sẵn của `v2 t0`, không phải hồi quy)**: `v2 t0` chạy
  `cargo clippy/test -p <crate>` **không** `--target`, nên crate `no_std` + asm theo arch không
  build được trên host:
  - `vifast-kbin-aarch64`: `error[E0152]: found duplicate lang item panic_impl` +
    `unwinding panics are not supported without std`;
  - `vifast-kbin-riscv64`: `invalid register a7` / `invalid Mach-O section specifier`
    (`.corespace`) — asm riscv64 biên dịch cho host x86_64;
  - `vifast-kbin-x86_64`: `unused import` khi chỉ bật `ktest` (tập feature khác cổng).
  Bằng chứng thay thế **đúng target**: `cargo build --target <arch> --release` xanh cho cả ba
  arch (mục 5.4, 6.2, 6.3) + `cargo clippy --target <arch>` chỉ còn lỗi **có sẵn** ngoài phạm vi
  KT-418 (`mapwin.rs` `is_multiple_of`, `main.rs:178` vòng `fp.bytes[i]`, unused import của
  `pshell.rs`/`main.rs` x86_64) — **không** có lỗi nào ở tệp KT-418 thêm/sửa.

### 8.2 `./v2/bin/v2 check`

```text
# step-time-total    97355 ms  TỔNG check
```
Exit 0 — xanh (fmt + clippy workspace v2 + luật cấu trúc + gen-check + fixtures + giấy phép).

### 8.3 T1 — `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev`

```text
--- wake paths + --strict (KT-151/155 — KT-333e: một lượt): OK
--- wake theo dep-info (KT-217: lớp 2): OK
--- test có răng (KT-163): OK
--- build (L0 compile): OK
--- L0 test: OK
--- L0 kernel workspace (KT-148): OK
--- L1 (sim 1.000 seed): OK (1318 ms, đo bởi cổng)
--- QEMU aarch64 (K4' bước 3): OK      # 39/39 kiểm sạch (boot=1, tests=50)
--- QEMU x86_64 (K5' bước 1): OK       # 39/39 kiểm sạch (boot=1, tests=50, exc=6)
--- selftest pre-measure (KT-95): FAIL
# step-time-total   485703 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**Một** bước đỏ, và nó **đỏ vì cây bẩn — đúng thiết kế**, không phải hồi quy:

```text
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
```

Tái lập bằng tay (ảnh giả + dấu vân tay **thật** của cây):

```text
# dấu vân tay: ảnh=cb850331aceb1f83 cây=cb850331aceb1f83
# cây nguồn (build-inputs): dirty=1
PRE-MEASURE: REFUSE reason=tree-dirty
rc=7
```

`pre-measure.sh` **từ chối đo** khi cây bẩn (`refuse 7 tree-dirty`) — đúng luật "không đo trên cây
bẩn". Worktree của agent **luôn** bẩn (lead commit sau), nên bước này chỉ xanh sau khi lead commit.
**Không** nới ngưỡng, **không** tắt bước, **không** retry để lấy xanh (D2-07).

### 8.4 Bài học đo được (ghi để task sau không lặp)

1. **`PAN` không có trên `cortex-a72`** (ARMv8.0): `ID_AA64MMFR1_EL1.PAN = 0` ⇒ `msr pan` là
   `#trap ec=0x00` và boot chết. Phải kiểm `pan_supported()` trước (fail-safe như `CPUID` của
   `SMEP`). Runner aarch64 dùng `-cpu cortex-a72` ⇒ nhánh ca âm **không chạy** trên cổng; log ghi
   `pan_supported=0` để không ai đọc thành "đã kiểm PAN".
2. **Assembler LLVM 1.100 không nhận tên `pan`** ở cả `msr`/`mrs` ⇒ phải dùng **mã hoá thô**
   `.inst 0xD503417F` (`msr pan, #1`) / `.inst 0xD503407F` (`msr pan, #0`), đối chiếu công thức
   với `msr spsel, #1` = `0xD50041BF` và `msr daifset, #2` = `0xD50342DF`.
3. **`mod` mới phải `#[cfg(feature = "ktest")]`**: `sec_probe`/`pan_probe` dùng `crate::exc_*`/
   `crate::faults`/`crate::tap` (đều `ktest`) ⇒ thiếu `cfg` thì `cargo check --workspace --target
   riscv64gc-unknown-none-elf` (bước `wake theo dep-info`) **ĐỎ** vì build crate x86_64/aarch64
   cho target riscv64.
4. **`#[cfg]` đặt sai chỗ giết code**: chèn khối `SSTATUS_SUM` ngay **giữa** `#[cfg(feature =
   "ktest")]` và `fn sfence_all` làm hằng số bị cfg ẩn ⇒ `cannot find value SSTATUS_SUM`.
5. **`#PF` `err` bit `U/S` là chế độ truy cập**, không phải `U/S` của trang ⇒ `SMEP` từ CPL0 cho
   `err=0x11` (P=1, I/D=1, U/S=0), không phải `0x15`.

## 9. Trạng thái bàn giao

- Nhánh `task/kt418-sec1`, worktree `/private/tmp/vifast-wt/2`, base `main` @ `fe79074d`.
- **Không** commit (lead commit). `git status --short`: **23 tệp sửa** (+690/−18) + **11 tệp mới**.
- **0 thay đổi `spec/`**; **0** `#[ignore]`/`#[should_panic]`; **0** nới ngưỡng; **0** sửa
  `accept()`/kỳ vọng/snapshot harness (chỉ **thêm** dòng kỳ vọng cho TAP mới — đúng hướng mở rộng).
- **Không** chạy T2/nightly/soak/L1 medium-full/`mutants --all`.
- **Đã xong**: ưu tiên 1 (FPU/SIMD eager theo TCB, 2 tác vụ U-mode FP trên 4 hart, 0 lẫn thanh ghi,
  có đột biến chứng minh), ưu tiên 2 (riscv64 `SUM=0` + ca âm; x86_64 `SMEP` bật + ca âm, `SMAP`
  hoãn có tên; aarch64 `PAN` bật + ca âm, cổng kiểm `pan_supported`), ưu tiên 3 **một nửa**
  (fuzz `vifast-net` xong; `virtio-rng` **chưa**).
- **T1**: 1 bước đỏ duy nhất `selftest pre-measure (KT-95)` — `REFUSE reason=tree-dirty`, đúng
  thiết kế trên cây bẩn; mọi bước khác **OK** (gồm `QEMU aarch64` 39/39 và `QEMU x86_64` 39/39).
