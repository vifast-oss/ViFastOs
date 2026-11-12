# SPEC-lex — Đặc tả từ vựng ViScript

Trạng thái: **REVIEW** (ST-02a). Nguồn sự thật: `docs/QUYET-DINH-sau-review.md` §7.1–7.2 (QD).
Tài liệu liên quan: `ViScript-thiet-ke.md` §2, §4.5, §9, Phụ lục A.
Quy ước: "scalar" = Unicode scalar value; mọi offset tính bằng byte, cột tính bằng scalar (1-based).

## 0. Nguyên tắc

1. Tệp UTF-8; **chuẩn hoá NFC toàn tệp trước khi lex** (QD §7.1). Mọi literal/định danh đều ở dạng NFC.
2. Lexer là hàm thuần: `bytes -> [Token] | [Diagnostic]`; không phụ thuộc kiểu.
3. Maximal munch, trừ các luật ghi rõ.
4. Token luôn có `{kind, text, span:{start,end}, line, col}`; `col` theo scalar.
5. Tab: sinh `VS0107` (cảnh báo-lỗi), **coi tab = 4 cột**, vẫn lex tiếp; `vis check` coi là lỗi, `vis fmt` sửa.

## 1. Bảng token

| Kind | Regex / luật | Ví dụ | Ghi chú |
|---|---|---|---|
| `IDENT` | UAX #31 + `_`; chữ số không đứng đầu; NFC | `số_lượng`, `_tmp` | từ khoá tra sau |
| `KW` | danh sách §2.1 tài liệu | `let`, `fn`, `const` | `test` theo ngữ cảnh |
| `INT` | `[0-9](_?[0-9])*` hoặc `0x[0-9a-fA-F](_?..)*` hoặc `0b[01](_?..)*` | `42`, `1_000`, `0xff` | `_` chỉ **giữa** hai chữ số; tràn i64 = `VS0104`. `0b`/`0x` là tiền tố cơ số **chỉ khi** theo sau là chữ số hợp lệ; `0b` đơn lẻ là `SIZE(0,b)` |
| `FLOAT` | `INT '.' [0-9]+ ([eE][+-]?[0-9]+)?` hoặc `INT [eE][+-]?[0-9]+` | `3.14`, `1e-9` | `1.` sai (`VS0104`); `1..2` là `INT RANGE INT` |
| `CHAR` | `'` scalar `'` | `'a'`, `'ố'` | đúng 1 scalar sau NFC; nhiều scalar = `VS0105` |
| `STRING` | `"..."` có escape + nội suy | `"xin {name}"` | xem §3 |
| `RAW_STRING` | `r"..."` không escape, không nội suy | `r"C:\raw"` | |
| `MULTI_STRING` | `"""..."""` có escape + nội suy | `"""a\nb"""` | |
| `RAW_MULTI` | `r"""..."""` | | thô |
| `BYTES` | `b"..."` ASCII + `\xNN` | `b"\x00"` | ký tự > 0x7F = `VS0106` |
| `PATH` | `p"..."` | `p"./a.txt"` | có escape, **có nội suy** (như STRING); là literal `path` |
| `SIZE` | `INT` + `b\|kb\|mb\|gb\|tb\|kib\|mib\|gib\|tib` **không cách** | `5kb` | `kb`=1000, `kib`=1024 |
| `DURATION` | `INT` + `ns\|us\|ms\|s\|m\|h\|d\|w`, ghép **giảm dần** không cách | `250ms`, `2h30m` | `2h30m` = **một** token |
| `PERCENT` | `(INT | FLOAT)` + `pct` | `50pct`, `12.5pct` | **không còn** `50%` |
| `PUNCT` | `( ) [ ] { } , : . .. ..= ; -> => = == != < <= > >= + - * / % ** & \| \|> ? ?. ?? ! @ # ## ^ ~ \\` | | xem §5 |
| `NEWLINE` | `\n` (chuẩn hoá CRLF→LF) | | trong ngoặc bị nuốt theo §4 |
| `INDENT`/`DEDENT` | theo §4 | | |
| `EOF` | hết tệp | | sinh DEDENT còn lại |

## 2. Số

- `INT` là `i64`; literal tràn là **lỗi biên dịch** (`VS0104`), không phải panic runtime.
- `FLOAT` là `f64`; `NaN`/`inf` không có literal; dùng `float.nan`/`float.inf`.
- `_` chỉ giữa hai chữ số: `1_000` đúng; `_1`, `1_`, `1__0` sai.
- `..`/`..=` thắng `.` khi gặp `1..2`; `1.2..3.4` là `FLOAT RANGE FLOAT`.
- `-` là toán tử một ngôi, không thuộc literal; `-9223372036854775808` = `-(922...)` và **tràn** ⇒ lỗi; viết `i64.min`.

## 3. Chuỗi, escape, nội suy

| Dạng | Escape | Nội suy | Ghi chú |
|---|---|---|---|
| `"..."` | có | có | |
| `r"..."` | không | không | `\` là ký tự thường |
| `"""..."""` | có | có | nhiều dòng, giữ newline |
| `r"""..."""` | không | không | |
| `b"..."` | chỉ `\xNN` | không | ASCII |
| `p"..."` | có | có | path literal |

Escape hợp lệ: `\n \t \r \0 \\ \" \' \{ \} \u{X..X}` (X là 1–6 hex, ≤ 0x10FFFF, không surrogate).
Nội suy: `{expr}` hoặc `{expr:spec}`; `{{` → `{`, `}}` → `}`. `{` không đóng = `VS0108`.
Trong `r"..."`/`r"""..."""` không có escape/nội suy, nên `{` là ký tự thường.
Chuỗi chưa đóng = `VS0101`; escape sai = `VS0102`; `\x` ngoài `b""` = `VS0102`.

## 4. Thụt lề và NEWLINE/INDENT/DEDENT

Thuật toán (chuẩn, QD §7.1):
1. Đầu vào: thụt lề nhất quán bất kỳ; **đơn vị = mức thụt đầu tiên > 0** gặp trong tệp. Mọi mức sau phải là bội của đơn vị; sai = `VS0109`.
2. Dòng trống hoặc chỉ có chú thích: **không** sinh NEWLINE/INDENT/DEDENT.
3. Trong `() [] {}`: bỏ qua NEWLINE/INDENT/DEDENT (xuống dòng tự do); đóng ngoặc trả lại trạng thái ngoài.
4. Chuỗi nhiều dòng (`"""`, `r"""`) nuốt newline bên trong trước khi tính cột.
5. Dòng bắt đầu bằng `|>` được nối với dòng trước (không sinh NEWLINE); áp dụng cả trong ngoặc và ngoài.
6. `DEDENT` ngầm ở EOF và ở cuối mỗi input REPL; `INDENT`/`DEDENT` không khớp = `VS0109`.
7. Tab: `VS0107`, tab = 4 cột (chỉ để tính cột), **vẫn lex tiếp**.

## 5. Xử lý nhập nhằng (maximal munch + luật riêng)

| Cặp | Luật |
|---|---|
| `.` vs `..` vs `..=` | dài nhất thắng; `1..2` không phải float |
| `?` vs `?.` vs `??` | dài nhất; `x??y` = `x ?? y` |
| `\|` vs `\|>` | dài nhất; `\|>` là pipe biểu thức, `\|` or-pattern/pipe lệnh |
| `*` vs `**` | dài nhất; `**` luỹ thừa, `*` nhân/glob |
| `%` | **chỉ** modulo; `50pct` là percent; `%1` không phải jobspec (dùng `job 1`) |
| `set{` | `set` **dính liền** `{` → token riêng `SET_LBRACE`; `set {` = `IDENT(set)` + `PUNCT({)` |
| `#` | script: luôn chú thích; lệnh: chỉ khi đứng đầu từ; `https://x/y#frag` giữ nguyên |
| `-5` | số âm nếu ngữ cảnh kiểu số; ngược lại là cờ (do `cmd_lower`, không phải lexer) |
| `@` | lexer **luôn** trả `PUNCT(@)`; kiểm ngữ cảnh (`cap[Dir] @ p"..."`, `cap[Net] @ "host"`) thuộc parser/kiểm kiểu |
| `^` | chỉ ở đầu stage chế độ lệnh (ép gọi ngoài) |

## 6. Chế độ lệnh

Metachar: khoảng trắng, `( ) [ ] { } ; & $ ' " \|`; `#` khi đứng đầu từ; `^` ở đầu stage.
- `word` = chuỗi liên tục không chứa metachar; có thể chứa `\` escape **chỉ trong chuỗi**.
- `"..."` có nội suy; `'...'` thô; `$x` biến phiên; `$env.NAME` môi trường.
- `;` tách pipeline; `&` nền chỉ khi token cuối + có khoảng trắng trước; `--` kết thúc cờ.
- `[`/`]` là list literal/glob class: list literal phải nằm trong `( )` khi làm đối số; glob class giữ nguyên.
- `closure_arg` kéo tới `|` kế tiếp ở độ sâu 0.
- `>` `<` là so sánh, **không** redirect; ghi tệp bằng `| save`.
- Luật `.vish`/REPL: `shell_item := use | let | fn | cmd | pipeline`.

## 7. Mã lỗi từ vựng

| Mã | Nghĩa |
|---|---|
| VS0101 | chuỗi chưa đóng |
| VS0102 | escape không hợp lệ |
| VS0103 | ký tự lạ ngoài bảng token |
| VS0104 | literal số sai/tràn i64 |
| VS0105 | char literal không đúng 1 scalar |
| VS0106 | byte string có ký tự > 0x7F |
| VS0107 | tab (cảnh báo-lỗi; tab = 4) |
| VS0108 | nội suy `{` không đóng |
| VS0109 | thụt lề không khớp đơn vị/DEDENT sai |
| VS0110 | `^` sai ngữ cảnh (lexer chế độ lệnh biết đầu stage) |

Mọi mã có `vis explain VSxxxx`; bảng đầy đủ ở ST-02b.

## 8. Golden test (định dạng `tests/lex/*.tok`)

Mỗi dòng: `input => kind(text) ...` hoặc `input => !VSxxxx`. 60 ca tối thiểu:

```
42 => INT(42)
1_000_000 => INT(1000000)
0xff => INT(255)
0b101 => INT(5)
0b => SIZE(0,b)
1. => !VS0104
3.14 => FLOAT(3.14)
1e-9 => FLOAT(1e-9)
0..10 => INT(0) RANGE INT(10)
0..=10 => INT(0) RANGE_EQ INT(10)
1.2..3.4 => FLOAT(1.2) RANGE FLOAT(3.4)
50pct => PERCENT(50)
12.5pct => PERCENT(12.5)
50 % 3 => INT(50) PUNCT(%) INT(3)
50% => INT(50) PUNCT(%)   (* parser báo lỗi kèm gợi ý "dùng 50pct" *)
5kb => SIZE(5,kb)
5kib => SIZE(5,kib)
5mib => SIZE(5,mib)
2h30m => DURATION(2h30m)
250ms => DURATION(250ms)
1d => DURATION(1d)
"a\nb" => STRING(a,b)
r"C:\raw" => RAW_STRING
"a{x:>8}b" => STRING(parts)
"{{" => STRING({)
"{x" => !VS0108
"""a\nb""" => MULTI_STRING
b"\x00\x01" => BYTES
b"ố" => !VS0106
p"./a.txt" => PATH
'ố' => CHAR(ố)
'ố' (NFD) => CHAR(ố)   (* sau NFC *)
'ab' => !VS0105
set{1,2} => SET_LBRACE INT(1) PUNCT(,) INT(2) PUNCT(})
set {1} => IDENT(set) PUNCT({) INT(1) PUNCT(})
x??y => IDENT ?? IDENT
x?.y => IDENT ?. IDENT
f()? => IDENT ( ) ?
a|b => IDENT | IDENT
a|>b => IDENT |> IDENT
2**3 => INT ** INT
# c => COMMENT
## doc => DOC_COMMENT
ls -5 => WORD(ls) WORD(-5)
ls -la => WORD(ls) WORD(-la)
ls -- -x => WORD(ls) WORD(--) WORD(-x)
echo "a b" => WORD(echo) STRING(a b)
echo 'a b' => WORD(echo) RAW_STRING(a b)
$HOME => DOLLAR IDENT(HOME)
$env.PATH => DOLLAR env . IDENT(PATH)
a;b => WORD(a) PUNCT(;) WORD(b)
a & => WORD(a) PUNCT(&)
a > b => WORD(a) PUNCT(>) WORD(b)
https://x/y#frag => WORD(https://x/y#frag)
^git => CARET WORD(git)
\tif => !VS0107 INDENT
if x:\n    y\n  z => !VS0109
(1 +\n 2) => INT + INT
x |> f => IDENT |> IDENT
1_ => !VS0104
```

(60 dòng bao gồm cả ca lỗi; ST-04 phải chạy đúng toàn bộ.)
