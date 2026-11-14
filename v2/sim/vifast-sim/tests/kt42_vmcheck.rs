// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-42 — **`check_vm` ở cổng (L0)** trên một trạng thái VM đầy đủ: Frame 3 cỡ, hai VSpace,
//! shared read-only frame, TLB đã nạp, shootdown, sau đó kiểm **một cửa** `Machine::check_vm`
//! (gộp `check_frames` + `check_pages` + `check_wx` + `wx_recount` + `check_vspace` +
//! `check_asid` + `check_tlb` + `check_fault`) — K2-PLAN §3 KT-42, `v2/docs/k2-vm.md` §16.
//!
//! **Không thay hook per-op**: `sched_after_op` vẫn gọi từng checker như trước; đây là lưới
//! **đầy đủ ở cổng** (chạy trong `gate --fast` qua L0). Ca **dương tính** của `check_vm` (tiêm
//! lỗi cố ý) nằm trong unit test `kobj::tests::check_vm_bat_pte_bi_xoa_ngam_va_entry_tlb_mo_coi`
//! — tệp này giữ phần **đường vàng đầy đủ** + đo chi phí (in ra, không phải ngưỡng cổng).
//!
//! Miri: tệp này **không** đọc đĩa/không I/O nên nằm trong danh sách `v2 nightly --only-miri`
//! (`kt42_vmcheck`); `.scn` runner (`kt42_scn`) **không** chạy dưới Miri (đọc tệp).

use vifast_sim::kobj::{Machine, MapAttrs, MapRights, ObjId, ObjType, SlotRef};

const K4: u64 = 1 << 12;
const G1: u64 = 1 << 30;

fn slot(c: ObjId, s: usize) -> SlotRef {
    SlotRef::new(c, s)
}

fn frame(m: &mut Machine, c: ObjId, src: usize, size_bits: u32) -> ObjId {
    let cap = m
        .retype(slot(c, src), ObjType::Frame, size_bits, 0)
        .expect("retype Frame");
    let f = m.cap_info(cap).expect("cap Frame").obj;
    m.put_cap(c, 200 + size_bits as usize, cap)
        .expect("cap Frame vào CNode");
    f
}

fn tbl(m: &mut Machine, c: ObjId, src: usize, slot_idx: usize) -> ObjId {
    let cap = m
        .retype(slot(c, src), ObjType::PageTable, 0, 0)
        .expect("retype PageTable");
    let t = m.cap_info(cap).expect("cap PageTable").obj;
    m.put_cap(c, slot_idx, cap)
        .expect("cap PageTable vào CNode");
    t
}

/// Đường vàng: dựng trạng thái VM đầy đủ và gọi `check_vm()`; không nới ngưỡng, không đọc I/O.
#[test]
fn check_vm_sach_tren_trang_thai_vm_day_du() {
    let mut m = Machine::with_harts(2);
    let c = m.add_cnode(1024);
    let uva = m.add_untyped(0, 2 * 1024 * 1024);
    let uf4 = m.add_untyped(1 << 32, 4096);
    let uf2 = m.add_untyped(1 << 33, 4096);
    let uf1 = m.add_untyped(1 << 34, 4096);
    for (s, u) in [(0usize, uva), (1, uf4), (2, uf2), (3, uf1)] {
        let cap = m.root_cap(u);
        m.put_cap(c, s, cap).expect("cap Untyped");
    }
    let f4 = frame(&mut m, c, 1, 12);
    let f2 = frame(&mut m, c, 2, 21);
    let f1 = frame(&mut m, c, 3, 30);

    // Hai VSpace, mỗi cái root → mid → leaf (4 KiB) + mid 2 MiB ở root[1].
    let mut vs = [0u32; 2];
    for (i, v) in vs.iter_mut().enumerate() {
        let cap = m
            .vspace_create(slot(c, 0))
            .unwrap_or_else(|e| panic!("vspace_create {i}: {e:?}"));
        *v = m.cap_info(cap).expect("cap VSpace").obj;
        let root = m.vspace_root(*v).expect("root");
        let mid4 = tbl(&mut m, c, 0, 10 + 2 * i);
        m.page_table_attach(root, 0, mid4).expect("attach mid 4K");
        let leaf = tbl(&mut m, c, 0, 11 + 2 * i);
        m.page_table_attach(mid4, 0, leaf).expect("attach leaf");
        let mid2 = tbl(&mut m, c, 0, 20 + i);
        m.page_table_attach(root, G1, mid2).expect("attach mid 2M");
    }

    // Shared read-only: cùng f4 map READ vào **hai** VSpace (không bản sao).
    m.vspace_map_obj(vs[0], K4, f4, MapRights::READ, MapAttrs::WB)
        .expect("map shared vs0");
    m.vspace_map_obj(vs[1], 2 * K4, f4, MapRights::READ, MapAttrs::WB)
        .expect("map shared vs1");
    // Hai cỡ còn lại (2 MiB / 1 GiB) + quyền/attrs khác nhau.
    m.vspace_map_obj(vs[0], G1, f2, MapRights::READ, MapAttrs::WB)
        .expect("map 2M");
    m.vspace_map_obj(
        vs[0],
        2 * G1,
        f1,
        MapRights::READ,
        MapAttrs::WB | MapAttrs::EXEC,
    )
    .expect("map 1G");

    let meta = m.frame_meta(f4).expect("meta f4");
    assert_eq!(
        (meta.maps, meta.mapped_w, meta.mapped_x),
        (2, 0, 0),
        "shared read-only: 2 mapping, không W/X"
    );
    let a = m.vspace_walk(vs[0], K4).expect("walk vs0");
    let b = m.vspace_walk(vs[1], 2 * K4).expect("walk vs1");
    assert_eq!(a.frame, f4);
    assert_eq!(b.frame, f4);
    assert_eq!(a.pa, b.pa, "shared frame phải cùng pa (không bản sao)");
    assert_eq!(m.frame_count(), 3, "không cấp Frame mới");

    // TLB: mỗi hart chạy một VSpace và nạp entry.
    m.vspace_activate(vs[0], 0).expect("activate vs0 hart 0");
    m.vspace_activate(vs[1], 1).expect("activate vs1 hart 1");
    m.set_current_hart(0).expect("hart 0");
    assert!(!m.tlb_translate(vs[0], K4).expect("fill 0").from_tlb);
    m.set_current_hart(1).expect("hart 1");
    assert!(!m.tlb_translate(vs[1], 2 * K4).expect("fill 1").from_tlb);

    // Một cửa kiểm duy nhất — trạng thái giàu nhất của K2 VM.
    m.check_vm().expect("check_vm sạch trên trạng thái đầy đủ");
    let recount = m.wx_recount().expect("wx_recount");
    assert_eq!(recount.frames, 3, "3 Frame còn mapping");
    assert_eq!(recount.maps, 4, "2 shared + 2M + 1G");
    assert_eq!((recount.mapped_w, recount.mapped_x), (0, 1));

    // Shootdown: unmap shared ở vs0 (active_on = {0}) ⇒ Ok chỉ khi ack đủ; entry mất.
    m.set_current_hart(0).expect("hart 0");
    m.vspace_unmap(vs[0], K4).expect("unmap shared vs0");
    assert!(m.tlb_lookup(0, vs[0], K4).expect("lookup").is_none());
    let s = m.last_tlb_shootdown().expect("shootdown");
    assert!(s.completed, "shootdown phải xong");
    assert_eq!(s.acked, s.targets, "Ok ⇔ acked == targets");
    // Frame vẫn sống với **một** mapping bên vs1 (shared không bị huỷ theo).
    let meta = m.frame_meta(f4).expect("meta f4");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (1, 0, 0));
    m.check_vm().expect("check_vm sạch sau shootdown");

    // Huỷ nốt vs1 ⇒ Frame về 0 mapping nhưng vẫn sống (không finalise).
    m.set_current_hart(1).expect("hart 1");
    m.vspace_unmap(vs[1], 2 * K4).expect("unmap shared vs1");
    let meta = m.frame_meta(f4).expect("meta f4");
    assert_eq!((meta.maps, meta.mapped_w, meta.mapped_x), (0, 0, 0));
    m.check_vm().expect("check_vm sạch cuối");
}
