# KT-23 — nhật ký (tích hợp K1b: scenario, coverage, Miri, parity, DoD)

Ngày: 2026-11-13 · Task: **KT-23** (task cuối K1b) · Vào: HEAD `67aa8bc`, KT-18..KT-22 DONE,
baseline `gate --fast` PASS **311 test / 46 suite** (`kt23-baseline-gate-fast.log`, L1 1000/999 trace).

## 1. Đỏ trước (D2-05)

| Log | Nội dung |
|---|---|
| [`kt23-red-compile.log`](kt23-red-compile.log) | `tests/kt23_k1b.rs` không biên dịch: **30 lỗi** = 27×`E0599` + 2×`E0425` + 1×`E0432` (API mới chưa có: `k1b_report`, `k1b_audit`, `check_budget`, `check_sc_refs`, `forbidden_attempt_count`, `design_coverage`, `tcb::reserved_count/forbidden_count/coverage_design`, bước `TcbSuspend/TcbResume/TcbReadState/SchedTick/SchedPick`). |
| [`kt23-red-interleave.log`](kt23-red-interleave.log) | Probe **tạm (đã xoá)** trên API **cũ**: interleaver KT-09 lọc "actor **không** runnable" ⇒ TCB `Ready` sẵn sàng chạy vẫn `Deadlock { remaining: 1 }`. Đúng lý do phải sửa `run_interleaved`. |

## 2. Sản phẩm

- `scenario.rs`: 6 bước mới (`TcbSuspend/TcbResume/TcbReadState/TcbSetSchedParams/SchedTick/
  SchedPick`); `Step::dispatchable` + `run_interleaved` chỉ chọn bước chạy được (Running / Ready
  trong hàng + budget / Inactive + budget; D2-50) — bước quản trị luôn chọn được; cùng seed ⇒
  cùng trace. `kt09_e2e` chỉ đổi tên `Step::ReadState` → `Step::TcbReadState` (3 dòng, cùng nghĩa).
- `kobj.rs`: tách `check_sc_refs` + `check_budget` khỏi `check_sched` (check_sched vẫn gọi cả hai
  ⇒ không nới kiểm); thêm kiểm tham chiếu SC sống/đúng kiểu + `SchedContext.bound`; đếm cặp cấm
  ở một cửa `transition_tcb`; đếm `budget_sleeps`/`budget_refills`; `K1bReport`/`k1b_report`/
  `k1b_audit`; `design_coverage`/`forbidden_attempt_count`.
- `tcb.rs`: `is_reserved`/`reserved_count`=2/`forbidden_count`=73/`coverage_design`=46 + unit test.
- `tests/kt23_k1b.rs`: **9 test** — ma trận 121 cặp (46/46 thiết kế, 48/48 bảng, 73/73 cấm,
  `forbidden_attempts=73`), 4 ca cấm theo thiết kế, interleaved 64 seed (64 trace), 2 ca âm
  (hết budget, `Ready+suspended`), fuzz 1.000×64, Exit `Busy` snapshot, regression exit-fault-waiter,
  chốt 2 cặp reserved.
- `v2/bin/v2`: `nightly --only-miri` thêm `kt18_tcb/kt19_sched/kt20_budget/kt21_donation/kt22_mailbox`
  (không thêm `kt23_k1b`: 1.000 seed, không có I/O cần Miri; không thêm test I/O).
- `.scn` 12: **một dòng** `Defer => ok` → `Defer => InvalidArgument` (D2-52).
- `v2/docs/k1b-tcb-transition.md`: đóng cảnh báo `Defer` + ghi bằng chứng phủ bảng.

## 3. Bug do fuzz tìm ra (đã sửa + regression)

`Exit` mang fault vào `fault_ep` khi endpoint **đang có `Recv` waiter** vẫn đẩy message vào hàng
⇒ vỡ bất biến KT-22 "có message ⇒ không `Recv` waiter" (fuzz 1.000 seed bắt 44 lần trên cặp
`(Inactive, Inactive)` khi sửa nửa vời). Nay: giao **thẳng** cho waiter (không vào hàng), procsrv
coi như `Recv` xong ⇒ TCB thoát `Inactive` (D2-25); `Exit` từ `Inactive` **giữ** `Inactive`
(cặp danh tính bị cấm). Nhánh **không** waiter giữ nguyên (`BlockedOnSend{fault}`, `.scn` 14/20).
Regression: `kt23_k1b::exit_fault_giao_thang_cho_recv_waiter`.

## 4. Số đo (log)

| Việc | Số | Log |
|---|---|---|
| Coverage | **46/46 (100%)**, toàn bảng 48/48, cặp cấm **73/73**, `forbidden_attempts = 73` | `cargo test -p vifast-sim --test kt23_k1b` |
| Fuzz | 1.000 seed × 64 bước, **256.000** kiểm 4 miền, **0 vi phạm**; ops 45/46 (`Stopping→Dead`) |cùng log |
| Budget | `budget_sleeps=542`, `refills=7056`, `ticks=12883` |cùng log |
| Gate | `gate --fast` **321 test/47 suite** (KT-22: 311/46); L1 1000 seed/999 trace 687 ms; scn/gen-check OK | `kt23-gate-fast.log` (lần 1 FAIL clippy do `doc_lazy_continuation` + `unused_variables` — **sửa code** rồi PASS, không retry) |
| Miri | `nightly --only-miri` **12/12 PASS** (2.608 giây, ~43 phút) — objmem + sim 11 mục (thêm kt18..kt22) | `kt23-miri-only.log` |
| Parity | `kt11_diff_sim` 10.000 seed / 0 lệch (chỉ Untyped/CNode — Q7) | `kt23-parity.log` |
| `.scn` | 21 tệp/11 nhóm, 0 blocked; fixtures 5 ca âm PASS | `kt23-scn.log` |
