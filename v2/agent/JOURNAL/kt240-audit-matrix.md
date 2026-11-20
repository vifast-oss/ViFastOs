# KT-240 — Audit hợp đồng, thành phần dùng chung, ma trận review

**Vai**: luồng D (KT-240, D2-189 §1, D2-190 §5). **Chỉ audit + tài liệu, CẤM sửa mã.**
**Base**: `298b92c5`. **Nhánh**: `night4/D-kt240`, worktree `/private/tmp/night4-D-kt240`.
**Thời gian**: 2026-11-20 ~10:10–12:10 JST (ngân sách 2 giờ).

Selftest đo tại đây: lớp 1 **295/295**, lớp 2 **37/37** (`kt240-logs/l1-selftest.log`,
`kt240-logs/l2-selftest.log`). `v2 check` **PASS** 10:19 JST. `v2 status` tại
`kt240-logs/v2-status.log`.

---

## 1. DANH SÁCH THÀNH PHẦN DÙNG CHUNG (10 mục, D2-189 §1)

"Lớp 1" = `v2/tools/check-wake-paths.py` (7338 dòng).
"Lớp 2" = `v2/tools/check-wake-depinfo.py` (1422 dòng) — import lớp 1 làm module `CWP`
(hàm `load_static_checker`, depinfo dòng 89–122), **không chép luật**.

| # | Tên | Lớp 1 (file:dòng) | Lớp 2 (file:dòng) | Rủi ro mù chung (ví dụ WB) | Selftest chéo? |
|---|---|---|---|---|---|
| S1 | Phân vùng test/sản phẩm theo **dòng** — `test_line_ranges` | paths.py **371** (def) | depinfo dùng `CWP.test_line_ranges` tại scan R2 (dòng **596**) + dựng `test_only` | **WB-13**: vùng test rộng trên nhánh `match` làm cả hai cùng XANH (cả hai đọc cùng hàm cho R2; R1 lớp 2 dùng `_test_char_ranges` nên còn mù chung sâu hơn — xem S2) | **CÓ một phần**: depinfo (9) tệp ngoài `src/` ⇒ ĐỎ; (11) test-only ⇒ XANH. Thiếu ca nhánh `match` ở lớp 2 |
| S2 | Phân vùng test/sản phẩm theo **token-tree** — `_test_char_ranges` | paths.py **2756** (def), dùng trong `_check_wake_text` dòng **2891** | **TÁI DÙNG NGUYÊN VĂN**: `_check_wake_text` (S4) gọi `_test_char_ranges` nội bộ — lớp 2 không có bản riêng (`grep _test_char_ranges depinfo` = 0) | **WB-13 (đã vá KT-219)**: trước vá, `#[cfg(test)]` trên nhánh `match` nuốt nhánh sản phẩm ⇒ **cả hai lớp XANH**, `cargo check` RC=0. Sau vá cả hai ĐỎ. Đây là **bằng chứng D2-189**: một lỗi phân vùng làm cả hai cùng mù | **CÓ**: lớp 1 có ca KT-219 (nhánh match âm/dương + mục không thân); lớp 2 **KHÔNG có ca nhánh `match`** — chỉ (11)(13) generic. **Thiếu selftest chéo**: không có ca nào ở lớp 2 dựng `#[cfg(test)]` trên nhánh `match` rồi assert ĐỎ `E_DEPINFO_WAKE` |
| S3 | Nhận dạng transition — `_transition_calls` (token-tree) + `_var_may_be_ready` + `find_functions` + `enclosing` + `in_ranges` | paths.py **2945** (def `_transition_calls`), **3029** (`_var_may_be_ready`), **311** (`find_functions`), **2685** (`enclosing`), **2568** (`in_ranges`); dùng tại scan_v2 dòng **3298** | **TÁI DÙNG NGUYÊN VĂN**: depinfo dòng **599** `CWP._transition_calls`, **600** `CWP.in_ranges`, **602** `CWP.enclosing`, **623** `CWP._var_may_be_ready`, **597** `CWP.find_functions` | **WB-15 #15** (đã vá KT-231): regex một dòng `RE_READY_TRANSITION` (paths.py **280**) mù alias `S::Ready` ⇒ cả hai XANH. Sau KT-231 cả hai ĐỎ. **WB-16/WB-17 (OPEN)**: helper liên hàm/`const`/`static`/closure (S3 chỉ dataflow-lite trong cùng `Fn`), fn-pointer/generic/trait-object/UFCS (S3 chỉ khớp ident `transition_tcb` + `(` kế tiếp, bỏ qua mọi dạng lấy hàm làm giá trị; UFCS đọc sai đối số — KT-232) ⇒ **cả hai cùng mù theo thiết kế** | **CÓ**: lớp 2 có (14)(15)(16)(33)(35)(36)(37) — alias ĐỎ, `Blocked` XANH, param XANH, macro lồng ĐỎ. **Nhưng**: không có ca UFCS/fn-pointer nào ở **cả hai lớp** chứng minh cùng mù (đúng vì đang OPEN — repro thuộc luồng C/KT-239) |
| S4 | Luật token `wake` — `_check_wake_text` (+ `strip_rust_noise`, `find_functions`, `RE_WAKE_TOKEN`) | paths.py **2874** (def); `strip_rust_noise` **2575**; `RE_WAKE_TOKEN` ~**190** | **TÁI DÙNG NGUYÊN VĂN**: depinfo dòng **571** `CWP._check_wake_text(relp, text, CWP.test_line_ranges(text), safe_macros, sub)`, mã lỗi đổi sang `E_DEPINFO_WAKE` (dòng **575**) | **WB-14 #14** (đã vá KT-230): `_shadowed_safe_macros` cũ mù macro lồng ⇒ `matches!(m, wake, t)` lọt **cả ba lượt** (thường/`--strict`/dep-info). Sau KT-230 cả ba ĐỎ | **CÓ**: lớp 2 có (31)(32)(33) KT-230 (macro lồng ĐỎ, macro thật XANH, macro trong `.txt` ĐỎ). Đủ chéo |
| S5 | Phân tích macro — `_shadowed_safe_macros` + `_collect_safe_macro_defs` + hằng `SAFE_VALUE_MACROS` (+ `SAFE_TOKEN_MACROS`) | paths.py **2410** (def `_shadowed_safe_macros`), **2372** (`_collect_safe_macro_defs`), **2347** (`SAFE_VALUE_MACROS`), **2048** (`SAFE_TOKEN_MACROS`); `MACRO_SCAN_ROOTS` **2030** | **TÁI DÙNG NGUYÊN VĂN**: depinfo dòng **550** `CWP.SAFE_VALUE_MACROS - CWP._shadowed_safe_macros(root, triples)` | Như S4 (WB-14). Thêm: lớp 2 truyền `triples` từ **danh sách rustc nạp** (không phải toàn cây) — khác biệt có chủ đích, hẹp hơn nhưng fail-closed (thiếu tệp ⇒ quét ít hơn, không miễn thêm) | **CÓ** (như S4: ca 31/32/33). Đủ chéo |
| S6 | Lọc thư mục build — `_is_build_dir` | paths.py **1355** (def): chỉ bỏ qua khi `target` đứng **trước** `src` | **TÁI DÙNG NGUYÊN VĂN**: depinfo dòng **369** `CWP._is_build_dir(root, p)` (KT-226) | **WB-12** (`src/target/evil.rs`): trước KT-215/226 cả hai (lớp 1 năm chỗ `"target" in parts`, lớp 2 `"target" in relp.parts`) cùng bỏ qua ⇒ cùng XANH. Sau vá cả hai ĐỎ (`src/target`) / XANH (`kernel/target`) | **CÓ**: lớp 2 có (28) bảng `_is_build_dir`, (29) `src/target` ĐỎ, (30) `kernel/target` XANH. Đủ chéo |
| S7 | Chuẩn hoá đường dẫn — `rel()` (+ `resolve_include_targets` chỉ lớp 1 / `resolve_dep` chỉ lớp 2) | paths.py **2706** (`rel`); **1752** (`resolve_include_targets`: phân giải `include!`/`#[path]` theo gốc rustc đúng, KT-213) | depinfo dòng **514/523** `CWP.rel`; dòng **271** `resolve_dep` (đổi mục `.d` thành tuyệt đối theo cwd workspace) | **WB-07/WB-11** (gốc `#[path]` non-mod-rs): chỉ lớp 1 phân giải (lớp 2 không phân giải — đọc danh sách rustc). **Không mù chung** ở đây: lớp 2 miễn nhiễm theo thiết kế (rustc đã phân giải đúng). Nhưng **S7b**: `in_test_only_wake_scope` (paths.py **176**, regex `^v2/(kernel|sim)/[^/]+/(src/.*\.rs|build\.rs)$`) vs `RE_CRATE_SRC` (depinfo **125**, gồm cả `v2/lib|tools`) — hai **mẫu phạm vi khác nhau**, đã gây lệch B3b/B12–B15 (KT-224/227) | **CÓ một phần**: (18)(19) `hidden_files`/`outside_static_scope`; (10) `evil.rs` ngoài mẫu tĩnh. **Thiếu**: không có ca chéo nào assert hai mẫu phạm vi đồng nhất trên cùng cây (lệch mẫu chỉ phát hiện qua review L53, không qua selftest) |
| S8 | Đồ thị module test-only — `test_only_modules` (+ `_module_graph`, `_mods_of`, `module_dirs`) | paths.py **1659** (`test_only_modules`), **1620** (`_module_graph` — KT-213 tách dùng chung với `module_dirs` **1681**), **1525** (`_mods_of`) | **TÁI DÙNG NGUYÊN VĂN**: depinfo dòng **1313** `CWP.test_only_modules(root)` (chỉ khi có tệp tên test, `RE_TESTISH` dòng **139**) | **WB-13** (qua S2); **B3b** (`#[cfg(feature="ktest")] mod`): đồ thị quyết định test-only — sai ở đồ thị ⇒ cả hai cùng miễn sai. KT-227 P0-4 đã đóng (lớp 2 trước đó miễn cả tệp producer) | **CÓ một phần**: (11) test-only ⇒ XANH. **Thiếu**: không có ca `feature="ktest"` ở lớp 2 (chỉ lớp 1 có); `RE_TESTISH` heuristic quyết định **có dựng đồ thị hay không** — heuristic sai (tệp test không khớp mẫu tên) ⇒ bỏ qua đồ thị ⇒ quét nhiều hơn (fail-closed, an toàn nhưng chưa có ca chứng minh) |
| S9 | Hằng allowlist transition — `WAKE_DOORS` + `NON_WAKE_READY_SITES` (+ `WAKE_PRODUCER`, `is_producer_file` riêng lớp 2) | paths.py **228** (`WAKE_DOORS`), **240** (`NON_WAKE_READY_SITES`), **140** (`WAKE_PRODUCER`); `RE_READY_TRANSITION` **280** (regex cũ, còn dùng cho fallback?) | **TÁI DÙNG NGUYÊN VĂN**: depinfo dòng **610** `CWP.WAKE_DOORS`, **641** `CWP.NON_WAKE_READY_SITES`, **504** `CWP.WAKE_PRODUCER`; docstring dòng **31–32, 45–46** liệt kê | Lệch allowlist đã từng gây mù một bên (KT-226: lớp 2 miễn cả tệp producer ⇒ XANH nơi lớp 1 ĐỎ — **mù một bên**, không phải mù chung; đã đồng nhất KT-226/227). `RE_READY_TRANSITION` còn trong docstring depinfo (dòng 31) nhưng code đã dùng `_transition_calls` — docstring cũ gây hiểu nhầm, không phải lỗi hành vi | **CÓ**: (14)(15)(16) R2 cửa/allowlist; (6)(27) producer chỉ miễn định nghĩa. Đủ chéo |
| S10 | Hạ tầng báo cáo/lọc tệp — `Report` + `hidden_files`/`outside_static_scope` (riêng lớp 2) + parse `.d` (`parse_depinfo`, `_split_dep_tokens`, `_unescape_dep`, `_join_continuations`) | paths.py **2715** (`Report`) | depinfo dòng **510** (`hidden_files`), **521** (`outside_static_scope`), **240** (`_split_dep_tokens`), **189** (`_join_continuations`); `depinfo_empty_errors` (fail-open P0-1, KT-227); parse `\ ` P0-2 | **Fail-open `.d` rỗng / parse `\ ` sai** (KT-224: 5 đường lọt cả hai lớp một phần do lớp 2 xanh-giả; KT-227 P0-1/P0-2 đã vá). Đây là **rủi ro riêng lớp 2**, không mù chung — nhưng khi lớp 2 xanh-giả + lớp 1 mù cùng repro (B3b, B12–B15, WB-13) thì hợp thành "lọt cả hai" | **CÓ**: (1)–(5), (17), (20)–(26) phủ parse/empty/JSON/skipped. Đủ cho hạ tầng lớp 2 |

**Tổng: 10 thành phần dùng chung. Thiếu selftest chéo: S1 (một phần), S2, S3 (UFCS/fn-pointer OPEN — thuộc KT-239), S7 (đồng nhất mẫu phạm vi), S8 (`feature="ktest"` ở lớp 2).**

Ghi chú quan trọng (phát hiện audit): `_check_wake_text(relp, text, tr, ...)` nhận tham số
`tr = test_line_ranges(text)` nhưng **không dùng** — hàm tự tính `test_ranges =
_test_char_ranges(code, relp)` ở dòng 2891 (biến `tr` chết). Tham số `tr` do caller truyền
(depinfo dòng 571) bị bỏ qua âm thầm. Không gây mù (cả hai cùng dùng `_test_char_ranges`),
nhưng là **mồi lệch tương lai**: người đọc tưởng `test_line_ranges` vẫn có hiệu lực trong R1.
Đề nghị lead: xoá tham số `tr` hoặc assert hai hàm đồng nhất (quyền Agent 1/lead, không thuộc KT-240).

---

## 2. AUDIT HỢP ĐỒNG `CHECKER-WAKE-CONTRACT.md` (phương pháp KT-220: khớp / lệch / không nguồn / không chứng cứ)

Phạm vi: §1–§5 (hợp đồng bảo vệ). Đối chiếu code HEAD + journal/log tương ứng.
Ký hiệu: **K** khớp · **L** lệch · **N** không nguồn · **C** không chứng cứ.

### 2.1 Kết quả tổng

| Mục | Khẳng định kiểm | K | L | N | C |
|---|---|---|---|---|---|
| §1 phạm vi cây nguồn (4 dòng bảng + KT-209/L2 + KT-181 + đích include + KT-211 + KT-225 + KT-202 + gốc `#[path]`) | 14 | 12 | 1 | 1 | 0 |
| §2 đã hỗ trợ (17 gạch đầu dòng) | 17 | 16 | 0 | 1 | 0 |
| §3 bị từ chối (14 dòng bảng) | 14 | 14 | 0 | 0 | 0 |
| §4 sai lệch còn mở (17 dòng bảng) | 17 | 15 | 1 | 1 | 0 |
| §5 ngoài phạm vi (5 gạch) | 5 | 5 | 0 | 0 | 0 |
| §6 cách chạy (§7 bảo trì không audit nội dung) | 1 | 1 | 0 | 0 | 0 |
| **Tổng** | **68** | **63** | **2** | **3** | **0** |

### 2.2 Chi tiết L / N

1. **L1 — §1 "Cả ba chế độ chạy trong `./v2/bin/v2 check` và `gate --fast`"** (§6 dòng 217):
   câu "Cả ba chế độ" gây hiểu "ba lớp độc lập" — **D2-189 §3 đính chính**: thường/`--strict`/dep-info
   là **ba lượt kiểm thuộc hai cơ chế**. `v2/bin/v2` dòng 344–361 đúng là 3 `run_step`
   (paths thường + `--strict` + depinfo) + 2 selftest. Hành vi đúng, **thuật ngữ trong hợp đồng
   chưa cập nhật D2-189**. Đề nghị lead sửa một câu.
2. **L2 — §4 dòng ":expr"**: "chỉ còn hở khi macro được gọi từ tệp ngoài `MACRO_SCAN_ROOTS`".
   Chưa chứng minh hết: `MACRO_SCAN_ROOTS` (paths.py 2030) không gồm `v2/test/fixtures`,
   `v2/tools` — đúng là "ngoài roots", nhưng hợp đồng §1 cũng ghi `v2/tools/**` ngoài phạm vi
   trong khi depinfo `RE_CRATE_SRC` (dòng 125) lại gồm `v2/tools` trong mẫu "hiển nhiên".
   Hai mẫu phạm vi **vênh nhau** (xem S7). Không phải lỗi hành vi đã đo, nhưng là **lệch mô tả
   phạm vi giữa hai tài liệu/code**. Đề nghị lead thống nhất.
3. **N1 — §1 KT-209/L2 "đo 2026-11-20: không có token `wake`"** (hai tệp shim/init):
   không tìm thấy log đo nào trong `JOURNAL/kt209-logs/` chứa lệnh/grep chứng minh.
   Kỹ thuật đúng (code không quét hai đường này — đã đọc `in_test_only_wake_scope`), nhưng
   **không có log nguồn**. Kế thừa phương pháp KT-220: **N**.
4. **N2 — §2 "`#[path]` phân giải theo thư mục tệp chứa (đúng rustc: `mod m { include!...`"**:
   ví dụ rustc cho `include!` trong `mod m` — log `kt212-attack-path-mod.md` §3 chỉ đo cho
   `#[path]`, không đo riêng cho `include!` trong inline module. Hành vi code
   (`resolve_include_targets`) đúng hướng, nhưng **thiếu log đối chứng cho khẳng định này**.
5. **N3 — §4 "F3 (v2/lib): `cfg_attr` lồng nhiều tầng ngoài thân macro ⇒ XANH ... §1 đã ghi"**:
   §1 thực tế **không ghi** trường hợp này (chỉ ghi "thân macro + include/path ngoài macro").
   Tham chiếu vòng sai mục. Nội dung kỹ thuật (không quét cấu trúc `v2/lib`) đúng, nhưng
   **không có nguồn cho câu "§1 đã ghi"**.

Không có **C** (mọi khẳng định "đã đóng" đều có repro ĐỎ + đối chứng + log trong journal
tương ứng: kt219/kt225/kt226/kt227/kt230/kt231 — đã kiểm file tồn tại ở §4 task này).

### 2.3 Ghi chú về `verify-numbers`

`v2/tools/verify-numbers.py` (KT-80, 5 luật OD-K6-5) kiểm **số đo tài liệu vs log**,
không phải hợp đồng checker. Hợp đồng wake **không thuộc phạm vi** verify-numbers
(verify-numbers kiểm sha/section/count, không kiểm khẳng định checker). Không ghi lệch —
chỉ ghi nhận phạm vi để lead khỏi kỳ vọng sai.

---

## 3. MA TRẬN REVIEW THEO NHÓM (cho reviewer độc lập sau này — KHÔNG tự chạy full)

Mỗi ô: mục tiêu · repro gợi ý · pass/fail. Reviewer **khác tác giả** (D2-190 §5, D2-192).

### Nhóm A — alias / UFCS / fn-pointer (WB-15 mở rộng, WB-17 OPEN)

| Ô | Mục tiêu | Repro gợi ý | Pass | Fail |
|---|---|---|---|---|
| A1 alias kiểu | `S::Ready` (use-as) qua mọi vị trí: đối số 2, `const X = S::Ready`, `static`, trả về hàm | `use C::TcbState as S; m.transition_tcb(t, S::Ready)` (đã ĐỎ sau KT-231 — giữ làm hồi quy) | ĐỎ `E_STRICT_TRANSITION`/`E_DEPINFO_TRANSITION` cả ba lượt | XANH bất kỳ lượt nào |
| A2 UFCS | Mọi dạng UFCS: `Machine::transition_tcb(m,t,Ready)`, `<M>::transition_tcb`, `<M as T>::transition_tcb`, UFCS qua alias trait | `let f = Machine::transition_tcb; f(m,t,TcbState::Ready)` (KT-232: SỐNG) | ĐỎ cả ba lượt, **đúng nguyên nhân** (không nhận lỗi lexer khác) | XANH hoặc ĐỎ sai mã |
| A3 fn-pointer/generic/trait-object | `let f: fn(..) = ...`, `F: Fn` generic, `Box<dyn Fn>`, `&dyn`, closure ép fn-pointer | 7 biến thể `JOURNAL/kt232-fnptr.md` (SỐNG) | ĐỎ cả ba lượt | XANH |
| A4 `Self`/đường dẫn lạ | `Self::Ready`, `<T>::Ready`, `crate::x::Ready` | biến thể của A1 | ĐỎ (fail-closed alias) | XANH |

### Nhóm B — helper liên hàm / closure / const / static (WB-16 OPEN)

| Ô | Mục tiêu | Repro gợi ý | Pass | Fail |
|---|---|---|---|---|
| B1 helper cùng tệp | `fn h(m,t,s){ m.transition_tcb(t,s) }` + `h(m,t,Ready)` | `JOURNAL/kt234-interfn.md` F1 (SỐNG) | Không biên dịch được (sau thu hẹp API, D2-187) HOẶC ĐỎ checker | XANH + `cargo check` RC=0 |
| B2 helper khác tệp | helper ở module/tệp khác | kt234 F2 (SỐNG) | như B1 | XANH + RC=0 |
| B3 hàm trả về `Ready` | `fn r() -> TcbState { Ready }` + `transition_tcb(t, r())` | kt234 (SỐNG) | như B1 | XANH + RC=0 |
| B4 const/static | `const C: TcbState = Ready;` / `static S: ...` | kt234 (SỐNG) | như B1 | XANH + RC=0 |
| B5 closure | `let c = \|m,t\| m.transition_tcb(t, Ready);` / closure trả Ready | kt234 (SỐNG) | như B1 | XANH + RC=0 |
| B6 dataflow-lite hiện tại không báo oan | `block_current(cur, to)` param; `TcbState::Blocked` | selftest (36)(37) lớp 2 + tương đương lớp 1 | XANH | ĐỎ oan |

### Nhóm C — macro / cfg / nguồn sinh (WB-10 PARTIAL, WB-14 đóng, B17, WB-16/17 qua macro)

| Ô | Mục tiêu | Repro gợi ý | Pass | Fail |
|---|---|---|---|---|
| C1 macro sinh `wake`/`Ready` | `macro_rules!` sinh lời gọi `wake`/`transition_tcb(Ready)` ở tệp gọi; macro lồng (KT-230 hồi quy) | kt230 repro (đã ĐỎ — giữ) | ĐỎ cả ba | XANH |
| C2 passthrough `:expr` | `$e:expr` chở `include!`/UFCS/fn-pointer | `m!{ transition_tcb(t, Ready) }` qua `:expr` | ĐỎ (hoặc không biên dịch sau thu hẹp API) | XANH + RC=0 |
| C3 `cfg(test)`/`cfg(feature)` trên item/match | `#[cfg(test)]` trên nhánh match (KT-219 hồi quy), trên item, `cfg_attr` lồng | kt219 repro (đã ĐỎ — giữ) | ĐỎ khi sản phẩm / XANH khi test thật | Ngược lại |
| C4 `build.rs`/`OUT_DIR`/`concat!`/`env!` | `include!(concat!(env!("OUT_DIR"), "/x.rs"))` | KT-233: 5 dạng bị bắt + 1 đường sống (crate ngoài gốc) | Fail-closed (ĐỎ), trừ đường WB-18 OPEN ghi nhận | PASS im lặng |
| C5 proc-macro | `paste!`/`concat_idents!` sinh `wake`/`Ready` | chưa có repro (hợp đồng §5 tuyên bố ngoài phạm vi) | Ghi nhận ngoài hợp đồng, không tuyên bố đóng | Tuyên bố đóng mà không có repro |

### Nhóm D — crate mới / dependency mới / đường dẫn đặc biệt (WB-18 OPEN, WB-12 đóng, WB-07/08 đóng)

| Ô | Mục tiêu | Repro gợi ý | Pass | Fail |
|---|---|---|---|---|
| D1 crate ngoài ba gốc | `v2/extra/evil` + `$OUT_DIR` (KT-233 repro SỐNG) | `JOURNAL/kt233-buildrs.md` | Sau luồng B: bị kiểm hoặc từ chối rõ (D2-188), không cảnh báo-rồi-PASS | Cảnh báo rồi PASS |
| D2 `src/target/` vs `target/` | `src/target/evil.rs` ĐỎ; `kernel/target/` XANH | selftest (28)(29)(30) lớp 2 + tương đương lớp 1 | Đúng cả hai | Ngược lại |
| D3 `#[path]` ra ngoài `src` / non-mod-rs | `#[path="../../../evil.rs"]` trong inline module non-mod-rs | kt212 repro #11 (đã ĐỎ — giữ) | ĐỎ cả hai | XANH |
| D4 dependency mới / workspace member mới | thêm path-dependency ngoài workspace, thêm member | chưa có repro chuẩn (thuộc luồng B/KT-238) | Nằm trong `cargo metadata` scope hoặc bị từ chối rõ | Lọt scope im lặng |
| D5 đuôi/dấu cách/không-`.rs` | `.txt` chứa macro (KT-225 hồi quy), tên tệp dấu cách, symlink | KT-225 TXT (đã ĐỎ — giữ); depinfo (21) dấu cách | ĐỎ / parse đúng | XANH-giả / parse sai |

**Tổng ma trận: 4 nhóm, 20 ô (A 4 + B 6 + C 5 + D 5).** Ô hồi quy (đã ĐỎ, giữ bền —
D2-190 §3): A1, C1, C3, D2, D3, D5. Ô OPEN chờ đóng (A2, A3, B1–B5, C2, C4-một phần, D1, D4)
thuộc luồng A/B/C — reviewer sau dùng ma trận này nghiệm thu, không phải KT-240.

---

## 4. KIỂM SỔ (STATE / QUEUE / BYPASS-REGISTRY) — chỉ đề xuất, KHÔNG sửa

| # | Sổ | Lệch | Đề nghị lead (Agent 0) |
|---|---|---|---|
| K1 | `STATE.md` khối hiện tại: `HEAD: 59bede14 (main, đã push — chưa push: 0)` | **LỆCH**: HEAD thực tế worktree này là `298b92c5` (nhánh `night4/D-kt240`); `59bede14` là commit cha. Ngoài ra khối ghi "khối trạng thái hiện tại duy nhất — cập nhật sau D2-187..D2-193" nhưng HEAD trong khối (`59bede14`) ≠ HEAD KT-236 (`298b92c5`) ngay sau D2-193 | Cập nhật HEAD + nhánh + dirty khi tích hợp; hoặc ghi rõ HEAD của **checkout nào** (tránh lẫn worktree song song D2-192) |
| K2 | `STATE.md`: `TASK: DONE=206` | **LỆCH NHẸ**: `v2 status` tại đây báo `DONE=207 READY=4 BLOCKED=0 REVIEW=2` (đã gồm KT-236 DONE). Chênh 1 = KT-236 | Đồng bộ số DONE/READY với `v2 status` khi tích hợp |
| K3 | `STATE.md`: `CỔNG: gate --fast PASS trên a08bf042` | **KHỚP có caveat đúng**: đã ghi rõ "KHÔNG phải gate trên HEAD" (đúng D2-191). Không lệch — chỉ nhắc lead: sau tích hợp đợt A–D **phải chạy full gate trên commit cuối** (D2-190 §6, D2-191 §3) | Giữ nguyên; lên lịch gate cuối |
| K4 | `STATE.md`: `BUILD-INPUTS 233 mục / fprint 43df126a / dirty=0` | **KHỚP**: `v2 status` tại đây báo đúng `233 / 43df126a4a6702ec / dirty=0` | Không cần sửa |
| K5 | `QUEUE.md` KT-237..240 | **KHỚP**: cả 4 ở `READY`, phụ thuộc `KT-236`, ngân sách/mô tả đúng D2-192. KT-240 đang thi hành tại worktree này (chưa `start`, đúng vì agent D không có quyền agentctl — lead xử lý) | Lead `start KT-240` (hoặc ghi nhận) khi tích hợp |
| K6 | `BYPASS-REGISTRY.md` mapping + trạng thái | **KHỚP**: WB-01..18 đầy đủ; mapping hệ cũ đúng (§D2-175: #11→WB-07/WB-11 phân biệt hai hệ, #13→WB-13, #14–#18 đúng); WB-16/17/18 OPEN + P0 chặn nghiệm thu đúng D2-193 §2; hướng đóng D2-187/188 đúng | Không cần sửa |
| K7 | Hợp đồng `CHECKER-WAKE-CONTRACT.md` vs QUEUE KT-184/KT-194 REVIEW | **KHỚP**: hợp đồng dòng 9 tự ghi "KT-184, KT-194, KT-211, KT-213 vẫn REVIEW" — QUEUE dòng 315/318 xác nhận KT-184/KT-194 REVIEW (KT-211/213 DONE nhưng hợp đồng viết trước khi đóng? — kiểm: QUEUE KT-211 DONE, KT-213 DONE; hợp đồng dòng 9 ghi cả hai REVIEW là **cũ** so với QUEUE). **Lệch nhẹ**: hợp đồng chưa cập nhật trạng thái KT-211/213 | Đề nghị tác giả hợp đồng (không phải KT-240) cập nhật dòng 9 khi sửa hợp đồng lần tới |

---

## 5. BẮT BUỘC KIỂM (chỉ đọc + chạy)

- `check-wake-paths.py --selftest`: **295/295** (`kt240-logs/l1-selftest.log`).
- `check-wake-depinfo.py --selftest`: **37/37** (`kt240-logs/l2-selftest.log`).
- `./v2/bin/v2 check`: **PASS** (10:19 JST, 2026-11-20; xem log cổng trong journal — không
  sửa mã nên không cần gate lại; **gate cuối thuộc về lead sau tích hợp A–D**, D2-191 §3).
- QEMU: **0** tiến trình do task này tạo (chỉ chạy checker Python + `v2 check` compile;
  không boot QEMU). `ps` kiểm orphan: không có QEMU nào của task này.

---

## 6. BÀN GIAO + BLOCKER

- Bàn giao: 10 thành phần dùng chung (5 thiếu selftest chéo — S1/S2/S7/S8 + S3-phần-OPEN);
  audit hợp đồng 68 khẳng định (63 khớp / 2 lệch thuật ngữ-phạm vi / 3 không nguồn /
  0 không chứng cứ); ma trận 4 nhóm 20 ô; 7 mục kiểm sổ (2 lệch + 1 lệch nhẹ + 1 cũ + 3 khớp).
- Blocker: **không có blocker kỹ thuật** (đọc + chạy xong trong ngân sách). Hết 2 giờ:
  KHÔNG tự chạy full ma trận §3 (đúng lệnh task); KHÔNG sửa QUEUE/STATE/DECISIONS/hợp đồng
  (quyền Agent 0/lead).
- Tệp nộp: `v2/agent/JOURNAL/kt240-audit-matrix.md` (tệp này) + `v2/agent/JOURNAL/kt240-logs/`
  (l1-selftest.log, l2-selftest.log, v2-status.log). Commit `[KT-240]` trên nhánh
  `night4/D-kt240`, KHÔNG push.
