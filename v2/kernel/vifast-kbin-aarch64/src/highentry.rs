// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338c** — miền **nửa cao + VSpace + SMP + U-mode** của ảnh aarch64 (tách khỏi `main.rs`).
//!
//! [`high_entry`] là điểm vào **sau** bước nhảy của KT-253: PC/SP ở **VA nửa cao** (qua `TTBR1`).
//! Nó đo ba thứ bằng bảng trang thật (PC thật, physmap, VSpace `activate`), rồi mới mở các pha
//! cần `VSpace`/GIC: `TLBI` shootdown đa hart (KT-262), PCI + virtio (KT-273/275/279/283/289),
//! W^X + cache DMA (KT-277) và cuối cùng `eret` xuống EL0 (KT-269) — marker của ảnh in ở
//! `umode::el0_done`.
//!
//! **Tách tệp** (KT-338c) để `main.rs` về dưới trần R1 theo **miền** chứ không cắt cơ học: toàn
//! bộ phần "sau khi MMU + nửa cao đã sống" nằm ở đây, `bootmmu.rs` giữ phần trước bước nhảy.

use core::fmt::Write;
use core::sync::atomic::Ordering;

use vifast_plat_aarch64::bootmem;
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::mmu::MapError;
use vifast_plat_aarch64::pci;
use vifast_plat_aarch64::resources::{GIC, GIC_CPU, UART0};
use vifast_plat_aarch64::shootdown::Targets;
use vifast_plat_aarch64::split_mmu::{high_alias_va, KERNEL_HIGH_BASE, PHYSMAP_BASE};
use vifast_plat_aarch64::virtio_gpu;
use vifast_plat_aarch64::vspace::{Frame, VSpace};
use vifast_plat_aarch64::{asm, debug_halt, gic, smp, vector};
use vifast_plat_common::fault::{FaultAccess, VmFaultReason};

use crate::bootmmu::{
    HIGH_ENTRY_COUNT, HIGH_EXPECT_PA, HIGH_EXPECT_VA, HIGH_TRANSLATED_PA, PHYSMAP_MAGIC,
    PHYSMAP_PROBE_PA, PHYSMAP_PROBE_VA, TEXT_START_PA, VECTOR_BASE_PA, VSPACE_ID, VSPACE_MAGIC,
    VSPACE_PROBE_PA, VSPACE_VA,
};
use crate::mapwin::{map_identity_into, map_window_blocks_into, map_window_into};
use crate::{bootram, ecam, faults, gpu, input, smpboot, tap, umode, wxprobe, Console};
/// **KT-253** — chạy ở **VA nửa cao** (được [`asm::jump_high`] nhảy tới).
///
/// Phép đo (không thể giả):
///
/// 1. `adr` đọc **PC thật** — phải nằm trong trang của [`HIGH_EXPECT_VA`] (VA nửa cao);
/// 2. `translate(HIGH_EXPECT_VA)` bằng walk phần mềm (làm **trước** khi nhảy) phải bằng
///    [`HIGH_EXPECT_PA`];
/// 3. đọc PA của trang [`PHYS_PROBE`] qua **cửa sổ physmap** phải ra [`PHYSMAP_MAGIC`] — chứng
///    minh physmap đọc được PA bất kỳ đã map, không cần identity.
///
/// Sau đó cài lại `VBAR_EL1` bằng **VA nửa cao** của bảng vector (từ đây cả exception vector
/// cũng chạy ở nửa cao), in marker rồi tắt máy sạch.
#[no_mangle]
pub extern "C" fn high_entry() -> ! {
    let mut console = Console;
    HIGH_ENTRY_COUNT.fetch_add(1, Ordering::Relaxed);
    let _ = writeln!(console, "# high: jump ok");

    // ---- Đo 1: PC thật (đọc bằng `adr`) + PA suy ra từ alias.
    let pc = asm::current_pc();
    let expect_va = HIGH_EXPECT_VA.load(Ordering::Relaxed);
    let expect_pa = HIGH_EXPECT_PA.load(Ordering::Relaxed);
    let translated = HIGH_TRANSLATED_PA.load(Ordering::Relaxed);
    // PA liên kết đọc **trước** khi nhảy (xem `TEXT_START_PA`): sau khi nhảy, `adrp` cho alias
    // nửa cao của symbol chứ không phải PA.
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    let pc_pa = text_start + (pc - KERNEL_HIGH_BASE);
    let same_page = (pc >> 12) == (expect_va >> 12);
    // `translate` trả PA **đáy trang** (descriptor chỉ giữ bit [47:12]) ⇒ so với đáy trang.
    let high_ok = same_page && translated == (expect_pa & !0xFFF);
    let _ = writeln!(
        console,
        "# high: va=0x{pc:x} pa=0x{pc_pa:x} match={} expect_va=0x{expect_va:x} expect_pa=0x{expect_pa:x} translated=0x{translated:x} same_page={}",
        high_ok as u8,
        same_page as u8,
    );

    // ---- Đo 2: physmap đọc PA đã ghi trước đó (qua VA thấp, trước khi nhảy).
    let physmap_va = PHYSMAP_PROBE_VA.load(Ordering::Relaxed);
    let physmap_pa = PHYSMAP_PROBE_PA.load(Ordering::Relaxed);
    let read = asm::read_u64(physmap_va);
    let physmap_ok = read == PHYSMAP_MAGIC;
    let _ = writeln!(
        console,
        "# physmap: pa=0x{physmap_pa:x} read=0x{read:016x} match={} va=0x{physmap_va:x} expect=0x{PHYSMAP_MAGIC:016x} base=0x{PHYSMAP_BASE:x}",
        physmap_ok as u8,
    );

    // ---- Cài lại VBAR_EL1 bằng VA **nửa cao** của bảng vector: từ đây cả exception vector
    // cũng đi qua `TTBR1` (nếu map nửa cao của ảnh sai, fault tại đây sẽ lộ ngay).
    let high_vector = high_alias_va(
        VECTOR_BASE_PA.load(Ordering::Relaxed),
        text_start,
        KERNEL_HIGH_BASE,
    );
    let vbar = vector::install(high_vector);
    let _ = writeln!(
        console,
        "# high: vbar=0x{vbar:x} high=0x{high_vector:x} match={}",
        (vbar == high_vector) as u8,
    );

    if !tap(
        &mut console,
        6,
        "high: PC at high VA, translate matches PA",
        high_ok,
    ) {
        debug_halt(5)
    }
    if !tap(
        &mut console,
        7,
        "physmap: read PA via physmap window",
        physmap_ok,
    ) {
        debug_halt(5)
    }

    // ---- KT-254 (B3.5): `VSpace` thật — map **một Frame 4 KiB**, `walk`, `activate`, `unmap`,
    // rồi đọc qua VA sau `unmap` để sinh fault `FaultKind::Vm`.
    let probe_pa = VSPACE_PROBE_PA.load(Ordering::Relaxed);
    // **KT-338a**: arena ngoài ảnh, PA đã công bố; `words` là lát trên VA **identity** nên dùng
    // được cả trước lẫn sau `activate`.
    let (low_mem, high_mem) = bootmem::shared_boot_memory();
    let mut vs = match VSpace::create(VSPACE_ID, low_mem, high_mem) {
        Ok(vs) => vs,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: create lỗi {error:?}");
            debug_halt(3)
        }
    };
    // VSpace phải **tự có** alias nửa cao + physmap của kernel: `activate` đổi cả `TTBR1`, nên
    // nếu thiếu, lệnh kế tiếp (PC/SP/static ở nửa cao) fault ngay.
    //
    // **KT-277**: cả **ba** đường (alias nửa cao, physmap, identity nửa thấp) map **W^X** qua
    // `wxprobe::map_vspace_image` — một định nghĩa duy nhất cho quyền của vùng ảnh, và quyền được
    // **đọc lại từ bảng trang** để dòng `# wx:` là bằng chứng chứ không phải lời khai.
    //
    // **Ảnh kernel identity** trong bảng nửa thấp là bắt buộc, không phải tuỳ chọn: ảnh liên kết
    // ở **PA** (`0x4008_0000`), nên `core::fmt` (vtable của `&dyn Display`) và mọi con trỏ hàm
    // trong `.rodata` là **địa chỉ tuyệt đối nửa thấp**. Đổi `TTBR0` sang bảng không map ảnh ⇒
    // `blr` tới vtable fault (đo được: prefetch abort `FAR=0x4008_0ab0`). Map với `user = false`
    // ⇒ EL0 **không** đọc được ảnh kernel (đúng bất biến cách ly).
    let image_pages = match wxprobe::map_vspace_image(&mut vs, &mut console) {
        Some(pages) => pages,
        None => debug_halt(3),
    };
    // KT-338a: identity-map vùng RAM boot trong `VSpace` (trang DMA/cache dùng PA; walk để đo).
    let pages_boot_ram = bootram::map_vspace(&mut vs, &mut console, bootmem::arena_base());
    let pages_high = image_pages.high;
    let pages_physmap = image_pages.phys;
    let pages_image = image_pages.identity;
    // UART + GIC identity (Device) — kernel vẫn in được sau khi `TTBR0` đổi sang bảng VSpace.
    let pages_uart = match map_identity_into(
        &mut vs,
        UART0.base,
        UART0.end(),
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map uart lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_gic = match map_identity_into(
        &mut vs,
        GIC.base,
        GIC_CPU.end(),
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map gic lỗi {error:?}");
            debug_halt(3)
        }
    };
    // **KT-259**: vùng **redistributor** GICv3 (`0x080A_0000 + hart × 0x2_0000`) — KT-254 chỉ map
    // distributor + CPU interface, nên nếu không map thêm ở đây thì lần đọc `GICR_WAKER` đầu tiên
    // sau `activate` sẽ **translation fault** (đo được khi dựng KT-259: `# trap: … ec=0x25`).
    // Map cả dải `MAX_HARTS` hart (1 MiB) — cùng một bảng mức 3 với GICD nên **không** tốn frame.
    let pages_gicr = match map_identity_into(
        &mut vs,
        gic::GICR_BASE,
        gic::GICR_BASE + (smp::MAX_HARTS as u64) * gic::GICR_STRIDE,
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: map gicr lỗi {error:?}");
            debug_halt(3)
        }
    };
    // ---- **KT-273** (K4' bước 3): PCI ECAM + cửa sổ BAR của virtio-blk.
    //
    // ECAM của QEMU `virt` aarch64 ở PA `0x40_1000_0000` — **không** canonical 39-bit và
    // `PHYSMAP_BASE + PA` **tràn u64**, nên phải map vào một **cửa sổ VA riêng**
    // (`pci::ECAM_VA_BASE = 0x8000_0000`, nối tiếp dãy VA 0x5/0x6/0x7 đã dùng). Cửa sổ BAR
    // (`0x1000_0000`, 2 MiB) thì identity-map được — cùng khuôn riscv64.
    //
    // Kiểm **trước** khi map: cửa sổ BAR phải **chưa** có mapping nào (nếu có, gán BAR vào đó là
    // đè im lặng — CAO-2/D2-110C). Đây là lớp kiểm **bảng trang** mà `pci::init_for` không làm
    // được (nó không có `VSpace`), bù lại `pci::RESERVED_RANGES` đã kiểm lúc biên dịch.
    if vs.walk(pci::BAR_ASSIGN_BASE) != Err(MapError::NotMapped) {
        let _ = writeln!(
            console,
            "PANIC: pci: cửa sổ BAR 0x{:x} đã có mapping trước khi gán",
            pci::BAR_ASSIGN_BASE
        );
        debug_halt(3)
    }
    // **KT-275**: leaf BAR của **virtio-net** (leaf 2, `0x1040_0000`) — mỗi loại thiết bị một
    // leaf 2 MiB (`pci::bar_assign_base`), nên driver mạng **không** đụng leaf của virtio-blk.
    // Kiểm "chưa có mapping" **trước** khi map, cùng lý do CAO-2/D2-110C ở trên.
    let net_bar_base = pci::bar_assign_base(pci::VirtioKind::Net);
    if vs.walk(net_bar_base) != Err(MapError::NotMapped) {
        let _ = writeln!(
            console,
            "PANIC: pci: cửa sổ BAR mạng 0x{net_bar_base:x} đã có mapping trước khi gán"
        );
        debug_halt(3)
    }
    // **KT-302**: ECAM 256 MiB map bằng **block descriptor 2 MiB** ở mức 2 (128 block trong
    // **một** bảng mức 2 ⇒ 2 frame), không phải 128 bảng mức 3 (129 frame — vượt arena 64 frame
    // (KT-338a) và trần `.data+.bss` I7). `map_window_blocks_into` từ chối nếu VA/PA/cỡ không căn 2 MiB.
    let ecam_blocks = match map_window_blocks_into(
        &mut vs,
        pci::ECAM_VA_BASE,
        pci::ECAM_BASE,
        pci::ECAM_MAP_BYTES,
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(blocks) => blocks,
        Err(error) => {
            let _ = writeln!(console, "PANIC: pci: map ECAM lỗi {error:?}");
            debug_halt(3)
        }
    };
    // Bằng chứng **đo được** rằng ECAM map bằng block mức 2 (walk bảng trang thật) — KT-302.
    let (ecam_block_pa, ecam_block_level, ecam_block_ok) = ecam::probe_block(&vs, &mut console);
    let pages_bar = match map_window_into(
        &mut vs,
        pci::BAR_ASSIGN_BASE,
        pci::BAR_ASSIGN_BASE,
        pci::BAR_ASSIGN_BYTES,
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: pci: map cửa sổ BAR lỗi {error:?}");
            debug_halt(3)
        }
    };
    let pages_bar_net = match map_window_into(
        &mut vs,
        net_bar_base,
        net_bar_base,
        pci::BAR_ASSIGN_BYTES,
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: pci: map cửa sổ BAR mạng lỗi {error:?}");
            debug_halt(3)
        }
    };
    // ---- **KT-279**: leaf BAR của **virtio-gpu** (leaf 4, `0x1080_0000`) — mỗi loại thiết bị một
    // leaf 2 MiB (`pci::bar_assign_base`), nên driver GPU **không** đụng leaf blk/net. Kiểm "chưa
    // có mapping" **trước** khi map, cùng lý do CAO-2/D2-110C ở trên.
    let gpu_bar_base = pci::bar_assign_base(pci::VirtioKind::Gpu);
    if vs.walk(gpu_bar_base) != Err(MapError::NotMapped) {
        let _ = writeln!(
            console,
            "PANIC: pci: cửa sổ BAR GPU 0x{gpu_bar_base:x} đã có mapping trước khi gán"
        );
        debug_halt(3)
    }
    let pages_bar_gpu = match map_window_into(
        &mut vs,
        gpu_bar_base,
        gpu_bar_base,
        pci::BAR_ASSIGN_BYTES,
        Perms::KERNEL_RW,
        MemAttr::Device,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: pci: map cửa sổ BAR GPU lỗi {error:?}");
            debug_halt(3)
        }
    };
    // ---- **KT-279**: cửa sổ **framebuffer** của virtio-gpu — 300 trang RAM **ngoài ảnh**
    // (`virtio_gpu::FB_PHYS_BASE = 0x4200_0000`), map **identity** (`VA == PA`) với `Normal` (đây
    // là RAM, không phải MMIO) để driver vẽ và thiết bị DMA đọc **cùng** một dải PA. Căn 2 MiB ⇒
    // **một** bảng mức 3 (1 frame arena) phủ trọn 300 trang.
    //
    // Vì sao **không** `.bss`: 640×480×4 = 1.228.800 B > trần `.data+.bss` 262.144 B (I7) — xem
    // bảng ba phương án đã đo trong tài liệu đầu `virtio_gpu.rs`.
    if vs.walk(virtio_gpu::FB_PHYS_BASE) != Err(MapError::NotMapped) {
        let _ = writeln!(
            console,
            "PANIC: gpu: cửa sổ framebuffer 0x{:x} đã có mapping trước khi map",
            virtio_gpu::FB_PHYS_BASE
        );
        debug_halt(3)
    }
    let pages_fb = match map_window_into(
        &mut vs,
        virtio_gpu::FB_PHYS_BASE,
        virtio_gpu::FB_PHYS_BASE,
        virtio_gpu::FB_BYTES as u64,
        Perms::KERNEL_RW,
        MemAttr::Normal,
    ) {
        Ok(pages) => pages,
        Err(error) => {
            let _ = writeln!(console, "PANIC: gpu: map framebuffer lỗi {error:?}");
            debug_halt(3)
        }
    };
    // PA **đọc lại từ bảng trang** cho VA của framebuffer — `gpu.rs` in `identity=` từ số này,
    // nên "framebuffer map identity" là phép **đo**, không phải lời khai.
    let fb_walk_pa = match vs.walk(virtio_gpu::FB_PHYS_BASE) {
        Ok(translation) => translation.pa,
        Err(error) => {
            let _ = writeln!(console, "PANIC: gpu: walk framebuffer lỗi {error:?}");
            debug_halt(3)
        }
    };
    gpu::publish_fb_walk(fb_walk_pa);
    // ---- **KT-283**: leaf BAR của **virtio-input** (leaf 3, `0x1060_0000`) + PA **đọc lại từ
    // bảng trang** của trang DMA. Chi tiết ở `input::map_window` (giữ `main.rs` dưới trần R1).
    let (input_bar_base, pages_bar_input, input_dma_va, input_dma_walk_pa) =
        match input::map_window(&mut vs) {
            Ok(measured) => measured,
            Err(stage) => {
                let _ = writeln!(console, "PANIC: input: map BAR/trang DMA lỗi {stage}");
                debug_halt(3)
            }
        };
    let _ = writeln!(
        console,
        "# pci: map ecam_pa=0x{:x} ecam_va=0x{:x} ecam_blocks={ecam_blocks} ecam_block_level={ecam_block_level} ecam_block_pa=0x{ecam_block_pa:x} ecam_block_ok={} bar_va=0x{:x} bar_pages={pages_bar} net_bar_va=0x{net_bar_base:x} net_bar_pages={pages_bar_net} gpu_bar_va=0x{gpu_bar_base:x} gpu_bar_pages={pages_bar_gpu} input_bar_va=0x{input_bar_base:x} input_bar_pages={pages_bar_input} fb_pa=0x{:x} fb_pages={pages_fb} fb_walk=0x{fb_walk_pa:x} input_dma_va=0x{input_dma_va:x} input_dma_walk=0x{input_dma_walk_pa:x} frames_used={}",
        pci::ECAM_BASE,
        pci::ECAM_VA_BASE,
        ecam_block_ok as u8,
        pci::BAR_ASSIGN_BASE,
        virtio_gpu::FB_PHYS_BASE,
        vs.mmu().low().memory().frames_used(),
    );

    // **KT-305**: `translate`/`unmap` **hiểu** block descriptor — tự kiểm trên bảng trang thật
    // của ảnh (chi tiết ở `ecam.rs`), in `# pci: unmap … blocks_split=1` cho runner.
    if let Err(error) = ecam::probe(&mut vs, &mut console) {
        let _ = writeln!(console, "PANIC: pci: probe block lỗi {error:?}");
        debug_halt(3)
    }

    // **Một Frame 4 KiB** vào VA người dùng (ngoài RAM, không identity).
    let frame = match Frame::new(probe_pa) {
        Some(frame) => frame,
        None => {
            let _ = writeln!(console, "PANIC: vspace: PA probe không căn 4 KiB");
            debug_halt(3)
        }
    };
    if let Err(error) = vs.map_obj(VSPACE_VA, frame, Perms::USER_RW) {
        let _ = writeln!(console, "PANIC: vspace: map_obj lỗi {error:?}");
        debug_halt(3)
    }
    let walk_pa = match vs.walk(VSPACE_VA) {
        Ok(t) => t.pa,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: walk lỗi {error:?}");
            debug_halt(3)
        }
    };
    let map_ok = walk_pa == probe_pa;
    let _ = writeln!(
        console,
        "# vspace: map va=0x{VSPACE_VA:x} pa=0x{probe_pa:x} walk=0x{walk_pa:x} match={} pages_high={pages_high} pages_physmap={pages_physmap} pages_image={pages_image} pages_boot_ram={pages_boot_ram} pages_mmio={}",
        map_ok as u8,
        pages_uart + pages_gic + pages_gicr,
    );

    // `activate`: TTBR0 ← bảng VSpace, TTBR1 ← bảng nửa cao của VSpace (mã hoá bằng `Ttbr`).
    // Marker **trước/sau** (R5): nếu treo/fault ở bước đổi gốc, log cho biết đúng bước.
    let _ = writeln!(console, "# vspace: activate begin");
    if let Err(error) = vs.activate() {
        let _ = writeln!(console, "PANIC: vspace: activate lỗi {error:?}");
        debug_halt(3)
    }
    let _ = writeln!(console, "# vspace: activate ok");
    let _ = writeln!(
        console,
        "# vspace: activate ttbr0=0x{:x} ttbr1=0x{:x} asid={}",
        vs.ttbr0().unwrap_or(0),
        vs.ttbr1().unwrap_or(0),
        vs.id(),
    );
    let read = asm::read_u64(VSPACE_VA);
    let active_ok = read == VSPACE_MAGIC;
    let _ = writeln!(
        console,
        "# vspace: active read=0x{read:016x} magic=0x{VSPACE_MAGIC:016x} match={}",
        active_ok as u8,
    );
    if !tap(
        &mut console,
        8,
        "vspace: map Frame 4 KiB + walk + activate + read",
        map_ok && active_ok,
    ) {
        debug_halt(5)
    }

    // `unmap` + **`TLBI` + shootdown** (KT-262, B3.6): `unmap_shootdown` chạy chuỗi cục bộ
    // `dsb ishst` → `tlbi vae1is` → `dsb ish` → `isb` rồi (nếu có hart phụ) SGI + chờ ack. Ở đây
    // **chưa** hart phụ nào online nên `Targets::NONE` ⇒ chỉ chuỗi cục bộ; đường đa hart được đo
    // ở TAP 13/14. Không xoá entry TLB thì lần đọc sau vẫn trúng entry cũ và **không** fault —
    // phép đo sẽ nói dối.
    let (unmap_pa, unmap_sd) = match vs.unmap_shootdown(VSPACE_VA, Targets::NONE) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(console, "PANIC: vspace: unmap lỗi {error:?}");
            debug_halt(3)
        }
    };
    let walk_after = vs.walk(VSPACE_VA);
    let unmap_ok = unmap_pa == probe_pa && walk_after == Err(MapError::NotMapped) && unmap_sd.ok();
    let _ = writeln!(
        console,
        "# vspace: unmap pa=0x{unmap_pa:x} walk={walk_after:?} match={} tlbi_acks={} tlbi_expected={} tlbi_ok={}",
        unmap_ok as u8,
        unmap_sd.acks,
        unmap_sd.expected,
        unmap_sd.ok() as u8,
    );

    // Đọc qua VA sau `unmap` ⇒ **fault**; handler phân loại `FaultKind::Vm` từ `ESR_EL1`/
    // `FAR_EL1` rồi resume (`ELR += 4`) để `high_entry` in bằng chứng và phát TAP 9.
    faults::reset(0);
    faults::expect(0);
    let _ = asm::read_u64(VSPACE_VA);
    let fault = faults::get(0);
    let vm = fault.is_some();
    let fault_va = fault.map(|f| f.va).unwrap_or(0);
    let access = FaultAccess::from_code(fault.map(|f| f.access).unwrap_or(u64::MAX));
    let reason = VmFaultReason::from_code(fault.map(|f| f.reason).unwrap_or(u64::MAX));
    let fault_ok = vm
        && fault_va == VSPACE_VA
        && access == FaultAccess::Read
        && reason == VmFaultReason::NotMapped
        && unmap_ok;
    let _ = writeln!(
        console,
        "# fault: kind={} hart=0 va=0x{fault_va:x} access={} reason={} esr=0x{:016x} far=0x{:x} elr=0x{:x} match={}",
        if vm { "Vm" } else { "?" },
        access.name(),
        reason.name(),
        fault.map(|f| f.esr).unwrap_or(0),
        fault.map(|f| f.far).unwrap_or(0),
        fault.map(|f| f.elr).unwrap_or(0),
        fault_ok as u8,
    );
    if !tap(
        &mut console,
        9,
        "vspace: read after unmap faults with FaultKind::Vm",
        fault_ok,
    ) {
        debug_halt(5)
    }

    // ---- KT-259 (B3.9) + KT-262 (B3.6): bốn hart. Chạy **sau** `activate` để mọi hart dùng
    // **cùng** bảng trang `VSpace` (hart phụ nạp đúng `TTBR0/TTBR1` mà hart 0 đã công bố) — nếu
    // chạy trước, hart phụ sẽ ở lại bảng boot trong khi hart 0 đổi sang bảng `VSpace`, tức hai
    // address space khác nhau.
    let sp_high = asm::read_current_sp();
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    smpboot::publish_hart0_sp_pa(text_start + (sp_high - KERNEL_HIGH_BASE));
    if !smpboot::smp_bringup(&mut console) {
        debug_halt(5)
    }

    // ---- KT-262 (B3.6): `unmap` trên hart 0 ⇒ **hart 1** thấy VA mất hiệu lực (TAP 13) và mọi
    // hart phụ đã ack shootdown (TAP 14).
    let probe_pa = VSPACE_PROBE_PA.load(Ordering::Relaxed);
    if !smpboot::tlbi_shootdown_test(&mut console, &mut vs, probe_pa) {
        debug_halt(5)
    }

    // ---- KT-262: **bốn** hart online, đúng `MPIDR` từng hart, cả bốn đã báo alive (TAP 15).
    if !smpboot::smp_four_harts(&mut console) {
        debug_halt(5)
    }

    // ---- **KT-269** (B3.7, D2-206): U-mode. Map trang mã EL0 + stack user rồi `eret` xuống
    // EL0; **không trả về** — đường `Exit` của EL0 `eret` về EL1h tại `umode::el0_done`, nơi in
    // marker **duy nhất** của ảnh rồi tắt máy. Marker chỉ in khi TAP 1..18 đã xanh.
    // **KT-338b** (D2-233): dựng `vifast-core::Machine` + rig IPC **trước** lần `eret` đầu tiên —
    // syscall của cả hai lượt U-mode đi qua `syscall::dispatch`, nên model + một TCB `Running`
    // phải có trước. Trả `false` ⇒ PANIC (fail-loud, không chạy tiếp với model nửa vời).
    if !crate::coretest::prepare(&mut vs, text_start, &mut console) {
        let _ = writeln!(console, "PANIC: coretest: không dựng được Machine/rig");
        debug_halt(3)
    }
    umode::run(&mut console, &mut vs, text_start)
}
