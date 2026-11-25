# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-25 09:43 JST (KT-406 DONE + KT-403 r2 vào main, T2 75/75) ===
HEAD          : `d39b86b9`. Cây sạch; đã push `main` (`KT-406` DONE + `KT-403` r2 hợp nhất). 2 tag lưu trữ đã đẩy trước đó.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424`, 25 task ≤ 3 h,
                4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 = PASS 75/75 tại `d39b86b9`** (`KT-406` + `KT-403` r2, 757,2 s, `selftest qemu guard` OK);
                trước đó **T2 = PASS 75/75 tại `6c1accd5`** (`KT-406`, 621,8 s).
                T2 lượt 1 đỏ **duy nhất** `selftest qemu guard` case 9 = `T-INFRA` (QEMU mồ côi slot khác,
                `qemu_ppid1=2`); chạy lại **1 lượt** đúng `D2-07` ⇒ PASS. W1 giữ T2 75/75 tại `6cf4640c`.
                `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) & `KT-348b` (a64 1000/1000, 48/48) `DONE` tại `899cbcb`.
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **367 task / 345 DONE / 1 READY / 21 BLOCKED** — `KT-243..KT-348b` đã khép (126 task).
                W1: `KT-400` DONE (kho 2.898→1.678 tệp, 2 tag); `KT-402` DONE (BootInfo v1 3 arch, 32 test, QEMU 3 arch).
                W2: `KT-406` DONE (UART RX IRQ thật PL011/COM1 + `product-{aarch64,x86_64}.sh` + `LD_PRELOAD`/`.dylib`;
                QEMU a64 141,7 s + x64 278,3 s; runner 3 arch 11/11, 12/12, fails=0).
                W1 nợ (`KT-401` BLOCKED — chờ Agent 0 duyệt tách task lúc Cao điểm):
                `task/kt401-r1`+`task/kt401-r2` (`85e7626f`) đã xong **NBSend `keep_committed` một luật cho mọi đường**
                + **fixture ELF 32 KiB > guard cũ**; còn lại **NX riscv64 4 KiB cần D2** ⇒ đề xuất `KT-401b` (NX).
                Dry-run merge r2 → `main`: **1 xung đột** `vifast-plat-x86_64/src/asm.rs` (r1 tách `asm.rs` ↔ `KT-406` sửa `asm.rs`)
                ⇒ phải gộp tay; `task/kt414-uefi` merge sạch (0 xung đột).
                `KT-403`: nhánh `task/kt403-r2` (`8214e5c1`) **ĐÃ merge `main` + T2 75/75 tại `d39b86b9`** (gỡ 43 `mut-*`,
                24/24 mutants, `nightly --all`); còn **ring trace kernel 32 B** ⇒ chờ Agent 0 chốt `DONE` + tách `KT-403b`
                (hoặc gộp vào `KT-405/408` theo journal).
                `KT-414` READY — nhánh `task/kt414-uefi` (`7974bde1`): UEFI x86_64 (`efi_main` PE/COFF + `GetMemoryMap`
                fail-closed 7 mã lỗi + handoff `BootInfo` v1, 18 test, 2 ca đỏ-trước); **aarch64 + `--firmware uefi` chưa nối**
                ⇒ chưa T1/T2. **BLOCKED theo DAG**: 20 task còn lại của `KT-404..KT-424`.
ĐANG CHẠY    : **Không có** — dừng 09:50 JST (10 phút trước Cao điểm 1). Nhánh chờ xử lý tiếp: `task/kt401-r2`
                (`85e7626f`, chờ duyệt) và `task/kt414-uefi` (`7974bde1`, x86_64 xong, chưa T1/T2).
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
