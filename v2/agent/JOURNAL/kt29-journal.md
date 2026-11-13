# KT-29 — SMP mô hình tất định 1/2/4/8 + push migration (K1c, D2-53 Q1/Q2)

**Vào**: READY (`agentctl next --project v2` trả KT-29) · HEAD `dd4761e` ([KT-28] DONE, Agent 0
lật QUEUE) · `git rev-list --count HEAD..origin/main` = 0 · working tree có thay đổi `viscript/`
của tab khác (**không chạm**, chỉ stage `v2/`).

**Nguồn ngữ nghĩa**: K1C-PLAN §5 KT-29; **D2-53 Q1/Q2** (mô hình N hart tất định, không
thread/QEMU; push cuối hàng/≤1 TCB/hart/bước/chỉ idle/cùng mức Ready cao nhất); D2-49(a) (1
hart mặc định, push thuộc K1c); D2-50 (SC-less không chạy); D2-51 (donation không đổi); D2-58
(không đổi WaitAddr). Thiết kế §4.8 `:336-344`, §4.16 luật 3 `:412-419`, §4.17 `:423-428`.

**Không làm** (đúng brief): không `vifast-sync`/MCS lock/MPSC (KT-30); không sửa ngữ nghĩa
IPC/Notification/WaitAddr; không sửa `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS,
`viscript/`; không dependency/`unsafe`/`#[allow]`/`#[ignore]`; không đổi ABI; không thread
thật/QEMU.

## 1. Test đỏ trước (D2-05)

- [`kt29-red-compile.log`](kt29-red-compile.log) — `tests/kt29_smp.rs` **không biên dịch** trước
  KT-29: **E0432** `Published`/`MAX_HARTS` chưa có + **E0599** toàn bộ API SMP/push/migration
  (`with_harts`, `check_harts`, `publish`, `push_migration`, `migrate_count`, `mailbox_len`, …).
- [`kt29-red-semantics.log`](kt29-red-semantics.log) — probe tạm bằng API **cũ**: `hart_count()
  = 1`; `dispatch(1)`/`post_mail(1, Wake)` ⇒ `InvalidArgument`; `post_mail(0, Migrate)` ⇒
  `NotSupported`; assert mô hình 2 hart **panic** ⇒ chưa biểu diễn được SMP/push.

## 2. Thay đổi (chỉ `v2/`)

| Tệp | Nội dung |
|---|---|
| `v2/sim/vifast-sim/src/sched.rs` | `Published { idle, cur_prio }`; `HartSched::last_at(prio)` (TCB **cuối hàng** — Q2) |
| `v2/sim/vifast-sim/src/mailbox.rs` | `HartMailbox::iter` (check_smp đếm yêu cầu); `Migrate` được **drain FIFO** như mọi message (không rút từ giữa hàng) |
| `v2/sim/vifast-sim/src/kobj.rs` | `MAX_HARTS = 8`; `with_harts(n)`; `published`/`publish` + guard luật 3; `current_hart`/`set_current_hart`; `push_step`/`push_migration`/`apply_migrate`/`migrate_count`; `post_mail` kiểm `Migrate` (≥2 hart, đúng hộp thư đích, TCB `Ready`, `home != to_hart`, affinity); `wake` chéo hart qua `Wake` + preempt theo `published.cur_prio`; `check_smp`/`check_harts`; `sched_after_op` làm tươi `published` + push khi `preempt_every_step`; `dispatch` chạy một vòng push khi `n > 1`; `tcb_set_affinity`/`tcb_set_home`/`transition_tcb` siết bất biến `home ∈ affinity`, `Running{hart} ⇒ home == hart` (chỉ ảnh hưởng `n > 1`) |
| `v2/sim/vifast-sim/tests/kt29_smp.rs` | 11 test: `with_harts` 1/2/4/8 + assert n=0/>8; `published` + guard; push đúng 1/cuối hàng/đích nhỏ nhất/qua `Migrate`; đích bận/ngoài affinity/top chỉ 1/không work stealing; Running không bị đẩy; push tự động khi preempt; `post_mail`+`process_mail` Migrate + 1 hart `NotSupported`; wake chéo hart 2 nhánh preempt; fuzz `1.000 seed × 64 op × smp {1,2,4,8}` cùng seed cùng trace (`cfg(miri)` 3 × 8) |
| `v2/sim/vifast-sim/tests/l1_k1c.rs` | driver L1 theo `VIFAST_SMP`/`VIFAST_SEEDS` (mặc định 1/1000), cùng seed cùng trace, kiểm `check_sched`/`check_harts`/`check_ipc` mỗi op |
| `v2/bin/v2` | `nightly --l1 [--seeds N] [--smp a,b,c]` (mặc định 10.000 seed × 1,2,4,8; 100k là cổng cuối phase §7.1); thêm `kt29_smp` vào `nightly --only-miri` |
| `v2/docs/k1c-smp.md` | tài liệu model-only `[TK]`: hằng số, published/luật 3, thuật toán push, wake chéo hart, bất biến `check_smp`, tất định/driver, phạm vi hoãn |
| `v2/agent/JOURNAL/kt29-*` | log đỏ, baseline/gate/check/miri |

**Quyết định mô hình đáng review** (ghi rõ thay vì ẩn):
1. **`published` là ảnh trung thực** của `HartSched` (làm tươi sau mỗi op). `publish` chỉ ghi được
   hart đang chạy **và** giá trị phải khớp trạng thái thật ⇒ hart không "nói dối" trong mô hình;
   thiết kế nói panic, mô hình trả `Err(IllegalOperation)` để test âm không cần `#[should_panic]`
   (K1C-PLAN §5 KT-29 DoD 2 cho phép ghi rõ cách này).
2. **Không in-flight**: push post `Migrate` rồi drain **trong cùng bước**; TCB vẫn ở hàng nguồn
   cho tới khi hart đích xử lý. Vì vậy `check_smp` đếm hạng tử "yêu cầu Migrate đang chờ" ở
   **cả hai vế** của đẳng thức bảo toàn (tương đương `hàng + current == Ready/Running`), và kiểm
   riêng nội dung từng yêu cầu (hart đích, `home != to_hart`, affinity, TCB sống).
3. **Push chỉ khi `n > 1`**, tự động ở `dispatch` và sau vòng preempt khi `preempt_every_step`
   bật; `push_migration` công khai cho test tất định. `n = 1` giữ nguyên toàn bộ ngữ nghĩa cũ
   (427 test baseline xanh).
4. **Siết bất biến `n > 1`**: `tcb_set_affinity` đòi `home` trong mask; `tcb_set_home` đòi home
   trong affinity và từ chối TCB đang `Running`; `transition_tcb → Running{hart}` đòi
   `home == hart`. `n = 1` không đổi hành vi (mask luôn 1, home 0).

## 2b. Verdict `reviewer-kt29`: **DRIFT** (không UNSAFE) — đã sửa 5/5 trước commit

1. **Drain `Migrate` phá FIFO KT-22** — `take_migrates` rút `Migrate` từ **giữa** hàng và `record`
   trước `Wake`/`Reschedule` đến sớm hơn. **Sửa**: xoá `take_migrates`; thêm
   `drain_mail_fifo` dùng chung cho `process_mail` và cuối vòng push — drain **FIFO** (message
   trước xử lý/ghi log trước), test mới `push_drain_fifo_va_mot_tcb_moi_hart_nguon` (a) kiểm
   `[Wake, Migrate]` cùng hộp thư.
2. **DoD 2 thiếu `debug_assert!` ở `publish`** — thêm `debug_assert_eq!` bất biến (iv)
   `published ⇔ HartSched` tại cửa ghi (sau khi đã validate + trả `Err` cho lỗi người dùng).
3. **`check_smp` hạng tử mailbox tautology** — thêm kiểm **≤ 1 yêu cầu `Migrate`/TCB**
   (`yeu_cau_cua`); ghi rõ hạng tử `pending` triệt tiêu hai vế, phần kiểm thật là nội dung +
   "≤1/TCB" (không còn gọi là "bảo toàn mailbox"). Unit test `check_smp_bat_published_lech_va_migrate_sai`
   vẫn phủ nhánh sai.
4. **"≤1 TCB/hart/bước" mơ hồ** — ghi rõ **mỗi hart nguồn/vòng** trong `k1c-smp.md` §3 và test;
   test mới (b) hai hart nguồn cùng đẩy sang một đích rảnh ⇒ đích nhận 2 (1/nguồn), mỗi nguồn
   mất đúng 1.
5. **`tcb_set_home` từ chối TCB `Running` cả khi `n = 1`** — gate điều kiện bằng
   `self.sched.len() > 1`; `n = 1` giữ nguyên hành vi cũ.

Sau sửa: chạy lại `kt29_smp` (12 test) + `l1_k1c` + `gate --fast` + Miri (log cập nhật bên dưới).

### 2c. `reviewer-kt29b`: CONFIRM 5/5 + 1 lỗi mới — đã vá

Reviewer xác nhận 5/5 điểm trên (gate 441/58, L1 747 ms, `kt29_smp` 12/12, Miri 12/12, scope
`v2/` sạch), nhưng phát hiện `post_mail` cho phép **2 yêu cầu `Migrate` cùng TCB** ⇒
`check_smp` bất biến (ii) vi phạm ⇒ `debug_assert!` panic ngay `post_mail` thứ hai (release tắt
assert nên ngữ nghĩa không đổi, nhưng debug/test build panic; không test nào phủ).

**Vá**: `check_migrate_request` (cửa của `post_mail`) từ chối yêu cầu thứ hai cho **cùng TCB**
đang chờ trong bất kỳ hộp thư ⇒ `InvalidArgument`; test mới `post_hai_migrate_cung_tcb_bi_tu_choi`
(lần 1 `Ok`, lần 2 `InvalidArgument`, `check_smp` xanh/không panic; xử lý xong yêu cầu mới hợp lệ).
Sau vá: `kt29_smp` **13/13**, `gate --fast` **PASS 442 test/58 suite**, L1 1.000 seed/999 trace **767 ms** (`kt29-gate-fast.log`); không retry.

## 3. Kết quả (lệnh + log)

- **Baseline** `./v2/bin/v2 gate --fast`: PASS **427 test/56 suite**, L1 1.000 seed/999 trace
  **725 ms** ([`kt29-baseline-gate-fast.log`](kt29-baseline-gate-fast.log)).
- **Gate cuối** (sau verdict, đã sửa 5 điểm) `./v2/bin/v2 gate --fast`: **PASS 441 test/58
  suite** (427 + 12 `kt29_smp` + 1 unit `check_smp` + 1 driver `l1_k1c`), fmt/clippy
  `-D warnings` sạch, gen-check ON, scn OK, R1..R5 OK; L1 1.000 seed/999 trace **756 ms**
  ([`kt29-gate-fast.log`](kt29-gate-fast.log)). Không retry (D2-07). (Trước verdict: 440 test,
  L1 663 ms — cùng bộ test, thêm 1 test FIFO sau verdict.)
- **Miri** `./v2/bin/v2 miri test --test kt29_smp` (sau verdict): **PASS 12/12**, 257,92 s
  ([`kt29-miri.log`](kt29-miri.log); fuzz `cfg(miri)` 3 seed × 8 op × smp 1/2/4/8; stable vẫn
  1.000 × 64; 2 warning integer-to-pointer của `vifast-objmem` là pre-existing).
- **Driver L1 theo smp**: `./v2/bin/v2 nightly --l1 --seeds 200 --smp 1,2,4,8` — xem mục 4.
- **Số liệu test**: 13 test `kt29_smp` + 1 unit `check_smp` + 1 driver L1; fuzz 1.000 seed × 64
  op × 4 cấu hình smp (256.000 op/lượt, chạy 2 lượt/seed để so trace).

## 4. Lệnh nightly --l1 (đo trong vòng task)

- `./v2/bin/v2 nightly --l1 --seeds 200 --smp 1,2,4,8` → **PASS** (800 lượt, 10,14 s;
  [`kt29-nightly-l1.log`](kt29-nightly-l1.log)).
- `./v2/bin/v2 nightly --l1 --seeds 2000 --smp 1,2,4,8` → **PASS** (8.000 lượt, 101 s;
  [`kt29-nightly-l1-2000.log`](kt29-nightly-l1-2000.log)); 100.000 seed thuộc cổng cuối phase
  §7.1, **không** chạy trong vòng task (Q13). (Log sau verdict được chạy lại; log trước verdict
  cùng lệnh vẫn PASS.)

## 4b. Commit + push

- Commit **`0ab2267`** `[KT-29] SMP mô hình tất định 1/2/4/8 + push migration (441 test, Miri
  12/12)` — chỉ stage `v2/` (18 tệp); `viscript/` của tab khác **không** chạm. `git push origin
  main` ff `1c3f1d3..0ab2267`; `HEAD..origin/main` = 0 trước/sau push. `dd4761e` ([KT-28]) là
  tổ tiên của `0ab2267`.

## 5. Đề xuất

**REVIEW** (không tự DONE): chờ reviewer độc lập (depth 1/1 — đã báo lead spawn reviewer). Các
điểm cần soi: (a) quy ước `published` trung thực + `publish` trả `Err` thay vì panic (DoD 2);
(b) đẳng thức bảo toàn `check_smp` với hạng tử mailbox đếm hai vế (không in-flight); (c) push tự
động ở `dispatch` (DoD 3 "và/hoặc dispatch"); (d) các guard mới của `tcb_set_affinity`/
`tcb_set_home`/`transition_tcb` (chỉ ảnh hưởng `n > 1`).

**YC mới (đề xuất, không tự thêm vào QUEUE)**: KT-30 (`vifast-sync` MCS lock + Mailbox MPSC
lock-free) như K1C-PLAN; cân nhắc KT-29b cho IPI/timer mô hình + `TlbFlush`/`VmKick`/`PowerReq`
(hiện chỉ `Wake`/`Reschedule`/`Migrate`/`Stop`) — thuộc K3/KT-30 nếu Agent 0 muốn.
