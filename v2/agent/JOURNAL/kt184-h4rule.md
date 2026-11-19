# KT-184 — D2-160 (H4) + D2-161 (luật cứng `include!`/`#[path]` ở mọi chế độ)

**Ngày**: 2026-11-19 (vòng hai) · **Agent 2 (thợ) L23** · nhánh `night2/L23-h4rule` ·
worktree `/private/tmp/night2-L23-h4rule` · HEAD `b6efc403` (từ `main`).
**Nguồn đọc trước**: `v2/agent/DECISIONS.md` D2-160/D2-161/D2-164,
[`kt177-wakefix4.md`](kt177-wakefix4.md), `v2/tools/check-wake-paths.py`.
**Ngân sách**: hạn cứng 12:00 JST. Không merge, không push, không `gate --fast`, không tạo
worktree mới, không QEMU.

## 0. Kết luận trước

| Yêu cầu | Trạng thái | Căn cứ (một dòng) |
|---|---|---|
| **A1** repro H4 **hai lớp** ⇒ ĐỎ | **ĐẠT** | `E_STRICT_TEST_ONLY_WAKE` @ `evil_b.rs:2`; log [`kt184-logs/h4-two-layer.log`](kt184-logs/h4-two-layer.log) |
| **A2** repro H4 **ba lớp** ⇒ ĐỎ | **ĐẠT** | `E_STRICT_TEST_ONLY_WAKE` @ `evil_c.rs:2`; log [`kt184-logs/h4-three-layer.log`](kt184-logs/h4-three-layer.log) |
| **A3** selftest thêm ca hai lớp + ba lớp | **ĐẠT** | **56 → 64** ca (8 ca mới); `# selftest: 64/64 ok` |
| **A4** checker cũ trên **cùng cây** ⇒ `errors=0` | **ĐẠT** (xem §6.1) | checker `7e7e1b53` (KT-174, **trước** fixpoint KT-177) ⇒ `errors=0` cho cả hai lớp |
| **B1** chuyển phân giải + phát lỗi ra **ngoài** `if strict:` | **ĐẠT** | `scan_v2` dòng ~1300; khối `--strict` chỉ dùng lại `extra` |
| **B2** thêm kiểm `#[path]` **không literal** | **ĐẠT** | `resolve_include_targets`; `E_STRICT_WAKE_INCLUDE` @ `core_sched.rs:1005` |
| **B3** ba đột biến **ĐỎ ở chế độ thường** | **ĐẠT** | log [`kt184-logs/d2161-normal.log`](kt184-logs/d2161-normal.log) — cả ba `errors=1` |
| **B4** cây hiện tại `errors=0` ở **cả ba** chế độ | **ĐẠT** | [`kt184-logs/checker-three-modes.log`](kt184-logs/checker-three-modes.log) + [`kt184-logs/v2-check.log`](kt184-logs/v2-check.log) |
| **B5** nháp một dòng cho `AGENTS.md` | **ĐẠT** | §4 dưới đây (KHÔNG sửa `AGENTS.md`) |
| **C** repro H6/H7 | **ĐẠT** — cả hai **ĐỎ** (đã đóng bởi KT-177) | [`kt184-logs/h6.log`](kt184-logs/h6.log), [`kt184-logs/h7.log`](kt184-logs/h7.log) |

Cây sạch: `python3 v2/tools/check-wake-paths.py` **`errors=0`**;
`--strict` **`errors=0`**; `--selftest` **64/64**; `./v2/bin/v2 check` **PASS**.
**Không** nới ngưỡng, không thêm allowlist, không `#[ignore]`/`#[should_panic]`, không retry.

## 1. Đã sửa gì

Toàn bộ thay đổi nằm trong `v2/tools/check-wake-paths.py` (chỉ **siết**).

### 1.1 D2-161 — luật cứng chạy ở **mọi** chế độ (`scan_v2`, dòng ~1297)

**Gốc lỗi**: `resolve_include_targets(...)` + vòng phát `E_STRICT_WAKE_INCLUDE` nằm **trong**
`if strict:` (dòng ~1542 bản `b6efc403`). `v2 check` **thường** và `gate` chạy checker **không**
`--strict`, nên ba đột biến của D2-161 đều **xanh** ở chế độ thường (xem §2.2).

**Vá**: khối phân giải + phát lỗi chuyển ra **ngoài** `if strict:`, ngay sau chỗ tính
`test_only` (cùng chỗ bản KT-174/H5 đã gọi `resolve_include_targets` để lấy
`_product_included`). Khối `--strict` **dùng lại** `extra` — không phân giải lần hai, nên
hai chế độ không thể lệch nhau. Vẫn giữ **hai lượt** phân giải khi `_product_included` khác
rỗng (lượt hai thấy nhiều tệp sản phẩm hơn ⇒ bắt được nhiều `include!`/`#[path]` không
literal hơn; đơn điệu, fail-closed) — đúng lượt mà khối `--strict` đã dùng trước KT-184.

Mã lỗi **giữ tên lịch sử** `E_STRICT_WAKE_INCLUDE` (KT-171) để không phá các ca selftest và
allowlist đã có; tên có chữ `STRICT` nay là **tên lịch sử**, không còn nghĩa "chỉ `--strict`"
— đã ghi rõ trong docstring đầu tệp.

### 1.2 D2-161 — `#[path]` **không literal** (`resolve_include_targets`, dòng ~783)

**Gốc lỗi**: bản KT-174/H3 chỉ bắt `include!` **không** literal. `#[path = concat!("evil",
".rs")]` không khớp `RE_PATH_ATTR_ANY` (đòi literal chuỗi) nên **không** vào `found` ⇒ không
phân giải, không báo lỗi, tệp đích **không** được quét. Đây là lỗ hổng thật: đột biến B3
dưới đây **lọt cả `--strict`** trước KT-184 (xem §2.3).

**Vá**: thêm vòng quét `#\[path\s*=` cho mọi `#[path]` **không** khớp literal, đối xứng với
vòng `include!` đã có, cùng điều kiện `if not is_test:` (tệp **sản phẩm**; tệp test-only vẫn
được kiểm "trỏ trong cây" như cũ).

### 1.3 D2-160 — H4: **không** đổi thuật toán, chỉ thêm ca

Fixpoint/worklist của KT-177/G4 đã đóng H4 (hai lớp). KT-184 **không** sửa
`resolve_include_targets` cho H4; chỉ thêm ca selftest **ba lớp** và một dạng **hai lớp**
khác (`#[path]` + `include!`) để khoá hành vi lan "sản phẩm thắng" qua nhiều lớp/cơ chế.

## 2. Bằng chứng (dán nguyên văn)

### 2.1 H4 hai lớp và ba lớp ⇒ ĐỎ; checker cũ ⇒ `errors=0`

Cây đột biến: bản sao `v2/kernel` + `v2/sim` + `v2/lib` trong thư mục tạm (không sửa cây
thật). Script: [`kt184-logs/repro-h4.sh`](kt184-logs/repro-h4.sh).

**Hai lớp** — `core_sched.rs` (sản phẩm) → `evil_a.rs` (test-only) → `evil_b.rs` (test-only,
`m.wake(t)`):

```
### Đối chứng: cây KHÔNG đột biến — checker hiện tại --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0

### H4 hai lớp — checker HIỆN TẠI (KT-184) --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:2: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### H4 hai lớp — checker b6efc403 (base worktree, ĐÃ có fixpoint KT-177) --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_b.rs:2: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### H4 hai lớp — checker 7e7e1b53 (KT-174, TRƯỚC fixpoint KT-177) --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

**Ba lớp** — thêm `evil_c.rs` (test-only, `m.wake(t)`), `evil_b.rs` chỉ còn
`include!("evil_c.rs")`:

```
### H4 ba lớp — checker HIỆN TẠI (KT-184) --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_c.rs:2: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### H4 ba lớp — checker b6efc403 (base worktree, ĐÃ có fixpoint KT-177) --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_c.rs:2: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### H4 ba lớp — checker 7e7e1b53 (KT-174, TRƯỚC fixpoint KT-177) --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

### 2.2 D2-161 — ba đột biến ở **chế độ thường** ⇒ ĐỎ

Script: [`kt184-logs/repro-d2161.sh`](kt184-logs/repro-d2161.sh). Mỗi đột biến thêm vào
**cuối** `v2/kernel/vifast-core/src/core_sched.rs` (tệp **sản phẩm**).

```
### B1 include!(concat!(…)) — checker HIỆN TẠI (KT-184), chế độ thường
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1005: `include!` không phải literal chuỗi — không phân giải được đích (fail-closed, KT-174 H3)
# check-wake-paths: mode=v2 strict=0 errors=1 warnings=0
rc=1

### B1 include!(concat!(…)) — checker b6efc403 (trước KT-184), chế độ thường
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0

### B2 #[path] ra ngoài cây — checker HIỆN TẠI (KT-184), chế độ thường
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1005: `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` (`../../../lib/vifast-abi/src/evil_out.rs`) — không quét được đích (fail-closed, KT-171 B6/B7)
# check-wake-paths: mode=v2 strict=0 errors=1 warnings=0
rc=1

### B2 #[path] ra ngoài cây — checker b6efc403 (trước KT-184), chế độ thường
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0

### B3 #[path = concat!(…)] — checker HIỆN TẠI (KT-184), chế độ thường
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1005: `#[path]` không phải literal chuỗi — không phân giải được đích (fail-closed, KT-184 D2-161)
# check-wake-paths: mode=v2 strict=0 errors=1 warnings=0
rc=1

### B3 #[path = concat!(…)] — checker b6efc403 (trước KT-184), chế độ thường
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0
```

### 2.3 D2-161 — B3 lọt **cả `--strict`** trước KT-184

Đối chứng phụ: cùng cây B3, checker `b6efc403` ở `--strict` **không** báo `#[path]` không
literal (chỉ bắt `wake` trong `evil_x.rs` vì tệp đó vẫn nằm trong `files`); checker KT-184
báo **cả hai**:

```
### B3 #[path = concat!(…)] — checker b6efc403, --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_x.rs:2: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### B3 #[path = concat!(…)] — checker HIỆN TẠI (KT-184), --strict
E E_STRICT_WAKE_INCLUDE   v2/kernel/vifast-core/src/core_sched.rs:1005: `#[path]` không phải literal chuỗi — không phân giải được đích (fail-closed, KT-184 D2-161)
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_x.rs:2: gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=2 warnings=0
rc=1
```

### 2.4 Cây hiện tại — ba chế độ

```
### python3 v2/tools/check-wake-paths.py (chế độ thường)
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
rc=0

### python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0

### python3 v2/tools/check-wake-paths.py --selftest
# selftest: 64/64 ok
rc=0
```

`./v2/bin/v2 check` (log đầy đủ [`kt184-logs/v2-check.log`](kt184-logs/v2-check.log)):

```
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
rc=0
```

### 2.5 Selftest — 8 ca mới, 56 → 64

```
ok - H4 ba lớp `include!` qua tệp test-only ⇒ đỏ (KT-184/D2-160)
ok - H4b ba lớp, đích chỉ `wake_from(` ⇒ xanh (KT-184)
ok - H4 `#[path]` (sản phẩm) + `include!` (test-only) hai lớp ⇒ đỏ (KT-184)
ok - H4b `#[path]` + `include!` hai lớp, đích chỉ `wake_from(` ⇒ xanh (KT-184)
ok - D2-161 `include!(concat!(…))` ở chế độ thường ⇒ đỏ (KT-184)
ok - D2-161 `#[path]` ra ngoài cây ở chế độ thường ⇒ đỏ (KT-184)
ok - D2-161 `#[path = concat!(…)]` ở chế độ thường ⇒ đỏ (KT-184)
ok - D2-161b `#[path]` literal trong cây ở chế độ thường ⇒ xanh (KT-184)
# selftest: 64/64 ok
```

Tám ca mới: **4 ca D2-160** (ba lớp + đối chứng dương; hai lớp dạng `#[path]`+`include!` +
đối chứng dương) và **4 ca D2-161** (ba đột biến ở chế độ thường + một đối chứng dương).
Ca **hai lớp kiểu ordering** (`core_ipc` → `evil_a` → `evil_b`, cả hai test-only) đã có từ
KT-177 là ca `4ak`/`4al` — KT-184 **không** nhân bản nó; ca hai lớp mới dùng **cơ chế khác**
(`#[path]` nạp tệp sản phẩm rồi `include!` tới tệp test-only) để phủ thêm một đường.

## 3. Phần C — repro H6/H7 (D2-160)

Script: [`kt184-logs/repro-h6h7.sh`](kt184-logs/repro-h6h7.sh). Cả hai dựng lại đúng đột
biến của reviewer L16 (KT-174 §2.6/§2.7) trên **bản sao** cây thật; KT-177 §2.3/§2.4 đã
kiểm hai đột biến này **biên dịch được** (`cargo check -p vifast-core` RC=0).

**H6** — `macro_rules! concat` trong `v2/kernel/vifast-core/src/evil_macro.rs` che macro
chuẩn; thân biến ident thành tên hàm (`$m.$name($t)`); `wake` là đối số macro + có biến cục
bộ `wake` cùng khối:

```
### H6 — checker HIỆN TẠI (KT-184) --strict
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-core/src/evil_macro.rs:10: `wake` là đối số của macro `concat!` — macro có thể biến ident thành tên hàm (KT-171 B10; tên `concat` bị `macro_rules!` trong cây che — KT-177 H6) trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/`wake_from_unlink`
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### H6 — checker 7e7e1b53 (KT-174, TRƯỚC KT-177) --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

**H7** — thân `macro_rules! evil_inc` trong `v2/lib/vifast-abi/src/lib.rs` chứa `include!`
(thân **không** có token `wake`):

```
### H7 — checker HIỆN TẠI (KT-184) --strict
E E_STRICT_MACRO_INCLUDE   v2/lib/vifast-abi/src/lib.rs:128: thân `macro_rules! evil_inc` chứa `include!` — đường dẫn giải theo **tệp gọi**, không phân giải tĩnh được (fail-closed, KT-177 H7)
# check-wake-paths: mode=v2 strict=1 errors=1 warnings=0
rc=1

### H7 — checker 7e7e1b53 (KT-174, TRƯỚC KT-177) --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
rc=0
```

⇒ **H6 và H7 đã đóng** (KT-177), repro dựng lại được và **ĐỎ** trên checker hiện tại.
**Không** còn nợ nào phải tuyên bố cho H6/H7 trong KT-184.

## 4. Nháp một dòng cho `AGENTS.md` (Agent 0 tự thêm — KT-184 **KHÔNG** sửa `AGENTS.md`)

Câu đề nghị, đúng nội dung D2-161 yêu cầu:

> Mọi `include!`/`#[path]` trong `v2/kernel` + `v2/sim` phải literal và trỏ trong cây; kiểm ở mọi chế độ.

Gợi ý chỗ dán: `AGENTS.md` §4 **Cấm**, thêm một mục (sau mục 10) — bản đầy đủ một dòng:

> 11. `include!`/`#[path]` trong `v2/kernel` + `v2/sim` phải **literal** và **trỏ trong cây**;
>     kiểm ở **mọi** chế độ (`v2 check` thường, `--strict`, `gate`) — `E_STRICT_WAKE_INCLUDE`.

(`v2/AGENTS.md` là bản rút gọn; nếu Agent 0 muốn một dòng ở đó thì cùng câu, ngắn hơn.)

## 5. Ghi chú / chỗ lệch so với đề bài

### 5.1 `b6efc403` **không phải** checker "cũ" cho H4

Đề bài ghi "checker cũ (`git show b6efc403:v2/tools/check-wake-paths.py`) chạy trên cùng cây
⇒ `errors=0`". **Kiểm lại thì `b6efc403` đã có fixpoint KT-177** (`eda9dbba` — commit KT-177
— nằm trong lịch sử của `b6efc403`), nên nó **ĐỎ** trên cây H4 hai lớp/ba lớp — xem §2.1.
Bản **thật sự trước** fixpoint là `7e7e1b53` (KT-174); bản này cho `errors=0` trên **cùng
cây**, đúng ý "chứng minh lỗ hổng là thật". Cả hai đều được chạy và ghi log để không ai phải
đoán.

### 5.2 Mã lỗi giữ tên `E_STRICT_WAKE_INCLUDE`

Luật nay chạy ở **mọi** chế độ nhưng mã lỗi vẫn mang chữ `STRICT` (tên lịch sử từ KT-171).
Đổi tên sẽ phá các ca selftest/allowlist đã có mà không thêm giá trị đo; đã ghi rõ trong
docstring. Nếu Agent 0 muốn đổi tên (`E_WAKE_INCLUDE`) thì đó là việc của một task riêng.

### 5.3 `#[path]` không literal chỉ bắt ở tệp **sản phẩm**

Đối xứng với `include!` không literal (KT-174/H3): điều kiện `if not is_test:`. Tệp
test-only vẫn bị kiểm **"trỏ trong cây"** (nhánh `_inside_scan_tree` không phụ thuộc
`is_test`). Nếu D2-161 muốn bắt **cả** tệp test-only thì cần một dòng quyết định nữa — ghi
ở đây để không âm thầm nới.

### 5.4 Không chạy `gate --fast` (theo đề), không QEMU, không push/merge

`./v2/bin/v2 check` **PASS**; `gate --fast` do lead chạy sau khi merge.

### 5.5 Review độc lập **chưa** có — D2-159

D2-159 đòi review của sub-agent **KHÁC tác giả** cho mọi task đóng một luật checker. KT-184
đóng **hai** luật (D2-160 H4, D2-161 `include!`/`#[path]`), nên **phải** có reviewer độc lập
dựng lại đột biến trước khi tuyên bố ĐẠT. Bản thân tệp này **không** tự tuyên bố "luật đã
đóng" thay reviewer — mọi dòng "ĐẠT" ở §0 là **kết quả đo của tác giả**, chờ review.

## 6. Tệp đã chạm

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | D2-161: chuyển phân giải + phát lỗi ra ngoài `if strict:`; thêm kiểm `#[path]` không literal; 8 ca selftest mới (56 → 64); docstring |
| `v2/agent/JOURNAL/kt184-h4rule.md` | tệp này |
| `v2/agent/JOURNAL/kt184-logs/repro-h4.sh` + `h4-two-layer.log` + `h4-three-layer.log` | repro H4 hai/ba lớp |
| `v2/agent/JOURNAL/kt184-logs/repro-d2161.sh` + `d2161-normal.log` + `d2161-strict.log` | repro D2-161 |
| `v2/agent/JOURNAL/kt184-logs/repro-h6h7.sh` + `h6.log` + `h7.log` | repro H6/H7 |
| `v2/agent/JOURNAL/kt184-logs/checker-three-modes.log` + `v2-check.log` | log ba chế độ + `v2 check` |

**Không** chạm: `spec/`, `AGENTS.md`, `docs/QUYET-DINH-sau-review.md`,
`docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`, `v2/kernel/`, `v2/sim/`, `v2/lib/`.
