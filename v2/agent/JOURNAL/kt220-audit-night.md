# KT-220 — Audit **tính trung thực** journal đêm 2026-11-20 (trước khi lead gửi A0)

**Vai**: L50 — **auditor** (không phải tác giả, không phải reviewer chức năng).
**Nhánh**: `night3/L50-kt220`, tạo từ `1c5c2946`. **Worktree**: `/private/tmp/night3-L50-kt220`.
**Phạm vi**: 20 journal đêm (`kt181`…`kt217`) + `DECISIONS.md` D2-169..D2-174.
**Nguyên tắc**: mỗi con số phải truy được về **log** trong `JOURNAL/ktXXX-logs/`. Không có log ⇒
**KHÔNG NGUỒN**. Log khác số ⇒ **LỆCH**. Khẳng định "đã đóng" mà không có đột biến + log
đỏ/xanh ⇒ **KHÔNG CHỨNG CỨ**. Audit này **không** sửa journal của ai; chỉ ghi nhận.

---

## 0. KẾT LUẬN NGẮN

**TRUNG THỰC, CÓ LỆCH NHỎ.** Không tìm thấy khẳng định nào bịa đặt có hệ thống: **mọi** con số
đo chính (selftest, sha ảnh, dấu vân tay, số ca đỏ, số đường lọt, thời gian soak) đều **khớp log**
khi log tồn tại. Các lệch tìm được là **số dẫn xuất** (off-by-one), **số không có khoá trong log**,
và **một mục DECISIONS không có journal** — không phải sai lệch về kết quả kỹ thuật.

| Hạng mục | Số |
|---|---|
| Khẳng định đã kiểm (trong 20 journal) | **190** |
| Khớp log | **183** |
| Lệch | **1** |
| Không nguồn (không có log chứa số) | **6** |
| Không chứng cứ (khẳng định "đã đóng" thiếu đột biến/log) | **1** (D2-174, ngoài bảng) |
| Đột biến tự dựng lại | **4/3** (vượt yêu cầu) — **4/4 ĐỎ đúng như journal khai** |

**Hai việc lead PHẢI xử lý trước khi gửi A0**: (1) **D2-174 / đường lọt #13 không có journal
và không có log** — chỉ tồn tại trong `DECISIONS.md` + commit message (tôi **đã tái lập được**
đường lọt này, nên nó **đúng về kỹ thuật**, chỉ **thiếu bằng chứng quy trình**); (2) **`kt182`
§4.1 ghi "35" trong khi log ghi 36** (off-by-one, dẫn xuất).

---

## 1. BẢNG KIỂM TỪNG JOURNAL

Ký hiệu: **K** = khớp, **L** = lệch, **N** = không nguồn, **C** = không chứng cứ.

| Journal | Khẳng định kiểm | K | L | N | C | Ghi chú |
|---|---|---|---|---|---|---|
| `kt181-macro-scope` | 12 | 12 | 0 | 0 | 0 | 118/118, 128/128, 124/128 (4 `not ok`), M1–M6 RC đúng |
| `kt182-help-map` | 16 | 14 | 1 | 1 | 0 | **L**: "35 tệp src/ có `#[cfg(test)]`" (log: 36). **N**: "986 hàm sản phẩm" |
| `kt183-cd-sau` | 15 | 15 | 0 | 0 | 0 | sha/inputs/dirty/rev/size, 73 PASS/0 FAIL, 46/46, A 5/8, B 4/8 |
| `kt201-review-vong4` | 8 | 8 | 0 | 0 | 0 | 118/118, 34 đột biến → 25 chết/6 sống/3 artifact |
| `kt202-macro-invocation` | 10 | 10 | 0 | 0 | 0 | 128→144, 134/144, 10 ca đỏ, `user.rs:477` |
| `kt203-review182` | 14 | 12 | 0 | 2 | 0 | **N**: "173" và "3" — script độc lập **không** tính hai chỉ số này |
| `kt204-review183` | 16 | 16 | 0 | 0 | 0 | M1 3/8, M2 8/8, M3 3/8, M4 8/8, A 5/8, B 4/8, `ok=31 bad=0 legacy=4` |
| `kt205-fix183` | 14 | 14 | 0 | 0 | 0 | `63db1f7f`, `3f7bb9ad`, 19/19, 78/0, 51/51, M2 7/9, M4 1/3 |
| `kt206-fix182` | 8 | 8 | 0 | 0 | 0 | 36→40, 40/40, 1019/23/80/20 |
| `kt207-sigtrap-rules` | 4 | 4 | 0 | 0 | 0 | 5 log, 3/3 PASS, `rules: PASS` |
| `kt208-review202` | 9 | 9 | 0 | 0 | 0 | 144/144, 144/146, 16/16 MỚI, 6/16 CŨ, 10 ca |
| `kt209-scope-gaps` | 8 | 8 | 0 | 0 | 0 | 118→131, 129→130 tệp, 26 tệp ngoài mẫu |
| `kt210-k6-remeasure` | 8 | 8 | 0 | 0 | 0 | 6/6, soak 1000/1000 297 s, `data+bss=224288`, 1521 dòng/81 `ok` |
| `kt211-include-global` | 9 | 7 | 0 | 2 | 0 | **N**: "20,8 s / 19,7 s" (hiệu năng) — không có log |
| `kt212-attack-path-mod` | 5 | 5 | 0 | 0 | 0 | 28 biến thể (85 tệp matrix), 2 đường lọt, 4/4 đối chứng |
| `kt213-path-base` | 5 | 5 | 0 | 0 | 0 | 157→165, `errors=0`, PASS |
| `kt214-depinfo-prototype` | 10 | 9 | 0 | 1 | 0 | **N**: "`--selftest` 20/20" không có log trong `kt214-logs/` (chỉ ở `kt217-logs/`) |
| `kt215-target-filter` | 6 | 6 | 0 | 0 | 0 | 165→176, `verify-numbers ok=5 fail=0` |
| `kt216-merge` | 8 | 8 | 0 | 0 | 0 | 194/194, 175+19, 9 đột biến, 6 đối chứng |
| `kt217-depinfo-layer2` | 5 | 5 | 0 | 0 | 0 | 26,92 s / 8,26 s, `errors=0`, PASS |
| **Tổng** | **190** | **183** | **1** | **6** | **0** | |

Ngoài bảng (mục D): **D2-174** — **KHÔNG CHỨNG CỨ** (xem §5).

---

## 2. CHI TIẾT CÁC CHỖ LỆCH

### 2.1 `kt182-help-map.md` §4.1 — "35" phải là **36** (LỆCH, off-by-one)

Journal dòng 99: `| Tệp \`src/\` có khối \`#[cfg(test)]\` | 35 |`

Log `kt182-logs/kt182-measure.log` (và `.json` `summary.n_inline_files`) ghi **36**:

```text
n_inline_files: 36
```

Đo lại độc lập trên cây hiện tại bằng chính `build_scope()` của checker:

```text
n_inline_files 36
inline in src/ 36
files with inline cfg(test) mod: 36
files with cfg(test) impl: 0
```

⇒ **Mọi** tệp có phạm vi test nội tuyến đều nằm dưới `src/`, nên con số đúng là **36**, không
phải 35. Đây là **số dẫn xuất viết tay**, không lấy từ log. Ảnh hưởng: **không** đổi kết luận
Q1 = 0 / Q4 = 0.

**Sửa**: `35` → `36`.

### 2.2 `kt182-help-map.md` §4.1 — "986 hàm sản phẩm" (KHÔNG NGUỒN)

Journal dòng 100: `| Hàm sản phẩm trong các tệp đó | 986 |`

`grep -rn "986" kt182-logs/` ⇒ **không có**. `measure-local-map.py` **không** tính chỉ số này
(không có biến `n_product_fns`). ⇒ Số **không có nguồn**.

**Sửa**: bỏ dòng, hoặc thêm script/log tính lại.

### 2.3 `kt203-review182.md` §1.2 — "173" và "3" (KHÔNG NGUỒN)

Journal dòng 74–75 ghi cột "L41 độc lập" = **173** và **3**, và §0 khẳng định "cho **đúng từng
con số** 1019 / 996 / 23 / 0 / 155 / 0 / 80 / **173** / **3**".

Nhưng `kt203-logs/independent-measure.log` **không có** hai khoá này:

```text
root: n_files: n_whole: n_inline_files: n_tests: n_own: n_local:
n_local_test_scope: n_local_product: n_global: n_none: n_call_product:
n_call_product_no_own: n_newly_red: n_product_only: g_teeth_size: n_item_cfg_test:
```

`grep -nE "173|collision|multi_def" independent-measure.py` ⇒ **không có**. JSON được dẫn
(`/tmp/l41-review/independent.json`) nằm **ngoài repo**, không kiểm được.

⇒ Hai con số **173** và **3** là **của tác giả KT-182** (có trong `kt182-measure.log`), **không**
phải kết quả đo độc lập của L41. Câu "đúng từng con số … 173 / 3" là **sai nguồn**.

**Sửa**: ghi rõ 173/3 là *đối chiếu với log tác giả*, không phải *đo độc lập*; hoặc bổ sung
script/log.

### 2.4 `kt211-include-global.md` §5 — "20,8 s / 19,7 s" (KHÔNG NGUỒN)

Journal dòng 222: "checker trên cây thật ~20,8 s (bản cũ ~19,7 s cùng máy đang tải) — **+5%**".

`grep -rn "20,8\|19,7\|20.8\|19.7" kt211-logs/` ⇒ **không có**. Không log nào ghi thời gian
checker. ⇒ Số hiệu năng **không có nguồn**.

**Sửa**: bỏ, hoặc thêm log `time`.

### 2.5 `kt214-depinfo-prototype.md` §2.1 — "`--selftest` 20/20" (KHÔNG NGUỒN **trong journal này**)

Journal dòng 4 và §2.1 khẳng định "20 ca `--selftest`", "`--selftest` — 20/20".

`kt214-logs/` (48 tệp) **không có** log `--selftest` của `check-wake-depinfo.py`. Chuỗi `20/20`
chỉ xuất hiện ở `kt217-logs/kt217-v2check-with-depinfo.log` (dòng `--- selftest wake dep-info
(KT-217): OK`).

⇒ Bằng chứng **có tồn tại** nhưng ở **journal khác**; `kt214` dẫn thiếu. Mức: **thấp** (không
phải bịa).

**Sửa**: thêm log `kt214-selftest.log`, hoặc dẫn `kt217-logs/kt217-v2check-with-depinfo.log`.

---

## 3. ĐỘT BIẾN TỰ DỰNG LẠI (yêu cầu ≥3, đã làm **4**)

Mỗi đột biến: tạo tệp trên cây thật của worktree này → chạy `check-wake-paths.py` → xoá tệp.
Log: `kt220-logs/kt220-m{1..4}-*.log`. Cây sạch sau mỗi lượt (`git status` chỉ còn `kt220-logs/`).

| # | Đột biến (theo journal) | Kỳ vọng journal | Kết quả đo | Log |
|---|---|---|---|---|
| M1 | KT-181 M1 — `macro_rules!` chứa `wake` trong `init/` | ĐỎ `E_STRICT_MACRO_WAKE` @ `:5` | **ĐỎ** `E_STRICT_MACRO_WAKE` @ `init/kt220_evil_wake.rs:5`, `errors=1`, RC=1 | `kt220-m1-kt181.log` |
| M2 | KT-215 #12 — `src/target/evil.rs` chứa `wake` | ĐỎ **cả hai** chế độ | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` @ `:4`, `errors=1` **cả** thường **và** `--strict`, RC=1 | `kt220-m2-kt215.log` |
| M3 | KT-211 B01 — `sink(include!{…})` trong `( … )` | ĐỎ (1 lỗi) cả hai chế độ | **ĐỎ** `E_STRICT_WAKE_INCLUDE` @ `:6`, `errors=1` cả hai chế độ, RC=1 | `kt220-m3-kt211.log` |
| M4 | KT-209 L2 — crate **mới** dưới `v2/kernel/` chứa `wake` | ĐỎ `E_STRICT_TEST_ONLY_WAKE` | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` @ `vifast-kt220new/src/lib.rs:2`, RC=1 | `kt220-m4-kt209.log` |

**Kết luận**: **4/4 khớp**. Các bản vá **thật sự cắn** trên cây hiện tại (HEAD `1c5c2946`), không
chỉ trên cây của tác giả. Baseline trước/sau: `errors=0`, RC=0.

---

## 4. NHẤT QUÁN GIỮA CÁC JOURNAL

### 4.1 Tiến trình selftest — **cộng dồn hợp lý, không mất ca**

Dãy trong phiếu: `118 → 128 → 144 → 157 → 165 → 175 → 176 → 194`. Kiểm từng bước:

| Bước | Nguồn | Số ca | Log |
|---|---|---|---|
| Gốc `8f5a1189` | kt181 | 118 | `kt181-logs/baseline-selftest.log` |
| KT-181 (+10) | kt181 | **128** = 118+10 | `after-selftest.log` |
| KT-202 (+16) | kt202 | **144** = 128+16 | `kt202-after-selftest.log` |
| KT-209 (+13) | kt209 | **131** = 118+13 (nhánh **khác**, gốc `cfee57d8`) | `05-selftest.log` |
| KT-211 (+18) | kt211 | **175** = 157+18 | `00-selftest-after.log` |
| KT-213 (+8) | kt213 | **165** = 157+8 | `after-selftest.log` |
| KT-215 (+11) | kt215 | **176** = 165+11 | `after-selftest.log` |
| KT-216 (merge) | kt216 | **194** = 175+19 (= 8+11) | `01-selftest-after-merge.log` |

**Nhận xét trung thực**: dãy trong phiếu là **tuyến tính hoá một DAG**, không phải chuỗi tuần tự.
`165` và `175` là **hai nhánh song song** cùng gốc `157` (KT-213 và KT-211), nên **không** thể
cộng `165 + 18 = 183`; đúng phải là `157 + 18 = 175`. Bản thân các journal **không** nói sai —
mỗi journal ghi đúng gốc nhánh của mình. Nhưng nếu lead viết "118 → … → 165 → 175 → 176 → 194"
trong báo cáo A0 thì **dễ bị hiểu là tuần tự** và người đọc sẽ tính ra 183.

**Không mất ca nào**: 194 = 175 + 19, và 19 = 8 (KT-213) + 11 (KT-215). Khớp `kt216` §2.1.

**Đề nghị**: trong báo cáo A0 ghi rõ "157 → **{165, 175}** song song → 176 → 194".

### 4.2 Mười ba đường lọt — mỗi đường có journal + log?

| # | Đường lọt | Journal | Log | Kết luận |
|---|---|---|---|---|
| 1 | `macro_rules!` `:item` + `include!` trong đối số lời gọi | `kt201` §B1 | `kt201-logs/B1-repro-*.log` | **có** |
| 2–6 | `include!` vị trí biểu thức trong `( … )`/`[ … ]` (5 biến thể) | `kt208` §B1 | `kt208-logs/B-B01..B25-*.log` | **có** |
| 7 | `#[path]` trong inline module tệp non-mod-rs | `kt212` §1 | `kt212-logs/repro1-*.log` | **có** |
| 8 | `#[path]` ra ngoài `src/` qua `..` | `kt214` §3 bảng dòng 8 | `kt214-logs/p08-*.log` | **có** |
| 9 | `include!` trong `v2/lib` trỏ sang `v2/kernel` | `kt214` §3 dòng 9 | `kt214-logs/p09-*.log` | **có** |
| 10 | `macro_rules!` `:expr` + `include!` | `kt214` §3 dòng 10 | `kt214-logs/p10-*.log` | **có** |
| 11 | `mod` trong lời gọi macro (`cfg_if!`) | `kt214` §3 dòng 11 | `kt214-logs/p11-*.log` | **có** |
| 12 | thư mục tên `target` (`src/target/evil.rs`) | `kt212` §5 + `kt215` | `kt212-logs/matrix/B8-*`, `kt215-logs/repro12-*` | **có** |
| **13** | **vùng test quá rộng trên nhánh `match`** | **KHÔNG CÓ** | **KHÔNG CÓ** (auditor tự tái lập được — §5.2) | **KHÔNG CHỨNG CỨ** (kỹ thuật **đúng**) |

**Phát hiện quan trọng**: đường lọt **#13** chỉ tồn tại trong `DECISIONS.md` D2-174 và trong
commit message `1c5c2946`. **Không** có `kt218-*.md`, **không** có `kt219-*.md`, **không** có
`kt218-logs/`, **không** có `kt219-logs/`:

```text
$ ls v2/agent/JOURNAL/ | grep -E "^kt21[89]"
(không có kết quả)
```

Hai commit `aadd818c` và `1c5c2946` **chỉ chạm** `v2/agent/QUEUE.md` và `v2/agent/DECISIONS.md`
— không có mã, không có log, không có journal.

⇒ **Không thể audit** đường lọt #13. Đây là **lỗ hổng bằng chứng lớn nhất của đêm**.

### 4.3 Đánh số đường lọt **không nhất quán** giữa các journal

Hai hệ đánh số song song cho **cùng** một tập đường lọt:

| Đường lọt | `kt212`/`kt213`/`kt215` gọi | `kt217` gọi |
|---|---|---|
| `#[path]` inline module tệp non-mod-rs | **#11** | **#7** |
| thư mục `target` | **#12** | **#12** |
| `#[path]` ra ngoài `src/` qua `..` | — | **#8** |
| `include!` `v2/lib` → `v2/kernel` | — | **#9** |
| `macro_rules!` `:expr` + `include!` | — | **#10** |
| `mod` trong lời gọi macro | — | **#11** |

⇒ Cùng nhãn "#11" chỉ **hai đường khác nhau** tuỳ journal. `D2-171` dùng hệ `kt212` (#11 =
`#[path]` non-mod-rs); `D2-174` dùng hệ `kt217` (#13 = sau #12). **Không** phải sai số, nhưng
báo cáo A0 phải chốt **một** hệ đánh số, nếu không sẽ lệch.

### 4.4 Journal tác giả nói "ĐẠT" / reviewer nói "KHÔNG ĐẠT"

| Cặp | Tác giả | Reviewer | Xử lý | Đánh giá |
|---|---|---|---|---|
| KT-202 | `kt202` §0: "Đã vá? **CÓ**" | `kt208` §0: "**KHÔNG ĐẠT**" | `D2-170`: KT-202 → DONE nhưng **luật CHƯA đóng**; mở KT-211 | **Trung thực**: `kt202` §5/§6 **đã** tự ghi `:expr` chưa đóng; reviewer phát hiện **lớp** rộng hơn. Không phải journal sai, là **phạm vi** khác. |
| KT-182 | `kt182` §5: quyết định **(a) SIẾT** | `kt203` §0: "**ĐẠT CÓ ĐIỀU KIỆN**" + F1 (ca 35/36 **không răng**) | `kt206` vá F1/F2/F3/F5 | **Trung thực**: `kt203` xác nhận số đo tác giả đúng; F1 là lỗi **chất lượng test**, đã vá. |
| KT-183 | `kt183` §1: "ĐẠT" (ngầm) | `kt204` §0: "**ĐẠT CÓ ĐIỀU KIỆN**" + 2 lỗ hổng ca kiểm + 2 sai số journal | `kt205` vá + **đính chính** `kt183` | **Trung thực cao**: `kt183` đã được **đính chính tại chỗ** (giữ nguyên lịch sử), không xoá dấu. |
| KT-211 | `kt211` §0: "**ĐẠT**" | (không có reviewer độc lập sau) | `kt216` merge kiểm lại | **Thiếu reviewer độc lập** — chỉ tác giả tự chấm. Ghi nhận, không phải lệch số. |
| KT-214 | `kt214` §0: "**ĐẠT CÓ ĐIỀU KIỆN**" | (không có reviewer độc lập) | `kt217` áp dụng | **Thiếu reviewer độc lập**. |

### 4.5 Sai số đã được **tự đính chính** (điểm cộng trung thực)

- `kt183` §5 "rev trong ảnh" — **SAI**, đã đính chính bằng khối `ĐÍNH CHÍNH (KT-205)` ở đầu tệp,
  kèm dấu ⚠ tại đúng chỗ. Bằng chứng chéo đúng: build từ `4fc80f61`, `0118d1ea`, `cd74d73e`,
  `8c921bbe` cho **cùng** sha `639ca6d1…`.
- `kt183` §3/§8 dẫn `kt183-logs/run-clean/qemu.log` — tệp **không có** trong nhánh; đã đính chính
  và bổ sung `kt205-logs/kt205-qemu-clean.log`.
- `kt182` §4.2 "cả `rig`, `run`" — đã sửa theo F5 của `kt203` (chỉ `run` bắt buộc).

⇒ Đây là hành vi **đúng** của một quy trình trung thực: sai thì ghi lại, không xoá.

---

## 5. `DECISIONS.md` D2-169..D2-174 — đối chiếu journal + commit

| Quyết định | Nội dung chính | Journal khớp? | Commit khớp? | Kết luận |
|---|---|---|---|---|
| **D2-169** | Đảo D2-168; KT-194/KT-184 → REVIEW; L40 (KT-201) tìm đường lọt #1; `8f5a1189` RC=0, `errors=0` hai chế độ | `kt201` §A1/A2/§B1 — **khớp** | `74e2b2c9`/`e4b69f9c` (KT-208) | **KHỚP** |
| **D2-170** | Đường lọt #10; KT-202 **KHÔNG ĐẠT**; mở KT-211; chuyển sang quét token toàn cục | `kt208` §B1 (6 biến thể) — **khớp** | `41e568bc` (KT-211 D2-170) | **KHỚP** |
| **D2-171** | Đường lọt #11 (`#[path]` non-mod-rs); mở KT-213; đề xuất dài hạn `--emit=dep-info` | `kt212` §1 — **khớp** | `dc6e01ae` (KT-213 D2-171) | **KHỚP** |
| **D2-172** | KT-213 **ĐẠT**; đường lọt #12 (`target`); mở KT-215; ma trận 28 biến thể, 16/16, selftest 157→165 | `kt213` + `kt212` §2 — **khớp** | `17935908` (KT-215 D2-172) | **KHỚP** |
| **D2-173** | Nối `check-wake-depinfo.py` vào `v2 check` lớp 2; 11/11 vs 5/11; 26,92 s / 8,26 s; `v2 check` 2 m 45 s | `kt217` §5/§6 — **khớp** | `e4f28098` (KT-217) | **KHỚP** |
| **D2-174** | Đường lọt **#13** (vùng test quá rộng trên nhánh `match`); lọt **cả hai lớp**; mở KT-219 | **KHÔNG CÓ JOURNAL** | `1c5c2946` **chỉ** chạm `DECISIONS.md` + `QUEUE.md` | **KHÔNG CHỨNG CỨ** |

### 5.1 D2-174 — ba vấn đề cụ thể

1. **Không có journal, không có log.** `D2-174` khẳng định một đường lọt **SỐNG** "nguy hiểm
   nhất từ đầu đêm" (phá **cả** lớp 1 **và** lớp 2), nhưng **không** có `kt218-*.md`/`kt219-*.md`
   và **không** có `kt218-logs/`/`kt219-logs/`. Không có log `cargo check RC=0`, không có log
   checker XANH, không có log đối chứng âm. Theo đúng luật của chính đêm nay ("không tuyên bố
   đóng/mở đường lọt khi không có đột biến + log"), **D2-174 chưa đủ bằng chứng để báo A0**.

2. **Số hiệu task bị trùng.** `D2-174` gán "reviewer **L49 (KT-218)**", nhưng commit `aadd818c`
   `[KT-218]` là commit **sổ** ("so: KT-181/183/202/204/205/209 DONE — DONE=189 READY=0
   REVIEW=3"), không phải task review. Task review tìm #13 **không có số hiệu riêng** trong
   `QUEUE.md`/journal.

3. **Không có mã trong commit.** `1c5c2946` chỉ chạm `DECISIONS.md` + `QUEUE.md`; **không** có
   `v2/tools/check-wake-paths.py` (bản vá KT-219) và **không** có ca selftest mới. Nghĩa là
   đường lọt #13 **vẫn đang mở** trên HEAD `1c5c2946` — nếu đúng như D2-174 mô tả.

### 5.2 Kiểm chứng D2-174 bằng tay (auditor tự dựng) — **ĐÚNG về kỹ thuật**

Vì D2-174 không có log, tôi **tự dựng** repro theo đúng mô tả §(1)/(3). Log:
`kt220-logs/kt220-d2174-probe.log`. Bốn hình dạng đã thử:

| # | Hình dạng | `errors=` (thường) | Kết luận |
|---|---|---|---|
| V1 | `#[cfg(test)] 0 => (),` (pattern **literal**) + `1 => { m.wake(t) }` | **1** (ĐỎ) | không lọt |
| **V2** | `#[cfg(test)] A => (),` (pattern **ident**) + `B => { let _ = m.wake(t); }` | **0** (XANH) | **LỌT — tái lập được** |
| V3 | `#[cfg(test)] A => {}` (nhánh có thân `{}`) + `B => { m.wake(t) }` | **1** (ĐỎ) | không lọt |
| **V4** | V2 + dấu `,` sau nhánh `B` | **0** (XANH) | **LỌT — tái lập được** |

⇒ **D2-174 ĐÚNG về kỹ thuật**: với `#[cfg(test)]` trên nhánh `match` có pattern **ident**
(`A => (),`), vùng test bị kéo sang nhánh kế tiếp và `m.wake(t)` trong nhánh sản phẩm **không bị
báo** — `errors=0` cả hai chế độ. Điều kiện để lọt: pattern là **ident** (không phải literal) và
nhánh **không** có thân `{}` — khớp đúng mô tả "bỏ qua dấu `,`, chỉ dừng ở `;`" của D2-174 §(3).

**Nhưng** khẳng định "lọt **CẢ HAI** lớp" (lớp 2 `check-wake-depinfo.py` cũng XANH) **chưa** kiểm
được trong audit này: `check-wake-depinfo.py` cần `cargo check` + `.d`; tôi **không** chạy build
trong audit (ngân sách + không muốn làm bẩn `target/`). Ghi rõ là **chưa xác nhận**.

**Kết luận cho lead**: D2-174 **không** phải khẳng định sai — nó **đúng** và tôi tái lập được.
Vấn đề là **quy trình**: đường lọt #13 là đường **SỐNG** trên HEAD `1c5c2946` mà **không có**
journal, **không có** log, **không có** bản vá, **không có** ca selftest. Nếu gửi A0 "13 đường
lọt" thì A0 sẽ hỏi bằng chứng và **không có** để đưa.

---

## 6. NHỮNG CHỖ LEAD PHẢI SỬA TRƯỚC KHI GỬI A0

Xếp theo mức ưu tiên.

### P0 — chặn báo cáo

1. **`D2-174` / đường lọt #13 không có journal + không có log.** Tôi **đã tái lập được** đường
   lọt này (§5.2, V2/V4 ⇒ `errors=0`), nên khẳng định **đúng về kỹ thuật** — nhưng **không có**
   `kt219-*.md`, **không có** `kt219-logs/`, **không có** bản vá, **không có** ca selftest. Trước
   khi gửi A0, phải **hoặc** (a) viết `kt219-*.md` + `kt219-logs/` với repro + log đỏ/xanh (dùng
   hình dạng **V2/V4** ở §5.2 — hình dạng **V1/V3 KHÔNG lọt**), **hoặc** (b) ghi rõ trong báo cáo
   là "đường lọt #13: tái lập được, **chưa** có journal/log, **chưa** vá". Không được báo A0
   "13 đường lọt" như thể đã có hồ sơ đầy đủ.
2. **`D2-174` gán sai số hiệu task** ("reviewer L49 (KT-218)" trong khi `[KT-218]` là commit sổ).
   Cần một số hiệu riêng cho task review #13.
3. **Đường lọt #13 vẫn đang MỞ trên HEAD `1c5c2946`** — commit `1c5c2946` chỉ chạm
   `DECISIONS.md` + `QUEUE.md`, **không** chạm `v2/tools/check-wake-paths.py`. Nếu báo cáo A0 nói
   "luật `wake` đã đóng" thì **sai**: còn ít nhất một đường **SỐNG** đã tái lập được.

### P1 — sửa số trong journal (nhỏ, nhưng phải sửa vì A0 sẽ đọc)

3. **`kt182-help-map.md` §4.1 dòng 99**: `35` → **`36`** (log `n_inline_files: 36`; đo lại = 36).
4. **`kt182-help-map.md` §4.1 dòng 100**: bỏ "986 hàm sản phẩm" (không có trong log/script) hoặc
   thêm phép đo.
5. **`kt203-review182.md` §0 + §1.2**: "173" và "3" **không** phải kết quả đo độc lập của L41
   (`independent-measure.py` không tính). Sửa cột "L41 độc lập" thành "đối chiếu log tác giả",
   hoặc bổ sung phép đo.
6. **`kt211-include-global.md` §5 dòng 222**: bỏ "~20,8 s / ~19,7 s / +5%" (không có log), hoặc
   thêm log `time`.
7. **`kt214-depinfo-prototype.md` §2.1**: "`--selftest` 20/20" — dẫn `kt217-logs/kt217-v2check-with-depinfo.log`
   hoặc thêm `kt214-logs/kt214-selftest.log`.

### P2 — trình bày trong báo cáo A0 (không phải sửa journal)

8. **Dãy selftest**: ghi `157 → {165, 175} song song → 176 → 194`, **không** viết tuyến tính
   `165 → 175` (người đọc sẽ tính 183 và tưởng mất 8 ca).
9. **Chốt một hệ đánh số đường lọt** (hiện có hai: `kt212` #11 = `#[path]` non-mod-rs; `kt217`
   #11 = `mod` trong lời gọi macro). Đề nghị dùng hệ `kt217` (12 đường) + #13.
10. **Ghi rõ KT-211 và KT-214 chưa có reviewer độc lập** — chỉ tác giả tự chấm (KT-216 merge
    kiểm lại KT-211 nhưng là cùng nhánh tác giả).
11. **Ghi nhận điểm trung thực**: `kt183` đã tự đính chính 2 sai số (rev trong ảnh; `run-clean/qemu.log`
    thiếu) — nên nêu trong báo cáo A0 như bằng chứng quy trình hoạt động, **không** giấu.

---

## 7. NHỮNG GÌ **KHÔNG** TÌM THẤY SAI (nói rõ để không bịa lệch)

- **Không** có journal nào bịa sha ảnh: cả 6 sha (`639ca6d1`, `9e2b7849`, `6e477524`, `caeb2b50`,
  `63db1f7f`, `3f7bb9ad`) đều khớp `shasum`/`REPRO` trong log.
- **Không** có journal nào bịa dấu vân tay: `43df126a4a6702ec` / `89f40ef16c97e6d5` khớp log build
  và `v2 status`.
- **Không** có journal nào bịa số ca selftest: 118/128/131/144/157/165/175/176/194 đều khớp
  `# selftest: N/N ok` trong log tương ứng.
- **Không** có journal nào bịa số ca đỏ của đột biến: M1 5/8, M2 8/8, M3 3/8, M4 8/8, A 5/8, B 4/8,
  M2(KT-205) 7/9, M4(KT-205) 1/3 — đều khớp dòng `FAIL` trong log.
- **Không** có journal nào bịa "v2 check PASS": mọi log `v2 check` được dẫn đều có dòng
  `v2 check: PASS` (trừ các lượt **đã khai** là T-INFRA SIGTRAP ở `kt203`/`kt204`/`kt206`/`kt208`
  — các journal đó **đều ghi rõ** là T-INFRA, không giấu).
- **Không** có journal nào nới ngưỡng/allowlist để lấy xanh (đã rà: `HELPER_ALLOW` rỗng,
  `ALLOW_NO_ASSERT` rỗng, `MACRO_WAKE_ALLOW` rỗng, `ALLOW_LET_UNDERSCORE` 20 mục không đổi).

---

## 8. LOG CỦA AUDIT

| Tệp | Nội dung |
|---|---|
| `kt220-logs/kt220-m1-kt181.log` | đột biến KT-181 M1 — ĐỎ `E_STRICT_MACRO_WAKE` |
| `kt220-logs/kt220-m2-kt215.log` | đột biến KT-215 #12 — ĐỎ cả hai chế độ |
| `kt220-logs/kt220-m3-kt211.log` | đột biến KT-211 B01 — ĐỎ `E_STRICT_WAKE_INCLUDE` |
| `kt220-logs/kt220-m4-kt209.log` | đột biến KT-209 L2 — ĐỎ `E_STRICT_TEST_ONLY_WAKE` |
| `kt220-logs/kt220-d2174-probe.log` | tự dựng repro D2-174 (đường lọt #13) |
| `kt220-logs/kt220-baseline.log` | checker trên cây sạch trước/sau audit: `errors=0` |

**Không** sửa: `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`, `docs/**`, `AGENTS.md`,
`v2/AGENTS.md`, **và mọi journal của agent khác**. **Không** push. **Không** chạy `gate --fast`.
