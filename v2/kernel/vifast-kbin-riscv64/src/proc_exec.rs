// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342 + KT-347** — `run <path>` trên ảnh **SẢN PHẨM** riscv64 (`product-exec`): **exec
//! kernel-mediated** (D2-96, cùng khuôn `KT-74`).
//!
//! # Vì sao exec là kernel-mediated mà không phải syscall
//!
//! `spec/abi.toml` đóng băng 16 syscall (`0..15`) và `[object].Tcb` **không** có method `Exec`
//! (D2-15/D2-74); luật vòng này cấm sửa `spec/` (D2-04). Quyết định *exec tiến trình nào sang
//! ảnh nào* là **policy** (D2-96) — kernel thi hành, shell chỉ gửi yêu cầu trên `Endpoint`
//! `EP_EXEC` bằng `NBSend` (đường IPC đã có, **không** mở ABI).
//!
//! # KT-347 — **đọc ELF từ FS** (đóng nợ KT-346 mục 1)
//!
//! Trước KT-347, `run` tra bảng ảnh **nhúng** (`static HELLO_ELF = include_bytes!(…)`, KT-342).
//! Nay:
//!
//! 1. runner host seed `/hello.elf` + `/u1a.elf` vào **đĩa** FS bằng `vifast-fsutil` (mkfs +
//!    put-file) **trước** khi QEMU boot;
//! 2. kernel đọc **cả tệp** từ `Fs` đã mount qua API [`crate::proc_fs::read_file`] vào buffer
//!    scratch (`Scratch<[u8; MAX_ELF_BYTES]>`, **không** trên stack — KT-73/75 đã đo tràn guard
//!    page 64 KiB với `Fs` 13 KiB; buffer 16 KiB cũng không được nằm trên khung gọi);
//! 3. [`vifast_plat_riscv64::exec::exec_bytes`] nạp chính các byte đó — **không** `include_bytes!`.
//!
//! Fail-closed: FS không sẵn sàng / tệp không có / tệp lớn hơn buffer ⇒ trả `EXEC_REPLY` với mã
//! có tên (shell in `# shell: run err=<n>`, không treo, ảnh cũ nguyên vẹn).
//!
//! # Bằng chứng (ảnh sản phẩm không có TAP — D2-08)
//!
//! Kernel in **một** dòng `# product exec: path=… src=fs bytes=… old_entry=… new_entry=…
//! frames=… objects=… caps=… check=1` (số đo từ `ExecReport`); ảnh đích in `RUN OK` +
//! `# hello: run ok` (hoặc `# u1a: rt done`) rồi `Exit(0)`.

use vifast_core::{Machine, ObjId, SlotRef};
use vifast_fs::FsError;
use vifast_plat_common::obj::CapId;
use vifast_plat_common::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::exec::{self, ExecError, ExecReport};
use vifast_plat_riscv64::static_slot::Scratch;
use vifast_plat_riscv64::user::{LoadedInit, USER_STACK_TOP};

use crate::proc_abi::{EP_EXEC, EXEC_REPLY, EXEC_REQ};
#[cfg(feature = "product-fs")]
use crate::proc_fs::{read_file, FsVol};

/// FS của ảnh sản phẩm khi bật `product-fs`; ảnh `product-exec` **không** FS thì kiểu rỗng
/// (mọi yêu cầu `run` trả `err=4` — fail-closed, không có bảng nhúng để rơi về).
#[cfg(feature = "product-fs")]
pub type FsArg<'a> = Option<&'a mut FsVol>;
#[cfg(not(feature = "product-fs"))]
pub type FsArg<'a> = Option<&'a mut ()>;

/// Số byte đường dẫn tối đa mang trong **hai** MR word (8 + 8).
const MAX_PATH: usize = 16;
/// Cỡ tối đa của một ELF U-mode nạp qua `run` — chặn trên **fail-closed** (không cắt bớt).
///
/// **KT-401** (D2-252/D2-255): nâng 24 KiB → **40 KiB** sau khi mở biên trang ELF user riscv64
/// (`USER_STACK_TOP = 0x4010_0000`, `MAX_INIT_PAGES = 32`), đủ cho user ELF **> 24 KiB** —
/// gồm `shell.elf` hiện tại 32.928 B (bằng
/// chứng biên ở `KT-347`/`ktest.sh`: `# init load … guard=0x400f7000`); buffer nằm trong
/// `.bss` của kernel chứ **không** trên stack, xem đầu tệp. `Machine` đã ra `.corespace`
/// NOLOAD nên `.data+.bss` còn rất nhiều headroom dưới trần 262.144 B (đo ở cổng ngân sách).
pub const MAX_ELF_BYTES: usize = 40 * 1024;

/// **KT-401** — bằng chứng biên ngay lúc biên dịch: `proc_exec` phải nạp được user ELF
/// **lớn hơn 24 KiB** (yêu cầu D2-255); hạ giới hạn lại ⇒ lỗi biên dịch, không phải lỗi lúc chạy.
const _: () = assert!(
    MAX_ELF_BYTES > 24 * 1024,
    "KT-401: MAX_ELF_BYTES phải > 24 KiB (đã mở biên ELF user riscv64)"
);

/// Buffer scratch đọc ELF từ FS — **tái dùng** (nhiều lượt `run` hỏng rồi thử lại).
static ELF_BUF: Scratch<[u8; MAX_ELF_BYTES]> = Scratch::new([0u8; MAX_ELF_BYTES]);

/// Đường IPC `run` shell → kernel: endpoint `EP_EXEC` + object của nó.
pub struct ExecRig {
    /// Slot `EP_EXEC` trong CNode gốc (kernel giữ).
    pub ep: SlotRef,
}

/// Dựng endpoint `EP_EXEC` (16 slot dùng chung cspace sản phẩm).
pub fn build(m: &mut Machine, root: ObjId, us: SlotRef) -> ExecRig {
    let cap = m
        .retype(us, vifast_core::ObjType::Endpoint, 0, 0)
        .expect("KT-342: retype EP_EXEC");
    m.put_cap(root, EP_EXEC, cap).expect("KT-342: put EP_EXEC");
    ExecRig {
        ep: SlotRef::new(root, EP_EXEC),
    }
}

/// Kết quả một lượt phục vụ `EP_EXEC`.
pub enum Outcome {
    /// Không có yêu cầu (endpoint rỗng) — vòng sau thử lại.
    Empty,
    /// Đã `exec` thành công: ảnh shell đã bị thay; `report` là số đo + đường dẫn nguồn.
    Executed(Executed),
    /// Từ chối/lỗi — kernel đã trả `EXEC_REPLY(EXEC_REPLY, err)` cho shell.
    Failed(u64),
    /// Lỗi IPC nội bộ (không phục vụ được).
    Error,
}

/// Số đo + đường dẫn của một lần `exec` thành công (dùng cho dòng evidence).
pub struct Executed {
    /// Số đo của loader (`ExecReport`: entry/vspace/frames/objects/caps).
    pub report: ExecReport,
    /// Đường dẫn **đã dùng** (copy ra khỏi message để in sau khi message bị thu hồi).
    pub path: [u8; MAX_PATH],
    /// Số byte đường dẫn có nghĩa trong [`Self::path`].
    pub path_len: u8,
    /// Số byte ELF đọc được từ FS (bằng chứng "đọc từ FS", không phải bảng nhúng).
    pub bytes: usize,
}

/// Mã lỗi `EXEC_REPLY` (ổn định — shell in số, product script kiểm `err=[0-9]+`).
const ERR_NO_FS: u64 = 4;
const ERR_READ: u64 = 6;
const ERR_EMPTY: u64 = 7;

/// Rút **một** yêu cầu `run` đang chờ trên `EP_EXEC` (không chặn), kiểm dạng rồi `exec`.
///
/// `cur` là ảnh hiện tại của **ô shell** (`pid`), `tf` là trapframe sẽ chạy tiếp: thành công ⇒
/// trapframe được thay bằng entry/`sp` của ảnh mới (mọi thanh ghi khác = 0, TLS = 0 — đúng hợp
/// đồng `exec` của `KT-74`). `fs` là FS đã mount của ảnh sản phẩm (`None` ⇔ ảnh không FS).
#[allow(clippy::too_many_arguments)]
pub fn service(
    m: &mut Machine,
    rig: &ExecRig,
    tcb_k: ObjId,
    cur: &mut LoadedInit,
    pid: usize,
    tcb: SlotRef,
    tf: &mut TrapFrame,
    untyped: CapId,
    reply: SlotRef,
    fs: FsArg<'_>,
) -> Outcome {
    // 1) Nhận yêu cầu (không chặn: endpoint rỗng ⇒ `Busy` ⇒ `Empty`).
    let request = match m.nb_recv(tcb_k, rig.ep) {
        Ok(Some(request)) => request,
        Ok(None) | Err(_) => return Outcome::Empty,
    };
    let label = u64::from(request.info.label());
    // 2) Kiểm **dạng** yêu cầu trước mọi thao tác (không nửa vời): label đúng + độ dài 1..=16.
    let head = request.words().first().copied().unwrap_or(0);
    let len = (head & 0xff) as usize;
    if label != EXEC_REQ {
        return reply_error(m, tcb_k, reply, 15);
    }
    if len == 0 || len > MAX_PATH {
        return reply_error(m, tcb_k, reply, 8);
    }
    // Shell đặt `length` = số MR **có nghĩa** (1..=4): `/u1a.elf` (7 B) vừa đúng MR0 nên
    // `length=1`; `/hello.elf` (10 B) cần thêm MR1 nên `length=2`. Chỉ đòi **>=1** (MR0 mang
    // `len` + 7 byte đầu); đòi 2 sẽ từ chối mọi đường dẫn ≤7 B (đo được: err=10 cho `/u1a.elf`).
    if request.info.length() < 1 {
        return reply_error(m, tcb_k, reply, 10);
    }
    // 3) Giải mã đường dẫn từ MR0 (8 B thấp ở byte cao) + MR1.
    let mut path = [0u8; MAX_PATH];
    // MR0 chứa **7** byte đầu (bit 8..63; byte 0 là `len`), phần còn lại ở MR1 — khớp
    // `shell_user::cmd_run`. 8 byte ở MR0 sẽ tràn khi `<<8` (lỗi cũ: `/hello.lf` ⇒ NotFound).
    let mr0 = head >> 8;
    let mr1 = request.words().get(1).copied().unwrap_or(0);
    for (index, slot) in path.iter_mut().take(len).enumerate() {
        *slot = if index < 7 {
            (mr0 >> (index * 8)) as u8
        } else {
            (mr1 >> ((index - 7) * 8)) as u8
        };
    }
    let Ok(path_str) = core::str::from_utf8(&path[..len]) else {
        return reply_error(m, tcb_k, reply, 9);
    };
    // 4) KT-347: đọc **cả tệp** ELF từ FS (kernel-mediated; không phải op IPC 24 B/lần).
    //    Ảnh `product-exec` **không** FS: không có bảng nhúng để rơi về ⇒ fail-closed có mã.
    #[cfg(not(feature = "product-fs"))]
    {
        let _ = (fs, path_str);
        return reply_error(m, tcb_k, reply, ERR_NO_FS);
    }
    #[cfg(feature = "product-fs")]
    {
        let Some(fs) = fs else {
            return reply_error(m, tcb_k, reply, ERR_NO_FS);
        };
        let buf = ELF_BUF.borrow_mut();
        let bytes = match read_file(fs, path_str.as_bytes(), &mut buf[..]) {
            Ok(0) => return reply_error(m, tcb_k, reply, ERR_EMPTY),
            Ok(bytes) => bytes,
            Err(FsError::NotFound) => return reply_error(m, tcb_k, reply, 2),
            Err(FsError::Invalid) => return reply_error(m, tcb_k, reply, 21),
            Err(FsError::IsDir) => return reply_error(m, tcb_k, reply, 22),
            Err(_) => return reply_error(m, tcb_k, reply, 23),
        };
        // 5) `exec` **nguyên tử** — lỗi ⇒ ảnh cũ nguyên vẹn (hợp đồng `KT-74`).
        match exec::exec_bytes(m, &buf[..bytes], cur, pid, untyped, tcb) {
            Ok(report) => {
                // 6) Trapframe của **lần chạy kế tiếp** = ảnh mới: `sepc = e_entry`, `sp` đỉnh
                //    stack user, mọi thanh ghi khác 0 (không rò ngữ cảnh cũ); TLS đã reset.
                *tf = TrapFrame::zeroed();
                tf.x[REG_SP] = USER_STACK_TOP;
                tf.sepc = report.new_entry;
                Outcome::Executed(Executed {
                    report,
                    path,
                    path_len: len as u8,
                    bytes,
                })
            }
            Err(error) => {
                let code = match error {
                    ExecError::NotFound => 2,
                    ExecError::Load(_) => 3,
                };
                reply_error(m, tcb_k, reply, code)
            }
        }
    }
}

/// Trả `EXEC_REPLY(err)` cho shell trên endpoint reply của procsrv (`EP_REPLY`, slot 4) — shell
/// đang `NBRecv` chờ trong `cmd_run`.
fn reply_error(m: &mut Machine, tcb_k: ObjId, reply: SlotRef, code: u64) -> Outcome {
    let info = match vifast_core::MessageInfo::new(EXEC_REPLY as u32, 0, false, 0, 1) {
        Ok(info) => info,
        Err(_) => return Outcome::Error,
    };
    let Ok(message) = vifast_core::Message::new(info, &[code]) else {
        return Outcome::Error;
    };
    let _ = m.nb_send(tcb_k, reply, message);
    Outcome::Failed(code)
}
