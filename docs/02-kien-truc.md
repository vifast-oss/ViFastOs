# 02. Kiến trúc tổng thể (bản bare-metal hiện tại)

> Cập nhật 2026-11-11. Mô tả này thay cho bản cũ nói về `vifast-sim` /
> `vifast-core` / `manifests` — những thứ **không còn** trong cây hiện tại
> (chỉ còn trong `python-model/` và `legacy/` dưới dạng tham chiếu lịch sử).

## 1. Các tầng thật

```
┌──────────────────────────────────────────────────────────────┐
│ User space: root (supervisor) · server (VFS) · client (shell) │
│             pager · /bin/vf (multicall: uname/uptime/free/…)   │
├──────────────────────────────────────────────────────────────┤
│ vifast-user: wrapper syscall, layout user, slot/untyped        │
══════════════════ ranh giới user/kernel (ecall/svc/int 0x80) ═══
│ vifast-kernel: object · cap · IPC · sched · VM · ramfs          │
│                (portable, no_std, không unsafe)                 │
├──────────────────────────────────────────────────────────────┤
│ vifast-runtime: boot sequence, syscall dispatch, timer/         │
│                 preemption, context switch dùng chung           │
│                 trait Platform (BSP contract) + trait Arch      │
├──────────────────────────────────────────────────────────────┤
│ vifast-platform-*: trap frame, MMU, timer, IRQ, SMP, user entry│
│ vifast-arch-*: UART, clock, IRQ mask, shutdown, port I/O       │
├──────────────────────────────────────────────────────────────┤
│ vifast-kernel-{riscv64,aarch64,x86_64-uefi}: entry asm + linker │
│ + ảnh user nhúng                                                │
└──────────────────────────────────────────────────────────────┘
   RISC-V64 (OpenSBI)   AArch64 (QEMU virt)   x86_64 (UEFI/OVMF)
```

Thiết bị: `vifast-virtio` (block/net/GPU/input/RNG, mmio v2) cho RISC-V và
AArch64; x86 dùng ATA PIO. `vifast-fs` là filesystem dùng chung; `vifast-elf`
parse/validate ELF; `vifast-pte` encode/decode PTE 3 kiến trúc;
`vifast-acpi` đọc RSDP/MADT cho x86 SMP; `vifast-maint` mailbox shootdown
dùng chung; `vifast-fsutil` là tool host tạo/kiểm ảnh FS.

## 2. Ranh giới phần cứng

`vifast-kernel` **chỉ** biết trait `vifast_arch::Arch` (trap frame, hart id,
MMIO copy, page-table ops…). Toàn bộ phần còn lại nằm ở `Platform`
(`vifast-runtime`), do từng port cài:

```rust
pub trait Platform: Arch + Console {
    type Context: Copy + 'static;
    const USER_CODE_VA: usize;
    const USER_DATA_VA: usize;
    const ELF_MACHINE: u16;

    fn init_platform(info: PlatformInfo);
    fn init_kernel() -> &'static mut Kernel;
    fn set_kernel(kernel: *mut Kernel);
    fn kernel() -> &'static mut Kernel;

    fn data_page() -> &'static mut [u8];      // data untyped
    fn metadata_page() -> &'static mut [u8];  // metadata untyped
    fn boot_page() -> &'static mut [u8];      // object khởi động, không vào cspace
    fn vspace_root() -> u64;                  // bảng trang gốc tĩnh (chỉ kernel)
    fn switch_root(root_pa: u64, asid: u16) -> bool;
    fn page_table_ops() -> &'static dyn PageTableOps;
    fn tick_hz() -> u32 { 100 }
    fn staging_release_safe() -> bool { false }
    // … trap frame, IRQ controller, user copy, enter user, SMP
}
```

Vì vậy: `vifast-kernel` **không** biết địa chỉ UART, page size thật, số cấp
bảng trang, hay vector ngắt. Đổi port không sửa kernel.

## 3. Bản đồ crate (25 crate)

| Nhóm | Crate | Vai trò |
|---|---|---|
| ABI | `vifast-abi` | Syscall (58), mã lỗi, quyền, object type, Message, layout user |
| Kernel | `vifast-kernel` | Object/capability/IPC/scheduler/VSpace/ramfs/checks; `no_std` |
| Runtime | `vifast-runtime` | Boot, dispatch syscall, timer/preempt, `Platform` trait |
| Paging | `vifast-pte` | Encode/decode/index PTE Sv39, AArch64 4-level, x86 PML4 |
| ELF | `vifast-elf` | Parse/validate ELF64 PT_LOAD, W^X, overlap |
| FS | `vifast-fs`, `vifast-fsutil` | VFS v3 journal; tool host format/migrate/inspect |
| Driver | `vifast-virtio` | VirtIO-MMIO: block, net, GPU, input, RNG |
| Platform | `vifast-acpi`, `vifast-maint` | ACPI/MADT; mailbox shootdown dùng chung |
| Arch primitives | `vifast-arch-{riscv64,aarch64,x86_64}` | UART/PL011/COM1, clock, IRQ mask, shutdown |
| BSP | `vifast-platform-{riscv64,aarch64,x86_64}` | Trap, MMU, timer, IRQ, SMP, user entry |
| Binary kernel | `vifast-kernel-{riscv64,aarch64,x86_64-uefi}` | Entry asm + linker + ảnh user nhúng |
| User | `vifast-user` | Wrapper syscall cho 3 arch + layout chung |
| Tasks | `vifast-root`, `vifast-server`, `vifast-client`, `vifast-bin` | Root supervisor, VFS server, shell/demo, ảnh multicall `/bin/vf` |

Quy tắc phụ thuộc: đi từ ngoài vào trong (`platform → runtime → kernel → abi`);
`vifast-kernel` **không bao giờ** phụ thuộc platform/arch cụ thể.

## 4. Luồng điển hình

**Call/Reply:** client `Call(ep, msg)` → kernel tra cap (quyền W) → server đang
`Recv` thì chuyển message, client `BlockedReply`, server ghi `caller = (client, epoch)`;
server `ReplyRecv` → kiểm `epoch` còn khớp mới đánh thức client.

**Ngắt:** thiết bị → BSP `deliver_irq` → signal notification → driver (user) đang
`Recv` trên notification nhận `LABEL_NOTIFICATION` → xử lý → `IrqAck`, kernel mở che.

**Page fault:** MMU fault → kernel gửi fault như `Call` tới fault endpoint →
pager `FrameMap` vào VSpace của task → `TcbResume`; task chạy lại đúng lệnh.

**Exec:** shell gửi request FS → root nạp ELF từ `/bin/vf` vào VSpace mới, mint cap
cần thiết qua `TcbGrantCap`, đặt `argc/argv` qua `TcbSetArgs` → `TcbResume`.

## 5. Nguyên tắc thiết kế (còn đúng)

1. **Object chỉ tham chiếu qua `ObjRef { index, gen }`** (generational arena);
   huỷ object thì `gen` tăng, mọi ref cũ tự vô hiệu.
2. **Object sinh từ Untyped do user giữ** — kernel không có heap chung.
3. **Cơ chế ở kernel, chính sách ở user** (fault → supervisor; page fault → pager;
   ngân sách CPU → SchedContext).
4. **Mọi thao tác có cận trên** (revoke có preemption point, destroy theo watermark).
5. **Lỗi trả về, không panic**; `panic!` chỉ cho bất biến nội bộ bị phá.
