# Hợp đồng của `check-wake-paths.py` — KT-184 / KT-194 / KT-196

**Trạng thái**: bản cập nhật theo **A0 2026-11-19 §5** (tách bốn loại). **Chưa** được reviewer độc
lập xác nhận trên HEAD cuối — KT-184 và KT-194 vẫn `REVIEW`.

Máy kiểm này **không** phải bộ phân tích Rust tổng quát. Nó là **lexer + token-tree + bộ đọc cấu
trúc tối thiểu**, chạy **không cần biên dịch**, và **fail-closed** ở mọi chỗ nó không phân giải được.

---

## 1. Phạm vi cây nguồn được bảo vệ

| Cây | Luật áp |
|---|---|
| `v2/kernel/**` | `include!`/`#[path]`/`cfg_attr(path)` literal + trong cây; `wake` chỉ-dùng-test; thân `macro_rules!` (**token `wake`** + `include!`/`#[path]`/`mod`/`:tt`) |
| `v2/sim/**` | như trên |
| `v2/lib/**` | thân `macro_rules!` (token `wake`, `include!`, `#[path]`, `mod`, `:tt`) |
| `v2/test/**`, `v2/tools/**`, `v2/agent/**` | **không** bảo vệ (ngoài phạm vi) |

**KT-181** (D2-164): thân `macro_rules!` được quét ở **cả ba** gốc `v2/lib`+`v2/kernel`+`v2/sim`
(`MACRO_SCAN_ROOTS`), **kể cả** `v2/kernel/vifast-kbin-riscv64/init/` (chương trình U-mode đóng
vào ảnh), `v2/kernel/*/tests/`, `v2/kernel/vifast-plat-user/` (shim ABI U-mode). Trước KT-181
luật token `wake` chỉ chạy ở `v2/lib`; `v2/kernel`+`v2/sim` chỉ có luật `include!`/`#[path]`.
`v2/tools/**` (công cụ phát triển, **không** link vào ảnh sản phẩm) và `v2/test/fixtures/**`
(fixture **cố tình sai**) nằm ngoài — lý do ở §4.

Đích của `include!`/`#[path]` phải nằm trong `v2/kernel` hoặc `v2/sim`; trỏ ra ngoài ⇒ **ĐỎ**.

---

## 2. **Đã hỗ trợ và được kiểm**

- `include!("x.rs")`, `include!["x.rs"]`, `include!{"x.rs"}`, `include ! ( "x.rs" )`.
- Chuỗi thường `"x.rs"`, chuỗi thô `r"x.rs"`, `r#"x.rs"#`, `r##"x.rs"##`; **escape** `\x2e`,
  `\u{2e}`, `\n`, `\t`, `\\`, `\"` — giải mã trước khi so.
- `#[path = "x.rs"]`, `#[ path = "x.rs" ]`, `#[\npath = "x.rs"]`, **`#[r#path = "x.rs"]`** (raw ident).
- `#[cfg_attr(not(test), path = "x.rs")]` và **`cfg_attr` lồng** (giới hạn `MAX_ATTR_DEPTH`).
- `mod NAME;` khai báo thường (đồ thị module), kể cả `#[cfg(test)]`/`#[cfg(feature = "ktest")]`.
- **Ngữ cảnh test theo item/token-tree** (KT-196/F2 + KT-198/F2b): áp cho **cả** đồ thị module
  (`_mods_of`) **và** luật `wake` (`_test_char_ranges`). `#[cfg(test)]` chỉ áp cho **mục kế tiếp**,
  không lan sang mục cùng dòng; `mod`/`fn` trong khối test lồng vẫn là test.
- `macro_rules!` với **mọi** tên (kể cả Unicode) và **cả ba** dấu thân `{ … }`, `( … )`, `[ … ]`.
- **KT-181**: thân `macro_rules!` ở **mọi** tệp `v2/lib`+`v2/kernel`+`v2/sim` được kiểm **cả
  hai** luật — token `wake` (`E_STRICT_MACRO_WAKE`, trừ `MACRO_WAKE_ALLOW` có lý do) **và**
  `include!`/`#[path]`/`mod`/`:tt` (`_check_macro_body`). Một hàm `scan_macro_bodies`, một phạm
  vi; không còn khoảng trống giữa `scan_lib_macros` (cũ) và `scan_kernel_macro_includes` (cũ).
- Chú thích chen giữa `#`/`[`/`path` (khử chú thích trước khi khớp).
- `#[path = "sub"] mod m { … }` — inline module: `sub` là **thư mục**, không phải cạnh tệp (KT-196/F4);
  module con của nó được duyệt trong **thư mục `#[path]`**, không phải `cur_dir/NAME` (KT-199/F4b).

---

## 3. **Bị từ chối có chủ đích** (fail-closed)

| Dạng | Mã lỗi | Lý do |
|---|---|---|
| `include!`/`#[path]`/`cfg_attr(path)` **không literal** (`concat!`, `env!`, biến) | `E_STRICT_WAKE_INCLUDE` | không phân giải được đích |
| Đích trỏ **ra ngoài** `v2/kernel`+`v2/sim` | `E_STRICT_WAKE_INCLUDE` | không quét được |
| Đích **không tồn tại** (và không phải thư mục) | `E_STRICT_WAKE_INCLUDE` | fail-closed |
| `#[path]` trỏ **thư mục** trên `mod` **không có thân** | `E_STRICT_WAKE_INCLUDE` | sai cú pháp, không phân giải được (KT-199/F4c) |
| Thân `macro_rules!` chứa `include!`/`#[path]` (bất kỳ dạng) | `E_STRICT_MACRO_INCLUDE` | rustc giải theo **tệp gọi** |
| Thân `macro_rules!` chứa `$…:tt` (passthrough) | `E_STRICT_MACRO_TT` | có thể chở `mod`/`include!` từ **lời gọi** |
| Thân `macro_rules!` khai `mod NAME;` (định danh literal) | `E_STRICT_MACRO_MOD` | đồ thị module không thấy |
| Thân `macro_rules!` khai `mod $n;` / `mod r#x;` | `E_STRICT_MACRO_INCLUDE` | không phân giải được tên module |
| Thân `macro_rules!` chứa token `wake` (mọi gốc `v2/lib`+`v2/kernel`+`v2/sim`, **KT-181**) | `E_STRICT_MACRO_WAKE` | macro có thể sinh `wake()` ở **tệp gọi**; trừ `MACRO_WAKE_ALLOW` có lý do |
| `wake(` trong mã sản phẩm **thuộc `TEST_ONLY_WAKE_SCOPE`** | `E_STRICT_TEST_ONLY_WAKE` | D2-144 |
| Chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá độ sâu | `E_LEX_*` | D2-166 §3C |

---

## 4. **Sai lệch implementation còn mở** (đã biết, chưa đóng)

| Mã | Mô tả | Ảnh hưởng |
|---|---|---|
| **F2 (một phần)** | Ngữ cảnh test đã theo item/token-tree, nhưng luật `wake` **vẫn chỉ chạy đầy đủ ở `--strict`** (D2-144). Chế độ thường chỉ quét tệp bị kéo vào bằng `#[path]`/`include!`. | `v2 check`/`gate` chạy `--strict` nên cổng vẫn bắt; nhưng gọi checker ở chế độ thường **không** đủ. |
| **Macro invocation** | `_walk_items` **bỏ qua** nhóm sau `!` (lời gọi macro). Nếu lời gọi chứa `mod`/`#[path]` (vd `cfg_if! { … }`), cạnh nguồn **không** được phân giải. | Cây thật **không** có lời gọi macro chứa `mod`/`#[path]` (đã grep). **Không** tuyên bố fail-closed cho phạm vi này. |
| **`#[path]` trong `v2/lib`** | `resolve_include_targets` chỉ lặp trên `v2/kernel`+`v2/sim`; `#[path]` trong `v2/lib` không được phân giải. | Luật `wake` cũng không phủ `v2/lib`; ngoài phạm vi hiện tại. |
| **F3 (v2/lib)** | `cfg_attr` lồng nhiều tầng trong `v2/lib` **ngoài** thân macro ⇒ XANH (chỉ `v2/kernel`+`v2/sim` được quét cấu trúc). | §1 đã ghi `v2/lib` chỉ bảo vệ thân macro; **không** phải lỗi. |
| **Báo oan có trước** (không phải hồi quy — đối chiếu checker `0fa0fe65` giống hệt) | `#[cfg(test)] include!`; `#[cfg(feature = "ktest")] fn` **inline** (chuỗi `"ktest"` bị `strip_rust_noise` xoá); `#[cfg(test)] const/static`; `macro_rules!` thân `()`/`[]`; block statement; match arm pattern literal. | Cây thật **không** có dạng nào (mọi chỗ `ktest` đều không chứa `wake`); ghi để không thành "phát hiện mới". |
| **Macro ngoài `MACRO_SCAN_ROOTS`** (KT-181) | `v2/tools/**` (công cụ phát triển, **không** link vào ảnh sản phẩm) và `v2/test/fixtures/**` (fixture **cố tình sai**) **không** được quét thân macro. | Không phải mã sản phẩm; cây thật **không** có `macro_rules!` ở hai gốc này. Nếu tương lai có mã sản phẩm ở đó, phải thêm gốc vào `MACRO_SCAN_ROOTS` — **không** được coi là đã bảo vệ. |
| **Không có đường lọt thật cho `wake` ngoài phạm vi cũ** (KT-181 — ghi để **không thổi phồng**) | Trước KT-181, macro ở `init/`/`tests/`/`vifast-plat-user/` chứa token `wake` **không** bị luật token bắt. Nhưng **không** dựng được đường lọt: `init/`+`vifast-plat-user/` biên dịch bằng `rustc` trực tiếp **không** `--extern vifast_core`; `tests/` không phải sản phẩm; nạp bằng `#[path]`/`include!` từ kernel thì `extra` đã quét (đột biến M1c: `cargo check` biên dịch được nhưng checker **ĐỎ**). | Đây là **phòng ngừa**, không phải lỗ hổng đã bị khai thác. |

---

## 5. **Ngoài phạm vi bảo đảm**

- **Ngữ nghĩa mở rộng macro** — không mô phỏng `macro_rules!`; chỉ từ chối thân macro có dấu hiệu.
- **Proc-macro** (`paste!`, `concat_idents!`) — không có trong allowlist phụ thuộc. **Không** có điểm
  kiểm soát cụ thể ⇒ **không** tuyên bố fail-closed cho proc-macro.
- **Mã sinh lúc build** (`build.rs`, `OUT_DIR`) — ngoài phạm vi; `include!(concat!(env!("OUT_DIR"), …))`
  **bị từ chối** (fail-closed), không phải báo oan.
- **`#[cfg]` phức tạp** — chỉ hiểu `#[cfg(test)]` và `#[cfg(feature = "ktest")]`.
- **Đường dẫn qua symlink** — phân giải theo `Path::resolve`, không theo symlink của rustc.
- **Code nằm ngoài `v2/kernel`/`v2/sim`** — **không** phân tích cấu trúc, **trừ** ba đường: thân
  `macro_rules!` ở **cả ba** gốc `v2/lib`+`v2/kernel`+`v2/sim` (`scan_macro_bodies`, KT-181),
  **quét từ vựng** `v2/lib` (`scan_lexical`: chuỗi/comment chưa đóng trong `v2/lib` vẫn ĐỎ
  `E_LEX_*`), và tệp **bị kéo vào** bằng `#[path]`/`include!` từ `v2/kernel`/`v2/sim` (`extra`).

---

## 6. Cách chạy

```bash
python3 v2/tools/check-wake-paths.py            # chế độ thường
python3 v2/tools/check-wake-paths.py --strict   # + luật wake đầy đủ + allowlist
python3 v2/tools/check-wake-paths.py --selftest # số ca hiện tại
```

Cả ba chế độ chạy trong `./v2/bin/v2 check` và `gate --fast`.

## 7. Bảo trì

- Mọi dạng mới **phải** thêm vào §2 (hỗ trợ) hoặc §3 (từ chối) **trước** khi sửa mã.
- Mọi ca hồi quy nằm trong `--selftest`; ca mới phải có **đối chứng dương** khi có thể.
- Sai lệch còn mở **phải** ghi ở §4, không được im lặng.
