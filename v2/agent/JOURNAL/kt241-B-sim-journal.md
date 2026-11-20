# KT-241 — luồng **B (sim)**: `NonReadyTarget` + 5 cửa `Ready` + `write_state` private

Nhánh `kt241/B-sim`, base `c5a83552`. Phạm vi: **chỉ** `v2/sim/**` (+ journal này).
Đặc tả thi hành: [`kt241-spec/API-SPEC.md`](kt241-spec/API-SPEC.md) (D2-187/D2-194).

## 1. Tệp

| Tệp | Việc |
|---|---|
| `v2/sim/vifast-sim/src/kobj/state_write.rs` | **mới**, 330 dòng: `NonReadyTarget` (chép y nguyên API-SPEC §1) + `write_state` (**private**) + 5 cửa `Ready` |
| `v2/sim/vifast-sim/src/kobj.rs` | `mod state_write;` + `pub use state_write::NonReadyTarget;`; bỏ thân `transition_tcb`/`wake_step` (chuyển sang `state_write.rs`); `block_current` đổi `to: NonReadyTarget` + `is_block_target()`; 27 call-site sản phẩm còn lại + 8 call-site `#[cfg(test)]` |
| `v2/sim/vifast-sim/tests/*.rs` (33 tệp) | 105 call-site `transition_tcb` + 13 call-site `block_current` |

`git diff --stat`: **34** tệp sửa (`kobj.rs` + 33 tệp test), 605(+)/408(−); thêm 1 tệp mới 330 dòng.

## 2. Số call-site đã chuyển

| Nhóm | Số | Chia |
|---|---|---|
| Sản phẩm `kobj.rs` | **29** | 2 chuyển vào `state_write.rs` (đệ quy `Running` ⇒ `preempt_current`; `wake_step` ⇒ `write_state`); 5 ⇒ cửa (`suspend_to_ready`, `preempt_current`, `yield_to_ready`×2, `start_tcb`); 22 ⇒ `NonReadyTarget::X`; 1 (`block_current` truyền `to`) **không đổi thân** (kiểu đổi ở chữ ký) |
| `#[cfg(test)] mod tests` của `kobj.rs` | **8** (journal ước lượng 11) | 4 ⇒ `start_tcb`; 1 ⇒ `wake` + gieo lại message fault; 3 ⇒ `NonReadyTarget::X` |
| `tests/*.rs` — `transition_tcb` | **105** (journal ước lượng 106; 106 là đếm **dòng**, gồm cả tên hàm `moi_duong_dung_mot_cua_transition_tcb`) | 61 ⇒ `start_tcb`; 4 ⇒ `tcb_yield`; 1 ⇒ `wake`; 2 ⇒ `NonReadyTarget::Inactive` (đích đổi, xem §5); 37 ⇒ `NonReadyTarget::X` |
| `tests/*.rs` — `block_current` | **13** | 12 ⇒ `NonReadyTarget::X`; 1 (`kt19_sched.rs:702`) ⇒ `NonReadyTarget::Running { hart: 0 }` |
| **Tổng** | **155** | |

## 3. Kiểm (RC thật, worktree B)

| Lệnh | RC | Ghi chú |
|---|---|---|
| `cargo check -p vifast-sim` | **0** | |
| `cargo test -p vifast-sim --no-run` | **101** | **chỉ** 10 tệp gọi **core** (`kt151_wake`, `kt154_n7fix`, `kt155_n7addr`, `kt159_wake_semantics`, `kt59/60_core_parity`, `kt62_syscall`, `kt63_core_cap_ipc`, `kt64_core_parity`, `kt64_core_tlb_bind_resume`): core trong worktree B còn API **cũ** (`kc::NonReadyTarget`/`start_tcb`/`tcb_yield` chưa có). Không có lỗi nào phía sim. |
| `cargo test -p vifast-sim` | **101** | cùng lý do |
| `cargo fmt --all -- --check` | **0** | |
| `cargo clippy -p vifast-sim -- -D warnings` | **0** | |

**Đo sớm trạng thái sau hợp nhất** (không thay phép đo có thẩm quyền của lead): bản sao
`/tmp/kt241-verify` = worktree B + `v2/kernel/` của A (commit `abfa3e0c`) ⇒
`cargo test -p vifast-sim` **RC=0, 490 passed / 0 failed** (toàn bộ target), `cargo fmt --all -- --check`
RC=0, `cargo clippy -p vifast-sim --all-targets -- -D warnings` RC=0.

42/42 target **chỉ-sim** trong worktree B xanh: **336 test passed, 0 failed** (chạy riêng từng `--test`).

## 4. `kt19_sched.rs:702`

`block_current(0, TcbState::Ready)` ⇒ **`block_current(0, NonReadyTarget::Running { hart: 0 })`**,
vẫn kỳ vọng `InvalidArgument` — giữ đúng phép kiểm **runtime** "không phải tập block hợp lệ"
(D2-194 §1(d)). Thông điệp `kiem` đổi `"block_current(Ready) bị từ chối"` →
`"block_current(Running) bị từ chối"`.

## 5. **Thay đổi phạm vi ca kiểm** (API thu hẹp, **không** nới ngưỡng)

### 5.1 `forbidden_pairs_attempted` 73 → 70 (`tests/kt23_k1b.rs`)

Cột `Ready` của bảng cấm **không còn chạm được** `forbidden_hits`: `forbidden_hits[from][to]`
chỉ tăng trong `write_state`, mà `write_state` với `to = Ready` **chỉ** được gọi từ 5 cửa — cả
5 đều kiểm điều kiện nguồn **trước** (`start_tcb`: `Inactive`; `preempt_current`/`yield_to_ready`:
`Running`+`current`; `wake_step`: `BlockedOn*`/`Sleeping`; `suspend_to_ready`: `BlockedOn*`/`Running`),
và mọi nguồn đó đều `allowed(_, Ready) == true`. Vậy 3 cặp cấm `(Ready,Ready)`,
`(Stopping,Ready)`, `(Dead,Ready)` **không** tới được `write_state`.

- **Giữ nguyên** kỳ vọng **an toàn**: vẫn duyệt đủ 121 cặp; 3 cặp trên vẫn phải bị **từ chối**
  — kiểm bằng **cả ba** cửa `pub`: `start_tcb` + `tcb_yield` phải `IllegalOperation`; `wake`
  phải `IllegalOperation` với nguồn `Stopping`/`Dead`, còn nguồn `Ready` thì `wake`
  **idempotent** trả `Ok` (API-SPEC §3 dòng 1) và **không** đổi trạng thái; `pair_hit_count == 0`.
- **Chỉ đổi kỳ vọng đếm**: `forbidden_attempt_count(from, Ready) == 0` cho 3 cặp đó;
  `forbidden_pairs_attempted == 70`; `forbidden_attempts >= 70`; `(10,1) == 0`.
- **Không** nới `ALLOWED`, **không** đổi `forbidden_hits`, **không** đổi `TCB_STATE_COUNT`.
- Chạy thật: `# KT-23 ma trận: transitions hit/allowed = 46/46 (bảng 48/48, reserved 2),
  forbidden_attempts=70 cặp 70/73`.

### 5.2 `kt27_notification.rs:694` + `kt28_waitaddr.rs:540` — đích `Ready` → `Inactive`

Hai ca "đường tắt sai" cần `BlockedOn* → Ready` **không** unlink để waiter còn treo; cửa
`wake` **có** unlink nên không dùng được. Đích đổi sang `NonReadyTarget::Inactive`
(`BlockedOnNotification/BlockedOnAddress → Inactive` là cặp `ALLOWED`, **không** unlink) ⇒
bất biến được kiểm **như nhau**; mọi assertion cũ giữ nguyên, thêm **đối chứng dương**:
`wake()` đúng (có unlink) ⇒ `check_notifications()`/`check_waitaddr()` phải `Ok`.

### 5.3 `kobj.rs` cfg(test) `check_fault_bat_fault_bi_mat_va_bi_nhan_ban`

`transition_tcb(t, Ready)` (nguồn `BlockedOnSend{fault}`) ⇒ `wake(t)` + **gieo lại** message
fault vào hàng (`endpoint_enqueue`) để dựng đúng trạng thái lệch cần kiểm. Trạng thái cuối
**giống hệt** bản cũ (TCB `Ready`, hàng còn fault); mọi assertion giữ nguyên.

### 5.4 `kt18_tcb.rs` ma trận 11×11

Cột `Ready` đi qua cửa ngữ nghĩa (`apply_pair`). Vì `wake` chạy `sched_after_op` ⇒
`check_reply_links` đòi `BlockedOnReply{reply}` trỏ **Reply thật**, ma trận cấp **một Reply
mới cho mỗi cặp** (như ma trận KT-23 đã làm). Không đổi kỳ vọng nào.

## 6. Lệch so với API-SPEC

1. **Intra-doc link**: `[`state_write`]` → `` `state_write` `` trong doc của `NonReadyTarget`
   (module **private** ⇒ `cargo doc` cảnh báo). Đây là lệch **duy nhất** so với API-SPEC §1.
   **Core cần A sửa cùng** (`v2/kernel/vifast-core/src/state_write.rs`).
2. `preempt_current`/`yield_to_ready` dùng helper private `require_running_current` thay vì
   lặp điều kiện — hành vi **giống hệt** core (cùng `IllegalOperation` cho mọi nhánh sai).

## 7. Xác nhận grep

- `grep -rn "transition_tcb" v2/sim/ | grep -c "TcbState::Ready"` = **0**.
- `grep -rn "write_state" v2/sim/` chỉ có trong `v2/sim/vifast-sim/src/kobj/state_write.rs`.

## 8. Còn nghi ngờ / không làm được

- `cargo test -p vifast-sim` trong worktree B **không thể** RC=0 trước khi hợp nhất A (10 tệp
  parity/`ktest` gọi core). Phép đo có thẩm quyền là sau hợp nhất.
- `v2/tools/check-wake-paths.py` (luồng C) sẽ thấy `NON_WAKE_READY_SITES`/`WAKE_DOORS` trỏ
  `v2/sim/vifast-sim/src/kobj.rs` cho `transition_tcb`/`wake_step` — hai hàm nay nằm ở
  `kobj/state_write.rs`. Không sửa `v2/tools/` (ngoài phạm vi B).
