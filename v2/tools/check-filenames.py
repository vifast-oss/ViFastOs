#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-331 / D2-230 — cổng kiểm **tên tệp/thư mục tương thích Windows/NTFS**.

Vì sao có tệp này
-----------------
Agent 0 (D2-229/D2-230) chốt: 4 tệp trong `v2/agent/JOURNAL/kt219-logs/cases/` có `:`/`>`
làm **gãy `git clone` trên Windows** — NTFS không cho các ký tự đó trong tên. Đổi tên 4 tệp
chỉ chữa triệu chứng; không có máy kiểm thì sự cố tái diễn ở lần soak sau. Tệp này là máy
kiểm đó (đỏ-trước theo D2-05: `--selftest` dựng ca âm **thật** rồi đòi ĐỎ **đúng mã**).

Kiểm **mọi thành phần** của đường dẫn (tên thư mục lẫn tên tệp), theo đúng giới hạn NTFS:

    E1_NTFS_EMPTY     tên rỗng (thành phần đường dẫn rỗng)
    E1_NTFS_CHAR      ký tự cấm trong tên: `[<>:"|?*]`, hoặc ký tự điều khiển `\\x00-\\x1f`
    E1_NTFS_DEVICE    tên thiết bị Windows dành riêng: `CON`, `PRN`, `AUX`, `NUL`,
                      `COM1..COM9`, `LPT1..LPT9` — **kể cả khi có đuôi** (`CON.txt`),
                      không phân biệt hoa/thường
    E1_NTFS_TRAILING  tên kết thúc bằng dấu chấm hoặc dấu cách
    E1_NTFS_LEN       một thành phần dài hơn **255 byte** UTF-8

Phạm vi: chỉ **tên tệp đã theo dõi Git** (`git ls-files`) — **không** quét toàn đĩa (JOURNAL
có ~9.000 tệp; quét đĩa sẽ chậm và bắt cả `target/`, ổ đĩa của người khác). Chế độ
`--staged` chỉ kiểm các đường dẫn **đang được stage** (thêm/sửa/đổi tên đích) để hook
`pre-commit` chặn tên xấu **trước khi** vào lịch sử — và **không** chặn oan commit khác khi
một tên xấu cũ còn sót (việc đổi tên tệp tracked là của người sửa, không phải của hook).

Dùng
----
    python3 v2/tools/check-filenames.py                 # toàn bộ tệp tracked của repo
    python3 v2/tools/check-filenames.py --root D        # repo ở D
    python3 v2/tools/check-filenames.py --staged        # chỉ tên đang stage (hook pre-commit)
    python3 v2/tools/check-filenames.py --selftest      # ca âm ĐỎ đúng mã + ca dương XANH

Exit: 0 = xanh, 1 = có vi phạm, 2 = dùng sai / không chứng minh được.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import List, Optional, Sequence, Tuple

HERE = Path(__file__).resolve()
# v2/tools/check-filenames.py → gốc repo. Giống khuôn các checker cùng thư mục.
DEFAULT_ROOT = HERE.parent.parent.parent

# Ký tự NTFS không cho phép trong **tên** (không tính `/` — dấu phân cách đường dẫn) và toàn
# bộ ký tự điều khiển ASCII. `\\` không nằm trong danh sách: nó là dấu phân cách của Windows
# nên không thể xuất hiện trong tên tệp Git (Git dùng `/`), không cần kiểm.
BAD_CHARS = frozenset('<>:"|?*')
MAX_BYTES = 255

# Tên **thiết bị** Windows dành riêng — NTFS từ chối cả tên có đuôi (`CON.txt`, `LPT9.log`).
DEVICE_NAMES = frozenset(
    {"CON", "PRN", "AUX", "NUL"}
    | {f"COM{i}" for i in range(1, 10)}
    | {f"LPT{i}" for i in range(1, 10)}
)

CODES = (
    "E1_NTFS_EMPTY",
    "E1_NTFS_CHAR",
    "E1_NTFS_DEVICE",
    "E1_NTFS_TRAILING",
    "E1_NTFS_LEN",
)


# ---------------------------------------------------------------- phép kiểm thuần (test được)
def check_component(name: str) -> Optional[Tuple[str, str]]:
    """Kiểm **một** thành phần đường dẫn. Trả (mã, lý do) của vi phạm **đầu tiên** hoặc None.

    Thứ tự kiểm cố định (EMPTY → CHAR → DEVICE → TRAILING → LEN) để đầu ra ổn định: cùng một
    tên luôn ra cùng một mã, không phụ thuộc thứ tự kiểm của người viết.
    """
    if name == "":
        return ("E1_NTFS_EMPTY", "tên rỗng")

    for ch in name:
        if ch in BAD_CHARS:
            return (
                "E1_NTFS_CHAR",
                f"ký tự {ch!r} (U+{ord(ch):04X}) không hợp lệ trên Windows/NTFS",
            )
        if ord(ch) < 0x20:
            return (
                "E1_NTFS_CHAR",
                f"ký tự điều khiển (U+{ord(ch):04X}) không hợp lệ trên Windows/NTFS",
            )

    # `CON`, `con`, `CON.txt`, `CON.tar.gz` — NTFS dành riêng theo **phần trước dấu chấm đầu**.
    if name.split(".", 1)[0].upper() in DEVICE_NAMES:
        return ("E1_NTFS_DEVICE", f"tên thiết bị Windows dành riêng ({name.split('.', 1)[0]!r})")

    if name.endswith("."):
        return ("E1_NTFS_TRAILING", "kết thúc bằng dấu chấm (NTFS tự bỏ ⇒ tên khác trên Windows)")
    if name.endswith(" "):
        return ("E1_NTFS_TRAILING", "kết thúc bằng dấu cách (NTFS tự bỏ ⇒ tên khác trên Windows)")

    n = len(name.encode("utf-8", "surrogateescape"))
    if n > MAX_BYTES:
        return ("E1_NTFS_LEN", f"{n} byte UTF-8 > {MAX_BYTES} byte")
    return None


def check_path(path: str) -> List[Tuple[str, str, str]]:
    """Kiểm mọi thành phần của một đường dẫn tương đối kiểu Git. Trả [(mã, path, lý do)].

    Dùng `split("/")` **nguyên văn** (không `normpath`): một thành phần rỗng là vi phạm thật
    (`E1_NTFS_EMPTY`), không được lặng lẽ bỏ qua.
    """
    out: List[Tuple[str, str, str]] = []
    for part in path.split("/"):
        bad = check_component(part)
        if bad is not None:
            out.append((bad[0], path, bad[1]))
            break
    return out


# ---------------------------------------------------------------- nguồn tên (git)
def git_out(root: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def _split_z(raw: bytes) -> List[str]:
    return [p.decode("utf-8", "surrogateescape") for p in raw.split(b"\0") if p]


def tracked_names(root: Path) -> List[str]:
    """Tên **đã theo dõi Git** (D2-230): nguồn sự thật duy nhất — không quét đĩa."""
    return _split_z(git_out(root, "ls-files", "-z"))


def staged_names(root: Path) -> List[str]:
    """Tên **đang stage** (thêm/giữ-nguyên/sửa/đổi tên đích).

    `--no-renames` là **bắt buộc**: đổi tên tệp xấu → tốt phải hiện thành xoá + thêm, nếu bật
    dò đổi tên thì đường dẫn **cũ** (xấu) cũng nằm trong diff và hook sẽ chặn chính bản sửa.
    `--diff-filter=ACMR` bỏ qua tệp bị xoá thuần.
    """
    return _split_z(
        git_out(
            root,
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=ACMR",
        )
    )


# ---------------------------------------------------------------- đầu ra
def emit(line: str) -> None:
    # Tên tệp có thể chứa byte không phải UTF-8 (surrogateescape) — ghi thẳng ra bytes để
    # cổng không chết vì UnicodeEncodeError, đúng tên như Git đã ghi.
    sys.stdout.buffer.write((line + "\n").encode("utf-8", "surrogateescape"))


def run_check(root: Path, staged: bool) -> int:
    if not root.is_dir():
        print(f"check-filenames: không có thư mục {root}", file=sys.stderr)
        return 2
    try:
        inside = git_out(root, "rev-parse", "--is-inside-work-tree").strip()
    except (OSError, subprocess.CalledProcessError):
        print(f"check-filenames: {root} không phải git worktree", file=sys.stderr)
        return 2
    if inside != b"true":
        print(f"check-filenames: {root} không phải git worktree", file=sys.stderr)
        return 2

    mode = "staged" if staged else "tracked"
    try:
        names = staged_names(root) if staged else tracked_names(root)
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"check-filenames: git lỗi ({exc})", file=sys.stderr)
        return 2

    bad = 0
    for path in names:
        for code, where, reason in check_path(path):
            emit(f"{code} {where}: {reason}")
            bad += 1

    label = "E1 tên tệp NTFS"
    if staged:
        label += " (staged)"
    if bad:
        emit(f"{label}: FAIL ({bad} vi phạm / {len(names)} tên {mode})")
        return 1
    emit(f"{label}: PASS ({len(names)} tên {mode})")
    return 0


# ---------------------------------------------------------------- tự kiểm (ca âm ĐỎ đúng mã)
class _TAP:
    def __init__(self) -> None:
        self.total = 0
        self.fail = 0

    def ok(self, cond: bool, name: str, detail: str = "") -> None:
        self.total += 1
        if cond:
            emit(f"ok {self.total} - {name}")
        else:
            self.fail += 1
            emit(f"not ok {self.total} - {name}" + (f" ({detail})" if detail else ""))

    def done(self) -> int:
        emit(f"# {self.total - self.fail}/{self.total} ca đúng kỳ vọng")
        return 0 if self.fail == 0 else 1


def _git(repo: Path, *args: str, env: Optional[dict] = None) -> subprocess.CompletedProcess:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    return proc


def _mk_repo(repo: Path) -> Path:
    repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    # Danh tính cục bộ: không đọc/ghi cấu hình git toàn cục của máy đang chạy.
    _git(repo, "config", "user.name", "KT-331 selftest")
    _git(repo, "config", "user.email", "kt331-selftest@example.invalid")
    _git(repo, "config", "commit.gpgsign", "false")
    return repo


def _write(repo: Path, rel: str, data: str = "x\n") -> None:
    p = repo / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(data, encoding="utf-8")


def _run_cli(root: Path, *extra: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(HERE), "--root", str(root), *extra],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def selftest() -> int:
    tap = _TAP()

    # ---- 1..9: phép kiểm thuần, một ca cho **mỗi mã lỗi** (D2-05: đỏ-trước, đúng mã).
    def want_code(name: str, code: str, note: str) -> None:
        got = check_component(name)
        tap.ok(
            got is not None and got[0] == code,
            f"{code}: {note}",
            f"tên {name!r} cho {got!r}, muốn {code}",
        )

    want_code("A1_cfg(feature=ktest)_inline:cargo.log", "E1_NTFS_CHAR", "dấu ':' trong tên tệp")
    want_code("dir>name", "E1_NTFS_CHAR", "dấu '>' trong tên thư mục")
    want_code("CON.txt", "E1_NTFS_DEVICE", "thiết bị Windows có đuôi tệp")
    want_code("lpt9", "E1_NTFS_DEVICE", "thiết bị Windows chữ thường")
    want_code("ten.txt ", "E1_NTFS_TRAILING", "dấu cách ở cuối tên")
    want_code("ten.", "E1_NTFS_TRAILING", "dấu chấm ở cuối tên")
    want_code("a" * 252 + ".txt", "E1_NTFS_LEN", "256 byte > 255 byte")
    want_code("", "E1_NTFS_EMPTY", "tên rỗng")
    want_code("\x1f", "E1_NTFS_CHAR", "ký tự điều khiển U+001F")

    # Ca dương của phép kiểm thuần: tên thật trong repo (Unicode, ngoặc, dấu chấm giữa) phải xanh.
    good_names = [
        "A10_cfg(feature=ktest)_inline.cargo.log",
        "A18_path_Unicode_đường_dẫn.json",
        "kt219-log_có-dấu_cách trong tên.md",
        "CONSOLE.txt",  # `CONSOLE` **không** phải `CON`
        "COM10.txt",  # `COM10` **không** phải `COM1`
        "NUL_đuôi.txt",
    ]
    got = [(n, check_component(n)) for n in good_names]
    tap.ok(
        all(v is None for _, v in got),
        "ca dương: tên hợp lệ (Unicode/ngoặc/COM10/CONSOLE) ⇒ XANH",
        f"bị bắt oan: {[n for n, v in got if v]}",
    )

    # ---- 10..15: end-to-end qua Git thật: tệp **tracked** xấu ⇒ CLI ĐỎ **đúng mã** cho từng tệp.
    with tempfile.TemporaryDirectory(prefix="kt331-selftest-") as td:
        tmp = Path(td)
        repo = _mk_repo(tmp / "repo")
        _write(repo, "bad:name.txt")
        _write(repo, "dir>name/inner.txt")
        _write(repo, "CON.txt")
        _write(repo, "space-end.txt ")
        _write(repo, "dot-end.")
        _write(repo, "ok-name.txt")
        _git(repo, "add", "-A")
        proc = _run_cli(repo)
        out = proc.stdout.decode("utf-8", "surrogateescape")
        stderr = proc.stderr.decode("utf-8", "replace")
        tap.ok(proc.returncode == 1, "E2E tracked: có tên xấu ⇒ exit 1 (ĐỎ)", f"exit {proc.returncode} {stderr}")
        for code, path, note in (
            ("E1_NTFS_CHAR", "bad:name.txt", "':' trong tên tệp"),
            ("E1_NTFS_CHAR", "dir>name/inner.txt", "'>' trong tên thư mục"),
            ("E1_NTFS_DEVICE", "CON.txt", "CON.txt (có đuôi)"),
            ("E1_NTFS_TRAILING", "space-end.txt ", "dấu cách cuối tên"),
            ("E1_NTFS_TRAILING", "dot-end.", "dấu chấm cuối tên"),
        ):
            tap.ok(f"{code} {path}:" in out, f"E2E tracked: {code} cho {path} ({note})")
        tap.ok("E1 tên tệp NTFS: FAIL" in out, "E2E tracked: có dòng tổng kết FAIL")

        # ---- 16: `--staged` bắt tên xấu **chưa** vào lịch sử (đúng đường hook pre-commit dùng).
        _git(repo, "reset", "-q")
        _write(repo, "staged:bad.txt")
        _git(repo, "add", "--", "staged:bad.txt")
        proc = _run_cli(repo, "--staged")
        out = proc.stdout.decode("utf-8", "surrogateescape")
        tap.ok(
            proc.returncode == 1 and "E1_NTFS_CHAR staged:bad.txt:" in out,
            "E2E staged: tên xấu đang stage ⇒ ĐỎ đúng mã",
            f"exit {proc.returncode}, ra {out!r}",
        )
        # ---- 17: đổi tên tệp xấu → tốt **không** bị chặn (đây là chỗ dễ chặn oan nhất).
        _git(repo, "reset", "-q")
        os.unlink(repo / "staged:bad.txt")
        _write(repo, "bad:old.txt")
        _git(repo, "add", "--", "bad:old.txt")
        _git(repo, "commit", "-q", "-m", "baseline tên xấu (cố ý, repo tạm)")
        os.rename(repo / "bad:old.txt", repo / "good-new.txt")
        # **Chỉ** stage đúng hai đường dẫn của lần đổi tên — không `add -A` (sẽ stage cả các
        # tệp xấu cố ý khác của fixture và ca này sẽ kiểm sai thứ).
        _git(repo, "add", "--", "good-new.txt", "bad:old.txt")
        proc = _run_cli(repo, "--staged")
        tap.ok(
            proc.returncode == 0,
            "E2E staged: đổi tên xấu → tốt KHÔNG bị chặn (--no-renames)",
            proc.stdout.decode("utf-8", "surrogateescape"),
        )

        # ---- 18..19: hook pre-commit thật: chỉ vá đúng chỗ, không phá ca cũ của KT-85.
        hook_dir = repo / ".git" / "hooks"
        hook_dir.mkdir(parents=True, exist_ok=True)
        src_hook = HERE.parent / "git-guard" / "pre-commit"
        hook_dst = hook_dir / "pre-commit"
        shutil.copy2(src_hook, hook_dst)
        hook_dst.chmod(0o755)
        # Hook gọi checker theo đường dẫn **tương đối gốc repo** ⇒ chép checker vào đúng chỗ.
        checker_dst = repo / "v2" / "tools" / "check-filenames.py"
        checker_dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(HERE, checker_dst)

        lead_env = dict(os.environ)
        lead_env["VIFAST_ROLE"] = "lead"
        _write(repo, "hook:bad.txt")
        _git(repo, "add", "--", "hook:bad.txt")
        proc = _git(repo, "commit", "-q", "-m", "[KT-331] ca am: ten xau", env=lead_env)
        both = proc.stdout + proc.stderr
        text = both.decode("utf-8", "surrogateescape")
        tap.ok(
            proc.returncode != 0
            and "E1_NTFS_CHAR hook:bad.txt:" in text
            and "CHO QUA" not in text,
            "hook pre-commit: VIFAST_ROLE=lead + tên xấu đã stage ⇒ TỪ CHỐI (E1_NTFS_CHAR)",
            f"exit {proc.returncode}, ra {text[-400:]!r}",
        )

        # Ca dương: cùng hook, cùng vai lead, tên hợp lệ ⇒ CHO QUA (không phá KT-85 DƯƠNG-1).
        _git(repo, "reset", "-q")
        os.unlink(repo / "hook:bad.txt")
        _write(repo, "hook-good.txt")
        _git(repo, "add", "--", "hook-good.txt")
        proc = _git(repo, "commit", "-q", "-m", "[KT-331] ca duong: ten hop le", env=lead_env)
        text = (proc.stdout + proc.stderr).decode("utf-8", "surrogateescape")
        tap.ok(
            proc.returncode == 0 and "CHO QUA" in text,
            "hook pre-commit: tên hợp lệ ⇒ CHO QUA (giữ nguyên KT-85)",
            f"exit {proc.returncode}, ra {text[-400:]!r}",
        )

        # Ca âm cũ của KT-85 phải giữ: **không** đặt VIFAST_ROLE ⇒ vẫn TỪ CHỐI.
        _write(repo, "hook-good2.txt")
        _git(repo, "add", "--", "hook-good2.txt")
        env_no_role = dict(os.environ)
        env_no_role.pop("VIFAST_ROLE", None)
        proc = _git(repo, "commit", "-q", "-m", "[KT-331] ca am: khong vai", env=env_no_role)
        text = (proc.stdout + proc.stderr).decode("utf-8", "surrogateescape")
        tap.ok(
            proc.returncode != 0 and "TỪ CHỐI COMMIT" in text,
            "hook pre-commit: không VIFAST_ROLE ⇒ vẫn TỪ CHỐI (fail-closed KT-85)",
            f"exit {proc.returncode}",
        )

        # ---- 20: ca dương end-to-end: repo tạm toàn tên hợp lệ ⇒ XANH, exit 0.
        repo2 = _mk_repo(tmp / "p2")
        _write(repo2, "A10_cfg(feature=ktest)_inline.cargo.log")
        _write(repo2, "dir con/A18_path_Unicode_đường_dẫn.json")
        _git(repo2, "add", "-A")
        proc = _run_cli(repo2)
        out = proc.stdout.decode("utf-8", "surrogateescape")
        tap.ok(
            proc.returncode == 0 and "E1 tên tệp NTFS: PASS" in out,
            "ca dương E2E: repo toàn tên hợp lệ ⇒ XANH, exit 0",
            f"exit {proc.returncode}, ra {out!r}",
        )

    return tap.done()


# ---------------------------------------------------------------- main
def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(
        description="KT-331/D2-230 — kiểm tên tệp tương thích Windows/NTFS (chỉ tệp Git theo dõi)."
    )
    ap.add_argument("--root", default=str(DEFAULT_ROOT), help="gốc repo (mặc định: gốc của repo chứa script)")
    ap.add_argument("--staged", action="store_true", help="chỉ kiểm tên đang stage (dùng cho hook pre-commit)")
    ap.add_argument("--selftest", action="store_true", help="ca tự kiểm: ca âm ĐỎ đúng mã + ca dương XANH")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()
    return run_check(Path(args.root), args.staged)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
