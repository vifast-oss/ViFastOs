// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vifast-abi` — hằng số ABI sinh từ `spec/abi.toml` (KT-05).
//!
//! `generated.rs` do `v2/bin/vifast-gen gen` sinh; **không sửa tay** — `gen-check` trong
//! `./v2/bin/v2 check` so byte và bắt mọi sửa tay.

mod generated;

pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_syscall_va_error_dung_spec() {
        assert_eq!(SYSCALL_COUNT, 16);
        assert_eq!(ERROR_COUNT, 19);
        assert_eq!(SYSCALLS[0].0, "Call");
        assert_eq!(SYSCALLS[15].0, "DebugGetInfo");
        assert_eq!(SYSCALLS[15].1, 15);
        assert_eq!(ERRORS[18].1, 19);
    }

    #[test]
    fn chu_ky_method_du_13_va_co_read_state() {
        assert_eq!(METHOD_SIGNATURES.len(), 13);
        let rs = METHOD_SIGNATURES
            .iter()
            .find(|(n, _, _)| *n == "Tcb.ReadState")
            .expect("phải có Tcb.ReadState");
        assert_eq!(rs.2.len(), 4, "ReadState trả 4 giá trị (D2-33)");
        let retype = METHOD_SIGNATURES
            .iter()
            .find(|(n, _, _)| *n == "Untyped.Retype")
            .expect("phải có Untyped.Retype");
        assert!(retype.1.contains(&"type"));
    }

    #[test]
    fn object_loi_va_reserved_tach_bach() {
        assert!(CORE_OBJECTS
            .iter()
            .any(|(n, id)| *n == "SchedControl" && *id == 8));
        assert!(RESERVED_OBJECTS
            .iter()
            .any(|(n, id)| *n == "VSpace" && *id == 9));
        assert_eq!(TCB_STATES.len(), 11);
    }
}
