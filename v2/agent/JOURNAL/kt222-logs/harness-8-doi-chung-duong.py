#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-222 — reviewer L51: **tám đối chứng dương** trên HEAD `be3a0cd8`.

Mỗi đối chứng là một dạng **hợp lệ** (không phải đường lọt). Kỳ vọng: **XANH ở cả hai lớp**.
Đỏ ⇒ **báo oan**.

Tám dạng (theo phiếu lead):
  1. `include_str!`
  2. `include_bytes!`
  3. `stringify!(include)`
  4. `#[path]` hợp lệ trong cây
  5. `mod` bình thường
  6. `#[cfg(test)] mod tests` (có `wake` bên trong — phải được miễn)
  7. `wake` trong tệp allowlist (tệp định nghĩa `fn wake` — `WAKE_PRODUCER`)
  8. tệp trong `v2/kernel/target/` (thư mục build — phải bị bỏ qua)

Chạy: python3 v2/agent/JOURNAL/kt222-logs/harness-8-doi-chung-duong.py [--root R]
Ghi log: <root>/v2/agent/JOURNAL/kt222-logs/pos/<mã>-<bước>.log
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
LOGDIR = ROOT / "v2/agent/JOURNAL/kt222-logs/pos"
KERNEL_LIB_RS = "v2/kernel/vifast-core/src/lib.rs"
KERNEL_ANCHOR = "pub mod arena;"

ENV = dict(os.environ)
ENV["PATH"] = str(Path.home() / ".cargo/bin") + ":" + ENV.get("PATH", "")
ENV["RUSTUP_TOOLCHAIN"] = "1.99.1"

POS_MAIN = """// SPDX-License-Identifier: MIT OR Apache-2.0
//! Đối chứng dương KT-222 — mọi dạng dưới đây phải XANH ở cả hai lớp.

pub const POS_STR: &str = include_str!("../../../lib/kt222_pos.txt");
pub const POS_BYTES: &[u8] = include_bytes!("../../../lib/kt222_pos.bin");
pub const POS_STRINGIFY: &str = stringify!(include);

#[path = "kt222_pos_path.rs"]
mod pos_path;

mod kt222_pos_mod;

#[cfg(test)]
mod tests {
    #[allow(dead_code)]
    fn helper(m: &mut crate::Machine, t: crate::ObjId) {
        let _ = m.wake(t);
    }
}
"""

POS_PATH = """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn seven() -> u32 {
    7
}
"""

POS_MOD = """// SPDX-License-Identifier: MIT OR Apache-2.0
#[allow(dead_code)]
pub fn seven() -> u32 {
    7
}
"""

POS_ALLOW = """// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tệp **định nghĩa** một cửa `wake` (`WAKE_PRODUCER`) — dạng hợp lệ, phải XANH.
#[allow(dead_code)]
pub fn wake(_t: u32) {}
"""

POS_BUILD_DIR = """// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tệp trong thư mục build `v2/kernel/target/` — phải bị bỏ qua.
#[allow(dead_code)]
pub fn evil(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
"""


def spec() -> Dict[str, object]:
    return {
        "ten": "tám đối chứng dương KT-222",
        "files": {
            "v2/kernel/vifast-core/src/kt222_pos.rs": POS_MAIN,
            "v2/kernel/vifast-core/src/kt222_pos_path.rs": POS_PATH,
            "v2/kernel/vifast-core/src/kt222_pos/kt222_pos_mod.rs": POS_MOD,
            "v2/kernel/vifast-core/src/kt222_pos_allow.rs": POS_ALLOW,
            "v2/lib/kt222_pos.txt": "kt222 pos text\n",
            "v2/lib/kt222_pos.bin": "kt222 pos bytes\n",
            "v2/kernel/target/kt222_pos_build.rs": POS_BUILD_DIR,
        },
        "mods": ["kt222_pos", "kt222_pos_allow"],
        "lib_rs": KERNEL_LIB_RS,
        "anchor": KERNEL_ANCHOR,
        "cargo": ["--manifest-path", "v2/kernel/Cargo.toml", "-p", "vifast-core"],
        "unit": "kernel",
    }


def run(cmd: Sequence[str], timeout: float = 900.0) -> Tuple[int, str]:
    try:
        p = subprocess.run(
            cmd, cwd=str(ROOT), env=ENV, capture_output=True, text=True, timeout=timeout
        )
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except subprocess.TimeoutExpired:
        return 124, "TIMEOUT"


def reset(s: Dict[str, object]) -> None:
    if s["mods"]:
        run(["git", "checkout", "--", str(s["lib_rs"])])
    for rel in s["files"]:  # type: ignore[index]
        p = ROOT / rel
        if p.is_file():
            p.unlink()
    for rel in s["files"]:  # type: ignore[index]
        d = (ROOT / rel).parent
        while d != ROOT and d.is_dir() and not any(d.iterdir()):
            d.rmdir()
            d = d.parent


def apply(s: Dict[str, object]) -> None:
    for rel, content in s["files"].items():  # type: ignore[union-attr]
        p = ROOT / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    if s["mods"]:
        p = ROOT / str(s["lib_rs"])
        text = p.read_text(encoding="utf-8")
        add = "".join(f"\nmod {m};" for m in s["mods"])  # type: ignore[union-attr]
        assert str(s["anchor"]) in text, "không thấy mỏ neo trong lib.rs"
        p.write_text(text.replace(str(s["anchor"]), str(s["anchor"]) + add, 1), encoding="utf-8")


def dem_loi(text: str) -> Tuple[int, List[str]]:
    codes = re.findall(r"^E (\S+)", text, re.M)
    return len(codes), codes


def main(argv: Sequence[str]) -> int:
    global ROOT, LOGDIR
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=str(ROOT))
    args = ap.parse_args(argv)
    ROOT = Path(args.root).resolve()
    LOGDIR = ROOT / "v2/agent/JOURNAL/kt222-logs/pos"
    LOGDIR.mkdir(parents=True, exist_ok=True)

    s = spec()
    reset(s)
    apply(s)
    row: Dict[str, object] = {"ten": s["ten"]}

    rc, out = run(["cargo", "check"] + list(s["cargo"]))  # type: ignore[arg-type]
    (LOGDIR / "pos-cargo.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
    row["cargo_rc"] = rc
    print(f"cargo RC={rc}")

    for mode, extra in (("normal", []), ("strict", ["--strict"])):
        rc, out = run([sys.executable, "v2/tools/check-wake-paths.py"] + extra)
        (LOGDIR / f"pos-static-{mode}.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
        n, codes = dem_loi(out)
        row[f"static_{mode}_rc"] = rc
        row[f"static_{mode}_n"] = n
        row[f"static_{mode}_codes"] = sorted(set(codes))
        print(f"lớp 1 {mode}: RC={rc} errors={n} {sorted(set(codes))}")

    rc, out = run([sys.executable, "v2/tools/check-wake-depinfo.py", "--unit", str(s["unit"])])
    (LOGDIR / "pos-depinfo.log").write_text(f"RC={rc}\n{out}", encoding="utf-8")
    n, codes = dem_loi(out)
    row["depinfo_rc"] = rc
    row["depinfo_n"] = n
    row["depinfo_codes"] = sorted(set(codes))
    print(f"lớp 2: RC={rc} errors={n} {sorted(set(codes))}")

    reset(s)
    (LOGDIR / "bang-8-doi-chung.json").write_text(
        json.dumps(row, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
