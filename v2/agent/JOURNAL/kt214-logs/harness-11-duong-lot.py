#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-214 — harness đo **mười một** đường lọt đã biết bằng hai checker.

Mỗi đường: dựng repro (tệp mới + dòng `mod` trong `lib.rs`), chạy
  (1) `cargo check` — repro có **biên dịch được** không,
  (2) `check-wake-paths.py` (thường) và `--strict` — checker CŨ,
  (3) `check-wake-depinfo.py` — nguyên mẫu MỚI,
rồi **hoàn nguyên** (git checkout tệp đã sửa + xoá tệp mới).

Chạy: python3 /tmp/kt214/harness.py [--only 1,2,3] [--root R]
Ghi log: <root>/v2/agent/JOURNAL/kt214-logs/<mã>-<bước>.log
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

ROOT = Path("/private/tmp/night3-L48-kt214")
LOGDIR = ROOT / "v2/agent/JOURNAL/kt214-logs"
LIB_RS = "v2/kernel/vifast-core/src/lib.rs"
ABI_LIB_RS = "v2/lib/vifast-abi/src/lib.rs"
MOD_ANCHOR = "pub mod arena;"

ENV = dict(os.environ)
ENV["PATH"] = str(Path.home() / ".cargo/bin") + ":" + ENV.get("PATH", "")
ENV["RUSTUP_TOOLCHAIN"] = "1.99.1"

# ------------------------------------------------------------------ nội dung repro

EVIL_ITEM = """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt214_evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
"""

EVIL_EXPR = """// SPDX-License-Identifier: MIT OR Apache-2.0
m.wake(t)
"""

EVIL_PATH = """// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{KError, Machine, ObjId};

#[allow(dead_code)]
pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.wake(t)
}
"""

DECOY = """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn decoy() -> u32 {
    7
}
"""


def p01() -> Dict[str, object]:
    return {
        "ten": "macro_rules! passthrough `:item` + include! trong đối số lời gọi (L40)",
        "files": {
            "v2/lib/kt214_evil_core.rs": EVIL_ITEM,
            "v2/kernel/vifast-core/src/kt214_p01.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt214_pass {
    ($i:item) => {
        $i
    };
}
kt214_pass!(include! {"../../../lib/kt214_evil_core.rs"});
""",
        },
        "mods": ["kt214_p01"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p02() -> Dict[str, object]:
    return {
        "ten": "include! vị trí biểu thức trong `( … )` (L44)",
        "files": {
            "v2/lib/kt214_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt214_p02.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
fn kt214_sink<T>(_: T) {}

#[allow(dead_code)]
pub fn kt214_p02(m: &mut crate::Machine, t: crate::ObjId) {
    kt214_sink(include! {"../../../lib/kt214_evil_expr.rs"});
}
""",
        },
        "mods": ["kt214_p02"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p03() -> Dict[str, object]:
    return {
        "ten": "include! vị trí biểu thức trong `[ … ]` (L44)",
        "files": {
            "v2/lib/kt214_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt214_p03.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt214_p03(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = [include! {"../../../lib/kt214_evil_expr.rs"}];
}
""",
        },
        "mods": ["kt214_p03"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p04() -> Dict[str, object]:
    return {
        "ten": "`(0u8, include!{…})` — tuple (L44)",
        "files": {
            "v2/lib/kt214_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt214_p04.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt214_p04(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = (0u8, include! {"../../../lib/kt214_evil_expr.rs"});
}
""",
        },
        "mods": ["kt214_p04"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p05() -> Dict[str, object]:
    return {
        "ten": "`(|| include!{…})` — thân closure trong `( … )` (L44)",
        "files": {
            "v2/lib/kt214_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt214_p05.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code, unused_mut)]
pub fn kt214_p05(m: &mut crate::Machine, t: crate::ObjId) {
    let mut f = (|| include! {"../../../lib/kt214_evil_expr.rs"});
    let _ = f();
}
""",
        },
        "mods": ["kt214_p05"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p06() -> Dict[str, object]:
    return {
        "ten": "`Some(include!{…})` — đối số hàm/biến thể (L44)",
        "files": {
            "v2/lib/kt214_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt214_p06.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt214_p06(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = Some(include! {"../../../lib/kt214_evil_expr.rs"});
}
""",
        },
        "mods": ["kt214_p06"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p07() -> Dict[str, object]:
    return {
        "ten": "#[path] trong inline module của tệp non-mod-rs (L47)",
        "files": {
            "v2/kernel/vifast-core/src/kt214_p07.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
mod inner {
    #[path = "../../../kt214_p07_evil.rs"]
    mod baz;
}
""",
            "v2/kernel/vifast-core/src/kt214_p07/inner/notes.txt": "giữ thư mục tồn tại\n",
            "v2/kernel/vifast-core/kt214_p07_evil.rs": EVIL_PATH,
            "v2/kernel/kt214_p07_evil.rs": DECOY,
        },
        "mods": ["kt214_p07"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p08() -> Dict[str, object]:
    return {
        "ten": "#[path] trỏ ra ngoài `src/` qua `..` (L47)",
        "files": {
            "v2/kernel/vifast-core/src/kt214_p08.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
mod inner {
    #[path = "../../../../lib/kt214_p08_evil.rs"]
    mod baz;
}
""",
            "v2/kernel/vifast-core/src/kt214_p08/inner/notes.txt": "giữ thư mục tồn tại\n",
            "v2/kernel/lib/kt214_p08_evil.rs": EVIL_PATH,
            "v2/lib/kt214_p08_evil.rs": DECOY,
        },
        "mods": ["kt214_p08"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p09() -> Dict[str, object]:
    return {
        "ten": "include! trong `v2/lib` trỏ sang `v2/kernel` (L40/L45)",
        "files": {
            "v2/kernel/vifast-core/kt214_p09_evil.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt214_p09() {
    wake(1);
}
""",
        },
        "append": {
            ABI_LIB_RS: """
#[allow(dead_code)]
fn wake(_t: u32) {}

include!("../../../kernel/vifast-core/kt214_p09_evil.rs");
""",
        },
        "mods": [],
        "cargo": ["--manifest-path", "v2/Cargo.toml", "-p", "vifast-abi"],
        "unit": "host",
    }


def p10() -> Dict[str, object]:
    return {
        "ten": "macro_rules! `:expr` + include! (đường còn hở có lý do) (L44)",
        "files": {
            "v2/lib/kt214_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt214_p10.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt214_pass_expr {
    ($e:expr) => {
        $e
    };
}

#[allow(dead_code)]
pub fn kt214_p10(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = kt214_pass_expr!(include! {"../../../lib/kt214_evil_expr.rs"});
}
""",
        },
        "mods": ["kt214_p10"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def p11() -> Dict[str, object]:
    return {
        "ten": "`mod` khai trong lời gọi macro (`cfg_if!`) (L40)",
        "files": {
            "v2/lib/kt214_evil_core.rs": EVIL_ITEM,
            "v2/kernel/vifast-core/src/kt214_p11.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt214_cfg_if {
    (if #[cfg($($t:tt)*)] { $($i:item)* }) => { $($i)* };
}
kt214_cfg_if! {
    if #[cfg(all())] {
        #[path = "../../../lib/kt214_evil_core.rs"]
        mod kt214_evil;
    }
}
""",
        },
        "mods": ["kt214_p11"],
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


PATHS = [p01, p02, p03, p04, p05, p06, p07, p08, p09, p10, p11]


# ------------------------------------------------------------------ chạy

def run(cmd: Sequence[str], timeout: float = 900.0) -> Tuple[int, str]:
    try:
        p = subprocess.run(cmd, cwd=str(ROOT), env=ENV, capture_output=True, text=True, timeout=timeout)
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except subprocess.TimeoutExpired:
        return 124, "TIMEOUT"


def reset(spec: Dict[str, object]) -> None:
    """Hoàn nguyên: bỏ dòng `mod` đã chèn, `git checkout` tệp đã sửa, xoá tệp mới."""
    for rel in spec.get("append", {}):  # type: ignore[union-attr]
        run(["git", "checkout", "--", rel])
    if spec["mods"]:
        run(["git", "checkout", "--", LIB_RS])
    for rel in spec["files"]:  # type: ignore[index]
        p = ROOT / rel
        if p.is_file():
            p.unlink()
    # dọn thư mục rỗng do repro tạo
    for rel in spec["files"]:  # type: ignore[index]
        d = (ROOT / rel).parent
        while d != ROOT and d.is_dir() and not any(d.iterdir()):
            d.rmdir()
            d = d.parent


def apply(spec: Dict[str, object]) -> None:
    for rel, content in spec["files"].items():  # type: ignore[union-attr]
        p = ROOT / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    for rel, content in spec.get("append", {}).items():  # type: ignore[union-attr]
        p = ROOT / rel
        p.write_text(p.read_text(encoding="utf-8") + content, encoding="utf-8")
    if spec["mods"]:
        p = ROOT / LIB_RS
        text = p.read_text(encoding="utf-8")
        add = "".join(f"\nmod {m};" for m in spec["mods"])  # type: ignore[union-attr]
        assert MOD_ANCHOR in text, "không thấy mỏ neo trong lib.rs"
        p.write_text(text.replace(MOD_ANCHOR, MOD_ANCHOR + add, 1), encoding="utf-8")


def dem_loi(text: str) -> Tuple[int, List[str]]:
    codes = re.findall(r"^E (\S+)", text, re.M)
    return len(codes), codes


def main(argv: Sequence[str]) -> int:
    global ROOT
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="")
    ap.add_argument("--root", default=str(ROOT))
    args = ap.parse_args(argv)
    ROOT = Path(args.root).resolve()
    LOGDIR.mkdir(parents=True, exist_ok=True)

    chon = [int(x) for x in args.only.split(",") if x.strip()] if args.only else list(range(1, 12))
    ket_qua: List[Dict[str, object]] = []

    for idx in chon:
        spec = PATHS[idx - 1]()
        ma = f"p{idx:02d}"
        print(f"\n===== {ma}: {spec['ten']}")
        reset(spec)
        apply(spec)
        row: Dict[str, object] = {"ma": ma, "ten": spec["ten"]}

        # (1) cargo check — repro có biên dịch được không
        rc, out = run(["cargo", "check"] + list(spec["cargo"]))  # type: ignore[arg-type]
        (LOGDIR / f"{ma}-cargo.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
        row["cargo_rc"] = rc
        print(f"  cargo RC={rc}")

        # (2) checker CŨ — thường + --strict
        for mode, extra in (("normal", []), ("strict", ["--strict"])):
            rc, out = run([sys.executable, "v2/tools/check-wake-paths.py"] + extra)
            (LOGDIR / f"{ma}-static-{mode}.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
            n, codes = dem_loi(out)
            row[f"static_{mode}_rc"] = rc
            row[f"static_{mode}_n"] = n
            row[f"static_{mode}_codes"] = sorted(set(codes))
            print(f"  static {mode}: RC={rc} errors={n} {sorted(set(codes))}")

        # (3) nguyên mẫu MỚI
        rc, out = run(
            [sys.executable, "v2/tools/check-wake-depinfo.py", "--unit", str(spec["unit"])]
        )
        (LOGDIR / f"{ma}-depinfo.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
        n, codes = dem_loi(out)
        hidden = re.search(r"tệp CHỈ tới được qua .*: (\d+)", out)
        row["depinfo_rc"] = rc
        row["depinfo_n"] = n
        row["depinfo_codes"] = sorted(set(codes))
        row["depinfo_hidden"] = int(hidden.group(1)) if hidden else -1
        print(f"  depinfo: RC={rc} errors={n} {sorted(set(codes))} hidden={row['depinfo_hidden']}")

        reset(spec)
        ket_qua.append(row)

    (LOGDIR / "bang-11-duong-lot.json").write_text(
        json.dumps(ket_qua, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print("\n===== BẢNG =====")
    for r in ket_qua:
        print(
            f"{r['ma']} cargo={r['cargo_rc']:>3} "
            f"static={r['static_normal_n']:>2}/{r['static_strict_n']:>2} "
            f"depinfo={r['depinfo_n']:>2} hidden={r['depinfo_hidden']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
