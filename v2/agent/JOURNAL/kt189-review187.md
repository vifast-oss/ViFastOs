# KT-189 — review đối kháng bản vá B1/B2/B3 (KT-187)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer độc lập L28** · nhánh `night2/L28-review187` ·
worktree `/private/tmp/night2-L28-review187` · HEAD `1182d1b4` (từ `main` @ `1182d1b4`).
**Nguồn đọc**: [`kt187-review184.md`](kt187-review184.md), `git show 1182d1b4`,
`v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 12:30 JST. Không merge/push/`gate --fast`/QEMU/worktree mới.
**Vai**: reviewer độc lập theo **D2-159** — mọi đột biến dựng lại trên **bản sao cây** trong
`/tmp/kt189-l28`, **không** dùng script của tác giả hay của L25; không sửa mã sản phẩm.

## 0. Phán quyết

| # | Yêu cầu | Kết quả | Căn cứ |
|---|---|---|---|
| 1 | B1 `cfg_attr(path)` dựng lại ⇒ `--strict` **ĐỎ**; `8670cdfe` cùng cây ⇒ `errors=0` | **ĐẠT** | §1.1, [`b1-orig.log`](kt189-logs/b1-orig.log) |
| 1b | `cfg_attr(all(not(test), target_os="none"), path = "…")` | **ĐẠT** (ĐỎ; nhưng không biên dịch được trên host) | §1.2, [`b1-allcfg.log`](kt189-logs/b1-allcfg.log) |
| 1c | `cfg_attr(test, path = "…")` — có báo oan **mới**? | **ĐẠT** — không có báo oan mới (ĐỎ ở `--strict` **cả** bản cũ lẫn mới) | §1.3, [`b1-testcfg.log`](kt189-logs/b1-testcfg.log) |
| 1d | `cfg_attr(not(test), path = concat!(…))` | **ĐẠT** (ĐỎ fail-closed; rustc từ chối) | §1.4, [`b1-concat.log`](kt189-logs/b1-concat.log) |
| 2 | B2 `#[path]` trong macro dựng lại ⇒ **ĐỎ ở chế độ thường**; `8670cdfe` ⇒ XANH | **ĐẠT** (đúng dạng **ngoặc nhọn**) | §2.1, [`b2-orig.log`](kt189-logs/b2-orig.log) |
| 2b | B2 biến thể: macro `v2/lib` gọi `v2/kernel`; macro lồng; macro `v2/sim` | **ĐẠT** (ĐỎ) | §2.2, [`b2-libmacro.log`](kt189-logs/b2-libmacro.log), [`b2-nested.log`](kt189-logs/b2-nested.log), [`t8.log`](kt189-logs/t8.log) |
| 3 | B3 `include!` trong macro dựng lại ⇒ **ĐỎ ở chế độ thường** | **ĐẠT** (đúng dạng **ngoặc nhọn**) | §2.3, [`b3-orig.log`](kt189-logs/b3-orig.log) |
| 4 | Dạng viết cách khác | **ĐẠT CÓ ĐIỀU KIỆN** — `include!{…}`/`include![…]`/raw string bắt được; `include ! (…)`, `#[ path = … ]`, `# [ path = … ]`, `#[\npath = …]` **lọt checker** (bộ lọc rẻ chưa chuẩn hoá) nhưng `fmt` chuẩn hoá | §3, [`t4b.log`](kt189-logs/t4b.log), [`t4c.log`](kt189-logs/t4c.log) |
| 5 | Báo oan | **ĐẠT** — không tìm được báo oan **mới** | §4, [`t4.log`](kt189-logs/t4.log), [`t8.log`](kt189-logs/t8.log) |
| 6 | Selftest 67 ca: 4ba/4bb/4bc có **thật** | **ĐẠT** — bỏ cạnh đột biến ⇒ xanh, thêm lại ⇒ đỏ | §5, [`t6.log`](kt189-logs/t6.log) |
| 7 | Đường vòng mới | **KHÔNG ĐẠT — bốn đường lọt thật, đều qua `fmt` + `clippy -D warnings` + checker** | §6, [`gate-level.log`](kt189-logs/gate-level.log) |

### Phán quyết: **KHÔNG ĐẠT**

Bản vá đóng **đúng** ba dạng L25 đã dựng (B1 `cfg_attr(path)` ngoài macro; B2/B3
`#[path]`/`include!` trong `macro_rules!` **ngoặc nhọn**), và ba ca selftest mới có răng thật.
Nhưng **luật B2/B3 chưa đóng**: còn **bốn** đường để mã sản phẩm biên dịch `wake` trong khi
**cả ba** cổng — `cargo fmt --all --check`, `cargo clippy -p vifast-core -- -D warnings`,
`check-wake-paths.py` **thường lẫn `--strict`** — đều **xanh**:

1. **B2c** — `macro_rules!` dùng **ngoặc tròn** (hoặc **vuông**) + `#[path]` (§6.1).
2. **B3a** — `macro_rules!` ngoặc tròn/vuông + `include!` (§6.2).
3. **B2d** — `#[cfg_attr(not(test), path = "…")]` **trong thân macro** (§6.3) — B1+B2 kết hợp.
4. **T3c** — `mod` khai **trong macro** + lời gọi `#[cfg(not(test))]` (§6.4) — đúng dạng
   L25 §5.4 khai là "**không** lọt".

Cả bốn đều **không** phải lý thuyết: mỗi cây có **đối chứng dương** (đổi `self.wake(t)` thành
`self.khong_ton_tai(t)` ⇒ `cargo check` **ĐỎ** tại đúng tệp đích, xem §6.5) — tức bản sản phẩm
**thật sự** biên dịch lời gọi `wake`.

---

## 1. B1 — `#[cfg_attr(not(test), path = "…")]`

### 1.1 Dựng lại đúng dạng L25

Cây: bản sao `v2/kernel` + `v2/sim` + `v2/lib` + workspace `v2` trong `/tmp/kt189-l28/b1/orig`
(script [`t1-b1.sh`](kt189-logs/t1-b1.sh), **không** dùng `t7-compile.sh` của L25).

```rust
// v2/kernel/vifast-core/src/lib.rs (thêm vào cuối)
#[cfg_attr(not(test), path = "../../../lib/vifast-abi/src/evil_out.rs")]
mod evil2;
// v2/lib/vifast-abi/src/evil_out.rs
impl Machine { fn duong_san_pham(&mut self, t: ObjId) { let _ = self.wake(t); } }
```

```
### B1 CUR (chế độ thường)
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/lib.rs:122: `#[cfg_attr(path)]` trỏ ra ngoài `v2/kernel`+`v2/sim` (`../../../lib/vifast-abi/src/evil_out.rs`) — không quét được đích (fail-closed, KT-171 B6/B7)
# check-wake-paths: mode=v2 strict=0 errors=1 warnings=0
rc=1
### B1 CUR --strict
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1
### B1 OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B1 OLD 8670cdfe --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
### B1 cargo fmt --all --check
fmt rc=0
### B1 cargo check -p vifast-core
warning: method `duong_san_pham` is never used
 --> vifast-core/src/../../../lib/vifast-abi/src/evil_out.rs:4:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.97s
rc=0
```

⇒ **ĐẠT**. Bản vá bắt được `cfg_attr(…, path = …)` ở **cả hai** chế độ; bản `8670cdfe` xanh
trên **cùng cây** — lỗ hổng là thật, bản vá là thật.

### 1.2 `cfg_attr(all(not(test), target_os="none"), path = "…")`

```
### B1a CUR (chế độ thường)
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/lib.rs:122: `#[cfg_attr(path)]` trỏ ra ngoài … (fail-closed, KT-171 B6/B7)
rc=1
### B1a CUR --strict
rc=1
### B1a OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B1a cargo check -p vifast-core (target_os=none ⇒ path KHÔNG bật trên host)
error[E0583]: file not found for module `evil3`
```

⇒ **ĐẠT** về checker (regex `cfg_attr\s*\([^;{}]*?\bpath\s*=` không phụ thuộc nội dung cfg).
Trên host, `target_os="none"` **sai** nên `path` không bật ⇒ không phải đường lọt biên dịch
được; muốn biên dịch được phải dùng cfg **đúng** trên host (đã thử `all(not(test), unix)` —
cùng kết quả ĐỎ).

### 1.3 `cfg_attr(test, path = "…")` — báo oan?

```
### B1b CUR (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B1b CUR --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:5: gọi `wake(` trong mã sản phẩm …
rc=1
### B1b OLD 8670cdfe --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:5: gọi `wake(` trong mã sản phẩm …
rc=1
### B1b cargo check -p vifast-core (bản sản phẩm: KHÔNG nạp evil_b.rs)
 --> vifast-core/src/evil_b_product.rs:1:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.98s
rc=0
```

⇒ **Không có báo oan mới**: `cfg_attr(test, path)` **đã** ĐỎ ở `--strict` trên bản `8670cdfe`
(lý do cũ: tệp đích không tới được từ crate root ⇒ mặc định **sản phẩm**). Bản vá chỉ **đổi
lý do** (nay phân giải được đích), không **thêm** báo oan. Đây là hành vi **có trước** KT-187,
không phải lỗi của bản vá — nhưng vẫn là **nợ** (xem §7.5).

### 1.4 `cfg_attr(not(test), path = concat!(…))`

```
### B1c CUR (chế độ thường)
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/lib.rs:122: `cfg_attr(…, path = …)` không phải literal chuỗi — không phân giải được đích (fail-closed, KT-187 B1)
rc=1
### B1c OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B1c rustc: path = concat!(…) có hợp lệ không?
error: malformed `path` attribute input
```

⇒ **ĐẠT** — fail-closed đúng; và đây là **phòng thủ**, không phải đường lọt (rustc từ chối
`path = concat!(…)`, khớp ghi chú L25 §2.1).

---

## 2. B2/B3 — `#[path]`/`include!` trong thân `macro_rules!`

### 2.1 B2 gốc (ngoặc nhọn)

Khung: macro định nghĩa ở `src/m.rs` (decoy sạch `src/evil.rs`), gọi ở `src/sub/s.rs`; tệp
thật `src/sub/evil.rs` là test-only (`#[cfg(test)] mod evil;` trong `src/sub.rs`) và có
`self.wake(t)`.

```
### B2 CUR (chế độ thường)
E E_STRICT_MACRO_INCLUDE   v2/kernel/vifast-core/src/m.rs:3: thân `macro_rules! evil_mod` chứa `#[path]` — đường dẫn giải theo **tệp gọi**, không phân giải tĩnh được (fail-closed, KT-187 B2)
# check-wake-paths: mode=v2 strict=0 errors=1 warnings=0
rc=1
### B2 CUR --strict
rc=1
### B2 OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B2 OLD 8670cdfe --strict
rc=0
### B2 cargo check -p vifast-core
warning: method `duong_san_pham` is never used
 --> vifast-core/src/sub/evil.rs:4:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.05s
rc=0
```

⇒ **ĐẠT** (đúng dạng ngoặc nhọn). `scan_kernel_macro_includes` chạy ở **mọi** chế độ
(`v2/tools/check-wake-paths.py:1757`), không còn trong `if strict:`.

### 2.2 B2 biến thể

| Biến thể | Kết quả | Căn cứ |
|---|---|---|
| macro định nghĩa ở `v2/lib`, gọi ở `v2/kernel` (ngoặc nhọn) | **ĐỎ** cả hai chế độ (`E_STRICT_MACRO_INCLUDE`, KT-177 H7) | [`b2-libmacro.log`](kt189-logs/b2-libmacro.log) |
| macro lồng trong macro (ngoặc nhọn) | **ĐỎ** (báo **hai** lần: macro ngoài + macro trong) | [`b2-nested.log`](kt189-logs/b2-nested.log) |
| macro ở `v2/sim` (ngoặc nhọn), gọi từ `v2/kernel` | **ĐỎ** cả hai chế độ | [`t8.log`](kt189-logs/t8.log) T8a |

### 2.3 B3 gốc (ngoặc nhọn)

```
### B3 CUR (chế độ thường)
E E_STRICT_MACRO_INCLUDE   v2/kernel/vifast-core/src/m.rs:3: thân `macro_rules! evil_mod` chứa `include!` — đường dẫn giải theo **tệp gọi**, không phân giải tĩnh được (fail-closed, KT-187 B3)
rc=1
### B3 OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B3 cargo check -p vifast-core
warning: method `duong_san_pham` is never used
 --> vifast-core/src/sub/evil.rs:4:8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.95s
rc=0
```

⇒ **ĐẠT** (đúng dạng ngoặc nhọn).

---

## 3. Dạng viết cách khác — **bộ lọc rẻ chưa chuẩn hoá**

Bản vá chuẩn hoá **regex** (`\binclude\s*!`, `#\s*\[\s*path`, `cfg_attr\s*\(…`) nhưng **quên**
bộ lọc rẻ đầu vòng — `v2/tools/check-wake-paths.py:759`:

```python
if "include!" not in text and "#[path" not in text and "cfg_attr" not in text:
    continue
```

Ba chuỗi này vẫn là **dạng chưa chuẩn hoá**. Tệp chỉ chứa `include ! (…)` / `#[ path = … ]` /
`# [ path = … ]` / `#[\npath = …]` **không** khớp cả ba ⇒ bị bỏ qua **trước khi** regex chạy.

### 3.1 Đích **ra ngoài cây** (phải ĐỎ nếu bắt được)

| Dạng | Checker | Ghi chú |
|---|---|---|
| `include!{…}` | **ĐỎ** | bắt được |
| `include![…]` | **ĐỎ** | bắt được |
| `include!(r"…")`, `include!(r#"…"#)` | **ĐỎ** | bắt được |
| `include! /*c*/ (…)` | **ĐỎ** | bắt được (chú thích bị khử) |
| `include ! (…)` | **XANH** | **lọt** — bộ lọc rẻ |
| `include ! { … }` | **XANH** | **lọt** — bộ lọc rẻ |
| `#[ path = … ]` | **XANH** | **lọt** — bộ lọc rẻ |
| `# [ path = … ]` | **XANH** | **lọt** — bộ lọc rẻ |
| `#[\npath = …]` | **XANH** | **lọt** — bộ lọc rẻ |
| `#[path="…"]`, `#[path = r"…"]` | **ĐỎ** | bắt được |
| `#[cfg_attr(not(test),path="…")]` (không dấu cách) | **ĐỎ** | bắt được |
| `#[cfg_attr(not(test), path = r"…")]` | **ĐỎ** | bắt được |
| `#[cfg_attr(not(test), path = "…", allow(dead_code))]` | **ĐỎ** | bắt được |
| `#[cfg_attr(not(test), cfg_attr(unix, path = "…"))]` | **ĐỎ** | bắt được (lồng) |
| `#[cfg_attr(not(test),\n path = "…")]` | **ĐẠT** | bắt được (xuống dòng) |
| `#[path = concat!(…)]`, `include!(concat!(…))` | **ĐỎ** | fail-closed |

Căn cứ: [`t4b.log`](kt189-logs/t4b.log).

### 3.2 Nguyên nhân — đối chứng

Thêm **một** `cfg_attr` vô hại vào cùng tệp ⇒ bộ lọc mở ⇒ regex chuẩn hoá **bắt được** ngay:

```
### P1 include ! (…) một mình
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### P2 include ! (…) + cfg_attr vô hại
E E_STRICT_WAKE_INCLUDE   … `include!` trỏ ra ngoài … (fail-closed, KT-171 B6/B7)
rc=1
### P3 #[ path = … ] một mình
rc=0
### P4 #[ path = … ] + cfg_attr vô hại
E E_STRICT_WAKE_INCLUDE   … `#[path]` trỏ ra ngoài … (fail-closed, KT-171 B6/B7)
rc=1
### P5 #[\npath = …] một mình
rc=0
### P6 #[\npath = …] + cfg_attr vô hại
E E_STRICT_WAKE_INCLUDE   … `#[path]` trỏ ra ngoài … (fail-closed, KT-171 B6/B7)
rc=1
```

Căn cứ: [`t4c.log`](kt189-logs/t4c.log).

### 3.3 Mức độ: **lỗ checker**, không phải lỗ cổng đầy đủ

`rustfmt` **chuẩn hoá** cả bốn dạng (đo bằng `cargo fmt --all --check` trên tệp mẫu):

```
-include ! ("a.rs");          +include!("a.rs");
-include ! { "b.rs" }         +include! { "b.rs" }
-include!{ "c.rs" }           +include! { "c.rs" }
-include![ "d.rs" ];          +include!["d.rs"];
-#[ path = "e.rs" ] mod e;    +#[path = "e.rs"]\nmod e;
-# [ path = "f.rs" ] mod f;   +#[path = "f.rs"]\nmod f;
-#[\npath = "g.rs"\n] mod g;  +#[path = "g.rs"]\nmod g;
-#[cfg_attr(not(test),path="h.rs")] mod h;  +#[cfg_attr(not(test), path = "h.rs")]\nmod h;
```

Vì `v2 check` chạy `fmt` **trước** checker (`v2/bin/v2:246` rồi `:302`), cổng đầy đủ vẫn bắt
được — **nhưng** bản vá tự nhận "chuẩn hoá regex" mà bộ lọc rẻ vẫn lệch, nên **chạy checker
trực tiếp** (như `v2/bin/v2:302` nếu fmt bị bỏ qua, hoặc trong selftest) vẫn lọt. Đáng vá cùng
lượt vì cùng gốc.

---

## 4. Báo oan

| Tình huống | CUR thường | CUR `--strict` | OLD `--strict` | Đánh giá |
|---|---|---|---|---|
| `cfg_attr(test, path = "…")` (chỉ bật khi test) | XANH | **ĐỎ** | **ĐỎ** | **không** phải báo oan **mới** — hành vi có trước |
| `include!` trong `#[cfg(test)]` của tệp sản phẩm, đích trong cây có `wake` | XANH | **ĐỎ** | **ĐỎ** | như trên — có trước |
| `#[path]` trong `#[cfg(test)]` của tệp sản phẩm, đích trong cây có `wake` | XANH | **ĐỎ** | **ĐỎ** | như trên — có trước |
| `include!(concat!(env!("OUT_DIR"), …))` trong `build.rs` | **ĐỎ** | **ĐỎ** | — | **báo oan tiềm ẩn** (L25 §4 đã ghi); mẫu này còn **không biên dịch được** trong `build.rs` (`env!("OUT_DIR")` không có ở compile-time của build script) |
| `include!(concat!(env!("OUT_DIR"), …))` trong mã sản phẩm | **ĐỎ** | **ĐỎ** | — | fail-closed đúng chữ D2-161, nhưng **cấm mẫu sinh mã hợp lệ** — nợ chính sách (L25 §7.4) |

Căn cứ: [`t4.log`](kt189-logs/t4.log) T5a/T5b/T5c, [`t8.log`](kt189-logs/t8.log) T8d,
[`t7.log`](kt189-logs/t7.log) T7a/T7b.

⇒ **Không tìm được báo oan mới** do bản vá sinh ra. Hai dòng `build.rs`/`OUT_DIR` là nợ **có
trước**, không phải hồi quy của KT-187.

---

## 5. Selftest 67 ca — "răng" của 4ba/4bb/4bc

Dựng lại **độc lập** ([`t6-teeth.py`](kt189-logs/t6-teeth.py)): dùng chính `_fixture_v2` của
checker để dựng cây, tự dựng lại đột biến, rồi **bỏ đúng cạnh đột biến** và **thêm lại**.

| Ca | Có đột biến | Bỏ cạnh đột biến | Kết luận |
|---|---|---|---|
| 4ba `#[cfg_attr(not(test), path = "evil_b.rs")]` → tệp test-only | **ĐỎ** (`E_STRICT_TEST_ONLY_WAKE`) | **XANH** (`errors=0`) | **có thật** |
| 4bb `#[path]` trong thân macro | **ĐỎ** (`E_STRICT_MACRO_INCLUDE` + `E_STRICT_WAKE_INCLUDE`) | **XANH** | **có thật** |
| 4bc `include!` trong thân macro | **ĐỎ** (`E_STRICT_MACRO_INCLUDE` + `E_STRICT_WAKE_INCLUDE`) | **XANH** | **có thật** |

```
ok - 4ba có đột biến: errors=1 hit=True
ok - 4bb có đột biến: errors=2 hit=True
ok - 4bc có đột biến: errors=2 hit=True
# cả ba ca "BỎ cạnh đột biến" đều errors=0
TEETH: PASS
```

`--selftest` của bản vá: **67/67 ok**; bản `8670cdfe`: **64/64 ok**. Không ca nào
`#[ignore]`/`#[should_panic]`; runner đếm đủ `len(cases)`.

**Nhưng**: ba ca mới chỉ phủ **đúng** dạng L25 đã dựng — macro **ngoặc nhọn**, `cfg_attr`
**ngoài** macro. Chúng **không** phủ bốn đường lọt ở §6, nên "67/67" không chứng minh luật
B2/B3 đã đóng.

---

## 6. Đường vòng mới — **bốn đường lọt thật, đều qua cả ba cổng**

Script: [`t2-b2b3.sh`](kt189-logs/t2-b2b3.sh), [`t3-more.sh`](kt189-logs/t3-more.sh),
[`t8-final.sh`](kt189-logs/t8-final.sh). Log tổng hợp: [`gate-level.log`](kt189-logs/gate-level.log).

### 6.1 **B2c — `macro_rules!` ngoặc TRÒN/VUÔNG + `#[path]`** (nặng nhất)

```rust
// v2/kernel/vifast-core/src/m.rs (sản phẩm)
#[macro_export]
macro_rules! evil_paren (
    () => (
        #[path = "evil.rs"] mod x;
    );
);
// src/evil.rs = decoy sạch; src/sub/s.rs gọi crate::evil_paren!();
// src/sub/evil.rs = test-only, có self.wake(t)
```

```
### B2c CUR (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B2c CUR --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
### B2c OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B2c cargo fmt --all --check
fmt rc=0
### B2c cargo clippy -p vifast-core -- -D warnings
clippy rc=0
### B2c cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s
```

**Cơ chế**: `scan_kernel_macro_includes` (dòng **955**) và `scan_lib_macros` (dòng **886**)
tìm thân macro bằng:

```python
j = m.end()
while j < len(code) and code[j] not in "{;":
    j += 1
if j >= len(code) or code[j] != "{":
    continue          # ← macro ngoặc tròn/vuông bị BỎ QUA
```

Với `macro_rules! evil_paren ( … );` **không có `{`** trước dấu `;` đầu tiên ⇒ `code[j] == ';'`
⇒ `continue` ⇒ **không quét thân**. Trong khi `resolve_include_targets` vẫn thấy `#[path]`
trong tệp **định nghĩa** và phân giải theo tệp định nghĩa (decoy sạch) ⇒ không lỗi. rustc giải
theo **tệp gọi** ⇒ nạp `sub/evil.rs` (test-only, có `wake`).

Biến thể **ngoặc vuông** `macro_rules! evil_br [ () => [ #[path = "evil.rs"] mod x; ]; ];` —
**cùng kết quả** (XANH cả hai chế độ, `fmt rc=0`, `cargo check` RC=0, cảnh báo chỉ
`sub/evil.rs:4:8`): [`t3.log`](kt189-logs/t3.log) T3a.

### 6.2 **B3a — `macro_rules!` ngoặc TRÒN/VUÔNG + `include!`**

```rust
#[macro_export]
macro_rules! evil_inc_paren (
    () => (
        include!("evil.rs");
    );
);
```

```
### B3a CUR (chế độ thường)   # và --strict
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B3a OLD 8670cdfe (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B3a cargo fmt --all --check
fmt rc=0
### B3a cargo clippy -p vifast-core -- -D warnings
clippy rc=0
### B3a cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.64s
```

Cùng gốc §6.1. Ngoặc vuông: [`t3.log`](kt189-logs/t3.log) T3b — cùng kết quả.

### 6.3 **B2d — `#[cfg_attr(not(test), path = "…")]` TRONG thân macro** (B1+B2)

```rust
#[macro_export]
macro_rules! evil_mod {
    () => {
        #[cfg_attr(not(test), path = "evil.rs")]
        mod x;
    };
}
```

```
### m.rs sau fmt:
#[macro_export]
macro_rules! evil_mod {
    () => {
        #[cfg_attr(not(test), path = "evil.rs")]
        mod x;
    };
}
### B2d CUR (chế độ thường)   # và --strict
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### B2d cargo fmt --all --check
fmt rc=0
### B2d cargo clippy -p vifast-core -- -D warnings
clippy rc=0
### B2d cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.71s
```

(`cargo check` không còn cảnh báo `dead_code` vì cây đã có call site sản phẩm gọi
`duong_san_pham`; đối chứng dương §6.5 chứng minh tệp **thật sự** được biên dịch.)

**Cơ chế**: `scan_kernel_macro_includes` chỉ tìm `#\s*\[\s*path\b` (dòng **980**) — `#[cfg_attr(`
**không** khớp. `resolve_include_targets` thấy `cfg_attr(…, path = "evil.rs")` trong tệp **định
nghĩa** macro, phân giải theo tệp định nghĩa (decoy sạch) ⇒ không lỗi. rustc mở `cfg_attr` khi
`not(test)` tại **tệp gọi** ⇒ nạp `sub/evil.rs` (test-only, có `wake`).

Biến thể **macro định nghĩa ở `v2/lib`** (gọi từ `v2/kernel`) — cùng kết quả XANH cả hai chế
độ, `cargo check` RC=0, cảnh báo chỉ `sub/evil.rs:4:8` ([`t8.log`](kt189-logs/t8.log) T8c):
`scan_lib_macros` cũng chỉ tìm `#\s*\[\s*path\b` (dòng **921**).

### 6.4 **T3c — `mod` khai trong macro + lời gọi `#[cfg(not(test))]`**

```rust
// src/m.rs
#[macro_export]
macro_rules! evil_decl { () => { mod x; }; }
// src/sub.rs
#[cfg(test)]
mod x;                       // đồ thị module: sub/x.rs là TEST-ONLY
#[cfg(not(test))]
crate::evil_decl!();         // bản sản phẩm: mở ra `mod x;` ⇒ sub/x.rs là SẢN PHẨM
// src/sub/x.rs = self.wake(t)
```

```
### T3c CUR (chế độ thường)   # và --strict
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
### T3c cargo fmt --all --check
fmt rc=0
### T3c cargo clippy -p vifast-core -- -D warnings
clippy rc=0
### T3c cargo check -p vifast-core
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.84s
```

**Cơ chế**: `_mods_of` (dòng 576) chỉ đọc `mod NAME;` ở cấp dòng — **không** thấy `mod` trong
thân macro. `scan_kernel_macro_includes` chỉ tìm `include!`/`#[path]` — **không** tìm `mod`.
Đồ thị module thấy `#[cfg(test)] mod x;` ⇒ `sub/x.rs` test-only ⇒ **không quét**. Nhưng lời gọi
`#[cfg(not(test))] evil_decl!()` mở ra `mod x;` ở bản sản phẩm ⇒ `sub/x.rs` được biên dịch là
**sản phẩm**.

Đây **phản chứng** trực tiếp ghi chú L25 §5.4 ("`mod` khai trong macro … **không** lọt"): L25
chỉ xét trường hợp **không** có `#[cfg(test)] mod` cùng tên; khi có (và lời gọi macro được
`#[cfg(not(test))]`), đồ thị module **lệch** với bản sản phẩm.

### 6.5 Đối chứng dương — chứng minh bản sản phẩm **thật sự** biên dịch `wake`

Trên cây B2d, đổi `self.wake(t)` → `self.khong_ton_tai(t)` trong `sub/evil.rs`:

```
error[E0599]: no method named `khong_ton_tai` found for mutable reference `&mut Machine` in the current scope
 --> vifast-core/src/sub/evil.rs:5:22
error: could not compile `vifast-core` (lib) due to 1 previous error
```

Khôi phục ⇒ `Finished`. Tức `sub/evil.rs` **được biên dịch** trong bản sản phẩm, và lời gọi
`wake` là **thật**, không phải cảnh báo chết.

### 6.6 Cả ba cổng đều xanh trên bốn cây lọt

Chạy **đúng lệnh của cổng** trên bốn cây (đã chép thêm `spec/` + `v2/docs/` để clippy chạy
được như cây thật):

```
======== b2/cfgattr-macro   (B2d)
checker thường: errors=0 · --strict: errors=0
cargo fmt --all --check --manifest-path v2/Cargo.toml        → fmt rc=0
cargo clippy --manifest-path v2/Cargo.toml --workspace --all-targets -- -D warnings → rc=0
======== b2/paren           (B2c)   — như trên
======== b2/inc-paren       (B3a)   — như trên
======== b3/mod-macro       (T3c)   — như trên
```

Căn cứ: [`gate-level.log`](kt189-logs/gate-level.log),
[`b2d-gate-clippy.txt`](kt189-logs/b2d-gate-clippy.txt) (`clippy rc=0` — **đúng** lệnh
`v2/bin/v2:247`), [`b2c-gate-clippy.txt`](kt189-logs/b2-paren-gate-clippy.txt),
[`b3a-gate-clippy.txt`](kt189-logs/b2-inc-paren-gate-clippy.txt),
[`t3c-gate-clippy.txt`](kt189-logs/b3-mod-macro-gate-clippy.txt).

Đây là **đường lọt cổng đầy đủ**, không phải lỗ checker thuần như §3.

**Lưu ý về `--all-targets` của riêng `vifast-core`**: `cargo clippy -p vifast-core
--all-targets` (siết hơn cổng — cổng chỉ lint workspace `v2`, `vifast-core` là **path
dependency** của `vifast-sim` nên chỉ được **build**, không bị lint) **đỏ** trên ba fixture
B2c/B3a/B2d vì lý do **fixture** (bản test nạp trùng tệp / `dead_code`), không phải vì bản vá.
T3c thì `-p vifast-core --all-targets` **xanh** cả hai bản. Tiêu chí D2-144 là **bản sản phẩm**
— và bản sản phẩm biên dịch `wake` ở cả bốn.

### 6.7 Các đường khác — **không** lọt

| Đường | Kết quả | Ghi chú |
|---|---|---|
| `include!` chuỗi qua tệp **không phải `.rs`** (`evil.inc` → `evil_t.rs`) | **ĐỎ** (`--strict`) | worklist của `resolve_include_targets` **có** đi theo đích không-`.rs`; không phải đường lọt |
| `include!` đường dẫn **tuyệt đối** (`/tmp/…`) | **ĐỎ** | `_inside_scan_tree` = None |
| `concat_idents!(wa, ke)` | XANH nhưng **không biên dịch** | macro không tồn tại trên stable ⇒ không phải đường lọt |
| `some_macro!(wake)` (macro lạ) | **ĐỎ** (`--strict`) | `E_STRICT_TEST_ONLY_WAKE` |
| `build.rs` sinh `include!` trong `OUT_DIR` | **ĐỎ** | non-literal ⇒ fail-closed (đồng thời là báo oan chính sách, §4) |
| `#[path]` trên `mod` inline | — | Rust từ chối; không phải đường lọt |
| macro ngoặc nhọn ở `v2/lib`/`v2/sim` | **ĐỎ** | §2.2 |

---

## 7. Đề xuất vòng vá (cho Agent 0 / task kế)

### 7.1 Sửa scanner thân macro — **bắt buộc** (đóng B2c/B3a)

`scan_lib_macros` (dòng 886) và `scan_kernel_macro_includes` (dòng 955) phải nhận **cả ba**
dấu mở, không chỉ `{`:

```python
j = m.end()
while j < len(code) and code[j] not in "([{;":
    j += 1
if j >= len(code) or code[j] not in "([{":
    continue
open_ch = code[j]
close_ch = {"(": ")", "[": "]", "{": "}"}[open_ch]
# khớp đúng cặp open_ch/close_ch (đếm lồng theo cả ba loại ngoặc)
```

Hoặc rẻ hơn: sau `macro_rules! NAME`, lấy **cửa sổ** tới `;`/hết tệp rồi quét
`include!`/`#[path]`/`cfg_attr(path)` trong cửa sổ đó (fail-closed, không cần khớp cặp chính
xác — chỉ cần **không bỏ sót**).

### 7.2 Thêm `cfg_attr(…, path = …)` vào quét thân macro — **bắt buộc** (đóng B2d)

Trong **cả** `scan_lib_macros` (dòng 921) và `scan_kernel_macro_includes` (dòng 980), thay
`#\s*\[\s*path\b` bằng **hai** mẫu:

```python
for pm in re.finditer(r"#\s*\[\s*path\b", body): …          # giữ nguyên
for pm in re.finditer(r"cfg_attr\s*\([^;{}]*?\bpath\s*=", body): …  # THÊM
```

(hoặc dùng lại `RE_CFG_ATTR_PATH`). Cùng lý do fail-closed: `cfg_attr` mở `path` theo **cfg của
tệp gọi**, không phân giải tĩnh được.

### 7.3 `mod` trong thân macro — **bắt buộc hoặc quyết định** (đóng T3c)

Thêm luật `E_STRICT_MACRO_MOD`: `mod NAME;` trong thân `macro_rules!` của
`v2/kernel`+`v2/sim`+`v2/lib` ⇒ **ĐỎ** (cùng lý do "giải theo tệp gọi"). Nếu không muốn cấm
tuyệt đối, tối thiểu phải **hợp nhất** đồ thị module với mọi điểm gọi macro — nhưng điểm gọi
có thể nằm ở crate khác, nên **fail-closed là lựa chọn đúng** (rẻ, đúng tinh thần D2-161).

### 7.4 Chuẩn hoá **bộ lọc rẻ** — **nên** (đóng §3)

`v2/tools/check-wake-paths.py:759`:

```python
if (
    not re.search(r"\binclude\s*!", text)
    and not re.search(r"#\s*\[\s*path", text)
    and "cfg_attr" not in text
):
    continue
```

### 7.5 Selftest — **bắt buộc** cùng lượt

Thêm ca cho **đúng** bốn đường lọt ở §6 (mỗi ca kèm đối chứng dương "bỏ đột biến ⇒ xanh"):

- (4bd) `macro_rules!` **ngoặc tròn** + `#[path]` ⇒ ĐỎ;
- (4be) `macro_rules!` **ngoặc vuông** + `include!` ⇒ ĐỎ;
- (4bf) `cfg_attr(not(test), path = …)` **trong** thân macro ⇒ ĐỎ;
- (4bg) `mod` trong macro + lời gọi `#[cfg(not(test))]` ⇒ ĐỎ;
- (4bh) `include ! (…)` / `#[ path = … ]` **một mình** (không kèm `cfg_attr`) ⇒ ĐỎ — ca này
  bắt đúng lỗi bộ lọc rẻ §3.

### 7.6 Nợ chính sách (cần Agent 0) — giữ nguyên từ L25 §7.4

- `build.rs` dưới `v2/kernel`: miễn luật literal hay vẫn áp?
- `#[cfg(test)]` include trỏ **ra ngoài** cây: giữ ĐỎ hay miễn cho tệp test-only?
- `include!(concat!(env!("OUT_DIR"), …))` trong mã sản phẩm: cấm tuyệt đối (hiện tại) hay mở
  ngoại lệ có kiểm soát?
- H4 có cần ĐỎ ở **chế độ thường** không? (Hiện chỉ `--strict`.)

---

## 8. Chưa review / giới hạn

- **Không** chạy `gate --fast` (theo đề). Chỉ chạy `check-wake-paths.py` (thường + `--strict`
  + `--selftest`), `cargo fmt --all --check`, `cargo clippy -p vifast-core -- -D warnings`,
  `cargo check -p vifast-core` trên **bản sao cây**.
- **Không** kiểm Miri/L1/QEMU — ngoài phạm vi KT-189.
- **Không** kiểm nhánh `--json` của checker.
- **Chưa** thử: `macro_rules!` tên raw ident (`r#evil`); `#[path]` trên `mod` trong **inline
  module** (`mod a { #[path] mod b; }` — Rust có luật thư mục khác, đáng thử vòng sau);
  `paste!` (không có trong allowlist phụ thuộc); `build.rs` sinh mã **trong cây** (đã kiểm
  gián tiếp qua worklist `.inc`).
- **Không** sửa mã sản phẩm; mọi đột biến trong `/tmp/kt189-l28`; cây worktree sạch trước
  commit (chỉ thêm `v2/agent/JOURNAL/kt189-*`).

---

## 9. Tệp bằng chứng

| Tệp | Nội dung |
|---|---|
| [`kt189-logs/t1-b1.sh`](kt189-logs/t1-b1.sh) + `b1-orig.log`, `b1-allcfg.log`, `b1-testcfg.log`, `b1-concat.log` | B1 gốc + ba biến thể |
| [`kt189-logs/t2-b2b3.sh`](kt189-logs/t2-b2b3.sh) + `b2-orig.log`, `b3-orig.log`, `b2-libmacro.log`, `b2-nested.log`, `b2-paren.log`, `b2-cfgattr-macro.log`, `b3-paren.log` | B2/B3 gốc + biến thể + **ba đường lọt** |
| [`kt189-logs/t3-more.sh`](kt189-logs/t3-more.sh) + `t3.log` | ngoặc vuông, `mod` trong macro, không decoy |
| [`kt189-logs/t4-spellings.sh`](kt189-logs/t4-spellings.sh) + `t4.log` | báo oan (T5a/T5b/T5c) |
| [`kt189-logs/t4b-spellings.sh`](kt189-logs/t4b-spellings.sh) + `t4b.log` | dạng viết cách khác, đích **ngoài cây** |
| [`kt189-logs/t4c-prefilter.sh`](kt189-logs/t4c-prefilter.sh) + `t4c.log` | **nguyên nhân**: bộ lọc rẻ dòng 759 |
| [`kt189-logs/t6-teeth.py`](kt189-logs/t6-teeth.py) + `t6.log` | "răng" 4ba/4bb/4bc |
| [`kt189-logs/t7-bypass.sh`](kt189-logs/t7-bypass.sh) + `t7.log` | build.rs, tuyệt đối, chuỗi `.inc`, `concat_idents!` |
| [`kt189-logs/t8-final.sh`](kt189-logs/t8-final.sh) + `t8.log` | macro `v2/sim`, macro `v2/lib` + `cfg_attr`, báo oan |
| [`kt189-logs/gate-level.log`](kt189-logs/gate-level.log) | **bốn cây lọt**: checker thường/`--strict` + `fmt` + `clippy -D warnings` đều xanh |
| `kt189-logs/b2d-gate-clippy.txt`, `b2-paren-gate-clippy.txt`, `b2-inc-paren-gate-clippy.txt`, `b3-mod-macro-gate-clippy.txt` | **đúng lệnh clippy của cổng** (`--manifest-path v2/Cargo.toml --workspace --all-targets -- -D warnings`) ⇒ `rc=0` trên bốn cây lọt |
| `kt189-logs/b2c-clippy.txt`, `b2d-clippy.txt`, `b2-inc-paren-clippy.txt`, `b3-mod-macro-clippy.txt` | `clippy -p vifast-core -- -D warnings` (bản sản phẩm) rc=0 trên bốn cây lọt |
| [`kt189-logs/lib.sh`](kt189-logs/lib.sh) | harness chung (dựng cây, chạy checker) |
