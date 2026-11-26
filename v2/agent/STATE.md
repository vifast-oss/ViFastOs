# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-27 04:10 JST (Lead: W8b T2 PASS @`4f6acb02`; cổng siết thêm 2 commit; hotfix `KT-417c` đang chạy) ===
HEAD          : `e6af3f76` — sau T2 gộp W8b (`4f6acb02`, PASS 81/81) có **2 commit siết cổng chưa T2**:
                `a0b5375e` (sàn test L0 1.097 + T1 phải bỏ đúng 14 bước) và `e6af3f76` (L1 bắt buộc
                dòng `# L1 OK: 1000 seed`). **HEAD này chưa có T2** — sẽ gộp vào lượt T2 cuối cùng
                cùng hotfix `KT-417c`. `origin/main` = `e6c56d8d` (2 commit trên chưa push).
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`..`D2-263`). W7b/W8a xong;
                W8b xong `KT-417c` nhưng **audit mã tìm 3 MAJOR** (X1 frame ghi đè `rsp0`; X2 cặp
                `swapgs` đường `Exit` ⇒ leo thang; X3 bằng chứng `cs` tautology) ⇒ **đang hotfix**
                (`task/kt417c-fix`); `KT-419` chờ `KT-417d` DONE (phần 2/2, 8,5–11,5 h `[ƯL]`) —
                **chờ Agent 0 duyệt tách** `KT-417e/f/g/h` (`/tmp/briefs/kt417e-split.md` + gói duyệt).
CỔNG          : **T2 PASS 81/81 @`4f6acb02`** (`registered=81 ran=81 skipped=0 failed=0`, 670 s);
                `JOURNAL/t2-w8b-20261126/`. T2 W8a PASS @`5e9f13de`. W7b đỏ 1/81 do DNS slirp
                (T-INFRA, không tính xanh `D2-07`). **HEAD `e6af3f76` chưa T2.**
KIẾN TRÚC     : (1) 1 Cây Capability/MDB: `KT-417`+`KT-417b`+`KT-417d` phần 1/2 + hotfix F1/F2/F3 DONE;
                (2) Lõi Kép DONE; (3) User Space: `KT-420` DONE, còn `KT-420b`; (4) W9 cần sửa DoD;
                (5) `vifast-ktest` 3 arch DONE. **Audit**: `code-audit` 3 MAJOR (KT-417c, đang vá),
                `test-audit` 12 gap, `harness-audit` 15 (3 CAO: L1 0-seed — **đã vá**; INFRA_INVALID
                không chặn; T2 khoá đo chưa đủ), `docs-audit` 15 (QUEUE lệch 6 task; KT-417c 3 trạng thái).
TASK          : **389** — **DONE 380** (`KT-420`, `KT-417c`) / **PARTIAL 1** (`KT-417d` 3/5, phần 2 chờ tách) /
                **READY 0** / **BLOCKED 8** (`KT-419`, `KT-419b`, `KT-420b`, `KT-421`..`KT-423b`, `KT-424`).
                **`KT-417c` chưa đóng chính thức** (QUEUE in BLOCKED cũ; audit X1/X2/X3) — Agent 0 chốt
                DONE/BLOCKED + `D2-264`; QUEUE còn lệch 6 task đã DONE trên git.
ĐANG CHẠY    : `fix-kt417c` (slot 3, `task/kt417c-fix`): X1/X2/X3 + pin selector `0x23/0x1b` vào
                `run-x86_64.sh`, red-first + T0; **T1 sau khi Lead commit**. Sau đó: merge → **T2 cuối**
                trên HEAD gộp (gồm 2 commit cổng) → chạy nặng bù (soak 600, hardening-max, product boot,
                aarch64 smoke). Khung thấp điểm `D2-256`; **00:50 UTC** dừng mở sub-agent mới.
MÁY (sim)     : `size_of::<Machine>()` **172.424** B / trần **184.832** / pin **196.608**; phần 2 `KT-417d`
                dự kiến +3,2–3,5 KiB (headroom 12.408 B) — pin `footprint`/4 tệp runner cập nhật cùng commit.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
