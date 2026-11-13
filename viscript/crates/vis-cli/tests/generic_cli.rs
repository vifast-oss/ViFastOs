// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-28 — generic qua **binary**: (1) `check` và `run` phải **cùng kết luận**, (2) VM chạy `fn main`
//! **như interp** (bug phát hiện ở ST-28: VM bỏ qua `main`), (3) thông điệp mâu thuẫn kiểu có `T`.

use std::process::Command;

const S: &str = "# SPDX-License-Identifier: MIT OR Apache-2.0\n";

fn tam(ten: &str, body: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vis-gen-{}-{}", ten, std::process::id()));
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    let p = d.join(format!("{ten}.vis"));
    std::fs::write(&p, format!("{S}{body}")).expect("ghi");
    p
}

fn chay(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(args)
        .output()
        .expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

const DUNG: &str =
    "fn cong[T](a: T, b: T) -> T:\n    a + b\n\nfn main() -> ():\n    print(cong(1, 2))\n";
const SAI: &str =
    "fn cong[T](a: T, b: T) -> T:\n    a + b\n\nfn main() -> ():\n    print(cong(1, \"a\"))\n";

#[test]
fn check_va_run_cung_ket_luan_hop_le() {
    let p = tam("dung", DUNG);
    let (c1, o1, e1) = chay(&["check", p.to_str().unwrap()]);
    let (c2, o2, e2) = chay(&["run", p.to_str().unwrap()]);
    assert_eq!(c1, 0, "check phải xanh: {o1} {e1}");
    assert_eq!(c2, 0, "run phải xanh: {o2} {e2}");
    assert_eq!(o2, "3\n");
}

#[test]
fn check_va_run_cung_ket_luan_mau_thuan() {
    let p = tam("sai", SAI);
    let (c1, o1, _e1) = chay(&["check", p.to_str().unwrap()]);
    let (c2, _o2, e2) = chay(&["run", p.to_str().unwrap()]);
    assert_ne!(c1, 0, "check phải `not ok`: {o1}");
    assert_ne!(c2, 0, "run phải lỗi: {e2}");
    assert!(o1.contains("VS0412"), "check: {o1}");
    assert!(e2.contains("VS0412"), "run: {e2}");
    for m in ["T", "int", "str"] {
        assert!(e2.contains(m), "thông điệp phải nêu `{m}`: {e2}");
    }
}

#[test]
fn vm_chay_ham_main_nhu_interp() {
    // Bug phát hiện ở ST-28: codegen trỏ entry vào **thân cấp tệp**, bỏ qua `fn main`.
    let p = tam(
        "mainfn",
        "fn giu[T](x: T) -> T:\n    x\n\nfn main() -> ():\n    print(giu(42))\n",
    );
    let (ci, oi, ei) = chay(&["run", p.to_str().unwrap()]);
    let (cv, ov, ev) = chay(&["run", "--vm", "--no-cache", p.to_str().unwrap()]);
    assert_eq!(ci, 0, "interp: {ei}");
    assert_eq!(cv, 0, "vm: {ev}");
    assert_eq!(oi, "42\n");
    assert_eq!(ov, oi, "VM phải chạy `main` **như interp**: {ev}");
}

#[test]
fn generic_hai_kieu_cung_chuong_trinh_hai_duong() {
    let p = tam(
        "haikieu",
        "fn giu[T](x: T) -> T:\n    x\n\nfn main() -> ():\n    print(giu(1))\n    print(giu(\"a\"))\n",
    );
    let (_, oi, _) = chay(&["run", p.to_str().unwrap()]);
    let (_, ov, _) = chay(&["run", "--vm", "--no-cache", p.to_str().unwrap()]);
    assert_eq!(oi, "1\na\n");
    assert_eq!(ov, oi);
}
