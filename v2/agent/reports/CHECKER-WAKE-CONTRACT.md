# Hợp đồng của `check-wake-paths.py` — KT-184 / KT-194 / KT-196 / KT-209

**Trạng thái**: bản cập nhật theo **A0 2026-11-19 §5** (tách bốn loại) + **KT-209** (L40 KT-201
§D2 L2/L3). **Chưa** được reviewer độc lập xác nhận trên HEAD cuối — KT-184 và KT-194 vẫn `REVIEW`.

Máy kiểm này **không** phải bộ phân tích Rust tổng quát. Nó là **lexer + token-tree + bộ đọc cấu
trúc tối thiểu**, chạy **không cần biên dịch**, và **fail-closed** ở mọi chỗ nó không phân giải được.

---

## 1. Phạm vi cây nguồn được bảo vệ

| Cây | Luật áp |
|---|---|
| `v2/kernel/<crate>/src/**` + `v2/kernel/<crate>/build.rs` | `include!`/`#[path]`/`cfg_attr(path)` literal + trong cây; `wake` chỉ-dùng-test; macro body |
| `v2/sim/<crate>/src/**` + `v2/sim/<crate>/build.rs` | như trên |
| `v2/lib/**` | thân `macro_rules!` (token `wake`, `include!`, `#[path]`, `mod`, `:tt`); `include!`/`#[path]` **ngoài** macro được phân giải, đích phải ở trong `v2/lib` |
| `v2/test/**`, `v2/tools/**`, `v2/agent/**` | **không** bảo vệ (ngoài phạm vi) |

**KT-209/L2** — phạm vi `wake` là **mẫu**, không liệt kê cứng: **mọi** crate dưới
`v2/kernel/<crate>/` và `v2/sim/<crate>/` (toàn bộ `src/**` cộng `build.rs`) nằm trong phạm vi,
kể cả crate ra đời sau. `tests/` cấp crate nằm ngoài (test-only, `is_test_file` miễn).
**Ngoài mẫu** (đo 2026-11-20: **không** có token `wake`): `v2/kernel/vifast-plat-user/lib.rs` (shim
U-mode, **không** phải crate — biên dịch bởi `vifast-kbin-riscv64/build.rs`) và
`v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode, biên dịch bởi `build.rs`).

Đích của `include!`/`#[path]` trong `v2/kernel`/`v2/sim` phải nằm trong `v2/kernel` hoặc
`v2/sim`; trỏ ra ngoài ⇒ **ĐỎ**. Đích của `include!`/`#[path]` trong `v2/lib` phải nằm trong
`v2/lib`; trỏ sang `v2/kernel`/`v2/sim` hoặc ra ngoài ⇒ **ĐỎ** (KT-209/L3).

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
- Chú thích chen giữa `#`/`[`/`path` (khử chú thích trước khi khớp).
- `#[path = "sub"] mod m { … }` — inline module: `sub` là **thư mục**, không phải cạnh tệp (KT-196/F4);
  module con của nó được duyệt trong **thư mục `#[path]`**, không phải `cur_dir/NAME` (KT-199/F4b).
- **`include!`/`#[path]` trong `v2/lib` ngoài thân macro** (KT-209/L3): worklist seed thêm **mọi**
  tệp `v2/lib/**/*.rs`; đích trong `v2/lib` được phân giải **tiếp** (lồng bao nhiêu lớp cũng hết).
  Đích trong `v2/lib` **không** vào tập bị luật `wake` quét (`extra`) — mã lib không có `Machine`
  nên `wake()` không thể xuất hiện; cạnh sang kernel/sim đã bị chặn ở §3.

---

## 3. **Bị từ chối có chủ đích** (fail-closed)

| Dạng | Mã lỗi | Lý do |
|---|---|---|
| `include!`/`#[path]`/`cfg_attr(path)` **không literal** (`concat!`, `env!`, biến) | `E_STRICT_WAKE_INCLUDE` | không phân giải được đích |
| Đích trỏ **ra ngoài** `v2/kernel`+`v2/sim` (nguồn kernel/sim) | `E_STRICT_WAKE_INCLUDE` | không quét được |
| Đích trỏ **ra ngoài** `v2/lib` (nguồn `v2/lib`, kể cả sang `v2/kernel`/`v2/sim`) | `E_STRICT_WAKE_INCLUDE` | cạnh nguồn ẩn; luật `wake` chỉ quét kernel/sim (KT-209/L3) |
| Đích **không tồn tại** (và không phải thư mục) | `E_STRICT_WAKE_INCLUDE` | fail-closed |
| `#[path]` trỏ **thư mục** trên `mod` **không có thân** | `E_STRICT_WAKE_INCLUDE` | sai cú pháp, không phân giải được (KT-199/F4c) |
| Thân `macro_rules!` chứa `include!`/`#[path]` (bất kỳ dạng) | `E_STRICT_MACRO_INCLUDE` | rustc giải theo **tệp gọi** |
| Thân `macro_rules!` chứa `$…:tt` (passthrough) | `E_STRICT_MACRO_TT` | có thể chở `mod`/`include!` từ **lời gọi** |
| Thân `macro_rules!` khai `mod NAME;` (định danh literal) | `E_STRICT_MACRO_MOD` | đồ thị module không thấy |
| Thân `macro_rules!` khai `mod $n;` / `mod r#x;` | `E_STRICT_MACRO_INCLUDE` | không phân giải được tên module |
| `wake(` trong mã sản phẩm **thuộc `TEST_ONLY_WAKE_SCOPE`** | `E_STRICT_TEST_ONLY_WAKE` | D2-144 |
| Chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá độ sâu | `E_LEX_*` | D2-166 §3C |

---

## 4. **Sai lệch implementation còn mở** (đã biết, chưa đóng)

| Mã | Mô tả | Ảnh hưởng |
|---|---|---|
| **F2 (một phần)** | Ngữ cảnh test đã theo item/token-tree, nhưng luật `wake` **vẫn chỉ chạy đầy đủ ở `--strict`** (D2-144). Chế độ thường chỉ quét tệp bị kéo vào bằng `#[path]`/`include!`. | `v2 check`/`gate` chạy `--strict` nên cổng vẫn bắt; nhưng gọi checker ở chế độ thường **không** đủ. |
| **Macro invocation** | `_walk_items` **bỏ qua** nhóm sau `!` (lời gọi macro). Nếu lời gọi chứa `mod`/`#[path]` (vd `cfg_if! { … }`), cạnh nguồn **không** được phân giải. | Cây thật **không** có lời gọi macro chứa `mod`/`#[path]` (đã grep). **Không** tuyên bố fail-closed cho phạm vi này. |
| ~~`#[path]`/`include!` trong `v2/lib`~~ | **ĐÓNG (KT-209/L3)**: worklist seed thêm mọi tệp `v2/lib/**/*.rs`; đích phải ở trong `v2/lib` (fail-closed). | — |
| **Tệp `.rs` không thuộc crate** | `v2/kernel/vifast-plat-user/lib.rs` (shim U-mode) và `v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode) **không** khớp mẫu `<crate>/src/**`/`build.rs` ⇒ không bị luật `wake` quét **trực tiếp** (một số tệp `init/*.rs` được kéo vào qua `#[path]` từ `src/` và xử lý như đích `extra`). | Đo 2026-11-20: **không** có token `wake`; chúng không có `Machine` (chỉ syscall U-mode). Ghi để không thành "phát hiện mới". |
| **F3 (v2/lib)** | `cfg_attr` lồng nhiều tầng trong `v2/lib` **ngoài** thân macro ⇒ XANH (chỉ `v2/kernel`+`v2/sim` được quét cấu trúc). | §1 đã ghi `v2/lib` chỉ bảo vệ thân macro + `include!`/`#[path]`; **không** phải lỗi. |
| **Báo oan có trước** (không phải hồi quy — đối chiếu checker `0fa0fe65` giống hệt) | `#[cfg(test)] include!`; `#[cfg(feature = "ktest")] fn` **inline** (chuỗi `"ktest"` bị `strip_rust_noise` xoá); `#[cfg(test)] const/static`; `macro_rules!` thân `()`/`[]`; block statement; match arm pattern literal. | Cây thật **không** có dạng nào (mọi chỗ `ktest` đều không chứa `wake`); ghi để không thành "phát hiện mới". |

---

## 5. **Ngoài phạm vi bảo đảm**

- **Ngữ nghĩa mở rộng macro** — không mô phỏng `macro_rules!`; chỉ từ chối thân macro có dấu hiệu.
- **Proc-macro** (`paste!`, `concat_idents!`) — không có trong allowlist phụ thuộc. **Không** có điểm
  kiểm soát cụ thể ⇒ **không** tuyên bố fail-closed cho proc-macro.
- **Mã sinh lúc build** (`build.rs`, `OUT_DIR`) — `build.rs` **nằm trong** phạm vi luật `wake`
  (KT-209/L2), nhưng `include!(concat!(env!("OUT_DIR"), …))` **bị từ chối** (fail-closed), không
  phải báo oan.
- **`#[cfg]` phức tạp** — chỉ hiểu `#[cfg(test)]` và `#[cfg(feature = "ktest")]`.
- **Đường dẫn qua symlink** — phân giải theo `Path::resolve`, không theo symlink của rustc.
- **Code nằm ngoài `v2/kernel`/`v2/sim`** — **không** phân tích cấu trúc, **trừ** ba đường: thân
  `macro_rules!` trong `v2/lib` (`scan_lib_macros`), **quét từ vựng** `v2/lib` (`scan_lexical`:
  chuỗi/comment chưa đóng trong `v2/lib` vẫn ĐỎ `E_LEX_*`), và **phân giải `include!`/`#[path]`
  trong `v2/lib`** (`resolve_include_targets`, KT-209/L3).

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
