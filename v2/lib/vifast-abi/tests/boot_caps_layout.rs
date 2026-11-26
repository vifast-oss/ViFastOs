// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-417 hardening (F3)** — ghim hợp đồng **bố cục byte** của `BootCapLayout` (`#[repr(C)]`,
//! `D2-261` nhóm 8), sinh từ `spec/abi.toml [boot_caps]` bởi `v2/tools/vifast-gen/gen.py`.
//!
//! `boot_abi = 1` là lời hứa ABI với kimage/người nạp: `size_of`, `align_of` và **offset của
//! từng trường** phải đúng thứ tự `BOOT_CAPS_CSPACE_ORDER` → `BOOT_CAPS_TAIL_ORDER` →
//! `BOOT_CAPS_PROC_ORDER` → `child` → `netsrv` (đúng thứ tự `gen.py` phát ra). Trước bản vá này
//! **không** có assert layout nào: đảo thứ tự mảng/trường mà giữ `boot_abi = 1` vẫn xanh.
//!
//! **Đổi bố cục (thêm/bớt/đảo trường) ⇒ BẮT BUỘC bump `[boot_caps].boot_abi`** rồi mới sửa số
//! kỳ vọng ở đây — test cố ý đỏ khi bố cục đổi mà `boot_abi` không đổi.
//!
//! Số kỳ vọng là **số thật** của `generated.rs` (rustc 1.100.0), không phải ước lượng.

use core::mem::{align_of, offset_of, size_of};

use vifast_abi::{
    BootCapLayout, BOOT_ABI, BOOT_CAPS_NONE_CSPACE, BOOT_CAPS_NONE_PROC, BOOT_CAPS_PRODUCT,
    BOOT_CAPS_TEST,
};

#[test]
fn boot_abi_va_sentinel_duoc_ghim() {
    // `boot_abi` là khoá RIÊNG của hợp đồng boot, độc lập `abi_version` (D2-261 §B.0).
    // Đổi bố cục bên dưới mà không bump hằng này là **phá ABI**.
    assert_eq!(BOOT_ABI, 1, "đổi bố cục BootCapLayout ⇒ phải bump boot_abi");
    assert_eq!(BOOT_CAPS_NONE_CSPACE, 0xFFFF);
    assert_eq!(BOOT_CAPS_NONE_PROC, 0xFF);
}

#[test]
fn boot_cap_layout_kich_thuoc_va_can_le() {
    // 31 slot u16 (17 cap + self_cspace + tcb[5] + sc[5] + 3 loop_*) = 62 B, rồi 4 ô u8
    // (procsrv, shell, child, netsrv) = 66 B; align = 2 (u16 không đệm cuối).
    assert_eq!(size_of::<BootCapLayout>(), 66);
    assert_eq!(align_of::<BootCapLayout>(), 2);
}

#[test]
fn boot_cap_layout_offset_cspace() {
    // BOOT_CAPS_CSPACE_ORDER — 17 slot cap u16, liền nhau từ 0.
    assert_eq!(offset_of!(BootCapLayout, fault), 0);
    assert_eq!(offset_of!(BootCapLayout, proc), 2);
    assert_eq!(offset_of!(BootCapLayout, uart), 4);
    assert_eq!(offset_of!(BootCapLayout, reply), 6);
    assert_eq!(offset_of!(BootCapLayout, ps), 8);
    assert_eq!(offset_of!(BootCapLayout, sched_control), 10);
    assert_eq!(offset_of!(BootCapLayout, fs), 12);
    assert_eq!(offset_of!(BootCapLayout, events), 14);
    assert_eq!(offset_of!(BootCapLayout, net), 16);
    assert_eq!(offset_of!(BootCapLayout, net_reply), 18);
    assert_eq!(offset_of!(BootCapLayout, net_kern), 20);
    assert_eq!(offset_of!(BootCapLayout, net_kern_reply), 22);
    assert_eq!(offset_of!(BootCapLayout, exec), 24);
    assert_eq!(offset_of!(BootCapLayout, mem), 26);
    assert_eq!(offset_of!(BootCapLayout, loop_ep), 28);
    assert_eq!(offset_of!(BootCapLayout, net_client), 30);
    assert_eq!(offset_of!(BootCapLayout, net_client_reply), 32);
    // BOOT_CAPS_TAIL_ORDER — self_cspace rồi tcb[5]/sc[5], loop_*.
    assert_eq!(offset_of!(BootCapLayout, self_cspace), 34);
    assert_eq!(offset_of!(BootCapLayout, tcb), 36);
    assert_eq!(offset_of!(BootCapLayout, sc), 46);
    assert_eq!(offset_of!(BootCapLayout, loop_arena), 56);
    assert_eq!(offset_of!(BootCapLayout, loop_tcb), 58);
    assert_eq!(offset_of!(BootCapLayout, loop_sc), 60);
}

#[test]
fn boot_cap_layout_offset_proc_va_anh() {
    // BOOT_CAPS_PROC_ORDER + child/netsrv: 4 ô u8 cuối (D2-261 nhóm 8 — child/netsrv CÙNG ô 2).
    assert_eq!(offset_of!(BootCapLayout, procsrv), 62);
    assert_eq!(offset_of!(BootCapLayout, shell), 63);
    assert_eq!(offset_of!(BootCapLayout, child), 64);
    assert_eq!(offset_of!(BootCapLayout, netsrv), 65);
}

#[test]
fn boot_caps_profile_khoi_tao_dung_cung_boc_cuc() {
    // Hai profile là **giá trị** của cùng kiểu: đọc lại qua struct để bắt lệch kiểu/khởi tạo
    // (nếu profile lệch bố cục, `size_of`/offset ở trên không bắt được).
    let test = BOOT_CAPS_TEST;
    let product = BOOT_CAPS_PRODUCT;
    assert_eq!(test.child, 2);
    assert_eq!(test.netsrv, BOOT_CAPS_NONE_PROC);
    assert_eq!(product.child, BOOT_CAPS_NONE_PROC);
    assert_eq!(product.netsrv, 2);
    assert_eq!(test.tcb, [10, 11, 12, 13, 14]);
    assert_eq!(test.sc, [30, 31, 32, 33, 34]);
    assert_eq!(test.fault, 1);
    assert_eq!(test.uart, 3);
    assert_eq!(test.exec, 18);
    assert_eq!(test.loop_sc, 25);
}
