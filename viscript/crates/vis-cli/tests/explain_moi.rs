// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-78** — `vis explain` cho 3 mã runtime mới (`VS0504`/`VS0505`/`VS0506`) đủ **3 ngôn ngữ**,
//! `--list` phủ **32** mã đúng thứ tự, lỗi rõ cho mã/ngôn ngữ sai, và trợ giúp nêu đúng **số mã**.

use std::process::Command;

fn vis(args: &[&str]) -> (i32, String, String) {
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

/// 9 lần gọi = 3 mã × 3 ngôn ngữ; so **nguyên văn** cả dòng.
#[test]
fn ba_ma_runtime_du_ba_ngon_ngu() {
    let cases = [
        ("VS0504", "vi", "VS0504 — lỗi: gửi vào kênh đã `close` (không nhận thêm phần tử)"),
        ("VS0504", "en", "VS0504 — error: send on a closed channel (no more elements accepted)"),
        ("VS0504", "ja", "VS0504 — エラー: 閉じたチャネルへの送信（これ以上受け付けません）"),
        ("VS0505", "vi", "VS0505 — lỗi: deadlock: kênh bị chặn và không còn tác vụ nào chạy được (khoá)"),
        ("VS0505", "en", "VS0505 — error: deadlock: the channel is blocked and no task is runnable"),
        ("VS0505", "ja", "VS0505 — エラー: デッドロック: チャネルがブロックされ、実行可能なタスクがありません"),
        ("VS0506", "vi", "VS0506 — lỗi: tác vụ (`async fn`) lỗi: lỗi phát sinh trong thân tác vụ (kèm mã gốc)"),
        ("VS0506", "en", "VS0506 — error: task (`async fn`) failed: an error occurred inside the task body (original code included)"),
        ("VS0506", "ja", "VS0506 — エラー: タスク (`async fn`) でエラー: タスク本体でエラーが発生しました（元のコード付き）"),
    ];
    for (ma, lang, muon) in cases {
        let (code, out, err) = vis(&["explain", ma, "--lang", lang]);
        assert_eq!(code, 0, "{ma}/{lang}: {err}");
        assert_eq!(out, format!("{muon}\n"), "{ma}/{lang}");
    }
}

/// `--list` đúng **32 dòng**, đúng thứ tự id của `all_codes()`, mỗi dòng có câu khác rỗng.
#[test]
fn list_du_32_dong_dung_thu_tu() {
    let (code, out, err) = vis(&["explain", "--list"]);
    assert_eq!(code, 0, "{err}");
    let dong: Vec<&str> = out.lines().collect();
    assert_eq!(dong.len(), 32, "phải đúng 32 mã: {}", dong.len());
    let muon: Vec<String> = vis_diag::all_codes().map(|c| c.short.to_string()).collect();
    assert_eq!(muon.len(), 32);
    for (i, d) in dong.iter().enumerate() {
        let ma = d.split_whitespace().next().unwrap_or("");
        assert_eq!(ma, muon[i], "dòng {i} sai thứ tự");
        let con_lai = d[ma.len()..].trim();
        assert!(!con_lai.is_empty(), "dòng {i} thiếu câu: {d:?}");
    }
    assert!(!out.contains("VS0801"), "không có mã ngoài sổ");
}

/// Mã không có ⇒ lỗi rõ + exit 2.
#[test]
fn ma_la_ro_rang_exit_2() {
    let (code, _, err) = vis(&["explain", "VS9999"]);
    assert_eq!(code, 2);
    assert!(err.contains("không có mã VS9999"), "stderr: {err}");
}

/// `--lang` sai (và thiếu giá trị) ⇒ exit 2, không in bảng.
#[test]
fn lang_sai_exit_2() {
    for args in [
        vec!["explain", "--lang", "fr", "VS0504"],
        vec!["explain", "--lang"],
        vec!["explain", "--list", "--lang", "de"],
    ] {
        let (code, out, _) = vis(&args);
        assert_eq!(code, 2, "args: {args:?}");
        assert!(out.is_empty(), "không được in: {out}");
    }
}

/// Trợ giúp `vis explain` (không tham số) phải nêu **đúng số mã** của sổ.
#[test]
fn tro_giup_neu_du_so_ma() {
    let (code, out, err) = vis(&["explain"]);
    assert_eq!(code, 2, "thiếu tham số ⇒ 2");
    let text = format!("{out}{err}");
    assert!(text.contains("dùng:"), "{text}");
    assert!(text.contains("32"), "phải nêu 32 mã: {text}");
    assert!(text.contains("7 khối"), "phải nêu 7 khối: {text}");
}

/// Quét **toàn bộ** sổ: cả 32 mã có `message_vi/en/ja` **không rỗng** và **khác nhau** (không
/// copy-paste lẫn ngôn ngữ).
#[test]
fn toan_bo_so_du_ba_ngon_ngu() {
    let mut n = 0usize;
    for c in vis_diag::all_codes() {
        for (ten, m) in [
            ("vi", c.message_vi),
            ("en", c.message_en),
            ("ja", c.message_ja),
        ] {
            assert!(!m.trim().is_empty(), "{} thiếu message_{ten}", c.short);
        }
        assert_ne!(c.message_vi, c.message_en, "{} vi == en", c.short);
        assert_ne!(c.message_vi, c.message_ja, "{} vi == ja", c.short);
        assert_ne!(c.message_en, c.message_ja, "{} en == ja", c.short);
        n += 1;
    }
    assert_eq!(n, 32, "sổ phải đúng 32 mã");
}
