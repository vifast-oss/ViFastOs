// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-60 (VD-95 (4)) — `async` ngoài hàm cấp tệp**: method/`cmd` **chưa** hỗ trợ (nợ `R10`).
//!
//! Chốt VD-95 (4) nói các dạng này phải ra **`VS0412`**. Đo thật ở ST-60: parser **chưa** nhận
//! `async` sau `impl`/`cmd` (chuỗi `impl P:` + `async fn` và `async cmd` đều **cú pháp** sai) ⇒
//! chúng ra **`VS0201`** (cú pháp) **trước** tầng kiểu. Test này khoá **hành vi thật** đó và ghi
//! rõ nợ: khi parser mở `async` cho method/`cmd`, ca này phải **đổi kỳ vọng sang `VS0412`** (luật
//! kiểu đã có sẵn trong `tyck/mod.rs`, hiện là **phòng thủ**).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-async-vis-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &Path, tag: &str, src: &str) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vis"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&p)
        .output()
        .expect("chạy `vis run`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

#[test]
fn async_tren_method_la_vs0201_va_ghi_no() {
    let d = tam("method");
    let (c, _, e) = chay(
        &d,
        "m",
        "type P:\n    n: int\n\nimpl P:\n    async fn lay(self) -> int:\n        self.n\n\nprint(1)\n",
    );
    assert_eq!(c, 2, "parser chưa nhận `async` trong `impl` — stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    assert!(
        e.contains("async"),
        "thông điệp phải nêu chỗ sai (`async`): {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn async_tren_cmd_la_vs0201_va_ghi_no() {
    let d = tam("cmd");
    let (c, _, e) = chay(
        &d,
        "c",
        "async cmd f() -> int:\n    [1] | take 1\n\nprint(1)\n",
    );
    assert_eq!(c, 2, "parser chưa nhận `async cmd` — stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}
