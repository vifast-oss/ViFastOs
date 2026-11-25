// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đăng ký linker script + cờ liên kết cho **cả ba** ảnh riscv64 K3 (KT-44, KT-53) và khai báo
//! artefact U-mode của arch này (ảnh sản phẩm `vifast-kbin-riscv64`, ảnh test `vifast-kbin-ktest*`).
//!
//! # KT-404 / KT-404c — build script này chỉ còn **khai báo**
//!
//! Máy móc dùng chung (danh tính bản dựng + gọi `$RUSTC` + `--remap-path-prefix` + sinh linker
//! script + vòng dựng artefact) ở crate [`vifast_kbuild`]; tệp này chỉ còn **dữ liệu của arch
//! này**: tệp phải theo dõi ([`WATCH`]), bảng chương trình U-mode ([`INIT_PROGRAMS`]) và `--cfg`
//! của shell sản phẩm. Ba bản sao trước `KT-404` đã **trôi** định nghĩa `dirty`: bản riscv64 tính
//! trên `v2/kernel v2/lib` **thiếu `v2/user`** trong khi `v2/user` nằm trong `VBREV_INPUT_ROOTS`
//! của `build-rev.sh` (KT-346) ⇒ sửa `v2/user` làm `inputs` đổi mà ảnh vẫn khai `dirty=0`.
//!
//! Khác aarch64/x86_64, riscv64 liên kết **mọi** chương trình bằng **cùng** `init/init.ld`
//! (`.stack` NOLOAD + `.stack_guard`, D2-72 Q5): kernel nạp ELF vào VSpace riêng nên không cần VA
//! tuyệt đối tách rời ⇒ [`Link::Fixed`], không phải [`Link::Generated`].

use std::path::Path;

use vifast_kbuild::cargo;
use vifast_kbuild::compile;
use vifast_kbuild::identity;
use vifast_kbuild::user::{self, Build, Link, Program, Rlib, Set};

/// Target của ảnh này (một chỗ duy nhất — trước KT-404 chuỗi này lặp 6 lần trong tệp).
const TARGET: &str = "riscv64gc-unknown-none-elf";

/// Cờ `-C link-arg=…` riêng của arch: riscv64 cấm linker nới rộng `c.jal`/`c.j` (relaxation) vì
/// `vifast-elf` nạp segment theo đúng offset đã liên kết.
const LINK_ARGS: [&str; 1] = ["--no-relax"];

// ---- nhóm `--extern` của chương trình U-mode: chỉ số vào bảng `sets` của từng lượt dựng.
/// Chỉ shim ABI (`vifast-user-abi`): chương trình chỉ dùng syscall, không ring/IDL/FS.
const SET_SHIM: Set = 1;
/// shim + ring + IDL + VFS + FS: client/server ring & pager (KT-69/KT-70/KT-157).
const SET_ALL: Set = 2;
/// shim + ring + codec `conchan` của virtio: shell in/đọc byte thật từ ring UART (KT-73).
const SET_SHELL: Set = 3;
/// shim + ring + virtio: consumer ring UART → U-mode (KT-73).
const SET_CON: Set = 4;
/// shim + `vifast-abi`: lấy method id **từ spec** (D2-11) thay vì chép số (KT-290/KT-309).
const SET_ABI: Set = 5;

/// Mọi tệp nguồn mà artefact U-mode phụ thuộc — cargo **phải** chạy lại build script khi đổi
/// (`linker.ld` do [`cargo::link_kernel`] khai; `init/init.ld` là linker script user ở đây).
const WATCH: &[&str] = &[
    "init/init_user.S",
    "../../user/programs/shell_cmds.rs",
    "../../user/programs/shell_fs.rs",
    "../../user/programs/shell_net.rs",
    "init/inv_user.S",
    "init/pager_common.rs",
    "init/pager_client_common.rs",
    "init/pager_client_cd_common.rs",
    "init/fpu_user.S",
    "../../user/rt/lib.rs",
    "../../user/runtime/bump.rs",
    "../../user/runtime/lib.rs",
    "../../lib/vifast-abi/src/lib.rs",
    "../../lib/vifast-abi/src/generated.rs",
    "../../lib/vifast-ring/src/lib.rs",
    "../../lib/vifast-idl/src/lib.rs",
    "../../lib/vifast-idl/src/generated.rs",
    "../../lib/vifast-idl/src/generated_pager.rs",
    "../../lib/vifast-vfs/src/lib.rs",
    "../../lib/vifast-vfs/src/cache.rs",
    "../../lib/vifast-vfs/src/core.rs",
    "../../lib/vifast-vfs/src/disk.rs",
    "../../lib/vifast-vfs/src/layout.rs",
    "../../lib/vifast-vfs/src/stats.rs",
    "../../lib/vifast-vfs/src/status.rs",
    "../../lib/vifast-fs/src/lib.rs",
    "init/init.ld",
];

/// Chương trình U-mode có nguồn trong `init/` của **ảnh này** (liên kết bằng `init/init.ld`).
const INIT_PROGRAMS: &[Program<'_>] = &[
    Program::new("init_user.rs", "init.elf", 0),
    Program::new("child_user.rs", "child.elf", SET_SHIM),
    Program::new("inv_user.rs", "inv.elf", SET_SHIM),
    // KT-67b: **hai tiến trình riêng** — memsrv (A) chuyển cap Frame, client (B) nhận + map.
    Program::new("memsrv_user.rs", "memsrv.elf", SET_SHIM),
    Program::new("client_user.rs", "client.elf", SET_SHIM),
    // KT-68: **capalloc** — server cấp/thu hồi cap + client dùng cap.
    Program::new("capa_server_user.rs", "capa_server.elf", SET_SHIM),
    Program::new("capa_client_user.rs", "capa_client.elf", SET_SHIM),
    // KT-69: hai tiến trình ring trên **một** Frame chuyển qua cap, dùng stub IDL sinh từ `.vidl`.
    Program::new("ring_server_user.rs", "ring_server.elf", SET_ALL),
    Program::new("ring_client_user.rs", "ring_client.elf", SET_ALL),
    Program::new("con_client_user.rs", "con_client.elf", SET_CON),
    // KT-74: **bốn** ảnh `exec` — A/B/C in marker + `Exit` riêng để `exec(path)` phải nạp ELF
    // **khác** thật; D **không** `SetTls` (ca "TLS reset sau exec").
    Program::new("exec_a_user.rs", "exec_a.elf", SET_ABI),
    Program::new("exec_b_user.rs", "exec_b.elf", SET_SHIM),
    Program::new("exec_c_user.rs", "exec_c.elf", SET_SHIM),
    Program::new("exec_d_user.rs", "exec_d.elf", SET_SHIM),
    // KT-70 + KT-157: pager server (page cache + LRU) và **bốn** client.
    Program::new("pager_server_user.rs", "pager_server.elf", SET_ALL),
    Program::new("pager_client_a_user.rs", "pager_client_a.elf", SET_ALL),
    Program::new("pager_client_b_user.rs", "pager_client_b.elf", SET_ALL),
    Program::new("pager_client_c_user.rs", "pager_client_c.elf", SET_ALL),
    Program::new("pager_client_d_user.rs", "pager_client_d.elf", SET_ALL),
    // KT-280 (D2-210): client `Call` chặn + server `Recv`/`Reply` (KT-309: cap qua IPC buffer).
    Program::new("ipc_client_user.rs", "ipc_client.elf", SET_ABI),
    Program::new("ipc_server_user.rs", "ipc_server.elf", SET_ABI),
    // KT-281 (D2-210, N1 phần 3): net server U-mode — driver mạng ở kernel, server không chạm
    // thiết bị, nên chỉ shim ABI.
    Program::new("netcli_user.rs", "netcli.elf", SET_SHIM),
    Program::new("netsrv_user.rs", "netsrv.elf", SET_SHIM),
    // KT-418: nạp `f0..f31` mẫu riêng, `Yield` 8 lần, kiểm lại **toàn bộ** 32 thanh ghi.
    Program::new("fpu_user.rs", "fpu.elf", SET_SHIM),
];

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    // **KT-86** (D2-125.3): danh tính bản dựng (`git rev` 12 hex + cờ dirty) — một nguồn cho cả
    // ba arch, xem `vifast_kbuild::identity`.
    identity::build_identity(&dir, "vifast-kbin-riscv64");
    cargo::link_kernel(&dir, &[]);
    cargo::rerun_if_changed(WATCH);

    // Chỉ ảnh test/sản phẩm mới cần ELF user; ảnh tối giản (feature off) không sinh/không nhúng.
    if !cargo::feature("USER") || !cargo::target_is(TARGET) {
        return;
    }
    let dir = Path::new(&dir);
    build_artefacts(dir, &dir.join("init"));
}

/// Đặc tả một lượt dựng: mọi trường cố định của ảnh này, chỉ `sets`/`programs`/`src_dir`/`extra`
/// đổi theo lượt — linker script và cờ arch giữ nguyên một chỗ.
fn spec<'a>(
    dir: &'a Path,
    script: &'a Path,
    src_dir: &'a Path,
    sets: &'a [&'a [(&'a str, &'a Path)]],
    programs: &'a [Program<'a>],
    extra: &'a [&'a str],
) -> Build<'a> {
    Build {
        target: TARGET,
        cwd: dir,
        src_dir,
        link: Link::Fixed(script),
        extra,
        link_args: &LINK_ARGS,
        sets,
        programs,
    }
}

/// Dựng `rlib` của shim + các lib dùng chung, rồi các lượt [`user::build`] của arch này.
fn build_artefacts(dir: &Path, init_dir: &Path) {
    let programs_dir = dir.join("../../user/programs");
    let init_ld = init_dir.join("init.ld");
    // KT-69/70/73: `rlib` `no_std` cho chương trình U-mode — hằng ABI sinh từ spec + stub IDL sinh
    // từ `.vidl` + ring (Acquire/Release, không unsafe), biên dịch thẳng cho riscv64 ở đây (cùng
    // toolchain, không cargo lồng). Pager dùng lại `vifast_fs::BlockDev`; codec `conchan` của
    // `vifast-virtio` là **cùng** crate kernel dùng.
    let libs = user::rlibs(
        dir,
        TARGET,
        &[
            Rlib::new("vifast_user_abi", "../../user/runtime/lib.rs", &[]),
            Rlib::new("vifast_abi", "../../lib/vifast-abi/src/lib.rs", &[]),
            Rlib::new("vifast_fs", "../../lib/vifast-fs/src/lib.rs", &[]),
            Rlib::new("vifast_ring", "../../lib/vifast-ring/src/lib.rs", &[]),
            Rlib::new("vifast_idl", "../../lib/vifast-idl/src/lib.rs", &[]),
            Rlib::new("vifast_virtio", "../../lib/vifast-virtio/src/lib.rs", &[]),
            Rlib::new(
                "vifast_vfs",
                "../../lib/vifast-vfs/src/lib.rs",
                &[("vifast_fs", 2), ("vifast_abi", 1)],
            ),
        ],
    );
    let p = |i: usize| libs[i].as_path();
    let shim = [("vifast_user_abi", p(0))];
    let shim_abi = [("vifast_user_abi", p(0)), ("vifast_abi", p(1))];
    let shim_all = [
        ("vifast_user_abi", p(0)),
        ("vifast_abi", p(1)),
        ("vifast_ring", p(3)),
        ("vifast_idl", p(4)),
        ("vifast_vfs", p(6)),
        ("vifast_fs", p(2)),
    ];
    // KT-73: shell đọc **byte thật** từ ring UART → `vifast_ring` + codec `conchan`; consumer
    // ring dùng đúng ba `rlib` đó (`shim_con`).
    let shim_con = [
        ("vifast_user_abi", p(0)),
        ("vifast_ring", p(3)),
        ("vifast_virtio", p(5)),
    ];
    let sets: [&[(&str, &Path)]; 6] = [&[], &shim, &shim_all, &shim_con, &shim_con, &shim_abi];

    // 1) chương trình U-mode trong `init/` của ảnh này.
    user::build(&spec(dir, &init_ld, init_dir, &sets, INIT_PROGRAMS, &[]));

    // 2) `procsrv` + `shell` — **cùng nguồn** `v2/user/programs/` với aarch64 (D2-233 mục 3).
    // **KT-345b** — `procsrv` là caller thứ hai của `netsrv`; chỉ ảnh có `product-net` mới biên
    // dịch nhánh đó (ảnh test giữ **nguyên từng byte** ELF cũ). GIỮ NGUYÊN `cfg!(feature = …)`
    // của bản trước `KT-404c`: `cfg!` trong build script **không** thấy feature của crate (feature
    // đến bằng `CARGO_FEATURE_*`) nên nhánh này luôn rỗng; đổi sang `cargo::feature` sẽ **đổi
    // byte** ELF `procsrv` ⇒ ngoài phạm vi task này (nợ đã ghi trong journal).
    let procsrv_extra: &[&str] = if cfg!(feature = "product-net") {
        &["--cfg", "product_net"]
    } else {
        &[]
    };
    // **KT-77c/KT-342** — ba năng lực **chỉ ảnh sản phẩm**: `ls`/`cat`/`write` (FS của kernel),
    // `ping <ip>` (qua `netsrv` U-mode), `run <path>` (exec kernel-mediated). Mỗi năng lực một
    // `--cfg` để ảnh test/ca âm "shell nhưng không FS" giữ nguyên từng byte (`help cmds=6`) và
    // kỳ vọng của harness **không** phải sửa.
    let shell_args = cargo::cfgs(&[
        ("shell_fs", Some(cargo::feature("PRODUCT_FS"))),
        ("shell_net", Some(cargo::feature("PRODUCT_NET"))),
        ("shell_exec", Some(cargo::feature("PRODUCT_EXEC"))),
    ]);
    let shell_args: Vec<&str> = shell_args.iter().map(String::as_str).collect();
    let shared = [
        Program::new("procsrv.rs", "procsrv.elf", SET_SHIM).with(procsrv_extra),
        Program::new("shell.rs", "shell.elf", SET_SHELL).with(&shell_args),
    ];
    user::build(&spec(dir, &init_ld, &programs_dir, &sets, &shared, &[]));

    // 3) **KT-342**: ảnh đích của `run /hello.elf` (product-exec) và net server U-mode của ảnh
    // sản phẩm (product-net). Chỉ build khi feature tương ứng **bật** ⇒ các ảnh khác không đổi
    // một byte `OUT_DIR` nào (D2-109/D2-111: không thêm đường cho ảnh cũ).
    if cargo::feature("PRODUCT_EXEC") {
        // **KT-347 (U1a)** — `vifast-rt`: shim + rt biên dịch với `--cfg rt_alloc` (arena +
        // `GlobalAlloc`), rồi `u1a.elf` (PoC `Vec`/`String`/`println!`). Không ảnh nào khác nhận
        // `rt_alloc` ⇒ ELF `shell`/`hello`/`procsrv` không đổi byte.
        let rt_alloc = [
            "--cfg",
            "rt_alloc",
            "--check-cfg=cfg(rt_alloc)",
            "--check-cfg=cfg(test)",
        ];
        let rt = user::rlibs(
            dir,
            TARGET,
            &[
                Rlib::new("vifast_user_abi_rt", "../../user/runtime/lib.rs", &[]).with(&rt_alloc),
                Rlib::new(
                    "vifast_rt",
                    "../../user/rt/lib.rs",
                    &[("vifast_user_abi", 0)],
                )
                .with(&rt_alloc),
            ],
        );
        let rt_set = [
            ("vifast_rt", rt[1].as_path()),
            ("vifast_user_abi", rt[0].as_path()),
        ];
        let exec_sets: [&[(&str, &Path)]; 2] = [&shim, &rt_set];
        let execs = [
            Program::new("hello.rs", "hello.elf", 0),
            Program::new("u1a.rs", "u1a.elf", 1).with(&rt_alloc),
        ];
        user::build(&spec(dir, &init_ld, &programs_dir, &exec_sets, &execs, &[]));
        // Runner host seed đĩa FS bằng `vifast-fsutil` — xuất ELF ra đường dẫn **xác định** dưới
        // `target/<triple>/release/user-elfs/` (không phải OUT_DIR có hash).
        compile::export_user_elf("hello.elf");
        compile::export_user_elf("u1a.elf");
    }
    if cargo::feature("PRODUCT_NET") {
        // **KT-346 (gộp, lead giải xung đột)** — tệp đã dời sang `v2/user/programs/netsrv_prod.rs`
        // (KT-346) **và** KT-345b thêm đột biến `mut-net-one-caller`; giữ **cả hai**.
        let mut net_extra: Vec<&str> = Vec::new();
        if cargo::feature("MUT_NET_ONE_CALLER") {
            net_extra.push("--cfg");
            net_extra.push("mut_net_one_caller");
        }
        let net_sets: [&[(&str, &Path)]; 1] = [&shim];
        let netsrv = [Program::new("netsrv_prod.rs", "netsrv_prod.elf", 0)];
        user::build(&spec(
            dir,
            &init_ld,
            &programs_dir,
            &net_sets,
            &netsrv,
            &net_extra,
        ));
    }
}
