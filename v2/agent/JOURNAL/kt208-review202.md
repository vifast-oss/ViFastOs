# KT-208 — Review độc lập vòng 5: bản vá KT-202 (lọt `include!`/`#[path]`/`mod` trong lời gọi macro)

**Reviewer**: L44 (sub-agent của Agent 1) · **Nhánh**: `night3/L44-review202` · **Gốc**: `78716b6d`
**Checker**: `v2/tools/check-wake-paths.py` (5174 dòng, **144** ca `--selftest`)
**Hợp đồng đối chiếu**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` (124 dòng, bản sau KT-202)
**Bản vá được review**: `[KT-202]` của L37 — `_scan_macro_calls` + `MACRO_PASSTHROUGH_FRAGS`

---

## 0. KẾT LUẬN NGẮN

**KHÔNG ĐẠT.**

Bản vá **đóng đúng** đường lọt L40 dựng (repro `:item` + `include!` trong đối số lời gọi macro:
trước XANH → sau **ĐỎ** cả hai chế độ, đã tái lập). Nhưng nó **không đóng lớp lỗi**: còn **một
đường lọt SỐNG khác**, cùng bản chất "cạnh nguồn không được phân giải", dựng lại được **6 biến
thể**, `cargo check` **RC=0**, checker **`errors=0` cả hai chế độ**, `./v2/bin/v2 check` chỉ đỏ
vì **T-INFRA đã biết** (SIGTRAP ở bước `luật cấu trúc`).

> **Đường lọt**: `include!` ở **vị trí biểu thức** nằm trong nhóm **`( … )`** hoặc **`[ … ]`**
> (không phải `{ … }`, không phải đối số lời gọi macro).
> `_walk_items` chỉ đệ quy `{ … }` ⇒ không thấy; `_scan_macro_calls` chỉ kiểm **nhóm sau `!`**
> ⇒ không thấy. Đích `include!` **không** được phân giải, **không** bị từ chối.

| Hạng mục | Kết quả |
|---|---|
| Repro L40 sau vá | **ĐỎ** cả hai chế độ (`errors=2`) — **xác nhận bản vá có tác dụng** |
| Biến thể lồng `( … )` của tác giả | **ĐỎ** cả hai chế độ (`errors=3`) — xác nhận |
| `--selftest` | **144/144** — xác nhận |
| `./v2/bin/v2 check` | **T-INFRA** (SIGTRAP `luật cấu trúc`, 2/2 lần; `rules` chạy bash tươi **RC=0**) |
| Đường lọt **SỐNG** còn lại | **1 lớp, 6 biến thể** (mục B1) |
| `:expr` (sai lệch tác giả khai) | **Bắt được** — không phải đường lọt |
| Báo oan do bản vá | **0** (17 macro phổ biến đều XANH) |
| Răng 16 ca mới | **16/16** có `assert` thật; 7/7 đột biến giết selftest; 10 ca cũ XANH |
| Khẳng định hợp đồng sai/thiếu | **6** |

---

## A. KIỂM LẠI CÁC TUYÊN BỐ NỀN

### A1. Repro L40 — XÁC NHẬN ĐÓNG (ĐỎ cả hai chế độ)

Dựng **nguyên văn** repro của L40 trên cây `78716b6d`:

`v2/lib/evil_kt201.rs`:
```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn evil_kt201(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
```

`v2/kernel/vifast-core/src/kt201_evil.rs` (+ `mod kt201_evil;` trong `lib.rs`):
```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt201_pass {
    ($i:item) => {
        $i
    };
}
kt201_pass!(include! {"../../../lib/evil_kt201.rs"});
```

| Lệnh | Kết quả |
|---|---|
| `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core` | **RC=0** |
| `python3 v2/tools/check-wake-paths.py` | **RC=1**, `errors=2` — `E_STRICT_MACRO_TT` + `E_STRICT_MACRO_INCLUDE` |
| `python3 v2/tools/check-wake-paths.py --strict` | **RC=1**, `errors=2` — cùng hai mã |

Log: [`kt208-logs/A1-repro-cargo.log`](kt208-logs/A1-repro-cargo.log),
[`A1-repro-normal.log`](kt208-logs/A1-repro-normal.log),
[`A1-repro-strict.log`](kt208-logs/A1-repro-strict.log).

⇒ **Tuyên bố "repro L1 trước XANH → sau ĐỎ cả hai chế độ" là ĐÚNG.**

### A2. Biến thể lồng `( … )` — XÁC NHẬN ĐÓNG

`v2/lib/evil_expr.rs` = `m.wake(t)`; `v2/kernel/vifast-core/src/kt201_nested.rs`:
```rust
macro_rules! pass_expr { ($e:expr) => { $e }; }
#[allow(dead_code)]
pub fn kt201_nested(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = (pass_expr!(include! {"../../../lib/evil_expr.rs"}));
}
```

| Lệnh | Kết quả |
|---|---|
| `cargo check` | **RC=0** (chỉ warning `unused_parens`) |
| checker thường | **RC=1**, `errors=3` |
| checker `--strict` | **RC=1**, `errors=3` |

Log: [`A2-nested-cargo.log`](kt208-logs/A2-nested-cargo.log),
[`A2-nested-normal.log`](kt208-logs/A2-nested-normal.log),
[`A2-nested-strict.log`](kt208-logs/A2-nested-strict.log).

### A3. `--selftest` — 144/144 (XÁC NHẬN)

```
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 144/144 ok
RC=0
```
Đếm thật: `grep -c '^ok - '` = **144**, `grep -c '^FAIL'` = **0**.
Log: [`A3-selftest.log`](kt208-logs/A3-selftest.log).

### A4. `./v2/bin/v2 check` — **T-INFRA** (đã biết, không phải hồi quy của KT-202)

```
--- fmt: OK
--- clippy: OK
--- luật cấu trúc R1..R5
./v2/bin/v2: line 255: 50871 Trace/BPT trap: 5       ( rules "$V2_ROOT" )
--- luật cấu trúc: FAIL
... (mọi bước còn lại OK, gồm `wake paths` + `wake paths --strict`, errors=0)
v2 check: 1 bước FAIL
```

Chạy lại **đúng một lần** (theo luật): **cùng SIGTRAP**, PID khác (`61612`).
Log: [`A4-v2check.log`](kt208-logs/A4-v2check.log),
[`A4-v2check-retry1.log`](kt208-logs/A4-v2check-retry1.log).

**Phân loại T-INFRA (đã kiểm, không đoán)**: trích nguyên hàm `rules` + `relpath` từ
`v2/bin/v2` và chạy trong **tiến trình bash mới** ⇒ **`RULES_RC=0`**.
Log: [`A4-rules-fresh-bash.log`](kt208-logs/A4-rules-fresh-bash.log).
Đây đúng là T-INFRA mà lead đã vá bằng KT-207 (chạy `rules` trong bash mới) — **bản vá đó chưa
có trên nhánh này**. Không phải lỗi của KT-202, không phải hồi quy.

---

## B. TẤN CÔNG

### B1. ĐƯỜNG LỌT **SỐNG** — `include!` ở vị trí biểu thức trong `( … )` / `[ … ]`

#### Nguyên nhân gốc (hai dòng, cộng lại thành lỗ)

1. `_walk_items` (dòng 952; nhánh quyết định ở **dòng 988**) **chỉ đệ quy nhóm `{ … }`**:
   ```python
   if node.open.val == "{":
       _walk_items(node.children, ...)
   i += 1
   ```
   ⇒ `include!` nằm trong `( … )`/`[ … ]` **không bao giờ** được `on_include` gọi ⇒
   `resolve_include_targets` không thấy cạnh ⇒ đích **không** bị quét, **không** bị từ chối.

2. `_scan_macro_calls` (mới, KT-202, dòng 1725) chỉ gọi `_check_macro_passthrough` cho **nhóm đứng sau `!`**:
   ```python
   if _is_punct(prev, "!"):
       _check_macro_passthrough(relp, "lời gọi macro", n.children, r)
   ```
   ⇒ với `include! { "…" }`, nhóm `{ "…" }` đứng sau `!` nên hàm chỉ kiểm **chuỗi bên trong**
   (không có token `include`), rồi **không đệ quy tiếp**. Token `include` **đứng trước** `!`
   không được kiểm ở đâu cả.

**Đo trực tiếp** `_walk_items` với callback `on_include` (log
[`E1-root-cause-walk-items.log`](kt208-logs/E1-root-cause-walk-items.log)):

| Dạng | `_walk_items` thấy `include!`? |
|---|---|
| `fn f() { sink(include! {"x.rs"}); }` — trong `( … )` | **KHÔNG** |
| `fn f() { let _ = [include! {"x.rs"}]; }` — trong `[ … ]` | **KHÔNG** |
| `fn f() { let _ = { include! {"x.rs"} }; }` — trong `{ … }` | CÓ |
| `fn f() { let _ = include! {"x.rs"}; }` — trần câu lệnh | CÓ |
| `include! {"x.rs"};` — mức mục | CÓ |

⇒ Bản vá KT-202 đóng đúng **đối số lời gọi macro**, nhưng **không** đóng `include!` trần trong
nhóm không-`{}`. Đây **không** phải hồi quy (lỗ có trước), nhưng hợp đồng §4 lại ghi
"**Macro invocation** … **Đã vá**" — tức tuyên bố **lớp lỗi** đã đóng, trong khi nó chưa.

#### Mã dựng lại (bản sạch, đã kiểm trên cây thật)

`v2/lib/evil_expr.rs` (tệp **ngoài** `v2/kernel`+`v2/sim`):
```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
m.wake(t)
```

`v2/kernel/vifast-core/src/kt208_attack.rs` (+ `mod kt208_attack;` trong `lib.rs`):
```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
fn kt208_sink<T>(_: T) {}

#[allow(dead_code)]
pub fn kt208_b01(m: &mut crate::Machine, t: crate::ObjId) {
    kt208_sink(include! {"../../../lib/evil_expr.rs"});
}
```

#### Lệnh và kết quả

| Lệnh | Kết quả |
|---|---|
| `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core` | **RC=0** |
| `python3 v2/tools/check-wake-paths.py` | **RC=0**, `errors=0` — **XANH** |
| `python3 v2/tools/check-wake-paths.py --strict` | **RC=0**, `errors=0` — **XANH** |
| `./v2/bin/v2 check` | chỉ **1** bước FAIL: `luật cấu trúc` = **T-INFRA SIGTRAP**; `wake paths` + `wake paths --strict` đều **OK** (`errors=0`) |

Log: [`B01-clean-cargo.log`](kt208-logs/B01-clean-cargo.log),
[`B01-clean-normal.log`](kt208-logs/B01-clean-normal.log),
[`B01-clean-strict.log`](kt208-logs/B01-clean-strict.log),
[`B01-clean-v2check.log`](kt208-logs/B01-clean-v2check.log).

#### Đối chứng âm — chứng minh `include!` được **mở thật**, không phải trang trí

Đổi `v2/lib/evil_expr.rs` thành `m.kt208_khong_ton_tai(t)`:
```
error[E0599]: no method named `kt208_khong_ton_tai` found for mutable reference `&mut Machine`
CARGO_RC=101
```
Khôi phục ⇒ `CARGO_RC=0`. Log:
[`B01-clean-negative-control.log`](kt208-logs/B01-clean-negative-control.log).

⇒ `include!` **thực sự** chèn `m.wake(t)` vào mã sản phẩm; checker vẫn xanh.

#### Sáu biến thể (cùng một lớp, đều `cargo RC=0` + checker **XANH** cả hai chế độ)

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| B01 | `sink(include! {"…"})` — đối số lời gọi **hàm** | 0 | **XANH** | **XANH** |
| B02 | `let _ = [include! {"…"}];` — mảng `[ … ]` | 0 | **XANH** | **XANH** |
| B03 | `let _ = (0u8, include! {"…"});` — tuple `( … )` | 0 | **XANH** | **XANH** |
| B23 | `let _ = (include! {"…"});` — trần trong `( … )` | 0 | **XANH** | **XANH** |
| B24 | `let mut f = (|| include! {"…"});` — thân closure trong `( … )` | 0 | **XANH** | **XANH** |
| B25 | `let _ = Some(include! {"…"});` — đối số hàm/biến thể | 0 | **XANH** | **XANH** |

Log: `kt208-logs/B-B01-*.log` … `B-B25-*.log`.

**Đối chứng dương của chính lớp này** (chứng minh `{ … }` thì bị bắt, `( … )` thì không):

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| B04 | `let _ = { include! {"…"} };` — khối `{ … }` | 0 | **ĐỎ** | **ĐỎ** |
| B05 | `let _ = include! {"…"};` — trần vị trí câu lệnh | 0 | **ĐỎ** | **ĐỎ** |

⇒ Khác biệt **duy nhất** là loại nhóm. Đây là lỗ hổng **cấu trúc**, không phải "người viết cố tình".

#### Cũng lọt ở `v2/lib` và `v2/sim`

| Ca | Kết quả |
|---|---|
| `v2/lib/vifast-abi/src/lib.rs`: `sink(include! {"../../../test/evil.rs"})` | **XANH** |
| `v2/sim/vifast-sim/src/lib.rs`: `sink(include! {"../../../test/evil.rs"})` | **XANH** |

Log: [`B27-lib-sim-coverage.log`](kt208-logs/B27-lib-sim-coverage.log).

#### Không ca selftest nào phủ đường lọt này

Thêm hai ca (B01 + B02) vào **bản sao** checker rồi chạy `--selftest`:
```
# selftest: 144/146 ok
RC=1
```
⇒ 144 ca hiện tại **không** phủ lớp lỗi này. Log:
[`C6-hole-not-covered-by-selftest.log`](kt208-logs/C6-hole-not-covered-by-selftest.log).

### B2. `:expr` — **BẮT ĐƯỢC**, sai lệch tác giả khai là **đúng**

Yêu cầu của phiếu: `macro_rules! p { ($e:expr) => { $e }; }` + `p!(include!{"…evil…"})` ở **vị
trí biểu thức** trong mã sản phẩm.

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| B06 | `let _ = kt208_pass!(include! {"…"});` | 0 | **ĐỎ** | **ĐỎ** |
| B07 | `let _ = (kt208_pass!(include! {"…"}));` | 0 | **ĐỎ** | **ĐỎ** |

Mã lỗi: `E_STRICT_MACRO_INCLUDE` — "lời gọi macro chứa `include!`".
Log: [`B-B06-*.log`](kt208-logs/B-B06-expr-frag-callargs.log),
[`B-B07-*.log`](kt208-logs/B-B07-expr-frag-nested.log).

⇒ **`:expr` KHÔNG phải đường lọt** trong phạm vi `MACRO_SCAN_ROOTS`: `include!` nằm trong **đối
số lời gọi** nên bị vá (1) bắt ở **tệp gọi**, đúng như tác giả khai. Lý do từ chối `:expr` của
tác giả (cây thật có `macro_rules! bail` dùng `$pages:expr, $e:expr`) là **hợp lệ** — tôi đã
kiểm `v2/kernel/vifast-plat-riscv64/src/user.rs:476` có thật.

**Lưu ý**: đường lọt B1 **không** liên quan `:expr` — nó là `include!` **trần** (không nằm trong
đối số macro), nên từ chối `:expr` cũng **không** đóng được nó.

### B3. Các fragment khác — không chở được `include!`

| Fragment | Thử | Kết quả rustc |
|---|---|---|
| `:ty` | `-> kt208_ty!(include! {"…"})` | `error: non-type macro in type position: include` |
| `:path` | `kt208_path!(include! {"…"})` | `error: no rules expected !` |
| `:pat` | `match x { kt208_pat!(include! {"…"}) => … }` | `error: non-pattern macro in pattern position: include` |
| `:ident`, `:literal`, `:vis`, `:lifetime` | — | không thể chở `include!`/`mod`/`#[path]` (ngữ pháp) |

⇒ Chỉ `:expr` chở được `include!`, và nó **đã bị bắt** ở tệp gọi. Danh sách
`MACRO_PASSTHROUGH_FRAGS = {tt, item, stmt, block, meta}` là **đủ** cho `include!`/`mod`/`#[path]`.

### B4. Lời gọi macro lồng, `#[cfg]` trong macro, `mod`/`#[path]` trong macro — **ĐỎ hết**

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| B11 | lồng **2** tầng: `a!{ b!{ include!{"…"} } }` | 0 | **ĐỎ** | **ĐỎ** |
| B12 | lồng **3** tầng | 0 | **ĐỎ** | **ĐỎ** |
| B15 | `pass!(#[cfg(feature="ktest")] include! {"…"});` | 0 | **ĐỎ** | **ĐỎ** |
| B16 | `pass!(#[cfg(test)] mod evil;);` | 0 | **ĐỎ** | **ĐỎ** |
| B17 | `pass!(mod evil;);` | 101¹ | **ĐỎ** | **ĐỎ** |
| B18 | `pass!(#[path = "…"] mod evil;);` | 0 | **ĐỎ** | **ĐỎ** |
| B19 | `cfg_if! { if #[cfg(…)] { mod evil; } }` | 101¹ | **ĐỎ** | **ĐỎ** |

¹ `cargo` 101 vì `mod evil;` **bên trong macro expansion** được rustc giải theo thư mục khác với
`kt208_attack.rs` (không tìm thấy tệp) — **không** phải lỗi của checker. Bản thân luật `mod`
vẫn bắt đúng (`E_STRICT_MACRO_MOD`).

Log: `kt208-logs/B-B11-*.log` … `B-B19-*.log`.

### B5. Đối chứng dương — **KHÔNG báo oan**

Một tệp thật trong `v2/kernel/vifast-core/src/` dùng 17 macro hợp lệ phổ biến:

`assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!`, `matches!`, `stringify!(mod)`,
`stringify!(include)`, `concat!`, `include_str!`, `include_bytes!`, `cfg!`, `env!`,
`option_env!`, `write!`, `writeln!`, `todo!`, `unreachable!`

| Lệnh | Kết quả |
|---|---|
| `cargo check` | **RC=0** |
| checker thường | **RC=0**, `errors=0` — **XANH** |
| checker `--strict` | **RC=0**, `errors=0` — **XANH** |

Log: [`B26-positive-controls-cargo.log`](kt208-logs/B26-positive-controls-cargo.log),
[`B26-positive-controls-normal.log`](kt208-logs/B26-positive-controls-normal.log),
[`B26-positive-controls-strict.log`](kt208-logs/B26-positive-controls-strict.log).

⇒ `stringify!(mod)` / `stringify!(include)` **XANH** — đúng như tác giả khai. Bản vá **không**
sinh báo oan mới.

### B6. Proc-macro — ngoài phạm vi (ghi nhận)

`paste!`/`concat_idents!` không có trong allowlist phụ thuộc; hợp đồng §5 đã ghi **không** tuyên
bố fail-closed cho proc-macro. **Không** kiểm thêm (không có trong cây).

---

## C. RĂNG CỦA 16 CA SELFTEST MỚI

### C1. Chạy 16 ca mới trên checker **CŨ** (`6793cc20`) — đúng **10** ca

Nạp hai checker bằng `importlib`, dựng **cùng** fixture (`_fixture_v2`), chạy `scan_v2`:

| # | Ca | kỳ vọng | MỚI | CŨ |
|---|---|---|---|---|
| 1 | repro `:item` + `include!` trong lời gọi (thường) | ĐỎ | ĐỎ | **XANH** |
| 2 | repro `:item` + `include!` trong lời gọi (`--strict`) | ĐỎ | ĐỎ | **XANH** |
| 3 | `:item` → `:tt` vẫn đỏ | ĐỎ | ĐỎ | ĐỎ |
| 4 | `include!` trần (không macro) vẫn đỏ | ĐỎ | ĐỎ | ĐỎ |
| 5 | `:item` passthrough trong thân (không lời gọi) | ĐỎ | ĐỎ | **XANH** |
| 6 | `include!` trong lời gọi macro (thân không passthrough) | ĐỎ | ĐỎ | **XANH** |
| 7 | `#[path]` trong lời gọi macro | ĐỎ | ĐỎ | **XANH** |
| 8 | `mod` trong lời gọi macro | ĐỎ | ĐỎ | **XANH** |
| 9 | `:stmt` passthrough | ĐỎ | ĐỎ | **XANH** |
| 10 | `:block` passthrough | ĐỎ | ĐỎ | **XANH** |
| 11 | `:meta` passthrough | ĐỎ | ĐỎ | **XANH** |
| 12 | `:expr` passthrough ⇒ xanh | XANH | XANH | XANH |
| 13 | đối chứng dương `assert_eq!`/`vec!`/`format!` | XANH | XANH | XANH |
| 14 | đối chứng dương `#[cfg(test)]` trong lời gọi macro | XANH | XANH | XANH |
| 15 | biến thể lồng trong `( … )` | ĐỎ | ĐỎ | **XANH** |
| 16 | đối chứng dương lời gọi macro lồng trong `( … )` | XANH | XANH | XANH |

**MỚI 16/16; CŨ 6/16; số ca kỳ vọng ĐỎ mà CŨ cho XANH = 10** — khớp tuyên bố của tác giả.
Log: [`C5-new-cases-on-old-checker.log`](kt208-logs/C5-new-cases-on-old-checker.log).

### C2. Đọc mã 16 ca — **16/16** có `assert` thật

Cả 16 ca đi qua **một** helper (dòng 4961):
```python
def _kt202_case(name, src, expect_red, strict=True):
    d = tmp / ("kt202-" + name)
    _fixture_v2(d)
    _write(d / "v2/kernel/vifast-core/src/kt202_evil.rs", src)
    rep = scan_v2(d, strict=strict, allowlist={})
    got = not rep.ok
    cases.append((f"KT-202 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                  got == expect_red, "; ".join(rep.errors)))
```
Điều kiện là `got == expect_red` — **phân biệt được đúng/sai** (không hằng-đúng, không `assert
True`). `expect_red` **khác nhau** giữa các ca (12 ĐỎ + 4 XANH) nên một checker luôn-ĐỎ hoặc
luôn-XANH đều **không** qua được. Chi tiết lỗi được giữ ở trường thứ ba.

### C3. Đột biến — **7/7 phép giết selftest** (mạnh hơn đọc mã)

Chép checker ra `/tmp`, chèn đột biến, chạy `--selftest` (**không** sửa tệp trong repo):

| Phép đột biến | Selftest | Kết luận |
|---|---|---|
| M1 — tắt `_scan_macro_calls(tree, relp, r)` | **140/144** | chết (4 ca) |
| M2 — `MACRO_PASSTHROUGH_FRAGS` chỉ còn `{tt}` | **140/144** | chết (4 ca) |
| M3 — `_check_macro_passthrough` thành `return` | **129/144** | chết (15 ca) |
| M4 — bỏ nhánh `include` | **138/144** | chết (6 ca) |
| M5 — bỏ nhánh `mod` | **140/144** | chết (4 ca) |
| M6 — bỏ nhánh `#[path]` | **141/144** | chết (3 ca) |
| M7 — `_scan_macro_calls` không đệ quy | **143/144** | chết (1 ca) |

⇒ 16 ca mới **có răng thật**, không phải trang trí.

---

## D. KIỂM HỢP ĐỒNG — 6 khẳng định sai/thiếu

Đối chiếu `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` (124 dòng) với hành vi **đo được**.
Log: [`D7-contract-claims.log`](kt208-logs/D7-contract-claims.log).

| # | Vị trí | Khẳng định | Thực tế | Đánh giá |
|---|---|---|---|---|
| 1 | §1 bảng | `v2/kernel/**` → `wake` chỉ-dùng-test | crate **mới** `v2/kernel/vifast-newcrate/src/lib.rs` chứa `m.wake(t)` ⇒ **XANH**; `v2/sim/vifast-sim2/src/lib.rs` ⇒ **XANH**; `v2/kernel/vifast-core/build.rs` ⇒ **XANH** | **SAI** — `TEST_ONLY_WAKE_SCOPE` chỉ phủ 4 mẫu cứng. §3 viết đúng ("thuộc `TEST_ONLY_WAKE_SCOPE`"), §1 viết sai. (L40 §B2 đã nêu; **chưa sửa**.) |
| 2 | §1 đoạn dưới | "Đích của `include!`/`#[path]` phải nằm trong `v2/kernel` hoặc `v2/sim`; trỏ ra ngoài ⇒ **ĐỎ** — **chỉ đúng khi cạnh phân giải được**" | Đúng chữ, nhưng **không** liệt kê trường hợp cạnh **không** phân giải được mà cũng **không** bị từ chối: `include!` trong `( … )`/`[ … ]` | **THIẾU** — đây chính là đường lọt B1. Câu "chỉ đúng khi cạnh phân giải được" là lối thoát hợp đồng, nhưng không nói **khi nào**. |
| 3 | §2 gạch đầu dòng KT-202 | "`_scan_macro_calls` quét **toàn cây token** (mọi tầng nhóm, kể cả `( … )`/`[…]`) tìm **lời gọi macro**" | Đúng với **lời gọi macro**; nhưng người đọc suy ra `include!` trong `( … )` được phủ — **không**. `include!` **trần** trong `( … )` **không** bị kiểm ở đâu | **THIẾU** — cần câu: "`include!` **trần** (không phải đối số lời gọi) trong `( … )`/`[ … ]` **không** được phân giải và **không** bị từ chối". |
| 4 | §4 dòng "Macro invocation (đã vá KT-202)" | "… Nay `_scan_macro_calls` quét **toàn cây token** … ⇒ `E_STRICT_MACRO_INCLUDE`/`E_STRICT_MACRO_MOD` (fail-closed). … **Đã vá**" | Đúng cho **đối số lời gọi macro**; **lớp lỗi** "cạnh nguồn không phân giải" **chưa** đóng (B1) | **SAI (quá mức)** — "Đã vá" tuyên bố đóng **lớp**, thực tế chỉ đóng **một dạng**. |
| 5 | §4 dòng "`#[path]` trong `v2/lib`" | Chỉ ghi `#[path]` trong `v2/lib` không được phân giải | `include!` trong `v2/lib` **ngoài** macro cũng **không** được phân giải: `v2/lib/vifast-abi/src/lib.rs` + `include!("../../../test/evil.rs")` (đích có `wake`) ⇒ **XANH** | **THIẾU** — L40 §B3 đã nêu; **chưa** thêm. |
| 6 | §4 "Báo oan có trước" | Danh sách 6 dạng | Đo thêm **4** dạng **ĐỎ** (strict) mà danh sách **không** có: (a) `#[cfg_attr(test, path = "evil.rs")] mod m;` (đích có `wake`) ⇒ **ĐỎ**; (b) `if let Some(wake) = o { wake }` ⇒ **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; (c) `let g = \|wake: u32\| wake + 1;` ⇒ **ĐỎ**; (d) `struct S { pub wake: u32 }` + `s.wake` ⇒ **ĐỎ** | **THIẾU** — L40 đã nêu; **chưa** thêm. |

**Khẳng định hợp đồng ĐÚNG (đã kiểm)**: §2 (KT-202 `_scan_macro_calls` + `MACRO_PASSTHROUGH_FRAGS`);
§3 dòng "`include!`/`#[path]`/`mod` trong **đối số lời gọi macro**" ⇒ ĐỎ; §3 dòng "`:expr`
**không** bị từ chối"; §4 dòng "`:expr` … chỉ còn hở khi macro được gọi từ tệp ngoài
`MACRO_SCAN_ROOTS`" (đúng — B06/B07 ĐỎ); §5 (proc-macro, `build.rs`, `#[cfg]` phức tạp).

---

## E. KẾT LUẬN

**KHÔNG ĐẠT.**

Bản vá KT-202 **làm đúng việc nó tuyên bố**: đóng đường lọt L40 (`:item` + `include!` trong đối
số lời gọi macro), mở rộng fragment, 16 ca selftest có răng, không báo oan, `v2 check` xanh
(trừ T-INFRA). Nhưng nó **không đóng lớp lỗi** mà nó được giao đóng: **`include!` ở vị trí biểu
thức trong `( … )`/`[ … ]` vẫn lọt**, `cargo check` RC=0, checker `errors=0` **cả hai** chế độ,
và **không** ca selftest nào phủ.

Theo tiêu chí phiếu: *"nếu nó không sạch, luật `wake`/`include!`/`#[path]` **không** được đóng"*
⇒ **luật `include!` KHÔNG được đóng**.

### Đường lọt cần đóng (đề xuất, **không** tự vá)

Cách sửa tối thiểu, **không** nới ngưỡng: cho `_scan_macro_calls` (hoặc một hàm mới) kiểm
**token `include` + `!` ở mọi vị trí**, không chỉ trong nhóm sau `!` — tức thêm nhánh
`if n.kind == "ident" and n.val == "include" and next is "!"` vào vòng lặp **cấp token** của
`_scan_macro_calls`, và đệ quy **mọi** nhóm (kể cả nhóm sau `!`, sau khi đã kiểm). Cách khác:
cho `_walk_items` đệ quy **mọi** nhóm khi tìm `include!` (giữ nguyên luật `mod` chỉ trong `{…}`).
**Phải** thêm ca selftest cho `( … )`/`[ … ]` **trước** khi sửa (hợp đồng §7).

---

## Phụ lục A — Bảng ca tấn công đầy đủ

| # | Dạng | cargo | thường | strict | Kết luận |
|---|---|---|---|---|---|
| B01 | `sink(include! {"…"})` | 0 | XANH | XANH | **LỌT** |
| B02 | `[include! {"…"}]` | 0 | XANH | XANH | **LỌT** |
| B03 | `(0u8, include! {"…"})` | 0 | XANH | XANH | **LỌT** |
| B04 | `{ include! {"…"} }` | 0 | ĐỎ | ĐỎ | đóng |
| B05 | `let _ = include! {"…"};` | 0 | ĐỎ | ĐỎ | đóng |
| B06 | `:expr` + `include!` trong đối số macro | 0 | ĐỎ | ĐỎ | đóng |
| B07 | `:expr` + `include!` trong đối số macro, lồng `( … )` | 0 | ĐỎ | ĐỎ | đóng |
| B11 | lồng 2 tầng | 0 | ĐỎ | ĐỎ | đóng |
| B12 | lồng 3 tầng | 0 | ĐỎ | ĐỎ | đóng |
| B15 | `#[cfg]` + `include!` trong đối số macro | 0 | ĐỎ | ĐỎ | đóng |
| B16 | `#[cfg(test)] mod` trong đối số macro | 0 | ĐỎ | ĐỎ | đóng |
| B17 | `mod` trong đối số macro | 101¹ | ĐỎ | ĐỎ | đóng |
| B18 | `#[path]` trong đối số macro | 0 | ĐỎ | ĐỎ | đóng |
| B19 | `cfg_if!` + `mod` | 101¹ | ĐỎ | ĐỎ | đóng |
| B23 | `(include! {"…"})` trần | 0 | XANH | XANH | **LỌT** |
| B24 | `(‖ include! {"…"})` thân closure | 0 | XANH | XANH | **LỌT** |
| B25 | `Some(include! {"…"})` | 0 | XANH | XANH | **LỌT** |
| B26 | 17 macro hợp lệ phổ biến | 0 | XANH | XANH | không báo oan |
| B27 | `v2/lib`/`v2/sim`: lời gọi macro + `include!` | — | ĐỎ | ĐỎ | đóng |
| B27 | `v2/lib`/`v2/sim`: `include!` trần trong `( … )` | — | XANH | XANH | **LỌT** |

¹ `cargo` 101 do cách rustc giải `mod` trong macro expansion, **không** phải lỗi checker.

## Phụ lục B — Tệp log

Tất cả trong [`kt208-logs/`](kt208-logs/):
`A1-repro-{cargo,normal,strict}.log`, `A2-nested-{cargo,normal,strict}.log`,
`A3-selftest.log`, `A4-v2check.log`, `A4-v2check-retry1.log`, `A4-rules-fresh-bash.log`,
`B01-clean-{cargo,normal,strict,v2check,negative-control}.log`,
`B-B01..B-B27-*.log`, `B26-positive-controls-*.log`, `B27-lib-sim-coverage.log`,
`C5-new-cases-on-old-checker.log`, `C6-hole-not-covered-by-selftest.log`,
`D7-contract-claims.log`.
