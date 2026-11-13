#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Cổng "mọi khối mã trong tài liệu parse được" (QD §7.4) + fixture VT09 (ST-09).

Ba chế độ:
  * (mặc định)  kiểm fixture `test/doc-examples/doc-*` == khối trong `docs/viscript/*.md`
  * `--regen`    sinh lại toàn bộ fixture `doc-*` từ tài liệu
  * `--gate`     bước cổng: đếm khối/fixture + chạy `vis check` trên **mọi** khối
                 (`.vis`/`.vish`), đối chiếu `doc-examples-allow.txt`, rồi kiểm fixture
                 viết tay `qd-*` theo dòng `# kỳ vọng:` của chúng

Ngoại lệ phải khai báo **tường minh** ở `docs/viscript/doc-examples-allow.txt`
(mỗi dòng `<tệp>:<dòng mở khối> <lý do>`); ngoại lệ cũ (không còn đỏ) và khối đỏ
không khai báo đều làm cổng **đỏ**.

`--root DIR --temp-root` là cây tạm cho **case âm**: bỏ kiểm "khớp fixture"/"đúng 24 khối"/
"đúng 7 mục allowlist" nhưng **giữ** các luật (mỗi mục allowlist phải đỏ, khối đỏ phải khai báo,
đúng 12 fixture qd-*). `--root` **không** kèm `--temp-root` vẫn đủ luật (dùng khi soi gốc repo).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
DOCS = REPO / "docs" / "viscript"
FIXTURES = REPO / "viscript" / "test" / "doc-examples"
SPDX = "# SPDX-License-Identifier: MIT OR Apache-2.0"
FENCE = re.compile(r"^```(vis|vish-session)\s*$")
# Số do Agent 0 chốt cho VT09/QD §7.4 — ĐỔI là thay đổi ngưỡng, phải xin Agent 0.
# **8 mục (ST-27, Agent 0 APPROVED): khối 662 `extern cmd git(…, ...rest: str)` đã XANH** vì variadic
# có mặt (đúng §9.5) ⇒ bỏ khỏi allowlist; đây là **chặt hơn**, không nới ngưỡng (xem VD-60).
# **6 mục (ST-37, Agent 0 APPROVED `59a407c` + `260fe8a`, VD-70): HAI khối `vish-session` §9.7 nay XANH**
# — 1451 (`walk`/`where`/`sort_by`/`to json`/`save`) **và** 672 (`ls ./photos | where size > 5mb |
# sort_by modified --desc | take 10`, `VS0603 "take không có khe nhận error"` cũ) — vì lệnh lạ =
# **chương trình ngoài** (§9.3) và khe ống **hợp nhất** theo khả năng nhận (VD-69) ⇒ bỏ khỏi allowlist;
# **chặt hơn**, không nới ngưỡng. Mỗi mục còn lại vẫn phải **thật đỏ** khi `vis check`.
ALLOW_COUNT = 6
BLOCK_COUNT = 24
VISH_COUNT = 2
QD_COUNT = 12


@dataclass(frozen=True)
class Block:
    md: str  # tên tệp markdown (không đường dẫn)
    line: int  # dòng 1-based của hàng ``` mở khối
    ext: str  # "vis" | "vish"
    body: str

    @property
    def stem(self) -> str:
        return f"doc-{self.md[:-3]}-{self.line}.{self.ext}"


def md_blocks(docs: Path) -> list[Block]:
    out: list[Block] = []
    for md in sorted(docs.glob("*.md")):
        lines = md.read_text(encoding="utf-8").splitlines()
        i = 0
        while i < len(lines):
            m = FENCE.match(lines[i])
            if not m:
                i += 1
                continue
            ext = "vis" if m.group(1) == "vis" else "vish"
            start = i + 1  # 1-based
            body: list[str] = []
            i += 1
            while i < len(lines) and lines[i].strip() != "```":
                body.append(lines[i])
                i += 1
            out.append(Block(md.name, start, ext, "\n".join(body) + "\n"))
            i += 1
    return out


def load_allow(path: Path) -> list[tuple[str, int, str]]:
    out: list[tuple[str, int, str]] = []
    if not path.is_file():
        raise SystemExit(f"doc-examples: thiếu {path}")
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        head, _, reason = line.partition(" ")
        name, sep, num = head.partition(":")
        if not sep or not num.isdigit() or not reason.strip():
            raise SystemExit(f"doc-examples: dòng allowlist sai định dạng: {raw!r}")
        out.append((name, int(num), reason.strip()))
    return out


def find_cargo() -> str | None:
    """`cargo` có thể không nằm trên PATH (vis-dev tự dò `$HOME/.cargo/bin/cargo`)."""
    import shutil

    if shutil.which("cargo"):
        return "cargo"
    cand = Path.home() / ".cargo" / "bin" / "cargo"
    return str(cand) if cand.is_file() else None


def resolve_bin(vis_bin: str | None) -> Path | None:
    """Binary `vis` phải **mới hơn mọi .rs** trong `crates/` (không dùng bản cũ)."""
    path = Path(vis_bin) if vis_bin else REPO / "viscript" / "target" / "debug" / "vis"
    newer = [p.stat().st_mtime for p in (REPO / "viscript" / "crates").rglob("*.rs")]
    newest = max(newer) if newer else 0.0
    if not path.is_file():
        return None
    if path.stat().st_mtime < newest:
        if vis_bin:
            raise SystemExit(
                f"doc-examples: binary {path} cũ hơn mã nguồn — từ chối chạy (build lại trước)"
            )
        cargo = find_cargo()
        if cargo is None:
            raise SystemExit("doc-examples: không thấy cargo để build `vis`")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--manifest-path",
                str(REPO / "viscript" / "Cargo.toml"),
                "-p",
                "vis-cli",
                "--bin",
                "vis",
            ],
            capture_output=True,
            text=True,
        )
        if build.returncode != 0:
            raise SystemExit(f"doc-examples: build vis thất bại: {build.stderr.strip()}")
    return path


def vis_check(files: list[Path], vis_bin: str | None) -> dict[str, tuple[bool, str]]:
    """Chạy `vis check` một lần; trả {tên tệp: (ok, mã chẩn đoán)}.

    Mã chẩn đoán gán **theo từng tệp**: dòng `# VSxxxx` ngay sau `not ok` là của tệp đó.
    """
    if not files:
        return {}
    path = resolve_bin(vis_bin)
    if path is not None:
        cmd = [str(path)]
    else:
        cargo = find_cargo()
        if cargo is None:
            raise SystemExit("doc-examples: không thấy cargo để chạy `vis`")
        cmd = [
            cargo,
            "run",
            "-q",
            "--manifest-path",
            str(REPO / "viscript" / "Cargo.toml"),
            "-p",
            "vis-cli",
            "--bin",
            "vis",
            "--",
        ]
    proc = subprocess.run(
        # **ST-28**: `--syntax` — cổng này kiểm khối mã **parse** được (QD §7.4), không đòi
        # chúng là chương trình VS0 hợp kiểu; `vis check` mặc định nay kiểm **cả kiểu** (đồng bộ
        # với `vis run`, bug hai đường lệch nhau sửa ở ST-28).
        [*cmd, "check", "--syntax", *[str(f) for f in files]],
        capture_output=True,
        text=True,
    )
    res: dict[str, tuple[bool, str]] = {}
    pending: str | None = None
    for line in proc.stdout.splitlines():
        m = re.match(r"^(not ok|ok) \d+ - (.+)$", line)
        if m:
            name = Path(m.group(2)).name
            res[name] = (m.group(1) == "ok", "")
            pending = None if m.group(1) == "ok" else name
            continue
        d = re.match(r"^# (VS\d{4}) ", line)
        if d and pending:
            res[pending] = (False, d.group(1))
            pending = None
    return res


def check_fixtures(blocks: list[Block], fix: Path) -> list[str]:
    errs: list[str] = []
    want = {b.stem for b in blocks}
    have = {p.name for p in fix.glob("doc-*") if p.is_file()}
    for name in sorted(want - have):
        errs.append(f"thiếu fixture {name} (chạy --regen)")
    for name in sorted(have - want):
        errs.append(f"fixture thừa {name} (khối không còn trong tài liệu — chạy --regen)")
    for b in blocks:
        p = fix / b.stem
        if not p.is_file():
            continue
        text = p.read_text(encoding="utf-8")
        first, _, rest = text.partition("\n")
        if first != SPDX:
            errs.append(f"{b.stem}: dòng đầu phải là SPDX")
            continue
        if rest != b.body:
            errs.append(f"{b.stem}: nội dung KHÔNG khớp khối {b.md}:{b.line} (chạy --regen)")
    return errs


def regen(blocks: list[Block], fix: Path) -> None:
    fix.mkdir(parents=True, exist_ok=True)
    want = {b.stem for b in blocks}
    for b in blocks:
        (fix / b.stem).write_text(SPDX + "\n" + b.body, encoding="utf-8")
    for p in fix.glob("doc-*"):
        if p.name not in want:
            p.unlink()
    print(f"doc-examples: đã sinh {len(blocks)} fixture vào {fix}")


def gate(docs: Path, fix: Path, root: Path | None, temp_root: bool, vis_bin: str | None) -> int:
    blocks = md_blocks(docs)
    errs: list[str] = []
    if not blocks:
        errs.append("không thấy khối ```vis/```vish-session nào")
    if not temp_root:
        if len(blocks) != BLOCK_COUNT:
            errs.append(
                f"phải đúng {BLOCK_COUNT} khối mã trong tài liệu (QD §7.4), thật {len(blocks)}"
            )
        n_vish = sum(1 for b in blocks if b.ext == "vish")
        if n_vish != VISH_COUNT:
            errs.append(f"phải đúng {VISH_COUNT} khối vish-session, thật {n_vish}")
        errs.extend(check_fixtures(blocks, fix))
    allow_path = docs / "doc-examples-allow.txt"
    try:
        allow = load_allow(allow_path)
    except SystemExit as e:
        print(str(e), file=sys.stderr)
        return 1
    if not temp_root and len(allow) != ALLOW_COUNT:
        errs.append(
            f"allowlist phải đúng {ALLOW_COUNT} mục (QD §7.4/VD-36), thật {len(allow)}"
        )
    keys = {(b.md, b.line) for b in blocks}
    allow_keys: set[tuple[str, int]] = set()
    for name, line, _ in allow:
        if (name, line) not in keys:
            errs.append(f"allowlist: {name}:{line} không ứng với khối nào trong tài liệu")
        allow_keys.add((name, line))
    # fixture viết tay QD §7.1..7.3 — số lượng do Agent 0 chốt (VT09)
    qd = sorted(p for p in fix.glob("qd*") if p.suffix in (".vis", ".vish"))
    if len(qd) != QD_COUNT:
        errs.append(f"phải đúng {QD_COUNT} fixture qd-* (VT09), thật {len(qd)}")

    with tempfile.TemporaryDirectory(prefix="vis-docex-") as td:
        tmp = Path(td)
        paths: dict[Path, Block] = {}
        for b in blocks:
            p = tmp / f"{b.md[:-3]}-{b.line}.{b.ext}"
            p.write_text(b.body, encoding="utf-8")
            paths[p] = b
        status = vis_check(list(paths), vis_bin)
        for p, b in paths.items():
            ok, code = status.get(p.name, (False, "?"))
            key = (b.md, b.line)
            if key in allow_keys:
                if ok:
                    errs.append(f"ngoại lệ cũ: {b.md}:{b.line} nay đã XANH — bỏ khỏi allowlist")
            elif not ok:
                errs.append(f"khối đỏ không khai báo: {b.md}:{b.line} ({code})")
        qd_status = vis_check(qd, vis_bin)
        for p in qd:
            head = p.read_text(encoding="utf-8").splitlines()[:6]
            qd_line = next((l for l in head if l.startswith("# QD 7.")), "")
            want_line = next((l for l in head if l.startswith("# kỳ vọng:")), "")
            if not qd_line:
                errs.append(f"{p.name}: thiếu dòng `# QD 7.x: …`")
            if not want_line:
                errs.append(f"{p.name}: thiếu dòng `# kỳ vọng: ok|VSxxxx`")
                continue
            want = want_line.split(":", 1)[1].strip()
            ok, code = qd_status.get(p.name, (False, "?"))
            if want == "ok" and not ok:
                errs.append(f"{p.name}: kỳ vọng ok nhưng đỏ ({code})")
            elif want.startswith("VS") and (ok or code != want):
                errs.append(f"{p.name}: kỳ vọng {want} nhưng thật {('ok' if ok else code)}")
    if errs:
        for e in errs:
            print(f"doc-examples: {e}", file=sys.stderr)
        return 1
    ok_n = len(blocks) - len(allow)
    print(
        f"--- doc-examples: OK ({len(blocks)} khối tài liệu: {ok_n} xanh + {len(allow)} ngoại lệ đỏ đúng; "
        f"{len(qd)} fixture QD §7.1..7.3)"
    )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--regen", action="store_true")
    ap.add_argument("--gate", action="store_true")
    ap.add_argument("--root", type=Path, default=None, help="gốc thay thế (case âm)")
    ap.add_argument(
        "--temp-root", action="store_true", help="cây tạm: bỏ kiểm khớp fixture/đếm khối/đếm allowlist"
    )
    ap.add_argument("--vis-bin", default=None, help="binary vis thay thế (kiểm cả freshness)")
    args = ap.parse_args()
    root = args.root
    docs = (root / "docs" / "viscript") if root else DOCS
    fix = (root / "viscript" / "test" / "doc-examples") if root else FIXTURES
    if args.regen:
        regen(md_blocks(docs), fix)
        return 0
    if args.gate:
        return gate(docs, fix, root, args.temp_root, args.vis_bin)
    errs = check_fixtures(md_blocks(docs), fix)
    for e in errs:
        print(f"doc-examples: {e}", file=sys.stderr)
    if errs:
        return 1
    print("doc-examples: fixture KHỚP tài liệu")
    return 0


if __name__ == "__main__":
    sys.exit(main())
