# 00. Hiện trạng triển khai

> **Cập nhật 2026-11-11, HEAD `3008601`** (r307 + PR #14, PR #15).
> Đây là **trang trạng thái chuẩn** của bản bare-metal đang chạy.
> `docs/01`–`docs/08`, `docs/03-thanh-phan/`, `legacy/`, `python-model/` là
> **thiết kế tham chiếu lịch sử** — nhiều chi tiết trong đó chưa/không được
> port sang kernel này (CNode nhiều cấp, CDT, `vifast-sim`, profile manifest).
> Xem bản đồ tài liệu ở [`docs/README.md`](README.md).

## 1. Đã chạy được (cả 3 kiến trúc)

| Hạng mục | RISC-V64 | AArch64 | x86_64 |
|---|---|---|---|
| Boot | OpenSBI + QEMU `virt` | QEMU `virt` | OVMF/UEFI |
| Console sớm | 16550 | PL011 | COM1 |
| Self-test portable | PASS | PASS | PASS |
| User mode | U-mode `sret` | EL0 `eret` | ring3 `iretq` |
| Syscall vào kernel | `ecall` | `svc #0` | `int 0x80` |
| Timer interrupt + preempt | SBI timer | GICv2 + generic timer | PIC/PIT + LAPIC |
| SMP 2 hart + scheduler | có (HSM) | có (PSCI) | có (ACPI MADT + LAPIC) |
| Per-hart ready queue + work stealing | có | có | có |
| BKL ticket + maintenance IPI + per-VA TLB | có | broadcast | có (còn hở AP→BSP, xem §4) |
| Root/server/client VSpace riêng | có | có | có |
| IPC Call/Reply/ReplyRecv, notification, cap transfer | có | có | có |
| Untyped con + BootInfo + TCB idle/restart/bind | có | có | có |
| `ObjectDestroy` + reclaim Untyped (9 loại object) | có | có | có |
| VFS v3 journal + thư mục + phân quyền | có | có | có |
| Hard link / symlink / double-indirect / O_APPEND / O_EXCL / O_DIRECTORY / STAT_PATH | có | có | có |
| Block cache write-back + `fsync`/`BlockFlush` | có | có | có |
| Persistence 7 phase | có | có | có |
| Software IOMMU + DMA map, W^X chéo CPU/DMA | có | có | có |
| Sleep deadline (BlockedSleep) | có | có | có |
| Console input IRQ + shell block trên notification | có | có | có |
| CNode phẳng + CNodeInstall | có | có | có |
| Demand paging + user-space pager server (4 client) | có | có | có |
| TCB destroy/reap + fault-slot identity | có | có | có |
| User fault suspend (không panic) | có | có | có |
| Exec ELF từ FS + argc/argv + ủy quyền cap cho con | có | có | có |

**Driver / thiết bị (VirtIO-MMIO v2 — chỉ RISC-V & AArch64):**

| Thiết bị | Trạng thái | Marker trong gate |
|---|---|---|
| Block | có, polling | `FS DISK OK`, `BLOCK OK` |
| Net | có: ARP, ICMP echo, UDP, DNS A, TCP listen/echo tối thiểu | `VIRTIO NET PING REPLY`, `VIRTIO TCP LISTEN OK` |
| GPU | có: init + draw đơn giản | `VIRTIO GPU DRAW OK` |
| Input | có: bàn phím virtio → shell | `VIRTIO INPUT READY` |
| RNG | có | `VIRTIO RNG READY` |

x86_64 **không dùng VirtIO**: đĩa qua ATA PIO, boot qua UEFI. Không có PCI/MSI ở cả 3 port.

## 2. Số liệu nền (đọc từ code, không phải số cũ)

- **58 syscall** đánh số `Send = 0` … `TcbGrantCap = 57`, 58/58 có handler thật.
- Object table **168 object**, **24 TCB**, **4 hart** (scheduler mới chứng minh trên 2 hart),
  **128 cap root / 16 cap task**, CNode phẳng 16 slot, **64 mapping/VSpace**, **8 fd**.
- **388 host test** (kernel 261 gồm 2 `#[ignore]`, fs 52, elf 20, virtio 11, abi 16,
  pte 12, acpi 8, arch 8) + **677 case âm** harness + persistence 7 phase.
- Cổng QEMU đo trên 3 arch: standard, `test-destroy-smp`, `test-panic-hart1`,
  `test-skip-tlb-ipi`, `test-check-boundary[-mutation]`, `--tcp-check`,
  `--input-check`, `--gpu-check`, `--interactive`.
- Giới hạn FS: ảnh 4 MiB (8192 block × 512 B), 64 inode, tên 28 B, path 128 B.
- Header virtio-net **12 byte** (modern, vì đã đàm phán `VIRTIO_F_VERSION_1`);
  dùng 10 byte (legacy) sẽ mất 2 byte đầu khung — lỗi đã vá ở r307.

## 3. Lệnh kiểm tra

```bash
./scripts/build.sh                    # host test + build 3 arch + biến thể feature
python3 scripts/test-qemu.py --repeat 3
python3 scripts/test-qemu.py --feature test-destroy-smp --repeat 5
python3 scripts/test-persist.py
python3 scripts/test-harness-spec.py  # 677 case âm
python3 scripts/gen-docs.py --check && python3 scripts/check-docs.py
./scripts/check-all.sh                # toàn bộ cổng
```

Kỳ vọng: 3/3 arch PASS; persistence 7 phase × 3 PASS; host test 388/388;
`DESTROY OK (frames=100, vspaces=10, reused=100, leaked=0)`; `SLEEP OK`;
`PREEMPT OK`; `shell>`; docs sạch.

## 4. Việc còn mở (đã biết, chưa sửa)

1. **T33** — treo sau `shell>` (thiếu `PREEMPT OK`) trên x86_64, tỉ lệ ~0.3–1 %;
   chưa chứng minh được nguyên nhân, cần soak QEMU dài. Đã chứng minh không do T32.
2. **x86 shootdown AP→BSP**: hook `poll_maintenance` chỉ chạy cho hart 1, nên yêu cầu
   maintenance gửi tới BSP chờ hết 5M vòng rồi bỏ → TLB BSP có thể cũ. (`flush_all_user`
   đã cố sửa ở r299 nhưng chưa trọn.)
3. **PCID**: CR3 đã đúng bits 11:0; nhưng `flush_tagged_va` cho ASID không active dùng
   `reload_cr3()` — với `CR4.PCIDE = 1` không xoá PCID khác (chỉ lộ trên KVM/`-cpu max`).
4. **Gate chưa bật `--strict`**: retry vẫn được tính xanh.
5. **T-IPC**: r307 kết luận "boot đỏ" là do cổng `test-exit` thoát đúng lúc client đang
   chạy user; **r308 đã vá** (hoãn exit một tick khi task còn `Running`), 0/80 boot thiếu marker.

## 5. Hạn chế bring-up cần nhớ

- Trap stack: RISC-V qua `sscratch`, AArch64 `SP_EL1`, x86_64 `TSS.RSP0`.
- x86_64 identity map 0..1 GiB; code RX, data/stack NX; AP đã online.
- Hart 0 có idle TCB; hart phụ dùng idle loop của platform.
- Driver block: VirtIO-MMIO (RISC-V/AArch64) và ATA PIO (x86), **polling**.
- Chưa kiểm chứng máy thật; mọi số đo là QEMU.

## 6. Chưa có

- Hardware IOMMU (VT-d/SMMU) — chỉ có mô hình software.
- CNode nhiều cấp/guard; UART IRQ hiện chỉ route BSP (hart 0).
- Compositor/framebuffer console, ViFastKey, job control thật.
- Mount/pager tổng quát nạp file thật; barrier phần cứng thật; driver ngoài QEMU.
- Khoá mịn/x2APIC; scheduler mới test 2 hart.
- Allocator vật lý toàn cục (reclaim nằm trong từng Untyped).
- CI (GitHub không dùng), formal verification (Kani/Verus).
- Userland đầy đủ: libc, shell hệ thống, dynamic linking, ứng dụng thật.
- Lớp tương thích Linux.

## 7. Tài liệu liên quan

- [`docs/README.md`](README.md) — bản đồ tài liệu (sống / tham chiếu / lịch sử).
- [`docs/11-hien-tai/00-index.md`](11-hien-tai/00-index.md) — 18 tài liệu thành phần theo subsystem.
- [`docs/11-hien-tai/09-testing.md`](11-hien-tai/09-testing.md) — hợp đồng marker và cách chạy gate.
- [`ViFastOs-architecture-summary.txt`](../ViFastOs-architecture-summary.txt) — bản tổng hợp tự sinh.
