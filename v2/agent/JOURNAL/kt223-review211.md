# KT-223 — Review **độc lập** KT-211 (đóng đường lọt #2–#6: quét `include!`/`#[path]` toàn cục)

**Vai**: L52 — **reviewer độc lập** (không phải tác giả; không sửa checker).
**Nhánh**: `night3/L52-kt223`, gốc `83275d94`. **Worktree**: `/private/tmp/night3-L52-kt223`.
**Phạm vi**: KT-211 (L46) — `_walk_all` trong `v2/tools/check-wake-paths.py`; tuyên bố trong
`v2/agent/JOURNAL/kt211-include-global.md` và `v2/agent/reports/CHECKER-WAKE-CONTRACT.md`.
**Nguyên tắc**: **không tin** tuyên bố; mỗi kết luận phải có log trong
[`kt223-logs/`](kt223-logs/). Không sửa `v2/tools/check-wake-paths.py` — chỉ tấn công + ghi nhận.

---

## 0. KẾT LUẬN NGẮN

**ĐẠT CÓ ĐIỀU KIỆN.**

Tuyên bố **chính** của KT-211 **đã kiểm lại và đúng**: 6 biến thể L44 ⇒ **6/6 ĐỎ** cả hai chế độ,
`cargo check` **RC=0**; repro L40 ⇒ **ĐỎ** cả hai chế độ; selftest **246/246**. `_walk_all` chịu
được **36 dạng** tấn công tôi dựng (mọi tầng nhóm, mọi loại nhóm, generic/const-generic/array
length/match guard/attribute/raw ident/comment/Unicode/nested) — **không** có đường lọt **đầy đủ**
(cả hai lớp XANH) nào.

**Nhưng** có **1 đường lọt lớp 1** (checker tĩnh XANH cả hai chế độ, `cargo check` RC=0) và **4
báo oan** (mã hợp lệ bị ĐỎ), trong đó **1 báo oan nằm đúng danh sách đối chứng dương bắt buộc**
của phiếu (`stringify!(include!)`). Vì vậy **không** thể kết luận "ĐẠT" trơn.

| Hạng mục | Số |
|---|---|
| Selftest | **246/246** |
| 6 biến thể L44 ĐỎ | **6/6** (cả hai chế độ; cargo RC=0) |
| Repro L40 | **ĐỎ** (cả hai chế độ; cargo RC=0) |
| Dạng tấn công đã thử | **36** (29 dạng mới + 6 biến thể L44 + repro L40) |
| Đường lọt **lớp 1** mới | **1** — `.txt` macro definition (lớp 2 bắt được) |
| Đường lọt **đầy đủ** (cả hai lớp XANH) | **0** |
| Báo oan | **4** — `stringify!(include!)`, `include != 0`, `self.include != 0`, `include!("x",)` |
| Ca selftest có răng (trong 8 ca chọn) | **6/8** |

---

## 1. KIỂM LẠI TUYÊN BỐ (A)

### A1. Selftest 246/246 — **ĐÚNG**

```text
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 246/246 ok
```

Khớp tuyên bố "sau KT-219 thành **246**". Log: [`00-selftest.log`](kt223-logs/00-selftest.log).

### A2. Sáu biến thể L44 — **6/6 ĐỎ**, cargo **RC=0**

Dựng lại **độc lập** trên cây thật (worktree này), mỗi biến thể một tệp
`v2/kernel/vifast-core/src/kt223_a0N.rs` + `mod` trong `lib.rs`, payload
`v2/lib/evil_expr.rs` = `m.wake(t)`:

| # | Dạng | cargo | thường | `--strict` | depinfo |
|---|---|---|---|---|---|
| a01 | `sink(include!{…})` | **0** | **ĐỎ** | **ĐỎ** | ĐỎ (bắt `wake` trong `v2/lib/evil_expr.rs`) |
| a02 | `[include!{…}]` | **0** | **ĐỎ** | **ĐỎ** | ĐỎ |
| a03 | `(0u8, include!{…})` | **0** | **ĐỎ** | **ĐỎ** | Đỏ |
| a04 | `(include!{…})` | **0** | **ĐỎ** | **ĐỎ** | Đỏ |
| a05 | `(|| include!{…})` | **0** | **ĐỎ** | **ĐỎ** | Đỏ |
| a06 | `Some(include!{…})` | **0** | **ĐỎ** | **ĐỎ** | Đỏ |

Log: [`03-attack-normal.log`](kt223-logs/03-attack-normal.log),
[`04-attack-strict.log`](kt223-logs/04-attack-strict.log),
[`05-attack-cargo.log`](kt223-logs/05-attack-cargo.log) (RC=0),
[`06-attack-depinfo.log`](kt223-logs/06-attack-depinfo.log).

### A3. Repro L40 (`:item` + `include!` trong đối số lời gọi) — **ĐỎ**

`macro_rules! pass223 { ($i:item) => { $i }; } pass223!(include! {"../../../lib/evil_item.rs"});`
⇒ cargo **RC=0**, checker thường **ĐỎ** (3 lỗi: `E_STRICT_WAKE_INCLUDE` + `E_STRICT_MACRO_TT` +
`E_STRICT_MACRO_INCLUDE`), `--strict` **ĐỎ** (3 lỗi). Log cùng bộ trên (tệp `kt223_a07.rs`).

### A4. `./v2/bin/v2 check` trên cây sạch — **PASS**

```text
$ ./v2/bin/v2 check
...
v2 check: PASS (L1 chạy trong gate --fast)
real 2m51.306s
```

Log: [`13-v2check.log`](kt223-logs/13-v2check.log). (Chỉ chạy `check`, **không** chạy
`gate --fast` — đúng phiếu.)

---

## 2. TẤN CÔNG `_walk_all` (B)

**Cách đo**: một lần chèn **34 tệp** vào `v2/kernel/vifast-core/src/` (mỗi dạng một tệp) + **6**
payload ở `v2/lib/` + `mod` trong `lib.rs`; chạy **một** lượt `cargo check -p vifast-core`, **một** lượt checker
thường, **một** lượt `--strict`, **một** lượt `check-wake-depinfo.py`; quy kết lỗi theo **đường
dẫn tệp**. Các dạng **không biên dịch được** (b03/b04/b05/b14/b16) vẫn được đo ở checker (chúng
không phải đường lọt vì không biên dịch được).

### 2.1. Bảng kết quả (29 dạng tấn công mới)

| # | Dạng | Biên dịch | thường | `--strict` | depinfo | Kết luận |
|---|---|---|---|---|---|---|
| b01 | `include!(concat!("…","…"))` | **0** | **ĐỎ** | **ĐỎ** | ĐỎ | fail-closed **đúng** hợp đồng |
| b02 | `include!(env!("X"))` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | fail-closed **đúng** hợp đồng |
| b03 | `include!()` rỗng | ✗ | **ĐỎ** | **ĐỎ** | — | fail-closed |
| b04 | `include!;` không đối số | ✗ | **ĐỎ** | **ĐỎ** | — | fail-closed |
| b05 | `include!("a","b")` hai đối số | ✗ | **ĐỎ** | **ĐỎ** | — | fail-closed |
| b06 | `include!("…",)` trailing comma | **0** | **ĐỎ** | **ĐỎ** | Đỏ | **báo oan** (mã hợp lệ) |
| b07 | `include!(r"…")` raw string | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng (đích ngoài cây) |
| b08 | `r#include!("…")` raw ident | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b09 | `include /*c*/ ! (…)` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b10 | generic `foo::<{ include!(…) }>()` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b11 | const generic `Foo::<{ include!(…) }>` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b12 | array length `[0u8; include!(…)]` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b13 | match guard `_ if include!(…)` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b14 | `include!(include!(include!(…)))` 3 lớp | ✗ | **ĐỎ** | **ĐỎ** | — | fail-closed |
| b15 | `#[doc = include_str!("…")]` | **0** | **XANH** | **XANH** | XANH | **đúng** (đối chứng dương) |
| b16 | `#[doc = include!("…")]` | ✗ | **XANH** | **XANH** | — | không biên dịch ⇒ không phải lọt |
| b17 | `include!` trong `macro_rules!` (tệp `.rs`) | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng (`E_STRICT_MACRO_INCLUDE`) |
| b18 | `#[path]` trong `macro_rules!` (tệp `.rs`) | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng |
| b19 | `#[path]` trên `mod` trong `( … )` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng (L46 báo đã bắt — **kiểm lại: đúng**) |
| b20 | `includé!` (Unicode) | **0** | **XANH** | **XANH** | XANH | **đúng** (macro khác) |
| b21 | `let include = 1; include!(…)` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | đúng (vẫn là `include!` thật) |
| b22 | `include != 0` (biến) | **0** | **ĐỎ** | **ĐỎ** | Đỏ | **báo oan** |
| b23 | `self.include != 0` (trường) | **0** | **ĐỎ** | **ĐỎ** | Đỏ | **báo oan** |
| b24 | `include!` trong `macro_rules!` định nghĩa ở **`.txt`** | **0** | **XANH** | **XANH** | **ĐỎ** | **ĐƯỜNG LỌT LỚP 1** (xem §3) |
| b25 | `stringify!(include!)` | **0** | **ĐỎ** | **ĐỎ** | Đỏ | **báo oan** (đúng danh sách đối chứng dương) |
| b26 | `include!(r"kt223_ok.rs")` trong cây | **0** | **XANH** | **XANH** | XANH | **đúng** |
| b27 | `#[path]` hợp lệ trong cây | **0** | **XANH** | **XANH** | XANH | **đúng** |
| b28 | `mod` bình thường | **0** | **XANH** | **XANH** | XANH | **đúng** |
| b29 | `#[cfg(test)] mod` | **0** | **XANH** | **XANH** | XANH | **đúng** |

Log: [`03-attack-normal.log`](kt223-logs/03-attack-normal.log),
[`04-attack-strict.log`](kt223-logs/04-attack-strict.log),
[`05-attack-cargo.log`](kt223-logs/05-attack-cargo.log) (**RC=0** — mọi dạng biên dịch được đều
biên dịch), [`06-attack-depinfo.log`](kt223-logs/06-attack-depinfo.log).

**Lưu ý cột depinfo**: đây là kết quả **một lượt chạy gộp** — depinfo bắt `wake` trong các payload
`v2/lib/evil_*.rs` mà các dạng `include!`/`#[path]` kéo vào. Với các dạng **không** kéo payload
(b22/b23/b25 — báo oan), depinfo **XANH** khi chạy riêng; điều này **không** ảnh hưởng kết luận
(vì chúng không phải đường lọt).

### 2.2. `cur_dir` khi vào inline module — **ĐÚNG** (kiểm lại tuyên bố L46)

| Dạng | Kỳ vọng | Kết quả |
|---|---|---|
| `#[path = "sub"] mod m { #[path = "y.rs"] mod n; }` + `sub/y.rs` | XANH | **XANH** |
| cùng dạng, `y.rs` ở `src/` (sai vị trí) | ĐỎ | **ĐỎ** (`sub/y.rs` không tồn tại) |
| inline module **không** `#[path]` trong `src/foo.rs` + `src/foo/m/y.rs` | XANH | **XANH** |
| cùng dạng, `y.rs` ở `src/m/y.rs` (sai) | ĐỎ | **ĐỎ** (`src/foo/m/y.rs` không tồn tại) |
| `include!` trong inline module, tệp cạnh tệp gốc | XANH | **XANH** |
| `include!` trong inline module trỏ ra ngoài | Đỏ | **ĐỎ** |
| `#[path]` trong `( … )` lồng 2 tầng | Đỏ | **ĐỎ** |

Log: [`02-inline-curdir.log`](kt223-logs/02-inline-curdir.log). Bộ thử synthetic (nhanh, trước khi
dựng lại trên cây thật): [`12-synthetic-attacks.log`](kt223-logs/12-synthetic-attacks.log).

---

## 3. ĐƯỜNG LỌT LỚP 1 — `macro_rules!` định nghĩa trong tệp **không phải `.rs`**

### 3.1. Dựng lại

```text
v2/kernel/vifast-core/src/macro_def.txt:
    macro_rules! evil_txt { () => { include!("../../../lib/evil_expr.rs") } }

v2/kernel/vifast-core/src/lib.rs (thêm):
    include!("macro_def.txt");
    mod kt223_c01;

v2/kernel/vifast-core/src/kt223_c01.rs:
    pub fn kt223_c01(m: &mut crate::Machine, t: crate::ObjId) {
        let _ = evil_txt!();
    }

v2/lib/evil_expr.rs:
    m.wake(t)
```

Biến thể `#[path]`: `macro_def2.txt` chứa
`macro_rules! evil_txt_path { () => { #[path = "../../../lib/evil_mod.rs"] mod evil_mod; } }`,
gọi từ `kt223_c02.rs`; `v2/lib/evil_mod.rs` = `pub fn evil_mod(m: &mut Machine, t: ObjId) { let _ = m.wake(t); }`.

### 3.2. Kết quả

| Lớp | Kết quả |
|---|---|
| `cargo check -p vifast-core` | **RC=0** (biên dịch được; rustc nạp `v2/lib/evil_expr.rs` / `v2/lib/evil_mod.rs`) |
| `check-wake-paths.py` (thường) | **XANH**, `errors=0` |
| `check-wake-paths.py --strict` | **XANH**, `errors=0` |
| `check-wake-depinfo.py` | **ĐỎ** — `E_DEPINFO_WAKE` trên `v2/lib/evil_expr.rs` + `v2/lib/evil_mod.rs` |

Log: [`07-bypass-txt-normal.log`](kt223-logs/07-bypass-txt-normal.log),
[`08-bypass-txt-strict.log`](kt223-logs/08-bypass-txt-strict.log),
[`09-bypass-txt-cargo.log`](kt223-logs/09-bypass-txt-cargo.log),
[`10-bypass-txt-depinfo.log`](kt223-logs/10-bypass-txt-depinfo.log).

### 3.3. Nguyên nhân

1. `_walk_all` **bỏ qua thân `macro_rules!`** (đúng thiết kế — thân macro do
   `_check_macro_body` xử lý riêng).
2. `_check_macro_body` chỉ chạy trong `scan_macro_bodies`, mà hàm này duyệt
   `base.rglob("*.rs")` cho `base ∈ MACRO_SCAN_ROOTS` ⇒ tệp **`.txt`** (hay mọi đuôi khác `.rs`)
   **không** được quét thân macro.
3. `include!("macro_def.txt")` ở vị trí item được `_walk_all` thấy (đích **trong** cây ⇒ không
   lỗi), nhưng khi `_walk_all` chạy trên `macro_def.txt` thì thân macro bị **bỏ qua** ⇒ không có
   cạnh nào tới `v2/lib/evil_expr.rs`.
4. `_scan_macro_calls` chỉ thấy lời gọi `evil_txt!()` với đối số rỗng ⇒ không có dấu hiệu.

⇒ **Lớp 1 (checker tĩnh) XANH cả hai chế độ** trong khi mã sản phẩm gọi `wake`. **Lớp 2**
(`check-wake-depinfo.py`, đã nối vào `v2 check` từ KT-217) **bắt được** vì rustc nạp
`v2/lib/evil_expr.rs` và `.d` liệt kê nó. Vì vậy **cổng `v2 check` không bị lọt** — nhưng
**tuyên bố của hợp đồng** "thân `macro_rules!` ở **mọi** tệp `v2/lib`+`v2/kernel`+`v2/sim` được
kiểm" (CHECKER-WAKE-CONTRACT §1/§4) là **quá mức**: chỉ đúng cho `*.rs`.

### 3.4. Mức

**CAO** (đường lọt lớp 1; lớp 2 che). Điều kiện để lọt **cả hai** lớp: đích `include!` nằm
**ngoài gốc repo** (depinfo bỏ qua `relative_to(root)` lỗi) hoặc dưới thư mục tên `target/`
(depinfo bỏ qua `"target" in relp.parts`) — cả hai đều **không** phải tệp commit được bình
thường (`target/` bị `.gitignore`), nên tôi **không** dựng được đường lọt **đầy đủ** từ nguồn
commit được. Ghi nhận là **lớp 1**.

---

## 4. BÁO OAN (C)

| # | Dạng | Biên dịch (rustc) | Checker | Mức |
|---|---|---|---|---|
| 1 | `stringify!(include!)` | **0** | **ĐỎ** (`E_STRICT_WAKE_INCLUDE` + `E_STRICT_MACRO_INCLUDE`) | **TRUNG BÌNH** — nằm **đúng** danh sách đối chứng dương bắt buộc của phiếu |
| 2 | `fn f(include: u8) { if include != 0 {} }` | **0** | **ĐỎ** (`include!` không phải literal) | **TRUNG BÌNH** — `!=` bị đọc thành `include` + `!` |
| 3 | `struct S { include: u8 } … s.include != 0` | **0** | **ĐỎ** | **TRUNG BÌNH** — cùng gốc |
| 4 | `include!("…",)` (trailing comma) | **0** | **ĐỎ** | **THẤP** — hiếm |

Ngoài ra **fail-closed có chủ đích** (không phải báo oan theo hợp đồng, nhưng là hạn chế cho mã
hợp lệ): `include!(concat!(…))`, `include!(env!(…))` — cả hai **biên dịch được** (đã kiểm bằng
`rustc`), checker **ĐỎ**. Hợp đồng §3 ghi rõ "không literal ⇒ ĐỎ", nên đây là **đúng hợp đồng**.

**Gốc chung của báo oan #2/#3**: `_walk_all` coi **mọi** `include` + `!` là `include!`, kể cả khi
`!` là ký tự đầu của `!=` (lexer tách `!=` thành hai punct `!` + `=`). Tương tự
`_check_macro_passthrough` (báo oan #1). Đây là **lỗi nhận dạng token**, không phải lỗi phân giải.

---

## 5. RĂNG CỦA CA SELFTEST (D)

Chọn **8 ca mới** của KT-211, đọc mã `_kt211_case` (dòng ~5995–6050), xác nhận **có assert
thật**: `got = any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors)`; ca qua ⇔ `got == expect_red`.
Sau đó **phá** fixture để ca không còn kiểm điều tuyên bố:

| Ca | Phá bằng | Ca còn xanh? | Răng |
|---|---|---|---|
| B01 `sink(include!{…})` | đổi `include!` → `include_str!` | **FAIL** | **có** |
| B02 `[include!{…}]` | đổi `include!` → `include_str!` | **FAIL** | **có** |
| B23 `(include!{…})` | đổi `include!` → `include_str!` | **FAIL** | **có** |
| B26 `include_str!` (dương) | đổi → `include!` ra ngoài | **FAIL** | **có** |
| B26 `stringify!(include)` (dương) | đổi → `stringify!(include!)` | **FAIL** (bắt được nhờ `E_STRICT_WAKE_INCLUDE`) | **có** |
| B28 `#[path]` trong `( … )` (dương) | đổi đích → ngoài cây | **FAIL** | **có** |
| B29 `include!` không literal | đổi `concat!` → literal ra ngoài | **PASS** | **KHÔNG** — chỉ assert **mã lỗi**, không assert **lý do** ("không phải literal") |
| B30 `include!` lồng 2 tầng | đổi → 1 tầng | **PASS** | **KHÔNG** — chỉ assert **mã lỗi**, không assert **lý do** ("lồng") |

**6/8 ca có răng.** Hai ca B29/B30 vẫn có răng cho tuyên bố **chung** ("`include!` trong `( … )`
bị bắt"), nhưng **không** có răng cho tuyên bố **cụ thể** (không-literal / lồng). Log:
[`01-teeth.log`](kt223-logs/01-teeth.log).

---

## 6. ĐỐI CHỨNG DƯƠNG (C) — tổng hợp

| Dạng | thường | `--strict` | depinfo |
|---|---|---|---|
| `include_str!` | **XANH** | **XANH** | XANH |
| `include_bytes!` | **XANH** | **XANH** | XANH |
| `stringify!(include)` | **XANH** | **XANH** | XANH |
| `stringify!(include!)` | **ĐỎ** | **ĐỎ** | Đỏ | ⇒ **báo oan** |
| `#[path]` hợp lệ trong cây | **XANH** | **XANH** | XANH |
| `mod` bình thường | **XANH** | **XANH** | XANH |
| `#[cfg(test)] mod tests` | **XANH** | **XANH** | XANH |
| `wake` trong tệp allowlist (`core_sched.rs`/`kobj.rs`) | **XANH** | **XANH** | XANH (miễn 2 tệp cửa wake) |

`wake` trong tệp allowlist: cây thật (không chèn gì) cho `errors=0` cả hai chế độ; depinfo miễn
đúng 2 tệp cửa wake (`# quét: 143 tệp (miễn 2 tệp cửa wake …)`). Log: baseline
[`06-attack-depinfo.log`](kt223-logs/06-attack-depinfo.log) + các log batch.

---

## 7. PHÁT HIỆN

| # | Mức | Mô tả | Bằng chứng | Cách dựng lại |
|---|---|---|---|---|
| **F1** | **CAO** | `macro_rules!` định nghĩa trong tệp **không phải `.rs`** (`.txt`) được `include!` ở vị trí item ⇒ thân macro **không** được `_check_macro_body` quét (`scan_macro_bodies` chỉ `rglob("*.rs")`), `_walk_all` bỏ qua thân ⇒ **lớp 1 XANH cả hai chế độ** trong khi `cargo check` RC=0 và mã sản phẩm gọi `wake`. Lớp 2 (depinfo) **bắt được** ⇒ cổng `v2 check` không lọt, nhưng hợp đồng §1/§4 tuyên bố "mọi tệp" là **quá mức**. | [`07`](kt223-logs/07-bypass-txt-normal.log), [`08`](kt223-logs/08-bypass-txt-strict.log), [`09`](kt223-logs/09-bypass-txt-cargo.log), [`10`](kt223-logs/10-bypass-txt-depinfo.log) | §3.1 |
| **F2** | **TRUNG BÌNH** | `stringify!(include!)` — mã hợp lệ — **ĐỎ** cả hai chế độ. Nằm **đúng** danh sách đối chứng dương bắt buộc của phiếu ⇒ **báo oan**. | [`03`](kt223-logs/03-attack-normal.log) (`kt223_b25.rs`) | `pub fn f() { let _ = stringify!(include!); }` |
| **F3** | **TRUNG BÌNH** | `include != 0` / `self.include != 0` — mã hợp lệ — **ĐỎ** (`!=` bị đọc thành `include` + `!`). | [`03`](kt223-logs/03-attack-normal.log) (`kt223_b22.rs`, `kt223_b23.rs`) | `pub fn f(include: u8) { if include != 0 {} }` |
| **F4** | **THẤP** | `include!("…",)` (trailing comma) — mã hợp lệ — **ĐỎ**. | [`03`](kt223-logs/03-attack-normal.log) (`kt223_b06.rs`) | `let _ = include!("x.rs",);` |
| **F5** | **THẤP** | Ca selftest B29/B30 chỉ assert **mã lỗi**, không assert **lý do** ⇒ không có răng cho tuyên bố cụ thể (không-literal / lồng). | [`01`](kt223-logs/01-teeth.log) | §5 |
| **F6** | **THÔNG TIN** | `include!(concat!(…))` / `include!(env!(…))` — mã hợp lệ — **ĐỎ** (fail-closed **đúng** hợp đồng §3). | [`03`](kt223-logs/03-attack-normal.log) (`kt223_b01.rs`, `kt223_b02.rs`) | §2.1 |

**Không** tìm thấy đường lọt **đầy đủ** (cả hai lớp XANH) nào từ nguồn commit được.

---

## 8. ĐIỀU KIỆN ĐỂ "ĐẠT"

1. **F1**: hoặc (a) mở `scan_macro_bodies` cho **mọi tệp** được `include!` vào cây (không chỉ
   `*.rs`), hoặc (b) hạ tuyên bố trong `CHECKER-WAKE-CONTRACT.md` §1/§4 từ "mọi tệp" xuống
   "mọi tệp `*.rs`" + ghi rõ lỗ hổng `.txt` và việc lớp 2 che. **Không** được coi là đã đóng.
2. **F2/F3/F4**: sửa nhận dạng `include` + `!` để chỉ coi là `include!` khi `!` được theo sau bởi
   **nhóm phân cách** (`(`, `[`, `{`) hoặc literal chuỗi; nếu không, hạ xuống **cảnh báo** thay vì
   ĐỎ. Tối thiểu: thêm 4 dạng vào danh sách "báo oan có trước" của hợp đồng §4 (hiện chỉ có
   `stringify!(include!)`, **không** có `include !=`).
3. **F5**: thêm assert **lý do** cho B29/B30 (kiểm chuỗi "không phải literal" / "lồng") — hoặc
   chấp nhận ghi rõ "chỉ kiểm mã lỗi".

---

## 9. TỆP ĐÃ TẠO

| Tệp | Nội dung |
|---|---|
| `v2/agent/JOURNAL/kt223-review211.md` | tệp này |
| `v2/agent/JOURNAL/kt223-logs/**` | 14 log (selftest, 6 biến thể, 29 dạng tấn công, đường lọt `.txt`, răng, `cur_dir`, rustc, `v2 check`) |

**Không** sửa: `v2/tools/check-wake-paths.py`, `spec/`, `v2/agent/QUEUE.md`,
`v2/agent/DECISIONS.md`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/v1/*`, `AGENTS.md`,
`v2/AGENTS.md`, journal của agent khác.
