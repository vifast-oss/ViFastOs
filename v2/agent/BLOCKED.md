# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-25 (`D2-260`)**: **không còn task nào bị chặn kỹ thuật cần chờ quyết định**;
> gói 5 mục sau W1/đầu W2 đã được Agent 0 duyệt (`KT-403` DONE, `KT-401`/`KT-407`/`KT-414` `READY`,
> tách `KT-401b`, `KT-402b`, `KT-414b`). Mọi nợ kiến trúc dưới đây đều đã có task sở hữu trực tiếp
> trong [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`KT-400..KT-424` + 3 task con = 28 task).
> Toàn bộ mục lịch sử vòng 1 → vòng 25 nằm nguyên văn tại
> [`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) — **đọc-only**.

## 1. Mục BLOCKED kỹ thuật đang mở

**Không có chặn thiết kế** (21 task trong `QUEUE.md` chỉ `BLOCKED` **theo DAG**; riêng `KT-417` mở `spec/` chờ `APPROVED` của Agent 0 cho `KT-416`).
- **Tiền đề hạ tầng máy đo cho `KT-414b` (`uefi-aarch64`)**: người điều phối chạy 1 lần `rustup target add x86_64-unknown-uefi aarch64-unknown-uefi --toolchain 1.100.0` trước khi khởi chạy `KT-414b` (không cài mạng trong lúc chạy task).

## 2. Nợ kiến trúc có chủ (đều đã gán task trong `KT-400..KT-424` + `KT-401b/402b/414b`)

| Nợ | Trạng thái hiện tại | Task sở hữu (`D2-259`/`D2-260`) |
|---|---|---|
| 2 Object/Cap Layer tách đôi (`vifast-core` vs `plat-common::obj`) | `[limits.core]` & `[limits.plat]` tồn tại song song trong `spec/kernel.toml` | **`KT-416` → `KT-417`** |
| Driver & dịch vụ chạy trong kernel (`B1`) | Ratchet `B1.1` đã khoá (`KT-403` DONE); `3.567` LOC ở `kbin-riscv64` + 3 `plat-*` → `vifast-net` | **`KT-403` (DONE) → `KT-419` → `KT-421..KT-423`** |
| `v2/user/` chưa là Cargo workspace (`B4`) + nợ `N-7` P20 | `v2/user/` chưa chạy `clippy`/`cargo test`; 2 thư mục build lệch `.text` digest | **`KT-404`** |
| `Tcb.Exec(path)` + 16 hằng `EP_*` hardcode + `ping`/`ifconfig` | Đang dùng bảng slot cứng; `exec` chưa qua syscall ABI `Tcb.Exec` | **`KT-416` → `KT-420` → `KT-421`** |
| `CNode.Mutate` (3) / `CNode.Rotate` (6) | Đang trả `NotSupported` (18); quyết định ở RFC kiến trúc tổng thể | **`KT-416` → `KT-417`** |
| Đọc **DTB** (`rv`/`a64`) & **ACPI** (`x64`) + harness thật 3 arch | `BootInfo` v1 xong (`KT-402` DONE); còn nối runner `aarch64 -dtb` + `x86_64 fw_cfg/table-loader` | **`KT-402` (DONE) → `KT-402b`** |
| W^X 4 KiB `riscv64` + `.corespace` NX đủ 3 arch | `x86_64` `.corespace` ở `PD[1]` chưa `PTE_NX`, `riscv64` đang map 2 MiB RWX | **`KT-401b`** *(D2-260 mục 2)* |
| Ring trace 32 B luôn bật trong kernel (`D2-09`) + `pmemsave` | Sim + `vifast-trace` xong (`KT-403`); nối vào `PerHartState`/`.corespace` tránh sửa 2 lần | **`KT-408`** *(D2-260 mục 3)* |
| Lỗ hổng `clippy -D warnings` cho `v2/kernel/` (`exclude = ["kernel"]`) | `vifast-core` còn 15 cảnh báo `clippy`; `v2 check` chưa chạy `clippy` lên `v2/kernel/` | **`KT-405` → `KT-409`** *(D2-260 mục 3)* |
| `vifast_sync::Mailbox<N>` tầng 2 trên `IpiMailbox` | Giữ `IpiMailbox` tầng 1 không mất wake; gắn tầng 2 `Mailbox<N>` + cờ `OVERFLOW` | **`KT-415`** |
| `v2 mutants` thật + bỏ 43 `mut-*` | **Đã xong (`KT-403` DONE tại `d39b86b9`)**; lượt chạy full 19 mục còn lại do Lead chạy ở Giờ Cao điểm | **`KT-403` (DONE)** |
| `exec`: ca lỗi-muộn `ObjKind::Untyped` + ca SMP thật | Giải quyết khi `UntypedStore` + SMP 4 hart 3 arch hoàn tất | **`KT-410`, `KT-412`, `KT-413`** |

## 3. Giới hạn có văn bản (không phải chặn)

- **`KT-75`** — ca âm `flush` là **mô hình cache** (QEMU ghi thẳng file ảnh), không phải mất dữ liệu thật;
  `ktest-persist` không nằm trong matrix vì một pha là hai boot trên cùng ảnh đĩa. Chi tiết: archive §KT-75,
  `v2/docs/k6-persist.md`.
- **`KT-78`** — harness QEMU phụ thuộc `perl` cho `setpgrp` (fail-closed nếu thiếu); `loadavg` chỉ là
  lưới phụ, bộ đếm QEMU mới là detector chính. Chi tiết: archive §KT-78, `v2/docs/k6-harness.md` §7.

## 4. Chuyển tiếp theo lịch (không phải chặn)

- Kế hoạch phát hành `KT-348` (`v2.0`) đã được **huỷ bỏ (`superseded`, `D2-259`)** theo chỉ đạo của người điều phối.
- Chương trình Đại tu Kiến trúc (`KT-400..KT-424` + `KT-401b/402b/414b`, 28 task): đã `DONE` 4 task (`KT-400`, `KT-402`, `KT-403`, `KT-406`), mở `READY` 3 task (`KT-401`, `KT-407`, `KT-414`); chỉ **`KT-417`** (cửa sổ mở `spec/` duy nhất) đòi `APPROVED` riêng của Agent 0 sau `KT-416`.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
