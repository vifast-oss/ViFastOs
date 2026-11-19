#!/usr/bin/env python3
"""KT-191 — rang cua bon ca selftest moi 4be/4bf/4bg/4bh."""
import sys

sys.path.insert(0, "/tmp/kt191-l29/bin")
import harness as H  # noqa: E402

CORE = "v2/kernel/vifast-core/src/core_ipc.rs"

CASES = [
    (
        "4be",
        'macro_rules! evil ( () => ( #[path = "evil_b.rs"] mod evil; ); );\n',
        "E_STRICT_MACRO_INCLUDE",
    ),
    (
        "4bf",
        'macro_rules! evil ( () => ( include!("evil_b.rs"); ); );\n',
        "E_STRICT_MACRO_INCLUDE",
    ),
    (
        "4bg",
        'macro_rules! evil ( () => ( #[cfg_attr(not(test), path = "evil_b.rs")] mod x; ); );\n',
        "E_STRICT_MACRO_INCLUDE",
    ),
    (
        "4bh",
        'macro_rules! evil ( () => ( mod evil3; ); );\n',
        "E_STRICT_MACRO_MOD",
    ),
]


def main():
    out = []

    def p(*a):
        s = " ".join(str(x) for x in a)
        print(s)
        out.append(s)

    for tag, mut, code in CASES:
        # co dot bien
        d = H.mk_tree("teeth-" + tag + "-mut")
        H.w(d, CORE, mut)
        rc_n, e_n, _ = H.run(H.NEW, d)
        rc_o, e_o, _ = H.run(H.OLD, d)
        hit = any(code in e for e in e_n)
        p(f"{tag} CO-DOT-BIEN  NEW rc={rc_n} errors={len(e_n)} hit_{code}={hit} | OLD rc={rc_o} errors={len(e_o)}")
        for e in e_n:
            p("    NEW " + e)
        for e in e_o:
            p("    OLD " + e)
        # bo canh dot bien (tep rong)
        d = H.mk_tree("teeth-" + tag + "-clean")
        H.w(d, CORE, "// khong co dot bien\n")
        rc_n2, e_n2, _ = H.run(H.NEW, d)
        rc_o2, e_o2, _ = H.run(H.OLD, d)
        p(f"{tag} BO-DOT-BIEN  NEW rc={rc_n2} errors={len(e_n2)} | OLD rc={rc_o2} errors={len(e_o2)}")
        for e in e_n2:
            p("    NEW " + e)
        p("")

    H.log("t5-teeth.log", "\n".join(out))


if __name__ == "__main__":
    main()
