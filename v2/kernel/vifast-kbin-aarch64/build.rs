// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh aarch64 (KT-247) + **biên dịch ELF user dùng chung** (KT-339).
//!
//! # KT-339 — vì sao build script này biên dịch chương trình U-mode
//!
//! `procsrv`/`shell` là **cùng một nguồn Rust** (`v2/kernel/vifast-plat-user/programs/`) cho cả
//! riscv64 và aarch64 (D2-233 mục 3). `vifast-kbin-riscv64/build.rs` đã biên dịch chúng lồng
//! bằng `$RUSTC`; ở đây làm **đúng** khuôn đó cho `aarch64-unknown-none`, thêm `echo.rs` (chương
//! trình kiểm IPC buffer của TAP 44).
//!
//! Chỉ ảnh **kiểm** (`--features ktest`) mới cần: ảnh sản phẩm **không** link `vifast-core` (trần
//! `.data+.bss` 65.536 B < `size_of::<Machine>()`), nên K5 chưa chạy trên ảnh sản phẩm — quyết
//! định ngân sách ghi ở journal KT-339. Khi feature tắt, build script chỉ đăng ký linker script;
//! không sinh ELF nào, không thêm phụ thuộc build.
//!
//! # Đột biến (bắt buộc ĐỎ)
//!
//! - `mut-info-x1` → `--cfg mut_info_x1` cho shim: `MessageInfo` đi `x1` thay `x2` (giả riscv64).
//! - `mut-no-x0-out` → `--cfg mut_no_x0_out` cho shim: stub **không** ghi lại `x0` (error).

use std::path::{Path, PathBuf};
use std::process::Command;

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

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
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
    if target != "aarch64-unknown-none" {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let programs = dir.join("../vifast-plat-user/programs");
    let shim_src = dir.join("../vifast-plat-user/lib.rs");

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

    // ---- 1) shim ABI (`vifast_user_abi`) — `rlib` cho target này.
    let shim = out_dir.join("libvifast_user_abi.rlib");
    compile_rlib(
        &rustc,
        "vifast_user_abi",
        &shim_src,
        &shim,
        &out_dir,
        &[],
        &cfgs,
    );
    // ---- 2) ring KT-69 + codec `conchan` của virtio (shell đọc ring UART).
    let ring = out_dir.join("libvifast_ring.rlib");
    compile_rlib(
        &rustc,
        "vifast_ring",
        &dir.join("../../lib/vifast-ring/src/lib.rs"),
        &ring,
        &out_dir,
        &[],
        &[],
    );
    let virtio = out_dir.join("libvifast_virtio.rlib");
    compile_rlib(
        &rustc,
        "vifast_virtio",
        &dir.join("../../lib/vifast-virtio/src/lib.rs"),
        &virtio,
        &out_dir,
        &[],
        &[],
    );

    // ---- 3) từng chương trình U-mode, mỗi chương trình một linker script sinh trong OUT_DIR.
    for (source, out_name, base) in USER_PROGRAMS {
        let script = out_dir.join(format!("{out_name}.ld"));
        write_linker_script(&script, base);
        let externs: &[(&str, &Path)] = match out_name {
            "shell.elf" => &[
                ("vifast_user_abi", shim.as_path()),
                ("vifast_ring", ring.as_path()),
                ("vifast_virtio", virtio.as_path()),
            ],
            "procsrv.elf" => &[("vifast_user_abi", shim.as_path())],
            _ => &[("vifast_user_abi", shim.as_path())],
        };
        compile_user_elf(
            &rustc,
            &programs.join(source),
            &out_dir.join(out_name),
            &script,
            externs,
            &out_dir,
            &cfgs,
        );
        println!("cargo:rerun-if-changed={}", programs.join(source).display());
    }
    println!("cargo:rerun-if-changed={}", shim_src.display());
    println!("cargo:rerun-if-changed=../../lib/vifast-ring/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-virtio/src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Biên dịch một crate `no_std` thành `rlib` cho `aarch64-unknown-none` (cùng `$RUSTC`).
fn compile_rlib(
    rustc: &str,
    crate_name: &str,
    source: &Path,
    out: &Path,
    out_dir: &Path,
    externs: &[(&str, &Path)],
    cfgs: &[String],
) {
    let mut cmd = Command::new(rustc);
    cmd.current_dir(out_dir)
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--crate-type")
        .arg("rlib")
        .arg("--target")
        .arg("aarch64-unknown-none")
        .arg("--edition")
        .arg("2021")
        .arg("-C")
        .arg("panic=abort")
        .arg("-C")
        .arg("opt-level=1")
        .arg("-C")
        .arg("debuginfo=0")
        .arg("-D")
        .arg("warnings")
        .args(cfgs)
        .arg("-L")
        .arg(format!("dependency={}", out_dir.display()));
    for (name, lib) in externs {
        cmd.arg("--extern")
            .arg(format!("{}={}", name, lib.display()));
    }
    let status = cmd
        .arg("-o")
        .arg(out)
        .arg(source)
        .status()
        .unwrap_or_else(|error| panic!("KT-339: không chạy được {rustc:?}: {error}"));
    if !status.success() {
        panic!("KT-339: build rlib {} FAIL", source.display());
    }
}

/// Biên dịch + liên kết một chương trình U-mode thành ELF64 ở VA `base`.
fn compile_user_elf(
    rustc: &str,
    source: &Path,
    out: &Path,
    script: &Path,
    externs: &[(&str, &Path)],
    out_dir: &Path,
    cfgs: &[String],
) {
    let mut cmd = Command::new(rustc);
    cmd.current_dir(out_dir)
        .arg("--target")
        .arg("aarch64-unknown-none")
        .arg("--edition")
        .arg("2021")
        .arg("-C")
        .arg("panic=abort")
        .arg("-C")
        .arg("opt-level=1")
        .arg("-C")
        .arg("debuginfo=0")
        .arg("-C")
        .arg("strip=symbols")
        .arg("-C")
        .arg("relocation-model=static")
        .args(cfgs)
        .arg("-C")
        .arg(format!("link-arg=-T{}", script.display()))
        .arg("-C")
        .arg("link-arg=--gc-sections")
        // LLD mặc định `p_align = 64 KiB` cho aarch64; `vifast-elf::parse` chỉ nhận `align` ≤
        // 4 KiB (một Frame). Ép trang 4 KiB để segment khớp ABI nạp của kernel.
        .arg("-C")
        .arg("link-arg=-zmax-page-size=0x1000")
        .arg("-o")
        .arg(out)
        .arg("-L")
        .arg(format!("dependency={}", out_dir.display()));
    for (name, lib) in externs {
        cmd.arg("--extern")
            .arg(format!("{}={}", name, lib.display()));
    }
    let status = cmd
        .arg(source)
        .status()
        .unwrap_or_else(|error| panic!("KT-339: không chạy được {rustc:?}: {error}"));
    if !status.success() {
        panic!("KT-339: build user ELF {} FAIL", source.display());
    }
    let len = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!("cargo:warning=KT-339 {} {len} B", out.display());
}

/// Sinh linker script cho **một** chương trình ở VA `base`: tách segment theo quyền để kernel
/// map W^X (`text` RX, `rodata` R, `data` RW, `bss` RW).
///
/// Vì sao sinh trong `OUT_DIR` mà không đặt trong cây nguồn: cùng một nguồn Rust được liên kết ở
/// ba VA khác nhau; ba tệp `.ld` trong cây là ba nguồn sự thật phải đồng bộ tay. Sinh bằng
/// `write` từ **một** mẫu (dưới đây) giữ đúng một chỗ.
fn write_linker_script(path: &Path, base: u64) {
    let script = format!(
        "/* SPDX-License-Identifier: MIT OR Apache-2.0 */\n\
         /* KT-339 — sinh bởi build.rs: chương trình U-mode dùng chung, liên kết ở 0x{base:08x}. */\n\
         OUTPUT_ARCH(aarch64)\n\
         ENTRY(_start)\n\
         SECTIONS\n\
         {{\n\
             . = 0x{base:08x};\n\
             .text : ALIGN(0x1000) {{ KEEP(*(.text.entry)) *(.text .text.*) }}\n\
             .rodata : ALIGN(0x1000) {{ *(.rodata .rodata.*) *(.srodata .srodata.*) }}\n\
             .data : ALIGN(0x1000) {{ *(.data .data.*) *(.sdata .sdata.*) }}\n\
             .bss : ALIGN(0x1000) {{ *(.bss .bss.*) *(COMMON) }}\n\
             /DISCARD/ : {{ *(.eh_frame) *(.comment) *(.ARM.attributes) }}\n\
         }}\n"
    );
    std::fs::write(path, script)
        .unwrap_or_else(|error| panic!("KT-339: ghi {} lỗi {error}", path.display()));
}
