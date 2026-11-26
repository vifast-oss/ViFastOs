# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-26 11:30 JST (D2-261: Agent 0 APPROVED gói KT-416/416b; mở W7a) ===
HEAD          : `0b28847` (cây code sạch sau lô 8; T2 PASS 81/81). 26 task DONE trong phiên Thấp điểm chính:
                lô 1 `KT-400/402/403/406/402b/404/408/414b`; lô 2 `KT-409/409b/410/411`; lô 3 `KT-412/413`;
                lô 4 `KT-415/416/418`; lô 5 `KT-411b`; lô 6 `KT-413b/418b`; lô 7 `KT-408b/404c`; lô 8 `KT-416b/404d`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`/`D2-260`/`D2-261`,
                42 task ≤ 3 h trong dải `KT-400..KT-424`, 4 Chặng — 100 % đại tu kiến trúc, KHÔNG soak phát hành).
CỔNG          : **T2 = PASS 81/81** (lô 8, 674,8 s). Tám đợt T2 liên tiếp PASS trong phiên: 79/79 @ `f3cf3c97` (647,3 s),
                80/80 @ `2b770814` (719,5 s), `e66b0e28` (692,3 s), `dac8023d` (834,7 s), `88d7473b` (659,6 s),
                `13c396ab` (678,7 s), `9eb0e589` (685,4 s), 81/81 @ `bdf5c61` (674,8 s).
                **L1-medium PASS** (lô 7, 24 task, 167 s) + **Miri PASS** (25 test, 6,49 s) tại `9eb0e589`
                — tái dùng hợp lệ cho lô 8 (§6.3). `bench-bkl` (`KT-415`): tranh chấp BKL 1/2/4 hart = 0 % / 39,6 % / 93,5 %.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (`KT-417` → `KT-417b` gỡ `plat-common::obj`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + `PerHartState` (**DONE**: `KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 17 `EP_*` (`KT-404/404d` DONE → `KT-417/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`/`rngsrv`) ra User Space (`KT-419/419b/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (**DONE**: `KT-411/411b/412/413/413b/415`).
TASK          : **384 task** — **DONE (371 task)** / **3 READY** / **10 BLOCKED** (Đại tu: **29/42 DONE**, 0 chờ duyệt).
                **`READY` (3 slot song song W7a, 0 % chồng tệp theo `D2-261`)**:
                • `KT-417` (P0 đường găng — **APPROVED `D2-261`**, giữ `abi_version = 1`: áp `spec/` + `vifast-gen` `[boot_caps]`
                  + cài `Tcb.{Bind,Unbind}Notification` & `CNode.{Mutate,Rotate}`);
                • `KT-413c` (P1 — **APPROVED `D2-261`** 24 mục `# sched:`: gỡ trọn 2.162 LOC `sched.rs` `x86_64`, +64 KiB `.bss`);
                • `KT-408c` (P1 — vá 7 `E_WRONG-REASON` `v2 mutants --all` + cấp tick `Machine::now` `riscv64` + 5 `SLOT_EP_*` `netsrv` + `vifast-bench --bkl`).
                **`BLOCKED` thuần theo DAG (10 task)**: `KT-417b` (1 cây Cap/MDB, mở khoá Chặng 3), `KT-417c` (`x86_64` `syscall`/`sysret`),
                `KT-418c` (vá `-cpu max` `VSpace` isolation + `hardening-max.sh`, KHÔNG nới ratchet `B1.1`),
                `KT-419` (`Irq*` + `MapIo`), `KT-419b` (`IOSpace` qua `virtio-iommu-pci` 3 arch), `KT-420..KT-424`.
ĐANG CHẠY    : **`KT-417` (slot 1 W7a, Lead)**: cửa sổ `spec/` + `[boot_caps]` + `Tcb.{Bind,Unbind}Notification`
                + `CNode.{Mutate,Rotate}` đã cài; **T0 PASS** (`t0 vifast-core` 133 test, clippy sạch) +
                `check_abi/check_kernel/gen-check/verify-numbers` PASS; **T1/T2 chờ chạy trên cây sạch sau commit**.
                Khe Thấp điểm trưa `04:00 UTC`: mở thêm `KT-413c` ∥ `KT-408c` (2 slot, worktree riêng).
                Nhật ký: `JOURNAL/kt417-spec-bootcaps.md`.
                **Ghi chú kiến trúc (`D2-261`)**: ratchet `B1.1` giữ cứng (`driver_files ≤ 23`, `virtio-rng` đi `rngsrv` User Space ở `KT-423`);
                ngưỡng BKL S3 đã kích hoạt (93,5 % ở 4 hart) ⇒ Khoá `PerHartState` + `HartTier2` là hạng mục #1 hậu `KT-424`.
MÁY (sim)     : `size_of::<Machine>()` **184.712** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
