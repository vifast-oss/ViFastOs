// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh aarch64 (KT-247) + khai báo artefact U-mode dùng chung (KT-339).
//!
//! `procsrv`/`shell` là **cùng một nguồn Rust** (`v2/user/programs/`) cho cả riscv64 và aarch64
//! (D2-233 mục 3); tệp này khai báo chúng cho `aarch64-unknown-none`, thêm `echo.rs` (chương
//! trình kiểm IPC buffer của TAP 44). Chỉ ảnh **kiểm** (`--features ktest`) và ảnh sản phẩm có
//! shell (`--features product-shell`, KT-339a) mới cần: ảnh sản phẩm tối giản **không** link
//! `vifast-core` (trần `.data+.bss` 65.536 B < `size_of::<Machine>()`).
//!
//! # KT-404 / KT-404c — build script này chỉ còn **khai báo**
//!
//! Máy móc dùng chung ở crate [`vifast_kbuild`]; tệp này chỉ còn **dữ liệu của arch này**
//! ([`PROGRAMS`], [`LINKER`], [`LINK_ARGS`]).
//!
//! # Đột biến (bắt buộc ĐỎ)
//!
//! - `mut-info-x1` → `--cfg mut_info_x1` cho shim: `MessageInfo` đi `x1` thay `x2` (giả riscv64).
//! - `mut-no-x0-out` → `--cfg mut_no_x0_out` cho shim: stub **không** ghi lại `x0` (error).

use std::path::{Path, PathBuf};

use vifast_kbuild::cargo;
use vifast_kbuild::compile::LinkerScript;
use vifast_kbuild::identity;
use vifast_kbuild::user::{self, Build, Link, Program, Rlib, Set};

/// Target của ảnh này (một chỗ duy nhất — trước KT-404 chuỗi này lặp 4 lần trong tệp).
const TARGET: &str = "aarch64-unknown-none";

/// Nhóm `--extern`: chỉ shim ABI, hoặc shim + ring + codec `conchan` (shell đọc ring UART).
const SET_SHIM: Set = 0;
const SET_SHELL: Set = 1;

/// Điểm vào liên kết của **từng** chương trình U-mode (VA tuyệt đối, `ET_EXEC`).
///
/// Ba mốc cách nhau 2 MiB: ảnh lớn nhất hiện tại (`shell`) dưới 8 KiB, cách nhau 2 MiB để một
/// segment mọc thêm không thể đè ảnh kế, và tất cả nằm trong cửa sổ
/// `vifast_plat_aarch64::userload::USER_WINDOW_BASE..USER_WINDOW_END`.
const PROGRAMS: &[Program<'_>] = &[
    Program::new("procsrv.rs", "procsrv.elf", SET_SHIM).at(0x7100_0000),
    Program::new("shell.rs", "shell.elf", SET_SHELL).at(0x7120_0000),
    Program::new("echo.rs", "echo.elf", SET_SHIM).at(0x7140_0000),
];

/// Mẫu linker script của arch này (phần khác nhau giữa 3 arch nằm gọn ở đây).
const LINKER: LinkerScript<'static> = LinkerScript {
    output_arch: "aarch64",
    rodata_extra: "",
    discard: "*(.eh_frame) *(.comment) *(.ARM.attributes)",
    note: "KT-339",
};

/// Cờ `-C link-arg=…` riêng của arch: LLD mặc định `p_align = 64 KiB` cho aarch64;
/// `vifast-elf::parse` chỉ nhận `align` ≤ 4 KiB (một Frame) ⇒ ép trang 4 KiB.
const LINK_ARGS: [&str; 1] = ["-zmax-page-size=0x1000"];

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // **KT-346** — danh tính bản dựng (dòng `# build:`) cho ảnh sản phẩm arch này; cùng khuôn
    // `vifast-kbin-riscv64/build.rs` (KT-86/D2-125.3) — đóng nợ P20 "chỉ riscv64 có danh tính".
    identity::build_identity(&dir, "vifast-kbin-aarch64");
    cargo::link_kernel(&dir, &[]);
    cargo::rerun_if_features(&[
        "MUT_INFO_X1",
        "MUT_NO_X0_OUT",
        "PRODUCT_SHELL",
        "MUT_NO_SVC",
    ]);
    build_user_programs(Path::new(&dir));
}

fn build_user_programs(dir: &Path) {
    // Ảnh sản phẩm tối giản: không K5 (D2-236/D2-244) ⇒ không sinh ELF user nào.
    if !cargo::feature("KTEST") && !cargo::feature("PRODUCT_SHELL") {
        return;
    }
    if !cargo::target_is(TARGET) {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let programs = dir.join("../../user/programs");
    let shim_src = dir.join("../../user/runtime/lib.rs");

    // Cờ `--cfg` của đột biến — **chỉ** có khi feature bật; kèm `--check-cfg` để bản không đột
    // biến không cảnh báo "unexpected cfg".
    let cfgs = cargo::cfgs(&[
        ("shell_fs", None),
        ("mut_info_x1", Some(cargo::feature("MUT_INFO_X1"))),
        ("mut_no_x0_out", Some(cargo::feature("MUT_NO_X0_OUT"))),
    ]);
    let cfgs: Vec<&str> = cfgs.iter().map(String::as_str).collect();

    // ---- 1) shim ABI (`vifast_user_abi`) — `rlib` cho target này.
    // ---- 2) ring KT-69 + codec `conchan` của virtio (shell đọc ring UART).
    let libs = user::rlibs(
        dir,
        TARGET,
        &[
            Rlib::new("vifast_user_abi", "../../user/runtime/lib.rs", &[]).with(&cfgs),
            Rlib::new("vifast_ring", "../../lib/vifast-ring/src/lib.rs", &[]),
            Rlib::new("vifast_virtio", "../../lib/vifast-virtio/src/lib.rs", &[]),
        ],
    );
    let p = |i: usize| libs[i].as_path();
    let shim_set = [("vifast_user_abi", p(0))];
    let shell_set = [
        ("vifast_user_abi", p(0)),
        ("vifast_ring", p(1)),
        ("vifast_virtio", p(2)),
    ];
    let sets: [&[(&str, &Path)]; 2] = [&shim_set, &shell_set];

    // ---- 3) từng chương trình U-mode, mỗi chương trình một linker script sinh trong OUT_DIR.
    user::build(&Build {
        target: TARGET,
        cwd: &out_dir,
        src_dir: &programs,
        link: Link::Generated(LINKER),
        extra: &cfgs,
        link_args: &LINK_ARGS,
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
