# Hợp đồng của `check-wake-paths.py` — KT-184 / KT-194 / KT-196 / KT-209 / KT-211 / KT-213 / KT-215 / KT-219 / KT-225

**Trạng thái**: bản cập nhật theo **A0 2026-11-19 §5** (tách bốn loại) + **KT-209** (L40 KT-201
§D2 L2/L3) + **KT-211** (L44 KT-208 §B1/D — quét token toàn cục cho `include!`/`#[path]`) +
**KT-213** (L47 KT-212 — gốc phân giải `#[path]` của tệp non-mod-rs) + **KT-215** (L47 KT-212 —
bộ lọc `target/`, `MAILBOX_DEFS`, đóng F2) + **KT-219** (L49 KT-218 — vùng test quá rộng trên
nhánh `match`, đường lọt #13) + **KT-225** (L52 KT-223 — bốn báo oan `include!` + đường lọt lớp một
`.txt` + răng ca B29/B30). **Chưa** được reviewer độc lập xác nhận trên HEAD
cuối — KT-184, KT-194, KT-211, KT-213 vẫn `REVIEW`.

Máy kiểm này **không** phải bộ phân tích Rust tổng quát. Nó là **lexer + token-tree + bộ đọc cấu
trúc tối thiểu**, chạy **không cần biên dịch**, và **fail-closed** ở mọi chỗ nó không phân giải được.

---

## 1. Phạm vi cây nguồn được bảo vệ

| Cây | Luật áp |
|---|---|
| `v2/kernel/<crate>/src/**` + `v2/kernel/<crate>/build.rs` | `include!`/`#[path]`/`cfg_attr(path)` literal + trong cây; `wake` chỉ-dùng-test; thân `macro_rules!` (**token `wake`** + `include!`/`#[path]`/`mod`/`:tt`) |
| `v2/sim/<crate>/src/**` + `v2/sim/<crate>/build.rs` | như trên |
| `v2/lib/**` | thân `macro_rules!` (token `wake`, `include!`, `#[path]`, `mod`, `:tt`); `include!`/`#[path]` **ngoài** macro được phân giải, đích phải ở trong `v2/lib` |
| `v2/test/**`, `v2/tools/**`, `v2/agent/**` | **không** bảo vệ (ngoài phạm vi) |

**KT-209/L2** — phạm vi `wake` là **mẫu**, không liệt kê cứng: **mọi** crate dưới
`v2/kernel/<crate>/` và `v2/sim/<crate>/` (toàn bộ `src/**` cộng `build.rs`) nằm trong phạm vi,
kể cả crate ra đời sau. `tests/` cấp crate nằm ngoài (test-only, `is_test_file` miễn).
**Ngoài mẫu** (đo 2026-11-20: **không** có token `wake`): `v2/kernel/vifast-plat-user/lib.rs` (shim
U-mode, **không** phải crate — biên dịch bởi `vifast-kbin-riscv64/build.rs`) và
`v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode, biên dịch bởi `build.rs`).

**KT-181** (D2-164): thân `macro_rules!` được quét ở **cả ba** gốc `v2/lib`+`v2/kernel`+`v2/sim`
(`MACRO_SCAN_ROOTS`), **kể cả** `v2/kernel/vifast-kbin-riscv64/init/` (chương trình U-mode đóng
vào ảnh), `v2/kernel/*/tests/`, `v2/kernel/vifast-plat-user/` (shim ABI U-mode). Trước KT-181
luật token `wake` chỉ chạy ở `v2/lib`; `v2/kernel`+`v2/sim` chỉ có luật `include!`/`#[path]`.
`v2/tools/**` (công cụ phát triển, **không** link vào ảnh sản phẩm) và `v2/test/fixtures/**`
(fixture **cố tình sai**) nằm ngoài — lý do ở §4.

Đích của `include!`/`#[path]` trong `v2/kernel`/`v2/sim` phải nằm trong `v2/kernel` hoặc
`v2/sim`; trỏ ra ngoài ⇒ **ĐỎ**. Đích của `include!`/`#[path]` trong `v2/lib` phải nằm trong
`v2/lib`; trỏ sang `v2/kernel`/`v2/sim` hoặc ra ngoài ⇒ **ĐỎ** (KT-209/L3).

**KT-211** — cạnh `include!`/`#[path]` **phân giải được** khi nó nằm ở **bất kỳ vị trí token
nào** trong tệp: quét **toàn bộ token** (`_walk_all`), **mọi** tầng nhóm, **mọi** loại nhóm
`{`/`(`/`[`. Trước KT-211 đường cấu trúc `_walk_items` chỉ đệ quy `{ … }` ⇒ `include!` ở **vị
trí biểu thức** trong `( … )`/`[ … ]` (`sink(include!{…})`, `[include!{…}]`, `(0u8,
include!{…})`, `(include!{…})`, `(|| include!{…})`, `Some(include!{…})`) và `#[path]` trên
`mod` trong khối lồng `( … )` **không** được phân giải, **không** bị từ chối — `cargo check`
**biên dịch được**, checker **XANH** **cả hai** chế độ (reviewer L44). **Hai** vị trí **không** phân giải
được và **bị từ chối riêng** (không phải lỗ hổng): thân `macro_rules!` (`_check_macro_body`,
`E_STRICT_MACRO_INCLUDE`) và **đối số lời gọi macro** (`_check_macro_passthrough`,
`E_STRICT_MACRO_INCLUDE`/`E_STRICT_MACRO_MOD`).

**KT-225** (L52 KT-223 — bốn báo oan) — `_walk_all` **chỉ** coi `include` + `!` là `include!` khi
sau `!` là **nhóm phân cách** `{`/`(`/`[` hoặc **literal chuỗi** (cú pháp macro hợp lệ). Hệ quả:
`include` + `!=` / `self.include` + `!=` (`!` + `=`) **không** còn bị ĐỎ oan; `include!("x",)`
(trailing comma) được **nhận** (XANH nếu đích trong cây). `_walk_all` **không** sinh cạnh từ
**đối số lời gọi macro** (`in_macro_args`): token trong đó là token stream, không phải mã cho tới
khi macro mở rộng — `_check_macro_passthrough` xử lý riêng, **trừ** `stringify!`/`concat!`
(`SAFE_TOKEN_MACROS`: không mở rộng đối số thành mã) ⇒ `stringify!(include!)` **XANH**; tên an toàn bị `macro_rules!` trong cây **che** thì **không** miễn (`_shadowed_safe_macros`, cùng lý do KT-177/G6) ⇒ `macro_rules! stringify { ($x:expr) => { $x } }` + `stringify!(include!("…"))` vẫn **ĐỎ**. Thân
`macro_rules!` trong tệp **không phải `.rs`** được `include!` vào cây cũng được quét
(`scan_macro_bodies` nhận `extra_files`) — đóng đường lọt lớp một `.txt` (reviewer L52 KT-223 §3).

**KT-202** — `include!`/`#[path]`/`mod` nằm trong **đối số lời gọi macro** bị **từ chối thẳng**
(`E_STRICT_MACRO_INCLUDE`/`E_STRICT_MACRO_MOD`) thay vì phân giải: `_scan_macro_calls` quét
**toàn cây token** (mọi tầng nhóm, kể cả `( … )`/`[…]`), nên không phụ thuộc đích. **KT-211**:
`_scan_macro_calls` **chỉ** phủ **đối số lời gọi macro** — `include!` **trần** (không phải đối
số lời gọi) trong `( … )`/`[ … ]` **không** được nó kiểm; lớp đó do **quét token toàn cục**
(`_walk_all`) phủ. `_scan_macro_calls` **giữ** vì nó bắt `mod` trong đối số lời gọi macro
(`E_STRICT_MACRO_MOD`) — quét toàn cục **không** xử lý `mod` (chỉ có nghĩa ở mức item); đã đo:
xoá nó ⇒ ca `pass!(mod evil;)` **XANH**.

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
- **KT-219** — vùng test kết thúc ở **cuối mục thật**: thân `{…}` nếu mục có thân, ngược lại
  `;` hoặc `,` **cùng mức**. Dấu `,` chỉ là mốc kết thúc khi đã thấy `=>` (nhánh `match`) — `,`
  còn xuất hiện trong generic params (`fn f<T, U>`), `use a::b, c::d;`, field struct, biến thể
  enum: các ngữ cảnh đó **không chứa được lời gọi `wake`**, cắt sớm chỉ làm vùng test **hẹp**
  hơn (báo oan), không làm rộng (lọt). Trước bản này `#[cfg(test)]` trên nhánh `match`
  `A => (),` bị quét tiếp sang nhánh kế và lấy thân `{…}` của nhánh **sản phẩm** làm thân mục
  test ⇒ nuốt `m.wake(t)` — đường lọt #13, lọt **cả hai** lớp vì `check-wake-depinfo.py` dùng
  lại chính `_test_char_ranges`. Số đo + repro đầy đủ: `JOURNAL/kt219-test-range.md`.
- `macro_rules!` với **mọi** tên (kể cả Unicode) và **cả ba** dấu thân `{ … }`, `( … )`, `[ … ]`.
- **KT-181**: thân `macro_rules!` ở **mọi** tệp `v2/lib`+`v2/kernel`+`v2/sim` được kiểm **cả
  hai** luật — token `wake` (`E_STRICT_MACRO_WAKE`, trừ `MACRO_WAKE_ALLOW` có lý do) **và**
  `include!`/`#[path]`/`mod`/`:tt` (`_check_macro_body`). Một hàm `scan_macro_bodies`, một phạm
  vi; không còn khoảng trống giữa `scan_lib_macros` (cũ) và `scan_kernel_macro_includes` (cũ).
- **KT-202**: `_scan_macro_calls` quét **toàn cây token** (mọi tầng nhóm, kể cả `( … )`/`[…]`)
  tìm **lời gọi macro** — `include!`/`#[path]`/`mod` trong **đối số lời gọi** bị từ chối
  (`_check_macro_passthrough`, dùng chung với thân macro); `$…:<frag>` với
  `frag ∈ {tt, item, stmt, block, meta}` bị từ chối (`E_STRICT_MACRO_TT`). **KT-211**: hàm này
  **chỉ** phủ **đối số lời gọi macro**; `include!` **trần** trong `( … )`/`[ … ]` **không** được
  nó kiểm (xem gạch đầu dòng KT-211 dưới).
- **KT-211**: `include!` và `#[path]` được **quét toàn cục** (`_walk_all`) — **mọi** tầng nhóm,
  **mọi** loại nhóm `{`/`(`/`[`, **mọi** vị trí token (kể cả vị trí biểu thức trong `( … )`/
  `[ … ]` và `#[path]` trên `mod` trong khối lồng `( … )`). `include!` phân giải theo **thư mục
  tệp chứa** (đúng rustc: `mod m { include!("x.rs") }` ⇒ `x.rs` cạnh tệp gốc); `#[path]` phân
  giải theo **thư mục module** (`cur_dir`), đổi khi vào thân **inline module** (đúng rustc:
  `#[path = "sub"] mod m { #[path = "y.rs"] mod n; }` ⇒ `sub/y.rs`). **Bỏ qua** thân
  `macro_rules!` (bị `_check_macro_body` từ chối riêng). `mod` **giữ** đường cấu trúc
  (`_walk_items`/`scan_structure`): `mod` chỉ có nghĩa ở mức item, và mọi vị trí `mod` hợp lệ
  đều nằm trong `{ … }` mà `_walk_items` đã đệ quy; đường thoát khỏi cây bằng `mod` **phải** qua
  `#[path]` — đã bị quét toàn cục bắt.
- **KT-225**: `include` + `!` **chỉ** là `include!` khi sau `!` là **nhóm phân cách** hoặc
  **literal chuỗi**; `include` + `!=` / `self.include` + `!=` **XANH**; `include!("x",)` (trailing
  comma) **XANH** nếu đích trong cây. Đối số `stringify!`/`concat!` (`SAFE_TOKEN_MACROS`) **không**
  bị coi là mã ⇒ `stringify!(include!)` **XANH**. Thân `macro_rules!` trong tệp **không phải
  `.rs`** được `include!` vào cây **được quét** (`scan_macro_bodies(extra_files=…)`).
- Chú thích chen giữa `#`/`[`/`path` (khử chú thích trước khi khớp).
- `#[path = "sub"] mod m { … }` — inline module: `sub` là **thư mục**, không phải cạnh tệp (KT-196/F4);
  module con của nó được duyệt trong **thư mục `#[path]`**, không phải `cur_dir/NAME` (KT-199/F4b).
- **`include!`/`#[path]` trong `v2/lib` ngoài thân macro** (KT-209/L3): worklist seed thêm **mọi**
  tệp `v2/lib/**/*.rs`; đích trong `v2/lib` được phân giải **tiếp** (lồng bao nhiêu lớp cũng hết).
  Đích trong `v2/lib` **không** vào tập bị luật `wake` quét (`extra`) — mã lib không có `Machine`
  nên `wake()` không thể xuất hiện; cạnh sang kernel/sim đã bị chặn ở §3.
- **Gốc phân giải `#[path]` đúng như rustc** (KT-213). Hai gốc **khác nhau**, không được lẫn:
  - `#[path = "…"]` ở **mức mục** giải theo **thư mục của tệp** (`p.parent`) — đúng cho cả tệp
    mod-rs lẫn non-mod-rs;
  - `#[path = "…"]` **trong inline module** giải theo **thư mục module của tệp** cộng các thành
    phần inline: mod-rs (crate root, `mod.rs`, tệp nạp bằng `#[path]`) ⇒ `<thư mục tệp>/<inline>/…`;
    non-mod-rs (`src/foo.rs` nạp bằng `mod foo;`) ⇒ `<thư mục tệp>/foo/<inline>/…`;
  - `mod NAME;` **không** `#[path]` giải theo **thư mục module của tệp** (`src/foo/NAME.rs` với
    `src/foo.rs`), không theo thư mục tệp;
  - `#[path = "sub"] mod m { … }` (inline module có `#[path]`) ⇒ thư mục con là `<mức hiện tại>/sub`,
    **không** áp tiền tố `<stem>/` (đã đo bằng `rustc`: `src/foo.rs` +
    `#[path="sub"] mod m { #[path="a.rs"] mod x; }` ⇒ `src/sub/a.rs`; log ở
    `v2/agent/JOURNAL/kt212-attack-path-mod.md` §3).
  `module_dirs()` (dựng từ **cùng** đồ thị module với `test_only_modules`) cung cấp thư mục module
  cho mọi tệp tới được từ crate root; tệp ngoài đồ thị (`build.rs`, `init/*.rs`,
  `vifast-plat-user/lib.rs` — crate root do `build.rs` biên dịch) dùng thư mục của chính tệp.

---

## 3. **Bị từ chối có chủ đích** (fail-closed)

| Dạng | Mã lỗi | Lý do |
|---|---|---|
| `include!`/`#[path]`/`cfg_attr(path)` **không literal** (`concat!`, `env!`, biến) — **mọi** vị trí token (KT-211) | `E_STRICT_WAKE_INCLUDE` | không phân giải được đích |
| `include!` **không có** đối số **nhưng** có nhóm rỗng `include!()` (KT-211) | `E_STRICT_WAKE_INCLUDE` | không phân giải được đích. **KT-225**: `include!;` (không có nhóm/chuỗi sau `!`) **không** phải cú pháp macro hợp lệ ⇒ **không** báo (không biên dịch được) |
| Đích trỏ **ra ngoài** `v2/kernel`+`v2/sim` (nguồn kernel/sim) — **mọi** vị trí token (KT-211) | `E_STRICT_WAKE_INCLUDE` | không quét được |
| Đích trỏ **ra ngoài** `v2/lib` (nguồn `v2/lib`, kể cả sang `v2/kernel`/`v2/sim`) | `E_STRICT_WAKE_INCLUDE` | cạnh nguồn ẩn; luật `wake` chỉ quét kernel/sim (KT-209/L3) |
| Đích **không tồn tại** (và không phải thư mục) | `E_STRICT_WAKE_INCLUDE` | fail-closed |
| `#[path]` trỏ **thư mục** trên `mod` **không có thân** | `E_STRICT_WAKE_INCLUDE` | sai cú pháp, không phân giải được (KT-199/F4c) |
| Thân `macro_rules!` chứa `include!`/`#[path]` (bất kỳ dạng) — **kể cả** thân trong tệp **không phải `.rs`** được `include!` vào cây (KT-225) | `E_STRICT_MACRO_INCLUDE` | rustc giải theo **tệp gọi** |
| Thân `macro_rules!` chứa `$…:<frag>` với `frag ∈ {tt, item, stmt, block, meta}` (passthrough) | `E_STRICT_MACRO_TT` | có thể chở `mod`/`include!`/`#[path]` từ **lời gọi**; `:expr` **không** bị từ chối (lý do §4) |
| `include!`/`#[path]`/`mod` trong **đối số lời gọi macro** (nhóm sau `!`) | `E_STRICT_MACRO_INCLUDE` / `E_STRICT_MACRO_MOD` | `_walk_items` cũ bỏ **mù** nhóm sau `!`; KT-202 fail-closed |
| Thân `macro_rules!` khai `mod NAME;` (định danh literal) | `E_STRICT_MACRO_MOD` | đồ thị module không thấy |
| Thân `macro_rules!` khai `mod $n;` / `mod r#x;` | `E_STRICT_MACRO_INCLUDE` | không phân giải được tên module |
| Thân `macro_rules!` chứa token `wake` (mọi gốc `v2/lib`+`v2/kernel`+`v2/sim`, **KT-181**) | `E_STRICT_MACRO_WAKE` | macro có thể sinh `wake()` ở **tệp gọi**; trừ `MACRO_WAKE_ALLOW` có lý do |
| `wake(` trong mã sản phẩm **thuộc `TEST_ONLY_WAKE_SCOPE`** | `E_STRICT_TEST_ONLY_WAKE` | D2-144 |
| Chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá độ sâu | `E_LEX_*` | D2-166 §3C |

---

## 4. **Sai lệch implementation còn mở** (đã biết, chưa đóng)

| Mã | Mô tả | Ảnh hưởng |
|---|---|---|
| ~~**Lớp hai mù `src/target/` + bất đối xứng allowlist producer**~~ | **ĐÓNG (KT-226, reviewer L51 của KT-222)**: (i) `check-wake-depinfo.py` dùng `"target" in relp.parts` ⇒ bỏ qua **cả** `src/target/` — tái hiện đường lọt thư mục build ở **lớp hai** (lớp một đã vá ở KT-215). Nay lớp hai gọi **chung** `CWP._is_build_dir` (import, không chép lần hai) ⇒ chỉ bỏ `target` **trước** `src`; (ii) lớp hai miễn **cả tệp** định nghĩa `fn wake` trong khi lớp một chỉ miễn **định nghĩa** ⇒ tệp producer chứa lời gọi `wake(` XANH ở lớp hai / ĐỎ ở lớp một. Nay lớp hai **không** miễn cả tệp (chỉ miễn định nghĩa qua `_check_wake_text`), đồng nhất lớp một; cây thật vẫn xanh (đo: mọi tệp nạp đều không lỗi). Ca selftest: `_is_build_dir`; `.d` nạp `src/target/evil.rs` ⇒ ĐỎ; `.d` nạp `v2/kernel/target/evil.rs` ⇒ bị lọc; tệp producer gọi `wake(` ⇒ ĐỎ. | **Đã vá**; cây thật xanh, `--selftest` xanh toàn bộ. Số đo: `JOURNAL/kt226-layer2-target.md`. |
| ~~**Vùng test quá rộng trên nhánh `match`**~~ | **ĐÓNG (KT-219, L49 KT-218 — đường lọt #13)**: `_test_char_ranges` (KT-198/F2b) quét từ mục tới **`{` group kế tiếp**, chỉ dừng ở `;`; với mục **không có thân `{…}`** mà kết thúc bằng **`,`** (nhánh `match` `A => (),`, field struct, biến thể enum) vùng test chạy tiếp sang mục kế và lấy thân `{…}` của **mục sản phẩm** làm thân mục test. Repro: `#[cfg(test)] A => (),` + nhánh `B => { m.wake(t); }` trong `v2/sim/vifast-sim/src/evil.rs` ⇒ `cargo check` **thành công**, checker thường **XANH**, `--strict` **XANH**, `check-wake-depinfo` **XANH** (lớp hai dùng lại chính hàm này). Đối chứng âm: bỏ dòng `#[cfg(test)]` ⇒ cả hai lớp **ĐỎ** (`E_STRICT_TEST_ONLY_WAKE`/`E_DEPINFO_WAKE`). Nay vùng test kết thúc ở **cuối mục thật**: thân `{…}` nếu có, ngược lại `;` hoặc `,` cùng mức (`,` chỉ khi đã thấy `=>`). Ca selftest KT-219: nhánh match âm/dương + các dạng mục không thân khác + đối chứng dương. | **Đã vá**; cây thật xanh cả hai chế độ, `--selftest` xanh toàn bộ, lớp hai xanh. Số đo: `JOURNAL/kt219-test-range.md`. |
| ~~**F2 (một phần)**~~ | **ĐÓNG (KT-215)**: luật `wake` trước đây chỉ chạy đầy đủ ở `--strict`; chế độ thường chỉ quét tệp bị kéo vào bằng `#[path]`/`include!` (`extra`). Đường lọt #12 (`src/target/evil.rs`) là module **bình thường** — không bao giờ vào `extra` — nên chế độ thường XANH trong khi `--strict` ĐỎ. Nay chế độ thường quét **mọi** tệp sản phẩm trong phạm vi (cùng tập đích với `--strict`); `--strict` chỉ còn thêm allowlist `transition_tcb` (`E_STRICT_TRANSITION`/`E_STRICT_STALE`). | **Đã đóng**; cây thật xanh cả hai chế độ. |
| **Macro invocation** (KT-202 đóng **một dạng**; KT-211 đóng **lớp**) | `_walk_items` **bỏ mù** nhóm sau `!` (lời gọi macro) — trước KT-202 `include!`/`#[path]`/`mod` trong đối số lời gọi không bao giờ được phân giải. Reviewer L40: `macro_rules! pass { ($i:item) => { $i }; } pass!(include! {"…"});` ⇒ `cargo check` biên dịch được, checker **xanh** **cả hai** chế độ. KT-202 thêm `_scan_macro_calls` quét **toàn cây token** tìm lời gọi macro ⇒ `E_STRICT_MACRO_INCLUDE`/`E_STRICT_MACRO_MOD` (fail-closed) — **chỉ** phủ **đối số lời gọi macro**. **KT-211** (reviewer L44) đo được **lớp lỗi vẫn sống**: `include!` **trần** ở vị trí biểu thức trong `( … )`/`[ … ]` (`sink(include!{…})`, `[include!{…}]`, `(0u8, include!{…})`, `(include!{…})`, `(|| include!{…})`, `Some(include!{…})`) và `#[path]` trên `mod` trong khối lồng `( … )` — `cargo check` **biên dịch được**, checker **XANH** **cả hai** chế độ. Nay `_walk_all` quét **toàn bộ token** (mọi tầng nhóm, mọi loại nhóm) cho `include!` + `#[path]`; `mod` giữ đường cấu trúc (lý do §2). | **Đã đóng lớp** (KT-211); cây thật xanh. `_scan_macro_calls` **giữ** (bắt `mod` trong đối số lời gọi macro — quét toàn cục không xử lý `mod`; đã đo: xoá nó ⇒ ca `pass!(mod evil;)` XANH). |
| **`:expr` không bị từ chối** (KT-202) | `$…:expr` chở được `include!` ở vị trí biểu thức, nhưng **không** chở được `mod`/`#[path]`; `include!` trong **đối số lời gọi** đã bị vá **đầu tiên** bắt ở **tệp gọi**. Cây thật có `macro_rules! bail` (`v2/kernel/vifast-plat-riscv64/src/user.rs`) dùng `$pages:expr, $e:expr` hợp lệ — từ chối `:expr` sẽ báo oan và làm `v2 check` ĐỎ (đo: các lỗi `E_STRICT_MACRO_TT` trên `bail`, `kt181-logs/kt202-expr-conflict.log`). | Chấp nhận: `:expr` chỉ còn hở khi macro được **gọi từ tệp ngoài `MACRO_SCAN_ROOTS`** (không phải mã sản phẩm). Ghi rõ, **không** tuyên bố đã đóng. |
| ~~`#[path]`/`include!` trong `v2/lib`~~ | **ĐÓNG (KT-209/L3)**: worklist seed thêm mọi tệp `v2/lib/**/*.rs`; đích phải ở trong `v2/lib` (fail-closed). | — |
| ~~**Gốc phân giải `#[path]` của tệp non-mod-rs**~~ | **ĐÓNG (KT-213, reviewer L47 KT-212 — đường lọt #11)**: `resolve_include_targets` truyền `p.parent` làm gốc cho **mọi** tệp, nên `#[path]` trong **inline module** của `src/foo.rs` giải theo `src/<inline>/` thay vì `src/foo/<inline>/` (rustc). Hệ quả: `#[path]` trỏ **ra ngoài `src/`** (ngoài phạm vi luật `wake`) vẫn **XANH** trong khi rustc biên dịch tệp thật. Repro đã đo: `src/foo.rs` = `mod inner { #[path = "../../../evil.rs"] mod baz; }` + `src/foo/inner/notes.txt` (chỉ để thư mục tồn tại) + `vifast-core/evil.rs` chứa `m.wake(t)` + decoy `v2/kernel/evil.rs` ⇒ `cargo check -p vifast-core` **thành công**, checker **XANH cả hai chế độ**. Cùng gốc sai ở `_mods_of` (nó truyền `mod_dir` làm `cur_dir` ⇒ `#[path]` **mức mục** trong `src/foo.rs` giải thành `src/foo/a.rs` thay vì `src/a.rs`). Nay `_walk_items` nhận **hai** gốc (`cur_dir` cho `#[path]` mức hiện tại, `mod_dir` cho inline module + `mod NAME;`), `module_dirs()` dùng chung đồ thị module với `test_only_modules`. Ca selftest KT-213: repro âm (thường + `--strict`), đối chứng dương cho `#[path]` trong inline module / `#[path]` mức mục / `#[path]` trên inline module, và hai ca `_mods_of`. | **Đã vá**; cây thật xanh cả hai chế độ, `--selftest` xanh toàn bộ (log ở `v2/agent/JOURNAL/kt213-logs/`). |
| ~~**Bộ lọc thư mục build `target/`**~~ | **ĐÓNG (KT-215, reviewer L47 KT-212 — đường lọt #12)**: cả **năm** chỗ quét (`iter_rs`, `_protected_files`, `scan_structure`, `scan_macro_bodies`, `_shadowed_safe_macros`) dùng `"target" in p.parts`, khớp **bất kỳ** thành phần `target` nào — kể cả `src/target/`. Hệ quả: `mod target;` + `src/target/evil.rs` chứa `wake` **biên dịch được** (`cargo check` thành công) mà checker **XANH cả hai chế độ**; tệp không vào `files`, không vào `_protected_files`, không vào `scan_structure`, không vào `extra`. Nay **một** hàm `_is_build_dir(root, p)` dùng chung: chỉ bỏ qua khi `target` đứng **trước** `src` trong đường dẫn tương đối (thư mục build thật `v2/kernel/target/`, `v2/target/`, `<crate>/target/`), **không** bỏ qua `src/**/target/**`; đường dẫn ngoài `root` ⇒ **không** bỏ qua (fail-closed). Ca selftest: repro âm (thường + `--strict`), đối chứng dương (tệp trong `v2/kernel/target/` vẫn bị bỏ qua), và bảng `_is_build_dir`. | **Đã vá**; cây thật xanh cả hai chế độ. |
| ~~**Miễn luật kênh theo tên `mailbox.rs`**~~ | **ĐÓNG (KT-215, cùng lớp)**: điều kiện `p.name == "mailbox.rs"` khớp **mọi** tệp cùng tên ở bất kỳ thư mục nào ⇒ một tệp `src/sub/mailbox.rs` chứa `post_mail(…, MailboxMsg::Wake(t))` được miễn luật kênh phát (kênh wake thứ hai). Nay so **đúng relpath** của hai tệp định nghĩa trong phạm vi quét (`MAILBOX_DEFS`). Ca selftest: tệp cùng tên ở thư mục khác phát `MailboxMsg::Wake` ⇒ ĐỎ. | **Đã vá**. |
| **Miễn luật arch theo tiền tố `virtio_`** (mở) | `RE_DOORBELL` bỏ qua tệp có tên bắt đầu `virtio_` (thiết bị virtio dùng `doorbell` hợp lệ). Một tệp `virtio_x.rs` trong `vifast-plat-*` dùng `doorbell` sẽ được miễn. **Chưa vá**: phân biệt "thiết bị virtio thật" cần kiểm ngữ nghĩa (kiểu `Virtio*`), không phải đổi tên; hạ tầng hiện tại không có. Ghi nợ có tên. | Không phải luật `wake`/`include!`/`#[path]`; rủi ro thấp (luật arch chỉ áp `vifast-plat-*`). |
| **Tệp `.rs` không thuộc crate** | `v2/kernel/vifast-plat-user/lib.rs` (shim U-mode) và `v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode) **không** khớp mẫu `<crate>/src/**`/`build.rs` ⇒ không bị luật `wake` quét **trực tiếp** (một số tệp `init/*.rs` được kéo vào qua `#[path]` từ `src/` và xử lý như đích `extra`). | Đo 2026-11-20: **không** có token `wake`; chúng không có `Machine` (chỉ syscall U-mode). Ghi để không thành "phát hiện mới". |
| **F3 (v2/lib)** | `cfg_attr` lồng nhiều tầng trong `v2/lib` **ngoài** thân macro ⇒ XANH (chỉ `v2/kernel`+`v2/sim` được quét cấu trúc). | §1 đã ghi `v2/lib` chỉ bảo vệ thân macro + `include!`/`#[path]`; **không** phải lỗi. |
| **Báo oan có trước** (không phải hồi quy — đối chiếu checker `0fa0fe65` giống hệt) | `#[cfg(test)] include!`; `#[cfg(feature = "ktest")] fn` **inline** (chuỗi `"ktest"` bị `strip_rust_noise` xoá); `#[cfg(test)] const/static`; `macro_rules!` thân `()`/`[]`; block statement; match arm pattern literal. **KT-211** (reviewer L44 §D6) đo thêm **bốn** dạng **ĐỎ** (`--strict`) mà danh sách **không** có: (a) `#[cfg_attr(test, path = "evil.rs")] mod m;` (đích có `wake`) ⇒ **ĐỎ**; (b) `if let Some(wake) = o { wake }` ⇒ **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; (c) `let g = \|wake: u32\| wake + <số>;` ⇒ **ĐỎ**; (d) `struct S { pub wake: u32 }` + `s.wake` ⇒ **ĐỎ**. **KT-225** (L52 KT-223 §C) **đã vá bốn** dạng báo oan của luật `include!`: `stringify!(include!)` (nay **XANH** — `SAFE_TOKEN_MACROS`), `include` + `!=` / `self.include` + `!=` (nay **XANH** — `!` + `=` không phải macro), `include!("x",)` (nay **XANH** — trailing comma hợp lệ). | Cây thật **không** có dạng nào (mọi chỗ `ktest` đều không chứa `wake`); ghi để không thành "phát hiện mới". |
| ~~**Thân `macro_rules!` trong tệp không phải `.rs`**~~ | **ĐÓNG (KT-225, L52 KT-223 — đường lọt lớp một)**: `scan_macro_bodies` chỉ `rglob("*.rs")`, nên `macro_rules! evil { () => { include!("…") } }` trong `src/macro_def.txt` (được `include!("macro_def.txt")` ở vị trí item) **không** bị `_check_macro_body` quét; `_walk_all` bỏ qua thân macro ⇒ checker tĩnh **XANH cả hai chế độ** trong khi `cargo check` thành công và mã sản phẩm gọi `wake` (lớp hai `check-wake-depinfo.py` bắt được). Nay `scan_macro_bodies` nhận `extra_files` = mọi tệp được `include!`/`#[path]` vào cây (không chỉ `.rs`) ⇒ quét thân macro ⇒ **ĐỎ** `E_STRICT_MACRO_INCLUDE`. Ca selftest KT-225 TXT. | **Đã vá**; cây thật xanh cả hai chế độ. |
| **Macro ngoài `MACRO_SCAN_ROOTS`** (KT-181) | `v2/tools/**` (công cụ phát triển, **không** link vào ảnh sản phẩm) và `v2/test/fixtures/**` (fixture **cố tình sai**) **không** được quét thân macro. | Không phải mã sản phẩm; cây thật **không** có `macro_rules!` ở hai gốc này. Nếu tương lai có mã sản phẩm ở đó, phải thêm gốc vào `MACRO_SCAN_ROOTS` — **không** được coi là đã bảo vệ. |
| **Không có đường lọt thật cho `wake` ngoài phạm vi cũ** (KT-181 — ghi để **không thổi phồng**) | Trước KT-181, macro ở `init/`/`tests/`/`vifast-plat-user/` chứa token `wake` **không** bị luật token bắt. Nhưng **không** dựng được đường lọt: `init/`+`vifast-plat-user/` biên dịch bằng `rustc` trực tiếp **không** `--extern vifast_core`; `tests/` không phải sản phẩm; nạp bằng `#[path]`/`include!` từ kernel thì `extra` đã quét (đột biến M1c: `cargo check` biên dịch được nhưng checker **ĐỎ**). | Đây là **phòng ngừa**, không phải lỗ hổng đã bị khai thác. |

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
  `macro_rules!` ở **cả ba** gốc `v2/lib`+`v2/kernel`+`v2/sim` (`scan_macro_bodies`, KT-181),
  **quét từ vựng** `v2/lib` (`scan_lexical`: chuỗi/comment chưa đóng trong `v2/lib` vẫn ĐỎ
  `E_LEX_*`), **phân giải `include!`/`#[path]` trong `v2/lib`** (`resolve_include_targets`,
  KT-209/L3), và tệp **bị kéo vào** bằng `#[path]`/`include!` từ `v2/kernel`/`v2/sim` (`extra`).

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
