# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-27 09:40 JST (Lead: T2 cuối PASS @`b97c33c1`; dừng mở việc mới 00:50 UTC) ===
HEAD          : `b97c33c1` — **T2 PASS 81/81** (00:20–00:31 UTC, `registered=81 ran=81 skipped=0 failed=0`,
                647 s; `JOURNAL/t2-final-20261127/`). Gồm: W8b `KT-417c` (`075b0639`), hotfix F1/F2/F3
                `KT-417d` (`a775affe`), F5 22 host test (`10fd92f8`), runner 1 test-thread (`4f6acb02`),
                sàn L0 1.097 + T1 đúng 14 bước (`a0b5375e`), L1 bắt buộc seed (`e6af3f76`). Đã push.
KẾ HOẠCH      : [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`D2-259`..`D2-263`). W7b/W8a xong; W8b
                `KT-417c` **đã merge + T2 xanh** nhưng audit mã tìm **X1/X2/X3 MAJOR** (frame 136 B ghi
                `rsp0`; cặp `swapgs` đường `Exit` ⇒ leo thang; `cs` hằng ⇒ bằng chứng tautology) —
                **hotfix dở dang**, xem ĐANG CHẠY. `KT-417d` phần 2/2 (8,5–11,5 h `[ƯL]`) **chờ Agent 0
                duyệt tách** `KT-417e/f/g/h` (`/tmp/briefs/kt417e-split.md`); `KT-419` chờ `KT-417d` DONE.
CỔNG          : **T2 PASS 81/81 @`b97c33c1`** (mới nhất). Trước: `4f6acb02` PASS 81/81; `5e9f13de` PASS;
                W7b đỏ 1/81 do DNS slirp (T-INFRA, không tính xanh `D2-07`).
KIẾN TRÚC     : (1) 1 Cây Capability/MDB: `KT-417`+`KT-417b`+`KT-417d` phần 1/2 + hotfix F1/F2/F3; (2) Lõi Kép DONE;
                (3) User Space `KT-420` DONE, `KT-420b` chưa; (4) W9 cần sửa DoD (recon: `MapIo`/`Irq*`/`IOSpace`
                chưa cài; `driver_files→0` bất khả thi; "0 % chồng tệp" sai); (5) `vifast-ktest` 3 arch DONE.
                **Audit**: `code` 3 MAJOR (KT-417c) +2 MINOR; `test` 12 gap; `harness` 15 (L1 0-seed **đã vá**,
                INFRA_INVALID chưa chặn, khoá đo T2 chưa đủ); `docs` 15 (QUEUE lệch 6 task, KT-417c 3 trạng thái).
TASK          : **389** — DONE 380 / PARTIAL 1 (`KT-417d`, 3/5) / READY 0 / BLOCKED 8. `KT-417c` **chưa đóng
                chính thức** (X1/X2/X3) — Agent 0 chốt trạng thái + `D2-264`; QUEUE còn lệch 6 task đã DONE.
ĐANG CHẠY    : **Miri** (`v2 nightly --only-miri`, bắt đầu 02:12 UTC). **Dở dang cần lượt sau**: (a) hotfix `KT-417c`
                X1/X2/X3 — mã chưa commit ở `/private/tmp/vifast-wt/3` nhánh `task/kt417c-fix` (5 tệp
                +283/−33 + `tests/syscall_frame_gs.rs` + red logs), **chưa T0/T1, không merge**;
                (b) `prep-approvals` + `fix-harness-2` phải chạy lại (3 agent chết do lỗi server LLM ~18:50 UTC);
                (c) chạy nặng bù: `soak-x86_64 600`, `hardening-max.sh`, product boot 3 arch,
                `soak-aarch64 --smoke 2 1`, `l1-tier full`; (d) chờ Agent 0: tách `KT-417d`, F4, `VmObjMeta`,
                DoD W9, `KT-417c` DONE/BLOCKED, `D2-264`.
MÁY (sim)     : `size_of::<Machine>()` **172.424** B / trần **184.832** / pin **196.608**.
CHẠY NẶNG     : 2026-11-27 00:44–02:10 UTC trên cây `44e60e67` (build-inputs ≡ `b97c33c1` T2 PASS),
                bằng chứng `JOURNAL/heavy-20261127/`: soak x86_64 **600/600 lỗi=0 PASS** (đóng nợ KT-413c);
                soak aarch64 smoke 2/2 (không phải bằng chứng cổng); hardening-max **PASS 4 chân**;
                product riscv64/aarch64/x86_64 **PASS** (10/10, 11/11); `l1-tier full` **400k seed PASS**.
                `mutants --all` **FAIL**: `mutants.py:480 TypeError str/bytes` (dừng giữa chừng) +
                `mut-info-x1` E_WRONG-REASON ⇒ vá công cụ + điều tra ở khung thấp điểm. Miri đang chạy.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
