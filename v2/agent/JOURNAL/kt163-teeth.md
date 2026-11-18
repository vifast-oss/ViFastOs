# KT-163 — máy kiểm "test có răng": chống lớp lỗi xanh-giả

**Ngày**: 2026-11-19 · **Agent 2 (L5)** · nhánh `night2/L5-teeth` · HEAD trước task: `6b2fdce7`
**Nguồn nợ**: hơn mười sự cố "test xanh nhưng giả" trong repo; gần nhất N7 suýt đóng sai
(D2-137) — chỉ reviewer độc lập mới phát hiện, vì **không có máy kiểm** nào bắt lớp lỗi đó.

## 0. Việc được giao (nguyên văn, trích)

> Viết `v2/tools/check-test-teeth.py` quét `v2/**/tests/*.rs` + khối `#[cfg(test)]` trong
> `v2/**/src/**/*.rs`. Bắt **bốn lớp chắc chắn**: (a) `assert!(true)` / `assert_eq!(x, x)`
> hằng-đúng; (b) `#[ignore]`; (c) `#[test]` không có token khẳng định nào **và** không gọi
> hàm trợ giúp đã biết; (d) `let _ =` nuốt `Result` trong thân test.
> **Allowlist tường minh** (`ALLOW` với `file:dòng` + lý do); **fail-closed**.
> `--selftest` ≥ 8 ca. Nếu PASS: nối vào `v2 check` + chứng minh bằng **đột biến**.

## 1. Kết luận trước (đọc nhanh)

- **ĐẠT cả bốn lớp**, `--selftest` **21/21**, quét cây **0 vi phạm** (944 `#[test]`, 124 tệp
  có phạm vi test), **đã nối vào `v2 check`**, đột biến `assert!(true)` ⇒ `v2 check` **ĐỎ**
  (exit 1, "2 bước FAIL"), gỡ ⇒ **PASS**.
- **Sửa 2 vi phạm thật** trên cây (không nới luật): `assert_eq!(pte, pte)` ở
  `v2/lib/vifast-pte/tests/kt35_pte.rs:203` và `assert_eq!(info, info)` ở
  `v2/sim/vifast-sim/src/vspace.rs:284` — cả hai là hằng-đúng (hai vế **cùng một biến**).
  Đã thay bằng so sánh với giá trị **dựng lại từ cùng bit/trường** ⇒ thật sự kiểm `PartialEq`.
- **Allowlist 20 mục** cho lớp (d), tất cả là fuzz/property hoặc "không panic" — xem §5.
  Lớp (a)(b) **không có allowlist** (cố ý): chúng chắc chắn, không có ca hợp lệ.
- **Còn thiếu** (ghi nợ, §7): lớp (d) chỉ bắt khi **đọc được** kiểu trả về trong cây; lớp (c)
  không thấy trợ giúp ở tệp khác trừ khi khai trong `HELPER_ALLOW`.

## 2. Bốn lớp — định nghĩa chính xác và vì sao "chắc chắn"

| Mã | Lớp | Cách nhận | Vì sao chắc chắn |
|---|---|---|---|
| `E_CONST_TRUE` | (a) hằng-đúng | `assert!(true)`, `assert!(N == N)` cùng hằng, `assert_eq!`/`debug_assert_eq!` hai vế **giống hệt** sau chuẩn hoá khoảng trắng **và không vế nào có lời gọi** | cú pháp là bằng chứng; không suy luận gì |
| `E_CONST_FALSE` | (a) hằng-sai | `assert_ne!`/`debug_assert_ne!` hai vế giống hệt, không lời gọi | cùng lớp suy biến (luôn sai) |
| `E_IGNORE` | (b) | `#[ignore]` / `#[ignore = "…"]` bất kỳ đâu trong tệp đã quét | AGENTS.md §4.3 cấm tuyệt đối |
| `E_NO_ASSERT` | (c) | `#[test]` mà thân **riêng** không có token khẳng định, không `panic!` có điều kiện, không `#[should_panic]`, và không gọi trợ giúp đã biết | danh sách token là hữu hạn và tường minh |
| `E_LET_UNDERSCORE` | (d) | trong thân test, `let _ = <gọi hàm>;` mà hàm đó **khai báo** trả `Result<…>` (tra bảng chữ ký dựng từ chính cây, phân giải `type X = Result<…>`) | đọc được kiểu trả về, không đoán theo tên |
| `E_ALLOW_STALE` | fail-closed | mục allowlist không còn khớp chỗ nào | allowlist chết = che vi phạm mới |
| `E_NO_TESTS` / `E_NO_TREE` | fail-closed | cây không có `#[test]` / không có `v2/` | "không có gì để kiểm" không phải xanh |

### 2.1 Ba quyết định thiết kế đáng ghi

**(i) `assert_eq!(f(seed), f(seed))` KHÔNG phải hằng-đúng.** Đây là **ca tất định**: gọi hai
lần phải ra cùng kết quả — test thật, và cây đang dùng đúng mẫu này ở ba chỗ
(`kt20_budget.rs:198`, `kt21_donation.rs:1134`, `kt22_mailbox.rs:779`). Phân biệt bằng **cú
pháp**: hai vế giống hệt **và** có lời gọi (`CALL_RE`) ⇒ bỏ qua. Lần chạy đầu bắt oan cả ba
⇒ đã sửa luật trước khi nối cổng, không đưa vào allowlist (allowlist cho một mẫu **đúng** là
nới luật trá hình).

**(ii) `unwrap()` không phải răng.** Danh sách token của đề bài **không** có `unwrap`; giữ
đúng như vậy và ghi rõ lý do: `unwrap` chứng minh "không lỗi", **không** chứng minh giá trị
đúng — đúng lớp xanh-giả cần bắt. Cây hiện **không** có test nào chỉ dựa vào `unwrap`
(0/944), nên luật này chưa cắn ai; ca `#[test]` chỉ có `.unwrap()` có mặt trong `--selftest`
để giữ biên.

**(iii) Khoá allowlist là `(tệp, hàm test, thứ tự)`, không phải số dòng.** Đề bài ghi
`file:dòng`; tôi giữ **nguyên văn `file:dòng` trong thông báo** nhưng khoá theo thứ tự, cùng
lựa chọn với `check-wake-paths.py` (`NON_WAKE_READY_SITES`) và ghi lý do ngay trong docstring:
số dòng trôi khi sửa tệp, và khoá theo dòng sẽ **im lặng** chuyển sang một vi phạm khác khi
dòng dịch — tức là fail-**open** đúng lúc cần fail-closed. Thứ tự chỉ đếm trên **ứng viên**
(đã phân giải `Result`), nên thêm một `let _ =` không-Result không làm trôi khoá. Trường
`line` trong allowlist giữ để người đọc đối chiếu.

## 3. Phạm vi quét

- `v2/**/tests/*.rs` (trừ `compile-fail/`) — cả tệp là phạm vi test.
- `v2/**/src/**/*.rs` — chỉ trong khối `#[cfg(test)]`, nhận **cả** `#[cfg(test)]` **và**
  `#[cfg(all(test, feature = "…"))]` (bản đầu chỉ khớp `cfg(test)` nên bỏ sót
  `vifast-traceformat/src/lib.rs` — 6 test), dạng `mod X { … }` **hoặc** `mod X;` trỏ tới tệp
  khác (`vifast-core/src/{capalloc,tls}_tests.rs` — 16 test).
- Bỏ `target/` và `v2/test/fixtures/` (fixture cố tình sai cho luật khác).
- `#[test]` **ngoài** `#[cfg(test)]` trong `src/` **không** quét (không báo oan) — có ca
  `--selftest` riêng.

## 4. Bằng chứng (nguyên văn dòng log)

### 4.1 `--selftest` — 21/21

`v2/agent/JOURNAL/kt163-logs/kt163-selftest.log`:

```
ok - cây sạch ⇒ xanh
ok - `assert!(true)` ⇒ đỏ
ok - `assert_eq!(x, x)` ⇒ đỏ
ok - `assert_eq!(f(x), f(x))` (tất định) ⇒ xanh
ok - `assert_ne!(x, x)` ⇒ đỏ
ok - `#[ignore]` ⇒ đỏ
ok - `#[test]` không khẳng định ⇒ đỏ
ok - `#[test]` chỉ có `.unwrap()` ⇒ đỏ
ok - `#[test]` không khẳng định + allowlist khớp ⇒ xanh
ok - allowlist `#[test]` mục ⇒ đỏ
ok - `#[test]` gọi trợ giúp transitive ⇒ xanh
ok - `#[test]` gọi trợ giúp trong HELPER_ALLOW ⇒ xanh
ok - `let _ =` nuốt `Result` ⇒ đỏ
ok - `let _ =` + allowlist khớp ⇒ xanh
ok - allowlist mục ⇒ đỏ
ok - `let _ = foo().unwrap()` ⇒ xanh
ok - `panic!` vô điều kiện ⇒ đỏ, có điều kiện ⇒ xanh
ok - chỉ quét `#[cfg(test)]` trong src/
ok - `#[cfg(all(test, …))]` ⇒ vẫn quét
ok - `#[should_panic]` ⇒ xanh
ok - cây không có test ⇒ đỏ (fail-closed)
# selftest: 21/21 ok
```

### 4.2 Quét cây hiện tại — 0 vi phạm

`v2/agent/JOURNAL/kt163-logs/kt163-scan.log`:

```
# tệp có phạm vi test: 124
# hàm `#[test]`: 944
# allowlist `let _ =`: 20 mục, khớp 20
# trợ giúp khai báo ngoài tệp: 0
# allowlist `#[test]` không khẳng định: 0 mục, khớp 0
# check-test-teeth: errors=0 warnings=0
```

### 4.3 Đột biến: test giả ⇒ `v2 check` ĐỎ

Thêm vào cuối `v2/lib/vifast-pte/tests/kt35_pte.rs`:

```rust
#[test]
fn kt163_dot_bien_test_gia() {
    assert!(true);
}
```

`v2/agent/JOURNAL/kt163-logs/kt163-check-mutation.log`:

```
--- test có răng (KT-163)
E E_CONST_TRUE v2/lib/vifast-pte/tests/kt35_pte.rs:274: `assert!(true)` — khẳng định hằng-đúng, test không có răng
--- test có răng (KT-163): FAIL
...
v2 check: 2 bước FAIL
```

(exit 1. Bước thứ hai đỏ là `clippy` — `assertions_on_constants` cũng bắt `assert!(true)`;
hai lớp độc lập cùng đỏ, không phải cùng một phép kiểm.)

### 4.4 Gỡ đột biến ⇒ `v2 check` PASS

`v2/agent/JOURNAL/kt163-logs/kt163-check-final.log`:

```
--- test có răng (KT-163)
# tệp có phạm vi test: 124
# hàm `#[test]`: 944
# allowlist `let _ =`: 20 mục, khớp 20
# trợ giúp khai báo ngoài tệp: 0
# allowlist `#[test]` không khẳng định: 0 mục, khớp 0
# check-test-teeth: errors=0 warnings=0
--- test có răng (KT-163): OK
--- selftest test có răng (KT-163)
...
# selftest: 21/21 ok
--- selftest test có răng (KT-163): OK
...
v2 check: PASS (L1 chạy trong gate --fast)
```

### 4.5 Bốn đột biến lớp khác (ngoài `v2 check`, chạy trực tiếp công cụ)

`v2/agent/JOURNAL/kt163-logs/kt163-mutations.log`:

```
== M1: assert!(true) ==
  exit=1  1 errors
    E E_CONST_TRUE v2/lib/vifast-pte/tests/kt35_pte.rs:206: `assert!(true)` — khẳng định hằng-đúng, test không có răng
== M2: #[ignore] ==
  exit=1  1 errors
    E E_IGNORE v2/lib/vifast-pte/tests/kt35_pte.rs:190: `#[ignore]` bị cấm (AGENTS.md §4.3) — sửa test hoặc bỏ, không né
== M3: test khong con khang dinh ==
  exit=1  1 errors
    E E_NO_ASSERT v2/lib/vifast-pte/tests/kt35_pte.rs:190: `#[test] fn pte_la_newtype_u64_tho` không có token khẳng định nào và không gọi trợ giúp đã biết — test xanh-giả
== M4: let _ = nuot Result ==
  exit=1  1 errors
    E E_LET_UNDERSCORE v2/lib/vifast-fs/src/tests/part2.rs:558: `let _ = lookup(…)` nuốt `Result` (kiểu trả về: Option<TlbEntry>) — kiểm kết quả hoặc thêm allowlist kèm lý do
== M5: assert_eq!(f(x), f(x)) tat dinh (KHONG duoc do) ==
  exit=0  0 errors
== M6: sach tro lai ==
  exit=0  0 errors
```

M5 là **đối chứng âm của luật (i)**: mẫu tất định **không** bị bắt.

## 5. Allowlist lớp (d) — 20 mục, đã rà từng mục

Tất cả đều là **fuzz/property** hoặc **"không panic"** — lỗi là **đầu vào** của phép đo, răng
nằm ở bất biến kiểm sau mỗi bước:

| Tệp | Hàm test | # | Lý do |
|---|---|---|---|
| `vifast-fs/src/tests/part2.rs` | `crash_every_write_for_mutations` | 1,2 | đếm số lần ghi / vòng cắt điện; răng ở assert kích thước sau |
| " | `crash_near_full_small_image_keeps_all_or_nothing` | 1 | cắt điện giữa lúc ghi; răng ở `size ∈ {0, 3}` |
| " | `corrupt_dir_entry_length_does_not_panic` | 1,2 | ca "không được panic" trên entry hỏng |
| " | `corrupt_dir_size_beyond_block_slots_does_not_panic` | 1,2 | ca "không được panic"; răng ở `blocks_used()` |
| `vifast-sim/tests/kt19_sched.rs` | `mot_nghin_seed_64_op_cung_seed_cung_trace_bat_bien_giu` | 1,2 | fuzz 1.000 seed; răng ở bất biến sau mỗi bước |
| `vifast-sim/tests/kt20_budget.rs` | `invariant_budget_moi_chu_ky_thang_hang_khong_vuot` | 1..7 | fuzz budget; răng ở bất biến budget |
| `vifast-sim/tests/kt23_k1b.rs` | `ma_tran_46_cap_va_73_cap_cam` | 1 | dọn slot tạm của ma trận |
| `vifast-sim/tests/kt37_pagetable.rs` | `map_qua_cap_loi_co_ban` | 1 | thăm dò "không panic" trên object đã chết |
| `vifast-sim/tests/kt40_tlb.rs` | `property_10k_tlb_shootdown` | 1 | fuzz 10k vòng; răng ở `check_tlb` + 0 vi phạm |
| `vifast-sim/tests/kt60_core_parity.rs` | `kt63_parity_chuyen_cap_qua_ipc` | 1 | cố ý bỏ kết quả từng op để pin **bộ đếm**; răng ở `assert_eq!(cm.cap_transfer_stats(), (9, 2, 8))` |

`HELPER_ALLOW` và `ALLOW_NO_ASSERT` hiện **rỗng** — cây không cần. Cơ chế vẫn có mặt và có ca
`--selftest` cho cả hai (khớp ⇒ xanh, mục chết ⇒ đỏ), để lần sau có ca hợp lệ thì khai báo
tường minh thay vì nới luật.

## 6. Sửa trên cây (2 vi phạm thật)

1. `v2/lib/vifast-pte/tests/kt35_pte.rs:203` — `assert_eq!(pte, pte)` ⇒
   `assert_eq!(pte, Pte::from_raw(0x8000_0000_0000_0fff))`. Vế phải dựng lại từ cùng bit nên
   thật sự kiểm `PartialEq` (trước đây chỉ so một biến với chính nó).
2. `v2/sim/vifast-sim/src/vspace.rs:284` — `assert_eq!(info, info)` ⇒ so với một `VSpaceInfo`
   dựng lại từ cùng bốn trường.

Không nới luật nào; không thêm `#[ignore]`/`#[should_panic]`; không retry.

## 7. Còn thiếu / nợ đã biết

1. **Lớp (d) chỉ bắt khi phân giải được kiểu trả về trong cây.** Hàm trả `Result` khai ở
   crate ngoài (hoặc qua trait) không bị bắt. Đây là **bỏ sót**, không phải báo oan. Muốn phủ
   hết phải có kiểu từ `cargo check` — ngoài ngân sách task này.
2. **Lớp (d) chỉ quét thân `#[test]`**, không quét hàm trợ giúp mà test gọi. Một `let _ =`
   nuốt `Result` nằm trong trợ giúp vẫn lọt. Đề bài ghi "trong thân test" nên giữ đúng phạm vi.
3. **Lớp (c) không thấy trợ giúp ở tệp khác** trừ khi khai trong `HELPER_ALLOW` (hiện rỗng).
   Cây hiện không cần; nếu lần sau có, khai tường minh.
4. **`assert!(1 == 1)` chỉ bắt dạng hai hằng số nguyên giống hệt.** Các dạng hằng-đúng khác
   (`assert!(2 > 1)`, `assert!(!"".is_empty())`…) không bắt — cố ý, để không rơi vào heuristic.
5. **`#[should_panic]` không bị cấm** (chỉ `#[ignore]` bị cấm). Một test `#[should_panic]`
   không assert gì vẫn được tính là có răng. Nếu cần siết, thêm lớp (e) ở task sau.
6. **Chưa chạy `gate --fast`** — đề bài cấm. `v2 check` PASS là bằng chứng của task này.

## 8. Hiệu năng

Bản đầu quét cây **11,7 s** (che bình luận/chuỗi 3 lần/tệp + `str.count("\n", 0, pos)` O(n)
cho **mỗi** assert). Sau khi gom phân tích một lần (`ParsedFile`) + chỉ số dòng bằng `bisect`
+ nhảy ngoặc bằng `str.find`: **2,6 s**. `--selftest` **0,3 s**. Đã đối chứng `mask_source`
mới cho **giống hệt từng byte** bản cũ trên **291/291** tệp `.rs` của `v2/`.

## 9. Tệp chạm

- **mới** `v2/tools/check-test-teeth.py` (máy kiểm + 21 ca `--selftest`)
- **mới** `v2/agent/JOURNAL/kt163-teeth.md` (tệp này) + `v2/agent/JOURNAL/kt163-logs/` (5 log)
- `v2/bin/v2` — thêm 2 `run_step` vào `do_check` (máy kiểm + selftest)
- `v2/lib/vifast-pte/tests/kt35_pte.rs` — sửa 1 hằng-đúng
- `v2/sim/vifast-sim/src/vspace.rs` — sửa 1 hằng-đúng
