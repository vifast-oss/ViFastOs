// SPDX-License-Identifier: MIT OR Apache-2.0
// **KT-404** (`D2-259`) — crate này **0 `unsafe`** ⇒ khoá bằng máy: mọi `unsafe`
// thêm vào (kể cả trong macro) là lỗi biên dịch, không chỉ là luật cấu trúc R5.
#![forbid(unsafe_code)]
//! `vifast-agentctl` — đọc sổ QUEUE của một dòng sản phẩm (v2 / viscript) và trả lời
//! "task kế là gì", "đang chờ duyệt gì", "JSON có `project`".
//!
//! Nguyên tắc an toàn: mặc định **chỉ đọc**. Các lệnh ghi (`done`, `block`) phải có
//! `--write` và chỉ ghi đúng ô trạng thái của QUEUE đích; Agent 2 không dùng chúng trên
//! sổ thật (sổ thuộc Agent 0 — `AGENTS.md` §3).

use std::fmt;

pub const PROJECTS: [&str; 2] = ["v2", "viscript"];
const STATUSES: [&str; 4] = ["READY", "BLOCKED", "REVIEW", "DONE"];

/// Chuẩn hoá ô trạng thái: bỏ backtick/`*` (sổ thật có `**DONE**`) và viết hoa.
pub fn norm_status(cell: &str) -> String {
    cell.trim()
        .trim_matches('`')
        .trim_matches('*')
        .trim()
        .to_uppercase()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub desc: String,
    pub deps: String,
    pub budget: String,
    pub status: String,
    pub dod: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queue {
    pub path: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnknownProject(String),
    MissingFile(String),
    BadRow(String),
    UnknownTask(String),
    WriteRefused,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownProject(p) => write!(f, "project lạ: {p} (chỉ nhận v2|viscript)"),
            Error::MissingFile(p) => write!(f, "không thấy {p}"),
            Error::BadRow(r) => write!(f, "dòng QUEUE không hợp lệ: {r}"),
            Error::UnknownTask(id) => write!(f, "không có task {id} trong QUEUE"),
            Error::WriteRefused => write!(f, "cần --write để sửa QUEUE"),
        }
    }
}

/// Tách một dòng bảng markdown thành các ô, tôn trọng `\|` (escape trong ô).
pub fn split_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let line = line.trim();
    let body = line.strip_prefix('|').unwrap_or(line);
    let body = body.strip_suffix('|').unwrap_or(body);
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' => {
                cells.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

/// Đọc QUEUE từ chuỗi markdown. Chỉ lấy các dòng bắt đầu bằng `| KT-`.
pub fn parse_queue(path: &str, text: &str) -> Queue {
    let mut tasks = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("| KT-") || trimmed.starts_with("| ST-")) {
            continue;
        }
        let cells = split_row(trimmed);
        if cells.len() < 6 {
            continue;
        }
        tasks.push(Task {
            id: cells[0].clone(),
            desc: cells[1].clone(),
            deps: cells[2].clone(),
            budget: cells[3].clone(),
            status: norm_status(&cells[4]),
            dod: cells[5].clone(),
        });
    }
    Queue {
        path: path.to_string(),
        tasks,
    }
}

/// `start` chỉ hợp lệ cho task đang READY (không đẩy agent vào task REVIEW/BLOCKED/DONE).
pub fn startable(task: &Task) -> bool {
    task.status == "READY"
}

impl Queue {
    pub fn find(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn by_status(&self, status: &str) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == status).collect()
    }

    /// Phụ thuộc đã DONE chưa. Trả `(done, ghi_chú)`; ghi chú khi ô phụ thuộc
    /// không phải danh sách ID (ví dụ "các task READY") — khi đó không chặn,
    /// nhưng phải báo để người đọc biết sổ không máy-đọc-được.
    pub fn deps_done(&self, task: &Task) -> (bool, Option<String>) {
        let dep = task.deps.trim();
        if dep.is_empty() || dep == "—" || dep == "-" {
            return (true, None);
        }
        let ids = dep_ids(dep);
        if ids.is_empty() {
            return (true, Some(format!("phụ thuộc không phải ID: {dep:?}")));
        }
        for id in ids {
            match self.find(&id) {
                Some(t) if t.status == "DONE" => {}
                Some(t) => return (false, Some(format!("{id} đang {}", t.status))),
                None => return (false, Some(format!("{id} không có trong QUEUE"))),
            }
        }
        (true, None)
    }

    /// Task kế: dòng READY đầu tiên có mọi phụ thuộc DONE. **Bỏ qua REVIEW/BLOCKED/DONE.**
    pub fn next(&self) -> Option<(&Task, Option<String>)> {
        for t in &self.tasks {
            if t.status != "READY" {
                continue;
            }
            let (ok, note) = self.deps_done(t);
            if ok {
                return Some((t, note));
            }
        }
        None
    }

    /// Đặt trạng thái một task trong chuỗi QUEUE (chỉ đổi ô thứ 5 của dòng đó).
    pub fn set_status(&self, text: &str, id: &str, status: &str) -> Result<String, Error> {
        if !STATUSES.contains(&status) {
            return Err(Error::BadRow(format!("trạng thái lạ {status}")));
        }
        if self.find(id).is_none() {
            return Err(Error::UnknownTask(id.to_string()));
        }
        let mut out = String::with_capacity(text.len());
        let mut changed = false;
        for line in text.lines() {
            let trimmed = line.trim_start();
            let is_row = trimmed.starts_with(&format!("| {id} "))
                || trimmed.starts_with(&format!("| {id} |"));
            if !changed && is_row {
                if let Some(new_line) = replace_cell(trimmed, 4, status) {
                    out.push_str(&new_line);
                    out.push('\n');
                    changed = true;
                    continue;
                }
            }
            out.push_str(line);
            out.push('\n');
        }
        if !changed {
            return Err(Error::UnknownTask(id.to_string()));
        }
        Ok(out)
    }
}

/// Thay nội dung ô `idx` (0-based, không tính dấu `|` bao ngoài) trên **dòng gốc**,
/// giữ nguyên mọi byte khác — kể cả escape `\|` trong ô khác.
pub fn replace_cell(line: &str, idx: usize, value: &str) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut seps = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == '|' {
            seps.push(i);
        }
        i += 1;
    }
    if seps.len() < idx + 2 {
        return None;
    }
    let start = seps[idx] + 1;
    let end = seps[idx + 1];
    let mut out: String = chars[..start].iter().collect();
    out.push(' ');
    out.push_str(value);
    out.push(' ');
    out.extend(chars[end..].iter());
    Some(out)
}

/// `KT-04a,b,c` → `[KT-04a, KT-04b, KT-04c]`; `KT-02, KT-03` → tách theo dấu phẩy/`+`.
pub fn dep_ids(dep: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in dep.split([',', '+', '&']) {
        let part = part.trim().trim_matches('`');
        if part.is_empty() {
            continue;
        }
        let mut prefix = String::new();
        for ch in part.chars() {
            if ch.is_ascii_digit() {
                prefix.push(ch);
            } else {
                break;
            }
        }
        if part.starts_with("KT-") || part.starts_with("ST-") {
            if part.contains('-') && part.len() > 4 {
                // "KT-04a" hoặc "KT-04"
                out.push(part.split('/').next().unwrap_or(part).to_string());
            }
        } else if !prefix.is_empty() {
            // "a", "b" của dòng trước: bỏ quả — không đoán
        }
    }
    // Trường hợp "KT-04a,b,c": phần đầu là ID, các phần sau là hậu tố 1 ký tự.
    if out.len() == 1 && dep.contains(',') {
        let base = out[0].clone();
        let stem = base.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let mut expanded = vec![base.clone()];
        for part in dep.split(',').skip(1) {
            let suffix = part.trim();
            if suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_alphabetic()) {
                expanded.push(format!("{stem}{suffix}"));
            }
        }
        if expanded.len() > 1 {
            return expanded;
        }
    }
    out
}

/// JSON báo cáo — **luôn có `project`** (khối E yêu cầu).
pub fn report_json(project: &str, queue: &Queue) -> String {
    let ready: Vec<String> = queue
        .by_status("READY")
        .iter()
        .map(|t| format!("\"{}\"", t.id))
        .collect();
    let review: Vec<String> = queue
        .by_status("REVIEW")
        .iter()
        .map(|t| format!("\"{}\"", t.id))
        .collect();
    let blocked: Vec<String> = queue
        .by_status("BLOCKED")
        .iter()
        .map(|t| format!("\"{}\"", t.id))
        .collect();
    let done = queue.by_status("DONE").len();
    let next = match queue.next() {
        Some((t, _)) => format!("\"{}\"", t.id),
        None => "null".to_string(),
    };
    format!(
        "{{\"project\":\"{project}\",\"queue\":\"{}\",\"counts\":{{\"total\":{},\"done\":{},\"ready\":{},\"review\":{},\"blocked\":{}}},\"next\":{},\"ready\":[{}],\"review\":[{}],\"blocked\":[{}]}}",
        queue.path,
        queue.tasks.len(),
        done,
        ready.len(),
        review.len(),
        blocked.len(),
        next,
        ready.join(","),
        review.join(","),
        blocked.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: &str = "\
| ID | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong |
|---|---|---|---|---|---|
| KT-01 | a | — | 1 h | DONE | x |
| KT-02 | b | KT-01 | 1 h | READY | x |
| KT-03 | c | KT-02 | 1 h | BLOCKED | x |
| KT-04d | d | KT-02 | 1 h | REVIEW | x |
| KT-05 | e | KT-04d | 1 h | BLOCKED | x |
";

    #[test]
    fn next_bo_qua_review_blocked_done() {
        let q = parse_queue("q", Q);
        let (t, note) = q.next().expect("phải có task kế");
        assert_eq!(t.id, "KT-02");
        assert!(note.is_none());
    }

    #[test]
    fn next_khong_lay_task_dep_chua_done() {
        let q = parse_queue("q", Q);
        // KT-03 phụ thuộc KT-02 (READY) ⇒ không được chọn; KT-05 phụ thuộc REVIEW ⇒ không chọn.
        assert_eq!(q.next().unwrap().0.id, "KT-02");
        assert!(q.by_status("REVIEW").iter().any(|t| t.id == "KT-04d"));
    }

    #[test]
    fn dep_khong_phai_id_thi_bao_nhung_khong_chan() {
        let text = "| KT-09 | z | các task READY | 1 h | READY | x |\n";
        let q = parse_queue("q", text);
        let (ok, note) = q.deps_done(&q.tasks[0]);
        assert!(ok);
        assert!(note.unwrap().contains("không phải ID"));
    }

    #[test]
    fn dep_danh_sach_hau_to_mo_rong() {
        assert_eq!(dep_ids("KT-04a,b,c"), vec!["KT-04a", "KT-04b", "KT-04c"]);
        assert_eq!(dep_ids("KT-02"), vec!["KT-02"]);
        assert!(dep_ids("—").is_empty());
    }

    #[test]
    fn json_co_project() {
        let q = parse_queue("v2/agent/QUEUE.md", Q);
        let js = report_json("v2", &q);
        assert!(js.contains("\"project\":\"v2\""));
        assert!(js.contains("\"next\":\"KT-02\""));
        assert!(js.contains("\"KT-04d\""));
        assert!(js.contains("\"KT-05\""));
    }

    #[test]
    fn set_status_chi_doi_o_trang_thai() {
        let q = parse_queue("q", Q);
        let out = q.set_status(Q, "KT-02", "DONE").expect("phải đổi được");
        assert!(out.contains("| KT-02 | b | KT-01 | 1 h | DONE | x |"));
        assert!(out.contains("| KT-01 | a | — | 1 h | DONE | x |"));
    }

    #[test]
    fn set_status_tu_choi_ma_khong_co() {
        let q = parse_queue("q", Q);
        assert_eq!(
            q.set_status(Q, "KT-99", "DONE"),
            Err(Error::UnknownTask("KT-99".into()))
        );
    }

    #[test]
    fn split_row_ton_trong_escape() {
        let cells = split_row("| KT-06 | `x` `v2\\|viscript` | KT-02 | 2 h | READY | y |");
        assert_eq!(cells[1], "`x` `v2|viscript`");
        assert_eq!(cells.len(), 6);
    }

    #[test]
    fn so_that_v2_doc_duoc_va_viscript_da_tach() {
        let v2 = std::fs::read_to_string("../../agent/QUEUE.md").expect("v2 QUEUE");
        let q = parse_queue("v2/agent/QUEUE.md", &v2);
        assert!(
            q.tasks.len() >= 17,
            "v2 phải có >=17 task, đang {}",
            q.tasks.len()
        );
        // Dòng ViScript đã **tách sang repo riêng**: repo này chỉ còn tệp trỏ `VISCRIPT.md`.
        // Test cũ đọc `viscript/agent/QUEUE.md` nên đỏ sau khi tách — đây là cập nhật theo
        // **thực tế mới**, không phải nới kỳ vọng để né lỗi.
        let ptr = std::fs::read_to_string("../../../VISCRIPT.md").expect("VISCRIPT.md");
        assert!(
            ptr.contains("ViScript"),
            "VISCRIPT.md phải trỏ sang repo ViScript"
        );
        assert!(
            !std::path::Path::new("../../../viscript/agent/QUEUE.md").exists(),
            "viscript/ phải đã được tách khỏi repo này"
        );
    }

    #[test]
    fn trang_thai_in_dam_van_doc_duoc() {
        let text = "| ST-01 | x | — | 1 h | **DONE** | y |\n";
        let q = parse_queue("q", text);
        assert_eq!(q.tasks[0].status, "DONE");
        assert_eq!(q.by_status("DONE").len(), 1);
    }

    #[test]
    fn set_status_giu_escape_va_khong_lech_cot() {
        let text = "| KT-06 | `x` `v2\\|viscript` | KT-02 | 2 h | READY | y |\n";
        let q = parse_queue("q", text);
        let out = q.set_status(text, "KT-06", "DONE").expect("đổi được");
        assert!(out.contains("`v2\\|viscript`"), "mất escape: {out}");
        let again = parse_queue("q", &out);
        assert_eq!(again.tasks.len(), 1);
        assert_eq!(again.tasks[0].status, "DONE");
        assert_eq!(again.tasks[0].deps, "KT-02");
    }

    #[test]
    fn start_chi_cho_ready() {
        let q = parse_queue("q", Q);
        assert!(startable(q.find("KT-02").unwrap()));
        assert!(!startable(q.find("KT-04d").unwrap()));
        assert!(!startable(q.find("KT-03").unwrap()));
    }

    #[test]
    fn project_la() {
        assert!(PROJECTS.contains(&"v2"));
        assert!(PROJECTS.contains(&"viscript"));
    }
}

// ======================================================================================
// KT-333d (D2-248 mục 2e) — `lint` / `sync-queue`: kiểm & đồng bộ sổ QUEUE/STATE **máy đọc được**.
//
// `lint` là cổng của quy trình Agent 2 (chạy được trên worktree, không cần QEMU/build):
//   * **QUEUE**: mã task đúng mẫu (`^KT-[0-9]+[a-z]?$` với dự án v2, `^ST-…$` với viscript),
//     trạng thái ∈ {READY,BLOCKED,REVIEW,DONE}, không trùng mã, đủ ≥ 6 cột, phụ thuộc tồn tại,
//     READY có đủ phụ thuộc DONE, BLOCKED **không** có đủ phụ thuộc DONE (phải lật READY),
//     không có vòng lặp.
//   * **STATE**: khối hiện tại tồn tại, có dòng `HEAD` + `TASK`, số DONE khai báo khớp QUEUE
//     (theo **khoảng** `KT-243..KT-345a` nếu dòng TASK nêu khoảng, ngược lại tổng DONE), HEAD
//     khớp `git rev-parse --short HEAD`.
//
// **Phân tầng mức độ (có lý do, ghi ở `JOURNAL/kt333d-v2lib.md`)**:
//   * `E_*` = ĐỎ mặc định: lỗi **cấu trúc** QUEUE/STATE mà agent sửa được ngay.
//   * `W_*` = CẢNH BÁO: mã lịch sử đã DONE (`KT-T26`, `KT-R1`), dòng có `|` chưa escape trong ô
//     DoD (7 cột — dữ liệu lịch sử, không đổi được nếu không viết lại sổ), và **lệch STATE**
//     (HEAD/số DONE) vì `STATE.md` thuộc Agent 0/lead — sub-agent bị **cấm** sửa (AGENTS §3).
//     `--strict-state` biến lệch STATE thành ĐỎ (lead dùng ở bước tích hợp); `--strict` biến
//     **mọi** cảnh báo thành ĐỎ.
//
// `sync-queue` chỉ sửa **số đếm** `(N task)` trên dòng `TASK` của khối hiện tại (văn xuôi khác
// giữ nguyên); `--dry-run` in thay đổi mà không ghi.

/// Một phát hiện của `lint`: mã (E_/W_), dòng 1-based, thông điệp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub code: String,
    pub line: usize,
    pub msg: String,
}

impl Issue {
    fn new(code: &str, line: usize, msg: impl Into<String>) -> Issue {
        Issue {
            code: code.to_string(),
            line,
            msg: msg.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.code.starts_with("E_")
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{} {}", self.code, self.msg)
        } else {
            write!(f, "{} dòng {}: {}", self.code, self.line, self.msg)
        }
    }
}

/// Mã task đúng mẫu `^(KT|ST)-[0-9]+[a-z]?$` (theo dự án: v2 = KT, viscript = ST).
pub fn id_pattern_ok(project: &str, id: &str) -> bool {
    let prefix = if project == "viscript" { "ST-" } else { "KT-" };
    let Some(rest) = id.strip_prefix(prefix) else {
        return false;
    };
    let mut digits = 0usize;
    let mut suffix = 0usize;
    for ch in rest.chars() {
        if ch.is_ascii_digit() {
            if suffix > 0 {
                return false;
            }
            digits += 1;
        } else if ch.is_ascii_lowercase() {
            suffix += 1;
            if suffix > 1 || digits == 0 {
                return false;
            }
        } else {
            return false;
        }
    }
    digits > 0
}

/// Đọc QUEUE kèm **số dòng** (1-based) — lint cần vị trí; `parse_queue` (API cũ) giữ nguyên.
pub fn parse_queue_rows(_path: &str, text: &str) -> Vec<(usize, Task)> {
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("| KT-") || trimmed.starts_with("| ST-")) {
            continue;
        }
        let cells = split_row(trimmed);
        if cells.len() < 6 {
            continue;
        }
        rows.push((
            i + 1,
            Task {
                id: cells[0].clone(),
                desc: cells[1].clone(),
                deps: cells[2].clone(),
                budget: cells[3].clone(),
                status: norm_status(&cells[4]),
                dod: cells[5].clone(),
            },
        ));
    }
    rows
}

/// Số cột của một dòng bảng QUEUE (≥ 0). Dùng để phát hiện dòng thiếu cột (`< 6`).
pub fn row_columns(line: &str) -> usize {
    split_row(line.trim_start()).len()
}

fn issue_for_task<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
    tasks.iter().find(|t| t.id == id)
}

fn id_sort_key(id: &str) -> Option<(u32, char)> {
    let (_, rest) = id.split_once('-')?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let num = digits.parse::<u32>().ok()?;
    let suffix = rest[digits.len()..].chars().next().unwrap_or(' ');
    Some((num, suffix))
}

/// Số task DONE trong khoảng `[from, to]` (bao gồm hai đầu) — dùng cho dòng `TASK` của STATE
/// (`**KT-243..KT-345a DONE** (104 task)`): con số là đếm **trong khoảng**, không phải tổng.
pub fn done_in_range(tasks: &[Task], from: &str, to: &str) -> Option<usize> {
    let a = id_sort_key(from.trim())?;
    let b = id_sort_key(to.trim())?;
    if a > b {
        return None;
    }
    Some(
        tasks
            .iter()
            .filter(|t| t.status == "DONE")
            .filter(|t| {
                id_sort_key(&t.id)
                    .map(|k| k >= a && k <= b)
                    .unwrap_or(false)
            })
            .count(),
    )
}

/// Lint **QUEUE**. Trả danh sách Issue theo thứ tự dòng.
pub fn lint_queue(project: &str, text: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let rows = parse_queue_rows("", text);
    let tasks: Vec<Task> = rows.iter().map(|(_, t)| t.clone()).collect();

    // 1) Mã task + trạng thái + cột. Quét **mọi** dòng bắt đầu bằng `| KT-`/`| ST-` (kể cả dòng
    //    parse_queue bỏ qua vì thiếu cột).
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("| KT-") || trimmed.starts_with("| ST-")) {
            continue;
        }
        let n = row_columns(trimmed);
        if n < 6 {
            issues.push(Issue::new(
                "E_QUEUE_COLUMNS",
                i + 1,
                format!("dòng bảng QUEUE chỉ có {n} cột (cần ≥ 6: ID|Task|Phụ thuộc|Ngân sách|Trạng thái|Tiêu chí)"),
            ));
            continue;
        }
        if n > 6 {
            issues.push(Issue::new(
                "W_QUEUE_EXTRA_COLUMNS",
                i + 1,
                format!("dòng có {n} cột > 6 — nhiều khả năng có `|` chưa escape trong ô DoD (dữ liệu lịch sử)"),
            ));
        }
        let cells = split_row(trimmed);
        let id = &cells[0];
        let status = norm_status(&cells[4]);
        if !id_pattern_ok(project, id) {
            let code = if status == "DONE" {
                "W_QUEUE_LEGACY_ID"
            } else {
                "E_QUEUE_ID"
            };
            issues.push(Issue::new(
                code,
                i + 1,
                format!("mã task '{id}' không khớp mẫu ^KT-[0-9]+[a-z]?$ (dự án {project})"),
            ));
        }
        if !STATUSES.contains(&status.as_str()) {
            issues.push(Issue::new(
                "E_QUEUE_STATUS",
                i + 1,
                format!("trạng thái '{status}' không thuộc {{READY,BLOCKED,REVIEW,DONE}}"),
            ));
        }
    }

    // 2) Trùng mã.
    for (i, t) in &rows {
        if rows.iter().filter(|(_, o)| o.id == t.id).count() > 1 {
            let first = rows
                .iter()
                .find(|(_, o)| o.id == t.id)
                .map(|(n, _)| *n)
                .unwrap_or(0);
            if *i == first {
                issues.push(Issue::new(
                    "E_QUEUE_DUP",
                    *i,
                    format!("mã task '{}' xuất hiện nhiều lần trong QUEUE", t.id),
                ));
            }
        }
    }

    // 3) Phụ thuộc: tồn tại + READY/BLOCKED nhất quán + vòng lặp.
    for (i, t) in &rows {
        let ids = dep_ids(&t.deps);
        let mut all_done = !ids.is_empty();
        for d in &ids {
            match issue_for_task(&tasks, d) {
                None => {
                    // Task **đã DONE** có thể trỏ tới task của sổ cũ/đã gộp — dữ liệu lịch sử,
                    // không sửa được mà không viết lại sổ ⇒ cảnh báo. Task **còn hoạt động**
                    // (READY/BLOCKED/REVIEW) trỏ tới ID không tồn tại = lỗi thật ⇒ ĐỎ.
                    let code = if t.status == "DONE" {
                        "W_QUEUE_DEP_MISSING_LEGACY"
                    } else {
                        "E_QUEUE_DEP_MISSING"
                    };
                    issues.push(Issue::new(
                        code,
                        *i,
                        format!("{} phụ thuộc '{}' không có trong QUEUE", t.id, d),
                    ));
                    all_done = false;
                }
                Some(dt) => {
                    if dt.status != "DONE" {
                        all_done = false;
                    }
                }
            }
        }
        if !ids.is_empty() && all_done {
            if t.status == "READY" {
                // READY có đủ dep DONE là **đúng** — không báo.
            } else if t.status == "BLOCKED" {
                issues.push(Issue::new(
                    "E_QUEUE_BLOCKED_READY",
                    *i,
                    format!(
                        "{} đang BLOCKED nhưng mọi phụ thuộc ({}) đã DONE — phải lật READY",
                        t.id, t.deps
                    ),
                ));
            }
        }
        if t.status == "READY" && !ids.is_empty() && !all_done {
            let chua: Vec<String> = ids
                .iter()
                .filter(|d| {
                    issue_for_task(&tasks, d)
                        .map(|x| x.status != "DONE")
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            if !chua.is_empty() {
                issues.push(Issue::new(
                    "E_QUEUE_READY_DEP",
                    *i,
                    format!(
                        "{} đang READY nhưng phụ thuộc chưa DONE: {}",
                        t.id,
                        chua.join(", ")
                    ),
                ));
            }
        }
    }
    // Vòng lặp (DFS 3 màu trên đồ thị phụ thuộc).
    let mut color: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
    let mut cyc: Option<Vec<String>> = None;
    fn dfs_cycle(
        id: &str,
        tasks: &[Task],
        color: &mut std::collections::HashMap<String, u8>,
        stack: &mut Vec<String>,
        out: &mut Option<Vec<String>>,
    ) {
        let c = color.get(id).copied().unwrap_or(0);
        if c == 2 || out.is_some() {
            return;
        }
        if c == 1 {
            let pos = stack.iter().position(|x| x == id).unwrap_or(0);
            *out = Some(stack[pos..].to_vec());
            return;
        }
        color.insert(id.to_string(), 1);
        stack.push(id.to_string());
        if let Some(t) = tasks.iter().find(|t| t.id == id) {
            for d in dep_ids(&t.deps) {
                if tasks.iter().any(|x| x.id == d) {
                    dfs_cycle(&d, tasks, color, stack, out);
                }
            }
        }
        stack.pop();
        color.insert(id.to_string(), 2);
    }
    for t in &tasks {
        let mut stack = Vec::new();
        dfs_cycle(&t.id, &tasks, &mut color, &mut stack, &mut cyc);
        if cyc.is_some() {
            break;
        }
    }
    if let Some(cycle) = cyc {
        issues.push(Issue::new(
            "E_QUEUE_CYCLE",
            0,
            format!("vòng lặp phụ thuộc: {}", cycle.join(" → ")),
        ));
    }
    issues
}

/// Thông tin máy đọc được từ khối trạng thái **hiện tại** của `STATE.md`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateInfo {
    pub has_block: bool,
    pub head: Option<String>,
    pub head_line: usize,
    pub task_line: usize,
    pub declared_done: Option<usize>,
    pub range: Option<(String, String)>,
}

/// Đọc `STATE.md`: chỉ khối giữa hai dòng mốc `=== KHỐI TRẠNG THÁI HIỆN TẠI` và `=== HẾT KHỐI`.
pub fn parse_state(text: &str) -> StateInfo {
    let mut info = StateInfo::default();
    let mut in_block = false;
    for (i, line) in text.lines().enumerate() {
        if line.contains("=== KHỐI TRẠNG THÁI HIỆN TẠI") {
            in_block = true;
            info.has_block = true;
            continue;
        }
        if line.contains("=== HẾT KHỐI") {
            in_block = false;
            continue;
        }
        if !in_block {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("HEAD") && trimmed.contains(':') {
            let v = trimmed.split_once(':').map(|(_, v)| v).unwrap_or("");
            // HEAD thường là `b845ac34` trong backtick.
            let head = v
                .split(['`', ' '])
                .find(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit()))
                .map(|s| s.to_string());
            info.head = head;
            info.head_line = i + 1;
        }
        if trimmed.starts_with("TASK") && trimmed.contains(':') {
            info.task_line = i + 1;
            let v = trimmed.split_once(':').map(|(_, v)| v).unwrap_or("");
            // Khoảng `KT-243..KT-345a` (nếu có) và số `(N task)`.
            for tok in v.split_whitespace() {
                let tok = tok.trim_matches(|c: char| c == '*' || c == '`');
                if let Some((a, b)) = tok.split_once("..") {
                    if id_pattern_ok("v2", a) && id_pattern_ok("v2", b) {
                        info.range = Some((a.to_string(), b.to_string()));
                    }
                }
            }
            let mut rest = v;
            while let Some(pos) = rest.find("task)") {
                // Số đếm nằm **ngay trước** ` task)` (giữa có thể là khoảng trắng):
                // `**KT-243..KT-345a DONE** (104 task)` ⇒ 104.
                let before = rest[..pos].trim_end();
                let start = before
                    .rfind(|c: char| !c.is_ascii_digit())
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let num = &before[start..];
                if !num.is_empty() {
                    info.declared_done = num.parse::<usize>().ok();
                    break;
                }
                rest = &rest[pos + 5..];
            }
        }
    }
    info
}

/// Lint khối `STATE.md` so với QUEUE (+ HEAD của git nếu có).
/// `strict_state`: biến lệch STATE (HEAD/số DONE) từ **cảnh báo** thành **ĐỎ**.
pub fn lint_state(
    queue_text: &str,
    state_text: &str,
    git_head: Option<&str>,
    strict_state: bool,
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let info = parse_state(state_text);
    if !info.has_block {
        issues.push(Issue::new(
            "E_STATE_BLOCK",
            0,
            "thiếu khối '=== KHỐI TRẠNG THÁI HIỆN TẠI … === HẾT KHỐI' trong STATE.md",
        ));
        return issues;
    }
    if info.head.is_none() {
        issues.push(Issue::new(
            "E_STATE_HEAD",
            info.head_line,
            "khối hiện tại thiếu dòng HEAD đọc được (mã hex)",
        ));
    }
    let rows = parse_queue_rows("", queue_text);
    let tasks: Vec<Task> = rows.iter().map(|(_, t)| t.clone()).collect();
    let total_done = tasks.iter().filter(|t| t.status == "DONE").count();
    let actual = match &info.range {
        Some((a, b)) => done_in_range(&tasks, a, b),
        None => Some(total_done),
    };
    let code_err = if strict_state {
        "E_STATE_COUNT"
    } else {
        "W_STATE_COUNT"
    };
    match (info.declared_done, actual) {
        (None, _) => issues.push(Issue::new(
            "E_STATE_COUNT",
            info.task_line,
            "dòng TASK thiếu mẫu đếm máy đọc được '(N task)' — sync-queue không sửa được",
        )),
        (Some(decl), Some(act)) if decl != act => {
            let pham = match &info.range {
                Some((a, b)) => format!("khoảng {a}..{b}"),
                None => "tổng QUEUE".to_string(),
            };
            issues.push(Issue::new(
                code_err,
                info.task_line,
                format!("STATE khai {decl} task DONE ({pham}) nhưng QUEUE có {act} — chạy 'vifast-agentctl sync-queue'"),
            ));
        }
        _ => {}
    }
    if let (Some(state_head), Some(real)) = (info.head.as_deref(), git_head) {
        let code = if strict_state {
            "E_STATE_HEAD_DRIFT"
        } else {
            "W_STATE_HEAD_DRIFT"
        };
        if !real.starts_with(state_head) && !state_head.starts_with(real) {
            issues.push(Issue::new(
                code,
                info.head_line,
                format!("STATE ghi HEAD '{state_head}' nhưng git HEAD là '{real}' — lead cập nhật STATE.md"),
            ));
        }
    }
    issues
}

/// `sync-queue`: chỉ sửa **số đếm** trên dòng `TASK` của khối hiện tại (`(N task)`), tái dùng bộ
/// đếm của `lint_state`. Trả `None` nếu không tìm được mẫu đếm máy đọc được (không sửa gì).
pub fn sync_state_counts(state_text: &str, queue_text: &str) -> Option<String> {
    let info = parse_state(state_text);
    if !info.has_block || info.task_line == 0 {
        return None;
    }
    let decl = info.declared_done?;
    let rows = parse_queue_rows("", queue_text);
    let tasks: Vec<Task> = rows.iter().map(|(_, t)| t.clone()).collect();
    let act = match &info.range {
        Some((a, b)) => done_in_range(&tasks, a, b)?,
        None => tasks.iter().filter(|t| t.status == "DONE").count(),
    };
    if decl == act {
        return Some(state_text.to_string());
    }
    let mut out = String::with_capacity(state_text.len());
    for (i, line) in state_text.lines().enumerate() {
        if i + 1 != info.task_line {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        // Chỉ thay **số** ngay trước ` task)` — giữ nguyên mọi byte khác (văn xuôi, dấu, backtick).
        let pat = format!("{decl} task)");
        if let Some(pos) = line.find(&pat) {
            out.push_str(&line[..pos]);
            out.push_str(&format!("{act} task)"));
            out.push_str(&line[pos + pat.len()..]);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    Some(out)
}

/// Cổng **lint** của `vifast-agentctl` (CLI gọi): in từng Issue + tổng kết; 0 = xanh.
pub fn run_lint(
    project: &str,
    queue_text: &str,
    state_text: &str,
    git_head: Option<&str>,
    strict: bool,
    strict_state: bool,
    no_state: bool,
) -> (Vec<Issue>, usize) {
    let mut issues = lint_queue(project, queue_text);
    if !no_state {
        issues.extend(lint_state(queue_text, state_text, git_head, strict_state));
    }
    let errors = issues
        .iter()
        .filter(|i| i.is_error() || (strict && i.code.starts_with("W_")))
        .count();
    (issues, errors)
}
