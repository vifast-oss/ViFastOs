// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **memsrv/allocator Frame** của ảnh test `vifast-kbin-ktest` (KT-67, K6 — D2-95).
//!
//! Nối sau pha core (KT-61/63/64) và **trước** TAP trap lồng (fatal). Năm TAP:
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 60 - mem alloc frame` | `vifast-alloc` (qua object layer): cấp `Frame` **2 MiB + 4 KiB** từ Untyped U0 (cấp giảm dần), PA nhãn đúng căn/cửa sổ, **ghi/đọc thật** qua physmap; **ba nguồn độc lập** bằng nhau: `live` (test biết nó cấp gì) = `stats.pa_used` (**Σ span Frame liệt kê từ bảng object**, không đọc allocator) = `used` (watermark của `FrameArena`) và `check_objmem` sạch (kiểm `Σ span + Σ hole == used` từ danh sách, không tautology) |
//! | `ok 61 - mem revoke frame` | `Revoke` Frame ⇒ byte/watermark/hole về 0, `frames=0`; cấp lại 4 KiB rơi **đúng PA cũ**; `check_objmem` sạch + `stats=0` |
//! | `ok 62 - mem out of memory` | Cửa sổ 1 GiB (U1): Frame 1 GiB thứ hai ⇒ `NotEnoughMemory` **mã 9**, nguyên tử (không đổi byte), **không panic**; trả Frame ⇒ byte về 0 |
//! | `ok 63 - mem bridge cap` | D2-83: Frame của object layer được **đại diện** bằng cap core; chuyển cap Frame qua **KT-63** (`Send` → receive window của TCB nhận) ⇒ cap cài **bỏ `RIGHT_GRANT`**, `resolve(core obj) → (obj cap, pa, bits)` đúng, `check_caps`/`check_ipc` xanh; `release` trả byte về 0 |
//! | `ok 64 - mem deny rights` | Từ chối đúng mã: cap client **sai loại** ⇒ `WrongObjectType{2}`, **thiếu `RIGHT_WRITE`** ⇒ `InsufficientRights{3}`, cap **chết** ⇒ `InvalidCapability{1}`; cả ba **nguyên tử** (0 byte cấp) |
//!
//! Số đo do chính boot in ở các dòng `# mem:` — `ktest.sh` grep đúng các dòng đó; tài liệu
//! **không** chép số tay (QD §9 Q12). Thiết kế/bất biến: `v2/docs/k6-memsrv.md`.

use core::fmt::Write;

use vifast_core::vm::{CapId, FrameBudget, ObjKind};
use vifast_core::{CapId as CoreCap, KError, Message, ObjId, ObjType, SlotRef};
use vifast_core::{RIGHTS_ALL, RIGHT_GRANT, RIGHT_WRITE};
use vifast_plat_common::{Platform, PlatformWriter};
use vifast_plat_riscv64::mem_bridge::{self, MemError};
use vifast_plat_riscv64::Riscv64Virt;

use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::obj as aobj;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// Dấu ghi vào Frame 4 KiB/2 MiB để chứng minh `pa` là RAM thật (không phải nhãn suông).
const TAG4: u64 = 0x4b54_3637_4d45_4d21; // "KT67MEM!"
const TAG2: u64 = 0x4b54_3637_4d45_4d22;
/// Nhãn `Untyped` **ngân sách client** của core cho pha mem (chỉ là nhãn PA — D2-83).
const CLIENT_TOKEN_PA: u64 = 0x8800_0000;
const CLIENT_TOKEN_BYTES: usize = 8192;

fn check(fails: &mut usize, ok: bool, tap: &str, detail: &str) -> bool {
    if ok {
        true
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(w, "not ok - {}: {}", tap, detail);
        *fails += 1;
        false
    }
}

fn budget_or_zero(cap: CapId) -> FrameBudget {
    mem_bridge::budget(cap).unwrap_or_default()
}

fn abi_code(error: Result<(), MemError>) -> u16 {
    error.err().and_then(|e| e.abi_code()).unwrap_or(0)
}

/// Pha memsrv/allocator — trả số TAP đỏ (0 = sạch).
pub fn mem_phase() -> usize {
    let mut fails = 0usize;
    // Dựng lại object layer từ đầu: số đo byte của pha này bắt đầu từ 0 (pha obj/core trước đã
    // xong; core giữ model của nó, không đụng object layer nữa).
    let report = aobj::init();
    let u0 = report.untyped0;
    let u1 = report.untyped1;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let b0 = budget_or_zero(u0);
        let b1 = budget_or_zero(u1);
        let _ = writeln!(
            w,
            "# mem: init untyped=2 u0={:?} u1={:?} base0=0x{:x} base1=0x{:x} window={} cap0={} cap1={} physmap={}",
            u0.0,
            u1.0,
            b0.base,
            b1.base,
            b0.capacity,
            b0.capacity,
            b1.capacity,
            report.physmap_ok as u8
        );
    }
    if !report.physmap_ok {
        check(
            &mut fails,
            false,
            "mem alloc frame",
            "physmap khong doc/ghi duoc",
        );
        return fails;
    }
    // ----------------------------------------------------------------------------------- TAP 60
    tap60_alloc(u0, &mut fails);
    // ----------------------------------------------------------------------------------- TAP 61
    tap61_revoke(u0, &mut fails);
    // ----------------------------------------------------------------------------------- TAP 62
    tap62_oom(u1, &mut fails);
    // ------------------------------------------------------------------------------ TAP 63/64
    tap63_bridge(u0, &mut fails);
    tap64_deny(u0, &mut fails);
    fails
}

/// TAP 60 — cấp Frame 2 MiB + 4 KiB từ U0, ghi/đọc thật, đối chiếu **hai phía** kế toán byte.
fn tap60_alloc(u0: CapId, fails: &mut usize) {
    let start = budget_or_zero(u0);
    let m = aobj::machine();
    let f2 = m.retype(u0, ObjKind::Frame, 21);
    let f4 = m.retype(u0, ObjKind::Frame, 12);
    let (Ok(f2), Ok(f4)) = (f2, f4) else {
        check(fails, false, "mem alloc frame", "retype Frame 2m/4k loi");
        return;
    };
    let pa2 = m.frame_pa(f2).unwrap_or(0);
    let pa4 = m.frame_pa(f4).unwrap_or(0);
    let s21 = m.frame_size_bits(f2).ok() == Some(21);
    let s12 = m.frame_size_bits(f4).ok() == Some(12);
    // PA nhãn là RAM thật (D2-66): ghi/đọc 8 B qua physmap.
    let real4 = m.write_phys_u64(pa4, TAG4) && m.read_phys_u64(pa4) == Some(TAG4);
    let real2 = m.write_phys_u64(pa2, TAG2) && m.read_phys_u64(pa2) == Some(TAG2);
    let after = budget_or_zero(u0);
    let stats = m.untyped_stats(u0).unwrap_or_default();
    let obj_check = m.check_objmem().is_ok();
    // Hai nguồn **độc lập** để so: `live` = tổng cỡ Frame mà **test này** biết nó đã cấp;
    // `stats.pa_used` = cùng tổng nhưng suy từ **bảng object** của object layer; `after.used` =
    // watermark nội bộ của allocator. Ba số bằng nhau mới xanh (không tautology — xem
    // `FrameArena::check_accounting`).
    let live = (1u64 << 21) + 4096;
    let want_used = live;
    let ok = s21
        && s12
        && real4
        && real2
        && pa2 & ((1 << 21) - 1) == 0
        && pa4 & 4095 == 0
        && after.used == want_used
        && after.live_4k == 1
        && after.live_2m == 1
        && after.live_1g == 0
        && after.hole_bytes == 0
        && after.allocs == 2
        // **Một nguồn sự thật byte**: allocator + object layer + `UntypedStats` cùng một số.
        && stats.pa_used == live
        && stats.pa_used == after.used
        && after.used == live + after.hole_bytes
        && stats.frames == 2
        && obj_check
        && after.base >= start.base;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mem: alloc cap0={} window={} base=0x{:x} used={} free={} hole={} live4={} live2={} live1g={} pa4=0x{:x} pa2=0x{:x} s21={} s12={} real4={} real2={} live={} stats={} frames={} allocs={} check={} obj={}",
            u0.0,
            after.capacity,
            after.base,
            after.used,
            after.free_bytes,
            after.hole_bytes,
            after.live_4k,
            after.live_2m,
            after.live_1g,
            pa4,
            pa2,
            s21 as u8,
            s12 as u8,
            real4 as u8,
            real2 as u8,
            live,
            stats.pa_used,
            stats.frames,
            after.allocs,
            (want_used == after.used) as u8,
            obj_check as u8
        );
    }
    if check(fails, ok, "mem alloc frame", "byte/PA/ghi-doc sai") {
        PLATFORM.put_str("ok 60 - mem alloc frame\n");
    }
    // Trả lại để TAP 61 bắt đầu từ trạng thái sạch.
    let m = aobj::machine();
    let _ = m.revoke(f4);
    let _ = m.revoke(f2);
}

/// TAP 61 — revoke ⇒ byte về 0, cấp lại rơi đúng PA cũ, `check_objmem` sạch.
fn tap61_revoke(u0: CapId, fails: &mut usize) {
    let m = aobj::machine();
    let f2 = m.retype(u0, ObjKind::Frame, 21).ok();
    let f4 = m.retype(u0, ObjKind::Frame, 12).ok();
    let (Some(f2), Some(f4)) = (f2, f4) else {
        check(fails, false, "mem revoke frame", "retype lai loi");
        return;
    };
    let pa2 = m.frame_pa(f2).unwrap_or(0);
    let used0 = budget_or_zero(u0).used;
    let r4 = m.revoke(f4).is_ok();
    let r2 = m.revoke(f2).is_ok();
    let after = budget_or_zero(u0);
    let stats = m.untyped_stats(u0).unwrap_or_default();
    // Cấp lại 4 KiB: watermark đã rewind ⇒ phải về đúng offset 0 (PA cũ).
    let again = m.retype(u0, ObjKind::Frame, 12);
    let pa_again = again.as_ref().ok().and_then(|c| m.frame_pa(*c).ok());
    let same = pa_again == Some(pa2);
    let (Ok(again), true) = (again, same) else {
        check(fails, false, "mem revoke frame", "cap lai khong dung PA cu");
        return;
    };
    let live_after = budget_or_zero(u0).live_4k;
    let _ = m.revoke(again);
    let end = budget_or_zero(u0);
    let obj_check = m.check_objmem().is_ok();
    let ok = r4
        && r2
        && used0 == (1u64 << 21) + 4096
        && after.used == 0
        && after.hole_bytes == 0
        && after.live_4k == 0
        && after.live_2m == 0
        && after.free_bytes == after.capacity
        && stats.pa_used == 0
        && stats.frames == 0
        && same
        && live_after == 1
        && end.used == 0
        && end.hole_bytes == 0
        && obj_check;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mem: revoke used0={} used1={} holes={} frames={} free={} realloc4=0x{:x} same={} stats={} check={} obj={}",
            used0,
            after.used,
            after.hole_bytes,
            after.live_4k + after.live_2m + after.live_1g,
            after.free_bytes,
            pa_again.unwrap_or(0),
            same as u8,
            stats.pa_used,
            (end.used == 0) as u8,
            obj_check as u8
        );
    }
    if check(fails, ok, "mem revoke frame", "byte/hole/PA sau revoke sai") {
        PLATFORM.put_str("ok 61 - mem revoke frame\n");
    }
}

/// TAP 62 — hết memory: Frame 1 GiB thứ hai ⇒ mã 9, nguyên tử, không panic.
fn tap62_oom(u1: CapId, fails: &mut usize) {
    let m = aobj::machine();
    let start = budget_or_zero(u1);
    let big = m.retype(u1, ObjKind::Frame, 30);
    let Ok(big) = big else {
        check(fails, false, "mem out of memory", "Frame 1g dau loi");
        return;
    };
    let pa = m.frame_pa(big).unwrap_or(0);
    let before = budget_or_zero(u1);
    let second = m.retype(u1, ObjKind::Frame, 30);
    let second_code = second
        .as_ref()
        .err()
        .and_then(|e| e.abi_code())
        .unwrap_or(0);
    let mid = budget_or_zero(u1);
    let small = m.retype(u1, ObjKind::Frame, 12);
    let small_code = small.as_ref().err().and_then(|e| e.abi_code()).unwrap_or(0);
    let after = budget_or_zero(u1);
    let _ = m.revoke(big);
    let end = budget_or_zero(u1);
    let obj_check = m.check_objmem().is_ok();
    let ok = pa == start.base
        && second.is_err()
        && second_code == 9
        && small.is_err()
        && small_code == 9
        && before.used == start.capacity
        && mid.used == before.used
        && after.used == before.used
        && after.live_1g == 1
        && end.used == 0
        && end.live_1g == 0
        && obj_check;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mem: oom window={} pa1g=0x{:x} second={} code={} small={} used={} used_after={} free=0 atomic={} panic=0 check={} obj={}",
            start.capacity,
            pa,
            if second.is_err() { "NotEnoughMemory" } else { "Ok" },
            second_code,
            small_code,
            mid.used,
            after.used,
            (mid == before && after == before) as u8,
            (second_code == 9 && small_code == 9) as u8,
            obj_check as u8
        );
    }
    if check(fails, ok, "mem out of memory", "ma/byte/atomic sai") {
        PLATFORM.put_str("ok 62 - mem out of memory\n");
    }
}

fn msg_caps(label: u32, words: &[u64], caps: &[CoreCap]) -> Message {
    Message::with_cap_ids(label, words, caps).expect("kt67: message cap hop le")
}

/// TAP 63 — cầu nối D2-83 + chuyển cap Frame qua KT-63 (2 TCB + endpoint + receive window).
fn tap63_bridge(u0: CapId, fails: &mut usize) {
    // --- core: Untyped ngân sách client + CNode + 2 TCB + Endpoint.
    let setup = core_bridge::with_machine(|m| {
        let token_obj = m.add_untyped(CLIENT_TOKEN_PA, CLIENT_TOKEN_BYTES);
        let root = m.add_cnode(48);
        let tok = m.root_cap(token_obj);
        m.put_cap(root, 0, tok)?;
        let us = SlotRef::new(root, 0);
        let mut mk = |ty: ObjType, slot: usize| -> Result<(ObjId, CoreCap), vifast_core::KError> {
            let cap = m.retype(us, ty, 0, 0)?;
            let obj = m.cap_obj(cap)?;
            m.put_cap(root, slot, cap)?;
            Ok((obj, cap))
        };
        let (t0, _c0) = mk(ObjType::Tcb, 1)?;
        let (t1, _c1) = mk(ObjType::Tcb, 2)?;
        mk(ObjType::Endpoint, 10)?;
        m.start_tcb(t0)?;
        m.start_tcb(t1)?;
        let token_cap = m.cap_at(root, 0)?;
        Ok::<_, vifast_core::KError>((root, token_cap, t0, t1))
    });
    let (root, token_cap, t0, t1) = match setup {
        Ok(v) => v,
        Err(e) => {
            check(fails, false, "mem bridge cap", e.name());
            return;
        }
    };
    // --- bước memsrv gọi: quyết định theo cap client + cấp Frame thật qua object layer.
    let grant = match mem_bridge::grant(token_cap, u0, 12) {
        Ok(g) => g,
        Err(e) => {
            check(fails, false, "mem bridge cap", e.name());
            return;
        }
    };
    let used_granted = budget_or_zero(u0).used;
    let transfer = core_bridge::with_machine(|m| -> Result<(CoreCap, bool, bool), u16> {
        m.put_cap(root, 3, grant.core_cap).map_err(KError::id)?;
        m.tcb_set_recv_window(
            SlotRef::new(root, 2),
            [
                SlotRef::new(root, 4),
                SlotRef::new(root, 5),
                SlotRef::new(root, 6),
                SlotRef::new(root, 7),
            ],
        )
        .map_err(KError::id)?;
        m.recv(t1, SlotRef::new(root, 10), None)
            .map_err(KError::id)?;
        let src = m.cap_at(root, 3).map_err(KError::id)?;
        m.send(
            t0,
            SlotRef::new(root, 10),
            msg_caps(0x901, &[0x6700], &[src]),
        )
        .map_err(KError::id)?;
        let installed = m.cap_at(root, 4).map_err(KError::id)?;
        let info = m.cap_info(installed).map_err(KError::id)?;
        let rights_ok = info.rights == (RIGHTS_ALL & !RIGHT_GRANT);
        let obj_ok = info.obj == grant.core_obj;
        Ok((installed, rights_ok, obj_ok))
    });
    let (slot, rights_ok, obj_ok) = transfer.unwrap_or((0, false, false));
    // --- tra ngược một chiều: core obj → object layer (cap Frame, pa, size_bits).
    let resolved = mem_bridge::resolve(grant.core_obj);
    let resolve_ok = resolved == Some((grant.obj_cap, grant.pa, grant.size_bits));
    let check_caps = core_bridge::with_machine(|m| m.check_caps().is_ok());
    let check_ipc = core_bridge::with_machine(|m| m.check_ipc().is_ok());
    // --- dọn: xoá cap (window → cap gốc) rồi trả Frame; byte phải về 0.
    let cleaned = core_bridge::with_machine(|m| {
        for s in [4usize, 3, 1, 2] {
            let _ = m.delete(SlotRef::new(root, s));
        }
        m.check_caps().is_ok()
    });
    let released = mem_bridge::release(grant).is_ok();
    let end = budget_or_zero(u0);
    let obj_check = aobj::machine().check_objmem().is_ok();
    let ok = rights_ok
        && obj_ok
        && slot != 0
        && resolve_ok
        && check_caps
        && check_ipc
        && cleaned
        && released
        && end.used == 0
        && end.hole_bytes == 0
        && used_granted == 4096
        && obj_check;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mem: bridge token={} core_obj={} obj_cap={} key=0x{:x} pa=0x{:x} bits={} slot={} rights_ok={} obj_ok={} resolve={} used_granted={} check_caps={} check_ipc={} check_objmem={}",
            token_cap,
            grant.core_obj,
            grant.obj_cap.0,
            grant.key,
            grant.pa,
            grant.size_bits,
            slot,
            rights_ok as u8,
            obj_ok as u8,
            resolve_ok as u8,
            used_granted,
            check_caps as u8,
            check_ipc as u8,
            obj_check as u8
        );
        let _ = writeln!(
            w,
            "# mem: release released={} used_after={} free={} cap_after={} caps_check={} obj={}",
            released as u8,
            end.used,
            end.free_bytes,
            end.live_4k + end.live_2m + end.live_1g,
            cleaned as u8,
            obj_check as u8
        );
    }
    if check(fails, ok, "mem bridge cap", "cap/rights/resolve/byte sai") {
        PLATFORM.put_str("ok 63 - mem bridge cap\n");
    }
}

/// TAP 64 — từ chối đúng mã + nguyên tử (0 byte cấp).
fn tap64_deny(u0: CapId, fails: &mut usize) {
    let start = budget_or_zero(u0);
    // (a) cap client **thiếu `RIGHT_WRITE`**.
    let no_rights = core_bridge::with_machine(|m| {
        let root = m.add_cnode(8);
        let obj = m.add_untyped(CLIENT_TOKEN_PA + 0x1_0000, 4096);
        let tok = m.root_cap(obj);
        m.put_cap(root, 0, tok)?;
        let src = SlotRef::new(root, 0);
        let cap = m.copy_rights(src, SlotRef::new(root, 1), Some(RIGHTS_ALL & !RIGHT_WRITE))?;
        Ok::<_, vifast_core::KError>(cap)
    });
    let rights_code = match no_rights {
        Ok(cap) => abi_code(mem_bridge::grant(cap, u0, 12).map(|_| ())),
        Err(_) => 0,
    };
    // (b) cap client **sai loại**: Frame đại diện của object layer (D2-83).
    let foreign = core_bridge::with_machine(|m| {
        let obj = m.add_foreign_object(ObjType::Frame, 0x1234_5678)?;
        let cap = m.root_cap(obj);
        Ok::<_, vifast_core::KError>(cap)
    });
    let wrong_code = match foreign {
        Ok(cap) => abi_code(mem_bridge::grant(cap, u0, 12).map(|_| ())),
        Err(_) => 0,
    };
    // (c) cap **không tồn tại** (id ngoài bảng cap) — `InvalidCapability`.
    let invalid_code = abi_code(mem_bridge::grant(CoreCap::MAX, u0, 12).map(|_| ()));
    let end = budget_or_zero(u0);
    let obj_check = aobj::machine().check_objmem().is_ok();
    let ok = rights_code == 3
        && wrong_code == 2
        && invalid_code == 1
        && start == end
        && end.used == 0
        && end.allocs == start.allocs
        && obj_check;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# mem: deny wrong={} rights={} invalid={} used={} allocs={} atomic={} obj={}",
            wrong_code,
            rights_code,
            invalid_code,
            end.used,
            end.allocs,
            (start == end) as u8,
            obj_check as u8
        );
    }
    if check(fails, ok, "mem deny rights", "ma loi/atomic sai") {
        PLATFORM.put_str("ok 64 - mem deny rights\n");
    }
    let _ = fails;
}
