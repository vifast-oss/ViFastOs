# KT-264 — nợ §7.3: luật **cấu trúc** cho crate ngoài `V2_SCAN`

**Ngày**: 2026-11-21 · **Task**: KT-264 · **Nhánh**: `task/kt264-struct` · **Worktree**:
`/tmp/kt264-s1` · **Base**: `main` @ `223d4f74` · **Điều kiện đóng**: D2-190 (7 mục).
**Tiền đề**: `JOURNAL/kt258-wake-attack.md` §7.3 (nợ cấu trúc), `JOURNAL/kt256-wake-debt.md`
(phạm vi một nguồn từ `cargo metadata`), `BYPASS-REGISTRY.md` (WB-01..WB-23),
`reports/CHECKER-WAKE-CONTRACT.md`, D2-188 (phạm vi từ metadata), D2-190 (tiêu chí đóng),
D2-204 (nhiệm vụ vòng này).

**Ràng buộc đã giữ**: **không** push/merge, **không** chạm `main`, **không** sửa
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `spec/`, **không** sửa `v2/bin/v2`,
**không** nới ngưỡng, **không** tắt ca, **không** allowlist rộng. Mọi biến thể chèn vào cây đều
**khôi phục** bằng `trap` (`v2/kernel`, `v2/sim`, `v2/lib`, `v2/Cargo.toml`,
`v2/kernel/Cargo.toml`, crate ở gốc mới, nguồn ngoài cây). Mọi thay đổi có **test đỏ trước**
(D2-05): 5 ca selftest mới **đỏ** trên bản checker cũ (§5.2).

---

## 1. Cách tấn công

Harness mới `v2/agent/JOURNAL/kt264-attack/` (khuôn `kt258-attack/`, **không** sửa harness cũ):
`run-attack.sh <nhóm> <tên> <ca.sh>` chèn biến thể vào cây **thật**, đo
`cargo check --manifest-path v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf` + **ba**
lượt kiểm (`check-wake-paths.py`, `--strict`, `check-wake-depinfo.py`), rồi khôi phục bằng
`trap`. Mỗi ca có `EXTRA_CARGO` (cấu hình `--features product-fs` hoặc workspace host) để chứng
minh mã **thật sự được biên dịch**, không chỉ "cargo RC=0 vì crate không được build".

Chỉ tính là **đường lọt** khi `cargo` RC=0 **và** cả ba lượt `errors=0`. Bảng đầy đủ:
`kt264-attack.tsv`; log từng ca ở `kt264-attack/logs/<tên>-{truoc,sau}-va/`.

**Khác KT-258**: vòng này **không** tấn công luật token/`include!` (đã đóng ở KT-258) mà tấn
công **luật cấu trúc** — `scan_structure` + `mod` graph. Vì vậy mỗi ca có **hai** lớp bằng chứng:
(a) ca **literal** đúng như đề bài, (b) ca **biến thể cấu trúc** cô lập đúng lỗ hổng cấu trúc.

---

## 2. Kết quả ma trận — **9 biến thể, 5 đường lọt**

### 2.1 Năm ca **literal** (đúng đề bài S1..S5)

| Ca | Dựng gì | `cargo` | l1 | l1s | l2 | Kết luận |
|---|---|---|---|---|---|---|
| **S1** `s1-newroot-mod-inner` | crate `v2/drivers/evil` (gốc mới, path-dep **tuỳ chọn** bật bởi `product-fs`), `src/lib.rs` có `mod inner;`, `src/inner.rs` gọi `wake` | 0 | 1 | 1 | 0 | **bị bắt** — `E_STRICT_TEST_ONLY_WAKE` (KT-258/WB-21) |
| **S2** `s2-path-outside-crate` | `#[path = "../../../evil.rs"] mod evil;` trong `vifast-core` (thuộc `V2_SCAN`), đích `v2/evil.rs` gọi `wake` | 0 | 1 | 1 | 2 | **bị bắt** — `E_STRICT_WAKE_INCLUDE` (KT-258/WB-22) + lớp dep-info |
| **S3** `s3-newroot-mod-nested3` | `mod` lồng **3 tầng** trong crate ở gốc mới, tệp lá gọi `wake` | 0 | 1 | 1 | 0 | **bị bắt** — `E_STRICT_TEST_ONLY_WAKE` (KT-258/WB-21) |
| **S4** `s4-newroot-include-in-mod` | `include!` **bên trong** `mod` của crate ở gốc mới, đích gọi `wake` | 0 | 2 | 2 | 0 | **bị bắt** — `E_STRICT_WAKE_INCLUDE` (KT-258/WB-22) |
| **S5** `s5-newroot-orphan` | tệp `.rs` **mồ côi** (`src/orphan.rs`, không `mod` nào khai báo) trong crate ở gốc mới | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT** |

**Phát hiện quan trọng — tiền đề của đề bài bị **bác bỏ** bởi phép đo**: bốn ca literal
S1–S4 **không** phải đường lọt sau KT-258. Luật token `wake` (WB-21) quét **mọi** tệp `.rs`
trong `token_scope` — **không** phụ thuộc `mod` graph — nên `wake` trong tệp con của `mod` graph
ở crate gốc mới **vẫn bị bắt**; luật `include!`/`#[path]` (WB-22) đã seed worklist bằng
`files + scope_targets` nên `include!`/`#[path]` trong crate gốc mới **vẫn bị phân giải**. Nói
cách khác: **`mod` graph không dựng lại được đường lọt `wake`** — nó chỉ dựng lại được đường lọt
**cấu trúc**. Đây là câu trả lời trực tiếp cho câu hỏi của lead ("có thể dựng lại đường lọt bằng
`mod` graph") — **không**, với `wake`; **có**, với cấu trúc.

### 2.2 Bốn ca **biến thể cấu trúc** (cô lập đúng lỗ hổng)

| Ca | Dựng gì | `cargo` | l1 | l1s | l2 | Kết luận |
|---|---|---|---|---|---|---|
| **S1b** `s1b-newroot-mod-missing` | `#[cfg(any())] mod ghost;` (không có `ghost.rs`) trong crate ở gốc mới | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT** |
| **S2b** `s2b-path-cross-crate` | `#[path = "../../vifast-plat-common/src/evil.rs"] mod evil;` trong `vifast-core`: đích **trong `v2/kernel`** nhưng **ngoài crate** | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT** |
| **S3b** `s3b-newroot-mod-nested3-missing` | `mod` lồng **3 tầng** ở crate gốc mới, tầng **lá** có `#[cfg(any())] mod c;` (không có `c.rs`) | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT** |
| **S4b** `s4b-lib-orphan-bad-mod` | tệp **mồ côi** trong `v2/lib/vifast-net/src/` có `mod 123;` (hỏng) trong nhánh `#[cfg(any())]` | 0 | 0 | 0 | 0 | **ĐƯỜNG LỌT** |

**Tổng**: **9 biến thể**; **5 đường lọt** (S5, S1b, S2b, S3b, S4b) — **tất cả** thuộc **một**
lớp: **luật cấu trúc không chạy ngoài `V2_SCAN`** + **`_mods_of` bỏ im lặng cạnh `mod` không
phân giải được**. Bốn ca literal S1–S4 **bị bắt** (đã đóng ở KT-258) và được giữ làm **ca hồi
quy**.

---

## 3. Ba gốc của đường lọt cấu trúc

### 3.1 G1 — `scan_structure` chỉ chạy `V2_SCAN` (nợ §7.3)

`scan_structure` duyệt `("v2/kernel", "v2/sim")`; `scan_lexical` duyệt thêm `v2/lib`. Crate ở
**gốc mới** trong **bao đóng sản phẩm** (`v2/drivers/evil`) và crate `v2/lib` **không** được
kiểm cấu trúc/từ vựng. Hệ quả đo được:

- **S4b** — `mod 123;` (khai báo `mod` hỏng) trong `v2/lib/vifast-net/src/` ⇒ **XANH cả ba**
  trước vá (`E_LEX_STRUCT` không bao giờ chạy ở `v2/lib`).
- **S5** — tệp `.rs` **mồ côi** trong crate gốc mới ⇒ **XANH cả ba** (không luật nào kiểm
  "tệp `.rs` có thuộc `mod` graph không").

### 3.2 G2 — `_mods_of` **bỏ im lặng** cạnh `mod` không phân giải được

`_mods_of` chỉ `out.append(...)` khi `cand.is_file()`; cạnh `mod NAME;` không có tệp đích **biến
mất** khỏi đồ thị, không lỗi, không cảnh báo. `resolve_include_targets` chỉ phân giải `#[path]`,
**không** phân giải `mod NAME;` trơn. Hệ quả đo được:

- **S1b** — `#[cfg(any())] mod ghost;` trong crate gốc mới ⇒ **XANH cả ba**.
- **S3b** — cùng dạng ở **tầng lá** của `mod` graph 3 tầng ⇒ **XANH cả ba** (đo **độ sâu**: luật
  phải đi hết 3 tầng mới thấy cạnh).

### 3.3 G3 — cạnh `mod`/`#[path]` **ra khỏi crate** không bị chặn

`resolve_include_targets` chặn đích ra ngoài `v2/kernel`+`v2/sim` (và `v2/lib` cho nguồn lib),
nhưng **không** chặn đích ra ngoài **crate**. Một `#[path]` từ `vifast-core` sang
`vifast-plat-common/src/` biên dịch **cùng một tệp** vào **hai** crate với hai phân loại
test/sản phẩm khác nhau ⇒ `is_test` của luật token có thể sai, và `mod` graph của crate không
còn khép kín. Hệ quả đo được:

- **S2b** — `#[path = "../../vifast-plat-common/src/evil.rs"] mod evil;` ⇒ **XANH cả ba**.

---

## 4. Vá (fail-closed, không allowlist)

| Thay đổi | Nội dung |
|---|---|
| `_crate_roots_in(root, crate_dirs)` | **Tách** khỏi `crate_roots` để luật cấu trúc dùng **cùng** một định nghĩa "crate root" cho crate ngoài `V2_SCAN` (một nguồn, không chép luật — cùng lý do KT-213 tách `_module_graph`). Thêm `src/bin/*.rs` + `src/bin/*/main.rs` (crate root **tự nhận** của cargo) — thiếu chúng thì `v2/lib/vifast-elf/src/bin/vifast-elf-validate.rs` bị coi là mồ côi (**báo oan** trên cây thật, đo được 1 ca). |
| `_scope_rs_files(root, scope)` | **Một** vòng lặp cho cả `scan_lexical` + `scan_structure`: mọi `.rs` trong `scope.contains()` (crate bảo vệ ∪ cây nguồn không-phải-crate), không còn hằng `V2_SCAN`/`("v2/kernel","v2/sim","v2/lib")`. |
| `scan_lexical(root, r, scope)` | Phạm vi từ `wake_scope.protected_scope` (nợ §7.3). Trước bản này lỗi từ vựng trong crate gốc mới **không** bị bắt, và `_mods_of`/`resolve_include_targets` **âm thầm** bỏ cạnh của tệp đó (chúng `continue` khi `parse_file` lỗi) ⇒ mù cấu trúc. |
| `scan_structure(root, r, scope)` | Phạm vi từ `wake_scope.protected_scope`; `E_LEX_STRUCT` nay chạy cả `v2/lib` + crate gốc mới. |
| `_crate_mod_walk(root, crate_dir)` | Đi `mod` graph của **một** crate từ crate root; trả `(tệp tới được, cạnh mod, có tệp hỏng?)`. Khác `_mods_of`: **giữ** cạnh không phân giải được (để phát `E_STRUCT_MOD_MISSING`) và **không** lọc `is_file()` (để phát `E_STRUCT_MOD_OUTSIDE`). `child_dir` đúng luật Rust (`#[path]` ⇒ thư mục tệp đích; `mod NAME;` ⇒ `plain_base/NAME`) — bản đầu lấy `target.parent` cho mọi cạnh ⇒ **báo oan** `kobj.rs`/`state_write.rs` trên cây thật (đo được 2 lỗi, đã sửa). |
| `scan_structure_scope(root, r, scope, include_targets)` | **Ba** bất biến, **ba** mã lỗi mới: `E_STRUCT_MOD_MISSING` (cạnh `mod` không phân giải được), `E_STRUCT_MOD_OUTSIDE` (cạnh ra ngoài crate), `E_STRUCT_ORPHAN` (tệp `.rs` dưới `<crate>/src/` không thuộc `mod` graph và không phải đích `include!`). Thêm `E_STRUCT_NO_ROOT` (crate **có** manifest mà **không** có crate root ⇒ fail-closed, không im lặng). Chạy ở **mọi** chế độ (như `include!`/`#[path]` từ KT-184), **sau** `resolve_include_targets` để biết đích `include!`. |

**Vì sao fail-closed**:
- Crate **có** manifest mà **không** có crate root ⇒ `E_STRUCT_NO_ROOT` (không im lặng bỏ qua).
- Crate có tệp **không phân tích được** ⇒ `E_LEX_*` đã ĐỎ; luật mồ côi **không** báo thêm (tránh
  oan) nhưng cổng **vẫn ĐỎ** — không có nhánh nào "bỏ qua rồi PASS".
- Cây **fixture** của selftest dựng metadata **giả** (`scope_from_metadata`) và **không** có
  `Cargo.toml` ⇒ luật cấu trúc bỏ qua (không phải cây cargo thật). Trên cây thật **mọi** crate
  trong `cargo metadata` đều có manifest (D2-188) ⇒ luật **luôn** chạy.
- **Không** allowlist: không có danh sách tệp/crate được miễn. Ngoại lệ duy nhất là **định
  nghĩa** "tệp ngoài `src/`" (`build.rs`, `init/*.rs` do `build.rs` biên dịch riêng, shim
  `vifast-plat-user`) — chúng **không** nằm trong `mod` graph của crate theo đúng luật Rust, nên
  không thuộc luật mồ côi.

---

## 5. Test có răng

### 5.1 Đột biến **hai chiều** trên cây thật (9 ca)

| Biến thể | Trước vá | Sau vá | Mã sau vá |
|---|---|---|---|
| S1 `s1-newroot-mod-inner` | bị bắt (l1=1) | bị bắt (l1=1) | `E_STRICT_TEST_ONLY_WAKE` |
| S2 `s2-path-outside-crate` | bị bắt (l1=1, l2=2) | bị bắt (l1=2, l2=2) | `E_STRICT_WAKE_INCLUDE` + `E_STRUCT_MOD_OUTSIDE` |
| S3 `s3-newroot-mod-nested3` | bị bắt (l1=1) | bị bắt (l1=1) | `E_STRICT_TEST_ONLY_WAKE` |
| S4 `s4-newroot-include-in-mod` | bị bắt (l1=2) | bị bắt (l1=3) | `E_STRICT_WAKE_INCLUDE` + `E_STRUCT_ORPHAN` + `E_STRICT_TEST_ONLY_WAKE` |
| **S5** `s5-newroot-orphan` | **XANH cả ba** | **ĐỎ** l1+l1s | `E_STRUCT_ORPHAN` |
| **S1b** `s1b-newroot-mod-missing` | **XANH cả ba** | **ĐỎ** l1+l1s | `E_STRUCT_MOD_MISSING` |
| **S2b** `s2b-path-cross-crate` | **XANH cả ba** | **ĐỎ** l1+l1s | `E_STRUCT_MOD_OUTSIDE` |
| **S3b** `s3b-newroot-mod-nested3-missing` | **XANH cả ba** | **ĐỎ** l1+l1s | `E_STRUCT_MOD_MISSING` (tầng lá) |
| **S4b** `s4b-lib-orphan-bad-mod` | **XANH cả ba** | **ĐỎ** l1+l1s | `E_LEX_STRUCT` + `E_STRUCT_ORPHAN` |

Log: `kt264-attack/logs/<tên>-truoc-va/` và `-sau-va/`. Đây là **đột biến hai chiều** thật:
5 ca **XANH → ĐỎ**, 4 ca literal **giữ ĐỎ** (hồi quy).

### 5.2 Ca selftest **có răng** (lớp tĩnh: 430 → **442**)

| Ca | Khẳng định | Răng (đo trên bản checker **cũ**) |
|---|---|---|
| **V1** | tệp `.rs` **mồ côi** trong crate ở gốc mới ⇒ `E_STRUCT_ORPHAN` | **`not ok`** trên bản cũ |
| **V2** | **đối chứng dương**: mọi tệp trong `mod` graph ⇒ không `E_STRUCT_ORPHAN` | `ok` (luật không phải luôn-đỏ) |
| **V3** | `mod ghost;` không phân giải được ⇒ `E_STRUCT_MOD_MISSING` | **`not ok`** trên bản cũ |
| **V4** | **đối chứng dương**: `mod ghost;` có `ghost.rs` ⇒ không `E_STRUCT_MOD_MISSING` | `ok` |
| **V5** | `#[path]` ra **ngoài crate** (vẫn trong `v2/kernel`) ⇒ `E_STRUCT_MOD_OUTSIDE` | **`not ok`** trên bản cũ |
| **V6** | **đối chứng dương**: `#[path]` **trong** crate ⇒ không `E_STRUCT_MOD_OUTSIDE` | `ok` |
| **V7** | `mod` hỏng trong `v2/lib` (ngoài `V2_SCAN`) ⇒ `E_LEX_STRUCT` | **`not ok`** trên bản cũ |
| **V8** | **đối chứng dương**: `v2/lib` sạch ⇒ không `E_LEX_STRUCT` | `ok` |
| **V9** | **đối chứng dương**: `src/bin/*.rs` là crate root tự nhận ⇒ không `E_STRUCT_ORPHAN` | `ok` (canh báo oan cây thật) |
| **V10** | **đối chứng dương**: tệp dưới `src/` nạp bằng `include!` **không** phải mồ côi | `ok` |
| **V11** | crate **có** manifest mà **không** có crate root ⇒ `E_STRUCT_NO_ROOT` | **`not ok`** trên bản cũ |
| **V12** | **đối chứng dương**: có `src/lib.rs` ⇒ không `E_STRUCT_NO_ROOT` | `ok` |

**Bằng chứng đỏ-trước (D2-05)**: chép **nguyên văn** khối 12 ca KT-264 vào bản checker **cũ**
(`git show HEAD:v2/tools/check-wake-paths.py`) rồi chạy `--selftest`:
**V1, V3, V5, V7, V11 = `not ok`** (5 ca âm **đỏ** trên bản cũ); **V2, V4, V6, V8, V9, V10,
V12 = `ok`** (7 đối chứng dương **xanh** trên cả hai bản). Sau vá: **12/12 `ok`**.

**Không ca nào bị xoá.** Một ca cũ **giữ nguyên** nhưng phải **nới điều kiện** đúng một chỗ:
`KT-242 N3 đối chứng dương: tests/*.rs cấp crate vẫn XANH` dựng crate **chỉ có** `tests/*.rs`
(không `src/lib.rs`) — luật mồ côi mới coi `src/*.rs` là mồ côi. Nay luật **chỉ** chạy khi crate
có crate root **lib/bin** (không tính `tests/*.rs`, vốn là crate **riêng**); ca cũ **không** đổi
kỳ vọng, chỉ đi qua nhánh mới. Ghi ở đây để vòng sau không phát hiện lại.

---

## 6. Số đo trước/sau **từng lớp**

| Phép đo | Trước KT-264 | Sau KT-264 |
|---|---|---|
| `check-wake-paths.py --selftest` | 430/430 | **442/442** (+12) |
| `check-wake-depinfo.py --selftest` | 67/67 | **67/67** (0 — dùng chung `wake_scope`) |
| `check-wake-paths.py` cây thật | `errors=0` | **`errors=0`** |
| `check-wake-paths.py --strict` cây thật | `errors=0` | **`errors=0`** |
| `check-wake-depinfo.py` cây thật | `errors=0` | **`errors=0`** (20,9 s) |
| `./v2/bin/v2 check` | PASS | **PASS** (log `/tmp/kt264-v2check2.log`) |

**Không báo oan**: cây thật **không** có crate ở gốc mới, **không** có cạnh `mod` ra ngoài
crate, **không** có tệp mồ côi dưới `src/` (đo trước khi vá: 0; sau khi thêm `src/bin/*.rs` là
crate root: 0). Hai lần **báo oan** phát hiện trong lúc làm đã sửa **trước** khi chốt:
`kobj.rs`/`state_write.rs` (sai `child_dir`) và `vifast-elf/src/bin/` (thiếu crate root tự nhận).

---

## 7. Bảng **7 mục D2-190** — trạng thái sau KT-264

| # | Tiêu chí D2-190 | Trạng thái | Bằng chứng |
|---|---|---|---|
| 1 | **Hợp đồng bảo vệ rõ**: crate, target, feature, nguồn sinh, phần hỗ trợ và phần từ chối | **ĐỦ** (mạnh hơn sau KT-264: luật **cấu trúc** nay cùng phạm vi với luật token — `wake_scope.protected_scope`; **bốn** mã cấu trúc `E_STRUCT_MOD_MISSING`/`E_STRUCT_MOD_OUTSIDE`/`E_STRUCT_ORPHAN`/`E_STRUCT_NO_ROOT`; phần **từ chối rõ** gồm crate có manifest mà không có crate root) | `CHECKER-WAKE-CONTRACT.md` §1/§3; `wake_scope.py`; §4 ở trên |
| 2 | **#16–#18 bị chặn**, gồm lỗi đọc sai vị trí đối số khi gọi UFCS | **ĐỦ** (WB-16/17 đóng ở tầng API — `E0308`/`E0631`; WB-18 đóng ở tầng phạm vi — KT-238 + KT-256; vòng này **tái xác nhận** bằng S1/S3: `wake` trong crate gốc mới **vẫn** bị luật token bắt) | `BYPASS-REGISTRY.md`; §2.1 |
| 3 | **Toàn bộ repro cũ giữ bền** + đối chứng hợp lệ | **ĐỦ** (430→442 ca lớp tĩnh, 67→67 ca lớp dep-info; **không** ca nào bị xoá; một ca cũ đi qua nhánh mới, ghi ở §5.2) | `--selftest` hai lớp |
| 4 | **Ranh giới API được compiler cưỡng chế**, không chỉ checker tự xác nhận | **ĐỦ** (KT-241: `NonReadyTarget` không có `Ready`; `write_state` private; mỏ neo kiểu). Vòng này **không** đổi API | §2; `JOURNAL/kt241-checker.md` |
| 5 | **Reviewer khác tác giả** tấn công theo nhóm | **ĐỦ cho vòng này** — **9 biến thể** trên HEAD **sau KT-258**, trọng tâm **cấu trúc** (`mod` graph / tệp mồ côi / cạnh ra ngoài crate / cạnh không phân giải được), đúng nhóm D2-190 §5 ("crate mới/dependency mới/đường dẫn đặc biệt") | §2; `kt264-attack.tsv` |
| 6 | **Gate trên commit tích hợp cuối PASS**, có log + danh tính công cụ | **CHƯA** — task chỉ chạy `v2 check` + hai selftest trên nhánh task; **T2 `gate --fast` trên commit tích hợp** do lead chạy sau merge | §6 |
| 7 | **Không còn đường lọt đã biết** nằm trong hợp đồng bảo vệ | **ĐỦ cho luật cấu trúc** sau khi đóng G1/G2/G3 (mọi đường lọt dựng được trong vòng này đều **ĐỎ** sau vá). Hợp đồng §4 còn **ba** mục mở **có tên** **ngoài** luật cấu trúc: `:expr` passthrough; miễn luật arch theo tiền tố `virtio_`; lớp dep-info chỉ kiểm feature mặc định | `BYPASS-REGISTRY.md`; §8 |

**Kết luận**: **KHÔNG** tự tuyên bố đóng KT-184/KT-194. Còn **mục 6** (gate T2 trên commit tích
hợp — lead) và **mục 7** còn **ba nợ có tên** ngoài luật cấu trúc. Mục 1–5 đạt.

---

## 8. Nợ còn lại **có tên** (không đổi, ghi để vòng sau không phát hiện lại)

1. **`:expr` passthrough** (KT-202) — `$…:expr` chở được `include!` ở vị trí biểu thức; chỉ hở
   khi macro được gọi từ tệp ngoài phạm vi sản phẩm. **Chưa vá** (cây thật có `bail` dùng `:expr`
   hợp lệ).
2. **Miễn luật arch theo tiền tố `virtio_`** — `RE_DOORBELL` bỏ qua tệp tên bắt đầu `virtio_`;
   phân biệt "thiết bị virtio thật" cần kiểm ngữ nghĩa. **Chưa vá**.
3. **Lớp dep-info chỉ kiểm feature mặc định** — `cargo check --workspace` không bật
   `product-fs`; mã chỉ tồn tại dưới feature của ảnh sản phẩm **không** được lớp dep-info thấy.
   Với luật `wake`/cấu trúc điều này **đã được bù** bằng lớp tĩnh (quét **mọi** tệp trong phạm
   vi, không phụ thuộc feature); các luật khác của lớp dep-info (R2/R3/R4) vẫn chỉ thấy cấu hình
   mặc định. **Chưa vá** — cần thêm lượt `cargo check --features product-fs` (~+40 s).
4. **`E_STRUCT_ORPHAN` chỉ áp cho tệp dưới `<crate>/src/`** — tệp `.rs` mồ côi **ngoài** `src/`
   (`build.rs`, `init/*.rs`, shim) **không** bị luật này bắt. Lý do: chúng **không** nằm trong
   `mod` graph của crate theo đúng luật Rust (được `build.rs` biên dịch riêng hoặc là shim
   `include!`). Rủi ro còn lại **thấp**: luật token quét **mọi** `.rs` trong phạm vi nên `wake`
   trong tệp đó vẫn bị bắt. **Chưa vá** — cần mô hình "crate root do `build.rs` sinh" nếu muốn.

---

## 9. Đề xuất cho lead (không tự quyết)

1. **Chạy T2 `gate --fast` trên commit tích hợp** (mục 6 D2-190) — điều kiện **bắt buộc** còn
   thiếu để **đề nghị** đóng KT-184/KT-194.
2. **Đề xuất: CHƯA đóng KT-184/KT-194.** Lý do: (a) mục 6 chưa đạt; (b) mục 7 còn **ba nợ có
   tên** (§8.1–§8.3) **ngoài** luật cấu trúc; (c) §8.4 là giới hạn **đã biết** của luật mồ côi.
   Nếu lead chấp nhận §8.1–§8.4 là **ngoài** hợp đồng hữu hạn đã công bố (D2-190 §7) **và** T2
   xanh trên commit tích hợp ⇒ **đủ điều kiện đề nghị đóng**.
3. **Nối `check-no-cfg-test.py` vào `gate --fast`** (đề xuất của KT-256, vẫn mở).
4. **Thêm lượt `--features product-fs` vào lớp dep-info** (nợ §8.3) — nếu muốn lớp dep-info phủ
   đúng cấu hình ảnh sản phẩm.
5. **Ghi nhận phát hiện**: tiền đề "`mod` graph dựng lại được đường lọt `wake`" **sai** sau
   KT-258 — luật token quét theo **tệp**, không theo `mod` graph. Vòng sau **không** cần tấn
   công lại lớp này cho `wake`; chỉ cần cho **cấu trúc**.

---

## 10. Tái lập

```bash
cd /tmp/kt264-s1
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.100.0

# (a) số ca + cây thật
python3 v2/tools/check-wake-paths.py --selftest          # 442/442
python3 v2/tools/check-wake-depinfo.py --selftest        # 67/67
python3 v2/tools/check-wake-paths.py                     # errors=0
python3 v2/tools/check-wake-paths.py --strict            # errors=0
python3 v2/tools/check-wake-depinfo.py                   # errors=0

# (b) một biến thể (tự khôi phục bằng trap)
bash v2/agent/JOURNAL/kt264-attack/run-attack.sh "S-sau-va" \
     s5-newroot-orphan-sau-va v2/agent/JOURNAL/kt264-attack/cases/s5-newroot-orphan.sh

# (c) cổng
./v2/bin/v2 check
```

**Lưu ý cho người chạy lại**: harness khôi phục `v2/{kernel,sim,lib}` + **hai manifest workspace**
(`v2/Cargo.toml`, `v2/kernel/Cargo.toml`) + crate ở **gốc mới** (`v2/{drivers,net,foo,evil}`) +
tệp phụ `v2/agent/kt264-evil-src.rs` + `v2/evil.rs` + `/tmp/kt264-evil`. `git clean -fd`
**không** xoá tệp bị `.gitignore` ⇒ harness xoá tường minh `v2/{kernel,sim,lib}/*/src/target`.
**Không** chạy `./v2/bin/v2 check` **song song** với harness: bước `api boundary (KT-241)`
fail-closed khi thấy `v2/kernel/vifast-core/src/lib.rs` bẩn (đo được: một lần chạy song song
⇒ `v2 check` FAIL oan; chạy lại trên cây sạch ⇒ PASS).

---

## 11. Tệp thay đổi trong KT-264

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | `_crate_roots_in` (tách khỏi `crate_roots`, thêm `src/bin/*.rs`); `_scope_rs_files`; `scan_lexical`/`scan_structure` nhận `scope`; `_crate_mod_walk`; `scan_structure_scope` (`E_STRUCT_MOD_MISSING`/`E_STRUCT_MOD_OUTSIDE`/`E_STRUCT_ORPHAN`/`E_STRUCT_NO_ROOT`); **+12 ca** selftest (V1..V12, 7 đối chứng dương) |
| `v2/agent/BYPASS-REGISTRY.md` | thêm **WB-24** (luật cấu trúc ngoài `V2_SCAN`) — CLOSED |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §1 (phạm vi cấu trúc), §3 (bốn mã `E_STRUCT_*`), §4 (bốn nợ §8) |
| `v2/agent/JOURNAL/kt264-struct-scope.md` | nhật ký này |
| `v2/agent/JOURNAL/kt264-attack.tsv` | bảng ma trận 9 biến thể × 2 lượt (trước/sau vá) |
| `v2/agent/JOURNAL/kt264-attack/**` | harness + 9 ca + log + `run-attack.sh` |

**Không** sửa: `v2/kernel/**`, `v2/sim/**`, `v2/lib/**`, `spec/`, `v2/bin/v2`,
`v2/agent/{STATE,QUEUE,DECISIONS}.md`.

---

## Phụ lục — T2 lần 1 đỏ ở `cắt điện giữa lúc ghi (KT-136)`: **flake đã biết (D2-151)**, không phải hồi quy

`gate --fast` trên `1f2d8310` (sau khi merge KT-264) đỏ **đúng một** bước:

```text
FAIL crash-host kt75-disk.img: get /pstate: 13 bản ghi (cần 11 hoặc 12 sau crash giữa pha 7)
```

Đây **chính xác** là flake **D2-151** đã ghi ngày 2026-11-19: ca này giết QEMU **theo dòng serial**
("ngay sau dòng verify pha 6"); khi host chậm (máy còn tải của ba sub-agent vừa xong), lệnh kill rơi
**sau** khi khách ghi xong pha 7 ⇒ `/pstate` 13 bản ghi thay vì 11–12. Bằng chứng trong log: `/p7.bin`
đủ `73728 B`, `partial=0`, `committed=1` — tức pha 7 **đã xong**, không phải mất toàn vẹn.

**Phân loại theo AGENTS §4.5** (chạy lại **một lần**, chỉ để phân loại): `bash v2/test/boot/persist.sh
--crash` trên máy **rảnh** ⇒ `# persist.sh: PASS`, `RC=0`. ⇒ **không phải hồi quy mã**; KT-264 chỉ
sửa `v2/tools/check-wake-paths.py` + journal, không chạm đường persist.

**Không** dùng lần đỏ này để kết luận (D2-151, D2-116). T2 bàn giao chạy lại trên máy rảnh.
