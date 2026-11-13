// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-33** — `json.decode` + `std.fs` tối thiểu (**qua `Host`**). Không fixture sem nào cần FS
//! (giữ 346 ca **hermetic**); toàn bộ ca ở đây dùng **thư mục tạm** và tự dọn.
//!
//! Khoá: round-trip encode↔decode, escape đủ (kể cả **surrogate pair**), tràn i64/`1e400`,
//! **byte offset** khi JSON hỏng, `fs` read/write/exists/list (sắp xếp, không đệ quy), `--explain-perms`
//! in `FS …`, và **không host ⇒ `VS0604` ở CẢ HAI đường** (interp + VM).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static DEM: AtomicU32 = AtomicU32::new(0);

/// Thư mục tạm **riêng** cho từng ca (chống đụng khi test chạy song song), tự dọn ở cuối ca.
fn tam(tag: &str) -> PathBuf {
    let n = DEM.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vis-st33-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tạo thư mục tạm");
    d
}

fn vis(args: &[&std::ffi::OsStr]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    for a in args {
        c.arg(a);
    }
    let o = c.output().expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

/// Chạy **cả hai đường** (interp + `--vm --no-cache`), đòi **cùng** stdout/exit; trả về stdout.
fn hai_duong(d: &std::path::Path, tag: &str, src: &str) -> String {
    let p = d.join(format!("{tag}.vis"));
    std::fs::write(
        &p,
        format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}"),
    )
    .expect("ghi");
    let (c1, o1, e1) = vis(&[std::ffi::OsStr::new("run"), p.as_os_str()]);
    let (c2, o2, e2) = vis(&[
        std::ffi::OsStr::new("run"),
        std::ffi::OsStr::new("--vm"),
        std::ffi::OsStr::new("--no-cache"),
        p.as_os_str(),
    ]);
    assert_eq!(c1, 0, "[{tag}] interp exit {c1}: {e1}");
    assert_eq!(c2, 0, "[{tag}] vm exit {c2}: {e2}");
    assert_eq!(o1, o2, "[{tag}] hai đường phải **khớp**: vm [{e2}]");
    o1
}

/// Escape **văn bản JSON thô** thành literal chuỗi của VS: `\`→`\\`, `"`→`\"`, `{`→`{{`,
/// `}`→`}}` (VS nội suy `{…}` trong chuỗi, VD-18).
fn vs_lit(json: &str) -> String {
    let mut out = String::with_capacity(json.len() + 8);
    for c in json.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str("\\\""),
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            '\n' => out.push_str(r"\n"),
            '\t' => out.push_str(r"\t"),
            '\r' => out.push_str(r"\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Thân chương trình: `json.decode(<json>)` ⇒ in `json.encode(v)`, lỗi ⇒ `print("E")` + `print(e)`.
fn ct_decode(json: &str) -> String {
    format!(
        "    match json.decode(\"{}\"):\n        ok(v):\n            print(json.encode(v))\n        err(e):\n            print(\"E\")\n            print(e)\n",
        vs_lit(json)
    )
}

/// Chương trình in **kết quả hoặc lỗi** của một chuỗi JSON.
fn chuong_trinh_json(json: &str) -> String {
    format!("use std.json\nfn main() -> ():\n{}", ct_decode(json))
}

// ---------- json.decode: round-trip (in encode(v) hoặc "E" + lỗi) ----------

#[test]
fn round_trip_so_va_bool_va_none() {
    let d = tam("rt-so");
    for (json, muon) in [
        ("42", "42"),
        ("-7", "-7"),
        ("1.5", "1.5"),
        ("true", "true"),
        ("false", "false"),
        ("null", "null"),
        ("0", "0"),
        ("1e3", "1000"),
        ("-2.5e-2", "-0.025"),
    ] {
        let o = hai_duong(&d, "rt", &chuong_trinh_json(json));
        assert_eq!(o, format!("{muon}\n"), "JSON {json}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn round_trip_chuoi_va_unicode() {
    let d = tam("rt-str");
    let o = hai_duong(&d, "rt", &chuong_trinh_json("\"xin chào 😀\""));
    assert_eq!(o, "\"xin chào 😀\"\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn round_trip_list_va_map_giu_thu_tu() {
    let d = tam("rt-arr");
    for (json, muon) in [
        ("[1, 2, 3]", "[1,2,3]"),
        (r#"{"b": 1, "a": 2}"#, r#"{"b":1,"a":2}"#),
        ("[]", "[]"),
        ("{}", "{}"),
    ] {
        let o = hai_duong(&d, "rt", &chuong_trinh_json(json));
        assert_eq!(o, format!("{muon}\n"), "JSON {json}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn round_trip_long_nhau_va_trung_khoa() {
    let d = tam("rt-nest");
    let o = hai_duong(
        &d,
        "rt",
        &chuong_trinh_json(r#"{"a": {"b": [1, {"c": "x"}]}, "d": null}"#),
    );
    assert_eq!(o, "{\"a\":{\"b\":[1,{\"c\":\"x\"}]},\"d\":null}\n");
    // Trùng khoá: **giá trị cuối**, **vị trí đầu**.
    let o2 = hai_duong(&d, "rt2", &chuong_trinh_json(r#"{"k": 1, "k": 2}"#));
    assert_eq!(o2, "{\"k\":2}\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn round_trip_ban_ghi_thanh_map() {
    let d = tam("rt-rec");
    let o = hai_duong(
        &d,
        "rec",
        "use std.json\ntype Hop:\n    gia_tri: int\n    ten: str\n\nfn main() -> ():\n    let h: Hop = {gia_tri: 7, ten: \"a\"}\n    let enc = json.encode(h)\n    match json.decode(enc):\n        ok(v):\n            print(json.encode(v))\n        err(e):\n            print(\"E\")\n",
    );
    assert_eq!(o, "{\"gia_tri\":7,\"ten\":\"a\"}\n");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- json.decode: escape ----------

#[test]
fn escape_co_ban() {
    let d = tam("esc");
    let o = hai_duong(&d, "esc", &chuong_trinh_json(r#""a\nb\tc\rd\\e\/f\"g""#));
    assert_eq!(o, "\"a\\nb\\tc\\rd\\\\e/f\\\"g\"\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn escape_surrogate_pair() {
    let d = tam("sur");
    // Cặp surrogate ⇒ **một** scalar 😀; `\u0041` ⇒ `A`.
    let o = hai_duong(&d, "sur", &chuong_trinh_json(r#""\uD83D\uDE00""#));
    assert_eq!(o, "\"😀\"\n");
    let o2 = hai_duong(&d, "sur2", &chuong_trinh_json(r#""\u0041\u00e1""#));
    assert_eq!(o2, "\"Aá\"\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn surrogate_don_la_loi() {
    let d = tam("sur-don");
    for json in [r#""\uD83D""#, r#""\uDE00""#] {
        let o = hai_duong(&d, "surdon", &chuong_trinh_json(json));
        assert!(
            o.starts_with("E\n") && o.contains("surrogate"),
            "JSON {json}: {o}"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- json.decode: số + lỗi + byte offset ----------

#[test]
fn so_tran_i64_la_loi() {
    let d = tam("tran");
    let o = hai_duong(&d, "tran", &chuong_trinh_json("9223372036854775808"));
    assert!(
        o.starts_with("E\n") && o.contains("byte 0"),
        "lỗi tràn: {o}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn float_tran_la_loi() {
    let d = tam("ftran");
    let o = hai_duong(&d, "ftran", &chuong_trinh_json("1e400"));
    assert!(
        o.starts_with("E\n") && o.contains("tràn"),
        "lỗi float tràn: {o}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn json_hong_co_byte_offset() {
    let d = tam("hong");
    // `{"a": }` ⇒ ký tự `}` ở **byte 6**.
    let o = hai_duong(&d, "hong", &chuong_trinh_json(r#"{"a": }"#));
    assert!(o.starts_with("E\n"), "phải là lỗi: {o}");
    assert!(o.contains("byte 6"), "offset phải 6: {o}");
    assert!(o.contains("JSON không hợp lệ"), "thông điệp: {o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn json_hong_utf8_nhieu_byte_offset_van_dung() {
    let d = tam("utf8");
    // `"á😀"` = 1 + 2 + 4 + 1 = **8 byte**; dấu cách ở byte 8 ⇒ ký tự thừa `x` ở **byte 9**
    // (không phải char index 4). Đây là điểm khác nhau giữa **byte** và **char**.
    let o = hai_duong(&d, "utf8", &chuong_trinh_json("\"á😀\" x"));
    assert!(o.starts_with("E\n"), "phải là lỗi: {o}");
    assert!(o.contains("byte 9"), "offset theo BYTE phải 9: {o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn json_hong_thieu_ngoac_va_literal_sai() {
    let d = tam("hong2");
    for json in ["[1, ]", "nul", "}", "{\"a\":1} 2"] {
        let o = hai_duong(&d, "hong2", &chuong_trinh_json(json));
        assert!(o.starts_with("E\n"), "JSON {json} phải lỗi: {o}");
        assert!(o.contains("JSON không hợp lệ"), "JSON {json}: {o}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn decode_van_la_result_that() {
    let d = tam("res");
    let o = hai_duong(
        &d,
        "res",
        "use std.json\nfn main() -> ():\n    let v = json.decode(\"null\")\n    match v:\n        ok(x):\n            print(\"ok\")\n        err(e):\n            print(\"err\")\n",
    );
    assert_eq!(o, "ok\n");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- std.fs ----------

#[test]
fn fs_write_read_round_trip() {
    let d = tam("fs-rw");
    let p = d.join("a.txt");
    let o = hai_duong(
        &d,
        "rw",
        &format!(
            "use std.fs\nfn main() -> ():\n    match fs.write(\"{}\", \"xin chào\"):\n        ok(_):\n            print(\"w\")\n        err(e):\n            print(\"err\")\n    match fs.read(\"{}\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"err\")\n",
            p.display(),
            p.display()
        ),
    );
    assert_eq!(o, "w\nxin chào\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fs_exists_va_read_thieu_la_err() {
    let d = tam("fs-ex");
    let p = d.join("co.txt");
    std::fs::write(&p, "x").expect("ghi");
    let thieu = d.join("khong.txt");
    let o = hai_duong(
        &d,
        "ex",
        &format!(
            "use std.fs\nfn main() -> ():\n    print(fs.exists(\"{}\"))\n    print(fs.exists(\"{}\"))\n    match fs.read(\"{}\"):\n        ok(s):\n            print(\"ok\")\n        err(e):\n            print(\"err\")\n",
            p.display(),
            thieu.display(),
            thieu.display()
        ),
    );
    assert_eq!(o, "true\nfalse\nerr\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fs_list_sap_xep_khong_de_quy() {
    let d = tam("fs-ls");
    for t in ["b.txt", "a.txt", "c.txt"] {
        std::fs::write(d.join(t), "x").expect("ghi");
    }
    std::fs::create_dir_all(d.join("sub")).expect("mkdir");
    std::fs::write(d.join("sub").join("z.txt"), "x").expect("ghi");
    let pdir = tam("fs-ls-prog");
    let o = hai_duong(
        &pdir,
        "ls",
        &format!(
            "use std.fs\nfn main() -> ():\n    match fs.list(\"{}\"):\n        ok(xs):\n            for x in xs:\n                print(x)\n        err(e):\n            print(\"err\")\n",
            d.display()
        ),
    );
    assert_eq!(o, "a.txt\nb.txt\nc.txt\nsub\n");
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&pdir);
}

#[test]
fn fs_ghi_vao_duong_dan_sai_la_err() {
    let d = tam("fs-bad");
    let p = d.join("khong-co-thu-muc").join("x.txt");
    let o = hai_duong(
        &d,
        "bad",
        &format!(
            "use std.fs\nfn main() -> ():\n    match fs.write(\"{}\", \"x\"):\n        ok(_):\n            print(\"ok\")\n        err(e):\n            print(\"err\")\n",
            p.display()
        ),
    );
    assert_eq!(o, "err\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fs_duong_dan_tuyet_doi_duoc_phep() {
    let d = tam("fs-abs");
    let p = d.join("abs.txt");
    let o = hai_duong(
        &d,
        "abs",
        &format!(
            "use std.fs\nfn main() -> ():\n    match fs.write(\"{}\", \"y\"):\n        ok(_):\n            print(fs.exists(\"{}\"))\n        err(e):\n            print(\"err\")\n",
            p.display(),
            p.display()
        ),
    );
    assert_eq!(o, "true\n");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn explain_perms_in_fs_nhat_ky() {
    let d = tam("fs-log");
    let p = d.join("log.txt");
    let tep = d.join("log.vis");
    std::fs::write(
        &tep,
        format!(
            "# SPDX-License-Identifier: MIT OR Apache-2.0\nuse std.fs\nfn main() -> ():\n    match fs.write(\"{}\", \"x\"):\n        ok(_):\n            print(\"w\")\n        err(e):\n            print(\"err\")\n",
            p.display()
        ),
    )
    .expect("ghi");
    let (c, _, e) = vis(&[
        std::ffi::OsStr::new("run"),
        std::ffi::OsStr::new("--explain-perms"),
        tep.as_os_str(),
    ]);
    assert_eq!(c, 0, "stderr: {e}");
    assert!(e.contains("FS write"), "phải in nhật ký `FS write`: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- không host ⇒ VS0604 ở **CẢ HAI** đường (in-process) ----------

fn hir_cua(src: &str) -> (vis_types::Hir, vis_types::Symbols, vis_types::TypeTable) {
    let text = format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}");
    let lexed = vis_syntax::lex(&text, vis_syntax::Mode::Script);
    let parsed = vis_syntax::parse(&text, vis_syntax::Mode::Script);
    let h = vis_types::lower_with(
        &parsed.cst,
        &lexed,
        vis_types::LowerOptions {
            std_mods: vec!["fs".into(), "json".into()],
            ..Default::default()
        },
    );
    (h.hir, h.symbols, h.types)
}

#[test]
fn khong_host_bao_vs0604_ca_interp_va_vm() {
    let src = "use std.fs\nfn main() -> ():\n    match fs.read(\"/etc/hostname\"):\n        ok(s):\n            print(s)\n        err(e):\n            print(\"err\")\n";
    let text = format!("# SPDX-License-Identifier: MIT OR Apache-2.0\n{src}");
    let (hir, syms, types) = hir_cua(src);
    // Đường **interp**: `run` (không host).
    let a = vis_interp::run(&text, &hir, &syms, &types, &[]);
    assert_eq!(a.diagnostics.len(), 1, "interp: {:?}", a.diagnostics);
    assert_eq!(a.diagnostics[0].code_str(), "VS0604");
    assert!(a.diagnostics[0].message.contains("không có host"));
    // Đường **VM**: `run` (không host).
    let m = vis_bytecode::compile(&hir, &syms, &text).expect("compile");
    let b = vis_vm::run(&m, &text, &[]);
    assert_eq!(b.diagnostics.len(), 1, "vm: {:?}", b.diagnostics);
    assert_eq!(b.diagnostics[0].code_str(), "VS0604");
    assert!(b.diagnostics[0].message.contains("không có host"));
}

#[test]
fn khong_host_json_van_chay_vi_thuan() {
    // `json.decode` là **thuần** (không cần host) ⇒ cả hai đường vẫn chạy được.
    let d = tam("json-thuan");
    let o = hai_duong(
        &d,
        "thuan",
        "use std.json\nfn main() -> ():\n    match json.decode(\"[1,2]\"):\n        ok(v):\n            print(json.encode(v))\n        err(e):\n            print(\"E\")\n",
    );
    assert_eq!(o, "[1,2]\n");
    let _ = std::fs::remove_dir_all(&d);
}

// ---------- differential: chương trình dùng fs + json chạy **cả hai đường** ----------

#[test]
fn differential_fs_va_json_khop_hai_duong() {
    let d = tam("diff");
    let p = d.join("d.txt");
    let o = hai_duong(
        &d,
        "diff",
        &format!(
            "use std.fs\nuse std.json\nfn main() -> ():\n    match fs.write(\"{}\", \"[1, 2]\", ):\n        ok(_):\n            print(\"w\")\n        err(e):\n            print(\"e\")\n",
            p.display()
        ),
    );
    assert_eq!(o, "w\n");
    // differential thật: đọc-tính-ghi qua **cả hai** đường.
    let o2 = hai_duong(
        &d,
        "diff2",
        &format!(
            "use std.fs\nuse std.json\nfn main() -> ():\n    match fs.read(\"{}\"):\n        ok(s):\n            match json.decode(s):\n                ok(v):\n                    print(json.encode(v))\n                err(e):\n                    print(\"je\")\n        err(e):\n            print(\"fe\")\n",
            p.display()
        ),
    );
    assert_eq!(o2, "[1,2]\n");
    let _ = std::fs::remove_dir_all(&d);
}
