// SPDX-License-Identifier: MIT OR Apache-2.0
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
    fn so_that_v2_va_viscript_deu_doc_duoc() {
        let v2 = std::fs::read_to_string("../../agent/QUEUE.md").expect("v2 QUEUE");
        let q = parse_queue("v2/agent/QUEUE.md", &v2);
        assert!(
            q.tasks.len() >= 17,
            "v2 phải có >=17 task, đang {}",
            q.tasks.len()
        );
        let st =
            std::fs::read_to_string("../../../viscript/agent/QUEUE.md").expect("viscript QUEUE");
        let qs = parse_queue("viscript/agent/QUEUE.md", &st);
        assert!(!qs.tasks.is_empty(), "viscript phải có task ST-, đang 0");
        assert!(qs.tasks.iter().any(|t| t.id.starts_with("ST-")));
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
