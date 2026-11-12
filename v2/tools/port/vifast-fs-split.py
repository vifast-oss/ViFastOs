# SPDX-License-Identifier: MIT OR Apache-2.0
# Công cụ một lần: chia crates/vifast-fs/src/lib.rs (v1, đóng băng) thành các tệp <= 1500 dòng
# của v2/lib/vifast-fs/src/ (D2-26, D2-28). Không sửa v1.
#!/usr/bin/env python3
"""Chia crates/vifast-fs/src/lib.rs (v1, đóng băng) thành các tệp v2 <= 1500 dòng.

Chỉ đọc v1, ghi sang v2/lib/vifast-fs/src/. Không sửa v1.
"""
import re
import pathlib

REPO = pathlib.Path(__file__).resolve().parents[3]
SRC = REPO / "crates/vifast-fs/src/lib.rs"
OUT = REPO / "v2/lib/vifast-fs/src"

lines = SRC.read_text().splitlines(keepends=True)


def rng(a: int, b: int) -> str:
    """1-based inclusive line range (giữ nguyên nội dung)."""
    return "".join(lines[a - 1:b])


def publish(code: str) -> str:
    """Thêm pub(crate) cho fn private; script strip_vis.py gỡ lại ở item của trait."""
    out = []
    for line in code.splitlines(keepends=True):
        if re.match(r"^fn ", line):
            line = "pub(crate) " + line
        elif re.match(r"^    fn ", line):
            line = "    pub(crate) " + line.lstrip()
        elif re.match(r"^const fn ", line):
            line = "pub(crate) " + line
        elif re.match(r"^static ", line):
            line = "pub(crate) " + line
        elif re.match(r"^const ", line):
            line = "pub(crate) " + line
        out.append(line)
    return "".join(out)


HDR = (
    "// SPDX-License-Identifier: MIT OR Apache-2.0\n"
    "// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5\n"
)


def write(rel: str, body: str) -> None:
    path = OUT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(HDR + body)
    n = len((HDR + body).splitlines())
    assert n <= 1500, f"{rel} có {n} dòng > 1500"


IMPL = "\nimpl<D: BlockDev> Fs<D> {\n"


def public_types(code: str) -> str:
    return code.replace("\nstruct SuperInfo {", "\npub(crate) struct SuperInfo {")


def publish_fields(code: str) -> str:
    """Field của `Fs` phải pub(crate) vì impl được tách sang tệp khác."""
    start = code.index("pub struct Fs<D: BlockDev> {")
    end = code.index("\n}\n", start)
    head, body, tail = code[:start], code[start:end], code[end:]
    out = []
    for line in body.splitlines(keepends=True):
        if re.match(r"^    [a-z_][a-z0-9_]*: ", line):
            line = "    pub(crate) " + line.lstrip()
        out.append(line)
    return head + "".join(out) + tail

# --- consts.rs: hằng số ---
write("consts.rs", rng(12, 49))

# --- fs.rs: types + block cache + crc + encode + format + phần lõi Fs (tới hết replay) ---
crc_cont = publish(rng(2791, 2799))
write(
    "fs.rs",
    publish_fields(
        public_types("use crate::consts::*;\n\n" + publish(rng(51, 1274))) + "\n}\n\n" + crc_cont
    ),
)

# --- tx.rs: stage/commit/replay helpers + fsync/inject + crc32_continue + impl Inode ---
tx1 = publish(rng(1276, 1399))
tx2 = publish(rng(2695, 2788))
tx3 = rng(2800, 2852)
tx3 = tx3.replace(
    "\n#[cfg(test)]\nfn crc32_bitwise", "\n#[cfg(test)]\npub(crate) fn crc32_bitwise"
)
tx3 = tx3.replace(
    "\n#[cfg(test)]\nfn crc32_continue_bitwise",
    "\n#[cfg(test)]\npub(crate) fn crc32_continue_bitwise",
)
write(
    "tx.rs",
    "use crate::consts::*;\nuse crate::fs::*;\n"
    + IMPL
    + tx1
    + "\n}\n"
    + IMPL
    + tx2
    + "\n}\n"
    + tx3,
)

# --- alloc.rs ---
write(
    "alloc.rs",
    "use crate::consts::*;\nuse crate::fs::*;\n" + IMPL + publish(rng(1401, 1637)) + "\n}\n",
)

# --- path.rs ---
write(
    "path.rs",
    "use crate::consts::*;\nuse crate::fs::*;\n" + IMPL + publish(rng(1638, 1926)) + "\n}\n",
)

# --- dir.rs ---
write(
    "dir.rs",
    "use crate::consts::*;\nuse crate::fs::*;\nuse crate::tx::*;\n" + IMPL + publish(rng(1927, 2694)) + "\n}\n",
)

# --- tests ---
tmod = rng(2856, 3002)
tmod = tmod.replace(
    "use super::*;",
    "use crate::*;\n"
    "    use crate::consts::*;\n"
    "    use crate::fs::*;\n"
    "    "
    "    use crate::alloc::*;\n"
    "    use crate::path::*;\n"
    "    use crate::dir::*;",
)
tmod = tmod.replace("\n    struct MemDev {", "\n    pub(crate) struct MemDev {")
tmod = tmod.replace("\n    struct CountingDev {", "\n    pub(crate) struct CountingDev {")
tmod = re.sub(r"\n        fn ", "\n        pub(crate) fn ", tmod)
tmod = re.sub(r"\n    fn ", "\n    pub(crate) fn ", tmod)
tmod += "\n    mod part1;\n    mod part2;\n"
write("tests/mod.rs", tmod)

part_hdr = (
    "use super::*;\n"
    "use super::std;\n"
    "use crate::*;\n"
    "use crate::consts::*;\n"
    "use crate::fs::*;\n"
    "use crate::tx::*;\n"
    "use crate::alloc::*;\n"
    "use crate::path::*;\n"
    "use crate::dir::*;\n"
)
write("tests/part1.rs", part_hdr + rng(3003, 3566))
write("tests/part2.rs", part_hdr + rng(3567, 4542))

print("OK — đã ghi", sorted(p.name for p in OUT.rglob("*.rs")))
