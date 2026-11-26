// SPDX-License-Identifier: Apache-2.0 OR MIT
//! **KT-67a** — invocation của syscall 0 trên object **không phải** Endpoint (D2-97).
//!
//! `spec/abi.toml` `[syscall].0` đã chốt: *“Endpoint: IPC; object khác: invocation (method =
//! label), trả ngay”*. Tệp này **cài đặt** đường đó theo **danh sách trắng hẹp** tăng dần:
//!
//! | Object (id `[object]`) | method | Tên | Trạng thái |
//! |---|---|---|---|
//! | `Untyped` (1) | 0 | `Retype` | **cài** (đợt 1) — chỉ `Frame`/`PageTable`/`VSpace` qua object layer |
//! | `Frame` (11) | 0 | `Map` | **cài** (đợt 2) — map chính Frame vào VSpace (arg: cap VSpace) |
//! | `Frame` (11) | 1 | `Unmap` | **cài** (đợt 2) |
//! | `Tcb` (3) | 0 | `Configure` | **cài** (KT-290) — bind `ipc_buffer?` (MR0 CPTR Frame, MR1 VA); **KT-311** — kênh đầy đủ `length = 3` (MR2 = mask, `cspace?`/`entry?`/`priority?` ở vùng 608..640 của IPC buffer) |
//! | `Tcb` (3) | 15 | `SetTls` | **cài** (KT-74) |
//! | `Tcb` (3) | 12 | `Resume` | **cài** (KT-314) — chuyển đích `Running` + giao `TcbData.entry` cho arch đặt `sepc` |
//! | `Tcb` (3) | 3 | `Suspend` | **cài** (KT-319) — `Running`/`Ready` → `Inactive`, **giữ** `entry` để `Resume` chạy lại |
//! | `Tcb` (3) | 13 | `BindNotification` | **cài** (KT-417) — gắn 1:1 `Notification` vào cap TCB đã gọi (D2-261 nhóm 2) |
//! | `Tcb` (3) | 14 | `UnbindNotification` | **cài** (KT-417) — idempotent |
//! | `CNode` (2) | 3 | `Mutate` | **cài** (KT-417) — đổi `rights?`/`badge?` **tại chỗ** (D2-261 nhóm 6) |
//! | `CNode` (2) | 6 | `Rotate` | **cài** (KT-417) — hoán vị hai slot của cùng CNode (`[TK]`) |
//! | khác | — | — | `NotSupported` **18** (giữ D2-53 Q11 cho phần chưa cài) |
//!
//! # Vì sao `Frame.Map` chứ không phải `VSpace.Map`
//!
//! `spec/abi.toml` cấp **số method** theo bảng `[object]`: `Frame` (id 11) có `Map=0`,
//! `Unmap=1`; `VSpace` (id 9) chỉ có `GetAsid=0` và **method id 1 để trống theo D2-32**;
//! `PageTable` (id 10) có `Map=0`/`Unmap=1` nhưng root PageTable **không có cap** trong object
//! layer (chỉ VSpace có). Vì vậy “VSpace.Map” của KT-67a **không có số method** trong ABI hiện
//! tại; phép map Frame vào VSpace đi qua đúng tên/số method mà spec cấp: `Frame.Map` (11.0).
//! Không thêm method id mới (cấm sửa `spec/`, D2-97).
//!
//! # Kênh tham số (không đổi `MessageInfo` 35 bit)
//!
//! `Call(cap, MessageInfo, MR0..MR3)`: `label` = method, `length` = số MR dùng, `caps` phải 0
//! (chuyển cap = KT-63, chưa mở cho invocation). MR theo method:
//!
//! - `Untyped.Retype`: `MR0 = cptr đích` (slot trong CSpace của caller), `MR1 = type id`
//!   (`[object]` 9/10/11), `MR2 = size_bits`, `MR3 = flags` (0; bit `GUEST` reserved ⇒
//!   `NotSupported` với `VSpace`, `InvalidArgument` với loại khác — đúng ngữ nghĩa core).
//! - `Frame.Map`: `MR0 = cptr cap VSpace`, `MR1 = va`, `MR2 = cờ` (bit 0 R, 1 W, 2 X, 3 U,
//!   4 Device).
//! - `Frame.Unmap`: `MR0 = cptr cap VSpace`, `MR1 = va`.
//!
//! Trả về qua `a0/a1` như mọi syscall (`a1 = value`; Retype trả slot đích, Map/Unmap trả 0).
//! **Không** block, **không** Reply, **không** đổi trạng thái TCB (D2-53 Q11).
//!
//! # Biên giới D2-83
//!
//! Core **không** sở hữu `Frame`/`PageTable`/`VSpace`: nó chỉ kiểm cap/quyền, giải mã method và
//! giao việc cho hook [`SyscallEnv::invoke`] (arch cài bằng object layer). Object layer cấp object
//! và trả **key**; core dựng **object đại diện** + cap rồi cài vào CSpace của caller
//! ([`Machine::add_foreign_object`]). Nhờ vậy core không đọc/ghi byte/PTE nào.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::ipc::MessageInfo;
use crate::machine::Machine;
use crate::spec::{CORE_OBJECTS, RESERVED_OBJECTS, RIGHTS_ALL, RIGHT_WRITE};
use crate::syscall::{SyscallEnv, VspaceOutcome};

/// Yêu cầu invocation **đã kiểm cap** chuyển cho hook arch (D2-83: core không sở hữu VM).
///
/// `Copy` + không cấp phát để dùng được trong `no_std` và trong trait `SyscallEnv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvokeOp {
    /// Loại cap được gọi.
    pub ty: ObjType,
    /// Method (`label`).
    pub method: u32,
    /// Object của cap được gọi (bridge tra Untyped đã đăng ký bằng id này).
    pub obj: ObjId,
    /// Object của cap phụ (`VSpace` với `Frame.Map/Unmap`; `0` khi không dùng).
    pub aux_obj: ObjId,
    /// Khoá object layer của cap được gọi (`0` = không có).
    pub key: u64,
    /// Khoá object layer của cap phụ (`0` = không có).
    pub aux_key: u64,
    /// `MR0..MR3` (đã kiểm `length`).
    pub args: [u64; 4],
}

/// Kết quả hook arch trả cho core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvokeOutcome {
    /// Thao tác xong (Map/Unmap) — `value` trả ở `a1`.
    Value(u64),
    /// Retype đã cấp object bên object layer: core dựng object đại diện + cap từ `key`.
    Object {
        /// Loại object vừa cấp (phải khớp loại caller yêu cầu).
        ty: ObjType,
        /// Khoá object layer (`(object_id << 16) | cap_index`).
        key: u64,
    },
}

/// `Untyped.Retype` — method 0 của object 1 (`[object]`).
pub const METHOD_RETYPE: u32 = 0;
/// `Frame.Map` — method 0 của object 11.
pub const METHOD_MAP: u32 = 0;
/// `Frame.Unmap` — method 1 của object 11.
pub const METHOD_UNMAP: u32 = 1;

// ---------------------------------------------------------------------------------------------
// KT-68 — `CNode.*` (object id 2). Số method **lấy từ `spec/abi.toml`** `[object].CNode`:
// `0 Copy`, `1 Mint`, `2 Move`, `3 Mutate`, `4 Delete`, `5 Revoke`, `6 Rotate`
// (chưa cài). Tên hằng giữ tiền tố `CNODE_` để không lẫn với method 0/1 của `Frame`.
// ---------------------------------------------------------------------------------------------

/// `CNode.Copy` — method 0.
pub const CNODE_COPY: u32 = 0;
/// `CNode.Mint` — method 1.
pub const CNODE_MINT: u32 = 1;
/// `CNode.Move` — method 2.
pub const CNODE_MOVE: u32 = 2;
/// `CNode.Delete` — method 4.
pub const CNODE_DELETE: u32 = 4;
/// `CNode.Revoke` — method 5.
pub const CNODE_REVOKE: u32 = 5;
/// **KT-417** — `CNode.Mutate` — method 3 (D2-261 nhóm 6): biến đổi cap **tại chỗ**.
pub const CNODE_MUTATE: u32 = vifast_abi::CNODE_MUTATE;
/// **KT-417** — `CNode.Rotate` — method 6: hoán vị nội dung hai slot (`[TK]`).
pub const CNODE_ROTATE: u32 = vifast_abi::CNODE_ROTATE;

// ---------------------------------------------------------------------------------------------
// KT-74 — `Tcb.SetTls` (object id 3). Số method **lấy từ `spec/abi.toml`** `[object].Tcb`
// (`SetTls = 15`), qua hằng sinh tự động `vifast_abi::TCB_SETTLS` — **không** chép số tay
// (D2-11/D2-102).
// ---------------------------------------------------------------------------------------------

/// `Tcb.SetTls` — method **15** của object `Tcb` (`spec/abi.toml` `[object].Tcb`).
pub const TCB_SET_TLS: u32 = vifast_abi::TCB_SETTLS;

/// **KT-290** — `Tcb.Configure` — method **0** của object `Tcb` (`spec/abi.toml` `[object].Tcb`),
/// qua hằng sinh tự động `vifast_abi::TCB_CONFIGURE` — **không** chép số tay (D2-11/D2-102).
pub const TCB_CONFIGURE: u32 = vifast_abi::TCB_CONFIGURE;

/// **KT-314** — `Tcb.Resume` — method **12** của object `Tcb` (`spec/abi.toml` `[object].Tcb`),
/// qua hằng sinh tự động `vifast_abi::TCB_RESUME` — **không** chép số tay (D2-11/D2-102).
pub const TCB_RESUME: u32 = vifast_abi::TCB_RESUME;

/// **KT-319** — `Tcb.Suspend` — method **3** của object `Tcb` (`spec/abi.toml` `[object].Tcb`),
/// qua hằng sinh tự động `vifast_abi::TCB_SUSPEND` — **không** chép số tay (D2-11/D2-102).
pub const TCB_SUSPEND: u32 = vifast_abi::TCB_SUSPEND;

/// **KT-417** — `Tcb.BindNotification` — method **13** của object `Tcb` (D2-261 nhóm 2), số sinh
/// từ `spec/abi.toml` qua `vifast_abi::TCB_BINDNOTIFICATION`.
pub const TCB_BIND_NOTIFICATION: u32 = vifast_abi::TCB_BINDNOTIFICATION;

/// **KT-417** — `Tcb.UnbindNotification` — method **14** của object `Tcb`.
pub const TCB_UNBIND_NOTIFICATION: u32 = vifast_abi::TCB_UNBINDNOTIFICATION;

/// **KT-417 / D2-261 nhóm 4** — `Tcb.Exec` — method **16** của object `Tcb`: spec đã cấp số
/// nhưng **chưa cài** (tới `KT-420`). Giữ hằng để test khẳng định `is_whitelisted` = `false`
/// (không trả `Ok` giả — D2-53 Q11).
pub const TCB_EXEC: u32 = vifast_abi::TCB_EXEC;

/// `Tcb.Suspend` — biến thể `mode` **`Abort`** (`[method_signature."Tcb.Suspend"].variants.mode`
/// = `["Abort", "Defer"]`, thứ tự trong spec: `0` = `Abort`).
pub const SUSPEND_MODE_ABORT: u64 = 0;
/// `Tcb.Suspend` — biến thể `mode` **`Defer`** (`1`). K1b chưa có `Restart` ⇒ `InvalidArgument`
/// **5** (cùng ngữ nghĩa model `Machine::tcb_suspend`, KT-65).
pub const SUSPEND_MODE_DEFER: u64 = 1;

/// `true` ⇔ `spec/abi.toml` khai `SetTls` đúng ở số này (test/TAP đối chiếu bảng sinh).
pub fn tcb_set_tls_matches_spec() -> bool {
    vifast_abi::TCB_METHODS
        .iter()
        .any(|(name, id)| *name == "SetTls" && u32::from(*id) == TCB_SET_TLS)
}

/// Id object `[object]` theo **tên** (sinh từ `spec/abi.toml`, không chép tay — D2-11).
fn object_id(name: &str) -> Option<u64> {
    CORE_OBJECTS
        .iter()
        .chain(RESERVED_OBJECTS.iter())
        .find(|(n, _)| *n == name)
        .map(|(_, id)| u64::from(*id))
}

/// Loại object **được phép** retype qua invocation (đợt 1) — ba loại VM của object layer.
fn retype_target(raw: u64) -> Option<ObjType> {
    if Some(raw) == object_id("Frame") {
        Some(ObjType::Frame)
    } else if Some(raw) == object_id("PageTable") {
        Some(ObjType::PageTable)
    } else if Some(raw) == object_id("VSpace") {
        Some(ObjType::VSpace)
    } else {
        None
    }
}

/// Cờ `Map` của invocation — bit 0 R, 1 W, 2 X, 3 U, 4 Device.
pub const MAP_READ: u64 = 1 << 0;
/// Ghi.
pub const MAP_WRITE: u64 = 1 << 1;
/// Thực thi.
pub const MAP_EXEC: u64 = 1 << 2;
/// Leaf U-mode.
pub const MAP_USER: u64 = 1 << 3;
/// Leaf Device (W^X: cấm `Device + X`).
pub const MAP_DEVICE: u64 = 1 << 4;

/// Số đếm invocation cho TAP `# capa:`/`# inv:` (không phải ABI): cho phép/bị từ chối theo
/// method + số lần từng method của `CNode` thành công (KT-68).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvocationStats {
    /// Tổng invocation trả thành công.
    pub allowed: u32,
    /// Tổng invocation bị từ chối (`NotSupported`, method/loại ngoài danh sách trắng).
    pub denied: u32,
    /// `Untyped.Retype` thành công.
    pub retype: u32,
    /// `Frame.Map` thành công.
    pub map: u32,
    /// `Frame.Unmap` thành công.
    pub unmap: u32,
    /// `CNode.Copy` thành công.
    pub copy: u32,
    /// `CNode.Mint` thành công.
    pub mint: u32,
    /// `CNode.Move` thành công.
    pub moved: u32,
    /// `CNode.Delete` thành công.
    pub delete: u32,
    /// `CNode.Revoke` thành công.
    pub revoke: u32,
    /// **KT-74** `Tcb.SetTls` thành công.
    pub set_tls: u32,
    /// **KT-290** `Tcb.Configure` thành công (bind IPC buffer qua invocation).
    pub configure: u32,
    /// **KT-314** `Tcb.Resume` thành công (chuyển `Running` + giao `entry` cho arch).
    pub resume: u32,
    /// **KT-319** `Tcb.Suspend` **`Abort`** thành công (`Running`/`Ready` → `Inactive`).
    pub suspend: u32,
    /// **KT-329** `Tcb.Suspend` **`Defer`** thành công (đặt cờ, **không** đổi trạng thái).
    pub deferred: u32,
    /// **KT-417** `CNode.Mutate` thành công.
    pub mutated: u32,
    /// **KT-417** `CNode.Rotate` thành công.
    pub rotated: u32,
    /// **KT-417** `Tcb.BindNotification` thành công.
    pub bound: u32,
    /// **KT-417** `Tcb.UnbindNotification` thành công.
    pub unbound: u32,
}

static ALLOWED: AtomicU32 = AtomicU32::new(0);
static DENIED: AtomicU32 = AtomicU32::new(0);
static RETYPE: AtomicU32 = AtomicU32::new(0);
static MAP: AtomicU32 = AtomicU32::new(0);
static UNMAP: AtomicU32 = AtomicU32::new(0);
static CNODE_COPIED: AtomicU32 = AtomicU32::new(0);
static CNODE_MINTED: AtomicU32 = AtomicU32::new(0);
static CNODE_MOVED: AtomicU32 = AtomicU32::new(0);
static CNODE_DELETED: AtomicU32 = AtomicU32::new(0);
static CNODE_REVOKED: AtomicU32 = AtomicU32::new(0);
/// **KT-74** số `Tcb.SetTls` thành công (TAP `# tls:` đối chiếu).
static TLS_SET: AtomicU32 = AtomicU32::new(0);
/// **KT-290** số `Tcb.Configure` thành công (TAP `# ipc configure:` đối chiếu).
static CONFIGURED: AtomicU32 = AtomicU32::new(0);
/// **KT-314** số `Tcb.Resume` thành công (TAP `# ipc: resume` đối chiếu).
static RESUMED: AtomicU32 = AtomicU32::new(0);
/// **KT-319** số `Tcb.Suspend` **`Abort`** thành công (TAP `# ipc: suspend` đối chiếu).
/// KT-329 tách khỏi `Defer` để `calls=2` của TAP 9 **không** đổi nghĩa.
static SUSPENDED: AtomicU32 = AtomicU32::new(0);
/// **KT-329** số `Tcb.Suspend` **`Defer`** thành công (TAP `# ipc: defer` đối chiếu).
static DEFERRED: AtomicU32 = AtomicU32::new(0);
/// **KT-417** số `CNode.Mutate` thành công.
static CNODE_MUTATED: AtomicU32 = AtomicU32::new(0);
/// **KT-417** số `CNode.Rotate` thành công.
static CNODE_ROTATED: AtomicU32 = AtomicU32::new(0);
/// **KT-417** số `Tcb.BindNotification` thành công.
static NOTIF_BOUND: AtomicU32 = AtomicU32::new(0);
/// **KT-417** số `Tcb.UnbindNotification` thành công.
static NOTIF_UNBOUND: AtomicU32 = AtomicU32::new(0);

fn bump(counter: &AtomicU32) {
    let _ = counter.fetch_add(1, Ordering::Relaxed);
}

/// Ảnh chụp số đếm invocation (TAP đọc — `# inv:`/`# capa:`).
pub fn stats() -> InvocationStats {
    InvocationStats {
        allowed: ALLOWED.load(Ordering::Relaxed),
        denied: DENIED.load(Ordering::Relaxed),
        retype: RETYPE.load(Ordering::Relaxed),
        map: MAP.load(Ordering::Relaxed),
        unmap: UNMAP.load(Ordering::Relaxed),
        copy: CNODE_COPIED.load(Ordering::Relaxed),
        mint: CNODE_MINTED.load(Ordering::Relaxed),
        moved: CNODE_MOVED.load(Ordering::Relaxed),
        delete: CNODE_DELETED.load(Ordering::Relaxed),
        revoke: CNODE_REVOKED.load(Ordering::Relaxed),
        set_tls: TLS_SET.load(Ordering::Relaxed),
        configure: CONFIGURED.load(Ordering::Relaxed),
        resume: RESUMED.load(Ordering::Relaxed),
        suspend: SUSPENDED.load(Ordering::Relaxed),
        deferred: DEFERRED.load(Ordering::Relaxed),
        mutated: CNODE_MUTATED.load(Ordering::Relaxed),
        rotated: CNODE_ROTATED.load(Ordering::Relaxed),
        bound: NOTIF_BOUND.load(Ordering::Relaxed),
        unbound: NOTIF_UNBOUND.load(Ordering::Relaxed),
    }
}

/// Một invocation **đã qua** kiểm cap/quyền của core, sẵn sàng cho hook arch.
///
/// `key`/`aux_key` là **khoá object layer** (`ObjectLayer::foreign_key`) của cap được gọi và cap
/// phụ (VSpace với `Frame.Map/Unmap`); `0` khi object không có khoá (Untyped do bridge đăng ký).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invocation {
    /// Loại cap được gọi (theo hướng **đỏ** thì mọi loại ngoài Endpoint vẫn tới đây).
    pub ty: ObjType,
    /// Method (`label`).
    pub method: u32,
    /// Object của cap được gọi (bridge dùng cho Untyped đã đăng ký).
    pub obj: ObjId,
    /// Object của cap phụ (`VSpace` với `Frame.Map/Unmap`; `0` khi không dùng).
    pub aux_obj: ObjId,
    /// Khoá object layer của cap được gọi (`0` = không có).
    pub key: u64,
    /// Khoá object layer của cap phụ (`0` = không có).
    pub aux_key: u64,
    /// `MR0..MR3` (đã kiểm `length`).
    pub args: [u64; 4],
}

impl Invocation {
    /// Đóng gói theo hook [`SyscallEnv::invoke`].
    pub fn op(&self) -> InvokeOp {
        InvokeOp {
            ty: self.ty,
            method: self.method,
            obj: self.obj,
            aux_obj: self.aux_obj,
            key: self.key,
            aux_key: self.aux_key,
            args: self.args,
        }
    }
}

/// Đích cài cap của `Untyped.Retype`: slot **trong CSpace của caller**.
fn install_slot(m: &Machine, tcb: ObjId, raw: u64) -> Result<(usize, ObjId), KError> {
    let cspace = m
        .tcb_info_obj(tcb)
        .map_err(|_| KError::InvalidCapability)?
        .cspace
        .ok_or(KError::InvalidCapability)?;
    let slot = usize::try_from(raw).map_err(|_| KError::InvalidCapability)?;
    // Cùng ngữ nghĩa `cap_slot` của syscall: chỉ số ngoài bảng slot của CSpace = cap không hợp
    // lệ (1), không phải `RangeError` 19 (dành cho số hiệu syscall ngoài dải — D2-79).
    if slot >= m.cnode_slot_count(cspace)? {
        return Err(KError::InvalidCapability);
    }
    Ok((slot, cspace))
}

/// Cờ map từ `MR2` của invocation — bit thô chuyển nguyên vẹn cho hook arch (object layer
/// dịch thành `MapFlags` của nó).
///
/// Core **không** biết `MapFlags` của object layer (D2-83): nó chỉ chuyển **bit** nguyên vẹn cho
/// hook arch, ở đây chỉ kiểm bit lạ và `W ⇒ R` (luật Sv39) để lỗi trả đúng mã ABI.
pub fn map_flags(raw: u64) -> Result<u64, KError> {
    if raw & !(MAP_READ | MAP_WRITE | MAP_EXEC | MAP_USER | MAP_DEVICE) != 0 {
        return Err(KError::InvalidArgument);
    }
    let read = raw & MAP_READ != 0;
    let write = raw & MAP_WRITE != 0;
    let exec = raw & MAP_EXEC != 0;
    if !(read || write || exec) || (write && !read) {
        return Err(KError::InvalidArgument);
    }
    Ok(raw)
}

/// `call_msg` đã tách nhánh Endpoint; hàm này xử lý **invocation** và trả kết quả qua `a0/a1`.
///
/// Lỗi trả **mã ABI** trực tiếp (`u64`) để `dispatch` không phải bọc thêm.
pub fn dispatch(
    m: &mut Machine,
    tcb: ObjId,
    slot: SlotRef,
    raw_info: u64,
    mrs: &[u64; 4],
    env: &mut impl SyscallEnv,
) -> Result<(u64, u64), u64> {
    let info = MessageInfo::unpack(raw_info).map_err(|e| e.id() as u64)?;
    if info.caps() != 0 {
        // D2-99(g): chuyển cap trong message qua invocation **chưa** cài (KT-67b mở IPC buffer
        // header) ⇒ nằm ngoài danh sách trắng: `NotSupported` 18, **có đếm** — không im lặng.
        bump(&DENIED);
        return Err(KError::NotSupported.id() as u64);
    }
    let method = info.label();
    // **KT-408** — ring trace 32 B **luôn bật** (`D2-09`): một bản ghi `syscall` mỗi invocation
    // (`a` = method, `b` = label thô của `MessageInfo`).
    m.trace_push(
        crate::trace::TraceKind::Syscall,
        tcb,
        method as u64,
        raw_info,
    );
    let len = usize::from(info.length());
    if len > mrs.len() {
        return Err(KError::InvalidArgument.id() as u64);
    }
    let mut args = [0u64; 4];
    args[..len].copy_from_slice(&mrs[..len]);
    let cap = m.cap_at(slot.cnode, slot.slot).map_err(|e| e.id() as u64)?;
    let c = m.cap_info(cap).map_err(|e| e.id() as u64)?;
    if !c.alive {
        return Err(KError::InvalidCapability.id() as u64);
    }
    match (c.ty, method) {
        (ObjType::Untyped, METHOD_RETYPE) => retype(m, tcb, c, args, env),
        (ObjType::Frame, METHOD_MAP) => map(m, tcb, c, args, env),
        (ObjType::Frame, METHOD_UNMAP) => unmap(m, tcb, c, args, env),
        // KT-68: `CNode.Copy/Mint/Move/Delete/Revoke` trên **chính cap CNode** đã gọi — cap
        // này chỉ vào CSpace của caller (khác `Untyped.Retype` không cần cap phụ).
        (ObjType::CNode, _) => cnode(m, tcb, c, method, args, len),
        // KT-290/KT-311: `Tcb.Configure` (method 0 của `[object].Tcb`) — bind **IPC buffer**
        // (kênh hẹp: `ipc_buffer?` = MR0 CPTR Frame + MR1 VA) + **kênh đầy đủ** (`length = 3`:
        // MR2 = mask, giá trị ở vùng 608..640 của IPC buffer — `[TK]` KT-311).
        (ObjType::Tcb, TCB_CONFIGURE) => configure(m, tcb, slot, c, args, len, env),
        // KT-74: `Tcb.SetTls` (method 15 của `[object].Tcb`) — cap **chính là** TCB đã gọi.
        (ObjType::Tcb, TCB_SET_TLS) => set_tls(m, slot, c, args, len),
        // KT-314: `Tcb.Resume` (method 12 của `[object].Tcb`) — chuyển tác vụ đích `Running` và
        // giao `TcbData.entry` cho arch đặt `sepc` (D2-83: core không ghi CSR).
        (ObjType::Tcb, TCB_RESUME) => resume(m, tcb, slot, c, args, len, env),
        // KT-319: `Tcb.Suspend` (method 3 của `[object].Tcb`) — `Running`/`Ready` → `Inactive`,
        // **giữ** `entry` để `Resume` chạy lại được (không đụng CSR/bộ nhớ user — D2-83).
        (ObjType::Tcb, TCB_SUSPEND) => suspend(m, slot, c, args, len),
        // **KT-417 / D2-261 nhóm 2** — `Tcb.BindNotification` (13) / `UnbindNotification` (14),
        // nối thẳng model API đã kiểm 1:1 (`core_sync.rs::{tcb_bind_notification,
        // tcb_unbind_notification}`); KHÔNG tái dùng cho `IrqHandler` (tránh nới cap TCB).
        (ObjType::Tcb, TCB_BIND_NOTIFICATION) => bind_notification(m, tcb, slot, c, args, len),
        (ObjType::Tcb, TCB_UNBIND_NOTIFICATION) => unbind_notification(m, slot, c, len),
        // Danh sách trắng hẹp: mọi loại/method khác giữ `NotSupported` (D2-53 Q11) — **không**
        // trả `Ok` giả. Đếm để TAP `# inv:` chứng minh nhánh này chạy thật.
        _ => {
            bump(&DENIED);
            Err(KError::NotSupported.id() as u64)
        }
    }
}

// =============================================================================================
// KT-74 — `Tcb.SetTls` (K6): con trỏ TLS của tiến trình U-mode
// =============================================================================================

/// `Tcb.SetTls` — ghi con trỏ TLS của **cap TCB đã gọi** (method `TCB_SET_TLS` = 15).
///
/// # Kênh tham số
///
/// | Tham số | Nguồn | Ghi chú |
/// |---|---|---|
/// | `tls` | `MR0` | con trỏ TLS (`tp` = `x4` của U-mode) |
///
/// **Arity chính xác 1** (D2-104): `length` khác 1 — thiếu (0) **hoặc** thừa (2..4) — đều
/// `InvalidArgument` **5**, **có đếm**. Cap TCB phải có `RIGHT_WRITE`: thiếu ⇒
/// `InsufficientRights` **3** (cùng luật quyền như `CNode.*`, D2-99(e)). Trả `a1 = 0`.
///
/// `SetTls` **không** đổi trạng thái TCB/scheduler: giá trị có hiệu lực ở lần vào U-mode kế
/// tiếp, nên dùng được giữa hai bước của chuỗi `exec` (KT-74).
fn set_tls(
    m: &mut Machine,
    slot: SlotRef,
    c: crate::error::CapInfo,
    args: [u64; 4],
    len: usize,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        bump(&DENIED);
        return Err(KError::InsufficientRights.id() as u64);
    }
    if len != 1 {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    m.tcb_set_tls(slot, args[0]).map_err(|e| e.id() as u64)?;
    bump(&TLS_SET);
    bump(&ALLOWED);
    Ok((0, 0))
}

// =============================================================================================
// KT-290 — `Tcb.Configure` (method 0 của `[object].Tcb`) qua invocation từ U-mode
// =============================================================================================

/// `Tcb.Configure` — cấu hình cap TCB đã gọi (method `TCB_CONFIGURE` = 0).
///
/// # Kênh tham số (`[TK]` KT-290 + KT-311 — **không** đổi `MessageInfo` 35 bit, **không** đổi
/// `spec/`)
///
/// | `length` | Nghĩa | MR |
/// |---|---|---|
/// | **2** | KT-290: chỉ `ipc_buffer?` | `MR0` CPTR `Frame` (CSpace caller), `MR1` VA |
/// | **3** | KT-311: `ipc_buffer?` + mask tham số mở rộng | `MR0`/`MR1` như trên, `MR2` = mask |
///
/// `MR2` (mask) — bit nào bật thì tham số đó **có mặt**; giá trị đọc từ **vùng riêng** của IPC
/// buffer (byte 608..640, `[TK]` KT-311 — xem [`crate::ipc::IPC_BUFFER_CONFIGURE_OFFSET`]):
///
/// | Bit | Tham số | Word | Kiểm |
/// |---|---|---|---|
/// | 0 | `cspace?` | 608..616 CPTR | cap `CNode` sống ⇒ không: `WrongObjectType` 2 / `InvalidCapability` 1 |
/// | 1 | `vspace?` | 616..624 CPTR | cap `VSpace` sống ⇒ không: `WrongObjectType` 2 / `InvalidCapability` 1 |
/// | 2 | `entry?` | 624..632 | căn 2 B (biên lệnh RISC-V có `C`) ⇒ không: `Alignment` 6 |
/// | 3 | `priority?` | 632..640 | vừa `u8` + ≤ `mcp` ⇒ không: `InvalidArgument` 5 |
///
/// **KT-325** — `vspace?` với CPTR **rỗng** (`0`) = **xoá** (về "dùng VSpace của ảnh đã nạp",
/// `TcbData.vspace = 0`, trả `(0, 0)`); `0` **không** được tra như ô 0 của CSpace. CPTR **khác 0**
/// mà không phải `VSpace` **vẫn** ⇒ `WrongObjectType` **2** (ngữ nghĩa KT-320 không đổi).
///
/// Bit ngoài `0..3` ⇒ `InvalidArgument` 5. Mask = 0 ⇒ như `length = 2`. **Arity chính xác**
/// (D2-104): `length` khác 2 và 3 ⇒ `InvalidArgument` **5**, có đếm. Cap TCB phải có
/// `RIGHT_WRITE`: thiếu ⇒ `InsufficientRights` **3**. Cap `Frame` sai loại/chết ⇒
/// `WrongObjectType` **2** / `InvalidCapability` **1**. Trả `a1 = 0`; **không** đổi trạng thái
/// TCB/scheduler (chỉ lưu cấu hình, như `Tcb.SetTls`).
fn configure(
    m: &mut Machine,
    tcb: ObjId,
    slot: SlotRef,
    c: crate::error::CapInfo,
    args: [u64; 4],
    len: usize,
    env: &mut impl SyscallEnv,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        bump(&DENIED);
        return Err(KError::InsufficientRights.id() as u64);
    }
    if len != 2 && len != 3 {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    // CPTR của `ipc_buffer` nằm trong CSpace **caller** (không phải CSpace của TCB đích).
    let cspace = m
        .tcb_info_obj(tcb)
        .map_err(|_| KError::InvalidCapability.id() as u64)?
        .cspace
        .ok_or(KError::InvalidCapability.id() as u64)?;
    let frame_slot = usize::try_from(args[0]).map_err(|_| KError::InvalidCapability.id() as u64)?;
    if frame_slot >= m.cnode_slot_count(cspace).map_err(|e| e.id() as u64)? {
        bump(&DENIED);
        return Err(KError::InvalidCapability.id() as u64);
    }
    let frame = SlotRef::new(cspace, frame_slot);
    // ---- KT-311: `length = 3` ⇒ mask + vùng tham số mở rộng của IPC buffer ----
    let mut params = crate::core_tcb::TcbConfigure {
        ipc_buffer: Some((frame, args[1])),
        ..crate::core_tcb::TcbConfigure::EMPTY
    };
    if len == 3 {
        let mask = args[2];
        if mask & !crate::ipc::CONFIGURE_MASK_ALL != 0 {
            bump(&DENIED);
            return Err(KError::InvalidArgument.id() as u64);
        }
        if mask != 0 {
            // D2-83: core **không** đọc bộ nhớ user — arch đọc vùng 608..640 **cùng lần** với
            // header IPC buffer (`SyscallEnv::ipc_header`, một trang + một magic).
            let Some(header) = env.ipc_header(tcb) else {
                bump(&DENIED);
                return Err(KError::InvalidArgument.id() as u64);
            };
            let extra = header.configure;
            if mask & crate::ipc::CONFIGURE_MASK_CSPACE != 0 {
                let cptr = usize::try_from(extra.cspace)
                    .map_err(|_| KError::InvalidCapability.id() as u64)?;
                if cptr >= m.cnode_slot_count(cspace).map_err(|e| e.id() as u64)? {
                    bump(&DENIED);
                    return Err(KError::InvalidCapability.id() as u64);
                }
                params.cspace = Some(SlotRef::new(cspace, cptr));
            }
            if mask & crate::ipc::CONFIGURE_MASK_VSPACE != 0 {
                // **KT-320** — CPTR `VSpace` giải trong CSpace **caller** (như `cspace?`); kiểm
                // **loại** ở `Machine::tcb_configure_invocation` (D2-83: core chỉ giữ danh tính).
                //
                // **KT-325** — CPTR **rỗng** (`0`) = **xoá** `vspace?` ⇒ về "dùng VSpace của ảnh
                // đã nạp" (`TcbData.vspace = 0`). `0` **không** phải phép tra ô 0: ô 0 của CSpace
                // thường là cap `CNode` (`CS_SELF`) nên nếu tra sẽ ra `WrongObjectType` 2 — đây
                // là **ngữ nghĩa mới có chủ đích** (task KT-325), không phải nới ngưỡng; CPTR
                // **khác 0** mà không phải `VSpace` **vẫn** ⇒ 2.
                if extra.vspace == 0 {
                    params.vspace = crate::core_tcb::VspaceUpdate::Clear;
                } else {
                    let cptr = usize::try_from(extra.vspace)
                        .map_err(|_| KError::InvalidCapability.id() as u64)?;
                    if cptr >= m.cnode_slot_count(cspace).map_err(|e| e.id() as u64)? {
                        bump(&DENIED);
                        return Err(KError::InvalidCapability.id() as u64);
                    }
                    params.vspace = crate::core_tcb::VspaceUpdate::Set(SlotRef::new(cspace, cptr));
                }
            }
            if mask & crate::ipc::CONFIGURE_MASK_ENTRY != 0 {
                params.entry = Some(extra.entry);
            }
            if mask & crate::ipc::CONFIGURE_MASK_PRIORITY != 0 {
                let Some(priority) = u8::try_from(extra.priority).ok() else {
                    bump(&DENIED);
                    return Err(KError::InvalidArgument.id() as u64);
                };
                params.priority = Some(priority);
            }
        }
    }
    // **D2-242 nhóm 3 (3.1A)** — giá trị `vspace` **trước** khi áp, để rollback nếu hook arch
    // báo `VspaceOutcome::Failed` (bảng khe đầy). Đọc **trước** `tcb_configure_invocation`; lỗi
    // đọc (TCB vừa kiểm sống) chỉ có thể là bất biến nội bộ ⇒ `None` = không rollback được.
    let prev_vspace = if params.vspace != crate::core_tcb::VspaceUpdate::Keep {
        m.tcb_vspace(c.obj).ok()
    } else {
        None
    };
    m.tcb_configure_invocation(slot, params).map_err(|e| {
        bump(&DENIED);
        e.id() as u64
    })?;
    // **KT-320** — giao **VSpace** đã cấu hình cho arch (D2-83: `satp`/PTE thuộc arch; core chỉ
    // giữ danh tính). Cùng khuôn `SyscallEnv::tcb_resume` (KT-314): hook **giữ** giá trị, arch
    // **tiêu thụ** ở lần vào U-mode kế tiếp của đúng TCB. Nền tảng không có U-mode trả `false`.
    //
    // **KT-325** — hook cũng được gọi khi **xoá** (CPTR rỗng), với `vspace = 0` = giá trị **kết
    // quả** vừa đọc từ `TcbData` ⇒ arch **quên** danh tính cũ (không còn hai nguồn sự thật lệch).
    if params.vspace != crate::core_tcb::VspaceUpdate::Keep {
        if let Ok(vspace) = m.tcb_vspace(c.obj) {
            match env.tcb_vspace(c.obj, vspace) {
                // Đường xanh: arch đã giữ danh tính.
                VspaceOutcome::Applied => {}
                // Nền tảng không có U-mode: danh tính trong core là hiệu ứng model (KT-320).
                VspaceOutcome::Unsupported => {}
                // **Fail-closed (D2-242 nhóm 3)**: arch **không** giữ được VSpace (vd bảng khe
                // đầy) ⇒ rollback `TcbData.vspace` về giá trị **trước** rồi trả mã lỗi ABI
                // (`Busy` 14 ở riscv64). Không để core nói "đã cấu hình" trong khi arch chưa có.
                VspaceOutcome::Failed(err) => {
                    if let Some(prev) = prev_vspace {
                        let _ = m.tcb_restore_vspace(c.obj, prev);
                    }
                    bump(&DENIED);
                    return Err(err.id() as u64);
                }
            }
        }
    }
    bump(&CONFIGURED);
    bump(&ALLOWED);
    Ok((0, 0))
}

// =============================================================================================
// KT-314 — `Tcb.Resume` (method 12 của `[object].Tcb`) qua invocation từ U-mode
// =============================================================================================

/// `Tcb.Resume` — chuyển **tác vụ đích** (cap TCB đã gọi) sang `Running` và giao điểm vào cho
/// arch đặt `sepc`.
///
/// # Kênh tham số
///
/// **Arity chính xác 0** (D2-104): `[method_signature."Tcb.Resume"]` khai `params = ["tcb"]`,
/// `returns = []` — `tcb` là **chính cap đã gọi**, nên invocation **không** mang MR nào.
/// `length` khác 0 ⇒ `InvalidArgument` **5**, có đếm.
///
/// # Ngữ nghĩa
///
/// 1. Cap TCB phải có `RIGHT_WRITE`: thiếu ⇒ `InsufficientRights` **3** (cùng luật quyền như
///    `Tcb.Configure`/`Tcb.SetTls`, D2-99(e)).
/// 2. [`Machine::tcb_resume_invocation`] kiểm **trạng thái trước** (`Ready`/`Inactive` chưa từng
///    chạy), rồi `TcbData.entry` (`0` ⇒ `InvalidArgument` 5 — fail-closed), rồi chuyển
///    `Running{hart của caller}` + xoá `exit_code` (D2-25).
/// 3. **Giao `entry` cho arch** qua [`SyscallEnv::tcb_resume`] — D2-83: core **không** ghi `sepc`
///    (CSR của arch). Arch nạp `sepc` ở lần vào U-mode kế tiếp của **đúng** TCB đó.
///
/// Trả `a1 = entry` (giá trị đã nạp vào `sepc`) để TAP/người đọc log đối chiếu được **cùng một
/// số** với `TcbData.entry` — không có đường "trả 0 giả".
fn resume(
    m: &mut Machine,
    tcb: ObjId,
    slot: SlotRef,
    c: crate::error::CapInfo,
    _args: [u64; 4],
    len: usize,
    env: &mut impl SyscallEnv,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        bump(&DENIED);
        return Err(KError::InsufficientRights.id() as u64);
    }
    if len != 0 {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    // Hart của **caller** (tác vụ đang gọi invocation) — `Resume` đưa đích lên chạy trên hart
    // đó; caller không `Running` (không có hart) ⇒ `0` (mô hình một hart của ảnh test).
    let hart = match m.tcb_state(tcb) {
        Ok(crate::tcb::TcbState::Running { hart }) => hart,
        _ => 0,
    };
    let entry = m.tcb_resume_invocation(slot, hart).map_err(|e| {
        bump(&DENIED);
        e.id() as u64
    })?;
    // D2-83: core **giao** điểm vào, arch ghi `sepc`. Nền tảng không có U-mode trả `false` —
    // không có `sepc` để nạp, trạng thái `Running` vẫn là hiệu ứng model (đã kiểm ở core).
    let _ = env.tcb_resume(c.obj, entry);
    bump(&RESUMED);
    bump(&ALLOWED);
    Ok((0, entry))
}

// =============================================================================================
// KT-319 — `Tcb.Suspend` (method 3 của `[object].Tcb`) qua invocation từ U-mode
// =============================================================================================

/// `Tcb.Suspend` — treo **tác vụ đích** (cap TCB đã gọi): `Running`/`Ready` → `Inactive`.
///
/// # Kênh tham số
///
/// **Arity chính xác 1** (D2-104): `[method_signature."Tcb.Suspend"]` khai
/// `params = ["tcb", "mode"]` — `tcb` là **chính cap đã gọi**, nên invocation mang **một** MR:
/// `MR0 = mode` (`0` = `Abort`, `1` = `Defer`; `variants.mode` của spec). `length` khác 1 ⇒
/// `InvalidArgument` **5**, có đếm.
///
/// # Ngữ nghĩa
///
/// 1. Cap TCB phải có `RIGHT_WRITE`: thiếu ⇒ `InsufficientRights` **3** (cùng luật quyền như
///    `Tcb.Configure`/`Tcb.SetTls`/`Tcb.Resume`, D2-99(e)).
/// 2. `mode = Defer` ⇒ **KT-329** nhận (đặt cờ `Defer`, **không** đổi trạng thái; TCB dừng ở
///    điểm an toàn kế tiếp, khôi phục bằng `Tcb.Resume`); mode lạ ⇒ `InvalidArgument` **5**.
/// 3. [`Machine::tcb_suspend_invocation`] kiểm **trạng thái trước**: chỉ `Running`/`Ready` ⇒ đi;
///    `Inactive`/`BlockedOn*`/`Sleeping`/`Stopping`/`Dead` ⇒ `IllegalOperation` **17**.
/// 4. Chuyển `Inactive` qua **một** cửa `transition_tcb` (bảng 11×11 D2-49(d) vẫn là nguồn sự
///    thật); **giữ** `TcbData.entry` (để `Tcb.Resume` sau đó chạy lại được) và `exit_code`
///    (D2-25: chỉ `Configure`/`Resume` xoá).
///
/// Trả `(a0, a1) = (0, 0)` — `[method_signature."Tcb.Suspend"].returns = []`.
fn suspend(
    m: &mut Machine,
    slot: SlotRef,
    c: crate::error::CapInfo,
    args: [u64; 4],
    len: usize,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        bump(&DENIED);
        return Err(KError::InsufficientRights.id() as u64);
    }
    if len != 1 {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    let mode = match args[0] {
        SUSPEND_MODE_ABORT => crate::tcb::SuspendMode::Abort,
        SUSPEND_MODE_DEFER => crate::tcb::SuspendMode::Defer,
        _ => {
            bump(&DENIED);
            return Err(KError::InvalidArgument.id() as u64);
        }
    };
    m.tcb_suspend_invocation(slot, mode).map_err(|e| {
        bump(&DENIED);
        e.id() as u64
    })?;
    match mode {
        crate::tcb::SuspendMode::Abort => bump(&SUSPENDED),
        crate::tcb::SuspendMode::Defer => bump(&DEFERRED),
    }
    bump(&ALLOWED);
    Ok((0, 0))
}

// =============================================================================================
// KT-417 — `Tcb.BindNotification` (13) / `Tcb.UnbindNotification` (14) — D2-261 nhóm 2
// =============================================================================================

/// `Tcb.BindNotification` — gắn **1:1** một `Notification` vào cap TCB đã gọi.
///
/// # Kênh tham số
///
/// | Tham số | Nguồn |
/// |---|---|
/// | `notification` | `MR0` = CPTR tới cap `Notification` trong CSpace của **caller** |
///
/// **Arity chính xác 1** (D2-104): thiếu/thừa ⇒ `InvalidArgument` **5**. Cap TCB phải có
/// `RIGHT_WRITE` (D2-99(e)); thiếu ⇒ `InsufficientRights` **3**. Notification đã gắn TCB khác
/// ⇒ `Busy` **14**; TCB đã gắn Notification khác ⇒ `IllegalOperation` **17** (model API
/// [`Machine::tcb_bind_notification`]). Trả `a1 = 0`.
fn bind_notification(
    m: &mut Machine,
    tcb: ObjId,
    call_slot: SlotRef,
    c: crate::error::CapInfo,
    args: [u64; 4],
    len: usize,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        bump(&DENIED);
        return Err(KError::InsufficientRights.id() as u64);
    }
    if len != 1 {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    let cspace = m
        .tcb_info_obj(tcb)
        .map_err(|_| KError::InvalidCapability.id() as u64)?
        .cspace
        .ok_or(KError::InvalidCapability.id() as u64)?;
    let slot = slot_arg(args[0]).inspect_err(|_| {
        bump(&DENIED);
    })?;
    if slot >= m.cnode_slot_count(cspace).map_err(|e| e.id() as u64)? {
        bump(&DENIED);
        return Err(KError::InvalidCapability.id() as u64);
    }
    let ntfn = SlotRef::new(cspace, slot);
    m.tcb_bind_notification(call_slot, ntfn).map_err(|e| {
        bump(&DENIED);
        e.id() as u64
    })?;
    bump(&NOTIF_BOUND);
    bump(&ALLOWED);
    Ok((0, 0))
}

/// `Tcb.UnbindNotification` — gỡ binding Notification của cap TCB đã gọi (**idempotent**).
///
/// **Arity chính xác 0** (D2-104): có MR ⇒ `InvalidArgument` **5**. Cap TCB phải có
/// `RIGHT_WRITE`; thiếu ⇒ `InsufficientRights` **3**. Trả `a1 = 0`.
fn unbind_notification(
    m: &mut Machine,
    call_slot: SlotRef,
    c: crate::error::CapInfo,
    len: usize,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        bump(&DENIED);
        return Err(KError::InsufficientRights.id() as u64);
    }
    if len != 0 {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    m.tcb_unbind_notification(call_slot).map_err(|e| {
        bump(&DENIED);
        e.id() as u64
    })?;
    bump(&NOTIF_UNBOUND);
    bump(&ALLOWED);
    Ok((0, 0))
}

// =============================================================================================
// KT-68 — `CNode.Copy/Mint/Move/Delete/Revoke` (capalloc)
// =============================================================================================

/// Đổi một CPTR/`u64` tham số thành chỉ số slot; tràn `usize` ⇒ `InvalidCapability` (như
/// [`cap_slot`] của syscall).
fn slot_arg(raw: u64) -> Result<usize, u64> {
    usize::try_from(raw).map_err(|_| KError::InvalidCapability.id() as u64)
}

/// `rights?` của `Copy/Mint`: 8 bit thấp; giá trị > 0xFF ⇒ `InvalidArgument` (không cắt ngầm).
fn rights_arg(raw: u64) -> Result<u8, u64> {
    u8::try_from(raw).map_err(|_| KError::InvalidArgument.id() as u64)
}

/// `CNode.*` — thực thi trên **CNode của cap được gọi** (target), nguồn ở **CSpace của caller**.
///
/// # Kênh tham số (D2-99: `MR0..MR3`, `length` = số MR dùng; không đổi `MessageInfo` 35 bit)
///
/// | Method (id spec) | MR0 | MR1 | MR2 | MR3 |
/// |---|---|---|---|---|
/// | `Copy` (0) | `src` CPTR trong CSpace caller | `dst` slot trong CNode target | `rights?` | — |
/// | `Mint` (1) | `src` | `dst` | `rights?` | `badge?` |
/// | `Move` (2) | `src` | `dst` | — | — |
/// | `Delete` (4) | `slot` trong CNode target | — | — | — |
/// | `Revoke` (5) | `slot?` (bỏ trống ⇒ mọi slot) | — | — | — |
/// | `Mutate` (3) | `slot` trong CNode target | `rights?` | `badge?` | — |
/// | `Rotate` (6) | `slot_a` | `slot_b` | — | — |
///
/// **Arity là CHÍNH XÁC** (D2-104): `Copy` = 2 hoặc 3; `Mint` = 2, 3 hoặc 4; `Move` = 2;
/// `Delete` = 1; `Revoke` = 0 hoặc 1. `length` **khác** các giá trị đó (thiếu **hoặc** thừa)
/// ⇒ `InvalidArgument` (5), có đếm — không bỏ qua tham số dư (hai bên sẽ hiểu khác nhau về cùng
/// một message). `Copy/Mint` bỏ `MR2` ⇒ giữ quyền/badge nguồn; `Mint` bỏ `MR3` ⇒ giữ badge
/// nguồn. `badge = 0` có mặt ⇒ **xoá** badge (ngữ nghĩa `Machine::mint`). Cap CNode được gọi
/// phải có `RIGHT_WRITE` (model F3, oracle cũ `vifast-model::invoke_*` — crate đã gỡ ở
/// KT-410/`B2`); thiếu ⇒
/// `InsufficientRights` (3). Đích phải nằm trong **CNode target** (không phải "CNode bất kỳ"
/// mà cap trỏ tới) — cùng ngữ nghĩa `check_dst_in_cnode` của model.
///
/// Trả `a1`: `Revoke` = số cap bị xoá; `Copy/Mint/Move/Delete` = 0.
fn cnode(
    m: &mut Machine,
    tcb: ObjId,
    c: crate::error::CapInfo,
    method: u32,
    args: [u64; 4],
    len: usize,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        return Err(KError::InsufficientRights.id() as u64);
    }
    let cspace = m
        .tcb_info_obj(tcb)
        .map_err(|_| KError::InvalidCapability.id() as u64)?
        .cspace
        .ok_or(KError::InvalidCapability.id() as u64)?;
    let target = c.obj;
    // D2-104: arity **chính xác** theo `spec/[method_signature]` (optional chỉ ở cuối) —
    // kiểm **trước** khi đổi CPTR để tham số dư không bao giờ chạm máy trạng thái.
    let arity_ok = match method {
        CNODE_COPY => len == 2 || len == 3,
        CNODE_MINT => (2..=4).contains(&len),
        CNODE_MOVE => len == 2,
        CNODE_MUTATE => (1..=3).contains(&len),
        CNODE_DELETE => len == 1,
        CNODE_REVOKE => len <= 1,
        CNODE_ROTATE => len == 2,
        _ => true,
    };
    if !arity_ok {
        bump(&DENIED);
        return Err(KError::InvalidArgument.id() as u64);
    }
    // Nguồn ở CSpace **caller**, đích ở CNode **target** (model F3: `check_dst_in_cnode`).
    let src = SlotRef::new(cspace, slot_arg(args[0])?);
    let dst = SlotRef::new(target, slot_arg(args[1])?);
    match method {
        CNODE_COPY => {
            let rights = if len >= 3 {
                Some(rights_arg(args[2])?)
            } else {
                None
            };
            m.copy_rights(src, dst, rights).map_err(|e| e.id() as u64)?;
            bump(&CNODE_COPIED);
            bump(&ALLOWED);
            Ok((0, 0))
        }
        CNODE_MINT => {
            // `rights?:` thiếu ⇒ giữ quyền nguồn (phải đọc trước khi `mint` kiểm lại).
            let rights = if len >= 3 {
                rights_arg(args[2])?
            } else {
                let cap = m.cap_at(cspace, src.slot).map_err(|e| e.id() as u64)?;
                m.cap_info(cap).map_err(|e| e.id() as u64)?.rights
            };
            let badge = if len >= 4 { Some(args[3]) } else { None };
            m.mint(src, dst, rights, badge).map_err(|e| e.id() as u64)?;
            bump(&CNODE_MINTED);
            bump(&ALLOWED);
            Ok((0, 0))
        }
        CNODE_MOVE => {
            m.move_cap(src, dst).map_err(|e| e.id() as u64)?;
            bump(&CNODE_MOVED);
            bump(&ALLOWED);
            Ok((0, 0))
        }
        CNODE_DELETE => {
            // `Delete` nhận `MR0 = slot` (spec `params = ["cnode","slot"]`), không dùng `MR1`.
            let target_slot = SlotRef::new(target, slot_arg(args[0])?);
            m.delete(target_slot).map_err(|e| e.id() as u64)?;
            bump(&CNODE_DELETED);
            bump(&ALLOWED);
            Ok((0, 0))
        }
        CNODE_REVOKE => {
            let slot = if len >= 1 {
                Some(slot_arg(args[0])?)
            } else {
                None
            };
            let deleted = m.revoke(target, slot).map_err(|e| e.id() as u64)?;
            bump(&CNODE_REVOKED);
            bump(&ALLOWED);
            Ok((0, deleted as u64))
        }
        // **KT-417** — `Mutate` (3): biến đổi **tại chỗ** cap ở slot thuộc CNode target
        // (D2-261 nhóm 6, D2-44): `rights?` = tập tên đầy đủ như `Copy`/`Mint`, `badge?` theo
        // quy ước `Mint`; KHÔNG đổi cha/con MDB, KHÔNG đổi slot.
        CNODE_MUTATE => {
            let slot = SlotRef::new(target, slot_arg(args[0])?);
            let rights = if len >= 2 {
                Some(rights_arg(args[1])?)
            } else {
                None
            };
            let badge = if len >= 3 { Some(args[2]) } else { None };
            m.mutate(slot, rights, badge).map_err(|e| e.id() as u64)?;
            bump(&CNODE_MUTATED);
            bump(&ALLOWED);
            Ok((0, 0))
        }
        // **KT-417** — `Rotate` (6): hoán vị nội dung hai slot của **cùng** CNode target (`[TK]`).
        CNODE_ROTATE => {
            let a = SlotRef::new(target, slot_arg(args[0])?);
            let b = SlotRef::new(target, slot_arg(args[1])?);
            m.rotate(a, b).map_err(|e| e.id() as u64)?;
            bump(&CNODE_ROTATED);
            bump(&ALLOWED);
            Ok((0, 0))
        }
        // Phần **chưa cài** (D2-53 Q11): `NotSupported` 18, có đếm — không tự đặt ngữ nghĩa.
        _ => {
            bump(&DENIED);
            Err(KError::NotSupported.id() as u64)
        }
    }
}

fn retype(
    m: &mut Machine,
    tcb: ObjId,
    c: crate::error::CapInfo,
    args: [u64; 4],
    env: &mut impl SyscallEnv,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        return Err(KError::InsufficientRights.id() as u64);
    }
    let (dst, cspace) = install_slot(m, tcb, args[0]).map_err(|e| e.id() as u64)?;
    let ty = retype_target(args[1]).ok_or(KError::NotSupported.id() as u64)?;
    // `size_bits` do hook arch kiểm theo cỡ Frame/PageTable/VSpace của object layer; core chỉ
    // chặn giá trị không thể là `u8` hợp lệ (> 30) để lỗi ABI không lẫn với tràn số.
    if args[2] > 30 {
        return Err(KError::InvalidArgument.id() as u64);
    }
    let flags = u32::try_from(args[3]).map_err(|_| KError::InvalidArgument.id() as u64)?;
    if flags & crate::spec::FLAG_GUEST != 0 {
        return Err(if ty == ObjType::VSpace {
            KError::NotSupported.id() as u64
        } else {
            KError::InvalidArgument.id() as u64
        });
    }
    // Đích phải rỗng: kiểm **trước** khi cấp byte bên object layer (nguyên tử).
    match m.cap_at(cspace, dst) {
        Err(KError::SlotEmpty) => {}
        Ok(_) => return Err(KError::SlotOccupied.id() as u64),
        Err(e) => return Err(e.id() as u64),
    }
    let op = Invocation {
        ty: c.ty,
        method: METHOD_RETYPE,
        obj: c.obj,
        aux_obj: 0,
        key: 0,
        aux_key: 0,
        args,
    };
    let outcome = env.invoke(op.op())?;
    let InvokeOutcome::Object { ty: made, key } = outcome else {
        bump(&DENIED);
        return Err(KError::NotSupported.id() as u64);
    };
    // L1 (reviewer-kt67a): object layer phải trả **đúng loại** caller yêu cầu; lệch ⇒ trả
    // object về object layer, không cài cap sai kiểu vào CSpace.
    if made != ty {
        env.release_object(key);
        return Err(KError::NotSupported.id() as u64);
    }
    // Object layer đã cấp ⇒ core dựng **object đại diện** + cap rồi cài vào CSpace. Lỗi core
    // ⇒ **rollback đủ ba bước** (L2): bỏ cap gốc chưa vào slot → đánh dấu object đại diện chết
    // → trả object/byte về object layer. Không còn cap mồ côi trỏ object chết.
    match m.add_foreign_object(made, key) {
        Ok(obj) => {
            let cap = m.root_cap(obj);
            match m.put_cap(cspace, dst, cap) {
                Ok(()) => {
                    bump(&RETYPE);
                    bump(&ALLOWED);
                    // `a1` = slot đích — **hợp đồng ABI** từ D2-99: `spec/abi.toml`
                    // `[method_signature."Untyped.Retype"].returns = ["slot"]` (KT-67b cập nhật
                    // spec đúng một lần theo D2-99).
                    Ok((0, dst as u64))
                }
                Err(e) => {
                    // Slot đích đã kiểm rỗng trước khi cấp ⇒ nhánh này là vi phạm bất biến nội
                    // bộ; vẫn rollback đủ để **không** để lại cap/object mồ côi.
                    if m.drop_root_cap(cap).is_ok() {
                        let _ = m.kill_foreign_object(obj);
                        env.release_object(key);
                    }
                    Err(e.id() as u64)
                }
            }
        }
        Err(e) => {
            env.release_object(key);
            Err(e.id() as u64)
        }
    }
}

/// Cap phụ `VSpace` cho `Frame.Map/Unmap`: kiểm loại + lấy khoá object layer.
fn aux_vspace(m: &Machine, tcb: ObjId, raw: u64) -> Result<(ObjId, u64), u64> {
    let cspace = m
        .tcb_info_obj(tcb)
        .map_err(|_| KError::InvalidCapability.id() as u64)?
        .cspace
        .ok_or(KError::InvalidCapability.id() as u64)?;
    let slot = usize::try_from(raw).map_err(|_| KError::InvalidCapability.id() as u64)?;
    if slot >= m.cnode_slot_count(cspace).map_err(|e| e.id() as u64)? {
        return Err(KError::InvalidCapability.id() as u64);
    }
    let cap = m.cap_at(cspace, slot).map_err(|e| e.id() as u64)?;
    let c = m.cap_info(cap).map_err(|e| e.id() as u64)?;
    if c.ty != ObjType::VSpace {
        return Err(KError::WrongObjectType.id() as u64);
    }
    // M2 (reviewer-kt67a): cap VSpace **chỉ đọc** không được map/unmap — thao tác này sửa PTE.
    if c.rights & RIGHT_WRITE == 0 {
        return Err(KError::InsufficientRights.id() as u64);
    }
    let key = m.foreign_key(c.obj).map_err(|e| e.id() as u64)?;
    Ok((c.obj, key))
}

fn map(
    m: &mut Machine,
    tcb: ObjId,
    c: crate::error::CapInfo,
    args: [u64; 4],
    env: &mut impl SyscallEnv,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        return Err(KError::InsufficientRights.id() as u64);
    }
    let (aux_obj, aux_key) = aux_vspace(m, tcb, args[0])?;
    let key = m.foreign_key(c.obj).map_err(|e| e.id() as u64)?;
    let _ = map_flags(args[2]).map_err(|e| e.id() as u64)?;
    let op = Invocation {
        ty: c.ty,
        method: METHOD_MAP,
        obj: c.obj,
        aux_obj,
        key,
        aux_key,
        args,
    };
    let value = env.invoke(op.op())?;
    let InvokeOutcome::Value(value) = value else {
        bump(&DENIED);
        return Err(KError::NotSupported.id() as u64);
    };
    bump(&MAP);
    bump(&ALLOWED);
    Ok((0, value))
}

fn unmap(
    m: &mut Machine,
    tcb: ObjId,
    c: crate::error::CapInfo,
    args: [u64; 4],
    env: &mut impl SyscallEnv,
) -> Result<(u64, u64), u64> {
    if c.rights & RIGHT_WRITE == 0 {
        return Err(KError::InsufficientRights.id() as u64);
    }
    let (aux_obj, aux_key) = aux_vspace(m, tcb, args[0])?;
    let key = m.foreign_key(c.obj).map_err(|e| e.id() as u64)?;
    let op = Invocation {
        ty: c.ty,
        method: METHOD_UNMAP,
        obj: c.obj,
        aux_obj,
        key,
        aux_key,
        args,
    };
    let value = env.invoke(op.op())?;
    let InvokeOutcome::Value(value) = value else {
        bump(&DENIED);
        return Err(KError::NotSupported.id() as u64);
    };
    bump(&UNMAP);
    bump(&ALLOWED);
    Ok((0, value))
}

/// `RIGHTS_ALL` — cap gốc của object đại diện (đủ 4 bit như `Machine::root_cap`); tái xuất cho
/// tài liệu/TAP đọc đúng hằng.
pub const FOREIGN_CAP_RIGHTS: u8 = RIGHTS_ALL;

/// `true` ⇔ `(ty, method)` nằm trong danh sách trắng của KT-67a (dùng cho docs/TAP).
pub const fn is_whitelisted(ty: ObjType, method: u32) -> bool {
    matches!(
        (ty, method),
        (ObjType::Untyped, METHOD_RETYPE)
            | (ObjType::Frame, METHOD_MAP)
            | (ObjType::Frame, METHOD_UNMAP)
            | (ObjType::CNode, CNODE_COPY)
            | (ObjType::CNode, CNODE_MINT)
            | (ObjType::CNode, CNODE_MOVE)
            | (ObjType::CNode, CNODE_DELETE)
            | (ObjType::CNode, CNODE_REVOKE)
            | (ObjType::CNode, CNODE_MUTATE)
            | (ObjType::CNode, CNODE_ROTATE)
            | (ObjType::Tcb, TCB_BIND_NOTIFICATION)
            | (ObjType::Tcb, TCB_UNBIND_NOTIFICATION)
            | (ObjType::Tcb, TCB_CONFIGURE)
            | (ObjType::Tcb, TCB_SET_TLS)
            | (ObjType::Tcb, TCB_RESUME)
            | (ObjType::Tcb, TCB_SUSPEND)
    )
}

/// `true` ⇔ `ty` là loại VM mà invocation Retype được phép tạo (đợt 1).
pub const fn retype_supported(ty: ObjType) -> bool {
    matches!(ty, ObjType::Frame | ObjType::PageTable | ObjType::VSpace)
}

/// Chuyển `ObjType` → id `[object]` (chỉ loại VM; dùng cho chẩn đoán).
pub fn type_id(ty: ObjType) -> Option<u64> {
    match ty {
        ObjType::Untyped => object_id("Untyped"),
        ObjType::Frame => object_id("Frame"),
        ObjType::PageTable => object_id("PageTable"),
        ObjType::VSpace => object_id("VSpace"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cờ map: `R`/`W ⇒ R`/`X`/`U`/`Device` hợp lệ; 0 quyền, `W` không `R`, bit lạ ⇒
    /// `InvalidArgument` (5).
    #[test]
    fn map_flags_kiem_bien() {
        assert_eq!(map_flags(MAP_READ), Ok(MAP_READ));
        assert_eq!(map_flags(MAP_READ | MAP_WRITE), Ok(MAP_READ | MAP_WRITE));
        assert_eq!(map_flags(MAP_READ | MAP_EXEC), Ok(MAP_READ | MAP_EXEC));
        assert_eq!(
            map_flags(MAP_READ | MAP_WRITE | MAP_USER),
            Ok(MAP_READ | MAP_WRITE | MAP_USER)
        );
        assert_eq!(
            map_flags(MAP_READ | MAP_WRITE | MAP_DEVICE),
            Ok(MAP_READ | MAP_WRITE | MAP_DEVICE)
        );
        assert_eq!(map_flags(0), Err(KError::InvalidArgument));
        assert_eq!(map_flags(MAP_WRITE), Err(KError::InvalidArgument));
        assert_eq!(map_flags(MAP_READ | (1 << 5)), Err(KError::InvalidArgument));
    }

    /// Id object lấy từ `spec/abi.toml` (`vifast_abi`), không chép số: Frame 11, PageTable 10,
    /// VSpace 9; loại lõi (Tcb) không có id trong bảng VM.
    #[test]
    fn type_id_sinh_tu_spec() {
        assert_eq!(type_id(ObjType::Frame), object_id("Frame"));
        assert_eq!(type_id(ObjType::PageTable), object_id("PageTable"));
        assert_eq!(type_id(ObjType::VSpace), object_id("VSpace"));
        assert_eq!(object_id("Frame"), Some(11));
        assert_eq!(object_id("PageTable"), Some(10));
        assert_eq!(object_id("VSpace"), Some(9));
        assert_eq!(type_id(ObjType::Tcb), None);
    }

    /// Danh sách trắng hẹp: đúng ba cặp VM (Untyped.Retype, Frame.Map, Frame.Unmap) + bảy cặp
    /// `CNode` (`Copy/Mint/Move/Mutate/Delete/Revoke/Rotate`) + các cặp `Tcb` đã cài
    /// (`Configure/SetTls/Resume/Suspend/BindNotification/UnbindNotification`); method **không**
    /// có trong `[object]` (vd `CNode` 7) và mọi cặp khác **không** nằm trong danh sách
    /// (⇒ `NotSupported`).
    #[test]
    fn danh_sach_trang_hep() {
        assert!(is_whitelisted(ObjType::Untyped, METHOD_RETYPE));
        assert!(is_whitelisted(ObjType::Frame, METHOD_MAP));
        assert!(is_whitelisted(ObjType::Frame, METHOD_UNMAP));
        for method in [
            CNODE_COPY,
            CNODE_MINT,
            CNODE_MOVE,
            CNODE_MUTATE,
            CNODE_DELETE,
            CNODE_REVOKE,
            CNODE_ROTATE,
        ] {
            assert!(is_whitelisted(ObjType::CNode, method));
        }
        // **KT-417**: `Mutate` (3) / `Rotate` (6) **đã cài**; method 7 vẫn ngoài bảng `[object]`.
        assert!(is_whitelisted(ObjType::CNode, 3));
        assert!(is_whitelisted(ObjType::CNode, 6));
        assert!(!is_whitelisted(ObjType::CNode, 7));
        assert!(!is_whitelisted(ObjType::Frame, 2));
        assert!(!is_whitelisted(ObjType::Untyped, 7));
        assert!(!is_whitelisted(ObjType::VSpace, METHOD_MAP));
        assert!(!is_whitelisted(ObjType::Endpoint, METHOD_RETYPE));
        // **KT-290**: `Tcb.Configure` (0) **cài**; **KT-319**: `Tcb.Suspend` (3) **cài**;
        // **KT-417**: `BindNotification` (13) / `UnbindNotification` (14) **cài**; method khác
        // của Tcb **chưa** cài.
        assert!(is_whitelisted(ObjType::Tcb, TCB_CONFIGURE));
        assert!(is_whitelisted(ObjType::Tcb, TCB_SUSPEND));
        assert!(is_whitelisted(ObjType::Tcb, TCB_BIND_NOTIFICATION));
        assert!(is_whitelisted(ObjType::Tcb, TCB_UNBIND_NOTIFICATION));
        assert!(!is_whitelisted(ObjType::Tcb, 1), "Tcb.SetFaultEp chưa cài");
        assert!(!is_whitelisted(ObjType::Tcb, 4), "Tcb.SetPriority chưa cài");
        assert!(!is_whitelisted(ObjType::Tcb, vifast_abi::TCB_EXEC));
    }

    /// Số method `CNode` **khớp `spec/abi.toml`** `[object].CNode` — đọc từ spec sinh
    /// (`vifast-abi`), không chép tay: `Copy=0, Mint=1, Move=2, Mutate=3, Delete=4, Revoke=5,
    /// Rotate=6`. Đây là chốt chống "tự đặt số" (yêu cầu KT-68).
    #[test]
    fn so_method_cnode_khop_spec() {
        assert_eq!(
            vifast_abi::CNODE_METHODS,
            [
                ("Copy", 0),
                ("Mint", 1),
                ("Move", 2),
                ("Mutate", 3),
                ("Delete", 4),
                ("Revoke", 5),
                ("Rotate", 6),
            ]
        );
        assert_eq!(CNODE_COPY, 0);
        assert_eq!(CNODE_MINT, 1);
        assert_eq!(CNODE_MOVE, 2);
        assert_eq!(CNODE_DELETE, 4);
        assert_eq!(CNODE_REVOKE, 5);
    }

    /// `Retype` chỉ nhận ba loại VM của object layer (D2-83); loại lõi do core tự cấp — không
    /// đi qua invocation.
    #[test]
    fn retype_chi_ba_loai_vm() {
        assert!(retype_supported(ObjType::Frame));
        assert!(retype_supported(ObjType::PageTable));
        assert!(retype_supported(ObjType::VSpace));
        assert!(!retype_supported(ObjType::Untyped));
        assert!(!retype_supported(ObjType::Tcb));
        assert!(!retype_supported(ObjType::Endpoint));
    }

    /// **L2**: `drop_root_cap` chỉ bỏ cap **chưa vào slot**; cap đang ở slot (hoặc đã chết) ⇒
    /// `IllegalOperation`. Sau khi bỏ, `check_caps` vẫn sạch (không cap mồ côi).
    #[test]
    fn drop_root_cap_chi_cap_chua_vao_slot() {
        let mut m = Machine::<crate::store::StaticStore, crate::observer::TestObserver>::new();
        let root = m.add_cnode(8);
        let u = m.add_untyped(0x1000, 1 << 16);
        let ucap = m.root_cap(u);
        let src = SlotRef::new(root, 0);
        m.put_cap(root, 0, ucap).expect("cap untyped");
        let cap = m.retype(src, ObjType::Tcb, 0, 0).expect("retype tcb");
        assert!(m.drop_root_cap(cap).is_ok());
        assert!(m.check_caps().is_ok());
        assert_eq!(
            m.drop_root_cap(cap),
            Err(KError::IllegalOperation),
            "cap đã chết"
        );
        let cap2 = m.retype(src, ObjType::Tcb, 0, 0).expect("retype tcb 2");
        m.put_cap(root, 1, cap2).expect("put cap");
        assert_eq!(
            m.drop_root_cap(cap2),
            Err(KError::IllegalOperation),
            "cap đang ở slot phải dùng delete"
        );
        assert!(m.check_caps().is_ok());
    }

    /// Số đếm `# inv:` chỉ tăng (TAP đọc **delta**, không phụ thuộc mốc cũ).
    #[test]
    fn so_dem_chi_tang() {
        let before = stats();
        bump(&DENIED);
        let after = stats();
        assert!(after.denied > before.denied);
        assert_eq!(after.allowed, before.allowed);
    }
}
