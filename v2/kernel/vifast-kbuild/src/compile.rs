// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-404** — gọi `$RUSTC` (cargo cấp) để biên dịch artefact U-mode, **một** bản cho cả 3 arch.
//!
//! # Vì sao gọi `$RUSTC` trực tiếp (không `cargo` lồng nhau)
//!
//! Chương trình U-mode là `no_std` cho target `*-unknown-none` và **không** là thành viên
//! workspace host; ba `build.rs` đã dùng `$RUSTC` từ `KT-65` (`KT-339` cho aarch64/x86_64). Giữ
//! nguyên đường đó: cùng toolchain, không đổi workspace, không phụ thuộc `llvm-objcopy`/assembler
//! ngoài Rust. `KT-404` **không** đổi cách biên dịch — chỉ gộp **ba bản sao** của cùng đoạn mã.
//!
//! # Bất biến bắt buộc
//!
//! * Mọi lần gọi `$RUSTC` **phải** truyền [`remap_flags`] — nếu không, đường dẫn tuyệt đối của
//!   thư mục nguồn lọt vào byte ELF ⇒ cùng commit ở hai thư mục cho hai ảnh khác nhau (A1-184,
//!   nợ P20/`N-7`). Đây là lý do `KT-92`/`KT-346` thêm hàm này; `KT-404` giữ nó ở **một** chỗ.
//! * `-D warnings` cho `rlib` (stub/ring/ABI không được cảnh báo), **không** cho ELF chương trình
//!   (giữ nguyên từng byte như trước `KT-404`).
//! * `--check-cfg=cfg(rt_alloc)`/`cfg(test)` **luôn** truyền: chỉ ảnh hưởng lint, không ảnh hưởng
//!   byte mã ⇒ thêm cho cả 3 arch không đổi artefact nào.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Cờ `--remap-path-prefix` mà cargo đã cấp cho **crate chính**, để các lần gọi `$RUSTC` trực
/// tiếp (shim U-mode, rlib, ELF user) dùng **cùng** ánh xạ đường dẫn.
///
/// Vì sao: `build.rs` gọi `$RUSTC` trực tiếp nên **không** nhận `RUSTFLAGS` của cargo ⇒ cùng một
/// commit build ở hai đường dẫn nguồn khác nhau cho ra sha khác nhau (A1-184: `c65ee0f6` vs
/// `07c34fc6`). Truyền lại đúng cờ remap làm đường dẫn nguồn biến mất khỏi byte ảnh.
pub fn remap_flags() -> Vec<String> {
    let mut out = Vec::new();
    // Cargo đặt `CARGO_ENCODED_RUSTFLAGS` cho build script, phân tách bằng 0x1f.
    if let Ok(encoded) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        for part in encoded.split('\u{1f}') {
            if part.starts_with("--remap-path-prefix") {
                out.push(part.to_string());
            }
        }
    }
    // Dự phòng: `RUSTFLAGS` dạng chuỗi (một số môi trường chỉ đặt biến này).
    if out.is_empty() {
        if let Ok(flags) = std::env::var("RUSTFLAGS") {
            for part in flags.split_whitespace() {
                if part.starts_with("--remap-path-prefix") {
                    out.push(part.to_string());
                }
            }
        }
    }
    out
}

/// `$RUSTC` của cargo (fallback `rustc` trong `PATH`).
fn rustc() -> String {
    std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string())
}

/// `$OUT_DIR` của crate ảnh đang build.
fn out_dir() -> PathBuf {
    PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"))
}

/// Biên dịch một crate `no_std` thành `rlib` cho `target` (cùng `$RUSTC`).
///
/// `source` là đường dẫn **tuyệt đối** (build.rs tự `join` từ `CARGO_MANIFEST_DIR`).
/// `extra` là cờ rustc phụ (`--cfg rt_alloc`, `--check-cfg=…`). Trả đường dẫn `rlib`.
pub fn compile_rlib(
    target: &str,
    crate_name: &str,
    source: &Path,
    externs: &[(&str, &Path)],
    extra: &[&str],
) -> PathBuf {
    let out_dir = out_dir();
    let rlib = out_dir.join(format!("lib{crate_name}.rlib"));
    let rustc = rustc();
    let mut cmd = Command::new(&rustc);
    cmd.args(remap_flags());
    cmd.current_dir(&out_dir)
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--crate-type")
        .arg("rlib")
        .arg("--target")
        .arg(target)
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
        // **KT-347**: shim ABI dùng chung có `#[cfg(rt_alloc)]` (module heap của `vifast-rt`);
        // target không bật cfg đó vẫn phải khai báo để `-D warnings` không báo `unexpected_cfgs`.
        .arg("--check-cfg=cfg(rt_alloc)")
        .arg("--check-cfg=cfg(test)")
        .arg("-L")
        .arg(format!("dependency={}", out_dir.display()));
    for arg in extra {
        cmd.arg(arg);
    }
    for (name, lib) in externs {
        cmd.arg("--extern")
            .arg(format!("{}={}", name, lib.display()));
    }
    let status = cmd
        .arg("-o")
        .arg(&rlib)
        .arg(source)
        .status()
        .unwrap_or_else(|error| panic!("KT-404: không chạy được {rustc:?}: {error}"));
    if !status.success() {
        panic!("KT-404: build rlib {crate_name} FAIL (rustc {rustc:?})");
    }
    rlib
}

/// Biên dịch + liên kết **một** chương trình U-mode thành ELF64 ở `$OUT_DIR/<out_name>`.
///
/// * `cwd` — thư mục làm việc của `$RUSTC` (giữ nguyên như trước `KT-404`: `riscv64` dùng thư
///   mục crate ảnh, `aarch64`/`x86_64` dùng `$OUT_DIR`);
/// * `script` — linker script (`init/init.ld` của ảnh, hoặc tệp sinh bởi
///   [`write_linker_script`]);
/// * `link_args` — cờ `-C link-arg=…` **riêng của arch** (`--no-relax` cho riscv64,
///   `-zmax-page-size=0x1000` cho aarch64, không có cho x86_64);
/// * `extra` — cờ rustc phụ (`--cfg shell_fs`, …).
pub fn compile_user_elf(
    target: &str,
    cwd: &Path,
    source: &Path,
    out_name: &str,
    script: &Path,
    externs: &[(&str, &Path)],
    extra: &[&str],
    link_args: &[&str],
) -> PathBuf {
    let out_dir = out_dir();
    let out = out_dir.join(out_name);
    let rustc = rustc();
    let mut cmd = Command::new(&rustc);
    cmd.args(remap_flags());
    cmd.current_dir(cwd)
        .arg("--target")
        .arg(target)
        .arg("--edition")
        .arg("2021")
        .arg("-C")
        .arg("panic=abort")
        .arg("-C")
        .arg("opt-level=1")
        .arg("-C")
        .arg("debuginfo=0");
    for arg in extra {
        cmd.arg(arg);
    }
    cmd.arg("-C")
        .arg("strip=symbols")
        .arg("-C")
        .arg("relocation-model=static")
        .arg("-C")
        .arg(format!("link-arg=-T{}", script.display()))
        .arg("-C")
        .arg("link-arg=--gc-sections");
    for arg in link_args {
        cmd.arg("-C").arg(format!("link-arg={arg}"));
    }
    cmd.arg("-o")
        .arg(&out)
        // rlib bắc cầu (`vifast_vfs` → `vifast_fs`/`vifast_abi`) nằm cùng OUT_DIR.
        .arg("-L")
        .arg(format!("dependency={}", out_dir.display()));
    for (name, lib) in externs {
        cmd.arg("--extern")
            .arg(format!("{}={}", name, lib.display()));
    }
    let status = cmd
        .arg(source)
        .status()
        .unwrap_or_else(|error| panic!("KT-404: không chạy được {rustc:?}: {error}"));
    if !status.success() {
        panic!(
            "KT-404: build user ELF {} FAIL (rustc {rustc:?})",
            source.display()
        );
    }
    let len = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    println!("cargo:warning=KT-404 {out_name} {len} B");
    out
}

/// Tham số linker script của **một** arch — ba arch khác nhau đúng ba chỗ (kiến trúc, section
/// `.rodata` thêm, danh sách `/DISCARD/`), phần còn lại là **một** mẫu.
pub struct LinkerScript<'a> {
    /// `OUTPUT_ARCH(...)` — `aarch64` hoặc `i386:x86-64`.
    pub output_arch: &'a str,
    /// Section đặt thêm vào `.rodata` (x86_64 cần `*(.got) *(.got.plt)`; aarch64 rỗng).
    pub rodata_extra: &'a str,
    /// Danh sách `/DISCARD/` riêng của arch.
    pub discard: &'a str,
    /// Nhãn task sinh ra mẫu này (giữ nguyên dấu vết trong tệp `.ld` sinh ra).
    pub note: &'a str,
}

/// Sinh linker script cho **một** chương trình ở VA `base`: tách segment theo quyền để kernel
/// map W^X (`text` RX, `rodata` R, `data` RW, `bss` RW).
///
/// Vì sao sinh trong `OUT_DIR` mà không đặt trong cây nguồn: cùng một nguồn Rust được liên kết ở
/// ba VA khác nhau; ba tệp `.ld` trong cây là ba nguồn sự thật phải đồng bộ tay. Sinh bằng
/// `write` từ **một** mẫu (dưới đây) giữ đúng một chỗ.
pub fn write_linker_script(path: &Path, base: u64, spec: &LinkerScript<'_>) {
    let script = format!(
        "/* SPDX-License-Identifier: MIT OR Apache-2.0 */\n\
         /* {note} — sinh bởi build.rs: chương trình U-mode dùng chung, liên kết ở 0x{base:08x}. */\n\
         OUTPUT_ARCH({arch})\n\
         ENTRY(_start)\n\
         SECTIONS\n\
         {{\n\
             . = 0x{base:08x};\n\
             .text : ALIGN(0x1000) {{ KEEP(*(.text.entry)) *(.text .text.*) }}\n\
             .rodata : ALIGN(0x1000) {{ *(.rodata .rodata.*) *(.srodata .srodata.*){rodata_extra} }}\n\
             .data : ALIGN(0x1000) {{ *(.data .data.*) *(.sdata .sdata.*) }}\n\
             .bss : ALIGN(0x1000) {{ *(.bss .bss.*) *(COMMON) }}\n\
             /DISCARD/ : {{ {discard} }}\n\
         }}\n",
        note = spec.note,
        arch = spec.output_arch,
        rodata_extra = spec.rodata_extra,
        discard = spec.discard,
    );
    std::fs::write(path, script)
        .unwrap_or_else(|error| panic!("KT-404: ghi {} lỗi {error}", path.display()));
}

/// **KT-347** — xuất một ELF user vừa build ra `target/<triple>/release/user-elfs/<name>`.
///
/// Vì sao: `$OUT_DIR` có hash package nên runner host không biết đường; thư mục `user-elfs/` là
/// đường **xác định** để `product-riscv64.sh` seed `/hello.elf` + `/u1a.elf` vào đĩa FS. Chỉ ghi
/// trong `target/` (không vào cây nguồn — D2-109/D2-111) và lỗi ghi **không** làm hỏng build
/// (runner sẽ fail-closed khi thiếu tệp).
pub fn export_user_elf(name: &str) {
    let out_dir = out_dir();
    // Tìm tổ tiên tên `release` (không đếm cứng số cấp — cấu trúc `target/<triple>/release` là
    // hợp đồng, còn số thư mục con của `build/<pkg>-<hash>/out` là chi tiết của cargo).
    let Some(release) = out_dir
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "release"))
    else {
        return;
    };
    let dest = release.join("user-elfs");
    if std::fs::create_dir_all(&dest).is_ok() {
        let _ = std::fs::copy(out_dir.join(name), dest.join(name));
    }
}
