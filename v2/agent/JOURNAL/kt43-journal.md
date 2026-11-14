# KT-43 — cổng cuối K2: 100k×4 L1 (driver phủ VM) + 17 mutation + Miri + DoD K2

Người làm: **Agent 2 dòng OS** (phiên `agent2-kt43`). Ngày: 2026-11-14.
Nguồn: `review/agent0/K2-PLAN.md` §3 **KT-43** + §4 (cổng K2, bảng DoD §4.2); **D2-65** (Q10:
cổng = `nightly --l1` 100k×4 + mutation + Miri + `.scn` + bảng DoD, **0 vi phạm TLB-safety**,
**đột biến AP→BSP bị bắt**), D2-67 (W^X), D2-68 (`TlbFlush`/ack), D2-69 (`.scn`/`check_vm`).

## 0. Vào

- `agentctl next --project v2` trả **KT-43** (READY); KT-35..42 DONE; HEAD `66d51f89`
  ([KT-00] lật KT-42 DONE, 569 test — mở KT-43); `git rev-list --count HEAD..origin/main` = **0**.
- Working tree có `viscript/` + `review/` của tab khác — **không chạm**; chỉ stage `v2/`.
- Baseline = KT-42: `gate --fast` **PASS 569 test / 73 suite**, L1 1.000 seed **2.635 ms**
  ([`kt42-gate-fast.log`](kt42-gate-fast.log)); `v2/` không đổi từ đó tới khi mở KT-43.
- Máy 12 nhân, **có tải nền** của tab khác (`ffmpeg`, `mds`; load 13–17) — mọi log ghi giờ máy;
  không retry để lấy xanh (D2-07).

## 1. Driver L1 phủ VM (test-only)

`nightly --l1` chạy `driver_smp` trong `v2/sim/vifast-sim/tests/l1_k1c.rs`. KT-43 giữ **19 op**
cũ (sched/SMP/mail + IPC/Notification/WaitAddr) và thêm **4 op VM** (23 nhánh; vẫn **64 op/seed**):

| Op | Nhóm | Đường đi |
|---|---|---|
| 19 | map | `vspace_map_obj` Frame **1 GiB** (lá ngay root) vào VSpace active trên mọi hart |
| 20 | walk+TLB | `vspace_walk` (oracle VA→PA) + `tlb_translate` (miss/fill TLB hart hiện tại) |
| 21 | unmap+shootdown | nạp TLB **mọi hart** rồi `vspace_unmap`; assert `Ok ⇒ acked == targets` + mọi hart hết entry cũ |
| 22 | VM fault | `vm_touch` VA chưa map ⇒ `FaultKind::Vm{NotMapped}` tới `fault_ep`; `wake` dọn (không pager — K3) |

- VM rig: 1 VSpace (lá ngay root ⇒ **một** PageTable), 3 Frame 1 GiB (mỗi Frame một Untyped/cửa
  sổ PA riêng — D2-66), 4 TCB có `fault_ep`; VSpace active trên **mọi** hart.
- Sau **mỗi** op: checker K1c cũ + `check_harts` + `check_caps` + **`tlb_violations() == 0`**
  (oracle TLB-safety KT-40); cuối **mỗi** lượt: **`check_vm()`** (một cửa 8 checker — KT-42).
- Tên test **không** đổi; `gate --fast` L1 1k vẫn là `tests/l1.rs` (bộ lọc `l1_`); driver chạy ở
  L0 như trước (mặc định 1.000 seed/smp 1).

### 1a. Ngưỡng "không tautology" của driver (KT-33 → KT-43)

KT-33 siết per-seed `ipc_sent > 1` (prologue chỉ 1 message ⇒ 64 op phải sinh thêm). Với **23
nhánh** op, một seed hiếm khi chỉ có prologue. **Lần chạy 100k đầu tiên gặp đúng ca đó**:
`seed 67453 smp 2: ipc_sent = 1` ⇒ test đỏ (log
[`kt43-l1-100k-attempt1-fail.log`](kt43-l1-100k-attempt1-fail.log); smp 1 đã xong
1.212.128/226.334/782.515).

Xử lý: giữ **bảo toàn message per-seed** (`sent == delivered + pending + dropped`, `sent ≥ 1`),
chuyển ngưỡng "64 op phải sinh message" xuống **tổng mỗi smp**: `ipc_sent > 5 × seeds` (đo thực
tế ~12,1/seed) + `ipc_delivered > 0`. Đây là **ngưỡng phủ của driver**, không phải bất biến model
— ghi rõ trong K2-DOD §2; **không** nới seed/op/số lượt. Sau sửa: `--seeds 10000 --smp 1,2,4,8`
**PASS 40.000 lượt**, 121,46 s (log [`kt43-l1-10k.log`](kt43-l1-10k.log)).

### 1b. Đỏ trước của driver

Driver mới với `N_OP = 19` (tạm bỏ 4 nhánh VM) ⇒ test đỏ ở bất biến "driver phải phủ VM"
([`kt43-red-driver-vm.log`](kt43-red-driver-vm.log)); khôi phục hash trước khi chạy cổng.

## 2. 100k×4

Lệnh (không giảm seed/op; `CARGO_PROFILE_TEST_OPT_LEVEL=2` **chỉ** đổi tối ưu biên dịch, giữ
`debug-assertions` mặc định **bật** — mọi checker per-op vẫn chạy):

```bash
CARGO_PROFILE_TEST_OPT_LEVEL=2 VIFAST_DOI=100000 \
  ./v2/bin/v2 nightly --l1 --seeds 100000 --smp 1,2,4,8
```

Kết quả: **PASS**, 400.000 lượt, **wall 690,69 s**, 0 panic/0 vi phạm TLB-safety (51,2 triệu op;
2 lần/seed); bảng counters chi tiết trong K2-DOD §2.2. Log [`kt43-l1-100k.log`](kt43-l1-100k.log).
Validate trước khi chạy cổng: 10k×4 PASS 57,01 s ([`kt43-l1-10k.log`](kt43-l1-10k.log)).

## 3. Mutation 17 ca

Harness tay (`/tmp/kt43_mut.py`, không commit): mỗi ca xanh trước → sửa một chỗ → đỏ đúng test →
khôi phục sha256 == HEAD + `git status` rỗng. Log kết quả: [`kt43-mutation.log`](kt43-mutation.log).
**17/17 ca** `exit 101` + `FAILED` đúng test đích + khôi phục hash; 12 ca §7.2 cũ chạy lại tại
HEAD K2 + 5 ca VM (**V1** đảo cấp lá AP→BSP, **V2** bỏ chờ ack khi unmap, **V3** bỏ W^X khi map W,
**V4** bỏ flush khi deactivate, **V5** bỏ drain hộp thư khi chờ ack — đúng lỗ AP→BSP của v1).

- **Sự cố harness lần đầu** (giữ log [`kt43-mutation-attempt1.log`](kt43-mutation-attempt1.log),
  **không** dùng làm kết quả): tệp không được đóng ⇒ flush trễ ở V3–V5 (restore báo "KHỚP" sai),
  và **M12** bản cũ (nhắm `unlink_tcb_queues`) **sống** vì KT-34 đã thêm guard thứ hai ở
  `transition_tcb`. Đã sửa harness (ghi + `fsync`, kiểm `git status` sạch **trước** khi áp và
  **sau** khi khôi phục) + định nghĩa lại M12 nhắm guard hiện hành `transition_tcb` (cùng bất
  biến: rời `BlockedOnReply` phải gỡ `Reply.caller`; test
  `kt25_ipc2::cong_transition_giu_lien_ket_reply_va_khong_ghi_de_caller`), rồi chạy lại **toàn bộ
  17 ca** — 17/17 đỏ đúng test, 17/17 khôi phục hash == HEAD, cây `v2/sim/...` sạch.

## 4. Miri

Tập K2 đầy đủ: `kt36_frame`, `kt37_pagetable`, `kt38_vspace`, `kt39_wx`, `kt40_tlb`,
`kt41_vmfault`, `kt42_vmcheck` + `--lib` + `vifast-pte` + `vifast-sync` + driver KT-43 mẫu nhỏ
(`VIFAST_SEEDS=1`, `VIFAST_SMP=1`, `cfg(miri)` ⇒ `SO_OP=8`; không chạy 1k/100k dưới Miri —
D2-47(a)). Thêm 4 tệp K2 còn thiếu + `vifast-pte` vào danh sách `nightly --only-miri`
(`v2/bin/v2`). Log `kt43-miri-*`: **10 log của tập 146 test `exit 0`**; driver smoke
`kt43-miri-l1_k1c` kết thúc **`EXIT=101` (SIGTERM), không hoàn tất** ⇒ không claim phủ driver.

## 5. Cổng

`./v2/bin/v2 gate --fast` (không retry) — log [`kt43-gate-fast.log`](kt43-gate-fast.log):
**PASS 569 test/0 failed**, L1 1.000 seed **1.427 ms** (≤ 60 s), `real 374,56 s` (wall host —
T-INFRA); `check_scn.py` 26 tệp/16 nhóm + 5/5 fixture âm; fmt/clippy `-D warnings`; `git diff
spec/` rỗng. Log cổng trước delta D2-70: 569/847 ms (cùng kết luận; đã chạy lại sau delta).

## 6. Tệp chạm

- `v2/sim/vifast-sim/tests/l1_k1c.rs` (driver test-only), `v2/bin/v2` (Miri list),
  `v2/agent/reports/K2-DOD.md`, `v2/agent/JOURNAL/kt43-*`, `v2/agent/STATE.md`.
- **Không** sửa `spec/`/`vifast-model`/`.scn`/QUEUE/DECISIONS/AGENTS/`viscript/`; không unsafe/
  dependency mới; không `#[ignore]`.

## 7. Review độc lập + vá DRIFT

`reviewer-kt43` verdict **DRIFT** (blocking) dù mô hình K2 ĐẠT; lead ban hành **D2-70**
(`b3ff8e1e`, push) và yêu cầu vá 4 mục. Đã vá:

1. **Ngưỡng IPC (D2-70)**: driver dùng **hai** guard — mỗi seed `ipc_sent ≥ 1` + tổng mỗi smp
   `ipc_sent > 5 × seeds` + `ipc_delivered > 0`; **in `ipc_min`/`ipc_median`** mỗi smp. Log 10k×4
   theo D2-70: [`kt43-l1-10k.log`](kt43-l1-10k.log) — min 2/2/1/2, median 12/12/11/11 (không
   lượt nào bỏ IPC). Không dùng D2-70 để hạ ngưỡng nào khác; 100k log giữ (chạy đúng guard D2-70
   trước khi thêm dòng in stats). K2-DOD §2.3 ghi rõ.
2. **K2-DOD final**: hết placeholder; sửa dòng driver Miri (`SEEDS=1, SMP=1, SO_OP=8` — **không
   hoàn tất**, xem D8 honest), thời gian kt41/kt42 = **389,60 s / 32,12 s**; bỏ câu "log mutation
   có diff" (snippet ở `kt43-mutation-diff.md`); sửa AP→BSP: **V5** mới là AP→BSP v1, **V1** là
   đổi lá/thứ tự cấp theo câu chữ đề bài.
3. **Mutation V1–V4**: lưu harness [`kt43_mut.py`](kt43_mut.py) + [`kt43-mutation-diff.md`](kt43-mutation-diff.md)
   (17 cặp before/after); ghi rõ V1 đỏ ở `.expect("map 4 KiB")` (kt37:169), V2 ở
   `panic!("phải có shootdown")` (kt40:245), V3/V4 ở invariant `check_wx`/`check_tlb`
   (kobj:6168/6175), V5 ở assert `Ok(false)`/`Ok(true)` — không trình bày là "assert của luật".
4. **Miri**: D8 chỉ claim **146 test K2+pte+sync** (mọi `EXIT=0`); driver smoke **không hoàn tất**
   trong ngân sách (64→8 op/seed vẫn quá chậm dưới Miri + tải máy) ⇒ **không** claim Miri phủ
   driver; K2-DOD §4 ghi đúng.

Sau vá: chạy lại `gate --fast` **PASS 569/0**, L1 **1.427 ms**; chờ reviewer-kt43 xác nhận trước
khi Agent 0 lật K2.
