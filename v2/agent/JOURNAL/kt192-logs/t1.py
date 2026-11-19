#!/usr/bin/env python3
"""L30 sweep 1 — nhóm attribute / identifier / escape + đối chứng dương."""
import sys

sys.path.insert(0, "/tmp/kt192-l30")
from fx import case  # noqa: E402

K = "v2/kernel/vifast-core/src/"
LIB = "v2/kernel/vifast-core/src/lib.rs"
BASE_LIB = "pub mod mailbox;\nmod core_sched;\n"

EVIL_OUT = "impl M { fn duong_san_pham(&mut self, t: u32) { let _ = self.wake(t); } }\n"
CLEAN = "pub fn clean() -> u32 { 1 }\n"


def lib(extra):
    return BASE_LIB + extra


print("======== NHÓM ATTRIBUTE / IDENTIFIER / ESCAPE ========")

# 1. raw-ident attribute name `#[r#path = ...]`
case(
    "A1 #[r#path = \"…\"] (raw ident tên thuộc tính)",
    {
        LIB: lib('#[r#path = "../../../lib/vifast-abi/src/evil_out.rs"]\nmod evil_raw;\n'),
        "v2/lib/vifast-abi/src/evil_out.rs": EVIL_OUT,
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)

# 2. escape trong #[path]
case(
    "A2 #[path = \"…evil_out\\x2ers\"] (escape) + decoy tên có backslash",
    {
        LIB: lib('#[path = "../../../lib/vifast-abi/src/evil_out\\x2ers"]\nmod evil_esc;\n'),
        "v2/lib/vifast-abi/src/evil_out.rs": EVIL_OUT,
        "v2/lib/vifast-abi/src/evil_out\\x2ers": CLEAN,
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)

# 3. cfg_attr + raw ident path
case(
    "A3 #[cfg_attr(not(test), r#path = \"…\")]",
    {
        LIB: lib('#[cfg_attr(not(test), r#path = "../../../lib/vifast-abi/src/evil_out.rs")]\nmod evil_ca;\n'),
        "v2/lib/vifast-abi/src/evil_out.rs": EVIL_OUT,
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ",
)

# 4. cfg(test) có chú thích chen — báo oan?
case(
    "A4 #[cfg(/*c*/test)] mod t; + wake trong t.rs",
    {
        LIB: lib("#[cfg(/*c*/test)]\nmod t;\n"),
        K + "t.rs": "impl M { fn f(&mut self, t: u32) { let _ = self.wake(t); } }\n",
    },
    ("GREEN", "GREEN"),
    "code hợp lệ — kỳ vọng XANH",
)

# 5. macro có metavariable tên `path`
case(
    "A5 macro_rules! m { ($path:expr) => { $path } }",
    {K + "core_ipc.rs": "macro_rules! m { ($path:expr) => { $path }; }\n"},
    ("GREEN", "GREEN"),
    "code hợp lệ — kỳ vọng XANH",
)

# 6. macro dùng std::path
case(
    "A6 macro_rules! m { () => { std::path::Path::new(\"x\") } }",
    {K + "core_ipc.rs": 'macro_rules! m { () => { std::path::Path::new("x") }; }\n'},
    ("GREEN", "GREEN"),
    "code hợp lệ — kỳ vọng XANH",
)

# 7. macro có metavariable tên `include`
case(
    "A7 macro_rules! m { ($include:expr) => { $include } }",
    {K + "core_ipc.rs": "macro_rules! m { ($include:expr) => { $include }; }\n"},
    ("GREEN", "GREEN"),
    "code hợp lệ — kỳ vọng XANH",
)

# 8. macro sinh mod trong tệp test-only
case(
    "A8 macro_rules! mk { () => { mod tests; } } trong tệp TEST-ONLY",
    {
        LIB: lib("#[cfg(test)]\nmod capalloc_tests;\n"),
        K + "capalloc_tests.rs": "macro_rules! mk { () => { mod tests; }; }\n",
    },
    ("GREEN", "GREEN"),
    "code hợp lệ — kỳ vọng XANH",
)

# 9. cfg_attr(test, path) trong macro
case(
    "A9 macro chứa #[cfg_attr(test, path = \"…\")]",
    {
        K + "core_ipc.rs": 'macro_rules! m { () => { #[cfg_attr(test, path = "evil.rs")] mod x; }; }\n',
    },
    ("RED", "RED"),
    "kỳ vọng ĐỎ (fail-closed)",
)

print("======== ĐỐI CHỨNG DƯƠNG (phải XANH) ========")

case(
    "P1 include!(\"clean.rs\") literal trong cây",
    {K + "core_ipc.rs": 'include!("clean.rs");\n', K + "clean.rs": CLEAN},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P2 #[path = \"clean.rs\"] mod x;",
    {K + "core_ipc.rs": '#[path = "clean.rs"]\nmod x;\n', K + "clean.rs": CLEAN},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P3 macro $x:expr",
    {K + "core_ipc.rs": "macro_rules! m { ($x:expr) => { $x + 1 }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P4 macro $($x:expr),*",
    {K + "core_ipc.rs": "macro_rules! m { ($($x:expr),*) => { vec![$($x),*] }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P5 macro $m:ident",
    {K + "core_ipc.rs": "macro_rules! m { ($m:ident) => { $m.foo() }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P6 macro tên Unicode é, thân sạch",
    {K + "core_ipc.rs": "macro_rules! é { () => { 1 }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P7 #[cfg(test)] mod t; + wake trong t.rs",
    {
        LIB: lib("#[cfg(test)]\nmod t;\n"),
        K + "t.rs": "impl M { fn f(&mut self, t: u32) { let _ = self.wake(t); } }\n",
    },
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P8 #[cfg(feature = \"ktest\")] mod k; + wake",
    {
        LIB: lib('#[cfg(feature = "ktest")]\nmod k;\n'),
        K + "k.rs": "impl M { fn f(&mut self, t: u32) { let _ = self.wake(t); } }\n",
    },
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P9 macro chứa chuỗi \"mod path include\"",
    {K + "core_ipc.rs": 'macro_rules! m { () => { "mod path include" }; }\n'},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P10 macro chứa chú thích /* mod */",
    {K + "core_ipc.rs": "macro_rules! m { () => { /* mod */ 1 }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P11 macro gọi modulo(1)",
    {K + "core_ipc.rs": "macro_rules! m { () => { modulo(1) }; }\n"},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
case(
    "P12 macro gọi include_str!(\"x\")",
    {K + "core_ipc.rs": 'macro_rules! m { () => { include_str!("x") }; }\n'},
    ("GREEN", "GREEN"),
    "hợp lệ",
)
