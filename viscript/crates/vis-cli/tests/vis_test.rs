// SPDX-License-Identifier: MIT OR Apache-2.0
//! ST-30 — **`vis test`**: chạy bộ `test/sem/**` in **TAP**, đếm khớp harness (346), `--vm` khớp interp.
//!
//! Khoá: `1..N` đúng số ca; ca đỏ ⇒ exit 1 + `not ok` + dòng `# <tệp>:<dòng>:<cột>: <mã>`; `--vm`
//! chạy đường VM (SKIP ca cần host chương trình ngoài — VM không có, ST-27).

use std::path::PathBuf;
use std::process::Command;

/// Số ca mà harness `sem.rs` đang khoá — `vis test` **phải** khớp, không tự bịa.
/// **SỬA KÈM LÝ DO (ST-56)**: 370 → **376** (+6 ca nợ lõi A).
/// **SỬA KÈM LÝ DO (ST-56b)**: 376 → **382** (+6 ca phép gán).
/// **SỬA KÈM LÝ DO (ST-57)**: 382 → **388** (+6 ca `trait/`: siêu trait ×2, `derives` ×4).
/// **SỬA KÈM LÝ DO (ST-59)**: 388 → **394** (+6 ca `vish/`: shell `let`/biến stage đầu/thân `cmd`).
/// **SỬA KÈM LÝ DO (ST-60)**: 394 → **416** (+22 ca `async/` — chốt VD-90/VD-95).
/// **SỬA KÈM LÝ DO (ST-60b)**: 416 → **420** (+4 ca hotfix closure/đích lồng/`std --tên`).
/// **SỬA KÈM LÝ DO (ST-60c)**: 419 → **426** (+7 ca `trait/`: derives↔siêu trait, enum Show, Eq sâu).
/// **SỬA KÈM LÝ DO (ST-61)**: 426 → **441** (+15 ca nhóm `kenh/`).
/// **SỬA KÈM LÝ DO (ST-72)**: 441 → **447** (+6 ca nhóm `cap/`).
const SO_CA: usize = 447;

fn vis(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_vis"))
        .args(args)
        .output()
        .expect("chạy `vis test`");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).to_string(),
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

fn dong(s: &str) -> Vec<&str> {
    s.lines().collect()
}

fn tam(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vis-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("thư mục tạm");
    d
}

#[test]
fn tap_hop_le_va_du_so_ca() {
    let (c, o, e) = vis(&["test"]);
    let ls = dong(&o);
    assert_eq!(c, 0, "exit phải 0 khi toàn xanh: {e}");
    assert_eq!(ls[0], "TAP version 13");
    assert_eq!(ls[1], format!("1..{SO_CA}"), "kế hoạch phải khớp harness");
    let ok = ls.iter().filter(|l| l.starts_with("ok ")).count();
    assert_eq!(ok, SO_CA, "phải {SO_CA} dòng `ok`");
    assert_eq!(ls.iter().filter(|l| l.starts_with("not ok")).count(), 0);
}

#[test]
fn cac_dong_ok_dung_thu_tu_khong_lap() {
    let (_, o, _) = vis(&["test"]);
    let ls = dong(&o);
    for (i, l) in ls.iter().filter(|l| l.starts_with("ok ")).enumerate() {
        assert!(
            l.starts_with(&format!("ok {} - ", i + 1)),
            "dòng thứ {} sai thứ tự: {l}",
            i + 1
        );
    }
}

#[test]
fn vm_khop_interp_va_co_skip_hop_le() {
    let (c, o, e) = vis(&["test", "--vm"]);
    let ls = dong(&o);
    assert_eq!(c, 0, "VM: exit phải 0 (ca cần host thì SKIP): {e}");
    assert_eq!(ls[1], format!("1..{SO_CA}"));
    assert_eq!(
        ls.iter().filter(|l| l.starts_with("not ok")).count(),
        0,
        "--vm không được có ca đỏ"
    );
    let skip = ls.iter().filter(|l| l.contains("# SKIP")).count();
    assert!(
        skip >= 4,
        "phải có ít nhất 4 ca SKIP (chương trình ngoài cần host), thật {skip}"
    );
}

#[test]
fn ca_do_thoat_1_va_co_dong_chi_vi_tri() {
    let d = tam("do");
    std::fs::write(d.join("sai.vis"), "print(1)\n").expect("ghi");
    std::fs::write(
        d.join("sai.out"),
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n2\n",
    )
    .expect("ghi");
    let (c, o, e) = vis(&["test", d.to_str().unwrap()]);
    assert_eq!(c, 1, "phải exit 1 khi có ca đỏ: {e}");
    let ls = dong(&o);
    assert_eq!(ls[1], "1..1");
    assert!(ls[2].starts_with("not ok 1"), "TAP: {o}");
    assert!(
        ls[3].starts_with("# sai.vis:1:1:"),
        "dòng chẩn đoán phải `<tệp>:<dòng>:<cột>:` — thật {:?}",
        ls.get(3)
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ca_loi_dung_ma_thi_ok() {
    let d = tam("err");
    std::fs::write(d.join("loi.vis"), "let x: int = \"a\"\n").expect("ghi");
    std::fs::write(d.join("loi.err"), "VS0412\n").expect("ghi");
    let (c, o, e) = vis(&["test", d.to_str().unwrap()]);
    assert_eq!(c, 0, "ca lỗi đúng mã phải ok: {o} {e}");
    assert!(dong(&o)[2].starts_with("ok 1"), "TAP: {o}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn mot_tep_chay_duoc() {
    let d = tam("mot");
    let p = d.join("ok.vis");
    std::fs::write(&p, "print(1 + 1)\n").expect("ghi");
    std::fs::write(
        d.join("ok.out"),
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n2\n",
    )
    .expect("ghi");
    let (c, o, _) = vis(&["test", p.to_str().unwrap()]);
    assert_eq!(c, 0);
    assert_eq!(dong(&o)[1], "1..1");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn co_la_la_dung_sai_thoat_2() {
    let (c, _, e) = vis(&["test", "--bogus"]);
    assert_eq!(c, 2);
    assert!(e.contains("dùng: vis test"), "stderr: {e}");
}

#[test]
fn duong_dan_khong_ton_tai_thoat_2() {
    let (c, _, e) = vis(&["test", "/khong/co/that"]);
    assert_eq!(c, 2, "stderr: {e}");
}

#[test]
fn tap_moi_not_ok_deu_co_dong_chan_doan() {
    let d = tam("nhieu");
    std::fs::write(d.join("a.vis"), "print(1)\n").expect("ghi");
    std::fs::write(
        d.join("a.out"),
        "# SPDX-License-Identifier: MIT OR Apache-2.0\n9\n",
    )
    .expect("ghi");
    std::fs::write(d.join("b.vis"), "print(\"x\")\n").expect("ghi");
    std::fs::write(
        d.join("b.out"),
        "# SPDX-License-Identifier: MIT OR Apache-2.0\nx\n",
    )
    .expect("ghi");
    let (c, o, _) = vis(&["test", d.to_str().unwrap()]);
    assert_eq!(c, 1);
    let ls = dong(&o);
    assert_eq!(ls[1], "1..2");
    for (i, l) in ls.iter().enumerate() {
        if l.starts_with("not ok") {
            assert!(
                ls.get(i + 1).map(|x| x.starts_with("# ")).unwrap_or(false),
                "mỗi `not ok` phải kèm dòng `# …`: {:?}",
                ls.get(i + 1)
            );
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// **ST-32**: ca `.err` lỗi **biên dịch** phải ra **cùng mã** ở `--vm` (không SKIP, không khác mã) —
/// trước ST-30b VM in `VS0501` cho lỗi chỉ số, nay đồng bộ; ca này khoá nhánh **biên dịch**.
#[test]
fn err_bien_dich_ra_cung_ma_o_vm() {
    let d = tam("biendich");
    std::fs::write(d.join("sai.vis"), "let x: int = \"chuoi\"\n").expect("ghi");
    std::fs::write(d.join("sai.err"), "VS0412\n").expect("ghi");
    let (ci, oi, ei) = vis(&["test", d.to_str().unwrap()]);
    let (cv, ov, ev) = vis(&["test", "--vm", d.to_str().unwrap()]);
    assert_eq!(ci, 0, "interp phải ok: {oi} {ei}");
    assert_eq!(cv, 0, "--vm phải ok (cùng mã VS0412): {ov} {ev}");
    assert!(oi.contains("ok 1"), "TAP interp: {oi}");
    assert!(ov.contains("ok 1"), "TAP vm: {ov}");
    assert!(
        !ov.contains("# SKIP"),
        "ca lỗi biên dịch **không** được SKIP ở --vm: {ov}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
