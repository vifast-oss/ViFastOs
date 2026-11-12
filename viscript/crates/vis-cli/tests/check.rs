// SPDX-License-Identifier: MIT OR Apache-2.0
//! TAP của `vis check` — ST-08 (runner chỉ **kiểm cú pháp**, chưa interpreter).
//!
//! Đây là test **đỏ trước** (D2-05): viết khi `vis check` chưa có ⇒ mọi ca đỏ vì binary chỉ in usage.
//!
//! Giao diện chốt (để ST-09 dùng lại): `vis check <tệp…>` in TAP 13; mỗi tệp `ok`/`not ok`; dưới
//! `not ok` **đúng một** dòng `# VSxxxx <dòng>:<cột> <thông điệp>` (lọc tầng theo VD-38 mục 8);
//! thoát 0 sạch / 1 có tệp hỏng / 2 dùng sai; `.vish` ⇒ `Bail out!` + thoát 2.

use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/check")
}

fn fx(name: &str) -> String {
    fixture_dir().join(name).to_string_lossy().into_owned()
}

/// Chạy `vis check <args…>` (luôn thêm tiểu lệnh `check`).
fn run(args: &[String]) -> Output {
    let mut all = vec!["check".to_string()];
    all.extend(args.iter().cloned());
    Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(all)
        .output()
        .expect("phải chạy được binary `vis`")
}

fn run_strs(args: &[&str]) -> Output {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    run(&owned)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn lines(o: &Output) -> Vec<String> {
    stdout(o).lines().map(str::to_string).collect()
}

/// Kiểm một dòng chẩn đoán TAP: `# VSxxxx <dòng>:<cột> <thông điệp>`.
fn assert_chan_doan(line: &str) {
    assert!(
        line.starts_with("# VS"),
        "dòng dưới `not ok` phải là chẩn đoán `# VS…`: {line:?}"
    );
    let rest = line.trim_start_matches("# ");
    let parts: Vec<&str> = rest.splitn(3, ' ').collect();
    assert_eq!(
        parts.len(),
        3,
        "chẩn đoán phải là `<mã> <dòng>:<cột> <thông điệp>`: {line:?}"
    );
    let ma = parts[0];
    assert!(
        ma.len() == 6 && ma.starts_with("VS") && ma.chars().skip(2).all(|c| c.is_ascii_digit()),
        "mã phải là `VS` + đúng 4 chữ số: {line:?}"
    );
    let (l, c) = parts[1]
        .split_once(':')
        .unwrap_or_else(|| panic!("vị trí phải là <dòng>:<cột>: {line:?}"));
    assert!(
        l.parse::<u32>().is_ok() && c.parse::<u32>().is_ok(),
        "dòng/cột phải là số: {line:?}"
    );
    assert!(!parts[2].is_empty(), "thông điệp không được rỗng: {line:?}");
}

#[test]
fn tap_sach_cau_truc_va_exit_0() {
    let files = [
        fx("ok_basic.vis"),
        fx("ok_match.vis"),
        fx("ok_pipeline.vis"),
    ];
    let o = run(&files);
    assert_eq!(
        o.status.code(),
        Some(0),
        "mọi tệp sạch ⇒ exit 0; stdout={}",
        stdout(&o)
    );
    let ls = lines(&o);
    assert_eq!(ls[0], "TAP version 13");
    assert_eq!(ls[1], "1..3");
    for (i, f) in files.iter().enumerate() {
        assert_eq!(ls[2 + i], format!("ok {} - {}", i + 1, f));
    }
    assert_eq!(ls.len(), 5, "TAP sạch chỉ có version + 1..N + N dòng ok");
    assert!(!stdout(&o).contains("not ok"));
}

#[test]
fn not_ok_co_dung_mot_dong_chan_doan_moi_tep() {
    let files = [
        fx("bad_syntax_thieu_ve.vis"),
        fx("bad_syntax_tham_so.vis"),
        fx("bad_lex_chuoi.vis"),
    ];
    let o = run(&files);
    assert_eq!(o.status.code(), Some(1), "có tệp hỏng ⇒ exit 1");
    let ls = lines(&o);
    assert_eq!(ls[0], "TAP version 13");
    assert_eq!(ls[1], "1..3");

    // Mỗi `not ok` theo sau đúng một dòng chẩn đoán, và mỗi tệp chỉ một dòng `# VS…`.
    let mut not_ok = 0usize;
    let mut i = 2usize;
    while i < ls.len() {
        if ls[i].starts_with("not ok ") {
            not_ok += 1;
            assert!(
                i + 1 < ls.len(),
                "`not ok` phải có dòng chẩn đoán ngay dưới: {:?}",
                ls[i]
            );
            assert_chan_doan(&ls[i + 1]);
            i += 2;
        } else {
            assert!(ls[i].starts_with("ok "), "dòng TAP lạ: {:?}", ls[i]);
            i += 1;
        }
    }
    assert_eq!(not_ok, 3);
    assert_eq!(
        ls.iter().filter(|l| l.starts_with("# VS")).count(),
        3,
        "mỗi tệp đúng **1** chẩn đoán (lọc tầng VD-38 mục 8)"
    );
    let out = stdout(&o);
    assert!(
        out.contains("# VS0101"),
        "tệp lỗi từ vựng phải báo VS0101 (không bị VS0201 che): {out}"
    );
    assert!(out.contains("# VS0201"), "tệp lỗi cú pháp phải báo VS0201");
}

#[test]
fn thu_tu_tep_dung_theo_tham_so_truyen_vao() {
    let files = [fx("bad_syntax_thieu_ve.vis"), fx("ok_basic.vis")];
    let o = run(&files);
    assert_eq!(o.status.code(), Some(1));
    let ls = lines(&o);
    assert_eq!(ls[1], "1..2");
    assert!(
        ls[2].starts_with("not ok 1 - "),
        "tệp 1 (hỏng) phải là `not ok 1`: {:?}",
        ls[2]
    );
    assert_eq!(ls[4], format!("ok 2 - {}", files[1]));
}

#[test]
fn vish_bail_out_va_thoat_2() {
    let o = run_strs(&[&fx("cmd_mode.vish")]);
    assert_eq!(o.status.code(), Some(2), "chế độ lệnh chưa hỗ trợ ⇒ exit 2");
    let out = stdout(&o);
    assert!(out.contains("TAP version 13"), "vẫn là TAP: {out}");
    assert!(
        out.contains("Bail out! chế độ lệnh (.vish) thuộc ST-06 — chưa hỗ trợ"),
        "thiếu dòng Bail out! đúng câu chữ: {out}"
    );
    // TAP bail-out: **không** có plan `1..N`, không có dòng `ok`, stderr rỗng.
    assert!(
        !out.lines().any(|l| l.starts_with("1..")),
        "không được in plan khi bail out: {out}"
    );
    assert!(!out.contains("ok "), "không được giả vờ `ok`: {out}");
    assert_eq!(stderr(&o), "", "bail out không ghi stderr");
}

#[test]
fn thieu_tham_so_la_dung_sai_thoat_2() {
    let o = run_strs(&[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("dùng: vis check"),
        "usage phải ra stderr: {:?}",
        stderr(&o)
    );
    assert_eq!(stdout(&o), "", "không in TAP khi dùng sai");
}

#[test]
fn co_la_la_dung_sai_thoat_2() {
    let o = run_strs(&["--json"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("dùng: vis check"),
        "usage phải ra stderr: {:?}",
        stderr(&o)
    );
}

#[test]
fn khong_doc_duoc_tep_la_not_ok() {
    let missing = fixture_dir()
        .join("khong_ton_tai.vis")
        .to_string_lossy()
        .into_owned();
    let o = run_strs(&[&missing]);
    assert_eq!(o.status.code(), Some(1));
    let ls = lines(&o);
    assert!(ls[2].starts_with("not ok 1 - "));
    assert!(
        ls[3].starts_with("# không đọc được:"),
        "dưới `not ok` phải là lý do không đọc được: {:?}",
        ls[3]
    );
}

/// Trích **tự động** mọi khối ```vis từ tài liệu (kèm dòng bắt đầu) — không chép tay.
fn doc_blocks() -> (usize, Vec<(usize, String)>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/viscript/ViScript-thiet-ke.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("đọc {}: {e}", path.display()));
    let mut total = 0usize;
    let mut blocks = Vec::new();
    let mut it = text.lines().enumerate();
    while let Some((idx, line)) = it.next() {
        if line.trim_end() == "```vis" {
            total += 1;
            let start = idx + 1;
            let mut body = String::new();
            for (_, l) in it.by_ref() {
                if l.trim_end() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            blocks.push((start, body));
        }
    }
    (total, blocks)
}

/// 7 khối **loại trừ tường minh** theo VD-36 (6 khối `...` + khối `Entry(trường: …)`/`assert`).
const DOC_EXCLUDED: [usize; 7] = [305, 432, 445, 610, 662, 737, 1412];

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-check-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tạo thư mục tạm");
    d
}

#[test]
fn fixture_tai_lieu_15_khoi_deu_ok() {
    let (total, all) = doc_blocks();
    assert_eq!(total, 22, "tài liệu phải có đúng 22 khối ```vis (VD-36)");
    let blocks: Vec<&String> = all
        .iter()
        .filter(|(line, _)| !DOC_EXCLUDED.contains(line))
        .map(|(_, body)| body)
        .collect();
    assert_eq!(blocks.len(), 15, "15 khối đủ thân (VD-36)");
    let dir = tmp_dir("doc-ok");
    let mut files = Vec::new();
    for (i, body) in blocks.iter().enumerate() {
        let p = dir.join(format!("khoi_{i:02}.vis"));
        std::fs::write(&p, body).expect("ghi khối tạm");
        files.push(p.to_string_lossy().into_owned());
    }
    let o = run(&files);
    let out = stdout(&o);
    let ls = lines(&o);
    assert_eq!(
        o.status.code(),
        Some(0),
        "15 khối tài liệu phải sạch cú pháp; stdout=\n{out}"
    );
    assert_eq!(ls[0], "TAP version 13");
    assert_eq!(ls[1], "1..15");
    assert_eq!(
        ls.iter().filter(|l| l.starts_with("ok ")).count(),
        15,
        "cả 15 khối phải ok; stdout=\n{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mixed_lex_va_syntax_phai_bao_vs0101() {
    // Có lỗi **từ vựng** thì báo `01xx` đầu tiên (chuỗi token không còn tin được), **không** báo
    // VS0201 dù vị trí của nó sớm hơn (VD-38 mục 8, review vòng 1).
    let dir = tmp_dir("mixed");
    let cases = [
        ("a.vis", "let a = 1 let b = \"x\n"),
        ("b.vis", "1 2 \"abc\n"),
    ];
    let mut files = Vec::new();
    for (name, src) in cases {
        let p = dir.join(name);
        std::fs::write(&p, src).expect("ghi tệp tạm");
        files.push(p.to_string_lossy().into_owned());
    }
    let o = run(&files);
    assert_eq!(o.status.code(), Some(1));
    let ls = lines(&o);
    assert_eq!(ls[1], "1..2");
    for i in 0..2 {
        assert!(
            ls[2 + i * 2].starts_with(&format!("not ok {}", i + 1)),
            "ca {i}: {:?}",
            ls[2 + i * 2]
        );
        assert!(
            ls[3 + i * 2].starts_with("# VS0101"),
            "ca mixed {i} phải báo VS0101, không bị VS0201 che: {:?}",
            ls[3 + i * 2]
        );
    }
    assert!(
        !stdout(&o).contains("# VS0201"),
        "không được in VS0201 khi có lỗi từ vựng: {}",
        stdout(&o)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn khoi_loai_tru_tai_lieu_bao_not_ok_dung_mot_chan_doan() {
    // 7 khối loại trừ (VD-36) phải `not ok`, exit 1, mỗi khối **đúng 1** dòng `# VS…`.
    let (total, all) = doc_blocks();
    assert_eq!(total, 22);
    let excluded: Vec<&(usize, String)> = all
        .iter()
        .filter(|(line, _)| DOC_EXCLUDED.contains(line))
        .collect();
    assert_eq!(excluded.len(), 7, "đúng 7 khối loại trừ (VD-36)");
    let dir = tmp_dir("doc-excluded");
    let mut files = Vec::new();
    for (i, (_, body)) in excluded.iter().enumerate() {
        let p = dir.join(format!("loai_{i:02}.vis"));
        std::fs::write(&p, body).expect("ghi khối tạm");
        files.push(p.to_string_lossy().into_owned());
    }
    let o = run(&files);
    assert_eq!(o.status.code(), Some(1), "7 khối loại trừ phải exit 1");
    let ls = lines(&o);
    assert_eq!(ls[0], "TAP version 13");
    assert_eq!(ls[1], "1..7");
    let mut not_ok = 0usize;
    let mut i = 2usize;
    while i < ls.len() {
        if ls[i].starts_with("not ok ") {
            not_ok += 1;
            assert_chan_doan(&ls[i + 1]);
            i += 2;
        } else {
            panic!("khối loại trừ không được `ok`: {:?}", ls[i]);
        }
    }
    assert_eq!(not_ok, 7);
    assert_eq!(
        ls.iter().filter(|l| l.starts_with("# VS")).count(),
        7,
        "mỗi khối đúng 1 chẩn đoán"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duong_dan_tuong_doi_in_nguyen_van() {
    // Chạy trong thư mục tạm, truyền đường dẫn **tương đối** ⇒ TAP in nguyên văn đường dẫn đó.
    let dir = tmp_dir("rel");
    std::fs::write(dir.join("r.vis"), "let x = 1\n").expect("ghi tệp");
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .current_dir(&dir)
        .args(["check", "r.vis"])
        .output()
        .expect("chạy được vis");
    assert_eq!(o.status.code(), Some(0));
    let ls = lines(&o);
    assert_eq!(ls[0], "TAP version 13");
    assert_eq!(ls[1], "1..1");
    assert_eq!(ls[2], "ok 1 - r.vis", "phải in nguyên văn `r.vis`");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn thu_muc_tep_rong_va_non_utf8() {
    let dir = tmp_dir("weird");
    let empty = dir.join("empty.vis");
    std::fs::write(&empty, "").expect("ghi tệp rỗng");
    let bad = dir.join("bad.vis");
    std::fs::write(&bad, [0xffu8, 0xfe, 0xfd]).expect("ghi tệp non-UTF8");
    // Tệp 0 byte = chương trình rỗng ⇒ `ok`, exit 0; non-UTF8 ⇒ `not ok` + lỗi I/O, không panic.
    let o = run(&[
        empty.to_string_lossy().into_owned(),
        bad.to_string_lossy().into_owned(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    let ls = lines(&o);
    assert_eq!(ls[1], "1..2");
    assert!(ls[2].starts_with("ok 1 - "), "tệp 0 byte phải ok: {ls:?}");
    assert!(
        ls[3].starts_with("not ok 2 - "),
        "non-UTF8 phải not ok: {ls:?}"
    );
    assert!(
        ls[4].starts_with("# không đọc được:"),
        "non-UTF8 là lỗi I/O, không phải `# VS…`: {ls:?}"
    );
    // Thư mục ⇒ lỗi I/O ⇒ `not ok`, exit 1 (không panic).
    let o2 = run(&[dir.to_string_lossy().into_owned()]);
    assert_eq!(o2.status.code(), Some(1));
    let ls2 = lines(&o2);
    assert!(ls2[2].starts_with("not ok 1 - "));
    assert!(ls2[3].starts_with("# không đọc được:"));
    let _ = std::fs::remove_dir_all(&dir);
}
