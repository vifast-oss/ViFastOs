# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-25 16:05 JST (KT-401b/405/407 DONE sau T2 75/75 @ a61921b4) ===
HEAD          : `a61921b4` (merge KT-401b + KT-407 + KT-405 + ghim harness, T2 PASS 75/75, 674,9 s). Cây sạch; `KT-400`, `KT-401`, `KT-401b`, `KT-402`, `KT-403`, `KT-405`, `KT-406`, `KT-407`, `KT-414` DONE trên `main`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424` + `KT-401b/402b/414b`,
                28 task ≤ 3 h, 4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 = PASS 75/75 tại `a61921b4`** (KT-401b + KT-407 + KT-405 + ghim harness, 674,9 s, 2026-11-25);
                trước đó **T2 = PASS 75/75 tại `26287331`** (KT-414, 640,5 s), `bdfe97dd` (KT-401, 788,5 s), `d39b86b9` (757,2 s), `6c1accd5` (621,8 s) và `6cf4640c` (W1).
                `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) & `KT-348b` (a64 1000/1000, 48/48) `DONE` tại `899cbcb`.
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **370 task / 351 DONE / 4 READY / 15 BLOCKED** — `KT-243..KT-348b` đã khép (126 task).
                Đã `DONE` (4/28): `KT-400` (dọn kho), `KT-402` (`BootInfo` v1), `KT-403` (ratchet `B1.1` + 24/24 mutants
                gỡ 43 `mut-*` + telemetry + trace 32 B + `vifast-bench` smoke; ring trace kernel → `KT-408`), `KT-406` (UART RX IRQ 3 arch).
                **`READY` (4 task)**:
                • `KT-402b` — harness `aarch64 -dtb` + `x86_64 fw_cfg` (mở từ `KT-402` + `KT-407` DONE).
                • `KT-404` — workspace Cargo thật `v2/user/` + `vifast-kbuild` + đóng `N-7` + 9 crate `forbid(unsafe_code)`.
                • `KT-408` — ring trace 32 B trong kernel (`FixedRing` + `pmemsave`, mở từ `KT-405` DONE).
                • `KT-414b` — UEFI `aarch64` (target `aarch64-unknown-uefi` đã cài cho `1.100.0`).
                **`BLOCKED` theo DAG (15 task)**: `KT-409..KT-413`, `KT-415..KT-424`.
ĐANG CHẠY    : **Không có** — 3 task song song đã khép (`KT-401b`, `KT-405`, `KT-407` DONE @ `a61921b4`).
                Khe Thấp điểm trưa kết thúc 15:00 JST; kế tiếp: `KT-404` + `KT-414b` ở khe thấp điểm tối (19:00 JST).
                Ghi chú: `v2 mutants --all` 2026-11-25 (17 KILLED / 7 E_WRONG-REASON — `JOURNAL/kt403-mutants-all.md`);
                Miri dừng chủ động 31/33 nhóm (PASS) để nhường máy cho T2; target UEFI `1.100.0` đã cài.
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
