# KT-159 — N7 nợ: `wake()` chỉ-dùng-cho-test + luật checker + ghim hai ngữ nghĩa

**Ngày**: 2026-11-19 (vòng hai) · **Agent 2 (L3)** · nhánh `night2/L3-wake2` · worktree
`/tmp/night2-L3-wake2` · HEAD trước task: `6b2fdce7` (tách từ `main`).
**Nguồn nợ**: `v2/agent/DECISIONS.md` **D2-144** (Agent 1 quyết thay Agent 0, 2026-11-19 03:55
JST) + `v2/agent/JOURNAL/kt154-n7fix.md` §11 (N7-2) + `kt155-n7-dong.md`.
**Ngân sách**: 2 h. **Không** merge, **không** push, **không** `gate --fast`, **không** QEMU.

## 0. Việc được giao (nguyên văn, trích)

> 1. Đọc `core_sched.rs` (`wake` ~221, `wake_from` ~243, `wake_from_unlink` ~258),
>    `check-wake-paths.py` (allowlist + `--strict` + `--selftest` 11 ca), `kt151_wake.rs`.
> 2. **Doc-comment** `wake()`: ghi rõ "CHỈ dùng cho test/ảnh test; sản phẩm dùng
>    `wake_from`/`wake_from_unlink`", kèm lý do hai ngữ nghĩa.
> 3. **Luật checker mới** trong `check-wake-paths.py --strict`: mọi lời gọi `wake(` trong
>    `v2/kernel/vifast-core/src/` hoặc `v2/sim/vifast-sim/src/` (ngoài `#[cfg(test)]`/`mod
>    tests`) ⇒ **ĐỎ** (`E_STRICT_TEST_ONLY_WAKE`), fail-closed. Phải **bỏ qua chú thích**.
>    Thêm ca `--selftest` (log giả có `wake(` ⇒ đỏ; chỉ có `wake_from(` ⇒ xanh).
> 4. **Test ghim hành vi**: (a) `wake()` chéo hart ⇒ TCB **không** đổi trạng thái ngay + đúng
>    **một** `Wake`; (b) `wake_from()` chéo hart ⇒ `Ready` **ngay** + đúng **một** `Wake`;
>    (c) cùng hart: cả hai `Ready` ngay, **không** gửi `Wake`.
> 5. **Đột biến bắt buộc**: thêm `wake(` trong `vifast-core/src/` ⇒ `--strict` **ĐỎ** với
>    `E_STRICT_TEST_ONLY_WAKE`; gỡ ra ⇒ xanh. Ghi log.
> 6. `./v2/bin/v2 check` phải PASS.

## 1. Kết luận trước (đọc nhanh)

| Mục | Trạng thái | Bằng chứng |
|---|---|---|
| (2) Doc-comment `wake()` kernel + sim | **ĐẠT** | `core_sched.rs:219-243`, `kobj.rs:5705-5730` |
| (3) Luật `E_STRICT_TEST_ONLY_WAKE` | **ĐẠT** | `kt159-strict-clean.log` (`errors=0`), `kt159-strict-mutation.log` (`errors=1`) |
| (3b) `--selftest` +4 ca | **ĐẠT** | `kt159-selftest.log` — **15/15 ok** (trước: 11/11) |
| (4) Test ghim 3 ca × 2 tầng | **ĐẠT** | `kt159-test-pin.log` — **6 passed; 0 failed** |
| (5) Đột biến bắt buộc | **ĐẠT** | `kt159-strict-mutation.log` — ĐỎ `E_STRICT_TEST_ONLY_WAKE` @ `core_ipc.rs:697`, gỡ ⇒ `errors=0` |
| (6) `./v2/bin/v2 check` | **ĐẠT** | `kt159-check.log` — `v2 check: PASS` |
| Test ghim **có răng** (đột biến ngữ nghĩa) | **ĐẠT** | `kt159-test-bites.log` — 2 đột biến, mỗi cái giết đúng 1 ca |
| Bẫy là **thật**, không phải kiểu cách | **ĐẠT** | `kt159-trap-real.log` — `wake()` ở `reply_with` ⇒ C2 vỡ |

**Không** sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
`docs/v1/`. **Không** nới ngưỡng/checker nào — chỉ **siết** thêm một luật. **Không**
`#[ignore]`, **không** `#[should_panic]`, **không** retry để lấy xanh.

## 2. Đo được trước khi làm (baseline, cây sạch `6b2fdce7`)

```
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 11/11 ok
$ grep -rn "wake(" v2/kernel/vifast-core/src/ v2/sim/vifast-sim/src/
v2/kernel/vifast-core/src/core_sched.rs:221:    pub fn wake(&mut self, tcb: ObjId) -> Result<(), KError> {
v2/sim/vifast-sim/src/kobj.rs:5715:    pub fn wake(&mut self, tcb: ObjId) -> Result<(), KError> {
```

Hai dòng duy nhất là **định nghĩa** — khớp D2-144 ("`wake()` không có call site sản phẩm
nào"). Đây cũng là lý do luật mới **phải** bỏ qua dòng `fn wake(`: nếu không, chính chỗ khai
báo API làm cổng đỏ.

## 3. Doc-comment `wake()` — "CHỈ dùng cho test" + lý do hai ngữ nghĩa

Sửa **chỉ chú thích**, không đổi một dòng mã nào:

- `v2/kernel/vifast-core/src/core_sched.rs` — `wake()` thêm mục
  `# CHỈ dùng cho test / ảnh test — sản phẩm dùng Machine::wake_from`, nêu:
  - đo được ở D2-144: không call site sản phẩm, chỉ `ktest_inv.rs:51`, `ktest_user.rs:692` +
    test sim;
  - luật máy kiểm `E_STRICT_TEST_ONLY_WAKE` (để người đọc doc biết vì sao cổng đỏ);
  - **vì sao hai ngữ nghĩa**: `wake()` = post-only (đúng luật 3 — hart này không ghi trạng thái
    hart khác; đúng cho người gọi **không** giữ bookkeeping) vs `wake_from()` = post + `Ready`
    ngay (đường sản phẩm đã tự gỡ bookkeeping ⇒ trạng thái **phải** nhất quán ngay, hoãn sẽ vi
    phạm C2 `Reply.caller ⇔ BlockedOnReply` và `check_ipc`);
  - câu chốt: gọi `wake()` trong mã sản phẩm là **bẫy**.
- `v2/sim/vifast-sim/src/kobj.rs` — bản **đối xứng** cho sim (cùng luật, cùng lý do, trỏ về doc
  của kernel).

## 4. Luật checker mới: `E_STRICT_TEST_ONLY_WAKE`

`v2/tools/check-wake-paths.py`:

```python
TEST_ONLY_WAKE_SCOPE = ("v2/kernel/vifast-core/src/", "v2/sim/vifast-sim/src/")
RE_TEST_ONLY_WAKE_CALL = re.compile(r"(?<![A-Za-z0-9_])wake\s*\(")
RE_FN_WAKE_DEF = re.compile(r"\bfn\s+wake\s*\(")
```

Chạy trong nhánh `if strict:` (mục 6), **fail-closed**:

- quét **mọi** tệp `.rs` dưới hai thư mục trên, **trừ** tệp test theo tên (`is_test_file`) và
  **trừ** khoảng dòng `#[cfg(test)] mod … { … }` (`test_line_ranges` — tái dùng hạ tầng có sẵn);
- **bỏ chú thích** bằng `strip_line_comment()` (mới): cắt `//` **ngoài** chuỗi (đếm nháy kép
  trước đó: số lẻ ⇒ `//` nằm trong chuỗi, ví dụ `"http://…"`). Nhờ vậy doc-comment của chính
  `wake()` (có chữ `wake(`) **không** bị báo oan;
- **bỏ dòng định nghĩa** `fn wake(` — đó là khai báo API, không phải lời gọi;
- regex **không** khớp `wake_from(`, `wake_from_unlink(`, `wake_step(`, `wake_addr(` (sau `wake`
  là `_` chứ không phải `(`) và **không** khớp `MailboxMsg::Wake(` (phân biệt hoa thường);
  lookbehind chặn hậu tố kiểu `rewake(`.

Phạm vi **hẹp có chủ ý**: ảnh test (`vifast-kbin-riscv64/src/ktest*.rs`) và `tests/` **được**
gọi `wake()` — đó là mục đích duy nhất còn lại của nó (D2-144a).

### 4.1 `--strict` trên cây sạch — `errors=0`

```
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
EXIT=0
```

(`kt159-strict-clean.log`)

### 4.2 `--selftest` — 11 → **15** ca, PASS

Bốn ca mới:

| Ca | Kỳ vọng | Kết quả |
|---|---|---|
| `wake(` trong mã sản phẩm (`core_ipc.rs`) | ĐỎ `E_STRICT_TEST_ONLY_WAKE` | ok |
| **chỉ** `wake_from(`/`wake_from_unlink(`/`wake_step(` | XANH (đối chứng dương) | ok |
| `wake(` trong `#[cfg(test)] mod tests` | XANH | ok |
| `wake(` trong chú thích (`//!`, `///`, `//`) | XANH (không báo oan) | ok |

```
$ python3 v2/tools/check-wake-paths.py --selftest
ok - v2 `--strict` gọi `wake(` trong mã sản phẩm ⇒ đỏ (E_STRICT_TEST_ONLY_WAKE)
ok - v2 `--strict` chỉ `wake_from(`/`wake_from_unlink(`/`wake_step(` ⇒ xanh
ok - v2 `--strict` `wake(` trong `#[cfg(test)]` ⇒ xanh
ok - v2 `--strict` `wake(` trong chú thích ⇒ xanh (không báo oan)
...
# selftest: 15/15 ok
EXIT=0
```

(`kt159-selftest.log`)

## 5. Test ghim hành vi — `v2/sim/vifast-sim/tests/kt159_wake_semantics.rs` (mới)

6 ca = 3 ngữ nghĩa × 2 tầng (kernel model `vifast-core` + sim oracle). Rig: 2 hart, 1 TCB
(slot 8, `home` cấu hình được, ưu tiên 5, affinity `0b11`) + 1 Notification (slot 9). Chặn TCB
bằng `notification_wait` (waiter ghi trên object — bất biến "waiter ⇔ TcbState").

- **(a) `wake()` chéo hart = post-only** — `kt159_{core,sim}_wake_cheo_hart_post_only`:
  - TCB **vẫn** `BlockedOnNotification` ngay sau `wake()` (không đổi trạng thái);
  - `mail_len(1) == 1`, `mail_log(1)` **rỗng** (chưa xử lý);
  - `notification_waiter(ntfn) == Some(t0)` — post-only **không** gỡ bookkeeping;
  - hart `home` `process_mail` ⇒ `mail_log == [Wake(t0)]` (đúng **một**), TCB `Running{hart:1}`.
- **(b) `wake_from()` chéo hart = post + `Ready` ngay** —
  `kt159_{core,sim}_wake_from_cheo_hart_ready_ngay`. Đường **sản phẩm**:
  `notification_signal` (gọi `wake_from` — `core_sync.rs:61` / `kobj.rs:8856`):
  - TCB `Ready` **ngay** (không chờ hart `home`);
  - `mail_len(1) == 1` (vẫn đánh thức hart `home`), `mail_log(1)` rỗng;
  - `process_mail(1)` ⇒ `mail_log == [Wake(t0)]`, TCB `Running{hart:1}`.
- **(c) cùng hart: cả hai `Ready` ngay, không `Wake`** —
  `kt159_{core,sim}_cung_hart_ca_hai_ready_ngay_khong_wake`: `wake()` rồi `wake_from()` (qua
  `notification_signal`) với `current_hart == home == 1` ⇒ `Ready` ngay **và** `mail_len(1) == 0`
  cả hai lần.

Bất biến kiểm sau **mỗi** bước: `check_sched` + `check_mailboxes` + `check_ipc` +
`check_notifications`.

```
$ cd v2 && cargo test -p vifast-sim --test kt159_wake_semantics
running 6 tests
test kt159_core_wake_cheo_hart_post_only ... ok
test kt159_core_wake_from_cheo_hart_ready_ngay ... ok
test kt159_core_cung_hart_ca_hai_ready_ngay_khong_wake ... ok
test kt159_sim_wake_cheo_hart_post_only ... ok
test kt159_sim_wake_from_cheo_hart_ready_ngay ... ok
test kt159_sim_cung_hart_ca_hai_ready_ngay_khong_wake ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0
```

(`kt159-test-pin.log`)

### 5.1 Test **có răng** — đột biến ngữ nghĩa giết đúng ca

Test ghim mà không đỏ dưới đột biến thì chỉ là trang trí. Hai đột biến trên `core_sched.rs`
(mỗi lần khôi phục ngay, `kt159-test-bites.log`):

| Đột biến | Ca phải chết | Kết quả |
|---|---|---|
| `wake()` bỏ nhánh post-only (thành post + `Ready`) | (a) `kt159_core_wake_cheo_hart_post_only` | **FAILED** — `wake() chéo hart KHÔNG được đổi trạng thái TCB ngay (post-only, D2-144)` |
| `wake_from()` bỏ `push_mail(Wake)` | (b) `kt159_core_wake_from_cheo_hart_ready_ngay` | **FAILED** — `wake_from() chéo hart vẫn phải gửi đúng một Wake về hart home` |

Khôi phục ⇒ `6 passed; 0 failed`.

## 6. Đột biến **bắt buộc** (mục 5 của đề) — luật checker bắt được

Đột biến **thật**, không phải thêm dòng rác: đổi đúng chỗ mà D2-144 mô tả là bẫy —
`core_ipc.rs` `reply_with` gọi `wake()` thay `wake_from()`:

```diff
@@ -696,3 +696,3 @@ impl Machine {
         if let Some(caller) = caller {
-            self.wake_from(caller)?;
+            self.wake(caller)?;
         }
```

```
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_ipc.rs:697: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
EXIT=1
```

Gỡ đột biến:

```
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
EXIT=0
```

(`kt159-strict-mutation.log`)

### 6.1 Bẫy là **thật** — không phải luật kiểu cách

Cùng đột biến đó, chạy test N7-2 có sẵn (`kt154_n7fix`) ⇒ **bất biến C2 vỡ**:

```
test kt154_core_reply_cheo_hart_phai_gui_wake ... FAILED
thread 'kt154_core_reply_cheo_hart_phai_gui_wake' panicked at kernel/vifast-core/src/core_sched.rs:409:13:
bất biến Reply (C2) vỡ sau public op: CheckFailure { what: "BlockedOnReply nhung Reply.caller lech", a: 6, b: 4 }
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Khôi phục ⇒ `7 passed; 0 failed`. Đây là **đúng** điều doc-comment mới viết: hoãn `Ready` sang
hart `home` trong khi `Reply.caller` đã bị gỡ ⇒ C2 đỏ. Luật `E_STRICT_TEST_ONLY_WAKE` chặn
đúng lớp lỗi này **trước** khi nó tới bất biến. (`kt159-trap-real.log`)

## 7. `./v2/bin/v2 check` — PASS

```
$ ./v2/bin/v2 check
...
--- wake paths (KT-151: một cơ chế wake): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155: allowlist): OK
...
v2 check: PASS (L1 chạy trong gate --fast)
EXIT=0
```

(`kt159-check.log`)

**Chạy lại sau commit** (cây đã có `JOURNAL/kt159-wake2.md` + 7 tệp log — để chắc chắn chính
nhật ký/log **không** làm cổng đỏ):

```
$ ./v2/bin/v2 check
--- wake paths --strict (KT-155: allowlist): OK
v2 check: PASS (L1 chạy trong gate --fast)
EXIT=0
```

(`kt159-check-postcommit.log`)

## 8. Tệp đã chạm

| Tệp | Loại | Ghi chú |
|---|---|---|
| `v2/kernel/vifast-core/src/core_sched.rs` | doc-comment | `wake()`: CHỈ-dùng-cho-test + lý do hai ngữ nghĩa. **0** dòng mã đổi. |
| `v2/sim/vifast-sim/src/kobj.rs` | doc-comment | bản đối xứng cho sim. **0** dòng mã đổi. |
| `v2/tools/check-wake-paths.py` | luật mới | `E_STRICT_TEST_ONLY_WAKE` + `strip_line_comment()` + 4 ca selftest + docstring. |
| `v2/sim/vifast-sim/tests/kt159_wake_semantics.rs` | **mới** | 6 ca ghim (3 ngữ nghĩa × 2 tầng). |
| `v2/agent/JOURNAL/kt159-wake2.md` + `kt159-logs/` | **mới** | nhật ký + log bằng chứng. |

## 9. Còn thiếu / ghi chú

- **Không** còn nợ kỹ thuật trong phạm vi KT-159. Ba mục (2)(3)(4) của D2-144 đã đóng bằng
  máy kiểm + test chạy, không bằng lời.
- **Không** chạy `gate --fast` (đề cấm — D2-148: lead chạy sau merge). `v2 check` PASS là cổng
  của sub-agent.
- **Không** chạy QEMU (task không cần; `wake()` chỉ có call site trong ảnh test, không phải
  đường boot sản phẩm).
- **Ghi nhận cho lead**: luật mới **hẹp có chủ ý** — chỉ hai thư mục `vifast-core/src` +
  `vifast-sim/src`. Nếu sau này có crate kernel mới (ví dụ `vifast-plat-aarch64` của KT-160)
  chứa đường wake, phải **mở rộng** `TEST_ONLY_WAKE_SCOPE`; hiện tại chưa cần vì crate đó chỉ
  có `Mmu` thuần dữ liệu.
- **Quan sát** (không sửa, ngoài phạm vi): `wake()` vẫn là `pub` và vẫn được ảnh test dùng —
  đúng D2-144a ("giữ `wake()` với ngữ nghĩa post-only"). Không có đề xuất đổi thành
  `#[cfg(test)]` vì ảnh test là crate khác (`vifast-kbin-riscv64`), không phải `cfg(test)` của
  `vifast-core`.
