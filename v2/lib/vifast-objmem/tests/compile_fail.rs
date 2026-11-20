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

/// Rlib của `vifast-objmem` mà test binary đang dùng.
///
/// **KT-242 (nâng toolchain 1.100.0)** — cargo ≥ 1.100 đổi **bố cục artifact**: binary test
/// nằm ở `<target>/debug/build/<pkg>/<hash>/out/` (không còn `<target>/debug/deps/`), và rlib
/// của lib nằm ở **một thư mục hash khác** cùng cấp. Vì vậy không thể đọc `exe.parent()` nữa:
/// phải tìm gốc `debug` (thư mục có `build/` hoặc `deps/`) rồi quét đệ quy. Cách này chạy
/// đúng cho **cả hai** bố cục (1.99.1 và 1.100.0) nên không phải sửa lần nữa khi đổi tiếp.
fn rlib_objmem() -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let mut root = exe.parent().expect("thư mục chứa binary").to_path_buf();
    // Đi lên tối đa 8 cấp; dừng ở thư mục có `build/` hoặc `deps/` (gốc `debug` của cargo).
    let mut len = 0;
    while !(root.join("build").is_dir() || root.join("deps").is_dir()) {
        len += 1;
        if len > 8 || !root.pop() {
            break;
        }
    }
    let mut moi_nhat: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
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
    }
    moi_nhat
        .map(|(_, p)| p)
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
    // **KT-242 (A0 duyệt 2026-11-20)** — thư mục tạm **duy nhất theo lần gọi** (PID + đồng hồ
    // nano) và **có cleanup**. PID một mình không đủ: nhiều test trong cùng một process có thể
    // dùng cùng tên, và tệp `.o` còn lại làm lần chạy sau đọc nhầm.
    let tmp = std::env::temp_dir().join(format!(
        "vifast_objmem_n9_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&tmp).expect("tạo thư mục tạm");
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
        .arg(tmp.join("n9.o"))
        .arg(&nguon)
        .output()
        .unwrap_or_else(|e| panic!("không chạy được {rustc}: {e}"));
    // Cleanup **trước** mọi assert: đỏ vì lý do gì cũng không để rác lại.
    let _ = std::fs::remove_dir_all(&tmp);
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
