// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-69** — danh sách trang của một chương trình U-mode + `LoadedInit::release`.
//!
//! Tách khỏi [`crate::user`] để giữ R1 (≤ 1.500 dòng/tệp). Không đổi ngữ nghĩa nạp: mỗi trang
//! (segment/stack/IPC buffer) được ghi lại `(VA, CapId)` để `release` **gỡ mapping rồi thu hồi
//! Frame** — cần cho ảnh test vì object layer chỉ 64 KiB/`MAX_OBJECTS = 96`, và các pha KT-69
//! (IDL + ring) cần chỗ cho hai chương trình mới sau KT-67a/67b/68.
use vifast_core::vm::CapId;

use crate::obj as aobj;

/// Danh sách trang đã map của một chương trình (VA + cap Frame).
#[derive(Clone, Copy, Debug)]
pub struct InitPages {
    vas: [u64; MAX_INIT_PAGES],
    caps: [CapId; MAX_INIT_PAGES],
    count: usize,
}

impl Default for InitPages {
    fn default() -> Self {
        Self::new()
    }
}

impl InitPages {
    /// Danh sách rỗng.
    pub const fn new() -> Self {
        Self {
            vas: [0; MAX_INIT_PAGES],
            caps: [CapId(0); MAX_INIT_PAGES],
            count: 0,
        }
    }

    /// Ghi một trang; `false` ⇔ vượt [`MAX_INIT_PAGES`] (loader trả `LoadError::Image`).
    pub fn push(&mut self, va: u64, cap: CapId) -> bool {
        if self.count >= MAX_INIT_PAGES {
            return false;
        }
        self.vas[self.count] = va;
        self.caps[self.count] = cap;
        self.count += 1;
        true
    }

    /// Số trang đã ghi.
    pub const fn len(&self) -> usize {
        self.count
    }

    /// `true` ⇔ chưa có trang nào.
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// VA của trang thứ `index`.
    pub fn va(&self, index: usize) -> u64 {
        self.vas[index]
    }

    /// Cap Frame của trang thứ `index`.
    pub fn cap(&self, index: usize) -> CapId {
        self.caps[index]
    }
}

/// Số trang tối đa một chương trình U-mode dùng (3-4 segment + 8 stack + 1 IPC buffer).
pub const MAX_INIT_PAGES: usize = 32;

/// Kết quả nạp `init` — địa chỉ/cờ **thật** của lần boot này.
#[derive(Clone, Copy, Debug)]
pub struct LoadedInit {
    /// Cap VSpace của user process.
    pub vspace: CapId,
    /// PA root page table.
    pub root: u64,
    /// ASID nhãn của VSpace.
    pub asid: u16,
    /// Entry point (`e_entry`).
    pub entry: u64,
    /// Số `PT_LOAD` đã map.
    pub segments: usize,
    /// Số trang segment executable.
    pub text_pages: usize,
    /// Số trang segment chỉ đọc.
    pub ro_pages: usize,
    /// Số trang segment ghi được.
    pub data_pages: usize,
    /// Số trang stack user.
    pub stack_pages: usize,
    /// Cuối vùng ảnh ELF (đã làm tròn trang).
    pub image_end: u64,
    /// Đáy trang guard dưới stack.
    pub stack_guard: u64,
    /// `check_objmem` ĐẠT sau khi nạp.
    pub check_ok: bool,
    /// Số trang leaf có **đồng thời** `W` và `X` (phải 0 — W^X đi lại bằng PTE thật).
    pub wx_pages: usize,
    /// Cờ leaf khớp quyền segment (`R|X` text, `R` rodata, `R|W` data, `U` mọi leaf user).
    pub flags_ok: bool,
    /// Nửa cao (kernel high alias + physmap) **không** map trong VSpace user.
    pub high_half_unmapped: bool,
    /// Trang guard ngay dưới stack user **không** map (walk invalid).
    pub guard_unmapped: bool,
    /// Các trang đã map (KT-69: để `release` gỡ mapping trước khi trả Frame).
    pub pages: InitPages,
}

impl LoadedInit {
    /// Trả **mọi** Frame + PageTable + VSpace của chương trình về object layer (KT-69).
    ///
    /// Gọi **sau** khi đã đo xong mọi TAP của pha: object layer chỉ 64 KiB/`MAX_OBJECTS = 96`,
    /// các pha sau (ring) cần chỗ cho hai chương trình mới. `unmap` trước `revoke` vì `revoke`
    /// Frame còn mapping ⇒ `HasChildren`. Không đổi kết quả TAP nào (chỉ dọn tài nguyên).
    pub fn release(&self) {
        release_pages(self.vspace, &self.pages);
    }
}

/// **KT-157** — map `stack_pages` trang stack user (RW NX) sát `USER_STACK_TOP`; trả số trang đã
/// map. Trang guard ngay dưới **không** map. Tách khỏi `user.rs` để giữ R1 (≤ 1.500 dòng/tệp).
///
/// `stack_pages` được kẹp vào `[1, USER_STACK_BYTES/4096]`; `0`/quá lớn ⇒ dùng mặc định 8 trang.
#[cfg(feature = "user")]
pub fn map_user_stack(
    m: &mut vifast_core::vm::Machine<aobj::PhysmapMem>,
    vspace: CapId,
    untyped: CapId,
    stack_pages: usize,
    init_pages: &mut InitPages,
) -> Result<usize, crate::user::LoadError> {
    use crate::user::{loading_ranges, write_page, LoadError, USER_STACK_BYTES, USER_STACK_TOP};
    use vifast_core::vm::{MapFlags, ObjKind};

    let max_pages = (USER_STACK_BYTES / 4096) as usize;
    let stack_pages = stack_pages.clamp(1, max_pages);
    let stack_bottom = USER_STACK_TOP - (stack_pages as u64) * 4096;
    let mut mapped = 0usize;
    let mut stack_va = stack_bottom;
    while stack_va < USER_STACK_TOP {
        let frame = m
            .retype(untyped, ObjKind::Frame, 12)
            .map_err(LoadError::Obj)?;
        let pa = m.frame_pa(frame).map_err(LoadError::Obj)?;
        if !write_page(pa, &[]) {
            return Err(LoadError::Physmap);
        }
        m.map(vspace, stack_va, frame, MapFlags::RW_NX_USER)
            .map_err(LoadError::Obj)?;
        if !init_pages.push(stack_va, frame) {
            return Err(LoadError::Image);
        }
        if mapped == 0 && !loading_ranges().push(stack_bottom, USER_STACK_TOP) {
            return Err(LoadError::Image);
        }
        mapped += 1;
        stack_va += 4096;
    }
    Ok(mapped)
}

/// **KT-74** — giải phóng `pages` + VSpace `vspace` (dùng chung cho [`LoadedInit::release`] và
/// đường **thất bại** của loader: `exec` hỏng phải **không** để lại Frame/VSpace mồ côi).
///
/// Trả về số Frame thật sự được thu hồi (`revoke` Frame thành công) — evidence `# exec:` đếm
/// `freed=`; VSpace thu hồi riêng.
pub fn release_pages(vspace: CapId, pages: &InitPages) -> usize {
    release_with(aobj::machine(), vspace, pages)
}

/// Như [`release_pages`] nhưng dùng **`m` đang có** — loader đang giữ `&mut Machine` nên không
/// được gọi `aobj::machine()` lần hai (hai `&mut` cùng lúc trên một static).
pub fn release_with(
    m: &mut vifast_core::vm::Machine<aobj::PhysmapMem>,
    vspace: CapId,
    pages: &InitPages,
) -> usize {
    let mut freed = 0usize;
    let mut index = 0;
    while index < pages.len() {
        let _ = m.unmap(vspace, pages.va(index));
        // Bảng flush chỉ 16 slot: xả ngay sau mỗi unmap (không xả ⇒ `NoSlots` ở lần release
        // sau và Frame không được trả).
        let _ = m.drain_flushes(aobj::run_flush);
        if m.revoke(pages.cap(index)).is_ok() {
            freed += 1;
        }
        index += 1;
    }
    let _ = m.revoke(vspace);
    let _ = m.drain_flushes(aobj::run_flush);
    freed
}
