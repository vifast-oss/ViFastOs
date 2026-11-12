// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-16 — sim dùng **allocator object của `vifast-objmem`** (D2-14, D2-47d).
//!
//! Viết **trước** khi đổi ruột (D2-05): `vifast-sim::Store` và ruột `kobj::Machine` còn là
//! bump/Vec giả nên các API dưới đây **chưa có** hoặc object **không** nằm trong arena:
//! - `Machine::with_arena_bytes` / `arena_capacity` (arena kernel mặc định 1 MiB [ƯL]);
//! - `Machine::untyped_arena` / `object_addr` — object Retype phải có địa chỉ **trong**
//!   `[arena.base, arena.base + cap)` của Untyped chứa nó;
//! - `Machine::with_object` — truy cập body qua objmem, object đã Revoke trả `Dead`;
//! - `Store::arena_base` / `arena_used` — bump của store giả phải do objmem cấp.
//!
//! Bất biến giữ nguyên nghĩa của mô hình: `untyped_used` (watermark) == byte arena đã cấp,
//! `allocated + gap == used`, địa chỉ object căn lề theo loại (CNode căn 64).

use vifast_objmem::Dead;
use vifast_sim::kobj::{Machine, ObjType, SlotRef};
use vifast_sim::Store;

const BASE: u64 = 0x1000;

fn slot(cnode: u32, s: usize) -> SlotRef {
    SlotRef::new(cnode, s)
}

/// `(machine, cnode gốc, untyped)`; slot 0 của cnode giữ cap Untyped.
fn setup(size: usize) -> (Machine, u32, u32) {
    let mut m = Machine::new();
    let c0 = m.add_cnode(8);
    let u0 = m.add_untyped(BASE, size);
    let cap = m.root_cap(u0);
    m.put_cap(c0, 0, cap).expect("đặt cap Untyped vào slot 0");
    (m, c0, u0)
}

#[test]
fn machine_co_arena_kernel_mac_dinh_1mib_va_with_arena_bytes() {
    let m = Machine::new();
    assert_eq!(
        m.arena_capacity(),
        1 << 20,
        "arena kernel mặc định 1 MiB [ƯL] (K1A-PLAN §4/KT-16)"
    );
    let m = Machine::with_arena_bytes(4096);
    assert_eq!(m.arena_capacity(), 4096);
    assert_eq!(m.arena_used(), 0, "Machine rỗng chưa cấp byte nào");
}

#[test]
fn object_retype_nam_trong_arena_cua_untyped() {
    let (mut m, c0, u0) = setup(8192);
    let (base, cap) = m.untyped_arena(u0).expect("Untyped phải có arena riêng");
    assert!(cap >= 8192, "arena của Untyped phải đủ cỡ vùng: {cap}");
    assert_eq!(m.untyped_used(u0), 0);

    let cap_tcb = m.retype(slot(c0, 0), ObjType::Tcb, 0, 0).expect("TCB");
    let tcb = m.cap_obj(cap_tcb).expect("cap TCB sống");
    let addr = m
        .object_addr(tcb)
        .expect("object phải có địa chỉ trong arena");
    assert!(
        addr >= base && addr < base + cap as u64,
        "object {tcb} ở {addr:#x} ngoài arena [{base:#x}, {:#x})",
        base + cap as u64
    );
    assert_eq!(addr % 16, 0, "TCB căn lề mặc định 16");
    assert_eq!(
        m.untyped_used(u0),
        512,
        "watermark = byte arena đã cấp sau TCB"
    );
    assert_eq!(
        m.untyped_used(u0),
        m.untyped_allocated(u0) + m.untyped_gap(u0),
        "arena.used() khớp tổng byte object + gap căn lề"
    );

    // CNode căn 64 (CNODE_BASE_BYTES) — chứng minh căn lề do arena objmem cấp.
    let cap_cn = m.retype(slot(c0, 0), ObjType::CNode, 4, 0).expect("CNode");
    let cn = m.cap_obj(cap_cn).expect("cap CNode sống");
    let (offset, len) = m.object_bytes(cn).expect("CNode có vùng nhớ");
    assert_eq!(offset, 512, "CNode nối sau TCB");
    assert_eq!(len, 64 + 16 * 48);
    let addr_cn = m.object_addr(cn).expect("địa chỉ CNode");
    assert_eq!(
        (addr_cn - base) % 64,
        0,
        "CNode phải căn 64 trong arena objmem"
    );
    assert_eq!(m.untyped_used(u0), 512 + 64 + 16 * 48);
    m.check_bytes(u0).expect("bất biến byte trên arena objmem");
}

#[test]
fn revoke_truy_cap_body_qua_objmem_tra_dead() {
    let (mut m, c0, _u0) = setup(8192);
    let cap_tcb = m.retype(slot(c0, 0), ObjType::Tcb, 0, 0).expect("TCB");
    let tcb = m.cap_obj(cap_tcb).expect("cap TCB sống");
    assert_eq!(
        m.with_object(tcb, |_| ()),
        Ok(()),
        "object sống phải truy cập được qua objmem"
    );

    // Revoke cap Untyped ở slot 0 ⇒ finalise toàn bộ object trong Untyped.
    let deleted = m.revoke(c0, Some(0)).expect("revoke Untyped");
    assert_eq!(deleted, 1);
    assert!(!m.object_alive(tcb));
    assert_eq!(
        m.with_object(tcb, |_| ()),
        Err(Dead),
        "body đã finalise trong objmem ⇒ Dead"
    );
    assert_eq!(m.untyped_used(_u0), 0, "arena của Untyped reset về 0");
}

#[test]
fn store_dung_bump_cua_objmem_va_can_le_64() {
    let mut s = Store::new(4096);
    assert_eq!(s.arena_base(), 0x1000, "địa chỉ logic của sim không đổi");
    assert_eq!(s.arena_used(), 0);
    let a = s.alloc(64, 16).expect("còn chỗ");
    let b = s.alloc(64, 64).expect("còn chỗ");
    assert_eq!(a, 0x1000, "block đầu ở đáy vùng");
    assert_eq!(b, 0x1040, "block 64 B nối sau block trước");
    assert_eq!(b % 64, 0, "căn lề 64 do arena objmem bảo đảm");
    assert_eq!(s.arena_used(), s.used(), "used của store = watermark arena");
    assert_eq!(s.used(), 0x80);
    assert_eq!(s.alloc(4096, 64), None, "hết chỗ ⇒ None");
    assert!(s.invariant());
}
