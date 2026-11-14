# KT-42 — 5 `.scn` VM + một cửa `check_vm` + shared read-only end-to-end

Người làm: **Agent 2 dòng OS** (phiên `agent2-kt42`). Ngày: 2026-11-14.
Nguồn ngữ nghĩa: `review/agent0/K2-PLAN.md` §3 **KT-42** + §2 **Q7/Q10**; **D2-65** (Q7 shared
read-only **có**/COW **không**; Q10 cổng `.scn` + bất biến), **D2-67** (W^X toàn cục),
**D2-68** (`TlbFlush`/ack); docs hiện hành [`v2/docs/k2-vm.md` §16](../../docs/k2-vm.md).

## 0. Vào

- `agentctl next --project v2` trả **KT-42** (READY); KT-35..41 DONE; HEAD vào task `6a9e3ccf`
  ([KT-00] lật KT-41 DONE, 562 test — mở KT-42); trước commit lead chèn **D2-69** ở nhánh
  chính (`dae06315` [KT-00]) — `.scn` 22..26 là số hiệu được chốt, thay G5 "≥30".
- `git rev-list --count HEAD..origin/main` = **0**.
- Working tree có `viscript/` + `review/` của tab khác — **không chạm**; chỉ stage `v2/`.
- Baseline: [`kt42-baseline-gate-fast.log`](kt42-baseline-gate-fast.log) — **PASS 562 test**/71
  suite, L1 1.000 seed **885 ms**.

## 1. Test đỏ trước (D2-05)

- [`kt42-red-scn-check.log`](kt42-red-scn-check.log): `python3.12 v2/test/scn/check_scn.py`
  **chưa cập nhật** ⇒ đỏ: "phải có đúng 21 tệp .scn, đang có 26" + tag `vm-map` lạ + mọi op
  `vm_*` bị coi là "syscall lạ" (exit 1).
- [`kt42-red-scn-runner.log`](kt42-red-scn-runner.log): runner VM **chưa cài op** ⇒ **5/5 ca
  đỏ**; log riêng từng tệp
  [`kt42-red-scn-22.log`](kt42-red-scn-22.log) … [`kt42-red-scn-26.log`](kt42-red-scn-26.log):
  panic đúng op đầu (`vm_map`, và `vm_activate` ở kịch bản 24). Đây là "chưa cài" — `.scn` là
  dữ liệu, runner là bản cài; sau khi cài 5/5 xanh.

## 2. Code

- `v2/test/scn/22-frame-map-unmap-walk.scn` … `26-shared-readonly-frame.scn` (**5 tệp mới**,
  tag `vm-map/vm-wx/vm-tlb/vm-fault/vm-shared`, số 22..26 = số trống tiếp theo; không sửa 21
  tệp cũ).
- `v2/test/scn/check_scn.py`: kỳ vọng **21 → 26 tệp**, thêm **5 nhóm** (tổng 16) + bảng đóng
  `VM_OPS` cho op model-only (kiểm tên/tham số; op ABI vẫn theo `spec/method_signature`; **không**
  nới kiểm cũ).
- `v2/sim/vifast-sim/tests/kt42_scn.rs` (**mới, 5 test**): runner đọc `.scn` (không sửa), rig
  chuẩn (harts/caps; 3 cỡ Frame; 2 VSpace; `t0` fault_ep/`t1` procsrv), chạy **21** op `vm_*`,
  so từng bước với kỳ vọng.
- `v2/sim/vifast-sim/src/kobj.rs`: **`Machine::check_vm`** — gộp `check_frames` + `check_pages`
  + `check_wx` + `wx_recount` + `check_vspace` + `check_asid` + `check_tlb` + `check_fault`
  (tiền tố lỗi theo checker); **không** thay hook per-op. Unit test dương tính
  `check_vm_bat_pte_bi_xoa_ngam_va_entry_tlb_mo_coi`: tiêm 2 lỗi (xoá PTE ngầm; entry TLB mồ
  côi) — cả hai bị bắt, khôi phục thì sạch.
- `v2/sim/vifast-sim/tests/kt42_vmcheck.rs` (**mới, 1 test**): đường vàng đầy đủ — Frame 4K/2M/1G,
  shared read-only 2 VSpace, TLB 2 hart, shootdown; `check_vm` sạch; `wx_recount` khớp; unmap
  shared một bên còn đúng bộ đếm.
- `v2/bin/v2`: thêm `kt42_vmcheck` vào `nightly --only-miri` (không thêm `kt42_scn` — đọc tệp).
- `v2/docs/k2-vm.md` **§16**: 5 kịch bản/nhóm, quy ước runner, `check_vm` + chi phí, self-test,
  shared read-only, log đỏ, cổng/Miri, ranh giới.

## 3. Cổng

- [`kt42-gate-fast.log`](kt42-gate-fast.log): **PASS 569 test / 73 suite**, L1 1.000 seed
  **2.635 ms**, fmt/clippy `-D warnings`, luật R1..R5, gen-check ON, scn-check **26 tệp/16
  nhóm**, fixtures âm PASS. Baseline [`kt42-baseline-gate-fast.log`](kt42-baseline-gate-fast.log)
  **PASS 562/71**, L1 885 ms. Chênh **+7 test** = 5 `.scn` runner + 1 `kt42_vmcheck` + 1 unit
  `check_vm`; +2 suite (2 test binary mới).
- Một lần gate trước đó L1 **78.462 ms** do tab khác chạy `vis-vm --test diff_100k` (host load
  ~8; **T-INFRA**): gate vẫn PASS; lần chạy sạch sau đó L1 **2.635 ms** — **không** nới seed
  hay ngưỡng (D2-07; QD §5.2).
- `git diff spec/` **rỗng**; không `#[ignore]`/unsafe/dependency mới; 21 tệp `.scn` cũ không
  đổi (chỉ `check_scn.py` nâng kỳ vọng 21→26 + bảng `VM_OPS`).

## 4. Miri

[`kt42-miri.log`](kt42-miri.log): `kt42_vmcheck` **1/1** (32,12 s) `MIRI1_EXIT=0`; unit test
`check_vm` trong `--lib check_vm_` **1/1** (3,37 s) `MIRI2_EXIT=0`. Không chạy `kt42_scn` dưới
Miri (đọc tệp — cùng quy ước kt24/kt25/kt26).

## 5. Review độc lập

`reviewer-kt42` **APPROVE**: tự chạy `kt42_scn` **5/5**, `kt42_vmcheck` **1/1**, `gate --fast`
**569/73** (L1 1.499 ms), `check_scn.py` **26 tệp/16 nhóm**, **5/5 fixture âm**, `git diff spec/`
rỗng; xác nhận `check_vm` gọi đủ **8** checker, hook per-op nguyên vẹn (**+117/−0**), self-test
tiêm **2 lỗi thật**; 5 `.scn` lần lượt 23/27/22/17/18 bước, runner **21 nhánh op** thật (không
hard-code kết quả). Reviewer ghi chú P3: (1) journal ghi "22 op" → **21**; (2) ghi verdict vào
journal/STATE; (3) ghi rõ **lệch chữ** với K2-PLAN — per-op giữ nguyên bộ checker cũ (**không**
có `wx_recount`), `check_vm` đầy đủ chỉ ở L0/`.scn`/cổng — **tương đương phủ, không nới**;
(4) số `.scn` 22..26 thay `G5 "≥30"` — Agent 0 chốt bằng **D2-69**. Đã sửa (1)(2)(3) trước
commit; (4) do Agent 0 ghi.

## 6. Tệp chạm

- `v2/test/scn/22-frame-map-unmap-walk.scn`, `23-wx-violation-counters.scn`,
  `24-shootdown-2-hart.scn`, `25-vm-fault-notmapped-resume.scn`,
  `26-shared-readonly-frame.scn` (mới)
- `v2/test/scn/check_scn.py`
- `v2/sim/vifast-sim/tests/kt42_scn.rs`, `kt42_vmcheck.rs` (mới)
- `v2/sim/vifast-sim/src/kobj.rs` (`check_vm` + unit test dương tính)
- `v2/bin/v2` (Miri list), `v2/docs/k2-vm.md` §16
- `v2/agent/JOURNAL/kt42-*.log`, `kt42-journal.md`; `v2/agent/STATE.md`
