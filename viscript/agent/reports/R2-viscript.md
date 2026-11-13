<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R2-viscript — báo cáo vòng 2, dòng ViScript

Agent 2 · 2026-11-13 · `head` = `0ce1adc` là **head lúc thu số liệu** (QUEUE có đủ **20 hàng** 6 cột; mọi task trước ST-17 đã `DONE`);
commit chứa báo cáo là commit `[ST-17]` kế tiếp. Bản máy đọc: [`R2-viscript.json`](R2-viscript.json) —
**đúng 11 khoá** Khối E (`project = "viscript"`, `round = 2`).

## 1. Tóm tắt (5 dòng)

1. **VS0 chạy được đầu–cuối**: `vis run <tệp.vis>` (parse → lower → check → interp), exit 0/1/2, TAP tuỳ
   chọn; nội suy `{expr}` hạ **trong cùng ngữ cảnh giải tên** (không parse lúc chạy).
2. **Bộ ngữ nghĩa 309 ca / 16 nhóm** (`viscript/test/sem/**`) chạy **qua binary** `vis run`: so **byte**
   stdout, mã thoát, `steps > 0`; khoá cứng số ca/nhóm + test **chống bỏ ca** (đếm `.vis` trên đĩa).
3. **247 test workspace** (96 `vis-syntax` + 56 `vis-types` + 24 `vis-interp` + 33 `vis-cli` + 20 `vis-diag`
   + 18 `vis-fmt`); `./vis-dev gate --fast` **PASS** (fmt, clippy `-D warnings`, luật R1..R4, self-test case
   âm, build, doc-examples, L0 test).
4. **`./vis-dev nightly` đã có** (trước đây `exit 2`): `gate --fast` + **fuzz parser N phút** + **vòng tròn
   fmt** ⇒ `vis-dev nightly: PASS`; fuzz parser **1 giờ liên tục: 182.328.926 ca / 4.145.431.263 token /
   0 vi phạm**.
5. **Sổ mã 5 khối `01xx..05xx`**, `all_codes()` = **21**; `vis explain` phủ cả 5 sổ; `next_block()` = **601**.

## 2. Bảng task vòng 2 (ST-11 → ST-17)

| ID | Trạng thái | Commit | Cổng | Kết quả đo được |
|---|---|---|---|---|
| ST-03 | DONE | `ada1ebc` | fast | `vis-diag`: cột theo Unicode scalar, `render`, bảng vi/en/ja; **20 snapshot `render`** khoá hành vi |
| ST-11 | DONE | `94a5342` | fast | `vis-types` nền: `Type` (VD-43), HIR, giải tên, `lower(CST)→HIR`; 15 khối tài liệu hạ đúng |
| ST-12 | DONE | `273a921` | fast | kiểm kiểu VS0 + `match` xét hết; 116 ca (89 trong `tyck.rs` + 30 `exhaustive.rs`); `VS0401/0402/0403/0412` |
| ST-12b | DONE | `dec75e3` | fast | 7 lỗi kiểm kiểu (review độc lập) + VD-48; **227** test |
| ST-13 | DONE | `5e9a962` | fast | `vis-interp` + builtins lõi; **79/79 ca đỏ trước**; sổ `05xx` |
| ST-13b | DONE | `f1ac635` | fast | 10 mục review ST-13 (Never, range tràn, vòng lặp, đơn vị, phương thức, vị trí, `break/?`); cap độ sâu; **246** test |
| ST-14 | DONE | `9aafdb8`, `4d52164` | fast | `vis run` + TAP + **100 ca** + nội suy `{expr}`; **247** test |
| ST-15b | DONE | `806e29a` | fast | 4 lệch thiết kế (push CoW, closure snapshot, `{x, ..}`, pattern trần): **4 ca sem mới** + **1 kỳ vọng cũ sửa có lý do** (`exhaustive.rs` ca `{name, ..}`) ⇒ 309 ca |
| ST-15 | DONE | `27aa6aa` | fast | **302 ca / 16 nhóm**; cap `MAX_CALL_DEPTH = 2.000`; luồng 256 MiB |
| ST-15b | DONE | `806e29a` | fast | 4 lệch thiết kế: `push` CoW, closure snapshot, `{x, ..}`, pattern trần ⇒ **309 ca** |
| ST-16 | DONE | `a89f036` | fast | fuzz parser **1 giờ / 0 vi phạm**; `vis-dev nightly` PASS |
| ST-17 | READY → **báo cáo này** | — | fast | tệp này + `R2-viscript.json` (task đang viết; QUEUE@head ghi `READY`) |

**20 hàng QUEUE** khớp đúng `queue_at(0ce1adc)` (không thừa, không thiếu); `ST-17` giữ `READY` trong JSON vì
báo cáo này chính là sản phẩm của nó (không tự lật trạng thái QUEUE).

## 3. Số đo (mọi số tái lập bằng lệnh ghi kèm, chạy 2026-11-13 09:44 JST)

| Số | Giá trị | Lệnh |
|---|---|---|
| test workspace | **247** = 96 syntax + 56 types + 24 interp + 33 cli + 20 diag + 18 fmt | `cargo test --workspace`; theo crate: `cargo test -p <crate> -- --list \| grep -c ': test'` |
| ca ngữ nghĩa | **309 ca / 16 nhóm** | `ls test/sem/*/*.vis \| wc -l`; `ls -d test/sem/*/ \| wc -l` |
| sổ mã | **21 mã**, 5 khối `01xx..05xx`, `next_block()` = 601 | `./target/debug/vis explain --list \| wc -l` |
| khối tài liệu | **24** (17 xanh + 7 ngoại lệ đỏ đúng), 12 fixture QD | dòng `--- doc-examples: OK …` của `./vis-dev gate --fast` |
| fuzz parser 1 giờ | **182.328.926 ca**, 4.145.431.263 token, 50.647 ca/s, 3.600 s, **0 vi phạm** | `./target/release/vis-parse-fuzz --seconds 3600 --seed 20261113` (log `JOURNAL/st16-fuzz-parse.log`) |
| nightly | **PASS** (L0 + fuzz + vòng tròn fmt 309 tệp) | `./vis-dev nightly --fuzz-minutes 1` |
| phụ thuộc ngoài | **3** (Unicode — VD-33) | `grep -h '^[a-z_]* = ' crates/*/Cargo.toml \| grep -v 'path = '\| grep -v 'workspace = true'` |

**Số không tái lập được** (không ghi vào báo cáo theo yêu cầu "bỏ, không đoán"): số token golden, số biểu
thức golden, số ca chế độ lệnh, số snapshot `render`, số ca tyck từng tệp — các con số này chỉ tồn tại trong
kỳ vọng của harness (không có bộ đếm độc lập); báo cáo dùng **số test chạy được** ở trên.

## 4. Quyết định vòng 2 (VD-42..VD-51, mỗi cái một dòng)

- **VD-42** — cấp mã: `01xx` từ vựng, `02xx` cú pháp, `03xx` tên, `04xx` kiểu, `05xx` runtime; 4 chữ số.
- **VD-43** — `Type` chuẩn hoá (int/float/size/duration/percent/…); literal điền kiểu ở tầng hạ.
- **VD-44** — hạ **permissive**: chỉ báo tên chưa khai khi **gán**; 8 tên dựng sẵn; không panic.
- **VD-45** — `match` xét hết theo miền hữu hạn của kiểu; guard/or-pattern tính là nhánh.
- **VD-46** — chẩn đoán runtime in `<dòng>:<cột>` **thật** từ `span` + văn bản.
- **VD-47** — ngữ nghĩa VS0: giá trị câu lệnh cuối (`if`/`match`/khối), `Show` đơn vị, nội suy hoãn.
- **VD-48** — phạm vi VS0: kiểu ngoài VS0 (`AnyError/cap/ref/Json/…`) vẫn `VS0301` + thông điệp vis-std;
  phép `percent/?/size` chốt (`percent / percent ⇒ float`, `size % size ⇒ size`).
- **VD-49** — bộ ngữ nghĩa `test/sem` + TAP: kỳ vọng theo **thiết kế**, không theo hiện thực; bỏ ca khi lệch.
- **VD-50** — cap độ sâu gọi `MAX_CALL_DEPTH = 2.000` (đo 10^4 tràn stack kể cả 256 MiB) ⇒ vượt ⇒ `VS0501`.
- **VD-51** — closure **chụp snapshot** env; `push` trên `var` ghi lại lvalue (CoW); `{x, ..}` phủ hết.

## 5. Lỗ hổng đã biết (đầy đủ vòng 2 — VD-38 + JOURNAL §6)

1. **Tham số lambda là `{error}`** — chưa suy kiểu hai chiều (§3.4); ca `let f = (x) => x; f(1)` khoá hành vi này.
2. **`match`/`if` làm đối số gọi** chưa parse (`print(match …)` ⇒ `VS0201`) — giới hạn văn phạm §A.3.
3. **TAP ra stderr** (`vis run --tap`) để giữ stdout sạch cho chương trình.
4. **Nội suy không đệ quy vào phần chữ**: `"{{x}}"` ⇒ `{x}`.
5. **`\u{…}`** trong chuỗi chưa hỗ trợ.
6. **`char` escape** (`'\n'`) bị lexer từ chối **VS0105** — **theo VD-30, không phải lỗi**.
7. **Range bị vật chất hoá** (cap `10^7` phần tử/lần) — giữ nguyên con số, không nới.
8. **`None(1)`** chưa kiểm đối số.
9. **Map literal trùng khoá** — giữ cả hai, `get` lấy bản đầu.
10. **`str.chars()`** trả `list[str]` (chưa phải `list[char]`).
11. **Closure snapshot** (VD-51): biến `var` cập nhật **sau** khi tạo closure thì closure không thấy.
12. **`vis fmt` cho `.vish`** chưa có đặc tả ⇒ `Bail out!`.
13. **`render`/`DiagLine`** chưa nối với `vis check --lang`.
14. **`.vish` khi chạy** chỉ `Bail out!` (chế độ lệnh = VS1).

## 6. Đối chiếu điều kiện chốt VS0 (§19) + đề xuất

| Điều kiện §19 | Kết quả | Bằng chứng |
|---|---|---|
| ≥ 300 test `.vis` | ✓ **309** | `ls test/sem/*/*.vis \| wc -l` |
| fuzz parser 1 giờ, không panic | ✓ 182.328.926 ca, **0 vi phạm** | `JOURNAL/st16-fuzz-parse.log` |
| vòng tròn fmt | ✓ trong `vis-dev nightly` (309 tệp bất động) | `./vis-dev nightly` |
| 100 % fixture tài liệu parse | ✓ 24/24 khối (17 xanh + 7 ngoại lệ đỏ đúng) | dòng `doc-examples` của cổng |

**Đề xuất cho Agent 0** (tôi **không** sửa các tệp này):
1. `viscript/AGENTS.md`: dòng lệnh thêm `./vis-dev nightly [--fuzz-minutes N]` (đã có, không còn `exit 2`);
   dải quyết định ViScript nay là **VD-01..VD-51** (không phải VD-41).
2. `AGENTS.md` §8: nhắc **vòng 2** đã chốt (`ST-11..ST-17`, `R2-viscript`).
3. `viscript/agent/QUEUE.md`: **đã sửa** (`0ce1adc` thêm cột DoD cho ST-03/ST-15b) — `report-check` giờ
   `PASS (2 báo cáo, 20 hàng QUEUE phủ đủ)`.
