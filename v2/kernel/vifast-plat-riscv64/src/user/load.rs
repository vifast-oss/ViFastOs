// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342b (D2-243)** — miền **NẠP ẢNH** của [`crate::user`]: `load_init*` (parse `vifast-elf`,
//! map segment **theo quyền**, stack user + guard, IPC buffer) + [`LoadError`] + [`write_page`].
//!
//! Tách khỏi `user.rs` cho trần R1 (≤ 1.500 dòng/tệp; mục tiêu `user.rs < 1.100` theo D2-243) —
//! **không** đổi một dòng logic nào. Đường dẫn công khai **giữ nguyên**: `crate::user::load_init`,
//! `crate::user::LoadError`, `crate::user::write_page`, … (xem `pub use` ở `user.rs`).
//!
//! `use super::*;` cố ý: mô-đun con **thấy** hằng/trạng thái/private của `user.rs` (hằng cửa sổ
//! user, bảng dải, `set_load_slot`, …) đúng như khi còn ở cùng tệp.

use super::*;
use core::fmt::Write;

/// Lỗi nạp `init` — đủ để TAP in/log, **không panic** trong đường nạp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// ELF không hợp lệ (`vifast-elf`).
    Elf(ElfError),
    /// Retype/map qua object layer.
    Obj(ObjError),
    /// PA của Frame không nằm trong physmap (RAM 128 MiB).
    Physmap,
    /// Ảnh ELF chạm vùng guard/stack user.
    StackOverlap,
    /// Segment `W+X` (loader kiểm lại ngoài parser).
    Wx,
    /// File offset/độ dài segment ngoài buffer ELF.
    Image,
    /// `satp` không đổi được sang root user.
    Satp,
}

impl LoadError {
    /// Tên ổn định cho log/TAP.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Elf(_) => "Elf",
            Self::Obj(_) => "Obj",
            Self::Physmap => "Physmap",
            Self::StackOverlap => "StackOverlap",
            Self::Wx => "Wx",
            Self::Image => "Image",
            Self::Satp => "Satp",
        }
    }
}

/// Nạp `init` ELF vào một VSpace mới: parse `vifast-elf`, map segment **theo quyền**, map stack
/// user + để hở trang guard, rồi `check_objmem`.
///
/// `untyped` là cap Untyped chứa arena/PA cho VSpace/Frame (KT-52).
pub fn load_init(bytes: &[u8], untyped: CapId) -> Result<LoadedInit, LoadError> {
    load_init_slot(bytes, untyped, 0)
}

/// Như [`load_init`] nhưng ghi bảng dải user vào **ô tác vụ** `slot` (KT-65: nhiều chương trình
/// U-mode trong cùng ảnh test, mỗi chương trình một VSpace).
pub fn load_init_slot(bytes: &[u8], untyped: CapId, slot: usize) -> Result<LoadedInit, LoadError> {
    load_init_slot_with_stack(bytes, untyped, slot, USER_STACK_PAGES)
}

/// Số trang stack user mặc định (8 trang = 32 KiB).
pub const USER_STACK_PAGES: usize = (USER_STACK_BYTES / 4096) as usize;

/// Như [`load_init_slot`] nhưng chọn số trang stack — KT-157 dùng 1 trang cho client C/D.
pub fn load_init_slot_with_stack(
    bytes: &[u8],
    untyped: CapId,
    slot: usize,
    stack_pages: usize,
) -> Result<LoadedInit, LoadError> {
    set_load_slot(slot);
    let image = vifast_elf::parse(bytes, EM_RISCV, USER_BASE, USER_TOP).map_err(LoadError::Elf)?;
    // Ảnh không được chạm guard/stack user; guard nằm ngay dưới stack.
    let mut image_end = USER_BASE;
    for segment in image.segments() {
        let end = segment.memory_end().ok_or(LoadError::Image)?;
        if end > image_end {
            image_end = end;
        }
        if segment.writable() && segment.executable() {
            return Err(LoadError::Wx);
        }
    }
    if image_end > USER_STACK_GUARD_BOTTOM {
        return Err(LoadError::StackOverlap);
    }

    let m = aobj::machine();
    let vspace = m
        .retype(untyped, ObjKind::VSpace, 0)
        .map_err(LoadError::Obj)?;
    // KT-74: danh sách trang của lần nạp **này** — khai báo ngay sau VSpace để **mọi** đường
    // lỗi phía dưới dùng được `bail!` (trả lại Frame/VSpace, không mồ côi).
    let mut init_pages = crate::init_pages::InitPages::new();
    // KT-74 (atomic của `exec`): từ đây **mọi** đường lỗi phải trả Frame/VSpace vừa cấp — nếu
    // không, `exec` hỏng để lại tài nguyên mồ côi và phép đo "objects/frames/caps bằng nhau"
    // của `exec` sẽ sai. `parse` + kiểm W^X/overlap ở **trên** đã chạy trước khi cấp bất cứ thứ
    // gì, nên lỗi hình dạng ELF không bao giờ tới đây; các lỗi còn lại (Obj/Physmap/Image) đều
    // đi qua `bail!`.
    macro_rules! bail {
        ($pages:expr, $e:expr) => {{
            // `m` đang giữ ⇒ dùng `release_with` (không gọi `aobj::machine()` lần hai). Truyền
            // `$pages` qua **đối số macro** vì macro hygiene: `init_pages` khai báo sau macro.
            crate::init_pages::release_with(m, vspace, &$pages);
            return Err($e);
        }};
    }
    let root = match m.vspace_root_pa(vspace) {
        Ok(root) => root,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    let asid = match m.vspace_asid(vspace) {
        Ok(asid) => asid,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    // Chỉ chép **nửa thấp** root kernel (index 0..256 trừ cửa sổ user index 1) để kernel sống
    // khi `satp` trỏ root user; **không** chép nửa cao (high alias/physmap) — U-mode không có
    // đường PTE nào tới đó, và syscall không được dùng physmap sau khi đổi `satp`.
    if !aobj::copy_kernel_low_root(root, USER_ROOT_INDEX) {
        bail!(init_pages, LoadError::Physmap);
    }
    loading_ranges().reset();

    let mut text_pages = 0usize;
    let mut ro_pages = 0usize;
    let mut data_pages = 0usize;
    for segment in image.segments() {
        let flags = MapFlags {
            read: true,
            write: segment.writable(),
            exec: segment.executable(),
            user: true,
            device: false,
        };
        let first_page = segment.vaddr & !0xfff;
        let pages = segment.page_count();
        let file_end = segment.vaddr + segment.file_size as u64;
        for index in 0..pages {
            let va = first_page + (index as u64) * 4096;
            let frame = match m.retype(untyped, ObjKind::Frame, 12) {
                Ok(frame) => frame,
                Err(e) => bail!(init_pages, LoadError::Obj(e)),
            };
            let pa = match m.frame_pa(frame) {
                Ok(pa) => pa,
                Err(e) => bail!(init_pages, LoadError::Obj(e)),
            };
            // Phần file rơi vào trang này (phần còn lại zero-fill).
            let page_end = va + 4096;
            let copy_start = va.max(segment.vaddr);
            let copy_end = page_end.min(file_end);
            let (file_start, copy_len) = if copy_end > copy_start {
                let offset = segment.file_offset as u64 + (copy_start - segment.vaddr);
                let len = copy_end - copy_start;
                let start = usize::try_from(offset).map_err(|_| LoadError::Image)?;
                let end = usize::try_from(offset + len).map_err(|_| LoadError::Image)?;
                if end > bytes.len() {
                    bail!(init_pages, LoadError::Image);
                }
                (start, len as usize)
            } else {
                (0, 0)
            };
            let data = &bytes[file_start..file_start + copy_len];
            if !write_page(pa, data) {
                bail!(init_pages, LoadError::Physmap);
            }
            if let Err(e) = m.map(vspace, va, frame, flags) {
                bail!(init_pages, LoadError::Obj(e));
            }
            if !init_pages.push(va, frame) {
                bail!(init_pages, LoadError::Image);
            }
            if index == 0 && !loading_ranges().push(va, va + (pages as u64) * 4096) {
                bail!(init_pages, LoadError::Image);
            }
            if segment.executable() {
                text_pages += 1;
            } else if segment.writable() {
                data_pages += 1;
            } else {
                ro_pages += 1;
            }
        }
    }

    // Stack user: `stack_pages` trang RW NX; trang ngay dưới (guard) **không map**.
    let stack_pages =
        match crate::init_pages::map_user_stack(m, vspace, untyped, stack_pages, &mut init_pages) {
            Ok(pages) => pages,
            Err(e) => bail!(init_pages, e),
        };

    // **KT-67b** — trang **IPC buffer** (một Frame 4 KiB, `R|W|U|NX`, zero):
    // `spec/abi.toml` dòng 58 chốt header IPC buffer chứa receive window (CPTR, ≤4 cap); shim
    // U-mode + kernel đọc/ghi cùng VA cố định này. Nằm trong cùng bảng cấp 1 với ảnh/stack
    // (0x4002_0000 ⇒ level-1 index 0) nên **không** cần bảng trang mới.
    let ipc_frame = match m.retype(untyped, ObjKind::Frame, 12) {
        Ok(frame) => frame,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    let ipc_pa = match m.frame_pa(ipc_frame) {
        Ok(pa) => pa,
        Err(e) => bail!(init_pages, LoadError::Obj(e)),
    };
    if !write_page(ipc_pa, &[]) {
        bail!(init_pages, LoadError::Physmap);
    }
    let ipc_va = crate::ipc_buffer::USER_IPC_BUFFER_VA;
    if let Err(e) = m.map(vspace, ipc_va, ipc_frame, MapFlags::RW_NX_USER) {
        bail!(init_pages, LoadError::Obj(e));
    }
    if !init_pages.push(ipc_va, ipc_frame) {
        bail!(init_pages, LoadError::Image);
    }
    if !loading_ranges().push(ipc_va, ipc_va + 4096) {
        bail!(init_pages, LoadError::Image);
    }

    // Đi lại PTE thật để chứng minh quyền leaf (không tin cờ lúc map): `W+X` = 0, text có X,
    // data có W, mọi leaf user có `U`; đây là bằng chứng `# init load: … wx=0 flags=1`.
    let mut wx_pages = 0usize;
    let mut flags_ok = true;
    for segment in image.segments() {
        let mut va = segment.vaddr & !0xfff;
        for _ in 0..segment.page_count() {
            let walk = match m.walk(vspace, va) {
                Ok(walk) => walk,
                Err(e) => bail!(init_pages, LoadError::Obj(e)),
            };
            let raw = walk.raw;
            let read = raw & PTE_READ != 0;
            let write = raw & PTE_WRITE != 0;
            let exec = raw & PTE_EXEC != 0;
            let user = raw & PTE_USER != 0;
            if write && exec {
                wx_pages += 1;
            }
            let shape_ok = walk.valid && walk.leaf && walk.level == 0;
            let expected = shape_ok
                && read
                && user
                && write == segment.writable()
                && exec == segment.executable();
            if !expected {
                flags_ok = false;
            }
            va += 4096;
        }
    }

    // Nửa cao phải **không** map: walk high alias kernel + physmap đều invalid.
    let high_alias_off = m
        .walk(vspace, mmu::KERNEL_HIGH_BASE)
        .map(|walk| !walk.valid)
        .unwrap_or(false);
    let physmap_off = m
        .walk(vspace, mmu::PHYSMAP_BASE + mmu::RAM_PHYS_BASE)
        .map(|walk| !walk.valid)
        .unwrap_or(false);
    let high_half_unmapped = high_alias_off && physmap_off;
    let guard_unmapped = m
        .walk(vspace, USER_STACK_GUARD_BOTTOM)
        .map(|walk| !walk.valid)
        .unwrap_or(false);

    let flush = m.drain_flushes(aobj::run_flush);
    let check_ok = m.check_objmem().is_ok();
    let _ = flush;
    Ok(LoadedInit {
        vspace,
        root,
        asid,
        entry: image.entry,
        segments: image.segment_count(),
        text_pages,
        ro_pages,
        data_pages,
        stack_pages,
        image_end,
        stack_guard: USER_STACK_GUARD_BOTTOM,
        check_ok,
        wx_pages,
        flags_ok,
        high_half_unmapped,
        guard_unmapped,
        pages: init_pages,
    })
}

/// Ghi một trang 4 KiB: zero toàn trang rồi chép `data` (≤ 4096) vào đầu trang.
///
/// # Safety
/// Logic: `pa` do object layer cấp trong cửa sổ PA của Untyped, `mmu::physmap_alias` chỉ trả
/// `Some` khi PA nằm trong RAM 128 MiB đã map — nên 4 KiB ghi là hợp lệ.
pub(crate) fn write_page(pa: u64, data: &[u8]) -> bool {
    if data.len() > 4096 {
        return false;
    }
    let Some(va) = mmu::physmap_alias(pa) else {
        return false;
    };
    // SAFETY: `va` là alias physmap của RAM thật (xem trên); ghi 4 KiB + chép `data`.
    unsafe {
        core::ptr::write_bytes(va as *mut u8, 0, 4096);
        if !data.is_empty() {
            core::ptr::copy_nonoverlapping(data.as_ptr(), va as *mut u8, data.len());
        }
    }
    true
}
