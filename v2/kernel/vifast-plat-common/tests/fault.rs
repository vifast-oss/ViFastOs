// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho `FaultKind` ABI (KT-56, D2-56a) — `vifast-plat-common::fault`.
//!
//! Kiểm **cùng** hàm mà trap handler QEMU gọi: `scause` 12/13/15 → `Vm` đúng access + `va=stval`;
//! `reason` từ PTE (NotMapped/Permission/Wx) theo luật K2; trap khác → `UserException`; `ecall`
//! không phải fault. Cũng kiểm `Display` (một định dạng cho log/TAP) và `FaultStats`.
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test fault
//! ```

use vifast_plat_common::fault::{
    classify_trap, vm_reason, FaultAccess, FaultKind, FaultStats, PteFacts, TrapFacts,
    VmFaultReason,
};
use vifast_pte::sv39::{PTE_EXEC, PTE_READ, PTE_USER, PTE_VALID, PTE_WRITE};

fn trap(scause: u64, stval: u64, from_user: bool) -> TrapFacts {
    TrapFacts {
        scause,
        stval,
        sepc: 0x4000_1000,
        from_user,
    }
}

fn leaf(read: bool, write: bool, exec: bool, user: bool) -> Option<PteFacts> {
    let mut raw = PTE_VALID;
    raw |= u64::from(read) * PTE_READ;
    raw |= u64::from(write) * PTE_WRITE;
    raw |= u64::from(exec) * PTE_EXEC;
    raw |= u64::from(user) * PTE_USER;
    Some(PteFacts::from_raw(raw))
}

#[test]
fn scause_page_fault_ba_loai_dung_access_va_stval() {
    for (scause, access) in [
        (12u64, FaultAccess::Execute),
        (13, FaultAccess::Read),
        (15, FaultAccess::Write),
    ] {
        let kind = classify_trap(&trap(scause, 0x2000_0040, true), 7, None).expect("phải là fault");
        assert_eq!(
            kind,
            FaultKind::Vm {
                vspace: 7,
                va: 0x2000_0040,
                access,
                reason: VmFaultReason::NotMapped,
            },
            "scause {scause}"
        );
        assert_eq!(
            kind.vm(),
            Some((7, 0x2000_0040, access, VmFaultReason::NotMapped))
        );
    }
}

#[test]
fn reason_tu_pte_khop_luat_k2() {
    // Chưa map (entry 0) ⇒ NotMapped, dù scause nói gì.
    assert_eq!(
        vm_reason(FaultAccess::Read, true, None),
        VmFaultReason::NotMapped
    );
    assert_eq!(
        vm_reason(FaultAccess::Write, false, Some(PteFacts::from_raw(0))),
        VmFaultReason::NotMapped
    );
    // Ghi trang RX ⇒ Wx (D2-67 chặn map W+X nên chỉ còn fault lúc dùng).
    assert_eq!(
        vm_reason(FaultAccess::Write, false, leaf(true, false, true, false)),
        VmFaultReason::Wx
    );
    // Thực thi trang RW ⇒ Wx.
    assert_eq!(
        vm_reason(FaultAccess::Execute, false, leaf(true, true, false, false)),
        VmFaultReason::Wx
    );
    // Ghi trang chỉ đọc ⇒ Permission.
    assert_eq!(
        vm_reason(FaultAccess::Write, false, leaf(true, false, false, false)),
        VmFaultReason::Permission
    );
    // Thực thi trang chỉ đọc (NX) ⇒ Permission.
    assert_eq!(
        vm_reason(FaultAccess::Execute, false, leaf(true, false, false, false)),
        VmFaultReason::Permission
    );
}

#[test]
fn quyen_privilege_thang_wx() {
    // U-mode chạm leaf kernel RX ⇒ Permission (không phải Wx dù thiếu W).
    assert_eq!(
        vm_reason(FaultAccess::Write, true, leaf(true, false, true, false)),
        VmFaultReason::Permission
    );
    // S-mode chạm leaf U=1 (SUM tắt trên đường fault) ⇒ Permission.
    assert_eq!(
        vm_reason(FaultAccess::Read, false, leaf(true, true, false, true)),
        VmFaultReason::Permission
    );
    // Cùng leaf đó, U-mode đọc ⇒ hợp lệ (không fault) — luật vẫn trả None? Hàm reason chỉ được
    // gọi khi phần cứng đã fault nên không cần "None"; kiểm nhánh user khớp để chắc không lật.
    assert_eq!(
        vm_reason(FaultAccess::Execute, true, leaf(true, true, false, true)),
        VmFaultReason::Wx
    );
}

#[test]
fn trap_khac_user_exception_va_ecall_khong_phai_fault() {
    // illegal (2) — K2 dùng `UserException{ip, kind}`; K3 giữ đúng biến thể đó.
    let illegal = classify_trap(&trap(2, 0xdead, true), 0, None).expect("illegal là fault");
    assert_eq!(
        illegal,
        FaultKind::UserException {
            ip: 0x4000_1000,
            kind: 2
        }
    );
    // breakpoint (3) và mã lạ.
    for code in [0u64, 1, 3, 4, 5, 6, 7, 9, 42] {
        let kind = classify_trap(&trap(code, 0, false), 0, None).expect("mã khác là fault");
        assert_eq!(
            kind,
            FaultKind::UserException {
                ip: 0x4000_1000,
                kind: code
            }
        );
    }
    // ecall từ U-mode (8) — syscall, không phải fault.
    assert_eq!(classify_trap(&trap(8, 0, true), 0, None), None);
}

#[test]
fn render_on_dinh_cho_log_va_tap() {
    assert_eq!(FaultKind::Exit { code: 42 }.to_string(), "Exit{42}");
    assert_eq!(
        FaultKind::Vm {
            vspace: 1,
            va: 0x200040,
            access: FaultAccess::Write,
            reason: VmFaultReason::NotMapped,
        }
        .to_string(),
        "Vm{vspace=1, va=0x200040, access=Write, reason=NotMapped}"
    );
    assert_eq!(
        FaultKind::UserException {
            ip: 0x1000,
            kind: 2
        }
        .to_string(),
        "UserException{ip=0x1000, kind=2}"
    );
    assert_eq!(
        FaultKind::UnknownSyscall { nr: 99 }.to_string(),
        "UnknownSyscall{nr=99}"
    );
    assert!(FaultKind::Exit { code: 1 }.is_exit());
    assert_eq!(FaultKind::Exit { code: 9 }.exit_code(), Some(9));
    assert_eq!(FaultKind::UnknownSyscall { nr: 1 }.exit_code(), None);
}

#[test]
fn fault_stats_ghi_va_doc_lai_tung_bien_the() {
    let s = FaultStats::new();
    assert_eq!(s.last(), None);
    assert_eq!(s.count(), 0);
    s.record(FaultKind::Exit { code: 7 });
    assert_eq!(s.last(), Some(FaultKind::Exit { code: 7 }));
    s.record(FaultKind::Vm {
        vspace: 3,
        va: 0x4000,
        access: FaultAccess::Execute,
        reason: VmFaultReason::Wx,
    });
    assert_eq!(
        s.last(),
        Some(FaultKind::Vm {
            vspace: 3,
            va: 0x4000,
            access: FaultAccess::Execute,
            reason: VmFaultReason::Wx,
        })
    );
    s.record(FaultKind::UnknownSyscall { nr: 77 });
    assert_eq!(s.last(), Some(FaultKind::UnknownSyscall { nr: 77 }));
    s.record(FaultKind::UserException { ip: 0x55, kind: 2 });
    assert_eq!(
        s.last(),
        Some(FaultKind::UserException { ip: 0x55, kind: 2 })
    );
    assert_eq!(s.count(), 4);
}
