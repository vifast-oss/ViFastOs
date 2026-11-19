# KT-230 — đường lọt #14: `macro_rules!` **lồng** che tên macro "an toàn"

- **Người tìm**: reviewer L54 (task KT-229, tấn công theo **lớp**).
- **Người vá**: L54 (task KT-230, cùng nhánh `night3/L54-kt229`).
- **Trạng thái**: **ĐÃ VÁ** — repro #14 ĐỎ cả ba; cây thật xanh; selftest xanh.
- **Nguồn sự thật**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` §4 (dòng KT-230).

---

## 1. Cơ chế (theo LỚP, không theo danh sách 13 đường lọt cũ)

Luật `wake()` chỉ-dùng-cho-test (D2-144) có **hai** lớp:

| Lớp | Tệp | Nguồn danh sách tệp |
|---|---|---|
| 1 (tĩnh) | `v2/tools/check-wake-paths.py` | `iter_rs(v2/kernel, v2/sim)` + `v2/lib/**/*.rs` + đích `include!`/`#[path]` |
| 2 (dep-info) | `v2/tools/check-wake-depinfo.py` | **danh sách tệp rustc thật nạp** (đọc `.d`) |

Cả hai lớp dùng chung `_check_wake_text` và `_shadowed_safe_macros` (import, không chép — KT-214).

`_check_wake_text` miễn token `wake` khi nó là **đối số của macro "an toàn"** (`SAFE_VALUE_MACROS`:
`assert`, `matches`, `format`, `stringify`, `concat`, …) **và** có **binding cục bộ** `wake` cùng khối
(miễn KT-171 B10 — ca hợp lệ: `let wake = 0u32; matches!(x, wake, y)`).

Điều kiện "macro an toàn" được bảo đảm bởi `_shadowed_safe_macros`: nếu trong cây có
`macro_rules! <tên-an-toàn>` thì tên đó **không** còn an toàn (fail-closed, KT-177/G6).

**Lỗ (đường lọt #14)**: `_shadowed_safe_macros` bản KT-177 chỉ thấy `macro_rules!` **mức mục** —
`_walk_items` **bỏ qua thân `macro_rules!`**. Một định nghĩa **lồng trong thân macro khác** (macro
sinh macro) **không** bị thấy ⇒ tên vẫn "an toàn" ⇒ chuỗi miễn hoạt động ⇒ **lọt cả hai lớp**.

## 2. Repro (đã đo, `cargo check` RC=0)

`v2/lib/vifast-abi/src/kt229_nested.rs`:

```rust
macro_rules! kt229_outer {
    () => {
        #[macro_export]
        macro_rules! matches {
            ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };
        }
    };
}
kt229_outer!();
```

`v2/lib/vifast-abi/src/lib.rs`: `mod kt229_nested;`

`v2/kernel/vifast-core/src/kt229_b2.rs`:

```rust
use crate::{Machine, ObjId};
pub fn kt229_b2(m: &mut Machine, t: ObjId) {
    let wake = 0u32;
    let _ = wake;
    let _ = vifast_abi::matches!(m, wake, t);   // ⇒ m.wake(t)
}
```

`v2/kernel/vifast-core/src/lib.rs`: `mod kt229_b2;`

### Bằng chứng TRƯỚC khi vá (XANH cả ba)

| Lớp | Lệnh | Kết quả | Log |
|---|---|---|---|
| cargo | `cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf` | **RC=0** | — |
| 1 thường | `python3 v2/tools/check-wake-paths.py` | `errors=0` (RC=0) | `kt230-logs/b2-l1.log` |
| 1 `--strict` | `python3 v2/tools/check-wake-paths.py --strict` | `errors=0` (RC=0) | `kt230-logs/b2-l1s.log` |
| 2 | `python3 v2/tools/check-wake-depinfo.py --root .` | `errors=0` (RC=0) | `kt230-logs/b2-l2.log` |

> Ghi chú phương pháp: lần chạy đầu của B1 (biến thể `.txt`) bị **sai cwd** (job nền chạy ở
> checkout chính) nên cho XANH giả. Mọi số dưới đây đã chạy lại với `cd` **trong từng** subshell;
> log lớp 2 ghi `root=/private/tmp/night3-L54-kt229`.

### Đối chứng âm (bỏ đúng dòng gây lọt)

Bỏ `let wake = 0u32;` (giữ nguyên macro lồng + lời gọi) ⇒ **cả ba ĐỎ**:

- lớp 1: `E_STRICT_TEST_ONLY_WAKE … dùng wake làm giá trị` (RC=1) — `kt230-logs/b2n-l1.log`
- lớp 1 `--strict`: như trên (RC=1) — `kt230-logs/b2n-l1s.log`
- lớp 2: `E_DEPINFO_WAKE …` (RC=1) — `kt230-logs/b2n-l2.log`

⇒ Đường lọt phụ thuộc **đúng** chuỗi miễn (macro "an toàn" + local-binding), không phải may mắn.

## 3. Bản vá

### 3.1 `_collect_safe_macro_defs` — đệ quy **mọi** tầng (Cách B)

Thay `_walk_items` (chỉ đệ quy `{…}`, bỏ thân macro) bằng bộ quét **toàn cây token**:

- đệ quy **mọi** nhóm `{}`/`()`/`[]`, kể cả **thân `macro_rules!`** và **đối số lời gọi macro**;
- thấy `macro_rules! <tên>` với `<tên> ∈ SAFE_VALUE_MACROS` ở **bất kỳ** đâu ⇒ đánh dấu bị che
  (fail-closed).

### 3.2 Lớp 1 nhận `extra_files` (đích `include!`/`#[path]`, kể cả tệp **không `.rs`**)

`_shadowed_safe_macros(root, files, extra_files=…)`; `scan_v2` truyền
`[p for p, _ in extra.values()]` ở **cả ba** chỗ gọi (strict, thường, `scan_macro_bodies`).

Vì sao cần: lớp 2 vốn dùng **danh sách tệp rustc nạp** (gồm đích `include!`) nên bắt được biến thể
`.txt`; lớp 1 thì không. Đo trước bản này: biến thể `.txt` trong `v2/lib` = **lỗ hổng riêng lớp 1**
(lớp 1 XANH, lớp 2 ĐỎ).

### 3.3 Đích `v2/lib` **vào `extra`**

Nhánh `v2/lib` của `resolve_include_targets` trước đây cố ý **không** đưa đích vào `extra`, lý do
ghi trong mã: "mã `v2/lib` không có `Machine` nên `wake()` không thể xuất hiện". Lý do đó **sai**:

- tệp đích có thể chứa token `wake` (B12–B15 của KT-224: `.txt`, symlink, `target/`, dấu cách —
  lớp 1 XANH, chỉ lớp 2 bắt);
- tệp đích **không `.rs`** có thể định nghĩa `macro_rules!` che tên an toàn (biến thể `.txt` của #14).

Nay đích `v2/lib` là **sản phẩm** (`is_test=False`) và vào `extra` như mọi đích khác — fail-closed.
Cây thật: `v2/lib` chỉ có **một** đích `#[path]` (`vifast-idl/src/generated_pager.rs`, 282 dòng,
**không** có `wake`/`macro_rules!`) ⇒ **0 báo oan**.

### 3.4 Vì sao **không** chọn "Cách A" một mình

Cách A phát biểu "chỉ miễn khi macro nằm trong `SAFE_TOKEN_MACROS`" **không đủ** và **mâu thuẫn**
với đối chứng dương bắt buộc:

- nếu bỏ miễn cho `SAFE_VALUE_MACROS` ⇒ ca KT-171 B10 (`let wake = 0u32; matches!(x, wake, y)` với
  macro **thật**) thành **ĐỎ** — báo oan;
- nếu vẫn để rơi xuống miễn local-binding ⇒ đường lọt #14 **vẫn sống** (nó cũng có `let wake`).

Vì vậy điều kiện "chứng minh macro **không** mở rộng đối số thành mã" phải là **phát hiện che tên
đầy đủ** (Cách B). Cách A được giữ ở dạng **hệ quả**: khi tên bị che (phát hiện được) ⇒ nhánh
macro-arg **ĐỎ** ngay, không rơi xuống miễn local-binding.

## 4. Đo sau khi vá

### 4.1 Repro #14 ⇒ ĐỎ cả ba

| Lớp | Kết quả | Log |
|---|---|---|
| 1 thường | **RC=1**, `E_STRICT_TEST_ONLY_WAKE … tên matches bị macro_rules! trong cây che` | `kt230-logs/repro-sau-va-l1.log` |
| 1 `--strict` | **RC=1**, cùng lỗi | `kt230-logs/repro-sau-va-l1-strict.log` |
| 2 | **RC=1**, `E_DEPINFO_WAKE …` | `kt230-logs/repro-sau-va-l2.log` |

### 4.2 Cây thật (không có repro) ⇒ XANH

- lớp 1 thường: `errors=0` — `kt230-logs/cay-sach-l1.log`
- lớp 1 `--strict`: `errors=0` — `kt230-logs/cay-sach-l1-strict.log`
- **0 báo oan** (không có lỗi mới trên cây thật).

### 4.3 Selftest

| Bộ | Trước | Sau | Thêm |
|---|---|---|---|
| lớp 1 (`check-wake-paths.py --selftest`) | **259/259** | **281/281** | **+22** ca KT-230 |
| lớp 2 (`check-wake-depinfo.py --selftest`) | **31/31** | **34/34** | **+3** ca KT-230 |

Ca lớp 1 (mỗi ca chạy **cả** chế độ thường và `--strict` khi ghi "(thường)/(--strict)"):

1. repro #14 lồng một tầng + `let wake` + `matches!(m, wake, t)` ⇒ **ĐỎ** (×2)
2. repro #14 lồng **hai** tầng (macro sinh macro sinh macro) ⇒ **ĐỎ** (×2)
3. biến thể `.txt` trong `v2/lib` (đích `include!` không `.rs`) ⇒ **ĐỎ** (×2)
4. biến thể `.txt` trong `v2/kernel` (đích vào `extra`) ⇒ **ĐỎ** (×2)
5. đối chứng âm: bỏ `let wake = …` ⇒ **ĐỎ** (×2)
6. đối chứng dương: `matches!(t, wake)` macro **thật** + `let wake` ⇒ **XANH** (×2)
7. đối chứng dương: `stringify!(wake)` + `let wake` ⇒ **XANH** (×2)
8. đối chứng dương: `concat!("wa", "ke")` ⇒ **XANH** (×2)
9. đối chứng dương: `assert!(wake)` với `let wake = true` ⇒ **XANH** (×2)
10. đối chứng dương: `macro_rules! matches` **mức mục** trong `v2/lib` (KT-177) ⇒ **ĐỎ** (×2)
11. đối chứng dương: macro tên **không** an toàn (`evil!`) ⇒ **ĐỎ** (×2)

Ca lớp 2:

- (31) macro lồng + `let wake` + `matches!(m, wake, t)` trong tệp **nạp** ⇒ **ĐỎ** `E_DEPINFO_WAKE`
- (32) đối chứng dương: `matches!(t, wake)` macro **thật** + `let wake` ⇒ **XANH**
- (33) macro lồng trong tệp **`.txt`** được nạp + lời gọi ⇒ **ĐỎ** `E_DEPINFO_WAKE`

## 5. Rủi ro còn lại (ghi rõ, **không** tuyên bố đã đóng)

| Dạng | Trạng thái | Ghi chú |
|---|---|---|
| Macro che tên an toàn định nghĩa bởi **proc-macro** | **Mở** (không có trong cây) | Cây **không** có crate `proc-macro = true` (đo: `grep proc-macro` trên mọi `Cargo.toml` = 0). Nếu tương lai thêm, `_shadowed_safe_macros` **không** thấy được — cần luật riêng (cấm `use <crate>::<tên-an-toàn>` ngoài `std`/`core`/`alloc`). |
| Macro che tên an toàn định nghĩa ở crate **ngoài** `v2/lib|kernel|sim` | **Mở** (không có trong cây) | `MACRO_SCAN_ROOTS` chỉ `v2/lib`, `v2/kernel`, `v2/sim`; lớp 2 quét **mọi tệp nạp** nên nếu crate đó là dependency thì bắt được; nếu không nạp thì không ảnh hưởng mã sản phẩm. |
| `macro_rules!` che tên an toàn trong **mã test-only** | **Đỏ có chủ đích** (fail-closed) | Cây thật không có; nếu có, `assert!(wake)` sản phẩm sẽ ĐỎ — chấp nhận (không nới). |

## 6. Việc **chưa** làm (bàn giao)

- Ma trận ≥40 tổ hợp của KT-229 **chưa chạy xong** (ưu tiên vá theo lệnh lead). Đã đo được:
  - **#14 SỐNG** (đã vá ở đây);
  - biến thể `.txt` của #14 = **lỗ hổng riêng lớp 1** (nay đã vá bằng `extra_files` + lib→`extra`);
  - **chưa đo**: alias `transition_tcb`/`TcbState::Ready` (nghi vấn: `use TcbState as S; m.transition_tcb(t, S::Ready)`
    và `let tt = Machine::transition_tcb;` + gọi cách xa >8 dòng — luật R2/E_STRICT_TRANSITION khớp **theo tên**
    trên **một dòng**, nên có thể lọt cả hai lớp). Đề nghị reviewer sau thử **trước**.
- Chưa chạy `gate --fast` (bị cấm trong task này); đã chạy `./v2/bin/v2 check` — xem `kt230-logs/v2-check.log`.
