#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-134 (C7 của Agent 0) — kiểm **máy** bảng đối chiếu số ↔ khối `scope=`.

Vì sao có tệp này: KT-133 thêm vào ba tài liệu K6 một bảng nói "trường đo X có số nằm trong khối
`scope=` Y". Nhưng một bảng **không được kiểm** thì đúng lớp lỗi đã gặp nhiều lần trong repo: tài
liệu khai một đằng, log một nẻo, và không ai phát hiện. Agent 0 chốt: **không có kiểm máy thì Q6
coi như chưa xong**.

Tệp này làm đúng năm việc:

1. **Cấu trúc** — mỗi dòng bảng phải trỏ tới một khối `scope=`/`scope-red=` **có thật** trong chính
   tài liệu đó, và trường đo phải xuất hiện **kèm giá trị số** trong khối ấy.
1b. **Không mập mờ** (`E_AMBIGUOUS_FIELD`) — nếu một trường có **nhiều hơn một** giá trị trong
   **tất cả** các khối của tài liệu thì dòng bảng **phải** ghi rõ nó nói về khối ở **tiết nào**
   (cột "Khối", dạng `§7` / `§2.3`), và checker kiểm khối được trỏ tới **thật sự** nằm trong tiết
   đó. Vì sao cần: KT-140 — dòng `observed` trong `k6-exec.md` trỏ một tệp log có **hai** khối in
   `observed` với **hai** giá trị (`0x74310001` ở §2.3, `0` ở §7); checker cũ resolve sang khối §7
   nên vẫn xanh, nhưng **người đọc** tra khối đầu tiên thấy mâu thuẫn. Một phép kiểm chỉ đúng khi
   người đọc cũng đọc ra cùng kết luận.
2. **Ngữ nghĩa** — nếu văn xuôi nói "`trường` bằng <chữ số>", chữ số đó phải **khớp** giá trị trong
   khối. Lệch ⇒ đỏ. Đây là vế bắt được "sửa một số trong log mà quên sửa văn xuôi".
2b. **So với khối mà câu nói tới** (`resolve_claim_block`, KT-142) — câu nói về khối `scope-red=`
   thì so với **khối đỏ**, không so với khối xanh mà dòng bảng trỏ tới. Trước KT-142 những câu
   ấy phải được **miễn** bằng từ khoá loại trừ, tức là phép kiểm bị nới đúng ở chỗ dễ sai nhất
   (số của ca âm). Nay chỉ câu **không quy được** khối nào mới rơi về khối của dòng bảng.
3. **Đếm lỗ có thể rộng dần** — vế (2) phải bỏ qua những câu nói về **tình huống khác** (giả
   thuyết, kỳ vọng, đột biến, "chưa có ca"), nên có bộ từ khoá loại trừ `QUALIFIERS`. Bộ từ khoá ấy
   là một **lỗ**: ai thêm một từ khoá để né một câu khó thì checker vẫn xanh nhưng phép kiểm đã bị
   nới. Vì vậy checker **in ra số câu bị bỏ qua + vị trí** (stdout, để nằm trong log cổng) và
   **CẢNH BÁO** khi số đó tăng so với mốc ở `v2/agent/JOURNAL/scope-table-skip-count.txt`. Cảnh
   báo **không** làm đỏ — Agent 0 nói "cảnh báo trong log", không nói đỏ — nhưng phải nổi bật.
   KT-142 đã quét **toàn bộ** danh sách (Agent 0 lượt 4 câu 3): 14 từ khoá ⇒ 9, số câu bị bỏ qua
   4 ⇒ 1. Sổ từng câu: `v2/agent/JOURNAL/kt142-quet-qualifiers.md`.
4. **Đột biến** (`--selftest`) — dựng tài liệu giả rồi phá từng bất biến, mỗi ca phải đỏ **kèm đúng
   mã lý do**; cộng ca dương (tài liệu đúng phải xanh) để một checker hỏng-luôn-đỏ không qua được;
   cộng ca đếm/cảnh báo (bắt stdout) để chính vế (3) cũng có ca âm.

Chạy: `python3 v2/tools/check-scope-table.py [--root R] [--selftest] [--quiet] [--no-record]`
Exit: 0 = xanh, 1 = có lỗi, 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import bisect
import contextlib
import io
import os
import re
import sys
import tempfile
from datetime import datetime
from typing import NamedTuple

# Tài liệu có bảng đối chiếu (KT-133). Thêm tệp mới thì thêm ở đây **và** thêm ca selftest.
DOCS = [
    "v2/docs/k6-exec.md",
    "v2/docs/k6-persist.md",
    "v2/agent/reports/K6-DOD.md",
]

# Mốc số câu bị bỏ qua của **lần chạy trước** (Agent 0: "Số câu bỏ qua tăng giữa hai lần chạy ⇒
# cảnh báo trong log"). Tệp này **không** phải ngưỡng — nó chỉ là mốc so sánh, do checker ghi tự
# động (trừ `--no-record`). Đường dẫn tính từ `--root`.
SKIP_COUNT_FILE = "v2/agent/JOURNAL/scope-table-skip-count.txt"

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
# Bốn cột **bắt buộc** của bảng, đúng thứ tự. Cột thứ năm ("Khối") là **tuỳ chọn** — xem
# `SECTION_COL`; bảng không có nó vẫn hợp lệ **nếu** không dòng nào cần chỉ rõ khối (luật
# `E_AMBIGUOUS_FIELD`). Bắt buộc đọc tiêu đề cột thay vì đoán vị trí: một bảng đổi thứ tự cột mà
# checker vẫn đọc theo vị trí là đúng lớp "xanh giả" tệp này sinh ra để chặn.
TABLE_COLS = ("Trường đo", "Giá trị (chữ)", "Loại khối", "Tệp log")
SECTION_COL = "Khối"
# Cột "Khối" ghi **số hiệu tiết** dạng `§7` / `§2.3` — máy đọc được, không phải suy luận từ văn xuôi.
SECTION_RE = re.compile(r"^§(\d+(?:\.\d+)*)$")
# Tiêu đề tiết **có đánh số** (`## 7. …`, `### 2.3 …`). Tiêu đề không đánh số (vd `## Bảng đối
# chiếu …`) đặt lại tiết = None: khối nằm dưới nó **không** thuộc tiết nào, nên không được dùng làm
# đích của cột "Khối".
HEADING_RE = re.compile(r"^(#{2,6})\s+(\d+(?:\.\d+)*)\.?\s")
BLOCK_RE = re.compile(r"^```text (scope(?:-red)?)=(\S+)\s*$")
FENCE_RE = re.compile(r"^```\s*$")


class Fail(Exception):
    """Một lỗi có mã — mã dùng cho ca âm của selftest."""

    def __init__(self, code: str, detail: str) -> None:
        super().__init__(f"{code}: {detail}")
        self.code = code
        self.detail = detail


class Skip(NamedTuple):
    """Một câu văn xuôi bị **bỏ qua** vì khớp từ khoá loại trừ.

    `line` là số dòng **trong tệp** (không phải trong văn xuôi đã cắt khối) để người đọc mở được
    đúng chỗ; `keyword` là từ khoá khớp **đầu tiên** theo thứ tự khai báo trong `QUALIFIERS`.
    """

    line: int
    keyword: str
    sentence: str


class DocResult(NamedTuple):
    errors: list[str]
    skips: list[Skip]
    claims: int  # tổng số câu khẳng định "`trường` … bằng <chữ số>" đã thấy (kể cả bị bỏ qua)


def split_blocks(
    text: str,
) -> tuple[
    str,
    list[tuple[int, int]],
    list[tuple[str, str, str, str | None]],
    set[str],
    dict[int, str | None],
]:
    """Trả (văn xuôi không có khối code, mốc dòng, [(loại, log, nội-dung, tiết)], các tiết có,
    tiết-của-từng-dòng-văn-xuôi).

    Mốc dòng là `[(offset_bắt_đầu_dòng_trong_văn_xuôi, số_dòng_trong_tệp)]`. Cần nó vì văn xuôi đã
    bị **cắt khối code** nên số dòng của văn xuôi không trùng số dòng của tệp — mà báo cáo "câu bị
    bỏ qua" phải chỉ đúng `tệp:số-dòng` để người đọc mở được.

    Phần tử thứ tư của mỗi khối là **số hiệu tiết** chứa khối (`"7"`, `"2.3"`, hoặc `None` nếu khối
    nằm trước tiêu đề đánh số đầu tiên / dưới một tiêu đề **không** đánh số). Cần nó cho luật
    `E_AMBIGUOUS_FIELD`: cột "Khối" của bảng phải trỏ tới khối **thật sự** nằm trong tiết đã ghi.

    Phần tử thứ năm là `{số_dòng_trong_tệp: tiết}` cho **dòng văn xuôi**. Cần nó cho luật
    `E_CLAIM_BLOCK` (KT-142): một câu văn xuôi nói về khối nào thì phải so với **khối đó**, không
    phải khối mà dòng bảng trỏ tới — nếu không, mọi câu nói về khối `scope-red=` đều buộc phải
    được miễn bằng từ khoá, và bộ từ khoá loại trừ lại rộng ra.
    """
    prose_lines: list[tuple[int, str]] = []
    blocks: list[tuple[str, str, str, str | None]] = []
    sections: set[str] = set()
    line_section: dict[int, str | None] = {}
    cur: tuple[str, str] | None = None
    buf: list[str] = []
    section: str | None = None
    for lineno, line in enumerate(text.splitlines(), 1):
        if cur is None:
            m = BLOCK_RE.match(line)
            if m:
                cur = (m.group(1), m.group(2))
                buf = []
                continue
            h = HEADING_RE.match(line)
            if h:
                section = h.group(2)
                sections.add(section)
            elif re.match(r"^#{2,6}\s", line):
                # Tiêu đề **không** đánh số (vd "## Bảng đối chiếu …"): khối dưới nó không thuộc
                # tiết nào ⇒ đặt lại None. Nếu không, một khối ở cuối tài liệu sẽ bị gán nhầm vào
                # tiết đánh số **cuối cùng** và cột "Khối" sẽ "đúng" một cách giả tạo.
                section = None
            prose_lines.append((lineno, line))
            line_section[lineno] = section
        else:
            if FENCE_RE.match(line):
                blocks.append((cur[0], cur[1], "\n".join(buf), section))
                cur = None
                buf = []
                continue
            buf.append(line)
    if cur is not None:  # khối không đóng
        raise Fail("E_BLOCK_UNCLOSED", f"khối {cur[0]}={cur[1]} không có dòng đóng ```")
    starts: list[tuple[int, int]] = []
    off = 0
    for lineno, line in prose_lines:
        starts.append((off, lineno))
        off += len(line) + 1
    return "\n".join(line for _, line in prose_lines), starts, blocks, sections, line_section


def line_at(starts: list[tuple[int, int]], pos: int) -> int:
    """Số dòng trong tệp của offset `pos` trong văn xuôi."""
    if not starts:
        return 1
    i = bisect.bisect_right(starts, (pos, 1 << 62)) - 1
    return starts[max(i, 0)][1]


def _cells(line: str) -> list[str] | None:
    """Tách một dòng bảng Markdown thành các ô; None nếu dòng không phải dòng bảng."""
    s = line.strip()
    if not (s.startswith("|") and s.endswith("|")):
        return None
    return [c.strip() for c in s[1:-1].split("|")]


def parse_table(prose: str) -> list[tuple[str, str, str, str, str]]:
    """Trả [(trường, giá-trị-chữ, loại-khối, tệp-log, tiết)]; rỗng nếu không có bảng.

    `tiết` là nội dung cột "Khối" (`""` nếu bảng không có cột đó hoặc ô để trống). Đọc **theo tên
    cột** ở dòng tiêu đề, không theo vị trí: bảng đổi thứ tự cột thì phải đỏ, không được đọc nhầm.
    """
    lines = prose.splitlines()
    start = None
    for i, line in enumerate(lines):
        if TABLE_HEADER in line:
            start = i
            break
    if start is None:
        return []
    # Dòng tiêu đề cột là dòng bảng **đầu tiên** sau tiêu đề mục (giữa hai thứ có thể có văn xuôi).
    hdr_i = None
    for i in range(start + 1, len(lines)):
        if lines[i].strip().startswith("|"):
            hdr_i = i
            break
    if hdr_i is None:
        raise Fail("E_TABLE_HEADER", "không thấy dòng tiêu đề cột của bảng đối chiếu")
    header = _cells(lines[hdr_i])
    if header is None or tuple(header[:4]) != TABLE_COLS:
        raise Fail(
            "E_TABLE_HEADER",
            f"tiêu đề cột phải bắt đầu bằng {list(TABLE_COLS)}, thấy {header}",
        )
    if len(header) == 5 and header[4] != SECTION_COL:
        raise Fail("E_TABLE_HEADER", f"cột thứ năm phải là '{SECTION_COL}', thấy '{header[4]}'")
    if len(header) > 5:
        raise Fail("E_TABLE_HEADER", f"bảng có {len(header)} cột, tối đa 5")
    ncols = len(header)
    rows: list[tuple[str, str, str, str, str]] = []
    for line in lines[hdr_i + 1 :]:
        cells = _cells(line)
        if cells is None:
            if rows and not line.strip():
                break
            continue
        if all(set(c) <= set("-: ") for c in cells):
            continue  # dòng phân cách |---|---|
        if len(cells) != ncols:
            raise Fail(
                "E_TABLE_ROW_SHAPE",
                f"dòng bảng có {len(cells)} cột, tiêu đề có {ncols}: {line.strip()}",
            )
        m = re.fullmatch(r"`([A-Za-z_][A-Za-z0-9_.]*)`", cells[0])
        if not m:
            continue  # không phải dòng dữ liệu (vd dòng tiêu đề lặp lại)
        km = re.fullmatch(r"`(scope(?:-red)?)=`", cells[2])
        if not km:
            raise Fail(
                "E_TABLE_ROW_SHAPE",
                f"cột 'Loại khối' phải là `scope=`/`scope-red=`, thấy {cells[2]!r}",
            )
        lm = re.fullmatch(r"`([^`]+)`", cells[3])
        if not lm:
            raise Fail("E_TABLE_ROW_SHAPE", f"cột 'Tệp log' phải là `tên-tệp`, thấy {cells[3]!r}")
        rows.append((m.group(1), cells[1], km.group(1), lm.group(1), cells[4] if ncols == 5 else ""))
    return rows


def field_value(block_text: str, field: str) -> int | None:
    """Giá trị **thập phân** của `field=` trong khối; None nếu không có.

    Đuôi `(?![0-9A-Za-z_])` là **bắt buộc**, không phải trang trí: thiếu nó thì `observed=0x74310001`
    bị đọc thành `observed=0` (cắt ở chữ `x`) — một **xanh giả** đúng lớp lỗi mà tệp này sinh ra để
    chặn. Giá trị hex **không** phải giá trị thập phân: cột "Giá trị (chữ)" không diễn đạt được hex,
    nên khối chỉ có hex thì coi như **không có** giá trị dùng được (và checker đi tiếp khối sau).
    """
    m = re.search(
        r"(?<![A-Za-z0-9_])" + re.escape(field) + r"=(-?\d+)(?![0-9A-Za-z_])", block_text
    )
    if not m:
        return None
    return int(m.group(1))


def field_values_all(blocks: list[tuple[str, str, str, str | None]], field: str) -> set[int]:
    """**Mọi** giá trị khác nhau của `field` trong **tất cả** các khối — thập phân **và** hex.

    Vì sao phải có hex (đây là lý do tồn tại của hàm này, đừng "tối giản" nó đi): `observed` trong
    `k6-exec.md` có `observed=0` ở §7 **và** `observed=0x74310001` ở §2.3. Nếu chỉ đếm giá trị thập
    phân thì trường này trông như **một** giá trị, luật `E_AMBIGUOUS_FIELD` bỏ sót đúng chỗ tài liệu
    mập mờ — tức luật không bắt được lỗi mà nó sinh ra để bắt. Hex được quy về số nguyên để `0x10`
    và `16` là **cùng** một giá trị (không báo mập mờ oan).
    """
    pat = re.compile(
        r"(?<![A-Za-z0-9_])" + re.escape(field) + r"=(0[xX][0-9A-Fa-f_]+|-?\d+)(?![0-9A-Za-z_])"
    )
    out: set[int] = set()
    for _kind, _log, body, _section in blocks:
        for m in pat.finditer(body):
            tok = m.group(1).replace("_", "")
            try:
                out.add(int(tok, 16) if tok[:2].lower() == "0x" else int(tok, 10))
            except ValueError:
                continue
    return out


def _fmt_values(values: set[int]) -> str:
    """Danh sách giá trị cho thông điệp lỗi: thập phân, kèm hex khi > 9 (để đọc được cả hai kiểu)."""
    return ", ".join(f"{v} (0x{v:x})" if v > 9 else str(v) for v in sorted(values))


def _claim_re(field: str) -> re.Pattern[str]:
    """Regex "`field` … bằng <chữ số>" — một chỗ duy nhất, để vế đếm và vế so không lệch nhau."""
    words = "|".join(sorted((re.escape(w) for w in WORDS), key=len, reverse=True))
    return re.compile(r"`" + re.escape(field) + r"`[^`\n]{0,60}?\bbằng\s+(" + words + r")\b")


# ---------------------------------------------------------------- từ khoá loại trừ
#
# **ĐỌC KỸ TRƯỚC KHI SỬA DANH SÁCH NÀY.**
#
# Ba tài liệu K6 có nhiều câu nói về **tình huống khác** (nhánh âm, ca đỏ, giả thuyết) chứ không
# phải giá trị đo của khối đang xét. Câu chứa một trong các từ khoá dưới đây thì **không** đem so
# với khối — nếu không, checker báo lỗi oan (bản đầu báo bảy lỗi oan trên chính ba tài liệu này).
#
# **Thêm một từ khoá vào đây là NỚI LỎNG phép kiểm**: mỗi từ khoá là một lỗ để một câu sai lọt qua,
# và lỗ đó rộng dần nếu ai muốn né một câu khó. Vì vậy:
#
#   1. Danh sách là **hằng số trong code** — không đọc từ tệp, biến môi trường hay CLI. Muốn nới
#      thì phải sửa code, tức là phải qua review.
#   2. Mỗi từ khoá phải có **lý do ghi ngay cạnh** (vì sao câu chứa nó không phải là khẳng định về
#      giá trị đo của khối đang xét). Không có lý do ⇒ không được thêm.
#   3. Checker in ra **số câu bị bỏ qua + vị trí** và **CẢNH BÁO** khi số đó tăng so với mốc ở
#      `SKIP_COUNT_FILE` — để người đọc thấy ngay nếu ai vừa nới.
#
# Từ khoá nào **rộng** thì ghi rõ là rộng: `nếu`, `sẽ` khớp với rất nhiều câu; chúng tồn tại
# vì văn xuôi của ba tài liệu này thật sự dùng chúng cho giả thuyết/dự định, **không** phải vì tiện.
#
# **KT-142 (Agent 0, lượt 4 câu 3) — đã quét toàn bộ danh sách.** Bản trước có 14 từ khoá; quét
# từng câu bị loại cho thấy **bốn** từ khoá (`nhánh`, `ca âm`, `ca đỏ`, `vắng`, `đỏ`) chỉ gánh
# những câu nói về **khối `scope-red=`** — tức là việc mà `resolve_claim_block` làm được. Nay câu
# nào chỉ rõ khối (hoặc nằm cùng tiết với đúng một khối có trường) thì **được so thật**, nên năm
# từ khoá ấy đã **bỏ**; số câu bị bỏ qua giảm từ bốn xuống một. Chi tiết từng câu: xem
# `v2/agent/JOURNAL/kt142-quet-qualifiers.md`.
QUALIFIERS: tuple[tuple[str, str], ...] = (
    ("chưa có", "câu nói 'chưa có ca' — mô tả **thiếu** bằng chứng, không khẳng định giá trị đo"),
    ("nếu", "câu **giả thuyết** ('nếu … thì đỏ') — không phải số đo"),
    ("đột biến", "câu nói về bản **đột biến**; log mutation không bao giờ là nguồn số (D2-112c)"),
    ("mong", "câu nói **kỳ vọng** ('mong đợi'), không phải số đo"),
    ("phải là", "câu nói **điều kiện phải thoả** (bất biến/kỳ vọng), không phải số đo"),
    ("sẽ", "câu nói **tương lai**/dự định — chưa đo"),
    ("bỏ ", "câu nói về việc **bỏ** một dòng/khối (đột biến 'bỏ `release_with`')"),
    ("tăng gấp đôi", "câu nói về **hệ quả của đột biến** ('frames tăng gấp đôi')"),
)


def first_qualifier(sentence: str) -> str | None:
    """Từ khoá loại trừ **đầu tiên** khớp câu (thứ tự khai báo), None nếu không có."""
    low = sentence.lower()
    for keyword, _reason in QUALIFIERS:
        if keyword in low:
            return keyword
    return None


# Ranh giới "câu" khi xét từ khoá loại trừ. Phải cắt **hai đầu**: bản đầu chỉ cắt đầu, nên một câu
# khẳng định đứng **trước** một mệnh đề có từ khoá (vd "`alpha` bằng một; nhánh `alpha` bằng hai")
# vẫn bị bỏ qua — bộ lọc rộng hơn ý định và ca selftest "đếm đúng 1 câu" bắt được đúng lỗi này.
# `; ` là ranh giới mệnh đề trong văn xuôi của ba tài liệu; thêm nó làm bộ lọc **hẹp lại** (chặt
# hơn), không phải nới.
SENTENCE_DELIMS = ("\n", ". ", "; ")


def _sentence_of(prose: str, pos: int) -> str:
    """Câu/mệnh đề chứa vị trí `pos` (cắt hai đầu theo `SENTENCE_DELIMS`)."""
    start = -1
    for delim in SENTENCE_DELIMS:
        i = prose.rfind(delim, 0, pos)
        if i >= 0:
            start = max(start, i + len(delim) - 1)
    end = len(prose)
    for delim in SENTENCE_DELIMS:
        j = prose.find(delim, pos)
        if j >= 0:
            end = min(end, j)
    return prose[start + 1 : end]


def _excerpt(sentence: str, n: int = 60) -> str:
    """60 ký tự đầu của câu, gộp khoảng trắng để một dòng log không bị ngắt."""
    s = " ".join(sentence.split())
    return s if len(s) <= n else s[:n] + "…"


# ---------------------------------------------------------------- khối mà **câu** nói tới (KT-142)
#
# Vì sao cần: bản trước so **mọi** câu văn xuôi với khối mà **dòng bảng** trỏ tới. Nhưng ba tài liệu
# K6 có những câu nói về khối `scope-red=` (ca âm) trong khi dòng bảng trỏ khối `scope=` xanh — so
# thẳng thì đỏ oan, nên chúng phải được **miễn** bằng từ khoá. Đó chính là lỗ mà Agent 0 bảo quét
# (lượt 4, câu 3): từ khoá loại trừ đang gánh việc mà phép kiểm làm được. Nay câu nào **chỉ rõ**
# khối (hoặc nằm cùng tiết với đúng một khối có trường đó) thì so với **khối ấy**; chỉ câu không
# quy được khối nào mới rơi về khối của dòng bảng.
#
# Thứ tự quy khối — **chặt trước, lỏng sau**, và mỗi bước chỉ nhận khi kết quả **duy nhất**:
#   1. Câu ghi đúng `scope=<tệp>` / `scope-red=<tệp>` ⇒ khối đó.
#   2. Câu ghi **loại** khối (`scope-red` / `scope`) mà không ghi tệp ⇒ khối **cùng tiết** cùng
#      loại; phải duy nhất về giá trị.
#   3. Câu nằm trong tiết có khối chứa trường ⇒ khối cùng tiết; phải duy nhất về giá trị.
#   4. Không quy được ⇒ `None` (dùng khối của dòng bảng, như bản cũ).
#
# "Duy nhất về **giá trị**" chứ không phải "duy nhất về số khối": tiết 3.1 có nhiều khối cùng ghi
# `recorded=1` — chúng không mâu thuẫn nên vẫn quy được. Chỉ khi các khối **nói khác nhau** mới bỏ
# qua bước đó, vì lúc ấy không có căn cứ chọn.
BLOCK_NAME_RE = re.compile(r"`(scope(?:-red)?)=([^`\s]+)`")
BLOCK_KIND_RE = re.compile(r"`(scope(?:-red)?)`")


def _unique_value(values: list[int]) -> int | None:
    """Giá trị nếu danh sách chỉ có **một** giá trị khác nhau, None nếu mâu thuẫn/rỗng."""
    if not values:
        return None
    first = values[0]
    return first if all(v == first for v in values) else None


def resolve_claim_block(
    blocks: list[tuple[str, str, str, str | None]],
    field: str,
    sentence: str,
    section: str | None,
) -> int | None:
    """Giá trị của `field` trong khối mà **câu** nói tới; None nếu không quy được duy nhất."""
    # (1) Câu ghi đúng tên khối.
    named = BLOCK_NAME_RE.findall(sentence)
    if named:
        vals = [
            v
            for k, l, body, _sec in blocks
            if (k, l) in named and (v := field_value(body, field)) is not None
        ]
        got = _unique_value(vals)
        if got is not None:
            return got
    # (2) Câu ghi **loại** khối, không ghi tệp.
    kinds = {k for k in BLOCK_KIND_RE.findall(sentence)}
    if kinds:
        vals = [
            v
            for k, _l, body, sec in blocks
            if k in kinds and sec == section and (v := field_value(body, field)) is not None
        ]
        got = _unique_value(vals)
        if got is not None:
            return got
    # (3) Cùng tiết.
    vals = [
        v for _k, _l, body, sec in blocks if sec == section and (v := field_value(body, field)) is not None
    ]
    return _unique_value(vals)


def check_doc(path: str) -> DocResult:
    """Trả (lỗi, câu bị bỏ qua, số câu khẳng định). Lỗi rỗng = xanh."""
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    prose, starts, blocks, sections, line_section = split_blocks(text)
    rows = parse_table(prose)
    if not rows:
        raise Fail("E_NO_TABLE", "không thấy bảng đối chiếu số ↔ khối `scope=`")
    errors: list[str] = []
    skips: list[Skip] = []
    claims = 0
    for field, word, kind, log, section in rows:
        # (0) **Không mập mờ** (KT-140): nếu trường này có **nhiều hơn một** giá trị trong **tất cả**
        #     các khối của tài liệu thì dòng bảng **phải** ghi rõ nó nói về khối ở tiết nào. Vì sao:
        #     checker cũ chọn "khối đầu tiên có trường" nên vẫn xanh, nhưng **người đọc** tra khối
        #     đầu tiên thấy giá trị khác — phép kiểm đúng mà tài liệu vẫn mập mờ. Đếm **cả hex** để
        #     bắt đúng ca `observed` (0x74310001 ở §2.3 vs 0 ở §7).
        all_values = field_values_all(blocks, field)
        if len(all_values) > 1 and not section:
            errors.append(
                f"E_AMBIGUOUS_FIELD: `{field}` có {len(all_values)} giá trị khác nhau trong tài liệu"
                f" ({_fmt_values(all_values)}) nhưng dòng bảng không ghi cột '{SECTION_COL}'"
                f" — người đọc tra khối đầu tiên sẽ thấy giá trị khác"
            )
        # (0b) Cột "Khối" phải **máy đọc được** và trỏ tới tiết **có thật**.
        want_section: str | None = None
        struct_bad = False
        if section:
            sm = SECTION_RE.match(section)
            if not sm:
                errors.append(
                    f"E_SECTION_UNKNOWN: `{field}` — cột '{SECTION_COL}' ghi {section!r},"
                    f" phải dạng §<số> (vd §7, §2.3)"
                )
                struct_bad = True
            elif sm.group(1) not in sections:
                errors.append(
                    f"E_SECTION_UNKNOWN: `{field}` — cột '{SECTION_COL}' ghi {section!r}"
                    f" nhưng tài liệu không có tiết đó"
                )
                struct_bad = True
            else:
                want_section = sm.group(1)
        # (1) Khối đích: một đường-dẫn log có thể xuất hiện ở **nhiều** khối (cùng tệp log trích hai
        #     đoạn khác nhau). Có cột "Khối" thì **chỉ** xét khối trong tiết đó — đây là chỗ làm cho
        #     "chỉ rõ khối" có hiệu lực thật, không chỉ là lời khai. Không có thì giữ luật cũ: khối
        #     **đầu tiên** có trường này kèm giá trị **thập phân**.
        value = None
        if not struct_bad:
            for k, l, body, sec in blocks:
                if k != kind or l != log:
                    continue
                if want_section is not None and sec != want_section:
                    continue
                v = field_value(body, field)
                if v is not None:
                    value = v
                    break
            if value is None:
                same_kind_log = [(sec, body) for k, l, body, sec in blocks if k == kind and l == log]
                if not same_kind_log:
                    errors.append(
                        f"E_BLOCK_MISSING: `{field}` trỏ khối `{kind}={log}` nhưng tài liệu không có khối đó"
                    )
                elif want_section is not None and not any(
                    sec == want_section for sec, _ in same_kind_log
                ):
                    errors.append(
                        f"E_SECTION_MISMATCH: `{field}` — cột '{SECTION_COL}' ghi §{want_section}"
                        f" nhưng khối `{kind}={log}` không nằm trong tiết đó"
                    )
                else:
                    errors.append(
                        f"E_FIELD_MISSING: `{field}` không có giá trị số trong khối `{kind}={log}`"
                    )
        if value is not None:
            # (2) Cột "Giá trị (chữ)" của bảng phải khớp số trong khối — đây là vế **chính**,
            #     không phụ thuộc cách hành văn.
            want = WORDS.get(word.strip())
            if want is None:
                errors.append(
                    f"E_WORD_UNKNOWN: `{field}` — cột giá trị ghi '{word}' không phải chữ số đã biết"
                )
            elif want != value:
                errors.append(
                    f"E_TABLE_MISMATCH: `{field}` — bảng ghi '{word}' ({want}), khối `{kind}={log}` ghi {value}"
                )
        # (3) Văn xuôi: chỉ so những câu **không** nói về tình huống khác. Quét **cả khi** khối
        #     không resolve được, để số câu bỏ qua không phụ thuộc vào việc tài liệu đang đỏ hay
        #     xanh — mốc so sánh phải ổn định, nếu không cảnh báo "tăng" sẽ là nhiễu.
        for m in _claim_re(field).finditer(prose):
            claims += 1
            line = line_at(starts, m.start())
            sent = _sentence_of(prose, m.start())
            keyword = first_qualifier(sent)
            if keyword is not None:
                skips.append(Skip(line, keyword, sent.strip()))
                continue
            # KT-142: câu nói về khối nào thì so với **khối đó** (xem `resolve_claim_block`).
            # Không quy được ⇒ giữ nguyên hành vi cũ: so với khối của dòng bảng.
            target = resolve_claim_block(blocks, field, sent, line_section.get(line))
            if target is None:
                target = value
            if target is None:
                continue  # đã có lỗi cấu trúc cho trường này ở trên
            claim = WORDS[m.group(1)]
            if claim != target:
                errors.append(
                    f"E_VALUE_MISMATCH: `{field}` — văn xuôi nói {claim}, khối ghi {target}"
                    f" (dòng bảng trỏ `{kind}={log}` ghi {value})"
                )
    return DocResult(errors, skips, claims)


# ---------------------------------------------------------------- mốc số câu bỏ qua


def read_counts(path: str) -> dict[str, int] | None:
    """Đọc mốc; None = **chưa có tệp** (khác với tệp rỗng = mốc 0)."""
    if not os.path.isfile(path):
        return None
    counts: dict[str, int] = {}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.rsplit(None, 1)
            if len(parts) != 2:
                continue
            try:
                counts[parts[0]] = int(parts[1])
            except ValueError:
                continue
    return counts


def write_counts(path: str, counts: dict[str, int]) -> bool:
    """Ghi mốc; trả True nếu tệp **đổi**. Chỉ ghi khi số đổi để git không bẩn sau mỗi lần chạy."""
    if read_counts(path) == counts and os.path.isfile(path):
        return False
    lines = [
        "# check-scope-table — mốc số câu bị bỏ qua vì từ khoá loại trừ (QUALIFIERS).",
        "#",
        "# Vì sao có tệp này: bộ từ khoá loại trừ là một **lỗ có thể rộng dần**. Ai đó thêm một từ",
        "# khoá để né một câu khó thì checker vẫn xanh nhưng phép kiểm đã bị nới. Checker so số câu",
        "# bỏ qua của lần chạy này với mốc dưới đây và in CẢNH BÁO khi số đó **tăng** (không làm đỏ).",
        "#",
        "# Định dạng: mỗi dòng `<tệp> <số câu>`; dòng bắt đầu bằng '#' là chú thích.",
        "# Tệp này KHÔNG phải ngưỡng: nó chỉ là mốc so sánh, do checker ghi tự động (trừ --no-record).",
        "# Xoá tệp = nhận mốc mới (không cảnh báo) — nhưng tệp nằm trong git nên việc xoá hiện trong diff.",
        f"# cập nhật: {datetime.now().astimezone().isoformat(timespec='seconds')}",
    ]
    for rel in DOCS:
        if rel in counts:
            lines.append(f"{rel} {counts[rel]}")
    for rel in sorted(counts):
        if rel not in DOCS:
            lines.append(f"{rel} {counts[rel]}")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
    return True


def report_skips(results: dict[str, DocResult]) -> None:
    """In **tổng** số câu bị bỏ qua + từng câu (tệp:dòng, từ khoá, 60 ký tự đầu).

    In ra **stdout** (không phải stderr) để nằm trong log cổng — người đọc log thấy ngay phép kiểm
    đang bị nới bao nhiêu, kể cả khi cổng xanh.
    """
    if not results:
        return
    total = sum(len(r.skips) for r in results.values())
    claims = sum(r.claims for r in results.values())
    per_doc = ", ".join(f"{rel} {len(r.skips)}" for rel, r in results.items())
    print(f"# check-scope-table: bỏ qua {total}/{claims} câu khẳng định (từ khoá loại trừ) — {per_doc}")
    for rel, res in results.items():
        # Sắp theo số dòng để người đọc quét log từ trên xuống, không phải theo thứ tự dòng bảng.
        for s in sorted(res.skips, key=lambda x: x.line):
            print(f"#   {rel}:{s.line}  [{s.keyword}]  {_excerpt(s.sentence)}")


def report_skip_trend(root: str, counts: dict[str, int], record: bool) -> None:
    """So số câu bỏ qua với mốc; CẢNH BÁO (không đỏ) khi **tăng**; ghi mốc mới."""
    path = os.path.join(root, SKIP_COUNT_FILE)
    prev = read_counts(path)
    total_now = sum(counts.values())
    if prev is None:
        print(f"# check-scope-table: chưa có mốc bỏ qua ({SKIP_COUNT_FILE}) — lần này {total_now} câu")
    else:
        total_prev = sum(prev.values())
        if total_now > total_prev:
            # Cảnh báo phải **nổi bật** nhưng **không** làm đỏ: Agent 0 nói "cảnh báo trong log".
            print("# " + "=" * 76)
            print(
                f"# CẢNH BÁO: số câu bỏ qua tăng {total_prev} → {total_now} — ai đó vừa nới lỏng phép kiểm?"
            )
            for rel in sorted(set(prev) | set(counts)):
                before, after = prev.get(rel, 0), counts.get(rel, 0)
                if after > before:
                    print(f"#   {rel}: {before} → {after}")
            print(
                "#   từ khoá loại trừ là lỗ có thể rộng dần:"
                " xem QUALIFIERS trong v2/tools/check-scope-table.py"
            )
            print(f"#   mốc: {SKIP_COUNT_FILE}")
            print("# " + "=" * 76)
        else:
            print(f"# check-scope-table: số câu bỏ qua {total_now} (mốc {total_prev}) — không tăng")
    if not record:
        print("# check-scope-table: --no-record — không ghi mốc")
        return
    if write_counts(path, counts):
        print(f"# check-scope-table: đã ghi mốc {total_now} vào {SKIP_COUNT_FILE}")


def run(
    root: str,
    quiet: bool = False,
    record: bool = True,
    docs: list[str] | None = None,
) -> int:
    docs = DOCS if docs is None else docs
    fails = 0
    results: dict[str, DocResult] = {}
    parsed_all = True
    for rel in docs:
        path = os.path.join(root, rel)
        if not os.path.isfile(path):
            print(f"not ok - {rel}: không có tệp", file=sys.stderr)
            fails += 1
            parsed_all = False
            continue
        try:
            res = check_doc(path)
        except Fail as exc:
            print(f"not ok - {rel}: {exc.code} {exc.detail}", file=sys.stderr)
            fails += 1
            parsed_all = False
            continue
        results[rel] = res
        if res.errors:
            for e in res.errors:
                print(f"not ok - {rel}: {e}", file=sys.stderr)
            fails += len(res.errors)
        elif not quiet:
            print(f"ok - {rel}: bảng đối chiếu khớp khối scope=")
    # Vế đếm **luôn** in, kể cả `--quiet`: đây là thứ Agent 0 yêu cầu thấy trong log cổng.
    report_skips(results)
    if parsed_all:
        report_skip_trend(root, {rel: len(r.skips) for rel, r in results.items()}, record)
    else:
        # Không đọc được đủ tài liệu ⇒ số câu bỏ qua là **thiếu**; so/ghi mốc sẽ làm mốc tụt
        # xuống một cách giả tạo (và lần chạy xanh sau đó bị cảnh báo oan).
        print("# check-scope-table: có tài liệu không đọc được — KHÔNG so/ghi mốc bỏ qua")
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

# Tài liệu có **đúng một** câu bị bỏ qua (dùng cho ca cảnh báo/ghi mốc).
SKIP_DOC = GOOD.replace(
    "Văn xuôi nói `alpha` bằng một",
    "Văn xuôi nói `alpha` bằng một; nhánh `alpha` bằng hai chưa có ca; `beta` bằng không",
)

# ---------------------------------------------------------------- fixture cho luật KT-142
#
# `alpha` có **hai** giá trị (1 ở khối `scope=`, 2 ở khối `scope-red=`) trong **cùng** tiết 1;
# `gamma` có ba giá trị (0 ở khối xanh, 5 ở khối đỏ tiết 1, 7 ở khối đỏ tiết 2). Dòng bảng trỏ
# khối **xanh** cho cả hai. Nhờ vậy mỗi luật quy khối đều có ca dương **và** ca âm:
#
#   - câu ghi loại khối (`scope-red`) cùng tiết  ⇒ luật 2;
#   - câu ghi tên khối (`scope-red=log-red.log`) ⇒ luật 1;
#   - câu không ghi khối, tiết chỉ có một khối chứa trường (`gamma` ở tiết 2) ⇒ luật 3;
#   - câu không ghi khối, tiết có **hai** khối nói khác nhau (`alpha` ở tiết 1) ⇒ không quy được,
#     phải rơi về khối của dòng bảng — ca này là ca chứng minh bước "duy nhất" thật sự có tác dụng.
#
# Khối `scope-red=` cố ý đứng **trước** khối `scope=` trong tiết 1: ca "hai khối nói khác nhau"
# chỉ bắt được đột biến "bỏ bước duy nhất" khi giá trị của khối **đầu** khác giá trị dòng bảng
# (đột biến trả `values[0]` = 2, dòng bảng = 1, câu nói 2 ⇒ đột biến xanh oan).
KT142 = """# Tài liệu giả

## 1. Tiết một

PROSE1

```text scope-red=log-red.log
# alpha=2 gamma=5
```

```text scope=log-a.log
# alpha=1 gamma=0
```

## 2. Tiết hai

PROSE2

```text scope-red=log-red2.log
# gamma=7
```

## Bảng đối chiếu số ↔ khối `scope=`

| Trường đo | Giá trị (chữ) | Loại khối | Tệp log | Khối |
|---|---|---|---|---|
| `alpha` | một | `scope=` | `log-a.log` | §1 |
| `gamma` | không | `scope=` | `log-a.log` | §1 |
```
"""

# ---------------------------------------------------------------- fixture cho luật KT-140
#
# `MULTI`: `alpha` có **hai** giá trị trong tài liệu (`1` ở §1, `2` ở §2) ⇒ dòng bảng **buộc** phải
# ghi cột "Khối". `beta` chỉ có **một** giá trị (`0`) ⇒ ô "Khối" để trống vẫn hợp lệ (ca (a) của
# Agent 0: "trường một giá trị, không cần ghi khối ⇒ xanh").
MULTI = """# Tài liệu giả

Văn xuôi nói `alpha` bằng một.

## 1. Tiết một

```text scope=log-a.log
# alpha=1 beta=0
```

## 2. Tiết hai

```text scope=log-b.log
# alpha=2 beta=0
```

## Bảng đối chiếu số ↔ khối `scope=`

| Trường đo | Giá trị (chữ) | Loại khối | Tệp log | Khối |
|---|---|---|---|---|
| `alpha` | một | `scope=` | `log-a.log` | §1 |
| `beta` | không | `scope=` | `log-a.log` | |
```
"""

# `MULTI_HEX`: đúng lớp lỗi KT-140 — `alpha` có `0x10` ở §1 và `1` ở §2. Nếu phép đếm **bỏ hex** thì
# trường này trông như **một** giá trị và luật bỏ sót đúng ca `observed` của `k6-exec.md`. Dòng bảng
# trỏ §2 (khối thập phân) nên **xanh**; bỏ cột "Khối" ⇒ phải đỏ `E_AMBIGUOUS_FIELD`.
MULTI_HEX = """# Tài liệu giả

Văn xuôi nói `alpha` bằng một.

## 1. Tiết một

```text scope=log-a.log
# alpha=0x10 beta=0
```

## 2. Tiết hai

```text scope=log-b.log
# alpha=1 beta=0
```

## Bảng đối chiếu số ↔ khối `scope=`

| Trường đo | Giá trị (chữ) | Loại khối | Tệp log | Khối |
|---|---|---|---|---|
| `alpha` | một | `scope=` | `log-b.log` | §2 |
| `beta` | không | `scope=` | `log-a.log` | |
```
"""

# Hồi quy hex của KT-134 **+** luật mới: hai khối **cùng tiết** §1, khối hex đứng trước. `alpha` có
# hai giá trị (0x10, 1) ⇒ dòng bảng **buộc** ghi §1; trong §1 khối hex **không** được che khối thập
# phân ⇒ resolve ra 1 (xanh). Bỏ bước "hex không phải thập phân" thì ca này đỏ.
HEX_FIRST = """# Tài liệu giả

Văn xuôi nói `alpha` bằng một và `beta` bằng không.

## 1. Tiết một

```text scope=log-a.log
# alpha=0x10 beta=0
```

```text scope=log-a.log
# alpha=1 beta=0
```

## Bảng đối chiếu số ↔ khối `scope=`

| Trường đo | Giá trị (chữ) | Loại khối | Tệp log | Khối |
|---|---|---|---|---|
| `alpha` | một | `scope=` | `log-a.log` | §1 |
| `beta` | không | `scope=` | `log-a.log` | §1 |
```
"""

ALPHA_ROW = "| `alpha` | một | `scope=` | `log-a.log` | §1 |"
BETA_ROW = "| `beta` | không | `scope=` | `log-a.log` | |"


def _run_capture(root: str, record: bool, docs: list[str]) -> tuple[int, str]:
    """Chạy `run` và **bắt stdout** — ca cảnh báo phải kiểm chữ in ra, không chỉ mã exit."""
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = run(root, quiet=True, record=record, docs=docs)
    return rc, buf.getvalue()


def _write_doc(root: str, rel: str, body: str) -> str:
    path = os.path.join(root, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(body)
    return path


def selftest() -> int:
    ok = 0
    bad = 0
    # (tên, nội dung, mã mong đợi ("" = xanh), số câu bỏ qua mong đợi (None = không kiểm))
    cases: list[tuple[str, str, str, int | None]] = []

    cases.append(("dương: tài liệu đúng", GOOD, "", 0))
    cases.append(
        (
            "dương: câu có từ khoá tình huống khác thì bỏ qua (câu cùng dòng không có từ khoá vẫn được so)",
            GOOD.replace(
                "Văn xuôi nói `alpha` bằng một",
                "Văn xuôi nói `alpha` bằng một; nhánh `alpha` bằng hai chưa có ca; `beta` bằng không",
            ),
            "",
            1,
        )
    )
    # (a) dương: **đếm** đúng số câu bị bỏ qua — hai câu, hai từ khoá khác nhau.
    cases.append(
        (
            "dương: hai câu bị bỏ qua thì đếm đúng 2",
            GOOD.replace(
                "Văn xuôi nói `alpha` bằng một và `beta` bằng không.",
                "Văn xuôi nói `alpha` bằng một và `beta` bằng không. "
                "Nhánh `alpha` bằng hai chưa có ca. Nếu `beta` bằng một thì đỏ.",
            ),
            "",
            2,
        )
    )
    # ---- KT-142: câu nói về khối nào thì so với **khối đó** (Agent 0 lượt 4 câu 3) ----
    #
    # Bốn ca dưới đây là **lý do** năm từ khoá loại trừ đã bị bỏ: câu nói về khối `scope-red=`
    # trước kia phải miễn bằng từ khoá, nay được so thật. Mỗi luật quy khối có một ca dương (đúng
    # số ⇒ xanh) **và** một ca âm (sai số ⇒ đỏ) — ca dương một mình không chứng minh gì, vì một
    # hàm luôn trả `None` cũng làm mọi ca dương xanh.
    cases.append(
        (
            "KT-142 dương: câu ghi **loại** khối `scope-red` cùng tiết ⇒ so với khối đỏ, xanh",
            KT142.replace("PROSE1", "Ca âm cho thấy `alpha` bằng hai (khối `scope-red` dưới)."),
            "",
            0,
        )
    )
    cases.append(
        (
            "KT-142 âm: câu ghi loại khối `scope-red` cùng tiết mà **sai** số ⇒ đỏ",
            KT142.replace("PROSE1", "Ca âm cho thấy `alpha` bằng một (khối `scope-red` dưới)."),
            "E_VALUE_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "KT-142 dương: câu ghi **tên** khối `scope-red=<tệp>` ở tiết khác ⇒ so với khối đó, xanh",
            KT142.replace("PROSE2", "`alpha` bằng hai — khối `scope-red=log-red.log`."),
            "",
            0,
        )
    )
    cases.append(
        (
            "KT-142 âm: câu ghi tên khối `scope-red=<tệp>` mà **sai** số ⇒ đỏ",
            KT142.replace("PROSE2", "`alpha` bằng một — khối `scope-red=log-red.log`."),
            "E_VALUE_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "KT-142 dương: câu không ghi khối, cùng tiết chỉ có **một** khối chứa trường ⇒ so khối đó",
            KT142.replace("PROSE2", "`gamma` bằng bảy."),
            "",
            0,
        )
    )
    cases.append(
        (
            "KT-142 âm: cùng tiết một khối chứa trường mà **sai** số ⇒ đỏ",
            KT142.replace("PROSE2", "`gamma` bằng không."),
            "E_VALUE_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "KT-142 âm: cùng tiết **hai** khối nói khác nhau ⇒ không quy được, rơi về khối dòng bảng",
            KT142.replace("PROSE1", "`alpha` bằng hai."),
            "E_VALUE_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "KT-142 dương: câu nói về khối đỏ **không** cần từ khoá nào ⇒ 0 câu bị bỏ qua",
            KT142.replace("PROSE1", "Ca âm cho thấy `alpha` bằng hai (khối `scope-red` dưới)."),
            "",
            0,
        )
    )
    cases.append(
        (
            "âm: cột giá trị lệch khối",
            GOOD.replace("| `alpha` | một |", "| `alpha` | hai |"),
            "E_TABLE_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "âm: văn xuôi lệch khối",
            GOOD.replace("`alpha` bằng một", "`alpha` bằng hai"),
            "E_VALUE_MISMATCH",
            None,
        )
    )
    # (b) âm: câu **không** có từ khoá mà lệch giá trị ⇒ vẫn đỏ, và **không** câu nào bị bỏ qua.
    #     Đây là ca chứng minh bộ lọc không nuốt quá nhiều: nếu ai thêm từ khoá quá rộng (vd "số"),
    #     ca này sẽ xanh và selftest đỏ.
    cases.append(
        (
            "âm: câu KHÔNG có từ khoá mà lệch giá trị vẫn đỏ (bộ lọc không nuốt quá nhiều)",
            GOOD.replace("`alpha` bằng một", "`alpha` bằng hai — số đo của khối"),
            "E_VALUE_MISMATCH",
            0,
        )
    )
    cases.append(
        (
            "âm: đột biến một số trong log",
            GOOD.replace("# alpha=1 beta=0", "# alpha=7 beta=0"),
            "E_TABLE_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "âm: trỏ khối không có",
            GOOD.replace("`log-a.log` |\n| `beta`", "`log-khong-co.log` |\n| `beta`"),
            "E_BLOCK_MISSING",
            None,
        )
    )
    cases.append(
        (
            "âm: trường không có trong khối",
            GOOD.replace("# alpha=1 beta=0", "# alpha=1 gamma=0"),
            "E_FIELD_MISSING",
            None,
        )
    )
    cases.append(
        (
            "âm: cột giá trị không phải chữ số",
            GOOD.replace("| `alpha` | một |", "| `alpha` | bốn mươi |"),
            "E_WORD_UNKNOWN",
            None,
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
            None,
        )
    )
    cases.append(
        (
            "âm: khối không đóng",
            GOOD.replace("# alpha=1 beta=0\n```", "# alpha=1 beta=0"),
            "E_BLOCK_UNCLOSED",
            None,
        )
    )
    # Hồi quy cho lỗi hex: `alpha=0x10` **không** được đọc thành `alpha=0` (xanh giả).
    cases.append(
        (
            "âm: giá trị hex không được đọc thành số thập phân (0x10 ≠ 0)",
            GOOD.replace("# alpha=1 beta=0", "# alpha=0x10 beta=0"),
            "E_FIELD_MISSING",
            None,
        )
    )
    # ... nhưng khối hex đứng **trước** không được che khối thập phân đứng sau (đúng luật "khối
    # đầu tiên có giá trị thập phân" — ca thật: `observed` trong k6-exec.md). Hai khối **cùng tiết**
    # §1 nên ca này còn kiểm luôn rằng cột "Khối" **không** làm mất phép kiểm hex.
    cases.append(
        (
            "dương: khối hex đứng trước trong cùng tiết ⇒ lấy khối thập phân",
            HEX_FIRST,
            "",
            0,
        )
    )

    # ---------------------------------------------------------------- KT-140: E_AMBIGUOUS_FIELD
    #
    # Ba ca Agent 0 yêu cầu, cộng ba ca phụ cho vế "chỉ rõ khối phải **đúng**":
    #   (a) trường **một** giá trị, không ghi khối ⇒ xanh;
    #   (b) trường **nhiều** giá trị, dòng bảng **không** ghi khối ⇒ đỏ `E_AMBIGUOUS_FIELD`;
    #   (c) trường **nhiều** giá trị, dòng bảng ghi khối **đúng** ⇒ xanh.
    cases.append(
        (
            "(a) dương: trường một giá trị, ô 'Khối' để trống ⇒ xanh",
            # Bỏ khối §2 ⇒ `alpha` chỉ còn **một** giá trị (1); ô "Khối" để trống vẫn hợp lệ.
            MULTI.replace(
                "## 2. Tiết hai\n\n```text scope=log-b.log\n# alpha=2 beta=0\n```\n\n", ""
            ).replace(ALPHA_ROW, "| `alpha` | một | `scope=` | `log-a.log` | |"),
            "",
            0,
        )
    )
    cases.append(
        (
            "(b) âm: trường nhiều giá trị, dòng bảng KHÔNG ghi khối ⇒ E_AMBIGUOUS_FIELD",
            MULTI.replace(ALPHA_ROW, "| `alpha` | một | `scope=` | `log-a.log` | |"),
            "E_AMBIGUOUS_FIELD",
            None,
        )
    )
    cases.append(
        (
            "(c) dương: trường nhiều giá trị, dòng bảng ghi khối ĐÚNG ⇒ xanh",
            MULTI,
            "",
            0,
        )
    )
    cases.append(
        (
            "âm: trường nhiều giá trị, dòng bảng ghi khối SAI ⇒ E_SECTION_MISMATCH",
            MULTI.replace(ALPHA_ROW, "| `alpha` | một | `scope=` | `log-a.log` | §2 |"),
            "E_SECTION_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "âm: cột 'Khối' ghi tiết không có trong tài liệu ⇒ E_SECTION_UNKNOWN",
            MULTI.replace(ALPHA_ROW, "| `alpha` | một | `scope=` | `log-a.log` | §9 |"),
            "E_SECTION_UNKNOWN",
            None,
        )
    )
    cases.append(
        (
            "âm: cột 'Khối' không đúng dạng §<số> ⇒ E_SECTION_UNKNOWN",
            MULTI.replace(ALPHA_ROW, "| `alpha` | một | `scope=` | `log-a.log` | tiết 1 |"),
            "E_SECTION_UNKNOWN",
            None,
        )
    )
    # Vế "khối phải **đúng**" phải có hiệu lực **cả khi** trường chỉ có một giá trị — nếu không,
    # cột "Khối" chỉ là lời khai và một dòng trỏ nhầm tiết vẫn xanh.
    cases.append(
        (
            "âm: trường một giá trị nhưng cột 'Khối' trỏ nhầm tiết ⇒ E_SECTION_MISMATCH",
            MULTI.replace(BETA_ROW, "| `beta` | không | `scope=` | `log-a.log` | §2 |"),
            "E_SECTION_MISMATCH",
            None,
        )
    )
    cases.append(
        (
            "dương: trường một giá trị, cột 'Khối' trỏ đúng tiết ⇒ xanh",
            MULTI.replace(BETA_ROW, "| `beta` | không | `scope=` | `log-a.log` | §1 |"),
            "",
            0,
        )
    )
    # Đếm **cả hex**: `alpha` có `0x10` (§1) và `1` (§2) ⇒ hai giá trị. Bỏ hex khỏi phép đếm thì ca
    # này xanh và luật bỏ sót đúng ca `observed` của k6-exec.md.
    cases.append(
        (
            "âm: nhiều giá trị vì có hex (0x10 vs 1), không ghi khối ⇒ E_AMBIGUOUS_FIELD",
            MULTI_HEX.replace("| `log-b.log` | §2 |", "| `log-b.log` | |"),
            "E_AMBIGUOUS_FIELD",
            None,
        )
    )
    cases.append(
        (
            "dương: nhiều giá trị vì có hex, ghi khối đúng (§2, khối thập phân) ⇒ xanh",
            MULTI_HEX,
            "",
            0,
        )
    )
    # Hình dạng bảng: tiêu đề 5 cột mà dòng chỉ 4 cột ⇒ đỏ, không được "đọc rộng tay".
    cases.append(
        (
            "âm: dòng bảng thiếu cột so với tiêu đề ⇒ E_TABLE_ROW_SHAPE",
            MULTI.replace(BETA_ROW, "| `beta` | không | `scope=` | `log-a.log` |"),
            "E_TABLE_ROW_SHAPE",
            None,
        )
    )
    cases.append(
        (
            "âm: cột thứ năm của tiêu đề không tên 'Khối' ⇒ E_TABLE_HEADER",
            MULTI.replace("| Tệp log | Khối |", "| Tệp log | Tiết |"),
            "E_TABLE_HEADER",
            None,
        )
    )

    for name, body, want, want_skips in cases:
        with tempfile.TemporaryDirectory() as d:
            rel = DOCS[0]
            path = _write_doc(d, rel, body)
            try:
                res = check_doc(path)
                got = res.errors[0].split(":")[0] if res.errors else ""
                nskip = len(res.skips)
            except Fail as exc:
                got = exc.code
                nskip = -1
            problems: list[str] = []
            if want == "":
                if got != "":
                    problems.append(f"mong XANH, thấy {got}")
            elif got != want:
                problems.append(f"mong {want}, thấy {got or 'XANH'}")
            if want_skips is not None and nskip != want_skips:
                problems.append(f"mong bỏ qua {want_skips} câu, thấy {nskip}")
            if problems:
                print(f"not ok - selftest {name}: " + "; ".join(problems), file=sys.stderr)
                bad += 1
            else:
                detail = "XANH" if want == "" else f"ĐỎ đúng ({want})"
                if want_skips is not None:
                    detail += f", bỏ qua {nskip} câu"
                print(f"ok - selftest {name}: {detail}")
                ok += 1

    # (c) cảnh báo: số câu bỏ qua **tăng** so với mốc giả ⇒ in CẢNH BÁO, exit vẫn 0 (không đỏ).
    trend: list[tuple[str, int, bool]] = [
        ("cảnh báo: số bỏ qua tăng so với mốc giả ⇒ in CẢNH BÁO, không đỏ", 0, True),
        ("không cảnh báo: số bỏ qua bằng mốc", 1, False),
        ("không cảnh báo: số bỏ qua giảm so với mốc", 3, False),
    ]
    for name, prev, want_warn in trend:
        with tempfile.TemporaryDirectory() as d:
            rel = DOCS[0]
            _write_doc(d, rel, SKIP_DOC)
            cpath = os.path.join(d, SKIP_COUNT_FILE)
            os.makedirs(os.path.dirname(cpath), exist_ok=True)
            with open(cpath, "w", encoding="utf-8") as fh:
                fh.write(f"# mốc giả\n{rel} {prev}\n")
            rc, out = _run_capture(d, record=False, docs=[rel])
            problems = []
            if rc != 0:
                problems.append(f"mong exit 0 (cảnh báo KHÔNG làm đỏ), thấy {rc}")
            if want_warn and "# CẢNH BÁO" not in out:
                problems.append("mong có '# CẢNH BÁO' trong stdout, không thấy")
            if not want_warn and "# CẢNH BÁO" in out:
                problems.append("mong KHÔNG có '# CẢNH BÁO', lại thấy")
            if problems:
                print(f"not ok - selftest {name}: " + "; ".join(problems), file=sys.stderr)
                bad += 1
            else:
                print(f"ok - selftest {name}: {'có cảnh báo' if want_warn else 'không cảnh báo'}, exit 0")
                ok += 1

    # Ghi mốc: chưa có tệp ⇒ tạo tệp với số mới.
    with tempfile.TemporaryDirectory() as d:
        rel = DOCS[0]
        _write_doc(d, rel, SKIP_DOC)
        rc, _out = _run_capture(d, record=True, docs=[rel])
        got = read_counts(os.path.join(d, SKIP_COUNT_FILE))
        if rc == 0 and got == {rel: 1}:
            print("ok - selftest ghi mốc: chưa có tệp ⇒ ghi đúng số câu bỏ qua")
            ok += 1
        else:
            print(f"not ok - selftest ghi mốc: mong {{{rel!r}: 1}}, thấy {got} (exit {rc})", file=sys.stderr)
            bad += 1

    # --no-record: **không** được tạo tệp mốc.
    with tempfile.TemporaryDirectory() as d:
        rel = DOCS[0]
        _write_doc(d, rel, SKIP_DOC)
        rc, _out = _run_capture(d, record=False, docs=[rel])
        cpath = os.path.join(d, SKIP_COUNT_FILE)
        if rc == 0 and not os.path.isfile(cpath):
            print("ok - selftest --no-record: không ghi tệp mốc")
            ok += 1
        else:
            print(
                f"not ok - selftest --no-record: mong không có tệp mốc, thấy {os.path.isfile(cpath)} (exit {rc})",
                file=sys.stderr,
            )
            bad += 1

    print(f"# check-scope-table-selftest: {'PASS' if bad == 0 else 'FAIL'} ({ok}/{ok + bad} ca)")
    return 0 if bad == 0 else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="KT-134 — kiểm máy bảng đối chiếu số ↔ khối scope=")
    ap.add_argument(
        "--root",
        default=os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
    )
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument(
        "--no-record",
        action="store_true",
        help="không ghi mốc số câu bỏ qua (dùng khi chạy thử, để không đổi mốc so sánh)",
    )
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    return run(args.root, args.quiet, record=not args.no_record)


if __name__ == "__main__":
    sys.exit(main())
