#!/usr/bin/env python3
"""KT-242 — harness dựng cây fixture tối thiểu + chèn repro, chạy lớp 1 (thường + strict).

Dùng cho review độc lập: KHÔNG sửa checker, KHÔNG nới ngưỡng. Mỗi repro là một hàm
`apply(base)` ghi tệp vào cây fixture; harness in lỗi của hai chế độ.
"""
import importlib.util
import shutil
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve()
ROOT = HERE.parents[4]          # v2/agent/JOURNAL/kt242-wake-repro → gốc repo
CHECKER = ROOT / "v2/tools/check-wake-paths.py"

spec = importlib.util.spec_from_file_location("cwp", CHECKER)
CWP = importlib.util.module_from_spec(spec)
spec.loader.exec_module(CWP)


def build(extra):
    base = Path(tempfile.mkdtemp(prefix="kt242-"))
    CWP._fixture_v2(base)
    extra(base)
    return base


def run(name, extra, show_ok=False):
    base = build(extra)
    try:
        out = {}
        for strict in (False, True):
            r = CWP.scan_v2(base, strict)
            out[strict] = r
        print(f"=== {name}")
        for strict in (False, True):
            r = out[strict]
            tag = "strict" if strict else "thuong"
            codes = sorted({e.split(" ", 1)[0] for e in r.errors})
            print(f"  [{tag}] errors={len(r.errors)} codes={codes}")
            if r.errors and show_ok:
                for e in r.errors[:6]:
                    print("     ", e.replace("\n", " ")[:200])
        return out
    finally:
        shutil.rmtree(base, ignore_errors=True)


def w(base, relp, text):
    p = base / relp
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p
