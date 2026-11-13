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
