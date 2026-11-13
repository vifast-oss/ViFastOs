// SPDX-License-Identifier: MIT OR Apache-2.0
//! TAP của `vis check` — ST-08 (runner chỉ **kiểm cú pháp**, chưa interpreter).
//!
//! Đây là test **đỏ trước** (D2-05): viết khi `vis check` chưa có ⇒ mọi ca đỏ vì binary chỉ in usage.
//!
//! Giao diện chốt (để ST-09 dùng lại): `vis check <tệp…>` in TAP 13; mỗi tệp `ok`/`not ok`; dưới
//! `not ok` **đúng một** dòng `# VSxxxx <dòng>:<cột> <thông điệp>` (lọc tầng theo VD-38 mục 8);
//! thoát 0 sạch / 1 có tệp hỏng / 2 dùng sai. `.vish` (chế độ lệnh) từ **ST-06** được kiểm bình
//! thường bằng CmdCST — không còn `Bail out!`.

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

/// ST-28: cổng "khối mã trong tài liệu **parse** được" (QD §7.4) chạy `--syntax` — `vis check`
/// mặc định nay kiểm **cả kiểu** (đồng bộ `vis run`).
fn run_syntax(files: &[String]) -> Output {
    let mut all = vec!["check".to_string(), "--syntax".to_string()];
    all.extend(files.iter().cloned());
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
fn vish_duoc_kiem_bang_cmdcst() {
    // ST-06: `.vish` kiểm bằng `Mode::Command` — ca **chỉ đúng ở chế độ lệnh** phải `ok`, tệp lỗi
    // phải `not ok` + đúng 1 chẩn đoán.
    // **SỬA KÈM LÝ DO (ST-18)**: từ ST-18 `vis check *.vish` **hạ thật** (`cmd_lower`) rồi kiểm kiểu,
    // nên ca cũ `cmd_ok.vish` (dùng `^git`, `ls`, `grep` — **ngoài bảng chữ ký VS0**) nay đúng là
    // `not ok` với `VS0601`/`VS0604`; ca "hợp lệ" mới là `cmd_vs0_ok.vish` chỉ dùng lệnh VS0.
    let ok = fx("cmd_vs0_ok.vish");
    let o = run_strs(&[&ok]);
    assert_eq!(o.status.code(), Some(0), "tệp .vish hợp lệ ⇒ exit 0");
    let out = stdout(&o);
    assert_eq!(out.lines().nth(1), Some("1..1"));
    assert!(
        out.contains(&format!("ok 1 - {ok}")),
        "phải `ok` thật (CmdCST): {out}"
    );
    assert!(!out.contains("Bail out!"), "không còn bail out: {out}");

    // Ca cũ (lệnh ngoài bảng VS0) — vẫn **không** Bail out, mà `not ok` + đúng 1 chẩn đoán `VS06xx`.
    let cu = fx("cmd_ok.vish");
    let o = run_strs(&[&cu]);
    assert_eq!(o.status.code(), Some(1), "lệnh ngoài bảng VS0 ⇒ exit 1");
    let out = stdout(&o);
    assert!(!out.contains("Bail out!"), "không còn bail out: {out}");

    let bad = fx("cmd_bad.vish");
    let o = run_strs(&[&bad]);
    assert_eq!(o.status.code(), Some(1), "tệp .vish lỗi ⇒ exit 1");
    let ls = lines(&o);
    assert!(ls[2].starts_with("not ok 1 - "), "{ls:?}");
    assert_chan_doan(&ls[3]);
    assert_eq!(
        ls.iter().filter(|l| l.starts_with("# VS")).count(),
        1,
        "đúng 1 chẩn đoán: {ls:?}"
    );
}

#[test]
fn hai_khoi_vish_session_qua_cli_deu_ok() {
    // DoD ST-06: 2 khối ```vish-session của tài liệu (trích tự động) phải `vis check` xanh.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/viscript/ViScript-thiet-ke.md");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("đọc {}: {e}", path.display()));
    let mut bodies = Vec::new();
    let mut it = text.lines();
    while let Some(line) = it.next() {
        if line.trim_end() == "```vish-session" {
            let mut body = String::new();
            for l in it.by_ref() {
                if l.trim_end() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            bodies.push(body);
        }
    }
    assert_eq!(bodies.len(), 2, "tài liệu phải có 2 khối vish-session");
    let dir = std::env::temp_dir().join(format!("vis-check-vish-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("thư mục tạm");
    let mut files = Vec::new();
    for (i, b) in bodies.iter().enumerate() {
        let p = dir.join(format!("s{i}.vish"));
        std::fs::write(&p, b).expect("ghi khối");
        files.push(p.to_string_lossy().into_owned());
    }
    // **SỬA KÈM LÝ DO (ST-37)**: hai khối ```vish-session của tài liệu dùng lệnh **VS1** (`ls`,
    // `where`, `sort_by`, `each`, …). Từ VS1c các lệnh **lạ** là **chương trình ngoài** (§9.3) và khe
    // ống nay chọn theo **khả năng nhận** (VD-69) ⇒ `vis check` **không còn** chẩn đoán cho hai khối
    // này: TAP đủ 2 dòng, **2 ok**, exit 0. Bất biến giữ nguyên: `vis check` chạy **hết** hai tệp,
    // không Bail out, không im lặng bỏ tệp.
    let o = run(&files);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "khối dùng lệnh ngoài VS0: {out}");
    assert_eq!(lines(&o)[1], "1..2");
    assert_eq!(out.matches("ok ").count(), 2, "{out}");
    assert_eq!(out.matches("not ok ").count(), 0, "{out}");
    let _ = std::fs::remove_dir_all(&dir);
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
    let o = run_syntax(&files);
    let out = stdout(&o);
    let ls = lines(&o);
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
    let o = run_syntax(&files);
    assert_eq!(o.status.code(), Some(1), "6 khối loại trừ phải exit 1");
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
            // **SỬA KÈM LÝ DO (ST-27)**: khối loại trừ thứ 5 (`extern cmd git(…, ...rest: str)`,
            // tài liệu dòng 662) nay **hợp lệ** vì variadic `...rest` đã có — đúng như tài liệu
            // §9.5 khai. Sáu khối còn lại vẫn phải `not ok` với đúng một chẩn đoán.
            assert!(
                ls[i].contains("loai_04.vis"),
                "chỉ khối `extern cmd git(…, ...rest)` được ok (ST-27), thấy: {:?}",
                ls[i]
            );
            i += 1;
        }
    }
    assert_eq!(
        not_ok, 6,
        "6 khối còn lại phải `not ok` (khối variadic đã hợp lệ từ ST-27)"
    );
    assert_eq!(
        ls.iter().filter(|l| l.starts_with("# VS")).count(),
        6,
        "6 khối `not ok` — mỗi khối đúng 1 chẩn đoán (khối variadic đã hợp lệ, ST-27)"
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
