# KT-242 — review **độc lập** KT-184 và KT-194 (luật `wake`, hai lớp)

**Reviewer**: sub-agent `wake-review` (khác tác giả KT-184/KT-194/KT-241).
**Ngày**: 2026-11-20 (JST). **HEAD review**: `c6127fa0` (`main`).
**Worktree**: `/tmp/kt242-s2`, nhánh `task/wake-review` (tạo từ `main`).
**Toolchain**: `RUSTUP_TOOLCHAIN=1.100.0` → `rustc 1.100.0 (0ef1bc386 2026-11-09)`;
`python3 3.9.6`.
**Ràng buộc đã giữ**: **không** sửa mã sản phẩm, `spec/`, docs cấm sửa; **không** nới ngưỡng;
**không** tắt ca; **không** sửa checker để repro xanh. Mọi repro chèn vào cây đều **khôi phục**
bằng `trap` (`git checkout -- . && git clean -fdq v2/kernel v2/sim v2/lib`); `git status` sau
mỗi lần đo chỉ còn thư mục repro này.

---

## 0. Xác nhận số ca + baseline (tự chạy, không tin báo cáo)

| Phép đo | Lệnh | Kết quả |
|---|---|---|
| Lớp 1 selftest | `python3 v2/tools/check-wake-paths.py --selftest` | **384/384 ok** (38,3 s) |
| Lớp 2 selftest | `python3 v2/tools/check-wake-depinfo.py --selftest` | **62/62 ok** (1,6 s) |
| Lớp 1 thường trên cây | `python3 v2/tools/check-wake-paths.py` | `errors=0 warnings=0`, RC=0 |
| Lớp 1 `--strict` trên cây | `python3 v2/tools/check-wake-paths.py --strict` | `errors=0 warnings=0`, RC=0 (67 s) |
| Lớp 2 trên cây | `python3 v2/tools/check-wake-depinfo.py` | `errors=0 warnings=0`, RC=0 (17,6 s) |

Số ca **khớp** báo cáo của tác giả (KT-241: 384 + 62). Baseline **xanh** — nên mọi ca dưới đây
là **đột biến** so với baseline, không phải nhiễu.

---

## 1. Cách tấn công: **theo lớp**, không theo danh sách

Theo D2-178(iv)/D2-179(4), vòng này **không** chọn mẫu từ 15 đường lọt đã biết. Tôi lập **ma
trận lớp** rồi tấn công từng lớp, mỗi lớp nhiều biến thể:

| Lớp | Bề mặt đã tấn công |
|---|---|
| **L1 lexer** | chuỗi thô/byte/C-string, escape `\x`/`\u{}`/`\0`, comment lồng, `\r\n`, BOM, literal ký tự vs lifetime, `r#ident` |
| **L2 token-tree** | delimiter lệch, độ sâu > `MAX_TREE_DEPTH`, `cfg_attr` lồng > `MAX_ATTR_DEPTH`, nhóm rỗng |
| **L3 walker** | `mod` trong `{…}`/`(…)`/`[…]`/thân hàm/const block/inline module lồng, `pub`/`pub(crate)`/`pub(in …)`, raw ident, `#[path]` trên inline module |
| **L4 `include!`/`#[path]`** | literal/không literal, byte string, C string, raw byte string, trailing comma, NUL, escape, thư mục, không tồn tại, symlink, `.txt`/`.RS`, đích ngoài cây, đích `v2/lib` ⇄ `v2/kernel` |
| **L5 `cfg`/`cfg_attr`** | `cfg(test)`, `cfg(all(test))`, `cfg(any(test))`, `cfg(not(test))`, `cfg(feature="ktest")`, `cfg_attr` lồng 2–3 tầng, trailing comma, `r#path` |
| **L6 macro** | `macro_rules!` lồng, `:tt`/`:item`/`:stmt`/`:block`/`:meta`/`:expr`, macro che tên an toàn, `include!` trong đối số lời gọi, `stringify!`/`concat!`, macro ở tệp không `.rs` |
| **L7 fn-pointer/generic/trait-object** | `Machine::wake` làm giá trị, `self.wake`, UFCS, turbofish, `impl Fn`, `dyn Fn`, `transmute` |
| **L8 tệp ngoài `src`** | `[[bin]] path` ngoài `src/`, `[lib] path`, `build.rs`, `init/*.rs`, `vifast-plat-user/lib.rs` |
| **L9 symlink** | symlink tệp/thư mục trỏ trong/ngoài cây |
| **L10 đường dẫn lạ** | `..`, đường dẫn tuyệt đối, `""`, NUL, dấu cách, hoa/thường |
| **L11 `OUT_DIR`/tệp sinh** | `include!(concat!(env!("OUT_DIR"), …))`, `build.rs` sinh mã, crate ngoài ba gốc |
| **L12 phân vùng test** | `#[cfg(test)]` trên item cùng dòng/khác dòng, nhánh `match`, field struct, biến thể enum, `impl`/`trait`/`fn`/`mod`/`let`/`use` |
| **L13 miễn local-binding** | `let wake`, `for wake in`, `(wake: …)`, macro an toàn, `wake` làm giá trị |
| **L14 ranh giới API (KT-241)** | `write_state`/`into_state`, 6 cửa, `From`/`TryFrom`, inherent impl, `pub fn` chuyển kiểu, hàm lồng/closure/impl lồng |

---

## 2. Bảng đường lọt **MỚI** (không có trong `BYPASS-REGISTRY.md`)

Ký hiệu: **X** = XANH (lọt), **Đ** = ĐỎ (bắt được). `cargo` = `cargo check --manifest-path
v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf` (hoặc host cho ca `v2/sim`).

| # | Đường lọt | Lớp 1 thường | Lớp 1 `--strict` | Lớp 2 | `cargo` | Repro | Gốc |
|---|---|---|---|---|---|---|---|
| **N1** | **"Cửa thứ bảy"**: `impl Machine { pub fn kt242_door(&mut self, t) { self.write_state(t, TcbState::Ready) } }` **lồng trong thân** cửa `wake_step`; mã sản phẩm gọi `m.kt242_door(t)` ⇒ TCB về `Ready` **không qua điều kiện nguồn nào** | **X** | **X** | **X** | RC=0 | `kt242-wake-repro/r3a.sh` + `logs/r3a/` | `_check_state_write` quy `write_state` cho hàm qua `enclosing(fns, line)`; `find_functions` **nhảy qua** hàm lồng (`i = end + 1`) ⇒ mọi `write_state` trong thân `fn`/`impl` lồng bị quy cho **cửa ngoài** ⇒ thuộc `STATE_WRITE_CALLERS` |
| **N2** | Như N1 nhưng `fn kt242_door(...)` lồng trong `wake_step` | **X** | **X** | **X** | (fixture) | `fixture-probes.py` ca N2 | cùng gốc N1 |
| **N3** | **`[[bin]] path` ngoài `src/`**: `v2/sim/vifast-sim/bin/kt242_bin.rs` = `m.wake(0)` + `[[bin]] path = "bin/kt242_bin.rs"` | **X** | **X** | **Đ** `E_DEPINFO_WAKE` | RC=0 | `kt242-wake-repro/binpath.sh` + `logs/binpath/` | `in_test_only_wake_scope` = `^v2/(kernel\|sim)/<crate>/(src/.*\.rs\|build\.rs)$` — bin/lib khai báo **ngoài `src/`** không khớp mẫu; `crate_roots` **có** thấy nó (đọc `[[bin]] path`) nhưng luật token `wake` vẫn lọc theo mẫu |
| **N4** | **Tệp `.RS` (hoa)**: `src/kt242_rs.RS` + `mod kt242_rs;` — macOS (FS không phân biệt hoa/thường) nạp được | **X** | **X** | **Đ** `E_DEPINFO_WAKE` | RC=0 | `kt242-wake-repro/rs-case.sh` + `logs/rs-case/` | `iter_rs`/`_protected_files`/`scan_structure`/`scan_macro_bodies`/`_shadowed_safe_macros` đều `rglob("*.rs")` — so khớp **phân biệt** hoa/thường; rustc trên macOS thì **không** |
| **N5** | **Luật 1–3 (một kênh wake)**: `post_mail(…, MailboxMsg::Wake)`, consumer `MailboxMsg::Wake`, arch `IpiKind::Wake` đặt **cùng dòng** với một mục `#[cfg(test)]` | **X** | **X** | (lớp 2 không có luật 1–3) | — | `fixture-probes.py` ca N5a/N5b/N5c + **đối chứng âm** N5a'/N5b' | `test_line_ranges` (dùng cho luật 1–3, R2, R3) **lan cả dòng**: `#[cfg(test)]\nfn helper() {} fn evil(…) { … }` ⇒ vùng test `(1,2)` phủ **cả** dòng 2. Hợp đồng §2 nói "`#[cfg(test)]` chỉ áp cho **mục kế tiếp**, không lan sang mục cùng dòng" — **sai** cho 5/6 call site |
| **N6** | **`include!("\0")`** ⇒ `ValueError: embedded null byte`, checker **crash** (traceback) thay vì `E_LEX_*` | crash (RC≠0) | crash | — | — | `fixture-probes.py` ca N6 | `resolve_include_targets` gọi `Path.resolve()` trên đường dẫn chứa NUL; `ValueError` **không** được bọc thành `LexError`/lỗi cổng ⇒ vi phạm D2-166 §3C ("lỗi **rõ** + exit khác 0") |

**Đối chứng dương** (để chứng minh repro có răng, không phải "luôn xanh"):

| Ca | Kỳ vọng | Kết quả |
|---|---|---|
| N5a' bỏ `#[cfg(test)]` khỏi N5a | ĐỎ `E_ARCH_DOORBELL` | **Đ** ✔ |
| N5b' bỏ `#[cfg(test)]` khỏi N5b | ĐỎ `E_CHANNEL_PRODUCER` | **Đ** ✔ |
| R1 cùng dòng `#[cfg(test)]` (`m.wake(t)` sau `fn helper() {}`) | ĐỎ `E_STRICT_TEST_ONLY_WAKE` | **Đ** ✔ (R1 **không** bị N5 lừa) |
| R1 trong `#[cfg(test)] mod t { … }` | XANH | **X** ✔ |
| N3b `cfg_attr` lồng trong `v2/lib` trỏ **ra ngoài** `v2/lib` | ĐỎ `E_STRICT_WAKE_INCLUDE` | **Đ** ✔ |
| L2 crate **mới** dưới `v2/kernel` chứa `m.wake(t)` | ĐỎ `E_STRICT_TEST_ONLY_WAKE` | **Đ** ✔ |

---

## 3. Ba nợ đã ghi: L2 / N3 / DEBT-KT227-1

### 3.1 **L2** — ba mẫu phạm vi vênh nhau ⇒ **CHƯA ĐÓNG** (không có đường lọt sống)

Đo được:

- `in_test_only_wake_scope` = `^v2/(?:kernel|sim)/[^/]+/(?:src/.*\.rs|build\.rs)$` — **mẫu**, nên
  crate **mới** dưới `v2/kernel` **được** quét (ca "L2 crate mới" ⇒ **ĐỎ**). Không có lỗ hổng
  "crate mới" ở lớp 1.
- `RE_CRATE_SRC` (lớp 2) = `^v2/(?:lib|kernel|sim|tools)/[^/]+/src/` — **rộng hơn** lớp 1 (gồm
  `v2/lib`, `v2/tools`).
- `MACRO_SCAN_ROOTS` = `("v2/lib", "v2/kernel", "v2/sim")` — **hẹp hơn** `RE_CRATE_SRC` (không có
  `v2/tools`).

⇒ Ba mẫu **vẫn** vênh nhau; đây là **mô tả không thống nhất**, không phải lỗ hổng đã khai thác
được: `v2/tools` không phải mã sản phẩm, và lớp 2 **nâng** crate `v2/tools` phụ thuộc crate bảo
vệ lên bảo vệ (`_classify_scope`). **Kết luận: nợ còn, mức thấp** — cần **một** định nghĩa phạm
vi dùng chung (sinh từ `cargo metadata`) thay ba mẫu.

### 3.2 **N3** — `cfg_attr` lồng trong `v2/lib` ⇒ **CHƯA ĐÓNG, và câu chữ hiện tại SAI một phần**

Đo được (ca N3/N3b):

- `#[cfg_attr(any(), cfg_attr(any(), path = "evil.rs"))] mod m;` trong `v2/lib/vifast-abi/src/lib.rs`
  **được phân giải** — `resolve_include_targets` seed **mọi** tệp `v2/lib/**/*.rs` và
  `_walk_all` → `on_path_attr` → `attr_path_edges` đệ quy `cfg_attr` **hai tầng**.
- Đích **trong** `v2/lib` ⇒ XANH (đúng: tệp đích vào `extra`, nhưng xem 3.3).
- Đích **ra ngoài** `v2/lib` ⇒ **ĐỎ** `E_STRICT_WAKE_INCLUDE` (cả hai chế độ).

⇒ Khẳng định của hợp đồng §4 ("`cfg_attr` lồng trong `v2/lib` **ngoài** thân macro ⇒ **XANH**")
**chỉ đúng khi đích ở trong `v2/lib`**; khi đích ra ngoài thì **ĐỎ**. Nợ N3 vì vậy **không** còn
là "chưa được chứng minh" theo nghĩa cũ — nhưng **vẫn chưa đóng** vì (i) câu chữ hợp đồng §4
chưa khớp hành vi đo được, (ii) phần thật sự còn hở là **3.3**.

### 3.3 **DEBT-KT227-1** — `v2/lib` ngoài luật token lớp 1 ⇒ **CHƯA ĐÓNG** (đo được, latent)

Ca "DEBT-KT227-1": `v2/lib/vifast-abi/src/lib.rs` = `pub fn evil(m: &mut Machine, t: u32) { let _
= m.wake(t); }` ⇒ **XANH cả hai chế độ**. Đúng như nợ đã ghi: `in_test_only_wake_scope` **không**
phủ `v2/lib`, nên luật token `wake` **không** chạy ở đó.

**Mức ảnh hưởng hiện tại: latent, không khai thác được** — đo được: **không** crate nào trong
`v2/lib` khai `vifast-core` trong `[dependencies]` (`grep -rn "vifast-core" v2/lib/*/Cargo.toml`
⇒ 0 dòng), nên `Machine` không tồn tại trong `v2/lib` và `m.wake(t)` **không biên dịch**. Nợ
thành **sống** ngay khi một crate `v2/lib` thêm phụ thuộc `vifast-core` — và lúc đó **cả hai
lớp** đều có thể mù (lớp 2 chỉ áp R1 cho tệp nạp; nếu tệp `v2/lib` **không** được nạp trong đơn
vị build nào thì không có trong `.d`).

---

## 4. Những gì **đã** chịu được tấn công (bằng chứng dương, không phải "không tìm thấy")

Ghi rõ để không thổi phồng và để vòng sau không làm lại:

| Bề mặt | Kết quả |
|---|---|
| `include!`/`#[path]` **không literal** (byte string, C string, raw byte string, `concat!`, `env!`, biến) | **ĐỎ** cả hai chế độ (`E_STRICT_WAKE_INCLUDE`) |
| `#[path]` **byte string** | **ĐỎ** |
| `#[path]` trong khối `( … )` / trong inline module của tệp **non-mod-rs** | **ĐỎ** (KT-211/KT-213 giữ) |
| `include!`/`#[path]` tới `.txt` / `.RS` (đích **trong** cây) chứa `wake` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` (KT-225 giữ) |
| `include!` trong `#[cfg(test)]` trỏ ra ngoài cây | **ĐỎ** (fail-closed, báo oan đã biết) |
| `mod` trong `( … )` | không phải Rust hợp lệ ⇒ không tính |
| `#[path]` trỏ thư mục trên `mod` **có** thân | **ĐỎ** nếu thư mục không tồn tại; bỏ qua nếu tồn tại (đúng) |
| **H4 fixpoint** (chuỗi `include!` 2–3 lớp, test-only → sản phẩm) | giữ: worklist đơn điệu `True → False`, `done` theo `(tệp, mod_dir)` |
| **R1 (luật token `wake`)** với `#[cfg(test)]` **cùng dòng** | **ĐỎ** — `_test_char_ranges` theo **item/token-tree**, không bị N5 lừa |
| **R1** với `m.wake` (method-reference), `Machine::wake`, `self.wake`, UFCS, turbofish | `self.wake` **không** phải Rust hợp lệ (`E0615`); các dạng còn lại **ĐỎ** (`::`/`(`) |
| **R1** với `let wake = …` + `matches!(m, wake, t)` (macro an toàn thật) | **XANH** đúng (đối chứng dương KT-230 giữ) |
| **R1** với `macro_rules! matches` lồng (che tên an toàn) | **ĐỎ** (KT-230 giữ) |
| **`macro_rules!`** chứa `include!`/`#[path]`/`mod`/`:tt`/`:item`/`:stmt`/`:block`/`:meta` | **ĐỎ** (KT-181/KT-202 giữ) |
| **`include!` trong đối số lời gọi macro** | **ĐỎ** `E_STRICT_MACRO_INCLUDE` |
| **`stringify!`/`concat!`** bọc `include!` | **XANH** đúng (không mở rộng thành mã) |
| **`cfg(test)`/`all(test)`/`any(test)`/`not(test)`/`feature="ktest"`** | chỉ `test`/`all(test)` là test; còn lại **sản phẩm** (fail-closed, KT-227 P0-4 giữ) |
| **`From`/`TryFrom`/`Into` `TcbState` ⇄ `NonReadyTarget`**, inherent impl, `pub fn` chuyển kiểu | **ĐỎ** `E_STRICT_NONREADY_CONV` (KT-241 giữ) |
| **Mỏ neo kiểu** (`NonReadyTarget` không `Ready`, chữ ký `NonReadyTarget`, `write_state` private) | **ĐỎ** khi sai (KT-241 giữ) |
| **`transition_tcb` với `TcbState::*`/`…::Ready`** (kể cả UFCS `args[2]`) | **ĐỎ** `E_STRICT_TRANSITION` |
| **`write_state` ngoài `state_write.rs`** | **ĐỎ** `E_STRICT_STATE_WRITE` |
| **`write_state` trong hàm **không** thuộc 6 cửa** (hàm mức module) | **ĐỎ** `E_STRICT_STATE_WRITE` |
| **`write_state` dùng làm giá trị** | **ĐỎ** |
| **`init/*.rs` + `vifast-plat-user/lib.rs`** (biên dịch bằng `rustc` trực tiếp) | **không** gọi được `wake`: `build.rs` **không** truyền `--extern vifast_core` cho `build_user_elf`/`build_user_lib` (đã đọc `v2/kernel/vifast-kbin-riscv64/build.rs:116-200,248-330`) ⇒ khẳng định của hợp đồng §1 **đúng** |
| **proc-macro** | cây **không** có crate `proc-macro = true` (grep 0) ⇒ ngoài phạm vi như hợp đồng §5 |

---

## 5. Verdict

### KT-184 (D2-160 + D2-161: H4 fixpoint + luật cứng `include!`/`#[path]` mọi chế độ) — **CHƯA ĐÓNG**

**Lý do (theo D2-190 §7 "không còn đường lọt đã biết nằm trong hợp đồng bảo vệ"):**

1. **N1/N2 là đường lọt SỐNG, nằm trong hợp đồng bảo vệ.** Hợp đồng §2b.2 ghi "`write_state`
   **chỉ** được gọi bởi các hàm ở `STATE_WRITE_CALLERS`". Đo được: một `impl`/`fn` **lồng trong
   thân** `wake_step` gọi thẳng `write_state(tcb, TcbState::Ready)`, và mã sản phẩm gọi method
   đó ⇒ TCB về `Ready` **không qua điều kiện nguồn nào**; `cargo check` **RC=0**; lớp 1 thường,
   lớp 1 `--strict`, lớp 2 **đều XANH**. Đây đúng là "cửa thứ bảy" mà D2-194 §2 **cấm**
   ("**Cấm** wrapper tổng quát nhận `TcbState` rồi gọi primitive private").
2. **N3/N4 là lỗ hổng phạm vi lớp 1** (bin/lib ngoài `src/`; tệp `.RS` hoa). Lớp 2 **bắt** cả
   hai ⇒ luật **không** lọt hoàn toàn, nhưng hợp đồng §1 nói phạm vi là **mẫu** "mọi crate dưới
   `v2/kernel/<crate>/`" — bin ngoài `src/` **không** khớp mẫu, tức hợp đồng **sai** so với
   hành vi. D2-190 §1 đòi "hợp đồng bảo vệ rõ: crate, target, feature, nguồn sinh" — chưa đạt.
3. **N5 vi phạm câu chữ hợp đồng §2** ("không lan sang mục cùng dòng") ở **cả bốn** call site
   **sống** của `test_line_ranges`; đo được **ba** luật (1, 2, 3) bị lách. Đây là luật "một kênh wake" —
   cùng luật gốc `wake`.
4. **N6**: đầu vào không hiểu gây **crash** thay vì lỗi rõ — vi phạm D2-166 §3C.

**Phần D2-161 (luật cứng `include!`/`#[path]`) tự nó chịu được tấn công**: mọi biến thể tôi thử
(byte/C string, raw byte string, `concat!`, `env!`, `..`, thư mục, không tồn tại, `( … )`, inline
module của tệp non-mod-rs, `.txt`, `.RS`, `v2/lib` ⇄ `v2/kernel`) đều **ĐỎ ở chế độ thường** —
điều kiện (ii) của D2-161 **đạt**. **H4 fixpoint** (D2-160) **đạt**. Nhưng KT-184 là **luật gốc
`wake`** (D2-167 §1: "nghiệm thu luật gốc riêng"), và luật gốc **còn** N1/N2/N5 ⇒ **CHƯA ĐÓNG**.

### KT-194 (D2-166/D2-167: lexer + token-tree + bộ đọc cấu trúc thay regex) — **CHƯA ĐÓNG**

**Lý do:**

1. **D2-167 §2 chưa được thi hành đủ.** Yêu cầu: "duyệt chuyển xác định phạm vi test sang
   **item/token-tree + vị trí nguồn chính xác**, **không** quyết định theo dòng". Đo được:
   `_test_char_ranges` (token-tree) **chỉ** phục vụ **R1**; **bốn** call site **sống** còn lại vẫn
   dùng `test_line_ranges` (**theo dòng**): `scan_v2` (phục vụ luật 1–3, R2, R3 —
   `check-wake-paths.py:3643`) và lớp 2 R2/R3 (`check-wake-depinfo.py:1234,1279`). Đó chính là
   gốc của **N5**. Thêm một chỗ **chết**: `_mods_of` tính `tr = test_line_ranges(text)`
   (`check-wake-paths.py:1596`) nhưng **không dùng** — cùng lớp lỗi "tham số `tr` chết" mà
   KT-241 đã bỏ ở `_check_wake_text`.
2. **Thành phần quyết định theo dòng thứ hai**: `find_functions` (thụt lề) + `enclosing` quyết
   định **R3** ở cả hai lớp ⇒ gốc của **N1/N2**. D2-189 §1 đòi "**bắt buộc** lập danh sách thành
   phần dùng chung, đặc biệt: phân vùng test/sản phẩm; nhận dạng transition; phân tích macro;
   lọc và chuẩn hoá đường dẫn" — `find_functions`/`enclosing` **chưa** có trong danh sách đó, và
   nó là thành phần **dùng chung hai lớp** quyết định một luật.
3. **N6**: lexer/`resolve` **không** biến đầu vào không hiểu thành lỗi rõ — D2-166 §3C nói
   "**không âm thầm bỏ qua** … ⇒ lỗi rõ + exit khác 0". Crash có exit khác 0 nhưng **không** rõ
   (traceback), và không có mã `E_LEX_*`.

**Phần lexer/token-tree tự nó chịu được tấn công**: không tìm được đường lọt nào ở tầng lexer
(chuỗi thô/byte/C-string, escape, comment lồng, `r#ident`, literal ký tự vs lifetime) hay tầng
token-tree (delimiter, độ sâu). `_walk_all` (quét token toàn cục) **giữ** — mọi vị trí
`include!`/`#[path]` tôi thử đều bị bắt. Nhưng KT-194 là **"thay regex bằng lexer + token-tree +
bộ đọc cấu trúc"**, và **hai** thành phần theo-dòng vẫn quyết định **ba** luật ⇒ **CHƯA ĐÓNG**.

---

## 6. Đề xuất vá **tối thiểu** cho Agent 1 (không tự vá)

Thứ tự theo mức chặn nghiệm thu. Mỗi mục nêu **một** thay đổi nhỏ nhất đóng được đường lọt,
kèm ca selftest **có đối chứng dương**.

### P0-1 — N1/N2: `write_state` phải bị quy theo **hàm gần nhất theo token-tree**, không theo thụt lề

- **Vá**: trong `_check_state_write`, thay `enclosing(fns, n.line)` bằng một hàm mới
  `_enclosing_fn_tree(tree, tok)` đọc **token-tree**: duyệt cây, giữ ngăn xếp `fn` đang mở
  (`fn` + nhóm `(…)` + nhóm `{…}`), trả **tên hàm trong cùng** (không phải hàm ngoài). Nếu
  `write_state` nằm trong `fn`/`impl` **lồng** ⇒ tên trả về là hàm lồng ⇒ **không** thuộc
  `STATE_WRITE_CALLERS` ⇒ **ĐỎ**.
- **Tối thiểu thay thế** (nếu không muốn viết walker mới): **cấm** `fn`/`impl`/`trait`/`mod`
  **lồng trong thân** một hàm ở `state_write.rs` ⇒ `E_STRICT_STATE_WRITE` (fail-closed). Một
  dòng kiểm trong `_check_state_write`: nếu `_walk_nodes` thấy `fn`/`impl` mà `enclosing` trả
  về hàm **khác** với hàm chứa nó theo token-tree ⇒ ĐỎ.
- **Ca selftest**: (a) `impl` lồng trong `wake_step` gọi `write_state` ⇒ **ĐỎ**; (b) `fn` lồng
  ⇒ **ĐỎ**; (c) **đối chứng dương**: 6 cửa hiện có vẫn **XANH**; (d) closure trong cửa gọi
  `write_state` ⇒ **XANH** (closure không phải cửa mới — hoặc **ĐỎ** nếu muốn chặt hơn; phải
  ghi vào hợp đồng).

### P0-2 — N5: bỏ `test_line_ranges` khỏi **mọi** luật quyết định

- **Vá**: thay `test_line_ranges(text)` bằng `_test_char_ranges(strip_rust_noise(text), relp)` ở
  **sáu** call site còn lại (luật 1–3, R2, R3 — cả hai lớp), rồi đổi `in_ranges(lineno, tr)`
  thành `in_ranges(char_offset, tr)`. Cần một helper `_line_to_offset(text, line)` hoặc chuyển
  các luật 1–3 sang quét **token** (đúng hướng KT-194).
- **Tối thiểu**: nếu chưa chuyển được luật 1–3, **ít nhất** sửa `test_line_ranges` để **không**
  lan sang mục cùng dòng: sau khi tìm được `end` (dòng đóng), **cắt** vùng tại **vị trí `}` đóng
  thật** thay vì cả dòng — hoặc trả `(start_line, end_line)` **và** một cột kết thúc, rồi so
  `(line, col)`.
- **Ca selftest**: (a) `#[cfg(test)]\nfn helper() {} fn evil(…) { post_mail(…, MailboxMsg::Wake) }`
  ⇒ **ĐỎ** `E_CHANNEL_PRODUCER`; (b) cùng dạng cho consumer và arch; (c) **đối chứng dương**:
  `#[cfg(test)] mod t { … }` ⇒ **XANH**; (d) `#[cfg(test)] fn helper() {}` rồi `fn evil` **dòng
  sau** ⇒ **ĐỎ** (giữ hành vi cũ).

### P0-3 — N3/N4: phạm vi lớp 1 phải **sinh từ `cargo metadata`**, không từ mẫu đường dẫn

- **Vá**: dùng lại `Scope` của `check-wake-depinfo.py` (hoặc `crate_roots` đã có) để lấy **danh
  sách tệp crate root** (`[lib] path`, `[[bin]] path`, `src/lib.rs`, `src/main.rs`, `build.rs`)
  và **mở rộng** `in_test_only_wake_scope` thành: "tệp thuộc crate bảo vệ" (mọi tệp dưới
  `<crate>/` mà `crate_roots`/đồ thị module nói là **sản phẩm**), thay vì khớp `src/**`.
- **Tối thiểu**: thêm **một** điều kiện — `relp` khớp `^v2/(kernel|sim)/<crate>/` **và** tệp nằm
  trong `crate_roots(root)` với `is_test=False` ⇒ vào phạm vi. Đồng thời đổi `rglob("*.rs")`
  thành `rglob("*")` + lọc `p.suffix.lower() == ".rs"` (hoặc `p.is_file()` + đuôi không phân
  biệt hoa/thường) ở **năm** chỗ.
- **Ca selftest**: (a) `[[bin]] path = "bin/x.rs"` chứa `wake` ⇒ **ĐỎ** lớp 1; (b) `src/x.RS`
  + `mod x;` ⇒ **ĐỎ** lớp 1; (c) **đối chứng dương**: `tests/x.rs` cấp crate vẫn **XANH**.

### P1-1 — N6: bọc `ValueError`/`OSError` của `Path.resolve()`

- **Vá**: trong `resolve_include_targets`, bọc `(base / raw).resolve()` trong `try/except
  (ValueError, OSError)` ⇒ `errors.append(f"  {relp}:{tok.line}: đường dẫn không phân giải được
  ({raw!r}) — fail-closed")`. Cùng chỗ cho `_mods_of`/`_walk_items`.
- **Ca selftest**: `include!("\0")` ⇒ **ĐỎ** `E_STRICT_WAKE_INCLUDE` (không crash); đối chứng
  dương: `include!("ok.rs")` trong cây ⇒ **XANH**.

### P1-2 — Hợp đồng: sửa **câu chữ** cho khớp hành vi đo được

- §2: bỏ khẳng định "`#[cfg(test)]` … không lan sang mục cùng dòng" **hoặc** ghi rõ "đúng cho
  R1 (`_test_char_ranges`); luật 1–3/R2/R3 còn dùng `test_line_ranges` (theo dòng) — **sai
  lệch đã biết**, xem §4".
- §4 (N3): sửa "`cfg_attr` lồng trong `v2/lib` ⇒ XANH" thành "**đích trong `v2/lib`** ⇒ XANH;
  **đích ra ngoài** ⇒ ĐỎ `E_STRICT_WAKE_INCLUDE` (đã đo)".
- §1: ghi rõ phạm vi lớp 1 **không** phủ bin/lib khai báo ngoài `src/` và tệp đuôi hoa — **đã
  đo** (N3/N4), lớp 2 bắt.
- §2b.2: ghi rõ "hàm **lồng** trong thân cửa **không** được coi là cửa" (sau khi vá P0-1) hoặc
  ghi nợ nếu chưa vá.

### P2 — Nợ còn lại (ghi, **không** tính là đóng)

| Mã | Nội dung | Trạng thái sau review |
|---|---|---|
| **L2** | ba mẫu phạm vi (`in_test_only_wake_scope` / `RE_CRATE_SRC` / `MACRO_SCAN_ROOTS`) vênh nhau | **còn** — không có đường lọt sống; cần **một** định nghĩa phạm vi dùng chung |
| **N3** | `cfg_attr` lồng trong `v2/lib` | **còn** — nhưng câu chữ hiện tại **sai một phần** (đích ra ngoài **ĐỎ**); phần thật sự hở là DEBT-KT227-1 |
| **DEBT-KT227-1** | `v2/lib` ngoài luật token lớp 1 | **còn** — đo được XANH; **latent** (không crate `v2/lib` nào phụ thuộc `vifast-core`) |
| **WB-16** | `Ready` qua helper/const/static/closure | **ĐÓNG ở biên dịch** (API `NonReadyTarget`) — nhưng **N1/N2 mở lại đường tương đương** ở tầng checker: "cửa thứ bảy" đưa `Ready` vào mà không qua điều kiện. **Không** được ghi WB-16 là đóng cho tới khi P0-1 xong |
| **WB-17** | fn-pointer/generic/trait-object | **ĐÓNG ở biên dịch** (kiểu loại trừ `Ready`); UFCS đã vá (KT-239). Không tìm được biến thể sống mới |
| **WB-18** | crate ngoài ba gốc + `$OUT_DIR` | **ĐÓNG** (KT-238) — không tìm được biến thể sống |
| **`init/*.rs` + `vifast-plat-user/lib.rs`** | ngoài **cả hai** lớp | **còn** — hiện **không** khai thác được (`build.rs` không truyền `--extern vifast_core`); nếu ai đó thêm extern đó vào `build.rs` thì **cả hai lớp mù**. Đề nghị: thêm luật "`build.rs` không được truyền `--extern vifast_core` cho `build_user_elf`/`build_user_lib`" **hoặc** đưa `init/*.rs` + `vifast-plat-user/lib.rs` vào phạm vi lớp 1 |
| **`v2/tools`** | ngoài `MACRO_SCAN_ROOTS` | **còn** — không phải mã sản phẩm; ghi như hợp đồng §4 |
| **`cfg(test)` do `--cfg test`** | nếu build sản phẩm đặt `--cfg test` thì `#[cfg(test)]` vào ảnh | **giả định** của hợp đồng, **chưa** kiểm máy. Đề nghị: một dòng trong `build.rs`/`v2 check` khẳng định **không** có `--cfg test` trong build sản phẩm |

---

## 7. Tái lập

```bash
cd /tmp/kt242-s2
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

# (a) số ca + baseline
python3 v2/tools/check-wake-paths.py --selftest          # 384/384
python3 v2/tools/check-wake-depinfo.py --selftest        # 62/62
python3 v2/tools/check-wake-paths.py --strict            # errors=0
python3 v2/tools/check-wake-depinfo.py                   # errors=0

# (b) repro trên cây fixture (nhanh, không cần cargo)
python3 v2/agent/JOURNAL/kt242-wake-repro/fixture-probes.py

# (c) repro trên cây THẬT (có cargo check + cả ba lượt kiểm; tự khôi phục bằng trap)
bash v2/agent/JOURNAL/kt242-wake-repro/run-repro.sh r3a     v2/agent/JOURNAL/kt242-wake-repro/r3a.sh
bash v2/agent/JOURNAL/kt242-wake-repro/run-repro.sh rs-case v2/agent/JOURNAL/kt242-wake-repro/rs-case.sh
bash v2/agent/JOURNAL/kt242-wake-repro/run-repro.sh binpath v2/agent/JOURNAL/kt242-wake-repro/binpath.sh
```

Log từng lần đo ở `v2/agent/JOURNAL/kt242-wake-repro/logs/<tên>/`:
`cargo.log`, `cargo-rc.txt`, `lop1-thuong.log`, `lop1-strict.log`, `lop2.log`,
`git-truoc.txt`, `git-sau.txt`.

---

## 8. Tóm tắt một khối

- **Đường lọt MỚI**: **6** (N1–N6). Trong đó **N1/N2** lọt **cả hai lớp** và `cargo check` RC=0
  (nghiêm trọng nhất — "cửa thứ bảy" đưa TCB về `Ready` không qua điều kiện); **N3/N4** lọt
  **lớp 1**, lớp 2 bắt; **N5** lọt **luật 1–3** (lớp 2 không có luật này); **N6** là crash.
- **KT-184**: **CHƯA ĐÓNG**. **KT-194**: **CHƯA ĐÓNG**.
- **L2**: còn (không có đường lọt sống). **N3**: còn, câu chữ hợp đồng sai một phần.
  **DEBT-KT227-1**: còn, latent.
- **WB-16**: **không** được coi là đóng — N1/N2 mở lại đường tương đương ở tầng checker.
  **WB-17/WB-18**: đóng ở biên dịch/phạm vi, không tìm được biến thể sống.
