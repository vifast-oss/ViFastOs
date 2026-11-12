#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Máy kiểm báo cáo vòng của dòng ViScript (Khối E, `viscript/agent/QUEUE.md`).

Mọi sự thật trong báo cáo phải đối chiếu được với máy: `head` tồn tại + là tổ tiên HEAD;
mọi `commits[]` tồn tại; `status` khớp QUEUE **hoặc** có trường `queue` ghi đúng giá trị QUEUE
(ghi cả hai khi sổ chưa lật); phủ đủ mọi hàng QUEUE; `blocked[]` phủ mọi hàng BLOCKED/REVIEW;
YC phải có thật trong `v2/agent/BLOCKED.md`; `project == "viscript"`.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REPORTS = ROOT / "viscript/agent/reports"
QUEUE = ROOT / "viscript/agent/QUEUE.md"
BLOCKED = ROOT / "v2/agent/BLOCKED.md"

KEYS = {"project", "round", "head", "build_id", "tasks", "gates",
        "sim", "coverage", "mutants", "perf", "blocked"}
STATUSES = {"READY", "BLOCKED", "REVIEW", "DONE"}
fails: list[str] = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def git_ok(sha: str) -> bool:
    if not re.fullmatch(r"[0-9a-f]{7,40}", sha or ""):
        return False
    return subprocess.run(["git", "cat-file", "-e", f"{sha}^{{commit}}"],
                          cwd=ROOT, capture_output=True, check=False).returncode == 0


def split_row(line: str) -> list[str]:
    out, cur, i = [], "", 0
    line = line.strip()
    body = line[1:] if line.startswith("|") else line
    body = body[:-1] if body.endswith("|") else body
    while i < len(body):
        if body[i] == "\\" and i + 1 < len(body) and body[i + 1] == "|":
            cur += "|"; i += 2
        elif body[i] == "|":
            out.append(cur.strip()); cur = ""; i += 1
        else:
            cur += body[i]; i += 1
    out.append(cur.strip())
    return out


def queue_at(head: str) -> dict[str, str]:
    out = subprocess.run(["git", "show", f"{head}:viscript/agent/QUEUE.md"],
                         cwd=ROOT, capture_output=True, text=True, check=False)
    if out.returncode != 0:
        return {}
    rows = {}
    for line in out.stdout.splitlines():
        t = line.strip()
        if t.startswith("| ST-") or t.startswith("| KT-"):
            c = split_row(t)
            if len(c) >= 6:
                rows[c[0]] = c[4].strip("`* ")
    return rows


def queue_statuses() -> dict[str, str]:
    rows = {}
    for line in QUEUE.read_text().splitlines():
        t = line.strip()
        if t.startswith("| ST-") or t.startswith("| KT-"):
            c = split_row(t)
            if len(c) >= 6:
                rows[c[0]] = c[4].strip("`* ")
    return rows


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
    check(set(data) == KEYS, f"{tag}: phải ĐÚNG 11 khoá (D2-39), thừa/thiếu: {sorted(set(data) ^ KEYS)}")
    check(data.get("project") == "viscript", f"{tag}: project phải 'viscript'")
    check(data.get("round") == round_no, f"{tag}: round phải {round_no}")

    head = str(data.get("head", ""))
    check(git_ok(head), f"{tag}: head {head!r} không tồn tại trong git")
    if git_ok(head):
        anc = subprocess.run(["git", "merge-base", "--is-ancestor", head, "HEAD"],
                             cwd=ROOT, check=False)
        check(anc.returncode == 0, f"{tag}: head {head} không phải tổ tiên HEAD")

    gates = data.get("gates") or {}
    for k in ("fast", "full", "nightly"):
        check(gates.get(k) in {"PASS", "FAIL", "—"}, f"{tag}: gates.{k} lạ")

    sim = data.get("sim") or {}
    check(isinstance(sim.get("seeds"), int), f"{tag}: sim.seeds phải số nguyên")
    check(isinstance(sim.get("failures"), int), f"{tag}: sim.failures phải số nguyên")

    cov = data.get("coverage") or {}
    check("tokens" in cov and "fixtures" in cov, f"{tag}: coverage phải có tokens+fixtures")

    mut = data.get("mutants") or {}
    check(isinstance(mut.get("measured"), bool), f"{tag}: mutants.measured phải bool")
    for k in ("killed", "alive"):
        v = mut.get(k, "missing")
        check(v is None or isinstance(v, int), f"{tag}: mutants.{k} phải số hoặc null")

    check(isinstance(data.get("perf"), dict), f"{tag}: perf phải object")

    tasks = data.get("tasks") or []
    seen: dict[str, str] = {}
    for t in tasks:
        tid = t.get("id")
        check(tid in queue, f"{tag}: task {tid!r} không có trong QUEUE")
        st = t.get("status")
        check(st in STATUSES, f"{tag}: {tid} status ngoài 4 trạng thái")
        if tid in queue:
            if st == queue[tid]:
                pass
            else:
                # Sổ chưa lật: được phép lệch CHỈ KHI ghi đúng giá trị QUEUE vào trường `queue`.
                check(t.get("queue") == queue[tid],
                      f"{tag}: {tid} status {st!r} ≠ QUEUE {queue[tid]!r} (thiếu/sai trường 'queue')")
        for c in t.get("commits") or []:
            check(git_ok(c), f"{tag}: {tid} commit {c!r} không tồn tại trong git")
        check(tid not in seen, f"{tag}: task {tid} trùng")
        seen[tid] = st

    missing = sorted(set(queue) - set(seen))
    check(not missing, f"{tag}: thiếu task trong báo cáo: {missing}")
    check(not sorted(set(seen) - set(queue)), f"{tag}: task lạ ngoài QUEUE")

    blocked = data.get("blocked") or []
    btext = BLOCKED.read_text() + QUEUE.read_text()
    for b in blocked:
        check(bool(b.get("id")), f"{tag}: blocked có mục thiếu id")
        check(bool(b.get("question")), f"{tag}: blocked {b.get('id')} thiếu question")
        if str(b.get("id", "")).startswith("YC-"):
            check(b["id"] in btext, f"{tag}: {b['id']} không có trong BLOCKED.md")
    ids = {b.get("id") for b in blocked}
    for tid, st in queue.items():
        if st in {"BLOCKED", "REVIEW"}:
            check(tid in ids, f"{tag}: {tid} ({st}) không xuất hiện trong blocked[]")


def rounds() -> list[int]:
    out = []
    for p in REPORTS.glob("R*-viscript.json"):
        m = re.fullmatch(r"R(\d+)-viscript\.json", p.name)
        if m:
            out.append(int(m.group(1)))
    return sorted(out)


def main() -> int:
    args = sys.argv[1:]
    queue = queue_statuses()
    check(queue, "QUEUE: không đọc được hàng nào")
    targets = []
    if "--file" in args:
        p = Path(args[args.index("--file") + 1])
        targets.append((p, int(json.loads(p.read_text()).get("round", 1))))
    elif "--all" in args:
        for n in rounds():
            targets.append((REPORTS / f"R{n}-viscript.json", n))
    else:
        n = int(args[args.index("--n") + 1]) if "--n" in args else 1
        targets.append((REPORTS / f"R{n}-viscript.json", n))
    json_only = "--json-only" in args
    check(targets, "không có báo cáo nào để kiểm")
    for path, n in targets:
        if not json_only:
            check_md(path.with_suffix(".md"))
        head = json.loads(path.read_text()).get("head", "")
        check_report(path, n, queue_at(head) or queue)
    if fails:
        for f in fails:
            print("not ok -", f)
        print(f"# {len(fails)} mục sai")
        return 1
    print(f"report-check: PASS ({len(targets)} báo cáo, {len(queue)} hàng QUEUE phủ đủ)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
