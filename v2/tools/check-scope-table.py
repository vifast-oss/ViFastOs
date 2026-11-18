#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-134 (C7 của Agent 0) — kiểm **máy** bảng đối chiếu số ↔ khối `scope=`.

Vì sao có tệp này: KT-133 thêm vào ba tài liệu K6 một bảng nói "trường đo X có số nằm trong khối
`scope=` Y". Nhưng một bảng **không được kiểm** thì đúng lớp lỗi đã gặp nhiều lần trong repo: tài
liệu khai một đằng, log một nẻo, và không ai phát hiện. Agent 0 chốt: **không có kiểm máy thì Q6
coi như chưa xong**.

Tệp này làm đúng ba việc:

1. **Cấu trúc** — mỗi dòng bảng phải trỏ tới một khối `scope=`/`scope-red=` **có thật** trong chính
   tài liệu đó, và trường đo phải xuất hiện **kèm giá trị số** trong khối ấy.
2. **Ngữ nghĩa** — nếu văn xuôi nói "`trường` bằng <chữ số>", chữ số đó phải **khớp** giá trị trong
   khối. Lệch ⇒ đỏ. Đây là vế bắt được "sửa một số trong log mà quên sửa văn xuôi".
3. **Đột biến** (`--selftest`) — dựng tài liệu giả rồi phá từng bất biến, mỗi ca phải đỏ **kèm đúng
   mã lý do**; cộng một ca dương (tài liệu đúng phải xanh) để một checker hỏng-luôn-đỏ không qua được.

Chạy: `python3 v2/tools/check-scope-table.py [--root R] [--selftest] [--quiet]`
Exit: 0 = xanh, 1 = có lỗi, 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import tempfile

# Tài liệu có bảng đối chiếu (KT-133). Thêm tệp mới thì thêm ở đây **và** thêm ca selftest.
DOCS = [
    "v2/docs/k6-exec.md",
    "v2/docs/k6-persist.md",
    "v2/agent/reports/K6-DOD.md",
]

# Chữ số tiếng Việt dùng trong văn xuôi của ba tài liệu này.
WORDS = {
    "không": 0,
    "một": 1,
    "hai": 2,
    "ba": 3,
    "bốn": 4,
    "năm": 5,
    "sáu": 6,
    "bảy": 7,
    "tám": 8,
    "chín": 9,
    "mười": 10,
    "mười một": 11,
    "mười hai": 12,
    "mười bốn": 14,
    "mười bảy": 17,
    "hai mươi mốt": 21,
    "hai mươi bảy": 27,
}

TABLE_HEADER = "Bảng đối chiếu số ↔ khối"
ROW_RE = re.compile(
    r"^\|\s*`([A-Za-z_][A-Za-z0-9_.]*)`\s*\|\s*([^|`]+?)\s*\|\s*`(scope(?:-red)?)=`\s*\|\s*`([^`]+)`\s*\|\s*$"
)
BLOCK_RE = re.compile(r"^```text (scope(?:-red)?)=(\S+)\s*$")
FENCE_RE = re.compile(r"^```\s*$")


class Fail(Exception):
    """Một lỗi có mã — mã dùng cho ca âm của selftest."""

    def __init__(self, code: str, detail: str) -> None:
        super().__init__(f"{code}: {detail}")
        self.code = code
        self.detail = detail


def split_blocks(text: str) -> tuple[str, list[tuple[str, str, str]]]:
    """Trả (văn xuôi không có khối code, [(loại, đường-dẫn-log, nội-dung)])."""
    prose: list[str] = []
    blocks: list[tuple[str, str, str]] = []
    cur: tuple[str, str] | None = None
    buf: list[str] = []
    for line in text.splitlines():
        if cur is None:
            m = BLOCK_RE.match(line)
            if m:
                cur = (m.group(1), m.group(2))
                buf = []
                continue
            prose.append(line)
        else:
            if FENCE_RE.match(line):
                blocks.append((cur[0], cur[1], "\n".join(buf)))
                cur = None
                buf = []
                continue
            buf.append(line)
    if cur is not None:  # khối không đóng
        raise Fail("E_BLOCK_UNCLOSED", f"khối {cur[0]}={cur[1]} không có dòng đóng ```")
    return "\n".join(prose), blocks


def parse_table(prose: str) -> list[tuple[str, str, str, str]]:
    """Trả [(trường, giá-trị-chữ, loại-khối, tệp-log)]; rỗng nếu không có bảng."""
    lines = prose.splitlines()
    start = None
    for i, line in enumerate(lines):
        if TABLE_HEADER in line:
            start = i
            break
    if start is None:
        return []
    rows: list[tuple[str, str, str, str]] = []
    for line in lines[start:]:
        m = ROW_RE.match(line.strip())
        if m:
            rows.append((m.group(1), m.group(2).strip(), m.group(3), m.group(4)))
        elif rows and line.strip().startswith("|"):
            continue
        elif rows and not line.strip():
            break
    return rows


def field_value(block_text: str, field: str) -> int | None:
    """Giá trị số của `field=` trong khối; None nếu không có."""
    m = re.search(r"(?<![A-Za-z0-9_])" + re.escape(field) + r"=(-?\d+)(?![0-9])", block_text)
    if not m:
        return None
    return int(m.group(1))


def prose_claims(prose: str, field: str) -> list[int]:
    """Các giá trị mà văn xuôi KHẲNG ĐỊNH cho trường: "`field` … bằng <chữ số>"."""
    out: list[int] = []
    pat = re.compile(
        r"`" + re.escape(field) + r"`[^`\n]{0,60}?\bbằng\s+(" + "|".join(
            sorted((re.escape(w) for w in WORDS), key=len, reverse=True)
        ) + r")\b"
    )
    for m in pat.finditer(prose):
        out.append(WORDS[m.group(1)])
    return out


# Từ khoá báo hiệu câu văn nói về **tình huống khác** (nhánh âm, giả thuyết, ca đỏ) chứ không
# phải giá trị đo của khối đang xét. Câu có một trong các từ này thì **không** đem so.
QUALIFIERS = (
    "nhánh",
    "ca âm",
    "ca đỏ",
    "chưa có",
    "nếu",
    "đột biến",
    "mong",
    "phải là",
    "sẽ",
    "vắng",
    "bỏ ",
    "tăng gấp đôi",
    "khác không",
    "đỏ",
)


def _sentence_of(prose: str, pos: int) -> str:
    """Câu chứa vị trí `pos` (cắt theo dấu chấm/xuống dòng)."""
    start = max(prose.rfind("\n", 0, pos), prose.rfind(". ", 0, pos))
    end = prose.find("\n", pos)
    if end < 0:
        end = len(prose)
    return prose[start + 1 : end]


def check_doc(path: str) -> list[str]:
    """Trả danh sách lỗi (rỗng = xanh)."""
    text = open(path, encoding="utf-8").read()
    prose, blocks = split_blocks(text)
    rows = parse_table(prose)
    if not rows:
        raise Fail("E_NO_TABLE", "không thấy bảng đối chiếu số ↔ khối `scope=`")
    errors: list[str] = []
    for field, word, kind, log in rows:
        # Một đường-dẫn log có thể xuất hiện ở **nhiều** khối (cùng tệp log trích hai đoạn khác
        # nhau). Chọn khối **đầu tiên** có trường này kèm giá trị số.
        value = None
        for k, l, body in blocks:
            if k == kind and l == log:
                v = field_value(body, field)
                if v is not None:
                    value = v
                    break
        if value is None:
            if not any(k == kind and l == log for k, l, _ in blocks):
                errors.append(
                    f"E_BLOCK_MISSING: `{field}` trỏ khối `{kind}={log}` nhưng tài liệu không có khối đó"
                )
            else:
                errors.append(
                    f"E_FIELD_MISSING: `{field}` không có giá trị số trong khối `{kind}={log}`"
                )
            continue
        # (1) Cột "Giá trị (chữ)" của bảng phải khớp số trong khối — đây là vế **chính**, không
        #     phụ thuộc cách hành văn.
        want = WORDS.get(word.strip())
        if want is None:
            errors.append(f"E_WORD_UNKNOWN: `{field}` — cột giá trị ghi '{word}' không phải chữ số đã biết")
        elif want != value:
            errors.append(
                f"E_TABLE_MISMATCH: `{field}` — bảng ghi '{word}' ({want}), khối `{kind}={log}` ghi {value}"
            )
        # (2) Văn xuôi: chỉ so những câu **không** nói về tình huống khác.
        for m in re.finditer(
            r"`" + re.escape(field) + r"`[^`\n]{0,60}?\bbằng\s+(" + "|".join(
                sorted((re.escape(w) for w in WORDS), key=len, reverse=True)
            ) + r")\b",
            prose,
        ):
            sent = _sentence_of(prose, m.start()).lower()
            if any(q in sent for q in QUALIFIERS):
                continue
            claim = WORDS[m.group(1)]
            if claim != value:
                errors.append(
                    f"E_VALUE_MISMATCH: `{field}` — văn xuôi nói {claim}, khối `{kind}={log}` ghi {value}"
                )
    return errors


def run(root: str, quiet: bool = False) -> int:
    fails = 0
    for rel in DOCS:
        path = os.path.join(root, rel)
        if not os.path.isfile(path):
            print(f"not ok - {rel}: không có tệp", file=sys.stderr)
            fails += 1
            continue
        try:
            errors = check_doc(path)
        except Fail as exc:
            print(f"not ok - {rel}: {exc.code} {exc.detail}", file=sys.stderr)
            fails += 1
            continue
        if errors:
            for e in errors:
                print(f"not ok - {rel}: {e}", file=sys.stderr)
            fails += len(errors)
        elif not quiet:
            print(f"ok - {rel}: bảng đối chiếu khớp khối scope=")
    if fails:
        print(f"# check-scope-table: FAIL ({fails} lỗi)", file=sys.stderr)
        return 1
    if not quiet:
        print("# check-scope-table: PASS")
    return 0


# ---------------------------------------------------------------- selftest

GOOD = """# Tài liệu giả

Văn xuôi nói `alpha` bằng một và `beta` bằng không.

## Bảng đối chiếu số ↔ khối `scope=`

| Trường đo | Giá trị (chữ) | Loại khối | Tệp log |
|---|---|---|---|
| `alpha` | một | `scope=` | `log-a.log` |
| `beta` | không | `scope=` | `log-a.log` |

```text scope=log-a.log
# alpha=1 beta=0
```
"""


def selftest() -> int:
    cases: list[tuple[str, str, str]] = []  # (tên, nội dung, mã mong đợi hoặc "")

    cases.append(("dương: tài liệu đúng", GOOD, ""))
    cases.append(
        (
            "dương: câu có từ khoá tình huống khác thì bỏ qua",
            GOOD.replace(
                "Văn xuôi nói `alpha` bằng một",
                "Văn xuôi nói `alpha` bằng một; nhánh `alpha` bằng hai chưa có ca",
            ),
            "",
        )
    )
    cases.append(
        (
            "âm: cột giá trị lệch khối",
            GOOD.replace("| `alpha` | một |", "| `alpha` | hai |"),
            "E_TABLE_MISMATCH",
        )
    )
    cases.append(
        (
            "âm: văn xuôi lệch khối",
            GOOD.replace("`alpha` bằng một", "`alpha` bằng hai"),
            "E_VALUE_MISMATCH",
        )
    )
    cases.append(
        (
            "âm: đột biến một số trong log",
            GOOD.replace("# alpha=1 beta=0", "# alpha=7 beta=0"),
            "E_TABLE_MISMATCH",
        )
    )
    cases.append(
        (
            "âm: trỏ khối không có",
            GOOD.replace("`log-a.log` |\n| `beta`", "`log-khong-co.log` |\n| `beta`"),
            "E_BLOCK_MISSING",
        )
    )
    cases.append(
        (
            "âm: trường không có trong khối",
            GOOD.replace("# alpha=1 beta=0", "# alpha=1 gamma=0"),
            "E_FIELD_MISSING",
        )
    )
    cases.append(
        (
            "âm: cột giá trị không phải chữ số",
            GOOD.replace("| `alpha` | một |", "| `alpha` | bốn mươi |"),
            "E_WORD_UNKNOWN",
        )
    )
    cases.append(
        (
            "âm: không có bảng",
            GOOD.replace("## Bảng đối chiếu số ↔ khối `scope=`", "## Không có bảng")
            .replace("| Trường đo | Giá trị (chữ) | Loại khối | Tệp log |", "")
            .replace("|---|---|---|---|", "")
            .replace("| `alpha` | một | `scope=` | `log-a.log` |", "")
            .replace("| `beta` | không | `scope=` | `log-a.log` |", ""),
            "E_NO_TABLE",
        )
    )
    cases.append(
        (
            "âm: khối không đóng",
            GOOD.replace("# alpha=1 beta=0\n```", "# alpha=1 beta=0"),
            "E_BLOCK_UNCLOSED",
        )
    )

    ok = 0
    bad = 0
    for name, body, want in cases:
        with tempfile.TemporaryDirectory() as d:
            rel = DOCS[0]
            path = os.path.join(d, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as fh:
                fh.write(body)
            try:
                errors = check_doc(path)
                got = errors[0].split(":")[0] if errors else ""
            except Fail as exc:
                got = exc.code
            if want == "":
                if got == "":
                    print(f"ok - selftest {name}: XANH đúng")
                    ok += 1
                else:
                    print(f"not ok - selftest {name}: mong XANH, thấy {got}", file=sys.stderr)
                    bad += 1
            else:
                if got == want:
                    print(f"ok - selftest {name}: ĐỎ đúng ({want})")
                    ok += 1
                else:
                    print(f"not ok - selftest {name}: mong {want}, thấy {got or 'XANH'}", file=sys.stderr)
                    bad += 1
    print(f"# check-scope-table-selftest: {'PASS' if bad == 0 else 'FAIL'} ({ok}/{len(cases)} ca)")
    return 0 if bad == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="KT-134 — kiểm máy bảng đối chiếu số ↔ khối scope=")
    ap.add_argument("--root", default=os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    return run(args.root, args.quiet)


if __name__ == "__main__":
    sys.exit(main())
