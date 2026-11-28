# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-29 01:40 JST / 2026-11-28 16:40 UTC (Lead: T2 PASS @`cc405ead`; đóng 2 MAJOR bằng chứng `KT-417c`) ===
HEAD          : `cc405ead` — **T2 PASS 81/81** (`# gate-steps registered=81 ran=81 skipped=0 failed=0`,
                `# gate-wall 674858 ms`; **16:27:56–16:39:13 UTC**; `# measure-key: commit=cc405ead
                build-inputs=b15fc2f60a7d436b dirty=0 untracked=114 toolchain=1.100.0 seeds=1000`).
                Hợp nhất thêm nhánh `KT-424 #0` (`0e42e3f0`: pha core gọi `Tcb.Suspend(Defer)` bằng
                **`syscall`** thay `int 0x80`; kernel in `# umode: switched via=syscall nr=… entry=0x…
                gs=0x…`; runner `check_switched_syscall` + ca âm (mm); test `core_phase_must_switch_via_syscall`).
                ⇒ **đóng 2 MAJOR bằng chứng** của `hotfix-review.md`: MAJOR-2 (nhánh `Switched` từ cửa
                `syscall` **đã chạy thật**) và MAJOR-1b (syscall **sau** `Exit`: dòng switched nằm sau
                `# umode: exit`, `gs` khớp `gs_scratch`). T1 `82/68/14/0` 654 s (runner x86_64 `47/47`).
PUSH          : `origin/main` = `cc405ead` sau commit bằng chứng này.
KẾ HOẠCH      : [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`D2-259`..`D2-264`). **Chờ Agent 0**
                (gói đã đóng gói trong repo: `reports/GOI-DUYET-AGENT0-2026-11-28/README.md`):
                (1) tách `KT-417d` p2/2 → `KT-417e/f/g/h` (11 h `[ƯL]`, 0 chạm `spec/`, thứ tự e→f→g→h);
                (2) **tái neo 2 pin byte** (bảng VM +2.284 B vượt pin `172.544`; trần/pin **không nới**) —
                nếu không, `KT-417f` bất khả thi; (3) `VmObjMeta` bảng riêng 20 B×256 (A3, `[ƯL]`≈179.828 B);
                (4) `BootCapLayout` v2 3 trường + 17 `EP_*` (a); (5) DoD W9 đo được (`driver_files` 23→15→10→0,
                "chồng tệp có chủ"). `KT-419`/`KT-419b` chờ `KT-417d` DONE.
CỔNG          : **T2 PASS 81/81 @`cc405ead`** (mới nhất, 674,9 s, `measure-key`). Trước: `5f48f92a` PASS 81/81
                (15:20–15:31 UTC, 662,6 s); `82502f7d` PASS (11:28–11:40 UTC, 727,1 s); `b97c33c1` PASS.
                Lần T2 này là lượt **+1 trên ứng viên bàn giao** (§6.3) vì đầu vào đổi (kernel + runner);
                lý do ghi trong commit `[KT-00]` bằng chứng.
KIẾN TRÚC     : (1) 1 Cây Capability/MDB: `KT-417`+`KT-417b`+`KT-417d` p1/2 + F1/F2/F3; (2) Lõi Kép DONE;
                (3) User Space `KT-420` DONE, `KT-420b` chưa; (4) `KT-417c` **đóng cả tầng mã và bằng chứng
                chạy** (v1 X1/X2/X3 + v2 đo GS + `KT-424 #0` `Switched` qua `syscall`); (5) **W9 DoD** phải
                viết lại: `driver_files` = 23 mục; `Frame.{MapIo,UnmapIo}`/`Irq*`/`IOSpace` còn `unsupported`;
                (6) `vifast-ktest` 3 arch DONE. **Audit**: `test` G7/G8/G9 **đã vá**; còn G1 (4 runner sản phẩm
                mồ côi), G2 (`hardening-max.sh` mồ côi), G5 (F3 pin tay), G10 (3/5 bất biến syscall_fast);
                `harness` F7/F8/F12 **đã vá**; `docs` QUEUE lệch 6 task + DoD W9 + `DECISIONS.md:28` — Agent 0.
TASK          : **389** — DONE 381 / PARTIAL 1 (`KT-417d`, 3/5 → 1/2) / READY 0 / BLOCKED 7 (sau khi Agent 0
                dán khối §1: 6 task thực tế DONE nhưng QUEUE còn `READY`/`BLOCKED`). Lead **không** sửa QUEUE.
ĐANG CHẠY    : (không). **Kế tiếp ngay**: (a) **chạy lại nhóm nặng bị ảnh hưởng bởi `KT-424`** trên `cc405ead`
                (`soak-x86_64 600`, `soak-aarch64 --smoke`, `hardening-max`, product 3 arch); **tái dùng**
                `l1-tier full` @`2d9135f3` với lý do: `diff 2d9135f3..cc405ead` chỉ chạm
                `v2/kernel/vifast-kbin-x86_64/{coretest,umode}.rs` + `v2/test/boot/run-x86_64.sh` + test/doc —
                **không** chạm `v2/sim`, cấu hình, seed, `l1-tier.sh` (§6.3 "chỉ sửa ... không ảnh hưởng phép kiểm");
                (b) chạy lại **6 mutant `ktest-riscv64`** với `--timeout 2400` để đóng nợ mutants;
                (c) Miri + `nightly` còn nợ trên HEAD cuối; (d) Agent 0 trả lời 5 câu (gói trong repo).
MÁY (sim)     : `size_of::<Machine<…>>()` = **172.424** B [V1:ĐO @`b97c33c1`] / trần **184.832** / pin
                **196.608**. `KT-424` chỉ chạm chương trình ring 3 + dòng log, **không** chạm `Machine` ⇒
                kỳ vọng không đổi; **chưa đo lại** trên `cc405ead` (không coi là đã kiểm).
CHẠY NẶNG     : **28/11 15:33–16:14 UTC trên `2d9135f3`** (HEAD trước `KT-424`): hardening-max **PASS**,
                product 3 arch **PASS** (11/10/11), soak-aarch64 smoke **2/2** (rc=1 theo hợp đồng SMOKE),
                soak-x86_64 **600/600 lỗi=0**, `l1-tier full` **400k PASS** (1395 s) — `JOURNAL/heavy-20261128/`.
                Nhóm chịu ảnh hưởng bởi `KT-424` **đang chạy lại** trên `cc405ead`; `l1-tier full` tái dùng có
                lý do (ghi ở mục ĐANG CHẠY). Bằng chứng 27/11 trên `44e60e67` **hết hiệu lực** (§6.3).
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
