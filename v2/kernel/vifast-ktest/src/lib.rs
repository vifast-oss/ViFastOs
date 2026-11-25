// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_std]
#![forbid(unsafe_code)]
//! `vifast-ktest` — **một** bộ ca kiểm dùng chung cho 3 arch (`B5`, KT-411, D2-259 mục 2).
//!
//! # Vì sao crate này tồn tại
//!
//! Ba ảnh kiểm đang chép lại cùng một bộ khẳng định trên `vifast_core::Machine`:
//! `vifast-kbin-riscv64/src/ktest_core.rs` (1.090 LOC), `vifast-kbin-aarch64/src/coretest.rs`
//! (845), `vifast-kbin-x86_64/src/coretest.rs` (1.079). Thêm một `check_*` mới ở core, hay đổi
//! một nhãn TAP, phải sửa **ba** nơi — và đã trôi thật (nhãn/thứ tự khác nhau giữa ba bản).
//!
//! Crate này giữ **một** bản, generic trên hai hợp đồng **dùng chung**:
//!
//! - [`vifast_plat_common::Platform`] — in/halt/đồng hồ (KT-44, đã có);
//! - [`CoreBridge`] — `with_machine` + báo cáo link + cửa sổ RAM + object layer. Trước KT-411
//!   đây là **ba** module `core_bridge` riêng (303/270/261 LOC) với cùng chữ ký nhưng không có
//!   hợp đồng chung, nên không thể viết một bộ ca dùng chung.
//!
//! # Vì sao `CoreBridge` ở **đây** mà không ở `vifast-plat-common`
//!
//! `vifast-plat-common` **không** phụ thuộc `vifast-core` (quyết định KT-409b: giữ crate đó
//! arch-neutral, một chiều D2-83). Hợp đồng này nhắc tới `vifast_core::Machine`/`Footprint` nên
//! phải nằm ở crate **có** phụ thuộc core. Ảnh **sản phẩm** không link crate này (D2-08): crate
//! arch chỉ bật `vifast-ktest` dưới feature `ktest`, nên ảnh sản phẩm vẫn 0 harness.
//!
//! # Không `unsafe`, không asm, không MMIO
//!
//! [`#![forbid(unsafe_code)]`] — mọi thứ chạm CSR/asm/MMIO nằm ở crate arch (D2-17). Nhờ vậy
//! **toàn bộ** bộ ca chạy được trên host (`cargo test -p vifast-ktest`), đúng AGENTS §4.3.

pub mod core_suite;
pub mod invariants;
pub mod smp_suite;

use vifast_core::{Footprint, Machine};

/// Pin **D2-84**: `size_of::<Machine>()` ≤ **196.608 B** (192 KiB) = trần ảnh 262.144 B trừ
/// 64 KiB headroom. Vượt ⇒ tối ưu cấu trúc, **không** nâng pin.
pub const CORE_PIN_BYTES: usize = 196_608;

/// Trần **ảnh link** (QD §3.5 / D2-85a): `.data+.sdata+.bss+.sbss` ≤ **262.144 B** — ngưỡng
/// **khác** pin trên, `budget.sh` đo trên ELF thật (không đọc lẫn).
pub const IMAGE_CAP_BYTES: usize = 262_144;

/// Cỡ vùng RAM **nhãn** cấp cho Untyped boot của core (bắt đầu ngay sau arena object layer).
/// Core chỉ kế toán byte (D2-83) — không dereference.
pub const CORE_ARENA_BYTES: usize = 64 * 1024;

/// Báo cáo **link** của `Machine` trong ảnh — hợp đồng chung thay cho ba `CoreReport` riêng.
///
/// Mỗi arch dựng kiểu này từ `core_bridge::init` của mình (xem `impl From<CoreReport>` ở crate
/// arch); bộ ca chung chỉ đọc, không tự suy ra số nào.
#[derive(Clone, Copy, Debug)]
pub struct LinkReport {
    /// Địa chỉ `static MACHINE` trong ảnh.
    pub addr: u64,
    /// `size_of::<Machine>()`.
    pub static_bytes: usize,
    /// Pin D2-84.
    pub pin: usize,
    /// Đáy Untyped boot (nhãn PA, D2-83).
    pub base: u64,
    /// Cỡ Untyped boot.
    pub arena_bytes: usize,
    /// Object Untyped của core.
    pub untyped: vifast_core::ObjId,
    /// Số hart **nền tảng** đã truyền vào `Machine::init_zeroed` (D2-88(a)).
    pub harts: usize,
    /// Số object trong bảng core.
    pub objects: usize,
    /// Số cap sống.
    pub caps: usize,
    /// `check_caps` ĐẠT.
    pub check_caps: bool,
    /// `check_bytes(untyped)` ĐẠT.
    pub check_bytes: bool,
    /// `check_all_bytes` ĐẠT.
    pub check_all_bytes: bool,
    /// Bảng phân bố byte (D2-84/D2-85c).
    pub footprint: Footprint,
    /// `true` ⇔ static nằm trong vùng ảnh kernel (không phải arena/RAM ngoài).
    pub in_image: bool,
}

impl LinkReport {
    /// `true` ⇔ mọi bất biến tầng 1 xanh + pin D2-84 + static trong ảnh.
    pub fn healthy(&self) -> bool {
        self.static_bytes <= self.pin
            && self.in_image
            && self.check_caps
            && self.check_bytes
            && self.check_all_bytes
    }
}

/// Một Frame do **object layer** cấp (D2-83: core không sở hữu Frame/PageTable/VSpace).
///
/// `window_base`/`window_used` là cửa sổ PA mà allocator của Untyped đã cấp — **nguồn khác**
/// `pa`, nên phép kiểm "PA nằm trong cửa sổ" không phải kiểm một giá trị với chính nó (D2-66).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLabel {
    /// PA thật của Frame.
    pub pa: u64,
    /// Nhãn futex = PPN của `pa` (`pa >> 12`).
    pub label: u32,
    /// Đáy cửa sổ PA của allocator.
    pub window_base: u64,
    /// Số byte đã cấp trong cửa sổ.
    pub window_used: u64,
}

/// Kết quả phép tự kiểm **object layer** (chủ sở hữu VM) trong TAP 51.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjVmReport {
    /// Retype `VSpace` ĐẠT.
    pub vspace: bool,
    /// Retype `Frame` ĐẠT.
    pub frame: bool,
    /// `check_objmem` ĐẠT **trước** revoke.
    pub health: bool,
    /// Số Frame sống.
    pub frames: usize,
    /// Số VSpace sống.
    pub vspaces: usize,
    /// `check_objmem` ĐẠT **sau** revoke (không rò).
    pub clean: bool,
}

/// Hợp đồng **tối thiểu** cho bộ ca SMP dùng chung — **hai** method, không cần object layer.
///
/// # Vì sao tách khỏi [`CoreBridge`]
///
/// [`CoreBridge`] mang cả object layer (`obj_alloc_frame`/`obj_vm_selftest`) vì bộ ca **core**
/// (TAP 50/51) cần Frame/VSpace thật. Ảnh `x86_64` **chưa** có object layer trong kernel
/// (`KT-340c` chỉ nối `Machine`), nên nếu bộ ca SMP đòi cả [`CoreBridge`] thì ảnh phải cài ba
/// method **giả** — đúng thứ `D2-83`/`D2-102` cấm ("không bịa bằng chứng"). Tách một hợp đồng
/// hẹp hơn giữ mỗi arch cài **đúng** thứ nó có.
///
/// Blanket impl dưới đây bảo đảm mọi `CoreBridge` **tự động** là `SmpBridge` ⇒ bộ ca SMP chạy
/// được trên nền tảng giả của host (đã cài `CoreBridge`) mà không cần impl thứ hai.
pub trait SmpBridge {
    /// Chạy `f` với **`&mut Machine` duy nhất** của ảnh (khoá lá, không re-entrant).
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R;

    /// Báo cáo link của `Machine` trong ảnh.
    fn link(&self) -> LinkReport;
}

impl<T: CoreBridge + ?Sized> SmpBridge for T {
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R {
        CoreBridge::with_machine(self, f)
    }

    fn link(&self) -> LinkReport {
        CoreBridge::link(self)
    }
}

/// Hợp đồng **cầu nối core** mà mỗi arch cài — thay cho ba module `core_bridge` không chung
/// chữ ký trước KT-411.
///
/// Cài đặt nằm ở crate arch (nơi có `static MACHINE` + asm/CSR), **không** ở đây: crate này
/// `forbid(unsafe_code)` và chạy được trên host.
pub trait CoreBridge {
    /// Chạy `f` với **`&mut Machine` duy nhất** của ảnh (khoá lá, không re-entrant).
    fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> R;

    /// Báo cáo link của `Machine` trong ảnh (dựng bởi `core_bridge::init` của arch).
    fn link(&self) -> LinkReport;

    /// Dải RAM **vật lý** thật của nền tảng `(base, bytes)` — dùng để kiểm PA của Frame nằm
    /// trong RAM (TAP 50).
    fn ram_window(&self) -> (u64, u64);

    /// Cấp một Frame từ **object layer** và trả PA + nhãn + cửa sổ allocator (TAP 50).
    fn obj_alloc_frame(&self) -> Option<FrameLabel>;

    /// Trả Frame về object layer theo nhãn (TAP 50).
    fn obj_free_frame(&self, label: u32);

    /// Tự kiểm object layer: tạo/revoke `VSpace` + `Frame` **thật** (TAP 51).
    fn obj_vm_selftest(&self) -> ObjVmReport;
}
