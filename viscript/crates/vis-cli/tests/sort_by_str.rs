// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-56 (nợ lõi A.1)** — khoá `str` cho `std.list.sort_by`.
//!
//! Khoá: nhận `int` **hoặc** `str` (dùng **chung** lõi `sap_xep_theo_khoa` với shell `sort_by`);
//! **sort ổn định**; chuỗi (Việt/emoji) so **byte UTF-8** — **nhất quán** với shell `sort_by`;
//! trộn `int`/`str` trong một lần gọi ⇒ **`VS0501`** nêu **cả hai** kiểu thật; khoá `int` cũ
//! **không đổi**; interp ↔ `--vm` **cùng byte** (cả stdout lẫn stderr).

use std::path::{Path, PathBuf};
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-st56-sort-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

fn ghi(d: &Path, ten: &str, src: &str, duoi: &str) -> PathBuf {
    let p = d.join(format!("{ten}.{duoi}"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    p
}

fn chay(p: &Path, vm: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    c.arg("run");
    if vm {
        c.arg("--vm").arg("--no-cache");
    }
    let o = c.arg(p).output().expect("chạy `vis run`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chạy **cả hai đường** trên **cùng một tệp** và đòi exit + stdout + stderr **byte-khớp**.
fn hai_duong(d: &Path, ten: &str, src: &str, duoi: &str) -> (i32, String, String) {
    let p = ghi(d, ten, src, duoi);
    let a = chay(&p, false);
    let b = chay(&p, true);
    assert_eq!(
        (a.0, &a.1, &a.2),
        (b.0, &b.1, &b.2),
        "interp ≠ `--vm` (exit/stdout/stderr)\n  interp: {a:?}\n  vm: {b:?}"
    );
    a
}

const DS: &str = r#"["b", "Á", "a", "à", "🍎", "z"]"#;

#[test]
fn khoa_str_chay_hai_duong_cung_byte() {
    let d = tam("co-ban");
    let src = format!(
        "use std.list\nfn main():\n    print(list.sort_by({DS}, s => s))\n    \
         print(list.sort_by([{{k: \"b\", i: 1}}, {{k: \"a\", i: 2}}, {{k: \"b\", i: 3}}], r => r.k).map(r => r.i))\n"
    );
    let (c, o, e) = hai_duong(&d, "co-ban", &src, "vis");
    assert_eq!(c, 0, "phải xanh, stderr: {e}");
    // Byte UTF-8: `a`(61) `b`(62) `z`(7A) `Á`(C3 81) `à`(C3 A0) `🍎`(F0 9F 8D 8E).
    // Khoá bằng nhau (`k = "b"`) giữ **thứ tự gốc** (i = 1 trước i = 3) ⇒ `[2, 1, 3]`.
    assert_eq!(o, "[a, b, z, Á, à, 🍎]\n[2, 1, 3]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khoa_str_nhat_quan_voi_shell_sort_by() {
    let d = tam("nhat-quan");
    // Cùng một danh sách: `std.list.sort_by` (khoá `str`) và shell `sort_by` phải **cùng thứ tự**.
    let a = hai_duong(
        &d,
        "std",
        &format!("use std.list\nfn main():\n    print(list.sort_by({DS}, s => s))\n"),
        "vis",
    );
    let b = hai_duong(
        &d,
        "shell",
        &format!("( {DS} ) | sort_by it => it\n"),
        "vish",
    );
    assert_eq!(a.0, 0, "std stderr: {}", a.2);
    assert_eq!(b.0, 0, "shell stderr: {}", b.2);
    assert_eq!(
        a.1, b.1,
        "thứ tự `str` phải **nhất quán** giữa `std.list.sort_by` và shell `sort_by`"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khoa_int_cu_khong_doi() {
    // Hợp đồng cũ `fn(int) -> int` (ST-53) — ca `test/sem`/`test/std` cũ phải giữ nguyên.
    let d = tam("int-cu");
    let (c, o, e) = hai_duong(
        &d,
        "int-cu",
        "use std.list\nfn main():\n    print(list.sort_by([3, 1, 2], x => 0 - x))\n    \
         print(list.sort_by([3, 2, 1], x => 0))\n    print(list.sort_by([10, 2, 33], x => x % 10))\n",
        "vis",
    );
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[3, 2, 1]\n[3, 2, 1]\n[10, 2, 33]\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// **SỬA KÈM LÝ DO (ST-58)**: `list.concat` nay là `(list[T], list[T]) -> list[T]` ⇒ `[1] + ["a"]`
/// bị **tyck bắt tĩnh** (`VS0412`), không còn dựng được danh sách trộn. Ca **runtime** vẫn phải khoá:
/// dựng danh sách trộn từ **dữ liệu không kiểu** `json.decode` (`Var` không gán ⇒ `{error}`) ⇒ front
/// end **sạch**, lỗi vẫn ở **lúc chạy** `VS0501` (nêu cả `int` và `str`).
const TRON: &str = "use std.json\nuse std.list\nfn main():\n    match json.decode(\"[1, \\\"a\\\"]\"):\n        Ok(xs):\n            print(list.sort_by(xs, x => x))\n        Err(e):\n            print(e)\n";

#[test]
fn tron_int_va_str_la_vs0501_neu_kieu_that() {
    let d = tam("tron");
    // `vis check` **không** chẩn đoán (lỗi động) ⇒ ca thuộc tầng lỗi lúc chạy.
    let p = ghi(&d, "tron", TRON, "vis");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .arg("check")
        .arg(&p)
        .output()
        .expect("chạy `vis check`");
    assert_eq!(o.status.code().unwrap_or(-1), 0, "check phải **rỗng**");
    let (c, out, e) = hai_duong(&d, "tron", TRON, "vis");
    assert_eq!(c, 1, "phải là lỗi **lúc chạy** (exit 1), stderr: {e}");
    assert!(out.is_empty(), "không được in gì trước lỗi: {out:?}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(
        e.contains("`int`") && e.contains("`str`"),
        "thông điệp phải nêu **cả hai** kiểu thật (`int` và `str`): {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khoa_bool_bao_vs0501_va_neu_kieu_that() {
    let d = tam("bool");
    let src = "use std.list\nfn main():\n    print(list.sort_by([1, 2], x => x > 1))\n";
    let (c, _, e) = hai_duong(&d, "bool", src, "vis");
    assert_eq!(c, 1, "stderr: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    // Thông điệp nêu **giá trị khoá thật** (`show()` của `false` — cùng quy ước thông điệp cũ).
    assert!(e.contains("`false`"), "phải nêu khoá thật: {e}");
    assert!(
        e.contains("`int`") && e.contains("`str`"),
        "hợp đồng mới phải nêu `int` hoặc `str`: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn khoa_khong_goi_duoc_neu_ten_hien_thi_moi() {
    // `Ty::FnIntInt.ten()` cũ nói "fn(int) -> int"; khoá giờ nhận cả `str` ⇒ `Ty::FnKhoa` phải
    // hiển thị đúng hợp đồng mới.
    let d = tam("khong-goi");
    let src = "use std.list\nfn main():\n    print(list.sort_by([1, 2], 3))\n";
    let (c, _, e) = hai_duong(&d, "khong-goi", src, "vis");
    assert_eq!(c, 1, "stderr: {e}");
    assert!(e.contains("VS0501"), "stderr: {e}");
    assert!(
        e.contains("int | str"),
        "tên hiển thị phải nói khoá `int` **hoặc** `str`: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
