<p align="center">
  <img src="resources/brand/vifastos-lockup-cream.png" alt="ViFastOs · microkernel kiểu seL4 viết bằng Rust" width="620">
</p>

# ViFastOs — microkernel nhỏ, viết bằng Rust

**Một hệ điều hành để học và nghiên cứu, chưa phải sản phẩm.** Kernel tối giản
kiểu seL4 (capability-based) viết bằng Rust `no_std`; driver, filesystem, shell…
chạy ở user space như tiến trình bình thường. Cùng một lõi kernel, ba cổng:
RISC-V64, AArch64 và x86_64 (UEFI). Hiện chạy tốt trên **QEMU**; chưa lên máy thật.

> **Mới đến?** Bạn chỉ cần 2 phút đọc phần dưới và 3 lệnh để thấy nó tự boot.

## Chạy thử

Yêu cầu: Rust stable + 3 target bare-metal, QEMU, và OVMF nếu chạy x86_64.

```bash
rustup target add x86_64-unknown-none aarch64-unknown-none riscv64gc-unknown-none-elf

./scripts/build.sh                      # host test + build cả 3 kiến trúc
python3 scripts/test-qemu.py --repeat 1 # boot thật 3 arch
```

Kết quả mong đợi (rút gọn):

```
ViFastOs booting on riscv64
...
SELFTEST PASS
...
SHELL READY
shell>
```

Muốn chạy đầy đủ mọi cổng (QEMU, persistence, 677 case âm, docs):

```bash
./scripts/check-all.sh
```

## Nó làm được gì?

| Hạng mục | Trạng thái |
|---|---|
| Boot + user mode trên 3 kiến trúc (OpenSBI / UEFI / QEMU virt) | ✅ |
| SMP 2 hart, preempt, IPC, capability, Untyped/retype | ✅ |
| VFS v3 có journal, hard link/symlink, fsck, persistence 7 phase | ✅ |
| Shell + `/bin/vf` (uname, uptime, free, ps, df…), exec ELF từ FS | ✅ |
| Pager user-space, fault chỉ treo tiến trình hỏng | ✅ |
| VirtIO: block, net (ARP/ICMP/UDP/DNS + TCP echo), GPU vẽ cơ bản, input, RNG | ✅ (RISC-V/ARM) |
| **389 host test · 677 case âm · 3/3 kiến trúc PASS** | ✅ |

**Chưa có:** compositor/framebuffer console, mạng đầy đủ (chưa socket API/TCP
stack), libc và ứng dụng thật, hardware IOMMU, máy thật, CI, kiểm chứng hình thức.
Còn một lỗi treo ngẫu nhiên ~0.3–1 % trên x86_64 (T33) đang được truy.

Bảng trạng thái chi tiết và số đo mới nhất: [`docs/00-hien-trang.md`](docs/00-hien-trang.md).

## Kiến trúc trong một khung

```
user space ── root (supervisor) · server (VFS) · client (shell) · /bin/vf
─────────────────────────── syscall (ecall / svc / int 0x80) ───────────
vifast-kernel   object · capability · IPC · scheduler · VSpace
vifast-runtime  boot · dispatch · timer · trait Platform
vifast-platform-*  trap · MMU · IRQ · SMP      vifast-arch-*  UART · clock
────────────────────────────────────────────────────────────────────────
RISC-V64            AArch64              x86_64 (UEFI)
```

Kernel **không biết** địa chỉ UART, page size hay vector ngắt — chi tiết nằm ở
tầng BSP. Giải thích đầy đủ: [`docs/02-kien-truc.md`](docs/02-kien-truc.md).

## Cấu trúc repo

```
crates/     25 crate: abi · kernel · runtime · arch-* · platform-* · fs · virtio …
scripts/    build.sh · test-qemu.py · check-all.sh · các cổng sinh/kiểm tài liệu
docs/       tài liệu sống, thiết kế tham chiếu và báo cáo lịch sử
```

## Tài liệu

| Bạn muốn… | Đọc |
|---|---|
| Hiểu dự án đang ở đâu | [`docs/00-hien-trang.md`](docs/00-hien-trang.md) |
| Xem bản đồ toàn bộ tài liệu | [`docs/README.md`](docs/README.md) |
| Đi tiếp từ đâu | [`docs/06-lo-trinh.md`](docs/06-lo-trinh.md) |
| Build, test, quy ước đóng góp | [`docs/05-port-rust-va-kiem-thu.md`](docs/05-port-rust-va-kiem-thu.md) |
| Chi tiết từng subsystem | [`docs/11-hien-tai/00-index.md`](docs/11-hien-tai/00-index.md) |

## Vì sao tên ViFast?

- **Vi** — Việt Nam: làm bởi người Việt, cho tiếng Việt (bộ gõ, giọng đọc, và
  hệ điều hành này).
- **Vi** — còn là *vi* trong *vi xử lý*: **nhỏ**. Kernel `no_std`, hạn mức tĩnh
  rõ ràng thay vì phình to.
- **Fast** — nhanh: Rust biên dịch ra mã máy native, không máy ảo, không GC.

Giải thích đầy đủ: [vifast.org/#ten-goi](https://vifast.org/#ten-goi).
ViFastOs **không liên kết** với VinFast hay bất kỳ công ty nào khác.

## Đóng góp

- Chạy `cargo fmt --all --check` và `./scripts/build.sh` trước khi mở PR.
- Đổi hành vi guest → cập nhật `scripts/harness_spec.py` và
  [`docs/11-hien-tai/09-testing.md`](docs/11-hien-tai/09-testing.md) cùng lúc.
- Đổi số test/lệnh shell/tài liệu → chạy các script `gen-*.py` tương ứng
  (xem [`docs/05-port-rust-va-kiem-thu.md`](docs/05-port-rust-va-kiem-thu.md)).

## Giấy phép

Mã nguồn theo **MIT HOẶC Apache-2.0** — chọn một: [`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE). Logo, wordmark và tài nguyên thương hiệu
**không** thuộc phạm vi hai giấy phép này — xem [`TRADEMARKS.md`](TRADEMARKS.md).
