# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-27 03:45 JST (Lead: W8b merge `075b0639`, T2 PASS 81/81 @`4f6acb02`, hotfix KT-417c đang chạy) ===
HEAD          : `4f6acb02` — W8b đã merge: `KT-417c` (`a5541601`, syscall/sysret + GDT planned) + hotfix
                `KT-417d` F1/F2/F3 (`824f4038`) + vá L0 1 test-thread (`4f6acb02`). **T2 = PASS 81/81** @`4f6acb02`
                (`registered=81 ran=81 skipped=0 failed=0`); bằng chứng `JOURNAL/t2-w8b-20261126/`. Đã push tới `4f6acb02`?
                (kiểm `git status -sb` trước khi đọc tiếp).
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`..`D2-263`). W7b/W8a xong; W8b xong phần `KT-417c`;
                `KT-419` **vẫn chờ** `KT-417d` DONE (phần 2/2). Phần còn lại của `KT-417d` (bảng VM vào `KernelStore`,
                xoá bảng thứ hai/`core_foreign.rs`/`ArchVmOps`, `Revoke` 4 loại) ước **8,5–11,5 h** `[ƯL]` ⇒ **chờ Agent 0
                duyệt tách** (`KT-417e/f/g/h`, `/tmp/briefs/kt417e-split.md`, gói duyệt `/tmp/briefs/approval-packet-kt417e.md`).
CỔNG          : T2 W8a PASS 81/81 @`5e9f13de`; **T2 W8b PASS 81/81 @`4f6acb02`** (lan 1 @`075b0639` đỏ 1/81 do race
                `capalloc_tests::cnode_ca_am_ma_loi` — vá `V2_TEST_ONE` chạy binary `vifast_core` 1 test-thread).
                W7b: T2 lan 1 @`b76e3bc8` đỏ 1/81 do DNS slirp (T-INFRA, không tính xanh `D2-07`).
KIẾN TRÚC     : (1) 1 Cây Capability/MDB trong `vifast-core`: `KT-417`+`KT-417b`+`KT-417d` phần 1/2 + hotfix F1/F2/F3 DONE;
                (2) Lõi Kép **DONE**; (3) User Space: `KT-420` DONE, còn `KT-420b`; (4) `KT-421`..`KT-423b`: **W9 cần sửa DoD**
                (recon: `MapIo`/`Irq*`/`IOSpace` chưa cài; `driver_files→0` gồm 23 mục chưa phủ; "0 % chồng tệp" sai);
                (5) `vifast-ktest` 3 arch DONE. **Audit mã W8b**: 3 MAJOR thuộc `KT-417c` (X1 frame 136 B ghi đè `rsp0`;
                X2 cặp swapgs đường Exit ⇒ leo thang; X3 `cs` hằng ⇒ bằng chứng tautology) — **đang hotfix** (`task/kt417c-fix`).
TASK          : **389** — **DONE 380** (`KT-420`, `KT-417c`) / **PARTIAL 1** (`KT-417d` 3/5 mục, phần 2/2 chờ tách) /
                **READY 0** / **BLOCKED 8** thuần DAG (`KT-419`, `KT-419b`, `KT-420b`, `KT-421`..`KT-423b`, `KT-424`).
                `QUEUE.md` còn in trạng thái cũ — Agent 0 đồng bộ; Lead không sửa QUEUE.
ĐANG CHẠY    : `fix-kt417c` (slot 3, `task/kt417c-fix`): X1/X2/X3 + pin selector `0x23/0x1b` vào `run-x86_64.sh`; red-first,
                T0, **T1 sau khi Lead commit**. Song song: `audit-harness`, `audit-docs` (chỉ đọc). Khung thấp điểm
                `D2-256`; **00:50 UTC** dừng mở sub-agent mới.
MÁY (sim)     : `size_of::<Machine>()` **172.424** B (`Cap` 32 B; bảng VM chưa vào store) / trần KT-312 **184.832** / pin **196.608**.
                Phần 2 `KT-417d` dự kiến +3,2–3,5 KiB (headroom 12.408 B) — pin `footprint`/4 tệp runner phải cập nhật cùng commit.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
