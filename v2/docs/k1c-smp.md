# SMP 1/2/4/8 + push migration (KT-29) — model-only `[TK]`

Tài liệu này mô tả **mô hình** SMP tất định của `vifast-sim` cho K1c (KT-29). Đây **không phải**
`spec/`, **không phải** ABI: không thêm method/chữ ký nào, không đổi `spec/abi.toml`. Nguồn ngữ
nghĩa: thiết kế §4.8 (`docs/v2/ViFastOs-v2-thiet-ke.md:336-344`), §4.16 luật 3 (`:412-419`),
§4.17 (`:423-428`); **D2-53 Q1/Q2**; `review/agent0/K1C-PLAN.md` §5 KT-29.

## 1. Phạm vi và hằng số `[TK]`

- **Không thread, không QEMU, không atomic**: mô hình một luồng mô phỏng **N hart tất định**.
  `Machine::with_harts(n)` cho `1 ≤ n ≤ MAX_HARTS`; `n = 0`/`n > MAX_HARTS` ⇒ `assert!` (cùng
  kiểu `with_arena_bytes`). `Machine::new()` **vẫn 1 hart** để không đổi ngữ nghĩa các test cũ.
- `MAX_HARTS = 8` `[TK]` (trần cấu hình 64 của thiết kế §4.16 là hằng model để K3; K1c chỉ cần
  1/2/4/8). `HART_COUNT = 1` chỉ còn là **số hart mặc định** của `Machine::new()`.
- Mỗi hart có **một** `HartSched` (256 mức + bitmap, KT-19), **một** `HartMailbox` (KT-22) và
  **một** trường công bố `Published`.
- Mọi API cũ (`ready_len`, `current`, `sched_idle`, `dispatch`, `wake`, …) giữ nguyên nghĩa với
  `n = 1`; các guard mới `tcb_set_home` (từ chối TCB `Running`) chỉ bật khi `n > 1`; `n > 1` dùng thêm `hart_count`, `current_hart`/`set_current_hart`, `published`/
  `publish`, `push_migration`, `migrate_count`, `mailbox_len`, `check_smp`/`check_harts`.

## 2. Trường công bố `Published { idle, cur_prio }` và guard luật 3

- `Published.idle` ⇔ `current == None`; `Published.cur_prio` = mức ưu tiên hiệu lực của TCB đang
  chạy (`0` khi rảnh). Model **làm tươi** trường này từ `HartSched` sau **mỗi** public op, nên nó
  luôn là ảnh trung thực của scheduler (bất biến `check_smp` mục 4).
- **Luật 3** (§4.16): hart chỉ được **ghi** trạng thái của chính nó. `Machine::publish(hart, idle,
  prio)` chỉ thành công khi `current_hart() == hart` **và** giá trị khớp trạng thái thật; ghi hart
  khác (hoặc nói dối trạng thái) ⇒ `Err(IllegalOperation)`, **không** đổi gì. Sau khi ghi, model
  `debug_assert!` bất biến (iv) `published ⇔ HartSched` (lỗi nội bộ lộ tại cửa ghi). Thiết kế nói
  "panic" cho build sim; mô hình trả lỗi cho **lỗi người dùng** để test âm quan sát được mà
  **không** cần `#[should_panic]` (K1C-PLAN §5 KT-29 DoD 2 — quy ước có chủ ý, ghi rõ để review).
- `published(hart)` **đọc** được từ mọi hart — đúng ngoại lệ "đọc trường công bố là được phép"
  của luật 3. Push migration chỉ đọc `published(y).idle`; **không** hart nào ghi trạng thái hart
  khác, **không** work stealing.

## 3. Push migration (D2-53 Q2)

Một **vòng push** (`push_step`, gọi công khai qua `push_migration`, tự động ở `dispatch` và sau
vòng preempt khi `preempt_every_step` bật):

1. Với **mỗi hart nguồn `x`** theo thứ tự tăng dần: nếu hàng của `x` có **> 1** TCB `Ready` ở
   **mức cao nhất** (`highest_priority`) thì lấy TCB **cuối hàng** mức đó (`last_at`).
2. Tìm hart đích `y ≠ x` **nhỏ nhất** sao cho: `y` nằm trong **affinity** của TCB **và**
   `published(y).idle == true`. Không có ⇒ hart `x` không đẩy gì.
3. Đẩy **đúng một** TCB/hart **nguồn**/vòng qua `MailboxMsg::Migrate { tcb, to_hart }` vào hộp
   thư `y`; hộp thư đầy ⇒ **bỏ** yêu cầu (backpressure, không mất TCB). `post_mail` kiểm **tại
   cửa vào**: chỉ **một** yêu cầu `Migrate`/TCB được chờ trong mọi hộp thư — yêu cầu thứ hai cho
   cùng TCB ⇒ `InvalidArgument` (khớp bất biến (ii) `check_smp`, không để `debug_assert` phải bắt). "≤ 1 TCB/hart/bước"
   (D2-53 Q2) được hiểu là **mỗi hart nguồn**: nếu nhiều hart nguồn cùng chọn một hart đích rảnh
   trong cùng vòng thì đích nhận **một TCB từ mỗi nguồn** (≤ `n − 1`), không phải ≤ 1/đích.
4. **Cuối vòng**, mỗi hart nhận `Migrate` xử lý hộp thư theo **FIFO** (cùng một hàm drain với
   `process_mail`): message đến trước (`Wake`/`Reschedule`/`Stop`) được xử lý + ghi log **trước**
   `Migrate`, không rút `Migrate` từ giữa hàng (giữ nguyên ngữ nghĩa KT-22). `Migrate` gỡ TCB
   khỏi hàng nguồn, đặt `home = y`, enqueue **cuối hàng** mức hiện tại của `y`,
   `migrate_count += 1`. Không có trạng thái "in-flight" giữa bước (rủi ro (a) của KT-29).

Guard âm: hart đích **bận** (`published.idle == false`) ⇒ 0 migrate; đích **ngoài affinity** ⇒ 0;
nguồn chỉ có **1** TCB ở mức cao nhất ⇒ 0; TCB `Running` **không** bị đẩy (chỉ lấy từ ready
queue); hart rảnh **không** tự kéo việc (không work stealing).

## 4. Wake chéo hart

- `wake(tcb)` khi `home(tcb) != current_hart()` (và `n > 1`) **không** ghi trạng thái hart khác:
  gửi `MailboxMsg::Wake(tcb)` vào hộp thư của `home`. Hart đích xử lý hộp thư mới đánh thức.
- Khi xử lý `Wake`, hart đích đánh thức TCB rồi **chỉ preempt** (`dispatch`) nếu ưu tiên hiệu lực
  của TCB **>** `cur_prio` công bố của đích (thiết kế §4.8 `:342`). Cùng mức/không cao hơn ⇒ TCB
  chỉ `Ready`.
- `wake` cùng hart (và `n = 1`) giữ nguyên đường cũ: đánh thức trực tiếp, không qua hộp thư.

## 5. Bất biến `check_smp` / `check_harts`

Kiểm sau **mỗi** op (và trong `sched_after_op` khi `n > 1`):

1. Không TCB nào `Running` trên **2** hart; `current` mỗi hart đúng `Running{hart}`.
2. **Bảo toàn TCB**: số TCB `Ready` (không treo)/`Running` = số TCB trong 256 hàng + `current`
   + số yêu cầu `Migrate` đang chờ. Vì mô hình **không** có in-flight (TCB vẫn nằm hàng nguồn khi
   yêu cầu đang chờ), hạng tử mailbox được đếm ở **cả hai vế** (triệt tiêu); phần kiểm **thật**
   là: mỗi TCB đúng một chỗ, **≤ 1 yêu cầu `Migrate`/TCB**, và nội dung từng yêu cầu (hart đích
   hợp lệ, `home != to_hart`, cả hai trong affinity, TCB sống `Ready`/`Running`).
3. `home` khớp hart đang giữ `current`/hàng.
4. `published.idle` ⇔ `current == None`; `published.cur_prio` khớp `cur_prio` thật.
5. Không TCB nào nằm trong hàng của hart **ngoài** affinity; `home` luôn trong affinity; mỗi TCB
   tối đa một chỗ.

Checker **không** rỗng: unit test trong `kobj.rs` gieo `published` lệch, TCB trong hàng sai hart
và yêu cầu `Migrate` sai để chắc `check_smp` bắt được.

## 6. Tất định và driver L1

- Không `HashMap`, không thời gian thực, không atomic: mọi quyết định (hart đích nhỏ nhất, TCB
  cuối hàng, thứ tự drain FIFO) là hàm của trạng thái ⇒ **cùng seed cùng trace**.
- `tests/kt29_smp.rs`: fuzz `1.000 seed × 64 op × smp {1,2,4,8}` (dưới Miri giảm còn
  `3 seed × 8 op` bằng `cfg(miri)` — stable vẫn 1.000 × 64); mỗi bước kiểm
  `check_sched` + `check_harts` + `check_ipc` + `check_mailboxes`.
- `tests/l1_k1c.rs`: driver L1 đọc `VIFAST_SMP` (mặc định `1`) và `VIFAST_SEEDS` (mặc định
  `1000`). `gate --fast` chạy mặc định (1.000 seed / smp 1, không nới ngưỡng);
  `./v2/bin/v2 nightly --l1 [--seeds N] [--smp a,b,c]` quét 1/2/4/8 với seed lớn (100k seed là
  cổng cuối phase §7.1, không nằm trong `--fast`).

## 7. Không làm ở KT-29 (giữ đúng phạm vi)

- Không `vifast-sync`/MCS lock/MPSC lock-free/loom (KT-30); không thread thật/QEMU.
- Không đổi ngữ nghĩa IPC/Notification/WaitAddr; không sửa `spec/`, `vifast-model`, `.scn`,
  QUEUE/DECISIONS/AGENTS/`viscript/`; không dependency/`unsafe`/`#[allow]`/`#[ignore]`; không
  đổi ABI.
- Chưa mô hình IPI/timer phần cứng, `TlbFlush`/`VmKick`/`PowerReq`, ack thế hệ (K3/KT-30).

## 8. Lệnh kiểm

```bash
cargo test --manifest-path v2/Cargo.toml -p vifast-sim --test kt29_smp
./v2/bin/v2 gate --fast                 # 1.000 seed L1/smp 1 + toàn bộ L0
./v2/bin/v2 nightly --only-miri         # Miri trên tập chọn, có kt29_smp
./v2/bin/v2 nightly --l1 --seeds 10000 --smp 1,2,4,8
```

Log: `v2/agent/JOURNAL/kt29-red-compile.log` (đỏ trước), `kt29-red-semantics.log` (probe API
cũ), `kt29-gate-fast.log`, `kt29-miri.log`.
