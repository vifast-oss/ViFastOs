// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-37 (VSH1a)** — khe ống **hợp nhất** theo khả năng nhận (§9.5) + `CmdSig` generic.
//!
//! Trước ST-37: khe chọn theo **so bằng** kiểu ⇒ `( [1,2] ) | take 1` đỏ (`take` khai `list[str]`).
//! Nay `take[T](xs: list[T], n: int)` + hợp nhất một chiều ⇒ `list[int]` vào được khe.

use std::path::PathBuf;
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-khe-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn chay(d: &std::path::Path, tag: &str, src: &str) -> (i32, String, String) {
    let p = d.join(format!("{tag}.vish"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("run")
        .arg(&p)
        .output()
        .expect("chạy");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

#[test]
fn nhom_list_int_vao_take() {
    let d = tam("int");
    let (c, o, e) = chay(&d, "int", "( [1, 2] ) | take 1\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_list_str_vao_take() {
    let d = tam("str");
    let (c, o, e) = chay(&d, "str", "( [\"x\", \"y\"] ) | take 1\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[x]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_list_bool_vao_take() {
    let d = tam("bool");
    let (c, o, e) = chay(&d, "bool", "( [true, false] ) | take 1\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[true]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_ba_phan_tu_take_hai() {
    let d = tam("ba");
    let (c, o, e) = chay(&d, "ba", "( [1, 2, 3] ) | take 2\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[1, 2]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn take_khong_lay_gi() {
    let d = tam("khong");
    let (c, o, e) = chay(&d, "khong", "( [1, 2] ) | take 0\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lines_roi_take_giu_nguyen() {
    let d = tam("lines");
    let (c, o, e) = chay(&d, "lines", "echo \"a b\" | lines | take 1\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[a b]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn str_khong_vao_duoc_take() {
    let d = tam("loi");
    let (c, _, e) = chay(&d, "loi", "echo x | take 1\n");
    assert_eq!(c, 2);
    assert!(e.contains("VS0603"), "stderr: {e}");
    assert!(e.contains("không có khe nhận `str`"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn take_khong_pipe_van_vs0602() {
    // VD-53 (4): không phải pipeline ⇒ **VS0602** (sai kiểu đối số), không phải VS0603.
    let d = tam("nopipe");
    let (c, _, e) = chay(&d, "nopipe", "take 1\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0602"), "stderr: {e}");
    assert!(!e.contains("VS0603"), "không được là VS0603: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khe_giu_cho_truoc_khong_de_bind() {
    // Pipe vào `echo` (khe `a`), đối số tường minh `"y"` rơi vào `b` — khe **giữ chỗ trước**.
    let d = tam("giu");
    let (c, o, e) = chay(&d, "giu", "echo a | echo \"y\"\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "a y\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn co_khong_bi_chiem_lam_khe() {
    let d = tam("co");
    let (c, o, e) = chay(&d, "co", "echo x | echo --upper\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "X\n", "--upper vẫn là cờ, khe rơi vào `a`");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khong_ngoac_van_la_loi_parse() {
    // Nợ **VSH1b**: stage đầu là **biểu thức** chưa được văn phạm §9.5 nhận ⇒ `VS0201`.
    let d = tam("parse");
    let (c, _, e) = chay(&d, "parse", "[1, 2] | take 1\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0201"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_khong_nhan_pipe() {
    let d = tam("nhom");
    let (c, _, e) = chay(&d, "nhom", "echo a | ( [1] )\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0603"), "stderr: {e}");
    assert!(e.contains("không nhận giá trị từ pipe"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_khong_phai_list_thi_bao_loi_luc_chay() {
    // `( 1 + 1 )` ⇒ `int` vào khe đầu của `take` (chưa suy được kiểu) ⇒ lỗi **lúc chạy** VS0501.
    let d = tam("int2");
    let (c, _, e) = chay(&d, "int2", "( 1 + 1 ) | take 1\n");
    assert_eq!(c, 1, "stderr: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(e.contains("`take` cần"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nhom_lam_doi_so_chua_ho_tro() {
    // Hiện trạng: `( … )` làm **đối số** bị parser tách thành stage ⇒ VS0603 (nợ VSH1b).
    let d = tam("arg");
    let (c, _, e) = chay(&d, "arg", "( [1, 2] ) | take ([9, 9]) 1\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0603"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}
