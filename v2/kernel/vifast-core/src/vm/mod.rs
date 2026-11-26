// SPDX-License-Identifier: MIT OR Apache-2.0
//! Object layer **nội bộ** của K3 (KT-52): Untyped boot + Retype/Revoke cho
//! `PageTable`/`Frame`/`VSpace`, cap/key tối thiểu và VSpace map/unmap/protect trên PTE Sv39.
//!
//! **KT-417b (`D2-262` `Q2(c)`, bước 1/2)** — module này **dời** từ
//! `vifast-plat-common/src/obj/` sang `vifast-core/src/vm/` (giữ **nguyên** mã + ngữ nghĩa):
//! từ nay **một** nguồn sự thật cho object/VM nằm trong crate core, `plat-common::obj` chỉ còn
//! là **shim re-export mỏng** (`pub use vifast_core::vm::*`) cho 33 caller tới khi `KT-417d`
//! (W8a) hợp nhất bảng VM vào `KernelStore` + xoá shim. `vifast-core` nhận thêm hai phụ thuộc
//! **thuần acyclic** `vifast-pte` + `vifast-alloc` (0 crate ngoài, D2-21) và giữ nguyên
//! `#![forbid(unsafe_code)]`, 100 % truy cập bộ nhớ qua trait [`PhysMem`].
//!
//! # Phạm vi (D2-66, D2-67, KT-52)
//!
//! - **Không** ABI, **không** `spec/`: `Retype` ở đây là API nội bộ kernel (không phải
//!   `Untyped.Retype` của ABI); `VSpace` vẫn `reserved`/`unsupported` trong `spec/abi.toml` —
//!   task KT-56 mới mở `spec/` một lần cho `FaultKind`.
//! - **Không** CNode/MDB/TCB/IPC/sched (K3b KT-58..61), **không** U-mode (KT-53). Cap ở đây là
//!   **handle + key tối thiểu**: bảng phẳng trong `.bss`, mỗi cap có `obj` (object nó trỏ),
//!   `parent` (cap sinh ra nó — đủ để `Revoke` đi xuống), `id` duy nhất (key) và `rights`.
//! - **Không** `unsafe`: mọi truy cập bộ nhớ đi qua trait [`PhysMem`] (arch cài bằng
//!   `physmap`/asm — D2-17); nhờ vậy toàn bộ logic chạy được **trên host** (`tests/obj.rs`)
//!   và không cần `alloc`.
//!
//! # Mô hình byte (bất biến KT-52)
//!
//! Mỗi Untyped có **hai** không gian tách biệt, đúng D2-66:
//!
//! 1. **Arena byte** — vùng RAM thật do Untyped sở hữu; object *tiêu byte* ở đây:
//!    `PageTable` = 4 KiB (một bảng PTE 512 × 8 B), `VSpace` = root PageTable 4 KiB; metadata
//!    object/cap nằm trong bảng tĩnh `.bss` (có biên, không cấp phát động) — nợ K3b: đưa cả
//!    header vào Untyped.
//! 2. **Cửa sổ PA** — `UNTYPED_PA_WINDOW = 1 GiB` `[TK]` (D2-66): `Frame` **không** tiêu byte
//!    arena; `pa = pa_base + offset` là **nhãn PA** trong cửa sổ này (4 KiB / 2 MiB / 1 GiB),
//!    cấp theo watermark + free-list, không chồng lấn **toàn cục** giữa mọi Frame sống.
//!
//! Bất biến [`Machine::check_objmem`] (tương đương `check_vm` của K2, chạy cả `no_std`):
//!
//! - `Σ byte object sống + Σ hole == watermark ≤ capacity` (mọi byte trong watermark là object
//!   sống hoặc hole tự do — **không** có byte "gap" tự khai);
//! - Frame: `pa` căn đúng cỡ, nằm trong cửa sổ, **không chồng** Frame sống khác, `pa_used`
//!   khớp `Σ cỡ Frame + Σ PA hole`;
//! - **không frame mồ côi**: mọi object sống phải có cap sống (Frame/PageTable/VSpace) hoặc là
//!   bảng nội bộ của một VSpace sống;
//! - **W^X toàn cục** (D2-67): `mapped_w == 0` hoặc `mapped_x == 0` cho mọi Frame; `Device +
//!   EXEC` bị từ chối ngay từ lúc map/protect;
//! - **walk khớp**: mỗi mapping đi lại bằng PTE thật ra đúng `phys` + cấp lá ứng với `size_bits`
//!   (`walk_shape` của `vifast-pte`), nên hình 3 cấp không thể lệch khỏi dữ liệu mapping.
//!
//! # Vì sao Frame không tiêu byte arena
//!
//! D2-66 chốt `Frame.pa` là **nhãn PA** trong cửa sổ 1 GiB, không phải vùng nhớ cấp phát; trên
//! QEMU `virt -m 128M` (D2-72 Q2) một Frame 1 GiB **không thể** có backing thật trong 128 MiB
//! RAM, nên nếu bắt Frame tiêu byte thì cỡ 1 GiB bất khả thi. KT-52 giữ đúng D2-66: Frame tiêu
//! **0 byte arena**, nhưng 4 KiB/2 MiB vẫn được **ghi/đọc thật** qua `physmap` trong TAP (địa
//! chỉ `pa` là RAM thật); 1 GiB chỉ kiểm nhãn/căn lề/cửa sổ — ghi rõ trong
//! `v2/docs/k3-riscv64.md` §15.

use vifast_alloc::FrameArena;
use vifast_pte::{FrameSize, PAGE_SIZE_BITS};

// **KT-409** — bit PTE Sv39 nay ở `vifast-pte` (định dạng — crate thuần, không `unsafe`).
// **KT-417b** — module ở trong `vifast-core` nên **không** được phụ thuộc `vifast-plat-riscv64`
// (giữ chiều phụ thuộc D2-83: plat → core, không ngược lại).
use vifast_pte::{
    PTE_ACCESSED, PTE_DIRTY, PTE_ENTRIES, PTE_EXEC, PTE_READ, PTE_USER, PTE_VALID, PTE_WRITE,
};

mod arena;
mod check;
mod vspace;

use arena::{Hole, Untyped};

/// Cửa sổ PA của một Untyped: **1 GiB** `[TK]` (D2-66) — `Frame.pa = pa_base + offset`.
///
/// KT-67: giá trị **không** còn là hằng thứ hai — lấy từ [`vifast_alloc::WINDOW_BYTES`], nơi
/// allocator Frame duy nhất định nghĩa cửa sổ.
pub const UNTYPED_PA_WINDOW: u64 = vifast_alloc::WINDOW_BYTES as u64;

/// Byte dữ liệu của một PageTable/VSpace-root: **4096** (512 PTE × 8 B).
pub const PAGE_TABLE_BYTES: u32 = 4096;

/// Số object tối đa (bảng tĩnh trong `.bss`, không cấp phát động).
pub const MAX_OBJECTS: usize = 96;
/// Số cap tối đa.
pub const MAX_CAPS: usize = 160;
/// Số Untyped boot tối đa.
pub const MAX_UNTYPEDS: usize = 4;
/// Số mapping VSpace cùng lúc.
pub const MAX_MAPPINGS: usize = 64;
/// Số hole mỗi không gian (arena/PA) của một Untyped — **cùng** hằng với allocator Frame
/// (KT-67: một nguồn, không khai lại).
pub use vifast_alloc::MAX_HOLES;
/// Số yêu cầu flush (`sfence.vma`) đang chờ.
pub const MAX_FLUSHES: usize = 16;

/// `CapId` không có (slot trống/handle nội bộ).
pub const CAP_NONE: u16 = u16::MAX;
/// Object nội bộ (không có cap) hay chưa gán.
pub const OBJ_NONE: u16 = u16::MAX;
/// Untyped index nội bộ chưa gán.
const U_NONE: u8 = u8::MAX;
/// VSpace owner của PageTable chưa gán (bảng rời có cap).
const VS_NONE: u8 = u8::MAX;

/// Quyền tối thiểu của cap (chưa có IPC — KT-58..61).
pub const RIGHT_READ: u8 = 1 << 0;
/// Quyền ghi: `Retype` trên Untyped cần quyền này (giống K2 KT-10 F3).
pub const RIGHT_WRITE: u8 = 1 << 1;
/// Quyền thực thi (chưa dùng ở KT-52; giữ đủ bộ ba để K3b mở rộng).
pub const RIGHT_EXEC: u8 = 1 << 2;
/// Đủ ba quyền.
pub const RIGHTS_ALL: u8 = RIGHT_READ | RIGHT_WRITE | RIGHT_EXEC;

/// Loại object của object layer K3 (không phải bảng `[object]` của `spec/`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjKind {
    /// Vùng nhớ nguồn (boot Untyped) — object gốc, có `capacity` byte arena + cửa sổ PA.
    Untyped,
    /// Khung vật lý 4 KiB / 2 MiB / 1 GiB (nhãn PA trong cửa sổ, D2-66).
    Frame,
    /// Bảng trang 3 cấp Sv39 — 4 KiB PTE thật trong arena.
    PageTable,
    /// Không gian địa chỉ: root PageTable + ASID nhãn + bảng mapping.
    VSpace,
}

impl ObjKind {
    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            ObjKind::Untyped => "Untyped",
            ObjKind::Frame => "Frame",
            ObjKind::PageTable => "PageTable",
            ObjKind::VSpace => "VSpace",
        }
    }
}

/// Lỗi **nội bộ** của object layer. Tên lấy đúng `name` các mã `[error]` của `spec/abi.toml`
/// khi trùng nghĩa (log không sinh tên thứ hai), nhưng **không** phải mã ABI và không đổi
/// `spec/`. `NoSlots`/`Corrupt`/`NotSupported` là lỗi cục bộ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjError {
    /// Handle không trỏ cap sống.
    InvalidCapability,
    /// Cap/object sai loại cho thao tác.
    WrongObjectType,
    /// Cap thiếu quyền (`Retype` cần `RIGHT_WRITE`).
    InsufficientRights,
    /// Tham số sai (`size_bits` lạ, cờ không hợp lệ).
    InvalidArgument,
    /// Untyped hết chỗ (arena hoặc cửa sổ PA).
    NotEnoughMemory,
    /// `pa`/`va` không căn đúng cỡ.
    Alignment,
    /// VA đã có mapping (hoặc Frame PA chồng Frame sống khác).
    AlreadyMapped,
    /// VA chưa có mapping.
    NotMapped,
    /// Object còn con (Frame còn mapping, PageTable gắn VSpace) ⇒ `Revoke` từ chối.
    HasChildren,
    /// W^X: map/giữ `W` khi Frame đã từng map `X` (hoặc ngược lại), hoặc `Device + EXEC`.
    WxViolation,
    /// Chỉ số ngoài biên (VA/cap/object).
    RangeError,
    /// Bảng tĩnh hết slot (object/cap/mapping/flush) — lỗi cục bộ, không phải ABI.
    NoSlots,
    /// Dữ liệu PTE/arena hỏng (đọc bộ nhớ thất bại, hole chồng lấn…).
    Corrupt,
    /// Loại object/thao tác chưa hỗ trợ (vd. `Retype(Untyped)` — K3b KT-58).
    NotSupported,
}

impl ObjError {
    /// Mã lỗi ABI (`spec/abi.toml` `[error]`) tương ứng — `None` cho lỗi **nội bộ** không có mã
    /// ABI (`NoSlots`/`Corrupt`). KT-56 dùng để chứng minh `WxViolation` đi đúng mã **4** qua
    /// đường công khai (không phải chuỗi tự khai).
    pub const fn abi_code(self) -> Option<u16> {
        match self {
            ObjError::InvalidCapability => Some(1),
            ObjError::WrongObjectType => Some(2),
            ObjError::InsufficientRights => Some(3),
            ObjError::WxViolation => Some(4),
            ObjError::InvalidArgument => Some(5),
            ObjError::Alignment => Some(6),
            ObjError::NotEnoughMemory => Some(9),
            ObjError::AlreadyMapped => Some(10),
            ObjError::NotMapped => Some(11),
            ObjError::HasChildren => Some(13),
            ObjError::NotSupported => Some(18),
            ObjError::RangeError => Some(19),
            ObjError::NoSlots | ObjError::Corrupt => None,
        }
    }

    /// Tên ổn định cho log/kiểm thử (trùng `spec/abi.toml` khi có).
    pub const fn code_name(self) -> &'static str {
        match self {
            ObjError::InvalidCapability => "InvalidCapability",
            ObjError::WrongObjectType => "WrongObjectType",
            ObjError::InsufficientRights => "InsufficientRights",
            ObjError::InvalidArgument => "InvalidArgument",
            ObjError::NotEnoughMemory => "NotEnoughMemory",
            ObjError::Alignment => "Alignment",
            ObjError::AlreadyMapped => "AlreadyMapped",
            ObjError::NotMapped => "NotMapped",
            ObjError::HasChildren => "HasChildren",
            ObjError::WxViolation => "WxViolation",
            ObjError::RangeError => "RangeError",
            ObjError::NoSlots => "NoSlots",
            ObjError::Corrupt => "Corrupt",
            ObjError::NotSupported => "NotSupported",
        }
    }
}

/// Cờ của một mapping (không phải bit PTE; `device` là **chính sách** vì Sv39 không có bit
/// Device — MMIO chỉ khác leaf thường ở chỗ cấm `X`; K3 dùng `A|D` cho mọi leaf để trap không
/// phải xử lý fault phụ).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapFlags {
    /// Có quyền đọc (`R`) — bắt buộc với mọi leaf Sv39.
    pub read: bool,
    /// Có quyền ghi (`W`).
    pub write: bool,
    /// Có quyền thực thi (`X`).
    pub exec: bool,
    /// Leaf dành cho U-mode (`U`) — KT-53 mới chạy thật; KT-52 kiểm bit + tự thử qua SUM.
    pub user: bool,
    /// MMIO/Device: cấm `X` (D2-67).
    pub device: bool,
}

impl MapFlags {
    /// `R` + `W`, **NX** (dữ liệu thường).
    pub const RW_NX: Self = Self {
        read: true,
        write: true,
        exec: false,
        user: false,
        device: false,
    };
    /// `R` + `X` (mã kernel).
    pub const RX: Self = Self {
        read: true,
        write: false,
        exec: true,
        user: false,
        device: false,
    };
    /// `R` + `W` + `U`, **NX** (leaf user đầu tiên).
    pub const RW_NX_USER: Self = Self {
        read: true,
        write: true,
        exec: false,
        user: true,
        device: false,
    };
    /// `R` + `X` + `U` (mã user; KT-53 dùng).
    pub const RX_USER: Self = Self {
        read: true,
        write: false,
        exec: true,
        user: true,
        device: false,
    };
    /// `R` + `W` **Device**, NX.
    pub const DEVICE_RW: Self = Self {
        read: true,
        write: true,
        exec: false,
        user: false,
        device: true,
    };
    /// `R` + `X` + **Device** — ca `Device+EXEC` phải bị `WxViolation` (D2-67).
    pub const DEVICE_RX: Self = Self {
        read: true,
        write: false,
        exec: true,
        user: false,
        device: true,
    };

    /// Leaf hợp lệ: ít nhất một quyền và `W ⇒ R` (Sv39 dành `W=1,R=0`).
    pub const fn is_valid(self) -> bool {
        (self.read || self.write || self.exec) && !(self.write && !self.read)
    }

    /// Bit PTE (không có bit Device — chính sách ở [`MapFlags::device`]).
    pub const fn bits(self) -> u64 {
        let mut bits = PTE_VALID | PTE_ACCESSED | PTE_DIRTY;
        if self.read {
            bits |= PTE_READ;
        }
        if self.write {
            bits |= PTE_WRITE;
        }
        if self.exec {
            bits |= PTE_EXEC;
        }
        if self.user {
            bits |= PTE_USER;
        }
        bits
    }

    /// Cờ khớp đúng `R|W|X|U` của PTE thô.
    pub fn matches_pte(self, raw: u64) -> bool {
        let mask = PTE_READ | PTE_WRITE | PTE_EXEC | PTE_USER;
        (self.bits() & mask) == (raw & mask)
    }
}

/// Một yêu cầu xả TLB do object layer sinh ra; arch (hoặc test host) thực thi bằng `sfence.vma`
/// theo `(asid, va)`/`(asid)`/`(*)` — KT-51 §14.2. Không tự gọi asm ở crate platform-common.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlushRequest {
    /// `sfence.vma va, asid` — một VA (map/unmap/protect).
    Page {
        /// ASID nhãn của VSpace.
        asid: u16,
        /// VA vừa đổi PTE.
        va: u64,
    },
    /// `sfence.vma zero, asid` — teardown VSpace.
    Asid {
        /// ASID cần xả toàn bộ.
        asid: u16,
    },
    /// `sfence.vma` — mọi ASID.
    All,
}

/// Truy cập bộ nhớ vật lý — phần `unsafe`/MMIO nằm ở arch (D2-17). Trả `None`/`false` thay vì
/// panic để lỗi PTE trở thành [`ObjError::Corrupt`] (không bao giờ đọc rác).
pub trait PhysMem {
    /// Đọc 8 B tại `pa`; `None` nếu `pa` không truy cập được.
    fn read_u64(&self, pa: u64) -> Option<u64>;
    /// Ghi 8 B tại `pa`; `false` nếu không ghi được.
    fn write_u64(&mut self, pa: u64, value: u64) -> bool;
}

/// Ảnh chụp thống kê của object layer (in TAP; không cấp phát).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjCounts {
    /// Object sống (mọi loại).
    pub objects: usize,
    /// Cap sống.
    pub caps: usize,
    /// Untyped (boot).
    pub untypeds: usize,
    /// PageTable sống (kể cả root/bảng nội bộ của VSpace).
    pub page_tables: usize,
    /// Frame sống.
    pub frames: usize,
    /// VSpace sống.
    pub vspaces: usize,
    /// Mapping sống.
    pub mappings: usize,
}

/// Thống kê byte/PA của **một** Untyped — nguồn cho TAP "untyped byte invariant".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UntypedStats {
    /// Cỡ arena (byte).
    pub capacity: usize,
    /// Tổng `len` object sống tiêu byte arena (không gồm Frame).
    pub used: usize,
    /// `capacity - used` — byte còn tự do (gồm hole + đuôi).
    pub free: usize,
    /// Số hole arena (byte đệm căn lề cũng là hole).
    pub holes: usize,
    /// Watermark arena (byte đã bump).
    pub watermark: usize,
    /// Frame PA đã dùng trong cửa sổ (byte).
    pub pa_used: u64,
    /// Số Frame sống của Untyped này.
    pub frames: usize,
    /// Số object sống khác Frame.
    pub objects: usize,
}

/// Ảnh chụp **cửa sổ PA** của một Untyped (KT-67) — view của allocator Frame dùng chung
/// ([`vifast_alloc::FrameArena`]) cho cầu nối D2-83 và memsrv.
///
/// Không phải allocator thứ hai: mọi trường đọc thẳng từ trạng thái mà `check_objmem` soát.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameBudget {
    /// Đáy cửa sổ PA (nhãn).
    pub base: u64,
    /// Cỡ cửa sổ (byte).
    pub capacity: u64,
    /// Watermark (byte đã chạm).
    pub used: u64,
    /// Tổng byte hole.
    pub hole_bytes: u64,
    /// Byte cấp lại được (`capacity - Σ Frame sống`).
    pub free_bytes: u64,
    /// Số hole.
    pub holes: u8,
    /// Frame 4 KiB đang sống.
    pub live_4k: u16,
    /// Frame 2 MiB đang sống.
    pub live_2m: u16,
    /// Frame 1 GiB đang sống.
    pub live_1g: u16,
    /// Số lần cấp thành công.
    pub allocs: u32,
    /// Số lần trả thành công.
    pub frees: u32,
}

/// Kết quả đi PTE phần mềm trên root của một VSpace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalkInfo {
    /// Entry hợp lệ.
    pub valid: bool,
    /// Entry là leaf (có `R|W|X`).
    pub leaf: bool,
    /// Cấp dừng (0/1/2; invalid ở cấp này).
    pub level: u8,
    /// PA đáy leaf (0 nếu invalid/non-leaf).
    pub phys: u64,
    /// Bit thô của entry dừng.
    pub raw: u64,
}

/// Ảnh chụp của [`Machine::check_objmem`]: mọi checker ĐẠT (nếu lỗi thì trả `Err` trước đó).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjStats {
    /// Số object/cap/... (xem [`ObjCounts`]).
    pub counts: ObjCounts,
    /// `Σ capacity` arena của mọi Untyped.
    pub bytes_cap: usize,
    /// `Σ used` của mọi Untyped.
    pub bytes_used: usize,
    /// `Σ free` của mọi Untyped.
    pub bytes_free: usize,
    /// Số mapping đã đi lại + đối chiếu PTE.
    pub walked: usize,
    /// Object sống không có cap/cụm nội bộ hợp lệ (phải 0).
    pub orphans: usize,
}

/// Handle cap — chỉ số trong bảng cap tĩnh; `Copy`, không mang quyền.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CapId(pub u16);

impl CapId {
    /// Handle không có.
    pub const NONE: CapId = CapId(CAP_NONE);
}
#[derive(Clone, Copy)]
struct Object {
    live: bool,
    kind: ObjKind,
    id: u32,
    untyped: u8,
    cap: u16,
    off: u32,
    len: u32,
    pa_off: u32,
    size_bits: u8,
    root: u16,
    owner: u8,
    maps: u16,
    mapped_w: u16,
    mapped_x: u16,
    /// Số mapping **Device** đang sống trên Frame (D2-67 toàn cục — KT-56, nợ L3 KT-52).
    mapped_device: u16,
    asid: u16,
}

impl Object {
    const EMPTY: Self = Self {
        live: false,
        kind: ObjKind::Untyped,
        id: 0,
        untyped: U_NONE,
        cap: CAP_NONE,
        off: 0,
        len: 0,
        pa_off: 0,
        size_bits: 0,
        root: OBJ_NONE,
        owner: VS_NONE,
        maps: 0,
        mapped_w: 0,
        mapped_x: 0,
        mapped_device: 0,
        asid: 0,
    };
}

#[derive(Clone, Copy)]
struct Cap {
    live: bool,
    obj: u16,
    kind: ObjKind,
    rights: u8,
    parent: u16,
    id: u32,
}

impl Cap {
    const EMPTY: Self = Self {
        live: false,
        obj: OBJ_NONE,
        kind: ObjKind::Untyped,
        rights: 0,
        parent: CAP_NONE,
        id: 0,
    };
}

#[derive(Clone, Copy)]
struct Mapping {
    live: bool,
    vspace: u8,
    frame: u16,
    va: u64,
    level: u8,
    flags: u8,
}

impl Mapping {
    const EMPTY: Self = Self {
        live: false,
        vspace: 0,
        frame: OBJ_NONE,
        va: 0,
        level: 0,
        flags: 0,
    };

    fn new(vspace: u8, frame: u16, va: u64, level: u8, flags: MapFlags) -> Self {
        Self {
            live: true,
            vspace,
            frame,
            va,
            level,
            flags: encode_flags(flags),
        }
    }

    fn flags(&self) -> MapFlags {
        decode_flags(self.flags)
    }
}

fn encode_flags(flags: MapFlags) -> u8 {
    let mut bits = 0u8;
    if flags.read {
        bits |= 1;
    }
    if flags.write {
        bits |= 2;
    }
    if flags.exec {
        bits |= 4;
    }
    if flags.user {
        bits |= 8;
    }
    if flags.device {
        bits |= 16;
    }
    bits
}

fn decode_flags(bits: u8) -> MapFlags {
    MapFlags {
        read: bits & 1 != 0,
        write: bits & 2 != 0,
        exec: bits & 4 != 0,
        user: bits & 8 != 0,
        device: bits & 16 != 0,
    }
}

#[derive(Clone, Copy, Default)]
struct Flush {
    kind: u8,
    asid: u16,
    va: u64,
}

const FLUSH_PAGE: u8 = 0;
const FLUSH_ASID: u8 = 1;

// ---------------------------------------------------------------------------------------------
// Machine — object layer
// ---------------------------------------------------------------------------------------------

/// Object layer K3: Untyped boot + Retype/Revoke + VSpace (PTE Sv39) + cap tối thiểu.
///
/// Một `Machine` giữ toàn bộ bảng tĩnh (không cấp phát); `M: PhysMem` là cửa truy cập bộ nhớ
/// vật lý (arch: `physmap`; host test: `Vec<u8>`).
pub struct Machine<M: PhysMem> {
    mem: M,
    untypeds: [Untyped; MAX_UNTYPEDS],
    n_untypeds: u8,
    objects: [Object; MAX_OBJECTS],
    caps: [Cap; MAX_CAPS],
    mappings: [Mapping; MAX_MAPPINGS],
    flushes: [Flush; MAX_FLUSHES],
    n_flushes: u8,
    next_id: u32,
    next_asid: u16,
}

impl<M: PhysMem> Machine<M> {
    /// Object layer rỗng (chưa có Untyped) — `const` để dựng được trong `static`.
    pub const fn new(mem: M) -> Self {
        Self {
            mem,
            untypeds: [Untyped::EMPTY; MAX_UNTYPEDS],
            n_untypeds: 0,
            objects: [Object::EMPTY; MAX_OBJECTS],
            caps: [Cap::EMPTY; MAX_CAPS],
            mappings: [Mapping::EMPTY; MAX_MAPPINGS],
            flushes: [Flush {
                kind: 0,
                asid: 0,
                va: 0,
            }; MAX_FLUSHES],
            n_flushes: 0,
            next_id: 1,
            next_asid: 1,
        }
    }

    /// Xoá toàn bộ trạng thái (giữ `mem`) — dùng cho `init`.
    pub fn reset(&mut self) {
        self.untypeds = [Untyped::EMPTY; MAX_UNTYPEDS];
        self.n_untypeds = 0;
        self.objects = [Object::EMPTY; MAX_OBJECTS];
        self.caps = [Cap::EMPTY; MAX_CAPS];
        self.mappings = [Mapping::EMPTY; MAX_MAPPINGS];
        self.flushes = [Flush {
            kind: 0,
            asid: 0,
            va: 0,
        }; MAX_FLUSHES];
        self.n_flushes = 0;
        self.next_id = 1;
        self.next_asid = 1;
    }

    // ----------------------------------------------------------------------------------- Untyped

    /// Thêm một **Untyped boot**: arena thật `[arena_base, arena_base+arena_bytes)` (byte) và
    /// cửa sổ PA nhãn bắt đầu ở `pa_base` (dài [`UNTYPED_PA_WINDOW`]) — `pa_base` phải căn cỡ
    /// Frame lớn nhất muốn cấp.
    ///
    /// Trả cap gốc (key duy nhất). Boot dùng 2 Untyped: một arena 64 KiB + cửa sổ PA căn 2 MiB,
    /// một cửa sổ 1 GiB căn cho Frame 1 GiB (K2/D2-66 dùng nhiều Untyped cho nhiều cửa sổ PA).
    pub fn add_untyped(
        &mut self,
        arena_base: u64,
        arena_bytes: u32,
        pa_base: u64,
    ) -> Result<CapId, ObjError> {
        if arena_bytes == 0 {
            return Err(ObjError::InvalidArgument);
        }
        if arena_base & (PAGE_TABLE_BYTES as u64 - 1) != 0 {
            return Err(ObjError::Alignment);
        }
        if pa_base.checked_add(UNTYPED_PA_WINDOW).is_none() {
            return Err(ObjError::InvalidArgument);
        }
        if self.n_untypeds as usize >= MAX_UNTYPEDS {
            return Err(ObjError::NoSlots);
        }
        let obj_idx = self.free_object_slot()?;
        let cap_idx = self.free_cap_slot()?;
        let u_idx = self.n_untypeds;
        self.untypeds[u_idx as usize] = Untyped {
            live: true,
            object: obj_idx,
            arena_base,
            capacity: arena_bytes,
            bump: 0,
            holes: [Hole { off: 0, len: 0 }; MAX_HOLES],
            n_holes: 0,
            pa: FrameArena::new(pa_base, UNTYPED_PA_WINDOW as u32),
        };
        self.n_untypeds += 1;
        let id = self.take_id();
        self.objects[obj_idx as usize] = Object {
            live: true,
            kind: ObjKind::Untyped,
            id,
            untyped: u_idx,
            cap: cap_idx,
            ..Object::EMPTY
        };
        self.caps[cap_idx as usize] = Cap {
            live: true,
            obj: obj_idx,
            kind: ObjKind::Untyped,
            rights: RIGHTS_ALL,
            parent: CAP_NONE,
            id,
        };
        Ok(CapId(cap_idx))
    }

    // ----------------------------------------------------------------------------------- Retype

    /// **Retype nội bộ** (không ABI): cấp một object `kind` từ Untyped của cap nguồn.
    ///
    /// - `PageTable`: `size_bits` `0`/`12` ⇒ 4 KiB arena, zeroed; trả cap.
    /// - `Frame`: `size_bits ∈ {12,21,30}` (đọc từ `vifast_pte::FrameSize`), nhãn PA trong cửa
    ///   sổ của Untyped; **không** tiêu byte arena; chồng PA toàn cục ⇒ `AlreadyMapped`.
    /// - `VSpace`: `size_bits = 0` ⇒ root PageTable 4 KiB (object nội bộ) + VSpace nhãn ASID;
    ///   trả cap. Root **không** có cap (giống K2 KT-38).
    /// - `Untyped`: **chưa hỗ trợ** (`NotSupported`) — K3b KT-58.
    ///
    /// Mọi lỗi **atomic**: không đổi object/byte/cap nào.
    pub fn retype(&mut self, src: CapId, kind: ObjKind, size_bits: u8) -> Result<CapId, ObjError> {
        let (u_idx, _) = self.untyped_cap(src)?;
        match kind {
            ObjKind::PageTable => self.retype_page_table(u_idx, src, size_bits),
            ObjKind::Frame => self.retype_frame(u_idx, src, size_bits),
            ObjKind::VSpace => self.retype_vspace(u_idx, src, size_bits),
            ObjKind::Untyped => Err(ObjError::NotSupported),
        }
    }

    fn retype_page_table(
        &mut self,
        u_idx: u8,
        src: CapId,
        size_bits: u8,
    ) -> Result<CapId, ObjError> {
        if size_bits != 0 && size_bits != PAGE_SIZE_BITS[0] {
            return Err(ObjError::InvalidArgument);
        }
        let obj_idx = self.free_object_slot()?;
        let cap_idx = self.free_cap_slot()?;
        let off = self.untypeds[u_idx as usize]
            .alloc(PAGE_TABLE_BYTES, PAGE_TABLE_BYTES)
            .ok_or(ObjError::NotEnoughMemory)?;
        let pa = self.untypeds[u_idx as usize].arena_base + off as u64;
        if !zero_block(&mut self.mem, pa, PAGE_TABLE_BYTES as usize) {
            let _ = self.untypeds[u_idx as usize].free(off, PAGE_TABLE_BYTES);
            return Err(ObjError::Corrupt);
        }
        let id = self.take_id();
        self.objects[obj_idx as usize] = Object {
            live: true,
            kind: ObjKind::PageTable,
            id,
            untyped: u_idx,
            cap: cap_idx,
            off,
            len: PAGE_TABLE_BYTES,
            ..Object::EMPTY
        };
        self.caps[cap_idx as usize] = Cap {
            live: true,
            obj: obj_idx,
            kind: ObjKind::PageTable,
            rights: self.caps[src.0 as usize].rights,
            parent: src.0,
            id,
        };
        Ok(CapId(cap_idx))
    }

    fn retype_frame(&mut self, u_idx: u8, src: CapId, size_bits: u8) -> Result<CapId, ObjError> {
        let size = FrameSize::from_size_bits(size_bits).map_err(|_| ObjError::InvalidArgument)?;
        let obj_idx = self.free_object_slot()?;
        let cap_idx = self.free_cap_slot()?;
        let off = self.untypeds[u_idx as usize].pa_alloc(size.size_bytes())?;
        let pa = self.untypeds[u_idx as usize].pa_base() + u64::from(off);
        if self.frame_overlap(pa, size.size_bytes(), OBJ_NONE) {
            let _ = self.untypeds[u_idx as usize].pa_free(off, size.size_bytes());
            return Err(ObjError::AlreadyMapped);
        }
        let id = self.take_id();
        self.objects[obj_idx as usize] = Object {
            live: true,
            kind: ObjKind::Frame,
            id,
            untyped: u_idx,
            cap: cap_idx,
            pa_off: off,
            size_bits: size.size_bits(),
            ..Object::EMPTY
        };
        self.caps[cap_idx as usize] = Cap {
            live: true,
            obj: obj_idx,
            kind: ObjKind::Frame,
            rights: self.caps[src.0 as usize].rights,
            parent: src.0,
            id,
        };
        Ok(CapId(cap_idx))
    }

    fn retype_vspace(&mut self, u_idx: u8, src: CapId, size_bits: u8) -> Result<CapId, ObjError> {
        if size_bits != 0 {
            return Err(ObjError::InvalidArgument);
        }
        // Root PT là object nội bộ ⇒ không cần cap; vẫn tiêu 4 KiB arena (bảng thật).
        let root_idx = self.free_object_slot()?;
        let vs_idx = self.free_object_slot_vs(root_idx)?;
        let cap_idx = self.free_cap_slot()?;
        let asid = self.free_asid()?;
        let off = self.untypeds[u_idx as usize]
            .alloc(PAGE_TABLE_BYTES, PAGE_TABLE_BYTES)
            .ok_or(ObjError::NotEnoughMemory)?;
        let root_pa = self.untypeds[u_idx as usize].arena_base + off as u64;
        if !zero_block(&mut self.mem, root_pa, PAGE_TABLE_BYTES as usize) {
            let _ = self.untypeds[u_idx as usize].free(off, PAGE_TABLE_BYTES);
            return Err(ObjError::Corrupt);
        }
        let root_id = self.take_id();
        let vs_id = self.take_id();
        self.objects[root_idx as usize] = Object {
            live: true,
            kind: ObjKind::PageTable,
            id: root_id,
            untyped: u_idx,
            cap: CAP_NONE,
            off,
            len: PAGE_TABLE_BYTES,
            owner: vs_idx as u8,
            ..Object::EMPTY
        };
        self.objects[vs_idx as usize] = Object {
            live: true,
            kind: ObjKind::VSpace,
            id: vs_id,
            untyped: u_idx,
            cap: cap_idx,
            root: root_idx,
            asid,
            ..Object::EMPTY
        };
        self.caps[cap_idx as usize] = Cap {
            live: true,
            obj: vs_idx,
            kind: ObjKind::VSpace,
            rights: self.caps[src.0 as usize].rights,
            parent: src.0,
            id: vs_id,
        };
        Ok(CapId(cap_idx))
    }

    // ----------------------------------------------------------------------------------- Revoke

    /// `Revoke` một cap: trả object + mọi object con về Untyped (arena/PA), giữ cap gốc.
    ///
    /// - Untyped: finalise **mọi** object cấp từ nó (VSpace teardown trước), trả sạch byte +
    ///   cửa sổ PA; cap nguồn còn sống (dùng lại được).
    /// - VSpace: gỡ mọi mapping (không trả Frame — Frame vẫn có cap riêng), trả root + bảng
    ///   trung gian.
    /// - Frame/PageTable: trả PA/arena; còn con ⇒ [`ObjError::HasChildren`].
    pub fn revoke(&mut self, cap: CapId) -> Result<(), ObjError> {
        let cap_rec = self.cap(cap)?;
        // Mỗi VSpace bị teardown cần một `sfence (asid)`; Untyped có thể teardown nhiều VSpace.
        let need = match cap_rec.kind {
            ObjKind::VSpace => 1,
            ObjKind::Untyped => {
                let obj = self.object(cap_rec.obj)?;
                self.objects
                    .iter()
                    .filter(|o| o.live && o.kind == ObjKind::VSpace && o.untyped == obj.untyped)
                    .count()
            }
            _ => 0,
        };
        self.ensure_flush_slots(need)?;
        match cap_rec.kind {
            ObjKind::Untyped => {
                let obj = self.object(cap_rec.obj)?;
                self.revoke_untyped(obj.untyped)
            }
            ObjKind::VSpace => {
                let obj = self.object(cap_rec.obj)?;
                self.teardown_vspace(cap_rec.obj as u8, obj.asid)?;
                self.kill_cap(cap.0);
                Ok(())
            }
            ObjKind::Frame => {
                let obj = self.object(cap_rec.obj)?;
                if obj.maps != 0 {
                    return Err(ObjError::HasChildren);
                }
                self.free_frame_meta(cap_rec.obj)?;
                Ok(())
            }
            ObjKind::PageTable => {
                let obj = self.object(cap_rec.obj)?;
                if obj.owner != VS_NONE {
                    return Err(ObjError::HasChildren);
                }
                self.free_table_meta(cap_rec.obj)?;
                Ok(())
            }
        }
    }

    fn revoke_untyped(&mut self, u_idx: u8) -> Result<(), ObjError> {
        // **Atomic**: Frame của Untyped này còn mapping từ VSpace **khác** ⇒ từ chối trước khi
        // đổi bất cứ thứ gì (không để mapping trỏ object chết — reviewer L1).
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live || obj.untyped != u_idx || obj.kind != ObjKind::Frame {
                continue;
            }
            for map in self.mappings.iter() {
                if !map.live || map.frame as usize != i {
                    continue;
                }
                let Some(vs) = self.objects.get(map.vspace as usize) else {
                    return Err(ObjError::Corrupt);
                };
                // VSpace **cùng** Untyped sẽ bị teardown ngay sau đây; VSpace khác thì không —
                // map sang đó sẽ trỏ object chết nếu ta giải phóng Frame ⇒ `HasChildren`.
                if !vs.live || vs.untyped != u_idx {
                    return Err(ObjError::HasChildren);
                }
            }
        }
        // VSpace trước (gỡ mapping + trả bảng con), rồi PageTable/Frame rời.
        let mut i = 0usize;
        while i < MAX_OBJECTS {
            let obj = self.objects[i];
            if obj.live && obj.kind == ObjKind::VSpace && obj.untyped == u_idx {
                self.teardown_vspace(i as u8, obj.asid)?;
            }
            i += 1;
        }
        let mut i = 0usize;
        while i < MAX_OBJECTS {
            let obj = self.objects[i];
            if obj.live && obj.untyped == u_idx && obj.kind != ObjKind::Untyped {
                match obj.kind {
                    ObjKind::PageTable => {
                        // Bảng nội bộ (owner != VS_NONE) đã bị teardown ở vòng trên; còn lại
                        // là bảng rời có cap ⇒ trả block.
                        if self.untypeds[u_idx as usize].free(obj.off, obj.len) {
                            self.objects[i].live = false;
                        } else {
                            return Err(ObjError::Corrupt);
                        }
                    }
                    ObjKind::Frame => {
                        let size = 1u64 << obj.size_bits;
                        if !self.untypeds[u_idx as usize].pa_free(obj.pa_off, size) {
                            return Err(ObjError::Corrupt);
                        }
                        self.objects[i].live = false;
                    }
                    _ => {}
                }
                if obj.cap != CAP_NONE {
                    self.kill_cap(obj.cap);
                }
                self.objects[i].live = false;
            }
            i += 1;
        }
        let u = &mut self.untypeds[u_idx as usize];
        u.bump = 0;
        u.n_holes = 0;
        u.holes = [Hole { off: 0, len: 0 }; MAX_HOLES];
        u.reset_pa();
        Ok(())
    }

    fn teardown_vspace(&mut self, vs_idx: u8, asid: u16) -> Result<(), ObjError> {
        let mut i = 0usize;
        while i < MAX_MAPPINGS {
            let map = self.mappings[i];
            if map.live && map.vspace == vs_idx {
                if let Ok(frame) = self.object(map.frame) {
                    if frame.live {
                        let flags = map.flags();
                        let mut f = frame;
                        f.maps = f.maps.saturating_sub(1);
                        if flags.write {
                            f.mapped_w = f.mapped_w.saturating_sub(1);
                        }
                        if flags.exec {
                            f.mapped_x = f.mapped_x.saturating_sub(1);
                        }
                        if flags.device {
                            f.mapped_device = f.mapped_device.saturating_sub(1);
                        }
                        self.objects[map.frame as usize] = f;
                    }
                }
                self.mappings[i].live = false;
            }
            i += 1;
        }
        // Trả mọi bảng 4 KiB thuộc VSpace (root + trung gian).
        let mut i = 0usize;
        while i < MAX_OBJECTS {
            let obj = self.objects[i];
            if obj.live && obj.kind == ObjKind::PageTable && obj.owner == vs_idx {
                if !self.untypeds[obj.untyped as usize].free(obj.off, obj.len) {
                    return Err(ObjError::Corrupt);
                }
                self.objects[i].live = false;
            }
            i += 1;
        }
        let cap = self.objects[vs_idx as usize].cap;
        if cap != CAP_NONE {
            self.kill_cap(cap);
        }
        self.objects[vs_idx as usize].live = false;
        if !self.push_flush(Flush {
            kind: FLUSH_ASID,
            asid,
            va: 0,
        }) {
            // Không thể xảy ra sau `ensure_flush_slots` ở `revoke`; trả lỗi thay vì rơi flush.
            return Err(ObjError::NoSlots);
        }
        Ok(())
    }

    fn free_table_meta(&mut self, idx: u16) -> Result<(), ObjError> {
        let obj = self.object(idx)?;
        if obj.kind != ObjKind::PageTable {
            return Err(ObjError::WrongObjectType);
        }
        if obj.owner != VS_NONE {
            return Err(ObjError::HasChildren);
        }
        if obj.len != 0 && !self.untypeds[obj.untyped as usize].free(obj.off, obj.len) {
            return Err(ObjError::Corrupt);
        }
        self.objects[idx as usize].live = false;
        if obj.cap != CAP_NONE {
            self.kill_cap(obj.cap);
        }
        Ok(())
    }

    fn free_frame_meta(&mut self, idx: u16) -> Result<(), ObjError> {
        let obj = self.object(idx)?;
        if obj.kind != ObjKind::Frame {
            return Err(ObjError::WrongObjectType);
        }
        if obj.maps != 0 {
            return Err(ObjError::HasChildren);
        }
        let size = 1u64 << obj.size_bits;
        if !self.untypeds[obj.untyped as usize].pa_free(obj.pa_off, size) {
            return Err(ObjError::Corrupt);
        }
        self.objects[idx as usize].live = false;
        if obj.cap != CAP_NONE {
            self.kill_cap(obj.cap);
        }
        Ok(())
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn free_asid(&mut self) -> Result<u16, ObjError> {
        for _ in 0..255 {
            let asid = self.next_asid;
            self.next_asid = if self.next_asid >= 255 {
                1
            } else {
                self.next_asid + 1
            };
            let used = self
                .objects
                .iter()
                .any(|o| o.live && o.kind == ObjKind::VSpace && o.asid == asid);
            if !used {
                return Ok(asid);
            }
        }
        Err(ObjError::NoSlots)
    }

    fn free_object_slot(&self) -> Result<u16, ObjError> {
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live {
                return Ok(i as u16);
            }
        }
        Err(ObjError::NoSlots)
    }

    /// Slot object còn trống **khác** `exclude` — VSpace cần 2 slot liên tiếp logic.
    fn free_object_slot_vs(&self, exclude: u16) -> Result<u16, ObjError> {
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live && i as u16 != exclude {
                return Ok(i as u16);
            }
        }
        Err(ObjError::NoSlots)
    }

    fn free_cap_slot(&self) -> Result<u16, ObjError> {
        for (i, cap) in self.caps.iter().enumerate() {
            if !cap.live {
                return Ok(i as u16);
            }
        }
        Err(ObjError::NoSlots)
    }

    fn free_mapping_slot(&self) -> Result<u16, ObjError> {
        for (i, map) in self.mappings.iter().enumerate() {
            if !map.live {
                return Ok(i as u16);
            }
        }
        Err(ObjError::NoSlots)
    }

    fn object(&self, idx: u16) -> Result<Object, ObjError> {
        let obj = *self.objects.get(idx as usize).ok_or(ObjError::RangeError)?;
        if !obj.live {
            return Err(ObjError::InvalidCapability);
        }
        Ok(obj)
    }

    fn cap(&self, cap: CapId) -> Result<Cap, ObjError> {
        let rec = *self.caps.get(cap.0 as usize).ok_or(ObjError::RangeError)?;
        if !rec.live || rec.obj == OBJ_NONE {
            return Err(ObjError::InvalidCapability);
        }
        if !self.objects.get(rec.obj as usize).is_some_and(|o| o.live) {
            return Err(ObjError::InvalidCapability);
        }
        Ok(rec)
    }

    fn untyped_cap(&self, cap: CapId) -> Result<(u8, Object), ObjError> {
        let rec = self.cap(cap)?;
        if rec.kind != ObjKind::Untyped {
            return Err(ObjError::WrongObjectType);
        }
        if rec.rights & RIGHT_WRITE == 0 {
            return Err(ObjError::InsufficientRights);
        }
        let obj = self.object(rec.obj)?;
        if obj.kind != ObjKind::Untyped || obj.untyped as usize >= self.n_untypeds as usize {
            return Err(ObjError::Corrupt);
        }
        Ok((obj.untyped, obj))
    }

    fn frame_cap_full(&self, cap: CapId) -> Result<(u16, Object), ObjError> {
        let rec = self.cap(cap)?;
        if rec.kind != ObjKind::Frame {
            return Err(ObjError::WrongObjectType);
        }
        let obj = self.object(rec.obj)?;
        Ok((rec.obj, obj))
    }

    fn vspace_cap_full(&self, cap: CapId) -> Result<(u8, Object), ObjError> {
        let rec = self.cap(cap)?;
        if rec.kind != ObjKind::VSpace {
            return Err(ObjError::WrongObjectType);
        }
        let obj = self.object(rec.obj)?;
        if obj.root == OBJ_NONE {
            return Err(ObjError::Corrupt);
        }
        Ok((rec.obj as u8, obj))
    }

    fn kill_cap(&mut self, idx: u16) {
        if let Some(cap) = self.caps.get_mut(idx as usize) {
            cap.live = false;
        }
    }

    /// Đẩy yêu cầu flush; trả `false` nếu hàng đợi đầy — caller **phải** xử lý (không im lặng).
    fn push_flush(&mut self, fl: Flush) -> bool {
        if (self.n_flushes as usize) < MAX_FLUSHES {
            self.flushes[self.n_flushes as usize] = fl;
            self.n_flushes += 1;
            true
        } else {
            false
        }
    }

    /// Chỗ cho `need` yêu cầu flush nữa — kiểm **trước** mọi thay đổi (atomic).
    fn ensure_flush_slots(&self, need: usize) -> Result<(), ObjError> {
        if self.n_flushes as usize + need > MAX_FLUSHES {
            return Err(ObjError::NoSlots);
        }
        Ok(())
    }

    fn frame_overlap(&self, pa: u64, size: u64, skip: u16) -> bool {
        for (i, obj) in self.objects.iter().enumerate() {
            if !obj.live || obj.kind != ObjKind::Frame || i as u16 == skip {
                continue;
            }
            let other_pa = self.untypeds[obj.untyped as usize].pa_base() + u64::from(obj.pa_off);
            let other_size = 1u64 << obj.size_bits;
            if pa < other_pa + other_size && other_pa < pa + size {
                return true;
            }
        }
        false
    }

    fn pte_load(&self, table_pa: u64, index: usize) -> Option<u64> {
        if index >= PTE_ENTRIES {
            return None;
        }
        self.mem.read_u64(table_pa + (index as u64) * 8)
    }

    fn pte_store(&mut self, table_pa: u64, index: usize, value: u64) -> bool {
        if index >= PTE_ENTRIES {
            return false;
        }
        self.mem.write_u64(table_pa + (index as u64) * 8, value)
    }
}

/// Căn `value` lên bội số `align` (u32, luỹ thừa 2); `None` nếu tràn.
fn align_up_u32(value: u32, align: u32) -> Option<u32> {
    if align == 0 || !align.is_power_of_two() {
        return None;
    }
    value.checked_add(align - 1).map(|v| v & !(align - 1))
}

fn zero_block<M: PhysMem>(mem: &mut M, pa: u64, bytes: usize) -> bool {
    for i in 0..bytes / 8 {
        if !mem.write_u64(pa + (i as u64) * 8, 0) {
            return false;
        }
    }
    true
}

/// Cấp lá của một cỡ Frame: `12 → 0`, `21 → 1`, `30 → 2`.
fn frame_level(size_bits: u8) -> Result<u8, ObjError> {
    for (level, bits) in PAGE_SIZE_BITS.iter().enumerate() {
        if *bits == size_bits {
            return Ok(level as u8);
        }
    }
    Err(ObjError::InvalidArgument)
}
