# Gói duyệt Agent 0 — 2026-11-28 (W8/W9 + sổ sách)

Tài liệu này gom **3 báo cáo read-only** do sub-agent soạn trên `main @5f48f92a` (sau T2 hợp nhất
`81/81`). Agent 0 chỉ cần: (A) dán các khối `QUEUE.md`/`DECISIONS.md` ở tệp `01`/`02`, và
(B) trả lời **5 câu** dưới đây. Không có thay đổi `spec/` nào bị chặn bởi gói này.

## 0. Hiện trạng cổng (để dán kèm `D2-264`)

- `HEAD = 5f48f92a` — **T2 PASS 81/81**, `# gate-wall 662554 ms`, 2026-11-28 15:20:07–15:31:13 UTC
  (`v2/agent/JOURNAL/t2-20261128/`), lần đầu in `# measure-key: commit/build-inputs/dirty/untracked/
  toolchain/seeds` (F12).
- 3 nhánh hợp nhất trong T2 này: hotfix `KT-417c` v1 (`c73eb77a`), hotfix vòng 2 (`4d6dd9e1`),
  hạ tầng đo (`1211181c`), test-gap (`27b51d8f`). T1 từng nhánh `82/68/14/0`
  (`JOURNAL/t1-20261128/`).
- `mutants --all --timeout 600`: **36 chạy / 30 KILLED / 0 SURVIVED / 6 E_WRONG-REASON**
  (`JOURNAL/mutants-2026-11-28/`). 6 mục đều `ktest-riscv64` bị cắt ngân sách — **nợ**, chưa phải
  bằng chứng xanh; lý do + cách đóng ghi trong README cùng thư mục.

## 1. Năm câu Agent 0 phải trả lời (chặn W8/W9)

| # | Câu hỏi | Khuyến nghị của sub-agent | Tệp |
|---|---|---|---|
| 1 | **Tách `KT-417d` p2/2 → `KT-417e/f/g/h`?** (11 h `[ƯL]`, **0 chạm `spec/`**, thứ tự bắt buộc e→f→g→h, 0 % chồng tệp trừ bàn giao khai báo `vm/**`/`invoke.rs`) | **Có** — khối dán 5 dòng `QUEUE` ở `02` §2 | `02` |
| 2 | **Cho tái neo 2 pin byte?** Bảng VM tối thiểu **+2.284 B** vượt pin `172.544` (slack 120 B); trần `184.832`/pin `196.608` **không nới** | **Có** — tái neo `172.544→184.832` + `kt408_tests` về cỡ mới đo; giữ chứng minh thu hồi 12.288 B. **Nếu không, `KT-417f` bất khả thi** | `02` §6, `03` §1 |
| 3 | **`VmObjMeta`: bảng riêng hay gộp?** | Bảng riêng **20 B × 256 = 5.120 B** (A3), giữ `Meta == 48`, tổng `≈179.828 B [ƯL]`, headroom 5.004 B; 96 phần tử là số của `[limits.plat]` **đã xoá** | `03` §1 |
| 4 | **`BootCapLayout` v2: 3 hay 4 trường? 17 `EP_*`: (a) hay (b)?** | **3 trường** (`boot_abi=2`, 6 B, 1 profile) + **(a)** bỏ khỏi `[boot_caps]`, giữ `proc_abi.rs` legacy + ratchet sunset `len(EP_NAMES) ≤ 17` | `03` §2–§3 |
| 5 | **DoD W9 viết lại (đo được) + đồng bộ QUEUE?** | Theo khối ở `01` §3: `driver_files` **23→15→10→0** theo task + `kernel_service_files` đóng băng **2** mục; bỏ nhãn "0 % chồng tệp" → **"chồng tệp có chủ, đo bằng `git diff --name-only`"**; `Frame.{MapIo,UnmapIo}`/`Irq*`/`IOSpace` là **tiền đề** `KT-419/419b` | `01` §1/§3 |

Kèm theo: **`KT-417c` = `DONE` có điều kiện** + `D2-264` (khối dán ở `01` §2), và **đồng bộ 13 dòng
`QUEUE.md`** (6 task thực tế đã `DONE` nhưng QUEUE còn `READY`/`BLOCKED`, `KT-417d` ghi
`PARTIAL 1/2`, 1 dòng ghi chú vận hành, 5 dòng DoD W9).

## 2. Nợ bằng chứng đang mở (không cần quyết, nhưng phải biết)

1. **MAJOR-2** (`hotfix-review.md`): nhánh `Switched` **từ cửa `syscall`** chưa từng chạy — pha core
   chỉ dùng `int 0x80`; **MAJOR-1b**: chưa có một `syscall` **sau** `Exit`. Đề xuất gắn `KT-424` #0,
   điều kiện đóng đo được: 1 boot in `# core resume … via=syscall` + `gs_in=` cả hai cửa + ca âm.
2. **Chạy nặng trên HEAD mới** đang được thực hiện lại (bằng chứng `JOURNAL/heavy-20261127/` đạt trên
   `44e60e67` — **không** tái dùng được cho `5f48f92a`, §6.3).
3. **6 mutant `ktest-riscv64`** cần `--timeout 2400` để có `KILLED` thật (ma trận 27 boot/đột biến).

## 3. Nguồn

- `01-so-sach-QUEUE-D2-DoD.md` — QUEUE sync 13 dòng, `D2-264`, DoD W9, khối `STATE`, rủi ro.
- `02-tach-KT-417d-dong-QUEUE.md` — 4 task con + khối dán, phát hiện chặn **tái neo 2 pin**.
- `03-tra-loi-ky-thuat-VmObjMeta-BootCap-EP.md` — 3 quyết định kỹ thuật + số `[ƯL]` + cách đo.

Tất cả là **đề xuất read-only** của sub-agent trên `5f48f92a`; Lead đã kiểm lại các commit/cổng dẫn
trong từng tệp. Không tệp nào trong gói này sửa `spec/`, `QUEUE.md`, `DECISIONS.md`.
