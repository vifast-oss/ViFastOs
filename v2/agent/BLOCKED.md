# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-26 (`D2-261`)**: **không còn bất kỳ task nào bị chặn kỹ thuật hay chờ duyệt**;
> toàn bộ gói RFC `KT-416` + `KT-416b` (`review/agent0/GOI-ABI-KIEN-TRUC.md`, 1 + 8 mục) đã được
> Agent 0 **APPROVED** (`D2-261`), mở khoá ngay **`KT-417`**, **`KT-413c`** và **`KT-408c`** (`READY`,
> 3 slot song song 0 % chồng tệp). `KT-417` tách sạch thành `KT-417`/`417b`/`417c`, `KT-419` tách thành
> `KT-419`/`419b`, và ratchet `B1.1` **giữ cứng** (`virtio-rng` đi User Space `rngsrv` ở `KT-423`).
> Toàn bộ chương trình Đại tu Kiến trúc nằm tại [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)
> (**42 task** trong dải `KT-400..KT-424`: **30 DONE**, **2 READY**, **10 BLOCKED** thuần theo DAG).
> Toàn bộ mục lịch sử vòng 1 → vòng 25 nằm nguyên văn tại
> [`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) — **đọc-only**.

## 1. Mục BLOCKED kỹ thuật đang mở

**Không có chặn thiết kế hoặc chặn chờ duyệt** (toàn bộ 10 task `BLOCKED` trong `QUEUE.md` chỉ chờ phụ thuộc DAG: `KT-417b`, `KT-417c`, `KT-418c`, `KT-419`, `KT-419b`, `KT-420..KT-424`).

## 1b. Yêu cầu quyết định mới (2026-11-26, sau `KT-417` DONE)

| # | Việc cần quyết định | Bằng chứng | Đề xuất |
|---|---|---|---|
| **Q1** | **Tách `KT-417b`**: planner đo **6,5–7,5 h** (> ngân sách 3 h); bước "bảng VM vào `Machine`" là bước **duy nhất** đổi `size_of` | báo cáo planner 04:15 UTC (nội dung ghi ở `JOURNAL/kt417-spec-bootcaps.md` §6) | `KT-417b-1` ≤ 2 h (dời `plat-common::obj` 2.424 LOC → `vifast-core/src/vm/` + shim `aobj` + `[limits.plat]` đổi `source`; **0** đổi `size_of`) rồi `KT-417b-2` ≤ 3 h (nhập bảng + xoá `core_foreign` + `PageTable.{Map,Unmap}` + hợp nhất bất biến) |
| **Q2** | **Bảng VM nằm ở đâu**: `size_of::<Machine>()` = 184.712 / trần 184.832 (dư **120 B**); `mappings[64]` + `flushes[16]` + cửa sổ PA ≈ **+3,2 KiB** ⇒ **vỡ pin** nếu đưa hết vào `Machine` | `footprint.rs:187,272`, `budget-common.sh:54` | (a) đặt `mappings`/`flushes` trong `.corespace` NOLOAD qua handle ZST (tiền lệ `TraceRing`) — **không** nới pin; hoặc (b) giảm `MAX_MAPPINGS` (**cần D2**) |
| **Q3** | **Trần LOC `vifast-core` 25.000** (QD §3.15): hiện **21.868** + 2.424 (obj) = **24.292** ⇒ còn ~708 dòng cho toàn bộ mã+test mới | planner 04:15 UTC | Cho phép test/`ktest_*` không tính vào trần, hoặc nâng trần **có D2** |
| **Q4** | **Gia cố máy kiểm `[boot_caps]`** (reviewer F2/F3/F5/F6): khoá lạ trong `[boot_caps]` bị **bỏ im lặng**; không assert `size_of`/offset `BootCapLayout`; `Tcb.Exec` không nằm trong danh sách phải-giữ-`unsupported`; 13 chữ ký mới thiếu `errors = [...]`; `_proc_const` fail-open khi tệp vắng | reviewer 04:15 UTC | dồn vào cửa sổ `spec/` kế tiếp (`KT-420`) hoặc task con riêng |
| **Q5** | **`KT-419` phải có TAP cho Notification chết** (F1 đã vá ở core + host test; chưa có đường `Revoke` thật trên ảnh) | `JOURNAL/kt417-spec-bootcaps.md` §7 | thêm 1 TAP vào `ktest_capa`/`proc_net` của `KT-419` |
| **Q6** | **Phủ đột biến cho 4 method mới**: `mutants.toml` 33 mục, **0** mục cho `Mutate`/`Rotate`/`Bind`/`Unbind`; 4 delta `mutated/rotated/bound/unbound` trong `ktest_inv` **được tính nhưng không assert** | reviewer F7 | thêm 1–2 đột biến + assert 4 delta (cùng `KT-417b`/`KT-419`) |

## 2. Nợ kiến trúc còn lại (đều đã gán task có chủ trong `D2-259`/`D2-261`)

| Nợ | Trạng thái hiện tại | Task sở hữu (`D2-261`) |
|---|---|---|
| 2 Object/Cap Layer tách đôi (`vifast-core` vs `plat-common::obj` 2.424 LOC / 33 caller) | RFC `KT-416`/`416b` đã **APPROVED** (`D2-261`); `[limits.plat]` còn trong `spec/kernel.toml` | **`KT-417` DONE → `KT-417b`** |
| `x86_64` GDT `planned` + `syscall`/`sysret` (giữ `int 0x80` & `7C-r13`) | `[arch.x86_64.gdt.planned]` đã có trong `spec/kernel.toml`; tách độc lập đường găng Chặng 3 | **`KT-417c`** |
| 2.162 LOC harness `sched.rs` `x86_64` + 24 mục `# sched:` | Đã duyệt D2 đổi 24 mục kỳ vọng (`D2-261`); PCID giữ mặc định TẮT tới `HW1` | **`KT-413c` (READY)** |
| Lỗi `-cpu max` `VSpace` isolation (`rflags=0x10202 [TF=1]`) & hardening `-cpu max` | Lộ ở `KT-418b` §6.4; chạy sau `KT-413c` để 0 chồng tệp `x86_64`; **không** nới ratchet `B1.1` | **`KT-418c`** |
| 7 mục `E_WRONG-REASON` của `v2 mutants --all` + `Machine::now` luôn 0 trên `riscv64` + 5 `SLOT_EP_*` `netsrv` + `vifast-bench --bkl` | Phát hiện từ `KT-403`/`KT-408b`/`KT-404d`/`KT-415` — gom vào 1 task công cụ & hạ tầng | **`KT-408c` (READY)** |
| Driver & dịch vụ còn trong kernel (`B1`: 3 dep `vifast-net`, 23 tệp driver đóng băng) | Ratchet `B1.1` giữ cứng (`≤ 23`, cấm tăng); `virtio-rng` đẩy thẳng sang `rngsrv` U-mode | **`KT-419` → `KT-419b` → `KT-421..KT-423`** |
| 17 hằng `EP_*` hardcode + 8 `SLOT_*`/`LOOP_*` + `Tcb.Exec(elf_frame)` + `# build:` a64/x64 | Đã có máy kiểm `check-boot-abi.py` (`KT-404d`); `[boot_caps]` sinh ở `KT-417`, gỡ hardcode ở `KT-420` | **`KT-417` DONE → `KT-420`** |
| `CNode.Mutate` (3) / `CNode.Rotate` (6) & dispatch `Tcb.{Bind,Unbind}Notification` (13/14) | Đã duyệt `D2-261` (`rights?` đầy đủ cho `Mutate`, MCS refill giữ 1 đoạn) | **`KT-417` DONE @ `c57895df`** |
| Tranh chấp Big Kernel Lock 4 hart (`KT-415`: 0 % → 39,6 % → 93,5 %) + nối `HartTier2` | **Ngưỡng S3 đã kích hoạt**; làm ngay sau `KT-424` để tránh sửa khoá 2 lần trong Chặng 3 | **Hậu `KT-424` (#1 `S3`/`F1`)** |
| Nhánh chết `cfg!(feature="product-net")` ở `riscv64/build.rs:206` (`KT-404c`) | Giữ nguyên có chủ đích — biến mất hoàn toàn khi `KT-421` gỡ `net_svc`/`proc_net` | **`KT-421`** |

*(Các nợ cũ ở Chặng 0 & Chặng 1 đã **ĐÓNG 100 %**: dọn kho v1 `KT-400`, `BootInfo` v1 + DTB/ACPI 3 arch `KT-402/402b`, ratchet `B1.1` + `v2 mutants` + telemetry `KT-403`, workspace `v2/user/` + `vifast-kbuild` + 3 lockfile + `N-7` `KT-404/404c/404d`, W^X 4 KiB + `.corespace` NX 3 arch `KT-401/401b`, `CoreObserver`/`PerHartState`/Lõi Kép `KernelStore`/`UntypedStore`/gỡ `vifast-model` `KT-405/408/408b/410`, UART RX IRQ + `LD_PRELOAD` `KT-406`, tooling `KT-407`, HAL + `clippy` kernel `KT-409/409b`, `vifast-ktest` + gom 14 bin `KT-411/411b`, SMP 4 hart 3 arch + `N-3` + `Tier2Mailbox` `KT-412/413/413b/415`, UEFI `x64`/`a64` `KT-414/414b`, FPU/SIMD + `SMEP`/`SMAP`/`PAN`/`SUM` + `EntropyPool` `KT-418/418b`.)*

## 3. Giới hạn có văn bản (không phải chặn)

- **`KT-75`** — ca âm `flush` là **mô hình cache** (QEMU ghi thẳng file ảnh), không phải mất dữ liệu thật;
  `ktest-persist` không nằm trong matrix vì một pha là hai boot trên cùng ảnh đĩa. Chi tiết: archive §KT-75,
  `v2/docs/k6-persist.md`.
- **`KT-78`** — harness QEMU phụ thuộc `perl` cho `setpgrp` (fail-closed nếu thiếu); `loadavg` chỉ là
  lưới phụ, bộ đếm QEMU mới là detector chính. Chi tiết: archive §KT-78, `v2/docs/k6-harness.md` §7.

## 4. Chuyển tiếp theo lịch (không phải chặn)

- Kế hoạch phát hành `KT-348` (`v2.0`) đã được **huỷ bỏ (`superseded`, `D2-259`)** theo chỉ đạo của người điều phối.
- Chương trình Đại tu Kiến trúc ([`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md), cập nhật `D2-261`): đã **`DONE` 29/42 task** (Chặng 0 xong 100 %, Chặng 1 xong 100 %, Chặng 2 còn `KT-417`/`417b`/`417c`, `KT-413c`, `KT-418c`, `KT-408c`). Không còn cửa sổ nào bị treo chờ duyệt.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
