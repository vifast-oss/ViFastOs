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

/// Rlib của `vifast-objmem` — lấy từ **artifact Cargo của đúng package/profile đang kiểm**.
///
/// **KT-242 (A0 duyệt 2026-11-20)** — **không** quét thư mục rồi lấy bản "mới nhất" hay "đầu
/// tiên": target dir có thể chứa nhiều rlib cùng crate (feature, profile, lần build khác) và
/// chọn sai nghĩa là test kiểm nhầm mã. Cách đúng: hỏi **cargo** bằng `--message-format=json`
/// (cùng manifest, cùng profile `dev` mà `cargo test` dùng để link integration test), lấy
/// `filenames` của artifact `vifast-objmem`. Không đúng **một** rlib ⇒ **thất bại rõ**, không
/// đoán.
fn rlib_objmem() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let out = Command::new(&cargo)
        .args(["build", "--manifest-path"])
        .arg(&manifest)
        .args(["-p", "vifast-objmem", "--message-format=json"])
        .output()
        .unwrap_or_else(|e| panic!("không chạy được {cargo}: {e}"));
    assert!(
        out.status.success(),
        "cargo build -p vifast-objmem đỏ:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cands = chon_rlib(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(
        cands.len(),
        1,
        "phải có đúng MỘT rlib của vifast-objmem từ cargo (không đoán khi mơ hồ), thấy: {cands:?}"
    );
    cands.into_iter().next().expect("đã kiểm len == 1")
}

/// Lọc rlib của `vifast-objmem` từ đầu ra `cargo build --message-format=json`.
///
/// Tách khỏi `rlib_objmem` để **kiểm được** ca nhiều ứng viên / artifact cũ mà không cần dựng
/// target dir thật: đầu vào là JSON, đầu ra là danh sách ứng viên; tầng gọi đòi **đúng một**.
fn chon_rlib(artifacts_json: &str) -> Vec<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    for line in artifacts_json.lines() {
        if !line.contains("\"reason\":\"compiler-artifact\"")
            || !line.contains("\"name\":\"vifast_objmem\"")
        {
            continue;
        }
        for f in filenames_of(line) {
            if f.ends_with(".rlib") {
                cands.push(PathBuf::from(f));
            }
        }
    }
    cands.sort();
    cands.dedup();
    cands
}

/// Trích mảng `"filenames":[…]` của một dòng JSON `compiler-artifact`.
fn filenames_of(line: &str) -> Vec<String> {
    const K: &str = "\"filenames\":[";
    let Some(i) = line.find(K) else {
        return Vec::new();
    };
    let start = i + K.len();
    let Some(j) = line[start..].find(']') else {
        return Vec::new();
    };
    line[start..start + j]
        .split("\",\"")
        .map(|s| s.trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// **KT-242** — chọn rlib phải **tất định** và **thất bại rõ** khi mơ hồ.
#[test]
fn chon_rlib_tu_artifact_json() {
    // Một artifact, một rlib ⇒ đúng một ứng viên.
    let mot = r#"{"reason":"compiler-artifact","target":{"name":"vifast_objmem"},"filenames":["/t/libvifast_objmem-aaaa.rlib","/t/libvifast_objmem-aaaa.rmeta"]}"#;
    assert_eq!(chon_rlib(mot).len(), 1, "một artifact ⇒ một rlib");
    // Artifact cũ + artifact mới cùng crate ⇒ **hai** ứng viên; tầng gọi phải từ chối.
    let hai = format!(
        "{mot}\n{}",
        r#"{"reason":"compiler-artifact","target":{"name":"vifast_objmem"},"filenames":["/t/libvifast_objmem-bbbb.rlib"]}"#
    );
    assert_eq!(
        chon_rlib(&hai).len(),
        2,
        "artifact cũ + mới ⇒ hai ứng viên (không tự chọn)"
    );
    // Artifact của crate khác ⇒ không lấy.
    let khac = r#"{"reason":"compiler-artifact","target":{"name":"vifast-alloc"},"filenames":["/t/libvifast_alloc-cccc.rlib"]}"#;
    assert!(chon_rlib(khac).is_empty(), "crate khác ⇒ không lấy");
    // Chỉ có .rmeta (chưa sinh rlib) ⇒ không có ứng viên ⇒ tầng gọi thất bại rõ.
    let meta = r#"{"reason":"compiler-artifact","target":{"name":"vifast_objmem"},"filenames":["/t/libvifast_objmem-dddd.rmeta"]}"#;
    assert!(chon_rlib(meta).is_empty(), "chỉ .rmeta ⇒ không có rlib");
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
