<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# WaitOnAddress — KT-28 (model-only `[TK]`)

**Trạng thái:** REVIEW (Agent 2 dòng OS soạn cho Agent 0/reviewer độc lập). **Không phải ABI.**
Nguồn sự thật của ABI vẫn là `spec/abi.toml` (D2-11); tệp này chỉ ghi **quy ước mô hình** đã dùng
trong `v2/sim/vifast-sim` cho **KT-28** (WaitAddr/WakeAddr), để KT-29/30 và K2 (Frame) không phải
suy lại.

**Nguồn ngữ nghĩa:** thiết kế §4.7 `:332` (*"`WaitAddr(frame_cap, offset, expected, timeout)`: đọc
giá trị và xếp hàng **trong cùng một lần giữ khoá**; `WakeAddr` cũng giữ khoá, nên không mất đánh
thức"*), thiết kế §9.2 `:750` (*"không mất đánh thức; Frame chia sẻ hai tiến trình"* — cột chứng
minh "Mutex std stress", đột biến "bỏ kiểm giá trị dưới khoá"), **QD §3.8**, **D2-53 Q3/Q4**,
**D2-33** (`BlockedOnAddress.offset: u32`), `spec/abi.toml:115-116` (syscall 9 `WaitAddr`,
10 `WakeAddr`), K1C-PLAN §5 KT-28 (DoD 1..8).

## 1. Dữ liệu (model-only `[TK]`)

```rust
// v2/sim/vifast-sim/src/kobj.rs
pub const WAITADDR_BUCKETS: usize = 64;             // [TK] (D2-53 Q3)

pub fn waitaddr_bucket(frame: ObjId, offset: u32) -> usize {
    ((frame.wrapping_mul(0x9E37_79B1) ^ offset) as usize) % WAITADDR_BUCKETS
}

// Machine
address_waiters: [Vec<(ObjId /*frame*/, u32 /*offset*/, ObjId /*tcb*/)>; WAITADDR_BUCKETS],
addr_words:      Vec<(ObjId, u32, u64)>,            // giá trị "từ" — mô hình thay vùng nhớ Frame
```

| Hạng mục | Giá trị | Nguồn |
|---|---|---|
| `WAITADDR_BUCKETS` | **64** `[TK]` — hằng **model** | D2-53 Q3: `spec/kernel.toml` **chưa tồn tại** (K3) nên **không** tạo; hằng số ghi ở tệp này |
| Hàm băm | `(frame.wrapping_mul(0x9E37_79B1) ^ offset) % 64` `[TK]` | K1C-PLAN §5 KT-28 DoD 1 |
| Khoá so khớp | **đầy đủ** `(frame, offset)` trong bucket | K1C-PLAN §5 KT-28 DoD 1 — va chạm bucket không ảnh hưởng |
| Nút waiter | `(frame, offset, tcb)`; mỗi TCB tối đa **một** nút | QD §3.8 ("nút nằm trong TCB"), DoD 1 |
| Giá trị tại `(frame, offset)` | `u64`, mặc định **0** (như bộ nhớ mới), ghi/đọc model API | thiết kế `:332`; `Frame` là object **reserved** (K2) nên mô hình dùng map |
| `offset` | **`u32`** — không đổi sang u64 | D2-33 |
| Timeout | **không có** (Q4 — spec thắng; timeout thuộc K6) | D2-53 Q4 |

`WAITADDR_BUCKETS = 2^6` nên `% 64` chỉ lấy **6 bit thấp** của phép XOR ⇒ hai `offset` lệch bội
64 **va chạm có chủ ý**; test KT-28 dùng đúng cặp `(f, 0)`/`(f, 64)` để kiểm "va chạm bucket không
ảnh hưởng tìm/đánh thức". Khoá được so **đầy đủ** trong bucket nên đây là va chạm **hợp lệ**.

## 2. Hành vi

### 2.1 `WaitAddr(frame, offset, expected)` — `Machine::wait_addr(tcb_slot, frame, offset, expected)`

1. TCB phải `Ready`/`Running` (hoặc `Inactive` ⇒ `ensure_started` đưa về `Ready` như API K1b);
   **`suspended` ⇒ `IllegalOperation`** (bất biến §4.4: TCB nằm trong hàng của X ⇔ trạng thái
   mang X **và** `!suspended`).
2. **"Khoá" của mô hình một luồng = một hàm**: đọc `addr_word(frame, offset)` **và** push nút vào
   bucket `waitaddr_bucket(frame, offset)` trong cùng hàm, **không** có public op nào chen giữa
   (không `sched_after_op` ở giữa). Mô hình **không có** cửa sổ lost wake để biểu diễn; bằng
   chứng là property theo **thứ tự bước** (§5), không phải thread thật.
3. `giá trị hiện tại != expected` ⇒ **`InvalidArgument`**, TCB **không** xếp hàng, giữ
   `Ready`/`Running` (DoD 2). Đột biến "xếp hàng trước rồi mới so" bị test bắt.
4. Bằng ⇒ `transition_tcb(BlockedOnAddress{frame, offset})` **trước**, rồi push nút (push không
   lỗi; nếu transition bị từ chối thì không để nút mồ côi).
5. **Không timeout** (Q4): không sinh `Sleeping{Timeout}`; `tick` ảo **không** đánh thức waiter;
   chỉ `WakeAddr` / `Suspend(Abort)` / `Exit` / finalise đưa TCB ra khỏi hàng.

`Machine::tcb_wait_address(tcb, frame, offset)` (API K1b của KT-18/19/23) **giữ nguyên**: đọc giá
trị hiện tại rồi đi **cùng một cửa** `wait_addr` ⇒ luôn khớp ⇒ luôn chặn như trước.

### 2.2 `WakeAddr(frame, offset, n)` — `Machine::wake_addr(frame, offset, n)`

1. Xét **đúng** bucket `waitaddr_bucket(frame, offset)`; chọn nút có khoá **đầy đủ** khớp, theo
   **FIFO** (thứ tự xếp hàng), **tối đa `n`** nút; `n = 0` ⇒ không ai; `n = u32::MAX` ⇒ tất cả.
2. Mỗi nút được chọn: `wake_step` gỡ TCB khỏi **mọi** hàng chờ (kể cả bucket này) rồi `→ Ready`.
3. Trả **số đã đánh thức** (`u32`). Waiter **còn lại** vẫn `BlockedOnAddress` + còn trong bucket ⇒
   **không đánh thức quá n, không mất**.
4. Waiter của khoá **khác** trong cùng bucket (va chạm) **không** bị đụng.
5. Không có waiter ⇒ trả 0; **không** có cờ "wake treo": tín hiệu cho lần Wait sau là **giá trị**
   ở `(frame, offset)` (đúng thiết kế futex — Wake trước chỉ trả 0).

### 2.3 Ghi/đọc giá trị

| API | Nghĩa |
|---|---|
| `Machine::set_addr_word(frame, offset, v)` | ghi giá trị "từ" (tạo nếu chưa có) |
| `Machine::addr_word(frame, offset) -> u64` | đọc; chưa ghi ⇒ **0** |

Model API `[TK]`, **không** thêm chữ ký ABI (D2-53 Q6 tinh thần: model API không vào `spec/`).

## 3. Bất biến — `Machine::check_waitaddr`

Chạy sau **mỗi** public op (qua `sched_after_op`, cùng chỗ `check_sched`/`check_ipc`/
`check_notifications`) và được test gọi tường minh sau mỗi bước fuzz:

1. mỗi nút nằm **đúng** bucket `waitaddr_bucket(frame, offset)` của khoá;
2. TCB của nút **còn sống**, **không** `suspended`, trạng thái đúng
   `BlockedOnAddress{frame, offset}`; TCB `Stopping`/`Dead` ⇒ lỗi "đã chết";
3. **không trùng**: một TCB tối đa **một** nút.

Đường nhanh: mọi bucket rỗng ⇒ `Ok` ngay (không đọc trạng thái TCB) — giữ `check_waitaddr` sau
mỗi public op mà không làm chậm L1 1.000 seed của cổng.

Chiều `TcbState ⇒ nút` **không** kiểm (như `check_notifications`): `transition_tcb` là cổng thấp
của ma trận 11×11 KT-23, được phép dựng `BlockedOnAddress{frame, offset}` với `ObjId` không tồn
tại để phủ bảng; mọi đường thật (`wait_addr`/`tcb_wait_address`) đều push nút.

`unlink_tcb_queues` gỡ nút theo **đúng bucket** suy từ trạng thái TCB (không quét tuyến tính toàn
bảng); `debug_assert!` bắt phần sót trên build debug, `check_waitaddr` bắt trên mọi build.

## 4. Giới hạn của mô hình (ghi để K2/K3 không hiểu nhầm)

1. **"Khoá" là hàm nguyên tử**, không phải lock thật. Bằng chứng "không mất đánh thức" là property
   theo **thứ tự bước**; "Mutex std stress" (thiết kế `:750` cột chứng minh) thuộc **K2/K3**.
2. **Frame chưa có**: `Frame` là object reserved (K2); khoá theo `ObjId` thô `(frame, offset)`,
   **không** theo địa chỉ vật lý. Mệnh đề "**Frame chia sẻ hai tiến trình**" của thiết kế `:750`
   **CHƯA ĐẠT** ở K1c.
3. **Không timeout** (D2-53 Q4): `timeout` của thiết kế `:332` và `Sleeping{Timeout}` thuộc **K6**.
4. **Chưa có `Cancelled` khi Frame bị unmap** (thiết kế `:237`: unmap Frame ⇒ đánh thức waiter
   WaitAddr với `Cancelled`): thuộc **K2** (chưa có Map/Unmap Frame). K1c ghi nợ, không giả vờ.
5. `spec/kernel.toml` **không** được tạo: `WAITADDR_BUCKETS` là hằng model `[TK]`; khi K3 mở
   `spec/kernel.toml` thì con số chuyển sang đó (task riêng, Agent 0).

## 5. Bằng chứng

| # | Nội dung | Bằng chứng |
|---|---|---|
| 1 | Bảng băm 64 bucket, cùng khoá cùng bucket, khoá trải nhiều bucket; va chạm có chủ ý `(f,0)`/`(f,64)` vẫn tìm/đánh thức đúng | `tests/kt28_waitaddr.rs`: `bang_bam_64_bucket_va_cung_khoa_cung_bucket`, `va_cham_bucket_khong_anh_huong_tim_kiem` |
| 2 | `expected` lệch ⇒ `InvalidArgument`, **không** waiter, TCB giữ `Ready`; khớp ⇒ chờ; đổi word **sau** khi chờ không tự đánh thức | `expected_lech_tra_ngay_khong_xep_hang` |
| 3 | Xếp hàng **một lần**: Wait lần hai ⇒ `IllegalOperation`, không nhân đôi nút | `xep_hang_mot_lan` |
| 4 | `WakeAddr` n=0/1/2/`u32::MAX`: FIFO, tối đa n, trả số đã đánh thức, waiter còn lại vẫn chờ | `wake_0_1_n_nhieu_va_fifo` |
| 5 | Hai frame/offset độc lập | `hai_frame_offset_doc_lap` |
| 6 | Không mất đánh thức (C2) hai chiều "trước"/"sau" | `khong_mat_danh_thuc_wake_truoc_va_sau` |
| 7 | `Suspend(Abort)`/`Exit`/finalise gỡ **đúng bucket** (`waitaddr_bucket_len`=0, `waitaddr_nonempty_buckets`=0), không đụng waiter khoá khác | `suspend_abort_go_dung_bucket`, `exit_va_finalise_go_waiter` |
| 8 | **Không timeout**: 10.000 tick ảo không đánh thức waiter | `khong_timeout_du_tick_rat_nhieu` |
| 9 | `check_waitaddr` bắt waiter lệch trạng thái / sai bucket / trùng / TCB chết / TCB suspended | `check_waitaddr_bat_waiter_lech_trang_thai` + 2 unit test trong `kobj.rs` (`check_waitaddr_bat_sai_bucket`, `check_waitaddr_bat_trung_va_tcb_chet`) |
| 10 | 1.000 seed × 64 op, bất biến sau **mỗi** bước, cùng seed cùng trace; Miri giảm seed bằng `cfg(miri)` | `fuzz_1000_seed_64_op_bat_bien_va_tat_dinh` |

## 6. Câu hỏi cho reviewer

1. "Giữ khoá" mô hình hoá bằng **một hàm** (không lock thật) — chấp nhận làm bằng chứng "không
   mất đánh thức" ở K1c, hay muốn thêm test theo **thứ tự bước** khác (ví dụ chèn `tick` giữa
   Wait và Wake)?
2. `WaitAddr` trên TCB `suspended` trả `IllegalOperation` (mới so với `tcb_wait_address` K1b) —
   có phá vỡ kỳ vọng `.scn`/API K1b nào không? (KT-23 fuzz chỉ đếm `is_ok`, không kỳ vọng cụ thể;
   toàn bộ test cũ xanh.)
3. Trả `InvalidArgument` khi `expected` lệch (theo K1C-PLAN §5 KT-28 DoD 2) — K3 conformance có
   muốn mã riêng kiểu `EAGAIN` không? (ABI 19 mã không có `EAGAIN`; ghi nợ K3.)
4. `WakeAddr` trả `u32` trong `Result` — giữ `Result` cho đồng bộ API `Machine` khác, dù hiện
   không có nhánh lỗi.
5. `set_addr_word`/`addr_word` là model API có thể gọi từ test/harness; có cần thêm vào danh sách
   API "chỉ test" (như `set_sched_params`, D2-49(b)) không?

**Chốt review độc lập `reviewer-kt28` (2026-11-13, APPROVE)**: (1) "giữ khoá = một hàm" **chấp
nhận cho K1c**; (2) `WaitAddr` trên TCB `suspended` ⇒ `IllegalOperation` là **nợ K3 conformance**;
(3) giữ `InvalidArgument` (mã riêng kiểu `EAGAIN` để K3); (4) giữ `Result<u32>` cho đồng bộ API
`Machine`; (5) `set_addr_word`/`addr_word` là **model API**, không vào danh sách chỉ-test. Nợ đã
ghi ở `v2/agent/STATE.md` (mục KT-28) và `JOURNAL/kt28-journal.md` §7.
