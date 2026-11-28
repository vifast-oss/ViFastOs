# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-29 04:50 JST / 2026-11-28 19:50 UTC (Lead: T2 @`f5f39f6d` + mutants 36/36 KILLED; chạy Miri) ===
HEAD          : `f5f39f6d` — **T2 PASS 81/81** (`# gate-steps registered=81 ran=81 skipped=0 failed=0`,
                `# gate-wall 679383 ms`; 18:06:00–18:17:22 UTC; `JOURNAL/t2-20261128/t2-gate-fast-f5f39f6d.log`).
                Vá thêm so với `cc405ead`: (a) **khoá build ảnh ktest** (`ktest-build.sh`): `mkdir -p` thư mục
                cha của khoá — bản sao worktree/mutant không có `target/` làm `mkdir` trả ENOENT và hàm chờ
                đủ 900 s **trước khi build** (đây là nguyên nhân thật của 6 mutant "treo", **không phải** ma
                trận chậm); (b) journal + README mutant ghi nguyên nhân.
PUSH          : `origin/main` = `f5f39f6d` + commit bằng chứng này (push ngay sau commit).
KẾ HOẠCH      : [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`D2-259`..`D2-264`). **Chờ Agent 0**
                (gói trong repo: `reports/GOI-DUYET-AGENT0-2026-11-28/README.md`): (1) tách `KT-417d` p2/2 →
                `KT-417e/f/g/h` (11 h `[ƯL]`, 0 chạm `spec/`, thứ tự e→f→g→h); (2) **tái neo 2 pin byte**
                (bảng VM tối thiểu +2.284 B vượt pin `172.544`; trần `184.832`/pin `196.608` **không nới**) —
                nếu không, `KT-417f` bất khả thi; (3) `VmObjMeta` bảng riêng 20 B×256 (A3, `[ƯL]`≈179.828 B);
                (4) `BootCapLayout` v2 3 trường + 17 `EP_*` (a); (5) DoD W9 đo được (`driver_files` 23→15→10→0,
                "chồng tệp có chủ"). `KT-419`/`KT-419b` chờ `KT-417d` DONE.
CỔNG          : **T2 PASS 81/81 @`f5f39f6d`** (mới nhất, 679,4 s, `# measure-key commit=f5f39f6d
                build-inputs=b15fc2f60a7d436b`; trùng `build-inputs` với `cc405ead` ⇒ bằng chứng nặng đã đo
                trên `cc405ead` **áp dụng được**, lý do: chỉ thêm `v2/test/boot/lib/ktest-build.sh` + docs).
                Trước: `cc405ead` PASS (674,9 s); `5f48f92a` PASS (662,6 s); `82502f7d` PASS (727,1 s).
                Lượt T2 này là **#4 trong 24 h** — lý do kỹ thuật: vá hạ tầng đo dùng chung 5 runner, cần T2
                chứng nhận ma trận `ktest.sh` trên cây hợp nhất + đóng nợ mutant; không retry lấy xanh.
KIẾN TRÚC     : (1) 1 Cây Capability/MDB: `KT-417`+`KT-417b`+`KT-417d` p1/2 + F1/F2/F3; (2) Lõi Kép DONE;
                (3) User Space `KT-420` DONE, `KT-420b` chưa; (4) `KT-417c` **đóng tầng mã + bằng chứng chạy**
                (X1/X2/X3 v1+v2, `KT-424 #0` `Switched` từ `syscall` + syscall sau `Exit`); (5) **W9 DoD**
                phải viết lại: `driver_files` 23 mục; `Frame.{MapIo,UnmapIo}`/`Irq*`/`IOSpace` còn
                `unsupported`; (6) `vifast-ktest` 3 arch DONE. **Audit**: `test` G7/G8/G9 đã vá; còn G1 (4 runner
                sản phẩm mồ côi), G2 (`hardening-max.sh` mồ côi), G5 (F3 pin tay), G10 (3/5 bất biến
                syscall_fast); `harness` F7/F8/F12 đã vá; `docs` QUEUE lệch 6 task + DoD W9 + `DECISIONS.md:28`
                — Agent 0.
TASK          : **389** — DONE 381 / PARTIAL 1 (`KT-417d`, 3/5 → 1/2) / READY 0 / BLOCKED 7 (sau khi Agent 0
                dán khối §1). Lead **không** sửa QUEUE (AGENTS §3).
ĐANG CHẠY    : **Miri** (`./v2/bin/v2 nightly --only-miri`, sau khi `mutants --all` xong 19:42 UTC) — kết quả
                điền sau. **Vừa đóng**: `mutants --all --timeout 900` @`f5f39f6d` = **36 chạy / 36 KILLED /
                0 survived / 0 wrong-reason / 0 stale** (84 m 33 s; `JOURNAL/mutants-2026-11-28/
                mutants-all-final-36-killed.log`) — lượt `--all` **sạch đầu tiên**. Kế tiếp: (a) chạy nặng
                **không cần lại** (build-inputs trùng `cc405ead`; chỉ harness+docs đổi); (b) Agent 0 trả lời
                5 câu; (c) nợ MINOR-6/8/10 + 4 NIT (doc/test, gộp một lượt sau).
MÁY (sim)     : `size_of::<Machine<…>>()` = **172.424** B [V1:ĐO @`b97c33c1`] / trần **184.832** / pin
                **196.608**. Chưa đo lại trên `f5f39f6d` (thay đổi chỉ harness/docs + kernel x86_64 ring-3
                test; kỳ vọng không đổi).
CHẠY NẶNG     : **28/11 15:33–16:14 UTC trên `2d9135f3`** + **16:40–16:58 UTC trên `cc405ead`**:
                hardening-max PASS, product 3 arch PASS (11/10/11), soak-aarch64 smoke 2/2 (rc=1 hợp đồng
                SMOKE), soak-x86_64 **600/600 lỗi=0**, `l1-tier full` **400k PASS** (1395 s) — `JOURNAL/
                heavy-20261128/`. `build-inputs` của `f5f39f6d` **trùng** `cc405ead` (b15fc2f6) ⇒ áp dụng
                được cho HEAD hiện tại. Bằng chứng 27/11 trên `44e60e67` **hết hiệu lực** (§6.3).
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
