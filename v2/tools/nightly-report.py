#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-333b — sinh & đọc `STATUS.md` của `v2 nightly` (QD Q12: số đo do máy sinh).

Vì sao có tệp này
-----------------
`QD §9 Q12` chốt: **chỉ** `./v2/bin/v2 nightly` sinh số đo; `STATUS.md` là nguồn sự thật, tài
liệu khác không được chứa số đo viết tay. `KT-80` (luật máy 1) cấm số ngoài khối
```` ```text scope=<log> ```` trong `v2/agent/reports/*.md`. Vì vậy tệp này sinh `STATUS.md` theo
đúng khuôn đó: **mọi** số đo nằm trong **một** khối `scope=` trích **nguyên văn** từ log tóm tắt
mà chính lần chạy ghi ra (`v2/agent/JOURNAL/nightly/<run-id>/summary.log`). Prose chỉ còn trạng
thái (`PASS`/`FAIL`) và tên tệp — không có số nào để "viết tay".

Luồng dữ liệu
-------------
`v2 nightly ...` (bash) ghi `manifest.tsv` cạnh log thô, rồi gọi:
    nightly-report.py render --manifest M.tsv --summary S.log --out STATUS.md --root .
Sau đó `./v2/bin/v2 nightly --status` chỉ **đọc** (không chạy lại):
    nightly-report.py show --status MD [--manifest M.tsv]
`--selftest` của `v2 nightly` dùng `verify` để chứng minh khối `scope=` trích đúng nguyên văn và
prose không lọt số đo.

Dùng:
    python3.12 v2/tools/nightly-report.py render --manifest F --summary F --out F [--root D]
    python3.12 v2/tools/nightly-report.py show --status F [--manifest F]
    python3.12 v2/tools/nightly-report.py verify --status F [--root D]
Exit: 0 = xanh/đọc được; 1 = hỏng (`STATUS_MALFORMED`, `STATUS_NOT_VERBATIM`,
`STATUS_NUMBER_OUTSIDE`); 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import os
import re
import sys

FENCE = re.compile(r"^```text[ \t]+scope=(\S+)[ \t]*$")
NUMERIC = re.compile(r"^[0-9][0-9.,/:%-]*$")
IDENT_OK = [
    re.compile(r"^v[0-9]+$"),
    re.compile(r"^L[0-9IVX]+$"),
    re.compile(r"^§\d+(\.\d+)*$"),
    re.compile(r"^\d{4}-\d{2}-\d{2}$"),
    re.compile(r"^\d{2}:\d{2}(:\d{2})?$"),
    re.compile(r"^0x[0-9a-fA-F_]+$"),
    re.compile(r"^(PASS|FAIL|OK|RED|GREEN|DRIFT|DONE|READY|BLOCKED|REVIEW)$"),
]


def _fail(code: str, msg: str) -> int:
    print(f"{code}: {msg}", file=sys.stderr)
    return 1


def load_manifest(path: str) -> dict:
    meta: dict = {}
    steps: list = []
    with open(path, encoding="utf-8") as fh:
        for lineno, line in enumerate(fh, 1):
            line = line.rstrip("\n")
            if not line or line.startswith("#!"):
                continue
            if line.startswith("#key\t"):
                _, key, val = line.split("\t", 2)
                meta[key] = val
            elif line.startswith("#step\t"):
                row = line.split("\t")
                if len(row) != 7:
                    raise ValueError(f"{path}:{lineno}: dòng #step phải 7 cột, nhận {len(row)}")
                _, idx, phase, label, rc, wall, log = row
                steps.append({"idx": int(idx), "phase": phase, "label": label,
                              "rc": int(rc), "wall_ms": int(wall), "log": log})
            else:
                raise ValueError(f"{path}:{lineno}: dòng lạ: {line[:80]}")
    for key in ("run_id", "head", "dirty", "flags", "started", "ended", "result"):
        if key not in meta:
            raise ValueError(f"{path}: thiếu khoá `{key}`")
    return {"meta": meta, "steps": steps}


def summary_text(m: dict) -> str:
    meta = m["meta"]
    lines = [
        f"# nightly run={meta['run_id']} head={meta['head']} dirty={meta['dirty']}",
        f"# flags={meta['flags']}",
        f"# started={meta['started']} ended={meta['ended']}",
    ]
    for s in m["steps"]:
        lines.append(f"# step={s['phase']} rc={s['rc']} wall_ms={s['wall_ms']} "
                     f"label={s['label']} log={s['log']}")
    lines.append(f"# result={meta['result']}")
    return "\n".join(lines) + "\n"


def status_text(m: dict) -> str:
    meta = m["meta"]
    result = meta["result"]
    return "\n".join([
        "<!-- SINH TỰ ĐỘNG bởi `./v2/bin/v2 nightly` — KHÔNG SỬA TAY (QD Q12). -->",
        "<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->",
        "",
        "# Trạng thái nightly — ViFastOs v2",
        "",
        f"- kết quả: {result}",
        "- nguồn số đo: khối `scope=` bên dưới, trích nguyên văn từ log tóm tắt của chính lần chạy",
        "  (`v2/agent/JOURNAL/nightly/<run-id>/summary.log`); commit log cùng tệp này.",
        "",
        f"```text scope=nightly/{meta['run_id']}/summary.log",
        summary_text(m).rstrip("\n"),
        "```",
        "",
    ])


def parse_status(text: str) -> tuple:
    """Trả (run_id, scope, fence_lines, result). Ném ValueError nếu tệp không đúng khuôn."""
    lines = text.splitlines()
    result = None
    scope = None
    fence: list = []
    inside = False
    for line in lines:
        if not inside:
            m = FENCE.match(line.strip())
            if m:
                scope = m.group(1)
                inside = True
                continue
            mm = re.match(r"^-\s+kết quả:\s*(\S+)\s*$", line)
            if mm:
                result = mm.group(1)
            continue
        if line.strip() == "```":
            inside = False
            continue
        fence.append(line)
    if scope is None:
        raise ValueError("thiếu khối ```text scope=<log>")
    if result is None:
        raise ValueError("thiếu dòng `- kết quả: PASS|FAIL`")
    m_run = re.match(r"^nightly/(.+/summary\.log)$", scope)
    if not m_run:
        raise ValueError(f"scope phải là `nightly/<run-id>/summary.log`, nhận `{scope}`")
    return m_run.group(1).split("/")[0], scope, fence, result


def resolve_scope(root: str, scope: str) -> str:
    for cand in (os.path.join(root, scope), os.path.join(root, "v2", "agent", "JOURNAL", scope)):
        if os.path.isfile(cand):
            return cand
    return ""


def cmd_render(args) -> int:
    try:
        m = load_manifest(args.manifest)
    except (OSError, ValueError) as exc:
        return _fail("STATUS_MALFORMED", str(exc))
    root = os.path.abspath(args.root or os.getcwd())
    summary = os.path.abspath(args.summary)
    # Scope phải trỏ được từ gốc repo: log tóm tắt nằm dưới `v2/agent/JOURNAL/nightly/`.
    journal = os.path.join(root, "v2", "agent", "JOURNAL")
    if not summary.startswith(os.path.join(journal, "nightly") + os.sep):
        return _fail("STATUS_MALFORMED",
                     f"summary `{summary}` phải nằm dưới `{journal}/nightly/` (để `scope=` hợp lệ)")
    os.makedirs(os.path.dirname(summary), exist_ok=True)
    with open(summary, "w", encoding="utf-8") as fh:
        fh.write(summary_text(m))
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write(status_text(m))
    print(f"nightly-report: đã ghi `{os.path.relpath(summary, root)}` + `{args.out}` "
          f"({len(m['steps'])} bước, kết quả {m['meta']['result']})")
    return 0


def cmd_show(args) -> int:
    try:
        text = open(args.status, encoding="utf-8").read()
        run_id, scope, fence, result = parse_status(text)
    except OSError:
        print(f"v2 nightly --status: chưa có bằng chứng (`{args.status}` không tồn tại) — "
              f"chưa chạy nightly lần nào hoặc bằng chứng chưa được commit.")
        return 0
    except ValueError as exc:
        return _fail("STATUS_MALFORMED", str(exc))
    print("== v2 nightly --status (chỉ đọc bằng chứng; KHÔNG chạy lại) ==")
    print(f"   run: {run_id}")
    print(f"   scope: {scope}")
    print(f"   kết quả: {result}")
    for line in fence:
        print(f"   {line}")
    if args.manifest and os.path.isfile(args.manifest):
        try:
            m = load_manifest(args.manifest)
        except (OSError, ValueError) as exc:
            return _fail("STATUS_MALFORMED", str(exc))
        print("   bảng bước (từ manifest cục bộ):")
        for s in m["steps"]:
            print(f"   {s['idx']:>2}. {s['phase']:<9} rc={s['rc']} wall_ms={s['wall_ms']:<8} {s['label']}")
    return 0


def cmd_verify(args) -> int:
    root = os.path.abspath(args.root or os.getcwd())
    try:
        text = open(args.status, encoding="utf-8").read()
        _run_id, scope, fence, _result = parse_status(text)
    except OSError as exc:
        return _fail("STATUS_MALFORMED", str(exc))
    except ValueError as exc:
        return _fail("STATUS_MALFORMED", str(exc))
    summary = resolve_scope(root, scope)
    if not summary:
        return _fail("STATUS_NOT_VERBATIM", f"không thấy log `{scope}` để đối chiếu")
    log_lines = set(open(summary, encoding="utf-8").read().splitlines())
    for i, line in enumerate(fence, 1):
        if line.strip() and line not in log_lines:
            return _fail("STATUS_NOT_VERBATIM",
                         f"dòng {i} của khối `scope=` không có nguyên văn trong `{scope}`: {line[:100]}")
    # prose (ngoài khối) không được có số đo — cùng tinh thần luật máy 1 (KT-80).
    inside = False
    for lineno, line in enumerate(text.splitlines(), 1):
        if FENCE.match(line.strip()):
            inside = True
            continue
        if inside and line.strip() == "```":
            inside = False
            continue
        if inside:
            continue
        for raw in line.split():
            tok = raw.strip("`*_()[]{}<>\"'.,;:!?|")
            if not tok or not NUMERIC.fullmatch(tok):
                continue
            if any(rx.fullmatch(tok) for rx in IDENT_OK):
                continue
            return _fail("STATUS_NUMBER_OUTSIDE",
                         f"dòng {lineno}: số '{tok}' ngoài khối `scope=`")
    print(f"nightly-report: PASS (khối `scope={scope}` trích nguyên văn; prose không có số đo)")
    return 0


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("render")
    p.add_argument("--manifest", required=True)
    p.add_argument("--summary", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--root", default=None)
    p.set_defaults(fn=cmd_render)

    p = sub.add_parser("show")
    p.add_argument("--status", required=True)
    p.add_argument("--manifest", default=None)
    p.set_defaults(fn=cmd_show)

    p = sub.add_parser("verify")
    p.add_argument("--status", required=True)
    p.add_argument("--root", default=None)
    p.set_defaults(fn=cmd_verify)

    args = ap.parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
