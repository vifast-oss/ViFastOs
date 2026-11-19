#!/usr/bin/env python3
"""KT-191 — bien the + bo loc re + duong vong moi."""
import sys

sys.path.insert(0, "/tmp/kt191-l29/bin")
import harness as H  # noqa: E402

LIB = "v2/kernel/vifast-core/src/lib.rs"
M = "v2/kernel/vifast-core/src/m.rs"
SUB = "v2/kernel/vifast-core/src/sub.rs"
SUBS = "v2/kernel/vifast-core/src/sub/s.rs"
SUB_EVIL = "v2/kernel/vifast-core/src/sub/evil.rs"
DECOY = "v2/kernel/vifast-core/src/evil.rs"
CORE = "v2/kernel/vifast-core/src/core_ipc.rs"
OUT_EVIL = "v2/lib/vifast-abi/src/evil_out.rs"

DECOY_TXT = "// decoy sach\npub fn khong_lam_gi() {}\n"
EVIL_TXT = "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t)\n    }\n}\n"
OUT_TXT = "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t)\n    }\n}\n"


def base(name, macro_body, call="crate::evil_paren!();"):
    d = H.mk_tree(name)
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, macro_body)
    H.w(d, DECOY, DECOY_TXT)
    H.w(d, SUB, "#[cfg(test)]\nmod evil;\nmod s;\n")
    H.w(d, SUBS, "pub fn go() {\n    " + call + "\n}\n")
    H.w(d, SUB_EVIL, EVIL_TXT)
    return d


def main():
    out = []

    def p(*a):
        s = " ".join(str(x) for x in a)
        print(s)
        out.append(s)

    def case(label, tree, strict=False):
        p("=" * 70)
        p(label)
        n, o = H.both(tree, label, strict)
        out.append(n[2])
        out.append(o[2])
        return n, o

    # ---------- bien the macro long ba lop ----------
    d = base(
        "nest3",
        '#[macro_export]\nmacro_rules! outer {\n    () => {\n        macro_rules! mid {\n            () => {\n                macro_rules! inner {\n                    () => {\n                        #[path = "evil.rs"] mod x;\n                    };\n                }\n            };\n        }\n    };\n}\n',
        call="crate::outer!();",
    )
    case("B2c-long3 — macro long ba lop + #[path]", d)

    # ---------- macro co ';' truoc than (matcher chua ';') ----------
    d = base(
        "semi-matcher",
        '#[macro_export]\nmacro_rules! evil_paren ( ($a:expr;) => ( #[path = "evil.rs"] mod x; ); );\n',
        call="crate::evil_paren!(1;);",
    )
    case("B2c-semi — matcher chua ';' + #[path]", d)

    # ---------- macro co ';' truoc than (than la (..) va co ';' sau) ----------
    d = base(
        "semi-body",
        '#[macro_export]\nmacro_rules! evil_paren ( () => ( #[path = "evil.rs"] mod x; ); );\n',
    )
    case("B2c-semi2 — than (..) co ';' ben trong", d)

    # ---------- mod co #[path] trong macro ----------
    d = base(
        "mod-path",
        '#[macro_export]\nmacro_rules! evil_paren {\n    () => {\n        #[path = "evil.rs"]\n        mod x;\n    };\n}\n',
    )
    case("T3c-path — mod co #[path] trong macro", d)

    # ---------- pub mod trong macro ----------
    d = base(
        "pub-mod",
        '#[macro_export]\nmacro_rules! evil_paren {\n    () => {\n        pub mod x;\n    };\n}\n',
    )
    case("T3c-pub — pub mod trong macro", d)

    # ---------- mod trong macro long ----------
    d = base(
        "mod-nested",
        '#[macro_export]\nmacro_rules! outer {\n    () => {\n        macro_rules! inner {\n            () => {\n                mod x;\n            };\n        }\n    };\n}\n',
        call="crate::outer!();",
    )
    case("T3c-long — mod trong macro long", d)

    # ================= BO LOC RE (dong ~759) =================
    # dich NGOAI cay: neu bo loc mo => E_STRICT_WAKE_INCLUDE "tro ra ngoai"
    for tag, line in [
        ("inc-space", 'include ! ("../../../lib/vifast-abi/src/evil_out.rs");\n'),
        ("inc-space-brace", 'include ! { "../../../lib/vifast-abi/src/evil_out.rs" }\n'),
        ("path-space", '#[ path = "../../../lib/vifast-abi/src/evil_out.rs" ]\nmod evil_out;\n'),
        ("path-space2", '# [ path = "../../../lib/vifast-abi/src/evil_out.rs" ]\nmod evil_out;\n'),
        ("path-nl", '#[\npath = "../../../lib/vifast-abi/src/evil_out.rs"\n]\nmod evil_out;\n'),
        ("inc-comment", 'include/*c*/!("../../../lib/vifast-abi/src/evil_out.rs");\n'),
        ("inc-linecomment", 'include//x\n!("../../../lib/vifast-abi/src/evil_out.rs");\n'),
        ("path-comment", '#[/*c*/path = "../../../lib/vifast-abi/src/evil_out.rs"]\nmod evil_out;\n'),
    ]:
        d = H.mk_tree("filter-" + tag)
        H.w(d, LIB, "mod core_ipc;\n")
        H.w(d, CORE, line)
        H.w(d, OUT_EVIL, OUT_TXT)
        case("BO-LOC " + tag, d)

    H.log("t2-variants-filter.log", "\n".join(out))


if __name__ == "__main__":
    main()
