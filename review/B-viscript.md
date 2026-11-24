# B-report: Review nội bộ thiết kế ViScript/ViShell (trước khi giao Agent 2)

- **Tệp được review:** `src/ViScript-thiet-ke.md` — 984 dòng, tiếng Việt. Chỉ đọc; báo cáo này **không sửa** tệp nguồn.
- **Tệp tham chiếu:** `src/ViFastOs-v2-thiet-ke.md` (chỉ tra §7.3, §11, §13).
- **Phương pháp:** đọc toàn văn; kiểm cơ học bằng script (31 khối mã, thụt lề, tab, tab-stop, ký hiệu, tiêu đề so với mục lục, số mục); đối chiếu từng dòng §21 (Q1–Q8) với thân tài liệu; đối chiếu Phụ lục A với §2, §4, §5, §9; đối chiếu `#| deps`, `@`, `(write)`, `...rest` với ngữ pháp; tra chéo tài liệu OS.
- **Quy ước dẫn chứng:** `Lnnn` = dòng trong `ViScript-thiet-ke.md`; `OS:nnn` = dòng trong `ViFastOs-v2-thiet-ke.md`.

---

## 1. Phán quyết

**Chưa được giao Agent 2.** Tài liệu đủ để định hướng sản phẩm, **không đủ để thi công**: (a) Phụ lục A không sinh ra được chính các ví dụ trong tài liệu (thiếu ~30 luật, kể cả luật cho `pattern`, `if`/`match` biểu thức, khối một dòng), (b) §9.3 yêu cầu **parse phụ thuộc chữ ký lệnh** trong khi kiến trúc §14.1/§14.2 đặt parser trước bộ kiểm kiểu và VT06 thi công ở vòng chưa có bộ kiểm kiểu, (c) đặc tả từ vựng (literal, escape, Unicode, ký hiệu chế độ lệnh) thiếu ~15 quyết định mà lexer buộc phải đoán. Ba chỗ này nằm đúng trên đường tới hạn của VT01–VT10, nên sửa sau sẽ phải làm lại lexer/parser/fmt.

---

## 2. Tổng hợp mức độ

| Mức | Số lượng | Bản chất |
|---|---|---|
| BLOCKER | 3 | Không thể thi công nếu không quyết định lại (kiến trúc parse chế độ lệnh; ngữ pháp Phụ lục A; đặc tả từ vựng) |
| MAJOR | 21 | Mâu thuẫn nội bộ hoặc thiếu đặc tả ở mức ngữ nghĩa/kiểu; phải sửa trước vòng 2, nhiều cái trước VT04–VT06 |
| MINOR | 16 | Thiếu sót cục bộ, không chặn nhưng sẽ thành câu hỏi BLOCKED của Agent 2 |
| NIT | 7 | Hình thức, nhãn, ví dụ minh hoạ không hợp lệ |

---

## 3. Đã kiểm chứng — **KHÔNG phải lỗi** (để tránh sửa nhầm)

Các điểm sau bị nghi oan hoặc dễ bị "sửa" sai; kiểm tra xong thấy tài liệu **nhất quán**:

| Điểm | Kết quả kiểm | Dẫn chứng |
|---|---|---|
| **Q1 từ khoá tiếng Anh** | Đúng toàn bộ. Tiếng Việt chỉ có trong chú thích tài liệu (`##`) và định danh, không có từ khoá Việt nào | L96, L100; `## Liệt kê tệp` L426; `fn tính_tổng` L979–L983 |
| **Q2 `int / int -> int`** | Đúng, không có phản ví dụ nào trong 984 dòng | L202–L204 |
| **Q3 thụt lề 4 dấu cách, cấm tab** | Đúng về mặt cơ học: 31 khối mã, **không có ký tự tab**, mọi mức thụt lề là bội số của 4 (sâu nhất 8), không có khoảng trắng cuối dòng. Ngoại lệ duy nhất là sơ đồ cây ASCII (L627–L635, L688–L691) và phần tiếp dòng của EBNF (L917–L919) — không phải mã | kiểm script trên 31 fence |
| **Q5 giấy phép** | Nhất quán 6 chỗ, không mâu thuẫn | L5, L617, L658, L843, L867, L886 |
| **Q6 `decimal` là thư viện** | Đúng: chỉ xuất hiện 2 lần, không có trong bảng kiểu (L141–L153) và bảng std (L598–L615) | L155, L868 |
| **Q7 toán tử bit là hàm** | Đúng: chỉ có `bit_and/bit_or/bit_xor/shl/shr`; **không có** `&`, `^`, `<<`, `>>` ở bất kỳ đâu (grep xác nhận; `&` chỉ xuất hiện ở L437/L721 nghĩa job control) | L261 |
| **Mục lục ↔ thân** | Khớp: 24 mục 0–23 + 2 phụ lục, đủ, đúng thứ tự; không mục nào thiếu/thừa | L13–L38 vs L42–L939 |
| **Số mục** | Không nhảy: 0–23 liên tục; các mục con 2.x…20.x liên tục; không bỏ cấp tiêu đề | grep `^#{1,4} ` |
| **Tra chéo tài liệu OS** | Đúng: `OS` §7.3 tồn tại (OS:566), §11 tồn tại, §11.7 tồn tại (OS:901) | L4, L832, L852 |
| **Hai "tiêu đề lạc" bị nghi** | **KHÔNG phải tiêu đề.** `## Liệt kê tệp` (L426) nằm trong fence L425–L429; `## Tìm 20 tệp lớn nhất…` (L951) nằm trong fence L941–L968. Đây là **chú thích tài liệu `##`** hợp lệ theo §2.3 (L111), gắn với khai báo kế tiếp. Markdown render đúng, không phá mục lục | L425–L429, L941–L968 |

> Hệ quả: đừng "sửa" hai dòng L426/L951 thành `###`; sẽ biến chú thích tài liệu thành tiêu đề và phá `vis doc`.

---

## 4. Kiểm tra từng dòng §21 (Q1–Q8)

| Mã | Chốt (L860–L869) | Thân tài liệu có tuân? | Kết luận |
|---|---|---|---|
| Q1 | Từ khoá tiếng Anh | Có (L96, L100) | **ĐẠT**. Nhưng danh sách từ khoá thiếu `const` (được dùng ở L908) và `as` là từ khoá chết — xem MIN-02 |
| Q2 | `int / int -> int`, lint chia literal | Có (L202–L204) | **ĐẠT** ở phạm vi số vô hướng; `%`, `**`, `size/duration/percent`, float `/0` không có quy tắc — xem MAJ-10 |
| Q3 | 4 dấu cách, cấm tab | Có, kiểm cơ học toàn bộ ví dụ | **ĐẠT**. Nhưng §2.2 nói "`vis fmt` tự sửa" tab trong khi tab là lỗi từ vựng — MIN-03 |
| Q4 | `ViScript`/`.vis`/`vis`, `ViShell`/`.vish`/`vish` | Có (L95, L865) | **ĐẠT**. Lệch nhẹ: thư mục cấu hình `~/.config/vishell/` (L706) không theo tên lệnh `vish` — MIN-15 |
| Q5 | `xs[i]` panic, `xs.get(i) -> T?` | Có ở mức tuyên bố (L356, L866) | **ĐẠT nhưng thiếu**: `map[k]`, `str[i]`, chỉ số âm, `.last()/first()` trả gì, và `get` trùng tên lệnh shell `get` (L481, L614) — MAJ-17... (xem MAJ-06, MIN-05) |
| Q6 | MIT OR Apache-2.0 | Có, 6 chỗ nhất quán | **ĐẠT** |
| Q7 | `decimal` ngoài lõi | Có | **ĐẠT** |
| Q8 | Bit là hàm, `\|` chỉ dùng cho pipe | **KHÔNG** | **VI PHẠM**: (a) L248 dùng `\|` cho or-pattern trong **script**; (b) L869 nói "giải phóng `\|` cho pipe **ở mọi chế độ**" nhưng script dùng `\|>` (L261, L921) và §9.4 nói script **không dùng** `\|` (L452) — MAJ-01 |

---

## 5. BLOCKER

### BLK-01 — §9.3 yêu cầu parse phụ thuộc kiểu; kiến trúc §14 và VT06 không cho phép

- **Hiện trạng:** L443: "Tham số được hiểu theo **chữ ký lệnh**. Từ trần `./photos` vào tham số `path` thành `path`; `5mb` vào `size` thành `size`; `*.txt` vào tham số `list[path]`/`glob` được mở rộng; vào `str` thành chuỗi." L446: tham số kiểu hàm được "phân tích ở chế độ biểu thức, có biến ngầm `it` và **viết tắt trường**" — muốn biết `size` trong `where size > 5mb` là trường của `FileEntry` thì phải biết kiểu.
- **Mâu thuẫn:** L416–L418 nói parser dịch sang "cùng cây cú pháp"; L625–L631 xếp thứ tự lexer → parser → **phân giải tên + kiểm kiểu**; L644 `vis-types` phụ thuộc `vis-syntax` (không thể ngược); L848 giao VT06 "hạ về cùng AST" ở vòng 1, trong khi L854 hoãn bộ kiểm kiểu sang vòng 2. Ngoài ra L439 phân biệt `tên cmd` với `chương trình ngoài` — cũng cần phân giải tên.
- **Hệ quả:** VT06 không thể đạt tiêu chí "Hai chế độ ra cùng AST" nếu không có bảng chữ ký; Agent 2 sẽ hoặc (i) hard-code chữ ký std (đoán), hoặc (ii) sinh node "Word" chưa định kiểu rồi không ai hạ tiếp, hoặc (iii) nhét `vis-types` vào parser (phá DAG crate ở L644).
- **Đề nghị:** chốt mô hình hai pha và viết thành bảng "luật → pha":
  1. `vis-syntax` sinh `CmdCST` gồm `Word` chưa định kiểu (giữ nguyên văn để báo lỗi);
  2. pha mới `cmd_lower` **trong `vis-types`** dùng bảng chữ ký để quyết: literal `5mb`→`size`, glob theo kiểu tham số, `it`+viết tắt trường, `tên cmd` vs chương trình ngoài, `--flag` binding;
  3. sửa L416–L418 và L780 thành "cùng **HIR**", không phải "cùng AST";
  4. sửa tiêu chí VT06 thành "CST + golden test `bảng dòng lệnh → CmdCST`", và thêm task riêng cho `cmd_lower` (vòng 2, sau `vis-types`).
- **Vì sao BLOCKER:** sai ở đây làm lại cả VT05, VT06, VT07, VT09 và API của `vish-edit` (L654 dùng `types`) — tức là toàn bộ vòng 1.

### BLK-02 — Phụ lục A không sinh ra được chính các ví dụ của tài liệu

- **Hiện trạng:** Phụ lục A (L906–L935) có 12 luật; thân tài liệu dùng thêm ~30 ký hiệu/luật không tồn tại.
- **Dẫn chứng cụ thể (mỗi cái là một ví dụ trong tài liệu không parse được):**
  1. **Khối một dòng:** L915 `block := stmt | NEWLINE INDENT stmt+ DEDENT` — không có dạng một dòng, trong khi L106 cho phép `if x > 0: return x` và L223–L225, L227, L272 dùng đúng dạng đó.
  2. **`if`/`match` là biểu thức:** `primary` (L933) gọi `if_expr | match_expr` nhưng **không định nghĩa**; `let label = if ok: "OK" else: "FAIL"` (L227) và `if loud: s.upper() else: s` (L272) không suy ra được.
  3. **`pattern`:** L916 dùng `pattern` nhưng **không có luật `pattern` nào**. Kéo theo: `let (a,b)` (L216), `let {name, age}` (L217), match arm L240–L244, or-pattern `|` (L248), `[first, ..rest]`, `{name, age: 18..}`, guard `if`, `Circle(r)` — tất cả vô nghĩa với parser.
  4. **`ref[T]` không phải kiểu hợp lệ:** L934 `type := IDENT [...]` trong khi `ref` là **từ khoá** (L100). Ví dụ L152, L289, L389 dùng `ref[T]` → parser không nhận.
  5. **`()` (unit) không phải kiểu hợp lệ:** L934 `'(' types ')'` với `types := type (',' type)*` đòi ≥1 kiểu; L148 dùng `()` làm kiểu (và L360, L539, L952 trong `Result[(), Error]`).
  6. **`json.parse[list[User]](text)` (L300) vs `xs[i]`:** L932 có cả `'[' expr ']'` và `'[' types ']'`, không có luật phân biệt; `xs[i]` khớp cả hai (vì `type := IDENT`). Parser buộc phải đoán hoặc cần lookahead không đặc tả.
  7. **`fn(x:int) -> int:` như biểu thức closure** (L284–L287): `primary` không có dạng `fn` (L933), và `lambda` được dùng ở L920 nhưng không định nghĩa.
  8. **Tham số có modifier:** L912 chỉ có `['--'] IDENT ':' type ['=' expr] | ['var'] 'self'` — thiếu `@ "./data"`, `(write)` (L539, L952), `...rest: str` (L468).
  9. **Item thiếu luật:** `use` (L908, dùng L399), `impl_decl`/`trait_decl` (dùng L305–L319), `const_decl` (chỉ có trong `item`), `test_decl` (L965), `extern_cmd` (L468).
  10. **`elif`** (L224) không có trong bất kỳ luật nào.
  11. **`assign`** được dùng ở L918 nhưng không định nghĩa → `+=` (L215) không có ngữ nghĩa cú pháp.
  12. **`with`:** L932 có `'with' field_updates` nhưng `field_updates` không định nghĩa; ví dụ L176 dùng `u with age: 21`.
  13. **Token `NEWLINE INDENT DEDENT`** chưa từng được định nghĩa ở đâu (kể cả §2.2).
- **Đề nghị:** viết lại Phụ lục A thành EBNF **đầy đủ, tự chứa** (khoảng 60–70 luật, gồm `pattern`, `if_expr`, `match_expr`, `lambda`, `args`, `field_updates`, `types` có `()`/tuple, `type` nhận `ref`/`cap`, `item` đủ 10 dạng), kèm bộ test "mọi khối mã trong tài liệu parse được". Đây chính là nội dung VT02 nhưng phải **xong và được duyệt trước VT05**.
- **Vì sao BLOCKER:** VT05/VT06/VT07/VT09 đều lấy Phụ lục A làm đặc tả; hiện tại nó không đặc tả gì đủ để viết parser.

### BLK-03 — Đặc tả từ vựng thiếu, lexer buộc phải đoán (VT04)

- **Hiện trạng:** §2.4 (L113–L133) liệt kê literal bằng ví dụ, không có luật token; §9.4 (L448–L458) chỉ giải 4 ký hiệu.
- **Những chỗ không thể viết lexer mà không đoán:**
  1. **`..` vs float:** `3.14` (L118) và `0..10`/`0..=10` (L229–L231). Chưa có luật (cấm `1.`, bắt buộc chữ số sau `.`, maximal munch?). Cùng chỗ: `1e-9` (L118), `1_000_000` (L117) — vị trí `_` hợp lệ?
  2. **`%`:** `50%` là literal `percent` (L125), `%` là modulo (L261, L929), `cmd %1` là job spec (§15 L721). Chưa có luật phân biệt; `50%3` là gì?
  3. **Đơn vị `size`/`duration`:** `5kb/5kib/2mb/1gib` (L123), `250ms/2h30m/1d` (L124). Danh sách đơn vị **không có**; `2h30m` là một token hay hai? `5b`? `1w`/`1y`? `5 kb` (có cách) là gì?
  4. **`p"..."` (L126)** trùng hình thức với định danh `p` + chuỗi; `b"..."` (L122), `r"..."` và `"""` (L120) — chưa có bảng tiền tố.
  5. **Escape:** tài liệu **không định nghĩa một escape nào** (`\n`, `\t`, `\\`, `\"`, `\{`, `\u{...}`, `\xNN`), nhưng L120 dùng `r"C:\raw"` (ngụ ý `\` có nghĩa trong chuỗi thường) và L122 dùng `b"\x00\x01"`.
  6. **Nội suy:** L133 định nghĩa `"{expr}"`, `"{x:>8}"`, `{{`. Thiếu: `}}`? nội suy trong `r"..."`/`"""`? cú pháp format spec đầy đủ? `{` trong `{expr}` lồng nhau? chuỗi thường có luôn nội suy (thì viết `{` thật thế nào — chỉ `{{`)?
  7. **`'ố'` (L121):** `char` là scalar value; nếu nguồn ở dạng NFD (`o` + U+0302 + U+0301) thì literal là 3 scalar → lỗi "char literal quá dài" hay lexer phải NFC hoá? L97 chỉ chuẩn hoá **định danh**, không nói literal.
  8. **`#` (L110) cắt tới cuối dòng** → `http get https://x/y#frag` (dạng L484) mất phần sau `#` **âm thầm**. Chưa có luật.
  9. **`?`/`?.`/`??`:** `?` postfix (L932), `?.` (L932), `??` (L926) và glob `?` (chưa có). Chưa có luật maximal munch cho `x??y` (`x? ? y` vs `x ?? y`).
  10. **`|>` vs `|`:** L921 và L437; cần luật token dài nhất, và luật cấm `|` trong script (nếu chốt như Q8) — hiện L248 vi phạm.
  11. **Từ chối số nguyên tràn literal** (`0xFFFFFFFFFFFFFFFF`, `-9223372036854775808`) — L143 nói tràn runtime là panic, không nói literal.
  12. **`set{1,2}` (L129)** vs `counter.set(...)` (L386): `set` là định danh hay tiền tố literal? Luật?
  13. **Cột chẩn đoán:** sơ đồ caret (L688–L690) trên văn bản tiếng Việt UTF-8, cộng LSP (L652): cột tính bằng byte, scalar, grapheme hay UTF-16? Chưa có → VT03 không thể khớp snapshot.
  14. **Chế độ lệnh:** từ trần chứa `#`, `&`, `?`, `%`, `$`, `[`, `(`, `;`, `=`, `~`, `!` — chưa có luật "ký tự nào được phép trong `word`" và cách thoát (L443 nói "không có trích dẫn kiểu Bash" nhưng L440 vẫn có `string`).
- **Đề nghị:** viết `SPEC-lex.md` 2–3 trang: bảng token (tên, regex, ví dụ, ví dụ sai), luật maximal munch, bảng escape, bảng đơn vị, luật `%`/`..`/`?`, luật NFC cho literal, đơn vị cột. Kèm golden test token (`tests/lex/*.tok`) trước khi VT04 bắt đầu.
- **Vì sao BLOCKER:** mọi lựa chọn ở đây là **không tương thích ngược** (đổi sau phải sửa mọi test + `vis fmt`), và VT04/VT06 là task đầu tiên của vòng 1.

---

## 6. MAJOR

**MAJ-01 — Q8 tự mâu thuẫn; `|` vừa là or-pattern vừa "không dùng".**
L248: pattern gồm "`|`" (or-pattern) trong **script**; L452: script "Không dùng" `|`; L869: `|` được giải phóng "cho pipe **ở mọi chế độ**" nhưng pipe script là `|>` (L261, L921). Ba câu không thể cùng đúng.
*Đề nghị:* chọn (i) or-pattern dùng `or` (`Circle(r) or Rect(w,h) =>`), giữ `|>` cho pipe script, sửa L869 thành "giải phóng `|` khỏi bit-ops để dùng cho pipe chế độ lệnh và or-pattern"; hoặc (ii) `|` = or-pattern, `|>` = pipe, và **bỏ** câu "pipe ở mọi chế độ". Phải chốt trước khi viết `pattern` (BLK-02).

**MAJ-02 — `from json` mâu thuẫn với "không `any`".**
L79–L80: dữ liệu chưa rõ cấu trúc có kiểu `Json` và "phải chuyển đổi tường minh"; L151: `Json` là enum động; L463: `| from json`; L481: `open users.json | where age >= 18 | get name` — truy cập trường trên `Json` **không chuyển đổi**. Nếu `from json` trả `Json` thì `where age >= 18` là truy cập động (vi phạm §1.2); nếu trả kiểu tĩnh thì schema lấy từ đâu? `json.parse[T]` có (L300) nhưng `from json` không có `[T]`.
*Đề nghị:* bắt buộc `from json[User]`; `where` trên `Json` là lỗi kiểu kèm gợi ý; hoặc thêm `from json --schema`. Ghi rõ trong §9.5.

**MAJ-03 — "Không hàm toàn cục nào chạm hệ thống" bị chính ví dụ phá.**
L532 khẳng định tuyệt đối; nhưng `print(...)` là hàm toàn cục ghi ra terminal (L344, L962, L691), `sh "..."` chạy `/bin/sh` (L723) không cần `cap[Proc]`. Thêm nữa L757 ưu tiên plugin **liên kết tĩnh** trên ViFastOs trong khi L562 nói "chế độ hạn chế quyền, plugin native bị tắt" — plugin tĩnh không thể tắt lúc chạy.
*Đề nghị:* `print` nhận `cap[Term]` hoặc khai báo là ngoại lệ "host effect" có kiểm soát; `sh` phải là `cap[Proc]` + cờ `--allow-sh`; sửa L562 thành "plugin động bị tắt; plugin tĩnh chỉ trong bản `vis` do người dùng tự build".

**MAJ-04 — `.vish` "toàn bộ ở chế độ lệnh" không chứa được `let`.**
L95: `.vish` = tệp toàn bộ ở chế độ lệnh; L436–L441: ngữ pháp chế độ lệnh chỉ có `pipeline`; nhưng L479 (`> let big = ls ./photos | where size > 5mb`) và L717 ("`let` trong shell sống hết phiên") cho thấy shell phải có `let`. `.vish` hiện không thể là chương trình, cũng không rõ `let` hạ về AST nào.
*Đề nghị:* `ShellFile := shell_item*` với `shell_item := use | let | fn | cmd | pipeline`; REPL nhận `statement | pipeline`. Nêu rõ `let big = <pipeline>` có kiểu gì (`stream[FileEntry]`?).

**MAJ-05 — Không có bảng ưu tiên; Phụ lục A và §4.5 không khớp.**
L261 chỉ liệt kê nhóm ("Số học `+ - * / % **`") — không phải bảng ưu tiên; L816 lại yêu cầu SPEC phải có "bảng ưu tiên toán tử". Appendix A ngầm định thứ tự nhưng: `**` nằm dưới `mul` và kết hợp phải (L930–L931) → **khác** với việc §4.5 xếp `**` cùng nhóm `* / %`; `??` (L926) đặt **giữa** so sánh và range — không có trong §4.5 và khác C#/Swift (`a ?? b == c` ở đây là `(a ?? b) == c`); `..`/`..=`; `with`; `?` postfix; `not` (L924 đặt **dưới** so sánh, tức `not a == b` = `not (a == b)`) — không câu nào nói.
*Đề nghị:* bảng 10–12 mức trong §4.5 + golden test biểu thức (`-2 ** 2`, `2 ** 3 ** 2`, `a ?? b == c`, `0..n+1`, `x? |> f`, `not a and b`).

**MAJ-06 — Optional/Result thiếu luật suy kiểu.**
L148 `T?`; L253–L256 (`??`, `?.`, `if let`); L295 `xs[0]` trả `T` cho hàm khai `T?` (cần **thăng cấp ngầm `T → T?`** — chưa hề nói); `T??` có làm phẳng không; `?` chỉ định nghĩa cho `Result` (L338) nhưng L333 `dir.read_text(...)?` là optional; `.last()` (L254) trả gì; `none` suy kiểu thế nào; `match` trên `T?`/`Result` dùng pattern gì (L248 không có `none`/`ok`/`err`) — thiếu luật pattern thì không thể "xét hết" (L247).
*Đề nghị:* một trang luật Option/Result: thăng cấp, làm phẳng, `?` cho cả hai, pattern `none`, `ok(v)`, `err(e)`, `.get/.last/.first` trả `T?`, `if let` = sugar của match một nhánh.

**MAJ-07 — Exhaustiveness chưa đủ luật để cài.**
L241 có guard (`Circle(r) if r > 100.0`); L247 đòi "xét hết" và "liệt kê nhánh thiếu"; L248 có range pattern (`0..10`), or-pattern, `..rest`; L665 dùng "ma trận pattern". Chưa nói: nhánh có guard **không** tính vào coverage (bắt buộc, nếu không `Circle(r) if r>100` sẽ được coi là phủ `Circle`); kiểu vô hạn (`int`, `str`, `float`) có bắt buộc `_`; range pattern có được dùng để chứng minh phủ `int`; `T?`/`Result` phủ `none`/`ok`/`err`.
*Đề nghị:* viết luật "usefulness" như Rust nhưng giới hạn: kiểu vô hạn **bắt buộc** `_`; in danh sách nhánh thiếu theo thứ tự khai báo.

**MAJ-08 — Mô hình lỗi không khép kín.**
Ba tên lỗi: `ParseError` (L198), `Error` (L338), `VisError` (L752). L338 nói `?` cần `Into` nhưng **không có cú pháp** `impl Into[Error] for MyErr`, không có `derives Error`, không nói `Error` là enum hay trait object, không nói `Error` có `source()`/vị trí (L338 nói "có vị trí" nhưng kiểu vị trí nào?). `?` trên `T?` (L333) không có luật. Mã thoát chỉ có 1 (L360) trong khi job/`complete` cần mã (L464, L721).
*Đề nghị:* `trait Error: Show { message, source, span }`; `Error` là kiểu xoá (boxed dyn) + `impl[T: Error] Into[Error] for T`; bảng mã thoát (0/1/2/101/126/127…); `?` cho optional chỉ trong hàm trả `T?`.

**MAJ-09 — Thiếu luật về scope/khả biến/thứ tự đánh giá.**
Không có chỗ nào nói **shadowing** (grep "shadow": 0 kết quả): `let x = 1; let x = 2` lỗi hay che? Tham số closure che biến ngoài? Arm `match` binding che? L277 "tham số bất biến" vs L313 `var self` — `var` có được dùng cho tham số thường, cho trường bản ghi (`type` L161–L163 không có `var`)? `let xs = [1]; xs.push(2)` (L373 chỉ cho `var b`) có hợp lệ? Truyền tham số có CoW (L368–L376) nhưng **chưa nói** ranh giới gọi hàm/trả về có sao chép ngữ nghĩa; `and`/`or` có short-circuit không (chưa từng nói); thứ tự đánh giá toán hạng/đối số/field của literal (L958) không có — trong khi L86 tuyên bố "tất định" và L784 chạy vi sai interpreter/VM, nên thứ tự đánh giá phải cố định.
*Đề nghị:* thêm §4.6 "Scope, khả biến, thứ tự đánh giá" (~15 dòng) làm nguồn duy nhất cho `vis-types`, `vis-fmt`, `vis-lsp`.

**MAJ-10 — Số học/float chưa đủ để viết VM và oracle vi sai.**
L143 (`int` i64, tràn panic) và L202–L204 chỉ nói `/`. Thiếu: dấu của `%` với số âm; `int % 0`; `float / 0.0` (inf? panic?); NaN (so sánh, `Ord`, `Hash`, `derives Eq` trên kiểu có `float` — L174; `map[float,T]`); `**` cho `int` (số mũ âm? tràn? `0**0`); literal tràn; `int.trunc` với NaN/ngoài miền → panic?; `size`/`duration`/`percent` (dấu, độ rộng, cộng/trừ/nhân/chia, so sánh chéo `percent` vs `float` ở L483).
*Đề nghị:* bảng ngữ nghĩa số 1 trang; đặc biệt chốt NaN để L784 (vi sai) có oracle.

**MAJ-11 — Chuỗi/ký tự và tiếng Việt chưa có ngữ nghĩa lõi.**
L121 `'ố'`; L145 `str` UTF-8 bất biến; L272 `s.upper()`; L601 `text` có grapheme/chuẩn hoá; L97 chỉ NFC cho **định danh**. Thiếu: `s[i]` có tồn tại không và đơn vị (byte/codepoint/grapheme); `len()` đếm gì; so sánh `"ố" == "ố"` giữa NFC và NFD; `upper/lower` cho `đ/Đ/ố` (locale?); `char` là scalar (thì literal NFD là lỗi) hay grapheme. Với ngôn ngữ lấy tiếng Việt làm định danh, đây là quyết định hạng nhất, không phải chi tiết.
*Đề nghị:* chốt "index theo grapheme cluster, `len()` = grapheme, so sánh theo codepoint sau NFC" hoặc ngược lại; viết vào §2.4/§13 và đồng bộ với luật token ở BLK-03.

**MAJ-12 — Chuỗi nội suy/escape chưa đặc tả (xem BLK-03 mục 5–6).**
Ngoài phần lexer: chưa nói `{"{x}"}` in `none` ra sao, `{x:>8}` áp cho `T?`/record, `print("{a}{b}")` với `b` là `Result` — nếu `Show` chưa có thì nội suy phụ thuộc trait nào (L174 có `Show`, nhưng chưa nói mọi kiểu đều `Show`).

**MAJ-13 — Cú pháp gọi hàm có tham số tên chưa tồn tại.**
L275 nói tham số `--x` "chỉ truyền theo tên"; L956/L961 gọi `sort_by(e => e.size, --desc)`, `json.to_string(files, --pretty)`; nhưng `args` không có luật (BLK-02 mục 7) và **không ví dụ nào** gọi `greet` (L270) để thấy cú pháp. Thêm: L912 `param` thiếu `@`, `(write)`, `...rest`.
*Đề nghị:* `call_args := expr (',' expr)* (',' '--' IDENT (':' expr)?)*`, thứ tự tham số (vị trí trước, tên sau), cấm trùng, giá trị mặc định; ví dụ gọi đầy đủ.

**MAJ-14 — Chế độ lệnh không có cơ chế trích dẫn/thoát; `$` không phải env.**
L443 nói "không có tách từ và trích dẫn kiểu Bash"; L440 chỉ có `'$' ident` (biến), không có `$HOME`. Vậy: tên tệp có dấu cách, có `|`, `#`, `(`, `"` truyền thế nào? Chuỗi `"..."` trong chế độ lệnh có nội suy không? `$` trong URL/regex? Đây là câu hỏi đầu tiên người dùng shell sẽ hỏi.
*Đề nghị:* luật rõ: `word` = run of non-metachar (liệt kê metachar); `"..."` = chuỗi có nội suy; `'...'` = chuỗi thô; `\` chỉ trong chuỗi; `$name` tra biến phiên, `$env.NAME` tra môi trường (hoặc `ctx.env`) — và ghi vào bảng 9.4.

**MAJ-15 — §9.5 mơ hồ ở các điểm sẽ thành bug.**
(a) "Mã thoát khác 0 thành lỗi pipeline" (L464) + "`take 10` dừng nguồn sớm" (L462) → chương trình ngoài bị SIGPIPE (141) sẽ thành **lỗi** hay bình thường? (b) "`| complete` để nhận `{stdout, stderr, code}`" — `complete` có chặn lỗi pipeline không? (c) `from json` phải đọc hết mảng (buffer) — xung đột với "lười, kéo, backpressure" (L462). (d) `from csv` trả kiểu gì (list/map/stream gì)? (e) `to csv` lấy thứ tự cột từ đâu (kiểu bản ghi? thứ tự chèn?). (f) `get name` khi thiếu trường → `T?`/lỗi/bỏ dòng? (g) `Render` (L473) in `T?`, `Json`, bản ghi lồng thế nào.
*Đề nghị:* một bảng "hành vi lỗi/kết thúc của pipeline" + chữ ký `from/to` với kiểu vào-ra.

**MAJ-16 — Bề mặt API của capability/std không tồn tại.**
`FileEntry` (L427), `Ctx` (L432), `GitSub` (L468), `Meta`, `VPath` — không định nghĩa. `cap[Dir]` có method gì (`read_text`, `write_text`, `walk`, `subdir`, glob) — L532/L333/L961 dùng nhưng không liệt kê. L427 `path = "."` là **chuyển ngầm `str` → `path`**, trái với L198 ("không chuyển đổi ngầm… ngoại lệ duy nhất literal nguyên ở vị trí `float`"). Hệ quả trực tiếp: golden test của §9 (`where size > 5mb`) cần `FileEntry.size`, tức VT06 phải hard-code một API không có trong tài liệu.
*Đề nghị:* phụ lục "chữ ký std" cho `FileEntry`, `Ctx`, `cap[Dir]/[File]/[Net]/[Proc]/[Env]/[Term]`, và bỏ/cho phép tường minh chuyển `str→path` cho literal (hoặc `p"."`).

**MAJ-17 — async thiếu mô hình kiểu.**
L503–L511: `async fn`, `await`, `scope`, `s.spawn(net.get_text(u))` — `spawn` nhận **giá trị future** hay **biểu thức**? Kiểu future tên gì (không có trong bảng kiểu L141–L153)? Không có cú pháp `async { ... }`. `await` trên không-future? `timeout(5s, fut)`, `select` (L512) chữ ký? `net.get_text` có phải async (bảng §13 không đánh dấu async cho hàm nào)? Panic trong `scope` có huỷ anh em không (L511 chỉ nói Ctrl-C/timeout)?
*Đề nghị:* `Task[T]` vào bảng kiểu; `spawn` nhận `Task[T]` (hoặc `async {}`), `await: Task[T] -> T`; đánh dấu async trong bảng std; luật panic trong scope.

**MAJ-18 — Phân giải tên/không gian tên chưa có; nhiều hàm không thuộc module nào.**
`int.parse`, `int.trunc`, `int.div_floor` (L198, L203) dùng **tên kiểu như namespace**; `bit_and`/`shl` (L261) không thuộc module nào trong §13; `parallel`/`thread` (L520) không có trong bảng std (L598–L615); `json` nằm trong `fmt` nhưng dùng trần (L300, L961); `http` nằm trong `net` (L607) nhưng dùng như lệnh (L484); `filter/sum/take/collect` vừa là method (L184) vừa là hàm trần (L955–L959). Thêm: `test` là **từ khoá** (L100) nhưng §13 có module `test` (L612) → `use std.test` không lex được.
*Đề nghị:* bảng "namespace → item" + luật method vs free fn (method thắng, free fn phải `use`); đổi tên module `test` (hoặc `test` thành contextual keyword).

**MAJ-19 — Quy trình giao việc tự mâu thuẫn; VT09 bất khả thi.**
(a) L844: VT02 "**không chặn** VT03 tới VT09" trong khi L817 đặt VS0 sau VS-SPEC và L884 (VD-08) nói thay đổi SPEC chỉ qua Agent 0 → parser vòng 1 sẽ mã hoá phỏng đoán, rồi bị VT02 duyệt lại làm invalid. (b) L851: VT09 "≥ 100 tệp lấy từ ví dụ trong tài liệu" — **đếm được**: 31 fence, 24 gắn nhãn `python`, 5 là sơ đồ/Rust/EBNF, ~19 khối ViScript, trong đó 6 khối có `...` (không hợp lệ), 1 khối shell 7 dòng đều bắt đầu bằng `>` (không phải cú pháp hợp lệ, xem MAJ-20/NIT-04) → chỉ ~10 chương trình dùng được; 90 tệp còn lại là **việc mới** không có trong task và không có đặc tả. (c) L849 tiêu chí VT07 ("parse→fmt→parse bằng nhau") **yếu hơn** L779 §18.2 (còn đòi "in lại CST đúng văn bản gốc"). (d) L779 "phục hồi lỗi có số lỗi hợp lý" không đo được. (e) L850 VT08 chạy "bộ test ngữ nghĩa … backend giả" nhưng interpreter bị hoãn (L854) → VT09 "Xanh" treo trên một runner chưa tồn tại.
*Đề nghị:* VT02 phải là **cổng chặn** VT04–VT09 (hoặc tách VT02a "đặc tả từ vựng + EBNF" chặn trước, phần còn lại song song); VT09 = "1 fixture cho mỗi khối mã trong tài liệu (~19), đánh dấu pass/fail kỳ vọng + ma trận phủ Q1–Q8"; VT07 thêm tiêu chí lossless; VT08 định nghĩa rõ chạy gì.

**MAJ-20 — Ví dụ " Flagship" của Phụ lục B không parse được theo chính tài liệu.**
- L954–L959: `let files = src.walk()` rồi xuống dòng `        |> filter(...)` — **ngoài ngoặc**, trái L105 ("trong ngoặc … mới được xuống dòng tự do") và không có luật nào cho phép (L915). Đây là chương trình mẫu số 1 của tài liệu.
- L958 `str(e.path)`: hàm chuyển đổi `str(...)` **chưa được định nghĩa** (L198 chỉ có `float(x)`, `int.*`).
- L973 `(now) - 7d`: `now` trần, trong khi L953 dùng `time.now()`; nếu `now` là hàm thì `hàm - duration` sai kiểu.
- L955 `e.is_file()` vs L973 `is_file` (gọi vs trường) — hai quy ước trong cùng một ví dụ.
- L271 `"Xin chào {name}" * times`: `str * int` **không có** trong §4.5 hay bảng std.
- L480 `f.copy_to(./backup)`: path trần trong **chế độ biểu thức** (L126 chỉ cho phép ở chế độ lệnh).
- L284–L287: closure dạng `fn(x: int) -> int:` không có trong Phụ lục A.
*Đề nghị:* sửa ví dụ + thêm cổng "mọi khối mã trong tài liệu parse được" (rẻ, bắt được cả BLK-02 và MAJ-20).

**MAJ-21 — Độ khả thi: VS1 và bộ kiểm kiểu là dự án con, không phải task.**
L818: VS1 gộp generic + trait + module + bytecode + VM + xác minh + cache + std lõi + host-posix + 4 lệnh CLI, tiêu chí "vi sai interp/VM 100.000 chương trình 0 lệch"; L660–L667: suy kiểu hai chiều + ràng buộc trait + coherence/orphan (L319) + ma trận pattern + phục hồi `{error}` + cache tăng dần; L784 cần **bộ sinh chương trình có kiểu** (không có milestone riêng). Ngân sách tham chiếu: 3 giờ/task (OS:948). Không có ước lượng công sức nào trong tài liệu (`[ƯL]` chỉ có 3 chỗ: L558, L682, L725), nên người điều phối không có cơ sở phát hiện task quá lớn.
*Đề nghị:* tách VS1 → VS1a (bytecode + VM không generic + cache + vi sai với interp), VS1b (generic biểu diễn đồng nhất + bảng chứng cứ), VS1c (trait/coherence), VS1d (std lõi + host-posix + 4 lệnh), VS1e (bộ sinh chương trình + ma trận phủ); mỗi cái có tiêu chí đo được và ước lượng `[ƯL]`.

---

## 7. MINOR

| Mã | Vấn đề | Dẫn chứng |
|---|---|---|
| MIN-01 | `#| deps: ...` là "chú thích ma thuật" không có trong §2.3 (chỉ có `#` và `##`); không có luật định dạng, nhiều phụ thuộc, phân biệt với chú thích thường, quan hệ với `vis.toml`/`vis.lock` | L405–L408 vs L110–L111, L402 |
| MIN-02 | Danh sách từ khoá thiếu `const` (được dùng trong `item`); `as` là từ khoá chết (không xuất hiện lần nào trong thân); `newtype`/`alias` có trong danh sách nhưng không có luật cú pháp | L100, L908, L170–L171 |
| MIN-03 | "Tab bị cấm (… `vis fmt` tự sửa)" bất khả: tab là **lỗi từ vựng**, `vis fmt` phải parse trước (`vis-fmt` phụ thuộc `syntax`) → không thể sửa tệp mà lexer từ chối | L104, L651, L736 |
| MIN-04 | `Option` xuất hiện trong bảng std dù ngôn ngữ dùng `T?` | L600 vs L148 |
| MIN-05 | `--desc`, `--pretty` được gọi ở L956/L961 nhưng không chữ ký std nào khai báo chúng | L956, L961, L598–L615 |
| MIN-06 | Ba dạng ngoặc nhọn (`map`, bản ghi ẩn danh, record-pun `{stdout, stderr, code}`) và `set{...}` cạnh method `counter.set(...)` | L127–L131, L386, L464 |
| MIN-07 | Quy ước biến thể enum không nhất quán: `Circle`/`Rect` viết hoa; giá trị `GitSub` viết thường (`git log`, `sub`); không nói cách định danh biến thể (`Shape.Circle`?) | L166, L468, L482 |
| MIN-08 | `derives Json` không nói cách mã hoá `datetime`, `size`, `str?` → không test được JSON ở Phụ lục B | L160, L946, L961 |
| MIN-09 | Không có sổ đăng ký mã lỗi: `VS0412` (4 chữ số) nhưng `vis explain VSxxxx`; VT02 đòi "bảng mã lỗi ban đầu" mà không có quy tắc cấp mã | L687, L739, L844 |
| MIN-10 | Đơn vị cột trong chẩn đoán/LSP chưa định nghĩa (byte/scalar/grapheme/UTF-16) — ảnh hưởng trực tiếp VT03 và LSP | L688–L690, L652 |
| MIN-11 | Luật định danh: chưa nói phiên bản UAX #31, `$`, `_`, chữ số đầu, dấu kết hợp; biến ngầm `it` có thể trùng định danh người dùng | L97, L446 |
| MIN-12 | Chỉ định nghĩa mã thoát 1; `| complete` trả `code`; job control cần phân biệt lỗi/panic/huỷ | L360, L464, L721 |
| MIN-13 | `--dry` in "danh sách quyền chương trình yêu cầu" nhưng quyền có thể **dẫn xuất** (`dir.subdir`, `net.restrict`) và bị ghi đè bởi `--grant`; chưa nói in quyền gốc hay quyền hiệu dụng | L533, L543–L544 |
| MIN-14 | `percent` không có luật: là kiểu riêng hay `float`? So sánh với `cpu` (kiểu gì?) ở L483; cộng/trừ/nhân? | L125, L483 |
| MIN-15 | Đường dẫn cấu hình `~/.config/vishell/` không theo tên lệnh `vish` (Q4) | L706 vs L865 |
| MIN-16 | `[GT]` về sandbox macOS còn mở nhưng VS2 nhận "bộ hợp chuẩn Host xanh 4 nền tảng" — rủi ro tiêu chí không đạt được | L558, L820 |

---

## 8. NIT

| Mã | Vấn đề | Dẫn chứng |
|---|---|---|
| NIT-01 | 24 khối mã ViScript gắn nhãn fence `python` — nên là `vis`; ảnh hưởng highlight, `vis doc`, công cụ trích ví dụ | L159, L425, L941… |
| NIT-02 | `...` dùng làm thân/placeholder **bên trong** ví dụ (không phải cú pháp hợp lệ) → VT09 không thể nạp nguyên văn | L229–L232, L297, L309, L428, L540 |
| NIT-03 | Bảng 9.4 có hàng "Ghi tệp" — không phải ký hiệu — nằm dưới cột "Ký hiệu" | L454 |
| NIT-04 | Tiền tố `> ` trong ví dụ shell trùng ký hiệu so sánh `>`; nếu coi là transcript thì không phải input hợp lệ | L478–L484, L973 |
| NIT-05 | `(A, B, ...)` trong bảng kiểu là văn xuôi lẫn cú pháp; `...` không phải kiểu | L147 |
| NIT-06 | Khẳng định "đoạn thử trong shell dán vào script gần như nguyên văn" sai theo chính tài liệu (`|`↔`|>`, `size`↔`e.size`, `$x`↔`x`, `take 10`↔`take(10)`, `./p`↔`p"./p"`, `is_file`↔`is_file()`) | L50 vs L452, L446, L440, L955, L973 |
| NIT-07 | `^cmd` (ép gọi ngoài) không có hàng trong bảng 9.4 dù là cú pháp riêng của chế độ lệnh | L439, L723 vs L450–L456 |

---

## 9. Phụ lục A — bảng luật thiếu

### 9.1 Được dùng nhưng **không định nghĩa** (parser không thể cài)

`NEWLINE`, `INDENT`, `DEDENT` (L915); `use` (L908); `impl_decl`, `trait_decl` (L908); `const_decl` (L908); `test_decl` (L908); `extern_cmd` (L908); `assign` (L918); `if`, `for`, `while`, `loop`, `match`, `scope` (L919); `lambda` (L920); `args` (L932); `list`, `map`, `set`, `record`, `literal` (L933); `if_expr`, `match_expr` (L933); `field_updates` (L932); `cmd_pipeline` (L933); `pattern` (L916); `types` (L934 — chỉ định nghĩa `type`, không định nghĩa `types`, dù L934 dùng `'(' types ')'`); `generics`, `path_list`, `field`, `variant`, `field_types` (L913–L914, L934).

### 9.2 Thiếu hẳn khái niệm (không chỉ thiếu tên luật)

- `elif` (L224) — không có luật nào.
- Khối một dòng sau `:` (L106, L223–L227, L272).
- `if let` (L255) — `if` + pattern, không có luật; `let` là từ khoá nên không thể coi là biểu thức thường.
- Pattern cho `T?`/`Result` (`none`, `ok`, `err`) — L248 không có.
- Pattern guard (L241) chỉ xuất hiện trong ví dụ.
- Enum constructor có tên trường trong pattern `Circle(r)` vs khai báo `Circle(r: float)` (L166, L241) — cần luật rút gọn tên trường.
- Tham số `self`/`var self` (L306, L313) có trong `param` nhưng `self` không phải kiểu, và vị trí `self` đầu tiên không được ràng buộc.
- `impl` hai dạng (`impl Show for Shape:` L308 và `impl User:` L322) — chỉ dạng thứ hai là "inherent", không có luật.
- Ghi chú tài liệu `##` gắn với item (L111) — không có luật.

### 9.3 Ví dụ trong tài liệu **không suy ra được** từ Phụ lục A

| Dòng | Ví dụ | Thiếu gì |
|---|---|---|
| L106/L223–L225 | `if x > 0: print("dương")` | khối một dòng |
| L227/L272 | `if ...: "OK" else: "FAIL"` | `if_expr` + `else` cùng dòng |
| L240–L244 | `match s:` + arm + guard | `match_expr`, `pattern`, guard |
| L248 | `{name, age: 18..}`, `[first, ..rest]`, `0..10`, `|` | `pattern` (tất cả) |
| L253–L256 | `??`, `?.`, `if let` | luật `??`, `?.` có (L932) nhưng `if let` không |
| L284–L287 | `fn(x: int) -> int:` | `lambda` |
| L294–L295 | `fn first[T](xs: list[T]) -> T?:` | `generics`, `T?` trong kiểu trả về (L934 có `['?']` — ổn), `xs[0]` |
| L300 | `json.parse[list[User]](text)` | phân biệt index vs generic |
| L305–L313 | `trait`, `impl`, `var self` | `trait_decl`, `impl_decl` |
| L343 | `let cfg = read_config(dir) else err:` | khớp L916 (ổn) nhưng `else` khối một dòng thì không |
| L385 | `ref(0)`, `counter.set/get` | `ref` là từ khoá; không có luật gọi `ref(...)` |
| L427 | `pub cmd ls(..., --all: bool = false)` | ổn với L910/L912 |
| L468 | `extern cmd git(sub: GitSub, --oneline: bool, ...rest: str)` | `extern_cmd`, `...rest` |
| L539/L952 | `cap[Dir] @ "./data"`, `(write)` | modifier tham số |
| L941–L968 | chương trình mẫu | xuống dòng treo `|>`, `str(...)`, `derives`, `test fn` |

### 9.4 Ưu tiên: Phụ lục A vs §4.5

| Cặp | §4.5 (L261) | Phụ lục A (L920–L933) | Vấn đề |
|---|---|---|---|
| `**` | cùng nhóm `* / %` | dưới `mul`, kết hợp phải | khác nhau; cần chốt |
| `??` | không nhắc | giữa so sánh và range | `a ?? b == c` = `(a ?? b) == c` — ngược thói quen C#/Swift |
| `..` `..=` | không nhắc | giữa `??` và `add` | `0..n+1` = `0..(n+1)` — phải nói rõ |
| `not` | nhóm "logic" | dưới `cmp` | `not a == b` = `not (a == b)` — phải nói rõ |
| so sánh | liệt kê | chỉ **một** phép (non-assoc) | `1 < x < 10` báo lỗi gì? |
| `?` postfix | không nhắc | cao nhất (trong `postfix`) | `x? + 1`, `f()? |> g` cần ví dụ |
| `with` | không nhắc | trong `postfix` | `u with age: 21 |> f`? |
| `|>` | "nối hàm", không nói mức | thấp nhất | nhất quán, nhưng cần câu chữ |

---

## 10. Mục 9 (chế độ lệnh) — đánh giá bảng 9.4

### 10.1 Bảng đối chiếu: bảng 9.4 **không** giải hết va chạm

| Ký hiệu | Có trong 9.4? | Va chạm thực tế chưa giải |
|---|---|---|
| `\|` | Có | mâu thuẫn với or-pattern script (L248) và với `\|>` (MAJ-01) |
| `>` | Có, nhưng hàng này **không giải gì** (hai cột giống nhau); ngoài "tham số biểu thức" thì `>` là gì? | ví dụ lại dùng `>` làm prompt (L478–L484) |
| `<` | **Không** | so sánh vs redirect-in (`where x < 3` chạy được? `<` ngoài expression?) |
| `(` `)` | **Không** | `(expr)` là đối số vs nhóm lệnh vs gọi hàm không cách: `ls (1)` ≠ `ls(1)`? Không có luật khoảng trắng |
| `[` `]` | **Không** | glob char-class (`[abc]*.txt`) vs list literal vs index/generic; `arg` (L440) **không có** dạng `[` → không truyền được list literal |
| `{` `}` | **Không** | brace expansion kiểu bash vs map/bản ghi (L128, L131) |
| `?` | **Không** | glob một ký tự vs `?` postfix vs `?.`/`??`; query string `?a=1` (L484) |
| `*` | Có | nhưng `**` (glob đệ quy vs luỹ thừa) không có; `*.txt` vào tham số `path` (số ít) thì sao — L443 chỉ nói `list[path]`/`glob` và `str` |
| `$` | Một phần (L440) | `$HOME` (env) không tồn tại; `$` trong URL/regex |
| `#` | **Không** | chú thích cắt URL fragment **âm thầm** (`https://x/y#frag`, L484) |
| `%` | **Không** | `percent` vs modulo vs job spec `%1` |
| `&` | Chỉ có trong ngữ pháp (L437), không có trong bảng | `&` nền vs `&&` vs query string `&b=2` |
| `;` | **Không** | nhiều lệnh một dòng (không có ký hiệu phân cách) |
| `^` | Có ở L439/L723, **không có hàng trong bảng** | `^` không tồn tại ở script mode (bất đối xứng) |
| `-` | Có | `-5` (số âm) vs cờ `-5`; `--` kết thúc cờ; `-` = stdin |
| `!` `~` `=` `\` | **Không** | history, home, gán, escape |

### 10.2 Các dòng lệnh mơ hồ cụ thể (nên thành golden test)

1. `http get https://x/api?a=1&b=2#frag` — bị cắt ở `&` (job control, L437) và `#` (chú thích, L110); `?`/`%20` chưa có luật.
2. `ls -la` vs `ls -5` vs `cp -r -- -x` — luật `-` chưa có.
3. `ls(1)` vs `ls (1)` — khoảng trắng có ý nghĩa không? `command arg*` (L438) không nói.
4. `sort_by size --desc -v` — trộn tham số vị trí, cờ, `-v` (lỗi? cờ ngắn có tồn tại?).
5. `where x in [1, 2]` — `arg` không có `[`.
6. `echo "a|b"` / tên tệp có dấu cách — không có luật trích dẫn/thoát (MAJ-14).
7. `glob **/*.txt` vs `2 ** 3` — `**` chưa có luật.
8. `take 10 + 2` — là `take(12)` hay lỗi cú pháp?
9. `let x = 5` ở dòng lệnh — không có trong ngữ pháp §9.3 (MAJ-04).
10. `let big = ls ./photos | where size > 5mb` (L479) — `big` có kiểu gì? `stream[FileEntry]`? pipeline có bị "chạy" ngay không?

### 10.3 §9.5 — pipeline dữ liệu có cấu trúc: **mơ hồ**

Xem MAJ-02 và MAJ-15. Tóm tắt: `from json` không có kiểu đích → hoặc vi phạm "không `any`" (L79–L80) hoặc không dùng được `where` (L481); `take`+SIGPIPE chưa có luật; `complete` chưa rõ tương tác với "mã thoát khác 0 = lỗi pipeline"; `from/to csv` chưa có kiểu; `Render` chưa có luật cho `T?`/`Json`.

---

## 11. Tính khả thi và tiêu chí xong (VT01–VT10)

| ID | Vấn đề | Đề nghị |
|---|---|---|
| VT01 | Ổn; nhưng "build wasm32 cho crate lõi" khi crate rỗng thì không chứng minh được gì | thêm: `vis-syntax`/`vis-types` phải biên dịch với `--target wasm32-unknown-unknown` **và** có 1 test chạy được |
| VT02 | Một task 3 giờ cho "EBNF đầy đủ + bảng ưu tiên + từ khoá + mã lỗi"; không có rubric duyệt; đang bị chính L844 nói là không chặn | tách VT02a (từ vựng + EBNF tự chứa, **chặn** VT04–VT09), VT02b (ưu tiên + 100 biểu thức vàng), VT02c (sổ mã lỗi), VT02d (đặc tả chế độ lệnh từ §9.3) |
| VT03 | Ổn; thiếu luật đơn vị cột (MIN-10) và sổ mã (MIN-09) | thêm 2 luật + snapshot 20 ca |
| VT04 | **Bị chặn bởi BLK-03** (không có bảng token/escape/đơn vị/NFC) | chỉ giao sau `SPEC-lex.md`; tiêu chí: bảng token vàng + fuzz 10 phút không panic + test NFC (NFC/NFD cùng một định danh, literal `'ố'` NFD phải lỗi rõ ràng) |
| VT05 | **Bị chặn bởi BLK-02**; tiêu chí "In lại CST đúng nguồn; fuzz" không đo được "phục hồi lỗi hợp lý" | tiêu chí: 100% ví dụ trong tài liệu parse; mỗi tệp lỗi trong `tests/parse-errors` cho **đúng 1** lỗi đầu tại dòng ghi trong fixture |
| VT06 | **Bị chặn bởi BLK-01 + BLK-03**; tiêu chí "hai chế độ ra cùng AST" không đạt được khi chưa có kiểu | đổi thành "CmdCST ổn định + golden test cho **từng hàng bảng 9.4** + 10 ca mơ hồ ở §10.2 ở trên"; `cmd_lower` tách sang vòng 2 |
| VT07 | Tiêu chí yếu hơn §18.2; chưa có đặc tả định dạng chuẩn (độ dài dòng, cách ngắt `|>`, dấu phẩy cuối) | thêm: (a) lossless CST→text trên 100% fixture, (b) `fmt` idempotent, (c) tệp có tab/lỗi cú pháp → hành vi xác định (đề nghị: báo lỗi, không sửa) |
| VT08 | "backend giả" là gì khi chưa có interpreter? VT09 treo vào nó | hoặc định nghĩa runner chỉ kiểm cú pháp (`--check`), hoặc hoãn VT08/VT09 sang vòng 2 |
| VT09 | **Bất khả thi như viết** (~10 chương trình dùng được, không phải 100) | 1 fixture/khối mã (~19) + fixture viết tay cho mỗi hàng Q1–Q8; cấm dùng `...` |
| VT10 | Ổn (OS §11.7 tồn tại) | — |

Bổ sung task còn thiếu: (1) `SPEC-lex.md` (chặn VT04); (2) bảng chữ ký std tối thiểu `FileEntry`/`Ctx`/`cap[*]` (chặn golden test của VT06); (3) sổ mã lỗi (chặn VT03); (4) bộ sinh chương trình có kiểu (chặn VS1e); (5) fixture "mọi khối mã trong tài liệu".

---

## 12. Ba việc **phải** sửa trước khi giao Agent 2

### (1) Chốt mô hình parse chế độ lệnh — BLK-01
Sửa §9.1/§9.3/§18.2 và VT06 theo mô hình hai pha (CmdCST → `cmd_lower` trong `vis-types` → HIR). Ghi bảng "luật → pha" cho: literal theo kiểu tham số, glob theo kiểu, `it` + viết tắt trường, phân biệt cmd/biến hàm/chương trình ngoài, `--flag` binding, `--help`. Đổi câu "cùng AST" thành "cùng HIR". Nếu không chốt, VT05/VT06/VT07 sẽ phải làm lại.

### (2) Viết lại Phụ lục A thành EBNF tự chứa — BLK-02
Đủ `pattern`, `if_expr`, `match_expr`, `lambda`, `args`, `field_updates`, `types` (có `()`), `type` nhận `ref`/`cap`, `item` 10 dạng, khối một dòng, `elif`, `assign`, token `NEWLINE/INDENT/DEDENT`. Kèm cổng: **mọi khối mã trong tài liệu parse được** (bắt luôn MAJ-20). Đây là điều kiện cần để VT05 có nghĩa.

### (3) Viết `SPEC-lex.md` và chốt ngữ nghĩa chuỗi/ký tự — BLK-03 (+ MAJ-11)
Bảng token + maximal munch cho `..`/`?`/`??`/`%`/`**`/`p"`/`b"`/`r"`/`"""`; bảng escape; bảng đơn vị `size`/`duration`; luật `#`/`&`/`$`/`;`/`[`/`(` trong chế độ lệnh; luật NFC cho literal; **đơn vị chỉ số chuỗi (grapheme) và đơn vị cột chẩn đoán** (ảnh hưởng VT03, VT04, std `text`). Không có ba việc này, Agent 2 sẽ phải đoán ở đúng những chỗ không thể sửa ngược.

---

## 13. Phụ lục — bộ test/gate tối thiểu nên thêm ngay

1. **Gate "doc-examples parse"**: trích mọi fence, chạy `vis check`; danh sách ngoại lệ phải khai báo tường minh (dự kiến ~6 khối `...`, 7 dòng `>`).
2. **Golden token**: 60 dòng, mỗi dòng một ca (`0..10`, `1..=10`, `50%`, `50 % 3`, `5kb`, `5kib`, `2h30m`, `1e-9`, `1_000`, `0xFF`, `p"a b"`, `r"\n"`, `b"\x00"`, `'ố'` NFC, `'ố'` NFD, `x??y`, `a?.b`, `f()?`, `set{1,2}`, `https://x/y#z`, `a|b`, `ls -5`…).
3. **Golden AST chế độ lệnh**: 1 ca cho mỗi hàng bảng 9.4 + 10 ca mơ hồ ở §10.2.
4. **Golden precedence**: `-2 ** 2`, `2 ** 3 ** 2`, `a ?? b == c`, `not a == b`, `0..n+1`, `x |> f == y`, `u with age: 21`.
5. **Golden optional**: `T → T?`, `T??`, `?` trên `T?`, `if let`, `match` trên `T?`/`Result`, `.get/.last` khi rỗng.
6. **UTF-8/NFC**: định danh NFC/NFD cùng một tên; `'ố'` NFD báo lỗi có thông điệp; cột chẩn đoán trên dòng có tiếng Việt + emoji (để chốt đơn vị cột).
7. **Pipeline**: `take` + chương trình ngoài (SIGPIPE), `complete`, `from json[User]`, `get` thiếu trường, `Render` cho `T?`/`Json`.
8. **Capability âm**: `..`, symlink, `dir.subdir` leo ngược, `sh` không có `cap[Proc]`, plugin tĩnh trong chế độ hạn chế.

---

## 14. Ghi chú về phạm vi review

- Không đánh giá phần kernel/OS; chỉ dùng `OS` §7.3/§11/§13 để kiểm tra tra chéo và ngân sách.
- Mọi kết luận "ĐẠT" ở §3 đều đã kiểm bằng máy (script trên 31 fence, grep ký hiệu, so tiêu đề với mục lục), không dựa vào đọc mắt.
- Các con số đếm được (31 fence, 24 nhãn `python`, 7 dòng `>`, 6 khối `...`, 2 dòng `##` trong fence) có thể tái lập bằng: `grep -n '^```' …`, `grep -n '^> ' …`, `grep -n '\.\.\.' …`.
