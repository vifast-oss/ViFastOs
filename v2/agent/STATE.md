# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-24 16:00 JST (D2-259, mở Chương trình Đại tu Kiến trúc) ===
HEAD          : `14630eb`. Cây sạch; kế hoạch phát hành cũ (`v2.0`/`v2.1`/`KT-348`) đã huỷ bỏ & archive.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424`, 25 task ≤ 3 h,
                4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 #7 = PASS 75/75** (`84adb4a2`); `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) &
                `KT-348b` (a64 1000/1000, `soak-aarch64 --selftest` 48/48) đã `DONE` tại `899cbcb`.
                Từ `D2-259`: không chạy soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **KT-243..KT-348b** DONE (126 task) — nền cũ đã khép (`KT-348` `superseded`, tổng 342 DONE).
                **READY NGAY (Đợt W1 — 4 slot, 0 chồng tệp)**: **`KT-400`**, **`KT-401`**, **`KT-402`**, **`KT-403`**.
                **BLOCKED (theo DAG)**: `KT-404..KT-424` (21 task chờ phụ thuộc trong `KE-HOACH-KIEN-TRUC.md`).
ĐANG CHẠY    : Không có — giữ quy tắc giờ Cao điểm 2 (`06:00–10:00 UTC`, `D2-256`); mở đồng loạt 4 slot W1
                (`KT-400 ∥ KT-401 ∥ KT-402 ∥ KT-403`) ngay đầu khung **Thấp điểm chính (`10:00 UTC`, 0,5×)**.
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
