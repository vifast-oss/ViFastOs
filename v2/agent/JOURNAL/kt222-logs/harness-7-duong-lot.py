#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-222 — reviewer L51: dựng lại **bảy** đường lọt trên HEAD `be3a0cd8` và đo **cả hai lớp**.

Mỗi đường: dựng repro (tệp mới + dòng `mod` trong `lib.rs`), chạy
  (1) `cargo check` — repro có **biên dịch được** không (RC=0 mới tính là đường lọt thật),
  (2) `check-wake-paths.py` (thường) và `--strict` — **lớp 1**,
  (3) `check-wake-depinfo.py` — **lớp 2**,
rồi **hoàn nguyên** (git checkout tệp đã sửa + xoá tệp mới).

Kỳ vọng của lead: mỗi đường ĐỎ ở **cả hai** lớp. Đường nào XANH ở **cả hai** lớp ⇒ đường lọt
**SỐNG** ⇒ báo ngay.

Chạy: python3 v2/agent/JOURNAL/kt222-logs/harness-7-duong-lot.py [--only 1,2,3] [--root R]
Ghi log: <root>/v2/agent/JOURNAL/kt222-logs/repro/<mã>-<bước>.log
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Sequence, Tuple

ROOT = Path("/private/tmp/night3-L51-kt222")
LOGDIR = ROOT / "v2/agent/JOURNAL/kt222-logs/repro"
KERNEL_LIB_RS = "v2/kernel/vifast-core/src/lib.rs"
SIM_LIB_RS = "v2/sim/vifast-sim/src/lib.rs"
ABI_LIB_RS = "v2/lib/vifast-abi/src/lib.rs"
KERNEL_ANCHOR = "pub mod arena;"
SIM_ANCHOR = "pub mod clock;"

ENV = dict(os.environ)
ENV["PATH"] = str(Path.home() / ".cargo/bin") + ":" + ENV.get("PATH", "")
ENV["RUSTUP_TOOLCHAIN"] = "1.99.1"

# ------------------------------------------------------------------ nội dung repro

EVIL_ITEM = """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt222_evil(m: &mut crate::Machine, t: crate::ObjId) {
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


def r01() -> Dict[str, object]:
    """L40 — `macro_rules!` passthrough `:item` + `include!` trong **đối số lời gọi**."""
    return {
        "ten": "L40 macro_rules! :item + include! trong đối số lời gọi",
        "files": {
            "v2/lib/kt222_evil_core.rs": EVIL_ITEM,
            "v2/kernel/vifast-core/src/kt222_r01.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt222_pass {
    ($i:item) => {
        $i
    };
}
kt222_pass!(include! {"../../../lib/kt222_evil_core.rs"});
""",
        },
        "mods": ["kt222_r01"],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def r02() -> Dict[str, object]:
    """L44 — `include!` vị trí biểu thức trong `( … )`."""
    return {
        "ten": "L44 include! vị trí biểu thức trong ( … )",
        "files": {
            "v2/lib/kt222_evil_expr.rs": EVIL_EXPR,
            "v2/kernel/vifast-core/src/kt222_r02.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
fn kt222_sink<T>(_: T) {}

#[allow(dead_code)]
pub fn kt222_r02(m: &mut crate::Machine, t: crate::ObjId) {
    kt222_sink(include! {"../../../lib/kt222_evil_expr.rs"});
}
""",
        },
        "mods": ["kt222_r02"],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def r03() -> Dict[str, object]:
    """L47 — `#[path]` trong inline module của tệp **non-mod-rs**."""
    return {
        "ten": "L47 #[path] trong inline module của tệp non-mod-rs",
        "files": {
            "v2/kernel/vifast-core/src/kt222_r03.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
mod inner {
    #[path = "../../../kt222_r03_evil.rs"]
    mod baz;
}
""",
            "v2/kernel/vifast-core/src/kt222_r03/inner/notes.txt": "giữ thư mục tồn tại\n",
            "v2/kernel/vifast-core/kt222_r03_evil.rs": EVIL_PATH,
            "v2/kernel/kt222_r03_evil.rs": DECOY,
        },
        "mods": ["kt222_r03"],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def r04() -> Dict[str, object]:
    """L47 — thư mục tên `target` (`src/target/evil.rs`), hình dạng KT-215."""
    return {
        "ten": "L47 src/target/evil.rs (thư mục tên target)",
        "files": {
            "v2/kernel/vifast-core/src/target/mod.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
mod evil;
""",
            "v2/kernel/vifast-core/src/target/evil.rs": EVIL_PATH,
        },
        "mods": ["target"],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def r05() -> Dict[str, object]:
    """L49 — `#[cfg(test)]` trên nhánh `match` (đường lọt #13, hình dạng KT-219)."""
    return {
        "ten": "L49 #[cfg(test)] trên nhánh match (đường lọt #13)",
        "files": {
            "v2/sim/vifast-sim/src/kt222_r05.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::kobj::{KError, Machine, ObjId};

const A: u32 = 0;
const B: u32 = 1;

#[allow(dead_code)]
pub fn evil(m: &mut Machine, t: ObjId, sel: u32) -> Result<(), KError> {
    match sel {
        #[cfg(test)]
        A => (),
        B => {
            m.wake(t)?;
        }
        _ => (),
    }
    Ok(())
}
""",
        },
        "mods": ["kt222_r05"],
        "lib_rs": SIM_LIB_RS,
        "anchor": SIM_ANCHOR,
        "cargo": ["--manifest-path", "v2/Cargo.toml", "-p", "vifast-sim"],
        "unit": "host",
    }


def r06() -> Dict[str, object]:
    """L40 — `mod` khai trong lời gọi macro (`cfg_if!`)."""
    return {
        "ten": "L40 mod trong lời gọi macro (cfg_if!)",
        "files": {
            "v2/lib/kt222_evil_core.rs": EVIL_ITEM,
            "v2/kernel/vifast-core/src/kt222_r06.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
macro_rules! kt222_cfg_if {
    (if #[cfg($($t:tt)*)] { $($i:item)* }) => { $($i)* };
}
kt222_cfg_if! {
    if #[cfg(all())] {
        #[path = "../../../lib/kt222_evil_core.rs"]
        mod kt222_evil;
    }
}
""",
        },
        "mods": ["kt222_r06"],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def r07() -> Dict[str, object]:
    """L40/L45 — `include!` trong `v2/lib` trỏ sang `v2/kernel`."""
    return {
        "ten": "L40/L45 include! trong v2/lib trỏ sang v2/kernel",
        "files": {
            "v2/kernel/vifast-core/kt222_r07_evil.rs": """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn kt222_r07() {
    wake(1);
}
""",
        },
        "append": {
            ABI_LIB_RS: """
#[allow(dead_code)]
fn wake(_t: u32) {}

include!("../../../kernel/vifast-core/kt222_r07_evil.rs");
""",
        },
        "mods": [],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/Cargo.toml", "-p", "vifast-abi"],
        "unit": "host",
    }


PATHS = [r01, r02, r03, r04, r05, r06, r07]


# ------------------------------------------------------------------ chạy

def run(cmd: Sequence[str], timeout: float = 900.0) -> Tuple[int, str]:
    try:
        p = subprocess.run(
            cmd, cwd=str(ROOT), env=ENV, capture_output=True, text=True, timeout=timeout
        )
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except subprocess.TimeoutExpired:
        return 124, "TIMEOUT"


def reset(spec: Dict[str, object]) -> None:
    """Hoàn nguyên: bỏ dòng `mod` đã chèn, `git checkout` tệp đã sửa, xoá tệp mới."""
    for rel in spec.get("append", {}):  # type: ignore[union-attr]
        run(["git", "checkout", "--", rel])
    if spec["mods"]:
        run(["git", "checkout", "--", str(spec["lib_rs"])])
    for rel in spec["files"]:  # type: ignore[index]
        p = ROOT / rel
        if p.is_file():
            p.unlink()
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
        p = ROOT / str(spec["lib_rs"])
        text = p.read_text(encoding="utf-8")
        add = "".join(f"\nmod {m};" for m in spec["mods"])  # type: ignore[union-attr]
        assert str(spec["anchor"]) in text, "không thấy mỏ neo trong lib.rs"
        p.write_text(text.replace(str(spec["anchor"]), str(spec["anchor"]) + add, 1), encoding="utf-8")


def dem_loi(text: str) -> Tuple[int, List[str]]:
    codes = re.findall(r"^E (\S+)", text, re.M)
    return len(codes), codes


def main(argv: Sequence[str]) -> int:
    global ROOT, LOGDIR
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="")
    ap.add_argument("--root", default=str(ROOT))
    args = ap.parse_args(argv)
    ROOT = Path(args.root).resolve()
    LOGDIR = ROOT / "v2/agent/JOURNAL/kt222-logs/repro"
    LOGDIR.mkdir(parents=True, exist_ok=True)

    chon = [int(x) for x in args.only.split(",") if x.strip()] if args.only else list(range(1, 8))
    ket_qua: List[Dict[str, object]] = []

    for idx in chon:
        spec = PATHS[idx - 1]()
        ma = f"r{idx:02d}"
        print(f"\n===== {ma}: {spec['ten']}")
        reset(spec)
        apply(spec)
        row: Dict[str, object] = {"ma": ma, "ten": spec["ten"]}

        # (1) cargo check — repro có biên dịch được không
        rc, out = run(["cargo", "check"] + list(spec["cargo"]))  # type: ignore[arg-type]
        (LOGDIR / f"{ma}-cargo.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
        row["cargo_rc"] = rc
        print(f"  cargo RC={rc}")

        # (2) lớp 1 — thường + --strict
        for mode, extra in (("normal", []), ("strict", ["--strict"])):
            rc, out = run([sys.executable, "v2/tools/check-wake-paths.py"] + extra)
            (LOGDIR / f"{ma}-static-{mode}.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
            n, codes = dem_loi(out)
            row[f"static_{mode}_rc"] = rc
            row[f"static_{mode}_n"] = n
            row[f"static_{mode}_codes"] = sorted(set(codes))
            print(f"  lớp 1 {mode}: RC={rc} errors={n} {sorted(set(codes))}")

        # (3) lớp 2 — dep-info
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
        print(f"  lớp 2: RC={rc} errors={n} {sorted(set(codes))} hidden={row['depinfo_hidden']}")

        reset(spec)
        ket_qua.append(row)

    (LOGDIR / "bang-7-duong-lot.json").write_text(
        json.dumps(ket_qua, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print("\n===== BẢNG =====")
    for r in ket_qua:
        print(
            f"{r['ma']} cargo={r['cargo_rc']:>3} "
            f"L1={r['static_normal_n']:>2}/{r['static_strict_n']:>2} "
            f"L2={r['depinfo_n']:>2} hidden={r['depinfo_hidden']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
