# KT-187 — review đối kháng KT-184 (D2-160 + D2-161)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer độc lập L25** · nhánh `night2/L25-review184` ·
worktree `/private/tmp/night2-L25-review184` · HEAD `9921f606` (từ `main` @ `9921f606`).
**Nguồn đọc**: [`kt184-h4rule.md`](kt184-h4rule.md), `git show 7723b46c --stat`,
`git show 9921f606`, `v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 12:00 JST. Không merge/push/`gate --fast`/QEMU/worktree mới.
**Vai**: reviewer độc lập theo **D2-159** — mọi đột biến dựng lại trên **bản sao cây**, không
dùng script của tác giả; không sửa mã sản phẩm trên `main`.

## 0. Phán quyết

| # | Yêu cầu | Kết quả | Căn cứ |
|---|---|---|---|
| 1 | H4 **hai lớp** ⇒ ĐỎ; checker `7e7e1b53` cùng cây ⇒ `errors=0` | **ĐẠT** | §1, [`kt187-logs/h4.log`](kt187-logs/h4.log) |
| 1b | H4 **ba lớp** ⇒ ĐỎ; `7e7e1b53` ⇒ `errors=0` | **ĐẠT** | §1, cùng log |
| 2 | D2-161: `include!` không literal, `#[path]` không literal, `#[path]` ra ngoài cây — **ĐỎ ở chế độ thường** | **ĐẠT** | §2, [`kt187-logs/d2161.log`](kt187-logs/d2161.log) |
| 2b | D2-161 áp cả **tệp test-only** (9921f606) | **ĐẠT** | §2.2, cùng log |
| 3 | Selftest 64 ca: 8 ca mới có thật, không tautology | **ĐẠT** (1 ca là khoá hành vi, không phải regression) | §3, [`kt187-logs/teeth.log`](kt187-logs/teeth.log) |
| 4 | Báo oan: `#[cfg(test)]`, tệp không tồn tại, `..`, symlink | **ĐẠT CÓ ĐIỀU KIỆN** — 1 báo oan tiềm ẩn (`build.rs`) | §4, [`kt187-logs/bypass.log`](kt187-logs/bypass.log) |
| 5 | Đường vòng mới | **KHÔNG ĐẠT — 3 đường lọt thật** | §5, [`kt187-logs/compile-cfgattr.log`](kt187-logs/compile-cfgattr.log), [`kt187-logs/compile-macro-path.log`](kt187-logs/compile-macro-path.log), [`kt187-logs/compile-macro-include.log`](kt187-logs/compile-macro-include.log) |
| 6 | H6/H7 repro: ĐỎ hiện tại, XANH `7e7e1b53` | **ĐẠT** (lý do H7 đúng — §6.1) | §6, [`kt187-logs/h6h7.log`](kt187-logs/h6h7.log) |

### Phán quyết: **KHÔNG ĐẠT**

Hai yêu cầu cốt lõi của KT-184 (H4 hai/ba lớp; D2-161 ba đột biến ở chế độ thường) **đều
đóng đúng** — dựng lại độc lập, không dùng script tác giả. Nhưng **luật D2-161 chưa đóng**:
còn **ba** đường để **mã sản phẩm biên dịch `wake`** trong khi checker **xanh ở cả chế độ
thường lẫn `--strict`**, và cả ba đều **qua `cargo fmt --all --check`** (đã đo `fmt rc=0`):

1. **B1** — `#[cfg_attr(not(test), path = "…")]` (§5.1): checker **không thấy** `#[path]`
   nào; tệp đích **ngoài** `v2/kernel`+`v2/sim` không bị quét. `cargo check -p vifast-core`
   **RC=0**, cảnh báo chỉ đúng `v2/lib/vifast-abi/src/evil_out.rs:4` (nơi có `self.wake(t)`).
2. **B2** — `#[path]` trong thân `macro_rules!` (§5.2): rustc giải theo **tệp gọi**, checker
   giải theo **tệp định nghĩa**; đích thật (test-only, có `wake`) không bị quét.
   `cargo check -p vifast-core` **RC=0**, cảnh báo chỉ `vifast-core/src/sub/evil.rs:4`.
3. **B3** — `include!` trong thân `macro_rules!` (§5.2): **cùng gốc** với B2 — rustc giải
   theo **tệp gọi**, checker theo **tệp định nghĩa**. `cargo check -p vifast-core` **RC=0**,
   cảnh báo chỉ `vifast-core/src/sub/evil.rs:4`.

Ngoài ra **ba** dạng viết cách khác (`include ! (…)`, `#[ path = … ]`, `#[\npath = …]`) cũng
**lọt checker** (§5.3) — nhưng `cargo fmt --check` chuẩn hoá chúng, nên chỉ là lỗ của
**checker**, không phải lỗ của **cổng đầy đủ**. B1/B2/B3 thì **không** bị fmt bắt.

---

## 1. H4 hai lớp + ba lớp — dựng lại độc lập

Cây: bản sao `v2/kernel` + `v2/sim` + `v2/lib` trong `$TMPDIR/kt187-l25/h4/…`; script
[`kt187-logs/t1-h4.sh`](kt187-logs/t1-h4.sh) (không dùng `repro-h4.sh` của tác giả).

**Hai lớp** — `core_sched.rs` (sản phẩm) `include!("evil_a.rs")` → `evil_a.rs` (test-only,
`#[cfg(test)] mod evil_a;` trong `lib.rs`) `include!("evil_b.rs")` → `evil_b.rs` (test-only)
`self.wake(t)`:

```
### two layers / current --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:3: gọi `wake(` trong mã sản phẩm …
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1
### two layers / 7e7e1b53 --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

**Ba lớp** — thêm `evil_c.rs` (test-only, `self.wake(t)`), `evil_b.rs` chỉ còn
`include!("evil_c.rs")`:

```
### three layers / current --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_c.rs:3: gọi `wake(` trong mã sản phẩm …
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1
### three layers / 7e7e1b53 --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

⇒ **ĐẠT**. Fixpoint/worklist của KT-177 thật sự lan "sản phẩm thắng" qua **cả hai** lớp;
bản `7e7e1b53` (KT-174, trước fixpoint) **xanh** trên **cùng cây** — lỗ hổng là thật, bản vá
là thật. Đối chứng: cây sạch `--strict` `errors=0`.

**Lưu ý**: H4 chỉ **ĐỎ ở `--strict`**; chế độ thường vẫn `errors=0` (luật `wake` là luật
`--strict`). Đó là thiết kế hiện tại, không phải lỗi của KT-184 — nhưng nếu D2-160 muốn H4
đỏ ở **mọi** chế độ thì cần một dòng quyết định nữa (xem §7.4).

## 2. D2-161 — mọi chế độ, cả tệp sản phẩm lẫn test-only

Script [`kt187-logs/t2-d2161.sh`](kt187-logs/t2-d2161.sh). Sáu cây đột biến: ba dạng ×
{sản phẩm, test-only}. Mỗi dạng thêm vào **cuối** `core_sched.rs` (sản phẩm) hoặc
`evil_t.rs` (test-only, `#[cfg(test)] mod evil_t;` trong `lib.rs`).

### 2.1 Tệp **sản phẩm** — chế độ thường

```
### prod-inc / current normal
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1004: `include!` không phải literal chuỗi …
# check-wake-paths: mode=v2 strict=0 errors=1 warnings=0
rc=1
### prod-path-nonlit / current normal
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1004: `#[path]` không phải literal chuỗi …
rc=1
### prod-path-out / current normal
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1004: `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` …
rc=1
```

Đối chứng **trước KT-184** (`b6efc403`, base worktree của tác giả): **cả ba** `errors=0`
(`rc=0`) ở chế độ thường — đúng như tác giả khai. Bản `9921f606^` (= `7723b46c`, đã có
D2-161) cũng đỏ — nhất quán.

**Lưu ý về B3**: `#[path = concat!(…)]` **không biên dịch được** — rustc báo
`malformed path attribute input` (đã kiểm, [`kt187-logs/rustc-semantics.log`](kt187-logs/rustc-semantics.log)).
Vì vậy luật `#[path]` không-literal là **fail-closed phòng thủ**, không phải đường lọt của
mã chạy được; đường lọt **thật** của D2-161 là `include!(concat!(…))` (Rust hợp lệ) và
`#[path]` trỏ ra ngoài cây (Rust hợp lệ). Giữ luật là đúng, nhưng journal KT-184 §2.3 gọi
B3 là "lỗ hổng thật" là **quá lời** — nó là mã sai cú pháp, không phải mã sản phẩm chạy được.

### 2.2 Tệp **test-only** — chế độ thường (thay đổi của `9921f606`)

```
### test-inc / current normal
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/evil_t.rs:1: `include!` không phải literal chuỗi …
rc=1
### test-path-nonlit / current normal
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/evil_t.rs:1: `#[path]` không phải literal chuỗi …
rc=1
### test-path-out / current normal
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/evil_t.rs:1: `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` …
rc=1
```

Đối chứng `9921f606^` (còn `if not is_test:`): `test-inc` và `test-path-nonlit` **xanh**
(`errors=0`) — đúng là lỗ mà `9921f606` đóng; `test-path-out` **đỏ** ngay cả trước
`9921f606` (nhánh `_inside_scan_tree` không phụ thuộc `is_test`) — khớp §5.3 của tác giả.

⇒ **ĐẠT**. Ba đột biến **ĐỎ ở chế độ thường** trên cả hai loại tệp; `b6efc403` xanh trên
cùng cây. `if True:` (dòng 776) là mùi mã nhưng hành vi đúng; `is_test` không còn dùng
trong khối đó — không phải lỗi.

## 3. Selftest 64 ca — "răng" của 8 ca mới

Script [`kt187-logs/t4-teeth.py`](kt187-logs/t4-teeth.py): dựng lại **độc lập** 8 fixture
của 8 ca mới (không chạy `--selftest` của tác giả), rồi (a) chạy checker hiện tại, (b) chạy
`7e7e1b53` trên cùng fixture, (c) bỏ đúng cạnh đột biến ⇒ phải xanh, (d) thêm lại ⇒ phải đỏ.

| Ca | Đột biến | Hiện tại | `7e7e1b53` | Bỏ đột biến | Kết luận |
|---|---|---|---|---|---|
| 4at H4 ba lớp | `wake` trong `evil_c` | ĐỎ | **XANH** | xanh (4au) | **regression thật** |
| 4au H4b ba lớp | `wake_from` | XANH | — | thêm `wake` ⇒ ĐỎ | đối chứng dương |
| 4av `#[path]`+`include!` | `wake` trong `evil_b` | ĐỎ | **ĐỎ** | xanh (4aw) | **khoá hành vi**, không phải regression |
| 4aw `#[path]`+`include!` | `wake_from` | XANH | — | thêm `wake` ⇒ ĐỎ | đối chứng dương |
| 4ax `include!(concat!)` | không literal | ĐỎ (`E_STRICT_WAKE_INCLUDE`) | **XANH** | literal ⇒ xanh | **regression thật** |
| 4ay `#[path]` ra ngoài | ngoài cây | ĐỎ (`E_STRICT_WAKE_INCLUDE`) | **XANH** | trong cây ⇒ xanh | **regression thật** |
| 4az `#[path = concat!]` | không literal | ĐỎ (`E_STRICT_WAKE_INCLUDE`) | **XANH** | literal ⇒ xanh | **regression thật** |
| 4ba `#[path]` literal | trong cây | XANH | — | ra ngoài ⇒ ĐỎ | đối chứng dương |

`teeth: 17/18 pass` — dòng "FAIL" duy nhất là **4av**: nó **đã đỏ trên `7e7e1b53`**, nên
**không** phải ca bắt hồi quy của KT-184; tác giả đã ghi đúng là "khoá hành vi" (§2.5 của
journal KT-184). Không có ca nào là tautology thuần: 4ba là đối chứng dương yếu (nếu checker
hỏng hoàn toàn — không bao giờ báo lỗi — nó vẫn xanh), nhưng đi cặp với 4ay/4az nên có nghĩa.

**Không** có ca nào `#[ignore]`/`#[should_panic]`; runner (`selftest`, dòng 1864) đếm đủ
`len(cases)` và in `not ok` — không nuốt lỗi.

## 4. Báo oan

| Tình huống | Kết quả | Đánh giá |
|---|---|---|
| `include!` trong `#[cfg(test)]` của tệp sản phẩm, đích **trong** cây | **XANH** | đúng — không báo oan |
| `#[path]` tới tệp **không tồn tại** | **ĐỎ** (`E_STRICT_WAKE_INCLUDE`) | đúng — Rust cũng lỗi biên dịch |
| `#[path = "../src/evil_ok.rs"]` (có `..`, trong cây) | **XANH** | đúng |
| symlink trong cây → trong cây | **XANH** | đúng (`resolve()` về đích thật) |
| symlink trong cây → **ngoài** cây | **ĐỎ** | đúng (fail-closed) |
| `include!` trong `#[cfg(test)]` trỏ **ra ngoài** cây | **ĐỎ** | **theo thiết kế D2-161** ("mọi … trỏ trong cây"), nhưng là **báo oan tiềm ẩn** cho helper test ngoài cây |
| `include!(concat!(env!("OUT_DIR"), …))` trong **`build.rs`** | **ĐỎ** | **báo oan tiềm ẩn** — mẫu build script hợp lệ; `iter_rs` (dòng 1202) quét cả `build.rs` dưới `v2/kernel` |

Hai dòng cuối **không** làm đỏ cây hiện tại (cây không dùng `include!`/`#[path]` ngoài cây
và `build.rs` không có `include!`), nhưng là **nợ** cần quyết định: hoặc miễn `build.rs`,
hoặc ghi rõ trong D2-161 là build script cũng phải literal.

## 5. Đường vòng mới — **ba đường lọt thật**

Script [`kt187-logs/t3-bypass.sh`](kt187-logs/t3-bypass.sh) (checker) +
[`kt187-logs/t7-compile.sh`](kt187-logs/t7-compile.sh) (biên dịch).

### 5.1 **B1 — `#[cfg_attr(not(test), path = "…")]`** (nặng nhất)

**Cơ chế**: `RE_PATH_ATTR_ANY` (dòng 700) và vòng quét không-literal (dòng 789) đều đòi
`#\[path\s*=` — **không** khớp `#[cfg_attr(not(test), path = "…")]`. `_mods_of` (dòng 576)
cũng chỉ đọc `RE_PATH_ATTR_LINE` (dòng 439). Vì vậy `#[path]` nằm trong `cfg_attr` **hoàn
toàn vô hình** với checker. Rust mở `cfg_attr` thành `#[path]` khi `not(test)` ⇒ **bản sản
phẩm** nạp tệp đích.

**Dựng lại** (trong `v2/kernel/vifast-core/src/lib.rs`):

```rust
#[cfg(test)]
mod evil;
#[cfg_attr(not(test), path = "../../../lib/vifast-abi/src/evil_out.rs")]
mod evil2;
```

`v2/lib/vifast-abi/src/evil_out.rs` chứa `impl Machine { fn duong_san_pham(&mut self, t: ObjId)
{ let _ = self.wake(t); } }`.

**Kết quả** ([`kt187-logs/compile-cfgattr.log`](kt187-logs/compile-cfgattr.log)):

```
### B1 checker hiện tại (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B1 checker hiện tại --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
### B1 cargo fmt --all --check
fmt rc=0
### B1 cargo check -p vifast-core
warning: method `duong_san_pham` is never used
 --> vifast-core/src/../../../lib/vifast-abi/src/evil_out.rs:4:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.27s
rc=0
```

⇒ **mã sản phẩm biên dịch `wake`**, checker **xanh cả hai chế độ**, `cargo fmt --check`
**không** sửa `cfg_attr` (đã kiểm bằng `rustfmt --check`). Đây là **đường lọt thật**, không
phải lý thuyết. Biến thể `cfgattr2` (đặt trong `core_sched.rs`, trỏ `../evil.rs`) cũng xanh.

### 5.2 **B2/B3 — `#[path]` và `include!` trong thân `macro_rules!`**

**Cơ chế**: `resolve_include_targets` phân giải `#[path]`/`include!` theo **tệp chứa token**
(tệp **định nghĩa** macro). Nhưng rustc giải **cả hai** theo **tệp gọi** — đã kiểm bằng
`rustc` độc lập ([`kt187-logs/rustc-semantics.log`](kt187-logs/rustc-semantics.log)): macro
định nghĩa ở `a/m.rs`, gọi ở `b/main.rs`; `include!("evil.rs")` nạp `b/evil.rs` (không phải
`a/evil.rs`), `#[path = "evil.rs"]` cũng nạp `b/evil.rs`. `scan_lib_macros` (dòng 833) chỉ
bắt `include!`/`#[path]` trong macro của **`v2/lib`**, **không** quét macro trong
`v2/kernel`/`v2/sim`.

**Dựng lại** (hai biến thể, cùng khung):

```rust
// v2/kernel/vifast-core/src/m.rs  (sản phẩm)
#[macro_export]
macro_rules! evil_mod {          // B2: #[path];  B3: include!
    () => { #[path = "evil.rs"] mod x; };   // B2
    // () => { include!("evil.rs"); };      // B3
}
// v2/kernel/vifast-core/src/evil.rs  (decoy sạch, cùng thư mục m.rs)
pub fn clean() {}
// v2/kernel/vifast-core/src/sub.rs
#[cfg(test)]
mod evil;
mod s;
// v2/kernel/vifast-core/src/sub/s.rs
crate::evil_mod!();   // hoặc crate::evil_inc!()
// v2/kernel/vifast-core/src/sub/evil.rs  (test-only, có wake)
```

**Kết quả** ([`kt187-logs/compile-macro-path.log`](kt187-logs/compile-macro-path.log),
[`kt187-logs/compile-macro-include.log`](kt187-logs/compile-macro-include.log)):

```
### B2 checker hiện tại (chế độ thường)   # B3 giống hệt
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B2 checker hiện tại --strict          # B3 giống hệt
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
### B2 cargo fmt --all --check
fmt rc=0
### B2 cargo check -p vifast-core
warning: method `duong_san_pham` is never used
 --> vifast-core/src/sub/evil.rs:4:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s
rc=0
### B3 cargo fmt --all --check
fmt rc=0
### B3 cargo check -p vifast-core
warning: method `duong_san_pham` is never used
 --> vifast-core/src/sub/evil.rs:4:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.16s
rc=0
```

⇒ **mã sản phẩm biên dịch `wake`** (đúng `sub/evil.rs`), checker xanh ở cả hai chế độ.
Đường này **khó hơn** B1 (cần decoy sạch cùng thư mục định nghĩa + tệp test-only cùng tên ở
thư mục gọi), nhưng là lỗ **thật** của thuật toán phân giải — và **rộng hơn** tác giả nghĩ:
không chỉ `#[path]` mà **cả `include!`** trong macro cũng bị phân giải sai.

### 5.3 Ba dạng viết cách khác — lọt **checker**, nhưng `fmt` bắt

| Dạng | Checker | `rustfmt --check` | Ghi chú |
|---|---|---|---|
| `include ! ("evil.rs")` (cách `!`) | **XANH** | **ĐỎ** (chuẩn hoá `include!(…)`) | `resolve_include_targets` bỏ qua tệp vì `"include!" not in text` |
| `#[ path = "evil.rs" ]` (cách `#[`) | **XANH** | **ĐỎ** (chuẩn hoá `#[path = …]`) | regex đòi `#[path` liền |
| `#[\npath = "evil.rs"\n]` | **XANH** | **ĐỎ** | như trên |

Ba dạng này **không** qua được `cargo fmt --check` (mà `v2 check` chạy đầu tiên), nên không
phải đường lọt **cổng đầy đủ** — nhưng là lỗ của **checker** (nếu ai đó chạy checker trực
tiếp, hoặc fmt bị bỏ qua). Đáng vá cùng lượt vì cùng gốc "regex giả định định dạng chuẩn".

### 5.4 Các đường khác — **không** lọt

| Đường | Kết quả | Ghi chú |
|---|---|---|
| `include!` trong macro `v2/kernel`/`v2/sim`, **cùng thư mục** định nghĩa/gọi | **ĐỎ** (`--strict`) | phân giải trùng tệp nên đúng; **khác thư mục** ⇒ lọt (B3, §5.2) |
| `mod` khai trong macro | **XANH** nhưng **không** lọt | tệp không tới được từ crate root ⇒ mặc định **sản phẩm** ⇒ `wake` bị bắt; muốn test-only phải có `#[cfg(test)] mod` khác, mà khi đó macro không được biên dịch ở bản sản phẩm |
| `paste!` / `concat_idents!` | **ĐỎ** (`--strict`) | `wake` là đối số macro không an toàn ⇒ `E_STRICT_TEST_ONLY_WAKE` |
| `#[path]` tuyệt đối (`/tmp/…`) | **ĐỎ** | `_inside_scan_tree` = None |
| `#[path]` với `\` (Windows) | **ĐỎ** | trên macOS là tên tệp literal ⇒ "không tồn tại" |
| `include!` trong `build.rs` | **ĐỎ** | **báo oan tiềm ẩn** (§4) |

## 6. H6/H7 — dựng lại

Script [`kt187-logs/t5-h6h7.sh`](kt187-logs/t5-h6h7.sh) (không dùng `repro-h6h7.sh`).

```
### H6 current --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_macro.rs:10: `wake` là đối số của macro `concat!` — … (KT-177 H6) …
rc=1
### H6 7e7e1b53 --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
### H7 current --strict
E E_STRICT_MACRO_INCLUDE   v2/lib/vifast-abi/src/lib.rs:128: thân `macro_rules! evil_inc` chứa `include!` — …
rc=1
### H7 7e7e1b53 --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

⇒ **ĐẠT** — cả hai **ĐỎ** trên checker hiện tại, **XANH** trên `7e7e1b53`.

### 6.1 Ghi chú: H7 "đường dẫn giải theo **tệp gọi**" là **đúng** — và đó là lý do B2/B3 lọt

Thông điệp `E_STRICT_MACRO_INCLUDE` (dòng ~905) và journal KT-184 §3 nói `include!` trong
thân macro giải theo **tệp gọi**. Kiểm bằng `rustc` độc lập: **đúng** — macro định nghĩa ở
`a/m.rs`, gọi ở `b/main.rs`, `include!("evil.rs")` nạp `b/evil.rs` (không phải `a/evil.rs`);
`#[path]` cũng vậy ([`kt187-logs/rustc-semantics.log`](kt187-logs/rustc-semantics.log)).
Quy tắc fail-closed cho `v2/lib` vì thế **đúng hướng**, nhưng **chưa đủ**: nó chỉ áp cho
`v2/lib`, trong khi macro trong `v2/kernel`/`v2/sim` cũng giải theo tệp gọi và **không** bị
chặn — đúng đường B2/B3. Đề xuất §7.2 là mở rộng chính quy tắc đó, không phải thêm luật mới.

## 7. Đề xuất vòng vá (cho Agent 0 / task kế)

### 7.1 Bắt `cfg_attr(…, path = …)` — **bắt buộc**

Trong `resolve_include_targets`, thêm quét `#\[\s*cfg_attr\s*\(` và trích mọi
`path\s*=\s*<literal>` **bên trong** ngoặc của `cfg_attr`; nếu `cfg_attr` có `path` mà
không trích được literal ⇒ **ĐỎ fail-closed** (`E_STRICT_WAKE_INCLUDE`). Đồng thời
`_mods_of` phải hiểu `cfg_attr(…, path = …)` để đồ thị module không bỏ sót module sản phẩm.
Tối thiểu: **cấm** `cfg_attr` chứa `path` trong `v2/kernel`+`v2/sim` (fail-closed) — rẻ và
đúng tinh thần D2-161.

### 7.2 Bắt `#[path]`/`include!` trong macro của `v2/kernel`+`v2/sim` — **bắt buộc**

Mở rộng `scan_lib_macros` (dòng 833) quét **cả** `v2/kernel`+`v2/sim`, không chỉ `v2/lib`:
mọi `include!`/`#[path]` trong thân `macro_rules!` ⇒ `E_STRICT_MACRO_INCLUDE` (fail-closed).
Lý do: **cả hai** đều giải theo **tệp gọi** (đã kiểm bằng `rustc`), mà tệp gọi thì checker
**không** biết khi chỉ nhìn thân macro — không phân giải tĩnh an toàn được. Đây là mở rộng
quy tắc đã có cho `v2/lib` (H7), không phải luật mới.

### 7.3 Chuẩn hoá trước khi regex — **nên**

Thay `#\[path\s*=` bằng `#\s*\[\s*path\s*=` và `\binclude!\s*[\(\{\[]` bằng
`\binclude\s*!\s*[\(\{\[]` (sau `strip_rust_noise`), hoặc tokenize. Cùng gốc với §5.3.

### 7.4 Quyết định chính sách — **cần Agent 0**

- `build.rs` dưới `v2/kernel`: miễn khỏi luật literal (build script hợp lệ dùng
  `include!(concat!(env!("OUT_DIR"), …))`), hay vẫn áp?
- `#[cfg(test)]` include trỏ **ra ngoài** cây: giữ ĐỎ (đúng chữ D2-161) hay miễn cho tệp
  test-only?
- H4 có cần **ĐỎ ở chế độ thường** không? Hiện H4 chỉ đỏ ở `--strict`; D2-160 không nói rõ
  chế độ. Nếu cần, thêm luật `wake` vào chế độ thường (hoặc một cờ riêng).

## 8. Chưa review / giới hạn

- **Không** chạy `gate --fast` (theo đề). `./v2/bin/v2 check` trên cây sạch: **PASS**
  (log [`kt187-logs/v2-check.log`](kt187-logs/v2-check.log)).
- **Không** kiểm Miri/L1/QEMU — ngoài phạm vi KT-187.
- **Không** kiểm nhánh `--json` của checker (không liên quan D2-160/161).
- Các đường vòng **chưa** thử: `#[cfg_attr(any(), path = …)]` với nhiều attr trong một
  `cfg_attr`; `include!` trong `macro_rules!` của `v2/lib` gọi từ `v2/kernel` (đã bị
  `E_STRICT_MACRO_INCLUDE` chặn); `#[path]` trên `mod` inline (Rust từ chối — không phải
  đường lọt); `build.rs` của crate khác trong `v2/kernel` (chỉ có một `build.rs`).
- **Không** sửa mã sản phẩm; mọi đột biến trong `$TMPDIR/kt187-l25`; cây worktree sạch
  trước commit.

## 9. Tệp bằng chứng

| Tệp | Nội dung |
|---|---|
| [`kt187-logs/t1-h4.sh`](kt187-logs/t1-h4.sh) + `h4.log` | H4 hai/ba lớp, checker hiện tại vs `7e7e1b53` |
| [`kt187-logs/t2-d2161.sh`](kt187-logs/t2-d2161.sh) + `d2161.log` | D2-161 sáu cây (sản phẩm + test-only) vs `b6efc403`/`9921f606^` |
| [`kt187-logs/t3-bypass.sh`](kt187-logs/t3-bypass.sh) + `bypass.log` | 14 đường vòng/báo oan |
| [`kt187-logs/t4-teeth.py`](kt187-logs/t4-teeth.py) + `teeth.log` | "răng" 8 ca selftest mới |
| [`kt187-logs/t5-h6h7.sh`](kt187-logs/t5-h6h7.sh) + `h6h7.log` | H6/H7 |
| [`kt187-logs/t6-fmt.sh`](kt187-logs/t6-fmt.sh) + `fmt.log` | ba dạng viết cách khác |
| [`kt187-logs/t7-compile.sh`](kt187-logs/t7-compile.sh) + `compile-cfgattr.log`, `compile-macro-path.log`, `compile-macro-include.log` | **ba đường lọt biên dịch được** |
| [`kt187-logs/t8-rustc.sh`](kt187-logs/t8-rustc.sh) + `rustc-semantics.log` | ngữ nghĩa rustc: `include!`/`#[path]` trong macro giải theo **tệp gọi**; `cfg_attr` nạp tệp đích; ba dạng viết cách khác hợp lệ |
| [`kt187-logs/run-all.sh`](kt187-logs/run-all.sh) | chạy lại toàn bộ |
| `v2-check.log` | `./v2/bin/v2 check` trên cây sạch (PASS) |
