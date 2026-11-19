# KT-200 — review vòng 3 KT-194 (lexer/token-tree) theo D2-167

**Ngày**: 2026-11-19 · **Reviewer độc lập L36** · nhánh `night2/L36-review199` ·
worktree `/private/tmp/night2-L36-review199` · HEAD **`98694376`** (từ `main` @ `98694376`).
**Nguồn đọc**: D2-167 (`v2/agent/DECISIONS.md`), `PHIEU-GUI-AGENT0-2026-11-19-VONG6.md`,
[`CHECKER-WAKE-CONTRACT.md`](../reports/CHECKER-WAKE-CONTRACT.md), [`kt194-lexer.md`](kt194-lexer.md),
[`kt195-review194.md`](kt195-review194.md), [`kt197-review196.md`](kt197-review196.md),
`v2/tools/check-wake-paths.py` @ `98694376`.
**Ngân sách**: hạn cứng 21:00 JST. Không merge/push/`gate --fast`/QEMU. Mọi đột biến dựng trên
**bản sao cây thật** (`/tmp/kt200-base` ← `rsync` từ worktree, không `target/`; `/tmp/kt200-case`
là bản sao làm việc), harness tự dựng (không dùng `_fixture_v2` cho F2b/F4b/F4c trên cây thật).
**Vai**: reviewer độc lập theo D2-159 — không phải lead, không phải tác giả.

---

## 0. Phán quyết: **ĐẠT CÓ ĐIỀU KIỆN**

Ba lỗi của vòng 2 **đã đóng thật** trên HEAD `98694376`:

| # | Lỗi vòng 2 | Kết quả vòng 3 | Bằng chứng |
|---|---|---|---|
| **F2b** | luật `wake` quyết định ngữ cảnh test theo **dòng** | **ĐÓNG** — `_test_char_ranges` theo token-tree; 3 ca đột biến **ĐỎ `--strict`**, 2 đối chứng **XANH**, `cargo check` RC=0 | §2 |
| **F4b** | con test-only của inline module `#[path]` bị **báo oan** | **ĐÓNG** — `#[cfg(test)] #[path="sub"] mod m { mod child; }` + `sub/child.rs` gọi `wake` ⇒ **XANH** cả hai chế độ; `cargo check --tests` RC=0 | §3 |
| **F4c** | `#[path]` trỏ thư mục trên `mod` không thân bị **bỏ qua im lặng** | **ĐÓNG** — **ĐỎ `E_STRICT_WAKE_INCLUDE`** cả hai chế độ | §4 |

Nhưng hợp đồng **còn dòng sai** (cùng dòng L35 đã nêu; lead thêm dòng tinh chỉnh nhưng **không**
sửa dòng tổng quát) và **chưa ghi** một số báo oan có trước:

| # | Vấn đề | Mức | Bằng chứng |
|---|---|---|---|
| **C1** | §3:50 "Thân `macro_rules!` chứa `include`/`path`/`mod` (**bất kỳ dạng**) → `E_STRICT_MACRO_INCLUDE`" — sai với `mod NAME;` literal (→ `E_STRICT_MACRO_MOD`, §3:52) và `$…:tt` (→ `E_STRICT_MACRO_TT`, §3:51) | TRUNG BÌNH (hợp đồng) | §5 |
| **C2** | §5:79 "Code nằm ngoài `v2/kernel`/`v2/sim` — **không quét**, trừ thân macro `v2/lib`" — `scan_lexical` **có** quét `v2/lib` (lỗi từ vựng ⇒ ĐỎ) | NHẸ (hợp đồng) | §5 |
| **C3** | §4 chưa ghi các **báo oan có trước** (không phải hồi quy): `#[cfg(test)]` trên `const`/`static`/`macro_rules!` thân `()`/`[]`/`include!`/block statement; `#[cfg(feature = "ktest")]` trên mục inline | NHẸ (tài liệu) | §6 |

**KT-194 xong KHÔNG tự làm KT-184 DONE.** KT-184 vẫn `REVIEW`.

---

## 1. Phương pháp

- Bản sao cây thật: `rsync -a --exclude target --exclude .git /private/tmp/night2-L36-review199/ /tmp/kt200-base/`.
- Mỗi ca: phục hồi tệp đã sửa từ base (reset tăng dần), thêm đột biến, chạy
  `python3 v2/tools/check-wake-paths.py` (thường) và `--strict`; đo
  `RUSTUP_TOOLCHAIN=1.99.1 CARGO_TARGET_DIR=/tmp/kt200-target cargo check --manifest-path
  v2/kernel/Cargo.toml -p vifast-core --offline`.
- F4b đo thêm `cargo check --tests -p vifast-core` (để `#[cfg(test)]` thật sự được biên dịch) và
  `rustc --edition 2021 --crate-type lib --test` trên crate tối thiểu (kiểm cú pháp `#[path]` trên
  inline module).
- Đối chứng dương: `wake` trong `#[cfg(test)] mod tests`/`#[cfg(test)] fn` ⇒ **XANH**; sản phẩm
  xuống dòng ⇒ **ĐỎ**.
- Log: [`kt200-logs/`](kt200-logs/).

---

## 2. F2b — luật `wake` theo token-tree (ĐÓNG)

`_check_wake_text` (`v2/tools/check-wake-paths.py:2181`) nay dùng
`test_ranges = _test_char_ranges(code, relp)` (`:2198`), không còn `in_ranges(lineno, tr)`.
`_test_char_ranges` (`:2104-2178`) duyệt token-tree: mỗi `#[cfg(test)]` chỉ đánh dấu **đúng mục**
nó gắn vào (từ token mục tới `}` khớp), không lan sang mục cùng dòng.

Đột biến trên **cây thật** (`lib.rs`), `cargo check` RC=0:

| # | Dạng | thường | `--strict` | `cargo check` |
|---|---|---|---|---|
| F2b-1 | `#[cfg(test)] fn h() { … } pub fn evil(…) { m.wake(t); }` cùng dòng | XANH | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE lib.rs:122` | **RC=0** |
| F2b-2 | `#[cfg(test)] mod tests { … } pub fn evil(…) { m.wake(t); }` cùng dòng | XANH | **ĐỎ** `lib.rs:122` | **RC=0** |
| F2b-3 | hàm sản phẩm bắt đầu cùng dòng `}` test, thân xuống dòng | XANH | **ĐỎ** `lib.rs:123` | **RC=0** |
| C1 | `wake` trong `#[cfg(test)] mod tests { … }` | XANH | **XANH** | RC=0 |
| C2 | `wake` trong `#[cfg(test)] fn f()` | XANH | **XANH** | RC=0 |
| C3 | đối chứng: hàm sản phẩm **xuống dòng** | XANH | **ĐỎ** `lib.rs:123` | RC=0 |

⇒ **F2b đóng**: ba ca lọt của L35 đều **ĐỎ `--strict`**; hai đối chứng test **XANH**; sản phẩm
thật sự biên dịch `wake` (`cargo check` RC=0). Chế độ thường **XANH** cho tệp sản phẩm là **đúng
hợp đồng §4 F2** (luật `wake` chỉ chạy đầy đủ ở `--strict`; `v2 check`/`gate` chạy `--strict` —
`v2/bin/v2:321`). Log: `kt200-logs/harness.log` (CASE F2b-*).

---

## 3. F4b — inline module `#[path]` + con test-only (ĐÓNG)

`_walk_items` (`:1071-1093`) nay tính `_child_dir` từ `#[path]` của inline module (không dùng
`cur_dir/NAME`), nên `sub/child.rs` vào đồ thị module với `is_test=True`.

| Ca | thường | `--strict` | `cargo check --tests` |
|---|---|---|---|
| `#[cfg(test)] #[path = "sub"] mod m { mod child; }` + `sub/child.rs` gọi `wake` | **XANH** | **XANH** | **RC=0** |

Kiểm cú pháp độc lập: `rustc --edition 2021 --crate-type lib --test` trên crate tối thiểu với
đúng layout ⇒ **RC=0** (không phải báo oan cú pháp). Log: `kt200-logs/harness.log` (CASE F4b),
`kt200-logs/f4b-cargo-tests.log`.

---

## 4. F4c — `#[path]` trỏ thư mục trên `mod` không thân (ĐÓNG)

`resolve_include_targets` (`:1523-1534`) phân biệt inline (`tok.start in inline_path_toks` ⇒ bỏ
qua) với không-inline (⇒ **ĐỎ**).

| Ca | thường | `--strict` | Mã |
|---|---|---|---|
| `#[path = "sub"] mod m;` + `sub` là thư mục | **ĐỎ** | **ĐỎ** | `E_STRICT_WAKE_INCLUDE` "trỏ tới thư mục … nhưng `mod` không có thân" |
| `#[path = "sub"] mod m;` + `sub` **không** tồn tại | **ĐỎ** | **ĐỎ** | `E_STRICT_WAKE_INCLUDE` "không tồn tại" |

Log: `kt200-logs/harness.log` (CASE F4c).

---

## 5. Hợp đồng — đối chiếu từng dòng §2/§3/§4/§5

Đo trên cây thật (43 ca, `kt200-logs/contract-summary.log` + `kt200-logs/harness.log`). Ghi chú:
4 ca "FAIL" trong log là **kỳ vọng sai của harness** (tôi kỳ vọng chế độ thường ĐỎ cho tệp sản
phẩm; thực tế thường XANH đúng hợp đồng §4 F2) — kết quả `--strict` của cả 4 ca **đúng**.

| Dòng hợp đồng | Code @ `98694376` | Đúng? |
|---|---|---|
| §2:26 `include!` ba dấu ngoặc + `include ! ( … )` | `_walk_items` `:1040-1052`; `on_include` `:1470-1478` | **ĐÚNG** (đã thử 4 dạng, ĐỎ cả hai chế độ) |
| §2:27-28 chuỗi thường/raw `r"…"`, `r#"…"#`, `r##"…"##`; escape `\x2e`, `\u{2e}`, … | `_RE_LEX` `:502`, `_decode_string` `:525` | **ĐÚNG** (đã thử `r"…"`, `r#"…"#`, `r##"…"##`, `\x2e`, `\u{2e}`) |
| §2:29 `#[path]`, `#[ path ]`, `#[\npath]`, `#[r#path]` | `_walk_items` `:1003-1019` + `parse_attr` `:835` | **ĐÚNG** (đã thử cả 4) |
| §2:30 `cfg_attr(not(test), path)` + `cfg_attr` lồng | `attr_path_edges` `:858-911` | **ĐÚNG** (đã thử 1 tầng + 2 tầng) |
| §2:31 `mod NAME;` + `#[cfg(test)]`/`#[cfg(feature = "ktest")]` | `_mods_of` `:1261-1339`; `cfg_is_test_tokens` `:914` | **ĐÚNG** (đã thử cả hai ⇒ test-only) |
| §2:32-34 ngữ cảnh test theo item/token-tree, áp cho **cả** `_mods_of` **và** `_test_char_ranges`; `#[cfg(test)]` chỉ áp mục kế tiếp; mục trong khối test lồng vẫn test | `_mods_of` `:1310-1312`; `_test_char_ranges` `:2104-2178` | **ĐÚNG cho các dạng có thân khối** (đã thử 6 ca `mod` + 6 ca `wake`); **hở** với mục **không có thân `{}`** (`const`/`static`/`macro_rules!` thân `()`/`[]`/`include!`/block statement) — báo oan **có trước**, xem C3/§6 |
| §2:35 `macro_rules!` mọi tên (kể cả Unicode) + ba dấu thân | `_walk_items` `:1024-1039` | **ĐÚNG** (đã thử `é` thân `{}`, thân `()`, thân `[]`) |
| §2:36 chú thích chen giữa `#`/`[`/`path` | tokenizer bỏ comment `:628-656` | **ĐÚNG** (đã thử `#/*x*/[path]`, `#[ /*x*/ path ]`) |
| §2:37-38 `#[path = "sub"] mod m { … }` inline: `sub` là thư mục; con duyệt trong thư mục `#[path]` | `_walk_items` `:1071-1093`; `resolve_include_targets` `:1523-1534` | **ĐÚNG** (F4b: XANH; `cargo check --tests` RC=0) |
| §3:46 `include!`/`#[path]`/`cfg_attr(path)` không literal | `attr_path_edges` `:889-891`; `on_include` `:1470-1478` | **ĐÚNG** (đã thử `concat!`, biến) |
| §3:47 đích ra ngoài `v2/kernel`+`v2/sim` | `_inside_scan_tree` `:1403` | **ĐÚNG** (đã thử `../../…/etc/passwd`) |
| §3:48 đích không tồn tại (và không phải thư mục) | `:1535-1540` | **ĐÚNG** (đã thử `nope_xyz.rs`) |
| §3:49 `#[path]` trỏ thư mục trên `mod` không thân | `:1523-1534` | **ĐÚNG** (F4c: ĐỎ cả hai chế độ) |
| §3:50 thân `macro_rules!` chứa `include`/`path`/`mod` (**bất kỳ dạng**) ⇒ `E_STRICT_MACRO_INCLUDE` | `_check_macro_body` `:1560-1670`: `include!`/`#[path]`/`cfg_attr(path)` ⇒ INCLUDE; `mod NAME;`/`mod NAME {…}` ⇒ **MOD** (`:1637-1649`); `mod r#x;`/`mod $n;` ⇒ INCLUDE (`:1630-1655`); `$…:tt` ⇒ **TT** (`:1656-1668`) | **SAI** — "bất kỳ dạng" không đúng: `mod` literal ⇒ `E_STRICT_MACRO_MOD`, `$…:tt` ⇒ `E_STRICT_MACRO_TT` (C1) |
| §3:51 `$…:tt` ⇒ `E_STRICT_MACRO_TT` | `:1656-1668` | **ĐÚNG** (đã thử) |
| §3:52 `mod NAME;` literal ⇒ `E_STRICT_MACRO_MOD` | `:1637-1642` | **ĐÚNG** (đã thử); code **cũng** cho MOD với `mod NAME {…}` (`:1643-1649`) — hợp đồng không ghi, không sai |
| §3:53 `mod $n;` / `mod r#x;` ⇒ `E_STRICT_MACRO_INCLUDE` | `:1630-1635`, `:1650-1655` | **ĐÚNG** (đã thử cả hai) |
| §3:54 `wake(` trong sản phẩm thuộc `TEST_ONLY_WAKE_SCOPE` | `in_test_only_wake_scope` `:174` | **ĐÚNG** (F2b/F1) |
| §3:55 chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá sâu ⇒ `E_LEX_*` | `tokenize`/`build_tree`/`attr_path_edges` | **ĐÚNG** cho `v2/kernel`+`v2/sim`+`v2/lib` (đã thử chuỗi chưa đóng trên cây thật) |
| §4:63 F2 (một phần): luật `wake` chỉ chạy đầy đủ ở `--strict`; chế độ thường chỉ quét tệp bị kéo vào bằng `#[path]`/`include!` | `scan_v2` `:2484-2636` (strict) vs `:2643-2660` (thường) | **ĐÚNG** (F2b-1 thường XANH; F1 repro thường ĐỎ) |
| §4:64 macro invocation: `_walk_items` bỏ qua nhóm sau `!`; cây thật **không** có lời gọi macro chứa `mod`/`#[path]` | `:980-985`; grep độc lập `v2/kernel`+`v2/sim`+`v2/lib`: **0** hit `cfg_if!`/`paste!`/`concat_idents!`/macro chứa `mod` | **ĐÚNG** (đã kiểm độc lập; đột biến `foo! { #[path = "../../…/agent/evil.rs"] mod e; }` ⇒ XANH — đúng là **lỗ đã ghi**) |
| §4:65 `#[path]` trong `v2/lib` không được phân giải | `resolve_include_targets` chỉ lặp `files` từ `V2_SCAN = ("v2/kernel","v2/sim")` `:264` | **ĐÚNG** (đột biến `#[path]` trong `v2/lib/vifast-abi/src/lib.rs` ⇒ XANH) |
| §4:66 F3 (v2/lib): `cfg_attr` lồng trong `v2/lib` ngoài macro ⇒ XANH | `scan_structure` chỉ `v2/kernel`+`v2/sim` `:1139-1141` | **ĐÚNG** (40 tầng trong `v2/lib` ⇒ XANH cả hai chế độ) |
| §5:72-76 macro expansion / proc-macro / build.rs / cfg phức tạp / symlink | không mô phỏng; grep: **0** proc-macro dep; `include!(concat!(env!("OUT_DIR"),…))` ⇒ ĐỎ INCLUDE | **ĐÚNG** (đã thử OUT_DIR) |
| §5:77 `#[cfg]` phức tạp — chỉ hiểu `#[cfg(test)]` và `#[cfg(feature = "ktest")]` | `cfg_is_test_tokens` `:914-939` | **ĐÚNG** (`any(test,…)`/`not(test)` ⇒ sản phẩm; `all(test, feature="ktest")` ⇒ test) |
| §5:79-80 code ngoài `v2/kernel`/`v2/sim` — **không quét**, trừ thân macro `v2/lib` | `scan_lexical` `_protected_files` `:1120-1128` **gồm `v2/lib`** ⇒ lỗi từ vựng trong `v2/lib` **ĐỎ** | **SAI một phần** — `v2/lib` **có** bị quét từ vựng (C2) |

**Kết luận §5**: hợp đồng đã đúng phần lớn; còn **C1** (dòng §3:50, cùng dòng L35 đã nêu — lead
thêm dòng 51-53 nhưng **không** sửa dòng 50) và **C2** (§5:79). **C3** là thiếu sót tài liệu
(báo oan có trước, không phải hồi quy).

---

## 6. Biến thể mới chưa nằm trong selftest

Đo trên cây thật (43 ca hợp đồng) + probe trực tiếp `_test_char_ranges`/`_check_wake_text`
(`kt200-logs/probe-variants.log`).

| Nhóm | Dạng | Kết quả |
|---|---|---|
| delimiter | `include! ( … )`, `include![…]`, `include!{…}`; `macro_rules!` thân `{}`/`()`/`[]` | **ĐÚNG** (include ĐỎ; macro rỗng XANH) |
| comment/whitespace | `#/*x*/[path]`, `#[ /*x*/ path ]`, `#[ path ]`, `#[\npath]` | **ĐÚNG** (ĐỎ) |
| identifier | `mod r#evil;`, `mod 日本;` (L35 đã thử; tôi thử `mod r#x;` trong macro ⇒ INCLUDE) | **ĐÚNG** |
| attribute | `#[path = r"…"]`, `#[r#path]`, `cfg_attr` 1/2 tầng | **ĐÚNG** (ĐỎ) |
| nesting | `cfg_attr` 31/32/33/34/40; ngoặc 126/127/128/140 (L35 đã đo; tôi kiểm lại 40 tầng trong `v2/lib` ⇒ XANH đúng §4:66) | **ĐÚNG** biên |
| token forwarding | `$…:tt` ⇒ TT; `mod $n;`/`mod r#x;` ⇒ INCLUDE; `mod NAME;` ⇒ MOD | **ĐÚNG** |
| `#[cfg(feature = "ktest")]` | `mod evil_k;` (file) ⇒ **XANH** (test-only); `fn f() { wake }` (inline) ⇒ **ĐỎ** (báo oan — xem dưới) | **ĐÚNG một phần** |
| `cfg_attr` lồng | 2 tầng ⇒ ĐỎ; 40 tầng trong `v2/lib` ⇒ XANH (đúng §4:66) | **ĐÚNG** |

### 6.1 Báo oan **có trước** (không phải hồi quy — đã đối chiếu checker `0fa0fe65`)

Đo cùng một bộ ca trên checker **cũ** (`0fa0fe65`) và **mới** (`98694376`): **giống nhau** —
tất cả đều ĐỎ ở cả hai (log `kt200-logs/false-positive-preexisting.log`):

| Dạng | Cũ `0fa0fe65` | Mới `98694376` | Ghi chú |
|---|---|---|---|
| `#[cfg(test)] include!("evil_inc.rs")` (tệp đích gọi `wake`) | ĐỎ | ĐỎ | `on_include` không nhận attr ⇒ tệp đích bị coi sản phẩm |
| `#[cfg(feature = "ktest")] fn f() { wake }` | ĐỎ | ĐỎ | `strip_rust_noise` xoá chuỗi `"ktest"` ⇒ `cfg_is_test_tokens` không nhận |
| `#[cfg(test)] const W = Machine::wake;` | ĐỎ | ĐỎ | `_test_char_ranges` chỉ đánh dấu mục có thân `{}` |
| `#[cfg(test)] macro_rules! m ( … )` / `[ … ]` | ĐỎ | ĐỎ | thân `()`/`[]` không được đánh dấu |
| `#[cfg(test)] { wake }` (block statement) | ĐỎ | ĐỎ | attr trên group không được xét |
| `#[cfg(test)]` trên match arm pattern literal | ĐỎ | ĐỎ | pattern không phải ident |

**Ảnh hưởng**: **không** có trên cây thật (`v2 check` PASS; 48 chỗ `#[cfg(feature = "ktest")]`
trong cây **không** chỗ nào chứa `wake`). Đây là **báo oan tiềm ẩn**, không phải đường lọt; nên
ghi vào §4/§5 (C3) hoặc sửa sau.

---

## 7. `v2 check` + `--selftest` trên cây sạch

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

`python3 v2/tools/check-wake-paths.py --selftest` → `# selftest: 118/118 ok` RC=0.
`python3 v2/tools/check-wake-paths.py` → `errors=0` RC=0; `--strict` → `errors=0` RC=0.
Log: `kt200-logs/v2check.log`.

---

## 8. Hồi quy

- **76 ca cũ**: chạy checker **trước KT-194** (`0fa0fe65`) ⇒ `76/76 ok`; đối chiếu **tên** 76 ca
  với 118 ca hiện tại ⇒ **0 ca thiếu** (`kt200-logs/old76.txt`, `kt200-logs/new118.txt`).
- **Không ca cũ nào bị đổi kỳ vọng**: diff hàm `selftest()` giữa `0fa0fe65` và `98694376` chỉ có
  docstring + **thêm** ca (không sửa dòng ca cũ) — `kt200-logs/selftest-old.txt` vs
  `kt200-logs/selftest-new.txt`.
- **Ca KT-194/196/198/199**: nằm trong 118 ca, tất cả `ok` (bao gồm 3 ca F2b mới, F4b, F4c).
- **F1/F3** (đã đóng ở vòng 2): giữ nguyên trong selftest; tôi không thấy hồi quy (118/118).

---

## 9. Việc **chưa** review / giới hạn của review này

- **Không** chạy `gate --fast` (ràng buộc task) — chỉ `v2 check`. L1 1.000 seed **chưa** kiểm.
- **Không** chạy QEMU (ràng buộc task).
- **Không** kiểm `v2/lib` `#[path]`/`include!` ngoài thân macro (hợp đồng §4:65 đã ghi ngoài
  phạm vi; tôi xác nhận bằng đột biến ⇒ XANH).
- **Không** kiểm proc-macro thật (không có trong cây; grep 0 dep).
- **Không** kiểm lời gọi macro chứa `mod`/`#[path]` trên cây thật (cây không có; đột biến
  `foo! { … }` ⇒ XANH — đúng lỗ đã ghi §4:64).
- **Không** sửa mã sản phẩm, không sửa checker, không nới ngưỡng.
- **C3** (báo oan có trước) chỉ đo trên fixture/probe, không phải trên cây thật (cây thật không
  có dạng đó).

---

## 10. Ca hồi quy cần lưu (đề nghị — reviewer **không** sửa checker)

1. **F2b-mod**: `#[cfg(test)] mod tests { } pub fn evil(…) { m.wake(t); }` (cùng dòng) ⇒
   `--strict` **ĐỎ**. (Selftest hiện có ca `fn` cùng dòng + ca `wake` trong `mod tests`; **thiếu**
   ca `mod` cùng dòng.)
2. **F2b-control**: `#[cfg(test)] fn h() { … }` rồi `pub fn evil(…)` **xuống dòng** ⇒ `--strict`
   **ĐỎ** (đối chứng dương cho F2b-1).
3. **F4b**: giữ ca KT-199/F4b (đã có) + thêm `cargo check --tests` RC=0 vào bằng chứng.
4. **F4c**: giữ ca KT-199/F4c (đã có).
5. **C1**: sửa §3:50 thành "… trừ `mod` literal (⇒ `E_STRICT_MACRO_MOD`) và `$…:tt` (⇒
   `E_STRICT_MACRO_TT`)".
6. **C2**: sửa §5:79 thành "… không quét **luật cấu trúc**, trừ thân macro `v2/lib`; lỗi **từ
   vựng** trong `v2/lib` vẫn bị báo".
7. **C3**: thêm vào §4 các báo oan có trước (mục 6.1) hoặc sửa `_test_char_ranges` để nhận
   `#[cfg(test)]` trên mục không thân `{}` và `#[cfg(feature = "ktest")]` (cần `strip_rust_noise`
   giữ chuỗi cho attr, hoặc dùng token-tree trên văn bản gốc).

---

## 11. Kết luận

**KT-194: ĐẠT CÓ ĐIỀU KIỆN** trên HEAD `98694376`.

- **Đạt**: **F2b** (3 ca lọt ĐỎ `--strict`, 2 đối chứng XANH, `cargo check` RC=0); **F4b**
  (XANH, `cargo check --tests` RC=0, `rustc --test` RC=0); **F4c** (ĐỎ `E_STRICT_WAKE_INCLUDE`
  cả hai chế độ); hợp đồng §2/§3/§4/§5 đúng **phần lớn** (43 ca đo trên cây thật); biến thể
  delimiter/comment/identifier/attribute/nesting/token-forwarding/`cfg_attr` lồng đúng;
  118/118 selftest; `v2 check` PASS; 76 ca cũ + KT-194/196/198/199 xanh.
- **Điều kiện**: **C1** — sửa dòng §3:50 (cùng dòng L35 đã nêu, vẫn còn sai chữ "bất kỳ dạng");
  **C2** — sửa §5:79 (`v2/lib` có bị quét từ vựng); **C3** — ghi các báo oan có trước vào §4/§5
  (hoặc sửa). Không có điều kiện nào đụng mã sản phẩm hay nới ngưỡng.
- **KT-194 xong không tự làm KT-184 DONE.** KT-184 vẫn `REVIEW`; C1-C3 phải xử lý trước khi xét
  đóng KT-194/KT-184.

**Ký**: reviewer độc lập L36, 2026-11-19.
