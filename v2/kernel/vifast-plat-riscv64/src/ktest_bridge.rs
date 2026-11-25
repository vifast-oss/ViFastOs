// SPDX-License-Identifier: MIT OR Apache-2.0
//! **Cài `vifast_ktest::CoreBridge` cho riscv64** — KT-411 (`B5`, D2-259 mục 2).
//!
//! Trước KT-411 mỗi arch có một module `core_bridge` (303/270/261 LOC) với **cùng** chữ ký
//! `with_machine`/`machine_addr`/`machine_bytes`/`in_image_region` nhưng **không** có hợp đồng
//! chung, nên bộ ca kiểm core không thể dùng chung. Tệp này chỉ là **adapter mỏng**: mọi thứ
//! chạm `static MACHINE`/CSR vẫn ở [`crate::core_bridge`] (D2-17), còn ngữ nghĩa ca kiểm ở
//! `vifast-ktest`.
//!
//! # Vì sao cần `record`
//!
//! `LinkReport` chứa `base`/`arena_bytes`/`untyped`/`harts` — bốn số **chỉ** `core_bridge::init`
//! biết (nó nhận chúng từ caller). Chúng được ghi vào atomic ngay sau `init` (hàm [`record`]),
//! nên [`CoreBridge::link`] dựng lại được báo cáo **mà không** phải thêm `static mut` hay
//! `unsafe` mới. `footprint`/`objects`/`caps`/`check_*` thì tính lại tại chỗ từ `Machine`.
//!
//! # Chỉ ảnh kiểm
//!
//! Toàn bộ tệp nằm sau `#[cfg(feature = "ktest")]` (xem `lib.rs`): ảnh **sản phẩm** không link
//! `vifast-ktest` (D2-08), nên 0 byte/0 chuỗi harness đi vào ảnh sản phẩm.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use vifast_core::Machine;
use vifast_ktest::{CoreBridge, FrameLabel, LinkReport, ObjVmReport};
use vifast_plat_common::obj::{CapId, ObjKind};

use crate::core_bridge::{self, CoreReport};
use crate::obj as aobj;
use crate::Riscv64Virt;

/// Đáy Untyped boot (nhãn PA, D2-83) — ghi bởi [`record`].
static LINK_BASE: AtomicU64 = AtomicU64::new(0);
/// Cỡ Untyped boot — ghi bởi [`record`].
static LINK_BYTES: AtomicU64 = AtomicU64::new(0);
/// Object Untyped của core — ghi bởi [`record`].
static LINK_UNTYPED: AtomicU32 = AtomicU32::new(0);
/// Số hart nền tảng — ghi bởi [`record`].
static LINK_HARTS: AtomicU64 = AtomicU64::new(1);
/// Cap Untyped `U0` của **object layer** — ghi bởi [`record`] (handle `u16` của `obj::CapId`).
static OBJ_UNTYPED0: AtomicU32 = AtomicU32::new(0);
/// Cap Frame cấp gần nhất cho TAP 50 (để `obj_free_frame` trả đúng cap).
static LAST_FRAME_CAP: AtomicU32 = AtomicU32::new(u32::MAX);

/// Handle `obj::CapId` (`u16`) ⇄ `u32` cho atomic — `u32::MAX` = "không có".
fn cap_of(raw: u32) -> Option<CapId> {
    if raw == u32::MAX {
        None
    } else {
        Some(CapId(raw as u16))
    }
}

/// Ghi bốn số **chỉ** `core_bridge::init`/`obj::init` biết, để [`CoreBridge::link`] dựng lại
/// báo cáo mà không cần `static mut`.
///
/// Gọi **một lần** ngay sau `core_bridge::init(...)` + `obj::init()` trong pha core của ảnh kiểm.
pub fn record(report: &CoreReport, obj: &crate::obj::ObjReport) {
    LINK_BASE.store(report.base, Ordering::Release);
    LINK_BYTES.store(report.arena_bytes as u64, Ordering::Release);
    LINK_UNTYPED.store(report.untyped, Ordering::Release);
    LINK_HARTS.store(report.harts as u64, Ordering::Release);
    OBJ_UNTYPED0.store(u32::from(obj.untyped0.0), Ordering::Release);
}

/// Dựng [`LinkReport`] từ trạng thái **hiện tại** của `Machine` + bốn số đã [`record`].
///
/// Tách khỏi `impl` để ảnh kiểm gọi được **trước** khi lấy khoá (TAP 43 không cần khoá).
pub fn link_report() -> LinkReport {
    let untyped = LINK_UNTYPED.load(Ordering::Acquire);
    core_bridge::with_machine(|m| LinkReport {
        addr: core_bridge::machine_addr(),
        static_bytes: core_bridge::machine_bytes(),
        pin: core_bridge::CORE_PIN_BYTES,
        base: LINK_BASE.load(Ordering::Acquire),
        arena_bytes: LINK_BYTES.load(Ordering::Acquire) as usize,
        untyped,
        harts: LINK_HARTS.load(Ordering::Acquire) as usize,
        objects: m.object_count(),
        caps: m.live_cap_count(),
        check_caps: m.check_caps().is_ok(),
        check_bytes: m.check_bytes(untyped).is_ok(),
        check_all_bytes: m.check_all_bytes().is_ok(),
        footprint: Machine::footprint(),
        in_image: core_bridge::in_image_region(
            core_bridge::machine_addr(),
            core_bridge::machine_bytes(),
        ),
    })
}

/// **KT-412** — cài [`vifast_ktest::smp_suite::SmpBridge`] cho ảnh kiểm `riscv64`.
///
/// Chỉ **ba** thứ bộ ca SMP cần: khoá `Machine`, số hart **thật** lúc boot (`BootInfo::n_harts`
/// mà `core_bridge::init` đã dùng để `set_harts`), và `ObjId` Untyped boot. Không cần object
/// layer ⇒ `aarch64` (không có `obj`) cũng cài được **cùng** hợp đồng.
impl vifast_ktest::smp_suite::SmpBridge for Riscv64Virt {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        core_bridge::with_machine(f)
    }

    fn boot_harts(&self) -> usize {
        LINK_HARTS.load(Ordering::Acquire) as usize
    }

    fn untyped(&self) -> vifast_core::ObjId {
        LINK_UNTYPED.load(Ordering::Acquire)
    }
}

impl CoreBridge for Riscv64Virt {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        core_bridge::with_machine(f)
    }

    fn link(&self) -> LinkReport {
        link_report()
    }

    fn ram_window(&self) -> (u64, u64) {
        (crate::mmu::RAM_PHYS_BASE, crate::mmu::RAM_BYTES)
    }

    fn obj_alloc_frame(&self) -> Option<FrameLabel> {
        let untyped0 = CapId(OBJ_UNTYPED0.load(Ordering::Acquire) as u16);
        let om = aobj::machine();
        let cap = om.retype(untyped0, ObjKind::Frame, 12).ok()?;
        LAST_FRAME_CAP.store(u32::from(cap.0), Ordering::Release);
        let pa = om.frame_pa(cap).unwrap_or(0);
        let budget = om.frame_budget(untyped0).unwrap_or_default();
        Some(FrameLabel {
            pa,
            label: (pa >> 12) as u32,
            window_base: budget.base,
            window_used: budget.used,
        })
    }

    fn obj_free_frame(&self, _label: u32) {
        if let Some(cap) = cap_of(LAST_FRAME_CAP.swap(u32::MAX, Ordering::AcqRel)) {
            let _ = aobj::machine().revoke(cap);
        }
    }

    fn obj_vm_selftest(&self) -> ObjVmReport {
        // D2-83: object layer **vẫn** là chủ sở hữu Frame/PageTable/VSpace — `init` lại để phép
        // đo độc lập với các object TAP 44..50 đã revoke.
        let obj = aobj::init();
        let om = aobj::machine();
        let ovs = om.retype(obj.untyped0, ObjKind::VSpace, 0);
        let ofr = om.retype(obj.untyped0, ObjKind::Frame, 12);
        let health = om.check_objmem().is_ok();
        let counts = om.counts();
        let vspace = ovs.is_ok();
        let frame = ofr.is_ok();
        if let Ok(cap) = ofr {
            let _ = om.revoke(cap);
        }
        if let Ok(cap) = ovs {
            let _ = om.revoke(cap);
        }
        ObjVmReport {
            vspace,
            frame,
            health,
            frames: counts.frames,
            vspaces: counts.vspaces,
            clean: om.check_objmem().is_ok(),
        }
    }
}
