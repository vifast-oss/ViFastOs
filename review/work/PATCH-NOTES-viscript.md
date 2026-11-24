# PATCH-NOTES — ViScript (vòng vá 2026-11-12)

Tệp đã vá: [`docs/viscript/ViScript-thiet-ke.md`](../../docs/viscript/ViScript-thiet-ke.md) (984 → 1478 dòng).
Nguồn sự thật: [`docs/QUYET-DINH-sau-review.md`](../../docs/QUYET-DINH-sau-review.md) (QD).
Chỗ QD **chưa nói** (không phải mâu thuẫn) mà tài liệu buộc phải chọn để tự chứa: liệt kê dưới đây để Agent 0 xác nhận. **Không** mục nào ở đây mâu thuẫn QD.

## 1. Chọn mặc định khi QD im lặng (cần xác nhận)

| # | Chỗ | Đã chọn | Vì sao / rủi ro |
|---|---|---|---|
| 1 | `item` của Phụ lục A | Thêm `alias_decl`, `newtype_decl` vào `type_item` (QD §7.5 chỉ liệt kê use/fn/cmd/type/enum/trait/impl/const/test/extern) | §3.2 và danh sách từ khoá vẫn dùng `alias`/`newtype`; thiếu luật thì cổng "mọi khối mã parse được" đỏ |
| 2 | `@` trong `main` | Bắt buộc `p"..."` (`@ p"./data"`), không dùng `@ "./data"` | Hệ quả trực tiếp của luật "path phải tường minh" (QD §7.3). Nếu Agent 0 muốn `@` nhận `str` thì phải khai ngoại lệ |
| 3 | `float / 0.0` | IEEE 754 (±inf/NaN), **không panic** | QD chỉ chốt NaN cho so sánh/`Eq`; chọn theo f64 để có oracle vi sai |
| 4 | `{` record vs map | Lookahead LL(2): `{ IDENT ':'` → record literal; còn lại → map. Map có khoá là định danh trần phải viết `{(k): v}` / `{"k": v}` | Không có luật này thì `{name: "An"}` và `{"a": 1}` mập mờ; QD không nói |
| 5 | `path("...")` với chuỗi sai | Không chốt hành vi (lỗi kiểu hay `Result`); chỉ chốt "phải tường minh" | QD §7.3 chỉ nói phải tường minh; để `ST-02b`/`ST-04` chốt cùng sổ mã lỗi |
| 6 | `size`/`percent` literal | `size` chỉ nhận `INT` (`1.5kb` không hợp lệ); `percent` nhận `INT` **và** `FLOAT` (`0.5pct`) | Bảng QD không nói; giữ tối thiểu |
| 7 | `\u{X..X}` | 1–6 chữ số hex [TK] | QD viết `\u{X..X}`, không nêu số chữ số |
| 8 | `str` nhiều dòng `"""` | **Không** tự cắt thụt lề nội dung (giữ nguyên như nguồn, đã NFC) | QD không nói; cắt thụt lề là quyết định ngữ nghĩa, cần Agent 0 nếu muốn |
| 9 | `bytes` không phải ASCII | `b"..."` chỉ ASCII + `\xNN`; chưa có cách viết byte ≥ 0x80 ngoài `\xNN` | QD chỉ nói "chỉ ASCII + `\xNN`"; đủ dùng, không thêm cú pháp |
| 10 | Trường bản ghi | Bất biến ở bản này (không `var` cho trường), cập nhật bằng `with` | QD không nói; `type` + `with` đã đủ ngữ nghĩa giá trị |
| 11 | `1 < x < 10` | Lỗi cú pháp (so sánh không kết hợp) | QD §7.3 ghi "so sánh (không kết hợp)" |
| 12 | Số luật Phụ lục A | **111 luật** (QD §7.4 và PATCH-BRIEF ước "60–70") | Nhiều hơn vì tách terminal (`SIZE_UNIT`, `DUR_UNIT`, `NEWLINE/INDENT/DEDENT`) và tách `let_head`/`assign_head` cho khối một dòng. Không thiếu luật nào; gộp lại chỉ để đẹp số đếm |

## 2. Việc chưa làm được (cần Agent 0 / task khác)

| # | Việc | Trạng thái |
|---|---|---|
| 1 | `docs/viscript/SPEC-lex.md` (bảng token, escape, đơn vị, maximal munch, 60 golden token) | **Chưa có trong repo** — thuộc **ST-02a** (SA-C). Tài liệu này chỉ trỏ tới, không thay thế |
| 2 | 30 golden biểu thức ưu tiên; 10 ca mơ hồ chế độ lệnh; 60 golden token | Nội dung đặc tả đã có (bảng ưu tiên 12 mức, §9.3/9.4); **fixture** thuộc ST-02a/ST-06 |
| 3 | `docs/viscript/doc-examples-allow.txt` | Cổng **doc-examples parse** đã ghi ở §18.4 nhưng tệp ngoại lệ chưa tạo (thuộc ST-01/ST-02a) |
| 4 | Sổ mã lỗi `VS####` (ngoài `VS0107` đã có) | Thuộc **ST-02b**; tài liệu chỉ tham chiếu |
| 5 | `queue`/`agent` của dòng ViScript | Thuộc SA-D; tài liệu chỉ dẫn chiếu `viscript/agent/QUEUE.md` |
| 6 | `cmd_lower` (hạ `CmdCST` → HIR) | Cố ý để **vòng 2** (QD Q-D); vòng 1 chỉ `CmdCST` + golden (ST-06) |

## 3. Đã vá theo đúng QD (đối chiếu nhanh)

- §7.1 từ vựng: NFC, thụt lề/tab `VS0107`, 4 ca biên, số/float/`_`/tràn literal, đơn vị `size`/`duration`, bỏ `50%` → `50pct`, 5 dạng chuỗi + escape + nội suy, `#`/`set{`/`?` maximal munch, cột scalar.
- §7.2 chế độ lệnh: metachar, `"..."`/`'...'`, `$x`/`$env.NAME`, `;`, `&` (token cuối có khoảng trắng trước), `--`, `-5`, closure-arg tới `|` độ sâu 0, `shell_item`, `from json`/`where` trên `Json`, SIGPIPE/`take`, `complete`.
- §7.3 ngữ nghĩa: `%`, `**`, bit (`bit_not`/`ushr`/`shr` số học/đếm ≥ 64 panic/`checked_*`), bảng ưu tiên 12 mức, float/NaN, `percent` vs `float`, optional, xét hết, `Error`/`AnyError`/`?`, mã thoát, scope/shadowing, `s[i]` cấm + `xs[i]?`, panic payload/arena, tài nguyên cap phiên, `print`, plugin hạn chế, consent hash closure, async `Future`/`Task`, generic vs index, từ khoá (`const`, bỏ `as`, `test` ngữ cảnh, `std.testing`), `str(x)`, path tường minh, `str * int` bỏ.
- §7.4: Phụ lục A viết lại; Phụ lục B sửa (`|>` đầu dòng, `str(e.path)`, `time.now()`, `is_file()`, fence `vis`/`vish-session`, bỏ `>`, `s.repeat(n)`).
- §7.5: `ST-01..ST-10` (bảng §20.2), `ST-02a` REVIEW chặn `ST-04..ST-09`, `vis-host-vifast` ngoài `viscript/`, SPDX theo loại tệp, 1.500 dòng/`unsafe` có case âm, cổng doc-examples.
- §2.1 nhãn: `[TK]` mặc định, `[ƯL]`/`[GT]`/`[V1:*]`; không thêm số đo viết tay.

## 4. Xác nhận phạm vi

Chỉ ghi **hai** tệp: `docs/viscript/ViScript-thiet-ke.md` (ghi đè) và `review/work/PATCH-NOTES-viscript.md` (tệp này). Không sửa `docs/QUYET-DINH-sau-review.md`, không sửa tệp nào khác.
