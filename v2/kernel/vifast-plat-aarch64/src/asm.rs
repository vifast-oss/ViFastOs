// SPDX-License-Identifier: MIT OR Apache-2.0
//! Asm nguyên thủy AArch64 — **mô-đun duy nhất** của crate có `unsafe` (KT-167 / K4' bước 2).
//!
//! # Vì sao tách riêng
//!
//! Crate giữ `#![deny(unsafe_code)]` ở gốc; **chỉ** tệp này `#![allow(unsafe_code)]` (D2-17 cho
//! phép `unsafe` trong `kernel/vifast-plat-*`). Mọi phần dữ liệu ([`crate::sysreg`],
//! [`crate::tlb`], [`crate::barrier`], [`crate::format`], [`crate::mmu`]) vẫn **không** `unsafe`.
//!
//! # Host stub
//!
//! Mỗi hàm có hai bản: bản **bare-metal aarch64** (`target_arch = "aarch64"` **và**
//! `target_os = "none"`) là asm thật; bản host **panic** (không phải no-op) để `cargo test` trên
//! host vẫn biên dịch được mà không ai lỡ gọi asm đặc quyền trên máy thật. Test
//! `host_stub_is_fail_loud` chứng minh stub **không im lặng**.
//!
//! **Vì sao có thêm `target_os = "none"`**: máy phát triển của repo là **Apple Silicon**
//! (`aarch64-apple-darwin`) — chỉ `cfg(target_arch = "aarch64")` sẽ biên dịch asm đặc quyền
//! (`TLBI`, `msr`) vào **test host** và chạy nó ở EL0 ⇒ fault. Điều kiện đúng là "aarch64
//! bare-metal", không phải "aarch64 bất kỳ"; `cargo build --target aarch64-unknown-none` thoả
//! `target_os = "none"` nên vẫn dùng asm thật.
//!
//! # Encoding lấy từ đâu
//!
//! `TLBI`/barrier dùng `.inst <hằng>` với hằng ở [`crate::tlb`]/[`crate::barrier`] — nghĩa là
//! **đúng giá trị đã được test host so tuyệt đối** là giá trị phát ra instruction, không có
//! đường thứ hai để lệch. Các hằng này đã đối chiếu bằng hợp dịch thật (`clang` +
//! `llvm-objdump`), xem tài liệu hai tệp đó.
//!
//! # Sysreg
//!
//! `write_*`/`read_*` chỉ là `msr`/`mrs`; **không** tự chèn barrier. Caller phải gọi
//! [`isb`] sau khi ghi thanh ghi điều khiển MMU (`TCR_EL1`, `SCTLR_EL1`, `TTBR*`, `VBAR_EL1`) —
//! đúng yêu cầu context synchronization của đặc tả.

#![allow(unsafe_code)] // D2-17: crate `kernel/vifast-plat-*`; chỉ asm mới cần.

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
use core::arch::global_asm;

// Probe `brk #0` (KT-248) — **một** instruction, địa chỉ lấy được từ Rust để so `ELR_EL1`.
//
// Đặt ở tệp này (không phải ảnh) vì gọi một `extern "C"` fn cần `unsafe`, mà `unsafe` chỉ được
// nằm trong mô-đun này (D2-17 / luật R5). Ảnh gọi `brk_probe` — hàm an toàn.
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
global_asm!(
    ".section .text, \"ax\"",
    ".globl __vifast_brk_probe",
    "__vifast_brk_probe:",
    "brk #0",
    "ret",
);

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
extern "C" {
    /// Nhãn asm của probe — chỉ dùng để lấy địa chỉ và gọi.
    fn __vifast_brk_probe();
}

/// `PSCI SYSTEM_OFF` (function id 64-bit) — tắt máy qua hypervisor/QEMU.
///
/// Giá trị theo PSCI spec (ARM DEN 0022): `0x8400_0008` = `SYSTEM_OFF` trong dải function id
/// 64-bit (`0x8400_0000` + offset 8).
pub const PSCI_SYSTEM_OFF: u64 = 0x8400_0008;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
mod imp {
    use core::arch::asm;

    // ------------------------------------------------------------------ TLBI

    /// `tlbi vmalle1` — xoá toàn bộ TLB EL1&0 của PE hiện tại.
    pub fn tlbi_vmalle1() {
        // SAFETY: system instruction hợp lệ ở EL1; không đọc/ghi bộ nhớ, không đổi cờ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VMALLE1,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi vmalle1is` — xoá toàn bộ TLB EL1&0, broadcast inner-shareable.
    pub fn tlbi_vmalle1is() {
        // SAFETY: như `tlbi_vmalle1`; broadcast chỉ ảnh hưởng TLB của các PE cùng domain.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VMALLE1IS,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi vae1, x0` — `arg` = `VA[55:12]`.
    pub fn tlbi_vae1(arg: u64) {
        // SAFETY: `arg` được đặt vào x0 đúng như encoding `Rt = 0`; system instruction hợp lệ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VAE1,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi vae1is, x0` — `arg` = `VA[55:12]`, broadcast inner-shareable.
    pub fn tlbi_vae1is(arg: u64) {
        // SAFETY: như `tlbi_vae1`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_VAE1IS,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi aside1, x0` — `arg` = ASID (16 bit thấp).
    pub fn tlbi_aside1(arg: u64) {
        // SAFETY: `arg` được đặt vào x0 đúng như encoding `Rt = 0`; system instruction hợp lệ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_ASIDE1,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `tlbi aside1is, x0` — `arg` = ASID, broadcast inner-shareable.
    pub fn tlbi_aside1is(arg: u64) {
        // SAFETY: như `tlbi_aside1`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::tlb::TLBI_ASIDE1IS,
                in("x0") arg,
                options(nostack, preserves_flags),
            )
        };
    }

    // ------------------------------------------------------------------ barrier

    /// `dsb sy`.
    pub fn dsb_sy() {
        // SAFETY: barrier hợp lệ ở mọi EL; không đọc/ghi thanh ghi, không đổi cờ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DSB_SY,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dsb ish`.
    pub fn dsb_ish() {
        // SAFETY: như `dsb_sy`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DSB_ISH,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dsb ishst` — chờ **store** hoàn tất trong inner-shareable domain.
    ///
    /// Bước **đầu** của chuỗi `TLBI` (KT-262): phải chờ store vào bảng trang xong **trước** khi
    /// xoá entry TLB, nếu không hart khác có thể walk bảng trang khi lá mới chỉ ghi một nửa.
    pub fn dsb_ishst() {
        // SAFETY: như `dsb_sy`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DSB_ISHST,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dmb sy`.
    pub fn dmb_sy() {
        // SAFETY: barrier hợp lệ ở mọi EL; không đọc/ghi thanh ghi, không đổi cờ.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DMB_SY,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dmb ish`.
    pub fn dmb_ish() {
        // SAFETY: như `dmb_sy`.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::DMB_ISH,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `isb`.
    pub fn isb() {
        // SAFETY: context synchronization event hợp lệ ở mọi EL; không đọc/ghi thanh ghi.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::barrier::ISB,
                options(nostack, preserves_flags),
            )
        };
    }

    // ------------------------------------------------------------------ cache maintenance (KT-277)

    /// `dc civac, x0` — clean **và** invalidate dòng dữ liệu chứa `addr` tới PoC.
    ///
    /// Dùng **trước** khi bàn giao vùng DMA cho thiết bị: ghi bản bẩn ra PoC rồi bỏ bản sao
    /// trong cache. Encoding ở [`crate::cache::DC_CIVAC`] (đối chiếu `llvm-objdump`).
    pub fn dc_civac(addr: u64) {
        // SAFETY: `dc civac` là system instruction hợp lệ ở EL1; nó **không** đọc/ghi thanh ghi
        // kiến trúc, chỉ tác động cache của dòng chứa `addr`. `addr` do caller cấp và phải là
        // địa chỉ đã map trong bản đồ hiện hành.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::cache::DC_CIVAC,
                in("x0") addr,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `dc ivac, x0` — invalidate dòng dữ liệu chứa `addr` tới PoC.
    ///
    /// Dùng **sau** khi thiết bị ghi xong, **trước** khi CPU đọc dữ liệu thiết bị vừa ghi.
    pub fn dc_ivac(addr: u64) {
        // SAFETY: như `dc_civac` — system instruction hợp lệ ở EL1, không đọc/ghi thanh ghi.
        unsafe {
            asm!(
                ".inst {op}",
                op = const crate::cache::DC_IVAC,
                in("x0") addr,
                options(nostack, preserves_flags),
            )
        };
    }

    /// Đọc `CTR_EL0` — cỡ dòng cache dữ liệu nhỏ nhất (`DminLine[19:16]`).
    ///
    /// `CTR_EL0` đọc được ở EL1 (và EL0 nếu `SCTLR_EL1.UCT`), không cần cấu hình gì.
    pub fn read_ctr_el0() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi chỉ đọc `CTR_EL0`; không đọc/ghi bộ nhớ.
        unsafe { asm!("mrs {}, ctr_el0", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    // ------------------------------------------------------------------ MMIO

    /// Đọc 32 bit từ MMIO (thanh ghi thiết bị) — `ldr w` với địa chỉ tuyệt đối.
    ///
    /// **Vì sao ở đây**: `unsafe` chỉ được nằm trong mô-đun này (D2-17); driver PL011
    /// ([`crate::pl011`]) vì vậy **không** có `unsafe` mà vẫn truy cập được thanh ghi.
    pub fn mmio_read_u32(addr: u64) -> u32 {
        // SAFETY: `addr` do caller cấp và phải là địa chỉ MMIO của thiết bị đã map (PL011 ở
        // `0x0900_0000` trên QEMU virt). Đọc 4 byte căn lề tự nhiên; không ghi.
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }

    /// Ghi 32 bit ra MMIO (thanh ghi thiết bị).
    pub fn mmio_write_u32(addr: u64, value: u32) {
        // SAFETY: như `mmio_read_u32`; ghi 4 byte vào thanh ghi thiết bị đã map.
        unsafe { core::ptr::write_volatile(addr as *mut u32, value) };
    }

    /// Ghi **8 bit** ra MMIO (thanh ghi thiết bị).
    ///
    /// Cần cho `GICR_IPRIORITYR<n>` (KT-262): mỗi INTID một **byte**, nên ghi 32 bit vào
    /// `base + 0x400 + intid` là ghi **lệch lề** với `intid` không chia hết 4 — trên Device
    /// memory đó là **alignment fault** (`ec=0x25`, `FAR = 0x080B_0401`; đo được khi bật
    /// `SGI 1`/`SGI 2` cho hart 0, log `kt262-red.log`). KT-259 chỉ bật `SGI 0` nên chưa lộ.
    pub fn mmio_write_u8(addr: u64, value: u8) {
        // SAFETY: như `mmio_read_u32`; ghi 1 byte vào thanh ghi thiết bị đã map.
        unsafe { core::ptr::write_volatile(addr as *mut u8, value) };
    }

    /// Đọc **8 bit** từ MMIO (thanh ghi thiết bị) — KT-273 (PCI config space).
    pub fn mmio_read_u8(addr: u64) -> u8 {
        // SAFETY: như `mmio_read_u32`; đọc 1 byte, không yêu cầu căn lề.
        unsafe { core::ptr::read_volatile(addr as *const u8) }
    }

    /// Đọc **16 bit** từ MMIO — KT-273 (`vendor_id`/`command` của PCI config space).
    pub fn mmio_read_u16(addr: u64) -> u16 {
        // SAFETY: như `mmio_read_u32`; đọc 2 byte căn 2 (thanh ghi PCI 16 bit).
        unsafe { core::ptr::read_volatile(addr as *const u16) }
    }

    /// Ghi **16 bit** ra MMIO — KT-273 (`command` của PCI: bật memory + bus master).
    pub fn mmio_write_u16(addr: u64, value: u16) {
        // SAFETY: như `mmio_write_u32`; ghi 2 byte vào thanh ghi thiết bị đã map.
        unsafe { core::ptr::write_volatile(addr as *mut u16, value) };
    }

    /// Ghi **64 bit** ra MMIO — KT-273 (ba thanh ghi địa chỉ 64 bit của virtio-pci:
    /// `queue_desc`/`queue_driver`/`queue_device`).
    pub fn mmio_write_u64(addr: u64, value: u64) {
        // SAFETY: như `mmio_write_u32`; ghi 8 byte căn 8 vào thanh ghi thiết bị đã map.
        unsafe { core::ptr::write_volatile(addr as *mut u64, value) };
    }

    // ------------------------------------------------------------------ bộ đếm (KT-273)

    /// Đọc `CNTPCT_EL0` — bộ đếm **vật lý** của generic timer (luôn có trên AArch64, không cần
    /// bật `CNTP_CTL_EL0`).
    ///
    /// **Vì sao cần**: driver virtio-blk phải có **trần** khi poll `used` (spec §2.7.13: không
    /// có ngắt thì driver poll; poll vô hạn = treo im lặng). Bước 3 **chưa** có timer per-hart
    /// (nợ §7.1 của `k4-aarch64-boot.md`), nhưng `CNTPCT_EL0` là bộ đếm **đọc được** không cần
    /// cấu hình — đúng thứ cần cho một trần **thời gian** thay vì trần số vòng.
    pub fn read_cntpct_el0() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống đọc được ở EL1; không truy cập bộ nhớ.
        unsafe {
            core::arch::asm!("mrs {}, cntpct_el0", out(reg) value, options(nostack, preserves_flags))
        };
        value
    }

    /// Đọc `CNTFRQ_EL0` — tần số bộ đếm (Hz). QEMU `virt` đặt 62,5 MHz; firmware thật có thể
    /// khác nên driver **không** hardcode tần số, và chịu được giá trị 0 (xem `virtio_blk`).
    pub fn read_cntfrq_el0() -> u64 {
        let value: u64;
        // SAFETY: như `read_cntpct_el0`.
        unsafe {
            core::arch::asm!("mrs {}, cntfrq_el0", out(reg) value, options(nostack, preserves_flags))
        };
        value
    }

    // ------------------------------------------------------------------ trang DMA (KT-273)

    // ------------------------------------------------------------------ vùng RAM boot (KT-338a)

    /// Lát `AtomicU64` trên **arena bảng trang** ngoài ảnh — nguồn frame của [`crate::bootmem`].
    ///
    /// # Vì sao ở `asm`
    ///
    /// Dựng một `&'static [AtomicU64]` từ **địa chỉ số** cần `unsafe`; `asm` là mô-đun duy nhất
    /// của crate được `unsafe` (D2-17). Bản thân việc đọc/ghi entry vẫn là atomics an toàn trong
    /// [`crate::bootmem`].
    ///
    /// # Hợp đồng của caller
    ///
    /// `va` phải là base 4 KiB của vùng [`crate::bootmem::BOOT_RAM_BYTES`] byte RAM **ngoài ảnh**
    /// đã map **identity** (`VA == PA`) và ghi được. `va == 0` ⇔ arena chưa công bố: trả lát
    /// **rỗng** để `PageTable::new` **PANIC** `OutOfFrames` thay vì đọc địa chỉ 0 — fail-loud.
    pub fn arena_words(va: u64) -> &'static [core::sync::atomic::AtomicU64] {
        if va == 0 {
            return &[];
        }
        // SAFETY: xem "Hợp đồng của caller" — `va` là base arena do `bootmem::publish_arena`
        // công bố, map identity RW, và chỉ `bootmem` đọc/ghi qua `AtomicU64`.
        unsafe {
            core::slice::from_raw_parts(
                va as *const core::sync::atomic::AtomicU64,
                crate::bootmem::BOOT_ARENA_WORDS,
            )
        }
    }

    /// Xoá `bytes` byte ở PA `pa` của vùng RAM boot (KT-338a).
    ///
    /// Dùng cho frame **DMA** vừa cấp: arena nằm ngoài `.bss` nên `_start` **không** zero hộ.
    /// Ở thời điểm gọi, PA == VA (MMU tắt) hoặc vùng đã map **identity** (sau `activate`) — cùng
    /// một địa chỉ số.
    pub fn zero_boot_ram(pa: u64, bytes: usize) {
        // SAFETY: `pa` do caller lấy từ `bootmem` (frame vừa cấp từ arena), `bytes` không vượt
        // vùng `BOOT_RAM_BYTES`; vùng chưa bàn giao cho ai khác.
        unsafe { core::ptr::write_bytes(pa as *mut u8, 0, bytes) };
    }

    // ------------------------------------------------- trang DMA động (KT-273/275/279/283, KT-338a)

    /// PA của trang DMA thứ `index` — cấp **một lần** từ arena boot
    /// ([`crate::bootmem::alloc_boot_frame`]) rồi ghi nhớ; `index` ngoài
    /// [`crate::bootmem::BOOT_DMA_FRAMES`] là lỗi lập trình (panic).
    ///
    /// **Vì sao không còn `static`**: bốn trang `static` cũ chiếm 16 KiB `.bss` (D2-236). Nay
    /// chúng là frame **động** của arena ngoài ảnh. Trang DMA **phải identity-map** (`VA == PA`)
    /// vì (a) thiết bị DMA bằng PA, (b) `DmaPool::sync_for_device` phát `dc civac` trên **PA**,
    /// (c) `Dma::read_u16` đọc thẳng PA. `main.rs` map identity cả vùng RAM boot vào bảng boot
    /// **và** `VSpace` trước khi driver chạy — thiếu mapping ấy là fault thật, không im lặng.
    fn dma_page_pa(index: usize) -> u64 {
        static PA: [core::sync::atomic::AtomicU64; crate::bootmem::BOOT_DMA_FRAMES] =
            [const { core::sync::atomic::AtomicU64::new(0) }; crate::bootmem::BOOT_DMA_FRAMES];
        let known = PA[index].load(core::sync::atomic::Ordering::Acquire);
        if known != 0 {
            return known;
        }
        let pa = match crate::bootmem::alloc_boot_frame() {
            Some(pa) => pa,
            None => panic!("arena boot hết frame cho trang DMA (KT-338a)"),
        };
        // Frame vừa cấp chưa ai zero (arena nằm ngoài `.bss`) — zero **trước** khi bàn giao.
        zero_boot_ram(pa, 4096);
        PA[index].store(pa, core::sync::atomic::Ordering::Release);
        pa
    }

    /// PA trang DMA của `virtio-blk` (KT-273) — **PA**, không phải alias.
    ///
    /// Trước KT-338a hàm này trả **VA** (nửa cao sau `activate`) và caller đổi sang PA bằng
    /// `virtio_blk::high_to_pa`; nay vùng RAM boot map identity nên PA **là** VA, và caller dùng
    /// thẳng — descriptor của thiết bị luôn cần PA.
    pub fn dma_page_addr() -> u64 {
        dma_page_pa(0)
    }

    /// Trang DMA của `virtio-blk` dưới dạng lát byte ghi được (4 KiB), tại **identity VA**
    /// (`VA == PA`).
    pub fn dma_page() -> &'static mut [u8; 4096] {
        // SAFETY: `dma_page_pa(0)` cấp một lần, đã zero, map identity RW; hợp đồng một hart +
        // driver tự tuần tự hoá (một request một lúc) giữ nguyên như trang `.bss` cũ.
        unsafe { &mut *(dma_page_pa(0) as *mut [u8; 4096]) }
    }

    /// PA trang DMA của `virtio-net` (KT-275) — **PA**, không phải alias (xem [`dma_page_addr`]).
    pub fn dma_page_net_addr() -> u64 {
        dma_page_pa(1)
    }

    /// Trang DMA của `virtio-net` dưới dạng lát byte ghi được (4 KiB), tại identity VA.
    pub fn dma_page_net() -> &'static mut [u8; 4096] {
        // SAFETY: như `dma_page` — một hart, driver tự tuần tự hoá.
        unsafe { &mut *(dma_page_pa(1) as *mut [u8; 4096]) }
    }

    /// PA trang DMA của `virtio-gpu` (KT-279) — **PA**, không phải alias (xem [`dma_page_addr`]).
    pub fn dma_page_gpu_addr() -> u64 {
        dma_page_pa(2)
    }

    /// Trang DMA của `virtio-gpu` dưới dạng lát byte ghi được (4 KiB), tại identity VA.
    pub fn dma_page_gpu() -> &'static mut [u8; 4096] {
        // SAFETY: như `dma_page` — một hart, driver tự tuần tự hoá.
        unsafe { &mut *(dma_page_pa(2) as *mut [u8; 4096]) }
    }

    /// PA trang DMA của `virtio-input` (KT-283) — **PA**, không phải alias (xem [`dma_page_addr`]).
    pub fn dma_page_input_addr() -> u64 {
        dma_page_pa(3)
    }

    /// Trang DMA của `virtio-input` dưới dạng lát byte ghi được (4 KiB), tại identity VA.
    pub fn dma_page_input() -> &'static mut [u8; 4096] {
        // SAFETY: như `dma_page` — một hart, driver tự tuần tự hoá.
        unsafe { &mut *(dma_page_pa(3) as *mut [u8; 4096]) }
    }

    // ------------------------------------------------------------------ framebuffer GPU (KT-279)

    /// Lát byte ghi được trên vùng RAM **ngoài ảnh** — framebuffer của virtio-gpu (KT-279).
    ///
    /// **Vì sao ở `asm`**: đây là bộ nhớ thô chia sẻ với **thiết bị** (DMA), cùng loại với bốn
    /// trang DMA động ở trên; `asm` là mô-đun duy nhất của crate được `unsafe` (D2-17). Framebuffer
    /// **không** nằm trong `.bss` (1.228.800 B sẽ vượt trần I7 262.144 B) mà ở dải PA
    /// `virtio_gpu::FB_PHYS_BASE`, đã map **identity** (`VA == PA`) trong `VSpace` của ảnh.
    ///
    /// # Hợp đồng của caller
    ///
    /// `addr` phải là base của một cửa sổ đã map `KERNEL_RW` + `MemAttr::Normal`, `len` phải nằm
    /// **trọn** trong cửa sổ ấy, và không ai khác (trừ DMA của thiết bị) giữ tham chiếu tới vùng
    /// này. Ảnh boot gọi **một** lần qua `virtio_gpu::framebuffer()`.
    pub fn gpu_framebuffer(addr: u64, len: usize) -> &'static mut [u8] {
        // SAFETY: xem "Hợp đồng của caller" — `addr`/`len` do `virtio_gpu` cấp từ hằng đã kiểm
        // `const _: () = assert!` (trong RAM, sau vùng ảnh, căn 2 MiB) và cửa sổ đã map trong
        // `main.rs` trước `activate`.
        unsafe { core::slice::from_raw_parts_mut(addr as *mut u8, len) }
    }

    // ------------------------------------------------------------------ bộ nhớ thô (KT-249)

    /// Đọc 64 bit tại **địa chỉ tuyệt đối** `addr` (VA sau khi MMU bật, PA trước đó).
    ///
    /// Dùng cho ca kiểm "MMU thật sự bật": đọc cùng một trang qua **VA không identity** — nếu
    /// MMU chưa bật thì địa chỉ ấy là PA và trả giá trị khác. `unsafe` nằm ở đây (D2-17).
    pub fn read_u64(addr: u64) -> u64 {
        // SAFETY: `addr` do caller cấp và phải là địa chỉ đọc được trong bản đồ hiện hành
        // (identity map của kernel hoặc VA đã map tường minh). Đọc 8 byte căn lề tự nhiên.
        unsafe { core::ptr::read_volatile(addr as *const u64) }
    }

    /// Ghi 64 bit tại **địa chỉ tuyệt đối** `addr` (xem [`read_u64`]).
    pub fn write_u64(addr: u64, value: u64) {
        // SAFETY: như `read_u64`; ghi 8 byte vào vùng nhớ đã map và ghi được.
        unsafe { core::ptr::write_volatile(addr as *mut u64, value) };
    }

    /// Đọc **8 bit** tại **địa chỉ tuyệt đối** `addr` (xem [`read_u64`]).
    ///
    /// Cần cho [`crate::image::image_digest`] (KT-265): hash FNV-1a 64 chạy trên **đúng** từng
    /// byte của `.text` — đọc `u64` sẽ vượt qua `__text_end` ở đoạn đuôi và băm cả byte của
    /// section kế tiếp. `unsafe` nằm ở đây (D2-17), caller là hàm an toàn.
    pub fn read_u8(addr: u64) -> u8 {
        // SAFETY: `addr` do caller cấp và phải là địa chỉ đọc được trong bản đồ hiện hành
        // (identity map của kernel hoặc VA đã map tường minh). Đọc 1 byte, không yêu cầu căn lề.
        unsafe { core::ptr::read_volatile(addr as *const u8) }
    }

    // ------------------------------------------------------------------ probe BRK (KT-248)

    /// `brk #0` — trap **có chủ đích** vào ô `SyncCurrentElSpX` (`0x200`) của bảng vector.
    ///
    /// `ELR_EL1` của `BRK` là **địa chỉ instruction `BRK`** (không phải `+4`), nên handler phải
    /// tự cộng 4 khi resume — xem `vec_handler` của ảnh. Hàm này **trả về** sau khi handler
    /// resume; nếu chưa cài bảng vector thì `brk` đi vào `VBAR_EL1 = 0` và treo (đúng: probe chỉ
    /// dùng sau khi đã cài).
    pub fn brk_probe() {
        // SAFETY: `brk #0` là instruction hợp lệ ở EL1; nó **không** đọc/ghi bộ nhớ, chỉ sinh
        // exception đồng bộ. Handler của ảnh resume bằng `ELR += 4`.
        unsafe { super::__vifast_brk_probe() };
    }

    /// Địa chỉ instruction `brk #0` của [`brk_probe`] — để handler so với `ELR_EL1`.
    pub fn brk_probe_addr() -> u64 {
        super::__vifast_brk_probe as *const () as u64
    }

    // ------------------------------------------------------------------ PSCI / dừng máy

    /// `PSCI SYSTEM_OFF` qua `hvc #0` với `x0 = 0x8400_0008` (PSCI v1.x, 64-bit calling
    /// convention, function id `0x8400_0008` = `SYSTEM_OFF`).
    ///
    /// QEMU `virt` đặt PSCI conduit = **HVC** khi khách khởi động ở EL1 (đúng đường `-kernel`
    /// của bước 3); `SYSTEM_OFF` làm QEMU thoát với mã 0. Hàm **trả về** nếu PSCI không có
    /// (hypervisor khác/không hỗ trợ) — caller phải có đường dự phòng, không giả định đã tắt.
    pub fn psci_system_off() {
        // SAFETY: `hvc #0` là lệnh gọi hypervisor hợp lệ ở EL1; `x0` mang function id PSCI.
        // Không đọc/ghi bộ nhớ của kernel.
        unsafe {
            asm!(
                "hvc #0",
                in("x0") super::PSCI_SYSTEM_OFF,
                options(nostack, preserves_flags),
            )
        };
    }

    /// `wfi` trong vòng lặp vô hạn — đường dừng **dự phòng** khi PSCI không tắt được máy.
    ///
    /// Không phải "treo im lặng": caller đã in marker/`PANIC` trước đó, và runner phân biệt
    /// đường sạch với đường treo bằng chính các dòng ấy (xem `v2/test/boot/run-aarch64.sh`).
    pub fn wfi_forever() -> ! {
        loop {
            // SAFETY: `wfi` chỉ chờ ngắt; không truy cập bộ nhớ, không đổi thanh ghi.
            unsafe { asm!("wfi", options(nostack, preserves_flags, nomem)) };
        }
    }

    // ------------------------------------------------------------------ nửa cao (KT-253)

    /// Nhảy sang **VA nửa cao**: đặt `sp` rồi `br` tới `entry` — **không** trả về.
    ///
    /// Dùng thanh ghi cố định (`x9`/`x10`) thay vì `in(reg)` để thứ tự hai lệnh không phụ thuộc
    /// bộ cấp phát thanh ghi: `sp` phải đổi **trước** khi `br` tới code nửa cao (code ấy dùng
    /// stack ngay ở lệnh đầu tiên nếu nó gọi hàm).
    ///
    /// # An toàn (caller)
    ///
    /// `entry` phải là VA **đã map** trong bảng nửa cao và `[sp - …, sp)` phải là vùng stack đã
    /// map ở nửa cao. Hàm này không kiểm — nó là bước cuối của đường boot, sai thì fault ngay
    /// (bảng vector in `# trap:` rồi dừng, không im lặng).
    pub fn jump_high(entry: u64, sp: u64) -> ! {
        // SAFETY: caller đã map `entry` + vùng stack quanh `sp` trong bảng nửa cao; `br` không
        // đọc/ghi bộ nhớ nào khác. `options(noreturn)` vì không có đường quay lại.
        unsafe {
            asm!(
                "mov sp, x9",
                "br x10",
                in("x9") sp,
                in("x10") entry,
                options(noreturn),
            )
        }
    }

    /// Địa chỉ instruction hiện tại (`adr x, .`) — bằng chứng **PC đang thật sự ở VA nửa cao**.
    ///
    /// Đây là phép đo không thể giả: nếu chỉ in một hằng số thì không chứng minh được gì; đọc
    /// PC thật rồi so với VA nửa cao mong đợi mới chứng minh instruction fetch đi qua `TTBR1`.
    pub fn current_pc() -> u64 {
        let pc: u64;
        // SAFETY: `adr` chỉ đọc PC vào thanh ghi, không truy cập bộ nhớ.
        unsafe { asm!("adr {pc}, .", pc = out(reg) pc) };
        pc
    }

    // ------------------------------------------------------------------ sysreg: ghi

    /// Ghi `TCR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_tcr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Tcr`.
        unsafe { asm!("msr tcr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `MAIR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_mair_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Mair`.
        unsafe { asm!("msr mair_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `SCTLR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_sctlr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Sctlr`.
        unsafe { asm!("msr sctlr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `TTBR0_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_ttbr0_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Ttbr`.
        unsafe { asm!("msr ttbr0_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `TTBR1_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_ttbr1_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Ttbr`.
        unsafe { asm!("msr ttbr1_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `VBAR_EL1` (gọi [`isb`] sau khi ghi).
    pub fn write_vbar_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Vbar`.
        unsafe { asm!("msr vbar_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `ELR_EL1`.
    pub fn write_elr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::ElrEl1`.
        unsafe { asm!("msr elr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `SPSR_EL1`.
    pub fn write_spsr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1; giá trị do caller dựng từ `sysreg::Spsr`.
        unsafe { asm!("msr spsr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `DAIF` (mask/unmask ngắt).
    pub fn write_daif(value: u64) {
        // SAFETY: `msr daif` chỉ đổi bốn bit mask ngắt của PSTATE; giá trị từ `sysreg::Daif`.
        unsafe { asm!("msr daif, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    // ------------------------------------------------------------------ sysreg: đọc

    /// Đọc `TCR_EL1`.
    pub fn read_tcr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc, không tác dụng phụ.
        unsafe { asm!("mrs {}, tcr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `MAIR_EL1`.
    pub fn read_mair_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, mair_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `SCTLR_EL1`.
    pub fn read_sctlr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, sctlr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `TTBR0_EL1`.
    pub fn read_ttbr0_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, ttbr0_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `TTBR1_EL1`.
    pub fn read_ttbr1_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, ttbr1_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `VBAR_EL1`.
    pub fn read_vbar_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, vbar_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `ESR_EL1` (nguyên nhân trap).
    pub fn read_esr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, esr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `FAR_EL1` (địa chỉ gây fault).
    pub fn read_far_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, far_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `ELR_EL1`.
    pub fn read_elr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, elr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `SPSR_EL1`.
    pub fn read_spsr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1; chỉ đọc.
        unsafe { asm!("mrs {}, spsr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `DAIF`.
    pub fn read_daif() -> u64 {
        let value: u64;
        // SAFETY: `mrs daif` chỉ đọc bốn bit mask ngắt của PSTATE.
        unsafe { asm!("mrs {}, daif", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    // ------------------------------------------------------------------ SMP (KT-259)

    /// Đọc `MPIDR_EL1` — **hart id thật** của CPU đang chạy (không tin `x3` của PSCI).
    pub fn read_mpidr_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs mpidr_el1` chỉ đọc thanh ghi nhận dạng CPU; không truy cập bộ nhớ.
        unsafe { asm!("mrs {}, mpidr_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Đọc `sp` hiện tại (`mov x, sp`) — bằng chứng "hart này đang chạy trên stack nào".
    pub fn read_current_sp() -> u64 {
        let value: u64;
        // SAFETY: `mov` từ `sp` chỉ đọc thanh ghi; không truy cập bộ nhớ.
        unsafe { asm!("mov {}, sp", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// `PSCI CPU_ON(target_cpu, entry_pa, context_id)` qua `hvc #0` — trả `x0` (mã PSCI).
    ///
    /// PSCI v1.x, calling convention 64-bit: `x0 = 0xC400_0003`, `x1 = target_cpu`,
    /// `x2 = entry_pa`, `x3 = context_id`. QEMU `virt` đặt conduit = **HVC** (DTB `psci.method =
    /// "hvc"`), nên `hvc #0` là đường đúng; `smc #0` sẽ trap (không có EL3).
    ///
    /// Hàm **trả về** mã PSCI (kể cả lỗi) — caller phải kiểm, không giả định đã đánh thức được.
    pub fn psci_cpu_on(target_cpu: u64, entry_pa: u64, context_id: u64) -> i64 {
        let rc: i64;
        // SAFETY: `hvc #0` là lệnh gọi hypervisor hợp lệ ở EL1; bốn thanh ghi tham số do PSCI
        // quy định. Không đọc/ghi bộ nhớ của kernel.
        unsafe {
            asm!(
                "hvc #0",
                inout("x0") crate::smp::PSCI_CPU_ON => rc,
                in("x1") target_cpu,
                in("x2") entry_pa,
                in("x3") context_id,
                options(nostack, preserves_flags),
            )
        };
        rc
    }

    /// `msr daifclr, #2` — **mở** mask IRQ (`DAIF.I = 0`). Chỉ hart nhận IPI gọi.
    pub fn daif_clear_irq() {
        // SAFETY: `daifclr` chỉ đổi mask ngắt của PSTATE của CPU hiện tại.
        unsafe { asm!("msr daifclr, #2", options(nostack, preserves_flags)) };
    }

    /// `sev` — đánh thức các CPU đang `wfe` (rào chắn giao tiếp giữa hart).
    pub fn sev() {
        // SAFETY: `sev` chỉ phát sự kiện tới các CPU khác; không truy cập bộ nhớ.
        unsafe { asm!("sev", options(nostack, preserves_flags, nomem)) };
    }

    /// `wfe` — chờ **sự kiện** (do `sev` của hart khác, hoặc một ngắt).
    ///
    /// Dùng trong vòng chờ lệnh của hart phụ (KT-262) thay cho spin: `sev` đặt event register
    /// **kể cả khi hart phụ chưa vào `wfe`**, nên không có lost-wakeup như `wfi` (SGI tới trước
    /// `wfi` thì `wfi` ngủ tiếp vì không còn ngắt chờ).
    pub fn wfe() {
        // SAFETY: `wfe` chỉ chờ sự kiện; không truy cập bộ nhớ, không đổi thanh ghi.
        unsafe { asm!("wfe", options(nostack, preserves_flags, nomem)) };
    }

    /// Ghi `ICC_SRE_EL1` (bật/tắt system register interface của GICv3 CPU interface).
    pub fn write_icc_sre_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1 của GICv3; giá trị do `crate::gic` dựng.
        unsafe { asm!("msr icc_sre_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Đọc `ICC_SRE_EL1` — bằng chứng `SRE = 1` (nếu 0 thì mọi ghi `ICC_PMR`/`ICC_IGRPEN1` bị bỏ).
    pub fn read_icc_sre_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1 của GICv3; chỉ đọc.
        unsafe { asm!("mrs {}, icc_sre_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Ghi `ICC_PMR_EL1` — mặt nạ ưu tiên (`0xF0` = cho qua mọi ưu tiên `0x00..0xEF`).
    pub fn write_icc_pmr_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1 của GICv3; giá trị do `crate::gic` dựng.
        unsafe { asm!("msr icc_pmr_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `ICC_IGRPEN1_EL1` — bật nhóm 1 của CPU interface hiện tại.
    pub fn write_icc_igrpen1_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1 của GICv3; giá trị do `crate::gic` dựng.
        unsafe {
            asm!("msr icc_igrpen1_el1, {}", in(reg) value, options(nostack, preserves_flags))
        };
    }

    /// Đọc `ICC_IAR1_EL1` — INTID của ngắt nhóm 1 đang chờ (`>= 1020` là spurious).
    pub fn read_icc_iar1_el1() -> u64 {
        let value: u64;
        // SAFETY: `mrs` thanh ghi hệ thống EL1 của GICv3; chỉ đọc (đọc `IAR1` còn "nhận" ngắt —
        // đó là ngữ nghĩa của thanh ghi, không phải tác dụng phụ ẩn).
        unsafe { asm!("mrs {}, icc_iar1_el1", out(reg) value, options(nostack, preserves_flags)) };
        value
    }

    /// Ghi `ICC_EOIR1_EL1` — kết thúc ngắt `intid` (không ghi thì ngắt không tới lại).
    pub fn write_icc_eoir1_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1 của GICv3; giá trị là INTID vừa đọc từ `IAR1`.
        unsafe { asm!("msr icc_eoir1_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    /// Ghi `ICC_SGI1R_EL1` — gửi SGI tới danh sách hart đích (encoding ở `crate::gic::sgi1r`).
    pub fn write_icc_sgi1r_el1(value: u64) {
        // SAFETY: `msr` thanh ghi hệ thống EL1 của GICv3; giá trị do `crate::gic` dựng từ hart id.
        unsafe { asm!("msr icc_sgi1r_el1, {}", in(reg) value, options(nostack, preserves_flags)) };
    }

    // ------------------------------------------------------------------ U-mode (KT-269)

    /// Đọc một ô của **khung trap** do stub vector dựng (`x0..x18, x29, x30` — 21 ô).
    ///
    /// `frame` là `sp` của stub sau `sub sp, sp, #176`; ô `index` nằm ở `frame + index*8`.
    /// Dùng `read_volatile`: khung do asm ghi, compiler **không** được gộp/đoán giá trị.
    ///
    /// # An toàn (caller)
    ///
    /// `frame` phải là khung 176 B còn sống trên stack kernel và `index < FRAME_SLOTS`; handler
    /// của ảnh chỉ đọc ba ô cố định (`FRAME_X0`/`FRAME_X1`/`FRAME_X8`).
    pub fn frame_get(frame: u64, index: usize) -> u64 {
        // SAFETY: `frame` do stub vector truyền vào (khung 176 B trên stack kernel); `index` do
        // caller giữ < 21. `read_volatile` để không bị tối ưu thành đọc thanh ghi.
        unsafe { core::ptr::read_volatile((frame as *const u64).add(index)) }
    }

    /// Ghi một ô của khung trap — dùng để đặt `x0`/`x1` (`error`/`value`) trước `eret`.
    ///
    /// # An toàn (caller)
    ///
    /// Như [`frame_get`]: `frame` là khung 176 B còn sống, `index < FRAME_SLOTS`.
    pub fn frame_set(frame: u64, index: usize, value: u64) {
        // SAFETY: như `frame_get` — cùng khung, cùng bất biến `index`.
        unsafe { core::ptr::write_volatile((frame as *mut u64).add(index), value) }
    }

    /// `eret` xuống **EL0**: đặt `SP_EL0`, `ELR_EL1`, `SPSR_EL1` rồi `eret` — **không** trả về.
    ///
    /// Thứ tự bắt buộc: `SP_EL0` trước (EL0 dùng nó ngay ở lệnh đầu nếu stub gọi hàm), rồi
    /// `ELR_EL1`/`SPSR_EL1`, `ISB` (context synchronization cho `eret`), rồi `eret`.
    ///
    /// `spsr` phải có `M[3:0] = 0b0000` (EL0t) — hàm **không** kiểm (caller dựng bằng
    /// [`crate::umode::spsr_el0`]). Nếu SPSR sai chế độ, `eret` về EL1 và handler bắt được
    /// (`# umode: svc from EL1`) — đó là đột biến (i) của KT-269, không phải đường sản phẩm.
    ///
    /// # An toàn (caller)
    ///
    /// `entry` phải là VA **đã map** `user` + exec trong bảng trang đang hoạt động, và
    /// `[sp - …, sp)` phải là stack user đã map `user` + write. Hàm không kiểm — sai thì fault
    /// từ EL0 và handler in `# umode: fault` rồi dừng (không im lặng).
    pub fn eret_to_el0(entry: u64, sp: u64, spsr: u64) -> ! {
        // SAFETY: caller đã map `entry` (user RX) và vùng stack quanh `sp` (user RW) trong bảng
        // trang đang hoạt động; `eret` không trả về nên `options(noreturn)`.
        unsafe {
            asm!(
                "msr sp_el0, {sp}",
                "msr elr_el1, {entry}",
                "msr spsr_el1, {spsr}",
                "isb",
                "eret",
                sp = in(reg) sp,
                entry = in(reg) entry,
                spsr = in(reg) spsr,
                options(noreturn),
            )
        }
    }
}

#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
mod imp {
    /// Stub host: **panic** thay vì no-op — gọi asm đặc quyền trên host là lỗi lập trình.
    fn host_stub(name: &str) -> ! {
        panic!("vifast-plat-aarch64: {name} chỉ chạy trên aarch64 (host stub, KT-167)")
    }

    /// Stub host của `tlbi vmalle1`.
    pub fn tlbi_vmalle1() {
        host_stub("tlbi vmalle1")
    }
    /// Stub host của `tlbi vmalle1is`.
    pub fn tlbi_vmalle1is() {
        host_stub("tlbi vmalle1is")
    }
    /// Stub host của `tlbi vae1`.
    pub fn tlbi_vae1(_arg: u64) {
        host_stub("tlbi vae1")
    }
    /// Stub host của `tlbi vae1is`.
    pub fn tlbi_vae1is(_arg: u64) {
        host_stub("tlbi vae1is")
    }
    /// Stub host của `tlbi aside1`.
    pub fn tlbi_aside1(_arg: u64) {
        host_stub("tlbi aside1")
    }
    /// Stub host của `tlbi aside1is`.
    pub fn tlbi_aside1is(_arg: u64) {
        host_stub("tlbi aside1is")
    }
    /// Stub host của `dsb sy`.
    pub fn dsb_sy() {
        host_stub("dsb sy")
    }
    /// Stub host của `dsb ish`.
    pub fn dsb_ish() {
        host_stub("dsb ish")
    }
    /// Stub host của `dsb ishst`.
    pub fn dsb_ishst() {
        host_stub("dsb ishst")
    }
    /// Stub host của `dmb sy`.
    pub fn dmb_sy() {
        host_stub("dmb sy")
    }
    /// Stub host của `dmb ish`.
    pub fn dmb_ish() {
        host_stub("dmb ish")
    }
    /// Stub host của `isb`.
    pub fn isb() {
        host_stub("isb")
    }
    /// Stub host của `dc civac` — **panic**: trên host không có cache của khách để làm sạch.
    pub fn dc_civac(_addr: u64) {
        host_stub("dc civac")
    }
    /// Stub host của `dc ivac` — **panic**: như `dc_civac`.
    pub fn dc_ivac(_addr: u64) {
        host_stub("dc ivac")
    }
    /// Stub host của `CTR_EL0` — **panic**: trên host không có `CTR_EL0` của khách.
    pub fn read_ctr_el0() -> u64 {
        host_stub("mrs ctr_el0")
    }
    /// Stub host của MMIO đọc — **panic**: trên host không có thiết bị PL011 đã map.
    pub fn mmio_read_u32(_addr: u64) -> u32 {
        host_stub("mmio read u32")
    }
    /// Stub host của MMIO ghi — **panic**: trên host không có thiết bị PL011 đã map.
    pub fn mmio_write_u32(_addr: u64, _value: u32) {
        host_stub("mmio write u32")
    }
    /// Stub host của MMIO ghi 8 bit — **panic**: như `mmio_write_u32`.
    pub fn mmio_write_u8(_addr: u64, _value: u8) {
        host_stub("mmio write u8")
    }
    /// Stub host của MMIO đọc 8 bit — **panic**: như `mmio_read_u32`.
    pub fn mmio_read_u8(_addr: u64) -> u8 {
        host_stub("mmio read u8")
    }
    /// Stub host của MMIO đọc 16 bit — **panic**: như `mmio_read_u32`.
    pub fn mmio_read_u16(_addr: u64) -> u16 {
        host_stub("mmio read u16")
    }
    /// Stub host của MMIO ghi 16 bit — **panic**: như `mmio_write_u32`.
    pub fn mmio_write_u16(_addr: u64, _value: u16) {
        host_stub("mmio write u16")
    }
    /// Stub host của MMIO ghi 64 bit — **panic**: như `mmio_write_u32`.
    pub fn mmio_write_u64(_addr: u64, _value: u64) {
        host_stub("mmio write u64")
    }
    /// Stub host của `CNTPCT_EL0` — **panic**: trên host không có generic timer của khách.
    pub fn read_cntpct_el0() -> u64 {
        host_stub("mrs cntpct_el0")
    }
    /// Stub host của `CNTFRQ_EL0` — **panic**: như `read_cntpct_el0`.
    pub fn read_cntfrq_el0() -> u64 {
        host_stub("mrs cntfrq_el0")
    }
    /// Stub host của trang DMA — **panic**: trên host không có vùng DMA của ảnh.
    pub fn dma_page_addr() -> u64 {
        host_stub("dma page addr")
    }
    /// Stub host của trang DMA — **panic**: như `dma_page_addr`.
    pub fn dma_page() -> &'static mut [u8; 4096] {
        host_stub("dma page")
    }
    /// Stub host của trang DMA thứ hai (virtio-net) — **panic**: như `dma_page_addr`.
    pub fn dma_page_net_addr() -> u64 {
        host_stub("dma page net addr")
    }
    /// Stub host của trang DMA thứ hai (virtio-net) — **panic**: như `dma_page_addr`.
    pub fn dma_page_net() -> &'static mut [u8; 4096] {
        host_stub("dma page net")
    }
    /// Stub host của trang DMA thứ ba (virtio-gpu) — **panic**: như `dma_page_addr`.
    pub fn dma_page_gpu_addr() -> u64 {
        host_stub("dma page gpu addr")
    }
    /// Stub host của trang DMA thứ ba (virtio-gpu) — **panic**: như `dma_page_addr`.
    pub fn dma_page_gpu() -> &'static mut [u8; 4096] {
        host_stub("dma page gpu")
    }
    /// Stub host của trang DMA thứ tư (virtio-input) — **panic**: như `dma_page_addr`.
    pub fn dma_page_input_addr() -> u64 {
        host_stub("dma page input addr")
    }
    /// Stub host của trang DMA thứ tư (virtio-input) — **panic**: như `dma_page_addr`.
    pub fn dma_page_input() -> &'static mut [u8; 4096] {
        host_stub("dma page input")
    }
    /// Stub host của lát arena boot — **panic**: trên host không có vùng RAM boot ngoài ảnh.
    pub fn arena_words(va: u64) -> &'static [core::sync::atomic::AtomicU64] {
        let _ = va;
        host_stub("boot arena words")
    }
    /// Stub host của zero vùng RAM boot — **panic**: như `arena_words`.
    pub fn zero_boot_ram(_pa: u64, _bytes: usize) {
        host_stub("zero boot ram")
    }
    /// Stub host của framebuffer GPU — **panic**: trên host không có cửa sổ RAM ngoài ảnh.
    pub fn gpu_framebuffer(_addr: u64, _len: usize) -> &'static mut [u8] {
        host_stub("gpu framebuffer")
    }
    /// Stub host của đọc bộ nhớ thô — **panic**: trên host không có bản đồ của kernel.
    pub fn read_u64(_addr: u64) -> u64 {
        host_stub("read u64")
    }
    /// Stub host của ghi bộ nhớ thô — **panic**: trên host không có bản đồ của kernel.
    pub fn write_u64(_addr: u64, _value: u64) {
        host_stub("write u64")
    }
    /// Stub host của đọc 8 bit — **panic**: trên host không có bản đồ của kernel.
    pub fn read_u8(_addr: u64) -> u8 {
        host_stub("read u8")
    }
    /// Stub host của `brk #0` — **panic**: trên host `brk` sẽ giết tiến trình test.
    pub fn brk_probe() {
        host_stub("brk #0")
    }
    /// Stub host của địa chỉ probe — **panic**: không có asm trên host.
    pub fn brk_probe_addr() -> u64 {
        host_stub("brk #0 addr")
    }
    /// Stub host của `PSCI SYSTEM_OFF` — **panic**: trên host không có PSCI.
    pub fn psci_system_off() {
        host_stub("psci system_off")
    }
    /// Stub host của `wfi` vô hạn — **panic**: trên host không được treo tiến trình test.
    pub fn wfi_forever() -> ! {
        host_stub("wfi forever")
    }
    /// Stub host của nhảy nửa cao — **panic**: trên host không có bảng trang nửa cao.
    pub fn jump_high(_entry: u64, _sp: u64) -> ! {
        host_stub("jump high")
    }
    /// Stub host của đọc PC — **panic**: không có asm trên host.
    pub fn current_pc() -> u64 {
        host_stub("current pc")
    }
    /// Stub host của `msr tcr_el1`.
    pub fn write_tcr_el1(_value: u64) {
        host_stub("msr tcr_el1")
    }
    /// Stub host của `msr mair_el1`.
    pub fn write_mair_el1(_value: u64) {
        host_stub("msr mair_el1")
    }
    /// Stub host của `msr sctlr_el1`.
    pub fn write_sctlr_el1(_value: u64) {
        host_stub("msr sctlr_el1")
    }
    /// Stub host của `msr ttbr0_el1`.
    pub fn write_ttbr0_el1(_value: u64) {
        host_stub("msr ttbr0_el1")
    }
    /// Stub host của `msr ttbr1_el1`.
    pub fn write_ttbr1_el1(_value: u64) {
        host_stub("msr ttbr1_el1")
    }
    /// Stub host của `msr vbar_el1`.
    pub fn write_vbar_el1(_value: u64) {
        host_stub("msr vbar_el1")
    }
    /// Stub host của `msr elr_el1`.
    pub fn write_elr_el1(_value: u64) {
        host_stub("msr elr_el1")
    }
    /// Stub host của `msr spsr_el1`.
    pub fn write_spsr_el1(_value: u64) {
        host_stub("msr spsr_el1")
    }
    /// Stub host của `msr daif`.
    pub fn write_daif(_value: u64) {
        host_stub("msr daif")
    }
    /// Stub host của `mrs tcr_el1`.
    pub fn read_tcr_el1() -> u64 {
        host_stub("mrs tcr_el1")
    }
    /// Stub host của `mrs mair_el1`.
    pub fn read_mair_el1() -> u64 {
        host_stub("mrs mair_el1")
    }
    /// Stub host của `mrs sctlr_el1`.
    pub fn read_sctlr_el1() -> u64 {
        host_stub("mrs sctlr_el1")
    }
    /// Stub host của `mrs ttbr0_el1`.
    pub fn read_ttbr0_el1() -> u64 {
        host_stub("mrs ttbr0_el1")
    }
    /// Stub host của `mrs ttbr1_el1`.
    pub fn read_ttbr1_el1() -> u64 {
        host_stub("mrs ttbr1_el1")
    }
    /// Stub host của `mrs vbar_el1`.
    pub fn read_vbar_el1() -> u64 {
        host_stub("mrs vbar_el1")
    }
    /// Stub host của `mrs esr_el1`.
    pub fn read_esr_el1() -> u64 {
        host_stub("mrs esr_el1")
    }
    /// Stub host của `mrs far_el1`.
    pub fn read_far_el1() -> u64 {
        host_stub("mrs far_el1")
    }
    /// Stub host của `mrs elr_el1`.
    pub fn read_elr_el1() -> u64 {
        host_stub("mrs elr_el1")
    }
    /// Stub host của `mrs spsr_el1`.
    pub fn read_spsr_el1() -> u64 {
        host_stub("mrs spsr_el1")
    }
    /// Stub host của `mrs daif`.
    pub fn read_daif() -> u64 {
        host_stub("mrs daif")
    }
    /// Stub host của `mrs mpidr_el1` — **panic**: trên host không có `MPIDR_EL1` của hart.
    pub fn read_mpidr_el1() -> u64 {
        host_stub("mrs mpidr_el1")
    }
    /// Stub host của `mov x, sp` — **panic**: `sp` của tiến trình test không phải stack hart.
    pub fn read_current_sp() -> u64 {
        host_stub("mov x, sp")
    }
    /// Stub host của `PSCI CPU_ON` — **panic**: trên host không có PSCI.
    pub fn psci_cpu_on(_target_cpu: u64, _entry_pa: u64, _context_id: u64) -> i64 {
        host_stub("psci cpu_on")
    }
    /// Stub host của `msr daifclr, #2` — **panic**: không mở mask ngắt trên tiến trình test.
    pub fn daif_clear_irq() {
        host_stub("msr daifclr")
    }
    /// Stub host của `sev` — **panic**: không phát sự kiện tới CPU thật từ host.
    pub fn sev() {
        host_stub("sev")
    }
    /// Stub host của `wfe` — **panic**: không chờ sự kiện CPU thật từ host.
    pub fn wfe() {
        host_stub("wfe")
    }
    /// Stub host của `msr icc_sre_el1`.
    pub fn write_icc_sre_el1(_value: u64) {
        host_stub("msr icc_sre_el1")
    }
    /// Stub host của `mrs icc_sre_el1`.
    pub fn read_icc_sre_el1() -> u64 {
        host_stub("mrs icc_sre_el1")
    }
    /// Stub host của `msr icc_pmr_el1`.
    pub fn write_icc_pmr_el1(_value: u64) {
        host_stub("msr icc_pmr_el1")
    }
    /// Stub host của `msr icc_igrpen1_el1`.
    pub fn write_icc_igrpen1_el1(_value: u64) {
        host_stub("msr icc_igrpen1_el1")
    }
    /// Stub host của `mrs icc_iar1_el1`.
    pub fn read_icc_iar1_el1() -> u64 {
        host_stub("mrs icc_iar1_el1")
    }
    /// Stub host của `msr icc_eoir1_el1`.
    pub fn write_icc_eoir1_el1(_value: u64) {
        host_stub("msr icc_eoir1_el1")
    }
    /// Stub host của `msr icc_sgi1r_el1`.
    pub fn write_icc_sgi1r_el1(_value: u64) {
        host_stub("msr icc_sgi1r_el1")
    }
    /// Stub host của đọc khung trap — **panic**: trên host không có khung trap của stub vector.
    pub fn frame_get(_frame: u64, _index: usize) -> u64 {
        host_stub("frame get")
    }
    /// Stub host của ghi khung trap — **panic**: như `frame_get`.
    pub fn frame_set(_frame: u64, _index: usize, _value: u64) {
        host_stub("frame set")
    }
    /// Stub host của `eret` xuống EL0 — **panic**: trên host không có EL0 của kernel.
    pub fn eret_to_el0(_entry: u64, _sp: u64, _spsr: u64) -> ! {
        host_stub("eret to el0")
    }
}

pub use imp::*;
