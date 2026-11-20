// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Giải mã fault aarch64** từ `ESR_EL1`/`FAR_EL1` — KT-254 (K4' bước 3, B3.5).
//!
//! # Vì sao chỉ **giải mã** ở đây, không dựng `FaultKind`
//!
//! `FaultKind` ABI (`Exit`/`Vm`/`UnknownSyscall`/`UserException`) nằm ở
//! `vifast-plat-common::fault` (D2-56a) — crate đó phụ thuộc `vifast-core`/`vifast-alloc` và
//! `PteFacts` của nó đọc **bit PTE riscv** (nợ M17). Crate arch **không** kéo `vifast-core` vào
//! ảnh boot, nên phần **arch-specific** (bóc `EC`/`WnR`/`FAR`) nằm ở đây; ảnh dựng
//! `FaultKind::Vm` từ [`Abort`] bằng `FaultAccess::from_code` (mã khớp ABI — test
//! `access_codes_match_abi`).
//!
//! # Nguồn số (DDI 0487)
//!
//! - `EC[31:26]`: `0x20` instruction abort từ EL0, `0x21` từ EL1, `0x24` data abort từ EL0,
//!   `0x25` từ EL1 (bảng D17-1);
//! - `ISS[6]` = `WnR` — **chỉ** có nghĩa với data abort (`1` = ghi);
//! - `FAR_EL1` = địa chỉ gây fault (VA, không làm tròn).
//!
//! Tệp này **không** asm, **không** `unsafe`, **không** `cfg(target_arch)` — chạy được trên host.

use crate::sysreg::{Esr, ExceptionClass};

/// `EC` — instruction abort từ EL0.
pub const EC_INSTRUCTION_ABORT_LOWER_EL: u8 = 0x20;
/// `EC` — instruction abort từ EL1.
pub const EC_INSTRUCTION_ABORT_SAME_EL: u8 = 0x21;
/// `EC` — data abort từ EL0.
pub const EC_DATA_ABORT_LOWER_EL: u8 = 0x24;
/// `EC` — data abort từ EL1.
pub const EC_DATA_ABORT_SAME_EL: u8 = 0x25;

/// `ISS[6]` — `WnR` (write-not-read) của **data abort**.
pub const ISS_WNR: u64 = 1 << 6;

/// Quyền truy cập gây fault — mã **khớp** `FaultAccess` của ABI (`Read`=0, `Write`=1,
/// `Execute`=2) để ảnh dùng `FaultAccess::from_code` mà không cần bảng ánh xạ thứ hai.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Đọc (data abort, `WnR = 0`).
    Read,
    /// Ghi (data abort, `WnR = 1`).
    Write,
    /// Thực thi (instruction abort).
    Execute,
}

impl Access {
    /// Mã ổn định — **trùng** `vifast_plat_common::fault::FaultAccess::code`.
    pub const fn code(self) -> u64 {
        match self {
            Access::Read => 0,
            Access::Write => 1,
            Access::Execute => 2,
        }
    }

    /// Tên ổn định cho log/TAP (trùng ABI).
    pub const fn name(self) -> &'static str {
        match self {
            Access::Read => "Read",
            Access::Write => "Write",
            Access::Execute => "Execute",
        }
    }
}

/// Một abort đã giải mã: quyền truy cập, địa chỉ (`FAR_EL1`), và `EC` gốc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abort {
    /// Quyền truy cập gây fault.
    pub access: Access,
    /// `FAR_EL1` — địa chỉ yêu cầu (không làm tròn).
    pub va: u64,
    /// `EC[5:0]` gốc — giữ để log/đối chiếu.
    pub ec: u8,
}

/// Giải mã `ESR_EL1` + `FAR_EL1` thành [`Abort`]; `None` nếu exception **không phải** abort.
///
/// Hàm **thuần**, không panic với mọi `u64`: `EC` lạ (BRK, SVC, alignment, breakpoint…) trả
/// `None` để caller đi đường khác — **không** bịa `Vm`.
pub fn classify_esr(esr: u64, far: u64) -> Option<Abort> {
    let esr = Esr::from_bits(esr);
    let ec = esr.ec_raw();
    match esr.ec() {
        Some(ExceptionClass::DataAbortLowerEl) | Some(ExceptionClass::DataAbortSameEl) => {
            // `WnR` chỉ có nghĩa với data abort (DDI 0487 §D17.2.32).
            let write = esr.bits() & ISS_WNR != 0;
            Some(Abort {
                access: if write { Access::Write } else { Access::Read },
                va: far,
                ec,
            })
        }
        Some(ExceptionClass::InstructionAbortLowerEl)
        | Some(ExceptionClass::InstructionAbortSameEl) => Some(Abort {
            access: Access::Execute,
            va: far,
            ec,
        }),
        _ => None,
    }
}
