# KT-211 — vá **đường lọt SỐNG thứ mười** của luật `wake`/`include!`/`#[path]`: quét token toàn cục

**Tác giả**: L46 (sub-agent của Agent 1) · **Nhánh**: `night3/L46-kt211` · **Gốc**: `74e2b2c9`
**Checker**: `v2/tools/check-wake-paths.py` · **Hợp đồng**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md`
**Reviewer dựng đường lọt**: L44 (KT-208, `v2/agent/JOURNAL/kt208-review202.md` §B1/§D)

---

## 0. KẾT LUẬN NGẮN

**ĐẠT.** Đường lọt L44 dựng (`include!` ở **vị trí biểu thức** trong `( … )`/`[ … ]`) **đã đóng**
bằng **quét token toàn cục** — không còn bản vá theo cấu trúc. Sáu biến thể ⇒ **6/6 ĐỎ** cả hai
chế độ; repro L40 (KT-202) ⇒ vẫn **ĐỎ**; repro L44 ⇒ **ĐỎ** cả hai chế độ; đối chứng dương
(`include_str!`, `include_bytes!`, `stringify!(include)`, `include!` hợp lệ trong cây, `#[path]`
hợp lệ) ⇒ **XANH**. Selftest **157 → 175** (18 ca mới, 11 ĐỎ + 7 đối chứng dương). `v2 check`
**PASS**; `--strict` `errors=0` trên cây thật.

**Phát hiện thêm (cùng lớp, chưa ai nêu)**: `#[path]` trên `mod` trong **khối lồng `( … )`**
(`({ #[path = "…"] mod m; 1 })`) cũng lọt — `_walk_items` không thấy `{ … }` bên trong `( … )`.
Đã đóng bằng cùng cơ chế quét toàn cục (ca B28).

---

## 1. NGUYÊN NHÂN GỐC (reviewer L44 đo trực tiếp; tôi tái lập)

Hai dòng cộng lại thành lỗ:

1. `_walk_items` (đường cấu trúc) **chỉ đệ quy nhóm `{ … }`**:
   ```python
   if node.open.val == "{":
       _walk_items(node.children, ...)
   i += 1
   ```
   ⇒ `include!` nằm trong `( … )`/`[ … ]` **không bao giờ** được `on_include` gọi ⇒
   `resolve_include_targets` không thấy cạnh ⇒ đích **không** bị quét, **không** bị từ chối.
2. `_scan_macro_calls` (KT-202) chỉ gọi `_check_macro_passthrough` cho **nhóm đứng sau `!`**:
   ```python
   if _is_punct(prev, "!"):
       _check_macro_passthrough(relp, "lời gọi macro", n.children, r)
   ```
   ⇒ với `include! { "…" }`, nhóm `{ "…" }` đứng sau `!` nên hàm chỉ kiểm **chuỗi bên trong**
   (không có token `include`), rồi **không đệ quy tiếp**. Token `include` **đứng trước** `!`
   không được kiểm ở đâu cả.

**Tái lập trước khi sửa** (cây thật, `v2/lib/evil_expr.rs` = `m.wake(t)`,
`v2/kernel/vifast-core/src/kt211_attack.rs` = `kt211_sink(include! {"../../../lib/evil_expr.rs"});`
+ `mod kt211_attack;` trong `lib.rs`):

| Lệnh | Kết quả |
|---|---|
| `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core` | **RC=0** |
| `python3 v2/tools/check-wake-paths.py` | **RC=0**, `errors=0` — **XANH** |
| `python3 v2/tools/check-wake-paths.py --strict` | **RC=0**, `errors=0` — **XANH** |

Log: [`kt211-logs/04-repro-L44-before-normal.log`](kt211-logs/04-repro-L44-before-normal.log),
[`04-repro-L44-before-strict.log`](kt211-logs/04-repro-L44-before-strict.log).

**Đối chứng âm** (chứng minh `include!` được **mở thật**): đổi `evil_expr.rs` thành
`m.kt211_khong_ton_tai(t)` ⇒ `cargo check` **RC=101** (`E0599`); khôi phục ⇒ **RC=0**. Đây là
hành vi đã được reviewer L44 ghi ở `kt208-logs/B01-clean-negative-control.log`; tôi không lặp
lại (cùng cây, cùng tệp).

---

## 2. CÁCH VÁ — **quét token toàn cục**, không theo cấu trúc

Đây là lần thứ **mười** luật này bị phá; mọi bản vá theo cấu trúc đều bỏ sót một vị trí token.
Vì vậy:

### 2.1. Hàm mới `_walk_all` (thay `on_include`/`on_attr` trong `resolve_include_targets`)

Đệ quy **mọi** nhóm (`{`/`(`/`[`), **mọi** tầng; với mỗi token:

- `include` + `!` ⇒ gọi `on_include(nodes, j, tok)`:
  - đối số là nhóm `{…}`/`(…)`/`[…]` chứa **đúng một** literal chuỗi ⇒ **cạnh** (phân giải theo
    **thư mục tệp chứa** — đúng rustc);
  - đối số là literal chuỗi trần (phòng thủ) ⇒ **cạnh**;
  - mọi dạng khác (kể cả **không có** đối số) ⇒ **ĐỎ** `E_STRICT_WAKE_INCLUDE` (fail-closed).
- `#` + `[` group ⇒ `parse_attr` + `attr_path_edges` ⇒ **cạnh** `#[path]`/`#[cfg_attr(path)]`
  (phân giải theo **thư mục module** `cur_dir`).
- `macro_rules` + `!` + NAME + Group ⇒ **bỏ qua thân** (đã bị `_check_macro_body` từ chối riêng
  — tránh báo trùng và tránh phân giải sai theo tệp định nghĩa).
- `mod` + NAME + `{ … }` (inline module) ⇒ đệ quy thân với `cur_dir` **đổi theo `#[path]`**
  (đúng rustc — đã kiểm bằng `rustc`: `#[path = "sub"] mod m { #[path = "y.rs"] mod n; }` ⇒
  `sub/y.rs`); đánh dấu token `#[path]` vào `inline_path_toks` (để bỏ qua đích **thư mục**).
- `pub`/`pub(…)` ⇒ bỏ qua tầm nhìn, giữ `pending` (để `#[path]` gắn đúng `mod`).

`include!` phân giải theo **thư mục tệp chứa**, **không** theo `cur_dir` — đã kiểm bằng `rustc`:
`mod m { include!("x.rs") }` ⇒ `x.rs` **cạnh tệp gốc**, không phải `m/x.rs`.

### 2.2. `#[path]` — giữ đường cấu trúc **nhưng** kiểm lại: **có** lọt

Reviewer L44 chỉ nêu `include!`; tôi kiểm `#[path]` theo yêu cầu phiếu và **dựng được đường lọt**:

```rust
pub fn kt211_b28(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = ({
        #[path = "../../../lib/evil_item.rs"]   // đích có `m.wake(t)`
        mod evil_item;
        1u8
    });
}
```

`cargo check` **RC=0** (đã kiểm), checker **XANH** cả hai chế độ trước bản vá. Lý do: `_walk_items`
không đệ quy `( … )` ⇒ không thấy `{ … }` bên trong ⇒ không thấy `#[path]`. Nay quét toàn cục
phủ (ca B28). **Không** có vị trí `#[path]` hợp lệ nào khác bị bỏ sót: `#[path]` chỉ gắn được
vào item/`mod`, và mọi vị trí item hợp lệ đều nằm trong `{ … }` (đã đệ quy) hoặc trong `( … )`
(đã quét toàn cục).

### 2.3. `mod` — **giữ** đường cấu trúc (lý do)

`mod` chỉ có nghĩa ở **mức item**; mọi vị trí `mod` hợp lệ đều nằm trong `{ … }` mà `_walk_items`
đã đệ quy (thân hàm, thân inline module, khối biểu thức). Quét toàn cục cho `mod` sẽ **báo oan**
(`mod` là từ khoá, xuất hiện trong macro args/`stringify!`), và **không** đóng thêm được gì:
đường thoát khỏi cây bằng `mod` **bắt buộc** phải qua `#[path]` — đã bị quét toàn cục bắt. Đã
kiểm: `mod` trong đối số lời gọi macro vẫn bị `_check_macro_passthrough` bắt (`E_STRICT_MACRO_MOD`).

### 2.4. `_scan_macro_calls` — **giữ**, không dư thừa

Đã đo: xoá `_scan_macro_calls(tree, relp, r)` ⇒ selftest **174/175**, ca
`KT-202 \`mod\` trong lời gọi macro` **XANH**. Lý do: quét toàn cục **không** xử lý `mod` (mục
2.3), nên `pass!(mod evil;)` chỉ còn `_check_macro_passthrough` bắt. Nó **bắt được gì mà quét
toàn cục không bắt**: `mod` trong **đối số lời gọi macro** (`E_STRICT_MACRO_MOD`) — đồ thị module
không thấy module đó nên không xác định được test-only. Ngoài ra nó còn là lớp phòng thủ thứ hai
cho `include!`/`#[path]` trong đối số lời gọi macro (quét toàn cục đã phủ, nhưng hai lớp độc lập
là có chủ đích — fail-closed).

---

## 3. SELFTEST — thêm ca **TRƯỚC** khi sửa (hợp đồng §7)

**Trước**: **157** ca (`# selftest: 157/157 ok`, đo trên `74e2b2c9`).
**Sau**: **175** ca (`# selftest: 175/175 ok`) — **+18** ca (11 ĐỎ + 7 đối chứng dương).

| # | Ca | Kỳ vọng |
|---|---|---|
| B01 | `sink(include!{…})` — đối số lời gọi **hàm** trong `( … )` | ĐỎ |
| B02 | `[include!{…}]` — mảng `[ … ]` | ĐỎ |
| B03 | `(0u8, include!{…})` — tuple `( … )` | ĐỎ |
| B23 | `(include!{…})` — trần trong `( … )` | ĐỎ |
| B24 | `(|| include!{…})` — thân closure trong `( … )` | ĐỎ |
| B25 | `Some(include!{…})` — đối số hàm/biến thể | ĐỎ |
| B04 | đối chứng: `{ include!{…} }` — khối `{ … }` vẫn ĐỎ | ĐỎ |
| B05 | đối chứng: `include!` trần câu lệnh vẫn ĐỎ | ĐỎ |
| B26 | đối chứng dương: `include_str!` | XANH |
| B26 | đối chứng dương: `include_bytes!` | XANH |
| B26 | đối chứng dương: `stringify!(include)` | XANH |
| B26 | đối chứng dương: `include!` hợp lệ **trong cây**, trong `( … )` | XANH |
| B28 | `#[path]` trên `mod` trong khối lồng `( … )` — đích ngoài cây | ĐỎ |
| B28 | đối chứng dương: `#[path]` trên `mod` trong khối lồng `( … )` — đích trong cây | XANH |
| B27 | `include!` trong `( … )` ở `v2/lib` — đích ngoài `v2/lib` | ĐỎ |
| B27 | `include!` trong `( … )` ở `v2/sim` — đích ngoài cây | ĐỎ |
| B29 | `include!` không literal trong `( … )` — fail-closed | ĐỎ |
| B30 | `include!` trong `( … )` lồng 2 tầng nhóm | ĐỎ |

**Răng**: chạy 18 ca mới trên checker **CŨ** (`74e2b2c9`, ghép bằng script — chỉ chèn khối ca,
không chèn bản vá): **164/175**, đúng **11** ca ĐỎ kỳ vọng bị **XANH** (B01/B02/B03/B23/B24/B25/
B28/B27-lib/B27-sim/B29/B30). 7 đối chứng dương XANH trên **cả hai** checker (đúng — chúng
không phải ca bắt lỗi). Log:
[`kt211-logs/03-new-cases-on-old-checker.log`](kt211-logs/03-new-cases-on-old-checker.log).

---

## 4. ĐỘT BIẾN CHỨNG MINH (cây thật, `cargo check` + checker cả hai chế độ)

Mỗi ca: ghi tệp tấn công vào `v2/kernel/vifast-core/src/kt211_attack.rs` (+ `mod kt211_attack;`),
chạy `cargo check -p vifast-core`, rồi checker thường + `--strict`. Log từng ca trong
[`kt211-logs/`](kt211-logs/) (`B*-cargo.log`, `B*-normal.log`, `B*-strict.log`).

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| B01 | `sink(include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B02 | `[include!{…}]` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B03 | `(0u8, include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B23 | `(include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B24 | `(|| include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B25 | `Some(include!{…})` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B28 | `#[path]` trên `mod` trong khối lồng `( … )` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B29 | `include!` không literal trong `( … )` | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B30 | `include!` lồng 2 tầng nhóm | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| L40 | repro KT-202 (`:item` + `include!` trong lời gọi) | **0** | **ĐỎ** (3) | **ĐỎ** (3) |
| B27 | `include!` trong `( … )` ở `v2/lib` (đích ngoài `v2/lib`) | **0** | **ĐỎ** (1) | **ĐỎ** (1) |
| B27 | `include!` trong `( … )` ở `v2/sim` (đích ngoài cây) | **0** (`--lib`) | **ĐỎ** (1) | **ĐỎ** (1) |

**Đối chứng dương** (phải XANH — chứng minh bản vá không báo oan):

| # | Dạng | cargo | thường | strict |
|---|---|---|---|---|
| P1 | `include_str!("../../../lib/evil_expr.rs")` | **0** | **XANH** | **XANH** |
| P2 | `include_bytes!("../../../lib/evil_expr.rs")` | **0** | **XANH** | **XANH** |
| P3 | `stringify!(include)` | **0** | **XANH** | **XANH** |
| P4 | `include!` hợp lệ **trong cây**, trong `( … )` | **0** | **XANH** | **XANH** |
| P5 | `#[path]` hợp lệ **trong cây**, trong khối lồng `( … )` | **0** | **XANH** | **XANH** |

Tổng hợp: [`kt211-logs/05-mutation-summary.log`](kt211-logs/05-mutation-summary.log),
[`06-mutation-summary-fix-cargo.log`](kt211-logs/06-mutation-summary-fix-cargo.log).

**Ghi chú về `cargo`**: B28/L40 ban đầu **RC=101** vì tệp đích chứa **biểu thức** (`m.wake(t)`)
được chèn vào **vị trí item** (`mod`/`include!` mức item) — lỗi hình dạng tệp đích, **không**
phải lỗi checker. Đã sửa bằng tệp đích hình dạng **item** (`pub fn … { let _ = m.wake(t); }`) ⇒
**RC=0**. B27-sim cần `--lib` vì bin `vifast-sim` import `Harness` từ `lib.rs` bị thay thế.

---

## 5. KHÔNG HỒI QUY

| Lệnh | Kết quả |
|---|---|
| `python3 v2/tools/check-wake-paths.py --selftest` | **175/175 ok**, RC=0 |
| `python3 v2/tools/check-wake-paths.py` (cây thật) | RC=0, `errors=0` |
| `python3 v2/tools/check-wake-paths.py --strict` (cây thật) | RC=0, `errors=0` |
| `./v2/bin/v2 check` | **PASS** (mọi bước OK, gồm `wake paths`, `selftest wake paths`, `wake paths --strict`) |

Log: [`kt211-logs/00-selftest-after.log`](kt211-logs/00-selftest-after.log),
[`01-real-tree-normal.log`](kt211-logs/01-real-tree-normal.log),
[`02-real-tree-strict.log`](kt211-logs/02-real-tree-strict.log),
[`07-v2check.log`](kt211-logs/07-v2check.log).

**157 ca cũ**: không mất ca nào — 175 = 157 + 18; `grep -c '^ok - '` = **175**.

**Hiệu năng**: checker trên cây thật ~20,8 s (bản cũ ~19,7 s cùng máy đang tải) — **+5%**, chấp
nhận được; `_walk_all` chỉ thêm việc đệ quy `( … )`/`[ … ]` (trước đây bỏ qua).

---

## 6. HỢP ĐỒNG — 6 khẳng định reviewer L44 liệt kê (§D)

| # | Vị trí | Trước | Sau |
|---|---|---|---|
| 1 | §1 bảng `v2/kernel/**` → `wake` chỉ-dùng-test | **SAI** (chỉ 4 mẫu cứng) | **ĐÃ SỬA bởi KT-209** — kiểm lại: bảng ghi `v2/kernel/<crate>/src/**` + `build.rs`, khớp `RE_TEST_ONLY_WAKE_SCOPE`; **không** ghi đè |
| 2 | §1 "chỉ đúng khi cạnh phân giải được" | **THIẾU** — không nói **khi nào** | **ĐÃ THÊM** đoạn **KT-211**: cạnh phân giải được ở **mọi** vị trí token (quét toàn cục), **trừ** thân `macro_rules!` và **đối số lời gọi macro** (bị từ chối riêng) |
| 3 | §2 gạch đầu dòng KT-202 | **THIẾU** — ngụ ý `_scan_macro_calls` phủ `( … )` cho `include!` | **ĐÃ SỬA**: nói rõ `_scan_macro_calls` **chỉ** phủ **đối số lời gọi macro**; `include!` **trần** trong `( … )`/`[ … ]` do **quét toàn cục** phủ |
| 4 | §4 "Macro invocation … Đã vá" | **SAI (quá mức)** — tuyên bố đóng **lớp** | **ĐÃ HẠ**: "KT-202 đóng **một dạng**; KT-211 đóng **lớp**"; nêu 6 biến thể + `#[path]` trong khối lồng |
| 5 | §4 "`#[path]` trong `v2/lib`" | **THIẾU** `include!` | **ĐÃ SỬA bởi KT-209** — dòng hiện ghi "`#[path]`/`include!` trong `v2/lib`"; kiểm lại: B27-lib **ĐỎ**; **không** ghi đè |
| 6 | §4 "Báo oan có trước" | **THIẾU** 4 dạng | **ĐÃ THÊM**: `cfg_attr(test, path)`, `if let Some(wake)`, tham số closure `wake`, trường struct `wake` — đã đo lại trên checker mới: **cả 4 ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |

Ngoài ra: §3 thêm hai dòng (không literal / không có đối số — **mọi** vị trí token); §2 thêm
gạch đầu dòng **KT-211** (cơ chế quét toàn cục + lý do `mod` giữ cấu trúc); tiêu đề/trạng thái
cập nhật KT-211.

---

## 7. GIỚI HẠN CÒN LẠI (ghi rõ, **không** tuyên bố đã đóng)

1. **`mod` vẫn theo đường cấu trúc** — lý do §2.3. Đường thoát khỏi cây bằng `mod` phải qua
   `#[path]` (đã quét toàn cục). Nếu tương lai có dạng `mod` hợp lệ ngoài `{ … }` (Rust mới),
   phải mở lại.
2. **`_scan_macro_calls` giữ** — bắt `mod` trong đối số lời gọi macro; đã đo xoá ⇒ 174/175.
3. **`:expr` không bị từ chối** (KT-202, giữ nguyên) — lý do cây thật `macro_rules! bail` dùng
   `$pages:expr, $e:expr`; `include!` trong đối số lời gọi đã bị bắt ở tệp gọi.
4. **Proc-macro** (`paste!`, `concat_idents!`) — ngoài phạm vi, không có trong allowlist phụ thuộc.
5. **`#[cfg]` phức tạp** — chỉ hiểu `#[cfg(test)]`/`#[cfg(feature = "ktest")]`; `#[path]` trong
   `cfg_attr` **lồng** vẫn bị từ chối fail-closed (không đánh giá `cfg`).
6. **Báo oan có trước** (6 + 4 dạng, §4 hợp đồng) — không phải hồi quy; cây thật không có dạng nào.
   Ghi thêm: `stringify!(include!)` (**có** `!`) **ĐỎ** `E_STRICT_MACRO_INCLUDE` **từ trước**
   (đo trên checker cũ: `RED=True codes=['E_STRICT_MACRO_INCLUDE']`); `stringify!(include)`
   (**không** `!`) **XANH** — đúng đối chứng dương của phiếu. KT-211 thêm mã
   `E_STRICT_WAKE_INCLUDE` cho dạng có `!` (vì `include` + `!` không có đối số), **không** đổi
   phán quyết.
7. **`v2 check` chạy trên máy đang tải** (nhiều agent song song) — thời gian 2m43s; không phải
   hồi quy (cùng lệnh trên bản cũ cùng điều kiện tương đương).

---

## 8. TỆP ĐÃ SỬA

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | thêm `_walk_all` (quét token toàn cục); `resolve_include_targets` dùng `_walk_all` thay `_walk_items` cho `include!`/`#[path]`; thêm 18 ca selftest KT-211 |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | 6 khẳng định §D + §2/§3/tiêu đề |
| `v2/agent/JOURNAL/kt211-include-global.md` | tệp này |
| `v2/agent/JOURNAL/kt211-logs/**` | log đột biến + selftest + `v2 check` |

**Không** sửa: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`.
**Không** nới ngưỡng; **không** thêm allowlist.
