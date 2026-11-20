// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho phân loại fault aarch64 — KT-254 (K4' bước 3, B3.5).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test fault
//! ```
//!
//! # Vì sao phân loại ở crate arch
//!
//! `FaultKind` ABI nằm ở `vifast-plat-common::fault` (D2-56a) — crate đó phụ thuộc `vifast-core`
//! và gắn **Sv39** (`PteFacts` đọc bit PTE riscv). Crate arch **không** kéo `vifast-core` vào ảnh
//! boot; vì vậy phần **giải mã ESR/FAR** (arch-specific) nằm ở đây, còn việc dựng
//! `FaultKind::Vm` từ kết quả giải mã là một dòng ở ảnh (`vifast-kbin-aarch64`). Nợ M17 ghi ở
//! `v2/agent/JOURNAL/kt254-vspace.md` §5.
//!
//! Đây là ca kiểm **đỏ trước** của KT-254 (log: `v2/agent/JOURNAL/kt254-red.log`).

use vifast_plat_aarch64::fault::{
    classify_esr, Access, EC_DATA_ABORT_LOWER_EL, EC_DATA_ABORT_SAME_EL,
    EC_INSTRUCTION_ABORT_LOWER_EL, EC_INSTRUCTION_ABORT_SAME_EL, ISS_WNR,
};
use vifast_plat_aarch64::sysreg::{Esr, ExceptionClass};

/// ESR thô từ `EC` + `ISS` (IL = 1 — instruction 32 bit).
fn esr(ec: ExceptionClass, iss: u32) -> u64 {
    Esr::new(ec, true, iss).bits()
}

/// **Data abort đọc** (EC = 0x25, `WnR = 0`) ⇒ `Access::Read`, `va = FAR`.
#[test]
fn data_abort_read() {
    let far = 0x6000_0000u64;
    let abort = classify_esr(esr(ExceptionClass::DataAbortSameEl, 0), far).expect("data abort");
    assert_eq!(abort.access, Access::Read);
    assert_eq!(abort.va, far);
    assert_eq!(abort.ec, EC_DATA_ABORT_SAME_EL);
}

/// **Data abort ghi** (EC = 0x25, `WnR = 1`) ⇒ `Access::Write`.
#[test]
fn data_abort_write() {
    let far = 0x6000_1000u64;
    let abort = classify_esr(esr(ExceptionClass::DataAbortSameEl, ISS_WNR as u32), far)
        .expect("data abort");
    assert_eq!(abort.access, Access::Write);
    assert_eq!(abort.va, far);
}

/// **Instruction abort** (EC = 0x21) ⇒ `Access::Execute`; `WnR` không có nghĩa với instruction
/// abort nên bit 6 **không** được đọc thành "write".
#[test]
fn instruction_abort_execute() {
    let far = 0xFFFF_FFC0_8000_0000u64;
    let abort = classify_esr(
        esr(ExceptionClass::InstructionAbortSameEl, ISS_WNR as u32),
        far,
    )
    .expect("instruction abort");
    assert_eq!(abort.access, Access::Execute);
    assert_eq!(abort.va, far);
    assert_eq!(abort.ec, EC_INSTRUCTION_ABORT_SAME_EL);
}

/// Abort **từ EL0** (EC = 0x24/0x20) cũng phân loại được — cùng luật, khác nguồn.
#[test]
fn lower_el_aborts_are_classified() {
    let read = classify_esr(esr(ExceptionClass::DataAbortLowerEl, 0), 0x1000).expect("read");
    assert_eq!(read.access, Access::Read);
    assert_eq!(read.ec, EC_DATA_ABORT_LOWER_EL);
    let exec = classify_esr(esr(ExceptionClass::InstructionAbortLowerEl, 0), 0x2000).expect("exec");
    assert_eq!(exec.access, Access::Execute);
    assert_eq!(exec.ec, EC_INSTRUCTION_ABORT_LOWER_EL);
}

/// Exception **không phải** abort ⇒ `None` (caller đi đường khác, không bịa `Vm`).
#[test]
fn non_abort_is_none() {
    for ec in [
        ExceptionClass::Brk64,
        ExceptionClass::Unknown,
        ExceptionClass::SpAlignment,
        ExceptionClass::PcAlignment,
        ExceptionClass::Svc64,
        ExceptionClass::BreakpointSameEl,
    ] {
        assert_eq!(
            classify_esr(esr(ec, 0), 0x1234),
            None,
            "EC 0x{:02x}",
            ec.bits()
        );
    }
}

/// Mã `Access` khớp `FaultAccess` của ABI (`Read`=0, `Write`=1, `Execute`=2) — để ảnh dựng
/// `FaultKind::Vm` bằng `FaultAccess::from_code` mà không cần bảng ánh xạ thứ hai.
#[test]
fn access_codes_match_abi() {
    assert_eq!(Access::Read.code(), 0);
    assert_eq!(Access::Write.code(), 1);
    assert_eq!(Access::Execute.code(), 2);
    assert_eq!(Access::Read.name(), "Read");
    assert_eq!(Access::Write.name(), "Write");
    assert_eq!(Access::Execute.name(), "Execute");
}

/// `EC` được bóc từ bit [31:26] — không phụ thuộc `IL`/`ISS`; `ISS` lạ không làm panic.
#[test]
fn ec_extraction_ignores_iss() {
    let raw = esr(ExceptionClass::DataAbortSameEl, 0x1FF_FFFF);
    let abort = classify_esr(raw, 0xDEAD_0000).expect("data abort");
    assert_eq!(abort.ec, EC_DATA_ABORT_SAME_EL);
    assert_eq!(abort.va, 0xDEAD_0000);
    // `WnR` = bit 6 của ISS: 0x1FF_FFFF có bit 6 = 1 ⇒ Write.
    assert_eq!(abort.access, Access::Write);
}
