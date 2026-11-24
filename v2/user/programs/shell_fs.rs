// SPDX-License-Identifier: MIT OR Apache-2.0
//! `shell_fs.rs` — nhánh **FS của shell** (`#[cfg(shell_fs)]`, KT-77c/KT-114/KT-137/KT-343):
//! `fs_dispatch` + `cd`/`pwd` + `ls`/`cat`/`write`/`truncate`/`time`/`uptime`/`sync`/`stat`,
//! `fs_call`/`fs_set_path`/`fs_status`/`no_fs`/`pack_bytes` qua dịch vụ FS của kernel (`EP_FS`,
//! `FS_REQ`/`FS_REPLY`). Tách cơ học khỏi `shell.rs` (KT-401, mỗi tệp < 950 dòng); không mở
//! ABI, không đổi giao thức — mọi lời gọi vẫn qua `NBSend`/`NBRecv` đã có.

#![allow(unused_imports)] // glob lấy helper/cfg từ tệp cha — không phải mọi mục đều dùng.

use super::*;

/// `truncate <path> <size>` — **KT-125**: đổi cỡ tệp về `size` byte qua `OP_TRUNC` (opcode **đã
/// có**, `write` đang dùng). Không ghi dữ liệu, chỉ đổi cỡ.
#[cfg(shell_fs)]
fn cmd_truncate(path: &[u8], size: u64) {
    // **KT-126** (review chéo): `head` chỉ dành **ba mươi hai** bit cho tham số (`arg = (head >> 8)
    // & 0xffff_ffff` ở kernel), nên `size >= 2^32` bị **cắt âm thầm** — đo được: `truncate /README
    // 4294967296` báo thành công nhưng tệp về **không** byte. Nay từ chối sớm thay vì nói dối.
    if size > 0xFFFF_FFFF {
        put(b"# shell: kich thuoc qua lon (toi da 4294967295)\n");
        return;
    }
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    let head = OP_TRUNC | (size << 8);
    match fs_call([head, 0, 0, 0]) {
        Some(reply) => {
            if fs_status(reply).is_none() {
                return;
            }
            put(b"# shell: truncate path=");
            put(path);
            put(b" size=");
            put_u64(size);
            put(b"\n");
        }
        None => no_fs(),
    }
}

// ---------------------------------------------------------------------------------------------
// **KT-77c** — lệnh FS (`ls`/`cat`/`write`) qua dịch vụ FS **của kernel** (`EP_FS`).
// **KT-114** thêm `time`/`uptime` (`ClockRead` 11) và `sync`/`stat` (`OP_SYNC`/`OP_STAT`) vào
// **cùng** nhánh `#[cfg(shell_fs)]` — ảnh test (không FS) không thấy bốn lệnh này.
//
// Mỗi request là **một** reply 4 MR word (bố cục ở `src/proc_fs.rs`): tên/dữ liệu đi trong MR
// (24 B/lần) chứ không phải Frame chia sẻ. Mọi lời gọi đều có **hạn** (`FS_REPLY_BUDGET` lượt
// `Yield`): ảnh build **không** `product-fs` (ca âm tiêu chí 5c) hoặc QEMU không có đĩa và không
// dựng được volume RAM ⇒ in `fs khong san sang` rồi **trả prompt**, không treo.
// ---------------------------------------------------------------------------------------------
/// `cd [path]` — **KT-137**. Không tham số ⇒ về `/` (ghim bằng ca kiểm `kt137-cdpwd`).
/// Đường **không có** hoặc **không phải thư mục** ⇒ báo lỗi và **KHÔNG** đổi thư mục hiện tại
/// (kiểm bằng `pwd` ngay sau đó trong ca kiểm).
#[cfg(shell_fs)]
fn cmd_cd(cwd: &mut Cwd, arg: Option<&[u8]>) {
    let mut out = [0u8; CWD_MAX];
    let len = match arg {
        None => {
            out[0] = b'/';
            1
        }
        Some(a) => match resolve_path(cwd, a, &mut out) {
            Ok(len) => len,
            Err(err) => {
                path_err(err);
                return;
            }
        },
    };
    if fs_set_path(&out[..len]).is_err() {
        no_fs();
        return;
    }
    let Some(reply) = fs_call([OP_STAT, 0, 0, 0]) else {
        no_fs();
        return;
    };
    let status = reply[0] & 0xff;
    if status != ST_OK {
        if status == ST_NOFS {
            no_fs();
        } else if (reply[0] >> 8) & 0xff == 5 {
            // 5 = `FsError::NotFound` (`proc_fs::err_code`) — thông điệp đọc được, không chỉ mã.
            put(b"# shell: cd khong co ");
            put(&out[..len]);
            put(b"\n");
        } else {
            // Mã lỗi FS khác (11 = NameTooLong, 7 = NotDir, …) — in kèm để không nói mơ hồ.
            put(b"# shell: cd loi code=");
            put_u64((reply[0] >> 8) & 0xff);
            put(b" ");
            put(&out[..len]);
            put(b"\n");
        }
        return;
    }
    let kind = (reply[0] >> 8) & 0xff;
    if kind != KIND_DIR {
        put(b"# shell: cd khong phai thu muc ");
        put(&out[..len]);
        put(b"\n");
        return;
    }
    cwd.set(&out[..len]);
    put(b"# shell: cd ");
    put(&out[..len]);
    put(b"\n");
}

/// `pwd` — **KT-137**: in thư mục hiện tại (đã chuẩn hoá) sau `# shell: pwd `.
#[cfg(shell_fs)]
fn cmd_pwd(cwd: &Cwd) {
    put(b"# shell: pwd ");
    put(cwd.as_slice());
    put(b"\n");
}

/// `true` ⇔ `line` là lệnh của nhánh `shell_fs` (`ls`/`cat`/`write` + **KT-114**
/// `time`/`uptime`/`sync`/`stat` + **KT-137** `cd`/`pwd`) và đã được thi hành.
#[cfg(shell_fs)]
pub(super) fn fs_dispatch(
    line: &[u8],
    cwd: &mut Cwd,
    last: &mut [u8; MAX_LINE],
    last_len: &mut usize,
) -> bool {
    if line == b"ls" {
        // **KT-137**: `ls` **không tham số** liệt kê **thư mục hiện tại** (trước: luôn `/`).
        cmd_ls(cwd.as_slice());
        true
    } else if starts_with(line, b"ls ") {
        if let Some((path, len)) = resolve_arg(cwd, &line[3..]) {
            cmd_ls(&path[..len]);
        }
        true
    } else if line == b"cd" {
        // **KT-137**: `cd` không tham số ⇒ về `/` (chọn một hành vi và ghim bằng ca kiểm).
        cmd_cd(cwd, None);
        true
    } else if starts_with(line, b"cd ") {
        cmd_cd(cwd, Some(&line[3..]));
        true
    } else if line == b"pwd" {
        cmd_pwd(cwd);
        true
    } else if line == b"cat" {
        // KT-104: `cat`/`write` **không tham số** trước đây rơi xuống nhánh "lệnh lạ" ⇒ in
        // `# shell: ?`, gây hiểu nhầm là shell không biết lệnh. Nay báo đúng: thiếu đường dẫn.
        put(b"# shell: thieu duong dan\n");
        true
    } else if starts_with(line, b"cat ") {
        if let Some((path, len)) = resolve_arg(cwd, &line[4..]) {
            cmd_cat(&path[..len]);
        }
        true
    } else if line == b"jobs" {
        cmd_jobs();
        true
    } else if line == b"truncate" {
        put(b"# shell: thieu duong dan\n");
        true
    } else if starts_with(line, b"truncate ") {
        let (path, size) = split_space(&line[9..]);
        if path.is_empty() {
            put(b"# shell: thieu duong dan\n");
        } else if let Some(size) = parse_u64(size) {
            if let Some((resolved, len)) = resolve_arg(cwd, path) {
                cmd_truncate(&resolved[..len], size);
            }
        } else {
            put(b"# shell: thieu kich thuoc\n");
        }
        true
    } else if line == b"write" {
        put(b"# shell: thieu duong dan\n");
        true
    } else if starts_with(line, b"write ") {
        let (path, text) = split_space(&line[6..]);
        if path.is_empty() {
            put(b"# shell: thieu duong dan\n");
        } else if text.is_empty() {
            // **K7 (Agent 0, phiếu lượt bốn câu 1)**: `write <path>` **thiếu nội dung** ⇒ LỖI, tệp
            // **KHÔNG** bị đổi. Trước đây nhánh này cắt tệp về rỗng (`TRUNC 0`) — cắt ngầm khi gõ
            // thiếu là rủi ro mất dữ liệu, đúng nhóm lỗi K7 ưu tiên. Muốn cắt rỗng thì dùng
            // `truncate <path> 0`, tường minh.
            put(b"# shell: thieu noi dung\n");
        } else if let Some((resolved, len)) = resolve_arg(cwd, path) {
            cmd_write(&resolved[..len], text);
        }
        true
    } else if line == b"time" {
        cmd_time();
        true
    } else if line == b"uptime" {
        cmd_uptime();
        true
    } else if line == b"sync" {
        cmd_sync();
        true
    } else if line == b"stat" {
        // Như `cat`/`write` (KT-104): thiếu tham số ⇒ báo đúng, không rơi xuống "lệnh lạ".
        put(b"# shell: thieu duong dan\n");
        true
    } else if starts_with(line, b"stat ") {
        if let Some((path, len)) = resolve_arg(cwd, &line[5..]) {
            cmd_stat(&path[..len]);
        }
        true
    } else if line == b"mkdir" || line == b"rm" || line == b"mv" || line == b"ln" {
        put(b"# shell: thieu duong dan\n");
        true
    } else if starts_with(line, b"mkdir ") {
        // **KT-343**: một đường dẫn.
        if let Some((path, len)) = resolve_arg(cwd, &line[6..]) {
            cmd_mkdir(&path[..len]);
        }
        true
    } else if starts_with(line, b"rm ") {
        if let Some((path, len)) = resolve_arg(cwd, &line[3..]) {
            cmd_rm(&path[..len]);
        }
        true
    } else if starts_with(line, b"mv ") || starts_with(line, b"ln ") {
        // **KT-343**: hai đường dẫn (nguồn, đích) — tách theo dấu cách đầu tiên; `mv`/`ln` dùng
        // **cùng** một nhánh phân tích, khác đúng opcode gửi xuống kernel.
        let (src, dst) = split_space(&line[3..]);
        if src.is_empty() || dst.is_empty() {
            put(b"# shell: thieu duong dan\n");
        } else if let (Some((src_path, src_len)), Some((dst_path, dst_len))) =
            (resolve_arg(cwd, src), resolve_arg(cwd, dst))
        {
            if line[0] == b'm' {
                cmd_mv(&src_path[..src_len], &dst_path[..dst_len]);
            } else {
                cmd_ln(&src_path[..src_len], &dst_path[..dst_len]);
            }
        }
        true
    } else if line == b"fsck" {
        // **KT-343**: không tham số.
        cmd_fsck();
        true
    } else if line == b"uname" {
        cmd_uname();
        true
    } else if line == b"history" {
        cmd_history(&last[..*last_len]);
        true
    } else if line == b"!!" {
        cmd_bang(last, last_len, cwd);
        true
    } else {
        // **KT-345b** — `free`/`df`/`loop`: thân lệnh ở `shell_cmds.rs` (R1), đây chỉ là dòng nối.
        extra_dispatch(line)
    }
}

/// Đóng gói tối đa [`FS_BYTES`] byte vào 3 MR word (little-endian từng byte).
#[cfg(shell_fs)]
pub(super) fn pack_bytes(bytes: &[u8]) -> [u64; 3] {
    let mut words = [0u64; 3];
    let mut index = 0usize;
    while index < bytes.len() && index < FS_BYTES {
        words[index / 8] |= u64::from(bytes[index]) << ((index % 8) * 8);
        index += 1;
    }
    words
}

/// Giải nén `len` byte từ 3 MR word.
#[cfg(shell_fs)]
fn unpack_bytes(words: [u64; 3], out: &mut [u8; FS_BYTES], len: usize) {
    let len = if len > FS_BYTES { FS_BYTES } else { len };
    let mut index = 0usize;
    while index < len {
        out[index] = (words[index / 8] >> ((index % 8) * 8)) as u8;
        index += 1;
    }
}

/// Gửi **một** request FS và chờ reply có hạn. `None` = không có dịch vụ FS trong ảnh này.
#[cfg(shell_fs)]
pub(super) fn fs_call(words: [u64; 4]) -> Option<[u64; 4]> {
    send(EP_FS, FS_REQ, words);
    let mut budget = FS_REPLY_BUDGET;
    while budget > 0 {
        let (err, info, mr) = recv(EP_REPLY);
        if err == 0 && info & 0xFFFFF == FS_REPLY {
            return Some([mr[0], mr[1], mr[2], mr[3]]);
        }
        budget -= 1;
        yield_cpu();
    }
    None
}

#[cfg(shell_fs)]
fn put_err(code: u64) {
    put(b"# shell: fs loi code=");
    put_u64(code);
    put(b"\n");
}

/// Status của reply: `ST_OK`/`ST_END`/`ST_ERR`; in lỗi (kèm mã) khi `ST_ERR`.
#[cfg(shell_fs)]
pub(super) fn fs_status(reply: [u64; 4]) -> Option<u64> {
    let status = reply[0] & 0xff;
    if status == ST_OK || status == ST_END {
        return Some(status);
    }
    // **KT-123**: `ST_NOFS` = dịch vụ FS không sẵn sàng ⇒ **không** in `fs loi code=…`; trả `None`
    // để caller gọi `no_fs()` in `fs khong san sang` (đúng nghĩa, và không lộ mã nội bộ).
    if status == ST_NOFS {
        return None;
    }
    put_err((reply[0] >> 8) & 0xff);
    None
}

/// Nạp đường dẫn vào kernel (reset rồi từng khối 24 B). `false` = không có dịch vụ FS.
#[cfg(shell_fs)]
pub(super) fn fs_set_path(path: &[u8]) -> Result<(), ()> {
    match fs_call([OP_PATH, 0, 0, 0]) {
        Some(reply) if fs_status(reply) == Some(ST_OK) => {}
        _ => return Err(()),
    }
    let mut offset = 0usize;
    while offset < path.len() {
        let count = core::cmp::min(FS_BYTES, path.len() - offset);
        let words = pack_bytes(&path[offset..offset + count]);
        let head = OP_PATH | ((count as u64) << 40);
        match fs_call([head, words[0], words[1], words[2]]) {
            Some(reply) if fs_status(reply) == Some(ST_OK) => {}
            Some(_) => return Err(()),
            None => return Err(()),
        }
        offset += count;
    }
    Ok(())
}

#[cfg(shell_fs)]
pub(super) fn no_fs() {
    put(b"# shell: fs khong san sang (image khong co product-fs hoac khong co phuong tien)\n");
}

/// `ls [path]` — in từng entry rồi dòng tổng kết `# shell: ls n=<n> path=<path>` (dòng neo của
/// `spec/v1-parity.toml`).
#[cfg(shell_fs)]
fn cmd_ls(path: &[u8]) {
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    let mut index = 0usize;
    let mut count = 0u64;
    loop {
        let head = OP_LS | ((index as u64) << 8);
        let Some(reply) = fs_call([head, 0, 0, 0]) else {
            no_fs();
            return;
        };
        match fs_status(reply) {
            Some(ST_END) => break,
            Some(_) => {}
            None => return,
        }
        let ino = reply[0] >> 24;
        if ino != 0 {
            let len = ((reply[0] >> 8) & 0xff) as usize;
            let kind = (reply[0] >> 16) & 0xff;
            let mut name = [0u8; FS_BYTES];
            unpack_bytes([reply[1], reply[2], reply[3]], &mut name, len);
            put(b"  ");
            put(match kind {
                2 => b"d ",
                3 => b"l ",
                _ => b"f ",
            });
            put(&name[..len]);
            put(b"\n");
            count += 1;
        }
        index += 1;
    }
    put(b"# shell: ls n=");
    put_u64(count);
    put(b" path=");
    put(path);
    put(b"\n");
    send(EP_EVENTS, EV_PS, [count, 0, 0, 0]);
}

/// `cat <path>` — in **nội dung thật** rồi `# shell: cat ok bytes=<n> first=<8 byte đầu>`.
#[cfg(shell_fs)]
fn cmd_cat(path: &[u8]) {
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    let mut offset = 0usize;
    let mut total = 0usize;
    let mut first = [0u8; 8];
    let mut first_len = 0usize;
    loop {
        let head = OP_CAT | ((offset as u64) << 8);
        let Some(reply) = fs_call([head, 0, 0, 0]) else {
            no_fs();
            return;
        };
        match fs_status(reply) {
            Some(ST_END) => break,
            Some(_) => {}
            None => return,
        }
        let count = ((reply[0] >> 8) & 0xff) as usize;
        let mut buf = [0u8; FS_BYTES];
        unpack_bytes([reply[1], reply[2], reply[3]], &mut buf, count);
        put(&buf[..count]);
        let mut index = 0usize;
        while index < count && first_len < first.len() {
            first[first_len] = buf[index];
            first_len += 1;
            index += 1;
        }
        total += count;
        offset += count;
        if count == 0 {
            break;
        }
    }
    put(b"\n# shell: cat ok bytes=");
    put_u64(total as u64);
    put(b" first=");
    put(&first[..first_len]);
    put(b"\n");
}

/// `write <path> <text>` — ghi theo khối 24 B, `TRUNC` về đúng cỡ rồi `SYNC` (fsync + flush) để
/// dữ liệu **bền qua reboot** trên cùng ảnh đĩa (ranh giới bền của KT-75, dùng lại nguyên).
#[cfg(shell_fs)]
fn cmd_write(path: &[u8], text: &[u8]) {
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    let mut offset = 0usize;
    while offset < text.len() {
        let count = core::cmp::min(FS_BYTES, text.len() - offset);
        let words = pack_bytes(&text[offset..offset + count]);
        let head = OP_WRITE | ((offset as u64) << 8) | ((count as u64) << 40);
        match fs_call([head, words[0], words[1], words[2]]) {
            Some(reply) => {
                if fs_status(reply).is_none() {
                    return;
                }
            }
            None => {
                no_fs();
                return;
            }
        }
        offset += count;
    }
    let trunc = OP_TRUNC | ((text.len() as u64) << 8);
    match fs_call([trunc, 0, 0, 0]) {
        Some(reply) => {
            if fs_status(reply).is_none() {
                return;
            }
        }
        None => {
            no_fs();
            return;
        }
    }
    let sync_ok = match fs_call([OP_SYNC, 0, 0, 0]) {
        Some(reply) => fs_status(reply) == Some(ST_OK),
        None => {
            no_fs();
            return;
        }
    };
    put(b"# shell: write ");
    put(path);
    put(b" bytes=");
    put_u64(text.len() as u64);
    put(b" sync=");
    put(if sync_ok { b"ok" } else { b"loi" });
    put(b"\n");
    send(EP_EVENTS, EV_ECHO, [text.len() as u64, 0, 0, 0]);
}

/// **KT-114** `time` — đọc đồng hồ **đơn điệu** của nền tảng bằng `ClockRead` (11) đã có trong
/// ABI (`vifast-abi::generated::SYS_CLOCK_READ`), in `# shell: time ns=<n>`.
/// Không dùng dịch vụ FS ⇒ **không** có hạn chờ nào để hết; syscall trả ngay.
#[cfg(shell_fs)]
fn cmd_time() {
    // Façade `syscall7` trả `[error, value, info, MR0..MR3]` ⇒ ns ở `[1]` (error 0).
    let r = syscall7(SYS_CLOCK_READ, 0, 0, 0, 0, 0, 0);
    put(b"# shell: time ns=");
    put_u64(r[1]);
    put(b"\n");
}

/// **KT-114** `uptime` — cùng nguồn `ClockRead` (ns), in mili-giây: `# shell: uptime ms=<n>`.
#[cfg(shell_fs)]
fn cmd_uptime() {
    let r = syscall7(SYS_CLOCK_READ, 0, 0, 0, 0, 0, 0);
    put(b"# shell: uptime ms=");
    put_u64(r[1] / 1_000_000);
    put(b"\n");
}

/// **KT-114** `sync` — `OP_SYNC` (`Fs::fsync` + `flush` thiết bị, ranh giới bền KT-75).
/// Cùng khuôn `cmd_write`: `fs_call` có **hạn** (`FS_REPLY_BUDGET`) ⇒ ảnh không FS trả prompt.
/// In `# shell: sync ok`; lỗi FS in `# shell: fs loi code=<n>` (do `fs_status`).
#[cfg(shell_fs)]
fn cmd_sync() {
    match fs_call([OP_SYNC, 0, 0, 0]) {
        Some(reply) => {
            if fs_status(reply) == Some(ST_OK) {
                put(b"# shell: sync ok\n");
            }
        }
        None => no_fs(),
    }
}

/// **KT-114** `stat <path>` — `OP_STAT` trả `[ST_OK | kind<<8, size, ino, 0]` (`proc_fs::stat`).
/// In `# shell: stat path=<p> kind=<k> size=<n> ino=<i>`; đường dẫn không có ⇒ `fs loi code=5`.
#[cfg(shell_fs)]
fn cmd_stat(path: &[u8]) {
    if fs_set_path(path).is_err() {
        no_fs();
        return;
    }
    let Some(reply) = fs_call([OP_STAT, 0, 0, 0]) else {
        no_fs();
        return;
    };
    if fs_status(reply).is_none() {
        return;
    }
    put(b"# shell: stat path=");
    put(path);
    put(b" kind=");
    put_u64((reply[0] >> 8) & 0xff);
    put(b" size=");
    put_u64(reply[1]);
    put(b" ino=");
    put_u64(reply[2]);
    put(b"\n");
}

/// Tách `line` theo dấu cách đầu tiên: `(đầu, phần còn lại sau dấu cách)`.
#[cfg(shell_fs)]
fn split_space(line: &[u8]) -> (&[u8], &[u8]) {
    let mut index = 0usize;
    while index < line.len() {
        if line[index] == b' ' {
            return (&line[..index], &line[index + 1..]);
        }
        index += 1;
    }
    (line, &[])
}
