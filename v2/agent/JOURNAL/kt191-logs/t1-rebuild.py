#!/usr/bin/env python3
"""KT-191 — dung lai dung cac duong lot cua L28 (B2c/B3a/B2d/T3c) + bien the."""
import sys

sys.path.insert(0, "/tmp/kt191-l29/bin")
import harness as H  # noqa: E402

LIB = "v2/kernel/vifast-core/src/lib.rs"
M = "v2/kernel/vifast-core/src/m.rs"
SUB = "v2/kernel/vifast-core/src/sub.rs"
SUBS = "v2/kernel/vifast-core/src/sub/s.rs"
SUB_EVIL = "v2/kernel/vifast-core/src/sub/evil.rs"
DECOY = "v2/kernel/vifast-core/src/evil.rs"

DECOY_TXT = "// decoy sach\npub fn khong_lam_gi() {}\n"
EVIL_TXT = "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t)\n    }\n}\n"


def base(name, macro_body, call="crate::evil_paren!();", extra_macro_attr="#[macro_export]\n"):
    d = H.mk_tree(name)
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, extra_macro_attr + macro_body)
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

    # ---------- 1. B2c: macro ngoac tron + #[path] (dung dang L28) ----------
    d = base(
        "b2c-paren",
        '#[macro_export]\nmacro_rules! evil_paren (\n    () => (\n        #[path = "evil.rs"] mod x;\n    );\n);\n',
    )
    p("=" * 70)
    p("B2c — macro ngoac TRON + #[path] (dang L28)")
    n, o = H.both(d, "B2c")
    out.append(n[2])
    out.append(o[2])

    # ---------- 2. B3a: macro ngoac tron + include! ----------
    d = base(
        "b3a-paren-inc",
        '#[macro_export]\nmacro_rules! evil_paren (\n    () => (\n        include!("evil.rs");\n    );\n);\n',
    )
    p("=" * 70)
    p("B3a — macro ngoac TRON + include!")
    n, o = H.both(d, "B3a")
    out.append(n[2])
    out.append(o[2])

    # ---------- 3. B2c vuong: macro ngoac VUONG + #[path] ----------
    d = base(
        "b2c-square",
        '#[macro_export]\nmacro_rules! evil_paren [\n    () => [\n        #[path = "evil.rs"] mod x;\n    ];\n];\n',
    )
    p("=" * 70)
    p("B2c-vuong — macro ngoac VUONG + #[path]")
    n, o = H.both(d, "B2c-square")
    out.append(n[2])
    out.append(o[2])

    # ---------- 4. B3a vuong: macro ngoac VUONG + include! ----------
    d = base(
        "b3a-square-inc",
        '#[macro_export]\nmacro_rules! evil_paren [\n    () => [\n        include!("evil.rs");\n    ];\n];\n',
    )
    p("=" * 70)
    p("B3a-vuong — macro ngoac VUONG + include!")
    n, o = H.both(d, "B3a-square")
    out.append(n[2])
    out.append(o[2])

    # ---------- 5. B2d: cfg_attr(path) trong macro ----------
    d = base(
        "b2d-cfgattr",
        '#[macro_export]\nmacro_rules! evil_paren {\n    () => {\n        #[cfg_attr(not(test), path = "evil.rs")]\n        mod x;\n    };\n}\n',
    )
    p("=" * 70)
    p("B2d — cfg_attr(not(test), path) trong than macro")
    n, o = H.both(d, "B2d")
    out.append(n[2])
    out.append(o[2])

    # ---------- 6. T3c: mod trong macro + loi goi #[cfg(not(test))] ----------
    d = H.mk_tree("t3c-mod-macro")
    H.w(d, LIB, "mod m;\nmod sub;\n")
    H.w(d, M, "#[macro_export]\nmacro_rules! evil_decl {\n    () => {\n        mod x;\n    };\n}\n")
    H.w(d, SUB, "#[cfg(test)]\nmod x;\n#[cfg(not(test))]\ncrate::evil_decl!();\n")
    H.w(d, "v2/kernel/vifast-core/src/sub/x.rs", EVIL_TXT)
    p("=" * 70)
    p("T3c — mod trong macro + loi goi #[cfg(not(test))]")
    n, o = H.both(d, "T3c")
    out.append(n[2])
    out.append(o[2])

    H.log("t1-b2c-b3a-b2d-t3c.log", "\n".join(out))


if __name__ == "__main__":
    main()
