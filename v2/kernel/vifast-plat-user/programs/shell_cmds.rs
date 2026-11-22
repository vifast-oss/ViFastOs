// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-343** — 8 lệnh `deferred-K7` của shell sản phẩm (`mkdir`/`rm`/`mv`/`ln`/`fsck` +
//! `uname`/`history`/`!!`) và 2 bí danh `runfs`/`exec` (trong `shell.rs`).
//!
//! Vì sao tách tệp: `shell.rs` đã chạm trần R1 (1.500 dòng); các lệnh KT-343 là **một nhóm**
//! (điều khiển FS + lịch sử một dòng) nên gom về đây thay vì nhồi tiếp. Tệp này chỉ được biên
//! dịch khi `--cfg shell_fs` (build.rs của ảnh sản phẩm riscv64); ảnh test/aarch64 không thấy.
//! Không `unsafe`, không `alloc`; mọi lỗi FS in `loi code=<n>` theo cùng bảng mã với `shell.rs`.

#![allow(dead_code)]

use super::{
    fs_call, fs_set_path, fs_status, no_fs, pack_bytes, put, put_u64, run_line, Cwd, FS_BYTES,
    MAX_LINE, ST_OK,
};
use vifast_user_abi::ARCH_NAME;

/// **KT-343** — 5 thao tác FS của 11 lệnh `deferred-K7` (khớp `proc_abi::OP_*`) + đường dẫn
/// thứ hai cho `mv`/`ln` (cùng bố cục 24 B/lần như `OP_PATH`).
#[cfg(shell_fs)]
const OP_MKDIR: u64 = 0x17;
#[cfg(shell_fs)]
const OP_RM: u64 = 0x18;
#[cfg(shell_fs)]
const OP_MV: u64 = 0x19;
#[cfg(shell_fs)]
const OP_LN: u64 = 0x1a;
#[cfg(shell_fs)]
const OP_FSCK: u64 = 0x1b;
#[cfg(shell_fs)]
const OP_PATH2: u64 = 0x1c;
/// **KT-343** — lỗi của một thao tác FS (mã ở byte 1 của word 0).
#[cfg(shell_fs)]
const ST_ERR: u64 = 2;

/// **KT-343** — nạp **đường dẫn thứ hai** (đích của `mv`/`ln`) vào kernel bằng `OP_PATH2`
/// (cùng bố cục 24 B/lần như [`fs_set_path`]). `Err` = không có dịch vụ FS.
#[cfg(shell_fs)]
pub(super) fn fs_set_path2(path: &[u8]) -> Result<(), ()> {
    match fs_call([OP_PATH2, 0, 0, 0]) {
        Some(reply) if fs_status(reply) == Some(ST_OK) => {}
        _ => return Err(()),
    }
    let mut offset = 0usize;
    while offset < path.len() {
        let count = core::cmp::min(FS_BYTES, path.len() - offset);
        let words = pack_bytes(&path[offset..offset + count]);
        let head = OP_PATH2 | ((count as u64) << 40);
        match fs_call([head, words[0], words[1], words[2]]) {
            Some(reply) if fs_status(reply) == Some(ST_OK) => {}
            Some(_) => return Err(()),
            None => return Err(()),
        }
        offset += count;
    }
    Ok(())
}

/// **KT-343** — in dòng lỗi **có tên lệnh** (không dùng `fs_status` chung): nhờ vậy một ca kiểm
/// `grep` theo dòng không khớp nhầm lỗi `code=` của lệnh trước, và log đọc được ngay lệnh nào hỏng.
/// Trả `true` ⇔ reply là lỗi `ST_ERR` (đã in).
#[cfg(shell_fs)]
fn fs_err_verb(verb: &[u8], reply: [u64; 4]) -> bool {
    if reply[0] & 0xff != ST_ERR {
        return false;
    }
    put(b"# shell: ");
    put(verb);
    put(b" loi code=");
    put_u64((reply[0] >> 8) & 0xff);
    put(b"\n");
    true
}

/// **KT-343** `mkdir <path>` — `OP_MKDIR` (`Fs::mkdir`). In `# shell: mkdir path=<p> ok`; lỗi FS
/// in `# shell: mkdir path=<p> loi code=<n>` (cùng bảng mã với mọi thao tác FS khác).
#[cfg(shell_fs)]
pub(super) fn cmd_mkdir(path: &[u8]) {
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    match fs_call([OP_MKDIR, 0, 0, 0]) {
        Some(reply) => {
            if reply[0] & 0xff == ST_OK {
                put(b"# shell: mkdir path=");
                put(path);
                put(b" ok\n");
            } else if reply[0] & 0xff == ST_ERR {
                put(b"# shell: mkdir path=");
                put(path);
                put(b" loi code=");
                put_u64((reply[0] >> 8) & 0xff);
                put(b"\n");
            } else {
                no_fs();
            }
        }
        None => no_fs(),
    }
}

/// **KT-343** `rm <path>` — `OP_RM` (`Fs::unlink`; thư mục phải rỗng).
#[cfg(shell_fs)]
pub(super) fn cmd_rm(path: &[u8]) {
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    match fs_call([OP_RM, 0, 0, 0]) {
        Some(reply) => {
            if reply[0] & 0xff == ST_OK {
                put(b"# shell: rm path=");
                put(path);
                put(b" ok\n");
            } else if reply[0] & 0xff == ST_ERR {
                put(b"# shell: rm path=");
                put(path);
                put(b" loi code=");
                put_u64((reply[0] >> 8) & 0xff);
                put(b"\n");
            } else {
                no_fs();
            }
        }
        None => no_fs(),
    }
}

/// **KT-343** `mv <src> <dst>` — `OP_MV` (`Fs::rename`, đích tồn tại thì thay thế). Nguồn ở
/// `path`, đích ở `path2`.
#[cfg(shell_fs)]
pub(super) fn cmd_mv(src: &[u8], dst: &[u8]) {
    if fs_set_path(src).is_err() || fs_set_path2(dst).is_err() {
        no_fs();
        return;
    }
    match fs_call([OP_MV, 0, 0, 0]) {
        Some(reply) => {
            if reply[0] & 0xff == ST_OK {
                put(b"# shell: mv ");
                put(src);
                put(b" -> ");
                put(dst);
                put(b" ok\n");
            } else if reply[0] & 0xff == ST_ERR {
                put(b"# shell: mv ");
                put(src);
                put(b" -> ");
                put(dst);
                put(b" loi code=");
                put_u64((reply[0] >> 8) & 0xff);
                put(b"\n");
            } else {
                no_fs();
            }
        }
        None => no_fs(),
    }
}

/// **KT-343** `ln <src> <dst>` — `OP_LN` (`Fs::link`, liên kết cứng).
#[cfg(shell_fs)]
pub(super) fn cmd_ln(src: &[u8], dst: &[u8]) {
    if fs_set_path(src).is_err() || fs_set_path2(dst).is_err() {
        no_fs();
        return;
    }
    match fs_call([OP_LN, 0, 0, 0]) {
        Some(reply) => {
            if reply[0] & 0xff == ST_OK {
                put(b"# shell: ln ");
                put(src);
                put(b" -> ");
                put(dst);
                put(b" ok\n");
            } else if reply[0] & 0xff == ST_ERR {
                put(b"# shell: ln ");
                put(src);
                put(b" -> ");
                put(dst);
                put(b" loi code=");
                put_u64((reply[0] >> 8) & 0xff);
                put(b"\n");
            } else {
                no_fs();
            }
        }
        None => no_fs(),
    }
}

/// **KT-343** `fsck` — `OP_FSCK` (`Fs::fsck` kiểm superblock/inode/block/liên kết thư mục).
/// In `# shell: fsck ok`; hỏng ⇒ `# shell: fsck loi code=14` (Corrupt).
#[cfg(shell_fs)]
pub(super) fn cmd_fsck() {
    match fs_call([OP_FSCK, 0, 0, 0]) {
        Some(reply) => {
            if reply[0] & 0xff == ST_OK {
                put(b"# shell: fsck ok\n");
            } else if !fs_err_verb(b"fsck", reply) {
                no_fs();
            }
        }
        None => no_fs(),
    }
}

/// **KT-343** `uname` — in `ViFastOs <arch> bootinfo-v3` (v1 in đúng dạng này). Tên arch lấy từ
/// shim [`vifast_user_abi::ARCH_NAME`] — shell dùng chung nguồn cho cả 3 kiến trúc.
#[cfg(shell_fs)]
pub(super) fn cmd_uname() {
    put(b"# shell: uname ViFastOs ");
    put(ARCH_NAME);
    put(b" bootinfo-v3\n");
}

/// **KT-343** `history` — in lệnh **trước** (`last=`); shell chỉ giữ **một** dòng như v1 (v1 in
/// `last: <lệnh>`), không cấp phát danh sách.
#[cfg(shell_fs)]
pub(super) fn cmd_history(last: &[u8]) {
    put(b"# shell: history last=");
    put(last);
    put(b"\n");
}

/// **KT-343** `!!` — chạy lại lệnh trước. Không bao giờ đệ quy vô hạn: `_start` **không** ghi
/// `!!`/`history` vào lịch sử, nên `last` luôn là một lệnh thật.
#[cfg(shell_fs)]
pub(super) fn cmd_bang(last: &mut [u8; MAX_LINE], last_len: &mut usize, cwd: &mut Cwd) {
    let len = *last_len;
    if len == 0 {
        put(b"# shell: !! none\n");
        return;
    }
    // Sao chép ra stack trước khi mượn lại `last` cho lệnh chạy lại (không giữ hai borrow).
    let mut copy = [0u8; MAX_LINE];
    copy[..len].copy_from_slice(&last[..len]);
    put(b"# shell: !! ");
    put(&copy[..len]);
    put(b"\n");
    run_line(&copy[..len], cwd, last, last_len);
}
