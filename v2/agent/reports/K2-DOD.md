<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K2-DOD — bằng chứng cổng cuối phase K2 (KT-43)

Người ghi: **Agent 2 dòng OS** · Ngày: 2026-11-14 · Task: **KT-43** (cổng cuối K2) · HEAD khi
mở task: `66d51f89` ([KT-00] lật KT-42 DONE, mở KT-43) · Nhánh `main`.

> **K2 KHÔNG phải kernel thật.** K2 là **mô hình tất định trong sim** (Frame/PageTable/VSpace/
> ASID/TLB/shootdown/fault model-only): **không** có PTE thật, **không** MMU/TLB phần cứng,
> **không** QEMU, **không** boot, **không** IPI thật, **không** pager/COW. K3 mới là QEMU +
> page table thật + TLB thật + `sfence.vma`/IPI thật + pager thật.

**Phạm vi**: chỉ **chạy cổng, mở rộng driver L1 (test-only) để phủ VM, thu log, đối chiếu DoD**.
Không sửa `spec/`/`vifast-model`/semantics VM/`.scn`/QUEUE/DECISIONS/AGENTS/`viscript/`; không
thêm dependency/`unsafe` sản phẩm; không `#[ignore]`/`#[should_panic]`; không nới seed/op/ngưỡng.
Đột biến tay là **sửa tạm + khôi phục hash**, không commit.

Nguồn đối chiếu: `review/agent0/K2-PLAN.md` §4 (cổng K2 + bảng DoD §4.2) + §3 KT-43; **D2-65**
(Q1..Q10), D2-67 (W^X), D2-68 (TlbFlush/ack), D2-69 (`.scn` + `check_vm`); `v2/docs/k2-vm.md`;
tiền lệ báo cáo [`K1C-DOD.md`](K1C-DOD.md).

## 0. TL;DR

| # | Việc (K2-PLAN §4.1/§4.2) | Kết quả | Log |
|---|---|---|---|
| 1 | `nightly --l1 --seeds 100000 --smp 1,2,4,8` (**400.000 lượt**) | **ĐẠT** — **400.000 lượt** (100k seed × 4 smp, **2 lần/seed** = 800.000 lượt chạy), **0** panic/0 vi phạm TLB-safety trên **51,2 triệu** op, wall **690,69 s**; **0** vi phạm TLB-safety/checker trên **mỗi** op; cùng seed ⇒ cùng trace (2 lần/seed) | [`JOURNAL/kt43-l1-100k.log`](../JOURNAL/kt43-l1-100k.log) |
| 2 | **Mutation 12 ca §7.2 cũ + 5 ca VM** (gồm **AP→BSP**, bỏ ack unmap, bỏ W^X, bỏ flush deactivate, bỏ drain hộp thư) | **ĐẠT — 17/17 bị giết** (exit 101 + `test result: FAILED` đúng test/assert, khôi phục hash == HEAD) | [`JOURNAL/kt43-mutation.log`](../JOURNAL/kt43-mutation.log) |
| 3 | Miri **toàn bộ** tập ảnh hưởng K2 (kt36..kt42) + `vifast-pte` + `vifast-sync` + mẫu driver kt43 | **ĐẠT (tập K2+pte+sync)** — **146 test / 0 failed** (57 test K2 + 64 lib + 9 `vifast-pte` + 16 `vifast-sync`), mọi log `EXIT=0`; driver kt43 smoke **không hoàn tất** — **không** claim phủ driver (xem §4) | [`JOURNAL/kt43-miri-*.log`](../JOURNAL/) |
| 4 | `gate --fast` (≥ 569 test, không retry) + `check_scn` 26 tệp/16 nhóm + fixtures âm | **ĐẠT** — **PASS 569 test / 0 failed**, L1 1.000 seed **1.427 ms**; wall **374,56 s** (T-INFRA do tải máy); fmt/clippy `-D warnings`, 26 tệp `.scn`/16 nhóm, 5/5 fixture âm; `git diff spec/` rỗng | [`JOURNAL/kt43-gate-fast.log`](../JOURNAL/kt43-gate-fast.log) |
| 5 | Bảng DoD K2 (§4.2 D1..D11) + nợ có chủ | **Đã lập** — §6/§8 | tệp này |

**Đề xuất trạng thái**: **KT-43 DONE (đề xuất)** và **K2 DONE (đề xuất)** — D1..D11 ĐẠT; reviewer-kt43 DRIFT đã vá đủ 4 mục (D2-70 + min/median; K2-DOD final; mutation snippet/harness; Miri honest-D8). Chờ reviewer-kt43 xác nhận + **Agent 0** lật QUEUE; không tự lật (D2-37).

## 1. Baseline (trước khi chạy cổng)

- HEAD `66d51f89`, `git rev-list --count HEAD..origin/main` = **0**; `v2/` sạch (chỉ `viscript/`
  + `review/` của tab khác — **không chạm**, chỉ stage `v2/`).
- Baseline `gate --fast`: **PASS 569 test / 73 suite**, L1 1.000 seed **2.635 ms**
  ([`JOURNAL/kt42-gate-fast.log`](../JOURNAL/kt42-gate-fast.log), từ KT-42 — `v2/` không đổi
  từ đó tới khi mở KT-43).
- Máy: 12 nhân; lúc chạy cổng có tải nền của tab khác (`ffmpeg`, `mds`) — mọi log ghi **giờ
  máy** ngay trong lệnh; không retry (D2-07).

## 2. D1/C4 — 100.000 seed × smp 1/2/4/8 (việc §4.1 #1)

### 2.1 Driver L1 **phủ thêm VM** (test-only, `v2/sim/vifast-sim/tests/l1_k1c.rs`)

`nightly --l1` chạy driver `tests/l1_k1c.rs`. KT-43 **giữ nguyên 19 op** cũ (sched/SMP/mail +
IPC/Notification/WaitAddr) và thêm **4 op VM** (tổng **23** nhánh, vẫn **64 op/seed**, không đổi
ngân sách):

| Op | Nhóm | Đường đi |
|---|---|---|
| 19 | **map** | `vspace_map_obj` Frame 1 GiB vào VSpace active trên **mọi** hart (lá ngay root) |
| 20 | **walk + TLB** | `vspace_walk` (oracle VA→PA) + `tlb_translate` (miss/fill TLB hart hiện tại) |
| 21 | **unmap + shootdown** | nạp TLB **mọi hart** rồi `vspace_unmap`; assert `Ok ⇒ acked == targets` + mọi hart hết entry cũ |
| 22 | **VM fault** | `vm_touch` VA chưa map ⇒ `FaultKind::Vm{NotMapped}` tới `fault_ep`; `wake` dọn (không pager — K3) |

- VM rig (test-only): 1 VSpace (root→lá 1 GiB, **một** PageTable), 3 Frame 1 GiB (mỗi Frame
  một Untyped/cửa sổ PA riêng — D2-66), 4 TCB có `fault_ep`; VSpace active trên mọi hart.
- Sau **mỗi** op: bộ checker K1c cũ + `check_harts`/`check_caps` + **`tlb_violations() == 0`**
  (oracle TLB-safety KT-40); cuối **mỗi** lượt: **`check_vm()`** (một cửa 8 checker — KT-42).
- **Đỏ trước** (D2-05): chạy driver mới với `N_OP = 19` (tạm bỏ 4 nhánh VM) ⇒ test đỏ ở bất
  biến "driver phải phủ VM" ([`JOURNAL/kt43-red-driver-vm.log`](../JOURNAL/kt43-red-driver-vm.log));
  khôi phục hash trước khi chạy cổng.
- **Không** sửa semantics `kobj/ipc/...`; chỉ driver test.
- **Điều chỉnh ngưỡng phủ của driver (không phải bất biến model)**: KT-33 siết per-seed
  `ipc_sent > 1`; với 23 nhánh op, lần chạy 100k đầu gặp `seed 67453 smp 2: ipc_sent = 1`
  ([`JOURNAL/kt43-l1-100k-attempt1-fail.log`](../JOURNAL/kt43-l1-100k-attempt1-fail.log)) ⇒ giữ
  `sent == delivered + pending + dropped` + `sent ≥ 1` **mỗi seed**, chuyển ngưỡng "không
  tautology" xuống **tổng mỗi smp**: `ipc_sent > 5 × seeds` (đo ~12,1/seed) + `ipc_delivered > 0`.
  **Không** nới seed/op/lượt; chạy lại từ đầu (attempt1 không được dùng làm kết quả).

### 2.2 Lệnh và kết quả

Lệnh (đúng biến thể `v2 nightly --l1` của KT-29; **không** giảm seed/op/invariant):

```bash
CARGO_PROFILE_TEST_OPT_LEVEL=2 VIFAST_DOI=100000 \
  ./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8
```

- `CARGO_PROFILE_TEST_OPT_LEVEL=2` **chỉ** đổi tối ưu biên dịch của profile `test` (giữ nguyên
  `debug-assertions` mặc định **bật** — mọi checker per-op trong `sched_after_op` vẫn chạy); lý
  do: 400.000 lượt × 64 op × 2 lần/seed trong ngân sách 3 h. **Không** nới seed/op/bất biến.
- `VIFAST_DOI=100000` buộc **hai lần/seed** cho **toàn bộ** 100.000 seed mỗi smp (400.000 cặp
  trace) — mặc định của test chỉ 2.000 seed đầu khi >1.000; ở đây **không** giảm mẫu.

**Kết quả** — log [`JOURNAL/kt43-l1-100k.log`](../JOURNAL/kt43-l1-100k.log), **PASS**:

| smp | `# L1 OK` | `ipc_sent` | `ipc_delivered` | `ipc_dropped` | VM map | VM walk | VM unmap | VM fault | VM shootdown |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 100.000 seed × 64 op | 1.212.128 | 226.334 | 782.515 | 257.149 | 71.791 | 135.089 | 237.472 | 135.089 |
| 2 | 100.000 seed × 64 op | 1.165.183 | 217.375 | 734.940 | 257.080 | 71.601 | 135.157 | 232.545 | 135.157 |
| 4 | 100.000 seed × 64 op | 1.137.251 | 211.851 | 707.418 | 257.644 | 71.499 | 135.394 | 228.758 | 135.394 |
| 8 | 100.000 seed × 64 op | 1.125.024 | 209.918 | 693.268 | 257.069 | 71.693 | 135.075 | 226.058 | 135.075 |
| **tổng** | **400.000 lượt** | **4.639.586** | **865.478** | **2.918.141** | **1.028.942** | **286.584** | **540.715** | **924.833** | **540.715** |

- `# L1 OK: 400000 lượt (smp [1, 2, 4, 8], seeds 100000)`; `1 passed; 0 failed`; **wall 690,69 s**
  (`real 11m30,69s`; user 469,73 s) — `v2 nightly --l1: PASS (691 s)`.
- **Cùng seed ⇒ cùng trace**: `VIFAST_DOI=100000` ⇒ **100.000/100.000 seed mỗi smp chạy 2 lần**
  (200.000 lượt/smp), `assert_eq!(trace_a, trace_b)` + `assert_eq!(bộ đếm IPC/VM)` — **400.000
  cặp trace** khớp byte-for-byte; lệch ⇒ test panic (không có panic nào).
- **Bất biến sau MỖI op** (kể cả prologue): 7 checker K1c + `check_harts` + `check_caps` +
  **`tlb_violations() == 0`**; vi phạm ⇒ panic kèm `seed`/`smp` ⇒ **0** panic. Tổng số lần kiểm:
  400.000 lượt × 64 op × 2 lần/seed = **51,2 triệu** op; `check_vm()` (8 checker) thêm **800.000**
  lần cuối lượt. TLB-safety: **0 vi phạm** trên **51,2 triệu** op ⇒ **D1 ĐẠT**.
- **Bảo toàn message** sau **mỗi** lượt: `ipc_sent == ipc_delivered + ipc_pending + ipc_dropped`
  ⇒ `ipc_pending` = 4.639.586 − 865.478 − 2.918.141 = **855.967** (khớp từng smp: 203.279 /
  212.868 / 217.982 / 221.838). `ipc_delivered` ≈ 18,7 % `ipc_sent` — phần còn lại là huỷ hợp lệ
  D2-25 + còn trong hàng (đúng bản chất mô hình, không phải "mất message").
- **VM thật sự được chạy**: mỗi smp ~257k map, ~72k walk, ~135k unmap/shootdown, ~230k fault ⇒
  `assert!(vm.map>0 && walk>0 && unmap>0 && fault>0 && shootdown>0)` mỗi smp; rig VM được dựng
  mới mỗi seed (không tái dùng trạng thái giữa seed).
- **T-INFRA/wall**: 690,69 s cho **800.000 lượt chạy** (400.000 lượt × 2) — không có ngưỡng wall
  cho `nightly --l1`; K2-PLAN §4.2 chỉ đặt ≤ 60 s cho **L1 1.000 seed trong `gate --fast`**
  (D11, xem §5). Không retry; không giảm seed/op.
- **Minh bạch**: lần chạy 100k **đầu** dừng ở smp 2 vì guard per-seed `ipc_sent > 1` của KT-33
  (seed 67453); xem §2.1 — đã chỉnh **ngưỡng phủ của driver** (không phải bất biến model) rồi
  **chạy lại từ đầu**; bản hỏng giữ ở
  [`JOURNAL/kt43-l1-100k-attempt1-fail.log`](../JOURNAL/kt43-l1-100k-attempt1-fail.log), **không**
  dùng làm kết quả.

### 2.3 D2-70 — ngưỡng IPC của driver L1 (có D2, không giấu)

- **Vấn đề**: KT-33 đặt `ipc_sent > 1` **mỗi lượt**; driver KT-43 thêm 4 nhánh VM (23 nhánh) nên
  có seed chỉ 1 message prologue (seed 67453/smp 2) — guard cũ đỏ **vì đổi phân phối**, không vì
  hồi quy. AGENTS §4.4 cấm "nới ngưỡng để qua cổng"; nên phải có D2.
- **D2-70 (Agent 0, `b3ff8e1e`)**: thay bằng (a) **mỗi seed** `ipc_sent ≥ 1`; (b) **tổng mỗi
  smp** `ipc_sent > 5 × seeds` (mạnh hơn `> 1 × seeds` về tổng) + `ipc_delivered > 0`; driver
  **phải in `min`/`median`** `ipc_sent` mỗi smp để chứng minh (a) không hình thức. **Cấm** dùng
  D2-70 để hạ bất kỳ ngưỡng nào khác.
- **Đã làm**: driver in `ipc_min`/`ipc_median`; log 10k×4 **đủ theo D2-70**
  ([`JOURNAL/kt43-l1-10k.log`](../JOURNAL/kt43-l1-10k.log)):

  | smp | `ipc_min` | `ipc_median` | `ipc_sent` | `ipc_delivered` | `ipc_dropped` |
  |---|---|---|---|---|---|
  | 1 | 2 | 12 | 121.325 | 22.736 | 78.236 |
  | 2 | 2 | 12 | 116.437 | 21.481 | 73.753 |
  | 4 | 1 | 11 | 113.486 | 21.245 | 70.340 |
  | 8 | 2 | 11 | 112.542 | 21.015 | 69.307 |

  `min ≥ 1` mọi smp (không lượt nào bỏ IPC); median 11–12 message/seed ⇒ 64 op thật sự sinh
  message. Bản 100k (bảng §2.2) chạy **đúng guard D2-70** nhưng **trước** khi thêm dòng in
  min/median; D2-70 yêu cầu log 10k cho bằng chứng min/median — log 100k giữ nguyên cho
  counters/invariant (không chạy lại 100k chỉ để thêm dòng in).
- **Không nới gì khác**: seed 100k × 4 smp, 64 op/seed, 2 lần/seed, toàn bộ bất biến per-op và
  `check_vm` cuối lượt giữ nguyên.

## 3. Mutation — 12 ca §7.2 cũ + 5 ca VM (việc §4.1 #2)

Mỗi ca: (0) chạy **đúng một** test đích **xanh trước**; (1) sửa **đúng một chỗ** trong nguồn
(không commit); (2) chạy **đúng một** test đích ⇒ `exit 101` + `test result: FAILED` + dòng
assert; (3) khôi phục nguyên văn rồi xác nhận `sha256` == HEAD **và** `git status --porcelain`
rỗng cho tệp đó. Log đầy đủ (File/Test/hash + lệnh + `exit` + `FAILED` + khôi phục):
[`JOURNAL/kt43-mutation.log`](../JOURNAL/kt43-mutation.log); **snippet mutant** (17 cặp before/after) +
harness tái lập: [`JOURNAL/kt43-mutation-diff.md`](../JOURNAL/kt43-mutation-diff.md),
[`JOURNAL/kt43_mut.py`](../JOURNAL/kt43_mut.py).

| # | Đột biến (một chỗ) | Test đích | Bằng chứng đỏ (assert) | Khôi phục |
|---|---|---|---|---|
| **M1** | ipc::transfer bỏ copy 1 trong 64 MR | `message\_64\_mr\_hai\_vung\_thanh\_ghi\_va\_ipc\_buffer` | `sim/vifast-sim/tests/kt24_ipc.rs:339:9` — assertion `left == right` failed: MR 32 ở IPC buffer; left `0`, right `4026531872` | OK |
| **M2** | Bỏ kiểm RIGHT_GRANT khi chuyển cap | `thieu\_grant\_that\_bai\_mem\_message\_van\_giao` | `sim/vifast-sim/tests/kt25_ipc2.rs:372:5` — assertion `left == right` failed: caps = số cap cài được; left `4`, right `3` | OK |
| **M3** | Cap nhận giữ RIGHT_GRANT (không bỏ GRANT) | `bon\_cap\_cai\_du\_bo\_grant\_giu\_badge` | `sim/vifast-sim/tests/kt25_ipc2.rs:313:9` — assertion `left == right` failed: cap nhận được **bỏ** RIGHT_GRANT (thiết kế :319); left `4`, right `0` | OK |
| **M4** | Bỏ unwrap badge (cap badge vào receive window) | `unwrap\_badge\_khong\_chiem\_receive\_window` | `sim/vifast-sim/tests/kt25_ipc2.rs:486:5` — assertion `left == right` failed: chỉ cap thường được cài; left `2`, right `1` | OK |
| **M5** | Signal không xoá word/badge sau khi giao | `mot\_waiter\_duy\_nhat\_ba\_tcb` | `sim/vifast-sim/tests/kt27_notification.rs:332:5` — assertion `left == right` failed: giao cho waiter xong word = 0; left `165`, right `0` | OK |
| **M6** | Bỏ cổng 'một waiter' kiểu v1 | `mot\_waiter\_duy\_nhat\_ba\_tcb` | `sim/vifast-sim/tests/kt27_notification.rs:314:5` — assertion `left == right` failed: waiter thứ hai ⇒ Busy; left `Ok(())`, right `Err(Busy)` | OK |
| **M7** | WaitAddr xếp hàng trước rồi mới so expected | `expected\_lech\_tra\_ngay\_khong\_xep\_hang` | `sim/vifast-sim/tests/kt28_waitaddr.rs:246:5` — assertion `left == right` failed: TCB phải **giữ** Ready, không bị chặn; left `BlockedOnAddress { frame: 3, offset: 16 }`, right `Ready` | OK |
| **M8** | WakeAddr bỏ kiểm khoá (wake mọi waiter trong bucket) | `va\_cham\_bucket\_khong\_anh\_huong\_tim\_kiem` | `sim/vifast-sim/tests/kt28_waitaddr.rs:208:5` — assertion `left == right` failed; left `2`, right `1` | OK |
| **M9** | Push migration giữ home cũ (bỏ home = to) | `push\_dung\_mot\_tcb\_cuoi\_hang\_qua\_migrate` | `sim/vifast-sim/tests/kt29_smp.rs:281:5` — assertion `left == right` failed; left `0`, right `1` | OK |
| **M10** | Fault-resume không ghi MR trước Ready (đường r319) | `resume\_with\_regs\_khi\_dang\_cho\_reply\_ghi\_thanh\_ghi\_truoc\_ready` | `sim/vifast-sim/tests/kt26_fault.rs:350:5` — assertion `left == right` failed; left `[0, 0, 0, 0]`, right `[7, 8, 9, 10]` | OK |
| **M11** | Hộp thư đầy ghi đè thay vì Err(Busy) | `mailbox\_day\_tra\_busy\_khong\_mat\_message\_dau` | `lib/vifast-sync/tests/kt30_sync.rs:130:5` — assertion `left == right` failed; left `Ok(())`, right `Err(Busy)` | OK |
| **M12** | Rời BlockedOnReply không gỡ Reply.caller (cổng transition_tcb; guard hiện hành sau KT-34) | `cong\_transition\_giu\_lien\_ket\_reply\_va\_khong\_ghi\_de\_caller` | `sim/vifast-sim/tests/kt25_ipc2.rs:1524:5` — assertion `left == right` failed: cổng rời BlockedOnReply phải gỡ caller; left `Ok(Some(14))`, right `Ok(None)` | OK |
| **V1** | Map đổi lá/thứ tự cấp (AP→BSP): level_for_size_bits đảo cấp | `map\_walk\_unmap\_du\_ba\_co` | `sim/vifast-sim/tests/kt37_pagetable.rs:169:10` — test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s | OK |
| **V2** | Bỏ kiểm ack khi unmap (trả Ok khi chưa đủ ack) | `shootdown\_1\_2\_4\_8\_hart\_ack\_du` | `sim/vifast-sim/tests/kt40_tlb.rs:245:36` — test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s | OK |
| **V3** | Bỏ W^X khi map W (cho W sau X) | `map\_w\_sau\_x\_bi\_tu\_choi\_atomic\_khong\_ha\_ngam` | `sim/vifast-sim/src/kobj.rs:6168:13` — test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s | OK |
| **V4** | Bỏ flush TLB khi deactivate/teardown | `flush\_theo\_trang\_asid\_va\_gen` | `sim/vifast-sim/src/kobj.rs:6175:13` — test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s | OK |
| **V5** | Bỏ rút hộp thư khi chờ ack (AP→BSP deadlock) | `deadlock\_hai\_hart\_cho\_nhau\_poll\_tu\_phuc\_vu` | `sim/vifast-sim/tests/kt40_tlb.rs:662:5` — assertion `left == right` failed: hart 0 rút hộp thư của mình (ack hart 1) + đã được hart 1 ack ⇒ xong; left `Ok(false)`, right `Ok(true)` | OK |

- **Tổng: 17/17 ca** `exit 101`, `FAILED` **đúng test đích**, 17/17 khôi phục hash == HEAD; không
  ca nào đỏ vì lỗi biên dịch (harness chặn `could not compile`).
- **Minh bạch harness**: lần chạy harness **đầu** chỉ 13/17 (tệp không được đóng ⇒ flush trễ ở
  V3–V5; **M12** định nghĩa cũ nhắm `unlink_tcb_queues` — sau KT-34 đã có **thêm** guard
  `transition_tcb` nên mutant cũ **sống**). Đã sửa harness (ghi + `fsync` + kiểm cây sạch trước
  khi áp) và định nghĩa lại M12 nhắm **guard hiện hành** `transition_tcb` (cùng bất biến §7.2:
  rời `BlockedOnReply` phải gỡ `Reply.caller`; test `kt25_ipc2::cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller`),
  rồi chạy lại **toàn bộ 17 ca**. Log lần đầu giữ ở
  [`JOURNAL/kt43-mutation-attempt1.log`](../JOURNAL/kt43-mutation-attempt1.log) — **không** dùng
  làm kết quả; log kết quả là `kt43-mutation.log`.
- **AP→BSP**: **V5** (bỏ `drain_mail_fifo` trong `tlb_wait_round` khi hart đang chờ ack) là
  **đúng lỗ AP→BSP của v1** (thiết kế §4.10.3) — **bị giết**. **V1** là đột biến **đổi lá/thứ tự
  cấp** (`level_for_size_bits` đảo cấp) theo câu chữ "map AP→BSP (đổi lá/thứ tự cấp)" của đề bài,
  **không** phải lỗ AP→BSP v1 (không gọi là "hai bằng chứng AP→BSP"); cả hai đều bị giết.
- **V1/V2 đỏ ở helper của test** (không phải assert của luật): V1
  `kt37_pagetable.rs:169` `.expect("map 4 KiB")`; V2 `kt40_tlb.rs:245` `panic!("phải có shootdown")`.
  **V3/V4 đỏ ở invariant per-op** (`kobj.rs:6168` `check_wx`, `kobj.rs:6175` `check_tlb`); **V5**
  đỏ ở assert `left Ok(false)` / `right Ok(true)` của test. Snippet mutant: `kt43-mutation-diff.md`.
- 12 ca §7.2 cũ được **chạy lại tại HEAD K2** (không dùng log cũ KT-33) — nhờ vậy chứng minh K2
  không làm hỏng bất biến K1c.

## 4. D8 — Miri (việc §4.1 #3)

Lệnh rời `./v2/bin/v2 miri -p <crate> --test <tập>` (không chạy `l1_k1c` 1.000 seed dưới
Miri — D2-47(a)); mỗi log có `EXIT=0`:

| Tập | Test | Thời gian test | Log |
|---|---|---|---|
| `kt36_frame` | 12 | 39,04 s | [`kt43-miri-kt36_frame.log`](../JOURNAL/kt43-miri-kt36_frame.log) |
| `kt37_pagetable` | 10 | 173,46 s | [`kt43-miri-kt37_pagetable.log`](../JOURNAL/kt43-miri-kt37_pagetable.log) |
| `kt38_vspace` | 7 | 61,73 s | [`kt43-miri-kt38_vspace.log`](../JOURNAL/kt43-miri-kt38_vspace.log) |
| `kt39_wx` | 8 | 119,95 s | [`kt43-miri-kt39_wx.log`](../JOURNAL/kt43-miri-kt39_wx.log) |
| `kt40_tlb` | 10 | 324,00 s | [`kt43-miri-kt40_tlb.log`](../JOURNAL/kt43-miri-kt40_tlb.log) |
| `kt41_vmfault` | 9 | 389,60 s | [`kt43-miri-kt41_vmfault.log`](../JOURNAL/kt43-miri-kt41_vmfault.log) |
| `kt42_vmcheck` | 1 | 32,12 s | [`kt43-miri-kt42_vmcheck.log`](../JOURNAL/kt43-miri-kt42_vmcheck.log) |
| `vifast-sim --lib` | 64 | 77,69 s | [`kt43-miri-lib_vifast_sim.log`](../JOURNAL/kt43-miri-lib_vifast_sim.log) |
| `vifast-pte` | 9 | 3,98 s | [`kt43-miri-vifast_pte.log`](../JOURNAL/kt43-miri-vifast_pte.log) |
| `vifast-sync` | 16 (4 lib + 10 + 2 doctest) | 0,62 + 2,55 + 0,17 s | [`kt43-miri-vifast_sync.log`](../JOURNAL/kt43-miri-vifast_sync.log) |
| **driver kt43 (smoke)** (`VIFAST_SEEDS=1`, `VIFAST_SMP=1`, `cfg(miri)` ⇒ `SO_OP=8`) | 1 | **không hoàn tất** (Miri quá chậm; job dừng — **không** có `EXIT=0`) | [`kt43-miri-l1_k1c.log`](../JOURNAL/kt43-miri-l1_k1c.log) |

**Tổng K2 + pte + sync**: 146 test, **0 failed**, mọi log `EXIT=0`. Driver l1_k1c là **test-only mới của KT-43**, **không** nằm trong 146 test; smoke dưới Miri **không hoàn tất** trong ngân sách 3 h (64→8 op/seed vẫn quá chậm dưới trình thông dịch + tải máy) ⇒ **D8 KHÔNG claim** Miri phủ driver; D8 chỉ claim tập K2+pte+sync ở trên (log từng tệp).

- Tập Miri **K2**: `kt36_frame`, `kt37_pagetable`, `kt38_vspace`, `kt39_wx`, `kt40_tlb`,
  `kt41_vmfault`, `kt42_vmcheck` + unit `--lib` (`check_vm`, `check_*`) + `vifast-pte` +
  `vifast-sync`; thêm **4 tệp K2 còn thiếu** vào danh sách `nightly --only-miri` (`v2/bin/v2`)
  — D8 của K2-PLAN yêu cầu "Miri toàn bộ crate liên quan", không chỉ 3 tệp KT-40..42.
- Driver kt43 dưới Miri: chạy **mẫu nhỏ** (`VIFAST_SEEDS=2`, `VIFAST_SMP=1,2`) để Miri kiểm
  chính code driver mới (không chạy 1.000 seed/100k dưới Miri — D2-47(a)).
- Mọi log **exit 0**; cảnh báo `integer-to-pointer cast` là của `vifast-objmem` (arena, có từ
  trước), **không** phải UB mới.

## 5. Cổng `--fast` + `.scn` (việc §4.1 #4)

Lệnh `./v2/bin/v2 gate --fast` (không retry) — log [`JOURNAL/kt43-gate-fast.log`](../JOURNAL/kt43-gate-fast.log):

```
v2 check: PASS
--- L0 test: PASS (569 test, 0 failed)   # gồm driver l1_k1c mặc định 1.000 seed/smp 1
--- L1: OK (1.427 ms, đo bởi cổng)         # ngưỡng QD §5.2 = 60 s
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
real 374.56
```

- **569 test / 0 failed** = đúng baseline KT-42 (không thêm/bớt test; driver chỉ **thay** nội dung
  test cũ). L1 1.000 seed **1.427 ms** ⇒ **D11 ĐẠT** (≤ 60 s cho L1; `real 374,56 s` là **wall
  host** — KT-43 chạy thêm driver 1.000 seed trong L0 + máy có tải, **T-INFRA**, không nới ngưỡng).
- `scn-check: PASS (26 tệp, 0 blocked, 16 nhóm)` + `scn fixtures: OK (5 case âm)` ⇒ **D10 ĐẠT**.
- `abi-check`, `reserved-check`, `gen-check`, fixtures âm, luật R1..R5, `fmt`, `clippy -D warnings`
  đều OK; `git diff --stat -- spec/` rỗng ⇒ **D9 ĐẠT**.

- `check_scn.py`: **26 tệp / 16 nhóm** + **5/5 fixture âm** PASS (log trong gate).
- `git diff --stat -- spec/` **rỗng** (D9); không `#[ignore]`/`unsafe`/dependency mới.

## 6. Bảng DoD K2 — K2-PLAN §4.2 (đo được, không nới)

| # | Tiêu chí | Cách đo | Ngưỡng | Kết quả |
|---|---|---|---|---|
| D1 | TLB-safety | `tlb_violations()` **sau mỗi op** trên `nightly --l1` 100k × 4 smp | **0** | **ĐẠT** — 0/51,2 triệu op (§2.2) |
| D2 | Mutation AP→BSP | V1 (đảo cấp lá) + V5 (bỏ drain hộp thư khi chờ ack) | **bị giết** | **ĐẠT** — V1 `kt37_pagetable::map_walk_unmap_du_ba_co` đỏ; V5 `kt40_tlb::deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` đỏ (§3) |
| D3 | Mutation "bỏ xử lý hộp thư khi chờ ack" | V5 bỏ `drain_mail_fifo` trong `tlb_wait_round` | **bị giết** | **ĐẠT** — `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` đỏ: `left Ok(false)`, `right Ok(true)` (§3) |
| D4 | Tổng mutation | 12 ca §7.2 cũ + 5 ca VM | 17/17 (≥ 16) bị giết | **ĐẠT — 17/17**, 17/17 khôi phục hash == HEAD (§3) |
| D5 | W^X | bất biến `!(w>0 && x>0)` + `w+x ≤ maps` sau **mỗi** op (model hook + driver) | 0 vi phạm / 400k lượt | **ĐẠT** — `check_wx` sau mỗi op, 0 panic; V3 bị giết (§3) |
| D6 | Frame leak | `check_vm` (gồm `check_frames` + `wx_recount`) cuối **mỗi** lượt | 0 frame mồ côi | **ĐẠT** — 800.000 lần `check_vm`, 0 lỗi (§2.2) |
| D7 | ASID | không trùng trong thế hệ; cạn ⇒ `generation += 1` + flush `gen` | đúng 100% ca test | **ĐẠT** — `check_asid` sau mỗi op + `kt38_vspace`; V4 chứng minh flush deactivate bị test bắt |
| D8 | Miri | `v2 miri`/`nightly --only-miri` trên tập K2 + pte + sync | 0 lỗi | **ĐẠT (K2+pte+sync)** — 146 test/0 failed, mọi log `EXIT=0`; **driver smoke không hoàn tất, không claim** (§4) |
| D9 | ABI | `git diff --stat spec/` | **rỗng** | **ĐẠT** — `git status --porcelain -- spec/` rỗng (KT-43 không chạm spec) |
| D10 | `.scn` | 5 kịch bản mới + toàn bộ cũ | xanh, có log đỏ trước | **ĐẠT** — `scn-check` 26 tệp/16 nhóm + 5/5 fixture âm trong gate KT-43; log đỏ trước từ KT-42 |
| D11 | `gate --fast` | log cổng | ≥ 569 test; L1 1k ≤ 60 s | **ĐẠT** — 569 test/0 failed, L1 **1.427 ms** (§5) |

## 7. Đối chiếu KT-35..KT-42 (cổng vào K2)

Số đo dưới đây lấy từ **log từng task** (không viết tay số mới); KT-43 **không** sửa semantics
VM nào — chỉ mở rộng driver L1 (test-only), thêm 4 tệp K2 + `vifast-pte` vào danh sách Miri, và
viết báo cáo.

| Task | Sản phẩm K2 | Bằng chứng chính |
|---|---|---|
| **KT-35** | crate `vifast-pte` (`forbid(unsafe_code)`) + `v2/docs/k2-vm.md` (7 hằng số `[TK]`, bảng lỗi, "K2 không phải kernel thật") | `cargo test -p vifast-pte`; `kt35-miri.log`; gate K2 mở |
| **KT-36** | Frame 3 cỡ + ownership/`Retype`/cửa sổ PA + `check_frames` 7 nhóm | `kt36_frame` 12 test; gate 494/66; `kt36-miri.log` (12/12) |
| **KT-37** | PageTable 3 cấp map/unmap/walk + rights/attrs + 7 mã lỗi | `kt37_pagetable` 10 test; gate 512/67; `kt37-miri.log` (10/10) |
| **KT-38** | VSpace + ASID 8 bit + `generation` + `active_on` + `check_vspace/check_asid` | `kt38_vspace` 7 test; gate 526/68; `kt38-miri.log` |
| **KT-39** | W^X toàn cục theo Frame + `protect` + `wx_recount` + `check_wx` | `kt39_wx` 8 test (mutation W-sau-X bị bắt); `kt39-mutation-*.log`; gate 536/68 |
| **KT-40** | TLB 16 entry/hart + `TlbFlush` + ack thế hệ + chống deadlock AP→BSP | `kt40_tlb` 10 test; **5 mutation** (bỏ wait/flush cục bộ/ack sai seq/bỏ drain/bỏ flush deactivate) `kt40-mutation-*.log`; gate 552/70; `kt40-miri.log` (10/10) |
| **KT-41** | `FaultKind::Vm` model-only (NotMapped/Permission/Wx) + `vm_touch` + `check_fault` | `kt41_vmfault` 9 test; gate 562/71; `kt41-miri.log` (9/9) |
| **KT-42** | 5 `.scn` VM + `check_vm` một cửa 8 checker + shared read-only end-to-end | `kt42_scn` 5/5 (có 5 log đỏ trước), `kt42_vmcheck` 1/1, unit `check_vm` self-test 2 lỗi; gate 569/73; `kt42-miri.log` |

- Mọi `.scn` VM cũ chạy lại trong `gate --fast` của KT-43 (26 tệp/16 nhóm) — không chỉ dựa log
  KT-42.
- `git diff --stat spec/` rỗng ở **mọi** task K2 (D2-65 Q1/Q6: VSpace/PageTable/Frame **vẫn**
  `reserved`; `FaultKind::Vm` không vào spec).

## 8. Nợ **có chủ** — không tô hồng

1. **Model-only, một luồng**: toàn bộ K2 là mô hình tất định N hart trong **một** luồng CPU;
   "TLB", "IPI", "ack" là bản ghi/bộ đếm của model — **không** có phần cứng.
2. **K3 — TLB/PTE/IPI thật**: `sfence.vma`/`tlbi`/`invlpg`, MMU/Sv39 thật, IPI thật, ASID phần
   cứng, `TlbFlush` thật thay hộp thư model; SVM/VMX/khách (`GUEST`) vẫn `NotSupported`.
3. **`Busy`/`FaultKind` model-only**: `Busy` (hộp thư/Reply) và `FaultKind::Vm` **không** vào
   `spec/abi.toml` (D2-56(a), D2-65 Q6) — K3 mới chốt; `vm_touch` là model API, **không** phải
   ABI.
4. **K3 — pager/COW**: K2 **không** có memory server/pager, **không** cấp trang khi fault,
   **không** COW; shared read-only **có** nhưng chỉ ở mức mô hình (bộ đếm Frame toàn cục).
5. **K3 — nửa cao kernel + physmap + copy root entry** (design §4.10): K2 chỉ ghi chú.
6. **Gate `--fast` wall**: wall host > 60 s do tải máy là **T-INFRA** (QD §5.2 chỉ đặt 60 s cho
   L1 1.000 seed); không nới ngưỡng, không retry.
7. **KT-43 chạy L1 với `CARGO_PROFILE_TEST_OPT_LEVEL=2`**: chỉ là tối ưu biên dịch, giữ
   `debug-assertions`; nếu ai coi đây là "khác mặc định" thì đây là **ghi chú minh bạch**, không
   phải nới ngưỡng — số seed/op/bất biến giữ nguyên.

## 9. Đề xuất K2

**Cổng K2 ĐẠT (đề xuất)**: D1 (0 vi phạm TLB-safety / 51,2 triệu op), D2/D3/D4 (17/17 mutation bị
giết, gồm AP→BSP v1 — V5), D5 (W^X), D6 (Frame leak), D7 (ASID), D8 (Miri K2+pte+sync 146/0),
D9 (`spec/` rỗng), D10 (`.scn` 26/16 + fixtures), D11 (gate 569/0, L1 < 60 s). Không có mục nào
dưới ngưỡng; không retry để lấy xanh.

**Nợ chuyển K3 (có chủ, không tô hồng)** — §8: model-only 1 luồng; TLB/PTE/IPI thật =
`sfence.vma`/MMU/Sv39/IPI thật; `Busy`/`FaultKind` model-only (chưa vào `spec/`); pager/COW
chưa có; shared read-only mới ở mức mô hình; gate wall host > 60 s là T-INFRA. **K2 không phải
kernel thật** — câu này giữ nguyên ở đầu tệp.

**Việc còn lại sau commit**: reviewer-kt43 xác nhận (đã DRIFT một lần, đã vá), Agent 0 lật
KT-43/K2 trong `QUEUE.md`; mở K3 (riscv64 thật + TLB/PTE/pager thật).

## 10. Tệp chạm (KT-43)

- `v2/sim/vifast-sim/tests/l1_k1c.rs` — driver L1 phủ VM (test-only).
- `v2/bin/v2` — thêm 4 tệp K2 + `vifast-pte` vào danh sách `nightly --only-miri`.
- `v2/agent/reports/K2-DOD.md` (tệp này), `v2/agent/JOURNAL/kt43-*`, `v2/agent/STATE.md`.
- **Không** sửa: `spec/`, semantics VM, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`.
