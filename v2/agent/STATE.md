# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-26 08:50 JST (KHÉP phiên Thấp điểm chính; T2 81/81) ===
HEAD          : `main` sạch sau lô 8. **26 task DONE** trong phiên: lô 1 `KT-400/402/403/406/402b/404/408/414b`;
                lô 2 `KT-409/409b/410/411`; lô 3 `KT-412/413`; lô 4 `KT-415/416/418`; lô 5 `KT-411b`;
                lô 6 `KT-413b/418b`; lô 7 `KT-408b/404c`; lô 8 `KT-416b/404d`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424` + `KT-401b/402b/409b/411b/413b/413c/414b/418b/418c`).
CỔNG          : **T2 = PASS 81/81** (lô 8, 674,8 s). Tám đợt T2 trong phiên, tất cả PASS: 79/79 @ `f3cf3c97` (647,3 s),
                80/80 @ `2b770814` (719,5 s), `e66b0e28` (692,3 s), `dac8023d` (834,7 s), `88d7473b` (659,6 s),
                `13c396ab` (678,7 s), `9eb0e589` (685,4 s), 81/81 @ `merge KT-404d` (674,8 s).
                **L1-medium PASS** (lô 7, 24 task, 167 s) + **Miri PASS** (25 test, 6,49 s) tại `9eb0e589`
                — tái dùng hợp lệ cho lô 8 (§6.3: lô 8 chỉ đổi tài liệu + tool/bước cổng, không chạm sim/kernel/ảnh).
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`, nay có **máy kiểm** `KT-404d`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung **3 arch** (`KT-411/411b/413b`) + SMP 4 hart 3 arch (`KT-412/413/415`).
TASK          : **384 task / 375 DONE / 2 READY / 7 BLOCKED**.
                **`READY` (2 task)**: `KT-413c` (gỡ 2.162 LOC `sched.rs` + bật PCID — cần **D2 duyệt** 24 mục kỳ vọng),
                `KT-418c` (adapter `virtio-rng` + TAP 3 arch — cần **duyệt baseline B1.1**; `-cpu max`).
                **`BLOCKED` (7 task)**: `KT-417` (**chờ `APPROVED` của Agent 0** — gói `KT-416b` đã sẵn: 8 khuyến nghị A/B
                + 8 đính chính; **không cần `abi_version = 2`**), `KT-419..KT-424`.
ĐANG CHẠY    : **KHÔNG** — phiên Song song đã khép. Nhật ký: `JOURNAL/thap-diem-chinh-20261125.md` (§1..§10).
                **Nợ chuyển phiên sau**: `KT-413c`, `KT-418c`, `KT-417` (chờ duyệt); 3 tệp `/tmp/kt418b-*.removed`
                (adapter rng) sẽ mất khi khởi động lại máy — nội dung đã ghi trong `JOURNAL/kt418b-rng-smap.md`.
                **Nợ mới phát hiện (chưa mở task)**: `Machine::now` **luôn 0** trên riscv64 ⇒ `t_ns` của mọi bản ghi
                trace = 0 (`KT-408b`); `cfg!(feature="product-net")` là nhánh chết trong `riscv64/build.rs`;
                2 arch chưa in `# build:`; 5 `SLOT_EP_*` của `netsrv_prod.rs` chưa được `check-boot-abi` canh.
MÁY (sim)     : `size_of::<Machine>()` **184.712** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
