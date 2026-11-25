// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh x86_64 (KT-257) + khai báo artefact U-mode dùng chung (KT-341).
//!
//! Cùng khuôn `vifast-kbin-aarch64/build.rs` (KT-339): chương trình U-mode `shell.rs`/`procsrv.rs`
//! là **cùng một nguồn Rust** (`v2/user/programs/`) cho cả ba arch; tệp này khai báo chúng cho
//! `x86_64-unknown-none` và nhúng bằng `include_bytes!` vào `vifast-kbin-x86_64/src/shellpath.rs`.
//! Chỉ build khi cần: feature `shell`/`product-shell`/`ktest` (ảnh sản phẩm mặc định không kéo
//! parser/ELF/chương trình U-mode nào — D2-08 vẫn tách hai artefact). `--gc-sections` cần vì
//! `global_asm!` đặt mỗi đoạn trong section riêng; linker script `KEEP` đúng ba thứ không ai tham
//! chiếu từ Rust: `.multiboot`, `.note.Xen`, `.text.entry`.
//!
//! # KT-404 / KT-404c — build script này chỉ còn **khai báo**
//!
//! Máy móc dùng chung ở crate [`vifast_kbuild`]; tệp này chỉ còn **dữ liệu của arch này**
//! ([`PROGRAMS`], [`LINKER`]).

use std::path::{Path, PathBuf};

use vifast_kbuild::cargo;
use vifast_kbuild::compile::LinkerScript;
use vifast_kbuild::identity;
use vifast_kbuild::user::{self, Build, Link, Program, Rlib};

/// Target của ảnh này (một chỗ duy nhất — trước KT-404 chuỗi này lặp 4 lần trong tệp).
const TARGET: &str = "x86_64-unknown-none";

/// Điểm vào liên kết của **từng** chương trình U-mode (VA tuyệt đối, `ET_EXEC`).
///
/// Hai ảnh cách nhau 2 MiB — ảnh lớn nhất hiện tại (`shell`) dưới 16 KiB, một segment mọc thêm
/// không thể đè ảnh kế. Cùng cửa sổ `userload::USER_WINDOW_BASE..USER_WINDOW_END` (0x7000_0000).
const PROGRAMS: &[Program<'_>] = &[
    Program::new("shell.rs", "shell.elf", 0).at(0x7000_0000),
    // **KT-341b** — `procsrv` cùng nguồn `v2/user/programs/procsrv.rs` (riscv64/aarch64 dùng
    // chung): trả lời `wait`/`kill` trên `EP_PROC`, báo `EV_KILL` cho kernel thi hành
    // `Tcb.Suspend`. Cách `shell` 2 MiB (`userload::USER_WINDOW_BASE..END`).
    Program::new("procsrv.rs", "procsrv.elf", 0).at(0x7020_0000),
];

/// Mẫu linker script của arch này (phần khác nhau giữa 3 arch nằm gọn ở đây).
const LINKER: LinkerScript<'static> = LinkerScript {
    output_arch: "i386:x86-64",
    // x86_64 cần `.got`/`.got.plt` trong segment **chỉ đọc** (không có `.ARM.attributes`).
    rodata_extra: " *(.got) *(.got.plt)",
    discard: "*(.eh_frame) *(.comment) *(.note.GNU-stack)",
    note: "KT-341",
};

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // **KT-346** — danh tính bản dựng (dòng `# build:`) cho ảnh sản phẩm arch này; cùng khuôn
    // `vifast-kbin-riscv64/build.rs` (KT-86/D2-125.3) — đóng nợ P20 "chỉ riscv64 có danh tính".
    identity::build_identity(&dir, "vifast-kbin-x86_64");
    // `-no-pie` là cờ của **ảnh kernel** (không dùng `-T`/link script user ở đây).
    cargo::link_kernel(&dir, &["-no-pie"]);
    build_user_programs(Path::new(&dir));
}

fn build_user_programs(dir: &Path) {
    if !(cargo::feature("SHELL") || cargo::feature("PRODUCT_SHELL") || cargo::feature("KTEST")) {
        return;
    }
    if !cargo::target_is(TARGET) {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let programs = dir.join("../../user/programs");
    let shim_src = dir.join("../../user/runtime/lib.rs");

    // ---- 1) shim ABI (`vifast_user_abi`) — backend x86_64 (`int 0x80`).
    // ---- 2) ring KT-69 + codec `conchan` của virtio (shell đọc ring UART).
    let libs = user::rlibs(
        dir,
        TARGET,
        &[
            Rlib::new("vifast_user_abi", "../../user/runtime/lib.rs", &[]),
            Rlib::new("vifast_ring", "../../lib/vifast-ring/src/lib.rs", &[]),
            Rlib::new("vifast_virtio", "../../lib/vifast-virtio/src/lib.rs", &[]),
        ],
    );
    let p = |i: usize| libs[i].as_path();
    let set = [
        ("vifast_user_abi", p(0)),
        ("vifast_ring", p(1)),
        ("vifast_virtio", p(2)),
    ];
    let sets: [&[(&str, &Path)]; 1] = [&set];

    // ---- 3) từng chương trình U-mode, mỗi chương trình một linker script sinh trong OUT_DIR.
    user::build(&Build {
        target: TARGET,
        cwd: &out_dir,
        src_dir: &programs,
        link: Link::Generated(LINKER),
        extra: &[],
        link_args: &[],
        sets: &sets,
        programs: PROGRAMS,
    });
    // **KT-401** — `shell.rs` tách tệp (R1): theo dõi cả ba tệp nguồn cùng ELF.
    cargo::rerun_if_changed(&[
        "../../user/programs/shell_cmds.rs",
        "../../user/programs/shell_fs.rs",
        "../../user/programs/shell_net.rs",
    ]);
    println!("cargo:rerun-if-changed={}", shim_src.display());
    cargo::rerun_if_changed(&[
        "../../lib/vifast-ring/src/lib.rs",
        "../../lib/vifast-virtio/src/lib.rs",
        "build.rs",
    ]);
}
