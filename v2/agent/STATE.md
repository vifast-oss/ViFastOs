# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-26 13:00 JST (KT-417 DONE @ c57895df; T2 PASS 81/81; mở W7a slot 2–3) ===
HEAD          : `c57895df` — **`KT-417` DONE** (cửa sổ `spec/` ABI sạch + `[boot_caps]` + 4 invocation core),
                3 commit `35282ab9` → `658221dc` → `c57895df`; **T2 PASS 81/81** (688,3 s) + `ktest` 9/9.
                Phiên trước (Thấp điểm chính): 26 task DONE, lô 1..8 `KT-400..KT-404d`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`/`D2-260`/`D2-261`,
                42 task ≤ 3 h trong dải `KT-400..KT-424`, 4 Chặng — 100 % đại tu kiến trúc, KHÔNG soak phát hành).
CỔNG          : **T2 = PASS 81/81 @ `c57895df`** (688,3 s) — gồm `ktest` 9/9 (TAP 23 capalloc âm theo luật
                `Mutate`/`Rotate` mới) + `pre-measure`/`api-boundary` xanh trên cây sạch (hai bước đó
                **fail-closed trên cây bẩn** — lý do phải commit trước khi chạy T2).
                Phiên trước: 8 đợt T2 79/79..81/81 @ `f3cf3c97`..`bdf5c61`; `L1-medium` + Miri @ `9eb0e589`.
                `bench-bkl` (`KT-415`): tranh chấp BKL 1/2/4 hart = 0 % / 39,6 % / 93,5 % (ngưỡng S3 đã kích hoạt).
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (`KT-417` → `KT-417b` gỡ `plat-common::obj`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + `PerHartState` (**DONE**: `KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 17 `EP_*` (**`KT-417` DONE** sinh `BootCapLayout` → `KT-420` bỏ 17 `EP_*`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`/`rngsrv`) ra User Space (`KT-419/419b/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (**DONE**: `KT-411/411b/412/413/413b/415`).
TASK          : **384 task** — **DONE (372 task)** / **2 READY** / **10 BLOCKED** (Đại tu: **30/42 DONE**, 0 chờ duyệt).
                `KT-417` **DONE** — dòng `QUEUE.md` còn `READY` (Agent 0 lật khi sync; `agentctl done` đòi
                `--write` = quyền Agent 0, AGENTS §3).
                **`READY` (2 slot song song W7a, 0 % chồng tệp theo `D2-261`)**:
                • `KT-413c` (P1 — **APPROVED `D2-261`** 24 mục `# sched:`: gỡ trọn 2.162 LOC `sched.rs` `x86_64`, +64 KiB `.bss`);
                • `KT-408c` (P1 — vá 7 `E_WRONG-REASON` `v2 mutants --all` + cấp tick `Machine::now` `riscv64` + 5 `SLOT_EP_*` `netsrv` + `vifast-bench --bkl`).
                **`BLOCKED` thuần theo DAG (10 task)**: `KT-417b` (1 cây Cap/MDB, mở khoá Chặng 3), `KT-417c` (`x86_64` `syscall`/`sysret`),
                `KT-418c` (vá `-cpu max` `VSpace` isolation + `hardening-max.sh`, KHÔNG nới ratchet `B1.1`),
                `KT-419` (`Irq*` + `MapIo`), `KT-419b` (`IOSpace` qua `virtio-iommu-pci` 3 arch), `KT-420..KT-424`.
ĐANG CHẠY    : **W7a slot 2–3** tại Khe Thấp điểm trưa `04:00–05:50 UTC`: **`KT-413c`** (gỡ `sched.rs` `x86_64`
                → `Machine::dispatch(hart)`, viết lại 24 mục `# sched:` + 64 dòng soak) ∥ **`KT-408c`**
                (`mutants.py` git-tree + `Machine::now` riscv64 + `check-boot-abi` `SLOT_EP_*`; `--bkl` Lead nối lúc merge).
                Worktree `wt.sh` slot 1/2; sub-agent chỉ T0, Lead commit/T1/T2 (hai bước `pre-measure`/`api-boundary`
                fail-closed trên cây bẩn).
                **`KT-417` DONE** — nhật ký `JOURNAL/kt417-spec-bootcaps.md`: 3 lượt T2, 2 lượt đỏ do **lỗi thật**
                (`InvocationStats` ktest + TAP 23 capalloc), 1 lượt đỏ do `T-INFRA` (QEMU mồ côi 12h44 đã dọn).
                **Reviewer độc lập** (04:15 UTC) trên `c57895df`: **CÓ** đủ nền cho `KT-417b`; phát hiện **F1**
                (`bound_notification` treo khi Notification chết, chặn `KT-419`) — **đã vá** + test đỏ-trước,
                `vifast-core` **134 test** xanh. Nợ máy kiểm F2/F3/F5/F6/F7 + tách `KT-417b` (6,5–7,5 h) và
                quyết định pin `.corespace`/`MAX_MAPPINGS` ⇒ **Q1..Q6 trong `BLOCKED.md` §1b**.
                **Planner `KT-417b`** (04:15 UTC): đề xuất `KT-417b-1` (≤ 2 h, dời obj, **0** đổi `size_of`) +
                `KT-417b-2` (≤ 3 h, nhập bảng + `PageTable.{Map,Unmap}`) — chờ Agent 0 chốt.
                **Ghi chú kiến trúc (`D2-261`)**: ratchet `B1.1` giữ cứng (`driver_files ≤ 23`, `virtio-rng` đi `rngsrv` User Space ở `KT-423`);
                ngưỡng BKL S3 đã kích hoạt (93,5 % ở 4 hart) ⇒ Khoá `PerHartState` + `HartTier2` là hạng mục #1 hậu `KT-424`.
MÁY (sim)     : `size_of::<Machine>()` **184.712** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
