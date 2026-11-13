// SPDX-License-Identifier: MIT OR Apache-2.0
//! `--dump-bytecode`: in module dạng text **chỉ để gỡ lỗi** (không phải định dạng ổn định).

use vis_bytecode::{Instr, Module};

/// Dump một module: hằng, hàm, lệnh + span.
pub fn dump(m: &Module) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "VBC1 version={} entry={} consts={} funcs={}\n",
        m.version,
        m.entry,
        m.consts.len(),
        m.funcs.len()
    ));
    for (i, c) in m.consts.iter().enumerate() {
        s.push_str(&format!("  const[{i}] kind={} {:?}\n", c.kind, c.text));
    }
    for (fi, f) in m.funcs.iter().enumerate() {
        s.push_str(&format!(
            "func[{fi}] {} arity={} nparams={} nlocals={} max_stack={} code={}\n",
            f.name,
            f.arity,
            f.nparams,
            f.nlocals,
            f.max_stack,
            f.code.len()
        ));
        for (pc, ins) in f.code.iter().enumerate() {
            let sp = f.spans.get(pc).copied().unwrap_or_default();
            s.push_str(&format!(
                "  {pc:04} {:?}  ; byte {}..{}\n",
                ins, sp.start, sp.end
            ));
        }
    }
    s
}

/// Số lệnh theo từng tag — tiện cho test "đã dùng bao nhiêu lệnh".
pub fn tag_histogram(m: &Module) -> Vec<(u8, usize)> {
    let mut h = std::collections::BTreeMap::new();
    for f in &m.funcs {
        for i in &f.code {
            *h.entry(i.tag()).or_insert(0usize) += 1;
        }
    }
    h.into_iter().collect()
}

/// `true` nếu module có lệnh `Jump` lùi (dùng cho test).
pub fn co_nhay_lui(m: &Module) -> bool {
    m.funcs.iter().any(|f| {
        f.code.iter().any(|i| match i {
            Instr::Jump(d) | Instr::JumpIfFalse(d) | Instr::IterNext(d) => *d < 0,
            _ => false,
        })
    })
}
