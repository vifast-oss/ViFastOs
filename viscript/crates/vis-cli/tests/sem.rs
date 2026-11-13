// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-14 — harness ngữ nghĩa (TAP) chạy qua binary `vis run`.**
//!
//! Mỗi ca: `test/sem/<nhóm>/<ca>.vis` + **hoặc** `<ca>.out` (stdout kỳ vọng, so **từng byte**)
//! **hoặc** `<ca>.err` (mã lỗi kỳ vọng: `VS05xx` ⇒ exit 1, `0xxx` ⇒ exit 2).
//!
//! Chống xanh-giả: (i) so **byte** stdout, không cắt/normalize; (ii) đọc `steps=N` từ TAP
//! (`vis run --tap`) và **đòi `N > 0`** cho ca chạy được — ca không chạy bước nào mà báo `ok` là
//! **SAI**; (iii) khoá **đúng số ca tìm thấy** (100) + mỗi nhóm ≥ 6 ca, để không âm thầm bỏ ca.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Bỏ **dòng SPDX đầu** của tệp kỳ vọng (QD §9.3 yêu cầu SPDX cho mọi tệp văn bản); phần còn lại
/// vẫn được so **từng byte** với stdout.
fn boc_spdx(t: &str) -> String {
    match t.strip_prefix("# SPDX-License-Identifier") {
        Some(rest) => match rest.find('\n') {
            Some(i) => rest[i + 1..].to_string(),
            None => String::new(),
        },
        None => t.to_string(),
    }
}

fn vis() -> &'static str {
    env!("CARGO_BIN_EXE_vis")
}

fn sem_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("test")
        .join("sem")
}

struct Ca {
    ten: String,
    nhom: String,
    vis: PathBuf,
    out: Option<String>,
    err: Option<String>,
}

fn gom_ca() -> Vec<Ca> {
    let goc = sem_dir();
    let mut ds = Vec::new();
    let mut nhom_dir: Vec<PathBuf> = std::fs::read_dir(&goc)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", goc.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    nhom_dir.sort();
    for d in nhom_dir {
        let nhom = d.file_name().unwrap().to_string_lossy().to_string();
        let mut ca: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap_or_else(|e| panic!("không đọc được {}: {e}", d.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .map(|x| x == "vis" || x == "vish")
                    .unwrap_or(false)
            })
            .collect();
        ca.sort();
        for f in ca {
            let stem = f.with_extension("");
            let out = std::fs::read_to_string(stem.with_extension("out"))
                .ok()
                .map(|t| boc_spdx(&t));
            let err = std::fs::read_to_string(stem.with_extension("err"))
                .ok()
                .map(|t| boc_spdx(&t));
            assert!(
                out.is_some() ^ err.is_some(),
                "{}: phải có **đúng một** trong `.out`/`.err`",
                f.display()
            );
            ds.push(Ca {
                ten: f.file_stem().unwrap().to_string_lossy().to_string(),
                nhom: nhom.clone(),
                vis: f,
                out,
                err,
            });
        }
    }
    ds
}

/// Bóc `steps=N` từ stderr TAP.
fn steps_cua(stderr: &str) -> usize {
    for line in stderr.lines() {
        if let Some(i) = line.find("steps=") {
            let rest = &line[i + "steps=".len()..];
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num.parse() {
                return n;
            }
        }
    }
    0
}

#[test]
fn sem_323_ca_qua_binary() {
    let ds = gom_ca();
    let mut theo_nhom: BTreeMap<String, usize> = BTreeMap::new();
    for c in &ds {
        *theo_nhom.entry(c.nhom.clone()).or_default() += 1;
    }
    // ST-15: khoá **ca / nhóm**, mỗi nhóm ≥ 6 — thiếu/thừa/thiếu nhóm là ĐỎ.
    // ST-18: 309 `.vis` + 14 `.vish` (nhóm `vish`, chế độ lệnh) ⇒ **323 ca / 17 nhóm**.
    // **SỬA KÈM LÝ DO (ST-35)**: +8 ca `generic/*` (instantiation thật: `Hop[int]` ≠ `Hop[str]`,
    // `Or[int, str]`, hàm liên kết) ⇒ **346 ca / 18 nhóm** (ST-28 trước: 338).
    // **SỬA KÈM LÝ DO (ST-37)**: +3 ca `vish/khe-*.vish` (khe ống hợp nhất: `( [1,2,3] ) | take 2`,
    // …) ⇒ **349 ca / 18 nhóm** — xếp vào nhóm `vish` để **không** tạo nhóm < 6 ca.
    assert_eq!(
        ds.len(),
        349,
        "phải đúng 349 ca (330 `.vis` + 22 `.vish`), thật {} — nhóm: {theo_nhom:?}",
        ds.len()
    );
    assert_eq!(
        theo_nhom.len(),
        18,
        "phải đúng 18 nhóm (ST-28 thêm `generic`), thật {theo_nhom:?}"
    );
    // **Chống bỏ ca**: số tệp `.vis` trên đĩa phải **bằng** số ca đã gom (không ca nào bị lọc mất).
    let tren_dia: usize = std::fs::read_dir(sem_dir())
        .expect("đọc test/sem")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .map(|d| {
            std::fs::read_dir(d)
                .expect("đọc nhóm")
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .map(|x| x == "vis" || x == "vish")
                        .unwrap_or(false)
                })
                .count()
        })
        .sum();
    assert_eq!(
        tren_dia,
        ds.len(),
        "số tệp `.vis`/`.vish` ({tren_dia}) phải bằng số ca chạy ({})",
        ds.len()
    );
    for (nhom, n) in &theo_nhom {
        assert!(*n >= 6, "nhóm `{nhom}` chỉ có {n} ca (cần ≥ 6)");
    }

    let mut sai = Vec::new();
    for c in &ds {
        let res = Command::new(vis())
            .arg("run")
            .arg("--tap")
            .arg(&c.vis)
            .output()
            .unwrap_or_else(|e| panic!("không chạy được `vis run`: {e}"));
        let stdout = String::from_utf8_lossy(&res.stdout).to_string();
        let stderr = String::from_utf8_lossy(&res.stderr).to_string();
        let code = res.status.code().unwrap_or(-1);
        let mo_ta = format!("{}/{}", c.nhom, c.ten);
        match (&c.out, &c.err) {
            (Some(want), None) => {
                if code != 0 || stdout != *want {
                    sai.push(format!(
                        "{mo_ta}: muốn exit 0 + stdout {want:?}, được exit {code} + stdout {stdout:?}\n  stderr: {stderr}"
                    ));
                    continue;
                }
                if steps_cua(&stderr) == 0 {
                    sai.push(format!(
                        "{mo_ta}: **chưa chạy bước nào** (steps=0) mà báo ok"
                    ));
                }
            }
            (None, Some(ma)) => {
                let ma = ma.trim();
                let muon_exit = if ma.starts_with("VS05") { 1 } else { 2 };
                if code != muon_exit || !stderr.contains(ma) {
                    sai.push(format!(
                        "{mo_ta}: muốn exit {muon_exit} + stderr chứa `{ma}`, được exit {code}\n  stderr: {stderr}"
                    ));
                    continue;
                }
                // Lỗi runtime thì chương trình **đã chạy** ít nhất một bước.
                if ma.starts_with("VS05") && steps_cua(&stderr) == 0 {
                    sai.push(format!("{mo_ta}: lỗi runtime mà steps=0 (chưa chạy gì)"));
                }
            }
            _ => unreachable!(),
        }
    }
    assert!(
        sai.is_empty(),
        "{}/{} ca sai:\n{}",
        sai.len(),
        ds.len(),
        sai.join("\n")
    );
}
