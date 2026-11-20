# Hợp đồng của `check-wake-paths.py` — KT-184 / KT-194 / KT-196 / KT-209 / KT-211 / KT-213 / KT-215 / KT-219 / KT-225 / KT-241 / KT-242 / KT-256

**Trạng thái**: bản cập nhật theo **A0 2026-11-19 §5** (tách bốn loại) + **KT-209** (L40 KT-201
§D2 L2/L3) + **KT-211** (L44 KT-208 §B1/D — quét token toàn cục cho `include!`/`#[path]`) +
**KT-213** (L47 KT-212 — gốc phân giải `#[path]` của tệp non-mod-rs) + **KT-215** (L47 KT-212 —
bộ lọc `target/`, `MAILBOX_DEFS`, đóng F2) + **KT-219** (L49 KT-218 — vùng test quá rộng trên
nhánh `match`, đường lọt #13) + **KT-225** (L52 KT-223 — bốn báo oan `include!` + đường lọt lớp một
`.txt` + răng ca B29/B30) + **KT-241** (D2-194 §3 — API thu hẹp `NonReadyTarget`, mỏ neo kiểu,
luật `write_state`, bỏ tham số `tr` chết; **phân loại hai lệch + ba mục chưa nguồn của KT-240** —
xem §4.1) + **KT-242** (review độc lập `JOURNAL/kt242-wake-review.md`: **sáu** đường lọt mới; vá
**hàm lồng không phải cửa**, **vùng test theo ký tự** cho **mọi** luật, **phạm vi = mẫu ∪ crate
root thật** + đuôi `.rs` không phân biệt hoa/thường, **đường dẫn không phân giải được ⇒ lỗi cổng
rõ** thay vì traceback; số ca selftest của hai lớp tăng — số đo đầy đủ ở
`JOURNAL/kt242-wake-review.md` §7) + **KT-256** (nợ **L2**: **ba** mẫu phạm vi vênh nhau
(`RE_TEST_ONLY_WAKE_SCOPE`, `MACRO_SCAN_ROOTS`, `RE_CRATE_SRC`) **hợp nhất** thành **một** định
nghĩa sinh từ `cargo metadata` — `v2/tools/wake_scope.py`, dùng chung **cả hai** checker; thêm
ba mã lỗi phạm vi `E_SCOPE_METADATA`/`E_SCOPE_REDUCTION`/`E_SCOPE_LIB_CORE`; **DEBT-KT227-1**
đóng bằng ca canh `E_SCOPE_LIB_CORE`; thêm phép kiểm máy `check-no-cfg-test.py` cho giả định
"ảnh sản phẩm **không** có `--cfg test`" — số đo ở `JOURNAL/kt256-wake-debt.md`) + **KT-258**
(vòng tấn công **mới** trên HEAD sau KT-256, trọng tâm **phạm vi**: **ba** đường lọt
WB-21/WB-22/WB-23 — crate trong **bao đóng sản phẩm** ở gốc mới, `include!`/`#[path]` trong crate
đó, và path-dependency **ra ngoài gốc repo**; vá `product_closure()` + `_scope_token_targets()` +
seed worklist + `E_SCOPE_OUTSIDE`; số đo `JOURNAL/kt258-wake-attack.md`). **Chưa** được reviewer
độc lập xác nhận lại trên HEAD sau KT-242/KT-256/KT-258 — KT-184, KT-194, KT-211, KT-213 vẫn
`REVIEW`.

Máy kiểm này **không** phải bộ phân tích Rust tổng quát. Nó là **lexer + token-tree + bộ đọc cấu
trúc tối thiểu**, chạy **không cần biên dịch**, và **fail-closed** ở mọi chỗ nó không phân giải được.

---

## 1. Phạm vi cây nguồn được bảo vệ

**KT-256 (L2, D2-188) — MỘT định nghĩa phạm vi.** Trước KT-256 có **ba** mẫu vênh nhau:
`RE_TEST_ONLY_WAKE_SCOPE` (luật token, lớp tĩnh), `MACRO_SCAN_ROOTS` (thân macro, lớp tĩnh) và
`RE_CRATE_SRC` (dự phòng, lớp dep-info). Nay **cả hai** checker import **cùng** một nguồn:
`v2/tools/wake_scope.py` → `protected_scope(root)`, sinh từ `cargo metadata --no-deps` của
`v2/Cargo.toml` + `v2/kernel/Cargo.toml` + **bao đóng path-dependency nội bộ** (D2-188), phân
loại **bảo vệ** / **từ chối rõ** (`v2/tools/`, nâng lên bảo vệ nếu bao đóng chứa crate bảo vệ).
Ba **view** suy từ cùng một đối tượng:

| View | Nội dung | Dùng cho |
|---|---|---|
| `contains(relp)` | tệp thuộc crate **bảo vệ** ∪ `NON_CRATE_SOURCE_TREES` | `hidden_files` (lớp dep-info), gốc quét thân macro |
| `token_scope(relp)` | `contains` **và** crate thuộc workspace **kernel** (ảnh sản phẩm) **hoặc** crate `vifast-sim` (oracle, tự định nghĩa `fn wake`) **hoặc** cây nguồn sản phẩm không-phải-crate | luật token `wake` (`in_test_only_wake_scope`) |
| `macro_roots()` | `roots()` = thư mục crate bảo vệ ∪ cây nguồn không-phải-crate | `scan_macro_bodies` |
| `product_closure()` | **KT-258 (WB-21)** — `dir_rel` của mọi crate trong **bao đóng sản phẩm**: đi từ crate workspace `kernel` + `vifast-sim` theo `deps` (path-dependency nội bộ, **kể cả** `optional`/build/dev) | `token_scope` (crate trong bao đóng ⇒ luật token áp, **trừ `v2/lib`**) |

**KT-258 (WB-21/WB-22/WB-23) — phạm vi định nghĩa phải bằng phạm vi **quét**.** Ba đường lọt
đo được (đều **biên dịch được** — `cargo check` xanh — **và** cả ba lượt XANH trước vá):

| Mã | Dạng | Gốc | Vá |
|---|---|---|---|
| **WB-21** | crate ở **gốc mới** trong **bao đóng sản phẩm** (path-dep **tuỳ chọn** của `vifast-kbin-riscv64`, chỉ bật bởi `product-fs`) gọi `wake` | `token_scope` trả `False` cho `workspace="closure"`; lớp tĩnh chỉ duyệt `V2_SCAN` | `product_closure()` + `_scope_token_targets()` (duyệt `scope.roots()`) |
| **WB-22** | `include!`/`#[path]` trong crate ở gốc mới | worklist `resolve_include_targets` chỉ seed `V2_SCAN` + `v2/lib` | seed `files + scope_targets`; đích ngoài `v2/kernel`+`v2/sim` ⇒ `E_STRICT_WAKE_INCLUDE` |
| **WB-23** | path-dependency **ra ngoài gốc repo** (tuỳ chọn, chỉ bật bởi `product-fs`) | `_crate_from_package` bỏ **im lặng** package ngoài gốc | `CrateScope.outside_deps` + `check_outside_dependencies()` ⇒ `E_SCOPE_OUTSIDE` / `E_DEPINFO_SCOPE` |

**Hệ quả chính sách**: `v2/lib` **vẫn** ngoài luật token (KT-256), kể cả khi nằm trong bao đóng
sản phẩm — canh bằng `E_SCOPE_LIB_CORE` (mọi đường tới `wake()` từ `v2/lib` phải qua
`vifast-core`). Crate ở gốc mới **không** thuộc bao đóng sản phẩm/oracle (ví dụ dep của một crate
host thường) **không** bị luật token quét — phạm vi không mở rộng mù (ca U3).

**Ngoại lệ có lý do** (`NON_CRATE_SOURCE_TREES`, không phải mẫu đường dẫn):
`v2/kernel/vifast-plat-user` — shim ABI U-mode, **không phải crate**, biên dịch bởi
`vifast-kbin-riscv64/build.rs` và đóng vào ảnh sản phẩm.

**Không giảm phạm vi** — `check_no_reduction(root)` đối chiếu **mọi** tệp `.rs` dưới ba gốc cũ
(`LEGACY_ROOTS` = `v2/lib`, `v2/kernel`, `v2/sim`) với `contains()`; tệp nào ngoài ⇒
`E_SCOPE_REDUCTION`. Đây là **lưới an toàn di trú** (không phải định nghĩa): nếu metadata bỏ sót
một cây nguồn từng được mẫu cũ che, cổng **ĐỎ** thay vì âm thầm hẹp lại.

**Hệ quả đã đo (đều là *mở rộng*, **không** token `wake` nào nên không báo oan)** — xem
`JOURNAL/kt256-wake-debt.md` §2: tệp sản phẩm **ngoài `src/`** của crate ảnh sản phẩm
(`vifast-kbin-riscv64/init/*.rs`, shim `vifast-plat-user/lib.rs`) vào phạm vi luật token; crate
thêm vào workspace ở **thư mục gốc mới** (`v2/drivers/…`) vào phạm vi cả hai lớp; crate
`v2/tools/` **phụ thuộc** crate bảo vệ được **nâng** ở **cả** lớp tĩnh (trước KT-256 chỉ lớp
dep-info nâng).

| Cây | Luật áp |
|---|---|
| `v2/kernel/<crate>/src/**` + `v2/kernel/<crate>/build.rs` | `include!`/`#[path]`/`cfg_attr(path)` literal + trong cây; `wake` chỉ-dùng-test; thân `macro_rules!` (**token `wake`** + `include!`/`#[path]`/`mod`/`:tt`); **KT-241**: mỏ neo kiểu + luật `write_state`/`into_state` + `transition_tcb` không nhận `TcbState` |
| `v2/sim/<crate>/src/**` + `v2/sim/<crate>/build.rs` | như trên |
| **KT-242 (phạm vi crate root)** — **mọi crate root thật** của `v2/kernel/<crate>`/`v2/sim/<crate>`: `[lib] path`, `[[bin]] path` (kể cả **ngoài `src/`**, ví dụ `bin/x.rs`), `src/lib.rs`, `src/main.rs`, `tests/*.rs` | như trên. Trước KT-242 phạm vi là **mẫu** `src/**`+`build.rs`; bin/lib khai báo **ngoài `src/`** là bố cục Rust hợp lệ mà `cargo` biên dịch thật, nhưng **không** khớp mẫu ⇒ luật token `wake` **không** chạy (đo: lớp tĩnh XANH cả hai chế độ, lớp dep-info ĐỎ `E_DEPINFO_WAKE`). Nay phạm vi = **mẫu ∪ crate root thật** (`crate_roots`, đọc `Cargo.toml`) |
| **KT-242 (đuôi hoa/thường)** — tệp đuôi `.rs` **không phân biệt hoa/thường** (`.RS`, `.Rs`) | như trên. Trên macOS/Windows `mod x;` nạp `src/x.RS` (đo: `cargo check` biên dịch được) nhưng `rglob("*.rs")` **bỏ sót** ⇒ luật token `wake` không chạy (lớp tĩnh XANH, lớp dep-info ĐỎ). Nay duyệt bằng `os.scandir` + `suffix.lower() == ".rs"` (`_iter_rs_ci`). **KT-256**: mẫu `RE_TEST_ONLY_WAKE_SCOPE` (mang `re.IGNORECASE`) đã **xoá** — phạm vi nay do `wake_scope` quyết định theo **crate**, đuôi hoa/thường vẫn được `_iter_rs_ci` duyệt |
| `v2/kernel/vifast-core/src/state_write.rs`, `v2/sim/vifast-sim/src/kobj/state_write.rs` | **KT-241**: nơi **duy nhất** được chứa `write_state`/`into_state`; `write_state` chỉ được gọi bởi sáu hàm ở `STATE_WRITE_CALLERS`; là **mỏ neo** của toàn bộ ranh giới API (xem §2b) |
| `v2/lib/**` | thân `macro_rules!` (token `wake`, `include!`, `#[path]`, `mod`, `:tt`); `include!`/`#[path]` **ngoài** macro được phân giải, đích phải ở trong `v2/lib` |
| `v2/test/**`, `v2/agent/**` | **không** bảo vệ (ngoài phạm vi) |
| `v2/tools/**` | **từ chối rõ** (công cụ phát triển, không link vào ảnh sản phẩm) — **trừ** crate `v2/tools/` **phụ thuộc** crate bảo vệ: được **nâng** lên bảo vệ (`_classify_scope`, KT-238). **KT-256**: việc nâng nay áp cho **cả lớp tĩnh** (trước KT-256 chỉ lớp dep-info nâng) — crate công cụ phụ thuộc sản phẩm có thể gọi `wake()` nên thân macro của nó cũng phải bị quét. |

**KT-209/L2** — phạm vi `wake` là **mẫu**, không liệt kê cứng: **mọi** crate dưới
`v2/kernel/<crate>/` và `v2/sim/<crate>/` (toàn bộ `src/**` cộng `build.rs`) nằm trong phạm vi,
kể cả crate ra đời sau. `tests/` cấp crate nằm ngoài (test-only, `is_test_file` miễn).
**Ngoài mẫu** (đo 2026-11-20: **không** có token `wake`): `v2/kernel/vifast-plat-user/lib.rs` (shim
U-mode, **không** phải crate — biên dịch bởi `vifast-kbin-riscv64/build.rs`) và
`v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode, biên dịch bởi `build.rs`).

**`cfg_attr` lồng trong `v2/lib`** (KT-240; **KT-242 đo lại**) — `v2/lib` **không** được quét
**cấu trúc** (`scan_structure` chỉ chạy `v2/kernel`+`v2/sim`). **ĐÃ ĐO** (KT-242, hai ca đối
chứng trong `JOURNAL/kt242-wake-repro/fixture-probes.py`):

- `#[cfg_attr(any(), cfg_attr(any(), path = "evil.rs"))] mod m;` trong `v2/lib` **được phân
  giải** — `resolve_include_targets` seed **mọi** tệp `v2/lib/**/*.rs` và `_walk_all` →
  `attr_path_edges` đệ quy `cfg_attr` **hai tầng**;
- đích **trong** `v2/lib` ⇒ **XANH** (tệp đích vào `extra`, nhưng `v2/lib` **không** thuộc luật
  token `wake` — xem nợ DEBT-KT227-1);
- đích **ra ngoài** `v2/lib` ⇒ **ĐỎ** `E_STRICT_WAKE_INCLUDE` (cả hai chế độ).

⇒ Câu cũ ("`cfg_attr` lồng trong `v2/lib` ⇒ XANH") **chỉ đúng khi đích ở trong `v2/lib`**; khi
đích ra ngoài thì **ĐỎ**. Phần thật sự còn hở là **DEBT-KT227-1** (`v2/lib` ngoài luật token
`wake`), **không** phải `cfg_attr`.

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
- **Ngữ cảnh test theo item/token-tree** (KT-196/F2 + KT-198/F2b + **KT-242**): áp cho **cả**
  đồ thị module (`_mods_of`) **và** **mọi** luật quyết định — luật kênh (`MailboxMsg`), luật
  consumer, luật arch, R2 (`transition_tcb`), R3 (`write_state`), R1 (`wake`) — ở **cả hai** lớp.
  `#[cfg(test)]` chỉ áp cho **mục kế tiếp**, **không** lan sang mục cùng dòng; `mod`/`fn` trong
  khối test lồng vẫn là test.
  **KT-242 (vùng test theo ký tự)** — trước bản này **năm** chỗ vẫn dùng `test_line_ranges`
  (**theo dòng**), nên `#[cfg(test)]\nfn helper() {} fn evil(…) { … }` cho vùng phủ **cả** dòng
  thứ hai ⇒ mã sản phẩm **cùng dòng** một mục test được miễn (đo: luật kênh/consumer/arch XANH;
  xem `JOURNAL/kt242-wake-review.md` §2). Nay **mọi** luật dùng `_test_char_ranges` + **offset ký
  tự**; `test_line_ranges` **không** còn call site sản xuất (có ca selftest chốt).
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
  `#[path = "sub"] mod m { #[path = "y.rs"] mod n; }` ⇒ `sub/y.rs`). **ĐÃ ĐO** (KT-240 N2):
  `rustc` — `src/foo.rs` = `mod inner { include!("x.rs"); }`: `src/x.rs` ⇒ biên dịch **được**;
  `src/foo/x.rs` ⇒ biên dịch **lỗi** `couldn't find file src/x.rs` ⇒ `include!` trong **inline module**
  giải theo **thư mục tệp chứa**, khớp khẳng định. Log:
  `JOURNAL/kt241-logs/n2-include-inline-module.log`. **Bỏ qua** thân
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

## 2b. **Hợp đồng KT-241 — API thu hẹp `NonReadyTarget`** (D2-194 §3)

**Vì sao đổi**: trước KT-241, `transition_tcb(tcb, to: TcbState)` nhận **mọi** trạng thái, nên
checker phải **suy luận** "đối số thứ hai có phải `Ready` không" (allowlist `NON_WAKE_READY_SITES`
+ dataflow-lite `_var_may_be_ready`). Cách đó vừa **hở** (WB-16/WB-17: helper/const/closure/
fn-pointer/UFCS — KT-234/KT-232) vừa **báo oan** (phải miễn mười lăm chỗ). KT-241 **thu hẹp API**:
`transition_tcb` nhận `NonReadyTarget` (enum mười biến thể, **không có `Ready`**), primitive ghi thô
`write_state` thành **private** trong module `state_write`, và `Ready` chỉ vào được qua **năm cửa
ngữ nghĩa**. Hệ quả: **hệ thống kiểu** đóng WB-16/WB-17 (`E0308`/`E0631`), checker chỉ còn phải
**bảo vệ ranh giới API** — không mô phỏng lại suy luận kiểu.

### 2b.1 Năm cửa `Ready` + cửa thông thường (API-SPEC §3)

| Nhóm | Cửa | Mức | Điều kiện nguồn | Ghi |
|---|---|---|---|---|
| thường | `transition_tcb(tcb, to: NonReadyTarget)` | `pub` | bảng `ALLOWED` trong `write_state` | cửa thông thường **duy nhất** |
| — | `write_state(tcb, to: TcbState)` | **private** | bảng `ALLOWED` + hook KT-21/KT-34 + bất biến `Running` | nguyên thuỷ thô |
| wake | `wake_step(tcb, unlink)` | `pub(crate)` | `Ready` ⇒ idempotent; `is_blocked()`/`Sleeping` ⇒ đi; còn lại ⇒ `IllegalOperation` | wake |
| preemption | `preempt_current(tcb)` | `pub(crate)` | `Running{hart}` **và** `sched[hart].current() == Some(tcb)` | preemption |
| yield nội bộ | `yield_to_ready(tcb)` | `pub(crate)` | như cửa preemption | yield (nội bộ) |
| yield ngoài | `tcb_yield(tcb)` | `pub` | gọi `yield_to_ready` (điều kiện do cửa yield nội bộ kiểm) | yield (crate ngoài) |
| khởi tạo | `start_tcb(tcb)` | `pub` | `Inactive` | khởi tạo |
| suspend | `suspend_to_ready(tcb)` | `pub(crate)` | đọc lại: `is_blocked()` hoặc `Running{..}` | suspend/abort |

**`STATE_WRITE_CALLERS`** (hàm được gọi **trực tiếp** `write_state` trong `state_write.rs`):
`transition_tcb` (cửa thường) + `wake_step` + `preempt_current` + `yield_to_ready` + `start_tcb` +
`suspend_to_ready` = **sáu**. `tcb_yield` **không** có mặt: nó **phải** đi qua `yield_to_ready`
(có kiểm điều kiện nguồn); gọi thẳng `write_state` là **bỏ kiểm** ⇒ ĐỎ.

### 2b.2 Luật thay thế trực tiếp `E_STRICT_TRANSITION` cũ

| Luật | Mã | Điều kiện | Vì sao |
|---|---|---|---|
| `transition_tcb` **đảo kiểu** | `E_STRICT_TRANSITION` | đối số thứ hai là/chứa `TcbState` **hoặc** kết thúc bằng/chứa `…::Ready` (alias `S::Ready`, `Self::Ready`, `NonReadyTarget::Ready`) | dưới API mới dạng này **không biên dịch được** ⇒ có người đã đảo `to: NonReadyTarget` về `to: TcbState`. Đây **không** còn là "phân tích Ready/không-Ready" — nó là **chốt chống đảo kiểu**. Không còn cửa/allowlist nào được miễn. |
| `write_state`/`into_state` **ngoài** `state_write.rs` | `E_STRICT_STATE_WRITE` | mọi xuất hiện (gọi, dùng làm giá trị, định nghĩa) ở tệp khác — **trừ** mã trong `#[cfg(test)]` (cùng quy ước miễn của mọi luật khác; test **không thể** gọi `write_state` vì private, nên miễn không tạo lỗ hổng) | primitive ghi thô là **private**; không tệp nào khác được chạm. Đây là **luật thay thế** cho allowlist `NON_WAKE_READY_SITES` (đã **xoá**: dưới API mới không còn chỗ `transition_tcb(…, Ready)` hợp lệ để miễn). |
| `write_state` **sai cửa** trong `state_write.rs` | `E_STRICT_STATE_WRITE` | gọi từ hàm **không** thuộc `STATE_WRITE_CALLERS`; hoặc dùng làm **giá trị** (`let f = Self::write_state;`) | mỗi cửa có điều kiện nguồn riêng; một wrapper/hàm thứ bảy là **bỏ kiểm** (D2-194 §2). `tcb_yield` là ca cụ thể: phải qua `yield_to_ready`. |
| **KT-242 (hàm lồng)** — `write_state` trong `fn`/`impl`/`trait`/`mod` **lồng trong thân** một cửa | `E_STRICT_STATE_WRITE` | "hàm gọi" = **hàm trong cùng theo token-tree** (`_innermost_fn_tree`), **không** phải hàm theo dòng/thụt lề (`enclosing`/`find_functions`). Hàm lồng **không** thuộc `STATE_WRITE_CALLERS` ⇒ ĐỎ | đường lọt "cửa thứ bảy": `impl Machine { pub fn kt242_door(…) { self.write_state(tcb, TcbState::Ready) } }` lồng trong `wake_step`; mã sản phẩm gọi `m.kt242_door(t)` ⇒ TCB về `Ready` **không qua điều kiện nguồn nào**. Trước KT-242: `cargo check` biên dịch được, lớp tĩnh thường/`--strict`/lớp dep-info **đều XANH** (đo: `JOURNAL/kt242-wake-repro/logs/r3a/`). Gốc: `find_functions` **nhảy qua** hàm lồng nên `enclosing` trả **cửa ngoài**. |
| **KT-252 (trùng tên)** — `fn wake_step` (cùng tên sáu cửa) ở `impl` **kiểu khác** / **module con** / **trait impl** / **free fn** trong `state_write.rs` | `E_STRICT_STATE_WRITE` | "hàm gọi" phải là method của **cùng kiểu** với `write_state` (`_enclosing_impl_type` = kiểu của `impl` chứa **định nghĩa** `write_state`); **tên khớp không đủ** | đường lọt **WB-20**: bốn hình dạng đều `cargo check` **thành công** và **cả ba lớp XANH** trước bản vá; sau vá **ĐỎ cả ba lớp**. Đo + vá: `JOURNAL/kt252-wake-attack.md` §2–§3 |
| `into_state` dùng làm giá trị trong `state_write.rs` | `E_STRICT_STATE_WRITE` | không phải định nghĩa/call | giữ ranh giới "chỉ gọi, không lấy hàm" |

**Bỏ hẳn**: `NON_WAKE_READY_SITES` (mười lăm mục) và `_var_may_be_ready`. Lý do: câu hỏi "biến này có
thể là `Ready` không" **không còn nghĩa** khi kiểu đã loại trừ `Ready`; giữ lại là giữ một mô
hình dataflow-lite **không đầy đủ** (không mô hình hoá call/const/closure/liên hàm) làm người đọc
tưởng checker chứng minh được nhiều hơn thực tế. `E_STRICT_STALE` **giữ** nhưng đổi đối tượng:
mục `extra_doors` (chỉ selftest dùng) không còn khớp ⇒ ĐỎ.

### 2b.3 Mỏ neo kiểu (fail-closed) — `_check_type_anchors`

**Nguyên tắc D2-194 §3**: *đơn giản hoá theo kiểu, **không tin tên hàm***. Nhưng **không** được
thay bằng "thấy `transition_tcb(` là mặc nhiên an toàn": nếu ai đó **đảo kiểu** thì đường lọt cũ
sống dậy mà checker vẫn XANH. Vì vậy checker đọc **định nghĩa** và **ĐỎ khi thiếu/sai**:

| # | Mỏ neo | Mã khi vi phạm |
|---|---|---|
| một | `state_write.rs` **tồn tại** (core + sim) | `E_ANCHOR_MISSING` |
| hai | `enum NonReadyTarget` tồn tại và **không** có biến thể `Ready` | `E_ANCHOR_MISSING` / `E_ANCHOR_TYPE` |
| ba | `transition_tcb` có tham số thứ hai kiểu `NonReadyTarget` (không `TcbState`) | `E_ANCHOR_MISSING` / `E_ANCHOR_TYPE` |
| bốn | `block_current` có tham số thứ hai kiểu `NonReadyTarget` | `E_ANCHOR_MISSING` / `E_ANCHOR_TYPE` |
| năm | `write_state` và `into_state` là `fn` **private** (không `pub`, `pub(crate)`, `pub(super)`, `pub(in …)`) | `E_ANCHOR_MISSING` / **`E_ANCHOR_API`** |
| sáu | `state_write.rs` có đủ năm cửa `Ready` + `tcb_yield` + `transition_tcb` | `E_ANCHOR_MISSING` |
| bảy | `core_tcb.rs`/`kobj.rs` **không** còn `fn transition_tcb(` (cửa cũ đã chuyển đi) | `E_ANCHOR_TYPE` |
| tám | không `impl From/TryFrom/Into<TcbState> for NonReadyTarget`; không `pub fn` nào nhận `TcbState` trả `NonReadyTarget` | **`E_STRICT_NONREADY_CONV`** (xem §2b.5) |

"Tham số thứ hai" = tham số thứ hai **của lời gọi** (`transition_tcb(tcb, to)`), tức tham số thứ
ba của **method** (`&mut self, tcb, to`). `self`/`&self`/`&mut self`/`self: …` bị **loại** khỏi
danh sách tham số khi kiểm mỏ neo (nếu không, mỏ neo đọc `tcb: ObjId` và **ĐỎ oan** mọi method).

### 2b.4 Tham số `tr` chết — **bỏ**

`_check_wake_text(relp, text, tr, safe_macros, r)` nhận `tr = test_line_ranges(text)` nhưng
**bỏ qua** và tự tính `_test_char_ranges(code, relp)` (phân vùng theo **token-tree**). Hai cách
phân vùng khác nhau ⇒ người đọc tưởng `test_line_ranges` còn hiệu lực trong luật `wake`, và một
caller mới có thể tưởng mình đang điều khiển vùng test. Nay **một** nơi sở hữu phân vùng: bỏ tham
số `tr` khỏi chữ ký và khỏi **cả bốn** call site (lớp một ×ba + lớp hai ×một). Ca selftest "KT-241 phân
vùng" dựng đúng chỗ hai cách phân vùng **khác nhau** (`#[cfg(test)] mod tests { … }` cùng dòng,
`pub fn evil(…) { m.wake(t); }` dòng sau: `test_line_ranges` phủ **cả** dòng `evil`,
`_test_char_ranges` chỉ phủ mục `mod tests`) và khẳng định kết quả theo cách của **callee** (ĐỎ).

---

### 2b.5 Luật `E_STRICT_NONREADY_CONV` — cấm phép chuyển `TcbState` ⇄ `NonReadyTarget`

**Vì sao cần luật riêng** (không chỉ dựa vào mỏ neo chữ ký): orphan rule **cho phép**
`impl From<TcbState> for NonReadyTarget` **bên trong** `vifast-core` (cả hai kiểu đều local) ⇒
`cargo check` **thành công** ⇒ `NonReadyTarget::from(TcbState::Ready)` / `TcbState::Ready.into()` biên
dịch được ⇒ **đường vòng tái tạo** qua ranh giới kiểu mà **không** đụng `transition_tcb`.
API-SPEC §1 nói rõ: "**Không** có `From`/`TryFrom` công khai nào từ `TcbState` sang kiểu này —
đó là toàn bộ ranh giới."

**Phạm vi**: `v2/kernel/**` + `v2/sim/**` + **`v2/lib/**`** (rộng hơn phạm vi luật `wake` — impl
có thể nằm ở crate lib nếu sau này lib phụ thuộc core; quét sẵn là fail-closed, không báo oan vì
`v2/lib` hiện không có hai kiểu này).

| Dạng | Ví dụ | Mã |
|---|---|---|
| trait impl có **cả hai** kiểu trong đầu `impl` | `impl From<TcbState> for NonReadyTarget`, `impl TryFrom<TcbState> for NonReadyTarget`, `impl Into<NonReadyTarget> for TcbState` | `E_STRICT_NONREADY_CONV` |
| inherent impl trên **đích**, method nhận **nguồn** trả `Self` | `impl NonReadyTarget { fn from_state(s: TcbState) -> Self }` | `E_STRICT_NONREADY_CONV` |
| inherent impl trên **nguồn**, method trả **đích** | `impl TcbState { fn to_target(self) -> NonReadyTarget }` | `E_STRICT_NONREADY_CONV` |
| **hàm công khai** (free fn hoặc trong impl) nhận `TcbState` trả `NonReadyTarget` | `pub fn tu_state(s: TcbState) -> NonReadyTarget` | `E_STRICT_NONREADY_CONV` |

Lớp dep-info áp **cùng** hàm (`CWP._check_nonready_conv_file`) trên **mọi** tệp rustc nạp, mã
`E_DEPINFO_NONREADY_CONV`.

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
| **KT-241** — `transition_tcb(…)` với đối số thứ hai là/chứa `TcbState` hoặc `…::Ready` | `E_STRICT_TRANSITION` | dưới API `to: NonReadyTarget` dạng này **không biên dịch được** ⇒ đảo kiểu (D2-194 §3) |
| **KT-241** — `write_state`/`into_state` xuất hiện **ngoài** `state_write.rs` | `E_STRICT_STATE_WRITE` | primitive ghi thô là private; luật thay thế `NON_WAKE_READY_SITES` |
| **KT-241** — `write_state` gọi từ hàm **không** thuộc `STATE_WRITE_CALLERS` (kể cả `tcb_yield`), hoặc dùng làm giá trị | `E_STRICT_STATE_WRITE` | mỗi cửa có điều kiện nguồn riêng; wrapper/hàm thứ bảy là bỏ kiểm (D2-194 §2) |
| **KT-241** — mỏ neo kiểu **thiếu** (`state_write.rs`, `enum NonReadyTarget`, `transition_tcb`, `block_current`, `write_state`, `into_state`, cửa) | `E_ANCHOR_MISSING` | fail-closed: thiếu định nghĩa **không** được coi là "không có gì để kiểm" |
| **KT-241** — mỏ neo kiểu **sai** (`NonReadyTarget` có `Ready`; chữ ký `transition_tcb`/`block_current` là `TcbState`; `core_tcb.rs`/`kobj.rs` còn `fn transition_tcb`) | `E_ANCHOR_TYPE` | API bị **đảo** ⇒ đường lọt cũ sống dậy (D2-194 §3) |
| **KT-241 (E5)** — `write_state`/`into_state` bị nới thành `pub`/`pub(crate)`/`pub(super)`/`pub(in …)` | `E_ANCHOR_API` | primitive ghi thô phải **private**; nới ⇒ mọi module trong crate gọi được (D2-187 §3) |
| **KT-241 (E1/E2)** — mọi phép chuyển `TcbState` ⇄ `NonReadyTarget` ngoài `into_state` private: `impl From`/`TryFrom`/`Into`, inherent impl `NonReadyTarget { fn …(TcbState) -> Self }`, inherent impl `TcbState { fn … -> NonReadyTarget }`, `pub fn` nhận `TcbState` trả `NonReadyTarget` | `E_STRICT_NONREADY_CONV` | orphan rule cho phép impl **bên trong** crate ⇒ `cargo check` thành công ⇒ đường vòng tái tạo (API-SPEC §1) |
| **KT-256** — không dựng được phạm vi từ `cargo metadata` trên cây **có** workspace manifest (không có `cargo`, metadata RC≠0, không crate nào) | `E_SCOPE_METADATA` | D2-188: phạm vi **phải** sinh từ metadata; quét-rồi-PASS với phạm vi rỗng là fail-open |
| **KT-256** — tệp `.rs` dưới ba gốc cũ (`v2/lib`, `v2/kernel`, `v2/sim`) **ngoài** phạm vi metadata | `E_SCOPE_REDUCTION` | lưới an toàn di trú: định nghĩa mới **không** được hẹp hơn ba mẫu cũ ở bất kỳ đâu |
| **KT-258 / WB-23** — path-dependency trỏ **ra ngoài gốc repo** (không quét được) | `E_SCOPE_OUTSIDE` (lớp tĩnh) / `E_DEPINFO_SCOPE` (lớp dep-info) | D2-188 §2: nguồn ngoài hợp đồng phải **từ chối rõ**, không được bỏ im lặng khỏi phạm vi |
| **KT-256 / DEBT-KT227-1** — crate `v2/lib` có `vifast-core` trong **bao đóng normal dependency** | `E_SCOPE_LIB_CORE` | `v2/lib` ngoài luật token `wake`; điều đó chỉ an toàn khi `wake()` **không gọi được** từ mã sản phẩm `v2/lib`. Có phụ thuộc ⇒ **mở rộng luật** (hoặc gỡ phụ thuộc) |
| Chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá độ sâu | `E_LEX_*` | D2-166 §3C |

---

## 4. **Sai lệch implementation còn mở** (đã biết, chưa đóng)

| Mã | Mô tả | Ảnh hưởng |
|---|---|---|
| ~~**Allowlist `NON_WAKE_READY_SITES` + dataflow-lite `_var_may_be_ready`**~~ | **ĐÓNG (KT-241, D2-194 §3)**: hai cơ chế này tồn tại vì `transition_tcb` nhận `TcbState` nên checker phải **suy luận** "đối số thứ hai có phải `Ready` không". Chúng vừa **hở** (WB-16/WB-17: helper/const/closure/fn-pointer/UFCS — KT-234/KT-232) vừa **báo oan** (mười lăm mục allowlist). KT-241 thu hẹp API (`to: NonReadyTarget`, enum **không có** `Ready`; `write_state` private; năm cửa ngữ nghĩa) ⇒ **bỏ** cả hai, thay bằng: (i) `E_STRICT_TRANSITION` chỉ còn là **chốt chống đảo kiểu** (`TcbState::*`/`…::Ready` làm đối số); (ii) `E_STRICT_STATE_WRITE` (`write_state`/`into_state` chỉ sống trong `state_write.rs`, chỉ sáu hàm được gọi trực tiếp); (iii) **mỏ neo kiểu** `E_ANCHOR_MISSING`/`E_ANCHOR_TYPE` (§2b.3). Selftest: cả hai lớp đều **tăng** số ca và **xanh** (số đo: `JOURNAL/kt241-checker.md`). Số đo: `JOURNAL/kt241-checker.md`. | **Đã vá**; cây thật **ĐỎ có chủ đích** cho tới khi A/B hợp nhất API (mỏ neo `state_write.rs` chưa tồn tại + mọi call-site còn `TcbState::*`). Cây fixture hậu hợp nhất (`/tmp/kt241-fixture`) **XANH** cả hai lớp, cả hai chế độ. |
| ~~**Alias kiểu che `transition_tcb(…, Ready)`**~~ | **ĐÓNG (KT-231, reviewer L54 — đường lọt #15)**: luật KT-155 cài bằng **regex văn bản** `transition_tcb\s*\([^;]*?TcbState::Ready` trên **một dòng**; `use crate::TcbState as S;` + `m.transition_tcb(t, S::Ready)` **XANH cả hai lớp** trong khi `cargo check` biên dịch được (biến thể `use …::TcbState::Ready as R;` thì bị bắt nhờ fail-closed "`TcbState::Ready` ngoài hàm" trên dòng `use`). Nay `_transition_calls` quét **token-tree**: đối số thứ hai phải **chứng minh** được — literal `TcbState::Ready` (kể cả path-qualified) ⇒ cửa/allowlist; literal `TcbState::<BiếnThểKhác>` ⇒ chứng minh không Ready ⇒ XANH; alias `S::Ready`/`Self::Ready`/đường dẫn lạ ⇒ **ĐỎ**; biến ⇒ ĐỎ nếu hàm gán từ `TcbState::Ready` (dataflow-lite, tránh báo oan `block_current(cur, to)`); dạng khác ⇒ **ĐỎ**. Luật chạy ở **mọi** chế độ (lead KT-231 yêu cầu repro ĐỎ cả lớp một thường); lớp hai dùng **cùng** bộ quét (import, không chép). Đối chứng âm: dùng thẳng `TcbState::Ready` ⇒ vẫn **ĐỎ**. Đối chứng dương: cửa wake, allowlist, `TcbState::<BiếnThểKhác>`, biến tham số ⇒ **XANH**. Selftest: cả hai lớp thêm ca (số đo trong journal). | **Đã vá**; cây thật xanh cả hai chế độ, `--selftest` xanh toàn bộ. Số đo: `JOURNAL/kt231-transition-alias.md`. |
| ~~**Macro `macro_rules!` LỒNG che tên macro "an toàn"**~~ | **ĐÓNG (KT-230, reviewer L54 — đường lọt #14)**: `_shadowed_safe_macros` (dùng chung bởi **cả hai** lớp) chỉ thấy `macro_rules!` **mức mục** vì `_walk_items` **bỏ qua thân macro**. Một `macro_rules! matches` định nghĩa **lồng trong thân** `macro_rules!` khác (rồi gọi macro ngoài để expansion sinh định nghĩa) **không** bị thấy ⇒ tên `matches` vẫn "an toàn" ⇒ `let wake = 0u32;` (miễn local-binding KT-171 B10) + `vifast_abi::matches!(m, wake, t)` (token `wake` là đối số macro "an toàn") ⇒ **XANH cả ba** (lớp một thường, lớp một `--strict`, lớp hai) trong khi `cargo check` **thành công** và macro mở rộng `$m.$x($t)` thành **`m.wake(t)` thật**. Repro: `v2/lib/vifast-abi/src/kt229_nested.rs` (macro lồng) + `v2/kernel/vifast-core/src/kt229_b2.rs` (lời gọi) — log `JOURNAL/kt230-logs/`. Nay: (i) `_collect_safe_macro_defs` đệ quy **mọi** tầng nhóm (thân macro, đối số lời gọi, mọi delimiter) tìm `macro_rules! <tên-an-toàn>`; (ii) lớp một nhận thêm `extra_files` (đích `include!`/`#[path]`, kể cả tệp **không `.rs`**) cho `_shadowed_safe_macros`; (iii) đích `v2/lib` **vào `extra`** (trước đây cố ý bỏ — lý do "v2/lib không có `Machine`" đã sai: tệp đích có thể chứa token `wake` hoặc định nghĩa macro che tên an toàn). Đối chứng âm: bỏ `let wake = …` ⇒ vẫn **ĐỎ** cả ba. Đối chứng dương: `matches!(t, wake)` macro **thật** + `let wake` ⇒ **XANH** (giữ ca KT-171 B10); `stringify!`/`concat!`/`assert!` hợp lệ ⇒ **XANH**. Selftest: cả hai lớp thêm ca (số đo trong journal). | **Đã vá**; cây thật xanh cả hai chế độ, `--selftest` xanh toàn bộ. Số đo: `JOURNAL/kt230-nested-macro.md`. |
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
| **Tệp `.rs` không thuộc crate** | `v2/kernel/vifast-plat-user/lib.rs` (shim U-mode) và `v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode) **không** khớp mẫu `<crate>/src/**`/`build.rs` ⇒ không bị luật `wake` quét **trực tiếp** (một số tệp `init/*.rs` được kéo vào qua `#[path]` từ `src/` và xử lý như đích `extra`). | **ĐÃ ĐO LẠI** (KT-240 N1): `grep` token `wake` trên `vifast-plat-user/lib.rs` + **hai mươi lăm** tệp `init/*.rs` ⇒ **không dòng nào khớp**; đối chứng trên `core_sched.rs` **có** khớp. Log: `JOURNAL/kt241-logs/n1-wake-ngoai-mau.log` (HEAD `c5a83552`). **Còn phải đo lại trên HEAD cuối** sau hợp nhất. Ghi để không thành "phát hiện mới". |
| **F3 (v2/lib)** | `cfg_attr` lồng nhiều tầng trong `v2/lib` **ngoài** thân macro: đích **trong** `v2/lib` ⇒ XANH; đích **ra ngoài** ⇒ ĐỎ `E_STRICT_WAKE_INCLUDE`. | **ĐÃ ĐO** (KT-242, hai ca đối chứng): `resolve_include_targets` seed **mọi** tệp `v2/lib/**/*.rs` và `attr_path_edges` đệ quy `cfg_attr` **hai tầng** ⇒ câu cũ "⇒ XANH" **chỉ đúng khi đích ở trong `v2/lib`**. Phần thật sự còn hở là **DEBT-KT227-1** (`v2/lib` ngoài luật token `wake`), **không** phải `cfg_attr`. |
| **Báo oan có trước** (không phải hồi quy — đối chiếu checker `0fa0fe65` giống hệt) | `#[cfg(test)] include!`; `#[cfg(feature = "ktest")] fn` **inline** (chuỗi `"ktest"` bị `strip_rust_noise` xoá); `#[cfg(test)] const/static`; `macro_rules!` thân `()`/`[]`; block statement; match arm pattern literal. **KT-211** (reviewer L44 §D6) đo thêm **bốn** dạng **ĐỎ** (`--strict`) mà danh sách **không** có: (a) `#[cfg_attr(test, path = "evil.rs")] mod m;` (đích có `wake`) ⇒ **ĐỎ**; (b) `if let Some(wake) = o { wake }` ⇒ **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; (c) `let g = \|wake: u32\| wake + <số>;` ⇒ **ĐỎ**; (d) `struct S { pub wake: u32 }` + `s.wake` ⇒ **ĐỎ**. **KT-225** (L52 KT-223 §C) **đã vá bốn** dạng báo oan của luật `include!`: `stringify!(include!)` (nay **XANH** — `SAFE_TOKEN_MACROS`), `include` + `!=` / `self.include` + `!=` (nay **XANH** — `!` + `=` không phải macro), `include!("x",)` (nay **XANH** — trailing comma hợp lệ). | Cây thật **không** có dạng nào (mọi chỗ `ktest` đều không chứa `wake`); ghi để không thành "phát hiện mới". |
| ~~**Thân `macro_rules!` trong tệp không phải `.rs`**~~ | **ĐÓNG (KT-225, L52 KT-223 — đường lọt lớp một)**: `scan_macro_bodies` chỉ `rglob("*.rs")`, nên `macro_rules! evil { () => { include!("…") } }` trong `src/macro_def.txt` (được `include!("macro_def.txt")` ở vị trí item) **không** bị `_check_macro_body` quét; `_walk_all` bỏ qua thân macro ⇒ checker tĩnh **XANH cả hai chế độ** trong khi `cargo check` thành công và mã sản phẩm gọi `wake` (lớp hai `check-wake-depinfo.py` bắt được). Nay `scan_macro_bodies` nhận `extra_files` = mọi tệp được `include!`/`#[path]` vào cây (không chỉ `.rs`) ⇒ quét thân macro ⇒ **ĐỎ** `E_STRICT_MACRO_INCLUDE`. Ca selftest KT-225 TXT. | **Đã vá**; cây thật xanh cả hai chế độ. |
| **Luật cấu trúc cho crate ngoài `V2_SCAN`** (KT-258, mở) | WB-22 mở **worklist `include!`/`#[path]`** cho crate trong bao đóng sản phẩm; `scan_lexical`/`scan_structure`/`mod` graph **vẫn** chỉ chạy `v2/kernel`+`v2/sim` (+`v2/lib` cho lexical). Hệ quả: crate trong bao đóng sản phẩm có **lỗi từ vựng** hoặc **`mod` graph lạ** không bị lớp tĩnh bắt; lớp dep-info bắt nếu cấu hình build đang kiểm **có** build nó. | **Chưa vá** — cần mở rộng `scan_structure` + `_inside_scan_tree` theo phạm vi. Ghi nợ có tên. |
| **Lớp dep-info chỉ kiểm feature mặc định** (KT-258, mở) | `cargo check --workspace` không bật `product-fs`; mã chỉ tồn tại dưới feature của ảnh sản phẩm **không** được lớp dep-info thấy. Với luật `wake` đã **bù** bằng lớp tĩnh (quét **mọi** tệp trong phạm vi, không phụ thuộc feature) sau WB-21/WB-22; R2/R3/R4 vẫn chỉ thấy cấu hình mặc định. | **Chưa vá** — cần thêm lượt `cargo check --features product-fs` (~+40 s). |
| **Macro ngoài `MACRO_SCAN_ROOTS`** (KT-181) | `v2/tools/**` (công cụ phát triển, **không** link vào ảnh sản phẩm) và `v2/test/fixtures/**` (fixture **cố tình sai**) **không** được quét thân macro. | Không phải mã sản phẩm; cây thật **không** có `macro_rules!` ở hai gốc này. Nếu tương lai có mã sản phẩm ở đó, phải thêm gốc vào `MACRO_SCAN_ROOTS` — **không** được coi là đã bảo vệ. |
| **Không có đường lọt thật cho `wake` ngoài phạm vi cũ** (KT-181 — ghi để **không thổi phồng**) | Trước KT-181, macro ở `init/`/`tests/`/`vifast-plat-user/` chứa token `wake` **không** bị luật token bắt. Nhưng **không** dựng được đường lọt: `init/`+`vifast-plat-user/` biên dịch bằng `rustc` trực tiếp **không** `--extern vifast_core`; `tests/` không phải sản phẩm; nạp bằng `#[path]`/`include!` từ kernel thì `extra` đã quét (đột biến M1c: `cargo check` biên dịch được nhưng checker **ĐỎ**). | Đây là **phòng ngừa**, không phải lỗ hổng đã bị khai thác. |

---

### 4.1 **Phân loại phát hiện của KT-240** (hai lệch hợp đồng + ba mục chưa nguồn)

KT-240 (`JOURNAL/kt240-audit-matrix.md` §2) audit sáu mươi tám khẳng định của hợp đồng: **sáu mươi ba khớp /
hai lệch / ba không nguồn / không mục nào không chứng cứ**. KT-241 **phân loại và xử lý** từng mục:

| # | Mục KT-240 | Loại | Ảnh hưởng | Xử lý trong KT-241 |
|---|---|---|---|---|
| **L1** | §1/§6 câu "**Cả ba chế độ** chạy trong `v2 check`" gây hiểu "ba lớp độc lập"; D2-189 §3 đính chính: thường/`--strict`/dep-info là **ba lượt kiểm thuộc hai cơ chế** | **thuần sổ sách** (thuật ngữ) | Không ảnh hưởng luật wake hay phạm vi bảo vệ — hành vi `v2/bin/v2:344-361` đúng (ba `run_step` + hai selftest) | **Sửa tài liệu ngay**: §6 dưới đây ghi rõ "ba **lượt** kiểm thuộc **hai** cơ chế (lớp tĩnh + lớp dep-info)". |
| **L2** | §4 dòng `:expr` nói "chỉ còn hở khi macro được gọi từ tệp ngoài `MACRO_SCAN_ROOTS`", nhưng §1 ghi `v2/tools/**` ngoài phạm vi trong khi depinfo `RE_CRATE_SRC` **gồm** `v2/tools` | **ảnh hưởng phạm vi bảo vệ** — **thuộc nhóm A**, phải xử lý **trước nghiệm thu** (reviewer E) | Ba mẫu phạm vi **vênh nhau**: `in_test_only_wake_scope` (lớp tĩnh, chỉ `v2/(kernel|sim)/…`), `RE_CRATE_SRC` (lớp dep-info, gồm `v2/lib|tools`), `MACRO_SCAN_ROOTS` (chỉ `v2/lib|kernel|sim`). Code hiện **fail-closed** ở chỗ quan trọng (lớp dep-info nâng crate `v2/tools/` phụ thuộc crate bảo vệ lên **bảo vệ**, `_classify_scope` KT-238), nhưng **ba mẫu** vẫn là mô tả không thống nhất. | **KT-241 sửa tài liệu** (§1 tách "ngoài phạm vi **tĩnh**", ghi rõ cơ chế **nâng** của lớp dep-info) — **không** đổi code. **Việc thống nhất ba mẫu phạm vi là của nhóm A** (reviewer E chỉ định); **không** tính là đóng cho tới khi A xử lý. |
| **N1** | §1 KT-209/L2 "đo 2026-11-20: **không** có token `wake`" ở `vifast-plat-user/lib.rs` + `vifast-kbin-riscv64/init/*.rs` | **chưa có nguồn** → **đã đo lại** | Kỹ thuật đúng (hai đường này không bị luật `wake` quét trực tiếp), nhưng **không có log đo** trong `JOURNAL/kt209-logs/` | **Đã đo lại** trên HEAD `c5a83552`: `grep -nE '(^|[^A-Za-z0-9_])wake([^A-Za-z0-9_]|$)'` trên `vifast-plat-user/lib.rs` + **hai mươi lăm** tệp `init/*.rs` ⇒ **không dòng nào khớp**; đối chứng cùng lệnh trên `core_sched.rs` **có** khớp (lệnh đúng). Log: `JOURNAL/kt241-logs/n1-wake-ngoai-mau.log`. **Còn phải đo lại trên HEAD cuối** sau khi A/B hợp nhất — chưa tính là đóng hẳn. |
| **N2** | §2 "`#[path]` phân giải theo thư mục tệp chứa (đúng rustc: `mod m { include!… }`)" | **chưa có nguồn** → **đã đo** | `kt212-attack-path-mod.md` §3 chỉ đo `#[path]`, **không** đo riêng `include!` trong inline module | **Đã đo bằng `rustc`**: `src/foo.rs` = `mod inner { include!("x.rs"); }` — Case A (`src/x.rs`) biên dịch **được**; Case B (`src/foo/x.rs`) biên dịch **lỗi** `couldn't find file src/x.rs` ⇒ `include!` trong inline module giải theo **thư mục tệp chứa**, khớp khẳng định. Log: `JOURNAL/kt241-logs/n2-include-inline-module.log`. **Đóng N2**. |
| **N3** | §4 "F3 (`v2/lib`): `cfg_attr` lồng nhiều tầng ngoài thân macro ⇒ XANH ... **§1 đã ghi**" | **chưa có nguồn** (tham chiếu vòng sai mục) | §1 **không** ghi trường hợp này; nội dung kỹ thuật (không quét cấu trúc `v2/lib`) đúng | **Ghi "chưa được chứng minh"** + **sửa tham chiếu** (§4 mục F3) + **thêm bullet §1** nói rõ `cfg_attr` lồng ngoài thân macro trong `v2/lib` **không** được quét cấu trúc (hệ quả phạm vi, **không** phải khẳng định đã đo). |

**Tổng kết phân loại**: **một lệch thuần sổ sách** (L1 → sửa tài liệu), **một lệch ảnh hưởng phạm vi
bảo vệ** (L2 → sửa tài liệu cho khớp code đúng, không đổi code), **ba mục chưa nguồn** (N1/N2/N3 →
ghi **"chưa được chứng minh"**, **không** tính là đạt). Không có mục nào đòi nới ngưỡng hay bỏ ca.

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
python3 v2/tools/check-wake-paths.py --strict   # + luật token `wake` đầy đủ
python3 v2/tools/check-wake-paths.py --selftest # số ca hiện tại (KT-241: xem journal)
python3 v2/tools/check-wake-depinfo.py --selftest # lớp dep-info (KT-241: xem journal)

# KT-256 — phép kiểm máy cho giả định "ảnh sản phẩm KHÔNG có --cfg test"
python3 v2/tools/check-no-cfg-test.py            # build thật ảnh sản phẩm (--features product-fs) + đọc `cargo build -v`
python3 v2/tools/check-no-cfg-test.py --log F    # kiểm lại log đã lưu (tái dùng bằng chứng)
python3 v2/tools/check-no-cfg-test.py --selftest # các ca trên log tổng hợp (không cần cargo)
python3 v2/tools/check-no-cfg-test.py --selftest-live  # ca âm end-to-end: RUSTFLAGS=--cfg test ⇒ ĐỎ
```

**KT-256 — chưa nối vào `v2/bin/v2`**: `check-no-cfg-test.py` chạy **độc lập** (build thật ~17 s
trên máy đo). Nối thành bước cổng cần sửa `v2/bin/v2` — **ngoài quyền** của task KT-256; đề xuất
ở `JOURNAL/kt256-wake-debt.md` §5.

**KT-241 sửa thuật ngữ (KT-240 L1)**: đây là **ba lượt kiểm thuộc HAI cơ chế**, không phải ba
lớp độc lập (D2-189 §3): (i) **lớp tĩnh** `check-wake-paths.py` chạy **hai lượt** — chế độ thường
và `--strict` (chỉ thêm luật token `wake`; luật `transition_tcb`/`write_state`/mỏ neo chạy ở
**cả hai**); (ii) **lớp dep-info** `check-wake-depinfo.py` chạy **một lượt** trên danh sách tệp
rustc thật nạp. `v2/bin/v2:344-361` chạy đúng **ba `run_step`** + **hai selftest**; cả ba lượt đều
nằm trong `./v2/bin/v2 check` và `gate --fast`.

## 7. Bảo trì

- Mọi dạng mới **phải** thêm vào §2 (hỗ trợ) hoặc §3 (từ chối) **trước** khi sửa mã.
- Mọi ca hồi quy nằm trong `--selftest`; ca mới phải có **đối chứng dương** khi có thể.
- Sai lệch còn mở **phải** ghi ở §4, không được im lặng.
- **KT-241**: mọi thay đổi **ranh giới API** (`NonReadyTarget`, cửa `Ready`, mức công khai của
  `write_state`/`into_state`) **phải** cập nhật **cả** `_check_type_anchors` (§2b.3) **và**
  `STATE_WRITE_CALLERS` (§2b.1) — mỏ neo là thứ thay cho suy luận kiểu; thiếu mỏ neo là
  **fail-open** (checker XANH trong khi API đã bị đảo). Số ca selftest **không được giảm**:
  ca cũ giả định API cũ phải được **cập nhật sang luật mới**, không xoá trắng.
- **KT-241**: `_check_wake_text` **tự** tính vùng test (`_test_char_ranges`); **không** thêm lại
  tham số vùng test — hai cách phân vùng là mồi lệch (KT-240 ghi chú `tr` chết).
