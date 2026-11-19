# KT-195 — review đối kháng KT-194 (lexer/token-tree) theo D2-166

**Ngày**: 2026-11-19 · **Reviewer độc lập L34** · nhánh `night2/L34-review194` ·
worktree `/private/tmp/night2-L34-review194` · HEAD **`275c78b4`** (từ `main` @ `275c78b4`).
**Nguồn đọc**: D2-166 (`v2/agent/DECISIONS.md:309`), [`CHECKER-WAKE-CONTRACT.md`](../reports/CHECKER-WAKE-CONTRACT.md),
[`CHECKER-LEXER-PROPOSAL.md`](../reports/CHECKER-LEXER-PROPOSAL.md), [`kt194-lexer.md`](kt194-lexer.md)
(cả **ĐÍNH CHÍNH của lead**), [`kt192-review184v4.md`](kt192-review184v4.md), `v2/tools/check-wake-paths.py` @ `275c78b4`.
**Ngân sách**: hạn cứng 16:30 JST. Không merge/push/`gate --fast`/QEMU. Mọi đột biến dựng trên
**bản sao cây thật** (`/tmp/kt195-base`, rsync từ worktree, không `target/`), harness **tự dựng**
(không dùng `_fixture_v2` cho ba repro), không sửa mã sản phẩm trên `main`.
**Vai**: reviewer độc lập theo D2-159 — không phải lead, không phải tác giả.

---

## 0. Phán quyết: **KHÔNG ĐẠT**

Lexer + token-tree là **bước tiến thật**: ba repro D2-166 §2 **đã đỏ** (với cách bố trí của tác
giả), tầng từ vựng vững, §3C đầu vào hỏng **có** đỏ trên cây thật, §3D **không** báo oan. Nhưng
còn **ba lỗi nặng** (hai là **đường lọt thật đã kiểm bằng `cargo check` RC=0**) và **một báo oan**:

| # | Lỗi | Mức | Bằng chứng |
|---|---|---|---|
| **F1** | **Vá chế độ thường phụ thuộc vị trí `#[path]`**: cùng ba repro, đặt thuộc tính ở **`lib.rs`** (crate root) ⇒ chế độ **thường XANH**, `--strict` ĐỎ; đặt ở `core_ipc.rs` (module con) ⇒ thường ĐỎ. Tức câu "ba repro đỏ **cả hai** chế độ" của tác giả **không tổng quát**; câu "thường XANH" của lead **đúng cho `lib.rs`**. | **NẶNG NHẤT** | §2, log `f1-libpath.log` |
| **F2** | **`mod` cùng dòng `}` đóng khối `#[cfg(test)]`** bị xếp **test-only** ⇒ `--strict` (và `v2 check`/`gate --fast`) **XANH** trong khi sản phẩm biên dịch `wake()`. | **NẶNG** | §3, log `f2-h1.log` |
| **F3** | **`cfg_attr` lồng quá `MAX_ATTR_DEPTH` bị nuốt lỗi**: `attr_path_edges` ném `E_LEX_DEPTH` nhưng **cả hai** call site bắt-rồi-bỏ; `scan_structure` **không** gọi `attr_path_edges`. Đầu-cuối: **XANH cả hai chế độ**, `cargo check` RC=0, sản phẩm biên dịch `wake()`. Vi phạm thẳng D2-166 §3C/§4 ("quá độ sâu ⇒ lỗi rõ + exit khác 0"). | **NẶNG** | §4, log `f3-attrdeep.log` |
| **F4** | **Báo oan `#[path]` trên inline module**: `#[path = "sub"] mod m { … }` là Rust **hợp lệ** (đặt thư mục cho module con), nhưng `resolve_include_targets` coi `sub` là **tệp** ⇒ `E_STRICT_WAKE_INCLUDE` "không tồn tại". `_mods_of` xử lý đúng (bỏ qua inline), `resolve_include_targets` **không** — hai đường lệch nhau. | TRUNG BÌNH | §11, log `f4-inline-path.log` |

Ngoài ra: hợp đồng **cũ** so với code (§5), selftest có **ca tautology** và **thiếu ca chế độ
thường** (§6). Theo D2-165 §1(e) đây vẫn là **cùng bản chất** "quyết định bằng văn bản thô thay vì
token" — KT-194 thay được lớp **phân giải `#[path]`/`include!`**, **chưa** thay lớp **quyết định
ngữ cảnh test** (`test_line_ranges`) và còn **nuốt lỗi độ sâu**.

**KT-194 xong KHÔNG tự làm KT-184 DONE.**

---

## 1. Phương pháp

- Bản sao cây thật: `rsync -a --exclude target --exclude .git /private/tmp/night2-L34-review194/ /tmp/kt195-base/`.
- Mỗi ca: `rsync` bản sạch → thêm tệp/đột biến → chạy `python3 v2/tools/check-wake-paths.py`
  (thường) và `--strict`; đo `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core
  --offline` với `RUSTUP_TOOLCHAIN=1.99.1` (toolchain ghim của `v2/kernel/rust-toolchain.toml`).
- Đối chứng dương: tệp đích dùng `wake_from` thay `wake` ⇒ phải **XANH** (C0).
- Không dùng `_fixture_v2` cho ba repro (harness tự dựng trên cây thật). `_fixture_v2` chỉ dùng
  cho phép thử biến thể nhanh, và **không** dùng làm bằng chứng phán quyết.

---

## 2. F1 — ba repro: kết quả **phụ thuộc vị trí** `#[path]` (NẶNG NHẤT)

### 2.1 Dựng lại độc lập (bố trí của tác giả: `#[path]` trong `core_ipc.rs`)

`evil_t.rs` = tệp **test-only** qua `#[cfg(test)] mod evil_t;` ở `lib.rs`, thân gọi `m.wake(t)`;
`core_ipc.rs` thêm thuộc tính + `mod evil_t_product;`.

| # | Dạng | thường | `--strict` | `cargo check` |
|---|---|---|---|---|
| R1 | `#[r#path = "evil_t.rs"]` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE evil_t.rs:7` | **ĐỎ** | RC=0 |
| R2 | `#[path = "evil_t\x2ers"]` | **ĐỎ** | **ĐỎ** | RC=0 |
| R3 | `#[cfg_attr(all(not(test)), cfg_attr(not(test), path = "evil_t.rs"))]` | **ĐỎ** | **ĐỎ** | RC=0 |
| C0 | đối chứng `#[r#path]` → tệp `wake_from` | **XANH** | **XANH** | — |

⇒ Với bố trí này, **báo cáo gốc của tác giả đúng**: ba repro đỏ **cả hai** chế độ. **ĐÍNH CHÍNH
của lead** (dòng 81–101 của `kt194-lexer.md`) **không tái lập** với repro mô tả trong chính journal
(dòng 23–26: "test-only qua `#[cfg(test)] mod evil_t;` trong `lib.rs`").

**Nhưng phải tách hai phần của ĐÍNH CHÍNH**:
- **Đo** "thường XANH" — **tái lập được** nếu `#[path]` đặt trong **`lib.rs`** (§2.2). Nhiều khả
  năng lead dựng theo bố trí KT-192/L30 (thuộc tính trong `lib.rs`), không theo bố trí journal
  (`core_ipc.rs`).
- **Giải thích** "luật `wake` là **strict-only** theo D2-144" — **SAI** với code @ `275c78b4`:
  khối `if not strict:` (`:2479-2491`) **có** chạy luật `wake` ở chế độ thường cho tệp `extra`
  test-only-bị-kéo-vào-sản-phẩm. Nếu luật thật sự strict-only thì **cả hai** bố trí đều phải XANH;
  thực tế `core_ipc.rs` ĐỎ. Vậy ĐÍNH CHÍNH **đúng kết quả cho một bố trí, sai lý do**.
- Phần "selftest ba ca `_repro` chỉ gọi `strict=True`" — **ĐÚNG** (`:4441`).

### 2.2 Cùng ba repro, `#[path]` đặt trong **`lib.rs`** (bố trí của KT-192/L30)

```
# lib.rs (thêm vào cuối)
#[cfg(test)]
mod evil_t;                       // evil_t.rs test-only
#[r#path = "evil_t.rs"]
mod evil_x;                       // rustc: nạp evil_t.rs ở BẢN SẢN PHẨM
```

| Chế độ | Kết quả | Log |
|---|---|---|
| thường | **XANH** `errors=0` RC=0 | `f1-libpath.log` |
| `--strict` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE evil_t.rs:4` RC=1 | `f1-libpath.log` |
| `cargo check -p vifast-core` | **RC=0** (chỉ warning `dead_code` tại `evil_t.rs:4`) | `f1-libpath.log` |

⇒ **ĐÍNH CHÍNH của lead đúng cho bố trí `lib.rs`**; báo cáo gốc đúng cho bố trí `core_ipc.rs`.
**Cả hai đều là repro hợp lệ** — và điều đó phơi ra **lỗi logic của bản vá chế độ thường**.

### 2.3 Nguyên nhân (file:dòng)

`v2/tools/check-wake-paths.py:2479-2491` (khối `if not strict:`):

```python
for relp, (p, is_test) in sorted(extra.items()):
    if is_test or relp not in orig_test_only:   # ← :2482
        continue
```

`orig_test_only` (`:2164`) là `set(test_only)` **sau** `test_only_modules(root)` (`:2146`). Với
`#[path]` trong **`lib.rs`**, `_mods_of` (`:1243`) phân giải được cạnh (`cur_dir` = `p.parent` =
`src/`) ⇒ `evil_t.rs` **đã** thành sản phẩm trong `test_only_modules` ⇒ `orig_test_only` **không**
chứa nó ⇒ khối chế độ thường **bỏ qua** ⇒ XANH.

Với `#[path]` trong **`core_ipc.rs`** (module con), `_mods_of` dùng `cur_dir = mod_dir` =
`src/core_ipc/` (`:1301`) — **sai** so với luật rustc (`#[path]` giải theo **thư mục tệp chứa**,
không theo thư mục module) ⇒ cạnh trỏ `src/core_ipc/evil_t.rs` (không tồn tại) ⇒ `evil_t.rs` **vẫn**
test-only trong `test_only_modules` ⇒ `orig_test_only` chứa nó ⇒ khối chế độ thường **chạy** ⇒ ĐỎ.

Tức **bản vá chế độ thường chỉ hoạt động khi `_mods_of` phân giải SAI cạnh `#[path]`**. Đây là
quan hệ ngược, không phải thiết kế. Ghi chú: `resolve_include_targets` (`:1454,1461`) dùng
`p.parent` — **đúng**; `_mods_of` (`:1301`) dùng `mod_dir` — **sai**. Hai đường lệch nhau.

**Đề nghị sửa tối thiểu** (không tự sửa): bỏ điều kiện `relp not in orig_test_only` — chạy luật
`wake` trên **mọi** tệp `extra` có `is_test=False` (đúng fail-closed, không nới ngưỡng); và sửa
`_mods_of` dùng `p.parent` cho cạnh `#[path]` (hoặc ghi rõ giới hạn nếu cố ý).

---

## 3. F2 — `mod` cùng dòng `}` của khối `#[cfg(test)]` ⇒ lọt `--strict` (NẶNG)

```
# lib.rs (thêm vào cuối)
#[cfg(test)]
fn kt195_helper() {
    let _ = 1;
} mod evil;                       // rustc: mod evil; là MỤC RIÊNG, luôn biên dịch
# evil.rs
pub fn evil_wake(m: &mut Machine, t: ObjId) { let _ = m.wake(t); }
```

| Chế độ | Kết quả | Log |
|---|---|---|
| thường | **XANH** `errors=0` RC=0 | `f2-h1.log` |
| `--strict` | **XANH** `errors=0` RC=0 | `f2-h1.log` |
| `cargo check -p vifast-core` | **RC=0** (warning `dead_code` tại `evil.rs:4` ⇒ tệp **có** được biên dịch vào sản phẩm) | `f2-h1.log` |
| đối chứng H2 (`mod evil;` **xuống dòng**) | thường XANH, `--strict` **ĐỎ** `E_STRICT_TEST_ONLY_WAKE evil.rs:6` | `f2-h1.log` |

**Nguyên nhân**: `test_line_ranges` (`:364`) tính khoảng dòng test bằng **văn bản**; `end` là dòng
có `}` đóng khối (`:409-415`). `_attrs_test_context` (`:942`) dùng `in_ranges(line, tr)` ⇒ dòng
chứa `} mod evil;` nằm trong vùng test ⇒ `mod evil;` bị xếp **test-only** ⇒ `evil.rs` được miễn
luật `wake`. Đây là **cùng bản chất** D2-166 §3D ("module declaration ≠ từ trong ngữ cảnh khác"):
quyết định ngữ cảnh test **vẫn** bằng dòng văn bản, không bằng token-tree.

**Lưu ý phạm vi**: lỗi này **có trước** KT-194 (KT-165), nhưng KT-194 **không** thay lớp này và
**không** công bố nó trong §5 "Còn thiếu / giới hạn" của journal hay §4 hợp đồng. D2-166 §3D đòi
nhận dạng **theo cấu trúc**; đây là chỗ còn lệch.

---

## 4. F3 — `cfg_attr` quá `MAX_ATTR_DEPTH`: lỗi bị **nuốt** (NẶNG)

```
# lib.rs: #[cfg(test)] mod evil_t;   (evil_t.rs test-only, thân gọi wake)
# core_ipc.rs:
#[cfg_attr(not(test), cfg_attr(not(test), … 40 tầng … path = "evil_t.rs"))]
mod evil_p;
```

| Chế độ | Kết quả | Log |
|---|---|---|
| thường | **XANH** `errors=0` RC=0 | `f3-attrdeep.log` |
| `--strict` | **XANH** `errors=0` RC=0 | `f3-attrdeep.log` |
| `cargo check -p vifast-core` | **RC=0** (warning `dead_code` tại `evil_t.rs:4` ⇒ sản phẩm **có** biên dịch `wake()`) | `f3-attrdeep.log` |

**Nguyên nhân**:
- `attr_path_edges` ném `E_LEX_DEPTH` khi `depth > MAX_ATTR_DEPTH` (`:871-878`).
- `_mods_of.on_mod` bắt `LexError` rồi `continue` (`:1295-1298`).
- `resolve_include_targets.on_attr` bắt `LexError` rồi `return`, kèm chú thích **sai**
  "Lỗi cấu trúc đã được `scan_structure` báo" (`:1436-1438`).
- `scan_structure` (`:1121-1137`) gọi `_walk_items` **không** có `on_attr` ⇒ **không bao giờ** gọi
  `attr_path_edges` ⇒ lỗi **không** được báo ở đâu cả.

⇒ Vi phạm D2-166 §3C ("quá độ sâu ⇒ lỗi rõ + exit khác 0") và §4 ("quá độ sâu ⇒ lỗi"). Ca
selftest "KT-194a `cfg_attr` lồng quá `MAX_ATTR_DEPTH` ⇒ `E_LEX_DEPTH`" (`:4295-4314`) gọi
`attr_path_edges` **trực tiếp** — **tautology ở tầng tích hợp**: nó chứng minh hàm ném lỗi, không
chứng minh `scan_v2` báo lỗi. Đây đúng là ca cần thêm vào selftest (qua `scan_v2`).

---

## 5. Hợp đồng vs code — từng dòng §2/§3/§3b

| Dòng hợp đồng | Code @ `275c78b4` | Đúng? |
|---|---|---|
| §2:24 `include!` ba dấu ngoặc + `include ! ( … )` | `_walk_items` `:1038-1050` | **ĐÚNG** |
| §2:25 chuỗi thường/raw `r"…"`, `r#"…"#`, `r##"…"##` | `_RE_LEX` `:507`, `:657-660` | **ĐÚNG** |
| §2:26 `#[path]`, `#[ path ]`, `#[\npath]` | `_walk_items` `:1001-1018` + `parse_attr` `:835` | **ĐÚNG** |
| §2:27 `cfg_attr(not(test), path)` một tầng | `attr_path_edges` `:893-910` | **ĐÚNG** |
| **§2:28-30** "**CHƯA** hỗ trợ `#[r#path]`, escape, `cfg_attr` lồng — ba dạng này **lọt** checker hiện tại" | code **đã** hỗ trợ cả ba (`:879-892`, `:525-590`, `:893-910`); selftest 194b/194c chứng minh | **SAI / CŨ** — D2-166 §2 đòi sửa; KT-194 **không** sửa hợp đồng |
| §2:31 `mod NAME;` + `#[cfg(test)]`/`#[cfg(feature="ktest")]` | `_mods_of` `:1243`, `_attrs_test_context` `:942` | **ĐÚNG** (trừ F2) |
| §2:32 `macro_rules!` mọi tên + ba dấu thân | `_walk_items` `:1022-1037` | **ĐÚNG** |
| §2:33 chú thích chen giữa `#`/`[`/`path` | tokenizer bỏ comment `:628-656` | **ĐÚNG** |
| §3:39 `include!`/`#[path]`/`cfg_attr(path)` không literal | `attr_path_edges` `:889-891`, `on_include` `:1449-1452` | **ĐÚNG** |
| §3:40 đích ra ngoài cây | `_inside_scan_tree` `:1379` | **ĐÚNG** |
| §3:41 đích không tồn tại | `:1469-1474` | **ĐÚNG** |
| §3:42 thân macro chứa `include`/`path`/`mod` (bất kỳ dạng) ⇒ `E_STRICT_MACRO_INCLUDE` | `_check_macro_body` `:1494` — `mod NAME;` phát `E_STRICT_MACRO_MOD` | **ĐÚNG một phần** (dòng kế §3:44 nói rõ `MOD`; dòng 42 nên ghi "trừ `mod` literal") |
| §3:43 `$…:tt` ⇒ `E_STRICT_MACRO_TT` | `:1584-1601` | **ĐÚNG** |
| §3:44 `mod …;` ⇒ `MOD` (literal) / `INCLUDE` (`mod $n;`, `mod r#x;`) | `:1548-1580` | **ĐÚNG** |
| §3:45 `wake(` trong sản phẩm thuộc `TEST_ONLY_WAKE_SCOPE` | `in_test_only_wake_scope` `:174` | **ĐÚNG** (v2/lib ngoài phạm vi — đã ghi) |
| **§3b:56-58** ba dạng **LỌT** cả hai chế độ | **SAI / CŨ** — đã đỏ (R1–R3) | **SAI** |
| **§3b:60** "Ba dạng này sẽ được xử lý ở KT-194" | đã xử lý | **CŨ** |
| §4:64-72 điều **KHÔNG** bảo đảm | thiếu: (a) `#[path]`/`include!` trong **lời gọi macro** bị `_walk_items` bỏ qua (`:979-984`); (b) `#[path]` trong `v2/lib` ngoài thân macro; (c) **ngữ cảnh test quyết định bằng khoảng dòng** (F2); (d) **`cfg_attr` quá sâu bị nuốt** (F3) | **THIẾU** |
| §5:77-82 ba chế độ chạy trong `v2 check` + `gate --fast` | `v2/bin/v2:314,315,321` | **ĐÚNG** (đã kiểm) |
| §6:87 "mỗi ca có **đối chứng dương**" | 31 ca mới: chỉ 3 repro có C0; 13 ca lexer + 5 ca đầu vào hỏng **không** có đối chứng dương | **SAI** |

**Kết luận §5**: hợp đồng **chưa** được cập nhật theo KT-194 — §2:28-30 và §3b:56-60 nói ngược
code; §4 thiếu bốn giới hạn đang có thật. D2-166 §2 đòi sửa hợp đồng ngay; D2-165 §1(a) đòi ghi
"điều **không** bảo đảm".

---

## 6. Selftest — tautology và thiếu ca

- `--selftest`: **107/107 ok** (log `selftest.log`), không `#[ignore]`, không `#[should_panic]`.
- **Ca tautology tầng tích hợp**: `cfg_attr` quá `MAX_ATTR_DEPTH` (`:4295-4314`) gọi
  `attr_path_edges` trực tiếp; `scan_v2` **không** báo (F3). Tương tự, ca delimiter (`:4246-4254`)
  và ca độ sâu cây (`:4283-4293`) gọi `build_tree` trực tiếp — chấp nhận được vì `parse_file`
  thật sự gọi `build_tree`, nhưng ca `cfg_attr` thì **không** có đường tích hợp tương ứng.
- **Ba ca `_repro` chỉ chạy `strict=True`** (`:4441`) — **không** có ca chế độ thường. Đây chính
  là lý do F1 không bị bắt. D2-166 §5 đòi tầng chính sách; tầng đó phải kiểm **cả hai** chế độ.
- **Thiếu ca cho F2** (`mod` cùng dòng `}`) và **F3** (qua `scan_v2`).
- **Đối chứng dương**: 3 repro có C0 (`:4457-4475`) — tốt. Nhưng 13 ca lexer + 5 ca đầu vào hỏng
  không có đối chứng dương, nên câu §6:87 của hợp đồng chưa đúng.

---

## 7. Tầng 1 — lexer (đạt)

Kiểm lại 13 ca + biến thể độc lập (probe trực tiếp `tokenize`/`build_tree`):

| Nhóm | Kết quả |
|---|---|
| raw ident `r#path` → `ident("path", raw=True)` | **ĐÚNG** |
| escape `\x2e`, `\u{2e}`, `\n`, `\\`, `\"`, `\0`, `\'` | **ĐÚNG** |
| escape sai `\q`, `\x` thiếu hex, `\u{…}` hở/ngoài Unicode | **ĐÚNG** → `E_LEX_INVALID_ESCAPE` |
| chuỗi thường/raw `r"…"`, `r#"…"#`, `r##"…"##`, `br"…"`, `cr"…"` | **ĐÚNG** |
| chuỗi thô sai số `#` / chưa đóng | **ĐÚNG** → `E_LEX_UNTERMINATED_STRING` |
| comment khối **lồng** `/* /* */ */` | **ĐÚNG** |
| comment khối chưa đóng | **ĐÚNG** → `E_LEX_UNTERMINATED_COMMENT` |
| char `'a'` vs lifetime `'b` vs `'\''` | **ĐÚNG** |
| delimiter lệch `(]`, `}` thừa, `(` hở | **ĐÚNG** → `E_LEX_UNBALANCED` |
| lồng ngoặc > `MAX_TREE_DEPTH` (128) | **ĐÚNG** → `E_LEX_DEPTH` |
| tên thuộc tính không phải định danh `#[1 = 2]` | **ĐÚNG** → `E_LEX_ATTR` |
| token giữ **vị trí nguồn** (`file/line/col/start/end`) | **ĐÚNG** (`Tok.__slots__` `:447`) |

**Không** thấy ca tautology ở tầng lexer. Một điểm nhỏ: `_RE_LEX` nhánh `num`
(`:514`) gộp `1.0` thành một token — không ảnh hưởng luật hiện tại.

---

## 8. Tầng 2 — tập cạnh nguồn (đạt)

Selftest `_edge_case` (`:4340-4363`) kiểm **đúng cạnh** `v2/kernel/vifast-core/src/evil_t.rs` với
`is_test=False`, không chỉ exit code — **đúng** yêu cầu D2-166 §5.

Kiểm độc lập thêm (probe `resolve_include_targets` trên bản sao cây thật):

| Dạng | Cạnh | Kết quả |
|---|---|---|
| `#[path = "evil_t.rs"]` | `evil_t.rs`, `is_test=False` | **ĐÚNG** |
| `#[r#path = "evil_t.rs"]` | `evil_t.rs`, `is_test=False` | **ĐÚNG** |
| `#[path = "evil_t\x2ers"]` | `evil_t.rs` (**đã giải mã**) | **ĐÚNG** |
| `cfg_attr` lồng 2 tầng | `evil_t.rs`, `is_test=False` | **ĐÚNG** |
| `include!["…"]`, `include!{"…"}` | `evil_t.rs` | **ĐÚNG** |
| `mod` tên Unicode | đúng tệp | **ĐÚNG** |
| `#[path]` trong comment/chuỗi | **không** cạnh | **ĐÚNG** |
| `#[path = concat!(…)]` | không cạnh + `E_STRICT_WAKE_INCLUDE` | **ĐÚNG** (fail-closed) |
| `#[path = "../../…/etc/x.rs"]` | `E_STRICT_WAKE_INCLUDE` (ra ngoài cây) | **ĐÚNG** |
| `include!("../../…/etc/x.rs")` | `E_STRICT_WAKE_INCLUDE` | **ĐÚNG** |
| `#[path]` trên **inline** `mod e { }` | **báo oan** `E_STRICT_WAKE_INCLUDE` (coi `sub` là tệp) | **SAI** — F4 |

**Lệch đã ghi**: `_mods_of` (`:1301`) phân giải `#[path]` theo `mod_dir` thay vì `p.parent` —
**sai** so với rustc; hiện được `resolve_include_targets` che (dùng `p.parent`), nhưng chính cái
sai này làm F1. Cần sửa hoặc ghi rõ.

---

## 9. Tầng 3 — chính sách + cây thật

- **76 ca cũ**: `--selftest` 107/107 ⇒ 76 ca cũ vẫn xanh (không đổi kỳ vọng — đã đối chiếu docstring
  `:2691-2707`).
- **Ba repro**: §2 (đỏ cả hai ở bố trí `core_ipc.rs`; thường XANH ở bố trí `lib.rs`).
- **Đối chứng hợp lệ**: C0 xanh cả hai; `#[cfg(test)] mod evil;` (một dòng, trên cây thật) ⇒
  `--strict` **XANH** `errors=0` (đúng: test-only).
- **Biến thể chưa nằm trong selftest** (đã thử, trên cây thật hoặc fixture có `lib.rs`):
  - delimiter: `include! ( "…" )`, `include!["…"]`, `include!{"…"}` — **đỏ** (đúng).
  - comment/whitespace: `#/*x*/[path]`, `#\n[path]`, `#[ path = "…" ]` — **đỏ** (đúng).
  - identifier: `mod r#evil;`, `mod 日本;`, `macro_rules! é` — **đỏ** (đúng).
  - attribute: `#[path = r"evil.rs"]`, `#[cfg_attr(not(test), path = "…")]`, `#[cfg_attr(test, path
    = "…")]` — **đỏ** (đúng, bảo thủ).
  - nesting: `cfg_attr` 2 tầng — **đỏ**; `cfg_attr` 40 tầng — **XANH** (F3, sai).
  - token forwarding: `$…:tt` trong thân macro — **đỏ** `E_STRICT_MACRO_TT` (đúng).
  - **F2** (`mod` cùng dòng `}`) — **XANH** (sai).

---

## 10. D2-166 §3C — đầu vào hỏng trên **cây thật** (đạt)

Thêm tệp hỏng vào `v2/kernel/vifast-core/src/kt195_broken.rs` trên bản sao cây thật:

| Đầu vào | thường | `--strict` | Mã |
|---|---|---|---|
| chuỗi chưa đóng | **ĐỎ** RC=1 | **ĐỎ** | `E_LEX_UNTERMINATED_STRING` `:1:17` |
| comment chưa đóng | **ĐỎ** | **ĐỎ** | `E_LEX_UNTERMINATED_COMMENT` `:1:1` |
| delimiter lệch `(]` | **ĐỎ** | **ĐỎ** | `E_LEX_UNBALANCED` `:1:11` |
| escape sai `\q` | **ĐỎ** | **ĐỎ** | `E_LEX_INVALID_ESCAPE` `:1:17` |
| lồng ngoặc 140 tầng | **ĐỎ** | **ĐỎ** | `E_LEX_DEPTH` `:1:128` |

⇒ §3C **đạt** cho năm dạng liệt kê — **trừ** "quá độ sâu" của **`cfg_attr`** (F3), dạng thứ sáu
mà D2-166 §4 nêu riêng.

---

## 11. D2-166 §3D — nhận dạng theo cấu trúc / báo oan (đạt một phần)

Tệp nhiễu trên cây thật (`kt195_noise.rs`): `use std::path::Path`, `$path:expr` trong
`macro_rules!`, `include_str!("../Cargo.toml")`, `modulo(x)`, chuỗi chứa `mod evil; #[path = "…"]
include!("…") wake(`, comment chứa cùng nội dung, raw string chứa `#[path]`.

| Chế độ | Kết quả |
|---|---|
| thường | **XANH** `errors=0` |
| `--strict` | **XANH** `errors=0` |

⇒ **Không** báo oan các dạng D2-166 §3D nêu. **Nhưng**:
- phần "module declaration ≠ từ trong ngữ cảnh khác" **chưa** đạt: F2 cho thấy khai báo `mod` vẫn
  bị xếp theo **dòng văn bản**;
- **F4** — `#[path]` trên **inline module** (Rust hợp lệ) bị coi là cạnh **tệp** ⇒ báo oan
  `E_STRICT_WAKE_INCLUDE`:

```
# core_ipc.rs — HỢP LỆ: #[path] trên inline module đặt thư mục cho module con
#[path = "sub"]
mod m { mod child; }
# sub/m/child.rs tồn tại
$ python3 v2/tools/check-wake-paths.py
E E_STRICT_WAKE_INCLUDE  …/core_ipc.rs:1: `#[path]` trỏ tới `…/src/sub` không tồn tại (fail-closed, KT-171 B6/B7)
```

`_mods_of.on_mod` (`:1289-1291`) **bỏ qua** inline module (đúng); `resolve_include_targets.on_attr`
(`:1433-1442`) **không** phân biệt ⇒ hai đường lệch nhau. Cây thật **không** dùng dạng này (21 chỗ
`#[path]` đều trên `mod …;` — đã grep), nên đây là **báo oan tiềm ẩn**, không phải lỗi đang đỏ.

---

## 12. D2-166 §4 — `cfg_attr`, macro, proc-macro

- **Cạnh chắc chắn vs có điều kiện**: `attr_path_edges` trả `certain` (`:864-865`), nhưng
  `resolve_include_targets` **bỏ** cờ này — chỉ dùng để chọn nhãn `#[path]`/`#[cfg_attr(path)]`
  trong **thông điệp lỗi** (`:1439-1440`); `extra` chỉ lưu `(target, is_test)` (`:1477`). `_mods_of`
  cũng bỏ (`:1300`). ⇒ "ghi rõ cạnh chắc chắn vs có điều kiện" **chưa** đạt ở tập cạnh; chỉ đạt ở
  nhãn chẩn đoán.
- **`cfg_attr` lồng quá độ sâu**: **F3** — lỗi bị nuốt.
- **Không suy ra test-only chỉ vì thấy chữ `test`**: `cfg_is_test_tokens` (`:914`) chỉ nhận
  `test`, `feature = "ktest"`, `all(...)` — **không** nhận `any(...)`, `not(test)` ⇒ bảo thủ
  (fail-closed). **Đạt**.
- **Macro ba loại**: `_check_macro_body` (`:1494`) phân biệt (i) đọc được + không có
  `include`/`path`/`mod`/`tt` ⇒ cho phép; (ii) cấu trúc bị cấm ⇒ `E_STRICT_MACRO_INCLUDE`/`MOD`;
  (iii) `$…:tt` chưa hỗ trợ ⇒ `E_STRICT_MACRO_TT`. **Đạt**.
- **Proc-macro**: hợp đồng §4:65-66 nói "không có trong allowlist phụ thuộc; nếu thêm, phải mở
  rộng hợp đồng". **Không** có **điểm kiểm soát cụ thể** trong checker (không grep `proc-macro`);
  đây là **tuyên bố giới hạn**, không phải cơ chế chặn. D2-166 §4 đòi "chỉ **điểm kiểm soát cụ
  thể** hoặc **hạ tuyên bố** thành 'giới hạn chưa bảo đảm'". Hợp đồng **chưa** dùng đúng câu chữ
  "giới hạn chưa bảo đảm" và **không** chỉ cơ chế nào đang giữ (allowlist D2-21 ở đâu?). ⇒ **đạt
  một phần**.

---

## 13. `v2 check` + `--selftest` trên cây sạch (đạt)

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
v2 check: PASS (L1 chạy trong gate --fast)          RC=0   (2m20s)
```

`python3 v2/tools/check-wake-paths.py --selftest` → `# selftest: 107/107 ok` RC=0.
`python3 v2/tools/check-wake-paths.py` → `errors=0` RC=0; `--strict` → `errors=0` RC=0.

---

## 14. Việc **chưa** review / giới hạn của review này

- **Không** chạy `gate --fast` (ràng buộc task) — chỉ `v2 check`. L1 1.000 seed **chưa** kiểm.
- **Không** chạy QEMU (ràng buộc task).
- **Không** kiểm `v2/lib` `#[path]`/`include!` ngoài thân macro (journal §5 đã nêu; tôi **không**
  dựng ca thật vì cần sửa crate `v2/lib` — ngoài phạm vi "không sửa mã sản phẩm").
- **Không** kiểm `#[path]`/`include!` trong **lời gọi macro** trên cây thật (cần định nghĩa macro
  mới; journal §5 đã nêu; tôi xác nhận bằng grep: **không** có `cfg_if!`/macro lời gọi chứa
  `mod`/`#[path]` trong `v2/kernel`+`v2/sim`).
- **Không** kiểm proc-macro thật (không có trong cây).
- **Không** đối chiếu từng ca cũ 76 với bản trước KT-194 (chỉ xác nhận 107/107 và docstring).
- **Không** sửa mã sản phẩm, không sửa checker, không nới ngưỡng.

---

## 15. Ca hồi quy cần lưu (đề nghị, **chưa** thêm vào selftest — reviewer không sửa checker)

1. **F1**: `#[r#path]`/escape/`cfg_attr` lồng đặt trong **`lib.rs`** (crate root) ⇒ phải **ĐỎ cả
   hai** chế độ. Hiện thường XANH.
2. **F2**: `#[cfg(test)] fn h() { … } mod evil;` (cùng dòng) ⇒ phải **ĐỎ `--strict`**. Hiện XANH.
3. **F3**: `cfg_attr` 40 tầng + `path` trỏ tệp test-only ⇒ phải **ĐỎ `E_LEX_DEPTH`** ở **cả hai**
   chế độ (qua `scan_v2`, không gọi `attr_path_edges` trực tiếp). Hiện XANH.
4. **F1b**: `_mods_of` phải phân giải `#[path]` theo `p.parent` (đối chứng: `#[path]` trong
   `core_ipc.rs` trỏ `src/evil_t.rs` ⇒ cạnh đúng).
5. **F4**: `#[path = "sub"] mod m { mod child; }` (inline) ⇒ phải **XANH** (đối chứng dương cho
   hợp đồng §2:26). Hiện ĐỎ oan.

---

## 16. Kết luận

**KT-194: KHÔNG ĐẠT.**

- **Đạt**: lexer (13 ca + biến thể), tập cạnh nguồn (đúng cạnh, không chỉ exit code), §3C đầu vào
  hỏng trên cây thật (5/5), §3D không báo oan, 107/107 selftest, `v2 check` PASS, ba repro **đỏ**
  với bố trí `core_ipc.rs`.
- **Không đạt**: F1 (vá chế độ thường phụ thuộc vị trí — câu "cả hai chế độ" của tác giả **không
  tổng quát**; ĐÍNH CHÍNH của lead đúng cho `lib.rs` nhưng **không** tái lập repro của tác giả),
  F2 (lọt `--strict` qua `mod` cùng dòng `}`), F3 (nuốt `E_LEX_DEPTH` của `cfg_attr`), F4 (báo oan
  `#[path]` trên inline module), hợp đồng cũ/thiếu (§2:28-30, §3b:56-60, §4), selftest thiếu ca
  chế độ thường + ca tautology.
- **KT-194 xong không tự làm KT-184 DONE.** KT-184 vẫn `REVIEW`; F1–F3 phải đóng trước khi xét
  đóng KT-184.

**Ký**: reviewer độc lập L34, 2026-11-19.
