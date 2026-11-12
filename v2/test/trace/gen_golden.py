#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Sinh/kiểm golden nhị phân của trace từ golden TEXT của sim (D2-40/D2-43).

Bản ghi 32 B little-endian: <Q t_ns><H hart><H kind><I tcb><Q a><Q b>.
Không đọc code Rust; chỉ đọc các dòng `step=...` đã ghim trong text.
"""
from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TEXT = ROOT / "v2/sim/vifast-sim/tests/golden/seed-7.s200.t4.cap4096.txt"
BIN = ROOT / "v2/test/trace/golden-sim-seed7.bin"
KIND = {"run": 6, "idle": 7}


def records(text: str):
    for line in text.splitlines():
        if not line.startswith("step="):
            continue
        parts = line[5:].split(" ")
        t_ns = int(parts[0])
        kind = 0
        tcb = 0xFFFFFFFF
        a = 0
        b = 0
        for kv in parts[1:]:
            k, v = kv.split("=", 1)
            if k == "kind":
                kind = KIND[v]
            elif k == "task":
                tcb = int(v)
            elif k == "used":
                a = int(v)
            elif k == "value":
                b = int(v)
            else:
                raise SystemExit(f"trường lạ {k}")
        yield struct.pack("<QHHIQQ", t_ns, 0, kind, tcb, a, b)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true", help="ghi lại golden")
    ap.add_argument("--check", action="store_true", help="chỉ kiểm (mặc định)")
    args = ap.parse_args()
    if not TEXT.exists():
        print(f"gen_golden: thiếu {TEXT}", file=sys.stderr)
        return 1
    data = b"".join(records(TEXT.read_text()))
    if args.write:
        BIN.write_bytes(data)
        print(f"gen_golden: ghi {len(data)} B")
        return 0
    if not BIN.exists():
        print(f"gen_golden: thiếu {BIN}", file=sys.stderr)
        return 1
    old = BIN.read_bytes()
    if old != data:
        print(f"gen_golden: lệch ({len(old)} B commit vs {len(data)} B sinh)", file=sys.stderr)
        return 1
    print(f"gen_golden: PASS ({len(data)} B, {len(data)//32} bản ghi)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
