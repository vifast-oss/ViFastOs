# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-27 02:20 JST (Lead: W8a merge `5e9f13de` + T2 PASS 81/81; W8b mở KT-417c) ===
HEAD          : `5e9f13de` — **W8a đã merge**: `b8bcbd74` `KT-420` (6 `.vidl` + stub typed + `namesrv` + `PageSource`; T1 PASS 750,6 s)
                + `60aba561` `KT-417d` **phần 1/2** (`PageTable.{Map,Unmap}` thật, xoá `[limits.plat]`, đóng Q4-F6, xoá shim
                `plat-common::obj`; T1 PASS 675,4 s) + `5e9f13de` vá race `STATS_LOCK` `ptmap_tests` (T2 lan 1 đỏ ⇒ lan 2 xanh).
                Đã push `origin/main`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`..`D2-263`). W7b DONE; W8a xong phần mở;
                phần còn lại của `KT-417d` (bảng VM vào `KernelStore`, xoá bảng thứ hai/`core_foreign.rs` + `ArchVmOps`,
                `Revoke` 4 loại) ước **8,5–11,5 h** `[ƯL]` ⇒ **chờ Agent 0 duyệt tách** (`KT-417e/f/g/h`, đề xuất
                `/tmp/briefs/kt417e-split.md`; `VmObjMeta` bảng rời hay gộp — cần chốt).
CỔNG          : **T2 = PASS 81/81 @ `5e9f13de`** (647 s, `# gate-steps registered=81 ran=81 skipped=0 failed=0`) — bằng chứng
                `JOURNAL/t2-w8a-20261126/`. Lan 1 @ `60aba561`: FAIL 1/81 do **race test** `ptmap_tests` (`invoke::stats()`
                toàn cục; tái hiện 1/20) ⇒ vá `5e9f13de` (giữ `STATS_LOCK`, khẳng định `==` nguyên) ⇒ 0/30; lan 2 xanh.
                W7b: T2 lan 1 @ `b76e3bc8` đỏ 1/81 do DNS slirp (`T-INFRA`, chạy lại riêng bước PASS) — không tính xanh `D2-07`.
KIẾN TRÚC     : (1) 1 Cây Capability/MDB trong `vifast-core`: `KT-417`+`KT-417b`+**`KT-417d` phần 1/2** DONE (chờ phần 2);
                (2) Lõi Kép **DONE**; (3) User Space: **`KT-420` DONE** (6 `.vidl`, `namesrv`, `PageSource`), còn `KT-420b`;
                (4) đẩy driver/dịch vụ ra U-mode (`KT-421`..`KT-423b`; **W9 cần xem lại DoD** — recon: `MapIo`/`Irq*`/`IOSpace`
                chưa cài, `driver_files→0` gồm 23 mục chưa task nào phủ, "0 % chồng tệp" sai); (5) `vifast-ktest` 3 arch **DONE**.
TASK          : **389** — **DONE 379** (thêm `KT-420`) / **READY 1** (`KT-417c`) / **PARTIAL 1** (`KT-417d`, 3/5 mục, phần 1 đã
                merge) / **BLOCKED 8** thuần DAG (`KT-419`, `KT-419b`, `KT-420b`, `KT-421`, `KT-422`, `KT-423`, `KT-423b`,
                `KT-424`). **`QUEUE.md` còn in trạng thái cũ** — Agent 0 cần đồng bộ; Lead không sửa QUEUE.
ĐANG CHẠY    : **W8b** `KT-417c` (x86_64 `syscall`/`sysret` + GDT `planned` + phục hồi preflight CPL3) — brief
                `/tmp/briefs/dev-kt417c.md`; `KT-419` **chưa** mở vì phụ thuộc `KT-417d` DONE. Đồng thời: xác nhận soak W7b
                (`soak-x86_64` 600×3 + `soak-aarch64 --smoke 2 1`). Khung thấp điểm `D2-256`; **00:50 UTC** dừng mở sub-agent mới.
MÁY (sim)     : `size_of::<Machine>()` **172.424** B (`Cap` 32 B; bảng VM **chưa** vào store) / trần KT-312 **184.832** / pin **196.608**.
                Phần 2 `KT-417d` dự kiến +3,2–3,5 KiB (headroom 12.408 B) — pin `footprint`/4 tệp runner phải cập nhật cùng commit.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
