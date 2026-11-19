# KT-203 — review độc lập KT-182: siết bản đồ trợ giúp **cùng tệp** về phạm vi test

**Ngày**: 2026-11-20 · **Reviewer**: L41 (sub-agent Agent 1) · **Nhánh**: `night3/L41-review182`
**Commit được review**: `a8abe344` — `[KT-182] siết bản đồ trợ giúp cùng tệp về phạm vi test (đo 0 test ảnh hưởng, 0 allowlist)`
**Worktree**: `/private/tmp/night3-L41-review182` (tạo từ `a8abe344`; **không** sửa checkout chính)
**Vai**: reviewer độc lập — tìm chỗ sai, không xác nhận. **Không** sửa `v2/tools/check-test-teeth.py`.

## 0. Kết luận trước (đọc nhanh)

**ĐẠT CÓ ĐIỀU KIỆN.**

Bản vá **đúng về chức năng** và **số đo của tác giả đúng**: phép đo độc lập của tôi (tự viết
parser, **không** import checker, **không** dùng `measure-local-map.py`) cho **đúng từng con
số** 1019 / 996 / 23 / 0 / 155 / 0 / 80 / 173 / 3. Checker TRƯỚC vs SAU trên cây thật:
**0 lỗi đổi**. Đột biến trên cây thật (thêm test chỉ gọi `Machine::add_cnode`): TRƯỚC xanh
oan, SAU đỏ `E_NO_ASSERT` — bản vá **cắn**. Selftest **36/36**.

Ba điều kiện (không chặn bản vá, nhưng phải sửa/ghi nhận):

1. **Ca selftest 35 và 36 KHÔNG có răng** (trung bình): cả hai vẫn xanh khi **xoá sạch** bản
   đồ trợ giúp cùng tệp, vì bản đồ toàn cây (KT-170) chứng nhận cùng tên. Chúng **không**
   kiểm điều tuyên bố ("trợ giúp cùng tệp trong `#[cfg(test)]`/`tests/` vẫn tính"). Fixture C
   của tác giả (§6.1 journal) cùng bệnh.
2. **Hàm `#[cfg(test)]` đặt trên item (`fn`/`impl`) bị loại oan** (trung bình–thấp): bản vá
   dùng `in_ranges` — chỉ biết `#[cfg(test)] mod` — nên trợ giúp test-only kiểu
   `#[cfg(test)] fn helper()` bị loại khỏi bản đồ cùng tệp ⇒ test gọi nó bị báo
   `E_NO_ASSERT` oan. Cây thật hiện **0 ảnh hưởng** (11 item `cfg(test)`, chỉ `alloc_block`
   có token và nó là `cfg(any(feature = "heap", test))` = mã sản phẩm), nhưng mẫu này **có**
   trong repo (`mcs.rs`, `tlb.rs`, `tx.rs`).
3. **Docstring ghi "155 test gọi hàm sản phẩm cùng tệp"** (thấp): con số là đo **theo tên**
   (upper bound), không phân giải kiểu; journal §7.3 có ghi, docstring không. Kết luận
   "0 test chỉ có răng nhờ hàm sản phẩm" **vẫn đúng** vì 155 là over-approximation.

Ngoài ra: `v2 check` trong worktree L41 **FAIL** ở bước `luật cấu trúc` do **T-INFRA**
(SIGTRAP bash 3.2), **không** liên quan KT-182 — `./v2/bin/v2 rules` chạy trực tiếp **PASS**,
bước `test có răng` **OK**. Log tác giả (`kt182-check-final.log`) PASS.

## 1. Số đo — đối chiếu độc lập

### 1.1 Phép đo độc lập

Script: [`kt203-logs/independent-measure.py`](kt203-logs/independent-measure.py) — **không**
import `v2/tools/check-test-teeth.py`, **không** dùng `measure-local-map.py`. Tự che bình
luận/chuỗi (kể cả raw string), tự tìm `fn`, tự dựng phạm vi test (`tests/`, `#[cfg(test)] mod`,
`mod X;`, `#![cfg(test)]`), tự dựng hai bản đồ cùng tệp (cũ = mọi hàm; mới = chỉ hàm trong
phạm vi test) và bản đồ toàn cây (luật "mọi định nghĩa cùng tên đều có răng").

Lệnh:

```bash
python3 v2/agent/JOURNAL/kt203-logs/independent-measure.py \
        --root /private/tmp/night3-L41-review182 \
        --json /tmp/kt203-independent.json
```

Log: [`kt203-logs/independent-measure.log`](kt203-logs/independent-measure.log).

### 1.2 Đối chiếu

| Số đo | Tác giả | L41 độc lập | Kết luận |
|---|---|---|---|
| `#[test]` trong phạm vi | 1019 | **1019** | khớp |
| … có token trong thân (`own`) | 996 | **996** | khớp |
| … dựa trợ giúp **cùng tệp** (`local`) | 23 | **23** | khớp |
| … trong đó trợ giúp **trong phạm vi test** | 23 | **23** | khớp |
| … trong đó trợ giúp là **hàm sản phẩm cùng tệp** | 0 | **0** | khớp |
| … dựa bản đồ **toàn cây** (`global`) | 0 | **0** | khớp |
| … **không** có răng (`none`) | 0 | **0** | khớp |
| Test **chỉ** có răng nhờ hàm sản phẩm cùng tệp (Q1) | 0 | **0** | khớp |
| Test gọi tên hàm sản phẩm cùng tệp (theo tên) | 155 | **155** | khớp |
| … trong đó **không** có token riêng | 0 | **0** | khớp |
| Test đổi XANH → ĐỎ sau siết | 0 | **0** | khớp |
| Tên trong bản đồ toàn cây | 80 | **80** | khớp |
| Tên có ≥2 định nghĩa trong phạm vi test cùng tệp | 173 | **173** | khớp |
| Tên vừa có định nghĩa sản phẩm vừa test cùng tệp | 3 | **3** | khớp |

**Kết luận A**: các con số **1019 / 0 / 155** của tác giả **ĐÚNG**. Không tìm được dù chỉ
một test chỉ có răng nhờ hàm sản phẩm cùng tệp.

### 1.3 Đối chứng ngược — checker TRƯỚC vs SAU trên cây thật

```bash
git show a8abe344~1:v2/tools/check-test-teeth.py > /tmp/old-teeth.py
python3 /tmp/old-teeth.py --root /private/tmp/night3-L41-review182
python3 v2/tools/check-test-teeth.py
```

Log: [`kt203-logs/old-vs-new.log`](kt203-logs/old-vs-new.log).

- TRƯỚC: `errors=0`, 1019 test, 80 tên toàn cây, **không** có dòng "cùng tệp".
- SAU: `errors=0`, 1019 test, 80 tên toàn cây, **23** test dùng bản đồ cùng tệp.
- **Diff lỗi: rỗng cả hai chiều** ⇒ không test nào bị báo oan, không test nào đổi trạng thái.

### 1.4 Đột biến trên cây thật

Thêm vào `#[cfg(test)] mod tests` của `v2/lib/vifast-model/src/machine.rs`:

```rust
#[test]
fn kt203_dot_bien_chi_goi_san_pham_cung_tep() {
    let mut m = Machine::new();
    let _a = m.add_cnode(4);   // hàm sản phẩm cùng tệp, có assert!
}
```

Log: [`kt203-logs/real-tree-mutation.log`](kt203-logs/real-tree-mutation.log).

| Checker | Kết quả |
|---|---|
| TRƯỚC (`a8abe344~1`) | **XANH** (`ok=True`, 0 lỗi) — xanh oan |
| SAU (`a8abe344`) | **ĐỎ** `E_NO_ASSERT v2/lib/vifast-model/src/machine.rs:1040` |

Đã gỡ đột biến; `git status` sạch. **Kết luận**: bản vá cắn trên cây thật, không chỉ fixture.

## 2. Tính đúng của bản vá

### 2.1 Điều kiện `if not (is_whole or in_ranges(f.start, ranges)): continue`

Đọc kỹ `scan()`: `is_whole` = tệp dưới `tests/` hoặc tệp `mod X;` của `#[cfg(test)]`;
`ranges` = các khoảng thân `#[cfg(test)] mod X { … }`. Điều kiện áp cho **cả** `helper_teeth`
**và** `calls_of` — đúng: nếu chỉ siết `helper_teeth` mà để `calls_of` nguyên, bao đóng bắc
cầu vẫn đi qua hàm sản phẩm (test → helper test → hàm sản phẩm). Tác giả đã siết cả hai.

### 2.2 Hàm trong phạm vi test bị loại oan?

| Mẫu | TRƯỚC | SAU | Kết luận |
|---|---|---|---|
| `#[cfg(test)] mod tests { … }` | xanh | xanh | OK |
| `#[cfg(all(test, feature = "x"))] mod` | xanh | xanh | OK |
| `#[cfg(test)] mod tests;` trỏ tệp khác | xanh | xanh | OK |
| `#[cfg(test)] mod` lồng trong `mod` khác | xanh | xanh | OK |
| hàm lồng trong hàm `#[test]` | xanh | xanh | OK |
| **`#[cfg(test)] fn helper()` (item, không phải mod)** | **xanh** | **ĐỎ oan** | **F2** |
| **`#[cfg(test)] impl S { fn helper() }`** | **xanh** | **ĐỎ oan** | **F2** |
| `#[cfg(feature = "ktest")] mod` (test-only feature) | xanh | **ĐỎ oan** | **F2 (cùng lớp)** |
| `#[cfg(test)] pub mod tests` | E_NO_TESTS | E_NO_TESTS | tiền tại (F7) |

Log: [`kt203-logs/edge-fixtures.log`](kt203-logs/edge-fixtures.log).

**F2** là lỗi **mới** do bản vá: `parse_cfg_mods` chỉ nhận `#[cfg(test)] mod`, không nhận
`#[cfg(test)]` trên `fn`/`impl`. Trước bản vá, bản đồ cùng tệp nhận **mọi** hàm nên trợ giúp
test-only item-level vẫn được tính; sau bản vá, nó bị loại. Cây thật hiện **0 ảnh hưởng**
(11 item `cfg(test)` = 10 hàm + 1 `use`: `window_words`, `crc32_bitwise`,
`crc32_continue_bitwise`, `test_enqueue`, `test_woken`, `test_release`, `slot`,
`seed_entry_for_test`, `entries_mut_for_test`, `alloc_block`, một `use`; chỉ `alloc_block` có token và nó là
`#[cfg(any(feature = "heap", test))]` = mã sản phẩm, loại là **đúng**). Nhưng mẫu
`#[cfg(test)] fn` **có** trong repo (`mcs.rs`, `tlb.rs`, `tx.rs`) ⇒ rủi ro báo oan tương lai.

**Đề nghị** (không bắt buộc trong KT-182, nhưng nên ghi nhận): thêm nhánh
`or any(is_cfg_test_attr(a) for a in f.attrs)` vào điều kiện, hoặc ghi rõ giới hạn trong
docstring mục "Phạm vi".

### 2.3 Hàm sản phẩm vẫn lọt vào bản đồ?

| Mẫu | TRƯỚC | SAU | Kết luận |
|---|---|---|---|
| `#[cfg(not(test))] mod prod` | xanh oan | **ĐỎ** | bản vá sửa **đúng** |
| `#[cfg(feature = "ktest")] mod` (test-only feature) | xanh | **ĐỎ oan** | **F2** (cùng lớp) |
| `#[cfg(any(test, feature = "x"))] mod` | xanh | xanh | **tiền tại** (F8): mod sản phẩm lọt phạm vi test |
| `#[cfg(test)] mod` chứa mã sản phẩm | — | — | không xảy ra (cfg(test) không biên dịch production) |

**F8** là giới hạn **tiền tại** của `parse_cfg_mods` (KT-170), không phải hồi quy KT-182:
`#[cfg(any(test, feature = "x"))] mod` được tính là phạm vi test, nên hàm sản phẩm bên trong
vẫn vào bản đồ. Cây thật **không** có mẫu này (`grep` chỉ thấy
`#[cfg(all(test, feature = "alloc"))]` — test-only). Không chặn KT-182.

### 2.4 `n_local_used`

- Đếm **số test** dùng đường bản đồ cùng tệp (không phải số tên), tăng một lần mỗi test,
  trong nhánh `elif` sau `helper_allow` và trước `global` — đúng thứ tự `scan()`.
- Cây thật: **23**, khớp phép đo độc lập (23 test `local`).
- `--json`: khoá `{ok, errors, warnings, info}` **không đổi**; `info` từ 7 → 8 dòng. Không
  consumer nào parse `info` theo chỉ số (`v2/bin/v2` chạy **không** `--json`; chỉ kiểm mã
  thoát). **Không** hỏng schema.

## 3. "Răng" của 3 ca selftest mới

Log: [`kt203-logs/selftest-mutations.log`](kt203-logs/selftest-mutations.log).

| Đột biến | Kết quả selftest | Diễn giải |
|---|---|---|
| M0 bản gốc | 36/36 | — |
| M1 **gỡ bản vá** | **35/36** — ca 34 đỏ | ca 34 **có răng** (bắt hồi quy) |
| M2 `helper_teeth` rỗng | **36/36** | **ca 35/36 không có răng** |
| M12 **cả bản đồ cùng tệp chết** | **36/36** | **ca 35/36 không có răng** |
| M3 ca 34: test tự assert | 35/36 — ca 34 đỏ | ca 34 kiểm đúng |
| M4 ca 34: hàm sản phẩm vào trong mod | 35/36 — ca 34 đỏ | ca 34 kiểm đúng |
| M5 ca 35: trợ giúp mất assert | 35/36 — ca 35 đỏ | ca 35 kiểm "trợ giúp có răng" |
| M6 ca 36: trợ giúp mất assert | 35/36 — ca 36 đỏ | ca 36 kiểm "trợ giúp có răng" |
| M7 bản đồ toàn cây rỗng | 32/36 | ca 35/36 vẫn xanh nhờ bản đồ cùng tệp |
| M8 ca 35 + tên mơ hồ (loại khỏi bản đồ toàn cây) | 36/36 | xanh nhờ bản đồ cùng tệp |
| **M9** M8 + bản đồ cùng tệp chết | **35/36 — ca 35 đỏ** | fixture sửa **có răng** |
| M10 ca 36 + tên mơ hồ | 36/36 | xanh nhờ bản đồ cùng tệp |
| **M11** M10 + bản đồ cùng tệp chết | **35/36 — ca 36 đỏ** | fixture sửa **có răng** |
| M13 chỉ loại hàm `debug_assert!` | 36/36 | selftest không bắt (cây thật cũng 0 lỗi) |

**Kết luận C**: **ca 34 có răng**; **ca 35 và 36 KHÔNG có răng** cho điều chúng tuyên bố.
Nguyên nhân: fixture đặt trợ giúp trong phạm vi test, nên **bản đồ toàn cây** (KT-170) cũng
chứng nhận cùng tên; test xanh dù bản đồ cùng tệp có bị xoá sạch. Muốn có răng, fixture phải
**loại tên khỏi bản đồ toàn cây** — ví dụ thêm một định nghĩa cùng tên **không răng** ở tệp
`tests/` khác (M8/M10); khi đó M9/M11 đỏ đúng ca. Đây là **lỗi chất lượng test**, không phải
lỗi chức năng của bản vá; nhưng tác giả đã tuyên bố "3 ca mới" là đối chứng dương cho bản đồ
cùng tệp — tuyên bố đó **sai** với 2/3 ca.

## 4. Docstring

Docstring mới (mục Fail-closed) nói:

> "Đo trước khi siết: **0** test chỉ có răng nhờ hàm sản phẩm cùng tệp (155 test gọi hàm sản
> phẩm cùng tệp nhưng **tất cả** đều có token khẳng định riêng) ⇒ siết **không** làm đỏ test
> nào, **0** mục allowlist."

Đối chiếu:

- **0** test chỉ có răng nhờ hàm sản phẩm cùng tệp — **ĐÚNG** (độc lập: 0).
- **155** test gọi hàm sản phẩm cùng tệp — **ĐÚNG theo nghĩa "theo tên"** (độc lập: 155),
  nhưng đây là **upper bound theo tên gọi**, không phân giải kiểu/phương thức. Journal §7.3
  của tác giả có ghi; docstring **không** ghi "theo tên" ⇒ **F3 (thấp)**.
- **tất cả** đều có token khẳng định riêng — **ĐÚNG** (độc lập: `n_call_product_no_own` = 0).
- **siết không làm đỏ test nào** — **ĐÚNG** trên cây hiện tại (diff 0).
- **0 mục allowlist** — **ĐÚNG** (`HELPER_ALLOW`, `ALLOW_NO_ASSERT`, `ALLOW_SHOULD_PANIC_BARE`
  đều rỗng; `ALLOW_LET_UNDERSCORE` 20 mục không đổi).

**Kết luận D**: docstring **khớp số đo**, trừ việc thiếu chữ "theo tên" ở con số 155.

## 5. `v2 check`

- Log tác giả `kt182-check-final.log`: **PASS** (bước `test có răng` OK, `selftest` OK).
- Worktree L41: **FAIL** ở bước `luật cấu trúc R1..R5` — `Trace/BPT trap: 5` (SIGTRAP) trong
  subshell bash 3.2, đúng lớp **T-INFRA** đã ghi ở `v2/bin/v2` (KT-155). Chạy trực tiếp
  `./v2/bin/v2 rules` ⇒ **PASS**. Bước `test có răng (KT-163)` và `selftest` trong cùng log
  ⇒ **OK**. Log: [`kt203-logs/v2check-T-INFRA.log`](kt203-logs/v2check-T-INFRA.log).
- **Không** phải hồi quy KT-182: bản vá chỉ chạm `v2/tools/check-test-teeth.py` (trong
  `tools/`, không thuộc R1) và thêm `v2/agent/JOURNAL/` (ngoài `SCAN_DIRS`).

## 6. Phát hiện (đầy đủ)

| # | Mức | Mô tả | Bằng chứng | Cách dựng lại |
|---|---|---|---|---|
| **F1** | **Trung bình** | Ca selftest 35/36 **không có răng**: vẫn 36/36 khi xoá sạch bản đồ cùng tệp, vì bản đồ toàn cây chứng nhận cùng tên. Fixture C (§6.1 journal) cùng bệnh. Tuyên bố "đối chứng dương — siết không quá tay" **không** được chứng minh. | `selftest-mutations.log` M2/M12 = 36/36; M9/M11 (thêm tên mơ hồ) = 35/36 đỏ đúng ca | Chép `check-test-teeth.py`, thay điều kiện `if not (is_whole or in_ranges(...))` trong vòng `helper_teeth` bằng `if True: continue`, chạy `--selftest` ⇒ vẫn 36/36 |
| **F2** | **Trung bình–thấp** | Hàm `#[cfg(test)]` trên **item** (`fn`/`impl`) bị loại khỏi bản đồ cùng tệp ⇒ test gọi trợ giúp test-only bị `E_NO_ASSERT` **oan**. Cây thật 0 ảnh hưởng (11 item, chỉ `alloc_block` có token và là mã sản phẩm), nhưng mẫu có trong repo. | `edge-fixtures.log`: `item-cfg-fn`, `item-cfg-impl`, `cfg-feature-ktest` — TRƯỚC xanh, SAU đỏ | Fixture `#[cfg(test)] fn tro_giup(x){assert_eq!(x,3);}` + `#[cfg(test)] mod tests { #[test] fn t(){ tro_giup(3); } }` |
| **F3** | **Thấp** | Docstring ghi "155 test gọi hàm sản phẩm cùng tệp" mà không nói **theo tên**; đây là upper bound theo tên gọi, không phân giải kiểu. Kết luận không đổi. | `independent-measure.log` `n_call_product=155`; journal §7.3 đã ghi "theo tên" | So docstring mục Fail-closed với journal §7.3 |
| **F4** | **Thông tin (T-INFRA)** | `v2 check` FAIL ở `luật cấu trúc` do SIGTRAP bash 3.2; `v2 rules` trực tiếp PASS; bước `test có răng` OK. Không liên quan KT-182. | `v2check-T-INFRA.log` | `./v2/bin/v2 check` (2 lần đều SIGTRAP) vs `./v2/bin/v2 rules` (PASS) |
| **F5** | **Thấp** | Journal §4.2 nói "Hai tên `rig`, `run` … bản đồ cùng tệp vẫn cần"; thực tế chỉ `run` **bắt buộc** (M12: 6 lỗi đều `run`). `rig` không có trong bản đồ toàn cây nhưng test gọi `rig` cũng gọi `run_steps`/`arm_rig` (có trong bản đồ toàn cây). Kết luận "vẫn cần" đúng, chi tiết hơi quá. | `independent-measure.log`; M12 trên cây thật = 6 lỗi `run` | Như F1, chạy bản đột biến với `--root /private/tmp/night3-L41-review182` ⇒ 6 `E_NO_ASSERT`, tất cả tên `run` |
| **F6** | **Thấp** | Trùng F1: fixture C / ca 35/36 được mô tả là "đối chứng dương" nhưng không chứng minh được. | như F1 | như F1 |
| **F7** | **Thông tin (tiền tại)** | `#[cfg(test)] pub mod tests` bị `parse_cfg_mods` bỏ sót ⇒ `E_NO_TESTS` (cả TRƯỚC và SAU). Không phải hồi quy KT-182. | `edge-fixtures.log` `cfg-test-pub-mod` | Fixture `#[cfg(test)] pub mod tests { … }` |
| **F8** | **Thông tin (tiền tại)** | `#[cfg(any(test, feature = "x"))] mod` được tính là phạm vi test ⇒ mã sản phẩm lọt (cả TRƯỚC và SAU). Không phải hồi quy KT-182; cây thật không có mẫu. | `edge-fixtures.log` `cfg-any-test-feature` | Fixture `#[cfg(any(test, feature="x"))] mod shared { pub fn kiem(x){assert_eq!(x,3);} }` |

## 7. Việc đã kiểm, kết luận ĐÚNG (không phát hiện)

- Số đo 1019 / 996 / 23 / 0 / 155 / 0 / 80 / 173 / 3 — **khớp độc lập**.
- Không test nào chỉ có răng nhờ hàm sản phẩm cùng tệp — **đúng**.
- Không test nào bị báo oan sau siết trên cây thật — **đúng** (diff 0).
- Bản vá siết **cả** `helper_teeth` **và** `calls_of` — **đúng** (nếu chỉ siết một, bao đóng
  bắc cầu vẫn đi qua hàm sản phẩm).
- `n_local_used` đếm đúng 23; `--json` schema không đổi (info +1 dòng, không consumer parse
  theo chỉ số).
- Selftest 33/33 → 36/36, **không** mất ca cũ (diff chỉ thêm).
- 0 allowlist mới; `ALLOW_LET_UNDERSCORE` 20 mục không đổi.
- Đột biến cây thật: TRƯỚC xanh oan → SAU đỏ `E_NO_ASSERT`; gỡ ⇒ sạch.
- Fixture A/B của tác giả: TRƯỚC xanh → SAU đỏ; **đúng** (chạy lại `kt182-fixture-proof.py`).

## 8. Bằng chứng (log)

| Tệp | Nội dung |
|---|---|
| [`kt203-logs/independent-measure.py`](kt203-logs/independent-measure.py) | phép đo độc lập (không import checker) |
| [`kt203-logs/independent-measure.log`](kt203-logs/independent-measure.log) | kết quả đo độc lập |
| [`kt203-logs/old-vs-new.log`](kt203-logs/old-vs-new.log) | checker TRƯỚC vs SAU trên cây thật, diff lỗi |
| [`kt203-logs/selftest-mutations.log`](kt203-logs/selftest-mutations.log) | 14 đột biến selftest (M0–M13) |
| [`kt203-logs/edge-fixtures.log`](kt203-logs/edge-fixtures.log) | fixture biên phạm vi test (TRƯỚC vs SAU) |
| [`kt203-logs/real-tree-mutation.log`](kt203-logs/real-tree-mutation.log) | đột biến `Machine::add_cnode` trên cây thật |
| [`kt203-logs/v2check-T-INFRA.log`](kt203-logs/v2check-T-INFRA.log) | `v2 check` FAIL do SIGTRAP + `v2 rules` trực tiếp PASS |

## 9. Điều kiện để ĐẠT (đề nghị cho vòng sau)

1. **Sửa fixture ca 35/36** để có răng: thêm định nghĩa cùng tên **không răng** ở tệp
   `tests/` khác (loại tên khỏi bản đồ toàn cây), rồi kiểm rằng xoá bản đồ cùng tệp ⇒ ca đỏ.
   (Đã chứng minh bằng M8/M9 và M10/M11.)
2. **Ghi nhận hoặc vá F2**: item-level `#[cfg(test)]` (`fn`/`impl`) là phạm vi test; bản vá
   hiện loại chúng. Cây thật 0 ảnh hưởng, nhưng nên thêm nhánh nhận `cfg(test)` trên chính
   item, hoặc ghi rõ giới hạn trong docstring "Phạm vi".
3. **Thêm "theo tên"** vào con số 155 trong docstring (F3).

Không điều kiện nào chặn bản vá: bản vá **đúng chức năng**, số đo **đúng**, cây thật **không
hồi quy**.
