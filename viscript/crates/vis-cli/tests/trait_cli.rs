// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-29 — **trait + derives qua binary**: dispatch **tĩnh** (cùng HIR) ⇒ interp và VM **khớp nhau**;
//! `derives Eq/Show/Json` chạy bằng ngữ nghĩa có sẵn; lỗi phương thức dùng `VS0403` (nghĩa mở rộng).

use std::process::Command;

const S: &str = "# SPDX-License-Identifier: MIT OR Apache-2.0\n";

fn tam(ten: &str, body: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vis-trait-{}-{}", ten, std::process::id()));
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

const TRAIT_SHOW: &str = "trait Show:\n    fn show(self) -> str\n\ntype Hop:\n    gia_tri: int\n\nimpl Show for Hop:\n    fn show(self) -> str:\n        \"hop\"\n\nfn main() -> ():\n    let h: Hop = {gia_tri: 1}\n    print(h.show())\n";

/// **Ca dispatch thật** (yêu cầu Agent 0): chạy **cả** interp và VM, in đúng, khớp nhau.
#[test]
fn dispatch_impl_show_chay_ca_hai_duong() {
    let p = tam("show", TRAIT_SHOW);
    let (ci, oi, ei) = chay(&["run", p.to_str().unwrap()]);
    let (cv, ov, ev) = chay(&["run", "--vm", "--no-cache", p.to_str().unwrap()]);
    assert_eq!(ci, 0, "interp: {ei}");
    assert_eq!(cv, 0, "vm: {ev}");
    assert_eq!(oi, "hop\n");
    assert_eq!(ov, oi, "hai đường phải khớp: {ev}");
}

#[test]
fn dispatch_inherent_hai_duong() {
    let p = tam(
        "inherent",
        "type Hop:\n    gia_tri: int\n\nimpl Hop:\n    fn gap(self) -> int:\n        self.gia_tri\n\nfn main() -> ():\n    let h: Hop = {gia_tri: 7}\n    print(h.gap())\n",
    );
    let (_, oi, _) = chay(&["run", p.to_str().unwrap()]);
    let (_, ov, ev) = chay(&["run", "--vm", "--no-cache", p.to_str().unwrap()]);
    assert_eq!(oi, "7\n");
    assert_eq!(ov, oi, "vm: {ev}");
}

#[test]
fn derives_eq_show_json_chay_that() {
    let p = tam(
        "derives",
        "type Hop derives Eq, Show, Json:\n    gia_tri: int\n\nfn main() -> ():\n    let a: Hop = {gia_tri: 1}\n    let b: Hop = {gia_tri: 1}\n    print(a == b)\n    print(a)\n",
    );
    let (c, o, e) = chay(&["run", p.to_str().unwrap()]);
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "true\n{gia_tri: 1}\n");
    let (c2, o2, _) = chay(&["run", "--vm", "--no-cache", p.to_str().unwrap()]);
    assert_eq!(c2, 0);
    assert_eq!(o2, o);
}

#[test]
fn thieu_phuong_thuc_la_vs0403_va_noi_phuong_thuc() {
    let p = tam(
        "thieu",
        "type Hop:\n    gia_tri: int\n\nlet h: Hop = {gia_tri: 1}\nprint(h.khong_co())\n",
    );
    let (c, _, e) = chay(&["run", p.to_str().unwrap()]);
    assert_ne!(c, 0);
    assert!(e.contains("VS0403"), "stderr: {e}");
    assert!(
        e.contains("phương thức"),
        "thông điệp phải nói **phương thức** (nghĩa mở rộng ST-29): {e}"
    );
}

#[test]
fn impl_thieu_phuong_thuc_trait_la_vs0403() {
    let p = tam(
        "implthieu",
        "trait Show:\n    fn show(self) -> str\n\ntype Hop:\n    gia_tri: int\n\nimpl Show for Hop:\n    fn khac(self) -> str:\n        \"x\"\n",
    );
    let (c, _, e) = chay(&["run", p.to_str().unwrap()]);
    assert_ne!(c, 0);
    assert!(e.contains("VS0403"), "stderr: {e}");
    assert!(e.contains("show"), "phải nêu tên phương thức thiếu: {e}");
}

#[test]
fn derives_la_bao_vs0403() {
    let p = tam("derive-la", "type Hop derives La:\n    gia_tri: int\n");
    let (c, o, e) = chay(&["check", p.to_str().unwrap()]);
    let tap = format!("{o}{e}");
    assert_ne!(c, 0);
    assert!(tap.contains("VS0403"), "TAP: {tap}");
}

#[test]
fn check_va_run_cung_ket_luan_phuong_thuc() {
    let p = tam(
        "cungketluan",
        "type Hop:\n    gia_tri: int\n\nlet h: Hop = {gia_tri: 1}\nprint(h.khong_co())\n",
    );
    let (c1, o1, _) = chay(&["check", p.to_str().unwrap()]);
    let (c2, _, e2) = chay(&["run", p.to_str().unwrap()]);
    assert_ne!(c1, 0, "check phải bắt: {o1}");
    assert_ne!(c2, 0, "run phải bắt: {e2}");
    assert!(o1.contains("VS0403"), "check: {o1}");
    assert!(e2.contains("VS0403"), "run: {e2}");
}
