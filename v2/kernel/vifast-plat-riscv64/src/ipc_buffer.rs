// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-67b** — đọc **header IPC buffer** của U-mode từ bộ nhớ user (D2-98).
//!
//! `spec/abi.toml` dòng 58: *"header chứa receive window (CPTR, ≤4 cap)"*. Arch là nơi **duy
//! nhất** chạm bộ nhớ user (D2-83); core chỉ nhận [`vifast_core::ipc::IpcHeader`] qua hook
//! `SyscallEnv::ipc_header`. Trang IPC buffer (4 KiB tại [`USER_IPC_BUFFER_VA`], `R|W|U|NX`) do
//! loader map cho **mọi** ELF U-mode; shim U-mode (`kernel/vifast-plat-user/lib.rs`) ghi magic
//! `b"KT67b"` + receive window 4 CPTR + 4 CPTR nguồn cap (layout word ở `vifast_core::ipc`).
//!
//! Tách khỏi `user.rs` để giữ luật cấu trúc R1 (≤ 1.500 dòng/tệp). `unsafe` thuộc danh sách đóng
//! `kernel/vifast-plat-*` (D2-17): chỉ một vòng đọc 10 word qword trong trang đã đăng ký.

use core::sync::atomic::{AtomicU32, Ordering};

use vifast_core::ipc::{
    IpcExtraCaps, IpcHeader, IPC_BUFFER_EXTRA_CAPS_OFFSET, IPC_BUFFER_EXTRA_CAPS_READ_WORDS,
    IPC_BUFFER_MR_OFFSET, IPC_HEADER_MAGIC, IPC_HEADER_WORDS, IPC_HEADER_W_INFO,
    IPC_HEADER_W_SEND_CAPS, IPC_HEADER_W_WINDOW, WORDS_IN_IPC_BUFFER, WORDS_IN_REGISTERS,
};

use crate::obj as aobj;
use crate::user::user_range_mapped;

/// VA cố định của trang **IPC buffer** — loader map `R|W|U|NX` + zero cho **mọi** ELF U-mode;
/// shim U-mode (`kernel/vifast-plat-user/lib.rs`) ghi/đọc cùng địa chỉ.
pub const USER_IPC_BUFFER_VA: u64 = 0x4002_0000;

/// Đọc header IPC buffer của tiến trình **đang chạy** — chỉ gọi trong syscall U-mode (`satp` =
/// root user, trang đã đăng ký ở `active_ranges`). `None` = trang chưa map hoặc magic chưa ghi
/// (ảnh cũ / chương trình không dùng header ⇒ kernel giữ nguyên hành vi trước KT-67b).
pub(crate) fn read_header() -> Option<IpcHeader> {
    let bytes = (IPC_HEADER_WORDS as u64) * 8;
    if !user_range_mapped(USER_IPC_BUFFER_VA, bytes) {
        return None;
    }
    let mut words = [0u64; IPC_HEADER_WORDS];
    aobj::set_sum(true);
    // SAFETY: dải `[USER_IPC_BUFFER_VA, +80)` đã được loader đăng ký là trang `U|R|W` của tiến
    // trình đang chạy; SUM bật nên S-mode đọc được trang U; tắt SUM ngay sau vòng.
    unsafe {
        for (index, slot) in words.iter_mut().enumerate() {
            *slot =
                core::ptr::read_volatile((USER_IPC_BUFFER_VA + (index as u64) * 8) as *const u64);
        }
    }
    aobj::set_sum(false);
    if words[0] != IPC_HEADER_MAGIC {
        return None;
    }
    // **KT-309** — vùng **extra caps** ở offset 560 (**cùng** trang 4 KiB, cùng magic): `n` +
    // 4 CPTR nguồn + `dst`. Dải `[560, 608)` nằm trong trang đã kiểm ở trên ⇒ đọc an toàn.
    let mut extra_words = [0u64; IPC_BUFFER_EXTRA_CAPS_READ_WORDS];
    aobj::set_sum(true);
    // SAFETY: `[USER_IPC_BUFFER_VA+560, +608)` nằm trong **cùng** trang `U|R|W` đã kiểm bằng
    // `user_range_mapped` ở đầu hàm; SUM bật nên S-mode đọc được trang U; tắt ngay sau vòng.
    unsafe {
        for (index, slot) in extra_words.iter_mut().enumerate() {
            *slot = core::ptr::read_volatile(
                (USER_IPC_BUFFER_VA + IPC_BUFFER_EXTRA_CAPS_OFFSET as u64 + (index as u64) * 8)
                    as *const u64,
            );
        }
    }
    aobj::set_sum(false);
    Some(IpcHeader {
        info: words[IPC_HEADER_W_INFO],
        recv_window: [
            words[IPC_HEADER_W_WINDOW],
            words[IPC_HEADER_W_WINDOW + 1],
            words[IPC_HEADER_W_WINDOW + 2],
            words[IPC_HEADER_W_WINDOW + 3],
        ],
        send_caps: [
            words[IPC_HEADER_W_SEND_CAPS],
            words[IPC_HEADER_W_SEND_CAPS + 1],
            words[IPC_HEADER_W_SEND_CAPS + 2],
            words[IPC_HEADER_W_SEND_CAPS + 3],
        ],
        extra: IpcExtraCaps::from_words(&extra_words),
    })
}

// ---------------------------------------------------------------------------------------------
// KT-282 — vùng **MR4..MR63** trong IPC buffer (frame 4 KiB bind qua `Tcb.Configure`)
// ---------------------------------------------------------------------------------------------

/// **KT-282** — đọc 60 word **MR4..MR63** của IPC buffer tại `va` (VA **đã bind** bằng
/// `Tcb.Configure` tham số `ipc_buffer?`), offset [`IPC_BUFFER_MR_OFFSET`] = 80 B (ngay sau
/// header 10 word — `[TK]` KT-282, xem `vifast_core::ipc`).
///
/// `None` ⇔ dải `[va+80, va+560)` **không** nằm trọn trong một dải đã map của tác vụ đang chạy
/// (buffer trỏ ra ngoài frame / trang chưa map) — fail-closed, không đọc bừa.
pub(crate) fn read_words(va: u64) -> Option<[u64; WORDS_IN_IPC_BUFFER]> {
    let start = va.checked_add(IPC_BUFFER_MR_OFFSET as u64)?;
    let bytes = (WORDS_IN_IPC_BUFFER as u64) * 8;
    if !user_range_mapped(start, bytes) {
        return None;
    }
    let mut words = [0u64; WORDS_IN_IPC_BUFFER];
    aobj::set_sum(true);
    // SAFETY: `[start, start+480)` đã kiểm nằm trong dải `U|R|W` đã map của tác vụ đang chạy;
    // SUM bật nên S-mode đọc được trang U; tắt ngay sau vòng (cùng khuôn `read_header`).
    unsafe {
        for (index, slot) in words.iter_mut().enumerate() {
            *slot = core::ptr::read_volatile((start + (index as u64) * 8) as *const u64);
        }
    }
    aobj::set_sum(false);
    Some(words)
}

/// **KT-282** — ghi `words` (MR4..) vào vùng MR của IPC buffer tại `va`. Trả `false` ⇔ dải
/// không nằm trọn trong trang đã map (không ghi bừa).
pub(crate) fn write_words(va: u64, words: &[u64]) -> bool {
    let Some(start) = va.checked_add(IPC_BUFFER_MR_OFFSET as u64) else {
        return false;
    };
    let bytes = (words.len() as u64) * 8;
    if !user_range_mapped(start, bytes) {
        return false;
    }
    aobj::set_sum(true);
    // SAFETY: như [`read_words`] — dải đã kiểm nằm trong trang `U|R|W` của tác vụ đang chạy.
    unsafe {
        for (index, word) in words.iter().enumerate() {
            core::ptr::write_volatile((start + (index as u64) * 8) as *mut u64, *word);
        }
    }
    aobj::set_sum(false);
    true
}

/// **KT-282** — ghi **MR4..MR63** của message **vừa nhận** vào IPC buffer của TCB `tcb`.
///
/// Nguồn là `Machine::tcb_ipc_area` (core sở hữu message — D2-83); arch chỉ chép ra bộ nhớ user
/// tại **VA đã bind**. Trả `false` ⇔ TCB chưa bind buffer, chưa nhận message, message ≤ 4 MR
/// (không có gì để chép), hoặc trang chưa map — **không** phải lỗi syscall.
pub(crate) fn apply_received(tcb: vifast_core::ObjId) -> bool {
    let Some(binding) =
        crate::core_bridge::with_machine(|m| m.tcb_ipc_buffer_binding(tcb).ok().flatten())
    else {
        return false;
    };
    let Some(area) = crate::core_bridge::with_machine(|m| m.tcb_ipc_area(tcb).ok()) else {
        return false;
    };
    let Some(info) = area.info else {
        return false;
    };
    let len = info.length() as usize;
    if len <= WORDS_IN_REGISTERS {
        return false;
    }
    write_words(binding.va, &area.buffer[..len - WORDS_IN_REGISTERS])
}

/// **KT-282** — TCB đang chờ kernel ghi **MR4..MR63** vào IPC buffer ở lần vào U-mode **kế tiếp**
/// (`u32::MAX` = không có). Vì sao cần: `apply_ipc_return` chạy **sau** khi `enter_loaded` đã trả
/// `satp` về **root kernel** ⇒ không chạm được VA user; `enter_loaded` là chỗ **duy nhất** chuyển
/// sang root user trước `sret`.
static PENDING_IPC_BUFFER: AtomicU32 = AtomicU32::new(u32::MAX);

/// **KT-282** — đánh dấu TCB `tcb` cần ghi MR4..MR63 vào IPC buffer ở lần vào U-mode kế tiếp.
pub(crate) fn request_write(tcb: vifast_core::ObjId) {
    PENDING_IPC_BUFFER.store(tcb, Ordering::Release);
}

/// **KT-282** — ghi MR4..MR63 của message vừa nhận vào IPC buffer **ngay** — chỉ gọi khi `satp`
/// đang trỏ **root user** (đường không chặn, trong handler syscall).
pub(crate) fn apply_received_now(tcb: vifast_core::ObjId) {
    let _ = apply_received(tcb);
}

/// **KT-282** — tiêu cờ [`request_write`] rồi ghi buffer; gọi từ `enter_loaded` **sau**
/// `switch_root` (lúc đó `satp` = root user).
pub(crate) fn flush_pending() {
    let pending = PENDING_IPC_BUFFER.swap(u32::MAX, Ordering::AcqRel);
    if pending != u32::MAX {
        let _ = apply_received(pending);
    }
}
