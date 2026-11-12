// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-17 — đóng 2 follow-up căn lề của KT-16 (reviewer-kt16) **trước** khi sửa code.
//!
//! 1. `alloc_place_span` phải cap/assert `align ≤ ARENA_ALIGN` (4096): đáy vùng của objmem
//!    chỉ được bảo đảm căn `ARENA_ALIGN`, nên xin `Layout` căn lớn hơn sẽ trả `pa` **sai
//!    căn tuyệt đối** (vi phạm hợp đồng `Layout`). `Machine::retype` Untyped con có
//!    `size_bits ≥ 13` (cỡ = căn = 8192) phải **bị từ chối rõ** (`Alignment`), không được
//!    trả object với `pa` sai căn và không được đốt byte.
//! 2. `Store::alloc` (bump của sim) chốt hợp đồng: `align` không phải lũy thừa 2 hoặc
//!    `> ARENA_ALIGN` ⇒ `None` (không im lặng cấp `pa` sai căn); mọi `pa` trả về căn đúng.
//!
//! Đối chứng: `size_bits = 12` (cỡ = căn = 4096, đúng biên) vẫn **được nhận**, địa chỉ
//! tuyệt đối căn 4096 và watermark/bytes khớp mô hình.

use vifast_sim::kobj::{KError, Machine, ObjType, SlotRef};
use vifast_sim::Store;

const BASE: u64 = 0x1000;

fn slot(cnode: u32, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `(machine, cnode gốc, untyped)`; slot 0 của cnode giữ cap Untyped cỡ `size`.
fn setup(size: usize) -> (Machine, u32, u32) {
    let mut m = Machine::new();
    let c0 = m.add_cnode(8);
    let u0 = m.add_untyped(BASE, size);
    let cap = m.root_cap(u0);
    m.put_cap(c0, 0, cap).expect("đặt cap Untyped vào slot 0");
    (m, c0, u0)
}

#[test]
fn retype_untyped_size_bits_13_bi_tu_choi_ro_va_khong_dot_byte() {
    // Nguồn 8192 B **đủ chỗ** cho con 8192 B ⇒ trước KT-17 retype trả Ok với `pa` có thể
    // sai căn 8192 (vi phạm hợp đồng Layout); nay phải trả `Alignment`.
    let (mut m, c0, u0) = setup(8192);
    assert_eq!(
        m.retype(slot(c0, 0), ObjType::Untyped, 13, 0),
        Err(KError::Alignment),
        "cỡ/căn 8192 > ARENA_ALIGN (4096) ⇒ từ chối rõ, không trả pa sai căn"
    );
    m.check_bytes(u0).expect("bất biến byte sau lần từ chối");
    assert_eq!(m.untyped_used(u0), 0, "từ chối không được đốt byte");

    // Cỡ lớn hơn nhưng **vẫn đủ chỗ** trong Untyped 1 MiB ⇒ vẫn bị cap căn lề từ chối
    // (không phải vì hết bộ nhớ), không cấp byte nào.
    let (mut m2, c2, u2) = setup(1 << 20);
    assert_eq!(
        m2.retype(slot(c2, 0), ObjType::Untyped, 20, 0),
        Err(KError::Alignment),
        "len 1 MiB vừa vùng nhưng căn > 4096 ⇒ Alignment"
    );
    assert_eq!(m2.untyped_used(u2), 0);
    m2.check_bytes(u2).expect("bất biến byte sau lần từ chối");

    // Nguồn 4096 B: vẫn `NotEnoughMemory` như KT-09 (thứ tự kiểm không đổi ngữ nghĩa cũ);
    // size_bits quá lớn so với vùng cũng vậy — không panic tràn shift.
    let (mut m3, c3, u3) = setup(4096);
    assert_eq!(
        m3.retype(slot(c3, 0), ObjType::Untyped, 13, 0),
        Err(KError::NotEnoughMemory)
    );
    assert_eq!(
        m3.retype(slot(c3, 0), ObjType::Untyped, 30, 0),
        Err(KError::NotEnoughMemory)
    );
    assert_eq!(m3.untyped_used(u3), 0);
}

#[test]
fn retype_untyped_size_bits_12_dung_bien_van_can_le_tuyet_doi() {
    let (mut m, c0, u0) = setup(8192);
    let (base, cap) = m.untyped_arena(u0).expect("Untyped phải có arena riêng");
    assert!(cap >= 8192);
    let child = m
        .retype(slot(c0, 0), ObjType::Untyped, 12, 0)
        .expect("Untyped con 4096 B đúng biên phải được nhận");
    let obj = m.cap_obj(child).expect("cap con sống");
    let addr = m.object_addr(obj).expect("địa chỉ object");
    assert!(
        addr >= base && addr < base + cap as u64,
        "object {addr:#x} ngoài arena [{base:#x}, {:#x})",
        base + cap as u64
    );
    assert_eq!(addr % 4096, 0, "căn tuyệt đối 4096 (đáy vùng căn 4096)");
    assert_eq!((addr - base) % 4096, 0, "căn tương đối = căn tuyệt đối");
    assert_eq!(m.untyped_used(u0), 4096, "con 4096 B chiếm đúng 4096 B");
    m.check_bytes(u0).expect("bất biến byte trên arena objmem");
}

#[test]
fn store_alloc_chot_hop_dong_align_lon_hon_arena_align() {
    let mut s = Store::new(8192);
    assert_eq!(
        s.alloc(64, 8192),
        None,
        "align 8192 > ARENA_ALIGN (4096) ⇒ None, không trả pa sai căn"
    );
    assert_eq!(s.alloc(64, 0), None, "align 0 không hợp lệ ⇒ None");
    assert_eq!(s.alloc(64, 3), None, "align không phải lũy thừa 2 ⇒ None");
    assert_eq!(s.used(), 0, "mọi lần từ chối phải không đốt byte");

    // Biên: đúng 4096 được nhận và căn đúng (địa chỉ logic 0x1000 + offset).
    let a = s.alloc(64, 4096).expect("align 4096 đúng biên");
    assert_eq!(a, 0x1000);
    assert_eq!(a % 4096, 0);
    assert_eq!(s.used(), 64);
    assert_eq!(s.alloc(64, 8192), None, "vẫn từ chối sau khi đã cấp");
    assert!(s.invariant());
}
