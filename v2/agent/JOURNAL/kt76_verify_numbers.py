#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-76 — kiểm **số đo trong tài liệu** theo D2-109C + D2-110A + D2-112 + D2-112(E).
#
# Bốn điều kiện D2-112 (thiếu một là **không đạt**):
#   (a) mỗi khẳng định có **phạm vi log RIÊNG**: mỗi khối ```text scope=<tệp>``` chỉ kiểm với
#       **đúng** tệp đó, **từng dòng**; không nối thành một haystack chung;
#   (b) **không** bảng ngoại lệ chứa chữ số đo: ALLOW chỉ là **định danh** (hex, mã task/D2,
#       ngày/giờ, `§N`, `file:line`, tên arch, mã lỗi v1) — mỗi mục có lý do;
#   (c) log mutation/ca âm/đỏ trước **không** được là nguồn (`FORBIDDEN`), chỉ dùng `scope-red=`;
#   (d) `STATE.md`: quét **toàn bộ** khối trạng thái đầu (mọi dòng) **và** toàn bộ mục `## KT-76`.
#
# Ngoài ra (bổ sung của KT-76, để "nguyên văn tiêu chí" không thành lỗ hổng): chữ số nằm trong
# đoạn **“trích dẫn”** chỉ được chấp nhận khi đoạn đó là **xâu con NGUYÊN VĂN** của
# `kt76-criteria.txt`, và tệp đó phải **sinh lại y hệt** từ nguồn đọc-only (so từng byte).
#
# Dùng: python3 kt76_verify_numbers.py <v2-root>
# Exit 0 = không có số vô nguồn; 1 = có.
import os
import re
import subprocess
import sys

V2 = sys.argv[1]
JOURNAL = os.path.join(V2, "agent", "JOURNAL")

ALLOW = [
    (re.compile(r"^0x[0-9a-fA-F_]+$"), "hex literal (địa chỉ/giá trị nhị phân)"),
    (re.compile(r"^KT-\d+[a-z]?$"), "mã task"),
    (re.compile(r"^D2-\d+[A-Za-z]?$"), "mã quyết định v2"),
    (re.compile(r"^D-\d+$"), "mã quyết định v1"),
    (re.compile(r"^K[0-9]+(\.[0-9]+)?$"), "mã mốc (K6, K3.5)"),
    (re.compile(r"^v[0-9]+$"), "nhãn phiên bản (v1/v2)"),
    (re.compile(r"^\d{4}-\d{2}-\d{2}$"), "ngày"),
    (re.compile(r"^\d{2}:\d{2}(:\d{2})?$"), "giờ"),
    (re.compile(r"^§\d+(\.\d+)*$"), "tham chiếu tiết"),
    (re.compile(r"^[A-Za-z_][A-Za-z0-9_/.-]*\.(rs|md|sh|toml|elf|log|txt|py|json|vidl):\d+(-\d+)?$"),
     "tham chiếu `file:line`"),
    (re.compile(r"^[A-Za-z][A-Za-z0-9_/-]*\.\d+(\.\d+)*$"), "định danh có tiền tố chữ + số hiệu"),
    (re.compile(r"^R\d+$"), "luật cấu trúc (R1..R5)"),
    (re.compile(r"^T-INFRA$"), "nhãn loại lỗi"),
    (re.compile(r"^(DONE|READY|BLOCKED|REVIEW|PASS|FAIL)$"), "trạng thái"),
    # (b) mã lỗi v1 / audit v1 / tên kiến trúc — **định danh**, không phải số đo.
    (re.compile(r"^T\d+[a-z]?$"), "mã lỗi v1 (T26/T31/T33a)"),
    (re.compile(r"^P\d-\d+$"), "mã audit v1 (P1-5)"),
    (re.compile(r"^Q\d-[A-Z]$"), "mã câu hỏi v1 (Q1-A)"),
    (re.compile(r"^(x86_64|aarch64|riscv64|arm64|i386)$"), "tên kiến trúc"),
]

# (c) Không được làm nguồn số đo.
FORBIDDEN = re.compile(r"(mutation|break|selfbreak|neg|ca-am|red-)", re.IGNORECASE)

FENCE = re.compile(r"^```text scope(-red)?=(\S+)\s*$")
NUMERIC = re.compile(r"^[0-9][0-9.,/:%-]*$")
QUOTED = re.compile(r"“([^”]+)”")


def strip_blocks(text):
    prose, blocks = [], []
    cur = None
    for line in text.splitlines():
        if cur is None:
            m = FENCE.match(line.strip())
            if m:
                cur = (m.group(2), [], bool(m.group(1)))
                continue
            prose.append(line)
        else:
            if line.strip() == "```":
                blocks.append(cur)
                cur = None
            else:
                cur[1].append(line.rstrip())
    if cur is not None:
        raise SystemExit("FAIL: khối scope chưa đóng")
    return prose, blocks


def numeric_tokens(line):
    out = []
    for raw in line.split():
        tok = raw.strip("`*_()[]{}<>\"'.,;:!?|")
        if tok and NUMERIC.fullmatch(tok):
            out.append(tok)
    return out


def allowed(tok):
    return any(rx.fullmatch(tok) for rx, _why in ALLOW)


def read(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()


criteria_text = read(os.path.join(JOURNAL, "kt76-criteria.txt"))

# Tiêu chí phải **sinh lại y hệt** từ nguồn đọc-only (D2-110A: không chép tay).
fresh = subprocess.run(
    [sys.executable, os.path.join(JOURNAL, "kt76_extract_criteria.py"), os.path.dirname(V2)],
    capture_output=True,
    text=True,
    check=False,
)
if fresh.returncode != 0:
    print(f"FAIL: sinh lại tiêu chí lỗi: {fresh.stderr.strip()[:200]}")
    sys.exit(1)
if fresh.stdout != criteria_text:
    print("FAIL: kt76-criteria.txt KHÁC bản sinh lại từ nguồn đọc-only (tiêu chí bị sửa)")
    sys.exit(1)

targets = [
    ("agent/reports/K6-DOD.md", os.path.join(V2, "agent/reports/K6-DOD.md"), None),
    ("docs/k3b-core.md#24", os.path.join(V2, "docs/k3b-core.md"), "## 24."),
    ("agent/STATE.md#top+KT-76", os.path.join(V2, "agent/STATE.md"), "STATE"),
    ("agent/BLOCKED.md#KT-76", os.path.join(V2, "agent/BLOCKED.md"), "## KT-76"),
    ("agent/JOURNAL/kt76-journal.md", os.path.join(V2, "agent/JOURNAL/kt76-journal.md"), None),
]

fails = 0
n_blocks = 0
n_lines = 0
n_tokens = 0
n_cite = 0


def state_top_block(text):
    start = text.find("\n```\n")
    if start < 0:
        return ""
    start += 1
    end = text.find("\n```", start + 4)
    return text[start:] if end < 0 else text[start:end + 4]


def section(text, heading):
    start = text.find(heading)
    if start < 0:
        return ""
    nxt = text.find("\n## ", start + len(heading))
    return text[start:] if nxt < 0 else text[start:nxt]


def check_text(rel, text):
    global fails, n_blocks, n_lines, n_tokens, n_cite
    prose, blocks = strip_blocks(text)
    for scope, lines, is_red in blocks:
        n_blocks += 1
        log_path = os.path.join(JOURNAL, scope)
        if not os.path.isfile(log_path):
            print(f"FAIL {rel}: không thấy log {scope} trong JOURNAL")
            fails += 1
            continue
        if is_red:
            if not FORBIDDEN.search(scope):
                print(f"FAIL {rel}: scope-red={scope} không phải log ca âm/mutation")
                fails += 1
                continue
        elif FORBIDDEN.search(scope):
            print(f"FAIL {rel}: scope={scope} là log ca âm/mutation — D2-112c cấm làm nguồn")
            fails += 1
            continue
        log_text = read(log_path)
        for line in lines:
            if not line.strip():
                continue
            n_lines += 1
            if line not in log_text:
                kind = "ca âm" if is_red else "log"
                print(f"FAIL {rel}: dòng không có trong {kind} {scope}: {line[:110]}")
                fails += 1
        # khối tiêu chí: phải bằng **toàn bộ** tệp (không trích lẻ để né số)
        if scope == "kt76-criteria.txt":
            if "\n".join(lines) != criteria_text.rstrip("\n"):
                print(f"FAIL {rel}: khối scope=kt76-criteria.txt không bằng toàn bộ tệp tiêu chí")
                fails += 1
    for line in prose:
        is_heading = line.lstrip().startswith("#")
        quotes = [q for q in QUOTED.findall(line) if len(q.replace(" ", "")) >= 4]
        # Đoạn trích dẫn hợp lệ (xâu con NGUYÊN VĂN của tiêu chí) được **bỏ khỏi** dòng trước khi
        # bắt token ⇒ không thể "mượn" một trích dẫn hợp lệ để hợp thức hoá số khác cùng dòng.
        scan = line
        for q in quotes:
            if q in criteria_text:
                scan = scan.replace(q, " ")
                n_cite += 1
        # Tên vai (**Agent 2** / **Agent 0**) là **định danh**, không phải số đo.
        scan = re.sub(r"Agent[ ]+\d+", "Agent", scan)
        for tok in numeric_tokens(scan):
            n_tokens += 1
            if allowed(tok):
                continue
            # (C — reviewer KT-76) Heading **không** được miễn số đo. Chỉ tha **số hiệu tiết** nằm
            # NGAY ĐẦU tiêu đề (ngay sau các dấu #), mỗi thành phần ≤ 2 chữ số — định danh cấu trúc.
            # `## 98765` KHÔNG khớp (5 chữ số) ⇒ đỏ; đó là ca tự phá số 5.
            if is_heading:
                hs = re.match(r"^#{1,6}\s+(\d{1,2}(?:\.\d{1,2})*)\.?(?:\s|$)", line.strip())
                if hs and tok.rstrip(".") == hs.group(1):
                    continue
            print(f"FAIL {rel}: số '{tok}' ngoài khối scope (phải là số đo trong log) — dòng: {line.strip()[:110]}")
            fails += 1
    return 0


for rel, path, section_heading in targets:
    if not os.path.isfile(path):
        print(f"FAIL {rel}: không thấy tệp")
        fails += 1
        continue
    text = read(path)
    if section_heading == "STATE":
        top = state_top_block(text)
        if not top:
            print(f"FAIL {rel}: không thấy khối trạng thái đầu")
            fails += 1
        else:
            check_text(rel + "#top", top)
        kt76 = section(text, "## KT-76")
        if not kt76:
            print(f"FAIL {rel}: không thấy tiêu đề [## KT-76]")
            fails += 1
        else:
            check_text(rel + "#KT-76", kt76)
        continue
    if section_heading:
        part = section(text, section_heading)
        if not part:
            print(f"FAIL {rel}: không thấy tiêu đề [{section_heading}]")
            fails += 1
            continue
        check_text(rel, part)
        continue
    check_text(rel, text)

print(
    f"# kt76-verify-numbers: docs={len(targets)} blocks={n_blocks} lines={n_lines} "
    f"tokens={n_tokens} cite={n_cite} fail={fails}"
)
sys.exit(0 if fails == 0 else 1)
