#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Máy kiểm báo cáo vòng (Khối E) cho dòng v2.

Mặc định kiểm `v2/agent/reports/R1-v2.{md,json}`; `--all` kiểm mọi `R*-v2.json` có thật.
`--file` để test fixture (vẫn kiểm git + QUEUE + BLOCKED của repo thật).

Triết lý: báo cáo là thứ Agent 0 đọc, nên mọi sự thật trong đó phải **đối chiếu được với máy**:
`head` tồn tại và là tổ tiên HEAD, mọi `commits[]` tồn tại, `status` khớp bảng QUEUE, phủ **đủ**
mọi hàng QUEUE, `blocked[]` phủ mọi hàng BLOCKED/REVIEW, và YC phải có thật trong `BLOCKED.md`.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REPORTS = ROOT / "v2/agent/reports"
QUEUE = ROOT / "v2/agent/QUEUE.md"
BLOCKED = ROOT / "v2/agent/BLOCKED.md"

KEYS = {
    "project", "round", "head", "build_id", "tasks", "gates",
    "sim", "coverage", "mutants", "perf", "blocked",
}
STATUSES = {"READY", "BLOCKED", "REVIEW", "DONE"}
GATES = {"fast", "full", "nightly", "boot50", "l1-1000"}
SIM_SEEDS = 1000  # L1 trong gate --fast (v2/sim/vifast-sim/tests/l1.rs)

fails: list[str] = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def git(*args) -> str:
    out = subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, check=False
    )
    return out.stdout.strip() if out.returncode == 0 else ""


def commit_exists(sha: str) -> bool:
    if not sha or not re.fullmatch(r"[0-9a-f]{7,40}", sha):
        return False
    return bool(git("cat-file", "-e", f"{sha}^{{commit}}") or True) and (
        subprocess.run(
            ["git", "cat-file", "-e", f"{sha}^{{commit}}"],
            cwd=ROOT,
            capture_output=True,
            check=False,
        ).returncode
        == 0
    )


def split_row(line: str) -> list[str]:
    """Tách ô bảng markdown, tôn trọng escape `\\|` (hàng KT-06 có `v2\\|viscript`)."""
    out, cur, i = [], "", 0
    line = line.strip()
    body = line[1:] if line.startswith("|") else line
    body = body[:-1] if body.endswith("|") else body
    while i < len(body):
        if body[i] == "\\" and i + 1 < len(body) and body[i + 1] == "|":
            cur += "|"
            i += 2
        elif body[i] == "|":
            out.append(cur.strip())
            cur = ""
            i += 1
        else:
            cur += body[i]
            i += 1
    out.append(cur.strip())
    return out


def queue_statuses() -> dict[str, str]:
    rows: dict[str, str] = {}
    for line in QUEUE.read_text().splitlines():
        t = line.strip()
        if t.startswith("| KT-") or t.startswith("| ST-"):
            cells = split_row(t)
            if len(cells) >= 6:
                rows[cells[0]] = cells[4].strip("`* ")
    return rows


def blocked_text() -> str:
    return BLOCKED.read_text()


def check_md(path: Path) -> None:
    check(path.is_file(), f"{path.name}: thiếu tệp MD")
    if not path.is_file():
        return
    text = path.read_text()
    for i in range(1, 7):
        check(f"## {i}." in text, f"{path.name}: thiếu mục '## {i}.'")
    check(len(text) > 800, f"{path.name}: quá ngắn ({len(text)} ký tự)")


def check_report(path: Path, round_no: int, queue: dict[str, str]) -> None:
    tag = path.name
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fails.append(f"{tag}: JSON hỏng ({exc})")
        return

    check(set(data) >= KEYS, f"{tag}: thiếu khoá {sorted(KEYS - set(data))}")
    check(data.get("project") == "v2", f"{tag}: project phải 'v2'")
    check(data.get("round") == round_no, f"{tag}: round phải {round_no}")

    head = str(data.get("head", ""))
    check(commit_exists(head), f"{tag}: head {head!r} không tồn tại trong git")
    if commit_exists(head):
        anc = subprocess.run(
            ["git", "merge-base", "--is-ancestor", head, "HEAD"], cwd=ROOT, check=False
        )
        check(anc.returncode == 0, f"{tag}: head {head} không phải tổ tiên HEAD")

    gates = data.get("gates") or {}
    check(gates.get("fast") in {"PASS", "FAIL", "—"}, f"{tag}: gates.fast lạ")
    check(gates.get("full") in {"PASS", "FAIL", "—"}, f"{tag}: gates.full lạ")
    check(gates.get("nightly") in {"PASS", "FAIL", "—"}, f"{tag}: gates.nightly lạ")

    sim = data.get("sim") or {}
    check(sim.get("seeds") == SIM_SEEDS, f"{tag}: sim.seeds phải {SIM_SEEDS}")
    check(isinstance(sim.get("failures"), int), f"{tag}: sim.failures phải số nguyên")

    cov = data.get("coverage") or {}
    check(isinstance(cov, dict) and cov, f"{tag}: coverage rỗng")

    mut = data.get("mutants") or {}
    check(isinstance(mut, dict), f"{tag}: mutants phải object")
    check(mut.get("measured") is False or isinstance(mut.get("measured"), bool),
          f"{tag}: mutants.measured phải bool")
    for k in ("killed", "alive"):
        v = mut.get(k, "missing")
        check(v is None or isinstance(v, int), f"{tag}: mutants.{k} phải số hoặc null")

    check(isinstance(data.get("perf"), dict), f"{tag}: perf phải object")

    tasks = data.get("tasks") or []
    check(len(tasks) > 0, f"{tag}: tasks rỗng")
    seen: dict[str, str] = {}
    for t in tasks:
        tid = t.get("id")
        check(tid in queue, f"{tag}: task {tid!r} không có trong QUEUE")
        if tid in queue:
            check(
                t.get("status") == queue[tid],
                f"{tag}: {tid} status {t.get('status')!r} ≠ QUEUE {queue[tid]!r}",
            )
        check(t.get("status") in STATUSES, f"{tag}: {tid} status ngoài 4 trạng thái")
        commits = t.get("commits")
        check(isinstance(commits, list), f"{tag}: {tid}.commits phải mảng")
        for c in commits or []:
            check(commit_exists(c), f"{tag}: {tid} commit {c!r} không tồn tại trong git")
        for g in t.get("gates") or []:
            check(g in GATES, f"{tag}: {tid} gate lạ {g!r}")
        check(tid not in seen, f"{tag}: task {tid} trùng")
        seen[tid] = t.get("status")

    # Phủ ĐỦ mọi hàng QUEUE (review #12: xoá 1 task vẫn PASS là lỗi).
    missing = sorted(set(queue) - set(seen))
    check(not missing, f"{tag}: thiếu task trong báo cáo: {missing}")
    extra = sorted(set(seen) - set(queue))
    check(not extra, f"{tag}: task lạ ngoài QUEUE: {extra}")

    # blocked[]: id + question; YC phải có thật; phủ mọi hàng BLOCKED/REVIEW.
    blocked = data.get("blocked") or []
    check(isinstance(blocked, list), f"{tag}: blocked phải mảng")
    btext = blocked_text()
    for b in blocked:
        check(bool(b.get("id")), f"{tag}: blocked có mục thiếu id")
        check(bool(b.get("question")), f"{tag}: blocked {b.get('id')} thiếu question")
        if str(b.get("id", "")).startswith("YC-"):
            check(b["id"] in btext, f"{tag}: {b['id']} không có trong BLOCKED.md")
    ids = {b.get("id") for b in blocked}
    for tid, st in queue.items():
        if st in {"BLOCKED", "REVIEW"}:
            # Review #13: KHÔNG được miễn trừ vì task đã mang status BLOCKED/REVIEW trong tasks[].
            check(tid in ids, f"{tag}: {tid} ({st}) không xuất hiện trong blocked[]")


def rounds() -> list[int]:
    found = []
    for p in REPORTS.glob("R*-v2.json"):
        m = re.fullmatch(r"R(\d+)-v2\.json", p.name)
        if m:
            found.append(int(m.group(1)))
    return sorted(found)


def main() -> int:
    args = sys.argv[1:]
    queue = queue_statuses()
    check(queue, "QUEUE: không đọc được hàng nào")
    targets: list[tuple[Path, int]] = []
    if "--file" in args:
        p = Path(args[args.index("--file") + 1])
        targets.append((p, int(json.loads(p.read_text()).get("round", 1))))
    elif "--all" in args:
        for n in rounds():
            targets.append((REPORTS / f"R{n}-v2.json", n))
    else:
        n = 1
        if "--n" in args:
            n = int(args[args.index("--n") + 1])
        targets.append((REPORTS / f"R{n}-v2.json", n))
    check(targets, "không có báo cáo nào để kiểm")
    json_only = "--json-only" in args
    for path, n in targets:
        if not json_only:
            check_md(path.with_suffix(".md"))
        check_report(path, n, queue)

    if fails:
        for f in fails:
            print("not ok -", f)
        print(f"# {len(fails)} mục sai")
        return 1
    print(f"report-check: PASS ({len(targets)} báo cáo, {len(queue)} hàng QUEUE phủ đủ)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
