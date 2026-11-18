// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script và các cờ liên kết cho **cả ba** ảnh riscv64 K3 (KT-44, KT-53):
//! ảnh sản phẩm `vifast-kbin-riscv64`, ảnh test `vifast-kbin-ktest*` (feature `ktest`).
//! Cùng một `linker.ld` (`.stack` NOLOAD + `.stack_guard`, D2-72 Q5).
//!
//! KT-53: khi build ảnh **test U-mode** (`CARGO_FEATURE_USER`), build script biên dịch
//! [`init/init_user.rs`] (chỉ `global_asm!` của [`init/init_user.S`]) bằng chính `$RUSTC` thành
//! **ELF64 user** tại `0x4000_0000` theo [`init/init.ld`], ghi vào `$OUT_DIR/init.elf` để
//! `src/ktest_user.rs` `include_bytes!` — không cần toolchain ngoài Rust, không sinh file trong
//! cây nguồn, không ảnh hưởng ảnh sản phẩm (feature off ⇒ không chạy).
//!
//! KT-65: thêm **procsrv** (Rust), **shell** (Rust) và **con** (Rust) — cùng đường build, mỗi
//! chương trình một ELF64 riêng (`procsrv.elf`/`shell.elf`/`child.elf`), nạp bằng cùng loader
//! `vifast-elf` như `init`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // **KT-86** (D2-125.3): danh tính bản dựng (`git rev` 12 hex + cờ dirty) — khối **phụ thêm**,
    // không sửa gì bên dưới; xem `build_identity` ở cuối tệp.
    build_identity(&dir);
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rerun-if-changed=linker.ld");

    // Chỉ ảnh test mới cần các ELF user; ảnh sản phẩm (feature off) không sinh/không nhúng.
    println!("cargo:rerun-if-changed=init/init_user.rs");
    println!("cargo:rerun-if-changed=init/init_user.S");
    println!("cargo:rerun-if-changed=init/procsrv_user.rs");
    println!("cargo:rerun-if-changed=init/shell_user.rs");
    println!("cargo:rerun-if-changed=init/child_user.rs");
    println!("cargo:rerun-if-changed=init/inv_user.rs");
    println!("cargo:rerun-if-changed=init/inv_user.S");
    println!("cargo:rerun-if-changed=init/memsrv_user.rs");
    println!("cargo:rerun-if-changed=init/client_user.rs");
    println!("cargo:rerun-if-changed=init/capa_server_user.rs");
    println!("cargo:rerun-if-changed=init/capa_client_user.rs");
    println!("cargo:rerun-if-changed=init/ring_server_user.rs");
    println!("cargo:rerun-if-changed=init/ring_client_user.rs");
    // KT-73: consumer ring UART → U-mode (ảnh `ktest-con`).
    println!("cargo:rerun-if-changed=init/con_client_user.rs");
    // KT-74: ba bước **A/B/C** của chuỗi `exec` — mỗi bước một ELF riêng (TAP `ktest-exec`).
    println!("cargo:rerun-if-changed=init/exec_a_user.rs");
    println!("cargo:rerun-if-changed=init/exec_b_user.rs");
    println!("cargo:rerun-if-changed=init/exec_c_user.rs");
    println!("cargo:rerun-if-changed=init/exec_d_user.rs");
    println!("cargo:rerun-if-changed=init/pager_common.rs");
    println!("cargo:rerun-if-changed=init/pager_client_common.rs");
    println!("cargo:rerun-if-changed=init/pager_server_user.rs");
    println!("cargo:rerun-if-changed=init/pager_client_a_user.rs");
    println!("cargo:rerun-if-changed=init/pager_client_b_user.rs");
    println!("cargo:rerun-if-changed=../vifast-plat-user/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-abi/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-abi/src/generated.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-ring/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-idl/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-idl/src/generated.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-idl/src/generated_pager.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/lib.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/cache.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/core.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/disk.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/layout.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/stats.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-vfs/src/status.rs");
    println!("cargo:rerun-if-changed=../../lib/vifast-fs/src/lib.rs");
    println!("cargo:rerun-if-changed=init/init.ld");
    if std::env::var_os("CARGO_FEATURE_USER").is_none() {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "riscv64gc-unknown-none-elf" {
        return;
    }
    let dir = Path::new(&dir);
    let user_lib = build_user_lib(dir);
    // KT-69: rlib `no_std` cho chương trình U-mode — hằng ABI sinh từ spec + stub IDL sinh từ
    // `.vidl` + ring (Acquire/Release, không unsafe). Không nằm trong workspace host vì mỗi
    // crate được biên dịch thẳng cho riscv64 ở đây (cùng toolchain, không cargo lồng).
    let abi_lib = build_rlib(dir, "vifast_abi", "../../lib/vifast-abi/src/lib.rs", &[]);
    let fs_lib = build_rlib(dir, "vifast_fs", "../../lib/vifast-fs/src/lib.rs", &[]);
    let ring_lib = build_rlib(dir, "vifast_ring", "../../lib/vifast-ring/src/lib.rs", &[]);
    let idl_lib = build_rlib(dir, "vifast_idl", "../../lib/vifast-idl/src/lib.rs", &[]);
    // KT-73: codec byte `conchan` + hằng console — **cùng** crate kernel dùng (`vifast-virtio`).
    let virtio_lib = build_rlib(
        dir,
        "vifast_virtio",
        "../../lib/vifast-virtio/src/lib.rs",
        &[],
    );
    // KT-70: pager/page-cache — **dùng lại** `vifast_fs::BlockDev` + bảng lỗi `vifast-abi`.
    let vfs_lib = build_rlib(
        dir,
        "vifast_vfs",
        "../../lib/vifast-vfs/src/lib.rs",
        &[
            ("vifast_fs", fs_lib.as_path()),
            ("vifast_abi", abi_lib.as_path()),
        ],
    );
    let shim = [("vifast_user_abi", user_lib.as_path())];
    let shim_all = [
        ("vifast_user_abi", user_lib.as_path()),
        ("vifast_abi", abi_lib.as_path()),
        ("vifast_ring", ring_lib.as_path()),
        ("vifast_idl", idl_lib.as_path()),
        ("vifast_vfs", vfs_lib.as_path()),
        ("vifast_fs", fs_lib.as_path()),
    ];
    build_user_elf(dir, "init_user.rs", "init.elf", &[]);
    build_user_elf(dir, "procsrv_user.rs", "procsrv.elf", &shim);
    // KT-73: shell đọc **byte thật** từ ring UART → cần `vifast_ring` + codec `conchan`.
    let shim_shell = [
        ("vifast_user_abi", user_lib.as_path()),
        ("vifast_ring", ring_lib.as_path()),
        ("vifast_virtio", virtio_lib.as_path()),
    ];
    // **KT-77c**: shell của ảnh **sản phẩm** có thêm `ls`/`cat`/`write` (dịch vụ FS của kernel);
    // shell của ảnh **test** (và của ảnh ca âm "shell nhưng không FS") **không** có ba lệnh đó ⇒
    // `help cmds=` giữ nguyên `6` và kỳ vọng của harness (`proc_demo.rs`, `v2/test/boot/ktest.sh`)
    // **không** phải sửa. Cấu hình đi bằng `--cfg shell_fs` cho **đúng một** ELF này.
    let shell_fs = std::env::var_os("CARGO_FEATURE_PRODUCT_FS").is_some();
    let mut shell_args: [&str; 3] = ["--check-cfg=cfg(shell_fs)", "", ""];
    if shell_fs {
        shell_args[1] = "--cfg";
        shell_args[2] = "shell_fs";
    }
    let shell_args = if shell_fs {
        &shell_args[..]
    } else {
        &shell_args[..1]
    };
    build_user_elf_with(dir, "shell_user.rs", "shell.elf", &shim_shell, shell_args);
    build_user_elf(dir, "child_user.rs", "child.elf", &shim);
    // KT-67a: chương trình U-mode invocation/memsrv (D2-97).
    build_user_elf(dir, "inv_user.rs", "inv.elf", &shim);
    // KT-67b: **hai tiến trình riêng** — memsrv (A) chuyển cap Frame, client (B) nhận + map.
    build_user_elf(dir, "memsrv_user.rs", "memsrv.elf", &shim);
    build_user_elf(dir, "client_user.rs", "client.elf", &shim);
    // KT-68: **capalloc** — server cấp/thu hồi cap + client dùng cap (hai tiến trình riêng).
    build_user_elf(dir, "capa_server_user.rs", "capa_server.elf", &shim);
    build_user_elf(dir, "capa_client_user.rs", "capa_client.elf", &shim);
    // KT-69: **hai tiến trình ring** — server producer + client consumer trên **một Frame**
    // chuyển qua cap; cả hai dùng stub IDL sinh từ `.vidl`.
    build_user_elf(dir, "ring_server_user.rs", "ring_server.elf", &shim_all);
    build_user_elf(dir, "ring_client_user.rs", "ring_client.elf", &shim_all);
    // KT-73: **consumer U-mode** của kênh UART → ring (đọc byte thật, không message cắm sẵn).
    let shim_con = [
        ("vifast_user_abi", user_lib.as_path()),
        ("vifast_ring", ring_lib.as_path()),
        ("vifast_virtio", virtio_lib.as_path()),
    ];
    build_user_elf(dir, "con_client_user.rs", "con_client.elf", &shim_con);
    // KT-74: **ba ảnh `exec`** — A/B/C chỉ in marker + `Exit` riêng, để `exec(path)` phải nạp ELF
    // **khác** thật (entry/segment khác) chứ không phải "chạy lại cùng ảnh".
    // D5 (reviewer): ảnh A phải lấy **method id từ hằng sinh tự động** (`vifast_abi::TCB_SETTLS`)
    // chứ không chép số — cùng nguồn với core, nên `const` và `method` quan sát được là **hai**
    // đường độc lập.
    build_user_elf(
        dir,
        "exec_a_user.rs",
        "exec_a.elf",
        &[
            ("vifast_user_abi", user_lib.as_path()),
            ("vifast_abi", abi_lib.as_path()),
        ],
    );
    build_user_elf(dir, "exec_b_user.rs", "exec_b.elf", &shim);
    build_user_elf(dir, "exec_c_user.rs", "exec_c.elf", &shim);
    // D = bước **không** `SetTls`: ca "TLS reset sau exec" + "SetTls rồi resume" cần ảnh không
    // tự ghi TLS (nếu không, 0 và 0xB2 không phân biệt được).
    build_user_elf(dir, "exec_d_user.rs", "exec_d.elf", &shim);
    // KT-70: **ba tiến trình** pager — server (page cache + LRU) và hai client.
    build_user_elf(dir, "pager_server_user.rs", "pager_server.elf", &shim_all);
    build_user_elf(
        dir,
        "pager_client_a_user.rs",
        "pager_client_a.elf",
        &shim_all,
    );
    build_user_elf(
        dir,
        "pager_client_b_user.rs",
        "pager_client_b.elf",
        &shim_all,
    );
}

/// **KT-92 / A1-184** — cờ `--remap-path-prefix` mà cargo đã cấp cho **crate chính**, để các lần
/// gọi `$RUSTC` trực tiếp (shim U-mode, rlib, ELF user) dùng **cùng** ánh xạ đường dẫn.
///
/// Vì sao: `build.rs` gọi `$RUSTC` trực tiếp nên **không** nhận `RUSTFLAGS` của cargo ⇒ cùng một
/// commit build ở hai đường dẫn nguồn khác nhau cho ra sha khác nhau (A1-184: `c65ee0f6` vs
/// `07c34fc6`). Truyền lại đúng cờ remap làm đường dẫn nguồn biến mất khỏi byte ảnh.
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

/// Biên dịch + liên kết một chương trình user thành ELF64 ở `$OUT_DIR/<out_name>`.
///
/// Dùng `$RUSTC` (cargo cấp) thay vì tự gọi `cargo` lồng nhau: cùng toolchain, không đổi
/// workspace, không phụ thuộc `llvm-objcopy`/assembler ngoài Rust.
/// Biên dịch shim ABI U-mode (`vifast-plat-user`) thành `rlib` riscv64 trong `$OUT_DIR`.
///
/// `unsafe` asm của U-mode nằm gọn ở crate nền tảng đã nằm trong danh sách D2-17
/// (`kernel/vifast-plat-*`); các chương trình Rust còn lại **không** có `unsafe` (luật R5).
fn build_user_lib(dir: &Path) -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rlib = out_dir.join("libvifast_user_abi.rlib");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let source = dir.join("../vifast-plat-user/lib.rs");
    let status = Command::new(&rustc)
        .args(remap_flags())
        .current_dir(dir)
        .arg("--crate-name")
        .arg("vifast_user_abi")
        .arg("--crate-type")
        .arg("rlib")
        .arg("--target")
        .arg("riscv64gc-unknown-none-elf")
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
        .arg("-o")
        .arg(&rlib)
        .arg(&source)
        .status()
        .unwrap_or_else(|error| panic!("KT-65: không chạy được {:?}: {error}", rustc));
    if !status.success() {
        panic!(
            "KT-65: build shim vifast-plat-user FAIL (rustc {:?})",
            rustc
        );
    }
    println!("cargo:rerun-if-changed=../vifast-plat-user/lib.rs");
    rlib
}

/// Biên dịch một crate `no_std` (`vifast-abi`/`vifast-ring`/`vifast-idl`) thành `rlib` riscv64.
///
/// `source` tương đối với thư mục crate kbin; `-D warnings` để stub/ring không cảnh báo.
fn build_rlib(dir: &Path, crate_name: &str, source: &str, externs: &[(&str, &Path)]) -> PathBuf {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rlib = out_dir.join(format!("lib{crate_name}.rlib"));
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let source = dir.join(source);
    let mut cmd = Command::new(&rustc);
    cmd.args(remap_flags());
    cmd.current_dir(dir)
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--crate-type")
        .arg("rlib")
        .arg("--target")
        .arg("riscv64gc-unknown-none-elf")
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
        .arg("-L")
        .arg(format!("dependency={}", out_dir.display()));
    for (name, lib) in externs {
        cmd.arg("--extern")
            .arg(format!("{}={}", name, lib.display()));
    }
    let status = cmd
        .arg("-o")
        .arg(&rlib)
        .arg(&source)
        .status()
        .unwrap_or_else(|error| panic!("KT-70: không chạy được {:?}: {error}", rustc));
    if !status.success() {
        panic!("KT-70: build rlib {crate_name} FAIL (rustc {:?})", rustc);
    }
    rlib
}

fn build_user_elf(dir: &Path, source_name: &str, out_name: &str, externs: &[(&str, &Path)]) {
    build_user_elf_with(dir, source_name, out_name, externs, &[]);
}

/// Như [`build_user_elf`] nhưng thêm cờ rustc (`extra`) — KT-77c dùng cho `--cfg shell_fs`.
fn build_user_elf_with(
    dir: &Path,
    source_name: &str,
    out_name: &str,
    externs: &[(&str, &Path)],
    extra: &[&str],
) {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let out = out_dir.join(out_name);
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let source = dir.join("init").join(source_name);
    let script = dir.join("init/init.ld");
    let mut cmd = Command::new(&rustc);
    cmd.args(remap_flags());
    cmd.current_dir(dir)
        .arg("--target")
        .arg("riscv64gc-unknown-none-elf")
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
        .arg("link-arg=--gc-sections")
        .arg("-C")
        .arg("link-arg=--no-relax")
        .arg("-o")
        .arg(&out);
    // rlib bắc cầu (`vifast_vfs` → `vifast_fs`/`vifast_abi`) nằm cùng OUT_DIR.
    cmd.arg("-L")
        .arg(format!("dependency={}", out_dir.display()));
    for (name, lib) in externs {
        cmd.arg("--extern")
            .arg(format!("{}={}", name, lib.display()));
    }
    let status = cmd
        .arg(&source)
        .status()
        .unwrap_or_else(|error| panic!("KT-65: không chạy được {:?}: {error}", rustc));
    if !status.success() {
        panic!(
            "KT-65: build user ELF {source_name} FAIL (rustc {:?})",
            rustc
        );
    }
    let len = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    println!("cargo:warning=KT-65 {out_name} {len} B");
}

// =============================================================================================
// **KT-86** (D2-125.3) — danh tính bản dựng nhúng vào ảnh.
//
// Vì sao: bốn lần artifact lệch mã nguồn (D2-108A/KT-71/76/79/77b) đều được phát hiện **sau**
// khi đo. Từ đây ảnh tự mang `git rev` + cờ dirty của **lúc build**, kernel in một dòng
// `# build: rev=<12 hex> dirty=<0|1>` (từ `const`, D2-102 — không literal), và harness
// `v2/test/boot/build-rev.sh` **từ chối đo** khi ảnh lệch `HEAD` hoặc dirty=1.
//
// Định nghĩa `dirty` (nguồn sự thật duy nhất: `v2/test/boot/build-rev.sh`, cùng phạm vi):
// chỉ tính `v2/kernel` + `v2/lib`, **có** tính tệp chưa track, **không** tính `v2/docs` /
// `v2/agent/JOURNAL` (nếu tính, mỗi lần ghi log sẽ làm mọi phép đo bị từ chối).
//
// Fail-closed: không đọc được rev ⇒ `unknown`; không chứng minh được là sạch ⇒ `1`. Cả hai đều
// làm cổng **từ chối**, không bao giờ dẫn tới "nhận nhầm là sạch".
// =============================================================================================

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

/// `"1"` khi `v2/kernel`/`v2/lib` có thay đổi chưa commit (kể cả tệp chưa track); **mọi bất định
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
        "v2/kernel/vifast-kbin-riscv64",
        "v2/kernel/vifast-core",
        "v2/kernel/vifast-plat-common",
        "v2/kernel/vifast-plat-riscv64",
        "v2/kernel/vifast-plat-user",
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
