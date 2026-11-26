// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test **host** cho object layer nội bộ KT-52 (nay ở `vifast_core::vm`; shim `plat-common::obj`
//! đã xoá ở `KT-417d`).
//!
//! ```text
//! cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
//! ```
//!
//! D2-72 Q12: Miri **không** áp kernel `no_std`/asm/MMIO; phần logic object layer đã tách khỏi
//! asm (đi qua trait `PhysMem`) nên chạy được trên host — tệp này kiểm **cùng** mã sẽ chạy trên
//! QEMU: Retype/Revoke (byte + PA), Frame 3 cỡ, VSpace map/unmap/protect + `walk_shape`, W^X
//! toàn cục 2 ca, bất biến byte, `check_objmem` (không frame mồ côi, walk khớp, 0 rò).

use vifast_core::vm::{
    CapId, FlushRequest, Machine, MapFlags, ObjError, ObjKind, PhysMem, UNTYPED_PA_WINDOW,
};

/// RAM phẳng `0..len` cho host: `pa` chính là chỉ số byte.
struct Ram {
    bytes: Vec<u8>,
}

impl Ram {
    fn new(len: usize) -> Self {
        Self {
            bytes: vec![0u8; len],
        }
    }
}

impl PhysMem for Ram {
    fn read_u64(&self, pa: u64) -> Option<u64> {
        let start = usize::try_from(pa).ok()?;
        let end = start.checked_add(8)?;
        let slice = self.bytes.get(start..end)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(slice);
        Some(u64::from_le_bytes(buf))
    }

    fn write_u64(&mut self, pa: u64, value: u64) -> bool {
        let Ok(start) = usize::try_from(pa) else {
            return false;
        };
        let Some(end) = start.checked_add(8) else {
            return false;
        };
        let Some(slice) = self.bytes.get_mut(start..end) else {
            return false;
        };
        slice.copy_from_slice(&value.to_le_bytes());
        true
    }
}

const ARENA: u64 = 0x00c0_0000; // 12 MiB, sau vùng PA test
const PA0: u64 = 0x0040_0000; // 4 MiB — căn 2 MiB, vùng RAM test
const PA1: u64 = 0x4000_0000; // 1 GiB — căn 1 GiB (cửa sổ Frame 1 GiB)
const VA: u64 = 0x4000_0000;

fn machine() -> (Machine<Ram>, CapId, CapId) {
    let mut m = Machine::new(Ram::new(16 * 1024 * 1024));
    let u0 = m.add_untyped(ARENA, 64 * 1024, PA0).expect("untyped U0");
    let u1 = m
        .add_untyped(ARENA + 64 * 1024, 4096, PA1)
        .expect("untyped U1");
    (m, u0, u1)
}

#[test]
fn retype_vspace_page_table_frame_va_bat_bien_byte() {
    let (mut m, u0, _u1) = machine();
    let before = m.untyped_stats(u0).unwrap();
    assert_eq!(before.used, 0);
    assert_eq!(before.capacity, 64 * 1024);
    let vs = m.retype(u0, ObjKind::VSpace, 0).expect("retype VSpace");
    assert_eq!(m.cap_kind(vs).unwrap(), ObjKind::VSpace);
    assert_eq!(m.vspace_asid(vs).unwrap(), 1);
    let root = m.vspace_root_pa(vs).expect("root PA");
    assert_eq!(root & 0xfff, 0, "root 4 KiB căn lề");
    assert_eq!(root, ARENA, "VSpace lấy root ở đầu arena");
    let pt = m.retype(u0, ObjKind::PageTable, 12).expect("retype PT");
    assert_eq!(m.cap_kind(pt).unwrap(), ObjKind::PageTable);
    let fr = m.retype(u0, ObjKind::Frame, 12).expect("retype Frame");
    assert_eq!(m.frame_pa(fr).unwrap(), PA0);
    assert_eq!(m.frame_size_bits(fr).unwrap(), 12);
    let after = m.untyped_stats(u0).unwrap();
    // VSpace (root 4 KiB) + PageTable (4 KiB) = 8 KiB tiêu byte; Frame 0 byte (D2-66).
    assert_eq!(after.used, 2 * 4096);
    assert_eq!(after.capacity, before.capacity);
    assert_eq!(after.free, after.capacity - after.used);
    let stats = m.check_objmem().expect("check_objmem sạch");
    assert_eq!(stats.counts.vspaces, 1);
    assert_eq!(stats.counts.frames, 1);
    assert!(stats.counts.page_tables >= 2);
    assert_eq!(stats.orphans, 0);
    // Revoke: trả sạch byte + PA.
    m.revoke(fr).unwrap();
    m.revoke(pt).unwrap();
    m.revoke(vs).unwrap();
    let end = m.untyped_stats(u0).unwrap();
    assert_eq!(end.used, 0);
    assert_eq!(end.free, end.capacity);
    assert_eq!(end.pa_used, 0);
    assert_eq!(m.counts().frames, 0);
    assert_eq!(m.counts().vspaces, 0);
    m.check_objmem().expect("check_objmem sau revoke");
}

#[test]
fn frame_ba_co_12_21_30_va_that_4k_2m() {
    let (mut m, u0, u1) = machine();
    // Cấp **giảm dần** (K2 KT-36: không đệm căn lề ngầm).
    let f2m = m.retype(u0, ObjKind::Frame, 21).expect("Frame 2 MiB");
    assert_eq!(m.frame_pa(f2m).unwrap(), PA0);
    assert_eq!(m.frame_pa(f2m).unwrap() & ((1 << 21) - 1), 0);
    let f4k = m.retype(u0, ObjKind::Frame, 12).expect("Frame 4 KiB");
    assert_eq!(m.frame_pa(f4k).unwrap(), PA0 + (1 << 21));
    // Ghi/đọc **thật** qua PhysMem tại PA của Frame (single source: RAM vật lý).
    let pa2m = m.frame_pa(f2m).unwrap();
    let pa4k = m.frame_pa(f4k).unwrap();
    assert!(m.write_phys_u64(pa2m, 0x1122_3344_5566_7788));
    assert_eq!(m.read_phys_u64(pa2m), Some(0x1122_3344_5566_7788));
    assert!(m.write_phys_u64(pa4k, 0xa5a5_5a5a_0123_4567));
    assert_eq!(m.read_phys_u64(pa4k), Some(0xa5a5_5a5a_0123_4567));
    // size_bits lạ: InvalidArgument, atomic (không đổi byte).
    let before = m.untyped_stats(u0).unwrap();
    assert_eq!(
        m.retype(u0, ObjKind::Frame, 13).unwrap_err(),
        ObjError::InvalidArgument
    );
    assert_eq!(m.untyped_stats(u0).unwrap(), before);
    m.revoke(f4k).unwrap();
    m.revoke(f2m).unwrap();
    // Frame 1 GiB: cửa sổ PA 1 GiB căn (QEMU -m 128M ⇒ chỉ kiểm nhãn/căn lề).
    let f1g = m.retype(u1, ObjKind::Frame, 30).expect("Frame 1 GiB");
    assert_eq!(m.frame_pa(f1g).unwrap(), PA1);
    assert_eq!(m.frame_pa(f1g).unwrap() & (UNTYPED_PA_WINDOW - 1), 0);
    assert_eq!(m.frame_size_bits(f1g).unwrap(), 30);
    assert_eq!(m.untyped_stats(u1).unwrap().pa_used, UNTYPED_PA_WINDOW);
    m.revoke(f1g).unwrap();
    assert_eq!(m.untyped_stats(u1).unwrap().pa_used, 0);
    let stats = m.check_objmem().unwrap();
    assert_eq!(stats.counts.frames, 0);
}

#[test]
fn vspace_map_unmap_protect_walk_khop() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let pa = m.frame_pa(fr).unwrap();
    // Chưa map ⇒ walk invalid ở cấp gốc.
    let w = m.walk(vs, VA).unwrap();
    assert!(!w.valid);
    m.map(vs, VA, fr, MapFlags::RW_NX_USER).expect("map RW");
    let w = m.walk(vs, VA).unwrap();
    assert!(w.valid && w.leaf);
    assert_eq!(w.level, 0);
    assert_eq!(w.phys, pa);
    assert!(MapFlags::RW_NX_USER.matches_pte(w.raw));
    let mut flush = Vec::new();
    let n = m.drain_flushes(|f| flush.push(f));
    assert_eq!(n, 1);
    assert!(matches!(flush[0], FlushRequest::Page { va, .. } if va == VA));
    // Map trùng ⇒ AlreadyMapped.
    assert_eq!(
        m.map(vs, VA, fr, MapFlags::RW_NX_USER).unwrap_err(),
        ObjError::AlreadyMapped
    );
    // Protect sang R-NX: W^X trên phần còn lại (không có) ⇒ OK; PTE mất W.
    let ro = MapFlags {
        read: true,
        write: false,
        exec: false,
        user: true,
        device: false,
    };
    m.protect(vs, VA, ro).expect("protect R");
    let w = m.walk(vs, VA).unwrap();
    assert_eq!(w.raw & (1 << 2), 0, "PTE không còn W");
    assert!(ro.matches_pte(w.raw));
    let stats = m.check_objmem().unwrap();
    assert_eq!(stats.counts.mappings, 1);
    assert_eq!(stats.walked, 1);
    // Unmap: PTE=0, walk invalid, mapping 0.
    m.unmap(vs, VA).unwrap();
    assert!(!m.walk(vs, VA).unwrap().valid);
    assert_eq!(m.check_objmem().unwrap().counts.mappings, 0);
    assert_eq!(m.unmap(vs, VA).unwrap_err(), ObjError::NotMapped);
    m.revoke(vs).unwrap();
    m.revoke(fr).unwrap();
    assert_eq!(m.untyped_stats(u0).unwrap().used, 0);
    m.check_objmem().unwrap();
}

#[test]
fn wx_ca_1_x_roi_w_bi_tu_choi_atomic() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    m.map(vs, VA, fr, MapFlags::RX_USER).expect("map RX");
    let before = m.counts();
    assert_eq!(
        m.map(vs, VA + 0x1000, fr, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::WxViolation
    );
    // Atomic: không mapping mới, không PTE, đếm Frame không đổi.
    assert!(!m.walk(vs, VA + 0x1000).unwrap().valid);
    assert_eq!(m.counts(), before);
    let stats = m.check_objmem().unwrap();
    assert_eq!(stats.counts.frames, 1);
    m.revoke(vs).unwrap();
    m.revoke(fr).unwrap();
}

#[test]
fn wx_ca_2_device_exec_bi_tu_choi() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    assert_eq!(
        m.map(vs, VA, fr, MapFlags::DEVICE_RX).unwrap_err(),
        ObjError::WxViolation
    );
    assert!(!m.walk(vs, VA).unwrap().valid);
    assert_eq!(m.counts().mappings, 0);
    // Device RW (NX) hợp lệ.
    m.map(vs, VA, fr, MapFlags::DEVICE_RW).expect("Device RW");
    assert_eq!(m.check_objmem().unwrap().counts.mappings, 1);
    m.revoke(vs).unwrap();
    m.revoke(fr).unwrap();
}

#[test]
fn untyped_byte_bat_bien_va_khong_ro() {
    let (mut m, u0, _u1) = machine();
    let s0 = m.untyped_stats(u0).unwrap();
    let pt1 = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let pt2 = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let s1 = m.untyped_stats(u0).unwrap();
    assert_eq!(s1.capacity, s0.capacity);
    assert_eq!(s1.used, s0.used + 2 * 4096);
    assert_eq!(s1.free, s0.free - 2 * 4096);
    m.check_objmem().unwrap();
    m.revoke(pt1).unwrap();
    m.revoke(pt2).unwrap();
    let s2 = m.untyped_stats(u0).unwrap();
    assert_eq!(s2.used, s0.used);
    assert_eq!(s2.free, s0.free);
    assert_eq!(s2.holes, 0);
    m.check_objmem().unwrap();
    // Hết arena ⇒ NotEnoughMemory **atomic** (byte trước/sau y hệt, check sạch).
    let mut caps = Vec::new();
    loop {
        let before = m.untyped_stats(u0).unwrap();
        match m.retype(u0, ObjKind::PageTable, 12) {
            Ok(cap) => caps.push(cap),
            Err(e) => {
                assert_eq!(e, ObjError::NotEnoughMemory);
                assert_eq!(
                    m.untyped_stats(u0).unwrap(),
                    before,
                    "lỗi hết chỗ phải atomic"
                );
                break;
            }
        }
    }
    assert!(!caps.is_empty());
    m.check_objmem().unwrap();
    for cap in caps.into_iter().rev() {
        m.revoke(cap).unwrap();
    }
    assert_eq!(m.untyped_stats(u0).unwrap().used, s0.used);
    assert_eq!(m.untyped_stats(u0).unwrap().holes, 0);
    m.check_objmem().unwrap();
}

#[test]
fn revoke_untyped_tra_sach_byte_va_pa() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    m.map(vs, VA, fr, MapFlags::RW_NX_USER).unwrap();
    let _ = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    assert!(m.check_objmem().unwrap().counts.mappings == 1);
    // Revoke Untyped: VSpace teardown trước, rồi Frame/PageTable; trả sạch.
    m.revoke(u0).unwrap();
    let s = m.untyped_stats(u0).unwrap();
    assert_eq!(s.used, 0);
    assert_eq!(s.free, s.capacity);
    assert_eq!(s.pa_used, 0);
    assert_eq!(s.frames, 0);
    let stats = m.check_objmem().unwrap();
    assert_eq!(stats.counts.mappings, 0);
    assert_eq!(stats.counts.frames, 0);
    assert_eq!(stats.counts.vspaces, 0);
    assert_eq!(stats.counts.page_tables, 0);
    // Cap cũ (Frame) đã chết.
    assert_eq!(m.cap_kind(fr).unwrap_err(), ObjError::InvalidCapability);
    // Untyped dùng lại được sau revoke.
    let pt = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    m.revoke(pt).unwrap();
    m.check_objmem().unwrap();
}

#[test]
fn revoke_vspace_giu_frame_va_tra_dem() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    m.map(vs, VA, fr, MapFlags::RW_NX_USER).unwrap();
    m.revoke(vs).unwrap();
    // Frame còn sống, đếm mapping về 0, cap Frame còn.
    let stats = m.check_objmem().unwrap();
    assert_eq!(stats.counts.mappings, 0);
    assert_eq!(stats.counts.frames, 1);
    assert_eq!(m.cap_kind(fr).unwrap(), ObjKind::Frame);
    // VSpace mới dùng lại được slot/arena đã trả (không rò).
    let vs2 = m.retype(u0, ObjKind::VSpace, 0).expect("VSpace sau revoke");
    assert_eq!(m.vspace_asid(vs2).unwrap(), 2);
    m.revoke(vs2).unwrap();
    m.revoke(fr).unwrap();
    m.check_objmem().unwrap();
}

#[test]
fn frame_dang_map_revoke_has_children() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    m.map(vs, VA, fr, MapFlags::RW_NX_USER).unwrap();
    assert_eq!(m.revoke(fr).unwrap_err(), ObjError::HasChildren);
    m.unmap(vs, VA).unwrap();
    m.revoke(fr).unwrap();
    m.revoke(vs).unwrap();
    m.check_objmem().unwrap();
}

#[test]
fn kiem_tra_bien_va_atomic() {
    let (mut m, u0, _u1) = machine();
    // VA không căn cỡ / không canonical.
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    assert_eq!(
        m.map(vs, 0x4000_0800, fr, MapFlags::RW_NX).unwrap_err(),
        ObjError::Alignment
    );
    assert_eq!(
        m.map(vs, 0x0000_8000_0000_0000, fr, MapFlags::RW_NX)
            .unwrap_err(),
        ObjError::Alignment
    );
    // W mà không R ⇒ InvalidArgument (Sv39 dành W=1,R=0).
    let bad = MapFlags {
        read: false,
        write: true,
        exec: false,
        user: false,
        device: false,
    };
    assert_eq!(
        m.map(vs, VA, fr, bad).unwrap_err(),
        ObjError::InvalidArgument
    );
    // size_bits lạ cho PageTable/VSpace.
    assert_eq!(
        m.retype(u0, ObjKind::PageTable, 21).unwrap_err(),
        ObjError::InvalidArgument
    );
    assert_eq!(
        m.retype(u0, ObjKind::VSpace, 12).unwrap_err(),
        ObjError::InvalidArgument
    );
    // Retype(Untyped) chưa hỗ trợ (K3b).
    assert_eq!(
        m.retype(u0, ObjKind::Untyped, 12).unwrap_err(),
        ObjError::NotSupported
    );
    m.check_objmem().unwrap();
}

#[test]
fn revoke_untyped_tu_choi_frame_map_cheo_untyped() {
    // L1 (reviewer): Frame(U0) đang map bởi VSpace(U1) ⇒ revoke(U0) phải `HasChildren`,
    // không được giải phóng Frame và để mapping trỏ object chết.
    let mut m = Machine::new(Ram::new(16 * 1024 * 1024));
    let u0 = m.add_untyped(ARENA, 64 * 1024, PA0).unwrap();
    let u1 = m.add_untyped(ARENA + 64 * 1024, 16 * 1024, PA1).unwrap();
    let vs1 = m.retype(u1, ObjKind::VSpace, 0).unwrap();
    let fr0 = m.retype(u0, ObjKind::Frame, 12).unwrap();
    m.map(vs1, VA, fr0, MapFlags::RW_NX).unwrap();
    assert_eq!(m.revoke(u0).unwrap_err(), ObjError::HasChildren);
    // Atomic: mapping + Frame còn nguyên, check sạch.
    let st = m.check_objmem().unwrap();
    assert_eq!(st.counts.mappings, 1);
    assert_eq!(st.counts.frames, 1);
    m.revoke(vs1).unwrap();
    m.revoke(fr0).unwrap();
    m.revoke(u0).unwrap();
    m.check_objmem().unwrap();
}

#[test]
fn hang_doi_flush_day_bao_loi_atomic() {
    // L2 (reviewer): hàng đợi `sfence` đầy ⇒ map/unmap trả `NoSlots` **trước** mutation,
    // không bao giờ rơi flush im lặng.
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    for i in 0..16u64 {
        m.map(vs, VA + i * 0x1000, fr, MapFlags::RW_NX)
            .expect("map trong 16 slot flush");
    }
    assert_eq!(m.counts().mappings, 16);
    assert_eq!(
        m.map(vs, VA + 16 * 0x1000, fr, MapFlags::RW_NX)
            .unwrap_err(),
        ObjError::NoSlots
    );
    let before = m.counts();
    assert_eq!(m.unmap(vs, VA).unwrap_err(), ObjError::NoSlots);
    assert_eq!(m.counts(), before, "unmap lỗi flush phải atomic");
    assert_eq!(m.drain_flushes(|_| {}), 16);
    m.unmap(vs, VA).unwrap();
    assert_eq!(m.drain_flushes(|_| {}), 1);
    m.revoke(vs).unwrap();
    m.revoke(fr).unwrap();
    m.check_objmem().unwrap();
}

#[test]
fn frame_1g_khong_duoc_map_khi_chua_co_backing() {
    // L7 (reviewer): cửa sổ PA 1 GiB là nhãn trên -m 128M ⇒ `map` Frame 1 GiB từ chối
    // `NotSupported` cho tới K4/K5 (TAP 32 chỉ kiểm nhãn).
    let (mut m, u0, u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let f1g = m.retype(u1, ObjKind::Frame, 30).unwrap();
    assert_eq!(m.frame_pa(f1g).unwrap(), PA1);
    assert_eq!(
        m.map(vs, VA, f1g, MapFlags::RW_NX).unwrap_err(),
        ObjError::NotSupported
    );
    assert_eq!(m.counts().mappings, 0);
    m.revoke(f1g).unwrap();
    m.revoke(vs).unwrap();
    m.check_objmem().unwrap();
}

/// **KT-55** (conformance tìm ra lỗi platform): một mapping `W|X` **cùng lúc** phải bị từ chối —
/// nếu không, `mapped_w` và `mapped_x` cùng tăng trên một Frame ⇒ chính bất biến W^X toàn cục của
/// `check_objmem` (D2-67) bị chính object layer tạo ra. K2 từ chối (`WxViolation`) nên kernel phải
/// khớp; đây là **lỗi đỏ trước** của KT-55, không phải nới ngưỡng.
#[test]
fn wx_ca_3_cung_map_w_x_bi_tu_choi_atomic() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let wx = MapFlags {
        read: true,
        write: true,
        exec: true,
        user: false,
        device: false,
    };
    let before = m.counts();
    assert_eq!(m.map(vs, VA, fr, wx).unwrap_err(), ObjError::WxViolation);
    assert!(!m.walk(vs, VA).unwrap().valid, "không được ghi PTE");
    assert_eq!(m.counts(), before, "atomic: không mapping/không đổi đếm");
    assert_eq!(m.frame_counts(fr).unwrap(), (0, 0, 0));
    // Protect sang W|X cũng bị từ chối, PTE cũ giữ nguyên.
    m.map(vs, VA, fr, MapFlags::RX_USER).expect("map RX");
    assert_eq!(m.protect(vs, VA, wx).unwrap_err(), ObjError::WxViolation);
    let raw = m.walk(vs, VA).unwrap().raw;
    assert_ne!(raw & (1 << 3), 0, "X còn (Sv39: X = bit 3)");
    assert_eq!(raw & (1 << 2), 0, "W không được bật (Sv39: W = bit 2)");
    assert_eq!(m.frame_counts(fr).unwrap(), (1, 0, 1));
    m.check_objmem().expect("W^X toàn cục vẫn đạt");
    m.revoke(vs).unwrap();
    m.revoke(fr).unwrap();
}

/// KT-56 (nợ L3 KT-52) — bất biến **Device toàn cục** của D2-67: một Frame đang là Device ở
/// mapping này **không** được trở thành executable ở mapping khác, và ngược lại; bộ đếm
/// `mapped_device` phải đối xứng qua map/protect/unmap/revoke và `check_objmem` phải bắt.
#[test]
fn device_toan_cuc_khong_lan_exec_theo_frame() {
    let (mut m, u0, _u1) = machine();
    let vs0 = m.retype(u0, ObjKind::VSpace, 0).expect("vs0");
    let vs1 = m.retype(u0, ObjKind::VSpace, 0).expect("vs1");
    let fr = m.retype(u0, ObjKind::Frame, 12).expect("frame");
    m.map(vs0, VA, fr, MapFlags::DEVICE_RW)
        .expect("map Device RW");
    assert_eq!(m.frame_device(fr).unwrap(), 1);
    // Frame đã Device ⇒ RX ở VSpace **khác** bị từ chối, không ghi PTE.
    let err = m.map(vs1, VA + 0x1000, fr, MapFlags::RX).unwrap_err();
    assert_eq!(err, ObjError::WxViolation);
    assert!(!m.walk(vs1, VA + 0x1000).unwrap().valid);
    assert_eq!(m.frame_device(fr).unwrap(), 1);
    // Protect mapping Device sang Device+EXEC cũng bị từ chối, PTE cũ giữ nguyên.
    let bad = MapFlags {
        read: true,
        write: false,
        exec: true,
        user: false,
        device: true,
    };
    assert_eq!(m.protect(vs0, VA, bad).unwrap_err(), ObjError::WxViolation);
    assert_eq!(m.frame_counts(fr).unwrap(), (1, 1, 0));
    m.check_objmem().expect("Device toàn cục vẫn đạt");
    m.unmap(vs0, VA).expect("unmap");
    assert_eq!(m.frame_device(fr).unwrap(), 0, "đếm Device trả về 0");

    // Chiều ngược: RX trước rồi Device sau ⇒ WxViolation.
    m.map(vs1, VA + 0x1000, fr, MapFlags::RX).expect("map RX");
    assert_eq!(
        m.map(vs0, VA, fr, MapFlags::DEVICE_RW).unwrap_err(),
        ObjError::WxViolation
    );
    assert_eq!(m.frame_device(fr).unwrap(), 0);
    m.revoke(vs0).unwrap();
    m.revoke(vs1).unwrap();
    m.revoke(fr).unwrap();
    m.check_objmem().expect("dọn sạch");
}

// -------------------------------------------------------------------------------------------
// **KT-417d** — `PageTable.{Map,Unmap}` (D2-262 nhóm 4, ABI `PageTable` id 10 method 0/1)
// -------------------------------------------------------------------------------------------

/// `PageTable.Map` **cài bảng trang do cap `pt` sở hữu** vào `VSpace` tại `va` rồi map `frame`
/// **bằng chính bảng đó**; `PageTable.Unmap` gỡ đúng lá ấy. Đây là đường mở cho pager userland
/// sở hữu bảng cấp Chặng 3 — khác `Frame.Map` (VSpace tự cấp bảng).
#[test]
fn page_table_map_unmap_qua_bang_so_huu() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let pt = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let pa = m.frame_pa(fr).unwrap();
    // Chưa có bảng mức lá: PageTable.Map phải cài `pt` rồi map Frame qua nó.
    assert!(!m.walk(vs, VA).unwrap().valid);
    m.map_page_table(pt, vs, VA, fr, MapFlags::RW_NX_USER)
        .expect("PageTable.Map");
    let w = m.walk(vs, VA).unwrap();
    assert!(w.valid && w.leaf, "PTE lá phải có sau PageTable.Map");
    assert_eq!(w.level, 0);
    assert_eq!(w.phys, pa);
    // Bảng đã thuộc VSpace ⇒ revoke trực tiếp bị `HasChildren` (không giải phóng byte).
    assert_eq!(m.revoke(pt).unwrap_err(), ObjError::HasChildren);
    // Map trùng VA qua đường thường ⇒ AlreadyMapped (bất biến giữ nguyên).
    assert_eq!(
        m.map(vs, VA, fr, MapFlags::RW_NX_USER).unwrap_err(),
        ObjError::AlreadyMapped
    );
    // VA thứ hai trong cùng vùng 2 MiB dùng lại bảng trung gian vừa cấp.
    let fr2 = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let ro = MapFlags {
        read: true,
        write: false,
        exec: false,
        user: true,
        device: false,
    };
    m.map(vs, VA + 0x1000, fr2, ro).expect("map VA+4K");
    assert_eq!(
        m.walk(vs, VA + 0x1000).unwrap().phys,
        m.frame_pa(fr2).unwrap()
    );
    // Unmap **qua bảng**: chỉ gỡ lá ở `VA`, không đụng `VA+4K`.
    m.unmap_page_table(pt, vs, VA).unwrap();
    assert!(!m.walk(vs, VA).unwrap().valid);
    assert!(m.walk(vs, VA + 0x1000).unwrap().valid);
    assert_eq!(
        m.unmap_page_table(pt, vs, VA).unwrap_err(),
        ObjError::NotMapped
    );
    m.check_objmem().unwrap();
    // Teardown VSpace: trả root + bảng trung gian + bảng `pt` (owner) về Untyped.
    m.revoke(vs).unwrap();
    assert_eq!(m.untyped_stats(u0).unwrap().used, 0, "không rò byte arena");
    m.revoke(fr).unwrap();
    m.revoke(fr2).unwrap();
    m.check_objmem().unwrap();
}

/// Ca âm của `PageTable.{Map,Unmap}`: sai loại cap, `va` lệch căn, W^X, bảng ngoài VSpace.
#[test]
fn page_table_map_unmap_ca_am() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let pt = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let pt2 = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    // Cap Frame thay cho PageTable ⇒ sai loại.
    assert_eq!(
        m.map_page_table(fr, vs, VA, fr, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::WrongObjectType
    );
    // `va` không căn 4 KiB ⇒ Alignment (kiểm trước mọi thay đổi).
    assert_eq!(
        m.map_page_table(pt, vs, VA + 1, fr, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::Alignment
    );
    // W|X cùng lúc ⇒ WxViolation.
    let wx = MapFlags {
        read: true,
        write: true,
        exec: true,
        user: false,
        device: false,
    };
    assert_eq!(
        m.map_page_table(pt, vs, VA, fr, wx).unwrap_err(),
        ObjError::WxViolation
    );
    // Bảng chưa cài vào VSpace ⇒ Unmap `InvalidCapability` (không im lặng).
    assert_eq!(
        m.unmap_page_table(pt2, vs, VA).unwrap_err(),
        ObjError::InvalidCapability
    );
    // Vẫn còn nguyên: chưa map gì, byte chưa rò.
    assert!(!m.walk(vs, VA).unwrap().valid);
    m.check_objmem().unwrap();
}

/// **F2 (review W8a)** — `PageTable.Map` chỉ nhận lá **4 KiB**: `frame` cỡ khác (2 MiB/1 GiB) phải
/// bị từ chối **ngay** bằng [`ObjError::NotSupported`] (mã ABI 18) **trước** mọi thay đổi — không
/// cài bảng lá, không cấp bảng mức 1, không để trạng thái nửa vời.
///
/// Trước bản vá: `install_leaf_table` cài bảng + cấp bảng mức 1 **trước**, rồi `map` tính
/// `level = 1` và ghi PTE siêu trang vào đúng ô vừa đặt `leaf_pte` ⇒ gặp `pte_load != 0` ⇒
/// `Corrupt` (qua cầu ABI rò thành 5) và `PageTable.Unmap` cùng bộ cap trả `NotMapped` — tức
/// hợp đồng "Map rồi Unmap cùng bộ cap" **không** đối xứng.
#[test]
fn page_table_map_tu_choi_frame_khong_phai_4kib() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let pt = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let fr2m = m.retype(u0, ObjKind::Frame, 21).unwrap();
    assert_eq!(m.frame_size_bits(fr2m).unwrap(), 21, "Frame 2 MiB");
    let before = m.untyped_stats(u0).unwrap();
    let tables_before = m.counts().page_tables;
    assert_eq!(
        m.map_page_table(pt, vs, VA, fr2m, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::NotSupported,
        "Frame 2 MiB phải bị từ chối bằng NotSupported (18), không phải Corrupt"
    );
    // Không thay đổi gì: byte arena (không cấp bảng mức 1), số bảng trang, PTE.
    assert_eq!(m.untyped_stats(u0).unwrap(), before, "không cấp bảng mức 1");
    assert_eq!(m.counts().page_tables, tables_before, "không cài/cấp bảng");
    assert!(!m.walk(vs, VA).unwrap().valid, "không được để PTE dở");
    // Đối xứng: `Unmap` cùng bộ cap bị từ chối **ngay** (bảng chưa từng được cài) và vẫn không
    // để lại trạng thái nửa vời.
    assert_eq!(
        m.unmap_page_table(pt, vs, VA).unwrap_err(),
        ObjError::InvalidCapability
    );
    assert_eq!(m.untyped_stats(u0).unwrap(), before);
    assert_eq!(m.counts().page_tables, tables_before);
    assert!(!m.walk(vs, VA).unwrap().valid);
    m.check_objmem().unwrap();
    m.revoke(vs).unwrap();
    m.revoke(pt).unwrap();
    m.revoke(fr2m).unwrap();
    m.check_objmem().unwrap();
}

/// **KT-417d** — `PageTable.Map` **atomic**: khi `map` hỏng *sau* bước cài bảng (hết slot
/// mapping), bảng vừa cài + bảng mức 1 vừa cấp phải được **tháo sạch** (byte arena nguyên vẹn,
/// `revoke(pt)` còn được ⇒ bảng trở lại "bảng rời").
#[test]
fn page_table_map_hong_sau_cai_thi_thao_sach() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let pt = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    // Lấp **hết** slot mapping (64) bằng lá 4 KiB trong cùng một vùng 2 MiB.
    let mut frames = Vec::new();
    let mut i = 0u64;
    while i < 64 {
        let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
        m.map(vs, VA + i * 0x1000, fr, MapFlags::RW_NX_USER)
            .expect("map lá");
        // Xả flush mỗi vòng: bảng flush chỉ 16 slot, không xả thì `NoSlots` vì flush chứ không
        // phải vì slot mapping.
        let _ = m.drain_flushes(|_| {});
        frames.push(fr);
        i += 1;
    }
    let used_before = m.untyped_stats(u0).unwrap().used;
    // Vùng 2 MiB khác: PageTable.Map phải cài bảng rồi mới map — `free_mapping_slot` hết ⇒ hỏng.
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let far = VA + 0x20_0000;
    assert_eq!(
        m.map_page_table(pt, vs, far, fr, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::NoSlots
    );
    // Rollback: không byte mới, không PTE, bảng `pt` vẫn là bảng rời revoke được.
    assert_eq!(
        m.untyped_stats(u0).unwrap().used,
        used_before,
        "bảng mức 1 vừa cấp phải được trả lại"
    );
    assert!(!m.walk(vs, far).unwrap().valid, "không được để PTE dở");
    // Rollback trả `pt` về **bảng rời**: revoke được (và trả 4 KiB về Untyped).
    let before_revoke = m.untyped_stats(u0).unwrap().used;
    m.revoke(pt).expect("revoke bảng rời sau rollback");
    assert_eq!(m.untyped_stats(u0).unwrap().used, before_revoke - 4096);
    // Không còn bảng mức 1 mồ côi ⇒ `check_objmem` xanh (orphan = 0).
    m.check_objmem().unwrap();
}

/// **KT-417d** — bảng do **Untyped khác** cấp vẫn cài được; teardown VSpace trả byte về **đúng**
/// Untyped của bảng (không rò/không trả nhầm).
#[test]
fn page_table_map_bang_tu_untyped_khac_tra_dung_chu() {
    let (mut m, u0, u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    // `pt` lấy từ U1 (arena 4 KiB riêng) — chủ sở hữu bảng là caller, không phải VSpace.
    let pt = m.retype(u1, ObjKind::PageTable, 12).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    let pa = m.frame_pa(fr).unwrap();
    m.map_page_table(pt, vs, VA, fr, MapFlags::RW_NX_USER)
        .expect("cài bảng từ U1");
    let w = m.walk(vs, VA).unwrap();
    assert!(w.valid && w.leaf);
    assert_eq!(w.phys, pa);
    m.check_objmem().unwrap();
    // Teardown: bảng mức 1 (U0) về U0, bảng lá `pt` (U1) về U1.
    m.revoke(vs).unwrap();
    assert_eq!(
        m.untyped_stats(u0).unwrap().used,
        0,
        "U0 sạch (root + bảng mức 1)"
    );
    assert_eq!(m.untyped_stats(u1).unwrap().used, 0, "U1 sạch (bảng lá)");
    m.revoke(fr).unwrap();
    m.check_objmem().unwrap();
}

/// **KT-417d** — `PageTable.Map` lần hai vào **cùng** VA với bảng **đã cài** phải đi tiếp đường
/// `map` (`AlreadyMapped`), và `protect` trên mapping do `PageTable.Map` tạo phải đổi được PTE.
#[test]
fn page_table_map_lan_hai_va_protect() {
    let (mut m, u0, _u1) = machine();
    let vs = m.retype(u0, ObjKind::VSpace, 0).unwrap();
    let pt = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    let fr = m.retype(u0, ObjKind::Frame, 12).unwrap();
    m.map_page_table(pt, vs, VA, fr, MapFlags::RW_NX_USER)
        .expect("PageTable.Map");
    // Cùng bảng, cùng VA ⇒ đã map.
    assert_eq!(
        m.map_page_table(pt, vs, VA, fr, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::AlreadyMapped
    );
    // Bảng thứ hai cài vào VA đã có bảng lá khác ⇒ AlreadyMapped (không ghi đè bảng).
    let pt2 = m.retype(u0, ObjKind::PageTable, 12).unwrap();
    assert_eq!(
        m.map_page_table(pt2, vs, VA + 0x1000, fr, MapFlags::RW_NX_USER)
            .unwrap_err(),
        ObjError::AlreadyMapped
    );
    m.revoke(pt2).unwrap();
    // `protect` trên mapping vừa tạo: W → R (PTE lá trong bảng của caller).
    let ro = MapFlags {
        read: true,
        write: false,
        exec: false,
        user: true,
        device: false,
    };
    m.protect(vs, VA, ro).expect("protect R");
    let w = m.walk(vs, VA).unwrap();
    assert!(ro.matches_pte(w.raw));
    assert_eq!(m.unmap_page_table(pt, vs, VA), Ok(()));
    m.check_objmem().unwrap();
    m.revoke(vs).unwrap();
    m.revoke(fr).unwrap();
    m.check_objmem().unwrap();
}
