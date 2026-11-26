// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-74** — `exec(path)`: nạp ELF mới **thay** ảnh hiện tại của tiến trình (K6, thiết kế
//! §10.1 dòng K6 "exec chain" + §4 `SetTls` ở K6).
//!
//! # Vì sao `exec` là **kernel-mediated** (không phải syscall, không phải invocation)
//!
//! ABI hiện tại **không** có đường nào cho U-mode yêu cầu `exec`: `[syscall]` đóng băng 16 số
//! hiệu (`spec/abi.toml` `[frozen].wire_syscalls = true`, D2-15) và `[object].Tcb` **không** có
//! method `Exec` (chỉ `SetTls` 15 là method K6 mới). KT-74 **không** thêm syscall/method
//! (ràng buộc task: *"Không thêm syscall"*), nên `exec` chạy ở **kernel** — cùng tiền lệ
//! **D2-96** ("memsrv POLICY-ONLY, kernel-mediated"): quyết định *exec tiến trình nào sang ảnh
//! nào* là **policy** của procsrv/kernel, cơ chế *nạp ảnh* là phần kernel cài ở đây. Nợ ghi ở
//! `docs/k6-exec.md` §6: mở ABI `Tcb.Exec(path)` khi có VFS thật (path phải đọc từ FS, không
//! phải bảng ảnh nhúng).
//!
//! # Hợp đồng
//!
//! - **Giữ `pid` + `cspace`**: ảnh mới được nạp vào **cùng ô tác vụ** (`pid` = ô bảng dải
//!   `user::load_init_slot`), và TCB **core** của tiến trình **không** bị thay/thu hồi ⇒ CSpace,
//!   cap, `fault_ep`, priority giữ nguyên. `exec` **không** tạo TCB mới.
//! - **Reset**: `entry`/`sepc` = `e_entry` của ELF mới, `sp` = đỉnh stack user, **mọi** thanh
//!   ghi khác = 0, **TLS = 0** (rồi `SetTls` đặt lại nếu cần) — xem
//!   [`crate::user::exec_frame`].
//! - **Giải phóng** VSpace + **mọi** Frame của ảnh cũ (unmap → revoke) ⇒ `objects/frames/caps`
//!   **bằng nhau** trước/sau (`# exec:` in cả ba cặp số).
//! - **Nguyên tử**: mọi kiểm tra/parse ELF và **mọi** đường lỗi của loader đều dọn tài nguyên
//!   vừa cấp (`init_pages::release_with`), và ảnh cũ **chỉ** bị giải phóng **sau khi** ảnh mới
//!   nạp xong ⇒ thất bại ⇒ tiến trình cũ **nguyên vẹn** (cùng `vspace`, `entry`, số đếm).
//!
//! Không `unsafe`: tệp này chỉ gọi object layer + loader (`kernel/vifast-plat-*`, D2-17).

use core::sync::atomic::{AtomicU32, Ordering};

use vifast_core::vm::{CapId, ObjCounts};
use vifast_core::{Machine, SlotRef};

use crate::init_pages::{release_with, LoadedInit};
use crate::obj as aobj;
use crate::user::{self, LoadError};

/// Số ảnh tối đa trong bảng `exec` (bảng nhúng của ảnh test; VFS thật là việc của KT-75+).
pub const MAX_IMAGES: usize = 8;

/// Một ảnh trong bảng: `path` + byte ELF.
#[derive(Clone, Copy)]
pub struct Image {
    /// Đường dẫn logic (`"/b"`, …) — khoá của `exec`.
    pub path: &'static str,
    /// Byte ELF64 riscv64 đã nhúng.
    pub bytes: &'static [u8],
}

impl Image {
    /// Cỡ ảnh (byte) — evidence `# exec:`.
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// `true` ⇔ ảnh rỗng (không dùng; giữ API đầy đủ của `clippy::len_without_is_empty`).
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Bảng ảnh **tĩnh** (không `alloc`) — thay cho FS ở K6.
#[derive(Clone, Copy)]
pub struct ImageTable {
    entries: [Option<Image>; MAX_IMAGES],
    count: usize,
}

impl Default for ImageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageTable {
    /// Bảng rỗng.
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_IMAGES],
            count: 0,
        }
    }

    /// Thêm một ảnh; `false` ⇔ bảng đầy hoặc `path` trùng (không ghi đè im lặng).
    pub fn add(&mut self, path: &'static str, bytes: &'static [u8]) -> bool {
        if self.find(path).is_some() || self.count >= MAX_IMAGES {
            return false;
        }
        self.entries[self.count] = Some(Image { path, bytes });
        self.count += 1;
        true
    }

    /// Số ảnh đã đăng ký.
    pub const fn len(&self) -> usize {
        self.count
    }

    /// `true` ⇔ bảng rỗng.
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Tìm ảnh theo `path`.
    pub fn find(&self, path: &str) -> Option<&Image> {
        self.entries[..self.count]
            .iter()
            .flatten()
            .find(|image| image.path == path)
    }
}

/// Lỗi của `exec` — **không** panic, mọi mã có tên ổn định cho evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecError {
    /// `path` không có trong bảng ảnh (`ENOENT` của exec).
    NotFound,
    /// ELF/loader hỏng — tiến trình cũ **giữ nguyên**.
    Load(LoadError),
}

impl ExecError {
    /// Tên ổn định (evidence `# exec: err=…`).
    pub const fn name(self) -> &'static str {
        match self {
            ExecError::NotFound => "NotFound",
            ExecError::Load(e) => e.name(),
        }
    }
}

/// Ảnh chụp bộ đếm object layer — **cùng** nguồn cho `# exec:` trước/sau mỗi bước.
pub fn counts() -> ObjCounts {
    aobj::machine().counts()
}

/// Kết quả một lần `exec` (đủ số cho TAP, không cấp phát).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecReport {
    /// Ô tác vụ (= `pid` quan sát được) — **phải** không đổi qua `exec`.
    pub pid: u32,
    /// TCB core của tiến trình — **không** đổi (giữ CSpace/cap).
    pub tcb: u32,
    /// VSpace **cũ** (bị giải phóng) và **mới** (đang chạy) — cap id của object layer.
    pub old_vspace: u32,
    /// VSpace mới.
    pub new_vspace: u32,
    /// Entry cũ → entry mới (đọc từ ELF thật, không phải hằng).
    pub old_entry: u64,
    /// Entry mới.
    pub new_entry: u64,
    /// Số Frame thu hồi được của ảnh cũ (`revoke` thành công).
    pub freed_frames: usize,
    /// Số trang của ảnh mới.
    pub new_pages: usize,
    /// Bộ đếm trước/sau — `objects/frames/caps` **phải** bằng nhau (không rò).
    pub before: ObjCounts,
    /// Bộ đếm sau.
    pub after: ObjCounts,
    /// Số lần `exec` **thành công** trên ảnh này (bộ đếm một chiều).
    pub execs: u32,
    /// `true` ⇔ object layer tự kiểm bất biến sau bước này.
    pub check_ok: bool,
}

impl ExecReport {
    /// `frames` **không** đổi (không rò Frame).
    pub fn frames_same(&self) -> bool {
        self.before.frames == self.after.frames
    }

    /// `objects` **không** đổi.
    pub fn objects_same(&self) -> bool {
        self.before.objects == self.after.objects
    }

    /// `caps` **không** đổi.
    pub fn caps_same(&self) -> bool {
        self.before.caps == self.after.caps
    }

    /// `true` ⇔ **cả ba** bộ đếm bằng nhau **và** object layer tự kiểm ĐẠT.
    pub fn no_leak(&self) -> bool {
        self.frames_same() && self.objects_same() && self.caps_same() && self.check_ok
    }

    /// Frame **được tái dùng**: phần Frame ảnh cũ trả lại mà ảnh mới đã lấy (D2-106 — công
    /// thức `recycled = freed - (after.frames - before.frames)`; khi không rò thì bằng `freed`).
    pub fn recycled(&self) -> usize {
        self.freed_frames
            .saturating_sub(self.after.frames.saturating_sub(self.before.frames))
    }
}

/// Bộ đếm `exec` **một chiều** của toàn ảnh (TAP `# exec:` đọc) — không phải ABI.
static EXECS: AtomicU32 = AtomicU32::new(0);

/// Số lần `exec` thành công kể từ khi boot (bộ đếm một chiều — không giảm khi lỗi).
pub fn exec_total() -> u32 {
    EXECS.load(Ordering::Relaxed)
}

/// `exec(path)` trên tiến trình `cur` (ô tác vụ `pid`).
///
/// Thứ tự **bắt buộc** (nguyên tử):
/// 1. tra `path` (không có ⇒ `NotFound`, **không** chạm gì);
/// 2. nạp ELF mới vào VSpace **mới** (lỗi ⇒ `Load`, loader đã tự dọn, tiến trình cũ nguyên);
/// 3. **đặt TLS = 0** trong TCB core (reset — `SetTls` sau đó đặt giá trị mới);
/// 4. giải phóng VSpace + Frame của ảnh **cũ**;
/// 5. hoán đổi `*cur = new`;
/// 6. đo lại bộ đếm + `check_objmem` ⇒ `ExecReport`.
///
/// `untyped` là cap Untyped cấp Frame/PageTable/VSpace (cùng nguồn với lần nạp đầu).
///
/// `m` là core `Machine` (caller đang giữ lock của `core_bridge`) — **không** gọi
/// `with_machine` ở đây: lồng lock là deadlock (đo được ở lần boot đầu của `ktest-exec`).
pub fn exec(
    m: &mut Machine,
    table: &ImageTable,
    cur: &mut LoadedInit,
    pid: usize,
    path: &str,
    untyped: CapId,
    tcb: SlotRef,
) -> Result<ExecReport, ExecError> {
    // (1) Tra path **trước** mọi thao tác — đây là phần "không nửa vời" rẻ nhất.
    let image = table.find(path).ok_or(ExecError::NotFound)?;
    exec_bytes(m, image.bytes, cur, pid, untyped, tcb)
}

/// **KT-347** — `exec` trên **byte ELF** đọc từ FS (thay bảng ảnh nhúng của [`exec`]).
///
/// Cùng **nguyên tử** và cùng thứ tự 6 bước với [`exec`]; khác duy nhất nguồn byte: caller đọc
/// tệp vào buffer (API `proc_fs::read_file`) rồi truyền lát cắt. `bytes` **không** cần `'static`
/// (buffer scratch của kernel), nên không phải nhúng ELF vào ảnh.
pub fn exec_bytes(
    m: &mut Machine,
    bytes: &[u8],
    cur: &mut LoadedInit,
    pid: usize,
    untyped: CapId,
    tcb: SlotRef,
) -> Result<ExecReport, ExecError> {
    let before = counts();
    let old_vspace = u32::from(cur.vspace.0);
    let old_entry = cur.entry;
    // (2) Nạp ảnh mới (atomic: lỗi ⇒ ảnh cũ chưa bị chạm; loader đã trả mọi thứ nó cấp).
    // **Nguyên tử (D3, reviewer KT-74)**: loader `reset()` bảng dải VA của ô này **trước** khi ảnh
    // mới chắc nạp được; lỗi **muộn** (Ob/Physmap/Image) sẽ để ảnh cũ chạy với bảng dải rác nếu
    // không khôi phục ⇒ chụp trước, trả lại đúng bảng cũ khi lỗi.
    let saved_ranges = user::ranges_save();
    let fresh = match user::load_init_slot(bytes, untyped, pid) {
        Ok(fresh) => fresh,
        Err(e) => {
            user::ranges_restore(&saved_ranges);
            return Err(ExecError::Load(e));
        }
    };
    // (3) TLS reset — `exec` tạo **ngữ cảnh mới**, TLS cũ không được rò sang ảnh mới. TCB core
    //     không có (ảnh không dựng core) thì bỏ qua: phần ảnh vẫn đúng, TLS coi như chưa đặt.
    let _ = m.tcb_set_tls(tcb, 0);
    // (4) Giải phóng ảnh cũ **sau khi** ảnh mới đã nạp xong (cùng `m` đang giữ — D2-83).
    let freed_frames = release_with(aobj::machine(), cur.vspace, &cur.pages);
    // (5) Hoán đổi.
    *cur = fresh;
    // (6) Đo lại + tự kiểm object layer.
    let after = counts();
    let check_ok = aobj::machine().check_objmem().is_ok();
    let execs = EXECS.fetch_add(1, Ordering::Relaxed) + 1;
    Ok(ExecReport {
        pid: pid as u32,
        tcb: u32::try_from(tcb.slot).unwrap_or(0),
        old_vspace,
        new_vspace: u32::from(cur.vspace.0),
        old_entry,
        new_entry: cur.entry,
        freed_frames,
        new_pages: cur.pages.len(),
        before,
        after,
        execs,
        check_ok,
    })
}
