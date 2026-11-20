// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-259** — trạng thái SMP giữa hai hart + `smp_bringup` + `kmain_secondary`.
//!
//! Tách khỏi `main.rs` vì luật **R1** (`v2/bin/v2 rules`): tệp trong `kernel/` ≤ **1.500 dòng**.
//! Tên mô-đun là `smpboot` (không phải `smp`) để **không** đụng `vifast_plat_aarch64::smp` —
//! crate arch giữ phần **dữ liệu** (PSCI/MPIDR/arena), tệp này giữ phần **điều phối** hai hart.
//!
//! # Vì sao mọi giao tiếp đi qua `AtomicU64` + `Release`/`Acquire`
//!
//! Hai hart chạy **song song thật** trên QEMU (`-smp 2`), không có lock, không có `unsafe`. Cache
//! đang **tắt** (`SCTLR_EL1.C = 0`) nên mọi truy cập đi thẳng bộ nhớ; thứ tự do `Release`/
//! `Acquire` + `dsb sy`/`sev` của [`asm`] bảo đảm.
//!
//! # Quy tắc in (đã trả giá bằng một lỗi thật)
//!
//! Hart nào **in xong** mới được đặt cờ mà hart kia đang chờ — **in trước, đếm sau**. Lần chạy
//! đầu của KT-259 đặt cờ trước khi in và hai hart in **xen ký tự** vào cùng PL011:
//! `# smp: # smp: ipiipi  send=recv=1 intiOkd=0`. Runner so **nguyên văn** từng dòng `# smp:` nên
//! lỗi ấy làm boot đỏ — đúng như phải.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::boot_mmu::{apply_step, mmu_is_on, MMU_ENABLE_SEQUENCE};
use vifast_plat_aarch64::gic;
use vifast_plat_aarch64::resources::GIC;
use vifast_plat_aarch64::smp::{self, hart_index, HartStacks, MmuBootRegs, HART_AREA_BYTES};
use vifast_plat_aarch64::split_mmu::KERNEL_HIGH_BASE;
use vifast_plat_aarch64::vector;
use vifast_plat_aarch64::{asm, debug_halt};

use crate::{tap, vector_base, Console, TEXT_START_PA};

// Arena stack per-hart do `linker.ld` cấp (`__smp_stacks_start`/`__smp_stacks_end`).
extern "C" {
    static __smp_stacks_start: u8;
    static __smp_stacks_end: u8;
}

// ---------------------------------------------------------------------------------------------
// KT-259 — trạng thái SMP giữa hai hart
// ---------------------------------------------------------------------------------------------
//
// Mọi giao tiếp giữa hai hart đi qua các `AtomicU64` này với `Release`/`Acquire`: hart 0 **công
// bố** dữ liệu rồi mới `CPU_ON`/gửi SGI, hart 1 **đọc** sau khi thấy cờ. Không có lock, không có
// `unsafe` — và vì cache đang **tắt** (`SCTLR_EL1.C = 0`) nên mọi truy cập đều đi thẳng bộ nhớ,
// thứ tự do barrier của `asm` bảo đảm.
//
// **Quy tắc in**: hart nào **in xong** mới được đặt cờ mà hart kia đang chờ (in trước, đếm sau).
// Nếu đặt cờ trước khi in, hai hart in xen ký tự vào cùng PL011 — lỗi thật đã gặp ở lần chạy đầu
// KT-259 (`# smp: # smp: ipiipi  send=recv=1 intiOkd=0`).

/// PA của `_start_secondary` — **phải** đọc trước khi nhảy nửa cao (sau đó `adrp` cho alias).
static SMP_ENTRY_PA: AtomicU64 = AtomicU64::new(0);
/// `TCR_EL1` hart 0 đã dùng — hart 1 nạp lại đúng giá trị này.
static SMP_BOOT_TCR: AtomicU64 = AtomicU64::new(0);
/// `MAIR_EL1` hart 0 đã dùng.
static SMP_BOOT_MAIR: AtomicU64 = AtomicU64::new(0);
/// `TTBR0_EL1` hart 0 đã dùng (bảng nửa thấp của `VSpace`).
static SMP_BOOT_TTBR0: AtomicU64 = AtomicU64::new(0);
/// `TTBR1_EL1` hart 0 đã dùng (bảng nửa cao của `VSpace`).
static SMP_BOOT_TTBR1: AtomicU64 = AtomicU64::new(0);
/// `SCTLR_EL1` hart 0 **sau** khi bật MMU (`M = 1`).
static SMP_BOOT_SCTLR: AtomicU64 = AtomicU64::new(0);
/// Hart 1 đã in `# smp: hart=1 online` và GIC của nó đã sẵn sàng (`1` = online).
static SMP_ONLINE: AtomicU64 = AtomicU64::new(0);
/// Số IPI (`SGI 0`) hart 1 đã nhận — tăng trong **handler IRQ**, không phải vòng chờ.
static SMP_IPI_COUNT: AtomicU64 = AtomicU64::new(0);
/// Hart 0 yêu cầu hart 1 in `# smp: hart=1 alive` (`1` = đã yêu cầu).
static SMP_REPORT_REQ: AtomicU64 = AtomicU64::new(0);
/// Hart 1 đã in `# smp: hart=1 alive` (`1` = đã in).
static SMP_ALIVE: AtomicU64 = AtomicU64::new(0);
/// `MPIDR_EL1` **hart 1 tự đọc** — bằng chứng hart id lấy từ phần cứng, không từ `x3`.
static SMP_HART1_MPIDR: AtomicU64 = AtomicU64::new(0);
/// `context_id` PSCI đã chuyển cho hart 1 (`x0` lúc vào `_start_secondary`).
static SMP_HART1_CTX: AtomicU64 = AtomicU64::new(u64::MAX);
/// `sp` hart 1 tự đọc sau khi bật MMU (VA thấp = PA).
static SMP_HART1_SP: AtomicU64 = AtomicU64::new(0);
/// `GICR_TYPER` redistributor của hart 1 (affinity phải khớp hart id).
static SMP_HART1_TYPER: AtomicU64 = AtomicU64::new(0);
/// `ICC_SRE_EL1` hart 1 đọc lại sau khi bật `SRE` (bằng chứng `SRE = 1`).
static SMP_HART1_SRE: AtomicU64 = AtomicU64::new(0);
/// `GICR_WAKER` hart 1 đọc lại sau khi wake (bằng chứng `ProcessorSleep = 0`).
static SMP_HART1_WAKER: AtomicU64 = AtomicU64::new(0);
/// `sp` của hart 0 **quy về PA** (hart 0 chạy VA nửa cao) — để so slot với hart 1.
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
/// Số vòng hart 0 đã chờ hart 1 online (bằng chứng "chờ bao lâu", không phải hằng số).
static SMP_WAIT_ONLINE: AtomicU64 = AtomicU64::new(0);
/// Số vòng hart 0 đã chờ IPI.
static SMP_WAIT_IPI: AtomicU64 = AtomicU64::new(0);
/// Số vòng hart 0 đã chờ hart 1 báo alive.
static SMP_WAIT_ALIVE: AtomicU64 = AtomicU64::new(0);

/// Trần vòng chờ một cờ SMP — **có trần** để hart 0 không treo im lặng khi hart 1 không lên.
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
pub(crate) fn note_ipi(intid: u64) {
    if intid != gic::SGI_IPI as u64 {
        return;
    }
    let n = SMP_IPI_COUNT.load(Ordering::Acquire) + 1;
    let mut console = Console;
    let _ = writeln!(console, "# smp: ipi recv={n} intid={intid}");
    SMP_IPI_COUNT.store(n, Ordering::Release);
}

/// **KT-259** — đánh thức hart 1, đo IPI `SGI 0`, phát TAP 10..12. Trả `true` ⇔ cả ba ca xanh.
///
/// Thứ tự **bắt buộc** (mỗi bước có dòng `#` làm bằng chứng):
///
/// 1. kiểm bố cục arena stack (linker ↔ crate) — lệch là PANIC, không đoán;
/// 2. GICv3: distributor (`ARE_NS | EnableGrp1NS`) + redistributor **hart 0** + CPU interface;
/// 3. công bố `TCR/MAIR/TTBR0/TTBR1/SCTLR` + PA điểm vào, `dsb sy` + `sev`;
/// 4. `PSCI CPU_ON(1, _start_secondary, 1)` — **không in gì** cho tới khi thấy `SMP_ONLINE`
///    (hart 1 in `# smp: hart=1 online` ngay khi lên; in xen vào sẽ làm hỏng dòng log);
/// 5. TAP 10 (hart 1 online + stack hai hart **khác slot**), gửi `SGI 0`, TAP 11 (IPI),
///    yêu cầu hart 1 báo alive, TAP 12 (cả hai hart báo).
pub(crate) fn smp_bringup(console: &mut Console) -> bool {
    // ---- 1. Bố cục arena: `linker.ld` và `smp` phải khớp **tuyệt đối**.
    //
    // `high_entry` chạy ở VA **nửa cao**, nên `addr_of!` cho alias nửa cao của arena; mọi phép so
    // slot phải dùng **PA** (hart 1 chạy VA thấp, `sp` của nó là PA). Quy đổi bằng
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
    if let Err(error) = gic::enable_sgi(0, gic::SGI_IPI) {
        let _ = writeln!(console, "PANIC: gic: enable SGI hart 0 lỗi {error:?}");
        debug_halt(4)
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

    // ---- 4. `CPU_ON` hart 1. **Không in** giữa lệnh này và lúc thấy `SMP_ONLINE`.
    let _ = writeln!(
        console,
        "# smp: cpu_on begin hart=1 entry=0x{entry:x} ctx=1 fn=0x{:x}",
        smp::PSCI_CPU_ON,
    );
    let rc = asm::psci_cpu_on(1, entry, 1);
    let waited = wait_flag(&SMP_ONLINE, 1);
    SMP_WAIT_ONLINE.store(waited.unwrap_or(u64::MAX), Ordering::Release);

    // ---- 5. TAP 10: hart 1 online + **hai stack khác slot** (đột biến iii).
    let h1_mpidr = SMP_HART1_MPIDR.load(Ordering::Acquire);
    let h1_ctx = SMP_HART1_CTX.load(Ordering::Acquire);
    let h1_sp = SMP_HART1_SP.load(Ordering::Acquire);
    let h1_slot = stacks.slot_of(h1_sp);
    let h0_sp_pa = SMP_HART0_SP_PA.load(Ordering::Acquire);
    let h0_slot = stacks.slot_of(h0_sp_pa);
    let online = waited.is_some();
    let hart1_ok = online
        && smp::cpu_on_ok(rc)
        && hart_index(h1_mpidr) == Some(1)
        && h1_ctx == 1
        && h1_slot == Some(1)
        && h0_slot == Some(0)
        && h0_slot != h1_slot;
    let _ = writeln!(
        console,
        "# smp: cpu_on rc={rc} ({}) online={} waited={} hart1_mpidr=0x{h1_mpidr:016x} ctx={h1_ctx} sp=0x{h1_sp:x} slot1={h1_slot:?} hart0_sp_pa=0x{h0_sp_pa:x} slot0={h0_slot:?}",
        smp::cpu_on_error_name(rc),
        online as u8,
        waited.unwrap_or(u64::MAX),
    );
    if !tap(console, 10, "smp: hart 1 online", hart1_ok) {
        return false;
    }

    // ---- 6. TAP 11: IPI `SGI 0` hart 0 → hart 1 (hart 1 đếm trong **handler IRQ**).
    let sent = gic::send_sgi(1, gic::SGI_IPI);
    let ipi_waited = wait_flag(&SMP_IPI_COUNT, 1);
    SMP_WAIT_IPI.store(ipi_waited.unwrap_or(u64::MAX), Ordering::Release);
    let ipi = SMP_IPI_COUNT.load(Ordering::Acquire);
    let ipi_ok = sent.is_ok() && ipi_waited.is_some() && ipi == 1;
    let _ = writeln!(
        console,
        "# smp: ipi send={sent:?} count={ipi} waited={} sgi={} target=1",
        ipi_waited.unwrap_or(u64::MAX),
        gic::SGI_IPI,
    );
    if !tap(console, 11, "smp: ipi 0->1", ipi_ok) {
        return false;
    }

    // ---- 7. TAP 12: cả hai hart báo alive (hart 1 in trước, hart 0 in sau — không xen dòng).
    SMP_REPORT_REQ.store(1, Ordering::Release);
    asm::dsb_sy();
    asm::sev();
    let alive_waited = wait_flag(&SMP_ALIVE, 1);
    SMP_WAIT_ALIVE.store(alive_waited.unwrap_or(u64::MAX), Ordering::Release);
    let _ = writeln!(console, "# smp: hart=0 alive");
    let alive = SMP_ALIVE.load(Ordering::Acquire) == 1;
    let both_ok = alive && alive_waited.is_some() && hart1_ok && ipi_ok;
    let _ = writeln!(
        console,
        "# smp: report hart0=1 hart1={} waited={} online={} ipi={}",
        alive as u8,
        alive_waited.unwrap_or(u64::MAX),
        online as u8,
        ipi,
    );
    tap(console, 12, "smp: both harts report", both_ok)
}

/// **KT-259** — điểm vào Rust của **hart phụ** (được `_start_secondary` gọi sau khi đã có stack
/// per-hart và `CPACR_EL1` bật).
///
/// `hart` = `Aff0` của `MPIDR_EL1` do **asm** đọc (không phải `x3`); `ctx` = `context_id` PSCI.
///
/// Thứ tự **bắt buộc**:
///
/// 1. kiểm `MPIDR` (topology phẳng), `ctx == hart`, và `sp` nằm trong **slot của chính mình** —
///    đây là chỗ đột biến (iii) "stack chồng nhau" lộ ra (PANIC, không chạy tiếp);
/// 2. nạp `TCR/MAIR/TTBR0/TTBR1/SCTLR` đã công bố theo đúng [`MMU_ENABLE_SEQUENCE`] — **một**
///    định nghĩa chuỗi cho cả hai hart;
/// 3. cài `VBAR_EL1` (VA **thấp**: hart này chạy VA thấp, bảng vector map identity trong `TTBR0`);
/// 4. GICv3 **của chính nó**: wake redistributor → enable `SGI 0` → CPU interface;
/// 5. in `# smp: hart=1 online`, đặt `SMP_ONLINE`, rồi **mở** IRQ (SGI tới sớm vẫn pending);
/// 6. chờ IPI (handler IRQ đếm), chờ yêu cầu báo alive, in `# smp: hart=1 alive`;
/// 7. `wfi` vô hạn — hart 1 **không** được tắt máy (chỉ hart 0 `SYSTEM_OFF`).
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
            "PANIC: smp: hart 1 thấy thanh ghi MMU chưa đủ {regs:?}"
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
            "PANIC: smp: hart 1 bật MMU không thành công sctlr=0x{sctlr_now:016x}"
        );
        debug_halt(4)
    }

    // ---- 3. Bảng vector: VA **thấp** (hart 1 chạy VA thấp; bảng map identity trong TTBR0).
    let vbar = vector::install(vector_base());

    // ---- 4. GICv3 của **chính hart này**.
    let typer = match gic::wake_redistributor(index) {
        Ok(typer) => typer,
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: gic: hart 1 wake redistributor lỗi {error:?}"
            );
            debug_halt(4)
        }
    };
    if let Err(error) = gic::enable_sgi(index, gic::SGI_IPI) {
        let _ = writeln!(console, "PANIC: gic: hart 1 enable SGI lỗi {error:?}");
        debug_halt(4)
    }
    let sre = match gic::init_cpu_interface() {
        Ok(sre) => sre,
        Err(error) => {
            let _ = writeln!(console, "PANIC: gic: hart 1 cpu interface lỗi {error:?}");
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
    SMP_HART1_MPIDR.store(mpidr, Ordering::Release);
    SMP_HART1_CTX.store(ctx, Ordering::Release);
    SMP_HART1_SP.store(sp, Ordering::Release);
    SMP_HART1_TYPER.store(typer, Ordering::Release);
    SMP_HART1_SRE.store(sre, Ordering::Release);
    SMP_HART1_WAKER.store(waker as u64, Ordering::Release);
    SMP_ONLINE.store(1, Ordering::Release);
    asm::dsb_sy();
    asm::sev();
    // Mở IRQ **sau** `SMP_ONLINE`: SGI tới sớm vẫn pending trong GIC, không mất.
    asm::daif_clear_irq();

    // ---- 6. Chờ IPI (handler IRQ đếm), rồi chờ yêu cầu báo alive.
    let mut spins = 0u64;
    while SMP_IPI_COUNT.load(Ordering::Acquire) == 0 && spins < SMP_WAIT_LIMIT {
        spins += 1;
        core::hint::spin_loop();
    }
    if SMP_IPI_COUNT.load(Ordering::Acquire) == 0 {
        let _ = writeln!(console, "# smp: hart={hart} ipi timeout spins={spins}");
    }
    let mut spins = 0u64;
    while SMP_REPORT_REQ.load(Ordering::Acquire) == 0 && spins < SMP_WAIT_LIMIT {
        spins += 1;
        core::hint::spin_loop();
    }
    let _ = writeln!(console, "# smp: hart={hart} alive");
    SMP_ALIVE.store(1, Ordering::Release);
    asm::dsb_sy();
    asm::sev();

    // ---- 7. Hart 1 sống tới khi hart 0 `SYSTEM_OFF` cả máy.
    asm::wfi_forever()
}
