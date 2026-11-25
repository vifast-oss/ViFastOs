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
//!
//! # KT-404 — build script này chỉ còn **khai báo**
//!
//! Toàn bộ máy móc dùng chung (danh tính bản dựng + gọi `$RUSTC` + `--remap-path-prefix` + sinh
//! linker script) đã rút về crate host [`vifast_kbuild`]; tệp này chỉ còn **danh sách artefact
//! của arch này** (`KE-HOACH-KIEN-TRUC.md` dòng `KT-404`). Ba bản sao trước `KT-404` đã **trôi**
//! định nghĩa `dirty`: bản riscv64 tính trên `v2/kernel v2/lib` **thiếu `v2/user`** trong khi
//! `v2/user` nằm trong `VBREV_INPUT_ROOTS` của `build-rev.sh` (KT-346) ⇒ sửa `v2/user` làm dấu
//! vân tay `inputs` đổi mà ảnh vẫn khai `dirty=0`. Từ `KT-404` cả 3 arch dùng **một** định nghĩa.

use std::path::{Path, PathBuf};

use vifast_kbuild::compile;
use vifast_kbuild::identity;

/// Target của ảnh này (một chỗ duy nhất — trước KT-404 chuỗi này lặp 6 lần trong tệp).
const TARGET: &str = "riscv64gc-unknown-none-elf";

/// Cờ `-C link-arg=…` riêng của arch: riscv64 cấm linker nới rộng `c.jal`/`c.j` (relaxation) vì
/// `vifast-elf` nạp segment theo đúng offset đã liên kết.
const LINK_ARGS: [&str; 1] = ["--no-relax"];

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // **KT-86** (D2-125.3): danh tính bản dựng (`git rev` 12 hex + cờ dirty) — khối **phụ thêm**,
    // không sửa gì bên dưới; xem `vifast_kbuild::identity`.
    identity::build_identity(&dir, "vifast-kbin-riscv64");
    println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
    println!("cargo:rustc-link-arg=--gc-sections");
    println!("cargo:rerun-if-changed=linker.ld");

    // Chỉ ảnh test mới cần các ELF user; ảnh sản phẩm (feature off) không sinh/không nhúng.
    println!("cargo:rerun-if-changed=init/init_user.rs");
    println!("cargo:rerun-if-changed=init/init_user.S");
    println!("cargo:rerun-if-changed=../../user/programs/procsrv.rs");
    println!("cargo:rerun-if-changed=../../user/programs/shell.rs");
    println!("cargo:rerun-if-changed=../../user/programs/shell_cmds.rs");
    // **KT-401** — shell tách tệp (R1): nhánh FS/mạng là tệp nguồn riêng của cùng ELF.
    println!("cargo:rerun-if-changed=../../user/programs/shell_fs.rs");
    println!("cargo:rerun-if-changed=../../user/programs/shell_net.rs");
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
    // KT-157: hai client C/D (bốn tiến trình U-mode ở mức kernel).
    println!("cargo:rerun-if-changed=init/pager_client_cd_common.rs");
    println!("cargo:rerun-if-changed=init/pager_client_c_user.rs");
    println!("cargo:rerun-if-changed=init/pager_client_d_user.rs");
    // KT-280: hai chương trình IPC từ U-mode (client `Call` chặn + server `Recv`/`Reply`).
    println!("cargo:rerun-if-changed=init/ipc_client_user.rs");
    println!("cargo:rerun-if-changed=init/ipc_server_user.rs");
    // **KT-281**: hai chương trình **net server U-mode** (client `Call` chặn + server điều phối
    // gọi dịch vụ mạng của kernel).
    println!("cargo:rerun-if-changed=init/netcli_user.rs");
    println!("cargo:rerun-if-changed=init/netsrv_user.rs");
    // **KT-342**: ảnh đích của `exec` trên ảnh sản phẩm (`run /hello.elf`) + netsrv U-mode sản phẩm.
    println!("cargo:rerun-if-changed=../../user/programs/hello.rs");
    // **KT-347**: runtime `vifast-rt` + chương trình PoC `u1a.elf` (exec-from-FS).
    println!("cargo:rerun-if-changed=../../user/rt/lib.rs");
    println!("cargo:rerun-if-changed=../../user/runtime/bump.rs");
    println!("cargo:rerun-if-changed=../../user/programs/u1a.rs");
    println!("cargo:rerun-if-changed=../../user/programs/netsrv_prod.rs");
    println!("cargo:rerun-if-changed=../../user/runtime/lib.rs");
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
    if target != TARGET {
        return;
    }
    let dir = Path::new(&dir);
    let user_lib = compile::compile_rlib(
        TARGET,
        "vifast_user_abi",
        &dir.join("../../user/runtime/lib.rs"),
        &[],
        &[],
    );
    // KT-69: rlib `no_std` cho chương trình U-mode — hằng ABI sinh từ spec + stub IDL sinh từ
    // `.vidl` + ring (Acquire/Release, không unsafe). Không nằm trong workspace host vì mỗi
    // crate được biên dịch thẳng cho riscv64 ở đây (cùng toolchain, không cargo lồng).
    let abi_lib = rlib(dir, "vifast_abi", "../../lib/vifast-abi/src/lib.rs", &[]);
    let fs_lib = rlib(dir, "vifast_fs", "../../lib/vifast-fs/src/lib.rs", &[]);
    let ring_lib = rlib(dir, "vifast_ring", "../../lib/vifast-ring/src/lib.rs", &[]);
    let idl_lib = rlib(dir, "vifast_idl", "../../lib/vifast-idl/src/lib.rs", &[]);
    // KT-73: codec byte `conchan` + hằng console — **cùng** crate kernel dùng (`vifast-virtio`).
    let virtio_lib = rlib(
        dir,
        "vifast_virtio",
        "../../lib/vifast-virtio/src/lib.rs",
        &[],
    );
    // KT-70: pager/page-cache — **dùng lại** `vifast_fs::BlockDev` + bảng lỗi `vifast-abi`.
    let vfs_lib = rlib(
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
    elf(dir, "init_user.rs", "init.elf", &[]);
    // **KT-345b** — `procsrv` là caller thứ hai của `netsrv`; chỉ ảnh có `product-net` mới biên
    // dịch nhánh đó (ảnh test giữ **nguyên từng byte** ELF cũ: không thêm đường nào).
    let procsrv_extra: &[&str] = if cfg!(feature = "product-net") {
        &["--cfg", "product_net"]
    } else {
        &[]
    };
    elf_shared(dir, "procsrv.rs", "procsrv.elf", &shim, procsrv_extra);
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
    // **KT-342**: hai năng lực **chỉ ảnh sản phẩm** — `ping <ip>` (qua `netsrv` U-mode) và
    // `run <path>` (exec kernel-mediated). Mỗi năng lực một `--cfg` để ảnh test/ca âm giữ nguyên
    // từng byte (`cmds=6`), cùng khuôn `shell_fs` của KT-77c.
    let shell_net = std::env::var_os("CARGO_FEATURE_PRODUCT_NET").is_some();
    let shell_exec = std::env::var_os("CARGO_FEATURE_PRODUCT_EXEC").is_some();
    let mut shell_args: Vec<&str> = vec![
        "--check-cfg=cfg(shell_fs)",
        "--check-cfg=cfg(shell_net)",
        "--check-cfg=cfg(shell_exec)",
    ];
    for (on, name) in [
        (shell_fs, "shell_fs"),
        (shell_net, "shell_net"),
        (shell_exec, "shell_exec"),
    ] {
        if on {
            shell_args.push("--cfg");
            shell_args.push(name);
        }
    }
    // **KT-342 + KT-339 (merge lead)**: giữ `shell_args` ba `--cfg` của KT-342 (`shell_fs`,
    // `shell_net`, `shell_exec`) nhưng gọi **builder dùng chung** `vifast-plat-user` của KT-339
    // (`shell.rs`, không còn `shell_user.rs`) — hai bên sửa cùng một khối.
    let shell_args: &[&str] = &shell_args;
    elf_shared(dir, "shell.rs", "shell.elf", &shim_shell, shell_args);
    elf(dir, "child_user.rs", "child.elf", &shim);
    // KT-67a: chương trình U-mode invocation/memsrv (D2-97).
    elf(dir, "inv_user.rs", "inv.elf", &shim);
    // KT-67b: **hai tiến trình riêng** — memsrv (A) chuyển cap Frame, client (B) nhận + map.
    elf(dir, "memsrv_user.rs", "memsrv.elf", &shim);
    elf(dir, "client_user.rs", "client.elf", &shim);
    // KT-68: **capalloc** — server cấp/thu hồi cap + client dùng cap (hai tiến trình riêng).
    elf(dir, "capa_server_user.rs", "capa_server.elf", &shim);
    elf(dir, "capa_client_user.rs", "capa_client.elf", &shim);
    // KT-69: **hai tiến trình ring** — server producer + client consumer trên **một Frame**
    // chuyển qua cap; cả hai dùng stub IDL sinh từ `.vidl`.
    elf(dir, "ring_server_user.rs", "ring_server.elf", &shim_all);
    elf(dir, "ring_client_user.rs", "ring_client.elf", &shim_all);
    // KT-73: **consumer U-mode** của kênh UART → ring (đọc byte thật, không message cắm sẵn).
    let shim_con = [
        ("vifast_user_abi", user_lib.as_path()),
        ("vifast_ring", ring_lib.as_path()),
        ("vifast_virtio", virtio_lib.as_path()),
    ];
    elf(dir, "con_client_user.rs", "con_client.elf", &shim_con);
    // KT-74: **ba ảnh `exec`** — A/B/C chỉ in marker + `Exit` riêng, để `exec(path)` phải nạp ELF
    // **khác** thật (entry/segment khác) chứ không phải "chạy lại cùng ảnh".
    // D5 (reviewer): ảnh A phải lấy **method id từ hằng sinh tự động** (`vifast_abi::TCB_SETTLS`)
    // chứ không chép số — cùng nguồn với core, nên `const` và `method` quan sát được là **hai**
    // đường độc lập.
    elf(
        dir,
        "exec_a_user.rs",
        "exec_a.elf",
        &[
            ("vifast_user_abi", user_lib.as_path()),
            ("vifast_abi", abi_lib.as_path()),
        ],
    );
    elf(dir, "exec_b_user.rs", "exec_b.elf", &shim);
    elf(dir, "exec_c_user.rs", "exec_c.elf", &shim);
    // D = bước **không** `SetTls`: ca "TLS reset sau exec" + "SetTls rồi resume" cần ảnh không
    // tự ghi TLS (nếu không, 0 và 0xB2 không phân biệt được).
    elf(dir, "exec_d_user.rs", "exec_d.elf", &shim);
    // KT-70: **ba tiến trình** pager — server (page cache + LRU) và hai client.
    elf(dir, "pager_server_user.rs", "pager_server.elf", &shim_all);
    elf(
        dir,
        "pager_client_a_user.rs",
        "pager_client_a.elf",
        &shim_all,
    );
    elf(
        dir,
        "pager_client_b_user.rs",
        "pager_client_b.elf",
        &shim_all,
    );
    // KT-157: **bốn** client U-mode — C/D tối giản (không khối kết quả) để vừa object/arena.
    elf(
        dir,
        "pager_client_c_user.rs",
        "pager_client_c.elf",
        &shim_all,
    );
    elf(
        dir,
        "pager_client_d_user.rs",
        "pager_client_d.elf",
        &shim_all,
    );
    // **KT-280** (D2-210): **hai tiến trình IPC** — client `Call` chặn, server `Recv`+`Reply`.
    // Chỉ shim ABI (không ring/IDL/FS): hai chương trình này chỉ dùng syscall 0/1/3/12/14.
    // **KT-290**: client thêm `vifast_abi` để lấy **method id `Tcb.Configure` từ spec**
    // (`vifast_abi::TCB_CONFIGURE`) — cùng nguồn với core, không chép số tay (D2-11).
    elf(
        dir,
        "ipc_client_user.rs",
        "ipc_client.elf",
        &[
            ("vifast_user_abi", user_lib.as_path()),
            ("vifast_abi", abi_lib.as_path()),
        ],
    );
    // **KT-309**: server thêm `vifast_abi` để lấy **method id `CNode.Copy` từ spec**
    // (`vifast_abi::CNODE_COPY`) khi **dùng** cap nhận được qua IPC buffer — cùng nguồn với core.
    elf(
        dir,
        "ipc_server_user.rs",
        "ipc_server.elf",
        &[
            ("vifast_user_abi", user_lib.as_path()),
            ("vifast_abi", abi_lib.as_path()),
        ],
    );
    // **KT-281** (D2-210, N1 phần 3): **hai tiến trình net server** — client `Call` chặn, server
    // `Recv` + `Call` dịch vụ mạng của kernel + `Send` trên cap `Reply`. Chỉ shim ABI (không
    // ring/IDL/FS/virtio): server **không** chạm thiết bị, driver ở kernel.
    elf(dir, "netcli_user.rs", "netcli.elf", &shim);
    elf(dir, "netsrv_user.rs", "netsrv.elf", &shim);
    // **KT-342**: ảnh đích của `run /hello.elf` (product-exec) và net server U-mode của ảnh
    // sản phẩm (product-net). Chỉ build khi feature tương ứng **bật** ⇒ các ảnh khác không đổi
    // một byte `OUT_DIR` nào (D2-109/D2-111: không thêm đường cho ảnh cũ).
    if std::env::var_os("CARGO_FEATURE_PRODUCT_EXEC").is_some() {
        elf_shared(dir, "hello.rs", "hello.elf", &shim, &[]);
        // **KT-347 (U1a)** — `vifast-rt`: shim + rt biên dịch với `--cfg rt_alloc` (arena +
        // `GlobalAlloc`), rồi `u1a.elf` (chương trình PoC dùng `Vec`/`String`/`println!`).
        // Không ảnh nào khác nhận `rt_alloc` ⇒ ELF `shell`/`hello`/`procsrv` không đổi byte.
        let rt_alloc = [
            "--cfg",
            "rt_alloc",
            "--check-cfg=cfg(rt_alloc)",
            "--check-cfg=cfg(test)",
        ];
        let rt_shim = compile::compile_rlib(
            TARGET,
            "vifast_user_abi_rt",
            &dir.join("../../user/runtime/lib.rs"),
            &[],
            &rt_alloc,
        );
        let rt_lib = compile::compile_rlib(
            TARGET,
            "vifast_rt",
            &dir.join("../../user/rt/lib.rs"),
            &[("vifast_user_abi", rt_shim.as_path())],
            &rt_alloc,
        );
        elf_shared(
            dir,
            "u1a.rs",
            "u1a.elf",
            &[
                ("vifast_rt", rt_lib.as_path()),
                ("vifast_user_abi", rt_shim.as_path()),
            ],
            &rt_alloc,
        );
        // Runner host seed đĩa FS bằng `vifast-fsutil` — xuất ELF ra đường dẫn **xác định** dưới
        // `target/<triple>/release/user-elfs/` (không phải OUT_DIR có hash).
        compile::export_user_elf("hello.elf");
        compile::export_user_elf("u1a.elf");
    }
    if std::env::var_os("CARGO_FEATURE_PRODUCT_NET").is_some() {
        // **KT-346 (gộp, lead giải xung đột)** — tệp đã dời sang `v2/user/programs/netsrv_prod.rs`
        // (KT-346) **và** KT-345b thêm đột biến `mut-net-one-caller`; giữ **cả hai**: builder
        // `_shared_with` (đường dẫn mới) + `extra` của đột biến.
        let mut extra: Vec<&str> = Vec::new();
        if std::env::var_os("CARGO_FEATURE_MUT_NET_ONE_CALLER").is_some() {
            extra.push("--cfg");
            extra.push("mut_net_one_caller");
        }
        elf_shared(dir, "netsrv_prod.rs", "netsrv_prod.elf", &shim, &extra);
    }
}

/// Biên dịch một `rlib` `no_std` cho riscv64 từ nguồn **tương đối với thư mục crate ảnh**.
fn rlib(dir: &Path, crate_name: &str, source: &str, externs: &[(&str, &Path)]) -> PathBuf {
    compile::compile_rlib(TARGET, crate_name, &dir.join(source), externs, &[])
}

/// Biên dịch một chương trình U-mode có nguồn trong `init/` của **ảnh này** (`init/init.ld`).
fn elf(dir: &Path, source_name: &str, out_name: &str, externs: &[(&str, &Path)]) {
    elf_at(&dir.join("init"), dir, source_name, out_name, externs, &[]);
}

/// **KT-339** — biên dịch chương trình dùng chung `v2/user/programs/` (procsrv, shell, hello…).
///
/// Nguồn **không** nằm trong `init/` của ảnh riscv64 nữa: cùng một tệp `programs/*.rs` được
/// **cả** `vifast-kbin-riscv64` **và** `vifast-kbin-aarch64` biên dịch cho target của mình —
/// đúng D2-233 mục (3): "cùng một mã nguồn `procsrv`/`shell` chạy trên cả 3 kiến trúc".
fn elf_shared(
    dir: &Path,
    source_name: &str,
    out_name: &str,
    externs: &[(&str, &Path)],
    extra: &[&str],
) {
    elf_at(
        &dir.join("../../user/programs"),
        dir,
        source_name,
        out_name,
        externs,
        extra,
    );
}

/// Thân chung: `src_dir` = thư mục chứa nguồn chương trình, `dir` = thư mục crate ảnh (cwd và
/// script linker).
fn elf_at(
    src_dir: &Path,
    dir: &Path,
    source_name: &str,
    out_name: &str,
    externs: &[(&str, &Path)],
    extra: &[&str],
) {
    compile::compile_user_elf(
        TARGET,
        dir,
        &src_dir.join(source_name),
        out_name,
        &dir.join("init/init.ld"),
        externs,
        extra,
        &LINK_ARGS,
    );
}
