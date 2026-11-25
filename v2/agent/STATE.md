# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-25 15:10 JST (KT-414 DONE sau T2 75/75 @ 26287331) ===
HEAD          : `26287331` (merge `task/kt414-uefi`, T2 PASS 75/75, 640,5 s). Cây sạch; `KT-400`, `KT-401`, `KT-402`, `KT-403`, `KT-406`, `KT-414` DONE trên `main`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424` + `KT-401b/402b/414b`,
                28 task ≤ 3 h, 4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 = PASS 75/75 tại `26287331`** (`KT-414` merge, 640,5 s, 2026-11-25);
                trước đó **T2 = PASS 75/75 tại `bdfe97dd`** (`KT-401` merge, 788,5 s), `d39b86b9` (757,2 s), `6c1accd5` (621,8 s) và `6cf4640c` (W1).
                `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) & `KT-348b` (a64 1000/1000, 48/48) `DONE` tại `899cbcb`.
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **370 task / 348 DONE / 5 READY / 17 BLOCKED** — `KT-243..KT-348b` đã khép (126 task).
                Đã `DONE` (4/28): `KT-400` (dọn kho), `KT-402` (`BootInfo` v1), `KT-403` (ratchet `B1.1` + 24/24 mutants
                gỡ 43 `mut-*` + telemetry + trace 32 B + `vifast-bench` smoke; ring trace kernel → `KT-408`), `KT-406` (UART RX IRQ 3 arch).
                **`READY` (5 task)**:
                • `KT-401b` — W^X 4 KiB `riscv64` + `.corespace` NX 3 arch (sub-agent `kt401b2` đang chạy).
                • `KT-404` — workspace Cargo thật `v2/user/` + `vifast-kbuild` + đóng `N-7` + 9 crate `forbid(unsafe_code)`.
                • `KT-405` — `CoreObserver` + `PerHartState` + intrusive ready queue + 245 accessor + dọn 15 lint clippy `vifast-core` (sub-agent `kt405b` đang chạy).
                • `KT-407` — đại tu tooling + vá `guard_selftest.sh` case 9 (sub-agent `kt407b` đang chạy).
                • `KT-414b` — UEFI `aarch64` (target `aarch64-unknown-uefi` đã cài cho `1.100.0`).
                **`BLOCKED` theo DAG (17 task)**: `KT-402b` (harness `a64 -dtb` + `x64 fw_cfg`), `KT-408..KT-413`, `KT-415..KT-424`.
ĐANG CHẠY    : **Khe Thấp điểm trưa (`04:00–06:00 UTC` / `13:00–15:00 JST`)** — 3 slot song song:
                • Lead: `KT-401` DONE (T2 75/75 @ `bdfe97dd`) + `KT-414` DONE (T2 75/75 @ `26287331`).
                • `kt407b` (slot 1): đại tu tooling + vá `guard_selftest.sh` case 9.
                • `kt401b2` (slot 3): W^X 4 KiB `riscv64` + `.corespace` NX 3 arch.
                • `kt405b` (slot 4): `CoreObserver`/`PerHartState` + dọn 15 lint clippy `vifast-core`.
                • Slot 2 trống (chờ `KT-404`).
                Ghi chú: `v2 mutants --all` đã chạy 2026-11-25 (17 KILLED / 7 E_WRONG-REASON — xem `JOURNAL/kt403-mutants-all.md`);
                Miri dừng chủ động ở 31/33 nhóm (PASS) để nhường máy cho T2; target UEFI `1.100.0` đã cài.
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
