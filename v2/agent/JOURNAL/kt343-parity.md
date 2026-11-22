# KT-343 — 25/28 parity v1: 11 lệnh `deferred-K7` → `active` trong `spec/v1-parity.toml`

**Nhánh:** `task/kt343-parity` · **Base:** `main` @ `387b8f9e` (sau `D2-250`, `KT-341`, `KT-342a`) ·
**Worktree:** `/private/tmp/kt343-s1` · **Ngày:** 2026-11-23 · **Sub-agent:** `parity343`
**Ngân sách:** 3 h (D2-233/D2-239/D2-242/D2-245/D2-247/D2-250) · **Tệp journal duy nhất:** tệp này.

## 0. Chốt một câu

Theo **D2-250**, đã lật **11 lệnh `deferred-K7` → `active` trong `spec/v1-parity.toml`**
(⇒ **25/28 active**, giữ nguyên `spec/abi.toml`), cài **5 thao tác FS + 3 lệnh mốc + 3 đường exec**
trên ảnh sản phẩm (cùng giao thức FS/Exec đã có, không mở ABI), và đo **parity `--mode serial`
25/25** trên ảnh sản phẩm ở **`smp=1`** (và `smp=4` — xem §3.2), kèm **ca âm từng nhóm**
(FS: `Exists`/`NotFound`; meta: `last` rỗng/`!! none`; exec: `run|runfs|exec` đường dẫn lạ ⇒
`err=<n>`, **không PANIC**). **2 đột biến ĐỎ**; R1 giữ **1.411** dòng cho `shell.rs`;
`.data+.bss` sản phẩm **224.472 B** / 262.144 (headroom **37.672 B**).

## 1. Bảng 28 — ABI v1.1 × 3 arch

Nguồn 28 lệnh: `spec/v1-parity.toml` (22 builtin + 6 applet v1); `spec/abi.toml` là 16 syscall +
object method bên dưới (không sửa). **Trước** = base `55d35b49`; **sau** = commit này.
**có** = đường chạy + ca kiểm; **khác** = có nhưng khác dạng/ngữ nghĩa v1 (ghi `note` trong spec);
**không** = chưa có đường chạy trên arch đó.

| # | Lệnh (v1) | riscv64 trước | riscv64 sau | aarch64 | x86_64 | Ghi chú |
|---:|---|---|---|---|---|---|
| 1 | `help` | có (khác) | **active** (khác) | có (khác, ktest) | có (khác) | v2 in `cmds=N` + danh sách; v1 in bảng 13+ lệnh |
| 2 | `echo` | có (khác) | **active** (khác) | có (khác, ktest) | có (khác) | tiền tố `# shell:` |
| 3 | `ps` | có (khác) | **active** (khác) | có (khác, ktest) | có (khác) | v1 `<slot> <prio> <state>`; v2 `pid=/state=` |
| 4 | `kill` | có (khác) | **active** (khác) | có (khác, ktest) | có (khác) | v1 `KILL EXEC OK/SKIP`; v2 `code=0` |
| 5 | `cd` | có | **active** | không | không | chỉ nhánh `shell_fs` |
| 6 | `pwd` | có | **active** | không | không | như trên |
| 7 | `time` | có (khác) | **active** (khác) | không | không | v1 tick; v2 `ns=` (`ClockRead` 11) |
| 8 | `ls` | có | **active** | không | không | dịch vụ FS kernel (`proc_fs`) |
| 9 | `cat` | có | **active** | không | không | như trên |
| 10 | `mkdir` | **không** | **active** | không | không | `OP_MKDIR` → `Fs::mkdir` |
| 11 | `rm` | **không** | **active** | không | không | `OP_RM` → `Fs::unlink` |
| 12 | `mv` | **không** | **active** | không | không | `OP_MV` + `OP_PATH2` → `Fs::rename` |
| 13 | `ln` | **không** | **active** | không | không | `OP_LN` + `OP_PATH2` → `Fs::link` |
| 14 | `stat` | có | **active** | không | không | v2 thêm `ino=` |
| 15 | `sync` | có | **active** | không | không | `OP_SYNC` (fsync + flush) |
| 16 | `truncate` | có | **active** | không | không | `OP_TRUNC` |
| 17 | `fsck` | **không** | **active** | không | không | `OP_FSCK` → `Fs::fsck` |
| 18 | `free` | không | **deferred-K7** | không | không | **để `KT-345b`** — cần `MemSvc`/`meminfo.vidl` |
| 19 | `df` | không | **deferred-K7** | không | không | **để `KT-345b`** — cần `OP_STATFS`/`fsstat.vidl` |
| 20 | `uptime` | có (khác) | **active** (khác) | không | không | v1 tick; v2 `ms=` |
| 21 | `uname` | **không** | **active** | không | không | `ARCH_NAME` từ shim → `ViFastOs <arch> bootinfo-v3` |
| 22 | `run` | có (khác) | **active** (khác) | không | không | exec kernel-mediated, bảng ảnh nhúng `/hello.elf` (không đọc FS) |
| 23 | `runfs` | **không** | **active** (khác) | không | không | bí danh `run`; **args bị bỏ qua** |
| 24 | `exec` | **không** | **active** (khác) | không | không | bí danh `runfs`; **args bị bỏ qua** |
| 25 | `jobs` | có (khác) | **active** (khác) | không | không | đếm 4 nhóm từ `PS_REQ` |
| 26 | `loop` | không | **deferred-K7** | không | không | **để `KT-345b`** — giao thức loop ở root chưa có |
| 27 | `history` | **không** | **active** (khác) | không | không | giữ **một** dòng (`last=`), v1 giữ danh sách |
| 28 | `!!` | **không** | **active** | không | không | chạy lại `last`; không đệ quy vô hạn |

**Đếm:** `spec/v1-parity.toml` **14 active → 25 active / 28** (3 `deferred-K7` còn lại: `free`, `df`,
`loop`); aarch64 **4/28** (`help`/`echo`/`ps`/`kill` — shell `ktest`, chưa có FS/`KT-339a`);
x86_64 **4/28** (shell sản phẩm `product-shell` của `KT-341`, `help cmds=6`; chưa có FS).

### 1.1 Đúng **3** cái để `KT-345b` (tên + lý do)

1. **`free`** — cần dịch vụ `MemSvc` + `spec/protocols/meminfo.vidl` (`total`/`free`); ABI v1.1
   **không** thêm syscall, `KT-345b` dựng dịch vụ cùng khuôn `netsrv` (D2-242 nhóm 4C).
2. **`df`** — cần `OP_STATFS` + `spec/protocols/fsstat.vidl` (`blocks`/`blocks_free`/`block_size`);
   `proc_fs` hiện chỉ có `OP_STAT` cho một đường dẫn.
3. **`loop`** — cần giao thức `loop` ở root (spawn/destroy N lần) + vòng đời TCB; đúng phạm vi
   `KT-345b`/`KT-347`.

## 2. Thay đổi (tệp + dòng)

| Tệp | +/− | Nội dung |
|---|---:|---|
| `spec/v1-parity.toml` | +71/−40 | **D2-250**: 11 mục `deferred-K7` → `active` + `send`/`expect` thật (regex v2) + `note`; `ps` gửi 2 lần để `serial_min_count=2` đúng trên ảnh sản phẩm; `free`/`df`/`loop` giữ nguyên `deferred-K7` |
| `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs` | +20 | `OP_MKDIR` 0x17, `OP_RM` 0x18, `OP_MV` 0x19, `OP_LN` 0x1a, `OP_FSCK` 0x1b, `OP_PATH2` 0x1c |
| `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs` | +104/−1 | `path2` + `load_path2`/`take_path2`; 5 handler (`Fs::mkdir/unlink/rename/link/fsck`); cùng bảng `err_code` cũ |
| `v2/kernel/vifast-plat-user/programs/shell_cmds.rs` | **+242 (mới)** | 8 lệnh `shell_fs`: `mkdir`/`rm`/`mv`/`ln`/`fsck`/`uname`/`history`/`!!` + `fs_set_path2`; tách để giữ **R1** |
| `v2/kernel/vifast-plat-user/programs/shell.rs` | +156/−36 | dispatch 8 lệnh + `runfs`/`exec`; lịch sử một dòng; `CMDS` 29; **1.411 dòng** (R1 1.500) |
| `v2/kernel/vifast-plat-user/lib.rs` | +20/−8 | `Sys::ARCH_NAME` cho **cả 3** backend (`riscv64`/`aarch64`/`x86_64`) — shell dùng chung nguồn |
| `v2/kernel/vifast-kbin-riscv64/build.rs` | +1 | `rerun-if-changed` cho `shell_cmds.rs` |
| `v2/lib/vifast-fs/tests/kt343_commands_host.rs` | **+134 (mới)** | 2 host test: hợp đồng 5 thao tác + `rename` thay thế / `link` từ chối đích |
| `v2/test/boot/product-riscv64.sh` | +84/−8 | 20 lệnh + 2 boot `runfs`/`exec` + **boot ca âm từng nhóm** (meta/exec) |
| `v2/test/parity/run.sh` | +48/−6 | `--extra`, `--extra-only`, `--timeout-ds` (tuỳ chọn; lần đo cuối dùng spec trực tiếp) |

Không chạm: `spec/abi.toml`, `spec/kernel.toml`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`v2/bin/**`, `.github/**`, 19 tệp `[[legacy]]`, `main`.

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

### 3.2 Parity `--mode serial` trên **ảnh sản phẩm** — đọc thẳng `spec/v1-parity.toml`

```text
$ bash v2/test/parity/run.sh --mode serial --image product --smp 1 --timeout-ds 60
# parity: spec_total=28 spec_active=25 deferred=3 extra=0 total=28
ok 1 - help ... ok 25 - !!
# parity: total=28 ok=25 deferred=3 active=25 (active_da_chay=25)
# parity: KET_LUAN=ok (exit 0)
```

<!-- SM4 -->

### 3.3 Ảnh sản phẩm riscv64 — 43 phép kiểm + ca âm từng nhóm

`product-riscv64.sh --smp 1`: **43 ok / 0 not ok** (5 FS + 3 meta + `run`/`runfs`/`exec` + net/FS/halt
giữ nguyên KT-342) + **7 ca âm nhóm** (`check_negative_groups`: `history last=` rỗng, `!! none`,
`run|runfs|exec /khong.elf` ⇒ `err=<n>`, shell còn sống, halt sạch, không PANIC).
`.data+.bss` **224.472 B** / 262.144; `# product fs:` `medium=virtio-blk`; 0 harness marker.

<!-- PRODUCT -->

## 4. Hai đột biến — cả hai ĐỎ

### (i) Sai mã lỗi syscall/FS (`NotFound` 5 → `Invalid` 2) ⇒ ca parity ĐỎ

Sửa `vifast-fs/src/dir.rs::unlink` `.ok_or(FsError::NotFound)` → `.ok_or(FsError::Invalid)`, chạy
`product-riscv64.sh --smp 1`:

```text
not ok 1 - fs: rm mục không có ⇒ NotFound 5 (thiếu /^# shell: rm path=/parity loi code=5$/)
# product-riscv64: checks=0 fails=1 ...
```

Khôi phục ⇒ host test **2/2 PASS** + product **43 ok / 0 not ok**.

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

## 5. Cổng (chạy trên cây sạch — commit ghi ở §6)

<!-- GATES -->

## 6. Commit & cây sạch

<!-- COMMITS -->

## 7. Nợ / khác biệt so với kế hoạch (nói thẳng)

1. **`runfs`/`exec` bỏ qua args**: v1 truyền `[args...]` cho chương trình đích; v2 `Exec` của kernel
   chỉ nhận đường dẫn + bảng ảnh nhúng (`/hello.elf`), nên `hello` không tới ảnh đích. Đã ghi
   `note` trong spec; mở ở Chu kỳ 3 (`U1`/`VSH2`) khi có exec-from-FS.
2. **`history`/`!!` chỉ một dòng**: v1 giữ danh sách; v2 giữ `last` (không `alloc`) — đủ parity
   hành vi `history`/`!!` nhưng chưa có `history <n>`.
3. **`run` không đọc ELF từ FS**: KT-342 dùng bảng ảnh nhúng; `runfs` là bí danh nên **chưa** phải
   "exec từ FS" đúng nghĩa v1. `KT-345b`/Chu kỳ 3 mới có.
4. **3 arch chưa cùng 25/28**: `aarch64` shell `ktest` chỉ có 4/28 lệnh (chưa FS; `KT-339a`),
   `x86_64` `product-shell` 4/28 (chưa FS). Phần dùng chung đã là **một nguồn** (`shell.rs` +
   `shell_cmds.rs` + `vifast-fs`); parity đầy đủ 3 arch cần FS service cho 2 arch (Chu kỳ 3).
5. **`ping` vẫn qua kernel** (nợ KT-342 §8.2) — không thay đổi trong task này.
6. **`free`/`df`/`loop`** còn `deferred-K7` — đúng `KT-345b`; `KT-343` không tự nhận 28/28.
