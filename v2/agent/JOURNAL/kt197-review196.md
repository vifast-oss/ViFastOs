# KT-197 — review vòng 2 KT-194 (lexer/token-tree) theo D2-167

**Ngày**: 2026-11-19 · **Reviewer độc lập L35** · nhánh `night2/L35-review196` ·
worktree `/private/tmp/night2-L35-review196` · HEAD **`d6468563`** (từ `main` @ `d6468563`).
**Nguồn đọc**: D2-167 (`v2/agent/DECISIONS.md`, commit `39b6180b` trên `main` — worktree này
tách từ `d6468563` nên **chưa** có D2-167; đã đọc từ `main`), `PHIEU-GUI-AGENT0-2026-11-19-VONG5.md`,
[`CHECKER-WAKE-CONTRACT.md`](../reports/CHECKER-WAKE-CONTRACT.md),
[`kt194-lexer.md`](kt194-lexer.md), [`kt195-review194.md`](kt195-review194.md),
`v2/tools/check-wake-paths.py` @ `d6468563`.
**Ngân sách**: hạn cứng 18:00 JST. Không merge/push/`gate --fast`/QEMU. Mọi đột biến dựng trên
**bản sao cây thật** (`/tmp/kt197-base`, `cp -al` từ worktree, không `target/`), harness tự dựng
(không dùng `_fixture_v2` cho repro). Không sửa mã sản phẩm trên `main`.
**Vai**: reviewer độc lập theo D2-159 — không phải lead, không phải tác giả.

---

## 0. Phán quyết: **KHÔNG ĐẠT**

F1 và F3 **đã đóng** trên HEAD `d6468563` (kiểm lại độc lập, có `cargo check` RC=0). Sáu ca
`mod` của F2 **đạt** (token-tree, `_mods_of` đúng `is_test`, đột biến thật đỏ/xanh đúng). Nhưng
còn **một đường lọt thật NẶNG** cùng bản chất F2 mà bản vá **không** đóng: **luật `wake` vẫn
quyết định ngữ cảnh test bằng `test_line_ranges` (theo dòng)**, nên một hàm sản phẩm bắt đầu
cùng dòng `}` của mục `#[cfg(test)]` bị **miễn toàn bộ** khỏi luật `wake`; `--strict` (và
`v2 check`/`gate`) **XANH** trong khi `cargo check` RC=0 và sản phẩm biên dịch `wake()`.
Ngoài ra F4 còn **báo oan** con test-only của inline module, và hợp đồng §4 nói sai rằng "ngữ
cảnh test đã theo item/token-tree".

| # | Lỗi | Mức | Bằng chứng |
|---|---|---|---|
| **F1** | **ĐÃ ĐÓNG** — ba repro (`#[r#path]`, escape `\x2e`, `cfg_attr` lồng) × hai bố trí (`lib.rs`, `core_ipc.rs`) × hai chế độ đều **ĐỎ**; `cargo check` RC=0 cả 6 ca | — | §2 |
| **F2** | **SÁU CA `mod` ĐẠT**, nhưng **F2b**: luật `wake` vẫn theo dòng ⇒ sản phẩm cùng dòng `}` của mục test bị miễn; `--strict` XANH, `cargo check` RC=0 | **NẶNG** | §3 |
| **F3** | **ĐÃ ĐÓNG** cho `v2/kernel`+`v2/sim` — biên độ sâu đúng (32/33 `cfg_attr`; 126/127 ngoặc), `E_LEX_DEPTH` nhận diện, exit≠0 cả hai chế độ; `_mods_of` còn nuốt nhưng `resolve_include_targets` báo cùng lỗi | — | §4 |
| **F4** | Ca chính **ĐẠT** (không báo oan; `sub` thiếu ⇒ ĐỎ fail-closed). **F4b**: con test-only của inline module bị **báo oan**; **F4c**: `#[path]` trên `mod` không-inline trỏ thư mục bị bỏ qua **im lặng** | TRUNG BÌNH / NHẸ | §5 |
| **F5** | Hợp đồng: §4 nói "ngữ cảnh test đã theo item/token-tree" — **SAI** với luật `wake`; §3 một dòng sai mã lỗi; §5 câu "ngoài `v2/kernel`/`v2/sim` không quét" mâu thuẫn §1 | TRUNG BÌNH | §6 |

**KT-194 xong KHÔNG tự làm KT-184 DONE.** KT-184 vẫn `REVIEW`.

---

## 1. Phương pháp

- Bản sao cây thật: `cp -al /private/tmp/night2-L35-review196 /tmp/kt197-base` (hardlink; mọi
  tệp bị sửa đều `rm` rồi ghi mới để không chạm bản gốc).
- Mỗi ca: tạo bản sao hardlink → thêm tệp/đột biến → chạy
  `python3 v2/tools/check-wake-paths.py` (thường) và `--strict`; đo `cargo check
  --manifest-path v2/kernel/Cargo.toml -p vifast-core --offline` với `RUSTUP_TOOLCHAIN=1.99.1`
  (toolchain ghim của `v2/kernel/rust-toolchain.toml`), `CARGO_TARGET_DIR` dùng chung.
- Đối chứng dương: tệp đích dùng `wake_from` thay `wake` ⇒ phải **XANH**; `#[cfg(test)] mod evil;`
  ⇒ phải **XANH**.
- Log lưu ở [`kt197-logs/`](kt197-logs/).

---

## 2. F1 — ba repro × hai bố trí × hai chế độ (ĐẠT)

`evil_t.rs` = tệp **test-only** qua `#[cfg(test)] mod evil_t;` trong `lib.rs`, thân gọi
`m.wake(t)`; thuộc tính sản phẩm đặt ở **`lib.rs`** hoặc **`core_ipc.rs`**, trỏ `evil_t.rs`.

| # | Dạng | Bố trí | thường | `--strict` | `cargo check` |
|---|---|---|---|---|---|
| R1 | `#[r#path = "evil_t.rs"]` | `lib.rs` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE evil_t.rs:6` | **ĐỎ** | RC=0 |
| R1 | `#[r#path = "evil_t.rs"]` | `core_ipc.rs` | **ĐỎ** | **ĐỎ** | RC=0 |
| R2 | `#[path = "evil_t\x2ers"]` | `lib.rs` | **ĐỎ** | **ĐỎ** | RC=0 |
| R2 | `#[path = "evil_t\x2ers"]` | `core_ipc.rs` | **ĐỎ** | **ĐỎ** | RC=0 |
| R3 | `#[cfg_attr(all(not(test)), cfg_attr(not(test), path = "evil_t.rs"))]` | `lib.rs` | **ĐỎ** | **ĐỎ** | RC=0 |
| R3 | `#[cfg_attr(all(not(test)), cfg_attr(not(test), path = "evil_t.rs"))]` | `core_ipc.rs` | **ĐỎ** | **ĐỎ** | RC=0 |

⇒ **F1 đóng**: bản vá `if not strict:` quét **mọi** tệp sản phẩm trong `extra`
(`v2/tools/check-wake-paths.py:2499-2516`) không còn phụ thuộc vị trí `#[path]`. Log:
`kt197-logs/f1-*.log`, `kt197-logs/f1-cargo-check.log`.

---

## 3. F2 — ngữ cảnh test theo item/token-tree

### 3.1 Sáu tình huống `mod` (ĐẠT)

Dựng `evil.rs` gọi `wake`; `mod evil;` đặt trong sáu tình huống; đo `_mods_of` (đúng `is_test`)
**và** chạy `--strict` (đột biến thật).

| Tình huống | `_mods_of` `is_test` | `--strict` | Kỳ vọng |
|---|---|---|---|
| hai item cùng dòng: `#[cfg(test)] fn h() {} mod evil;` | `False` | **ĐỎ** | sản phẩm |
| khác dòng: `#[cfg(test)]\nfn h() {}\nmod evil;` | `False` | **ĐỎ** | sản phẩm |
| comment/whitespace xen giữa | `False` | **ĐỎ** | sản phẩm |
| nhiều attribute trước `mod` test | `True` | **XANH** | test |
| item sau `}` của module test: `#[cfg(test)] mod tests { } mod evil;` | `False` | **ĐỎ** | sản phẩm |
| module test lồng: `#[cfg(test)] mod tests { mod evil; }` | `True` | **XANH** | test |
| **đối chứng** `#[cfg(test)] mod evil;` | `True` | **XANH** | test |

Thêm 12 biến thể (cùng dòng/khác dòng sau `fn`; `#[cfg(not(test))]`; `#[cfg(all(test, feature =
"ktest"))]`; `#[cfg(any(test, …))]`; `pub mod`; `pub(crate) mod`; `mod r#evil;`; `mod 日本;`;
inline module test/product) — **19/19 khớp** (`_mods_of` + `--strict`). Log: `kt197-logs/f2-mods.log`.

### 3.2 **F2b — luật `wake` vẫn quyết định ngữ cảnh test theo DÒNG (NẶNG, CÒN MỞ)**

`_check_wake_text` (`v2/tools/check-wake-paths.py:2066`) vẫn dùng
`if in_ranges(lineno, tr): continue`, với `tr = test_line_ranges(text)` (`:364`) — **theo dòng
văn bản**. Bản vá F2 chỉ đổi `_mods_of` (`:1291-1300`), **không** đổi luật `wake`.

Đột biến thật (cây thật, `cargo check` RC=0):

```rust
// lib.rs (thêm vào cuối)
#[cfg(test)]
fn h() { let _ = 1; } pub fn evil(m: &mut Machine, t: ObjId) { let _ = m.wake(t); }
```

| Ca | thường | `--strict` | `cargo check` |
|---|---|---|---|
| `wake` cùng dòng `}` của `#[cfg(test)] fn` | XANH | **XANH** `errors=0` | **RC=0** |
| `wake` cùng dòng `}` của `#[cfg(test)] mod` | XANH | **XANH** | RC=0 |
| `wake` dòng kế, hàm sản phẩm bắt đầu cùng dòng `}` test | XANH | **XANH** | RC=0 |
| đối chứng: `wake` dòng kế, hàm sản phẩm **xuống dòng** | XANH | **ĐỎ** | — |

**Nguyên nhân**: `test_line_ranges` tính `end` là dòng đầu tiên có `depth <= 0` kể từ dòng mở
`{` của mục test. Khi hàm sản phẩm bắt đầu **cùng dòng** `}` của mục test, `{` của hàm sản phẩm
cũng nằm trên dòng đó ⇒ `depth` chưa về 0 ⇒ **toàn bộ thân hàm sản phẩm** nằm trong vùng test ⇒
mọi `wake` trong đó bị miễn. Đây đúng là "quyết định theo dòng" mà D2-167 §2 cấm.

**Ảnh hưởng**: `v2 check` và `gate --fast` chạy `--strict` (`v2/bin/v2:321`) ⇒ **cổng XANH**
trong khi sản phẩm biên dịch `wake()`. Đây là lớp lỗi D2-144 sinh ra để chặn.

**Đề nghị** (không tự sửa): thay `in_ranges(lineno, tr)` trong `_check_wake_text` bằng ngữ cảnh
**item/token-tree** (dùng `_walk_items`/`in_test` như `_mods_of`), hoặc ít nhất tính `end` của
vùng test theo **token-tree** thay vì đếm ngoặc trên dòng. Thêm ca hồi quy: sản phẩm cùng dòng
`}` của `#[cfg(test)] fn`/`mod` ⇒ `--strict` **ĐỎ**. Log: `kt197-logs/f2b-leak.log`.

---

## 4. F3 — truyền lỗi toàn luồng (ĐẠT cho `v2/kernel`+`v2/sim`)

### 4.1 Rà caller

| Caller | Xử lý lỗi | Kết luận |
|---|---|---|
| `scan_lexical` (`:1120`) | báo mọi `parse_file` error của `v2/kernel`+`v2/sim`+`v2/lib` | **đạt** |
| `scan_structure` (`:1128`) | `_walk_items` errors → `E_LEX_STRUCT`; bỏ qua tệp đã có lỗi từ vựng (đã báo) | **đạt** |
| `_mods_of` (`:1276`) | `parse_file` lỗi ⇒ trả `[]` **im lặng**; `attr_path_edges` `LexError` ⇒ `continue` (`:1310`) | **nuốt**, nhưng `scan_lexical`/`resolve_include_targets` báo cùng lỗi ⇒ tổng thể ĐỎ |
| `resolve_include_targets` (`:1403`) | `parse_file` lỗi ⇒ `continue` (đã báo); `attr_path_edges` `LexError` ⇒ **append error** (`:1448-1452`) | **đạt** (F3 fix) |
| `scan_lib_macros` (`:1627`) | `parse_file` lỗi ⇒ `continue` (đã báo); `_check_macro_body` `LexError` ⇒ `E_STRICT_MACRO_INCLUDE` | **đạt** |
| `scan_kernel_macro_includes` (`:1671`) | như trên | **đạt** |
| `_shadowed_safe_macros` (`:1726`) | `parse_file` lỗi ⇒ `continue` (đã báo) | **đạt** |

### 4.2 Biên độ sâu

| Ca | N | thường | `--strict` | Mã |
|---|---|---|---|---|
| `cfg_attr` lồng, dưới giới hạn | 31 | **ĐỎ** (luật `wake`) | **ĐỎ** | `E_STRICT_TEST_ONLY_WAKE` |
| `cfg_attr` lồng, **tại** giới hạn | 32 | **ĐỎ** (luật `wake`) | **ĐỎ** | `E_STRICT_TEST_ONLY_WAKE` |
| `cfg_attr` lồng, **vượt** | 33 | **ĐỎ** | **ĐỎ** | `E_LEX_DEPTH` (`E_STRICT_WAKE_INCLUDE … E_LEX_DEPTH`) |
| `cfg_attr` lồng, vượt xa | 34, 40 | **ĐỎ** | **ĐỎ** | `E_LEX_DEPTH` |
| ngoặc lồng, dưới | 126 | XANH | XANH | — |
| ngoặc lồng, **tại/vượt** | 127, 128, 140 | **ĐỎ** | **ĐỎ** | `E_LEX_DEPTH` |

⇒ `E_LEX_DEPTH` **còn nhận diện được**, exit≠0 ở **cả hai** chế độ; biên độ đúng (32/33 cho
`cfg_attr`, 127/128 cho ngoặc — tính cả thân hàm). Log: `kt197-logs/f3-depth.log`.

### 4.3 Khoảng trống còn lại (ghi, không chặn)

- `cfg_attr` 40 tầng trong **`v2/lib` ngoài thân macro** ⇒ **XANH cả hai chế độ** (không caller
  nào gọi `attr_path_edges` trên `v2/lib` ngoài macro). Hợp đồng §1 giới hạn `v2/lib` ở "thân
  `macro_rules!`", nên có thể coi là ngoài phạm vi — nhưng §3 lại ghi "quá độ sâu ⇒ `E_LEX_*`"
  không kèm điều kiện. **Cần một dòng hợp đồng nói rõ** (hoặc gọi `attr_path_edges` cho `v2/lib`).
- `_mods_of` vẫn nuốt `LexError` (`:1310`); hiện được `resolve_include_targets` che, nhưng nếu
  sau này `_mods_of` được dùng độc lập thì lỗi sẽ mất. Nên sửa `_mods_of` trả lỗi hoặc ghi rõ.

---

## 5. F4 — inline module

| Ca | thường | `--strict` | Kết luận |
|---|---|---|---|
| `#[path = "sub"] mod m { mod child; }`, `sub/child.rs` sạch | XANH | XANH | **ĐẠT** — không báo oan |
| như trên, `sub` **không tồn tại** | **ĐỎ** `E_STRICT_WAKE_INCLUDE … không tồn tại` | **ĐỎ** | **ĐẠT** — fail-closed, không im lặng |
| `#[cfg(test)] #[path = "sub"] mod m { mod child; }`, `sub/child.rs` gọi `wake` | XANH | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE sub/child.rs:4` | **F4b — báo oan** (con test-only bị coi sản phẩm) |
| `#[path = "sub"] mod m;` (không-inline), `sub` là thư mục | XANH | XANH | **F4c — bỏ qua im lặng** (rustc sẽ lỗi) |

**F4b — nguyên nhân**: `_walk_items` đệ quy inline module với `cur_dir / name_tok.val`
(`:1071`), **không** dùng thư mục `#[path]` (`sub`). `_mods_of` bỏ qua inline (`:1301`) nên
không có cạnh; `sub/child.rs` không vào đồ thị module ⇒ bị coi **sản phẩm**. Cây thật **không**
dùng dạng này (hợp đồng §4 đã ghi), nên đây là **báo oan tiềm ẩn**, không phải lỗi đang đỏ.

**F4c — nguyên nhân**: `resolve_include_targets` bỏ qua **mọi** `target.is_dir()` (`:1484-1489`),
không phân biệt inline (`mod m { … }`) với không-inline (`mod m;`). Với `mod m;`, rustc báo lỗi
(E0583) nhưng checker **im lặng**. Không phải đường lọt (cây không biên dịch), nhưng trái tinh
thần "cạnh nguồn không phân giải ⇒ lỗi rõ" (D2-166 §3C). Nên chỉ bỏ qua khi `mod` là **inline**.

---

## 6. Hợp đồng 4 mục — đối chiếu từng dòng

| Dòng hợp đồng | Code @ `d6468563` | Đúng? |
|---|---|---|
| §2 `include!` ba dấu ngoặc + `include ! ( … )` | `_walk_items` `:1022-1035`; `on_include` `:1457-1466` | **ĐÚNG** (đã thử 4 dạng) |
| §2 chuỗi thường/raw `r"…"`, `r#"…"#`, `r##"…"##` | `_RE_LEX` `:507`, `_decode_string` `:525` | **ĐÚNG** (đã thử) |
| §2 escape `\x2e`, `\u{2e}`, `\n`, `\t`, `\\`, `\"` | `_decode_string` `:525-590` | **ĐÚNG** (đã thử `\x2e`, `\u{2e}`) |
| §2 `#[path]`, `#[ path ]`, `#[\npath]`, `#[r#path]` | `_walk_items` `:1001-1018` + `parse_attr` `:835` | **ĐÚNG** (đã thử) |
| §2 `cfg_attr` lồng (MAX_ATTR_DEPTH) | `attr_path_edges` `:858-912` | **ĐÚNG** (biên 32/33) |
| §2 `mod NAME;` + `#[cfg(test)]`/`#[cfg(feature = "ktest")]` | `_mods_of` `:1250`; `cfg_is_test_tokens` `:914` | **ĐÚNG** (đã thử) |
| §2 **Ngữ cảnh test theo item/token-tree** | `_mods_of` `:1291-1300` **đúng**; `_check_wake_text` `:2066` **vẫn theo dòng** | **SAI một phần** — xem F2b |
| §2 `macro_rules!` mọi tên + ba dấu thân | `_walk_items` `:1022-1037` | **ĐÚNG** (đã thử `{}`, `()`, `[]`) |
| §2 chú thích chen giữa `#`/`[`/`path` | tokenizer bỏ comment `:628-656` | **ĐÚNG** (đã thử `#/*x*/[path]`) |
| §2 `#[path = "sub"] mod m { … }` inline module | `resolve_include_targets` `:1484-1489` | **ĐÚNG** cho ca chính; **F4b/F4c** là giới hạn |
| §3 `include!`/`#[path]`/`cfg_attr(path)` không literal | `attr_path_edges` `:889-891`; `on_include` `:1457-1466` | **ĐÚNG** (đã thử `concat!`) |
| §3 đích ra ngoài cây | `_inside_scan_tree` `:1392` | **ĐÚNG** (đã thử `../../etc/passwd`) |
| §3 đích không tồn tại (và không phải thư mục) | `:1490-1495` | **ĐÚNG** (đã thử) |
| §3 thân macro chứa `include`/`path`/`mod` (bất kỳ dạng) ⇒ `E_STRICT_MACRO_INCLUDE` | `_check_macro_body` `:1514` — `mod NAME;` phát `E_STRICT_MACRO_MOD` | **SAI một phần** (dòng kế §3:44 nói rõ `MOD`; dòng này nên ghi "trừ `mod` literal") |
| §3 `$…:tt` ⇒ `E_STRICT_MACRO_TT` | `:1584-1601` | **ĐÚNG** (đã thử) |
| §3 `mod …;` ⇒ `MOD` | `:1548-1580` | **ĐÚNG** (đã thử) |
| §3 `wake(` trong sản phẩm thuộc `TEST_ONLY_WAKE_SCOPE` | `in_test_only_wake_scope` `:174` | **ĐÚNG** (F1/F2) |
| §3 chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá sâu ⇒ `E_LEX_*` | `tokenize`/`build_tree`/`attr_path_edges` | **ĐÚNG** cho `v2/kernel`+`v2/sim`; **v2/lib ngoài macro** còn hở (F3 §4.3) |
| §4 F2 (một phần): "Ngữ cảnh test **đã** theo item/token-tree" | `_check_wake_text` `:2066` vẫn theo dòng | **SAI** — xem F2b |
| §4 Macro invocation: `_walk_items` bỏ qua nhóm sau `!`; cây thật **không** có lời gọi macro chứa `mod`/`#[path]` | `:979-984`; probe độc lập trên `v2/kernel`+`v2/sim`+`v2/lib`: **0** hit | **ĐÚNG** (đã kiểm độc lập) |
| §4 `#[path]` trong `v2/lib` không được phân giải | `resolve_include_targets` chỉ lặp `v2/kernel`+`v2/sim` | **ĐÚNG** |
| §5 macro expansion / proc-macro / build.rs / cfg phức tạp / symlink | không mô phỏng; không có `paste`/`concat_idents` trong cây | **ĐÚNG** (đã grep) |
| §5 "Code nằm ngoài `v2/kernel`/`v2/sim` — không quét" | `scan_lib_macros` **có** quét `v2/lib` (thân macro) | **SAI** — mâu thuẫn §1 |

Log đối chiếu: `kt197-logs/contract.log`.

**Kết luận §6**: hợp đồng đã tốt hơn bản cũ (tách 4 mục, ghi F2/F4/macro), nhưng còn **ba dòng
sai** (§2 "ngữ cảnh test theo item/token-tree" khi luật `wake` vẫn theo dòng; §3 dòng
`E_STRICT_MACRO_INCLUDE` cho `mod`; §5 "ngoài v2/kernel/v2/sim không quét") và **thiếu** F2b,
F4b, F4c trong §4.

---

## 7. Biến thể mới chưa nằm trong selftest (ĐẠT)

Đã thử trên cây thật (đột biến + đối chứng), tất cả đúng:

| Nhóm | Dạng | Kết quả |
|---|---|---|
| delimiter | `include! ( "…" )`, `include!["…"]`, `include!{"…"}` | **ĐỎ** (đúng) |
| comment/whitespace | `#/*x*/[path]`, `#\n[path]`, `#[ path = "…" ]` | **ĐỎ** (đúng) |
| identifier | `mod r#evil_x;`, `mod 日本;` | **ĐỎ** (đúng) |
| attribute | `#[path = r"…"]`, `#[cfg_attr(not(test), path)]`, `#[cfg_attr(test, path)]`, `cfg_attr` 2 tầng | **ĐỎ** (đúng) |
| nesting | `cfg_attr` 31/32/33/34/40; ngoặc 126/127/128/140 | đúng biên (xem §4.2) |
| token forwarding | `$…:tt`, `mod …;`, `include!`, `#[path]` trong thân macro | **ĐỎ** `E_STRICT_MACRO_TT`/`MOD`/`INCLUDE` (đúng) |
| đối chứng dương | `#[cfg(test)] mod evil;`; `#[cfg(feature = "ktest")] mod evil;`; `macro_rules!` thân rỗng `()`/`[]` | **XANH** (đúng) |

---

## 8. `v2 check` + `--selftest` trên cây sạch (ĐẠT)

```
$ ./v2/bin/v2 check
--- fmt: OK
--- clippy: OK
--- luật cấu trúc: OK
--- self-test luật: OK
--- luật máy K6: OK
--- wake paths (KT-151): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155): OK
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)          RC=0
```

`python3 v2/tools/check-wake-paths.py --selftest` → `# selftest: 113/113 ok` RC=0.
`python3 v2/tools/check-wake-paths.py` → `errors=0` RC=0; `--strict` → `errors=0` RC=0.
Log: `kt197-logs/v2check.log`, `kt197-logs/selftest.log`, `kt197-logs/clean-normal.log`, `kt197-logs/clean-strict.log`.

---

## 9. Việc **chưa** review / giới hạn của review này

- **Không** chạy `gate --fast` (ràng buộc task) — chỉ `v2 check`. L1 1.000 seed **chưa** kiểm.
- **Không** chạy QEMU (ràng buộc task).
- **Không** kiểm `v2/lib` `#[path]`/`include!` ngoài thân macro (hợp đồng §4 đã ghi ngoài phạm vi).
- **Không** kiểm proc-macro thật (không có trong cây).
- **Không** đối chiếu từng ca cũ 76 với bản trước KT-194 (chỉ xác nhận 113/113 và docstring).
- **Không** sửa mã sản phẩm, không sửa checker, không nới ngưỡng.
- **F2b** là đường lọt **thật** đã kiểm bằng `cargo check` RC=0; **chưa** dựng ca qua `v2 check`
  đầy đủ (vì `cargo fmt --check` sẽ đỏ vì đột biến một dòng — không cần thiết: `v2 check` chạy
  đúng `--strict` tại `v2/bin/v2:321`).

---

## 10. Ca hồi quy cần lưu (đề nghị, **chưa** thêm vào selftest — reviewer không sửa checker)

1. **F2b**: `#[cfg(test)] fn h() {} pub fn evil(...) { m.wake(t); }` (cùng dòng) ⇒ `--strict`
   **ĐỎ**; đối chứng `pub fn evil` xuống dòng ⇒ **ĐỎ**; `wake` trong `#[cfg(test)] fn` ⇒ **XANH**.
2. **F2b-mod**: `#[cfg(test)] mod tests { } pub fn evil(...) { m.wake(t); }` (cùng dòng) ⇒
   `--strict` **ĐỎ**.
3. **F4b**: `#[cfg(test)] #[path = "sub"] mod m { mod child; }` + `sub/child.rs` gọi `wake` ⇒
   **XANH** (test-only).
4. **F4c**: `#[path = "sub"] mod m;` + `sub` là thư mục ⇒ **ĐỎ** `E_STRICT_WAKE_INCLUDE`
   (không im lặng).
5. **F3-v2lib**: `cfg_attr` 40 tầng trong `v2/lib` ngoài macro ⇒ **ĐỎ** `E_LEX_DEPTH` (hoặc
   hợp đồng ghi rõ ngoài phạm vi).
6. **F1**: giữ 6 ca repro × hai bố trí × hai chế độ (đã có một phần trong selftest; bổ sung
   bố trí `lib.rs` + chế độ thường).

---

## 11. Kết luận

**KT-194: KHÔNG ĐẠT** trên HEAD `d6468563`.

- **Đạt**: F1 (6/6 repro đỏ cả hai chế độ, `cargo check` RC=0); F3 cho `v2/kernel`+`v2/sim`
  (biên độ sâu đúng, `E_LEX_DEPTH` nhận diện, exit≠0); sáu ca `mod` của F2 (token-tree,
  `_mods_of` đúng, đột biến thật); F4 ca chính (không báo oan, `sub` thiếu ⇒ ĐỎ); biến thể
  delimiter/comment/identifier/attribute/nesting/token-forwarding; 113/113 selftest; `v2 check`
  PASS.
- **Không đạt**: **F2b** — luật `wake` vẫn quyết định ngữ cảnh test theo **dòng**; sản phẩm cùng
  dòng `}` của mục `#[cfg(test)]` bị miễn, `--strict` XANH trong khi `cargo check` RC=0 và sản
  phẩm biên dịch `wake()` (NẶNG). **F4b** — con test-only của inline module bị báo oan
  (TRUNG BÌNH). **F4c** — `#[path]` trên `mod` không-inline trỏ thư mục bị bỏ qua im lặng (NHẸ).
  **F5** — hợp đồng còn ba dòng sai và thiếu F2b/F4b/F4c trong §4.
- **KT-194 xong không tự làm KT-184 DONE.** KT-184 vẫn `REVIEW`; F2b phải đóng trước khi xét
  đóng KT-194/KT-184.

**Ký**: reviewer độc lập L35, 2026-11-19.
