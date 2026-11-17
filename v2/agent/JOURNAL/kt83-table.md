# KT-83 — Bảng bảy lỗi v1 (`KNOWN-ISSUES`) với `file:dòng`

**Task**: KT-83 (Agent 2 dòng OS) · **Ngày**: 2026-11-17 · **Ngân sách**: 2 h
**Phạm vi ghi**: chỉ `v2/agent/JOURNAL/kt83-*`. Không sửa mã, không commit, không `git add`.
**Nguồn đã đọc**: `docs/v1/KNOWN-ISSUES.md` (đọc-only), `docs/v1/decisions/log.md`,
`docs/v1/overnight-log.md`, `docs/v1/agent2-report-34.md`, `docs/v1/agent2-report-35.md`,
`docs/v1/agent2-report-89.md`, `v2/agent/DECISIONS.md` (D2-10, D2-118, D2-119, D2-120),
`v2/agent/reports/K6-DOD.md` §4, `v2/agent/reports/K6-CLOSURE-DRAFT.md` §3,
`v2/agent/reports/QUYET-DINH-A0-K6-2026-11-17.md` (OD-K6-4/OD-K6-7).

**Quy ước**: mọi số dưới đây trích từ `file:dòng` của log/tài liệu — **không** có số viết tay.
`docs/v1/` là **đọc-only**; nguồn gốc nằm ngoài `docs/v1` (dossier) được ghi rõ là **ngoài** `docs/v1`.

## 0. Số chốt (D2-119 + D2-120)

**2/7 đóng** (N4, N6) · **1/7 hoãn K5′** (N5) · **4/7 mở** (N1, N2, N3, N7). **K7 không nhận N5.**

- `v2/agent/DECISIONS.md:209` (D2-119, đính chính của lead): “**2/7 đóng · 1/7 hoãn K5' · 4/7 mở**”.
- `v2/agent/DECISIONS.md:211` (D2-120 A.0/A.2): “**2/7** (N4, N6), **1/7 hoãn K5'** (N5), **4/7 mở**
  (N1/N2/N3/N7)”; “**K7 không nhận N5**”.
- `v2/agent/reports/K6-CLOSURE-DRAFT.md:54-63` (§3) dùng đúng ba trạng thái này.

## 1. Bảng bảy dòng

| mã | mô tả | trạng thái | bằng chứng v1 | ca kiểm v2 | log chứng minh | ghi chú |
|---|---|---|---|---|---|---|
| **N1** | **T33a** — treo `x86_64` sau `shell>` (~0,3–0,5 % boot; worker C `TIME MONOTONIC BAD` → `panic!` → root chờ mãi) | **MỞ** | `docs/v1/decisions/log.md:97` (r294.A: “6 treo sau `shell>` (T33a)”); `docs/v1/KNOWN-ISSUES.md:10` | `v2/sim/vifast-sim/tests/kt62_syscall.rs:862-871` — `ClockRead` đơn điệu **một hart** (`rig.t0`), **không migrate**. **Chưa có ca kiểm “đơn điệu sau migrate” + conformance chéo hart** | `v2/agent/JOURNAL/kt76-baseline-gate.log:1096` — `test kt62_waitaddr_wakeaddr_clock_put_halt ... ok` | Thiếu **migrate** + **chéo hart**; D2-10 chỉ hoãn **phần cứng x86**, **không** miễn ca kiểm sim (`v2/agent/DECISIONS.md:207` D2-118 B) |
| **N2** | **T26** — FS vi phạm read-your-own-write ở `MAX_IO=256` (10/50 = 20 % boot đỏ) | **MỞ** | `docs/v1/decisions/log.md:91` (r293.1: “12/49 hỏng ở MAX_IO=256”); `docs/v1/KNOWN-ISSUES.md:11` | `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs:179` (`ryow_1000_seeds`), `:188` (`ryow_across_remount_1000_seeds`), `:217` (`harness_catches_silent_write_loss` — **ca âm**) | `v2/agent/JOURNAL/kt76-baseline-gate.log:249-251` — 3 test `... ok` | Ca kiểm v2 **đủ theo cột 5** nhưng **không tái hiện** vi phạm (v2 ghi qua `commit()`+`flush()` ngay) và **không có phép đo 50 boot / 0 đỏ** — `v2/docs/kt-t26-analysis.md:48-50` (§5 kết luận trung thực), `:57-60` (§6 việc chưa làm) |
| **N3** | **T31** — PAGER2 nhận message lặp (`PAGER EVICT OK` in hai lần) | **MỞ** | `docs/v1/KNOWN-ISSUES.md:12`; `docs/v1/overnight-log.md:978-980` (marker `PAGER EVICT OK` exact-one, fault 6→7); `docs/v1/agent2-report-34.md:9-14` | `v2/kernel/vifast-kbin-riscv64/src/ktest_vfs.rs:9` (TAP 33, `clients=2`); `v2/lib/vifast-vfs/tests/pager.rs` (host test, **không** có client). **Chưa có** checker “giao đúng một lần”, **chưa có** ca 4 client, **chưa có** đột biến “phát lại Call khi Resume” | `v2/agent/JOURNAL/kt70-green-qemu-ktest-user.log:230-231` (stats `clients=2`), `:240` (`ok 33 - pager hai client chia se mot trang`) | Thiếu **đúng ba** thứ cột 5 đòi; pager chỉ đối chiếu `faults` với `served + rejected` |
| **N4** | **T-IPC** — “boot đỏ” r307 thực chất là cổng `test-exit` thoát đúng lúc client đang chạy user; r308 vá ⇒ 0/80 boot thiếu marker | **ĐÓNG** | `docs/v1/KNOWN-ISSUES.md:13` (nguồn gốc: `review/src/ViFastOs-dossier.md:1707` §15.4 — **ngoài** `docs/v1`) | `v2/test/boot/selftest.sh:1-20` (18 ca — log `18/18`; 16 ca đỏ phá đúng một bất biến, phải đỏ **kèm đúng token**); `v2/test/boot/guard_selftest.sh:1-15` (ca âm cho QEMU guard) | `v2/agent/JOURNAL/kt76-selftest.log:35` (`# selftest: 18/18 ca đúng như kỳ vọng`), `:36` (`# selftest.sh: PASS`); `v2/agent/JOURNAL/kt78-guard/selftest-20261117T015708Z.log:39` (`# guard_selftest: PASS`) | Luật “mọi harness mới phải có ca âm” đã hiện thực; **đóng** |
| **N5** | **x86 AP→BSP + PCID** — TLB cũ sau unmap ⇒ tiến trình đọc được trang đã thu hồi | **HOÃN K5′** | `docs/v1/KNOWN-ISSUES.md:14` (nguồn gốc: `review/src/ViFastOs-dossier.md:1725` §15.6 — **ngoài** `docs/v1`); bối cảnh ASID/PCID: `docs/v1/overnight-log.md:459-469` | Đột biến “bỏ xử lý hộp thư khi chờ ack” **có** trong sim: `v2/sim/vifast-sim/src/kobj.rs:2940`, `:2955`, `:3014`; test `v2/sim/vifast-sim/tests/kt64_core_tlb_bind_resume.rs:209` (`kt64_tlb_ack_khong_toi_tra_busy_khong_treo`). Conformance TLB **x86**: **chưa có** (hoãn K5′) | `v2/agent/JOURNAL/kt76-baseline-gate.log:1146` — `test kt64_tlb_ack_khong_toi_tra_busy_khong_treo ... ok` | Lý do hoãn: D2-119 (OD-K6-7: v2.0 **chỉ riscv64**; x86_64 thuộc v2.1+), D2-120(A.1) (N5 thuộc K5′ x86_64, **ngoài** phạm vi v2.0), D2-120(A.2) (**K7 không nhận N5**), D2-10 (chỉ riscv64 tới K6). Lỗi **đúng đắn** trên x86 ⇒ chỉ đóng được khi có x86 |
| **N6** | **Audit P1-5** — `enqueue` trả sớm ⇒ TCB `Ready` nhưng **không** trong runqueue | **ĐÓNG** | `docs/v1/KNOWN-ISSUES.md:15`; `docs/v1/agent2-report-89.md:28-33` (“xét lại, **không phải lỗi thực**”); `docs/v1/overnight-log.md:1472`; `docs/v1/overnight-log.md:3210-3212` (mutation test: TCB `Ready` không trong runqueue ⇒ `Err(IllegalOperation)`) | `v2/sim/vifast-sim/src/kobj.rs:7105` (`check_sched`), `:7188` (hai chiều “Ready ⇔ trong hàng; Running ⇔ current”); gọi sau **mỗi** op công khai ở `v2/sim/vifast-sim/src/kobj.rs:6159` (`sched_after_op` → `check_sched`); test `v2/sim/vifast-sim/tests/kt19_sched.rs:320` (`check_sched_sau_moi_op_cong_khai`) | `v2/agent/JOURNAL/kt76-baseline-gate.log:668` — `test check_sched_sau_moi_op_cong_khai ... ok` | Invariant **hai chiều** + test sau mỗi op; **đóng**. Lưu ý: v1 **mâu thuẫn nội bộ** — `KNOWN-ISSUES.md:15` ghi “Audit ghi **OPEN**”, `agent2-report-89.md:28` ghi “xét lại, không phải lỗi thực”; v2 đóng bằng invariant, **không** dựa vào kết luận v1 |
| **N7** | **Audit Q1-A** — kênh wake lệch 3 port (riscv doorbell riêng; arm/x86 mailbox chung) | **MỞ** | `docs/v1/KNOWN-ISSUES.md:16`; `docs/v1/decisions/log.md:85` (r289: “riscv64 CHƯA nối hộp thư dùng chung”); nguồn gốc: `review/src/ViFastOs-dossier.md:1750` §15.8 — **ngoài** `docs/v1` | `v2/lib/vifast-sync/tests/kt30_sync.rs` (10 test: mailbox + MCS), `v2/lib/vifast-sync/tests/kt30_stress.rs` (3 test) — **chưa có ca kiểm “một cơ chế wake chung / 3 port”** | `v2/agent/JOURNAL/kt76-baseline-gate.log:1204-1215` — 10 test `... ok` | Crate **có**, test “3 port” **chưa có**; D2-10 chỉ hoãn **phần cứng** x86, **không** hoãn test sim (`v2/agent/DECISIONS.md:207` D2-118 B) |

## 2. Chi tiết từng mục

### N1 — T33a (MỞ)

- **Mô tả**: treo `x86_64` sau `shell>`, không in `PREEMPT OK`; gốc đã lộ một nửa: worker C thấy
  `TIME MONOTONIC BAD` → `panic!` → root chờ mãi (`docs/v1/KNOWN-ISSUES.md:10`).
- **v1**: `docs/v1/decisions/log.md:97` — r294.A: “Lớp còn lại: **6 treo sau `shell>` (T33a)** +
  1 chết trước kernel (T-BOOT) = 2,3 %”. Nguồn gốc chi tiết: `review/src/ViFastOs-dossier.md:1612`
  (§15.1, **ngoài** `docs/v1`).
- **v2**: `v2/sim/vifast-sim/tests/kt62_syscall.rs:862-871` — chỉ kiểm `ClockRead` đơn điệu **một
  hart** (`rig.t0`), hook tăng 10 ns mỗi lần, `assert!(b > a, ...)` ở `:871`. **Không** có
  `migrate` giữa hai lần đọc, **không** có conformance chéo hart.
- **Log**: `v2/agent/JOURNAL/kt76-baseline-gate.log:1096` — `test kt62_waitaddr_wakeaddr_clock_put_halt ... ok`.
- **Vì sao chưa đóng**: **thiếu ca kiểm** đúng cột 5 (“đơn điệu **sau migrate** + conformance chéo
  hart”). D2-10 chỉ hoãn **phần cứng x86**, không miễn ca kiểm sim — `v2/agent/DECISIONS.md:207`
  (D2-118 B: “D2-10 chỉ hoãn **phần cứng x86**, **không** miễn ca kiểm sim”).
- **Kết luận**: **MỞ** — thiếu ca kiểm (một phần).

### N2 — T26 (MỞ)

- **Mô tả**: ở `MAX_IO=256`, `smp2`: 10/50 = 20 % boot đỏ; entry thư mục ghi xong, liệt kê ngay
  sau không thấy, lần sau thấy (`docs/v1/KNOWN-ISSUES.md:11`).
- **v1**: `docs/v1/decisions/log.md:91` — r293.1: “**12/49 hỏng ở MAX_IO=256** vẫn ra xanh nhờ
  retry”. Nguồn gốc chi tiết: `review/src/ViFastOs-dossier.md:1675` (§15.2, **ngoài** `docs/v1`).
- **v2**: `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` — ba lớp đo:
  `:179` `ryow_1000_seeds` (1.000 seed, có kẻ thứ ba ghi xen + xoá cache, `lost_writes() == 0`),
  `:188` `ryow_across_remount_1000_seeds` (`fsync` → mount lại từ **ảnh RAW**),
  `:217` `harness_catches_silent_write_loss` (**ca âm**: thiết bị mất ghi im lặng ⇒ phép đo phải đỏ).
  `MAX_IO = 256` cục bộ + `const _: () = assert!(MAX_IO == 256);` (`:21-24`).
- **Log**: `v2/agent/JOURNAL/kt76-baseline-gate.log:249-251` — `harness_catches_silent_write_loss ... ok`,
  `ryow_1000_seeds ... ok`, `ryow_across_remount_1000_seeds ... ok`.
- **Vì sao chưa đóng**: ca kiểm v2 **đủ theo cột 5**, nhưng **không tái hiện** vi phạm (v2 ghi qua
  `commit()` + `flush()` ngay) và **không có phép đo 50 boot / 0 đỏ** — chính tài liệu của task nói:
  “Bản port v2 **không** tái hiện T26 ở 1.000 seed … Điều đó **chưa** chứng minh v1 hết lỗi”
  (`v2/docs/kt-t26-analysis.md:48-50`), và “Chưa chạy trên sim/QEMU (`-smp 2`, 50 boot)”
  (`v2/docs/kt-t26-analysis.md:57-60`).
- **Kết luận**: **MỞ** — thiếu **phép đo** (0/50 boot), không phải thiếu ca kiểm.

### N3 — T31 (MỞ)

- **Mô tả**: `PAGER EVICT OK` in **hai lần** ⇒ nhánh xử lý chạy ≥ 3 lần ⇒ có message bị giao/xử lý
  lặp; r309 đã thêm bảng `pager2_step(seen)` + marker `PAGER2 DUP`, **chưa** tìm đường giao lặp
  (`docs/v1/KNOWN-ISSUES.md:12`).
- **v1**: `docs/v1/overnight-log.md:978-980` — “Marker `PAGER EVICT OK` exact-one, `PAGER EVICT BAD`
  forbidden. Fault 6→7 (`PAGER2_VA` ×2)”; `docs/v1/agent2-report-34.md:9-14` — “`PAGER EVICT OK`
  (exact-one) … Fault count 6 → 7 (`PAGER2_VA=0x1001001000` xuất hiện 2 lần); harness kiểm
  `faults.count(0x1001001000) == 2`”. Nguồn gốc: `review/src/ViFastOs-dossier.md:1694` (§15.3,
  **ngoài** `docs/v1`).
- **v2**: `v2/kernel/vifast-kbin-riscv64/src/ktest_vfs.rs:9` — TAP 33 “pager hai client chia se mot
  trang”, `clients=2`; `v2/lib/vifast-vfs/tests/pager.rs` — 12 test host (hit/miss/LRU/backend/stats),
  **không** có khái niệm client. **Chưa có** checker “giao **đúng một lần**”, **chưa có** ca
  `clients=4`, **chưa có** đột biến “phát lại Call khi Resume”.
- **Log**: `v2/agent/JOURNAL/kt70-green-qemu-ktest-user.log:230-231` — `# pager: cache … const:clients=2 …`
  và `# pager: map p2_a=… p2_b=… p2_same=1 …`; `:240` — `ok 33 - pager hai client chia se mot trang`.
- **Vì sao chưa đóng**: **thiếu ca kiểm** đúng cột 5 (4 client + checker exactly-once + đột biến).
  Pager v2 chỉ đối chiếu `faults` với `served + rejected` (`v2/agent/reports/K6-DOD.md:172`).
- **Kết luận**: **MỞ** — thiếu ca kiểm (một phần).

### N4 — T-IPC (ĐÓNG)

- **Mô tả**: “Boot đỏ” r307 thực chất là cổng `test-exit` thoát đúng lúc client đang chạy user;
  r308 vá: hoãn exit một tick khi task còn `Running` ⇒ 0/80 boot thiếu marker
  (`docs/v1/KNOWN-ISSUES.md:13`).
- **v1**: `docs/v1/KNOWN-ISSUES.md:13` — trạng thái nguồn “**Đã đóng ở v1**; bài học **mang sang v2**
  dưới dạng luật harness”. Nguồn gốc chi tiết: `review/src/ViFastOs-dossier.md:1707` (§15.4,
  **ngoài** `docs/v1`). **Không** tìm thấy báo cáo v1 gốc (r307/r308) trong `docs/v1/` — xem §4.
- **v2 (ca kiểm chứng minh đã đóng)**: luật “mọi harness mới phải có **ca âm**” hiện thực ở
  `v2/test/boot/selftest.sh:1-20` — 18 ca quanh một log thật (log `18/18`): 1 ca xanh, 16 ca đỏ
  (mỗi ca phá **đúng một** bất biến và phải đỏ **kèm đúng token lý do**), 1 ca kiểm `expected.txt`
  nguyên vẹn;
  và `v2/test/boot/guard_selftest.sh:1-15` — ca âm cho harness QEMU (giết cha, boot treo).
- **Log**: `v2/agent/JOURNAL/kt76-selftest.log:35` — `# selftest: 18/18 ca đúng như kỳ vọng`;
  `:36` — `# selftest.sh: PASS`; `v2/agent/JOURNAL/kt78-guard/selftest-20261117T015708Z.log:39` —
  `# guard_selftest: PASS`.
- **Kết luận**: **ĐÓNG** — có ca kiểm + log xanh.

### N5 — x86 AP→BSP + PCID (HOÃN K5′)

- **Mô tả**: lỗi **đúng đắn**: sau unmap, TLB của BSP có thể còn cũ ⇒ tiến trình đọc được trang đã
  thu hồi; không lộ trên TCG mặc định nên mọi cổng vẫn xanh (`docs/v1/KNOWN-ISSUES.md:14`).
- **v1**: `docs/v1/KNOWN-ISSUES.md:14` — trạng thái nguồn “**Mang sang v2**, mở ở mốc K5' (x86_64)”.
  Bối cảnh ASID/PCID: `docs/v1/overnight-log.md:459-469` — “x86_64: bật CR4.PCIDE khi CPUID leaf1
  ECX bit17 … QEMU TCG hiện không expose PCID nên đường QEMU dùng CR3 fallback”. Nguồn gốc chi tiết:
  `review/src/ViFastOs-dossier.md:1725` (§15.6, **ngoài** `docs/v1`).
- **v2**: phần **sim** của cột 5 đã có — đột biến “bỏ xử lý hộp thư khi chờ ack” được ghi rõ trong
  `v2/sim/vifast-sim/src/kobj.rs:2940` (“mutation ‘bỏ xử lý hộp thư khi chờ ack’ bị bắt”), `:2955`
  (“Đột biến … xoá đúng dòng trong `tlb_wait_round`”), `:3014` (“xoá đúng `drain_mail_fifo` ở đây”);
  test `v2/sim/vifast-sim/tests/kt64_core_tlb_bind_resume.rs:209` (`kt64_tlb_ack_khong_toi_tra_busy_khong_treo`).
  Phần **conformance TLB x86**: **chưa có** — đúng theo quyết định hoãn.
- **Log**: `v2/agent/JOURNAL/kt76-baseline-gate.log:1146` — `test kt64_tlb_ack_khong_toi_tra_busy_khong_treo ... ok`.
- **Lý do hoãn** (tra `v2/agent/DECISIONS.md`):
  - `:209` (D2-119, OD-K6-7): v2.0 **chỉ riscv64 trên QEMU**; “x86_64/AArch64/phần cứng thật/…
    thuộc **v2.1+**”.
  - `:211` (D2-120 A.1): “**N5 thuộc K5' x86_64, ngoài phạm vi v2.0**”.
  - `:211` (D2-120 A.2): “**K7 không nhận N5**”.
  - `:39` (D2-10): “chỉ riscv64 tới K6”.
- **Kết luận**: **HOÃN K5′** — không phải “đã đóng”, không tính vào tử số 2/7.

### N6 — Audit P1-5 (ĐÓNG)

- **Mô tả**: `enqueue` trả sớm ⇒ TCB mang trạng thái `Ready` nhưng **không** nằm trong runqueue
  (liên quan K37–K40); audit ghi **OPEN** (`docs/v1/KNOWN-ISSUES.md:15`).
- **v1**: `docs/v1/KNOWN-ISSUES.md:15`; `docs/v1/agent2-report-89.md:28-33` — “`enqueue` bỏ rơi TCB
  — **xét lại, không phải lỗi thực** … mỗi runqueue chứa `MAX_TCBS = 18` = tổng số TCB, và `queued`
  chặn xếp trùng, nên mọi TCB luôn tìm được hàng”; `docs/v1/overnight-log.md:1472` (cùng kết luận);
  `docs/v1/overnight-log.md:3210-3212` — mutation test: “TCB `Ready` **không** nằm trong runqueue ⇒
  `Err(IllegalOperation)`”. Nguồn gốc: `review/src/ViFastOs-dossier.md:1751` (§15.8, **ngoài** `docs/v1`).
- **v2 (ca kiểm chứng minh đã đóng)**: `v2/sim/vifast-sim/src/kobj.rs:7105` — `pub fn check_sched(&self)`;
  `:7188` — “Mọi TCB sống: **Ready ⇔ trong hàng** (đúng hart home); **Running ⇔ current**; còn lại
  **không** được nằm trong hàng”; gọi sau **mỗi** op công khai ở `v2/sim/vifast-sim/src/kobj.rs:6159`
  (`sched_after_op` → `check_sched`, `debug_assert!` khi vỡ); test
  `v2/sim/vifast-sim/tests/kt19_sched.rs:320` (`check_sched_sau_moi_op_cong_khai`).
- **Log**: `v2/agent/JOURNAL/kt76-baseline-gate.log:668` — `test check_sched_sau_moi_op_cong_khai ... ok`.
- **Kết luận**: **ĐÓNG** — có ca kiểm + log xanh. Ghi chú: v1 **mâu thuẫn nội bộ** (KNOWN-ISSUES ghi
  OPEN, report-89 ghi “không phải lỗi thực”); v2 đóng bằng **invariant**, không dựa vào kết luận v1.

### N7 — Audit Q1-A (MỞ)

- **Mô tả**: trong `Mailbox`, kênh đánh thức chưa thống nhất 3 port: riscv64 có doorbell riêng cho
  reschedule, arm/x86 dùng mailbox chung; nợ từ r293 (`docs/v1/KNOWN-ISSUES.md:16`).
- **v1**: `docs/v1/KNOWN-ISSUES.md:16`; `docs/v1/decisions/log.md:85` — r289: “riscv64 **CHƯA** nối
  hộp thư dùng chung: nối vào boot hỏng 0/5, revert thì 3/3”. Nguồn gốc: `review/src/ViFastOs-dossier.md:1750`
  (§15.8, **ngoài** `docs/v1`).
- **v2**: `v2/lib/vifast-sync/tests/kt30_sync.rs` — 10 test (MCS lock + mailbox FIFO/backpressure/
  generation/ack); `v2/lib/vifast-sync/tests/kt30_stress.rs` — 3 test stress. **Chưa có** ca kiểm
  “**một cơ chế wake chung** / 3 port”.
- **Log**: `v2/agent/JOURNAL/kt76-baseline-gate.log:1204-1215` — 10 test `... ok`.
- **Vì sao chưa đóng**: **thiếu ca kiểm** đúng cột 5 (“`vifast-sync` dùng chung cho mọi arch; ca
  kiểm chạy ở sim + riscv64 trước”). D2-10 chỉ hoãn **phần cứng** x86, **không** hoãn test sim —
  `v2/agent/DECISIONS.md:207` (D2-118 B).
- **Kết luận**: **MỞ** — thiếu ca kiểm.

## 3. Đếm bằng chứng

| Hạng mục | Số | Mục |
|---|---|---|
| Bằng chứng v1 (`docs/v1/…:dòng`) | **7/7** | N1..N7 |
| Ca kiểm v2 tương ứng (có, dù đủ hay một phần) | **6/7** | N1, N2, N3, N4, N5, N6 |
| — trong đó **đầy đủ theo cột 5** | **3/7** | N2, N4, N6 |
| — trong đó **một phần** | **3/7** | N1, N3, N5 |
| **Chưa có ca kiểm** đúng tiêu chí | **1/7** | N7 |
| Log xanh cho ca kiểm hiện có | **7/7** | N1..N7 |
| — trong đó log chứng minh **tiêu chí đóng** | **2/7** | N4, N6 |
| Thiếu hoàn toàn bằng chứng (cả v1 lẫn v2) | **0/7** | — |

**Đọc bảng này thế nào**: “đầy đủ theo cột 5” **không** đồng nghĩa “đã đóng”. N2 có đủ ca kiểm v2
nhưng vẫn **mở** vì thiếu phép đo 0/50 boot; N1/N3/N5 có ca kiểm **một phần**; N7 **chưa có ca kiểm**
đúng tiêu chí. Chỉ N4 và N6 có **cả** ca kiểm đúng tiêu chí **và** log xanh ⇒ 2/7 đóng, khớp D2-119.

## 4. KHÔNG TÌM THẤY (và đã tìm ở đâu)

1. **Báo cáo v1 gốc của N4 (r307/r308)** trong `docs/v1/` — **KHÔNG TÌM THẤY**. Chỉ có
   `docs/v1/KNOWN-ISSUES.md:13`; nguồn gốc thật ở `review/src/ViFastOs-dossier.md:1707` (§15.4),
   **ngoài** `docs/v1`. Đã tìm: `grep -rn "r307\|r308\|T-IPC" docs/v1/` (chỉ 1 match: KNOWN-ISSUES.md:13).
2. **Báo cáo v1 gốc của N5 (x86 AP→BSP + PCID)** trong `docs/v1/` — **KHÔNG TÌM THẤY**. Chỉ có
   `docs/v1/KNOWN-ISSUES.md:14`; nguồn gốc ở `review/src/ViFastOs-dossier.md:1725` (§15.6),
   **ngoài** `docs/v1`. Đã tìm: `grep -rn "TLB cũ\|AP→BSP\|AP->BSP" docs/v1/` (chỉ 1 match:
   KNOWN-ISSUES.md:14); các match `PCID` khác là mô tả **cài đặt** ASID/PCID, không phải lỗi.
3. **Ca kiểm v2 “đơn điệu sau migrate” (N1)** — **KHÔNG TÌM THẤY**. Đã tìm:
   `grep -rn "ClockRead" v2/sim/ --include=*.rs` (chỉ `kt62_syscall.rs`) và
   `grep -n "migrate" v2/sim/vifast-sim/tests/kt62_syscall.rs` (**rỗng**). `kt29_smp.rs` có migrate
   nhưng **không** clock.
4. **Ca kiểm v2 “pager 4 client + giao đúng một lần + đột biến phát lại Call khi Resume” (N3)** —
   **KHÔNG TÌM THẤY**. Đã tìm: `grep -rn "clients=4\|clients = 4" v2/` (**rỗng**);
   `grep -rn "pager" v2/ | grep "đúng một lần"` (**rỗng**); `grep -rn "phát lại Call" v2/` chỉ thấy
   khẳng định “**KHÔNG** phát lại Call” (`kt18_tcb.rs:640`, `kobj.rs:5696`) — invariant có, nhưng
   **không** có đột biến cố tình phát lại để chứng minh checker bắt được. Pager chỉ có `clients=2`
   (`v2/kernel/vifast-kbin-riscv64/src/ktest_vfs.rs:9`).
5. **Ca kiểm v2 “một cơ chế wake chung / 3 port” (N7)** — **KHÔNG TÌM THẤY**. Đã tìm:
   `grep -rn "3 port\|ba port\|mọi arch" v2/lib/vifast-sync/ v2/docs/` (**rỗng**).
6. **Phép đo v2 “50 boot / 0 đỏ” cho N2** — **KHÔNG TÌM THẤY**. Đã tìm:
   `grep -rn "boot_red" v2/` (chỉ match câu **mô tả tiêu chí** ở `K6-DOD.md:171`, không phải phép đo);
   `grep -rn "\b0/50 boot" v2/` (**rỗng** — các match `50/50` là boot **sạch**, không phải “0/50 đỏ”).
   Chính `v2/docs/kt-t26-analysis.md:57-60` ghi “Chưa chạy trên sim/QEMU (`-smp 2`, 50 boot)”.
7. **Log chứng minh N5 phần x86** — **KHÔNG TÌM THẤY** (đúng theo quyết định hoãn: v2.0 không có
   x86). Đã tìm: `grep -rn "PCID\|x86" v2/sim/ v2/kernel/ --include=*.rs` (không có test TLB x86).

## 5. Chỗ `K6-DOD.md` §4 cần sửa (đính chính nhỏ, không đổi kết luận)

- `v2/agent/reports/K6-DOD.md:171` dẫn `kt76-baseline-gate.log:250,251,252` — dòng **252 là dòng
  trống**; ba test nằm ở **249-251** (`harness_catches_silent_write_loss` ở 249). Đề nghị sửa thành
  `:249-251`.
- `v2/agent/reports/K6-DOD.md:176` dẫn `kt76-baseline-gate.log:1202` — đó là dòng
  `Running tests/kt30_sync.rs`, **không** phải kết quả test; 10 test `... ok` ở **1204-1215**.
  Đề nghị dẫn `:1204-1215`.
- Các dẫn còn lại đã kiểm **đúng**: `:170` → `kt76-baseline-gate.log:1096`; `:173` →
  `kt76-selftest.log:35`; `:175` → `kt76-baseline-gate.log:668`; `:172` →
  `kt70-green-qemu-ktest-user.log:230-231`.
- `K6-DOD.md:174` ghi N5 “**KHÔNG ÁP DỤNG ở K6**” — theo D2-119/D2-120 phải đọc là **HOÃN K5′**
  (không phải “đã đóng”, không tính vào tử số). Bảng này dùng **HOÃN K5′**.

## 6. Kết luận cho văn bản đóng K6 (KT-82)

Số lấy từ bảng này: **2/7 đóng** (N4, N6) · **1/7 hoãn K5′** (N5) · **4/7 mở** (N1, N2, N3, N7).
**K7 không nhận N5.** Bốn mục mở chuyển K7 với tiêu chí đo được ở
`v2/agent/reports/QUYET-DINH-A0-K6-2026-11-17.md:57-63` (OD-K6-4): N1 “test đơn điệu **có migrate**
thật … trên 2 và 4 hart”; N2 “tái hiện được vi phạm T26 … rồi **50 boot / 0 đỏ**”; N3 “checker ‘giao
đúng một lần’, ca 4 client”; N7 “test ‘3 port’ dùng chung một cơ chế wake”.
