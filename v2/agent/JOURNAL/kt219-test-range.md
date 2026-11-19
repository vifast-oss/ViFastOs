# KT-218/219 — tấn công cuối hai lớp + vá đường lọt #13 (vùng test quá rộng trên nhánh `match`)

**Vai**: L49 — reviewer tấn công cuối (KT-218), sau đó tác giả bản vá (KT-219) theo lệnh lead.
**Nhánh**: `night3/L49-kt218`, gốc `e4f28098` (KT-217).
**Worktree**: `/private/tmp/night3-L49-kt218`.
**Nguồn**: lead giao KT-218 (tấn công hai lớp) + KT-219 (vá đường lọt #13) trong cùng phiên.

## 0. Kết quả

| Hạng mục | Kết quả |
|---|---|
| Đường lọt **#13** | **SỐNG** — lọt **cả hai lớp** (lớp 1 thường, lớp 1 `--strict`, lớp 2 dep-info) |
| Gốc | `_test_char_ranges` (token-tree) quét tới `{` group kế tiếp, chỉ dừng ở `;` ⇒ `#[cfg(test)]` trên nhánh `match` `A => (),` nuốt nhánh sản phẩm kế |
| Bản vá KT-219 | Vùng test kết thúc ở **cuối mục thật**: thân `{…}` nếu có, ngược lại `;`/`,` cùng mức (`,` chỉ khi đã thấy `=>`) |
| Repro #13 sau vá | **ĐỎ** cả hai lớp (thường + `--strict` + dep-info) |
| Đối chứng âm (bỏ `#[cfg(test)]`) | **ĐỎ** cả hai lớp (không hồi quy) |
| Đối chứng dương | **XANH** (15 dạng test thật trong selftest + 7 ca P1–P7 ngoài cây) |
| Selftest | **194 → 246** ca (thêm **52** ca KT-219), 246/246 ok |
| Cây thật | lớp 1 thường XANH, `--strict` XANH, lớp 2 XANH (errors=0) |
| Báo oan mới trên cây thật | **0** |

## 1. Đường lọt #13 — `#[cfg(test)]` trên nhánh `match`

### 1.1 Repro (biên dịch được, mã sản phẩm)

`v2/sim/vifast-sim/src/evil.rs`:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::kobj::{KError, Machine, ObjId};

const A: u32 = 0;
const B: u32 = 1;

pub fn evil(m: &mut Machine, t: ObjId, sel: u32) -> Result<(), KError> {
    match sel {
        #[cfg(test)]
        A => (),
        B => {
            m.wake(t)?;
        }
        _ => (),
    }
    Ok(())
}
```

`v2/sim/vifast-sim/src/lib.rs` thêm `mod evil;`.

### 1.2 Kết quả **trước** vá

| Lệnh | Kết quả |
|---|---|
| `cargo check --manifest-path v2/Cargo.toml --workspace` | **RC=0** (mã sản phẩm biên dịch thật) |
| `check-wake-paths.py` (thường) | **XANH** `errors=0` |
| `check-wake-paths.py --strict` | **XANH** `errors=0` |
| `check-wake-depinfo.py --unit host` | **XANH** `errors=0` (nạp 75 tệp `.rs`, có `evil.rs`) |

### 1.3 Đối chứng âm (chứng minh `wake` thật sự được biên dịch)

Bỏ **đúng** dòng `#[cfg(test)]`, giữ nguyên phần còn lại:

| Lớp | Kết quả |
|---|---|
| lớp 1 (thường) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE v2/sim/vifast-sim/src/evil.rs:14: gọi wake(` |
| lớp 2 (dep-info) | **ĐỎ** `E_DEPINFO_WAKE v2/sim/vifast-sim/src/evil.rs:14: gọi wake(` |

⇒ Hai lớp chỉ XANH vì mẹo `#[cfg(test)]`, không phải vì mã không được biên dịch.

### 1.4 Gốc

`_test_char_ranges` (KT-198/F2b) duyệt token-tree; khi gặp `#[cfg(test)]` + một định danh, nó
quét tới **`{` group kế tiếp** và chỉ dừng ở `;`. Với nhánh `match` `A => (),` — mục **không có
thân `{…}`**, kết thúc bằng `,` — vòng quét **bỏ qua dấu `,`**, chạy sang nhánh kế và lấy thân
`{…}` của nhánh **sản phẩm** `B => { m.wake(t); }` làm thân của mục test. Vùng test phủ luôn
`m.wake(t)`.

Vì `check-wake-depinfo.py` **dùng lại** `_test_char_ranges` (import `check-wake-paths.py`), cùng
một lỗi làm **cả hai lớp** mù — đây là lý do đường lọt #13 nguy hiểm hơn 12 đường trước (mọi
đường trước chỉ lọt một lớp, lớp kia bắt).

## 2. Bản vá KT-219

### 2.1 Quy tắc mới

Vùng test của một mục kết thúc ở **cuối mục thật**:

- mục có thân `{…}` ⇒ tới `}` khớp (như cũ);
- mục **không thân** ⇒ tới `;` **hoặc** `,` ở **cùng mức**;
- dấu `,` chỉ là mốc kết thúc khi đã thấy `=>` (nhánh `match`). Lý do: `,` còn xuất hiện trong
  generic params (`fn f<T, U>`), `use a::b, c::d;`, field struct, biến thể enum — các ngữ cảnh
  đó **không chứa được lời gọi `wake`**; cắt sớm chỉ làm vùng test **hẹp** hơn (báo oan), không
  làm rộng (lọt). Đây là điểm khác duy nhất so với quy tắc "dừng ở `,`/`;`" thuần: nếu dừng ở
  **mọi** `,` thì `#[cfg(test)] fn f<T, U>() { m.wake(t); }` sẽ bị báo oan (vùng test cắt ở `,`
  trong `<T, U>`).

### 2.2 Ca selftest (52 ca mới, tổng 246)

**Chiều âm** (phải ĐỎ — vùng test không được nuốt mục kế): nhánh `match` không thân, có guard,
đảo thứ tự, `#[cfg(feature = "ktest")]`; `use`, `const`, `static`, `mod tests;`, `fn` không thân
(trait), field struct, biến thể enum, `if`, `if let`, `let` tuple, `let` closure nhiều tham số,
`use` nhiều nhánh, `macro_rules!`, lời gọi macro `{}`/`()`, `impl`, `trait`, `struct`, `enum`,
`union`, `extern`, `unsafe`, `while`, `for`, `match`, `let match`, `let struct literal`,
`let array`, `return` block, `async` block, `const` block — mỗi dạng + một `fn` sản phẩm chứa
`m.wake(t)` đứng sau.

**Chiều dương** (phải XANH — test thật vẫn được miễn): nhánh `match` có thân, không thân, có
guard; `fn` có thân, `fn` generic nhiều tham số; `mod tests`; `use` nhập `wake`; `let` gọi
`wake`; `impl`; `trait` default method; `if`; `match`; `loop`; `unsafe`.

### 2.3 Đo sau vá

| Lệnh | Kết quả |
|---|---|
| `check-wake-paths.py --selftest` | **246/246 ok** |
| `check-wake-paths.py` (cây thật) | **XANH** `errors=0` |
| `check-wake-paths.py --strict` (cây thật) | **XANH** `errors=0` |
| `check-wake-depinfo.py` (cây thật, cả hai unit) | **XANH** `errors=0` |
| Repro #13 (thường / `--strict` / dep-info) | **ĐỎ** cả ba |
| Đối chứng âm repro #13 | **ĐỎ** cả hai lớp |

## 3. Ma trận tấn công lớp 2 (KT-218)

Mỗi ca: mã biên dịch được (`cargo check` RC=0) + chạy **cả hai** lớp. `L1n` = lớp 1 thường,
`L1s` = lớp 1 `--strict`, `L2` = lớp 2 dep-info.

| Ca | cargo | L1 thường | L1 `--strict` | L2 dep-info |
|---|---|---|---|---|
| A1 path non-rs .txt | 0 | DO | DO | XANH |
| A10 cfg(feature=ktest) inline | 0 | DO | DO | DO |
| A11 macro_rules sinh wake | 0 | DO | DO | DO |
| A12 macro nhan ten ham wake | 0 | DO | DO | DO |
| A13 concat_idents (unstable) | 101 | DO | DO | DO |
| A14 #[path] trong macro_rules | 0 | DO | DO | DO |
| A15 mod $n:ident trong macro_rules | 0 | DO | DO | DO |
| A16 include! chuoi 3 lop | 0 | DO | DO | DO |
| A17 include! tep khong ton tai | 101 | DO | DO | DO |
| A18 path Unicode | 0 | DO | DO | DO |
| A19 path co backslash | 0 | DO | DO | DO |
| A2 path ngoai workspace /tmp | 0 | DO | DO | XANH |
| A20 path toi v2/target | 0 | DO | DO | XANH |
| A3 symlink src/link.rs -> /tmp | 0 | DO | DO | XANH |
| A4 path co dau cach | 0 | DO | DO | XANH |
| A5 src/target/evil.rs | 0 | DO | DO | XANH |
| A6 OUT_DIR include (build.rs) | 0 | DO | DO | XANH |
| A7 cfg(any()) mod evil | 0 | DO | DO | XANH |
| A8 cfg(target_os=none) trong sim | 0 | DO | DO | XANH |
| A9 cfg(feature=ktest) mod evil | 0 | XANH | XANH | XANH |
| K1 cfg(target_os=none) trong kernel | 0 | DO | DO | DO |
| K2 cfg(not(target_os=none)) trong kernel | 0 | DO | DO | XANH |
| K3 path non-rs trong kernel | 0 | DO | DO | XANH |
| K4 symlink trong kernel | 0 | DO | DO | XANH |
| K5 src/target/evil.rs trong kernel | 0 | DO | DO | XANH |
| K6 match-arm #13 trong kernel | 0 | DO | DO | DO |

## 4. Ma trận tấn công lớp 1 (KT-218)

Không có ca nào lọt **chỉ** lớp 1: mọi dạng đã thử đều hoặc bị lớp 1 bắt, hoặc bị lớp 2 bắt (xem §7). Các dạng "lớp 1 mù" duy nhất là tệp ngoài phạm vi **và** ngoài `.d` (`init/*.rs`, `vifast-plat-user/lib.rs`) — không có `Machine` nên không gọi được API thật; ghi ở §7.

## 5. Đối chứng dương

| Ca | cargo | L1 thường | L1 `--strict` | L2 dep-info |
|---|---|---|---|---|
| P1 include_str/include_bytes/stringify | 0 | XANH | XANH | XANH |
| P2 path hop le trong cay | 0 | XANH | XANH | XANH |
| P3 mod binh thuong | 0 | XANH | XANH | XANH |
| P4 cfg(test) mod tests + wake | 0 | XANH | XANH | XANH |
| P5 cfg(test) fn helper + wake | 0 | XANH | XANH | XANH |
| P6 cfg(test) nhanh match co than + wake | 0 | XANH | XANH | XANH |
| P7 cfg(test) nhanh match khong than + wake | 0 | XANH | XANH | XANH |

## 6. Báo oan

**Trên cây thật: 0 báo oan mới.** Sau vá: lớp 1 thường `errors=0`, `--strict` `errors=0`, lớp 2
`errors=0` (cả hai unit). Bản vá chỉ **siết** vùng test (thêm mốc `,`/`;`), không nới; các mục
`#[cfg(test)]` trên `use`/`const`/`static`/`mod` không thân/`fn` không thân **nay được miễn
đúng** (trước đây không được miễn) ⇒ báo oan **giảm**, không tăng.

Các dạng **fail-closed đã biết** (không phải hồi quy — ghi để không thành "phát hiện mới"):

| Dạng | Hành vi | Vì sao chấp nhận |
|---|---|---|
| `#[cfg(test)] { … m.wake(t) … }` (block statement) | **ĐỎ** (báo oan) | `walk` gặp `Group('{')` trực tiếp, không dùng `pending` ⇒ không đánh dấu test. Không có trên cây thật. |
| `#[cfg(test)] A => if c { … } else { … m.wake(t) }` | **ĐỎ** (báo oan) | Vùng test dừng ở `}` của nhánh `if`; nhánh `else` bị quét. Không có trên cây thật. |
| `#[cfg(test)]` trên field struct / biến thể enum | **ĐỎ** nếu field/variant chứa token `wake` | Không đánh dấu test (không có `=>`), nhưng field/variant **không chứa được lời gọi** `wake` (const context) ⇒ không ảnh hưởng. |
| `#[cfg(test)]` trên `use`/`const`/`static`/`mod` không thân/`fn` không thân | **XANH** (miễn đúng) | Đây là **giảm** báo oan so với trước (trước đây không miễn). |
| `#[cfg(feature = "ktest")] fn … { m.wake(t) }` **inline** | **ĐỎ** (báo oan) | `strip_rust_noise` xoá chuỗi `"ktest"` trước khi `cfg_is_test_tokens` chạy ⇒ không nhận ra ngữ cảnh test. Đã có trong hợp đồng §4 (báo oan **có trước** KT-219); dạng **mod tệp riêng** thì vẫn được miễn (A9). Không có trên cây thật. |

Đối chứng dương trong selftest: **15 ca** (3 nhánh `match` + 12 dạng khác) đều XANH.


## 7. Đánh giá phủ hai lớp

**Lớp 1 (tĩnh) phủ rộng:** quét **mọi** tệp `.rs` dưới `v2/kernel`+`v2/sim` (kể cả tệp không
được biên dịch), **mọi nhánh `#[cfg]`**, và **mọi đích** `include!`/`#[path]` trong cây — kể cả
tệp không `.rs`, symlink, thư mục tên `target`, tên có dấu cách/Unicode; đích ra ngoài cây ⇒
**ĐỎ fail-closed**. Đây là lớp duy nhất bắt được các ca "lớp 2 mù" (A1–A10, K2–K5).

**Lớp 2 (dep-info) phủ sâu:** đúng danh sách tệp rustc nạp — kể cả tệp ngoài `src/`, `#[path]`
lồng, tệp trong `v2/lib` (lớp 1 **không** quét token `wake` ở `v2/lib`), và mọi tệp được cargo
biên dịch. Đây là lớp bắt được `wake` trong `v2/lib` (trừ tệp có `fn wake(` — producer).

**Lỗ của lớp 2 (lớp 1 che):** tệp không `.rs`; tệp ngoài root; symlink ra ngoài root; tên có dấu
cách (parse `.d` tách theo khoảng trắng); thư mục tên `target` (bộ lọc `"target" in relp.parts`
— **tái hiện đường lọt #12 ở lớp 2**); `OUT_DIR`; nhánh `#[cfg]` không bật (`cfg(any())`,
`cfg(target_os = "none")` trên host, `cfg(not(target_os = "none"))` trên kernel); `#[path]` tới
`v2/target`. Tất cả **đã đo** trong §3; lớp 1 bắt **toàn bộ** ⇒ cổng vẫn ĐỎ.

**Lỗ của lớp 1 (lớp 2 che):** token `wake` trong `v2/lib` — lớp 1 chỉ bảo vệ thân macro +
`include!`/`#[path]` ở `v2/lib`, không quét token `wake`; lớp 2 quét mọi tệp nạp (trừ producer)
⇒ bắt. Bù đúng.

**Lỗ ở CẢ HAI lớp (còn lại sau vá):**

| # | Dạng | Sống? | Ghi chú |
|---|---|---|---|
| 1 | `#[cfg(feature = "ktest")] mod evil;` (tệp riêng) chứa `m.wake(t)` | **Không** | Cả hai coi `feature = "ktest"` là test-only (`cfg_is_test_tokens` trên token-tree **chưa khử chuỗi** ở đồ thị module); bản sản phẩm **không** bật `ktest` (`default = []`; ảnh sản phẩm chỉ bật `product-shell`/`product-fs`/`user`). Nếu tương lai có bản sản phẩm bật `ktest` ⇒ thành lỗ **sống** — **nợ có tên**. **Lưu ý**: dạng **inline** `#[cfg(feature = "ktest")] fn … { m.wake(t) }` **không** lọt — `strip_rust_noise` xoá chuỗi `"ktest"` nên `cfg_is_test_tokens` không nhận ra ⇒ **ĐỎ** cả hai lớp (báo oan đã biết, fail-closed). |
| 2 | Tệp ngoài phạm vi + ngoài `.d` (`init/*.rs`, `vifast-plat-user/lib.rs`) chứa `wake` | **Không** | Không có `Machine` (chỉ syscall U-mode) ⇒ không gọi được API thật. Nếu tương lai có `Machine` ⇒ thành lỗ — **nợ có tên**. |
| 3 | Mã do `macro_rules!` sinh **không chứa token `wake`** (proc-macro/`concat_idents!`) | **Không** | `concat_idents!` cần `#![feature]` ⇒ stable **RC≠0** (đo A13); không có proc-macro trong allowlist phụ thuộc. Nếu thêm proc-macro ⇒ thành lỗ — **nợ có tên** (hợp đồng §5). |

**Kết luận phủ:** hai lớp **phủ nhau** ở mọi dạng đã đo; đường lọt #13 là lỗ **chung** duy nhất
tìm được và **đã đóng**. Ba nợ có tên ở trên đều **không sống** trên cấu hình hiện tại; chúng
phải được kiểm lại nếu (a) một bản sản phẩm bật `ktest`, (b) `init/`/`vifast-plat-user` có
`Machine`, (c) allowlist phụ thuộc có proc-macro.


## 8. Tệp + log

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | vá `_test_char_ranges` (KT-219) + 52 ca selftest |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §2 (dạng hỗ trợ), §4 (đóng đường lọt #13) |
| `v2/agent/JOURNAL/kt219-test-range.md` | tệp này |
| `v2/agent/JOURNAL/kt219-logs/` | log từng ca + log cây thật + log selftest |
