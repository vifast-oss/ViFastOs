#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""mutants.py — **`v2 mutants`**: chạy đột biến theo manifest `v2/test/mutants.toml` (KT-403).

Vì sao có tệp này
-----------------
Trước KT-403, bộ kiểm có **43 khai báo feature `mut-*`** rải trong 7 `Cargo.toml` + `selfbreak`,
không có manifest, không có lệnh chạy: mỗi lần dùng là sửa tay `--features`, không ai đếm được
đột biến nào còn sống. Luật D2-113F đòi mỗi phép kiểm phải chứng minh **có răng**; thiết kế §9.9
đòi "đột biến sống = lỗ hổng bộ kiểm, mở task bổ sung kịch bản".

Cơ chế (không bao giờ sửa cây thật — D2-113F)
--------------------------------------------
- `kind = "feature"`: chép **bản sao** của cây (`v2/`, `spec/`, `docs/`, `Cargo.toml`) vào
  `mktemp -d`, rồi lật cfg của **đúng một** đột biến:
      `not(feature = "X")` → `any()`        (nhánh gốc tắt)
      `feature = "X"`     → `all()`          (nhánh đột biến bật)
  Phải có ≥ 1 điểm cfg, nếu không ⇒ `E_STALE` (đột biến đã chết vì mã đổi — sửa manifest).
- `kind = "script"`: chạy thẳng lệnh (self-test tự bảo vệ cây thật, vd `selfbreak.sh`).

Phân loại kết quả
-----------------
| Kết quả | Nghĩa |
|---|---|
| `KILLED` | lệnh ĐỎ **và** marker khớp ⇒ bộ kiểm bắt được đột biến |
| `SURVIVED` | lệnh XANH ⇒ đột biến sống = lỗ hổng bộ kiểm (exit 1) |
| `E_WRONG-REASON` | lệnh ĐỎ nhưng marker không khớp ⇒ đỏ vì lý do khác (không tính là bắt) |
| `E_STALE` | 0 điểm cfg còn tồn tại |
| `E_UNWIRED` | `wired = false` — bộ bắt chưa chốt (exit 3 cho `--all`) |

`--selftest` dựng một cây mini trong `/tmp` và chứng minh **cả 5** kết luận trên (đỏ-trước cho
chính công cụ): mỗi ca có kỳ vọng riêng, sai kỳ vọng ⇒ exit 1.

Chạy
----
    python3 v2/tools/mutants.py --root <REPO> --list
    python3 v2/tools/mutants.py --root <REPO> --lint
    python3 v2/tools/mutants.py --root <REPO> mut-no-irq
    python3 v2/tools/mutants.py --root <REPO> --all
    python3 v2/tools/mutants.py --selftest

Exit: 0 = KILLED (hoặc lint/selftest xanh); 1 = SURVIVED / sai lý do / lint đỏ;
      2 = dùng sai, manifest hỏng, `E_STALE`, hoặc mutant chưa nối; 3 = `--all` CHƯA NỐI ĐỦ.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

MANIFEST_REL = "v2/test/mutants.toml"
CFG_FEATURE = 'feature = "%s"'
CFG_NOT = 'not(feature = "%s")'
KINDS = ("feature", "script", "build")
EXPECTS = ("red", "pass")
COPY_ENTRIES = ("v2", "spec", "docs")
COPY_FILES = ("Cargo.toml", "rust-toolchain.toml", "AGENTS.md")


class ManifestError(Exception):
    """Manifest hỏng/dùng sai ⇒ exit 2."""


def _tomllib():
    try:
        import tomllib  # type: ignore[import-not-found]
    except Exception:  # pragma: no cover
        raise ManifestError("cần python ≥ 3.11 có `tomllib`")
    return tomllib


# --------------------------------------------------------------------------- manifest


def load_manifest(root):
    path = os.path.join(root, MANIFEST_REL)
    if not os.path.isfile(path):
        raise ManifestError("thiếu %s" % MANIFEST_REL)
    tomllib = _tomllib()
    try:
        with open(path, "rb") as fh:
            data = tomllib.load(fh)
    except Exception as exc:  # noqa: BLE001
        raise ManifestError("không parse được %s: %s" % (MANIFEST_REL, exc))
    if data.get("version") != 1:
        raise ManifestError("version=%r (cần 1)" % (data.get("version"),))
    if not isinstance(data.get("suite"), list) or not data["suite"]:
        raise ManifestError("thiếu mảng `[[suite]]`")
    if not isinstance(data.get("mutant"), list) or not data["mutant"]:
        raise ManifestError("thiếu mảng `[[mutant]]`")
    return data


def suites_by_id(data):
    suites = {}
    for s in data["suite"]:
        sid = s.get("id")
        if not sid or sid in suites:
            raise ManifestError("suite id thiếu/trùng: %r" % (sid,))
        for key in ("command", "marker"):
            if not isinstance(s.get(key), str) or not s[key]:
                raise ManifestError("suite %s thiếu `%s`" % (sid, key))
        suites[sid] = s
    return suites


def declarations(root):
    """Mọi khai báo `mut-*` trong `[features]` của `v2/kernel/*/Cargo.toml` (crate, feature)."""
    out = []
    kernel = os.path.join(root, "v2", "kernel")
    for name in sorted(os.listdir(kernel)):
        manifest = os.path.join(kernel, name, "Cargo.toml")
        if not os.path.isfile(manifest):
            continue
        with open(manifest, "r", encoding="utf-8") as fh:
            text = fh.read()
        crate = re.search(r'^name\s*=\s*"([^"]+)"', text, re.MULTILINE)
        if not crate:
            continue
        section = ""
        for line in text.splitlines():
            stripped = line.strip()
            head = re.match(r"^\[([^\]]+)\]$", stripped)
            if head:
                section = head.group(1).strip()
                continue
            if section != "features":
                continue
            m = re.match(r"^(mut-[A-Za-z0-9_-]+)\s*=", stripped)
            if m:
                out.append((crate.group(1), m.group(1)))
    return out


def cfg_sites(root, feature):
    """Số điểm cfg `feature = "<id>"` trong `v2/kernel/**/*.rs` (không tính chú thích? — tính hết,
    vì `cfg!()` / `#[cfg]` đều là điểm thật; một dòng chú thích có chuỗi này vẫn là tín hiệu
    manifest còn sống, và phép thử "0 điểm" chỉ để bắt đột biến đã bị xoá hẳn)."""
    needle = CFG_FEATURE % feature
    count = 0
    kernel = os.path.join(root, "v2", "kernel")
    for dirpath, _dirnames, filenames in os.walk(kernel):
        for name in filenames:
            if not name.endswith(".rs"):
                continue
            with open(os.path.join(dirpath, name), "r", encoding="utf-8", errors="replace") as fh:
                count += fh.read().count(needle)
    return count


def lint(root, data):
    problems = []
    suites = suites_by_id(data)
    seen = set()
    covered = set()
    n_feature = 0
    for m in data["mutant"]:
        mid = m.get("id")
        if not mid:
            problems.append("E_MANIFEST mutant thiếu `id`")
            continue
        if mid in seen:
            problems.append("E_MANIFEST id trùng: %s" % mid)
        seen.add(mid)
        kind = m.get("kind")
        if kind not in KINDS:
            problems.append("E_MANIFEST %s: kind=%r" % (mid, kind))
            continue
        if m.get("expect") not in EXPECTS:
            problems.append("E_MANIFEST %s: expect=%r" % (mid, m.get("expect")))
        suite = m.get("suite")
        if suite not in suites:
            problems.append("E_MANIFEST %s: suite lạ %r" % (mid, suite))
        if not isinstance(m.get("wired"), bool):
            problems.append("E_MANIFEST %s: thiếu `wired` (bool)" % mid)
        if kind == "build":
            n_feature += 1
            feature = m.get("feature")
            if not isinstance(feature, str) or not feature.startswith("mut-"):
                problems.append("E_MANIFEST %s: feature=%r" % (mid, feature))
                continue
            crates = m.get("crates") or []
            covered.add((crates[0] if crates else "?", feature))
            if m.get("evidence") == "graph":
                continue
            seen_build = False
            for c in crates:
                build_rs = os.path.join(root, "v2", "kernel", c, "build.rs")
                if os.path.isfile(build_rs):
                    with open(build_rs, "r", encoding="utf-8", errors="replace") as fh:
                        if feature in fh.read():
                            seen_build = True
            if not seen_build:
                problems.append("E_STALE %s: không thấy `%s` trong build.rs" % (mid, feature))
            continue
        if kind == "feature":
            n_feature += 1
            feature = m.get("feature")
            if not isinstance(feature, str) or not feature.startswith("mut-"):
                problems.append("E_MANIFEST %s: feature=%r" % (mid, feature))
                continue
            crates = m.get("crates")
            if not isinstance(crates, list) or not all(isinstance(c, str) for c in crates):
                problems.append("E_MANIFEST %s: crates phải là mảng chuỗi" % mid)
                continue
            for c in crates:
                covered.add((c, feature))
            sites = cfg_sites(root, feature)
            if sites == 0:
                problems.append("E_STALE %s: 0 điểm cfg `%s` trong v2/kernel" % (mid, feature))
    decls = declarations(root)
    want = data.get("declarations")
    if isinstance(want, int) and want != len(decls):
        problems.append(
            "E_MANIFEST khai báo `mut-*` thực tế=%d, manifest ghi %d" % (len(decls), want)
        )
    for crate, feature in sorted(set(decls) - covered):
        problems.append("E_MANIFEST khai báo (%s, %s) chưa có trong manifest" % (crate, feature))
    print(
        "# mutants lint: mutants=%d feature=%d declarations=%d covered=%d suites=%d problems=%d"
        % (len(data["mutant"]), n_feature, len(decls), len(covered), len(suites), len(problems))
    )
    for p in problems:
        print(p)
    return problems


# --------------------------------------------------------------------------- chạy một đột biến


def _copy_tree(root, dest):
    """Bản sao **đủ để harness chạy**: `v2/`, `spec/`, `docs/` + tệp gốc. Bỏ `target/`."""
    os.makedirs(dest, exist_ok=True)
    ignore = shutil.ignore_patterns("target", ".git")
    for name in COPY_ENTRIES:
        src = os.path.join(root, name)
        if os.path.isdir(src):
            shutil.copytree(src, os.path.join(dest, name), ignore=ignore, symlinks=False)
    for name in COPY_FILES:
        src = os.path.join(root, name)
        if os.path.isfile(src):
            shutil.copy2(src, os.path.join(dest, name))
    return dest


def apply_feature(copy_root, feature):
    """Lật cfg của **một** đột biến trên bản sao; trả số lần thay thế."""
    needle_not = CFG_NOT % feature
    needle = CFG_FEATURE % feature
    n = 0
    kernel = os.path.join(copy_root, "v2", "kernel")
    for dirpath, _dirnames, filenames in os.walk(kernel):
        for name in filenames:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(dirpath, name)
            with open(path, "r", encoding="utf-8", errors="replace") as fh:
                text = fh.read()
            if needle not in text:
                continue
            new = text.replace(needle_not, "any()").replace(needle, "all()")
            if new != text:
                # `not()` đã bị thay trước nên `feature = "X"` chỉ còn ở nhánh đột biến.
                n += text.count(needle)
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write(new)
    return n


def _run_command(cmd, cwd, timeout):
    env = dict(os.environ)
    env["GIT_OPTIONAL_LOCKS"] = "0"  # git chỉ đọc (build-rev.sh) — không ghi .git
    env.pop("VIFAST_ROLE", None)  # không bao giờ đóng giả lead
    t0 = time.time()
    try:
        proc = subprocess.run(
            ["bash", "-c", cmd],
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return proc.returncode, (proc.stdout or "") + (proc.stderr or ""), time.time() - t0
    except subprocess.TimeoutExpired as exc:
        out = (exc.stdout or "") + (exc.stderr or "")
        if isinstance(out, bytes):
            out = out.decode("utf-8", "replace")
        return 124, out, time.time() - t0


def run_mutant(root, mutant, suites, args):
    """Trả `(verdict, rc, marker_ok, detail, elapsed)`."""
    mid = mutant["id"]
    if not mutant.get("wired"):
        return "E_UNWIRED", None, False, mutant.get("why", ""), 0.0
    suite = suites[mutant["suite"]]
    cmd = suite["command"]
    marker = suite["marker"]
    kind = mutant["kind"]
    if kind == "build":
        return "E_UNWIRED", None, False, mutant.get("why", "kind=build chưa nối"), 0.0
    if kind == "feature":
        copy_root = os.path.join(args.scratch, "mut-" + re.sub(r"[^A-Za-z0-9_-]", "_", mid))
        _copy_tree(root, copy_root)
        sites = apply_feature(copy_root, mutant["feature"])
        if sites == 0:
            return "E_STALE", None, False, "0 điểm cfg `%s`" % mutant["feature"], 0.0
        cwd = copy_root
        detail = "copy=%s sites=%d" % (copy_root, sites)
    else:
        cwd = root
        detail = "cwd=repo (script tự bảo vệ cây thật)"
    rc, out, elapsed = _run_command(cmd, cwd, args.timeout)
    marker_ok = re.search(marker, out, re.MULTILINE) is not None
    if mutant["expect"] == "pass":
        verdict = "KILLED" if (rc == 0 and marker_ok) else ("E_WRONG-REASON" if rc == 0 else "SURVIVED")
    elif rc != 0 and marker_ok:
        verdict = "KILLED"
    elif rc == 0:
        verdict = "SURVIVED"
    else:
        verdict = "E_WRONG-REASON"
    log = os.path.join(args.scratch, mid + ".log")
    with open(log, "w", encoding="utf-8") as fh:
        fh.write(out)
    detail += " rc=%d marker=%s log=%s" % (rc, "ok" if marker_ok else "KHONG", log)
    return verdict, rc, marker_ok, detail, elapsed


# --------------------------------------------------------------------------- selftest

MINI_LIB = """\
// SPDX-License-Identifier: MIT OR Apache-2.0
#[cfg(not(feature = "mut-demo"))]
pub fn f() -> u32 {
    0
}
#[cfg(feature = "mut-demo")]
pub fn f() -> u32 {
    1
}
"""

ORACLE_RED = """\
#!/usr/bin/env bash
if grep -q 'cfg(all())' v2/kernel/vifast-plat-demo/src/lib.rs; then
    echo "not ok 1 - demo: đột biến bị bắt"
    exit 1
fi
echo "ok 1 - demo"
exit 0
"""

ORACLE_GREEN = "#!/usr/bin/env bash\necho 'ok 1 - demo'\nexit 0\n"

ORACLE_WRONG = "#!/usr/bin/env bash\necho 'lỗi hạ tầng khác'\nexit 1\n"


def _mini_manifest():
    return {
        "version": 1,
        "suite": [
            {"id": "red", "command": "bash v2/test/oracle/oracle-red.sh", "marker": "^not ok "},
            {"id": "green", "command": "bash v2/test/oracle/oracle-green.sh", "marker": "^not ok "},
            {"id": "wrong", "command": "bash v2/test/oracle/oracle-wrong.sh", "marker": "^not ok "},
            {"id": "pass", "command": "bash v2/test/oracle/oracle-green.sh", "marker": "^ok 1"},
        ],
        "mutant": [
            {
                "id": "mut-demo",
                "kind": "feature",
                "feature": "mut-demo",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "red",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-ghost",
                "kind": "feature",
                "feature": "mut-ghost",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "red",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-survive",
                "kind": "feature",
                "feature": "mut-demo",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "green",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "mut-wrong",
                "kind": "feature",
                "feature": "mut-demo",
                "archs": ["host"],
                "crates": ["vifast-plat-demo"],
                "suite": "wrong",
                "expect": "red",
                "wired": True,
            },
            {
                "id": "green-pass",
                "kind": "script",
                "suite": "pass",
                "expect": "pass",
                "wired": True,
            },
            {
                "id": "unwired",
                "kind": "script",
                "suite": "pass",
                "expect": "pass",
                "wired": False,
                "why": "ca minh hoạ `E_UNWIRED`",
            },
        ],
    }


def _mini_tree(base):
    lib = os.path.join(base, "v2", "kernel", "vifast-plat-demo", "src", "lib.rs")
    os.makedirs(os.path.dirname(lib), exist_ok=True)
    with open(lib, "w", encoding="utf-8") as fh:
        fh.write(MINI_LIB)
    with open(os.path.join(base, "v2", "kernel", "vifast-plat-demo", "Cargo.toml"), "w") as fh:
        fh.write('[package]\nname = "vifast-plat-demo"\nversion = "0.1.0"\n')
    for name, body in (("oracle-red.sh", ORACLE_RED), ("oracle-green.sh", ORACLE_GREEN),
                       ("oracle-wrong.sh", ORACLE_WRONG)):
        path = os.path.join(base, "v2", "test", "oracle", name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(body)
        os.chmod(path, 0o755)


def _hash_tree(root):
    import hashlib

    h = hashlib.sha256()
    for dirpath, _dirnames, filenames in os.walk(root):
        for name in sorted(filenames):
            path = os.path.join(dirpath, name)
            h.update(os.path.relpath(path, root).encode())
            with open(path, "rb") as fh:
                h.update(fh.read())
    return h.hexdigest()


class _Args:
    def __init__(self, scratch, timeout):
        self.scratch = scratch
        self.timeout = timeout


def run_selftest(root, args):
    base = tempfile.mkdtemp(prefix="kt403-mutants-", dir="/tmp")
    data = _mini_manifest()
    suites = suites_by_id(data)
    case_args = _Args(os.path.join(base, "scratch"), 120)
    os.makedirs(case_args.scratch)
    _mini_tree(os.path.join(base, "mini"))
    mini = os.path.join(base, "mini")
    before = _hash_tree(mini)
    cases = [
        ("mut-demo", "KILLED", "đột biến bị bắt (đỏ + marker)"),
        ("mut-ghost", "E_STALE", "0 điểm cfg ⇒ E_STALE"),
        ("mut-survive", "SURVIVED", "lệnh XANH ⇒ SURVIVED (lỗ hổng bộ kiểm)"),
        ("mut-wrong", "E_WRONG-REASON", "đỏ vì lý do khác ⇒ E_WRONG-REASON"),
        ("green-pass", "KILLED", "script expect=pass, marker khớp ⇒ KILLED"),
        ("unwired", "E_UNWIRED", "wired=false ⇒ E_UNWIRED"),
    ]
    by_id = {m["id"]: m for m in data["mutant"]}
    total = len(cases) + 1
    fail = 0
    print("== mutants — SELFTEST (cây mini %s) ==" % mini)
    for i, (mid, want, label) in enumerate(cases, 1):
        verdict, _rc, _mk, detail, _el = run_mutant(mini, by_id[mid], suites, case_args)
        ok = verdict == want
        print(
            "%s %d - %s: verdict=%s (muốn %s) => %s"
            % ("ok" if ok else "not ok", i, label, verdict, want, "ĐÚNG" if ok else "SAI")
        )
        if not ok:
            print("#    chi tiết: %s" % detail)
            fail += 1
    after = _hash_tree(mini)
    ok = before == after
    print(
        "%s %d - cây thật (mini) không bị sửa bởi phép chạy => %s"
        % ("ok" if ok else "not ok", total, "ĐÚNG" if ok else "SAI")
    )
    if not ok:
        fail += 1
    shutil.rmtree(base, ignore_errors=True)
    if fail:
        print("== SELFTEST mutants: SAI — %d/%d ca sai kỳ vọng ==" % (fail, total))
        return 1
    print("== SELFTEST mutants: OK — %d/%d ca (đỏ-trước cho chính công cụ) ==" % (total, total))
    return 0


# --------------------------------------------------------------------------- CLI


def main(argv):
    ap = argparse.ArgumentParser(description="v2 mutants — chạy đột biến theo v2/test/mutants.toml")
    ap.add_argument("--root", default=None, help="gốc repo (mặc định: suy từ vị trí script)")
    ap.add_argument("--list", action="store_true", help="liệt kê đột biến + trạng thái `wired`")
    ap.add_argument("--lint", action="store_true", help="kiểm manifest (không chạy đột biến)")
    ap.add_argument("--all", action="store_true", help="chạy mọi đột biến ĐÃ NỐI")
    ap.add_argument("--dry-run", action="store_true", help="in việc sẽ làm, không thực thi")
    ap.add_argument("--selftest", action="store_true", help="đỏ-trước cho chính công cụ")
    ap.add_argument("--timeout", type=float, default=1800.0, help="timeout mỗi đột biến (giây)")
    ap.add_argument("--keep", action="store_true", help="giữ bản sao/log sau khi chạy")
    ap.add_argument("--verbose", action="store_true", help="in cả log đường dẫn")
    ap.add_argument("name", nargs="?", default=None, help="id đột biến cần chạy")
    args = ap.parse_args(argv)

    if args.root is None:
        args.root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    args.root = os.path.abspath(args.root)

    if args.selftest:
        return run_selftest(args.root, args)

    try:
        data = load_manifest(args.root)
        suites = suites_by_id(data)
    except ManifestError as exc:
        print("mutants: %s" % exc, file=sys.stderr)
        return 2

    if args.list:
        print("# mutants: id\tkind\tarchs\tsuite\texpect\twired")
        for m in data["mutant"]:
            print(
                "%s\t%s\t%s\t%s\t%s\t%s"
                % (
                    m["id"],
                    m.get("kind"),
                    ",".join(m.get("archs", ["-"])) if m.get("kind") == "feature" else "-",
                    m.get("suite"),
                    m.get("expect"),
                    "yes" if m.get("wired") else "NO",
                )
            )
        wired = sum(1 for m in data["mutant"] if m.get("wired"))
        print("# wired=%d/%d declarations=%s" % (wired, len(data["mutant"]), data.get("declarations")))
        return 0

    if args.lint or not (args.all or args.name):
        problems = lint(args.root, data)
        if args.lint:
            return 1 if problems else 0
        print("mutants: cần `--list`, `--lint`, `--all`, `--selftest` hoặc <id>", file=sys.stderr)
        return 2

    selected = data["mutant"] if args.all else [m for m in data["mutant"] if m["id"] == args.name]
    if not selected:
        print("mutants: id lạ %r" % args.name, file=sys.stderr)
        return 2
    unwired = [m["id"] for m in selected if not m.get("wired")]
    if args.dry_run:
        for m in selected:
            print(
                "# dry-run %s: kind=%s suite=%s expect=%s wired=%s"
                % (m["id"], m.get("kind"), m.get("suite"), m.get("expect"), m.get("wired"))
            )
        print("# dry-run: %d đột biến, %d chưa nối" % (len(selected), len(unwired)))
        return 3 if unwired else 0

    scratch = tempfile.mkdtemp(prefix="kt403-mutants-", dir="/tmp")
    args.scratch = scratch
    rc_total = 0
    killed = survived = wrong = stale = 0
    try:
        for m in selected:
            verdict, _rc, _mk, detail, elapsed = run_mutant(args.root, m, suites, args)
            if verdict == "KILLED":
                killed += 1
            elif verdict == "SURVIVED":
                survived += 1
                rc_total = 1
            elif verdict == "E_WRONG-REASON":
                wrong += 1
                rc_total = 1
            elif verdict == "E_STALE":
                stale += 1
                rc_total = 2
            elif verdict == "E_UNWIRED":
                rc_total = 3
            print(
                "%s - %s: %s (%.1f s) %s"
                % ("ok" if verdict == "KILLED" else "not ok", m["id"], verdict, elapsed, detail)
            )
        print(
            "# mutants: chạy=%d killed=%d survived=%d wrong-reason=%d stale=%d chưa-nối=%d"
            % (len(selected), killed, survived, wrong, stale, len(unwired))
        )
    finally:
        if args.keep:
            print("# giữ bản sao: %s" % scratch)
        else:
            shutil.rmtree(scratch, ignore_errors=True)
    if unwired and not args.all:
        return 2
    return rc_total


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
