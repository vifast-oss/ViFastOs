# KT-343 — 25/28 parity v1: 11 lệnh `deferred-K7` đi cùng một hợp đồng 3 arch

**Nhánh:** `task/kt343-parity` · **Base:** `main` @ `55d35b49` (KT-342) · **Worktree:** `/private/tmp/kt343-s1`
**Ngày:** 2026-11-23 · **Sub-agent:** `parity343` · **Ngân sách:** 3 h (D2-233/D2-239/D2-242/D2-245/D2-247)
**Tệp journal duy nhất:** `v2/agent/JOURNAL/kt343-parity.md` (D2-245).

## 0. Chốt một câu

Đã đóng **11 lệnh `deferred-K7`** (`mkdir`/`rm`/`mv`/`ln`/`fsck`/`uname`/`history`/`!!`/`run`/`runfs`/`exec`)
trên ảnh sản phẩm `riscv64` bằng **cùng** giao thức FS/Exec đã có (không mở ABI, không sửa `spec/`),
đo **25/28** lệnh parity (14 active cũ + 11 mới; còn đúng `free`/`df`/`loop` cho `KT-345b`),
với **43 ca product riscv64** + **2 host test FS** + **11/11 serial ở `smp=1` và `smp=4`** và
**2 đột biến ĐỎ**. Không nới ngưỡng nào; `shell.rs` **1.411** dòng (R1 ≤ 1.500); `.data+.bss`
sản phẩm **224.472 B** (trần 262.144, headroom **37.672 B**) — **không đổi**.

## 1. Bảng 28 — ABI v1.1 × 3 arch (nguồn: `spec/v1-parity.toml` = 28 lệnh người dùng; `spec/abi.toml` = 16 syscall + object method bên dưới)

Ghi chú cột: **có** = đường chạy + ca kiểm trong task; **khác** = có nhưng đầu ra/ngữ nghĩa khác v1
(parity một phần, đã ghi trong spec `note`); **không** = chưa có đường chạy trên arch đó.

| # | Lệnh (v1) | riscv64 trước | riscv64 sau | aarch64 | x86_64 | Ghi chú |
|---:|---|---|---|---|---|---|
| 1 | `help` | có (khác) | có (khác) | có (khác) | không | v2 in `cmds=N` + danh sách; v1 in bảng 13+ lệnh |
| 2 | `echo` | có (khác) | có (khác) | có (khác) | không | v2 có tiền tố `# shell:`; v1 in text trần |
| 3 | `ps` | có (khác) | có (khác) | có (khác) | không | v1 `<slot> <prio> <state>`; v2 `pid=/state=` |
| 4 | `kill` | có (khác) | có (khác) | có (khác) | không | v1 `KILL EXEC OK/SKIP`; v2 `kill pid=N code=0` |
| 5 | `cd` | có | có | không | không | chỉ `shell_fs` (ảnh sản phẩm); aarch64 shell không FS |
| 6 | `pwd` | có | có | không | không | như trên |
| 7 | `time` | có (khác) | có (khác) | không | không | v1 tick; v2 `ns=` qua `ClockRead` 11 |
| 8 | `ls` | có | có | không | không | FS kernel (`proc_fs`) |
| 9 | `cat` | có | có | không | không | như trên |
| 10 | `mkdir` | **không** | **có** | không | không | **KT-343**: `OP_MKDIR` → `Fs::mkdir` |
| 11 | `rm` | **không** | **có** | không | không | **KT-343**: `OP_RM` → `Fs::unlink` |
| 12 | `mv` | **không** | **có** | không | không | **KT-343**: `OP_MV` + `OP_PATH2` → `Fs::rename` |
| 13 | `ln` | **không** | **có** | không | không | **KT-343**: `OP_LN` + `OP_PATH2` → `Fs::link` |
| 14 | `stat` | có | có | không | không | v1 `<size> bytes dir/file`; v2 thêm `ino=` |
| 15 | `sync` | có | có | không | không | `OP_SYNC` (fsync + flush thiết bị) |
| 16 | `truncate` | có | có | không | không | `OP_TRUNC` |
| 17 | `fsck` | **không** | **có** | không | không | **KT-343**: `OP_FSCK` → `Fs::fsck` |
| 18 | `free` | không | không | không | không | **để `KT-345b`** — cần `MemSvc`/`meminfo.vidl` |
| 19 | `df` | không | không | không | không | **để `KT-345b`** — cần `OP_STATFS`/`fsstat.vidl` |
| 20 | `uptime` | có (khác) | có (khác) | không | không | v1 tick; v2 `ms=` |
| 21 | `uname` | **không** | **có** | không | không | **KT-343**: `ARCH_NAME` từ shim (`ViFastOs <arch> bootinfo-v3`) |
| 22 | `run` | có (khác) | có (khác) | không | không | KT-342: exec kernel-mediated, bảng ảnh nhúng `/hello.elf` (không đọc FS) |
| 23 | `runfs` | **không** | **có** (khác) | không | không | **KT-343**: cùng đường `run`; **bỏ qua args** (v1 truyền args) |
| 24 | `exec` | **không** | **có** (khác) | không | không | **KT-343**: bí danh `runfs`; **bỏ qua args** |
| 25 | `jobs` | có (khác) | có (khác) | không | không | đếm 4 nhóm từ `PS_REQ` |
| 26 | `loop` | không | không | không | không | **để `KT-345b`** — giao thức loop ở root chưa có |
| 27 | `history` | **không** | **có** (khác) | không | không | **KT-343**: giữ **một** dòng (`last=`), v1 giữ danh sách |
| 28 | `!!` | **không** | **có** | không | không | **KT-343**: chạy lại `last`; không đệ quy vô hạn |

**Đếm:** riscv64 **14/28 → 25/28** (11 lệnh mới); aarch64 **4/28** (`help`/`echo`/`ps`/`kill` — shell
`ktest`, không FS); x86_64 **0/28** (chưa có shell sản phẩm; phụ thuộc `KT-341`, ngoài base `55d35b49`).

### 1.1 Đúng **3** cái để `KT-345b` (tên + lý do)

1. **`free`** — cần dịch vụ `MemSvc` + `spec/protocols/meminfo.vidl` (`total`/`free`); ABI v1.1 **không**
   thêm syscall, `KT-345b` dựng dịch vụ cùng khuôn `netsrv` (D2-242 nhóm 4C).
2. **`df`** — cần `OP_STATFS` + `spec/protocols/fsstat.vidl` (`blocks`/`blocks_free`/`block_size`);
   hiện `proc_fs` chỉ có `OP_STAT` cho một đường dẫn, chưa có thống kê toàn volume.
3. **`loop`** — cần giao thức `loop` ở root (spawn/destroy N lần) chưa có trên v2; không phải
   syscall ABI mới, nhưng cần procsrv + vòng đời TCB — đúng phạm vi `KT-345b`/`KT-347`.

## 2. Thay đổi (tệp + dòng)

| Tệp | +/− | Nội dung |
|---|---:|---|
| `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs` | +20 | 6 opcode FS mới: `OP_MKDIR` 0x17, `OP_RM` 0x18, `OP_MV` 0x19, `OP_LN` 0x1a, `OP_FSCK` 0x1b, `OP_PATH2` 0x1c |
| `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs` | +104/−1 | `path2` + `load_path2`/`take_path2`; 5 handler (`Fs::mkdir/unlink/rename/link/fsck`); mọi lỗi → `err_code` **cũ** (một bảng mã) |
| `v2/kernel/vifast-plat-user/programs/shell_cmds.rs` | **+243 (mới)** | 8 lệnh `shell_fs` (`mkdir`/`rm`/`mv`/`ln`/`fsck`/`uname`/`history`/`!!`) + `fs_set_path2`; tách khỏi `shell.rs` để giữ **R1** |
| `v2/kernel/vifast-plat-user/programs/shell.rs` | +156/−36 | dispatch 8 lệnh + `runfs`/`exec`; `history` một dòng; `CMDS` 29 (ảnh đầy đủ); **1.411 dòng** |
| `v2/kernel/vifast-plat-user/lib.rs` | +16/−8 | `Sys::ARCH_NAME` (`riscv64`/`aarch64`) cho `uname` — một chỗ biết target |
| `v2/kernel/vifast-kbin-riscv64/build.rs` | +1 | `rerun-if-changed` cho `shell_cmds.rs` |
| `v2/lib/vifast-fs/tests/kt343_commands_host.rs` | **+134 (mới)** | 2 host test: hợp đồng 5 thao tác + `rename` thay thế / `link` từ chối đích |
| `v2/test/boot/product-riscv64.sh` | +62/−8 | bơm 20 lệnh + 2 boot `runfs`/`exec`; 43 phép kiểm |
| `v2/test/parity/run.sh` | +48/−6 | `--extra`, `--extra-only`, `--timeout-ds` (spec/ **không** sửa) |
| `v2/test/parity/kt343-extra.toml` | **+85 (mới)** | 11 mục active tạm đo serial (chờ Agent 0 lật spec) |

Không chạm: `spec/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`, `.github/**`,
19 tệp `[[legacy]]`, `main`.

## 3. Bằng chứng

### 3.1 Host test FS (arch-independent — cả 3 arch dùng chung `vifast-fs`)

```text
$ cargo test --manifest-path v2/lib/vifast-fs/Cargo.toml --test kt343_commands_host
running 2 tests
test kt343_mkdir_rm_mv_ln_fsck_contract ... ok
test kt343_rename_replaces_but_link_refuses_existing ... ok
test result: ok. 2 passed; 0 failed
```

Khoá **mã lỗi**: `mkdir` lặp ⇒ `Exists` (6); thiếu cha ⇒ `NotFound` (5); `rm` thư mục còn con ⇒
`NotEmpty` (9); `ln` nguồn thiếu ⇒ 5; `ln` thư mục ⇒ `IsDir` (8); `mv` nguồn thiếu ⇒ 5; `fsck` sạch.

### 3.2 Parity serial 11 lệnh mới — `smp=1` **và** `smp=4` (ảnh sản phẩm FS+exec, không net)

```text
$ bash v2/test/parity/run.sh --mode serial --extra-only --image /tmp/kt343-product-fs-exec \
    --extra v2/test/parity/kt343-extra.toml --smp 1 --timeout-ds 60
ok 1 - mkdir ... ok 11 - exec
# parity: total=11 ok=11 deferred=0 active=11 spec_total=0 extra=11 (active_da_chay=11)
# parity: KET_LUAN=ok (exit 0)
```

`--smp 4` cho **đúng** cùng kết quả: `ok 1..11`, `total=11 ok=11`, `KET_LUAN=ok (exit 0)`.

### 3.3 Ảnh sản phẩm riscv64 — 43 phép kiểm

Log đầy đủ: `/tmp/kt343-product-smp1-prefix.log` (trước commit) và log của lần chạy sau commit.
`grep -c '^ok '` = **43**, `grep -c '^not ok '` = **0**. Gồm 5 FS + 3 meta + `run`/`runfs`/`exec`
(mỗi exec một boot riêng vì exec **thay ảnh**), net + FS + halt giữ nguyên KT-342.
`.data+.bss` **224.472 B** / 262.144 (budget.sh), `.markers=0`.

## 4. Hai đột biến — cả hai ĐỎ

### (i) Sai mã lỗi syscall/FS (thay `NotFound` 5 bằng `Invalid` 2) ⇒ ca parity ĐỎ

Sửa `vifast-fs/src/dir.rs::unlink` `.ok_or(FsError::NotFound)` → `.ok_or(FsError::Invalid)`, chạy
`product-riscv64.sh --smp 1`:

```text
not ok 1 - fs: rm mục không có ⇒ NotFound 5 (thiếu /^# shell: rm path=/parity loi code=5$/)
# product-riscv64: checks=0 fails=1 ...
```

Khôi phục ⇒ 2 host test PASS và product **43 ok / 0 not ok**.

### (ii) Bỏ kiểm quyền cap ở một syscall (`Tcb.SetTls`) ⇒ ca âm ĐỎ

Bỏ khối `if c.rights & RIGHT_WRITE == 0 { … InsufficientRights }` trong
`vifast-core/src/invoke.rs::set_tls`:

```text
thread 'tls_tests::set_tls_thieu_quyen_ghi' panicked:
assertion `left == right` failed
  left: Ok((0, 0))
 right: Err(3)
test result: FAILED. 0 passed; 1 failed
```

Khôi phục ⇒ `1 passed` (đã chạy lại).

## 5. Cổng (chạy trên cây sạch sau commit — xem §6)

<!-- GATES -->
