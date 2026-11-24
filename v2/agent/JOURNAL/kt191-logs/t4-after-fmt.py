#!/usr/bin/env python3
"""KT-191 — chay checker tren cay da qua fmt (mo phong cong: fmt truoc, checker sau)."""
import shutil
import sys
from pathlib import Path

sys.path.insert(0, "/tmp/kt191-l29/bin")
import harness as H  # noqa: E402

COMPILE = Path("/tmp/kt191-l29/compile")
CORE = "v2/kernel/vifast-core/src"


def mirror(name):
    d = H.mk_tree("fmt-" + name)
    src = COMPILE / name / "src"
    for f in sorted(src.rglob("*.rs")):
        relp = f.relative_to(src)
        H.w(d, str(Path(CORE) / relp), f.read_text(encoding="utf-8"))
    return d


def main():
    out = []

    def p(*a):
        s = " ".join(str(x) for x in a)
        print(s)
        out.append(s)

    for name in ["modmetavar", "modraw", "passthrough", "macrospace", "inlinepath", "inc-comment"]:
        d = mirror(name)
        p("=" * 70)
        p("SAU-FMT " + name)
        n, o = H.both(d, name)
        out.append(n[2])
        out.append(o[2])

    H.log("t4-after-fmt.log", "\n".join(out))


if __name__ == "__main__":
    main()
