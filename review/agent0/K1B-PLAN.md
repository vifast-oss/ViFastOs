<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
> **KHÔNG PHẢI DUYỆT.** Đây là đầu vào để Agent 0 quyết. Duyệt chỉ có giá trị bằng dòng
> `APPROVED <YYYY-MM-DD> Agent 0` ghi trong `v2/agent/QUEUE.md` (AGENTS.md §8).
> Planner độc lập cho phase **K1b**; không sửa code / QUEUE / DECISIONS / `spec/`; chỉ tạo tệp này.

# K1B-PLAN — kế hoạch phase K1b (TCB, scheduler 1 hart, MCS donation)

**Lập ngày** 2026-11-13 · **HEAD khi lập** `ab87bb9` (`[KT-17]` DONE — K1a' DONE) · Kế hoạch đọc theo
commit này; cây làm việc chỉ còn thay đổi dòng ViScript (không liên quan K1b). Mọi task dưới đây **chưa mở**:
chờ Agent 0 duyệt kế hoạch + chốt các câu ở §6.

---

## 0. TL;DR

| Việc | Kết luận |
|---|---|
| Định nghĩa xong K1b | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:874`: *TCB + transition + `suspended`; scheduler theo hart; hộp thư; MCS giai đoạn đầu (donation)* — tiêu chí: **bao phủ cặp 100%**, **kiểm budget**, **push migration** |
| Code K1a đã có gì | `TcbState` đủ 11 tên; `call/recv/reply/exit` mức tối giản; `SchedData {budget, period}`; **chưa có** transition table, `suspended`, scheduler/run queue, `Running` thực, budget accounting, donation, hộp thư, hart |
| Khoảng trống lớn nhất | (1) `SchedContext` **không có đường gắn vào TCB** — `SchedContext.Bind` chỉ nhận Reply/Notification (`kobj.rs:1344`, `spec/abi.toml:218-221`); (2) budget "mọi cửa sổ `period`" (`thiết kế:296`) **không tương thích** với "nạp lại đầu mỗi chu kỳ" (`thiết kế:302`); (3) push migration cần ≥2 hart trong khi K1b được giao **1 hart** |
| Đề xuất | **6 task `KT-18..KT-23`**, tổng **16 h** (17 h nếu Agent 0 chọn mô hình 2 hart), mỗi task ≤ 3 h, tuần tự |
| Cần Agent 0 chốt | **8 câu ở §6**; nặng nhất là **Q1** (1 hart hay 2-hart logic/push migration), **Q2** (đường gắn TCB↔SchedContext), **Q3** (donation tự động hay qua `Bind`), **Q5** (định nghĩa cửa sổ budget) |
| Không đụng | Không sửa `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS trong K1b nếu Agent 0 không chọn phương án mở; K1b làm **ngữ nghĩa mô hình** trước, ABI binding sau |

---

## 1. Tiêu chí K1b và ràng buộc bắt buộc

### 1.1 Định nghĩa xong (nguồn: thiết kế `:874`)

K1b đạt khi có **bằng chứng đo được** cho 4 mục đầu; mục 5 (push migration) đang chờ **Q1**:

1. **TCB + transition + `suspended`**: bảng chuyển trạng thái có một đường duy nhất; phủ **100% cặp ALLOWED**;
   ca âm của mọi cặp cấm bị từ chối (`IllegalOperation`), không im lặng đổi trạng thái.
2. **Scheduler theo hart**: ready queue 256 mức + bitmap, `pick` O(1), preempt được ép ở **mọi bước**,
   affinity, không đói; bất biến hàng đợi kiểm sau mỗi event.
3. **Hộp thư**: hàng đợi cố định, đầy thì backpressure, thứ tự tất định.
4. **MCS giai đoạn đầu**: `SchedContext` budget/period nạp lại đầu mỗi chu kỳ; TCB hết budget →
   `Sleeping{reason=Budget}`; **donation qua Call/Reply**; passive server; bind Notification (phần thức dậy
   để K1c).
5. **Push migration**: thiết kế ghi ở cột "tiêu chí xong" của K1b (`:874`) — xem **Q1** vì K1b được giao
   1 hart, còn K1c mới yêu cầu `smp 1/2/4/8` (`:875`).

### 1.2 Bảng ràng buộc

| Ràng buộc | Nội dung | Hệ quả cho K1b |
|---|---|---|
| **Thiết kế §4.4** (`:256-286`) | Enum 11 trạng thái; `transition(tcb,to)` duy nhất tra `const ALLOWED`; bảng xuất tài liệu; `suspended` trực giao; `Abort` gỡ khỏi hàng **trước** khi bật cờ; `Resume` **không phát lại** IPC; `ReadState` trả 4 giá trị | Test-first cho transition table + suspend/abort/exit; không thêm enum mới |
| **D2-33** (`DECISIONS.md:69`) | Enum đầy đủ, `BlockedOnRecv{ep, reply}`, `BlockedOnSend{ep, call, fault}`, `Sleeping{Timeout hoặc Budget}`, cờ `suspended` + `exit_code` | KT-18 phải sửa drift `BlockedOnRecv` thiếu `reply`; `offset` chốt kiểu theo thiết kế `:269` (u32) |
| **Thiết kế §4.5** (`:288-311`) | `SchedContext` budget/period/refill, TCB chỉ chạy khi SC còn budget; hết budget → `Sleeping{Budget}`; `SchedControl.Configure`; passive server; donation Call/Reply; bind Notification | KT-20/21; **không** làm refill sporadic/timeout fault (hoãn sau K6, D2-16) |
| **Thiết kế §4.8** (`:336-344`) | 256 mức ilist + bitmap; `pick` O(1); affinity HartMask; home; push migration đọc `idle`/`cur_prio` công bố; **không work stealing** | KT-19 (queue 1 hart), KT-22 (hộp thư/push nếu Q1=A) |
| **Thiết kế §4.13** (`:382-387`) | Preempt được ép tại **mọi bước**; trạng thái trung gian giữ invariant | KT-19: `preempt_every_step` + `check_sched()` sau mỗi op |
| **Thiết kế §4.16/§4.17** (`:403-428`) | Luật 2/3/5; hộp thư MPSC cố định; lock-free kiểm bằng **loom nếu có, không thì stress** | K1b **không có loom** (đo §8) và sim 1 luồng ⇒ chỉ mô hình hoá hộp thư; MPSC thật + stress để K1c/SMP (Q6) |
| **Thiết kế §9.2** (`:746`) | TCB/transition: mọi cặp ALLOWED và cấm; MCS: budget theo cửa sổ, passive server; scheduler: push migration, affinity, không đói | Bộ kiểm K1b phải có cả ca dương và ca âm |
| **Thiết kế §9.5** (`:819`) | Cổng K1: **100% cặp ALLOWED**, ≥95% nhánh lỗi `[ƯL]` | K1b báo coverage transition; nhánh lỗi để K1c |
| **D2-17 + R5** | `unsafe` chỉ trong `vifast-sync`, `vifast-objmem`, `kernel/vifast-plat-*`, `boot/` | Mọi code K1b trong `v2/sim/**` phải **0 unsafe**; **không** tạo `vifast-sync` trong K1b (Q6) |
| **D2-14 / D2-47** | Miri từ commit đầu; Miri chạy lệnh rời, **không** vào `gate --fast` | KT-23 chạy `v2 miri` trên tập chọn + `nightly --only-miri`; không nới gate |
| **QD §5.2** | `gate --fast` ≤ 60 s; L1 1.000 seed; nightly 100.000 seed | Mỗi task cuối bằng `gate --fast`; không chạy nightly đầy đủ trong vòng task |
| **QUEUE Khối A/B/C** | Test đỏ trước, log đỏ; ≤3 h/task; kẹt 2 lần → BLOCKED; không retry/nới ngưỡng | Mỗi task dưới có mục "test đỏ trước" và "BLOCKED khi" |

### 1.3 K1b **không** phải K1c

K1b chỉ mở **ngữ nghĩa TCB/scheduler/MCS**; các phần sau **không** làm trong phase này:

- IPC đầy đủ (4 cap, unwrap badge, `no_reply`, ReplyRecv, fault VM/unknown/user exception), Notification
  signal/wait, WaitOnAddress, `Exit` đầy đủ theo procsrv — thuộc **K1c** (thiết kế `:875`).
- Trong K1b `call/recv/reply` chỉ dùng để kiểm **transition + donation**; không mở rộng message/cap.
- Không mở K2/VSpace/Frame, không QEMU, không SMP thật.

---

## 2. Hiện trạng code (đọc, không sửa)

| Thành phần | Vị trí | Hiện trạng |
|---|---|---|
| `TcbState` | `v2/sim/vifast-sim/src/kobj.rs:272-348` | Đủ 11 tên; `BlockedOnRecv { ep }` **thiếu `reply`** (`:283`); `BlockedOnAddress.offset: u64` (`:292`) trong khi thiết kế `:269` ghi `u32` |
| `TcbData` | `kobj.rs:382-388` | Chỉ `state`, `fault_ep`, `cspace`, `exit_code`; **không** có `suspended`, `priority`, `affinity`, `home`, `sched_context`, `bound_notification` |
| `set_tcb_state` | `kobj.rs:1711-1719` | Gán trực tiếp, **không kiểm ALLOWED**, không đếm cặp transition |
| `tcb_read_state` | `kobj.rs:1369-1372` | Chỉ trả `TcbState`; **không** trả `suspended`/`blocked_on_kind`/`exit_code` như D2-33 |
| `SchedData` | `kobj.rs:390-394` | Có `bound`, `budget`, `period`; **không** có `consumed`, `refill`, timer, hart |
| `SchedControl.Configure` | `kobj.rs:1322-1342` | Ghi budget/period; **không** trừ budget, không sinh `Sleeping{Budget}` |
| `SchedContext.Bind` | `kobj.rs:1344-1360` | Chỉ nhận `Reply`/`Notification`; **không có đường TCB → SchedContext** |
| `call/recv/reply` | `kobj.rs:1375-1471` | `call` đặt server `Ready` (`:1386`) nhưng **không đưa vào ready queue** (chưa có queue); `recv` đặt sender `Inactive`/`Ready`; không donation |
| `Machine` | `kobj.rs:477-485` | Chỉ `kernel`, `regions`, `objects`, `caps`; **không hart, không scheduler, không mailbox**; `Machine::new()` 1 arena (`:525`) |
| Scheduler "tối thiểu" KT-08 | `v2/sim/vifast-sim/src/lib.rs:2,133,229-257` | Là scheduler của **task harness** (tick/period), **không** liên quan `TcbState`/hart; không được dùng làm bằng chứng TCB |
| Interleaver | `v2/sim/vifast-sim/src/scenario.rs:328-394` | Chen **giữa các bước kịch bản**, chọn theo seed; không có `Running`/queue của TCB |
| `vifast-model` | `v2/lib/vifast-model/src/machine.rs:56-70` | Chỉ Untyped/CNode; **không** TCB/scheduler; `kt11_diff_sim` băm metadata, không băm body/state ⇒ parity 10k là **hồi quy**, không phải oracle TCB |
| `.scn` suspend/donation | `v2/test/scn/09..12,18,19` | Có tệp nhưng **chưa chạy** trên mô hình Rust (`kt11_scn` chỉ chạy 9 ca Untyped/CNode) |
| Miri / loom | môi trường | Miri có (`miri 0.1.0 (3ef0c22cdb 2026-11-11)`); **không có loom** trong registry (§8) |

**Kết luận phạm vi**: K1b không phải "thêm vài method" mà là **thay ruột TCB + dựng scheduler/MCS từ đầu**,
trong khi vẫn phải giữ xanh 231 test/41 suite hiện có, parity 10k, L1 1000 và Miri.

---

## 3. Phân rã theo trục (tránh lẫn "MCS" hai nghĩa)

| Trục | Nội dung K1b | Task | Ghi chú |
|---|---|---|---|
| TCB fields/lifecycle | `suspended`, `exit_code`, priority/affinity/home, `sched_context`, `bound_notification`; `Suspend(Abort/Defer)`, `Resume`, `ReadState`; finalise TCB | **KT-18** | Transition table là "xương sống"; mọi method phải đi qua một cửa |
| Scheduler run queue + preempt | 256 mức + bitmap, `Ready/Running`, `pick`, preempt mọi bước, affinity, không đói, finalise khỏi queue | **KT-19** | Chỉ **1 hart** theo brief; cấu trúc viết theo hart để mở rộng |
| MCS budget | `SchedContext` consumed/refill một đoạn, timer ảo, `Sleeping{Budget}`, invariant budget theo chu kỳ | **KT-20** | **Không** refill sporadic/timeout (sau K6) |
| MCS donation | SC đi theo Call sang server passive; trả về khi Reply; charge cho client; MCP | **KT-21** | Cần Q2/Q3; tách khỏi budget để test được độc lập |
| Hộp thư / push migration | Mailbox cố định, FIFO, đầy → `Busy`; nếu Q1=A thêm mô hình 2 hart + push migration tất định | **KT-22** | **Không** làm MPSC lock-free thật/loom trong K1b (Q6) |
| Tích hợp + parity + Miri | Scenario steps mới, L1 K1b 1.000 seed, coverage 100%, Miri chọn, parity 10k hồi quy, bảng DoD | **KT-23** | Không sửa `vifast-model`; parity chỉ là hồi quy |

### 3.1 "MCS" trong K1b là **Mixed-Criticality Scheduling**, không phải MCS lock

Thiết kế dùng cùng chữ "MCS" cho hai thứ khác nhau:

- **MCS giai đoạn đầu** (`:288-311`, QD §3.4, D2-16) = `SchedContext`, budget/period, **donation** → K1b.
- **MCS lock** (`:407`, hàng S1) = queue lock cho BKL, và **hộp thư lock-free** (`:423-428`) → thuộc hạ tầng
  đồng thời; chỉ kiểm được khi có SMP/loom/stress. K1b **không** có hai thứ này (Q6).

Nếu Agent 0 muốn MCS lock trong K1b thì phải mở một task riêng, và trước hết phải giải bài toán **không có
loom** — kế hoạch này **không** đề xuất làm vậy.

---

## 4. Bảng task đề xuất

| ID | Mục tiêu | Phụ thuộc | Ngân sách | Trạng thái đề xuất |
|---|---|---|---|---|
| **KT-18** | TCB đầy đủ + `suspended` + bảng `ALLOWED` + `Suspend/Resume/ReadState` + finalise TCB | K1a' DONE; Q4 | **3 h** | READY sau Q4 |
| **KT-19** | Scheduler 1 hart: 256 mức + bitmap, `Ready/Running`, preempt mọi bước, affinity, không đói, finalise khỏi queue | KT-18 | **3 h** | chờ KT-18 |
| **KT-20** | MCS budget: timer ảo, consumed/refill một đoạn, `Sleeping{Budget}`, invariant budget theo chu kỳ | KT-19; Q2/Q5 | **3 h** | chờ KT-19 + Q2/Q5 |
| **KT-21** | Donation qua Call/Reply + passive server + bind Reply/Notification + MCP + budget khi donation | KT-20; Q2/Q3 | **3 h** | chờ KT-20 + Q3 |
| **KT-22** | Hộp thư mô hình (FIFO, đầy → `Busy`, backpressure); **nếu Q1=A**: mô hình 2 hart + push migration tất định | KT-19; Q1/Q6 | **2 h** (3 h nếu Q1=A) | chờ KT-19 + Q1 |
| **KT-23** | Tích hợp scenario + L1 K1b 1.000 seed + coverage 100% + Miri + parity 10k + bảng DoD K1b | KT-18..KT-22 | **2 h** | chờ KT-22 |

Tổng **16 h** (17 h nếu Q1=A), tuần tự vì cùng chạm `kobj.rs`/`sched.rs`; không task nào chạm
`spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS.

---

## 5. Chi tiết từng task

### KT-18 — TCB đầy đủ, transition table, `suspended`, vòng đời

**Mục tiêu**: biến `TcbData` thành đúng §4.4/§4.5; mọi đổi trạng thái đi qua **một** hàm `transition()`;
`ReadState` trả đủ 4 giá trị; `Suspend(Abort)` gỡ khỏi hàng chờ **trước** khi bật cờ; finalise TCB sạch.

**Test đỏ trước** (phải dán log vào `JOURNAL`):

1. `tests/kt18_tcb.rs` gọi `m.tcb_suspend(slot, SuspendMode::Abort)` → **E0599** (chưa có method);
   `let s: TcbSnapshot = m.tcb_read_state(slot)` → **E0308** (hiện trả `TcbState`).
2. Sau khi có khung: gọi `m.transition_tcb(tcb, Running{hart:0})` từ `Inactive` phải trả
   `IllegalOperation`; hiện `set_tcb_state` (`kobj.rs:1711`) nhận mọi cặp ⇒ test đỏ vì `Ok`.
3. `Suspend(Abort)` trên client `BlockedOnReply`: hiện không có method; sau khung phải thấy
   `Reply.caller == None`, state `Ready + suspended`, `exit_code` giữ nguyên. Ca hiện tại còn caller ⇒ đỏ.

**DoD đo được**:

1. `TcbData` có đủ: `state`, `suspended`, `exit_code`, `fault_ep`, `cspace`, `priority: u8`,
   `max_controlled_priority: u8`, `affinity: u64` (HartMask ≤64), `home: u16`,
   `sched_context: Option<ObjId>`, `bound_notification: Option<ObjId>`. Test đọc/ghi từng trường qua
   `ReadState` hoặc model API nội bộ `[TK]`.
2. `TcbState` khớp **D2-33**: thêm `reply` vào `BlockedOnRecv`; `BlockedOnAddress.offset` chốt `u32`
   (theo thiết kế `:269`); `render()` cập nhật; `TCB_STATES` 11 tên không đổi. Ca unit
   `tcb_state_du_11_ten_theo_spec` (`kobj.rs:1990`) chỉ được sửa **cơ học** cho field mới, không đổi kỳ vọng.
3. `const ALLOWED: [[bool; 11]; 11]` + `fn transition_tcb(tcb, to)` **duy nhất**; mọi đường
   `call/recv/reply/exit/suspend/resume/finalise` dùng nó. Cặp cấm trả `IllegalOperation`, **không** đổi state.
4. Bộ đếm `pair_hits: [u32; 121]`; `transition_coverage()` trả `(hit, allowed)`; test liệt kê **mọi**
   cặp 11×11: cặp ALLOWED phải `Ok`, cặp cấm phải `IllegalOperation`; 100% cặp ALLOWED có ít nhất 1 hit.
5. `Suspend(Abort)` từ `BlockedOnSend/Recv/Reply/Notification/Address`: gỡ khỏi endpoint send/recv,
   `Reply.caller`, notification waiter, address bucket; đặt `Ready + suspended`; **giữ** `exit_code`.
   Test đếm: hàng chờ giảm đúng 1, không còn tham chiếu TCB trong object chờ.
6. `Suspend(Defer)`: với IPC trả `InvalidArgument`, state không đổi; chưa có thao tác dài nên **mọi**
   `Defer` hiện trả `InvalidArgument` (ghi rõ trong doc test).
7. `Resume` chỉ xoá `suspended`; **không** phát lại Call/Send/Recv; test đếm số message/queue trước/sau bằng nhau.
8. `Exit` giữ đúng D2-25: có `fault_ep` → `BlockedOnSend{call=false, fault=Exit{code}}`; `Recv` xong →
   `Inactive`; không `fault_ep` → `Inactive` ngay; `Suspend(Abort)` khi đang chờ Exit huỷ message nhưng
   `ReadState.exit_code` vẫn đọc được.
9. Finalise TCB khi `Delete`/`Revoke` cap: gọi hook `finalise_tcb` **trước** `destroy` → state `Dead`,
   gỡ endpoint/notification/Reply, trả `sched_context` (hook), ghi log finalise; sau đó object chết.
   Gỡ **ready queue** là hook do KT-19 cài (test ở KT-19).
10. `gate --fast` PASS không retry; `v2 miri -p vifast-sim --test kt18_tcb` PASS.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/kobj.rs`, `v2/sim/vifast-sim/src/tcb.rs` (mới, ~200 dòng),
`v2/sim/vifast-sim/tests/kt18_tcb.rs`, `v2/agent/{STATE.md,JOURNAL/}`. Nếu Q4=B: thêm
`v2/docs/k1b-tcb-transition.md` (**model-only**, `[TK]`, không phải spec).

**Rủi ro**: (a) đổi field `BlockedOnRecv` làm vỡ pattern match/unit test — sửa cơ học, không đổi kỳ vọng;
(b) `Box`/`Vec` trong `Kind` + `destroy` ⇒ thứ tự finalise sai gây Miri UB; (c) `exit_code` bị xoá nhầm bởi
`Resume/Configure` (D2-25 nói `Resume`/`Configure` xoá — kiểm cả hai chiều); (d) bảng 121 cặp dễ tự chứng minh
vòng tròn ⇒ ca cấm phải viết từ lý do thiết kế, không chỉ đọc bảng.

**BLOCKED khi**: Q4 chưa chốt vị trí bảng; hoặc `ALLOWED` mâu thuẫn thiết kế ở một cặp không suy ra được
(ví dụ `BlockedOnReply → Dead`) ⇒ dừng, nêu `file:line` + 2 lựa chọn; hoặc vượt 3 h.

---

### KT-19 — Scheduler 1 hart: run queue, `pick`, preempt, affinity, không đói

**Mục tiêu**: dựng scheduler thật cho TCB (không phải scheduler harness KT-08), 1 hart, cấu trúc theo hart
để K1c mở rộng; mọi chuyển `Ready/Running` đi qua queue.

**Test đỏ trước**:

1. `m.ready_len(0)`/`m.sched_pick()`/`m.current(0)` → **E0599**.
2. Sau khung: `m.call(...)` làm server `Ready` nhưng `ready_len(0) == 0` (hiện `set_tcb_state` chỉ đổi state,
   `kobj.rs:1386`) ⇒ `check_sched()` báo *"Ready không nằm trong ready queue"* — đỏ đúng hành vi.
3. Chưa từng có `Running` trong code chạy (chỉ enum/test) ⇒ test dispatch phải đỏ vì `current(0) == None`.

**DoD đo được**:

1. `HartSched` gồm `queues: [VecDeque<ObjId>; 256]` (hoặc tương đương), `bitmap: [u64; 4]`, `len`,
   `current: Option<ObjId>`, `idle: bool`, `cur_prio: u8`. `pick()` O(1) bằng bit cuối; test đủ 256 mức
   priority `0..=255`, mức cao nhất được chọn, cùng mức FIFO theo thứ tự wake.
2. `dispatch(hart)`: nếu đang `current` thì `Running→Ready` (enqueue lại) rồi `Ready→Running`; bảo đảm
   **≤1 Running/hart**, `Running` **không** nằm trong queue, `Ready` **iff** có trong queue và `!suspended`.
3. `check_sched()` kiểm: không trùng TCB trong queue; priority bucket khớp `priority`; mọi TCB trong queue
   tồn tại và `Ready`; `current` khớp `Running{hart}`; `suspended` không nằm trong queue. Gọi sau **mỗi**
   public op và sau mỗi preempt.
4. Preempt: `m.set_preempt_every_step(true)`; sau mỗi op, TCB đang `Running` bị đưa về `Ready` rồi chọn lại;
   test 2 TCB cùng priority chạy xen kẽ, không TCB nào bị bỏ đói quá N dispatch `[TK]`; test 1.000 seed ×
   64 op ngẫu nhiên (call/recv/reply/suspend/resume/exit/retê) — cùng seed ⇒ cùng trace, invariant giữ.
5. `block_current`, `wake(tcb)`, `yield_current` cập nhật queue + state qua `transition_tcb`; wake TCB
   `suspended` thì **không** enqueue; resume TCB `Ready` thì enqueue đúng priority.
6. Finalise/suspend gỡ khỏi queue: `Delete`/`Suspend(Abort)` khi `Running`/`Ready` ⇒ `ready_len` giảm,
   `current` được thay; không còn `ObjId` chết trong queue.
7. Priority đặt qua model API nội bộ `set_priority(tcb,u8)` `[TK]` (chưa tuyên bố ABI `Tcb.SetPriority`);
   affinity mặc định full mask; home = hart 0.
8. `gate --fast` PASS; L1 1.000 seed hiện có vẫn PASS; `v2 miri -p vifast-sim --test kt19_sched` PASS.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/sched.rs` (mới, ~350 dòng), `kobj.rs` (gọi `transition`/queue),
`tests/kt19_sched.rs`, `v2/agent/{STATE,JOURNAL/}`.

**Rủi ro**: (a) borrow `body_mut` + `&mut self` scheduler dễ vướng — dùng helper lấy state rồi thả borrow;
(b) sequence/FIFO phải tất định, không dùng `HashMap`; (c) preempt sau **mọi** op có thể làm vỡ thứ tự
`call/recv` hiện có ⇒ chạy test cũ trước khi bật mặc định; (d) đừng nhầm scheduler harness `lib.rs:229`.

**BLOCKED khi**: Q1 chọn phương án không có scheduler 1 hart (không hợp lệ — K1b vẫn cần); hoặc
`Ready + suspended` không biểu diễn được nếu không thêm cờ (phải giữ đúng §4.4, không thêm enum).

---

### KT-20 — MCS budget: timer ảo, consumed/refill, `Sleeping{Budget}`

**Mục tiêu**: TCB chỉ chạy khi có `SchedContext` còn budget; hết budget → `Sleeping{Budget}`; nạp lại đầu
mỗi chu kỳ; kiểm invariant budget; **không** làm refill sporadic/timeout.

**Test đỏ trước**:

1. `m.tick(1)`/`m.sched_consumed(sc)`/`m.sc_attach(tcb, sc)` → **E0599**.
2. Sau khung: chạy TCB quá `budget` tick, state vẫn `Running` (hiện không trừ budget) ⇒ test `Sleeping{Budget}`
   đỏ; `sched_consumed == budget+1` thay vì dừng đúng biên.
3. TCB không có SC vẫn được scheduler chọn (hiện scheduler chưa có) ⇒ sau KT-19, test "không SC ⇒ không
   chọn" đỏ.

**DoD đo được**:

1. Thời gian ảo `now: u64` + `tick(n)` tất định; **không** đọc `Instant`/wall clock; timer là danh sách
   deadline sắp xếp (không heap cũng được, N nhỏ); test cùng seed ⇒ cùng mốc refill.
2. `SchedContext` có `consumed`, `next_refill` (hoặc `period_start`), `budget`, `period`; `Configure`
   kiểm `budget>0`, `period>0`, `budget≤period` (giữ như hiện tại).
3. Đường gắn TCB↔SC theo **Q2**: model API `set_sched_params(tcb, sc)` `[TK]` (không tuyên bố ABI nếu
   Agent 0 chưa duyệt chữ ký `Tcb.SetSchedParams`); `SchedContext.Bind` vẫn chỉ Reply/Notification.
4. Scheduler chỉ `pick` TCB có SC và `consumed < budget`; TCB không SC **không** chạy (passive server là
   ngoại lệ do KT-21 cài, không nới ở đây). **Không** từ chối `Call/Recv` ở API vì thiếu SC — test cũ
   KT-09/kt11 phải vẫn xanh; chỉ `pick` bỏ qua.
5. Charge **chỉ** khi `Running`: mỗi tick trừ 1; blocked/suspended không trừ; `Consumed` trả đúng.
6. Hết budget: `Running → Sleeping{Budget}`, gỡ khỏi queue; timer ở biên chu kỳ **kế tiếp** (theo
   `period_start + period`); không TCB nào cùng SC chạy trong lúc `Sleeping`.
7. Refill: tại biên chu kỳ `consumed = 0`; nếu `Sleeping{Budget}` và `!suspended` → `Ready` + enqueue;
   nếu `suspended` → vẫn `Sleeping{Budget}` (bảng `:304-311`); `Resume` không phát lại gì.
8. Invariant budget: với 1.000 seed × 64 tick ngẫu nhiên, mỗi SC có tổng tick chạy trong **mỗi chu kỳ
   thẳng hàng** `[k·period, (k+1)·period)` ≤ `budget`; không có tick nào chạy khi `consumed == budget`.
   **Ghi rõ** đây là cửa sổ thẳng hàng; cửa sổ trượt thuộc Q5.
9. `yield_current()` nội bộ trả budget còn lại và reschedule; test `Yield` không làm mất budget/thời gian.
10. Đột biến tay (không commit): bỏ dòng trừ budget ⇒ test (8) đỏ; dán log `JOURNAL`.
11. `gate --fast` PASS; Miri `--test kt20_mcs` PASS.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/sched.rs`/`mcs.rs`, `kobj.rs`, `tests/kt20_mcs.rs`.

**Rủi ro**: (a) biên chu kỳ lệch 1 tick — chốt "charge trước hay sau khi chạy" bằng test biên;
(b) `u64` overflow khi tick lâu — dùng `checked_add`/bão hoà có test; (c) refill khi TCB đang blocked;
(d) đừng trộn "cửa sổ trượt" vào test (Q5); (e) đặt điều kiện SC vào `pick`, không vào `call/recv`,
    nếu không sẽ phá các test cũ không cấu hình SC.

**BLOCKED khi**: Q2 chưa chốt đường gắn SC ⇒ **không tự thêm field vào `Tcb.Configure`**; Q5 chưa chốt
nghĩa "mọi cửa sổ" ⇒ ghi nhận, không tự nới; hoặc vượt 3 h.

---

### KT-21 — MCS donation: passive server, Call/Reply, MCP

**Mục tiêu**: SC của client đi theo Call sang server không có SC; thời gian phục vụ tính cho client; Reply
trả SC về; server passive không tự chạy khi không được donate.

**Test đỏ trước**:

1. `m.sched_donated_to(server)`/`m.sc_of(tcb)` → **E0599**.
2. Sau khung: sau `call`, server `Ready` nhưng `sc_of(server) == None`, `sched_consumed(client_sc) == 0`
   khi server chạy ⇒ test donation đỏ (hiện `call` chỉ đổi state, `kobj.rs:1375-1410`).
3. Server passive (không SC) bị scheduler bỏ qua hoàn toàn ⇒ test "server không chạy" đỏ nếu chưa có
   đường donate.

**DoD đo được** (theo **Q3**; mặc định donation tự động):

1. `Call` từ client có SC tới server **không SC**: client `BlockedOnReply`; SC được đánh dấu `donated_to =
   server`; server `Ready` ở priority của client; scheduler charge tick của server vào **SC của client**;
   client không bị charge khi đang chờ.
2. `Reply`: SC trả về client (`donated_to = None`); server mất SC (passive → không runnable; nếu Q3 chọn
   "active server giữ SC riêng" thì server trở lại SC của nó); client `Ready`, budget còn lại giữ nguyên.
3. `SchedContext.Bind(sc, reply)` (chữ ký đã duyệt) chỉ **gắn/kiểm** quan hệ SC–Reply theo Q3; không tạo
   donation thứ hai; `Bind` Notification chỉ kiểm cấu trúc (thức dậy thật ở K1c).
4. Hết budget khi server đang chạy: server `Sleeping{Budget}`, client vẫn `BlockedOnReply`; refill → server
   `Ready`; Reply hoàn tất; **không deadlock**, không mất reply.
5. `Suspend(Abort)`/`Exit`/`Revoke` server khi đang được donate: SC trả về client (hoặc thu hồi theo Q3),
   `donated_to` sạch, không rò SC; client nhận `Cancelled` hoặc được đánh thức theo đúng trạng thái — test
   cả hai nhánh nếu Q3 cho phép.
6. MCP: server chạy không vượt `max_controlled_priority` của client; test client priority thấp hơn/khác
   server; không đảo ưu tiên.
7. Invariant: một SC chỉ có **một** TCB dùng tại một thời điểm; donor và donee không cùng `Running`;
   `consumed` đơn điệu trong chu kỳ; 1.000 seed kịch bản donation tất định, cùng seed cùng trace.
8. `gate --fast` PASS; Miri `--test kt21_donation` PASS.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/mcs.rs`, `kobj.rs` (`call/reply`), `tests/kt21_donation.rs`.

**Rủi ro**: (a) donation lồng nhau (server gọi tiếp) — K1b **không** làm chuỗi >1, ghi rõ; (b) Reply bị
xoá giữa chừng — K1c, nhưng test finalise phải không rò SC; (c) borrow khi vừa đổi TCB vừa đổi SC;
(d) đừng dùng `SchedContext.Bind` làm nguồn donation nếu Q3 chọn tự động.

**BLOCKED khi**: Q2/Q3 chưa chốt ⇒ dừng, **không tự đặt ngữ nghĩa donation** (đây là ABI-adjacent);
hoặc phát hiện `SchedContext` không thể gắn TCB bằng bất kỳ chữ ký đã duyệt nào ⇒ BLOCKED + 2 lựa chọn.

---

### KT-22 — Hộp thư mô hình (+ push migration nếu Q1=A)

**Mục tiêu**: mô hình hoá hộp thư theo hart đúng ngữ nghĩa §4.17 (FIFO, cố định, đầy → backpressure);
nếu Agent 0 chọn Q1=A, thêm mô hình **2 hart tất định** và push migration theo §4.8.

**Test đỏ trước**:

1. `m.mailbox(0)`/`m.with_harts(2)` → **E0599**; chưa có message type.
2. Sau khung: đẩy message khi đầy phải trả `Busy`/backpressure; hiện không có hàng đợi nên test đỏ.
3. `Q1=A`: 2 hart, X có 2 TCB cùng priority cao nhất, Y idle trong affinity → phải có đúng 1 `Migrate`;
   hiện không có hart/migration ⇒ đỏ.

**DoD đo được**:

1. `Mailbox` cố định **8 message `[TK]`** (số nhỏ, có thể tăng nếu Agent 0 chốt), FIFO; message tối thiểu
   `Wake`, `Reschedule`, `Migrate`, `Stop`; `send` khi đầy trả `Busy` (không ghi đè/mất); `drain` xử lý
   đúng thứ tự, idempotent theo thế hệ/ack nếu có.
2. 1 hart: `Migrate` trả `NotSupported` (hoặc `IllegalOperation` — chốt trong test), không im lặng bỏ;
   `Wake`/`Reschedule`/`Stop` xử lý được; test đầy/backpressure 1.000 lần gửi.
3. **Q1=A**: `Machine::with_harts(2)`; mỗi hart có `HartSched` + `current`; trường công bố `idle: bool`,
   `cur_prio: u8` chỉ **đọc** được từ hart khác (test cấm ghi trực tiếp — mô hình dùng hàm `publish`).
4. Push: X có >1 `Ready` ở mức cao nhất, Y trong affinity `idle` → đẩy **một** TCB (chọn tất định: FIFO
   cũ nhất `[TK]`) sang Y qua `Migrate`; `home` đổi; Y nhận đúng 1 lần; không nhân bản/mất; hart rảnh có
   chỉ số nhỏ nhất được chọn.
5. Không push nếu: Y không idle, Y ngoài affinity, X chỉ có 1 Ready mức cao nhất; **không work stealing**
   (Y không tự kéo); test âm cho cả ba.
6. Hộp thư xử lý ngay cuối bước (mô hình tất định) ⇒ không có trạng thái trung gian quan sát được; nếu
   cần mô phỏng in-flight, phải thêm cờ và invariant tương ứng — **K1b không làm**, ghi rõ.
7. 1.000 seed × 64 bước: không TCB nào chạy 2 hart cùng lúc; tổng TCB trong queue/current = tổng TCB sống;
   cùng seed cùng trace.
8. Không `unsafe`, không thread, không loom; `v2 miri` test mailbox PASS.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/mailbox.rs` (mới ~150 dòng), `sched.rs` (2 hart nếu Q1=A),
`tests/kt22_mailbox.rs` (và `kt22_migrate.rs` nếu Q1=A).

**Rủi ro**: (a) 3 h là sát nếu làm cả 2 hart ⇒ được phép tách `KT-22a` (mailbox, 2 h) / `KT-22b`
(migration, 3 h) **sau khi Agent 0 đồng ý**, không tự tách; (b) bất biến "Ready iff in queue" vỡ lúc
migrate nếu mô hình không nguyên tử — dùng xử lý cuối bước; (c) đừng tuyên bố "lock-free" cho mô hình này.

**BLOCKED khi**: Q1 chưa chốt (1 hart hay 2-hart) ⇒ chỉ làm phần mailbox 1 hart, không làm migration;
Q6 chưa chốt ⇒ không tạo `vifast-sync`/atomic.

---

### KT-23 — Tích hợp scenario, coverage, Miri, parity, chốt DoD K1b

**Mục tiêu**: nối các mảnh vào bộ chạy kịch bản/l1, chứng minh 4 tiêu chí K1b bằng log, không nới cổng.

**Test đỏ trước**:

1. `scenario::Step::TcbSuspend/TcbResume/SchedTick/SchedPick` chưa có ⇒ test/`match` không biên dịch
   (E0599/E0004) — đỏ trước khi thêm.
2. `Machine::k1b_report()` (coverage/queue/budget counters) chưa có ⇒ test gọi **E0599**.
3. Chạy thử 1 kịch bản suspend + donation trên `run_interleaved` hiện tại: interleaver chỉ chọn theo
   `TcbState`, **không** theo scheduler/SC ⇒ trace không phản ánh thứ tự chạy thật (đỏ về ngữ nghĩa).

**DoD đo được**:

1. `Step` mới: `TcbSuspend`, `TcbResume`, `TcbReadState`, `SchedTick`, `SchedPick` (và `SchedDonate` nếu
   cần); `execute` gọi đúng API; `run_interleaved` tôn trọng `Running`/queue + SC budget; cùng seed ⇒ cùng trace.
2. `tests/kt23_k1b.rs`: **1.000 seed × 64 bước** sinh từ LCG, trộn retype/call/recv/reply/suspend/resume/
   tick/exit; sau **mỗi** bước kiểm `check_sched`, `check_budget`, `check_sc_refs`, `check_all_bytes`;
   0 vi phạm; in `transitions hit/allowed`, `forbidden_attempts`, `budget_violations`.
3. Coverage: `hit == allowed` (100%); mọi cặp cấm thử ít nhất 1 lần trả `IllegalOperation`;
   `forbidden_attempts > 0`; nếu thiếu cặp ⇒ test đỏ, **không** hạ tiêu chí.
4. Miri: `v2 miri -p vifast-sim --test kt18_tcb --test kt19_sched --test kt20_mcs --test kt21_donation
   [--test kt22_mailbox]` PASS; `v2 nightly --only-miri` cập nhật danh sách này (không thêm test I/O).
5. Parity: `cargo test -p vifast-model --test kt11_diff_sim` 10.000 seed / 0 lệch vẫn xanh; ghi rõ
   **parity chỉ phủ Untyped/CNode**, không phải bằng chứng TCB/scheduler (Q7).
6. `gate --fast` PASS **không retry**; L1 1.000 seed PASS; gen-check/`scn` 21 tệp không đổi; test count
   không giảm so với baseline 231 test/41 suite (STATE ghi).
7. `STATE.md` có bảng **"K1b DoD"** 4 dòng (transition 100%, budget, mailbox, push migration nếu Q1=A)
   kèm log; dòng nào chưa đạt ghi rõ **chưa đạt**, không tô hồng.
8. Không sửa `.scn`, `vifast-model`, `spec/`; `git diff --stat v2/test/scn v2/lib/vifast-model` rỗng.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/scenario.rs`, `tests/kt23_k1b.rs`, `v2/bin/v2` (danh sách
`nightly --only-miri`), `v2/agent/{STATE.md,JOURNAL/}`.

**Rủi ro**: (a) L1 mới + 1.000 seed có thể làm `gate --fast` > 60 s ⇒ đo ngay, giảm **số op/seed của test
mới** (ghi lý do), **không** giảm invariant; (b) Miri chạy cả kt21/22 có thể chậm ⇒ chọn tập con có lý do,
không bỏ objmem; (c) sửa `scenario.rs` làm vỡ kt09_preempt — chạy toàn bộ test cũ trước khi commit;
(d) đừng biến `k1b_report` thành API sản phẩm — chỉ test/harness.

**BLOCKED khi**: Q1/Q4/Q7 chưa chốt làm DoD không xác định; Miri phát hiện UB thật ⇒ **revert trước**
(D2-03), mở test tái hiện, rồi điều tra.

---

## 6. Quyết định Agent 0 phải chốt **trước khi mở task**

| # | Câu hỏi | Đề xuất mặc định | Chặn gì |
|---|---|---|---|
| **Q1** | K1b giữ **scheduler 1 hart** (theo brief) và **hoãn push migration sang K1c** (nơi đã yêu cầu `smp 1/2/4/8`), hay mở **mô hình 2 hart tất định** ngay trong K1b để đạt chữ "push migration" của thiết kế `:874`? | **1 hart + hoãn push migration**; ghi một **D2 mới** sửa tiêu chí K1b (hoặc chấp nhận K1b "đạt 3/4 tiêu chí, push migration chuyển K1c"). Nếu Agent 0 muốn đủ chữ: chọn 2-hart logic, KT-22 thành 3 h, **không** dùng thread/SMP thật | **KT-22**, kết luận K1b DONE |
| **Q2** | Đường **gắn `SchedContext` vào TCB** hiện không có: `SchedContext.Bind` chỉ nhận Reply/Notification (`kobj.rs:1344`, `spec/abi.toml:218-221`), `Tcb.Configure` không có tham số `sched`, `Tcb.SetSchedParams` chưa có chữ ký. Chọn: (a) model API `set_sched_params(tcb, sc)` `[TK]`, không đụng spec; (b) duyệt chữ ký `Tcb.SetSchedParams(tcb, sched)` + D2; (c) cho `SchedContext.Bind` nhận TCB (sửa note spec)? | **(a)** cho K1b: model API nội bộ, ABI binding để K1c/K3; nếu muốn ABI ngay thì **(b)** — gần seL4 nhất, method id 7 đã có | **KT-20, KT-21** |
| **Q3** | Donation: **tự động** (Call biết SC của caller, gắn vào Reply; server passive chạy bằng SC client; Reply trả về) hay **qua `SchedContext.Bind`** (server bind SC vào Reply trước khi trả)? Server **có SC riêng** thì giữ SC riêng hay vẫn nhận donation? Các method `Tcb.SetPriority/SetMcp/SetAffinity`, `SchedContext.Consumed/YieldTo` chưa có chữ ký — K1b có được cài **model API** không? | **Tự động** (đúng seL4 MCS, khớp `:293`/`:320`); server có SC riêng giữ SC riêng; K1b cài model API cho các method chưa có chữ ký, **không** tuyên bố ABI; `SetTimeoutEp` hoãn theo QD §3.4 | **KT-20, KT-21** |
| **Q4** | Bảng `ALLOWED` 11×11 đặt ở đâu: (a) `spec/abi.toml` + generator (Agent 0 ghi, KT-18 regenerate), (b) `v2/docs/k1b-tcb-transition.md` model-only `[TK]` + code, (c) chỉ trong test? | **(b)** để K1b tự chứa, không sửa spec; Agent 0 review bảng + ký khi K3 conformance. Nếu muốn normative ngay: **(a)** | **KT-18** (DoD coverage) |
| **Q5** | "Tổng thời gian chạy trong **mọi cửa sổ `period`** ≤ budget" (`:296`) không đúng với "nạp lại đầu mỗi chu kỳ" (`:302`) — cửa sổ **trượt** có thể thấy 2×budget ở biên. K1b kiểm **chu kỳ thẳng hàng** hay yêu cầu cửa sổ trượt? | **Chu kỳ thẳng hàng** cho K1b; ghi rõ cửa sổ trượt + refill sporadic là phần hoãn sau K6 (D2-16). Nếu Agent 0 yêu cầu trượt: phải làm refill nhiều đoạn **ngay**, trái D2-16 ⇒ cần D2 mới | **KT-20** |
| **Q6** | Xác nhận K1b chỉ làm **MCS donation**, **không** làm **MCS lock**/hộp thư lock-free thật; hộp thư K1b là **mô hình tất định** (không atomic/thread/loom); `vifast-sync` + stress để K1c/SMP? | **Đúng** — không có loom (đo §8), sim 1 luồng; làm lock-free bây giờ là test flaky/không kiểm được | **KT-22**, phạm vi K1b |
| **Q7** | Có mở rộng `vifast-model` để parity TCB/scheduler không? Hiện model chỉ Untyped/CNode; `kt11_diff_sim` băm metadata, không băm state. | **Không** trong K1b; parity 10k giữ làm **hồi quy**, K1b chứng minh bằng invariant/coverage. Nếu cần parity ngữ nghĩa: task riêng (chưa cấp ID) >3 h + D2, tách khỏi 6 task đề xuất | **KT-23** (cách tuyên bố bằng chứng) |
| **Q8** | Preempt và budget trong K1b dùng **tick ảo** của sim, **không** cần timer phần cứng/QEMU — xác nhận? | **Đúng**; timer heap chỉ để deadline refill; timer thật thuộc K3. Nếu Agent 0 muốn timer HW ngay: ngoài phạm vi K1b | KT-19/20 (nhẹ) |

> Trả lời bằng chat **không** mở task; cần D2 mới trong `DECISIONS.md` + dòng `APPROVED` trong QUEUE
> (AGENTS §8). Planner không tự chốt thay.

---

## 7. Thứ tự, cổng, bằng chứng

```
KT-18 (TCB/transition/suspended) → KT-19 (scheduler 1 hart) → KT-20 (budget) → KT-21 (donation)
       3 h                              3 h                        3 h               3 h
   → KT-22 (mailbox [+ 2-hart nếu Q1=A]) → KT-23 (tích hợp/coverage/Miri/parity/DoD)
              2 h (3 h)                              2 h
```

- Mỗi task: `./v2/bin/v2 check` sau từng bước nhỏ → test đỏ dán `JOURNAL/ktNN-red.log` → cài →
  `./v2/bin/v2 gate --fast` (PASS, **không retry**) → `./v2/bin/v2 miri` trên test mới → commit
  `[KT-NN] <tóm tắt>` → `STATE.md` + `JOURNAL/` (người thi hành, không phải planner).
- K1b **không** chạy `nightly` đầy đủ/L1 100k/mutation/soak (để K1c); chỉ `nightly --only-miri` ở KT-23.
- K1b **không** chạy QEMU; mọi thời gian là tick ảo tất định.
- Không task nào chạm `spec/`, `v2/lib/vifast-model`, `v2/test/scn/*.scn`, `viscript/`; nếu Q2/Q3/Q4 chọn
  phương án cần spec thì **Agent 0** làm trước, task chỉ tiêu thụ.
- Bảng "K1b DoD" cuối phase: `transitions 100%`, `budget 0 vi phạm (chu kỳ thẳng hàng)`, `mailbox
  đầy/backpressure`, `push migration` (đạt nếu Q1=A; **hoãn có D2** nếu Q1=B), kèm log — không viết số
  đo tay vào tài liệu (QD §9 Q12).

---

## 8. Bằng chứng lệnh đã chạy (read-only)

```text
$ git rev-parse --short HEAD
ab87bb9
$ git show -s --format='%h %s' HEAD
ab87bb9 [KT-17] nightly --only-miri PASS 365 s, đóng 2 follow-up căn lề, DoD K1a' 2 dòng
$ git status --porcelain=v2 | head -3 | awk '{print $2, $NF}'
.M viscript/Cargo.lock
.M viscript/Cargo.toml
.M viscript/agent/JOURNAL/2026-11-12.md
$ git status --porcelain=v2 | grep -c 'review/agent0/K1B-PLAN.md'
1
$ wc -l v2/sim/vifast-sim/src/kobj.rs
2103 v2/sim/vifast-sim/src/kobj.rs
$ grep -n "fn set_tcb_state" v2/sim/vifast-sim/src/kobj.rs
1711:    fn set_tcb_state(&mut self, tcb: ObjId, state: TcbState) -> Result<(), KError> {
$ grep -n "BlockedOnRecv" v2/sim/vifast-sim/src/kobj.rs
283:    BlockedOnRecv {
$ grep -n "fn sched_control_configure\|fn sched_context_bind\|fn tcb_read_state" v2/sim/vifast-sim/src/kobj.rs
1322:    pub fn sched_control_configure(
1344:    pub fn sched_context_bind(&mut self, sched: SlotRef, target: SlotRef) -> Result<(), KError> {
1369:    pub fn tcb_read_state(&self, tcb: SlotRef) -> Result<TcbState, KError> {
$ grep -c 'method."Tcb.SetSchedParams"' v2/docs/abi-call-signatures.toml
0
$ grep -n 'SetSchedParams' spec/abi.toml | cut -c1-100
143:Tcb = { id = 3, stability = "frozen", method = { 0 = { name = "Configure" }, 1 = { name = "SetFa
$ export PATH="$HOME/.cargo/bin:$PATH"; rustup component list --installed --toolchain nightly | grep -E 'miri|rust-src'
miri-aarch64-apple-darwin
rust-src
$ cargo +nightly miri --version
miri 0.1.0 (3ef0c22cdb 2026-11-11)
$ ls ~/.cargo/registry/src/*/ | grep -i '^loom'
(không có kết quả)
```

Không chạy `cargo test`/`gate` toàn bộ trong lúc lập kế hoạch (tránh chen với dòng ViScript đang chạy);
baseline lấy từ `STATE.md` (231 test/41 suite, L1 1.000 seed/999 trace, gate 517 ms) và sẽ được tái xác
nhận ở KT-23.

---

## 9. Ngoài phạm vi K1b (không đề xuất, không làm)

- **K1c**: IPC đầy đủ (Reply object/cap, `no_reply`, 4 cap, unwrap badge, ReplyRecv, fault VM/unknown/user
  exception), Notification signal/wait, WaitOnAddress, `Exit` theo procsrv, `smp 1/2/4/8`, 100.000 seed.
- **K2/K3**: VSpace/Frame/ASID/shootdown, QEMU, conformance, perf.
- **MCS lock** (queue lock S1) và **hộp thư lock-free thật** + `vifast-sync`: cần SMP/loom/stress; K1b
  không có (Q6).
- Refill sporadic nhiều đoạn, timeout fault/`timeout_ep`, phân tích độ trễ audio: hoãn sau K6 (D2-16).
- Mở rộng `vifast-model` để parity TCB/scheduler (Q7); nếu mở là task riêng >3 h.
- Bất kỳ thay đổi `spec/`, ABI, `.scn`, QUEUE/DECISIONS trong K1b — trừ khi Agent 0 chọn Q2/Q3/Q4 phương án
  cần spec và tự thực hiện trước.
