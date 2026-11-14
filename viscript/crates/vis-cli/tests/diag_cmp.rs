// SPDX-License-Identifier: MIT OR Apache-2.0
//! **ST-32** — harness so **chẩn đoán BIÊN DỊCH** giữa hai đường, đóng nợ `R4` §5 (26 ca lỗi biên dịch
//! trước đây **không** được so).
//!
//! Ba tầng **khoá cứng** (tổng **441**, **0 ca bỏ qua** — khớp bộ sem: ST-61 thêm nhóm `kenh/`):
//! - **361 xanh** — `check` không chẩn đoán, `run` và `run --vm --no-cache` cùng stdout + exit 0.
//! - **46 lỗi BIÊN DỊCH** (`VS01/02/03/04/05/06/07` — `VS0506` lỗi **động** nên vẫn ở tầng dưới) —
//!   `check` và `run --vm --no-cache` phải cho **cùng** `(mã, dòng, cột, thông điệp)` sau
//!   [`chuan_hoa`]; **exit KHÔNG bằng nhau** theo thiết kế: `check` = **1** ↔ `run` = **2**.
//! - **34 lỗi LÚC CHẠY** (`VS05`) — `check` **rỗng** (đúng: lỗi động), `run` = `run --vm` **y hệt**.
//!
//! **Đường bytecode có thật**: với 26 ca biên dịch, test gọi `vis_bytecode::compile()` in-process và
//! khẳng định **không panic** + **không sinh module chạy được** — tức đường bytecode được **chạm tới**,
//! không chỉ front end.

use std::path::PathBuf;
use std::process::Command;

/// Tổng số ca `test/sem/**` — khoá cứng (ai thêm ca phải cập nhật **có ý thức**).
/// **SỬA KÈM LÝ DO (ST-56)**: 370 → **376** — thêm 6 ca (`ham/ref-*`, `ban-ghi/self-amp*`,
/// `generic/hop-long-*`): 3 xanh + 2 lỗi biên dịch + 1 lỗi lúc chạy. (Ca `str` của
/// `std.list.sort_by` ở `test/std/` — harness `test/sem` chạy **in-process**, không nạp `use std.*`.)
/// **SỬA KÈM LÝ DO (ST-56b)**: 376 → **382** — +4 ca xanh + 2 ca lỗi **biên dịch**
/// (`ban-ghi/self-gan-truong` VS0412, `list/let-chi-so-gan` VS0412).
/// **SỬA KÈM LÝ DO (ST-57)**: 382 → **388** — +4 ca xanh (`trait/sieu-trait-du`, `derive-show`,
/// `derive-eq`, `derive-nguoi-dung-thang`) + 2 ca lỗi **biên dịch** (`sieu-trait-thieu`,
/// `derive-eq-float` — `VS0403` từ **hạ**, không phải tyck).
/// **SỬA KÈM LÝ DO (ST-59)**: 388 → **394** — +3 ca xanh (`vish/let-suy-kieu`,
/// `vish/stage-dau-bien`, `vish/than-khai-lenh`) + 2 ca lỗi **biên dịch** (`vish/let-ngoai-vung-khai`
/// VS0201, `vish/than-hai-pipeline` VS0201) + 1 ca lỗi **lúc chạy** (`vish/stage-dau-chua-bind` VS0501).
/// **SỬA KÈM LÝ DO (ST-60)**: 394 → **416** — +15 ca xanh `async/` + 5 ca lỗi **biên dịch**
/// (VS0412 ×4: `await` không phải `Task`, `async fn` làm giá trị, `spawn` `--tên`, `spawn` không
/// async; VS0201: `async` trong `impl`) + 2 ca lỗi **lúc chạy** (`VS0506` ×2: thân tác vụ lỗi ở
/// `await` và ở join cuối).
// **SỬA KÈM LÝ DO (ST-72)**: 441 → **447** (+6 ca `cap/`: 3 xanh, 3 lỗi biên dịch).
const TONG: usize = 447;
const XANH: usize = 364;
const LOI_BIEN_DICH: usize = 49;
const LOI_LUC_CHAY: usize = 34;
/// **SỬA KÈM LÝ DO (ST-89)**: VM **đã có host `Ngoai`** ⇒ **0** ca xanh cần host bị `VS0604`.
const XANH_CAN_HOST: usize = 0;
/// **SỬA KÈM LÝ DO (ST-89)**: 3 → **0** — VM có host `Ngoai`, cả 3 ca lỗi-chạy cần host nay so
/// **bình thường** (cùng chẩn đoán), không còn `VS0604`.
const LUC_CHAY_CAN_HOST: usize = 0;
/// **SỬA KÈM LÝ DO (ST-54, Agent 0 duyệt)**: trước là **21** — số ca `compile()` vẫn ra module
/// chạy "sạch" dù HIR sai (codegen **tin** tyck). Nay `compile_kiem` nhận **chẩn đoán front end**
/// và **từ chối** ⇒ **0** ca lọt. HIR không mang dấu `Type::Error` (đo 370 ca: 0 nút) nên đây là
/// cửa duy nhất đúng.
const CODEGEN_TIN_TYCK: usize = 0;

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
    // ST-54: `compile_kiem` phải **từ chối** đủ 26 ca; số ca còn ra module "sạch" = 0.
    let mut so_bi_tu_choi = 0usize;
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
        // ST-54: gom **đủ 3 tầng** chẩn đoán rồi đưa vào **cửa codegen** (`compile_kiem`).
        let mut chan: Vec<vis_syntax::Diagnostic> = Vec::new();
        chan.extend(parsed.diagnostics.iter().cloned());
        chan.extend(h.diagnostics.iter().cloned());
        chan.extend(kq.diags.iter().cloned());
        let dau = chan
            .iter()
            .find(|d| d.code.severity == vis_diag::Severity::Error)
            .expect("ca lỗi biên dịch phải có chẩn đoán Error");
        // Gọi **đường bytecode** in-process: **không panic**, và phải **từ chối** với **đủ**
        // `mã + dòng:cột + thông điệp` của chẩn đoán đầu tiên.
        let kq_compile = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vis_bytecode::compile_kiem(&h.hir, &h.symbols, &text, &chan)
        }));
        assert!(
            kq_compile.is_ok(),
            "{}: `compile_kiem()` **panic** trên HIR lỗi",
            p.display()
        );
        match kq_compile {
            Ok(Err(vis_bytecode::CodegenError::ChanDoan {
                short,
                dong,
                cot,
                msg,
            })) => {
                assert_eq!(short, dau.code.short, "{}: mã chẩn đoán", p.display());
                assert_eq!(dong, dau.line, "{}: dòng chẩn đoán", p.display());
                assert_eq!(cot, dau.col, "{}: cột chẩn đoán", p.display());
                assert_eq!(msg, dau.message, "{}: thông điệp chẩn đoán", p.display());
                so_bi_tu_choi += 1;
            }
            Ok(Ok(m)) => {
                let o = vis_vm::run(&m, &text, &[]);
                if o.exit == 0 && o.diagnostics.is_empty() {
                    codegen_khong_kiem += 1;
                }
            }
            Ok(Err(khac)) => panic!("{}: lỗi codegen lạ: {khac}", p.display()),
            Err(_) => panic!("{}: `compile_kiem()` panic", p.display()),
        }
    }
    assert_eq!(
        so, LOI_BIEN_DICH,
        "phải chạm đường bytecode đủ {LOI_BIEN_DICH} ca"
    );
    // ST-54: **mọi** ca lỗi biên dịch bị cửa codegen từ chối; **không** ca nào ra module "sạch".
    assert_eq!(
        so_bi_tu_choi, LOI_BIEN_DICH,
        "compile_kiem phải từ chối đủ {LOI_BIEN_DICH} ca"
    );
    assert_eq!(
        codegen_khong_kiem, CODEGEN_TIN_TYCK,
        "số ca codegen ra module chạy sạch dù HIR sai phải khoá = {CODEGEN_TIN_TYCK}"
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
