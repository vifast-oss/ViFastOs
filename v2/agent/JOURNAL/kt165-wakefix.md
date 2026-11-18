# KT-165 — vá sáu lỗ hổng của luật `wake()` (review L6 phát hiện)

**Ngày**: 2026-11-19 (vòng hai) · **Agent 2 (L8)** · nhánh `night2/L8-wakefix` · worktree
`/private/tmp/night2-L8-wakefix` · HEAD trước task: `e022cdbe` (tách từ `main`).
**Nguồn nợ**: `v2/agent/JOURNAL/kt161-review.md` §2.3 (reviewer L6 phá luật `E_STRICT_TEST_ONLY_WAKE`
của KT-159 bằng **sáu** cách H1..H6) + §2.4 (điều kiện: mở phạm vi sang `vifast-kbin-riscv64/src`
và `vifast-plat-*/src`, chặn H3..H6).
**Ngân sách**: hạn cứng 06:15 JST. **Không** merge, **không** push, **không** `gate --fast`,
**không** QEMU, **không** tạo worktree mới.

## 0. Việc được giao (nguyên văn, trích)

> **Sáu lỗ hổng phải vá**:
> 1. **Phạm vi hẹp**: `TEST_ONLY_WAKE_SCOPE` chỉ có `vifast-core/src/` + `vifast-sim/src/`. Đặt
>    `m.wake(t)` trong `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs` ⇒ `--strict` vẫn `errors=0`.
>    **Mở rộng** phạm vi sang `v2/kernel/vifast-kbin-riscv64/src/` + `v2/kernel/vifast-plat-*/src/`.
> 2. **Alias / fn-pointer**: `let f = Machine::wake; f(t)` — tên `wake` không có `(` ngay sau.
> 3. **Block comment chen giữa**: `wake /*x*/ (t)`.
> 4. **Gọi cắt dòng**: `wake\n(t)`.
> 5. **`test_line_ranges` nuốt `mod` sản phẩm**: `#[cfg(test)]` trên một `use` cách `mod` ≤ 4
>    dòng ⇒ cả `mod` bị coi là vùng test.
> 6. (nếu có) bất kỳ cách lách nào khác ghi trong journal L6.
>
> - Sửa `v2/tools/check-wake-paths.py` (chỉ **siết**, không nới).
> - Thêm ca `--selftest` cho **từng** lỗ hổng: log giả chứa cách lách ⇒ **ĐỎ**; đối chứng dương
>   (chỉ `wake_from(`) ⇒ **XANH**. Giữ 15 ca cũ xanh.
> - **Chứng minh bằng đột biến thật** trên cây: `m.wake(t)` trong `proc_rt.rs` ⇒ `--strict` **ĐỎ**;
>   gỡ ⇒ `errors=0`. Làm tương tự cho alias `let f = Machine::wake;`.
> - `./v2/bin/v2 check` phải PASS.

## 1. Kết luận trước (đọc nhanh)

| Mục | Trạng thái | Bằng chứng |
|---|---|---|
| H1 `wake(` trong `vifast-kbin-riscv64/src/` | **ĐỎ** (trước: XANH) | `m1-proc-rt.log` — `E_STRICT_TEST_ONLY_WAKE` @ `proc_rt.rs:215` |
| H2 `wake(` trong `vifast-plat-*/src/` | **ĐỎ** (trước: XANH) | `m3-sau-cach-lach.log` — @ `vifast-plat-riscv64/src/lib.rs:904` |
| H2b crate plat **mới** (`-aarch64`) | **ĐỎ** | selftest ca `H2b` (mẫu `vifast-plat-*/src/`, không liệt kê cứng) |
| H3 alias `Machine::wake` (không `(`) | **ĐỎ** (trước: XANH) | `m2-alias.log` — @ `core_sched.rs:247` |
| H3b `use …::{wake, …}` | **ĐỎ** | selftest ca `H3b` |
| H4 `wake /*x*/ (t)` | **ĐỎ** (trước: XANH) | `m3-sau-cach-lach.log` — @ `core_ipc.rs:1448` |
| H5 `wake` ⏎ `(t)` | **ĐỎ** (trước: XANH) | `m3-sau-cach-lach.log` — @ `core_ipc.rs:1451` |
| H6 `#[cfg(test)]` trên `use` nuốt `mod` sản phẩm | **ĐỎ** (trước: XANH) | `m3-sau-cach-lach.log` — @ `core_ipc.rs:1461` |
| Cây sạch `--strict` | **XANH** `errors=0` | `clean-strict.log` |
| `--selftest` | **26/26 ok** (trước 15/15) | `selftest.log` |
| Selftest **có răng** (gỡ từng vá ⇒ ca đỏ) | **ĐẠT** | `selftest-teeth.log` — 4 phép gỡ, 7 ca đỏ đúng |
| `./v2/bin/v2 check` | **PASS** | `v2-check.log` |

**Không** sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
`docs/v1/`. **Không** nới ngưỡng/checker nào — chỉ **siết**. **Không** `#[ignore]`/
`#[should_panic]`, **không** retry để lấy xanh, **không** QEMU.

## 2. Sáu lỗ hổng → sáu vá

| # | Cách lách (KT-161 §2.3) | Cơ chế cũ hỏng | Vá (KT-165) |
|---|---|---|---|
| **H1** | `m.wake(t)` trong `vifast-kbin-riscv64/src/proc_rt.rs` | `TEST_ONLY_WAKE_SCOPE` chỉ có core + sim | thêm `v2/kernel/vifast-kbin-riscv64/src/` vào phạm vi |
| **H2** | `m.wake(t)` trong `vifast-plat-riscv64/src/lib.rs` | cùng lý do | **mẫu** `^v2/kernel/vifast-plat-[a-z0-9_]+/src/` ⇒ **mọi** crate plat, kể cả crate sinh sau |
| **H3** | `let f = Machine::wake; f(m, t)` | regex `wake\s*\(` đòi `(` ngay sau `wake` | quét **token** `wake`; `::wake` ⇒ ĐỎ; `use …wake` ⇒ ĐỎ; `wake` làm giá trị ⇒ ĐỎ |
| **H4** | `m.wake /* L6 */ (t)` | `strip_line_comment` chỉ cắt `//` | `strip_rust_noise` khử **cả** block comment (lồng nhau) + literal chuỗi/ký tự |
| **H5** | `m.wake` ⏎ `(t)` | checker chạy **từng dòng** | quét **toàn văn bản** đã khử nhiễu, đổi offset ⇒ số dòng bằng `bisect` |
| **H6** | `#[cfg(test)]` trên `use`, rồi `mod` sản phẩm trong ≤ 4 dòng | `test_line_ranges` tìm `mod … {` trong 4 dòng kế | chỉ nhận `#[cfg(test)]` gắn **trực tiếp** mục **có thân khối** (`mod`/`fn`/`impl`/…) |

## 3. Chi tiết vá (`v2/tools/check-wake-paths.py`)

### 3.1 H1/H2 — phạm vi

```python
TEST_ONLY_WAKE_SCOPE = (
    "v2/kernel/vifast-core/src/",
    "v2/sim/vifast-sim/src/",
    "v2/kernel/vifast-kbin-riscv64/src/",
)
RE_TEST_ONLY_WAKE_SCOPE_PLAT = re.compile(r"^v2/kernel/vifast-plat-[a-z0-9_]+/src/")
```

Vì sao **mẫu** chứ không liệt kê cứng: crate plat thứ tư (`vifast-plat-aarch64` là crate mới của
KT-160) ra đời sau vẫn tự động nằm trong phạm vi — nếu liệt kê cứng thì mỗi crate mới lại là một
lỗ hổng H2 mới. Ảnh test (`ktest*.rs`) và `tests/` vẫn được miễn qua `is_test_file` — đó là mục
đích duy nhất còn lại của `wake()`.

### 3.2 H3 — alias / fn-pointer / `use`

Luật cũ bắt `wake\s*\(` theo dòng. Nay quét **token** `(?<![A-Za-z0-9_])wake(?![A-Za-z0-9_])`
trên toàn văn bản đã khử nhiễu, rồi phân loại **từng** tham chiếu:

| Ngữ cảnh | Kết quả |
|---|---|
| `fn wake(` (định nghĩa, kể cả `pub fn`/`unsafe fn`) | **miễn** — chỗ khai báo API |
| `Machine::wake`, `Self::wake`, `<Machine>::wake`, `::wake` | **ĐỎ** — tham chiếu hàm làm giá trị |
| `use …::{wake, wake_from};` | **ĐỎ** — nhập hàm vào phạm vi |
| `wake` + khoảng trắng/xuống dòng + `(` | **ĐỎ** — lời gọi (phủ H4/H5) |
| `wake` làm giá trị khác | **ĐỎ** |
| `let mut wake = …` (biến cục bộ) | **miễn** — không phải hàm |

Biến cục bộ tên `wake` **có thật** trong cây (`core_tcb.rs:721` `let mut wake = [0u32; MAX_TCBS];`,
`kobj.rs:8095` `let mut wake: Vec<ObjId> = Vec::new();`) nên phải miễn, nếu không cổng đỏ oan. Cách
miễn **hẹp**: chỉ miễn khi **hàm bao quanh** có khai báo `let (mut) wake` / `for wake in` /
tham số `wake:`; `::wake` **luôn** ĐỎ bất kể biến cục bộ (nên `let wake = 0; let f = Machine::wake;`
vẫn bị bắt).

### 3.3 H4 — block comment chen giữa

`strip_line_comment` (chỉ cắt `//`) thay bằng `strip_rust_noise`: thay **toàn bộ** chú thích
(`//`, `///`, `//!`, `/* … */` — Rust cho **lồng**) và literal chuỗi/ký tự (`"…"`, `r#"…"#`,
`br#"…"#`, `'x'`, `'\n'`) bằng dấu cách, **giữ nguyên `\n`** ⇒ số dòng + số cột không đổi.
`'a` (lifetime) **không** bị coi là literal ký tự (phân biệt bằng độ dài: một ký tự rồi `'`).
Nhờ vậy `m.wake /* L6 */ (t)` thành `m.wake          (t)` — khớp luật lời gọi.

### 3.4 H5 — gọi cắt dòng

Checker cũ `enumerate(text.splitlines())` nên không bao giờ thấy `wake` và `(` ở hai dòng khác
nhau. Nay `RE_WAKE_TOKEN.finditer(code)` chạy trên **toàn văn bản**; `after.lstrip()[:1] == "("`
bỏ **mọi** khoảng trắng kể cả `\n`. Số dòng tính bằng `bisect.bisect_right(line_starts, offset)`
nên báo cáo vẫn đúng `file:dòng`.

### 3.5 H6 — `test_line_ranges` nuốt `mod` sản phẩm

Bản cũ: thấy `#[cfg(test)]` rồi tìm `mod … {` trong **4 dòng kế** ⇒ `#[cfg(test)]` đặt trên một
`use` **nuốt luôn** `mod` sản phẩm đứng sau. Nay:

1. bỏ qua dòng thuộc tính kế tiếp (`#[…]`) và dòng trống (chú thích đã thành dòng trắng);
2. dòng kế **phải** là mục **có thân khối** — `mod`/`fn`/`impl`/`trait`/`struct`/`enum`/`union`
   (kể cả `pub(crate)`, `const`, `unsafe`, `async`, `extern "C"`);
3. `#[cfg(test)]` trên `use`/`const`/`static`/`type` ⇒ **không** mở vùng test;
4. `mod tests;` (không thân) ⇒ không mở vùng (tệp ngoài do `is_test_file` miễn).

Đếm ngoặc dùng văn bản **đã khử nhiễu** nên `{`/`}` trong chuỗi không làm lệch độ sâu.

### 3.6 Không báo oan (đối chứng dương)

Cây sạch vẫn `errors=0` sau khi siết. Ba lớp dễ báo oan đã kiểm:
`/// wake(tcb)` / `//! …` (chú thích), `"wake(tcb)"` (chuỗi), `let mut wake = …` (biến cục bộ),
`wake_from`/`wake_from_unlink`/`wake_step`/`wake_addr` (token khác — `_` sau `wake`),
`MailboxMsg::Wake` (hoa).

## 4. Bằng chứng (log nguyên văn)

Thư mục: `v2/agent/JOURNAL/kt165-logs/`.

### 4.1 Cây sạch — `--strict` `errors=0`

```
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
```
(`clean-strict.log`; chạy lại sau mỗi lần gỡ đột biến: `m1-revert-clean.log`, `m3-revert-clean.log`)

### 4.2 M1 — `m.wake(t)` trong **ảnh sản phẩm** `proc_rt.rs` ⇒ ĐỎ

Đột biến: `let _ = m.wake(rig.tcb_obj[T_KERN]);` đầu `fn service` (`proc_rt.rs:215`).

```
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:215: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
```
(`m1-proc-rt.log`)

**Đột biến là mã thật, biên dịch được** (không phải mã chết):

```
$ cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
      --target riscv64gc-unknown-none-elf
    Compiling vifast-kbin-riscv64 v0.1.0 (/private/tmp/night2-L8-wakefix/v2/kernel/vifast-kbin-riscv64)
warning: vifast-kbin-riscv64@0.1.0: KT-89 build identity algo=kt134-v4 inputs=efe45ec08a656d39 dirty=1 rev=e022cdbed90e
    Checking vifast-plat-riscv64 v0.1.0 (/private/tmp/night2-L8-wakefix/v2/kernel/vifast-plat-riscv64)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.55s
```
(`m1-cargo-check.log`, exit 0)

Gỡ ⇒ `# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0` (`m1-revert-clean.log`).

### 4.3 M2 — alias `let f = Machine::wake;` ⇒ ĐỎ

Đột biến trong `core_sched.rs::wake` (`:247`): `let f = Machine::wake; let _ = f;`

```
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_sched.rs:247: tham chiếu `…::wake` (alias/fn-pointer), không có `(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
```
(`m2-alias.log`)

Biên dịch được: `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core` ⇒
`Finished dev profile … in 1.66s` (`m2-cargo-check.log`, exit 0). Gỡ ⇒ `errors=0`.

### 4.4 M3 — **cả sáu** cách lách cùng lúc trên cây ⇒ **6** dòng `E_STRICT_TEST_ONLY_WAKE`

Đột biến áp vào 4 tệp thật (H1 `proc_rt.rs`, H2 `vifast-plat-riscv64/src/lib.rs`, H3
`core_sched.rs`, H4+H5+H6 `core_ipc.rs`), rồi chạy `--strict`:

```
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_ipc.rs:1448: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_ipc.rs:1451: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_ipc.rs:1461: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_sched.rs:247: tham chiếu `…::wake` (alias/fn-pointer), không có `(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:215: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-plat-riscv64/src/lib.rs:904: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=6 warnings=0
```
(`m3-sau-cach-lach.log` — 6 dòng, đúng một dòng cho mỗi H1..H6)

**Đối chứng âm quan trọng nhất**: chạy **checker cũ** (`git show e022cdbe:v2/tools/check-wake-paths.py`)
trên **cùng cây đột biến đó**:

```
$ python3 /tmp/l8-old-checker.py --root . --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
```
(`m3-checker-cu-xanh.log`) ⇒ sáu lỗ hổng là **thật**, không phải suy đoán: cùng một cây, checker
cũ XANH, checker mới ĐỎ 6.

Gỡ cả 4 tệp ⇒ `errors=0` (`m3-revert-clean.log`).

### 4.5 `--selftest` — 26/26 (15 ca cũ giữ nguyên + 11 ca mới)

```
$ python3 v2/tools/check-wake-paths.py --selftest
ok - v2 sạch ⇒ xanh
…
ok - H1 `wake(` trong `vifast-kbin-riscv64/src/` ⇒ đỏ (KT-165)
ok - H2 `wake(` trong `vifast-plat-*/src/` ⇒ đỏ (KT-165)
ok - H2b `wake(` trong crate plat **mới** ⇒ đỏ (mẫu, không liệt kê cứng)
ok - H3 alias `let g = Machine::wake;` ⇒ đỏ (không cần `(`)
ok - H3b `use …::{wake, …}` ⇒ đỏ (nhập hàm vào phạm vi)
ok - H4 `wake /*x*/ (t)` ⇒ đỏ (block comment chen giữa)
ok - H5 `wake` ⏎ `(t)` ⇒ đỏ (gọi cắt dòng)
ok - H6 `#[cfg(test)]` trên `use` không được miễn `mod` sản phẩm ⇒ đỏ
ok - phạm vi mới: chỉ `wake_from(`/`wake_step(`/`wake_addr(` ⇒ xanh
ok - `#[cfg(test)] mod tests` thật vẫn được miễn ⇒ xanh
ok - biến cục bộ `let mut wake = …` ⇒ xanh (không báo oan)
ok - v1 ba kênh ⇒ đỏ
ok - v1 một kênh ⇒ xanh
ok - cây rỗng ⇒ đỏ (fail-closed)
# selftest: 26/26 ok
```
(`selftest.log`)

**Selftest có răng** — gỡ từng vá trên **bản sao** checker ở `/tmp` (không sửa tệp trong repo):

```
--- A: phạm vi cũ KT-159 (chỉ core + sim): # selftest: 23/26 ok
    not ok - H1 `wake(` trong `vifast-kbin-riscv64/src/` ⇒ đỏ (KT-165)
    not ok - H2 `wake(` trong `vifast-plat-*/src/` ⇒ đỏ (KT-165)
    not ok - H2b `wake(` trong crate plat **mới** ⇒ đỏ (mẫu, không liệt kê cứng)
--- B: regex cũ theo dòng `wake\s*\(`: # selftest: 24/26 ok
    not ok - H3 alias `let g = Machine::wake;` ⇒ đỏ (không cần `(`)
    not ok - H3b `use …::{wake, …}` ⇒ đỏ (nhập hàm vào phạm vi)
--- C: test_line_ranges cũ (quét 4 dòng kế): # selftest: 25/26 ok
    not ok - H6 `#[cfg(test)]` trên `use` không được miễn `mod` sản phẩm ⇒ đỏ
--- D: khử nhiễu thành no-op: # selftest: 25/26 ok
    not ok - v2 `--strict` `wake(` trong chú thích ⇒ xanh (không báo oan)
```
(`selftest-teeth.log`) ⇒ mỗi vá có **ít nhất một** ca selftest chết khi gỡ vá; ca không phải
trang trí.

### 4.6 `./v2/bin/v2 check` — PASS

```
$ ./v2/bin/v2 check
--- wake paths (KT-151: một cơ chế wake): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155: allowlist): OK
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
…
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
```
(`v2-check.log`, `RC=0`)

Chạy lại **sau khi commit** trên cây đã commit (chỉ thêm journal + log, không đổi mã):
`./v2/bin/v2 check` ⇒ `v2 check: PASS (L1 chạy trong gate --fast)`, `RC=0` — cùng nội dung,
khác `HEAD` trong dòng tiêu đề (bản chạy sau commit ghi hash của commit lúc đó) và số ms.
Log giữ bản chạy **trước** commit (`HEAD e022cdbe`) vì đó là lần chạy đúng trạng thái mã được
commit.

## 5. Giới hạn / còn thiếu

- **Không** chạy `gate --fast` (luật task) — chỉ `v2 check` (đúng yêu cầu).
- **Không** chạy `cargo test` cho `vifast-core`/`vifast-sim`: KT-165 chỉ sửa **checker Python**,
  không chạm mã Rust. `cargo check` đã chạy cho hai đột biến (M1 riscv64, M2 host) để chứng minh
  chúng là mã thật.
- **Không** quét `v2/kernel/vifast-kbin-riscv64/init/` (chương trình U-mode: `shell_user.rs`, …).
  Đó là crate **khác** (`init/` có `init.ld`, `init_user.S`), không nằm trong phạm vi task giao
  (`vifast-kbin-riscv64/src/`). Nếu muốn phủ, cần một task riêng — **ghi ở đây để không im lặng**.
  Lý do chưa làm: `init/` không link `vifast-core` theo cùng cách, và mở rộng phạm vi ngoài yêu
  cầu có thể tạo đỏ oan chưa kiểm.
- **Không** kiểm được "mọi cách lách tương lai": luật vẫn là phân tích **văn bản**, không phải
  trình biên dịch. Cụ thể còn hở về lý thuyết: (a) macro sinh ra `wake` (`macro_rules!`/`quote!`)
  — token nằm trong chuỗi macro, không thấy; (b) `include!` một tệp ngoài `v2/kernel`+`v2/sim`;
  (c) tên hàm qua `stringify!`/`concat_idents!`. Cả ba đều **không có** trong cây hiện tại và đều
  là mã kỳ dị; ghi lại thay vì tuyên bố "tuyệt đối".
- **Không** sửa `v2/bin/v2` (nhãn bước cổng vẫn ghi "KT-155: allowlist") — chỉ là nhãn, không
  phải phép kiểm; sửa nhãn không thêm giá trị đo.

## 6. Luật đã giữ

- Không merge, không push, không `gate --fast`, không tạo worktree mới, không `pkill -f
  qemu-system` (không mở QEMU nào).
- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`.
- Chỉ **siết** checker: 6 lỗ hổng đóng, 0 ngưỡng nới, 0 ca bị bỏ, 15 ca selftest cũ giữ nguyên.
- Mọi đột biến đã `git checkout --`; cây sạch (`git status` chỉ có tệp của task) trước khi commit.
