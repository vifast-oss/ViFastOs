#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""check-boot-abi.py — **KT-404d**: canh khớp bố cục boot ABI kernel ↔ userland.

Vì sao có tệp này
-----------------
`v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs` là **một** nguồn phía kernel cho 17 slot endpoint
(`EP_*`), 8 hằng bố cục (`SLOT_*`/`LOOP_*`) và 2 mảng slot TCB/SchedContext. Chương trình U-mode
**không link** crate kernel nên phải chép tay hằng số; trước KT-404d ràng buộc "khớp bảng" chỉ
được ghi bằng **comment** (`shell.rs:73` "khớp `proc_abi::EP_FS`"…) và `proc_abi.rs:13-15` dẫn
một máy kiểm **không tồn tại** (`JOURNAL/kt77b-harness-scan.sh` không có mục `slot` — phát hiện
của `KT-416b`, `review/agent0/GOI-ABI-KIEN-TRUC.md` §B.1 #8). Lệch một số là `NBSend`/`NBRecv`
tới sai đích hoặc `InsufficientRights` — im lặng cho tới lúc chạy ảnh.

Luật (fail-closed)
------------------
1. Parse `proc_abi.rs`: đủ **17** `EP_*`, **8** `SLOT_*`/`LOOP_*`, **2** mảng (`TCB_SLOTS`,
   `SC_SLOTS`); thiếu/trùng/không parse được dòng `pub const` nào ⇒ **ĐỎ**.
2. Parse **mọi** `.rs` dưới `v2/user/`: gom `const EP_*: u64 = N;`. Dòng bắt đầu bằng
   `const EP_` mà không parse được ⇒ **ĐỎ** (không bỏ qua dòng lạ).
3. Mỗi bản sao **bắt buộc** (bảng `EXPECTED_COPIES`) phải có **đúng một** khai báo trong đúng
   tệp, **cùng tên + cùng giá trị** với kernel; thiếu ⇒ ĐỎ, lệch giá trị ⇒ ĐỎ.
4. Tên `EP_*` phía U-mode **không** có trong `proc_abi.rs` ⇒ ĐỎ (`USER_ONLY_KNOWN` là danh sách
   miễn trừ **có lý do** ghi trong mã, không phải bỏ qua âm thầm).
5. Bất biến bố cục cspace (bắt lỗi "đổi số âm thầm" mà không cần chép lại bảng giá trị):
   mọi slot `EP_*`/`SLOT_CSPACE`/`LOOP_*` phải **không trùng** và `< max_caps = 512`
   (`spec/kernel.toml`); `TCB_SLOTS`/`SC_SLOTS` **cùng độ dài** (§B.2), không trùng nhau và
   không đụng slot endpoint. Slot là **danh từ riêng** của cspace nên trùng số = hỏng bố cục.
6. Không đọc/không sửa `spec/`; không suy ra `BootCapLayout` — việc gộp 17 hằng vào
   `BootCapLayout` là `KT-420`, tệp này chỉ canh **hiện trạng khớp**.

Chạy
----
    python3 v2/tools/check-boot-abi.py [--root REPO_ROOT] [--verbose]
    python3 v2/tools/check-boot-abi.py --selftest     # 13 ca, thuần tệp tạm, không đọc cây thật

Exit: 0 = xanh; 1 = lệch/thiếu/hỏng (fail-closed); 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import re
import sys
import tempfile
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

# ------------------------------------------------------------------ hợp đồng

KERNEL_REL = "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs"
USER_REL = "v2/user"

# `spec/kernel.toml` `[cspace].max_caps` = 512 ⇒ slot hợp lệ 0..511.
MAX_CAPS = 512

# 17 `EP_*` của `proc_abi.rs` (thứ tự đọc trong tệp). Tên **phải** còn đủ: đổi tên ở kernel mà
# quên bản sao U-mode là đúng lớp lỗi cần bắt; đổi tên **có chủ ý** thì sửa cả bảng này.
EP_NAMES: Tuple[str, ...] = (
    "EP_FAULT",
    "EP_PROC",
    "EP_UART",
    "EP_REPLY",
    "EP_PS",
    "EP_CTL",
    "EP_EVENTS",
    "EP_FS",
    "EP_NET",
    "EP_NET_REPLY",
    "EP_NET_KERN",
    "EP_NET_KERN_REPLY",
    "EP_EXEC",
    "EP_MEM",
    "EP_LOOP",
    "EP_NET_CLIENT",
    "EP_NET_CLIENT_REPLY",
)

# 8 hằng `SLOT_*`/`LOOP_*` (5 + 3) — §B.2.2. `SLOT_CHILD` và `SLOT_NETSRV` **cùng số 2** là đúng
# chủ ý (hai ảnh khác nhau: `ktest-proc` vs ảnh sản phẩm), nên chúng nằm ở **không gian ô chương
# trình** (`user::MAX_PROCS`), không thuộc không gian cspace và không tham gia bất biến trùng slot.
SLOT_LOOP_NAMES: Tuple[str, ...] = (
    "SLOT_PROCSRV",
    "SLOT_SHELL",
    "SLOT_CHILD",
    "SLOT_CSPACE",
    "SLOT_NETSRV",
    "LOOP_ARENA",
    "LOOP_TCB_SLOT",
    "LOOP_SC_SLOT",
)

# 2 mảng bố cục (§B.2.2 #19/#20).
ARRAY_NAMES: Tuple[str, ...] = ("TCB_SLOTS", "SC_SLOTS")

# Không gian cspace: 17 EP_* + `SLOT_CSPACE` (cap CNode của chính cspace) + 3 slot `LOOP_*`.
CSPACE_SLOTS: Tuple[str, ...] = EP_NAMES + ("SLOT_CSPACE", "LOOP_ARENA", "LOOP_TCB_SLOT", "LOOP_SC_SLOT")

# 13 tên có bản sao phía U-mode, kèm **đúng** tệp phải chứa nó (đếm được ở base `7f4d217b`:
# 6 + 8 + 2 = 16 khai báo; +1 `EP_ECHO` ngoài phạm vi = 17 `const EP_*`). Bốn hằng **chỉ kernel**
# (không có bản sao): `EP_UART` (đường dây KT-73 đã bỏ, giữ slot), `EP_CTL` (cap `SchedControl`,
# không phải endpoint), `EP_NET_KERN`/`EP_NET_KERN_REPLY` (kernel nhận `Call` từ netsrv; phía
# netsrv dùng **cspace riêng** `SLOT_EP_KERN = 3`, không phải slot của `proc_abi`).
EXPECTED_COPIES: Dict[str, Tuple[str, ...]] = {
    "EP_FAULT": ("programs/procsrv.rs",),
    "EP_PROC": ("programs/procsrv.rs", "programs/shell.rs"),
    "EP_REPLY": ("programs/procsrv.rs", "programs/shell.rs"),
    "EP_EVENTS": ("programs/procsrv.rs", "programs/shell.rs"),
    "EP_NET_CLIENT": ("programs/procsrv.rs",),
    "EP_NET_CLIENT_REPLY": ("programs/procsrv.rs",),
    "EP_PS": ("programs/shell.rs",),
    "EP_FS": ("programs/shell.rs",),
    "EP_NET": ("programs/shell.rs",),
    "EP_NET_REPLY": ("programs/shell.rs",),
    "EP_EXEC": ("programs/shell.rs",),
    "EP_MEM": ("programs/shell_cmds.rs",),
    "EP_LOOP": ("programs/shell_cmds.rs",),
}

# `EP_*` phía U-mode **không** thuộc `proc_abi.rs` — miễn trừ **có lý do**, in ra khi thấy:
# `echo.rs` là chương trình của **ảnh ktest aarch64** (`k5user::SLOT_EP_ECHO = 9`,
# `v2/kernel/vifast-kbin-aarch64/src/k5user.rs:58`), không phải cspace của ảnh sản phẩm riscv64 do
# `proc_abi.rs` dựng. Tên/giá trị của nó do `k5user.rs` canh, **ngoài** phạm vi tệp này.
USER_ONLY_KNOWN: Dict[str, str] = {
    "EP_ECHO": "cspace ảnh ktest aarch64 (`k5user::SLOT_EP_ECHO`), không thuộc proc_abi riscv64",
}

RE_KERNEL_SCALAR = re.compile(
    r"^\s*pub const\s+([A-Z][A-Z0-9_]*)\s*:\s*(?:usize|u64|u32|u16|u8)\s*"
    r"=\s*(0[xX][0-9a-fA-F_]+|\d[\d_]*)\s*;\s*$"
)
RE_KERNEL_ARRAY = re.compile(
    r"^\s*pub const\s+([A-Z][A-Z0-9_]*)\s*:\s*\[\s*(?:usize|u64|u32|u16|u8)\s*;\s*(\d+)\s*\]\s*"
    r"=\s*\[([^\]]*)\]\s*;\s*$"
)
RE_USER_SCALAR = re.compile(
    r"^\s*const\s+(EP_[A-Z0-9_]+)\s*:\s*(?:usize|u64|u32)\s*=\s*"
    r"(0[xX][0-9a-fA-F_]+|\d[\d_]*)\s*;\s*$"
)


class AbiError(Exception):
    """Parse/lệch/thiếu — luôn là ĐỎ (fail-closed), không phải 'bỏ qua'."""


def _parse_int(token: str) -> int:
    return int(token.replace("_", ""), 0)


def _iter_lines(path: Path) -> List[Tuple[int, str]]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise AbiError("không đọc được %s: %s" % (path, exc)) from exc
    out = []
    for number, raw in enumerate(text.splitlines(), 1):
        stripped = raw.strip()
        if stripped.startswith("//"):
            continue
        out.append((number, raw))
    return out


# ------------------------------------------------------------------ parse

class KernelAbi:
    def __init__(self) -> None:
        self.scalars: Dict[str, int] = {}
        self.arrays: Dict[str, List[int]] = {}
        self.dup: List[str] = []
        self.bad_lines: List[str] = []

    def value(self, name: str) -> int:
        return self.scalars[name]


def parse_kernel(path: Path) -> KernelAbi:
    abi = KernelAbi()
    for number, raw in _iter_lines(path):
        line = raw.strip()
        if not line.startswith("pub const "):
            continue
        match = RE_KERNEL_ARRAY.match(raw)
        if match:
            name, declared, body = match.group(1), int(match.group(2)), match.group(3)
            try:
                values = [_parse_int(part) for part in body.split(",") if part.strip()]
            except ValueError:
                abi.bad_lines.append("%d: %s" % (number, line))
                continue
            if name in abi.arrays or name in abi.scalars:
                abi.dup.append(name)
                continue
            if len(values) != declared:
                raise AbiError(
                    "%s:%d: `%s` khai báo [..; %d] nhưng có %d phần tử"
                    % (path, number, name, declared, len(values))
                )
            abi.arrays[name] = values
            continue
        match = RE_KERNEL_SCALAR.match(raw)
        if not match:
            abi.bad_lines.append("%d: %s" % (number, line))
            continue
        name, value = match.group(1), _parse_int(match.group(2))
        if name in abi.scalars or name in abi.arrays:
            abi.dup.append(name)
            continue
        abi.scalars[name] = value
    return abi


class UserCopy:
    def __init__(self, rel: str, name: str, value: int, line: int) -> None:
        self.rel = rel
        self.name = name
        self.value = value
        self.line = line


def parse_user(root: Path) -> List[UserCopy]:
    user_dir = root / USER_REL
    if not user_dir.is_dir():
        raise AbiError("không thấy thư mục %s" % user_dir)
    copies: List[UserCopy] = []
    seen: Dict[Tuple[str, str], int] = {}
    files = sorted(
        path for path in user_dir.rglob("*.rs") if "target" not in path.parts
    )
    if not files:
        raise AbiError("không có tệp .rs nào dưới %s" % user_dir)
    for path in files:
        rel = path.relative_to(user_dir).as_posix()
        for number, raw in _iter_lines(path):
            line = raw.strip()
            if not line.startswith("const EP_"):
                continue
            match = RE_USER_SCALAR.match(raw)
            if not match:
                raise AbiError(
                    "%s:%d: dòng `const EP_` không parse được: %s" % (path, number, line)
                )
            name, value = match.group(1), _parse_int(match.group(2))
            key = (rel, name)
            if key in seen:
                raise AbiError(
                    "%s: `%s` khai báo hai lần (dòng %d và %d)"
                    % (path, name, seen[key], number)
                )
            seen[key] = number
            copies.append(UserCopy(rel, name, value, number))
    return copies


# ------------------------------------------------------------------ kiểm

def run_check(root: Path, verbose: bool = False) -> int:
    kernel_path = root / KERNEL_REL
    if not kernel_path.is_file():
        raise AbiError("không thấy %s" % kernel_path)
    abi = parse_kernel(kernel_path)
    if abi.bad_lines:
        raise AbiError(
            "không parse được %d dòng `pub const` trong %s: %s"
            % (len(abi.bad_lines), kernel_path, "; ".join(abi.bad_lines[:3]))
        )
    if abi.dup:
        raise AbiError("hằng khai báo trùng trong %s: %s" % (kernel_path, ", ".join(abi.dup)))

    # 1) đủ tên kernel (fail-closed: thiếu một hằng = bố cục đã đổi mà chưa cập nhật máy kiểm).
    missing = [name for name in EP_NAMES + SLOT_LOOP_NAMES if name not in abi.scalars]
    if missing:
        raise AbiError("thiếu hằng bố cục trong %s: %s" % (kernel_path, ", ".join(missing)))
    missing = [name for name in ARRAY_NAMES if name not in abi.arrays]
    if missing:
        raise AbiError("thiếu mảng bố cục trong %s: %s" % (kernel_path, ", ".join(missing)))

    # 2) bất biến cspace: không trùng slot, trong 0..max_caps-1.
    slot_owner: Dict[int, str] = {}
    for name in CSPACE_SLOTS:
        value = abi.scalars[name]
        if not 0 <= value < MAX_CAPS:
            raise AbiError(
                "slot `%s` = %d ngoài 0..%d (max_caps)" % (name, value, MAX_CAPS - 1)
            )
        if value in slot_owner:
            raise AbiError(
                "slot trùng: `%s` và `%s` cùng = %d" % (slot_owner[value], name, value)
            )
        slot_owner[value] = name
    tcb_slots, sc_slots = abi.arrays["TCB_SLOTS"], abi.arrays["SC_SLOTS"]
    if len(tcb_slots) != len(sc_slots):
        raise AbiError(
            "`TCB_SLOTS` %d phần tử ≠ `SC_SLOTS` %d phần tử (phải cùng độ dài, §B.2)"
            % (len(tcb_slots), len(sc_slots))
        )
    seen_array: Dict[int, str] = {}
    for name, values in (("TCB_SLOTS", tcb_slots), ("SC_SLOTS", sc_slots)):
        for value in values:
            if not 0 <= value < MAX_CAPS:
                raise AbiError("`%s` có slot %d ngoài 0..%d" % (name, value, MAX_CAPS - 1))
            if value in slot_owner:
                raise AbiError(
                    "`%s` slot %d trùng slot của `%s`" % (name, value, slot_owner[value])
                )
            if value in seen_array:
                raise AbiError(
                    "`%s` slot %d trùng với `%s`" % (name, value, seen_array[value])
                )
            seen_array[value] = name

    # 3) bản sao U-mode: đúng tệp, đúng tên, đúng giá trị, đúng một lần.
    copies = parse_user(root)
    by_key: Dict[Tuple[str, str], UserCopy] = {(c.rel, c.name): c for c in copies}
    values: Dict[str, int] = {}
    exempt: Dict[str, int] = {}
    for copy in copies:
        if copy.name in USER_ONLY_KNOWN:
            exempt[copy.name] = copy.value
            continue
        if copy.name not in abi.scalars:
            raise AbiError(
                "%s:%d: `%s` không có trong %s (lệch tên? miễn trừ có lý do ở USER_ONLY_KNOWN)"
                % (root / USER_REL / copy.rel, copy.line, copy.name, KERNEL_REL)
            )
        if copy.name in values and values[copy.name] != copy.value:
            raise AbiError(
                "`%s` lệch giữa các bản sao: %s:%d = %d, %s = %d"
                % (copy.name, copy.rel, copy.line, copy.value, "bản khác", values[copy.name])
            )
        values[copy.name] = copy.value

    compared = 0
    for name, files in EXPECTED_COPIES.items():
        want = abi.scalars[name]
        for rel in files:
            copy = by_key.get((rel, name))
            if copy is None:
                raise AbiError(
                    "thiếu bản sao `%s` trong %s/%s (kernel = %d)"
                    % (name, USER_REL, rel, want)
                )
            if copy.value != want:
                raise AbiError(
                    "lệch giá trị `%s`: kernel %s = %d, %s/%s:%d = %d"
                    % (name, KERNEL_REL, want, USER_REL, rel, copy.line, copy.value)
                )
            compared += 1
    for name in sorted(exempt):
        print(
            "check-boot-abi: miễn trừ `%s` — %s (giá trị %d không đối chiếu)"
            % (name, USER_ONLY_KNOWN[name], exempt[name])
        )

    # 4) báo cáo.
    if verbose:
        for name in EP_NAMES:
            owners = [rel for rel in EXPECTED_COPIES.get(name, ()) if (rel, name) in by_key]
            print(
                "  %-24s = %-2d  kernel;  bản sao: %s"
                % (name, abi.scalars[name], ", ".join(owners) if owners else "(chỉ kernel)")
            )
        for name in SLOT_LOOP_NAMES:
            print("  %-24s = %-2d  (kernel-only)" % (name, abi.scalars[name]))
        for name in ARRAY_NAMES:
            print("  %-24s = %s" % (name, abi.arrays[name]))
    n_user = len(copies)
    n_files = len({copy.rel for copy in copies})
    print(
        "check-boot-abi: kernel 17 EP_* + 8 SLOT_*/LOOP_* + 2 mảng; userland %d const EP_* "
        "trong %d tệp; %d bản sao khớp tên+giá trị; 0 lệch"
        % (n_user, n_files, compared)
    )
    return 0


# ------------------------------------------------------------------ tự kiểm

# Bảng giá trị dựng fixture. Cố ý **chép** giá trị thật để ca "đỏ" chứng minh máy kiểm so **giá
# trị**, không chỉ so tên; ca xanh dùng đúng bảng này.
_K_EP = {
    "EP_FAULT": 1,
    "EP_PROC": 2,
    "EP_UART": 3,
    "EP_REPLY": 4,
    "EP_PS": 5,
    "EP_CTL": 6,
    "EP_EVENTS": 8,
    "EP_FS": 7,
    "EP_NET": 9,
    "EP_NET_REPLY": 15,
    "EP_NET_KERN": 16,
    "EP_NET_KERN_REPLY": 17,
    "EP_EXEC": 18,
    "EP_MEM": 21,
    "EP_LOOP": 22,
    "EP_NET_CLIENT": 26,
    "EP_NET_CLIENT_REPLY": 27,
}
_K_SLOT = {
    "SLOT_PROCSRV": 0,
    "SLOT_SHELL": 1,
    "SLOT_CHILD": 2,
    "SLOT_CSPACE": 20,
    "SLOT_NETSRV": 2,
    "LOOP_ARENA": 23,
    "LOOP_TCB_SLOT": 24,
    "LOOP_SC_SLOT": 25,
}
_K_ARRAY = {"TCB_SLOTS": [10, 11, 12, 13, 14], "SC_SLOTS": [30, 31, 32, 33, 34]}


def _kernel_text(ep: Dict[str, int], slot: Dict[str, int], arrays: Dict[str, List[int]]) -> str:
    lines = ["// SPDX-License-Identifier: MIT OR Apache-2.0", "// fixture KT-404d", ""]
    for name in sorted(ep):
        lines.append("pub const %s: usize = %d;" % (name, ep[name]))
    for name in sorted(slot):
        lines.append("pub const %s: usize = %d;" % (name, slot[name]))
    for name in ("TCB_SLOTS", "SC_SLOTS"):
        values = arrays[name]
        lines.append(
            "pub const %s: [usize; %d] = [%s];"
            % (name, len(values), ", ".join(str(v) for v in values))
        )
    return "\n".join(lines) + "\n"


def _user_file(copies: Sequence[Tuple[str, int]]) -> str:
    lines = ["// SPDX-License-Identifier: MIT OR Apache-2.0", "// fixture KT-404d"]
    for name, value in copies:
        lines.append("const %s: u64 = %d;" % (name, value))
    return "\n".join(lines) + "\n"


def _base_user() -> Dict[str, List[Tuple[str, int]]]:
    # 17 khai báo như cây thật (16 bản sao thuộc hợp đồng + `EP_ECHO` ngoài phạm vi).
    return {
        "programs/procsrv.rs": [
            ("EP_FAULT", 1),
            ("EP_PROC", 2),
            ("EP_REPLY", 4),
            ("EP_EVENTS", 8),
            ("EP_NET_CLIENT", 26),
            ("EP_NET_CLIENT_REPLY", 27),
        ],
        "programs/shell.rs": [
            ("EP_PROC", 2),
            ("EP_REPLY", 4),
            ("EP_PS", 5),
            ("EP_EVENTS", 8),
            ("EP_FS", 7),
            ("EP_NET", 9),
            ("EP_NET_REPLY", 15),
            ("EP_EXEC", 18),
        ],
        "programs/shell_cmds.rs": [("EP_MEM", 21), ("EP_LOOP", 22)],
        "programs/echo.rs": [("EP_ECHO", 9)],
    }


def _make_tree(root: Path) -> None:
    kernel = root / KERNEL_REL
    kernel.parent.mkdir(parents=True, exist_ok=True)
    kernel.write_text(_kernel_text(_K_EP, _K_SLOT, _K_ARRAY), encoding="utf-8")
    for rel, copies in _base_user().items():
        path = root / USER_REL / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(_user_file(copies), encoding="utf-8")


def _edit(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if old not in text:
        raise AssertionError("fixture: không thấy %r trong %s" % (old, path))
    path.write_text(text.replace(old, new), encoding="utf-8")


def _case(tmp: Path, name: str, mutate, expected: int) -> Tuple[bool, str]:
    root = tmp / name
    _make_tree(root)
    mutate(root)
    out, err = io.StringIO(), io.StringIO()
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = run_check(root)
    except AbiError as exc:
        rc = 1
        err.write(str(exc))
    ok = rc == expected
    detail = "rc=%d (kỳ vọng %d)" % (rc, expected)
    if ok and expected == 1:
        message = err.getvalue().strip().splitlines()
        detail += " | %s" % (message[-1] if message else "")
    elif not ok:
        detail += " | stdout=%r stderr=%r" % (out.getvalue().strip(), err.getvalue().strip())
    return ok, detail


def run_selftest() -> int:
    cases = oks = 0

    def record(name: str, result: Tuple[bool, str]) -> None:
        nonlocal cases, oks
        cases += 1
        ok, detail = result
        if ok:
            oks += 1
            print("ok %d - %s: %s" % (cases, name, detail))
        else:
            print("not ok %d - %s: %s" % (cases, name, detail))

    def keep(_root: Path) -> None:
        return None

    with tempfile.TemporaryDirectory(prefix="kt404d-abi-") as td:
        tmp = Path(td)

        record("cây khớp (16 bản sao + EP_ECHO ngoài phạm vi) ⇒ XANH", _case(tmp, "green", keep, 0))

        def value_drift(root: Path) -> None:
            _edit(root / USER_REL / "programs/procsrv.rs", "const EP_FAULT: u64 = 1;",
                  "const EP_FAULT: u64 = 2;")

        record("lệch giá trị (EP_FAULT 1→2) ⇒ ĐỎ", _case(tmp, "value-drift", value_drift, 1))

        def value_drift_b(root: Path) -> None:
            _edit(root / USER_REL / "programs/shell.rs", "const EP_FS: u64 = 7;",
                  "const EP_FS: u64 = 8;")

        record("lệch giá trị (EP_FS 7→8) ⇒ ĐỎ", _case(tmp, "value-drift-b", value_drift_b, 1))

        def rename(root: Path) -> None:
            _edit(root / USER_REL / "programs/shell_cmds.rs", "const EP_MEM: u64 = 21;",
                  "const EP_MEMORY: u64 = 21;")

        record("lệch tên (EP_MEM→EP_MEMORY) ⇒ ĐỎ", _case(tmp, "rename", rename, 1))

        def missing_copy(root: Path) -> None:
            _edit(root / USER_REL / "programs/procsrv.rs",
                  "const EP_NET_CLIENT: u64 = 26;\n", "")

        record("thiếu bản sao (EP_NET_CLIENT) ⇒ ĐỎ", _case(tmp, "missing-copy", missing_copy, 1))

        def missing_kernel(root: Path) -> None:
            _edit(root / KERNEL_REL, "pub const EP_UART: usize = 3;\n", "")

        record("kernel thiếu hằng (EP_UART) ⇒ ĐỎ", _case(tmp, "missing-kernel", missing_kernel, 1))

        def dup_kernel(root: Path) -> None:
            path = root / KERNEL_REL
            path.write_text(
                path.read_text(encoding="utf-8") + "pub const EP_PS: usize = 5;\n",
                encoding="utf-8",
            )

        record("kernel khai báo trùng ⇒ ĐỎ", _case(tmp, "dup-kernel", dup_kernel, 1))

        def parse_kernel(root: Path) -> None:
            _edit(root / KERNEL_REL, "pub const EP_PS: usize = 5;",
                  "pub const EP_PS: usize = ;")

        record("kernel parse fail ⇒ ĐỎ (fail-closed)", _case(tmp, "parse-kernel", parse_kernel, 1))

        def parse_user(root: Path) -> None:
            _edit(root / USER_REL / "programs/shell.rs", "const EP_PS: u64 = 5;",
                  "const EP_PS: u64 = ;")

        record("userland parse fail ⇒ ĐỎ (fail-closed)", _case(tmp, "parse-user", parse_user, 1))

        def unknown_name(root: Path) -> None:
            path = root / USER_REL / "programs/shell.rs"
            path.write_text(
                path.read_text(encoding="utf-8") + "const EP_BOGUS: u64 = 99;\n",
                encoding="utf-8",
            )

        record("bản sao tên lạ (EP_BOGUS) ⇒ ĐỎ", _case(tmp, "unknown-name", unknown_name, 1))

        def slot_collision(root: Path) -> None:
            _edit(root / KERNEL_REL, "pub const LOOP_ARENA: usize = 23;",
                  "pub const LOOP_ARENA: usize = 21;")

        record("slot trùng (LOOP_ARENA=21↔EP_MEM) ⇒ ĐỎ", _case(tmp, "slot-collision", slot_collision, 1))

        def array_len(root: Path) -> None:
            _edit(root / KERNEL_REL,
                  "pub const SC_SLOTS: [usize; 5] = [30, 31, 32, 33, 34];",
                  "pub const SC_SLOTS: [usize; 4] = [30, 31, 32, 33];")

        record("2 mảng khác độ dài ⇒ ĐỎ", _case(tmp, "array-len", array_len, 1))

        def missing_file(root: Path) -> None:
            (root / USER_REL / "programs/shell_cmds.rs").unlink()

        record("thiếu hẳn tệp bản sao ⇒ ĐỎ", _case(tmp, "missing-file", missing_file, 1))

    print("check-boot-abi: selftest %d/%d ok" % (oks, cases))
    return 0 if oks == cases else 1


# ------------------------------------------------------------------ main

def main(argv: List[str]) -> int:
    ap = argparse.ArgumentParser(
        description="KT-404d — khớp bố cục boot ABI kernel (`proc_abi.rs`) ↔ userland (`v2/user/`)"
    )
    ap.add_argument(
        "--root",
        default=str(Path(__file__).resolve().parents[2]),
        help="gốc repo (mặc định: hai cấp trên tệp này)",
    )
    ap.add_argument("--selftest", action="store_true", help="chạy 13 ca tự kiểm, không đọc cây thật")
    ap.add_argument("--verbose", action="store_true", help="in bảng 17 EP_* + 8 slot + 2 mảng")
    args = ap.parse_args(argv)

    if args.selftest:
        return run_selftest()

    root = Path(args.root).resolve()
    if not root.is_dir():
        print("check-boot-abi: không thấy %s" % root, file=sys.stderr)
        return 2
    try:
        return run_check(root, args.verbose)
    except AbiError as exc:
        print("check-boot-abi: ĐỎ — %s" % exc, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
