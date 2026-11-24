<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
> **KHÔNG PHẢI DUYỆT.** Đây là đầu vào để Agent 0 quyết. Duyệt chỉ có giá trị bằng dòng
> `APPROVED <YYYY-MM-DD> Agent 0` ghi trong `v2/agent/QUEUE.md` (AGENTS.md §8).
> Planner độc lập cho phase **K1c**; **chỉ tạo tệp này** — không sửa code / QUEUE / DECISIONS /
> `spec/` / `v2/` / `viscript/`, không commit, không cài gì.

# K1C-PLAN — kế hoạch phase K1c (IPC/Reply/Notification/WaitAddr/Exit + SMP/push migration)

**Lập ngày** 2026-11-13 · **HEAD khi lập** `2657ee0` (`[ST-00]` của dòng **ViScript** — không liên
quan K1c) · **K1b chưa đóng**: KT-18 DONE (`5cccb72`, theo `STATE.md`), KT-19 xong mã nhưng **chưa
commit** (cây làm việc có `v2/sim/vifast-sim/src/sched.rs` + `tests/kt19_sched.rs` chưa track, log
`kt19-gate-fast.log` PASS), KT-20..KT-23 **chưa**. Mọi task dưới đây **chưa mở**: chờ Agent 0 duyệt
kế hoạch + chốt §6, và chờ **K1b đóng** (KT-23 DONE).

---

## 0. TL;DR

| Việc | Kết luận |
|---|---|
| Định nghĩa xong K1c | Thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:875`: *IPC + Reply + Notification + WaitAddr + fault + **Exit*** — tiêu chí: **bảo toàn message**, **không mất đánh thức**, **mọi đột biến chuẩn bị bắt**, **100.000 seed × smp 1/2/4/8** |
| ABI đã có đủ chưa | **Đủ cho IPC/Notification/WaitAddr/Exit**: 16 syscall (`spec/abi.toml:105-121`), MessageInfo 35 bit (`:40-49`), 64 MR + **4 cap** (`:31-38`), ReplyRecv/NBSendRecv/NBSendWait (`:109-111`), `Exit` (`:120`), push migration/SMP là **mô hình** (không phải ABI) |
| Khoảng trống lớn nhất | (1) **Không có `MessageInfo`/MR/cap transfer** — `call` chỉ mang **một** `mr: u64` (`kobj.rs:2138`), `EndpointState.last_mr: Option<u64>` (`:268`); (2) `no_reply` trả `IllegalOperation` thay vì `Cancelled` (`kobj.rs:2150-2151`, trái `thiết kế:322`); (3) `Notification` chỉ có `waiter`, **không** word/badge/Signal (`kobj.rs:334-336`); (4) `WaitAddr` xếp hàng **không** so giá trị, **không có** `WakeAddr` (`kobj.rs:2066-2080`); (5) `HART_COUNT = 1` cứng (`kobj.rs:73`), chưa có hộp thư/Migrate (KT-22 chưa làm); (6) **không có** `vifast-sync` (MCS lock/lock-free) và **không có loom** trong registry |
| Đề xuất | **7 task `KT-24..KT-30`**, tổng **19,5 h** `[ƯL]`, mỗi task ≤ 3 h, tuần tự trên `kobj.rs`; **KT-30 chạy song song được** (crate riêng, không chạm `v2/sim/**`) |
| Cổng cuối phase | `100.000 seed × smp 1/2/4/8` + mutation tay + Miri + bảng DoD K1c `[ƯL] 1,5–2 h` — **chưa phải task**; nếu Agent 0 muốn tách thì cấp `KT-31` (xem §7) |
| Cần Agent 0 chốt | **10 câu chính + 5 câu nhỏ §6**; nặng nhất: **Q1** (SMP mô hình tất định tới 8 hart), **Q3** (`spec/kernel.toml` chưa tồn tại ⇒ bucket WaitAddr/`MAX_HARTS` lấy ở đâu), **Q9** (loom không có ⇒ kiểm MCS lock/lock-free bằng gì) |
| Không đụng | Không sửa `spec/`, `v2/lib/vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; K1c **không** làm K2/K3 (Frame, TLB, QEMU, conformance, procsrv shell) |

---

## 1. Tiêu chí K1c và ràng buộc bắt buộc

### 1.1 Định nghĩa xong (nguồn: thiết kế `:875`) — 4 mệnh đề, mỗi mệnh đề phải có log

| # | Mệnh đề | Cách chứng minh (đo được) |
|---|---|---|
| C1 | **Bảo toàn message** | Mỗi message có `msg_id` `[TK]`; bất biến `check_ipc()`: mỗi `msg_id` xuất hiện **đúng một** nơi (hàng gửi endpoint ⎮ slot nhận ⎮ đã giao cho TCB) sau **mỗi** op; 0 vi phạm trên 1.000 seed `--fast` và 100.000 seed nightly |
| C2 | **Không mất đánh thức** | Property: nếu `Signal`/`WakeAddr` xảy ra **sau** khi waiter xếp hàng thì waiter **phải** được đánh thức; nếu xảy ra **trước** thì trạng thái (word/giá trị) phải còn để lần Wait sau thấy — kiểm trên mọi thứ tự bước do interleaver sinh (1.000 seed) |
| C3 | **Mọi đột biến chuẩn bị bắt** | Danh sách đột biến tay (bảng §7.2), mỗi đột biến phải làm **đúng một** test đỏ; log đỏ dán `JOURNAL`, revert, ghi `mutants: killed` |
| C4 | **100.000 seed × smp 1/2/4/8** | 4 lần chạy 100.000 seed (smp = 1, 2, 4, 8), cùng seed ⇒ cùng trace, bất biến giữ; log + thời gian đo bằng lệnh cổng (§7.1) |

### 1.2 Hợp đồng ABI K1c **tiêu thụ** (không sửa)

| Hạng mục | Nội dung | Nguồn |
|---|---|---|
| 16 syscall | `Call, Send, NBSend, Recv, NBRecv, ReplyRecv, NBSendRecv, NBSendWait, Yield, WaitAddr, WakeAddr, ClockRead, DebugPutStr, DebugHalt, Exit, DebugGetInfo` | `spec/abi.toml:105-121` |
| MessageInfo | **35 bit** một thanh ghi: `label:20 ⎮ caps:3 (≤4) ⎮ no_reply:1 ⎮ extra_caps_unwrapped:4 ⎮ len:7 (≤64)`; thứ tự bit `label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27, length:28..34` | `spec/abi.toml:40-49`, thiết kế `:316` |
| MR | **64 MR**: 4 ở thanh ghi (`words_in_registers = 4`), 60 trong IPC buffer (`words_in_ipc_buffer = 60`), buffer 4 KiB/căn 16 | `spec/abi.toml:31-38` |
| **4 cap** | `max_caps = 4`; chuyển cap **cần GRANT**; cap nhận **bỏ GRANT**; cap cùng endpoint + badge **unwrap** thành badge; **thất bại không mất message** | thiết kế `:319`, `spec/abi.toml:34` |
| Receive window | CPTR (địa chỉ cap giải qua CSpace **của thread nhận**), tối đa 4 cap; kernel kiểm **slot trống** + **quyền ghi** trước khi cài | thiết kế `:318` |
| ReplyRecv | server nóng: `Send(reply)` + `Recv(ep, reply_cap)` gộp; Reply **do bên nhận cấp**; **không có** `Tcb.SetReplyObject` | thiết kế `:320`, QD §3.2 |
| `no_reply` | Call gặp bên nhận đã `Recv` **không** Reply ⇒ message **vẫn giao**, cờ bật phía nhận, caller trả ngay **`Cancelled`** | thiết kế `:322` |
| Notification | Word **64 bit**; `Signal` **OR badge**, đánh thức **một** waiter, giao word, **xoá word**; Wait/Poll; bound với TCB **và** SchedContext; **không** "đánh thức tất cả" (khác v1 có chủ ý) | thiết kế `:328`, QD §3.11 |
| WaitAddr | Bảng băm **trong arena kernel**, khoá `(frame_id, offset)`, số bucket theo `kernel.toml`; `WaitAddr(frame_cap, offset, expected, timeout)`: **đọc giá trị + xếp hàng trong cùng một lần giữ khoá**; `WakeAddr` cũng giữ khoá ⇒ **không mất đánh thức** | thiết kế `:330-334`, QD §3.8 |
| Exit | syscall **14**: lưu `exit_code`; có `fault_ep` ⇒ `BlockedOnSend{call:false, fault=Exit{code}}` (giao qua hàng gửi, **không Reply**); procsrv `Recv` xong ⇒ `Inactive`; không `fault_ep` ⇒ `Inactive` ngay; `Resume`/`Configure` xoá `exit_code` | `spec/abi.toml:120`, thiết kế `:469` (hàng 14) + `:285`, QD §1 Q-A, D2-25 |
| Fault khác Exit | `Call` tới fault endpoint **với Reply**; trả lời Reply để **tiếp tục**, có thể kèm thanh ghi mới; loại: **VM, unknown syscall, user exception**, timeout (MCS), Exit, VM exit (H1) | thiết kế `:321` |
| Invocation | `Call` tới object **không phải Endpoint** = invocation (method = label), **trả ngay**, **không** sinh Reply | thiết kế `:323` |
| Một hàm lõi | `ipc::transfer` — copy MR + cap **một chỗ**; fastpath sau này phải gọi cùng hàm | thiết kế `:324` |
| SMP/push | Mỗi hart **công bố** `idle`/`cur_prio` (đọc được, **ghi** thì cấm); X có **>1** TCB Ready **cùng mức cao nhất** + Y trong affinity **idle** ⇒ đẩy **một** TCB qua hộp thư `Migrate`; **tất định** (hart rảnh **chỉ số nhỏ nhất**); **không** work stealing | thiết kế `:340-342`, QD §3.10 |
| MCS lock/lock-free | `vifast-sync`: MPSC cố định (`Wake/TlbFlush/Reschedule/Migrate/Stop/VmKick/PowerReq`), ack bằng **bộ đếm thế hệ**, đầy ⇒ người gửi xử lý hộp thư mình rồi thử lại; kiểm bằng **loom nếu có, không thì stress** + Miri phần không atomic | thiết kế `:423-428`, `:407-408` |

### 1.3 Bảng ràng buộc

| Ràng buộc | Nội dung | Hệ quả cho K1c |
|---|---|---|
| **D2-49** (`DECISIONS.md:85`) | (a) 1 hart + **hoãn push migration sang K1c**; (b) `set_sched_params` model API; (c) donation tự động; (d) bảng `ALLOWED` model-only; (e) budget chu kỳ thẳng hàng; (f) **K1b không MCS lock/lock-free** ⇒ **vifast-sync/loom để K1c/SMP**; (g) không mở rộng `vifast-model`; (h) tick ảo | K1c **kế thừa** (a)(f): push migration + `smp 1/2/4/8` + `vifast-sync` là **của K1c**; (g) chặn "so mô hình IPC" (§9.2 `:748`) — xem Q9 |
| **D2-25** | `FaultKind::Exit{code}`; `Suspend(Abort)` khi đang chờ giao Exit **huỷ message** nhưng `exit_code` vẫn đọc được | KT-26 kiểm **cả hai** chiều (huỷ message + đọc được mã) |
| **D2-33** | Enum TCB **đầy đủ 11 tên**; `BlockedOnSend.fault: Option<FaultKind>`; `BlockedOnRecv{ep, reply}` | K1c **không** thêm trạng thái nào; chỉ dùng `BlockedOnNotification`/`BlockedOnAddress` (đã có) |
| **Thiết kế §4.13** `:382-387` | Preempt ép tại **mọi bước**, trạng thái trung gian giữ invariant | IPC nhiều bước (transfer MR + cap + giao) phải giữ `check_ipc`/`check_sched` sau **mỗi** op; test bật `preempt_every_step` |
| **Thiết kế §4.16** `:412-419` | 5 luật; **không ghi** trạng thái hart khác; **đọc** `idle`/`cur_prio` được phép; build sim có **guard** ghi lại hart truy cập `PerHart`, vi phạm là panic | KT-29 cài guard (API trả `Err` để test được, kèm `debug_assert!`) và test âm "ghi trường công bố hart khác" |
| **Thiết kế §4.17** `:425-428` | MPSC cố định, ack thế hệ, đầy ⇒ backpressure; **lock-free duy nhất**; kiểm loom/stress + Miri | KT-30; **loom không có** ⇒ phải chốt Q9 |
| **Thiết kế §9.2** `:748-750` | IPC/Reply: 4 cap, unwrap badge, **Reply bị xoá**, **tái tạo đường mất message của v1**, "so mô hình"; Notification: OR badge, **một** waiter, bound + Recv; WaitAddr: **không mất đánh thức**, đột biến "bỏ kiểm giá trị dưới khoá" | DoD của KT-24/25/26/27/28 lấy **đúng** các mục này |
| Đường mất message v1 | *"Không ô `resume`: message ghi thẳng vào context + IPC buffer rồi mới chuyển Ready. Đường mất message kiểu r319 không biểu diễn được"* | KT-26 test âm: fault-resume **phải** ghi MR/registers **trước** khi `Ready`; đột biến "resume không ghi" phải bị bắt |
| **D2-17 + thiết kế §8** `:684,695,702` | `unsafe` chỉ trong danh sách crate đóng: `vifast-sync(*)`, `vifast-objmem(*)`, `kernel/vifast-plat-*`, `boot/`; **crate mới có `unsafe` phải qua Agent 0** | `vifast-sync` **đã nằm trong danh sách** ⇒ KT-30 tạo crate được; mọi code `v2/sim/**` vẫn **0 unsafe** |
| **D2-21** | License `MIT OR Apache-2.0`, không dependency ngoài allowlist | KT-30 **không** thêm dependency (chỉ `core::sync::atomic`); stress dùng `std::thread` của test |
| **QD §5.1/§5.2** | Task ≤ 3 h; `gate --fast` ≤ 60 s, L1 1.000 seed; nightly 100.000 | Mỗi task đóng bằng `gate --fast`; 100k chỉ ở cổng cuối phase (§7.1), **không** nhét vào `--fast` |
| **QD §5.3 + §9 (câu 12 của dossier)** | Số đo chỉ do `./v2/bin/v2 nightly` sinh; không số đo viết tay vào tài liệu | C4 phải chạy bằng lệnh cổng, không chép số tay |
| **D2-05/D2-07** | Test đỏ trước; không retry để qua cổng | Mỗi task có mục "test đỏ trước" + log |

### 1.4 K1c **không** phải K2/K3

- **Không** làm Frame/VSpace/PageTable/ASID/shootdown/TLB (K2) — nên WaitAddr khoá theo `(ObjId frame, offset)`
  như K1b, **không** theo địa chỉ vật lý; "Frame chia sẻ hai tiến trình" (§9.2 `:750`) và "Mutex std stress"
  (L3) **hoãn K2/K3** — ghi rõ, không tuyên bố đạt.
- **Không** QEMU/ktest/conformance/perf (K3); **không** procsrv/shell thật (K3.5, `:878`).
- **Không** IrqHandler → Notification thật (K3): K1c chỉ có `signal` gọi từ model (mô phỏng IRQ bằng API).
- **Không** phát bản ghi trace 32 B: `vifast-sim` **chưa** phụ thuộc `vifast-traceformat` (kiểm
  `v2/sim/vifast-sim/Cargo.toml`); trace của model vẫn là scenario log (D2-38), bản ghi `kind=2/4` thuộc K3 (D2-40).
- **Không** mở rộng `vifast-model` (D2-49(g)) — trừ khi Agent 0 chốt khác (Q9).

---

## 2. Hiện trạng code (đọc, không sửa) — cái gì có, cái gì thiếu

| Thành phần | Vị trí | Hiện trạng | K1c cần |
|---|---|---|---|
| `MessageInfo` | — | **không có** (grep `MessageInfo` trong `v2/lib` + `v2/sim` = 0 hit) | Struct 35 bit + pack/unpack theo `spec/abi.toml:40-49` |
| Message/MR | `kobj.rs:265-269` (`EndpointState { recv, send: VecDeque<(ObjId, Option<FaultKind>)>, last_mr: Option<u64> }`), `call(tcb, ep, mr: u64)` `:2138` | **1 word** duy nhất; không label/len/caps | `Message { info, mr: [u64;64], caps, badge, msg_id }` + IPC buffer model |
| Cap transfer | — | **không có**; `Cap { rights, badge }` có `RIGHT_GRANT` (`kobj.rs:61`) nhưng không dùng cho IPC | Transfer 4 cap: GRANT, bỏ GRANT khi nhận, unwrap badge, receive window, cờ thất bại |
| `no_reply` | `kobj.rs:2150-2151` | `if has_waiter && reply_cap.is_none() { return Err(KError::IllegalOperation) }` — **trái** `thiết kế:322` | Giao message + `Cancelled` cho caller; hai chiều (caller đặt cờ = `.scn` 17) |
| `send` / `nb_send` / `nb_recv` | — | **không có** (chỉ `call`/`recv`/`reply`/`exit`; comment `kobj.rs:1607` chỉ nhắc tên) | 4 syscall + `nb_send_recv`/`nb_send_wait` |
| `reply_recv` | — | **không có** | Đường nóng server: `Send(reply)` rồi `Recv(ep, reply_cap)` |
| Reply lifecycle | `kobj.rs:346-348` (`ReplyData { caller: Option<ObjId> }`) | `Suspend(Abort)` tách caller (KT-18); **chưa** có ngữ nghĩa `Delete/Revoke` Reply khi caller đang chờ | Chốt Q8 + test `.scn` 07/08 |
| Fault | `tcb.rs:21-23` (`pub enum FaultKind { Exit { code } }`) | **chỉ `Exit`**; `spec/abi.toml:268-271` cũng chỉ `Exit` | VM/unknown syscall/user exception + Reply-resume (Q5) |
| Notification | `kobj.rs:334-336` (`NotificationState { waiter: Option<ObjId> }`) | **không** word/badge/Signal; `tcb_wait_notification` `:2038` chỉ đặt waiter | `word: u64`, `signal(badge)`, OR + 1 waiter + xoá word, Wait/Poll, bound SC/TCB |
| WaitAddr | `kobj.rs:471` (`address_waiters: Vec<(ObjId, u32, ObjId)>`), `tcb_wait_address` `:2066-2080` | xếp hàng **không** so giá trị; **không** có `WakeAddr`; tuyến tính (không bucket) | `wait_addr(expected)` + `wake_addr(n)` + bucket `(frame,offset)` + no lost wake |
| Exit | `kobj.rs:2287-2330` | **đã đủ kernel-side** D2-25 (fault_ep ⇒ `BlockedOnSend{call:false,fault:Exit}`; Recv xong ⇒ `Inactive`; `Suspend(Abort)` huỷ message) | Chỉ thiếu **mô hình procsrv** (`wait`→`{code, how}`) + `.scn` 13/14/20 chạy thật |
| Scheduler | `sched.rs` (267 dòng, `HartSched`: 256 mức + bitmap, `pick` O(1)) | **1 hart cứng**: `HART_COUNT: u16 = 1` (`kobj.rs:73`); `sched: Vec<HartSched>` đã theo hart | `with_harts(1/2/4/8)`, published `idle`/`cur_prio`, `Migrate`, push |
| Hộp thư | — | **chưa có** (KT-22 chưa làm; `v2/sim/vifast-sim/src/` chỉ có `kobj/lib/main/scenario/sched/tcb`) | KT-29 tiêu thụ hộp thư **mô hình** của KT-22 + `Migrate` |
| `vifast-sync` | — | **không có** crate (`v2/lib/` = abi, elf, fs, fsutil, model, objmem, traceformat) | KT-30 tạo: `McsLock` + `Mailbox` (MPSC) |
| loom | registry `~/.cargo/registry/src/index.crates.io-*/` | **không có `loom`** (chỉ crossbeam/atomic của công cụ khác) | Q9: stress + Miri, hoặc tự viết explorer |
| Nền test K1b | `kt19-gate-fast.log` | `gate --fast` PASS, **263 test / 43 suite** (tổng `test result: ok. N passed` trong log), L1 1.000 seed/999 trace (657 ms) | K1c **không** được giảm dưới 263; cộng test mới mỗi task |

**Kết luận phạm vi**: K1c là **thay ruột IPC** (từ 1 word → message 64 MR + 4 cap + badge + ReplyRecv +
fault) và **dựng SMP/hộp thư lock-free**, trong khi vẫn xanh 263 test, L1 1.000, Miri và parity 10k (hồi quy).

---

## 3. Phân rã theo trục (khớp 6 nhóm việc của brief)

| Trục | Nội dung K1c | Task | Ghi chú |
|---|---|---|---|
| **IPC core 1** | `MessageInfo`, Message 64 MR + IPC buffer model, **một hàm `ipc::transfer`**, `Send/NBSend/Recv/NBRecv`, `no_reply` (2 chiều), Call trên non-Endpoint **không chặn** | **KT-24** | Xương sống: mọi đường giao message đi qua `transfer` |
| **IPC core 2** | `Call/Reply/ReplyRecv/NBSendRecv/NBSendWait`, **4 cap** (GRANT/bỏ GRANT/unwrap badge/receive window), vòng đời `Reply` (bị xoá/revoke), donation không rò SC | **KT-25** | Phụ thuộc KT-24 + KT-21 (donation) |
| **Fault + Exit/procsrv** | `FaultKind` VM/unknown/user + fault qua **Call+Reply** + **resume có thanh ghi mới** (đường r319), `Exit` D2-25 + **mô hình procsrv** `{code, how}` | **KT-26** | Test âm "resume không ghi message ⇒ mất message" |
| **Notification** | Word 64 + badge, Signal OR + **một** waiter + xoá word, Wait/Poll, bound TCB + `SchedContext.Bind` | **KT-27** | `.scn` 21 |
| **WaitAddr** | Bảng băm `(frame, offset)`, đọc giá trị + xếp hàng **một lần giữ khoá**, `WakeAddr(n)`, không mất đánh thức | **KT-28** | Đột biến "bỏ kiểm giá trị dưới khoá" (§9.2 `:750`) |
| **SMP + push migration** | `with_harts(1/2/4/8)`, published `idle`/`cur_prio` + guard luật 3, hộp thư `Migrate`/`Wake`, push **tất định**, **không** work stealing | **KT-29** | Tiêu thụ hộp thư KT-22; driver L1 theo `smp` |
| **MCS lock / lock-free** | `vifast-sync`: `McsLock` + `Mailbox` MPSC cố định (7 loại message, ack thế hệ, đầy ⇒ backpressure), stress + Miri | **KT-30** | **Song song được** (crate riêng) |

### 3.1 Hai nghĩa của "MCS" — không lẫn

- **MCS giai đoạn đầu** = `SchedContext`/budget/donation → **K1b** (KT-20/21), K1c chỉ **tiêu thụ**.
- **MCS lock** = queue lock của BKL (`thiết kế:407`) + **hộp thư lock-free** (`:423-428`) → **K1c** (KT-30).
  Trong sim **không** có khoá (một luồng, tất định) ⇒ crate kiểm trên **host**, nối vào platform là việc K3.

---

## 4. Bảng task đề xuất

| ID | Mục tiêu | Phụ thuộc | Ngân sách | Trạng thái đề xuất |
|---|---|---|---|---|
| **KT-24** | IPC core 1: `MessageInfo` + Message 64 MR/IPC buffer + **`ipc::transfer`** + `Send/NBSend/Recv/NBRecv` + `no_reply` (2 chiều) + Call non-Endpoint không chặn | **KT-23** (K1b đóng); Q4/Q7 | **3 h** | READY sau KT-23 + Q4/Q7 |
| **KT-25** | IPC core 2: `Call/Reply/ReplyRecv/NBSendRecv/NBSendWait` + **4 cap** (GRANT/bỏ GRANT/unwrap badge/receive window) + vòng đời Reply + donation không rò SC | KT-24, KT-21; Q6/Q8 | **3 h** | chờ KT-24 + Q6/Q8 |
| **KT-26** | Fault + Exit: `FaultKind` VM/unknown/user, fault qua Call+Reply, **resume có thanh ghi** (chống r319), Exit D2-25 + **mô hình procsrv** `{code, how}` | KT-25; Q5/Q10 | **3 h** | chờ KT-25 + Q5 |
| **KT-27** | Notification: word 64 + badge, Signal OR + 1 waiter + xoá word, Wait/Poll, bound TCB + `SchedContext.Bind` | KT-24, KT-21; Q6 | **2,5 h** | chờ KT-24 + Q6 |
| **KT-28** | WaitOnAddress: bảng băm `(frame, offset)`, đọc giá trị + xếp hàng **một lần giữ khoá**, `WakeAddr(n)`, không mất đánh thức | KT-23; Q3/Q4 | **2 h** | chờ KT-23 + Q3/Q4 |
| **KT-29** | SMP 1/2/4/8 + **push migration**: `with_harts`, published `idle`/`cur_prio` + guard luật 3, hộp thư `Migrate`/`Wake`, tất định, không work stealing, driver L1 theo `smp` | KT-22, KT-28; Q1/Q2 | **3 h** | chờ KT-22 + Q1/Q2 |
| **KT-30** | `vifast-sync`: **MCS lock** + **Mailbox MPSC** cố định (7 loại, ack thế hệ, đầy ⇒ backpressure) + stress/Miri (thay loom) | KT-23; Q9 | **3 h** | READY **song song** sau KT-23 + Q9 |

Tổng **19,5 h** `[ƯL]`. KT-24..KT-29 **tuần tự** (cùng chạm `kobj.rs`/`sched.rs`/`scenario.rs`);
**KT-30 chạm duy nhất** `v2/lib/vifast-sync/**` + `v2/Cargo.toml` (member) + danh sách Miri trong
`v2/bin/v2` ⇒ **chạy song song được** với chuỗi sim nếu có agent thứ hai (write-scope rời nhau).

Không task nào chạm `spec/`, `v2/lib/vifast-model`, `v2/test/scn/*.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`.

---

## 5. Chi tiết từng task

> Quy ước chung mỗi task: (i) viết test **đỏ trước** (dán log `JOURNAL/ktNN-red-*.log`);
> (ii) sau mỗi bước nhỏ `./v2/bin/v2 check`; (iii) đóng bằng `./v2/bin/v2 gate --fast` **PASS, không
> retry** (≥ 263 test); (iv) Miri test mới (tập nhỏ, **không** chạy 1.000 seed dưới Miri — bài học KT-19:
> `JOURNAL/kt19-miri-1000seed-aborted.log` chỉ có dòng compile rồi dừng); (v) cập nhật `STATE.md` +
> `JOURNAL/`; (vi) commit `[KT-NN] …` — người thi hành, không phải planner.

### KT-24 — IPC core 1: MessageInfo, Message 64 MR, `ipc::transfer`, Send/Recv/NB*, `no_reply`

**Mục tiêu**: thay IPC "1 word" bằng **message thật** theo `spec/abi.toml:31-49`; mọi đường giao
message đi qua **một** hàm `ipc::transfer` (thiết kế `:324`); `no_reply` đúng hai chiều (QD §3.2 /
`.scn` 17); Call trên object không phải Endpoint **không chặn** (thiết kế `:323`).

**Phụ thuộc**: KT-23 (K1b đóng); Q4/Q7 chốt. Không phụ thuộc KT-25..KT-30.

**Test đỏ trước**
1. `tests/kt24_ipc.rs`: `use vifast_sim::ipc::{MessageInfo, Message};` ⇒ **E0432** (chưa có module `ipc`);
   `m.send(...)`/`m.nb_send(...)`/`m.nb_recv(...)` ⇒ **E0599**.
2. Ngữ nghĩa **no_reply**: server đã `Recv` **không** kèm Reply cap, client `call` ⇒ hiện trả
   `IllegalOperation` (`kobj.rs:2150-2151`), kỳ vọng `Cancelled` + message **vẫn** tới server + cờ bật
   (`.scn` 17 + thiết kế `:322`) ⇒ **đỏ**.
3. Ngữ nghĩa **64 MR**: `send` 64 word rồi `recv` phải trả đủ 64 + label/len; hiện `EndpointState.last_mr`
   chỉ giữ **1** word (`kobj.rs:268`) ⇒ test "mất 63 word" **đỏ đúng lý do**.

**DoD đo được**
1. `ipc::MessageInfo` pack/unpack khớp **chính xác** `spec/abi.toml:46` (`label:0..19, caps:20..22,
   no_reply:23, extra_caps_unwrapped:24..27, length:28..34`); `bits() & !((1u64<<35)-1) == 0`; round-trip
   1.000 giá trị ngẫu nhiên (LCG) không lệch; `caps > 4` ⇒ `InvalidArgument`; `len > 64` ⇒ `InvalidArgument`.
2. `ipc::Message { info, mr: [u64; 64], caps: Vec<CapId>, badge: u64, msg_id: u64 }` (`[TK]`); `mr` giữ
   **byte-for-byte**; **4 MR đầu** đi "thanh ghi", 60 MR sau đi **IPC buffer model** (field riêng trong
   `TcbData`, không gộp) — test đọc riêng hai vùng, và test `mr[4]` chỉ có trong buffer.
3. `ipc::transfer` là **hàm duy nhất** copy MR (+ cap ở KT-25); `send`/`call`/`reply`/`recv` **không** copy
   tại chỗ; bằng chứng = bộ đếm `transfer_calls` **test-only** trong mô hình: sau 1.000 op thì
   `transfer_calls == số message đã giao` (không đếm op lỗi tham số).
4. `send` (chặn) / `nb_send` (**không chặn**: bên nhận đang `Recv` ⇒ giao ngay; không ⇒ vào hàng gửi của
   Endpoint, `spec/abi.toml:108` chỉ ghi "không chờ") / `recv` / `nb_recv` (`Busy` khi rỗng —
   `spec/abi.toml:110`) trên Endpoint; FIFO giữ nguyên thứ tự `msg_id`; **không** mất message khi người
   nhận chưa có; hàng gửi của Endpoint **không giới hạn** trong mô hình `[TK]` (seL4 giới hạn 1 — ghi rõ
   trong `k1c-ipc-semantics.md`, không phải ABI).
5. `no_reply` **hai chiều** (Q7 mặc định): (a) caller đặt `no_reply=1` ⇒ Call trả `Ok` **ngay**, **không**
   sinh Reply, bên nhận thấy cờ; (b) Call gặp bên nhận `Recv` **không** Reply ⇒ message vẫn giao, cờ bật,
   caller nhận **`Cancelled`**; test hai ca + `.scn` 17.
6. `Call` trên **object không phải Endpoint**: **không chặn**, **không** sinh Reply, trả `NotSupported`
   (method chưa cài), `TcbState` **không đổi**; test trên cap TCB/CNode/Untyped.
7. **Bảo toàn message** (C1): `check_ipc()` sau **mỗi** public op — mỗi `msg_id` xuất hiện **đúng một**
   nơi (hàng gửi ⎮ slot nhận ⎮ đã giao); `ipc_sent == ipc_delivered + ipc_pending`; test 1.000 seed × 64 op
   hỗn hợp `send/recv/nb_*/call/exit`, 0 vi phạm, cùng seed ⇒ cùng trace.
8. `scenario.rs` có `Step::{Send, NBSend, NBRecv}` + `Msg` đầy đủ; runner `tests/kt24_scn.rs` chạy
   `.scn` **04, 06, 17** (đọc tệp, không sửa tệp) — 0 lệch.
9. `gate --fast` PASS không retry (≥ 263 test); Miri `-p vifast-sim --test kt24_ipc` PASS; **0 unsafe**;
   `git diff --stat spec/ v2/lib/vifast-model v2/test/scn` rỗng.

**Tệp dự kiến**: `v2/sim/vifast-sim/src/ipc.rs` (**mới**, `[ƯL]` 350–450 dòng), `kobj.rs`
(`EndpointState` → hàng message, `TcbData` thêm vùng IPC buffer + `send/recv/nb_*`), `scenario.rs`
(3 `Step`), `tests/kt24_ipc.rs` + `tests/kt24_scn.rs` (**mới**), `v2/docs/k1c-ipc-semantics.md`
(model-only `[TK]`), `v2/agent/{STATE.md,JOURNAL/}`.

**Rủi ro**: (a) `Kind` phải ≤ 48 B (assert `kobj.rs:382-384`) — message 64 MR phải nằm trong `Box` của
`TcbData`/`EndpointState`, **không** nhét mảng vào enum; (b) đổi `EndpointState.last_mr` làm vỡ
`kt09_e2e`/`kt18_tcb` (dùng `endpoint_last_mr`/`endpoint_send_len`) ⇒ **giữ** API cũ trả MR0, sửa cơ học;
(c) `no_reply` nhánh (b) đụng `call` hiện tại ⇒ chạy lại toàn bộ `kt09_*`/`kt18_*` trước khi bật mặc định;
(d) golden trace của `Harness` **không** bị ảnh hưởng (golden là trace KT-08), nhưng nếu vô tình sửa
`lib.rs`/`scenario.rs` thì chạy `tests/golden.rs`.

**BLOCKED khi**: Q7 chưa chốt nghĩa `no_reply` hai chiều ⇒ **không** tự đặt ngữ nghĩa (ABI-adjacent);
hoặc phát hiện `spec/abi.toml` **không** đủ bit cho cờ "transfer thất bại" buộc phải đổi MessageInfo
⇒ dừng, nêu 2 lựa chọn (đây là **thứ đóng băng** D2-15 mục 2); hoặc vượt 3 h.

---

### KT-25 — IPC core 2: Call/Reply/ReplyRecv/NB*, **4 cap**, unwrap badge, vòng đời Reply

**Mục tiêu**: đường nóng server (`ReplyRecv`) + chuyển **4 cap** đúng `thiết kế:318-320`; Reply object
do bên nhận cấp, bị xoá/revoke thì không treo caller; donation (KT-21) không rò `SchedContext`.

**Phụ thuộc**: KT-24 (transfer MR) + KT-21 (donation, `SchedContext` đi theo Reply); Q6/Q8 chốt.

**Test đỏ trước**
1. `m.reply_recv(...)`, `m.nb_send_recv(...)`, `m.nb_send_wait(...)` ⇒ **E0599**; `m.call(..., msg_with_caps)`
   ⇒ E0599 (chưa có `caps` trong `Message`).
2. Ngữ nghĩa **4 cap**: cap không có `RIGHT_GRANT` (`kobj.rs:61`) phải **không** chuyển được nhưng
   **message vẫn giao** (thiết kế `:319`) — hiện không có đường transfer ⇒ **không biên dịch**; sau khung,
   test đỏ nếu cài cap **dù** thiếu GRANT.
3. Ngữ nghĩa **unwrapped badge**: cap tới **chính endpoint** đang gọi + có badge ⇒ **không** cài vào
   receive window, badge **OR** vào message, `extra_caps_unwrapped` tăng — hiện `Cap.badge` tồn tại
   nhưng không có đường IPC ⇒ **đỏ**.
4. `.scn` 15 (`caps=c0,c0,c0,c0` ⇒ `ok`; **5 cap** ⇒ `InvalidArgument`) và 16 (`Mint` vượt quyền ⇒
   `InsufficientRights`) hiện **chưa chạy** trên mô hình ⇒ test runner **đỏ** vì thiếu API.

**DoD đo được**
1. `reply_recv(tcb, reply, ep, msg, recv_reply_slot)`: giao reply **trước**, rồi `recv`; nếu không có
   message ⇒ `BlockedOnRecv{ep, reply}` (mang Reply **mới**); không mất reply, không mất message.
2. `nb_send_recv` / `nb_send_wait` (Q15): `NBSendRecv` = `NBSend` **không chặn** + `Recv` (chặn);
   `NBSendWait` = `NBSend` + `recv` trên Notification; ngữ nghĩa khi `NBSend` **không giao được** chốt
   theo Q15 và ghi vào `v2/docs/k1c-ipc-semantics.md`; test **cả hai** nhánh (người nhận sẵn sàng /
   không sẵn sàng), không mất message nào đã giao `Ok`.
3. **4 cap**: `caps=[c0,c1,c2,c3]` tới receive window 4 slot rỗng ⇒ nhận đủ 4, mỗi cap **mất
   `RIGHT_GRANT`**, `badge` giữ nguyên; **5 cap** ⇒ `InvalidArgument` **trước** khi giao (không mất message).
4. **Thất bại mềm** (Q6 mặc định): thiếu GRANT / slot đích **đã có cap** ⇒ message **vẫn giao**,
   `MessageInfo.caps` = **số cap cài được**, bộ đếm `cap_transfer_failed` tăng, **không** trả lỗi cứng;
   lỗi **cứng** chỉ khi cap nguồn chết/không tồn tại (`InvalidCapability`) — kiểm cả hai.
5. **Unwrap badge**: `extra_caps_unwrapped` = số cap unwrap; `Message.badge` = OR các badge (field
   **model-only** `[TK]`, không thêm bit vào MessageInfo); receive window **không** bị chiếm.
6. **Vòng đời Reply** (Q8 mặc định): `Delete(reply)`/`Revoke` khi caller đang `BlockedOnReply` ⇒ caller
   được đánh thức `Cancelled` (không deadlock), `Reply.caller == None`, server `Send(rep0)` ⇒
   `InvalidCapability` (đúng `.scn` 07: `ok` rồi `InvalidCapability`); `.scn` 08 ⇒ `Delete` sau `Revoke`
   trả `SlotEmpty`.
7. **Donation không rò** (KT-21): khi Reply bị xoá/revoke/`Suspend(Abort)` caller, `SchedContext` được
   trả về **đúng chủ** đúng một lần (`check_sc_refs` = 0 rò); test 3 nhánh.
8. `scenario.rs` có `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}`; runner `.scn` **05, 07, 08,
   15, 16** — 0 lệch.
9. **Bảo toàn message** giữ sau mọi nhánh lỗi (`check_ipc` sau mỗi op); 1.000 seed × 64 op hỗn hợp
   (call/reply/reply_recv/cap lỗi) — 0 vi phạm, cùng seed cùng trace.
10. `gate --fast` PASS; Miri `--test kt25_reply` PASS; không đụng `spec/`.

**Tệp dự kiến**: `ipc.rs` (transfer cap + report), `kobj.rs` (`reply_recv`, NB variants, Reply lifecycle,
`recv` gắn receive window), `scenario.rs`, `tests/kt25_reply.rs` + `tests/kt25_scn.rs` (**mới**),
`v2/docs/k1c-ipc-semantics.md`.

**Rủi ro**: (a) thứ tự "giao reply rồi recv" trong `ReplyRecv` dễ mất reply nếu recv chặn — làm
**một** hàm, test ca `Recv` rỗng ngay sau đó; (b) receive window là `[Option<SlotRef>;4]` trong
`TcbData` (bind qua `tcb_configure`) ⇒ phải phân biệt "chưa bind" (⇒ lỗi `InvalidArgument`) với "bind
rỗng"; (c) unwrap badge dễ OR nhầm giữa nhiều message ⇒ reset `badge` mỗi message; (d) donation hook
của KT-21 có thể bị gọi hai lần khi Reply finalise ⇒ test đếm `donated_to` chuyển `None` đúng 1 lần.

**BLOCKED khi**: Q6 (ngữ nghĩa thất bại chuyển cap) hoặc Q8 (Reply bị xoá khi caller chờ) chưa chốt;
hoặc phát hiện `MessageInfo` **không** đủ chỗ biểu diễn "số cap cài được" mà không đổi layout ⇒ dừng,
nêu 2 lựa chọn; hoặc vượt 3 h.

---

### KT-26 — Fault + Exit + mô hình procsrv

**Mục tiêu**: fault **khác Exit** đi đúng đường `Call` tới `fault_ep` **kèm Reply**, trả lời Reply để
**tiếp tục** với thanh ghi mới (thiết kế `:321`); chứng minh **không** tái diễn đường **mất message
r319** (`:279`); `Exit` đủ D2-25 và có **mô hình procsrv** cho `wait`/`kill`.

**Phụ thuộc**: KT-25 (đường Reply + chuyển message); Q5/Q10 chốt.

**Test đỏ trước**
1. `m.inject_fault(tcb, FaultKind::UnknownSyscall{ nr })` ⇒ **E0599**; `FaultKind::Vm{..}`/`UserException`
   ⇒ **E0599/E0433** (enum hiện chỉ `Exit`, `tcb.rs:21-23`).
2. Ngữ nghĩa: fault **phải** tới `fault_ep` bằng **Call + Reply** và caller chờ `BlockedOnReply`;
   hiện `exit` chỉ `Send` không Reply (`kobj.rs:2310-2325`) ⇒ test fault-VM **đỏ** (không có đường).
3. Đường **r319**: sau `Send(reply)` resume, MR/thanh ghi **mới** phải nằm trong TCB **trước** khi
   `Ready`; test đọc state ngay sau `reply` ⇒ hiện không có → **đỏ**.
4. `.scn` 13/14/20 hiện **chưa chạy** trên mô hình; runner phải **đỏ** vì thiếu `procsrv` model.

**DoD đo được**
1. `FaultKind` có **4 biến thể**: `Exit{code}`, `Vm{fault_addr, access}`, `UnknownSyscall{nr}`,
   `UserException{ip, kind}` (`[TK]`; xem Q5 về `spec/abi.toml:268-271` chỉ có `Exit`).
2. Fault **không phải Exit**: `inject_fault` ⇒ TCB `BlockedOnSend{ep: fault_ep, call: true, fault: Some(k)}`
   + Reply gắn caller; procsrv `Reply` ⇒ TCB **Ready** với **thanh ghi/MR mới** từ message (ghi **trước**
   khi chuyển `Ready`); test đọc `tcb_regs(tcb)` sau reply = đúng giá trị gửi.
3. **Không mất message** trên đường fault: `check_ipc` sau mỗi bước; **đột biến tay** "resume không ghi
   MR" ⇒ đúng test (3) **đỏ** (log dán `JOURNAL`).
4. `Exit` đủ D2-25 (đã có từ KT-18, K1c **kiểm lại** bằng test): có `fault_ep` ⇒ `BlockedOnSend{call:false,
   fault=Exit{code}}`; procsrv `Recv` xong ⇒ `Inactive`; **không** `fault_ep` ⇒ `Inactive` ngay;
   `Suspend(Abort)` khi đang chờ Exit **huỷ message** nhưng `ReadState.exit_code` **vẫn đọc được** — 4 test.
5. **Mô hình procsrv** `[TK]` (`src/procsrv_model.rs`): `recv_exit()` gom `(tcb, code)`; `wait(tcb)` trả
   `{code, how}` với `how ∈ {Exited, Cancelled, Faulted, Killed}` đúng nhánh: `Exited` khi `exit` không
   fault, `Cancelled` khi bị `Suspend(Abort)` trước khi giao, `Faulted` khi fault ≠ Exit, `Killed` khi
   `kill` (= `Suspend(Abort)` + `Revoke` cap TCB); **không** có shell/user binary (K3.5 — Q10).
6. `.scn` **13, 14, 20** chạy qua runner (đọc tệp): 13 `halt` không fault_ep; 14 `SetFaultEp` rồi `exit`
   ⇒ procsrv `Recv` `ok`; 20 `expect_states = BlockedOnSend{ep, call=false, fault=Exit{7}}` khớp.
7. 1.000 seed × 64 op trộn `exit`/fault/recv/reply/suspend — 0 vi phạm, cùng seed cùng trace.
8. `gate --fast` PASS; Miri `--test kt26_fault_exit` PASS.

**Tệp dự kiến**: `tcb.rs` (`FaultKind` 4 biến thể + `name()`), `kobj.rs` (đường fault Call+Reply,
`inject_fault`, `tcb_regs`), `v2/sim/vifast-sim/src/procsrv_model.rs` (**mới**, `[ƯL]` 120–180 dòng),
`scenario.rs` (`Step::{Fault, ReplyFault}`), `tests/kt26_fault_exit.rs` + `tests/kt26_scn.rs`,
`v2/docs/k1c-fault.md` (model-only `[TK]`).

**Rủi ro**: (a) `FaultKind` nằm trong `TcbState::BlockedOnSend.fault` ⇒ thêm biến thể làm vỡ
`ALLOWED`/`kt18` pattern match (sửa cơ học, **không** đổi bảng 11×11); (b) `procsrv_model` là **model-only**,
đừng tuyên bố là procsrv thật (K3.5); (c) fault-resume ghi "thanh ghi" phải qua **cùng** `ipc::transfer`
(thiết kế `:324`) nếu không sẽ có đường copy thứ hai; (d) đừng dùng `#[should_panic]` cho ca âm.

**BLOCKED khi**: Q5 chưa chốt (mở rộng `FaultKind` **model-only** hay sửa `spec/abi.toml:268-271` bằng
một D2) ⇒ **không** tự thêm vào spec; hoặc cần thêm trạng thái TCB cho fault (trái D2-33) ⇒ dừng;
hoặc vượt 3 h.

---

### KT-27 — Notification: word 64 + badge, Signal OR + **một** waiter + xoá word, Wait/Poll, bound

**Mục tiêu**: Notification đúng **nguyên ngữ nghĩa seL4** (QD §3.11, thiết kế `:328`); bound notification
với TCB **và** `SchedContext`; **không** "đánh thức tất cả".

**Phụ thuộc**: KT-24 (`recv`/`nb_recv`) + KT-21 (`SchedContext.Bind`); Q6 chốt.

**Test đỏ trước**
1. `m.notification_signal(ntfn, badge)`, `m.notification_word(ntfn)`, `m.notification_poll(ntfn)` ⇒ **E0599**.
2. Ngữ nghĩa: `Signal` khi có waiter ⇒ waiter `Ready`, word giao, word **xoá**; hiện
   `NotificationState` chỉ có `waiter` (`kobj.rs:334-336`) và `tcb_wait_notification` chỉ đặt waiter
   (`:2038`) ⇒ **đỏ**.
3. OR: `signal(0b01)` rồi `signal(0b10)` trước khi wait ⇒ word `0b11`; hiện không có word ⇒ **đỏ**.
4. `.scn` 21 (`nb_recv ntf0 ⇒ Busy`, `send ⇒ ok`, `recv ⇒ ok`, `nb_recv ⇒ Busy`) chưa chạy ⇒ runner **đỏ**.

**DoD đo được**
1. `NotificationState { word: u64, waiter: Option<ObjId>, bound_sc: Option<ObjId> }` (`[TK]`).
2. `signal(ntfn, badge)`: `word |= badge`; nếu có waiter ⇒ **một** waiter được đánh thức, nhận `word` rồi
   `word = 0`; nếu không có waiter ⇒ word giữ nguyên cho lần Wait sau (không mất tín hiệu).
3. **Một waiter**: waiter thứ hai trên cùng notification ⇒ `Busy` (giữ như KT-18) — test 3 TCB, chỉ 1 chờ được.
4. `recv`/`nb_recv` trên cap Notification = **Wait**/**Poll**: word ≠ 0 ⇒ trả ngay (`Ok` + word + badge);
   word = 0 ⇒ `BlockedOnNotification` / `Busy`; test `.scn` 21 đúng 4 bước.
5. **Bound notification**: `tcb_bind_notification` (đã có) + `SchedContext.Bind(sc, ntfn)` set
   `bound_sc`/`SchedData.bound`; `signal` đánh thức TCB đang `BlockedOnRecv`/`BlockedOnNotification`
   **và** trả badge; test 2 nhánh (đang chờ notification / đang chờ Recv).
6. **Không mất đánh thức** (C2): 1.000 seed × 64 bước trộn `signal/wait/poll` — với **mọi** thứ tự,
   nếu tín hiệu phát sau khi waiter xếp hàng thì waiter được đánh thức; nếu trước thì word còn; 0 vi phạm.
7. Bất biến: `word` chỉ bị xoá **khi** giao cho waiter; `notification_waiter(ntfn)` khớp `TcbState`;
   notification chưa bind SC vẫn chạy được.
8. `gate --fast` PASS; Miri `--test kt27_notification` PASS; `.scn` 21 xanh.

**Tệp dự kiến**: `kobj.rs` (NotificationState, `signal`/`poll`/`recv` trên Notification,
`SchedData.bound_sc`), `scenario.rs` (`Step::{Signal, WaitNotification, PollNotification}`),
`tests/kt27_notification.rs` + `tests/kt27_scn.rs`, `v2/docs/k1c-notification.md` (model-only `[TK]`).

**Rủi ro**: (a) `recv` trên Notification vs Endpoint — phân nhánh theo `ObjType` của cap, đừng nhầm
`BlockedOnRecv`/`BlockedOnNotification`; (b) badge 0 hợp lệ (không dùng 0 làm "chưa có"); (c)
`SchedContext.Bind` chữ ký chỉ nhận Reply/Notification (`spec/abi.toml:218-220`) — **không** mở rộng
sang TCB (Q6); (d) đừng để `signal` vô tình đánh thức **nhiều** waiter (khác v1 — đây là điểm kiểm có chủ ý).

**BLOCKED khi**: Q6 (bind Notification: model-only hay cần chữ ký `Tcb.BindNotification` trong
`v2/docs/abi-call-signatures.toml`) chưa chốt ⇒ **không** tự thêm chữ ký; hoặc phát hiện cần thêm
trạng thái TCB ⇒ dừng; hoặc vượt 3 h.

---

### KT-28 — WaitOnAddress: bảng băm, đọc giá trị + xếp hàng một lần, `WakeAddr(n)`

**Mục tiêu**: `WaitAddr`/`WakeAddr` đúng thiết kế `:330-334` + QD §3.8: hàng chờ là **bảng băm**
khoá `(frame_id, offset)`, nút trong TCB, **đọc giá trị và xếp hàng trong cùng một lần giữ khoá**;
`WakeAddr` cùng khoá ⇒ **không mất đánh thức**.

**Phụ thuộc**: KT-23 (scheduler `Ready`/`BlockedOnAddress` đã có); Q3/Q4 chốt. Song song được với KT-24..KT-27 nếu muốn.

**Test đỏ trước**
1. `m.wait_addr(tcb, frame, offset, expected)` / `m.wake_addr(frame, offset, n)` / `m.set_addr_word(frame, offset, v)`
   ⇒ **E0599** (hiện chỉ `tcb_wait_address(tcb, frame, offset)` **không** có `expected`, `kobj.rs:2066`).
2. Ngữ nghĩa: giá trị hiện tại **≠** `expected` ⇒ `InvalidArgument` và **không** xếp hàng; hiện xếp hàng
   vô điều kiện (`:2077`) ⇒ test **đỏ**.
3. Không mất đánh thức: A `wait(0)`, đổi word = 1, `wake` ⇒ A phải `Ready`; hiện **không có** `wake` ⇒ **đỏ**.

**DoD đo được**
1. Bảng băm `[Vec<(ObjId, u32, ObjId)>; WAITADDR_BUCKETS]` với `WAITADDR_BUCKETS = 64` `[TK]` (Q3: lấy
   hằng số model vì **`spec/kernel.toml` chưa tồn tại** — `ls spec/` chỉ có `abi.toml`); hash =
   `frame.wrapping_mul(0x9E37_79B1) ^ offset` rồi `% BUCKETS` `[TK]`; tìm waiter theo **(frame, offset)
   đầy đủ** trong bucket ⇒ va chạm bucket không ảnh hưởng; test 1.000 cặp (cùng khoá ⇒ cùng bucket) +
   một ca **cố ý va chạm** (2 khoá cùng bucket) vẫn tìm/đánh thức đúng.
2. `wait_addr(tcb, frame, offset, expected)`: **một** hàm làm cả hai việc — so word với `expected`
   (`InvalidArgument` nếu lệch, **không** xếp hàng) **và** xếp hàng `BlockedOnAddress{frame, offset}`;
   test "lệch giá trị ⇒ không để lại waiter" (`address_waiters_len` không đổi).
3. `wake_addr(frame, offset, n)`: đánh thức **tối đa n** waiter **cùng khoá** (FIFO), trả số đã đánh thức;
   `n = u32::MAX` = tất cả; waiter còn lại vẫn `BlockedOnAddress`; test 3 waiter + n=2.
4. **Không mất đánh thức** (C2): property theo mọi thứ tự bước — nếu `wake` sau khi waiter xếp hàng
   ⇒ waiter `Ready`; nếu `wake` trước (không waiter) ⇒ trả 0 và **không** nuốt tín hiệu (lần Wait sau vẫn
   thấy giá trị mới). 1.000 seed × 64 bước, 0 vi phạm.
5. **Đột biến**: "xếp hàng **trước** rồi mới so giá trị" ⇒ test (2) **đỏ** (stale waiter); log `JOURNAL`.
6. `Suspend(Abort)`/`finalise` gỡ đúng **bucket** (không chỉ vector tuyến tính) — test đếm bucket rỗng sau.
7. `check_sched` + `check_ipc` không bị ảnh hưởng; 1.000 seed giữ xanh.
8. `gate --fast` PASS; Miri `--test kt28_waitaddr` PASS.

**Tệp dự kiến**: `kobj.rs` (`address_waiters` → bảng băm; `wait_addr`/`wake_addr`/`set_addr_word`;
`unlink_tcb_queues` theo bucket), `tests/kt28_waitaddr.rs`, `v2/docs/k1c-waitaddr.md` (model-only `[TK]`).

**Rủi ro**: (a) "khoá" trong mô hình một luồng là **hàm nguyên tử** — đừng giả vờ có lock; bằng chứng
"không mất đánh thức" là **property theo thứ tự bước**, không phải thread thật (L3 "Mutex std stress"
thuộc K2/K3); (b) `Frame` là object reserved (K2) ⇒ khoá theo `ObjId` **không** theo địa chỉ vật lý —
ghi rõ "Frame chia sẻ hai tiến trình" **chưa đạt**; (c) đổi `Vec` → bảng băm làm vỡ test KT-18 dùng
`address_waiter(frame, offset)` (giữ API cũ); (d) `offset: u32` (D2-33) — không đổi sang u64.

**BLOCKED khi**: Q3 chưa chốt nguồn `WAITADDR_BUCKETS` (model const vs mở `spec/kernel.toml`) hoặc Q4
chưa chốt `timeout` ⇒ dừng, **không** tự tạo `spec/kernel.toml` (spec chỉ Agent 0/task được ghi);
hoặc vượt 3 h.

---

### KT-29 — SMP 1/2/4/8 + push migration

**Mục tiêu**: mở mô hình **N hart tất định** (không thread) tới 8 hart; mỗi hart **công bố**
`idle`/`cur_prio`; **push migration** đúng `:341` (tất định, hart rảnh **chỉ số nhỏ nhất**, **không**
work stealing); wake chéo hart qua hộp thư `Wake`; guard **luật 3** (`:415,419`).

**Phụ thuộc**: KT-22 (hộp thư mô hình — **phải DONE trước**) + chuỗi IPC (KT-24..KT-28, vì `kobj.rs`); Q1/Q2 chốt.

**Test đỏ trước**
1. `Machine::with_harts(2)`, `m.publish(hart, ...)`, `m.migrate_count()`, `m.mailbox_len(hart)`,
   `m.set_current_hart(hart)` ⇒ **E0599** (`HART_COUNT = 1` cứng, `kobj.rs:73`).
2. Ngữ nghĩa: 2 hart, X có **2** TCB `Ready` cùng mức cao nhất, Y **idle** trong affinity ⇒ **đúng 1**
   `Migrate`; hiện chỉ 1 hart ⇒ **đỏ**.
3. Guard luật 3: ghi `idle`/`cur_prio` của hart khác phải **bị từ chối**; hiện không có API ⇒ **đỏ**.

**DoD đo được**
1. `Machine::with_harts(n)` cho `n ∈ {1,2,4,8}` (`n = 0`/`> 8` ⇒ `assert!` như `with_arena_bytes`,
   `kobj.rs:527-529`); `MAX_HARTS = 8` `[TK]` mặc định, trần 64 chỉ là hằng số model (Q1/Q3);
   `hart_count()` trả `n`; mọi API cũ (`ready_len`, `current`, `sched_idle`…) giữ nguyên nghĩa với `n = 1`.
2. **Published fields**: `Published { idle: bool, cur_prio: u8 }` mỗi hart; API `publish(hart, idle, prio)`
   **chỉ** gọi được khi `current_hart() == hart`, ngược lại **`Err(IllegalOperation)`** + `debug_assert!`
   (thiết kế nói "panic"; mô hình trả lỗi để test được mà **không** dùng `#[should_panic]` — ghi rõ);
   `published(hart)` **đọc** được từ mọi hart (luật 3 cho phép).
3. **Push**: sau mỗi op (khi `preempt_every_step` bật, và/hoặc `dispatch`), với mỗi hart X: nếu
   `ready_len_at(X, top) > 1` và tồn tại Y ≠ X trong **affinity** của TCB, `Y` có `published.idle = true`
   ⇒ chọn **Y nhỏ nhất** (tất định), đẩy **một** TCB (chốt Q2: **cuối** hàng mức cao nhất) qua
   `Mailbox::Migrate`; cuối bước Y xử lý: `home = Y`, enqueue vào hàng Y, `migrate_count += 1`;
   **không** nhân bản, **không** mất TCB (đếm trước/sau bằng nhau).
4. **Ca âm**: Y **không** idle ⇒ 0 migrate; Y **ngoài** affinity ⇒ 0; X chỉ có **1** Ready mức cao nhất
   ⇒ 0; **không** work stealing (Y **không** tự kéo; test: Y rảnh, X có 1 TCB ⇒ 0 migrate).
5. **Wake chéo hart**: `wake(tcb home=Y)` từ hart X ⇒ `Wake` vào mailbox Y; Y chỉ preempt nếu
   ưu tiên **>** `published(Y).cur_prio` (thiết kế `:342`) — test 2 nhánh (cao hơn/không cao hơn).
6. **Bất biến toàn cục** `check_smp()` sau mỗi op: (i) không TCB nào `Running` trên **2** hart;
   (ii) **bảo toàn TCB**: số TCB ở trạng thái `Ready`/`Running` = số TCB trong 256 hàng + `current` +
   số đang nằm trong mailbox `Migrate` (không thừa/thiếu); (iii) `home` khớp hart đang giữ
   `current`/hàng; (iv) `published.idle` ⇔ `current == None`; (v) không TCB trong hàng của hart
   **ngoài** affinity của nó.
7. **100.000 seed** không chạy ở đây (cổng cuối phase §7.1), nhưng `--fast` có **1.000 seed × 64 op ×
   `n ∈ {1,2,4,8}`** (test riêng `tests/kt29_smp.rs`), cùng seed ⇒ cùng trace, 0 vi phạm.
8. **Driver L1 theo `smp`**: `tests/l1_k1c.rs` đọc `VIFAST_SMP` (mặc định `1`) và `VIFAST_SEEDS`
   (mặc định `1000`); `v2/bin/v2` thêm `nightly --l1 [--seeds N] [--smp a,b,c]` (mẫu `--only-miri`,
   D2-47(a)) — `gate --fast` **vẫn** 1.000 seed/smp 1 và ≤ 60 s (đo lại, ghi log).
9. `gate --fast` PASS; Miri `--test kt29_smp` PASS (seed nhỏ, **không** 1.000 seed dưới Miri).

**Tệp dự kiến**: `sched.rs` (`Published`, push, `check_smp`), `kobj.rs` (`with_harts`,
`current_hart`/`publish`/mailbox integration), `mailbox.rs` (từ KT-22 — **không** sửa ngữ nghĩa, chỉ thêm
`Migrate`), `scenario.rs` (`Step::{SetHome, MigrateTick}`), `tests/kt29_smp.rs`, `tests/l1_k1c.rs`,
`v2/bin/v2`, `v2/docs/k1c-smp.md` (model-only `[TK]`).

**Rủi ro**: (a) push làm vỡ bất biến "Ready ⇔ trong hàng" **giữa** bước ⇒ xử lý hộp thư **cuối bước**
(như KT-22), không có trạng thái in-flight; (b) preempt **mọi op** × nhiều hart làm `--fast` chậm ⇒ đo,
giảm **số op/seed của test mới** (ghi lý do), **không** giảm seed L1; (c) `sched_sync` hiện gỡ/đặt theo
`home` — với `n > 1` phải kiểm affinity trước khi enqueue; (d) đừng biến `with_harts` thành đường mặc định
(`Machine::new()` vẫn 1 hart để giữ nguyên 263 test).

**BLOCKED khi**: Q1 (SMP mô hình tất định) hoặc Q2 (chọn TCB đẩy/tần suất) chưa chốt; **hộp thư KT-22
chưa DONE** ⇒ chỉ làm `with_harts` + published, **không** làm push; hoặc vượt 3 h.

---

### KT-30 — `vifast-sync`: MCS lock + Mailbox MPSC (thay loom bằng stress + Miri)

**Mục tiêu**: dựng crate **`vifast-sync`** (đã nằm trong danh sách crate đóng có `unsafe`,
`thiết kế:684,695`): **MCS queue lock** (S1, `:407`) và **hộp thư MPSC cố định** (S2, `:423-428`) —
ack bằng **bộ đếm thế hệ**, đầy ⇒ **backpressure**; kiểm bằng **stress host + Miri** vì **loom không có**
trong registry và không được cài (Q9).

**Phụ thuộc**: KT-23; Q9 chốt. **Song song được** với chuỗi sim (crate riêng).

**Test đỏ trước**
1. `use vifast_sync::{McsLock, Mailbox, Msg};` ⇒ **E0432** (crate chưa tồn tại, `v2/lib/` không có).
2. Ngữ nghĩa: mailbox đầy phải trả `Busy` và **không** ghi đè/mất (thiết kế `:427`); hiện không có crate
   ⇒ test **đỏ**.

**DoD đo được**
1. Crate `v2/lib/vifast-sync` (`#![no_std]`; `unsafe` **được phép** vì crate nằm trong danh sách đóng
   `thiết kế:684,695` — nhưng gói trong module nhỏ và **Miri sạch**; SPDX
   `// SPDX-License-Identifier: MIT OR Apache-2.0`; ≤ 1.500 dòng `[TK]`; **không** `cfg(target_arch)`);
   thêm vào `v2/Cargo.toml` `members`; **không** dependency ngoài (chỉ `core::sync::atomic`, D2-21).
2. `McsLock<T>`: `lock() -> Guard`, `try_lock()`, `Drop` mở khoá; **FIFO** theo thứ tự xếp hàng (test
   single-thread qua API nội bộ `queue_len()`/`next_owner()`); `try_lock` khi đã giữ ⇒ `None`;
   **không** reentrant (test ghi rõ).
3. `Mailbox<const N: usize>`: `Msg` đủ **7 loại** `Wake/TlbFlush/Reschedule/Migrate/Stop/VmKick/PowerReq`
   (+ payload hart/tcb `[TK]`); `send() -> Result<(), Busy>`; `drain(|msg|)` đúng **FIFO**; `ack(gen)`
   theo **bộ đếm thế hệ** (`generation: AtomicU32`), `pending(gen)` trả số chưa ack.
4. **Đầy ⇒ backpressure**: gửi `N+1` message ⇒ message thứ `N+1` trả `Busy`, **không** mất message đầu;
   sau `drain` gửi lại được; test 1.000 vòng gửi/đầy/rút.
5. **Stress host** (test riêng, **có biên** `[ƯL]` ≤ 3 s): 4 thread × 500 op `lock/unlock` + `send`;
   assert **mutual exclusion** (bộ đếm tăng đúng) + **không mất message** (tổng `drain` = tổng `send`
   `Ok`); **không** assert trace/thứ tự (thread không tất định) — chỉ assert **bất biến**; nếu flake ⇒
   `T-INFRA` + log, **không** retry (D2-07).
6. **Miri**: test single-thread (lock/unlock, mailbox FIFO/đầy) PASS dưới `v2 miri -p vifast-sync`; stress
   **không** chạy dưới Miri.
7. **Không** thay loom bằng "explorer tự viết" trong task này (Q9 mặc định); nếu Agent 0 chọn (b) thì
   **task riêng** — ghi rõ trong DoD.
8. `v2 nightly --only-miri` thêm `-p vifast-sync` (1 dòng trong `v2/bin/v2`); `gate --fast` PASS
   (test mới ≤ 3 s, tổng cổng vẫn ≤ 60 s — đo và ghi log).

**Tệp dự kiến**: `v2/lib/vifast-sync/{Cargo.toml,src/lib.rs,src/mcs.rs,src/mailbox.rs,tests/*.rs}`
(**mới**), `v2/Cargo.toml` (member), `v2/bin/v2` (danh sách Miri), `v2/docs/k1c-sync.md`.

**Rủi ro**: (a) `unsafe` là **được phép** trong crate này nhưng vẫn phải Miri sạch — `AtomicPtr` cho node
MCS dễ UB nếu node không `Pin`; (b) stress thread có thể flake trên máy bận ⇒ biên nhỏ, chỉ bất biến;
(c) đừng tuyên bố "lock-free" cho toàn bộ — chỉ **hộp thư** là lock-free (thiết kế `:428`), MCS lock là
**khoá**; (d) `no_std` + test cần `std` ⇒ tách feature/test như `vifast-traceformat` (D2-42).

**BLOCKED khi**: Q9 chưa chốt (stress+Miri hay tự viết explorer hay thêm loom); hoặc phát hiện cần
dependency ngoài (D2-21) ⇒ dừng; hoặc stress **không** giữ được ≤ 3 s/ổn định ⇒ ghi `T-INFRA` + đề xuất
tách, **không** nới ngưỡng; hoặc vượt 3 h.

---

## 6. Quyết định Agent 0 phải chốt **trước khi mở task**

| # | Câu hỏi | Đề xuất mặc định | Chặn gì |
|---|---|---|---|
| **Q1** | **SMP thật hay mô hình?** K1c cần `smp 1/2/4/8` nhưng sim là một luồng tất định. Chọn: (a) **mô hình N hart tất định** trong sim (`with_harts`, không thread, không QEMU) tới 8 hart; (b) thread thật trong SimPlatform (mất tất định, Miri khó); (c) hoãn SMP sang K3 (trái tiêu chí `:875`) | **(a)** — đúng D2-49(h) "tick ảo", đúng SimPlatform §9.5 "N hart"; (b)/(c) bác | **KT-29**, C4 |
| **Q2** | **Chi tiết push**: (a) đẩy TCB **cuối** hàng mức cao nhất (giữ FIFO cho TCB cũ) hay **đầu** hàng; (b) tối đa **1** TCB/hart/bước; (c) chỉ đẩy khi `published.idle`; (d) X đang `Running` mức cao hơn TCB được đẩy thì có đẩy không? | **(a) cuối hàng + (b) 1/bước + (c) đúng + (d) chỉ đẩy TCB cùng mức cao nhất của hàng Ready** — tất định, dễ kiểm | **KT-29** DoD 3/4 |
| **Q3** | **`spec/kernel.toml` chưa tồn tại** (`ls spec/` = chỉ `abi.toml`; thiết kế `:680` liệt kê `kernel.toml`) ⇒ `WAITADDR_BUCKETS` (`:331`) và `MAX_HARTS` (`:421`) lấy ở đâu? (a) hằng số model `[TK]` + ghi trong `v2/docs/k1c-waitaddr.md`/`k1c-smp.md`, **không** tạo spec; (b) Agent 0 tạo `spec/kernel.toml` ngay (spec chỉ Agent 0/task được ghi) | **(a)** — K1c tự chứa; `kernel.toml` để mốc K3 (Agent 0 mở task spec riêng nếu muốn) | **KT-28, KT-29** |
| **Q4** | **`timeout` của `WaitAddr`**: `spec/abi.toml:115` ghi `args = "mrs: addr, expected"` (**không** timeout), còn thiết kế `:332` ghi `WaitAddr(frame_cap, offset, expected, timeout)`. K1c: (a) **không** timeout (nguồn sự thật `spec/` thắng; `Sleeping{Timeout}`/`Timeout` để K6 theo D2-16); (b) mô hình timeout ảo ngay | **(a)** — spec/ thắng (AGENTS §1), timeout fault hoãn sau K6 (D2-16) | **KT-28** DoD 2 |
| **Q5** | **`FaultKind` mở rộng**: `spec/abi.toml:268-271` chỉ có `Exit`, nhưng thiết kế `:321` cần **VM, unknown syscall, user exception**. (a) thêm **model-only** 3 biến thể + ghi rõ spec là **tập con** tới K3; (b) Agent 0 thêm vào `spec/abi.toml` bằng D2 mới; (c) K1c **chỉ** làm `Exit` (đọc chữ "fault" ở `:875` là fault-họ-Exit) và để 3 loại kia cho K3 | **(a)** — không đụng spec, K3 conformance chốt sau; nếu Agent 0 muốn normative thì **(b)** | **KT-26** |
| **Q6** | **Notification bind + thất bại chuyển cap**: `Tcb.BindNotification` (method 13) **không có chữ ký** trong `spec/`; `SchedContext.Bind` chỉ nhận Reply/Notification (`:218-220`, "TCB bind không có nguồn — bỏ"). Và "chuyển cap thất bại đặt cờ trong MessageInfo" (`:319`) nhưng **35 bit đã kín** — cờ là gì? | **(a)** `tcb_bind_notification` + `SchedContext.Bind(sc,ntfn)` là **model API** (không thêm chữ ký); bind Notification **không** qua TCB trong ABI ở K1c. **(b)** thất bại **mềm**: message vẫn giao, `MessageInfo.caps` = số cap cài được, bộ đếm `cap_transfer_failed` (không thêm bit, MessageInfo giữ nguyên — D2-15 mục 2) | **KT-25, KT-27** |
| **Q7** | **`no_reply` hai chiều**: `.scn` 17 đặt `no_reply=1` từ **bên gọi** và kỳ vọng `ok`; thiết kế `:322` chỉ nói kernel bật cờ khi bên nhận `Recv` không Reply. Caller đặt `no_reply=1` có nghĩa **Call = Send** (không chờ Reply) không? | **Có**: caller `no_reply=1` ⇒ trả `Ok` ngay, **không** sinh Reply, bên nhận thấy cờ; nhánh kernel bật cờ khi thiếu Reply ⇒ caller `Cancelled` (đúng `:322`) | **KT-24** DoD 5 |
| **Q8** | **Reply bị xoá khi caller đang `BlockedOnReply`**: `.scn` 07/08 **không** assert trạng thái client. (a) caller được đánh thức **`Cancelled`** (không deadlock); (b) giữ blocked (để user tự chết); (c) `Delete` Reply đang dùng trả `Busy` (khác `.scn` 07 `=> ok`) | **(a)** — nhất quán nghĩa `Cancelled` (QD §3.1), `.scn` 07 vẫn đúng (`ok` rồi `InvalidCapability`) | **KT-25** DoD 6 |
| **Q9** | **MCS lock/lock-free kiểm bằng gì** (loom **không có** trong registry, không được cài): (a) `vifast-sync` + **stress host có biên** (bất biến, không trace) + **Miri** phần không atomic; (b) tự viết **explorer đan xen tất định** (kiểu loom mini) — đắt; (c) thêm dependency `loom` (cần mạng + license + D2, **bác**); (d) hoãn MCS lock sang K3, K1c chỉ hộp thư mô hình | **(a)** — thiết kế `:428` cho phép "nếu không có loom thì stress"; (b) tách task riêng nếu Agent 0 muốn; (c) bác | **KT-30** |
| **Q10** | **procsrv**: K1c chỉ làm **mô hình** procsrv trong sim (`recv_exit`/`wait`/`how`), **không** shell/user binary — shell ở **K3.5** (`:878`). Xác nhận? | **Đúng**; `wait` RPC + `kill` = `Suspend(Abort)` + `Revoke` cap TCB (chỉ mô hình) | **KT-26** DoD 5 |

**Câu hỏi nhỏ (không chặn mở task, trả lời bằng chat cũng đủ để ghi chú):**
- **Q11** `Call` trên object không phải Endpoint (§4.6 `:323`): K1c chỉ cần **không chặn/không Reply**
  + `NotSupported`, bảng dispatch `label → method` để **K3 conformance** — đúng không?
- **Q12** "So mô hình" cho IPC (§9.2 `:748`) cần mở rộng `vifast-model` — D2-49(g) **chưa** nói tới IPC.
  Đề xuất: **không** mở ở K1c, bằng chứng = bất biến + coverage + mutation; parity 10k giữ làm **hồi quy**.
- **Q13** 100.000 seed × smp 1/2/4/8 chạy bằng **`v2 nightly --l1`** (mẫu `--only-miri`; `gate --fast`
  vẫn 1.000 seed) — đúng chưa, hay muốn cổng này thành **task `KT-31`** (2 h)?
- **Q14** Dòng ViScript đang sửa `viscript/**` trong cùng cây: K1c **không** chạm; xác nhận không cần
  đồng bộ gì.
- **Q15** `NBSendRecv`/`NBSendWait` (thiết kế `:315` chỉ nêu tên "cho passive server và driver", không
  nêu ngữ nghĩa khi **không gửi được**): đề xuất `[TK]` (a) `NBSendRecv` = `NBSend` (không chặn) rồi
  `Recv` (chặn) — nếu `NBSend` không giao được thì **vẫn** `Recv` và coi message gửi là của bên gọi
  (không tính "mất" của kernel); (b) `NBSend` lỗi ⇒ trả lỗi ngay, không `Recv`. Cần chốt để KT-25 có
  test hai nhánh.

> Trả lời bằng chat **không** mở task; cần D2 mới trong `DECISIONS.md` + dòng `APPROVED` trong QUEUE
> (AGENTS §8). Planner không tự chốt thay.

---

## 7. Thứ tự, cổng, bằng chứng

### 7.1 Cổng cuối phase (chưa phải task — đề nghị `KT-31` nếu Agent 0 muốn tách)

```
Chuỗi sim (tuần tự, cùng chạm kobj.rs/sched.rs/scenario.rs):

  KT-24 → KT-25 → KT-26 → KT-27 → KT-28 → KT-29
   3 h     3 h     3 h     2,5 h   2 h     3 h

Song song (crate riêng, chỉ chạm v2/lib/vifast-sync + v2/Cargo.toml + v2/bin/v2):

  KT-30 (3 h) — bắt đầu được ngay sau KT-23, không chờ chuỗi sim

                              ↓
        CỔNG K1C: 100.000 seed × smp 1/2/4/8 + mutation tay + Miri + bảng DoD  [ƯL] 1,5–2 h
```

Cổng K1C gồm **5 việc**, chạy bằng lệnh (không viết tay số đo — QD §9 (câu 12 của dossier)):

1. `./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8` (lệnh do KT-29 thêm) — 4 log, 4 dòng
   `# L1 OK: N seed, M trace`, cùng seed ⇒ cùng trace, **0** vi phạm `check_ipc/check_sched/check_smp`.
   Thời gian `[ƯL]`: log cổng KT-19 ghi suite `kt19_sched` (10 test, gồm ca **1.000 seed × 64 op**) chạy
   **2,96 s**; 100.000 seed ⇒ `[ƯL]` ~5 phút/cấu hình ⇒ **~20 phút cho 4 cấu hình** — **phải đo lại**,
   không chép số này vào tài liệu (QD §9, câu 12 của dossier).
2. `./v2/bin/v2 gate --fast` PASS **không retry** (≥ 263 test + test mới), thời gian ≤ 60 s.
3. `./v2/bin/v2 nightly --only-miri` (danh sách có `kt24..kt30` + `vifast-sync`) PASS.
4. **Mutation tay** bảng §7.2: mỗi đột biến → **đúng một** test đỏ → revert → log `JOURNAL`.
5. **Bảng DoD K1c** trong `STATE.md`: 4 dòng C1..C4 kèm đường dẫn log; dòng nào **chưa** đạt ghi rõ
   "chưa đạt" — không tô hồng.

### 7.2 Danh sách đột biến **chuẩn bị** (C3) — mỗi đột biến phải bị bắt

| # | Đột biến (sửa tạm, **không** commit) | Test phải đỏ |
|---|---|---|
| M1 | `ipc::transfer` bỏ copy **1** trong 64 MR | `kt24` "64 MR byte-for-byte" |
| M2 | Bỏ kiểm `RIGHT_GRANT` khi chuyển cap | `kt25` "thiếu GRANT ⇒ không cài cap" |
| M3 | Bỏ **bỏ GRANT** trên cap nhận | `kt25` "cap nhận không còn GRANT" |
| M4 | Bỏ **unwrap** badge (cài cap vào receive window thay vì OR badge) | `kt25` "unwrap badge" |
| M5 | `Signal` **không xoá** word sau khi giao | `kt27` "word xoá sau khi giao" |
| M6 | `Signal` đánh thức **tất cả** waiter (kiểu v1) | `kt27` "một waiter" |
| M7 | `WaitAddr` xếp hàng **trước** rồi mới so giá trị | `kt28` "lệch giá trị không để lại waiter" (§9.2 `:750`) |
| M8 | `WakeAddr` bỏ qua bucket, wake **mọi** khoá | `kt28` "wake đúng khoá" |
| M9 | Push migration bỏ `home = Y` (giữ home cũ) | `kt29` "conservation + home" |
| M10 | Fault-resume **không** ghi MR trước `Ready` (đường r319) | `kt26` "resume có thanh ghi" |
| M11 | Mailbox đầy **ghi đè** message cũ | `kt30` "đầy ⇒ Busy, không mất" |
| M12 | `Suspend(Abort)` trên `BlockedOnReply` **không** tách caller (hồi quy KT-18) | `kt18`/`kt25` |

### 7.3 Bằng chứng cần có ở mỗi task (D2-05, Khối C QUEUE)

1. Log đỏ trước (`JOURNAL/ktNN-red-*.log`) **đúng lý do**.
2. Log `gate --fast` PASS (không retry) + số test **không giảm** dưới 263.
3. Log Miri test mới (tập nhỏ).
4. `.scn` liên quan chạy qua runner (không sửa `.scn`).
5. `STATE.md` + `JOURNAL/` cập nhật; commit `[KT-NN]` **người thi hành**, không phải planner.

---

## 8. Bằng chứng lệnh đã chạy (read-only, không sửa gì)

```text
$ git rev-parse --short HEAD
2657ee0
$ git show -s --format='%h %s' HEAD
2657ee0 [ST-00] lật ST-12b DONE (dec75e3, 237 test) — ST-12 nay DONE thật; mở ST-14; bổ sung VD-48 (percent↔float chỉ là giá trị)
$ git status --porcelain --untracked-files=all | grep 'v2/sim'
 M v2/sim/vifast-sim/src/kobj.rs
 M v2/sim/vifast-sim/src/lib.rs
?? v2/sim/vifast-sim/src/sched.rs
?? v2/sim/vifast-sim/tests/kt19_sched.rs
?? v2/sim/vifast-sim/tests/tmp_miri_probe.rs
$ grep -o "test result: ok\. [0-9]* passed" v2/agent/JOURNAL/kt19-gate-fast.log | awk '{s+=$4} END {print s}'
263
$ grep -c "test result: ok" v2/agent/JOURNAL/kt19-gate-fast.log
43
$ tail -3 v2/agent/JOURNAL/kt19-gate-fast.log
# L1 OK: 1000 seed, 999 trace khác nhau
--- L1: OK (657 ms, đo bởi cổng)
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
$ /bin/ls -1 spec/
abi.toml
$ grep -n "WaitAddr\|WakeAddr" spec/abi.toml
115:9 = { name = "WaitAddr", args = "mrs: addr, expected", ret = "error, value", note = "futex wait" }
116:10 = { name = "WakeAddr", args = "mrs: addr, n", ret = "error, value", note = "futex wake" }
$ grep -n "fault_kind" -A 3 spec/abi.toml
268:[method_signature.fault_kind]
269:names = ["Exit"]
271:fields = ["code: u64"]
$ grep -n "    pub fn call\|    pub fn recv\|    pub fn reply\|    pub fn exit" v2/sim/vifast-sim/src/kobj.rs
2138:    pub fn call(&mut self, tcb: ObjId, ep: SlotRef, mr: u64) -> Result<(), KError> {
2193:    pub fn recv(&mut self, tcb: ObjId, ep: SlotRef, reply: Option<SlotRef>) -> Result<(), KError> {
2264:    pub fn reply(&mut self, tcb: ObjId, reply: SlotRef) -> Result<(), KError> {
2287:    pub fn exit(&mut self, tcb: ObjId, code: u64) -> Result<(), KError> {
$ grep -rn "MessageInfo" v2/lib v2/sim --include=*.rs | wc -l
0
$ grep -n "HART_COUNT: u16\|^struct NotificationState\|    address_waiters:" v2/sim/vifast-sim/src/kobj.rs
73:pub const HART_COUNT: u16 = 1;
334:struct NotificationState {
471:    address_waiters: Vec<(ObjId, u32, ObjId)>,
$ grep -n "size_of::<Kind>" v2/sim/vifast-sim/src/kobj.rs
275:/// `EndpointState`, `ReplyData`) nằm sau `Box`/`Vec` để `size_of::<Kind>()` vừa 48 B —
383:    core::mem::size_of::<Kind>() + vifast_objmem::HEADER_SIZE <= 64,
$ for d in ~/.cargo/registry/src/*/; do /bin/ls "$d" | grep -i '^loom'; done
(exit=1, không dòng nào — loom KHÔNG có trong registry; tệp này không cài gì)
$ wc -l v2/sim/vifast-sim/src/{kobj,sched,tcb,scenario,lib}.rs
3056 kobj.rs   267 sched.rs   415 tcb.rs   395 scenario.rs   382 lib.rs
$ wc -l v2/sim/vifast-sim/tests/{kt18_tcb,kt19_sched}.rs
1014 kt18_tcb.rs   824 kt19_sched.rs
```

Ghi chú: `tmp_miri_probe.rs` là tệp tạm còn sót trong cây làm việc (không phải của planner, **không**
chạm); `kobj.rs`/`lib.rs` đang **sửa dở** bởi KT-19 ⇒ mọi số dòng ở trên là ảnh chụp lúc lập kế hoạch và
sẽ đổi sau khi KT-19..KT-23 commit.

Không chạy `cargo test`/`gate` toàn bộ khi lập kế hoạch (tránh chen dòng ViScript đang chạy); baseline
lấy từ log `JOURNAL/kt19-gate-fast.log` (263 test/43 suite) và sẽ tái xác nhận ở task đầu tiên.

---

## 9. Ngoài phạm vi K1c (không đề xuất, không làm)

- **K2**: VSpace/PageTable/Frame/ASID/shootdown/TLB-safety, W^X, khoá WaitAddr theo **địa chỉ vật lý**,
  "Frame chia sẻ hai tiến trình" (§9.2 `:750`).
- **K3/K3.5**: QEMU/ktest/conformance/perf, IRQ thật → Notification, `Mutex std stress` (L3),
  nối `vifast-sync` vào platform, procsrv + shell Rust thật (`:878`).
- **K6**: refill sporadic nhiều đoạn, timeout fault/`timeout_ep` (D2-16), `SetTls`.
- **K4'/K5'**: aarch64/x86_64, SBI RFENCE, PCID/AP→BSP.
- Mở rộng `vifast-model` để parity IPC/fault (Q12) — task riêng > 3 h nếu Agent 0 muốn.
- Bất kỳ thay đổi `spec/`, `.scn`, ABI, QUEUE/DECISIONS/AGENTS trong K1c — trừ khi Agent 0 chọn
  Q3(b)/Q5(b)/Q6(a) và **tự làm trước**; task chỉ tiêu thụ.
- Viết loom mini/explorer đan xen (Q9(b)) — trừ khi Agent 0 chốt; mặc định dùng stress + Miri.
