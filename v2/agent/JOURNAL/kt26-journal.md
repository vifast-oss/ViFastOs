# KT-26 — Fault + Exit + mô hình procsrv (K1c)

Agent 2 dòng OS · 2026-11-13 · HEAD vào `20e137c` ([KT-00] lật KT-25 DONE) ·
`gate --fast` baseline PASS **361 test/51 suite** (L1 1.000 seed 890 ms) —
[`kt26-baseline-gate-fast.log`](kt26-baseline-gate-fast.log).

## 1. Đối chiếu DoD (K1C-PLAN §5 KT-26)

| # | DoD | Bằng chứng |
|---|---|---|
| 1 | `FaultKind` **4 biến thể** `Exit{code}`/`Vm{fault_addr, access}`/`UnknownSyscall{nr}`/`UserException{ip, kind}`; spec là **tập con** tới K3 | `tcb.rs` (`FaultKind`, `FaultAccess`, `name/is_exit/exit_code/render`); test `fault_kind_bon_bien_the_model_only`; **`spec/` không đổi** (`git diff --stat` chỉ `v2/`) |
| 2 | Fault ≠ Exit: `inject_fault` ⇒ `BlockedOnSend{ep: fault_ep, call: true, fault}` + Reply gắn caller; procsrv `Reply` ⇒ `Ready` với **thanh ghi/MR mới** ghi **trước** `Ready`; test đọc `tcb_regs` sau reply | `kobj.rs::inject_fault`, `deliver_queued` (nhánh fault), `tcb_resume_with_regs`, `tcb_regs`; test `fault_unknown_syscall_qua_call_roi_reply_ghi_thanh_ghi`, `fault_vm_va_user_exception_cung_duong_call_reply` |
| 3 | Không mất message trên đường fault: `check_ipc` sau mỗi bước; **đột biến tay** "resume không ghi MR" ⇒ test (2) **đỏ** | [`kt26-mutation-resume.log`](kt26-mutation-resume.log) (**43 dòng**, đủ tên test): bỏ `deliver_message` trong `tcb_resume_with_regs` ⇒ **3 test resume** `FAILED` (`resume_with_regs_giu_exit_code_va_khong_phat_lai`, `..._khi_dang_cho_reply...`, `..._khi_fault_chua_giao...`), mỗi test lệch `left: [0,0,0,0]`; khôi phục nguyên trạng + xanh. **Giới hạn**: `check_ipc` chỉ đếm message có `msg_id`; chiều sender↔hàng của fault do `check_mailboxes` chứng minh (xem `v2/docs/k1c-fault.md` §6) |
| 4 | `Exit` đủ D2-25 — **4 test** | `exit_co_fault_ep_thanh_blocked_on_send_call_false`, `exit_procsrv_recv_xong_moi_inactive`, `exit_khong_fault_ep_inactive_ngay`, `exit_suspend_abort_huy_message_giu_code` |
| 5 | Mô hình procsrv `[TK]` `{code, how}` | `src/procsrv_model.rs` (`WaitHow{Exited,Cancelled,Faulted,Killed}`, `recv_exit`, `wait`, `kill`, `resume_fault`, `track`); test `procsrv_wait_*` ×3 + `procsrv_kill_*` |
| 6 | `.scn` **13, 14, 20** chạy qua runner (đọc tệp) | `tests/kt26_scn.rs` (`runner_scn_13_14_20_khong_lech`); 20 kiểm `BlockedOnSend{ep, call=false, fault=Exit{7}}` **ngay sau `exit`, trước Recv** (D2-41) |
| 7 | 1.000 seed × 64 op trộn exit/fault/recv/reply/suspend — 0 vi phạm, cùng seed cùng trace | `fuzz_1000_seed_64_op_bat_bien_va_tat_dinh` (bất biến `check_sched`/`check_mailboxes`/`check_ipc`/`check_caps`/`check_all_bytes` sau **mỗi** bước; chạy 2 lần/seed so trace) |
| 8 | `gate --fast` PASS; Miri `--test kt26_fault` PASS | [`kt26-gate-fast.log`](kt26-gate-fast.log) **386 test/53 suite**, L1 713 ms; [`kt26-miri.log`](kt26-miri.log) **24/24** |

## 2. Test đỏ trước (D2-05)

- [`kt26-red-compile.log`](kt26-red-compile.log) — `tests/kt26_fault.rs` **không biên dịch**:
  **53 lỗi** = **51×`E0599`** (`Machine::{inject_fault, tcb_resume_with_regs, tcb_regs,
  tcb_last_fault, take_last_fault}`; `FaultKind::{Vm, UnknownSyscall, UserException, name, is_exit,
  exit_code, render}`; `FaultAccess`) + **2×`E0432`** (`kobj::FaultAccess`, `vifast_sim::procsrv_model`).
- [`kt26-red-semantics.log`](kt26-red-semantics.log) — probe **tạm (đã xoá)** trên mô hình trước KT-26:
  1. `FaultKind` chỉ có `Exit` ⇒ không biểu diễn VM/unknown/user;
  2. `exit` đi `BlockedOnSend{call:false, fault=Exit}` (không phải đường Call+Reply);
  3. procsrv `Recv` ⇒ `Inactive`, **không** có `BlockedOnReply` ⇒ không có đường resume;
  4. `tcb_resume` **xoá** `exit_code` (`None`) và **không** ghi thanh ghi (`[0,0,0,0]`) ⇒ thiếu
     API "message + Ready" (r319);
  5. không có procsrv model ⇒ không có `wait`/`how`/`kill`.

## 3. Quyết định mô hình cần reviewer soi

1. **Fault không `fault_ep` ⇒ `InvalidArgument`, không đổi trạng thái** (KT-26 chỉ mô hình đường
   có handler; fault không handler để K3 chốt — không tự bịa "thread chết"). Ghi ở
   `v2/docs/k1c-fault.md` §6.
2. **`tcb_resume_with_regs` giữ `exit_code`** (khác `Tcb.Resume` KT-18 vốn xoá theo D2-25): D2-25
   chỉ buộc `Resume`/`Configure` xoá; đường fault-resume cần giữ mã để procsrv đọc (nhánh
   `Suspend(Abort)` huỷ message Exit). Có test `resume_with_regs_giu_exit_code_va_khong_phat_lai`.
3. **`tcb_resume_with_regs` nhận `Ready`/`Sleeping`** (kiểu `Tcb.Resume` + thanh ghi): `Sleeping`
   **giữ nguyên** `Sleeping{reason}` (bảng §4.5 `:309-310`, không có cặp danh tính), `Ready` không
   gọi `transition_tcb` (cặp `(1,1)` bị cấm).
4. **Fault Call không có Reply cap** ⇒ caller `Ready + last_ipc_error=Cancelled` (cùng khuôn D2-53
   Q7(b) cho `Call`), **không** treo vĩnh viễn. Test `fault_call_gap_recv_khong_co_reply_bi_cancelled`.
5. **`kill` = `Suspend(Abort)` + `CNode.Revoke` cap TCB** (Revoke con của cap, **không** Delete/
   finalise object — K3.5), ghi `Killed`; đúng câu chữ §6.4 + Q10. Fault/Exit message đang chờ bị
   huỷ nhưng `exit_code` giữ (D2-25).
6. **`last_fault: Option<(ObjId, FaultKind)>`** trên `TcbData` (model-only): message fault không
   mang MR nên procsrv cần `(sender, kind)`; ghi **một chỗ** trong `deliver_queued` (+ đường nhanh
   `exit`), tiêu thụ bằng `take_last_fault`, xoá khi procsrv `Reply`. Không đưa vào `TcbInfo`/
   `ReadState` (không đổi ABI).
7. **`check_mailboxes` siết hai chiều**: fault `Exit` cấm `call=true`; fault ≠ Exit **bắt buộc**
   `call=true` (Call + Reply) — bảng `ALLOWED` 11×11 **không** đổi.
8. Tên tệp test `kt26_fault.rs` (lead brief) thay cho `kt26_fault_exit.rs` (K1C-PLAN §5) — cùng
   nội dung; Miri list trong `v2/bin/v2` thêm `kt26_fault`.

## 4. Cổng và Miri

| Hạng mục | Kết quả | Log |
|---|---|---|
| `gate --fast` | **PASS 386 test/53 suite** (baseline 361/51 ⇒ **+25** = 24 `kt26_fault` + 1 `kt26_scn`), L1 1.000 seed/999 trace **713 ms** (đo bởi cổng), clippy `-D warnings` sạch, R1..R5 OK, `scn: OK` 21 tệp/11 nhóm | [`kt26-gate-fast.log`](kt26-gate-fast.log) |
| Miri | `--test kt26_fault` **PASS 24/24** (fuzz giảm `cfg(miri)` 3 seed × 8 op; stable 1.000 × 64) | [`kt26-miri.log`](kt26-miri.log) |
| `check` | PASS (fmt/clippy/gen fixtures/build) | [`kt26-check.log`](kt26-check.log) |

## 5. Tệp thay đổi

- `v2/sim/vifast-sim/src/tcb.rs` — `FaultKind` 4 biến thể + `FaultAccess` + `name/is_exit/
  exit_code/render`; `TcbState::render` dùng `FaultKind::render`.
- `v2/sim/vifast-sim/src/kobj.rs` — `TcbData.last_fault`; `inject_fault`; nhánh fault trong
  `deliver_queued` (Exit/khác Exit/thiếu Reply); `tcb_resume_with_regs`; `tcb_regs`/
  `tcb_last_fault`/`take_last_fault`; `record_last_fault`; `check_mailboxes` hai chiều;
  `reply_with` xoá `last_fault`.
- `v2/sim/vifast-sim/src/procsrv_model.rs` (**mới**, ~300 dòng) — mô hình procsrv.
- `v2/sim/vifast-sim/src/scenario.rs` — `Step::{Fault, ResumeWithRegs, ReplyFault}` + `actor`/
  `is_tcb_syscall`/`execute`/`name`.
- `v2/sim/vifast-sim/src/lib.rs` — `pub mod procsrv_model`.
- `v2/sim/vifast-sim/tests/kt26_fault.rs` (**mới, 24 test**), `tests/kt26_scn.rs` (**mới, 1 test**).
- `v2/docs/k1c-fault.md` (**mới**) — model-only `[TK]`, giới hạn, quy ước.
- `v2/bin/v2` — thêm `kt26_fault` vào `nightly --only-miri`.
- `v2/agent/STATE.md`, `v2/agent/JOURNAL/kt26-*.log`, `v2/agent/JOURNAL/kt26-journal.md`.

**Không** chạm: `spec/`, `vifast-model`, `.scn`, QUEUE, DECISIONS, AGENTS, `viscript/`; không
dependency/`unsafe`/`#[allow]`/`#[ignore]`; không đổi ABI.

## 6. Review độc lập (reviewer-kt26)

- 15:00 JST gửi `lead` yêu cầu spawn `reviewer-kt26` (depth 1/1, chưa commit). Reviewer tự chạy
  `gate --fast` 383/53 + L1 1000/999 + `kt26_fault` 21/21 + `kt26_scn` 1/1; xác nhận spec không
  đổi, code/ABI sạch.
- **Verdict: APPROVE** kèm 3 yêu cầu **bằng chứng/tài liệu** (không sửa lõi) — đã xử lý:
  1. **Mutation log** — chạy lại đột biến, log nay **43 dòng** có đủ tên 3 test + assertion lệch;
     khôi phục nguyên trạng (diff rỗng). Không còn claim quá log.
  2. **`v2/docs/k1c-fault.md` §6** — sửa câu "không mất fault message: `check_ipc`": nêu rõ
     `check_ipc` chỉ chứng minh bảo toàn message **có `msg_id`**; chiều sender↔hàng của fault do
     `check_mailboxes` (khớp `BlockedOnSend{call,fault}` ⇔ hàng) chứng minh. Không tuyên bố quá.
  3. **3 lỗ hổng test** — đã **thêm 3 test** (không nợ): `procsrv_kill_revoke_xoa_cap_dan_xuat`
     (Revoke xoá cap dẫn xuất), `resume_with_regs_giu_sleeping_va_ghi_thanh_ghi` (nhánh
     `Sleeping`), `fault_donation_di_theo_call_va_tra_lai_khi_reply` (donation với procsrv
     **passive** — `Rig::new_variant(false)`). Tổng `kt26_fault` = **24 test**.
- Sau sửa: `kt26_fault` 24/24 + `kt26_scn` 1/1, `gate --fast` **386 test/53 suite** PASS, Miri
  **24/24**; không sửa lõi (`kobj.rs` restore từ backup, `diff` rỗng).

## 7. Đề xuất

**DONE** (reviewer APPROVE + 3 yêu cầu đã xử lý; chờ Agent 0 lật QUEUE). YC mới: (a)
`spec/abi.toml` mở rộng `FaultKind` normative — chỉ Agent 0 quyết (Q5(a) đang để K3); (b) K3.5:
procsrv thật + shell thay `procsrv_model.rs` (Q10); (c) K3: chốt semantics fault không `fault_ep`.
