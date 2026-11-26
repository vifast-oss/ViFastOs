// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-418c** — khoá **mã hoá** lệnh `msr pan, #imm` (hardening `PAN` của `KT-418`) bằng test
//! host: bản KT-418 dùng `op1 = 0b011` (`0xD503417F`) — **không** phải trường PSTATE nào ⇒ trên
//! CPU **có** `PAN` (`-cpu max`) lệnh trap `ec=0x00` và boot chết trước TAP 50 (đo được, xem
//! `v2/agent/JOURNAL/kt418c-hardening-max.md` §2). Test này giải mã lại từng trường của
//! [`MSR_PAN_ON`]/[`MSR_PAN_OFF`] theo công thức `MSR (immediate)` của ARMv8.1 nên một lần nữa
//! sửa sai `op1`/`op2`/`CRm` là **ĐỎ** ở host, không phải chờ QEMU.
//!
//! Đối chiếu ngoài (ghi trong journal): `clang --target=aarch64-none-elf -march=armv8.2-a -c`
//! với `msr pan, #1` / `msr pan, #0` sinh đúng `0xD500419F` / `0xD500409F`.

use vifast_plat_aarch64::asm::{MSR_PAN_OFF, MSR_PAN_ON};

/// Giải mã một `MSR (immediate)` của PSTATE: trả `(op1, CRn, CRm, op2)`; `None` nếu không khớp
/// khuôn `1101 0101 0000 0 op1 0100 CRm op2 11111`.
fn decode_msr_imm(word: u32) -> Option<(u32, u32, u32, u32)> {
    let op0 = (word >> 19) & 0x1fff; // bits 31..19 = 0b1101_0101_0000_0
    let op1 = (word >> 16) & 0x7;
    let crn = (word >> 12) & 0xf;
    let crm = (word >> 8) & 0xf;
    let op2 = (word >> 5) & 0x7;
    let low5 = word & 0x1f;
    if op0 != 0b1_1010_1010_0000 || low5 != 0b1_1111 {
        return None;
    }
    Some((op1, crn, crm, op2))
}

#[test]
fn msr_pan_on_is_pstate_field_op1_0_op2_4() {
    let (op1, crn, crm, op2) = decode_msr_imm(MSR_PAN_ON).expect("khuôn MSR (immediate)");
    assert_eq!(op1, 0b000, "PAN là op1 0b000 (bản KT-418 sai: 0b011)");
    assert_eq!(crn, 0b0100, "CRn = 0100 (trường PSTATE)");
    assert_eq!(crm, 0b0001, "CRm = giá trị 1");
    assert_eq!(op2, 0b100, "PAN là op2 0b100");
}

#[test]
fn msr_pan_off_differs_only_in_value() {
    let (op1, crn, crm, op2) = decode_msr_imm(MSR_PAN_OFF).expect("khuôn MSR (immediate)");
    assert_eq!((op1, crn, op2), (0b000, 0b0100, 0b100));
    assert_eq!(crm, 0b0000, "CRm = giá trị 0");
    assert_eq!(
        MSR_PAN_ON ^ MSR_PAN_OFF,
        0x0000_0100,
        "hai lệnh chỉ được khác nhau ở CRm (bit 8)"
    );
}

#[test]
fn msr_immediate_known_good_encoding_is_rejected_for_pan() {
    // Đối chứng: `msr daifset, #2` = 0xD50342DF là một `MSR (immediate)` **hợp lệ** (op1 = 0b011)
    // — chính vì op1 = 0b011 là **DAIF**, không phải PAN, nên mã hoá cũ 0xD503417F bị CPU coi là
    // instruction không xác định.
    let (op1, crn, crm, op2) = decode_msr_imm(0xD503_42DF).expect("khuôn MSR (immediate)");
    assert_eq!((op1, crn, crm, op2), (0b011, 0b0100, 0b0010, 0b110));
    assert_ne!(
        MSR_PAN_ON, 0xD503_417F,
        "mã hoá sai của KT-418 phải biến mất"
    );
}
