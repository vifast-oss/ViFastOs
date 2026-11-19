# KT-225 — Vá **bốn báo oan** `include!` + đường lọt lớp 1 `.txt` + răng ca B29/B30

**Tác giả**: L52 (chuyển từ reviewer KT-223). **Nhánh**: `night3/L52-kt223`, gốc `11b21eac`.
**Checker**: `v2/tools/check-wake-paths.py`. **Hợp đồng**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md`.
**Nguồn phát hiện**: [`kt223-review211.md`](kt223-review211.md) §3/§4/§5 (review độc lập KT-211).

---

## 0. KẾT LUẬN NGẮN

**ĐẠT.**

| Hạng mục | Trước KT-225 | Sau KT-225 |
|---|---|---|
| `stringify!(include!)` | **ĐỎ oan** | **XANH** |
| `include != 0` (biến) | **ĐỎ oan** | **XANH** |
| `self.include != 0` (trường) | **ĐỎ oan** | **XANH** |
| `include!("…",)` (trailing comma) | **ĐỎ oan** | **XANH** (đích trong cây) |
| Đường lọt `.txt` (macro body trong tệp không phải `.rs`) | lớp 1 **XANH** | lớp 1 **ĐỎ** (`E_STRICT_MACRO_INCLUDE`) |
| Ca B29 (non-literal) | không răng | **có răng** (assert lý do) |
| Ca B30 (lồng 2 tầng) | không răng | **có răng** (assert dòng) |
| Selftest | 246/246 | **259/259** |
| `v2 check` | PASS | **PASS** |

**Không** nới ngưỡng, **không** thêm allowlist, **không** xoá ca cũ (246 ca cũ giữ nguyên).

---

## 1. BỐN BÁO OAN (ưu tiên 1)

### 1.1. Nguyên nhân chung

`_walk_all` (KT-211) coi **mọi** cặp `include` + `!` là `include!`, bất kể sau `!` là gì:

- `include != 0` — lexer tách `!=` thành **hai** punct `!` + `=`; `_walk_all` thấy `include` + `!`
  rồi gọi `on_include` với token `=` ⇒ "không phải literal chuỗi" ⇒ **ĐỎ oan**.
- `stringify!(include!)` — `_walk_all` đệ quy vào **đối số** `stringify!` (token stream, không
  phải mã) và thấy `include` + `!` ⇒ **ĐỎ oan**; `_check_macro_passthrough` cũng báo
  `E_STRICT_MACRO_INCLUDE`.
- `include!("x",)` — `on_include` đòi nhóm có **đúng một** token literal; trailing comma là cú
  pháp Rust hợp lệ (đã kiểm bằng `rustc`) ⇒ **ĐỎ oan**.

### 1.2. Cách vá

| # | Vị trí | Thay đổi |
|---|---|---|
| 1 | `_walk_all` nhánh `include` | Chỉ gọi `on_include` khi sau `!` là **nhóm phân cách** `{`/`(`/`[` hoặc **literal chuỗi**. `!` + `=` ⇒ bỏ qua (không phải macro). |
| 2 | `_walk_all` tham số mới `in_macro_args` | Nhóm sau `!` là **đối số lời gọi macro** ⇒ không sinh cạnh `include!`/`#[path]`/`mod` từ token stream. `_check_macro_passthrough` xử lý riêng. |
| 3 | `_check_macro_passthrough` | Cùng luật cú pháp (sau `!` phải là nhóm/chuỗi) + **miễn** đối số `stringify!`/`concat!` (`SAFE_TOKEN_MACROS`). |
| 4 | `_scan_macro_calls` | Không gọi `_check_macro_passthrough` cho `stringify!`/`concat!`. |
| 5 | `on_include` | Nhận `include!("x",)` — nhóm `[str, ","]` ⇒ coi như `[str]`. |
| 6 | `_scan_macro_calls`/`_check_macro_passthrough` | Miễn `stringify!`/`concat!` **chỉ khi** tên **không** bị `macro_rules!` trong cây che (`_shadowed_safe_macros`, cùng lý do KT-177/G6): `macro_rules! stringify { ($x:expr) => { $x } }` + `stringify!(include!("…"))` vẫn **ĐỎ**. |

**Vì sao `stringify!`/`concat!` an toàn** (đã đo bằng `rustc`):
- `stringify!(include!("x"))` — `stringify!` **không** mở rộng đối số; `include!` chỉ là token.
- `concat!(include!("x"))` — `concat!` **có** mở rộng `include!`, nhưng kết quả **bắt buộc** là
  literal chuỗi ⇒ không thể chở `wake` (không phải mã).

### 1.3. Đối chứng âm (không nới lỏng)

| Dạng | Kết quả |
|---|---|
| `include!("../../../lib/evil_expr.rs")` | **ĐỎ** `E_STRICT_WAKE_INCLUDE` |
| `include!("../../../lib/evil_expr.rs",)` | **ĐỎ** `E_STRICT_WAKE_INCLUDE` |
| `pass!(include! {"…"})` (L40) | **ĐỎ** `E_STRICT_MACRO_INCLUDE` (+ `E_STRICT_MACRO_TT`) |
| `pass!(include!())` | **ĐỎ** `E_STRICT_MACRO_INCLUDE` |
| `pass!(include! {"…"})` lồng trong `( … )` | **ĐỎ** `E_STRICT_MACRO_INCLUDE` |
| `pass!(#[path = "…"] mod m;)` | **ĐỎ** `E_STRICT_MACRO_INCLUDE` + `E_STRICT_MACRO_MOD` |
| `pass!(mod evil;)` | **ĐỎ** `E_STRICT_MACRO_MOD` |

Log: [`01-fp-after.log`](kt225-logs/01-fp-after.log), [`02-macro-args-after.log`](kt225-logs/02-macro-args-after.log).

---

## 2. ĐƯỜNG LỌT `.txt` — chọn **(a)**

### 2.1. Vấn đề

`scan_macro_bodies` chỉ duyệt `base.rglob("*.rs")` cho `base ∈ MACRO_SCAN_ROOTS`. Một
`macro_rules!` định nghĩa trong tệp **không phải `.rs`** (ví dụ `src/macro_def.txt`) được
`include!("macro_def.txt")` ở vị trí item **không** bị `_check_macro_body` quét; `_walk_all` bỏ
qua thân macro ⇒ checker tĩnh **XANH cả hai chế độ** trong khi `cargo check` RC=0 và mã sản phẩm
gọi `wake` (lớp 2 `check-wake-depinfo.py` bắt được).

### 2.2. Cách vá — (a) mở `scan_macro_bodies` cho **mọi tệp** được `include!` vào cây

- `scan_macro_bodies(root, r, extra_files=None)` — tham số mới `extra_files`.
- `scan_v2` truyền `extra_files=[p for p, _ in extra.values()]` (mọi tệp được `include!`/`#[path]`
  vào cây, **không** chỉ `.rs`).
- Hàm con `scan_one(p)` dùng chung cho cả `rglob("*.rs")` và `extra_files`; `seen` chống quét
  trùng.
- `scan_one` chạy **cả hai** luật: token `wake` trong thân macro + `_check_macro_body`
  (`include!`/`#[path]`/`mod`/`:tt`), và `_scan_macro_calls` (đối số lời gọi macro).

**Chi phí**: cây thật hiện có **0** tệp `extra` (đo: `# tệp CHỈ tới được qua
#[path]/include!/macro: 0`), nên **không** tăng thời gian trên cây thật. Với cây có `extra`, chi
phí là parse thêm đúng các tệp đó.

### 2.3. Kết quả

| Lớp | Trước | Sau |
|---|---|---|
| `check-wake-paths.py` (thường) | **XANH** | **ĐỎ** — `E_STRICT_MACRO_INCLUDE` trên `macro_def.txt` |
| `check-wake-paths.py --strict` | **XANH** | **ĐỎ** — như trên |
| `check-wake-depinfo.py` | ĐỎ | ĐỎ (không đổi) |
| `cargo check -p vifast-core` | RC=0 | RC=0 (mã vẫn biên dịch — đúng là đường lọt thật) |

Log: [`10-batch-txt-normal.log`](kt225-logs/10-batch-txt-normal.log),
[`11-batch-txt-strict.log`](kt225-logs/11-batch-txt-strict.log),
[`12-batch-txt-cargo.log`](kt225-logs/12-batch-txt-cargo.log),
[`13-batch-txt-depinfo.log`](kt225-logs/13-batch-txt-depinfo.log).

---

## 3. RĂNG CA B29/B30

### 3.1. Vấn đề

`_kt211_case` chỉ assert `any("E_STRICT_WAKE_INCLUDE" in e)`. B29 (non-literal) và B30 (lồng 2
tầng) khi bị **phá fixture** (đổi `concat!` → literal ra ngoài; bỏ lồng) vẫn cho **cùng mã lỗi**
⇒ ca vẫn PASS ⇒ **không răng** cho tuyên bố cụ thể.

### 3.2. Cách vá

`_kt211_case` nhận thêm:
- `msg_contains: Optional[str]` — thông điệp lỗi phải chứa chuỗi này;
- `line: Optional[int]` — lỗi phải ở đúng dòng này;
- `code: str = "E_STRICT_WAKE_INCLUDE"` — mã lỗi cần kiểm (dùng cho ca `.txt`).

B29 dùng `msg_contains="không phải literal chuỗi"`; B30 dùng `line=3` (fixture đổi để
`include!` lồng nằm **dòng 3**).

### 3.3. Chứng minh có răng

| Ca | Gốc | Phá fixture | Kết quả |
|---|---|---|---|
| B29 | `include! {concat!(…)}` ⇒ PASS | đổi `concat!` → literal ra ngoài ⇒ thông điệp "trỏ ra ngoài" | **FAIL** (mất răng nếu không assert lý do) |
| B30 | `include!` lồng ở dòng 3 ⇒ PASS | bỏ lồng, `include!` về dòng 2 | **FAIL** (mất răng nếu không assert dòng) |

Log: [`03-teeth-after.log`](kt225-logs/03-teeth-after.log).

---

## 4. SELFTEST 246 → 259

**13 ca mới** (giữ nguyên 246 ca cũ):

| # | Ca | Kỳ vọng |
|---|---|---|
| 1 | `KT-225 FP1 stringify!(include!)` | XANH |
| 2 | `KT-225 FP1b stringify!(include!())` | XANH |
| 3 | `KT-225 FP2 include != 0` | XANH |
| 4 | `KT-225 FP3 self.include != 0` | XANH |
| 5 | `KT-225 FP4 include!("…",)` (đích trong cây) | XANH |
| 6 | `KT-225 NEG1 include!("…",)` (đích ngoài cây) | ĐỎ |
| 7 | `KT-225 NEG2 pass!(include!{…})` (L40) | ĐỎ |
| 8 | `KT-225 NEG3 pass!(include!())` | ĐỎ |
| 9 | `KT-225 NEG4 include != 0 trong thân macro` | XANH |
| 10 | `KT-225 NEG5 concat!(include!(…))` | XANH |
| 11 | `KT-225 NEG6 stringify!(include!{…})` | XANH |
| 12 | `KT-225 TXT macro_rules! trong `.txt` được include!` | ĐỎ |
| 13 | `KT-225 SHADOW macro_rules! stringify` che tên an toàn | ĐỎ |

```text
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 259/259 ok
```

Log: [`00-selftest.log`](kt225-logs/00-selftest.log).

---

## 5. KHÔNG HỒI QUY

| Lệnh | Kết quả |
|---|---|
| `python3 v2/tools/check-wake-paths.py --selftest` | **259/259 ok**, RC=0 |
| `python3 v2/tools/check-wake-paths.py` (cây thật) | RC=0, `errors=0` |
| `python3 v2/tools/check-wake-paths.py --strict` (cây thật) | RC=0, `errors=0` |
| `python3 v2/tools/check-wake-depinfo.py` (cây thật) | RC=0, `errors=0` |
| `./v2/bin/v2 check` | **PASS** |
| 6 biến thể L44 + repro L40 (cây thật) | **ĐỎ** cả hai chế độ (không hồi quy) |
| `pass!(include!…)` / `pass!(#[path]…)` / `pass!(mod …)` | **ĐỎ** (không hồi quy) |

Log: [`04-real-tree-normal.log`](kt225-logs/04-real-tree-normal.log),
[`05-real-tree-strict.log`](kt225-logs/05-real-tree-strict.log),
[`06-real-tree-depinfo.log`](kt225-logs/06-real-tree-depinfo.log),
[`07-batch-attack-normal.log`](kt225-logs/07-batch-attack-normal.log),
[`08-batch-attack-strict.log`](kt225-logs/08-batch-attack-strict.log),
[`09-batch-attack-cargo.log`](kt225-logs/09-batch-attack-cargo.log),
[`14-v2check.log`](kt225-logs/14-v2check.log),
[`15-shadow-after.log`](kt225-logs/15-shadow-after.log).

---

## 6. TỆP ĐÃ SỬA

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | `_walk_all` (`in_macro_args` + luật cú pháp `include!`); `on_include` (trailing comma); `_check_macro_passthrough` (luật cú pháp + `SAFE_TOKEN_MACROS` + `shadowed_safe`); `_scan_macro_calls` (miễn `stringify!`/`concat!` trừ khi bị che); `scan_macro_bodies` (`extra_files` + `shadowed_safe`); `_kt211_case` (`msg_contains`/`line`/`code`); 13 ca selftest KT-225 |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §1/§2/§3/§4 cập nhật hành vi KT-225 (bốn báo oan đã vá; `.txt` đã đóng; `include!;` không báo) |
| `v2/agent/JOURNAL/kt225-false-positives.md` | tệp này |
| `v2/agent/JOURNAL/kt225-logs/**` | 15 log |

**Không** sửa: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`, journal
của agent khác. **Không** push; **không** chạy `gate --fast`.
