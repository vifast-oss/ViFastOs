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

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // **KT-346** — danh tính bản dựng (dòng `# build:`) cho ảnh sản phẩm arch này; cùng khuôn
    // `vifast-kbin-riscv64/build.rs` (KT-86/D2-125.3) — đóng nợ P20 "chỉ riscv64 có danh tính".
    build_identity(&dir);
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
    // **KT-341b** — `procsrv` cùng nguồn `v2/user/programs/procsrv.rs` (riscv64/aarch64
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
    let programs = dir.join("../../user/programs");
    let shim_src = dir.join("../../user/runtime/lib.rs");

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

/// **KT-346 / A1-184** — cờ `--remap-path-prefix` mà cargo đã cấp cho **crate chính**, để các lần
/// gọi `$RUSTC` trực tiếp (shim U-mode, rlib, ELF user) dùng **cùng** ánh xạ đường dẫn.
///
/// Vì sao: `build.rs` gọi `$RUSTC` trực tiếp nên **không** nhận `RUSTFLAGS` của cargo ⇒ cùng một
/// commit build ở hai đường dẫn nguồn khác nhau cho ra ELF user (và do đó ảnh nhúng nó) khác
/// sha256. Truyền lại đúng cờ remap làm đường dẫn nguồn biến mất khỏi byte ảnh.
fn remap_flags() -> Vec<String> {
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
    cmd.args(remap_flags());
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
        // **KT-347**: shim ABI dùng chung có `#[cfg(rt_alloc)]` (module heap của `vifast-rt`);
        // target này không bật cfg đó — khai báo để `-D warnings` không báo `unexpected_cfgs`.
        .arg("--check-cfg=cfg(rt_alloc)")
        .arg("--check-cfg=cfg(test)")
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
    cmd.args(remap_flags());
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

/// `rev` khi không đọc được `git` (không phải repo, không có git): harness thấy giá trị này là
/// **từ chối đo** — cố ý, không có nhánh "đo được dù không biết nguồn".
const UNKNOWN_REV: &str = "unknown";

/// Số ký tự hex của `rev` trong dòng bằng chứng (khớp `VBREV_REV_LEN` của harness).
const REV_LEN: usize = 12;

/// Dấu vân tay build-inputs khi không tính được (khớp `VBREV_INPUTS_LEN` = 16 của harness).
const UNKNOWN_INPUTS: &str = "unknown";

/// **KT-143** — tên **định nghĩa** dấu vân tay build-inputs (`VBREV_ALGO` của harness), in vào
/// dòng `# build:` để một log tự khai nó được tính bằng **công thức nào**. Vì sao cần: đổi định
/// nghĩa (thêm/bớt thư mục khỏi phạm vi) làm dấu vân tay đổi mà **không** đổi một byte mã nguồn
/// nào — nhìn log cũ và log mới thấy hai số khác nhau mà không có cách nào biết chúng có so được
/// với nhau hay không. Có `algo=` thì câu trả lời nằm ngay trong log.
const UNKNOWN_ALGO: &str = "unknown";

/// `true` ⇔ `s` là một tên định nghĩa hợp lệ (chữ/số/`-`/`_`/`.`, không rỗng, ≤ 32 ký tự).
fn is_algo(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// `true` ⇔ `s` là 16 ký tự hex thường (đúng độ dài `VBREV_INPUTS_LEN`).
fn is_inputs16(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Đọc `VBREV_ALGO` bằng **chính** `v2/test/boot/build-rev.sh` (một nguồn sự thật — không chép
/// lại tên định nghĩa vào build script). Trả `None` khi không chạy được ⇒ ảnh mang
/// `algo=unknown` ⇒ cổng **từ chối** (fail-closed).
fn algo_from_script(root: &Path) -> Option<String> {
    let script = root.join("v2/test/boot/build-rev.sh");
    if !script.is_file() {
        return None;
    }
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(". \"$1\" && printf '%s' \"$VBREV_ALGO\"")
        .arg("bash")
        .arg(&script)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if is_algo(&s) {
        Some(s)
    } else {
        None
    }
}

/// Tính dấu vân tay build-inputs bằng **chính** `v2/test/boot/build-rev.sh` (một nguồn sự thật).
/// Trả `None` khi không chạy được ⇒ ảnh mang `inputs=unknown` ⇒ cổng **từ chối** (fail-closed).
fn inputs_from_script(root: &Path) -> Option<String> {
    let script = root.join("v2/test/boot/build-rev.sh");
    if !script.is_file() {
        return None;
    }
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(". \"$1\" && vbrev_inputs_calc \"$2\" && printf '%s' \"$VBREV_CALC_SHORT\"")
        .arg("bash")
        .arg(&script)
        .arg(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if is_inputs16(&s) {
        Some(s)
    } else {
        None
    }
}

/// Đặt `VIFAST_BUILD_REV`/`VIFAST_BUILD_DIRTY` cho crate (⇒ `env!` trong `src/main.rs`) và khai
/// báo mọi thứ phải làm build script **chạy lại** để giá trị không bị cũ.
fn build_identity(manifest_dir: &str) {
    // Gốc cây nguồn = `<manifest>/../../..` (v2/kernel/vifast-kbin-riscv64 → gốc repo).
    let root = std::fs::canonicalize(Path::new(manifest_dir).join("../../.."))
        .unwrap_or_else(|_| PathBuf::from(manifest_dir).join("../../.."));
    // Env do `v2/tools/repro-build.sh` đặt được **ưu tiên**: giá trị của nó do
    // `v2/test/boot/build-rev.sh` tính (nguồn sự thật duy nhất của định nghĩa dirty) ⇒ dòng
    // `REPRO` của script và dòng `# build:` của ảnh **không thể** lệch định nghĩa.
    let rev = std::env::var("VIFAST_BUILD_REV")
        .ok()
        .map(|v| normalize_rev(&v))
        .filter(|v| is_rev12(v))
        .unwrap_or_else(|| git_head12(&root).unwrap_or_else(|| UNKNOWN_REV.to_string()));
    let dirty = std::env::var("VIFAST_BUILD_DIRTY")
        .ok()
        .filter(|v| v == "0" || v == "1")
        .unwrap_or_else(|| git_dirty(&root));
    // D2-127: danh tính **quyết định** là dấu vân tay **nội dung build-inputs** (không phải rev).
    // Nguồn sự thật duy nhất: `vbrev_inputs_calc` của `v2/test/boot/build-rev.sh` — build script
    // **gọi chính script đó** thay vì chép lại định nghĩa (A0: danh sách do script sinh, không viết tay).
    let inputs = std::env::var("VIFAST_BUILD_INPUTS")
        .ok()
        .filter(|v| is_inputs16(v))
        .unwrap_or_else(|| inputs_from_script(&root).unwrap_or_else(|| UNKNOWN_INPUTS.to_string()));
    // KT-143: tên **định nghĩa** dấu vân tay, cùng nguồn sự thật với `inputs`.
    let algo = std::env::var("VIFAST_BUILD_ALGO")
        .ok()
        .filter(|v| is_algo(v))
        .unwrap_or_else(|| algo_from_script(&root).unwrap_or_else(|| UNKNOWN_ALGO.to_string()));
    println!("cargo:rustc-env=VIFAST_BUILD_REV={rev}");
    println!("cargo:rustc-env=VIFAST_BUILD_DIRTY={dirty}");
    println!("cargo:rustc-env=VIFAST_BUILD_INPUTS={inputs}");
    println!("cargo:rustc-env=VIFAST_BUILD_ALGO={algo}");
    println!(
        "cargo:warning=KT-89 build identity algo={algo} inputs={inputs} dirty={dirty} rev={rev}"
    );
    // Bản build **sau** phải tính lại danh tính: env đổi, HEAD/index đổi, hoặc mã nguồn trong
    // phạm vi dirty đổi. Thiếu các dòng này cargo dùng lại build script cũ ⇒ ảnh mang rev cũ
    // (đúng lớp lỗi KT-76). Mọi bất định chỉ dẫn tới **từ chối đo**, không dẫn tới nhận nhầm.
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_REV");
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_DIRTY");
    // D2-127: dấu vân tay build-inputs cũng là **đầu vào** — thiếu dòng này thì cargo dùng lại
    // build script cũ ⇒ ảnh mang dấu vân tay cũ (đúng lớp lỗi KT-76).
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_INPUTS");
    // KT-143: tên định nghĩa cũng là **đầu vào** — thiếu dòng này thì cargo dùng lại build script
    // cũ ⇒ ảnh mang `algo=` cũ (đúng lớp lỗi KT-76).
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_ALGO");
    for path in identity_watch_paths(&root) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// `git -C <root> <args>`: stdout đã trim khi exit 0, `None` khi không chạy được/lỗi.
fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 12 ký tự hex đầu của `HEAD` (thường hoá thường). `None` khi không có git/repo.
fn git_head12(root: &Path) -> Option<String> {
    let head = git_stdout(root, &["rev-parse", "--verify", "HEAD"])?;
    let head = head.to_ascii_lowercase();
    if head.len() < REV_LEN || !head.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(head[..REV_LEN].to_string())
}

/// `"1"` khi `v2/kernel`/`v2/lib`/`v2/user` có thay đổi chưa commit (kể cả tệp chưa track); **mọi bất định
/// ⇒ `"1"`** (`git` lỗi, không có git, không phải repo) — fail-closed như harness.
fn git_dirty(root: &Path) -> String {
    match git_stdout(
        root,
        &[
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignore-submodules=all",
            "--",
            "v2/kernel",
            "v2/lib",
            "v2/user",
        ],
    ) {
        Some(out) if out.is_empty() => "0".to_string(),
        _ => "1".to_string(),
    }
}

/// Danh sách đường dẫn phải làm build script chạy lại (giữ danh tính không bị cũ).
///
/// KHÔNG watch cả `v2/kernel`: `v2/kernel/target/` nằm trong đó, mỗi lần build lại đổi ⇒ build
/// script (và ~20 ELF U-mode) chạy lại mọi lần. Watch từng crate + `v2/lib` là đủ phạm vi dirty.
fn identity_watch_paths(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(git_dir) = git_stdout(root, &["rev-parse", "--absolute-git-dir"]) {
        out.push(Path::new(&git_dir).join("HEAD"));
    }
    if let Some(index) = git_stdout(root, &["rev-parse", "--git-path", "index"]) {
        let index = Path::new(&index);
        out.push(if index.is_absolute() {
            index.to_path_buf()
        } else {
            root.join(index)
        });
    }
    for rel in [
        "v2/kernel/vifast-kbin-x86_64",
        "v2/kernel/vifast-core",
        "v2/kernel/vifast-plat-common",
        "v2/kernel/vifast-plat-x86_64",
        "v2/user",
        "v2/kernel/Cargo.toml",
        "v2/kernel/Cargo.lock",
        "v2/lib",
    ] {
        out.push(root.join(rel));
    }
    out
}

/// 12 hex thường của `rev` (env có thể là SHA đầy đủ hoặc có khoảng trắng/xuống dòng).
fn normalize_rev(value: &str) -> String {
    let hex: String = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(REV_LEN)
        .collect();
    hex
}

/// `true` nếu đúng 12 ký tự hex (định dạng dòng bằng chứng của harness).
fn is_rev12(value: &str) -> bool {
    value.len() == REV_LEN && value.chars().all(|c| c.is_ascii_hexdigit())
}
