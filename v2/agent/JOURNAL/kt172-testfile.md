# KT-172 — đóng nốt lỗ hổng `is_test_file_heuristic` cho **luật 1–5** của `check-wake-paths.py`

**Ngày**: 2026-11-19 (vòng hai) · **Agent 2 (thợ) L15** · nhánh `night2/L15-testfile` ·
worktree `/private/tmp/night2-L15-testfile` · HEAD `2dc03fed` (từ `main`).
**Nguồn đọc trước**: [`kt171-wakefix3.md`](kt171-wakefix3.md) §"Còn thiếu" mục 1,
`v2/tools/check-wake-paths.py`, `v2/agent/DECISIONS.md` D2-155.
**Ngân sách**: hạn cứng 08:00 JST. Không merge, không push, không `gate --fast`, không tạo
worktree mới, không QEMU.

## 0. Kết luận trước

| Việc | Trạng thái | Căn cứ (một dòng) |
|---|---|---|
| 4 call site `is_test_file_heuristic` → `is_test_file` (đồ thị module) | **XONG** | luật 1/2/3/5 (`check-wake-paths.py:1158/1219/1254/1305`); `test_only` tính **một lần** ở đầu `scan_v2` |
| Xoá `is_test_file_heuristic` | **XONG** | không còn định nghĩa "test-only" thứ hai; chỉ còn `is_test_file` + `test_only_modules` |
| Hệ quả `proc_demo.rs` thành test-only ⇒ `E_STRICT_STALE` | **XONG** | **bỏ** 2 mục `NON_WAKE_READY_SITES` (không nới luật; xem §2) |
| Đột biến A: `ktest_evil.rs` + `mod` từ `main.rs`, `transition_tcb(…, Ready)` trực tiếp | **ĐỎ đúng** | `E_STRICT_TRANSITION` @ `v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs:8 (duong_san_pham)` |
| Đột biến B: `vifast-core/src/tests/evil.rs` + `mod tests;` không `#[cfg(test)]` | **ĐỎ đúng** | `E_STRICT_TRANSITION` @ `v2/kernel/vifast-core/src/tests/evil.rs:7 (duong_san_pham)` |
| Gỡ cả hai đột biến | **XANH** | `errors=0` |
| Selftest | **46/46** | 39 ca cũ **vẫn xanh** + **7 ca KT-172** |
| `./v2/bin/v2 check` | **PASS** | log `kt172-logs/kt172-v2-check.log` |

Không còn mục nào phải tuyên bố "chưa đóng".

## 1. Đã sửa gì

### 1.1 Bốn call site của luật 1–5 → `is_test_file` (đồ thị module)

Trước KT-172, `is_test_file_heuristic` (miễn theo **tên** `ktest*`/`*_tests.rs`/`tls_tests.rs`
và theo **đường dẫn** `/tests/`) được dùng ở **bốn** chỗ:

| # | Luật | Dòng (trước) | Mã lỗi |
|---|---|---|---|
| 1 | Kênh chéo hart — producer `MailboxMsg::{Wake,Reschedule,Migrate}` | 1142 | `E_CHANNEL_MISSING`/`E_CHANNEL_PRODUCER` |
| 2 | Consumer `MailboxMsg::*` chỉ trong `apply_mail` | 1203 | `E_CONSUMER` |
| 3 | Tầng arch không có kênh wake riêng | 1238 | `E_ARCH_WAKE`/`E_ARCH_DOORBELL` |
| 5 | `--strict` allowlist `transition_tcb(…, TcbState::Ready)` | 1289 | `E_STRICT_TRANSITION` |

Nay cả bốn dùng `is_test_file(p, root, test_only)` — cùng hàm mà KT-171 đã dùng cho luật
`wake`. `test_only = test_only_modules(root)` được tính **một lần** ở đầu `scan_v2` và truyền
vào cả luật 1–5 **lẫn** luật `wake` (bỏ lần tính lại trong nhánh `if strict:`), nên **không
thể** có hai định nghĩa "test-only" lệch nhau.

`is_test_file_heuristic` **xoá hẳn** (không còn call site nào). Đây là hướng **siết**: không
còn đường nào miễn theo tên/thư mục.

### 1.2 Hệ quả đã biết — `proc_demo.rs` thành test-only

`proc_demo.rs` **chỉ** được `mod proc_demo;` từ `ktest_proc.rs` (`src/ktest_proc.rs:49`), mà
`ktest_proc.rs` là bin `vifast-kbin-ktest-proc` với `required-features = ["ktest", "user"]`
⇒ **test-only thật** theo đồ thị module. Vì vậy hai mục allowlist của nó:

```
("v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs", "build_rig", 1)
("v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs", "task_ready", 1)
```

không còn được quét ⇒ `E_STRICT_STALE` đỏ (đã đo đúng như dự đoán: `errors=2`, hai dòng
`allowlist có mục không còn khớp: … proc_demo.rs build_rig #1 / task_ready #1`).

**Cách xử lý: BỎ hai mục** (không thêm lý do, không nới `E_STRICT_STALE`). Lý do chọn hướng
này thay vì "giữ mục + thêm lý do":

- giữ mục = allowlist chứa mục cho mã **không còn được quét** ⇒ đúng thứ `E_STRICT_STALE`
  sinh ra để bắt (allowlist mục);
- nếu mai kia `proc_demo.rs` được `mod` từ `main.rs` (thành **sản phẩm**), mục cũ sẽ **âm thầm
  miễn** hai chỗ chuyển `Ready` — đúng lỗ hổng KT-172 vừa đóng. Bỏ mục ⇒ thành sản phẩm là
  `E_STRICT_TRANSITION` đỏ **ngay** (fail-closed).

Không nới ngưỡng, không thêm ngoại lệ, không `#[ignore]`.

### 1.3 Ba tệp đổi trạng thái test-only (đã kiểm, không có hệ quả đỏ)

Đo `test_only_modules` trên cây thật: **97** tệp test-only theo đồ thị vs **94** theo heuristic
cũ. Chênh lệch **một chiều** (không tệp nào trước đây được miễn mà nay thành sản phẩm ⇒
không có chỗ nào **mới** bị quét, nên luật 1–3 không phát sinh đỏ):

| Tệp | Vì sao test-only | Hệ quả |
|---|---|---|
| `v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs` | `mod` từ `ktest_proc.rs` (bin `ktest`) | 2 mục allowlist bỏ (§1.2) |
| `v2/kernel/vifast-plat-riscv64/src/probe.rs` | `mod` dưới `#[cfg(feature = "ktest")]` | luật 3 không quét nữa (đúng: ảnh test) |
| `v2/kernel/vifast-kbin-riscv64/init/pager_common.rs` | `mod` từ ngữ cảnh test | không thuộc luật nào đang bật |

## 2. Bằng chứng (dán nguyên văn)

### 2.1 Cây sạch — `--strict` `errors=0`

```
$ python3 v2/tools/check-wake-paths.py --strict
…
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
$ echo $?
0
```

(log đầy đủ: `kt172-logs/kt172-clean-strict.log`)

### 2.2 Đột biến A — `ktest_evil.rs` + `mod ktest_evil;` trong `main.rs` (ảnh **sản phẩm**)

Tệp dựng lại **đúng** cách reviewer L11 đã phá (KT-168 §1.2 B8):

```rust
// v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs
impl M {
    fn duong_san_pham(&mut self, tcb: u32) -> u8 {
        self.transition_tcb(tcb, TcbState::Ready)   // ← trực tiếp, KHÔNG qua wake_from
    }
}
```

```rust
// v2/kernel/vifast-kbin-riscv64/src/main.rs  (thêm sau khối `mod proc_rt;`)
mod ktest_evil;
```

```
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TRANSITION 1 chỗ chuyển `TcbState::Ready` ngoài cửa wake (allowlist tường minh KT-155)
E E_STRICT_TRANSITION   v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs:8 (duong_san_pham)
# check-wake-paths: mode=v2 strict=1 errors=2 warnings=0
$ echo $?
1
```

**Đối chứng "lỗ hổng là thật"**: chạy **checker của HEAD `2dc03fed`** (trước KT-172) trên
**cùng** cây đột biến:

```
$ git show HEAD:v2/tools/check-wake-paths.py > /tmp/kt172-old-checker.py
$ python3 /tmp/kt172-old-checker.py --root . --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
$ echo $?
0
```

⇒ Trước KT-172: **XANH** (`errors=0`). Sau KT-172: **ĐỎ** `E_STRICT_TRANSITION` @
`ktest_evil.rs:8`. (log: `kt172-logs/kt172-mutation-A-truoc-fix.log`)

### 2.3 Đột biến B — `vifast-core/src/tests/evil.rs` + `mod tests;` **không** `#[cfg(test)]`

```rust
// v2/kernel/vifast-core/src/tests.rs
mod evil;
// v2/kernel/vifast-core/src/tests/evil.rs
impl M {
    fn duong_san_pham(&mut self, tcb: u32) -> u8 {
        self.transition_tcb(tcb, TcbState::Ready)   // ← trực tiếp
    }
}
// v2/kernel/vifast-core/src/lib.rs  (thêm sau `#[cfg(test)] mod tls_tests;`)
mod tests;
```

```
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TRANSITION 1 chỗ chuyển `TcbState::Ready` ngoài cửa wake (allowlist tường minh KT-155)
E E_STRICT_TRANSITION   v2/kernel/vifast-core/src/tests/evil.rs:7 (duong_san_pham)
# check-wake-paths: mode=v2 strict=1 errors=2 warnings=0
$ echo $?
1
```

Đối chứng trước fix (cùng checker HEAD): `errors=0` — log
`kt172-logs/kt172-mutation-B-truoc-fix.log`.

### 2.4 Gỡ cả hai đột biến ⇒ `errors=0`

```
$ rm v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs
$ git checkout -- v2/kernel/vifast-kbin-riscv64/src/main.rs
$ rm -rf v2/kernel/vifast-core/src/tests v2/kernel/vifast-core/src/tests.rs
$ git checkout -- v2/kernel/vifast-core/src/lib.rs
$ git status --short
 M v2/tools/check-wake-paths.py
?? v2/agent/JOURNAL/kt172-logs/
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
```

### 2.5 `--selftest` — 46/46 (39 ca cũ vẫn xanh + 7 ca mới)

```
$ python3 v2/tools/check-wake-paths.py --selftest
…
ok - KT-172 A: `ktest_evil.rs` + `mod` từ `main.rs`, `transition_tcb(…, Ready)` trực tiếp ⇒ đỏ `E_STRICT_TRANSITION`
ok - KT-172 A2: `#[cfg(feature = "ktest")] mod ktest_evil;` ⇒ xanh (test-only thật)
ok - KT-172 B: `src/tests/evil.rs` + `mod tests;` không `#[cfg(test)]`, `transition_tcb(…, Ready)` trực tiếp ⇒ đỏ `E_STRICT_TRANSITION`
ok - KT-172 B2: `#[cfg(test)] mod tests;` ⇒ xanh (test-only thật)
ok - KT-172 C: luật 1 — `ktest_evil.rs` (sản phẩm) phát `MailboxMsg::Wake` ngoài cửa wake ⇒ đỏ `E_CHANNEL_PRODUCER`
ok - KT-172 D: luật 2 — `ktest_evil.rs` (sản phẩm) xử lý `MailboxMsg::Wake` ngoài `apply_mail` ⇒ đỏ `E_CONSUMER`
ok - KT-172 E: luật 3 — `ktest_evil.rs` (sản phẩm) trong `vifast-plat-*` dùng `wake_step` ⇒ đỏ `E_ARCH_WAKE`
# selftest: 46/46 ok
$ echo $?
0
```

(log đầy đủ: `kt172-logs/kt172-selftest.log`; `grep -c '^ok - '` = **46**, `not ok` = **0**)

### 2.6 `./v2/bin/v2 check` — PASS

```
$ ./v2/bin/v2 check
== v2 check (HEAD 2dc03fed) ==
--- fmt
--- fmt: OK
…
--- wake paths (KT-151: một cơ chế wake): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155: allowlist): OK
…
v2 check: PASS (L1 chạy trong gate --fast)
$ echo $?
0
```

(log đầy đủ: `kt172-logs/kt172-v2-check.log`)

## 3. Bảy ca selftest mới (KT-172)

| Ca | Nội dung | Kỳ vọng |
|---|---|---|
| **A** | `ktest_evil.rs` + `mod` từ `main.rs`, `transition_tcb(…, Ready)` trực tiếp | ĐỎ `E_STRICT_TRANSITION` |
| **A2** | cùng tệp, `#[cfg(feature = "ktest")] mod` | XANH (test-only thật) |
| **B** | `src/tests/evil.rs` + `mod tests;` không `#[cfg(test)]`, `transition_tcb(…, Ready)` | ĐỎ `E_STRICT_TRANSITION` |
| **B2** | `#[cfg(test)] mod tests;` | XANH (test-only thật) |
| **C** | luật 1: `ktest_evil.rs` (sản phẩm) phát `MailboxMsg::Wake` ngoài cửa wake | ĐỎ `E_CHANNEL_PRODUCER` |
| **D** | luật 2: `ktest_evil.rs` (sản phẩm) xử lý `MailboxMsg::Wake` ngoài `apply_mail` | ĐỎ `E_CONSUMER` |
| **E** | luật 3: `ktest_evil.rs` (sản phẩm) trong `vifast-plat-*` dùng `wake_step` | ĐỎ `E_ARCH_WAKE` |

Vì sao có C/D/E: đề chỉ đòi ca cho **hai đột biến** (luật 5). Nhưng KT-172 đổi **bốn** call
site; nếu chỉ kiểm luật 5 thì ba call site còn lại có thể bị revert mà selftest vẫn xanh. Ba ca
C/D/E khoá đúng ba call site đó bằng **cùng** kỹ thuật "tệp tên `ktest*` nhưng `mod` từ crate
root sản phẩm". A2/B2 là đối chứng dương để chứng minh luật không phải "thấy `ktest*` là đỏ".

## 4. Còn thiếu / bàn giao

- **Không còn mục nào của KT-172 chưa đóng.** Bốn call site đã đổi, heuristic đã xoá, allowlist
  đã làm sạch, hai đột biến bắt buộc đều đỏ đúng mã + `file:dòng`, gỡ ⇒ `errors=0`,
  selftest 46/46, `v2 check` PASS.
- **Không** chạy `gate --fast` (đề cấm), **không** QEMU, **không** merge/push.
- **Bàn giao cho lead**: commit `[KT-172] …` trên nhánh `night2/L15-testfile`; lead quyết
  merge. Nếu lead muốn KT-172 vào `DECISIONS.md` (D2-xxx) thì đó là việc của Agent 0 — tôi
  **không** sửa `DECISIONS.md`/`QUEUE.md`.
- **Ghi chú cho reviewer**: `E_STRICT_STALE` vẫn fail-closed hai chiều như cũ; chỉ **dữ liệu**
  allowlist đổi (bỏ 2 mục của một tệp đã thành test-only), **không** đổi luật. Muốn kiểm:
  `git diff v2/tools/check-wake-paths.py` — phần `NON_WAKE_READY_SITES` chỉ **xoá** 2 dòng và
  thêm chú thích, không thêm mục nào.

## 5. Tệp chạm

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | 4 call site → `is_test_file`; xoá `is_test_file_heuristic`; `test_only` tính một lần; bỏ 2 mục `proc_demo.rs`; +7 ca selftest; docstring KT-172 |
| `v2/agent/JOURNAL/kt172-testfile.md` | tệp này |
| `v2/agent/JOURNAL/kt172-logs/*.log` | 7 log bằng chứng (clean-strict, mutation A/B, A/B trước fix, selftest, v2 check) |

Không chạm `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
`docs/v1/`, `v2/agent/{QUEUE,STATE,BLOCKED,DECISIONS}.md`.
