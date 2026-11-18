# KT-178 — review đối kháng vòng 4 bốn bản vá H5/H1/H2/H3 (KT-174)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer độc lập L20** · nhánh `night2/L20-review4` ·
worktree `/private/tmp/night2-L20-review4` · HEAD `d68d7eb8` (từ `main`), bản vá cần phá
`c5f1ab00`.
**Nguồn đọc trước**: [`kt174-review3.md`](kt174-review3.md), `git show c5f1ab00`,
`v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 08:10 JST. Không merge, không push, không sửa `main`, không
`gate --fast`, không tạo worktree mới, không QEMU.

## 0. Phán quyết

**KHÔNG ĐẠT.** Bốn bản vá đóng **đúng** H1 và **một phần** H2/H3/H5; vẫn còn **năm cách lọt
thật** (mã sản phẩm biên dịch `wake`, `--strict` `errors=0`), trong đó **hai cách là biến thể
trực tiếp của chính bản vá** (H2 `r#"…"#`, H5 chuỗi `include!`), **một cách là H7 cũ chưa
đóng**, và **hai cách mới** (`cfg_attr` mở `path`, macro `v2/lib` chứa `#[path]`). Thêm một
**báo oan** (`include!` trong `#[cfg(test)]` của tệp sản phẩm) và **không có ca selftest nào**
cho bốn bản vá.

| Bản vá | Kết luận | Vì sao |
|---|---|---|
| **H1** `include!(r"…")` | **ĐẠT** | `r"…"` ĐỎ đúng; `r#"…"#` ĐỎ nhờ fail-closed H3 (thông điệp sai nhưng vẫn đỏ) |
| **H2** `#[path = r"…"]` | **KHÔNG ĐẠT** | `r"…"` ĐỎ đúng; **`r#"…"#` XANH** — regex `r?"…"` không khớp raw string có `#` |
| **H3** `include!` không literal | **ĐẠT CÓ ĐIỀU KIỆN** | tệp sản phẩm ĐỎ (fail-closed); tệp test-only không báo oan; nhưng **chuỗi qua tệp test-only** vẫn lọt (cùng gốc H5) |
| **H5** sản phẩm thắng | **KHÔNG ĐẠT** | include trực tiếp + hai nơi ĐỎ đúng; **chuỗi hai lớp XANH** — `_product_included` chỉ một lớp |

**Nặng nhất — H2 `r#"…"#`**: một dòng `#[path = r#"evil_b.rs"#] mod evil_b_product;` trong
`core_sched.rs` (sản phẩm) + `#[cfg(test)] mod evil_b;` trong `lib.rs` ⇒ ảnh sản phẩm
`vifast-kbin-riscv64` **RC=0**, `--strict` **errors=0**. Đây đúng là lớp H2 mà bản vá tuyên bố
đã đóng; regex mới `r?"…"` chỉ thêm `r` mà quên `#`. Cùng một dòng với đường dẫn ra **ngoài**
cây (`r#"../../../lib/…"#`) cũng lọt (E14).

## 1. Bảng thử nghiệm (17 ca)

Mỗi ca: đột biến **thật** trên cây, `cargo check` (host `-p vifast-core`; ca đánh dấu
**kbin** thêm `-p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf`), rồi
`python3 v2/tools/check-wake-paths.py --strict`, rồi `git checkout --` + xoá tệp mới.
Log tóm tắt ở `/tmp/kt178-mut/logs/summary.log` (không commit).

| # | Cách thử | `cargo` | `--strict` | Kết luận |
|---|---|---|---|---|
| **E1** | H5 trực tiếp: `include!("evil_b.rs")` từ `core_sched.rs`; `evil_b.rs` test-only (`#[cfg(test)] mod evil_b;`) | RC=0 (host + **kbin**) | **ĐỎ** `errors=1` (`evil_b.rs:2`) | **ĐÓNG** |
| **E2** | H5 **chuỗi**: product `include!("evil_a.rs")`; `evil_a.rs` `include!("evil_b.rs")`; cả hai test-only | RC=0 (host + **kbin**) | **XANH** `errors=0` | **HỞ** |
| **E3** | H5 **hai nơi**: `evil_b.rs` include từ product **và** từ `evil_t.rs` test-only | RC=0 | **ĐỎ** `errors=1` | **ĐÓNG** |
| **E4** | H2 `#[path = r#"evil_b.rs"#] mod evil_b_product;` trong product; `evil_b.rs` test-only | RC=0 (host + **kbin**) | **XANH** `errors=0` | **HỞ** |
| **E5** | H2 đối chứng `#[path = r"evil_b.rs"]` (không `#`) | RC=0 | **ĐỎ** `errors=1` | **ĐÓNG** |
| **E6** | H1 `include!(r"evil_b.rs")` | RC=0 | **ĐỎ** `errors=1` | **ĐÓNG** |
| **E7** | H1 `include!(r#"evil_b.rs"#)` | RC=0 | **ĐỎ** `errors=1` (`E_STRICT_WAKE_INCLUDE`, fail-closed H3) | **ĐÓNG** (đỏ, thông điệp sai) |
| **E8** | H3 `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/evil_b.rs"))` trong tệp **test-only** | RC=0 | **XANH** `errors=0` | **ĐÚNG** (không báo oan) |
| **E9** | H3 non-literal trong tệp **sản phẩm** | RC=0 | **ĐỎ** `errors=2` | **ĐÓNG** |
| **E10** | H7 cũ: macro `v2/lib/vifast-abi` chứa `include!("…/evil_inc_body.rs")` (thân **không** token `wake`), gọi từ `core_sched.rs` | RC=0 (host + **kbin**) | **XANH** `errors=0` | **HỞ** (H7 chưa đóng) |
| **E11** | `#[cfg_attr(not(test), path = "evil_b.rs")] mod evil_b_product;` | RC=0 (host + **kbin**) | **XANH** `errors=0` | **HỞ** (mới) |
| **E13** | macro `v2/lib` chứa `#[path = "evil.rs"] mod evil;` gọi từ `core_sched.rs`; `evil.rs` test-only | RC=0 (host + **kbin**) | **XANH** `errors=0` | **HỞ** (mới) |
| **E14** | H2 `#[path = r#"../../../lib/vifast-abi/src/evil_out.rs"#] mod evil_out_mod;` (đích **ngoài** cây) | RC=0 (host + **kbin**) | **XANH** `errors=0` | **HỞ** |
| **E15** | `include!("evil_b.rs")` trong `#[cfg(test)] mod evil_cfgtest_block { … }` của tệp **sản phẩm** | RC=0 | **ĐỎ** `errors=1` | **BÁO OAN** |
| **E16** | H5 chuỗi + non-literal: `evil_a.rs` (test-only, được product include trực tiếp) chứa `include!(concat!(…))` | RC=0 | **ĐỎ** `errors=1` | **ĐÓNG** (nhờ H3) |
| **E17** | H1 `include!(r#"evil_b.rs"#)` trong tệp test-only **được product include trực tiếp** | RC=0 | **ĐỎ** `errors=1` | **ĐÓNG** (fail-closed) |

Đối chứng nền: cây sạch `--strict` **errors=0**; `--selftest` **46/46 ok**; `./v2/bin/v2 check`
**PASS** (L0 compile OK). Không ca nào trong 46 ca selftest chạm `r#"…"#`, `include!` không
literal, hay chuỗi `include!` — bản vá **không thêm ca hồi quy nào**.

## 2. Bằng chứng nguyên văn cho năm cách lọt

### 2.1 E2 — H5 **chuỗi** (nặng, cùng lớp H5 gốc)

Đột biến:
- `v2/kernel/vifast-core/src/evil_a.rs`: `include!("evil_b.rs");`
- `v2/kernel/vifast-core/src/evil_b.rs`:
  `pub fn evil_nested(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `lib.rs`: `#[cfg(test)] mod evil_a;` + `#[cfg(test)] mod evil_b;`
- `core_sched.rs`: `include!("evil_a.rs");`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s      RC=0
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.06s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Kiểm bằng API nội bộ của checker (đúng như review3 §2.5):

```
test_only evil_a: True
test_only evil_b: True
extra: evil_a is_test= False
extra: evil_b is_test= True          ← SAI: sản phẩm include evil_a → evil_b
product_included: {'…/evil_a.rs', …}
after fix test_only evil_a: False
after fix test_only evil_b: True     ← vẫn test-only ⇒ bị bỏ qua
extra2: evil_b is_test= False        ← lần gọi 2 đã lan đúng, nhưng KHÔNG dùng
errs2: []
```

**Gốc lỗi**: bản vá H5 (`check-wake-paths.py:1173-1176`) chỉ chạy **một** lượt
`resolve_include_targets` với `test_only` **gốc**. `resolve_include_targets` xử lý tệp theo
thứ tự `queue` (LIFO) và **không xử lý lại** tệp đã `resolved` (`:686`, `:692`). Khi `evil_a`
được xử lý **trước** tệp sản phẩm include nó, `evil_b` được ghi `is_test=True`; sau đó
`extra[evil_a]=False` nhưng `evil_b` **không được xử lý lại** ⇒ `_product_included` chỉ có
`evil_a`. Lượt gọi thứ hai (`:1443`) **có** lan `False` xuống `evil_b` trong `extra`, nhưng
vòng quét `wake` dùng `is_test_file(p, root, test_only)` cho mọi tệp trong `files`
(`:1452`) và vòng `extra` **bỏ qua** tệp đã có trong `added` (`:1455`) ⇒ `evil_b` bị bỏ.
Đúng như review3 G3/G4 dự đoán: vá một dòng chỉ đóng **một lớp**.

### 2.2 E4/E14 — H2 raw string có `#`

Đột biến E4:
- `evil_b.rs`: `pub fn evil_rawpath(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `lib.rs`: `#[cfg(test)] mod evil_b;`
- `core_sched.rs`: `#[path = r#"evil_b.rs"#] mod evil_b_product;`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s      RC=0
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.40s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Đột biến E14 (đích **ngoài** cây, không cần tệp test-only):
- `v2/lib/vifast-abi/src/evil_out.rs`: `pub fn evil_out(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `core_sched.rs`: `#[path = r#"../../../lib/vifast-abi/src/evil_out.rs"#] mod evil_out_mod;`

```
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.57s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

Đối chứng E5: **cùng** đột biến nhưng `#[path = r"evil_b.rs"]` (không `#`) ⇒ **ĐỎ**
`E_STRICT_TEST_ONLY_WAKE … evil_b.rs:2`, `errors=1`. Vậy lỗ hổng nằm đúng ở `#`.

**Gốc lỗi**: `RE_PATH_ATTR_LINE` (`:407`) và `RE_PATH_ATTR_ANY` (`:659`) sau bản vá là
`r?"([^"]+)"` — khớp `r"…"` nhưng **không** khớp `r#"…"#` (sau `r` là `#`, không phải `"`).
Rust chấp nhận raw string với **mọi** số `#` trong `#[path]`. Hệ quả: `_mods_of` không thấy
cạnh sản phẩm (đồ thị module bỏ), `resolve_include_targets` không thấy `#[path]` (không có
`E_STRICT_WAKE_INCLUDE`), và tệp đích ngoài cây không bị quét.

### 2.3 E10 — H7 cũ: macro `v2/lib` chứa `include!`

Đột biến:
- `v2/lib/vifast-abi/src/evil_inc_body.rs`: `m.wake(t)`
- `v2/lib/vifast-abi/src/lib.rs`:
  ```rust
  #[macro_export]
  macro_rules! evil_inc {
      () => { include!("../../../lib/vifast-abi/src/evil_inc_body.rs") };
  }
  ```
- `core_sched.rs`: `pub fn evil_inc_probe(m: &mut Machine, t: ObjId) -> Result<(), KError> { vifast_abi::evil_inc!(); Ok(()) }`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.85s      RC=0
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.25s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `scan_lib_macros` (`:762`) chỉ tìm **token** `wake` trong thân `macro_rules!`;
thân `include!("…")` không có token `wake` ⇒ xanh. `resolve_include_targets` chỉ lặp trên
`files` = `v2/kernel`+`v2/sim` (`:673`) ⇒ `include!` trong `v2/lib` **không** được phân giải;
đích nằm trong `v2/lib` ⇒ không bị quét token. Đây đúng H7 của review3 §2.7 — **chưa đóng**.

### 2.4 E11 — `#[cfg_attr(…, path = "…")]`

Đột biến:
- `evil_b.rs`: `pub fn evil_cfgattr(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `lib.rs`: `#[cfg(test)] mod evil_b;`
- `core_sched.rs`: `#[cfg_attr(not(test), path = "evil_b.rs")] mod evil_b_product;`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.72s      RC=0
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.73s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `_attrs_before` (`:499`) có thu thập dòng `#[cfg_attr(…)]`, nhưng
`RE_PATH_ATTR_LINE` (`:407`) chỉ khớp `#[path = "…"]`; `RE_PATH_ATTR_ANY` (`:659`) cũng vậy.
`cfg_attr` mở `path` khi điều kiện đúng (`not(test)` đúng trong ảnh sản phẩm) ⇒ module nạp
`evil_b.rs`, checker không thấy đường nào.

### 2.5 E13 — macro `v2/lib` chứa `#[path]`

Đột biến:
- `evil.rs`: `pub fn evil_mod_fn(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `lib.rs`: `#[cfg(test)] mod evil;`
- `v2/lib/vifast-abi/src/lib.rs`:
  ```rust
  #[macro_export]
  macro_rules! evil_mod {
      () => { #[path = "evil.rs"] mod evil; };
  }
  ```
- `core_sched.rs`: `vifast_abi::evil_mod!();`

```
$ cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.78s      RC=0
$ cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.70s      RC=0   ← ẢNH SẢN PHẨM
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0                      RC=0
```

**Gốc lỗi**: `_mods_of` chỉ quét tệp trong `v2/kernel`+`v2/sim`; `mod` nằm trong thân macro
ở `v2/lib` không được thấy. `scan_lib_macros` chỉ tìm token `wake`, không tìm `#[path]`.
Đã kiểm bằng `rustc` độc lập (`/tmp/kt178-probe3`): `mod` trong `macro_rules!` giải theo
**tệp gọi** (`app/src/body_mod.rs`), nên `#[path = "evil.rs"]` trỏ đúng tệp test-only trong
cây — không cần tệp ngoài.

## 3. Báo oan — E15

Đột biến: `core_sched.rs` thêm

```rust
#[cfg(test)]
mod evil_cfgtest_block {
    include!("evil_b.rs");
}
```

`evil_b.rs` test-only, có `m.wake(t)`. `cargo check -p vifast-core` **RC=0** (include chỉ
biên dịch trong test), nhưng `--strict` **ĐỎ** `errors=1` (`evil_b.rs:2`).

**Gốc lỗi**: `resolve_include_targets` không dùng `test_line_ranges` để bỏ qua `include!`/
`#[path]` nằm trong khối `#[cfg(test)]` của tệp sản phẩm; nó gán `is_test` theo **cả tệp**.
Hệ quả: một `include!` hợp lệ trong test của tệp sản phẩm bị coi là sản phẩm ⇒ đích bị trừ
khỏi `test_only` ⇒ báo oan. Cây hiện tại **không** có `include!` nào nên chưa đỏ, nhưng đây
là hướng sai (fail-closed quá tay) và sẽ cắn khi có mã sinh/test dùng `include!`.

## 4. Gốc lỗi gộp — đề xuất vòng vá 5

| Gốc | Dòng | Ảnh hưởng | Vá đề xuất |
|---|---|---|---|
| **G1** — regex raw string chỉ `r?"…"`, không có `#` | `:407`, `:658`, `:659` | E4, E14 (H2) | Nhận `(?:r(#*)?"(.*?)"\1)` (hoặc parse literal đúng); áp cho **cả** `_mods_of` và `resolve_include_targets` |
| **G2** — H5 chỉ một lượt, `resolved` chặn lan | `:686`, `:692`, `:1173-1176`, `:1452-1455` | E2 (H5 chuỗi), E16 gián tiếp | Fixpoint: lặp `resolve_include_targets` cho tới khi `test_only` ổn định; **hoặc** dùng `extra` cho **mọi** tệp: `is_test = is_test_file(...) and extra.get(relp, (None, True))[1]` |
| **G3** — H3 fail-closed chỉ khi `not is_test` | `:715` | chuỗi qua tệp test-only | Sửa G2 là đủ; nếu không, kiểm `include!` không literal **theo tệp đến được từ sản phẩm** (không theo cờ `is_test` của tệp) |
| **G4** — `scan_lib_macros` chỉ tìm token `wake` | `:762` | E10 (H7), E13 | Phát hiện `include!`/`#[path]` trong thân `macro_rules!` của `v2/lib` ⇒ **fail-closed** (`E_STRICT_MACRO_WAKE` hoặc mã mới) |
| **G5** — `cfg_attr` mở `path` không được thấy | `:407`, `:499`, `:659` | E11 | Parse `cfg_attr(…, path = "…")`; coi là cạnh `#[path]` (fail-closed nếu điều kiện không tĩnh) |
| **G6** — `include!`/`#[path]` trong `#[cfg(test)]` của tệp sản phẩm bị coi là sản phẩm | `:673-760` | E15 (báo oan) | Dùng `test_line_ranges` để bỏ qua vùng test khi phân giải include/path |
| **G7** — không có ca selftest cho bốn bản vá | `:1713-2400` | — | Thêm ca âm E2/E4/E10/E11/E13/E14 + đối chứng dương E1/E3/E5/E6/E8/E9/E15 |

**Thứ tự vá đề xuất**: G1 (một regex, đóng H2 — nặng nhất) → G2 (fixpoint, đóng H5 chuỗi) →
G4 (đóng H7/E13) → G5 (cfg_attr) → G6 (báo oan) → G7 (khóa hồi quy). Sau vá: `--strict` phải
**ĐỎ** cả E2/E4/E10/E11/E13/E14, **XANH** E1/E3/E5/E6/E8/E9/E15, và 46 ca selftest cũ vẫn xanh.

## 5. Còn hở / chưa review (ghi rõ, **không** tuyên bố đã đóng)

1. **Chưa thử** `paste`/`concat_idents!` (proc-macro ngoài allowlist D2-21). `concat_idents!`
   còn unstable; `paste` không có trong allowlist ⇒ đường này chỉ mở nếu allowlist đổi.
2. **Chưa thử** `include!` trong `build.rs` sinh mã **thật**. Đã suy luận: `include!` với
   `concat!(env!("OUT_DIR"), …)` trong tệp sản phẩm **bị** H3 fail-closed (E9); `build.rs`
   ghi tệp vào `src/` rồi `include!("gen.rs")` literal thì tệp bị quét (sản phẩm); `#[path]`
   trỏ tệp chưa tồn tại ⇒ `E_STRICT_WAKE_INCLUDE` "không tồn tại" (fail-closed). Nhưng chưa
   dựng `build.rs` thật để xác nhận thứ tự quét/`OUT_DIR`.
3. **Chưa thử** macro `v2/lib` chứa `mod` **không** `#[path]` trỏ tệp trong `src/<mod>/` —
   theo lý thuyết tệp mồ côi `.rs` vẫn bị quét (sản phẩm) nên không lọt; chỉ `#[path]` mới
   trỏ được tệp test-only (E13).
4. **Chưa thử** `#[path]` với escape (`"evil\u{2e}rs"`): checker phân giải chuỗi **thô** nên
   báo "không tồn tại" (đỏ, fail-closed) — không phải lỗ hổng, nhưng là báo oan tiềm năng.
5. **Chưa thử** `include!` với `b"…"`/`c"…"` — `include!` đòi string literal, byte/C string
   không hợp lệ; nếu hợp lệ ở phiên bản Rust sau thì H3 fail-closed vẫn bắt (không khớp
   `RE_INCLUDE_MACRO`).
6. **Chưa chạy** `gate --fast` (luật task). `./v2/bin/v2 check` **PASS** trên cây sạch
   (`RC=0`); `--selftest` **46/46 ok**.
7. **Chưa review** phần còn lại của `check-wake-paths.py` ngoài luật `wake` (luật 1–5,
   `E_STRICT_TRANSITION`) — ngoài phạm vi bốn bản vá.

## 6. Luật đã giữ

- Không merge, không push, không sửa `main`, không `gate --fast`, không tạo worktree mới.
- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`.
- Không nới ngưỡng/checker; không `#[ignore]`/`#[should_panic]`; không retry để lấy xanh.
- Không `pkill -f qemu-system`; task **không** dùng QEMU.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không
  `local a=1 b="$a"`.
- Mọi đột biến đã `git checkout --` + xoá tệp mới; cây sạch trước commit (chỉ journal này).
