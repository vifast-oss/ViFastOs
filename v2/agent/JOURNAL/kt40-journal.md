# KT-40 — TLB 16 entry/hart + shootdown `TlbFlush` + ack thế hệ + chống deadlock

Người làm: **Agent 2 dòng OS** (phiên `agent2-kt40`). Ngày: 2026-11-14.
Nguồn ngữ nghĩa: K2-PLAN §3 KT-40 + §2 **Q4/Q5**; **D2-65** (Q4/Q5, nới D2-59) + **D2-60**
(ack `fetch_max`); thiết kế §4.10 `:353-366` + §9.5 `:799`, dòng DoD 876; docs hiện hành
[`v2/docs/k2-vm.md` §14](../../docs/k2-vm.md).

## 0. Vào

- `agentctl next --project v2` trả **KT-40** (READY); KT-35..39 DONE; HEAD `4ae932e`
  ([KT-00] lật KT-39 DONE, 536 test).
- `git rev-list --count HEAD..origin/main` = **0**.
- Working tree có `viscript/` + `review/` của tab khác — **không chạm**; chỉ stage `v2/`.
- Baseline: [`kt40-baseline-gate-fast.log`](kt40-baseline-gate-fast.log) — **PASS 536 test**/68
  suite, L1 1.000 seed **800 ms**.

## 1. Test đỏ trước (D2-05)

[`kt40-red-compile.log`](kt40-red-compile.log): `cargo test -p vifast-sim --test kt40_tlb --no-run`
⇒ **E0432** (`TlbFlushScope`, `TLB_ENTRIES_PER_HART`) + **E0599** (`tlb_translate`, `tlb_lookup`,
`tlb_info`, `tlb_entries`, `tlb_flush_local`, `tlb_shootdown_begin/wait/poll`, `tlb_pending`,
`last_tlb_shootdown`, `check_tlb`, `tlb_violations`), 970 dòng, `could not compile` exit 101.
Không có đỏ ngữ nghĩa cũ vì KT-40 **không đổi** API cũ (VSpace/PageTable/Frame vẫn `reserved`;
`vspace_unmap/protect` giữ chữ ký).

## 2. Code

- `v2/sim/vifast-sim/src/tlb.rs` **(mới)**: `TLB_ENTRIES_PER_HART = 16`, `TlbKey (gen, asid, va)`,
  `TlbEntry` (covers/translation), `TlbFlush` (message), `TlbFlushScope{Page,Asid,Generation}`,
  `TlbTranslation`, `TlbHartInfo`, `TlbShootdownInfo`, `HartTlb` (LRU tất định theo `used`, hoà ⇒
  chỉ số nhỏ; hit/miss/fill/evict/flush/invalidated/delivered/acked; `ack` = `fetch_max` bằng
  `u64` thường, **không** atomic/unsafe), 5 unit test.
- `v2/sim/vifast-sim/src/mailbox.rs`: **đúng một** kind mới `MailboxMsg::TlbFlush(TlbFlush)`
  (D2-65 Q5 — nới D2-59; `VmKick`/`PowerReq` vẫn K3); doc module cập nhật.
- `v2/sim/vifast-sim/src/kobj.rs`: 9 field mới (`tlbs`, `tlb_pending`, `last_tlb_shootdown`,
  `tlb_shootdown_seq/count`, `tlb_msg_total`, `tlb_ack_total`, `tlb_fill_total`,
  `tlb_wait_timeouts`), `PendingShootdown` (+`implicit`), 16 API TLB (translate/lookup/info/
  flush_local/begin/wait/**poll**/pending/last/count/check_tlb/tlb_violations…), `push_mail`
  (gán `ack = delivered` — Q5 "seq = số send Ok"), `apply_mail(TlbFlush)` (flush + ack), hook
  `vspace_unmap`/`vspace_protect` (prepare → sửa PTE → post → wait; `Ok` chỉ khi
  `acked_mask == active_on`), flush cục bộ khi `vspace_activate` đổi VSpace / `vspace_deactivate`
  / `set_hart_online(false)` / teardown VSpace (Asid, mọi hart) / cạn ASID (Generation, mọi hart);
  `check_tlb` trong `sched_after_op` (cửa sổ nhanh `tlb_fill_total==0 && tlb_msg_total==0`);
  `check_mailboxes` thêm đẳng thức `delivered - acked == số TlbFlush trong hàng` + `acked ≤
  delivered`; 1 unit test dương tính oracle.
- `v2/sim/vifast-sim/tests/kt40_tlb.rs` **(mới, 10 test)**.
- `v2/docs/k2-vm.md` **§14** (TLB, 3 scope flush, shootdown/ack, D2-68 self-target, chống
  deadlock + poll, oracle, 10 test, 5 mutation, ranh giới).
- `v2/bin/v2`: thêm `kt40_tlb` vào danh sách `nightly --only-miri`.

## 3. Cổng

- [`kt40-check.log`](kt40-check.log): `v2 check` **PASS** (fmt, clippy `-D warnings`, R1..R5,
  gen-check).
- [`kt40-gate-fast.log`](kt40-gate-fast.log): **PASS 552 test / 70 suite**, L1 1.000 seed
  **793 ms**; `kt40_tlb` 10/10; lib **62/62** (56 cũ + 5 unit `tlb` + 1 unit `check_tlb`);
  L0 552/0. Baseline 536 ⇒ +16 test (10 integration + 5 unit + 1 unit).
- [`kt40-miri.log`](kt40-miri.log): `--test kt40_tlb` **10/10** (324,35 s, `MIRI1_EXIT=0`) +
  `--lib tlb::tests` **5/5** (0,73 s, `MIRI2_EXIT=0`) + `--lib 'kobj::tests::check_tlb'` **1/1**
  (1,98 s, `MIRI3_EXIT=0`). Property tự giảm còn 2 hart × 150 vòng; ca cạn 256 ASID tự bỏ bằng
  `cfg(miri)` (không `#[ignore]`).

## 4. Mutation (5 ca, mỗi ca một chỗ, log thật)

Baseline hash `kobj.rs` trước/sau mọi ca:
`e0bfbe6914ab0ff3a8dfc9852df4d547e4b89c2cd6b0500869409dc78470c7fe` (khớp từng ca; khôi phục bằng
`cp` bản sao, **không** `git checkout` vì tệp chưa commit).

| Mã | Đột biến | Ca đỏ (log) | Hash |
|---|---|---|---|
| M1 | bỏ `tlb_shootdown_wait` trong `vspace_unmap` ⇒ `Ok` khi thiếu ack | `shootdown_1_2_4_8_hart_ack_du`, `hart_goi_ngoai_active_on_van_shootdown_dung`, `deadlock_ack_tu_hart_dang_cho`, `property_10k_tlb_shootdown` — [`kt40-mutation-bo-wait.log`](kt40-mutation-bo-wait.log) | khớp |
| M2 | bỏ **flush cục bộ** trong `tlb_shootdown_prepare` | `protect_cap_nhat_quyen_tren_moi_hart`, `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu`, `shootdown_1_2_4_8_hart_ack_du`, `property_10k_tlb_shootdown` — [`kt40-mutation-bo-flush-cuc-bo.log`](kt40-mutation-bo-flush-cuc-bo.log) (bắt ở `check_tlb`) | khớp |
| M3 | ack **sai seq** (`delivered + 1` trong `tlb_shootdown_post`) | `ack_thieu_ok_chua_tra`, `shootdown_…`, `protect_…`, `deadlock_*`, `property_10k…` — [`kt40-mutation-ack-sai-seq.log`](kt40-mutation-ack-sai-seq.log) | khớp |
| M4 | bỏ **rút hộp thư** trong `tlb_wait_round` (AP→BSP) | `deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu` (không self-target) — [`kt40-mutation-bo-rut-hop-thu.log`](kt40-mutation-bo-rut-hop-thu.log) | khớp |
| M5 | bỏ flush khi `vspace_deactivate` | `flush_theo_trang_asid_va_gen`, `property_10k_tlb_shootdown` — [`kt40-mutation-bo-flush-deactivate.log`](kt40-mutation-bo-flush-deactivate.log) | khớp |

Sau mỗi khôi phục: `kt40_tlb` **10/10** xanh (ghi ở mục 5 từng log).

**Ghi chú hash:** `b8c7274…` hiện tại của `kobj.rs` khác `e0bfbe…` **tại thời điểm chạy
mutation** vì sau CONFIRM của reviewer-kt40b có sửa **docstring** `vspace_activate`/
`vspace_deactivate` (3 nit — không đụng biểu thức nào của TLB/shootdown/ack). Bản sao khôi phục
vẫn đúng: hash trước/sau **từng** ca mutation trùng nhau (mục 5 log), và `kt40_tlb` 10/10 xanh
sau khôi phục; `gate --fast` cuối PASS 552/70.

## 5. DRIFT review độc lập (reviewer-kt40) và cách xử lý

Reviewer verdict: **DRIFT (không UNSAFE)** — lõi `Ok ⇔ đủ ack` đúng nhưng chưa đủ bằng chứng.
Đã xử lý đủ 5 việc; `reviewer-kt40b` **CONFIRM 5/5** (tự chạy 10 + 5 + 1 + 62, gate 552/70;
M2 đỏ thật; M4 đỏ đúng 1 ca deadlock; oracle `active_on` bắt M5; docs khớp). Sau CONFIRM, sửa
3 nit: docstring `vspace_activate`/`vspace_deactivate` hết mâu thuẫn hành vi flush cục bộ mới;
hàng M3 §14.6 thêm `hart_goi_ngoai_active_on_van_shootdown_dung`; tiền tố `v2/agent/JOURNAL/`
cho mọi dẫn chứng log. D2-68 được Agent 0 đính chính ở `258eeeeb` — **khớp code** (hart gọi
flush cục bộ + ack ngầm, không self-target), không phải sửa logic. Đã xử lý 5 việc:

1. **4 mutation** (M1..M4) chạy đủ + log thật (mục 4); dẫn chứng log trong `k2-vm.md` khớp tên
   tệp thật (đã tạo đủ 5 log; không còn dẫn chứng không tồn tại).
2. **Test hart gọi ngoài `active_on`** (`hart_goi_ngoai_active_on_van_shootdown_dung`) + **test
   AP→BSP tương hỗ không self-target** (`deadlock_hai_hart_cho_nhau_poll_tu_phuc_vu`, qua model
   API mới `tlb_shootdown_poll` — mỗi hart tự phục vụ hộp thư của mình, không chạy hộ).
3. **Oracle** thêm bất biến "entry phải nằm trên hart thuộc `active_on` của VSpace" (nhóm 3 §14.5)
   + mutation **M5** "bỏ flush khi `deactivate`" bị bắt.
4. **Miri** `--test kt40_tlb` 10/10 xong (324 s) + log hoàn tất; thêm `kt40_tlb` vào danh sách
   `v2 miri` / `nightly --only-miri` (`v2/bin/v2`).
5. **Docs**: ghi rõ deviation **D2-68** — message `TlbFlush` **không** gửi cho hart gọi;
   `acked_mask = {hart gọi nếu ∈ active_on} ∪ {hart đích đã ack}` ⇒ bất biến Q5
   `ack_mask == active_on` vẫn giữ, và local flush trở thành **đường tải thật** (M2 bắt được).
   Docstring `process_mail` có nhánh `TlbFlush`; §14.6 khớp đúng 5 mutation.

## 6. Ghi chú / ranh giới

- `active_on != ∅` nhưng hart gọi **không** thuộc `active_on`: local flush hart gọi (không có
  entry nhờ flush khi rời VSpace) + gửi đúng các hart trong mask; test riêng.
- Teardown VSpace và cạn ASID flush **broadcast trực tiếp** mọi hart (không qua hộp thư/ack):
  xảy ra trong `Revoke`/tạo VSpace, không được chờ message; teardown chỉ khi root tinh khiết.
  `active_on` (không phải "ever-active") là đủ vì hart rời VSpace luôn flush cục bộ ở
  `deactivate`/đổi VSpace/offline.
- **Không** `unsafe`, dependency, `#[ignore]`, `#[should_panic]`; không sửa test cũ; không chạm
  `spec/` (`git diff --stat spec/` rỗng), `viscript/`, QUEUE/DECISIONS/AGENTS; không lật QUEUE.
- **Đề xuất: REVIEW** (chờ reviewer xác nhận verdict; không tự DONE). Follow-up: KT-41 dùng
  `vspace_walk`/TLB oracle cho `FaultKind::Vm`; KT-42 gộp `check_tlb` vào `check_vm` + `.scn`
  shootdown; KT-43 chạy `nightly --l1` 100k×4 + 4 mutation còn lại của K2-PLAN + soak.
