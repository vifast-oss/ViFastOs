// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **sản phẩm** riscv64 cho K3 (KT-44, KT-45).
//!
//! Không chứa chuỗi/đường dẫn `ktest` (D2-08); ảnh test là bin `vifast-kbin-ktest`
//! (`src/ktest.rs`, feature `ktest`). KT-45 cài `stvec` **direct mode** ngay đầu boot và in lại
//! `stvec` để chứng minh vector thật trên phần cứng; trap thật được kiểm trong ảnh test.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::{read_trap_vector, Riscv64Virt};

/// Nền tảng QEMU `virt` — zero-sized, dựng được trong `static`.
static PLATFORM: Riscv64Virt = Riscv64Virt;
/// Một handler panic dùng chung cho cả ảnh sản phẩm và ảnh test (`vifast-plat-common`).
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Được `_start` (trong `vifast-plat-riscv64`) gọi sau khi đặt `sp`/`tp`/`sscratch`.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..1\n");
    PLATFORM.put_str("# k3 image=product (kernel thật trên QEMU virt, không phải model)\n");
    // KT-45: cài vector trap direct-mode trước mọi việc khác; in lại stvec để runner thấy giá trị
    // thật (không phải hằng số viết tay trong tài liệu — QD §9 Q12). Trap sống trước MMU để lỗi
    // mapping lúc bật `satp` được dump thay vì treo im lặng.
    PLATFORM.set_trap_vector(vifast_plat_riscv64::trap_vector());
    let stvec = read_trap_vector();
    // KT-46: dựng bảng Sv39 từ arena RAM **xong** mới bật `satp` + `sfence.vma`, rồi in địa chỉ
    // thật (root/arena/guard) do lần boot này sinh ra.
    let mmu = mmu::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mmu: satp=0x{:x} mode={} root=0x{:x} tables={} used={} arena=0x{:x}-0x{:x} kernel_end=0x{:x}",
            mmu.satp,
            mmu.satp >> 60,
            mmu.root,
            mmu.tables,
            mmu.used,
            mmu.arena_start,
            mmu.arena_end,
            mmu.kernel_end
        );
        let _ = writeln!(
            w,
            "# mmu map: high=0x{:x} physmap=0x{:x} guard=0x{:x}-0x{:x} guard_none=1 alias={}",
            mmu.high_base, mmu.physmap_base, mmu.guard_start, mmu.guard_end, mmu.alias_ok as u8
        );
        let _ = writeln!(
            w,
            "# hart={} tick_hz={} time={} stvec=0x{:x} direct={}",
            PLATFORM.hart_id(),
            PLATFORM.tick_hz(),
            PLATFORM.now(),
            stvec,
            stvec & 0b11 == 0
        );
    }
    // Ảnh sản phẩm chỉ TAP 1 dòng, nhưng MMU phải thật: satp mode 8, đọc lại được, alias ghi/đọc
    // khớp. Sai ⇒ PANIC (run.sh coi là đỏ), không giả xanh.
    if mmu.satp >> 60 != 8 || !mmu::mmu_enabled() || !mmu.alias_ok {
        PLATFORM.put_str("PANIC: mmu init product\n");
        PLATFORM.halt(1)
    }
    PLATFORM.put_str("ok 1 - k3 boot\n");
    PLATFORM.put_str("# k3 halt\n");
    PLATFORM.halt(0)
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
