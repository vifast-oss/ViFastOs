// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Miền trap của ảnh x86_64** — IST1 per-hart, trạng thái exception, arm/resume và handler
//! chung (`exc_dispatch`) — tách khỏi `main.rs` ở **KT-340b** (luật R1).
//!
//! # Vì sao tách (không phải cắt cơ học)
//!
//! `KT-340b` phải đưa `main.rs` từ **1.500** (trần R1) về **< 1.100** dòng. Miền "trap" là một
//! **miền riêng**: toàn bộ trạng thái exception (`EXC_*`, `EXPECT_VEC`, `RESUME_RIP`,
//! `EXPECT_CR2`), các hàm arm/reset/kiểm, và **một** handler `exc_dispatch` mà stub asm gọi. Nó
//! chỉ đọc/ghi trạng thái của chính nó + ba bộ đếm APIC/timer (`TICK_COUNT`/`EOI_COUNT`/
//! `SPURIOUS_COUNT`) và gọi `sched`/`smp` — không có đường nào khác trong `main.rs` phụ thuộc
//! thân handler, nên tách nguyên khối **không** đổi ngữ nghĩa (khuôn `KT-338c`).
//!
//! # Bất biến (giữ nguyên từ KT-261/KT-266/KT-278/KT-286/KT-298/KT-304/KT-340a)
//!
//! - handler **không bao giờ** resume một exception không được arm: in `# exc:` + `# exc: frame`
//!   rồi `halt(2)` — che fault thật bằng resume là điều tệ nhất;
//! - ngắt (timer/spurious/IPI) **không** in `# exc:` (bất biến đếm `# exc:` của runner);
//! - `rsp` của tác vụ kế đi qua **giá trị trả** (`rdx`), không qua ô `static` (KT-298);
//! - `#PF` **đã arm** của tác vụ ring 3 (KT-340b) in `# vspace: cross fault …` và resume đúng
//!   nhãn — **không** đụng `# exc:`; `CR2`/`cs` lệch ⇒ `halt` (fail-closed).

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_x86_64::asm;
use vifast_plat_x86_64::exc;
use vifast_plat_x86_64::exc::FrameReturn;
use vifast_plat_x86_64::ipi;

use crate::smp;
use crate::{lapic_eoi, lapic_id, Console, EOI_COUNT, SPURIOUS_COUNT, TICK_COUNT};

/// Sentinel "không chờ exception nào" — khác mọi vector thật (0..31) và [`exc::VEC_UNKNOWN`].
const NO_EXPECT: u64 = u64::MAX;

// ---------------------------------------------------------------------------------------------
// IST1 per-hart — KT-278
// ---------------------------------------------------------------------------------------------

// Bốn stack IST1 (mỗi hart một stack 4 KiB) do `global_asm!` ở trên khai trong `.bss`.
extern "C" {
    static __ist1_top: u8;
    static __ist1_stacks_end: u8;
}

/// Đỉnh stack `IST1` của `hart` — `__ist1_top + hart * 0x1000`.
///
/// `__ist1_top` là đỉnh stack của **hart 0** (giá trị `long_mode_entry` truyền vào `kmain`), nên
/// hart 0 giữ nguyên đường boot; hart `h` dùng stack kế tiếp. `hart >= 4` trả `0` để caller
/// fail-closed thay vì trỏ vào rác.
pub(crate) fn ist1_top(hart: usize) -> u64 {
    let base = core::ptr::addr_of!(__ist1_top) as u64;
    let end = core::ptr::addr_of!(__ist1_stacks_end) as u64;
    let top = base + (hart as u64) * 0x1000;
    if top > end {
        return 0;
    }
    top
}

// ---------------------------------------------------------------------------------------------
// Trạng thái exception — handler ghi, ca kiểm đọc
// ---------------------------------------------------------------------------------------------

/// Số exception đã vào handler (bằng chứng "handler chạy thật", không chỉ in).
pub(crate) static EXC_COUNT: AtomicU64 = AtomicU64::new(0);
/// Vector của exception **gần nhất**.
pub(crate) static EXC_VEC: AtomicU64 = AtomicU64::new(NO_EXPECT);
/// Mã lỗi của exception gần nhất.
pub(crate) static EXC_ERR: AtomicU64 = AtomicU64::new(NO_EXPECT);
/// `rip` của exception gần nhất (CPU đẩy — **địa chỉ lệnh gây lỗi**).
pub(crate) static EXC_RIP: AtomicU64 = AtomicU64::new(0);
/// `cs` của exception gần nhất.
pub(crate) static EXC_CS: AtomicU64 = AtomicU64::new(0);
/// `rflags` của exception gần nhất.
pub(crate) static EXC_RFLAGS: AtomicU64 = AtomicU64::new(0);
/// `cr2` tại thời điểm exception gần nhất (chỉ có nghĩa với `#PF`).
pub(crate) static EXC_CR2: AtomicU64 = AtomicU64::new(0);
/// Vector mà ca kiểm **đang chờ** (`NO_EXPECT` = không chờ) — quyết định resume hay halt.
static EXPECT_VEC: AtomicU64 = AtomicU64::new(NO_EXPECT);
/// `rip` sẽ resume cho `#PF`/`#GP` (nhãn ngay sau lệnh gây lỗi).
static RESUME_RIP: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------------------------
// **KT-340b** — `#PF` **của tác vụ ring 3** trong `VSpace` riêng (cô lập bộ nhớ)
// ---------------------------------------------------------------------------------------------

/// `CR2` mà ca `# vspace: cross fault` **đang chờ** (`0` = không chờ). Khác `EXPECT_VEC` ở chỗ
/// nó ràng buộc **địa chỉ** của fault: một `#PF` khác trong lúc chờ **không** được resume mù —
/// handler in chẩn đoán rồi `halt` (fail-closed), vì resume một fault thật là điều tệ nhất.
static EXPECT_CR2: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ đã thấy đúng `#PF` cô lập (bằng chứng cho TAP 14 + dòng `# vspace:`).
static VSPACE_FAULT: AtomicU64 = AtomicU64::new(0);
/// `1` ⇔ phép đo `INVLPG` (`map → đọc → unmap → map → đọc`) cho kết quả đúng.
static VSPACE_PROBE_OK: AtomicU64 = AtomicU64::new(0);

/// Arm `#PF` **của ring 3** tại đúng `cr2`, resume ở `resume` (VA user của nhãn ngay sau lệnh
/// ghi). Chỉ dùng cho ca `VSpace` của `KT-340b`.
pub(crate) fn exc_arm_user_pf(cr2: u64, resume: u64) {
    RESUME_RIP.store(resume, Ordering::Relaxed);
    EXPECT_CR2.store(cr2, Ordering::Relaxed);
    EXPECT_VEC.store(VEC_PF, Ordering::Relaxed);
}

/// `1` ⇔ ca cô lập `VSpace` đã **thấy fault thật** (handler ghi khi vào).
pub(crate) fn vspace_fault_seen() -> bool {
    VSPACE_FAULT.load(Ordering::Relaxed) == 1
}

/// Ghi kết quả phép đo `INVLPG` (1 = đúng) — đọc lại ở `TAP 14`.
pub(crate) fn vspace_set_probe_ok(ok: bool) {
    VSPACE_PROBE_OK.store(ok as u64, Ordering::Relaxed);
}

/// `true` ⇔ fault **và** phép đo `INVLPG` đều đúng — điều kiện `ok` của TAP 14.
pub(crate) fn vspace_ok() -> bool {
    vspace_fault_seen() && VSPACE_PROBE_OK.load(Ordering::Relaxed) == 1
}

/// Vector `#BP` dưới dạng `u64` — hằng để dùng làm pattern.
pub(crate) const VEC_BP: u64 = exc::VEC_BP as u64;
/// Vector `#UD`.
pub(crate) const VEC_UD: u64 = exc::VEC_UD as u64;
/// Vector `#GP`.
pub(crate) const VEC_GP: u64 = exc::VEC_GP as u64;
/// Vector `#PF`.
pub(crate) const VEC_PF: u64 = exc::VEC_PF as u64;

/// Xoá bằng chứng cũ trước một ca kiểm — nếu handler **không** chạy, `EXC_VEC` vẫn là
/// [`NO_EXPECT`] và ca kiểm đỏ (không thể "xanh nhờ dữ liệu cũ").
pub(crate) fn exc_reset() {
    EXC_VEC.store(NO_EXPECT, Ordering::Relaxed);
    EXC_ERR.store(NO_EXPECT, Ordering::Relaxed);
    EXC_RIP.store(0, Ordering::Relaxed);
    EXC_CS.store(0, Ordering::Relaxed);
    EXC_RFLAGS.store(0, Ordering::Relaxed);
    EXC_CR2.store(0, Ordering::Relaxed);
}

/// Arm ca kiểm: chờ `vec`, và (với `#PF`/`#GP`) resume ở `resume`.
///
/// **KT-418** — xoá luôn `EXPECT_CR2`: nhánh `#PF` **theo `CR2`** là của ca cô lập `VSpace`
/// (`exc_arm_user_pf`). Không xoá thì một `EXPECT_CR2` còn sót từ phép đo trước **nuốt** fault
/// của ca mới: đo được `# vspace: BAD fault vec=14 err=0x11 cr2=0x1000000000 cs=0x8
/// expect_cr2=0x1000003000` (fault SMEP đúng, nhưng handler đi nhánh `CR2` rồi halt).
pub(crate) fn exc_arm(vec: u64, resume: u64) {
    RESUME_RIP.store(resume, Ordering::Relaxed);
    EXPECT_CR2.store(0, Ordering::Relaxed);
    EXPECT_VEC.store(vec, Ordering::Relaxed);
}

/// `true` ⇔ exception gần nhất đúng `(vec, err, rip)` và đến từ kernel (`cs = 0x08`).
pub(crate) fn exc_caught(vec: u64, err: u64, rip: u64) -> bool {
    EXC_VEC.load(Ordering::Relaxed) == vec
        && EXC_ERR.load(Ordering::Relaxed) == err
        && EXC_RIP.load(Ordering::Relaxed) == rip
        && EXC_CS.load(Ordering::Relaxed) == exc::KERNEL_CS
}

/// Handler **chung** của mọi exception **và** ngắt — stub asm (`vifast_isr_common`) gọi hàm này.
///
/// KT-266 thêm hai nhánh **ngắt** (vector 32 timer, vector 255 spurious) xử lý **trước** mọi
/// thứ khác và **không** in dòng `# exc:`; phần còn lại vẫn là đường exception của KT-261.
///
/// Trả [`FrameReturn`] — **hai** qword trong `rax`/`rdx` (SysV): `rip` để resume (⇒ stub `iretq`
/// về đó) và `rsp` frame tác vụ kế (`0` ⇒ giữ frame hiện tại). `rip = 0` ⇒ stub halt sạch. Hàm
/// **không** bao giờ trả về nếu exception không phải ca kiểm đã arm: nó in nguyên nhân rồi
/// `halt(2)` — che một fault thật bằng cách resume là điều tệ nhất có thể làm.
///
/// # Vì sao nhận **giá trị** chứ không nhận con trỏ frame
///
/// Ảnh này **không** được có `unsafe` (R5), nên không thể deref con trỏ frame. Stub asm đọc 5
/// trường của frame và truyền thành 5 tham số SysV; **KT-286** thêm tham số thứ 6 `frame_rsp`
/// (địa chỉ frame, **giá trị** `u64`) để scheduler lưu `rsp` của tác vụ đang chạy — cùng thông
/// tin, không cần `unsafe`.
///
/// # Vì sao `rsp` trả về trong **thanh ghi** (KT-298)
///
/// KT-286 đặt `rsp` đích vào `static VIFAST_SCHED_NEW_RSP` — ô **dùng chung mọi hart**. Hart phụ
/// nhận tick PIT trong khoảng giữa lúc handler ghi ô và lúc stub đọc ô sẽ **lấy trộm** giá trị
/// của BSP ⇒ hai hart chạy **cùng** một ngữ cảnh trên **cùng** một stack ⇒ `#GP` storm (boot
/// 410/600 của KT-293). `rax`/`rdx` là **của hart đang chạy** ⇒ không có cửa sổ, không cần khoá.
#[cfg(feature = "ktest")]
#[no_mangle]
pub extern "C" fn exc_dispatch(
    vec: u64,
    err: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    frame_rsp: u64,
) -> FrameReturn {
    // ---- KT-266: **ngắt** (không phải exception) — xử lý trước, **không** in dòng `# exc:`.
    //
    // Timer: tăng tick, ghi `EOI`, tăng bộ đếm EOI, rồi `iretq` về đúng chỗ bị ngắt. Không in
    // gì: một dòng log mỗi tick sẽ làm nghẽn COM1 và biến phép đo thành đo tốc độ UART.
    if vec == exc::VEC_TIMER as u64 {
        // **KT-276**: vector 32 dùng chung cho tick PIT của BSP **và** timer đánh thức của hart
        // phụ. `LAPIC_ID` đọc qua MMIO là **của hart đang chạy**, nên đây là chỗ duy nhất phân
        // biệt được ai vừa ngắt — không có nó, tick của AP làm bẩn `TICK_COUNT`/`EOI_COUNT` của
        // BSP (và `timer: eoi` sẽ đỏ oan).
        let id = lapic_id() as u64;
        if id == smp::bsp_lapic_id() {
            TICK_COUNT.fetch_add(1, Ordering::Relaxed);
            // **Đột biến (iii)** (`--features no-eoi`): không `EOI` ⇒ LAPIC giữ ISR bit của
            // vector 32 ⇒ timer **tắt sau tick đầu** (ĐỎ ở TAP 10 `timer: ticks`).
            if !cfg!(feature = "no-eoi") {
                lapic_eoi();
            }
            EOI_COUNT.fetch_add(1, Ordering::Relaxed);
        } else {
            smp::note_ap_tick();
            lapic_eoi();
        }
        return FrameReturn::resume(rip);
    }
    // ---- **KT-413** (nợ `N-3`): **watchdog LAPIC** (vector `0x32`) — nguồn đánh thức dự phòng
    // cho `hlt` khi mất IRQ0. Handler **chỉ** tăng `WATCHDOG_WAKES` rồi `EOI`:
    //
    // - **không** đụng `TICK_COUNT`/`EOI_COUNT` — nếu đụng, một ngắt watchdog sẽ **giả** một tick
    //   và phép đo `hz`/`ticks_ok`/`eoi == ticks` mất nghĩa (đó là lý do phải có vector **riêng**,
    //   không dùng lại vector 32);
    // - **không** in gì: watchdog bắn ~100 lần trong cửa sổ 500 ms, in mỗi lần sẽ nghẽn COM1 và
    //   biến phép đo thành đo tốc độ UART (bài học KT-259/KT-278).
    if vec == exc::VEC_WATCHDOG as u64 {
        crate::WATCHDOG_WAKES.fetch_add(1, Ordering::Relaxed);
        lapic_eoi();
        return FrameReturn::resume(rip);
    }
    // Spurious: SDM Vol 3A §10.9 — **không** `EOI`, chỉ đếm rồi quay lại.
    if vec == exc::VEC_SPURIOUS as u64 {
        SPURIOUS_COUNT.fetch_add(1, Ordering::Relaxed);
        return FrameReturn::resume(rip);
    }
    // **KT-278**: IPI (vector `0x30`) — hart đích đếm + in `# smp: ipi recv=<n> intid=0x30` rồi
    // `EOI`. In **trước**, tăng biến đếm **sau** (quy tắc "in trước, đếm sau" của KT-259): BSP
    // chờ đúng biến đếm nên dòng log chắc chắn đã ra trước khi BSP in dòng của nó.
    if vec == exc::VEC_IPI as u64 {
        smp::note_ipi();
        // **KT-340d** — `IpiMailbox` **dùng chung** (`vifast-plat-common`) + shootdown TLB: rút
        // **mọi** kind đang chờ trên hart này rồi phục vụ. `TlbFlush` ⇒ `invlpg(va)` (+ `CR3` nếu
        // yêu cầu chỉ định gốc khác) rồi ghi ACK — thứ tự "việc trước, ACK sau" nằm trong
        // `ipi::drain_current`/`service_flush`. `Wake`/`VmKick` chỉ được đếm: tầng arch **không**
        // tự đánh thức tác vụ (D2-244 tầng 2 mới mang payload; `Wake` do `vifast-core` phát).
        let _kinds = ipi::drain_current();
        lapic_eoi();
        return FrameReturn::resume(rip);
    }

    // **KT-340b** — `#PF` **đã arm** của tác vụ ring 3 trong `VSpace` **riêng**: tác vụ ghi vào
    // `VSPACE_PEER_VA` — VA **không** map trong VSpace của nó (chỉ VSpace của tác vụ kia map) ⇒
    // fault thật (`P=0, W=1, U=1` ⇒ `err=0x6`, `cs=0x1b`). Đây là **ca kiểm cô lập bộ nhớ**: nếu
    // bảng trang dùng chung (trước KT-340b), lệnh ghi **không** fault và ca này đỏ vì thiếu dòng.
    //
    // Nhánh này **không** in `# exc:` (giữ nguyên bất biến đếm `# exc:` của runner) và **không**
    // resume mù: `CR2`/`cs` phải khớp đúng kỳ vọng, lệch ⇒ `halt` có chẩn đoán.
    if vec == VEC_PF {
        let expect_cr2 = EXPECT_CR2.load(Ordering::Relaxed);
        if expect_cr2 != 0 {
            let now_cr2 = asm::read_cr2();
            if now_cr2 == expect_cr2 && cs == vifast_plat_x86_64::umode::USER_CS as u64 {
                EXPECT_CR2.store(0, Ordering::Relaxed);
                EXPECT_VEC.store(NO_EXPECT, Ordering::Relaxed);
                VSPACE_FAULT.store(1, Ordering::Relaxed);
                let mut console = Console;
                let _ = writeln!(
                    console,
                    "# vspace: cross fault vec=14 err=0x{err:x} cr2=0x{now_cr2:x} cs=0x{cs:x} rip=0x{rip:x} user=1"
                );
                return FrameReturn::resume(RESUME_RIP.load(Ordering::Relaxed));
            }
            let mut console = Console;
            let _ = writeln!(
                console,
                "# vspace: BAD fault vec={vec} err=0x{err:x} cr2=0x{now_cr2:x} cs=0x{cs:x} expect_cr2=0x{expect_cr2:x}"
            );
            asm::halt(2)
        }
    }

    let cr2 = asm::read_cr2();
    let mut console = Console;
    let _ = writeln!(
        console,
        "# exc: vec={vec} err=0x{err:x} rip=0x{rip:x} cs=0x{cs:x} rflags=0x{rflags:x}"
    );
    if vec == VEC_PF {
        let _ = writeln!(console, "# exc: cr2=0x{cr2:x}");
    }

    EXC_COUNT.fetch_add(1, Ordering::Relaxed);
    EXC_VEC.store(vec, Ordering::Relaxed);
    EXC_ERR.store(err, Ordering::Relaxed);
    EXC_RIP.store(rip, Ordering::Relaxed);
    EXC_CS.store(cs, Ordering::Relaxed);
    EXC_RFLAGS.store(rflags, Ordering::Relaxed);
    EXC_CR2.store(cr2, Ordering::Relaxed);

    if EXPECT_VEC.load(Ordering::Relaxed) == vec {
        EXPECT_VEC.store(NO_EXPECT, Ordering::Relaxed);
        return FrameReturn::resume(match vec {
            // `INT3` là **software interrupt**: RIP đã lưu đã trỏ lệnh **kế tiếp** ⇒ resume nguyên.
            VEC_BP => rip,
            // `ud2` = 2 byte (`0F 0B`) ⇒ `rip + 2`.
            VEC_UD => rip + 2,
            // `int imm8` cũng là **software interrupt** ⇒ RIP đã lưu trỏ lệnh kế tiếp.
            exc::VEC_UNKNOWN => rip,
            // `#PF`/`#GP`: độ dài lệnh gây lỗi không biết chắc ⇒ nhảy tới nhãn resume do ca kiểm
            // đặt (ngay sau lệnh, cùng trạng thái stack).
            _ => RESUME_RIP.load(Ordering::Relaxed),
        });
    }

    // Exception **không** được chờ: in **chẩn đoán** rồi halt sạch — không quay lại chỗ lỗi.
    //
    // **KT-298** — dòng chẩn đoán thứ hai `# exc: frame …` (không khớp `^# exc: vec=` nên
    // **không** đổi bất biến đếm-exception của `soak-x86_64.sh`/`run-x86_64.sh`). Vì sao cần:
    // boot 410/600 của KT-293 chỉ có `# exc: vec=13 err=0x0 rip=0x10890` — **không** đủ để biết
    // fault xảy ra trên **stack nào**. `frame_rsp` + `ss`/`rsp` đọc từ frame (khi frame có hai ô
    // ấy, tức frame 22 qword của tác vụ) phân biệt được "fault trên stack boot" với "fault trên
    // stack tác vụ" — đúng câu hỏi đã chặn KT-293.
    let _ = writeln!(
        console,
        "# exc: frame rsp=0x{frame_rsp:x} ss=0x{:x} task_rsp=0x{:x}",
        asm::read_u64(frame_rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 16),
        asm::read_u64(frame_rsp + exc::FRAME_RFLAGS_OFFSET as u64 + 8),
    );
    asm::halt(2)
}
