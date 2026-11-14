// SPDX-License-Identifier: MIT OR Apache-2.0
//! `vis` — lệnh dòng ViScript. Vòng 1 (ST-02b) mới có `explain`.

use std::process::ExitCode;

mod cache;
mod fsmock;
mod lsp;
mod modules;
mod net;
mod ngoai;
mod pkg;
mod quyen;
mod vistest;

use vis_diag::{all_codes, lookup_str, severity_label, Lang};
use vis_syntax::Mode;

/// **ST-63 (VD-105)**: host mạng của CLI (TCP thật, chỉ loopback) — tên cục bộ cho rõ điểm nối.
use net::Net as HostNet;

fn usage() -> ExitCode {
    eprintln!(
        "dùng: vis explain [--lang vi|en|ja] <VSxxxx> | vis explain --list [--lang vi|en|ja] | vis run [--tap] [--strict-caps] <tệp.vis> [-- args…] | vis check <tệp…> | vis fmt <tệp> | vis fmt --write <tệp…> | vis fmt --check <tệp…> | vis fmt --indent-report <tệp…> | vis lsp --stdio | vis pkg add|install|list"
    );
    ExitCode::from(2)
}

/// **ST-78/ST-83**: trợ giúp riêng của `vis explain` — đúng số khối/số mã (đếm động) và **theo
/// `--lang`** (vi/en/ja); không viết cứng số.
fn usage_explain(lang: Lang) -> ExitCode {
    let khoi = vis_diag::BLOCKS_RESERVED.len();
    let ma = vis_diag::all_codes().count();
    match lang {
        Lang::Vi => {
            eprintln!(
                "dùng: vis explain <VSxxxx> [--lang vi|en|ja] | vis explain --list [--lang vi|en|ja]"
            );
            eprintln!(
                "sổ mã: {khoi} khối, {ma} mã — `vis explain --list` in toàn bộ (mã + câu ngắn)"
            );
        }
        Lang::En => {
            eprintln!(
                "usage: vis explain <VSxxxx> [--lang vi|en|ja] | vis explain --list [--lang vi|en|ja]"
            );
            eprintln!(
                "code book: {khoi} blocks, {ma} codes — `vis explain --list` prints all (code + short text)"
            );
        }
        Lang::Ja => {
            eprintln!(
                "使い方: vis explain <VSxxxx> [--lang vi|en|ja] | vis explain --list [--lang vi|en|ja]"
            );
            eprintln!(
                "コード帳: {khoi} ブロック、{ma} コード — `vis explain --list` で全件表示（コード + 短い説明）"
            );
        }
    }
    ExitCode::from(2)
}

/// **FsHost** của CLI (ST-42): `--fs-mock` ⇒ mock tất định, ngược lại ⇒ hệ tệp thật.
enum HostFs {
    That(ngoai::Runner),
    Mock(fsmock::FsMock),
}

impl vis_interp::builtin::FsHost for HostFs {
    fn read(&mut self, p: &str) -> Result<String, String> {
        match self {
            HostFs::That(r) => r.read(p),
            HostFs::Mock(m) => m.read(p),
        }
    }
    fn write(&mut self, p: &str, s: &str) -> Result<(), String> {
        match self {
            HostFs::That(r) => r.write(p, s),
            HostFs::Mock(m) => m.write(p, s),
        }
    }
    fn exists(&mut self, p: &str) -> bool {
        match self {
            HostFs::That(r) => r.exists(p),
            HostFs::Mock(m) => m.exists(p),
        }
    }
    fn list(&mut self, p: &str) -> Result<Vec<String>, String> {
        match self {
            HostFs::That(r) => r.list(p),
            HostFs::Mock(m) => m.list(p),
        }
    }
    fn list_entries(&mut self, p: &str) -> Result<Vec<vis_interp::builtin::MucFs>, String> {
        match self {
            HostFs::That(r) => r.list_entries(p),
            HostFs::Mock(m) => m.list_entries(p),
        }
    }
}

impl HostFs {
    fn tao(giai_thich: bool, mock: Option<&fsmock::FsMock>) -> Self {
        match mock {
            Some(m) => HostFs::Mock(m.clone()),
            None => HostFs::That(ngoai::Runner {
                giai_thich,
                ..ngoai::Runner::default()
            }),
        }
    }
}

/// Ghi ra stdout **một lần**; `BrokenPipe` (vd `vis fmt … | head`) ⇒ im lặng thoát 0, không panic.
/// Lỗi I/O khác ⇒ báo stderr và trả `false` (caller thoát 2).
fn xuat_stdout(s: &str) -> bool {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    match w.write_all(s.as_bytes()).and_then(|()| w.flush()) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(0),
        Err(e) => {
            eprintln!("# không ghi được stdout: {e}");
            false
        }
    }
}

/// Escape ký tự điều khiển để chẩn đoán nằm gọn trên **một** dòng (TAP/stderr).
fn dong_chan_doan(code: &str, line: u32, col: u32, message: &str) -> String {
    format!(
        "{code} {line}:{col} {}",
        message
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

fn usage_check() -> ExitCode {
    eprintln!("dùng: vis check [--syntax] [--no-extern] <tệp…>");
    ExitCode::from(2)
}

/// Kết quả kiểm **một** tệp: `Ok(())` sạch, `Err(<mã> <dòng>:<cột> <thông điệp>)` hoặc
/// `Err(không đọc được: …)`. Chỉ trả **một** chẩn đoán (lọc tầng như [`chon_chan_doan`]).
/// Chẩn đoán của `vis check` cho một tệp (ST-25: có thể đến từ tệp **phụ**).
struct ChanDoanCheck {
    ma: &'static str,
    line: u32,
    col: u32,
    msg: String,
    /// Tệp phụ (rỗng nếu chẩn đoán ở tệp chính).
    phu: String,
}

#[allow(dead_code)] // giữ API cũ (mặc định kiểm kiểu) cho người đọc/tests.
fn check_one_co(path: &str, cho_ngoai: bool) -> Result<(), ChanDoanCheck> {
    check_mode(path, cho_ngoai, true)
}

/// `kiem_kieu = false` ⇒ `--syntax`: chỉ cú pháp/hạ (bỏ `tyck`).
fn check_mode(path: &str, cho_ngoai: bool, kiem_kieu: bool) -> Result<(), ChanDoanCheck> {
    if let Err(e) = std::fs::read_to_string(path) {
        return Err(ChanDoanCheck {
            ma: "VS0201",
            line: 1,
            col: 1,
            msg: format!("không đọc được: {e}"),
            phu: String::new(),
        });
    }
    let loaded = modules::load_full(path, cho_ngoai, kiem_kieu).map_err(|e| ChanDoanCheck {
        ma: "VS0201",
        line: 1,
        col: 1,
        msg: e.to_string(),
        phu: String::new(),
    })?;
    let la_vish = path.ends_with(".vish");
    let Some((rel, d)) = loaded.diags.first() else {
        return Ok(());
    };
    let ma = d.code.short;
    let chinh = path.rsplit('/').next().unwrap_or(path);
    let phu = if rel == chinh {
        String::new()
    } else {
        rel.clone()
    };
    // **SỬA KÈM LÝ DO (ST-28)**: `vis check` **mặc định** báo **đúng chẩn đoán đầu** như `vis run`
    // (bỏ bộ lọc `01xx/02xx/07xx` của ST-25 — nó làm hai đường **lệch kết luận**: check xanh trong
    // khi run `VS0412`/`VS0301`). Bộ lọc chỉ còn ở chế độ **`--syntax`** (chỉ parse + cấu trúc
    // module), dùng cho cổng "khối mã trong tài liệu parse được" (QD §7.4).
    if !kiem_kieu {
        let giu = la_vish
            || !phu.is_empty()
            || ma.starts_with("VS01")
            || ma.starts_with("VS02")
            || ma.starts_with("VS06")
            || ma.starts_with("VS07");
        if !giu {
            return Ok(());
        }
    }
    Err(ChanDoanCheck {
        ma,
        line: d.line,
        col: d.col,
        msg: d.message.clone(),
        phu,
    })
}

/// **ST-85**: đếm dòng lệch thụt lề của `.vish` theo chuẩn **4 × độ sâu khối**; `None` nếu tệp
/// không parse được (tab cũng là lỗi lexer `VS0107`).
fn dem_lech_thut_le(text: &str) -> Option<usize> {
    use vis_syntax::{Mode, Node, NodeKind};
    let out = vis_syntax::parse(text, Mode::Script);
    if out.diagnostics.is_empty() {
        fn walk(n: &Node, depth: u32, out: &mut Vec<(usize, usize, u32)>) {
            for c in n.children() {
                let d = if matches!(c.kind, NodeKind::Suite | NodeKind::CmdBlock) {
                    out.push((c.span.start, c.span.end, depth + 1));
                    depth + 1
                } else {
                    depth
                };
                walk(c, d, out);
            }
        }
        let mut suites: Vec<(usize, usize, u32)> = Vec::new();
        walk(&out.cst.root, 0, &mut suites);
        let mut lech = 0;
        let mut off = 0;
        for line in text.split_inclusive('\n') {
            let noi_dung = line.trim_end_matches(['\n', '\r']);
            if !noi_dung.trim().is_empty() {
                let indent = noi_dung.len() - noi_dung.trim_start_matches([' ', '\t']).len();
                let mut depth = 0;
                for (s, e, d) in &suites {
                    if *s <= off && off < *e {
                        depth = depth.max(*d);
                    }
                }
                if indent != (depth * 4) as usize {
                    lech += 1;
                }
            }
            off += line.len();
        }
        return Some(lech);
    }
    // Thân `cmd` chỉ là **lệnh** (script parse không qua) ⇒ ước lượng khối theo dòng: dòng cột 1
    // mở/đóng khối `cmd`/`impl`/`type`; dòng thụt trong khối chuẩn **4**. Đây là **đếm** cho báo
    // cáo, không dùng để sửa.
    let mut trong = false;
    let mut lech = 0;
    let mut co_noi_dung = false;
    for line in text.lines() {
        let noi_dung = line.trim_end();
        if noi_dung.trim().is_empty() {
            continue;
        }
        co_noi_dung = true;
        let indent = noi_dung.len() - noi_dung.trim_start_matches([' ', '\t']).len();
        if indent == 0 {
            let t = noi_dung.trim_start();
            trong = t.ends_with(':')
                && (t.starts_with("cmd ") || t.starts_with("impl ") || t.starts_with("type "));
        }
        let mong = if trong && indent != 0 { 4 } else { 0 };
        if indent != mong {
            lech += 1;
        }
    }
    if co_noi_dung {
        Some(lech)
    } else {
        Some(0)
    }
}

fn usage_fmt() -> ExitCode {
    eprintln!(
        "dùng: vis fmt <tệp|thư-mục> | vis fmt --write <tệp|thư-mục…> | vis fmt --check <tệp|thư-mục…> | vis fmt --indent-report <tệp|thư-mục…> | vis lsp --stdio | vis pkg add|install|list"
    );
    ExitCode::from(2)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FmtMode {
    /// **ST-85**: chỉ **đếm** dòng lệch thụt lề của `.vish` (không sửa, không đỏ).
    IndentReport,
    Stdout,
    Write,
    Check,
}

/// `vis fmt` — chuẩn hoá **thụt lề** (ST-07, VD-39). Thoát 0 (xong / đã đúng), 1 (`--check` có tệp
/// cần sửa), 2 (dùng sai, tệp lỗi cú pháp, `.vish`).
/// **ST-77**: gom `.vis`/`.vish` dưới `goc` theo thứ tự byte (sắp sau khi gom).
fn gom_nguon(goc: &std::path::Path, ra: &mut Vec<std::path::PathBuf>) {
    let Ok(ds) = std::fs::read_dir(goc) else {
        return;
    };
    for e in ds.flatten() {
        let p = e.path();
        if p.is_dir() {
            gom_nguon(&p, ra);
        } else if p
            .extension()
            .map(|x| x == "vis" || x == "vish")
            .unwrap_or(false)
        {
            ra.push(p);
        }
    }
}

fn run_fmt(args: &[String]) -> ExitCode {
    let (mode, files): (FmtMode, &[String]) = match args.first().map(String::as_str) {
        None => return usage_fmt(),
        Some("--write") => (FmtMode::Write, &args[1..]),
        Some("--check") => (FmtMode::Check, &args[1..]),
        Some("--indent-report") => (FmtMode::IndentReport, &args[1..]),
        Some(f) if f.starts_with('-') => {
            eprintln!("vis fmt: không hỗ trợ cờ `{f}`");
            return usage_fmt();
        }
        Some(_) => {
            if args.len() != 1 {
                return usage_fmt();
            }
            (FmtMode::Stdout, args)
        }
    };
    if files.is_empty() {
        return usage_fmt();
    }
    // **ST-77 (VD-108)**: nhận **thư mục** (đệ quy `.vis`/`.vish`) để `vis fmt --check test/`
    // kiểm được toàn bộ fixture; tệp lẻ giữ nguyên hành vi cũ.
    let mut ds: Vec<std::path::PathBuf> = Vec::new();
    for f in files {
        let p = std::path::Path::new(f);
        if p.is_dir() {
            gom_nguon(p, &mut ds);
        } else {
            ds.push(p.to_path_buf());
        }
    }
    // **ST-77**: bỏ hai loại **input cố ý** khỏi kiểm `fmt` (allowlist nhỏ nhất, có lý do):
    // (1) `test/fmt/**` = fixture của chính formatter (sai thụt lề/tab/crlf — đầu vào để sửa);
    // (2) `test/check/**` + `test/doc-examples/**` = fixture **cố ý sai** (ca âm của `check` và
    //     khối tài liệu sai theo thiết kế) — fmt từ chối đúng, không phải lỗi formatter;
    // (3) tệp có sibling `.err` = ca **âm** cố ý sai cú pháp.
    ds.retain(|p| {
        let comps: Vec<_> = p.components().map(|c| c.as_os_str()).collect();
        let la_fixture = comps.windows(3).any(|w| {
            w[0] == "test" && matches!(w[1].to_str(), Some("fmt" | "check" | "doc-examples"))
        });
        !la_fixture && !p.with_extension("err").exists()
    });
    ds.sort();
    if mode == FmtMode::IndentReport {
        let mut tong_dong = 0usize;
        let mut so_tep = 0usize;
        for path in &ds {
            if path.extension().map(|x| x != "vish").unwrap_or(true) {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(path) else {
                continue;
            };
            if let Some(n) = dem_lech_thut_le(&src) {
                if n > 0 {
                    so_tep += 1;
                    tong_dong += n;
                    println!("{}: {n} dòng lệch", path.display());
                }
            }
        }
        println!("indent-report: {so_tep} tệp, {tong_dong} dòng lệch (chuẩn 4 × độ sâu khối)");
        return ExitCode::SUCCESS;
    }
    let mut ra = String::new();
    let mut can_sua = false;
    let mut co_loi = false;
    for path in &ds {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("# không đọc được: {e}");
                co_loi = true;
                continue;
            }
        };
        // `.vish` = chế độ lệnh (CmdCST); `.vis` = script. Tệp khác (nếu người dùng chỉ định
        // thẳng) giữ đường script như trước.
        let che_do = if path.extension().map(|x| x == "vish").unwrap_or(false) {
            Mode::Command
        } else {
            Mode::Script
        };
        match vis_fmt::format_mode(&src, che_do) {
            Err(e) => {
                // (d) tệp lỗi: báo lỗi đầu, **không** ghi.
                eprintln!("# {}", dong_chan_doan(e.code, e.line, e.col, &e.message));
                co_loi = true;
            }
            Ok(moi) => match mode {
                // **ST-85**: thụt lề `.vish` **có** ngữ nghĩa ở cấp tệp (đo được) ⇒ **không** sửa;
                // `--check` chỉ **cảnh báo** (không đỏ), `--indent-report` đếm.
                FmtMode::IndentReport => {}
                FmtMode::Check if che_do == Mode::Command => {
                    if let Some(n) = dem_lech_thut_le(&src) {
                        if n > 0 {
                            eprintln!(
                                "# cảnh báo: {}: {n} dòng thụt lề khác chuẩn 4×độ sâu — KHÔNG tự sửa (thụt lề .vish có ngữ nghĩa; xem agent/HOST-BAO-DAM.md)",
                                path.display()
                            );
                        }
                    }
                    if moi != src {
                        can_sua = true;
                        ra.push_str(&format!("{}\n", path.display()));
                    }
                }
                FmtMode::Stdout => ra.push_str(&moi),
                FmtMode::Write => {
                    if moi == src {
                        ra.push_str(&format!("giữ nguyên {}\n", path.display()));
                    } else if let Err(e) = std::fs::write(path, &moi) {
                        eprintln!("# không ghi được {}: {e}", path.display());
                        co_loi = true;
                    } else {
                        can_sua = true;
                        ra.push_str(&format!("đã sửa {}\n", path.display()));
                    }
                }
                FmtMode::Check => {
                    if moi != src {
                        can_sua = true;
                        ra.push_str(&format!("{}\n", path.display()));
                    }
                }
            },
        }
    }
    if !xuat_stdout(&ra) {
        return ExitCode::from(2);
    }
    if co_loi {
        return ExitCode::from(2);
    }
    if mode == FmtMode::Check && can_sua {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// `vis check <tệp…>` — in TAP 13. Thoát 0 (sạch) / 1 (có tệp hỏng) / 2 (dùng sai, `.vish`).
fn run_check(args: &[String]) -> ExitCode {
    if args.is_empty() {
        return usage_check();
    }
    // ST-27: `--no-extern` — lệnh lạ báo `VS0601` (không cho chương trình ngoài).
    let cho_ngoai = !args.iter().any(|a| a == "--no-extern");
    // ST-28: `--syntax` — chỉ cú pháp/hạ (bỏ `tyck`), dùng cho cổng "khối mã trong tài liệu
    // **parse** được" (QD §7.4).
    let kiem_kieu = !args.iter().any(|a| a == "--syntax");
    let args: Vec<&String> = args
        .iter()
        .filter(|a| a.as_str() != "--no-extern" && a.as_str() != "--syntax")
        .collect();
    if let Some(bad) = args.iter().find(|a| a.starts_with('-')) {
        eprintln!("vis check: không hỗ trợ cờ `{bad}`");
        return usage_check();
    }
    let mut tap = String::new();
    tap.push_str("TAP version 13\n");
    tap.push_str(&format!("1..{}\n", args.len()));
    let mut failed = false;
    for (i, path) in args.iter().enumerate() {
        let n = i + 1;
        match check_mode(path, cho_ngoai, kiem_kieu) {
            Ok(()) => tap.push_str(&format!("ok {n} - {path}\n")),
            Err(d) => {
                failed = true;
                // ST-25: chẩn đoán ở tệp phụ ⇒ dòng `not ok` nêu `tệp:dòng:cột` (TAP đúng tệp).
                let them = if d.phu.is_empty() {
                    String::new()
                } else {
                    format!(" ({}:{}:{})", d.phu, d.line, d.col)
                };
                // Lỗi **đọc tệp** giữ nguyên định dạng cũ: `# không đọc được: …` (không mã).
                let dong = if d.msg.starts_with("không đọc được") {
                    d.msg.clone()
                } else {
                    dong_chan_doan(d.ma, d.line, d.col, &d.msg)
                };
                tap.push_str(&format!("not ok {n} - {path}{them}\n# {dong}\n"));
            }
        }
    }
    if !xuat_stdout(&tap) {
        return ExitCode::from(2);
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `vis run [--tap] <tệp.vis> [-- args…]` — parse → lower → check → interp (ST-14).
fn run_run(rest: &[String]) -> ExitCode {
    let mut tap = false;
    let mut dung_vm = false;
    let mut dump = false;
    let mut dung_cache = true;
    let mut dump_deps = false;
    // **ST-72 (VD-106 (2))**: in quyền khai báo của `main` rồi exit 0, **không** chạy.
    let mut dry = false;
    // **ST-73 (VD-107)**: probe Landlock (chỉ đọc) + mức cưỡng chế runtime; **không** chạy chương trình.
    let mut sandbox_check = false;
    // **ST-72b (VD-103)**: `--grant <tên>=<path>` thay root của cap đã khai (không thêm quyền mới).
    let mut grants: Vec<(String, String)> = Vec::new();
    let mut explain_perms = false;
    // **ST-81 (VD-108)**: mọi truy cập fs/net phải nằm trong `cap` đã khai (mặc định: không siết).
    let mut strict_caps = false;
    // `vis run --no-extern`: tắt chương trình ngoài (lệnh lạ ⇒ `VS0601`).
    let mut co_no_extern = false;
    let mut tep: Option<String> = None;
    let mut prog_args: Vec<String> = Vec::new();
    let mut mock_tep: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--tap" => tap = true,
            // ST-20: chạy đường **VM** thay cho interp (cùng ngữ nghĩa; `steps` đơn vị **lệnh**).
            "--vm" => dung_vm = true,
            "--dump-bytecode" => dump = true,
            // ST-23: cache bytecode bật **mặc định** cho đường VM; `--no-cache` để tắt.
            "--no-cache" => dung_cache = false,
            // ST-25: in cây phụ thuộc của module.
            "--dump-deps" => dump_deps = true,
            "--dry" => dry = true,
            "--strict-caps" => strict_caps = true,
            "--sandbox-check" => sandbox_check = true,
            "--grant" => {
                i += 1;
                let Some(v) = rest.get(i) else {
                    eprintln!("vis run: `--grant` cần giá trị dạng <tên>=<đường-dẫn>");
                    return ExitCode::from(2);
                };
                let Some((ten, duong)) = v.split_once('=') else {
                    eprintln!("vis run: `--grant {v}` phải dạng <tên>=<đường-dẫn>");
                    return ExitCode::from(2);
                };
                grants.push((ten.to_string(), duong.to_string()));
            }
            // ST-27: in nhật ký quyền trước mỗi lần chạy chương trình ngoài.
            "--explain-perms" => explain_perms = true,
            // ST-27: tắt chương trình ngoài ⇒ lệnh lạ `VS0601`.
            "--no-extern" => co_no_extern = true,
            // ST-42: `FsHost` **mock tất định** từ kịch bản JSON (test hermetic).
            "--fs-mock" => {
                if i + 1 >= rest.len() {
                    eprintln!("vis run: `--fs-mock` cần đường dẫn tệp kịch bản");
                    return ExitCode::from(2);
                }
                i += 1;
                mock_tep = Some(rest[i].clone());
            }
            "--" => {
                prog_args = rest[i + 1..].to_vec();
                break;
            }
            a if a.starts_with('-') => {
                eprintln!("vis run: không hỗ trợ cờ `{a}`");
                return ExitCode::from(2);
            }
            a => {
                if tep.is_some() {
                    eprintln!("vis run: chỉ nhận **một** tệp");
                    return ExitCode::from(2);
                }
                tep = Some(a.to_string());
            }
        }
        i += 1;
    }
    let Some(tep) = tep else {
        eprintln!(
            "dùng: vis run [--tap] [--vm] [--dump-bytecode] [--dump-deps] [--dry] [--sandbox-check] [--strict-caps] [--grant tên=path] [--no-cache] <tệp.vis> [-- args…]\n  --strict-caps: mọi truy cập `std.fs`/`net` phải nằm trong `cap` đã khai (mặc định: không siết)"
        );
        return ExitCode::from(2);
    };
    // ST-18: `.vish` chạy được (parse → `cmd_lower` → tyck → interp), không còn `Bail out!`.
    let fs_mock: Option<fsmock::FsMock> = match &mock_tep {
        None => None,
        Some(p) => match std::fs::read_to_string(p) {
            Ok(s) => match fsmock::FsMock::tu_json(&s) {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!("vis run: {e}");
                    return ExitCode::from(2);
                }
            },
            Err(e) => {
                eprintln!("vis run: không đọc được kịch bản `--fs-mock` `{p}`: {e}");
                return ExitCode::from(2);
            }
        },
    };
    let che_do = if tep.ends_with(".vish") {
        vis_syntax::Mode::Command
    } else {
        vis_syntax::Mode::Script
    };
    let text = match std::fs::read_to_string(&tep) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("vis run: không đọc được `{tep}`: {e}");
            return ExitCode::from(2);
        }
    };
    // **ST-73 (VD-107)**: `--sandbox-check` — probe Landlock **chỉ đọc** + mức cưỡng chế runtime;
    // in **stdout**, exit 0/3/4, **không** chạy chương trình.
    if sandbox_check {
        let caps = quyen::ds_cap(&text, che_do);
        return in_sandbox_check(&caps);
    }
    // **ST-72 (VD-106 (2))**: `--dry` — chỉ in quyền từ CST của `main`, exit 0, **không** chạy.
    if dry {
        return run_dry(&tep, &text, che_do);
    }
    // **ST-72b (VD-103/VD-106)**: quyền theo `cap` của `main` + `--grant`.
    let ds_cap = quyen::ds_cap(&text, che_do);
    let goc = std::env::current_dir().unwrap_or_default();
    let quyen = match quyen::tao(&ds_cap, &grants, &goc, strict_caps) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("vis run: {e}");
            return ExitCode::from(2);
        }
    };
    if tap {
        eprintln!("TAP version 13");
        eprintln!("1..1");
    }
    // **ST-87b (VD-109)**: **BỎ fast-path cache một tệp** — nó đọc khoá v1 (`cache::khoa`, không có
    // `rel`/deps) trong khi mọi chỗ ghi dùng khoá v2 (`cache::khoa_multi`) ⇒ **hit = 0 lần**, chỉ là
    // nhánh chết. Đường duy nhất: nạp module → frontend → `khoa_multi` (cache hit bỏ codegen). Đo:
    // tệp nhỏ lần 2 **~12 ms** (frontend ~1 ms), không hồi quy.
    // 1–3) nạp **module** (ST-25): đa tệp, `use`, cấm vòng, chẩn đoán đúng tên tệp.
    let loaded = match modules::load_with(&tep, !co_no_extern) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vis run: {e}");
            return ExitCode::from(2);
        }
    };
    if dump_deps {
        eprint!("{}", modules::dump_deps(&loaded));
    }
    if !loaded.diags.is_empty() {
        for (rel, d) in &loaded.diags {
            eprintln!(
                "{rel}:{}:{}: error {}: {}",
                d.line, d.col, d.code.short, d.message
            );
        }
        if tap {
            let ma = loaded
                .diags
                .first()
                .map(|(_, d)| d.code.short)
                .unwrap_or("VS0201");
            eprintln!("not ok 1 - {} ({ma})", tep);
        }
        return ExitCode::from(2);
    }
    let h = vis_types::HirOutput {
        hir: loaded.hir,
        diagnostics: Vec::new(),
        symbols: loaded.symbols,
        types: loaded.types,
        error: None,
    };
    // ST-54: chẩn đoán front end (parse/hạ/tyck) cho **cửa codegen** — defense-in-depth.
    let chan_doan: Vec<vis_syntax::Diagnostic> =
        loaded.diags.iter().map(|(_, d)| d.clone()).collect();
    // 4) chạy — **trong luồng có stack lớn** (ST-15): đệ quy sâu (đến cap `MAX_CALL_DEPTH` = 10^4)
    //    không được làm tràn stack process; cap vượt ⇒ `VS0501` bình thường.
    // **ST-56b (VD-97 (5))**: phần tử thứ 4 = **lỗi codegen** (đường `--vm`) ⇒ exit ≠ 0.
    let (stdout, diags, steps, loi_codegen) = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("vis-run".into())
            // 256 MiB: đo được ~8 KiB/khung ⇒ 10^4 khung ≈ 80 MiB, còn dư.
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let out = if dung_vm {
                    // ST-20: đường **VM** — cùng `Show`/builtin/bảng ngữ nghĩa với interp.
                    // ST-23: cache theo hash closure; **luôn** nạp qua `load_verified`.
                    let khoa = cache::khoa_multi(&loaded.files, che_do);
                    let tu_cache = if dung_cache { cache::nap(&khoa) } else { None };
                    let m = match tu_cache {
                        Some(m) => m,
                        None => {
                            match vis_bytecode::compile_kiem(&h.hir, &h.symbols, &text, &chan_doan)
                            {
                                Ok(m) => {
                                    if dung_cache {
                                        let _ = cache::luu(&khoa, &m);
                                    }
                                    m
                                }
                                Err(e) => {
                                    // **ST-56b (VD-97 (5))**: in chẩn đoán **và** báo hiệu
                                    // lỗi để CLI **exit ≠ 0** (trước: in rồi exit 0 — im lặng).
                                    eprintln!("vis run --vm: codegen lỗi: {e}");
                                    return (String::new(), Vec::new(), 0, true);
                                }
                            }
                        }
                    };
                    if dump {
                        eprintln!("{}", vis_vm::dump::dump(&m));
                    }
                    // ST-33/ST-63: `std.fs`/`std.net` đi qua host (chỉ CLI biết OS).
                    // **ST-89 (VD-110)**: `.vish` (trừ `--no-extern`) truyền **cùng** runner Ngoai mà
                    // interp dùng — VM chỉ gọi qua trait, không chạm OS.
                    let mut runner = ngoai::Runner {
                        giai_thich: explain_perms,
                        ..ngoai::Runner::default()
                    };
                    let co_ngoai = che_do == vis_syntax::Mode::Command && !co_no_extern;
                    let mut fs_tho = HostFs::tao(explain_perms, fs_mock.as_ref());
                    let mut net_tho = HostNet::moi(explain_perms);
                    let mut fs = quyen::FsGuard {
                        trong: &mut fs_tho,
                        quyen: &quyen,
                    };
                    let mut net = quyen::NetGuard {
                        trong: &mut net_tho,
                        quyen: &quyen,
                    };
                    let ngoai_host: Option<&mut dyn vis_interp::builtin::Ngoai> =
                        if co_ngoai { Some(&mut runner) } else { None };
                    vis_vm::run_with_full_hosts_caps(
                        &m,
                        &text,
                        &prog_args,
                        ngoai_host,
                        Some(&mut fs),
                        Some(&mut net),
                        &quyen.gia_tri(),
                    )
                } else if che_do == vis_syntax::Mode::Command && !co_no_extern {
                    // ST-27: `.vish` với runner chương trình ngoài (chỉ CLI biết OS).
                    let mut runner = ngoai::Runner {
                        giai_thich: explain_perms,
                        ..ngoai::Runner::default()
                    };
                    let mut fs_tho = HostFs::tao(explain_perms, fs_mock.as_ref());
                    let mut net_tho = HostNet::moi(explain_perms);
                    let mut fs = quyen::FsGuard {
                        trong: &mut fs_tho,
                        quyen: &quyen,
                    };
                    let mut net = quyen::NetGuard {
                        trong: &mut net_tho,
                        quyen: &quyen,
                    };
                    vis_interp::run_with_hosts_caps(
                        &text,
                        &h.hir,
                        &h.symbols,
                        &h.types,
                        &prog_args,
                        Some(&mut runner),
                        Some(&mut fs),
                        Some(&mut net),
                        &quyen.gia_tri(),
                    )
                } else {
                    // ST-33: `.vis` cũng có `std.fs` (không có chương trình ngoài).
                    let mut fs_tho = HostFs::tao(explain_perms, fs_mock.as_ref());
                    let mut net_tho = HostNet::moi(explain_perms);
                    let mut fs = quyen::FsGuard {
                        trong: &mut fs_tho,
                        quyen: &quyen,
                    };
                    let mut net = quyen::NetGuard {
                        trong: &mut net_tho,
                        quyen: &quyen,
                    };
                    vis_interp::run_with_hosts_caps(
                        &text,
                        &h.hir,
                        &h.symbols,
                        &h.types,
                        &prog_args,
                        None,
                        Some(&mut fs),
                        Some(&mut net),
                        &quyen.gia_tri(),
                    )
                };
                let d: Vec<(String, u32, u32, String)> = out
                    .diagnostics
                    .iter()
                    .map(|x| (x.code.short.to_string(), x.line, x.col, x.message.clone()))
                    .collect();
                (out.stdout, d, out.steps, false)
            })
            .expect("không tạo được luồng chạy")
            .join()
            .expect("luồng chạy lỗi")
    });
    if loi_codegen {
        if tap {
            eprintln!("not ok 1 - {tep} (codegen)");
        }
        return ExitCode::from(1);
    }
    if !xuat_stdout(&stdout) {
        return ExitCode::from(2);
    }
    if diags.is_empty() {
        if tap {
            eprintln!("ok 1 - {} (steps={})", tep, steps);
        }
        ExitCode::from(0)
    } else {
        for (ma, line, col, msg) in &diags {
            eprintln!("{tep}:{line}:{col}: error {ma}: {msg}");
        }
        if tap {
            eprintln!("not ok 1 - {} ({}, steps={})", tep, diags[0].0, steps);
        }
        ExitCode::from(ma_thoat_khi_loi(&diags))
    }
}

/// **ST-72b**: lỗi **vượt quyền** (`VS0604`, message đánh dấu `[quyền]`) là lỗi **môi trường/dùng**
/// ⇒ exit **2** (yêu cầu ST-72b); các lỗi runtime khác — kể cả `VS0604` **không do quyền** (thiếu
/// host) — giữ exit **1** như trước (tương thích `pipeline::vm_khong_host_bao_vs0604`).
fn ma_thoat_khi_loi(diags: &[(String, u32, u32, String)]) -> u8 {
    if diags.iter().any(|d| d.3.starts_with("[quyền]")) {
        2
    } else {
        1
    }
}

/// **ST-73 (VD-107)**: probe Landlock **chỉ đọc tệp** (không syscall, không `unsafe`).
/// Trả `(dòng in, mã thoát)`: Linux có ⇒ 0, không ⇒ 3, không đọc được ⇒ 4; nền khác ⇒ 3.
fn probe_landlock() -> (String, u8) {
    let os = std::env::consts::OS;
    if os != "linux" {
        return (format!("landlock: không hỗ trợ trên {os}"), 3);
    }
    match std::fs::read_to_string("/sys/kernel/security/lsm") {
        Ok(raw) => {
            let lsm: Vec<String> = raw
                .trim()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let co = lsm.iter().any(|x| x == "landlock");
            (
                format!(
                    "landlock: {} (LSM: {})",
                    if co { "có" } else { "không" },
                    lsm.join(",")
                ),
                if co { 0 } else { 3 },
            )
        }
        Err(e) => (format!("landlock: không rõ ({e})"), 4),
    }
}

/// `vis run --sandbox-check`: in **stdout** 3 dòng chẩn đoán, exit theo probe, **không** chạy.
fn in_sandbox_check(caps: &[quyen::CapKhai]) -> ExitCode {
    let (dong_landlock, ma) = probe_landlock();
    println!("{dong_landlock}");
    println!(
        "cưỡng chế runtime: {}",
        if caps.is_empty() { "tắt" } else { "bật" }
    );
    let mut dk: Vec<&str> = Vec::new();
    if caps.iter().any(|c| c.kind == "Dir" || c.kind == "File") {
        dk.push("fs");
    }
    if caps.iter().any(|c| c.kind == "Net") {
        dk.push("net");
    }
    println!(
        "cap điều kiện: {}",
        if dk.is_empty() {
            "không".to_string()
        } else {
            dk.join(",")
        }
    );
    ExitCode::from(ma)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // ST-23: quản lý cache bytecode.
        Some("cache") => match args.get(1).map(String::as_str) {
            Some("--clear") => {
                let n = cache::xoa();
                eprintln!("vis cache: đã xoá {n} tệp");
                ExitCode::from(0)
            }
            Some("--path") | None => {
                match cache::thu_muc() {
                    Some(p) => println!("{}", p.display()),
                    None => eprintln!("vis cache: không xác định được thư mục cache"),
                }
                ExitCode::from(0)
            }
            Some(khac) => {
                eprintln!("vis cache: cờ lạ `{khac}` (dùng --clear | --path)");
                ExitCode::from(2)
            }
        },
        Some("explain") => {
            let mut lang = Lang::Vi;
            let mut list = false;
            let mut ma: Option<String> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--list" => list = true,
                    "--lang" => match args.get(i + 1) {
                        Some(v) => match Lang::parse(v) {
                            Some(l) => {
                                lang = l;
                                i += 1; // bỏ qua giá trị ngôn ngữ
                            }
                            None => {
                                eprintln!("vis explain: ngôn ngữ không hỗ trợ `{v}` (vi|en|ja)");
                                return ExitCode::from(2);
                            }
                        },
                        None => {
                            eprintln!("vis explain: `--lang` cần giá trị vi|en|ja");
                            return ExitCode::from(2);
                        }
                    },
                    a if a.starts_with('-') => {
                        eprintln!("vis explain: không hỗ trợ cờ `{a}`");
                        return usage_explain(lang);
                    }
                    a => ma = Some(a.to_string()),
                }
                i += 1;
            }
            if list {
                // `explain --list` phủ **cả ba** sổ đã cấp (01xx + 02xx + 03xx, VD-44 #7).
                for c in all_codes() {
                    println!(
                        "{}  {:<8} {}",
                        c.short,
                        severity_label(c.severity, lang),
                        c.message(lang)
                    );
                }
                return ExitCode::SUCCESS;
            }
            match ma {
                None => usage_explain(lang),
                Some(code) => match lookup_str(&code) {
                    Some(c) => {
                        println!(
                            "{} — {}: {}",
                            c.short,
                            severity_label(c.severity, lang),
                            c.message(lang)
                        );
                        ExitCode::SUCCESS
                    }
                    None => {
                        // VD-20: exit 1 dành cho lỗi từ main; mã sai là **lỗi dùng** ⇒ 2.
                        eprintln!("vis: không có mã {code} trong sổ (SPEC-lex §7)");
                        ExitCode::from(2)
                    }
                },
            }
        }
        Some("run") => run_run(&args[1..]),
        Some("check") => run_check(&args[1..]),
        // ST-30: chạy bộ ca `test/sem/**` in TAP.
        Some("test") => vistest::run(&args[1..]),
        // ST-67 (VD-102): server LSP tối thiểu trên stdio (khung + JSON-RPC).
        Some("lsp") => lsp::run(&args[1..]),
        // ST-70 (VD-101): registry **local** cho gói ViScript.
        Some("pkg") => pkg::run(&args[1..]),
        Some("fmt") => run_fmt(&args[1..]),
        _ => usage(),
    }
}

/// **ST-72 (VD-106 (2))**: in danh sách quyền khai báo của `main` (từ **CST**), exit 0, không chạy.
///
/// Định dạng cố định (stdout **rỗng**; danh sách ra **stderr** như `--dump-deps`):
/// `tên: cap[Loại] đường-dẫn (write)`. Không có quyền ⇒ `(không khai báo quyền)`.
fn run_dry(tep: &str, text: &str, mode: Mode) -> ExitCode {
    let caps = quyen::ds_cap(text, mode);
    let _ = tep;
    if caps.is_empty() {
        eprintln!("(không khai báo quyền)");
        return ExitCode::SUCCESS;
    }
    for c in &caps {
        let quyen = if c.write { " (write)" } else { "" };
        if c.path.is_empty() {
            eprintln!("{}: cap[{}]{quyen}", c.name, c.kind);
        } else {
            eprintln!("{}: cap[{}] {}{quyen}", c.name, c.kind, c.path);
        }
    }
    ExitCode::SUCCESS
}
