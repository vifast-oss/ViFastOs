# KT-191 — review đối kháng bản vá B2c/B3a/B2d/T3c (KT-189)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer độc lập L29** · nhánh `night2/L29-review189` ·
worktree `/private/tmp/night2-L29-review189` · HEAD `09629647` (từ `main` @ `09629647`).
**Nguồn đọc**: [`kt189-review187.md`](kt189-review187.md) (L28 — **không** nằm trên `main`, đọc
qua `git show 013d850a:v2/agent/JOURNAL/kt189-review187.md`), `git show 09629647`,
`v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 13:00 JST. Không merge/push/`gate --fast`/QEMU/worktree mới.
**Vai**: reviewer độc lập theo **D2-159** — mọi đột biến dựng trên **bản sao cây** trong
`/tmp/kt191-l29`, **không** dùng script của tác giả hay của L28; không sửa mã sản phẩm.

## 0. Phán quyết

| # | Yêu cầu | Kết quả | Căn cứ |
|---|---|---|---|
| 1 | B2c/B3a dựng lại đúng dạng L28 ⇒ **ĐỎ ở chế độ thường**; checker `56b693a9` cùng cây ⇒ `errors=0` | **ĐẠT** | §1, [`t1-b2c-b3a-b2d-t3c.log`](kt191-logs/t1-b2c-b3a-b2d-t3c.log) |
| 1b | Ngoặc **vuông** `[ … ]`, macro lồng **ba lớp**, macro có `;` trước thân | **ĐẠT** (đều ĐỎ) | §1.2–1.4, [`t2-variants-filter.log`](kt191-logs/t2-variants-filter.log) |
| 2 | B2d `cfg_attr(path)` trong macro ⇒ **ĐỎ**; checker cũ ⇒ XANH | **ĐẠT** | §2, [`t1-…log`](kt191-logs/t1-b2c-b3a-b2d-t3c.log) |
| 3 | T3c `mod` trong macro ⇒ **ĐỎ** (`E_STRICT_MACRO_MOD`); checker cũ ⇒ XANH | **ĐẠT** (đúng dạng literal) | §3, [`t1-…log`](kt191-logs/t1-b2c-b3a-b2d-t3c.log) |
| 3b | `mod` có `#[path]`, `pub mod`, `mod` trong macro lồng | **ĐẠT** (đều ĐỎ) | §3.2, [`t2-…log`](kt191-logs/t2-variants-filter.log) |
| 4 | Bộ lọc rẻ: `include ! (…)`, `#[ path = … ]`, `# [ path = … ]`, `#[\npath = …]` | **ĐẠT** (bốn dạng đều ĐỎ) | §4.1, [`t2-…log`](kt191-logs/t2-variants-filter.log) |
| 4b | Bộ lọc rẻ với **chú thích** chen giữa token | **KHÔNG ĐẠT** — `#[/*c*/path]`, `#/*c*/[path]`, `#\n/*c*/\n[path]` **lọt** (fmt không chuẩn hoá) | §4.2, [`t6-real-tree.log`](kt191-logs/t6-real-tree.log) |
| 5 | Báo oan: macro hợp lệ có `mod` trong tệp test-only; `cfg_attr(test, path)` trong macro | **CÓ báo oan** (fail-closed, nhưng là báo oan) | §5, [`t3-bypass-fp.log`](kt191-logs/t3-bypass-fp.log) |
| 6 | Selftest 71 ca: 4be/4bf/4bg/4bh có **thật** (bỏ đột biến ⇒ xanh, thêm lại ⇒ đỏ) | **ĐẠT** | §6, [`t5-teeth.log`](kt191-logs/t5-teeth.log) |
| 7 | Đường vòng mới | **KHÔNG ĐẠT — năm đường lọt thật, đều qua `fmt` + `clippy -D warnings` + checker (cả hai chế độ)** | §7, [`t6-real-tree.log`](kt191-logs/t6-real-tree.log) |

### Phán quyết: **KHÔNG ĐẠT**

Bản vá đóng **đúng** bốn dạng L28 đã dựng (B2c/B3a macro ngoặc tròn/vuông; B2d `cfg_attr(path)`
trong macro; T3c `mod` literal trong macro), chuẩn hoá **đúng** bốn dạng bộ lọc rẻ ở §4.1, và
bốn ca selftest mới **có răng thật**. Nhưng luật **chưa đóng**: còn **năm** đường để mã sản phẩm
biên dịch `wake` trong khi **cả ba** cổng — `cargo fmt --all --check`,
`cargo clippy … -D warnings`, `check-wake-paths.py` **thường lẫn `--strict`** — đều **xanh**:

1. **Tên macro Unicode** — `macro_rules! é { … }` (§7.1). `RE_MACRO_RULES` chỉ nhận
   `[A-Za-z_][A-Za-z0-9_]*` ⇒ **toàn bộ** thân macro không được quét ⇒ lọt **cả bốn** luật
   B2c/B3a/B2d/T3c cùng lúc. **Nặng nhất.**
2. **`mod $n;`** (metavariable) trong thân macro — `_check_macro_body` đòi **định danh literal**
   (§7.2).
3. **`mod r#x;`** (raw ident) trong thân macro — cùng regex ASCII (§7.3).
4. **Macro passthrough** `$($t:tt)*` + `crate::pass! { mod x; }` — thân macro **không** có chữ
   `mod` literal (§7.4).
5. **`#[/*c*/path = …]`** / `#/*c*/[path = …]` / `#\n/*c*/\n[path = …]` — bộ lọc rẻ vẫn là
   regex trên **văn bản thô**, chú thích chen giữa `#`/`[`/`path` làm nó không mở (§4.2).

Cả năm đều **không** phải lý thuyết: mỗi cây có **đối chứng dương** (đổi `self.wake(t)` thành
`self.khong_ton_tai(t)` ⇒ `rustc` **ĐỎ** tại đúng tệp đích) — tức bản sản phẩm **thật sự** biên
dịch lời gọi `wake`. Bốn đường đầu còn qua `rustc -D warnings` + `rustfmt --check` (tương đương
bước `clippy -D warnings` + `fmt` của cổng).

---

## 1. B2c/B3a — macro ngoặc tròn/vuông

### 1.1 Dựng lại đúng dạng L28

Cây: fixture `_fixture_v2` + `lib.rs` khai `mod m; mod sub;`; macro định nghĩa ở `src/m.rs`
(**decoy sạch** `src/evil.rs`), gọi ở `src/sub/s.rs`; tệp thật `src/sub/evil.rs` là test-only
(`#[cfg(test)] mod evil;` trong `src/sub.rs`) và có `self.wake(t)`.

```rust
// v2/kernel/vifast-core/src/m.rs
#[macro_export]
macro_rules! evil_paren (
    () => (
        #[path = "evil.rs"] mod x;
    );
);
```

```
### B2c  strict=0
  NEW rc=1 errors=2
    E E_STRICT_MACRO_INCLUDE   v2/kernel/vifast-core/src/m.rs:5: thân `macro_rules! evil_paren` chứa `#[path]` — … (fail-closed, KT-187 B2)
    E E_STRICT_MACRO_MOD   v2/kernel/vifast-core/src/m.rs:5: thân `macro_rules! evil_paren` khai `mod …;` — … (fail-closed, KT-189 T3c)
  OLD rc=0 errors=0
```

⇒ **ĐẠT**. `_macro_bodies` (`v2/tools/check-wake-paths.py:856`) nhận cả ba dấu mở; bản
`56b693a9` xanh trên **cùng cây** — lỗ hổng là thật, bản vá là thật.

### 1.2 Ngoặc vuông

`macro_rules! evil_paren [ () => [ #[path = "evil.rs"] mod x; ]; ];` ⇒ **ĐỎ** (2 lỗi), OLD
xanh. **ĐẠT**.

### 1.3 Macro lồng ba lớp

`outer { () => { macro_rules! mid { … macro_rules! inner { () => { #[path] mod x; } } } } }` ⇒
**ĐỎ 6 lỗi** (mỗi lớp một cặp `E_STRICT_MACRO_INCLUDE` + `E_STRICT_MACRO_MOD`). Bản cũ cũng ĐỎ
3 lỗi (đã bắt `#[path]` trong ba thân `{ … }`) — không phải hồi quy. **ĐẠT**.

### 1.4 Macro có `;` trước thân

Hai dạng: `;` trong **matcher** (`($a:expr;) => ( … )`) và `;` trong **transcriber**
(`() => ( … );`). Cả hai ⇒ **ĐỎ** (2 lỗi), OLD xanh. **ĐẠT**.

---

## 2. B2d — `cfg_attr(path)` trong macro

```rust
#[macro_export]
macro_rules! evil_paren {
    () => {
        #[cfg_attr(not(test), path = "evil.rs")]
        mod x;
    };
}
```

```
### B2d  strict=0
  NEW rc=1 errors=2
    E E_STRICT_MACRO_INCLUDE   m.rs:5: thân `macro_rules! evil_paren` chứa `cfg_attr(…, path = …)` — … (fail-closed, KT-189 B2d)
    E E_STRICT_MACRO_MOD   m.rs:6: thân `macro_rules! evil_paren` khai `mod …;` — … (KT-189 T3c)
  OLD rc=0 errors=0
```

⇒ **ĐẠT**. `_check_macro_body` (`:886`) bắt `cfg_attr\s*\([^;{}]*?\bpath\s*=`; bản cũ xanh.

---

## 3. T3c — `mod` trong macro

### 3.1 Dạng L28

```rust
// src/m.rs
#[macro_export]
macro_rules! evil_decl { () => { mod x; }; }
// src/sub.rs
#[cfg(test)]
mod x;                       // đồ thị module: sub/x.rs là TEST-ONLY
#[cfg(not(test))]
crate::evil_decl!();         // bản sản phẩm: mở ra `mod x;` ⇒ sub/x.rs là SẢN PHẨM
```

```
### T3c  strict=0
  NEW rc=1 errors=1
    E E_STRICT_MACRO_MOD   m.rs:4: thân `macro_rules! evil_decl` khai `mod …;` — … (fail-closed, KT-189 T3c)
  OLD rc=0 errors=0
```

⇒ **ĐẠT** (đúng dạng literal).

### 3.2 Biến thể

| Biến thể | NEW | OLD |
|---|---|---|
| `#[path = "evil.rs"] mod x;` trong macro | **ĐỎ** (2 lỗi) | ĐỎ 1 lỗi (đã bắt `#[path]`) |
| `pub mod x;` trong macro | **ĐỎ** (`E_STRICT_MACRO_MOD`) | XANH |
| `mod x;` trong macro lồng | **ĐỎ** (2 lỗi) | XANH |

⇒ **ĐẠT** cho các dạng literal.

---

## 4. Bộ lọc rẻ (dòng ~759)

### 4.1 Bốn dạng L28 nêu — **đã bắt**

`v2/tools/check-wake-paths.py:760` nay là
`re.search(r"include\s*!|#\s*\[\s*path|cfg_attr", text)`. Trên fixture (đích **ngoài cây**):

| Dạng | NEW | OLD |
|---|---|---|
| `include ! ("…")` | **ĐỎ** (`E_STRICT_WAKE_INCLUDE`) | XANH |
| `include ! { "…" }` | **ĐỎ** | XANH |
| `#[ path = "…" ]` | **ĐỎ** | XANH |
| `# [ path = "…" ]` | **ĐỎ** | XANH |
| `#[\npath = "…"]` | **ĐỎ** | XANH |

⇒ **ĐẠT** cho bốn dạng được nêu.

### 4.2 Nhưng bộ lọc vẫn là regex trên **văn bản thô** — chú thích chen giữa token ⇒ **lọt**

| Dạng | NEW (thường + `--strict`) | `rustfmt --check` | Kết luận |
|---|---|---|---|
| `#[/*c*/path = "…"]` | **XANH** | `rc=0` (không chuẩn hoá) | **LỌT CỔNG** |
| `#/*c*/[path = "…"]` | **XANH** | `rc=0` | **LỌT CỔNG** |
| `#\n/*c*/\n[path = "…"]` | **XANH** | `rc=0` | **LỌT CỔNG** |
| `#[path/*c*/ = "…"]` | **ĐỎ** | — | bắt được (bộ lọc thấy `#[path`) |
| `#[path = /*c*/ "…"]` | **ĐỎ** | — | bắt được |
| `include/*c*/!("…")` | **XANH** (checker) | `rc=1` (chuẩn hoá) | lỗ **checker**, fmt cứu |
| `include//c\n!("…")` | **XANH** (checker) | `rc=1` | lỗ **checker**, fmt cứu |
| `include\n/*c*/\n!("…")` | **XANH** (checker) | `rc=1` | lỗ **checker**, fmt cứu |

Căn cứ: [`t2-variants-filter.log`](kt191-logs/t2-variants-filter.log) (fixture),
[`t6-real-tree.log`](kt191-logs/t6-real-tree.log) mục H/I (bản sao cây thật), `rustfmt --check`
trên `core_ipc.rs`.

⇒ **KHÔNG ĐẠT** cho §4.2: bản vá tự nhận "chuẩn hoá bộ lọc" nhưng bộ lọc vẫn **lệch** với
`strip_rust_noise` — chú thích bị khử **sau** bộ lọc, không phải **trước**.

---

## 5. Báo oan

| Tình huống | NEW thường | OLD thường | Đánh giá |
|---|---|---|---|
| `macro_rules! mk_tests { () => { mod tests; }; }` trong tệp **test-only** | **ĐỎ** (`E_STRICT_MACRO_MOD`) | XANH | **báo oan** — macro chỉ dùng trong test, sản phẩm không biên dịch |
| `macro_rules! mk_tests { … mod tests; … }` gọi trong `#[cfg(test)]` | **ĐỎ** | XANH | **báo oan** |
| `#[cfg_attr(test, path = "evil.rs")] mod x;` trong macro | **ĐỎ** (2 lỗi) | XANH | **báo oan** — `path` chỉ bật khi test |
| `cfg_attr(test, path = "…")` **ngoài** macro | XANH | XANH | không đổi (L28 §1.3 đã ghi nợ) |

Căn cứ: [`t3-bypass-fp.log`](kt191-logs/t3-bypass-fp.log) mục BAO-OAN-A/B/C.

⇒ Luật mới **fail-closed** đúng tinh thần D2-161, nhưng **không phân biệt** macro trong tệp
test-only / `cfg_attr(test, …)` ⇒ báo oan **mới** (bản `56b693a9` xanh trên cùng cây). Đây là
đánh đổi chính sách cần Agent 0 quyết, không phải lỗi kỹ thuật — nhưng phải ghi nhận vì
`E_STRICT_MACRO_MOD` sẽ chặn mọi macro sinh `mod` hợp lệ trong test.

---

## 6. Selftest 71 ca — răng của 4be/4bf/4bg/4bh

Dựng lại **độc lập** ([`t5-teeth.py`](kt191-logs/t5-teeth.py)): dùng `_fixture_v2`, tự dựng
đột biến, rồi **bỏ** và **thêm lại**.

| Ca | Có đột biến | Bỏ đột biến | Kết luận |
|---|---|---|---|
| 4be `#[path]` trong macro ngoặc tròn | **ĐỎ** (`E_STRICT_MACRO_INCLUDE` + 2 lỗi khác) | **XANH** | **có thật** |
| 4bf `include!` trong macro ngoặc tròn | **ĐỎ** (`E_STRICT_MACRO_INCLUDE` + 1) | **XANH** | **có thật** |
| 4bg `cfg_attr(path)` trong macro | **ĐỎ** (`E_STRICT_MACRO_INCLUDE` + 2) | **XANH** | **có thật** |
| 4bh `mod …;` trong macro | **ĐỎ** (`E_STRICT_MACRO_MOD`) | **XANH** | **có thật** |

`--selftest`: bản vá **71/71 ok**; bản `56b693a9` **67/67 ok**. Không ca nào
`#[ignore]`/`#[should_panic]`; runner đếm đủ `len(cases)`.

**Nhưng**: 4be/4bf/4bg **cũng ĐỎ trên bản cũ** (vì `evil_b.rs` không tồn tại ⇒
`E_STRICT_WAKE_INCLUDE`), nên chúng **không** chứng minh đường lọt — chỉ chứng minh **mã lỗi
mới** có mặt. Chỉ 4bh phân biệt được cũ/mới. Và **không** ca nào phủ năm đường lọt ở §7.

---

## 7. Đường vòng mới — **năm đường lọt thật, đều qua cả ba cổng**

Script: [`t6-real.py`](kt191-logs/t6-real.py) (chạy trên **bản sao cây thật**
`/tmp/kt191-l29/real` = `v2/kernel`+`v2/sim`+`v2/lib`), [`t3-bypass.py`](kt191-logs/t3-bypass.py),
[`t4-after-fmt.py`](kt191-logs/t4-after-fmt.py). Log tổng hợp:
[`t6-real-tree.log`](kt191-logs/t6-real-tree.log), [`gate-level.log`](kt191-logs/gate-level.log).

### 7.1 **Tên macro Unicode** — lọt **cả bốn** luật (nặng nhất)

```rust
// v2/kernel/vifast-core/src/m.rs (sản phẩm)
#[macro_export]
macro_rules! é {
    () => {
        #[path = "evil.rs"]
        mod x;
    };
}
// src/evil.rs = decoy sạch; src/sub/s.rs gọi crate::é!();
// src/sub/evil.rs = test-only, có self.wake(t)
```

```
### A. Unicode macro name é + #[path] (B2c) — BAN SAO CAY THAT
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  strict=1  NEW rc=0 errors=0 | OLD rc=0 errors=0
### B. Unicode macro name é + include! (B3a)
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  strict=1  NEW rc=0 errors=0 | OLD rc=0 errors=0
### C. Unicode macro name é + mod x; (T3c)
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  strict=1  NEW rc=0 errors=0 | OLD rc=0 errors=0
```

**Cơ chế**: `RE_MACRO_RULES = re.compile(r"\bmacro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)")`
(`v2/tools/check-wake-paths.py:849`) chỉ nhận **định danh ASCII**. `macro_rules! é` (Rust cho
định danh Unicode — `é` là XID_Start) **không khớp** ⇒ `_macro_bodies` không yield thân nào ⇒
`scan_lib_macros`/`scan_kernel_macro_includes` **không quét**. `resolve_include_targets` thấy
`#[path = "evil.rs"]` trong **tệp định nghĩa** và phân giải theo tệp định nghĩa (decoy sạch) ⇒
không lỗi. rustc giải theo **tệp gọi** ⇒ nạp `sub/evil.rs` (test-only, có `wake`).

**Đối chứng dương** (cây tối thiểu `/tmp/kt191-l29/clippyprobe`):

```
### rustc -D warnings
rc=0
### POSITIVE CONTROL (self.wake -> self.khong_ton_tai)
error[E0599]: no method named `khong_ton_tai` found for mutable reference `&mut Machine` in the current scope
 --> src/sub/evil.rs:3:14
rc=1
### rustfmt --check
rc=0
```

⇒ **LỌT CỔNG ĐẦY ĐỦ**: `fmt` xanh, `clippy -D warnings` xanh, checker **thường lẫn `--strict`**
xanh, bản sản phẩm biên dịch `wake`. Một tên macro Unicode vô hiệu hoá **toàn bộ** bốn luật
B2c/B3a/B2d/T3c cùng lúc.

### 7.2 **`mod $n;`** (metavariable) — T3c

```rust
#[macro_export]
macro_rules! evil_decl {
    ($n:ident) => {
        mod $n;
    };
}
// src/sub.rs
#[cfg(test)]
mod x;
#[cfg(not(test))]
crate::evil_decl!(x);
```

```
### K. mod $n; trong macro — BAN SAO CAY THAT
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  strict=1  NEW rc=0 errors=0 | OLD rc=0 errors=0
  rustc -D warnings rc=0 ; rustfmt --check rc=0 ; positive control E0599 tai src/sub/x.rs
```

**Cơ chế**: `_check_macro_body` (`:901`) dùng
`r"\bmod\s+[A-Za-z_][A-Za-z0-9_]*\s*;"` — đòi **định danh literal** sau `mod`. `mod $n;` (hợp
lệ trong transcriber — `$n` thay bằng ident) **không khớp** ⇒ không lỗi. `_mods_of` cũng không
thấy (cùng regex ASCII, `:432`) ⇒ đồ thị module nói `sub/x.rs` test-only ⇒ luật `wake` bỏ qua.
rustc mở `mod x;` ở bản sản phẩm ⇒ `sub/x.rs` là **sản phẩm**.

### 7.3 **`mod r#x;`** (raw ident) — T3c

```rust
macro_rules! evil_decl { () => { mod r#x; }; }
```

```
### L. mod r#x; trong macro — BAN SAO CAY THAT
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  strict=1  NEW rc=0 errors=0 | OLD rc=0 errors=0
  rustc -D warnings rc=0 ; rustfmt --check rc=0 ; positive control E0599 tai src/sub/x.rs
```

**Cơ chế**: cùng regex `:901` — `mod r#x;` có `r` rồi `#` ⇒ `[A-Za-z0-9_]*\s*;` không khớp.
`_mods_of` (`:432`) cũng không thấy. rustc nạp `x.rs` (raw prefix bị bỏ khi tra tệp).

### 7.4 **Macro passthrough** — T3c

```rust
#[macro_export]
macro_rules! pass {
    ($($t:tt)*) => { $($t)* };
}
// src/sub.rs
#[cfg(test)]
mod x;
#[cfg(not(test))]
crate::pass! { mod x; }
```

```
### M. passthrough macro + crate::pass! { mod x; } — BAN SAO CAY THAT
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  strict=1  NEW rc=0 errors=0 | OLD rc=0 errors=0
  rustc -D warnings rc=0 ; rustfmt --check rc=0 ; positive control E0599 tai src/sub/x.rs
```

**Cơ chế**: thân macro **không** có chữ `mod` literal ⇒ `_check_macro_body` không thấy. Lời gọi
`crate::pass! { mod x; }` — `_mods_of` chỉ khớp `^\s*mod …;` **đầu dòng**; rustfmt giữ dạng
`{ mod x; }` **một dòng** ⇒ không thấy. Đồ thị module nói test-only; rustc mở `mod x;` ở sản
phẩm. (Dạng `crate::pass!(mod x;);` **bị** rustfmt tách dòng ⇒ `_mods_of` thấy ⇒ đóng; dạng
`{ … }` thì **không**.)

### 7.5 **`#[/*c*/path]`** — bộ lọc rẻ (B2/B6)

```
### H. #[/*c*/path = ...] cap tep — BAN SAO CAY THAT
  strict=0  NEW rc=0 errors=0 | OLD rc=0 errors=0
  rustfmt --check rc=0 (KHONG chuan hoa) => LOT CONG
```

**Cơ chế**: bộ lọc `re.search(r"include\s*!|#\s*\[\s*path|cfg_attr", text)` (`:760`) chạy trên
**văn bản thô**; `#[/*c*/path` không khớp `#\s*\[\s*path` ⇒ tệp bị **bỏ qua trước khi**
`strip_rust_noise` khử chú thích. rustc chấp nhận (chú thích = khoảng trắng) và nạp tệp đích
**ngoài cây** (`v2/lib/…`) — đúng thứ D2-161 cấm.

**Đối chứng dương** (cây tối thiểu `/tmp/kt191-l29/cmtprobe`, `#[/*c*/path = "evil_out.rs"]`):

```
### rustc (binh thuong)
warning: method `duong_san_pham` is never used
 --> src/evil_out.rs:2:8
### POSITIVE CONTROL (self.wake -> self.khong_ton_tai)
error[E0599]: no method named `khong_ton_tai` found for mutable reference `&mut Machine` in the current scope
 --> src/evil_out.rs:3:14
rc=1
```

### 7.6 Các đường khác — **không** lọt (đã kiểm)

| Đường | Kết quả | Ghi chú |
|---|---|---|
| `macro_rules! r#evil` (raw ident **tên macro**) | **ĐỎ** (2 lỗi) | regex khớp `r` rồi nhảy tới thân |
| `#[path]` trong **inline module** (decoy `src/evil.rs`, thật `src/a/evil.rs`) | **XANH thường**, **ĐỎ `--strict`** | luật `wake` bắt `a/evil.rs` (không trong đồ thị module) ⇒ chỉ lọt chế độ thường |
| `mod 日本;` trong macro ASCII | **XANH thường**, **ĐỎ `--strict`** | như trên — `_mods_of`/`_check_macro_body` ASCII không thấy, nhưng luật `wake` bắt |
| `cfg_attr` lồng trong `cfg_attr` | **ĐỎ** (2 lỗi) | regex `[^;{}]*?` phủ được |
| `include!` đường dẫn **tuyệt đối** | **ĐỎ** | `_inside_scan_tree` = None |
| `macro_rules !` (dấu cách trước `!`) | **XANH checker**, `fmt rc=1` | fmt chuẩn hoá ⇒ không lọt cổng |
| `include/*c*/!` | **XANH checker**, `fmt rc=1` | fmt chuẩn hoá ⇒ không lọt cổng |
| `paste!` | — | **không** có trong allowlist phụ thuộc ⇒ không biên dịch; lớp tương đương là passthrough §7.4 |

---

## 8. Đề xuất vòng vá (cho Agent 0 / task kế)

### 8.1 `RE_MACRO_RULES` — nhận **mọi** định danh Rust, không chỉ ASCII (**bắt buộc**, §7.1)

`v2/tools/check-wake-paths.py:849`:

```python
# Unicode XID_Start/XID_Continue — hoặc đơn giản: bỏ nhóm tên, chỉ cần `macro_rules!`
RE_MACRO_RULES = re.compile(r"\bmacro_rules!\s*([^\s(\[{;]+)")
```

Tên chỉ dùng cho thông điệp/allowlist; **không** cần chính xác để quét thân. Hoặc dùng
`\w` với cờ `re.UNICODE` (mặc định) — nhưng `\w` gồm cả số; an toàn hơn là
`[^\s(\[{;]+`. **Bắt buộc** vì một tên Unicode vô hiệu hoá cả bốn luật.

### 8.2 `_check_macro_body` — `mod` nhận **metavariable** và **raw ident** (**bắt buộc**, §7.2–7.3)

`:901`:

```python
for mm in re.finditer(r"\bmod\s+(?:r#)?(?:[A-Za-z_][A-Za-z0-9_]*|\$[A-Za-z_][A-Za-z0-9_]*)\s*;", body):
```

Nhưng §7.4 (passthrough) cho thấy **không** thể đóng bằng regex thân macro. Đề xuất **mạnh
hơn**: `mod` **bất kỳ** trong thân `macro_rules!` ⇒ ĐỎ, kể cả `mod` không có `;` (khai báo
inline) — vì mọi `mod` trong macro đều giải theo tệp gọi. Cùng lý do fail-closed đã áp cho
`include!`/`#[path]`.

### 8.3 Passthrough — **không** đóng được bằng quét token (**bắt buộc quyết định**, §7.4)

Macro `$($t:tt)*` có thể sinh **bất kỳ** token nào, kể cả `mod`. Hai lựa chọn:

- **(a) fail-closed**: thân `macro_rules!` trong `v2/kernel`+`v2/sim`+`v2/lib` chứa
  `$($…:tt)*` (repetition `tt`) ⇒ ĐỎ, trừ allowlist có lý do. Rẻ, đúng D2-161, nhưng cấm
  nhiều macro hợp lệ.
- **(b) hợp nhất đồ thị**: mọi điểm gọi macro phải được mở rộng tĩnh — **không khả thi** vì
  điểm gọi có thể ở crate khác.

Đề xuất **(a)** + allowlist `MACRO_TT_ALLOW` (giống `MACRO_WAKE_ALLOW`).

### 8.4 Bộ lọc rẻ — khử chú thích **trước** khi lọc (**bắt buộc**, §4.2)

`:760`:

```python
code_probe = strip_rust_noise(text, keep_strings=True)
if not re.search(r"include\s*!|#\s*\[\s*path|cfg_attr", code_probe):
    continue
```

`strip_rust_noise` đã giữ nguyên số dòng/cột nên dùng lại `code_probe` cho phần sau luôn (bỏ
`strip_rust_noise` lần hai). Như vậy `#[/*c*/path]`, `#/*c*/[path]`, `#\n/*c*/\n[path]` đều
khớp. **Bắt buộc** vì đây là đường lọt **cổng** (fmt không chuẩn hoá).

### 8.5 Báo oan — **nên** (chính sách, §5)

`E_STRICT_MACRO_MOD` hiện ĐỎ cả macro trong tệp **test-only** và `cfg_attr(test, path)`. Đề
xuất: nếu **mọi** điểm gọi macro nằm trong ngữ cảnh test (đồ thị module) thì miễn — nhưng
điểm gọi có thể ở crate khác ⇒ khó. Tối thiểu: ghi rõ trong DECISIONS rằng đây là **đánh đổi
có chủ ý** (fail-closed), không phải sơ suất.

### 8.6 Selftest — **bắt buộc** cùng lượt

Thêm ca cho **đúng** năm đường lọt ở §7 (mỗi ca kèm đối chứng dương "bỏ đột biến ⇒ xanh"):

- (4bj) `macro_rules! é` + `#[path]` ⇒ ĐỎ;
- (4bk) `mod $n;` trong macro ⇒ ĐỎ;
- (4bl) `mod r#x;` trong macro ⇒ ĐỎ;
- (4bm) passthrough `$($t:tt)*` + `crate::pass! { mod x; }` ⇒ ĐỎ;
- (4bn) `#[/*c*/path = "…"]` **một mình** (không kèm `cfg_attr`) ⇒ ĐỎ.

---

## 9. Tệp bằng chứng

| Tệp | Nội dung |
|---|---|
| [`kt191-logs/harness.py`](kt191-logs/harness.py) | harness chung (dựng fixture, chạy checker mới/cũ) |
| [`kt191-logs/t1-rebuild.py`](kt191-logs/t1-rebuild.py) + [`t1-b2c-b3a-b2d-t3c.log`](kt191-logs/t1-b2c-b3a-b2d-t3c.log) | dựng lại B2c/B3a/B2d/T3c (đúng dạng L28) |
| [`kt191-logs/t2-variants.py`](kt191-logs/t2-variants.py) + [`t2-variants-filter.log`](kt191-logs/t2-variants-filter.log) | ngoặc vuông, lồng ba lớp, `;` trước thân, `mod` biến thể, **bộ lọc rẻ** |
| [`kt191-logs/t3-bypass.py`](kt191-logs/t3-bypass.py) + [`t3-bypass-fp.log`](kt191-logs/t3-bypass-fp.log) | **năm đường lọt** + **báo oan** |
| [`kt191-logs/t4-after-fmt.py`](kt191-logs/t4-after-fmt.py) + [`t4-after-fmt.log`](kt191-logs/t4-after-fmt.log) | checker trên cây **đã qua fmt** |
| [`kt191-logs/t5-teeth.py`](kt191-logs/t5-teeth.py) + [`t5-teeth.log`](kt191-logs/t5-teeth.log) | **răng** của 4be/4bf/4bg/4bh |
| [`kt191-logs/t6-real.py`](kt191-logs/t6-real.py) + [`t6-real-tree.log`](kt191-logs/t6-real-tree.log) | **bản sao cây thật**: Unicode macro, `mod $n`, `mod r#x`, passthrough, `#[/*c*/path]`, raw ident, `macro_rules !` |
| [`kt191-logs/gate-level.log`](kt191-logs/gate-level.log) | tổng hợp: checker thường/`--strict` + `rustc -D warnings` + `rustfmt --check` + đối chứng dương |

---

## 10. Chưa review / giới hạn

- **Không** chạy `gate --fast` (theo đề). Chỉ chạy `check-wake-paths.py` (thường + `--strict`
  + `--selftest`), `rustc -D warnings`, `rustfmt --check` trên **bản sao cây** và cây tối thiểu.
- **Không** kiểm Miri/L1/QEMU — ngoài phạm vi KT-191.
- **Không** kiểm nhánh `--json` của checker.
- **Chưa** thử: `paste!` (không có trong allowlist phụ thuộc — lớp tương đương đã phủ bằng
  passthrough §7.4); `build.rs` sinh mã trong cây; macro `$k:ident $n:ident` (keyword
  metavariable) — cùng lớp passthrough.
- **Không** sửa mã sản phẩm; mọi đột biến trong `/tmp/kt191-l29`; cây worktree sạch trước
  commit (chỉ thêm `v2/agent/JOURNAL/kt191-*`).
