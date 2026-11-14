// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-98 (VD-112)** — `bin/vis-soak` chỉ **mô tả** 8 vòng tuần tự (không chạy soak trong test):
//! tồn tại, có SPDX, `set -u`, không dùng `--ignore`/`|| true` để “chạy cho có”, và `--dry-run` in
//! **đúng 8 tên vòng**.

use std::path::PathBuf;
use std::process::Command;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin/vis-soak")
}

const VONG: [&str; 8] = [
    "gate-1", "gate-2", "diff-1", "diff-2", "host-1", "host-2", "cache-1", "cache-2",
];

#[test]
fn vis_soak_hop_le_va_in_du_8_vong() {
    let p = script();
    assert!(p.exists(), "thiếu {}", p.display());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p)
            .expect("metadata")
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "bin/vis-soak phải +x (mode {mode:o})");
    }
    let s = std::fs::read_to_string(&p).expect("đọc script");
    assert!(
        s.lines()
            .any(|l| l.starts_with("# SPDX-License-Identifier: MIT OR Apache-2.0")),
        "thiếu dòng SPDX `#`"
    );
    assert!(s.contains("set -u"), "thiếu `set -u`");
    assert!(!s.contains("--ignore"), "không được né bằng --ignore");
    assert!(!s.contains("|| true"), "không được `|| true`");
    assert!(
        !s.contains("#[ignore]") && !s.contains("#[should_panic]"),
        "không được tắt/ẩn ca"
    );

    // `--dry-run` chỉ in danh sách, không chạy soak.
    let o = Command::new(script())
        .arg("--dry-run")
        .output()
        .expect("chạy --dry-run");
    assert!(
        o.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8_lossy(&o.stdout);
    let dong: Vec<&str> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(dong, VONG, "danh sách vòng: {out:?}");
}
