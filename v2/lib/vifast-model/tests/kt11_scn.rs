// SPDX-License-Identifier: MIT OR Apache-2.0
//! KT-11 — bộ chạy tối giản cho các ca **Untyped/CNode** của KT-04c (`.scn` tĩnh).
//!
//! Phạm vi: `01-cnode-copy`, `02-cnode-mint-badge`, `03-cnode-revoke-children`,
//! `16-cap-transfer-no-grant` (4 tệp có bước `invoke CNode.*`/`Untyped.Retype`).
//! Không sửa `.scn`/`spec/`: bộ chạy chỉ đọc tệp và dựng cảnh theo quy ước dưới đây.
//!
//! Quy ước cảnh (test-only, `[TK]` — `.scn` không có phần khai báo slot/fixture):
//! - một CNode gốc `root` 32 slot; mỗi tên trong `caps` được cấp một cap gốc đủ 4 quyền,
//!   đặt ở `root` slot = thứ tự khai báo (u0→0, c0→1, c1→2, ...);
//! - `cnode=<tên>`: CNode được triệu hồi (cap của nó ở `root`); `src=<tên>`: cap nguồn ở `root`;
//! - `dst=<tên>`/`slot=<tên>`: slot trong CNode được triệu hồi, chỉ số = thứ tự khai báo
//!   của tên đó (01: `dst=c1` = slot 2 của c0) — đúng dạng CPtr của ABI;
//! - `setup` có `Untyped.Retype type=Frame` **chưa mô hình** (Frame là reserved tới K2);
//!   bộ chạy ghi nhận và coi `u0` là cap Untyped gốc — đủ cho các bước CNode.
//!
//! Kết quả: **9/9** bước khớp kỳ vọng tĩnh. Theo **D2-46** (Agent 0), fixture
//! `02-cnode-mint-badge.scn` sửa lại cho đúng ngữ nghĩa badge: bước 1 `src=ep0` (không badge
//! trên `Untyped`); bước 2 thêm `c2 = "CNode"` và `dst=c2` (Mint đích rỗng, không lặp `dst`).
//! KT-11 **không** tự sửa ABI; đây chỉ là dữ liệu test.

use std::collections::BTreeMap;
use std::path::PathBuf;

use vifast_model::{
    KError, Machine, ObjId, ObjType, SlotRef, RIGHTS_ALL, RIGHT_GRANT, RIGHT_GRANT_REPLY,
    RIGHT_READ, RIGHT_WRITE,
};

const FILES: [&str; 4] = [
    "01-cnode-copy.scn",
    "02-cnode-mint-badge.scn",
    "03-cnode-revoke-children.scn",
    "16-cap-transfer-no-grant.scn",
];

fn scn_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/scn")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScnStep {
    line: usize,
    text: String,
    method: String,
    params: Vec<(String, String)>,
    expect: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Scn {
    name: String,
    caps: Vec<(String, String)>,
    setup: Vec<ScnStep>,
    thread: Vec<ScnStep>,
}

/// Tách `k=v` (giá trị có thể chứa `|`, không chứa khoảng trắng).
fn parse_params(tokens: &[&str]) -> Vec<(String, String)> {
    tokens
        .iter()
        .map(|t| {
            let (k, v) = t
                .split_once('=')
                .unwrap_or_else(|| panic!("tham số lạ {t:?}"));
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn parse_step(line_no: usize, raw: &str) -> ScnStep {
    let (body, expect) = raw
        .rsplit_once("=>")
        .unwrap_or_else(|| panic!("dòng {line_no}: thiếu `=>`: {raw}"));
    let tokens: Vec<&str> = body.split_whitespace().collect();
    assert_eq!(tokens[0], "invoke", "dòng {line_no}: chỉ hỗ trợ `invoke`");
    ScnStep {
        line: line_no,
        text: raw.trim().to_string(),
        method: tokens[1].to_string(),
        params: parse_params(&tokens[2..]),
        expect: expect.trim().to_string(),
    }
}

fn parse_scn(text: &str, name: &str) -> Scn {
    let mut caps = Vec::new();
    let mut setup = Vec::new();
    let mut thread = Vec::new();
    let mut in_setup = false;
    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("caps = {") {
            let body = rest.trim_end_matches('}');
            for item in body.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                let (k, v) = item
                    .split_once('=')
                    .unwrap_or_else(|| panic!("caps dòng {line_no}: {item:?}"));
                caps.push((k.trim().to_string(), v.trim().trim_matches('"').to_string()));
            }
        } else if t.starts_with("setup = [") {
            in_setup = true;
        } else if t.starts_with("thread = [") {
            in_setup = false;
        } else if t.starts_with('"') {
            let end = t.rfind('"').expect("chuỗi bước phải đóng bằng \"");
            let body = &t[1..end];
            if !body.contains("=>") {
                continue;
            }
            let step = parse_step(line_no, body);
            if in_setup {
                setup.push(step);
            } else {
                thread.push(step);
            }
        }
    }
    Scn {
        name: name.to_string(),
        caps,
        setup,
        thread,
    }
}

fn read_scn(file: &str) -> Scn {
    let path = scn_dir().join(file);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("đọc {}: {e}", path.display()));
    let name = file.trim_end_matches(".scn");
    let scn = parse_scn(&text, name);
    assert_eq!(scn.name, name, "{file}: name phải trùng tên tệp");
    scn
}

struct Alias {
    name: String,
    obj: ObjId,
    root_slot: SlotRef,
}

struct Fixture {
    m: Machine,
    root: ObjId,
    aliases: Vec<Alias>,
    /// Ghi đè cap nguồn của một tên (ca 16: cap đã chuyển không còn GRANT).
    src_override: BTreeMap<String, SlotRef>,
}

impl Fixture {
    fn new(scn: &Scn) -> Self {
        let mut m = Machine::new();
        let root = m.add_cnode(32);
        let mut aliases = Vec::new();
        for (i, (name, ty_name)) in scn.caps.iter().enumerate() {
            let ty = ObjType::from_name(ty_name)
                .unwrap_or_else(|| panic!("{name}: kiểu {ty_name:?} không có trong spec"));
            let obj = match ty {
                ObjType::Untyped => m.add_untyped(0x1000 * (i as u64 + 1), 16 * 1024),
                ObjType::CNode => m.add_cnode(16),
                _ => m.add_kernel_object(ty),
            };
            let cap = m.root_cap(obj);
            m.put_cap(root, i, cap)
                .unwrap_or_else(|e| panic!("{name}: đặt cap gốc: {e:?}"));
            aliases.push(Alias {
                name: name.clone(),
                obj,
                root_slot: SlotRef::new(root, i),
            });
        }
        let mut fx = Self {
            m,
            root,
            aliases,
            src_override: BTreeMap::new(),
        };
        // 16-cap-transfer-no-grant: cap nguồn `c1` đã chuyển **không còn GRANT** (chú thích tệp).
        if scn.name == "16-cap-transfer-no-grant" {
            let src = fx.alias("c1").root_slot;
            let cap =
                fx.m.mint(src, SlotRef::new(fx.root, 20), RIGHT_READ, None)
                    .expect("16: cap nguồn chỉ còn READ");
            let _ = cap;
            fx.src_override
                .insert("c1".to_string(), SlotRef::new(fx.root, 20));
        }
        fx
    }

    fn alias(&self, name: &str) -> &Alias {
        self.aliases
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("không có cap {name:?} trong caps"))
    }

    fn alias_index(&self, name: &str) -> usize {
        self.aliases
            .iter()
            .position(|a| a.name == name)
            .unwrap_or_else(|| panic!("không có cap {name:?}"))
    }

    fn cnode_cap(&self, name: &str) -> SlotRef {
        self.alias(name).root_slot
    }

    fn cnode_obj(&self, name: &str) -> ObjId {
        self.alias(name).obj
    }

    fn src(&self, name: &str) -> SlotRef {
        *self
            .src_override
            .get(name)
            .unwrap_or(&self.alias(name).root_slot)
    }

    /// `dst=<tên>` = slot trong CNode được triệu hồi, chỉ số theo thứ tự khai báo.
    fn dst(&self, cnode: &str, name: &str) -> SlotRef {
        SlotRef::new(self.cnode_obj(cnode), self.alias_index(name))
    }

    fn rights(params: &[(String, String)]) -> Option<u8> {
        params.iter().find(|(k, _)| k == "rights").map(|(_, v)| {
            let mut r = 0u8;
            for part in v.split('|') {
                match part {
                    "READ" => r |= RIGHT_READ,
                    "WRITE" => r |= RIGHT_WRITE,
                    "GRANT" => r |= RIGHT_GRANT,
                    "GRANT_REPLY" => r |= RIGHT_GRANT_REPLY,
                    other => panic!("quyền lạ {other:?}"),
                }
            }
            r
        })
    }

    fn badge(params: &[(String, String)]) -> Option<u64> {
        params
            .iter()
            .find(|(k, _)| k == "badge")
            .map(|(_, v)| v.parse().expect("badge số"))
    }

    fn param<'a>(step: &'a ScnStep, key: &str) -> &'a str {
        step.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("bước {:?} thiếu {key}", step.text))
    }

    fn run(&mut self, step: &ScnStep) -> Result<(), KError> {
        match step.method.as_str() {
            "CNode.Copy" => {
                let cnode = Self::param(step, "cnode");
                let src = self.src(Self::param(step, "src"));
                let dst = self.dst(cnode, Self::param(step, "dst"));
                self.m
                    .invoke_copy(self.cnode_cap(cnode), src, dst, Self::rights(&step.params))
                    .map(|_| ())
            }
            "CNode.Mint" => {
                let cnode = Self::param(step, "cnode");
                let src = self.src(Self::param(step, "src"));
                let dst = self.dst(cnode, Self::param(step, "dst"));
                let rights = Self::rights(&step.params).unwrap_or(RIGHTS_ALL);
                let badge = Self::badge(&step.params);
                self.m
                    .invoke_mint(self.cnode_cap(cnode), src, dst, rights, badge)
                    .map(|_| ())
            }
            "CNode.Move" => {
                let cnode = Self::param(step, "cnode");
                let src = self.src(Self::param(step, "src"));
                let dst = self.dst(cnode, Self::param(step, "dst"));
                self.m.invoke_move(self.cnode_cap(cnode), src, dst)
            }
            "CNode.Delete" => {
                let cnode = Self::param(step, "cnode");
                let dst = self.dst(cnode, Self::param(step, "slot"));
                self.m.invoke_delete(self.cnode_cap(cnode), dst)
            }
            "CNode.Revoke" => {
                let cnode = Self::param(step, "cnode");
                let slot = step
                    .params
                    .iter()
                    .find(|(k, _)| k == "slot")
                    .map(|(_, v)| self.alias_index(v));
                self.m
                    .invoke_revoke(self.cnode_cap(cnode), slot)
                    .map(|_| ())
            }
            other => panic!("bộ chạy chưa hỗ trợ {other}"),
        }
    }
}

#[derive(Debug)]
struct Outcome {
    file: &'static str,
    line: usize,
    text: String,
    expected: String,
    actual: String,
}

impl Outcome {
    fn matched(&self) -> bool {
        self.expected == self.actual
    }
}

#[test]
fn scn_untyped_cnode_cua_kt04c_parse_duoc() {
    let want = [
        ("01-cnode-copy.scn", 1usize, 3usize),
        ("02-cnode-mint-badge.scn", 1, 2),
        ("03-cnode-revoke-children.scn", 1, 3),
        ("16-cap-transfer-no-grant.scn", 0, 1),
    ];
    for (file, setup, thread) in want {
        let scn = read_scn(file);
        assert_eq!(scn.setup.len(), setup, "{file}: số bước setup");
        assert_eq!(scn.thread.len(), thread, "{file}: số bước thread");
    }
}

#[test]
fn scn_untyped_cnode_cua_kt04c_chay_va_doi_chieu() {
    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut unmodeled_setup = 0usize;
    for file in FILES {
        let scn = read_scn(file);
        let mut fx = Fixture::new(&scn);
        for step in &scn.setup {
            // Frame là object reserved tới K2: `Untyped.Retype type=Frame` chưa mô hình.
            assert_eq!(step.method, "Untyped.Retype", "{file}: setup lạ");
            let ty = Fixture::param(step, "type");
            assert_eq!(ty, "Frame", "{file}: chỉ ca Frame được miễn");
            unmodeled_setup += 1;
        }
        for step in &scn.thread {
            let actual = match fx.run(step) {
                Ok(()) => "ok".to_string(),
                Err(e) => e.name().to_string(),
            };
            fx.m.check_caps()
                .unwrap_or_else(|e| panic!("{file}:{}: cây cap hỏng: {e}", step.line));
            fx.m.check_all_bytes()
                .unwrap_or_else(|e| panic!("{file}:{}: byte hỏng: {e}", step.line));
            outcomes.push(Outcome {
                file,
                line: step.line,
                text: step.text.clone(),
                expected: step.expect.clone(),
                actual,
            });
        }
    }

    let matched = outcomes.iter().filter(|o| o.matched()).count();
    let devs: Vec<&Outcome> = outcomes.iter().filter(|o| !o.matched()).collect();
    println!(
        "# KT-11 scn: {matched}/{} bước khớp; {} bước setup chưa mô hình (Frame/K2); {} bước lệch",
        outcomes.len(),
        unmodeled_setup,
        devs.len()
    );
    for d in &devs {
        println!(
            "#   lệch {}:{}: kỳ vọng {}, thực tế {} — {}",
            d.file, d.line, d.expected, d.actual, d.text
        );
    }

    // D2-46: cả 4 tệp (01, 02, 03, 16) khớp hết; setup Frame vẫn chưa mô hình (K2).
    assert_eq!(unmodeled_setup, 3, "chỉ 3 tệp có setup Frame");
    assert_eq!(matched, outcomes.len(), "mọi bước phải khớp: {devs:?}");
    assert!(devs.is_empty(), "không còn bước lệch sau D2-46: {devs:?}");
}
