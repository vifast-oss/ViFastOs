#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-74 — kiểm **số đo trong tài liệu** theo D2-109C + **D2-110A** + **D2-112**.
#
# Vì sao có script này: mọi con số nêu trong `docs/k6-exec.md`, `docs/k3b-core.md` §22, `STATE.md`
# và `JOURNAL/kt74-*.md` phải **xuất hiện nguyên văn trong một log ĐÃ NẰM TRONG REPO**, và mỗi
# khẳng định phải có **phạm vi log riêng** (D2-112a) — không có "haystack chung" để hoán số giữa
# hai ảnh vẫn xanh.
#
# Bốn điều kiện D2-112 (thiếu một là **không đạt**):
#   (a) mỗi khối ```text scope=<tệp-log>``` được kiểm **với đúng tệp log đó** — không nối log;
#   (b) **không** bảng ngoại lệ chứa chữ số đo; chỉ được miễn **định danh** (hex, mã task, ngày,
#       số hiệu spec `x.y`, tham chiếu tiết `§N`, `file:line`), mỗi mục có lý do ghi ngay trong
#       `ALLOW` của script;
#   (c) log **mutation / ca âm / break-test** KHÔNG được là nguồn (danh sách `FORBIDDEN` dưới);
#   (d) `STATE.md`: quét **toàn bộ** khối trạng thái (mọi fence), không chỉ fence đầu.
#
# Dùng:
#   bash v2/agent/JOURNAL/kt74-verify-numbers.sh            # kiểm cây thật
#   KT74_DOCS_ROOT=<dir> KT74_LOG_ROOT=<dir> bash …         # kiểm **bản copy** (self-break)
# Exit: 0 = mọi số khớp; 1 = có số không có nguồn (in `fail=N` + từng dòng).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
DOCS_ROOT="${KT74_DOCS_ROOT:-$V2_ROOT}"
LOG_ROOT="${KT74_LOG_ROOT:-$HERE}"

python3.12 - "$DOCS_ROOT" "$LOG_ROOT" <<'PY'
import os
import re
import sys

docs_root, log_root = sys.argv[1], sys.argv[2]

# (b) Miễn trừ **định danh** — KHÔNG phải số đo. Mỗi mục kèm lý do; không có mục nào miễn một
# con số đo được (không có "số một chữ số được miễn" như lỗi D2-112b của KT-73).
ALLOW = [
    (re.compile(r"^0x[0-9a-fA-F]+$"), "hex literal (địa chỉ/giá trị nhị phân)"),
    (re.compile(r"^KT-\d+[a-z]?$"), "mã task"),
    (re.compile(r"^D2-\d+[A-Za-z]?$"), "mã quyết định"),
    (re.compile(r"^D-\d+$"), "mã quyết định v1"),
    (re.compile(r"^K[0-9]+(\.[0-9]+)?$"), "mã phase (K3.5, K6)"),
    (re.compile(r"^v[0-9]+$"), "nhãn phiên bản (v1)"),
    (re.compile(r"^\d{4}-\d{2}-\d{2}$"), "ngày"),
    (re.compile(r"^\d{2}:\d{2}(:\d{2})?$"), "giờ trong log"),
    (re.compile(r"^§\d+(\.\d+)*$"), "tham chiếu tiết thiết kế"),
    (re.compile(r"^\d+\.\d+$"), "số hiệu method ABI `object.method` / số tiết"),
    (re.compile(r"^[A-Za-z_][A-Za-z0-9_/.-]*\.(rs|md|sh|toml|elf|log|txt):\d+(-\d+)?$"), "tham chiếu `file:line`"),
    (re.compile(r"^R\d+$"), "luật cấu trúc (R1..R5)"),
    (re.compile(r"^T-INFRA$"), "nhãn loại lỗi"),
    (re.compile(r"^(DONE|READY|BLOCKED|REVIEW|PASS|FAIL)$"), "trạng thái"),
]

# (c) Log **không** được làm nguồn: mutation/ca âm/break-test cố tình chứa giá trị sai.
FORBIDDEN = re.compile(r"(mutation|break|selfbreak|neg|ca-am|red-)", re.IGNORECASE)

# Hai loại khối, **cùng** được kiểm nguyên văn với đúng tệp log:
#   `scope=<log>`     nguồn cho **số đo của tính năng** — cấm log mutation/ca âm (D2-112c);
#   `scope-red=<log>` **trích dẫn ca âm / mutation** — bắt buộc phải có, nhưng KHÔNG được dùng làm
#                     nguồn cho số đo (script đánh dấu riêng, không trộn vào `blocks`).
FENCE = re.compile(r"^```text scope(-red)?=(\S+)\s*$")
# "Số" trong prose = token **thuần số** (có thể có , . / : %) sau khi bỏ dấu câu hai đầu.
# Token có chữ cái (`KT-74`, `main.rs:360-361`, `k6-exec.md`) là **định danh**, không phải số đo.
NUMERIC = re.compile(r"^[0-9][0-9.,/:%-]*$")

def strip_blocks(text):
    """Trả (prose_lines, blocks) với blocks = [(scope, [dòng])]."""
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
        if not tok:
            continue
        if NUMERIC.fullmatch(tok):
            out.append(tok)
    return out

def allowed(tok):
    for rx, _why in ALLOW:
        if rx.fullmatch(tok):
            return True
    return False

def read(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()

def state_block(text):
    """D2-112(d): khối trạng thái của KT-74 — **toàn bộ** phần `## KT-74` (mọi fence)."""
    start = text.find("## KT-74")
    if start < 0:
        return ""
    nxt = text.find("\n## ", start + len("## KT-74"))
    return text[start:] if nxt < 0 else text[start:nxt]

targets = [
    ("docs/k6-exec.md", os.path.join(docs_root, "docs/k6-exec.md")),
    ("docs/k3b-core.md#22", os.path.join(docs_root, "docs/k3b-core.md"), "## 22."),
    ("agent/STATE.md#KT-74", os.path.join(docs_root, "agent/STATE.md"), "## KT-74"),
]
# Mọi `JOURNAL/kt74-*.md` (nếu có) cũng phải sạch.
journal_dir = os.path.join(docs_root, "agent", "JOURNAL")
if os.path.isdir(journal_dir):
    for name in sorted(os.listdir(journal_dir)):
        if name.startswith("kt74-") and name.endswith(".md"):
            targets.append((f"agent/JOURNAL/{name}", os.path.join(journal_dir, name)))

fails = 0
checked_blocks = 0
checked_lines = 0
checked_tokens = 0

for entry in targets:
    rel, path = entry[0], entry[1]
    section = entry[2] if len(entry) > 2 else None
    if not os.path.isfile(path):
        print(f"FAIL {rel}: không thấy tệp")
        fails += 1
        continue
    text = read(path)
    if section == "## KT-74":  # chỉ phần KT-74 của STATE.md (D2-112d)
        text = state_block(text)
        if not text:
            print(f"FAIL {rel}: không thấy tiêu đề [{section}]")
            fails += 1
            continue
        section = None
    if section:
        start = text.find(section)
        if start < 0:
            print(f"FAIL {rel}: không thấy tiêu đề [{section}]")
            fails += 1
            continue
        nxt = text.find("\n## ", start + len(section))
        text = text[start:] if nxt < 0 else text[start:nxt]
    prose, blocks = strip_blocks(text)

    # (a) mỗi khối: kiểm **với đúng** tệp log của nó, từng dòng **nguyên văn**.
    for scope, lines, is_red in blocks:
        checked_blocks += 1
        if is_red:
            # Trích dẫn ca âm: **vẫn** kiểm nguyên văn, nhưng không phải nguồn số đo.
            log_path = os.path.join(log_root, scope)
            if not os.path.isfile(log_path):
                print(f"FAIL {rel}: không thấy log ca âm {scope}")
                fails += 1
                continue
            log_text = read(log_path)
            for line in lines:
                if line.strip():
                    checked_lines += 1
                    if line not in log_text:
                        print(f"FAIL {rel}: dòng ca âm không có trong {scope}: {line[:100]}")
                        fails += 1
            continue
        if FORBIDDEN.search(scope):
            print(f"FAIL {rel}: scope={scope} là log mutation/ca âm — D2-112c cấm làm nguồn")
            fails += 1
            continue
        log_path = os.path.join(log_root, scope)
        if not os.path.isfile(log_path):
            print(f"FAIL {rel}: không thấy log {scope}")
            fails += 1
            continue
        log_text = read(log_path)
        for line in lines:
            if not line.strip():
                continue
            checked_lines += 1
            if line not in log_text:
                print(f"FAIL {rel}: dòng không có trong {scope}: {line[:100]}")
                fails += 1

    # (b)+(d) prose (và **toàn bộ** STATE.md khi gọi từ đó): mọi số phải là định danh được miễn.
    for line in prose:
        # D1 (reviewer): miễn số **chỉ trong đúng dòng tiêu đề** — KHÔNG giữ cờ qua các dòng sau
        # (bản trước đặt `heading = True` một lần rồi không reset ⇒ miễn toàn bộ số ở prose, làm
        # điều (b) "cấm ngoại lệ chữ số" của D2-112 mất hiệu lực).
        is_heading = line.lstrip().startswith("#")
        for tok in numeric_tokens(line):
            checked_tokens += 1
            if allowed(tok) or (is_heading and re.fullmatch(r"\d+(\.\d+)*\.?", tok)):
                continue
            print(f"FAIL {rel}: số '{tok}' ngoài khối scope (phải là số đo trong log) — dòng: {line.strip()[:110]}")
            fails += 1

print(f"# kt74-verify-numbers: docs={len(targets)} blocks={checked_blocks} lines={checked_lines} "
      f"tokens={checked_tokens} fail={fails}")
sys.exit(0 if fails == 0 else 1)
PY
