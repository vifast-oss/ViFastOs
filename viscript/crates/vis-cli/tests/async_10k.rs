// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-60 (VD-90/VD-95) — benchmark 10.000 tác vụ `async`**: test **thật** (không `#[ignore]`).
//!
//! Chốt VD-90 (5): 10.000 tác vụ dùng **hàng đợi phẳng** (`Vec`), **không** đệ quy theo số tác vụ
//! ⇒ không tràn stack. Test này sinh chương trình 10k `spawn` trong **một** vòng lặp rồi để **join
//! cuối** chạy hết, chạy **cả hai** đường (`run` và `run --vm --no-cache`) và khoá:
//! - đủ **10.001** dòng stdout (`main` trước, rồi `0`..`9999` **theo thứ tự tạo**),
//! - hai đường **cùng byte**,
//! - thời gian mỗi đường dưới **60 s** (đo thật ~0,2 s; ngưỡng rộng để không flake trên máy bận —
//!   số đo thật in ra ở cuối mỗi lần chạy).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-async10k-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

/// Sinh chương trình: 10.000 `spawn` trong **một** vòng lặp; `main` in trước, join cuối chạy hết
/// theo **thứ tự tạo** nên stdout là `main`, `0`, `1`, …, `9999`.
fn chuong_trinh() -> String {
    "# SPDX-License-Identifier: MIT OR Apache-2.0\n\
     async fn viec(n: int) -> int:\n\
     \x20   print(n)\n\
     \x20   n\n\
     \n\
     var i = 0\n\
     while i < 10000:\n\
     \x20   let t = spawn viec(i)\n\
     \x20   i = i + 1\n\
     print(\"main\")\n"
        .to_string()
}

fn chay(p: &Path, vm: bool) -> (i32, String, String, u128) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm");
        c.arg("--no-cache");
    }
    let t0 = Instant::now();
    let o = c.arg(p).output().expect("chạy `vis run`");
    let ms = t0.elapsed().as_millis();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
        ms,
    )
}

#[test]
fn muoi_nghin_task_khong_tran_stack() {
    let d = tam("chinh");
    let p = d.join("k10.vis");
    std::fs::write(&p, chuong_trinh()).expect("ghi chương trình");

    let (c1, o1, e1, ms1) = chay(&p, false);
    assert_eq!(c1, 0, "interp: {e1}");
    let dong1: Vec<&str> = o1.lines().collect();
    assert_eq!(
        dong1.len(),
        10_001,
        "phải in `main` + 10.000 dòng của tác vụ, được {}",
        dong1.len()
    );
    assert_eq!(dong1[0], "main", "`spawn` chỉ xếp hàng — main in trước");
    assert_eq!(dong1[1], "0", "join cuối theo **thứ tự tạo**");
    assert_eq!(dong1[10_000], "9999", "tác vụ cuối vẫn chạy");

    let (c2, o2, e2, ms2) = chay(&p, true);
    assert_eq!(c2, 0, "vm: {e2}");
    assert_eq!(o1, o2, "hai đường phải **cùng byte** stdout");
    assert_eq!(e1, e2, "hai đường phải **cùng byte** stderr");

    // Ngưỡng **rộng** (đo thật ~0,2 s/đường): chỉ để bắt hồi quy bậc thuật toán (10k task phải
    // tuyến tính), **không** phải ngưỡng hiệu năng chặt.
    assert!(ms1 < 60_000, "interp chậm bất thường: {ms1} ms");
    assert!(ms2 < 60_000, "vm chậm bất thường: {ms2} ms");
    println!("10k task: interp {ms1} ms, vm {ms2} ms");

    let _ = std::fs::remove_dir_all(&d);
}
