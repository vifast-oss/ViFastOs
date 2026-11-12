# ViScript + ViShell: Tài liệu thiết kế

Người viết: Agent 0 (Copilot). Đối tượng: người điều phối + Agent 2.
Quan hệ với ViFastOs: ViScript là **sản phẩm độc lập**, chạy trên Linux, macOS, Windows, WebAssembly và ViFastOs. Tài liệu `ViFastOs-v2-thiet-ke.md` (mục 7.3) chỉ mô tả phần tích hợp; mọi quyết định về ngôn ngữ nằm ở đây.
Giấy phép mã nguồn: **MIT OR Apache-2.0** (giống Rust).

Nhãn: `[V1:CODE]`, `[V1:ĐO]`, `[V1:DOC]` = lấy từ dossier v1, giữ độ tin cậy gốc; **`[TK]`** = quyết định thiết kế (**mặc định** cho dòng không nhãn); `[GT]` = giả thuyết cần kiểm; `[ƯL]` = ước lượng cần đo lại.

## Thay đổi sau review (2026-11-12)

Bản này đã vá theo [`docs/QUYET-DINH-sau-review.md`](../QUYET-DINH-sau-review.md) (gọi tắt **QD**) — nguồn sự thật; chỗ nào QD nói khác tài liệu này thì **QD thắng**. Đặc tả từ vựng chi tiết (bảng token, escape, đơn vị, maximal munch, golden token) nằm ở `docs/viscript/SPEC-lex.md` — **đã có**; ST-02a duyệt/bổ sung. Phụ lục A dưới đây là **EBNF tự chứa** và là chuẩn tắc cho parser.

| # | Nhóm đã vá | Mục |
|---|---|---|
| 1 | Không gian tên: task ViScript `VT01..VT10` → **`ST-01..ST-10`**; lỗi gốc `G1..G6` → `R1..R6`; tag commit `[ST-##]` | 19, 20, 21 |
| 2 | Từ vựng: NFC toàn tệp trước khi lex; thụt lề suy đơn vị; tab = `VS0107` nhưng **vẫn lex**; bảng literal mới; bỏ `50%` → `50pct`; escape/nội suy; cột theo Unicode scalar | 2.1–2.4, 3.5 |
| 3 | Parse chế độ lệnh **hai pha**: `vis-syntax` → `CmdCST` → `cmd_lower` (trong `vis-types`) → HIR; "cùng AST" đổi thành "cùng **HIR**" | 9.1, 9.3, 14.1, 18.2 |
| 4 | Ngữ nghĩa số: bảng đầy đủ `%`, `**`, bit, float/NaN, `size/duration/percent`; **bảng ưu tiên 12 mức** | 3.6, 4.5 |
| 5 | Kiểu và optional: thêm `Future[T]`, `Task[T]`, `AnyError`; bỏ `s[i]`; tự nâng `T → T?`; pattern `none/some/ok/err` | 3, 4.3–4.4 |
| 6 | Lỗi: `trait Error: Show`, `AnyError`, `?` tự chuyển; **bảng mã thoát**; panic payload + arena lệnh | 6 |
| 7 | Chế độ lệnh: metachar, `$env.NAME`, `;`/`&`/`--`/`-5`, closure-arg, `shell_item`, `from json[T]`, `complete` | 9 |
| 8 | Host/ViShell: proc suspend/resume/cancel + wait; shell sở hữu tty, con nhận pipe, pty để sau | 12, 15 |
| 9 | Crate/plugin/cổng: `vis-host-vifast` ngoài `viscript/`; plugin động tắt ở chế độ hạn chế; cổng **doc-examples parse** | 14.2, 17, 18.4 |
| 10 | **Phụ lục A viết lại** thành EBNF tự chứa; Phụ lục B sửa ví dụ; thêm VD-11..VD-20 | A, B, 22 |

Ba quyết định lớn ảnh hưởng thi công: (a) QD §1 **Q-D** — parse chế độ lệnh hai pha; `VT06` cũ chỉ làm `CmdCST` + golden, `cmd_lower` là task vòng 2; (b) QD §7.4 — **ST-02a chặn ST-04..ST-09**; (c) QD §7.3 — **`|` không bao giờ là toán tử nhị phân trong biểu thức**, pattern vẫn dùng `|`.


---

## Mục lục

0. Định vị
1. Nguyên tắc
2. Từ vựng và cú pháp cơ bản
3. Hệ kiểu
4. Biểu thức, câu lệnh, pattern
5. Hàm, closure, generic, trait
6. Xử lý lỗi
7. Mô hình giá trị và bộ nhớ
8. Module và gói
9. Chế độ lệnh (shell)
10. Bất đồng bộ và song song
11. Capability và quyền
12. Trait Host (đa nền tảng)
13. Thư viện chuẩn
14. Kiến trúc cài đặt
15. Kiến trúc ViShell
16. Công cụ
17. Plugin Rust
18. Kiểm thử theo từng phần
19. Lộ trình
20. Quy trình Agent 2 và giao việc vòng 1
21. Các lựa chọn đã chốt
22. Quyết định
23. Rủi ro
Phụ lục A. EBNF tự chứa (chế độ script + chế độ lệnh)
Phụ lục B. Chương trình mẫu

---

## 0. Định vị

**Cú pháp gọn như Python, kiểu tĩnh như Rust/Swift, chạy ngay không cần build, một ngôn ngữ cho cả script lẫn shell, quyền theo capability, chạy đa nền tảng.**

### 0.1 Mục tiêu

- Viết công cụ, tự động hoá, xử lý dữ liệu, ứng dụng nhỏ, cấu hình hệ thống nhanh như Python.
- Bắt lỗi kiểu, lỗi thiếu giá trị, lỗi thiếu nhánh **trước khi chạy**.
- Shell tương tác dùng chính ngôn ngữ này; đoạn thử trong shell dán vào script gần như nguyên văn.
- Cùng một chương trình chạy giống nhau trên mọi nền tảng được hỗ trợ.
- Script không có quyền gì ngoài những quyền được trao.
- Là ngôn ngữ script và shell chính của ViFastOs.

### 0.2 Không phải mục tiêu

- Thay Rust cho phần cần hiệu năng cao (dùng plugin Rust).
- JIT, biên dịch ra mã máy.
- Tương thích cú pháp Bash/POSIX sh.
- Lập trình hệ thống mức thấp (con trỏ, bố cục bộ nhớ).
- Kế thừa class, metaprogramming động, `eval` chuỗi trong chế độ quyền hạn chế.

### 0.3 So sánh

| | Python | TypeScript | Nushell | ViScript |
|---|---|---|---|---|
| Cú pháp | Thụt lề | Ngoặc nhọn | Riêng | Thụt lề |
| Kiểu | Động | Tĩnh, có `any` | Động + chữ ký lệnh | **Tĩnh, không `any`**, tự suy cục bộ |
| Null | `None` mọi nơi | `null/undefined` | `null` | Không null, `T?` |
| Lỗi | Exception | Exception | Lỗi có cấu trúc | `Result` + `?` |
| Shell | Không | Không | Có | Có, cùng ngôn ngữ |
| Quyền | Toàn quyền | Deno có quyền | Toàn quyền | Capability |

---

## 1. Nguyên tắc

1. **Một ngôn ngữ, hai chế độ cú pháp** (script và lệnh), cùng **HIR**, cùng kiểm kiểu, cùng VM.
2. **Kiểu tĩnh, không lối thoát ngầm.** Không `any`. Dữ liệu chưa rõ cấu trúc có kiểu riêng `Json` và phải chuyển đổi tường minh.
3. **Chữ ký hàm ghi kiểu, thân hàm tự suy.** Suy kiểu cục bộ hai chiều, không suy toàn chương trình.
4. **Ngữ nghĩa giá trị.** Gán là sao chép về mặt ngữ nghĩa, cài đặt bằng chia sẻ + copy-on-write.
5. **Không quyền ngầm.** Thư viện chỉ chạm hệ thống qua capability nhận làm tham số.
6. **Lỗi là giá trị.** `panic` chỉ cho lỗi lập trình, không cho lỗi môi trường.
7. **Lõi không biết hệ điều hành.** Mọi thứ ra ngoài đi qua trait `Host`.
8. **Thông báo lỗi là tính năng.** Chỉ đúng vị trí, nói rõ nguyên nhân, gợi ý sửa; có tiếng Việt, tiếng Anh, tiếng Nhật.
9. **Tất định.** `map`/`set` duyệt theo thứ tự chèn; không hành vi phụ thuộc địa chỉ bộ nhớ.
10. **Mỗi tính năng phải có ví dụ thực tế và test trước khi vào SPEC.**

---

## 2. Từ vựng và cú pháp cơ bản

### 2.1 Tệp, mã hoá, định danh, từ khoá

- Phần mở rộng `.vis` (script), `.vish` (tệp ở chế độ lệnh, xem 9.1). UTF-8 bắt buộc. Dòng đầu tuỳ chọn `#!/usr/bin/env vis`.
- **Chuẩn hoá NFC trước khi lex** (QD §7.1): **toàn bộ** tệp nguồn được chuẩn hoá Unicode NFC **trước** khi lex. Hệ quả: định danh, literal `str`/`char` và **cột chẩn đoán** đều tính trên dạng NFC; `'ố'` gõ dạng tổ hợp hay dựng sẵn đều là **một** Unicode scalar và là một literal `char` hợp lệ; `số_lượng` viết dạng tổ hợp (NFD) và dạng dựng sẵn (NFC) là **cùng một** tên. `vis fmt` ghi ra NFC. Vị trí báo lỗi vẫn trỏ được về tệp gốc vì cả lexer và editor cùng chuẩn hoá một cách tất định.
- **Từ khoá tiếng Anh** (giống Python, Rust, JS/TS). Danh sách:
  `let var const fn cmd type enum trait impl alias newtype pub use if elif else match for in while loop break continue return and or not true false none self async await scope test extern derives with has dyn ref`
  - **Thêm `const`** (dùng cho `const_decl`, Phụ lục A).
  - **Bỏ `as`**: bản này không có cú pháp alias trong `use`; `as` là định danh bình thường.
  - **`test` là từ khoá theo ngữ cảnh**: chỉ là từ khoá khi đứng đầu item và ngay sau là `fn` (`test fn name():`). Ở vị trí khác `test` là định danh, nên `test.run()` hợp lệ.
  - Module kiểm thử đổi tên `test` → **`std.testing`** (xem 13), nhờ vậy `use std.testing` lex được.
- **Định danh Unicode** theo UAX #31: được viết tiếng Việt, tiếng Nhật (`số_lượng`, `tính_tổng`). So sánh sau chuẩn hoá NFC, nên `ố` gõ bằng tổ hợp hay dựng sẵn đều là một tên.
- Lint cảnh báo định danh trộn chữ dễ nhầm (ví dụ Latin + Cyrillic giống nhau) để tránh tấn công homoglyph.

### 2.2 Khối và thụt lề

- Khối mở bằng `:` ở cuối dòng.
- **Đầu vào: thụt lề nhất quán bất kỳ.** Lexer **suy đơn vị thụt lề** từ **cấp thụt lề đầu tiên** trong tệp (dòng đầu tiên có thân khối); mọi mức sau phải là bội số của đơn vị đó. Trộn 2 và 4 dấu cách trong cùng tệp là lỗi (mã trong `ST-02b`). Nhờ vậy dán mã từ nơi khác không chết ngay dòng đầu, mà vẫn giữ "một kiểu duy nhất" ở đầu ra.
- **Đầu ra: `vis fmt` luôn 4 dấu cách** [TK].
- **Tab:** lexer báo **cảnh báo-lỗi `VS0107`**, quy ước **tab = 4 cột**, và **vẫn lex tiếp** (không dừng). `vis check` coi `VS0107` là **lỗi**; `vis fmt` **sửa** tab thành dấu cách. Chính vì "vẫn lex tiếp" mà `vis-fmt` có CST để sửa — đây là chỗ vá mâu thuẫn cũ "tab là lỗi từ vựng nhưng `vis fmt` tự sửa".
- Thân ngắn một dòng sau `:` được phép: `if x > 0: return x`. Khối một dòng **không** sinh `INDENT`/`DEDENT`.
- **Bốn ca biên** (luật cho lexer; golden token ở `SPEC-lex.md`):
  1. **Dòng trống hoặc chỉ có chú thích** không sinh `INDENT`/`DEDENT` và **không đóng khối**; chú thích ở cột 0 vẫn nằm trong khối đang mở.
  2. **Chuỗi nhiều dòng** (`"""..."""`, `r"""..."""`) được nuốt **trọn** thành một token **trước** khi tính cột, nên nội dung ở cột 0 không đóng khối và không sinh `DEDENT`.
  3. **Trong `()`, `[]`, `{}`** tắt `INDENT`/`DEDENT`: xuống dòng tự do, không sinh `NEWLINE` có nghĩa.
  4. **`DEDENT` ngầm** ở `EOF` và ở **cuối mỗi input REPL** (dán khối nhiều dòng vào shell rồi Enter là hết khối).
- **Nối dòng bằng `|>`:** một dòng bắt đầu bằng `|>` ở **độ sâu ngoặc 0** được nối với dòng trước (không sinh `NEWLINE`). Đây là ngoại lệ **duy nhất** của luật "xuống dòng ngoài ngoặc chỉ sau dấu `:`" (QD §7.4).
- CST giữ **độ rộng thụt lề gốc** (để `vis fmt` lossless); AST/HIR chỉ giữ cấu trúc khối.

### 2.3 Chú thích

- **Chế độ script:** `#` ngoài chuỗi **luôn** là chú thích, cắt tới cuối dòng.
- **Chế độ lệnh:** `#` chỉ là chú thích khi **đứng đầu từ** — đầu dòng hoặc ngay sau khoảng trắng. Ở giữa từ, `#` là ký tự thường: `https://x/y#frag` giữ nguyên `#frag` (QD §7.1, §7.2).
- `##` là tài liệu, gắn với khai báo kế tiếp (dùng cho `vis doc` và LSP). `##` chỉ có nghĩa khi đứng đầu dòng (sau thụt lề).
- `#|` mở đầu một dòng chú thích meta của script (`deps`, xem mục 8); phần còn lại của dòng là chú thích theo luật trên, định dạng do mục 8 quy định.

### 2.4 Literal

| Loại | Ví dụ | Kiểu / luật |
|---|---|---|
| Số nguyên | `42`, `1_000_000`, `0xff`, `0b1010` | `int` (i64). `_` **chỉ** đứng giữa hai chữ số (`1_0` đúng; `_1`, `1_`, `1__0` sai). **Literal tràn i64 là lỗi biên dịch** (khác tràn lúc chạy = panic) |
| Số thực | `3.14`, `1e-9` | `float` (f64). **Bắt buộc có chữ số sau `.`**: `1.` sai; `1..2` là **range** (`1` `..` `2`), không phải float |
| Bool | `true`, `false` | `bool` |
| Ký tự | `'a'`, `'ố'` | `char` (một Unicode scalar; nguồn đã NFC nên `'ố'` NFD thành một scalar) |
| Chuỗi | `"xin chào {name}"`, `r"C:\raw"`, `"""nhiều dòng"""`, `r"""thô"""` | `str` (xem bảng dưới) |
| Bytes | `b"\x00\x01"` | `bytes` (chỉ ASCII + `\xNN`) |
| Kích thước | `5kb`, `5kib`, `2mb`, `1gib` | `size`; đơn vị `b kb mb gb tb kib mib gib tib`, **không dấu cách**; `kb`=1000, `kib`=1024 |
| Thời lượng | `250ms`, `2h30m`, `1d` | `duration`; đơn vị `ns us ms s m h d w`; ghép **giảm dần** là **một token** (`2h30m` đúng, `30m2h` sai) |
| Phần trăm | `50pct` | `percent`; **bỏ literal `50%`** (QD §7.1): `%` **chỉ** là modulo; job spec viết `job 1`, không phải `%1` |
| Đường dẫn | `p"./data/a.txt"` (script), `./data/a.txt` (chế độ lệnh, từ trần) | `path` |
| List | `[1, 2, 3]` | `list[int]` |
| Map | `{"a": 1, "b": 2}` | `map[str, int]` |
| Set | `set{1, 2}` | `set[int]` |
| Tuple | `(1, "a")` | `(int, str)` |
| Bản ghi ẩn danh | `{name: "An", age: 20}` | `{name: str, age: int}` |

**Năm dạng chuỗi** (QD §7.1):

| Dạng | Escape | Nội suy | Nhiều dòng |
|---|---|---|---|
| `"..."` | có | có | không |
| `r"..."` | **không** (thô) | **không** | không |
| `"""..."""` | có | có | có |
| `r"""..."""` | **không** | **không** | có |
| `b"..."` | chỉ ASCII + `\xNN` | không | không |
| `p"..."` | như `"..."`, kết quả là `path` | có | không |

- **Escape** (chỉ trong `"..."`, `"""..."""`, `b"..."`, `p"..."`): `\n` `\t` `\r` `\0` `\\` `\"` `\'` `\{` `\}` `\u{X..X}` (1–6 chữ số hex) [TK]; `\xNN` **chỉ** trong `b"..."`; `\` khác là lỗi. Trong `r"..."`/`r"""..."""` **không** có escape: `\` là ký tự thường.
- **Nội suy:** `{expr}` hoặc `{expr:spec}`, `spec` theo cú pháp Rust format (`{x:>8}`, `{n:.2}`); `{{` và `}}` cho ra `{` `}` thật. Biểu thức trong `{}` là biểu thức đầy đủ, được kiểm kiểu; giá trị in qua `Show` (3.5).
- **`set{` dính liền** là set literal; có khoảng trắng (`set {`, `set (`, `counter.set(...)`) thì `set` là **định danh**.
- **Maximal munch:** `??`, `?.`, `?` → `x??y` là `x ?? y`; `|>` dài nhất trước `|`.
- **Cột chẩn đoán** đếm theo **Unicode scalar, bắt đầu từ 1**, trên văn bản **đã NFC**; LSP đổi sang **UTF-16 ở biên** giao thức (`Position.character` của LSP là UTF-16), nội bộ `vis-diag` luôn là scalar (QD §7.1).
- `kb` = 1000, `kib` = 1024.

---

## 3. Hệ kiểu

### 3.1 Kiểu dựng sẵn

| Nhóm | Kiểu |
|---|---|
| Số | `int` (i64, tràn là panic; phép quấn tường minh `wrapping_add`), `float` (f64) |
| Logic, ký tự | `bool`, `char` |
| Văn bản | `str` (UTF-8 bất biến, **không có `s[i]`**, so sánh theo dãy scalar), `bytes` |
| Hệ thống | `path`, `size`, `duration`, `percent`, `instant`, `datetime` |
| Tập hợp | `list[T]`, `map[K, V]`, `set[T]`, `(A, B, ...)` |
| Rỗng, lỗi | `T?`, `Result[T, E]`, `AnyError`, `never`, `()` |
| Hàm | `fn(A, B) -> C` |
| Luồng, bất đồng bộ | `stream[T]` (lười, kéo), `Future[T]` (kết quả lười), `Task[T]` (future đã spawn) |
| Dữ liệu động | `Json` (enum: Null, Bool, Num, Str, Arr, Obj) |
| Tham chiếu chia sẻ | `ref[T]`, `weak[T]` |
| Quyền | `cap[Dir]`, `cap[File]`, `cap[Net]`, `cap[Proc]`, `cap[Env]`, `cap[Term]`, `cap[Service[P]]` |

- `Future[T]` là giá trị lười do gọi `async fn` sinh ra; `Task[T]` là future **đã được spawn** trong `scope`, có handle để `await`/huỷ (10.1).
- `AnyError` là **kiểu lỗi xoá kiểu** dùng ở ranh giới (6.1).
- `T??` là **lỗi kiểu** — optional không lồng (4.4).
- `percent` so sánh được với `float` **theo tỉ lệ** (`50pct == 0.5`, 3.6); `ps.cpu` có kiểu `percent`.
- `bigint`, `decimal` là thư viện, không nằm trong lõi.

### 3.2 Kiểu người dùng

```vis
type User derives Eq, Show, Json:
    name: str
    age: int
    email: str? = none

enum Shape derives Show:
    Circle(r: float)
    Rect(w: float, h: float)
    Empty

alias UserId = int
newtype Email = str             # kiểu mới, không tự chuyển đổi với str
```

- `derives` sinh cài đặt trait chuẩn: `Eq`, `Ord`, `Hash`, `Show`, `Json`, `Default`. Kiểu chứa `float` **không** derive được `Eq`/`Hash` (3.6).
- Tạo bản ghi: `User(name: "An", age: 20)`; tham số có tên bắt buộc khi bản ghi có hơn 2 trường.
- Cập nhật: `u with age: 21` trả bản ghi mới. Trường **bất biến** ở bản này (không có `var` cho trường).
- `alias` là tên khác của cùng kiểu; `newtype` là kiểu mới, phải chuyển đổi tường minh (`Email(s)` / `e.0`).

### 3.3 Kiểu cấu trúc cho dữ liệu bảng

Bản ghi ẩn danh so khớp theo cấu trúc. Hàm có thể nhận "mọi bản ghi có trường `size: size`":

```vis
fn total[R: has {size: size}](rows: list[R]) -> size:
    rows.map(r => r.size).sum()
```

Dùng chủ yếu cho pipeline shell. Kiểu `type` có tên so khớp theo tên.

### 3.4 Suy kiểu

- Hai chiều cục bộ: kiểu mong đợi chảy từ ngữ cảnh vào biểu thức (closure, literal rỗng), kiểu thực tế chảy ngược lên.
- Tự suy: biến cục bộ, closure, kiểu trả về closure.
- Bắt buộc ghi: chữ ký hàm cấp cao nhất, trường bản ghi, hằng `pub`.
- `[]`, `{}` không đủ thông tin: lỗi yêu cầu ghi kiểu (`let xs: list[int] = []`).

### 3.5 Chuyển đổi

Không chuyển đổi ngầm giữa số. `float(x)`, `int.trunc(f)`, `int.parse(s) -> Result[int, ParseError]`. Ngoại lệ duy nhất: literal số nguyên ở vị trí `float`.

- **`str(x)` = `x.show()`** (QD §7.3) — cùng một đường, không có "ép kiểu chuỗi" riêng. Mọi kiểu in được đều cài `Show` (qua `derives Show` hoặc viết tay); nội suy `{expr}` dùng đúng `show()`.
- **Path từ chuỗi phải tường minh**: `path("...")` hoặc literal `p"..."`; **không** còn chuyển ngầm `str → path`, kể cả ở tham số mặc định. Tham số mặc định kiểu `path` viết `path = p"."`.
- `p"..."` là literal `path`; `path("...")` là hàm chuyển đổi tường minh cùng nghĩa (dùng khi chuỗi đến từ biến).
- `str` **không có** `s[i]`: dùng `.chars()`, `.graphemes()`, `.bytes()`, `.byte_at(i)?`; `byte_len()`, `char_count()`, `grapheme_count()` (3.1, 4.5).
- So sánh `str` theo **dãy Unicode scalar** (sau NFC), không theo grapheme.

### 3.6 Ngữ nghĩa số

Bảng bắt buộc (QD §7.3, chấp nhận bảng D điểm 3):

| Biểu thức | Kiểu | Luật |
|---|---|---|
| `int / int` | `int` | cắt về 0; `/0` **panic**; có `div_floor`, `div_euclid`, `checked_div -> int?` |
| `int % int` | `int` | dấu theo **số bị chia** (C/Rust: `-7 % 2 == -1`); `%0` **panic**; có `rem_euclid`, `mod_floor` |
| `int ** int` | `int` | **kết hợp phải**, **cao hơn** `*`; mũ âm **panic**; tràn **panic**; có `checked_pow -> int?` |
| `float` cơ bản | `float` | f64; `/0.0` theo IEEE 754 (±inf/NaN), **không panic** |
| NaN | — | `==` trả `false` (kể cả `nan == nan`); float **không** có `Eq`/`Ord`/`Hash`; kiểu chứa `float` không `derives Eq`/`Hash`; sắp xếp dùng `total_cmp` |
| Bit | `int` | `bit_and bit_or bit_xor bit_not shl shr ushr` trên 64 bit có dấu (`bit_not(0) == -1`); `shr` **số học** (giữ dấu), `ushr` **logic**; số đếm ngoài `0..63` **panic**; có `checked_shl`, `checked_shr` |
| `size ± size` | `size` | tràn panic; `size * int`, `size / int`; `size / size -> float` |
| `duration ± duration` | `duration` | `duration * int`, `duration / int`, `duration / duration -> float` |
| `datetime - datetime` | `duration` | `datetime ± duration -> datetime` **theo UTC**, không theo lịch (DST để thư viện `time`) |
| `percent * size` | `size` | `percent * int -> int` |
| `percent` vs `float` | — | so sánh **theo tỉ lệ**: `50pct == 0.5`, `25pct < 0.3`; `percent` lưu tỉ lệ 0..1 |

- `int` tràn lúc chạy = panic; literal tràn i64 = lỗi biên dịch (2.4).
- Lint: cảnh báo khi kết quả `int/int` được so sánh hoặc dùng với `float`/`percent` (lint `percent-of-int-division`); lint chia hai literal nguyên giữ nhưng **hạ mức** cảnh báo.
- `0 ** 0 == 1`; `float` luỹ thừa theo `f64.powf`.

---

## 4. Biểu thức, câu lệnh, pattern

### 4.1 Biến và scope

```vis
let x = 10            # bất biến
var count = 0         # biến đổi được
count += 1
let (a, b) = pair
let {name, age} = user
```

- `let` bất biến, `var` biến đổi được; `count += 1` là `count = count + 1`.
- **Shadowing** (QD §7.3): được **che tên ở scope trong** — `let x = 1` ngoài, `let x = "a"` trong khối là hợp lệ, **kiểu được phép khác**. **Khai lại cùng một scope là lỗi** kiểm kiểu, kể cả khi kiểu giống hệt.
- Tham số, biến `for`, binding của arm `match` và `if let` cũng là khai báo: chúng che tên ngoài, không khai lại trong cùng scope.
- Gán cho `let` hoặc cho tham số là lỗi; `var self` là ngoại lệ duy nhất cho tham số (5.4).

### 4.2 Điều khiển

```vis
if x > 0: print("dương")
elif x < 0: print("âm")
else: print("không")

let label = if ok: "OK" else: "FAIL"

for i in 0..10: ...        # 0 tới 9
for i in 0..=10: ...
for (k, v) in m: ...
while cond: ...
loop:
    if done(): break
```

- `if`/`match` dùng được như **biểu thức** khi có `else`; như câu lệnh thì `else` tuỳ chọn (Phụ lục A).
- `..` không gồm cận trên, `..=` gồm cận trên.

### 4.3 match và pattern

```vis
match s:
    Circle(r) if r > 100.0: "lớn"
    Circle(r) | Rect(r, _): "tròn {r}"
    Rect(w, h): "chữ nhật {w}x{h}"
    Empty: "rỗng"
```

- Phải xét hết; thiếu nhánh là lỗi kiểm kiểu, liệt kê nhánh thiếu.
- **Or-pattern `|` giữ nguyên** (QD §7.3): `Circle(r) | Rect(w, h)` trong một arm. `|` **không bao giờ** là toán tử nhị phân trong biểu thức (Q8 sửa); bit-or là hàm `bit_or`.
- **Guard không tính vào coverage**: nhánh có `if` **không** chứng minh được phủ, nên `Circle(r) if r > 100.0:` không làm `Circle` được coi là đã xét; phải có nhánh `Circle(r)` không guard hoặc `_`.
- **Kiểu vô hạn** (`int`, `float`, `str`, `path`, `datetime`...) **bắt buộc** có `_`; range pattern **không** dùng để chứng minh phủ kiểu vô hạn.
- Pattern đầy đủ: literal (kể cả `-1`), biến, `_`, tuple `(a, b)`, bản ghi `{name, age: 18..}`, enum `Circle(r)` / `Shape.Circle(r)`, list `[first, ..rest]`, range `0..10` / `0..=10`, or `|`, guard `if`. Pattern cho optional/Result: **`none`**, **`some(x)`**, **`ok(v)`**, **`err(e)`**.
- `if let PAT = EXPR:` là đường tắt của `match` một nhánh, **không** xét hết.

### 4.4 Optional

```vis
let e = user.email ?? "không có"
let domain = user.email?.split("@").last()
if let mail = user.email:
    send(mail)
```

- **Tự nâng `T → T?`** ở ba vị trí: **gán** (`let x: int? = 5`), **trả về** (`-> int?` với `5`), **đối số** (`f(x)` với tham số `int?`). Chiều ngược lại **không** có: `T?` không tự thành `T`.
- `T??` là **lỗi kiểu** (optional không lồng).
- `?` dùng được trên `T?` khi hàm đang viết trả `U?`; trên `Result[T, E]` khi trả `Result[U, F]` với `E: Error` (6.1).
- `?.` trên `T?` trả `T?`; `??` nhận toán hạng phải **cùng kiểu đã bỏ optional**.
- `.get(i)`, `.first()`, `.last()`, `xs[i]?` trả `T?`; `xs[i]` panic (4.5, Q5).

### 4.5 Toán tử

Số học `+ - * / % **`; so sánh `== != < <= > >=`; logic `and or not`; bit bằng hàm `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `shl`, `shr`, `ushr`; nối hàm `|>`: `xs |> filter(even) |> sum()`.

**`|` không bao giờ là toán tử nhị phân trong biểu thức** (Q8 sửa, QD §7.3). Biểu thức chỉ có `|>`. `|` còn nghĩa ở đúng hai chỗ: **or-pattern** (4.3) và **pipe của chế độ lệnh** (9.3). Hệ quả: bit-or **phải** viết `bit_or(a, b)`.

**Bảng ưu tiên 12 mức** [TK] (cao → thấp, đúng QD §7.3):

| Mức | Nhóm | Toán tử | Kết hợp |
|---|---|---|---|
| 1 | postfix | `.` `?.` `()` `[]` `?` `with` | trái |
| 2 | luỹ thừa | `**` | **phải** |
| 3 | một ngôi | `-` (đổi dấu) | phải |
| 4 | nhân | `* / %` | trái |
| 5 | cộng | `+ -` | trái |
| 6 | khoảng | `.. ..=` | không kết hợp |
| 7 | coalesce | `??` | phải |
| 8 | so sánh | `== != < <= > >=` | **không kết hợp** (`1 < x < 10` là lỗi cú pháp) |
| 9 | phủ định | `not` | phải |
| 10 | hội | `and` | trái, **ngắn mạch** |
| 11 | tuyển | `or` | trái, **ngắn mạch** |
| 12 | nối | `\|>` | trái |

Hệ quả, phải có golden test ở **ST-02a**:

- `-2 ** 2` = `-(2 ** 2)` = `-4` (một ngôi **thấp hơn** `**`).
- `2 ** 3 ** 2` = `2 ** (3 ** 2)` = `512`.
- `a ?? b == c` = `(a ?? b) == c`.
- `0..n+1` = `0..(n+1)`.
- `not a == b` = `not (a == b)`.
- `x? |> f` = `(x?) |> f`; `u with age: 21 |> f` = `(u with age: 21) |> f`.
- `a and b or c` = `(a and b) or c`.

- `and`/`or` **ngắn mạch** ("short-circuit"); đánh giá **trái sang phải** cho toán hạng, đối số, phần tử literal và trường bản ghi (4.6).
- `xs[i]` **panic** với thông điệp nêu chỉ số và độ dài; `xs[i]?` trả `T?` (checked, không panic); `xs[-1]` panic với thông điệp "chỉ số âm không hỗ trợ; dùng `xs.last()`".
- **Không có `s[i]`** trên `str` (lỗi kiểu, gợi ý `.chars()/.graphemes()/.byte_at(i)?`).
- `str * int` **không có**; dùng `s.repeat(n)` (QD §7.3).

### 4.6 Thứ tự đánh giá

- **Đánh giá trái sang phải** [TK], tất định: toán hạng nhị phân, phần tử trong literal list/map/set/tuple, trường trong bản ghi, đối số trong lời gọi (vị trí trước, tên sau, giữ nguyên thứ tự viết).
- `and`/`or` ngắn mạch: toán hạng phải **không được đánh giá** khi trái quyết định.
- `??` ngắn mạch theo cùng luật: toán hạng phải chỉ đánh giá khi trái là `none`.
- Thứ tự này là một phần của đặc tả: bộ vi sai interpreter/VM (18.2) dùng nó làm oracle.

---

## 5. Hàm, closure, generic, trait

### 5.1 Hàm

```vis
pub fn greet(name: str, times: int = 1, --loud: bool = false) -> str:
    let s = "Xin chào {name}".repeat(times)
    if loud: s.upper() else: s
```

- Tham số vị trí, tham số mặc định, tham số có tên bắt đầu bằng `--` (chỉ truyền theo tên; thành cờ trong chế độ lệnh). Gọi: `greet("An", 2, --loud: true)`.
- **Thứ tự đối số:** vị trí trước, tên sau; tên **không trùng**; không được bỏ tham số vị trí bắt buộc; `...rest: T` (nếu có) đứng **cuối**.
- Biểu thức cuối khối là giá trị trả về; `return` để trả sớm.
- Tham số **bất biến**; chỉ `var self` (5.4) là ngoại lệ.
- Không có quá tải theo kiểu; cùng tên trong cùng scope là lỗi (4.1).

### 5.2 Closure

`x => x * 2`, `(a, b) => a + b`, hoặc nhiều dòng:

```vis
let f = fn(x: int) -> int:
    let y = x * 2
    y + 1
```

Closure bắt biến theo giá trị. Muốn chia sẻ trạng thái biến đổi: `ref[T]`. Hai dạng closure (`x => expr` và `fn(...) -> T:` nhiều dòng) là **cùng một kiểu** `fn(A) -> B`.

### 5.3 Generic

```vis
fn first[T](xs: list[T]) -> T?:
    if xs.is_empty(): none else: xs[0]

fn max_by[T, K: Ord](xs: list[T], key: fn(T) -> K) -> T?: ...
```

Chỉ định tường minh khi cần: `json.parse[list[User]](text)`.

- **Generic vs index** (QD §7.3): `name[types](args)` chỉ được hiểu là **danh sách kiểu** khi ngay sau `]` là `(` **và** `name` là hàm generic đã biết; ngược lại là **index**. Nhờ vậy `xs[i]` và `f[T](x)` không mập mờ.

### 5.4 Trait

```vis
trait Show:
    fn show(self) -> str

impl Show for Shape:
    fn show(self) -> str: ...

trait Storage:
    fn get(self, key: str) -> bytes?
    fn put(var self, key: str, value: bytes)
```

- `self` bất biến, `var self` cho phương thức sửa.
- Ràng buộc generic `[T: Show]` hoặc giá trị động `dyn Show`.
- Không kế thừa cài đặt; trait có phương thức mặc định.
- Quy tắc mồ côi: `impl Trait for Type` chỉ trong module định nghĩa Trait hoặc Type.
- `trait Error: Show { message, source, span }` là trait lõi (6.1).

### 5.5 Phương thức

`impl User:` thêm phương thức. Gọi `u.method()`. Không UFCS. Phương thức thắng hàm trần cùng tên; hàm trần phải `use` tường minh.

---

## 6. Xử lý lỗi

### 6.1 Result, `?`, trait `Error`

```vis
fn read_config(dir: cap[Dir]) -> Result[Config, AnyError]:
    let text = dir.read_text("config.json")?
    let cfg = json.parse[Config](text)?
    Ok(cfg)
```

- `trait Error: Show { message, source, span }` (QD §7.3): mọi kiểu lỗi in được (`Show`) và cung cấp `message: str`, `source: Error?` (nguyên nhân lồng), `span: Span?` (vị trí nguồn).
- `AnyError` là **kiểu xoá kiểu** (boxed `dyn` là chi tiết cài đặt): bọc mọi `E: Error`, dùng ở ranh giới (main, plugin, task).
- **Mọi `E: Error` tự chuyển sang `AnyError` qua `?`** — không cần `impl Into`. `?` trên `Result[T, E]` trong hàm trả `Result[U, AnyError]` hợp lệ khi `E: Error`.
- `?` trên `T?` chỉ trong hàm trả `U?` (4.4). `?` **không** tự bọc `Result` vào `Result` lồng; `T??` là lỗi kiểu.
- `main -> Result[(), AnyError]`. Phía plugin Rust, tên của cùng trait là **`vis::Error`** (17).

### 6.2 `else` xử lý tại chỗ

```vis
let cfg = read_config(dir) else err:
    print("Dùng cấu hình mặc định: {err}")
    Config.default()
```

Khối `else` phải trả giá trị cùng kiểu hoặc thoát (`return`, `break`, `panic`).

### 6.3 Optional sang Result

`x.ok_or("thiếu email")?`

### 6.4 panic

Cho vi phạm bất biến, kể cả lỗi số học: tràn, chia 0, `%0`, mũ âm với `int`, chỉ số ngoài phạm vi, `assert`. **Payload của panic là `AnyError`** (QD §7.3): `panic(err)` nhận `E: Error`; `panic("chuỗi")` là đường tắt tạo `AnyError` với `message`. panic dừng task hiện tại, in vết ngăn xếp có vị trí nguồn. Không bắt được trong code thường; chỉ ranh giới task/shell bắt để báo cáo.

### 6.5 Lỗi trong `main` và mã thoát

`fn main(...) -> Result[(), AnyError]`: lỗi được in đẹp, mã thoát 1.

| Mã | Nghĩa | Nguồn |
|---|---|---|
| 0 | Thành công | `main` trả `Ok`, hoặc lệnh shell xong bình thường |
| 1 | Lỗi từ `main` | `Err` của `main` |
| 2 | Lỗi dùng / lỗi kiểu / lỗi cú pháp | `vis` trước khi chạy (cờ sai, tệp sai) |
| 101 | panic | task panic; ViShell đặt mã này cho lệnh bị panic |
| 126 | Không chạy được (không phải tệp thực thi, thiếu quyền) | host |
| 127 | Không tìm thấy lệnh | host |
| 130 | Bị huỷ (Ctrl-C, `Cancel`) | tín hiệu, job control |

Bảng này đúng QD §7.3 và khớp quy ước POSIX cho 126/127/130; `| complete` trả `code`/`how` theo cùng phân loại (9.5).

### 6.6 Ranh giới lệnh trong ViShell

- ViShell bọc **mỗi lệnh** trong **một task có heap arena riêng** (QD §7.3). Ranh giới lỗi là **lệnh**, không phải tiến trình và không phải phiên.
- panic giải phóng arena của lệnh, in trace + span, đặt mã **101**, và **giữ** biến phiên, cwd, job, lịch sử.
- Tài nguyên mà lệnh mở (tệp, tiến trình con, kênh) là **cap của phiên**, đăng ký vào arena của lệnh và **tự đóng** khi arena giải phóng.
- **Chưa thêm `defer`** ở bản này (QD §7.3); nếu sau này cần là thay đổi ngữ nghĩa phải qua Agent 0 (VD-08).

---

## 7. Mô hình giá trị và bộ nhớ

### 7.1 Ngữ nghĩa giá trị

Mọi kiểu (trừ `ref`, `cap`, `stream`, kênh) có ngữ nghĩa giá trị:

```vis
var a = [1, 2, 3]
var b = a
b.push(4)        # a vẫn là [1, 2, 3]
```

Cài đặt: đối tượng heap đếm tham chiếu; ghi vào đối tượng có số tham chiếu > 1 thì sao chép trước (copy-on-write).

### 7.2 Không GC truy vết

Ngữ nghĩa giá trị không tạo được chu trình. Chu trình chỉ có qua `ref[T]`; dùng `weak[T]` để phá. Bản debug có bộ phát hiện chu trình báo rò rỉ trong test. Không GC truy vết nên không có khoảng dừng, phù hợp giao diện và âm thanh.

### 7.3 `ref[T]`

```vis
let counter = ref(0)
let inc = () => counter.set(counter.get() + 1)
```

Chỉ dùng trong một luồng. Chia sẻ giữa luồng: kênh hoặc `sync.Mutex[T]`.

### 7.4 Gửi giữa luồng

Giá trị có ngữ nghĩa giá trị là `Send`. Khi gửi, nếu chỉ có một tham chiếu thì chuyển quyền sở hữu, ngược lại sao chép sâu. Mỗi luồng có heap và bộ đếm tham chiếu không atomic riêng.

---

## 8. Module và gói

- Mỗi tệp là một module. `use std.fs`, `use ./util`, `use pkg.http`.
- `pub` để xuất; mặc định riêng tư.
- **Không có `as`** (đã bỏ khỏi từ khoá, 2.1): muốn tên ngắn thì `use` đúng mục cụ thể.
- Cấm vòng phụ thuộc giữa module.
- Gói: thư mục có `vis.toml` (tên, phiên bản, giấy phép, phụ thuộc theo đường dẫn, git URL + commit, hoặc registry sau này), `vis.lock` khoá phiên bản + hash nội dung.
- Script một tệp khai phụ thuộc ở đầu tệp:

```vis
#!/usr/bin/env vis
#| deps: http = "git+https://example.org/vis-http@a1b2c3"
```

- Gói không có mã chạy lúc cài đặt.
- Hash của **closure phụ thuộc** (tệp chính + mọi phụ thuộc + phiên bản compiler) là khoá của cache (14.5) **và** của consent quyền (11.3).

---

## 9. Chế độ lệnh (shell)

### 9.1 Ý tưởng và mô hình **hai pha**

Chế độ lệnh là **cú pháp thứ hai**, và được xử lý thành **hai pha** (QD §1 **Q-D**):

1. **`vis-syntax` → `CmdCST`**: lex + parser chế độ lệnh sinh CST **chưa định kiểu**; đơn vị cơ bản là `Word` giữ **nguyên văn** từ trần (để báo lỗi và để `vish-edit` tô màu). Ở pha này chưa biết `5mb` là `size` hay `str`, chưa biết `size` là trường hay tên lệnh.
2. **`cmd_lower` (trong `vis-types`) → HIR**: dùng **bảng chữ ký** để quyết định: literal theo kiểu tham số, glob theo kiểu tham số, `it` + viết tắt trường, `tên cmd` vs chương trình ngoài, binding `--flag`, `--help`.

Hệ quả thuật ngữ: hai chế độ ra **cùng HIR**, không phải "cùng AST" (sửa câu cũ ở 1.1 và 18.2). Dùng ở:
- Mỗi dòng gõ trong ViShell.
- Khối `cmd { ... }` trong tệp `.vis`.
- Tệp `.vish`.

Bảng "luật → pha":

| Luật | Pha |
|---|---|
| tách `word`, `"..."`, `'...'`, `$x`, `$env.NAME`, `--flag`, `--`, `;`, `&`, `\|`, `^` | `vis-syntax` (`CmdCST`) |
| `CmdCST` + vị trí nguồn + golden test từng hàng bảng 9.4 | `vis-syntax` |
| `5mb` → `size` theo kiểu tham số | `cmd_lower` |
| `*.txt` → glob theo kiểu tham số (`list[path]`/`glob`) | `cmd_lower` |
| `where size > 5mb` → `where it => it.size > 5mb` | `cmd_lower` |
| `ls` là `cmd` nội bộ, biến hàm, hay chương trình ngoài | `cmd_lower` |
| `--flag` binding vào tham số, sinh `--help` | `cmd_lower` |
| `-5` là số hay cờ (theo kiểu tham số) | `cmd_lower` |

### 9.2 Khai báo lệnh

```vis
## Liệt kê tệp
pub cmd ls(path: path = p".", --all: bool = false, --long: bool = false) -> stream[FileEntry]:
    ...
```

- `cmd` là hàm bình thường, cộng thêm: tự sinh `--help`, gọi được bằng cú pháp lệnh, tham số `--x` thành cờ, `bool` thành cờ không giá trị.
- Lệnh nhận `ctx: Ctx` ngầm (thư mục hiện tại, môi trường, terminal, quyền của phiên). Hàm thường không có `ctx`.
- Tham số mặc định kiểu `path` viết `p"."` (3.5), không viết `"."`.

### 9.3 Phân tích một dòng lệnh

```
pipeline   := stage ('|' stage)*  ['&']
stage      := command arg*   |   '(' expr ')'
command    := ten_cmd | bien_ham | chuong_trinh_ngoai | '^' ten_cmd
arg        := word | '--flag' [value] | '$' ident ('.' ident)* | '$env' '.' ident | '(' expr ')' | string | closure_arg
shell_item := use | fn | cmd | shell_let | pipeline    // .vish và REPL
shell_let  := 'let' IDENT [':' type] '=' pipeline NEWLINE   // vế phải là pipeline chế độ lệnh
```

- **Metachar**: khoảng trắng, `( ) [ ] { } ; & $ ' " |`; `#` khi đứng đầu từ. Từ trần là chuỗi liên tục không chứa metachar. `"..."` có nội suy; `'...'` thô; `\` chỉ trong chuỗi.
- `$x` biến phiên; `$env.NAME` biến môi trường. `;` tách pipeline; `&` nền chỉ khi là token cuối và có khoảng trắng trước; `--` kết thúc cờ; `-5` là số nếu tham số kiểu số, ngược lại là cờ.
- **Tham số hiểu theo chữ ký lệnh — pha `cmd_lower` (QD §1 Q-D)**: `vis-syntax` sinh `CmdCST` với từ trần **chưa định kiểu**; `cmd_lower` trong `vis-types` dùng bảng chữ ký quyết `./photos`→`path`, `5mb`→`size`, glob theo kiểu, `it` + viết tắt trường, `--flag` binding. Không hard-code chữ ký trong parser.
- Chương trình ngoài: mọi tham số là `str`, glob mở rộng trước khi truyền; `^ten` ép gọi ngoài.
- Closure-arg kéo tới `|` kế tiếp ở **độ sâu ngoặc 0**; or-pattern trong một stage phải đặt trong ngoặc.
- `from json` không tham số trả `Json`; `where` trên `Json` là lỗi kiểu, gợi ý `from json[T]`.

### 9.4 Xung đột ký hiệu và cách giải

| Ký hiệu | Chế độ lệnh | Chế độ script |
|---|---|---|
| `\|` | pipe ở độ sâu 0 | or-pattern trong pattern; **không** là toán tử nhị phân |
| `\|>` | — | pipe biểu thức |
| `>` `<` | so sánh trong tham số biểu thức; không redirect | so sánh |
| `#` | chú thích khi đứng đầu từ; `https://x#y` giữ nguyên | chú thích |
| `%` | modulo; jobspec dùng `job 1` | modulo |
| `*` `**` | glob (một/đệ quy) | nhân/luỹ thừa |
| `-x --x --` | cờ; `--` kết thúc cờ; `-5` là số nếu kiểu số | trừ |
| `$` | `$x`, `$env.NAME` | không dùng |
| `[` `(` | biểu thức/list literal phải trong `( )`; glob char-class | index/gọi |
| `;` `&` | tách pipeline / nền | không dùng |
| `^ten` | ép gọi chương trình ngoài | không dùng |
| Ghi tệp | `\| save out.txt` | `file.write(...)` |

### 9.5 Luồng dữ liệu

- `stream[T]` lười, kéo, có backpressure, huỷ khi bị bỏ. `take 10` dừng nguồn sớm; SIGPIPE/early-close do `take` **không phải lỗi** pipeline.
- Chương trình ngoài: stdout là `stream[bytes]`; chuyển đổi bằng `| lines`, `| from json[T]`, `| from csv`, `| from toml`; ngược lại `| to json`.
- Mã thoát khác 0 thành lỗi pipeline; `| complete` **chặn** lỗi pipeline và trả `{stdout, stderr, code, how}`.
- Khai chữ ký cho chương trình ngoài:

```vis
extern cmd git(sub: GitSub, --oneline: bool, ...rest: str)
```

### 9.6 Hiển thị

Giá trị cuối pipeline được hiển thị qua trait `Render`: `list` bản ghi thành bảng, bản ghi thành khối khoá-giá trị, `stream[bytes]` in thẳng. ViTerm trên ViFastOs nhận dữ liệu có cấu trúc thay vì văn bản.

### 9.7 Ví dụ

```vish-session
ls ./photos | where size > 5mb | sort_by modified --desc | take 10
let big = ls ./photos | where size > 5mb
big | each f => f.copy_to(p"./backup")
open users.json | from json[User] | where age >= 18 | get name
git log --oneline | lines | take 5
ps | where cpu > 50pct | each p => p.kill()
http get https://api.example.com/items | from json[Item] | to csv | save items.csv
```

Trong script:

```vis
fn backup_big(ctx: Ctx, dir: path) -> Result[int, AnyError]:
    let files = cmd { ls (dir) | where size > 5mb } |> collect()
    for f in files:
        f.copy_to(p"./backup")?
    Ok(files.len())
```

---

## 10. Bất đồng bộ và song song

### 10.1 async có cấu trúc

```vis
async fn fetch_all(net: cap[Net], urls: list[str]) -> list[Result[str, AnyError]]:
    scope s:
        let tasks = urls.map(u => s.spawn(net.get_text(u)))
        tasks.map(t => await t)
```

- **Kiểu:** `Future[T]` là **kết quả lười**; `Task[T]` là future **đã spawn** (có handle). Gọi một `async fn` **chỉ tạo `Future[T]`, không chạy** (QD §7.3); việc chạy bắt đầu ở `await` hoặc ở `s.spawn`.
- `s.spawn(fut: Future[T]) -> Task[T]` (QD §7.3); `await t: Task[T] -> T`; `await f: Future[T]` chạy nó.
- Mọi task con nằm trong `scope`; ra khỏi `scope` thì task con đã xong hoặc bị huỷ. Không task mồ côi.
- Huỷ (Ctrl-C, timeout) lan theo cây `scope`, điểm huỷ tại mỗi `await`.
- **Panic trong scope** (QD §7.3): **huỷ các task anh em** trong scope trước, rồi **lan** lên scope cha. Đây là ngữ nghĩa bắt buộc, có test ở 18.2.
- `timeout(5s, fut)`, `select` trên nhiều future/kênh.

### 10.2 Kênh

`chan[T](capacity)` trả `(Sender[T], Receiver[T])`. Receiver là `stream[T]`.

### 10.3 Song song thật

`parallel.map(xs, f)` và `thread.spawn` chạy trên luồng OS; giá trị qua lại phải `Send`. Mỗi luồng một VM với heap riêng.

### 10.4 Lớp dưới

Vòng sự kiện trong runtime: POSIX dùng epoll/kqueue, Windows IOCP, ViFastOs notification + ring. Khác biệt nằm trong `Host`.

---

## 11. Capability và quyền

### 11.1 Mô hình

- Không hàm toàn cục nào chạm hệ thống. Không có `fs.read_text("x")`; phải là `dir.read_text("x")` với `dir: cap[Dir]`.
- Capability chỉ có ba nguồn: tham số `main`, `ctx` của phiên shell, hoặc dẫn xuất hẹp hơn từ capability đang có (`dir.subdir("logs")`, `net.restrict("api.example.com")`).
- Hệ kiểu bảo đảm thư viện chỉ có quyền được truyền vào.
- `print` là stdio của **chính tiến trình**, không phải capability; `sh "..."` cần `cap[Proc]` (QD §7.3).

### 11.2 Khai báo trong `main`

```vis
fn main(data: cap[Dir] @ p"./data", out: cap[Dir] @ p"./out" (write), net: cap[Net] @ "api.example.com") -> Result[(), AnyError]:
    ...
```

- `@` khai đường dẫn/host mặc định yêu cầu: `cap[Dir]` dùng literal `path` (`p"./data"`), `cap[Net]` dùng **chuỗi host thường** (`"api.example.com"`, không phải path). Kiểm loại ở pha kiểm kiểu. Ghi đè bằng `vis run app.vis --grant data=./other`.
- `vis run --dry` in danh sách quyền chương trình yêu cầu.

### 11.3 Shell

- Phiên ViShell có quyền của người dùng (`ctx`).
- Chạy một tệp `.vis` từ shell: chỉ trao quyền nó khai trong `main`; shell hỏi khi quyền lần đầu được dùng và **nhớ consent theo hash closure** (QD §7.3): hash = **nguồn tệp chính + toàn bộ phụ thuộc (`use`/`deps`) + phiên bản compiler**, dùng **chung hash cache** với 14.5. Nhờ vậy sửa `./util.vis` là consent cũ **mất hiệu lực** — vá lỗ hổng "theo hash tệp chính, bỏ qua phụ thuộc".
- `cmd` trong cấu hình shell chạy với `ctx` của phiên.

### 11.4 Mức bảo đảm theo nền tảng

| Nền tảng | Bảo đảm bởi |
|---|---|
| ViFastOs | Kernel (capability thật); runtime chỉ giữ cap được trao |
| Linux | Runtime kiểm; tăng cường bằng Landlock + seccomp khi có |
| macOS | Runtime kiểm [GT: khả năng sandbox của hệ điều hành] |
| Windows | Runtime kiểm; AppContainer sau |
| Wasm | Môi trường trình duyệt |

Ở chế độ hạn chế quyền: **plugin động bị tắt**, **plugin tĩnh chỉ có trong bản `vis` người dùng tự build** (17).

---

## 12. Trait Host (đa nền tảng)

```rust
pub trait Host: Send + Sync {
    // tệp: luôn tương đối với một handle thư mục (kiểu openat)
    fn dir_open(&self, base: &DirHandle, rel: &VPath, mode: OpenMode) -> HResult<Handle>;
    fn dir_list(&self, d: &DirHandle) -> HResult<DirIter>;
    fn file_read(&self, f: &Handle, buf: &mut [u8], off: Option<u64>) -> HResult<usize>;
    fn file_write(&self, f: &Handle, buf: &[u8], off: Option<u64>) -> HResult<usize>;
    fn stat(&self, base: &DirHandle, rel: &VPath) -> HResult<Meta>;
    // rename, remove, mkdir, watch ...
    // tiến trình: stdio là ỐNG (pipe); không có pty ở bản này
    fn spawn(&self, spec: &ProcSpec) -> HResult<ProcHandle>;     // argv, env, cwd, pipe stdio
    // list_procs ...; pty/tty server: chưa có ở bản đầu, xem §15
    fn proc_wait(&self, p: &ProcHandle) -> HResult<ProcStatus>;   // { code: i32, how: Exited|Cancelled|Faulted|Killed }
    fn proc_suspend(&self, p: &ProcHandle) -> HResult<()>;
    fn proc_resume(&self, p: &ProcHandle) -> HResult<()>;
    fn proc_cancel(&self, p: &ProcHandle) -> HResult<()>;
    // mạng: tcp_connect, tcp_listen, udp_bind, dns_resolve ...
    // thời gian
    fn now_mono(&self) -> Instant;
    fn now_wall(&self) -> DateTime;
    // môi trường, terminal: env_get, env_vars, term_size, term_raw_mode, term_read_key ...
    fn reactor(&self) -> &dyn Reactor;
    fn sandbox_harden(&self, grants: &Grants) -> HResult<()>;
}
```

- **Tiến trình** (QD §3.1): `spawn` dựng stdio bằng **ống/pipe**, không cấp `cap[Term]` cho con; `proc_wait` trả **`{code, how}`** với `how` dùng chung với `| complete` (9.5) và job control (15); `proc_suspend/resume/cancel` là API bắt buộc của backend.
- **pty chưa có** ở bản này; pty + controlling terminal để **tty server** sau (15.1). Vì vậy chương trình toàn màn hình chưa hứa chạy.
- **Trên ViFastOs**, các thao tác tiến trình ở trên là **RPC tới `procsrv`**, không phải syscall: kernel chỉ có `Exit` (syscall 14) và `Tcb.Suspend/Resume`. Đừng đi tìm syscall `wait`.
- Đường dẫn trong lõi là `VPath` (luôn `/`, UTF-8; tên không UTF-8 giữ dạng bytes thoát). Backend chuyển sang đường dẫn gốc.
- Backend: `vis-host-posix` (Linux, macOS, BSD), `vis-host-windows`, `vis-host-wasm`, `vis-host-vifast` (workspace ngoài `viscript/`), `vis-host-mock` (test, tất định).
- **Shell sở hữu tty**; tiến trình con nhận pipe. Chương trình toàn màn hình (vim/less) để sau với tty server riêng (QD §3.1).
- **Bộ hợp chuẩn Host** chạy giống nhau trên mọi backend (mục 18).

---

## 13. Thư viện chuẩn

| Module | Nội dung |
|---|---|
| `core` | kiểu dựng sẵn, helper Option/Result, trait chuẩn (`Show`, `Error`, `Render`), `Future`/`Task` |
| `text` | chuỗi, grapheme, chuẩn hoá Unicode, regex, định dạng, so sánh theo locale cơ bản |
| `coll` | list, map, set, deque, heap, sắp xếp ổn định |
| `stream` | map, filter, take, skip, chunk, zip, group_by, sort_by, uniq, sum, count, collect |
| `fs` | Dir, File, đọc/ghi, duyệt, watch, glob |
| `path` | xử lý `path` |
| `proc` | chạy chương trình, pipe, suspend/resume/cancel, mã thoát + `how` |
| `net` | TCP, UDP, DNS, `http` client, TLS (thuần Rust) |
| `time` | `now()`, instant, datetime, múi giờ, duration |
| `fmt` | json, toml, csv, base64, hex (yaml là thư viện ngoài) |
| `crypto` | hash (SHA-2, BLAKE3), random an toàn |
| `term` | màu, bảng, prompt, tiến độ |
| `testing` | `test fn`, assert, snapshot (đổi tên từ `test`, QD §7.3) |
| `sync` | Mutex, kênh, Once |
| `shell` | `cmd` chuẩn: ls, cd, open, save, where, each, sort_by, get, select, take, lines, from, to, ps, kill, http, walk, ... |
| `vifast` | chỉ có trên ViFastOs: binding sinh từ IDL |

Viết bằng ViScript nhiều nhất có thể; phần cần nhanh (regex, hash, json parse) là Rust trong runtime. Phụ thuộc Rust phải thuần Rust và giấy phép tương thích MIT/Apache-2.0.

---

## 14. Kiến trúc cài đặt

### 14.1 Đường xử lý

```
nguồn (.vis / .vish / dòng lệnh)
  └─ chuẩn hoá NFC (2.1)
       └─ lexer (2 chế độ, trạng thái thụt lề, VS0107)
            └─ parser (đệ quy xuống, phục hồi lỗi) ──> CST không mất thông tin (cho fmt, LSP)
                 ├─ (script) hạ xuống AST
                 └─ (lệnh)  CmdCST: `Word` chưa định kiểu
                      └─ phân giải tên + module
                           └─ kiểm kiểu + `cmd_lower` (bảng chữ ký)
                                └─ HIR có kiểu
                                     ├─ interpreter cây (mô hình tham chiếu)
                                     └─ sinh bytecode ──> xác minh bytecode ──> VM
                                            └─ cache theo hash closure
```

`cmd_lower` nằm trong `vis-types` (không phải trong parser) để giữ DAG crate: parser không cần bảng chữ ký, còn `vis-types` đã có kiểu.

### 14.2 Crate

| Crate | Vai trò | Phụ thuộc |
|---|---|---|
| `vis-syntax` | lexer, parser, CST, AST, `CmdCST`, vị trí nguồn | |
| `vis-diag` | chẩn đoán, mã lỗi, bản dịch vi/en/ja | |
| `vis-types` | phân giải tên, kiểm kiểu, `cmd_lower`, HIR | syntax, diag |
| `vis-interp` | interpreter cây trên HIR (mô hình tham chiếu, đơn giản) | types |
| `vis-bytecode` | định dạng bytecode, sinh mã, xác minh | types |
| `vis-vm` | VM, heap RC/CoW, scheduler task, FFI | bytecode |
| `vis-host` | trait Host + `vis-host-mock` | |
| `vis-host-{posix,windows,wasm}` | backend | host |
| `vis-std` | thư viện chuẩn (.vis + Rust) | vm, host |
| `vis-fmt` | định dạng mã từ CST | syntax |
| `vis-lsp` | language server | types, fmt |
| `vis-cli` | lệnh `vis` | tất cả |
| `vish-edit` | soạn dòng, tô màu, gợi ý | syntax, types |
| `vishell` | shell | cli, edit |
| `vis-plugin` | macro + ABI plugin Rust | vm |

- **`vis-host-vifast` KHÔNG nằm trong `viscript/`** (QD §7.5 ST-01): nó ở **workspace ngoài** vì phụ thuộc IDL của ViFastOs; `viscript/` chỉ chứa crate thuần (lõi + 3 backend còn lại + mock). Cổng ST-01 kiểm điều này bằng case âm.
- **SPDX theo loại tệp** (QD §9.3): `//` cho Rust/asm, `#` cho `.vis .vish .vidl .toml`; tệp không có chú thích dùng sidecar `.license`; tệp sinh do generator phát header.
- **Giấy phép (D2-21)**: MIT OR Apache-2.0; phụ thuộc qua `cargo-deny` với allowlist MIT, Apache-2.0, BSD-2/3-Clause, ISC, 0BSD, Zlib, Unicode-3.0, Unicode-DFS-2016; cấm MPL/GPL/LGPL; `THIRD-PARTY.md` sinh tự động; LICENSE-MIT/APACHE một bản ở gốc repo; **không** tạo NOTICE.
- Luật: `vis-syntax`, `vis-types`, `vis-bytecode`, `vis-vm` không phụ thuộc hệ điều hành (cổng build target wasm32). **Tệp không quá 1.500 dòng** [TK]. `unsafe` chỉ trong **danh sách crate đóng** (`vis-vm` cho heap, `vis-plugin` cho ABI), có Miri; **mỗi luật có case âm** chứng minh cổng bắt được vi phạm.

### 14.3 Kiểm kiểu

- Phân giải tên hai lượt (khai báo trước, thân sau).
- Suy kiểu hai chiều, biến kiểu trong phạm vi một hàm; hợp nhất cục bộ.
- Trait: tra cài đặt theo kiểu cụ thể; generic kiểm một lần với ràng buộc.
- Xét hết `match` bằng thuật toán ma trận pattern; **guard không tính coverage**, **kiểu vô hạn bắt buộc `_`** (4.3).
- Phục hồi lỗi: kiểu `{error}` lan truyền im lặng, chỉ báo lỗi gốc.
- Kiểm tăng dần cho LSP và shell: cache theo module.
- Sau khi có chữ ký, `cmd_lower` kiểm literal/glob/`it` của chế độ lệnh rồi mới sang HIR (9.1).

### 14.4 VM

- **Bytecode thanh ghi**, khung hàm có số thanh ghi cố định.
- Ô 64 bit: số nguyên, số thực, bool lưu thẳng; còn lại là con trỏ heap. Nhờ kiểu tĩnh, mỗi hàm có **bản đồ thanh ghi con trỏ** sinh lúc biên dịch; VM không cần thẻ kiểu lúc chạy.
- Generic: biểu diễn đồng nhất + **bảng chứng cứ trait** truyền ngầm (kiểu Swift). Chuyên biệt hoá hàm nóng là tối ưu sau, khi có số đo.
- Lệnh có kiểu (`add_i`, `add_f`, `call_static`, `call_trait`).
- Trình xác minh bytecode: bản đồ con trỏ, đích nhảy, kiểu thanh ghi; bytecode từ cache cũng phải qua xác minh.
- Heap theo luồng, RC không atomic, CoW cho list/map/str/bản ghi.
- Task async là coroutine không ngăn xếp (khung lưu trên heap).
- Giới hạn tài nguyên tuỳ chọn: số lệnh, bộ nhớ, độ sâu đệ quy.

### 14.5 Cache

`<thư mục cache>/vis/<hash>.vbc`, hash = **hash closure**: nguồn tệp chính + toàn bộ phụ thuộc + phiên bản trình biên dịch (cùng hash với consent ở 11.3). Mục tiêu khởi động script đã cache dưới 20 ms [ƯL].

### 14.6 Chẩn đoán

```
error[VS0412]: kiểu không khớp
  ┌─ app.vis:12:18
12│     let n: int = user.email
  │                  ^^^^^^^^^^ đây là `str?`, cần `int`
  = gợi ý: dùng `int.parse(user.email ?? "0")?` nếu email chứa số
```

Mã lỗi ổn định (`VS` + 4 chữ số, quy tắc cấp ở `ST-02b`), `vis explain VS0412`. Cột theo Unicode scalar (2.4). Ngôn ngữ thông báo theo cấu hình (mặc định theo locale hệ thống).

---

## 15. Kiến trúc ViShell

```
┌────────────────────────────────────────────────────────────┐
│ ViShell                                                     │
│  vish-edit: soạn dòng, tô màu theo CST, gợi ý theo kiểu,     │
│             gợi ý từ lịch sử, nhiều dòng, phím vi/emacs      │
│  phiên: biến, ctx (cwd, env, quyền), job, lịch sử            │
│  cấu hình: ~/.config/vishell/config.vis (prompt là hàm)      │
├────────────────────────────────────────────────────────────┤
│ vis-types (kiểm tăng dần + cmd_lower) │ vis-vm │ vis-std.shell │
├────────────────────────────────────────────────────────────┤
│ Terminal trait: ANSI (POSIX) │ Windows console │ ViTerm      │
└────────────────────────────────────────────────────────────┘
```

| Thành phần | Thiết kế |
|---|---|
| REPL | đọc dòng, phân tích chế độ lệnh, kiểm kiểu, chạy, hiển thị; lỗi kiểu thì không chạy gì; `DEDENT` ngầm ở cuối mỗi input (2.2) |
| Biến phiên | `let` trong shell sống hết phiên |
| Gợi ý | theo kiểu tham số: `path` gợi ý tệp, `enum` gợi ý biến thể, trường bản ghi trong `where`, cờ của `cmd`, lệnh ngoài theo `extern cmd` |
| Tô màu | theo CST + kết quả phân giải (lệnh không tồn tại tô đỏ) |
| Lịch sử | append-only + chỉ mục; lưu dòng, cwd, thời điểm, thời lượng, mã thoát; tìm mờ |
| Job | `&`, `jobs`, `fg`, `bg`, `kill` — POSIX dùng process group; ViFastOs qua **RPC procsrv** (15.1) |
| Huỷ | Ctrl-C huỷ pipeline qua token huỷ lan theo stream và scope |
| Tương thích | `^cmd` ép gọi chương trình ngoài; `sh "..."` chạy chuỗi qua `/bin/sh` khi thật cần, cần `cap[Proc]` |
| Prompt | hàm ViScript, có giới hạn thời gian |
| Khởi động | cấu hình cache bytecode; mục tiêu dưới 50 ms [ƯL] |
| Terminal | ViShell không tự là terminal; ViTerm nhận giao thức hiển thị có cấu trúc tuỳ chọn |

### 15.1 Job control và terminal

- **Shell sở hữu tty** (QD §3.1, §7.3, §15): ViShell giữ `cap[Term]`; tiến trình con nhận **pipe**, **không** nhận `cap[Term]`.
- Job control `&`, `jobs`, `fg`, `bg`, `kill` là lệnh của shell, thực hiện qua **RPC `procsrv`** (trên ViFastOs) hoặc process group (POSIX). RPC procsrv: `Spawn`, `Wait(pid|gid) -> {code, how}`, `Stop(gid)`, `Cont(gid)`, `Cancel(gid)`, `List`.
- **Ctrl-C huỷ pipeline**: huỷ từ **stage cuối về đầu**, chờ tối đa `T` [ƯL 2 s], rồi finalise; mã thoát **130**.
- **Chương trình toàn màn hình** (vim, less) **để sau**: cần **tty server** riêng cấp pty + controlling terminal. Bản này không hứa; `fg` chỉ hứa với chương trình dùng pipe.
- Ranh giới panic/arena của lệnh: xem 6.6.

---

## 16. Công cụ

| Lệnh | Chức năng |
|---|---|
| `vis run f.vis [args]` | kiểm kiểu + chạy |
| `vis check` | chỉ kiểm |
| `vis fmt` | định dạng (một kiểu duy nhất: 4 dấu cách, sửa tab) |
| `vis test` | chạy `test fn`, snapshot |
| `vis doc` | sinh tài liệu từ `##` |
| `vis explain VSxxxx` | giải thích mã lỗi |
| `vis lsp` | language server (extension VS Code mỏng) |
| `vis new`, `vis add` | tạo gói, thêm phụ thuộc |
| `vis repl` / `vish` | REPL thuần / ViShell |

---

## 17. Plugin Rust

```rust
#[vis::module(name = "image")]
mod image {
    #[vis::func]
    pub fn resize(data: Bytes, w: u32, h: u32) -> Result<Bytes, vis::Error> { ... }
}
```

- Macro sinh chữ ký ViScript từ chữ ký Rust, kiểm lúc build. Lỗi plugin dùng **`vis::Error`** — tên phía Rust của `trait Error` (6.1).
- Nạp: **liên kết tĩnh** vào bản `vis` tuỳ biến, hoặc **thư viện động** với ABI ổn định có số phiên bản.
- **Chế độ hạn chế quyền** (QD §7.3): **plugin động bị tắt**; **plugin tĩnh chỉ có** trong bản `vis` do **người dùng tự build** — bản phát hành mặc định không kèm plugin tĩnh nào.
- Plugin không nhận quyền ngầm; muốn chạm tệp phải nhận `cap[...]`.

---

## 18. Kiểm thử theo từng phần

### 18.1 Tầng

| Tầng | Nội dung | Mục tiêu |
|---|---|---|
| L0 | unit, property test từng crate | < 15 s |
| L1 | bộ test ngữ nghĩa `.vis` trên interpreter và VM | < 60 s |
| L2 | vi sai interpreter/VM trên chương trình sinh ngẫu nhiên; fuzz parser/typechecker; hợp chuẩn Host trên máy hiện tại | < 5 phút |
| L3 | ma trận Linux, macOS, Windows, wasm (và ViFastOs khi có) | CI |
| L4 | fuzz dài, benchmark, Miri | đêm |

### 18.2 Theo từng phần

| Phần | Cách kiểm |
|---|---|
| Lexer | bảng token vàng (60 dòng ở `SPEC-lex.md`); fuzz không panic; **thụt lề suy đơn vị**, **tab = `VS0107` nhưng vẫn lex**, **NFC** (`'ố'` NFD thành một literal hợp lệ) |
| Parser | **vòng tròn**: parse → fmt → parse cho HIR bằng nhau; in lại CST **đúng văn bản gốc** (lossless); fuzz không panic; mỗi tệp lỗi trong `tests/parse-errors` cho **đúng một** lỗi đầu tại dòng ghi trong fixture |
| CST | **không mất thông tin**: giữ độ rộng thụt lề gốc, chú thích, dấu phẩy cuối, thứ tự trường; `fmt` idempotent |
| Chế độ lệnh | `bảng dòng lệnh → CmdCST` vàng cho **từng hàng bảng 9.4** + 10 ca mơ hồ (ST-06); "cùng HIR" chỉ kiểm sau khi có `cmd_lower` (vòng 2) |
| Kiểm kiểu | tệp `.vis` có chú thích kỳ vọng `# error: VS0412` hoặc `# ok`; mỗi mã lỗi có ca dương và ca âm; snapshot thông báo lỗi |
| Xét hết match | so với bộ liệt kê vét cạn trên enum nhỏ sinh ngẫu nhiên; **guard không tính**, **kiểu vô hạn cần `_`** |
| Optional | `T → T?` tự nâng; `T??` lỗi; `?` trên `T?`; `if let`; `.get/.last` khi rỗng; `xs[i]?` |
| Số | `%` dấu theo số bị chia; `%0` panic; `**` phải-kết-hợp; mũ âm panic; bit count ≥ 64 panic; NaN `==` false |
| Interpreter | mô hình tham chiếu, giữ đơn giản |
| Bytecode + VM | **vi sai**: bộ sinh chương trình đúng kiểu theo seed, chạy trên interpreter và VM, so đầu ra, mã thoát, lỗi; tự thu nhỏ ca lệch |
| Trình xác minh | đột biến bytecode hợp lệ phải bị từ chối hoặc vẫn an toàn |
| Heap RC/CoW | Miri; test rò rỉ (đối tượng sống về 0); test ngữ nghĩa giá trị |
| async/scope | lập lịch tất định theo seed trong `vis-host-mock`; không task mồ côi; huỷ lan đúng; **panic trong scope huỷ anh em rồi lan** |
| Host | **bộ hợp chuẩn**: cùng tệp test chạy trên mọi backend; khác biệt hợp lệ khai báo rõ; `wait -> {code, how}`; suspend/resume/cancel |
| Quyền | test âm: `..`, symlink ra ngoài, dẫn xuất rộng hơn đều phải bị chặn; consent theo **hash closure** mất hiệu lực khi phụ thuộc đổi |
| Thư viện chuẩn | test `.vis` + so với triển khai tham chiếu trên host (ví dụ json với serde_json) |
| ViShell | phiên kịch bản qua pty giả: gõ phím, chụp màn hình văn bản, so snapshot; gợi ý; job; Ctrl-C; panic giữ biến/cwd |
| LSP | kịch bản JSON-RPC, snapshot; cột scalar ↔ UTF-16 ở biên |
| Hiệu năng | benchmark cố định (vòng lặp số, chuỗi, map, pipeline 1 triệu dòng, khởi động lạnh/ấm); lịch sử, cảnh báo hồi quy > 10% |

### 18.3 Hồi quy

Mỗi lỗi sửa có `tests/regress/<mã>.vis`. Ca lệch từ vi sai được thu nhỏ trước khi ghi.

### 18.4 Cổng

| Lệnh | Nội dung |
|---|---|
| `vis-dev check` | L0 crate bị chạm |
| `vis-dev gate --fast` | fmt, clippy, build wasm32 cho crate lõi, header SPDX **theo loại tệp**, **doc-examples parse**, luật 1.500 dòng + `unsafe` (có case âm), L0, L1 |
| `vis-dev gate` | + L2 |
| `vis-dev nightly` | L4 + benchmark + báo cáo |

Không retry. Không `#[ignore]` để qua cổng.

- **Cổng "doc-examples parse"** (QD §7.4): trích **mọi** khối mã nhãn `vis`/`vish-session` trong `docs/`, chạy `vis check`; **danh sách ngoại lệ phải khai báo tường minh** trong `docs/viscript/doc-examples-allow.txt` — dự kiến: khối sơ đồ ASCII, Rust, EBNF và các khối cố ý sai. Không có ngoại lệ ngầm.
- **Tiêu chí lossless của ST-07**: (a) `fmt` in lại **đúng văn bản gốc** trên 100 % fixture đã đúng chuẩn (lossless CST→text); (b) `fmt` **idempotent** — `fmt(fmt(x)) == fmt(x)`; (c) tệp có tab: `fmt` sửa thành 4 dấu cách; (d) tệp có lỗi cú pháp: `fmt` **không sửa**, báo lỗi, mã thoát 2.

---

## 19. Lộ trình

| Mốc | Nội dung | Tiêu chí xong | Phụ thuộc |
|---|---|---|---|
| VS-SPEC | **Áp errata S1..S11/L1..L8 + APPROVED** cho `SPEC-lex.md` + **Phụ lục A** + bảng ưu tiên (Agent 1 soạn) + sổ mã lỗi (`ST-02b`); 60 golden token chuyển ST-04, 30 golden biểu thức chuyển ST-05 | `APPROVED <ngày> Agent 0` trong `viscript/agent/QUEUE.md`; 60 golden token + 30 golden biểu thức; mọi khối mã trong tài liệu parse được (ngoại lệ khai báo) | |
| VS0 | syntax 2 chế độ + diag + types (kiểu cơ bản, bản ghi, enum, `T?`, Result, list/map, match xét hết, hàm, closure) + interpreter + bộ test ngữ nghĩa | ≥ 300 test `.vis`; fuzz parser 1 giờ không panic; vòng tròn fmt; **100 % fixture tài liệu parse** | VS-SPEC |
| VS1 | generic, trait, module, bytecode + VM + xác minh + cache, vis-std lõi, vis-host-posix, `vis run/check/fmt/test`, `cmd_lower` | vi sai interp/VM 100.000 chương trình 0 lệch; khởi động ấm < 20 ms [ƯL] | VS0 |
| VSH1 | ViShell trên Linux/macOS: chế độ lệnh đầy đủ, stream, chương trình ngoài, gợi ý theo kiểu, lịch sử, job (chưa chương trình toàn màn hình) | dùng hằng ngày được; test pty xanh | VS1 |
| VS2 | async có cấu trúc, kênh, net/http, quyền + Landlock, vis-host-windows, vis-host-wasm, LSP, gói | bộ hợp chuẩn Host xanh 4 nền tảng | VS1 |
| VSH2 | ViShell trên ViFastOs + vis-host-vifast (workspace ngoài) + binding IDL | thay shell tạm của ViFastOs | VS2, ViFastOs U1 |
| VS3 | plugin Rust, thread song song, binding toolkit, ViTerm hiển thị có cấu trúc | ứng dụng desktop nhỏ viết bằng ViScript | VS2, ViFastOs G3 |

VS-SPEC tới VS2 không phụ thuộc ViFastOs, chạy song song với kernel.

**Cổng chặn:** `ST-02a` ở trạng thái `REVIEW` **chặn `ST-04..ST-09`** (QD §7.5); lexer/parser/fmt không bắt đầu trước khi `ST-02a` được duyệt. `ST-02b`, `ST-03`, `ST-10` không bị chặn.

---

## 20. Quy trình Agent 2 và giao việc vòng 1

### 20.1 Quy trình

Dùng nguyên quy trình mục 11 tài liệu ViFastOs (QUEUE, STATE, JOURNAL, BLOCKED, DECISIONS, báo cáo MD + JSON, vòng 8 bước, luật cấm, ngân sách, chạy qua đêm), với các điểm riêng:

- Thư mục điều khiển `viscript/agent/`; `vifast-agentctl --project viscript`. Hai dòng có QUEUE riêng, không giẫm nhau.
- Lệnh cổng là **`vis-dev ...`** thay cho `v2 ...` (`vis-dev check`, `vis-dev gate --fast`, `vis-dev nightly`).
- Thay đổi ngữ nghĩa ngôn ngữ (cú pháp, quy tắc kiểu) **luôn** qua Agent 0; Agent 2 cài đặt theo tài liệu này, `SPEC-lex.md` và Phụ lục A. Chỗ chưa rõ: ghi BLOCKED kèm đề xuất + ví dụ, làm task khác.
- Lỗi phát hiện từ fuzz/vi sai: thu nhỏ, ghi `tests/regress/`, test đỏ trước rồi mới sửa.
- Duyệt **chỉ** bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `viscript/agent/QUEUE.md` + một mục VD mới; không duyệt bằng chat (QD §4, D2-22).

### 20.2 Giao việc vòng 1

Nhãn task đổi **VT → ST** (QD §2.1); cột Trạng thái dùng `READY | BLOCKED | REVIEW | DONE`.

| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| ST-01 | Workspace `viscript/` + `viscript/agent/` (không ghi đè) + SPDX **theo loại tệp** + cổng 1.500 dòng/`unsafe` có case âm + **`vis-host-vifast` không nằm trong `viscript/`** | — | 3 h | READY | cổng chạy; 3 case âm; `cargo build` workspace rỗng xanh |
| ST-02a | **Áp errata mục 3 + APPROVED** (Agent 1 soạn SPEC-lex/Phụ lục A/bảng ưu tiên) | — | 3 h (2 phiên) | **REVIEW** | errata áp xong; `APPROVED <ngày> Agent 0`; sau APPROVED là **DONE** |
| ST-02b | Sổ mã lỗi `VS` + 4 chữ số + quy tắc cấp | ST-01 | 2 h | READY | bảng mã; `vis explain` được |
| ST-03 | `vis-diag`: cột theo **Unicode scalar**, render, bảng vi/en/ja | ST-01 | 3 h | BLOCKED | snapshot 20 ca, có tiếng Việt/emoji; cột LSP đổi UTF-16 ở biên |
| ST-04 | Lexer theo `SPEC-lex` (mở ngay sau ST-02a DONE) | ST-02a | 3 h | BLOCKED | **60 golden token đỏ trước** + fuzz 10 phút + NFC/NFD; tab = `VS0107` vẫn lex |
| ST-05 | Parser + **CST lossless** | ST-04 | 3 h | BLOCKED | **30 golden biểu thức đỏ trước**; 100 % fixture tài liệu parse; mỗi tệp lỗi đúng 1 lỗi đầu |
| ST-06 | `CmdCST` + golden từng hàng bảng 9.4 + 10 ca mơ hồ | ST-05 | 3 h | BLOCKED | golden `CmdCST`; `cmd_lower` để vòng 2 |
| ST-07 | `vis-fmt`: lossless, idempotent, sửa tab | ST-05 | 3 h | BLOCKED | vòng tròn parse→fmt→parse; 4 tiêu chí ở 18.4 |
| ST-08 | Runner chỉ `--check` cú pháp (chưa interpreter) | ST-05 | 2 h | BLOCKED | TAP; chạy fixture |
| ST-09 | Fixture theo QD §7.4 + **cổng doc-examples parse** (1 fixture/khối mã + 1/quyết định; cấm `...`) | ST-06, ST-07, ST-08 | 2 h | BLOCKED | xanh; cổng doc-examples bật |
| ST-10 | Báo cáo `R1-viscript` (MD + JSON, có `project`) | các task | 1 h | READY | theo §11.7 tài liệu ViFastOs |

`ST-02a` **chặn `ST-04..ST-09`** cho tới khi có `APPROVED`; sau đó **DONE** và ST-04 mở ngay. Golden token là test đỏ đầu tiên của ST-04; golden biểu thức là test đỏ đầu tiên của ST-05; cổng doc-examples bật ở ST-09. `ST-02b/ST-03/ST-10` chạy song song. `cmd_lower` ở vòng 2 (sau `vis-types`).

---

## 21. Các lựa chọn đã chốt

| Mã | Câu hỏi | Chốt | Lý do |
|---|---|---|---|
| Q1 | Từ khoá tiếng Anh hay tiếng Việt? | **Tiếng Anh**; định danh Unicode cho phép tiếng Việt; thông báo lỗi có tiếng Việt; **thêm `const`, bỏ `as`, `test` là từ khoá theo ngữ cảnh** | Giống JS/TS, Python, Rust: từ khoá tiếng Anh, tên biến Unicode. Từ khoá có dấu gõ chậm, phụ thuộc bộ gõ |
| Q2 | `int / int` cho `int` hay `float`? | `int`, có lint khi chia hai literal (hạ mức) và lint khi kết quả dùng với `float`/`percent`; `%`/`**` theo bảng 3.6 | Nhất quán với kiểu tĩnh, không chuyển đổi ngầm |
| Q3 | Thụt lề | **Đầu vào: thụt lề nhất quán bất kỳ, đơn vị suy từ cấp đầu; đầu ra `vis fmt` luôn 4 dấu cách** [TK]. **Tab**: `VS0107` là cảnh báo-lỗi của lexer, coi tab = 4 cột, **vẫn lex tiếp**; `vis check` coi là lỗi; `vis fmt` sửa | Dán mã không chết ở dòng đầu; vẫn một kiểu ở đầu ra; vá mâu thuẫn "tab là lỗi từ vựng nhưng fmt tự sửa" |
| Q4 | Tên | ViScript (`.vis`, lệnh `vis`), ViShell (`.vish`, lệnh `vish`) | Ngắn, nhất quán với hệ ViFast |
| Q5 | Chỉ số ngoài phạm vi | **`xs[i]` panic** với thông điệp nêu chỉ số + độ dài; **`xs[i]?` trả `T?`**; `xs.get(i)` trả `T?`; `xs[-1]` panic có thông điệp; **cấm `s[i]`** trên `str` (dùng `.chars()/.graphemes()/.bytes()/.byte_at(i)?`) | Lỗi lập trình hiện rõ; vẫn có đường an toàn; `str` không có chỉ số theo byte/scalar |
| Q6 | Giấy phép | **MIT OR Apache-2.0** | Giống Rust; dễ dùng lại, có điều khoản **bằng sáng chế** (patent grant) của Apache |
| Q7 | `decimal` trong lõi | Không, là thư viện | Giữ lõi nhỏ |
| Q8 | Toán tử bit | Hàm `bit_and`/`bit_or`/`bit_xor`/`bit_not`/`shl`/`shr`/`ushr`, không ký hiệu. **`\|` không bao giờ là toán tử nhị phân trong biểu thức; pattern giữ `\|`** (or-pattern) và chế độ lệnh giữ `\|` cho pipe | Giải phóng `\|` khỏi bit-ops mà vẫn giữ or-pattern quen thuộc; sửa câu cũ "giải phóng `\|` cho pipe ở mọi chế độ" |

### 21.1 Chốt bổ sung sau review (trỏ QD §7)

| # | Chốt | QD |
|---|---|---|
| 1 | **NFC toàn tệp trước khi lex**; cột chẩn đoán theo scalar, LSP đổi UTF-16 ở biên | §7.1 |
| 2 | Float phải có chữ số sau `.`; `1..2` là range; `_` chỉ giữa hai chữ số; literal tràn i64 = lỗi biên dịch | §7.1 |
| 3 | Đơn vị `size` = `b kb mb gb tb kib mib gib tib`; `duration` = `ns us ms s m h d w`, ghép giảm dần là một token | §7.1 |
| 4 | **Bỏ `50%` → `50pct`**; `%` chỉ modulo; job spec `job 1` | §7.1 |
| 5 | Năm dạng chuỗi + bảng escape + nội suy `{expr}`/`{expr:spec}`/`{{`/`}}` | §7.1 |
| 6 | `#`: script luôn là chú thích; lệnh chỉ khi đứng đầu từ; `https://x/y#frag` giữ nguyên | §7.1 |
| 7 | `set{` dính liền là set literal; có khoảng trắng là định danh; `?` maximal munch | §7.1 |
| 8 | `%` dấu theo số bị chia, `%0` panic, có `rem_euclid`/`mod_floor`; `**` phải-kết-hợp, cao hơn `*`, mũ âm panic | §7.3 |
| 9 | Bit: thêm `bit_not`, `ushr`; `shr` số học; đếm ≥ 64 panic; có `checked_shl/shr` | §7.3 |
| 10 | Bảng ưu tiên **12 mức** (postfix → `**` → một ngôi → `* / %` → `+ -` → `..` → `??` → so sánh → `not` → `and` → `or` → `\|>`) | §7.3 |
| 11 | Optional: tự nâng `T → T?` ở gán/trả/đối số; `T??` lỗi; `?` trên `T?`; pattern `none/some/ok/err` | §7.3 |
| 12 | Xét hết: guard **không** tính coverage; kiểu vô hạn **bắt buộc** `_` | §7.3 |
| 13 | `trait Error: Show { message, source, span }`; `AnyError` xoá kiểu; `?` tự chuyển; `main -> Result[(), AnyError]`; plugin dùng `vis::Error` | §7.3 |
| 14 | Mã thoát 0/1/2/101/126/127/130; panic payload `AnyError`; ViShell bọc **mỗi lệnh** trong task arena riêng, panic giải phóng arena, giữ biến/cwd/job/lịch sử; chưa có `defer` | §7.3 |
| 15 | Consent nhớ theo **hash closure** (nguồn + phụ thuộc + phiên bản compiler), dùng chung hash cache | §7.3 |

---

## 22. Quyết định

| Mã | Nội dung |
|---|---|
| VD-01 | ViScript có kiểu tĩnh bắt buộc, không `any`, không null |
| VD-02 | Một ngôn ngữ, hai chế độ cú pháp, cùng **HIR**/kiểm kiểu/VM; ViShell là chương trình riêng |
| VD-03 | Ngữ nghĩa giá trị + RC + copy-on-write; không GC truy vết; chu trình chỉ qua `ref`, phá bằng `weak` |
| VD-04 | Lõi (syntax, types, bytecode, vm) không phụ thuộc hệ điều hành, build được wasm32 |
| VD-05 | Mọi truy cập hệ thống qua capability; không hàm toàn cục chạm hệ thống |
| VD-06 | Interpreter cây là mô hình tham chiếu, giữ mãi để kiểm vi sai VM |
| VD-07 | Không JIT; không `eval` chuỗi trong chế độ quyền hạn chế |
| VD-08 | Thay đổi SPEC chỉ qua Agent 0; mỗi tính năng mới phải có ví dụ + test trước |
| VD-09 | Từ khoá tiếng Anh, định danh Unicode (NFC), thông báo lỗi vi/en/ja |
| VD-10 | Giấy phép MIT OR Apache-2.0; phụ thuộc phải tương thích |
| VD-11 | Chuẩn hoá: **toàn bộ tệp nguồn được chuẩn hoá NFC trước khi lex** — 'ố' gõ kiểu nào cũng là một scalar. |
| VD-12 | Thụt lề: đầu vào nhận thụt lề nhất quán bất kỳ, đơn vị suy từ cấp đầu; đầu ra `vis fmt` luôn **4**. Tab: lexer báo cảnh báo-lỗi **VS0107**, coi tab = 4, **vẫn lex tiếp**; `vis check` coi là lỗi, `vis fmt` sửa. |
| VD-13 | Phần trăm: **bỏ literal `50%`**, dùng `50pct`; `%` chỉ là modulo; job spec dùng `job 1`. |
| VD-14 | Chú thích: script — `#` ngoài chuỗi luôn là chú thích; lệnh — `#` chỉ là chú thích khi **đứng đầu từ** (sau khoảng trắng hoặc đầu dòng), nên `https://x/y#frag` giữ nguyên. |
| VD-15 | `|` **không bao giờ là toán tử nhị phân trong biểu thức**. Pattern: or-pattern dùng `|`; lệnh: pipe ở **độ sâu ngoặc 0**; `|>` là pipe script. (Sửa Q8.) |
| VD-16 | Bảng ưu tiên chốt (cao → thấp): postfix (`. ?. () [] ? with`) → `**` → `-` một ngôi → `* / %` → `+ -` → `.. ..=` → `??` → so sánh (không kết hợp) → `not` → `and` → `or` → `|>`. `**` kết hợp **phải**, cao hơn `*`; mũ âm với int panic; có `checked_pow`. |
| VD-17 | Float: NaN — `==` trả false; float **không** có `Eq/Ord/Hash`; kiểu chứa float không derive được `Eq/Hash`; dùng `total_cmp`. `percent` so sánh được với float theo tỉ lệ (`50pct == 0.5`); `ps.cpu` có kiểu `percent`. |
| VD-18 | Chuỗi: **cấm `s[i]`**; dùng `.chars() .graphemes() .bytes() .byte_at(i)?`; có `byte_len()`, `char_count()`, `grapheme_count()`; so sánh theo dãy scalar. Index: thêm `xs[i]?` trả `T?`; `xs[-1]` panic có thông điệp. |
| VD-19 | Optional: tự nâng `T → T?` ở gán, trả, đối số; `T??` là lỗi kiểu; `?` dùng được trên `T?` trong hàm trả `U?`; pattern `none`, `some(x)`, `ok(v)`, `err(e)`. |
| VD-20 | Lỗi: `trait Error: Show { message, source, span }`; `AnyError` là kiểu xoá; mọi `E: Error` tự chuyển sang `AnyError` qua `?`; `main -> Result[(), AnyError]`; tên phía plugin Rust là `vis::Error`. Mã thoát: 0 ok, 1 lỗi từ main, 2 lỗi dùng/kiểu, 101 panic, 126 không chạy được, 127 không tìm thấy, 130 bị huỷ. |
| VD-21 | Chế độ lệnh (QD §7.2): metachar là khoảng trắng, `( ) [ ] { } ; & $ ' " |`, `#` ở đầu từ; `"..."` có nội suy, `'...'` thô; `$x` biến phiên, `$env.NAME` môi trường; `;` tách pipeline; `&` nền chỉ khi là token cuối có khoảng trắng phía trước; `--` kết thúc cờ; `-5` là số nếu tham số kiểu số, ngược lại là cờ; `.vish`/REPL: `shell_item := use | let | fn | cmd | pipeline`. |
| VD-22 | Parse chế độ lệnh **hai pha**: `vis-syntax` sinh `CmdCST` (từ trần chưa định kiểu) → `cmd_lower` trong `vis-types` dùng bảng chữ ký → **HIR**. Tiêu chí "cùng AST" đọc thành **"cùng HIR"**; vòng 1 chỉ làm `CmdCST` + golden, `cmd_lower` là task vòng 2. |
| VD-23 | `docs/viscript/SPEC-lex.md` + **Phụ lục A** là nguồn từ vựng/cú pháp duy nhất; Agent 0 duyệt qua errata; ST-02a DONE sau APPROVED. |
| VD-24 | **Shell sở hữu tty**; tiến trình con nhận pipe, không nhận `cap[Term]`; chương trình toàn màn hình để sau với tty server. |
| VD-25 | `from json` không tham số trả `Json`; `where`/`get` trên `Json` là lỗi kiểu gợi ý `from json[T]`; SIGPIPE/`take` không phải lỗi; `| complete` trả `{stdout,stderr,code,how}`. |
| VD-26 | Consent và cache dùng chung **hash closure** (nguồn + phụ thuộc + phiên bản compiler). |

---

## 23. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Phạm vi ngôn ngữ phình | VD-08; lộ trình bậc; tính năng chỉ vào khi có ca dùng thật |
| Kiểm kiểu generic + trait phức tạp, thông báo lỗi khó hiểu | Suy kiểu cục bộ; generic kiểm một lần với ràng buộc; snapshot thông báo lỗi là test bắt buộc |
| Hai chế độ cú pháp mập mờ | Bảng xung đột 9.4 là một phần SPEC; mọi dòng lệnh mẫu có test `CmdCST`; hai pha tách bạch (VD-11) |
| VM chậm | Lệnh có kiểu, không thẻ kiểu; benchmark từ VS1; chuyên biệt hoá generic khi có số đo |
| Ngữ nghĩa giá trị gây sao chép tốn | CoW + chuyển quyền khi RC = 1; đo trong benchmark |
| Quyền trên OS khác không chặt bằng ViFastOs | Ghi rõ mức bảo đảm (11.4); tăng cường Landlock |
| Hai dòng ViScript và ViFastOs tranh tài nguyên Agent 2 | QUEUE riêng; ViScript làm nhánh độc lập khi dòng OS BLOCKED |
| Tài liệu lệch mã (như v1) | Cổng **doc-examples parse** (18.4) + fixture ST-09 |

---

## Phụ lục A. EBNF tự chứa (chế độ script + chế độ lệnh)

Đây là **văn phạm chuẩn tắc**, không phải "giả ngữ pháp": mọi luật dưới đây là điều kiện cần và đủ để parser của `ST-05`/`ST-06` chấp nhận đúng các ví dụ trong tài liệu. Token viết HOA (`IDENT`, `INT`, `STRING`, `WORD`, `NAME`, `NEWLINE`, `INDENT`, `DEDENT`...) do `vis-syntax` sinh theo `SPEC-lex.md`; mọi ký tự còn lại là terminal của ngôn ngữ. `(* ... *)` là chú thích của văn phạm.

```
(* ===== A.1 Khung tệp và item ===== *)

file            := NEWLINE* item* EOF
item            := use_decl | fn_decl | cmd_decl | type_item | enum_decl
                 | trait_decl | impl_decl | const_decl | test_decl | extern_decl

use_decl        := 'use' dotted_path NEWLINE
dotted_path     := IDENT ('.' IDENT)*

fn_decl         := ['pub'] ['async'] 'fn' IDENT generics? '(' params? ')' ['->' type] ':' suite
cmd_decl        := ['pub'] 'cmd' IDENT '(' params? ')' ['->' type] ':' suite
extern_decl     := 'extern' 'cmd' IDENT '(' params? ')'

params          := param (',' param)*
param           := '...' IDENT ':' type
                 | ['--'] IDENT ':' type ['@' path_lit] ['(' grant_list ')'] ['=' expr]
                 | ['var'] 'self'
grant_list      := IDENT (',' IDENT)*

generics        := '[' generic (',' generic)* ']'
generic         := IDENT [':' bound]
bound           := dotted_path | 'has' record_type

type_item       := type_decl | alias_decl | newtype_decl
type_decl       := ['pub'] 'type' IDENT generics? derives? ':' NEWLINE INDENT field+ DEDENT
field           := IDENT ':' type ['=' expr] NEWLINE
alias_decl      := 'alias' IDENT '=' type NEWLINE
newtype_decl    := 'newtype' IDENT '=' type NEWLINE

enum_decl       := ['pub'] 'enum' IDENT generics? derives? ':' NEWLINE INDENT variant+ DEDENT
variant         := IDENT ['(' variant_fields ')'] NEWLINE
variant_fields  := IDENT ':' type (',' IDENT ':' type)*
derives         := 'derives' dotted_path (',' dotted_path)*

trait_decl      := ['pub'] 'trait' IDENT generics? [':' supertraits] ':' NEWLINE INDENT trait_item+ DEDENT
supertraits     := dotted_path (',' dotted_path)*
trait_item      := ['pub'] 'fn' IDENT '(' params? ')' ['->' type] (':' suite | NEWLINE)

impl_decl       := 'impl' [dotted_path 'for'] type generics? ':' NEWLINE INDENT impl_item+ DEDENT
impl_item       := fn_decl

const_decl      := ['pub'] 'const' IDENT ':' type '=' expr NEWLINE
test_decl       := 'test' 'fn' IDENT '(' ')' ':' suite

(* ===== A.2 Khối và câu lệnh ===== *)

suite           := simple_stmt NEWLINE                  (* khối một dòng: `if x > 0: return x` *)
                 | NEWLINE INDENT stmt+ DEDENT
simple_stmt     := let_head | var_head | assign_head | return_stmt | break_stmt
                 | continue_stmt | expr

stmt            := let_stmt | var_stmt | assign_stmt | return_stmt | break_stmt
                 | continue_stmt | for_stmt | while_stmt | loop_stmt | scope_stmt
                 | if_stmt | match_stmt | expr_stmt

let_stmt        := let_head NEWLINE
let_head        := 'let' pattern [':' type] '=' expr [else_clause]
else_clause     := 'else' [IDENT] ':' suite

var_stmt        := var_head NEWLINE
var_head        := 'var' IDENT [':' type] '=' expr
assign_stmt     := assign_head NEWLINE
assign_head     := lvalue assign_op expr
assign_op       := '=' | '+=' | '-=' | '*=' | '/=' | '%='
lvalue          := IDENT postfix_op*

return_stmt     := 'return' [expr] NEWLINE
break_stmt      := 'break' NEWLINE
continue_stmt   := 'continue' NEWLINE

for_stmt        := 'for' pattern 'in' expr ':' suite
while_stmt      := 'while' expr ':' suite
loop_stmt       := 'loop' ':' suite
scope_stmt      := 'scope' [IDENT] ':' suite

if_stmt         := 'if' if_cond ':' suite elif_arm* else_arm?
if_cond         := 'let' pattern '=' expr | expr        (* `if let` = 6.3 *)
elif_arm        := 'elif' if_cond ':' suite
else_arm        := 'else' ':' suite

match_stmt      := 'match' expr ':' NEWLINE INDENT match_arm+ DEDENT
match_arm       := pattern ['if' expr] ':' suite
expr_stmt       := expr NEWLINE

(* ===== A.3 Biểu thức — đúng bảng ưu tiên A.5 ===== *)

expr            := lambda | pipe_expr
lambda          := 'fn' '(' params? ')' ['->' type] ':' suite
                 | lambda_params '=>' expr
lambda_params   := IDENT | '(' IDENT (',' IDENT)* ')'

pipe_expr       := or_expr ('|>' or_expr)*
or_expr         := and_expr ('or' and_expr)*
and_expr        := not_expr ('and' not_expr)*
not_expr        := 'not' not_expr | cmp_expr
cmp_expr        := coalesce [cmp_op coalesce]
cmp_op          := '==' | '!=' | '<' | '<=' | '>' | '>='
coalesce        := range_expr ('??' range_expr)*
range_expr      := add_expr [('..' | '..=') add_expr]
add_expr        := mul_expr (('+' | '-') mul_expr)*
mul_expr        := unary_expr (('*' | '/' | '%') unary_expr)*
unary_expr      := '-' unary_expr | power_expr
power_expr      := postfix ['**' unary_expr]
postfix         := primary postfix_op*
postfix_op      := '.' IDENT | '?.' IDENT | '(' args? ')' | '[' expr ']'
                 | '[' types ']' | '?' | 'with' field_updates

primary         := literal | 'self' | IDENT | 'ref' '(' expr ')' | '(' ')'
                 | '(' expr ')' | '(' expr ',' expr (',' expr)* ','? ')'
                 | list_lit | map_lit | set_lit | record_lit
                 | 'cmd' '{' pipeline '}' | if_expr | match_expr
                 | 'await' postfix

args            := positional_args? named_args?
positional_args := expr (',' expr)*
named_args      := (',' '--' IDENT [':' expr])+

field_updates   := IDENT ':' expr (',' IDENT ':' expr)*

list_lit        := '[' [expr (',' expr)* ','?] ']'
map_lit         := '{' [map_entry (',' map_entry)* ','?] '}'
map_entry       := expr ':' expr
set_lit         := 'set' '{' [expr (',' expr)* ','?] '}'
record_lit      := '{' record_field (',' record_field)* ','? '}'
record_field    := IDENT ':' expr | IDENT

if_expr         := 'if' if_cond ':' suite elif_arm* else_arm      (* else bắt buộc *)
match_expr      := 'match' expr ':' NEWLINE INDENT match_arm+ DEDENT

(* ===== A.4 Pattern ===== *)

pattern         := or_pattern
or_pattern      := range_pattern ('|' range_pattern)*
range_pattern   := atom_pattern [ '..' [atom_pattern] | '..=' atom_pattern ]
atom_pattern    := '_' | literal | '-' INT | 'none' | dotted_path
                 | '(' pattern (',' pattern)* ')'
                 | '[' [list_pat] ']'
                 | '{' [record_pat (',' record_pat)*] '}'
                 | dotted_path '(' [pattern (',' pattern)*] ')'
list_pat        := pattern (',' pattern)* [',' '..' [IDENT]]
record_pat      := IDENT ':' pattern | IDENT | '..'

(* ===== A.5 Kiểu ===== *)

type            := type_primary ['?']                 (* T?? là lỗi kiểu, 4.4 *)
type_primary    := dotted_path ['[' types ']']
                 | 'ref' '[' type ']'
                 | 'weak' '[' type ']'
                 | 'cap' '[' cap_kind (',' grant_list)? ']'
                 | 'fn' '(' [types] ')' '->' type
                 | '(' ')'                            (* unit *)
                 | '(' type ',' type (',' type)* ')'  (* tuple *)
                 | '{' record_type '}'
types           := type (',' type)*
cap_kind        := dotted_path
record_type     := IDENT ':' type (',' IDENT ':' type)*

(* ===== A.6 Token ===== *)

literal         := INT | FLOAT | CHAR | BOOL | 'none'
                 | STRING | RAW_STRING | TRIPLE_STRING | RAW_TRIPLE_STRING
                 | BYTES | PATH_STRING | SIZE | DURATION | PERCENT   (* terminal một token, theo SPEC-lex *)
path_lit        := PATH_STRING | STRING

(* SIZE     = INT + đơn vị, không cách: 5kb 5kib 2mb 1gib *)
(* DURATION = INT + đơn vị, ghép giảm dần: 250ms 2h30m 1d *)
(* PERCENT  = (INT | FLOAT) + 'pct': 50pct 12.5pct *)
(* PATH_STRING = p"..." *)

NEWLINE         := xuống dòng ngoài ngoặc và ngoài chuỗi; bị bỏ khi dòng này
                   **bắt đầu** bằng `|>` ở độ sâu 0 (nối với dòng trước)
INDENT          := một mức thụt lề mới; đơn vị suy từ cấp thụt lề đầu tiên của tệp
DEDENT          := hết mức thụt lề; ngầm ở EOF và ở cuối mỗi input REPL

(* ===== A.7 Chế độ lệnh ===== *)

pipeline        := stage ('|' stage)*
stage           := word_cmd arg* | '(' expr ')'
word_cmd        := NAME | '^' NAME | '$' IDENT ('.' IDENT)*
arg             := word | flag | '(' expr ')' | STRING | closure_arg
flag            := '--' IDENT ['=' expr] | '-' IDENT ['=' expr] | '--'
word            := WORD
closure_arg     := IDENT '=>' closure_body | field_shorthand
closure_body    := biểu thức kéo tới `|` / `;` / `&` / NEWLINE ở độ sâu 0
(* Ràng buộc: lambda `fn(...) -> T:` **nhiều dòng chỉ ở cấp câu lệnh**
   (vế phải let/var/gán/return). Trong ngoặc INDENT bị tắt nên chỉ dùng `=>`
   hoặc `fn` một dòng. *)
field_shorthand := biểu thức chế độ biểu thức với `it` ngầm

shell_file      := shell_item*
shell_let       := 'let' IDENT [':' type] '=' pipeline NEWLINE
shell_item      := use_decl | fn_decl | cmd_decl | shell_let | pipeline NEWLINE
```

### A.5 Bảng ưu tiên (bản chuẩn tắc, khớp 4.5)

| Mức | Nhóm | Toán tử | Kết hợp |
|---|---|---|---|
| 1 | postfix | `.` `?.` `()` `[]` `?` `with` | trái |
| 2 | luỹ thừa | `**` | phải |
| 3 | một ngôi | `-` | phải |
| 4 | nhân | `* / %` | trái |
| 5 | cộng | `+ -` | trái |
| 6 | khoảng | `.. ..=` | không kết hợp |
| 7 | coalesce | `??` | phải |
| 8 | so sánh | `== != < <= > >=` | không kết hợp |
| 9 | phủ định | `not` | phải |
| 10 | hội | `and` | trái, ngắn mạch |
| 11 | tuyển | `or` | trái, ngắn mạch |
| 12 | nối | `\|>` | trái |

### Ghi chú ràng buộc văn phạm

1. **Khối một dòng** (`suite := simple_stmt NEWLINE`) không sinh `INDENT`/`DEDENT`; dùng cho `if x > 0: return x`, `if loud: s.upper() else: s`.
2. **`if`/`match` vừa là câu lệnh vừa là biểu thức**: bản biểu thức (`if_expr`) **bắt buộc** có `else`; bản câu lệnh thì không.
3. **`elif`** có mặt ở cả hai dạng (`elif_arm`), và có thể lặp.
4. **Phân biệt bằng lookahead 2 token (LL(2)):**
   - `{ IDENT ':'` → `record_lit`; ngược lại `{` → `map_lit`. Map có khoá là định danh trần phải viết khoá không-trần (`{"a": 1}`, `{(k): v}`).
   - `IDENT '[' ... ']' '('` **và** `IDENT` là hàm generic đã biết → danh sách kiểu (gọi generic); ngược lại → index (5.3).
   - `'(' IDENT (',' IDENT)* ')' '=>'` → `lambda`; ngược lại → nhóm/tuple.
   - `set` **dính liền** `{` → `set_lit`; có khoảng trắng → định danh (2.4).
   - `cmd` `{` → khối lệnh; `cmd` khác → định danh.
5. **`|` không có luật sinh trong biểu thức** (chỉ có trong `or_pattern` và `pipeline`); đó là cách văn phạm cưỡng chế Q8/VD-16.
6. **Chế độ lệnh** dùng `CmdCST` chưa định kiểu (A.7); `closure_body` và `field_shorthand` được cắt theo **độ sâu ngoặc 0** và được phân giải ở `cmd_lower`, không ở parser. `word` không chứa metachar (9.3).
7. **`NEWLINE`, `INDENT`, `DEDENT`** do lexer sinh theo 2.2; "giả ngữ pháp" của bản trước (12 luật, thiếu `pattern`, `if_expr`, khối một dòng) **bị bỏ** — thay bằng văn phạm này.

---

## Phụ lục B. Chương trình mẫu

```vis
#!/usr/bin/env vis
use std.fmt.json
use std.time

type Entry derives Json, Show:
    path: str
    size: size
    modified: datetime

## Tìm 20 tệp lớn nhất đã sửa trong 7 ngày, ghi báo cáo JSON
fn main(src: cap[Dir] @ p".", out: cap[Dir] @ p"./reports" (write)) -> Result[(), AnyError]:
    let since = time.now() - 7d
    let files = src.walk()
        |> filter(e => e.is_file() and e.modified >= since)
        |> sort_by(e => e.size, --desc)
        |> take(20)
        |> map(e => Entry(path: str(e.path), size: e.size, modified: e.modified))
        |> collect()

    out.write_text("big-files.json", json.to_string(files, --pretty))?
    print("Đã ghi {files.len()} mục")
    Ok(())

test fn sort_desc_works():
    let xs = [3, 1, 2] |> sort_by(x => x, --desc) |> collect()
    assert xs == [3, 2, 1]
```

Ghi chú cho ví dụ trên (đều là sửa sau review):

- Dòng bắt đầu bằng `|>` **nối với dòng trước** (2.2, A.6) — không cần ngoặc.
- `str(e.path)` hợp lệ vì `str(x) = x.show()` (3.5).
- `time.now()` thay `now` trần; `is_file()` nhất quán ở cả hai chế độ (gọi phương thức, có `()`).
- `@ p"."` là literal `path` tường minh (3.5, 11.2).
- `Result[(), AnyError]` theo VD-20.

Cùng việc trong ViShell (mỗi dòng là **input hợp lệ**, không có tiền tố `>`):

```vish-session
walk . | where it => it.is_file() and it.modified >= time.now() - 7d | sort_by size --desc | take 20 | to json --pretty | save reports/big-files.json
ps | where cpu > 50pct | each p => p.kill()
ls ./photos | where size > 5mb | sort_by modified --desc | take 10
```

Định danh tiếng Việt:

```vis
fn tính_tổng(danh_sách: list[int]) -> int:
    var tổng = 0
    for số in danh_sách:
        tổng += số
    tổng
```
