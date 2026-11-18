# KT-177 — vá bốn lớp còn nợ của luật `E_STRICT_TEST_ONLY_WAKE` (reviewer L16, KT-174)

**Ngày**: 2026-11-19 (vòng hai) · **Agent 2 (thợ) L19** · nhánh `night2/L19-wakefix4` ·
worktree `/private/tmp/night2-L19-wakefix4` · HEAD `d68d7eb8` (từ `main`).
**Nguồn đọc trước**: [`kt174-review3.md`](kt174-review3.md) §2.4/§2.6/§2.7/§2.8 + §4,
`v2/tools/check-wake-paths.py`, commit `c5f1ab00` (lead vá H5/H1/H2/H3).
**Ngân sách**: hạn cứng 08:10 JST. Không merge, không push, không `gate --fast`, không tạo
worktree mới, không QEMU.

## 0. Kết luận trước

| Lớp (reviewer L16, KT-174 §2) | Trạng thái | Căn cứ (một dòng) |
|---|---|---|
| **H4** — `include!` **lồng hai lớp** qua tệp test-only | **ĐÃ ĐÓNG** | `resolve_include_targets` thành **fixpoint/worklist**; đột biến thật ⇒ `E_STRICT_TEST_ONLY_WAKE` @ `evil_b.rs:3` |
| **H6** — macro `v2/lib`/cây **che** tên trong `SAFE_VALUE_MACROS` | **ĐÃ ĐÓNG** | `_shadowed_safe_macros`; đột biến `macro_rules! concat` + `let mut wake` ⇒ ĐỎ @ `evil_macro.rs:11` |
| **H7** — thân `macro_rules!` trong `v2/lib` chứa `include!` | **ĐÃ ĐÓNG** | `E_STRICT_MACRO_INCLUDE` (fail-closed) @ `vifast-abi/src/lib.rs:128` |
| **H8** — `#[path = r"…"]` chuỗi thô tới tệp test-only **trong cây** | **ĐÃ ĐÓNG** (hai dạng) | dạng `r"…"` **đã** đóng bởi `c5f1ab00` (kiểm lại: ĐỎ); dạng `r#"…"#` còn hở ⇒ vá bằng `RE_STR_LIT` ⇒ ĐỎ @ `evil_b.rs:3` |
| **H9** (thêm, cùng gốc G1/G2) — `include!["…"]` / `include!{"…"}` | **ĐÃ ĐÓNG** | `RE_INCLUDE_MACRO` nhận cả ba dấu ngoặc; đột biến `include!["evil_b.rs"]` ⇒ ĐỎ @ `evil_b.rs:3` |

Cây sạch: `--strict` **`errors=0`**; `--selftest` **56/56** (46 ca cũ + 10 ca mới);
`./v2/bin/v2 check` **PASS**. **Không** lớp nào phải tuyên bố "chưa đóng trong ngân sách".

## 1. Đã sửa gì (theo thứ tự ưu tiên của đề)

Chỉ **siết**: không nới ngưỡng, không thêm allowlist, không `#[ignore]`/`#[should_panic]`,
không đổi kỳ vọng của ca cũ. Toàn bộ thay đổi nằm trong `v2/tools/check-wake-paths.py`.

### 1.1 H4 — `include!` lồng hai lớp (G4, `:712-790`)

**Gốc lỗi** (L16 §2.4): `resolve_include_targets` dùng `resolved` để **không xử lý lại** một
tệp. Với `core_sched.rs` (sản phẩm) → `evil_a.rs` (test-only) → `evil_b.rs` (test-only),
`evil_a` được xử lý **trước** với `is_test=True` ⇒ `evil_b` nhận `True`; khi `core_sched.rs`
include `evil_a` thì `evil_a` đã `resolved` nên **không** lan `False` xuống `evil_b` ⇒ `evil_b`
ở lại test-only ⇒ lọt.

**Vá**: thay `resolved: set` bằng `status: Dict[Path, bool]` + worklist **fixpoint** (cùng
khuôn `test_only_modules`): trạng thái mỗi tệp chỉ đi `True → False` (đơn điệu), và khi nó
đổi thì tệp được **xử lý lại** để lan tiếp xuống con. Không có vòng lặp vô hạn vì trạng thái
đơn điệu; mỗi tệp được xử lý lại **tối đa một lần**.

### 1.2 H6 — macro che tên "an toàn" (G6, `:906-935` + `:1563-1565` + `:1601-1618`)

**Gốc lỗi** (L16 §2.6): `SAFE_VALUE_MACROS` tin **tên** macro là "chỉ đọc giá trị", nhưng
`macro_rules! concat` trong cây **che** macro chuẩn cùng tên và có thể biến ident thành **tên
hàm** (`$m.$name($t)`). Khi đó `wake` là đối số macro "an toàn" ⇒ nhánh `SAFE_VALUE_MACROS`
bỏ qua ⇒ rơi xuống `_exempt_local_wake` ⇒ biến cục bộ `wake` cùng khối miễn ⇒ **lọt**.

**Vá**: `_shadowed_safe_macros(root, files)` quét **toàn cây** (`v2/kernel` + `v2/sim` +
`v2/lib`) tìm `macro_rules! <tên>` trùng tên an toàn; tên nào bị che thì **không** còn an toàn
(`safe_macros = SAFE_VALUE_MACROS - _shadowed_safe_macros(...)`). Trên cây sạch hiện **không**
có `macro_rules!` nào trùng tên an toàn (chỉ `sctlr_setters`, `bail`) ⇒ không báo oan.

### 1.3 H7 — `include!`/`#[path]` trong thân macro `v2/lib` (G5, `:811-875`)

**Gốc lỗi** (L16 §2.7): `scan_lib_macros` chỉ tìm **token** `wake` trong thân macro; thân
`include!("…")` không có token `wake` ⇒ xanh. `resolve_include_targets` chỉ lặp trên `files` =
`v2/kernel`+`v2/sim` ⇒ `include!` trong `v2/lib` **không** được phân giải; đích nằm trong
`v2/lib` nên cũng không bị quét token.

**Vá**: trong `scan_lib_macros`, mọi `include!`/`#[path]` trong thân `macro_rules!` của
`v2/lib/**` ⇒ `E_STRICT_MACRO_INCLUDE` (fail-closed). Lý do fail-closed thay vì phân giải:
đường dẫn `include!` trong thân macro giải theo **tệp gọi**, không theo tệp định nghĩa (L16 đã
kiểm bằng `rustc` độc lập) ⇒ **không thể** phân giải tĩnh an toàn.

### 1.4 H8 — literal chuỗi Rust **đầy đủ** (G1, `:421-426` + `:678-687`)

**Gốc lỗi** (L16 §2.8): `RE_PATH_ATTR_LINE`/`RE_PATH_ATTR_ANY`/`RE_INCLUDE_MACRO` chỉ khớp
`"…"`; bản `c5f1ab00` thêm `r?` nên `r"…"` **đã** khớp — nhưng dạng **có `#`**
(`r#"evil_b.rs"#`, `br#"…"#`) vẫn **không** khớp ⇒ `_mods_of` bỏ cạnh sản phẩm ⇒ tệp ở lại
test-only; `RE_PATH_ATTR_ANY` cũng không khớp ⇒ không có `E_STRICT_WAKE_INCLUDE`.

**Vá**: một hằng dùng chung `RE_STR_LIT = r'(?:b|c)?r?(?P<hashes>#*)"(?P<path>[^"]*)"(?P=hashes)'`
— nhận `"…"`, `r"…"`, `r#"…"#`, `r##"…"##`, `b"…"`, `br#"…"#`, `c"…"`; backreference
`(?P=hashes)` buộc số `#` đóng **bằng** số `#` mở. Áp cho **cả** `_mods_of` (đồ thị module) và
`resolve_include_targets` (phân giải đích). Đổi sang **nhóm có tên** (`path`) để không phụ
thuộc chỉ số nhóm.

### 1.5 H9 — dấu ngoặc của `include!` (thêm, cùng gốc G1/G2, `:681-686` + `:759`)

**Gốc lỗi** (phát hiện khi dựng đột biến H8): `include!` nhận **cả ba** dấu ngoặc
(`include!("…")`, `include!["…"]`, `include!{"…"}`) — rustc **chấp nhận** cả ba
(`cargo check` RC=0), nhưng `RE_INCLUDE_MACRO` chỉ khớp `(` ⇒ `include!["evil_b.rs"]` lọt
**hoàn toàn** (đã kiểm: `errors=0` trước vá).

**Vá**: `RE_INCLUDE_MACRO` nhận `[\(\{\[]` … `[\)\}\]]`; nhánh fail-closed H3 cũng nhận cả ba
dấu ngoặc. Đây là **siết thêm** ngoài bốn lớp bắt buộc — ghi rõ để reviewer sau kiểm.

## 2. Bằng chứng (nguyên văn)

Mỗi đột biến: dựng **mã biên dịch được** trên cây thật, `cargo check` **RC=0**, `--strict`
**ĐỎ** đúng `file:dòng`, rồi `git checkout --` + xoá tệp mới. Log đầy đủ ở
`/tmp/kt177/logs/` (không commit — log tạm).

### 2.1 Cây sạch

```
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
RC=0
```

```
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 56/56 ok
RC=0
```

```
$ ./v2/bin/v2 check
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
RC=0
```

### 2.2 H4 — `include!` lồng hai lớp ⇒ ĐỎ

Đột biến:
- `v2/kernel/vifast-core/src/evil_b.rs`:
  `pub fn evil_nested(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `v2/kernel/vifast-core/src/evil_a.rs`: `include!("evil_b.rs");`
- `v2/kernel/vifast-core/src/lib.rs`: `#[cfg(test)] mod evil_a;` + `#[cfg(test)] mod evil_b;`
- `v2/kernel/vifast-core/src/core_sched.rs` (cuối tệp): `include!("evil_a.rs");`

```
$ (cd v2/kernel && cargo check -p vifast-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s
cargo_host_rc=0
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:3: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
strict_rc=1
```

Trước vá (cùng đột biến): `# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0`.

### 2.3 H6 — macro che `concat!` + biến cục bộ ⇒ ĐỎ

Đột biến (`v2/kernel/vifast-core/src/evil_macro.rs`, `mod evil_macro;` từ `lib.rs`):

```rust
macro_rules! concat {
    ($m:expr, $name:ident, $t:expr) => {
        $m.$name($t)
    };
}

pub fn evil_macro(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    let mut wake = 0u32;
    wake += 1;
    concat!(m, wake, t)
}
```

```
$ (cd v2/kernel && cargo check -p vifast-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s
cargo_host_rc=0
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_macro.rs:11: `wake` là đối số của macro `concat!` — macro có thể biến ident thành tên hàm (KT-171 B10; tên `concat` bị `macro_rules!` trong cây che — KT-177 H6) trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
strict_rc=1
```

Trước vá: `errors=0`.

### 2.4 H7 — `include!` trong thân macro `v2/lib` ⇒ ĐỎ

Đột biến:
- `v2/lib/vifast-abi/src/lib.rs` (cuối tệp):
  ```rust
  #[macro_export]
  macro_rules! evil_inc {
      () => {
          include!("../../../lib/vifast-abi/src/evil_inc_body.rs")
      };
  }
  ```
- `v2/lib/vifast-abi/src/evil_inc_body.rs`: `m.wake(t)`
- `v2/kernel/vifast-core/src/core_sched.rs` (cuối tệp):
  ```rust
  pub fn evil_inc_probe(m: &mut Machine, t: ObjId) -> Result<(), KError> {
      vifast_abi::evil_inc!();
      Ok(())
  }
  ```

```
$ (cd v2/kernel && cargo check -p vifast-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s
cargo_host_rc=0
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_MACRO_INCLUDE   v2/lib/vifast-abi/src/lib.rs:128: thân `macro_rules! evil_inc` chứa `include!` — đường dẫn giải theo **tệp gọi**, không phân giải tĩnh được (fail-closed, KT-177 H7)
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
strict_rc=1
```

Trước vá: `errors=0`.

### 2.5 H8 — `#[path = r#"…"#]` tới tệp test-only trong cây ⇒ ĐỎ

Đột biến:
- `v2/kernel/vifast-core/src/evil_b.rs`:
  `pub fn evil_raw_path(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> { m.wake(t) }`
- `v2/kernel/vifast-core/src/lib.rs`: `#[cfg(test)] mod evil_b;`
- `v2/kernel/vifast-core/src/core_sched.rs` (cuối tệp):
  `#[path = r#"evil_b.rs"#] mod evil_b_product;`

```
$ (cd v2/kernel && cargo check -p vifast-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.81s
cargo_host_rc=0
$ (cd v2/kernel && cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.81s
cargo_prod_rc=0
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:3: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
strict_rc=1
```

Trước vá: `errors=0` (cả ảnh sản phẩm `kbin` RC=0).

**Đối chứng dạng L16 (`r"…"`, không `#`)**: đã đóng từ `c5f1ab00` — kiểm lại trên cây này:
`#[path = r"evil_b.rs"]` ⇒ `E_STRICT_TEST_ONLY_WAKE … evil_b.rs:3`, `errors=1`.

### 2.6 H9 — `include!["…"]` (thêm) ⇒ ĐỎ

Đột biến: như H8 nhưng `core_sched.rs` có `include!["evil_b.rs"];`.

```
$ (cd v2/kernel && cargo check -p vifast-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s
$ python3 v2/tools/check-wake-paths.py --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:3: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
```

Trước vá: `errors=0` (rustc chấp nhận `include!["…"]`).

### 2.7 Không hồi quy các lớp đã đóng (H1/H2/H3/H5 + H8 dạng L16)

Chạy lại **đúng** đột biến của L16 trên cây sau vá — tất cả vẫn **ĐỎ**:

| Đột biến | Kết quả |
|---|---|
| H1 `include!(r"../../../lib/vifast-abi/src/evil_raw.rs")` | `E_STRICT_WAKE_INCLUDE … core_sched.rs:1005`, `errors=1` |
| H2 `#[path = r"../../../lib/vifast-abi/src/evil_path.rs"]` | `E_STRICT_WAKE_INCLUDE … core_sched.rs:1005`, `errors=1` |
| H3 `include!(concat!(env!("CARGO_MANIFEST_DIR"), …))` | `E_STRICT_WAKE_INCLUDE … core_sched.rs:1005`, `errors=1` |
| H5 `include!("evil_b.rs")` một lớp | `E_STRICT_TEST_ONLY_WAKE … evil_b.rs:1`, `errors=1` |
| H8 `#[path = r"evil_b.rs"]` | `E_STRICT_TEST_ONLY_WAKE … evil_b.rs:1`, `errors=1` |

## 3. Ca `--selftest` mới (10 ca, tổng 56)

Mỗi lớp một ca âm + một đối chứng dương; thêm hai ca H9. Tất cả dựng trên `_fixture_v2`
(cây tối thiểu), không phụ thuộc cây thật.

| # | Ca | Kỳ vọng |
|---|---|---|
| 4ak | `H4 include! lồng hai lớp qua tệp test-only ⇒ đỏ` | `E_STRICT_TEST_ONLY_WAKE` |
| 4al | `H4b include! lồng hai lớp, đích chỉ wake_from( ⇒ xanh` | xanh |
| 4am | `H6 macro_rules! concat che tên an toàn + biến wake ⇒ đỏ` | `E_STRICT_TEST_ONLY_WAKE` |
| 4an | `H6b assert_eq!(wake, 1) không bị che ⇒ xanh` | xanh |
| 4ao | `H7 thân macro_rules! trong v2/lib chứa include! ⇒ đỏ` | `E_STRICT_MACRO_INCLUDE` |
| 4ap | `H7b macro v2/lib dùng concat!, không include! ⇒ xanh` | xanh |
| 4aq | `H8 #[path = r#"…"#] tới tệp test-only trong cây ⇒ đỏ` | `E_STRICT_TEST_ONLY_WAKE` |
| 4ar | `H8b #[path = r#"…"#] tới tệp sản phẩm chỉ wake_from( ⇒ xanh` | xanh |
| 4as | `H9 include![…] (ngoặc vuông) tới tệp test-only ⇒ đỏ` | `E_STRICT_TEST_ONLY_WAKE` |
| 4as | `H9 include!{…} (ngoặc nhọn) tới tệp test-only ⇒ đỏ` | `E_STRICT_TEST_ONLY_WAKE` |

**Chứng minh ca âm là thật** (không phải "luôn đỏ"): dựng lại từng cây nhưng **bỏ** đúng cạnh
đột biến ⇒ **xanh**:

```
H8 không cạnh #[path] ⇒ ok = True | errors: []
H4 không cạnh include sản phẩm ⇒ ok = True | errors: []
H6 không macro che ⇒ ok = True | errors: []
```

46 ca cũ **vẫn xanh** (không sửa kỳ vọng, không bỏ ca): `# selftest: 56/56 ok`.

## 4. Còn hở / chưa review (ghi rõ, **không** tuyên bố đã đóng)

1. **`include!` trong thân macro ở `v2/kernel`/`v2/sim`** (không phải `v2/lib`): H7 chỉ vá
   `v2/lib`. Với macro ở `v2/kernel`, `resolve_include_targets` **có** thấy `include!` trong
   thân macro và phân giải theo **tệp định nghĩa** — nếu đường dẫn thật giải theo **tệp gọi**
   thì có thể trỏ sai chỗ. Chưa dựng đột biến cho đường này (ngoài bốn lớp của đề). Đề xuất
   vòng sau: fail-closed `include!`/`#[path]` trong **mọi** thân `macro_rules!` của cây quét.
2. **`macro_rules!` che tên an toàn định nghĩa ở crate ngoài cây** (proc-macro/dep ngoài
   allowlist D2-21): `_shadowed_safe_macros` chỉ quét `v2/kernel`+`v2/sim`+`v2/lib`. Nếu sau
   này thêm crate ngoài có `macro_rules! concat`, đường H6 mở lại. Hiện allowlist D2-21 không
   có crate nào như vậy.
3. **`#[cfg_attr(…, path = "…")]`** — chưa thử; `_mods_of` chỉ đọc `#[path]` trực tiếp.
4. **`include!` với đường dẫn chứa escape** (`"a\\b.rs"`): `RE_STR_LIT` lấy nguyên văn, không
   giải escape — đường dẫn lạ sẽ thành "không tồn tại" ⇒ **ĐỎ** (fail-closed), không lọt.
5. **Giả định build** "`#[cfg(feature = "ktest")]` ⇒ test-only" (L16 §5 mục 4) **không** thuộc
   phạm vi KT-177; vẫn đúng với ba đường build hiện tại.
6. **Chưa chạy** `gate --fast` (luật task). `./v2/bin/v2 check` **PASS** trên cây sạch.

## 5. Luật đã giữ

- Không merge, không push, không sửa `main`, không `gate --fast`, không tạo worktree mới.
- Không sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`.
- Không nới ngưỡng/checker (chỉ siết); không `#[ignore]`/`#[should_panic]`; không retry để lấy
  xanh; không bỏ ca selftest nào.
- Không `pkill -f qemu-system`; task **không** dùng QEMU.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không
  `local a=1 b="$a"`.
- Mọi đột biến đã `git checkout --` + xoá tệp mới; cây sạch trước commit (chỉ
  `v2/tools/check-wake-paths.py` + journal này).
