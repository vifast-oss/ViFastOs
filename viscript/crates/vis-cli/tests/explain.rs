// SPDX-License-Identifier: MIT OR Apache-2.0
//! Test đỏ trước của ST-02b: `vis explain` phải chạy được (DoD "vis explain được").

use std::process::Command;

fn vis(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(args)
        .output()
        .expect("chạy được binary vis");
    let code = out.status.code().unwrap_or(-1);
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (code, text)
}

#[test]
fn explain_mot_ma_co_nghia() {
    let (code, text) = vis(&["explain", "VS0103"]);
    assert_eq!(code, 0, "explain mã có thật phải exit 0, được: {text}");
    assert!(text.contains("VS0103"), "{text}");
    assert!(
        text.contains("ký tự lạ"),
        "phải in nghĩa của VS0103: {text}"
    );
}

#[test]
fn explain_ma_la_thi_do() {
    let (code, text) = vis(&["explain", "VS9999"]);
    assert_eq!(code, 2, "mã lạ là lỗi dùng ⇒ exit 2 (VD-20): {text}");
    assert!(text.contains("không có mã"), "{text}");
}

#[test]
fn explain_khong_tham_so_thi_doi_cach_dung() {
    let (code, text) = vis(&["explain"]);
    assert_eq!(code, 2, "thiếu tham số phải exit 2: {text}");
    assert!(text.contains("dùng:"), "{text}");
}

#[test]
fn list_in_du_10_ma_cua_spec_lex() {
    let (code, text) = vis(&["explain", "--list"]);
    assert_eq!(code, 0, "{text}");
    for n in 101..=110u16 {
        let want = format!("VS{n:04}");
        assert!(text.contains(&want), "bảng thiếu {want}: {text}");
    }
}

#[test]
fn explain_lang_en_va_ja() {
    let (code, text) = vis(&["explain", "--lang", "en", "VS0101"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("unterminated string"), "{text}");
    let (code, text) = vis(&["explain", "--lang", "ja", "VS0202"]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("50pct") && text.contains("パーセント"),
        "{text}"
    );
}

#[test]
fn list_lang_in_du_ba_ngon_ngu_tung_ma() {
    for lang in ["vi", "en", "ja"] {
        let (code, text) = vis(&["explain", "--list", "--lang", lang]);
        assert_eq!(code, 0, "{text}");
        for n in 101..=110u16 {
            assert!(
                text.contains(&format!("VS{n:04}")),
                "bảng {lang} thiếu VS{n:04}"
            );
        }
        assert!(text.contains("VS0201") && text.contains("VS0202"), "{text}");
    }
    let (_, en) = vis(&["explain", "--list", "--lang", "en"]);
    assert!(en.contains("unterminated string"), "{en}");
    let (_, ja) = vis(&["explain", "--list", "--lang", "ja"]);
    assert!(ja.contains("文字列が閉じられていません"), "{ja}");
}

#[test]
fn explain_lang_sai_hoac_thieu_gia_tri_la_loi_dung() {
    let (code, _) = vis(&["explain", "--lang", "fr", "VS0101"]);
    assert_eq!(code, 2, "ngôn ngữ lạ là lỗi dùng");
    let (code, _) = vis(&["explain", "VS0101", "--lang"]);
    assert_eq!(code, 2, "thiếu giá trị --lang là lỗi dùng");
}

#[test]
fn explain_so_ten_03xx() {
    // ST-11 (VD-42/VD-44 #7): `vis explain` phải tra được **sổ thứ ba** (tên).
    let (code, text) = vis(&["explain", "VS0302"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("VS0302"), "{text}");
    assert!(text.contains("khai báo trùng"), "{text}");
    let (code, text) = vis(&["explain", "--lang", "en", "VS0301"]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("name does not exist"), "{text}");
    // `--list` phủ cả 3 sổ; `VS0412` (sổ kiểu) chưa cấp ở ST-11 ⇒ không có trong danh sách.
    let (code, text) = vis(&["explain", "--list"]);
    assert_eq!(code, 0, "{text}");
    for want in ["VS0301", "VS0302"] {
        assert!(text.contains(want), "--list thiếu {want}: {text}");
    }
    // Sửa kỳ vọng cũ kèm lý do: ST-12 cấp `04xx`, ST-13 cấp `05xx`, **ST-18 cấp `06xx`** ⇒
    // `VS0501`/`VS0601` nay có trong danh sách + `explain` exit 0; mã **chưa** cấp là `VS0701`.
    assert!(text.contains("VS0412"), "ST-12 cấp 04xx: {text}");
    assert!(text.contains("VS0501"), "ST-13 cấp 05xx: {text}");
    assert!(text.contains("VS0601"), "ST-18 cấp 06xx: {text}");
    let (code, _) = vis(&["explain", "VS0701"]);
    assert_eq!(code, 2, "mã chưa cấp (07xx) phải exit 2");
}

#[test]
fn explain_so_kieu_04xx() {
    // ST-12 (VD-42): sổ thứ tư; `04xx` cấp đủ 4 mã.
    // **Sửa kỳ vọng cũ kèm lý do**: ST-13 đã cấp sổ runtime `05xx` ⇒ `VS0501` nay exit 0, không phải 2.
    for (ma, tu) in [
        ("VS0401", "đối số"),
        ("VS0402", "xét hết"),
        ("VS0403", "không tồn tại"),
        ("VS0412", "không khớp"),
    ] {
        let (code, text) = vis(&["explain", ma]);
        assert_eq!(code, 0, "{ma}: {text}");
        assert!(text.contains(ma) && text.contains(tu), "{ma}: {text}");
    }
    let (code, text) = vis(&["explain", "--lang", "en", "VS0412"]);
    assert_eq!(code, 0);
    assert!(text.contains("type mismatch"), "{text}");
    let (code, text) = vis(&["explain", "--list"]);
    assert_eq!(code, 0);
    for want in ["VS0301", "VS0412", "VS0402"] {
        assert!(text.contains(want), "--list thiếu {want}");
    }
    let (code, _) = vis(&["explain", "VS0501"]);
    assert_eq!(code, 0, "ST-13 đã cấp 05xx");
    // **SỬA KÈM LÝ DO (ST-18)**: `06xx` đã cấp ⇒ `VS0601` exit 0; mã chưa cấp là `VS0701`.
    let (code, _) = vis(&["explain", "VS0601"]);
    assert_eq!(code, 0, "ST-18 đã cấp 06xx");
    let (code, _) = vis(&["explain", "VS0701"]);
    assert_eq!(code, 2, "07xx chưa cấp");
}

#[test]
fn explain_so_runtime_05xx() {
    // ST-13 (VD-42): sổ thứ năm; đủ 3 mã.
    for (ma, tu) in [
        ("VS0501", "runtime"),
        ("VS0502", "cho 0"),
        ("VS0503", "ngoài phạm vi"),
    ] {
        let (code, text) = vis(&["explain", ma]);
        assert_eq!(code, 0, "{ma}: {text}");
        assert!(text.contains(ma) && text.contains(tu), "{ma}: {text}");
    }
    let (code, text) = vis(&["explain", "--lang", "ja", "VS0503"]);
    assert_eq!(code, 0);
    assert!(text.contains("範囲外"), "{text}");
}
