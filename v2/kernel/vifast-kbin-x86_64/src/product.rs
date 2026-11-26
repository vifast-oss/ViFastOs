// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340a** (D2-08) — nhánh **ảnh sản phẩm** của `vifast-kbin-x86_64`.
//!
//! # Vì sao tách bằng **feature `ktest`** (và vì sao nhánh này trả `!`)
//!
//! Trước KT-340a, `vifast-kbin-x86_64` là **một** ảnh vừa sản phẩm vừa harness: `kmain` in
//! `TAP version 13` + `1..34` rồi chạy 34 ca kiểm — trái D2-08 ("test kernel không nằm trong boot
//! sản phẩm"). Ảnh sản phẩm ở đây build **không** feature `ktest`; `kmain` rẽ vào
//! [`boot`] **trước** dòng TAP đầu tiên. Vì [`boot`] trả `!`, toàn bộ mã kiểm phía sau `kmain`
//! là **không thể tới** trong ảnh sản phẩm ⇒ bị MIR/LLVM loại khỏi ảnh. Đây không phải lời hứa:
//! runner `v2/test/boot/run-x86_64.sh` quét **bằng máy** (`strings | grep -c`) và có **ca âm**
//! (chính ảnh `ktest` phải đếm > 0).
//!
//! # Phạm vi ảnh sản phẩm ở bước này (ghi thẳng, không tô hồng)
//!
//! **Có**: long mode (PML4/PDPT/PD identity 1 GiB + MMIO 2..4 GiB — dựng ở `_start` asm),
//! COM1 115200 8N1, neo ảnh D2-80 (`# conf image:`), GDT/TSS/IDT **thật** (7 entry + `ltr` +
//! 256 ô), marker một lần, halt sạch (ACPI shutdown).
//!
//! **Chưa** (sẽ có ở `KT-340b`/`KT-340c`): LAPIC/IOAPIC/timer, U-mode ring 3, scheduler,
//! PCI/net/blk, W^X động. Các pha ấy hiện vẫn **chỉ** chạy trong ảnh `ktest` vì chúng gắn với
//! phép đo TAP; `KT-340c` sẽ dựng nhánh sản phẩm thật (vifast-core + IPC ring 3) và chuyển dần
//! phần khởi tạo thuần sang đây. Ghi rõ để không ai đọc báo cáo thành "sản phẩm đã đủ".

use core::arch::global_asm;
use core::fmt::Write;

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{CR0_PE, CR0_PG, CR4_PAE};
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::exc::FrameReturn;
use vifast_plat_x86_64::gdt;
use vifast_plat_x86_64::idt;
use vifast_plat_x86_64::serial;
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::wx::{self, ImageLayout, WxOptions, WxReport};

use crate::{Console, BOOT_MARKER, HALT_LINE};

// Symbol **linker** của ảnh (định nghĩa trong `linker.ld`) — W^X đọc ranh giới đoạn từ đây,
// **không** hardcode địa chỉ. Khai báo ở tệp này (không phải `main.rs`) để R1 của `main.rs`
// không phình thêm: cả hai ảnh đều biên dịch `product.rs`.
extern "C" {
    static __pd: u8;
    static __pt_kernel: u8;
    static __text_start: u8;
    static __text_end: u8;
    static __data_start: u8;
    static __kernel_end: u8;
}

/// `true` ⇔ ảnh này là ảnh **kiểm** (`--features ktest`). Ảnh sản phẩm = `false`.
///
/// `cfg!` là hằng biên dịch: nhánh còn lại bị loại ngay ở MIR, nên ảnh sản phẩm **không** mang
/// chuỗi/mã harness (không phải "tắt lúc chạy").
pub(crate) const KTEST: bool = cfg!(feature = "ktest");

/// Độ dài chuỗi `__user_msg` trong `global_asm!` của `main.rs` (`"ring3 says hi\n"` = 14 byte).
///
/// Ảnh sản phẩm **không** biên dịch `umode.rs` (D2-08) nên hằng này không còn lấy từ đó; giữ
/// **một** assert biên dịch để hai ảnh không lệch độ dài.
pub(crate) const USER_MSG_LEN: u64 = 14;
const _: () = assert!(USER_MSG_LEN == 14);

/// Khởi tạo **sản phẩm** tới marker rồi halt — không in TAP, không gọi hàm kiểm nào.
///
/// Trả `!` là **hợp đồng** với `kmain`: mọi dòng sau điểm gọi (toàn bộ harness) là unreachable
/// trong ảnh sản phẩm. Đổi kiểu trả về thành `()` sẽ làm harness quay lại ảnh — runner có ca
/// kiểm `strings` bắt được.
pub(crate) fn boot(
    boot_eax: u32,
    boot_ebx: u32,
    stack_top: u64,
    ist1_top: u64,
    divisor: (u8, u8),
    console: &mut Console,
) -> ! {
    // SeaBIOS in `Booting from ROM..` **không** kèm newline — xuống dòng trước để dòng
    // `# conf image:` bắt đầu **đầu dòng** (bộ đọc log dùng `^# conf image:`), cùng lý do ktest.
    let _ = writeln!(console);
    // D2-80 — neo ảnh: FNV-1a 64 trên `.text` đã nạp + độ dài, cùng định dạng với ảnh ktest để
    // `soak-x86_64.sh` dùng **một** bộ đọc log cho cả hai ảnh.
    let (image_build, image_text) = vifast_plat_x86_64::image::image_digest();
    let _ = writeln!(
        console,
        "# conf image: build={image_build:016x} text={image_text}"
    );
    // **KT-346** (P20) — danh tính bản dựng **ngay sau** `# conf image:`; `repro-build.sh` đọc
    // chuỗi này từ byte ảnh và harness so với dấu vân tay hiện tại của `v2/test/boot/build-rev.sh`.
    let _ = console.write_str(crate::BUILD_LINE);

    // Bằng chứng long mode đọc lại **thanh ghi**, không tin asm.
    let cr0 = asm::read_cr0();
    let cr3 = asm::read_cr3();
    let cr4 = asm::read_cr4();
    let efer = asm::read_msr(boot::LONG_MODE_EFER_MSR);
    let lma = (efer & boot::LONG_MODE_EFER_CHECK) != 0;
    let pg = (cr0 & CR0_PG) != 0;
    let pe = (cr0 & CR0_PE) != 0;
    let pae = (cr4 & CR4_PAE) != 0;
    let pvh_magic = asm::read_u32(boot_ebx as u64);
    let _ = writeln!(
        console,
        "# viFastOs x86_64: long mode cr0=0x{cr0:016x} cr4=0x{cr4:016x} cr3=0x{cr3:016x} efer=0x{efer:016x} lma={}",
        lma as u8,
    );
    let _ = writeln!(
        console,
        "# viFastOs x86_64: boot eax=0x{boot_eax:08x} ebx=0x{boot_ebx:08x} pvh_magic=0x{pvh_magic:08x} pvh_ok={}",
        (pvh_magic == boot::PVH_START_MAGIC) as u8,
    );
    let _ = writeln!(
        console,
        "# viFastOs x86_64: serial com1=0x{:03x} divisor={} lcr=0x{:02x}",
        serial::COM1_BASE,
        (divisor.0 as u16) | ((divisor.1 as u16) << 8),
        serial::LCR_8N1,
    );
    let _ = writeln!(
        console,
        "# viFastOs x86_64: identity map 0x0..0x{:x} pages={} pml4=0x{:x}",
        boot::IDENTITY_MAP_BYTES,
        boot::PD_ENTRIES,
        cr3 & !0xFFF,
    );

    // Marker **chỉ** in khi long mode thật sự bật (giữ nguyên luật của ảnh ktest: ảnh hỏng không
    // được có marker).
    if !(lma && pg && pe && pae) {
        let _ = writeln!(
            console,
            "PANIC: long mode chưa bật (lma={} pg={} pe={} pae={})",
            lma as u8, pg as u8, pe as u8, pae as u8,
        );
        asm::halt(1)
    }

    // GDT/TSS/IDT **thật** — nền sản phẩm, không phải phép kiểm: bất kỳ exception nào sau đây
    // phải vào handler của kernel thay vì triple-fault. `RSP0`/`IST1` = stack boot hiện tại.
    tss::init(0, stack_top, ist1_top);
    gdt::install(0, tss::base(0));
    idt::install();
    let _ = writeln!(console, "# viFastOs x86_64: product gdt/idt ok=1");

    // **KT-341** — ảnh sản phẩm **có shell**: `Machine` (NOLOAD `.corespace`) + loader ELF +
    // rig `int 0x80` + ring KT-69; shell chạy CPL3, `Exit(0)` ⇒ in `# shell:` + marker + halt.
    // Không feature `product-shell` ⇒ đường tối giản cũ (marker ngay sau GDT/IDT) không đổi.
    #[cfg(feature = "product-shell")]
    {
        let _ = writeln!(
            console,
            "# viFastOs x86_64: product shell core+procsrv=shell drive (D2-233)"
        );
        crate::shellpath::start_product(console)
    }

    // Marker đúng **một** lần + halt sạch (ACPI shutdown ⇒ QEMU exit 0).
    let _ = console.write_str(BOOT_MARKER);
    let _ = writeln!(console, "{HALT_LINE}");
    asm::halt(0)
}

/// **KT-340a** (D2-08) — handler exception của ảnh **sản phẩm**: chẩn đoán rồi halt, **không**
/// nuôi đường kiểm (không `sched`/`smp`/`net`). Handler đầy đủ (TAP) chỉ có trong ảnh `ktest`
/// (`main.rs`, `#[cfg(feature = "ktest")]`): nhờ vậy **mọi** mã harness mất **gốc tham chiếu**
/// ở ảnh sản phẩm và bị loại — không phải "tắt lúc chạy".
#[cfg(not(feature = "ktest"))]
#[no_mangle]
pub extern "C" fn exc_dispatch(
    vec: u64,
    err: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    _frame_rsp: u64,
) -> FrameReturn {
    // **KT-406** — COM1 RX: ngắt phần cứng thật (không phải exception). Vét `RBR` vào ring, EOI
    // rồi **resume đúng chỗ bị ngắt**. Không in dòng nào (mỗi byte gõ mà in log là ồn + chậm);
    // bằng chứng là `# uart: irq=… rx=…` in lúc kết thúc pha shell.
    #[cfg(feature = "product-shell")]
    if vec == crate::comirq::COM1_VECTOR as u64 {
        let _ = crate::comirq::on_irq();
        return FrameReturn::resume(rip);
    }
    // Spurious LAPIC (vector 255) **không** phải lỗi: bỏ qua để một lần nhiễu không giết shell.
    if vec == exc::VEC_SPURIOUS as u64 {
        return FrameReturn::resume(rip);
    }
    let mut console = Console;
    let _ = writeln!(
        console,
        "# exc: vec={vec} err=0x{err:x} rip=0x{rip:x} cs=0x{cs:x} rflags=0x{rflags:x}"
    );
    if vec == exc::VEC_PF as u64 {
        let _ = writeln!(console, "# exc: cr2=0x{:x}", asm::read_cr2());
    }
    asm::halt(1)
}

/// **KT-340a** (D2-08) — stub `syscall_dispatch` của ảnh **sản phẩm tối giản** (không
/// `product-shell`): stub asm `int 0x80` luôn được liên kết, nên ảnh nào cũng phải có **một**
/// định nghĩa ký hiệu này. Ảnh có shell (`KT-341`) dùng dispatcher thật ở `shellpath`; ảnh tối
/// giản không có ring 3 ⇒ in chẩn đoán rồi halt, không nuôi TAP.
#[cfg(not(any(feature = "ktest", feature = "product-shell")))]
#[no_mangle]
pub extern "C" fn syscall_dispatch(
    nr: u64,
    _a0: u64,
    _a1: u64,
    _mi: u64,
    _mr0: u64,
    _mr1: u64,
    _cs: u64,
    _rip: u64,
) -> vifast_plat_x86_64::umode::SyscallRet {
    let mut console = Console;
    let _ = writeln!(
        console,
        "# syscall: nr={nr} — ảnh sản phẩm chưa có ring 3 (KT-340c), halt"
    );
    asm::halt(1)
}

/// **KT-340a** — trang `.trampoline` **rỗng** cho ảnh sản phẩm.
///
/// `linker.ld` có `ASSERT(__trampoline_end - __trampoline_start == 0x1000)` (KT-276: trang SIPI
/// đúng một trang, 64 KiB-aligned). Ảnh sản phẩm **không** biên dịch `smp.rs` (D2-08) nên không
/// còn khối `global_asm!` sinh trang ấy; giữ **một** trang `hlt` để bất biến bố cục của linker
/// vẫn đúng — **không** phải mã trampoline thật (không có `INIT`/`SIPI` trong ảnh sản phẩm).
#[cfg(not(feature = "ktest"))]
global_asm!(
    ".section .trampoline, \"ax\"",
    ".globl __trampoline_start",
    "__trampoline_start:",
    "hlt",
    ".skip 4095, 0x90",
    ".globl __trampoline_end",
    "__trampoline_end:",
);

/// **KT-340a** — bật **W^X kernel** (`.text` RX / `.rodata` R / `.data` RW NX) + `CR0.WP`, gọi
/// **một lần** trong `kmain` **trước** mọi pha khác (cả ảnh sản phẩm và ảnh kiểm).
///
/// Trả [`WxReport`] đọc lại **từ bảng trang** để pha TAP dùng — không trả hằng mong đợi.
pub(crate) fn wx_init() -> WxReport {
    let layout = wx_layout();
    let opts = WxOptions {
        // Đột biến (i) `no-wx`: tắt bảo vệ (ảnh ktest TAP 35/36 phải ĐỎ).
        protect: !cfg!(feature = "no-wx"),
        // Đột biến (ii) `wx-data-exec`: bỏ NX trên `.data/.bss` (TAP 35 phải ĐỎ).
        nx_data: !cfg!(feature = "wx-data-exec"),
        // Trang trampoline AP chỉ tồn tại trong ảnh kiểm (smp.rs bị cfg gỡ ở ảnh sản phẩm).
        ap_trampoline: cfg!(feature = "ktest"),
    };
    // SAFETY: gọi **một lần** trong `kmain`, sau `_start` (paging đã bật) và **trước** mọi ghi
    // vào `.text`/`.rodata`; `__pt_kernel`/`__pd` là trang 4 KiB trong `.bss` của ảnh.
    let pt = core::ptr::addr_of!(__pt_kernel) as u64;
    let pd = core::ptr::addr_of!(__pd) as u64;
    wx::enable(pt, pd, &layout, &opts)
}

/// Hình ảnh của **lần boot này** —ranh giới đoạn lấy từ symbol linker.
pub(crate) fn wx_layout() -> ImageLayout {
    ImageLayout {
        text_start: core::ptr::addr_of!(__text_start) as u64,
        text_end: core::ptr::addr_of!(__text_end) as u64,
        data_start: core::ptr::addr_of!(__data_start) as u64,
        kernel_end: core::ptr::addr_of!(__kernel_end) as u64,
    }
}

/// Đọc lại quyền **hiện tại** từ bảng trang đang hoạt động — nguồn số liệu của TAP W^X.
pub(crate) fn wx_report() -> WxReport {
    wx::report(&wx_layout())
}
