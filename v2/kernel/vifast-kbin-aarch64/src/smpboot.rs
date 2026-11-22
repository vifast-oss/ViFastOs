// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-259 + KT-262** — trạng thái SMP giữa các hart + `smp_bringup` + `kmain_secondary`.
//!
//! Tách khỏi `main.rs` vì luật **R1** (`v2/bin/v2 rules`): tệp trong `kernel/` ≤ **1.500 dòng**.
//! Tên mô-đun là `smpboot` (không phải `smp`) để **không** đụng `vifast_plat_aarch64::smp` —
//! crate arch giữ phần **dữ liệu** (PSCI/MPIDR/arena/shootdown), tệp này giữ phần **điều phối**.
//!
//! # KT-262 — bốn hart + shootdown `TLBI`
//!
//! KT-259 làm **2 hart**; KT-262 mở rộng lên **4** ([`SMP_HARTS`]) và dùng đúng ba SGI khác nhau:
//!
//! | SGI | Tên | Ai gửi → ai nhận | Việc |
//! |---|---|---|---|
//! | `0` | [`gic::SGI_IPI`] | hart 0 → hart 1 | IPI của KT-259 (đếm + in `# smp: ipi recv=`) |
//! | `1` | [`gic::SGI_SHOOTDOWN`] | hart 0 → **mọi** hart phụ | `tlbi vae1is` + `dsb ish` + `isb` + **ack** |
//! | `2` | [`gic::SGI_COMMAND`] | hart 0 → **một** hart phụ | đánh thức hart phụ thực thi lệnh trong [`SMP_CMD`] |
//!
//! Ba số **khác nhau** là điều kiện sống còn: `note_ipi` của KT-259 đếm **mọi** SGI 0 và in
//! `# smp: ipi recv=<n>`; dùng chung số thì dòng ấy xuất hiện nhiều lần và runner ĐỎ (ca âm (g)).
//!
//! # Vì sao mọi giao tiếp đi qua `AtomicU64` + `Release`/`Acquire`
//!
//! Các hart chạy **song song thật** trên QEMU (`-smp 4`), không có lock, không có `unsafe`. Cache
//! đang **tắt** (`SCTLR_EL1.C = 0`) nên mọi truy cập đi thẳng bộ nhớ; thứ tự do `Release`/
//! `Acquire` + `dsb sy`/`sev` của [`asm`] bảo đảm.
//!
//! # Quy tắc in (đã trả giá bằng một lỗi thật)
//!
//! Hart nào **in xong** mới được đặt cờ mà hart kia đang chờ — **in trước, đếm sau**. Lần chạy
//! đầu của KT-259 đặt cờ trước khi in và hai hart in **xen ký tự** vào cùng PL011:
//! `# smp: # smp: ipiipi  send=recv=1 intiOkd=0`. Runner so **nguyên văn** từng dòng `# smp:` nên
//! lỗi ấy làm boot đỏ — đúng như phải. Với 4 hart, quy tắc càng bắt buộc: hart 0 **tuần tự**
//! từng hart phụ (gửi lệnh → chờ `SMP_DONE` → mới sang hart kế), nên không có hai hart phụ nào
//! in cùng lúc.
//!
//! # Vì sao hart phụ chờ bằng `wfe` (không `wfi`, không spin)
//!
//! `wfi` mất đánh thức nếu SGI tới **giữa** lúc đọc cờ và lúc vào `wfi` (ngắt đã được handler
//! xử lý xong ⇒ không còn ngắt chờ ⇒ `wfi` ngủ tiếp). `sev` đặt **event register** kể cả khi
//! hart phụ chưa vào `wfe`, nên `wfe` không mất đánh thức. Spin thì đúng nhưng đốt CPU của QEMU
//! TCG (4 hart × vòng quay) và làm mọi phép đo thời gian vô nghĩa.

use core::fmt::Write;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_aarch64::boot_mmu::{apply_step, mmu_is_on, MMU_ENABLE_SEQUENCE};
use vifast_plat_aarch64::format::Perms;
use vifast_plat_aarch64::gic;
use vifast_plat_aarch64::mmu::TableMemory;
use vifast_plat_aarch64::resources::GIC;
use vifast_plat_aarch64::shootdown::Targets;
use vifast_plat_aarch64::smp::{
    self, hart_index, HartStacks, MmuBootRegs, HART_AREA_BYTES, MAX_HARTS,
};
use vifast_plat_aarch64::split_mmu::KERNEL_HIGH_BASE;
use vifast_plat_aarch64::vector;
use vifast_plat_aarch64::vspace::{Frame, VSpace};
use vifast_plat_aarch64::{asm, debug_halt};
use vifast_plat_common::fault::{FaultAccess, VmFaultReason};

use vifast_plat_aarch64::ipi;
use vifast_plat_common::ipi::IpiKind;

use crate::bootmmu::{TEXT_START_PA, VSPACE_MAGIC, VSPACE_VA};
use crate::{faults, tap, vector_base, Console};

// Arena stack per-hart do `linker.ld` cấp (`__smp_stacks_start`/`__smp_stacks_end`).
extern "C" {
    static __smp_stacks_start: u8;
    static __smp_stacks_end: u8;
}

/// Số hart ảnh đánh thức — **4** (hart 0 + 3 hart phụ), tiêu chí KT-262.
///
/// `MAX_HARTS` vẫn là **8** (D2-195 Q5): arena stack đã đủ 8 slot, `CPU_ON` đã tổng quát theo
/// hart id; `SMP_HARTS` chỉ là **số hart ảnh này thật sự đánh thức** (runner `-smp 4`).
pub const SMP_HARTS: usize = 4;

/// Mặt nạ hart **kỳ vọng** online (`0b1111` với `SMP_HARTS = 4`).
pub const SMP_EXPECT_MASK: u32 = (1u32 << SMP_HARTS) - 1;

// ---------------------------------------------------------------------------------------------
// Lệnh cho hart phụ (SGI 2 đánh thức, `SMP_CMD` nói phải làm gì)
// ---------------------------------------------------------------------------------------------

/// Không có lệnh — hart phụ `wfe`.
const CMD_NONE: u64 = 0;
/// In `# smp: hart=<id> alive` rồi bật bit trong [`SMP_ALIVE_MASK`].
const CMD_REPORT: u64 = 1;
/// Đọc [`VSPACE_VA`] **một lần** (nạp entry vào TLB của hart này) và ghi giá trị lại.
const CMD_TLBI_READ1: u64 = 2;
/// Đọc [`VSPACE_VA`] sau khi hart 0 `unmap` + shootdown — **phải fault** `FaultKind::Vm`.
const CMD_TLBI_READ2: u64 = 3;

// ---------------------------------------------------------------------------------------------
// Trạng thái SMP
// ---------------------------------------------------------------------------------------------

/// PA của `_start_secondary` — **phải** đọc trước khi nhảy nửa cao (sau đó `adrp` cho alias).
static SMP_ENTRY_PA: AtomicU64 = AtomicU64::new(0);
/// `TCR_EL1` hart 0 đã dùng — hart phụ nạp lại đúng giá trị này.
static SMP_BOOT_TCR: AtomicU64 = AtomicU64::new(0);
/// `MAIR_EL1` hart 0 đã dùng.
static SMP_BOOT_MAIR: AtomicU64 = AtomicU64::new(0);
/// `TTBR0_EL1` hart 0 đã dùng (bảng nửa thấp của `VSpace`).
static SMP_BOOT_TTBR0: AtomicU64 = AtomicU64::new(0);
/// `TTBR1_EL1` hart 0 đã dùng (bảng nửa cao của `VSpace`).
static SMP_BOOT_TTBR1: AtomicU64 = AtomicU64::new(0);
/// `SCTLR_EL1` hart 0 **sau** khi bật MMU (`M = 1`).
static SMP_BOOT_SCTLR: AtomicU64 = AtomicU64::new(0);

/// Mặt nạ hart đã online (bit `i` = hart `i` đã in `# smp: hart=i online` + GIC sẵn sàng).
static SMP_ONLINE_MASK: AtomicU32 = AtomicU32::new(0);
/// Mặt nạ hart đã in `# smp: hart=i alive` (bit `i` = hart `i`).
static SMP_ALIVE_MASK: AtomicU32 = AtomicU32::new(0);
/// Số IPI (`SGI 0`) hart 1 đã nhận — tăng trong **handler IRQ**, không phải vòng chờ.
static SMP_IPI_COUNT: AtomicU64 = AtomicU64::new(0);
/// `MPIDR_EL1` **từng hart tự đọc** — bằng chứng hart id lấy từ phần cứng, không từ `x3`.
static SMP_HART_MPIDR: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `context_id` PSCI đã chuyển cho từng hart phụ (`x0` lúc vào `_start_secondary`).
static SMP_HART_CTX: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
/// `sp` từng hart phụ tự đọc sau khi bật MMU (VA thấp = PA).
static SMP_HART_SP: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `GICR_TYPER` redistributor của từng hart phụ (affinity phải khớp hart id).
static SMP_HART_TYPER: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `ICC_SRE_EL1` từng hart phụ đọc lại sau khi bật `SRE` (bằng chứng `SRE = 1`).
static SMP_HART_SRE: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `GICR_WAKER` từng hart phụ đọc lại sau khi wake (bằng chứng `ProcessorSleep = 0`).
static SMP_HART_WAKER: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `sp` của hart 0 **quy về PA** (hart 0 chạy VA nửa cao) — để so slot với hart phụ.
static SMP_HART0_SP_PA: AtomicU64 = AtomicU64::new(0);
/// `GICD_CTLR` sau khởi tạo distributor.
static SMP_GICD_CTLR: AtomicU64 = AtomicU64::new(0);
/// `GICD_TYPER` (số dòng ngắt + `ARE`).
static SMP_GICD_TYPER: AtomicU64 = AtomicU64::new(0);
/// `GICR_TYPER` redistributor của hart 0.
static SMP_HART0_TYPER: AtomicU64 = AtomicU64::new(0);
/// `GICR_WAKER` hart 0 đọc lại sau khi wake.
static SMP_HART0_WAKER: AtomicU64 = AtomicU64::new(0);
/// `ICC_SRE_EL1` hart 0 đọc lại.
static SMP_HART0_SRE: AtomicU64 = AtomicU64::new(0);
/// Số vòng hart 0 đã chờ từng hart phụ online (bằng chứng "chờ bao lâu", không phải hằng số).
static SMP_WAIT_ONLINE: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
/// Số vòng hart 0 đã chờ IPI.
static SMP_WAIT_IPI: AtomicU64 = AtomicU64::new(0);

/// Lệnh đang chờ **từng** hart phụ thực thi ([`CMD_NONE`] = không có).
static SMP_CMD: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(CMD_NONE) }; MAX_HARTS];
/// Mặt nạ hart phụ đã **xong** lệnh hiện tại (bit `i` = hart `i`).
static SMP_DONE: AtomicU32 = AtomicU32::new(0);
/// Số vòng hart 0 đã chờ từng hart phụ xong lệnh.
static SMP_WAIT_CMD: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
/// Giá trị từng hart phụ đọc được ở [`VSPACE_VA`] (bằng chứng "TLB của hart ấy đã nạp entry").
static SMP_TLBI_READ: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];

/// Trần vòng chờ một cờ SMP — **có trần** để hart 0 không treo im lặng khi hart phụ không lên.
///
/// Đo được trên QEMU TCG: hart 1 lên sau ~1,5×10^5 vòng (log `kt259-aarch64-smp.md`), nên trần
/// này rộng hơn hai bậc độ lớn; vượt trần ⇒ in `not ok` + dừng, không treo.
const SMP_WAIT_LIMIT: u64 = 50_000_000;

/// Chờ `flag` đạt `want`; trả số vòng đã chờ, `None` nếu quá [`SMP_WAIT_LIMIT`].
fn wait_flag(flag: &AtomicU64, want: u64) -> Option<u64> {
    let mut spins = 0u64;
    while spins < SMP_WAIT_LIMIT {
        if flag.load(Ordering::Acquire) == want {
            return Some(spins);
        }
        spins += 1;
        core::hint::spin_loop();
    }
    None
}

/// Chờ hart `hart` online; trả số vòng đã chờ, `None` nếu quá [`SMP_WAIT_LIMIT`].
fn wait_online(hart: usize) -> Option<u64> {
    let mut spins = 0u64;
    while spins < SMP_WAIT_LIMIT {
        if SMP_ONLINE_MASK.load(Ordering::Acquire) & (1u32 << hart) != 0 {
            return Some(spins);
        }
        spins += 1;
        core::hint::spin_loop();
    }
    None
}

/// Gửi lệnh `cmd` cho hart phụ `hart`: xoá cờ done, ghi lệnh, `dsb sy` + `sev`, rồi SGI 2.
///
/// Trả `true` ⇔ SGI đã gửi được. `sev` **trước** SGI là cố ý: `sev` đặt event register nên hart
/// phụ đang `wfe` thức dậy **kể cả khi** SGI tới sau và bị xử lý xong trước khi nó vào `wfe`.
fn send_cmd(hart: usize, cmd: u64) -> bool {
    if hart == 0 || hart >= SMP_HARTS {
        return false;
    }
    SMP_DONE.fetch_and(!(1u32 << hart), Ordering::AcqRel);
    SMP_CMD[hart].store(cmd, Ordering::Release);
    asm::dsb_sy();
    asm::sev();
    // **KT-338c** (D2-244 tầng 1): lệnh đi qua **hộp thư IPI bitmask dùng chung** (kind `VmKick`) —
    // `post` đặt bit rồi gửi `SGI_COMMAND` **chỉ khi** bit chưa chờ (coalesce). Đã coalesce vẫn
    // là "hart đích sẽ thấy lệnh": `sev` ở trên đã đánh thức `wfe`, và vòng lệnh của hart phụ đọc
    // `SMP_CMD` mỗi vòng.
    ipi::post(hart, IpiKind::VmKick).queued()
}

/// Chờ hart phụ `hart` xong lệnh hiện tại; trả số vòng đã chờ, `None` nếu quá trần.
fn wait_done(hart: usize) -> Option<u64> {
    let mut spins = 0u64;
    while spins < SMP_WAIT_LIMIT {
        if SMP_DONE.load(Ordering::Acquire) & (1u32 << hart) != 0 {
            SMP_WAIT_CMD[hart].store(spins, Ordering::Release);
            return Some(spins);
        }
        spins += 1;
        core::hint::spin_loop();
    }
    SMP_WAIT_CMD[hart].store(u64::MAX, Ordering::Release);
    None
}

/// Công bố **PA** của `_start_secondary` (đọc trước khi nhảy nửa cao — xem `mmu_bringup`).
pub(crate) fn publish_entry_pa(pa: u64) {
    SMP_ENTRY_PA.store(pa, Ordering::Release);
}

/// Công bố **PA** của `sp` hart 0 (hart 0 chạy VA nửa cao; phép so slot cần PA).
pub(crate) fn publish_hart0_sp_pa(pa: u64) {
    SMP_HART0_SP_PA.store(pa, Ordering::Release);
}

/// Ghi nhận **một** IPI (`SGI 0`) vừa tới — gọi từ handler IRQ của `main.rs`.
///
/// **In trước, đếm sau**: hart 0 chờ đúng bộ đếm này rồi mới in dòng của nó, nên nếu đếm trước
/// khi in thì hai hart in xen ký tự vào cùng PL011 (xem tài liệu đầu tệp).
///
/// Chỉ đếm `SGI 0`: `SGI 1` (shootdown) và `SGI 2` (lệnh) do handler khác xử lý, **không** in —
/// nếu không, dòng `# smp: ipi recv=1` xuất hiện nhiều lần và runner ĐỎ.
pub(crate) fn note_ipi(intid: u64) {
    if intid != gic::SGI_IPI as u64 {
        return;
    }
    let n = SMP_IPI_COUNT.load(Ordering::Acquire) + 1;
    let mut console = Console;
    let _ = writeln!(console, "# smp: ipi recv={n} intid={intid}");
    SMP_IPI_COUNT.store(n, Ordering::Release);
}

/// **KT-259 + KT-262** — đánh thức `SMP_HARTS - 1` hart phụ, đo IPI, phát TAP 10..12.
///
/// Thứ tự **bắt buộc** (mỗi bước có dòng `#` làm bằng chứng):
///
/// 1. kiểm bố cục arena stack (linker ↔ crate) — lệch là PANIC, không đoán;
/// 2. GICv3: distributor (`ARE_NS | EnableGrp1NS`) + redistributor **hart 0** + CPU interface;
/// 3. công bố `TCR/MAIR/TTBR0/TTBR1/SCTLR` + PA điểm vào, `dsb sy` + `sev`;
/// 4. `PSCI CPU_ON(hart, _start_secondary, hart)` cho **từng** hart phụ, tuần tự — **không in gì**
///    giữa lệnh và lúc thấy hart ấy online (hart phụ in ngay khi lên; in xen sẽ hỏng dòng log);
/// 5. TAP 10 (hart 1 online + stack hai hart **khác slot**), TAP 11 (IPI `SGI 0`),
///    TAP 12 (**mọi** hart online đều báo alive).
pub(crate) fn smp_bringup(console: &mut Console) -> bool {
    // ---- 1. Bố cục arena: `linker.ld` và `smp` phải khớp **tuyệt đối**.
    //
    // `high_entry` chạy ở VA **nửa cao**, nên `addr_of!` cho alias nửa cao của arena; mọi phép so
    // slot phải dùng **PA** (hart phụ chạy VA thấp, `sp` của nó là PA). Quy đổi bằng
    // `text_start + (alias - KERNEL_HIGH_BASE)` — cùng công thức nghịch của `high_alias_va`.
    let arena_high = core::ptr::addr_of!(__smp_stacks_start) as u64;
    let arena_high_end = core::ptr::addr_of!(__smp_stacks_end) as u64;
    let text_start = TEXT_START_PA.load(Ordering::Relaxed);
    let arena_start = text_start + (arena_high - KERNEL_HIGH_BASE);
    let arena_bytes = arena_high_end.wrapping_sub(arena_high);
    let stacks = match HartStacks::new(arena_start) {
        Some(stacks) => stacks,
        None => {
            let _ = writeln!(
                console,
                "PANIC: smp: arena 0x{arena_start:x} không căn 4 KiB"
            );
            debug_halt(4)
        }
    };
    let layout = stacks.check();
    let layout_ok = layout.is_ok() && arena_bytes == HART_AREA_BYTES;
    let _ = writeln!(
        console,
        "# smp: arena pa=0x{arena_start:x} alias=0x{arena_high:x} bytes={arena_bytes} expect={HART_AREA_BYTES} layout={layout:?} ok={}",
        layout_ok as u8,
    );
    if !layout_ok {
        let _ = writeln!(console, "PANIC: smp: bố cục arena stack không khớp linker");
        debug_halt(4)
    }

    // ---- 2. GICv3: distributor + redistributor hart 0 + CPU interface hart 0.
    let info = match gic::init_distributor(GIC.base) {
        Ok(info) => info,
        Err(error) => {
            let _ = writeln!(console, "PANIC: gic: distributor lỗi {error:?}");
            debug_halt(4)
        }
    };
    let typer0 = match gic::wake_redistributor(0) {
        Ok(typer) => typer,
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: gic: wake redistributor hart 0 lỗi {error:?}"
            );
            debug_halt(4)
        }
    };
    for sgi in [gic::SGI_IPI, gic::SGI_SHOOTDOWN, gic::SGI_COMMAND] {
        if let Err(error) = gic::enable_sgi(0, sgi) {
            let _ = writeln!(console, "PANIC: gic: enable SGI {sgi} hart 0 lỗi {error:?}");
            debug_halt(4)
        }
    }
    let sre0 = match gic::init_cpu_interface() {
        Ok(sre) => sre,
        Err(error) => {
            let _ = writeln!(console, "PANIC: gic: cpu interface hart 0 lỗi {error:?}");
            debug_halt(4)
        }
    };
    let waker0 = gic::redistributor_waker(0).unwrap_or(u32::MAX);
    SMP_GICD_CTLR.store(info.ctlr as u64, Ordering::Release);
    SMP_GICD_TYPER.store(info.typer as u64, Ordering::Release);
    SMP_HART0_TYPER.store(typer0, Ordering::Release);
    SMP_HART0_WAKER.store(waker0 as u64, Ordering::Release);
    SMP_HART0_SRE.store(sre0, Ordering::Release);
    let _ = writeln!(
        console,
        "# gic: dist ctlr=0x{:08x} typer=0x{:08x} lines={} are_ns={} grp1ns={}",
        info.ctlr,
        info.typer,
        info.lines,
        ((info.ctlr & gic::GICD_CTLR_ARE_NS) != 0) as u8,
        ((info.ctlr & gic::GICD_CTLR_ENABLE_GRP1NS) != 0) as u8,
    );
    let _ = writeln!(
        console,
        "# gic: rd hart=0 typer=0x{typer0:016x} aff=0x{:x} waker=0x{waker0:08x} asleep={} sre=0x{sre0:x} sgi={}",
        typer0 >> 32,
        ((waker0 & gic::GICR_WAKER_CHILDREN_ASLEEP) != 0) as u8,
        gic::SGI_IPI,
    );

    // ---- 3. Công bố thanh ghi MMU + PA điểm vào của hart phụ, rồi rào chắn bộ nhớ.
    let regs = MmuBootRegs::new(
        asm::read_tcr_el1(),
        asm::read_mair_el1(),
        asm::read_ttbr0_el1(),
        asm::read_ttbr1_el1(),
        asm::read_sctlr_el1(),
    );
    if !regs.is_complete() {
        let _ = writeln!(
            console,
            "PANIC: smp: thanh ghi MMU công bố chưa đủ {regs:?}"
        );
        debug_halt(4)
    }
    SMP_BOOT_TCR.store(regs.tcr, Ordering::Release);
    SMP_BOOT_MAIR.store(regs.mair, Ordering::Release);
    SMP_BOOT_TTBR0.store(regs.ttbr0, Ordering::Release);
    SMP_BOOT_TTBR1.store(regs.ttbr1, Ordering::Release);
    SMP_BOOT_SCTLR.store(regs.sctlr, Ordering::Release);
    // Hart 0 cũng ghi `MPIDR` của mình vào mảng — TAP 15 so **cả bốn** hart với topology kỳ vọng.
    // Và hart 0 **luôn** online (nó đang chạy đây): bật bit 0 để `SMP_ONLINE_MASK` là mặt nạ
    // **cả bốn** hart, không phải "ba hart phụ" — TAP 12/15 so `alive == online` trên cùng mặt nạ.
    SMP_HART_MPIDR[0].store(asm::read_mpidr_el1(), Ordering::Release);
    SMP_ONLINE_MASK.fetch_or(1, Ordering::Release);
    let entry = SMP_ENTRY_PA.load(Ordering::Acquire);
    if entry == 0 {
        let _ = writeln!(console, "PANIC: smp: chưa có PA của _start_secondary");
        debug_halt(4)
    }
    asm::dsb_sy();
    asm::sev();
    let _ = writeln!(
        console,
        "# smp: boot regs tcr=0x{:016x} mair=0x{:016x} ttbr0=0x{:x} ttbr1=0x{:x} sctlr=0x{:016x}",
        regs.tcr, regs.mair, regs.ttbr0, regs.ttbr1, regs.sctlr,
    );

    // ---- 4. `CPU_ON` từng hart phụ, **tuần tự**: hart 0 không in gì giữa lệnh và lúc hart ấy
    // online (hart phụ in ngay khi lên; in xen sẽ hỏng dòng log — bài học KT-259).
    let mut rc = [0i64; MAX_HARTS];
    let mut waited = [None; MAX_HARTS];
    for hart in 1..SMP_HARTS {
        let _ = writeln!(
            console,
            "# smp: cpu_on begin hart={hart} entry=0x{entry:x} ctx={hart} fn=0x{:x}",
            smp::PSCI_CPU_ON,
        );
        rc[hart] = asm::psci_cpu_on(hart as u64, entry, hart as u64);
        waited[hart] = wait_online(hart);
        SMP_WAIT_ONLINE[hart].store(waited[hart].unwrap_or(u64::MAX), Ordering::Release);
        let mpidr = SMP_HART_MPIDR[hart].load(Ordering::Acquire);
        let ctx = SMP_HART_CTX[hart].load(Ordering::Acquire);
        let sp = SMP_HART_SP[hart].load(Ordering::Acquire);
        let slot = stacks.slot_of(sp);
        let _ = writeln!(
            console,
            "# smp: cpu_on hart={hart} rc={} ({}) online={} waited={} mpidr=0x{mpidr:016x} ctx={ctx} sp=0x{sp:x} slot={slot:?}",
            rc[hart],
            smp::cpu_on_error_name(rc[hart]),
            waited[hart].is_some() as u8,
            waited[hart].unwrap_or(u64::MAX),
        );
    }

    // ---- 5. TAP 10: hart 1 online + **hai stack khác slot** (đột biến iii của KT-259).
    let h1_mpidr = SMP_HART_MPIDR[1].load(Ordering::Acquire);
    let h1_ctx = SMP_HART_CTX[1].load(Ordering::Acquire);
    let h1_sp = SMP_HART_SP[1].load(Ordering::Acquire);
    let h1_slot = stacks.slot_of(h1_sp);
    let h0_sp_pa = SMP_HART0_SP_PA.load(Ordering::Acquire);
    let h0_slot = stacks.slot_of(h0_sp_pa);
    let online_mask = SMP_ONLINE_MASK.load(Ordering::Acquire);
    let hart1_ok = online_mask & 0b10 != 0
        && smp::cpu_on_ok(rc[1])
        && hart_index(h1_mpidr) == Some(1)
        && h1_ctx == 1
        && h1_slot == Some(1)
        && h0_slot == Some(0)
        && h0_slot != h1_slot;
    let _ = writeln!(
        console,
        "# smp: hart1 mpidr=0x{h1_mpidr:016x} ctx={h1_ctx} sp=0x{h1_sp:x} slot1={h1_slot:?} hart0_sp_pa=0x{h0_sp_pa:x} slot0={h0_slot:?} online=0x{online_mask:x}",
    );
    if !tap(console, 10, "smp: hart 1 online", hart1_ok) {
        return false;
    }

    // ---- 6. TAP 11: IPI `SGI 0` hart 0 → hart 1 (hart 1 đếm trong **handler IRQ**).
    //
    // **KT-338c** (D2-244 tầng 1): chuông đi qua **hộp thư IPI bitmask dùng chung** — kind `Wake` vào hộp
    // thư hart 1 **trước**, rồi `SGI 0`. Điều kiện TAP vì vậy **mạnh hơn** KT-259: không chỉ
    // "hart 1 đếm được 1 IPI", mà **bitmask đã được drain** (`drained >= 1`) và **không** có SGI
    // nào rơi vào bitmask rỗng (`empty == 0`) — nếu hộp thư hỏng, TAP 11 đỏ dù bộ đếm IPI vẫn 1.
    let posted = ipi::post(1, IpiKind::Wake);
    let ipi_waited = wait_flag(&SMP_IPI_COUNT, 1);
    SMP_WAIT_IPI.store(ipi_waited.unwrap_or(u64::MAX), Ordering::Release);
    let ipi = SMP_IPI_COUNT.load(Ordering::Acquire);
    let ipibox_stats = ipi::stats();
    let ipibox_ok = ipibox_stats.drained >= 1 && ipibox_stats.empty == 0;
    let ipi_ok = posted.queued() && ipi_waited.is_some() && ipi == 1 && ipibox_ok;
    let _ = writeln!(
        console,
        "# smp: ipi post={} count={ipi} waited={} sgi={} target=1",
        posted.name(),
        ipi_waited.unwrap_or(u64::MAX),
        gic::SGI_IPI,
    );
    // Bằng chứng **máy kiểm** cho tầng hộp thư IPI (D2-244): runner đòi đúng **một** dòng này.
    let _ = writeln!(
        console,
        "# ipi: mailbox posted={} coalesced={} sent={} drained={} empty={} sgi={} target=1",
        ipibox_stats.posted,
        ipibox_stats.coalesced,
        ipibox_stats.sent,
        ipibox_stats.drained,
        ipibox_stats.empty,
        gic::SGI_IPI,
    );
    if !tap(console, 11, "smp: ipi 0->1", ipi_ok) {
        return false;
    }

    // ---- 7. TAP 12: **mọi** hart online đều báo alive. Gửi lệnh **tuần tự** từng hart phụ rồi
    // chờ nó xong ⇒ không có hai hart phụ in cùng lúc (quy tắc in ở đầu tệp).
    let mut report_ok = true;
    for hart in 1..SMP_HARTS {
        let cmd_sent = send_cmd(hart, CMD_REPORT);
        let done = wait_done(hart);
        let alive = SMP_ALIVE_MASK.load(Ordering::Acquire) & (1u32 << hart) != 0;
        let _ = writeln!(
            console,
            "# smp: report hart={hart} sent={cmd_sent} done={} waited={} alive={}",
            done.is_some() as u8,
            done.unwrap_or(u64::MAX),
            alive as u8,
        );
        report_ok &= cmd_sent && done.is_some() && alive;
    }
    let _ = writeln!(console, "# smp: hart=0 alive");
    SMP_ALIVE_MASK.fetch_or(1, Ordering::Release);
    let alive_mask = SMP_ALIVE_MASK.load(Ordering::Acquire);
    let online_mask = SMP_ONLINE_MASK.load(Ordering::Acquire);
    let all_ok = report_ok && alive_mask == online_mask && online_mask.count_ones() >= 2;
    let _ = writeln!(
        console,
        "# smp: report hart0=1 alive=0x{alive_mask:x} online=0x{online_mask:x} harts={}",
        online_mask.count_ones(),
    );
    tap(console, 12, "smp: all harts report", all_ok)
}

/// **KT-262** — TAP 13/14: `unmap` trên hart 0 ⇒ **hart 1** thấy VA mất hiệu lực, và **mọi** hart
/// phụ đã ack shootdown.
///
/// Phép đo **không thể giả** (đây là toàn bộ mục đích của B3.6):
///
/// 1. map lại [`VSPACE_VA`] (TAP 9 đã `unmap` nó) rồi `dsb sy`;
/// 2. lệnh cho **hart 1** đọc VA ấy ⇒ entry vào **TLB của hart 1**; giá trị đọc được phải bằng
///    [`VSPACE_MAGIC`] — nếu không, "hart 1 fault" sau đó **không** chứng minh được gì (nó fault
///    vì chưa bao giờ có entry);
/// 3. hart 0 `unmap_shootdown` — xoá lá bảng trang + chuỗi `TLBI` cục bộ + SGI tới **mọi** hart
///    phụ + chờ đủ ack;
/// 4. lệnh cho hart 1 đọc **lại** ⇒ phải fault `FaultKind::Vm` (`va` = [`VSPACE_VA`], `access` =
///    `Read`, `reason` = `NotMapped`).
///
/// TAP 13 xanh ⇔ (2) **và** (4) **và** `unmap` trả đúng PA. TAP 14 xanh ⇔ số ack = số hart phụ
/// **và** không timeout **và** không lỗi gửi SGI.
pub(crate) fn tlbi_shootdown_test<M: TableMemory>(
    console: &mut Console,
    vs: &mut VSpace<M>,
    probe_pa: u64,
) -> bool {
    // ---- 1. Map lại VA của phép đo (TAP 9 đã unmap nó).
    let frame = match Frame::new(probe_pa) {
        Some(frame) => frame,
        None => {
            let _ = writeln!(
                console,
                "PANIC: tlbi: PA probe 0x{probe_pa:x} không căn 4 KiB"
            );
            debug_halt(3)
        }
    };
    if let Err(error) = vs.map_obj(VSPACE_VA, frame, Perms::USER_RW) {
        let _ = writeln!(console, "PANIC: tlbi: map lại VA lỗi {error:?}");
        debug_halt(3)
    }
    asm::dsb_sy();
    let _ = writeln!(
        console,
        "# tlbi: remap va=0x{VSPACE_VA:x} pa=0x{probe_pa:x} magic=0x{VSPACE_MAGIC:016x}",
    );

    // ---- 2. Hart 1 đọc VA ⇒ nạp entry vào TLB **của nó**.
    faults::reset(1);
    let sent1 = send_cmd(1, CMD_TLBI_READ1);
    let done1 = wait_done(1);
    let read1 = SMP_TLBI_READ[1].load(Ordering::Acquire);
    let read1_ok = read1 == VSPACE_MAGIC;
    let _ = writeln!(
        console,
        "# tlbi: hart=1 read1=0x{read1:016x} magic=0x{VSPACE_MAGIC:016x} match={} sent={sent1} done={}",
        read1_ok as u8,
        done1.is_some() as u8,
    );

    // ---- 3. `unmap` + shootdown: chuỗi cục bộ, SGI tới **mọi** hart phụ, chờ đủ ack.
    let online = SMP_ONLINE_MASK.load(Ordering::Acquire) as u16;
    let targets = Targets::all_but(0, online);
    let (unmap_pa, sd) = match vs.unmap_shootdown(VSPACE_VA, targets) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(console, "PANIC: tlbi: unmap lỗi {error:?}");
            debug_halt(3)
        }
    };
    let _ = writeln!(
        console,
        "# tlbi: shootdown va=0x{:x} targets=0x{:x} expected={} acks={} waited={} timeout={} send_failed=0x{:x} ok={}",
        sd.va,
        sd.targets,
        sd.expected,
        sd.acks,
        sd.waited,
        sd.timed_out as u8,
        sd.send_failed,
        sd.ok() as u8,
    );

    // ---- 4. Hart 1 đọc **lại** ⇒ phải fault `FaultKind::Vm`.
    let sent2 = send_cmd(1, CMD_TLBI_READ2);
    let done2 = wait_done(1);
    let fault = faults::get(1);
    let fault_ok = match fault {
        Some(f) => {
            f.va == VSPACE_VA
                && f.access == FaultAccess::Read.code()
                && f.reason == VmFaultReason::NotMapped.code()
        }
        None => false,
    };
    let _ = writeln!(
        console,
        "# tlbi: hart=1 read2 fault={} va=0x{:x} access={} reason={} esr=0x{:016x} far=0x{:x} sent={sent2} done={} seen=0x{:x}",
        fault_ok as u8,
        fault.map(|f| f.va).unwrap_or(0),
        FaultAccess::from_code(fault.map(|f| f.access).unwrap_or(u64::MAX)).name(),
        VmFaultReason::from_code(fault.map(|f| f.reason).unwrap_or(u64::MAX)).name(),
        fault.map(|f| f.esr).unwrap_or(0),
        fault.map(|f| f.far).unwrap_or(0),
        done2.is_some() as u8,
        faults::seen_mask(),
    );

    // ---- 5. TAP 13/14.
    let sees = read1_ok && fault_ok && unmap_pa == probe_pa;
    if !tap(console, 13, "tlbi: hart 1 sees unmap", sees) {
        return false;
    }
    let ack_ok = sd.ok() && sd.expected == (SMP_HARTS as u32 - 1) && sd.acks == sd.expected;
    tap(console, 14, "tlbi: ack from all harts", ack_ok)
}

/// **KT-262** — TAP 15: **bốn** hart online, mỗi hart đúng `MPIDR` của chỉ số của nó, và **cả
/// bốn** đã in `# smp: hart=<id> alive`.
///
/// Khác TAP 12 (nhất quán `alive == online`), TAP 15 chốt **đúng con số 4** và **danh tính**
/// từng hart: một ảnh chỉ đánh thức được 2 hart vẫn có thể xanh TAP 12 nhưng **không** xanh TAP 15.
pub(crate) fn smp_four_harts(console: &mut Console) -> bool {
    let online = SMP_ONLINE_MASK.load(Ordering::Acquire);
    let alive = SMP_ALIVE_MASK.load(Ordering::Acquire);
    let mut mpidr_ok = true;
    let mut mpidr_seen = [0u64; MAX_HARTS];
    for hart in 0..SMP_HARTS {
        let mpidr = SMP_HART_MPIDR[hart].load(Ordering::Acquire);
        mpidr_seen[hart] = mpidr;
        if Some(mpidr) != smp::mpidr_for_hart(hart) {
            mpidr_ok = false;
        }
    }
    let ok = online == SMP_EXPECT_MASK && alive == SMP_EXPECT_MASK && mpidr_ok;
    let _ = writeln!(
        console,
        "# smp: online mask=0x{online:x} alive=0x{alive:x} expect=0x{SMP_EXPECT_MASK:x} harts={} mpidr_ok={} mpidr=0x{:x},0x{:x},0x{:x},0x{:x}",
        online.count_ones(),
        mpidr_ok as u8,
        mpidr_seen[0],
        mpidr_seen[1],
        mpidr_seen[2],
        mpidr_seen[3],
    );
    tap(console, 15, "smp: 4 harts online", ok)
}

/// **KT-259 + KT-262** — điểm vào Rust của **hart phụ** (được `_start_secondary` gọi sau khi đã
/// có stack per-hart và `CPACR_EL1` bật).
///
/// `hart` = `Aff0` của `MPIDR_EL1` do **asm** đọc (không phải `x3`); `ctx` = `context_id` PSCI.
///
/// Thứ tự **bắt buộc**:
///
/// 1. kiểm `MPIDR` (topology phẳng), `ctx == hart`, và `sp` nằm trong **slot của chính mình** —
///    đây là chỗ đột biến (iii) "stack chồng nhau" lộ ra (PANIC, không chạy tiếp);
/// 2. nạp `TCR/MAIR/TTBR0/TTBR1/SCTLR` đã công bố theo đúng [`MMU_ENABLE_SEQUENCE`] — **một**
///    định nghĩa chuỗi cho mọi hart;
/// 3. cài `VBAR_EL1` (VA **thấp**: hart phụ chạy VA thấp, bảng vector map identity trong `TTBR0`);
/// 4. GICv3 **của chính nó**: wake redistributor → enable **ba** SGI (0/1/2) → CPU interface;
/// 5. in `# smp: hart=<id> online`, bật bit trong [`SMP_ONLINE_MASK`], rồi **mở** IRQ;
/// 6. vòng lệnh: `wfe` → đọc [`SMP_CMD`] → thực thi → `SMP_DONE`. Không bao giờ trả về.
#[no_mangle]
pub extern "C" fn kmain_secondary(hart: u64, ctx: u64) -> ! {
    let mut console = Console;
    let mpidr = asm::read_mpidr_el1();
    let index = match hart_index(mpidr) {
        Some(index) => index,
        None => {
            let _ = writeln!(
                console,
                "PANIC: smp: MPIDR 0x{mpidr:016x} không phải topology phẳng (Aff1..3 khác 0 hoặc Aff0 >= MAX_HARTS)"
            );
            debug_halt(4)
        }
    };
    if index as u64 != hart {
        let _ = writeln!(
            console,
            "PANIC: smp: hart từ MPIDR={index} nhưng asm truyền {hart} (lệch nguồn hart id)"
        );
        debug_halt(4)
    }
    if ctx != hart {
        let _ = writeln!(
            console,
            "PANIC: smp: context_id={ctx} != hart={hart} — PSCI truyền sai, không tin x3"
        );
        debug_halt(4)
    }
    // Stack **per-hart**: `sp` phải nằm trong slot của chính hart này (không phải slot 0, không
    // phải guard). Đột biến (iii) — stride thiếu guard — làm hai slot đè nhau và PANIC ở đây.
    let arena = core::ptr::addr_of!(__smp_stacks_start) as u64;
    let stacks = match HartStacks::new(arena) {
        Some(stacks) => stacks,
        None => {
            let _ = writeln!(console, "PANIC: smp: arena 0x{arena:x} không căn 4 KiB");
            debug_halt(4)
        }
    };
    let sp = asm::read_current_sp();
    let slot = stacks.slot_of(sp);
    if slot != Some(index) {
        let _ = writeln!(
            console,
            "PANIC: smp: sp=0x{sp:x} không thuộc slot {index} (slot={slot:?}) — stack per-hart sai"
        );
        debug_halt(4)
    }

    // ---- 2. MMU: nạp **đúng** bộ thanh ghi hart 0 đã công bố, theo đúng chuỗi 6 bước.
    let regs = MmuBootRegs::new(
        SMP_BOOT_TCR.load(Ordering::Acquire),
        SMP_BOOT_MAIR.load(Ordering::Acquire),
        SMP_BOOT_TTBR0.load(Ordering::Acquire),
        SMP_BOOT_TTBR1.load(Ordering::Acquire),
        SMP_BOOT_SCTLR.load(Ordering::Acquire),
    );
    if !regs.is_complete() {
        let _ = writeln!(
            console,
            "PANIC: smp: hart {hart} thấy thanh ghi MMU chưa đủ {regs:?}"
        );
        debug_halt(4)
    }
    for step in MMU_ENABLE_SEQUENCE {
        apply_step(
            step, regs.tcr, regs.mair, regs.ttbr0, regs.ttbr1, regs.sctlr,
        );
    }
    let sctlr_now = asm::read_sctlr_el1();
    if !mmu_is_on(sctlr_now) {
        let _ = writeln!(
            console,
            "PANIC: smp: hart {hart} bật MMU không thành công sctlr=0x{sctlr_now:016x}"
        );
        debug_halt(4)
    }

    // ---- 3. Bảng vector: VA **thấp** (hart phụ chạy VA thấp; bảng map identity trong TTBR0).
    let vbar = vector::install(vector_base());

    // ---- 4. GICv3 của **chính hart này**: wake redistributor, bật **ba** SGI, CPU interface.
    let typer = match gic::wake_redistributor(index) {
        Ok(typer) => typer,
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: gic: hart {hart} wake redistributor lỗi {error:?}"
            );
            debug_halt(4)
        }
    };
    for sgi in [gic::SGI_IPI, gic::SGI_SHOOTDOWN, gic::SGI_COMMAND] {
        if let Err(error) = gic::enable_sgi(index, sgi) {
            let _ = writeln!(
                console,
                "PANIC: gic: hart {hart} enable SGI {sgi} lỗi {error:?}"
            );
            debug_halt(4)
        }
    }
    let sre = match gic::init_cpu_interface() {
        Ok(sre) => sre,
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: gic: hart {hart} cpu interface lỗi {error:?}"
            );
            debug_halt(4)
        }
    };
    let waker = gic::redistributor_waker(index).unwrap_or(u32::MAX);

    // ---- 5. Báo online **sau** khi GIC sẵn sàng (hart 0 gửi SGI ngay khi thấy cờ này).
    let _ = writeln!(console, "# smp: hart={hart} online");
    let _ = writeln!(
        console,
        "# smp: hart={hart} mpidr=0x{mpidr:016x} ctx={ctx} sp=0x{sp:x} slot={index} stack=0x{:x}-0x{:x} vbar=0x{vbar:x} sctlr=0x{sctlr_now:016x} gic_typer=0x{typer:016x} aff=0x{:x} waker=0x{waker:08x} asleep={} sre=0x{sre:x}",
        stacks.stack(index).map(|(b, _)| b).unwrap_or(0),
        stacks.stack(index).map(|(_, t)| t).unwrap_or(0),
        typer >> 32,
        ((waker & gic::GICR_WAKER_CHILDREN_ASLEEP) != 0) as u8,
    );
    SMP_HART_MPIDR[index].store(mpidr, Ordering::Release);
    SMP_HART_CTX[index].store(ctx, Ordering::Release);
    SMP_HART_SP[index].store(sp, Ordering::Release);
    SMP_HART_TYPER[index].store(typer, Ordering::Release);
    SMP_HART_SRE[index].store(sre, Ordering::Release);
    SMP_HART_WAKER[index].store(waker as u64, Ordering::Release);
    SMP_ONLINE_MASK.fetch_or(1u32 << index, Ordering::Release);
    asm::dsb_sy();
    asm::sev();
    // Mở IRQ **sau** khi báo online: SGI tới sớm vẫn pending trong GIC, không mất.
    asm::daif_clear_irq();

    // ---- 6. Vòng lệnh. `wfe` không mất đánh thức (`sev` đặt event register trước SGI).
    loop {
        let cmd = SMP_CMD[index].load(Ordering::Acquire);
        if cmd == CMD_NONE {
            asm::wfe();
            continue;
        }
        match cmd {
            CMD_REPORT => {
                // **In trước, đếm sau** — hart 0 chờ đúng bit này rồi mới in dòng của nó.
                let _ = writeln!(console, "# smp: hart={hart} alive");
                SMP_ALIVE_MASK.fetch_or(1u32 << index, Ordering::Release);
            }
            CMD_TLBI_READ1 => {
                // Đọc **một lần** ⇒ entry của VA vào TLB của hart này (điều kiện của TAP 13).
                let value = asm::read_u64(VSPACE_VA);
                SMP_TLBI_READ[index].store(value, Ordering::Release);
            }
            CMD_TLBI_READ2 => {
                // Đăng ký fault được chờ **trước** khi đọc: handler chỉ resume khi hart này đã
                // đăng ký, nên fault thật (không chờ) vẫn là fatal.
                faults::expect(index);
                asm::dsb_sy();
                let value = asm::read_u64(VSPACE_VA);
                // Không fault (đột biến "bỏ TLBI ở hart phụ") ⇒ ghi lại giá trị để log thấy.
                SMP_TLBI_READ[index].store(value, Ordering::Release);
            }
            _ => {}
        }
        SMP_CMD[index].store(CMD_NONE, Ordering::Release);
        SMP_DONE.fetch_or(1u32 << index, Ordering::Release);
        asm::dsb_sy();
        asm::sev();
    }
}
