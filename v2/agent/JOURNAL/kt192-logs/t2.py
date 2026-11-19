#!/usr/bin/env python3
"""L30 sweep 2 — đường lọt `#[r#path]` + escape + các nhóm còn lại."""
import sys

sys.path.insert(0, "/tmp/kt192-l30")
from fx import case  # noqa: E402

K = "v2/kernel/vifast-core/src/"
LIB = "v2/kernel/vifast-core/src/lib.rs"
BASE_LIB = "pub mod mailbox;\nmod core_sched;\n"
WAKE = "impl M { fn f(&mut self, t: u32) { let _ = self.wake(t); } }\n"
CLEAN = "pub fn clean() -> u32 { 1 }\n"


def lib(extra):
    return BASE_LIB + extra


print("======== ĐƯỜNG LỌT NGHI VẤN ========")

# B1. `#[r#path]` trỏ tới tệp TEST-ONLY trong cây — rustc nạp ở bản sản phẩm
case(
    "B1 #[r#path = \"t.rs\"] mod x; (t.rs test-only, có wake)",
    {
        LIB: lib('#[cfg(test)]\nmod t;\n#[r#path = "t.rs"]\nmod x;\n'),
        K + "t.rs": WAKE,
    },
    ("RED", "RED"),
    "KỲ VỌNG ĐỎ — nếu XANH là LỌT",
)

# B2. `#[r#path]` trỏ ra ngoài cây (v2/lib)
case(
    "B2 #[r#path = \"../../../lib/vifast-abi/src/evil_out.rs\"]",
    {
        LIB: lib('#[r#path = "../../../lib/vifast-abi/src/evil_out.rs"]\nmod evil_raw;\n'),
        "v2/lib/vifast-abi/src/evil_out.rs": WAKE,
    },
    ("RED", "RED"),
    "KỲ VỌNG ĐỎ — nếu XANH là LỌT",
)

# B3. escape trong #[path] trỏ tới tệp test-only, decoy tên có backslash
case(
    "B3 #[path = \"t\\x2ers\"] + decoy `t\\x2ers` sạch, `t.rs` test-only có wake",
    {
        LIB: lib('#[cfg(test)]\nmod t;\n#[path = "t\\x2ers"]\nmod x;\n'),
        K + "t.rs": WAKE,
        K + "t\\x2ers": CLEAN,
    },
    ("RED", "RED"),
    "KỲ VỌNG ĐỎ — nếu XANH là LỌT",
)

# B4. `#[r#path]` trỏ tới tệp SẢN PHẨM trong cây (đối chứng: luật wake phải bắt)
case(
    "B4 #[r#path = \"evil.rs\"] mod x; (evil.rs sản phẩm, có wake)",
    {
        LIB: lib('#[r#path = "evil.rs"]\nmod x;\n'),
        K + "evil.rs": WAKE,
    },
    ("RED", "RED"),
    "KỲ VỌNG ĐỎ (evil.rs là sản phẩm nên luật wake bắt)",
)

print("======== NHÓM DELIMITER ========")
case(
    "C1 macro ngoặc tròn + #[path]",
    {K + "core_ipc.rs": 'macro_rules! m ( () => ( #[path = "evil.rs"] mod x; ); );\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "C2 macro ngoặc vuông + include!",
    {K + "core_ipc.rs": 'macro_rules! m [ () => [ include!("evil.rs"); ]; ];\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "C3 macro hỗn hợp { ( [ ] ) }",
    {K + "core_ipc.rs": 'macro_rules! m { ([] {}) => { mod x; }; }\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "C4 macro có `;` trước thân",
    {K + "core_ipc.rs": 'macro_rules! m { ($a:expr;) => { mod x; }; }\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)

print("======== NHÓM COMMENT / WHITESPACE ========")
case(
    "D1 #[/*c*/path]",
    {K + "core_ipc.rs": '#[/*c*/path = "evil.rs"]\nmod x;\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "D2 include/*c*/!",
    {K + "core_ipc.rs": 'include/*c*/!("evil.rs");\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "D3 macro_rules/*c*/!",
    {K + "core_ipc.rs": 'macro_rules/*c*/! m { () => { mod x; }; }\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "D4 mod/*c*/ x;",
    {K + "core_ipc.rs": "mod/*c*/ x;\n", K + "x.rs": CLEAN},
    ("GREEN", "GREEN"),
    "khai báo thường, đích sạch — kỳ vọng XANH",
)
case(
    "D5 #[path\\n=\\n\"evil.rs\"] xuống dòng",
    {K + "core_ipc.rs": '#[path\n=\n"evil.rs"]\nmod x;\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "D6 include ! ( \"evil.rs\" ) tab",
    {K + "core_ipc.rs": 'include\t!\t(\t"evil.rs"\t);\n'},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)

print("======== NHÓM NESTING ========")
case(
    "E1 macro trong macro + #[path]",
    {
        K + "core_ipc.rs": 'macro_rules! outer { () => { macro_rules! inner { () => { #[path = "evil.rs"] mod x; }; } }; }\n'
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "E2 include! trong macro trong macro",
    {
        K + "core_ipc.rs": 'macro_rules! outer { () => { macro_rules! inner { () => { include!("evil.rs"); }; } }; }\n'
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "E3 mod trong macro lồng ba lớp",
    {
        K + "core_ipc.rs": 'macro_rules! a { () => { macro_rules! b { () => { macro_rules! c { () => { mod x; }; } }; } }; }\n'
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)

print("======== NHÓM TOKEN FORWARDING ========")
case(
    "F1 $($t:tt)*",
    {K + "core_ipc.rs": "macro_rules! pass { ($($t:tt)*) => { $($t)* }; }\n"},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "F2 $t:tt đơn",
    {K + "core_ipc.rs": "macro_rules! pass { ($t:tt) => { $t }; }\n"},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "F3 $($t:/*c*/tt)* chú thích chen",
    {K + "core_ipc.rs": "macro_rules! pass { ($($t:/*c*/tt)*) => { $($t)* }; }\n"},
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)
case(
    "F4 $($x:expr),* (hợp lệ)",
    {K + "core_ipc.rs": "macro_rules! m { ($($x:expr),*) => { vec![$($x),*] }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ — kỳ vọng XANH",
)
case(
    "F5 $m:ident (hợp lệ)",
    {K + "core_ipc.rs": "macro_rules! m { ($m:ident) => { $m.foo() }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ — kỳ vọng XANH",
)
