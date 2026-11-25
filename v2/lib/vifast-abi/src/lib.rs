// SPDX-License-Identifier: MIT OR Apache-2.0
// **KT-404** (`D2-259`) — crate này **0 `unsafe`** ⇒ khoá bằng máy: mọi `unsafe`
// thêm vào (kể cả trong macro) là lỗi biên dịch, không chỉ là luật cấu trúc R5.
#![forbid(unsafe_code)]
//! `vifast-abi` — hằng số ABI sinh từ `spec/abi.toml` (KT-05).
//!
//! `generated.rs` do `v2/bin/vifast-gen gen` sinh; **không sửa tay** — `gen-check` trong
//! `./v2/bin/v2 check` so byte và bắt mọi sửa tay.
//!
//! `no_std`: từ KT-58 kernel `no_std` (`vifast-core`) dùng chung hằng số sinh từ spec;
//! test vẫn chạy trên host (std).
#![cfg_attr(not(test), no_std)]

mod generated;

pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_syscall_va_error_dung_spec() {
        assert_eq!(SYSCALL_COUNT, 16);
        // KT-62: `min_compat` sinh từ spec (DebugGetInfo trả nó) — không hardcode trong core.
        assert_eq!(MIN_COMPAT, 1);
        // KT-62/D2-88(c): tên + số hiệu syscall sinh từ `[syscall]`, không đọc chép tay.
        assert_eq!(SYS_CALL, 0);
        assert_eq!(SYS_YIELD, 8);
        assert_eq!(SYS_DEBUG_PUT_STR, 12);
        assert_eq!(SYS_EXIT, 14);
        assert_eq!(SYS_DEBUG_GET_INFO, 15);
        assert_eq!(ERROR_COUNT, 19);
        assert_eq!(SYSCALLS[0].0, "Call");
        assert_eq!(SYSCALLS[15].0, "DebugGetInfo");
        assert_eq!(SYSCALLS[15].1, 15);
        assert_eq!(ERRORS[18].1, 19);
    }

    /// KT-67b: **không** pin số chữ ký (pin 13 cũ đã lỗi thời khi spec mở theo D2-99) — số kỳ
    /// vọng **suy từ `spec/abi.toml`** (đếm khoá `[method_signature."X.Y"]`), nên thêm chữ ký
    /// không phải sửa test mà vẫn bắt được generated.rs lệch spec.
    #[test]
    fn chu_ky_method_sinh_tu_spec_va_co_read_state() {
        let spec = include_str!("../../../../spec/abi.toml");
        let expected = spec
            .lines()
            .filter(|line| line.trim_start().starts_with("[method_signature.\""))
            .count();
        assert_eq!(METHOD_SIGNATURES.len(), expected);
        let mut names: Vec<&str> = METHOD_SIGNATURES.iter().map(|(n, _, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            METHOD_SIGNATURES.len(),
            "tên chữ ký phải không trùng"
        );
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
        // KT-67b (D2-99): Retype có `dst` và trả `slot` (hợp đồng ABI, không còn `returns=[]`).
        assert!(retype.1.contains(&"dst"), "Retype phải có dst (D2-99)");
        assert_eq!(retype.2, &["slot"], "Retype trả slot (D2-99)");
        assert!(METHOD_SIGNATURES.iter().any(|(n, _, _)| *n == "Frame.Map"));
        assert!(METHOD_SIGNATURES
            .iter()
            .any(|(n, _, _)| *n == "Frame.Unmap"));
    }

    #[test]
    fn fault_kind_du_bon_bien_the_dung_spec() {
        // KT-56/D2-56a: `spec/abi.toml` mở **một lần**; code sinh phải khớp đúng 4 biến thể.
        assert_eq!(FAULT_KIND_COUNT, 4);
        let names: Vec<&str> = FAULT_KINDS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["Exit", "Vm", "UnknownSyscall", "UserException"]);
        let vm = FAULT_KINDS
            .iter()
            .find(|(n, _)| *n == "Vm")
            .expect("phải có Vm");
        for field in ["vspace: u64", "va: u64", "access", "reason"] {
            assert!(
                vm.1.iter().any(|f| f.contains(field)),
                "Vm thiếu trường {field}"
            );
        }
        assert!(FAULT_KINDS
            .iter()
            .any(|(n, f)| *n == "Exit" && f.contains(&"code: u64")));
    }

    #[test]
    fn kich_thuoc_object_dung_spec_khong_hardcode() {
        // KT-58: `vifast-core` đọc các hằng này thay vì chép tay 48/64/4096 (D2-11).
        assert_eq!(SLOT_BYTES, 48);
        assert_eq!(SLOT_ALIGN_BYTES, 16);
        assert_eq!(CNODE_BASE_BYTES, 64);
        assert_eq!(DEFAULT_ALIGN_BYTES, 16);
        assert_eq!(PAGE_BYTES, 4096);
        assert_eq!(UNTYPED_MIN_BYTES, 4096);
        assert_eq!(OBJ_TCB_BYTES, 512);
        assert_eq!(OBJ_SCHED_CONTEXT_BYTES, 256);
        assert_eq!(OBJ_ENDPOINT_BYTES, 64);
        assert_eq!(OBJ_NOTIFICATION_BYTES, 64);
        assert_eq!(OBJ_REPLY_BYTES, 64);
        assert_eq!(OBJ_SCHED_CONTROL_BYTES, 64);
    }

    #[test]
    fn object_loi_va_reserved_tach_bach() {
        assert!(CORE_OBJECTS
            .iter()
            .any(|(n, id)| *n == "SchedControl" && *id == 8));
        // **D2-242 nhóm 9 (9B)**: `VSpace`/`PageTable` → `core`, `Frame` → `frozen` ⇒ ba object
        // VM nằm trong `CORE_OBJECTS`; reserved còn **5** object `12..16` (IrqControl..PowerControl).
        for (name, id) in [("VSpace", 9u16), ("PageTable", 10), ("Frame", 11)] {
            assert!(
                CORE_OBJECTS.iter().any(|(n, i)| *n == name && *i == id),
                "{name} id {id} phải là object lõi (D2-242 nhóm 9)"
            );
            assert!(
                !RESERVED_OBJECTS.iter().any(|(n, _)| *n == name),
                "{name} KHÔNG còn reserved (D2-242 nhóm 9)"
            );
        }
        assert_eq!(
            RESERVED_OBJECTS.len(),
            5,
            "reserved còn 12..16 (D2-242 nhóm 9)"
        );
        assert!(RESERVED_OBJECTS
            .iter()
            .any(|(n, id)| *n == "IrqControl" && *id == 12));
        assert_eq!(TCB_STATES.len(), 11);
    }

    /// **KT-345a / D2-242 nhóm 2 + 7 + 8** — hằng vùng IPC buffer, kênh trả `MessageInfo` của
    /// x86_64 (`7C-r13`) và kênh `DebugGetInfo` **suy từ `[calling.*]`** phải sinh đúng.
    #[test]
    fn ipc_buffer_khop_kenh_tra_sinh_tu_spec() {
        assert_eq!(IPC_BUFFER_BYTES, 4096);
        assert_eq!(IPC_BUFFER_ALIGN, 16);
        assert_eq!(IPC_BUFFER_MR_OFFSET, 80);
        assert_eq!(IPC_BUFFER_MR_WORDS, 60);
        assert_eq!(IPC_BUFFER_EXTRA_CAPS_OFFSET, 560);
        assert_eq!(IPC_BUFFER_EXTRA_CAPS_MAX, 4);
        assert_eq!(IPC_BUFFER_EXTRA_CAPS_DST_OFFSET, 600);
        assert_eq!(IPC_BUFFER_CONFIGURE_OFFSET, 608, "[TK] chưa đóng băng");
        // 7C-r13: MessageInfo trả về ở r13, KHÔNG phải r11; clobbered đủ 7 thanh ghi.
        assert_eq!(X86_64_RETURN_MESSAGE_INFO_REG, "r13");
        assert_eq!(
            X86_64_CLOBBERED_REGS,
            ["rax", "rdx", "r13", "r10", "r8", "r9", "r12"]
        );
        assert_eq!(X86_64_PRESERVED_REGS, ["rsp", "rbx", "rbp", "r14", "r15"]);
        // 8C: kênh DebugGetInfo theo vai trò, tên thanh ghi lấy từ [calling.<arch>].
        assert_eq!(DEBUG_INFO_RETURN_VALUE_ROLE, "abi_version");
        assert_eq!(
            DEBUG_INFO_RETURN_MR_ROLES,
            ["min_compat", "features", "num_harts"]
        );
        assert_eq!(RISCV64_DEBUG_INFO_RETURN_VALUE_REG, "a1");
        assert_eq!(RISCV64_DEBUG_INFO_RETURN_MR_REGS, ["a3", "a4", "a5"]);
        assert_eq!(AARCH64_DEBUG_INFO_RETURN_VALUE_REG, "x1");
        assert_eq!(AARCH64_DEBUG_INFO_RETURN_MR_REGS, ["x3", "x4", "x5"]);
        assert_eq!(X86_64_DEBUG_INFO_RETURN_VALUE_REG, "rdx");
        assert_eq!(X86_64_DEBUG_INFO_RETURN_MR_REGS, ["r10", "r8", "r9"]);
    }
}
