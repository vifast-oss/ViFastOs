// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh x86_64 (KT-257) + **biên dịch ELF user dùng chung** (KT-341).
//!
//! Cùng khuôn `vifast-kbin-aarch64/build.rs` (KT-339): chương trình U-mode
//! `vifast-plat-user/programs/shell.rs` là **cùng một nguồn Rust** cho riscv64/aarch64/x86_64;
//! build script này biên dịch nó cho `x86_64-unknown-none` và nhúng bằng `include_bytes!` vào
//! `vifast-kbin-x86_64/src/shellpath.rs`.
//!
//! Chỉ build khi cần: feature `shell`/`product-shell`/`ktest` (ảnh sản phẩm mặc định không kéo
//! parser/ELF/chương trình U-mode nào — D2-08 vẫn tách hai artefact).
//!
//! `--gc-sections` cần thiết vì `global_asm!` đặt mỗi đoạn trong section riêng; linker script
//! `KEEP` đúng ba thứ không ai tham chiếu từ Rust: `.multiboot`, `.note.Xen`, `.text.entry`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rerun-if-changed=linker.ld");
    build_user_programs(Path::new(&dir));
}

/// `true` ⇔ feature của Cargo đang bật.
fn feature(name: &str) -> bool {
    std::env::var_os(format!("CARGO_FEATURE_{name}")).is_some()
}

/// Điểm vào liên kết của **từng** chương trình U-mode (VA tuyệt đối, `ET_EXEC`).
///
/// Hai ảnh cách nhau 2 MiB — ảnh lớn nhất hiện tại (`shell`) dưới 16 KiB, một segment mọc thêm
/// không thể đè ảnh kế. Cùng cửa sổ `userload::USER_WINDOW_BASE..USER_WINDOW_END` (0x7000_0000).
const USER_PROGRAMS: [(&str, &str, u64); 2] = [
    ("shell.rs", "shell.elf", 0x7000_0000),
    // **KT-341b** — `procsrv` cùng nguồn `vifast-plat-user/programs/procsrv.rs` (riscv64/aarch64
    // dùng chung): trả lời `wait`/`kill` trên `EP_PROC`, báo `EV_KILL` cho kernel thi hành
    // `Tcb.Suspend`. Cách `shell` 2 MiB (`userload::USER_WINDOW_BASE..END`).
    ("procsrv.rs", "procsrv.elf", 0x7020_0000),
];

fn build_user_programs(dir: &Path) {
    if !(feature("SHELL") || feature("PRODUCT_SHELL") || feature("KTEST")) {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "x86_64-unknown-none" {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let programs = dir.join("../vifast-plat-user/programs");
    let shim_src = dir.join("../vifast-plat-user/lib.rs");

    // ---- 1) shim ABI (`vifast_user_abi`) — backend x86_64 (`int 0x80`).
    let shim = out_dir.join("libvifast_user_abi.rlib");
    compile_rlib(&rustc, "vifast_user_abi", &shim_src, &shim, &out_dir, &[]);
    // ---- 2) ring KT-69 + codec `conchan` của virtio (shell đọc ring UART).
    let ring = out_dir.join("libvifast_ring.rlib");
    compile_rlib(
        &rustc,
        "vifast_ring",
        &dir.join("../../lib/vifast-ring/src/lib.rs"),
        &ring,
        &out_dir,
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
    );

    // ---- 3) từng chương trình U-mode, mỗi chương trình một linker script sinh trong OUT_DIR.
    for (source, out_name, base) in USER_PROGRAMS {
        let script = out_dir.join(format!("{out_name}.ld"));
        write_linker_script(&script, base);
        let externs: &[(&str, &Path)] = &[
            ("vifast_user_abi", shim.as_path()),
            ("vifast_ring", ring.as_path()),
            ("vifast_virtio", virtio.as_path()),
        ];
        compile_user_elf(
            &rustc,
            &programs.join(source),
            &out_dir.join(out_name),
            &script,
            externs,
            &out_dir,
        );
        println!("cargo:rerun-if-changed={}", programs.join(source).display());
    }
    println!("cargo:rerun-if-changed={}", shim_src.display());
    println!("cargo:rerun-if-changed=../../lib/vifast-ring/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-virtio/src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Biên dịch một crate `no_std` thành `rlib` cho `x86_64-unknown-none` (cùng `$RUSTC`).
fn compile_rlib(
    rustc: &str,
    crate_name: &str,
    source: &Path,
    out: &Path,
    out_dir: &Path,
    cfgs: &[String],
) {
    let mut cmd = Command::new(rustc);
    cmd.current_dir(out_dir)
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--crate-type")
        .arg("rlib")
        .arg("--target")
        .arg("x86_64-unknown-none")
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
        .arg(format!("dependency={}", out_dir.display()))
        .arg("-o")
        .arg(out)
        .arg(source);
    let status = cmd
        .status()
        .unwrap_or_else(|error| panic!("KT-341: không chạy được {rustc:?}: {error}"));
    if !status.success() {
        panic!("KT-341: build rlib {} FAIL", source.display());
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
) {
    let mut cmd = Command::new(rustc);
    cmd.current_dir(out_dir)
        .arg("--target")
        .arg("x86_64-unknown-none")
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
        .arg("-C")
        .arg(format!("link-arg=-T{}", script.display()))
        .arg("-C")
        .arg("link-arg=--gc-sections")
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
        .unwrap_or_else(|error| panic!("KT-341: không chạy được {rustc:?}: {error}"));
    if !status.success() {
        panic!("KT-341: build user ELF {} FAIL", source.display());
    }
    let len = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!("cargo:warning=KT-341 {} {len} B", out.display());
}

/// Sinh linker script cho **một** chương trình ở VA `base`: tách segment theo quyền để kernel
/// map W^X (`text` RX, `rodata` R, `data` RW, `bss` RW).
fn write_linker_script(path: &Path, base: u64) {
    let script = format!(
        "/* SPDX-License-Identifier: MIT OR Apache-2.0 */\n\
         /* KT-341 — sinh bởi build.rs: chương trình U-mode dùng chung, liên kết ở 0x{base:08x}. */\n\
         OUTPUT_ARCH(i386:x86-64)\n\
         ENTRY(_start)\n\
         SECTIONS\n\
         {{\n\
             . = 0x{base:08x};\n\
             .text : ALIGN(0x1000) {{ KEEP(*(.text.entry)) *(.text .text.*) }}\n\
             .rodata : ALIGN(0x1000) {{ *(.rodata .rodata.*) *(.srodata .srodata.*) *(.got) *(.got.plt) }}\n\
             .data : ALIGN(0x1000) {{ *(.data .data.*) *(.sdata .sdata.*) }}\n\
             .bss : ALIGN(0x1000) {{ *(.bss .bss.*) *(COMMON) }}\n\
             /DISCARD/ : {{ *(.eh_frame) *(.comment) *(.note.GNU-stack) }}\n\
         }}\n"
    );
    std::fs::write(path, script)
        .unwrap_or_else(|error| panic!("KT-341: ghi {} lỗi {error}", path.display()));
}
