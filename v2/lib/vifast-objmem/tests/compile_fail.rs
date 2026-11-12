// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hồi quy **lúc biên dịch**: `with_many_mut` với `N = 9` phải **không build** (KT-15).
//!
//! DoD K1A-PLAN §4/KT-15 mục 1: trần `N ≤ 8` cưỡng chế lúc biên dịch. Không thêm
//! dependency `trybuild` (D2-21), test này tự gọi `rustc` trên
//! `tests/compile-fail/n9.rs` với rlib của chính crate và khẳng định:
//! 1. `rustc` thoát **khác 0**;
//! 2. stderr chứa thông điệp trần `N <= 8` (không phải đỏ vì lý do khác).
//!
//! Dưới Miri: bỏ qua **có lý do** — Miri cấm spawn tiến trình, còn đây là test về biên
//! dịch chứ không phải về bộ nhớ; toàn bộ test alias/fuzz vẫn chạy đầy đủ dưới Miri
//! (xem `kt15-miri*.log`).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Rlib của `vifast-objmem` mà test binary đang dùng (cùng thư mục `deps`).
fn rlib_objmem() -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let deps = exe.parent().expect("thư mục deps").to_path_buf();
    let mut moi_nhat: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&deps).expect("đọc deps") {
        let p = entry.expect("entry").path();
        let ten = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if ten.starts_with("libvifast_objmem-") && ten.ends_with(".rlib") {
            let t = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if moi_nhat.as_ref().is_none_or(|(t0, _)| t > *t0) {
                moi_nhat = Some((t, p));
            }
        }
    }
    moi_nhat
        .map(|(_, p)| p)
        .or_else(|| {
            // Dự phòng: artifact lib ở `<target>/debug/libvifast_objmem.rlib`.
            let debug = deps.parent()?;
            let p = debug.join("libvifast_objmem.rlib");
            p.is_file().then_some(p)
        })
        .expect("phải tìm thấy rlib của vifast-objmem")
}

#[test]
fn n9_khong_build_va_stderr_neu_tran_n() {
    if cfg!(miri) {
        eprintln!("compile_fail: bỏ qua dưới Miri (Miri cấm spawn rustc; test này về biên dịch)");
        return;
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let nguon = manifest.join("tests/compile-fail/n9.rs");
    assert!(nguon.is_file(), "thiếu {}", nguon.display());
    let rlib = rlib_objmem();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let out = Command::new(&rustc)
        .args([
            "--edition",
            "2021",
            "--crate-name",
            "n9",
            // `--emit=obj` (không phải metadata): lỗi `const { assert!(N <= 8) }` chỉ lộ ra
            // khi monomorphize + sinh mã cho `with_many_mut::<9, ...>`.
            "--emit=obj",
            "-L",
        ])
        .arg(format!(
            "dependency={}",
            rlib.parent().expect("deps").display()
        ))
        .arg("--extern")
        .arg(format!("vifast_objmem={}", rlib.display()))
        .arg("-o")
        .arg(std::env::temp_dir().join("vifast_objmem_n9.o"))
        .arg(&nguon)
        .output()
        .unwrap_or_else(|e| panic!("không chạy được {rustc}: {e}"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "N = 9 phải không build, nhưng rustc thoát 0 (stdout: {})",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        stderr.contains("N <= 8"),
        "stderr phải nêu trần 'N <= 8' (đỏ vì đúng const assert), nhận được:\n{stderr}"
    );
}
