# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-26 07:25 JST (T2 80/80 @ `13c396ab`; mở lô 7 — KT-408b + KT-404c) ===
HEAD          : `13c396ab` (bằng chứng lô 6). Cây sạch; **21** task DONE trong phiên: `KT-400/402/403/406/402b/404/408/414b` (lô 1),
                `KT-409/409b/410/411` (lô 2), `KT-412/413` (lô 3), `KT-415/416/418` (lô 4), `KT-411b` (lô 5), `KT-413b/418b` (lô 6).
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424` + `KT-401b/402b/409b/411b/413b/413c/414b/418b/418c`).
CỔNG          : **T2 = PASS 80/80 tại `13c396ab`** (lô 6, 678,7 s). Sáu đợt T2 trong phiên: `f3cf3c97` 79/79 (647,3 s),
                `2b770814` 80/80 (719,5 s), `e66b0e28` 80/80 (692,3 s), `dac8023d` 80/80 (834,7 s), `88d7473b` 80/80 (659,6 s),
                `13c396ab` 80/80 (678,7 s). **L1-medium PASS** (lô 2, 40k seed, 166 s) + **Miri PASS** (25 test, 6,5 s).
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium`/`L1-full` + Miri + `v2 mutants` + `vifast-bench`.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung **3 arch** (`KT-411/411b/413b`) + SMP 4 hart 3 arch (`KT-412/413/415`).
TASK          : **380 task / 367 DONE / 6 READY / 7 BLOCKED**.
                **`READY` (6 task)**: `KT-408b`/`KT-404c` (đang chạy), `KT-413c` (gỡ 2.162 LOC `sched.rs` + bật PCID — cần **D2 duyệt** 24 mục kỳ vọng),
                `KT-418c` (adapter `virtio-rng` + TAP 3 arch — cần **duyệt baseline B1.1**; `-cpu max`),
                `KT-417` (**chờ `APPROVED` của Agent 0** cho RFC `KT-416` — 8 nhóm), `KT-419` (sau `KT-417`).
                **`BLOCKED` theo DAG (7 task)**: `KT-420..KT-424` (+ `KT-419` chờ `KT-417`).
ĐANG CHẠY    : **Lô 7 (2 slot)** — `KT-408b` (bằng chứng `pmemsave` + nối 3 đột biến vào `mutants.toml`) và
                `KT-404c` (đồng bộ `v2/user/Cargo.lock` + giảm `build.rs` về ~450 dòng). Cả hai là **nợ đã ghi**,
                không đổi kỳ vọng harness, không chạm `spec/`.
                **Dừng mở task mới lúc `00:50 UTC`** (biên 10 phút trước Cao điểm 1 `01:00–04:00 UTC`); T2 cuối trên
                ứng viên đã commit, rồi khép phiên. Nhật ký phiên: `JOURNAL/thap-diem-chinh-20261125.md` (§1..§8).
                Nợ chuyển sang phiên sau: `KT-413c`, `KT-418c`, `KT-417` (chờ duyệt); 3 tệp `/tmp/kt418b-*.removed`
                (adapter rng) sẽ mất khi khởi động lại máy ⇒ nội dung đã ghi trong `JOURNAL/kt418b-rng-smap.md`.
MÁY (sim)     : `size_of::<Machine>()` **184.712** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
