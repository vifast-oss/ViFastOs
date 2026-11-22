// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-336b** — test nội bộ của `kobj` (tách nguyên văn khỏi `kobj.rs`; `#[cfg(test)] mod tests;`).

use super::*;

#[test]
fn ma_loi_lay_dung_tu_spec() {
    assert_eq!(KError::InvalidCapability.name(), "InvalidCapability");
    assert_eq!(KError::InvalidCapability.id(), 1);
    assert_eq!(KError::Alignment.id(), 6);
    assert_eq!(KError::NotEnoughMemory.id(), 9);
    assert_eq!(KError::NotSupported.id(), 18);
    assert_eq!(KError::RangeError.id(), 19);
    for (i, (name, id)) in vifast_abi::ERRORS.iter().enumerate() {
        let k = [
            KError::InvalidCapability,
            KError::WrongObjectType,
            KError::InsufficientRights,
            KError::WxViolation,
            KError::InvalidArgument,
            KError::Alignment,
            KError::SlotOccupied,
            KError::SlotEmpty,
            KError::NotEnoughMemory,
            KError::AlreadyMapped,
            KError::NotMapped,
            KError::MissingPageTable,
            KError::HasChildren,
            KError::Busy,
            KError::Cancelled,
            KError::Timeout,
            KError::IllegalOperation,
            KError::NotSupported,
            KError::RangeError,
        ][i];
        assert_eq!(k.name(), *name, "thứ tự mã lỗi lệch spec ở {i}");
        assert_eq!(k.id(), *id);
    }
}

#[test]
fn ten_object_khop_spec() {
    for (name, _id) in vifast_abi::CORE_OBJECTS {
        assert!(ObjType::from_name(name).is_some(), "thiếu {name}");
    }
    for (name, _id) in vifast_abi::RESERVED_OBJECTS {
        let t = ObjType::from_name(name).expect("reserved phải có tên");
        assert!(!t.is_core(), "{name} phải là reserved");
    }
    assert_eq!(ObjType::from_name("KhongCo"), None);
}

#[test]
fn tcb_state_du_11_ten_theo_spec() {
    use std::collections::BTreeSet;
    let all = [
        TcbState::Inactive,
        TcbState::Ready,
        TcbState::Running { hart: 0 },
        TcbState::BlockedOnSend {
            ep: 1,
            call: true,
            fault: None,
        },
        TcbState::BlockedOnRecv { ep: 1, reply: None },
        TcbState::BlockedOnReply { reply: 1 },
        TcbState::BlockedOnNotification { ntfn: 1 },
        TcbState::BlockedOnAddress {
            frame: 1,
            offset: 16,
        },
        TcbState::Sleeping {
            reason: SleepReason::Timeout,
        },
        TcbState::Stopping,
        TcbState::Dead,
    ];
    let names: BTreeSet<&str> = all.iter().map(TcbState::name).collect();
    let want: BTreeSet<&str> = vifast_abi::TCB_STATES.iter().copied().collect();
    assert_eq!(names, want, "TcbState phải đủ 11 tên của spec D2-33");
    assert_eq!(all.len(), 11);
    assert_eq!(TcbState::Running { hart: 2 }.render(), "Running{hart=2}");
    assert_eq!(
        TcbState::BlockedOnNotification { ntfn: 5 }.render(),
        "BlockedOnNotification{ntfn=5}"
    );
    assert_eq!(
        TcbState::BlockedOnAddress {
            frame: 3,
            offset: 16
        }
        .render(),
        "BlockedOnAddress{frame=3, offset=16}"
    );
    assert_eq!(
        TcbState::Sleeping {
            reason: SleepReason::Budget
        }
        .render(),
        "Sleeping{reason=Budget}"
    );
    assert_eq!(TcbState::Stopping.render(), "Stopping");
    assert!(!TcbState::Stopping.is_runnable());
    assert!(TcbState::Running { hart: 0 }.is_runnable());
}

/// Công thức căn lề cũ — giữ làm **mốc so** cho phần đã chuyển vào arena objmem
/// (KT-16: `Arena::alloc` phải cho cùng kết quả alignment này).
fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

#[test]
fn can_le_va_can_duoi() {
    assert_eq!(align_up(0, 16), 0);
    assert_eq!(align_up(1, 16), 16);
    assert_eq!(align_up(512, 4096), 4096);
    assert_eq!(align_up(4096, 4096), 4096);
}

/// `check_caps` không được là checker rỗng: gieo hỏng cây thì phải bắt.
#[test]
fn check_caps_bat_cap_mo_coi() {
    let mut m = Machine::new();
    let a = m.add_cnode(4);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let root = m.root_cap(ep);
    m.put_cap(a, 0, root).expect("cap gốc");
    let child = m
        .copy(SlotRef::new(a, 0), SlotRef::new(a, 1))
        .expect("cap con");
    m.check_caps().expect("cây hợp lệ");
    // Cha bị xoá (mất slot, chết) nhưng con **không được reparent** — đúng lỗi cũ
    // của `delete`; checker phải bắt cap mồ côi.
    m.caps[root as usize - 1].alive = false;
    m.caps[root as usize - 1].slot = None;
    m.cnode_set_slot(a, 0, None).expect("CNode");
    let err = m.check_caps().expect_err("phải bắt cap mồ côi");
    assert!(err.contains("mồ côi"), "{err}");
    assert!(m.cap_info(child).expect("con").alive);
}

/// Slot trỏ cap nhưng cap không trỏ ngược (MDB hỏng) phải bị bắt.
#[test]
fn check_caps_bat_slot_khong_doi_xung() {
    let mut m = Machine::new();
    let a = m.add_cnode(4);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let cap = m.root_cap(ep);
    m.put_cap(a, 0, cap).expect("cap gốc");
    m.caps[cap as usize - 1].slot = None;
    let err = m.check_caps().expect_err("phải bắt slot không đối xứng");
    assert!(
        err.contains("không trỏ ngược") || err.contains("không trỏ lại"),
        "{err}"
    );
    // Cap chết còn giữ slot cũng là hỏng.
    let mut m2 = Machine::new();
    let b = m2.add_cnode(4);
    let ep2 = m2.add_kernel_object(ObjType::Endpoint);
    let cap2 = m2.root_cap(ep2);
    m2.put_cap(b, 0, cap2).expect("cap gốc");
    m2.caps[cap2 as usize - 1].alive = false;
    let err = m2.check_caps().expect_err("phải bắt cap chết còn giữ slot");
    assert!(err.contains("cap chết"), "{err}");
}

/// **KT-28** — `check_waitaddr` không được là checker rỗng: nút nằm **sai bucket** phải bị
/// bắt (va chạm bucket là hợp lệ, sai bucket là hỏng).
#[test]
fn check_waitaddr_bat_sai_bucket() {
    let mut m = Machine::new();
    let tcb = m.add_kernel_object(ObjType::Tcb);
    m.start_tcb(tcb).expect("Ready");
    m.transition_tcb(
        tcb,
        NonReadyTarget::BlockedOnAddress {
            frame: 5,
            offset: 8,
        },
    )
    .expect("BlockedOnAddress");
    let dung = waitaddr_bucket(5, 8);
    let sai = (dung + 1) % WAITADDR_BUCKETS;
    m.address_waiters[sai].push((5, 8, tcb));
    let e = m.check_waitaddr().expect_err("phải bắt sai bucket");
    assert!(e.contains("bucket"), "{e}");
    // Về đúng bucket ⇒ sạch.
    m.address_waiters[sai].clear();
    m.address_waiters[dung].push((5, 8, tcb));
    m.check_waitaddr().expect("đúng bucket phải sạch");
    // TCB bị treo mà vẫn còn nút cũng hỏng (§4.4: hàng chỉ chứa TCB `!suspended`).
    m.set_tcb_suspended(tcb, true).expect("bật suspended");
    let e = m
        .check_waitaddr()
        .expect_err("phải bắt waiter của TCB suspended");
    assert!(e.contains("treo") || e.contains("suspended"), "{e}");
}

/// **KT-28** — trùng nút và nút của TCB **đã chết** phải bị bắt.
#[test]
fn check_waitaddr_bat_trung_va_tcb_chet() {
    let mut m = Machine::new();
    let tcb = m.add_kernel_object(ObjType::Tcb);
    m.start_tcb(tcb).expect("Ready");
    m.transition_tcb(
        tcb,
        NonReadyTarget::BlockedOnAddress {
            frame: 9,
            offset: 16,
        },
    )
    .expect("BlockedOnAddress");
    let b = waitaddr_bucket(9, 16);
    m.address_waiters[b].push((9, 16, tcb));
    m.address_waiters[b].push((9, 16, tcb));
    let e = m.check_waitaddr().expect_err("phải bắt trùng");
    assert!(e.contains("trùng") || e.contains("nhiều hơn một"), "{e}");
    // TCB chết: `finalise_tcb` gỡ nút đúng đường; gieo lại nút chết để kiểm checker.
    m.address_waiters[b].clear();
    m.finalise_tcb(tcb).expect("finalise TCB");
    m.address_waiters[b].push((9, 16, tcb));
    let e = m
        .check_waitaddr()
        .expect_err("phải bắt waiter của TCB đã chết");
    assert!(e.contains("chết") || e.contains("không tồn tại"), "{e}");
}

/// **KT-29** — `check_smp` không là checker rỗng: `published` lệch trạng thái thật, TCB
/// nằm trong hàng **sai hart**, và yêu cầu `Migrate` sai (home == đích) đều bị bắt.
#[test]
fn check_smp_bat_published_lech_va_migrate_sai() {
    // (a) published.idle lệch `current == None`.
    let mut m = Machine::with_harts(2);
    let tcb = m.add_kernel_object(ObjType::Tcb);
    m.start_tcb(tcb).expect("Ready");
    m.check_smp().expect("mô hình sạch");
    m.published[0].idle = false;
    let e = m.check_smp().expect_err("phải bắt published lệch");
    assert!(e.contains("published"), "{e}");
    m.published[0].idle = true;
    m.check_smp().expect("khôi phục thì sạch");

    // (b) TCB Ready home 0 bị đặt thêm vào hàng hart 1.
    m.sched[1].enqueue(tcb, 0);
    let e = m.check_smp().expect_err("phải bắt hàng sai hart");
    assert!(e.contains("hart") || e.contains("hàng"), "{e}");
    m.sched[1].remove_all(tcb);
    m.check_smp().expect("gỡ bản sao thì sạch");

    // (c) yêu cầu Migrate có `to_hart == home` (không di trú được) trong hộp thư hart 0.
    m.mail[0]
        .try_push(MailboxMsg::Migrate { tcb, to_hart: 0 })
        .expect("hộp thư còn chỗ");
    let e = m.check_smp().expect_err("phải bắt Migrate home == to_hart");
    assert!(e.contains("home == to_hart"), "{e}");
    assert!(matches!(m.mail[0].pop(), Some(MailboxMsg::Migrate { .. })));
    m.check_smp().expect("gỡ yêu cầu sai thì sạch");
}

// ------------------------------------------------ KT-36: check_frames phải **bắt** lỗi

/// Dựng rig `(machine, cnode, untyped, src)` như test tích hợp: Untyped 4096 B, `base = 0`.
fn rig_frame() -> (Machine, ObjId, ObjId, SlotRef) {
    let mut m = Machine::new();
    let c = m.add_cnode(8);
    let u = m.add_untyped(0, 4096);
    let cap = m.root_cap(u);
    m.put_cap(c, 0, cap).expect("cap Untyped");
    (m, c, u, SlotRef::new(c, 0))
}

/// `check_frames` không được là checker rỗng: Frame mồ côi (Untyped sở hữu chết) bị bắt.
#[test]
fn check_frames_bat_frame_mo_coi() {
    let (mut m, c, u, src) = rig_frame();
    let cap = m.retype(src, ObjType::Frame, 12, 0).expect("Frame 4 KiB");
    m.put_cap(c, 1, cap).expect("đặt cap Frame");
    m.check_frames().expect("mô hình sạch");
    // Chủ sở hữu chết mà Frame còn sống ⇒ mồ côi (đường duy nhất tạo ra được là gieo hỏng
    // sổ cái, vì `Revoke` Untyped luôn finalise Frame trong cây).
    m.objects[u as usize - 1].alive = false;
    let e = m.check_frames().expect_err("phải bắt Frame mồ côi");
    assert!(e.contains("mồ côi"), "{e}");
    m.objects[u as usize - 1].alive = true;
    m.check_frames().expect("khôi phục thì sạch");
}

/// Frame **đã chết** còn bộ đếm mapping khác 0 ⇒ bất biến "frame chết sạch" bị bắt.
#[test]
fn check_frames_bat_frame_chet_con_mapping() {
    let (mut m, c, _u, src) = rig_frame();
    let cap = m.retype(src, ObjType::Frame, 12, 0).expect("Frame");
    let f = m.cap_info(cap).expect("cap").obj;
    m.put_cap(c, 1, cap).expect("đặt cap");
    m.revoke(c, Some(0)).expect("revoke Untyped");
    m.check_frames().expect("Frame chết sạch");
    // Gieo hỏng: Frame đã chết mà còn 1 mapping (đúng lỗi mà KT-43 sẽ đột biến).
    m.objects[f as usize - 1].frame.as_mut().expect("meta").maps = 1;
    let e = m
        .check_frames()
        .expect_err("phải bắt frame chết còn mapping");
    assert!(e.contains("còn mapping"), "{e}");
    m.objects[f as usize - 1].frame.as_mut().expect("meta").maps = 0;
    m.check_frames().expect("khôi phục thì sạch");
}

/// `pa` sai căn / `pa` ngoài cửa sổ / watermark sai đều bị bắt.
#[test]
fn check_frames_bat_pa_va_watermark_sai() {
    let (mut m, _c, u, src) = rig_frame();
    let cap = m.retype(src, ObjType::Frame, 12, 0).expect("Frame");
    let f = m.cap_info(cap).expect("cap").obj;
    m.check_frames().expect("mô hình sạch");

    // (a) pa lệch căn.
    m.objects[f as usize - 1].frame.as_mut().expect("meta").pa = 1;
    let e = m.check_frames().expect_err("phải bắt pa lệch căn");
    assert!(e.contains("lệch căn"), "{e}");
    m.objects[f as usize - 1].frame.as_mut().expect("meta").pa = 0;
    m.check_frames().expect("khôi phục thì sạch");

    // (b) pa ra ngoài cửa sổ 1 GiB (`pa` căn đúng nhưng `offset`/watermark vẫn khớp
    //     ⇒ lỗi phải là "ra ngoài cửa sổ", không phải lỗi khác).
    let out = 2 * UNTYPED_PA_WINDOW;
    m.objects[f as usize - 1].frame.as_mut().expect("meta").pa = out;
    m.objects[f as usize - 1].offset = out as usize;
    m.objects[u as usize - 1]
        .frame_area
        .as_mut()
        .expect("cửa sổ")
        .watermark = out;
    let e = m.check_frames().expect_err("phải bắt pa ngoài cửa sổ");
    assert!(e.contains("cửa sổ"), "{e}");
    m.objects[f as usize - 1].frame.as_mut().expect("meta").pa = 0;
    m.objects[f as usize - 1].offset = 0;
    m.objects[u as usize - 1]
        .frame_area
        .as_mut()
        .expect("cửa sổ")
        .watermark = 4096;
    m.check_frames().expect("khôi phục thì sạch");

    // (c) watermark cửa sổ của Untyped không khớp Frame sống.
    m.objects[u as usize - 1]
        .frame_area
        .as_mut()
        .expect("cửa sổ")
        .watermark = 8192;
    let e = m.check_frames().expect_err("phải bắt watermark sai");
    assert!(e.contains("watermark"), "{e}");
    m.objects[u as usize - 1]
        .frame_area
        .as_mut()
        .expect("cửa sổ")
        .watermark = 4096;
    m.check_frames().expect("khôi phục thì sạch");
}

/// Hai Frame sống chồng `pa` (khác Untyped) bị bắt; metadata Frame trên object không phải
/// Frame cũng bị bắt.
#[test]
fn check_frames_bat_chong_pa_va_metadata_thua() {
    let (mut m, c, _u, src) = rig_frame();
    let cap = m.retype(src, ObjType::Frame, 12, 0).expect("Frame 4 KiB");
    let f = m.cap_info(cap).expect("cap").obj;
    m.put_cap(c, 1, cap).expect("đặt cap");
    // Untyped thứ hai ở **cửa sổ kế** (base = 1 GiB) ⇒ cấp được; rồi gieo `pa` chồng nhau.
    let u2 = m.add_untyped(UNTYPED_PA_WINDOW, 4096);
    let cap2 = m.root_cap(u2);
    m.put_cap(c, 2, cap2).expect("cap Untyped 2");
    let cap2 = m
        .retype(SlotRef::new(c, 2), ObjType::Frame, 12, 0)
        .expect("Frame #2");
    let g = m.cap_info(cap2).expect("cap").obj;
    m.put_cap(c, 3, cap2).expect("đặt cap Frame 2");
    m.check_frames().expect("hai cửa sổ rời nhau ⇒ sạch");
    // Gieo hỏng: kéo **cả cửa sổ** của Untyped 2 về `base = 0` (khớp `pa = base + offset`
    // để lỗi phải là "chồng pa", không phải lỗi khác) ⇒ Frame #2 chồng Frame #1.
    let area2 = m.objects[u2 as usize - 1]
        .frame_area
        .as_mut()
        .expect("cửa sổ");
    area2.base = 0;
    area2.watermark = 4096;
    m.objects[g as usize - 1].frame.as_mut().expect("meta").pa = 0;
    m.objects[g as usize - 1].offset = 0;
    let e = m.check_frames().expect_err("phải bắt hai Frame chồng pa");
    assert!(e.contains("chồng"), "{e}");
    let _ = f;
    // Khôi phục: Frame #2 về đúng cửa sổ của nó.
    let area2 = m.objects[u2 as usize - 1]
        .frame_area
        .as_mut()
        .expect("cửa sổ");
    area2.base = UNTYPED_PA_WINDOW;
    area2.watermark = 4096;
    m.objects[g as usize - 1].frame.as_mut().expect("meta").pa = UNTYPED_PA_WINDOW;
    m.objects[g as usize - 1].offset = 0;
    m.check_frames().expect("khôi phục thì sạch");

    // Metadata Frame gắn nhầm vào object không phải Frame.
    let cn = m.add_cnode(2);
    m.objects[cn as usize - 1].frame = Some(FrameMeta::new(0, FrameSize::K4));
    let e = m
        .check_frames()
        .expect_err("phải bắt metadata Frame trên CNode");
    assert!(e.contains("không phải Frame"), "{e}");
    m.objects[cn as usize - 1].frame = None;
    m.check_frames().expect("gỡ metadata thừa thì sạch");
}

/// `FrameArea::carve` — đơn vị căn lề: lệch căn ⇒ `Alignment` **không** đổi watermark; cửa
/// sổ đầy ⇒ `NotEnoughMemory`; cấp giảm dần (2 MiB → 4 KiB) thì đúng căn tự nhiên.
#[test]
fn frame_area_carve_bien_can_le_va_day_cua_so() {
    let mut a = FrameArea::new(1);
    assert_eq!(a.carve(FrameSize::K4), Err(KError::Alignment));
    assert_eq!(a.watermark, 0, "Alignment không đổi watermark");

    let mut a = FrameArea::new(0);
    assert_eq!(a.carve(FrameSize::G1), Ok(0));
    assert_eq!(a.carve(FrameSize::M2), Err(KError::NotEnoughMemory));
    assert_eq!(a.watermark, UNTYPED_PA_WINDOW, "cửa sổ đã đầy");

    let mut a = FrameArea::new(0);
    assert_eq!(a.carve(FrameSize::M2), Ok(0));
    assert_eq!(a.carve(FrameSize::K4), Ok(1 << 21));
    assert_eq!(a.carve(FrameSize::K4), Ok((1 << 21) + (1 << 12)));
    assert_eq!(a.window(), Some((0, UNTYPED_PA_WINDOW)));

    // Cửa sổ gần trần `u64`: `pa + cỡ` tràn ⇒ `NotEnoughMemory`.
    let mut a = FrameArea::new(u64::MAX - 4095);
    assert_eq!(a.carve(FrameSize::K4), Err(KError::NotEnoughMemory));
    // `base + UNTYPED_PA_WINDOW` tràn ⇒ cửa sổ không hợp lệ ⇒ `InvalidArgument`.
    let mut a = FrameArea::new(u64::MAX - 8191);
    assert_eq!(a.carve(FrameSize::K4), Err(KError::InvalidArgument));
}

// ------------------------------------------------ KT-37: check_pages phải **bắt** lỗi

/// Retype một PageTable từ cap Untyped ở `src`.
fn mk_page_table(m: &mut Machine, src: SlotRef) -> ObjId {
    let cap = m
        .retype(src, ObjType::PageTable, 0, 0)
        .expect("retype PageTable");
    m.cap_info(cap).expect("cap").obj
}

/// Rig PageTable tối thiểu: root → mid → leaf, một Frame 4 KiB map ở `0x1000`.
fn rig_pages() -> (Machine, ObjId, ObjId, ObjId, ObjId, ObjId, SlotRef) {
    let mut m = Machine::new();
    let c = m.add_cnode(16);
    let u = m.add_untyped(0, 64 * 1024);
    let uf = m.add_untyped(1 << 32, 4096);
    let cap = m.root_cap(u);
    m.put_cap(c, 0, cap).expect("cap Untyped PageTable");
    let capf = m.root_cap(uf);
    m.put_cap(c, 1, capf).expect("cap Untyped Frame");
    let src = SlotRef::new(c, 0);
    let root = mk_page_table(&mut m, src);
    let mid = mk_page_table(&mut m, src);
    m.page_table_attach(root, 0, mid).expect("attach mid");
    let leaf = mk_page_table(&mut m, src);
    m.page_table_attach(mid, 0, leaf).expect("attach leaf");
    let capf = m
        .retype(SlotRef::new(c, 1), ObjType::Frame, 12, 0)
        .expect("retype Frame");
    let frame = m.cap_info(capf).expect("cap").obj;
    m.page_table_map_obj(root, 0x1000, frame, MapRights::READ, MapAttrs::WB)
        .expect("map 4K");
    m.check_pages().expect("mô hình sạch");
    (m, c, root, mid, leaf, frame, src)
}

#[test]
fn check_pages_bat_dem_va_frame_counter_sai() {
    let (mut m, _c, root, _mid, _leaf, frame, _src) = rig_pages();
    let rslot = m.objects[root as usize - 1].pt.expect("pt root");
    m.page_tables[rslot].as_mut().expect("dữ liệu").mapped += 1;
    let e = m.check_pages().expect_err("phải bắt mapped sai");
    assert!(e.contains("mapped"), "{e}");
    m.page_tables[rslot].as_mut().expect("dữ liệu").mapped -= 1;
    m.check_pages().expect("khôi phục thì sạch");

    // Frame còn lá nhưng bộ đếm maps = 0 ⇒ bị bắt (check_pages là cửa thứ hai).
    m.objects[frame as usize - 1]
        .frame
        .as_mut()
        .expect("meta")
        .maps = 0;
    let e = m.check_pages().expect_err("phải bắt bộ đếm Frame sai");
    assert!(e.contains("bộ đếm"), "{e}");
    m.objects[frame as usize - 1]
        .frame
        .as_mut()
        .expect("meta")
        .maps = 1;
    m.check_pages().expect("khôi phục thì sạch");
}

#[test]
fn check_pages_bat_con_attach_sai_va_bang_chet_con_du_lieu() {
    let (mut m, _c, _root, mid, leaf, _frame, _src) = rig_pages();
    // Con trỏ `attached` sai chỉ số ⇒ bất biến cây bị bắt.
    let lslot = m.objects[leaf as usize - 1].pt.expect("pt leaf");
    m.page_tables[lslot].as_mut().expect("dữ liệu").attached = Some((mid, 1));
    let e = m.check_pages().expect_err("phải bắt attached sai");
    assert!(e.contains("attach") || e.contains("PTE"), "{e}");
    m.page_tables[lslot].as_mut().expect("dữ liệu").attached = Some((mid, 0));
    m.check_pages().expect("khôi phục thì sạch");

    // Object PageTable đã chết nhưng dữ liệu còn ⇒ bị bắt (không rò 4 KiB PTE).
    m.objects[leaf as usize - 1].alive = false;
    let e = m
        .check_pages()
        .expect_err("phải bắt PageTable chết còn dữ liệu");
    assert!(e.contains("đã chết"), "{e}");
    m.objects[leaf as usize - 1].alive = true;
    m.check_pages().expect("khôi phục thì sạch");
}

#[test]
fn check_pages_bat_pte_sai_cap_hoac_tro_nham() {
    let (mut m, c, _root, mid, leaf, _frame, _src) = rig_pages();
    let mslot = m.objects[mid as usize - 1].pt.expect("pt mid");
    // Đổi PTE của mid[0] thành trỏ nhầm bảng leaf ở **sai** chỉ số (leaf ở [0]).
    // Ghi đè PTE[mid][0] bằng một bảng khác chưa attach ⇒ bị bắt.
    let cap = m
        .retype(SlotRef::new(c, 0), ObjType::PageTable, 0, 0)
        .expect("PageTable lạ");
    let la = m.cap_info(cap).expect("cap").obj;
    m.page_tables[mslot]
        .as_mut()
        .expect("dữ liệu")
        .entries
        .set(0, table_pte(la))
        .expect("ghi PTE");
    let e = m
        .check_pages()
        .expect_err("phải bắt PTE trỏ bảng chưa attach");
    assert!(
        e.contains("PTE") || e.contains("attach") || e.contains("mồ côi"),
        "{e}"
    );
    // Khôi phục PTE đúng.
    m.page_tables[mslot]
        .as_mut()
        .expect("dữ liệu")
        .entries
        .set(0, table_pte(leaf))
        .expect("ghi PTE");
    m.check_pages().expect("khôi phục thì sạch");
}

#[test]
fn map_bat_pa_lech_can_alignment() {
    let (mut m, c, root, _mid, _leaf, _frame, _src) = rig_pages();
    // Frame thứ hai, gieo `pa` lệch 1 ⇒ map phải Alignment, không đổi cây/bộ đếm.
    let cap = m
        .retype(SlotRef::new(c, 1), ObjType::Frame, 12, 0)
        .expect("Frame 2");
    let f2 = m.cap_info(cap).expect("cap").obj;
    m.objects[f2 as usize - 1].frame.as_mut().expect("meta").pa += 1;
    assert_eq!(
        m.page_table_map_obj(root, 0x2000, f2, MapRights::READ, MapAttrs::WB),
        Err(KError::Alignment)
    );
    assert_eq!(m.objects[f2 as usize - 1].frame.expect("meta").maps, 0);
    m.objects[f2 as usize - 1].frame.as_mut().expect("meta").pa -= 1;
    m.check_pages().expect("khôi phục thì sạch");
}

// ------------------------------------------------ KT-38: check_vspace/check_asid phải **bắt** lỗi

/// Rig VSpace tối thiểu: CNode 16 + Untyped 64 KiB + **một** VSpace (root + ASID 0).
fn rig_vspace() -> (Machine, ObjId, ObjId, ObjId) {
    let mut m = Machine::new();
    let c = m.add_cnode(16);
    let u = m.add_untyped(0, 64 * 1024);
    let cap = m.root_cap(u);
    m.put_cap(c, 0, cap).expect("cap Untyped");
    // KT-38: VSpace tạo bằng **model API** (`Retype(VSpace)` vẫn NotSupported/reserved).
    let cap = m.vspace_create(SlotRef::new(c, 0)).expect("vspace_create");
    let vs = m.cap_info(cap).expect("cap").obj;
    let root = m.vspace_root(vs).expect("root");
    (m, c, vs, root)
}

#[test]
fn check_vspace_bat_active_on_ngoai_online_va_hart_vspace_lech() {
    let (mut m, _c, vs, _root) = rig_vspace();
    let slot = m.objects[vs as usize - 1].space.expect("space");
    // (1) `active_on` có bit hart không online (1 hart ⇒ chỉ bit 0 hợp lệ).
    m.vspaces[slot].as_mut().expect("dữ liệu").active_on = HartMask::from_bits(0b10);
    let e = m.check_vspace().expect_err("phải bắt active_on ⊄ online");
    assert!(e.contains("online"), "{e}");
    m.vspaces[slot].as_mut().expect("dữ liệu").active_on = HartMask::EMPTY;
    m.check_vspace().expect("khôi phục thì sạch");

    // (2) `hart_vspace[h] == Some(vs)` nhưng bit `active_on` **tắt** ⇒ lệch.
    m.hart_vspace[0] = Some(vs);
    let e = m
        .check_vspace()
        .expect_err("phải bắt lệch active_on/hart_vspace");
    assert!(e.contains("lệch") || e.contains("online"), "{e}");
    m.hart_vspace[0] = None;
    m.check_vspace().expect("khôi phục thì sạch");
}

#[test]
fn check_vspace_bat_vspace_chet_con_du_lieu_va_root_khong_phai_goc() {
    let (mut m, _c, vs, root) = rig_vspace();
    // (1) Object chết nhưng dữ liệu VSpace còn (rò metadata/asid/active_on).
    m.objects[vs as usize - 1].alive = false;
    let e = m
        .check_vspace()
        .expect_err("phải bắt VSpace chết còn dữ liệu");
    assert!(e.contains("đã chết") || e.contains("sống"), "{e}");
    m.objects[vs as usize - 1].alive = true;
    m.check_vspace().expect("khôi phục thì sạch");

    // (2) Root không còn là bảng gốc (bị attach) ⇒ bất biến "root = bảng gốc".
    let rslot = m.objects[root as usize - 1].pt.expect("pt root");
    m.page_tables[rslot].as_mut().expect("dữ liệu").attached = Some((vs, 0));
    let e = m.check_vspace().expect_err("phải bắt root đã attach");
    assert!(e.contains("bảng gốc") || e.contains("attach"), "{e}");
    m.page_tables[rslot].as_mut().expect("dữ liệu").attached = None;
    m.check_vspace().expect("khôi phục thì sạch");
}

#[test]
fn check_asid_bat_free_list_sai_va_asid_trung_the_he() {
    let (mut m, c, vs, _root) = rig_vspace();
    // (1) Free-list mất tính giảm dần nghiêm ngặt (ASID bị "trả lại" sai ⇒ trùng).
    let dup = *m.asid_free.last().expect("free-list còn ASID");
    m.asid_free.push(dup);
    let e = m.check_asid().expect_err("phải bắt free-list trùng");
    assert!(e.contains("giảm dần") || e.contains("≠"), "{e}");
    m.asid_free.pop();
    m.check_asid().expect("khôi phục thì sạch");

    // (2) Hai VSpace cùng `(generation, asid)` ⇒ trùng trong một thế hệ.
    let cap = m.vspace_create(SlotRef::new(c, 0)).expect("VSpace 2");
    let vs2 = m.cap_info(cap).expect("cap").obj;
    let s1 = m.objects[vs as usize - 1].space.expect("space 1");
    let s2 = m.objects[vs2 as usize - 1].space.expect("space 2");
    m.vspaces[s2].as_mut().expect("dữ liệu").asid = m.vspaces[s1].as_ref().expect("dữ liệu").asid;
    let e = m.check_asid().expect_err("phải bắt ASID trùng");
    assert!(e.contains("trùng"), "{e}");
    m.vspaces[s2].as_mut().expect("dữ liệu").asid = 1;
    m.check_asid().expect("khôi phục thì sạch");

    // (3) ASID của VSpace thế hệ hiện tại còn nằm trong free-list ⇒ bị bắt.
    let free_asid = *m.asid_free.last().expect("free-list còn ASID");
    m.vspaces[s1].as_mut().expect("dữ liệu").asid = free_asid;
    let e = m
        .check_asid()
        .expect_err("phải bắt ASID còn trong free-list");
    assert!(e.contains("free-list"), "{e}");
    m.vspaces[s1].as_mut().expect("dữ liệu").asid = 0;
    m.check_asid().expect("khôi phục thì sạch");
}

#[test]
fn check_asid_can_asid_tang_the_he_mot_lan_va_flush_gen() {
    // Ca cạn ASID **rẻ** cho Miri: gieo free-list còn đúng 1 slot (thay vì tạo 256 VSpace),
    // rồi kiểm `generation += 1` **một lần** + **một** lệnh flush `Generation`.
    let (mut m, c, _vs, _root) = rig_vspace();
    m.asid_free = vec![255];
    m.asid_issued = ASID_COUNT as u32 - 1;
    m.check_asid().expect("trạng thái gieo hợp lệ");
    let cap = m
        .vspace_create(SlotRef::new(c, 0))
        .expect("VSpace cuối thế hệ 0");
    let vs = m.cap_info(cap).expect("cap").obj;
    let info = m.vspace_info(vs).expect("info");
    assert_eq!((info.asid, info.generation), (255, 0));
    assert_eq!(m.asid_state().free, 0);
    assert_eq!(m.flush_count(), 0);

    let cap = m
        .vspace_create(SlotRef::new(c, 0))
        .expect("VSpace thế hệ 1");
    let vs2 = m.cap_info(cap).expect("cap").obj;
    let info = m.vspace_info(vs2).expect("info");
    assert_eq!(
        (info.asid, info.generation),
        (0, 1),
        "cạn ASID ⇒ generation + 1, free-list nạp lại từ 0"
    );
    assert_eq!(
        m.asid_state(),
        AsidState {
            generation: 1,
            free: ASID_COUNT - 1,
            issued: 1,
            exhaustions: 1,
        }
    );
    assert_eq!(m.flush_count(), 1, "flush toàn bộ **một lần** theo gen");
    let f = m.last_flush().expect("flush gen");
    assert_eq!(f.scope, FlushScope::Generation);
    assert_eq!(f.generation, 1);
    assert_eq!(f.asid, None);
    assert_eq!(f.harts, HartMask::from_bits(0b1));
    m.check_asid().expect("sau cạn");
    m.check_vspace().expect("sau cạn");
}
// ------------------------------------------------ KT-39: check_wx/wx_recount phải **bắt** lỗi

/// `check_wx` không là checker rỗng: Frame vừa W vừa X, và `w + x > maps` đều bị bắt.
#[test]
fn check_wx_bat_frame_vua_w_vua_x_va_tong_lon_hon_maps() {
    let (mut m, c, _u, src) = rig_frame();
    let cap = m.retype(src, ObjType::Frame, 12, 0).expect("Frame");
    let f = m.cap_info(cap).expect("cap").obj;
    m.put_cap(c, 1, cap).expect("đặt cap");
    m.check_wx().expect("Frame mới sạch");
    // Gieo trạng thái **không thể** sinh qua public op: w=1, x=1, maps=2.
    {
        let fm = m.objects[f as usize - 1].frame.as_mut().expect("meta");
        fm.maps = 2;
        fm.mapped_w = 1;
        fm.mapped_x = 1;
    }
    let e = m.check_wx().expect_err("phải bắt W∧X");
    assert!(e.contains("W^X") || e.contains("vừa"), "{e}");
    // Gieo `w + x > maps` (w=2, x=0, maps=1) — bộ đếm cộng vượt số mapping thật.
    {
        let fm = m.objects[f as usize - 1].frame.as_mut().expect("meta");
        fm.maps = 1;
        fm.mapped_w = 2;
        fm.mapped_x = 0;
    }
    let e = m.check_wx().expect_err("phải bắt w+x > maps");
    assert!(e.contains("maps"), "{e}");
    {
        let fm = m.objects[f as usize - 1].frame.as_mut().expect("meta");
        fm.maps = 0;
        fm.mapped_w = 0;
        fm.mapped_x = 0;
    }
    m.check_wx().expect("khôi phục thì sạch");
}

/// `wx_recount` đếm lại **từ cây PageTable** và bắt bộ đếm Frame bị sửa tay (không lệch
/// âm thầm); trước khi sửa, recount khớp đúng bộ đếm thật.
#[test]
fn wx_recount_bat_bo_dem_frame_lech() {
    let (mut m, c, _u, src) = rig_frame();
    let ptcap = m.retype(src, ObjType::PageTable, 0, 0).expect("PageTable");
    let root = m.cap_info(ptcap).expect("cap").obj;
    m.put_cap(c, 1, ptcap).expect("cap PageTable");
    // Frame 1 GiB lá ngay root (không cần bảng trung gian); body Frame ở kernel arena nên
    // không đụng 4096 B arena của Untyped.
    let fcap = m.retype(src, ObjType::Frame, 30, 0).expect("Frame 1G");
    let f = m.cap_info(fcap).expect("cap").obj;
    m.put_cap(c, 2, fcap).expect("cap Frame");
    m.page_table_map_obj(
        root,
        1 << 30,
        f,
        MapRights::READ | MapRights::WRITE,
        MapAttrs::WB,
    )
    .expect("map 1G");
    assert_eq!(
        m.wx_recount(),
        Ok(WxRecount {
            frames: 1,
            maps: 1,
            mapped_w: 1,
            mapped_x: 0,
        })
    );
    // Gieo hỏng: bộ đếm Frame không còn khớp số lá (đúng lỗi mutation "bỏ giảm bộ đếm").
    m.objects[f as usize - 1]
        .frame
        .as_mut()
        .expect("meta")
        .mapped_w = 0;
    let e = m.wx_recount().expect_err("phải bắt bộ đếm lệch cây");
    assert!(e.contains("đếm lại"), "{e}");
    m.objects[f as usize - 1]
        .frame
        .as_mut()
        .expect("meta")
        .mapped_w = 1;
    m.wx_recount().expect("khôi phục thì khớp");
    // Gieo W∧X **khớp cây** (đúng lỗi mà luật phải bắt, không phải lỗi đếm): thêm một lá X
    // ở VA 2 GiB rồi đặt bộ đếm tương ứng — trạng thái này không thể sinh qua public op vì
    // `frame_note_map` chặn map X khi `mapped_w > 0`.
    let pte_x = leaf_pte(f, MapRights::READ, MapAttrs::WB | MapAttrs::EXEC);
    m.page_table_data_mut(root)
        .expect("dữ liệu root")
        .entries
        .set(2, pte_x)
        .expect("ghi lá X");
    m.page_table_data_mut(root).expect("dữ liệu root").mapped = 2;
    {
        let fm = m.objects[f as usize - 1].frame.as_mut().expect("meta");
        fm.maps = 2;
        fm.mapped_w = 1;
        fm.mapped_x = 1;
    }
    let e = m.wx_recount().expect_err("phải bắt W∧X");
    assert!(e.contains("W^X") || e.contains("vừa"), "{e}");
}

// ------------------------------------------------ KT-41: check_fault phải **bắt** lỗi

/// `check_fault` không là checker rỗng: (1) TCB rời `BlockedOnSend` mà message fault còn
/// trong hàng, (2) fault bị **nhân bản** trong hàng — cả hai đều bị bắt (ca dương tính của
/// bất biến "fault không mất" — K2-PLAN KT-41).
#[test]
fn check_fault_bat_fault_bi_mat_va_bi_nhan_ban() {
    let mut m = Machine::new();
    let c = m.add_cnode(64);
    let ep = m.add_kernel_object(ObjType::Endpoint);
    let ep_cap = m.root_cap(ep);
    m.put_cap(c, 0, ep_cap).expect("cap Endpoint");
    let t = m.add_kernel_object(ObjType::Tcb);
    let tcap = m.root_cap(t);
    m.put_cap(c, 1, tcap).expect("cap TCB");
    m.tcb_set_fault_ep(SlotRef::new(c, 1), SlotRef::new(c, 0))
        .expect("fault_ep");
    m.start_tcb(t).expect("Ready");
    let kind = FaultKind::Vm {
        vspace: 0,
        va: 0x3000,
        access: FaultAccess::Read,
        reason: VmFaultReason::NotMapped,
    };
    m.inject_fault(t, kind).expect("inject Vm");
    m.check_fault().expect("fault sạch ngay sau inject");
    assert_eq!(m.endpoint_send_len(ep), Ok(1));

    // (1) Trạng thái không còn khớp hàng (fault "mất" khỏi TCB).
    //
    // **KT-241** — `Ready` **không** còn biểu diễn được bằng `transition_tcb`; cửa đúng
    // ngữ nghĩa cho `BlockedOnSend → Ready` là `wake` (test-only, D2-144). `wake` gỡ
    // message fault khỏi hàng **đúng đường**, nên phải **gieo lại** message để dựng đúng
    // trạng thái lệch cần kiểm: hàng còn fault, TCB **không** còn `BlockedOnSend{fault}`.
    m.wake(t).expect("rời hàng trần");
    m.endpoint_enqueue(
        ep,
        QueuedMessage {
            msg: None,
            sender: t,
            call: true,
            fault: Some(kind),
            blocking: true,
            snaps: Vec::new(),
        },
    )
    .expect("gieo lại fault để dựng trạng thái lệch");
    let e = m
        .check_fault()
        .expect_err("phải bắt fault không khớp trạng thái");
    assert!(e.contains("trạng thái") || e.contains("hàng"), "{e}");
    m.transition_tcb(
        t,
        NonReadyTarget::BlockedOnSend {
            ep,
            call: true,
            fault: Some(kind),
        },
    )
    .expect("khôi phục trạng thái");
    m.check_fault().expect("khôi phục thì sạch");

    // (2) Fault **nhân bản** trong hàng (đúng một message cho mỗi TCB bị fault).
    m.endpoint_enqueue(
        ep,
        QueuedMessage {
            msg: None,
            sender: t,
            call: true,
            fault: Some(kind),
            blocking: true,
            snaps: Vec::new(),
        },
    )
    .expect("chèn fault trùng");
    let e = m.check_fault().expect_err("phải bắt fault nhân bản");
    assert!(e.contains("đúng 1"), "{e}");
    m.body_mut(ep, |k| match k {
        Kind::Endpoint(x) => {
            while x.send.pop_front().is_some() {}
            Ok(())
        }
        _ => Err(KError::WrongObjectType),
    })
    .expect("xoá hàng");
}

// ------------------------------------------------ KT-40: check_tlb phải **bắt** lỗi

/// `check_tlb` không là checker rỗng: entry của VSpace **không còn sống** và entry của VA
/// **không còn map** đều bị bắt (ca dương tính của oracle TLB-safety — K2-PLAN KT-40
/// "oracle quá dễ dãi ⇒ 0 vi phạm vô nghĩa").
#[test]
fn check_tlb_bat_entry_mo_coi_va_entry_khong_con_map() {
    let (mut m, c, vs, _root) = rig_vspace();
    // Frame 1 GiB (lá ngay root) từ Untyped riêng ⇒ không cần bảng con.
    let uf = m.add_untyped(1 << 32, 4096);
    let capu = m.root_cap(uf);
    m.put_cap(c, 1, capu).expect("cap Untyped Frame");
    let capf = m
        .retype(SlotRef::new(c, 1), ObjType::Frame, 30, 0)
        .expect("Frame 1G");
    let f = m.cap_info(capf).expect("cap").obj;
    m.put_cap(c, 2, capf).expect("cap Frame");
    m.vspace_activate(vs, 0).expect("activate");
    m.vspace_map_obj(vs, 1 << 30, f, MapRights::READ, MapAttrs::WB)
        .expect("map 1G");
    let t = m.tlb_translate(vs, 1 << 30).expect("fill");
    assert!(!t.from_tlb);
    assert_eq!(m.tlb_entries(0), Ok(1));
    m.check_tlb().expect("TLB sạch sau fill");
    assert_eq!(m.tlb_violations(), 0);

    // (1) Gieo entry của `(gen, asid)` **không còn VSpace sống**.
    let info = m.vspace_info(vs).expect("info");
    let mut gia = HartTlb::new();
    let e = gia.fill_keyed(
        info.generation,
        info.asid + 7,
        Mapping {
            va: 0x5000,
            pa: 0x9000,
            size_bits: 12,
            frame: 1,
            rights: MapRights::READ,
            attrs: MapAttrs::WB,
        },
    );
    m.tlbs[0].seed_entry_for_test(e);
    let loi = m.check_tlb().expect_err("phải bắt entry mồ côi");
    assert!(loi.contains("không còn VSpace sống"), "{loi}");
    assert_eq!(m.tlb_violations(), 1);
    m.tlbs[0].entries_mut_for_test().pop();

    // (2) Gieo entry **đúng** `(gen, asid)` của VSpace sống nhưng VA không còn map.
    let mut gia = HartTlb::new();
    let e = gia.fill_keyed(
        info.generation,
        info.asid,
        Mapping {
            va: 0x5000,
            pa: 0x9000,
            size_bits: 12,
            frame: 1,
            rights: MapRights::READ,
            attrs: MapAttrs::WB,
        },
    );
    m.tlbs[0].seed_entry_for_test(e);
    let loi = m.check_tlb().expect_err("phải bắt entry của VA đã hết map");
    assert!(loi.contains("walk trả"), "{loi}");
    m.tlbs[0].entries_mut_for_test().pop();
    m.check_tlb().expect("khôi phục thì sạch");
}

// ------------------------------------------------ KT-42: check_vm phải **bắt** lỗi

/// `check_vm` **không rỗng** (ca dương tính của cổng KT-42): hai lỗi cố ý tiêm vào bản
/// sao trạng thái qua **cùng** ảnh chụp `Machine` (không thêm đường API nào) —
/// (1) **xoá PTE ngầm** (cây mất lá nhưng `PageTableData.mapped`/`FrameMeta.maps` còn 1) và
/// (2) **entry TLB mồ côi** (VA không còn map) — đều bị [`Machine::check_vm`] bắt; khôi
/// phục thì sạch. Đây là chứng minh `check_vm` không phải checker rỗng (K2-PLAN KT-42
/// "rủi ro: `.scn` chỉ kiểm đường vàng").
#[test]
fn check_vm_bat_pte_bi_xoa_ngam_va_entry_tlb_mo_coi() {
    let (mut m, c, vs, root) = rig_vspace();
    let uf = m.add_untyped(1 << 32, 4096);
    let capu = m.root_cap(uf);
    m.put_cap(c, 1, capu).expect("cap Untyped Frame");
    let capf = m
        .retype(SlotRef::new(c, 1), ObjType::Frame, 30, 0)
        .expect("Frame 1G");
    let f = m.cap_info(capf).expect("cap").obj;
    m.put_cap(c, 2, capf).expect("cap Frame");
    m.vspace_activate(vs, 0).expect("activate");
    m.vspace_map_obj(vs, 1 << 30, f, MapRights::READ, MapAttrs::WB)
        .expect("map 1G");
    assert!(
        !m.tlb_translate(vs, 1 << 30).expect("fill").from_tlb,
        "lần đầu phải miss"
    );
    m.check_vm()
        .expect("check_vm sạch trên trạng thái VM đầy đủ");

    // (1) Xoá PTE ngầm: bảng vẫn `mapped = 1`, FrameMeta vẫn `maps = 1` — chỉ cây đổi.
    let rslot = m.objects[root as usize - 1].pt.expect("pt root");
    let pte = m.page_tables[rslot]
        .as_ref()
        .expect("dữ liệu root")
        .entries
        .get(1)
        .expect("PTE root[1]");
    m.page_tables[rslot]
        .as_mut()
        .expect("dữ liệu root")
        .entries
        .set(1, Pte::EMPTY)
        .expect("xoá PTE root[1]");
    let e = m.check_vm().expect_err("check_vm phải bắt PTE bị xoá ngầm");
    assert!(
        e.contains("check_pages") || e.contains("bộ đếm") || e.contains("mapped"),
        "{e}"
    );
    m.page_tables[rslot]
        .as_mut()
        .expect("dữ liệu root")
        .entries
        .set(1, pte)
        .expect("khôi phục PTE root[1]");
    m.check_vm().expect("khôi phục PTE thì check_vm sạch");

    // (2) Entry TLB mồ côi: `(gen, asid)` đúng VSpace sống nhưng VA `0x6000` không map.
    let info = m.vspace_info(vs).expect("info VSpace");
    let mut gia = HartTlb::new();
    let entry = gia.fill_keyed(
        info.generation,
        info.asid,
        Mapping {
            va: 0x6000,
            pa: 0x9000,
            size_bits: 12,
            frame: f,
            rights: MapRights::READ,
            attrs: MapAttrs::WB,
        },
    );
    m.tlbs[0].seed_entry_for_test(entry);
    let e = m
        .check_vm()
        .expect_err("check_vm phải bắt entry TLB mồ côi");
    assert!(e.contains("check_tlb"), "{e}");
    assert!(e.contains("walk trả") || e.contains("VSpace"), "{e}");
    m.tlbs[0].entries_mut_for_test().pop();
    m.check_vm().expect("khôi phục TLB thì check_vm sạch");
}
