# KT-153 — Review độc lập đêm 2026-11-18/19 (L6)

**Người review**: sub-agent L6 (reviewer độc lập) · **Worktree**: `/private/tmp/night-L6-review`
· **Nhánh**: `night/L6-review` · **HEAD**: `a9460f0c` (tách từ `main` tại `aef3e372`)
· **Bắt đầu**: 2026-11-18 23:05 JST · **Hạn**: 2026-11-19 07:30 JST
· **Vai**: phá bằng chứng của các task vừa merge, không làm thêm tính năng.

> **Luật 3**: trong tệp này, chữ "đã chạy" / "đã đo" chỉ dùng khi ngay sau đó có (a) lệnh,
> (b) dòng output nguyên văn, (c) loại cây. Không có "đã chứng minh" suông.

## 0. Tóm tắt

| Task | Kết luận | Lý do một dòng |
|---|---|---|
| **KT-148** (B7) | **ĐẠT** | Test `smp` sửa đúng 4 số hạng; bước `L0 kernel workspace` có trong `gate --fast`; đột biến mã (không phải đột biến test) bị const-assert chặn ở compile-time. |
| **KT-149** (N2) | **ĐẠT CÓ ĐIỀU KIỆN** | Tại HEAD `a9460f0c` (đầu review) **chưa merge**; `main` đã tiến lên `4e6dcea1` (merge KT-149) trong lúc review — cây merge có `t26.sh` + bước `selftest t26` và `v2 check` PASS. Vế 1 đo trên mã trước vá, không phải tag `v1-archive`. |
| **KT-150** (N3) | **ĐẠT CÓ ĐIỀU KIỆN** | TAP 37 mức kernel là thật và bắt được giao lặp; nhưng checker host là tự-đối-chiếu sổ của chính harness, ca 4 client chỉ mức host, vế 1 đo trên mã trước r309 (không phải ảnh `v1-archive`). |
| **KT-151** (N7) | **ĐẠT CÓ ĐIỀU KIỆN** | Kênh chéo hart đã thống nhất + test chạy thật; nhưng cổng **không** bắt được đột biến "thêm đường wake thứ hai đi vòng qua mailbox" (D2-131 (c)) — chỉ `--strict` (không trong cổng) đỏ. |
| **KT-152** (N1) | **ĐẠT CÓ ĐIỀU KIỆN** | Kịch bản sim + 2 đột biến đỏ là thật; nhưng vế 1 chỉ là **trích dẫn** `[V1:ĐO]` ("chưa đo lại"), và lưới chắn chỉ ở mức mô hình, không nối `rdtime`. |

**Phát hiện nặng nhất**:
1. **KT-149 chưa nằm trên `main` tại HEAD `a9460f0c`** — toàn bộ harness T26 + bước cổng nằm ở nhánh chưa merge; **đã được lead merge lên `main` tại `4e6dcea1` trong lúc review** (xem §2(A) — race).
2. **KT-151: cổng xanh trước một đường wake thứ hai** — đúng lớp lỗi "test xanh nhưng giả" mà
   repo đã gặp hơn mười lần.

---

## 1. KT-148 (B7) — sửa test `smp` + L0 workspace kernel vào `gate --fast`

### (A) Đọc diff

Tệp: `v2/kernel/vifast-plat-common/tests/smp.rs`, `v2/kernel/vifast-plat-common/src/smp.rs`,
`v2/kernel/vifast-plat-common/src/lib.rs`, `v2/kernel/vifast-plat-riscv64/src/mmu.rs`,
`v2/bin/v2`.

- Công thức test đổi từ **3** số hạng (`GUARD + STACK + TRAP`) sang **4** số hạng
  (`+ SYSCALL_STACK`) — đúng `HART_STRIDE` đã có từ KT-66. Đây là **sửa test sai**, không phải
  nới ngưỡng.
- `assert!(trap_top_offset(hart) <= guard_offset(hart + 1))` → `assert_eq!(syscall_top_offset(hart),
  guard_offset(hart + 1))`: **siết** từ `<=` sang `==` (đúng D2-119).
- `assert_eq!(trap_top_offset(MAX_HARTS - 1), HART_AREA_BYTES)` → `syscall_top_offset(...)`:
  đúng đỉnh record mới.
- Thêm 2 assert cho `syscall_bottom_offset`/`syscall_top_offset`.
- Không có `#[ignore]`, không bỏ ca, không nhánh `if` làm test luôn xanh.
- **Điểm yếu (A)**: assert đầu tiên `GUARD + STACK + TRAP + SYSCALL == HART_STRIDE` là **hằng
  đẳng thức tự chứng minh** vì `HART_STRIDE` được định nghĩa đúng bằng tổng đó
  (`v2/kernel/vifast-plat-common/src/smp.rs:49-50`). Nó không bắt được một `HART_STRIDE` sai.
  Neo thật là khối `const _: () = { assert!(...) }` ở `smp.rs:374-398` (kiểm cả giá trị tuyệt
  đối `0x1D000`, `0xE8000` và các offset).

### (B) Đột biến độc lập

**Đột biến L6-148**: bỏ số hạng `HART_SYSCALL_STACK_BYTES` khỏi **định nghĩa mã**
`HART_STRIDE` (không phải khỏi test — khác đột biến của tác giả, vốn sửa chính test).

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test smp
   Compiling vifast-core v0.1.0 (/private/tmp/night-L6-review/v2/kernel/vifast-core)
   Compiling vifast-plat-common v0.1.0 (/private/tmp/night-L6-review/v2/kernel/vifast-plat-common)
error[E0080]: evaluation panicked: assertion failed: HART_STRIDE == 0x1D000
   --> vifast-plat-common/src/smp.rs:380:5
    |
380 |     assert!(HART_STRIDE == 0x1D000);
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ evaluation of `smp::_` failed here

For more information about this error, try `rustc --explain E0080`.
error: could not compile `vifast-plat-common` (lib) due to 1 previous error
EXIT=101
```

Cây: `/private/tmp/night-L6-review`, nhánh `night/L6-review`, HEAD `a9460f0c`; log đầy đủ
`v2/agent/JOURNAL/kt153-logs/kt153-kt148-mutation.log`.

**Đột biến L6-148b**: `syscall_top_offset` lệch 1 byte (đột biến hàm, không đụng hằng số):

```text
error[E0080]: evaluation panicked: assertion failed: syscall_top_offset(0) == 0x1D000
   --> vifast-plat-common/src/smp.rs:388:5
```

⇒ Cả hai đột biến **đỏ ở compile-time** nhờ const-assert. Đây là bằng chứng mạnh hơn đột biến
của tác giả (tác giả chỉ sửa test rồi thấy test đỏ — tự quy chiếu).

Khôi phục: `cp /tmp/l6-smp-orig3.rs ...`; `git status --short` rỗng, `git diff` rỗng (xem §6).

### (C) Kiểm số

Đối chiếu số trong `v2/agent/JOURNAL/kt148-b7.md` với log thật:

| Số trong journal | Nguồn | Kết quả |
|---|---|---|
| `9 passed; 0 failed` (`--test smp`) | §4 | **khớp** — chạy lại: `test result: ok. 9 passed; 0 failed; 0 ignored; ...` |
| `13 dòng test result` | §4 | **khớp** — `grep -c "^test result"` = `13` |
| `md5 6b1bd6a2abf8f4bc1d76236c1def04e5` | §6 | **khớp** — `md5 -q v2/kernel/vifast-plat-common/tests/smp.rs` = `6b1bd6a2abf8f4bc1d76236c1def04e5` |
| `left: 86016 / right: 118784` | §1, §6 | **khớp số học** — `86016 = 0x15000`, `118784 = 0x1D000` |
| `cold 7657 ms / run1 1280 ms / run2 539 ms` | §7 | **không có log riêng** — journal chỉ chép tay, không có tệp log trong `kt148-logs/` (thư mục này không tồn tại). Ghi "chưa kiểm". |
| `0 ignored` (dòng cuối) | §4 | **KHÔNG khớp** — chạy lại package: dòng cuối là `test result: ok. 0 passed; 0 failed; 1 ignored; ...` (doc-test `PanicHandler` bị `ignore` có chủ đích). Đây là lỗi chép số nhẹ. |

Lệnh kiểm: `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common` (log
`v2/agent/JOURNAL/kt153-logs/kt153-kt148-package.log`).

### (D) Vế 1

"Vế 1" của KT-148 là **test đỏ trước khi sửa** (không phải mã cũ có lỗi): tại `aef3e372`, test
dùng công thức 3 số hạng nên đỏ `left: 86016 right: 118784`. Đây là **đỏ thật trên cây cũ**
(test sai, không phải fixture selftest). Tuy nhiên:
- Đột biến của tác giả (khôi phục công thức cũ **trong test**) chỉ chứng minh "test đỏ khi test
  sai", không chứng minh "test bắt được mã sai". Đột biến L6-148 ở (B) mới là phép thử đúng.
- Không có "ảnh cũ" nào ở đây — B7 là lỗi test, không phải lỗi chạy.

### Kết luận KT-148: **ĐẠT**

Điều kiện ghi kèm: đột biến của tác giả là đột biến test (yếu); nên thay bằng đột biến mã
(const-assert đã bắt). Không có phát hiện nặng.

---

## 2. KT-149 (N2) — T26 read-your-own-write

### (A) Đọc diff — **chưa có trên `main` tại HEAD `a9460f0c`; đã merge tại `4e6dcea1` trong lúc review**

**Race**: lúc bắt đầu review (HEAD `a9460f0c`), KT-149 **chưa** nằm trên `main`. Trong lúc tôi
đang review, lead đã merge nhánh `night/L1-n2` lên `main` tại `4e6dcea1` (commit date
2026-11-18 23:02:33). Vì vậy phát hiện "KT-149 chưa merge" đúng tại thời điểm HEAD `a9460f0c`,
nhưng **đã được giải quyết** bởi merge `4e6dcea1`. Tôi kiểm lại trên cây merge (xem cuối mục).

Bằng chứng tại HEAD `a9460f0c` (đầu review):

```text
$ git log --oneline aef3e372..HEAD | grep -i "KT-149" || echo "KHONG CO COMMIT KT-149 TRONG main"
KHONG CO COMMIT KT-149 TRONG main

$ git branch -a --contains 780aa750
+ night/L1-n2

$ git merge-base --is-ancestor night/L1-n2 HEAD && echo ANCESTOR || echo NOT-ANCESTOR
NOT-ANCESTOR

$ ls v2/test/boot/t26.sh
ls: v2/test/boot/t26.sh: No such file or directory

$ grep -n "t26" v2/bin/v2
539:# **KT-26** (K1C-PLAN §5 KT-26 DoD 8): thêm `kt26_fault` ...
541:# 3 seed × 8 op bằng `cfg(miri)`). KHÔNG thêm `kt26_scn` (đọc tệp — test I/O).
610:    run_miri_test "vifast-sim kt26_fault" -p vifast-sim --test kt26_fault || return 1
```

⇒ Trên `main`/HEAD `a9460f0c`: **không** có `v2/test/boot/t26.sh`, **không** có bước
`t26.sh --selftest` trong `v2/bin/v2`. Hai commit KT-149 (`780aa750`, `e6d70a2b`) chỉ nằm trên
nhánh `night/L1-n2`.

**Kiểm lại trên cây merge `4e6dcea1`** (sau khi lead merge):

```text
$ git show --stat 4e6dcea1 | head -3
commit 4e6dcea196e9b574040cbc192ee56407deb42b2b
Merge: a9460f0c e6d70a2b
...
 v2/test/boot/t26.sh | 408 ++++++++++++++++++++++++

$ git ls-tree -r --name-only 4e6dcea1 | grep "v2/test/boot/t26.sh"
v2/test/boot/t26.sh

$ git show 4e6dcea1:v2/bin/v2 | grep -n "selftest t26"
262:    run_step "selftest t26 (KT-149: g1..g4)" bash "$V2_ROOT/test/boot/t26.sh" --selftest || fails=$((fails + 1))

$ ./v2/bin/v2 check   # trên cây merge 4e6dcea1 (detached)
...
--- selftest t26 (KT-149: g1..g4): OK
...
v2 check: PASS (L1 chạy trong gate --fast)
CHECK_EXIT=0
```

⇒ Trên cây merge `4e6dcea1`: `t26.sh` **có**, bước `selftest t26` **có** trong `v2 check`, và
`v2 check` **PASS**. Log đầy đủ: `v2/agent/JOURNAL/kt153-logs/kt153-check-merged.log`.

### (B) Đột biến độc lập (trên mã có trên `main`)

Test host `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` **có** trên `main` (từ KT-T26,
commit `02e0a963`), nên vẫn kiểm được phần "test host 1000 seed".

**Đột biến L6-149**: `CachedDev::flush()` bỏ qua các block **dữ liệu** (`lba >= DATA_START`),
tức flush metadata nhưng không flush data — khác đột biến của tác giả (bỏ hẳn lời gọi
`self.dev.flush()?` thứ hai trong `commit()`).

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-fs --test t26_read_your_own_write
running 3 tests
test harness_catches_silent_write_loss ... ok
test ryow_1000_seeds ... FAILED
test ryow_across_remount_1000_seeds ... FAILED

---- ryow_1000_seeds stdout ----
thread 'ryow_1000_seeds' (77546924) panicked at lib/vifast-fs/tests/t26_read_your_own_write.rs:182:13:
seed 1: read-your-own-write sai sau write[0] (seed 1) — kẻ chen giữa vừa tác động

---- ryow_across_remount_1000_seeds stdout ----
thread 'ryow_across_remount_1000_seeds' (77546925) panicked at lib/vifast-fs/tests/t26_read_your_own_write.rs:211:9:
assertion `left == right` failed: seed 0: RAW read khác write

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
EXIT=101
```

Cây: `/private/tmp/night-L6-review`, HEAD `a9460f0c`; log đầy đủ
`v2/agent/JOURNAL/kt153-logs/kt153-kt149-mutation.log`.

⇒ Test host **bắt đúng kênh T26** (read-your-own-write), không chỉ "luôn xanh". Khôi phục:
`git status --short` rỗng.

### (C) Kiểm số

Đối chiếu `v2/agent/JOURNAL/kt149-n2.md` (trên nhánh `night/L1-n2`) với log thật:

| Số trong journal | Log thật | Kết quả |
|---|---|---|
| `SUMMARY red=2 green=48 total=50` (tag `v1-archive`) | `v1-50boots-tag.log:59` | **khớp** |
| `SUMMARY red=7 green=43 total=50` (mã trước vá) | `v1-50boots-prefix.log:86` | **khớp** |
| `5/50 boot T-FS` | `v1-50boots-prefix.log` — boot 12, 14, 35, 40, 48 | **khớp** (5 dòng `T-FS=1`) |
| `boots=50 green=50 red=0` (v2) | `v2-50boots.log:59` | **khớp** |
| `3 passed; ... 2.62s` (host) | `kt149-host-green.log:9` | **khớp** |
| `1 passed; 2 failed` (đột biến) | `kt149-host-mutation-red.log:30` | **khớp** |
| `host-green.log` / `host-mutation-red.log` (đường dẫn) | thực tế là `kt149-host-green.log` / `kt149-host-mutation-red.log` | **lệch tên tệp** (nhẹ) |

### (D) Vế 1

- **Trên tag `v1-archive` (`ca2eaf54`)**: **0/50 T-FS** — tag đã chứa bản vá r296b
  (`9cd233f3`), nên **không tái hiện** T26. Journal ghi đúng điều này (§1.3).
- **Trên mã trước vá `5fc7b3ff`**: **5/50 T-FS** — serial thật
  `v1-prefix-red-tfs-serial.txt` có đúng chữ ký T26:
  `V2MK open=1` / `V2RD end n=06` / `V2FAIL v2=1 fsck=0` / `V2GROUP 06` / `FS V2 BAD`.
  Đây là **đỏ thật trên mã cũ**, không phải fixture selftest.
- **Kết luận (D)**: vế 1 **không** đỏ trên ảnh `v1-archive`; đỏ trên commit **trước vá**. Journal
  đã ghi đúng và **không** tuyên bố đóng N3. Đây là cách ghi trung thực.

### Kết luận KT-149: **ĐẠT CÓ ĐIỀU KIỆN**

Lý do: tại HEAD `a9460f0c` (đầu review) KT-149 **chưa merge** — đó là phát hiện nặng ban đầu.
Lead đã merge lên `main` tại `4e6dcea1` trong lúc review; cây merge có `t26.sh` + bước
`selftest t26` và `v2 check` **PASS**. Điều kiện còn lại: vế 1 đo trên mã **trước vá**
(`5fc7b3ff`), không phải tag `v1-archive` (tag đã vá r296b ⇒ 0/50 T-FS); journal ghi đúng nhãn
này. Nếu lead muốn "vế 1 theo đúng chữ tag" thì **chưa đạt**.

---

## 3. KT-150 (N3) — pager giao đúng một lần

### (A) Đọc diff

Tệp: `v2/kernel/vifast-kbin-riscv64/src/ktest_vfs.rs`, `.../ktest_vfs_once.rs` (mới),
`.../ktest_user.rs`, `v2/lib/vifast-vfs/tests/pager.rs`, `v2/test/boot/expected.txt`,
`v2/test/boot/ktest.sh`.

- TAP 37 thêm vào `pager_phase`; `expected.txt` `ktest-user 36 36` → `37 37`; manifest
  `ktest.sh` thêm `pager giao dung mot lan`; evidence regex thêm `# pager: once ...`.
- `ktest_user.rs`: `taps=35` → `37`, PANIC `(35 TAP, 0 fail)` → `(37 TAP, 0 fail)`. Đây là sửa
  di chứng KT-122 (expected.txt đã 36 nhưng hai dòng này còn 35) — **đúng**, không nới.
- `ktest_vfs_once.rs`: `DeliveryLedger` + `check_once` fail-closed. `check_once` đòi
  `deliveries == planned`, `faults == planned`, `served + rejected == faults`, và **theo từng
  VA** `delivered_to(va) == want`.
- `pager.rs`: thêm `PagerSession` + `Ledger` + `check_delivered_once` + 3 test
  (`bon_client...`, `ca_am_phat_lai...`, `checker_fail_closed...`).
- **Điểm yếu (A)**: `check_delivered_once` (host) là **tự đối chiếu sổ của chính harness**:
  `fault()` luôn `intended.push` rồi `send()`; `send()` luôn `delivered.push` rồi `served.push`.
  Vì vậy hai điều kiện `delivered.len() != intended.len()` và `served.len() != delivered.len()`
  **không thể** xảy ra trừ khi bật cờ `replay_on_resume` của chính harness. Nó không kiểm được
  `PagerCore` (mã sản phẩm) — chỉ kiểm sổ của mô hình.
- **Điểm yếu (A)**: ca 4 client chỉ ở **mức host** (`PagerSession`), không phải kernel. Kernel
  vẫn `CLIENTS = 2` (`v2/kernel/vifast-kbin-riscv64/init/pager_common.rs:70`). Journal ghi nợ
  này ở §5 — trung thực, nhưng D2-130 (b) nói "ca `clients=4`" không giới hạn mức host.

### (B) Đột biến độc lập

**Đột biến L6-150**: gửi **hai lần** đúng message `pager.fault` trong `deliver_and_pump`
(đường giao thật), khác đột biến của tác giả (phát lại **một** fault cụ thể sau `served`).

```text
$ bash /tmp/l6-boot-user.sh /tmp/l6-kt150-red-double-send-smp1.log 1
# infra l6: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.50 loadavg_5m=4.35 loadavg_15m=4.71 ncpu=12 boot_done=1 verdict=OK
BOOT_RC=0

$ bash v2/test/boot/ktest.sh --check-log ktest-user /tmp/l6-kt150-red-double-send-smp1.log 1
not ok - check-log ktest-user smp=1: thiếu ok-count=31/37 not-ok=6 tap-day-1..37 panic-tong=0/1 tap31 tap32 tap33 tap34 tap35 tap37 ...

--- các dòng quyết định trong log boot ---
225:ok 30 - pager fault den server va nap trang
226:not ok 31 - pager hit khong doc backend: hit khong do duoc hoac PA khac nhau
227:not ok 32 - pager eviction LRU khong mat du lieu: eviction/reload du lieu sai
228:not ok 33 - pager hai client chia se mot trang: khong chung minh duoc mot ban nhieu mapping
229:not ok 34 - pager page ngoai file tra ma: ma loi sai hoac doc backend sai
230:not ok 35 - pager ca am quyen va khong ghi de: ca am khong do duoc hoac PTE/du lieu bi ghi de
232:ok 36 - pager bat bien reads==misses
233:# pager: once deliveries=4 faults=8 served=4 rejected=4 expected_va=7 distinct_va=4 ok=0
234:not ok 37 - pager giao dung mot lan: message bi giao/xu ly lap, hoac so dem theo dia chi khong doc duoc
```

Cây: `/private/tmp/night-L6-review`, HEAD `a9460f0c`; log đầy đủ
`v2/agent/JOURNAL/kt153-logs/kt153-kt150-mutation.log`.

**Đọc số**: `deliveries=4` nhưng `faults=8` ⇒ checker **bắt đúng** giao lặp. Tuy nhiên đỏ **lan
sang TAP 31–35** (pager bị hai fault cho cùng trang ⇒ lệch trạng thái), không cô lập được về
TAP 37. Đột biến của tác giả cô lập hơn (TAP 30–36 xanh, chỉ 37 đỏ). Đây là hạn chế của đột
biến L6, không phải lỗi của checker.

Khôi phục: build lại ảnh cho **cùng sha** `25c54558da88657b1ad06a375d3b8a2968687d5365bda59a8c0827cde9fba4b4`
như trước đột biến; `git status --short` rỗng.

### (C) Kiểm số

Đối chiếu `v2/agent/JOURNAL/kt150-n3.md` với log thật:

| Số trong journal | Log thật | Kết quả |
|---|---|---|
| `deliveries=10 faults=10 served=9 rejected=1 expected_va=7 distinct_va=7 ok=1` | `kt150-green-smp1.log:245`, `kt150-green-smp2.log:245` | **khớp** |
| `plan=37 ok=37, 64 evidence` | chạy lại `ktest.sh --check-log` trên log xanh: `ok - check-log ktest-user smp=1 (plan=37 ok=37, 64 evidence)` | **khớp** |
| `deliveries=10 faults=12 served=9 rejected=3 ok=0` (đột biến) | `kt150-red-replay-once-smp1.log:245` | **khớp** |
| `giao 24 message cho 12 fault` (host) | `kt150-host-red-replay.log:12` | **khớp** |
| `15 passed` (host) | `kt150-host-green.log:21` | **khớp** |
| `left: 2 right: 1` (v1 pre-r309) | `kt150-v1-pre-r309-red.log:8-9` | **khớp** |
| `evict_ok=1 dup=0` (v1-archive) | `kt150-v1-boot-smp1.log` (journal §3.1) | **khớp** (đã đọc log) |

### (D) Vế 1

- **Trên ảnh `v1-archive`**: `PAGER EVICT OK` = **1**, `PAGER2 DUP` = **0** ⇒ **không tái hiện**
  triệu chứng. Journal ghi đúng (§3.1) và **không** tuyên bố đóng N3.
- **Trên mã v1 trước r309 (`6f6ab5b1^`)**: test host mới `kt150_pager2_dup` **đỏ**
  (`left: 2 right: 1`). Đây là **đỏ thật trên mã cũ**, nhưng là **test do tác giả viết thêm**
  vào mã cũ, không phải đèn có sẵn của v1. Journal ghi đúng nhãn "đỏ trên mã v1 TRƯỚC r309".
- **Kết luận (D)**: vế 1 **không** đỏ trên ảnh `v1-archive`; đỏ trên mã trước r309 qua test
  thêm. Trung thực, nhưng không phải "đỏ trên ảnh cũ".

### Kết luận KT-150: **ĐẠT CÓ ĐIỀU KIỆN**

Điều kiện: (i) checker host phải kiểm **mã sản phẩm** (`PagerCore`/kernel), không chỉ sổ của
harness; (ii) ca 4 client mức kernel còn nợ; (iii) vế 1 chỉ là "đỏ trên mã trước r309", không
phải ảnh `v1-archive`. TAP 37 mức kernel là bằng chứng thật và bắt được giao lặp.

---

## 4. KT-151 (N7) — một cơ chế wake

### (A) Đọc diff

Tệp: `v2/tools/check-wake-paths.py` (mới, 822 dòng), `v2/sim/vifast-sim/tests/kt151_wake.rs`
(mới, 355 dòng), `v2/sim/vifast-sim/src/kobj.rs`, `v2/sim/vifast-sim/src/lib.rs`, `v2/bin/v2`.

- Checker có **hai mức**: mặc định (trong `v2 check`) chỉ kiểm **kênh chéo hart**; `--strict`
  kiểm **mọi** chuyển `blocked/sleeping → Ready` nằm trong `wake_step` — nhưng `--strict`
  **không** nằm trong cổng và **đỏ** trên v2 (4 chỗ, nợ N7-2).
- Test `kt151_wake.rs` chạy trên **mô hình kernel** (`vifast-core`) + sim, không QEMU. 5 ca:
  wake chéo hart đúng một lần, wake cùng hart không qua hộp thư, Reschedule, Migrate.
- **Điểm yếu (A)**: mức mặc định **không** kiểm "mọi đường wake đi qua hộp thư" — nó chỉ kiểm
  producer/consumer của `MailboxMsg::Wake/Reschedule/Migrate` + không doorbell arch. Một đường
  wake thứ hai **trực tiếp** (không phát `MailboxMsg`) sẽ **lọt**. Đây đúng là điều D2-131 (c)
  mô tả ("thêm đường wake thứ hai đi vòng qua mailbox ⇒ test đỏ").
- **Điểm yếu (A)**: `RE_ARCH_FORBIDDEN` được định nghĩa (`check-wake-paths.py:94`) nhưng
  **không dùng** — dead code.
- **Điểm yếu (A)**: `vifast-sync::Mailbox` (kiểu thiết kế D2-131) **không** được kernel/sim
  dùng; chỉ test host `kt30_sync.rs` chạm. Journal ghi nợ N7-1 — trung thực.
- **Điểm yếu (A)**: `Machine::wake` chỉ được gọi từ test (`ktest_inv.rs:51`,
  `ktest_user.rs:692`, các test sim); **không** có đường sản phẩm nào gọi. Journal ghi nợ N7-2
  — trung thực.

### (B) Đột biến độc lập

**Đột biến L6-151**: thêm hàm `wake_direct_l6` vào `v2/kernel/vifast-core/src/core_sched.rs`
— đường wake **thứ hai** chuyển thẳng `blocked/sleeping → Ready`, **giữ nguyên** producer
`MailboxMsg::Wake` của `wake()`. Khác đột biến của tác giả (tác giả **bỏ** producer, nên
`E_CHANNEL_MISSING` bắt).

```text
$ python3.12 v2/tools/check-wake-paths.py
# kênh Wake: v2/kernel/vifast-core/src/core_sched.rs:224 (wake)
# kênh Wake: v2/sim/vifast-sim/src/kobj.rs:5718 (wake)
# kênh Migrate: v2/kernel/vifast-core/src/core_sched.rs:399 (push_step)
# kênh Migrate: v2/sim/vifast-sim/src/kobj.rs:6056 (push_step)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
DEFAULT_EXIT=0

$ python3.12 v2/tools/check-wake-paths.py --strict
# chuyển Ready: v2/kernel/vifast-core/src/core_sched.rs:236 (wake_direct_l6)
...
E E_STRICT_TRANSITION 5 chỗ chuyển `TcbState::Ready` ngoài `wake_step` (nợ N7-2, không nằm trong cổng mặc định)
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/core_sched.rs:236 (wake_direct_l6)
...
# check-wake-paths: mode=v2 strict=1 errors=6 warnings=0
STRICT_EXIT=1

$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt151_wake
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0
```

Cây: `/private/tmp/night-L6-review`, HEAD `a9460f0c`; log đầy đủ
`v2/agent/JOURNAL/kt153-logs/kt153-kt151-mutation.log`.

⇒ **Cổng mặc định XANH** trước một đường wake thứ hai; test `kt151_wake` cũng **XANH** (không
gọi hàm mới). Chỉ `--strict` — **không nằm trong cổng** — đỏ. Đây là **phát hiện nặng**: tiêu
chí (c) của D2-131 **không đạt** với đúng loại đột biến nó mô tả.

Khôi phục: `git status --short` rỗng.

### (C) Kiểm số

Đối chiếu `v2/agent/JOURNAL/kt151-n7.md` với log thật:

| Số trong journal | Log thật | Kết quả |
|---|---|---|
| `errors=0` (mặc định) | `kt151-v2-checker.log`, `kt151-check.log` | **khớp** |
| `errors=5` (`--strict`, 4 chỗ + 1 tổng) | `kt151-strict.log` | **khớp** |
| `5 passed` (test) | `kt151-test-green.log` | **khớp** |
| `left: 0 right: 1` (đột biến) | `kt151-mutation-test.log` | **khớp** |
| `errors=1` / `E_CHANNEL_MISSING kernel` (đột biến) | `kt151-mutation-checker.log` | **khớp** |
| `7/7 ok` (selftest) | `kt151-selftest.log` | **khớp** |
| `3 cơ chế`, `errors=6` (v1) | `kt151-v1-checker.log` | **khớp** — tôi chạy lại trên `/tmp/l6-v1` (giải nén `v1-archive`, commit `ca2eaf54`): `# check-wake-paths: mode=v1 strict=0 errors=6 warnings=0`, `V1_EXIT=1` |
| `kobj.rs:5707` / `6045` | log nhánh `night/L3-n7` | **khớp log nhánh**, nhưng **lệch HEAD**: trên HEAD là `5718` / `6056` (merge KT-152 thêm 11 dòng). Nhẹ. |

### (D) Vế 1

- Checker chạy trên cây v1 giải nén từ tag `v1-archive` (`ca2eaf54`, tree `6535d1ce...`) —
  **đỏ thật**: 3 cơ chế (`riscv64:MAINT_KIND+send_ipi`, `aarch64:RESCHED_PENDING+GIC SGI`,
  `x86_64:AP_RESCHED_PENDING+LAPIC`), `errors=6`, `V1_EXIT=1`. Đây là **đỏ trên mã cũ thật**,
  không phải fixture selftest.
- Đối chứng dương: cùng checker trên v2 **xanh** (`errors=0`) ⇒ không phải máy luôn-đỏ.
- **Kết luận (D)**: **ĐẠT** — vế 1 là thật.

### Kết luận KT-151: **ĐẠT CÓ ĐIỀU KIỆN**

Điều kiện: (i) cổng phải bắt được đường wake thứ hai **trực tiếp** — hiện chỉ `--strict` bắt,
mà `--strict` không nằm trong cổng; (ii) `vifast-sync::Mailbox` chưa được kernel/sim dùng;
(iii) `Machine::wake` chưa có đường sản phẩm. Đây là nợ N7-1/N7-2 mà journal đã ghi, nhưng
tiêu chí "một cơ chế wake chung cho **mọi** đường" **chưa đạt**.

---

## 5. KT-152 (N1) — đơn điệu sau migrate

### (A) Đọc diff

Tệp: `v2/sim/vifast-sim/src/clock.rs` (mới, 213 dòng), `v2/sim/vifast-sim/src/kobj.rs`,
`v2/sim/vifast-sim/src/lib.rs`, `v2/sim/vifast-sim/tests/kt152_n1.rs` (mới, 333 dòng).

- `MonotonicClock`: `system` (đồng hồ hệ thống), `per_hart` (nguồn theo hart), `last_read`,
  `backwards`, `reads`. `tick_on` max-update; `read` trả `system` cho mọi hart; `reset_hart`
  chỉ đưa `per_hart[hart]` về 0, **không** đụng `system`.
- `Machine`: `clock_tick_on`, `clock_read`, `hart_reinit`, `clock_system`, `clock_per_hart`,
  `clock_backwards`, `clock_reads`, `check_clock`.
- Test: 4 ca — (a) đơn điệu sau migrate, (a') reset hart muộn, (b) conformance chéo hart, ca âm
  API.
- **Điểm yếu (A)**: đây là **mô hình** (`clock.rs`), **không** nối vào `rdtime` của kernel thật.
  Journal ghi rõ giới hạn này (§6) — trung thực.
- **Điểm yếu (A)**: `check_clock` chỉ kiểm `system >= per_hart[h]`; một đột biến làm `read` trả
  sai (M1 của tác giả) **không** bị `check_clock` bắt, chỉ bị assert của test bắt. Đó là lý do
  M1 cần thiết.

### (B) Đột biến độc lập

**Đột biến L6-152**: `tick_on` dùng **cộng dồn** (`system = system.saturating_add(n)`) thay vì
**max-update** — sai công thức nhưng **vẫn qua** `check_clock` (`system >= per_hart`). Khác
đột biến M1 (read trả `per_hart`) và M2 (reset_hart đưa `system` về 0) của tác giả.

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt152_n1
running 4 tests
test ca_am_api_dong_ho ... ok
test reset_hart_muon_khong_dua_dong_ho_ve_0 ... ok
test don_dieu_sau_migrate_hai_hart ... FAILED
test conformance_cheo_hart_mot_thu_tu ... ok

---- don_dieu_sau_migrate_hai_hart stdout ----
thread 'don_dieu_sau_migrate_hai_hart' (77558765) panicked at sim/vifast-sim/tests/kt152_n1.rs:183:5:
assertion `left == right` failed: đồng hồ hệ thống giữ max-update, không theo hart đích
  left: 6
 right: 5

test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=101
```

Cây: `/private/tmp/night-L6-review`, HEAD `a9460f0c`; log đầy đủ
`v2/agent/JOURNAL/kt153-logs/kt153-kt152-mutation.log`.

⇒ Test **đỏ đúng chỗ** (`sau == 5` bị phá thành `6`). Lưu ý: `conformance_cheo_hart_mot_thu_tu`
**vẫn xanh** dưới đột biến này — nó chỉ kiểm đơn điệu + bằng nhau, không kiểm công thức
max-update. Ca `don_dieu_sau_migrate_hai_hart` mới là ca bắt công thức.

Khôi phục: `git status --short` rỗng.

### (C) Kiểm số

Đối chiếu `v2/agent/JOURNAL/kt152-n1.md` với log thật (journal không có tệp log riêng — các
output nằm inline; tôi chạy lại):

| Số trong journal | Chạy lại | Kết quả |
|---|---|---|
| `4 passed` | `cargo test ... --test kt152_n1` | **khớp** |
| M1: `trước=5 sau=1 (hart 1 nguồn=1)` | tái hiện M1 (`read` trả `per_hart[hart]`) | **khớp** |
| M1: `left: 1 right: 0` | tái hiện M1 | **khớp** |
| M1: `left: 0 right: 7` | tái hiện M1 | **khớp** |
| M1: `1 passed; 3 failed` | tái hiện M1 | **khớp** |
| M2: `check_clock sau hart_reinit: hart 0: nguồn 7 > đồng hồ hệ thống 0 (max-update sai)` | tái hiện M2 | **khớp** |
| M2: `3 passed; 1 failed` | tái hiện M2 | **khớp** |
| `50` dòng `test result: ok` (suite sim) | `cargo test -p vifast-sim` trên HEAD | **KHÔNG khớp**: `51` (merge KT-151 thêm `kt151_wake`). Journal đo trên nhánh `night/L4-b7-n1` trước merge — đúng cho nhánh, lệch HEAD. Nhẹ. |

### (D) Vế 1

- Vế 1 của KT-152 là **trích dẫn** `[V1:ĐO]` từ `docs/v1/KNOWN-ISSUES.md` + đường mã v1
  (`TIME MONOTONIC BAD` → `panic!` → root chờ), nhãn **"CHƯA ĐO LẠI"**. **Không** có lần chạy
  600 boot x86_64 nào trong task.
- Đây **không** phải "đỏ trên mã cũ" đo được; là **bằng chứng lịch sử** (dossier v1). D2-132
  cho phép đúng dạng này, và journal ghi nhãn trung thực.
- **Kết luận (D)**: **chưa kiểm** theo nghĩa đo lại; **đạt** theo nghĩa "trích dẫn nguyên văn +
  file:dòng + nhãn chưa đo lại" mà D2-132 yêu cầu.

### Kết luận KT-152: **ĐẠT CÓ ĐIỀU KIỆN**

Điều kiện: vế 1 chỉ là trích dẫn (chưa đo lại); lưới chắn chỉ ở mức mô hình, không chứng minh
`rdtime` trên phần cứng. Các vế (a)(b)(c) có test + đột biến thật.

---

## 6. Kiểm mức repo

### 6.1 Cổng có thật chạy không

`./v2/bin/v2 check` trên HEAD `a9460f0c`:

```text
$ ./v2/bin/v2 check
...
--- wake paths (KT-151: một cơ chế wake): OK
--- selftest wake paths (KT-151): OK
...
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
CHECK_EXIT=0
```

Log đầy đủ: `v2/agent/JOURNAL/kt153-logs/kt153-check.log`.

Đối chiếu danh sách bước với tuyên bố trong journal:

| Bước mới | Trong `v2/bin/v2`? | Trong `check` in ra? | Ghi chú |
|---|---|---|---|
| `wake paths (KT-151)` | **CÓ** — dòng 292 | **CÓ** | `errors=0` |
| `selftest wake paths (KT-151)` | **CÓ** — dòng 293 | **CÓ** | `7/7 ok` |
| `L0 kernel workspace (KT-148)` | **CÓ** — dòng 400 | **KHÔNG** (nằm trong `do_gate_fast`, không phải `do_check`) | Đúng thiết kế D2-128 ("vào `gate --fast`"); tôi **không** chạy `gate --fast` theo luật. |
| `selftest t26 (KT-149: g1..g4)` | **CÓ** trên cây merge `4e6dcea1` (dòng 262); **KHÔNG** trên HEAD `a9460f0c` | **CÓ** trên `4e6dcea1` (`--- selftest t26 (KT-149: g1..g4): OK`) | Merge KT-149 đến trong lúc review; `v2 check` trên `4e6dcea1` PASS (`kt153-check-merged.log`). |

⇒ **Kết luận 6.1**: trên cây merge `4e6dcea1`, `check` gọi đủ **ba** bước mới: `selftest t26`
(KT-149), `wake paths` + `selftest wake paths` (KT-151); bước `L0 kernel workspace` (KT-148)
nằm đúng trong `gate --fast` (không chạy theo luật). Trên HEAD `a9460f0c` (đầu review), bước
KT-149 còn thiếu — merge đến sau.

### 6.2 Không có test nào bị mất

| Tệp | Trước `aef3e372` | Sau HEAD | Kết luận |
|---|---|---|---|
| `v2/test/boot/shell-edge.sh` | 26 ca `selftest_*` | 26 | **không đổi** (tệp không nằm trong diff) |
| `v2/test/boot/ktest.sh` | 7 dòng `^pager ` | 8 | **+1** (`pager giao dung mot lan`) |
| `v2/lib/vifast-vfs/tests/pager.rs` | 12 `#[test]` | 15 | **+3** |
| `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` | 3 `#[test]` | 3 | **không đổi** (tệp không nằm trong diff) |

Lệnh + output: `v2/agent/JOURNAL/kt153-logs/kt153-counts.log`. Không tệp nào **giảm** số ca.

---

## 7. Danh sách phát hiện

### Nặng

**N1. KT-149 chưa nằm trên `main` tại HEAD `a9460f0c` (đầu review).** `v2/test/boot/t26.sh`
không tồn tại trên HEAD; `v2/bin/v2` không có bước `t26.sh --selftest`; nhánh `night/L1-n2` là
`NOT-ANCESTOR`. **Đã giải quyết**: lead merge lên `main` tại `4e6dcea1` trong lúc review; cây
merge có `t26.sh` + bước `selftest t26`, `v2 check` PASS. Lệnh + output: §2(A).

**N2. KT-151: cổng xanh trước đường wake thứ hai.** Đột biến L6-151 (thêm `wake_direct_l6`,
giữ producer `MailboxMsg::Wake`) cho `check-wake-paths.py` mặc định `errors=0` và
`kt151_wake` `5 passed`; chỉ `--strict` (không trong cổng) `errors=6`. Đúng tiêu chí (c) của
D2-131 nhưng **không** đạt. Lệnh + output: §4(B).

### Vừa

**V1. KT-150: checker host tự đối chiếu sổ của chính harness.** `check_delivered_once` chỉ đỏ
khi bật `replay_on_resume` của harness; `PagerCore` (mã sản phẩm) không được kiểm
"giao đúng một lần". Bằng chứng: cấu trúc `fault()`/`send()` trong `pager.rs` (§3(A)).

**V2. KT-150: ca 4 client chỉ mức host.** Kernel vẫn `CLIENTS = 2`
(`v2/kernel/vifast-kbin-riscv64/init/pager_common.rs:70`). Journal ghi nợ — trung thực, nhưng
D2-130 (b) không giới hạn mức host.

**V3. KT-150: vế 1 không đỏ trên ảnh `v1-archive`.** Ảnh `v1-archive` cho `PAGER EVICT OK` = 1,
`PAGER2 DUP` = 0; đỏ đo trên mã **trước r309** qua test host do tác giả thêm. Journal ghi đúng
nhãn — trung thực, nhưng không phải "đỏ trên ảnh cũ".

**V4. KT-152: vế 1 chỉ là trích dẫn, chưa đo lại.** Không có lần chạy 600 boot x86_64; lưới
chắn chỉ ở mức mô hình, không nối `rdtime`. Journal ghi nhãn "CHƯA ĐO LẠI" — trung thực.

**V5. KT-149: vế 1 không đỏ trên tag `v1-archive`.** Tag đã vá r296b ⇒ 0/50 T-FS; 5/50 T-FS
đo trên `5fc7b3ff` (trước vá). Journal ghi đúng — trung thực, nhưng "vế 1 theo đúng chữ tag"
**không đạt**.

**V6. KT-151: `vifast-sync::Mailbox` không được kernel/sim dùng.** Chỉ test host
`kt30_sync.rs` chạm; kernel dùng `vifast-core::mailbox::MailboxMsg`, sim dùng bản port. Journal
ghi nợ N7-1 — trung thực, nhưng D2-131 nói "đi qua `vifast-sync::Mailbox`".

**V7. KT-151: `Machine::wake` không có đường sản phẩm.** Chỉ test gọi
(`ktest_inv.rs:51`, `ktest_user.rs:692`, test sim). Journal ghi nợ N7-2 — trung thực, nhưng
kênh hộp thư chưa được sản phẩm dùng.

### Nhẹ

**L1. KT-148: đột biến của tác giả là đột biến test, không phải đột biến mã.** Nó chứng minh
"test đỏ khi test sai", không chứng minh "test bắt mã sai". Đột biến L6-148 (mã) mới là phép
thử đúng — và bị const-assert chặn ở compile-time.

**L2. KT-148: journal ghi `0 ignored`, thực tế `1 ignored`** (doc-test `PanicHandler` bị
`ignore` có chủ đích). Lệch số nhẹ.

**L3. KT-151: `RE_ARCH_FORBIDDEN` định nghĩa nhưng không dùng** (`check-wake-paths.py:94`).

**L4. KT-151: số dòng trong journal lệch HEAD** — `kobj.rs:5707`/`6045` (nhánh) vs
`5718`/`6056` (HEAD, sau merge KT-152). Nhẹ.

**L5. KT-149: dùng `git worktree add` trên repo gốc chỉ-đọc** (journal §1.1) — ghi
`.git/worktrees/` vào repo gốc; KT-151 đã tránh bằng `git archive`. Quy trình, không phải mã.

**L6. KT-149: đường dẫn log trong journal lệch tên tệp** — `host-green.log` /
`host-mutation-red.log` vs thực tế `kt149-host-green.log` / `kt149-host-mutation-red.log`.

**L7. KT-152: `50` dòng `test result` trong journal vs `51` trên HEAD** — merge KT-151 thêm
`kt151_wake`. Đúng cho nhánh, lệch HEAD.

---

## 8. Kết luận từng task

| Task | Kết luận | Ghi chú |
|---|---|---|
| **KT-148** | **ĐẠT** | Test sửa đúng; bước cổng có; đột biến mã bị const-assert chặn. Đột biến của tác giả yếu (sửa test). |
| **KT-149** | **ĐẠT CÓ ĐIỀU KIỆN** | Tại HEAD `a9460f0c` chưa merge; đã merge tại `4e6dcea1` trong lúc review, `v2 check` PASS có bước `selftest t26`. Vế 1 đo trên mã trước vá, không phải tag. |
| **KT-150** | **ĐẠT CÓ ĐIỀU KIỆN** | TAP 37 kernel thật; checker host tự-quy chiếu; 4 client host-only; vế 1 trước r309. |
| **KT-151** | **ĐẠT CÓ ĐIỀU KIỆN** | Kênh chéo hart thật; cổng **không** bắt đường wake thứ hai; N7-1/N7-2 còn nợ. |
| **KT-152** | **ĐẠT CÓ ĐIỀU KIỆN** | Test + 2 đột biến thật; vế 1 chỉ trích dẫn; lưới chắn mức mô hình. |

**Task chưa review**: không. Cả năm task đều đã đọc diff + chạy đột biến/kiểm số. KT-149 được
review trên **nhánh chưa merge** tại HEAD `a9460f0c`, rồi **kiểm lại trên cây merge `4e6dcea1`**
sau khi lead merge trong lúc review.

---

## 9. Cam kết cây sạch

Trước khi commit tệp journal này (cây `/private/tmp/night-L6-review`, HEAD `a9460f0c`):

```text
$ git status --short
?? v2/agent/JOURNAL/kt153-logs/
?? v2/agent/JOURNAL/kt153-review.md
$ git diff --stat
(rong)
```

`git diff` rỗng ⇒ **không có thay đổi mã** nào còn sót. Hai mục `??` là tệp journal + log của
chính review này (đã commit trong `[KT-153]`). Không commit bất kỳ thay đổi mã nào; chỉ commit
`v2/agent/JOURNAL/kt153-review.md` + `v2/agent/JOURNAL/kt153-logs/**`.

**Ghi chú race**: `main` đã tiến từ `a9460f0c` lên `4e6dcea1` (merge KT-149) trong lúc review.
Mọi kết luận ở trên ghi rõ cây nào: HEAD `a9460f0c` (đầu review) hay cây merge `4e6dcea1`
(kiểm lại). `./v2/bin/v2 check` cuối trên **cả hai** cây đều `CHECK_EXIT=0`.
