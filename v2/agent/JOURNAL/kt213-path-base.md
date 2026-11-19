# KT-213 — vá gốc phân giải `#[path]` của tệp non-mod-rs (đường lọt #11)

**Vai**: L47 — tác giả bản vá (chuyển từ reviewer tấn công KT-212).
**Nhánh**: `night3/L47-kt212`, gốc `41e568bc`.
**Worktree**: `/private/tmp/night3-L47-kt212`.
**Nguồn**: đường lọt #11 do chính L47 dựng ở KT-212 (`kt212-attack-path-mod.md` §1).

## 0. Kết quả

| Hạng mục | Trước | Sau |
|---|---|---|
| Repro #11 (thường) | **XANH** (errors=0) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| Repro #11 (`--strict`) | **XANH** (errors=0) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| Cây thật (thường) | XANH | **XANH** |
| Cây thật (`--strict`) | XANH | **XANH** |
| `--selftest` | 157/157 | **165/165** (thêm 8 ca KT-213) |
| Báo oan mới trên cây thật | — | **0** |

## 1. Gốc lỗi (một câu)

`resolve_include_targets()` dùng `p.parent` làm gốc cho **mọi** tệp; rustc dùng **thư mục module**
của tệp cho `#[path]` nằm trong **inline module** — với tệp non-mod-rs `src/foo.rs` đó là
`src/foo/`, không phải `src/`. `_mods_of()` đã có `mod_dir` đúng từ KT-171 nhưng chỉ dùng cho
`test_only_modules`, không cho đường phân giải-và-quét.

## 2. Bản vá

### 2.1 Hai gốc, không một

`_walk_items()` nhận thêm tham số `mod_dir` (mặc định `cur_dir`):

- `cur_dir` — gốc của **mức hiện tại**: `#[path]` ở mức mục giải theo đây. Mức gốc = **thư mục
  tệp** (`p.parent`); trong inline module = thư mục inline module.
- `mod_dir` — **thư mục module của tệp**: dùng khi vào **inline module không có `#[path]`**
  (`mod_dir/NAME`) và khi giải `mod NAME;` không `#[path]` (`mod_dir/NAME.rs`).

Khi vào inline module: `_child_dir = mod_dir/NAME`, và nếu inline module có `#[path = "X"]` thì
`_child_dir = (cur_dir/X).resolve()` — `#[path]` **thay** tiền tố `<stem>/` (đo bằng rustc, xem
`kt212-attack-path-mod.md` §3). Đệ quy tiếp với `cur_dir = mod_dir = _child_dir`.

### 2.2 `module_dirs()` — một định nghĩa `mod_dir`, không chép lần hai

Tách `_module_graph(root)` khỏi `test_only_modules()`; nó trả `(status, moddir)` và được cache
(`_GRAPH_CACHE`). `test_only_modules()` và `module_dirs()` cùng đọc **một** đồ thị ⇒ không có hai
định nghĩa `mod_dir` lệch nhau (đúng chỉ đạo "tái dùng logic `mod_dir` mà `_mods_of` đã tính").

`moddir` được dựng từ `crate_roots()` qua `_mods_of()` — chính là logic `child_dir` đã có:
crate root / `mod.rs` / tệp nạp bằng `#[path]` ⇒ thư mục tệp; tệp non-mod-rs ⇒ `<thư mục>/<stem>/`.

### 2.3 Hàng đợi mang `mod_dir`

`resolve_include_targets()`:

- seed từ `files`/`v2/lib`: `moddir.get(p) or _default_mod_dir(p)`;
- đích của `#[path]`/`include!`: `target.parent` (đo bằng rustc: tệp nạp bằng `#[path]` và tệp
  `include!` đều giải `#[path]` trong inline module theo **thư mục tệp đích**);
- `done: set` theo `(tệp, mod_dir)` — cùng một tệp có thể được nạp bằng hai cạnh khác nhau
  (`mod foo;` ⇒ `src/foo/`, `#[path="foo.rs"] mod bar;` ⇒ `src/`) và `#[path]` bên trong inline
  module giải **khác nhau** theo từng cạnh ⇒ xử lý **mỗi cặp** một lần (vẫn đơn điệu
  `True → False` cho trạng thái test-only ⇒ không vòng lặp vô hạn).

### 2.4 `_default_mod_dir` — tệp ngoài đồ thị

`module_dirs()` phủ **mọi** tệp được biên dịch như module của crate cargo. Tệp ngoài đồ thị là
**crate root** do `build.rs` biên dịch trực tiếp (`v2/kernel/*/init/*.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `build.rs`) hoặc tệp chết — với cả hai, gốc là **thư mục của
chính tệp**. Đây cũng là hành vi cũ trước KT-213 ⇒ **không thêm báo oan**.

> Bản nháp đầu dùng `p.parent/p.stem` cho tệp ngoài đồ thị ⇒ **ĐỎ oan** năm tệp
> `v2/kernel/vifast-kbin-riscv64/init/*.rs` (`#[path = "ring_common.rs"]` giải theo `init/`, không
> phải `init/ring_server_user/`). Đã sửa; ghi lại để reviewer biết đây là bẫy.

### 2.5 `_mods_of` — sửa cùng gốc

`_mods_of()` trước đây truyền `mod_dir` làm `cur_dir` ⇒ `#[path]` **mức mục** trong `src/foo.rs`
giải thành `src/foo/a.rs` thay vì `src/a.rs`. Nay `_walk_items(tree, relp, p.parent, mod_dir=mod_dir)`
và `on_mod` nhận thêm `mod_dir` để giải `mod NAME;` theo **thư mục module** (không theo thư mục
tệp). Hai gốc, hai mục đích — không gộp.

## 3. Ca selftest (thêm 8, tổng 165)

| Ca | Kỳ vọng |
|---|---|
| repro #11 `#[path]` trong inline module tệp non-mod-rs → ngoài `src/` (thường) | ĐỎ |
| repro #11 (--strict) | ĐỎ |
| đối chứng dương: `#[path]` trong inline module tệp non-mod-rs → tệp **thật** trong cây | XANH |
| đối chứng dương: `#[path]` **mức mục** trong tệp non-mod-rs → tệp cạnh tệp | XANH |
| `#[path]` mức mục trong tệp non-mod-rs → ngoài `src/` (không hồi quy) | ĐỎ |
| đối chứng dương: `#[path]` **trên inline module** trong tệp non-mod-rs | XANH |
| `_mods_of`: `mod bar;` trong tệp non-mod-rs ⇒ `src/foo/bar.rs` | đúng cạnh |
| `_mods_of`: `#[path]` mức mục trong tệp non-mod-rs ⇒ `src/bar.rs` | đúng cạnh |

## 4. Đo sau vá

| Lệnh | Kết quả | Log |
|---|---|---|
| `python3 v2/tools/check-wake-paths.py --root .` | XANH | `kt213-logs/after-normal.log` |
| `… --strict` | XANH | `kt213-logs/after-strict.log` |
| `… --selftest` | 165/165 ok | `kt213-logs/after-selftest.log` |
| `cargo check -p vifast-core` trên repro | **RC=0** (mã vẫn biên dịch; checker mới là thứ đỏ) | `kt213-logs/repro-cargo.log` |
| `./v2/bin/v2 check` | xem `kt213-logs/v2-check.log` | — |

## 5. Không hồi quy

- 157 ca cũ **giữ nguyên**, không ca nào bị bỏ/xoá; tổng 165.
- Cây thật `errors=0` cả hai chế độ (log `kt213-logs/after-*.log`).
- Ma trận KT-212 (28 biến thể) chạy lại **sau** vá: mọi dạng `#[path]` biên dịch được đều ĐỎ;
  4/4 đối chứng dương XANH (`kt212-logs/matrix/`).

## 6. Còn mở (khác lớp — nợ có tên)

**Đường lọt #12** — thư mục tên `target` trong `src/` (`src/target/evil.rs`) bị **năm** bộ lọc
`"target" in p.parts` bỏ qua ⇒ `mod target;` biên dịch được mà checker XANH cả hai chế độ. Đề xuất
vá ở `kt212-attack-path-mod.md` §5.4 (kiểm **vị trí** `target` trước `src`, không kiểm sự có mặt).
**Chưa vá trong KT-213** vì khác lớp gốc và lead chỉ đạo ghi thành nợ có tên.

Ngoài ra, hạn chế đã biết **không đổi**: `v2/lib` không có đồ thị module riêng (chỉ một `#[path]`
mức mục ở `v2/lib/vifast-idl/src/lib.rs`, giải đúng bằng `p.parent`); `v2/lib` không có `Machine`
nên không thể có `wake()`.

## 7. Tệp chạm

- `v2/tools/check-wake-paths.py` — `_module_graph`/`module_dirs`/`_default_mod_dir`, `_walk_items`
  (`mod_dir`), `_mods_of` (hai gốc), `resolve_include_targets` (hàng đợi mang `mod_dir`, `done`
  theo cặp), 8 ca selftest.
- `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` — §2 (gốc phân giải đúng như rustc) + §4 (điểm đã
  từng sai).
- `v2/agent/JOURNAL/kt212-attack-path-mod.md` — ma trận tấn công + hai đường lọt.
- `v2/agent/JOURNAL/kt212-logs/`, `v2/agent/JOURNAL/kt213-logs/` — log.
