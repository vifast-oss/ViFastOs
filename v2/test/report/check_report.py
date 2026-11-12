#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-R1 — kiểm báo cáo `R<n>-v2.md` + `.json` theo Khối E (QUEUE.md §Khối E).

Đỏ trước: viết khi `v2/agent/reports/` chưa có báo cáo nào.
Dùng: `python3 check_report.py [--dir v2/agent/reports] [--n 1]`
"""

import json
import pathlib
import re
import sys

KEYS = ["project", "round", "head", "build_id", "tasks", "gates", "sim",
        "coverage", "mutants", "perf", "blocked"]
MD_SECTIONS = ["## 1.", "## 2.", "## 3.", "## 4.", "## 5.", "## 6."]


def main():
    repo = pathlib.Path(__file__).resolve().parents[3]
    d = repo / "v2/agent/reports"
    n = 1
    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--n":
            n = int(args[i + 1]); i += 2
        elif args[i] == "--dir":
            d = pathlib.Path(args[i + 1]); i += 2
        else:
            print(f"tham số lạ: {args[i]}")
            return 2

    fails = []
    md_path, js_path = d / f"R{n}-v2.md", d / f"R{n}-v2.json"
    if not md_path.exists():
        print(f"report-check: KHÔNG có {md_path.relative_to(repo)}")
        return 1
    if not js_path.exists():
        print(f"report-check: KHÔNG có {js_path.relative_to(repo)}")
        return 1

    md = md_path.read_text()
    for sec in MD_SECTIONS:
        if sec not in md:
            fails.append(f"MD thiếu mục {sec}")

    try:
        doc = json.loads(js_path.read_text())
    except json.JSONDecodeError as exc:
        print(f"report-check: JSON không parse được: {exc}")
        return 1

    for key in KEYS:
        if key not in doc:
            fails.append(f"JSON thiếu khoá {key!r}")
    if doc.get("project") != "v2":
        fails.append('JSON "project" phải = "v2" (Agent 0 trả lại nếu thiếu)')
    if doc.get("round") != n:
        fails.append(f'JSON "round" phải = {n}')

    tasks = doc.get("tasks") or []
    if not tasks:
        fails.append("JSON tasks rỗng")
    for t in tasks:
        if not isinstance(t, dict):
            fails.append("tasks phải là object")
            continue
        for key in ("id", "status", "commits", "gates"):
            if key not in t:
                fails.append(f"task {t.get('id')} thiếu {key!r}")
        if t.get("status") not in ("DONE", "REVIEW", "BLOCKED", "READY"):
            fails.append(f"task {t.get('id')} trạng thái lạ {t.get('status')!r}")

    for b in doc.get("blocked") or []:
        if not isinstance(b, dict) or "id" not in b or "question" not in b:
            fails.append("blocked[] phải có id + question")

    # đối chiếu sổ: mọi task báo cáo phải có trong QUEUE (không tự đặt ID)
    queue = (repo / "v2/agent/QUEUE.md").read_text()
    ids_in_queue = set(re.findall(r"^\| (KT-[A-Za-z0-9]+) \|", queue, re.M))
    for t in tasks:
        if t.get("id") not in ids_in_queue:
            fails.append(f"task {t.get('id')} không có trong QUEUE")

    # blocked[] phải trỏ YC có thật trong BLOCKED.md
    blk = (repo / "v2/agent/BLOCKED.md").read_text()
    for b in doc.get("blocked") or []:
        if b.get("id", "").startswith("YC-") and b["id"] not in blk:
            fails.append(f"blocked {b['id']} không có trong BLOCKED.md")

    if fails:
        for msg in fails:
            print(f"not ok - {msg}")
        print(f"# {len(fails)} mục sai")
        return 1
    print(f"report-check: PASS ({js_path.name}: {len(tasks)} task, {len(doc.get('blocked') or [])} blocked)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
