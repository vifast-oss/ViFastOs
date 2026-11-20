# KT-237 — Inventory cửa ghi trạng thái TCB + đề xuất kiểu đích loại trừ `Ready`

- **Người làm**: sub-agent A (Agent 1), luồng A theo **D2-192** (D2-187 hướng, D2-193 registry).
- **Worktree**: `/private/tmp/night4-A-kt237`, nhánh `night4/A-kt237`, base `298b92c5`.
- **Ngân sách**: 1,5 giờ. **Trạng thái**: xong **inventory + đề xuất**; **KHÔNG sửa mã**
  (chỉ đọc + ghi journal; prototype đo compile-fail là tệp tạm, **đã hoàn nguyên** —
  `git status` sạch trước khi commit).
- **Kết luận một dòng**: mã sản phẩm có **62** call-site `transition_tcb` (17 chuyển `Ready`,
  43 không, 2 qua tham số) + **154** call-site test; đề xuất `NonReadyTarget` (10 biến thể,
  **không có `Ready`**) làm kiểu đích của `transition_tcb`, **5 cửa `Ready` có ngữ nghĩa**, và
  primitive thô `write_state` **private** trong module `state_write`; **WB-16 và WB-17 bị chặn
  ở biên dịch** (`E0308`/`E0631`), primitive thô bị chặn từ module ngoài (**`E0624`**).

---

## 0. TL;DR

| Câu hỏi | Trả lời |
|---|---|
| WB-16 chặn được? | **CÓ** — 6/6 dạng `E0308` (helper cùng tệp, helper khác tệp, hàm trả về, `const`, `static`, closure) |
| WB-17 chặn được? | **CÓ** — 7/7 dạng `E0308`/`E0631` (fn-pointer, fn-pointer có kiểu, truyền qua tham số, generic `F: Fn`, trait object, UFCS, `<Machine>::…`) |
| Primitive thô | `fn write_state(&mut self, tcb, to: TcbState)` **private** trong module `state_write`; module sản phẩm ngoài gọi ⇒ **`E0624`** (đo trên cả core và sim) |
| Chi phí | **62** call-site sản phẩm + **154** test + **13** `block_current`; **2** module mới; đổi `pub` API (`transition_tcb`, `block_current`; thêm `start_tcb`, `tcb_yield`) |
| Đổi `spec/`? | **KHÔNG** — `NonReadyTarget` là model-only `[TK]`, không phải ABI; `TcbState`/layout không đổi |
| Chặn thật sự | **checker `check-wake-paths.py` + `check-wake-depinfo.py` phải sửa cùng task** — nếu không `v2 check` đỏ (xem §4.4) |

---

## 1. Inventory cửa ghi trạng thái TCB

### 1.1 Định nghĩa + mức công khai

| Cửa | Tệp (core) | Mức | Tệp (sim) | Mức |
|---|---|---|---|---|
| `Machine::transition_tcb(tcb, to: TcbState)` | `v2/kernel/vifast-core/src/core_tcb.rs:389` | **`pub`** | `v2/sim/vifast-sim/src/kobj.rs:5079` | **`pub`** |
| `Machine::wake_step(tcb, unlink)` | `core_sched.rs:296` | `pub(crate)` | `kobj.rs:5783` | private (`kobj`) |
| `Machine::wake_from(tcb)` | `core_sched.rs:267` | `pub(crate)` | `kobj.rs:5752` | private (`kobj`) |
| `Machine::wake_from_unlink(tcb)` | `core_sched.rs:282` | `pub(crate)` | `kobj.rs:5766` | private (`kobj`) |
| `Machine::wake(tcb)` | `core_sched.rs:246` | **`pub`** (test-only theo checker) | `kobj.rs:5731` | **`pub`** |
| `Machine::apply_mail(hart, msg)` | `core_sched.rs:647` | private (`core_sched`) | `kobj.rs:5981` | private (`kobj`) |
| `Machine::wake_addr(frame, offset, n)` | `core_sync.rs:544` | **`pub`** | `kobj.rs:8697` | **`pub`** |
| `Machine::block_current(hart, to: TcbState)` | `core_sched.rs:190` | **`pub`** | `kobj.rs:5667` | **`pub`** |
| `Machine::post_mail(hart, MailboxMsg)` | `core_sched.rs:507` | **`pub`** | `kobj.rs:5830` | **`pub`** |
| `Machine::process_mail` / `send_mail` | `core_sched.rs:566/576` | **`pub`** | `kobj.rs:5922/5934` | **`pub`** |

**Primitive ghi trạng thái thô** = đúng **hai** chỗ gán `state` trong toàn bộ `v2/kernel/**` +
`v2/sim/**` (đã grep `\.state\s*=`):

```rust
// v2/kernel/vifast-core/src/core_tcb.rs:440
self.tcb_data_mut(tcb)?.state = to;
// v2/sim/vifast-sim/src/kobj.rs:5138
t.state = to;
```

Cả hai nằm **trong thân `transition_tcb`**; không có đường gán `state` nào khác. `tcb_configure`
**không** đổi `state` (chỉ `cspace`/`exit_code`). Vậy "primitive thô" hiện tại **chính là**
`transition_tcb` — nó vừa là cửa công khai, vừa là chỗ ghi thô, vừa nhận `TcbState` đầy đủ
(gồm `Ready`). Đó là gốc của WB-16/WB-17.

### 1.2 `MailboxMsg::Wake` — producer/consumer

| Vai trò | Core | Sim |
|---|---|---|
| Producer (chéo hart) | `core_sched.rs:248` (`wake`), `:272` (`wake_from`), `:287` (`wake_from_unlink`) | `kobj.rs:5735`, `:5757`, `:5771` |
| Consumer | `core_sched.rs:649` (`apply_mail` → `wake_step(tcb, true)`) | `kobj.rs:5983` |
| Định nghĩa/biến thể | `mailbox.rs:85/96` | `mailbox.rs:289` |
| Test | `mailbox.rs:411/435` | `mailbox.rs:289` |

Không có `Msg::Wake` trong `v2/kernel/**`/`v2/sim/**` (chỉ `MailboxMsg::Wake`).

### 1.3 Bảng call-site **sản phẩm** — 62 dòng

Phân loại: **17** chuyển `Ready` (cột "Ready? = CÓ"), **43** không, **2** truyền tham số
(`block_current`). Cột "sau khi đổi" là cửa/kiểu đề xuất (§2, §3).

| # | tệp:dòng | hàm chứa | ngữ nghĩa | Ready? | sau khi đổi |
|---|---|---|---|---|---|
| 1 | `v2/kernel/vifast-core/src/core_ipc.rs:192` | `ensure_started` | khởi tạo/start | CÓ | `start_tcb` |
| 2 | `v2/kernel/vifast-core/src/core_ipc.rs:322` | `deliver_queued` | exit/kết thúc | KHÔNG | `NonReadyTarget::Inactive` |
| 3 | `v2/kernel/vifast-core/src/core_ipc.rs:327` | `deliver_queued` | chặn reply | KHÔNG | `NonReadyTarget::BlockedOnReply` |
| 4 | `v2/kernel/vifast-core/src/core_ipc.rs:342` | `deliver_queued` | chặn reply | KHÔNG | `NonReadyTarget::BlockedOnReply` |
| 5 | `v2/kernel/vifast-core/src/core_ipc.rs:425` | `call_msg` | chặn reply | KHÔNG | `NonReadyTarget::BlockedOnReply` |
| 6 | `v2/kernel/vifast-core/src/core_ipc.rs:448` | `call_msg` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 7 | `v2/kernel/vifast-core/src/core_ipc.rs:533` | `endpoint_send` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 8 | `v2/kernel/vifast-core/src/core_ipc.rs:597` | `recv` | chặn nhận | KHÔNG | `NonReadyTarget::BlockedOnRecv` |
| 9 | `v2/kernel/vifast-core/src/core_ipc.rs:700` | `reply_with` | yield (replier) | CÓ | `yield_to_ready` |
| 10 | `v2/kernel/vifast-core/src/core_ipc.rs:807` | `exit` | exit/kết thúc | KHÔNG | `NonReadyTarget::Inactive` |
| 11 | `v2/kernel/vifast-core/src/core_ipc.rs:823` | `exit` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 12 | `v2/kernel/vifast-core/src/core_ipc.rs:838` | `exit` | exit/kết thúc | KHÔNG | `NonReadyTarget::Inactive` |
| 13 | `v2/kernel/vifast-core/src/core_ipc.rs:868` | `inject_fault` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 14 | `v2/kernel/vifast-core/src/core_ipc.rs:903` | `inject_fault` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 15 | `v2/kernel/vifast-core/src/core_ipc.rs:1148` | `release_donation_of` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 16 | `v2/kernel/vifast-core/src/core_sched.rs:171` | `dispatch_step` | preemption | CÓ | `preempt_current` |
| 17 | `v2/kernel/vifast-core/src/core_sched.rs:184` | `dispatch_step` | dispatch (vào chạy) | KHÔNG | `NonReadyTarget::Running` |
| 18 | `v2/kernel/vifast-core/src/core_sched.rs:211` | `block_current` | block_current (theo tham số) | KHÔNG | `NonReadyTarget (tham số)` |
| 19 | `v2/kernel/vifast-core/src/core_sched.rs:307` | `wake_step` | wake | CÓ | `(giữ) wake_step` |
| 20 | `v2/kernel/vifast-core/src/core_sched.rs:318` | `yield_current` | yield | CÓ | `yield_to_ready` |
| 21 | `v2/kernel/vifast-core/src/core_sync.rs:121` | `notification_wait` | chặn notification | KHÔNG | `NonReadyTarget::BlockedOnNotification` |
| 22 | `v2/kernel/vifast-core/src/core_sync.rs:509` | `wait_addr_steps` | chặn địa chỉ | KHÔNG | `NonReadyTarget::BlockedOnAddress` |
| 23 | `v2/kernel/vifast-core/src/core_tcb.rs:436` | `transition_tcb` | preemption (vào Running) | CÓ | `preempt_current` |
| 24 | `v2/kernel/vifast-core/src/core_tcb.rs:508` | `tcb_suspend` | suspend/abort | CÓ | `suspend_to_ready` |
| 25 | `v2/kernel/vifast-core/src/core_tcb.rs:620` | `finalise_tcb` | finalise | KHÔNG | `NonReadyTarget::Dead` |
| 26 | `v2/kernel/vifast-core/src/core_tcb.rs:666` | `charge_tick` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 27 | `v2/kernel/vifast-core/src/core_tcb.rs:675` | `charge_tick` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 28 | `v2/kernel/vifast-core/src/core_tcb.rs:685` | `charge_tick` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 29 | `v2/kernel/vifast-core/src/core_tcb.rs:754` | `enforce_budget` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 30 | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:200` | `build_rig` | khởi tạo/start | CÓ | `start_tcb` |
| 31 | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:314` | `turn` | dispatch (vào chạy) | KHÔNG | `NonReadyTarget::Running` |
| 32 | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:325` | `turn` | yield (U-mode) | CÓ | `tcb_yield` |
| 33 | `v2/kernel/vifast-plat-riscv64/src/core_bridge.rs:270` | `boot_smoke` | khởi tạo/start | CÓ | `start_tcb` |
| 34 | `v2/sim/vifast-sim/src/kobj.rs:5132` | `transition_tcb` | preemption (vào Running) | CÓ | `preempt_current` |
| 35 | `v2/sim/vifast-sim/src/kobj.rs:5218` | `tcb_suspend` | suspend/abort | CÓ | `suspend_to_ready` |
| 36 | `v2/sim/vifast-sim/src/kobj.rs:5428` | `finalise_tcb` | finalise | KHÔNG | `NonReadyTarget::Dead` |
| 37 | `v2/sim/vifast-sim/src/kobj.rs:5640` | `dispatch_step` | preemption | CÓ | `preempt_current` |
| 38 | `v2/sim/vifast-sim/src/kobj.rs:5654` | `dispatch_step` | dispatch (vào chạy) | KHÔNG | `NonReadyTarget::Running` |
| 39 | `v2/sim/vifast-sim/src/kobj.rs:5691` | `block_current` | block_current (theo tham số) | KHÔNG | `NonReadyTarget (tham số)` |
| 40 | `v2/sim/vifast-sim/src/kobj.rs:5794` | `wake_step` | wake | CÓ | `(giữ) wake_step` |
| 41 | `v2/sim/vifast-sim/src/kobj.rs:5807` | `yield_current` | yield | CÓ | `yield_to_ready` |
| 42 | `v2/sim/vifast-sim/src/kobj.rs:8022` | `charge_tick` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 43 | `v2/sim/vifast-sim/src/kobj.rs:8038` | `charge_tick` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 44 | `v2/sim/vifast-sim/src/kobj.rs:8052` | `charge_tick` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 45 | `v2/sim/vifast-sim/src/kobj.rs:8128` | `enforce_budget` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 46 | `v2/sim/vifast-sim/src/kobj.rs:8520` | `release_donation_of` | ngủ ngân sách | KHÔNG | `NonReadyTarget::Sleeping` |
| 47 | `v2/sim/vifast-sim/src/kobj.rs:8590` | `notification_wait` | chặn notification | KHÔNG | `NonReadyTarget::BlockedOnNotification` |
| 48 | `v2/sim/vifast-sim/src/kobj.rs:8670` | `wait_addr` | chặn địa chỉ | KHÔNG | `NonReadyTarget::BlockedOnAddress` |
| 49 | `v2/sim/vifast-sim/src/kobj.rs:9329` | `deliver_queued` | exit/kết thúc | KHÔNG | `NonReadyTarget::Inactive` |
| 50 | `v2/sim/vifast-sim/src/kobj.rs:9338` | `deliver_queued` | chặn reply | KHÔNG | `NonReadyTarget::BlockedOnReply` |
| 51 | `v2/sim/vifast-sim/src/kobj.rs:9353` | `deliver_queued` | chặn reply | KHÔNG | `NonReadyTarget::BlockedOnReply` |
| 52 | `v2/sim/vifast-sim/src/kobj.rs:9438` | `call_msg` | chặn reply | KHÔNG | `NonReadyTarget::BlockedOnReply` |
| 53 | `v2/sim/vifast-sim/src/kobj.rs:9463` | `call_msg` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 54 | `v2/sim/vifast-sim/src/kobj.rs:9579` | `endpoint_send` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 55 | `v2/sim/vifast-sim/src/kobj.rs:9658` | `recv` | chặn nhận | KHÔNG | `NonReadyTarget::BlockedOnRecv` |
| 56 | `v2/sim/vifast-sim/src/kobj.rs:9791` | `reply_with` | yield (replier) | CÓ | `yield_to_ready` |
| 57 | `v2/sim/vifast-sim/src/kobj.rs:9974` | `exit` | exit/kết thúc | KHÔNG | `NonReadyTarget::Inactive` |
| 58 | `v2/sim/vifast-sim/src/kobj.rs:9998` | `exit` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 59 | `v2/sim/vifast-sim/src/kobj.rs:10013` | `exit` | exit/kết thúc | KHÔNG | `NonReadyTarget::Inactive` |
| 60 | `v2/sim/vifast-sim/src/kobj.rs:10070` | `inject_fault` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 61 | `v2/sim/vifast-sim/src/kobj.rs:10108` | `inject_fault` | chặn gửi | KHÔNG | `NonReadyTarget::BlockedOnSend` |
| 62 | `v2/sim/vifast-sim/src/kobj.rs:10368` | `ensure_started` | khởi tạo/start | CÓ | `start_tcb` |

Tổng: 62

**Đọc bảng**: 17 dòng `Ready? = CÓ` khớp **đúng** 15 mục `NON_WAKE_READY_SITES` hiện có
(`v2/tools/check-wake-paths.py:240`) + `wake_step` (đã là `WAKE_DOORS`). Không có dòng nào
"không phân loại được": 17 dòng chia hết vào **5 ngữ nghĩa** (§2.4).

### 1.4 Call-site **test** — 154 dòng (chỉ liệt kê tổng hợp)

| Nhóm | Số call-site | Số tệp |
|---|---|---|
| `v2/sim/vifast-sim/tests/*.rs` | 106 | 33 |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_*.rs` + `proc_demo.rs` | 35 | 13 |
| `#[cfg(test)] mod tests` trong tệp sản phẩm (`core_tcb.rs`, `kobj.rs`) | 11 | 2 |
| `v2/kernel/vifast-core/src/tls_tests.rs` | 1 | 1 |
| `v2/kernel/vifast-plat-common/tests/user.rs` | 1 | 1 |
| **Tổng** | **154** | **50** |

Trong 154: **96** chuyển `Ready` (gồm 6 dạng path-qualified `kc::TcbState::Ready`,
`vs::kobj::TcbState::Ready`), **33** không, **14** truyền biến/`state_for(...)`/`st.sim()`.
Ngoài ra **13** call-site `block_current` (toàn bộ trong `v2/sim/vifast-sim/tests/`), trong đó
`kt19_sched.rs:702` **cố ý** truyền `TcbState::Ready` để kiểm `InvalidArgument` — dưới API mới
phép thử này **không còn biểu diễn được** (xem §5.1).

### 1.5 Các cửa `wake_*` — call-site (không đổi chữ ký)

| Cửa | Sản phẩm | Test | Ghi chú |
|---|---|---|---|
| `wake_from` | **28** (15 core + 13 sim) | 0 | giữ nguyên chữ ký; chỉ đổi module chứa (§3) |
| `wake_from_unlink` | **2** (`core_sync.rs:561`, `kobj.rs:8713`) | 0 | giữ nguyên |
| `wake_step` | **8** (4 core + 4 sim) | 0 | giữ nguyên; thành `pub(crate)`/`pub(super)` |
| `apply_mail` | **2** | 0 | giữ nguyên |
| `wake_addr` | **1** (`syscall.rs:624`) | 10 | giữ nguyên chữ ký |
| `wake` | **0** | 56 | test-only; giữ nguyên |

---

## 2. Đề xuất kiểu đích chuyển trạng thái **loại trừ `Ready`**

### 2.1 Chữ ký cụ thể

```rust
// v2/kernel/vifast-core/src/state_write.rs  (mới; sim: kobj/state_write.rs)
/// Đích chuyển trạng thái **không bao giờ là `Ready`** — 10 biến thể còn lại của `TcbState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonReadyTarget {
    Inactive,
    Running { hart: u16 },
    BlockedOnSend { ep: ObjId, call: bool, fault: Option<FaultKind> },
    BlockedOnRecv { ep: ObjId, reply: Option<ObjId> },
    BlockedOnReply { reply: ObjId },
    BlockedOnNotification { ntfn: ObjId },
    BlockedOnAddress { frame: ObjId, offset: u32 },
    Sleeping { reason: SleepReason },
    Stopping,
    Dead,
}

impl NonReadyTarget {
    /// **private** — chỉ module này đổi được sang `TcbState`.
    fn into_state(self) -> TcbState { /* match 10 biến thể */ }
}

impl Machine {
    /// Cửa công khai duy nhất cho chuyển trạng thái **không-Ready**.
    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError> {
        self.write_state(tcb, to.into_state())
    }

    /// **Nguyên thủy thô** — private to `state_write`; KHÔNG `pub`, KHÔNG `pub(crate)`.
    fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> { /* thân cũ */ }
}
```

**Không** có `impl From<TcbState> for NonReadyTarget`, **không** `TryFrom`, **không**
`From<NonReadyTarget> for TcbState` công khai (chỉ `into_state` private). `NonReadyTarget`
**không** `#[non_exhaustive]` (crate ngoài phải dựng được mọi biến thể).

### 2.2 Vì sao **WB-16** không biểu diễn được

`Ready` **không phải** giá trị của `NonReadyTarget` — không có biến thể nào mang `TcbState`,
không có hàm/`const`/`From` công khai nào sinh ra `NonReadyTarget` từ `TcbState`. Do đó mọi
dạng "mang `Ready` tới cửa" đều **sai kiểu** ở đúng đối số thứ hai:

| Dạng WB-16 | Vì sao chết | Mã lỗi đo được |
|---|---|---|
| f1 helper cùng tệp `fn helper(..., s: TcbState)` | `s: TcbState` ≠ `NonReadyTarget` | `E0308` |
| f2 helper **tệp khác** | như f1, biên dịch chéo tệp không đổi kiểu | `E0308` |
| f3 `fn ready_state() -> TcbState` | giá trị trả về `TcbState` | `E0308` |
| f4 `const R: TcbState = TcbState::Ready` | hằng có kiểu `TcbState` | `E0308` |
| f5 `static S: TcbState = TcbState::Ready` | như f4 | `E0308` |
| f6 closure `\|s: TcbState\| m.transition_tcb(t, s)` | tham số closure `TcbState` | `E0308` |

Không có đường "ép" nào ở mức kiểu: `Ready` chỉ tồn tại như **giá trị của `TcbState`**, mà
`TcbState` không chuyển được thành `NonReadyTarget` (không `From`/`TryFrom` công khai).

### 2.3 Vì sao **WB-17** không biểu diễn được

Chữ ký đích là `fn(&mut Machine, ObjId, NonReadyTarget) -> Result<(), KError>`. Mọi cách
"dùng `transition_tcb` làm giá trị" đều mang **kiểu hàm đó**, nên đối số `TcbState::Ready`
luôn sai kiểu:

| Dạng WB-17 | Vì sao chết | Mã lỗi đo được |
|---|---|---|
| f1 `let tt = Machine::transition_tcb; tt(m,t,Ready)` | `tt` có tham số 3 là `NonReadyTarget` | `E0308` |
| f2 fn-pointer khai kiểu `fn(..., TcbState)` | gán fn item khác kiểu | `E0308` |
| f3 truyền qua `fn call(f: fn(..., TcbState), ...)` | như f2 tại lời gọi | `E0308` |
| f5a generic `F: Fn(&mut Machine, ObjId, TcbState)` | ràng buộc `Fn` không khớp | `E0631` |
| f5b trait object `&dyn Fn(..., TcbState)` | như f5a tại chỗ truyền | `E0631` |
| f6 UFCS `Machine::transition_tcb(m,t,Ready)` | đối số 3 sai kiểu | `E0308` |
| f7 `<Machine>::transition_tcb` | như f1 | `E0308` |

**Lưu ý**: WB-17 dạng 4 (closure chứa lời gọi thẳng) vốn đã bị checker bắt; dưới API mới nó
cũng chết ở `E0308` nếu closure khai `TcbState`, hoặc **không chết** nếu closure khai
`NonReadyTarget` — nhưng khi đó nó chỉ có thể nhận giá trị không-Ready, nên **không phải
bypass**.

### 2.4 Các cửa hợp lệ đưa về `Ready` — **5 ngữ nghĩa**, không ép chung thành wake

| # | Ngữ nghĩa | Cửa đề xuất | Mức | Call-site sản phẩm | Điều kiện `from` |
|---|---|---|---|---|---|
| 1 | **wake** | `wake_step(tcb, unlink)` (giữ tên) | `pub(crate)` core / `pub(super)` sim | `wake_from` (28), `wake_from_unlink` (2), `apply_mail` (2) | `BlockedOn*`/`Sleeping` |
| 2 | **preemption** | `preempt_current(tcb)` | `pub(crate)` / `pub(super)` | `dispatch_step` (2), thân `transition_tcb` khi vào `Running` (2) | `Running{hart}` |
| 3 | **yield** | `yield_to_ready(tcb)` + `pub fn tcb_yield(tcb)` | `pub(crate)` + `pub` | `yield_current` (2), `reply_with` (2), `proc_rt::turn` (1) | `Running{hart}` |
| 4 | **khởi tạo/start** | `pub fn start_tcb(tcb)` | **`pub`** | `ensure_started` (2), `proc_rt::build_rig` (1), `boot_smoke` (1) | `Inactive` |
| 5 | **suspend/abort** | `suspend_to_ready(tcb)` | `pub(crate)` / `pub(super)` | `tcb_suspend` (2) | `BlockedOn*`/`Running` |

Tất cả 5 cửa gọi `write_state(tcb, TcbState::Ready)` — **cửa duy nhất** được phép ghi `Ready`.
Giữ nguyên nghĩa vụ hiện tại: `wake_step` vẫn `unlink` theo cờ; `wake_from` vẫn `push_mail` chéo
hart; `preempt_current` vẫn chỉ gọi khi `sched[hart].current() == Some(cur)`; `start_tcb` chỉ
`Inactive`; `suspend_to_ready` chỉ `BlockedOn*`/`Running` (caller `tcb_suspend` đã kiểm).

**Vì sao `start_tcb`/`tcb_yield` phải `pub`**: `vifast-plat-riscv64` (`core_bridge.rs:270`) và
`vifast-kbin-riscv64` (`proc_rt.rs:200/325`) là **crate ngoài** `vifast-core`; chúng cần cửa
khởi tạo/yield. Đây là **thêm** `pub` API có ngữ nghĩa, không phải mở lại cửa thô.

### 2.5 Bằng chứng compile-fail (prototype tạm, đã hoàn nguyên)

Prototype dựng `state_write.rs` + `NonReadyTarget` + `transition_tcb_nonready` (tên tạm; kiểu
là thứ quyết định, không phải tên) rồi thêm các module "sản phẩm giả" vào `vifast-core`:

```text
vifast-core/src/kt237_evil.rs:6:7: error[E0624]: method `write_state_raw` is private
vifast-core/src/kt237_wb16.rs:6:34:  error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f1)
vifast-core/src/kt237_wb16.rs:18:34: error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f3)
vifast-core/src/kt237_wb16.rs:22:34: error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f4)
vifast-core/src/kt237_wb16.rs:26:34: error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f5)
vifast-core/src/kt237_wb16.rs:29:49: error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f6)
vifast-core/src/kt237_wb16_helper.rs:6:34: error[E0308]: expected `NonReadyTarget`, found `TcbState` (f2)
vifast-core/src/kt237_wb17.rs:7:14:  error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f1)
vifast-core/src/kt237_wb17.rs:11:9:  error[E0308]: expected fn pointer, found fn item        (f2)
vifast-core/src/kt237_wb17.rs:18:10: error[E0308]: expected fn pointer, found fn item        (f3)
vifast-core/src/kt237_wb17.rs:24:9:  error[E0631]: type mismatch in function arguments      (f5a)
vifast-core/src/kt237_wb17.rs:30:44: error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f6)
vifast-core/src/kt237_wb17.rs:34:13: error[E0308]: expected `NonReadyTarget`, found `TcbState`   (f7)
vifast-core/src/kt237_wb17.rs:37:9:  error[E0631]: type mismatch in function arguments      (f5b)
error: could not compile `vifast-core` (lib) due to 14 previous errors
```

Log đầy đủ: [`kt237-logs/proto-core-full.log`](kt237-logs/proto-core-full.log),
[`kt237-logs/proto-core-all.log`](kt237-logs/proto-core-all.log). Nguồn prototype:
[`kt237-logs/proto/`](kt237-logs/proto/).

**Đối chứng dương**: cùng prototype, bỏ các module giả, giữ `state_write` + module gọi đúng
cách (`NonReadyTarget::Inactive`, `start_tcb`) ⇒ `cargo check` **RC=0**
([`kt237-logs/proto-core-ok.log`](kt237-logs/proto-core-ok.log)). Nghĩa là API mới **dùng được**,
không phải "chặn bằng cách làm hỏng biên dịch".

---

## 3. Ranh giới module cho primitive thô

### 3.1 Hiện trạng

- Core: `transition_tcb` là **`pub`** trên `Machine` (`core_tcb.rs:389`), thân chứa dòng ghi
  `state` (`:440`). Mọi module trong crate **và** crate ngoài (`ktest`, `core_bridge`,
  `proc_rt`) gọi được với `TcbState::Ready`.
- Sim: `transition_tcb` **`pub`** (`kobj.rs:5079`), thân ghi `state` (`:5138`). `kobj.rs` là
  **một** module khổng lồ (~11.900 dòng) — một `fn` private trong `kobj` vẫn bị **toàn bộ**
  `kobj.rs` gọi được, không phải ranh giới hẹp.

### 3.2 Đề xuất: module `state_write` — primitive thô **private**

**Core** — `v2/kernel/vifast-core/src/state_write.rs` (mới, `mod state_write;` trong `lib.rs`):

```rust
impl Machine {
    fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> { /* thân cũ */ }
    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError> { … }
    pub(crate) fn wake_step(&mut self, tcb: ObjId, unlink: bool) -> Result<(), KError> { … }
    pub(crate) fn preempt_current(&mut self, tcb: ObjId) -> Result<(), KError> { … }
    pub(crate) fn yield_to_ready(&mut self, tcb: ObjId) -> Result<(), KError> { … }
    pub fn start_tcb(&mut self, tcb: ObjId) -> Result<(), KError> { … }
    pub(crate) fn suspend_to_ready(&mut self, tcb: ObjId) -> Result<(), KError> { … }
}
```

**Sim** — `v2/sim/vifast-sim/src/kobj/state_write.rs` (mới; `kobj.rs` thêm `mod state_write;`).
Đây là **module con** của `kobj`, nên **truy cập được item private của `kobj`** (field
`pair_hits`, `sched`, `budget_sleeps`; method `sched_sync`, `unlink_tcb_queues`,
`release_donation_of`, `reply_set_caller`, `is_donation_donor`, `tcb_data`, `tcb_data_mut`)
**mà không cần nới `pub(crate)` cho bất kỳ field/method nào**. Cửa trong module này dùng
`pub(super)` (chỉ `kobj` gọi được) thay vì `pub(crate)`.

**Vì sao `pub(crate)` không đủ**: nếu `write_state` là `pub(crate)`, **mọi** module trong crate
(`core_ipc`, `core_sync`, `core_finalize`, …) gọi được `write_state(t, TcbState::Ready)` —
đúng đường lọt cũ, chỉ đổi tên. Private trong `state_write` thì **chỉ** thân module đó gọi
được; 5 cửa ngữ nghĩa là **bề mặt duy nhất** còn lại, và mỗi cửa có tên + điều kiện `from`.

### 3.3 Bằng chứng compile-fail — **E0624**

Prototype (đã hoàn nguyên) thêm module sản phẩm giả **anh em** với `state_write`:

```rust
// vifast-core/src/kt237_evil.rs
pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.write_state_raw(t, TcbState::Ready)      // ← private
}
```

```text
error[E0624]: method `write_state_raw` is private
  --> vifast-core/src/kt237_evil.rs:6:7
   |
 6 |     m.write_state_raw(t, TcbState::Ready)
   |       ^^^^^^^^^^^^^^^ private method
   |
  ::: vifast-core/src/state_write.rs:46:5
   |
46 |     fn write_state_raw(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
   |     ----------------------------------------------------------------------------- private method defined here
```

Sim — module giả **anh em** với `kobj` (không phải module con):

```text
error[E0624]: method `write_state_raw` is private
  --> sim/vifast-sim/src/kt237_evil.rs:6:7
   |
 6 |     m.write_state_raw(t, TcbState::Ready)
   |       ^^^^^^^^^^^^^^^ private method
   |
  ::: sim/vifast-sim/src/kobj/state_write.rs:27:5
   |
27 |     fn write_state_raw(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
   |     ----------------------------------------------------------------------------- private method defined here
```

Log: [`kt237-logs/proto-core-full.log`](kt237-logs/proto-core-full.log),
[`kt237-logs/proto-sim-full.log`](kt237-logs/proto-sim-full.log). Đối chứng dương sim:
[`kt237-logs/proto-sim-ok.log`](kt237-logs/proto-sim-ok.log) (RC=0 — module con đọc được field
private của `kobj`, module giả bị chặn).

### 3.4 Tổ chức lại **nhỏ nhất**

| Việc | Core | Sim |
|---|---|---|
| Tệp mới | `src/state_write.rs` (~150 dòng) | `src/kobj/state_write.rs` (~150 dòng) |
| Dòng khai báo | `mod state_write;` + `pub use state_write::NonReadyTarget;` | `mod state_write;` trong `kobj.rs` + `pub use` |
| Chuyển vào | thân `transition_tcb` (`core_tcb.rs:389-455`), `wake_step` (`core_sched.rs:296-308`) | thân `transition_tcb` (`kobj.rs:5079-5160`), `wake_step` (`kobj.rs:5783-5795`) |
| Nới visibility | **không** (mọi helper đã `pub(crate)`) | **không** (module con thấy private của `kobj`) |
| Giữ nguyên chỗ cũ | `wake_from`, `wake_from_unlink`, `wake`, `apply_mail`, `dispatch_step`, `yield_current`, `block_current` | như core |

Đây là **mức nhỏ nhất** đã kiểm: không đổi bố cục `Machine`, không đổi `TcbState`, không đổi
`ALLOWED`, không đổi `sched`/`mailbox`.

---

## 4. Chi phí sửa

### 4.1 Số call-site / tệp

| Nhóm | Số | Ghi chú |
|---|---|---|
| `transition_tcb` sản phẩm — không-Ready | **43** | đổi `TcbState::X` → `NonReadyTarget::X` (cơ học) |
| `transition_tcb` sản phẩm — `Ready` | **17** | đổi sang 1 trong 5 cửa (§2.4) |
| `transition_tcb` sản phẩm — tham số (`block_current`) | **2** | đổi chữ ký `block_current` sang `NonReadyTarget` |
| `transition_tcb` test | **154** | 96 `Ready` → cửa/helper test; 33 không-Ready → `NonReadyTarget`; 14 biến → helper |
| `block_current` test | **13** | đổi sang `NonReadyTarget`; 1 ca `Ready` (kiểm `InvalidArgument`) **thành vô nghĩa** |
| **Tổng call-site phải sửa** | **229** | |
| Tệp sản phẩm phải sửa | **7** | `core_ipc.rs`, `core_sched.rs`, `core_sync.rs`, `core_tcb.rs`, `proc_rt.rs`, `core_bridge.rs`, `kobj.rs` |
| Tệp test phải sửa | **50** | gồm 2 tệp sản phẩm có `#[cfg(test)]`; hợp với 7 tệp sản phẩm = **55 tệp khác nhau** |
| Tệp mới | **2** | `state_write.rs` (core), `kobj/state_write.rs` (sim) |

### 4.2 Đổi `pub` API?

**Có** — nhưng chỉ API **model** `[TK]`, không phải ABI:

| API | Trước | Sau | Ai gọi |
|---|---|---|---|
| `Machine::transition_tcb` | `pub fn(…, TcbState)` | `pub fn(…, NonReadyTarget)` | crate ngoài: `ktest_*`, `core_bridge`, `proc_rt`, sim tests |
| `Machine::block_current` | `pub fn(…, TcbState)` | `pub fn(…, NonReadyTarget)` | chỉ test sim (13) |
| `Machine::start_tcb` | — | **mới** `pub fn(tcb)` | `core_bridge`, `proc_rt`, test |
| `Machine::tcb_yield` | — | **mới** `pub fn(tcb)` | `proc_rt`, test |
| `Machine::wake_step` | `pub(crate)`/private | `pub(crate)`/`pub(super)` | nội bộ |
| `Machine::wake_from`/`wake_from_unlink` | `pub(crate)`/private | giữ nguyên mức | nội bộ |

`TcbState`, `TcbSnapshot`, `TcbInfo`, `ALLOWED`, `TCB_STATE_COUNT` **không đổi**.

### 4.3 Ảnh hưởng `v2/lib`?

**Không**. `v2/lib/**` không dùng `TcbState` (chỉ `vifast-abi::TCB_STATES: [&str; 11]` — tên
chuỗi, không phải kiểu). Không có crate nào trong `v2/lib` gọi `transition_tcb`.

### 4.4 Ảnh hưởng **checker** — chặn thật sự, phải sửa **cùng task**

`v2/bin/v2 check` chạy `check-wake-paths.py` (thường + `--strict`) và
`check-wake-depinfo.py` (`v2/bin/v2:344-361`). Hai checker dùng chung `_transition_calls`
(`check-wake-paths.py:2945`), phân loại đối số thứ hai:

| Đối số mới | `_transition_calls` phân loại | Kết quả |
|---|---|---|
| `NonReadyTarget::Inactive` | `segs = ["NonReadyTarget","Inactive"]` → `unknown` | **ĐỎ** (`E_STRICT_TRANSITION`) cho **43** chỗ |
| `NonReadyTarget::BlockedOnSend{…}` | `unknown` | **ĐỎ** |
| `to` (biến) | `var` → `_var_may_be_ready` | có thể ĐỎ/XANH tuỳ thân hàm |

Ngoài ra:
- `NON_WAKE_READY_SITES` (15 mục) **hết khớp** ⇒ `E_STRICT_STALE` **ĐỎ**.
- `WAKE_DOORS` khoá `(KERNEL_SCHED, "wake_step")` / `(SIM_KOBJ, "wake_step")` **hết khớp** khi
  `wake_step` chuyển sang `state_write.rs` ⇒ phải đổi khoá.
- `check-wake-depinfo.py` `import` lại `_transition_calls` ⇒ cùng sửa.

**Đề xuất cho task hiện thực**: đơn giản hoá checker thay vì vá:
1. `_transition_calls`: mọi lời gọi `transition_tcb(` là **không-Ready theo kiểu** ⇒ không cần
   phân tích đối số thứ hai nữa; **xoá** `_var_may_be_ready` cho `transition_tcb`.
2. Thay `NON_WAKE_READY_SITES` bằng allowlist **cửa `Ready`** (`wake_step`, `preempt_current`,
   `yield_to_ready`, `start_tcb`, `suspend_to_ready`) theo `(tệp, hàm, thứ tự)`.
3. Thêm luật mới: `write_state`/`write_state_raw` **không** được xuất hiện ngoài `state_write.rs`
   (fail-closed) — đây là luật thay thế trực tiếp cho `E_STRICT_TRANSITION`.
4. `WAKE_DOORS` đổi khoá sang tệp `state_write.rs`.

Đây là **điều kiện tiên quyết**: nếu không sửa checker cùng lúc, `v2 check` đỏ toàn cây.

---

## 5. Rủi ro + parity

### 5.1 Ngữ nghĩa **không** phân loại được rõ

Không có call-site sản phẩm nào "không rõ". Ba chỗ cần A0 xác nhận **tên cửa** (ngữ nghĩa đã
rõ, chỉ là đặt tên):

1. `reply_with` (`core_ipc.rs:700`, `kobj.rs:9791`): replier đang `Running`, sau `Reply` thành
   `Ready` — xếp vào **yield** (tự nhường) hay **dispatch**? Đề xuất: yield.
2. `proc_rt::turn` (`proc_rt.rs:325`): sau U-mode `Stop::Yield`, TCB `Running → Ready` — **yield**
   từ U-mode; cần cửa `pub` (`tcb_yield`).
3. `tcb_suspend` (`core_tcb.rs:508`, `kobj.rs:5218`): `Blocked*`/`Running → Ready` + cờ
   `suspended` — **suspend/abort**, không phải wake (TCB không chạy được vì `suspended`).

**Ca test thành vô nghĩa**: `kt19_sched.rs:702` gọi `block_current(0, TcbState::Ready)` để kiểm
`InvalidArgument`. Dưới API mới, `Ready` **không biểu diễn được** ⇒ phải **xoá** ca này (hoặc
đổi thành ca kiểm "không biên dịch" ở tầng checker). Ghi nhận là **thay đổi phạm vi test**,
không phải mất năng lực kiểm.

### 5.2 Parity core/sim

- `NonReadyTarget` phải **giống hệt** ở core và sim (cùng biến thể, cùng payload) — nếu lệch,
  `kt59/60/64_core_parity` không biên dịch.
- Thân `write_state` phải **giữ nguyên thứ tự hook**: trả donation (KT-21) → `Reply.caller`
  (KT-34) → preempt khi vào `Running` → ghi `state` → `pair_hits` → `budget_sleeps` → `sched_sync`.
  Đổi thứ tự ⇒ parity đỏ.
- `wake_step` giữ `unlink`; `wake_from` giữ `push_mail` chéo hart; `apply_mail` giữ
  `wake_step(tcb, true)`.
- Rủi ro **thấp** nhưng phải đo: `kt155_n7addr`/`kt155_n7sim` (chéo hart), `kt151_wake`,
  `kt159_wake_semantics`, `kt60_core_parity`.

### 5.3 `spec/` — **KHÔNG** cần đổi

`NonReadyTarget` là **model-only `[TK]`**, không có trong `spec/abi.toml`; `TcbState` (11 tên)
vẫn lấy từ `spec/[method_signature.state]` qua `vifast_abi::TCB_STATES`; `transition_tcb` là
API model, không phải ABI. **Không** đổi `spec/`, **không** đổi ABI, **không** đổi layout công bố.

### 5.4 `unsafe` / ABI / layout

- **Không** thêm `unsafe` (core `#![forbid(unsafe_code)]`; prototype không dùng `unsafe`).
- `NonReadyTarget` không `#[repr]`, không đi qua ABI; `TcbState` không đổi ⇒ layout công bố
  không đổi.
- Còn lại một đường **lách bằng `unsafe`** (`transmute::<TcbState, NonReadyTarget>`) — ngoài
  phạm vi "không thêm unsafe"; checker hiện có luật cấm `unsafe` ngoài danh sách (D2-17).

### 5.5 Điểm **không** chặn được (trung thực)

- `NonReadyTarget::Running { hart }` vẫn cho phép đặt TCB `Ready → Running` — nhưng bảng
  `ALLOWED` cho thấy `Running` **chỉ** vào được từ `Ready` (`Inactive/Blocked*/Sleeping/Stopped
  → Running` đều `false`), nên **không** biến TCB đang chờ thành chạy được. Không phải bypass
  `Ready`.
- `post_mail(hart, MailboxMsg::Wake(t))` vẫn là `pub` — nhưng là **wake chéo hart** hợp lệ, đã
  bị luật `MailboxMsg` của checker quản; không phải cửa thô.
- `wake()` vẫn `pub` (test-only theo `E_STRICT_TEST_ONLY_WAKE`) — giữ nguyên, checker đã phủ.

---

## 6. Câu hỏi cho Agent 0

1. **Tên kiểu**: `NonReadyTarget` (enum 10 biến thể, đề xuất) hay newtype `NonReady(TcbState)`
   với field private + constructor? Enum rõ hơn cho checker; newtype ít trùng lặp hơn nhưng
   call-site vẫn đổi tương đương.
2. **Giữ tên `transition_tcb`** (đề xuất — checker chỉ cần bỏ phân tích đối số) hay đổi
   `transition_tcb_nonready` (rõ hơn, nhưng đổi 216 call-site + mọi tài liệu)?
3. **`start_tcb`/`tcb_yield` `pub`**: chấp nhận thêm 2 cửa `pub` có ngữ nghĩa cho crate ngoài
   (`core_bridge`, `proc_rt`), hay muốn gom qua `ensure_started`/`yield_current` (kéo theo đổi
   hành vi dispatch)?
4. **`block_current`**: đổi sang `NonReadyTarget` (đề xuất, chặn ở kiểu) hay giữ `TcbState` +
   kiểm runtime (không đạt yêu cầu "không biểu diễn được")?
5. **Checker**: task hiện thực nào sửa `check-wake-paths.py` + `check-wake-depinfo.py`? Đây là
   **chặn cứng** — không sửa thì `v2 check` đỏ. Có cần tách task checker **trước** task API
   không (D2-192 nói gate chạy độc quyền)?
6. **Ca test `kt19_sched.rs:702`** (`block_current(0, Ready)` kỳ vọng `InvalidArgument`): xoá
   hay chuyển thành ca kiểm tầng checker?
7. **`reply_with`**: xếp "yield" (đề xuất) hay "dispatch"? Ảnh hưởng tên cửa, không ảnh hưởng
   hành vi.
8. **`NonReadyTarget::Running`**: giữ trong enum (đề xuất — `ALLOWED` chứng minh không bypass)
   hay tách cửa `dispatch_to_running` riêng?

---

## 7. Bằng chứng (log)

| Tệp | Nội dung |
|---|---|
| [`kt237-logs/callsites.json`](kt237-logs/callsites.json) | inventory thô 323 call-site (mọi cửa) |
| [`kt237-logs/inventory.py`](kt237-logs/inventory.py) | script inventory (token-tree + enclosing fn) |
| [`kt237-logs/classify.py`](kt237-logs/classify.py) | phân loại đối số thứ hai |
| [`kt237-logs/proto-core-full.log`](kt237-logs/proto-core-full.log) | **E0624 + E0308 + E0631** (core) |
| [`kt237-logs/proto-core-all.log`](kt237-logs/proto-core-all.log) | bản `--message-format=short` |
| [`kt237-logs/proto-core-ok.log`](kt237-logs/proto-core-ok.log) | đối chứng dương core (RC=0) |
| [`kt237-logs/proto-sim-full.log`](kt237-logs/proto-sim-full.log) | **E0624** (sim) |
| [`kt237-logs/proto-sim-ok.log`](kt237-logs/proto-sim-ok.log) | đối chứng dương sim (RC=0) |
| [`kt237-logs/proto/`](kt237-logs/proto/) | nguồn prototype (đã hoàn nguyên khỏi cây) |

**Trạng thái cây**: `git status` chỉ có `v2/agent/JOURNAL/kt237-logs/` + tệp journal này;
`cargo check` kernel (riscv64) và sim **RC=0** sau khi hoàn nguyên.
