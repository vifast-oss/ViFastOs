# KT-170 — mở rộng máy kiểm "test có răng": lớp (e) `#[should_panic]` trần + bản đồ trợ giúp toàn cây

**Ngày**: 2026-11-19 · **Agent 2 (L13)** · nhánh `night2/L13-teeth2` · HEAD trước task: `25306b3f`
**Nguồn nợ**: [`kt163-teeth.md`](kt163-teeth.md) §7 mục **3** (lớp (c) không thấy trợ giúp ở tệp
khác trừ khi khai `HELPER_ALLOW`) và mục **5** (`#[should_panic]` không bị cấm).

## 0. Việc được giao (nguyên văn, trích)

> 1. **Lớp (e) `E_SHOULD_PANIC_BARE`**: `#[should_panic]` **không** có `expected = "…"` ⇒ ĐỎ
>    (bắt panic bất kỳ là răng yếu, dễ che lỗi khác). `#[should_panic(expected = "…")]` ⇒
>    **XANH** (có răng thật). Allowlist tường minh nếu cây có ca hợp lệ.
> 2. **Trợ giúp ở tệp khác**: dựng **bản đồ toàn cây** hàm không-phải-`#[test]` có token khẳng
>    định (theo `mod`/`use`/đường dẫn), để test gọi trợ giúp ở tệp khác được tính là có răng
>    **mà không cần** `HELPER_ALLOW`. Giữ `HELPER_ALLOW` làm đường dự phòng.
> 3. **Selftest**: thêm ca cho (e) và (2) — log giả `#[should_panic]` trần ⇒ ĐỎ;
>    `#[should_panic(expected=…)]` ⇒ XANH; test gọi trợ giúp ở tệp khác ⇒ XANH. Giữ 22 ca cũ xanh.
> 4. Chạy trên cây hiện tại: nếu lớp (e) bắt được ca thật, **sửa** (thêm `expected`) hoặc
>    allowlist kèm lý do — không nới luật.
> 5. **Đột biến bắt buộc**: thêm `#[should_panic]` trần vào một test ⇒ `v2 check` **ĐỎ**; gỡ ⇒ xanh.

## 1. Kết luận trước (đọc nhanh)

- **Lớp (e) — ĐÓNG.** `#[should_panic]` trần ⇒ `E_SHOULD_PANIC_BARE`; `expected = "…"` (chuỗi
  **khác rỗng**) ⇒ xanh. `expected = ""` (khớp **mọi** thông điệp) tính là **trần** — nếu không,
  đó là lỗ hổng một dòng. Allowlist `ALLOW_SHOULD_PANIC_BARE` (hiện **rỗng**) + kiểm mục chết.
- **Trợ giúp tệp khác — ĐÓNG.** `build_global_helpers` dựng bản đồ **toàn cây** (295 tệp `.rs`)
  hàm không-phải-`#[test]` có răng, bao đóng bắc cầu, phân giải thêm bí danh `use … as …`.
  Cây hiện có **319** tên trong bản đồ. `HELPER_ALLOW` giữ làm đường dự phòng cho tên mơ hồ.
- **Selftest 31/31** (22 → 31; **9 ca mới**). Ca (15) cũ **phải đổi** từ `#[should_panic]` trần
  sang `expected = …` — xem §4.2, đây là ca duy nhất bị đổi và lý do là luật mới **ngược** nó.
- **Quét cây: `errors=0`** (977 `#[test]`, 127 tệp có phạm vi test). Cây **không có**
  `#[should_panic]` nào (0/977) ⇒ lớp (e) không bắt được ca thật nào để sửa; allowlist rỗng.
- **Đột biến**: thêm `#[should_panic]` trần vào `kt35_pte.rs:33` ⇒ `v2 check` **ĐỎ**
  (`1 bước FAIL`, `E_SHOULD_PANIC_BARE`); gỡ ⇒ **PASS**.
- **`./v2/bin/v2 check` PASS** (1m29s, log `kt170-logs/kt170-check-final.log`).

## 2. Lớp (e) — `#[should_panic]` trần

### 2.1 Vì sao "bắt panic bất kỳ" là răng yếu

`#[should_panic]` không có `expected` chỉ đòi **một** panic nào đó. Test vẫn xanh nếu panic đến
từ chỗ **không liên quan**: tràn số, `unwrap()` hỏng ở dòng khác, `index out of bounds`… Đúng
lớp xanh-giả mà KT-163 sinh ra để bắt: test "xanh" nhưng không chứng minh điều nó tuyên bố.
`expected = "…"` khẳng định **đúng** panic được chờ ⇒ răng thật, giữ xanh.

Cú pháp là bằng chứng, không suy luận: chỉ đọc thuộc tính, không đoán theo tên test.

### 2.2 Cài đặt

- `SHOULD_PANIC_EXPECTED_RE = ^should_panic\s*\(\s*expected\s*=\s*"[^"]+"` — chạy trên văn bản
  **đã che** (`mask_source`). Nội dung chuỗi bị che thành dấu cách nhưng **độ dài giữ nguyên**,
  nên `""` (không có ký tự nào giữa hai nháy) **không** khớp, còn `"x"` (một dấu cách) khớp.
  Nhờ vậy phân biệt được `expected = ""` mà **không** cần đọc lại nguồn thô.
- `Fn.should_panic_has_expected` — thuộc tính mới, cạnh `is_should_panic`.
- Trong `scan`, sau lớp (c): nếu `f.is_should_panic and not f.should_panic_has_expected` ⇒
  `E_SHOULD_PANIC_BARE` (trừ khi có trong `ALLOW_SHOULD_PANIC_BARE`).
- **Quan hệ với lớp (c)**: `#[should_panic]` (cả trần) **vẫn** tính là răng cho (c) như trước —
  nên một test trần **không** bị báo hai lỗi, chỉ một `E_SHOULD_PANIC_BARE`. Lớp (e) là phép đo
  **độc lập**, không đụng ngưỡng của (c).
- Allowlist `ALLOW_SHOULD_PANIC_BARE: Dict[(tệp, tên test), (dòng, lý do)]` — **rỗng**, kèm kiểm
  `E_ALLOW_STALE` (mục chết ⇒ ĐỎ), cùng khuôn với hai allowlist còn lại.

### 2.3 Cây hiện tại: 0 ca

`grep -rn "should_panic" --include=*.rs v2/` chỉ ra **một** dòng, và đó là **doc-comment** ở
`v2/sim/vifast-sim/src/kobj.rs:5488` (đã bị `mask_source` che nên không tính). **0/977** test
dùng `#[should_panic]` ⇒ không có ca thật để sửa, allowlist để rỗng. Đột biến ở §5.3 chứng minh
lớp (e) **cắn** khi có ca.

## 3. Bản đồ trợ giúp toàn cây

### 3.1 Cài đặt

`build_global_helpers(parsed)` — chạy **một lần** trên toàn bộ tệp `.rs` đã phân tích:

1. Với **mọi** `fn` không phải `#[test]` trong cây: `direct` = thân có token khẳng định (hoặc
   `panic!` **có điều kiện**), `calls` = các tên được gọi (`CALL_NAME_RE`, có turbofish).
2. Khởi tạo `teeth` = các tên mà **mọi** định nghĩa đều `direct`.
3. Lặp tới điểm bất động: thêm tên `n` nếu **mọi** định nghĩa của `n` hoặc `direct`, hoặc gọi
   một tên đã trong `teeth`. Vòng thuần (gọi nhau mà không có assert) **không** bao giờ vào.
4. Trong `scan`, lớp (c) thử theo thứ tự: `HELPER_ALLOW` → trợ giúp **cùng tệp** (bắc cầu, như
   cũ) → **bản đồ toàn cây**. Đường thứ ba là cái mới; hai đường đầu giữ nguyên hành vi.

### 3.2 Vì sao đòi **mọi** định nghĩa cùng tên

Tên trùng giữa các tệp là chuyện thường (`kiem`, `setup`, `check`…). Nếu chỉ cần **một** định
nghĩa có răng thì một test gọi trợ giúp **không** răng trùng tên sẽ được xanh oan — đúng kiểu
fail-open mà KT-163 cấm. Đòi **tất cả** là fail-closed: tên **mơ hồ** ⇒ không tính, người viết
khai `HELPER_ALLOW` nếu ca thật. Có ca selftest riêng cho cả hai chiều (§4).

### 3.3 Bí danh `use … as …`

`build_use_aliases` đọc các câu `use` **trong tệp có phạm vi test** và dựng `bí danh -> tên gốc`
(`use common::kiem as kt;` ⇒ `kt -> kiem`). Trước khi tra bản đồ, tên gọi được mở rộng bí danh.
`use x::*` (glob) **không** phân giải được ⇒ bỏ qua; bỏ qua chỉ làm **đỏ oan** (an toàn), không
làm xanh giả. Chỉ quét 127 tệp trong phạm vi test thay vì 295 tệp (đo: nhanh hơn ~0,2 s).

### 3.4 Đo trên cây thật

| Số đo | Giá trị |
|---|---|
| Tên trong bản đồ toàn cây | **319** |
| Test **cần** đường dự phòng này (không có răng nào khác) | **0** |
| Test gọi tên chéo tệp có trong bản đồ (đo riêng) | **285** |
| … trong đó tên chỉ định nghĩa **ngoài** phạm vi test (hàm sản phẩm) | **277** |
| … trong đó có tên định nghĩa **trong** phạm vi test | **16** (chỉ `sched_pick`) |

**0 test cần** nghĩa là: trên cây hiện tại bản đồ **không** làm xanh thêm test nào (trước và sau
đều `errors=0`) — nó là **hạ tầng cho tương lai**, đúng như nợ §7.3 mô tả ("cây hiện không cần").

### 3.5 Đánh đổi đã biết — ghi rõ để reviewer quyết

Bản đồ phủ **toàn cây** (đúng nguyên văn đề bài: "hàm không-phải-`#[test]`"), nên nó cũng nhận
hàm **sản phẩm** có `debug_assert!`/`assert!` bên trong (277 test ở bảng trên gọi loại đó). Hệ
quả: một test **không** có assert mà gọi `fsck()`/`kill()`/`page_table_map()`… sẽ được tính là
có răng nhờ assert **của sản phẩm**.

Đây **không** phải lớp lỗi mới: bản đồ **cùng tệp** của KT-163 (`helper_teeth`) đã tính **mọi**
hàm không-phải-`#[test]` trong tệp, kể cả hàm sản phẩm — bản đồ toàn cây chỉ là bản tổng quát
hoá theo đúng yêu cầu. Nếu Agent 0 muốn siết: giới hạn bản đồ vào **phạm vi test** (tệp
`tests/` + khối `#[cfg(test)]`) là **một điều kiện** trong `build_global_helpers`; khi đó 277
test trên không còn được tính, còn 16 test dùng trợ giúp test thật vẫn xanh. Tôi **không** tự
chọn siết vì đề bài ghi rõ "bản đồ toàn cây" — ghi ở đây để quyết định có dữ liệu.

## 4. Selftest: 22 → 31 ca

### 4.1 Chín ca mới

| # | Ca | Kỳ vọng |
|---|---|---|
| 17 | `#[should_panic]` trần (thân **có** assert, để chứng minh (e) độc lập (c)) | ĐỎ `E_SHOULD_PANIC_BARE` |
| 18 | `#[should_panic(expected = "")]` | ĐỎ (khớp mọi thông điệp) |
| 19 | `#[should_panic]` trần + allowlist khớp | XANH |
| 20 | allowlist `#[should_panic]` **mục** | ĐỎ `E_ALLOW_STALE` |
| 21 | test gọi trợ giúp ở **tệp khác** (`tests/common/mod.rs`) | XANH, **không** `HELPER_ALLOW` |
| 22 | đối chứng **âm**: trợ giúp tệp khác **không** răng | ĐỎ `E_NO_ASSERT` |
| 23 | bắc cầu **ba tệp**: test → `boc` (tệp B) → `kiem` (tệp C, có assert) | XANH |
| 24 | bí danh `use common::kiem_ngoai as kt;` rồi `kt(3)` | XANH |
| 25 | tên **mơ hồ** (hai định nghĩa, một không răng) | ĐỎ (fail-closed) |

Ca 22 và 25 là **đối chứng âm** cố ý: nếu ai đó "vá" bản đồ thành "mọi lời gọi chéo tệp đều
xanh", hai ca này **đỏ** ngay. Ca 23 chứng minh bao đóng bắc cầu **qua tệp**, không chỉ trong
một tệp.

### 4.2 Ca (15) cũ **phải đổi** — và vì sao

Ca (15) của KT-163 là: `#[should_panic]` **trần** ⇒ **xanh**. Luật mới nói **ngược lại** (trần
⇒ đỏ), nên không thể giữ nguyên mà vẫn "22 ca cũ xanh". Tôi đổi ca (15) sang dạng
`#[should_panic(expected = "chia cho 0")]` ⇒ xanh — **giữ nguyên ý định** của ca (ý: panic là
một dạng khẳng định, nên test không cần assert), chỉ đổi sang dạng có răng thật. Dạng trần
được **chuyển** thành ca (17) với kỳ vọng ĐỎ. **21 ca còn lại không sửa một chữ.**

## 5. Bằng chứng (nguyên văn)

### 5.1 `--selftest` — 31/31

`v2/agent/JOURNAL/kt170-logs/kt170-selftest.log` (đuôi):

```text
ok - `#[should_panic(expected = …)]` ⇒ xanh
ok - cây không có test ⇒ đỏ (fail-closed)
ok - `#[should_panic]` trần ⇒ đỏ
ok - `#[should_panic(expected = "")]` ⇒ đỏ (khớp mọi thông điệp)
ok - `#[should_panic]` trần + allowlist khớp ⇒ xanh
ok - allowlist `#[should_panic]` mục ⇒ đỏ
ok - `#[test]` gọi trợ giúp ở tệp khác ⇒ xanh (không cần HELPER_ALLOW)
ok - `#[test]` gọi trợ giúp tệp khác **không** răng ⇒ đỏ
ok - trợ giúp tệp khác **bắc cầu** ⇒ xanh
ok - `use … as …` ⇒ phân giải tên gốc
ok - trợ giúp tệp khác **tên mơ hồ** ⇒ đỏ (fail-closed)
# selftest: 31/31 ok
```

### 5.2 Quét cây — `errors=0`

`v2/agent/JOURNAL/kt170-logs/kt170-scan.log`:

```text
# tệp có phạm vi test: 127
# hàm `#[test]`: 977
# allowlist `let _ =`: 20 mục, khớp 20
# trợ giúp khai báo ngoài tệp: 0
# bản đồ trợ giúp toàn cây: 319 tên có răng, 0 test dùng đường này
# allowlist `#[test]` không khẳng định: 0 mục, khớp 0
# allowlist `#[should_panic]` trần: 0 mục, khớp 0
# check-test-teeth: errors=0 warnings=0
```

### 5.3 Đột biến — `v2 check` **ĐỎ**

Đột biến: thêm `#[should_panic]` trần vào `v2/lib/vifast-pte/tests/kt35_pte.rs` (test
`hang_so_q1_dung_gia_tri_chot`, dòng 33). `v2/agent/JOURNAL/kt170-logs/kt170-check-mutation.log`:

```text
--- test có răng (KT-163)
# tệp có phạm vi test: 127
# hàm `#[test]`: 977
# allowlist `let _ =`: 20 mục, khớp 20
# trợ giúp khai báo ngoài tệp: 0
# bản đồ trợ giúp toàn cây: 319 tên có răng, 0 test dùng đường này
# allowlist `#[test]` không khẳng định: 0 mục, khớp 0
# allowlist `#[should_panic]` trần: 0 mục, khớp 0
E E_SHOULD_PANIC_BARE v2/lib/vifast-pte/tests/kt35_pte.rs:34: `#[test] fn hang_so_q1_dung_gia_tri_chot` có `#[should_panic]` **không** `expected = "…"` — bắt panic bất kỳ là răng yếu, dễ che lỗi khác; thêm `expected` hoặc allowlist kèm lý do
# check-test-teeth: errors=1 warnings=0
--- test có răng (KT-163): FAIL
...
v2 check: 1 bước FAIL
```

`git checkout -- v2/lib/vifast-pte/tests/kt35_pte.rs` (gỡ đột biến) ⇒ chạy lại §5.4 **PASS**.

### 5.4 `./v2/bin/v2 check` — PASS

`v2/agent/JOURNAL/kt170-logs/kt170-check-final.log`:

```text
--- test có răng (KT-163)
# tệp có phạm vi test: 127
# hàm `#[test]`: 977
# allowlist `let _ =`: 20 mục, khớp 20
# trợ giúp khai báo ngoài tệp: 0
# bản đồ trợ giúp toàn cây: 319 tên có răng, 0 test dùng đường này
# allowlist `#[test]` không khẳng định: 0 mục, khớp 0
# allowlist `#[should_panic]` trần: 0 mục, khớp 0
# check-test-teeth: errors=0 warnings=0
--- test có răng (KT-163): OK
--- selftest test có răng (KT-163): OK
...
v2 check: PASS (L1 chạy trong gate --fast)
```

## 6. Còn thiếu / nợ đã biết

1. **Bản đồ theo tên, không theo đường dẫn module đầy đủ.** Hai hàm cùng tên ở hai module khác
   nhau ⇒ mơ hồ ⇒ **không** tính (fail-closed). Muốn phân giải chính xác phải có `cargo check`
   / rust-analyzer — ngoài ngân sách. `use x::*` (glob) cũng không phân giải.
2. **Bản đồ nhận cả hàm sản phẩm** (277 test đo được) — xem §3.5. Đây là hệ quả của nguyên văn
   "toàn cây"; muốn siết thì giới hạn phạm vi test (một điều kiện).
3. **Lớp (e) chỉ xét `#[test]` trong phạm vi quét** (tệp `tests/` + khối `#[cfg(test)]`), giống
   lớp (c); `#[should_panic]` ngoài phạm vi không bị bắt — nhất quán với KT-163.
4. **Lớp (d) vẫn chỉ quét thân `#[test]`** (nợ KT-163 §7.2) — không đổi trong task này.
5. **Không chạy `gate --fast`** — đề bài cấm. `v2 check` PASS là bằng chứng của task này.

## 7. Hiệu năng

Quét cây: **2,64 s → 3,58 s** (+0,94 s, +36 %) vì thêm hai lượt toàn cây (`build_global_helpers`
~0,6 s, `build_use_aliases` ~0,2 s). So với `v2 check` **1m29s** thì phần tăng là **~1 %** —
không cần tối ưu thêm. `--selftest` vẫn **~0,3 s**.

## 8. Tệp chạm

- `v2/tools/check-test-teeth.py` — lớp (e) + bản đồ trợ giúp toàn cây + bí danh `use … as …` +
  9 ca selftest (22 → 31) + docstring.
- **mới** `v2/agent/JOURNAL/kt170-teeth2.md` (tệp này) + `v2/agent/JOURNAL/kt170-logs/` (4 log:
  selftest, scan, check-mutation, check-final).
- **Không** sửa `spec/`, `docs/`, `v2/bin/v2` (đã nối từ KT-163), không sửa test nào trên cây.
