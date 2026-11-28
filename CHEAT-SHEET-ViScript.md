<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Cheat sheet — ViScript ↔ Python ↔ Rust ↔ TS/JS

> **Đây là tài liệu HỌC, không phải nguồn sự thật của dự án.**
> Sinh ngày **2026-11-16**, kiểm chứng bằng `viscript/target/debug/vis` @ commit `f4005aed`
> (111 lượt chạy snippet). Nguồn sự thật của ngôn ngữ: code + `viscript/test/sem/**`,
> [`docs/viscript/SPEC-lex.md`](docs/viscript/SPEC-lex.md), [`docs/viscript/ViScript-thiet-ke.md`](docs/viscript/ViScript-thiet-ke.md).

**Quy ước**

- ✔ = **đã chạy thật** đúng như ô ViScript trong lần kiểm chứng.
- ✖ = thiết kế có nhưng **chưa có tại `<commit>`**; cột ghi chú nêu cách thay thế.
- Cột Python/Rust/TS chỉ để **so cho quen mắt**, không phải cú pháp ViScript.
- Bảng "Mức hiện thực VS2" ở cuối tài liệu thiết kế là nơi ghi chênh lệch thiết kế ↔ code.

**Chạy thử**

```bash
vis run x.vis          # chạy (không cần build)
vis check x.vis        # kiểm kiểu + cú pháp, in mã lỗi VSxxxx
vis fmt x.vis          # định dạng, luôn 4 dấu cách
vis test x.vis         # chạy mọi `test fn`
vis explain VS0412     # giải thích mã lỗi
vis pkg add "gói@^1"   # gói + vis.lock
```

**Từ khoá** (đủ, không thêm bớt):
`let var const fn cmd type enum trait impl alias newtype pub use if elif else match for in while
loop break continue return and or not true false none self async await scope test extern derives with has dyn ref`

---

## 1. Khai báo & giá trị

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `print("hi")` ✔ | `print("hi")` | `println!("hi")` | `console.log("hi")` | `print` nhận **đúng 1** đối số (2 đối số ⇒ `VS0401`) |
| `let x = 1` ✔ | `x = 1` | `let x = 1;` | `const x = 1` | **bất biến** — gán lại ⇒ `VS0412` |
| `var x = 1`<br>`x += 1` ✔ | `x = 1` | `let mut x = 1;` | `let x = 1` | biến đổi được |
| `const N: int = 10` ✔ | — | `const N: i64 = 10;` | `const N = 10` | **bắt buộc ghi kiểu** (`const N = 10` ⇒ `VS0201`) |
| `let x: int = 1` ✔ | `x: int = 1` | `let x: i64 = 1;` | `const x: number = 1` | kiểu đặt sau `:` |
| `let x = 1` → `let x = 2` (cùng phạm vi) ✖ | OK | OK | OK | `VS0302` khai báo trùng; **khác** phạm vi thì che tên được ✔ |
| `# chú thích` | `#` | `//` | `//` | `##` là doc-comment |
| `assert 1 + 1 == 2` ✔ | `assert ...` | `assert!(...)` | `console.assert(...)` | |

## 2. Hàm & closure

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `fn add(a: int, b: int) -> int:`<br>`    a + b` ✔ | `def add(a, b):`<br>`    return a + b` | `fn add(a: i64, b: i64) -> i64 { a + b }` | `function add(a: number, b: number) { return a + b }` | **biểu thức cuối khối = giá trị trả về** |
| `fn f(a: int, b: int = 2) -> int:` ✔ | `def f(a, b=2)` | — | `function f(a, b = 2)` | tham số mặc định |
| `fn f(a: int, --b: int = 2) -> int:`<br>`f(1, --b: 5)` ✔ | `def f(a, b=2)`<br>`f(1, b=5)` | — | — | tham số tên kiểu **cờ CLI** |
| `x => x * 2` ✔ | `lambda x: x * 2` | `\|x\| x * 2` | `x => x * 2` | |
| `fn(x: int) -> int:`<br>`    x + 1` ✔ | `lambda x: x + 1` | `\|x: i64\| x + 1` | `(x: number) => x + 1` | closure nhiều dòng |
| `fn id[T](x: T) -> T:` ✔ | `def id[T](x: T) -> T:` (3.12) | `fn id<T>(x: T) -> T` | `function id<T>(x: T): T` | generic dùng **`[]`**, không `<>` |
| `fn fact(n: int) -> int:`<br>`    if n <= 1: 1 else: n * fact(n - 1)` ✔ | đệ quy | đệ quy | đệ quy | `if` là **biểu thức** |
| `test fn cong():` ✔ | `def test_cong():` | `#[test] fn cong()` | `test("cong", ...)` | chạy bằng `vis test` → `ok 1 - cong` |

## 3. Điều khiển

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `if x > 0: ...`<br>`elif x < 0: ...`<br>`else: ...` ✔ | y hệt | `if x > 0 { } else if ...` | `if (x > 0) { } else if ...` | `elif` (không phải `else if`) |
| `let s = if ok: "y" else: "n"` ✔ | `"y" if ok else "n"` | `let s = if ok { "y" } else { "n" };` | `ok ? "y" : "n"` | `if` là biểu thức khi có `else` |
| `for i in 0..3:` ✔ | `for i in range(3):` | `for i in 0..3 {` | `for (let i = 0; i < 3; i++)` | `..` **không** gồm cận trên |
| `for i in 0..=2:` ✔ | `for i in range(3):` | `for i in 0..=2 {` | như trên | `..=` gồm cận trên |
| `for x in [1, 2, 3]:` ✔ | `for x in [1,2,3]:` | `for x in xs {` | `for (const x of xs)` | |
| `for (k, v) in m:` ✔ | `for k, v in m.items():` | `for (k, v) in &m {` | `for (const [k,v] of Object.entries(m))` | |
| `while i < 3:`<br>`    i += 1` ✔ | y hệt | `while i < 3 { }` | `while (i < 3)` | |
| `loop:`<br>`    if done(): break` ✔ | `while True:` | `loop { if done() { break } }` | `while (true)` | `continue` cũng có ✔ |

## 4. `match` & pattern

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `match s:`<br>`    Circle(r): "tròn {r}"`<br>`    Empty: "rỗng"` ✔ | `match s:`<br>`    case Circle(r): ...` | `match s {`<br>`    Shape::Circle(r) => ...,`<br>`    Shape::Empty => ... }` | `switch (s.kind)` | **phải xét hết**; thiếu nhánh ⇒ `VS0402` ("`match` thiếu nhánh: B") |
| `Circle(r) if r > 100.0: "lớn"` ✔ | `case Circle(r) if r > 100:` | `Shape::Circle(r) if r > 100.0 => ...` | — | guard **không** tính vào coverage |
| `A(x) \| B(x): x` ✔ | `case A(x) \| B(x):` | `A(x) \| B(x) => x` | — | or-pattern; `\|` **không** phải bit-or |
| `_` | `case _:` | `_ =>` | `default:` | bắt buộc với kiểu vô hạn (`int`, `str`…) |
| `if let v = x:`<br>`    print(v)` ✔ | `if (v := x) is not None:` | `if let Some(v) = x {` | `if (x != null)` | có `else:` ✔ |

## 5. Dữ liệu

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `[1, 2, 3]` / `list[int]` ✔ | `[1,2,3]` / `list[int]` | `vec![1,2,3]` / `Vec<i64>` | `[1,2,3]` / `number[]` | |
| `{"a": 1, "b": 2}` / `map[str, int]` ✔ | `{"a": 1}` | `HashMap::from([("a",1)])` | `new Map([["a",1]])` | |
| `set{1, 2}` ✔ | `{1, 2}` | `HashSet::from([1,2])` | `new Set([1,2])` | `set{` **dính liền** |
| `(1, "a")` ✔ | `(1, "a")` | `(1, "a")` | `[1, "a"] as const` | `let (n, s) = t` ✔ |
| `type U:`<br>`    name: str`<br>`    age: int` ✔ | `@dataclass`<br>`class U:` | `struct U { name: String, age: i64 }` | `type U = { name: string; age: number }` | tên `type`, không `struct` |
| `let u: U = {name: "An", age: 20}` ✔ | `U(name="An", age=20)` | `U { name: "An".into(), age: 20 }` | `const u: U = {name: "An", age: 20}` | record literal + **phải ghi kiểu** |
| `u with age: 21` ✔ | `dataclasses.replace(u, age=21)` | `U { age: 21, ..u }` | `{...u, age: 21}` | trả bản ghi **mới** |
| `enum Shape:`<br>`    Circle(r: float)`<br>`    Empty` ✔ | `Enum` không mang dữ liệu | `enum Shape { Circle(f64), Empty }` | `type Shape = {kind:'circle', r:number} \| {kind:'empty'}` | enum **có dữ liệu** |
| `Circle(2.0)` ✔ | `Circle(2.0)` | `Shape::Circle(2.0)` | `{kind:'circle', r:2.0}` | dựng variant bằng `Tên(...)` |
| `a == b` trên bản ghi ✔ | `a == b` | `a == b` | `a === b` | so theo giá trị |
| `newtype Email = str` ✖ | `NewType('Email', str)` | `struct Email(String);` | branded type | khai báo qua được, nhưng `Email("x")` ⇒ "`str` không phải hàm" |
| `alias Id = int` ✔ | `Id = int` | `type Id = i64;` | `type Id = number;` | alias **cùng** kiểu |

## 6. Chuỗi & literal

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `"Xin chào {name}"` ✔ | `f"Xin chào {name}"` | `format!("Xin chào {name}")` | `` `Xin chào ${name}` `` | nội suy **ngay trong `"..."`**, không cần tiền tố |
| `"{{ok}}"` ✔ | `"{{}}"` | `"{{}}"` | — | `{{` `}}` cho ra ngoặc thật; in `{ok}` |
| `r"C:\x"` ✔ | `r"C:\x"` | `r"C:\x"` | `String.raw` | thô, **không** escape, không nội suy |
| `"""a`<br>`b"""` ✔ | `"""a`<br>`b"""` | `"a\nb"` | template literal | nhiều dòng |
| `b"\x41"` ✔ | `b"\x41"` | `b"\x41"` | `Uint8Array` | chỉ ASCII + `\xNN` |
| `p"./x"` ✔ | `Path("./x")` | `Path::new("./x")` | — | literal đường dẫn, kiểu `path` |
| `"a,b".split(",")` ✔ | `"a,b".split(",")` | `"a,b".split(',')` | `"a,b".split(",")` | |
| `"ab".upper()` / `.lower()` / `.chars()` / `.contains("b")` / `.len()` ✔ | `.upper()` / `list(s)` / `"b" in s` / `len(s)` | `.to_uppercase()` / `.chars()` / `.contains('b')` / `.len()` | `.toUpperCase()` / `[...s]` / `.includes("b")` | **đúng 6** method của `str` |
| `"a" + "b"` ✖ | `"a" + "b"` | `format!` / `String + &str` | `"a" + "b"` | **lỗi kiểu `VS0412`** → dùng `"{a}{b}"` hoặc `str.join` |
| `"{n:.2}"` ✖ | `f"{n:.2f}"` | `format!("{n:.2}")` | `n.toFixed(2)` | format spec **chưa hỗ trợ** ở VS0 |

**Literal đặc biệt** (không ngôn ngữ nào trong 3 cái kia có):

| Viết | Kiểu | Ghi chú |
|---|---|---|
| `1_000`, `0xff`, `0b1010` ✔ | `int` (i64) | tràn i64 là **lỗi biên dịch** |
| `3.14`, `1e-9` ✔ | `float` (f64) | `1.` **sai** (bắt buộc có số sau dấu chấm) |
| `5mb`, `5kib` ✔ | `size` | `kb`=1000, `kib`=1024 |
| `250ms`, `2h30m` ✔ | `duration` | in ra `150m` (chuẩn hoá) |
| `50pct` ✔ | `percent` | `%` **chỉ** là modulo |

## 7. Optional & xử lý lỗi

| ViScript | Python | Rust | TS/JS | Ghi chú |
|---|---|---|---|---|
| `int?` / `none` ✔ | `int \| None` / `None` | `Option<i64>` / `None` | `number \| undefined` | không có `null` toàn nơi |
| `x ?? 7` ✔ | `x if x is not None else 7` | `x.unwrap_or(7)` | `x ?? 7` | |
| `u?.age` ✔ | — | `u.map(\|u\| u.age)` | `u?.age` | |
| `Some(5)` (dựng) ✔<br>`some(v)` / `none` (pattern) ✔ | — | `Some(5)` / `Some(v) =>` | — | **dựng viết hoa, pattern viết thường** |
| `Result[int, str]`, `Ok(7)`, `Err("hỏng")` ✔ | exception | `Result<i64, String>` | `throw` / union | |
| `let v = f()?` ✔ | `v = f()` (try/except) | `let v = f()?;` | `try { f() }` | |
| `let v = f() else e:`<br>`    print(e)`<br>`    0` ✔ | `try/except` | `match f() { ... }` | `catch` | `else` phải trả **cùng kiểu** hoặc thoát |
| `assert cond` ✔ | `assert` | `assert!(...)` | `console.assert` | |
| `panic("hỏng")` ✖ | `raise` | `panic!` | `throw` | chưa có ở VS0 |
| `AnyError` ✖ | — | `Box<dyn Error>` | `unknown` | chưa có — dùng `str` làm kiểu lỗi |

## 8. Trait, `impl`, `derives`

| ViScript | Python | Rust | TS/JS |
|---|---|---|---|
| `trait Hien:`<br>`    fn hien(self) -> str` ✔ | `class P(Hien):` / `Protocol` | `trait Hien { fn hien(&self) -> String; }` | `interface Hien { hien(): string }` |
| `impl Hien for P:`<br>`    fn hien(self) -> str: ...` ✔ | kế thừa | `impl Hien for P { ... }` | `class P implements Hien` |
| `type P derives Show:` ✔ | — | `#[derive(Debug)]` | — |
| `dyn Hien` ✖ | — | `dyn Trait` | — |

`derives` sinh sẵn: `Eq`, `Ord`, `Hash`, `Show`, `Json`, `Default` — không cần viết tay.

## 9. Thư viện chuẩn (đóng băng — `viscript/agent/API-STD.txt`)

Gọi dạng **module.chấm_hàm**, sau khi `use std.<module>`:

| Module | Hàm chính | Ví dụ đã chạy ✔ |
|---|---|---|
| `std.fs` | `exists` `list` `read` `write` | `fs.exists(".")` → `true` |
| `std.list` | `sort` `take` `drop` `chunk` `concat` `reverse` `unique` | `list.sort([3,1,2])` → `[1,2,3]` |
| `std.map` | `keys` `values` `merge` `remove` `get_or` | `map.keys(m)` |
| `std.str` | `split` `join` `upper` `lower` `trim` `replace` `repeat` `starts_with` `ends_with` `contains` | `str.join(["a","b"], "-")` → `a-b` |
| `std.path` | `join` `basename` `dirname` `ext` `stem` `is_abs` | `path.join("a","b")` → `a/b` |
| `std.time` | `now_ms()` `monotonic_ms()` | |
| `std.json` | `encode` `decode` `encode_pretty` | `json.decode("[1,2]")` → `Ok([1, 2])` |
| `std.net` | `listen` `connect` `read` `write` `accept` `close` | loopback, **không TLS** |
| `std.http` | `get` `head` + `_timeout` | HTTP/1.1 loopback, chunked, redirect GET ≤5 |

Method trên giá trị (không cần `use`):

| Kiểu | Method ✔ |
|---|---|
| `list` | `len` `get` `first` `last` `push` `map` `filter` `each` `sum` `join` |
| `map` | `len` `get` `has` `keys` `values` |
| `str` | `len` `upper` `lower` `split` `chars` `contains` |

## 10. Chế độ shell (chỉ ViScript có)

| Viết | Ghi chú |
|---|---|
| `ls ./photos \| where size > 5mb \| sort_by size --desc \| take 10` | pipeline trong `.vish` / ViShell |
| `pub cmd ls(path: path = p".") -> stream[FileEntry]:` | `cmd` = hàm + tự sinh `--help` + cờ `--x` |
| `cmd { ls (dir) \| where size > 5mb }` | dùng pipeline ngay trong `.vis` |
| `extern cmd git(sub: GitSub, --oneline: bool, ...rest: str)` | khai chữ ký cho chương trình ngoài |
| `^ten` | ép gọi chương trình **ngoài** |
| `\| complete` | chặn lỗi pipeline, trả `{stdout, stderr, code, how}` |

## 11. Những thứ **không có** (và thay bằng gì)

| Không có | Thay bằng | Python | Rust | TS/JS |
|---|---|---|---|---|
| `class`, kế thừa | `type` + `impl` | `class` | `struct` + `impl` | `class` |
| `try/catch`, exception | `Result` + `?` + `else err:` | `try/except` | `Result` | `try/catch` |
| `null` / `undefined` | `none` + `T?` | `None` | `Option` | `null/undefined` |
| `switch` | `match` | `match` | `match` | `switch` |
| `any` | `Json` + chuyển đổi tường minh | — | — | `any` |
| `1 < x < 10` | `1 < x and x < 10` | **hợp lệ** | lỗi | lỗi |
| `s[i]`, `s[1:3]` | `.chars()`, `xs.get(i)` | **hợp lệ** | không có | **hợp lệ** |
| `"a" + "b"` | `"{a}{b}"`, `str.join` | **hợp lệ** | **hợp lệ** | **hợp lệ** |
| `1.` (float) | `1.0` | **hợp lệ** | **hợp lệ** | **hợp lệ** |
| `++`, `--` | `x += 1` | — | — | **có** |
| macro (`println!`) | `print(...)` | — | **có** | — |
| `unsafe`, con trỏ, borrow | ngữ nghĩa giá trị + `ref[T]` | — | **có** | — |

## 12. Mười cái bẫy cho người mới

1. **`let` là bất biến** — ngược với `let` của JS. Muốn gán lại: `var`.
2. **`var` không phải `var` của JS** (không function-scope) — nó chỉ là "biến đổi được", giống `let mut`.
3. **`"a" + "b"` là lỗi kiểu** — nối chuỗi bằng nội suy `"{a}{b}"`.
4. **`match` phải xét hết**; nhánh có `if` (guard) **không** được tính là đã xét.
5. **`Some`/`None`/`Ok`/`Err` viết hoa khi dựng**, nhưng pattern optional viết **thường**: `some(v)`, `none`.
6. **Record literal phải liệt kê đủ trường** — giá trị mặc định của `type` **không** tự áp:
   `type U: email: str? = none` thì vẫn phải viết `{name: …, email: none}`.
7. **`const` phải ghi kiểu**: `const N: int = 10`.
8. **`?` chỉ dùng cho `T?`/`Result`** — không có `xs[i]?`; chỉ số âm/ngoài phạm vi dùng `.last()` / `.get(i)`.
9. **`print` nhận đúng 1 đối số** (khác Python).
10. **Thụt lề**: đơn vị thụt lề do **cấp đầu tiên trong tệp** quyết định; trộn 2 và 4 dấu cách là lỗi; tab báo `VS0107` (vẫn lex tiếp để `vis fmt` sửa).

## 13. Phụ lục — cách kiểm chứng & chênh lệch đã biết

- **Cách kiểm**: mỗi ô ✔ là một snippet độc lập chạy `vis run`; ✖ là snippet trả mã lỗi, đã đọc thông điệp.
  Tổng **111 lượt chạy**, ngày 2026-11-16, binary `viscript/target/debug/vis` @ `f4005aed`.
- **Chênh lệch thiết kế ↔ code đã gặp** (theo thứ tự nguồn sự thật, **code thắng**):
  `User(name: "An")` (thiết kế §3.2) **chưa có** → dùng `{...}` + `let u: U = …`;
  `AnyError` **chưa có** → dùng `str`; format spec `{x:.2}` **chưa có**;
  `dyn` **chưa dùng được**; `panic(...)` **chưa có**; `newtype` khai báo được nhưng chưa dựng được;
  `async` chạy **tuần tự một luồng**; `chan(0)` bị từ chối `VS0505`.
- **Nguồn đọc thêm**: `viscript/test/sem/**` (447 ca ngữ nghĩa chạy thật),
  [`docs/viscript/SPEC-lex.md`](docs/viscript/SPEC-lex.md) (từ vựng + EBNF),
  mục "Mức hiện thực VS2" cuối [`docs/viscript/ViScript-thiet-ke.md`](docs/viscript/ViScript-thiet-ke.md).
