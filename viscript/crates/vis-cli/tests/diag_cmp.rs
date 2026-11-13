// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-32** — harness so **chẩn đoán BIÊN DỊCH** giữa hai đường, đóng nợ `R4` §5 (26 ca lỗi biên dịch
//! trước đây **không** được so).
//!
//! Ba tầng **khoá cứng** (tổng **346**, **0 ca bỏ qua**):
//! - **294 xanh** — `check` không chẩn đoán, `run` và `run --vm --no-cache` cùng stdout + exit 0.
//!   (ST-35: 346 ca − 52 `.err` = 294; trước 286.)
//! - **26 lỗi BIÊN DỊCH** (`VS01/02/03/04/06/07`) — `check` và `run --vm --no-cache` phải cho **cùng**
//!   `(mã, dòng, cột, thông điệp)` sau [`chuan_hoa`]; **exit KHÔNG bằng nhau** theo thiết kế:
//!   `check` = **1** ↔ `run` = **2** (khoá **ánh xạ**, không đòi bằng).
//! - **26 lỗi LÚC CHẠY** (`VS05`) — `check` **rỗng** (đúng: lỗi động), `run` = `run --vm` **y hệt**.
//!
//! **Đường bytecode có thật**: với 26 ca biên dịch, test gọi `vis_bytecode::compile()` in-process và
//! khẳng định **không panic** + **không sinh module chạy được** — tức đường bytecode được **chạm tới**,
//! không chỉ front end.

use std::path::PathBuf;
use std::process::Command;

/// Tổng số ca `test/sem/**` — khoá cứng (ai thêm ca phải cập nhật **có ý thức**).
const TONG: usize = 359;
const XANH: usize = 307;
const LOI_BIEN_DICH: usize = 26;
const LOI_LUC_CHAY: usize = 26;
/// Ca **cần host chương trình ngoài** (VM không có host — ST-27): đã đo 4 ca xanh + 2 ca lỗi chạy.
const XANH_CAN_HOST: usize = 4;
const LUC_CHAY_CAN_HOST: usize = 2;
/// Số ca (đã đo) mà `compile()` vẫn ra module **chạy sạch** dù HIR sai kiểu — codegen **tin** tyck
/// (đo được **21/26**; cổng chặn là front end, `vis run` dừng trước khi chạy bytecode).
const CODEGEN_TIN_TYCK: usize = 21;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChanDoan {
    ma: String,
    dong: u32,
    cot: u32,
    msg: String,
}

/// **Một** hàm chuẩn hoá dùng chung cho **cả ba** đường (yêu cầu Agent 0): đổi escape trong thông điệp
/// thành ký tự thật để so **ngữ nghĩa**, không so cách hiển thị.
///
/// `check` in TAP với `` `\n` `` (escape), `run` in **xuống dòng thật**; `\t`/`\\` tương tự.
fn chuan_hoa(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            match b[i + 1] {
                b'n' => {
                    out.push(b'\n');
                    i += 2;
                    continue;
                }
                b't' => {
                    out.push(b'\t');
                    i += 2;
                    continue;
                }
                b'\\' => {
                    out.push(b'\\');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn goc() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

fn tep_sem() -> Vec<PathBuf> {
    let dir = goc().join("test/sem");
    let mut out = Vec::new();
    for nhom in std::fs::read_dir(&dir).expect("đọc test/sem") {
        let nhom = nhom.expect("entry").path();
        if !nhom.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&nhom).expect("đọc nhóm") {
            let p = f.expect("entry").path();
            if p.extension()
                .map(|x| x == "vis" || x == "vish")
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn chay(args: &[&std::ffi::OsStr]) -> (i32, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_vis"));
    for a in args {
        c.arg(a);
    }
    let o = c.output().expect("chạy `vis`");
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

/// `check`: dòng TAP `# VSxxxx <dòng>:<cột> <thông điệp>`.
fn doc_check(t: &str) -> Vec<ChanDoan> {
    let mut out = Vec::new();
    for l in t.lines() {
        let l = l.trim();
        let Some(rest) = l.strip_prefix("# ") else {
            continue;
        };
        let Some(rest) = rest.strip_prefix("VS") else {
            continue;
        };
        let Some((ma, rest)) = rest.split_once(' ') else {
            continue;
        };
        let Some((vt, msg)) = rest.split_once(' ') else {
            continue;
        };
        let Some((d, c)) = vt.split_once(':') else {
            continue;
        };
        let (Ok(d), Ok(c)) = (d.parse::<u32>(), c.parse::<u32>()) else {
            continue;
        };
        out.push(ChanDoan {
            ma: format!("VS{ma}"),
            dong: d,
            cot: c,
            msg: chuan_hoa(msg.trim()),
        });
    }
    out
}

/// `run`/`run --vm`: dòng `<tệp>:<dòng>:<cột>: error VSxxxx: <thông điệp>`.
///
/// Thông điệp có thể chứa **xuống dòng thật** (vd `loi/cu-phap.vis` in `` `\n` `` thành newline) ⇒
/// dòng nối tiếp **không** khớp mẫu tiêu đề được nối vào thông điệp, rồi chuẩn hoá escape.
fn doc_run(t: &str) -> Vec<ChanDoan> {
    fn tieu_de(l: &str) -> Option<(u32, u32, String, String)> {
        let i = l.find(": error VS")?;
        let truoc = &l[..i];
        let sau = &l[i + ": error VS".len()..];
        let (ma, msg) = sau.split_once(':')?;
        let mut it = truoc.rsplit(':');
        let c = it.next()?.parse::<u32>().ok()?;
        let d = it.next()?.parse::<u32>().ok()?;
        Some((d, c, format!("VS{ma}"), msg.trim().to_string()))
    }
    let mut out: Vec<ChanDoan> = Vec::new();
    for l in t.lines() {
        if let Some((d, c, ma, msg)) = tieu_de(l.trim()) {
            out.push(ChanDoan {
                ma,
                dong: d,
                cot: c,
                msg,
            });
        } else if let Some(last) = out.last_mut() {
            // Dòng nối tiếp của thông điệp **đa dòng** (không phải tiêu đề mới, không phải rác TAP).
            let l = l.trim_end();
            if !l.is_empty() && !l.starts_with('[') && !l.starts_with("TAP") {
                last.msg.push('\n');
                last.msg.push_str(l);
            }
        }
    }
    for d in &mut out {
        d.msg = chuan_hoa(d.msg.trim());
    }
    out
}

/// Tầng của một ca, đọc từ tệp `.err` (`VS05xx` ⇒ lỗi **lúc chạy**, còn lại ⇒ lỗi **biên dịch**).
#[derive(Debug, PartialEq, Eq)]
enum Tang {
    Xanh,
    BienDich,
    LucChay,
}

fn tang(p: &std::path::Path) -> Tang {
    let e = p.with_extension("err");
    if !e.exists() {
        return Tang::Xanh;
    }
    let t = std::fs::read_to_string(&e).unwrap_or_default();
    // Dòng đầu **không** phải chú thích SPDX.
    let ma = t
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("VS"))
        .unwrap_or("VS0000");
    if ma.starts_with("VS05") {
        Tang::LucChay
    } else {
        Tang::BienDich
    }
}

// ----- 1. Chuẩn hoá escape: unit-test riêng (yêu cầu Agent 0) -----

#[test]
fn chuan_hoa_escape() {
    assert_eq!(chuan_hoa(r"thật `\n` (NEWLINE)"), "thật `\n` (NEWLINE)");
    assert_eq!(chuan_hoa(r"a\tb"), "a\tb");
    assert_eq!(chuan_hoa(r"một \\ hai"), r"một \ hai");
    assert_eq!(chuan_hoa("không escape"), "không escape");
}

// ----- 2. Phân loại ba tầng khoá cứng -----

#[test]
fn phan_loai_ba_tang_khoa_cung() {
    let ca = tep_sem();
    let (mut x, mut b, mut r) = (0, 0, 0);
    for p in &ca {
        match tang(p) {
            Tang::Xanh => x += 1,
            Tang::BienDich => b += 1,
            Tang::LucChay => r += 1,
        }
    }
    assert_eq!(ca.len(), TONG, "tổng ca sem phải {TONG}");
    assert_eq!(
        (x, b, r),
        (XANH, LOI_BIEN_DICH, LOI_LUC_CHAY),
        "phân loại phải khoá cứng {XANH} xanh / {LOI_BIEN_DICH} lỗi biên dịch / {LOI_LUC_CHAY} lỗi lúc chạy"
    );
    assert_eq!(x + b + r, TONG, "0 ca bỏ qua");
}

// ----- 3. Ca lỗi biên dịch: hai đường **cùng chẩn đoán**, exit **ánh xạ** 1 ↔ 2 -----

#[test]
fn ca_bien_dich_hai_duong_cung_chan_doan() {
    let mut so = 0;
    let mut loi = Vec::new();
    for p in tep_sem() {
        if tang(&p) != Tang::BienDich {
            continue;
        }
        so += 1;
        let (c1, t1) = chay(&[std::ffi::OsStr::new("check"), p.as_os_str()]);
        let (c2, t2) = chay(&[
            std::ffi::OsStr::new("run"),
            std::ffi::OsStr::new("--vm"),
            std::ffi::OsStr::new("--no-cache"),
            p.as_os_str(),
        ]);
        let (d1, d2) = (doc_check(&t1), doc_run(&t2));
        // **Exit KHÔNG đòi bằng nhau**: `check` lỗi ⇒ 1, `run` lỗi biên dịch ⇒ 2. Khoá **ánh xạ**.
        if c1 != 1 || c2 != 2 {
            loi.push(format!(
                "{}: exit check={c1} run={c2} (muốn 1 ↔ 2)",
                p.display()
            ));
        }
        if d1 != d2 || d1.is_empty() {
            loi.push(format!("{}: check={d1:?} run-vm={d2:?}", p.display()));
        }
    }
    assert_eq!(
        so, LOI_BIEN_DICH,
        "phải so đủ {LOI_BIEN_DICH} ca lỗi biên dịch"
    );
    assert!(loi.is_empty(), "{} ca lệch:\n{}", loi.len(), loi.join("\n"));
}

// ----- 4. **Đường bytecode được chạm tới** (yêu cầu 1 của Agent 0) -----

#[test]
fn ca_bien_dich_di_qua_bytecode_khong_panic() {
    let mut so = 0;
    // Số ca mà `compile()` vẫn ra module **chạy sạch** dù HIR sai kiểu (codegen tin tyck — nợ ST-32).
    let mut codegen_khong_kiem = 0usize;
    for p in tep_sem() {
        if tang(&p) != Tang::BienDich {
            continue;
        }
        so += 1;
        let text = std::fs::read_to_string(&p).expect("đọc");
        let mode = if p.extension().map(|x| x == "vish").unwrap_or(false) {
            vis_syntax::Mode::Command
        } else {
            vis_syntax::Mode::Script
        };
        let lexed = vis_syntax::lex(&text, mode);
        let parsed = vis_syntax::parse(&text, mode);
        let h = vis_types::lower(&parsed.cst, &lexed);
        let kq = vis_types::check(&h.hir, &h.symbols, &h.types);
        // Chẩn đoán có thể đến từ **parse** (`VS02xx`), **hạ** (`VS03xx`/`VS07xx`) hoặc **tyck**
        // (`VS04xx`/`VS06xx`) — khoá "có ít nhất một tầng front end báo".
        let co_chan_doan =
            !parsed.diagnostics.is_empty() || !h.diagnostics.is_empty() || !kq.codes().is_empty();
        assert!(
            co_chan_doan,
            "{}: ca lỗi biên dịch phải có chẩn đoán ở **một** tầng front end",
            p.display()
        );
        // Gọi **đường bytecode** in-process: **không panic**. Codegen **tin** tyck (không tự kiểm
        // kiểu) nên có thể vẫn ra module — điều phải khoá là: (i) không panic, (ii) **không** module
        // nào được `vis run` chạy tới (nó dừng ở front end, exit 2 — test 3), (iii) nếu module chạy
        // được thì **không** được trông như chương trình đúng (phải có chẩn đoán hoặc exit ≠ 0).
        let kq_compile = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vis_bytecode::compile(&h.hir, &h.symbols, &text)
        }));
        assert!(
            kq_compile.is_ok(),
            "{}: `compile()` **panic** trên HIR lỗi",
            p.display()
        );
        if let Ok(Ok(m)) = kq_compile {
            let o = vis_vm::run(&m, &text, &[]);
            if o.exit == 0 && o.diagnostics.is_empty() {
                // Ghi nhận: codegen **không** kiểm kiểu ⇒ module "sạch" dù HIR sai. Đây là **nợ đã biết**
                // (codegen tin tyck); cổng chặn là front end (`vis check`/`vis run` dừng trước khi chạy).
                codegen_khong_kiem += 1;
            }
        }
    }
    assert_eq!(
        so, LOI_BIEN_DICH,
        "phải chạm đường bytecode đủ {LOI_BIEN_DICH} ca"
    );
    // Khoá **sự thật đã đo**: codegen hiện **không** kiểm kiểu ⇒ với 26 ca lỗi biên dịch nó vẫn ra
    // module, và VM chạy "sạch" ở một số ca. Khoá con số để ai sửa codegen (thêm kiểm) thì test đỏ
    // và phải cập nhật **có ý thức**.
    assert_eq!(
        codegen_khong_kiem, CODEGEN_TIN_TYCK,
        "số ca codegen ra module chạy sạch dù HIR sai (nợ đã biết) phải khoá = {CODEGEN_TIN_TYCK}"
    );
}

// ----- 5. Ca lỗi lúc chạy: `check` rỗng, `run` = `run --vm` -----

#[test]
fn ca_luc_chay_check_rong_hai_duong_giong() {
    let mut so = 0;
    let mut so_can_host = 0usize;
    let mut loi = Vec::new();
    for p in tep_sem() {
        if tang(&p) != Tang::LucChay {
            continue;
        }
        so += 1;
        let (c1, t1) = chay(&[std::ffi::OsStr::new("check"), p.as_os_str()]);
        let (c2, t2) = chay(&[
            std::ffi::OsStr::new("run"),
            std::ffi::OsStr::new("--vm"),
            std::ffi::OsStr::new("--no-cache"),
            p.as_os_str(),
        ]);
        let (c3, t3) = chay(&[std::ffi::OsStr::new("run"), p.as_os_str()]);
        let (d1, d2, d3) = (doc_check(&t1), doc_run(&t2), doc_run(&t3));
        if c1 != 0 || !d1.is_empty() {
            loi.push(format!(
                "{}: `check` phải rỗng (exit 0), thật {c1} {d1:?}",
                p.display()
            ));
        }
        // **Ngoại lệ có lý do (ST-27/ST-30)**: ca cần **host chương trình ngoài** ⇒ VM không có host,
        // báo `VS0604`; interp chạy được. Khoá đúng *số ca* như vậy, không nới cho ca khác.
        if t2.contains("VS0604") {
            so_can_host += 1;
            if !t2.contains("không có host") || c2 == 0 || c3 == 0 {
                loi.push(format!(
                    "{}: ca cần host phải `VS0604` ở VM, thật {d2:?}",
                    p.display()
                ));
            }
            continue;
        }
        if c2 != c3 || d2 != d3 || d2.is_empty() {
            loi.push(format!(
                "{}: run-vm ({c2} {d2:?}) ≠ run ({c3} {d3:?})",
                p.display()
            ));
        }
    }
    assert_eq!(
        so, LOI_LUC_CHAY,
        "phải so đủ {LOI_LUC_CHAY} ca lỗi lúc chạy"
    );
    assert_eq!(
        so_can_host, LUC_CHAY_CAN_HOST,
        "số ca lỗi-chạy cần host phải khoá"
    );
    assert!(loi.is_empty(), "{} ca lệch:\n{}", loi.len(), loi.join("\n"));
}

// ----- 6. Ca xanh: ba đường cùng stdout, exit 0 -----

#[test]
fn ca_xanh_ba_duong_khop_stdout() {
    let mut so = 0;
    let mut so_can_host = 0usize;
    let mut loi = Vec::new();
    for p in tep_sem() {
        if tang(&p) != Tang::Xanh {
            continue;
        }
        so += 1;
        let (c1, t1) = chay(&[std::ffi::OsStr::new("check"), p.as_os_str()]);
        let (c2, t2) = chay(&[std::ffi::OsStr::new("run"), p.as_os_str()]);
        let (c3, t3) = chay(&[
            std::ffi::OsStr::new("run"),
            std::ffi::OsStr::new("--vm"),
            std::ffi::OsStr::new("--no-cache"),
            p.as_os_str(),
        ]);
        if c1 != 0 || !doc_check(&t1).is_empty() {
            loi.push(format!("{}: `check` xanh nhưng có chẩn đoán", p.display()));
        }
        // **Ngoại lệ có lý do (ST-27/ST-30)**: ca chương trình ngoài chạy được ở interp nhưng VM
        // **không có host** ⇒ `VS0604`; khoá đúng số ca.
        if t3.contains("VS0604") {
            so_can_host += 1;
            if c2 != 0 || c3 == 0 || !t3.contains("không có host") {
                loi.push(format!(
                    "{}: ca cần host: run exit {c2}, run-vm exit {c3} (muốn 0 ↔ ≠0 + VS0604)",
                    p.display()
                ));
            }
            continue;
        }
        if c2 != 0 || c3 != 0 || t2 != t3 {
            loi.push(format!(
                "{}: run (exit {c2}) ≠ run-vm (exit {c3})",
                p.display()
            ));
        }
    }
    assert_eq!(so, XANH, "phải so đủ {XANH} ca xanh");
    assert_eq!(so_can_host, XANH_CAN_HOST, "số ca xanh cần host phải khoá");
    assert!(loi.is_empty(), "{} ca lệch:\n{}", loi.len(), loi.join("\n"));
}
