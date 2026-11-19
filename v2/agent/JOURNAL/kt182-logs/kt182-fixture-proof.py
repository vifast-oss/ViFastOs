#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-182 — chứng minh lỗ hổng và bản vá bằng fixture (đột biến ở mức checker).

Ba fixture, mỗi cái là một cây `v2/` tối thiểu:

  A. `same-file-product-debug` — hàm **sản phẩm** cùng tệp có `debug_assert!`; test trong
     `#[cfg(test)]` gọi nó mà **không** assert. Trước KT-182: **XANH OAN**; sau: **ĐỎ**.
  B. `same-file-product-assert` — như A nhưng hàm sản phẩm có `assert!` (răng **thật**).
     Trước: **XANH OAN**; sau: **ĐỎ**.
  C. `same-file-test-helper` — trợ giúp **trong phạm vi test** (`#[cfg(test)]`) có
     `assert_eq!`; test gọi nó mà không assert. Trước: XANH; sau: **XANH** (đối chứng
     dương — siết không quá tay).

Chạy **hai** bản checker: bản trước KT-182 (lấy từ git, mặc định `8f5a1189`) và bản hiện
tại. In bảng `fixture × checker → errors`.

Dùng:
    python3 v2/agent/JOURNAL/kt182-logs/kt182-fixture-proof.py [--base 8f5a1189]
"""

from __future__ import annotations

import argparse
import importlib.util
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = HERE.parents[4]
CURRENT = REPO / "v2" / "tools" / "check-test-teeth.py"

FIXTURES = {
    "A-same-file-product-debug": {
        "v2/lib/a/src/lib.rs": (
            "pub fn kiem_san_pham(x: u32) -> u32 {\n"
            "    debug_assert!(x > 0);\n"
            "    x + 1\n"
            "}\n\n"
            "#[cfg(test)]\nmod tests {\n    use super::*;\n\n"
            "    #[test]\n    fn chi_goi_san_pham_cung_tep() {\n"
            "        let _x = kiem_san_pham(3);\n    }\n}\n"
        ),
    },
    "B-same-file-product-assert": {
        "v2/lib/a/src/lib.rs": (
            "pub fn kiem_san_pham(x: u32) -> u32 {\n"
            "    assert!(x > 0, \"phải dương\");\n"
            "    x + 1\n"
            "}\n\n"
            "#[cfg(test)]\nmod tests {\n    use super::*;\n\n"
            "    #[test]\n    fn chi_goi_san_pham_cung_tep() {\n"
            "        let _x = kiem_san_pham(3);\n    }\n}\n"
        ),
    },
    "C-same-file-test-helper": {
        "v2/lib/a/src/lib.rs": (
            "pub fn f() -> u32 {\n    1\n}\n\n"
            "#[cfg(test)]\nmod tests {\n"
            "    fn kiem_cung_tep(x: u32) {\n        assert_eq!(x, 3);\n    }\n\n"
            "    #[test]\n    fn dung_tro_giup_cung_tep() {\n"
            "        kiem_cung_tep(3);\n    }\n}\n"
        ),
    },
}


def load(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:  # pragma: no cover
        raise SystemExit(f"không nạp được {path}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def main(argv) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="8f5a1189", help="commit chứa checker TRƯỚC KT-182")
    args = ap.parse_args(argv)

    tmp = Path(tempfile.mkdtemp(prefix="kt182-fixture-"))
    try:
        before_src = subprocess.run(
            ["git", "show", f"{args.base}:v2/tools/check-test-teeth.py"],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        before_path = tmp / "check-test-teeth-before.py"
        before_path.write_text(before_src, encoding="utf-8")

        before = load(before_path, "teeth_before")
        after = load(CURRENT, "teeth_after")

        print(f"# checker TRƯỚC: {args.base}:v2/tools/check-test-teeth.py")
        print(f"# checker SAU:   {CURRENT}")
        print()
        print(f"{'fixture':<32} {'TRƯỚC':<28} {'SAU':<28} kỳ vọng")
        print("-" * 110)
        expect = {
            "A-same-file-product-debug": "TRƯỚC xanh oan → SAU đỏ",
            "B-same-file-product-assert": "TRƯỚC xanh oan → SAU đỏ",
            "C-same-file-test-helper": "TRƯỚC xanh → SAU xanh",
        }
        ok = True
        for name, files in FIXTURES.items():
            d = tmp / name
            for rel, text in files.items():
                p = d / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text, encoding="utf-8")
            rb = before.scan(d, allow_let={}, helper_allow={})
            ra = after.scan(d, allow_let={}, helper_allow={})
            sb = "XANH" if rb.ok else f"ĐỎ ({len(rb.errors)} lỗi)"
            sa = "XANH" if ra.ok else f"ĐỎ ({len(ra.errors)} lỗi)"
            print(f"{name:<32} {sb:<28} {sa:<28} {expect[name]}")
            for e in ra.errors:
                print(f"    SAU: {e}")
            # kiểm kỳ vọng
            if name.startswith("A") or name.startswith("B"):
                ok = ok and rb.ok and not ra.ok
            else:
                ok = ok and rb.ok and ra.ok
        print()
        print(f"# kết luận: {'ĐÚNG kỳ vọng' if ok else 'SAI kỳ vọng'}")
        return 0 if ok else 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
