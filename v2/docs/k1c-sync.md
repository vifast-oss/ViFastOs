<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# `vifast-sync` — MCS lock + hộp thư MPSC (KT-30) — model-only `[TK]`

Tài liệu này mô tả crate `v2/lib/vifast-sync` của K1c. Đây **không phải** `spec/`, **không phải**
ABI: không thêm method/chữ ký nào, không đổi `spec/abi.toml`. Nguồn ngữ nghĩa: thiết kế §4.16
(`docs/v2/ViFastOs-v2-thiet-ke.md:404-419`, S1 + 5 luật), §4.17 (`:421-428`, hộp thư theo hart),
`review/agent0/K1C-PLAN.md` §5 KT-30 (**D2-53 Q9** phương án **(a)**: stress host có biên + Miri,
**không** loom, **không** explorer tự viết); D2-17 (crate trong danh sách đóng được `unsafe`),
D2-21 (không dependency ngoài), D2-42 (`no_std` + test dùng `std`).

Số đo chỉ do lệnh cổng sinh (`./v2/bin/v2 gate --fast`, `./v2/bin/v2 miri -p vifast-sync`); log
nằm trong `v2/agent/JOURNAL/` (QD §5.3, §9 Q12 — tài liệu **không** chứa số đo viết tay).

## 1. Phạm vi, hằng số, ràng buộc `[TK]`

| Hạng mục | Nội dung |
|---|---|
| Vị trí | `v2/lib/vifast-sync` (member của workspace `v2/Cargo.toml`) |
| `no_std` | `#![no_std]`; chỉ `core::sync::atomic`, `core::cell::UnsafeCell`, `core::mem::MaybeUninit` |
| Dependency | **không** (D2-21); test dùng `std` của crate test, crate **không** cần feature `std` (D2-42) |
| Không cấp phát | không `alloc`, không `Box`, không con trỏ thô trong thuật toán |
| `unsafe` | chỉ trong crate này (D2-17): **8 khối** — 2 `Deref`/`DerefMut` của `Guard`, 2 ghi/đọc slot `MaybeUninit` của `Mailbox`, 4 `unsafe impl Send/Sync`; **không** có `unsafe fn` |
| Kiến trúc | **không** cfg theo kiến trúc (luật R2); `cfg!(miri)` chỉ để giảm số vòng của **một** test |
| `MAX_WAITERS` | **16** `[TK]` — pool node của MCS lock; `MAX_HARTS` = 8 (thiết kế §4.16) + dư |
| `Mailbox<N>` | `N ≥ 2` `[TK]` (lý do ở §3.4), `N` slot cấp sẵn; test dùng 2/4/8/16 |
| Hai vai trò | `McsLock` là **khoá**; `Mailbox` là phần **lock-free** duy nhất (đúng thiết kế `:428`) |

## 2. `McsLock<T>` — khoá xếp hàng MCS

### 2.1 API

| Hàm | Nghĩa |
|---|---|
| `lock() -> Guard<'_, T>` | xin khoá, **chờ** tới khi được trao; `Drop` của guard **mở khoá** |
| `try_lock() -> Option<Guard<'_, T>>` | không chờ: đang giữ/đang có người xếp hàng ⇒ `None` |
| `queue_len() -> usize` | số node **đang trong hàng** (tính cả chủ) — **chẩn đoán/test** |
| `next_owner() -> Option<u32>` | ticket của node **đầu hàng**; `None` khi hàng rỗng — **chẩn đoán/test** |
| `capacity()` | `MAX_WAITERS` |
| `test_enqueue/test_woken/test_release`, `Guard::slot` | **`pub(crate)` + `cfg(test)`** (không có trong build thường): kiểm FIFO/không reentrant/pool cạn **single-thread**. **Không** `pub`: nếu code ngoài crate (safe) mở khoá hộ slot thì có hai `Guard` cùng lúc ⇒ alias `&mut T` = UB (verdict `reviewer-kt30` mục 2) |

### 2.2 Thuật toán

MCS (Mellor-Crummey & Scott): mỗi người xin khoá chèn **node** của mình vào đuôi hàng
(`tail.swap`); node trước ghi liên kết `next` tới node mới. Khi mở khoá, chủ trao khoá **đúng
một** node kế tiếp (`locked = false`) và người được trao tự quay vòng trên cờ `locked` **của node
mình** — khác test-and-set, không có tranh chấp cache line giữa những người chờ.

Node nằm trong **pool cố định của chính khoá** (`[Node; MAX_WAITERS]`), **không** trên stack người
gọi: trong Rust, `Guard` trả về theo giá trị nên node sẽ bị **di chuyển** sau khi địa chỉ của nó
đã được công bố cho node trước ⇒ con trỏ treo, UB (K1C-PLAN §5 KT-30 rủi ro (a)). Pool cố định
cho địa chỉ bất động suốt đời khoá, `Guard` chỉ giữ **chỉ số slot** (một `usize`) nên di chuyển
vô hại. Hàng đợi liên kết bằng **chỉ số slot + 1** (`0` = không có) ⇒ không có con trỏ thô.

Đây là biến thể "node tĩnh theo CPU" quen thuộc của MCS trong kernel (mỗi hart một node tĩnh);
ở đây pool chung `MAX_WAITERS` slot để `lock()` không cần tham số hart. Tham số hoá kích thước
pool/tham số hoá `N` để mốc sau, nếu cần.

Chọn slot: **bitmap** `AtomicUsize` (bit `i` = 1 ⇔ slot `i` trống); người xin `compare_exchange`
để claim, trả bit khi mở khoá. `try_lock` **không** chờ slot: pool cạn ⇒ `None`.

### 2.3 Bậc bộ nhớ

`tail`/`head`/`next`/`locked` dùng `Ordering::SeqCst`; bitmap slot dùng `AcqRel`/`Release`. Lý do
`[TK]`: K1c **không** có mục tiêu hiệu năng, và FIFO + mutual exclusion cần một mô hình dễ chứng
minh. Hạ xuống `AcqRel`/`Release` để dành cho mốc đo hiệu năng (K3 trở đi); khi hạ phải kèm test
đa luồng + (nếu có) loom.

### 2.4 Bất biến

1. `tail == 0` ⇔ hàng rỗng (trừ khoảng chuyển tiếp một lệnh khi người vừa chiếm khoá chưa kịp
   ghi `head`).
2. Mỗi slot có **tối đa một** chủ tại một thời điểm (bitmap).
3. Chủ hiện tại là node ở **đầu** hàng (`head`).
4. Trao khoá theo **đúng thứ tự chèn** `tail.swap` ⇒ **FIFO** theo thứ tự xếp hàng.
5. **Không reentrant**: xếp hàng lần hai khi đang giữ **không** được acquire (gọi `lock()` thật
   trên cùng luồng sẽ **treo** — đúng nghĩa; harness kiểm bằng API nội bộ để không treo).
6. `Guard` **không** `Send`: miền tới hạn gắn với luồng đã lấy khoá (như `MutexGuard`).

## 3. `Mailbox<N>` — hộp thư theo hart (MPSC cố định)

### 3.1 API

| Hàm | Nghĩa |
|---|---|
| `send(Msg) -> Result<(), Busy>` | gửi; **đầy ⇒ `Err(Busy)`**, không ghi đè, không mất message cũ |
| `try_recv() -> Option<Msg>` | rút **một** message theo FIFO |
| `drain(f) -> usize` | rút **toàn bộ** theo FIFO, gọi `f` từng message; trả số đã rút |
| `len()` / `is_empty()` / `capacity()` | `head - tail` / rỗng? / `N` |
| `generation() / acked() / ack(gen) / pending(gen)` | bộ đếm thế hệ (xem §3.3) |

### 3.2 Thuật toán

Hàng đợi vòng biên `N`, **nhiều người gửi — một người nhận** (MPSC), lock-free hai phía, kiểu
Vyukov: mỗi slot có `seq: AtomicUsize` mã hoá "vị trí đã sẵn sàng" (`pos` cho người gửi,
`pos + 1` cho người nhận, `pos + N` sau khi rút); người gửi đặt chỗ bằng `head.compare_exchange`,
người nhận đặt chỗ bằng `tail.compare_exchange`. Message chưa rút **không** bị ghi đè: người gửi
thấy slot còn thuộc vòng trước ⇒ `Busy` (**backpressure**, thiết kế `:427`: đầy thì người gửi xử
lý hộp thư của mình rồi thử lại). Giá trị message nằm trong `UnsafeCell<MaybeUninit<Msg>>`:
người gửi ghi trước khi `seq` công bố (release), người nhận đọc sau khi thấy `seq` (acquire).

Hộp thư **không** cấp phát và không có khoá; bộ đếm `head`/`tail` đơn điệu, wrap `usize`.

### 3.3 Bộ đếm thế hệ `[TK]`

- `generation` = số message `send` đã nhận (`Ok`) — **bộ đếm thế hệ** `AtomicU32`.
- `ack(gen)` đánh dấu đã xử lý tới thế hệ `gen` (`fetch_max`, **không** bao giờ lùi; kẹp về
  `generation`); trả số message **mới** được ack.
- `pending(gen)` = `min(gen, generation) - acked` = số message **chưa ack** tính tới `gen`.
- `send` trả `Busy` **không** tiêu thế hệ (thế hệ chỉ tăng khi message được nhận).
- Trạng thái chuyển tiếp `[TK]`: `send` tăng `generation` **trước** khi công bố message, nên
  người nhận có thể thấy "đã đếm" trước khi rút được; bất biến **không mất message** không phụ
  thuộc thứ tự này.
- **Việc của K3** (ghi để không hiểu nhầm): `acked` là **lời khai của người nhận** ("đã xử lý
  tới thế hệ `gen`"), **không** phải bằng chứng "message đã giao". Người nhận có thể ack trước
  khi `drain` xong hoặc ack thế hệ mới hơn mức đã xử lý — model không ép. Khi nối vào kernel
  phải chốt `acked ≠ delivered` (thêm bộ đếm `delivered` hoặc ràng buộc `ack` sau `drain`).

### 3.4 Vì sao `N ≥ 2`

`seq` của slot mã hoá cả vị trí đặt chỗ (`pos + 1`) và vị trí đã rút (`pos + N`); với `N = 1`
hai giá trị trùng nhau nên không phân biệt được "đầy" và "trống". `Mailbox::new` `assert!(N >= 2)`.

### 3.5 Bảy loại message `[TK]` cho payload

`Wake/TlbFlush/Reschedule/Migrate/Stop/VmKick/PowerReq` — đúng danh sách thiết kế §4.17. Payload
**model-only**, chưa phải ABI (chưa có nguồn `spec/`):

| Loại | Payload `[TK]` | Nghĩa |
|---|---|---|
| `Wake` | `hart: u16, tcb: u32` | đánh thức TCB trên hart |
| `TlbFlush` | `hart: u16, tcb: u32` | xả TLB cho không gian địa chỉ của TCB |
| `Reschedule` | `hart: u16, tcb: u32` | hart lập lịch lại |
| `Migrate` | `hart: u16, tcb: u32` | đẩy TCB sang hart (push migration §4.8) |
| `Stop` | `hart: u16` | dừng hart |
| `VmKick` | `hart: u16` | đá hart ra khỏi guest (H1) |
| `PowerReq` | `hart: u16, state: PowerState` | yêu cầu trạng thái nguồn (`On/Off/Suspend`) |

## 4. Kiểm chứng (D2-53 Q9 (a): stress host + Miri, **không** loom)

| Hạng mục | Cách kiểm | Tệp |
|---|---|---|
| Ngữ nghĩa single-thread | **10 test** (API công khai): lock/unlock, `try_lock` khi đang giữ ⇒ `None`, ticket tăng, `queue_len`/`next_owner`; mailbox 7 loại/FIFO/đầy ⇒ `Busy`/không mất message đầu/**1.000 vòng** gửi-đầy-rút/ack-pending/`Busy` không tiêu thế hệ | `tests/kt30_sync.rs` |
| FIFO/không reentrant/pool cạn | **4 unit test** trong crate (dùng `pub(crate)` test hook): FIFO theo ticket, không reentrant, pool **cạn 16 slot** ⇒ `try_lock() == None` không panic + hồi phục, `free_mask` không tràn (`n ≥ 64`) | `src/mcs.rs` (`#[cfg(test)] mod tests`) |
| Miri | `./v2/bin/v2 miri -p vifast-sync` (**4 unit + 10 test + 2 doctest**): an toàn bộ nhớ của `Deref`/`DerefMut`, `MaybeUninit` của slot, vòng đời node | `src/mcs.rs`, `tests/kt30_sync.rs` |
| Stress host | 4 luồng × 500 op `lock/unlock` + `send`, **chỉ** assert mutual exclusion (bộ đếm tăng đúng) + **không mất message** (tổng `drain` = tổng `send Ok`, đối chiếu từng hart) + `send Ok + Busy = số op`; **không** assert trace/thứ tự | `tests/kt30_stress.rs` |
| FIFO đa luồng có điều khiển | luồng chính giữ khoá, B xếp hàng trước, C sau (chờ `queue_len`), mở khoá ⇒ thứ tự trao `[B, C]` | `tests/kt30_stress.rs` |
| `try_lock` tranh chấp | 4 luồng, bộ đếm "số chủ đang trong miền tới hạn" phải luôn = 1 | `tests/kt30_stress.rs` |
| Cổng | test mới vào `gate --fast`; stress **không** chạy dưới Miri (`#![cfg(not(miri))]` — lý do ở đầu tệp) | `v2/bin/v2` |

- **Không** thay loom bằng explorer tự viết trong task này (Q9 mặc định (a)); nếu Agent 0 chọn
  (b) thì **task riêng**. **Không** thêm dependency `loom` (Q9 (c) bác: cần mạng + license + D2).
- Stress chỉ assert **bất biến**, **không** assert thứ tự/trace (luồng không tất định). Flake ⇒
  `T-INFRA` + log, **không** retry (D2-07), **không** nới biên `[ƯL]` 3 s.

## 5. Giới hạn và phần hoãn

1. **Chưa nối vào kernel/sim**: K1c chưa có hart thật/khoá thật; `vifast-sync` là crate độc lập
   (KT-30 chạm duy nhất `v2/lib/vifast-sync/**` + member + danh sách Miri). Nối BKL/hộp thư vào
   `vifast-sim`/kernel là mốc sau (K3), khi đó mới có test đồng thời thật.
2. **Một người nhận**: `Mailbox` là MPSC; nhiều người gọi `drain`/`try_recv` đồng thời **không**
   được hỗ trợ (đúng thiết kế: mỗi hart một hộp thư, chỉ hart đích rút).
3. **Pool MCS cố định**: `lock()` chờ slot khi pool cạn; với `MAX_WAITERS = 16 ≥ MAX_HARTS + dư`
   điều này không xảy ra trong cấu hình K1c. Tham số hoá `N` để sau.
4. **Bậc bộ nhớ chưa tối ưu** (§2.3); tối ưu kèm đo hiệu năng, không đổi ngữ nghĩa.
5. **Payload `Msg` là model-only**; khi có `spec/kernel.toml`/ABI IPI thật thì payload phải theo
   spec (D2-04/D2-22 — không tự thêm vào `spec/`).
6. **Không tuyên bố toàn bộ lock-free**: chỉ **hộp thư** lock-free; MCS lock là **khoá**.
7. **`release` có thể quay vòng chờ liên kết**: khi người kế tiếp đã `tail.swap` nhưng chưa kịp
   ghi `next` (bị preempt giữa hai lệnh), chủ đang mở khoá quay vòng tới khi thấy `next`
   (`mcs.rs`, nhánh CAS `tail` thất bại). Trên host, đó là preempt của luồng; trong kernel, đó là
   cửa sổ vài lệnh của hart khác — **không** phải deadlock (người kế luôn ghi `next` ngay sau
   `swap`), nhưng là chi phí cần biết. Test đa luồng phủ xác suất; **không** có test tất định cho
   cửa sổ này (cần hook chia đôi `enqueue`, để mốc sau nếu Agent 0 muốn).
8. **Test hook chỉ trong crate**: `test_*`/`Guard::slot` là `pub(crate)` + `cfg(test)`; nếu sau
   này cần kiểm từ integration test thì phải mở bằng **feature** rõ ràng, **không** `pub` trần.
