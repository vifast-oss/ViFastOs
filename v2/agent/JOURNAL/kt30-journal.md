# KT-30 — `vifast-sync`: MCS lock + Mailbox MPSC cố định (K1c, D2-53 Q9)

**Vào**: READY (`agentctl next --project v2` trả KT-30) · HEAD `27f9a8d` ([KT-00] lật KT-29 DONE,
442 test, D2-59) · `git rev-list --count HEAD..origin/main` = **0** · working tree có thay đổi
`viscript/` của tab khác (**không chạm**, chỉ stage `v2/`).

**Nguồn ngữ nghĩa**: `review/agent0/K1C-PLAN.md` §5 KT-30 (DoD 1..8); **D2-53 Q9 phương án (a)**
(stress host có biên + Miri, **không** loom, **không** explorer tự viết; (c) thêm `loom` **bác**);
D2-17 (crate `vifast-sync` **đã** nằm trong danh sách đóng được `unsafe`); D2-21 (không dependency
ngoài, MIT OR Apache-2.0); D2-42 (`no_std` + test dùng `std`); D2-07 (không retry); thiết kế §4.16
(`:404-419`, S1 BKL + 5 luật), §4.17 (`:421-428`, hộp thư theo hart).

**Không làm** (đúng brief): không loom/explorer/dependency; không thread trong kernel/sim; không
sửa IPC/Notification/WaitAddr/SMP sim; không `vifast-model`, `spec/`, `.scn`, QUEUE/DECISIONS/
AGENTS, `viscript/`; không `#[allow]`/`#[ignore]`; không tuyên bố toàn crate lock-free.

## 1. Test đỏ trước (D2-05)

- [`kt30-red-compile.log`](kt30-red-compile.log) — `tests/kt30_sync.rs` viết trước khi có code:
  **E0432** `unresolved imports vifast_sync::{Busy, Mailbox, McsLock, Msg, MsgKind, PowerState}`
  + **E0425** `MAX_WAITERS` (crate mới chỉ có khung rỗng). Ngữ nghĩa "mailbox đầy phải `Busy`,
  không ghi đè" nằm trong chính các test đỏ đó (`mailbox_day_tra_busy_khong_mat_message_dau`,
  `mailbox_backpressure_1000_vong`) — không biên dịch được ⇒ đỏ, đúng K1C-PLAN §5 KT-30.

## 2. Thay đổi (chỉ `v2/`)

| Tệp | Nội dung |
|---|---|
| `v2/lib/vifast-sync/Cargo.toml` **(mới)** | crate `vifast-sync`, `no_std`, **không** dependency/feature |
| `v2/lib/vifast-sync/src/lib.rs` **(mới)** | `#![no_std]`, `Busy` (+`Display`), re-export; 40 dòng |
| `v2/lib/vifast-sync/src/mcs.rs` **(mới)** | `McsLock<T>`: `lock() → Guard` (Drop mở khoá), `try_lock()`, FIFO MCS, pool node cố định `MAX_WAITERS = 16`, bitmap slot, `queue_len`/`next_owner` + API test nội bộ |
| `v2/lib/vifast-sync/src/mailbox.rs` **(mới)** | `Mailbox<N>` MPSC vòng biên (seq theo slot), `Msg` **7 loại** + payload `[TK]`, `send → Result<(), Busy>`, `try_recv`/`drain` FIFO, `generation`/`ack`/`pending` (`AtomicU32`) |
| `v2/lib/vifast-sync/tests/kt30_sync.rs` **(mới)** | 12 test single-thread (Miri): lock/unlock, `try_lock` khi giữ ⇒ `None`, FIFO qua `queue_len`/`next_owner`/`test_*`, **không reentrant**, ticket; mailbox 7 loại/FIFO/đầy ⇒ `Busy`/không mất đầu/1.000 vòng/ack-pending/`Busy` không tiêu thế hệ |
| `v2/lib/vifast-sync/tests/kt30_stress.rs` **(mới)** | 3 test đa luồng (`#![cfg(not(miri))]`): 4 luồng × 500 op (mutual exclusion + không mất message), FIFO có điều khiển, `try_lock` tranh chấp; biên `[ƯL]` 3 s |
| `v2/Cargo.toml` | thêm member `lib/vifast-sync` |
| `v2/bin/v2` | **1 dòng** `run_miri_test "vifast-sync (lib + kt30_sync)" -p vifast-sync` trong `nightly --only-miri` |
| `v2/docs/k1c-sync.md` **(mới)** | tài liệu model-only `[TK]`: API, thuật toán, vì sao pool node, bậc bộ nhớ, 7 loại message, ngữ nghĩa thế hệ/ack, kiểm chứng (Q9(a)), giới hạn/hoãn |
| `v2/agent/JOURNAL/kt30-*` | log đỏ/baseline/check/miri/gate |

**Quyết định có thể gây tranh luận** (ghi rõ thay vì ẩn):
1. **MCS dùng pool node cố định trong chính khoá** (`[Node; 16]`, liên kết bằng **chỉ số slot +
   1**, không con trỏ thô) thay vì node trên stack: `Guard` trả về **theo giá trị** sẽ di chuyển
   node sau khi địa chỉ đã công bố cho node trước ⇒ con trỏ treo/UB (đúng rủi ro (a) của KT-30).
   Pool cố định giữ địa chỉ bất động, `Guard` chỉ giữ `usize`; đây là biến thể "node tĩnh theo
   CPU" của MCS trong kernel. **Không** `Pin`, **không** cấp phát, **không** `unsafe` cho node.
2. **`Ordering::SeqCst`** cho `tail/head/next/locked` (bitmap slot: `AcqRel`/`Release`). K1c
   **không** có mục tiêu hiệu năng và cần mô hình dễ chứng minh; hạ bậc để dành cho mốc đo hiệu
   năng (ghi trong `k1c-sync.md` §2.3, có điều kiện kèm test/loom sau này).
3. **Ngữ nghĩa ack `[TK]`** (`k1c-sync.md` §3.3): `generation` = số message `send` **Ok**;
   `ack(gen)` = `fetch_max` (không lùi, kẹp về `generation`), trả số message mới được ack;
   `pending(gen)` = `min(gen, generation) - acked`. `send` trả `Busy` **không** tiêu thế hệ.
4. **`Mailbox` yêu cầu `N ≥ 2`**: `seq` của slot mã hoá cả `pos+1` (đặt chỗ) và `pos+N` (đã rút);
   `N = 1` làm hai giá trị trùng nhau ⇒ không phân biệt đầy/trống. `new` `assert!(N >= 2)`.
5. `kt30_stress.rs` gạt khỏi Miri bằng `#![cfg(not(miri))]` (DoD 6 cho phép; lý do ghi đầu tệp);
   `mailbox_backpressure_1000_vong` giảm còn **40 vòng dưới Miri** bằng `cfg!(miri)` (stable
   vẫn **1.000 vòng** — đúng DoD 4; đây **không** phải nới ngưỡng cổng, chỉ giảm khối lượng cho
   Miri, cùng quy ước các test `kt19..kt29` đã dùng).
6. Luật R2 (`grep 'cfg(target_arch'`) từng bắt **câu doc** mô tả "không dùng cfg theo kiến trúc"
   của tôi (không phải code) — đã sửa câu chữ để cổng xanh; kiểm lại không còn mẫu cấm.

## 3. Kết quả đo (log trong `v2/agent/JOURNAL/`)

| Hạng mục | Kết quả | Log |
|---|---|---|
| Test đỏ trước | E0432 (6 item) + E0425 `MAX_WAITERS` | `kt30-red-compile.log` |
| Baseline trước KT-30 | `gate --fast` **PASS 442 test**, L1 1.000 seed/999 trace (648 ms) | `kt30-baseline-gate-fast.log` |
| `v2 check` | **PASS** (fmt, clippy `-D warnings`, R1..R5, ABI/scn/gen-check, build) | `kt30-check.log` |
| `gate --fast` (sau sửa DRIFT) | **PASS 461 test** (+19 = 4 unit lib + 10 integration + 3 stress + 2 doctest), L1 1.000 seed/999 trace **750 ms** (cổng đo), **wall 199,3 s** | `kt30-gate-fast.log` |
| Miri `-p vifast-sync` (sau sửa DRIFT) | **PASS 4 unit + 10 test + 2 doctest**, 13,5 s | `kt30-miri.log` |
| Crate test riêng | 4 unit + 10 integration + 3 stress + 2 doctest, **1,42 s** cả crate; **thân** 3 test stress **≈2,6 ms** | `kt30-gate-fast.log` (mục L0 test) |
| Stress 4 × 500 op | `send Ok 1978–1984` (phần còn lại `Busy`, `Ok + Busy = 2000`), `drain == send Ok` **từng hart**, bộ đếm mutual exclusion **= 2000**, **≈2,6 ms** (biên 3 s) | `kt30-stress-200.log` |
| Stress chạy lặp **200 lần** | **200/200 PASS, 0 fail** (sau khi sửa assertion sai — verdict mục 1) | `kt30-stress-200.log` |
| FIFO đa luồng có điều khiển | thứ tự trao `[B, C]` đúng thứ tự xếp hàng | `kt30-stress-200.log` |
| `try_lock` tranh chấp | 1.5–1.7 k lần acquire, **không** lần nào 2 chủ cùng lúc | `kt30-stress-200.log` |

**T-INFRA (tải máy, không nới ngưỡng)**: tổng `gate --fast` wall đo **199,3 s** (lần cuối; lần
trước 216,7 s) trong khi QD §5.2 đặt ≤ 60 s; reviewer đo **202–216 s**. Nguyên nhân là tải máy
(load avg 3,7–5,7, 12 core, các tab khác chạy song song): log **baseline trước KT-30** trên cùng
máy đã có **tổng thời gian test 159,5 s** (gate sau KT-30: 185,0 s; phần tăng nằm ở test sẵn có +
nhiễu tải, không phải crate mới — bộ test mới **1,42 s** cả crate, thân test mới **≈2,6 ms**).
Không giảm seed/ngưỡng nào, **không** gọi là "đạt 60 s"; ghi `T-INFRA` theo D2-07. Số wall đã ghi
thẳng vào `kt30-gate-fast.log` (`# gate_wall_sec = 199.3`).

## 4. Đối chiếu DoD 1..8 (K1C-PLAN §5 KT-30)

| # | DoD | Kết quả |
|---|---|---|
| 1 | Crate `v2/lib/vifast-sync`, `no_std`, SPDX, ≤1.500 dòng, member `v2/Cargo.toml`, **không** `cfg(target_arch)`, **không** dependency ngoài; `unsafe` được phép + gói nhỏ + Miri sạch | ✅ **1.346 dòng** cả crate (src 845, test 501); SPDX `//` mọi tệp; 1 member; R2 OK; 0 dependency; **8 khối `unsafe`** (4 impl + 4 thao tác) đều có `SAFETY`; Miri PASS |
| 2 | `McsLock<T>`: `lock() → Guard`, `try_lock()`, `Drop` mở khoá, **FIFO**, `try_lock` khi đã giữ ⇒ `None`, **không** reentrant (test rõ) | ✅ `mcs_lock_unlock_mutates_value`, `mcs_try_lock_khi_dang_giu_tra_none`, `mcs_ticket_tang_theo_lan_xep_hang` (API công khai) + **unit test trong crate**: `fifo_theo_thu_tu_xep_hang` (FIFO qua `queue_len`/`next_owner` + hook), `khong_reentrant`, `pool_can_muoi_sau_slot_...`, `free_mask_khong_tran` |
| 3 | `Mailbox<const N>`: `Msg` **7 loại** + payload hart/tcb; `send → Result<(), Busy>`; `drain` FIFO; `ack(gen)` theo `AtomicU32`; `pending(gen)` | ✅ `mailbox_du_7_loai_fifo` (so `MsgKind::ALL` 7/7), `mailbox_the_he_ack_pending`, `mailbox_len_rong_va_capacity` |
| 4 | Đầy ⇒ backpressure: `N+1` ⇒ `Busy`, **không** mất đầu; sau `drain` gửi lại được; **1.000 vòng** gửi/đầy/rút | ✅ `mailbox_day_tra_busy_khong_mat_message_dau`, `mailbox_backpressure_1000_vong` (8.000 message Ok + 1.000 `Busy`) |
| 5 | Stress host **4 thread × 500 op** `lock/unlock` + `send`, biên `[ƯL]` ≤3 s; assert mutual exclusion + không mất message; **không** assert trace/thứ tự; flake ⇒ `T-INFRA` | ✅ `stress_4_luong_500_op_...` (≈2,6 ms), `stress_fifo_khi_xep_hang_co_dieu_khien`, `stress_try_lock_khong_cho_hai_chu_cung_luc`; chỉ bất biến |
| 6 | Miri: test single-thread PASS `v2 miri -p vifast-sync`; stress **không** chạy Miri | ✅ 4 unit + 10 integration + 2 doctest (`kt30-miri.log`); `kt30_stress.rs` có `#![cfg(not(miri))]` |
| 7 | **Không** thay loom bằng explorer; **không** thêm `loom` | ✅ không dependency mới, không explorer (Q9 (a)) |
| 8 | `v2 nightly --only-miri` thêm `-p vifast-sync` (1 dòng); `gate --fast` PASS, test mới ≤3 s, tổng cổng ≤60 s (đo + ghi log) | ✅ 1 dòng; gate PASS; test mới 1,42 s cả crate/≈2,6 ms thân stress; ⚠️ tổng cổng **216,7 s do tải máy** (baseline đã 159,5 s tổng test) — `T-INFRA`, xem §3 |

## 5. Verdict `reviewer-kt30` — **DRIFT** (thuật toán MCS/Mailbox đạt) — đã sửa 2/2 chặn + 4 việc

1. **Assertion sai trong stress `try_lock`** (`kt30_stress.rs`): mỗi luồng trả `inside.load()` rồi
   main `assert_eq!(join, 0)` **sau từng** join — không phải bất biến (luồng khác có thể đang giữ
   khoá); reviewer đo **93/200 lần fail (46,5%)**. `now == 1` bên trong miền **không** bắn ⇒ mutual
   exclusion vẫn đúng, chỉ assertion sai. **Sửa**: closure trả `()`; **sau khi join hết** mới
   `assert_eq!(inside.load(SeqCst), 0)`; giữ `now == 1`. Chạy lặp **200 lần: 0 fail**.
2. **Hook test `pub`** (`McsLock::test_release`/`test_enqueue`/`test_woken`, `Guard::slot`): code
   ngoài crate (safe) mở khoá hộ slot ⇒ hai `Guard` cùng lúc ⇒ alias `&mut T`/data race = **UB từ
   safe code**; `#[doc(hidden)]` **không** đủ. **Sửa**: `pub(crate)` **+ `#[cfg(test)]`**; test
   FIFO/không-reentrant chuyển vào `#[cfg(test)] mod tests` của `src/mcs.rs`; integration test chỉ
   dùng API công khai.

Làm luôn (không chặn): (a) unit test **pool cạn** 16 slot ⇒ `try_lock() == None`, không panic,
hồi phục sau khi mở hết; (b) `free_mask(n)` nhận tham số + test `n ≥ 64` không tràn + `const _`
assert `MAX_WAITERS ≤ usize::BITS`; (c) `k1c-sync.md`: `acked` là **lời khai người nhận**
(`acked ≠ delivered`, việc K3) + giới hạn `release` quay vòng chờ `next` khi người kế bị preempt;
(d) ghi `gate_wall` vào `kt30-gate-fast.log` + T-INFRA (không nới ngưỡng).

Sau sửa: `cargo test -p vifast-sync` **19/19 PASS** (4 unit + 10 integration + 3 stress + 2 doctest);
stress **200 lần / 0 fail**; `gate --fast` **461 test PASS**; Miri **16/16 PASS** (13,5 s).

## 6. Tệp / dòng và bước tiếp

- Tệp thay đổi: xem §2 (chỉ `v2/`). Không chạm `viscript/`, `spec/`, QUEUE/DECISIONS/AGENTS,
  `vifast-model`, `.scn`.
- Bước tiếp: verdict → sửa (nếu có) → commit `[KT-30]` **`29718a6`** + push `origin/main` → **không** lật QUEUE
  (đề xuất Agent 0: KT-30 DONE/REVIEW) → cổng cuối K1c (§7.1: 100.000 seed × smp 1/2/4/8) là
  **task riêng**, chưa phải KT-30.
