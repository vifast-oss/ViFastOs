# ViScript + ViShell: Tài liệu thiết kế

Người viết: Agent 0 (Copilot). Đối tượng: người điều phối + Agent 2.
Quan hệ với ViFastOs: ViScript là **sản phẩm độc lập**, chạy trên Linux, macOS, Windows, WebAssembly và ViFastOs. Tài liệu `ViFastOs-v2-thiet-ke.md` (mục 7.3) chỉ mô tả phần tích hợp; mọi quyết định về ngôn ngữ nằm ở đây.
Giấy phép mã nguồn: **MIT OR Apache-2.0** (giống Rust).

Nhãn: [ƯL] ước lượng cần đo lại; [GT] giả thuyết cần kiểm.

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
Phụ lục A. Ngữ pháp rút gọn
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

1. **Một ngôn ngữ, hai chế độ cú pháp** (script và lệnh), cùng cây cú pháp, cùng kiểm kiểu, cùng VM.
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

- Phần mở rộng `.vis` (script), `.vish` (tệp toàn bộ ở chế độ lệnh). UTF-8 bắt buộc. Dòng đầu tuỳ chọn `#!/usr/bin/env vis`.
- **Từ khoá tiếng Anh** (giống Python, Rust, JS/TS).
- **Định danh Unicode** theo UAX #31: được viết tiếng Việt, tiếng Nhật (`số_lượng`, `tính_tổng`). So sánh sau chuẩn hoá NFC, nên `ố` gõ bằng tổ hợp hay dựng sẵn đều là một tên.
- Lint cảnh báo định danh trộn chữ dễ nhầm (ví dụ Latin + Cyrillic giống nhau) để tránh tấn công homoglyph.

Danh sách từ khoá: `let var fn cmd type enum trait impl alias newtype pub use as if elif else match for in while loop break continue return and or not true false none self async await scope test extern derives with has dyn ref`.

### 2.2 Khối và thụt lề

- Khối mở bằng `:` cuối dòng, thân thụt lề **4 dấu cách**. Tab bị cấm (lỗi từ vựng, `vis fmt` tự sửa).
- Trong ngoặc `()`, `[]`, `{}` được xuống dòng tự do.
- Thân ngắn một dòng sau `:` được phép: `if x > 0: return x`.

### 2.3 Chú thích

- `#` tới cuối dòng.
- `##` là tài liệu, gắn với khai báo kế tiếp (dùng cho `vis doc` và LSP).

### 2.4 Literal

| Loại | Ví dụ | Kiểu |
|---|---|---|
| Số nguyên | `42`, `1_000_000`, `0xff`, `0b1010` | `int` |
| Số thực | `3.14`, `1e-9` | `float` |
| Bool | `true`, `false` | `bool` |
| Chuỗi | `"xin chào {name}"`, `r"C:\raw"`, `"""nhiều dòng"""` | `str` |
| Ký tự | `'a'`, `'ố'` | `char` |
| Bytes | `b"\x00\x01"` | `bytes` |
| Kích thước | `5kb`, `5kib`, `2mb`, `1gib` | `size` |
| Thời lượng | `250ms`, `2h30m`, `1d` | `duration` |
| Phần trăm | `50%` | `percent` |
| Đường dẫn | `p"./data/a.txt"` (script), `./data/a.txt` (chế độ lệnh) | `path` |
| List | `[1, 2, 3]` | `list[int]` |
| Map | `{"a": 1, "b": 2}` | `map[str, int]` |
| Set | `set{1, 2}` | `set[int]` |
| Tuple | `(1, "a")` | `(int, str)` |
| Bản ghi ẩn danh | `{name: "An", age: 20}` | `{name: str, age: int}` |

`kb` = 1000, `kib` = 1024. Chuỗi nội suy `"{expr}"`, định dạng `"{x:>8}"`, `{{` để ra `{`.

---

## 3. Hệ kiểu

### 3.1 Kiểu dựng sẵn

| Nhóm | Kiểu |
|---|---|
| Số | `int` (i64, tràn là panic; phép quấn tường minh `wrapping_add`), `float` (f64) |
| Logic, ký tự | `bool`, `char` |
| Văn bản | `str` (UTF-8 bất biến), `bytes` |
| Hệ thống | `path`, `size`, `duration`, `percent`, `instant`, `datetime` |
| Tập hợp | `list[T]`, `map[K, V]`, `set[T]`, `(A, B, ...)` |
| Rỗng, lỗi | `T?`, `Result[T, E]`, `never`, `()` |
| Hàm | `fn(A, B) -> C` |
| Luồng | `stream[T]` (lười, kéo) |
| Dữ liệu động | `Json` (enum: Null, Bool, Num, Str, Arr, Obj) |
| Tham chiếu chia sẻ | `ref[T]`, `weak[T]` |
| Quyền | `cap[Dir]`, `cap[File]`, `cap[Net]`, `cap[Proc]`, `cap[Env]`, `cap[Term]`, `cap[Service[P]]` |

`bigint`, `decimal` là thư viện, không nằm trong lõi.

### 3.2 Kiểu người dùng

```python
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

- `derives` sinh cài đặt trait chuẩn: `Eq`, `Ord`, `Hash`, `Show`, `Json`, `Default`.
- Tạo bản ghi: `User(name: "An", age: 20)`; tham số có tên bắt buộc khi bản ghi có hơn 2 trường.
- Cập nhật: `u with age: 21` trả bản ghi mới.

### 3.3 Kiểu cấu trúc cho dữ liệu bảng

Bản ghi ẩn danh so khớp theo cấu trúc. Hàm có thể nhận "mọi bản ghi có trường `size: size`":

```python
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

### 3.6 Phép chia

- `int / int -> int` (cắt về 0; chia 0 là panic), `float / float -> float`.
- `int.div_floor`, `int.checked_div -> int?`.
- Lint cảnh báo khi chia hai literal nguyên (`1 / 2`), gợi ý `1.0 / 2.0`.

---

## 4. Biểu thức, câu lệnh, pattern

### 4.1 Biến

```python
let x = 10            # bất biến
var count = 0         # biến đổi được
count += 1
let (a, b) = pair
let {name, age} = user
```

### 4.2 Điều khiển

```python
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

### 4.3 match

```python
match s:
    Circle(r) if r > 100.0: "lớn"
    Circle(r): "tròn {r}"
    Rect(w, h): "chữ nhật {w}x{h}"
    Empty: "rỗng"
```

- Phải xét hết; thiếu nhánh là lỗi kiểm kiểu, liệt kê nhánh thiếu.
- Pattern: literal, biến, `_`, tuple, bản ghi `{name, age: 18..}`, enum, list `[first, ..rest]`, khoảng `0..10`, `|`, guard `if`.

### 4.4 Optional

```python
let e = user.email ?? "không có"
let domain = user.email?.split("@").last()
if let mail = user.email:
    send(mail)
```

### 4.5 Toán tử

Số học `+ - * / % **`; so sánh `== != < <= > >=`; logic `and or not`; bit bằng hàm `bit_and`, `bit_or`, `bit_xor`, `shl`, `shr` (để `|` dành cho pipe); nối hàm `|>`: `xs |> filter(even) |> sum()`.

---

## 5. Hàm, closure, generic, trait

### 5.1 Hàm

```python
pub fn greet(name: str, times: int = 1, --loud: bool = false) -> str:
    let s = "Xin chào {name}" * times
    if loud: s.upper() else: s
```

- Tham số vị trí, tham số mặc định, tham số có tên bắt đầu bằng `--` (chỉ truyền theo tên; thành cờ trong chế độ lệnh).
- Biểu thức cuối khối là giá trị trả về; `return` để trả sớm.
- Tham số bất biến.

### 5.2 Closure

`x => x * 2`, `(a, b) => a + b`, hoặc nhiều dòng:

```python
let f = fn(x: int) -> int:
    let y = x * 2
    y + 1
```

Closure bắt biến theo giá trị. Muốn chia sẻ trạng thái biến đổi: `ref[T]`.

### 5.3 Generic

```python
fn first[T](xs: list[T]) -> T?:
    if xs.is_empty(): none else: xs[0]

fn max_by[T, K: Ord](xs: list[T], key: fn(T) -> K) -> T?: ...
```

Chỉ định tường minh khi cần: `json.parse[list[User]](text)`.

### 5.4 Trait

```python
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

### 5.5 Phương thức

`impl User:` thêm phương thức. Gọi `u.method()`. Không UFCS.

---

## 6. Xử lý lỗi

### 6.1 Result và `?`

```python
fn read_config(dir: cap[Dir]) -> Result[Config, Error]:
    let text = dir.read_text("config.json")?
    let cfg = json.parse[Config](text)?
    Ok(cfg)
```

`?` cần lỗi nguồn chuyển được sang lỗi đích (trait `Into`). `Error` là kiểu lỗi chung có thông điệp, nguyên nhân lồng, vị trí.

### 6.2 `else` xử lý tại chỗ

```python
let cfg = read_config(dir) else err:
    print("Dùng cấu hình mặc định: {err}")
    Config.default()
```

Khối `else` phải trả giá trị cùng kiểu hoặc thoát (`return`, `break`, `panic`).

### 6.3 Optional sang Result

`x.ok_or("thiếu email")?`

### 6.4 panic

Cho vi phạm bất biến: `xs[i]` ngoài phạm vi (dùng `xs.get(i) -> T?` nếu không chắc), tràn số, `assert`. panic dừng task hiện tại, in vết ngăn xếp có vị trí nguồn. Không bắt được trong code thường; chỉ ranh giới task/shell bắt để báo cáo.

### 6.5 Lỗi trong `main`

`fn main(...) -> Result[(), Error]`: lỗi được in đẹp, mã thoát 1.

---

## 7. Mô hình giá trị và bộ nhớ

### 7.1 Ngữ nghĩa giá trị

Mọi kiểu (trừ `ref`, `cap`, `stream`, kênh) có ngữ nghĩa giá trị:

```python
var a = [1, 2, 3]
var b = a
b.push(4)        # a vẫn là [1, 2, 3]
```

Cài đặt: đối tượng heap đếm tham chiếu; ghi vào đối tượng có số tham chiếu > 1 thì sao chép trước (copy-on-write).

### 7.2 Không GC truy vết

Ngữ nghĩa giá trị không tạo được chu trình. Chu trình chỉ có qua `ref[T]`; dùng `weak[T]` để phá. Bản debug có bộ phát hiện chu trình báo rò rỉ trong test. Không GC truy vết nên không có khoảng dừng, phù hợp giao diện và âm thanh.

### 7.3 `ref[T]`

```python
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
- Cấm vòng phụ thuộc giữa module.
- Gói: thư mục có `vis.toml` (tên, phiên bản, giấy phép, phụ thuộc theo đường dẫn, git URL + commit, hoặc registry sau này), `vis.lock` khoá phiên bản + hash nội dung.
- Script một tệp khai phụ thuộc ở đầu tệp:

```python
#!/usr/bin/env vis
#| deps: http = "git+https://example.org/vis-http@a1b2c3"
```

- Gói không có mã chạy lúc cài đặt.

---

## 9. Chế độ lệnh (shell)

### 9.1 Ý tưởng

Chế độ lệnh là **cú pháp thứ hai**, được parser dịch sang **cùng cây cú pháp** với chế độ script. Dùng ở:
- Mỗi dòng gõ trong ViShell.
- Khối `cmd { ... }` trong tệp `.vis`.
- Tệp `.vish`.

### 9.2 Khai báo lệnh

```python
## Liệt kê tệp
pub cmd ls(path: path = ".", --all: bool = false, --long: bool = false) -> stream[FileEntry]:
    ...
```

- `cmd` là hàm bình thường, cộng thêm: tự sinh `--help`, gọi được bằng cú pháp lệnh, tham số `--x` thành cờ, `bool` thành cờ không giá trị.
- Lệnh nhận `ctx: Ctx` ngầm (thư mục hiện tại, môi trường, terminal, quyền của phiên). Hàm thường không có `ctx`.

### 9.3 Phân tích một dòng lệnh

```
pipeline   := stage ('|' stage)*  ['&']
stage      := command arg*   |   '(' expr ')'
command    := tên cmd | biến hàm | chương trình ngoài | '^' tên (ép gọi ngoài)
arg        := word | '--flag' [value] | '$' ident ('.' ident)* | '(' expr ')' | string | closure-arg
```

- **Tham số được hiểu theo chữ ký lệnh.** Từ trần `./photos` vào tham số `path` thành `path`; `5mb` vào `size` thành `size`; `*.txt` vào tham số `list[path]`/`glob` được mở rộng; vào `str` thành chuỗi. Không có tách từ và trích dẫn kiểu Bash.
- Chương trình ngoài: mọi tham số là `str`, glob mở rộng trước khi truyền.
- Biểu thức: `$x`, `$x.field`, `(expr)`.
- Tham số kiểu hàm (`where`, `each`, `sort_by`...) được phân tích ở **chế độ biểu thức**, có biến ngầm `it` và **viết tắt trường**: `where size > 5mb` nghĩa là `where it => it.size > 5mb`.

### 9.4 Xung đột ký hiệu và cách giải

| Ký hiệu | Chế độ lệnh | Chế độ script |
|---|---|---|
| `\|` | Pipe | Không dùng (bit OR là `bit_or`) |
| `>` | So sánh (trong tham số biểu thức) | So sánh |
| Ghi tệp | `\| save out.txt`, `\| save --append log.txt` | `file.write(...)` |
| `*` | Glob trong từ trần | Nhân |
| `-x`, `--x` | Cờ | Trừ |

Không dùng `>` để chuyển hướng nên không bao giờ mập mờ.

### 9.5 Luồng dữ liệu

- `stream[T]` lười, kéo, có backpressure, huỷ khi bị bỏ. `take 10` dừng nguồn sớm.
- Chương trình ngoài: stdout là `stream[bytes]`; chuyển đổi bằng `| lines`, `| from json`, `| from csv`, `| from toml`; ngược lại `| to json`.
- Mã thoát khác 0 thành lỗi pipeline; `| complete` để nhận `{stdout, stderr, code}`.
- Khai chữ ký cho chương trình ngoài để có gợi ý và kiểm tham số:

```python
extern cmd git(sub: GitSub, --oneline: bool, ...rest: str)
```

### 9.6 Hiển thị

Giá trị cuối pipeline được hiển thị qua trait `Render`: `list` bản ghi thành bảng, bản ghi thành khối khoá-giá trị, `stream[bytes]` in thẳng. ViTerm trên ViFastOs nhận dữ liệu có cấu trúc thay vì văn bản.

### 9.7 Ví dụ

```python
> ls ./photos | where size > 5mb | sort_by modified --desc | take 10
> let big = ls ./photos | where size > 5mb
> big | each f => f.copy_to(./backup)
> open users.json | where age >= 18 | get name
> git log --oneline | lines | take 5
> ps | where cpu > 50% | each p => p.kill()
> http get https://api.example.com/items | from json | to csv | save items.csv
```

Trong script:

```python
fn backup_big(ctx: Ctx, dir: path) -> Result[int, Error]:
    let files = cmd { ls (dir) | where size > 5mb } |> collect()
    for f in files:
        f.copy_to(p"./backup")?
    Ok(files.len())
```

---

## 10. Bất đồng bộ và song song

### 10.1 async có cấu trúc

```python
async fn fetch_all(net: cap[Net], urls: list[str]) -> list[Result[str, Error]]:
    scope s:
        let tasks = urls.map(u => s.spawn(net.get_text(u)))
        tasks.map(t => await t)
```

- `async fn`, `await`. Mọi task con nằm trong `scope`; ra khỏi `scope` thì task con đã xong hoặc bị huỷ. Không task mồ côi.
- Huỷ (Ctrl-C, timeout) lan theo cây `scope`, điểm huỷ tại mỗi `await`.
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

### 11.2 Khai báo trong `main`

```python
fn main(data: cap[Dir] @ "./data", out: cap[Dir] @ "./out" (write), net: cap[Net] @ "api.example.com") -> Result[(), Error]:
    ...
```

- `@` khai đường dẫn/host mặc định yêu cầu; ghi đè bằng `vis run app.vis --grant data=./other`.
- `vis run --dry` in danh sách quyền chương trình yêu cầu.

### 11.3 Shell

- Phiên ViShell có quyền của người dùng (`ctx`).
- Chạy một tệp `.vis` từ shell: chỉ trao quyền nó khai trong `main`; shell hỏi khi quyền lần đầu được dùng (nhớ theo hash tệp).
- `cmd` trong cấu hình shell chạy với `ctx` của phiên.

### 11.4 Mức bảo đảm theo nền tảng

| Nền tảng | Bảo đảm bởi |
|---|---|
| ViFastOs | Kernel (capability thật); runtime chỉ giữ cap được trao |
| Linux | Runtime kiểm; tăng cường bằng Landlock + seccomp khi có |
| macOS | Runtime kiểm [GT: khả năng sandbox của hệ điều hành] |
| Windows | Runtime kiểm; AppContainer sau |
| Wasm | Môi trường trình duyệt |

Ở chế độ hạn chế quyền, plugin native bị tắt.

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
    // tiến trình
    fn spawn(&self, spec: &ProcSpec) -> HResult<ProcHandle>;     // argv, env, cwd handle, stdio
    // wait, kill, list_procs ...
    // mạng: tcp_connect, tcp_listen, udp_bind, dns_resolve ...
    // thời gian
    fn now_mono(&self) -> Instant;
    fn now_wall(&self) -> DateTime;
    // môi trường, terminal: env_get, env_vars, term_size, term_raw_mode, term_read_key ...
    fn reactor(&self) -> &dyn Reactor;
    fn sandbox_harden(&self, grants: &Grants) -> HResult<()>;
}
```

- Đường dẫn trong lõi là `VPath` (luôn `/`, UTF-8; tên không UTF-8 giữ dạng bytes thoát). Backend chuyển sang đường dẫn gốc.
- Backend: `vis-host-posix` (Linux, macOS, BSD), `vis-host-windows`, `vis-host-wasm`, `vis-host-vifast`, `vis-host-mock` (test, tất định).
- **Bộ hợp chuẩn Host** chạy giống nhau trên mọi backend (mục 18).

---

## 13. Thư viện chuẩn

| Module | Nội dung |
|---|---|
| `core` | kiểu dựng sẵn, helper Option/Result, trait chuẩn |
| `text` | chuỗi, grapheme, chuẩn hoá Unicode, regex, định dạng, so sánh theo locale cơ bản |
| `coll` | list, map, set, deque, heap, sắp xếp ổn định |
| `stream` | map, filter, take, skip, chunk, zip, group_by, sort_by, uniq, sum, count, collect |
| `fs` | Dir, File, đọc/ghi, duyệt, watch, glob |
| `path` | xử lý `path` |
| `proc` | chạy chương trình, pipe, mã thoát |
| `net` | TCP, UDP, DNS, `http` client, TLS (thuần Rust) |
| `time` | instant, datetime, múi giờ, duration |
| `fmt` | json, toml, csv, base64, hex (yaml là thư viện ngoài) |
| `crypto` | hash (SHA-2, BLAKE3), random an toàn |
| `term` | màu, bảng, prompt, tiến độ |
| `test` | `test fn`, assert, snapshot |
| `sync` | Mutex, kênh, Once |
| `shell` | `cmd` chuẩn: ls, cd, open, save, where, each, sort_by, get, select, take, lines, from, to, ps, kill, http, walk, ... |
| `vifast` | chỉ có trên ViFastOs: binding sinh từ IDL |

Viết bằng ViScript nhiều nhất có thể; phần cần nhanh (regex, hash, json parse) là Rust trong runtime. Phụ thuộc Rust phải thuần Rust và giấy phép tương thích MIT/Apache-2.0.

---

## 14. Kiến trúc cài đặt

### 14.1 Đường xử lý

```
nguồn (.vis / dòng lệnh)
  └─ lexer (2 chế độ, trạng thái thụt lề)
       └─ parser (đệ quy xuống, phục hồi lỗi) ──> CST không mất thông tin (cho fmt, LSP)
            └─ hạ xuống AST
                 └─ phân giải tên + module
                      └─ kiểm kiểu (hai chiều, ràng buộc trait, xét hết match)
                           └─ HIR có kiểu
                                ├─ interpreter cây (mô hình tham chiếu)
                                └─ sinh bytecode ──> xác minh bytecode ──> VM
                                       └─ cache theo hash
```

### 14.2 Crate

| Crate | Vai trò | Phụ thuộc |
|---|---|---|
| `vis-syntax` | lexer, parser, CST, AST, vị trí nguồn | |
| `vis-diag` | chẩn đoán, mã lỗi, bản dịch vi/en/ja | |
| `vis-types` | phân giải tên, kiểm kiểu, HIR | syntax, diag |
| `vis-interp` | interpreter cây trên HIR (mô hình tham chiếu, đơn giản) | types |
| `vis-bytecode` | định dạng bytecode, sinh mã, xác minh | types |
| `vis-vm` | VM, heap RC/CoW, scheduler task, FFI | bytecode |
| `vis-host` | trait Host + `vis-host-mock` | |
| `vis-host-{posix,windows,wasm,vifast}` | backend | host |
| `vis-std` | thư viện chuẩn (.vis + Rust) | vm, host |
| `vis-fmt` | định dạng mã từ CST | syntax |
| `vis-lsp` | language server | types, fmt |
| `vis-cli` | lệnh `vis` | tất cả |
| `vish-edit` | soạn dòng, tô màu, gợi ý | syntax, types |
| `vishell` | shell | cli, edit |
| `vis-plugin` | macro + ABI plugin Rust | vm |

Luật: `vis-syntax`, `vis-types`, `vis-bytecode`, `vis-vm` không phụ thuộc hệ điều hành (cổng build target wasm32). Tệp không quá 1.500 dòng. `unsafe` chỉ trong `vis-vm` (heap) và `vis-plugin`, có Miri. Mọi tệp có header SPDX `MIT OR Apache-2.0`.

### 14.3 Kiểm kiểu

- Phân giải tên hai lượt (khai báo trước, thân sau).
- Suy kiểu hai chiều, biến kiểu trong phạm vi một hàm; hợp nhất cục bộ.
- Trait: tra cài đặt theo kiểu cụ thể; generic kiểm một lần với ràng buộc.
- Xét hết `match` bằng thuật toán ma trận pattern.
- Phục hồi lỗi: kiểu `{error}` lan truyền im lặng, chỉ báo lỗi gốc.
- Kiểm tăng dần cho LSP và shell: cache theo module.

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

`<thư mục cache>/vis/<hash>.vbc`, hash = nguồn + phiên bản trình biên dịch + hash phụ thuộc. Mục tiêu khởi động script đã cache dưới 20 ms [ƯL].

### 14.6 Chẩn đoán

```
error[VS0412]: kiểu không khớp
  ┌─ app.vis:12:18
12│     let n: int = user.email
  │                  ^^^^^^^^^^ đây là `str?`, cần `int`
  = gợi ý: dùng `int.parse(user.email ?? "0")?` nếu email chứa số
```

Mã lỗi ổn định, `vis explain VS0412`. Ngôn ngữ thông báo theo cấu hình (mặc định theo locale hệ thống).

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
│ vis-types (kiểm tăng dần) │ vis-vm │ vis-std.shell          │
├────────────────────────────────────────────────────────────┤
│ Terminal trait: ANSI (POSIX) │ Windows console │ ViTerm      │
└────────────────────────────────────────────────────────────┘
```

| Thành phần | Thiết kế |
|---|---|
| REPL | đọc dòng, phân tích chế độ lệnh, kiểm kiểu, chạy, hiển thị; lỗi kiểu thì không chạy gì |
| Biến phiên | `let` trong shell sống hết phiên |
| Gợi ý | theo kiểu tham số: `path` gợi ý tệp, `enum` gợi ý biến thể, trường bản ghi trong `where`, cờ của `cmd`, lệnh ngoài theo `extern cmd` |
| Tô màu | theo CST + kết quả phân giải (lệnh không tồn tại tô đỏ) |
| Lịch sử | append-only + chỉ mục; lưu dòng, cwd, thời điểm, thời lượng, mã thoát; tìm mờ |
| Job | `&`, `jobs`, `fg`, `bg`, `kill`; POSIX dùng process group; ViFastOs qua process server |
| Huỷ | Ctrl-C huỷ pipeline qua token huỷ lan theo stream và scope |
| Tương thích | `^cmd` ép gọi chương trình ngoài; `sh "..."` chạy chuỗi qua `/bin/sh` khi thật cần |
| Prompt | hàm ViScript, có giới hạn thời gian |
| Khởi động | cấu hình cache bytecode; mục tiêu dưới 50 ms [ƯL] |
| Terminal | ViShell không tự là terminal; ViTerm nhận giao thức hiển thị có cấu trúc tuỳ chọn |

---

## 16. Công cụ

| Lệnh | Chức năng |
|---|---|
| `vis run f.vis [args]` | kiểm kiểu + chạy |
| `vis check` | chỉ kiểm |
| `vis fmt` | định dạng (một kiểu duy nhất) |
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
    pub fn resize(data: Bytes, w: u32, h: u32) -> Result<Bytes, VisError> { ... }
}
```

- Macro sinh chữ ký ViScript từ chữ ký Rust, kiểm lúc build.
- Nạp: liên kết tĩnh vào bản `vis` tuỳ biến (ưu tiên, dùng trên ViFastOs) hoặc thư viện động với ABI ổn định có số phiên bản.
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
| Lexer | test bảng token; fuzz không panic; thụt lề, tab bị cấm, NFC |
| Parser | **vòng tròn**: parse → fmt → parse cho AST bằng nhau; in lại CST đúng văn bản gốc; fuzz không panic; phục hồi lỗi có số lỗi hợp lý |
| Chế độ lệnh | bảng dòng lệnh → AST mong đợi; cùng chương trình viết hai chế độ ra cùng HIR |
| Kiểm kiểu | tệp `.vis` có chú thích kỳ vọng `# error: VS0412` hoặc `# ok`; mỗi mã lỗi có ca dương và ca âm; snapshot thông báo lỗi |
| Xét hết match | so với bộ liệt kê vét cạn trên enum nhỏ sinh ngẫu nhiên |
| Interpreter | mô hình tham chiếu, giữ đơn giản |
| Bytecode + VM | **vi sai**: bộ sinh chương trình đúng kiểu theo seed, chạy trên interpreter và VM, so đầu ra, mã thoát, lỗi; tự thu nhỏ ca lệch |
| Trình xác minh | đột biến bytecode hợp lệ phải bị từ chối hoặc vẫn an toàn |
| Heap RC/CoW | Miri; test rò rỉ (đối tượng sống về 0); test ngữ nghĩa giá trị |
| async/scope | lập lịch tất định theo seed trong `vis-host-mock`; không task mồ côi; huỷ lan đúng |
| Host | **bộ hợp chuẩn**: cùng tệp test chạy trên mọi backend; khác biệt hợp lệ khai báo rõ |
| Quyền | test âm: `..`, symlink ra ngoài, dẫn xuất rộng hơn đều phải bị chặn |
| Thư viện chuẩn | test `.vis` + so với triển khai tham chiếu trên host (ví dụ json với serde_json) |
| ViShell | phiên kịch bản qua pty giả: gõ phím, chụp màn hình văn bản, so snapshot; gợi ý; job; Ctrl-C |
| LSP | kịch bản JSON-RPC, snapshot |
| Hiệu năng | benchmark cố định (vòng lặp số, chuỗi, map, pipeline 1 triệu dòng, khởi động lạnh/ấm); lịch sử, cảnh báo hồi quy > 10% |

### 18.3 Hồi quy

Mỗi lỗi sửa có `tests/regress/<mã>.vis`. Ca lệch từ vi sai được thu nhỏ trước khi ghi.

### 18.4 Cổng

| Lệnh | Nội dung |
|---|---|
| `vis-dev check` | L0 crate bị chạm |
| `vis-dev gate --fast` | fmt, clippy, build wasm32 cho crate lõi, header SPDX, L0, L1 |
| `vis-dev gate` | + L2 |
| `vis-dev nightly` | L4 + benchmark + báo cáo |

Không retry. Không `#[ignore]` để qua cổng.

---

## 19. Lộ trình

| Mốc | Nội dung | Tiêu chí xong | Phụ thuộc |
|---|---|---|---|
| VS-SPEC | `SPEC.md` chuẩn: ngữ pháp đầy đủ (EBNF, cả chế độ lệnh), bảng ưu tiên toán tử, quy tắc kiểu, bảng mã lỗi | Agent 0 duyệt | |
| VS0 | syntax 2 chế độ + diag + types (kiểu cơ bản, bản ghi, enum, `T?`, Result, list/map, match xét hết, hàm, closure) + interpreter + bộ test ngữ nghĩa | ≥ 300 test `.vis`; fuzz parser 1 giờ không panic; vòng tròn fmt | VS-SPEC |
| VS1 | generic, trait, module, bytecode + VM + xác minh + cache, vis-std lõi, vis-host-posix, `vis run/check/fmt/test` | vi sai interp/VM 100.000 chương trình 0 lệch; khởi động ấm < 20 ms [ƯL] | VS0 |
| VSH1 | ViShell trên Linux/macOS: chế độ lệnh đầy đủ, stream, chương trình ngoài, gợi ý theo kiểu, lịch sử, job | dùng hằng ngày được; test pty xanh | VS1 |
| VS2 | async có cấu trúc, kênh, net/http, quyền + Landlock, vis-host-windows, vis-host-wasm, LSP, gói | bộ hợp chuẩn Host xanh 4 nền tảng | VS1 |
| VSH2 | ViShell trên ViFastOs + vis-host-vifast + binding IDL | thay shell tạm của ViFastOs | VS2, ViFastOs U1 |
| VS3 | plugin Rust, thread song song, binding toolkit, ViTerm hiển thị có cấu trúc | ứng dụng desktop nhỏ viết bằng ViScript | VS2, ViFastOs G3 |

VS-SPEC tới VS2 không phụ thuộc ViFastOs, chạy song song với kernel.

---

## 20. Quy trình Agent 2 và giao việc vòng 1

### 20.1 Quy trình

Dùng nguyên quy trình mục 11 tài liệu ViFastOs (QUEUE, STATE, JOURNAL, BLOCKED, DECISIONS, báo cáo MD + JSON, vòng 8 bước, luật cấm, ngân sách, chạy qua đêm), với các điểm riêng:

- Thư mục điều khiển `viscript/agent/`; `vifast-agentctl --project viscript`. Hai dòng có QUEUE riêng, không giẫm nhau.
- Lệnh cổng là `vis-dev ...` thay cho `v2 ...`.
- Thay đổi ngữ nghĩa ngôn ngữ (cú pháp, quy tắc kiểu) **luôn** qua Agent 0; Agent 2 cài đặt theo tài liệu này và SPEC. Chỗ chưa rõ: ghi BLOCKED kèm đề xuất + ví dụ, làm task khác.
- Lỗi phát hiện từ fuzz/vi sai: thu nhỏ, ghi `tests/regress/`, test đỏ trước rồi mới sửa.

### 20.2 Giao việc vòng 1

| ID | Task | Phụ thuộc | Tiêu chí xong |
|---|---|---|---|
| VT01 | Workspace `viscript/` theo 14.2 (crate rỗng biên dịch được), `viscript/agent/`, LICENSE-MIT + LICENSE-APACHE, cổng `vis-dev gate --fast` khung, build wasm32 cho crate lõi | | Cổng chạy |
| VT02 | `SPEC.md` từ tài liệu này: EBNF đầy đủ (cả chế độ lệnh), bảng ưu tiên toán tử, danh sách từ khoá, bảng mã lỗi ban đầu | | Gửi Agent 0 duyệt; **không chặn** VT03 tới VT09 (các task đó bám theo tài liệu này) |
| VT03 | `vis-diag`: cấu trúc chẩn đoán, render như 14.6, bảng dịch vi/en/ja | VT01 | Snapshot test |
| VT04 | Lexer 2 chế độ: thụt lề, literal (size, duration, percent, path, chuỗi nội suy), cấm tab, định danh Unicode + NFC | VT01 | Test bảng + fuzz 10 phút |
| VT05 | CST + parser chế độ script (khai báo, biểu thức, match, khối), phục hồi lỗi | VT04 | In lại CST đúng nguồn; fuzz |
| VT06 | Parser chế độ lệnh (pipeline, từ trần, cờ, `$x`, `(expr)`, closure-arg `it`, viết tắt trường) hạ về cùng AST | VT05 | Hai chế độ ra cùng AST |
| VT07 | `vis-fmt` tối thiểu + test vòng tròn | VT05 | parse → fmt → parse bằng nhau |
| VT08 | Runner bộ test ngữ nghĩa: đọc `.vis` + chú thích kỳ vọng, xuất TAP | VT01 | Chạy được với backend giả |
| VT09 | Bộ test cú pháp ban đầu ≥ 100 tệp lấy từ ví dụ trong tài liệu (cả hai chế độ) | VT05, VT06, VT08 | Xanh |
| VT10 | Báo cáo R1 (MD + JSON) | tất cả | Theo mục 11.7 tài liệu ViFastOs |

Kiểm kiểu và interpreter (phần còn lại của VS0) giao ở vòng 2 sau khi SPEC được duyệt. Nếu SPEC duyệt có thay đổi, Agent 2 cập nhật lexer/parser ở vòng 2.

---

## 21. Các lựa chọn đã chốt

| Mã | Câu hỏi | Chốt | Lý do |
|---|---|---|---|
| Q1 | Từ khoá tiếng Anh hay tiếng Việt? | **Tiếng Anh**; định danh Unicode cho phép tiếng Việt; thông báo lỗi có tiếng Việt | Giống JS/TS, Python, Rust: từ khoá tiếng Anh, tên biến Unicode. Từ khoá có dấu gõ chậm, phụ thuộc bộ gõ, mất quen thuộc với công cụ và tài liệu |
| Q2 | `int / int` cho `int` hay `float`? | `int`, có lint khi chia hai literal | Nhất quán với kiểu tĩnh, không chuyển đổi ngầm |
| Q3 | Thụt lề | 4 dấu cách bắt buộc, cấm tab | Một kiểu duy nhất, `vis fmt` tự sửa |
| Q4 | Tên | ViScript (`.vis`, lệnh `vis`), ViShell (`.vish`, lệnh `vish`) | Ngắn, nhất quán với hệ ViFast |
| Q5 | Chỉ số ngoài phạm vi | `xs[i]` panic, `xs.get(i)` trả `T?` | Lỗi lập trình hiện rõ; vẫn có đường an toàn |
| Q6 | Giấy phép | **MIT OR Apache-2.0** | Giống Rust; dễ dùng lại, có điều khoản bằng sáng chế của Apache |
| Q7 | `decimal` trong lõi | Không, là thư viện | Giữ lõi nhỏ |
| Q8 | Toán tử bit | Hàm (`bit_or`...), không ký hiệu | Giải phóng `\|` cho pipe ở mọi chế độ |

---

## 22. Quyết định

| Mã | Nội dung |
|---|---|
| VD-01 | ViScript có kiểu tĩnh bắt buộc, không `any`, không null |
| VD-02 | Một ngôn ngữ, hai chế độ cú pháp, cùng AST/kiểm kiểu/VM; ViShell là chương trình riêng |
| VD-03 | Ngữ nghĩa giá trị + RC + copy-on-write; không GC truy vết; chu trình chỉ qua `ref`, phá bằng `weak` |
| VD-04 | Lõi (syntax, types, bytecode, vm) không phụ thuộc hệ điều hành, build được wasm32 |
| VD-05 | Mọi truy cập hệ thống qua capability; không hàm toàn cục chạm hệ thống |
| VD-06 | Interpreter cây là mô hình tham chiếu, giữ mãi để kiểm vi sai VM |
| VD-07 | Không JIT; không `eval` chuỗi trong chế độ quyền hạn chế |
| VD-08 | Thay đổi SPEC chỉ qua Agent 0; mỗi tính năng mới phải có ví dụ + test trước |
| VD-09 | Từ khoá tiếng Anh, định danh Unicode (NFC), thông báo lỗi vi/en/ja |
| VD-10 | Giấy phép MIT OR Apache-2.0; phụ thuộc phải tương thích |

---

## 23. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Phạm vi ngôn ngữ phình | VD-08; lộ trình bậc; tính năng chỉ vào khi có ca dùng thật |
| Kiểm kiểu generic + trait phức tạp, thông báo lỗi khó hiểu | Suy kiểu cục bộ; generic kiểm một lần với ràng buộc; snapshot thông báo lỗi là test bắt buộc |
| Hai chế độ cú pháp mập mờ | Bảng xung đột 9.4 là một phần SPEC; mọi dòng lệnh mẫu có test AST |
| VM chậm | Lệnh có kiểu, không thẻ kiểu; benchmark từ VS1; chuyên biệt hoá generic khi có số đo |
| Ngữ nghĩa giá trị gây sao chép tốn | CoW + chuyển quyền khi RC = 1; đo trong benchmark |
| Quyền trên OS khác không chặt bằng ViFastOs | Ghi rõ mức bảo đảm (11.4); tăng cường Landlock |
| Hai dòng ViScript và ViFastOs tranh tài nguyên Agent 2 | QUEUE riêng; ViScript làm nhánh độc lập khi dòng OS BLOCKED |

---

## Phụ lục A. Ngữ pháp rút gọn (chế độ script)

```
file        := item*
item        := use | fn_decl | cmd_decl | type_decl | enum_decl | trait_decl | impl_decl | const_decl | test_decl | extern_cmd
fn_decl     := ['pub'] ['async'] 'fn' IDENT generics? '(' params ')' ['->' type] ':' block
cmd_decl    := ['pub'] 'cmd' IDENT '(' params ')' ['->' type] ':' block
params      := (param (',' param)*)?
param       := ['--'] IDENT ':' type ['=' expr] | ['var'] 'self'
type_decl   := ['pub'] 'type' IDENT generics? ['derives' path_list] ':' NEWLINE INDENT field+ DEDENT
enum_decl   := ['pub'] 'enum' IDENT generics? ['derives' path_list] ':' NEWLINE INDENT variant+ DEDENT
block       := stmt | NEWLINE INDENT stmt+ DEDENT
stmt        := 'let' pattern [':' type] '=' expr ['else' IDENT? ':' block]
             | 'var' IDENT [':' type] '=' expr
             | assign | 'return' expr? | 'break' | 'continue'
             | if | for | while | loop | match | scope | expr
expr        := lambda | pipe_expr
pipe_expr   := or_expr ('|>' or_expr)*
or_expr     := and_expr ('or' and_expr)*
and_expr    := not_expr ('and' not_expr)*
not_expr    := 'not' not_expr | cmp_expr
cmp_expr    := coalesce (cmp_op coalesce)?
coalesce    := range ('??' range)*
range       := add (('..' | '..=') add)?
add         := mul (('+' | '-') mul)*
mul         := unary (('*' | '/' | '%') unary)*
unary       := '-' unary | power
power       := postfix ('**' unary)?
postfix     := primary ( '.' IDENT | '?.' IDENT | '(' args ')' | '[' expr ']' | '?' | '[' types ']' | 'with' field_updates )*
primary     := literal | IDENT | '(' expr ')' | list | map | set | record | 'cmd' '{' cmd_pipeline '}' | if_expr | match_expr | 'await' postfix
type        := IDENT ['[' type (',' type)* ']'] ['?'] | '(' types ')' | 'fn' '(' types ')' '->' type | '{' field_types '}'
```

Chế độ lệnh: xem 9.3. Bản đầy đủ nằm trong `SPEC.md` (VT02).

## Phụ lục B. Chương trình mẫu

```python
#!/usr/bin/env vis
use std.fmt.json
use std.time

type Entry derives Json, Show:
    path: str
    size: size
    modified: datetime

## Tìm 20 tệp lớn nhất đã sửa trong 7 ngày, ghi báo cáo JSON
fn main(src: cap[Dir] @ ".", out: cap[Dir] @ "./reports" (write)) -> Result[(), Error]:
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

Cùng việc trong ViShell:

```python
> walk . | where is_file and modified >= (now) - 7d | sort_by size --desc | take 20 | to json --pretty | save reports/big-files.json
```

Định danh tiếng Việt:

```python
fn tính_tổng(danh_sách: list[int]) -> int:
    var tổng = 0
    for số in danh_sách:
        tổng += số
    tổng
```
