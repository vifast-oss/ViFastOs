// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script cho ảnh x86_64 (KT-257) + **biên dịch ELF user dùng chung** (KT-341).
//!
//! Cùng khuôn `vifast-kbin-aarch64/build.rs` (KT-339): chương trình U-mode
//! `v2/user/programs/shell.rs` là **cùng một nguồn Rust** cho riscv64/aarch64/x86_64;
//! build script này biên dịch nó cho `x86_64-unknown-none` và nhúng bằng `include_bytes!` vào
//! `vifast-kbin-x86_64/src/shellpath.rs`.
//!
//! Chỉ build khi cần: feature `shell`/`product-shell`/`ktest` (ảnh sản phẩm mặc định không kéo
//! parser/ELF/chương trình U-mode nào — D2-08 vẫn tách hai artefact).
//!
//! `--gc-sections` cần thiết vì `global_asm!` đặt mỗi đoạn trong section riêng; linker script
//! `KEEP` đúng ba thứ không ai tham chiếu từ Rust: `.multiboot`, `.note.Xen`, `.text.entry`.
//!
//! # KT-404 — build script này chỉ còn **khai báo**
//!
//! Toàn bộ máy móc dùng chung (danh tính bản dựng + gọi `$RUSTC` + sinh linker script) đã rút về
//! crate host [`vifast_kbuild`]; tệp này chỉ còn **danh sách artefact của arch này**.

use std::path::{Path, PathBuf};

use vifast_kbuild::compile::{self, LinkerScript};
use vifast_kbuild::identity;

/// Target của ảnh này (một chỗ duy nhất — trước KT-404 chuỗi này lặp 4 lần trong tệp).
const TARGET: &str = "x86_64-unknown-none";

/// Điểm vào liên kết của **từng** chương trình U-mode (VA tuyệt đối, `ET_EXEC`).
///
/// Hai ảnh cách nhau 2 MiB — ảnh lớn nhất hiện tại (`shell`) dưới 16 KiB, một segment mọc thêm
/// không thể đè ảnh kế. Cùng cửa sổ `userload::USER_WINDOW_BASE..USER_WINDOW_END` (0x7000_0000).
const USER_PROGRAMS: [(&str, &str, u64); 2] = [
    ("shell.rs", "shell.elf", 0x7000_0000),
    // **KT-341b** — `procsrv` cùng nguồn `v2/user/programs/procsrv.rs` (riscv64/aarch64
    // dùng chung): trả lời `wait`/`kill` trên `EP_PROC`, báo `EV_KILL` cho kernel thi hành
    // `Tcb.Suspend`. Cách `shell` 2 MiB (`userload::USER_WINDOW_BASE..END`).
    ("procsrv.rs", "procsrv.elf", 0x7020_0000),
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

fn build_user_programs(dir: &Path) {
    if !(feature("SHELL") || feature("PRODUCT_SHELL") || feature("KTEST")) {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != TARGET {
        return;
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let programs = dir.join("../../user/programs");
    let shim_src = dir.join("../../user/runtime/lib.rs");

    // ---- 1) shim ABI (`vifast_user_abi`) — backend x86_64 (`int 0x80`).
    let shim = compile::compile_rlib(TARGET, "vifast_user_abi", &shim_src, &[], &[]);
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
        let externs: &[(&str, &Path)] = &[
            ("vifast_user_abi", shim.as_path()),
            ("vifast_ring", ring.as_path()),
            ("vifast_virtio", virtio.as_path()),
        ];
        compile::compile_user_elf(
            TARGET,
            &out_dir,
            &programs.join(source),
            out_name,
            &script,
            externs,
            &[],
            &[],
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
