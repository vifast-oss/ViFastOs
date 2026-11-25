# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-25 10:05 JST (D2-260: Agent 0 duyệt gói 5 mục) ===
HEAD          : `78b2799f` (cây code đã T2 tại `d39b86b9`). Cây sạch; `KT-400`, `KT-402`, `KT-403`, `KT-406` DONE trên `main`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424` + `KT-401b/402b/414b`,
                28 task ≤ 3 h, 4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 = PASS 75/75 tại `d39b86b9`** (`KT-406` + `KT-403` r2, 757,2 s, `selftest qemu guard` OK);
                trước đó **T2 = PASS 75/75 tại `6c1accd5`** (`KT-406`, 621,8 s) và `6cf4640c` (W1).
                `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) & `KT-348b` (a64 1000/1000, 48/48) `DONE` tại `899cbcb`.
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **370 task / 346 DONE / 3 READY / 21 BLOCKED** — `KT-243..KT-348b` đã khép (126 task).
                Đã `DONE` (4/28): `KT-400` (dọn kho), `KT-402` (`BootInfo` v1), `KT-403` (ratchet `B1.1` + 24/24 mutants
                gỡ 43 `mut-*` + telemetry + trace 32 B + `vifast-bench` smoke; ring trace kernel → `KT-408`), `KT-406` (UART RX IRQ 3 arch).
                **`READY` (3 task theo `D2-260`)**:
                • `KT-401` — đã xong trên `task/kt401-r2` (`85e7626f`: R1 5 tệp + ELF 32 KiB + `.corespace` NOLOAD rv +
                  `NBSend` một luật); Lead gộp tay xung đột `plat-x86_64/src/asm.rs` (đưa `vifast_isr_serial` sang `asm/trap.rs`) + T2 → `DONE` (mở ngay `KT-401b`, `KT-404`, `KT-405`).
                • `KT-407` — mở từ `KT-400`/`KT-403` DONE; đã gắn thêm vá T-INFRA `guard_selftest.sh` case 9.
                • `KT-414` — đã có `task/kt414-uefi` (`7974bde1`); duyệt `BootSource::Uefi` trong `BootInfo` v1 (giữ v1) +
                  `rsdp_pa`/`dtb_pa` trong `UefiHandoffV1` + Lead nối `--firmware uefi` x86_64 + T1/T2.
                **`BLOCKED` theo DAG (21 task)**: `KT-401b` (W^X 4 KiB rv + `.corespace` NX 3 arch), `KT-402b` (harness `a64 -dtb` +
                `x64 fw_cfg`), `KT-404`, `KT-405` (kèm dọn 15 lint clippy `vifast-core`), `KT-408..KT-413`, `KT-414b` (`uefi-aarch64`,
                chờ cài target UEFI trên toolchain `1.100.0`), `KT-415..KT-424`.
ĐANG CHẠY    : **Không có** — đang trong Giờ Cao điểm 1 (`01:00–04:00 UTC` / `10:00–13:00 JST`, `D2-256`).
                Lượt kế ở Khe Thấp điểm trưa (`04:00 UTC` / `13:00 JST`): Lead gộp `KT-401` + T2; mở `KT-407`, `KT-414`.
                Ghi chú Cao điểm: Lead có thể chạy `v2 mutants --all` (0 token LLM) khi máy rảnh; người điều phối cài 1 lần
                `rustup target add x86_64-unknown-uefi aarch64-unknown-uefi --toolchain 1.100.0` cho `KT-414b`.
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
