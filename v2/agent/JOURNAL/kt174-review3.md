# KT-174 — review đối kháng vòng 3 luật `E_STRICT_TEST_ONLY_WAKE` (KT-171)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer độc lập L16** · nhánh `night2/L16-review3` ·
worktree `/private/tmp/night2-L16-review3` · HEAD `2dc03fed` (từ `main`).
**Nguồn đọc trước**: [`kt171-wakefix3.md`](kt171-wakefix3.md), `v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 08:00 JST. Không merge, không push, không sửa `main`, không `gate --fast`,
không tạo worktree mới, không QEMU.

## 0. Phán quyết

**KHÔNG ĐẠT.** KT-171 đóng đúng **ba** lớp (B8/B9/B10) và **hai** lớp một phần (B6/B7, B4),
nhưng vẫn còn **8 cách lọt** — trong đó **2 cách chỉ dùng tệp trong cây** (không cần tệp ngoài,
không cần `build.rs`), `cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf`
**RC=0** và `--strict` **errors=0**. Vòng vá 4 là bắt buộc.

| # | Cách lọt | `cargo` | `--strict` | Lớp KT-171 bị phá |
|---|---|---|---|---|
| **H1** | `include!(r"…")` chuỗi **thô** trỏ ra ngoài cây | **RC=0** | **errors=0** | B6 |
| **H2** | `#[path = r"…"]` chuỗi **thô** trỏ ra ngoài cây | **RC=0** | **errors=0** | B7 |
| **H3** | `include!(concat!(env!(…)))` trỏ ra ngoài cây | **RC=0** | **errors=0** | B6 |
| **H4** | `include!` **lồng hai lớp** qua tệp test-only | **RC=0** | **errors=0** | B6 |
| **H5** | `include!` **một lớp** tệp test-only (`.rs`) từ tệp sản phẩm | **RC=0** (kbin **RC=0**) | **errors=0** | B6 + B8/B9 |
| **H6** | macro `v2/lib` **không** chứa token `wake`, che tên `concat!` + biến cục bộ | **RC=0** | **errors=0** | B4 + B10 |
| **H7** | macro `v2/lib` chứa `include!` (thân không có token `wake`) | **RC=0** | **errors=0** | B4 + B6 |
| **H8** | `#[path = r"…"]` chuỗi **thô** trỏ tới tệp test-only **trong cây** | **RC=0** (kbin **RC=0**) | **errors=0** | B7 + B8/B9 |

**Nặng nhất — H5**: một dòng `include!("evil_b.rs")` trong tệp sản phẩm, tệp đích là module
`#[cfg(test)]` (test-only theo đồ thị module). Bản vá B6 có ý định "sản phẩm thắng nếu cả hai"
nhưng **kết quả đó bị bỏ** cho mọi tệp `.rs` trong cây — xem §2.5. Đây là lỗ hổng **một dòng**,
không cần tệp ngoài, không cần `build.rs`, và `cargo check` **ảnh sản phẩm** (kbin riscv64) xanh.

## 1. Cách đã đóng (đối chứng — KT-171 làm đúng)

Mỗi ca dưới đây: đột biến **thật** trên cây, `cargo check` **RC=0**, `--strict` **ĐỎ** đúng chỗ,
rồi `git checkout --` + xoá tệp mới. Log ở `/tmp/kt174-mut/logs/`.

| Cách thử | Kết quả | Bằng chứng |
|---|---|---|
| **C1** — tệp `mod` từ **hai** nơi (một `#[cfg(test)]`, một sản phẩm qua `#[path]`) | **ĐỎ** (đúng: mọi đường phải test) | `E_STRICT_TEST_ONLY_WAKE … evil_dual.rs:4`; `errors=1` |
| **C2** — `#[path]` trỏ tới tệp **mồ côi** ngoài `src/` (chỉ tới được bằng `#[path]`) | **ĐỎ** (đúng: `extra` quét đích) | `E_STRICT_TEST_ONLY_WAKE … evil_orphan.rs:4`; `errors=1` |
| **C3** — `wake` gọi qua **trait object** `dyn WakeApi` | **ĐỎ** (đúng: token + `(`) | 2 lỗi: `…::wake` @ `evil_trait.rs:9`, `gọi wake(` @ `:14` |
| **C4** — `#[path = "…"]` **chuỗi thường** trỏ tới tệp test-only trong cây | **ĐỎ** (đúng: đồ thị module thấy cạnh sản phẩm) | `E_STRICT_TEST_ONLY_WAKE … evil_b.rs:4`; `errors=1` |
| **C5** — `#[cfg(feature = "ktest")]` trên `mod` + ảnh sản phẩm **bật** `ktest`? | **KHÔNG** bật (xem §3 mục 2) | `run.sh:75`, `repro-build.sh:189`, `soak.sh:314` — không `--features` |

C1 trả lời đúng câu hỏi 1 của đề: `test_only_modules` **có** đòi **mọi** đường đều test
(fixpoint "sản phẩm thắng" ở `check-wake-paths.py:621-631`). C3 trả lời câu hỏi 6: trait object
**bị bắt** vì token `wake` vẫn nằm trong mã sản phẩm.

## 2. Tám cách lọt — bằng chứng nguyên văn

Mỗi ca: mô tả đột biến, `cargo check` (host `-p vifast-core`, và với H5/H8 thêm **ảnh sản phẩm**
`-p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf`), `--strict`, gốc lỗi `file:dòng`,
đề xuất vá. Log đầy đủ ở `/tmp/kt174-mut/logs/` (không commit — log tạm).

### 2.1 H1 — `include!` chuỗi **thô** trỏ ra ngoài cây (B6)

Đột biến:
- `v2/lib/vifast-abi/src/evil_raw.rs`: `pub fn evil_raw(m: &mut crate::Machine, t: crate::ObjId)
  -> Result<(), crate::KError> { m.wake(t) }`
- `v2/kernel/vifast-core/src/core_sched.rs` (cuối tệp):
  `include!(r"../../../lib/vifast-abi/src/evil_raw.rs");`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s      RC=0
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `RE_INCLUDE_MACRO` (`check-wake-paths.py:650`) chỉ khớp `"…"`:
`r'\binclude!\s*\(\s*"([^"]+)"\s*\)'`. Chuỗi thô `r"…"` (và `br#"…"#`) **không** khớp ⇒
`resolve_include_targets` không thấy gì ⇒ không có `E_STRICT_WAKE_INCLUDE`, và tệp đích nằm
ngoài `v2/kernel`+`v2/sim` nên không bị quét token. Rust chấp nhận `include!(r"…")` (đã kiểm
bằng `rustc` độc lập: `/tmp/kt174-probe`).

### 2.2 H2 — `#[path = r"…"]` chuỗi **thô** trỏ ra ngoài cây (B7)

Đột biến:
- `v2/lib/vifast-abi/src/evil_path.rs`: như trên, thân `m.wake(t)`.
- `core_sched.rs`: `#[path = r"../../../lib/vifast-abi/src/evil_path.rs"] mod evil_path_mod;`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.95s      RC=0
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `RE_PATH_ATTR_ANY` (`:651`) và `RE_PATH_ATTR_LINE` (`:400`) cũng chỉ khớp `"…"`.
Hai hệ quả: (a) `resolve_include_targets` không báo `E_STRICT_WAKE_INCLUDE`; (b) `_mods_of`
(`:577`) không thấy `#[path]` ⇒ đồ thị module bỏ cạnh — với đích ngoài cây thì không sao, nhưng
xem **H8** (đích trong cây, test-only ⇒ lọt hẳn).

### 2.3 H3 — `include!(concat!(env!(…)))` trỏ ra ngoài cây (B6)

Đột biến:
- `v2/lib/vifast-abi/src/evil_concat.rs`: thân `m.wake(t)`.
- `core_sched.rs`:
  `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../lib/vifast-abi/src/evil_concat.rs"));`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s      RC=0
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `RE_INCLUDE_MACRO` chỉ khớp **literal trực tiếp**; `include!` có đối số là biểu
thức (`concat!`, `env!`) **không** khớp và **không** bị fail-closed. Đây là dạng rất thật
(`include!(concat!(env!("OUT_DIR"), "/gen.rs"))` — mã sinh). Đề xuất: mọi `include!` có mặt
trong tệp quét mà đối số **không** phải literal chuỗi ⇒ `E_STRICT_WAKE_INCLUDE` (fail-closed).

### 2.4 H4 — `include!` **lồng hai lớp** qua tệp test-only (B6)

Đột biến:
- `src/evil_a.rs`: `include!("evil_b.rs");`
- `src/evil_b.rs`: `pub fn evil_nested(m: &mut crate::Machine, t: crate::ObjId)
  -> Result<(), crate::KError> { m.wake(t) }`
- `src/lib.rs`: `#[cfg(test)] mod evil_a;` + `#[cfg(test)] mod evil_b;`
- `core_sched.rs`: `include!("evil_a.rs");`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s      RC=0
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Kiểm bằng API nội bộ của checker:

```
test_only: v2/kernel/vifast-core/src/evil_a.rs
test_only: v2/kernel/vifast-core/src/evil_b.rs
extra: v2/kernel/vifast-core/src/evil_a.rs is_test= False
extra: v2/kernel/vifast-core/src/evil_b.rs is_test= True     ← SAI: sản phẩm include evil_a → evil_b
errs: []
```

**Gốc lỗi**: `resolve_include_targets` (`:682-687`) dùng `resolved` để **không xử lý lại** một
tệp. `evil_a` được xử lý trước với `is_test=True` (từ `files`/`test_only`) ⇒ `evil_b` được
đẩy vào `extra` với `True`; khi tệp sản phẩm `core_sched.rs` include `evil_a` (⇒ `extra[evil_a]
= False`), `evil_a` đã `resolved` nên **không** lan `False` xuống `evil_b`. `extra[evil_b]`
giữ `True` ⇒ bị bỏ qua. Đề xuất: fixpoint trên đồ thị include (như `test_only_modules`), hoặc
tính đồ thị trước rồi lan "sản phẩm thắng" một lần.

### 2.5 H5 — `include!` **một lớp** tệp test-only (`.rs`) từ tệp sản phẩm (B6 + B8/B9) — **NẶNG NHẤT**

Đột biến:
- `src/evil_b.rs`: `pub fn evil_single(m: &mut crate::Machine, t: crate::ObjId)
  -> Result<(), crate::KError> { m.wake(t) }`
- `src/lib.rs`: `#[cfg(test)] mod evil_b;`
- `core_sched.rs`: `include!("evil_b.rs");`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.59s      RC=0
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.32s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Kiểm bằng API nội bộ:

```
evil_b in test_only: True
extra: v2/kernel/vifast-core/src/evil_b.rs is_test= False    ← sản phẩm thắng (đúng)
errs: []
```

**Gốc lỗi** (hai dòng, `check-wake-paths.py:1406-1422`):

```python
for p, text, fns in files:
    ...
    wake_targets.append((p, text, fns, is_test_file(p, root, test_only)))   # :1412
    added.add(p)
for relp, (p, is_test) in sorted(extra.items()):
    if p in added:                                                          # :1415
        continue
```

Mọi tệp `.rs` trong `v2/kernel`+`v2/sim` đều nằm trong `files`, nên **mọi** tệp đều lấy
`is_test` từ **đồ thị module** (`test_only`), **bỏ qua** kết quả "sản phẩm thắng" của đồ thị
`include!` trong `extra`. Vòng lặp `extra` chỉ có tác dụng với tệp **không** nằm trong `files`
(ví dụ `.inc`, hoặc tệp ngoài `src/`). Vì vậy đúng ca mà B6 nhắm tới — "đích **trong** cây ⇒
quét chính tệp đích (sản phẩm thắng nếu cả hai)" — **không hoạt động** với đích `.rs`.

Đề xuất vá (một dòng): trước khi quét, trừ các tệp bị include bởi sản phẩm khỏi `test_only`:

```python
product_includes = {relp for relp, (_, t) in extra.items() if not t}
test_only = test_only - product_includes
```

hoặc tại `:1412`: `is_test = is_test_file(...) and extra.get(relp, (None, True))[1]`.

### 2.6 H6 — macro `v2/lib` **không** chứa token `wake`, che tên `concat!` + biến cục bộ (B4 + B10)

Đột biến (`src/evil_macro.rs`, `mod evil_macro;` từ `lib.rs`):

```rust
macro_rules! concat {
    ($m:expr, $name:ident, $t:expr) => { $m.$name($t) };
}
pub fn evil_macro(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    let mut wake = 0u32;
    wake += 1;
    concat!(m, wake, t)
}
```

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s      RC=0
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Kiểm chứng macro thật sự sinh lời gọi: đổi `wake` → `wake_typo` trong `concat!` ⇒
`error[E0599]: no method named wake_typo found for mutable reference &mut Machine` — tức bản
`wake` biên dịch **đúng là** `m.wake(t)`.

**Gốc lỗi**: `_macro_arg_of` (`:905-921`) trả tên macro theo **cú pháp**; `SAFE_VALUE_MACROS`
(`:795-816`) tin tên đó là macro **chỉ-đọc-giá-trị**. Nhưng `macro_rules! concat` **che** macro
chuẩn cùng tên, và thân nó biến ident thành **tên hàm**. Token `wake` là đối số macro ⇒ nhánh
`:1461` bỏ qua (vì `concat` nằm trong danh sách an toàn) ⇒ rơi xuống `_exempt_local_wake`
(`:1466`) ⇒ biến cục bộ `wake` cùng khối miễn ⇒ **lọt**. Đề xuất: (a) quét toàn cây tìm
`macro_rules! <tên>` trùng tên trong `SAFE_VALUE_MACROS`; nếu có ⇒ **không** coi tên đó an toàn;
hoặc (b) bỏ hẳn miễn trừ "macro an toàn" khi có biến cục bộ cùng tên (chấp nhận báo oan
`assert_eq!(wake, 0)` — nhưng khi đó phải xử lý ca B10c).

### 2.7 H7 — macro `v2/lib` chứa `include!` (thân **không** có token `wake`) (B4 + B6)

Đột biến:
- `v2/lib/vifast-abi/src/lib.rs`:
  ```rust
  #[macro_export]
  macro_rules! evil_inc {
      () => { include!("../../../lib/vifast-abi/src/evil_inc_body.rs") };
  }
  ```
- `v2/lib/vifast-abi/src/evil_inc_body.rs`: `m.wake(t)` (một biểu thức).
- `core_sched.rs`:
  ```rust
  pub fn evil_inc_probe(m: &mut Machine, t: ObjId) -> Result<(), KError> {
      vifast_abi::evil_inc!();
      Ok(())
  }
  ```

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s      RC=0
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `scan_lib_macros` (`:740-789`) quét thân `macro_rules!` trong `v2/lib/**` tìm
**token** `wake` — thân `include!("…")` không có token `wake` ⇒ xanh. `resolve_include_targets`
chỉ lặp trên `files` = `v2/kernel`+`v2/sim` (`:680`) ⇒ `include!` trong `v2/lib` **không** được
phân giải. Đích `evil_inc_body.rs` nằm trong `v2/lib` ⇒ không bị quét token. Đề xuất: trong
`scan_lib_macros`, phát hiện `include!`/`#[path]` trong thân macro ⇒ fail-closed (đường dẫn
`include!` trong macro giải theo **tệp gọi**, không theo tệp định nghĩa — đã kiểm bằng `rustc`
độc lập: `/tmp/kt174-probe2`; không thể phân giải tĩnh an toàn).

### 2.8 H8 — `#[path = r"…"]` chuỗi **thô** trỏ tới tệp test-only **trong cây** (B7 + B8/B9)

Đột biến:
- `src/evil_b.rs`: `pub fn evil_raw_path(m: &mut crate::Machine, t: crate::ObjId)
  -> Result<(), crate::KError> { m.wake(t) }`
- `src/lib.rs`: `#[cfg(test)] mod evil_b;`
- `core_sched.rs`: `#[path = r"evil_b.rs"] mod evil_b_product;`

```
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.55s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Đối chứng: **cùng** đột biến nhưng `#[path = "evil_b.rs"]` (chuỗi thường) ⇒ **ĐỎ**:
`E_STRICT_TEST_ONLY_WAKE … evil_b.rs:4` (`errors=1`). Vậy lỗ hổng nằm đúng ở chuỗi thô.

**Gốc lỗi**: `RE_PATH_ATTR_LINE` (`:400`) không khớp `r"…"` ⇒ `_mods_of` (`:577`) không thấy
`#[path]` ⇒ đồ thị module **bỏ cạnh sản phẩm**; `evil_b` chỉ còn cạnh `#[cfg(test)]` ⇒
`test_only=True` ⇒ bị bỏ qua. `RE_PATH_ATTR_ANY` (`:651`) cũng không khớp ⇒ không có
`E_STRICT_WAKE_INCLUDE`. Đây là lỗ hổng **hoàn toàn trong cây**, không cần tệp ngoài.

## 3. Trả lời trực tiếp bảy câu hỏi của đề

1. **Tệp `mod` từ hai nơi (test + sản phẩm)** — `test_only_modules` **có** đòi **mọi** đường
   đều test (fixpoint "sản phẩm thắng", `:621-631`). Đột biến C1 ⇒ **ĐỎ**. **ĐÓNG.**
2. **`#[cfg(feature = "ktest")]` trên `mod` + ảnh sản phẩm bật `ktest`?** — **Không** bật:
   `v2/test/boot/run.sh:75` build ảnh sản phẩm **không** `--features`; `repro-build.sh:189`
   dùng `${VIFAST_REPRO_CARGO_EXTRA:-}` (mặc định rỗng); `soak.sh:314` chỉ bật
   `product-shell,product-fs` (không kéo `ktest`; `vblk = ["block","ktest"]` nhưng ảnh sản phẩm
   dùng `block`, không `vblk`). Vậy **đường trực tiếp ĐÓNG** — nhưng **giả định** "ktest ⇒
   test-only" chỉ đúng chừng nào ảnh sản phẩm còn không bật `ktest`; và **biến thể `include!`**
   (H5 với `#[cfg(feature = "ktest")] mod evil_b;`) vẫn **lọt** (đã kiểm: kbin RC=0,
   `--strict` errors=0 — log `m11`).
3. **`#[path]` trỏ trong `v2/kernel` tới tệp mồ côi** — tệp `.rs` mồ côi vẫn bị `iter_rs` quét
   (sản phẩm) ⇒ **ĐỎ** (C2). Tệp **không** `.rs` (`.inc`) mồ côi chỉ tới được bằng `#[path]`
   từ tệp test-only thì đúng là test-only; nếu tệp sản phẩm cũng include thì "sản phẩm thắng"
   hoạt động **một lớp** — nhưng **lồng hai lớp** thì lọt (H4). **ĐÓNG một phần.**
4. **`include!` lồng hai lớp** — **LỌT** (H4). **HỞ.**
5. **Macro `v2/lib` không chứa token `wake` nhưng sinh `wake`** — **LỌT** theo hai đường:
   che tên macro an toàn + biến cục bộ (H6), và `include!` trong thân macro (H7). **HỞ.**
6. **`wake` qua method resolution trên trait object (`dyn WakeApi`)** — **ĐỎ** (C3): token
   `wake` vẫn nằm trong mã sản phẩm, cả `::wake` lẫn `wake(` đều bị bắt. **ĐÓNG.**
7. **Cách khác** — ba đường nữa: chuỗi thô `include!` (H1), chuỗi thô `#[path]` (H2/H8),
   `include!(concat!(…))` (H3). **HỞ.**

## 4. Gốc lỗi gộp — đề xuất vòng vá 4

| Gốc | Dòng | Ảnh hưởng | Vá đề xuất |
|---|---|---|---|
| **G1** — regex đường dẫn chỉ khớp `"…"`, không khớp `r"…"`/`br#"…"#` | `:400`, `:650`, `:651` | H1, H2, H8 | Nhận `(?:b\|c)?r(#*)?"…"\1` (hoặc parse literal đúng); áp cho **cả** `_mods_of` và `resolve_include_targets` |
| **G2** — `include!` đối số **không** literal bị bỏ qua im lặng | `:650`, `:698-705` | H3 | Mọi `include!` có mặt mà đối số không phải literal chuỗi ⇒ `E_STRICT_WAKE_INCLUDE` (fail-closed) |
| **G3** — "sản phẩm thắng" của đồ thị include **bị bỏ** cho tệp trong `files` | `:1406-1422` | H5 (+ biến thể `ktest`) | `test_only -= {relp : extra[relp].is_test == False}` trước khi quét, hoặc `is_test = is_test_file(...) and extra.get(relp, (None, True))[1]` |
| **G4** — `resolved` chặn lan "sản phẩm thắng" xuống con | `:682-687` | H4 | Fixpoint/worklist: khi `extra[p]` đổi `True → False`, xử lý lại `p` và lan tiếp |
| **G5** — `scan_lib_macros` chỉ tìm token `wake`, không tìm `include!`/`#[path]` | `:740-789` | H7 | Phát hiện `include!`/`#[path]` trong thân macro `v2/lib` ⇒ fail-closed |
| **G6** — `SAFE_VALUE_MACROS` tin **tên** macro, không kiểm macro bị che | `:795-816`, `:905-921`, `:1461` | H6 | Quét `macro_rules! <tên>` trùng tên an toàn trong toàn cây ⇒ tên đó **không** an toàn; hoặc bỏ miễn trừ khi có biến cục bộ cùng tên |
| **G7** — selftest thiếu ca cho 8 đường trên | `:1670-2400` | — | Thêm 8 ca âm (H1..H8) + đối chứng dương (chuỗi thường, `#[path]` thường) |

**Thứ tự vá đề xuất**: G3 (một dòng, đóng H5 — nặng nhất) → G1 (đóng H1/H2/H8) → G2 (H3) →
G5 (H7) → G4 (H4) → G6 (H6) → G7 (khóa hồi quy). Sau vá: `--strict` phải **ĐỎ** cả 8 đột biến
trong §2, và 39 ca selftest cũ vẫn xanh.

## 5. Còn hở / chưa review (ghi rõ, **không** tuyên bố đã đóng)

1. **Chưa thử** macro `paste`/`concat_idents!` (proc-macro ngoài allowlist D2-21 — nếu có crate
   ngoài được thêm, đường "sinh ident" còn rộng hơn H6). Chưa thử `#[cfg_attr]` mở `path`.
2. **Chưa thử** `include!` trong `build.rs` sinh mã (H3 đã phủ dạng `concat!(env!("OUT_DIR"))`
   nhưng chưa dựng `build.rs` thật).
3. **`is_test_file_heuristic`** vẫn dùng cho mục 1–5 (kênh/arch/transition) — ngoài phạm vi
   KT-171, đã ghi ở `kt171-wakefix3.md` §4.1; **không** phải lỗ hổng của luật `wake`.
4. **Giả định build**: "`#[cfg(feature = "ktest")]` ⇒ test-only" chỉ đúng khi ảnh sản phẩm
   **không** bật `ktest`. Đã kiểm ba đường build hiện tại (`run.sh`, `repro-build.sh`,
   `soak.sh`); nếu tương lai thêm feature sản phẩm kéo `ktest` thì **mọi** module `ktest` thành
   sản phẩm mà checker vẫn miễn — cần ghim bằng máy (kiểm `Cargo.toml` + script build) hoặc
   ghi thành bất biến có test.
5. **Chưa chạy** `gate --fast` (luật task). `./v2/bin/v2 check` **PASS** trên cây sạch
   (`RC=0`, log `/tmp/kt174-mut/logs/v2check.log`).

## 6. Luật đã giữ

- Không merge, không push, không sửa `main`, không `gate --fast`, không tạo worktree mới.
- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`.
- Không nới ngưỡng/checker; không `#[ignore]`/`#[should_panic]`; không retry để lấy xanh.
- Không `pkill -f qemu-system`; task **không** dùng QEMU.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không
  `local a=1 b="$a"`.
- Mọi đột biến đã `git checkout --` + xoá tệp mới; cây sạch trước commit (chỉ journal này).
