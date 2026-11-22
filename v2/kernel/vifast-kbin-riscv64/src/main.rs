// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **sản phẩm** riscv64 cho K3 (KT-44, KT-45, KT-46, KT-47).
//!
//! Không chứa chuỗi/đường dẫn `ktest` (D2-08); ảnh test là bin `vifast-kbin-ktest`
//! (`src/ktest.rs`, feature `ktest`). KT-45 cài `stvec` **direct mode** ngay đầu boot và in lại
//! `stvec` để chứng minh vector thật trên phần cứng; trap thật được kiểm trong ảnh test.
//!
//! KT-47: sau MMU, ảnh sản phẩm **arm timer 100 Hz** bằng SBI TIME (`sbi_set_timer`) + `sie.STIE`,
//! chờ **≥1 tick** thật (timeout 1 s) rồi in `# timer …` bằng chứng. Timer hỏng ⇒ `PANIC` (runner
//! đỏ), không giả xanh.
//!
//! KT-48: ảnh sản phẩm cũng **bật PLIC + UART IRQ thật** (không chỉ ảnh test): `plic::init` đọc
//! `ndev` từ register, context `2*hart+1`, threshold 0, priority IRQ 10 = 1; bật `IER.TX-empty`,
//! mở cổng `sie.SEIE`/`sstatus.SIE`, chờ handler claim/complete đủ [`UART_IRQ_LIMIT`] ngắt rồi
//! tắt nguồn (không ngập), in `# plic:`/`# uart irq:`; sai ⇒ `PANIC`.
//!
//! KT-49: sau timer, ảnh sản phẩm **bring-up SMP thật** — `smp::init` probe `sbi_hart_get_status`,
//! `hart_start` cho hart `STOPPED` (matrix `-smp 1|2|4`), chờ barrier online, in `# smp:`; hart
//! ngoài DTB bị từ chối + log (không panic cả máy). Hart thứ cấp park (chưa có IPI — KT-50).
//! IPI (KT-50), U-mode (KT-53) **không** thuộc ảnh này.
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_plat_common::plic::{PLIC_STATS, UART_IRQ_LIMIT};
use vifast_plat_common::timer::{ticks_to_ns, TICK_HZ, TICK_TICKS, TIMEBASE_HZ, TIMER_STATS};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::mmu;
use vifast_plat_riscv64::plic;
use vifast_plat_riscv64::smp;
use vifast_plat_riscv64::timer;
use vifast_plat_riscv64::{read_trap_vector, Riscv64Virt, UART0_IRQ};

// **KT-77b** (OD-K6-2): đường U-mode + shell của **ảnh sản phẩm**. Bật bằng feature
// `product-shell` (không `ktest`) nên ảnh không link module harness nào (D2-08).
#[cfg(feature = "product-shell")]
mod proc_abi;
#[cfg(feature = "product-shell")]
mod proc_rt;

/// Nền tảng QEMU `virt` — zero-sized, dựng được trong `static`.
static PLATFORM: Riscv64Virt = Riscv64Virt;
/// Một handler panic dùng chung cho cả ảnh sản phẩm và ảnh test (`vifast-plat-common`).
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// **KT-86** (D2-125.3) — dòng **danh tính bản dựng**, sinh **lúc build** bởi `build.rs`
/// (`cargo:rustc-env`, không viết tay trong mã — D2-102: in từ `const`, không literal):
///
/// ```text
/// # build: algo=<tên định nghĩa dấu vân tay> inputs=<16 hex> dirty=<0|1>
/// ```
///
/// `dirty=1` ⇔ có thay đổi chưa commit trong `v2/kernel`/`v2/lib` **tại thời điểm build**
/// (định nghĩa đầy đủ: `v2/test/boot/build-rev.sh` + `v2/docs/k6-build-rev.md` §2). Harness
/// `v2/test/boot/build-rev.sh` đọc dòng này từ log boot và **TỪ CHỐI ĐO** khi `rev` lệch
/// `git rev-parse HEAD`, khi `dirty=1`, hoặc khi log **không có** dòng này (fail-closed).
///
/// **KT-143**: `algo=` là tên **định nghĩa** đã tính ra `inputs` (`VBREV_ALGO` của harness). Đổi
/// định nghĩa (thêm/bớt thư mục khỏi phạm vi) làm `inputs` đổi mà **không** đổi một byte mã nguồn
/// nào; thiếu `algo=` thì hai log khác số mà người đọc không có cách nào biết chúng có so được
/// với nhau hay không. Harness **từ chối** log thiếu `algo=` hoặc `algo=` khác định nghĩa hiện
/// hành — log cũ (trước KT-143) không được sửa và không được phân tích lại, truy nguyên bằng
/// commit.
const BUILD_LINE: &str = concat!(
    "# build: algo=",
    env!("VIFAST_BUILD_ALGO"),
    " inputs=",
    env!("VIFAST_BUILD_INPUTS"),
    " dirty=",
    env!("VIFAST_BUILD_DIRTY"),
    "\n"
);

/// Được `_start` (trong `vifast-plat-riscv64`) gọi sau khi đặt `sp`/`tp`/`sscratch`.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..1\n");
    PLATFORM.put_str("# k3 image=product (kernel thật trên QEMU virt, không phải model)\n");
    // **KT-86** (D2-125.3): in danh tính bản dựng **ngay đầu boot** (trước mọi PANIC có thể có) để
    // log nào cũng trả lời được "ảnh này build từ mã nào" — harness từ chối đo nếu lệch.
    PLATFORM.put_str(BUILD_LINE);
    // KT-57 / D2-80 — bằng chứng neo **ảnh**: in `# conf image: build=<16 hex> text=<len>`
    // (FNV-1a 64 trên `.text` đã nạp, hàm tái dùng của KT-55) để `soak.sh` băm lại từ ELF và từ
    // chối log không có/lệch hash. Đây là **dòng bằng chứng**, không đổi ngữ nghĩa/threshold.
    vifast_plat_riscv64::report_image_digest();
    // KT-45: cài vector trap direct-mode trước mọi việc khác; in lại stvec để runner thấy giá trị
    // thật (không phải hằng số viết tay trong tài liệu — QD §9 Q12). Trap sống trước MMU để lỗi
    // mapping lúc bật `satp` được dump thay vì treo im lặng.
    PLATFORM.set_trap_vector(vifast_plat_riscv64::trap_vector());
    let stvec = read_trap_vector();
    // KT-46: dựng bảng Sv39 từ arena RAM **xong** mới bật `satp` + `sfence.vma`, rồi in địa chỉ
    // thật (root/arena/guard) do lần boot này sinh ra.
    let mmu = mmu::init();
    // KT-51: ASID thật cho kernel VSpace (lease đầu = ASID 0) — cấp **trước** `smp::init` để hart
    // thứ cấp `adopt_satp` nhận đúng `satp.ASID`; ảnh sản phẩm không có shootdown/TAP nhưng dùng
    // đúng đường ASID của kernel.
    let tlb = vifast_plat_riscv64::tlb::init();
    // KT-52: object layer nội bộ trên Untyped boot (arena RAM sau `__kernel_end` + cửa sổ PA).
    // `init` tự kiểm physmap + `check_objmem`; sai ⇒ PANIC (runner đỏ), không chạy với object
    // layer dở. Ảnh sản phẩm **không** dùng Retype (đó là ảnh test TAP 30..36) nhưng vẫn dựng
    // và tự kiểm để KT-53 (U-mode) dùng lại.
    let obj = vifast_plat_riscv64::obj::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# tlb: asid={} generation={} satp_asid={} active_on=0x{:x}",
            tlb.asid, tlb.generation, tlb.satp_asid, tlb.active_on
        );
        let _ = writeln!(
            w,
            "# obj: arena=0x{:x}-0x{:x} bytes={} untyped={} window=0x{:x} pa0=0x{:x} pa1=0x{:x} physmap={} healthy={}",
            obj.arena_base,
            obj.arena_end,
            obj.arena_bytes,
            obj.untypeds,
            obj.window,
            obj.pa0,
            obj.pa1,
            obj.physmap_ok as u8,
            vifast_plat_riscv64::obj::healthy() as u8
        );
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
            "# hart={} timebase_hz={} tick_hz={} time={} stvec=0x{:x} direct={}",
            PLATFORM.hart_id(),
            PLATFORM.timebase_hz(),
            TICK_HZ,
            PLATFORM.now_ticks(),
            stvec,
            stvec & 0b11 == 0
        );
    }

    // KT-61: core `no_std` **chạy trên nhân thật** — `Machine` static trong `.bss` của ảnh
    // (D2-84), khởi tạo từ vùng RAM **nhãn PA** ngay sau arena object layer (D2-83: core không sở
    // hữu Frame/PageTable/VSpace — object layer vẫn giữ). Ảnh sản phẩm chạy **một vòng đời TCB
    // thật** (Retype→Configure→Suspend/Resume→revoke) để soak 1.000 boot kiểm core trên kernel
    // sản phẩm, không chỉ trên ảnh test.
    let core = vifast_plat_riscv64::core_bridge::init(
        obj.arena_end,
        vifast_plat_riscv64::core_bridge::CORE_ARENA_BYTES,
    );
    let smoke = vifast_plat_riscv64::core_bridge::boot_smoke(core.untyped);
    let fp = core.footprint;
    let mut fp_sum = 0usize;
    for i in 0..vifast_core::FOOTPRINT_PARTS {
        fp_sum += fp.bytes[i];
    }
    let fp_ok = fp_sum + fp.scalars + fp.padding == fp.total && fp.padding <= 8;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# core: product addr=0x{:x} bytes={} pin={} image_cap={} untyped={} retype={} configure={} suspend={} resume={} state_ready={} objects={} caps={} used={} check_caps={} check_bytes={} check_all_bytes={} check_sched={} footprint_ok={}",
            core.addr,
            core.static_bytes,
            core.pin,
            vifast_plat_riscv64::core_bridge::IMAGE_CAP_BYTES,
            core.arena_bytes,
            smoke.retype as u8,
            smoke.configure as u8,
            smoke.suspend as u8,
            smoke.resume as u8,
            smoke.state_ready as u8,
            smoke.objects,
            smoke.caps,
            smoke.used,
            core.check_caps as u8,
            core.check_bytes as u8,
            core.check_all_bytes as u8,
            smoke.checks as u8,
            fp_ok as u8,
        );
    }
    let core_ok = core.healthy()
        && fp_ok
        && smoke.retype
        && smoke.configure
        && smoke.suspend
        && smoke.resume
        && smoke.state_ready
        && smoke.checks
        && smoke.used == 0
        && smoke.caps == 1;
    if !core_ok {
        PLATFORM.put_str("PANIC: core init product\n");
        PLATFORM.halt(1)
    }

    // KT-48: **PLIC + UART IRQ thật** — `init` đọc `ndev` từ register (probe priority, không giả
    // định 95), context S-mode `2*hart+1`, threshold 0, priority IRQ 10 = 1, enable IRQ 10. Bật
    // `IER.TX-empty` rồi mở cổng `sie.SEIE`/`sstatus.SIE`; handler claim → đọc IIR → complete
    // (re-arm `IER` để có ngắt kế) và tự tắt `IER` + enable sau `UART_IRQ_LIMIT` ngắt.
    let plic = plic::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# plic: base=0x{:x} ndev={} hart={} ctx={} threshold={} priority_uart={} enable_ctx={} enable_mmode={} claim_idle={}",
            plic.base,
            plic.ndev,
            plic.hart,
            plic.context,
            plic.threshold,
            plic.priority_uart,
            plic.enable_ctx as u8,
            plic.enable_mmode as u8,
            plic.claim_idle
        );
    }
    let uart_ier_on = plic::uart_enable_tx_irq();
    let external_gate_on = plic::enable_external();
    let plic_start = PLATFORM.now_ticks();
    let mut plic_snap = PLIC_STATS.get(PLATFORM.hart_id()).snapshot();
    while plic_snap.uart_irqs < UART_IRQ_LIMIT {
        if PLATFORM.now_ticks().wrapping_sub(plic_start) > PLATFORM.timebase_hz() {
            PLATFORM.put_str("PANIC: uart irq khong den trong 1 s\n");
            PLATFORM.halt(1)
        }
        core::hint::spin_loop();
        plic_snap = PLIC_STATS.get(PLATFORM.hart_id()).snapshot();
    }
    plic::disable_external();
    let uart_ier_off = plic::uart_disable_tx_irq();
    let plic_enable_off = !plic::irq_enabled(UART0_IRQ);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# uart irq: irqs={} claims={} completes={} balanced={} spurious={} unexpected={} last_irq={} last_iir=0x{:x} ier_off={} plic_enable={}",
            plic_snap.uart_irqs,
            plic_snap.claims,
            plic_snap.completions,
            plic_snap.balanced() as u8,
            plic_snap.spurious,
            plic_snap.unexpected,
            plic_snap.last_irq,
            plic_snap.last_iir,
            uart_ier_off as u8,
            plic::irq_enabled(UART0_IRQ) as u8
        );
    }
    let plic_ok = plic.is_configured()
        && uart_ier_on
        && external_gate_on
        && plic_snap.uart_irqs >= 1
        && plic_snap.claims == plic_snap.completions
        && plic_snap.spurious == 0
        && plic_snap.unexpected == 0
        && plic_snap.last_irq == UART0_IRQ as u64
        && plic_snap.last_iir == 0x2
        && plic_snap.uart_ier_off == 1
        && uart_ier_off
        && plic_enable_off
        && !plic::external_gate_enabled();
    if !plic_ok {
        PLATFORM.put_str("PANIC: plic init product\n");
        PLATFORM.halt(1)
    }

    // KT-47: **timer SBI** — arm deadline đầu (`now + TICK_TICKS`) rồi mới bật `sie.STIE` +
    // `sstatus.SIE` (P2a: trap entry/handler không ghi `sstatus`; `sret` khôi phục SIE từ SPIE).
    let timer_arm = timer::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer arm: now={} first_deadline={} tick_ticks={} tick_hz={} timebase_hz={} sie_stie={} sstatus_sie={}",
            timer_arm.now,
            timer_arm.first_deadline,
            TICK_TICKS,
            TICK_HZ,
            TIMEBASE_HZ,
            timer_arm.sie_stie as u8,
            timer_arm.sstatus_sie as u8
        );
    }
    // Chờ **≥1 tick thật**; timeout 1 s (100 tick) để boot hỏng không treo im lặng. Busy-spin để
    // không phụ thuộc `wfi` (QEMU chỉ phục vụ timer ảo khi vCPU chạy — xem `ktest.sh` về icount).
    let start = PLATFORM.now_ticks();
    let mut snap = TIMER_STATS.get(PLATFORM.hart_id()).snapshot();
    while snap.ticks == 0 {
        if PLATFORM.now_ticks().wrapping_sub(start) > PLATFORM.timebase_hz() {
            PLATFORM.put_str("PANIC: timer khong tick trong 1 s\n");
            PLATFORM.halt(1)
        }
        core::hint::spin_loop();
        snap = TIMER_STATS.get(PLATFORM.hart_id()).snapshot();
    }
    let sret_sie = timer::read_sstatus() & timer::SSTATUS_SIE != 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# timer: ticks={} delta_ns={} late={} missed={} non_monotonic={} last={} deadline={} sret_sie={}",
            snap.ticks,
            ticks_to_ns(snap.max_delta),
            snap.late,
            snap.missed,
            snap.non_monotonic,
            snap.last,
            snap.deadline,
            sret_sie as u8
        );
    }
    // Dừng timer: deadline xa dập ngắt đang chờ rồi tắt cổng (sstatus.SIE → sie.STIE).
    PLATFORM.set_timer_deadline(u64::MAX);
    PLATFORM.disable_timer();
    let timer_ok = timer_arm.sie_stie
        && timer_arm.sstatus_sie
        && sret_sie
        && snap.ticks >= 1
        && snap.non_monotonic == 0
        && timer::read_sie() & timer::SIE_STIE == 0
        && timer::read_sstatus() & timer::SSTATUS_SIE == 0;
    // Ảnh sản phẩm chỉ TAP 1 dòng, nhưng MMU + timer phải thật: satp mode 8, đọc lại được, alias
    // ghi/đọc khớp, timer nạp deadline + bật SIE + có ≥1 ngắt thật qua `stvec`. Sai ⇒ PANIC
    // (run.sh coi là đỏ), không giả xanh.
    if mmu.satp >> 60 != 8 || !mmu::mmu_enabled() || !mmu.alias_ok {
        PLATFORM.put_str("PANIC: mmu init product\n");
        PLATFORM.halt(1)
    }
    if !timer_ok {
        PLATFORM.put_str("PANIC: timer init product\n");
        PLATFORM.halt(1)
    }

    // KT-49 SMP: probe hart theo SBI HSM, `hart_start` cho hart STOPPED (matrix `-smp 1|2|4`), chờ
    // barrier online. Hart ngoài DTB/lỗi SBI bị log + bỏ qua trong `smp::init` — **hart lỗi không
    // panic cả máy**; ảnh sản phẩm chỉ in `# smp: WARN` ở dưới, `run.sh` đỏ nếu `harts` khác
    // `-smp N` nên cấu hình hỏng không bị che.
    let smp = smp::init();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# smp: boot={} harts={} present=0x{:x} online=0x{:x} started=0x{:x} absent=0x{:x} timeout={} dtb=0x{:x} dtb_magic={} base=0x{:x} stride=0x{:x}",
            smp.boot_hart,
            smp.harts,
            smp.present_mask,
            smp.online_mask,
            smp.started_mask,
            smp.absent_mask,
            smp.timed_out as u8,
            smp.dtb,
            smp.dtb_magic_ok as u8,
            smp.hart_area_base,
            smp.hart_stride
        );
    }
    // KT-49: hart lỗi **không panic cả máy** — log WARN rồi tiếp tục; runner `run.sh` vẫn đỏ nếu
    // `harts` khác `-smp N` (cấu hình hỏng không bị che). DTB magic chỉ là chẩn đoán (D2-72 Q7).
    if !smp.dtb_magic_ok {
        PLATFORM.put_str("# smp: WARN dtb magic khong hop le (chi chan doan, tiep tuc)\n");
    }
    if !smp.is_online() {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# smp: WARN thieu hart online present=0x{:x} online=0x{:x} missing=0x{:x} timeout={} (tiep tuc, khong panic ca may)",
            smp.present_mask,
            smp.online_mask,
            smp.missing_mask(),
            smp.timed_out as u8
        );
    }
    for hart in 0..vifast_plat_common::smp::MAX_HARTS {
        let present = smp.present_mask & (1u64 << hart) != 0;
        let absent = smp.absent_mask & (1u64 << hart) != 0;
        if !present && !absent && smp.status[hart] != -99 && smp.status[hart] < 0 {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "# smp: WARN hart {} status={} loi SBI (bo qua, khong panic ca may)",
                hart, smp.status[hart]
            );
        }
    }
    PLATFORM.put_str("ok 1 - k3 boot\n");
    // **KT-77b**: ảnh `product-shell` giao máy cho runtime U-mode (procsrv + shell) và
    // **không** halt sau boot — shell sống để nhận kịch bản từ serial. Ca âm của tiêu chí 3
    // là build **không** feature này: đường dưới giữ **nguyên** byte (`# k3 halt` + halt(0)).
    #[cfg(feature = "product-shell")]
    {
        PLATFORM.put_str("# k3 shell: handoff product-shell=1\n");
        proc_rt::run(&PLATFORM, obj.untyped0, core.untyped);
    }
    #[cfg(not(feature = "product-shell"))]
    {
        PLATFORM.put_str("# k3 halt\n");
        PLATFORM.halt(0)
    }
}

/// Entry hart **thứ cấp** của ảnh sản phẩm (KT-49): adopt `satp`/`stvec` của BSP, báo online, rồi
/// park (SMP chưa có việc cho AP ở ảnh sản phẩm; IPI/scheduler là KT-50/K3b).
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
// **KT-77c** (D2-123.2): dịch vụ FS + virtio-blk của ảnh sản phẩm — chỉ biên dịch khi bật
// `product-fs` (kéo theo `product-shell` + nhãn `block` của crate arch, **không** `ktest`).
//
// Vì sao khai báo `mod` ở **cuối tệp** (không phải cạnh `mod proc_rt` ở đầu): ảnh **không**
// feature phải **giữ nguyên từng byte** — baseline `c1072d72` (KT-79, D2-122). Chèn ba dòng ở
// giữa tệp làm **số dòng** của mọi panic location phía sau lệch +3, và `.rodata` đổi 5 byte (đo
// được: `JOURNAL/kt77c-logs/kt77c-baseline-lineshift.log`). Khai báo ở cuối ⇒ không dòng nào
// của mã cũ bị dịch ⇒ baseline giữ đúng sha.
#[cfg(feature = "product-fs")]
mod proc_fs;
// **KT-342** (D2-233/D2-243): mạng + exec của ảnh **sản phẩm** — cùng điều kiện biên dịch theo
// feature sản phẩm, **không** kéo `ktest` (D2-08). Khai báo ở cuối tệp để ảnh không feature giữ
// nguyên từng dòng panic location (cùng lý do `mod proc_fs` của KT-77c).
#[cfg(feature = "product-net")]
mod net_svc;
#[cfg(feature = "product-exec")]
mod proc_exec;
#[cfg(feature = "product-net")]
mod proc_net;
