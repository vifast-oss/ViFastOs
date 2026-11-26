// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-67** — cầu nối D2-83 cho `memsrv`: **core giữ cap, object layer giữ Frame/PA**.
//!
//! # Biên giới (một chiều, đúng D2-83)
//!
//! - `Frame`/`PageTable`/`VSpace` **chỉ** do object layer (`vifast_core::vm`, KT-52) cấp
//!   và thu hồi; core **không** Retype ba loại này (pin TAP `core d2-83 boundary`).
//! - Khi cấp một Frame cho client, `vifast-core` giữ một **object đại diện**
//!   ([`vifast_core::Machine::add_foreign_object`]) + cap trỏ nó; `key` của object là danh tính
//!   bên object layer (`(object_id << 16) | cap_index`). Chiều tra cứu là **một chiều**:
//!   cap core → key → cap object layer → `(pa, size_bits)`; core **không** đọc byte/PTE nào của
//!   Frame và **không** kế toán PA (D2-83).
//! - Byte của cửa sổ PA do **một** allocator Frame (`vifast_alloc::FrameArena`) trong object layer
//!   quản; [`budget`] chỉ là **view** của cùng trạng thái ấy (không phải allocator thứ hai).
//!
//! # Vì sao quyết định nằm ở đây, chưa phải trong `memsrv` U-mode
//!
//! KT-67 ưu tiên phần chắc chắn đo được: allocator + cầu nối + TAP trên QEMU (theo chỉ đạo
//! "ưu tiên `vifast-alloc` + TAP alloc/revoke trên kernel"). Phần **memsrv U-mode** bị **BLOCKED**
//! vì ABI hiện tại **không** chở được cap trong message từ U-mode (`NBSend` chỉ có label + 4 MR —
//! `v2/kernel/vifast-plat-user/lib.rs` `send()`), và U-mode **không** có invocation để tạo
//! `Frame` (bảng dispatch `v2/kernel/vifast-core/src/syscall.rs` chỉ 0..15; Retype VM trong core
//! trả `NotSupported` tại `v2/kernel/vifast-core/src/machine.rs`). Vì vậy hàm [`grant`] ở đây là
//! **bước quyết định + thi hành** mà vòng `NBRecv` của memsrv sẽ gọi khi có request (cùng mẫu
//! "kernel làm scheduler/bridge" của KT-65), và **cơ chế chuyển cap KT-63**
//! (`prepare_caps`/`deliver_caps`) là đường đưa cap Frame tới client — TAP `mem bridge cap`
//! chạy đúng đường đó trên 2 TCB thật.
//!
//! `unsafe`: **không** — tệp này chỉ gọi API safe của object layer và core (D2-17).

use core::sync::atomic::{AtomicU32, Ordering};

use vifast_core::invoke::{InvokeOp, InvokeOutcome};
use vifast_core::spec::{CORE_OBJECTS, RESERVED_OBJECTS};
use vifast_core::vm::{CapId, FrameBudget, MapFlags, ObjError, ObjKind};
use vifast_core::{CapId as CoreCap, KError, ObjId, ObjType, RIGHT_WRITE};

use crate::core_bridge;
use crate::obj as aobj;

/// Mask phần thấp của `key` (chỉ số cap object layer).
const KEY_CAP_MASK: u64 = 0xFFFF;

/// Lỗi của cầu nối: lỗi object layer hoặc lỗi core (giữ nguyên nguồn, không bọc mã mới).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemError {
    /// Lỗi từ object layer (`Retype`/`Revoke`/truy vấn Frame).
    Obj(ObjError),
    /// Lỗi từ core (cap người gửi, bảng object/cap, MDB).
    Core(KError),
}

impl MemError {
    /// Mã lỗi ABI (`spec/abi.toml`) — `None` với lỗi nội bộ không có mã.
    pub const fn abi_code(self) -> Option<u16> {
        match self {
            MemError::Obj(error) => error.abi_code(),
            MemError::Core(error) => Some(error.id()),
        }
    }

    /// Tên ổn định cho log/TAP (trùng `spec/abi.toml` khi có).
    pub const fn name(self) -> &'static str {
        match self {
            MemError::Obj(error) => error.code_name(),
            MemError::Core(error) => error.name(),
        }
    }
}

/// Một Frame đã cấp cho client: cap object layer + object/cap đại diện trong core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameGrant {
    /// Object đại diện trong core (`ObjType::Frame`, không thuộc Untyped nào).
    pub core_obj: ObjId,
    /// Cap gốc của object đại diện trong core (chưa nằm trong slot — caller `put_cap`).
    pub core_cap: CoreCap,
    /// Cap Frame của object layer (nguồn sự thật PA/byte).
    pub obj_cap: CapId,
    /// Danh tính bên object layer: `(object_id << 16) | cap_index`.
    pub key: u64,
    /// PA nhãn của Frame (D2-66).
    pub pa: u64,
    /// `size_bits` (`12`/`21`/`30`).
    pub size_bits: u8,
}

/// Ghép `key` từ danh tính object layer: `object_id` (không tái dùng) ở cao, chỉ số cap ở thấp.
pub const fn make_key(cap: CapId, object_id: u32) -> u64 {
    (object_id as u64) << 16 | cap.0 as u64
}

/// Tách `key` thành `(cap, object_id)` — dùng để phát hiện cap đã chết/tái dùng chỉ số.
pub const fn decode_key(key: u64) -> (CapId, u32) {
    (CapId((key & KEY_CAP_MASK) as u16), (key >> 16) as u32)
}

/// **Quyết định + thi hành** một yêu cầu cấp Frame (bước **kernel-mediated** — D2-96; `memsrv`
/// U-mode chỉ là **policy façade**, chưa cài): kiểm **cap client gửi** (phải là `Untyped` còn
/// sống, có `RIGHT_WRITE`) rồi cấp Frame thật từ Untyped nguồn của object layer và dựng đại diện
/// trong core.
///
/// **Giới hạn có chủ đích (reviewer-kt67 #6)**: kiểm cap client là **quyền thao tác**, **không**
/// phải uỷ quyền/ngân sách — KT-67 **không** thi hành quota theo cap client (`CLIENT_TOKEN_BYTES`
/// trong TAP chỉ là cỡ nhãn của Untyped dựng cho test, không phải hạn mức được trừ). Ngân sách
/// thật của Frame là **cửa sổ PA của Untyped nguồn** (một allocator — `vifast-alloc`).
///
/// Lỗi **nguyên tử**: cap client sai ⇒ không cấp byte nào; core không dựng được đại diện ⇒
/// object layer được **rollback** (`revoke`) ngay, không để Frame mồ côi.
pub fn grant(client: CoreCap, source: CapId, size_bits: u8) -> Result<FrameGrant, MemError> {
    // 1) Quyền của **cap client gửi**: sai loại/thiếu `RIGHT_WRITE` ⇒ từ chối trước khi cấp.
    core_bridge::with_machine(|m| {
        let info = m.cap_info(client).map_err(MemError::Core)?;
        if info.ty != ObjType::Untyped {
            return Err(MemError::Core(KError::WrongObjectType));
        }
        if info.rights & RIGHT_WRITE == 0 {
            return Err(MemError::Core(KError::InsufficientRights));
        }
        Ok(())
    })?;
    // 2) Object layer cấp Frame **thật** (tầng duy nhất sở hữu Frame — D2-83).
    let obj_cap = aobj::machine()
        .retype(source, ObjKind::Frame, size_bits)
        .map_err(MemError::Obj)?;
    let (pa, bits, key) = {
        let m = aobj::machine();
        let pa = m.frame_pa(obj_cap).map_err(MemError::Obj)?;
        let bits = m.frame_size_bits(obj_cap).map_err(MemError::Obj)?;
        let id = m.cap_key(obj_cap).map_err(MemError::Obj)?;
        (pa, bits, make_key(obj_cap, id))
    };
    // 3) Core: object đại diện + cap gốc; lỗi ⇒ rollback object layer (nguyên tử).
    let made = core_bridge::with_machine(|m| {
        let obj = m.add_foreign_object(ObjType::Frame, key)?;
        let cap = m.root_cap(obj);
        Ok::<(ObjId, CoreCap), KError>((obj, cap))
    });
    match made {
        Ok((core_obj, core_cap)) => Ok(FrameGrant {
            core_obj,
            core_cap,
            obj_cap,
            key,
            pa,
            size_bits: bits,
        }),
        Err(error) => {
            match aobj::machine().revoke(obj_cap) {
                Ok(()) => Err(MemError::Core(error)),
                // Rollback hỏng là lỗi **nặng hơn** lỗi gốc (Frame mồ côi) — báo đúng nó,
                // không nuốt (reviewer-kt67 #6).
                Err(rollback) => Err(MemError::Obj(rollback)),
            }
        }
    }
}

/// Tra ngược **một chiều** `core obj → (cap object layer, pa, size_bits)` (D2-83).
///
/// `None` khi object không phải đại diện Frame, cap bên object layer đã chết, hoặc `object_id`
/// không khớp (chỉ số cap bị tái dùng) — fail-closed, không đoán.
pub fn resolve(core_obj: ObjId) -> Option<(CapId, u64, u8)> {
    let key = core_bridge::with_machine(|m| m.foreign_key(core_obj).ok())?;
    let (cap, want_id) = decode_key(key);
    let m = aobj::machine();
    if m.cap_kind(cap).ok()? != ObjKind::Frame {
        return None;
    }
    if m.cap_key(cap).ok()? != want_id {
        return None;
    }
    let pa = m.frame_pa(cap).ok()?;
    let bits = m.frame_size_bits(cap).ok()?;
    Some((cap, pa, bits))
}

/// Trả Frame về allocator (revoke) — thứ tự **an toàn**: kiểm hết điều kiện có thể hỏng **trước**
/// mọi thay đổi, rồi `revoke` (object layer = nguồn sự thật Frame), cuối cùng đánh dấu đại diện
/// core chết. Lỗi giữa đường vì vậy **không** để lại "core đã chết object, Frame còn sống"
/// (reviewer-kt67 #6).
///
/// Caller phải `delete` mọi cap (core) trỏ tới object đại diện trước — giữ bất biến
/// `check_caps` "cap sống không trỏ object chết".
pub fn release(grant: FrameGrant) -> Result<(), MemError> {
    let (maps, writes, execs) = aobj::machine()
        .frame_counts(grant.obj_cap)
        .map_err(MemError::Obj)?;
    if maps != 0 || writes != 0 || execs != 0 {
        return Err(MemError::Obj(ObjError::HasChildren));
    }
    if core_bridge::with_machine(|m| m.foreign_object_has_caps(grant.core_obj))
        .map_err(MemError::Core)?
    {
        return Err(MemError::Core(KError::IllegalOperation));
    }
    // 1) Trả Frame về allocator (object layer là nguồn sự thật byte).
    aobj::machine()
        .revoke(grant.obj_cap)
        .map_err(MemError::Obj)?;
    // 2) Đánh dấu object đại diện chết (đã chắc không còn cap sống).
    core_bridge::with_machine(|m| m.kill_foreign_object(grant.core_obj)).map_err(MemError::Core)?;
    Ok(())
}

/// Ảnh chụp cửa sổ PA mà `memsrv`/TAP dùng để **đối chiếu** với object layer — view của
/// allocator Frame duy nhất, không phải allocator thứ hai.
pub fn budget(source: CapId) -> Option<FrameBudget> {
    aobj::machine().frame_budget(source).ok()
}

/// Cấp key mới cho một Frame **đã có** (chỉ dùng cho test/chẩn đoán cap cũ): `None` nếu hỏng.
pub fn key_of(core_obj: ObjId) -> Option<u64> {
    core_bridge::with_machine(|m| m.foreign_key(core_obj).ok())
}

// ---------------------------------------------------------------------------------------------
// KT-67a — invocation hẹp (D2-97): object layer thi hành, core giữ cap/quyền (D2-83)
// ---------------------------------------------------------------------------------------------

/// Số Untyped tối đa được đăng ký với bridge (memsrv giữ cap trực tiếp) — bảng tĩnh, không cấp
/// phát; mỗi entry là `(core ObjId, object-layer CapId)`.
pub const MAX_REGISTERED_UNTYPEDS: usize = 4;

static REG_OBJ: [AtomicU32; MAX_REGISTERED_UNTYPEDS] =
    [const { AtomicU32::new(0) }; MAX_REGISTERED_UNTYPEDS];
static REG_CAP: [AtomicU32; MAX_REGISTERED_UNTYPEDS] =
    [const { AtomicU32::new(0) }; MAX_REGISTERED_UNTYPEDS];
static REG_N: AtomicU32 = AtomicU32::new(0);

/// Đăng ký **một chiều** `core ObjId → cap Untyped của object layer` (D2-83): memsrv giữ cap
/// `Untyped` trong CSpace của nó; invocation `Untyped.Retype` chỉ chở `ObjId` của cap ấy qua
/// hook, nên bridge cần bảng này để biết **cap nào** bên object layer sở hữu byte.
///
/// Trả `false` khi bảng đầy hoặc `obj == 0` — không ghi đè entry cũ (fail-closed).
pub fn register_untyped(obj: ObjId, cap: CapId) -> bool {
    if obj == 0 {
        return false;
    }
    let n = REG_N.load(Ordering::Relaxed) as usize;
    if n >= MAX_REGISTERED_UNTYPEDS {
        return false;
    }
    REG_OBJ[n].store(obj, Ordering::Relaxed);
    REG_CAP[n].store(u32::from(cap.0), Ordering::Relaxed);
    REG_N.store((n + 1) as u32, Ordering::Relaxed);
    true
}

/// Cap Untyped của object layer đã đăng ký cho `core ObjId` — `None` ⇒ fail-closed.
pub fn registered_untyped(obj: ObjId) -> Option<CapId> {
    let n = REG_N.load(Ordering::Relaxed) as usize;
    let mut i = 0usize;
    while i < n.min(MAX_REGISTERED_UNTYPEDS) {
        if REG_OBJ[i].load(Ordering::Relaxed) == obj {
            return Some(CapId(REG_CAP[i].load(Ordering::Relaxed) as u16));
        }
        i += 1;
    }
    None
}

/// `ObjKind` của object layer → `ObjType` của core (chiều ngược của [`obj_kind`]).
const fn core_type(kind: ObjKind) -> ObjType {
    match kind {
        ObjKind::Frame => ObjType::Frame,
        ObjKind::PageTable => ObjType::PageTable,
        ObjKind::VSpace => ObjType::VSpace,
        ObjKind::Untyped => ObjType::Untyped,
    }
}

/// Cờ `Frame.Map` (bit 0 R, 1 W, 2 X, 3 U, 4 Device) → [`MapFlags`] của object layer.
fn decode_map_flags(bits: u64) -> Result<MapFlags, ObjError> {
    if bits & !0x1f != 0 {
        return Err(ObjError::InvalidArgument);
    }
    let flags = MapFlags {
        read: bits & 1 != 0,
        write: bits & 2 != 0,
        exec: bits & 4 != 0,
        user: bits & 8 != 0,
        device: bits & 16 != 0,
    };
    if !flags.is_valid() {
        return Err(ObjError::InvalidArgument);
    }
    Ok(flags)
}

/// **Thi hành** một invocation (KT-67a) trên object layer — arch cài [`vifast_core::syscall::
/// SyscallEnv::invoke`]. Core đã kiểm cap/quyền/loại; ở đây chỉ còn **danh sách trắng hẹp** và
/// biên object layer:
///
/// - `Untyped.Retype` (đợt 1): retype `Frame`/`PageTable`/`VSpace` từ cap Untyped **đã đăng ký**
///   của memsrv; trả `key` để core dựng object đại diện + cap (D2-83).
/// - `Frame.Map`/`Unmap` (đợt 2): map/gỡ **chính Frame** trong VSpace của cap phụ, qua
///   `ObjectLayer::map/unmap` + `sfence.vma` — không đọc/ghi PTE ở tầng khác.
///
/// Lỗi giữ **nguồn gốc** ([`MemError::Obj`]/[`MemError::Core`]) để TAP đọc đúng mã ABI.
pub fn invoke(op: InvokeOp) -> Result<InvokeOutcome, MemError> {
    // **KT-417d (D2-262 nhóm 4)** — `PageTable.{Map,Unmap}`: cap được gọi là **bảng trang do
    // caller sở hữu** (`key`), `MR0` là cap `VSpace` (`aux_key`), `MR2` là cap `Frame`
    // (`aux2_key`). Object layer cài bảng vào VSpace rồi map/gỡ Frame — không đường tắt nào
    // bỏ qua kiểm PTE.
    if op.ty == ObjType::PageTable {
        if op.method != METHOD_MAP && op.method != METHOD_UNMAP {
            return Err(MemError::Core(KError::NotSupported));
        }
        let (pt, want_pt) = decode_key(op.key);
        let (vs, want_vs) = decode_key(op.aux_key);
        {
            let m = aobj::machine();
            if m.cap_kind(pt).ok() != Some(ObjKind::PageTable)
                || m.cap_key(pt).ok() != Some(want_pt)
            {
                return Err(MemError::Core(KError::InvalidCapability));
            }
            if m.cap_kind(vs).ok() != Some(ObjKind::VSpace) || m.cap_key(vs).ok() != Some(want_vs) {
                return Err(MemError::Core(KError::InvalidCapability));
            }
        }
        let va = op.args[1];
        if op.method == METHOD_MAP {
            let (frame, want_frame) = decode_key(op.aux2_key);
            {
                let m = aobj::machine();
                if m.cap_kind(frame).ok() != Some(ObjKind::Frame)
                    || m.cap_key(frame).ok() != Some(want_frame)
                {
                    return Err(MemError::Core(KError::InvalidCapability));
                }
            }
            let flags = decode_map_flags(op.args[3]).map_err(MemError::Obj)?;
            aobj::machine()
                .map_page_table(pt, vs, va, frame, flags)
                .map_err(MemError::Obj)?;
        } else {
            aobj::machine()
                .unmap_page_table(pt, vs, va)
                .map_err(MemError::Obj)?;
        }
        let _ = aobj::machine().drain_flushes(aobj::run_flush);
        return Ok(InvokeOutcome::Value(0));
    }
    let kind = {
        if op.ty == ObjType::Untyped {
            if op.method != METHOD_RETYPE {
                return Err(MemError::Core(KError::NotSupported));
            }
            let cap =
                registered_untyped(op.obj).ok_or(MemError::Core(KError::InvalidCapability))?;
            let kind = obj_kind_of_id(op.args[1]).ok_or(MemError::Core(KError::NotSupported))?;
            let size_bits =
                u8::try_from(op.args[2]).map_err(|_| MemError::Obj(ObjError::InvalidArgument))?;
            let new = aobj::machine()
                .retype(cap, kind, size_bits)
                .map_err(MemError::Obj)?;
            let id = aobj::machine().cap_key(new).map_err(MemError::Obj)?;
            return Ok(InvokeOutcome::Object {
                ty: core_type(kind),
                key: make_key(new, id),
            });
        }
        match op.ty {
            ObjType::Frame => op.method,
            _ => {
                return Err(MemError::Core(KError::NotSupported));
            }
        }
    };
    // Frame.Map (0) / Frame.Unmap (1) — cả hai cần cap phụ VSpace.
    let (vs, want_vs) = decode_key(op.aux_key);
    let (frame, want_frame) = decode_key(op.key);
    {
        let m = aobj::machine();
        if m.cap_kind(vs).ok() != Some(ObjKind::VSpace) || m.cap_key(vs).ok() != Some(want_vs) {
            return Err(MemError::Core(KError::InvalidCapability));
        }
        if m.cap_kind(frame).ok() != Some(ObjKind::Frame)
            || m.cap_key(frame).ok() != Some(want_frame)
        {
            return Err(MemError::Core(KError::InvalidCapability));
        }
    }
    let va = op.args[1];
    match kind {
        METHOD_MAP => {
            let flags = decode_map_flags(op.args[2]).map_err(MemError::Obj)?;
            aobj::machine()
                .map(vs, va, frame, flags)
                .map_err(MemError::Obj)?;
        }
        METHOD_UNMAP => {
            // M1 (reviewer-kt67a): **ràng buộc Frame ↔ mapping** — `ObjectLayer::unmap` tìm
            // theo `va`, nên nếu chỉ gọi nó thì ai có **một** cap Frame bất kỳ + cap VSpace
            // cũng gỡ được mapping của Frame khác (lỗ quyền). Kiểm leaf tại `va` thuộc **đúng**
            // cap Frame này trước (so PA — PA Frame là duy nhất), sai ⇒ lỗi **không im lặng**.
            let walk = aobj::machine().walk(vs, va).map_err(MemError::Obj)?;
            if !walk.valid || !walk.leaf {
                return Err(MemError::Obj(ObjError::NotMapped));
            }
            let frame_pa = aobj::machine().frame_pa(frame).map_err(MemError::Obj)?;
            if walk.phys != frame_pa {
                return Err(MemError::Core(KError::InvalidCapability));
            }
            aobj::machine().unmap(vs, va).map_err(MemError::Obj)?;
        }
        _ => return Err(MemError::Core(KError::NotSupported)),
    }
    let _ = aobj::machine().drain_flushes(aobj::run_flush);
    Ok(InvokeOutcome::Value(0))
}

/// Trả object vừa retype về object layer khi core không dựng nổi đại diện (bảng cap đầy) —
/// best-effort, giữ nguyên tử của `Untyped.Retype`.
pub fn release_key(key: u64) {
    let (cap, want) = decode_key(key);
    let m = aobj::machine();
    if m.cap_key(cap).ok() != Some(want) {
        return;
    }
    let _ = m.revoke(cap);
}

/// Số method — **một nguồn**: lấy từ `vifast_core::invoke` (spec `[object]`).
const METHOD_RETYPE: u32 = vifast_core::invoke::METHOD_RETYPE;
const METHOD_MAP: u32 = vifast_core::invoke::METHOD_MAP;
const METHOD_UNMAP: u32 = vifast_core::invoke::METHOD_UNMAP;

/// Id `[object]` → `ObjKind` (chỉ ba loại VM): **không** chép số — đọc `vifast_abi`.
fn obj_kind_of_id(raw: u64) -> Option<ObjKind> {
    // **D2-242 nhóm 9 (9B)**: `VSpace`/`PageTable`/`Frame` đã là object **lõi** (không còn ở
    // `RESERVED_OBJECTS`) ⇒ tra **hợp** hai bảng, nếu chỉ tra `RESERVED_OBJECTS` thì Retype/Map
    // VM của riscv64 sẽ hỏng (fail-closed sai chỗ). Số vẫn đọc từ `vifast_abi`, không chép tay.
    let find = |name: &str| {
        RESERVED_OBJECTS
            .iter()
            .chain(CORE_OBJECTS.iter())
            .find(|(n, _)| *n == name)
            .map(|(_, id)| u64::from(*id))
    };
    if Some(raw) == find("Frame") {
        Some(ObjKind::Frame)
    } else if Some(raw) == find("PageTable") {
        Some(ObjKind::PageTable)
    } else if Some(raw) == find("VSpace") {
        Some(ObjKind::VSpace)
    } else {
        None
    }
}
