# KT-226 — vá lớp 2 mù `src/target/` + đồng nhất allowlist producer (lớp 2)

**Vai**: L51 — tác giả bản vá (tiếp KT-222 reviewer cùng phiên).
**Nhánh**: `night3/L51-kt222`, gốc `a854dad6` (KT-222). **Worktree**: `/private/tmp/night3-L51-kt222`.
**Nguồn**: hai rủi ro reviewer L51 nêu ở `kt222-verify-final.md` §7 (rủi ro 1 và 2).
**Ràng buộc**: không nới ngưỡng, không allowlist để lấy xanh, không push, không `gate --fast`.

---

## 0. KẾT QUẢ

| Hạng mục | Trước | Sau |
|---|---|---|
| Lớp 2 với `src/target/evil.rs` (mutation, `cargo check` RC=0) | **XANH** `errors=0` | **ĐỎ** `errors=1` `E_DEPINFO_WAKE` |
| Đối chứng dương: tệp trong `v2/kernel/target/` | bị bỏ qua | **vẫn bị bỏ qua** (XANH) |
| Bất đối xứng allowlist producer | lớp 2 miễn **cả tệp** `fn wake`, lớp 1 chỉ miễn định nghĩa | **đồng nhất**: lớp 2 cũng chỉ miễn định nghĩa |
| `check-wake-depinfo.py --selftest` | 20/20 | **24/24** (+4 ca: 6b, 21, 22, 23) |
| `check-wake-paths.py --selftest` | 246/246 | **246/246** (không đổi) |
| Cây thật — lớp 1 thường / `--strict` | `errors=0` | **`errors=0`** |
| Cây thật — lớp 2 (2 unit) | `errors=0` | **`errors=0`** (145 tệp nạp, 145 tệp quét) |
| `./v2/bin/v2 check` | PASS | **PASS** |

---

## 1. VÁ 1 — lớp 2 mù `src/target/` (ưu tiên)

### 1.1 Gốc

`check-wake-depinfo.py` lọc tệp nạp bằng `"target" in relp.parts` ở **hai** chỗ (vòng lặp trong
`run_unit` và vòng lặp `--no-build`). Bộ lọc khớp **bất kỳ** thành phần `target` — kể cả
`src/target/` — nên tệp `v2/kernel/vifast-core/src/target/evil.rs` **không vào** `loaded`, không
bị quét. Đây đúng là đường lọt #12 **tái hiện ở lớp 2**; lớp 1 đã vá ở KT-215 (`_is_build_dir`)
nhưng lớp 2 **chưa** — reviewer L51 đo được ở KT-222 §3 (r04: L1 ĐỎ / L2 XANH).

### 1.2 Vá

- Thêm `_is_build_dir(root, p)` — **gọi thẳng** `CWP._is_build_dir` (import từ
  `check-wake-paths.py`, **không chép lần hai** — bài học đường lọt #13: hai lớp dùng chung hàm
  thì phải kiểm **cả hai**). Chuẩn hoá `root.resolve()`/`p.resolve()` trước khi so: `resolve_dep`
  trả đường dẫn đã resolve (macOS `/var` → `/private/var`) còn `root` có thể chưa — lệch gốc làm
  `relative_to` ném `ValueError` ⇒ bộ lọc **mất tác dụng** (đã đo: ca (23) FAIL trước khi thêm
  resolve).
- Tách vòng lặp đọc `.d` thành `_load_dep_files(...)` dùng chung cho `run_unit` và `--no-build`
  (trước đây **hai** vòng lặp chép tay cùng bộ lọc — sửa một chỗ là chỗ kia lệch).
- **Không** đổi ngữ nghĩa `_is_build_dir`: chỉ bỏ qua khi `target` đứng **trước** `src`; đường
  dẫn ngoài `root` ⇒ **không** bỏ qua (fail-closed).

### 1.3 Đo trước/sau (mutation `src/target/evil.rs`, `cargo check` RC=0)

Repro: `mod target;` trong `v2/kernel/vifast-core/src/lib.rs` + `src/target/mod.rs` (`mod evil;`)
+ `src/target/evil.rs` chứa `m.wake(t)`.

| Checker | Kết quả | Log |
|---|---|---|
| **Trước** (`git show HEAD:v2/tools/check-wake-depinfo.py`) | **XANH** `errors=0` (RC=0) | `kt226-logs/mut-src-target-layer2-before.log` |
| **Sau** (KT-226) | **ĐỎ** `errors=1` `E_DEPINFO_WAKE v2/kernel/vifast-core/src/target/evil.rs:6` (RC=1) | `kt226-logs/mut-src-target-layer2-after.log` |
| `cargo check -p vifast-core` | **RC=0** (mã biên dịch thật) | `kt226-logs/mut-src-target-cargo.log` |

### 1.4 Đối chứng dương

Tệp `v2/kernel/target/kt226_pos_build.rs` chứa `m.wake(t)` (thư mục build thật — `target` **trước**
`src`) ⇒ lớp 2 **XANH** `errors=0` (bị bỏ qua đúng). Log:
`kt226-logs/pos-kernel-target-layer2.log`.

### 1.5 Ca selftest (không cần cargo)

| Ca | Nội dung | Kỳ vọng |
|---|---|---|
| (21) | `_is_build_dir`: `src/target/` **không** bỏ qua; `v2/kernel/target/` bỏ qua; `src/clean.rs` không bỏ qua | đúng |
| (22) | `.d` nạp `src/target/evil.rs` (wake) ⇒ vào `loaded` ⇒ **ĐỎ** `E_DEPINFO_WAKE` (trước KT-226: XANH) | đúng |
| (23) | `.d` nạp `v2/kernel/target/evil.rs` (build dir) ⇒ **bị lọc** ⇒ XANH | đúng |

---

## 2. VÁ 2 — bất đối xứng allowlist producer

### 2.1 Đo **trước** khi chọn (a)

Tạm tắt miễn-cả-tệp (`is_producer_file` luôn `False`) rồi chạy lớp 2 trên **cây thật**:

| Unit | Kết quả | Log |
|---|---|---|
| host | **`errors=0`** (RC=0) | `kt226-logs/producer-a-host.log` |
| kernel | **`errors=0`** (RC=0) | `kt226-logs/producer-a-kernel.log` |

⇒ (a) **không** làm cây thật ĐỎ ⇒ theo phiếu lead, **chọn (a)**.

### 2.2 Chọn (a) — lớp 2 chỉ miễn **định nghĩa**, không miễn cả tệp

- Xoá nhánh `if is_producer_file(text): … continue` trong `scan`; xoá hàm `is_producer_file`.
- Tệp producer (`core_sched.rs`, `kobj.rs`) nay **được quét**; `_check_wake_text` miễn **định
  nghĩa** `fn wake` (và biến cục bộ/chú thích/chuỗi) — **cùng** ngữ nghĩa lớp 1.
- `counts["exempt_producer"]` giữ khoá (luôn 0) để JSON không đổi hình dạng; dòng báo cáo đổi
  thành `(miễn N tệp test-only; KT-226: không miễn cả tệp cửa wake)`.
- Cập nhật docstring module (mục "Miễn") + `CHECKER-WAKE-CONTRACT.md` §4 (một dòng KT-226).

### 2.3 Ca selftest

| Ca | Nội dung | Kỳ vọng |
|---|---|---|
| (5) | cây sạch ⇒ XANH, `scanned=3`, `exempt_producer=0` (trước: `scanned=2`, `exempt_producer=1`) | đúng |
| (6) | tệp định nghĩa `fn wake` được **quét** nhưng định nghĩa được miễn ⇒ XANH | đúng |
| (6b) | tệp producer **gọi** `wake(` ⇒ **ĐỎ** (trước KT-226: XANH ở lớp 2, ĐỎ ở lớp 1) | đúng |

---

## 3. LOG GATE — ghi nhận cách ghi đúng

Reviewer L51 (KT-222 §1.1) nêu: log `gate --fast` trên `be3a0cd8` nằm ở commit `83275d94` (commit
kế tiếp), không nằm trong `be3a0cd8`. Lead đã ghi rõ trong `STATE.md` và `kt221-rules-segv.md`.
**Đây là cách ghi đúng**: log của cổng chạy trên commit X **có thể** nằm ở commit X+1, **miễn**
ghi rõ "log ở commit kế tiếp" (và log tự khai bất biến `main = X` trước/sau). Không cần sửa gì.

---

## 4. KHÔNG HỒI QUY

| Kiểm | Kết quả | Log |
|---|---|---|
| `check-wake-paths.py --selftest` | **246/246 ok** | `kt226-logs/layer1-selftest.log` |
| `check-wake-paths.py` (cây thật) | `errors=0` | `kt226-logs/layer1-tree.log` |
| `check-wake-paths.py --strict` (cây thật) | `errors=0` | `kt226-logs/layer1-tree-strict.log` |
| `check-wake-depinfo.py --selftest` | **24/24 ok** | `kt226-logs/layer2-selftest.log` |
| `check-wake-depinfo.py` (cây thật, 2 unit) | `errors=0` — 145 tệp nạp, **145 tệp quét** (trước: 143 quét + 2 miễn) | `kt226-logs/real-tree-layer2-all.log` |
| `check-wake-depinfo.py --no-build` | `errors=0` (đường `--no-build` dùng chung `_load_dep_files`) | `kt226-logs/real-tree-layer2-nobuild.log` |
| `./v2/bin/v2 check` | **PASS** | `kt226-logs/kt226-v2check.log` |

**Không** nới ngưỡng, **không** thêm allowlist, **không** sửa `check-wake-paths.py`/`v2/bin/v2`.

---

## 5. TỆP + LOG

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-depinfo.py` | `_is_build_dir` (import `CWP._is_build_dir`), `_load_dep_files` dùng chung, bỏ miễn-cả-tệp producer, 4 ca selftest mới |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §4: một dòng KT-226 |
| `v2/agent/JOURNAL/kt226-layer2-target.md` | tệp này |
| `v2/agent/JOURNAL/kt226-logs/` | log trước/sau, đối chứng, selftest, cây thật, `v2 check` |
