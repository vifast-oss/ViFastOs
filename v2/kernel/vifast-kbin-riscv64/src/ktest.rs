// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ảnh **test** riscv64 — **một** bin `vifast-kbin-ktest`, nhiều **bộ ca** chọn bằng feature
//! `suite-*`. Nạp qua OpenSBI; D2-08: ảnh test **không** nằm trong ảnh sản phẩm.
//!
//! **KT-411b** (D2-259 mục 2) — trước đây crate này có **14 `[[bin]]`** (1 ảnh sản phẩm + 13 ảnh
//! kiểm), mỗi ảnh một crate root riêng lặp lại `#![no_std]`/`#![no_main]`/`rust_main`/
//! `rust_secondary_main`/`#[panic_handler]`. Nay còn **2 `[[bin]]`**:
//!
//! * `vifast-kbin-riscv64` — ảnh **sản phẩm** (`src/main.rs`, không đổi một dòng);
//! * `vifast-kbin-ktest` — ảnh **kiểm**, chọn bộ ca bằng **đúng một** feature `suite-*`.
//!
//! Tệp này chỉ còn là **dispatcher**: `rust_main`/`rust_secondary_main`/`#[panic_handler]` **một
//! bản duy nhất**; thân từng bộ ca nằm trong `ktest_<suite>.rs` với `pub fn run() -> !`.
//!
//! Bất biến **không đổi** (D2-08, D2-109, D2-111): ảnh sản phẩm không link harness nào; mỗi ảnh
//! kiểm vẫn là **một** ảnh riêng với `.text`/`.data+.bss`/TAP như trước — harness `cp` ảnh ra
//! đúng **tên cũ** nên `expected.txt`, `image_path`, `budget.sh` không đổi một dòng.
//!
//! Mỗi lần build **đúng một** `suite-*`; bật hai suite là lỗi biên dịch (xem `const _` bên dưới).
#![no_std]
#![no_main]

// ---------------------------------------------------------------- bộ ca (mỗi lần build đúng 1)
// **KT-411b**: `#[path]` giữ nguyên vị trí tệp (`src/<tên>.rs`) — module nạp bằng `#[path]` phân
// giải module con theo **thư mục của tệp**, nên `ktest_vfs` vẫn thấy `ktest_vfs_once`/`ktest_vfs_rig`
// và `ktest_ipc` vẫn thấy `rig` như trước khi gom bin.
#[cfg(feature = "suite-base")]
#[path = "ktest_base.rs"]
mod ktest_base;
#[cfg(feature = "suite-base")]
#[path = "ktest_conf.rs"]
mod ktest_conf;
#[cfg(feature = "suite-base")]
#[path = "ktest_core.rs"]
mod ktest_core;
#[cfg(feature = "suite-base")]
#[path = "ktest_core64.rs"]
mod ktest_core64;
#[cfg(feature = "suite-base")]
#[path = "ktest_fault.rs"]
mod ktest_fault;
#[cfg(feature = "suite-base")]
#[path = "ktest_fs.rs"]
mod ktest_fs;
#[cfg(feature = "suite-base")]
#[path = "ktest_ipi.rs"]
mod ktest_ipi;
#[cfg(feature = "suite-base")]
#[path = "ktest_lock.rs"]
mod ktest_lock;
#[cfg(feature = "suite-base")]
#[path = "ktest_mem.rs"]
mod ktest_mem;
#[cfg(feature = "suite-base")]
#[path = "ktest_obj.rs"]
mod ktest_obj;
#[cfg(feature = "suite-base")]
#[path = "ktest_smp.rs"]
mod ktest_smp;
#[cfg(feature = "suite-base")]
#[path = "ktest_tlb.rs"]
mod ktest_tlb;

#[cfg(feature = "suite-guard")]
#[path = "ktest_guard.rs"]
mod ktest_guard;

#[cfg(feature = "suite-user")]
#[path = "ktest_capa.rs"]
mod ktest_capa;
#[cfg(feature = "suite-user")]
#[path = "ktest_capx.rs"]
mod ktest_capx;
#[cfg(feature = "suite-user")]
#[path = "ktest_inv.rs"]
mod ktest_inv;
#[cfg(feature = "suite-user")]
#[path = "ktest_ring.rs"]
mod ktest_ring;
#[cfg(feature = "suite-user")]
#[path = "ktest_user.rs"]
mod ktest_user;
#[cfg(feature = "suite-user")]
#[path = "ktest_vfs.rs"]
mod ktest_vfs;

#[cfg(feature = "suite-proc")]
#[path = "ktest_proc.rs"]
mod ktest_proc;
#[cfg(feature = "suite-proc")]
#[path = "proc_abi.rs"]
mod proc_abi;
#[cfg(feature = "suite-proc")]
#[path = "proc_demo.rs"]
mod proc_demo;

#[cfg(feature = "suite-vblk")]
#[path = "ktest_vblk.rs"]
mod ktest_vblk;

#[cfg(feature = "suite-persist")]
#[path = "ktest_persist.rs"]
mod ktest_persist;

#[cfg(feature = "suite-con")]
#[path = "ktest_con.rs"]
mod ktest_con;

#[cfg(feature = "suite-exec")]
#[path = "ktest_exec.rs"]
mod ktest_exec;

#[cfg(feature = "suite-ipc")]
#[path = "ktest_ipc.rs"]
mod ktest_ipc;

#[cfg(feature = "suite-net")]
#[path = "ktest_net.rs"]
mod ktest_net;

#[cfg(feature = "suite-netsrv")]
#[path = "ktest_netsrv.rs"]
mod ktest_netsrv;
#[cfg(feature = "suite-netsrv")]
#[path = "net_svc.rs"]
mod net_svc;

#[cfg(feature = "suite-input")]
#[path = "ktest_input.rs"]
mod ktest_input;

#[cfg(feature = "suite-gpu")]
#[path = "ktest_gpu.rs"]
mod ktest_gpu;

use vifast_plat_common::PanicHandler;
use vifast_plat_riscv64::Riscv64Virt;

/// Đúng **một** `suite-*` mỗi lần build — bật 0 hoặc ≥ 2 là lỗi biên dịch, không phải ảnh lai
/// (fail-closed: ảnh kiểm phải chứng minh được nó chạy **bộ ca nào**).
const _: () = {
    let n = cfg!(feature = "suite-base") as u8
        + cfg!(feature = "suite-guard") as u8
        + cfg!(feature = "suite-user") as u8
        + cfg!(feature = "suite-proc") as u8
        + cfg!(feature = "suite-vblk") as u8
        + cfg!(feature = "suite-persist") as u8
        + cfg!(feature = "suite-con") as u8
        + cfg!(feature = "suite-exec") as u8
        + cfg!(feature = "suite-ipc") as u8
        + cfg!(feature = "suite-net") as u8
        + cfg!(feature = "suite-netsrv") as u8
        + cfg!(feature = "suite-input") as u8
        + cfg!(feature = "suite-gpu") as u8;
    assert!(n == 1, "phải bật đúng MỘT feature `suite-*`");
};

static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Entry hart **boot** — OpenSBI nhảy vào `_start` rồi `call rust_main` (xem `plat-riscv64`).
///
/// **KT-411b**: chỉ chọn bộ ca rồi giao lại; **không** in gì trước khi vào suite (TAP của từng
/// ảnh bắt đầu bằng `TAP version 13` do chính suite in, giữ nguyên thứ tự byte).
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    #[cfg(feature = "suite-base")]
    {
        ktest_base::run()
    }
    #[cfg(feature = "suite-guard")]
    {
        ktest_guard::run()
    }
    #[cfg(feature = "suite-user")]
    {
        ktest_user::run()
    }
    #[cfg(feature = "suite-proc")]
    {
        ktest_proc::run()
    }
    #[cfg(feature = "suite-vblk")]
    {
        ktest_vblk::run()
    }
    #[cfg(feature = "suite-persist")]
    {
        ktest_persist::run()
    }
    #[cfg(feature = "suite-con")]
    {
        ktest_con::run()
    }
    #[cfg(feature = "suite-exec")]
    {
        ktest_exec::run()
    }
    #[cfg(feature = "suite-ipc")]
    {
        ktest_ipc::run()
    }
    #[cfg(feature = "suite-net")]
    {
        ktest_net::run()
    }
    #[cfg(feature = "suite-netsrv")]
    {
        ktest_netsrv::run()
    }
    #[cfg(feature = "suite-input")]
    {
        ktest_input::run()
    }
    #[cfg(feature = "suite-gpu")]
    {
        ktest_gpu::run()
    }
}

/// Entry hart **thứ cấp** (SBI HSM gọi `_start_secondary` với `a0 = hartid`, `a1 = opaque`).
///
/// **KT-411b**: suite `base` có chuỗi pha SMP riêng (`ktest_smp::secondary_main` — barrier, timer,
/// PLIC, IPI, TLB); 12 suite còn lại chỉ cần symbol để link nên adopt `satp`/`stvec` của BSP rồi
/// park (đúng hành vi cũ của từng ảnh).
#[no_mangle]
pub extern "C" fn rust_secondary_main(hart: usize, opaque: usize) -> ! {
    #[cfg(feature = "suite-base")]
    {
        ktest_smp::secondary_main(hart, opaque)
    }
    #[cfg(not(feature = "suite-base"))]
    {
        let _ = hart;
        let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
        vifast_plat_riscv64::park_forever()
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
