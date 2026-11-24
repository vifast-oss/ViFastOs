# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-24 (`D2-259`)**: **không còn task nào bị chặn kỹ thuật**; toàn bộ kế hoạch phát hành cũ
> (`v2.0`/`v2.1`/`KT-348`) đã huỷ bỏ, mọi nợ kiến trúc dưới đây đều đã có task sở hữu trực tiếp trong
> [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md) (`KT-400..KT-424`, 4 slot Đợt W1 đang `READY`).
> Toàn bộ mục lịch sử vòng 1 → vòng 25 nằm nguyên văn tại
> [`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) — **đọc-only**.

## 1. Mục BLOCKED kỹ thuật đang mở

**Không có** (`KT-404..KT-424` chỉ `BLOCKED` **theo DAG** trong `QUEUE.md`; riêng `KT-417` mở `spec/` chờ `APPROVED` của Agent 0 cho `KT-416`).

## 2. Nợ kiến trúc có chủ (đều đã gán task trong `KT-400..KT-424`)

| Nợ | Trạng thái hiện tại | Task sở hữu (`D2-259`) |
|---|---|---|
| 2 Object/Cap Layer tách đôi (`vifast-core` vs `plat-common::obj`) | `[limits.core]` & `[limits.plat]` tồn tại song song trong `spec/kernel.toml` | **`KT-416` → `KT-417`** |
| Driver & dịch vụ chạy trong kernel (`B1`) | `3.567` LOC ở `kbin-riscv64` + `plat-*` phụ thuộc `vifast-virtio`/`vifast-net` | **`KT-403` → `KT-419` → `KT-421..KT-423`** |
| `v2/user/` chưa là Cargo workspace (`B4`) + nợ `N-7` P20 | `v2/user/` chưa chạy `clippy`/`cargo test`; 2 thư mục build lệch `.text` digest | **`KT-404`** |
| `Tcb.Exec(path)` + 16 hằng `EP_*` hardcode + `ping`/`ifconfig` | Đang dùng bảng slot cứng; `exec` chưa qua syscall ABI `Tcb.Exec` | **`KT-416` → `KT-420` → `KT-421`** |
| `CNode.Mutate` (3) / `CNode.Rotate` (6) | Đang trả `NotSupported` (18); quyết định ở RFC kiến trúc tổng thể | **`KT-416` → `KT-417`** |
| Đọc **DTB** (`rv`/`a64`) & **ACPI** (`x64`) | `kmain(_dtb)` chưa parse bảng phần cứng thật | **`KT-402`** |
| `x86_64`: W^X **toàn ảnh** + `.corespace` NX 3 arch | Đã đóng `VSpace` động + TLB shootdown; W^X toàn ảnh + NX `.corespace` 3 arch | **`KT-401`** |
| `vifast_sync::Mailbox<N>` tầng 2 trên `IpiMailbox` | Giữ `IpiMailbox` tầng 1 không mất wake; gắn tầng 2 `Mailbox<N>` + cờ `OVERFLOW` | **`KT-415`** |
| `v2 mutants` thật + bỏ 43 `mut-*` | `nightly` tạm dùng `selfbreak` | **`KT-403`** |
| `exec`: ca lỗi-muộn `ObjKind::Untyped` + ca SMP thật | Giải quyết khi `UntypedStore` + SMP 4 hart 3 arch hoàn tất | **`KT-410`, `KT-412`, `KT-413`** |

## 3. Giới hạn có văn bản (không phải chặn)

- **`KT-75`** — ca âm `flush` là **mô hình cache** (QEMU ghi thẳng file ảnh), không phải mất dữ liệu thật;
  `ktest-persist` không nằm trong matrix vì một pha là hai boot trên cùng ảnh đĩa. Chi tiết: archive §KT-75,
  `v2/docs/k6-persist.md`.
- **`KT-78`** — harness QEMU phụ thuộc `perl` cho `setpgrp` (fail-closed nếu thiếu); `loadavg` chỉ là
  lưới phụ, bộ đếm QEMU mới là detector chính. Chi tiết: archive §KT-78, `v2/docs/k6-harness.md` §7.

## 4. Chuyển tiếp theo lịch (không phải chặn)

- Kế hoạch phát hành `KT-348` (`v2.0`) đã được **huỷ bỏ (`superseded`, `D2-259`)** theo chỉ đạo của người điều phối.
- Chương trình Đại tu Kiến trúc **`KT-400..KT-424`** mở `READY` ngay 4 slot Đợt W1 (`KT-400`, `KT-401`, `KT-402`, `KT-403`); chỉ **`KT-417`** (cửa sổ mở `spec/` duy nhất) đòi `APPROVED` riêng của Agent 0 sau `KT-416`.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
