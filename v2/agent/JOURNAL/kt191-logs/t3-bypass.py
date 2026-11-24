#!/usr/bin/env python3
"""KT-191 — duong vong moi + bao oan."""
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

DECOY_TXT = "// decoy sach\npub fn khong_lam_gi() {}\n"
EVIL_TXT = "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t)\n    }\n}\n"


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

    # ============ DUONG VONG MOI ============
    # 1. mod $n; trong than macro (T3c qua metavariable)
    d = H.mk_tree("bypass-mod-metavar")
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, '#[macro_export]\nmacro_rules! evil_decl {\n    ($n:ident) => {\n        mod $n;\n    };\n}\n')
    H.w(d, SUB, "#[cfg(test)]\nmod x;\n#[cfg(not(test))]\ncrate::evil_decl!(x);\n")
    H.w(d, "v2/kernel/vifast-core/src/sub/x.rs", EVIL_TXT)
    case("VONG-1 mod $n; trong macro (T3c metavar)", d)

    # 2. mod r#x; trong than macro (raw ident)
    d = H.mk_tree("bypass-mod-raw")
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, '#[macro_export]\nmacro_rules! evil_decl {\n    () => {\n        mod r#x;\n    };\n}\n')
    H.w(d, SUB, "#[cfg(test)]\nmod x;\n#[cfg(not(test))]\ncrate::evil_decl!();\n")
    H.w(d, "v2/kernel/vifast-core/src/sub/x.rs", EVIL_TXT)
    case("VONG-2 mod r#x; trong macro (raw ident)", d)

    # 3. macro_rules ! (co dau cach truoc !)
    d = base(
        "bypass-macro-space",
        '#[macro_export]\nmacro_rules ! evil_paren (\n    () => (\n        #[path = "evil.rs"] mod x;\n    );\n);\n',
    )
    case("VONG-3 macro_rules ! (dau cach truoc !)", d)

    # 4. passthrough macro: than khong co 'mod' literal
    d = H.mk_tree("bypass-passthrough")
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, '#[macro_export]\nmacro_rules! pass {\n    ($($t:tt)*) => {\n        $($t)*\n    };\n}\n')
    H.w(d, SUB, "#[cfg(test)]\nmod x;\n#[cfg(not(test))]\ncrate::pass!(mod x;);\n")
    H.w(d, "v2/kernel/vifast-core/src/sub/x.rs", EVIL_TXT)
    case("VONG-4 passthrough macro ($($t:tt)*) + mod x;", d)

    # 5. #[path] trong inline module (thu muc module khac)
    d = H.mk_tree("bypass-inline-path")
    H.w(d, LIB, "mod a {\n    #[path = \"evil.rs\"]\n    mod b;\n}\n")
    H.w(d, "v2/kernel/vifast-core/src/evil.rs", DECOY_TXT)  # decoy: checker giai sai ve day
    H.w(d, "v2/kernel/vifast-core/src/a/evil.rs", EVIL_TXT)  # rustc nap tep nay
    case("VONG-5 #[path] trong inline module (decoy o src/evil.rs)", d)

    # 6. include! trong inline module? (include! giai theo tep chua no => khong doi)
    # 7. cfg_attr long trong cfg_attr trong than macro
    d = base(
        "bypass-cfgattr-nested",
        '#[macro_export]\nmacro_rules! evil_paren {\n    () => {\n        #[cfg_attr(not(test), cfg_attr(unix, path = "evil.rs"))]\n        mod x;\n    };\n}\n',
    )
    case("VONG-7 cfg_attr long trong cfg_attr trong macro", d)

    # 8. include! voi duong dan tuyet doi
    d = H.mk_tree("bypass-inc-abs")
    H.w(d, LIB, "mod core_ipc;\n")
    H.w(d, CORE, 'include!("/tmp/kt191-l29/evil_abs.rs");\n')
    H.w(d, "v2/lib/vifast-abi/src/evil_out.rs", EVIL_TXT)
    case("VONG-8 include! duong dan tuyet doi", d)

    # 9. mod co #[path] trong inline module (khac voi 5)
    d = H.mk_tree("bypass-inline-path2")
    H.w(d, LIB, "mod a {\n    #[path = \"evil.rs\"]\n    pub mod b;\n}\n")
    H.w(d, "v2/kernel/vifast-core/src/evil.rs", DECOY_TXT)
    H.w(d, "v2/kernel/vifast-core/src/a/evil.rs", EVIL_TXT)
    case("VONG-9 #[path] inline module + pub mod", d)

    # ============ BAO OAN ============
    # A. macro hop le co mod tests; trong tep test-only
    d = H.mk_tree("fp-mod-tests")
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, '#[macro_export]\nmacro_rules! mk_tests {\n    () => {\n        mod tests;\n    };\n}\n')
    H.w(d, SUB, "#[cfg(test)]\nmod tests;\n#[cfg(test)]\ncrate::mk_tests!();\n")
    H.w(d, "v2/kernel/vifast-core/src/sub/tests.rs", "fn t() {}\n")
    case("BAO-OAN-A macro co mod tests; trong tep test-only", d)

    # B. cfg_attr(test, path = ...) trong than macro
    d = base(
        "fp-cfgattr-test",
        '#[macro_export]\nmacro_rules! evil_paren {\n    () => {\n        #[cfg_attr(test, path = "evil.rs")]\n        mod x;\n    };\n}\n',
    )
    case("BAO-OAN-B cfg_attr(test, path) trong macro", d)

    # C. macro co mod nhung chi goi trong #[cfg(test)]
    d = H.mk_tree("fp-mod-testcall")
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, '#[macro_export]\nmacro_rules! mk_tests {\n    () => {\n        mod tests;\n    };\n}\n')
    H.w(d, SUB, "#[cfg(test)]\ncrate::mk_tests!();\n")
    H.w(d, "v2/kernel/vifast-core/src/sub/tests.rs", "fn t() {}\n")
    case("BAO-OAN-C macro mod tests; goi trong #[cfg(test)]", d)

    H.log("t3-bypass-fp.log", "\n".join(out))


if __name__ == "__main__":
    main()
