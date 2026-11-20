# KT-241 — Đặc tả API `NonReadyTarget` (nguồn chung cho core và sim)

Trạng thái: **đặc tả thi hành** cho KT-241, theo **D2-194** (duyệt enum `NonReadyTarget`).
Tệp này là **nguồn duy nhất** cho định nghĩa enum: core và sim phải **chép y nguyên** khối
`NonReadyTarget` dưới đây (chỉ khác dòng `use`). Lệch một biến thể hoặc một payload ⇒
`kt59/60/64_core_parity` không biên dịch.

---

## 1. Định nghĩa enum — **chép y nguyên** vào cả hai crate

```rust
/// Đích chuyển trạng thái TCB **loại trừ `Ready`** — 10 biến thể còn lại của [`TcbState`].
///
/// **D2-187/D2-194**: cửa chuyển trạng thái thông thường **không nhận kiểu biểu diễn được
/// `Ready`**. `Ready` chỉ vào được qua **năm cửa ngữ nghĩa** trong [`state_write`], mỗi cửa
/// kiểm điều kiện nguồn của mình. Không có `From`/`TryFrom` công khai nào từ [`TcbState`]
/// sang kiểu này — đó là toàn bộ ranh giới.
///
/// **Không** `#[non_exhaustive]`: crate ngoài (`ktest_*`, `core_bridge`, `proc_rt`, test sim)
/// phải dựng được mọi biến thể.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonReadyTarget {
    /// Chưa kích hoạt.
    Inactive,
    /// Đang chạy trên hart.
    Running {
        /// Hart đang chạy.
        hart: u16,
    },
    /// Đang chờ gửi.
    BlockedOnSend {
        /// Endpoint chờ.
        ep: ObjId,
        /// Lời gọi (Call) hay gửi thường.
        call: bool,
        /// Fault đi kèm (nếu có).
        fault: Option<FaultKind>,
    },
    /// Đang chờ nhận.
    BlockedOnRecv {
        /// Endpoint chờ.
        ep: ObjId,
        /// Cap Reply (nếu có).
        reply: Option<ObjId>,
    },
    /// Đang chờ Reply.
    BlockedOnReply {
        /// Reply object.
        reply: ObjId,
    },
    /// Đang chờ Notification.
    BlockedOnNotification {
        /// Notification object.
        ntfn: ObjId,
    },
    /// Đang chờ địa chỉ.
    BlockedOnAddress {
        /// Frame.
        frame: ObjId,
        /// Offset.
        offset: u32,
    },
    /// Đang ngủ.
    Sleeping {
        /// Lý do ngủ.
        reason: SleepReason,
    },
    /// Đang dừng.
    Stopping,
    /// Đã chết (trạng thái cuối).
    Dead,
}

impl NonReadyTarget {
    /// **private** — chỉ module `state_write` đổi được sang [`TcbState`].
    ///
    /// **Không** `pub`, **không** `pub(crate)`, **không** `From`/`TryFrom`, **không** method
    /// công khai nào khác trả `TcbState`. Đây là ranh giới (D2-187 §3): `pub(crate)` là
    /// **không đủ** — mọi module trong crate sẽ gọi được, tức đường lọt cũ đổi tên.
    fn into_state(self) -> TcbState {
        match self {
            NonReadyTarget::Inactive => TcbState::Inactive,
            NonReadyTarget::Running { hart } => TcbState::Running { hart },
            NonReadyTarget::BlockedOnSend { ep, call, fault } => {
                TcbState::BlockedOnSend { ep, call, fault }
            }
            NonReadyTarget::BlockedOnRecv { ep, reply } => TcbState::BlockedOnRecv { ep, reply },
            NonReadyTarget::BlockedOnReply { reply } => TcbState::BlockedOnReply { reply },
            NonReadyTarget::BlockedOnNotification { ntfn } => {
                TcbState::BlockedOnNotification { ntfn }
            }
            NonReadyTarget::BlockedOnAddress { frame, offset } => {
                TcbState::BlockedOnAddress { frame, offset }
            }
            NonReadyTarget::Sleeping { reason } => TcbState::Sleeping { reason },
            NonReadyTarget::Stopping => TcbState::Stopping,
            NonReadyTarget::Dead => TcbState::Dead,
        }
    }

    /// **`pub(crate)`** — **vị từ**, KHÔNG phải phép chuyển đổi: chỉ trả lời "đích này có phải
    /// đích **block** hợp lệ không", **không** sinh ra [`TcbState`].
    ///
    /// **D2-194 §1(d)**: "không phải `Ready`" **chưa** đồng nghĩa "được phép block vào" —
    /// `block_current` phải giữ kiểm tra tập trạng thái block riêng.
    pub(crate) fn is_block_target(self) -> bool {
        matches!(
            self,
            NonReadyTarget::BlockedOnSend { .. }
                | NonReadyTarget::BlockedOnRecv { .. }
                | NonReadyTarget::BlockedOnReply { .. }
                | NonReadyTarget::BlockedOnNotification { .. }
                | NonReadyTarget::BlockedOnAddress { .. }
                | NonReadyTarget::Sleeping { .. }
        )
    }
}
```

---

## 2. Bố cục module

### 2.1 Core — `v2/kernel/vifast-core/src/state_write.rs` (mới)

```rust
use crate::error::{KError, ObjId};
use crate::machine::Machine;
use crate::tcb::{FaultKind, SleepReason, TcbState};
```

`lib.rs`: thêm `mod state_write;` (giữ thứ tự alphabet: sau `mod sched;`? — đặt cạnh
`mod machine;`/`mod tcbdata;` cho dễ đọc) **và** `pub use state_write::NonReadyTarget;`.

### 2.2 Sim — `v2/sim/vifast-sim/src/kobj/state_write.rs` (mới)

`kobj.rs` là **một tệp**; thêm `mod state_write;` **trong** `kobj.rs` ⇒ tệp nằm ở
`src/kobj/state_write.rs` (Rust 2018: `kobj.rs` + thư mục `kobj/`).

```rust
use super::{FaultKind, KError, Machine, ObjId, SleepReason, TcbState};
```

`kobj.rs`: thêm `mod state_write;` + `pub use state_write::NonReadyTarget;` (để
`vifast_sim::kobj::NonReadyTarget` hoạt động, đối xứng `vifast_core::NonReadyTarget`).

**Vì sao module con của `kobj`**: module con **thấy được item private của `kobj`**
(`pair_hits`, `sched`, `budget_sleeps`; `sched_sync`, `unlink_tcb_queues`,
`release_donation_of`, `reply_set_caller`, `is_donation_donor`, `tcb_data`, `tcb_data_mut`).
Nhờ vậy **không** phải nới `pub(crate)` cho bất kỳ field/method nào.

---

## 3. Năm cửa `Ready` + cửa thông thường — chữ ký và **điều kiện**

Tất cả nằm trong `state_write.rs`. `write_state` là **private** (`fn`, không `pub`, không
`pub(crate)`).

| # | Cửa | Mức | Điều kiện **nguồn** (fail-closed) | Ghi |
|---|---|---|---|---|
| 0 | `transition_tcb(tcb, to: NonReadyTarget)` | **`pub`** | bảng `ALLOWED` (trong `write_state`) | cửa thông thường **duy nhất** |
| — | `write_state(tcb, to: TcbState)` | **private** | bảng `ALLOWED` + hook KT-21/KT-34 + bất biến `Running` | nguyên thuỷ thô |
| 1 | `wake_step(tcb, unlink: bool)` | `pub(crate)` | `Ready` ⇒ `Ok` (idempotent, giữ nguyên); `is_blocked()`/`Sleeping` ⇒ đi; còn lại ⇒ `IllegalOperation` | wake |
| 2 | `preempt_current(tcb)` | `pub(crate)` | `Running{hart}` **và** `sched[hart].current() == Some(tcb)`; sai ⇒ `IllegalOperation` | preemption |
| 3a | `yield_to_ready(tcb)` | `pub(crate)` | như (2) | yield (nội bộ) |
| 3b | `tcb_yield(tcb)` | **`pub`** | gọi `yield_to_ready` (điều kiện do (3a) kiểm) | yield (crate ngoài) |
| 4 | `start_tcb(tcb)` | **`pub`** | `Inactive`; sai ⇒ `IllegalOperation` | khởi tạo |
| 5 | `suspend_to_ready(tcb)` | `pub(crate)` | **đọc lại** trạng thái: `is_blocked()` hoặc `Running{..}`; sai ⇒ `IllegalOperation` | suspend/abort |

**Quy tắc bắt buộc**:
- Mỗi cửa gọi `write_state(tcb, TcbState::Ready)` — **không** cửa nào khác được ghi `Ready`.
- **Không** tạo wrapper tổng quát nhận `TcbState` rồi gọi `write_state` (D2-194 §2).
- **Không** thêm `impl From<TcbState> for NonReadyTarget`, `TryFrom`, hay bất kỳ hàm công khai
  nào trả `NonReadyTarget` từ `TcbState`.
- `tcb_yield` **không** phải wrapper ghi `Ready`: nó đi qua `yield_to_ready` có kiểm điều kiện.

### 3.1 `write_state` — thân cũ, **hai** thay đổi

Thân là nguyên văn `transition_tcb` hiện tại (`core_tcb.rs:389-455` / `kobj.rs:5079-5160`),
**trừ**:

1. Lời gọi đệ quy `self.transition_tcb(cur, TcbState::Ready)?` trong nhánh `Running{hart}`
   ⇒ **`self.preempt_current(cur)?`**.
2. Chữ ký `pub fn transition_tcb(&mut self, tcb: ObjId, to: TcbState)` ⇒
   `fn write_state(&mut self, tcb: ObjId, to: TcbState)`.

**Giữ nguyên thứ tự hook** (parity): trả donation (KT-21) → `Reply.caller` (KT-34) → preempt
khi vào `Running` → ghi `state` → `pair_hits` → `budget_sleeps` → `sched_sync`.

### 3.2 `block_current` — đổi kiểu, **giữ** kiểm tập block

```rust
pub fn block_current(&mut self, hart: u16, to: NonReadyTarget) -> Result<ObjId, KError> {
    if hart as usize >= self.n_harts {
        return Err(KError::InvalidArgument);
    }
    // **D2-194 §1(d)** — "không phải Ready" CHƯA đồng nghĩa "được phép block vào":
    // `Running`/`Inactive`/`Stopping`/`Dead` vẫn phải bị từ chối ở **runtime**.
    if !to.is_block_target() {
        return Err(KError::InvalidArgument);
    }
    // … phần còn lại giữ nguyên, `to` thay bằng `to.into_state()` khi cần so khớp
    // `BlockedOnNotification { ntfn }` — so khớp **trên `NonReadyTarget`**, không cần
    // `into_state` (module `core_sched`/`kobj` KHÔNG được thấy `into_state`).
}
```

`block_current` **không** được gọi `into_state` (private). Nó truyền `to` xuống
`transition_tcb(cur, to)`.

---

## 4. Bảng chuyển call-site

| Đối số cũ | Đối số mới |
|---|---|
| `TcbState::Inactive` | `NonReadyTarget::Inactive` |
| `TcbState::Running { hart }` | `NonReadyTarget::Running { hart }` |
| `TcbState::BlockedOnSend { ep, call, fault }` | `NonReadyTarget::BlockedOnSend { ep, call, fault }` |
| `TcbState::BlockedOnRecv { ep, reply }` | `NonReadyTarget::BlockedOnRecv { ep, reply }` |
| `TcbState::BlockedOnReply { reply }` | `NonReadyTarget::BlockedOnReply { reply }` |
| `TcbState::BlockedOnNotification { ntfn }` | `NonReadyTarget::BlockedOnNotification { ntfn }` |
| `TcbState::BlockedOnAddress { frame, offset }` | `NonReadyTarget::BlockedOnAddress { frame, offset }` |
| `TcbState::Sleeping { reason }` | `NonReadyTarget::Sleeping { reason }` |
| `TcbState::Stopping` | `NonReadyTarget::Stopping` |
| `TcbState::Dead` | `NonReadyTarget::Dead` |
| `TcbState::Ready` | **cửa** theo bảng §5 — **không** có `NonReadyTarget::Ready` |

Biến/tham số kiểu `TcbState` truyền vào `transition_tcb` ⇒ đổi kiểu khai báo sang
`NonReadyTarget` (ví dụ `block_current`).

---

## 5. Bảng cửa cho từng call-site `Ready` (sản phẩm)

| Tệp:dòng (gốc) | Hàm | Cửa |
|---|---|---|
| `core_ipc.rs:192` / `kobj.rs:10368` | `ensure_started` | `start_tcb` |
| `core_ipc.rs:700` / `kobj.rs:9791` | `reply_with` | `yield_to_ready` (**chỉ** đoạn đưa TCB hiện chạy về `Ready`) |
| `core_sched.rs:171` / `kobj.rs:5640` | `dispatch_step` | `preempt_current` |
| `core_sched.rs:318` / `kobj.rs:5807` | `yield_current` | `yield_to_ready` |
| `core_tcb.rs:436` / `kobj.rs:5132` | `write_state` (nhánh `Running`) | `preempt_current` |
| `core_tcb.rs:508` / `kobj.rs:5218` | `tcb_suspend` | `suspend_to_ready` |
| `core_sched.rs:307` / `kobj.rs:5794` | `wake_step` | `write_state(…, Ready)` — **thân cửa** |
| `proc_rt.rs:200` | `build_rig` | `start_tcb` |
| `proc_rt.rs:325` | `turn` (U-mode yield) | `tcb_yield` |
| `core_bridge.rs:270` | `boot_smoke` | `start_tcb` |

**`reply_with` (D2-194 §1(g))**: **không** phân loại toàn hàm bằng tên. Đoạn
`if matches!(state, TcbState::Running { .. }) { … }` đưa TCB **hiện chạy** về `Ready` ⇒
**yield**. Nếu trong cùng hàm có đoạn **chọn/chạy** TCB `Ready` (dispatch) thì ghi **hai bước
riêng** — hiện `reply_with` **không** có đoạn dispatch, nên chỉ **một** bước.

---

## 6. Điều **cấm** trong task này

1. **Không** đổi `TcbState`, `ALLOWED`, `TCB_STATE_COUNT`, `TcbSnapshot`, `TcbInfo`.
2. **Không** đổi `spec/`, **không** đổi ABI, **không** đổi layout công bố.
3. **Không** thêm `unsafe` (core `#![forbid(unsafe_code)]`).
4. **Không** nới `pub(crate)` cho field/method nào của `Machine` để làm cho module mới biên dịch.
5. **Không** sửa `v2/tools/` (checker do luồng C làm) và **không** sửa `v2/sim/vifast-sim/tests/`
   nếu bạn là luồng core (và ngược lại).
6. **Không** "sửa test cho xanh": nếu một test đỏ vì điều kiện cửa mới chặt hơn, **báo lại**
   kèm `tệp:dòng` + trạng thái nguồn thực tế — **không** tự nới điều kiện.
7. **Không** dùng `#[allow(...)]` để che cảnh báo mới.
