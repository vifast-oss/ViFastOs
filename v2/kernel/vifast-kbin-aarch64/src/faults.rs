// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Bản ghi fault `FaultKind::Vm` theo hart** — KT-262 (K4' bước 3, B3.6).
//!
//! # Vì sao phải theo hart (không phải một ô toàn cục)
//!
//! KT-254 ghi fault vào **một** bộ biến (`FAULT_KIND_VM`, `FAULT_VA`, …) vì chỉ hart 0 sinh
//! fault. KT-262 đo fault trên **hart 1** (đọc VA sau khi hart 0 `unmap` + shootdown) ⇒ nếu vẫn
//! dùng một ô, fault của hart 1 ghi đè lên fault của hart 0 và phép đo TAP 9/TAP 13 không còn
//! phân biệt được **hart nào** đã fault. Mọi ô ở đây vì thế là **mảng theo hart id**.
//!
//! # Cơ chế "fault được chờ"
//!
//! Handler đồng bộ **không** được nuốt fault thật: nó chỉ resume khi hart ấy đã **đăng ký** một
//! fault được chờ ([`expect`]). Cờ là mặt nạ bit `AtomicU32` (một bit một hart) và được **xoá**
//! khi fault tới ([`take_expect`]) — nên một fault thứ hai trên cùng hart vẫn là fault thật
//! (in `# trap:` rồi dừng), không bị "hy vọng" che mất.
//!
//! # Không in gì trong handler
//!
//! Hart phụ **không** in từ handler: PL011 không có lock, in từ hai hart là xen ký tự (lỗi thật
//! của KT-259). Handler chỉ ghi số vào mảng; hart 0 đọc lại và in **một** dòng `# tlbi:`.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_plat_aarch64::smp::MAX_HARTS;

/// Một fault `FaultKind::Vm` đã bắt trên một hart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VmFault {
    /// `FaultKind::Vm.va` — VA gây fault (từ `FAR_EL1`).
    pub va: u64,
    /// `FaultKind::Vm.access` — mã ABI (`Read`=0, `Write`=1, `Execute`=2).
    pub access: u64,
    /// `FaultKind::Vm.reason` — mã ABI (`NotMapped`=0, …).
    pub reason: u64,
    /// `ESR_EL1` thô.
    pub esr: u64,
    /// `FAR_EL1` thô.
    pub far: u64,
    /// `ELR_EL1` — instruction gây fault (handler resume ở `ELR + 4`).
    pub elr: u64,
}

/// Mặt nạ hart **đang chờ** một fault `Vm` (bit `i` = hart `i`).
static EXPECT: AtomicU32 = AtomicU32::new(0);
/// Mặt nạ hart **đã** bắt được fault `Vm` (bit `i` = hart `i`).
static SEEN: AtomicU32 = AtomicU32::new(0);

/// `FaultKind::Vm.va` theo hart.
static VA: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `FaultKind::Vm.access` theo hart (`u64::MAX` = chưa có).
static ACCESS: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
/// `FaultKind::Vm.reason` theo hart (`u64::MAX` = chưa có).
static REASON: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(u64::MAX) }; MAX_HARTS];
/// `ESR_EL1` theo hart.
static ESR: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `FAR_EL1` theo hart.
static FAR: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];
/// `ELR_EL1` theo hart.
static ELR: [AtomicU64; MAX_HARTS] = [const { AtomicU64::new(0) }; MAX_HARTS];

/// Đăng ký **một** fault `Vm` được chờ trên `hart` — handler sẽ resume thay vì dừng máy.
///
/// Hart ngoài `MAX_HARTS` là lỗi lập trình: bỏ qua (không bit nào để bật) thay vì panic trong
/// handler — panic trong handler là treo không log.
pub fn expect(hart: usize) {
    if hart < MAX_HARTS {
        EXPECT.fetch_or(1u32 << hart, Ordering::Release);
    }
}

/// Lấy (và xoá) cờ "đang chờ fault" của `hart`; `true` ⇔ fault kế tiếp của hart ấy được chờ.
pub fn take_expect(hart: usize) -> bool {
    if hart >= MAX_HARTS {
        return false;
    }
    let bit = 1u32 << hart;
    EXPECT.fetch_and(!bit, Ordering::AcqRel) & bit != 0
}

/// Xoá bản ghi của `hart` (dùng trước một phép đo để không đọc nhầm fault cũ).
pub fn reset(hart: usize) {
    if hart >= MAX_HARTS {
        return;
    }
    SEEN.fetch_and(!(1u32 << hart), Ordering::AcqRel);
    VA[hart].store(0, Ordering::Release);
    ACCESS[hart].store(u64::MAX, Ordering::Release);
    REASON[hart].store(u64::MAX, Ordering::Release);
    ESR[hart].store(0, Ordering::Release);
    FAR[hart].store(0, Ordering::Release);
    ELR[hart].store(0, Ordering::Release);
}

/// Ghi một fault `Vm` đã bắt trên `hart` rồi bật bit trong [`SEEN`] (bit bật **sau** cùng).
pub fn record(hart: usize, va: u64, access: u64, reason: u64, esr: u64, far: u64, elr: u64) {
    if hart >= MAX_HARTS {
        return;
    }
    VA[hart].store(va, Ordering::Release);
    ACCESS[hart].store(access, Ordering::Release);
    REASON[hart].store(reason, Ordering::Release);
    ESR[hart].store(esr, Ordering::Release);
    FAR[hart].store(far, Ordering::Release);
    ELR[hart].store(elr, Ordering::Release);
    SEEN.fetch_or(1u32 << hart, Ordering::Release);
}

/// Bản ghi fault của `hart`; `None` nếu hart ấy chưa fault `Vm` lần nào.
pub fn get(hart: usize) -> Option<VmFault> {
    if hart >= MAX_HARTS || SEEN.load(Ordering::Acquire) & (1u32 << hart) == 0 {
        return None;
    }
    Some(VmFault {
        va: VA[hart].load(Ordering::Acquire),
        access: ACCESS[hart].load(Ordering::Acquire),
        reason: REASON[hart].load(Ordering::Acquire),
        esr: ESR[hart].load(Ordering::Acquire),
        far: FAR[hart].load(Ordering::Acquire),
        elr: ELR[hart].load(Ordering::Acquire),
    })
}

/// Mặt nạ hart đã bắt được fault `Vm` — bằng chứng "hart nào" cho log/TAP.
pub fn seen_mask() -> u32 {
    SEEN.load(Ordering::Acquire)
}
