// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh aarch64 (KT-247) + **biên dịch ELF user dùng chung** (KT-339).
//!
//! # KT-339 — vì sao build script này biên dịch chương trình U-mode
//!
//! `procsrv`/`shell` là **cùng một nguồn Rust** (`v2/user/programs/`) cho cả
//! riscv64 và aarch64 (D2-233 mục 3). `vifast-kbin-riscv64/build.rs` đã biên dịch chúng lồng
//! bằng `$RUSTC`; ở đây làm **đúng** khuôn đó cho `aarch64-unknown-none`, thêm `echo.rs` (chương
//! trình kiểm IPC buffer của TAP 44).
//!
//! Chỉ ảnh **kiểm** (`--features ktest`) mới cần: ảnh sản phẩm **không** link `vifast-core` (trần
//! `.data+.bss` 65.536 B < `size_of::<Machine>()`), nên K5 chưa chạy trên ảnh sản phẩm — quyết
//! định ngân sách ghi ở journal KT-339. Khi feature tắt, build script chỉ đăng ký linker script;
//! không sinh ELF nào, không thêm phụ thuộc build.
//!
//! # KT-404 — build script này chỉ còn **khai báo**
//!
//! Toàn bộ máy móc dùng chung (danh tính bản dựng + gọi `$RUSTC` + sinh linker script) đã rút về
//! crate host [`vifast_kbuild`]; tệp này chỉ còn **danh sách artefact của arch này** — đúng
//! `KE-HOACH-KIEN-TRUC.md` dòng `KT-404` ("3 `build.rs` chỉ nhúng artefact"). Ba bản sao trước
//! `KT-404` đã **trôi** định nghĩa `dirty` (bản riscv64 thiếu `v2/user`).
//!
//! # Đột biến (bắt buộc ĐỎ)
//!
//! - `mut-info-x1` → `--cfg mut_info_x1` cho shim: `MessageInfo` đi `x1` thay `x2` (giả riscv64).
//! - `mut-no-x0-out` → `--cfg mut_no_x0_out` cho shim: stub **không** ghi lại `x0` (error).

use std::path::{Path, PathBuf};

use vifast_kbuild::compile::{self, LinkerScript};
use vifast_kbuild::identity;

/// Target của ảnh này (một chỗ duy nhất — trước KT-404 chuỗi này lặp 4 lần trong tệp).
const TARGET: &str = "aarch64-unknown-none";

/// Điểm vào liên kết của **từng** chương trình U-mode (VA tuyệt đối, `ET_EXEC`).
///
/// Ba mốc cách nhau 2 MiB: ảnh lớn nhất hiện tại (`shell`) dưới 8 KiB, cách nhau 2 MiB để một
/// segment mọc thêm không thể đè ảnh kế, và tất cả nằm trong cửa sổ
/// `vifast_plat_aarch64::userload::USER_WINDOW_BASE..USER_WINDOW_END`.
const USER_PROGRAMS: [(&str, &str, u64); 3] = [
    ("procsrv.rs", "procsrv.elf", 0x7100_0000),
    ("shell.rs", "shell.elf", 0x7120_0000),
    ("echo.rs", "echo.elf", 0x7140_0000),
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
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_MUT_INFO_X1");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_MUT_NO_X0_OUT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PRODUCT_SHELL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_MUT_NO_SVC");
    build_user_programs(Path::new(&dir));
}

/// `true` ⇔ feature X của Cargo đang bật.
fn feature(name: &str) -> bool {
    std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}

fn build_user_programs(dir: &Path) {
    // Ảnh sản phẩm: không K5 (D2-236/D2-244) ⇒ không sinh ELF user nào.
    // **KT-339a**: ảnh sản phẩm có shell (`--features product-shell`) cũng cần ELF user
    // cùng nguồn — cùng một đường biên dịch, không có bản sao thứ hai.
    if !feature("KTEST") && !feature("PRODUCT_SHELL") {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != TARGET {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let programs = dir.join("../../user/programs");
    let shim_src = dir.join("../../user/runtime/lib.rs");

    // Cờ `--cfg` của đột biến — **chỉ** có khi feature bật; kèm `--check-cfg` để bản không đột
    // biến không cảnh báo "unexpected cfg".
    let mut cfgs: Vec<String> = vec![
        "--check-cfg=cfg(shell_fs)".to_string(),
        "--check-cfg=cfg(mut_info_x1)".to_string(),
        "--check-cfg=cfg(mut_no_x0_out)".to_string(),
    ];
    if feature("MUT_INFO_X1") {
        cfgs.push("--cfg".to_string());
        cfgs.push("mut_info_x1".to_string());
    }
    if feature("MUT_NO_X0_OUT") {
        cfgs.push("--cfg".to_string());
        cfgs.push("mut_no_x0_out".to_string());
    }
    let cfgs: Vec<&str> = cfgs.iter().map(String::as_str).collect();

    // ---- 1) shim ABI (`vifast_user_abi`) — `rlib` cho target này.
    let shim = compile::compile_rlib(TARGET, "vifast_user_abi", &shim_src, &[], &cfgs);
    // ---- 2) ring KT-69 + codec `conchan` của virtio (shell đọc ring UART).
    let ring = compile::compile_rlib(
        TARGET,
        "vifast_ring",
        &dir.join("../../lib/vifast-ring/src/lib.rs"),
        &[],
        &[],
    );
    let virtio = compile::compile_rlib(
        TARGET,
        "vifast_virtio",
        &dir.join("../../lib/vifast-virtio/src/lib.rs"),
        &[],
        &[],
    );

    // ---- 3) từng chương trình U-mode, mỗi chương trình một linker script sinh trong OUT_DIR.
    for (source, out_name, base) in USER_PROGRAMS {
        let script = out_dir.join(format!("{out_name}.ld"));
        compile::write_linker_script(&script, base, &LINKER);
        let externs: &[(&str, &Path)] = match out_name {
            "shell.elf" => &[
                ("vifast_user_abi", shim.as_path()),
                ("vifast_ring", ring.as_path()),
                ("vifast_virtio", virtio.as_path()),
            ],
            _ => &[("vifast_user_abi", shim.as_path())],
        };
        compile::compile_user_elf(
            TARGET,
            &out_dir,
            &programs.join(source),
            out_name,
            &script,
            externs,
            &cfgs,
            &LINK_ARGS,
        );
        println!("cargo:rerun-if-changed={}", programs.join(source).display());
    }
    // **KT-401** — `shell.rs` tách tệp (R1): theo dõi cả hai tệp nguồn cùng ELF.
    println!("cargo:rerun-if-changed=../../user/programs/shell_cmds.rs");
    println!("cargo:rerun-if-changed=../../user/programs/shell_fs.rs");
    println!("cargo:rerun-if-changed=../../user/programs/shell_net.rs");
    println!("cargo:rerun-if-changed={}", shim_src.display());
    println!("cargo:rerun-if-changed=../../lib/vifast-ring/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-virtio/src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
}
