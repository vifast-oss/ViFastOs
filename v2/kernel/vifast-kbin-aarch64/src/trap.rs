// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338c** — miền **trap** của ảnh aarch64 (tách khỏi `main.rs` để giữ R1 ≤ 1.500 dòng/tệp).
//!
//! Đây là toàn bộ phần **sau khi** `vec_common` (asm trong `main.rs`) đã lưu khung thanh ghi và
//! đọc `ESR_EL1`/`FAR_EL1`/`ELR_EL1`/`SPSR_EL1`: phân loại trap theo **ô vector** + `ESR`, rồi
//! quyết định `eret` (đường resume) hay dừng máy.
//!
//! | Miền | Nội dung |
//! |---|---|
//! | BRK | `brk #0` cố ý của KT-248 — ghi `BRK_*` để `kmain` in bằng chứng, `ELR += 4` rồi resume |
//! | IRQ | GICv3: `ICC_IAR1_EL1` → `shootdown::handle_ipi` / `smpboot::note_ipi` → `ICC_EOIR1_EL1` |
//! | SVC/U-mode | `umode::handle` (KT-269) — `svc` từ EL0, fault từ EL0 |
//! | fault | `FaultKind::Vm` (KT-254) + fault thô W^X (KT-277) khi hart đã **đăng ký** chờ |
//!
//! Mọi trap **không** thuộc bốn miền trên là **fatal** (in `# trap:` rồi `debug_halt`): che lỗi
//! là điều tệ nhất có thể làm.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};
use vifast_plat_aarch64::vector;
use vifast_plat_aarch64::{asm, debug_halt, gic, shootdown, smp};
use vifast_plat_common::fault::{vm_reason, FaultAccess, FaultKind};

use crate::bootmmu::VSPACE_ID;
use crate::{faults, smpboot, umode, Console};
/// Ô vector **IRQ** khi đang ở EL1 dùng `SP_EL0` (`0x080`) — KT-259.
const IRQ_SLOT_EL1T: u64 = 1;
/// Ô vector **IRQ** khi đang ở EL1 dùng `SP_ELx` (`0x280`) — ô mà kernel này nhận (EL1h).
const IRQ_SLOT_EL1H: u64 = 5;

// ---------------------------------------------------------------------------------------------
// Trạng thái trap đã bắt (in bằng chứng sau khi resume)
// ---------------------------------------------------------------------------------------------

/// Số lần `brk #0` đã bắt (phải đúng **1**).
pub(crate) static BRK_COUNT: AtomicU64 = AtomicU64::new(0);
/// Ô vector đã dùng (phải là `4` = `SyncCurrentElSpX`).
pub(crate) static BRK_SLOT: AtomicU64 = AtomicU64::new(u64::MAX);
/// `ESR_EL1` thô của lần `brk` (phải có `EC = 0x3C`).
pub(crate) static BRK_ESR: AtomicU64 = AtomicU64::new(0);
/// `ELR_EL1` của lần `brk` (phải bằng địa chỉ instruction `brk #0`).
pub(crate) static BRK_ELR: AtomicU64 = AtomicU64::new(0);
/// `FAR_EL1` của lần `brk` (BRK không đặt FAR ⇒ 0 hoặc giá trị cũ; chỉ in, không khẳng định).
pub(crate) static BRK_FAR: AtomicU64 = AtomicU64::new(0);
/// `SPSR_EL1` của lần `brk` (phải là `EL1h` = `M[3:0] = 0b0101`).
pub(crate) static BRK_SPSR: AtomicU64 = AtomicU64::new(0);
/// Số trap **không** phải `BRK` (phải 0 — mọi trap khác là lỗi thật, in rồi dừng).
pub(crate) static OTHER_TRAPS: AtomicU64 = AtomicU64::new(0);
// ---------------------------------------------------------------------------------------------
// Handler đồng bộ (Rust) — được `vec_common` gọi
// ---------------------------------------------------------------------------------------------

/// Handler của **mọi** ô vector (16 ô dùng chung một stub).
///
/// Trả `1` ⇒ `vec_common` khôi phục thanh ghi và `eret` (đường resume); trả `0` ⇒ dừng máy.
/// Hàm **không** trả về khi trap không phải `BRK`: nó in nguyên nhân rồi `debug_halt(2)` — một
/// fault thật ở bước 3 là lỗi, không được "resume rồi hy vọng".
#[no_mangle]
pub extern "C" fn vec_handler(
    slot: u64,
    esr_raw: u64,
    far: u64,
    elr: u64,
    spsr: u64,
    frame: u64,
) -> u64 {
    let esr = Esr::from_bits(esr_raw);
    let ec = esr.ec_raw();

    // ---- KT-259: **IRQ** (ô 1 = EL1t, ô 5 = EL1h). `ESR_EL1` của IRQ là 0 (không có syndrome),
    // nên phân loại bằng **ô vector**, không bằng `ESR`. Handler đọc `ICC_IAR1_EL1` (nhận ngắt),
    // rồi phân phối theo **INTID**:
    //
    //   - `SGI 1` (`SGI_SHOOTDOWN`, KT-262): `shootdown::handle_ipi` chạy `tlbi vae1is` + `dsb ish`
    //     + `isb` **trong handler** rồi ack — đây là toàn bộ phía hart phụ của B3.6;
    //   - `SGI 0` (`SGI_IPI`, KT-259): `note_ipi` in + đếm (chỉ SGI 0, nên dòng `# smp: ipi recv=`
    //     không bị lẫn với shootdown/lệnh);
    //   - `SGI 2` (`SGI_COMMAND`): không làm gì trong handler — lệnh nằm ở `SMP_CMD` và hart phụ
    //     xử lý ở vòng lệnh của nó (`wfe` đã được đánh thức bởi `sev`).
    //
    // **Bắt buộc** ghi `ICC_EOIR1_EL1` — không ghi thì ngắt không bao giờ tới lại và phép đo sẽ
    // treo thay vì đỏ.
    if slot == IRQ_SLOT_EL1T || slot == IRQ_SLOT_EL1H {
        let intid = gic::ack_irq();
        if !gic::is_spurious(intid) {
            // **KT-338c** (D2-244 tầng 1): drain **hộp thư IPI bitmask dùng chung** của hart này trên
            // **mọi** SGI. Kind nằm trong bitmask (`IpiMailbox`, coalesce không bao giờ đầy),
            // `INTID` chỉ là chuông — nên `end_irq` không làm mất kind. Bitmask rỗng trên một SGI
            // thật là **mất đánh thức** ⇒ in chẩn đoán để runner/soak thấy, không im lặng.
            let kinds = vifast_plat_aarch64::ipi::drain_current();
            if kinds.is_empty() {
                let mut console = Console;
                let _ = writeln!(console, "# ipi: mailbox empty intid={intid}");
            }
            if !shootdown::handle_ipi(intid) {
                smpboot::note_ipi(intid);
            }
            gic::end_irq(intid);
        }
        return 1;
    }

    // ---- **KT-269** (B3.7): U-mode. `svc` từ EL0 (ô 8), fault từ EL0 (đột biến (ii): trang
    // không `user=true`) và `svc` từ EL1 (phòng thủ — nếu trang EL0 cho EL1 thực thi; đột biến
    // (i) thực tế dừng **sớm hơn** ở instruction abort `EC=0x21` vì `USER_RX` đặt `PXN=1`) đều
    // do `umode::handle` quyết định — nó in bằng chứng rồi resume hoặc dừng, **không** rơi vào
    // đường trap chung.
    if let Some(result) = umode::handle(slot, esr_raw, far, elr, spsr, frame) {
        return result;
    }

    if esr.ec() == Some(ExceptionClass::Brk64) {
        BRK_COUNT.fetch_add(1, Ordering::Relaxed);
        BRK_SLOT.store(slot, Ordering::Relaxed);
        BRK_ESR.store(esr_raw, Ordering::Relaxed);
        BRK_ELR.store(elr, Ordering::Relaxed);
        BRK_FAR.store(far, Ordering::Relaxed);
        BRK_SPSR.store(spsr, Ordering::Relaxed);
        // `BRK`: ELR_EL1 = **địa chỉ instruction BRK** (DDI 0487 §D17.2.37) ⇒ resume ở ELR + 4.
        asm::write_elr_el1(elr + 4);
        return 1;
    }
    // ---- KT-254 + KT-262: fault **được chờ** của phép đo VSpace (đọc VA sau `unmap`).
    //
    // Phân loại bằng `ESR_EL1`/`FAR_EL1` thật (không suy diễn từ chỗ khác), dựng
    // `FaultKind::Vm` của ABI (`vifast-plat-common::fault`, D2-56a), ghi vào **ô của hart đang
    // chạy** (`faults::record`) rồi **resume** (`ELR += 4` — instruction `ldr` 4 byte, không thực
    // thi nên resume an toàn). Chỉ làm vậy khi hart ấy đã **đăng ký** fault được chờ
    // (`faults::take_expect`); mọi fault khác vẫn **fatal** như trước — che lỗi là điều tệ nhất.
    if let Some(abort) = vifast_plat_aarch64::fault::classify_esr(esr_raw, far) {
        let hart = smp::hart_index(asm::read_mpidr_el1()).unwrap_or(usize::MAX);
        if faults::take_expect(hart) {
            let access = FaultAccess::from_code(abort.access.code());
            let reason = vm_reason(access, false, None);
            let kind = FaultKind::Vm {
                vspace: VSPACE_ID as u64,
                va: abort.va,
                access,
                reason,
            };
            if let Some((_vspace, va, access, reason)) = kind.vm() {
                faults::record(hart, va, access.code(), reason.code(), esr_raw, far, elr);
            }
            asm::write_elr_el1(elr + 4);
            return 1;
        }
        // ---- **KT-277**: fault **thô** được chờ của phép đo W^X (ghi vào `.text` ⇒ data abort
        // quyền). Không dựng `FaultKind::Vm`: trang **có** map, chỉ read-only — ghi
        // `reason=NotMapped` sẽ là lời nói dối trong log. Ghi nguyên văn `ESR`/`FAR`/`ELR` rồi
        // resume (`ELR += 4` — instruction `str` 8 byte, không thực thi nên resume an toàn).
        if faults::take_expect_raw(hart) {
            faults::record_raw(hart, esr_raw, far, elr);
            asm::write_elr_el1(elr + 4);
            return 1;
        }
    }
    // Trap thật: in đủ nguyên nhân rồi dừng. Không resume — che lỗi là điều tệ nhất có thể làm.
    OTHER_TRAPS.fetch_add(1, Ordering::Relaxed);
    let mut console = Console;
    let _ = writeln!(
        console,
        "# trap: slot={slot} off=0x{:03x} ec=0x{ec:02x} il={} far=0x{far:x} elr=0x{elr:x} spsr=0x{spsr:x}",
        slot * vector::VECTOR_STRIDE,
        esr.il() as u8,
    );
    debug_halt(2)
}
