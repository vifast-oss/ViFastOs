# KT-41 — VM fault + `FaultKind::Vm` (model-only)

Người làm: **Agent 2 dòng OS** (phiên `agent2-kt41`). Ngày: 2026-11-14.
Nguồn ngữ nghĩa: `review/agent0/K2-PLAN.md` §3 **KT-41** + §2 **Q6**; **D2-65 Q6** (4 trường
`vspace/va/access/reason`, model-only); **D2-56(a)/(c)** (`FaultKind` vào spec là K3; không
`fault_ep` giữ K1c); thiết kế §4.6 `:321` ("Fault: Call tới fault endpoint **với Reply**; trả
lời Reply để tiếp tục, có thể kèm thanh ghi mới"); docs hiện hành
[`v2/docs/k2-vm.md` §15](../../docs/k2-vm.md).

## 0. Vào

- `agentctl next --project v2` trả **KT-41** (READY); KT-35..40 DONE; HEAD `f3872df0`
  ([KT-00] lật KT-40 DONE, 552 test — mở KT-41).
- `git rev-list --count HEAD..origin/main` = **0**.
- Working tree có `viscript/` + `review/` của tab khác — **không chạm**; chỉ stage `v2/`.
- Baseline: [`kt41-baseline-gate-fast.log`](kt41-baseline-gate-fast.log) — **PASS 552 test**/70
  suite, L1 1.000 seed **774 ms**.

## 1. Test đỏ trước (D2-05)

[`kt41-red-compile.log`](kt41-red-compile.log): `cargo test -p vifast-sim --test kt41_vmfault
--no-run` ⇒ **20 lỗi**: **E0599** ×17 (`Machine::vm_touch`, `Machine::check_fault` chưa có),
**E0559** ×4 (`FaultKind::Vm` dựng bằng 4 trường `vspace/va/access/reason` nhưng biến thể cũ chỉ
có 2), **E0432** ×3 (`VmFaultReason` chưa có), `could not compile` exit 101.
Không có đỏ ngữ nghĩa cũ vì KT-41 **không đổi** API cũ (VSpace/PageTable/Frame/TLB giữ nguyên
chữ ký); `FaultKind::Vm` đổi **hình** theo D2-65 Q6 — xem §2.

## 2. Code

- `v2/sim/vifast-sim/src/tcb.rs`: thêm `VmFaultReason { NotMapped, Permission, Wx }` (+`name`);
  `FaultKind::Vm` đổi sang **đúng bốn trường** `{vspace: ObjId, va: u64, access: FaultAccess,
  reason: VmFaultReason}` (D2-65 Q6); `name`/`render` cập nhật. `FaultKind::Exit`/
  `UnknownSyscall`/`UserException` không đổi; `is_exit`/`exit_code` không đổi.
- `v2/sim/vifast-sim/src/kobj.rs`:
  - re-export `VmFaultReason`; field `fault_events` (cửa sổ nhanh cho `check_fault`);
  - **`vm_touch(vs, va, access)`**: TCB đang chạy trên hart hiện tại (không có ⇒
    `IllegalOperation`); `vspace_walk` — `Ok` ⇒ tính reason, `NotMapped` ⇒ `NotMapped`, lỗi
    khác trả nguyên; có reason ⇒ `inject_fault(tcb, FaultKind::Vm{..})` (**đúng** đường fault
    K1c, không mở đường thứ hai); không `fault_ep` ⇒ `InvalidArgument` (D2-56(c)); **không**
    cấp trang/sửa PTE;
  - **`vm_touch_reason(&Mapping, access)`** — hàm thuần: đọc cần `READ`, ghi cần `WRITE`, exec
    cần `EXEC && !NX`; exec trang W / ghi trang X ⇒ `Wx` (W^X ở **thời điểm dùng** — KT-39 đã
    chặn map W+X nên `Wx` chỉ quan sát được ở đây); còn thiếu quyền ⇒ `Permission`;
  - **`check_fault()`** — bất biến "fault không mất/không nhân bản": (1) mỗi message fault
    trong hàng Endpoint khớp `BlockedOnSend{ep, call, fault}` của sender (`Exit` ⇒ call=false,
    fault khác ⇒ call=true); (2) mỗi TCB `BlockedOnSend` mang fault có **đúng một** message
    khớp trong hàng; gọi trong `sched_after_op` sau `check_tlb`; 1 unit test **dương tính**
    (`check_fault_bat_fault_bi_mat_va_bi_nhan_ban`: TCB rời hàng trần + fault nhân bản đều bị
    bắt, khôi phục thì sạch);
  - `fault_events += 1` trong `inject_fault` và `exit` (cửa sổ nhanh, không phải bộ đếm ABI).
- `v2/sim/vifast-sim/src/procsrv_model.rs`: `fault_log()` (nhật ký `(tcb, kind)`); doc nói rõ
  KT-41 — procsrv model **chỉ nhận + ghi log**, **không** cấp trang (không pager ở K2).
- `v2/sim/vifast-sim/tests/kt41_vmfault.rs` **(mới, 9 test)** — xem docs §15.6.
- `v2/sim/vifast-sim/tests/kt26_fault.rs`: **7 điểm dựng `FaultKind::Vm`** + **1 assertion
  `render`** cập nhật **cơ học** theo D2-65 Q6 (`fault_addr` → `va`, thêm `vspace: 1`,
  `reason`); **không** assertion nào bị nới/bỏ.
- `v2/bin/v2`: thêm `kt41_vmfault` vào danh sách `nightly --only-miri`.

## 3. Cổng

- [`kt41-check.log`](kt41-check.log): `v2 check` **PASS** (fmt, clippy `-D warnings`, R1..R5,
  gen-check, reserved-check).
- [`kt41-gate-fast.log`](kt41-gate-fast.log): **PASS 562 test / 71 suite**, `kt41_vmfault` 9/9;
  lib **63/63** (62 cũ + 1 unit `check_fault`); L0 562/0. Baseline 552/70 ⇒ **+10 test** (9
  integration + 1 unit) + **+1 suite** (binary `kt41_vmfault`). L1 1.000 seed: 1.428 ms (đợt
  gate đầu) / **47.758 ms** (đợt gate lại sau nit — tab ViScript chạy 821 test cùng lúc: host
  load, `T-INFRA`; **không** nới seed/ngưỡng, vẫn <60 s).
- `git diff --stat spec/` **rỗng** — `spec/abi.toml` không đổi (D2-65 Q6, D2-56(a)).

## 4. Miri

[`kt41-miri.log`](kt41-miri.log):

- `--test kt41_vmfault` — **9/9**, **389,60 s**, `MIRI1_EXIT=0` (property tự giảm còn **40**
  vòng bằng `cfg(miri)`; stable/gate vẫn **10.000**). Đây là chạy lại sau nit 3/4 của reviewer
  (bản trước chạy property 150 vòng dưới Miri quá chậm, đã dừng đúng job của mình, không đụng
  tiến trình khác).
- `--lib 'kobj::tests::check_fault_bat_fault_bi_mat_va_bi_nhan_ban'` — **1/1**, **0,84 s**,
  `MIRI2_EXIT=0`.
- Hai warning provenance của Miri (help text `with_exposed_provenance`, thuộc `vifast-objmem`)
  **không** phải lỗi UB; Miri vẫn exit 0.
- `kt41_vmfault` đã vào danh sách `v2 miri`/`nightly --only-miri` (`v2/bin/v2`).

## 5. Docs

- `v2/docs/k2-vm.md` **§15** — kiểu dữ liệu model-only; `vm_touch` một cửa; bảng
  `Permission`/`Wx`; bất biến `check_fault` + ca dương tính; C2 fault không mất; 9 test +
  property; **cập nhật test cũ cơ học**; ranh giới (không pager/spec/.scn/100k).
- `v2/agent/STATE.md` + JOURNAL này.

## 6. Review độc lập

`reviewer-kt41` (read-only) verdict **APPROVE**: tự chạy gate **562/71**, `kt41_vmfault` **9/9**,
`kt26_fault` **24/24**, `git diff spec/` rỗng; xác nhận 3 reason không trộn, dùng đúng
`inject_fault` K1c, không `fault_ep` ⇒ `InvalidArgument` (không kill ngầm), C2 checker hai chiều
+ ca dương tính, `kt26_fault` là **cơ học** (không nới), procsrv không thành pager.

Bốn nit đã sửa **trước commit**:

1. đếm đúng **7 điểm dựng + 1 assertion `render`** (docs §15.1/§15.7 + journal — trước ghi nhầm
   8 điểm/2 assertion);
2. số suite đúng **562/71** (baseline 552/70; +1 binary `kt41_vmfault`), không phải 562/70;
3. thêm câu §15.2 "`va` không cần căn trang — payload giữ đúng `va`, không `Alignment`" + ca
   `va=0x7001` trong test 1; chứng minh **đỏ trước** bằng mutation "làm tròn `va`" ⇒
   [`kt41-red-va-khong-can.log`](kt41-red-va-khong-can.log) (FAILED đúng assertion payload),
   khôi phục `cp` + `shasum -a 256` **khớp**, chạy lại 9/9 xanh;
4. bảng §15.3 thêm hàng `WRITE`/`WB｜NX`: exec ⇒ **`Wx`** (thắng Permission/NX) + assert trong
   `wx_exec_trang_write_va_ghi_trang_exec` (thêm mapping 0x3000).

`gate --fast` chạy lại sau 4 nit (log ghi đè [`kt41-gate-fast.log`](kt41-gate-fast.log)); Miri
chạy lại với property **40 vòng** (Miri) / 10.000 (stable) — xem §4.

## 7. Commit + push

- Stage **chỉ `v2/`** (`git add v2/`, không `git add -A`): `viscript/` + `review/` của tab khác
  còn nguyên trong working tree; tab ViScript có commit mới trên `main` trong lúc làm (`86079213`
  [ST-00] lật ST-67) nhưng **không chạm `v2/`** — không xung đột.
- `git rev-list --count HEAD..origin/main` = **0** trước push ⇒ `git push origin main`.
- Commit `[KT-41]`, hash báo lead; **không** lật QUEUE (D2-37) — đề xuất **REVIEW**.
