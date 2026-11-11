# 06. Lộ trình

> Cập nhật 2026-11-11. Bản cũ mô tả "Giai đoạn 0: hiện tại, 27 test, vifast-sim
> chưa biên dịch" — đã lỗi thời nhiều tháng. Dưới đây là lộ trình tính từ trạng
> thái thật ở [`00-hien-trang.md`](00-hien-trang.md).

## Đã xong (đo được)

| Giai đoạn cũ | Trạng thái | Bằng chứng |
|---|---|---|
| 0–1. Logic kernel trên host (Python → Rust) | **Xong** | 388 host test; invariant checker; `no_std`, không heap cho object |
| 2. QEMU RISC-V 1 hart (boot, trap, user, timer, VirtIO) | **Xong** | `test-qemu.py` riscv64 PASS; shell, VFS, pager, persistence |
| 3. SMP (2 hart, BKL, IPI, shootdown) | **Xong mức demo** | `SMP OK`, `SMP SCHED OK`, `BKL STATS`; chưa chứng minh >2 hart |
| 2 mở rộng. AArch64 + x86_64/UEFI | **Xong** | 3/3 arch PASS cổng QEMU |

## Đang làm (2026-11)

1. **Ổn định T-IPC** — r307 kết luận "boot đỏ" là cổng `test-exit` thoát đúng lúc
   client đang chạy user; **r308 đã vá** (hoãn exit một tick khi task còn `Running`).
   Tiêu chí xong: `test-qemu.py --repeat 20` × 3 arch, 0 retry, không cần `--strict` tắt.
2. **T33** — treo `PREEMPT OK` trên x86_64 ~0.3–1 %. Tiêu chí: soak 300 boot x86
   liên tiếp 0 treo, hoặc chỉ ra nguyên nhân bằng trace.
3. **Hoàn thiện x86 TLB shootdown AP→BSP** (hook `poll_maintenance` chỉ chạy hart 1)
   và nhánh PCID (`invpcid` hoặc tắt PCID có kiểm soát).
4. **Follow-up audit r299** (đã merge PR #15): fs `inject_pending_write`,
   `set_file_block`, truncate `INODE_FREE`, đệm alias `exec`, 2 test yếu.

## Mốc kế tiếp (có tiêu chí)

### M1 — Gate nghiêm
- `check-all.sh` chạy `--strict` (retry = 0).
- CI tự động (thay GitHub Actions bằng script + máy chạy định kỳ).

### M2 — Driver chạy bằng IRQ
- VirtIO block/net/GPU dùng ngắt + notification thay polling.
- Đo: `VIRTIO IRQ OK` trên 3 arch; độ trễ syscall không tăng.

### M3 — Mạng dùng được
- TCP/IP tối thiểu đủ `connect/listen/read/write` (tự viết hoặc `smoltcp`).
- Socket API trong `vifast-user`; test echo + HTTP GET đơn giản.

### M4 — Filesystem lớn hơn
- Ảnh > 64 MiB, > 4096 inode; mount nhiều ảnh; pager nạp file thật từ FS.
- Test: persistence 7 phase giữ nguyên, thêm fuzz fsck.

### M5 — Desktop tối thiểu
- Framebuffer console → compositor đơn giản (1 cửa sổ) → input service.
- ViFastKey chạy như service user space (tách lõi gõ khỏi web).

### M6 — Máy thật / Space
- Bản RISC-V trên bo thật (hoặc FPGA) có UART + timer + đĩa.
- Watchdog, ảnh A/B, kiểm chứng lõi `unsafe` (Kani/Verus), soak dài.

## Rủi ro chính

| Rủi ro | Giảm thiểu |
|---|---|
| Driver/hệ sinh thái tốn công nhất | Bám VirtIO trước; dùng crate `smoltcp` khi cần |
| SMP/race khó tìm | Giữ BKL lâu hơn mức cần; soak nhiều arch; `loom` cho phần lock-free |
| Cổng QEMU nhạy tải máy (retry) | Chạy gate trên máy rảnh + `--strict`; tách flake khỏi lỗi thật |
| Tài liệu lệch code | `check-docs` + `gen-*` chạy trong gate; cập nhật cùng commit |
| Phạm vi quá rộng | Mỗi mốc có tiêu chí đo được; không mở mốc sau khi mốc trước chưa xanh |
