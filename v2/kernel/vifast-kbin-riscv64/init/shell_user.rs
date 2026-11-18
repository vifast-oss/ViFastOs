// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-65 — **shell tối thiểu** chạy U-mode (Rust, ELF riêng nạp như `init`).
//!
//! **KT-73**: đọc **byte thật** từ kênh UART → ring của KT-69. Kernel nhận **ngắt RX của UART**,
//! đẩy byte vào ring (`vifast_ring`, slot 24 B do `vifast_virtio::conchan` mã hoá) và map Frame
//! đó vào VSpace này tại `VA_CON_RING = 0x4300_0000`; shell `Ring::recv` từng slot, ghép dòng theo
//! `\n` rồi chạy lệnh. **Không** còn message `LINE` do kernel cắm sẵn — shell đọc đúng đường
//! IRQ → RBR → ring → U-mode (đóng nợ "shell chưa UART IRQ" từ K3.5).
//!
//! Lệnh thật:
//! - `help`  — in trợ giúp (do **shell** in, không phải kernel).
//! - `echo <text>` — in đúng phần text.
//! - `ps`    — gửi `PS_REQ`, nhận bảng TCB **do kernel đọc từ `Machine`** rồi tự định dạng/in.
//! - `wait <pid>` / `kill <pid>` — gọi **giao thức procsrv** (`NBSend(proc_ep)` + `NBRecv(reply_ep)`),
//!   in mã trả về. Không phải syscall mới.
//! - **KT-77c** (chỉ khi build ELF này với `--cfg shell_fs`, tức ảnh `product-fs`):
//!   `ls`, `cat <path>`, `write <path> <text>` — **dịch vụ FS của kernel**
//!   (`EP_FS`, `FS_REQ`/`FS_REPLY`, bố cục MR ở `src/proc_fs.rs`). Cùng hai syscall đã có
//!   (`NBSend`/`NBRecv`), **không** mở ABI. `write` tự `fsync` (journal + `flush` thiết bị) nên
//!   dữ liệu qua reboot; chờ reply có **hạn** (`FS_REPLY_BUDGET` lượt) để ảnh **không** FS/không
//!   đĩa **không treo** — in `fs khong san sang` rồi trả prompt.
//! - **KT-114** (cùng nhánh `#[cfg(shell_fs)]`, ảnh `product-fs`): `time`, `uptime`, `sync`,
//!   `stat <path>` — **không** mở ABI mới: `time`/`uptime` gọi `ClockRead` (11) đã có,
//!   `sync`/`stat` dùng `OP_SYNC`/`OP_STAT` của dịch vụ FS đã có. Ảnh test (không FS) giữ **6**
//!   lệnh ⇒ kỳ vọng `# shell: help cmds=6` của harness không đổi.
//! - **KT-137** (cùng nhánh `#[cfg(shell_fs)]`): `cd [path]` + `pwd` — shell giữ **thư mục hiện
//!   tại** trong buffer cố định ([`Cwd`]) và **tự chuẩn hoá từ vựng** đường dẫn (`.`/`..`/`//`)
//!   trước khi gửi `OP_PATH`; mọi lệnh có tham số đường dẫn (`ls`/`cat`/`write`/`truncate`/`stat`)
//!   nhận **đường tương đối** so với thư mục hiện tại. **Không** đổi ngữ nghĩa FS, không đổi giao
//!   thức FS server, không mở ABI: `vifast-fs` vốn từ chối `.`/`..` và đòi đường tuyệt đối.
//! - `exit`  — `Exit(0)`.
//!
//! Vòng lặp polling + `Yield`: kernel là scheduler tuần tự của pha demo (không có scheduler
//! U-mode trong core; xem `docs/k35-procsrv.md` §2).
#![no_std]
#![no_main]


extern crate vifast_ring;
extern crate vifast_user_abi;
extern crate vifast_virtio;
use vifast_ring::{Mem, Ring, ERR_EMPTY};
/// **KT-114**: `ClockRead` (11) — chỉ nhánh `shell_fs` dùng (`time`/`uptime`); ảnh test không
/// import để không sinh cảnh báo "unused import".
#[cfg(shell_fs)]
use vifast_user_abi::SYS_CLOCK_READ;
use vifast_user_abi::{
    atomic_load_acquire, atomic_load_relaxed, atomic_store_release, atomic_store_relaxed, exit,
    put, syscall7, SYS_EXIT, SYS_NB_RECV, SYS_NB_SEND, SYS_YIELD,
};
use vifast_virtio::conchan;

// Cspace dùng chung của ảnh demo (kernel dựng; xem `proc_demo.rs`).
const EP_PROC: u64 = 2;
const EP_REPLY: u64 = 4;
const EP_PS: u64 = 5;
const EP_EVENTS: u64 = 8;
/// **KT-77c**: endpoint dịch vụ FS phía kernel (slot 7 — khớp `proc_abi::EP_FS`).
#[cfg(shell_fs)]
const EP_FS: u64 = 7;
/// Label/giao thức FS — khớp `proc_abi::FS_REQ`/`FS_REPLY` và `OP_*`/`ST_*`.
#[cfg(shell_fs)]
const FS_REQ: u64 = 0x612;
#[cfg(shell_fs)]
const FS_REPLY: u64 = 0x613;
#[cfg(shell_fs)]
const OP_PATH: u64 = 0x10;
#[cfg(shell_fs)]
const OP_LS: u64 = 0x11;
#[cfg(shell_fs)]
const OP_CAT: u64 = 0x12;
#[cfg(shell_fs)]
const OP_WRITE: u64 = 0x13;
#[cfg(shell_fs)]
const OP_TRUNC: u64 = 0x14;
/// **KT-114**: `OP_STAT` — `[ST_OK | kind<<8, size, ino, 0]` (khớp `proc_abi::OP_STAT`).
#[cfg(shell_fs)]
const OP_STAT: u64 = 0x15;
#[cfg(shell_fs)]
const OP_SYNC: u64 = 0x16;
#[cfg(shell_fs)]
const ST_OK: u64 = 0;
#[cfg(shell_fs)]
const ST_END: u64 = 1;
/// **KT-123**: dịch vụ FS không sẵn sàng (khớp `proc_abi::ST_NOFS`).
#[cfg(shell_fs)]
const ST_NOFS: u64 = 3;
/// Số byte tên/dữ liệu mỗi request/reply (3 MR word).
#[cfg(shell_fs)]
const FS_BYTES: usize = 24;
/// Hạn chờ một reply FS (số lượt `Yield`) — ảnh **không** FS/không đĩa ⇒ trả prompt, không treo.
#[cfg(shell_fs)]
const FS_REPLY_BUDGET: u32 = 20_000;

/// VA ring UART → shell (kernel map; khớp `vifast_plat_riscv64::uart_console::VA_CON_RING`).
const VA_CON_RING: u64 = 0x4300_0000;
/// Cỡ dòng tối đa shell nhận — **KT-77c** nâng 32 → 64 để `write <path> <text>` của FS vừa một
/// dòng (đường dẫn ≤ 24 B + payload; `MAX_PATH` của kernel là 48 B, shell gửi từng khối 24 B).
#[cfg(shell_fs)]
const MAX_LINE: usize = 64;
#[cfg(not(shell_fs))]
const MAX_LINE: usize = 32;
/// Số lệnh shell biết (in trong `help`) — **một** chỗ để `cmds=` không lệch với danh sách.
/// **KT-77c** chỉ thêm ba lệnh FS vào ELF build với `--cfg shell_fs` (ảnh `product-fs`); ảnh test
/// và ảnh ca âm "shell nhưng không FS" giữ **6** ⇒ kỳ vọng `# shell: help cmds=6` của harness
/// (`proc_demo.rs`, `v2/test/boot/ktest.sh`) **không** phải sửa.
/// **KT-114** thêm bốn lệnh nữa (`time`/`uptime`/`sync`/`stat`) **cũng chỉ** trong nhánh
/// `shell_fs` ⇒ ảnh sản phẩm **9 → 13**, ảnh test vẫn **6**.
/// **KT-137** thêm `cd`/`pwd` (cùng nhánh) ⇒ ảnh sản phẩm **15 → 17**, ảnh test vẫn **6**.
#[cfg(shell_fs)]
const CMDS: u64 = 17;
#[cfg(not(shell_fs))]
const CMDS: u64 = 6;

/// Adapter [`Mem`] trên VA user (cùng shim `ring_common` của KT-69).
struct Shm {
    base: u64,
}

impl Mem for Shm {
    fn load(&self, offset: usize) -> u64 {
        atomic_load_relaxed(self.base + offset as u64)
    }
    fn store(&self, offset: usize, value: u64) {
        atomic_store_relaxed(self.base + offset as u64, value);
    }
    fn acquire(&self, offset: usize) -> u64 {
        atomic_load_acquire(self.base + offset as u64)
    }
    fn store_rel(&self, offset: usize, value: u64) {
        atomic_store_release(self.base + offset as u64, value);
    }
}

const PS_REQ: u64 = 0x602;
const PS_REPLY: u64 = 0x603;
const WAIT_REQ: u64 = 0x604;
const KILL_REQ: u64 = 0x605;
const REPLY_WAIT: u64 = 0x606;
const REPLY_KILL: u64 = 0x607;
const EV_HELP: u64 = 0x507;
const EV_ECHO: u64 = 0x506;
const EV_PS: u64 = 0x505;

const INFO_LEN_SHIFT: u64 = 28;

/// In hex (không `unsafe`, không `alloc`) — để evidence in từ **hằng**, không viết tay chuỗi.
fn put_hex(mut value: u64) {
    let digits = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        buf[i] = digits[(value & 0xf) as usize];
        value >>= 4;
    }
    put(&buf[i..]);
}

fn put_u64(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    put(&buf[i..]);
}

fn yield_cpu() {
    // Ghi chú: syscall 8 không tham số; trong pha đa nhiệm kernel dùng làm điểm nhường CPU.
    
    let _ = syscall7(SYS_YIELD, 0, 0, 0, 0, 0, 0);

}

fn send(ep: u64, label: u64, mr: [u64; 4]) {
    let words = if mr[3] != 0 {
        4
    } else if mr[2] != 0 {
        3
    } else if mr[1] != 0 {
        2
    } else if mr[0] != 0 {
        1
    } else {
        0
    };
    let info = label | (words << INFO_LEN_SHIFT);
    // Ghi chú: syscall 2 với cap Endpoint trong cspace của chính tiến trình.
    
    let _ = syscall7(SYS_NB_SEND, ep, info, mr[0], mr[1], mr[2], mr[3]);

}

fn recv(ep: u64) -> (u64, u64, [u64; 4]) {
    // Ghi chú: syscall 4 với cap Endpoint; kernel trả info a2 + MR a3..a6.
    let r =  syscall7(SYS_NB_RECV, ep, 0, 0, 0, 0, 0) ;
    (r[0], r[2], [r[3], r[4], r[5], r[6]])
}

fn starts_with(line: &[u8], prefix: &[u8]) -> bool {
    line.len() >= prefix.len() && &line[..prefix.len()] == prefix
}

fn parse_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0u64;
    for &b in bytes {
        if !b.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    Some(value)
}

fn state_name(code: u64) -> &'static [u8] {
    match code {
        0 => b"Inactive",
        1 => b"Ready",
        2 => b"Running",
        3 => b"BlockedOnSend",
        4 => b"BlockedOnRecv",
        5 => b"BlockedOnReply",
        6 => b"BlockedOnNotification",
        7 => b"BlockedOnAddress",
        8 => b"Sleeping",
        9 => b"Stopping",
        10 => b"Dead",
        _ => b"Unknown",
    }
}

/// Chờ **một** reply có đúng `want` label; `Busy`/message khác ⇒ `Yield` rồi thử lại.
fn wait_reply(want: u64) -> (u64, [u64; 4]) {
    loop {
        let (err, info, mr) = recv(EP_REPLY);
        if err == 0 && info & 0xFFFFF == want {
            return (info & 0xFFFFF, mr);
        }
        yield_cpu();
    }
}

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

/// `jobs` — **KT-125**: đếm tiến trình theo trạng thái từ **cùng** reply `PS_REQ` mà `ps` dùng
/// (không thêm đường kernel). In `ready=`, `running=`, `blocked=`, `other=`.
#[cfg(shell_fs)]
fn cmd_jobs() {
    send(EP_PS, PS_REQ, [0, 0, 0, 0]);
    let (_, mr) = wait_reply(PS_REPLY);
    let mut ready = 0u64;
    let mut running = 0u64;
    let mut blocked = 0u64;
    let mut other = 0u64;
    for word in mr {
        for half in 0..2 {
            let entry = if half == 0 { word & 0xFFFF } else { word >> 16 };
            if entry == 0 {
                continue;
            }
            // Cùng cách giải mã **bốn bit** trạng thái như `cmd_ps` (KT-105).
            match (entry >> 1) & 0x0F {
                1 => ready += 1,
                2 => running += 1,
                3..=7 => blocked += 1,
                _ => other += 1,
            }
        }
    }
    put(b"# shell: jobs ready=");
    put_u64(ready);
    put(b" running=");
    put_u64(running);
    put(b" blocked=");
    put_u64(blocked);
    put(b" other=");
    put_u64(other);
    put(b"\n");
}

fn cmd_ps() {
    send(EP_PS, PS_REQ, [0, 0, 0, 0]);
    let (_, mr) = wait_reply(PS_REPLY);
    let mut list = [0u64; 8];
    let mut count = 0usize;
    for word in mr {
        for half in 0..2 {
            let entry = if half == 0 { word & 0xFFFF } else { word >> 16 };
            if entry != 0 && count < list.len() {
                list[count] = entry;
                count += 1;
            }
        }
    }
    put(b"# shell: ps n=");
    put_u64(count as u64);
    put(b"\n");
    for entry in list.iter().take(count) {
        let pid = entry >> 5;
        // KT-105: kernel đóng gói `(id << 5) | (state.index() << 1) | suspended` ⇒ trạng thái
        // chiếm **bốn** bit (1..4), pid bắt đầu ở bit 5. Bản cũ lấy `& 0x1F` (năm bit) nên
        // `(entry >> 1) & 0x1F` = 16 + state ⇒ **mọi** tiến trình in `state=Unknown`.
        let state = (entry >> 1) & 0x0F;
        put(b"  pid=");
        put_u64(pid);
        put(b" state=");
        put(state_name(state));
        if entry & 1 != 0 {
            put(b" suspended");
        }
        put(b"\n");
    }
    send(EP_EVENTS, EV_PS, [count as u64, 0, 0, 0]);
}

fn cmd_wait(pid: u64) {
    send(EP_PROC, WAIT_REQ, [pid, 0, 0, 0]);
    let (_, mr) = wait_reply(REPLY_WAIT);
    put(b"# shell: wait pid=");
    put_u64(mr[0]);
    put(b" code=");
    put_u64(mr[1]);
    put(b"\n");
}

fn cmd_kill(pid: u64) {
    send(EP_PROC, KILL_REQ, [pid, 0, 0, 0]);
    let (_, mr) = wait_reply(REPLY_KILL);
    put(b"# shell: kill pid=");
    put_u64(mr[0]);
    put(b" code=");
    put_u64(mr[1]);
    put(b"\n");
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

// ---------------------------------------------------------------------------------------------
// **KT-137** — `cd`/`pwd` + thư mục hiện tại + **đường dẫn tương đối**.
//
// Vì sao **chỉ** sửa shell: `vifast-fs` đòi đường **tuyệt đối** và **từ chối** hai tên `.`/`..`
// (`v2/lib/vifast-fs/src/path.rs:123,148`), nên shell phải tự chuẩn hoá **từ vựng** rồi mới gửi
// `OP_PATH`. Nhờ vậy **không** đổi ngữ nghĩa FS, **không** đổi giao thức FS server, **không** mở
// ABI — đúng ràng buộc KT-137.
//
// Thư mục hiện tại sống trong **buffer cố định** trên stack của `_start` (không `static mut`,
// không `unsafe`, không `alloc`), truyền xuống `run_line`/`fs_dispatch` bằng `&mut`.
// ---------------------------------------------------------------------------------------------

/// Cỡ tối đa của thư mục hiện tại = `proc_fs::MAX_PATH` (kernel **từ chối** đường dài hơn 48 B).
#[cfg(shell_fs)]
const CWD_MAX: usize = 48;
/// `kind` của thư mục trong `OP_STAT` (`proc_fs::stat` trả `kind<<8`; 2 = `INODE_DIR`).
#[cfg(shell_fs)]
const KIND_DIR: u64 = 2;

/// Thư mục hiện tại: đường dẫn **tuyệt đối, đã chuẩn hoá** (không `.`/`..`, không `//`, không
/// `/` cuối — trừ gốc `/`).
#[cfg(shell_fs)]
struct Cwd {
    buf: [u8; CWD_MAX],
    len: usize,
}

#[cfg(shell_fs)]
impl Cwd {
    /// Gốc `/` — trạng thái đầu của shell (khớp v1: `pwd` đầu tiên in `/`).
    fn root() -> Self {
        let mut buf = [0u8; CWD_MAX];
        buf[0] = b'/';
        Cwd { buf, len: 1 }
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Ghi đè bằng đường **đã chuẩn hoá** (caller bảo đảm `path.len() <= CWD_MAX`).
    fn set(&mut self, path: &[u8]) {
        let take = core::cmp::min(path.len(), CWD_MAX);
        self.buf[..take].copy_from_slice(&path[..take]);
        self.len = take;
    }
}

/// Lỗi phân giải đường dẫn (không phải lỗi FS — chưa gửi request nào).
#[cfg(shell_fs)]
enum PathErr {
    /// Tham số rỗng (`cd ` / `cat ` với toàn dấu cách).
    Empty,
    /// Đường sau khi ghép dài hơn [`CWD_MAX`] ⇒ kernel sẽ từ chối, báo sớm cho đúng.
    TooLong,
}

/// In thông điệp lỗi phân giải đường dẫn (dùng chung cho mọi lệnh có tham số đường dẫn).
#[cfg(shell_fs)]
fn path_err(err: PathErr) {
    match err {
        PathErr::Empty => put(b"# shell: thieu duong dan\n"),
        PathErr::TooLong => put(b"# shell: duong qua dai\n"),
    }
}

/// Ghép `arg` với `cwd` rồi **chuẩn hoá từ vựng** vào `out`; trả độ dài (luôn bắt đầu bằng `/`).
///
/// Quy tắc (giống POSIX ở mức từ vựng, **không** chạm đĩa):
/// - `arg` bắt đầu bằng `/` ⇒ tính từ gốc; ngược lại tính từ `cwd`.
/// - `//` gộp, `.` bỏ, `..` lùi **một** thành phần; ở gốc thì `..` **đứng yên** (không lên trên `/`).
/// - Kết quả không có `/` cuối (trừ gốc) ⇒ `pwd` in đúng một dạng duy nhất cho mỗi thư mục.
#[cfg(shell_fs)]
fn resolve_path(cwd: &Cwd, arg: &[u8], out: &mut [u8; CWD_MAX]) -> Result<usize, PathErr> {
    if arg.is_empty() {
        return Err(PathErr::Empty);
    }
    let mut len;
    if arg[0] == b'/' {
        out[0] = b'/';
        len = 1;
    } else {
        len = cwd.len;
        out[..len].copy_from_slice(cwd.as_slice());
    }
    let mut pos = 0usize;
    while pos < arg.len() {
        while pos < arg.len() && arg[pos] == b'/' {
            pos += 1;
        }
        let start = pos;
        while pos < arg.len() && arg[pos] != b'/' {
            pos += 1;
        }
        let comp = &arg[start..pos];
        if comp.is_empty() || comp == b"." {
            continue;
        }
        if comp == b".." {
            // Lùi một thành phần: bỏ dấu `/` cuối + tên cuối; ở gốc (`len == 1`) thì đứng yên.
            if len > 1 {
                let mut cut = len;
                while cut > 1 && out[cut - 1] != b'/' {
                    cut -= 1;
                }
                len = if cut > 1 { cut - 1 } else { 1 };
            }
            continue;
        }
        // Gốc `/` đã kết thúc bằng `/` ⇒ **không** thêm dấu phân cách (nếu thêm sẽ ra `//x`).
        let sep = if len != 0 && out[len - 1] == b'/' { 0 } else { 1 };
        if len + sep + comp.len() > CWD_MAX {
            return Err(PathErr::TooLong);
        }
        if sep == 1 {
            out[len] = b'/';
            len += 1;
        }
        out[len..len + comp.len()].copy_from_slice(comp);
        len += comp.len();
    }
    Ok(len)
}

/// Phân giải tham số đường dẫn của một lệnh; `None` = đã in lỗi, caller **không** gọi lệnh.
#[cfg(shell_fs)]
fn resolve_arg(cwd: &Cwd, arg: &[u8]) -> Option<([u8; CWD_MAX], usize)> {
    let mut out = [0u8; CWD_MAX];
    match resolve_path(cwd, arg, &mut out) {
        Ok(len) => Some((out, len)),
        Err(err) => {
            path_err(err);
            None
        }
    }
}

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
fn fs_dispatch(line: &[u8], cwd: &mut Cwd) -> bool {
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
    } else {
        false
    }
}

/// Đóng gói tối đa [`FS_BYTES`] byte vào 3 MR word (little-endian từng byte).
#[cfg(shell_fs)]
fn pack_bytes(bytes: &[u8]) -> [u64; 3] {
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
fn fs_call(words: [u64; 4]) -> Option<[u64; 4]> {
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
fn fs_status(reply: [u64; 4]) -> Option<u64> {
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
fn fs_set_path(path: &[u8]) -> Result<(), ()> {
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
fn no_fs() {
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

/// Chạy **một** dòng lệnh. `cwd` chỉ tồn tại ở ảnh có FS (`shell_fs`) — ảnh test không có thư mục
/// hiện tại nên tham số được `#[cfg]` bỏ hẳn (không sinh cảnh báo "unused").
fn run_line(line: &[u8], #[cfg(shell_fs)] cwd: &mut Cwd) {
    // **KT-77c**: lệnh FS của shell sản phẩm đi qua `fs_dispatch` — tách khỏi chuỗi `if/else` để
    // ảnh **không** `shell_fs` giữ nguyên chuỗi lệnh cũ (kỳ vọng harness không đổi).
    #[cfg(shell_fs)]
    if fs_dispatch(line, cwd) {
        return;
    }
    if starts_with(line, b"echo ") {
        put(b"# shell: echo ");
        put(&line[5..]);
        put(b"\n");
        send(EP_EVENTS, EV_ECHO, [line.len() as u64, 0, 0, 0]);
    } else if line == b"help" {
        put(b"# shell: help cmds=");
        put_u64(CMDS);
        put(b"\n");
        #[cfg(shell_fs)]
        put(b"  help | echo <text> | ps | wait <pid> | kill <pid> | ls [path] | cd [path] | pwd | cat <path> | write <path> <text> | time | uptime | sync | stat <path> | truncate <path> <size> | jobs | exit\n");
        #[cfg(not(shell_fs))]
        put(b"  help | echo <text> | ps | wait <pid> | kill <pid> | exit\n");
        send(EP_EVENTS, EV_HELP, [CMDS, 0, 0, 0]);
    } else if line == b"ps" {
        cmd_ps();
    } else if starts_with(line, b"wait ") {
        if let Some(pid) = parse_u64(&line[5..]) {
            cmd_wait(pid);
        }
    } else if starts_with(line, b"kill ") {
        if let Some(pid) = parse_u64(&line[5..]) {
            cmd_kill(pid);
        }
    } else if line == b"exit" {
        put(b"# shell: exit code=0\n");
        // Kernel ghi `FaultKind::Exit{0}` vào fault_ep của shell (procsrv nhận).
        exit(0);
    } else {
        put(b"# shell: ?\n");
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    #[cfg(shell_fs)]
    put(b"# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/exit\n");
    #[cfg(not(shell_fs))]
    put(b"# shell: up cmds=help/echo/ps/wait/kill/exit\n");
    put(b"# shell: input=ring va=0x");
    put_hex(VA_CON_RING as u64);
    put(b"\n");
    // KT-77b: prompt của shell. In **một dòng riêng** (`shell>\n`, không phải tiền tố `shell> `
    // cùng dòng) để mọi dòng kết quả `# shell: …` vẫn bắt đầu ở cột 0 — kịch bản parity
    // (`spec/v1-parity.toml`, `expect` dạng `^# shell: …$`) so khớp theo dòng.
    put(b"shell>\n");
    let ring = Ring::new(Shm {
        base: VA_CON_RING,
    });
    // Ring do **kernel** khởi tạo (header KT-69); shell chỉ mở/kiểm.
    if ring.open().is_err() {
        put(b"# shell: ring open FAIL\n");
        exit(90)
    }
    let mut words = [0u64; 4];
    let mut line = [0u8; MAX_LINE];
    let mut len = 0usize;
    // **KT-137**: thư mục hiện tại của shell — sống **trong** `_start` (không `static mut`), bắt
    // đầu ở gốc `/` như v1.
    #[cfg(shell_fs)]
    let mut cwd = Cwd::root();
    // KT-103: cờ "dòng này đã báo quá dài" — trước đây mỗi byte vượt `MAX_LINE` in **một** dòng
    // cảnh báo ⇒ một dòng dài một nghìn byte in gần một nghìn dòng, làm ngập log và làm chậm
    // UART. Nay báo **một lần** cho mỗi dòng.
    let mut overflowed = false;
    loop {
        match ring.recv(&mut words) {
            Ok(()) => {
                let mut slot = [0u8; 24];
                let count = conchan::unpack(&words, &mut slot);
                if count == 0 {
                    put(b"# shell: slot hong\n");
                    exit(91)
                }
                for byte in &slot[..count] {
                    if *byte == b'\n' {
                        run_line(&line[..len], #[cfg(shell_fs)] &mut cwd);
                        len = 0;
                        overflowed = false;
                        // Prompt sau **mỗi** dòng lệnh (kể cả dòng sai) — trừ khi `run_line`
                        // đã `exit` (không trả về).
                        put(b"shell>\n");
                    } else if len < MAX_LINE {
                        line[len] = *byte;
                        len += 1;
                    } else if !overflowed {
                        // Dòng quá dài: bỏ phần dư, không tràn buffer — báo **một lần** mỗi dòng.
                        overflowed = true;
                        put(b"# shell: dong qua dai\n");
                    }
                }
            }
            Err(ERR_EMPTY) => yield_cpu(),
            Err(_) => {
                put(b"# shell: ring loi\n");
                exit(92)
            }
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // Panic U-mode không phục hồi; thoát 99 để kernel đỏ (không giả xanh).
    exit(99)
}
