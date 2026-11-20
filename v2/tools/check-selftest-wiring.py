#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-134 (phần 3 của Agent 0) — kiểm **máy**: mọi script tự kiểm phải có cổng gọi.

Vì sao có tệp này
-----------------
`v2/test/boot/build-rev.sh --selftest` đã **ĐỎ từ trước** mà không ai biết, vì **không cổng nào
gọi nó**. Cùng lớp lỗi đó có thể tái diễn với bất kỳ script tự kiểm nào: viết xong, chạy tay một
lần, rồi để mục. Rà tay không chống được tái diễn — phải có kiểm máy, chạy trong cổng.

Tệp này làm bốn việc
--------------------
1. **Quét** `v2/` tìm script tự kiểm: tệp có chuỗi `--selftest`/`--red-selftest` **và** thực sự
   xử lý nó (nhánh `case`, so sánh kề cờ trong `if`/`[`/`test`, `add_argument`, hoặc hàm
   `*selftest`), không chỉ nhắc trong chú thích.
2. **Đối chiếu cổng**: tên tệp phải xuất hiện trên một **lời gọi thật** trong `v2/bin/v2` hoặc
   `v2/test/rules/*` — dòng đó phải chứa cờ tự kiểm và **không** phải chú thích.
3. **Báo cáo**: script không cổng nào gọi ⇒ ĐỎ `E_ORPHAN`; script trong `NGOAI_LE` (có lý do) ⇒
   in ra nhưng không đỏ.
4. **Báo cáo thêm** (không đỏ): script tự kiểm **độc lập** (không cờ) như `selfbreak.sh`,
   `neg-test*.sh` — cùng một lớp lỗi nhưng ngoài phạm vi đỏ của đặc tả; `--standalone-red` để
   biến chúng thành ĐỎ nếu Agent 0 muốn chặn cả lớp.

Chạy: `python3 v2/tools/check-selftest-wiring.py [--root R] [--selftest] [--standalone-red]`
Exit: 0 = xanh, 1 = có script mồ côi, 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

# ------------------------------------------------------------------ hằng số

# Cờ tự kiểm được công nhận. So khớp theo **ranh giới token** (xem FLAG_RE): `--selftest-stack`
# KHÔNG phải `--selftest`. Vì sao: `ktest.sh` có cờ riêng `--selftest-stack`, được `selftest.sh`
# gọi, và `selftest.sh` đã có cổng trong `gate --fast`. Nếu so khớp chuỗi con thì `ktest.sh` bị báo
# mồ côi oan — một dương tính giả như vậy sẽ dạy người ta bỏ qua công cụ, đúng thứ cần tránh.
CO_SELFTEST = ("--selftest", "--red-selftest")

FLAG_RE = re.compile(r"(?<![A-Za-z0-9_-])(--selftest|--red-selftest)(?![A-Za-z0-9_-])")

# Nhận diện "thực sự xử lý cờ" (không chỉ nhắc trong chú thích).
#   - nhánh `case`: `--selftest)` hoặc `--selftest|--help)`
#   - so sánh **kề cờ**: `[ "$1" = "--selftest" ]`, `[[ ... == ... ]]`, `test ... = ...`
#   - hàm có `selftest` trong tên: `selftest()`, `red_selftest()`, `do_selftest()`
#     (Không cho phép khoảng trắng giữa cờ và `|`/`)`: `--selftest || exit 1` là **lời gọi**,
#     không phải nhánh `case`; nếu nới ra thì chính dòng cổng bị coi là "xử lý cờ".)
RE_CASE_ARM = re.compile(r"(?:^|[\s|(])(--selftest|--red-selftest)[|)]")
#     So sánh phải **kề** cờ (chỉ khoảng trắng ở giữa): `= "--selftest"` hoặc `"--selftest" =`.
#     Vì sao không dùng `[ ... --selftest ... = ... ]` kiểu lỏng: dòng cổng
#     `run_step "…" bash "$V2_ROOT/test/boot/build-rev.sh" --selftest || fails=$((fails + 1))`
#     có `/test/` (khớp `\btest\b`) và `=` trong `fails=$((` ⇒ bị coi là "xử lý cờ" oan, và chính
#     `v2/bin/v2` thành "script tự kiểm mồ côi". Đây là dương tính giả đã gặp thật khi viết tệp này.
RE_COMPARE = re.compile(
    r"(?:=|==|!=)\s*['\"]?(--selftest|--red-selftest)(?![A-Za-z0-9_-])"
    r"|['\"]?(--selftest|--red-selftest)['\"]?\s*(?:=|==|!=)"
)
#     Tên hàm phải **kết thúc** bằng `selftest` (`selftest`, `red_selftest`, `do_selftest`).
#     Vì sao không cho `*selftest*`: `ktest.sh` có `do_selftest_stack()` cho cờ **khác**
#     (`--selftest-stack`) — nếu khớp chuỗi con thì `ktest.sh` bị báo mồ côi oan.
RE_FUNC = re.compile(r"^\s*(?:function\s+)?([A-Za-z0-9_]*selftest)\s*\(\s*\)")
RE_PY_ARG = re.compile(r"add_argument\(\s*['\"](--selftest|--red-selftest)['\"]")
RE_PY_ARGV = re.compile(
    r"sys\.argv[^\n]*(--selftest|--red-selftest)|(--selftest|--red-selftest)[^\n]*sys\.argv"
)
RE_PY_FUNC = re.compile(r"^\s*def\s+([A-Za-z0-9_]*selftest)\s*\(")

# Đường dẫn script xuất hiện trong một dòng cổng (để phát hiện "cổng gọi cờ vào script không xử
# lý cờ" — bước cổng tưởng kiểm mà thực ra không kiểm gì).
RE_DUONG_DAN_SCRIPT = re.compile(r"[A-Za-z0-9_./$@{}-]+\.(?:sh|py|ps1)\b")

# Ngoại lệ **có lý do**: script tự kiểm mà việc gọi từ cổng là không phù hợp. Mỗi mục là
# (đường-dẫn-tính-từ-gốc-repo, lý-do). Script trong danh sách này KHÔNG bị coi là mồ côi, nhưng
# vẫn được in ra (mục "NGOẠI LỆ") để người đọc thấy nó tồn tại và biết vì sao nó không qua cổng.
# Đừng để danh sách này rỗng: một ngoại lệ không tên = một lỗ mục âm thầm.
#
# Ghi chú lịch sử: `v2/test/boot/persist.sh` (`--red-selftest`) từng là ứng viên ngoại lệ (cần
# QEMU + build kernel/fsutil). Trong lúc làm task này, một thay đổi **song song** đã nối nó vào
# `gate --fast` ⇒ **không cần ngoại lệ nữa**. Nếu dòng đó bị gỡ, thêm lại:
#   ("v2/test/boot/persist.sh", "cần QEMU + build kernel/fsutil; `v2 check` không chạy QEMU")
NGOAI_LE: Tuple[Tuple[str, str], ...] = (
    # KT-134: **rỗng là trạng thái đúng** hiện nay — mọi script tự kiểm đều đã có cổng gọi.
    # Thêm một mục vào đây là **miễn trừ** một script khỏi phép kiểm, nên phải kèm lý do thật.
    # (Trước đây có một mục cho chính công cụ này; nay nó đã được nối vào `v2 check` nên bỏ —
    # để lại sẽ sinh `W_EXEMPT_UNUSED`, đúng như công cụ tự báo.)
    #
    # **KT-217 (D2-173)** — `v2/tools/check-wake-depinfo.py` **đã** được nối vào `v2 check`
    # (lớp 2 cho luật `wake`), nên mục miễn trừ của KT-214 đã **xoá** — để lại sẽ sinh
    # `W_EXEMPT_UNUSED`, đúng như công cụ tự báo.
    #
    # **KT-256** — `v2/tools/check-no-cfg-test.py` kiểm giả định "ảnh sản phẩm **không** có
    # `--cfg test`" bằng cách **build thật** ảnh sản phẩm (`--features product-fs`, ~40 s, cần
    # cargo + target `riscv64gc-unknown-none-elf`). `v2 check` là cổng **nhanh** (không build ảnh)
    # nên chưa nối; đề xuất nối vào `gate --fast` — một dòng `run_step` trong `v2/bin/v2`, **ngoài
    # quyền** của KT-256 (task cấm sửa `v2/bin/v2`). Ca tự kiểm của nó (10 ca trên log tổng hợp,
    # **không** cần cargo) chạy độc lập: `python3 v2/tools/check-no-cfg-test.py --selftest`.
    (
        "v2/tools/check-no-cfg-test.py",
        "build thật ảnh sản phẩm (~40 s, cần cargo + target riscv64) — không phù hợp `v2 check` "
        "(cổng nhanh); đề xuất nối vào `gate --fast` (cần sửa `v2/bin/v2`, ngoài quyền KT-256)",
    ),
)

# Script tự kiểm **độc lập** (không có cờ): nhận theo tên tệp. Mục này CHỈ BÁO CÁO — không tính
# mồ côi đỏ — vì đặc tả của task định nghĩa ĐỎ cho script có cờ `--selftest`/`--red-selftest`.
# Vì sao vẫn in: cùng một lớp lỗi (viết xong, không cổng nào gọi) vẫn xảy ra với lớp này; in ra
# để Agent 0 quyết nối hay thêm ngoại lệ, thay vì để nó mục âm thầm lần nữa.
RE_TEN_DOC_LAP = re.compile(
    r"(selftest|self-test|selfbreak|neg-test|run_fixtures|run_idl_fixtures)", re.IGNORECASE
)

# Thư mục bỏ qua khi quét (đặc tả: `v2/agent/`, `v2/docs/`, `v2/target*`, `v2/artifacts/`).
# `target*` bỏ ở **mọi** độ sâu (không chỉ `v2/target/`) vì mỗi crate có `target/` riêng.
THU_MUC_BO_QUA = ("agent", "docs", "artifacts")


# ------------------------------------------------------------------ tiện ích

def bo_chu_thich(line: str, marker: str = "#") -> str:
    """Bỏ phần chú thích `#` **ngoài chuỗi**; giữ nguyên phần mã.

    Vì sao cần: đặc tả nói dòng cổng phải là **lời gọi thật**, không phải chỉ nhắc trong chú
    thích. `v2/bin/v2` có ít nhất ba dòng chú thích nhắc `build-rev.sh --selftest`; nếu không bỏ
    chú thích thì chính những dòng đó sẽ "chứng minh" script đã có cổng — đúng dương tính giả
    làm lỗi tái diễn.
    """
    out: List[str] = []
    quote: Optional[str] = None
    i = 0
    while i < len(line):
        ch = line[i]
        if quote is not None:
            out.append(ch)
            if ch == quote:
                quote = None
        elif ch in ("'", '"'):
            quote = ch
            out.append(ch)
        elif ch == marker and (i == 0 or line[i - 1] in " \t"):
            break
        else:
            out.append(ch)
        i += 1
    return "".join(out)


def doc_tep(p: Path) -> str:
    try:
        return p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def la_script(p: Path) -> bool:
    """Tệp có phải script không (theo đuôi hoặc shebang + quyền thực thi)."""
    if p.suffix.lower() in (".sh", ".bash", ".py", ".ps1"):
        return True
    if p.suffix == "" and os.access(p, os.X_OK):
        try:
            with p.open("rb") as f:
                return f.read(2) == b"#!"
        except OSError:
            return False
    return False


def xu_ly_selftest(p: Path, text: str) -> bool:
    """Tệp có **thực sự xử lý** cờ tự kiểm không (không chỉ nhắc trong chú thích)?"""
    la_py = p.suffix.lower() == ".py"
    for line in text.splitlines():
        code = bo_chu_thich(line)
        if not code.strip():
            continue
        if la_py:
            if RE_PY_ARG.search(code) or RE_PY_ARGV.search(code) or RE_PY_FUNC.search(code):
                return True
        else:
            if RE_CASE_ARM.search(code) or RE_COMPARE.search(code) or RE_FUNC.search(code):
                return True
    return False


def co_ten(code: str, ten: str) -> bool:
    """Tên tệp xuất hiện như một **token đường dẫn** (không khớp `build-rev.sh.bak`)."""
    return (
        re.search(
            r"(?<![A-Za-z0-9_.-])" + re.escape(ten) + r"(?![A-Za-z0-9_.-])", code
        )
        is not None
    )


# ------------------------------------------------------------------ quét

def quet_ung_vien(root: Path) -> List[Path]:
    """Mọi script tự kiểm (có cờ và thực sự xử lý cờ) dưới `root/v2`."""
    v2 = root / "v2"
    ket_qua: List[Path] = []
    for dirpath, dirnames, filenames in os.walk(v2):
        # Bỏ thư mục loại trừ + mọi `target*` (kể cả `kernel/target/`).
        dirnames[:] = [
            d
            for d in dirnames
            if d not in THU_MUC_BO_QUA and not d.startswith("target")
        ]
        for fn in filenames:
            p = Path(dirpath) / fn
            if not la_script(p):
                continue
            text = doc_tep(p)
            if not any(f in text for f in CO_SELFTEST):
                continue
            if not xu_ly_selftest(p, text):
                continue
            ket_qua.append(p)
    return sorted(ket_qua)


def liet_ke_script(root: Path) -> List[Path]:
    """Mọi script dưới `root/v2` (để tra tên tệp trong dòng cổng + mục báo cáo độc lập)."""
    v2 = root / "v2"
    ket_qua: List[Path] = []
    for dirpath, dirnames, filenames in os.walk(v2):
        dirnames[:] = [
            d
            for d in dirnames
            if d not in THU_MUC_BO_QUA and not d.startswith("target")
        ]
        for fn in filenames:
            p = Path(dirpath) / fn
            if la_script(p):
                ket_qua.append(p)
    return sorted(ket_qua)


def tep_cong(root: Path) -> List[Path]:
    """Tệp cổng: `v2/bin/v2` + mọi tệp trong `v2/test/rules/` (đặc tả)."""
    v2 = root / "v2"
    ket_qua: List[Path] = []
    gate = v2 / "bin" / "v2"
    if gate.is_file():
        ket_qua.append(gate)
    rules = v2 / "test" / "rules"
    if rules.is_dir():
        for p in sorted(rules.rglob("*")):
            if p.is_file():
                ket_qua.append(p)
    return ket_qua


def tim_loi_goi(
    root: Path,
    rel: str,
    ten: str,
    can_co: bool,
    cho_basename: bool,
) -> List[Tuple[Path, int, str]]:
    """Các **lời gọi thật** tới script trong tệp cổng.

    `can_co=True`  ⇒ dòng phải chứa cờ tự kiểm (script có cờ).
    `can_co=False` ⇒ chỉ cần gọi tên (script tự kiểm độc lập, không cờ).
    Dòng chú thích bị loại bằng `bo_chu_thich` — đó là điều kiện "không phải chỉ nhắc".
    """
    hits: List[Tuple[Path, int, str]] = []
    for gate in tep_cong(root):
        for i, line in enumerate(doc_tep(gate).splitlines(), 1):
            code = bo_chu_thich(line)
            if not code.strip():
                continue
            if can_co and not FLAG_RE.search(code):
                continue
            if rel in code or (cho_basename and co_ten(code, ten)):
                hits.append((gate, i, line.strip()))
    return hits


# ------------------------------------------------------------------ báo cáo

def _in_bang(rows: Sequence[Tuple[str, str, str]]) -> None:
    print("| script | cổng gọi? | cổng nào |")
    print("|---|---|---|")
    for rel, trang_thai, cong_nao in rows:
        print(f"| {rel} | {trang_thai} | {cong_nao} |")


def kiem(
    root: Path,
    standalone_red: bool = False,
    extra_exempt: Sequence[Tuple[str, str]] = (),
) -> int:
    v2 = root / "v2"
    if not v2.is_dir():
        print(f"check-selftest-wiring: không có thư mục {v2}", file=sys.stderr)
        return 2

    ung_vien = quet_ung_vien(root)
    tat_ca = liet_ke_script(root)
    cong = tep_cong(root)
    # Ngoại lệ = hằng số trong code + (chỉ khi `--selftest`/thử nghiệm) mục tiêm qua `--exempt`.
    ngoai_le = dict(NGOAI_LE)
    ngoai_le.update(extra_exempt)

    # Bản đồ basename -> [rel] để tra tên tệp trong dòng cổng.
    ban_do: Dict[str, List[str]] = {}
    for p in tat_ca:
        ban_do.setdefault(p.name, []).append(p.relative_to(root).as_posix())

    ung_vien_rel = {p.relative_to(root).as_posix() for p in ung_vien}

    print(f"== check-selftest-wiring: quét {root} ==")
    print(f"# tệp cổng: v2/bin/v2 + v2/test/rules/ ({len(cong)} tệp)")
    print(f"# script tự kiểm có cờ: {len(ung_vien)}")

    rows: List[Tuple[str, str, str]] = []
    mo_coi: List[str] = []
    da_ngoai_le: List[str] = []
    for p in ung_vien:
        rel = p.relative_to(root).as_posix()
        hits = tim_loi_goi(root, rel, p.name, can_co=True, cho_basename=True)
        if hits:
            trang_thai = "CÓ"
            cong_nao = ", ".join(
                f"{g.relative_to(root).as_posix()}:{n}" for g, n, _ in hits
            )
        elif rel in ngoai_le:
            trang_thai = "NGOẠI LỆ"
            cong_nao = "—"
            da_ngoai_le.append(rel)
        else:
            trang_thai = "KHÔNG"
            cong_nao = "—"
            mo_coi.append(rel)
        rows.append((rel, trang_thai, cong_nao))

    # --- script tự kiểm độc lập (không cờ): tính TRƯỚC để ngoại lệ + `--standalone-red` dùng
    #     chung một cơ chế với lớp có cờ ------------------------------------------------------
    doc_lap_rows: List[Tuple[str, str, str]] = []
    doc_lap_mo_coi: List[str] = []
    for p in tat_ca:
        rel = p.relative_to(root).as_posix()
        if rel in ung_vien_rel:
            continue
        if not RE_TEN_DOC_LAP.search(p.name):
            continue
        # Tên trùng (vd 4 tệp `run_fixtures.sh`) ⇒ phải khớp **đường dẫn tương đối**, không
        # khớp basename, nếu không tệp không được gọi cũng bị coi là "có cổng".
        cho_basename = len(ban_do[p.name]) == 1
        hits = tim_loi_goi(root, rel, p.name, can_co=False, cho_basename=cho_basename)
        if hits:
            trang_thai = "CÓ"
            cong_nao = ", ".join(
                f"{g.relative_to(root).as_posix()}:{n}" for g, n, _ in hits
            )
        elif rel in ngoai_le:
            # Ngoại lệ dùng chung cho cả hai lớp: script độc lập cũng có thể được miễn có lý do.
            trang_thai = "NGOẠI LỆ"
            cong_nao = "—"
            da_ngoai_le.append(rel)
        else:
            trang_thai = "KHÔNG"
            cong_nao = "—"
            doc_lap_mo_coi.append(rel)
        doc_lap_rows.append((rel, trang_thai, cong_nao))

    print()
    _in_bang(rows)

    # --- ngoại lệ: in ra để người đọc thấy, kèm lý do -------------------------------------
    print()
    print("NGOẠI LỆ (có lý do — không tính mồ côi)")
    if not da_ngoai_le:
        print("- (không có ngoại lệ nào được dùng)")
    for rel in da_ngoai_le:
        print(f"- {rel}: {ngoai_le[rel]}")

    # --- cảnh báo: ngoại lệ thừa / cổng gọi cờ vào script không xử lý cờ ------------------
    canh_bao: List[str] = []
    for rel, ly_do in ngoai_le.items():
        if rel in da_ngoai_le:
            continue
        if (root / rel).exists():
            canh_bao.append(
                f"W_EXEMPT_UNUSED {rel}: có trong danh sách ngoại lệ nhưng đã có cổng gọi (hoặc không còn "
                f"là script tự kiểm) — xoá ngoại lệ để báo cáo khỏi nhiễu"
            )
    for gate in cong:
        rel_gate = gate.relative_to(root).as_posix()
        for i, line in enumerate(doc_tep(gate).splitlines(), 1):
            code = bo_chu_thich(line)
            if not code.strip() or not FLAG_RE.search(code):
                continue
            for token in RE_DUONG_DAN_SCRIPT.findall(code):
                ten = Path(token).name
                if ten not in ban_do or len(ban_do[ten]) != 1:
                    continue
                rel = ban_do[ten][0]
                if rel in ung_vien_rel:
                    continue
                canh_bao.append(
                    f"W_FAKE_SELFTEST {rel_gate}:{i}: gọi `{ten} --selftest` nhưng script KHÔNG "
                    f"xử lý cờ này (cờ bị bỏ qua im lặng) — bước cổng tưởng kiểm mà không kiểm gì"
                )
    if canh_bao:
        print()
        print("CẢNH BÁO (không làm đỏ)")
        for c in canh_bao:
            print(f"- {c}")

    # --- mồ côi: ĐỎ ----------------------------------------------------------------------
    if standalone_red:
        mo_coi.extend(doc_lap_mo_coi)
    if mo_coi:
        print()
        print("MỒ CÔI (ĐỎ)")
        for rel in mo_coi:
            if rel in doc_lap_mo_coi:
                print(
                    f"- E_ORPHAN {rel}: script tự kiểm độc lập, không cổng nào gọi "
                    f"(đã quét v2/bin/v2 + v2/test/rules/ ({len(cong)} tệp))"
                )
            else:
                print(
                    f"- E_ORPHAN {rel}: không cổng nào gọi `--selftest`/`--red-selftest` "
                    f"(đã quét v2/bin/v2 + v2/test/rules/ ({len(cong)} tệp))"
                )

    if doc_lap_rows:
        print()
        tieu_de = "BÁO CÁO — script tự kiểm độc lập (không cờ"
        tieu_de += "; ĐÃ tính mồ côi đỏ)" if standalone_red else "; KHÔNG tính mồ côi đỏ)"
        print(tieu_de)
        _in_bang(doc_lap_rows)

    # --- kết luận ------------------------------------------------------------------------
    print()
    if mo_coi:
        print(
            f"check-selftest-wiring: FAIL ({len(mo_coi)} script tự kiểm không cổng nào gọi: "
            f"{', '.join(mo_coi)})"
        )
        return 1
    canh_bao_txt = f"; {len(canh_bao)} cảnh báo" if canh_bao else ""
    # Đếm từ bảng (không trừ `da_ngoai_le`): ngoại lệ của lớp **độc lập** không được trừ vào
    # số script có cờ — nếu không, con số trong dòng kết luận sẽ sai khi có ngoại lệ độc lập.
    so_co_cong = sum(1 for _, tt, _ in rows if tt == "CÓ")
    so_ngoai_le = sum(1 for _, tt, _ in rows if tt == "NGOẠI LỆ")
    print(
        f"check-selftest-wiring: PASS ({len(ung_vien)} script tự kiểm: "
        f"{so_co_cong} có cổng, {so_ngoai_le} ngoại lệ, 0 mồ côi"
        f"{canh_bao_txt})"
    )
    return 0


# ------------------------------------------------------------------ tự kiểm

# Nội dung script giả **có** xử lý `--selftest`.
_GIA_CO_SELFTEST = """#!/usr/bin/env bash
set -u
if [ "${1:-}" = "--selftest" ]; then
    echo "# gia: selftest PASS"
    exit 0
fi
echo "gia"
"""

# Nội dung script giả **chỉ nhắc** `--selftest` trong chú thích (ca d: không được coi là tự kiểm).
_GIA_CHI_NHAC = """#!/usr/bin/env bash
# Tệp này chỉ NHẮC `--selftest` trong chú thích, KHÔNG xử lý cờ.
# (ca (d): không được coi là script tự kiểm — tránh dương tính giả)
echo "không có chế độ tự kiểm"
"""

# Nội dung script giả xử lý **cờ khác** `--selftest-stack` (ca g: không được coi là `--selftest`).
_GIA_STACK = """#!/usr/bin/env bash
set -u
case "${1:-}" in
    --selftest-stack) echo "# gia: stack"; exit 0 ;;
esac
echo "gia"
"""

# Nội dung script giả **không** xử lý cờ (ca f: cổng gọi cờ vào đây ⇒ cảnh báo, không đỏ).
_GIA_KHONG_CO = """#!/usr/bin/env bash
set -u
echo "gia: không có chế độ tự kiểm"
"""

_GIA_PY = """#!/usr/bin/env python3
import argparse
ap = argparse.ArgumentParser()
ap.add_argument("--selftest", action="store_true")
args = ap.parse_args()
print("# gia python: selftest" if args.selftest else "gia")
"""


def _dung_repo_gia(tmp: Path, scripts: Dict[str, str], dong_cong: Sequence[str]) -> None:
    """Dựng repo giả tối thiểu: `v2/bin/v2` (cổng) + các script trong `scripts`."""
    (tmp / "v2" / "bin").mkdir(parents=True, exist_ok=True)
    (tmp / "v2" / "test" / "rules").mkdir(parents=True, exist_ok=True)
    gate = tmp / "v2" / "bin" / "v2"
    gate.write_text(
        "#!/usr/bin/env bash\n# cổng giả cho --selftest của check-selftest-wiring\nset -u\n"
        + "\n".join(dong_cong)
        + "\n",
        encoding="utf-8",
    )
    gate.chmod(0o755)
    for rel, noi_dung in scripts.items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(noi_dung, encoding="utf-8")
        p.chmod(0o755)


def _chay_con(root: Path, extra: Sequence[str] = ()) -> Tuple[int, str]:
    proc = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--root", str(root), *extra],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return proc.returncode, proc.stdout


def chay_selftest() -> int:
    """Bốn ca bắt buộc (a..d) + năm ca bổ sung (e..i) — mỗi ca dựng repo giả trong thư mục tạm."""
    cases: List[Tuple[str, bool, str]] = []  # (nhãn, đúng như kỳ vọng?, chi tiết)

    def ghi(nhan: str, ok: bool, chi_tiet: str = "") -> None:
        cases.append((nhan, ok, chi_tiet))

    with tempfile.TemporaryDirectory(prefix="kt134-selftest-wiring.") as td:
        base = Path(td)

        # (a) DƯƠNG: mọi script tự kiểm đều có cổng gọi ⇒ xanh.
        a = base / "a"
        _dung_repo_gia(
            a,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/b.sh": _GIA_CO_SELFTEST,
                "v2/tools/c.py": _GIA_PY,
            },
            [
                'run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1',
                'run_step "b" bash "$V2_ROOT/test/boot/b.sh" --selftest || exit 1',
                'run_step "c" "$PY" "$V2_ROOT/tools/c.py" --selftest || exit 1',
            ],
        )
        rc, out = _chay_con(a)
        ghi(
            "(a) dương: mọi script tự kiểm đều có cổng gọi ⇒ xanh",
            rc == 0 and "E_ORPHAN" not in out and "| CÓ |" in out,
            f"exit={rc}",
        )

        # (b) ÂM: một script không có cổng gọi ⇒ đỏ E_ORPHAN, in đúng tên script.
        b = base / "b"
        _dung_repo_gia(
            b,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/d.sh": _GIA_CO_SELFTEST,
            },
            ['run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1'],
        )
        rc, out = _chay_con(b)
        ghi(
            "(b) âm: một script không có cổng gọi ⇒ đỏ E_ORPHAN, in đúng tên",
            rc == 1 and "E_ORPHAN v2/test/boot/d.sh" in out and "E_ORPHAN v2/test/boot/a.sh" not in out,
            f"exit={rc}",
        )

        # (c) DƯƠNG: script mồ côi nhưng nằm trong danh sách ngoại lệ ⇒ xanh, và **vẫn in** nó ra.
        #     Dùng `--exempt` (chỉ có tác dụng khi thử nghiệm) để ca kiểm **không phụ thuộc** vào
        #     nội dung `NGOAI_LE` thật — nếu không, Agent 0 xoá một ngoại lệ là ca kiểm đỏ oan.
        c = base / "c"
        _dung_repo_gia(
            c,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/orphan.sh": _GIA_CO_SELFTEST,
            },
            ['run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1'],
        )
        rc, out = _chay_con(c, ["--exempt", "v2/test/boot/orphan.sh=lý do của ca kiểm (c)"])
        ghi(
            "(c) dương: script mồ côi nằm trong ngoại lệ ⇒ xanh, vẫn in ra",
            rc == 0
            and "E_ORPHAN" not in out
            and "v2/test/boot/orphan.sh" in out
            and "NGOẠI LỆ" in out,
            f"exit={rc}",
        )

        # (d) ÂM: script chỉ **nhắc** `--selftest` trong chú thích, không xử lý ⇒ KHÔNG bị coi
        #     là script tự kiểm (tránh dương tính giả).
        d = base / "d"
        _dung_repo_gia(
            d,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/e.sh": _GIA_CHI_NHAC,
            },
            ['run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1'],
        )
        rc, out = _chay_con(d)
        ghi(
            "(d) âm: chỉ nhắc trong chú thích ⇒ không phải script tự kiểm",
            rc == 0 and "e.sh" not in out and "E_ORPHAN" not in out,
            f"exit={rc}",
        )

        # (e) ÂM: cổng chỉ **nhắc** script trong chú thích (không có lời gọi thật) ⇒ vẫn mồ côi.
        e = base / "e"
        _dung_repo_gia(
            e,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/f.sh": _GIA_CO_SELFTEST,
            },
            [
                'run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1',
                '# run_step "f" bash "$V2_ROOT/test/boot/f.sh" --selftest  (chú thích, KHÔNG gọi)',
            ],
        )
        rc, out = _chay_con(e)
        ghi(
            "(e) âm: cổng chỉ nhắc trong chú thích ⇒ vẫn mồ côi (không phải lời gọi)",
            rc == 1 and "E_ORPHAN v2/test/boot/f.sh" in out,
            f"exit={rc}",
        )

        # (f) CẢNH BÁO: cổng gọi cờ vào script **không** xử lý cờ ⇒ cảnh báo, không đỏ.
        f = base / "f"
        _dung_repo_gia(
            f,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/g.sh": _GIA_KHONG_CO,
            },
            [
                'run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1',
                'run_step "g" bash "$V2_ROOT/test/boot/g.sh" --selftest || exit 1',
            ],
        )
        rc, out = _chay_con(f)
        ghi(
            "(f) cảnh báo: cổng gọi cờ vào script không xử lý cờ ⇒ W_FAKE_SELFTEST, không đỏ",
            rc == 0 and "W_FAKE_SELFTEST" in out and "g.sh" in out,
            f"exit={rc}",
        )

        # (g) ÂM: `--selftest-stack` KHÔNG phải `--selftest` ⇒ không phải script tự kiểm.
        g = base / "g"
        _dung_repo_gia(
            g,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/h.sh": _GIA_STACK,
            },
            ['run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1'],
        )
        rc, out = _chay_con(g)
        ghi(
            "(g) âm: `--selftest-stack` không bị coi là `--selftest`",
            rc == 0 and "h.sh" not in out and "E_ORPHAN" not in out,
            f"exit={rc}",
        )

        # (h) `--standalone-red`: script tự kiểm **độc lập** mồ côi mặc định chỉ báo cáo, nhưng
        #     khi bật cờ thì thành ĐỎ. Vì sao cần: đặc tả chỉ định nghĩa ĐỎ cho script có cờ; lớp
        #     script độc lập (`selfbreak.sh`, `neg-test*.sh`) cùng một lỗi nhưng ngoài phạm vi đỏ.
        h = base / "h"
        _dung_repo_gia(
            h,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/rules/selfbreak.sh": _GIA_KHONG_CO,
            },
            ['run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1'],
        )
        rc_mac_dinh, out_mac_dinh = _chay_con(h)
        rc_chat, out_chat = _chay_con(h, ["--standalone-red"])
        ghi(
            "(h) `--standalone-red`: độc lập mồ côi mặc định báo cáo, bật cờ thì đỏ",
            rc_mac_dinh == 0
            and "selfbreak.sh" in out_mac_dinh
            and rc_chat == 1
            and "E_ORPHAN v2/test/rules/selfbreak.sh" in out_chat,
            f"exit mặc định={rc_mac_dinh}, exit --standalone-red={rc_chat}",
        )

        # (i) Ngoại lệ dùng chung cho lớp **độc lập**: script độc lập nằm trong danh sách ngoại lệ
        #     thì không đỏ kể cả khi bật `--standalone-red`. Nội dung **không** xử lý cờ ⇒ nó chỉ
        #     là script độc lập theo tên, không phải ứng viên có cờ.
        i = base / "i"
        _dung_repo_gia(
            i,
            {
                "v2/test/boot/a.sh": _GIA_CO_SELFTEST,
                "v2/test/boot/selfbreak-gia.sh": _GIA_KHONG_CO,
            },
            ['run_step "a" bash "$V2_ROOT/test/boot/a.sh" --selftest || exit 1'],
        )
        rc_i, out_i = _chay_con(
            i,
            [
                "--standalone-red",
                "--exempt",
                "v2/test/boot/selfbreak-gia.sh=lý do của ca kiểm (i)",
            ],
        )
        ghi(
            "(i) ngoại lệ dùng chung: script độc lập trong ngoại lệ không đỏ dù bật --standalone-red",
            rc_i == 0
            and "E_ORPHAN" not in out_i
            and "v2/test/boot/selfbreak-gia.sh" in out_i
            and "NGOẠI LỆ" in out_i,
            f"exit={rc_i}",
        )

    print("TAP version 13")
    print(f"1..{len(cases)}")
    fail = 0
    for i, (nhan, ok, chi_tiet) in enumerate(cases, 1):
        if ok:
            print(f"ok {i} - {nhan}")
        else:
            print(f"not ok {i} - {nhan} ({chi_tiet})")
            fail += 1
    if fail:
        print(f"# selftest: {fail}/{len(cases)} ca sai kỳ vọng")
        return 1
    print(f"# selftest: {len(cases)}/{len(cases)} ca đúng như kỳ vọng")
    return 0


# ------------------------------------------------------------------ vào việc

def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = argparse.ArgumentParser(
        description="Kiểm máy: mọi script tự kiểm phải có cổng gọi (KT-134)."
    )
    ap.add_argument(
        "--root",
        default=str(Path(__file__).resolve().parents[2]),
        help="gốc repo (mặc định: hai cấp trên tệp này)",
    )
    ap.add_argument(
        "--selftest",
        action="store_true",
        help="dựng repo giả trong thư mục tạm và kiểm chính công cụ (a..i)",
    )
    ap.add_argument(
        "--standalone-red",
        action="store_true",
        help=(
            "coi script tự kiểm **độc lập** (không cờ) không có cổng gọi là ĐỎ; mặc định chỉ "
            "báo cáo (đặc tả của task chỉ định nghĩa ĐỎ cho script có cờ)"
        ),
    )
    ap.add_argument(
        "--exempt",
        action="append",
        default=[],
        metavar="PATH=LÝ_DO",
        help=(
            "thêm một ngoại lệ có lý do (lặp được). **Chỉ dùng cho --selftest/thử nghiệm**: "
            "cổng không truyền cờ này, nên không có đường vòng qua cổng"
        ),
    )
    args = ap.parse_args(argv)

    if args.selftest:
        return chay_selftest()

    root = Path(args.root).resolve()
    extra: List[Tuple[str, str]] = []
    for item in args.exempt:
        duong, _, ly_do = item.partition("=")
        extra.append((duong, ly_do or "(không ghi lý do)"))
    return kiem(root, standalone_red=args.standalone_red, extra_exempt=extra)


if __name__ == "__main__":
    sys.exit(main())
