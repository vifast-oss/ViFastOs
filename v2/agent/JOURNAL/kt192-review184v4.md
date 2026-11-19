# KT-192 — review vòng 4 KT-184 (tiêu chí A0 mới, D2-165)

**Ngày**: 2026-11-19 · **Reviewer độc lập L30** · nhánh `night2/L30-review184v4` ·
worktree `/private/tmp/night2-L30-review184v4` · HEAD **`8581c218`** (từ `main` @ `8581c218`).
**Nguồn đọc**: [`CHECKER-WAKE-CONTRACT.md`](../reports/CHECKER-WAKE-CONTRACT.md) (hợp đồng),
[`kt187-review184.md`](kt187-review184.md) (L25), `kt189-review187.md` (L28 — **không** nằm trên
`main`, đọc qua `git show 013d850a:…`), [`kt191-review189.md`](kt191-review189.md) (L29),
`v2/tools/check-wake-paths.py` @ `8581c218`.
**Ngân sách**: hạn cứng 15:00 JST. Không merge/push/`gate --fast`/QEMU/worktree mới.
**Vai**: reviewer độc lập theo **D2-159/D2-165** — mọi đột biến dựng trên **bản sao cây** trong
`/tmp/kt192-l30`, harness **tự dựng** (không dùng `_fixture_v2` của tác giả), không sửa mã sản phẩm.

---

## 0. Phán quyết: **KHÔNG ĐẠT**

Bản vá KT-191 (`a90f1d77`) đóng **đúng** năm đường lọt L29 đã dựng (tên macro Unicode, `mod $n;`,
`mod r#x;`, passthrough `$…:tt`, `#[/*c*/path]`), và **chiều dương** đạt: 12 ca hợp lệ độc lập
được nhận (P1–P12). Nhưng vòng 4 còn **ba đường lọt cùng bản chất** — máy kiểm là **regex trên văn
bản**, không phải tokenizer/parser, nên mọi cách **viết khác đi** của cùng một token Rust đều lọt:

| # | Đường lọt | Chế độ thường | `--strict` | Biên dịch | Mức |
|---|---|---|---|---|---|
| 1 | **`#[r#path = "…"]`** (raw ident tên thuộc tính) | **XANH** | **XANH** | `cargo check` RC=0 | **NẶNG NHẤT** |
| 2 | **`#[path = "t\x2ers"]` / `"t\u{2e}rs"`** (escape chuỗi) | **XANH** | **XANH** | `cargo check` RC=0 | nặng |
| 3 | `#[cfg_attr(not(test), cfg_attr(all(), path = "…"))]` (lồng) → tệp test-only | **XANH** | ĐỎ (nhờ luật `wake`) | — | trung bình |

Cả ba đều **không** phải lý thuyết: mỗi cây có **đối chứng dương** (`self.wake(t)` →
`self.khong_ton_tai(t)` ⇒ `rustc` **ĐỎ** tại đúng tệp đích), tức bản sản phẩm **thật sự** biên dịch
lời gọi `wake`. Đường 1 và 2 lọt **cả hai** chế độ checker trên **bản sao cây thật**.

Theo **D2-165 §2(e)**: còn lọt **cùng bản chất** ⇒ **dừng vá cục bộ**, trình phương án
lexer/token-tree (§6). **Không** đề xuất thêm regex.

Ngoài ra: **`v2 check` trên cây sạch ĐỎ** — 8 lỗi K6 luật 1 **trong chính hợp đồng**
(`CHECKER-WAKE-CONTRACT.md:46-54,61,69`), xem §5.

---

## 1. Chiều âm — đường lọt (đột biến thật, biên dịch được)

### 1.1 `#[r#path = "…"]` — raw identifier cho **tên thuộc tính** (nặng nhất)

Rust cho phép **raw identifier** ở mọi vị trí định danh, kể cả tên thuộc tính. `#[r#path = "t.rs"]`
là **cùng một thuộc tính** với `#[path = "t.rs"]` — `rustc` chấp nhận và nạp đúng tệp.

```rust
// v2/kernel/vifast-core/src/lib.rs (thêm vào cuối)
#[cfg(test)]
mod evil_t;                       // evil_t.rs là TEST-ONLY theo đồ thị module
#[r#path = "evil_t.rs"]
mod x;                            // rustc: nạp evil_t.rs ở BẢN SẢN PHẨM
// v2/kernel/vifast-core/src/evil_t.rs
use crate::Machine;
impl Machine { fn duong_san_pham(&mut self, t: u32) { let _ = self.wake(t); } }
```

```
# check-wake-paths.py --root <bản sao cây thật>            → errors=0  rc=0
# check-wake-paths.py --root <bản sao cây thật> --strict   → errors=0  rc=0
# cargo +1.99.1 check -p vifast-core                       → RC=0 (chỉ warning dead_code)
# ĐỐI CHỨNG DƯƠNG: self.wake → self.khong_ton_tai
error[E0599]: no method named `khong_ton_tai` found for mutable reference `&mut Machine`
 --> vifast-core/src/evil_t.rs:2:68
RC=101
```

**Cơ chế**: bộ lọc rẻ `include\s*!|#\s*\[\s*path|cfg_attr` (`:760`) **không khớp** `#[r#path` ⇒
tệp bị **bỏ qua trước** mọi bước phân giải; `RE_PATH_ATTR_ANY = #\s*\[\s*path\s*=` (`:700`) cũng
không khớp. `_mods_of` (`:576`) không thấy `#[path]` nên **không thêm cạnh** cho `mod x;` ⇒
`evil_t.rs` giữ trạng thái **test-only** từ `#[cfg(test)] mod evil_t;` ⇒ luật `wake` miễn.
`rustc` thì nạp `evil_t.rs` ở bản sản phẩm.

**Biến thể đã kiểm** (đều lọt): `#[r#path = r#"t.rs"#]` (raw ident + raw string) — XANH cả hai
chế độ; `#[r#path = "../../../lib/vifast-abi/src/evil_out.rs"]` (ra **ngoài** cây) — XANH cả hai
chế độ, vi phạm thẳng D2-161. Trong **thân macro** thì `#[r#path]` **bị bắt** (vì `\bpath\b` khớp
`r#path`) — nên đường lọt chỉ nằm **ngoài** macro.

### 1.2 Escape chuỗi trong `#[path]` / `include!`

`#[path = "t\x2ers"]` — `rustc` **giải mã escape** (`\x2e` = `.`) ⇒ nạp `t.rs`; checker lấy
**literal thô** `t\x2ers` ⇒ phân giải tệp **decoy** cùng tên (sạch) ⇒ XANH.

```rust
// lib.rs
#[cfg(test)]
mod evil_t;                                  // evil_t.rs TEST-ONLY, có self.wake(t)
#[path = "evil_t\x2ers"]
mod x;                                       // rustc: evil_t.rs;  checker: tệp decoy
// evil_t\x2ers  (tên tệp CÓ backslash) = decoy sạch
```

```
# checker thường + --strict trên bản sao cây thật → errors=0 rc=0
# cargo +1.99.1 check (probe tối thiểu, #[path = "t\x2ers"]) → RC=0
```

Cùng lớp: `#[path = "t\u{2e}rs"]` — XANH cả hai chế độ (đã kiểm trên fixture).
`RE_STR_LIT` (`:438`) lấy `[^"]*` **nguyên văn**, không giải mã escape.

### 1.3 `cfg_attr` lồng — chế độ thường XANH

```rust
#[cfg(test)]
mod t;                                       // t.rs TEST-ONLY, có self.wake(t)
#[cfg_attr(not(test), cfg_attr(all(), path = "t.rs"))]
mod x;
```

```
# thường  → errors=0 rc=0   (XANH)
# --strict → 1 lỗi E_STRICT_TEST_ONLY_WAKE ở t.rs (chỉ nhờ luật wake, KHÔNG nhờ phân giải path)
```

`RE_CFG_ATTR_PATH` **khớp** được attr lồng (đích ra ngoài cây vẫn ĐỎ — đã kiểm), nhưng `_mods_of`
không thêm **cạnh sản phẩm** cho `mod x;` ⇒ đồ thị module vẫn nói `t.rs` test-only. Chế độ thường
**lọt hoàn toàn**; `--strict` chỉ cứu được khi tệp đích **nằm trong** phạm vi luật `wake` và
**không** test-only. Nếu đích là tệp test-only **và** ngoài phạm vi luật `wake` (vd `v2/lib`) thì
lọt cả hai chế độ.

### 1.4 Các nhóm khác — **đã đóng** (kiểm lại, không hồi quy)

| Nhóm | Ca | Kết quả |
|---|---|---|
| delimiter | macro `( … )`, `[ … ]`, hỗn hợp `{ ( [ ] ) }`, `;` trước thân | **ĐỎ** cả hai chế độ |
| comment/whitespace | `#[/*c*/path]`, `include/*c*/!`, `macro_rules/*c*/!`, `#[path\n=\n"…"]`, `include\t!\t(…)` | **ĐỎ** cả hai chế độ |
| identifier | `macro_rules! é`, `mod $n;`, `mod r#x;`, `mod 日本;` | **ĐỎ** cả hai chế độ |
| attribute | `#[path]`, `#[cfg_attr(…, path)]`, `cfg_attr` lồng ra ngoài cây, `#[cfg]` trước `mod` | **ĐỎ** (trừ §1.3) |
| nesting | macro trong macro, `mod` trong macro lồng ba lớp, `include!` trong macro | **ĐỎ** cả hai chế độ |
| token forwarding | `$($t:tt)*`, `$t:tt`, `$($t:/*c*/tt)*` | **ĐỎ** (`E_STRICT_MACRO_TT`) |

---

## 2. Chiều dương — code hợp lệ **được nhận**

12 ca **độc lập** (không phải "bỏ cạnh đột biến thì xanh"), tất cả **XANH** cả hai chế độ:

| Ca | Nội dung |
|---|---|
| P1 | `include!("clean.rs")` literal, đích trong cây, sạch |
| P2 | `#[path = "clean.rs"] mod x;` literal, đích trong cây, sạch |
| P3 | `macro_rules! m { ($x:expr) => { $x + 1 }; }` |
| P4 | `macro_rules! m { ($($x:expr),*) => { vec![$($x),*] }; }` |
| P5 | `macro_rules! m { ($m:ident) => { $m.foo() }; }` |
| P6 | `macro_rules! é { () => { 1 }; }` (tên Unicode, thân sạch) |
| P7 | `#[cfg(test)] mod t;` + `wake` trong `t.rs` |
| P8 | `#[cfg(feature = "ktest")] mod k;` + `wake` |
| P9 | macro chứa chuỗi `"mod path include"` (literal bị khử) |
| P10 | macro chứa chú thích `/* mod */` |
| P11 | macro gọi `modulo(1)` (không phải token `mod`) |
| P12 | macro gọi `include_str!("x")` (không phải `include!`) |
| A4 | `#[cfg(/*c*/test)] mod t;` + `wake` — **XANH** (đồ thị module đọc được qua `RE_CFG_ATTR_LINE`) |

### 2.1 Báo oan — code hợp lệ **bị từ chối**

| Ca | Kết quả | Hợp đồng nói gì |
|---|---|---|
| `macro_rules! m { ($path:expr) => { $path }; }` | **ĐỎ** (2 lỗi `E_STRICT_MACRO_INCLUDE`) | §3 dòng 39 "bất kỳ dạng" — **có tuyên bố**, nhưng §2 dòng 29 nói macro_rules! "được hỗ trợ" ⇒ **mâu thuẫn** |
| `macro_rules! m { () => { std::path::Path::new("x") }; }` | **ĐỎ** | như trên — `std::path` bị coi là `path` |
| `macro_rules! m { ($include:expr) => { $include }; }` | **ĐỎ** | như trên |
| `macro_rules! mk { () => { mod tests; }; }` trong tệp **test-only** | **ĐỎ** (`E_STRICT_MACRO_INCLUDE` + `E_STRICT_MACRO_MOD`) | §3 dòng 39/41 **không** miễn tệp test-only ⇒ báo oan **đã tuyên bố** nhưng không nêu rõ |
| `#[cfg_attr(test, path = "evil.rs")] mod x;` trong macro | **ĐỎ** | §3 dòng 39 — báo oan (path chỉ bật khi test) |

Đây là **đánh đổi chính sách** (fail-closed) đã tuyên bố một phần, **không** phải lỗi kỹ thuật —
nhưng §2 dòng 29 ("macro_rules! … được hỗ trợ") phải sửa thành "**phân giải được thân**", nếu không
người đọc hiểu là "được chấp nhận".

---

## 3. Hợp đồng **có đúng với code** không — từng dòng §2/§3

| Dòng hợp đồng | Code | Đúng? |
|---|---|---|
| §2:24 `include!` ba dấu ngoặc + `include ! ( … )` | `RE_INCLUDE_MACRO` (`:697`) | **ĐÚNG** (selftest 52/53) |
| §2:25 chuỗi thường/raw `r"…"`, `r#"…"#`, `r##"…"##` | `RE_STR_LIT` (`:438`) | **ĐÚNG** (selftest 50/51) — **thiếu** escape `\x2e`/`\u{2e}` ⇒ §1.2 |
| §2:26 `#[path]`, `#[ path ]`, `#[\npath]` | `RE_PATH_ATTR_ANY` (`:700`) | **ĐÚNG** — **thiếu** `#[r#path]` ⇒ §1.1 |
| §2:27 `cfg_attr(not(test), path)` một tầng | `RE_CFG_ATTR_PATH` (`:704`) | **ĐÚNG** (selftest 61) — lồng nhiều tầng: §1.3 |
| §2:28 `mod NAME;` + `#[cfg(test)]`/`#[cfg(feature="ktest")]` | `_mods_of`/`_cfg_is_test` (`:576,549`) | **ĐÚNG** (selftest 22/25/27/38/40) |
| §2:29 `macro_rules!` mọi tên + ba dấu thân | `RE_MACRO_RULES` (`:852`) + `_macro_bodies` (`:859`) | **ĐÚNG về phân giải**; **SAI về ngữ nghĩa "hỗ trợ"** (mọi thân chứa `path`/`include`/`mod` đều ĐỎ — §2.1) |
| §2:30 chú thích chen giữa `#`/`[`/`path` | `strip_rust_noise` trước bộ lọc (`:760`) | **ĐÚNG** (selftest 72) |
| §3:36 `include!`/`#[path]`/`cfg_attr(path)` không literal | `:790-830` | **ĐÚNG** cho dạng thấy được; `#[r#path = concat!(…)]` **vô hình** |
| §3:37 đích ra ngoài cây | `_inside_scan_tree` (`:707`) | **ĐÚNG** cho dạng thấy được; `#[r#path]` ra ngoài **lọt** (§1.1) |
| §3:38 đích không tồn tại | `:800` | **ĐÚNG** |
| §3:39 thân macro chứa `include`/`path`/`mod` | `_check_macro_body` (`:889`) | **ĐÚNG** (rộng hơn mô tả — bắt cả `$path`, `std::path`) |
| §3:40 thân macro chứa `$…:tt` | `:\s*tt\b` (`:905`) | **ĐÚNG** (selftest 71) |
| §3:41 thân macro khai `mod …;` ⇒ `E_STRICT_MACRO_MOD` | `:920` | **SAI một phần**: `mod $n;`/`mod r#x;`/`mod 日本;` phát `E_STRICT_MACRO_INCLUDE` (luật `\bmod\b`), **không** phải `E_STRICT_MACRO_MOD` — selftest 69/70 khẳng định đúng mã `INCLUDE`, tức hợp đồng §3:41 không khớp code |
| §3:42 `wake(` trong mã sản phẩm ⇒ `E_STRICT_TEST_ONLY_WAKE` | `in_test_only_wake_scope` (`:174`) | **SAI phạm vi**: chỉ `v2/kernel/*/src/` + `v2/sim/vifast-sim/src/`; **`v2/lib/**` sản phẩm không bị luật này** (chỉ thân macro). §1 nói rõ v2/lib chỉ bảo vệ thân macro ⇒ §3:42 phải ghi "trong phạm vi §1" |
| §4.4 `cfg_attr` lồng "có thể không phân giải" | — | **ĐÚNG về hiệu ứng**, sai về lý do: regex **khớp** được, nhưng `_mods_of` không thêm cạnh sản phẩm (§1.3) |
| §5:64 cả ba chế độ chạy trong `v2 check` + `gate --fast` | `v2/bin/v2:314,315,321` | **ĐÚNG** (đã kiểm) |
| §6:69 "mỗi ca có **đối chứng dương**" | `selftest()` | **SAI**: 5 ca KT-191 (4bj–4bn) + 4 ca KT-189 (4be–4bh) + 58–60, 61–63, 67 **không** có đối chứng dương (§4) |

---

## 4. Selftest 76 ca — tautology / thiếu đối chứng dương

`--selftest` trên cây sạch: **76/76 ok** (không `#[ignore]`, không `#[should_panic]`, runner đếm
đủ `len(cases)`).

**Thiếu đối chứng dương** (chỉ có "đột biến ⇒ đỏ", không có "bỏ đột biến ⇒ xanh"):

- **4bj–4bn** (KT-191): tên macro Unicode, `mod $n;`, `mod r#x;`, passthrough `$…:tt`,
  `#[/*c*/path]` — **cả năm** đều không có ca "bỏ đột biến ⇒ xanh". Đây đúng là năm đường lọt L29;
  ca hồi quy cho chúng **không** chứng minh được chúng **có răng**.
- **4be–4bh** (KT-189): macro ngoặc tròn/vuông, `cfg_attr(path)` trong macro, `mod` trong macro —
  cùng vấn đề; thêm nữa fixture dùng `evil_b.rs` **không tồn tại** nên cây cũng ĐỎ vì
  `E_STRICT_WAKE_INCLUDE` (assert đúng mã macro nên vẫn phân biệt được, nhưng **không cô lập**).
- **58–60** (D2-161), **61** (B1), **62–63** (B2/B3), **67** (T3c): không có ca dương tương ứng.

**Tautology / không phân biệt**: không có ca nào assert `not rep.ok` mà thiếu mã lỗi cụ thể (20 ca
"xanh" assert `rep.ok` — đúng vai trò đối chứng dương). Nhưng **5 ca KT-191** và **4 ca KT-189**
không có đối chứng dương ⇒ theo đúng tiêu chí A0 ("thêm ca hợp lệ độc lập, không chỉ bỏ cạnh đột
biến thì xanh") thì **phần selftest mới chưa đạt**.

**Thiếu ca cho đường lọt vòng 4**: **không** có ca nào cho `#[r#path]`, escape `\x2e`/`\u{2e}`,
`cfg_attr` lồng → tệp test-only. Đó chính là ba đường lọt §1.

---

## 5. `v2 check` trên cây sạch — **ĐỎ** (8 lỗi K6 trong chính hợp đồng)

```
$ ./v2/bin/v2 check
--- fmt: OK
--- clippy: OK
--- luật cấu trúc: OK
--- self-test luật: OK
not ok - luat 1: v2/agent/reports/CHECKER-WAKE-CONTRACT.md:46: E1_NUMBER_OUTSIDE [tracked] số '1' ngoài khối `scope=`
not ok - luat 1: …:47 '2' | :49 '3' | :51 '4' | :53 '5' | :54 '6' | :61 '76' | :69 '76'
# verify-numbers: ok=4 fail=1
--- luật máy K6 (KT-80: 1,2,3,7,8): FAIL
--- wake paths (KT-151): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155): OK
v2 check: 1 bước FAIL   (RC=1)
```

Nguyên nhân: **hợp đồng** (do KT-184 thêm ở `cadd0f67`) có danh sách số **1–6** ở §4 và số **76** ở
§5/§6 **ngoài khối `scope=`** ⇒ vi phạm K6 luật 1. Đây là **cổng đỏ trên `main`** do chính bàn giao
KT-184 gây ra, **không** liên quan checker. Ba bước wake paths đều **OK**.

---

## 6. Đề xuất theo D2-165 §2(e) — **lexer/token-tree**, không vá regex

Ba đường lọt §1 là **cùng một lớp**: checker so khớp **văn bản thô**, còn `rustc` so khớp **token**.
Mỗi lần vá regex lại sinh ra cách viết khác (L25: `cfg_attr`; L28: ngoặc tròn/vuông; L29: tên
Unicode/`$n`/`r#x`/passthrough/chú thích; L30: `r#path`/escape/cfg_attr lồng). Vá tiếp là **đúng
vòng lặp** A0 đã cấm.

**Phạm vi thay thế đề xuất** (không tự mở viết lại lớn):

1. **Một tệp duy nhất**: `v2/tools/check-wake-paths.py`. Giữ nguyên CLI (`--strict`, `--selftest`,
   `--json`, `--root`), mã lỗi, và **toàn bộ 76 ca** làm hồi quy.
2. **Thay lớp regex bằng tokenizer ~250–350 dòng Python**:
   - token: định danh (kể cả `r#…`), literal (chuỗi **đã giải mã escape**, raw string, char),
     dấu câu, **nhóm ngoặc lồng** (cây delimiter), chú thích bị bỏ.
   - thuộc tính = dãy token `#` `[` … `]`; tên thuộc tính so **sau khi bỏ `r#`**; giá trị `path`
     = literal **đã giải mã**.
   - `include!` = token `include` (hoặc `r#include`) + `!` + nhóm ngoặc chứa **một** literal.
   - `macro_rules!` = token + tên (mọi định danh) + nhóm thân; `mod` = token `mod` + định danh
     (kể cả `$n`, `r#x`, Unicode) + `;`.
3. **Giữ nguyên** đồ thị module, luật `wake`, allowlist, và các luật 1–5 — chúng chỉ **đọc** token
   thay vì regex; không đổi ngữ nghĩa.
4. **Ca hồi quy bắt buộc** (thêm vào `--selftest`, mỗi ca kèm đối chứng dương):
   `#[r#path]`, `#[r#path = r#"…"#]`, `#[path = "t\x2ers"]`, `#[path = "t\u{2e}rs"]`,
   `#[cfg_attr(not(test), cfg_attr(all(), path = …))]` → tệp test-only, và **ca dương** cho cả năm
   ca KT-191 (bỏ đột biến ⇒ xanh).
5. **Ước lượng**: 1 task ≤ 3 h (tokenizer + cổng 6 luật + ca hồi quy). **Không** đụng `spec/`,
   không đổi ngưỡng, không đổi mã lỗi.

**Không** đề xuất: (a) thêm regex `(?:r#)?path` — vá được §1.1 nhưng §1.2 vẫn lọt; (b) giải mã
escape bằng `unicode_escape` — vá được §1.2 nhưng `r#path` vẫn lọt; (c) bất kỳ bản vá cục bộ nào
khác — đúng lý do D2-165 §2(e).

---

## 7. Bằng chứng (commit cùng)

| Tệp | Nội dung |
|---|---|
| [`kt192-logs/fx.py`](kt192-logs/fx.py) | fixture tối thiểu **tự dựng** (không dùng `_fixture_v2`) |
| [`kt192-logs/h.py`](kt192-logs/h.py) | harness chạy checker 2 chế độ trên bản sao cây thật |
| [`kt192-logs/t1.py`](kt192-logs/t1.py) + [`t1-sweep.log`](kt192-logs/t1-sweep.log) | nhóm attribute/identifier/escape + 12 ca dương |
| [`kt192-logs/t2.py`](kt192-logs/t2.py) + [`t2-sweep.log`](kt192-logs/t2-sweep.log) | `#[r#path]`, escape, delimiter, comment, nesting, token forwarding |
| [`kt192-logs/real-tree.log`](kt192-logs/real-tree.log) | **bản sao cây thật**: `#[r#path]` (test-only + ngoài cây), escape, `cargo check` RC=0, đối chứng dương E0599 |
| [`kt192-logs/clean-tree.log`](kt192-logs/clean-tree.log) | HEAD + ba chế độ checker trên cây sạch |
| [`kt192-logs/v2-check-clean-tree.log`](kt192-logs/v2-check-clean-tree.log) | `./v2/bin/v2 check` đầy đủ — 1 bước FAIL (K6) |

**Lệnh tái lập** (bash 3.2, không `timeout`):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/night2-L30-review184v4
python3 v2/tools/check-wake-paths.py            # errors=0
python3 v2/tools/check-wake-paths.py --strict   # errors=0
python3 v2/tools/check-wake-paths.py --selftest # 76/76 ok
./v2/bin/v2 check                               # RC=1 — K6 luật 1 trong hợp đồng

# đường lọt 1 (bản sao cây thật):
rsync -a --exclude target v2/ /tmp/kt192-l30/real/v2/
printf '\n#[cfg(test)]\nmod evil_t;\n#[r#path = "evil_t.rs"]\nmod x;\n' >> /tmp/kt192-l30/real/v2/kernel/vifast-core/src/lib.rs
printf 'use crate::Machine;\nimpl Machine { fn f(&mut self, t: u32) { let _ = self.wake(t); } }\n' \
  > /tmp/kt192-l30/real/v2/kernel/vifast-core/src/evil_t.rs
python3 v2/tools/check-wake-paths.py --root /tmp/kt192-l30/real            # errors=0
python3 v2/tools/check-wake-paths.py --root /tmp/kt192-l30/real --strict   # errors=0
(cd /tmp/kt192-l30/real/v2/kernel && cargo +1.99.1 check -p vifast-core)   # RC=0
```

---

## 8. Chưa review / giới hạn

- **Không** chạy `gate --fast` (theo đề). Chỉ `v2 check` + ba chế độ checker + `cargo check` trên
  bản sao cây.
- **Không** kiểm Miri/L1/QEMU — ngoài phạm vi KT-192.
- **Không** kiểm nhánh `--json` (đã dùng nội bộ trong harness) và `scan_v1`.
- **Chưa** thử: `build.rs` sinh mã trong cây; symlink; `#[path]` với escape **kết hợp** raw ident;
  `r#include!` (đã phân tích: `\binclude` khớp nên **bị bắt**); `macro_rules!` 2.0 (không ổn định).
- **Không** sửa mã sản phẩm; mọi đột biến trong `/tmp/kt192-l30`; cây worktree sạch trước commit
  (chỉ thêm `v2/agent/JOURNAL/kt192-*`).
- **K6**: `v2 check` đỏ vì hợp đồng — **không** sửa hợp đồng trong task này (không phải quyền
  reviewer); đề nghị lead/Agent 1 đưa các số vào khối `scope=` hoặc xin miễn như `A1-decision-log.md`.
