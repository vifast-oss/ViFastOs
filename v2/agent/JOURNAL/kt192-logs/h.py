#!/usr/bin/env python3
"""Harness độc lập L30 — dựng đột biến trên BẢN SAO cây thật, chạy checker 2 chế độ."""
import json
import os
import shutil
import subprocess
import sys

WORK = "/tmp/kt192-l30/work"
PRISTINE = "/tmp/kt192-l30/real"
CHECKER = "/private/tmp/night2-L30-review184v4/v2/tools/check-wake-paths.py"


def reset():
    if os.path.isdir(WORK):
        shutil.rmtree(WORK)
    subprocess.run(["cp", "-c", "-R", PRISTINE, WORK], check=True)


def run(strict):
    cmd = [sys.executable, CHECKER, "--root", WORK, "--json"]
    if strict:
        cmd.append("--strict")
    p = subprocess.run(cmd, capture_output=True, text=True)
    try:
        d = json.loads(p.stdout)
    except Exception:
        return p.returncode, ["<json parse fail: %r>" % p.stdout[:200]]
    return p.returncode, d["errors"]


def case(name, files, expect, note=""):
    reset()
    for relp, content in files.items():
        path = os.path.join(WORK, relp)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
    rc0, e0 = run(False)
    rc1, e1 = run(True)
    got = "RED" if rc0 else "GREEN"
    got_s = "RED" if rc1 else "GREEN"
    verdict = "OK" if (got == expect[0] and got_s == expect[1]) else "**MISMATCH**"
    print(f"### {name}  [{note}]")
    print(f"  normal={got} rc={rc0} errors={len(e0)} | strict={got_s} rc={rc1} errors={len(e1)}  expect={expect} {verdict}")
    for e in e0[:4]:
        print(f"    N {e.strip()[:180]}")
    for e in e1[:4]:
        print(f"    S {e.strip()[:180]}")
    print()
    return got, got_s, e0, e1


if __name__ == "__main__":
    pass
