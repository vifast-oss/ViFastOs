// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-38 (VSH1a)** — hạ closure-arg trong `.vish` + `where`/`each`/`first`/`count`.
//!
//! Khoá: closure ⇒ `HExpr::Lambda` với **span tuyệt đối** (đệm theo **byte**), `where`/`count`/`first`
//! chạy đúng, `first` trên list rỗng ⇒ `none`, `each` trả `()` và **không** nối tiếp được,
//! lỗi kiểu trong thân ⇒ `VS0412` đúng **dòng:cột** (kể cả khi tệp có ký tự **nhiều byte** trước đó).

use std::path::PathBuf;
use std::process::Command;

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-cl-{tag}-{}", std::process::id()));
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
fn where_loc_theo_closure() {
    let d = tam("where");
    let (c, o, e) = chay(&d, "where", "( [1, 2, 3] ) | where x => x > 1\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[2, 3]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_voi_list_str_va_len() {
    let d = tam("wstr");
    let (c, o, e) = chay(&d, "wstr", "( [\"a\", \"bb\"] ) | where s => len(s) > 1\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[bb]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_list_rong_van_rong() {
    let d = tam("wempty");
    let (c, o, e) = chay(&d, "wempty", "( [] ) | where x => x > 0\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "[]\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn count_dem_phan_tu() {
    let d = tam("count");
    let (c, o, e) = chay(&d, "count", "( [\"a\", \"bb\"] ) | count\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "2\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn first_lay_dau() {
    let d = tam("first");
    let (c, o, e) = chay(&d, "first", "( [1, 2] ) | first\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "some(1)\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn first_tren_list_rong_la_none() {
    // Yêu cầu của Agent 0: **không** phải lỗi.
    let d = tam("fempty");
    let (c, o, e) = chay(&d, "fempty", "( [] ) | first\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "none\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn each_in_tung_phan_tu() {
    let d = tam("each");
    let (c, o, e) = chay(&d, "each", "( [\"a\", \"b\"] ) | each x => print(x)\n");
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "a\nb\n()\n", "in từng phần tử, giá trị cuối `()`");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn each_noi_tiep_la_vs0603() {
    let d = tam("each2");
    let (c, _, e) = chay(&d, "each2", "( [1, 2] ) | each x => print(x) | count\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0603"), "stderr: {e}");
    assert!(e.contains("unit"), "khe không nhận `unit`: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn than_sai_kieu_la_vs0412() {
    let d = tam("body");
    let (c, _, e) = chay(&d, "body", "( [1, 2] ) | where x => x + 1\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    assert!(e.contains("thân lambda"), "phải nói rõ thân: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_khong_phai_closure_la_loi_kiem_tinh() {
    // ST-38 (A): `where 1` đi đường **sugar** ⇒ `it => 1`, thân là `int` ⇒ **VS0412** (Agent 0 cho
    // phép `VS0412` **hoặc** `VS0602`, miễn là lỗi **kiểm tĩnh**). Trước sugar nó là `VS0602`.
    let d = tam("nocl");
    let (c, _, e) = chay(&d, "nocl", "( [1, 2] ) | where 1\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(
        e.contains("VS0412") || e.contains("VS0602"),
        "phải là lỗi kiểm tĩnh: {e}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn closure_hai_tham_so_la_vs0602() {
    let d = tam("two");
    let (c, _, e) = chay(&d, "two", "( [1, 2] ) | where x, y => x\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0602"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// **Bẫy của đệm theo byte**: tệp có tiếng Việt + emoji **trước** closure ⇒ `dòng:cột` trong thân
/// phải **giống hệt** tệp không có dòng nhiều byte (cùng độ dài dòng).
#[test]
fn span_than_dung_khi_truoc_do_co_ky_tu_nhieu_byte() {
    let d = tam("span");
    let dong = "( [1, 2] ) | where x => x + \"s\"\n";
    let (c1, _, e1) = chay(&d, "plain", dong);
    let (c2, _, e2) = chay(
        &d,
        "utf8",
        "# Tiếng Việt: ước mơ 😀 — nhiều byte ở dòng trước\nprint(\"a😀\")\n( [1, 2] ) | where x => x + \"s\"\n",
    );
    assert_eq!(c1, 2, "stderr: {e1}");
    assert_eq!(c2, 2, "stderr: {e2}");
    assert!(
        e1.contains("VS0412") && e2.contains("VS0412"),
        "{e1} | {e2}"
    );
    // Cột trong **cùng** một dòng phải bằng nhau; chỉ **số dòng** được khác (2 vs 4).
    let cot = |e: &str| -> String {
        let cuoi = e.lines().find(|l| l.contains("VS0412")).unwrap_or("");
        cuoi.split(':').nth(2).unwrap_or("?").trim().to_string()
    };
    assert_eq!(cot(&e1), cot(&e2), "cột lệch: {e1} | {e2}");
    let dong_cua = |e: &str| -> String {
        let cuoi = e.lines().find(|l| l.contains("VS0412")).unwrap_or("");
        cuoi.split(':').nth(1).unwrap_or("?").to_string()
    };
    assert_eq!(dong_cua(&e1), "2");
    assert_eq!(dong_cua(&e2), "4", "lỗi trong thân closure ở dòng 4: {e2}");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- ST-38 (A): sugar vị từ ngầm `it` (§9.7) ----------

const HOP: &str = "( [{size: 6mb}, {size: 1mb}] )";

#[test]
fn sugar_vi_tu_ngam_it() {
    let d = tam("sugar");
    let (c, o, e) = chay(&d, "sugar", &format!("{HOP} | where size > 5mb\n"));
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "size\n 6mb\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn sugar_va_it_viet_tuong_minh_tuong_duong() {
    let d = tam("sugar2");
    let (c, o, e) = chay(&d, "sugar2", &format!("{HOP} | where it.size > 5mb\n"));
    assert_eq!(c, 0, "stderr: {e}");
    assert_eq!(o, "size\n 6mb\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn sugar_cac_dang_so_co_don_vi() {
    for (tag, src, muon) in [
        (
            "age",
            "( [{age: 20}, {age: 10}] ) | where age >= 18\n",
            "age\n 20\n",
        ),
        (
            "cpu",
            "( [{cpu: 60pct}, {cpu: 20pct}] ) | where cpu > 50pct\n",
            "  cpu\n60pct\n",
        ),
    ] {
        let d = tam(tag);
        let (c, o, e) = chay(&d, tag, src);
        assert_eq!(c, 0, "{tag}: stderr: {e}");
        assert_eq!(o, muon, "{tag}");
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn where_hang_so_la_loi_kiem_tinh() {
    // Agent 0 chốt: `where 1` **phải** là lỗi kiểm tĩnh — chọn **VS0412** (thân lambda không phải bool).
    let d = tam("hangso");
    let (c, _, e) = chay(&d, "hangso", "( [1, 2] ) | where 1\n");
    assert_eq!(c, 2, "stderr: {e}");
    assert!(e.contains("VS0412"), "stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn where_ten_tran_khong_so_sanh_la_loi() {
    // `where size` — `size` là `size`, không phải `bool` ⇒ **lỗi** (hiện là VS0501 lúc chạy vì bộ lọc
    // trả `size`; ghi JOURNAL là nợ "kiểm tĩnh khi pipe chưa rõ kiểu").
    let d = tam("tentran");
    let (c, _, e) = chay(&d, "tentran", "( [{size: 6mb}] ) | where size\n");
    assert_ne!(c, 0, "phải là lỗi, stderr: {e}");
    let _ = std::fs::remove_dir_all(&d);
}
