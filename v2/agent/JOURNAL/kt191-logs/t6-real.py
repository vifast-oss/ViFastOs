#!/usr/bin/env python3
"""KT-191 — chay tren BAN SAO CAY THAT: moi tan cong + checker moi/cu, ca hai che do."""
import shutil
import subprocess
import sys
from pathlib import Path

BASE = Path("/tmp/kt191-l29")
REAL = BASE / "real"
NEW = BASE / "bin/check-new.py"
OLD = BASE / "bin/check-old.py"
CORE = "v2/kernel/vifast-core/src"
LIB = CORE + "/lib.rs"

EVIL = (
    "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
    "impl crate::Machine {\n"
    "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
    "        self.wake(t)\n"
    "    }\n"
    "}\n"
)
DECOY = "// SPDX-License-Identifier: MIT OR Apache-2.0\n// decoy sach\npub fn khong_lam_gi() {}\n"


def fresh(name):
    d = BASE / ("run-" + name)
    if d.exists():
        shutil.rmtree(d)
    shutil.copytree(REAL, d)
    # reset lib.rs (bo mod m; mod sub;)
    orig = (BASE / "real" / "lib.rs.orig").read_text(encoding="utf-8")
    (d / LIB).write_text(orig, encoding="utf-8")
    for junk in ("m.rs", "sub.rs", "evil.rs"):
        p = d / CORE / junk
        if p.exists():
            p.unlink()
    shutil.rmtree(d / CORE / "sub", ignore_errors=True)
    return d


def w(d, relp, text):
    p = d / relp
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


def add_mods(d):
    with open(d / LIB, "a", encoding="utf-8") as f:
        f.write("\nmod m;\nmod sub;\n")


def run(checker, d, strict):
    cmd = [sys.executable, str(checker), "--root", str(d)]
    if strict:
        cmd.append("--strict")
    r = subprocess.run(cmd, capture_output=True, text=True)
    errs = [ln for ln in r.stdout.splitlines() if ln.startswith("E ")]
    return r.returncode, errs


def report(tag, d):
    print("=" * 72)
    print(tag)
    for strict in (False, True):
        rc_n, e_n = run(NEW, d, strict)
        rc_o, e_o = run(OLD, d, strict)
        print(f"  strict={int(strict)}  NEW rc={rc_n} errors={len(e_n)} | OLD rc={rc_o} errors={len(e_o)}")
        for e in e_n:
            print("    NEW " + e)
        for e in e_o:
            print("    OLD " + e)


def main():
    out = []

    def p(*a):
        s = " ".join(str(x) for x in a)
        print(s)
        out.append(s)

    # A. Unicode macro name + #[path]  (B2c)
    d = fresh("uni-path")
    add_mods(d)
    w(d, CORE + "/m.rs", "#[macro_export]\nmacro_rules! é {\n    () => {\n        #[path = \"evil.rs\"]\n        mod x;\n    };\n}\n")
    w(d, CORE + "/sub.rs", "#[cfg(test)]\nmod evil;\nmod s;\n")
    w(d, CORE + "/sub/s.rs", "pub fn go() {\n    crate::é!();\n}\n")
    w(d, CORE + "/sub/evil.rs", EVIL)
    w(d, CORE + "/evil.rs", DECOY)
    report("A. Unicode macro name é + #[path] (B2c)", d)

    # B. Unicode macro name + include!  (B3a)
    d = fresh("uni-inc")
    add_mods(d)
    w(d, CORE + "/m.rs", "#[macro_export]\nmacro_rules! é {\n    () => {\n        include!(\"evil.rs\");\n    };\n}\n")
    w(d, CORE + "/sub.rs", "#[cfg(test)]\nmod evil;\nmod s;\n")
    w(d, CORE + "/sub/s.rs", "pub fn go() {\n    crate::é!();\n}\n")
    w(d, CORE + "/sub/evil.rs", EVIL)
    w(d, CORE + "/evil.rs", DECOY)
    report("B. Unicode macro name é + include! (B3a)", d)

    # C. Unicode macro name + mod x;  (T3c)
    d = fresh("uni-mod")
    add_mods(d)
    w(d, CORE + "/m.rs", "#[macro_export]\nmacro_rules! é {\n    () => {\n        mod x;\n    };\n}\n")
    w(d, CORE + "/sub.rs", "#[cfg(test)]\nmod x;\n#[cfg(not(test))]\ncrate::é!();\n")
    w(d, CORE + "/sub/x.rs", EVIL)
    report("C. Unicode macro name é + mod x; (T3c)", d)

    # D. mod é; trong macro ASCII  (T3c, ten module Unicode)
    d = fresh("uni-modname")
    add_mods(d)
    w(d, CORE + "/m.rs", "#[macro_export]\nmacro_rules! evil_decl {\n    () => {\n        mod 日本;\n    };\n}\n")
    w(d, CORE + "/sub.rs", "#[cfg(test)]\nmod 日本;\n#[cfg(not(test))]\ncrate::evil_decl!();\n")
    w(d, CORE + "/sub/日本.rs", EVIL)
    report("D. mod 日本; trong macro ASCII (T3c)", d)

    # E. macro_rules! r#evil (raw ident macro name) + #[path]
    d = fresh("raw-macroname")
    add_mods(d)
    w(d, CORE + "/m.rs", "#[macro_export]\nmacro_rules! r#evil {\n    () => {\n        #[path = \"evil.rs\"]\n        mod x;\n    };\n}\n")
    w(d, CORE + "/sub.rs", "#[cfg(test)]\nmod evil;\nmod s;\n")
    w(d, CORE + "/sub/s.rs", "pub fn go() {\n    crate::r#evil!();\n}\n")
    w(d, CORE + "/sub/evil.rs", EVIL)
    w(d, CORE + "/evil.rs", DECOY)
    report("E. macro_rules! r#evil (raw ident) + #[path]", d)

    # F. macro_rules ! (dau cach) + #[path]  — checker-level
    d = fresh("space-bang")
    add_mods(d)
    w(d, CORE + "/m.rs", "#[macro_export]\nmacro_rules ! evil {\n    () => {\n        #[path = \"evil.rs\"]\n        mod x;\n    };\n}\n")
    w(d, CORE + "/sub.rs", "#[cfg(test)]\nmod evil;\nmod s;\n")
    w(d, CORE + "/sub/s.rs", "pub fn go() {\n    crate::evil!();\n}\n")
    w(d, CORE + "/sub/evil.rs", EVIL)
    w(d, CORE + "/evil.rs", DECOY)
    report("F. macro_rules ! (dau cach) + #[path]", d)

    # G. include/*c*/! o cap tep (bo loc re)
    d = fresh("inc-comment")
    w(d, LIB, "mod core_ipc;\n")
    w(d, CORE + "/core_ipc.rs", 'include/*c*/!("../../../lib/vifast-abi/src/evil_out.rs");\n')
    w(d, "v2/lib/vifast-abi/src/evil_out.rs", EVIL)
    report("G. include/*c*/! cap tep (bo loc re)", d)

    # H. #[/*c*/path = ...] cap tep
    d = fresh("path-comment")
    w(d, LIB, "mod core_ipc;\n")
    w(d, CORE + "/core_ipc.rs", '#[/*c*/path = "../../../lib/vifast-abi/src/evil_out.rs"]\nmod evil_out;\n')
    w(d, "v2/lib/vifast-abi/src/evil_out.rs", EVIL)
    report("H. #[/*c*/path = ...] cap tep", d)

    (BASE / "logs" / "t6-real-tree.log").write_text("\n".join(out), encoding="utf-8")


if __name__ == "__main__":
    main()
