#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""check-lockfile-sync.py — **B3** (KE-HOACH-KIEN-TRUC.md nút thắt 11): đồng bộ 3 `Cargo.lock`.

Vì sao có tệp này
-----------------
Repo có **ba** workspace Cargo độc lập:

  * `v2/Cargo.lock`        — workspace host/sim (`v2/Cargo.toml`, `exclude = ["kernel"]`);
  * `v2/kernel/Cargo.lock` — workspace kernel/no_std (`v2/kernel/Cargo.toml`);
  * `v2/user/Cargo.lock`   — workspace userland (`v2/user/Cargo.toml`, KT-404).

Ba workspace **dùng chung** nhiều crate nội bộ (`vifast-core`, `vifast-abi`, `vifast-pte`,
`vifast-ring`, `vifast-virtio`, …) và crate ngoài (registry). Không có gì bắt ba lockfile phải
cùng phiên bản: sửa `Cargo.toml` một bên rồi `cargo update` bên đó là ba bên trôi khỏi nhau **âm
thầm** — sim/oracle chạy một phiên bản, kernel chạy phiên bản khác, chương trình U-mode chạy
phiên bản thứ ba, và mọi phép so parity trở nên vô nghĩa mà cổng vẫn xanh. Đây đúng là nút thắt
**B3** của kế hoạch đại tu.

**KT-404c** — phủ lockfile **thứ ba** (`v2/user`): workspace `v2/user/` của KT-404 trước đó
không có cổng nào bắt nó đồng bộ với hai workspace kia. Các ca `--selftest` cũ giữ nguyên.

Luật (fail-closed)
-----------------
1. **Cả ba** lockfile phải tồn tại và parse được; thiếu/rỗng/hỏng ⇒ **ĐỎ** (không bỏ qua).
2. Với **mọi cặp** lockfile và **mọi tên gói** xuất hiện ở **cả hai** bên của cặp: **tập phiên
   bản phải bằng nhau**. Kiểm **cả ba cặp** (v2↔kernel, v2↔user, kernel↔user): chỉ so với
   `v2/Cargo.lock` sẽ bỏ lọt cặp kernel↔user. Một lockfile có thể chứa nhiều phiên bản của cùng
   một crate (semver-major kép) — so **tập**, không so một phần tử.
3. Với mỗi cặp `(tên, phiên bản)` dùng chung: **`source` phải bằng nhau** (registry vs path vs
   git là ba nguồn khác nhau; cùng số phiên bản nhưng khác nguồn vẫn là trôi).
4. Gói chỉ có ở **một** bên **không** phải lỗi: mỗi workspace có phụ thuộc riêng là hợp lệ.

Chạy
----
    python3 v2/tools/check-lockfile-sync.py [--root REPO_ROOT] [--verbose]
    python3 v2/tools/check-lockfile-sync.py --selftest     # 14 ca, thuần tệp tạm, không cargo

Exit: 0 = xanh; 1 = lệch/thiếu/hỏng; 2 = dùng sai.

**Trạng thái**: nối vào `./v2/bin/v2 check` (bước `B3`, KT-407; mở rộng 3 lockfile ở KT-404c).
Nhật ký: `v2/agent/JOURNAL/kt407-tooling.md`, `v2/agent/JOURNAL/kt404c-lock-buildrs.md`.
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from pathlib import Path
from typing import Dict, List, Optional, Tuple

# Ba lockfile của repo, tương đối với gốc repo (`KT-404c` thêm `v2/user`).
LOCKFILES = ("v2/Cargo.lock", "v2/kernel/Cargo.lock", "v2/user/Cargo.lock")

RE_PACKAGE = re.compile(r"^\[\[package\]\]\s*$", re.M)
RE_NAME = re.compile(r'^name\s*=\s*"([^"]+)"', re.M)
RE_VERSION = re.compile(r'^version\s*=\s*"([^"]+)"', re.M)
RE_SOURCE = re.compile(r'^source\s*=\s*"([^"]+)"', re.M)


class LockError(Exception):
    """Lockfile thiếu/hỏng — luôn là ĐỎ (fail-closed), không phải 'bỏ qua'."""


def parse_lockfile(path: Path) -> Dict[str, List[Tuple[str, Optional[str]]]]:
    """Trả `{tên: [(phiên bản, source|None), …]}`.

    Parser tối giản cho định dạng `Cargo.lock` v3/v4: tách theo dòng `[[package]]` rồi đọc ba
    trường `name`/`version`/`source` của mỗi khối. Không cần `tomllib` ⇒ chạy được cả `python3`
    3.9 của máy đo lẫn `python3.12` của cổng.
    """
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise LockError("không đọc được %s: %s" % (path, exc)) from exc
    blocks = RE_PACKAGE.split(text)
    if len(blocks) < 2:
        raise LockError("%s không có khối `[[package]]` nào (rỗng/hỏng?)" % path)
    pkgs: Dict[str, List[Tuple[str, Optional[str]]]] = {}
    for block in blocks[1:]:
        m_name = RE_NAME.search(block)
        m_ver = RE_VERSION.search(block)
        if not m_name or not m_ver:
            raise LockError("%s: khối `[[package]]` thiếu name/version" % path)
        m_src = RE_SOURCE.search(block)
        pkgs.setdefault(m_name.group(1), []).append(
            (m_ver.group(1), m_src.group(1) if m_src else None)
        )
    return pkgs


def compare(
    a: Dict[str, List[Tuple[str, Optional[str]]]],
    b: Dict[str, List[Tuple[str, Optional[str]]]],
    name_a: str,
    name_b: str,
) -> Tuple[List[str], int]:
    """So hai bản đồ gói. Trả `(danh sách lỗi, số tên dùng chung)`."""
    errors: List[str] = []
    shared = sorted(set(a) & set(b))
    for name in shared:
        va = sorted(v for v, _ in a[name])
        vb = sorted(v for v, _ in b[name])
        if va != vb:
            errors.append(
                "%s: %s=%s vs %s=%s" % (name, name_a, va, name_b, vb)
            )
            continue
        # Cùng tập phiên bản ⇒ so nguồn theo từng phiên bản.
        src_a = {v: s for v, s in a[name]}
        src_b = {v: s for v, s in b[name]}
        for ver in va:
            if src_a.get(ver) != src_b.get(ver):
                errors.append(
                    "%s %s: source %s=%r vs %s=%r"
                    % (name, ver, name_a, src_a.get(ver), name_b, src_b.get(ver))
                )
    return errors, len(shared)


def run_check(root: Path, verbose: bool) -> int:
    maps: List[Dict[str, List[Tuple[str, Optional[str]]]]] = []
    for rel in LOCKFILES:
        path = root / rel
        if not path.is_file():
            print("check-lockfile-sync: LỖI thiếu %s" % path, file=sys.stderr)
            return 1
        try:
            maps.append(parse_lockfile(path))
        except LockError as exc:
            print("check-lockfile-sync: LỖI %s" % exc, file=sys.stderr)
            return 1
    print(
        "check-lockfile-sync: %s"
        % ", ".join("%s=%d gói" % (rel, len(m)) for rel, m in zip(LOCKFILES, maps))
    )
    # **KT-404c**: so **cả ba cặp** — chỉ so với `v2/Cargo.lock` sẽ bỏ lọt cặp kernel↔user.
    errors: List[str] = []
    shared: List[Tuple[str, str, int]] = []
    for i in range(len(LOCKFILES)):
        for j in range(i + 1, len(LOCKFILES)):
            pair_errors, count = compare(maps[i], maps[j], LOCKFILES[i], LOCKFILES[j])
            errors.extend(pair_errors)
            shared.append((LOCKFILES[i], LOCKFILES[j], count))
    if verbose:
        for i in range(len(LOCKFILES)):
            for j in range(i + 1, len(LOCKFILES)):
                names = sorted(set(maps[i]) & set(maps[j]))
                print("  %s ↔ %s: %d crate dùng chung" % (LOCKFILES[i], LOCKFILES[j], len(names)))
                for name in names:
                    print("  %-24s %s" % (name, sorted(v for v, _ in maps[i][name])))
    if errors:
        for err in errors:
            print("check-lockfile-sync: LỆCH %s" % err, file=sys.stderr)
        print(
            "check-lockfile-sync: %d lỗi, 0 cảnh báo — các Cargo.lock trôi khỏi nhau (B3)"
            % len(errors),
            file=sys.stderr,
        )
        return 1
    print(
        "check-lockfile-sync: OK — %d/%d cặp cùng phiên bản + cùng nguồn (B3), dùng chung %s"
        % (
            len(shared),
            len(shared),
            ", ".join("%s↔%s=%d" % (a, b, c) for a, b, c in shared),
        )
    )
    return 0


# ------------------------------------------------------------------ selftest

def _write(path: Path, packages: List[Tuple[str, str, Optional[str]]]) -> None:
    lines = [
        "# This file is automatically @generated by Cargo.",
        "# It is not intended for manual editing.",
        "version = 4",
        "",
    ]
    for name, ver, src in packages:
        lines.append("[[package]]")
        lines.append('name = "%s"' % name)
        lines.append('version = "%s"' % ver)
        if src is not None:
            lines.append('source = "%s"' % src)
        lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def _case(
    tmp: Path,
    tag: str,
    a_pkgs: List[Tuple[str, str, Optional[str]]],
    b_pkgs: List[Tuple[str, str, Optional[str]]],
    expect_rc: int,
    write_a: bool = True,
    write_b: bool = True,
    c_pkgs: Optional[List[Tuple[str, str, Optional[str]]]] = None,
    write_c: bool = True,
) -> Tuple[bool, str]:
    root = tmp / tag
    (root / "v2" / "kernel").mkdir(parents=True, exist_ok=True)
    (root / "v2" / "user").mkdir(parents=True, exist_ok=True)
    if write_a:
        _write(root / "v2" / "Cargo.lock", a_pkgs)
    if write_b:
        _write(root / "v2" / "kernel" / "Cargo.lock", b_pkgs)
    if write_c:
        # Mặc định: lockfile thứ ba **đồng bộ với `v2/Cargo.lock`** ⇒ ca cũ giữ nguyên ý nghĩa
        # (chỉ đo quan hệ a↔b), không bị nhiễu bởi `v2/user`.
        _write(root / "v2" / "user" / "Cargo.lock", a_pkgs if c_pkgs is None else c_pkgs)
    import contextlib
    import io

    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = run_check(root, verbose=False)
    ok = rc == expect_rc
    detail = "rc=%d (kỳ vọng %d)" % (rc, expect_rc)
    if not ok:
        detail += " | stdout=%r stderr=%r" % (out.getvalue().strip(), err.getvalue().strip())
    return ok, detail


def run_selftest() -> int:
    cases = 0
    oks = 0

    def record(name: str, ok: bool, detail: str) -> None:
        nonlocal cases, oks
        cases += 1
        if ok:
            oks += 1
            print("ok %d - %s: %s" % (cases, name, detail))
        else:
            print("not ok %d - %s: %s" % (cases, name, detail))

    with tempfile.TemporaryDirectory(prefix="kt407-b3-") as td:
        tmp = Path(td)
        base = [("vifast-core", "0.1.0", None), ("vifast-abi", "0.1.0", None)]

        record(
            "cùng phiên bản ⇒ XANH",
            *_case(tmp, "same", base, list(base), 0),
        )
        record(
            "lệch phiên bản crate dùng chung ⇒ ĐỎ",
            *_case(
                tmp,
                "version-drift",
                base,
                [("vifast-core", "0.2.0", None), ("vifast-abi", "0.1.0", None)],
                1,
            ),
        )
        record(
            "cùng phiên bản nhưng khác source ⇒ ĐỎ",
            *_case(
                tmp,
                "source-drift",
                [("serde", "1.0.0", "registry+https://github.com/rust-lang/crates.io-index")],
                [("serde", "1.0.0", "git+https://example.invalid/serde")],
                1,
            ),
        )
        record(
            "thiếu v2/kernel/Cargo.lock ⇒ ĐỎ (fail-closed)",
            *_case(tmp, "missing-kernel", base, base, 1, write_b=False),
        )
        record(
            "thiếu v2/Cargo.lock ⇒ ĐỎ (fail-closed)",
            *_case(tmp, "missing-v2", base, base, 1, write_a=False),
        )
        record(
            "lockfile rỗng ⇒ ĐỎ (fail-closed)",
            *_case(tmp, "empty", [], base, 1),
        )
        record(
            "gói chỉ có một bên ⇒ XANH (không phải dùng chung)",
            *_case(
                tmp,
                "one-side",
                base + [("only-host", "1.0.0", None)],
                base + [("only-kernel", "2.0.0", None)],
                0,
            ),
        )
        record(
            "hai phiên bản cùng tên, hai bên cùng tập ⇒ XANH",
            *_case(
                tmp,
                "multi-same",
                [("hashbrown", "0.14.0", None), ("hashbrown", "0.15.0", None)],
                [("hashbrown", "0.15.0", None), ("hashbrown", "0.14.0", None)],
                0,
            ),
        )
        record(
            "hai phiên bản cùng tên, hai bên khác tập ⇒ ĐỎ",
            *_case(
                tmp,
                "multi-drift",
                [("hashbrown", "0.14.0", None), ("hashbrown", "0.15.0", None)],
                [("hashbrown", "0.14.0", None), ("hashbrown", "0.16.0", None)],
                1,
            ),
        )
        record(
            "khối [[package]] thiếu version ⇒ ĐỎ (không parse mù)",
            *_broken_case(tmp),
        )
        # ---- KT-404c: lockfile **thứ ba** `v2/user/Cargo.lock` (workspace `v2/user/` của KT-404).
        # Bốn ca dưới đây là **đỏ-trước**: trước KT-404c công cụ chỉ đọc hai lockfile nên ba ca
        # "phải ĐỎ" trả 0 ⇒ `not ok`. Giữ nguyên các ca cũ phía trên (cùng ý nghĩa, cùng kỳ vọng).
        record(
            "lockfile thứ ba (v2/user) lệch phiên bản ⇒ ĐỎ",
            *_case(
                tmp,
                "third-drift",
                base,
                base,
                1,
                c_pkgs=[("vifast-core", "0.3.0", None), ("vifast-abi", "0.1.0", None)],
            ),
        )
        record(
            "thiếu v2/user/Cargo.lock ⇒ ĐỎ (fail-closed)",
            *_case(tmp, "missing-user", base, base, 1, write_c=False),
        )
        record(
            "gói chỉ có ở v2/user ⇒ XANH (không phải dùng chung)",
            *_case(
                tmp,
                "third-only",
                base,
                base,
                0,
                c_pkgs=base + [("vifast-urt", "0.1.0", None)],
            ),
        )
        record(
            "v2/user cùng phiên bản nhưng khác source ⇒ ĐỎ",
            *_case(
                tmp,
                "third-source",
                base,
                base,
                1,
                c_pkgs=[
                    ("vifast-core", "0.1.0", "git+https://example.invalid/core"),
                    ("vifast-abi", "0.1.0", None),
                ],
            ),
        )

    print("check-lockfile-sync: selftest %d/%d ok" % (oks, cases))
    return 0 if oks == cases else 1


def _broken_case(tmp: Path) -> Tuple[bool, str]:
    root = tmp / "broken"
    (root / "v2" / "kernel").mkdir(parents=True, exist_ok=True)
    (root / "v2" / "user").mkdir(parents=True, exist_ok=True)
    _write(root / "v2" / "Cargo.lock", [("vifast-core", "0.1.0", None)])
    _write(root / "v2" / "user" / "Cargo.lock", [("vifast-core", "0.1.0", None)])
    (root / "v2" / "kernel" / "Cargo.lock").write_text(
        "version = 4\n\n[[package]]\nname = \"vifast-core\"\n", encoding="utf-8"
    )
    import contextlib
    import io

    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = run_check(root, verbose=False)
    ok = rc == 1
    detail = "rc=%d (kỳ vọng 1)" % rc
    if not ok:
        detail += " | stdout=%r stderr=%r" % (out.getvalue().strip(), err.getvalue().strip())
    return ok, detail


# ------------------------------------------------------------------ main

def main(argv: List[str]) -> int:
    ap = argparse.ArgumentParser(
        description="B3 — đồng bộ 2 Cargo.lock (cùng phiên bản + nguồn cho crate dùng chung)"
    )
    ap.add_argument(
        "--root",
        default=str(Path(__file__).resolve().parents[2]),
        help="gốc repo (mặc định: hai cấp trên tệp này)",
    )
    ap.add_argument("--selftest", action="store_true", help="chạy 10 ca tự kiểm, không đọc cây thật")
    ap.add_argument("--verbose", action="store_true", help="in từng crate dùng chung")
    args = ap.parse_args(argv)

    if args.selftest:
        return run_selftest()

    root = Path(args.root).resolve()
    if not (root / "v2").is_dir():
        print("check-lockfile-sync: không thấy %s/v2" % root, file=sys.stderr)
        return 2
    return run_check(root, args.verbose)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
