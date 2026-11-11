# 01. Tổng quan và mục tiêu

> Cập nhật 2026-11-11 (HEAD `3008601`). Đây là tài liệu **định hướng**;
> trạng thái chạy được nằm ở [`00-hien-trang.md`](00-hien-trang.md).

## 1. ViFastOs là gì

Một hệ điều hành viết từ đầu bằng Rust, lõi là **microkernel dựa trên
capability** theo hướng seL4. Kernel chỉ làm 5 việc:

1. Quản lý object và capability (ai được làm gì với cái gì)
2. IPC (chuyển message, báo hiệu)
3. Lập lịch (ai được chạy CPU, bao lâu)
4. Bộ nhớ ảo (page table, TLB)
5. Chuyển ngắt thành thông báo cho driver

Mọi thứ khác (driver, file system, pager, supervisor, shell) chạy ở user
space như tiến trình bình thường, có capability. Mạng, GPU, compositor,
bộ gõ ViFastKey là các dịch vụ **dự kiến** — hiện mới có driver mức thấp và
dịch vụ ở mức demo (xem `00-hien-trang.md`).

## 2. Hai hướng sản phẩm, một kernel

| | Desktop | Space |
|---|---|---|
| Mục đích | Máy tính cá nhân, máy trạm | Hệ thống an toàn sống còn |
| Tạo tiến trình lúc chạy | Có | Không |
| Cắm thiết bị lúc chạy | Có | Không |
| Ngân sách CPU (MCS) | Tuỳ chọn | Nên dùng cho task không quan trọng |
| Cách khoá | Không khoá | Boot xong xoá Untyped gốc, IRQ/sched control |

**Hiện trạng:** kernel có đủ cơ chế cho cả hai (Untyped, MCS budget/period,
quyền tối thiểu) nhưng **chưa có cơ chế "profile tĩnh" hoàn chỉnh**; bản đang
chạy là cấu hình desktop-demo. Không sửa kernel để "làm tĩnh" — tính tĩnh đạt
được bằng việc không còn ai giữ cap tạo object.

## 3. Các quyết định lớn và lý do

| Quyết định | Lựa chọn | Lý do | Cái giá |
|---|---|---|---|
| Kiến trúc kernel | Microkernel | Driver lỗi chỉ chết driver; kernel nhỏ dễ kiểm chứng | IPC tốn hơn gọi hàm |
| Mô hình quyền | Capability, không có root | Quyền tối thiểu theo thiết kế; thu hồi được | Khó tương thích POSIX |
| Bộ nhớ kernel | Không heap chung; object sinh từ Untyped do user giữ | Kernel không hết bộ nhớ vì một task xấu | User phải tự quản lý bộ nhớ |
| Chính sách | Nằm ở user space (supervisor, pager) | Kernel đơn giản, đổi chính sách không đổi kernel | Phải viết thêm dịch vụ user |
| ISA | RISC-V64 đầu tiên, rồi AArch64, x86_64 | ISA mở, hợp nghiên cứu; 3 port chứng minh tính portable | Mỗi port một BSP riêng |
| Ngôn ngữ | Rust, `no_std`; `unsafe` gom vào `vifast-arch-*`/`vifast-platform-*`/entry | An toàn bộ nhớ trong kernel | `unsafe` vẫn cần audit thủ công |
| Phương pháp | Host test → QEMU gate → soak; mọi marker có hợp đồng `harness_spec` | Chốt ngữ nghĩa trước khi tăng độ phức tạp | Cổng chạy lâu, nhạy tải máy |
| Đa lõi | BKL trước, khoá mịn sau | Đơn giản, đúng; seL4 cũng dùng | Syscall bị tuần tự hoá giữa các hart |

## 4. Phạm vi hiện tại

**Đã có (đo được bằng gate, xem `00-hien-trang.md`):**
capability install/copy/mint/derive/revoke + badge; CNode phẳng + CNodeInstall;
Untyped/retype/zero + ObjectDestroy + reclaim; IPC `Send/Call/Recv/Reply/ReplyRecv`,
notification, chuyển cap qua IPC có kiểm TOCTOU; scheduler ưu tiên + round-robin
theo hart, work stealing, preemption 100 Hz; MCS budget/period; Sleep; TcbWait/
TcbInfo/MemInfo/TimerInfo/ModuleRelease/TcbSetArgs/TcbGrantCap;Sv39 / 4-level
AArch64 / PML4 x86, W^X, ASID/PCID, first-fit VA, TLB shootdown theo VA; software
IOMMU + `IoMap/IoUnmap`; fault endpoint + pager user-space; VFS v3 journal,
hard link/symlink, double-indirect, block cache, `fsync`, fsck, persistence;
VirtIO block/net/GPU/input/RNG (mmio v2); ATA PIO (x86); shell + `/bin/vf`
multicall; exec ELF từ FS + argv; 388 host test + 677 case âm harness.

**Chưa có:**
máy thật; CNode nhiều cấp/guard; CDT đầy đủ; hardware IOMMU; khoá mịn thay BKL;
x2APIC; superpage; mạng đầy đủ (chưa có socket API, TCP stack thật); compositor/
framebuffer console; ViFastKey; libc/dynamic linking/ứng dụng thật; lớp tương
thích Linux; CI; formal verification.

## 5. Nguyên tắc làm việc

1. **Đo được mới gọi là xong.** Mỗi thay đổi phải có test host hoặc marker gate.
2. **Test là hợp đồng.** `harness_spec` giữ danh sách marker; đổi hành vi phải đổi
   hợp đồng và tài liệu cùng lúc.
3. **Cơ chế ở kernel, chính sách ở user.**
4. **Mọi syscall có thời gian giới hạn.** Thao tác dài phải có preemption point.
5. **`unsafe` gom một chỗ.** Kernel portable cấm `unsafe`; phần cứng nằm ở
   `vifast-arch-*` / `vifast-platform-*` / entry assembly.
