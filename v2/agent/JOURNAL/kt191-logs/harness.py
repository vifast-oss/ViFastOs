#!/usr/bin/env python3
"""KT-191 harness — reviewer L29. Dung cay fixture doc lap, chay checker moi + cu."""
import importlib.util
import os
import shutil
import subprocess
import sys
from pathlib import Path

BASE = Path("/tmp/kt191-l29")
TREES = BASE / "trees"
NEW = BASE / "bin/check-new.py"
OLD = BASE / "bin/check-old.py"


def load_checker(path):
    spec = importlib.util.spec_from_file_location("chk_" + Path(path).stem.replace("-", "_"), path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


NEWMOD = load_checker(NEW)
OLDMOD = load_checker(OLD)


def mk_tree(name):
    d = TREES / name
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    NEWMOD._fixture_v2(d)
    return d


def w(d, relp, text):
    p = d / relp
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p


def run(checker, tree, strict=False):
    cmd = [sys.executable, str(checker), "--root", str(tree)]
    if strict:
        cmd.append("--strict")
    r = subprocess.run(cmd, capture_output=True, text=True)
    errs = [ln for ln in r.stdout.splitlines() if ln.startswith("E ")]
    return r.returncode, errs, r.stdout + r.stderr


def both(tree, label, strict=False):
    rc_n, e_n, out_n = run(NEW, tree, strict)
    rc_o, e_o, out_o = run(OLD, tree, strict)
    print(f"### {label}  strict={int(strict)}")
    print(f"  NEW rc={rc_n} errors={len(e_n)}")
    for e in e_n:
        print(f"    {e}")
    print(f"  OLD rc={rc_o} errors={len(e_o)}")
    for e in e_o:
        print(f"    {e}")
    return (rc_n, e_n, out_n), (rc_o, e_o, out_o)


def log(name, text):
    (BASE / "logs" / name).write_text(text, encoding="utf-8")
