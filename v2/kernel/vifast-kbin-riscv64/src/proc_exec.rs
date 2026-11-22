// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-342** — `run <path>` trên ảnh **SẢN PHẨM** riscv64 (`product-exec`): **exec
//! kernel-mediated** (D2-96, cùng khuôn `KT-74`), dùng **bảng ảnh nhúng** của ảnh sản phẩm.
//!
//! # Vì sao exec là kernel-mediated mà không phải syscall
//!
//! `spec/abi.toml` đóng băng 16 syscall (`0..15`) và `[object].Tcb` **không** có method `Exec`
//! (D2-15/D2-74); luật vòng này cấm sửa `spec/` (D2-04). Quyết định *exec tiến trình nào sang
//! ảnh nào* là **policy** (D2-96) — kernel thi hành, shell chỉ gửi yêu cầu trên `Endpoint`
//! `EP_EXEC` bằng `NBSend` (đường IPC đã có, **không** mở ABI).
//!
//! # Vì sao bảng ảnh nhúng, không đọc `/hello.elf` từ FS
//!
//! Cơ chế `exec` hiện có (`vifast_plat_riscv64::exec`) nhận **bảng ảnh tĩnh** (`ImageTable`) —
//! nợ "path phải đọc từ FS" đã ghi ở `docs/k6-exec.md` §6 và là việc của `KT-343`/`KT-346`. Ở
//! `KT-342`, tiêu chí là **một** boot sản phẩm chạy **đồng thời** FS + `netsrv` + `exec`; đường
//! exec dùng đúng cơ chế `KT-74` đã đo (nguyên tử, giữ pid/cspace, giải phóng VSpace cũ), với
//! `'/hello.elf'` là khoá trong bảng nhúng.
//!
//! # Bằng chứng (ảnh sản phẩm không có TAP — D2-08)
//!
//! Kernel in **một** dòng `# product exec: path=… old_entry=… new_entry=… frames=… objects=…
//! caps=… check=1` (số đo từ `ExecReport`); ảnh đích in `RUN OK` + `# hello: run ok` rồi
//! `Exit(0)`. Lỗi ⇒ kernel trả `EXEC_REPLY` và shell in `# shell: run err=<n>` (không treo).

use vifast_core::{Machine, ObjId, SlotRef};
use vifast_plat_common::obj::CapId;
use vifast_plat_common::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::exec::{self, ExecError, ExecReport, ImageTable};
use vifast_plat_riscv64::user::{LoadedInit, USER_STACK_TOP};

use crate::proc_abi::{EP_EXEC, EXEC_REPLY, EXEC_REQ};

/// Ảnh đích của `run /hello.elf` — do `build.rs` sinh khi bật `product-exec`.
static HELLO_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/hello.elf"));
/// Khoá của ảnh trong bảng `exec` (đường dẫn **logic**, chưa đọc FS — nợ `docs/k6-exec.md` §6).
pub const HELLO_PATH: &str = "/hello.elf";
/// Số byte đường dẫn tối đa mang trong **hai** MR word (8 + 8).
const MAX_PATH: usize = 16;

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

/// Bảng ảnh của ảnh sản phẩm — dựng lại mỗi lần phục vụ (192 B trên stack, **không** `static`
/// `mut`, không cấp phát).
fn table() -> ImageTable {
    let mut table = ImageTable::new();
    assert!(table.add(HELLO_PATH, HELLO_ELF), "KT-342: bang anh");
    table
}

/// Kết quả một lượt phục vụ `EP_EXEC`.
pub enum Outcome {
    /// Không có yêu cầu (endpoint rỗng) — vòng sau thử lại.
    Empty,
    /// Đã `exec` thành công: ảnh shell đã bị thay; `report` là số đo.
    Executed(ExecReport),
    /// Từ chối/lỗi — kernel đã trả `EXEC_REPLY(EXEC_REPLY, err)` cho shell.
    Failed(u64),
    /// Lỗi IPC nội bộ (không phục vụ được).
    Error,
}

/// Rút **một** yêu cầu `run` đang chờ trên `EP_EXEC` (không chặn), kiểm dạng rồi `exec`.
///
/// `cur` là ảnh hiện tại của **ô shell** (`pid`), `tf` là trapframe sẽ chạy tiếp: thành công ⇒
/// trapframe được thay bằng entry/`sp` của ảnh mới (mọi thanh ghi khác = 0, TLS = 0 — đúng hợp
/// đồng `exec` của `KT-74`).
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
    if label != EXEC_REQ || len == 0 || len > MAX_PATH || request.info.length() < 2 {
        return reply_error(m, tcb_k, reply, 5);
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
    let Ok(path) = core::str::from_utf8(&path[..len]) else {
        return reply_error(m, tcb_k, reply, 5);
    };
    // 4) `exec` **nguyên tử** — lỗi ⇒ ảnh cũ nguyên vẹn (hợp đồng `KT-74`).
    let images = self::table();
    match exec::exec(m, &images, cur, pid, path, untyped, tcb) {
        Ok(report) => {
            // 5) Trapframe của **lần chạy kế tiếp** = ảnh mới: `sepc = e_entry`, `sp` đỉnh stack
            //    user, mọi thanh ghi khác 0 (không rò ngữ cảnh cũ); TLS đã reset trong `exec`.
            *tf = TrapFrame::zeroed();
            tf.x[REG_SP] = USER_STACK_TOP;
            tf.sepc = report.new_entry;
            Outcome::Executed(report)
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
