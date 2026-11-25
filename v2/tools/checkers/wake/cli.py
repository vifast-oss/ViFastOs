#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-407 — `main()` của máy kiểm `wake` (tách từ `v2/tools/check-wake-paths.py`)."""

from __future__ import annotations

import argparse
import bisect
import inspect
import json
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

from . import core as _core
from .selftest import selftest

globals().update({k: v for k, v in vars(_core).items() if not k.startswith("__")})

# ------------------------------------------------------------------ main

def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--root", default=None, help="gốc cây nguồn (mặc định: repo chứa tệp này)")
    ap.add_argument(
        "--strict",
        action="store_true",
        help="thêm luật token `wake` đầy đủ (KT-241: luật transition/write_state/mỏ neo chạy ở MỌI chế độ)",
    )
    ap.add_argument(
        "--combined",
        action="store_true",
        help=(
            "**KT-333e (D2-247)** — một lượt quét cho **cả** chế độ thường và `--strict`: "
            "phần chung (đọc tệp, token hoá, đồ thị module, luật 1–5) chạy một lần; in hai "
            "bảng như hai lượt riêng, đỏ nếu một trong hai đỏ"
        ),
    )
    ap.add_argument("--json", action="store_true", help="in kết quả dạng JSON")
    ap.add_argument("--selftest", action="store_true", help="chạy các ca tự kiểm (KT-256: 420)")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()
    if args.combined and args.strict:
        print("check-wake-paths: `--combined` đã gồm `--strict` (bỏ cờ kia)", file=sys.stderr)
        return 2

    here = Path(__file__).resolve()
    root = Path(args.root).resolve() if args.root else here.parents[4]  # KT-407: cli.py sâu 4 cấp
    if not root.is_dir():
        print(f"check-wake-paths: không có thư mục {root}", file=sys.stderr)
        return 2

    rep_n: Optional[Report] = None
    if (root / "v2/kernel").is_dir():
        mode = "v2"
        if args.combined:
            rep_n, rep = scan_v2(root, strict=True, combined=True)
        else:
            rep = scan_v2(root, strict=args.strict)
    elif (root / "crates").is_dir():
        mode = "v1"
        # v1 không có luật riêng theo chế độ (`strict` chỉ giữ chỗ trong chữ ký) — chạy hai
        # lượt v1 là đường cũ, rẻ (chỉ `glob` vài tệp), nên `--combined` ở đây = hai report
        # độc lập y như trước; **không** gộp gì của v1 vào v2.
        rep = scan_v1(root, strict=args.strict)
        if args.combined:
            rep_n = scan_v1(root, strict=False)
    else:
        print(
            f"check-wake-paths: {root} không có `v2/kernel` lẫn `crates` — fail-closed",
            file=sys.stderr,
        )
        return 1

    def _dump(rep: Report, strict_flag: int) -> None:
        for line in rep.info:
            print(f"# {line}")
        for line in rep.warnings:
            print(f"W {line}")
        for line in rep.errors:
            print(f"E {line}")
        print(
            f"# check-wake-paths: mode={mode} strict={strict_flag} "
            f"errors={len(rep.errors)} warnings={len(rep.warnings)}"
        )

    if args.json:
        def _as_dict(rep: Report) -> dict:
            return {
                "mode": mode,
                "ok": rep.ok,
                "errors": rep.errors,
                "warnings": rep.warnings,
                "info": rep.info,
            }

        if args.combined:
            assert rep_n is not None
            print(
                json.dumps(
                    {
                        "mode": mode,
                        "combined": True,
                        "normal": _as_dict(rep_n),
                        "strict": _as_dict(rep),
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
        else:
            print(json.dumps(_as_dict(rep), ensure_ascii=False, indent=2))
    else:
        if args.combined:
            assert rep_n is not None
            # In **nguyên văn** hai bảng như hai lượt riêng trước KT-333e: cùng thứ tự
            # `#`/`W`/`E`/dòng tổng, nên log cổng so sánh được từng dòng với bản cũ.
            _dump(rep_n, 0)
            _dump(rep, 1)
        else:
            _dump(rep, int(args.strict))
    if args.combined:
        assert rep_n is not None
        return 0 if (rep_n.ok and rep.ok) else 1
    return 0 if rep.ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
