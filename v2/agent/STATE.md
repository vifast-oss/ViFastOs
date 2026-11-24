# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-25 07:55 JST (đóng phần làm được của W1) ===
HEAD          : `6cf4640c`. Cây sạch; W1 đã hợp nhất `main` + push (2 tag lưu trữ đã đẩy).
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424`, 25 task ≤ 3 h,
                4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 = PASS 75/75 tại `6cf4640c`** (ứng viên W1, gồm bản vá T-INFRA `guard_selftest`).
                `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) & `KT-348b` (a64 1000/1000, 48/48) `DONE` tại `899cbcb`.
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **367 task / 344 DONE / 0 READY / 23 BLOCKED** — `KT-243..KT-348b` đã khép (126 task).
                W1: `KT-400` DONE (kho 2.898→1.678 tệp, 2 tag) và `KT-402` DONE (BootInfo v1 3 arch, 32 test, QEMU 3 arch).
                W1 nợ: `KT-401` BLOCKED (NBSend `keep_committed` toàn đường; NX riscv64 cần D2; fixture ELF > 24 KiB)
                và `KT-403` BLOCKED (nối 23 mutant + gỡ 43 `mut-*`; ring trace kernel) — **đề xuất `KT-401a/b/c` +
                `KT-403a/b` chờ Agent 0 duyệt trong giờ Cao điểm**.
                **BLOCKED (theo DAG)**: `KT-404..KT-424` (23 task chờ phụ thuộc trong `KE-HOACH-KIEN-TRUC.md`).
ĐANG CHẠY    : Không có — đã dừng trước mốc Cao điểm 1 (`01:00 UTC` / `10:00 JST`, `D2-256`). `next` = ∅: mọi task
                còn lại đều BLOCKED theo DAG hoặc chờ quyết định tách task con (`KT-401a/b/c`, `KT-403a/b`).
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
