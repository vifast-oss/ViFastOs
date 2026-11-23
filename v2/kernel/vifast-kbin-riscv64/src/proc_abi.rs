// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-77b** — bố cục cspace + nhãn message **dùng chung** giữa ảnh test (`proc_demo`) và ảnh
//! sản phẩm có shell (`proc_rt`).
//!
//! # Vì sao tệp này tồn tại
//!
//! Ba chương trình U-mode (`procsrv`, `shell`, `child`) được biên dịch **riêng** (`build.rs`) và
//! hardcode số endpoint trong ELF của chính chúng (`init/procsrv_user.rs:29`, `init/shell_user.rs:45`);
//! kernel dựng cspace phải khớp **từng slot** thì `NBSend`/`NBRecv` mới tới đúng đích. Trước
//! KT-77b bảng slot đó chỉ nằm trong `proc_demo.rs` (ảnh test) nên ảnh sản phẩm phải chép lại —
//! hai nguồn dễ lệch. Tệp này là **một** nguồn cho phía kernel.
//!
//! Chương trình U-mode vẫn giữ bản sao hằng số (chúng không link crate kernel): ràng buộc "khớp
//! bảng" được kiểm bằng `v2/agent/JOURNAL/kt77b-harness-scan.sh` §slot (so số trong ELF với
//! `proc_abi`) — không phải bằng lời.
//!
//! Không logic, không `unsafe`, không cấp phát (R1/R5 không áp dụng; tệp chỉ khai báo hằng).

// Mỗi ảnh dùng một tập con (ảnh test dùng `SLOT_CHILD`/`EV_*`, ảnh sản phẩm không) —
// hằng ở đây là **hợp đồng** giữa kernel và chương trình U-mode, không phải code chết.
#![allow(dead_code)]

/// Ô chương trình U-mode trong `user::MAX_PROCS` (bảng dải VA per-slot, KT-65).
pub const SLOT_PROCSRV: usize = 0;
/// Ô của shell.
pub const SLOT_SHELL: usize = 1;
/// Ô của tiến trình con (chỉ ảnh test `ktest-proc` nạp; ảnh sản phẩm giữ ô trống).
pub const SLOT_CHILD: usize = 2;

/// Slot cspace của endpoint fault (core ghi `FaultKind` — D2-25/QD §1 Q-A).
pub const EP_FAULT: usize = 1;
/// Slot endpoint yêu cầu `wait`/`kill` → procsrv.
pub const EP_PROC: usize = 2;
/// Slot endpoint `LINE` của KT-65 (KT-73 bỏ đường này; giữ slot để bố cục không đổi).
pub const EP_UART: usize = 3;
/// Slot endpoint reply của procsrv.
pub const EP_REPLY: usize = 4;
/// Slot endpoint `PS_REQ` → kernel.
pub const EP_PS: usize = 5;
/// Slot cap `SchedControl` (không phải endpoint).
pub const EP_CTL: usize = 6;
/// Slot endpoint sự kiện procsrv → kernel (`EV_*`).
pub const EP_EVENTS: usize = 8;

/// Slot giữ cap `CNode` của chính cspace (shell/procsrv không dùng; bố cục cố định).
pub const SLOT_CSPACE: usize = 20;

/// Slot cap TCB của các tác vụ kernel dựng (theo thứ tự ảnh: procsrv, shell, …).
pub const TCB_SLOTS: [usize; 5] = [10, 11, 12, 13, 14];
/// Slot cap `SchedContext` tương ứng [`TCB_SLOTS`].
pub const SC_SLOTS: [usize; 5] = [30, 31, 32, 33, 34];

/// `PS_REQ` — shell yêu cầu bảng TCB; kernel trả `PS_REPLY` (đọc từ `Machine`, không bảng tĩnh).
pub const PS_REQ: u64 = 0x602;
/// Reply của kernel cho [`PS_REQ`].
pub const PS_REPLY: u64 = 0x603;
/// Sự kiện: procsrv vừa nhận một `FaultKind::Exit`.
pub const EV_FAULT: u64 = 0x502;
/// Sự kiện: kết quả `wait(pid)`.
pub const EV_WAIT: u64 = 0x503;
/// Sự kiện: kết quả `kill(pid)` — kernel thi hành `Tcb.Suspend(Abort)`.
pub const EV_KILL: u64 = 0x504;
/// Sự kiện: procsrv đã chạy (`# proc: up`).
pub const EV_UP: u64 = 0x501;
/// Sự kiện: shell đã in bảng `ps` (evidence).
pub const EV_PS: u64 = 0x505;
/// Sự kiện: shell đã chạy `echo` (evidence).
pub const EV_ECHO: u64 = 0x506;
/// Sự kiện: shell đã chạy `help` (evidence).
pub const EV_HELP: u64 = 0x507;
/// Label message kernel chuyển tiếp `FaultKind::Exit{code}` sang procsrv (cầu nối arch, nợ ABI).
pub const FAULT_EXIT: u64 = 0x608;

// ---------------------------------------------------------------------------------------------
// **KT-77c** — dịch vụ **FS của ảnh sản phẩm** (`proc_fs.rs`). Không mở ABI mới: request/reply đi
// trên `NB_SEND`/`NB_RECV` đã có, thêm **một** endpoint và **một** label. Bố cục MR ở
// `proc_fs.rs` (mỗi request = một reply 4 word).
// ---------------------------------------------------------------------------------------------

/// Slot endpoint `FS_REQ` → kernel (slot 7 trống từ KT-65; `EP_CTL` = 6 là cap, không phải ep).
pub const EP_FS: usize = 7;
/// Label request FS từ shell U-mode → kernel (shell gửi trên [`EP_FS`]).
pub const FS_REQ: u64 = 0x612;
/// Label reply FS từ kernel → shell (kernel gửi trên [`EP_REPLY`]).
pub const FS_REPLY: u64 = 0x613;

/// `OP_PATH` — nạp đường dẫn (byte trong 3 MR word tiếp theo) vào kernel trước thao tác.
pub const OP_PATH: u64 = 0x10;
/// `OP_LS` — liệt kê entry thứ `index` của thư mục đang nạp.
pub const OP_LS: u64 = 0x11;
/// `OP_CAT` — đọc 24 B tệp tại `offset`.
pub const OP_CAT: u64 = 0x12;
/// `OP_WRITE` — ghi `len` B (tạo tệp nếu chưa có).
pub const OP_WRITE: u64 = 0x13;
/// `OP_TRUNC` — cắt tệp về `size` (giữ `write` idempotent).
pub const OP_TRUNC: u64 = 0x14;
/// `OP_STAT` — kích thước/kiểu của đường dẫn đang nạp.
pub const OP_STAT: u64 = 0x15;
/// `OP_SYNC` — `Fs::fsync` + `flush` xuống phương tiện.
pub const OP_SYNC: u64 = 0x16;

// ---------------------------------------------------------------------------------------------
// **KT-343** — 11 lệnh `deferred-K7` (D2-239): thao tác FS còn thiếu đi trên **cùng** giao thức
// `FS_REQ`/`FS_REPLY` đã có của KT-77c (không mở ABI syscall, không thêm endpoint). `OP_PATH2`
// là **đường dẫn thứ hai** cho `mv`/`ln` — cùng bố cục 24 B/lần như `OP_PATH`; `mv`/`ln` đọc
// `path` (nguồn) + `path2` (đích), các op còn lại đọc `path`.
// ---------------------------------------------------------------------------------------------

/// **KT-343** — tạo thư mục (`Fs::mkdir`) trên đường dẫn đang nạp.
pub const OP_MKDIR: u64 = 0x17;
/// **KT-343** — xoá mục (`Fs::unlink`; thư mục phải rỗng).
pub const OP_RM: u64 = 0x18;
/// **KT-343** — đổi tên/di chuyển (`Fs::rename`): nguồn ở `path`, đích ở `path2`.
pub const OP_MV: u64 = 0x19;
/// **KT-343** — tạo liên kết cứng (`Fs::link`): nguồn ở `path`, đích ở `path2`.
pub const OP_LN: u64 = 0x1a;
/// **KT-343** — kiểm tra toàn vẹn FS (`Fs::fsck`).
pub const OP_FSCK: u64 = 0x1b;
/// **KT-343** — nạp **đường dẫn thứ hai** vào kernel; `len = 0` = bắt đầu mới.
pub const OP_PATH2: u64 = 0x1c;
/// **KT-345b** — số liệu volume cho `df` (`fsstat.vidl`): reply `[ST_OK, used, total, block_size]`.
pub const OP_STATFS: u64 = 0x1d;

/// Reply: thành công (`len`/`kind`/`ino` nằm ở byte cao của word 0).
pub const ST_OK: u64 = 0;
/// Reply: hết dữ liệu (hết entry của thư mục / hết tệp).
pub const ST_END: u64 = 1;
/// Reply: lỗi — mã lỗi ở byte 1 của word 0 ([`crate::proc_fs`] `err_code`).
pub const ST_ERR: u64 = 2;
/// **KT-123**: dịch vụ FS **không sẵn sàng** (ảnh có `product-fs` nhưng mount hỏng). Khác `ST_ERR`
/// (lỗi của một thao tác cụ thể) — shell in `fs khong san sang` thay vì `fs loi code=<n>`.
pub const ST_NOFS: u64 = 3;

// ---------------------------------------------------------------------------------------------
// **KT-342** — ảnh **sản phẩm** chạy **đồng thời** FS + `netsrv` + `exec` (D2-233/D2-243).
// Không mở ABI mới: ba đường đều đi trên `NB_SEND`/`NB_RECV`/`Call` + Endpoint đã có; thêm
// endpoint/slot cho **ảnh sản phẩm** (ảnh test giữ nguyên bố cục của nó ở `ktest_netsrv`).
// ---------------------------------------------------------------------------------------------

/// **KT-342** — `Endpoint` `Ping(ip)` shell → `netsrv` U-mode (ô 9 trống từ KT-65).
pub const EP_NET: usize = 9;
/// **KT-342** — `Endpoint` `netsrv` trả lời shell (ô 15 trống).
pub const EP_NET_REPLY: usize = 15;
/// **KT-342** — `Endpoint` `netsrv` → dịch vụ mạng của **kernel** (`Call`).
pub const EP_NET_KERN: usize = 16;
/// **KT-342** — `Reply` cap của kernel cho `Call` từ `netsrv`.
pub const EP_NET_KERN_REPLY: usize = 17;
/// **KT-342** — `Endpoint` `run <path>` shell → kernel (exec kernel-mediated).
pub const EP_EXEC: usize = 18;

/// **KT-342** — label `Ping(ip)` shell → `netsrv` (hợp đồng nội ảnh, **không** phải ABI).
pub const NET_REQ: u64 = 0x290;
/// **KT-342** — label `Reply(code)` `netsrv` → shell.
pub const NET_REPLY: u64 = 0x292;
/// **KT-342** — label `Exec(path)` shell → kernel.
pub const EXEC_REQ: u64 = 0x620;
/// **KT-342** — label reply lỗi `exec` kernel → shell (thành công thì ảnh shell bị thay).
pub const EXEC_REPLY: u64 = 0x621;

/// **KT-342** — ô chương trình `netsrv` U-mode của ảnh sản phẩm (`user::MAX_PROCS` = 6; ô 2
/// không dùng cho ảnh sản phẩm — ảnh test `ktest-proc` dùng `SLOT_CHILD`).
pub const SLOT_NETSRV: usize = 2;

// ---------------------------------------------------------------------------------------------
// **KT-345b** — `free` (`meminfo.vidl` 4C), `df` (`fsstat.vidl` 4C) và `loop` (D2-252).
// Cùng nguyên tắc KT-343/KT-342: **không** mở ABI syscall, không thêm method vào `abi.toml`;
// ba đường đi trên `NB_SEND`/`NB_RECV`/`Call` đã có.
// ---------------------------------------------------------------------------------------------

/// **KT-345b** — `Endpoint` dịch vụ bộ nhớ của kernel (`MemSvc`, `meminfo.vidl`).
/// Ô 21 trống (bố cục cũ dùng 0..18 + 20).
pub const EP_MEM: usize = 21;
/// **KT-345b** — `Endpoint` dịch vụ `loop` của kernel (spawn/destroy TCB n lần).
/// Ô 22 trống.
pub const EP_LOOP: usize = 22;
/// **KT-345b** — ô cap `Untyped` 4 KiB riêng cho `loop` (spawn/destroy, `Revoke` sau mỗi vòng).
pub const LOOP_ARENA: usize = 23;
/// **KT-345b** — `Endpoint` client thứ hai (`procsrv`) → `netsrv` (ô 26 trống) — 6A đa caller.
pub const EP_NET_CLIENT: usize = 26;
/// **KT-345b** — `Endpoint` `netsrv` trả lời client thứ hai (ô 27 trống).
pub const EP_NET_CLIENT_REPLY: usize = 27;
/// **KT-345b** — hai ô nháp cho TCB/SchedContext của từng vòng `loop` (xoá sau mỗi vòng).
pub const LOOP_TCB_SLOT: usize = 24;
/// **KT-345b** — ô nháp `SchedContext` của từng vòng `loop`.
pub const LOOP_SC_SLOT: usize = 25;

/// **KT-345b** — label request `MemInfo(op)` shell → kernel (`meminfo.vidl`: 0 = `total`, 1 = `free`).
pub const MEM_REQ: u64 = 0x630;
/// **KT-345b** — label reply `MemInfo(value)` kernel → shell.
pub const MEM_REPLY: u64 = 0x631;
/// **KT-345b** — label request `Loop(n)` shell → kernel.
pub const LOOP_REQ: u64 = 0x634;
/// **KT-345b** — label reply `Loop(ok, before, after)` kernel → shell.
pub const LOOP_REPLY: u64 = 0x635;

/// **KT-345b** — `meminfo.total`: tổng byte bộ nhớ do object layer quản lý (mọi Untyped sống).
pub const MEM_TOTAL: u64 = 0;
/// **KT-345b** — `meminfo.free`: byte còn cấp được (tổng − watermark đã dùng).
pub const MEM_FREE: u64 = 1;
/// **KT-345b** — số vòng tối đa của `loop <n>` (chặn trên, cùng ngưỡng v1 `≤32`).
pub const LOOP_MAX: u64 = 32;
