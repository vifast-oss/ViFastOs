// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pha **object layer** của ảnh test `vifast-kbin-ktest` (KT-52).
//!
//! Nối sau pha TLB/ASID KT-51; TAP **30..36** (trap lồng dời sang TAP 37):
//!
//! | TAP | Điều kiện đo |
//! |---|---|
//! | `ok 30 - retype vspace` | `Retype` **nội bộ** (không ABI): 1 `VSpace` (root PageTable 4 KiB + ASID nhãn), **3** `PageTable` rời, 1 `Frame` 4 KiB; cap/key sống, walk root rỗng invalid; `check_objmem` sạch |
//! | `ok 31 - untyped byte invariant` | bất biến byte: `capacity` không đổi; 2 PageTable ⇒ `used +8192`, `free −8192`; revoke ⇒ `used/free` về đúng mốc; `size_bits` lạ ⇒ `InvalidArgument` **atomic** (không đổi byte) |
//! | `ok 32 - frame 3 sizes` | Frame **4 KiB / 2 MiB / 1 GiB** (`size_bits` 12/21/30 từ `vifast-pte`): 4 KiB + 2 MiB **ghi/đọc thật** qua physmap tại `pa`; 1 GiB kiểm nhãn/căn 1 GiB/cửa sổ (D2-66, `-m 128M`); `size_bits=13` ⇒ `InvalidArgument`, atomic |
//! | `ok 33 - wx violation x-then-w` | **W^X ca 1**: Frame map `R|X` rồi map `R|W` (VA khác) ⇒ `WxViolation`; **atomic**: không PTE mới, `maps/w/x` của Frame không đổi |
//! | `ok 34 - wx violation device+exec` | **W^X ca 2**: `Device + EXEC` ⇒ `WxViolation`; không mapping, Frame 0 mapping; in bằng chứng `# wx:` |
//! | `ok 35 - map/unmap 4 KiB` | VSpace **thật**: chép root kernel vào root VSpace, `csrw satp` (ASID nhãn) + `sfence.vma`, map 4 KiB `U|R|W` tại `0x4000_0000`, ghi/đọc **qua VA user** bằng `sstatus.SUM` (phần cứng chuyển dịch), `protect` sang `R|NX` (PTE mất W), `unmap` + `sfence` ⇒ walk invalid; khôi phục `satp` kernel, máy còn sống |
//! | `ok 36 - 0 ro` | 0 rò: sau revoke hết, `used=0`, `free=capacity`, `holes=0` (đệm căn lề là hole), không Frame/PageTable/VSpace/mapping, `orphans=0`, `check_objmem` ĐẠT |
//!
//! Số đo do chính boot in (`# obj init:`, `# retype:`, `# untyped bytes:`, `# frame:`, `# wx:`,
//! `# map:`, `# leak:`) — `ktest.sh` grep đúng các dòng đó; tài liệu không chép số tay
//! (QD §9 Q12). Xem `v2/docs/k3-riscv64.md` §15.

use core::fmt::Write;

use vifast_plat_common::obj::{CapId, FlushRequest, Machine, MapFlags, ObjError, ObjKind};
use vifast_plat_common::{Platform, PlatformWriter};

use vifast_plat_riscv64::obj::{self as aobj, PhysmapMem};
use vifast_plat_riscv64::Riscv64Virt;

static PLATFORM: Riscv64Virt = Riscv64Virt;

/// VA thử nghiệm của TAP 30/35 (root index 1, ngoài mọi mapping kernel).
const MAP_VA: u64 = 0x4000_0000;
/// VA cho ca W^X 1 (cùng root index với MAP_VA nhưng khác bảng lá).
const WX_VA: u64 = 0x6000_0000;
/// VA cho ca W^X 2 (`Device+EXEC`).
const WX2_VA: u64 = 0x6020_0000;

type Mac = Machine<PhysmapMem>;

/// In một dòng "not ok" + tăng `fails` khi điều kiện sai.
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

/// Thực thi + đếm mọi flush đang chờ bằng `sfence.vma` thật (KT-51).
fn flush_all(m: &mut Mac) -> usize {
    let mut page = 0usize;
    let n = m.drain_flushes(|req| {
        if matches!(req, FlushRequest::Page { .. }) {
            page += 1;
        }
        aobj::run_flush(req);
    });
    let _ = n;
    page
}

/// Pha object layer — trả số TAP đỏ (0 = sạch).
pub fn obj_phase() -> usize {
    let mut fails = 0usize;
    let report = aobj::init();
    let m = aobj::machine();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# obj init: arena=0x{:x}-0x{:x} bytes={} untyped={} window=0x{:x} pa0=0x{:x} pa1=0x{:x} physmap={}",
            report.arena_base,
            report.arena_end,
            report.arena_bytes,
            report.untypeds,
            report.window,
            report.pa0,
            report.pa1,
            report.physmap_ok as u8
        );
    }
    let u0 = report.untyped0;
    let u1 = report.untyped1;
    if !report.physmap_ok {
        PLATFORM.put_str("not ok - obj init: physmap khong doc/ghi duoc\n");
        return fails + 1;
    }

    // ----------------------------------------------------------------------------------- TAP 30
    tap30_retype_vspace(m, u0, &mut fails);
    // ----------------------------------------------------------------------------------- TAP 31
    tap31_byte_invariant(m, u0, &mut fails);
    // ----------------------------------------------------------------------------------- TAP 32
    tap32_frame_sizes(m, u0, u1, &mut fails);
    // ------------------------------------------------------------------------------ TAP 33/34
    tap34_wx(m, u0, &mut fails);
    // ----------------------------------------------------------------------------------- TAP 35
    tap35_map_unmap(m, u0, &mut fails);
    // ----------------------------------------------------------------------------------- TAP 36
    tap36_no_leak(m, u0, &mut fails);
    fails
}

fn tap30_retype_vspace(m: &mut Mac, u0: CapId, fails: &mut usize) {
    let vs = match m.retype(u0, ObjKind::VSpace, 0) {
        Ok(cap) => cap,
        Err(e) => {
            check(fails, false, "retype vspace", aobj::error_name(e));
            return;
        }
    };
    let root = m.vspace_root_pa(vs).unwrap_or(0);
    let asid = m.vspace_asid(vs).unwrap_or(0);
    let pt1 = m.retype(u0, ObjKind::PageTable, 12);
    let pt2 = m.retype(u0, ObjKind::PageTable, 12);
    let pt3 = m.retype(u0, ObjKind::PageTable, 12);
    let fr = m.retype(u0, ObjKind::Frame, 12);
    let cap_ok = m.cap_kind(vs).ok() == Some(ObjKind::VSpace)
        && m.cap_key(vs).map(|k| k > 0).unwrap_or(false)
        && pt1.is_ok()
        && pt2.is_ok()
        && pt3.is_ok()
        && fr.is_ok();
    let walk_empty = m.walk(vs, MAP_VA).map(|w| !w.valid).unwrap_or(false);
    let stats = m.check_objmem();
    let counts = stats.as_ref().map(|s| s.counts).unwrap_or_default();
    let ok = root != 0
        && root & 0xfff == 0
        && asid >= 1
        && cap_ok
        && walk_empty
        && counts.vspaces == 1
        && counts.frames == 1
        && counts.page_tables >= 4 // 3 PageTable rời + root
        && stats.is_ok();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# retype: vspace={} root=0x{:x} asid={} pt=3 frame={} cap_ok={} walk_empty={}",
            m.counts().vspaces,
            root,
            asid,
            m.counts().frames,
            cap_ok as u8,
            walk_empty as u8
        );
    }
    if check(
        fails,
        ok,
        "retype vspace",
        "cap/root/walk/check_objmem khong dat",
    ) {
        PLATFORM.put_str("ok 30 - retype vspace\n");
    }
    // Dọn sạch trước TAP 31 (byte invariant đo từ mốc trắng).
    let _ = fr.map(|cap| m.revoke(cap));
    let _ = pt1.map(|cap| m.revoke(cap));
    let _ = pt2.map(|cap| m.revoke(cap));
    let _ = pt3.map(|cap| m.revoke(cap));
    let _ = m.revoke(vs);
}

fn tap31_byte_invariant(m: &mut Mac, u0: CapId, fails: &mut usize) {
    let s0 = match m.untyped_stats(u0) {
        Ok(s) => s,
        Err(e) => {
            check(fails, false, "untyped byte invariant", aobj::error_name(e));
            return;
        }
    };
    let a = m.retype(u0, ObjKind::PageTable, 12);
    let b = m.retype(u0, ObjKind::PageTable, 12);
    let s1 = m.untyped_stats(u0).unwrap_or_default();
    let grew = s1.used == s0.used + 2 * 4096 && s1.free == s0.free - 2 * 4096;
    // Lỗi retype phải atomic: không đổi byte.
    let bad = m.retype(u0, ObjKind::Frame, 13).err();
    let s2 = m.untyped_stats(u0).unwrap_or_default();
    let atomic = bad == Some(ObjError::InvalidArgument) && s2 == s1;
    // Hết arena ⇒ mọi lần gọi sau trả `NotEnoughMemory` **atomic** (byte trước/sau y hệt).
    // KT-157: arena object layer 64 → 96 KiB ⇒ 24 PageTable 4 KiB; mảng phải đủ chỗ để chạm
    // `NotEnoughMemory` thật (20 phần tử cũ dừng sớm ⇒ `oom=0` ⇒ TAP 31 đỏ oan).
    let mut caps = [CapId::NONE; 32];
    let mut n_caps = 0usize;
    let mut oom = false;
    let mut oom_atomic = false;
    loop {
        let before_fail = m.untyped_stats(u0).unwrap_or_default();
        match m.retype(u0, ObjKind::PageTable, 12) {
            Ok(cap) => {
                if n_caps < caps.len() {
                    caps[n_caps] = cap;
                    n_caps += 1;
                } else {
                    let _ = m.revoke(cap);
                    break;
                }
            }
            Err(e) => {
                oom = e == ObjError::NotEnoughMemory;
                oom_atomic = oom && m.untyped_stats(u0).unwrap_or_default() == before_fail;
                break;
            }
        }
    }
    let mut revoke_ok = true;
    for i in (0..n_caps).rev() {
        revoke_ok &= m.revoke(caps[i]).is_ok();
    }
    revoke_ok = revoke_ok
        && match (a, b) {
            (Ok(x), Ok(y)) => m.revoke(x).is_ok() && m.revoke(y).is_ok(),
            _ => false,
        };
    let s3 = m.untyped_stats(u0).unwrap_or_default();
    let restored = s3.used == s0.used && s3.free == s0.free;
    let check_ok = m.check_objmem().is_ok();
    let invariant = s1.capacity == s0.capacity && s3.capacity == s0.capacity;
    let ok = grew && atomic && oom && oom_atomic && revoke_ok && restored && check_ok && invariant;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# untyped bytes: cap={} used0={} after_retype={} after_revoke={} free0={} free1={} invariant={} atomic={} oom={}",
            s0.capacity,
            s0.used,
            s1.used,
            s3.used,
            s0.free,
            s1.free,
            invariant as u8,
            (atomic && oom_atomic) as u8,
            oom as u8
        );
    }
    if check(
        fails,
        ok,
        "untyped byte invariant",
        "used/free lech hoac retype loi khong atomic",
    ) {
        PLATFORM.put_str("ok 31 - untyped byte invariant\n");
    }
}

fn tap32_frame_sizes(m: &mut Mac, u0: CapId, u1: CapId, fails: &mut usize) {
    let marker2m = 0x2123_4567_89ab_cdefu64;
    let marker4k = 0x4fed_cba9_8765_4321u64;
    let f2 = m.retype(u0, ObjKind::Frame, 21);
    let f4 = m.retype(u0, ObjKind::Frame, 12);
    let (pa2, pa4) = match (f2, f4) {
        (Ok(f2), Ok(f4)) => (m.frame_pa(f2).unwrap_or(0), m.frame_pa(f4).unwrap_or(0)),
        _ => {
            check(fails, false, "frame 3 sizes", "khong retype duoc 2MiB/4KiB");
            return;
        }
    };
    let real2 = m.write_phys_u64(pa2, marker2m) && m.read_phys_u64(pa2) == Some(marker2m);
    let real4 = m.write_phys_u64(pa4, marker4k) && m.read_phys_u64(pa4) == Some(marker4k);
    // size_bits lạ: InvalidArgument + atomic (pa_used không đổi).
    let before = m.untyped_stats(u0).unwrap_or_default();
    let bad = m.retype(u0, ObjKind::Frame, 13).err();
    let after = m.untyped_stats(u0).unwrap_or_default();
    let atomic = bad == Some(ObjError::InvalidArgument) && before == after;
    let _ = m.revoke(f2.unwrap());
    let _ = m.revoke(f4.unwrap());
    // Frame 1 GiB: cửa sổ PA căn 1 GiB (chỉ kiểm nhãn/căn lề trên -m 128M — D2-66).
    let f1 = m.retype(u1, ObjKind::Frame, 30);
    let (pa1g, s1g, f1_cap) = match f1 {
        Ok(cap) => (
            m.frame_pa(cap).unwrap_or(0),
            m.frame_size_bits(cap).unwrap_or(0),
            Some(cap),
        ),
        Err(_) => (0, 0, None),
    };
    let aligned =
        pa2 & ((1 << 21) - 1) == 0 && pa4 & 0xfff == 0 && pa1g & ((1 << 30) - 1) == 0 && s1g == 30;
    // Đáy cửa sổ PA của `U1` lấy từ hằng số arch (không hardcode trong TAP) + căn 1 GiB.
    let window_base = aobj::OBJ_PA_BASE_U1;
    let window = pa1g == window_base && window_base & ((1 << 30) - 1) == 0;
    let ok = real2 && real4 && atomic && aligned && window && m.check_objmem().is_ok();
    if let Some(cap) = f1_cap {
        let _ = m.revoke(cap);
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# frame: s12=1 s21=1 s30={} pa4=0x{:x} pa2=0x{:x} pa1g=0x{:x} aligned={} real4={} real2={} window={} bad13={} atomic={}",
            (s1g == 30) as u8,
            pa4,
            pa2,
            pa1g,
            aligned as u8,
            real4 as u8,
            real2 as u8,
            window as u8,
            if bad == Some(ObjError::InvalidArgument) {
                "InvalidArgument"
            } else {
                "wrong"
            },
            atomic as u8
        );
    }
    if check(
        fails,
        ok,
        "frame 3 sizes",
        "co/kich thuoc/PA/real/atomic khong dat",
    ) {
        PLATFORM.put_str("ok 32 - frame 3 sizes\n");
    }
}

fn tap34_wx(m: &mut Mac, u0: CapId, fails: &mut usize) {
    // --- Ca 1: map R|X rồi map R|W trên cùng Frame ⇒ WxViolation.
    let vs = match m.retype(u0, ObjKind::VSpace, 0) {
        Ok(cap) => cap,
        Err(e) => {
            check(fails, false, "wx violation x-then-w", aobj::error_name(e));
            return;
        }
    };
    let fr = match m.retype(u0, ObjKind::Frame, 12) {
        Ok(cap) => cap,
        Err(e) => {
            check(fails, false, "wx violation x-then-w", aobj::error_name(e));
            return;
        }
    };
    let mapped_x = m.map(vs, WX_VA, fr, MapFlags::RX_USER).is_ok();
    let counts_before = m.frame_counts(fr).unwrap_or_default();
    let x_then_w = m.map(vs, WX_VA + 0x1000, fr, MapFlags::RW_NX_USER).err();
    let counts_after = m.frame_counts(fr).unwrap_or_default();
    let no_pte = m
        .walk(vs, WX_VA + 0x1000)
        .map(|w| !w.valid)
        .unwrap_or(false);
    let atomic1 =
        x_then_w == Some(ObjError::WxViolation) && counts_before == counts_after && no_pte;
    let ok1 = mapped_x && atomic1 && counts_after == (1, 0, 1);
    if check(
        fails,
        ok1,
        "wx violation x-then-w",
        "khong WxViolation hoac mat atomic",
    ) {
        PLATFORM.put_str("ok 33 - wx violation x-then-w\n");
    }

    // --- Ca 2: Device + EXEC ⇒ WxViolation (chính sách, Sv39 không có bit Device).
    let vs2 = match m.retype(u0, ObjKind::VSpace, 0) {
        Ok(cap) => cap,
        Err(e) => {
            check(
                fails,
                false,
                "wx violation device+exec",
                aobj::error_name(e),
            );
            let _ = m.revoke(fr);
            let _ = m.revoke(vs);
            return;
        }
    };
    let fr2 = match m.retype(u0, ObjKind::Frame, 12) {
        Ok(cap) => cap,
        Err(e) => {
            check(
                fails,
                false,
                "wx violation device+exec",
                aobj::error_name(e),
            );
            let _ = m.revoke(vs2);
            let _ = m.revoke(fr);
            let _ = m.revoke(vs);
            return;
        }
    };
    let device_exec = m.map(vs2, WX2_VA, fr2, MapFlags::DEVICE_RX).err();
    let counts2 = m.frame_counts(fr2).unwrap_or_default();
    let no_pte2 = m.walk(vs2, WX2_VA).map(|w| !w.valid).unwrap_or(false);
    let atomic2 = device_exec == Some(ObjError::WxViolation) && counts2 == (0, 0, 0) && no_pte2;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# wx: x_then_w={} device_exec={} atomic={} maps={} w={} x={}",
            if x_then_w == Some(ObjError::WxViolation) {
                "WxViolation"
            } else {
                "wrong"
            },
            if device_exec == Some(ObjError::WxViolation) {
                "WxViolation"
            } else {
                "wrong"
            },
            atomic1 as u8 & atomic2 as u8,
            counts_after.0,
            counts_after.1,
            counts_after.2
        );
    }
    if check(
        fails,
        atomic2 && m.check_objmem().is_ok(),
        "wx violation device+exec",
        "khong WxViolation hoac mat atomic",
    ) {
        PLATFORM.put_str("ok 34 - wx violation device+exec\n");
    }
    // Dọn: gỡ mapping X rồi revoke VSpace/Frame.
    let _ = m.unmap(vs, WX_VA);
    let _ = m.revoke(vs2);
    let _ = m.revoke(fr2);
    let _ = m.revoke(vs);
    let _ = m.revoke(fr);
    let _ = flush_all(m);
}

fn tap35_map_unmap(m: &mut Mac, u0: CapId, fails: &mut usize) {
    let vs = match m.retype(u0, ObjKind::VSpace, 0) {
        Ok(cap) => cap,
        Err(e) => {
            check(fails, false, "map/unmap 4 KiB", aobj::error_name(e));
            return;
        }
    };
    let fr = match m.retype(u0, ObjKind::Frame, 12) {
        Ok(cap) => cap,
        Err(e) => {
            check(fails, false, "map/unmap 4 KiB", aobj::error_name(e));
            return;
        }
    };
    let pa = m.frame_pa(fr).unwrap_or(0);
    let root = m.vspace_root_pa(vs).unwrap_or(0);
    let asid = m.vspace_asid(vs).unwrap_or(0);
    // Chép root kernel trước; **chỉ** khi chép xong mới đổi `satp`. Từ đây tới lúc khôi phục
    // KHÔNG in/log (log cần UART; nếu hỏng thì panic handler vẫn chạy được vì root VSpace đã chép
    // đủ mapping kernel — nhưng đường TAP không dựa vào đó).
    let prepared = aobj::copy_kernel_root(root);
    let switched = prepared && aobj::switch_root(root, asid);
    let marker = 0x524b_3532_4d41_5000u64; // "RK52MAP\0"
    let mut flush = 0usize;
    let mut map_ok = false;
    let mut write_ok = false;
    let mut read_ok = false;
    let mut protect_ok = false;
    let mut flags_ok = false;
    let mut unmapped = false;
    let mut after_invalid = false;
    if switched {
        map_ok = m.map(vs, MAP_VA, fr, MapFlags::RW_NX_USER).is_ok();
        flush += flush_all(m);
        write_ok = aobj::va_write_u64_sum(MAP_VA, marker);
        read_ok = aobj::va_read_u64_sum(MAP_VA) == Some(marker) && aobj::sum_off();
        let ro = MapFlags {
            read: true,
            write: false,
            exec: false,
            user: true,
            device: false,
        };
        protect_ok = m.protect(vs, MAP_VA, ro).is_ok();
        flush += flush_all(m);
        flags_ok = m
            .walk(vs, MAP_VA)
            .map(|w| w.valid && w.leaf && w.level == 0 && w.phys == pa && ro.matches_pte(w.raw))
            .unwrap_or(false);
        unmapped = m.unmap(vs, MAP_VA).is_ok();
        flush += flush_all(m);
        after_invalid = m.walk(vs, MAP_VA).map(|w| !w.valid).unwrap_or(false);
    }
    // Khôi phục `satp` kernel **vô điều kiện** (kể cả `switched = false`: `switch_root` có thể
    // đã ghi CSR rồi đọc lại lệch — không bao giờ để hart ở root VSpace của TAP).
    let restored = aobj::restore_kernel_root();
    let kernel = restored && aobj::platform_ticks() > 0 && aobj::kernel_root_active();
    // Dọn: VSpace teardown (flush ASID), Frame trả PA.
    let _ = m.revoke(vs);
    flush += flush_all(m);
    let _ = m.revoke(fr);
    let cleanup_ok = m.check_objmem().is_ok();
    let ok = switched
        && restored
        && map_ok
        && write_ok
        && read_ok
        && protect_ok
        && flags_ok
        && unmapped
        && after_invalid
        && kernel
        && flush >= 3
        && cleanup_ok;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# map: va=0x{:x} level=0 pa=0x{:x} satp={} asid={} sum={} write={} read={} protect={} unmapped={} flush={} walk_after={} kernel={}",
            MAP_VA,
            pa,
            switched as u8,
            asid,
            (write_ok && read_ok) as u8,
            write_ok as u8,
            read_ok as u8,
            protect_ok as u8 & unmapped as u8,
            unmapped as u8,
            flush,
            (!after_invalid) as u8,
            kernel as u8
        );
    }
    if check(
        fails,
        ok,
        "map/unmap 4 KiB",
        "satp/map/SUM/protect/unmap/walk khong dat",
    ) {
        PLATFORM.put_str("ok 35 - map/unmap 4 KiB\n");
    }
}

fn tap36_no_leak(m: &mut Mac, u0: CapId, fails: &mut usize) {
    let s = m.untyped_stats(u0).unwrap_or_default();
    let stats = m.check_objmem();
    // Không in cờ do checker tự khai: `check=1` chỉ khi `check_objmem` thật sự trả `Ok`.
    let (check_ok, orphans) = match stats {
        Ok(st) => (true, st.orphans),
        Err(_) => (false, 1),
    };
    let counts = m.counts();
    let ok = check_ok
        && s.used == 0
        && s.free == s.capacity
        && s.holes == 0
        && s.frames == 0
        && counts.frames == 0
        && counts.page_tables == 0
        && counts.vspaces == 0
        && counts.mappings == 0
        && orphans == 0;
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# leak: cap={} used={} free={} holes={} frames={} pts={} vspaces={} maps={} orphans={} check={}",
            s.capacity,
            s.used,
            s.free,
            s.holes,
            counts.frames,
            counts.page_tables,
            counts.vspaces,
            counts.mappings,
            orphans,
            check_ok as u8
        );
    }
    if check(
        fails,
        ok,
        "0 ro",
        "con byte/object/mapping ro hoac check_objmem do",
    ) {
        PLATFORM.put_str("ok 36 - 0 ro\n");
    }
}
