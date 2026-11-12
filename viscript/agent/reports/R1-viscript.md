<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R1-viscript — báo cáo vòng 1, dòng ViScript

Agent 2 · 2026-11-12 · `head` = `c666f57` là **head lúc thu số liệu**; commit chứa báo cáo là commit kế tiếp.
Bản máy đọc: [`R1-viscript.json`](R1-viscript.json) — 11 khoá Khối E, `project = "viscript"`. Mọi số dưới đây do máy sinh.

## 1. Tóm tắt (5 dòng)

1. **Hạ tầng ViScript xong**: workspace `viscript/`, cổng `./vis-dev` (fmt, clippy, R1 ≤1.500 dòng, R2 cấm `unsafe`, R3 SPDX theo loại tệp, R4 `vis-host-vifast` ngoài cây) + **5 case âm** chứng minh cổng bắt được vi phạm.
2. **Sổ mã lỗi + CLI**: `vis-diag` có đúng **10 mã `VS0101..VS0110`** của SPEC-lex §7, `Code`/`Code::severity`/`lookup`/`next_block`; `vis-cli` cho `vis explain` (`exit 0` mã có thật, `exit 2` mã sai/thiếu tham số).
3. **10 test** xanh (6 `vis-diag` + 4 `vis-cli` integration chạy binary thật); `./vis-dev gate --fast` **PASS**.
4. **`./vis` chạy trong shell sạch** (tự tìm `cargo` ở `$HOME/.cargo/bin`); cổng có bước `vis explain` nên lỗi 127 không lọt lại.
5. **Chưa có lexer/parser** (ST-04 chưa mở): SPEC-lex còn `REVIEW`, thiếu kind/keyword dùng trong chính golden ⇒ **không tự đặt ngữ nghĩa** (YC-14).

## 2. Bảng task (11 hàng QUEUE)

| ID | Trạng thái | Commit | Cổng | Ghi chú |
|---|---|---|---|---|
| ST-01 | DONE | `cbf512e`, `f6222a5` | fast | workspace + `vis-dev` R1..R4 + 5 case âm |
| ST-02a | DONE | — (Agent 1 soạn) | — | APPROVED ghi trong ô bảng, **chưa đúng mẫu dòng riêng** (YC-14) |
| ST-02b | DONE | `a48857b`, `c666f57` | fast | sổ 10 mã + `vis explain`; sửa theo review #14 |
| ST-03 | READY | — | — | `vis-diag` cột scalar/render/vi-en-ja |
| ST-04 | BLOCKED (QUEUE ghi `READY`) | — | — | **chưa được mở**: SPEC-lex `REVIEW` + thiếu kind/keyword |
| ST-05..ST-09 | BLOCKED | — | — | chuỗi phụ thuộc ST-04 |
| ST-10 | DONE | `bc1dcf5` | fast | báo cáo này |

## 3. Lỗi tìm thấy

- **`./vis` exit 127** trong shell sạch (dùng `cargo` trần) — tái phát đúng lỗi vừa vá cho `vis-dev`; đã sửa bằng `find_cargo` và **cổng bắt được** (fixture `env -i`).
- **Sổ lệch `VS0107`**: bản code paraphrase "tab (tab = 4, vẫn lex nhưng phải báo)" trong khi SPEC ghi "tab (cảnh báo-lỗi; tab = 4)"; test đối chiếu `CODES ↔ SPEC-lex §7` bắt được và đã sửa về **nguyên văn**.
- **Quy tắc cấp tự chế đè thiết kế**: code gán `04xx` = runtime trong khi thiết kế dùng `VS0412` = "kiểu không khớp" ⇒ đổi thành `04xx` **kiểu** và ghi rõ là **đề xuất chờ Agent 0** (YC-14).
- **`code()` cho 5 chữ số** khi `n > 9999` và `Diag::new(u16)` nhận mã ngoài sổ ⇒ `code()` panic khi vượt 4 chữ số, `Diag::new(&'static Code)`.
- **Không có lỗi runtime nào** (chưa có lexer/parser để sai).

## 4. Số đo thay đổi (máy sinh)

| Số | Giá trị | Nguồn |
|---|---|---|
| test workspace `viscript` | **10** (6 diag + 4 cli) | `cargo test --manifest-path viscript/Cargo.toml --workspace -- --list` |
| mã lỗi trong sổ | **10** (`VS0101..VS0110`), đối chiếu SPEC-lex §7 | test `so_khop_spec_lex_that` |
| case âm của cổng | **5** (SPDX, 1.501 dòng, `unsafe`, `vis-host-vifast`, thiếu cargo) | `viscript/test/fixtures/run_fixtures.sh` |
| phụ thuộc ngoài | **0** (`vis-cli` → `vis-diag` là path nội bộ) | `viscript/Cargo.toml`, `Cargo.lock` |
| tệp lớn nhất | 138 dòng (`viscript/bin/vis-dev`) | `wc -l` |
| L1/sim | **chưa có** (dòng ViScript chưa có sim) | `./vis-dev nightly` → exit 2 |

## 5. Golden/fixture đã thêm

- **5 case âm** của cổng (`run_fixtures.sh`, TAP `1..5`): thiếu SPDX; tệp 1.501 dòng; `unsafe`; `vis-host-vifast` trong `viscript/`; môi trường sạch không `cargo` (đòi "không thấy cargo", **cấm 127**).
- **Test đối chiếu nguồn**: `CODES` ↔ `docs/viscript/SPEC-lex.md` §7 đọc thẳng từ đĩa (không chép tay).
- **Bảng mã**: `vis explain --list` in đủ 10 mã kèm nhãn lỗi/cảnh báo — dùng được cho người đọc lẫn máy.
- **Chưa có golden token** (60 golden thuộc ST-04, test đỏ đầu tiên của lexer).

## 6. BLOCKED + câu hỏi + đề xuất

| Câu hỏi | Nội dung | Cần gì |
|---|---|---|
| **YC-14** | ST-02a APPROVED (dòng riêng); VD-27 chốt `04xx` kiểu, ST-02b chỉ sửa bảng mã; ST-02b/ST-10 DONE, ST-03/ST-04 READY; `3d5539a` do người điều phối | **Đã chốt** |
| **ST-03** | `vis-diag`: cột theo Unicode scalar, render, bảng vi/en/ja | chờ lật READY |
| **ST-04** | Lexer theo SPEC-lex — cần bổ sung kind đang thiếu (`COMMENT`, `DOC_COMMENT`, `RANGE`, `RANGE_EQ`, `SET_LBRACE`, `WORD`, `DOLLAR`, `CARET`) + keyword + 4 ca âm golden (`VS0101/0102/0103/0110`) | errata SPEC-lex |

**Task bị chặn**: ST-05..ST-09 (chuỗi phụ thuộc). ST-03/ST-04 READY.

**Đề xuất 3 task kế**: (1) `ST-03` ngay khi được lật (độc lập SPEC); (2) `ST-04` chỉ sau errata có `APPROVED`; (3) quay lại dòng OS ngay khi có `APPROVED` cho KT-04d (mở KT-05) hoặc trả lời YC-12.
