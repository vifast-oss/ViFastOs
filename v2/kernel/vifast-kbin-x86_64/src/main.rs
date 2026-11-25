// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh boot **x86_64** của ViFastOs — KT-257 (K5' bước 1) + KT-261 (K5' bước 2: GDT/TSS/IDT) +
//! KT-266 (K5' bước 3: APIC + timer) + **KT-270** (K5' bước 4: U-mode ring 3 + `int 0x80`) +
//! KT-274 (K5' bước 5: PCI + virtio-net) + KT-276 (K5' bước 6: SMP 2 hart) + **KT-278** (K5' bước
//! 7: SMP **4 hart** + **IPI** + **TSS per-hart**).
//!
//! # Đường boot
//!
//! ```text
//!   qemu-system-x86_64 -m 128M -nographic -kernel <elf> -no-reboot -smp 2
//!        │
//!        ├─ QEMU đọc **PVH note** (`.note.Xen`, type 18) ⇒ entry 32-bit = `_start`
//!        ├─ _start (32-bit protected mode): cli/cld → esp = __stack_top → zero .bss
//!        │    → dựng PML4/PDPT/PD (identity 1 GiB + **MMIO map 3..4 GiB**, 2 MiB/page)
//!        │    → CR4.PAE → CR3 → EFER.LME → CR0.PG|PE → lgdt (GDT tạm 3 entry)
//!        │    → ljmp 0x08:long_mode_entry
//!        ├─ long_mode_entry (64-bit): DS/ES/SS = 0x10 → rsp = __stack_top
//!        │    → kmain(eax, ebx, __stack_top, __ist1_top)
//!        ├─ kmain: COM1 115200 8N1 → TAP version/plan → in bằng chứng long mode
//!        │    → TSS (RSP0 + IST1) → GDT thật 7 entry + `ltr` → IDT 256 ô + `lidt`
//!        │    → TAP 1..7 (gdt/idt loaded + int3/ud2/#PF/#GP/unknown)
//!        │    → LAPIC (SVR/TPR) → IOAPIC (mask hết) → timer (LVT + initial count)
//!        │    → `sti` → TAP 8..11 (lapic id, ioapic, timer ticks, timer eoi) → `cli`
//!        │    → TSS.RSP0 = stack syscall riêng → `iretq` xuống ring 3 (CS=0x1B, SS=0x23, IF=1)
//!        │    ↓
//!        │  ring 3: ghi stack user → `int 0x80` DebugPutStr → handler in chuỗi user
//!        │    → TAP 12/13 + `# umode: ring3 ran nr=12 ret=14` → `iretq` về ring 3
//!        │    → `int 0x80` Exit → TAP 14 → PCI/net (TAP 15..17) → **SMP** (TAP 18..21)
//!        │    → **preemption** (TAP 22/23) → marker → halt
//!        └─ asm::halt: chờ LSR.TEMT (có trần) → outw(0x604, 0x2000) ⇒ QEMU thoát mã 0
//!
//! # KT-276 + KT-278 — SMP 4 hart, IPI, TSS per-hart (K5' bước 6 → bước 7)
//!
//! Sau TAP 17, [`smp::run`] copy trampoline (`0x8000`) rồi `INIT`+`SIPI` **tuần tự** từng hart
//! 1..3. Hart phụ chạy real mode → long mode, nạp TSS/GDT riêng (`ltr`), bật LAPIC của nó, `hlt`
//! chờ timer; BSP gửi IPI vector `0x30`. Cuối cùng bốn hart in `# smp: hart=N alive` (TAP 18..21).
//! **Không** scheduler/preemption, **không** `-cpu max`, **không** `syscall`/`sysret`.
//!
//! # Vì sao có **hai** header
//!
//! QEMU 11.1.3 `-kernel` cho x86 **không** dùng multiboot2: `hw/i386/multiboot.c` chỉ tìm magic
//! multiboot **1** (`0x1BADB002`) và từ chối ELF64 (`"Cannot load x86-64 image, give a 32bit
//! one."`). Đường QEMU thật sự nhận ELF64 là **PVH** (`hw/i386/x86-common.c:load_elfboot`): nó
//! đọc note `"Xen"` type 18 trong `PT_NOTE` để lấy entry 32-bit. Vì vậy ảnh mang:
//!
//! - **multiboot2 header** (đúng đặc tả, 24 byte, đầu `.text`) — để GRUB/multiboot2 cũng nạp được;
//! - **PVH note** — đường `qemu-system-x86_64 -kernel <ELF>` thực tế dùng.
//!
//! `_start` **không** phụ thuộc giao thức: nó chỉ ghi lại `eax`/`ebx` để `kmain` in bằng chứng.
//!
//! # KT-261 — GDT/TSS/IDT + handler exception
//!
//! - **GDT thật** 7 entry (null, kernel code/data, user code/data, TSS 16 byte) nằm ở
//!   [`vifast_plat_x86_64::gdt`]; `_start` vẫn dùng **GDT tạm** 3 entry để vào long mode (cùng
//!   selector `0x08`/`0x10`, cùng giá trị descriptor) rồi `kmain` nạp bảng thật.
//! - **TSS** ([`vifast_plat_x86_64::tss`]): `RSP0` = đỉnh stack kernel, `IST1` = stack riêng cho
//!   `#DF` (vector 8), `I/O map base = 104` (không bitmap).
//! - **IDT** ([`vifast_plat_x86_64::idt`]): **256** ô × 16 byte, mọi ô `P=1`; 14 vector có stub
//!   riêng, còn lại trỏ `isr_unknown` (đẩy sentinel [`exc::VEC_UNKNOWN`]).
//! - **Handler** [`exc_dispatch`]: in `# exc: vec=… err=0x… rip=0x… cs=0x… rflags=0x…` (thêm
//!   `# exc: cr2=0x…` cho `#PF`), ghi lại bằng chứng, rồi **resume** (`iretq`) nếu exception là ca
//!   kiểm đã arm, hoặc **halt sạch** nếu không (không bao giờ quay lại chỗ lỗi).
//!
//! # Vì sao ảnh **không** có `unsafe`
//!
//! Luật R5 (`v2/bin/v2 rules`) chỉ cho `unsafe` trong `kernel/vifast-plat-*`/`boot/`. Ảnh này chỉ
//! dùng `global_asm!` (macro **không** cần `unsafe`) cho `_start` 32-bit + `long_mode_entry`; mọi
//! lệnh đặc quyền (`in`/`out`/`mov crN`/`rdmsr`/`lgdt`/`lidt`/`ltr`/`hlt`) và **stub exception**
//! nằm ở [`vifast_plat_x86_64::asm`] — crate `kernel/vifast-plat-*` mà D2-17 cho phép `unsafe`.
//! Nhờ vậy ảnh giữ đúng luật R5 mà vẫn có đủ đường boot + xử lý exception.
//!
//! # Phạm vi (D2-203, KT-261; D2-204, KT-266; D2-207, KT-270)
//!
//! **Đã có**: GDT/TSS/IDT + handler exception (KT-261); LAPIC (ID/version/SVR/TPR), IOAPIC
//! (ID/version + mask toàn bộ redirection), LAPIC timer periodic + `EOI` (KT-266); **U-mode ring
//! 3** — trang mã/stack user `US = 1`, `iretq` xuống CPL3, `int 0x80` (gate `DPL = 3`) → handler
//! → `iretq` về ring 3, `Exit` kết thúc (KT-270); **preemption** — hai tác vụ kernel, timer
//! vector 32 chuyển ngữ cảnh bằng cách đổi `rsp` sang frame tác vụ kế (KT-286).
//!
//! **Chưa**: IPC từ ring 3, nhiều process, U-mode task, nhiều hart schedule, priority,
//! sleep/wake, `-cpu max`, `syscall`/`sysret`, soak 600 boot. **Không** tuyên bố K5' đạt (D2-149).

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::apic;
use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::boot;
use vifast_plat_x86_64::control::{
    CR0_PE, CR0_PG, CR4_PAE, EFER_LME, EFER_NXE, MSR_IA32_APIC_BASE,
};
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::gdt;
use vifast_plat_x86_64::idt;
use vifast_plat_x86_64::sched::FrameReturn;
use vifast_plat_x86_64::serial;
use vifast_plat_x86_64::timebase::PitClock;
use vifast_plat_x86_64::tss;

/// **KT-340b** — tám bảng trang boot + arena bảng trang động ra **ngoài `.bss`** (section NOLOAD
/// `.bootpt`). Có mặt ở **cả** ảnh sản phẩm (symbol `__pml4`…`__pt_user` mà `_start` tham chiếu).
mod bootpt;

/// **KT-340b** — `_start` 32-bit + long mode + stub ring 3 (tách khỏi `main.rs`, luật R1).
/// `global_asm!` ở đây phát `.multiboot`/`.text.entry`/`.user`/`.bss` — linker giữ bằng `KEEP`.
mod bootasm;

/// **KT-340b** — miền **trap**: IST1 per-hart, trạng thái exception, arm/resume, handler chung
/// (`#BP`/`#UD`/`#PF`/`#GP`, timer, spurious, IPI, `#PF` cô lập `VSpace`). Tách khỏi `main.rs`
/// theo miền (luật R1: `main.rs` 1.500 → < 1.100 dòng). Chỉ ảnh `ktest` có handler này; ảnh sản
/// phẩm dùng handler tối giản ở [`product`] (D2-08).
#[cfg(feature = "ktest")]
mod trap;
#[cfg(feature = "ktest")]
pub(crate) use trap::{
    exc_arm, exc_arm_user_pf, exc_caught, exc_reset, ist1_top, vspace_fault_seen, vspace_ok,
    vspace_set_probe_ok, EXC_COUNT, EXC_CR2, EXC_CS, EXC_ERR, EXC_RFLAGS, EXC_RIP, EXC_VEC, VEC_BP,
    VEC_GP, VEC_PF, VEC_UD,
};

/// **KT-316** — Console COM1 + khoá **một dòng** (`write_fmt` giữ khoá suốt dòng). Tách tệp
/// riêng (luật R1) và tái xuất để mọi chỗ dùng `crate::Console` như cũ.
mod com;

/// **KT-406** — COM1 RX **IRQ thật** (`IRQ 4 → IOAPIC → LAPIC → IDT 0x24 → ring`). Chỉ ảnh
/// `product-shell` (ảnh kiểm giữ nguyên đường TAP; ảnh tối giản không có U-mode).
#[cfg(all(feature = "product-shell", not(feature = "ktest")))]
mod comirq;
/// **KT-340a** (D2-08): nhánh **ảnh sản phẩm** (không harness) — harness chỉ có trong ảnh
/// build `--features ktest`.
mod product;

pub(crate) use com::{com_guard, com_report, Console};

// **KT-340a** (D2-08): các mô-đun **chỉ** phục vụ pha kiểm bị `cfg` gỡ khỏi ảnh sản phẩm (mô tả
// đầy đủ nằm trong từng tệp — R1 nhờ tách tệp, xem `v2/docs/k5-x86_64-boot.md`).
#[cfg(feature = "ktest")]
mod blk; // KT-288: virtio-blk đọc sector 0 (TAP 26/27).
#[cfg(feature = "ktest")]
mod coresmp; // KT-413: 4 hart vào `vifast-core` qua `SmpBridge` + bộ ca SMP chung — TAP 48/49.
#[cfg(feature = "ktest")]
mod coretest; // KT-340c: `vifast-core::Machine` + IPC ring 3 qua `int 0x80` — TAP 38..43.
#[cfg(all(feature = "ktest", feature = "gpu"))]
mod gpu; // KT-337: virtio-gpu 2D (TAP 35) — runner `gpu-x86_64.sh`.
#[cfg(feature = "ktest")]
mod lock_report; // KT-336a: khoá MCS bọc IRQ (TAP 34).
#[cfg(feature = "ktest")]
mod net; // KT-274: PCI ECAM + virtio-net + ARP (TAP 15..17).
#[cfg(feature = "ktest")]
mod sched; // KT-286/304/310: preemption + tác vụ ring 3 + nhiều hart (TAP 22..25).
#[cfg(feature = "ktest")]
mod sec_probe; // KT-418: SMEP/SMAP + ca âm phần cứng — TAP 50.
/// **KT-341** — shell **sản phẩm dùng chung** (`vifast-plat-user/programs/shell.rs`) nạp ELF thật,
/// chạy CPL3 trên **cả** ảnh sản phẩm (`--features product-shell`) và ảnh kiểm (TAP 44/45).
#[cfg(feature = "shell")]
mod shellpath;
#[cfg(feature = "ktest")]
mod smp; // KT-276/278: SMP 4 hart, IPI, TSS/IST1 per-hart (TAP 18..21).
#[cfg(feature = "ktest")]
mod tlbshoot; // KT-340d: TLB shootdown đa hart + IPI mailbox — TAP 37.
#[cfg(feature = "ktest")]
mod umode; // KT-270: U-mode ring 3 + `int 0x80` + `Exit`.
#[cfg(feature = "ktest")]
mod wx_probe; // KT-340a: W^X kernel — TAP 35/36.

/// Dòng marker **duy nhất** của ảnh — runner so **nguyên văn** dòng này và đòi **đúng một lần**.
#[cfg(not(feature = "wrong-marker"))]
pub const BOOT_MARKER: &str = "# viFastOs x86_64: boot ok\n";
/// Ca âm của runner: **cùng** ảnh, chỉ đổi chuỗi marker (feature `wrong-marker`).
#[cfg(feature = "wrong-marker")]
pub const BOOT_MARKER: &str = "# viFastOs x86_64: boot BAD\n";

/// Dòng in **trước** ACPI shutdown — chứng minh thoát sạch, không triple-fault.
pub const HALT_LINE: &str = "# viFastOs x86_64: halt";

/// Số ca TAP mà ảnh phát ra (khớp `1..N` và `EXPECT_TESTS` của runner).
///
/// KT-266 thêm **bốn** ca: `apic: lapic id`, `apic: ioapic`, `timer: ticks`, `timer: eoi`.
/// **KT-270** thêm **ba** ca U-mode: `umode: ring3 entered`, `umode: syscall handled`,
/// `umode: returned to ring3`. **KT-274** thêm **ba** ca PCI/net: `pci: ecam`, `net: mac`,
/// `net: arp` (TAP 15..17, chạy sau `Exit` của ring 3 — xem [`net`]).
/// **KT-276** thêm **hai** ca SMP: `smp: ap online`, `smp: both harts report` (TAP 18/19, chạy
/// sau TAP 17 — xem [`smp`]).
/// **KT-278** thay hai ca SMP bằng **bốn** (18..21); **KT-286** thêm preemption (22/23);
/// **KT-288/304/310/316/318/326/328/336a** lấp 24..34; **KT-340a** thêm W^X `wx: kernel
/// sections` (35) + `wx: text write faults` (36); **KT-341** thêm shell CPL3 (44/45) và
/// **KT-341b** thêm `ps` (46) + `wait`/`kill` qua procsrv (47); **KT-413** thêm SMP của
/// `vifast-core` (48/49) — tổng **49** (**50** khi bật `gpu`).
pub const TAP_TESTS: usize = 50 + cfg!(feature = "gpu") as usize;

/// `IA32_EFER` mà `_start` bật: `LME` (bit 8) | `NXE` (bit 11).
///
/// `NXE` cần cho **NX** của trang stack ring 3 (KT-270): không bật thì bit 63 của entry bảng
/// trang là reserved và mọi truy cập sinh `#PF` với `RSVD = 1`.
pub(crate) const EFER_LME_NXE: u32 = (EFER_LME | EFER_NXE) as u32;

/// Địa chỉ **canonical** nhưng **chưa map** dùng cho ca `#PF`: `0x0000_7000_0000_0000`.
///
/// Canonical vì bit 63..48 = 0 trùng bit 47 = 0 (4-level paging, 48-bit VA); chưa map vì
/// `PML4[224]` = 0 (identity map của ảnh chỉ phủ `0..1 GiB`, MMIO map phủ `3..4 GiB`). Đọc nó
/// ⇒ `#PF` với `P = 0`.
pub const PF_UNMAPPED_ADDR: u64 = 0x0000_7000_0000_0000;

/// Selector **ngoài** GDT (bảng có 7 entry ⇒ limit 55; selector `0x38` = index 7 = byte 56).
///
/// Nạp nó vào `DS` ⇒ `#GP(0x38)` theo SDM Vol 3A §5.4.1 ("segment selector index outside the
/// descriptor table limits") — cách sinh `#GP` **chắc chắn** và **không đổi trạng thái** (DS giữ
/// nguyên khi fault), khác với `mov cr0` (có thể tắt paging nếu QEMU không fault).
pub const GP_BAD_SELECTOR: u16 = 0x38;

// **KT-340b** — `NO_EXPECT` (sentinel "không chờ exception nào") đã chuyển vào [`trap`] cùng
// toàn bộ trạng thái exception.

/// Khởi tạo COM1 115200 8N1 theo [`serial::init_sequence`]; trả divisor đã dùng để in bằng chứng.
fn serial_init() -> (u8, u8) {
    let divisor = serial::divisor(serial::BAUD_115200).unwrap_or(serial::DIVISOR_FALLBACK);
    for step in serial::init_sequence(divisor) {
        asm::outb(serial::COM1_BASE + step.offset, step.value);
    }
    divisor
}

/// Ghi một byte: chờ `LSR.THRE` (**có trần** [`serial::TX_WAIT_LIMIT`]) rồi ghi `THR`.
///
/// Trần là **số vòng**, không phải thời gian: UART hỏng không được treo máy, và một lần ghi bình
/// thường không bao giờ chạm trần.
pub(crate) fn write_byte(byte: u8) {
    // **KT-340c** — một đường byte duy nhất: `syscall_env::X86_64Env::put_str` (crate arch)
    // dùng **cùng** hàm này, nên `DebugPutStr` và `Console` không thể lệch nhau về cách chờ
    // `LSR.THRE`.
    asm::com1_write_byte(byte);
}

/// `#[panic_handler]` của ảnh: in `PANIC` + thông điệp rồi tắt máy (QEMU treo ⇒ runner đỏ).
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let mut console = Console;
    let _ = writeln!(console, "PANIC: {info}");
    asm::halt(1)
}

// ---------------------------------------------------------------------------------------------
// TAP
// ---------------------------------------------------------------------------------------------

/// In một dòng TAP `ok N - <tên>` / `not ok N - <tên>`; trả `ok`.
///
/// TAP 13 (Test Anything Protocol): dòng `ok`/`not ok` phải bắt đầu dòng, số hiệu liên tục, và
/// runner (`run-x86_64.sh`) đòi **đủ** `1..N` dòng `ok`, **0** dòng `not ok`, và coi mọi
/// `not ok`/`PANIC` là đỏ.
fn tap(console: &mut Console, n: usize, name: &str, ok: bool) -> bool {
    let _ = writeln!(console, "{} {n} - {name}", if ok { "ok" } else { "not ok" });
    ok
}

/// Phát TAP `n`; **halt ngay** nếu đỏ (fail-fast: một ca sai thì các ca sau không còn nghĩa).
pub(crate) fn check(console: &mut Console, n: usize, name: &str, ok: bool) {
    if !tap(console, n, name, ok) {
        let _ = writeln!(console, "# TAP {n} FAIL — dừng (không in marker)");
        asm::halt(3)
    }
}

// ---------------------------------------------------------------------------------------------
// APIC + timer — KT-266 (K5' bước 3)
// ---------------------------------------------------------------------------------------------

/// Địa chỉ MMIO của LAPIC, đọc từ `IA32_APIC_BASE` lúc khởi tạo — handler `EOI` dùng lại.
pub(crate) static LAPIC_BASE: AtomicU64 = AtomicU64::new(0);
/// Số tick timer đã nhận (handler tăng) — nguồn tick là PIT kênh 0 qua IOAPIC → LAPIC.
pub(crate) static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
/// Số lần ghi `EOI` (handler tăng **sau** khi ghi EOI thành công).
pub(crate) static EOI_COUNT: AtomicU64 = AtomicU64::new(0);
/// Số ngắt **spurious** đã nhận — không `EOI` (SDM Vol 3A §10.9).
pub(crate) static SPURIOUS_COUNT: AtomicU64 = AtomicU64::new(0);

/// **KT-413** (nợ `N-3`) — số lần **watchdog LAPIC** (vector `0x32`) đã đánh thức `hlt`.
///
/// Handler của vector riêng **chỉ** tăng bộ đếm này rồi `EOI`: nó **không** đụng `TICK_COUNT`
/// (nếu đụng, một ngắt watchdog sẽ giả một tick và phép đo `hz`/`ticks_ok` mất nghĩa).
pub(crate) static WATCHDOG_WAKES: AtomicU64 = AtomicU64::new(0);

/// Số chia LAPIC timer: **1** (mã `0b1011`).
const TIMER_DIVIDE: u32 = 1;
/// Giá trị đếm đầu tiên của LAPIC timer — 1 ms danh định (mô hình QEMU: 1 count/ns).
///
/// **Không** dùng làm nguồn tick: QEMU TCG giao ngắt LAPIC timer không đều (đo được chu kỳ
/// 2,8–38,5 ms cho chu kỳ danh định 100 µs). Ảnh cấu hình đủ ba thanh ghi + **kiểm counter
/// đổi giá trị** (bằng chứng timer chạy, KT-297) rồi mask `LVT`; nguồn tick là **PIT** (xem
/// [`timer_init`]).
const TIMER_INITIAL_COUNT: u32 = 1_000_000;

/// **KT-413** (nợ `N-3`) — `INITIAL_COUNT` của **watchdog LAPIC** (vector `0x32`).
///
/// Cùng bộ chia `TIMER_DIVIDE = 1` với timer thường, nên chu kỳ watchdog **xấp xỉ** chu kỳ
/// timer đã cấu hình (bus LAPIC không được đặc tả ⇒ không hardcode `Hz`). Giá trị **không** cần
/// chính xác: watchdog chỉ là **nguồn đánh thức** cho `hlt`; cửa sổ đo vẫn do **bộ đếm PIT**
/// quyết định (đọc cổng I/O, không phụ thuộc IRQ0).
const WATCHDOG_INITIAL_COUNT: u32 = 1_000_000;

/// **KT-413** — số lần watchdog phải đánh thức **trước khi** kết luận "bộ đếm PIT chết".
///
/// Chỉ dùng khi `clock.counts() == 0` (PIT **không** đếm): lúc ấy cửa sổ PIT không bao giờ đóng
/// và trần `HZ_WINDOW_SPIN_BOUND` sẽ rất chậm. Có watchdog, trần này biến "treo vĩnh viễn" thành
/// "thoát sau `WATCHDOG_MAX_WAKES` lần đánh thức" ⇒ boot **luôn** thoát, TAP 10 đỏ có chẩn đoán.
const WATCHDOG_MAX_WAKES: u64 = 64;

/// Số chia PIT cho **100 Hz**: 1_193_182 / 100 = 11_931,82 ⇒ 11_932 (sai số 0,0015 %).
const PIT_DIVISOR_100HZ: u16 = 11_932;
/// Tần số tick danh định của PIT (Hz) — **tính** từ hằng số PIT, làm tròn, không hardcode.
const PIT_TICK_HZ: u32 =
    (apic::PIT_FREQUENCY_HZ + PIT_DIVISOR_100HZ as u32 / 2) / PIT_DIVISOR_100HZ as u32;
/// Chân IOAPIC của IRQ0 (PIT) trên PC: **pin 2** (IRQ0 → pin 2, IRQ1 → pin 1, IRQ2 → pin 0).
const IOAPIC_PIT_PIN: u32 = 2;
/// Trần vòng chờ **counter LAPIC đổi giá trị** — timer hỏng thì cắt (ĐỎ) thay vì treo.
///
/// Mỗi vòng là một lần đọc MMIO (~1 µs); counter QEMU đổi mỗi ns nên timer chạy đổi ngay vòng
/// đầu. 100_000 vòng ≈ 0,1 s là dư cho timer chết (đột biến `no-lapic-timer`).
const LAPIC_RUNS_SPIN_BOUND: u32 = 100_000;
/// Số count PIT kênh 2 của cửa sổ đo `hz` — **500 ms** (1.193.182 × 0,5 = 596.591 count).
///
/// # Vì sao cửa sổ **cố định** thay vì "chờ ba mốc tick" (KT-297)
///
/// KT-293 đo `hz` trên **hai** khoảng tick (mỗi khoảng 1 tick): dưới TCG QEMU giao tick không
/// đều (đo được 9..55 ms cho chu kỳ danh định 10 ms) ⇒ `hz` trải 0..24.350 và 2/600 boot ra
/// `window_pit=0` (mốc tick đã chờ sẵn ⇒ vòng chờ thoát trước lần đọc PIT đầu). Cửa sổ **cố
/// định 500 ms** đo trên ~20 tick ⇒ `hz` là trung bình, không phụ thuộc một mốc tick đơn lẻ;
/// cửa sổ luôn có ảnh PIT (đọc mốc trước khi chờ) nên không thể ra 0.
const HZ_WINDOW_COUNTS: u32 = 596_591;
/// Trần vòng poll cửa sổ đo — PIT hỏng thì cắt (ĐỎ) thay vì treo.
///
/// Mỗi vòng ~2 µs (3 cổng I/O) ⇒ 500 ms cần ~250.000 vòng; 500.000 là dư ~2×. Với `hlt` mỗi
/// vòng ~20 ms (một tick) nên trần này chỉ cắt khi `hlt` trả về ngay (ngắt spurious liên tục).
const HZ_WINDOW_SPIN_BOUND: u32 = 500_000;

/// Đọc thanh ghi LAPIC (MMIO 32-bit) theo offset.
pub(crate) fn lapic_read(offset: u32) -> u32 {
    asm::read_u32(LAPIC_BASE.load(Ordering::Relaxed) + offset as u64)
}

/// Ghi thanh ghi LAPIC (MMIO 32-bit) theo offset.
pub(crate) fn lapic_write(offset: u32, value: u32) {
    asm::write_u32(LAPIC_BASE.load(Ordering::Relaxed) + offset as u64, value);
}

/// Địa chỉ MMIO LAPIC mà BSP đã dùng — **KT-276**: hart phụ đọc lại từ `IA32_APIC_BASE` của nó
/// (cùng giá trị) và boot block công bố giá trị này.
pub(crate) fn lapic_base() -> u64 {
    LAPIC_BASE.load(Ordering::Relaxed)
}

/// `LAPIC_ID` của hart **đang chạy** — đọc MMIO nên mỗi hart thấy ID của chính nó.
pub(crate) fn lapic_id() -> u8 {
    apic::lapic_id(lapic_read(apic::LAPIC_ID))
}

/// Ghi `EOI` — báo LAPIC đã xử lý xong ngắt hiện tại (SDM §10.6.1).
pub(crate) fn lapic_eoi() {
    lapic_write(apic::LAPIC_EOI, 0);
}

/// Đọc thanh ghi IOAPIC qua cặp `IOREGSEL`/`IOWIN`.
fn ioapic_read(reg: u32) -> u32 {
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_REGSEL as u64, reg);
    asm::read_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_WIN as u64)
}

/// Ghi thanh ghi IOAPIC qua cặp `IOREGSEL`/`IOWIN`.
fn ioapic_write(reg: u32, value: u32) {
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_REGSEL as u64, reg);
    asm::write_u32(apic::IOAPIC_DEFAULT_BASE + apic::IOAPIC_WIN as u64, value);
}

/// Nạp PIT kênh 0 **mode 2** (rate generator — tự nạp lại) với `divisor`.
fn pit_program_mode2(divisor: u16) {
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_CH0_MODE2_LOHI);
    asm::outb(apic::PIT_CH0_DATA, divisor as u8);
    asm::outb(apic::PIT_CH0_DATA, (divisor >> 8) as u8);
}

/// Đọc counter PIT kênh 0 bằng lệnh **latch** (hai lần đọc LSB/MSB là một ảnh ổn định).
fn pit_latch_read() -> u16 {
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_LATCH_CH0);
    let lo = asm::inb(apic::PIT_CH0_DATA) as u16;
    let hi = asm::inb(apic::PIT_CH0_DATA) as u16;
    (hi << 8) | lo
}

/// Nạp PIT **kênh 2** mode 2 (rate generator) với `divisor` — **đồng hồ đo** độc lập kênh 0.
///
/// Kênh 2 là timer của loa: phải bật **gate** (cổng `0x61` bit 0) thì counter mới chạy, và giữ
/// bit dữ liệu loa = 0 để không phát âm. Dùng divisor `0xFFFF` (chu kỳ 54,9 ms) — lớn hơn nhiều
/// so với khoảng cách giữa hai lần đọc (~10 ms), nên `pit_elapsed_ticks` không bị mất vòng.
fn pit2_program_mode2(divisor: u16) {
    let p61 = asm::inb(apic::PIT_SPEAKER_PORT);
    asm::outb(
        apic::PIT_SPEAKER_PORT,
        (p61 & !apic::PIT_SPEAKER_DATA) | apic::PIT_SPEAKER_GATE2,
    );
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_CH2_MODE2_LOHI);
    asm::outb(apic::PIT_CH2_DATA, divisor as u8);
    asm::outb(apic::PIT_CH2_DATA, (divisor >> 8) as u8);
}

/// Đọc counter PIT **kênh 2** bằng lệnh latch.
///
/// **KT-274**: `pub(crate)` — đồng hồ của bước PCI/net ([`net`]) dùng lại chính counter này
/// (mode 2 divisor `0xFFFF` đã được [`measure_timer`] lập trình, không phát ngắt).
pub(crate) fn pit2_latch_read() -> u16 {
    asm::outb(apic::PIT_CH0_CMD, apic::PIT_CMD_LATCH_CH2);
    let lo = asm::inb(apic::PIT_CH2_DATA) as u16;
    let hi = asm::inb(apic::PIT_CH2_DATA) as u16;
    (hi << 8) | lo
}

/// Kết quả khởi tạo LAPIC — bằng chứng in ra log + dùng cho TAP.
struct LapicInfo {
    /// `IA32_APIC_BASE` đọc lại sau khi bật (bit 11 = enable).
    base_msr: u64,
    /// Địa chỉ MMIO đã dùng.
    base: u64,
    /// `LAPIC_ID` thô.
    id_raw: u32,
    /// `LAPIC_VERSION` thô.
    version_raw: u32,
    /// `SVR` đọc lại sau khi ghi.
    svr: u32,
}

/// Khởi tạo LAPIC: che 8259 PIC, đọc `IA32_APIC_BASE`, bật APIC toàn cục nếu cần, đọc
/// ID/version, bật `SVR`, đặt `TPR = 0` (nhận mọi ưu tiên), mask `LINT0`/`LINT1`.
#[cfg(feature = "ktest")]
fn lapic_init() -> LapicInfo {
    // 0. Che **8259 PIC** trước mọi thứ: trên PC, PIC nối vào `LAPIC.LINT0` (virtual wire) và
    //    BIOS đặt vector IRQ0 = `0x08` — **trùng `#DF`**. Không che thì ngắt PIT (IRQ0) được giao
    //    thẳng cho CPU với vector 8 và ảnh chết trong `#DF` ngay sau `sti` (đo được ở lần chạy
    //    đầu KT-266: `# exc: vec=8 err=0x100479 rip=0x8 cs=0x203`).
    asm::outb(apic::PIC1_DATA, apic::PIC_MASK_ALL);
    asm::outb(apic::PIC2_DATA, apic::PIC_MASK_ALL);

    // 1. `IA32_APIC_BASE`: base ở bit 12..51, bit 11 = enable toàn cục.
    let mut msr = asm::read_msr(MSR_IA32_APIC_BASE);
    if msr & apic::APIC_BASE_ENABLE == 0 {
        msr |= apic::APIC_BASE_ENABLE;
        asm::write_msr(MSR_IA32_APIC_BASE, msr);
    }
    let base = msr & apic::APIC_BASE_ADDR_MASK;
    LAPIC_BASE.store(base, Ordering::Relaxed);

    // 2. Đọc ID/version **trước** khi bật SVR — nếu MMIO không map/không có APIC, đây là chỗ
    //    fault hoặc đọc ra 0/0xFFFFFFFF và TAP sẽ đỏ.
    let id_raw = lapic_read(apic::LAPIC_ID);
    let version_raw = lapic_read(apic::LAPIC_VERSION);
    // **KT-276**: chốt `LAPIC_ID` của BSP **trước** khi bật ngắt — handler timer dùng nó để tách
    // tick của BSP khỏi tick đánh thức của hart phụ.
    smp::set_bsp_lapic_id(apic::lapic_id(id_raw));

    // 3. Bật APIC bằng phần mềm: `SVR = vector spurious | APIC_ENABLE`.
    lapic_write(apic::LAPIC_SVR, apic::svr_value(exc::VEC_SPURIOUS));
    // 4. `TPR = 0` — không che ưu tiên nào (mọi ngắt đều được nhận).
    lapic_write(apic::LAPIC_TPR, 0);
    // 4b. Mask `LINT0`/`LINT1` — chặn nốt đường virtual wire của PIC và NMI ngoài; nguồn ngắt
    //     duy nhất còn lại là IOAPIC (đã mask toàn bộ, sau đó mở đúng IRQ0 cho PIT).
    lapic_write(apic::LAPIC_LVT_LINT0, apic::LVT_MASKED);
    lapic_write(apic::LAPIC_LVT_LINT1, apic::LVT_MASKED);
    // 5. Đọc lại SVR — bằng chứng ghi **thật**, không chỉ "đã gọi".
    let svr = lapic_read(apic::LAPIC_SVR);

    LapicInfo {
        base_msr: asm::read_msr(MSR_IA32_APIC_BASE),
        base,
        id_raw,
        version_raw,
        svr,
    }
}

/// Kết quả khởi tạo IOAPIC.
struct IoapicInfo {
    /// `IOAPICID` thô.
    id_raw: u32,
    /// `IOAPICVER` thô.
    version_raw: u32,
    /// Số redirection entry (`IOAPICVER` + 1).
    entries: u32,
    /// `true` ⇔ **mọi** entry đọc lại đều có bit mask.
    all_masked: bool,
}

/// Khởi tạo IOAPIC: đọc `IOAPICID`/`IOAPICVER`, rồi **mask toàn bộ** redirection entry.
///
/// Bước này **chưa** cần ngoại vi: mục tiêu là chứng minh có IOAPIC thật (đọc được ID/version)
/// và che hết nguồn ngắt ngoài. [`timer_init`] sau đó mở **đúng một** entry (IRQ0/PIT) làm nguồn
/// tick — đó là lựa chọn dự phòng của KT-266, ghi rõ trong tài liệu.
///
/// **KT-348a (D2-257)** — mask **kèm** xoá `Destination` (word cao): bản cũ chỉ ghi word thấp nên
/// `Destination` giữ nguyên giá trị firmware. [`timer_init`] sau đó **mở mask trước khi** đặt
/// `Destination` ⇒ có cửa sổ vector 32 **không mask** với đích **cũ/rác** ⇒ ngắt timer có thể đi
/// sai đích hoặc không tới BSP. Xoá ở đây làm trạng thái "mask + dest=0" **tường minh**.
fn ioapic_init() -> IoapicInfo {
    let id_raw = ioapic_read(apic::IOAPIC_ID_REG);
    let version_raw = ioapic_read(apic::IOAPIC_VER_REG);
    let entries = apic::ioapic_redirection_count(version_raw);
    let mut all_masked = true;
    let mut pin = 0u32;
    while pin < entries {
        if let Some(reg) = apic::ioapic_redtbl_reg(pin) {
            let low = ioapic_read(reg);
            ioapic_write(reg, apic::ioapic_redir_low_masked(low));
            // Thứ tự **an toàn**: mask (word thấp) **trước**, rồi mới xoá Destination (word cao) —
            // trong lúc entry còn mask thì không ngắt nào đi ra ngoài.
            ioapic_write(reg + 1, 0);
            // Đọc lại: mask phải **thật sự** nằm trong thanh ghi.
            if !apic::ioapic_redir_is_masked(ioapic_read(reg)) {
                all_masked = false;
            }
        }
        pin += 1;
    }
    IoapicInfo {
        id_raw,
        version_raw,
        entries,
        all_masked,
    }
}

/// Kết quả khởi tạo timer.
struct TimerInit {
    /// `LAPIC_TIMER_CURRENT_COUNT` lần đọc thứ nhất.
    lapic_count0: u32,
    /// `LAPIC_TIMER_CURRENT_COUNT` lần đọc thứ hai (sau vài vòng).
    lapic_count1: u32,
    /// `true` ⇔ current count **giảm** ⇒ LAPIC timer chạy thật.
    lapic_timer_runs: bool,
    /// **KT-348a (D2-257)** — `true` ⇔ trong cửa sổ probe counter, `LVT_TIMER` **đã mask**
    /// (không phát ngắt trên vector 32 khi đang đo). Đọc lại từ thanh ghi, không tin hằng số.
    lvt_probe_masked: bool,
    /// **KT-348a** — `true` ⇔ **trước khi mở mask** redirection IRQ0, entry đã mask (thứ tự an
    /// toàn: đổi `Destination` trong lúc mask, rồi mới mở mask).
    ioapic_masked_before: bool,
    /// **KT-348a** — `true` ⇔ `Destination` của redirection IRQ0 **= BSP LAPIC ID** đọc từ thanh
    /// ghi (không phải hằng `0` giả định).
    ioapic_dest_ok: bool,
    /// `true` ⇔ redirection IRQ0 của IOAPIC trỏ vector 32, **không** mask, `Destination` = BSP
    /// LAPIC ID (đọc từ thanh ghi — KT-348a bỏ giả định "dest luôn là 0").
    ioapic_route_ok: bool,
    /// Counter PIT đọc ngay sau khi nạp mode 2 (phải ≈ divisor).
    pit_counter0: u16,
    /// Counter PIT sau vài vòng (phải **giảm**).
    pit_counter1: u16,
}

/// Khởi tạo timer: cấu hình + kiểm LAPIC timer, rồi dùng **PIT** làm nguồn tick.
///
/// # Vì sao nguồn tick là PIT (D2-204, KT-266)
///
/// Đo trên QEMU 11.1.3 TCG: LAPIC timer đặt `initial_count = 100_000`, chia 1 (chu kỳ danh định
/// 100 µs theo mô hình QEMU `(count+1) ns`) nhưng **khoảng cách giữa hai ngắt đo được** là
/// 2,8–38,5 ms và đổi giữa các lần boot — QEMU chỉ xử lý timer ở ranh giới vòng lặp chính, không
/// theo từng chu kỳ. Vì vậy ảnh dùng **PIT kênh 0 mode 2** (1,193182 MHz, chia 11_932 ⇒ 100 Hz)
/// làm nguồn tick, còn LAPIC **nhận** ngắt qua IOAPIC (redirection IRQ0 → vector 32) và ghi
/// `EOI`. LAPIC timer vẫn được cấu hình đủ ba thanh ghi và **kiểm counter đổi giá trị** — bằng
/// chứng nó chạy — rồi mask để không tranh nguồn tick.
fn timer_init() -> TimerInit {
    // ---- 1. LAPIC timer: đủ ba thanh ghi (divide, LVT, initial count) + kiểm **periodic** chạy.
    //
    // **KT-348a (D2-257)** — `LVT_TIMER` **mask ngay từ đầu** trong cửa sổ probe. Bản cũ arm
    // LVT *không mask* rồi mới mask sau khi đo counter ⇒ trên **cùng vector 32** (vector tick
    // PIT) một ngắt probe có thể chen vào cửa sổ đo, và nếu LVT bắn trước `sti` thì bit IRR còn
    // treo tới lần `sti` đầu tiên. Mask **không** dừng bộ đếm (SDM 3A §10.5.4) ⇒ phép kiểm
    // "timer chạy" (counter đổi giá trị) **giữ nguyên giá trị**; chỉ chặn đường phát ngắt.
    let divide_code = apic::timer_divide_code(TIMER_DIVIDE).unwrap_or(apic::TIMER_DIVIDE_BY_1);
    lapic_write(apic::LAPIC_TIMER_DIVIDE, divide_code);
    lapic_write(
        apic::LAPIC_LVT_TIMER,
        apic::lvt_timer_value(exc::VEC_TIMER, true) | apic::LVT_MASKED,
    );
    // **Đột biến (i)** (`--features no-lapic-timer`): **không** nạp `INITIAL_COUNT` ⇒ counter
    // đứng yên ở 0 ⇒ `runs=0` **thật** (ĐỎ ở TAP 10). Chỉ dùng để chứng minh bộ kiểm có răng.
    if !cfg!(feature = "no-lapic-timer") {
        lapic_write(apic::LAPIC_TIMER_INITIAL_COUNT, TIMER_INITIAL_COUNT);
    }
    // **Bất biến runtime** (KT-348a): trong **suốt** cửa sổ probe, LVT phải **masked**. Đọc lại
    // từ thanh ghi (không tin hằng số) — trên mã cũ giá trị này là `false` ⇒ một dòng `# timer:`
    // mới + TAP 10 đỏ, tức bộ kiểm **có răng** với chính lỗi này.
    let lvt_probe_masked = apic::lvt_is_masked(lapic_read(apic::LAPIC_LVT_TIMER));
    let lapic_count0 = lapic_read(apic::LAPIC_TIMER_CURRENT_COUNT);
    // Chờ counter **đổi giá trị** — **không** đòi giảm: `LVT_TIMER` là periodic nên counter
    // quay vòng và `count1 > count0` là hợp lệ (KT-293: 4/600 boot `runs=0` giả). Xem
    // [`apic::lapic_timer_runs`].
    let mut lapic_count1 = lapic_count0;
    let mut spins = 0u32;
    while spins < LAPIC_RUNS_SPIN_BOUND {
        core::hint::spin_loop();
        spins += 1;
        lapic_count1 = lapic_read(apic::LAPIC_TIMER_CURRENT_COUNT);
        if lapic_count1 != lapic_count0 {
            break;
        }
    }
    let lapic_timer_runs = apic::lapic_timer_runs(lapic_count0, lapic_count1, TIMER_INITIAL_COUNT);
    // Mask `LVT`: nguồn tick là PIT, LAPIC timer **không** được phát ngắt.
    lapic_write(
        apic::LAPIC_LVT_TIMER,
        apic::lvt_timer_value(exc::VEC_TIMER, true) | apic::LVT_MASKED,
    );

    // ---- 2. PIT kênh 0 mode 2 — 100 Hz.
    //
    // **Đột biến (ii)** (`--features wrong-hz`): nạp divisor **nhỏ hơn 10×** ⇒ nguồn tick
    // 1.000 Hz danh định ⇒ `div_ok=0` (counter đọc lại ≈ 1.125, ngoài dải) ⇒ TAP 10 ĐỎ. Chỉ
    // dùng để chứng minh bộ kiểm có răng. (Đột biến **chậm** hơn cũng bị bắt: counter > divisor.)
    let pit_divisor = if cfg!(feature = "wrong-hz") {
        PIT_DIVISOR_100HZ / 10
    } else {
        PIT_DIVISOR_100HZ
    };
    // Đọc lại counter ngay sau khi nạp. Nếu lần đọc bị trễ (host deschedule), nạp lại và đọc
    // lại — lấy giá trị **lớn nhất** (gần divisor nhất), tối đa 4 lần. KT-297 soak 600 bắt
    // 1/600 boot đọc trễ ~7 ms (counter 3.628 thay vì ~11.900) ⇒ false ĐỎ nếu không retry.
    let mut pit_counter0 = 0u16;
    let mut tries = 0u32;
    while tries < 4 {
        pit_program_mode2(pit_divisor);
        let c = pit_latch_read();
        if c > pit_counter0 {
            pit_counter0 = c;
        }
        if apic::pit_divisor_ok(pit_counter0, PIT_DIVISOR_100HZ) {
            break;
        }
        tries += 1;
    }
    let mut pit_spins = 0u32;
    while pit_spins < 2000 {
        core::hint::spin_loop();
        pit_spins += 1;
    }
    let pit_counter1 = pit_latch_read();

    // ---- 3. IOAPIC: mở **đúng một** redirection (IRQ0/PIT) → vector 32, dest = **BSP LAPIC ID**.
    //
    // **KT-348a (D2-257)** — **thứ tự là bất biến**: ghi `Destination` (word cao) **trước**, rồi
    // mới ghi word thấp (mở mask). Bản cũ ghi word thấp (mở mask) trước ⇒ giữa hai lệnh ghi có
    // cửa sổ vector 32 **không mask** với `Destination` **cũ/rác** (firmware) ⇒ ngắt PIT có thể
    // đi sai đích hoặc không tới BSP. Đích lấy từ **thanh ghi** `LAPIC_ID` của hart đang chạy
    // (BSP) — **không** dùng `smp::` (mô-đun đó gắn `feature = "ktest"`, còn `timer_init` được
    // biên dịch cho **cả** ảnh sản phẩm).
    let bsp_dest = apic::lapic_id(lapic_read(apic::LAPIC_ID)) as u32;
    let mut ioapic_route_ok = false;
    let mut ioapic_masked_before = false;
    let mut ioapic_dest_ok = false;
    if let Some(reg) = apic::ioapic_redtbl_reg(IOAPIC_PIT_PIN) {
        // Tiền điều kiện **đo được**: entry còn mask trước khi ta đổi `Destination`.
        ioapic_masked_before = apic::ioapic_redir_is_masked(ioapic_read(reg));
        ioapic_write(reg + 1, bsp_dest);
        ioapic_write(reg, exc::VEC_TIMER as u32);
        let low = ioapic_read(reg);
        let high = ioapic_read(reg + 1);
        ioapic_dest_ok = high == bsp_dest;
        ioapic_route_ok = (low & 0xFF) == exc::VEC_TIMER as u32
            && !apic::ioapic_redir_is_masked(low)
            && ioapic_dest_ok;
    }

    TimerInit {
        lapic_count0,
        lapic_count1,
        lapic_timer_runs,
        lvt_probe_masked,
        ioapic_masked_before,
        ioapic_dest_ok,
        ioapic_route_ok,
        pit_counter0,
        pit_counter1,
    }
}

/// Kết quả đo timer.
struct TimerInfo {
    /// Tần số tick **đo được** (Hz) trên cửa sổ cố định — không phải tần số danh định.
    hz: u32,
    /// `TICK_COUNT` tại cuối cửa sổ đo (tổng số tick đã nhận từ đầu boot).
    ticks: u64,
    /// `true` ⇔ cửa sổ đo **đủ dài**, tick **tăng**, và `hz` trong khoảng hợp lệ.
    ticks_ok: bool,
    /// Cửa sổ đo **đo được** (count PIT kênh 2) — bằng chứng nhịp.
    period_pit: u32,
    /// **KT-413** (nợ `N-3`) — số lần **watchdog LAPIC** đã đánh thức `hlt` trong cửa sổ đo.
    watchdog_wakes: u64,
    /// Watchdog đã **arm** được (LVT ghi lại đúng vector `0x32`, không mask).
    watchdog_armed: bool,
    /// Watchdog đã **mask lại** sau cửa sổ đo (nguồn tick vẫn là PIT, không phải LAPIC timer).
    watchdog_masked_after: bool,
    /// `true` ⇔ bộ đếm PIT **không** đếm (cửa sổ không bao giờ đóng) — watchdog đã cắt vòng.
    pit_dead: bool,
}

/// Đo timer: cửa sổ **cố định 500 ms** bằng PIT kênh 2 + `hz` **đo** + đối chiếu `EOI` với tick.
///
/// # Vì sao cửa sổ cố định, không "chờ ba mốc tick" (KT-297)
///
/// KT-293 đo `hz` trên **hai** khoảng tick (mỗi khoảng 1 tick). Dưới TCG, QEMU giao tick không
/// đều (đo được 9..55 ms cho chu kỳ danh định 10 ms) nên `hz` trải **0..24.350**; 2/600 boot ra
/// `window_pit=0` vì mốc tick đã chờ sẵn (vòng chờ thoát trước lần đọc PIT đầu), và một số boot
/// `ticks_ok=false` vì một mốc nhảy quá `MAX_TICK_DELTA`. Cửa sổ **cố định 500 ms**:
///
/// - đo trên ~20 tick ⇒ `hz` là **trung bình**, không phụ thuộc một mốc tick đơn lẻ;
/// - đọc mốc PIT **trước** khi đọc tick ⇒ cửa sổ luôn có ảnh PIT, không thể `window_pit=0`;
/// - `ticks_ok` chỉ đòi tick **tăng** trong cửa sổ (không đòi từng mốc cách nhau ≤ 1.000 tick
///   — dưới TCG host bận, một mốc có thể nhảy xa; đó là **jitter của QEMU**, không phải timer
///   hỏng), **và** `hz` giao được nằm trong [`apic::HZ_MIN`]..=[`apic::HZ_MAX`].
///
/// # Vì sao vẫn cần kiểm divisor PIT (KT-297)
///
/// Dưới TCG, tốc độ tick **giao được** bị chặn trên bởi độ trễ đánh thức `hlt` (~20 ms) nên
/// `hz` **bão hoà** ~30..55 Hz cho cả nguồn 100 Hz lẫn 1.000 Hz. Vì vậy TAP 10 còn đòi
/// [`apic::pit_divisor_ok`] — đọc lại counter PIT ngay sau khi nạp mode 2 — để bắt nguồn tick
/// **sai divisor** (đột biến `wrong-hz`). Đây là **kiểm đúng đắn**, không phải nới ngưỡng.
///
/// Số tick **tuyệt đối** không phải hợp đồng; `hz` là số **đo**, không hardcode.
fn measure_timer() -> TimerInfo {
    // ---- 1. Cửa sổ cố định 500 ms: PIT kênh 2 mode 2 divisor 0xFFFF, cộng dồn qua mỗi lần
    //         counter nạp lại (`PitClock` xử lý bọc — cùng bộ đếm `net`/`blk`/`smp` dùng).
    pit2_program_mode2(apic::PIT_MAX_DIVISOR);
    let mut clock = PitClock::new();
    clock.reset(pit2_latch_read());
    let t0 = TICK_COUNT.load(Ordering::Relaxed);
    // ---- **KT-413** (nợ `N-3`): arm **watchdog LAPIC** trên vector **riêng** `0x32` **trước**
    // vòng `hlt`. Vì sao cần: `hlt` chỉ thức khi **có** ngắt; nếu mất IRQ0 (IOAPIC mask sai, PIT
    // hỏng, redirection đứt) thì **không** ngắt nào tới ⇒ `hlt` ngủ **vĩnh viễn** ⇒ boot treo im
    // lặng. `KT-348a` đo được đúng lớp ấy và để lại nợ `N-3`.
    //
    // Watchdog **không** phải nguồn tick: nó chỉ **đánh thức** `hlt`; cửa sổ đo vẫn do **bộ đếm
    // PIT** quyết định (đọc cổng I/O, **không** phụ thuộc IRQ0) ⇒ phép đo `hz` giữ nguyên nghĩa.
    let watchdog_armed = watchdog_arm();
    let wakes0 = WATCHDOG_WAKES.load(Ordering::Relaxed);
    let mut t1;
    let mut spins = 0u32;
    let mut pit_dead = false;
    loop {
        clock.update(pit2_latch_read());
        t1 = TICK_COUNT.load(Ordering::Relaxed);
        if clock.counts() >= HZ_WINDOW_COUNTS as u64 || spins >= HZ_WINDOW_SPIN_BOUND {
            break;
        }
        spins += 1;
        // `hlt` **bắt buộc**: vòng busy-wait (kể cả đọc PIT thưa) giữ BQL của QEMU, làm callback
        // PIT của main loop trễ ⇒ đo được 1..7 tick/1 s thay vì ~40. `hlt` nhường vCPU, QEMU
        // phục vụ timer rồi đánh thức bằng chính ngắt tick.
        asm::hlt();
        // **KT-413** — trần **hữu hạn** cho trường hợp bộ đếm PIT **chết** (cửa sổ không bao giờ
        // đóng): watchdog đã đánh thức `WATCHDOG_MAX_WAKES` lần mà PIT vẫn 0 count ⇒ thoát với
        // `pit_dead=1` (TAP 10 đỏ **có chẩn đoán**), không treo. Bình thường `clock.counts()`
        // tăng ngay từ vòng đầu nên nhánh này **không** chạy.
        let wakes = WATCHDOG_WAKES
            .load(Ordering::Relaxed)
            .saturating_sub(wakes0);
        if wakes >= WATCHDOG_MAX_WAKES && clock.counts() == 0 {
            pit_dead = true;
            break;
        }
    }
    // Mask lại **ngay**: nguồn tick của ảnh là PIT, watchdog chỉ tồn tại trong cửa sổ đo.
    let watchdog_masked_after = watchdog_disarm();
    let watchdog_wakes = WATCHDOG_WAKES
        .load(Ordering::Relaxed)
        .saturating_sub(wakes0);
    let window = clock.counts().min(u32::MAX as u64) as u32;
    let tick_delta = t1.saturating_sub(t0).min(u32::MAX as u64) as u32;
    let hz = apic::hz_from_pit(tick_delta, window);
    // `ticks_ok`: cửa sổ **đủ dài** + tick **tăng** + `hz` giao được trong khoảng hợp lệ. Cửa sổ
    // bị trần cắt (PIT hỏng) ⇒ `window < HZ_WINDOW_COUNTS` ⇒ ĐỎ; timer không tick ⇒
    // `tick_delta=0` ⇒ ĐỎ; nguồn tick quá chậm/vô lý ⇒ `hz_ok=false` ⇒ ĐỎ. Nguồn tick **sai
    // divisor** do `pit_divisor_ok` bắt ở TAP 10 (xem đầu hàm).
    //
    // **KT-413** — thêm hai điều kiện **đo được**: watchdog phải **arm** được và phải **mask lại**
    // sau cửa sổ (nếu không, LAPIC timer thành nguồn ngắt thứ hai suốt boot). `pit_dead` ⇒ ĐỎ.
    let ticks_ok = !pit_dead
        && window >= HZ_WINDOW_COUNTS
        && tick_delta >= 1
        && apic::hz_ok(hz)
        && watchdog_armed
        && watchdog_masked_after;

    TimerInfo {
        hz,
        ticks: t1,
        ticks_ok,
        period_pit: window,
        watchdog_wakes,
        watchdog_armed,
        watchdog_masked_after,
        pit_dead,
    }
}

/// **KT-413** (nợ `N-3`) — arm **watchdog LAPIC** trên vector riêng [`exc::VEC_WATCHDOG`].
///
/// Trả `true` ⇔ **đọc lại** `LVT_TIMER` thấy đúng vector watchdog và **không** mask (đo từ thanh
/// ghi, không tin hằng số — cùng khuôn `lvt_probe_masked` của `KT-348a`).
fn watchdog_arm() -> bool {
    lapic_write(apic::LAPIC_TIMER_DIVIDE, apic::TIMER_DIVIDE_BY_1);
    lapic_write(
        apic::LAPIC_LVT_TIMER,
        apic::lvt_timer_value(exc::VEC_WATCHDOG, true),
    );
    lapic_write(apic::LAPIC_TIMER_INITIAL_COUNT, WATCHDOG_INITIAL_COUNT);
    let raw = lapic_read(apic::LAPIC_LVT_TIMER);
    apic::lvt_vector(raw) == exc::VEC_WATCHDOG && !apic::lvt_is_masked(raw)
}

/// **KT-413** — mask redirection IRQ0 của IOAPIC (ca âm `--features no-pit-irq`).
///
/// Ghi **word thấp** với bit mask (`KT-348a` đã chốt thứ tự: `Destination` đã đúng từ
/// `timer_init`, nên chỉ cần bật mask). Sau lời gọi này **không** ngắt PIT nào tới BSP ⇒
/// `TICK_COUNT` đứng yên ⇒ `measure_timer` chỉ thoát được nhờ **watchdog LAPIC**.
fn mask_pit_irq() {
    if let Some(reg) = apic::ioapic_redtbl_reg(IOAPIC_PIT_PIN) {
        let low = ioapic_read(reg);
        ioapic_write(reg, low | apic::IOAPIC_REDIR_MASKED);
    }
}

/// **KT-413** — mask lại `LVT_TIMER` sau cửa sổ đo; trả `true` ⇔ **đọc lại** thấy đã mask.
fn watchdog_disarm() -> bool {
    lapic_write(
        apic::LAPIC_LVT_TIMER,
        apic::lvt_timer_value(exc::VEC_WATCHDOG, true) | apic::LVT_MASKED,
    );
    apic::lvt_is_masked(lapic_read(apic::LAPIC_LVT_TIMER))
}
/// **KT-346** (D2-252) — danh tính bản dựng của **ảnh sản phẩm**, in ngay sau `# conf image:`
/// để đóng nợ P20: trước KT-346 chỉ `riscv64` có dòng này, nên `repro-build.sh`/`repro-check.sh`
/// không chạy được cho hai arch còn lại. Giá trị do `build.rs` đặt qua `cargo:rustc-env` (tính
/// **tại thời điểm build** từ `v2/test/boot/build-rev.sh`); `inputs` là dấu vân tay **nội dung**
/// build-inputs (không phải sha256 ảnh — sha256 ảnh phụ thuộc thư mục build, xem §6.3 AGENTS).
pub(crate) const BUILD_LINE: &str = concat!(
    "# build: algo=",
    env!("VIFAST_BUILD_ALGO"),
    " inputs=",
    env!("VIFAST_BUILD_INPUTS"),
    " dirty=",
    env!("VIFAST_BUILD_DIRTY"),
    "\n"
);

// ---------------------------------------------------------------------------------------------
// kmain
// ---------------------------------------------------------------------------------------------

/// Điểm vào Rust, được `long_mode_entry` gọi **sau** khi long mode đã bật.
///
/// `boot_eax`/`boot_ebx` là hai thanh ghi giao thức boot lúc `_start` (PVH: `ebx` =
/// `hvm_start_info`; multiboot2: `eax = 0x36D76289`, `ebx` = MBI) — chỉ dùng để **in bằng chứng**.
/// `stack_top` = đỉnh stack kernel (`RSP0` của TSS), `ist1_top` = đỉnh stack IST1 cho `#DF`.
#[no_mangle]
pub extern "C" fn kmain(boot_eax: u32, boot_ebx: u32, stack_top: u64, ist1_top: u64) -> ! {
    // **KT-402** (D2-259) — `BootInfo` v1 từ PVH `hvm_start_info` (`%ebx`) + `fw_cfg` (E820,
    // MADT/FADT/MCFG thật). Bảng hỏng/thiếu ⇒ fail-closed có tên, không dùng hằng QEMU.
    let bootinfo = vifast_plat_x86_64::bootinfo::detect(boot_ebx);
    let divisor = serial_init();
    // **KT-276**: chốt `sp` ban đầu của BSP (`__stack_top` của linker) để bước SMP so nó với
    // slot 0 của arena — lệch nghĩa là linker và crate arch không còn khớp bố cục.
    #[cfg(feature = "ktest")]
    smp::set_bsp_stack_top(stack_top);
    let mut console = Console;
    // **KT-340a** — W^X kernel + `CR0.WP`: bật **trước** mọi pha (cả ảnh sản phẩm lẫn ảnh kiểm).
    let _ = product::wx_init();
    // **KT-340a** (D2-08): ảnh sản phẩm **không** vào harness — `product::boot` trả `!` nên
    // toàn bộ phần dưới là unreachable và bị loại khỏi ảnh (runner kiểm `strings`).
    if !product::KTEST {
        if !vifast_plat_x86_64::bootinfo::write_log(&bootinfo, &mut console) {
            asm::halt(9)
        }
        product::boot(
            boot_eax,
            boot_ebx,
            stack_top,
            ist1_top,
            divisor,
            &mut console,
        );
    }

    // **KT-340a** (D2-08): thân kiểm bị `cfg` gỡ khỏi ảnh sản phẩm **trước** name-resolution —
    // nên mô-đun kiểm không còn gốc tham chiếu và toàn bộ chuỗi harness bị loại khỏi `.rodata`.
    #[cfg(feature = "ktest")]
    {
        // TAP 13: dòng phiên bản phải là dòng **đầu tiên**; plan `1..N` ngay sau.
        //
        // Xuống dòng **trước** là bắt buộc trên x86: SeaBIOS in `Booting from ROM..` **không** kèm
        // newline, nên nếu in thẳng thì dòng đầu log là `Booting from ROM..TAP version 13` và runner
        // (`grep -qx 'TAP version 13'`) đỏ oan. Một newline đặt TAP về đúng dòng đầu của **phần ảnh**.
        let _ = writeln!(console);
        let _ = writeln!(console, "TAP version 13");
        let _ = writeln!(console, "1..{TAP_TESTS}");
        // **KT-402** — `# bootinfo:` (PVH + fw_cfg thật); bảng hỏng ⇒ halt có tên.
        if !vifast_plat_x86_64::bootinfo::write_log(&bootinfo, &mut console) {
            asm::halt(9)
        }

        // **KT-293 / D2-80** — neo ảnh: FNV-1a 64 trên `.text` **đã nạp** + độ dài, để runner/soak
        // đối chiếu với `test/conformance/image_digest.py` băm **cùng** section từ tệp ELF. In **ngay
        // sau** plan TAP (trước mọi ca kiểm) — dòng này là bằng chứng neo ảnh, phải có mặt kể cả khi
        // boot hỏng ở ca sau. Cùng định dạng `# conf image: build=<16 hex> text=<len>` của riscv64
        // (KT-57) và aarch64 (KT-265) ⇒ `soak-x86_64.sh` dùng chung một bộ đọc log.
        let (image_build, image_text) = vifast_plat_x86_64::image::image_digest();
        let _ = writeln!(
            console,
            "# conf image: build={image_build:016x} text={image_text}"
        );

        let cr0 = asm::read_cr0();
        let cr3 = asm::read_cr3();
        let cr4 = asm::read_cr4();
        let efer = asm::read_msr(boot::LONG_MODE_EFER_MSR);
        let lma = (efer & boot::LONG_MODE_EFER_CHECK) != 0;
        let pg = (cr0 & CR0_PG) != 0;
        let pe = (cr0 & CR0_PE) != 0;
        let pae = (cr4 & CR4_PAE) != 0;
        let pvh_magic = asm::read_u32(boot_ebx as u64);
        let pvh_ok = pvh_magic == boot::PVH_START_MAGIC;

        // Bằng chứng long mode: đọc lại **thanh ghi** sau khi bật, không chỉ tin asm.
        let _ = writeln!(
        console,
        "# viFastOs x86_64: long mode cr0=0x{cr0:016x} cr4=0x{cr4:016x} cr3=0x{cr3:016x} efer=0x{efer:016x} lma={}",
        lma as u8,
    );
        let _ = writeln!(
        console,
        "# viFastOs x86_64: boot eax=0x{boot_eax:08x} ebx=0x{boot_ebx:08x} pvh_magic=0x{pvh_magic:08x} pvh_ok={}",
        pvh_ok as u8,
    );
        // **KT-418** — cứng hóa SMEP/SMAP: bật **chỉ khi** `CPUID.07H:EBX` báo có (model mặc
        // định `qemu64` có thể không có ⇒ ghi `CR4` mù sẽ `#GP`), rồi đọc lại `CR4` làm bằng chứng.
        let smep_smap = asm::fpu::enable_smep_smap();
        let _ = writeln!(
            console,
            "# smep/smap: cpuid_smep={} cpuid_smap={} smep={} smap={} cr4=0x{:016x} hardened={}",
            smep_smap.cpuid_smep as u8,
            smep_smap.cpuid_smap as u8,
            smep_smap.smep as u8,
            smep_smap.smap as u8,
            smep_smap.cr4,
            smep_smap.hardened() as u8,
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
        // KT-266: MMIO map 3..4 GiB — LAPIC/IOAPIC nằm ngoài identity map 1 GiB.
        // KT-274: thêm MMIO map 2..3 GiB — ECAM của q35 (`0xB000_0000`) nằm dưới 3 GiB.
        let _ = writeln!(
            console,
            "# viFastOs x86_64: mmio map 0x{:x}..0x{:x} pages={} pcd={}",
            boot::MMIO_MAP_LOW_BASE,
            boot::MMIO_MAP_BASE + boot::MMIO_MAP_BYTES,
            boot::PD_ENTRIES * 2,
            (boot::PD_MMIO_FLAGS & 0x10) >> 4,
        );

        // Marker **chỉ** in khi long mode thật sự bật (LMA + PG + PE + PAE). Nếu in sớm, một ảnh hỏng
        // vẫn có marker và runner dễ nhận nhầm là boot sạch.
        if !(lma && pg && pe && pae) {
            let _ = writeln!(
                console,
                "PANIC: long mode chưa bật (lma={} pg={} pe={} pae={})",
                lma as u8, pg as u8, pe as u8, pae as u8,
            );
            asm::halt(1)
        }

        // ---- KT-261: TSS → GDT thật (7 entry) + `ltr` → IDT 256 ô + `lidt`.
        //
        // Thứ tự là hợp đồng: TSS phải hợp lệ **trước** khi descriptor trỏ vào nó, và GDT phải nạp
        // **trước** `ltr` (nếu không `ltr` đọc bảng tạm 3 entry ⇒ #GP).
        // **KT-278**: TSS/GDT là **per-hart** — hart 0 dùng slot 0, `IST1` là stack IST1 của slot 0.
        tss::init(0, stack_top, ist1_top);
        gdt::install(0, tss::base(0));
        idt::install();

        // ---- TAP 1: GDT đã nạp **thật** — `sgdt`/`str` đọc lại khớp bảng + descriptor TSS.
        let (gdt_base, gdt_limit) = gdt::read_gdtr(0);
        let tr = asm::str_reg();
        // Sau `ltr`, CPU đánh dấu descriptor TSS là **busy** (`0x89 → 0x8B`) — kiểm dạng busy
        // là bằng chứng `ltr` đã nạp thật, không chỉ "đã gọi".
        let want_tss =
            gdt::tss_descriptor_type(tss::base(0), tss::TSS_LIMIT as u32, gdt::TSS_TYPE_BUSY_64);
        let got_tss = [
            gdt::entry(0, gdt::INDEX_TSS),
            gdt::entry(0, gdt::INDEX_TSS + 1),
        ];
        let gdt_ok = gdt_base == gdt::base(0)
            && gdt_limit == gdt::GDT_LIMIT
            && tr == gdt::SELECTOR_TSS
            && got_tss == want_tss
            && tss::rsp0(0) == stack_top
            && tss::ist(0, 1) == ist1_top
            && tss::io_map_base(0) == tss::TSS_IO_MAP_BASE;
        let _ = writeln!(
        console,
        "# viFastOs x86_64: gdt base=0x{gdt_base:x} limit={gdt_limit} tr=0x{tr:x} tss=0x{:x} ist1=0x{ist1_top:x}",
        tss::base(0),
    );
        check(&mut console, 1, "exc: gdt loaded", gdt_ok);

        // ---- TAP 2: IDT đã nạp **thật** — `sidt` khớp bảng, ô `#BP` trỏ stub, `#DF` có IST1.
        let (idt_base, idt_limit) = idt::read_idtr();
        let bp = exc::VEC_BP as usize;
        let df = exc::VEC_DF as usize;
        let idt_ok = idt_base == idt::base()
            && idt_limit == idt::IDT_LIMIT
            && idt::offset_of(bp) == asm::stub_addr(exc::STUB_BP)
            && idt::selector_of(bp) == gdt::SELECTOR_KERNEL_CODE
            && idt::type_attr_of(bp) == idt::GATE_INTERRUPT
            && idt::ist_of(bp) == 0
            && idt::ist_of(df) == idt::IST_DOUBLE_FAULT
            && idt::offset_of(df) == asm::stub_addr(exc::STUB_DF);
        let _ = writeln!(
            console,
            "# viFastOs x86_64: idt base=0x{idt_base:x} limit={idt_limit} entries={} df_ist={}",
            idt::IDT_ENTRIES,
            idt::ist_of(df),
        );
        check(&mut console, 2, "exc: idt loaded", idt_ok);

        // ---- TAP 3: `int3` ⇒ `#BP` (vector 3, err 0), resume ở lệnh kế tiếp.
        exc_reset();
        exc_arm(VEC_BP, 0);
        asm::probe_int3();
        check(
            &mut console,
            3,
            "exc: int3",
            exc_caught(VEC_BP, 0, asm::probe_int3_next()),
        );

        // ---- TAP 4: `ud2` ⇒ `#UD` (vector 6, err 0).
        exc_reset();
        exc_arm(VEC_UD, 0);
        asm::probe_ud2();
        check(
            &mut console,
            4,
            "exc: ud2",
            exc_caught(VEC_UD, 0, asm::probe_ud2_insn()),
        );

        // ---- TAP 5: đọc địa chỉ **chưa map** ⇒ `#PF` (vector 14, `P = 0`, `cr2` đúng).
        exc_reset();
        exc_arm(VEC_PF, asm::probe_pf_resume());
        asm::probe_pf(PF_UNMAPPED_ADDR);
        let pf_err = EXC_ERR.load(Ordering::Relaxed);
        let pf_ok = exc_caught(VEC_PF, 0, asm::probe_pf_insn())
            && (pf_err & exc::PF_ERR_PRESENT) == 0
            && EXC_CR2.load(Ordering::Relaxed) == PF_UNMAPPED_ADDR;
        check(&mut console, 5, "exc: page fault", pf_ok);

        // ---- TAP 6: nạp `DS` bằng selector **ngoài** GDT ⇒ `#GP` (vector 13, err = selector).
        exc_reset();
        exc_arm(VEC_GP, asm::probe_gp_resume());
        asm::probe_gp();
        check(
            &mut console,
            6,
            "exc: gp",
            exc_caught(VEC_GP, GP_BAD_SELECTOR as u64, asm::probe_gp_insn()),
        );

        // ---- TAP 7: `int 0x30` ⇒ ô `isr_unknown` (sentinel 254) — mọi vector khác vẫn **thấy được**.
        exc_reset();
        exc_arm(exc::VEC_UNKNOWN, asm::probe_unknown_resume());
        asm::probe_unknown();
        check(
            &mut console,
            7,
            "exc: unknown",
            exc_caught(exc::VEC_UNKNOWN, 0, asm::probe_unknown_resume()),
        );

        // ---- KT-266: APIC + timer.
        //
        // Thứ tự: LAPIC (SVR/TPR) → IOAPIC (mask hết) → timer (LVT + initial count) → `sti` → đo.
        // Ngắt **chỉ** bật sau khi mọi thứ đã cấu hình: một ngắt đến sớm sẽ vào handler khi
        // `LAPIC_BASE`/`TICK_COUNT` chưa sẵn sàng.

        // ---- TAP 8: LAPIC — đọc được ID/version qua MMIO, `SVR` đã bật (bit 8) + vector spurious.
        let lapic = lapic_init();
        let lapic_id = apic::lapic_id(lapic.id_raw);
        let lapic_version = apic::lapic_version(lapic.version_raw);
        let lapic_ok = lapic.base == apic::LAPIC_DEFAULT_BASE
            && lapic.base_msr & apic::APIC_BASE_ENABLE != 0
            && lapic.svr & apic::SVR_APIC_ENABLE != 0
            && (lapic.svr & apic::SVR_VECTOR_MASK) == exc::VEC_SPURIOUS as u32
            && lapic_version != 0
            && lapic_version != 0xFF;
        let _ = writeln!(
            console,
            "# apic: id={lapic_id} version=0x{lapic_version:02x} svr=0x{:03x}",
            lapic.svr,
        );
        check(&mut console, 8, "apic: lapic id", lapic_ok);

        // ---- TAP 9: IOAPIC — đọc được `IOAPICID`/`IOAPICVER`, **mask toàn bộ** redirection entry.
        let ioapic = ioapic_init();
        let ioapic_id = apic::ioapic_id(ioapic.id_raw);
        let ioapic_version = apic::ioapic_version(ioapic.version_raw);
        let ioapic_ok = ioapic.entries >= 1
            && ioapic.entries <= apic::IOAPIC_MAX_REDIRECTIONS
            && ioapic_version != 0
            && ioapic_version != 0xFF
            && ioapic.all_masked;
        let _ = writeln!(
            console,
            "# apic: ioapic id={ioapic_id} ver=0x{ioapic_version:02x} entries={}",
            ioapic.entries,
        );
        check(&mut console, 9, "apic: ioapic", ioapic_ok);

        // ---- TAP 10/11: timer — cấu hình LAPIC timer + PIT, bật ngắt, đo tick + EOI.
        let timer_init = timer_init();
        // **KT-297** — divisor PIT đọc lại từ counter: dưới TCG `hz` đo được bão hoà ở tốc độ
        // **giao được** (độ trễ đánh thức `hlt`), nên kiểm divisor trực tiếp mới bắt được nguồn
        // tick sai nhịp (đột biến `wrong-hz`).
        let pit_div_ok = apic::pit_divisor_ok(timer_init.pit_counter0, PIT_DIVISOR_100HZ);
        let _ = writeln!(
        console,
        "# timer: lapic count0={} count1={} runs={} ioapic_route={} div_ok={} nominal_hz={} pit=0x{:04x}->0x{:04x}",
        timer_init.lapic_count0,
        timer_init.lapic_count1,
        timer_init.lapic_timer_runs as u8,
        timer_init.ioapic_route_ok as u8,
        pit_div_ok as u8,
        PIT_TICK_HZ,
        timer_init.pit_counter0,
        timer_init.pit_counter1,
    );
        // **KT-348a (D2-257)** — bằng chứng **thứ tự** hai cửa sổ ngắt (dòng riêng, không đổi
        // định dạng dòng `# timer: lapic …` mà `check_boot_log` neo bằng regex `$`):
        //   lvt_masked          — LVT mask trong suốt cửa sổ probe counter
        //   masked_before_unmask — redirection IRQ0 còn mask trước khi đổi `Destination`
        //   dest_bsp            — `Destination` sau khi mở mask = BSP LAPIC ID
        let _ = writeln!(
            console,
            "# timer: lapic-window lvt_masked={} masked_before_unmask={} dest_bsp={} order=dest_first",
            timer_init.lvt_probe_masked as u8,
            timer_init.ioapic_masked_before as u8,
            timer_init.ioapic_dest_ok as u8,
        );
        // **KT-413** — ca **âm** của nợ `N-3` (`--features no-pit-irq`): mask redirection IRQ0
        // **ngay trước** cửa sổ đo ⇒ không tick nào tới ⇒ `hlt` chỉ còn **watchdog LAPIC** đánh
        // thức. Điều kiện ĐẠT của ca âm: boot **vẫn thoát** (không treo) và TAP 10 **ĐỎ** vì
        // `ticks=0`. Chỉ dùng để chứng minh watchdog có răng, **không** phải feature sản phẩm.
        if cfg!(feature = "no-pit-irq") {
            mask_pit_irq();
        }
        asm::sti();
        let timer = measure_timer();
        asm::cli();
        // `EOI` phải bằng tick: handler ghi EOI **mỗi** tick, không thừa không thiếu. Đọc **sau**
        // `cli` để không có tick chen giữa hai lần đọc (KT-297: bỏ race của phép so cũ).
        let eoi_ok = EOI_COUNT.load(Ordering::Relaxed) == TICK_COUNT.load(Ordering::Relaxed);
        let _ = writeln!(
            console,
            "# timer: hz={} ticks={} window_pit={}",
            timer.hz, timer.ticks, timer.period_pit,
        );
        // **KT-413** (nợ `N-3`) — bằng chứng **watchdog LAPIC** (vector `0x32`): đã arm, đã đánh
        // thức `hlt` `wakes` lần, đã mask lại sau cửa sổ. `wakes > 0` là phép đo "watchdog **thật
        // sự** phát ngắt" — nếu LVT bị mask nhầm hoặc vector không có stub, `wakes=0` ⇒ TAP 10 ĐỎ.
        let _ = writeln!(
            console,
            "# timer: watchdog vector=0x{:02x} armed={} wakes={} masked_after={} pit_dead={} ok={}",
            exc::VEC_WATCHDOG,
            timer.watchdog_armed as u8,
            timer.watchdog_wakes,
            timer.watchdog_masked_after as u8,
            timer.pit_dead as u8,
            u8::from(
                timer.watchdog_armed
                    && timer.watchdog_wakes > 0
                    && timer.watchdog_masked_after
                    && !timer.pit_dead
            ),
        );
        check(
            &mut console,
            10,
            "timer: ticks",
            timer.ticks_ok
                && timer.hz > 0
                && timer_init.lapic_timer_runs
                && timer_init.ioapic_route_ok
                && timer_init.ioapic_masked_before
                && timer_init.ioapic_dest_ok
                && timer_init.lvt_probe_masked
                && pit_div_ok
                // **KT-413** (nợ `N-3`): watchdog phải **arm** được, **phát ngắt thật** (`wakes>0`)
                // và **mask lại** sau cửa sổ — thiếu một điều kiện là `hlt` có thể ngủ vô hạn.
                && timer.watchdog_armed
                && timer.watchdog_wakes > 0
                && timer.watchdog_masked_after
                && !timer.pit_dead,
        );
        let _ = writeln!(
            console,
            "# timer: eoi={} ticks={} spurious={}",
            EOI_COUNT.load(Ordering::Relaxed),
            TICK_COUNT.load(Ordering::Relaxed),
            SPURIOUS_COUNT.load(Ordering::Relaxed),
        );
        check(&mut console, 11, "timer: eoi", eoi_ok);

        let _ = writeln!(
            console,
            "# viFastOs x86_64: exc count={} last_vec={}",
            EXC_COUNT.load(Ordering::Relaxed),
            EXC_VEC.load(Ordering::Relaxed),
        );

        // ---- **KT-270** (K5' bước 4, D2-207): U-mode ring 3. Map trang mã/stack user đã dựng ở
        // `_start`, đặt `TSS.RSP0` sang stack syscall riêng rồi `iretq` xuống CPL3; **không trả về**
        // — đường `Exit` của ring 3 in marker **duy nhất** của ảnh rồi tắt máy. Marker chỉ in khi
        // TAP 1..14 đã xanh.
        // **KT-340c** — dựng `vifast-core::Machine` + rig **trước** lần `iretq` đầu tiên: mọi
        // `int 0x80` (kể cả `DebugPutStr`/`Exit` của lượt 1) đi qua `syscall::dispatch` của core.
        if !coretest::prepare(&mut console) {
            let _ = writeln!(console, "PANIC: coretest::prepare thất bại");
            asm::halt(3)
        }
        umode::run(&mut console)
    }

    // Ảnh sản phẩm đã halt trong `product::boot`; dòng dưới giữ kiểu trả về `!` của `kmain`
    // tường minh khi thân kiểm bị `cfg` gỡ (nhánh sản phẩm không chạy tới đây).
    #[cfg(not(feature = "ktest"))]
    asm::halt(0)
}
