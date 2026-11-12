# DECISIONS — ViScript

Người ghi: **Agent 0**. Agent 2 **không** sửa tệp này; cần đổi thì ghi `BLOCKED.md` + đề xuất.
Nguồn: `docs/viscript/ViScript-thiet-ke.md` §21–§22 và `docs/QUYET-DINH-sau-review.md` (QD) §7.

---

## Phần I — VD-01..VD-10 (chép từ tài liệu ViScript §22)

| Mã | Nội dung |
|---|---|
| **VD-01** | ViScript có kiểu tĩnh bắt buộc, không `any`, không null |
| **VD-02** | Một ngôn ngữ, hai chế độ cú pháp, cùng **HIR**/kiểm kiểu/VM; ViShell là chương trình riêng |
| **VD-03** | Ngữ nghĩa giá trị + RC + copy-on-write; không GC truy vết; chu trình chỉ qua `ref`, phá bằng `weak` |
| **VD-04** | Lõi (syntax, types, bytecode, vm) không phụ thuộc hệ điều hành, build được wasm32 |
| **VD-05** | Mọi truy cập hệ thống qua capability; không hàm toàn cục chạm hệ thống |
| **VD-06** | Interpreter cây là mô hình tham chiếu, giữ mãi để kiểm vi sai VM |
| **VD-07** | Không JIT; không `eval` chuỗi trong chế độ quyền hạn chế |
| **VD-08** | Thay đổi SPEC chỉ qua Agent 0; mỗi tính năng mới phải có ví dụ + test trước |
| **VD-09** | Từ khoá tiếng Anh, định danh Unicode (NFC), thông báo lỗi vi/en/ja |
| **VD-10** | Giấy phép MIT OR Apache-2.0; phụ thuộc phải tương thích |

## Phần II — Q1..Q8 (tóm tắt 1 dòng/mục, kèm chỗ QD §7 đã sửa)

| Mã | Chốt | Ghi chú sau QD §7 |
|---|---|---|
| Q1 | Từ khoá **tiếng Anh**; định danh Unicode cho phép tiếng Việt; thông báo lỗi có tiếng Việt | giữ nguyên |
| Q2 | `int / int` cho `int`; có lint khi chia hai literal | giữ nguyên |
| Q3 | Thụt lề **4 dấu cách bắt buộc, cấm tab** | **sửa bởi VD-12**: đầu vào nhận thụt lề nhất quán bất kỳ (đơn vị suy từ cấp đầu), tab = 4 + VS0107, `vis fmt` luôn ra 4 |
| Q4 | Tên **ViScript** (`.vis`, lệnh `vis`), **ViShell** (`.vish`, lệnh `vish`) | giữ nguyên |
| Q5 | `xs[i]` ngoài phạm vi **panic**; `xs.get(i)` trả `T?` | **thêm bởi VD-18**: có `xs[i]?` trả `T?`; `xs[-1]` panic có thông điệp |
| Q6 | Giấy phép **MIT OR Apache-2.0** | giữ nguyên (= VD-10, D2-21) |
| Q7 | `decimal` **không** trong lõi, là thư viện | giữ nguyên |
| Q8 | Toán tử bit là **hàm** (`bit_or`…), không ký hiệu, để giải phóng `\|` cho pipe | **sửa bởi VD-15**: `\|` **không bao giờ** là toán tử nhị phân trong biểu thức |

## Phần III — VD-11..VD-22: chốt bổ sung từ QD §7 (mỗi dòng một mục)

- **VD-11** Chuẩn hoá: **toàn bộ tệp nguồn được chuẩn hoá NFC trước khi lex** — 'ố' gõ kiểu nào cũng là một scalar.
- **VD-12** Thụt lề: đầu vào nhận thụt lề nhất quán bất kỳ, đơn vị suy từ cấp đầu; đầu ra `vis fmt` luôn **4**. Tab: lexer báo cảnh báo-lỗi **VS0107**, coi tab = 4, **vẫn lex tiếp**; `vis check` coi là lỗi, `vis fmt` sửa.
- **VD-13** Phần trăm: **bỏ literal `50%`**, dùng `50pct`; `%` chỉ là modulo; job spec dùng `job 1`.
- **VD-14** Chú thích: script — `#` ngoài chuỗi luôn là chú thích; lệnh — `#` chỉ là chú thích khi **đứng đầu từ** (sau khoảng trắng hoặc đầu dòng), nên `https://x/y#frag` giữ nguyên.
- **VD-15** `|` **không bao giờ là toán tử nhị phân trong biểu thức**. Pattern: or-pattern dùng `|`; lệnh: pipe ở **độ sâu ngoặc 0**; `|>` là pipe script. (Sửa Q8.)
- **VD-16** Bảng ưu tiên chốt (cao → thấp): postfix (`. ?. () [] ? with`) → `**` → `-` một ngôi → `* / %` → `+ -` → `.. ..=` → `??` → so sánh (không kết hợp) → `not` → `and` → `or` → `|>`. `**` kết hợp **phải**, cao hơn `*`; mũ âm với int panic; có `checked_pow`.
- **VD-17** Float: NaN — `==` trả false; float **không** có `Eq/Ord/Hash`; kiểu chứa float không derive được `Eq/Hash`; dùng `total_cmp`. `percent` so sánh được với float theo tỉ lệ (`50pct == 0.5`); `ps.cpu` có kiểu `percent`.
- **VD-18** Chuỗi: **cấm `s[i]`**; dùng `.chars() .graphemes() .bytes() .byte_at(i)?`; có `byte_len()`, `char_count()`, `grapheme_count()`; so sánh theo dãy scalar. Index: thêm `xs[i]?` trả `T?`; `xs[-1]` panic có thông điệp.
- **VD-19** Optional: tự nâng `T → T?` ở gán, trả, đối số; `T??` là lỗi kiểu; `?` dùng được trên `T?` trong hàm trả `U?`; pattern `none`, `some(x)`, `ok(v)`, `err(e)`.
- **VD-20** Lỗi: `trait Error: Show { message, source, span }`; `AnyError` là kiểu xoá; mọi `E: Error` tự chuyển sang `AnyError` qua `?`; `main -> Result[(), AnyError]`; tên phía plugin Rust là `vis::Error`. Mã thoát: 0 ok, 1 lỗi từ main, 2 lỗi dùng/kiểu, 101 panic, 126 không chạy được, 127 không tìm thấy, 130 bị huỷ.
- **VD-21** Chế độ lệnh (QD §7.2): metachar là khoảng trắng, `( ) [ ] { } ; & $ ' " |`, `#` ở đầu từ; `"..."` có nội suy, `'...'` thô; `$x` biến phiên, `$env.NAME` môi trường; `;` tách pipeline; `&` nền chỉ khi là token cuối có khoảng trắng phía trước; `--` kết thúc cờ; `-5` là số nếu tham số kiểu số, ngược lại là cờ; `.vish`/REPL: `shell_item := use | let | fn | cmd | pipeline`.
- **VD-22** Parse chế độ lệnh **hai pha**: `vis-syntax` sinh `CmdCST` (từ trần chưa định kiểu) → `cmd_lower` trong `vis-types` dùng bảng chữ ký → **HIR**. Tiêu chí "cùng AST" đọc thành **"cùng HIR"**; vòng 1 chỉ làm `CmdCST` + golden, `cmd_lower` là task vòng 2.
- **VD-23** `docs/viscript/SPEC-lex.md` + **Phụ lục A** là nguồn từ vựng/cú pháp duy nhất; Agent 0 duyệt qua errata; ST-02a DONE sau APPROVED.
- **VD-24** **Shell sở hữu tty**; tiến trình con nhận pipe, không nhận `cap[Term]`; chương trình toàn màn hình để sau với tty server.
- **VD-25** `from json` không tham số trả `Json`; `where`/`get` trên `Json` là lỗi kiểu gợi ý `from json[T]`; SIGPIPE/`take` không phải lỗi; `| complete` trả `{stdout,stderr,code,how}`.
- **VD-26** Consent và cache dùng chung **hash closure** (nguồn + phụ thuộc + phiên bản compiler).

- **VD-27** Mã lỗi `VS` + 4 chữ số: `01xx` từ vựng, `02xx` cú pháp, `03xx` tên, `04xx` kiểu, `05xx` runtime. `VS0412` giữ nghĩa "kiểu không khớp" (thiết kế §9). ST-02b được sửa **bảng mã lỗi** trong `SPEC-lex.md`/phụ lục (không sửa quy tắc từ vựng); `AGENTS.md` §4.1 cập nhật cho khớp.

- **VD-28** `b"…"` **chỉ** có escape `\xNN` (SPEC-lex §1 hàng `BYTES`, §3 dòng 54). Mọi escape khác — kể cả `\\`, `\n`, `\u{…}` — là **VS0102**; muốn một byte `\` thì viết `\x5c`. Chốt câu hỏi Q4 trong `BLOCKED.md` §3 (thiết kế §2.4 xếp `b"…"` vào nhóm đủ bộ escape — SPEC-lex đứng trước). Có test âm ở `crates/vis-syntax/src/lex/lit.rs`.
- **VD-29** `_` **trong phần lẻ** của `FLOAT` theo luật chung của §2: chỉ được **giữa hai chữ số** ⇒ `1.2_3` hợp lệ, `1.2_` và `1.2__3` là **VS0104**. (Chốt Q5: §1 viết phần lẻ là `[0-9]+`; chọn cách đọc của §2 vì nó là luật phát biểu cho literal số nói chung.)
- **VD-30** `char` **không** có escape ở vòng 1 (đúng §1 "`'` scalar `'`" và §3 chỉ định nghĩa escape cho các dạng chuỗi) ⇒ `'\n'` là VS0105 và không viết được literal dấu nháy đơn. Xem lại ở vòng 2 nếu có nhu cầu (VD mới + test trước).
- **VD-31** Phiên giải kind khi §1 và §5–§8 lệch nhau (chốt Q3): `..`/`..=` là **`PUNCT`** (golden §8 gọi `RANGE`/`RANGE_EQ` là bí danh của cùng token); `$`, `^`, `set{` là **kind riêng** `DOLLAR`/`CARET`/`SET_LBRACE` (theo §5–§6); `#` **không bao giờ** là `PUNCT` (chú thích thắng); `##` chỉ là `DOC_COMMENT` ở đầu dòng. `test` là **`IDENT`** (từ khoá theo ngữ cảnh `test fn`): parser điện phân, không phải lexer.
- **VD-32** Ba ví dụ của SPEC-lex §8 là **văn bản minh hoạ**, không phải danh sách đầy đủ: #56 (`\tif`) và #58 (`(1 +\n 2)`) **lược** token ⇒ golden ghi danh sách **đầy đủ** (`!VS0107 INDENT KW(if)`, `( INT(1) + INT(2) )`) và chép nguyên văn ví dụ gốc trong `//`; #26 (`"{x" => !VS0108`) **đúng nguyên văn**, không có errata. Không dòng nào bị bỏ, không kỳ vọng nào bị hạ.
- **VD-33** Ba phụ thuộc Unicode của `vis-syntax` được duyệt theo allowlist QD §9.3: `unicode-normalization` (MIT OR Apache-2.0 AND Unicode-3.0) cho NFC toàn tệp (VD-11), `unicode-ident` (như trên) cho UAX #31, kéo theo `tinyvec` (Zlib OR Apache-2.0 OR MIT). Hệ quả: con số "0 phụ thuộc ngoài" trong `reports/R1-viscript.*` phải ghi lại ở báo cáo vòng sau.
- **VD-34** **30 golden biểu thức** của ST-05 do **Agent 0** soạn tại `viscript/test/expr/golden.expr` (kỳ vọng, **Agent 2 không sửa**: thấy sai thì báo). Định dạng: `<nguồn> => <sexp>` / `=> !ERR` / `=> !ERR "<chuỗi gợi ý>"`; sexp là **dạng ngoặc hoá của CST** (giữ nút `paren`, postfix `field/?field/call/index/try/with`, literal cấu trúc `list/set/map/record/tuple`), kết hợp trái **viết lồng trái** để phân biệt với kết hợp phải (`??`, `**`). Nguồn kỳ vọng: §4.5 (8 hệ quả ưu tiên bắt buộc), §A.5 (12 mức + kết hợp), §A.3 (văn phạm), và ca `50%` của L3 (`docs/DUYET-AGENT0-vong-va.md`). Bất biến kèm mọi ca: **CST lossless** — ghép CST theo thứ tự phải ra nguyên văn nguồn.
- **VD-35** **Mã lỗi cú pháp (02xx)** cho parser ST-05 nằm ở **sổ riêng** trong `vis-diag` (`SYNTAX_CODES`),
  **không** sửa `CODES` — sổ `CODES` vẫn đúng bằng bảng SPEC-lex §7 (10 mã 01xx) để test "CODES khớp §7"
  tiếp tục là hàng rào thật (không nới kỳ vọng cũ). Cấp 2 mã: `VS0201` = lỗi cú pháp chung (token không
  mong đợi/thiếu vế); `VS0202` = `%` dùng như phần trăm ⇒ thông điệp phải chứa `50pct` (L3 của
  `docs/DUYET-AGENT0-vong-va.md`, ca #30 của `viscript/test/expr/golden.expr`). `lookup`/`lookup_str` tra
  **cả hai** sổ, `vis explain` in được cả 02xx; số không tái dùng, đúng 4 chữ số. Mã mới khác phải qua Agent 0.
  Kèm theo: "mỗi tệp lỗi đúng **1** lỗi đầu" = parser báo lỗi đầu tiên rồi **dừng** ở cấp tệp (không báo dồn),
  không panic với mọi chuỗi token.
- **VD-36** **Bộ fixture tài liệu của ST-05** (chốt sau khi kiểm 22 khối ```vis trong `docs/viscript/ViScript-thiet-ke.md`):
  đúng **15 khối đủ thân** phải parse 0 lỗi + lossless; **7 khối loại trừ tường minh** (liệt kê `file:dòng` +
  lý do trong test, máy kiểm đúng con số này):
  - **6 khối có `...`** (dòng 312-315, 436, 450, 613, 663, 739) — QD §7.4 **cấm `...` trong fixture**, nên đây
    là ví dụ lược thân, không phải chương trình; không được thêm `...` vào văn phạm (VD-08).
  - **1 khối dòng 1438 dùng `assert`** — `assert` **không có** trong văn phạm §A.1/A.2; đây là lỗ hổng tài liệu
    (thiết kế dùng mà văn phạm không định nghĩa) ⇒ ghi vào báo cáo R2 để Agent 0/1 vá tài liệu, **không** tự thêm
    cú pháp.
  Khối ```vish-session là pipeline chế độ lệnh ⇒ thuộc ST-06. Nếu để parse một khối **đủ thân** mà phải thêm cú
  pháp ngoài §A.1/A.2 ⇒ dừng, báo Agent 0 (không tự mở rộng ngôn ngữ).
- **VD-37** **Năm chỗ Phụ lục A không tự chứa** khi đối chiếu với chính ví dụ trong tài liệu (chốt cho ST-05,
  mỗi chỗ phải có **test khoá**; đây là mức tối thiểu để tài liệu parse được, **không** mở thêm cú pháp):
  1. `file := (item | stmt)*` — §A.1 viết `item*` nhưng 4 khối tài liệu (dòng 290, 342, 535, 549) đặt **câu lệnh**
     ở cấp tệp; `.vis` là script nên câu lệnh cấp tệp là hợp lệ.
  2. `suite` **một dòng** dừng ở `else`/`elif` **cùng dòng** (`if loud: s.upper() else: s`), không đòi NEWLINE.
  3. Giá trị trường của `with` parse ở mức **`or_expr`** (dưới `|>`), để `u with age: 21 |> f` =
     `(u with age: 21) |> f` (golden #17, §4.5), không nuốt `|>`.
  4. `+=`/`-=` (§A.2) **chưa có token** trong SPEC-lex §1 ⇒ vòng 1 nhận **cặp `+` `=` / `-` `=` dính nhau**
     (span liền kề, không khoảng trắng); `+ =` có khoảng trắng **không** phải `+=`. Vòng 2: thêm token vào §1 +
     lexer (task riêng, không tự làm).
  5. `param := '...'` (§A.1, variadic) chưa có token `...` ⇒ **không** hỗ trợ vòng 1; khối duy nhất dùng nó
     (dòng 663) đã bị loại trừ theo VD-36.
  Kèm theo: `next_block()` trả **301** sau khi 02xx được cấp (VD-35) — test cũ cập nhật kèm lý do, không nới
  kỳ vọng nào khác. Phát hiện chỗ lệch **mới** ⇒ dừng, báo Agent 0 (không tự mở rộng ngôn ngữ, VD-08).





## Phần IV — Duyệt

- Mẫu: `APPROVED <YYYY-MM-DD> Agent 0`, ghi ngay dưới mục **ST-02a** trong `viscript/agent/QUEUE.md`; nếu duyệt làm đổi nội dung SPEC thì thêm một mục VD mới ở tệp này. Không duyệt bằng chat.
- `ST-02a` = **APPROVED 2026-11-12 Agent 0** / DONE: `SPEC-lex.md` + EBNF + bảng ưu tiên + 60 golden token + 30 golden biểu thức; **chỉ Agent 0** sửa sau duyệt (D2-22, VD-08).
- Cổng "mọi khối mã trong tài liệu parse được" là điều kiện duyệt; ngoại lệ phải khai báo trong SPEC (QD §7.4).

## Phần V — Bảng nguồn cho các con số đang có hiệu lực

| Số | Giá trị | Nguồn |
|---|---|---|
| golden token (ST-02a) | **60** | QD §7.5 |
| golden biểu thức (ST-02a) | **30** | QD §7.5 |
| mã lỗi | `VS` + **4 chữ số**, quy tắc cấp (ST-02b) | QD §7.5 |
| thụt lề ra của `vis fmt` | **4** | VD-12 |
| giới hạn tệp | **1.500** dòng/tệp | QD §3.15 áp cho cả hai dòng |
| ca mơ hồ chế độ lệnh (ST-06) | **10** + mỗi hàng bảng 9.4 | QD §7.5, B §10.2 |
| fuzz lexer | **10 phút** | QD §7.5 (ST-04) |
| snapshot `vis-diag` (ST-03) | **20** ca | QD §7.5 |
