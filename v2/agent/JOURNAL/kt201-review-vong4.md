# KT-201 — Review độc lập vòng 4: luật `wake` / `include!` / `#[path]`

**Reviewer**: L40 (sub-agent của Agent 1) · **Nhánh**: `night3/L40-verify` · **Gốc**: `8f5a1189`
**Checker**: `v2/tools/check-wake-paths.py` (4893 dòng, 118 ca `--selftest`)
**Hợp đồng đối chiếu**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` (100 dòng, bản A0 2026-11-19 §5)

---

## 0. KẾT LUẬN NGẮN

**KHÔNG ĐẠT.**

Tuyên bố "KT-194 + KT-184 DONE" ở `8f5a1189` **không đứng vững**. Có **một đường lọt
sống** (đã dựng lại đầy đủ, `cargo check` RC=0, `./v2/bin/v2 check` **PASS**, checker
`errors=0` ở **cả hai** chế độ) và **hai lỗ tiềm ẩn** (chưa có trong cây, nhưng một thay
đổi hợp lệ là lọt). Thêm **năm báo oan chưa ghi trong hợp đồng** và **bảy khẳng định hợp
đồng sai/thiếu**.

| Hạng mục | Kết quả |
|---|---|
| `--selftest` | **118/118** — xác nhận tuyên bố |
| `./v2/bin/v2 check` trên cây sạch | **PASS** — xác nhận |
| Đường lọt sống | **1** (lớp `macro_rules!` passthrough `:item`/`:expr`/`:stmt`/`:block`) |
| Lỗ tiềm ẩn | **2** (phạm vi `wake` hẹp hơn §1; `include!` trong `v2/lib`) |
| Báo oan | **8** (3 đã ghi §4, **5 chưa ghi**) |
| Khẳng định hợp đồng sai/thiếu | **7** |
| Răng selftest | 101 điểm `cases.append`, **0** hằng-đúng; mutation 34 phép → 25 chết, **6 sống**, 3 artifact |

---

## A. KIỂM LẠI CÁC TUYÊN BỐ NỀN

### A1. `--selftest` — 118/118 (XÁC NHẬN)

```
$ python3 v2/tools/check-wake-paths.py --selftest
...
# selftest: 118/118 ok
RC=0
```

Đếm thật: `grep -c '^ok - '` = **118**, `grep -c '^FAIL'` = **0**. Log:
[`kt201-logs/A1-selftest.log`](kt201-logs/A1-selftest.log).

### A2. `./v2/bin/v2 check` trên cây sạch — PASS (XÁC NHẬN)

```
$ ./v2/bin/v2 check
...
--- wake paths (KT-151: một cơ chế wake): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155: allowlist): OK
v2 check: PASS (L1 chạy trong gate --fast)
RC=0
```

Log: [`kt201-logs/A2-v2check.log`](kt201-logs/A2-v2check.log). **Không** chạy
`gate --fast` (bị cấm trong vòng này).

### A3. Đối chiếu hợp đồng với hành vi thật

Mỗi dòng §2/§3/§4 được dựng thành ca thật trong cây fixture (xem Phụ lục B). Ký hiệu:
**Đ** = khớp hợp đồng, **S** = hợp đồng sai/thiếu.

#### §1 — Phạm vi cây nguồn

| Khẳng định | Cách kiểm | Kết quả | Lệch |
|---|---|---|---|
| `v2/kernel/**` → `wake` chỉ-dùng-test | thêm `v2/kernel/vifast-newcrate/src/lib.rs` chứa `m.wake(t)` | **XANH** | **S** — `in_test_only_wake_scope` (dòng 174) chỉ phủ `vifast-core/src/`, `vifast-kbin-riscv64/src/`, `vifast-plat-*/src/`. Crate mới dưới `v2/kernel/` **không** bị quét `wake`. |
| `v2/sim/**` → như trên | thêm `v2/sim/vifast-sim2/src/lib.rs` chứa `m.wake(t)` | **XANH** | **S** — chỉ `v2/sim/vifast-sim/src/` nằm trong phạm vi. |
| `v2/kernel/**` (mọi tệp) | thêm `v2/kernel/vifast-core/build.rs` chứa `m.wake(t)` | **XANH** | **S** — chỉ `src/` được quét. |
| `v2/lib/**` → chỉ thân macro | `#[path]`/`include!` ngoài macro trong `v2/lib` | **XANH** | **Đ** với `#[path]` (§4 đã ghi); **S** với `include!` (§4 **không** ghi). |
| Đích `include!`/`#[path]` ra ngoài `v2/kernel`+`v2/sim` ⇒ ĐỎ | `include!("../../../test/evil.rs")` không macro | **ĐỎ** | **Đ** |
| … nhưng đặt trong **lời gọi macro** | `pass!(include!{"../../../test/evil.rs"})` | **XANH** | **S** — khẳng định chỉ đúng khi cạnh được **phân giải**; lời gọi macro làm cạnh biến mất. |

#### §2 — Đã hỗ trợ và được kiểm (18 ca, tất cả **Đ**)

| Khẳng định | Cách kiểm | Kết quả |
|---|---|---|
| `include!("x")`, `include!["x"]`, `include!{"x"}`, `include ! ( "x" )` | 4 ca, đích thật trong cây | XANH cả 4 |
| `"x"`, `r"x"`, `r#"x"#`, `r##"x"##` | 4 ca `#[path]` | XANH cả 4 |
| escape `\x2e`, `\u{2e}` | 2 ca `#[path]` | XANH cả 2 |
| `#[path = "x"]`, `#[ path = "x" ]`, `#[\npath = "x"]`, `#[r#path = "x"]` | 4 ca | XANH cả 4 |
| chú thích chen `#[/*c*/path = "x"]` | 1 ca | XANH |
| `cfg_attr(not(test), path=…)`, `cfg_attr` lồng | 3 ca | ĐỎ cả 3 (đúng: đích sản phẩm) |
| `mod NAME;` + `#[cfg(test)]`/`#[cfg(feature="ktest")]` | ca KT-196/F2 trong selftest | Đúng |
| ngữ cảnh test theo item/token-tree | ca KT-198/F2b | Đúng |
| `macro_rules!` mọi tên (Unicode), cả ba dấu thân | 3 ca | XANH cả 3 |
| `#[path = "sub"] mod m { … }` inline module | 1 ca | XANH |

#### §3 — Bị từ chối có chủ đích (11 ca, tất cả **Đ**)

| Khẳng định | Cách kiểm | Kết quả |
|---|---|---|
| `include!`/`#[path]`/`cfg_attr(path)` không literal | `concat!`, `env!` | ĐỎ `E_STRICT_WAKE_INCLUDE` |
| Đích ra ngoài cây | `include!` ra `v2/test` | ĐỎ |
| Đích không tồn tại | `#[path="nope.rs"]`, `include!("nope.rs")` | ĐỎ |
| `#[path]` trỏ **thư mục** trên `mod` không thân | `#[path="sub"] mod m;` + `sub/mod.rs` | ĐỎ — **rustc cũng từ chối** (`error: src/sub is a directory`), nên đây **không** phải báo oan |
| Thân macro chứa `include!`/`#[path]` | 2 ca | Đỏ `E_STRICT_MACRO_INCLUDE` |
| Thân macro chứa `$…:tt` | 1 ca | Đỏ `E_STRICT_MACRO_TT` |
| Thân macro `mod NAME;` | 1 ca | Đỏ `E_STRICT_MACRO_MOD` |
| Thân macro `mod $n;` / `mod r#x;` | 2 ca | Đỏ `E_STRICT_MACRO_INCLUDE` |
| `wake(` trong mã sản phẩm | 1 ca | Đỏ `E_STRICT_TEST_ONLY_WAKE` |
| Lỗi từ vựng | 5 ca `E_LEX_*` | Đỏ |

#### §4 — Sai lệch còn mở

| Khẳng định | Cách kiểm | Kết quả | Lệch |
|---|---|---|---|
| **F2**: luật `wake` chỉ đầy đủ ở `--strict` | `m.wake(t)` trong `vifast-core/src/evil.rs` | thường **XANH**, strict **ĐỎ** | **Đ** — trung thực |
| **Macro invocation**: `_walk_items` bỏ nhóm sau `!` | `pass!(include!{…})` | **XANH** | **Đ** nhưng **thiếu**: hợp đồng không nói checker **chấp nhận** định nghĩa macro `:item`/`:expr`/`:stmt`/`:block` — tức lỗ chỉ cách **một định nghĩa macro 3 dòng**. Xem §B1. |
| **`#[path]` trong `v2/lib`** không phân giải | `#[path]` trong `v2/lib` → đích ngoài cây | **XANH** | **Đ** nhưng **thiếu**: `include!` trong `v2/lib` (ngoài macro) cũng **không** phân giải — §4 chỉ nhắc `#[path]`. |
| **F3**: `cfg_attr` lồng trong `v2/lib` ngoài macro ⇒ XANH | — | **Đ** | **Đ** |
| Báo oan có trước: `#[cfg(test)] include!` | 1 ca | **ĐỎ** | **Đ** (đúng là báo oan) |
| Báo oan có trước: `#[cfg(feature="ktest")] fn` inline | 1 ca | strict **ĐỎ** | **Đ** |
| Báo oan có trước: `#[cfg(test)] const/static` | `#[cfg(test)] const X: fn(..) = M::wake;` | strict **ĐỎ** | **Đ** |
| Báo oan có trước: `macro_rules!` thân `()`/`[]` | `macro_rules! m ( ($x:expr) => { $x } );` | **XANH** | **S** — không tái hiện được báo oan |
| Báo oan có trước: block statement | `pub fn f() { { let x = 1; } }` | **XANH** | **S** — không tái hiện được |
| Báo oan có trước: match arm pattern **literal** | `match x { 1 => 1, _ => 0 }` | **XANH** | **S** — dạng "literal" **không** tái hiện; dạng **binding** `match x { wake => 1, _ => 0 }` mới ĐỎ (xem §B4 #5) |

---

## B. TẤN CÔNG

### B1. ĐƯỜNG LỌT SỐNG — `macro_rules!` passthrough `:item`/`:expr`/`:stmt`/`:block`

#### Nguyên nhân gốc

Hai dòng trong checker, cộng lại thành lỗ:

1. `_walk_items` (dòng 982) **bỏ toàn bộ nhóm sau `!`**:
   ```python
   if isinstance(node, Group):
       prev = nodes[i - 1] if i > 0 else None
       if _is_punct(prev, "!"):
           i += 1
           pending = []
           continue
   ```
   ⇒ `include!`/`#[path]` nằm trong **đối số** của một lời gọi macro **không bao giờ được
   phân giải**, nên `resolve_include_targets` không thấy cạnh nguồn, và đích **không** bị
   quét.

2. `_check_macro_body` (dòng 1560) chỉ từ chối **`$…:tt`** (dòng 1661):
   ```python
   if n.kind == "punct" and n.val == ":":
       if (i + 1 < len(nodes) and isinstance(nodes[i + 1], Tok)
           and nodes[i + 1].kind == "ident" and nodes[i + 1].val == "tt"):
           r.err("E_STRICT_MACRO_TT", ...)
   ```
   Nhưng `$i:item`, `$e:expr`, `$s:stmt`, `$b:block`, `$m:meta` **cũng** chở được
   `include!`/`#[path]` từ **lời gọi** — và **không** bị từ chối.

Hợp đồng §3 nói: *"Thân `macro_rules!` chứa `$…:tt` (passthrough) | `E_STRICT_MACRO_TT` |
có thể chở `mod`/`include!` từ **lời gọi**"*. Câu này **đúng nhưng gây hiểu sai**: nó khiến
người đọc tin rằng "đường chở từ lời gọi đã đóng". Thực tế chỉ đóng **một** fragment
specifier; năm fragment còn lại mở toang.

#### Mã dựng lại (đã kiểm trên cây thật)

`v2/lib/evil_kt201.rs` (tệp **ngoài** `v2/kernel`+`v2/sim`, nên không bị quét trực tiếp):

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn evil_kt201(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
```

`v2/kernel/vifast-core/src/kt201_evil.rs`:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt201_pass {
    ($i:item) => {
        $i
    };
}
kt201_pass!(include! {"../../../lib/evil_kt201.rs"});
```

thêm `mod kt201_evil;` vào `v2/kernel/vifast-core/src/lib.rs`.

#### Lệnh và kết quả (log đầy đủ ở Phụ lục C)

| Lệnh | Kết quả |
|---|---|
| `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core` | **RC=0** (chỉ warning `dead_code`; đã thêm `#[allow(dead_code)]` cho `v2 check` sạch) |
| `python3 v2/tools/check-wake-paths.py` | `errors=0 warnings=0` — **XANH** |
| `python3 v2/tools/check-wake-paths.py --strict` | `errors=0 warnings=0` — **XANH** |
| `./v2/bin/v2 check` | **PASS** (sau khi thêm header SPDX cho hai tệp repro) |

⇒ **Mã sản phẩm biên dịch lời gọi `wake()` (API chỉ-dùng-test, D2-144) mà cổng vẫn xanh.**

#### Các biến thể đã xác nhận (cùng một lớp)

| Biến thể | Kết quả | Ghi chú |
|---|---|---|
| `$i:item` + `include!{…}` | XANH | ca chính |
| `$i:item` + `include!(…);` | XANH | dấu `;` trong lời gọi |
| `$i:item` + `#[path = "…"] mod evil;` | XANH | không cần `include!` |
| `$e:expr` + `include!` (đích là biểu thức `m.wake(t)`) | XANH | |
| `$s:stmt` + `include!` (đích là câu lệnh) | XANH | |
| `$b:block` + `include!` | XANH | |
| macro định nghĩa ở `v2/lib` (`#[macro_export]`), gọi từ `v2/kernel` | XANH | |
| lời gọi macro **lồng hai lớp** | XANH | |
| đích ở `v2/lib` hoặc `v2/test` | XANH | ngoài mọi cây được quét |
| **đối chứng**: cùng `include!` **không** bọc macro | **ĐỎ** | chứng minh lời gọi macro là mấu chốt |
| **đối chứng**: `$t:tt` thay `$i:item` | **ĐỎ** | chứng minh `:tt` được kiểm còn `:item` thì không |

#### Vì sao đây là lỗi **mới** chứ không phải "đã biết"

Hợp đồng §4 ghi *"Cây thật **không** có lời gọi macro chứa `mod`/`#[path]` (đã grep).
**Không** tuyên bố fail-closed cho phạm vi này."* — đúng, nhưng **grep không phải cổng**.
Điều hợp đồng **không** nói: checker **chấp nhận** định nghĩa macro `:item` (một mẫu
hoàn toàn bình thường, không có dấu hiệu xấu), nên bất kỳ ai thêm một macro passthrough
hợp lý rồi dùng nó với `include!` là lọt **mà không cần cố ý lách**. Đây là lỗi **thiết
kế phòng thủ**, không phải "người viết cố tình".

### B2. Lỗ tiềm ẩn — phạm vi luật `wake` hẹp hơn §1

`in_test_only_wake_scope` (dòng 174) chỉ phủ:

```
v2/kernel/vifast-core/src/
v2/sim/vifast-sim/src/
v2/kernel/vifast-kbin-riscv64/src/
v2/kernel/vifast-plat-*/src/
```

§1 lại khẳng định `v2/kernel/**` và `v2/sim/**`. Ba ca dựng được **XANH**:

| Ca | Kết quả |
|---|---|
| `v2/kernel/vifast-newcrate/src/lib.rs` chứa `m.wake(t)` | XANH |
| `v2/sim/vifast-sim2/src/lib.rs` chứa `m.wake(t)` | XANH |
| `v2/kernel/vifast-core/build.rs` chứa `m.wake(t)` | XANH |

Hôm nay chưa có crate nào như vậy, nên đây là **lỗ tiềm ẩn**, không phải đường lọt sống.
Nhưng nó là **bẫy cho crate kế tiếp** — đúng loại bẫy đã sinh ra `vifast-plat-*` regex
(KT-165). Cách sửa đúng hướng: phủ **mọi** crate dưới `v2/kernel/` + `v2/sim/` trừ
allowlist tường minh, thay vì liệt kê cứng.

### B3. Lỗ tiềm ẩn — `include!` trong `v2/lib` ngoài macro

§4 chỉ ghi `#[path]` trong `v2/lib` không được phân giải. Nhưng `include!` trong `v2/lib`
**ngoài** thân macro cũng **không** được phân giải (`resolve_include_targets` chỉ lặp trên
`v2/kernel`+`v2/sim`):

| Ca | Kết quả |
|---|---|
| `v2/lib/vifast-abi/src/lib.rs`: `include!("../../../test/evil.rs")` (đích có `wake`) | **XANH** |
| `v2/lib/vifast-abi/src/lib.rs`: `#[path = "../../../test/evil.rs"] mod x;` | **XANH** |

Không khai thác trực tiếp được cho `wake` (crate `v2/lib` không có kiểu `Machine`), nhưng
**hợp đồng thiếu dòng** — người đọc §4 sẽ tưởng chỉ `#[path]` là hở.

### B4. Báo oan (checker ĐỎ, mã hợp lệ)

| # | Dạng | Chế độ | Đã ghi §4? |
|---|---|---|---|
| 1 | `#[cfg(test)] include!("evil.rs")` (đích có `wake`) | cả hai | **có** |
| 2 | `#[cfg(feature = "ktest")] fn f() { m.wake(t); }` inline | strict | **có** |
| 3 | `#[cfg(test)] const X: fn(&mut M, u32) = M::wake;` | strict | **có** |
| 4 | `#[cfg_attr(test, path = "evil.rs")] mod m;` (đích có `wake`) | cả hai | **KHÔNG** |
| 5 | `match x { wake => 1, _ => 0 }` (pattern binding tên `wake`) | strict | có (gộp "match arm pattern literal") |
| 6 | `if let Some(wake) = o { wake }` | strict | **KHÔNG** |
| 7 | `let g = \|wake: u32\| wake + 1;` (tham số closure) | strict | **KHÔNG** |
| 8 | `struct S { pub wake: u32 }` + `s.wake` | strict | **KHÔNG** |

Gốc của #4: `attr_path_edges` **không đọc vị từ `cfg`** — `cfg_attr(test, path=…)` và
`cfg_attr(not(test), path=…)` bị đối xử **giống nhau** (đều là cạnh "có thể sản phẩm"), nên
đích **chỉ-test** bị quét như sản phẩm. Gốc của #5–#8: `_exempt_local_wake` chỉ miễn
`let`/`for`/pattern-tuple, không miễn pattern binding, tham số closure, hay tên trường.

Cây thật **không** có dạng nào (checker xanh trên cây sạch), nên đây là báo oan **tiềm ẩn**
— nhưng hợp đồng §4 tự nhận là "danh sách báo oan có trước", và danh sách đó **thiếu 5
dòng**.

---

## C. RĂNG CỦA SELFTEST

### C1. Đọc mã 12 ca (yêu cầu ≥10)

Trích bằng AST (`ast.walk` trên `selftest`): **101** điểm `cases.append` với tuple
`(tên, điều_kiện, chi_tiết)`; **0** điều kiện hằng-đúng (`True`, `1`, `not False`).
118 ca lúc chạy > 101 điểm vì một số điểm nằm trong vòng lặp.

| # | Ca (dòng) | Điều kiện thật | Phân biệt đúng/sai? |
|---|---|---|---|
| 1 | `v2 sạch ⇒ xanh` (2885) | `rep.ok` | Có — đổi fixture là đỏ |
| 2 | `v2 thêm IpiKind::Wake ở arch ⇒ đỏ` (2891) | `(not rep.ok) and any("E_ARCH_DOORBELL" in e …)` | Có — đòi **đúng mã lỗi** |
| 3 | `v2 thiếu wake_step ⇒ đỏ` (2903) | `(not rep.ok) and any("E_ANCHOR_MISSING" in e …)` | Có |
| 4 | `--strict transition ngoài cửa wake ⇒ đỏ` (2919) | `(not rep.ok) and any("E_STRICT_TRANSITION" …)` | Có |
| 5 | `--strict gọi wake( trong mã sản phẩm ⇒ đỏ` (3012) | `(not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" …)` | Có |
| 6 | `--strict wake( trong #[cfg(test)] ⇒ xanh` (3048) | `rep.ok` | Có — đối chứng dương |
| 7 | `--strict wake( trong chú thích ⇒ xanh` (3072) | `rep.ok` | Có — đối chứng dương |
| 8 | `B10 wake làm đối số macro $name:ident ⇒ đỏ` (3474) | `(not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" …)` | Có |
| 9 | `B10c binding + dùng cùng khối ⇒ xanh` (3514) | `rep.ok` | Có |
| 10 | `B4 macro_rules! trong v2/lib chứa token wake ⇒ đỏ` (3532) | `(not rep.ok) and any("E_STRICT_MACRO_WAKE" …)` | Có |
| 11 | `KT-194a chuỗi escape \x2e ⇒ giải mã .` (4367) | `(not errs) and len(toks)==1 and toks[0].kind=="str" and toks[0].val=="a.b"` | Có — so **giá trị giải mã** |
| 12 | `KT-194a delimiter lệch (] ⇒ E_LEX_UNBALANCED` (4423) | `delim_ok` (so mã lỗi) | Có |

⇒ **12/12** ca có ít nhất một khẳng định thật sự phân biệt được đúng/sai. Không có
`assert True`.

### C2. Mutation test (mạnh hơn đọc mã)

Chép checker ra `/tmp`, chèn `return`/giá-trị-cố-định vào **đầu** từng hàm, chạy lại
`--selftest`. Nếu selftest vẫn 118/118 ⇒ **không ca nào phủ hàm đó**. (Không sửa tệp trong
repo — chỉ sửa bản sao.)

| Hàm bị đột biến | Selftest | Kết luận |
|---|---|---|
| `in_test_only_wake_scope` | 103/118 | chết |
| `_test_char_ranges` | 114/118 | chết |
| `attr_path_edges` | 101/118 | chết |
| `_check_macro_body` | 106/118 | chết |
| `cfg_is_test_tokens` | 106/118 | chết |
| `_check_wake_text` | 92/118 | chết |
| `resolve_include_targets` | 94/118 | chết |
| `scan_lexical` | 113/118 | chết |
| **`scan_structure`** | **118/118** | **SỐNG** |
| `_mods_of` | 106/118 | chết |
| `test_only_modules` | 113/118 | chết |
| `scan_lib_macros` | 116/118 | chết |
| `scan_kernel_macro_includes` | 107/118 | chết |
| `_shadowed_safe_macros` | 117/118 | chết |
| `_exempt_local_wake` | 115/118 | chết |
| `_macro_arg_of` | 116/118 | chết |
| **`_in_use_item`** | **118/118** | **SỐNG** |
| `strip_rust_noise` | 117/118 | chết |
| `_walk_items` | 78/118 | chết |
| `_decode_string` | 112/118 | chết |
| `_local_wake_scopes` | 115/118 | chết |
| `_delim_pairs` | 115/118 | chết |
| `_inside_scan_tree` | 93/118 | chết |
| `_protected_files` | 113/118 | chết |
| `classify_v1_channel` | 117/118 | chết |
| `find_functions` | 93/118 | chết |
| **`test_line_ranges`** | **118/118** | **SỐNG** |
| **`_attrs_test_context`** | **118/118** | **SỐNG** (và là **dead code** — không nơi nào gọi) |
| **`_feature_list`** | **118/118** | **SỐNG** |
| **`_cargo_sections`** | **118/118** | **SỐNG** |
| `crate_roots` | 113/118 | chết |
| `is_test_file` | 113/118 | chết |
| `tokenize` / `build_tree` / `parse_attr` | crash | artifact — hàm **có** được gọi, chỉ là đột biến làm script nổ |

**Đọc kết quả cho đúng**:

- **6 hàm sống**, nhưng **không** hàm nào trong số đó là đường phòng thủ **an ninh**:
  - `scan_structure`: chỉ phát `E_LEX_STRUCT` cho `mod` **không có định danh** — mã Rust
    **không hợp lệ**, rustc từ chối. Không có ca ⇒ lỗ **chất lượng chẩn đoán**, không phải
    lỗ an ninh.
  - `_in_use_item`: nếu trả `False`, `use …wake` vẫn **ĐỎ** qua nhánh dự phòng
    ("dùng `wake` làm giá trị") — chỉ **đổi thông điệp lỗi**. Không có ca phân biệt ⇒ lỗ
    chẩn đoán.
  - `test_line_ranges`, `_feature_list`, `_cargo_sections`: đột biến làm checker **chặt
    hơn** (fail-closed), nên không lộ đường lọt.
  - `_attrs_test_context`: **dead code** — định nghĩa nhưng **không nơi nào gọi**.
- **Mọi hàm phòng thủ an ninh đều bị giết** bởi ít nhất một ca. Đây là điểm **tốt** của
  selftest — và là lý do đường lọt ở §B1 là lỗi **thiết kế**, không phải "thiếu ca".

---

## D. KẾT LUẬN

### D1. Chấm

**KHÔNG ĐẠT.**

Lý do: có **một đường lọt sống** đã dựng lại được trên cây thật, `cargo check` RC=0,
`./v2/bin/v2 check` **PASS**, checker `errors=0` ở cả hai chế độ — trong khi mã sản phẩm
biên dịch lời gọi `wake()` (API chỉ-dùng-test, D2-144). Tuyên bố "KT-194 + KT-184 DONE" ở
`8f5a1189` là **sai**.

### D2. Danh sách đường lọt

| # | Đường | Mã dựng lại | Lệnh | Kết quả |
|---|---|---|---|---|
| **L1** | `macro_rules!` passthrough `:item`/`:expr`/`:stmt`/`:block` + `include!`/`#[path]` trong **lời gọi** ⇒ cạnh nguồn không phân giải ⇒ đích ngoài cây không bị quét | §B1 | `cargo check` RC=0; `v2 check` PASS; checker `errors=0` | **XANH (lọt)** |
| **L2** | Phạm vi `wake` hẹp hơn §1: crate mới dưới `v2/kernel/`/`v2/sim/`, hoặc `build.rs` | §B2 | checker | **XANH (lọt tiềm ẩn)** |
| **L3** | `include!` trong `v2/lib` ngoài macro không phân giải | §B3 | checker | **XANH (lọt tiềm ẩn, không khai thác trực tiếp được)** |

### D3. Danh sách báo oan

8 dạng ở §B4. **5 dạng chưa có trong §4**: #4 `cfg_attr(test, path)`, #5 pattern **binding**
tên `wake` (§4 chỉ ghi "pattern literal" — dạng literal XANH, dạng binding ĐỎ), #6 `if let`,
#7 tham số closure, #8 tên trường struct.

### D4. Khẳng định hợp đồng sai/thiếu

1. **§1** — "`v2/kernel/**` → `wake` chỉ-dùng-test": **sai** với crate mới và `build.rs`.
2. **§1** — "`v2/sim/**` → như trên": **sai** với crate sim mới.
3. **§1** — "Đích `include!`/`#[path]` ra ngoài ⇒ ĐỎ": **sai** khi đặt trong lời gọi macro.
4. **§3** — dòng `$…:tt`: **đúng nhưng thiếu** — `:item`/`:expr`/`:stmt`/`:block`/`:meta`
   cũng chở được, và **không** bị từ chối.
5. **§4** — "Macro invocation": **đúng nhưng thiếu** — không nói checker **chấp nhận**
   định nghĩa macro passthrough, nên lỗ chỉ cách một định nghĩa 3 dòng.
6. **§4** — "`#[path]` trong `v2/lib`": **thiếu** `include!` trong `v2/lib`.
7. **§4** — danh sách "báo oan có trước": **thiếu 5 dòng**; và **3 dòng không tái hiện
   đúng như mô tả** (`macro_rules!` thân `()`/`[]`, "block statement", "match arm pattern
   literal" — đều XANH trong các dạng tôi thử; riêng dạng **binding** tên `wake` thì ĐỎ).

### D5. Phạm vi đã kiểm

- 118 ca `--selftest` (đếm + đọc 12 ca + mutation 34 phép).
- `./v2/bin/v2 check` trên cây sạch (PASS) và trên cây có repro (PASS).
- 6 bộ ca tấn công, **~90 ca** dựng trên cây fixture tối thiểu (dùng chính `_fixture_v2`
  của checker) + **1 repro đầy đủ trên cây thật** có `cargo check` + `v2 check`.
- Đối chiếu **toàn bộ** §1–§5 của hợp đồng với hành vi thật.

### D6. Những gì **CHƯA** kiểm (đừng thổi phồng)

- **Không** chạy `gate --fast` (bị cấm). Chỉ suy ra phần checker của `gate` sẽ xanh vì
  `v2/bin/v2` gọi **đúng ba lệnh** checker (dòng 314/315/321) và cả ba đã xanh.
- **Không** kiểm L1/sim/Miri/soak — ngoài phạm vi luật này.
- **Không** kiểm nhánh v1 (`scan_v1`) ngoài 2 ca selftest có sẵn.
- **Không** kiểm proc-macro / `build.rs` / `OUT_DIR` (hợp đồng §5 đã tuyên bố ngoài phạm vi).
- **Không** kiểm symlink thật (chỉ đọc mã: `Path::resolve` giải symlink; không dựng ca).
- **Không** kiểm `#[cfg]` phức tạp khác `test`/`feature="ktest"` (hợp đồng §5 đã tuyên bố).
- **Không** kiểm đường lọt qua **macro đến từ crate phụ thuộc ngoài cây** — cây chỉ có
  `vifast-abi` (đã quét), không có `cfg-if`.
- **Không** kiểm `v2/lib` như một đường lọt `wake` **sống** — chỉ chứng minh cạnh không
  phân giải; chưa dựng được mã `v2/lib` gọi `wake` (crate lib không có `Machine`).

---

## Phụ lục A — Bảng ca tấn công đầy đủ

Xem log thô:
[`B-battery1.log`](kt201-logs/B-battery1.log) (34 ca nền + lớp đầu),
[`B-battery2.log`](kt201-logs/B-battery2.log) (macro passthrough + báo oan §4),
[`B-battery3.log`](kt201-logs/B-battery3.log) (đích ngoài cây),
[`B-battery4.log`](kt201-logs/B-battery4.log) (macro ở `v2/lib`, phạm vi),
[`B-battery5.log`](kt201-logs/B-battery5.log) (§2/§3/§4),
[`B-battery6.log`](kt201-logs/B-battery6.log) (`v2/lib`, `cfg_attr(test)`).

## Phụ lục B — Cách dựng ca

Mỗi ca dùng `_fixture_v2` của **chính checker** để dựng cây v2 tối thiểu hợp lệ, thêm tệp
tấn công, rồi gọi `scan_v2(root, strict=False)` và `scan_v2(root, strict=True)`. Vì
fixture không có chỗ `transition_tcb(…, Ready)` nào, harness đặt
`NON_WAKE_READY_SITES = {}` để loại nhiễu `E_STRICT_STALE` — **không** đụng luật `wake`.
Harness: `/tmp/kt201/harness.py`, các bộ ca `/tmp/kt201/battery{1..6}.py`.

## Phụ lục C — Log repro đầy đủ trên cây thật

- [`B1-repro-evil_kt201.rs.txt`](kt201-logs/B1-repro-evil_kt201.rs.txt) — tệp đích
- [`B1-repro-kt201_evil.rs.txt`](kt201-logs/B1-repro-kt201_evil.rs.txt) — macro + `include!`
- [`B1-repro-normal.log`](kt201-logs/B1-repro-normal.log) — `errors=0`
- [`B1-repro-strict.log`](kt201-logs/B1-repro-strict.log) — `errors=0`
- [`B1-repro-v2check.log`](kt201-logs/B1-repro-v2check.log) — `v2 check: PASS`

## Phụ lục D — Log mutation + trích ca

- [`C-mutation.log`](kt201-logs/C-mutation.log)
- [`C-cases-extract.log`](kt201-logs/C-cases-extract.log)
