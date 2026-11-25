// SPDX-License-Identifier: MIT OR Apache-2.0
// **KT-404** (`D2-259`) — crate này **0 `unsafe`** ⇒ khoá bằng máy: mọi `unsafe`
// thêm vào (kể cả trong macro) là lỗi biên dịch, không chỉ là luật cấu trúc R5.
#![forbid(unsafe_code)]
//! **KT-347 (U1a)** — `vifast-rt`: runtime `no_std + alloc` cho chương trình U-mode của
//! ViFastOs, biên dịch bằng **stable 1.100.0** (toolchain ghim `v2/rust-toolchain.toml`).
//!
//! # Vì sao có crate này (và vì sao ở `v2/user/rt/`)
//!
//! Trước KT-347, mỗi chương trình U-mode (`shell`, `hello`, `procsrv`, …) tự viết `_start`,
//! `#[panic_handler]` và **không** có `alloc` — `hello.rs` 33 dòng chỉ gọi 3 syscall. D2-252
//! Pha A đặt userland sản phẩm vào cây `v2/user/`; KT-362 (`U1a-Full`) sẽ cần một runtime có
//! `Vec`/`String`/`println!` để port `ViShell`. Task này là **PoC** của runtime đó:
//!
//! | Thành phần | Ở đâu | Ghi chú |
//! |---|---|---|
//! | `alloc` + `GlobalAlloc` | `v2/user/runtime/lib.rs` (`mod heap`, `#[cfg(rt_alloc)]`) | phần `unsafe` nằm trong danh sách đóng D2-17 (D2-253); lõi bump **an toàn** ở `v2/user/runtime/bump.rs` (test host) |
//! | `main` entry | [`entry`] | `init_ipc_buffer` → `main()` → `Exit(0)` — cùng hợp đồng `_start` của `hello.rs` |
//! | `println!`/`print!` | [`_print`] + macro | đi syscall `DebugPutStr` (12) đã có, **không** mở ABI |
//! | panic handler | [`panic`] | fail-closed: in `# u1a: panic …` rồi `Exit(98)` — không treo |
//! | OOM | `heap::fail_named` | fail-closed: in `# u1a: alloc fail …` rồi `Exit(95)` — **có tên**, không panic mù |
//!
//! **Vì sao không đặt trong `v2/user/runtime/`**: shim ABI là crate `vifast_user_abi` mà **mọi**
//! chương trình U-mode link (kể cả `hello.elf` 33 dòng); nhét `alloc`/`main`/`panic` vào đó sẽ
//! làm mọi ảnh cũ phình to và trùng `#[panic_handler]`. Runtime tách riêng (`vifast-rt`) chỉ được
//! link khi chương trình **chọn** dùng — `u1a.elf` của PoC là chương trình đầu tiên.
//!
//! # Bộ nhớ: arena tĩnh, chưa phải `Untyped` (nói thẳng)
//!
//! U-mode hiện **chưa** có cap `Untyped`/`Frame` để tự map trang (đường cấp phát của kernel là
//! `Untyped.Retype`, chưa mở cho U-mode). Vì vậy PoC cấp phát trên **arena tĩnh** 8 KiB trong
//! `.bss` của chương trình (`heap::HEAP_BYTES`); API [`heap_stats`] trả `(used, cap)` để chương
//! trình tự in bằng chứng. Khi `Tcb.Exec`/`Frame.Map` mở cho U-mode (KT-362), thay lõi bump bằng
//! allocator trên Untyped — **hợp đồng API của crate này không đổi**.
//!
//! # Trạng thái
//!
//! | | |
//! |---|---|
//! | Toolchain | **stable 1.100.0**, `-C panic=abort`, target `riscv64gc-unknown-none-elf` |
//! | Dùng `alloc` | `Vec`, `String`, `format!` (qua `#[global_allocator]` của chương trình) |
//! | `unsafe` | **không có** trong tệp này; chỉ ở `v2/user/runtime/lib.rs` `mod heap` |
//! | Đích chạy | ảnh sản phẩm riscv64: `run /u1a.elf` — **exec nạp ELF từ FS** (KT-347 nợ a/b/c) |

#![no_std]

extern crate alloc;

pub use vifast_user_abi::heap::{heap_stats, BumpAlloc, HEAP_BYTES};

use core::fmt::{self, Write};

use vifast_user_abi::{exit, init_ipc_buffer, put};

/// Đích của `core::fmt` — in qua syscall `DebugPutStr` (12), **không** mở ABI mới.
struct Sink;

impl Write for Sink {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        put(text.as_bytes());
        Ok(())
    }
}

/// In một `core::fmt::Arguments` (nền của [`print!`]/[`println!`]). Không cấp phát.
pub fn _print(args: fmt::Arguments<'_>) {
    let _ = Sink.write_fmt(args);
}

/// `print!("…")` — không cấp phát, không panic; in qua `DebugPutStr` (12).
///
/// Mảnh macro là **`expr`** (không phải `tt` passthrough): checker `check-wake-paths.py`
/// (KT-194/KT-202) cấm `$…:tt` trong thân macro vì nó có thể chở `mod`/`include!`/`#[path]`
/// từ lời gọi; `expr` không chở được item nên giữ đúng bất biến.
#[macro_export]
macro_rules! print {
    ($($arg:expr),+ $(,)?) => {{
        $crate::_print(::core::format_args!($($arg),+));
    }};
}

/// `println!("…")` — như [`print!`] rồi thêm `\n`.
#[macro_export]
macro_rules! println {
    () => {{
        $crate::_print(::core::format_args!("\n"));
    }};
    ($($arg:expr),+ $(,)?) => {{
        $crate::_print(::core::format_args!("{}\n", ::core::format_args!($($arg),+)));
    }};
}

/// `main` entry của chương trình `vifast-rt`: dựng IPC buffer, chạy `main`, rồi `Exit(0)`.
///
/// Cùng thứ tự với `_start` của các chương trình cũ (`hello.rs`): buffer IPC phải có **trước**
/// khi chương trình gọi syscall IPC đầu tiên; `Exit(0)` để kernel thấy `shell_exit=0` sau `exec`.
pub fn entry(main: fn()) -> ! {
    init_ipc_buffer();
    main();
    exit(0)
}

/// Panic handler **duy nhất** của chương trình dùng `vifast-rt` (D2-05: bỏ nó ⇒ không link).
///
/// Fail-closed: in một dòng **có tên** (`# u1a: panic …`) rồi `Exit(98)` — không bỏ qua, không
/// treo, không `loop {}`. Ô exit khác 0 để kernel/harness thấy ảnh chết có kiểm soát.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let _ = writeln!(&mut Sink, "# u1a: panic {} ", info);
    exit(98)
}
