// SPDX-License-Identifier: Apache-2.0
//! **ST-84 (VD-109)** — completion/hover **module-aware**: tệp chính có trên đĩa ⇒ nạp module (dùng
//! [`modules::load_full_voi_text`]) và đưa item `pub` của module + hàm `std.*` đã `use` vào gợi ý.
//!
//! Chỉ **item** (label/kind/detail); `span` không dùng cho completion/hover module. `detail` ghi rõ
//! **nguồn** (`util: fn tinh(x: int) -> int`) để người dùng biết ký hiệu đến từ đâu.

use std::path::Path;

use vis_syntax::Mode;
use vis_types::{HItem, TypeTable};

use super::phan_tich::{self, GoiY, KetQua};

fn ten_nguon(rel: &str) -> String {
    rel.trim_end_matches(".vis")
        .trim_end_matches(".vish")
        .to_string()
}

/// Tên hiển thị gọn cho `Ty` (`list[str]`, `Result[(), str]`, …) — dùng cho detail hàm `std.*`.
fn ten_ty(t: vis_types::stdlib::Ty) -> &'static str {
    t.ten()
}

/// Gợi ý cho một item **không** thuộc tệp chính; `None` nếu không phải item công khai/không hỗ trợ.
fn goi_y_module(it: &HItem, tbl: &TypeTable, nguon: &str) -> Option<GoiY> {
    match it {
        // HIR chỉ giữ `is_pub` cho `fn`; item khác coi như hiển thị được (nợ: HIR thiếu cờ pub).
        HItem::Fn(f) if !f.is_pub => None,
        _ => phan_tich::goi_y_item(it, tbl).map(|mut g| {
            if let Some(d) = g.detail {
                g.detail = Some(format!("{nguon}: {d}"));
            } else {
                g.detail = Some(format!("{nguon}: {}", g.label));
            }
            g
        }),
    }
}

/// Hàm `std.<m>.<tên>` đã `use` ⇒ gợi ý (kind 3), detail `std.<m>: fn <tên>(…​) -> …`.
fn goi_y_std(m: &str, f: &vis_types::stdlib::StdFn) -> Option<GoiY> {
    let label = f.name.strip_prefix(&format!("{m}."))?;
    let ps: Vec<String> = f
        .params
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{}: {}", (b'a' + i as u8) as char, ten_ty(*t)))
        .collect();
    Some(GoiY {
        label: label.to_string(),
        kind: 3,
        detail: Some(format!(
            "std.{m}: fn {}({}) -> {}",
            label,
            ps.join(", "),
            ten_ty(f.ret)
        )),
    })
}

/// Phân tích tệp **có trên đĩa** kèm item module + `std.*` đã `use`; `None` nếu nạp lỗi.
pub(crate) fn phan_tich_module(path: &Path, text: &str, mode: Mode) -> Option<KetQua> {
    let l = crate::modules::load_full_voi_text(path.to_str()?, text, true, true).ok()?;
    let mut kq = phan_tich::phan_tich(text, mode);
    let ten_chinh = path.file_name()?.to_string_lossy().to_string();
    let mut da: std::collections::HashSet<String> =
        kq.items.iter().map(|g| g.label.clone()).collect();
    // 1) Item của module (không thuộc tệp chính), ưu tiên sau item tệp chính.
    for (i, it) in l.hir.items.iter().enumerate() {
        let rel = l.item_rel.get(i).cloned().unwrap_or_default();
        if rel == ten_chinh {
            continue;
        }
        let nguon = ten_nguon(&rel);
        if let Some(g) = goi_y_module(it, &l.types, &nguon) {
            if da.insert(g.label.clone()) {
                kq.items.push(g);
            }
        }
    }
    // 2) Hàm `std.*` của các `use std.<m>` (kể cả `use` trong module phụ? chỉ HIR có `Use`).
    let mut mod_std: Vec<String> = Vec::new();
    for it in &l.hir.items {
        if let HItem::Use(u) = it {
            if u.path.len() >= 2 && u.path[0] == "std" && !mod_std.contains(&u.path[1]) {
                mod_std.push(u.path[1].clone());
            }
        }
    }
    for m in mod_std {
        for f in vis_types::stdlib::FUNCS.iter() {
            if !f.name.starts_with(&format!("{m}.")) {
                continue;
            }
            if let Some(g) = goi_y_std(&m, f) {
                if da.insert(g.label.clone()) {
                    kq.items.push(g);
                }
            }
        }
    }
    Some(kq)
}

/// Phân tích theo URI: tệp có trên đĩa ⇒ module-aware; ngược lại đường **một tệp** như trước.
pub(crate) fn phan_tich_uri(uri: &str, text: &str, mode: Mode) -> KetQua {
    if let Some(p) = super::uri_thanh_duong(uri).filter(|p| Path::new(p).is_file()) {
        if let Some(kq) = phan_tich_module(Path::new(&p), text, mode) {
            return kq;
        }
    }
    phan_tich::phan_tich(text, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **ST-84**: nạp module cho 5 tệp nhỏ **< 100 ms** (đo **trong tiến trình**, không tính spawn);
    /// mỗi tệp có `use ./util` và `util.vis` `pub fn` — kiểm luôn item module xuất hiện.
    #[test]
    fn hieu_nang_nap_module_5_tep() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static DEM: AtomicUsize = AtomicUsize::new(0);
        let n = DEM.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vis-mod-perf-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("tmp");
        std::fs::write(
            d.join("util.vis"),
            "pub fn tinh(x: int) -> int:\n    return x + 1\n",
        )
        .expect("ghi util");
        let src = "use ./util\n\nfn main():\n    print(tinh(1))\n";
        let tep: Vec<std::path::PathBuf> = (0..5)
            .map(|i| {
                let p = d.join(format!("t{i}.vis"));
                std::fs::write(&p, src).expect("ghi");
                p
            })
            .collect();
        let t = std::time::Instant::now();
        for p in &tep {
            let kq = phan_tich_module(p, src, Mode::Script).expect("nạp module");
            assert!(
                kq.items.iter().any(|g| g.label == "tinh"),
                "thiếu item module: {}",
                p.display()
            );
        }
        let ms = t.elapsed().as_millis();
        assert!(ms < 100, "nạp module 5 tệp mất {ms} ms (≥ 100 ms)");
        let _ = std::fs::remove_dir_all(&d);
    }
}
