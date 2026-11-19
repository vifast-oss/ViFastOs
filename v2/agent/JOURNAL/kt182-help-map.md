# KT-182 — đo tác động: bản đồ trợ giúp **cùng tệp** nhận hàm **sản phẩm** cùng tệp

**Ngày**: 2026-11-19/20 · **Agent 2 (L38)** · nhánh `night3/L38-kt182` · HEAD trước task: `8f5a1189`
**Nguồn nợ**: KT-170 §3.5 (ghi chú trung thực: "bản đồ **cùng tệp** vẫn tính mọi hàm
không-phải-`#[test]` trong tệp, kể cả hàm sản phẩm cùng tệp — … cần đo trước") + D2-164.

## 0. Việc được giao (nguyên văn, trích)

> "**đo tác động** bản đồ trợ giúp KT-163 nhận hàm không phải `#[test]`" — "bản đồ **cùng
> tệp** vẫn nhận hàm **sản phẩm** cùng tệp; **đo tác động trước khi siết**".

Bốn câu hỏi có số: (Q1) bao nhiêu `#[test]` **chỉ** có răng nhờ hàm sản phẩm cùng tệp;
(Q2) danh sách đầy đủ `tệp:dòng` + test + hàm + token; (Q3) răng thật vs răng giả;
(Q4) nếu siết: tổng ĐỎ, hồi quy giả, lỗ hổng thật. Rồi chọn (a) siết hay (b) không siết.

## 1. Kết luận trước (đọc nhanh)

- **Q1 = 0.** Trên cây `8f5a1189` **không** test nào chỉ có răng nhờ hàm **sản phẩm cùng
  tệp**. 1019 `#[test]`: **996** có token khẳng định **trong thân**, **23** còn lại dựa vào
  trợ giúp **cùng tệp trong phạm vi test** (tất cả ở tệp `tests/`), **0** dựa vào hàm sản
  phẩm cùng tệp, **0** dựa vào bản đồ toàn cây, **0** không có răng.
- **Lỗ hổng là thật nhưng tiềm ẩn.** Fixture A/B (§6) chứng minh: trước bản vá, một
  test trong `#[cfg(test)]` chỉ gọi hàm sản phẩm cùng tệp (`debug_assert!` **hoặc**
  `assert!`) mà không assert ⇒ **XANH OAN**; sau bản vá ⇒ **ĐỎ**.
- **Q4 = 0 ĐỎ.** Siết bản đồ cùng tệp về phạm vi test: **0** test mới đỏ, **0** hồi quy
  giả, **0** lỗ hổng thật, **0** mục allowlist. Đo cả biến thể chặt hơn (fail-closed theo
  tên mơ hồ như bản đồ toàn cây): vẫn **0** đỏ.
- **Quyết định: (a) SIẾT.** Vì tác động đo được **bằng 0** — siết là **miễn phí**, đóng
  lỗ hổng tiềm ẩn đã chứng minh bằng fixture, và làm bản đồ cùng tệp **đồng bộ** với bản
  đồ toàn cây (KT-170). Không phải nới luật, không thêm allowlist.
- **Selftest 33 → 36** (3 ca mới: (34) sản phẩm cùng tệp `debug_assert!` ⇒ ĐỎ; (35) trợ
  giúp cùng tệp trong `#[cfg(test)]` ⇒ XANH; (36) trợ giúp cùng tệp trong `tests/` ⇒
  XANH). **Không** mất ca cũ.
- **Đột biến trên cây thật**: thêm test chỉ gọi `Machine::add_cnode` (hàm sản phẩm cùng
  tệp có `assert!`) ⇒ `v2 check` **ĐỎ** (`E_NO_ASSERT`, `1 bước FAIL`); gỡ ⇒ **PASS**.
- **`./v2/bin/v2 check` PASS** trên cây sau bản vá (log `kt182-logs/kt182-check-final.log`).

## 2. Lỗ hổng là gì

`scan()` dựng **hai** bản đồ trợ giúp:

| Bản đồ | Dựng từ | Phạm vi | Từ |
|---|---|---|---|
| **toàn cây** (`build_global_helpers`) | mọi tệp `.rs` | **chỉ hàm trong phạm vi test** | KT-170 (đã siết) |
| **cùng tệp** (`helper_teeth` + `calls_of` trong `scan`) | một tệp | **mọi** `fn` không-phải-`#[test]` — **kể cả hàm sản phẩm** | KT-163 (chưa siết) |

Với tệp `v2/**/src/**/*.rs`, "phạm vi test" chỉ là khối `#[cfg(test)]`. Hàm sản phẩm ở
cấp module **nằm ngoài** khối đó nhưng vẫn vào `helper_teeth`. Hệ quả: một test trong
`#[cfg(test)] mod tests` gọi `super::fsck()` (hoặc `m.add_cnode(4)`) mà **không** assert
được chứng nhận "có răng" nhờ `debug_assert!`/`assert!` **của sản phẩm** — đúng lớp
xanh-giả mà KT-163 sinh ra để bắt, và đúng lý do KT-170 đã siết bản đồ toàn cây.

Fixture §6 chứng minh lỗ hổng **cắn được** (trước bản vá: XANH; sau: ĐỎ). Trên cây thật
thì **chưa ai vấp** — đó là toàn bộ nội dung của "đo tác động".

## 3. Phương pháp đo (tái lập được)

Script: [`kt182-logs/measure-local-map.py`](kt182-logs/measure-local-map.py). Nó **import
chính checker** (`v2/tools/check-test-teeth.py`) làm module, dựng lại **đúng** các bảng
`scan()` dựng (`parse_file`, `build_scope`, `build_global_helpers`, `build_use_aliases`),
rồi chạy lại phép chứng nhận lớp (c) với **hai** bản đồ cùng tệp:

- `current` — mọi `fn` không-phải-`#[test]` trong tệp (hành vi KT-163);
- `scoped` — chỉ `fn` trong phạm vi test (đúng luật KT-170 áp cho bản đồ toàn cây).

Không sửa checker để đo (không làm bẩn công cụ). Mỗi test được gán **một** đường chứng
nhận theo đúng thứ tự `scan()`: `own` → `helper_allow` → `local` → `global` →
`allow_no_assert` → `none`. "Chỉ dựa hàm sản phẩm cùng tệp" = đường `local` mà **mọi** tên
chứng nhận đều **không** nằm trong phạm vi test.

Phân loại "răng" của hàm sản phẩm (Q3): **thật** = có `assert!`/`assert_eq!`/`assert_ne!`/
`assert_matches!`/`expect_err`/`unwrap_err`, hoặc `panic!` **có điều kiện**; **giả** = chỉ
`debug_assert*!` hoặc không token nào (chỉ bắc cầu). Tính cả **bắc cầu** (hàm gọi hàm).

Lệnh:

```bash
python3 v2/agent/JOURNAL/kt182-logs/measure-local-map.py \
        --json v2/agent/JOURNAL/kt182-logs/kt182-measure.json \
        | tee v2/agent/JOURNAL/kt182-logs/kt182-measure.log
```

## 4. Số đo đầy đủ

### 4.1 Tổng quan (cây `8f5a1189`, 308 tệp `.rs`, 130 tệp có phạm vi test)

| Số đo | Giá trị |
|---|---|
| `#[test]` trong phạm vi | **1019** |
| … có token khẳng định **trong thân** (`own`) | **996** |
| … dựa vào trợ giúp **cùng tệp** (`local`) | **23** |
| … trong đó trợ giúp **trong phạm vi test** | **23** |
| … trong đó trợ giúp là **hàm sản phẩm cùng tệp** | **0** |
| … dựa vào bản đồ **toàn cây** (`global`) | **0** |
| … dựa vào `HELPER_ALLOW` | **0** |
| … dựa vào `ALLOW_NO_ASSERT` | **0** |
| … **không** có răng (đỏ hiện tại) | **0** |
| Tên trong bản đồ toàn cây | 80 |
| Tệp `src/` có khối `#[cfg(test)]` | 35 |
| Hàm sản phẩm trong các tệp đó | 986 |

### 4.2 Q1/Q2/Q3 — test **chỉ** có răng nhờ hàm **sản phẩm cùng tệp**

**Không có mục nào.** Bảng đầy đủ rỗng (script in tiêu đề, không in dòng). Vì vậy Q3
(răng thật vs răng giả) **không áp dụng**: 0 mục ⇒ 0 răng thật, 0 răng giả.

Đối chứng **dương** cho phép đo — 23 test dựa vào trợ giúp **cùng tệp trong phạm vi test**
(đều ở tệp `tests/`, nên cả tệp là phạm vi test; siết **không** đụng):

| # | tệp:dòng | test | trợ giúp cùng tệp |
|---|---|---|---|
| 1 | `v2/kernel/vifast-plat-common/tests/mmu_shared.rs:242` | `dinh_dang_aarch64` | `kiem_tung_dinh_dang` |
| 2 | `v2/kernel/vifast-plat-common/tests/mmu_shared.rs:247` | `dinh_dang_sv39` | `kiem_tung_dinh_dang` |
| 3 | `v2/kernel/vifast-plat-common/tests/mmu_shared.rs:254` | `dinh_dang_x86_64` | `kiem_tung_dinh_dang` |
| 4 | `v2/sim/vifast-sim/tests/kt42_scn.rs:740` | `scn22_map_unmap_walk_bo_cuc` | `run_scn` |
| 5 | `v2/sim/vifast-sim/tests/kt42_scn.rs:745` | `scn23_wx_violation` | `run_scn` |
| 6 | `v2/sim/vifast-sim/tests/kt42_scn.rs:750` | `scn24_shootdown_2_hart` | `run_scn` |
| 7 | `v2/sim/vifast-sim/tests/kt42_scn.rs:755` | `scn25_vm_fault_resume` | `run_scn` |
| 8 | `v2/sim/vifast-sim/tests/kt42_scn.rs:760` | `scn26_shared_readonly` | `run_scn` |
| 9 | `v2/sim/vifast-sim/tests/kt58_core_parity.rs:2403` | `parity_kich_ban_kt09_kt10_kt11` | `run_script` |
| 10 | `v2/sim/vifast-sim/tests/kt59_core_parity.rs:1640` | `kt59_parity_scheduler_fifo_round_robin` | `arm_rig, rig, run_steps` |
| 11 | `v2/sim/vifast-sim/tests/kt59_core_parity.rs:1728` | `kt59_parity_budget_va_refill` | `rig, run_steps` |
| 12 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:1948` | `kt60_parity_ipc24_hang_fifo_va_busy` | `run` |
| 13 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:2083` | `kt60_parity_ipc25_call_reply_replyrecv_va_kt34` | `run` |
| 14 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:2278` | `kt60_parity_ipc27_notification` | `run` |
| 15 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:2499` | `kt60_parity_ipc28_waitaddr` | `run` |
| 16 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:2665` | `kt60_parity_ipc26_fault_exit` | `run` |
| 17 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:2754` | `kt60_parity_ipc21_donation_hook` | `run` |
| 18 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:3965` | `kt63_parity_revoke_finalise_caller_va_sender` | `run63_full` |
| 19 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:4044` | `kt64a_parity_finalise_recv_waiter` | `run63_full` |
| 20 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:4069` | `kt64a_parity_finalise_notification_waiter` | `run63_full` |
| 21 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:4091` | `kt63_parity_delete_cap_reply_huy_caller` | `run63` |
| 22 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:4136` | `kt63_parity_revoke_cap_reply_con_huy_caller` | `run63` |
| 23 | `v2/sim/vifast-sim/tests/kt60_core_parity.rs:4182` | `kt63_parity_reply_cap_chet_trong_waiter_q7b` | `run63` |

Hai tên `rig`, `run` **không** có trong bản đồ toàn cây (tên mơ hồ toàn cây). **KT-206**
(review L41 F5): chỉ **`run`** là **bắt buộc** — xoá sạch bản đồ cùng tệp thì 6 test đỏ, cả
6 đều dùng `run`; `rig` không bắt buộc vì các test gọi `rig` cũng gọi `run_steps`/`arm_rig`
(có trong bản đồ toàn cây). Kết luận "bản đồ cùng tệp **vẫn cần** sau khi siết" **giữ
nguyên**; chi tiết "cả hai tên" là quá mức.

### 4.3 Vì sao 0 — nhưng lỗ hổng vẫn thật

**155** test **có gọi** một hàm sản phẩm cùng tệp (theo tên), nhưng **cả 155** đều có
token khẳng định **riêng trong thân** ⇒ siết không đổi trạng thái của chúng (log:
`kt182-measure.log`, khoá `n_tests_call_product` = 155, `n_tests_call_product_no_own` = 0).
Phân bố
(theo tệp, nhiều nhất trước):

| Số test | Tệp |
|---|---|
| 25 | `v2/sim/vifast-sim/src/kobj.rs` |
| 12 | `v2/tools/vifast-agentctl/src/lib.rs` |
| 9 | `v2/lib/vifast-objmem/src/many.rs` |
| 9 | `v2/sim/vifast-sim/src/mcs.rs` |
| 7 | `v2/sim/vifast-sim/src/lib.rs` |
| 6 | `v2/kernel/vifast-core/src/invoke.rs`, `…/syscall.rs`, `v2/lib/vifast-elf/src/lib.rs`, `v2/lib/vifast-traceformat/src/lib.rs` |
| … | 24 tệp còn lại, 1–5 test mỗi tệp |

Đây chính là "tác động bằng 0": bản đồ cùng tệp **đang** phủ 155 test, nhưng **không** test
nào **cần** nó để xanh. Nó là **hạ tầng cho tương lai** — và tương lai đó có thể là một
test xanh-giả (fixture §6).

### 4.4 Q4 — nếu siết (chỉ nhận hàm trong phạm vi test)

| Số đo | Giá trị |
|---|---|
| Tổng test ĐỎ **hiện tại** | **0** |
| Tổng test ĐỎ **sau siết** | **0** |
| Test **mới** đỏ | **0** |
| … **hồi quy giả** (test thật sự có răng, mất chứng nhận) | **0** |
| … **lỗ hổng thật** (test vốn không có răng) | **0** |
| Mục allowlist phải thêm | **0** |

Đo thêm **biến thể chặt hơn** (siết phạm vi **và** fail-closed theo tên mơ hồ trong tệp,
đúng luật bản đồ toàn cây): vẫn **0** đỏ. Ghi nhận như dữ liệu, **không** đưa vào bản vá
(ngoài phạm vi KT-182).

## 5. Quyết định: **(a) SIẾT**

Tiêu chí đề bài: "nếu số **hồi quy giả** lớn và phải allowlist quá nhiều ⇒ (b); nếu số
**lỗ hổng thật** đáng kể và allowlist nhỏ ⇒ (a)". Số đo: **0 hồi quy giả, 0 allowlist**.
Cả hai vế đều **không** cản (a); và (a) **đóng** một lỗ hổng đã chứng minh bằng fixture
(§6) với chi phí **bằng 0**. Chọn **(a)**.

Vì sao **không** (b): (b) chỉ thêm cảnh báo, để lỗ hổng mở. Ở đây giá của (a) đã đo được
là **0** — không có lý do trung thực nào để giữ lỗ hổng. Ngược lại, (b) sẽ khiến bản đồ
cùng tệp **lệch** bản đồ toàn cây (một cái siết, một cái không) — thêm một chỗ hiểu nhầm.

### 5.1 Bản vá

`v2/tools/check-test-teeth.py`, trong `scan()`:

```python
        helper_teeth: Dict[str, bool] = {}
        for f in fns:
            if f.is_test:
                continue
            if not (is_whole or in_ranges(f.start, ranges)):   # KT-182
                continue
            ...
        calls_of: Dict[str, Set[str]] = {}
        for f in fns:
            if not (is_whole or in_ranges(f.start, ranges)):   # KT-182
                continue
            ...
```

`is_whole` / `ranges` là **đúng** phạm vi test mà `scan()` đã dựng cho lớp (c)/(d) — không
thêm khái niệm mới. `calls_of` cũng siết để bao đóng bắc cầu **không** đi qua hàm sản phẩm
(nếu không, test → helper test → hàm sản phẩm vẫn được chứng nhận).

Thêm dòng `info` đối xứng với bản đồ toàn cây:

```
# bản đồ trợ giúp cùng tệp (chỉ hàm trong phạm vi test): 23 test dùng đường này
```

Docstring: sửa định nghĩa "hàm trợ giúp đã biết" (thêm "cùng tệp **trong phạm vi test**")
và thêm gạch đầu dòng trong mục **Fail-closed** ghi số đo + đường dẫn log.

**Không** thêm mục allowlist nào. **Không** nới ngưỡng nào.

## 6. Đột biến — đỏ/xanh

### 6.1 Fixture (đột biến ở mức checker, tái lập được)

Script: [`kt182-logs/kt182-fixture-proof.py`](kt182-logs/kt182-fixture-proof.py) — chạy
**hai** bản checker (bản trước KT-182 lấy từ `git show 8f5a1189:…`, bản hiện tại) trên ba
cây tối thiểu. Log: `kt182-logs/kt182-fixture-proof.log`.

| Fixture | Nội dung | TRƯỚC | SAU | Kỳ vọng |
|---|---|---|---|---|
| A `same-file-product-debug` | hàm **sản phẩm** cùng tệp có `debug_assert!`; test `#[cfg(test)]` gọi mà không assert | **XANH** | **ĐỎ** (`E_NO_ASSERT`) | xanh oan → đỏ |
| B `same-file-product-assert` | như A nhưng hàm sản phẩm có `assert!` (răng **thật**) | **XANH** | **ĐỎ** (`E_NO_ASSERT`) | xanh oan → đỏ |
| C `same-file-test-helper` | trợ giúp **trong `#[cfg(test)]`** có `assert_eq!`; test gọi mà không assert; **KT-206**: thêm hàm sản phẩm cùng tên đặt **sau** mod + định nghĩa cùng tên không răng ở `tests/` khác | **ĐỎ** (last-wins lấy hàm sản phẩm) | **XANH** | đỏ → xanh (đối chứng dương; **có răng**: SAU+bản-đồ-cùng-tệp-chết ⇒ **ĐỎ**) |

A và B chứng minh lỗ hổng **cắn** cả `debug_assert!` (răng giả) **lẫn** `assert!` (răng
thật) — tức đây không phải chuyện "chỉ debug". C (fixture **mới** của KT-206) chứng minh
hai điều: (i) bản vá **sửa báo oan** do trùng tên — TRƯỚC đỏ vì bản đồ cùng tệp last-wins
lấy hàm sản phẩm cùng tên, SAU xanh; (ii) siết **không** quá tay — trợ giúp test-scope vẫn
được tính, và C **có răng**: SAU+bản-đồ-cùng-tệp-chết ⇒ đỏ.

### 6.2 Cây thật — `v2 check` đỏ rồi xanh

Đột biến: thêm vào `#[cfg(test)] mod tests` của `v2/lib/vifast-model/src/machine.rs`:

```rust
    #[test]
    fn kt182_dot_bien_chi_goi_san_pham_cung_tep() {
        let mut m = Machine::new();
        let _a = m.add_cnode(4);   // hàm sản phẩm cùng tệp, có `assert!`
    }
```

| Bước | Lệnh | Kết quả | Log |
|---|---|---|---|
| Đột biến | `./v2/bin/v2 check` | **ĐỎ** — `1 bước FAIL`, `E_NO_ASSERT v2/lib/vifast-model/src/machine.rs:1041` | `kt182-logs/kt182-check-mutation.log` |
| Gỡ | `git checkout -- v2/lib/vifast-model/src/machine.rs` + `./v2/bin/v2 check` | **PASS** | `kt182-logs/kt182-check-final.log` |

Đột biến **không** làm đỏ bước nào khác (fmt/clippy/L0 vẫn xanh) — đỏ đúng một bước, đúng
mã lỗi, đúng dòng. Đây là bằng chứng bản vá **cắn** trên cây thật, không chỉ trên fixture.

### 6.3 Selftest

| Trước | Sau |
|---|---|
| **33/33** ok | **36/36** ok |

Ba ca mới (không mất ca cũ, không đổi ca cũ):

| # | Ca | Kỳ vọng |
|---|---|---|
| 34 | trợ giúp **cùng tệp** là hàm **sản phẩm** (`debug_assert!`) ⇒ KHÔNG tính là răng | **ĐỎ** `E_NO_ASSERT` |
| 35 | trợ giúp **cùng tệp** trong `#[cfg(test)]` ⇒ vẫn tính là răng | **XANH** |
| 36 | trợ giúp **cùng tệp** trong `tests/` ⇒ vẫn tính là răng | **XANH** |

Ca 34 là **đối chứng âm** (nếu ai "vá" bản đồ cùng tệp thành nhận lại hàm sản phẩm, ca
này đỏ ngay). Ca 35/36 là **đối chứng dương** cho hai nhánh phạm vi test (`#[cfg(test)]`
và tệp `tests/`) — chứng minh siết không quá tay.

**KT-206** (review L41 F1): fixture **cũ** của ca 35/36 **không có răng** — bản đồ toàn cây
(KT-170) chứng nhận cùng tên nên ca vẫn xanh dù bản đồ cùng tệp chết. KT-206 đã thêm tên
mơ hồ + hàm sản phẩm cùng tên để ca có răng (xem `kt206-fix182.md` §F1); selftest 36 → 40
ca (thêm 37–40 cho item-level `#[cfg(test)]`).

## 7. Giới hạn còn lại

1. **Bản đồ cùng tệp khoá theo *tên*, last-wins.** 173 tên có ≥2 định nghĩa trong phạm vi
   test cùng tệp (log: `n_multi_def_test_scope` = 173; phần lớn là method trong `impl`
   khác nhau — `kind`, `new`, `read`…).
   `helper_teeth[f.name] = …` ghi đè, nên một test gọi tên trùng có thể được chứng nhận
   nhờ **định nghĩa khác** cùng tên. Đo thô: siết thêm fail-closed theo tên mơ hồ vẫn
   **0** test đỏ, nên **chưa** cắn; nhưng đây là lỗ hổng **cùng lớp** (định danh theo tên,
   không theo `impl`/kiểu) — nên đo riêng trước khi siết, đừng gộp vào KT-182.
2. **3 tên vừa có định nghĩa sản phẩm vừa có định nghĩa test trong cùng tệp** (log:
   `n_collision_names` = 3 + danh sách):
   `ipc_header` (`v2/kernel/vifast-core/src/syscall.rs`), `kind`
   (`v2/lib/vifast-objmem/src/obj.rs`), `place` (`v2/lib/vifast-objmem/src/store.rs`).
   Sau KT-182, định nghĩa **sản phẩm** không còn vào bản đồ ⇒ một nửa lỗ hổng đã đóng;
   nửa còn lại (nhiều định nghĩa **test** cùng tên) thuộc mục 1.
3. **Đo theo *tên gọi*, không theo phân giải kiểu.** `m.add_cnode(4)` khớp tên
   `add_cnode`; nếu có hàm tự do `add_cnode` khác, phép đo không phân biệt. Với 155 test
   đều có token riêng, sai số này **không** đổi kết luận Q1 = 0.
4. **`debug_assert!` vẫn tính là răng** (thiết kế KT-163, không đổi ở KT-182). Test chỉ có
   `debug_assert!` là "răng yếu" (release không chạy) — chưa có phép đo riêng; nếu muốn
   siết, đó là task khác (sẽ cần đo tác động tương tự).
5. **Không kiểm được đường thực thi.** Checker là phân tích tĩnh theo tên: một test gọi
   trợ giúp có `assert!` trong nhánh **không** được chạy vẫn xanh. Ngoài phạm vi.
6. **Bản đồ toàn cây vẫn khoá theo tên** (KT-170) — cùng lớp giới hạn mục 1, khác phạm vi.

## 8. Bằng chứng (log)

| Tệp | Nội dung |
|---|---|
| `kt182-logs/measure-local-map.py` | script đo (import checker, hai bản đồ) |
| `kt182-logs/kt182-measure.log` | kết quả đo đầy đủ (Q1–Q4 + 155 test gọi hàm sản phẩm, 173 tên trùng, 3 tên vừa sản phẩm vừa test) |
| `kt182-logs/kt182-measure.json` | tổng quan + 23 test dùng bản đồ cùng tệp (13 KB) |
| `kt182-logs/kt182-fixture-proof.py` | script đột biến mức checker |
| `kt182-logs/kt182-fixture-proof.log` | bảng fixture × checker (A/B/C) |
| `kt182-logs/kt182-scan-before.log` | checker **trước** KT-182 trên cây: `errors=0` |
| `kt182-logs/kt182-scan-after.log` | checker **sau** KT-182 trên cây: `errors=0`, 23 test dùng bản đồ cùng tệp |
| `kt182-logs/kt182-selftest.log` | `--selftest` sau: **36/36** |
| `kt182-logs/kt182-check-baseline.log` | `v2 check` trước đột biến: **PASS** |
| `kt182-logs/kt182-check-mutation.log` | `v2 check` với đột biến: **ĐỎ** (`E_NO_ASSERT`) |
| `kt182-logs/kt182-check-final.log` | `v2 check` sau khi gỡ đột biến: **PASS** |

## 9. Tệp đã sửa

- `v2/tools/check-test-teeth.py` — siết bản đồ cùng tệp về phạm vi test; docstring; dòng
  `info`; 3 ca selftest (33 → 36).
- `v2/agent/JOURNAL/kt182-help-map.md` — tệp này.
- `v2/agent/JOURNAL/kt182-logs/` — script + log (mục 8).
