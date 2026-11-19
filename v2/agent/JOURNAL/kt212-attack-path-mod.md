# KT-212 — tấn công `#[path]`/`mod`: đường lọt thứ mười một

**Vai**: L47 (reviewer tấn công) → chuyển **tác giả** KT-213 cho bản vá.
**Nhánh**: `night3/L47-kt212`, gốc `41e568bc`.
**Worktree**: `/private/tmp/night3-L47-kt212` (KHÔNG chạm checkout chính).
**Thời điểm**: 2026-11-20.

## 0. Kết luận trước

| Câu hỏi | Trả lời |
|---|---|
| Luật `#[path]`/`mod` đủ chưa? | **CHƯA — CÒN LỌT** (trước KT-213) |
| Số đường lọt SỐNG tìm được | **2** (#11 `#[path]` gốc non-mod-rs; #12 thư mục tên `target`) |
| Đường lọt #11 | **ĐÃ VÁ** trong KT-213 (cùng nhánh) |
| Đường lọt #12 | **NỢ CÓ TÊN** — khác lớp gốc (bộ lọc đường dẫn, không phải gốc `#[path]`); xem §5 |
| Báo oan | **0** trên cây thật (sau khi vá; xem §4) |
| Đối chứng dương | **4/4 XANH** (§3) |

## 1. Đường lọt #11 — gốc phân giải `#[path]` của tệp **non-mod-rs**

### 1.1 Gốc lỗi

`resolve_include_targets()` gọi:

```python
_walk_items(tree, relp, p.parent, on_attr=on_attr, …)
```

`p.parent` là **thư mục tệp** — đúng cho `#[path]` **mức mục**, nhưng **sai** cho `#[path]` nằm
**trong inline module** của tệp **non-mod-rs**. Rust Reference (Items > Modules > The path
attribute): với tệp non-mod-rs `src/foo.rs`, gốc là `src/foo/<inline>/…`; checker dùng
`src/<inline>/…` — **lệch đúng một tầng `foo/`**.

`_mods_of()` **đã** nhận `mod_dir` đúng từ KT-171, nhưng `_mods_of` chỉ dùng cho
`test_only_modules` (đồ thị test-only), **không** đổ vào `extra` — nên tệp thật không bao giờ
được quét.

### 1.2 Repro (đo được, `cargo check` RC=0)

```rust
// v2/kernel/vifast-core/src/foo.rs   (non-mod-rs; `mod foo;` trong lib.rs)
mod inner {
    #[path = "../../../evil.rs"]
    mod baz;
}
```

```rust
// v2/kernel/vifast-core/evil.rs   ← rustc biên dịch TỆP NÀY (ngoài `src/` ⇒ ngoài phạm vi luật wake)
use crate::{KError, Machine, ObjId};
pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.wake(t)
}
```

```rust
// v2/kernel/evil.rs   ← checker phân giải TỆP NÀY (decoy sạch; trong `v2/kernel` nên không báo "ra ngoài cây")
pub fn decoy() -> u32 { 7 }
```

```text
v2/kernel/vifast-core/src/foo/inner/notes.txt   ← tệp KHÔNG phải .rs, chỉ để thư mục
                                                   `src/foo/inner/` tồn tại (OS cần nó để `..`
                                                   phân giải; git theo tệp, không theo thư mục)
```

Vì sao cần `notes.txt`: rustc mở `vifast-core/src/foo/inner/../../../evil.rs`; `..` chỉ phân giải
được nếu `src/foo/inner/` **tồn tại thật**. Đây là chi tiết tấn công, không phải chi tiết checker.

### 1.3 Đo trước khi vá

| Lệnh | Kết quả |
|---|---|
| `cargo check -p vifast-core` (trong `v2/kernel`) | **RC=0**; warning trỏ `vifast-core/src/foo/inner/../../../evil.rs` ⇒ rustc nạp tệp evil |
| `python3 v2/tools/check-wake-paths.py --root .` | **errors=0, RC=0** (XANH) |
| `… --strict` | **errors=0, RC=0** (XANH) |
| **Đối chứng âm**: chép nguyên `evil.rs` thành `v2/kernel/vifast-core/src/evil_ctl.rs` + `mod evil_ctl;` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE v2/kernel/vifast-core/src/evil_ctl.rs:8: gọi wake(`, RC=1 |

Cùng nội dung, chỉ khác **vị trí** ⇒ nội dung là vi phạm thật; đường `#[path]` là thứ duy nhất
che nó. Log: `kt212-logs/repro1-*.log`, `kt212-logs/repro1-source.txt`.

### 1.4 Vì sao mười bản vá trước không bắt

KT-171/177/184/194/195/199/209 đều chỉ vá **đích** của `#[path]` (ra ngoài cây, không tồn tại,
thư mục, không literal, `cfg_attr` lồng, raw string) — **chưa bản nào kiểm gốc** mà `#[path]`
được giải. `_mods_of` có `mod_dir` đúng từ KT-171 nhưng đường phân giải-**và-quét** thì không
dùng.

## 2. Ma trận tấn công (28 biến thể, cây thật, sau KT-213)

Mỗi biến thể: dựng mã trong worktree → `cargo check -p vifast-core` → checker **cả hai** chế độ.
Log đầy đủ: `kt212-logs/matrix/<tên>.{cargo,normal,strict}.log` + `matrix.json`.

### A. Tấn công `#[path]`

| # | Dạng | cargo | thường | strict | Kết luận |
|---|---|---|---|---|---|
| A1 | `#[path]` trong inline module tệp non-mod-rs → ngoài `src/` | 0 | **ĐỎ** | **ĐỎ** | repro #11 — **đã vá** |
| A2 | `#[path]` trên `mod` trong thân `fn` (mức mục) | 0 | ĐỎ | ĐỎ | gốc = thư mục tệp, đúng |
| A3 | `mod` trong `impl` | 101 | ĐỎ | ĐỎ | Rust **không cho** ⇒ không phải đường lọt |
| A4 | `mod` trong `trait` | 101 | ĐỎ | ĐỎ | Rust **không cho** ⇒ không phải đường lọt |
| A5 | `#[path]` trong thân `macro_rules!` | 0 | ĐỎ | ĐỎ | `E_STRICT_MACRO_INCLUDE` + `E_STRICT_MACRO_MOD` |
| A6 | raw string `r#"../../../evil.rs"#` | 0 | ĐỎ | ĐỎ | giải mã đúng |
| A7 | escape `..\x2f..\x2f..\x2fevil.rs` | 0 | ĐỎ | ĐỎ | giải mã đúng |
| A9 | `#[path]` **hai lần** trên cùng `mod` | 0 | ĐỎ | ĐỎ | checker thu **cả hai** cạnh ⇒ quét cả hai đích |
| A10 | `#[path]` trỏ **thư mục**, `mod` không có thân | 101 | ĐỎ | ĐỎ | `E_STRICT_WAKE_INCLUDE` (KT-199 F4c) |
| A11 | `#[path]` trỏ tệp **không tồn tại** | 101 | ĐỎ | ĐỎ | `E_STRICT_WAKE_INCLUDE` |
| A13 | `cfg_attr` lồng **ba** tầng | 0 | ĐỎ | ĐỎ | đệ quy có giới hạn |
| A14 | `cfg_attr` **nhiều** `path` cùng nhánh | 0 | ĐỎ | ĐỎ | thu cả hai cạnh |
| A15 | `#[path]` trong lời gọi macro kiểu `cfg_if!` | 101 | ĐỎ | ĐỎ | `E_STRICT_MACRO_TT` + `E_STRICT_MACRO_INCLUDE` + `E_STRICT_MACRO_MOD` |
| A16 | tên module **trùng** tên thư mục | 0 | ĐỎ | ĐỎ | `#[path]` thắng, đúng |
| A17 | `#[path]` tới tệp trong cây, tệp đó `include!` **ra ngoài** | 101 | ĐỎ | ĐỎ | worklist phân giải tiếp |
| A18 | chuỗi `#[path]` **hai lớp** | 101 | ĐỎ | ĐỎ | worklist phân giải tiếp |

Ghi chú: A3/A4/A10/A11/A15/A17/A18 có `cargo=101` vì bản thân mã **không biên dịch được**
(Rust không cho `mod` trong `impl`/`trait`; `#[path]` sai đích; macro `:item` không nhận
`#[path]`; `include!` ra ngoài). Chúng vẫn **ĐỎ** ở checker — không phải đường lọt.

### B. Tấn công `mod`

| # | Dạng | cargo | thường | strict | Kết luận |
|---|---|---|---|---|---|
| B1 | `mod baz;` trong thân `fn` | 101 | XANH | ĐỎ | Rust **không cho** file module trong block nếu không có `#[path]` ⇒ không phải đường lọt; strict đỏ vì tệp `src/foo/baz.rs` nằm trong phạm vi |
| B2 | `mod` trong **lời gọi macro** (`cfg_if!`) | 0 | ĐỎ | ĐỎ | `E_STRICT_MACRO_MOD` |
| B3 | `mod r#baz;` (raw ident) | 0 | XANH | ĐỎ | **F2 đã biết**: chế độ thường chỉ quét `extra`; `v2 check` chạy `--strict` nên cổng vẫn bắt |
| B5a | `#[cfg(test)] fn h() {} mod baz;` **cùng dòng** | 0 | XANH | ĐỎ | như B3 — `mod` là **sản phẩm** (KT-196/F2), strict bắt |
| B5b | `#[cfg(test)]` **dòng riêng** trên `mod baz;` | 0 | XANH | XANH | **đúng**: test-only thật, không biên dịch trong sản phẩm |
| B6 | `#[path]` kéo tệp **test-only** vào sản phẩm | 0 | ĐỎ | ĐỎ | "sản phẩm thắng" (KT-174/H5) |
| B7 | `mod baz;` trong **block expression** | 101 | XANH | ĐỎ | như B1 — Rust không cho |
| **B8** | `mod target;` → `src/target/mod.rs` → `src/target/evil.rs` | **0** | **XANH** | **XANH** | **ĐƯỜNG LỌT #12** — xem §5 |

### C. Đối chứng dương (bắt buộc XANH)

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| C1 | `#[path]` trong inline module tệp non-mod-rs → tệp **thật** trong cây | 0 | **XANH** | **XANH** |
| C2 | `mod` bình thường (`src/foo/bar.rs`) | 0 | **XANH** | **XANH** |
| C3 | `#[cfg(test)] mod tests { #[test] fn t() {} }` | 0 | **XANH** | **XANH** |
| C4 | `#[path = "sub"] mod inner { #[path = "ok.rs"] mod baz; }` | 0 | **XANH** | **XANH** |

⇒ **Không báo oan** trên bốn dạng hợp lệ chính. Cây thật (sau vá): `errors=0` cả hai chế độ.

## 3. Đo bằng rustc (không suy đoán)

Mọi kết luận về gốc phân giải đều **đo** bằng `rustc 1.100.0`, không đọc tài liệu suông:

| Thí nghiệm | Kết quả |
|---|---|
| `src/foo.rs` + `mod inner { #[path="../../../evil.rs"] mod baz; }` | rustc mở `src/foo/inner/../../../evil.rs` ⇒ gốc = `src/foo/inner/` |
| `src/foo.rs` + `#[path="sub"] mod inner { #[path="a.rs"] mod m; }` | rustc mở `src/sub/a.rs` (không phải `src/foo/sub/a.rs`) ⇒ `#[path]` trên inline module **thay** tiền tố `<stem>/` |
| `#[path="sub/ktest_vfs.rs"] mod ktest_vfs;` + `mod ktest_vfs_once;` | rustc tìm `src/sub/ktest_vfs_once.rs` (cạnh tệp) ⇒ tệp nạp bằng `#[path]` là **mod-rs-like** |
| `src/ktest_vfs.rs` (nạp bằng `#[path]`) + `mod rig;` + `rig.rs` + `mod inner { #[path="a.rs"] }` | rustc tìm `src/ktest_vfs_rig/inner/a.rs` ⇒ con của tệp path-loaded là **non-mod-rs** bình thường |
| `include!("sub/inc.rs")` + `mod foo;` trong `inc.rs` | rustc tìm `src/sub/foo.rs` ⇒ `include!` giải theo **thư mục tệp include** |
| `include!("sub/inc.rs")` + `mod inner { #[path="ok.rs"] }` trong `inc.rs` | rustc tìm `src/sub/inner/ok.rs` ⇒ như trên |

## 4. Báo oan

**0** trên cây thật. Trong quá trình vá có **hai** lần báo oan tạm thời, đã sửa:

1. Bản vá đầu dùng `_default_mod_dir = p.parent/p.stem` cho **mọi** tệp ngoài đồ thị module ⇒
   ĐỎ oan 5 tệp `v2/kernel/vifast-kbin-riscv64/init/*.rs` (crate root do `build.rs` biên dịch,
   `#[path = "ring_common.rs"]` giải theo `init/`, không phải `init/ring_server_user/`). Sửa:
   tệp ngoài đồ thị ⇒ **thư mục của chính tệp** (chúng là crate root).
2. Bản vá đầu truyền `mod_dir` làm `cur_dir` cho `_mods_of` ⇒ `#[path]` **mức mục** trong
   `src/foo.rs` giải thành `src/foo/a.rs` thay vì `src/a.rs`. Sửa: `_walk_items` nhận **hai**
   gốc (`cur_dir` cho `#[path]` mức hiện tại, `mod_dir` cho inline module + `mod NAME;`).

## 5. Đường lọt #12 — thư mục tên `target` (NỢ CÓ TÊN, khác lớp gốc)

### 5.1 Repro

```rust
// v2/kernel/vifast-core/src/lib.rs
mod target;
```

```rust
// v2/kernel/vifast-core/src/target/mod.rs
mod evil;
```

```rust
// v2/kernel/vifast-core/src/target/evil.rs
use crate::{KError, Machine, ObjId};
pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.wake(t)
}
```

### 5.2 Đo

| Lệnh | Kết quả |
|---|---|
| `cargo check -p vifast-core` | **RC=0** (rustc biên dịch `src/target/evil.rs`) |
| checker thường | **errors=0, RC=0** |
| checker `--strict` | **errors=0, RC=0** |

Log: `kt212-logs/matrix/B8-target-dir.*.log`.

### 5.3 Gốc lỗi

**Năm** chỗ bỏ qua tệp có `"target" in p.parts`:

- `iter_rs()` (nguồn `files` của mọi luật),
- `_protected_files()` (`scan_lexical`),
- `scan_structure()`,
- `scan_macro_bodies()`,
- `_shadowed_safe_macros()`.

Bộ lọc này nhằm bỏ thư mục build của cargo (`v2/kernel/target/`), nhưng khớp **bất kỳ** thành
phần `target` nào — kể cả `src/target/`. Tệp trong `src/target/` **không** vào `files`, **không**
vào `_protected_files`, **không** vào `scan_structure`; `resolve_include_targets` cũng không seed
nó (seed từ `files`), nên **không** vào `extra`. `_mods_of` (đồ thị test-only) **có** đọc nó
(parse trực tiếp) nhưng chỉ để tính test-only — không kéo vào tập bị quét.

### 5.4 Đề xuất vá (KT-214)

Thay `"target" in p.parts` bằng kiểm **vị trí**: chỉ bỏ qua khi `target` xuất hiện **trước**
`src` trong đường dẫn tương đối (thư mục build thật), không bỏ qua `src/**/target/**`:

```python
def _in_build_dir(root: Path, p: Path) -> bool:
    try:
        parts = p.relative_to(root).parts
    except ValueError:
        parts = p.parts
    if "target" not in parts:
        return False
    ti = parts.index("target")
    return ("src" not in parts) or (ti < parts.index("src"))
```

Thay ở **cả năm** call site. Thêm ca selftest: `src/target/evil.rs` chứa `wake` ⇒ **ĐỎ** cả hai
chế độ; đối chứng dương: `v2/kernel/target/…` (build dir) vẫn bị bỏ qua.

**Vì sao chưa vá trong KT-213**: khác lớp gốc (bộ lọc đường dẫn, không phải gốc `#[path]`), và
lead chỉ đạo "ghi thành nợ có tên (nếu khác lớp)". Đây là **đường lọt SỐNG** — cần task riêng
ngay sau KT-213.

## 6. Khuyến nghị: `#[path]`/`mod` có cần quét token toàn cục như `include!`?

**Không cần** — và **không đủ**. Lý do định lượng:

1. **`include!` khác bản chất**: `include!` là **macro**, đích giải theo **tệp gọi** bất kể vị
   trí token; nó có thể nằm trong đối số macro, trong `( … )`/`[…]`, trong biểu thức — nên phải
   quét token toàn cục (KT-211). `#[path]`/`mod` là **mục Rust**: rustc chỉ công nhận chúng ở
   **vị trí mục** (hoặc trong block, với `#[path]`). Quét token toàn cục sẽ **báo oan** mọi
   `mod` trong chuỗi/comment/đối số macro (đã có luật riêng fail-closed cho macro).
2. **Đường lọt #11 không phải "bỏ sót vị trí token"** mà là **sai gốc phân giải**. Quét token
   toàn cục vẫn phải chọn một gốc — nếu gốc sai thì vẫn lọt. Bản vá đúng là **hai gốc**
   (`cur_dir`/`mod_dir`), không phải đổi cách quét.
3. **Đo được**: sau KT-213, **16/16** biến thể `#[path]` biên dịch được trong ma trận đều **ĐỎ**
   cả hai chế độ; **4/4** đối chứng dương **XANH**; cây thật `errors=0`. Không còn khoảng trống
   nào cần quét token toàn cục cho `#[path]`/`mod`.
4. **Cái còn hở là bộ lọc đường dẫn** (#12), không phải vị trí token. Vá #12 bằng vị trí
   `target/`, không bằng quét token.

## 7. Tệp/thay đổi

- `v2/tools/check-wake-paths.py` — bản vá KT-213 (§`kt213-path-base.md`).
- `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` — §2 (dạng hỗ trợ) + §4 (điểm đã từng sai).
- `v2/agent/JOURNAL/kt212-logs/` — log repro #11 (trước vá) + ma trận 28 biến thể.
- `v2/agent/JOURNAL/kt213-logs/` — log sau vá.
