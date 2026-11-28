# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-29 00:35 JST / 2026-11-28 15:35 UTC (Lead: T2 hợp nhất PASS @`5f48f92a`) ===
HEAD          : `5f48f92a` — **T2 PASS 81/81** (`# gate-steps registered=81 ran=81 skipped=0 failed=0`,
                `# gate-wall 662554 ms`; **15:20:07–15:31:13 UTC**; `JOURNAL/t2-20261128/`). Lần đầu T2 in
                **khoá đo**: `# measure-key: commit=5f48f92a… build-inputs=d428c8149c3e86e9 dirty=0
                untracked=56 toolchain=1.100.0 seeds=1000` (F12). Gồm 3 nhánh hợp nhất:
                (a) hotfix `KT-417c` v1 X1/X2/X3 (`c73eb77a`, merge `82502f7d`, T2 PASS);
                (b) hotfix vòng 2 theo review (`4d6dd9e1`/`07d93d51`, merge `5f48f92a`: đo `GS.base` tại
                cửa vào thay `ug` tautology, `gs_is_kernel` hết dead code, chốt chống revert ở `kbin`,
                runner 38 ca + ca dương + đòi đúng lý do);
                (c) hạ tầng đo `1211181c` (F7 `INFRA_INVALID` ⇒ mã 4 T-INFRA, F8 làn chết/thiếu `.rc`,
                F12 `measure-key.sh` + từ chối `??` đường build); (d) test-gap `27b51d8f` (G9 `page.rs`
                fail-closed `align>page_size` + `new(0)`, G7 pin số ca selftest, G8 bỏ số viết tay).
                T1 từng nhánh: hotfix vòng 2 `82/68/14/0` @`4d6dd9e1` (649 s; lần 1 ĐỎ 1/81 vì
                `product-shell` `halt(5)` oan — đã vá `KERNEL_GS_BASE = 0` hợp lệ), harness `82/68/14/0`
                @`1211181c` (747 s), test-gap `82/68/14/0` @`27b51d8f` (727 s). Log `JOURNAL/t1-20261128/`.
PUSH          : `origin/main` = `5f48f92a` **sau commit bằng chứng này** (push ngay sau T2 xanh, AGENTS §7).
KẾ HOẠCH      : [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`D2-259`..`D2-264`). **Chờ Agent 0**
                (gói `/tmp/briefs/agent0-*.md` + `agent0-split-lines.md` + `agent0-ky-thuat-answers.md`):
                (1) tách `KT-417d` p2/2 → `KT-417e/f/g/h` (11 h `[ƯL]`, **0 chạm `spec/`**, thứ tự bắt buộc);
                (2) **tái neo 2 pin byte**: bảng VM tối thiểu +2.284 B vượt pin `172.544` (slack 120 B;
                trần 184.832/pin 196.608 **không nới**) — nếu không, `KT-417f` bất khả thi;
                (3) `VmObjMeta` bảng riêng 20 B×256 (A3, tổng ≈179.828 B `[ƯL]`) vs gộp;
                (4) `BootCapLayout` v2 **3 trường** + 17 `EP_*` phương án (a); (5) DoD W9 viết lại **đo được**
                (`driver_files` 23→15→10→0 + `kernel_service_files` đóng băng 2; "chồng tệp có chủ").
                `KT-419`/`KT-419b` chờ `KT-417d` DONE.
CỔNG          : **T2 PASS 81/81 @`5f48f92a`** (mới nhất, 662,6 s, có `measure-key`). Trước: `82502f7d` PASS
                81/81 (11:28–11:40 UTC, 727,1 s); `b97c33c1` PASS (27/11 00:20–00:31 UTC); `4f6acb02` PASS.
KIẾN TRÚC     : (1) 1 Cây Capability/MDB: `KT-417`+`KT-417b`+`KT-417d` p1/2 + F1/F2/F3; (2) Lõi Kép DONE;
                (3) User Space `KT-420` DONE, `KT-420b` chưa; (4) `KT-417c` **đóng ở tầng mã** cả 2 vòng
                hotfix; **nợ bằng chứng chạy** còn: MAJOR-2 (nhánh `Switched` từ cửa `syscall` chưa từng
                chạy — pha core chỉ dùng `int 0x80`) và MAJOR-1b (một `syscall` **sau** `Exit`); đề xuất
                gắn `KT-424` #0 (không cấp ID mới). (5) **W9 DoD** phải viết lại: `driver_files` = 23 mục;
                `Frame.{MapIo,UnmapIo}`/`IrqControl`/`IrqHandler`/`IOSpace` còn `unsupported`; (6) `vifast-ktest`
                3 arch DONE. **Audit**: `test` G7/G8/G9 **đã vá**; còn G1 (4 runner sản phẩm mồ côi), G2
                (`hardening-max.sh` mồ côi), G5 (F3 pin tay), G10 (3/5 bất biến syscall_fast); `harness` F7/F8/F12
                **đã vá**; `docs` QUEUE lệch 6 task + DoD W9 + `DECISIONS.md:28` — **Agent 0**.
TASK          : **389** — DONE 380 / PARTIAL 1 (`KT-417d`, 3/5 → 1/2) / READY 0 / BLOCKED 8 (sau khi Agent 0
                dán khối §1 của `agent0-so-sach-2026-11-28.md`: 6 task thực tế đã DONE nhưng QUEUE còn
                `READY`/`BLOCKED` — `KT-417b`, `KT-418c`, `KT-408d`, `KT-408e`, `KT-420`, `KT-417c`).
                Lead **không** sửa `QUEUE.md` (AGENTS §3/§8).
ĐANG CHẠY    : (không — máy đang rảnh để chạy nặng). **Kế tiếp ngay**: (a) **chạy nặng bù trên HEAD mới**
                theo §6.3 vì kernel/runner đổi: `soak-x86_64 600`, `soak-aarch64 --smoke`, `hardening-max.sh`,
                product 3 arch, `l1-tier --tier full`, Miri — bằng chứng cũ (`JOURNAL/heavy-20261127/`,
                `nightly/nightly-20261127-020807-44e60e67/`) **không tái dùng được**;
                (b) `mutants --all` đã chạy **36/30 KILLED/0 SURVIVED/6 E_WRONG-REASON** (`JOURNAL/mutants-2026-11-28/`):
                6 mục đều `ktest-riscv64` bị cắt ở ngân sách 600 s (ma trận 27 boot khi đột biến làm treo) —
                **nợ**: chạy lại riêng 6 mục với `--timeout 2400` để có KILLED thật;
                (c) Agent 0 trả lời 5 câu (gói sẵn); (d) **đóng MAJOR-1b/2** bằng 1 boot `via=syscall`.
MÁY (sim)     : `size_of::<Machine<…>>()` = **172.424** B [V1:ĐO @`b97c33c1`] / trần **184.832** / pin
                **196.608**. Các thay đổi 28/11 chỉ chạm `plat-x86_64`/`kbin-x86_64`/runner/`v2/user/runtime`
                ⇒ kỳ vọng **không đổi**, nhưng **chưa đo lại** trên `5f48f92a` (không coi là đã kiểm).
CHẠY NẶNG     : (lịch sử — **đã lạc hậu so với HEAD**) 27/11 00:44–02:10 UTC trên `44e60e67`:
                soak x86_64 600/600 lỗi=0, aarch64 smoke 2/2, hardening-max PASS 4 chân, product 3 arch PASS
                (10/10, 11/11), `l1-tier full` 400k PASS; Miri PASS 7261 s (`a0f1f2e7`). Từ 28/11 chưa chạy lại
                lượt nặng nào trên cây đã hợp nhất ⇒ **không** được coi là bằng chứng nghiệm thu.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
