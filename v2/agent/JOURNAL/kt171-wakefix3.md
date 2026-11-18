# KT-171 — vá vòng 3 luật `E_STRICT_TEST_ONLY_WAKE` (reviewer L11: KT-165 KHÔNG ĐẠT)

**Ngày**: 2026-11-19 (vòng hai) · **Agent 2 (thợ) L14** · nhánh `night2/L14-wakefix3` ·
worktree `/private/tmp/night2-L14-wakefix3` · HEAD `be0010a9` (từ `main`).
**Nguồn đọc trước**: [`kt168-review2.md`](kt168-review2.md) §1 (KT-165), `v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 07:30 JST. Không merge, không push, không `gate --fast`, không tạo worktree mới.

## 0. Kết luận trước

| Lớp (reviewer L11, KT-168 §1.2) | Trạng thái | Căn cứ (một dòng) |
|---|---|---|
| **B8** — miễn theo **tên** `ktest*` | **ĐÃ ĐÓNG** | `test_only_modules` (đồ thị module); đột biến thật `ktest_evil.rs` + `mod` từ `main.rs` ⇒ `E_STRICT_TEST_ONLY_WAKE` @ `ktest_evil.rs:6` |
| **B9** — miễn theo **đường dẫn** `src/tests/` | **ĐÃ ĐÓNG** | `mod tests;` **không** `#[cfg(test)]` ⇒ sản phẩm; đột biến thật ⇒ ĐỎ @ `tests/evil.rs:6` |
| **B6** — `include!` trỏ ra ngoài | **ĐÃ ĐÓNG** | phân giải + quét đích; đích ngoài `v2/kernel`+`v2/sim` ⇒ `E_STRICT_WAKE_INCLUDE` @ `core_sched.rs:1006` |
| **B7** — `#[path]` trỏ ra ngoài | **ĐÃ ĐÓNG** | như B6 (cùng mã phân giải) |
| **B10** — miễn local-binding **cả hàm** | **ĐÃ ĐÓNG** (hai lớp) | miễn theo **khối** + token `wake` làm **đối số macro** không miễn ⇒ ĐỎ @ `core_sched.rs:1016` |
| **B4** — macro ở crate khác | **ĐÃ ĐÓNG** | quét thân `macro_rules!` trong `v2/lib/**` ⇒ `E_STRICT_MACRO_WAKE` @ `vifast-abi/src/lib.rs:129` |

Cây sạch: `--strict` **`errors=0`**; `--selftest` **39/39** (26 ca cũ + 13 ca mới);
`./v2/bin/v2 check` **PASS**. Không lớp nào phải tuyên bố "chưa đóng".

## 1. Đã sửa gì (theo thứ tự ưu tiên của đề)

### 1.1 B8/B9 — "test-only" theo **đồ thị module**, không theo tên/đường dẫn

`is_test_file` cũ (`check-wake-paths.py:357-366`) miễn `name.startswith("ktest")`,
`"/tests/" in relp`, `*_tests.rs`. Nay:

- `is_test_file_heuristic` giữ **nguyên** heuristic đó nhưng **chỉ** dùng cho các mục 1–5
  (kênh/arch/transition) — xem §4 "còn hở".
- `is_test_file` (mới) = `relpath in test_only_modules(root)`.
- `test_only_modules` dựng **đồ thị module** từ crate root:
  - root: `src/lib.rs`/`src/main.rs` (sản phẩm), `[[bin]]` có `required-features` chứa
    `ktest` (test), `tests/*.rs` cấp crate (test);
  - cạnh: `mod NAME;` (kèm `#[path]`), đệ quy;
  - một tệp **test-only** chỉ khi **mọi** đường tới nó từ ngữ cảnh test: (a) `tests/` cấp
    crate, (b) `mod` trong khối `#[cfg(test)]` hoặc có `#[cfg(test)]` trực tiếp, (c)
    `#[cfg(feature = "ktest")]`, (d) bin `required-features = ["ktest"]`;
  - tệp **không** tới được từ crate root nào ⇒ **sản phẩm** (fail-closed);
  - thư mục module con theo **đúng** luật Rust: crate root (`lib.rs`/`main.rs`/`[[bin]] path`)
    ⇒ thư mục của chính nó; tệp nạp bằng `#[path]` ⇒ thư mục của tệp đích; module thường
    `src/foo.rs` ⇒ `src/foo/`. (Nếu thiếu bước này, `ktest.rs` — bin root — bị coi là module
    `src/ktest/` và 12 module con của nó rơi khỏi đồ thị; đã sửa.)
- Hệ quả: `ktest_evil.rs` `mod` từ `main.rs` là **sản phẩm**; `src/tests/evil.rs` với
  `mod tests;` không `#[cfg(test)]` là **sản phẩm**. Tên/thư mục **không** đủ.

### 1.2 B6/B7 — phân giải `include!`/`#[path]`

`resolve_include_targets` (mới) chạy trong `--strict`:

- tìm `include!("…")` + `#[path = "…"]` trên văn bản **đã khử chú thích nhưng giữ literal**
  (`strip_rust_noise(text, keep_strings=True)`), bỏ match nằm trong literal chuỗi;
- phân giải tương đối theo thư mục tệp chứa;
- đích **ngoài** `v2/kernel`+`v2/sim` ⇒ `E_STRICT_WAKE_INCLUDE` (fail-closed);
- đích **trong** cây ⇒ **quét chính tệp đích** (kế thừa trạng thái test-only của tệp kéo nó
  vào; sản phẩm thắng nếu cả hai), đệ quy.

### 1.3 B10 — miễn local-binding theo **khối** + chặn **đối số macro**

- `_local_wake_scopes` + `_exempt_local_wake`: `let (mut) wake`/`for wake in` chỉ miễn token
  `wake` nằm trong **cùng khối** (hoặc khối con) và **sau** binding — không miễn cả hàm.
- **Phát hiện trung thực**: chỉ sửa scope **không đủ** cho đúng cách L11 đã làm. Log
  `kt168-logs/b10-cargo-check.log` cho thấy `let mut wake = 0u32;` @ `core_sched.rs:1007` và
  lời gọi `_b10!(m, wake, t)` @ `:1009` **cùng khối**; thứ biến ident `wake` thành **tên hàm**
  là thân macro `$m.$name($t)`. Vì vậy thêm lớp thứ hai: token `wake` nằm trong **danh sách
  đối số của một lời gọi macro** **không** được miễn (trừ `SAFE_VALUE_MACROS` chỉ-đọc-giá-trị:
  `assert!`, `matches!`, `format!`, …). Đột biến thật B10 ⇒ ĐỎ đúng nhờ lớp này.

### 1.4 B4 — quét thân `macro_rules!` trong `v2/lib/**`

`scan_lib_macros` (mới, trong `--strict`): mọi `macro_rules! NAME { … }` dưới `v2/lib/**`,
thân đã khử nhiễu, tìm `RE_WAKE_TOKEN` (không phải `wake_from`/`wake_step`/`wake_addr`) ⇒
`E_STRICT_MACRO_WAKE`, trừ `MACRO_WAKE_ALLOW[(relpath, name)]` có lý do (hiện **rỗng** — không
macro nào trong `v2/lib` chạm `wake`).

## 2. Bằng chứng (dán nguyên văn)

### 2.1 Cây sạch

```
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
```

```
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 39/39 ok
```

```
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

### 2.2 Từng cách lách — đột biến **thật** trên cây, `cargo check` chứng minh biên dịch được

Mỗi đột biến: dựng lại đúng cách L11 (KT-168 §1.2), `cargo check` **PASS**, `--strict` **ĐỎ**,
rồi `git checkout --` + xoá tệp mới. Log đầy đủ ở `/tmp/kt171-mut/logs/` (không commit — log
tạm; bằng chứng nguyên văn dưới đây).

| # | Đột biến | `cargo check` | `--strict` (KT-171) | `--strict` (bản KT-165, đối chứng) |
|---|---|---|---|---|
| **B8** | `src/ktest_evil.rs` + `mod ktest_evil;` @ `main.rs:41` | **RC=0** (`Finished dev profile … in 4.22s`) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` @ `ktest_evil.rs:6` | `errors=0` |
| **B9** | `src/tests.rs` + `src/tests/evil.rs` + `mod tests;` @ `lib.rs:78` | **RC=0** (`… in 1.70s`) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` @ `tests/evil.rs:6` | `errors=0` |
| **B6** | `include!("../../../lib/vifast-abi/src/evil_include.rs")` @ `core_sched.rs:1006` | **RC=0** (`… in 1.09s`) | **ĐỎ** `E_STRICT_WAKE_INCLUDE` @ `core_sched.rs:1006` | `errors=0` |
| **B7** | `#[path = "../../../lib/vifast-abi/src/evil_path.rs"] mod _b7;` @ `core_sched.rs:1006` | **RC=0** (`… in 0.55s`) | **ĐỎ** `E_STRICT_WAKE_INCLUDE` @ `core_sched.rs:1006` | `errors=0` |
| **B10** | `let mut wake` + `_b10!(self, wake, t)` @ `core_sched.rs:1016` | **RC=0** (`… in 0.86s`, 2 warning) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` @ `core_sched.rs:1016` | `errors=0` |
| **B4** | `#[macro_export] macro_rules! wake_call` @ `vifast-abi/src/lib.rs:129` + gọi từ `core_sched.rs` | **RC=0** (`… in 0.95s`) | **ĐỎ** `E_STRICT_MACRO_WAKE` @ `vifast-abi/src/lib.rs:129` | `errors=0` |

Nguyên văn dòng `--strict` của từng ca:

```
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs:6: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/tests/evil.rs:6: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1006: `include!` trỏ ra ngoài `v2/kernel`+`v2/sim` (`../../../lib/vifast-abi/src/evil_include.rs`) — không quét được đích (fail-closed, KT-171 B6/B7)
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1006: `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` (`../../../lib/vifast-abi/src/evil_path.rs`) — không quét được đích (fail-closed, KT-171 B6/B7)
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/core_sched.rs:1016: `wake` là đối số của macro `_b10!` — macro có thể biến ident thành tên hàm (KT-171 B10) trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
E E_STRICT_MACRO_WAKE   v2/lib/vifast-abi/src/lib.rs:129: thân `macro_rules! wake_call` chứa token `wake` — macro có thể sinh lời gọi `wake()` trong mã sản phẩm (KT-171 B4); dùng `wake_from`/`wake_step` hoặc thêm allowlist có lý do
```

Đối chứng: **cùng** sáu đột biến, chạy bản checker **KT-165** (`git show HEAD:v2/tools/check-wake-paths.py`)
⇒ **cả sáu** `errors=0` — tái lập đúng phán quyết "KHÔNG ĐẠT" của L11, và chứng minh bản vá
KT-171 đóng đúng lớp.

### 2.3 Selftest mới (13 ca, tổng 39)

```
ok - B8 module sản phẩm tên `ktest_evil.rs` + `mod` từ `main.rs` ⇒ đỏ (KT-171)
ok - B8b `#[cfg(feature = "ktest")] mod ktest_ok;` ⇒ xanh (test-only thật)
ok - B9 `src/tests/evil.rs` + `mod tests;` không `#[cfg(test)]` ⇒ đỏ (KT-171)
ok - B9b `#[cfg(test)] mod tests;` ⇒ xanh (test-only thật)
ok - B6 `include!` trỏ ra ngoài `v2/kernel`+`v2/sim` ⇒ đỏ (KT-171)
ok - B6b `include!` trỏ trong `v2/kernel` (ngoài `src/`) ⇒ quét đích ⇒ đỏ
ok - B7 `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` ⇒ đỏ (KT-171)
ok - B7b `#[path]` trỏ trong cây, đích chỉ `wake_from(` ⇒ xanh
ok - B10 `wake` làm đối số macro `$name:ident` ⇒ đỏ (KT-171)
ok - B10b binding ở khối con, token ở khối cha ⇒ đỏ (scope, KT-171)
ok - B10c binding + dùng cùng khối ⇒ xanh (không báo oan)
ok - B4 `macro_rules!` trong `v2/lib` chứa token `wake` ⇒ đỏ (KT-171)
ok - B4b `macro_rules!` trong `v2/lib` chỉ `wake_from(` ⇒ xanh
# selftest: 39/39 ok
```

26 ca cũ (KT-151/155/159/165) vẫn xanh — không ca nào bị sửa kỳ vọng.

## 3. Vì sao bản vá đúng hướng (không phải "đỏ mọi thứ")

- **B8b/B9b/B7b/B10c/B4b** là đối chứng dương: test-only **thật** (`#[cfg(feature = "ktest")]`,
  `#[cfg(test)] mod tests;`), `#[path]` trỏ trong cây, biến cục bộ dùng đúng khối, macro chỉ
  `wake_from` — đều **XANH**.
- Cây thật: **97** tệp test-only được nhận đúng — 25 `ktest*.rs` (bin
  `required-features = ["ktest"]` + module con, kể cả nạp bằng `#[path]`), 67 `tests/*.rs`
  integration test cấp crate, 2 `*_tests.rs` của core (`#[cfg(test)]`),
  `vifast-plat-riscv64/src/probe.rs` (`#[cfg(feature = "ktest")]`), `init/pager_common.rs`
  (qua `#[path]` từ `ktest_vfs*.rs`), `proc_demo.rs` (chỉ `mod` từ `ktest_proc.rs`). Không tệp
  sản phẩm nào bị miễn oan.
- `--strict` chậm hơn: **4,6 s → 6,0 s** (đồ thị module + phân giải `#[path]`), chấp nhận được
  trong `v2 check`.

## 4. Còn hở / ngoài phạm vi (ghi rõ, **không** tuyên bố đã đóng)

1. **`is_test_file_heuristic` vẫn dùng cho mục 1–5** (`check-wake-paths.py`: kênh `MailboxMsg`,
   consumer, arch, `E_STRICT_TRANSITION`). Nghĩa là một module **sản phẩm** tên `ktest_evil.rs`
   vẫn có thể lọt **luật KT-155** (`transition_tcb(…, Ready)` ngoài cửa wake) và luật kênh.
   **Ngoài phạm vi KT-171** (đề chỉ giao `E_STRICT_TEST_ONLY_WAKE`). Muốn đóng: đổi bốn call
   site sang `is_test_file` (đồ thị module) — nhưng khi đó `proc_demo.rs` (chỉ `mod` từ
   `ktest_proc.rs`, bin `required-features = ["ktest","user"]`) thành test-only, hai mục
   `NON_WAKE_READY_SITES` của nó thành "mục" ⇒ phải xử lý `E_STRICT_STALE` (bỏ mục hoặc miễn
   trừ theo test-only). **Chưa làm** — cần quyết định của Agent 0 vì nó đổi ngữ nghĩa luật
   KT-155.
2. **`include!`/`#[path]` chỉ phân giải trong `--strict`** (đúng phạm vi luật `wake`). Các mục
   1–5 không phân giải — cùng lý do (1).
3. **`#[path]` trong khối `mod` inline** (`mod x { #[path="…"] mod y; }`) phân giải theo thư mục
   tệp chứa, không theo thư mục module — Rust định nghĩa khác cho trường hợp này. Cây hiện tại
   **không** có dạng đó; nếu xuất hiện, đích vẫn bị kiểm "trong/ngoài `v2/kernel`+`v2/sim`"
   (fail-closed) nhưng có thể lệch thư mục. Ghi nhận, không phải lỗ hổng đã biết.
4. **`macro_rules!` ngoài `v2/lib`** (ví dụ định nghĩa trong `v2/kernel/.../init/`) không được
   quét thân — nhưng `init/` không phải mã sản phẩm kernel (biên dịch bằng `rustc` trực tiếp,
   không `--extern vifast_core`). Macro trong `v2/kernel/*/src/` + `v2/sim/*/src/` đã bị quét
   toàn văn bản bởi mục 6 (B3 của KT-165).
5. **Không** chạy `gate --fast` (luật task). `./v2/bin/v2 check` PASS.

## 5. Luật đã giữ

- Không merge, không push, không sửa `main`, không `gate --fast`, không tạo worktree mới.
- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`.
- Không nới ngưỡng/checker (chỉ **siết**: thêm `E_STRICT_WAKE_INCLUDE`, `E_STRICT_MACRO_WAKE`,
  thu hẹp miễn local-binding); không `#[ignore]`/`#[should_panic]`; không retry để lấy xanh.
- Không `pkill -f qemu-system`; task **không** dùng QEMU.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không
  `local a=1 b="$a"`.
- Mọi đột biến đã `git checkout --` + xoá tệp mới; cây sạch trước commit (chỉ
  `v2/tools/check-wake-paths.py` + journal này).
