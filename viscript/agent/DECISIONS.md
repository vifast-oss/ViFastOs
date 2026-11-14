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
- **VD-38** **Sổ lỗ hổng đã biết, hoãn có ý thức** (ghi để không ai "phát hiện lại" rồi tưởng là hồi quy; mỗi mục
  phải vào báo cáo **R2** khi làm báo cáo vòng 2):
  1. pattern chưa nhận `dotted_path` (`Color.Red`) — §A.4; 2. `lvalue` chưa nhận postfix cho `+=` (`a.b += 1`);
  3. `weak`/`cap` **chưa** là từ khoá ở SPEC-lex §1 nên `cap[Dir]` parse thành type-application (ảnh hưởng ST-06);
  4. tài liệu dùng `Entry(trường: …)` (dòng 1429) và `assert` (dòng 1438) — **không** có trong §A.1/§A.2;
  5. 6 khối tài liệu có `...` (dòng 312-315/436/450/613/663/739) — QD §7.4 cấm `...` trong fixture;
  6. §A.1 có `param := '...'` (variadic) nhưng SPEC-lex §1 **không có** token `...`;
  7. `+=`/`-=` có ở §A.2 nhưng **không có token** ở §1 (vòng 1 nhận cặp dính nhau, VD-37);
  8. "mỗi tệp đúng **1** lỗi đầu" là quy tắc **tầng parser**: tệp có lỗi từ vựng vẫn ra 2 chẩn đoán
     (`VS0101` + `VS0201`) — lọc theo tầng là việc của `vis check` (ST-08).
  Không mục nào chặn ST-06/07/08. Muốn vá ⇒ task riêng + test đỏ trước (VD-08).
- **VD-39** **Phạm vi `vis fmt` vòng 1** (ST-07). Chỉ chuẩn hoá **thụt lề**, không định dạng lại trong dòng:
  mỗi cấp khối = đúng **4 dấu cách** (suy từ cấu trúc CST, không đếm theo tệp gốc); **tab → 4 dấu cách**
  (VS0107); bỏ **khoảng trắng cuối dòng**; bảo đảm **đúng một** newline cuối tệp; **giữ nguyên văn mọi thứ
  trong dòng** (khoảng trắng giữa token, chú thích, thứ tự token); ghi ra **NFC** (vì `LexOutput.text` đã NFC).
  Hệ quả là 4 tiêu chí §18.4 đo được: (a) **lossless** trên tệp "đã đúng chuẩn" — định nghĩa: tệp là **điểm
  bất động** của `fmt`; (b) **idempotent**; (c) sửa tab; (d) tệp **lỗi cú pháp**: không sửa, báo lỗi, mã thoát
  **2**. Định dạng lại biểu thức/danh sách/ngắt dòng dài là **vòng 2** (không làm ở ST-07).
- **VD-40** **Chế độ lệnh vòng 1 (ST-06) — 5 chỗ §A.7/§9.3 cần chốt** (kèm 22 golden ở
  `viscript/test/cmd/golden.cmd`, Agent 0 duyệt trước khi hiện thực):
  1. **`;` và `&`**: §A.7 không có luật sinh, nhưng §9.3 ("`;` tách pipeline") và §9.4 hàng 10 quy định
     nghĩa. Chốt: nội dung **một dòng** là `(items <pipeline> …)`; `&` là **thuộc tính của pipeline**
     (`pipeline := stage ('|' stage)* ['&']`) ⇒ nút `(bg (pipe …))`; `&` **không** ở cuối dòng = lỗi;
     `;` không có vế sau = lỗi. **Tệp** `.vish` = `(shell <item>*)`, mỗi item một dòng.
  2. **`-5`**: §A.7 `flag := '--' IDENT ['=' expr] | '-' IDENT ['=' expr] | '--'` **không** cho chữ số sau
     `-`; §9.3 nói `-5` là số nếu kiểu tham số là số (quyết ở `cmd_lower`). Chốt: parser giữ **`(word -5)`**
     (từ trần chưa định kiểu), **không** nới `flag`; `cmd_lower` (vòng 2, VD-22) phân biệt số/cờ.
  3. **`[ab]` glob char-class**: `[` `]` là **metachar** ở §9.3 nên lexer cắt thành 3 token (ST-04 đã DONE,
     **không** sửa lexer). Chốt: parser **ghép** cụm `[` … `]` **liền kề** (span liền, không khoảng trắng)
     ở vị trí từ thành **một** nút `(word [ab])`; có khoảng trắng bên trong = lỗi; list literal chỉ trong
     `( … )` (chế độ biểu thức).
  4. **`--`**: theo §A.7 `'--'` là một dạng `flag` ⇒ nút `(flag --)`; `cmd_lower` phải hiểu đây là **kết thúc
     cờ** (mọi token sau là từ trần).
  5. **`|>` ở chế độ lệnh**: không phải pipe (§9.4 hàng 2 ghi "—") ⇒ **lỗi**; pipe chỉ là `|` ở độ sâu 0.
  Không chỗ nào ở trên mở rộng ngôn ngữ ngoài §9.3/§9.4; `cmd_lower`/`vis-types` vẫn là vòng 2 (VD-22).
- **VD-41** **Đính chính 3 ca của bộ golden chế độ lệnh** (sau khi hiện thực pha B va vào 2 khối
  ```vish-session của tài liệu — DoD của ST-06):
  1. **`>` `<` `>=` `<=` `==` `!=` ngoài `( … )` KHÔNG phải lỗi và KHÔNG phải redirect**: §9.4 hàng 3 viết
     "so sánh trong tham số biểu thức; **không** redirect" — nghĩa là `>` **không bao giờ** là redirect, và
     2 khối tài liệu dùng `where size > 5mb`, `where age >= 18` **không ngoặc**. Chốt: CmdCST chưa định kiểu
     giữ chúng là nút từ `(word >)`; `cmd_lower` (vòng 2) quyết nghĩa theo chữ ký lệnh; ghi tệp **chỉ** bằng
     `| save`. Ca #13 đổi từ `!ERR` thành ca hợp lệ. (Bài học: `>` redirect kiểu POSIX **không** có trong ngôn ngữ.)
  2. **`^` không ở đầu stage = LỖI**: lexer ST-04 đã bắt bằng `VS0110` (§5/§6) ⇒ ca #22 `ls ^x` là `!ERR`, parser
     không có cơ hội thấy token `^` ở giữa. Không sửa lexer.
  3. **Thêm ca #23** (tổng **23**): `.vish` nhiều dòng `let out = ls | grep foo` + `pwd` ⇒
     `(shell (let out (pipe …)) (pipe …))`, khoá nút `(shell …)` và `(let …)` (VD-40 #1) — vì `vis check` nay
     kiểm được `.vish` và 2 khối ```vish-session là DoD của ST-06.
  Kèm theo (đã ghi `JOURNAL`): `tests/check.rs` của ST-08 đổi ca `.vish` từ `Bail out!` sang "kiểm như tệp lệnh"
  — **có lý do**, không nới kỳ vọng; log đỏ ST-08 giữ nguyên sha256 của harness **thời điểm đó** (là bản ghi lịch
  sử, không phải bằng chứng cho cây hiện tại).
- **VD-42** **Sổ mã lỗi vòng 2** (theo VD-27: `03xx` tên, `04xx` kiểu, `05xx` runtime) — vẫn là **sổ riêng** trong
  `vis-diag` (`NAME_CODES`, `TYPE_CODES`, `RUNTIME_CODES`), **không** sửa `CODES`/`SYNTAX_CODES`. Cấp tối thiểu:
  `VS0301` tên không tồn tại · `VS0302` khai báo trùng; `VS0412` **kiểu không khớp** (giữ đúng nghĩa §9) ·
  `VS0401` sai số/thứ tự đối số · `VS0402` `match` không xét hết · `VS0403` trường/kiểu không tồn tại;
  `VS0501` lỗi runtime chung · `VS0502` chia cho 0 · `VS0503` chỉ số ngoài phạm vi. `lookup`/`lookup_str`/
  `vis explain` tra **cả 5 sổ**; `next_block()` ⇒ **601**; mỗi mã mới phải có `message_vi/en/ja` + snapshot.
- **VD-43** **Phạm vi ngữ nghĩa VS0** (chốt để ST-11..ST-15 có hợp đồng; thiết kế §3–§6 là nguồn, chỗ nào
  thiết kế không nói rõ thì **hỏi Agent 0**, không tự chọn):
  1. **Kiểu**: `int` (i64 có dấu), `size`, `duration`, `percent`, `str`, `bool`, `bytes`, `path`, `char`,
     `list[T]`, `map[K,V]`, bản ghi có tên, enum (biến thể có/không đối số), `T?` (Option), `Result[T,E]`,
     hàm `(A, B) -> R`, `unit`. **Không generic** (VS1).
  2. **Số học**: `+ -` chỉ **cùng kiểu/đơn vị** (`size + duration` = `VS0412`); `size * int`, `size / int`,
     `duration * int`, `duration / int`; `percent` chỉ nhân với `size`/`duration` (`size * percent` ⇒ `size`);
     trộn `int` với đơn vị = `VS0412`; chia/lấy dư cho 0 = `VS0502`; **tràn i64 = `VS0501`** (không wrap).
     Toán tử số học áp lên `int` là `+ - * / % **`; `**` mũ âm trên `int` = `VS0501`.
  3. **So sánh**: chỉ cùng kiểu (số có thể cùng đơn vị); `==`/`!=` cho mọi kiểu **trừ hàm**. Không so sánh
     chuỗi với số.
  4. **Logic**: `and`/`or` ngắn mạch, hai vế `bool`; `not` nhận `bool`; `??` trả kiểu **bỏ Option** của vế trái,
     vế phải cùng kiểu đó.
  5. **`match`**: phải xét hết ⇒ `VS0402` nếu thiếu; được `else`.
  6. **Bản ghi**: bất biến, tạo `{ten: …}`, cập nhật bằng `with`; trường lạ = `VS0403`.
  7. **`print(x)`** in `str(x)` + `\n` ra **stdout**; `str` của list/map/bản ghi/enum/Option/Result theo **một**
     định dạng cố định (ghi vào `JOURNAL` + snapshot để ST-15 dùng).
  8. **Lỗi runtime**: in `<tệp>:<dòng>:<cột>: lỗi VS05xx: …` ra **stderr**, exit **1**; lỗi kiểu/tên ⇒ exit **2**
     (như `vis check`).
- **VD-44** **Hạ CST→HIR phải PERMISSIVE** (ST-11): HIR biểu diễn **mọi** thứ trong Phụ lục A ở mức cú pháp —
  `float`, `cap[Dir]`, `ref`, `async fn`, generic `fn f[T]`, `derives`, `scope s`, `cmd { … }`, `await` — **không**
  báo "tính năng chưa hỗ trợ" ở tầng hạ; giới hạn VS0 chỉ báo ở **kiểm kiểu (ST-12)**. Hệ quả: 15 khối tài liệu
  hạ **0 chẩn đoán**. `Mode::Command` vào `lower` ⇒ lỗi **API** tường minh (VD-22), không panic; không
  `unwrap/todo!` trên dữ liệu người dùng. ST-11 **không** dựng prelude: chỉ 8 tên giá trị builtin (`print`, `len`,
  `str`, `Ok`, `Err`, `Some`, `None`, `ref`) khoá bằng test; `true`/`false` là literal.
- **VD-45** **Chính sách giải tên** (ST-11): `NamePolicy::Permissive` là **mặc định** — dùng tên tự do được phép
  (fragment như 30 golden, khối `match s:`), `VS0301` **chỉ** khi **gán** cho tên chưa khai (báo ở **gốc lvalue**,
  đúng **một** lần cho cả hai policy); `VS0302` khi khai lại **cùng** khung. `NamePolicy::Strict` (mọi lần dùng
  phải phân giải) có test riêng và sẽ là mặc định từ **ST-13/14** cho chương trình đầy đủ. Tên kiểu **đã khai
  trong tệp** ⇒ `Record/Enum`; **chưa khai** ⇒ `TypeRef` (ST-12 phân giải + báo `VS0301` cho kiểu lạ). `HirOutput`
  có 5 trường (`hir, diagnostics, symbols, types, error`). Thứ tự hạ `let`: **kiểu → init → khai `pat`** (§4.1).
- **VD-46** **Chốt 3 câu hỏi của ST-12** (kiểm kiểu VS0):
  1. **Gán cho `let`/tham số** (§4.1 cấm) dùng **`VS0412`** + thông điệp "chỉ `var` mới gán được" — đây là lỗi
     bất biến/kiểu, **không** cấp mã `04xx` mới và **không** mượn `05xx`; nếu sau này cần mã riêng thì cấp sau
     (VD mới).
  2. **Kiểu tham số lambda** để `{error}` ở VS0: văn phạm §A.3 **không** cho `(x: int) =>` nên parser không mở;
     **suy kiểu lambda hai chiều (§3.4) hoãn** ⇒ ghi "lỗ hổng đã biết" cho ST-14/R2.
  3. **Pattern bản ghi trần** `{name, age}` mất `sym` trong HIR ⇒ chỉ dùng được cho xét hết ở VS0; thêm `sym` là
     việc của **ST-14** (hoặc task riêng), không làm trong ST-12.
  Kèm: chẩn đoán của tầng tyck có `line/col = 0` (chỉ có HIR, không có văn bản) — **chấp nhận**, nhưng **ST-14
  bắt buộc** in `line`/`col` thật từ `span` + văn bản nguồn; sửa 2 test cũ (`snapshot_6_ca_so_ten_03xx`,
  `explain_so_ten_03xx`) là **bắt buộc** vì VD-42 đã cấp sổ `04xx` (không phải nới kỳ vọng).
- **VD-47** **Ngữ nghĩa VS0 chốt ở ST-13**: (1) **khối cho giá trị của câu lệnh cuối** — `Expr`, `If`, `Match`, khối
  con đều lấy giá trị; `if` **không** `else` ⇒ `unit`; vòng lặp **không** rò giá trị lần lặp cuối; tyck theo **cùng**
  luật (§4, §6). (2) `Show`: `size`/`duration` in **đơn vị lớn nhất chia hết** (`5000`⇒`5kb`, `1e9 ns`⇒`1s`, không
  chia hết ⇒ gốc); `percent` = `ratio*100` + `pct` bỏ `.0`; `record` ⇒ `Ten(truong: gt)`, `enum` ⇒ `Bien(arg)`,
  `opt` ⇒ `some(x)`/`none`, `Result` ⇒ `Ok/Err`, `char` ⇒ `'a'`, `bytes` ⇒ `b"…"`, `fn` ⇒ `<fn …>` (khoá snapshot).
  (3) `percent * size|int` và `percent * duration` **cắt về 0** (quyết định, không phải mặc định ngầm). (4) **Bảng
  phương thức lõi** khoá bằng test tyck (tên lạ ⇒ `VS0403`): `list` `len get first last push map filter each sum
  join`; `str` `len upper lower split chars contains`; `map` `len get has keys values`. (5) **Nội suy `{expr}` trong
  `MULTI_STRING` hoãn tới ST-14** (HIR phải mang phần nội suy; hiện in nguyên văn).
- **VD-48** **Phạm vi kiểu VS0 (chốt sau review ST-12)**: `AnyError`, `cap`, `ref`, `Json` **không** thuộc VS0 —
  vẫn báo `VS0301` nhưng **thông điệp phải nêu rõ** "không có ở VS0 (thuộc vis-std/vòng 2)". Toán tử VS0:
  `size % int`/`duration % int` ⇒ **giữ đơn vị**; `percent / int` ⇒ `percent`; `percent / percent` ⇒ **`float`**;
  `percent + percent` ⇒ `percent`; còn lại như VD-43. Luật `?` (§4.4): `T?` chỉ khi hàm trả `U?`, `Result[T,E]` chỉ
  khi hàm trả `Result[U,F]` (ràng buộc `E: Error` bỏ qua ở VS0, ghi JOURNAL). `int→float` **chỉ** cho **literal**
  số nguyên (§3.5). `--` chỉ dùng cho tham số **khai `--`** (`VS0401` nếu sai, tên trùng cũng `VS0401`).
  `unit` là kiểu **hữu hạn** khi xét hết `match` (pattern `()`); tuple theo §4.3 nếu mở.
  **Bổ sung (sau ST-12b):** so sánh `percent` ↔ `float` là **hợp lệ về kiểu** (`50pct == 0.6` ⇒ `false` lúc chạy,
  **không** `VS0412` — chỉ khác **giá trị**, không phải lỗi kiểu); ràng buộc `E: Error` của `?` trên `Result`
  **bỏ qua ở VS0** (làm khi có `vis-std`); 4 test cũ đã sửa kèm lý do (3 do VD-42 + 1 kỳ vọng `?` sai ở ST-12).
- **VD-49** **Chốt sau review ST-13** (ST-13b): (1) thêm `Type::Never` — `return`, nhánh `if`/`match` toàn phân kỳ
  cho `Never`, `coerce`/`unify_branch` bỏ qua nó. (2) `..=` với `i64::MAX`: **dừng dải** (không panic, không lỗi);
  dùng `checked_add`. (3) **thân vòng lặp bỏ giá trị** câu lệnh cuối (`for i in 0..1: 1` hợp lệ); `scope` **cho
  giá trị khối** (thống nhất tyck/interp). (4) `for c in "str"` ⇒ `char`; **range ở VS0 chỉ `int`** (`size/duration/
  char` ⇒ `VS0412`). (5) `DUR_UNITS` có **`w`**; `Show` escape ký tự điều khiển; đơn vị **decimal trước binary**
  (`1024kb`). (6) **keyword được dùng sau `.`** (`m.has(..)`, `xs.len()`), sửa ở parser — không đổi tên phương thức.
  (7) `break`/`continue` ngoài vòng lặp, `?` ngoài hàm ⇒ **`VS0501`** (mức runtime).
- **VD-50** **Chốt ST-14**: nội suy `{expr}` nằm ở **HIR** (`HExpr::Interp(Vec<InterpPart>)`), hạ **trong cùng
  ngữ cảnh giải tên** — **không** parse lại lúc chạy; `{{`/`}}` là literal. `vis run` exit **0/1/2** (0 xong,
  1 lỗi runtime `VS05xx`, 2 lỗi cú pháp/tên/kiểu); `--tap` in TAP ra **stderr** (giữ stdout sạch); `.vish` ⇒
  `Bail out!` (chế độ lệnh là VS1). Harness `test/sem/**` chạy **qua binary** `vis run`, so **byte** + `steps > 0`,
  và **khoá cứng số ca + số nhóm** (thêm/bớt ca phải sửa cả khoá — chống âm thầm bỏ ca).














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

- **VD-51** **Closure chụp theo snapshot (VS0)** — ST-15b chọn: `HExpr::Lambda` chụp **bản sao** các biến đang sống
  lúc tạo; biến `var` cập nhật **sau** khi tạo closure **không** được closure thấy (nhất quán với ngữ nghĩa giá trị
  của §7.1). Ngữ nghĩa **tham chiếu** (closure thấy giá trị mới) cần env chuỗi frame ⇒ task riêng, đã ghi JOURNAL.
  Kèm: `push` trên `var` **ghi lại lvalue** (CoW); trên `let`/tạm trả list mới (không no-op im lặng). `{x, ..}` hợp
  lệ (§4.3), thiếu trường **không** có `..` ⇒ `VS0402`. Sửa **1 kỳ vọng cũ** `tests/exhaustive.rs` (ca `{name, ..}`
  đang khoá hành vi **trái §4.3**) — ca thứ **5** bị sửa trong dòng ViScript, có comment lý do + ca âm.

- **VD-52** **Phạm vi vòng 3 (khung VS1)** — chốt trước khi làm: (1) **bytecode** tự chứa, có **magic `VBC1`** +
  phiên bản u16 + hash nguồn; `vis-bytecode` **không** phụ thuộc OS, **không** `unsafe`; (2) **VM an toàn** (Rust
  safe, không `unsafe` ở vòng này — thiết kế cho phép `unsafe` trong `vis-vm` nhưng **chưa cần**), stack + frame
  tường minh, giới hạn độ sâu như interp (VD-49: 2.000); (3) **verifier chạy cả bytecode từ cache** (thiết kế
  §14.5), từ chối: magic/phiên bản sai, đích nhảy ngoài, thanh ghi/frame sai, hàm thiếu, hằng số lệch; (4) **cache**
  ở `<dir>/vis/<hash>.vbc`, hash = **hash closure** (nguồn + phụ thuộc + phiên bản compiler) — nạp **luôn** qua
  verifier; (5) **differential** phải so **stdout + mã thoát + mã chẩn đoán**, sinh chương trình **có seed** trong
  subset VS0 (không sinh tính năng VS1b); (6) **mặc định vẫn chạy interp**, `vis run --vm` chọn VM cho tới khi
  differential 100.000 ca xanh (khi đó mới bàn đổi mặc định); (7) **không** làm generic/trait/module/`vis-std`/
  `vis-host-posix` ở vòng này — ghi rõ ở R3 là **VS1b**.

- **VD-53** **Chế độ lệnh chạy được (ST-18)**: (1) **khe ống** = tham số **vị trí đầu tiên theo thứ tự khai báo** của
  stage sau khớp **kiểu ra** của stage trước **và** không được ghi tường minh; khe **giữ chỗ trước** (đối số tường
  minh không chiếm mất); không có khe ⇒ `VS0603`. (2) **fail-fast**: lỗi giữa pipeline dừng, không chạy stage sau.
  (3) `;` ⇒ items tuần tự, `&` cuối dòng / `$x` / `$env` / `^prog` ⇒ **`VS0604`** "chưa hỗ trợ ở VS0" (không dùng
  `VS0602`). (4) **Sổ mã thứ sáu `COMMAND_CODES`**: `VS0601` lệnh không tồn tại · `VS0602` sai đối số/kiểu ·
  `VS0603` pipeline không nối được · `VS0604` tính năng chưa hỗ trợ ở VS0 ⇒ `BOOKS` 6, `all_codes()` **25**,
  `next_block()` **701**. (5) **3 builtin**: `echo(a, b="", c="", --upper:bool=false)`, `lines(s)`, `take(xs, n)`;
  **không variadic** (`echo` ≤ 3 phần) và **chưa** có `--x=v` — hạn chế đã ghi sổ. (6) **Tên lạ KHÔNG** bị coi là
  "chương trình ngoài" ở vòng này (giữ `VS0601`) để `vis check` còn bắt lỗi gõ; `command := … | chuong_trinh_ngoai`
  (§9.3) là việc của **VS1b** cùng `vis-host-posix`. (7) Hai khối `vish-session` §9.7 (dùng `ls/where/sort_by/take`,
  `walk/ps/to json/save`) nằm trong `doc-examples-allow.txt` (**9 mục**) vì là lệnh shell/vis-std VS1.

- **VD-54** **Bytecode VBC1 (ST-19)**: (1) hash nguồn 32 byte = **FNV-1a×4** (không thêm dep) — **không** phải SHA-256;
  doc ghi rõ "chỉ là khoá cache"; **khi làm consent/quyền (§11.3) phải thay bằng SHA-256 thật** (`sha2`, cần duyệt
  allowlist); hàng rào bù: **mọi** bytecode nạp từ cache **luôn** qua verifier (VD-52 #3). (2) `max_stack` chỉ là
  **ước lượng trên** (mô phỏng tuần tự, kẹp 0) — **ST-21 kiểm lại độc lập từ byte stream** và có quyền từ chối.
  (3) `match` hạ thành **chuỗi test + nhảy**; VM **không** biết pattern. (4) `Bin`/`Un` mang **enum của `vis-types`**;
  VM phải **gọi chung** bảng ngữ nghĩa của `vis-interp` (không chép ngữ nghĩa toán tử/`Show`/đơn vị vào bytecode).
  (5) `ref`/`await`/`cmd {}` (VS1) ⇒ `CodegenError::Unsupported` nêu tên + span, **không** cắt ca.

- **VD-55** **VM dùng chung ngữ nghĩa (ST-20/20b)**: (1) `vis-vm` **không** chép ngữ nghĩa — gọi `vis-interp`
  (`ops::{bin,un}`, `builtin::{call,method}`, `value::{show,eq,cmp,iter_items,lit_value,exit_code}`); `Interp` chỉ còn
  là vỏ mỏng. (2) VM dùng **`Vec<Value>` động**, **không** lấy `max_stack` làm giới hạn; `StackUnderflow` ⇒
  `VS0501` "bytecode hỏng"; cap khung **2.000** + cap **10^7 lệnh** như interp. (3) Differential so **`stdout` +
  `exit` + `(mã,dòng,cột)`**, **không** so `steps` (interp đếm bước ngữ nghĩa, VM đếm lệnh — ghi rõ trong doc).
  (4) **19 ca lệch đầu tiên** được khoá trong `KNOWN_LECH` (hai chiều: lệch mới ⇒ đỏ, sửa mà không cập nhật ⇒ đỏ);
  ST-20b đưa về **0 lệch** và để `KNOWN_LECH` **rỗng** — giữ test khoá "rỗng" để không ai thêm lại. (5) Codegen
  được sửa **theo differential** (bind pattern + thứ tự bind→guard, `MakeEnum` cho `E.A`, closure tham số trước
  biến bắt, `CallValue`, `?.` phẳng, `T?` tự nâng) — đây là **hoàn thiện codegen**, không đổi ngữ nghĩa; VBC1 vẫn
  **version 1** vì chưa ghi cache.

- **VD-56** **Chốt VS1-khung (ST-21..ST-23)**: (1) verifier **kiểm lại từ byte stream** và codegen dùng **chính**
  hàm mô phỏng ⇒ `max_stack` khai **bằng** `need`; 13 `VerifyError`; `load_verified` là **cửa duy nhất** để có
  `Module` từ byte (cache cũng đi qua đây). (2) Differential: **cổng `--fast` chạy 1.000** chương trình sinh tự
  động (~1,3 s); **100.000** do `bin/vis-diff` và **ghi log thật** (`st22-diff-100k.log`) — **không** tuyên bố cổng
  chạy 100k. (3) Cache: `$XDG_CACHE_HOME/vifast/vis/<hash>.vbc`, khoá FNV-1a×4 trên (phiên bản compiler, VBC1,
  chế độ, **toàn văn nguồn**), ghi tệp tạm + `rename`, hit thì **bỏ qua frontend**; hỏng/lệch ⇒ biên dịch lại.
  (4) **`vis run` mặc định vẫn INTERP**, cache áp cho đường `--vm` — đổi mặc định sang VM **chưa** làm vì sẽ đổi
  `steps` trong TAP (đơn vị lệnh vs bước ngữ nghĩa); đó là quyết định của vòng sau, có đo lại.
  (5) **Chưa** có generic/trait/module/`vis-std`/`vis-host-posix` ⇒ **VS1b** (ghi rõ trong R3).

- **VD-57** **Phạm vi vòng 4 (VS1b)**: (1) **module** = **mỗi tệp một module** (§7.10); `use ./util` **đường dẫn tương
  đối**, `use std.x` cho thư viện chuẩn, `use pkg.x` **hoãn** (cần registry/`deps`, VS2); **cấm vòng** phụ thuộc;
  mã lỗi khối **07xx**: `VS0701` không tìm thấy module · `VS0702` vòng phụ thuộc · `VS0703` `use` trùng/khoá mơ hồ;
  `next_block()` ⇒ **801**; hash closure (**VD-54**) **gồm mọi phụ thuộc** (sửa file `./util` ⇒ cache mất hiệu lực).
  (2) **`vis-std`** ở vòng này = **prelude trong Rust** (`std.str`, `std.list`, `std.map`, `std.path`, `std.time` mức
  tối thiểu) + `use std.x` phân giải tĩnh; **chưa** có gói ngoài. (3) **`vis-host-posix`**: tên lệnh **lạ** trong
  `.vish` = **chương trình ngoài** (§9.3) ⇒ `VS0601` **chỉ còn** khi `vis check --no-extern`; chương trình ngoài
  chạy thật, **stdout dạng `str`/`list[str]`**, mã thoát, **quyền**: mặc định chạy được nhưng ghi vào nhật ký quyền
  (`vis run --explain-perms`), **consent thật** ⇒ VS2. (4) **generic**: hàm + kiểu `[T]`, **đơn hình hoá**
  (monomorphize) ở codegen — chưa có constraint; **trait** ở ST-29 chỉ `derives Eq/Show/Json` + `impl` tối thiểu.
  (5) **SHA-256** thay FNV cho hash closure/consent khi `sha2` được duyệt allowlist (VD-54 điều kiện (1)).
  (6) **`vis test [đường dẫn]`** chạy bộ `test/sem/**` và in TAP, dùng **cùng** harness đã khoá số ca.

- **VD-58** **Module (ST-25)**: (1) **mỗi tệp một module**; `use ./x` tương đối (thử `.vish` trước nếu tệp chính là
  `.vish`), `./x` và `./x.vis` là **một** module; `..` cho phép nhưng `canonicalize`; `use std.*` ⇒ `VS0701` (ST-26),
  `use pkg.*` ⇒ `VS0701` (VS2). (2) **Cấm vòng**; DFS **hậu duyệt**, mỗi tệp một lần, tệp chính cuối ⇒ item hợp nhất
  tất định. (3) **Tầm nhìn**: chỉ item **`pub`** mới dùng được từ module khác **và** tệp phải `use` module đó — kiểm
  ở **giải tên (`lower`)**, không ở `tyck`; sai ⇒ `VS0301` nêu tên module. `pub` cho `let`/`const` **chưa** có (nợ).
  (4) `lower_into` **append** vào bảng chung ⇒ `SymbolId`/`RecId`/`EnumId` **ổn định** qua các tệp, không remap.
  (5) Sổ `07xx`: `VS0701` không thấy module · `VS0702` vòng · `VS0703` `use` trùng/mơ hồ ⇒ `BOOKS` 7,
  `all_codes()` 28, `next_block()` **801**. (6) Hash closure (VD-54) = FNV-1a×4 trên `"vifast-vis-cache-v2\n"` +
  compiler + VBC1 + mode + **Σ(đường dẫn tương đối + nội dung)** theo hậu duyệt ⇒ sửa phụ thuộc là **mất cache**.

- **VD-59** **vis-std lõi (ST-26)**: (1) tên **đầy đủ theo module** (`str.split`, `path.join`, `time.now_ms`,
  `json.encode`) — không rò tên trần, không đụng `join`/`take` của VS0. (2) **Một hiện thực** ở `vis-interp`
  (`stdlib.rs`), `builtin::call` chuyển tiếp, **VM gọi cùng đường** (VD-55) ⇒ differential 70/70 là bằng chứng.
  (3) `std.list.*`/`std.map.*` nhận **`Any`** (tyck chỉ kiểm số đối số; sai kiểu ⇒ `VS0501` nêu hàm + kiểu nhận) —
  **nợ VS1c**: tổng quát hoá `list[T]`/`map[K,V]` khi có generic. (4) `sort_by` = **closure khoá**, tăng dần,
  **ổn định**; `--desc` = nợ. (5) `json.encode`/`encode_pretty` (map theo **thứ tự chèn**, bản ghi theo thứ tự
  trường, `none` ⇒ `null`, `Result` ⇒ `{"ok":…}`/`{"err":…}`, enum ⇒ `{"variant":…,"value":…}`); `json.decode`
  **không** export, gọi ⇒ `VS0301` **"chưa hỗ trợ (VS1b)"** (bảng `HA_CHUA_HO_TRO`, **không** cấp sổ mới).
  (6) `std.time.*` đọc đồng hồ trong Rust (không thêm `Host`), `std.path.*` **thuần chuỗi**, `std.fs` ⇒ ST-27.

- **VD-60** **Chương trình ngoài (ST-27)**: (1) tên lệnh **lạ** trong `.vish` = **chương trình ngoài** (§9.3); chạy
  bằng `std::process::Command` (**không** shell, đối số mảng, env kế thừa, **cwd** hiện tại); `stdout` ⇒ `list[str]`
  (tách `\n`, bỏ dòng rỗng cuối) hoặc `--raw` ⇒ `str` nguyên văn; `stderr` ⇒ **chẩn đoán**, không bao giờ trộn
  stdout; **`exit != 0` ⇒ `VS0501`** "thoát mã N" (§9.4) — hệ quả đã ghi: `grep` không khớp (exit 1) hiện **là lỗi**
  cho tới khi có `| complete` (VS1b). (2) **Ranh giới OS**: `vis-interp` **không** phụ thuộc OS — chỉ có
  `trait Ngoai` + `Interp.ngoai`; **không** host ⇒ `VS0604`; `vis-cli` cài runner. (3) `vis check` mặc định **cho**
  ngoài, `--no-extern` ⇒ `VS0601`. (4) **variadic `...rest: str`** gom mọi đối số vị trí còn lại thành `list[str]`;
  đối số **sau** variadic ⇒ `VS0601`; **`--x=v`** không lấy khoảng trắng. (5) timeout **10 s**/lần (kill rồi
  `VS0501`), chạy **tuần tự** (cap 1), `--explain-perms` in `EXEC …` ra stderr; consent thật + `cap[Proc]` ⇒ **VS2**.
  (6) Allowlist `doc-examples` **9 → 8**: khối 662 (`extern cmd … ...rest`) **hợp lệ** — chặt hơn, không nới.

- **VD-61** **Generic VS1b (ST-28)**: (1) **xoá kiểu (erase) + hợp nhất ở call-site**, **không** đơn hình hoá —
  để interp/VM/bytecode có **một** hiện thực (differential 0 lệch theo cấu trúc). (2) `FnSig` giữ **kiểu nguyên bản**
  (`raw_params`/`raw_ret` có `TypeRef{"T"}`) để hợp nhất; **thân hàm** dùng kiểu đã xoá (`Type::Error`) ⇒ không
  `VS0301` oan; call-site suy `T` đệ quy qua `list/map/opt/Result`, mâu thuẫn ⇒ **`VS0412`** nêu `T` + hai kiểu;
  kiểu trả thay `T` ⇒ `let n: int = lay("x")` vẫn `VS0412`. (3) Record/enum generic: `Hop[int]` ≡ `Hop[str]`
  (chưa có instantiation thật — **nợ VS1b**), payload đọc ra đúng theo giá trị. (4) Phép toán trên `T` **kiểm động**
  lúc chạy, sai ⇒ **mã hiện có** của toán tử (không bịa mã); constraint/trait ⇒ ST-29. (5) **`vis check` phải báo
  cùng chẩn đoán đầu như `vis run`** (không lọc theo lô); lọc `01xx/02xx/07xx` chỉ còn ở **`--syntax`** — dùng cho
  cổng "khối mã tài liệu **parse được**" (QD §7.4). (6) **VM entry**: tệp có `fn main` ⇒ entry phát `Call(main, 0)`
  (trước đó VM chạy thân cấp tệp và bỏ qua `main` — bug thật, đã khoá bằng test).

- **VD-62** **Trait/impl VS1b (ST-29)**: (1) **dispatch TĨNH ở tầng hạ** — `x.m(…)` với `x` có kiểu **tĩnh**
  `Record`/`Enum` (biến/tham số/trường/`let` có chú thích) và `m` **không** nội tại ⇒ hạ thẳng `Call(impl_fn)` +
  chèn `self` **theo giá trị** đầu tiên ⇒ interp và VM dùng **cùng HIR** (không bảng `impls`, không bump VBC1).
  (2) Thứ tự phân giải: **nội tại** (list/str/map/`std.*`) → **impl tĩnh** → `VS0403`; kiểu nhận **không tĩnh**
  (generic `T`, `{error}`, `Any`) ⇒ `VS0403` "không phân giải được … (VS1b — thêm chú thích kiểu)".
  (3) `VS0403` **mở rộng nghĩa** thành "trường, kiểu hoặc **phương thức** không tồn tại" và dùng cho: không có
  phương thức, **mơ hồ** (≥2 impl cùng tên), impl **thiếu/thừa** so với trait, trait lạ, `derives` lạ, `Json` trên
  kiểu chứa closure — **không** cấp `VS0404`; `TYPE_CODES` vẫn 4, `all_codes()` 28. (4) `derives Eq/Show/Json`
  **dùng ngữ nghĩa có sẵn** (so sánh cấu trúc, `show`, `json.encode`) — **chưa** sinh phương thức riêng (nợ VS1b).
  (5) `self` **theo giá trị**; `&self`/`&mut self` = **nợ VS1**. (6) Mồ côi (§5.4) **mới kiểm trong một tệp**;
  siêu trait chưa kiểm — nợ VS1b.

- **VD-63** **`vis test` + SHA-256 + đồng bộ chẩn đoán (ST-30/30b)**: (1) **SHA-256 thật** (`sha2 0.10`, MIT OR
  Apache-2.0 — duyệt allowlist theo QD §9.3) **thay** FNV-1a×4 cho **hash nguồn `VBC1`** và **`cache::khoa_multi`** —
  gỡ nợ VD-54 (1); vẫn 32 byte, **không** bump version; cache cũ tự miss; nạp **luôn** qua `load_verified`.
  (2) `vis test [--vm] [thư mục|tệp]` in **TAP** (`1..N`, `ok/not ok`, `# <tệp>:<dòng>:<cột>: <mã> <thông điệp>`),
  exit **0/1/2**; `--vm` **so đúng mã** như interp, ca cần **host chương trình ngoài** ⇒ **`ok … # SKIP`** đúng TAP
  (không giả xanh). (3) **Hai đường phải cho CÙNG danh sách chẩn đoán** — differential so **đủ** `(mã, dòng, cột,
  thông điệp)`, không chỉ phần tử đầu (lỗ hổng cũ: bỏ 100% ca `.err`); sửa **bên sai** (VM `index` ⇒ `VS0503` +
  câu interp, thêm `Bytes`, thông điệp đệ quy sâu; interp "tên …") ⇒ **312/312 khớp, 0 lệch**; ca khoá
  `hai_duong_cung_so_chan_doan`. (4) **26 ca bỏ qua** = ca **lỗi biên dịch** (hai đường chung front end) — muốn so
  chúng cần harness riêng cho chẩn đoán biên dịch (**nợ**).

- **VD-64** **Phạm vi vòng 5 (VS1c — đóng nợ `R4` §5)**: (1) **harness chẩn đoán biên dịch**: so **đủ**
  `(mã, dòng, cột, thông điệp)` giữa đường **interp** và đường **bytecode/VM** cho **cả** ca lỗi biên dịch (26 ca
  hiện bị bỏ qua) ⇒ **0 lệch**; nếu hai đường khác nhau thì sửa **bên sai**, **không** bỏ ca. (2) **`json.decode`**:
  đối xứng `encode`, trả `Result[Any, str]`? — chốt: `Result[T, str]` với `T = Any`, lỗi cú pháp ⇒ `Err(msg)` chứ
  **không** panic; `std.fs` **tối thiểu** (`read`, `write`, `exists`, `list`) **đi qua `trait Ngoai`/`Host`**, tuyệt
  đối **không** để `vis-interp`/`vis-vm` phụ thuộc OS; không host ⇒ `VS0604`. (3) **`| complete`**: stage cuối nhận
  `{stdout: list[str], stderr: list[str], code: int}` để `grep`/`exit 1` **không** còn là lỗi; **pipe→stdin** giữa
  hai chương trình ngoài (ghi vào stdin tiến trình sau), cap/timeout giữ như VD-60. (4) **generic thật**:
  `Hop[int]` ≠ `Hop[str]` — instantiation ở `tyck` + `codegen` (nếu buộc phải bump `VBC1` thì **hỏi Agent 0**);
  sai kiểu ⇒ `VS0412`; differential 100k **phải vẫn 0 lệch**. (5) **Quy trình**: từ vòng này **Agent 2 tự commit**
  task của mình (`--only` + push); Agent 0 chỉ commit `QUEUE`/`DECISIONS`/`AGENTS` + `docs/viscript/doc-examples-allow.txt`.

- **VD-65** **Harness chẩn đoán 3 đường (ST-32)**: (1) mọi ca `.vis`/`.vish` được phân loại **cứng** thành 3 tầng —
  **xanh** / **lỗi biên dịch** (`01xx–04xx`, `06xx`, `07xx`) / **lỗi lúc chạy** (`05xx`) — tổng phải bằng số ca sem;
  **không** ca nào bị bỏ qua. (2) Với ca lỗi biên dịch: `vis check` và `vis run --vm --no-cache` phải cho **cùng**
  `(mã, dòng, cột, thông điệp)`; **exit code ánh xạ `1 ↔ 2`** (không đòi bằng) — và `vis_bytecode::compile()` phải
  **được gọi tới** (không panic). (3) Với ca lỗi lúc chạy: `vis check` **rỗng**, `run` = `run --vm`. (4) Một hàm
  **`chuan_hoa`** dùng chung cho cả 3 đường (escape `\n`/`\t`/`\\`) + unit-test riêng — không rải logic chuẩn hoá.
  (5) **Codegen TIN tyck** (không tự kiểm kiểu): hiện **21/26** ca lỗi biên dịch vẫn sinh module chạy được; khoá hằng
  số `CODEGEN_TIN_TYCK = 21` để thay đổi phải có ý thức — đây là **nợ**, không phải hợp đồng.

- **VD-66** **`json.decode` + `std.fs` (ST-33)**: (1) `json.decode(s) -> Result[Any, str]` (tyck dùng `Ty::Any` ⇒ **bỏ
  kiểm tĩnh**, đúng VD-59(3)); object giữ **thứ tự xuất hiện**, trùng khoá ⇒ **giá trị cuối, vị trí đầu**; number có
  `.`/`e` ⇒ `float`, còn lại ⇒ `int`, **tràn i64 ⇒ `Err`** (không bão hoà/không thành float); escape **đủ** gồm
  **surrogate pair** (nửa cặp ⇒ `Err`); `null` ⇒ `none` kiểu `Opt(Error)` — hệ quả **nợ**: `let v: int? = json.decode(…)`
  ⇒ `VS0412` (đóng ở **ST-35**); lỗi ⇒ `Err("JSON không hợp lệ ở byte N: …")` với **byte offset** trong chuỗi gốc; cap
  độ sâu **128**. (2) **`std.fs`**: `read/write/exists/list` khai **có kiểu**, đi qua **`FsHost`** (một hàm `Host::fs()`
  mặc định `None`) ⇒ `vis-interp`/`vis-vm` **không** phụ thuộc OS; **không host ⇒ `VS0604` ở cả hai đường**;
  `vis-cli` impl bằng `std::fs`; `--explain-perms` in `FS read|write|exists|list <p>`; **cho phép** đường dẫn tuyệt
  đối/`..` (không sandbox), **consent thật + `cap[Dir]` ⇒ VS2**; `list` sắp xếp theo byte, **không** đệ quy. (3) **Bộ
  sem giữ hermetic**: không fixture nào cần FS ⇒ **338 ca không đổi**; test FS/JSON nằm trong `crates/vis-cli/tests/`.

- **VD-67** **Pipe→stdin + `| complete` (ST-34)**: (1) `a | b` khi `b` là **chương trình ngoài** ⇒ stdout của `a` được
  ghi vào **stdin** của `b`: `str` ⇒ **nguyên văn** (không thêm newline), `list[str]` ⇒ nối `\n` + **một** newline cuối;
  stdin **đóng** ngay khi ghi xong. (2) **`MAX_STDIN` = 8 MiB** (byte UTF-8) mỗi stage — vượt ⇒ `VS0501` nêu **số byte
  thật** + giới hạn, **trước khi spawn**; ghi stdin bằng thread riêng để tránh deadlock; `EPIPE` sau timeout được bỏ qua.
  (3) **`| complete`** ⇒ record `{stdout: list[str], stderr: list[str], code: int}` — **chỉ** dùng sau stage **ngoài**
  (Vis ⇒ `VS0602`); nhờ đó `grep` không khớp (`exit 1`) **không** còn là lỗi. (4) **Timeout 10 s/stage**, thông điệp
  nêu **stage nào**; **không** cap tổng (chuỗi N stage ⇒ tối đa N×10 s) — ghi JOURNAL. (5) `complete` có kiểu tĩnh
  **`Ty::Any`** (nợ kiểm tĩnh: `r.code` chưa kiểm được; pipeline chưa là biểu thức ở `.vish`).

- **VD-68** **Chữ ký lệnh generic (ST-37)**: (1) `CmdSig` mang **tham số kiểu** `[T]`; so khớp tham số khi hạ
  dùng **hợp nhất** (`unify`, mượn máy ST-28) thay so sánh bằng — khe ống (VD-53 (1)) tìm tham số vị trí **đầu
  tiên** hợp nhất được với kiểu giá trị vào; `take`/`lines` thành `take[T](xs: list[T], n: int) -> list[T]` /
  `lines(s: str) -> list[str]`. (2) **Xoá kiểu ở biên lệnh** như ST-28; **không** bump `VBC1`; **không** đơn
  hình hoá. (3) `CmdSig` thêm `nguon: SigSource { Builtin, FileCmd, ModuleCmd }` — tra chữ ký **không** hard-code
  "builtin ⇒ Rust"; vòng 6 chỉ dùng `Builtin`/`FileCmd`, `ModuleCmd` để dành cho `std.shell` viết bằng ViScript
  sau khi tách repo. (4) Không đổi hành vi `echo/lines/take` hiện có; 338 ca sem **không đổi**.

- **VD-69** **Closure-arg trong chế độ lệnh (ST-38)**: (1) `closure_arg` (§A.7) hạ **trong `cmd_lower`** bằng
  **re-parse span** — parser **không** giữ cây con (chỉ giữ token), chỉ kiểm cú pháp thân; hạ xong thành
  `HExpr::Lambda`; kiểu tham số là `fn(T) -> R`. (2) **Cộng offset** span của thân với vị trí bắt đầu trong
  nguồn ⇒ chẩn đoán bên trong thân báo **đúng** `dòng:cột` của tệp (ca khoá riêng). (3) `it` **chưa** có ở bước
  này (xem VD-72) — ST-38 chỉ nhận lambda tường minh `x => …`. (4) Không hạ closure-arg cho **chương trình
  ngoài** (mọi tham số ngoài vẫn là `str`, VD-60).

- **VD-70** **Ngữ nghĩa verb shell (ST-38)**: (1) **một** ngữ nghĩa, **hai** tên: `where` ≡ `.filter`,
  `each` ≡ `.each`, `first` ≡ `.first`, `count` ≡ `.len` — verb là **alias có chữ ký**, gọi lại bảng phương thức
  (`builtin::method`) để interp và VM dùng **cùng** đường (VD-55) ⇒ differential giữ 0 lệch. (2) `get`/`select`
  **không** phải `.get(i)` mà là **chiếu theo trường** (§9.7 `get name`) ⇒ thuộc nhóm bản ghi (ST-40b).
  (3) Vòng 6 làm trên **`list`**; `stream[T]` lười/SIGPIPE/`take` dừng nguồn sớm để **VSH1b** (§9.5) — ghi rõ
  trong `STATE`/báo cáo, không hứa ngữ nghĩa stream. (4) `first` trả `T?` (Opt); `count` trả `int`; `each` trả
  `()`.

- **VD-71** **`sort_by` (ST-39)**: (1) Mở rộng `list.sort_by` cho khoá `K ∈ {int, size, duration, str}` (hiện
  chỉ `int`, `Ty::FnIntInt`); sắp xếp **ổn định** (§13 `coll`), khoá tính **một lần**/phần tử (giữ như ST-26).
  (2) `--desc` đảo chiều; `--asc` không có. (3) Khoá `bool`/`path` **chưa** sắp được ở vòng 6 ⇒ `VS0412` kèm
  gợi ý. (4) Ca cũ của `list.sort_by` **phải vẫn xanh** (mở rộng, không đổi hành vi).

- **VD-72** **`it` + viết tắt trường (ST-40a)**: (1) Chỉ áp cho **tham số closure** của `cmd` đã biết chữ ký;
  dải token của tham số dừng ở `--flag`/`|`/`;`/`&` **độ sâu ngoặc 0**. (2) Ident trần tra `RecDef` của kiểu
  phần tử ⇒ `it.<field>`; `it` tường minh vẫn hợp lệ; shell **không** có biến trần nên không nhập nhằng với
  `$x`. (3) Trường không tồn tại ⇒ mã mới **`VS0605`** (sổ `COMMAND_CODES` 4 → 5; `all_codes()` tăng 1;
  `next_block()` **không** đổi). (4) Không áp cho chương trình ngoài; `where size > 5mb` trên `list[str]` ⇒
  `VS0602`/`VS0605` (không có trường), không im lặng.

- **VD-73** **Prelude bản ghi + `Host` proc (ST-40a)**: (1) `FileEntry{name: str, path: path, size: size,
  modified: int, is_dir: bool}` — `modified` là **ms** ở vòng 6 (`Type` chưa có `datetime`; §3.1 để VS2);
  `ProcEntry{pid: int, ppid: int, name: str, cpu: percent, mem: size, state: ProcState}`. (2) Bản ghi prelude
  nằm trong `TypeTable` do **ViScript** cấp (không phải tệp người dùng) — vì `sig_builtin()` không biểu diễn
  được `Record(RecId)` của tệp. (3) `ProcEntry` **portable**: không `tcb`/`service`/`cap`; ViFastOs tự ánh xạ.
  (4) `Host::procs`/`list_procs` **chưa** vào hợp đồng ở vòng 6 — chỉ chốt khi có backend; thêm `HOST_API: u32`
  (bắt đầu 1) để backend ngoài `viscript/` biết mình lệch hợp đồng.

- **VD-74** **`Render` (ST-41)**: (1) Giá trị cuối pipeline in qua `Render`: `list[bản ghi]` ⇒ **bảng** (tiêu đề
  = tên trường, canh trái, **một** dấu cách phân cột); bản ghi đơn ⇒ khối `khoá: giá trị`; `list[str]` giữ
  nguyên `Show` cũ (không phá 338 ca). (2) Độ rộng cột đo theo **Unicode scalar** trên chuỗi **NFC** (NFD tiếng
  Việt làm lệch cột; §9.6 + kinh nghiệm ST-03). (3) **Không** màu, **không** cắt theo bề rộng terminal, **không**
  ViTerm ở vòng 6 (VSH1b). (4) `print` của giá trị khác `list[bản ghi]`/bản ghi **không** đổi.

- **VD-75** **`ls`/`open` qua `Host` (ST-42)**: (1) `FsHost::list_entries(dir) -> Result[list[FileEntry], str]`
  — **một** lần gọi, có metadata, thứ tự **đã sắp** theo tên (byte); không đệ quy. (2) `ls(path = p".", --all:
  bool = false, --long: bool = false) -> list[FileEntry]`; `--long` **không đổi kiểu** (chỉ đổi hiển thị, VD-74).
  (3) `vis-interp`/`vis-vm` **không** gọi OS (ca âm khoá lại); thiếu host ⇒ `VS0604`. (4) Cờ **`--fs-mock`** của
  `vis-cli` chỉ để **test tất định** (không phải API); `ls`/`open` test qua mock, không đọc đĩa thật. (5) Cưỡng
  chế quyền (`cap[Dir]`) **không** thuộc vòng 6 — VS2.

- **VD-68** **Generic thật (ST-35)**: (1) instantiation biểu diễn bằng **`RecId`/`EnumId` interned theo tổ hợp**
  (`rec_inst`/`enum_inst` + `rec_pos`/`enum_pos`), **không** thêm biến thể `Type` ⇒ `Hop[int]` ≠ `Hop[str]` mà
  `same`/`coerce`/`tbl.field` **không** phải sửa lan rộng. (2) **Payload runtime vẫn xoá kiểu** ⇒ **không** đụng
  `codegen`/`VBC1`; differential 100k **0 lệch**. (3) `T` **chưa ràng buộc** ⇒ **giữ** hành vi xoá kiểu (không làm đỏ
  ca cũ). (4) Gán chéo hai instantiation ⇒ **`VS0412`**; cùng instantiation ⇒ **sạch**; trường thay `T` ⇒ kiểu cụ thể;
  `match` trên `Or[int,str]` thiếu nhánh ⇒ `VS0402`. (5) **Nợ**: kiểu trả hàm generic còn `{error}` khi `T` chưa suy;
  `Hop[Hop[int]]` chưa có id cho lớp trong (an toàn vì payload xoá kiểu).

- **VD-69** **Phạm vi vòng 6 (VSH1a)**: (1) **khe ống hợp nhất**: khe không còn phụ thuộc so **bằng** kiểu — chọn vị trí
  theo **khả năng nhận** (generic `CmdSig`), nên `( [1,2] ) | take 1` hợp lệ; seam `SigSource` để `vis-std`/prelude cấp
  chữ ký **generic** cho lệnh. (2) **closure-arg** (`CmdClosure`) hạ thành `Lambda` **cộng offset span** để lỗi trong
  thân closure báo đúng `dòng:cột`; thêm `where`/`each`/`first`/`count`. (3) `sort_by` **`--desc`** (giữ **ổn định**),
  khoá `int/size/duration/str`; `bool` ⇒ `VS0412`. (4) **prelude bản ghi** `FileEntry`/`ProcEntry` + `it` + **viết tắt
  trường** ⇒ mã mới **`VS0605`** trong sổ `06xx` (4 → 5 mã; `BOOKS`/`all_codes()`/`next_block()` cập nhật **kèm lý do**).
  (5) `get`/`select` chiếu theo trường. (6) `Render`: bảng `list[bản ghi]` + khối khoá–giá trị, căn cột theo **NFC**;
  `print` cũ cho `list[str]` **không đổi**. (7) `ls`/`open` qua **`FsHost`** (+ `list_entries` metadata), `--all/--long`,
  cờ **`--fs-mock`** cho test tất định; pipeline §9.7 dòng 672 chạy được ⇒ allowlist **8 → 7** (chỉ khi thật xanh).

- **VD-70** **Khe ống hợp nhất + allowlist 8 → 6 (ST-37)**: (1) khe ống chọn tham số vị trí **đầu tiên nhận được**
  kiểu vào, kiểm bằng **hợp nhất một chiều** (`T := int` rồi so phần còn lại) — **không** dùng so `==`; đầu vào
  `{error}` (stage nhóm/lệnh lạ) ⇒ chọn khe **vị trí đầu**; khe **không** vào tham số `--cờ`, **không** đè bind tường
  minh; không khớp ⇒ `VS0603`; **`take 1` không có pipe** vẫn là `VS0602` (đúng VD-53 (4)). (2) `CmdSig`/`SigSource`
  đặt ở **`vis-types`** (`src/cmdsig.rs`) — **một** nguồn chữ ký cho `tyck` **và** khe ống; `vis-std`/prelude cắm
  bằng impl trait. (3) `CmdGroup` **suy kiểu literal** (list/str/int/bool) thay `{error}`. (4) **Allowlist doc-examples
  giảm 8 → 6** vì khối **1451** (`walk`/`where`/`sort_by`/`to json`/`save`) và **672** (`ls | where | sort_by --desc |
  take 10`) **nay hợp lệ** — lệnh lạ = chương trình ngoài + khe hợp nhất; **chặt hơn, không nới**. (5) Nợ **VSH1b**:
  stage đầu là **biểu thức** không ngoặc (`[1,2] | take 1` ⇒ `VS0201`); `( … )` làm **đối số** ⇒ `VS0603`; chẩn đoán
  lúc chạy trong `( … )` dùng vị trí **tương đối** trong ngoặc.

- **VD-71** **Closure-arg + lệnh shell VS1a (ST-38)**: (1) `CmdClosure` hạ thành `Lambda`; **span tuyệt đối** bằng cách
  lex/parse `" ".repeat(offset_byte) + thân` ⇒ lỗi trong thân báo **đúng `dòng:cột` của tệp** (kể cả khi trước đó có
  ký tự nhiều byte). (2) Lệnh mới `where[T](list[T], fn(T)->bool) -> list[T]`, `each[T](list[T], fn(T)->()) -> ()`,
  `first[T](list[T]) -> T?`, `count[T](list[T]) -> int` — khai **hai chỗ** (`cmdsig` cho tyck/khe, `builtin` cho chạy),
  **một** hiện thực dùng **`Host::call`** cho cả interp/VM; **không** thêm vào `BUILTIN_VALUE_NAMES` (chúng là lệnh
  **chế độ shell**, thêm vào sẽ chặn `fn first` của người dùng ⇒ `VS0302`). (3) `each` trả `()` và **là stage cuối**
  (nối tiếp ⇒ `VS0603`); `first` trên list rỗng ⇒ `none`. (4) **Sugar vị từ ngầm `it`**: đối số `where` không phải
  closure ⇒ hạ `it => …`, chỉ đổi **định danh trần chưa khai báo** thành `it.<tên>`; `where 1` ⇒ **`VS0412`**.
  (5) `lower_vach` hạ nhóm **vào bảng chung** (`RecId` ẩn danh phải cùng bảng). Nợ: `where size` (kiểu không `bool`)
  hiện `VS0501` lúc chạy.

- **VD-72** **`sort_by` chế độ shell (ST-39)**: (1) lệnh builtin **mới** `sort_by[T](xs: list[T], f: fn(T)->K,
  --desc: bool = false) -> list[T]`, **một** hiện thực qua `Host::call`; **`std.list.sort_by` giữ nguyên** (khoá
  `fn(int)->int`) — **hai** `sort_by` là **nợ VS2** (hợp nhất khi có generic + trait). (2) `--desc` là **cờ** (khe ống
  không chiếm); `--desc=false` hợp lệ; thân closure **dừng** ở `--…` (parser không nuốt cờ vào thân). (3) So sánh
  khoá: `int`/`float` theo số, **`size` theo BYTE**, **`duration` theo NANOGIÂY**, `percent` theo `f64`, `str`/`path`
  theo **byte UTF-8**; **ổn định cả hai chiều** (đảo comparator, không đảo mảng). (4) Khoá `bool`/`map`/`record`/`list`/
  `set`/`unit` ⇒ **`VS0412`** (kiểm ở lowering cho `.vish` + `kiem_builtin` cho `.vis`; runtime `VS0501` là chốt hai).
  (5) `sort_by` 1 đối số (không closure) vẫn là **chương trình ngoài** (không bị tyck chặn).

- **VD-73** **Prelude bản ghi + `VS0605` (ST-40a)**: (1) `FileEntry { name: str, size: size, modified: duration,
  is_file: bool }` và `ProcEntry { pid: int, name: str, cpu: percent, mem: size }` khai **trong Rust**
  (`vis-types/src/prelude.rs`) và đăng ký **chỉ khi hạ `.vish`** — `.vis` **không** thấy chúng (khai trùng vẫn chạy).
  (2) `it` suy kiểu **thật** từ kiểu phần tử của giá trị vào (`kieu_text`/`kieu_stage`); không suy được (`{error}`/`Any`)
  ⇒ **kiểm động** như cũ, **không** đỏ oan. (3) Viết tắt trường (`size` ≡ `it.size`) giữ luật VD-71. (4) **`VS0605`**
  "trường không có trên `it`" trong sổ `06xx` (4 → 5 mã) ⇒ `all_codes()` **29**; `BOOKS` 7, `next_block()` **801**.
  (5) `.vish` **chưa có** cú pháp khai `cmd`/`type`/`let` có kiểu ⇒ kiểu `it` thật đến từ **literal bản ghi** khớp
  prelude; nợ: khi `.vish` có khai `type` ⇒ phải `VS0302` với tên prelude.

- **VD-74** **`get`/`select` (ST-40b)**: (1) `get(xs, f) -> list[K]` — `… | get name` ⇒ sugar `it => it.name` (VD-71);
  kiểu `K` tính **từ thân lambda** ở `kiem_builtin` (không để `TypeRef` xuống tyck ⇒ không `VS0301` oan). (2)
  `select(xs, k1, k2, …) -> list[record#N ẩn danh]` — hạ thành lambda trả **record literal**, trường kiểu lấy từ `it`,
  **giữ đúng thứ tự đối số**; **không** thêm `Type::Any` (nợ VS2: chưa `to json` được). (3) Trường lạ trên `it` kiểu
  **thật** ⇒ **`VS0605`**; `it` kiểu `{error}`/`Any` ⇒ **kiểm động**, **không** báo oan. (4) `get`/`select` 0 đối số ⇒
  `VS0602`; `get` trên list rỗng ⇒ `[]`. (5) R1 (>1.500 dòng) ⇒ **tách** `lower/chieu.rs`, không nới ngưỡng.

- **VD-75** **`Render` (ST-41)**: (1) giá trị **cuối pipeline** ở chế độ shell được render qua builtin **`render`**
  (`vis-interp/src/render.rs`) — `boc_print` → `boc_render`; **`print` giữ nguyên từng byte**. (2) `list[bản ghi]` ⇒
  **bảng**: tiêu đề = tên trường, căn **trái** `str`, **phải** `int`/`float`/`size`/`duration`/`percent`; bản ghi đơn ⇒
  **khối khoá–giá trị**; `list[bản ghi]` **rỗng** ⇒ `(rỗng)`; **`list[str]`/`list[int]` rỗng và kiểu `{error}` giữ
  `Show` (`[]`)**. (3) Độ rộng cột = **số `char` sau NFC** (không `display_width`) ⇒ NFD/NFC cùng bảng, emoji/CJK = 1.
  (4) **Đổi hành vi có chủ ý**: 7 fixture `.out` chuyển `Show` → bảng (liệt kê trong commit `f2c256f`), ~20 assertion
  Rust theo cùng luật. (5) **Cache**: nhãn `PHIEN_BAN` phải đổi khi ngữ nghĩa render đổi (`+render-st41`) — nếu không,
  `--vm` dùng module cache cũ và in `Show`.

- **VD-76** **`ls`/`open` + `--fs-mock` (ST-42)**: (1) `ls [p] [--all] [--long]` ⇒ `list[FileEntry]` (prelude VD-73),
  **sắp xếp theo tên byte**, `--all` gồm tên bắt đầu `.`, **`--long` no-op** (nợ), **không đối số ⇒ `"."`**;
  `open(p)` ⇒ `str`, lỗi ⇒ `VS0501` nêu **đường dẫn + lỗi OS**. (2) **`FsHost` mở rộng** `MucFs{name,size,modified_ns,
  is_file}` + `list_entries` **có default `Err`** ⇒ impl cũ vẫn biên dịch; `vis-interp`/`vis-vm` **không** biết OS.
  (3) **`--fs-mock <tệp.json>`** (`dirs`/`files`; `size` = literal **theo bảng lexer** ⇒ `2mb` = **2 000 000**, `2mib` =
  2 097 152; `modified` = duration **từ epoch** ⇒ **tất định**); không mock ⇒ `FsHost` thật. (4) **§9.7 khối 672 chạy
  thật end-to-end** với `--fs-mock` (bảng, interp ↔ `--vm` cùng byte). (5) `vis test` **không** có `--fs-mock` ⇒ fixture
  cần FS nằm ở `test/cmd/fs/**` chạy bằng test riêng, **không** tăng `sem` ảo.

- **VD-77** **Phạm vi vòng 7 (VSH1b)**: (1) **`.vish` có khai báo kiểu** — `type`/`enum`/`let x: T`/`cmd f(…) -> T` được
  chấp nhận **trong chế độ lệnh** (trước đây `(` bị hiểu là `CmdGroup`): khai báo là **item** đứng trước stage, không
  trộn với biểu thức; tên trùng prelude ⇒ **`VS0302`**. (2) **`where` kiểm tĩnh đầy đủ**: khi kiểu `it` **biết được**
  mà vị từ không ra `bool` ⇒ lỗi **kiểm tĩnh** (`VS0412`), hết nợ `VS0501` lúc chạy; kiểu `{error}` ⇒ **giữ kiểm động**.
  (3) **`ls --long` thật**: thêm cột thời gian (định dạng **tất định**, UTC) + quyền/kích thước **chỉ đọc** từ `FsHost`
  (`MucFs` mở rộng `mode`/`mtime`), **không** đụng `Render` bảng mặc định (chỉ khi `--long`). (4) **`( … )` làm đối số**:
  nhóm được dùng như biểu thức ở vị trí đối số; **chẩn đoán trong ngoặc** dùng vị trí **tuyệt đối** trong tệp (hết nợ
  "vị trí tương đối"). (5) **Không** thêm mã lỗi mới trừ khi được duyệt; mọi thay đổi hành vi phải liệt kê ca `.out` đổi.

- **VD-78** **Khai báo trong `.vish` (ST-44)**: (1) `.vish` = **vùng item ở đầu tệp** + **vùng stage** phía sau; ranh giới là
  dòng **cột 1** đầu tiên không bắt đầu bằng từ khoá item (`type|enum|cmd|fn|use|const|trait|impl|alias|newtype|extern`)
  và không phải `let` ⇒ từ đó là stage. (2) Tiền tố parse lại **Mode::Script** với **span tuyệt đối** (đệm offset) và hạ
  vào **cùng** `Symbols`/`TypeTable` ⇒ item thấy prelude, stage thấy item. (3) Khai báo **sau** stage ⇒ **`VS0201`** kèm
  gợi ý "khai báo phải đứng trước stage đầu tiên"; **trùng tên** (kể cả `FileEntry`/`ProcEntry`) ⇒ **`VS0302`** (không
  thêm mã). (4) **`f()`** = lời gọi **0 đối số**; nhóm rỗng đứng riêng ⇒ `VS0201`. (5) `let x: T = <có `|`/`;`/`>`/`<`/`&`>`
  ⇒ **shell-let**, ngược lại ⇒ khai báo script. (6) **Nợ VSH1b-2**: thân `cmd` viết bằng **pipeline lệnh** (thụt) chưa
  hỗ trợ — thân hiện là **script**.

- **VD-79** **`where` kiểm tĩnh / `each` không kiểm (ST-45)**: (1) **`where`** kiểm ở **`tyck`** (`coerce` thân lambda,
  khai `fn(T)->bool`) — đã chạy cho cả `.vish`; vị từ sai ⇒ **`VS0412`** nêu **kiểu thật**. (2) **`each`** khai kiểu trả
  lambda = **`{error}` (any)** ⇒ **không** kiểm kiểu trả (giống `for` trong Rust); lỗi khác trong thân **vẫn** báo.
  (3) **`it` không suy được kiểu** (`{error}`/`Any`) ⇒ **giữ kiểm động** (`VS0501` lúc chạy), **không** đỏ oan.
  (4) **Hai mã field-access khác nhau đang tồn tại**: closure tường minh ⇒ **`VS0403`** (tyck), viết tắt `it` ⇒
  **`VS0605`** (hạ) — **cả hai đều đỏ**; **thống nhất** thành một mã ⇒ task riêng (nợ). (5) R1 (>1.500 dòng) ⇒ tách
  `lower/sugar.rs`, không nới ngưỡng.

- **VD-80** **`ls --long` + nhóm-đối-số (ST-46)**: (1) `ls --long` ⇒ **`list[FileEntryLong]`** (`name,size,modified,is_file,
  mode,modified_time`) — **bản ghi riêng**, prelude `FileEntry` **không đổi**; `mode: u32` trong `MucFs` (**0 ⇒ `-`**),
  `modified_time` = **UTC `YYYY-MM-DD HH:MM`** (civil-from-days, **không** thêm crate). (2) **Nhóm là đối số** = biểu
  thức (`take ( 1 + 1 )`), **bỏ `VS0603`** cho trường hợp này, sai kiểu ⇒ `VS0412`; nhóm **rỗng** vẫn là **lời gọi
  0 đối số**; shell-let (VD-78(5)) **không** đổi. (3) **Chẩn đoán tuyệt đối** trong `( … )` bằng **đệm dấu cách**
  (không `\n` ⇒ không sinh `Indent`/`Dedent` giả). (4) Nợ: **nhóm-lệnh có cờ** (`( ls … --long )`), nhóm **nhiều dòng**,
  `where ( x => x )` kiểm **động** (tyck chưa kiểm call `where`).

- **VD-81** **Phạm vi vòng 8 (VSH1c)**: (1) **thân `cmd` bằng pipeline lệnh**: khối thụt sau `cmd f(...) -> T:` parse ở
  **chế độ lệnh** (Indent/Dedent + stage), hạ thành thân hàm trả giá trị cuối pipeline; chạy **cả hai đường**.
  (2) **Nhóm-lệnh có cờ** (`( ls … --long )`) được hạ như stage (không rơi về lệnh ngoài `(`); **nhóm nhiều dòng**
  hợp lệ trong ngoặc. (3) **Thống nhất mã field-access**: một mã duy nhất cho "không có trường" giữa **tyck** và **hạ**
  (hiện `VS0403` vs `VS0605`) — chọn **`VS0605`** (chuyên biệt, đã có vi/en/ja từ ST-40a), đổi phía tyck; snapshot +
  test cũ sửa **kèm lý do**. (4) **`where ( x => x )`** kiểm **tĩnh** (mở rộng `kiem_builtin` cho `where`).
  (5) **Stage đầu là biểu thức** (`[1,2] | take 1`) nếu **không** phá văn phạm §9.5; nếu phá ⇒ **giữ `VS0201`** + ghi nợ.
  (6) Mỗi task ≤ 3 h; VS2 vẫn ngoài phạm vi và **phải** được nêu trong R8.

- **VD-82** **Thân `cmd` bằng pipeline (ST-48)**: (1) Parser **thử `Mode::Script`** trên thân (bỏ thụt): sạch ⇒ **script**
  (raw như VD-78), hỏng ⇒ **thân lệnh** (`CmdKhai` dừng sau `:`; dòng thân do parser lệnh chính parse ⇒ **span tuyệt đối**).
  (2) Tiền tố được thay bằng **filler `()` giữ nguyên độ dài byte** ⇒ offset của **mọi** item sau **không đổi**.
  (3) Thân lệnh = **đúng một pipeline**; rỗng / nhiều pipeline / lẫn `let`-và-lệnh ⇒ **`VS0201`**. (4) Hạ bằng
  `ha_pipeline` trên Lowerer **con** (chung bảng) ⇒ `HItem::Fn.body = [giá trị cuối pipeline]`, **không** `HExpr::Pipeline`,
  **bỏ** `boc_render`. (5) Nợ: thân lệnh **chưa dùng tham số `cmd`**; body "vừa script vừa lệnh" ⇒ `VS0201` (bảo thủ).

- **VD-83** **Nhóm-lệnh + nhóm nhiều dòng (ST-49)**: (1) **một** đường hạ nhóm duy nhất (`lower_nhom_lenh` tự-dựng-Call
  **đã xoá**): nội dung nhóm được **đổi newline thành dấu cách** (1 byte→1 byte ⇒ offset **tuyệt đối** giữ nguyên), parse
  lại (`parse_voi`), hạ **cả pipeline** bằng Lowerer **con** chung `Symbols`/`TypeTable`. (2) Parser **không** còn chặn `|`
  trong nhóm; newline **không** kết thúc nhóm. (3) `kieu_stage` trả kiểu **đúng** của nhóm ⇒ `( ls --long ) | select mode`
  kiểm tĩnh được. (4) Nợ: trong nhóm chỉ **một** pipeline (nhiều pipeline/`;` ⇒ `VS0201`); kiểu tĩnh lấy **stage đầu**.

- **VD-84** **Trường là lỗi kiểu ⇒ `VS0405` (ST-50)**: (1) mã "không có trường" nằm ở sổ **`04xx`** (**`VS0405`**), không ở
  `06xx`; `COMMAND_CODES` **4**, `TYPE_CODES` **5**, `all_codes()` **29**, `BOOKS` 7, `next_block()` **801**. (2) `VS0403`
  giữ nghĩa **khai báo** (literal bản ghi, `with`, pattern, literal enum, phương thức, impl/derive trait). (3) **Tra mã
  theo `id`, không theo chỉ số mảng** (`ma_truong()`) — đổi thứ tự sổ không được làm lệch mã. (4) **`where ( x => … )`**
  kiểm **tĩnh** (nhánh `"where"` trong `kiem_builtin`; thân phải `bool` ⇒ `VS0412`; `{error}` ⇒ kiểm động); ở hạ, lambda
  trong ngoặc được gán kiểu tham số = **kiểu phần tử**. (5) Nợ: `VS0403` còn nghĩa rộng; chưa kiểm kiểu tham số khi lambda
  không chú thích.

- **VD-85** **Stage đầu là biểu thức (ST-51)**: (1) Nhận **hẹp nhất** trong chế độ lệnh: token **đầu dòng** là `[`, `{`, hoặc
  **string literal**; có `|` ở **độ sâu 0**; văn bản tiền tố parse **`Mode::Script` phải 0 chẩn đoán**. (2) CST = `CmdStage`
  bọc `CmdGroup` **không ngoặc** ⇒ **tái dùng** đường hạ `( … )` (không thêm đường mới). (3) `{…}` được **bọc
  `HExpr::List` một phần tử** ⇒ stage hạ nguồn (`where`/`take`/`select`/bảng) hiểu như một dòng. (4) **KHÔNG** nhận:
  số (`1 | …` vốn là lệnh ngoài), **biến** (`x | …` nhập nhằng §9.5), **tên lệnh**; không `|` ⇒ giữ **`VS0201`**.
  (5) Nợ: bọc-list chỉ cho stage **đầu**; biến ⇒ **VSH1c-2**.

- **VD-86** **Phạm vi vòng 9 (VSH2)**: (1) **Bản ghi ẩn danh đi được JSON**: `select`/literal ẩn danh phải `json.encode`
  được (dùng **tên tổng hợp** ổn định hoặc `null` cho tên, **không** cần `Type::Any`). (2) **Một `sort_by`**: shell
  `sort_by` và `std.list.sort_by` dùng **cùng** comparator (khoá `int/float/size/duration/percent/str`; **ổn định**;
  `--desc` chỉ ở shell) — `std.list.sort_by` cũ **không** phá (nhận `fn(int)->int` vẫn chạy). (3) **`CODEGEN_TIN_TYCK`**:
  codegen **không** im lặng sinh module khi HIR còn kiểu `Error` — phải **từ chối** với chẩn đoán rõ **hoặc** chứng minh
  và **hạ hằng số** 21 xuống; differential **0 lệch** bắt buộc. (4) `Ty::Any`/`&self` **chỉ** nếu rẻ, còn lại nợ có chủ đích.
  (5) **VS2** phải có **tiêu chí đo** trong `R9` (async/kênh/net/Landlock/LSP/gói/registry/4 nền tảng).

- **VD-87** **`to json` + hợp nhất `sort_by` (ST-53)**: (1) **`to json`** là **stage lệnh** (hạ `Call(std.json.encode,
  [prev])`, `--pretty` ⇒ `encode_pretty`), **không** cần `use std.json`; bản ghi **ẩn danh** encode **theo trường**
  (serializer **bỏ tên** bản ghi) ⇒ **không** cần `Type::Any`. (2) **Một comparator**: `sap_xep_theo_khoa` — tính khoá
  **một lần**, **sort ổn định**, `desc` chỉ **đảo comparator**; shell `sort_by` = `KieuKhoa::Moi`
  (`int/float/size/duration/percent/str`), `std.list.sort_by` = `KieuKhoa::ChiInt` (**giữ** `FnIntInt` + câu lỗi cũ).
  (3) Nợ: khoá `str` cho `std.list.sort_by`.

- **VD-88** **Codegen từ chối HIR có chẩn đoán (ST-54)**: (1) **Không** chặn bằng soi `Type::Error`/`TypeRef` — walker chứng
  minh HIR **sạch kiểu** kể cả khi tyck báo lỗi. (2) Cửa đúng: `compile_kiem(hir, symbols, text, chan_doan)` — **có**
  chẩn đoán `error` ⇒ `Err(CodegenError::ChanDoan{short, dong, cot, msg})`, **không** sinh module; `compile()` giữ nguyên
  (= `compile_kiem(…, &[])`). (3) Phòng thủ `HExpr::Error` ⇒ `Err(Unsupported("error-expr"))`. (4) Cửa **chỉ chặn
  `Severity::Error`**; cảnh báo **không** chặn (có ca). (5) `CODEGEN_TIN_TYCK` = **0**; `diag_cmp` so **đủ** ba thứ
  (mã, dòng:cột, thông điệp) qua `compile_kiem`.

- **VD-89** **Phạm vi vòng 10**: (1) Sáu task **ST-56..ST-61** + **ST-62 (R10)**, chạy **tuần tự** (đụng chung
  `stdlib.rs`/`tyck`/`codegen`/`lower` ⇒ không song song ghi). (2) Mỗi task **red-first**, commit riêng `[ST-##]`,
  push sau khi cổng xanh. (3) **Không** nới ngưỡng, **không** tăng allowlist (chỉ được giảm). (4) Async/kênh **phải**
  cùng byte interp ↔ `--vm`; nếu một task không đủ thì **tách** thành task nhỏ hơn, **không** hạ tiêu chí.

- **VD-90** **Async — tác vụ chạy tới hoàn tất, một luồng, tất định (ST-60)**: (1) `async fn` khai báo được; **gọi**
  `async fn` **không** chạy thân ngay mà trả `Task[T]`; `spawn f(args)` = tạo tác vụ + đưa vào **hàng đợi FIFO**;
  `await t` chạy `t` **tới hoàn tất** (nếu chưa chạy) rồi trả `T`. (2) Bộ lập lịch **một luồng, tất định**; **không
  nhường giữa thân** (run-to-completion) — đây là **nợ ghi rõ**: chưa có đình chỉ giữa thân nên **không** có song song
  thật; mô hình này để hai đường (interp/VM) ra **cùng byte**. (3) Cuối chương trình: task chưa `await` được **join**
  theo **thứ tự spawn**; thân task lỗi ⇒ `VS0506` (mã mới, sổ 05) kèm tên hàm. (4) `await` lần hai trả **cùng** giá trị
  đã lưu (không chạy lại). (5) 10.000 task dùng **hàng đợi phẳng** (Vec), không đệ quy theo số task ⇒ không tràn.
  (6) Benchmark 10k task là **test thật**, không `#[ignore]`.

- **VD-91** **Kênh — đệm hữu hạn + deadlock có chẩn đoán (ST-61)**: (1) `chan[T](n)`, `n ≥ 0`; `send(v)`,
  `recv() -> T?`, `close()`. (2) `recv` khi rỗng: **chạy tác vụ khác** trong hàng đợi (FIFO, tới hoàn tất) rồi thử lại;
  vẫn rỗng và **không còn task** ⇒ `VS0505` **deadlock** (không treo). (3) `send` khi đầy: như (2); vẫn đầy ⇒ `VS0505`;
  `send` sau `close` ⇒ `VS0504`. (4) `recv` sau `close`: còn hàng ⇒ trả phần còn lại; rỗng ⇒ **`none`**.
  (5) `Chan[T]` là kiểu dựng sẵn; tyck **đòi khớp `T`** khi `send`/`recv`.

- **VD-92** **`&self`/`&expr` — không mượn thật (ST-56)**: (1) `&self` trong `impl` là receiver **không mượn thật**,
  hạ **y như** `self`; `&expr` giữ **nguyên kiểu trong**, hạ **identity** (không tạo giá trị mới). (2) Codegen **hết**
  `HExpr::Ref ⇒ Unsupported`; interp ↔ `--vm` cùng kết quả. (3) Mượn thật/kiểm vòng đời là **nợ VS2+**, phải ghi rõ —
  **không** được mô tả là "đã có borrow".

- **VD-93** **`derives` sinh mã ở `lower` (ST-57)**: (1) Sinh **HIR** trong `lower` (không macro, không sửa parser).
  (2) Tối thiểu: `Show`, `Eq` (thêm `Json` nếu rẻ) — sinh **phương thức thật** gọi được như `p.show()`/`a.eq(b)`, thứ tự
  theo **trường khai báo**, tất định. (3) Trường không derive được (`float` với `Eq`) ⇒ chẩn đoán **tĩnh** `VS04xx`,
  **không** sinh mã. (4) Người dùng viết tay cùng tên ⇒ **người dùng thắng**, không sinh bản trùng.

- **VD-94** **Bỏ `Ty::Any` bằng biến kiểu trong chữ ký std (ST-58)**: (1) `stdlib.rs` dùng **biến kiểu** thay `Any`
  (`list.sort: (list[T]) -> list[T]`; `list.sort_by: (list[T], fn(T) -> int) -> list[T]`; `map.get_or: (map[K,V], K, V) -> V`).
  (2) `tyck` **hợp nhất** kiểu đối số thật với biến kiểu; không khớp ⇒ `VS0401` **tĩnh**. (3) Ngoại lệ **có ghi** (closure
  chưa suy hai chiều) phải viết rõ tại chỗ, **không** để `Any` quay lại. (4) `all_codes()` **không** đổi, trừ mã mới
  của ST-60/ST-61.

- **VD-95** **Hiện thực async — builtin nội bộ, không thêm tag `VBC1` (ST-60)**: (1) `async`/`await` **đã** lex+parse+lower
  sẵn (`HFn::is_async`, `HExpr::Await`); chỉ thiếu tyck + runtime. (2) Hạ `async fn`/`spawn`/`await` thành **lời gọi
  builtin nội bộ** `$tac_vu`, `$spawn`, `$await` (tên có `$` ⇒ người dùng không viết được) + **một** runtime chia sẻ
  `vis-interp/src/rt.rs`; **không** thêm tag `VBC1`, không đổi verifier/cache. (3) `Task[T]`/`Chan[T]` =
  `Type::TypeRef{name,args}` (đã đệ quy sẵn); `FnSig.is_async`; `await` bỏ **một** lớp `Task`. (4) `spawn` chỉ nhận
  **tên hàm `async` cấp tệp**, đối số **vị trí**, **không** `--tên`; `async fn` không dùng làm giá trị; `async` trên
  method/`cmd` ⇒ **`VS0412`** (nợ ghi `R10` §5). (5) Lỗi thân tác vụ: **mọi** lỗi bọc `VS0506` (message kèm mã gốc),
  span = **lúc tạo**, áp **cả** `await` lẫn join — một quy tắc, một kết quả. (6) Join cuối **theo thứ tự spawn**,
  **chỉ khi main thành công**; `await` lần hai trả cache. (7) `MAX_TASK_NEST = 256` cho chặn lồng; 10k task phẳng là
  test thật. Nợ: không nhường giữa thân, không GC (chu trình `Rc`), `await` sâu sát ngưỡng frame.

- **VD-96** **Hiện thực kênh + ba mã runtime mới (ST-61)**: (1) `$kenh` tạo `Chan` (**một** hiện thực trong `builtin`),
  `send/recv/close` đi cùng một nhánh method cho cả interp lẫn VM. (2) Chặn (`send` đầy, `recv` rỗng) ⇒ **drain** hàng
  đợi FIFO (chạy task đầu tới hoàn tất, bỏ task đã xong), thử lại; hết queue ⇒ **`VS0505`** (deadlock, không treo).
  (3) `send` sau `close` ⇒ **`VS0504`**; `recv` sau `close` còn hàng ⇒ trả tiếp, rỗng ⇒ `none`. (4) Sổ `05xx` thêm
  **`VS0504`/`VS0505`/`VS0506`** (giữ index 0..2), `all_codes()` **29 → 32**; sửa **đủ** 6 chỗ khoá số ca. (5) Byte
  giống nhau nhờ: cùng `rt.rs`, cùng thứ tự join, cùng hằng thông điệp, span **nút gọi** — cấm in địa chỉ/thứ tự map.

- **VD-97** **Một ngữ nghĩa cho phép gán (ST-56b)**: (1) `self` là **tham số bất biến** như mọi tham số ⇒ `self.x = v`
  khi `self` (không `var`) là **`VS0412`** ở **cả hai** đường (tyck chặn trước codegen). (2) `var self`/`var x` được gán:
  `x.f = v` = **rebind cục bộ** `x = x with f: v` (đúng codegen hiện có; người gọi **không** thấy đổi — giá trị), `x[i] = v`
  = sửa **tại chỗ**. (3) `let`/tham số thường: giữ `VS0412` (đã có). (4) **Interp** phải làm đúng hai đường gán trên
  (hiện chỉ VM làm) ⇒ interp ↔ VM cùng byte. (5) `vis run --vm` gặp **lỗi codegen** phải **exit ≠ 0** + chẩn đoán ra
  stderr (hiện in rồi exit 0); nếu chương trình đó là lỗi **tĩnh** thì tyck phải chặn trước để parity không lệch.

- **VD-98** **Mồ côi đa tệp + siêu trait + derives sinh mã (ST-57)**: (1) **Mồ côi**: cho `impl` khi **trait hoặc
  kiểu** được khai trong **chương trình** (luật Rust); `impl` inherent trên kiểu nhập **giữ nguyên** (ca đang xanh);
  mồ côi **thật** = cả trait lẫn kiểu ngoài chương trình ⇒ `VS0403` thông điệp riêng. Dữ liệu: `TraitSig{name,
  supertraits, methods}` + `LowerOptions.trait_ngoai/impl_ngoai` (module xuất `pub_traits`). (2) **Siêu trait**: dùng
  **`VS0403`** (không cấp mã mới — tiền lệ ST-29); kiểm **một tầng**, **sau** vòng 1b (thứ tự impl tự do); bắc cầu = nợ.
  (3) **`derives`**: sinh **HIR ở lượt 1c** (sau 1b, trước mọi lượt hạ thân — để `p.show()` trước khai báo vẫn phân giải);
  tên nội bộ **có tiền tố** `<Kiểu>.show`/`<Kiểu>.eq` (không `VS0302`); `show` dựng bằng `HExpr::Interp` để **khớp byte**
  `Value::show`; `eq` = `self == other` (dùng `eq_value` sẵn có); người dùng viết tay **thắng**; generic/`float`+`Eq`/
  `Fn` ⇒ `VS0403` **tĩnh, không sinh mã**; `Json` giữ **kiểm-only**; `enum show` **cắt** (nợ), `enum eq` làm được.
  (4) Ca mới ở **nhóm `trait/`** ⇒ **19 nhóm**; khoá interp=`--vm` tự động.

- **VD-99** **Bỏ `Ty::Any`: biến kiểu trong chữ ký `std.*` (ST-58)**: (1) `Ty` const-friendly giữ `Copy`, thêm
  `Var(u8)` (`0=T,1=K,2=V`), `List(&'static Ty)`, `Map(&'static Ty,&'static Ty)`; **xoá hẳn** `Ty::Any` và `Ty::kieu()`
  (cái `else { continue }` là lỗ làm "Any" lọt im lặng). (2) `sang_type()` **tổng**: `Var(i)` → `Type::TypeRef{name:
  ["T","K","V"][i]}` (tái dùng `TypeRef`, **không** thêm `Type::Param`). (3) `kiem_std` **hợp nhất** bằng `hop_nhat` +
  `thay_gen` sẵn có (trái→phải, `{error}` không gán, biến xuất hiện 2 lần phải `same`); số đối số ⇒ `VS0401`, kiểu ⇒
  `VS0412`; **không** mã mới. (4) **16/38** chữ ký đổi (list 8, map 5, json 3); `json.decode` trả `Var` **không gán** ⇒
  `{error}` (giữ VD-66). (5) **Ngoại lệ có ghi duy nhất**: `list.sort_by` khoá = `Ty::FnKhoa` (lambda chưa chú thích là
  `{error}`; `Type` không có union `int|str` sau ST-56) — tầng chạy vẫn kiểm. (6) `kiem()` của `vis-interp` coi
  `Var/List/Map` là **nhận** (thân hàm tự kiểm như hôm nay) ⇒ 71 ca `test/std` + differential không đổi.

- **VD-100** **Net/HTTP: socket thô ở host, giao thức ở `vis-interp` (ST-63..ST-66, vòng 11)**: (1) Thêm `NetHost` cạnh
  `FsHost` (`connect/listen/local_addr/accept/read/write/close`, handle = `str` mờ); `Host::net` default `None` ⇒
  **`VS0604`** cả hai đường. (2) **HTTP client nằm trong `vis-interp`** (module mới `net.rs`) ⇒ interp ↔ VM **cùng byte**,
  codegen **không** sửa (duyệt `std.*` theo tên). (3) API: `net.connect/connect_timeout/listen/local_addr/accept/
  accept_timeout/read/write/close`, `http.get/get_timeout` — tham số tuỳ chọn **tách tên hàm** (builtin bỏ đối số có tên);
  handle là `str`, **không** thêm variant `Ty`. (4) **Chỉ loopback** (`127.0.0.1`/`localhost`; host khác ⇒ `Err`) trong
  vòng này; **không** thêm `--net-mock` — ca net dựng **server loopback thật** trong test Rust (`bind("127.0.0.1:0")`);
  timeout ≤ 400 ms; coi `WouldBlock` **và** `TimedOut` là hết giờ. (5) **Không mã lỗi mới**: lỗi mạng = `Err(str)`,
  không host = `VS0604`, sai kiểu/arity = `VS04xx`/`VS0501`. (6) R1: `interp.rs` sát 1.500 dòng ⇒ **tách `impl Host`**
  hoặc phần net ra tệp riêng **trước** khi thêm. (7) Ca: ≥13 ca e2e (≥2 lỗi) + `--vm` **có cache và `--no-cache`**.

- **VD-101** **Gói + registry local (VS2 #6, ST-6x)**: (1) **Không mạng**: registry là **thư mục local** (mặc định `./registry`,
  `--registry`/`VIS_REGISTRY`); **không** crate mới, **không** semver range — phụ thuộc ghi **phiên bản chính xác**. (2) Giữ
  tiền tố **`pkg.`**: `use pkg.<tên>[.<mod>]`; tên gói `[a-z][a-z0-9_-]{0,31}` (không chứa `.`); cài vào
  `.vis/pkg/<tên>/<phiên bản>/`. (3) Manifest + lockfile = **JSON** (dùng `vis_interp::json`, không thêm dep); `files[]`
  **tường minh** (không glob), mỗi tệp có `sha256`; `tree_sha` = SHA-256 trên danh sách `(path, file_sha)` **sắp theo byte**,
  chuẩn hoá BOM/CRLF/đúng một LF cuối; **không** timestamp, **không** đường dẫn tuyệt đối trong lock. (4) Cài **tất định**:
  kiểm hết rồi mới ghi; thư mục đích tạo lại từ đầu; ghi tmp + rename. `install` chỉ đọc lock (không cần registry).
  (5) `vis run` kiểm `file_sha` của tệp gói khi nạp ⇒ lệch = **`VS0701`** "gói bị sửa". (6) **Không mã lỗi mới**; lỗi CLI
  `vis pkg` = exit 2 + `vis pkg: …`. (7) Mọi `manifest.json` commit kèm `.license` (R3); `.gitignore` thêm `.vis/`.

- **VD-102** **LSP `vis lsp` (VS2 #5, ST-6x)**: (1) **Tự viết framing** `Content-Length` trên `stdin/stdout` nhị phân; **không**
  `println!`, **không** dùng `xuat_stdout`; thoát khi `exit` **hoặc EOF**. (2) JSON dùng lại `vis_interp::json` (decode) +
  `vis_interp::stdlib::json` (encode) — **không** thêm crate. (3) Nguồn chẩn đoán = `parse`+`lex`+`lower_into`+`check`
  (đúng đường `vis check`), gộp 3 danh sách, sắp `(span.start, code)`; severity Error=1/Warning=2; `source:"vis"`.
  (4) **Vị trí LSP = 0-based, UTF-16, tab=1** — tự viết `vi_tri_utf16`/`byte_tu_vi_tri`, **không** dùng `modules::vi_tri`
  (1-based, scalar, tab=4); tính trên văn bản đã chuẩn hoá NFC/CRLF→LF của `LexOutput::text`, ghi rõ giả định.
  (5) v1 **một tệp**: completion = từ khoá + item + symbol + builtin (khử trùng, sắp byte, cap 200); hover = chữ ký khai
  báo/từ khoá/symbol; **không** module-aware, **không** suy kiểu biểu thức (nợ). (6) Method lạ ⇒ `-32601`; JSON hỏng ⇒
  `-32700` (id `null`); trước `initialize` ⇒ `-32002`; `shutdown` ⇒ `null`; `exit` sau shutdown ⇒ 0, trước ⇒ 1.
  (7) ≥13 frame snapshot tất định (`file:///t.vis`, không timestamp/path tuyệt đối).

- **VD-103** **Quyền: `cap[Dir]`/`cap[Proc]`/`cap[Net]` cưỡng chế ở **biên host** (ST-72, ST-73)**: (1) Ngôn ngữ **đã** có cú
  pháp `@ path` + `(write)` trên tham số và `--explain-perms`; vòng này **cưỡng chế** thay vì chỉ in. (2) Cưỡng chế nằm ở
  **host** (`FsHost`/`NetHost` nhận **tập quyền của phiên**), **không** ở interp/VM ⇒ hai đường vẫn cùng byte và
  `viscript/` không chạm OS. (3) Vi phạm ⇒ **`VS0604`** (chưa hỗ trợ/chưa cấp quyền) với thông điệp nêu **quyền thiếu** +
  đường dẫn; **không** mã mới. (4) `--cap-mock <json>` cho test tất định (giống `--fs-mock`), **không** phải cờ thật để
  "xin quyền". (5) **Landlock thật** chỉ trên Linux ≥6.x: crate `vis-host-linux` (ngoài `viscript/`) `cfg(target_os =
  "linux")`; test chỉ biên dịch/chạy trên Linux và **ghi rõ** trên macOS là **chưa đo** — cấm mô tả là "đã có Landlock".

- **VD-104** **Đa nền tảng: bằng chứng theo **mức** (ST-74, ST-75)**: (1) `x86_64-pc-windows-msvc` + `wasm32-unknown-unknown`
  chỉ đòi **`cargo check` xanh** (biên dịch chéo), **không** tuyên bố "chạy trên Windows/wasm" khi chưa có CI/runner —
  báo cáo phải ghi **mức** (check/biên dịch/chạy). (2) `vis-host-windows`/`vis-host-wasm` là **crate khung** trong
  workspace, **không** nằm trong `viscript/`; phần lõi ngôn ngữ **không** được `cfg(target_os)`. (3) Bộ
  `host-conformance` ≥50 ca chạy **thật** trên macOS trong `viscript/test/host/` + harness Rust; 3 nền còn lại ghi rõ
  "chưa đo" cho tới khi có runner. (4) Không thêm dependency mới cho cross-check.

- **VD-105** **`Host` mở rộng: `NetHost` + quyền của phiên (ST-63..ST-66)**: (1) Thêm `NetHost` cạnh `FsHost` trong
  `vis-interp/src/builtin.rs` (handle = `str` mờ; `connect/listen/local_addr/accept/read/write/close`), `Host::net()`
  default `None` ⇒ **`VS0604`** cả hai đường. (2) **Giao thức HTTP nằm trong `vis-interp`** (`net.rs`) ⇒ interp ↔ VM cùng
  byte, codegen **không** sửa (`std.*` hạ theo tên). (3) **Chỉ loopback** trong vòng này; host khác ⇒ `Err`. (4) **Không**
  mã lỗi mới: lỗi mạng = `Err(str)`, không host = `VS0604`, sai kiểu = `VS04xx`. (5) Tham số tuỳ chọn **tách tên hàm**
  (`connect_timeout`, `get_timeout`) vì builtin **bỏ** đối số có tên. (6) **R1 trước, code sau**: `interp.rs` đang
  **1.486** dòng ⇒ task **ST-63 bắt đầu** bằng việc **tách** (`impl Host` đã ở `host.rs`; tách tiếp `eval_call`/phần
  runtime sang `interp/goi.rs` hoặc tương đương) rồi mới thêm net. (7) Ca net/http nằm ở `crates/vis-cli/tests/net.rs`
  + `http.rs` (server loopback thật trong test Rust), **không** đặt vào `test/sem` (sem không có host net) ⇒ con số
  sem **441** giữ nguyên; ghi rõ trong báo cáo.

- **VD-106** **`cap[...]` là kiểu thật, cưỡng chế **chỉ khi khai báo** (ST-72/ST-72b)**: (1) `cap[Dir|File|Net|Proc|Env|Term]` là
  kiểu **dựng sẵn** (`TypeRef{"cap",[X]}`), **không** người dùng tạo được giá trị — chỉ nhận qua **tham số `main`**; truyền
  qua tham số hàm/record bình thường. (2) `@ p"…"`/`@ "host"` + `(write)` **giữ** ở CST; **`vis run --dry`** in danh sách
  quyền khai báo (dạng `tên: cap[Dir] path (write)`) rồi thoát 0, **không** chạy chương trình. (3) **Cưỡng chế** nằm ở
  `HostFs`/`HostNet` (CLI): thao tác ngoài tập quyền ⇒ **`VS0604`** nêu **quyền thiếu** + đường dẫn/host; `write` cần cờ
  `(write)`. (4) **Chương trình KHÔNG khai `cap` ⇒ không hạn chế** (tương thích 854 test hiện có) — ghi rõ trong tài liệu và
  báo cáo; siết mặc định là **nợ** (phải có `--strict-caps` + cập nhật toàn bộ fixture). (5) `--grant <tên>=<đường dẫn>`
  **ghi đè** đường dẫn của quyền đã khai (không thêm quyền mới). (6) Landlock thật: **chỉ** Linux, crate ngoài `viscript/`;
  trên macOS **không** giả vờ — test `cfg(target_os="linux")`, báo cáo ghi "chưa đo trên macOS".

- **VD-107** **Landlock: chỉ **probe** + khung, **không** giả vờ (ST-73)**: (1) Máy phát triển là macOS ⇒ **không** có Landlock để
  đo; luật hiện hành **cấm `unsafe`** (R2, danh sách crate đóng **rỗng**) và **cấm thêm crate ngoài allowlist** ⇒ **không** thể
  gọi syscall Landlock trong vòng này. (2) Vì vậy ST-73 làm: **`vis run --sandbox-check`** — probe **chỉ đọc**
  (`/sys/kernel/security/lsm` trên Linux; macOS/khác ⇒ "không có"), in kết quả + exit 0, và khung crate `vis-host-linux`
  **ngoài** `viscript/` với `#[cfg(target_os = "linux")]` **chỉ đọc tệp** (không syscall), bằng chứng là
  `cargo check --target x86_64-unknown-linux-gnu` **xanh**. (3) **Cấm** viết trong báo cáo/tài liệu rằng "đã cưỡng chế bằng
  Landlock" — chỉ được nói "**cưỡng chế ở tầng runtime** (ST-72b) + **probe** Landlock (ST-73)". (4) Muốn Landlock thật: cần
  Agent 0 duyệt **một** trong hai: (a) cho phép crate `landlock` (MIT/Apache) trong allowlist, hoặc (b) cho phép `unsafe`
  **đóng** trong `vis-host-linux` + case âm; cả hai phải kèm runner Linux để **đo** — ghi thành **nợ có điều kiện**.

- **VD-108** **Vòng 12 — "nhất quán hoá" tính năng mới (ST-77..ST-82)**: (1) Sau vòng 11, các tính năng VS2 **chạy được** nhưng chưa
  **nhất quán** với phần còn lại của công cụ ⇒ vòng 12 đóng 5 lỗ: **`vis fmt`** cho cú pháp mới (`cap[...]`, `@`/`(write)`,
  `async`, `spawn`, `chan[T]`, `use pkg.*`) — **lossless + idempotent**; **`vis explain`/sổ mã** đủ **3 ngôn ngữ** cho
  `VS0504/0505/0506` + `--list` phủ **32 mã**; **`vis test`** chạy được khối `test` có `async`/`await`/`chan`; **LSP
  module-aware** (giải `use ./x` + `std.*` để **hết chẩn đoán giả**); **`--strict-caps`** (tuỳ chọn, **mặc định không đổi**).
  (2) **Không** đụng ngữ nghĩa đã khoá; mỗi task ≥6 ca, giữ **874 test/447 sem/parity 398/398/diff 0 lệch** (số sẽ tăng).
  (3) Vòng 12 **không** hứa: Landlock thật, runner Windows/Linux/wasm, TLS — đó là **điều kiện môi trường**, ghi ở R12 §5.
