// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-96 (VD-112)** — bất biến **sổ mã ↔ mã thực dùng** trong **mã nguồn sản phẩm**:
//! `A` = literal `"VS0xxx"` quét từ `crates/*/src/**/*.rs`; `B` = `vis_diag::all_codes()`.
//! Khoá: `A ⊆ B` (mã dùng phải có trong sổ) và `B \ A == DU_PHONG` **đúng thứ tự**; mã dự phòng phải
//! khai kèm lý do, nên thêm mã vào sổ mà không dùng sẽ **làm đỏ** test.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Mã **dự phòng**: có trong sổ nhưng **chưa** dùng trong mã nguồn sản phẩm.
///
/// Hiện **rỗng** vì `B \ A` rỗng (32/32 mã đều được dùng). Khi thêm mã mới vào sổ mà chưa dùng, phải
/// thêm vào đây **kèm lý do** (ví dụ: dành cho lỗi `VS0704` sẽ dùng ở ST-9x) — không được để test
/// “xanh im lặng”.
const DU_PHONG: &[&str] = &[];

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Tệp `.rs` thuộc **mã nguồn sản phẩm** (`crates/<crate>/src/...`), **không** phải test.
fn la_nguon_san_pham(p: &Path) -> bool {
    let c: Vec<_> = p.components().map(|c| c.as_os_str()).collect();
    c.windows(3).any(|w| w[0] == "crates" && w[2] == "src")
}

fn quet_thu_muc(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            quet_thu_muc(&p, out);
        } else if p.extension().map(|x| x == "rs").unwrap_or(false) && la_nguon_san_pham(&p) {
            out.push(p);
        }
    }
}

/// Quét literal `"VS0ddd"` (đúng 3 chữ số sau `VS0`, đóng bằng `"`).
fn quet_a() -> BTreeSet<String> {
    let mut rs = Vec::new();
    quet_thu_muc(&goc().join("crates"), &mut rs);
    let mut a = BTreeSet::new();
    for p in rs {
        let s = std::fs::read_to_string(&p).unwrap_or_default();
        let b = s.as_bytes();
        let mut i = 0;
        while i + 8 <= b.len() {
            if b[i] == b'"' && &b[i + 1..i + 4] == b"VS0" {
                let ok = b[i + 4..i + 7].iter().all(u8::is_ascii_digit) && b[i + 7] == b'"';
                if ok {
                    let ma = std::str::from_utf8(&b[i + 1..i + 7]).unwrap_or("");
                    a.insert(ma.to_string());
                    i += 8;
                    continue;
                }
            }
            i += 1;
        }
    }
    a
}

fn b() -> Vec<String> {
    vis_diag::all_codes().map(|c| c.short.to_string()).collect()
}

/// (a) `A ⊆ B`: mọi literal `"VS0xxx"` trong mã nguồn sản phẩm phải có trong sổ.
#[test]
fn a_la_tap_con_cua_b() {
    let a = quet_a();
    let b: BTreeSet<String> = b().into_iter().collect();
    let thieu: Vec<&String> = a.difference(&b).collect();
    assert!(
        thieu.is_empty(),
        "mã dùng nhưng KHÔNG có trong sổ: {thieu:?}\nA={a:?}"
    );
    assert!(!a.is_empty(), "quét được rỗng — sai đường dẫn?");
}

/// (b) `B \ A == DU_PHONG` **đúng thứ tự** của sổ.
#[test]
fn b_tru_a_dung_du_phong() {
    let a = quet_a();
    let b = b();
    let b_a: Vec<String> = b.into_iter().filter(|m| !a.contains(m)).collect();
    let du_phong: Vec<String> = DU_PHONG.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        b_a, du_phong,
        "B\\A phải khớp DU_PHONG (thứ tự sổ) — thêm mã mới mà chưa dùng thì phải khai lý do"
    );
}

/// (c) Mọi mã trong `B` có `message_vi/en/ja` **không rỗng** (giữ bất biến ST-78).
#[test]
fn moi_ma_du_ba_ngon_ngu() {
    for c in vis_diag::all_codes() {
        for (ten, m) in [
            ("vi", c.message_vi),
            ("en", c.message_en),
            ("ja", c.message_ja),
        ] {
            assert!(!m.trim().is_empty(), "{} thiếu message_{ten}", c.short);
        }
    }
}

/// (d) `DU_PHONG` rỗng ở vòng này; mỗi mục (nếu có) phải có lý do ghi ngay trong `const` ở trên.
#[test]
fn du_phong_rong_va_co_ly_do() {
    // Không kiểm nội dung comment bằng máy (dễ giòn); test khoá **độ dài** và tài liệu trong mã.
    assert!(
        DU_PHONG.is_empty(),
        "DU_PHONG phải rỗng khi B\\A rỗng; nếu thêm, khai lý do trong const"
    );
}
