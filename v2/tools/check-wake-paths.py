#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-151 (N7, D2-131) — kiểm **máy** "một cơ chế đánh thức TCB" trên mã nguồn.

Vì sao có tệp này
-----------------
N7 (Audit Q1-A) là **nợ cấu trúc** của v1: ba port có **ba** kênh đánh thức khác nhau
(riscv64 doorbell riêng cho reschedule; arm/x86 dùng mailbox chung). v2 tuyên bố đã thống
nhất qua hộp thư theo hart, nhưng **chưa có máy kiểm** — một bảng viết tay không chống được
tái diễn. Tệp này là máy kiểm đó: nó **quét** cây nguồn, **liệt kê** mọi đường đánh thức TCB
kèm `file:dòng`, và **đỏ** khi có kênh thứ hai.

Phạm vi (định nghĩa lại so với câu chữ D2-131 — ghi ở `v2/agent/JOURNAL/kt151-n7.md`)
------------------------------------------------------------------------------------
D2-131 viết "đòi **tất cả** đi qua `vifast-sync::Mailbox`". Trên mã v2 hiện tại, **mọi**
`transition_tcb(…, Ready)` không đi qua hộp thư: các đường IPC/Reply/Notification/finalise
đánh thức **trực tiếp** TCB trên hart hiện tại (mô hình một luồng, `current_hart` toàn cục).
Đó là **đường cùng hart**, không phải **kênh chéo hart** — N7 nói về kênh chéo hart (v1:
doorbell riêng theo arch). Vì vậy máy kiểm có **hai** mức:

- **Mức mặc định (cổng)**: kênh **chéo hart** phải là hộp thư duy nhất; không có doorbell
  riêng theo arch; ba loại `Wake`/`Reschedule`/`Migrate` có mặt và được tiêu thụ đúng một
  chỗ. Mức này **xanh** trên v2 và **đỏ** trên v1 (ba kênh).
- **`--strict`**: đòi **mọi** chuyển `blocked/sleeping → Ready` nằm trong **một** hàm
  `wake_step`. Mức này **đỏ** trên v2 (nợ N7-2: các đường IPC/finalise đánh thức trực tiếp)
  — chạy để **đo nợ**, không nằm trong cổng.

Fail-closed: thiếu mỏ neo (không tìm thấy `wake_step`/`apply_mail`/`MailboxMsg`/`Msg`) ⇒ ĐỎ,
không được coi là "không có gì để kiểm".

Dùng
----
    python3 v2/tools/check-wake-paths.py [--root R] [--strict] [--json]
    python3 v2/tools/check-wake-paths.py --selftest

Exit: 0 = xanh; 1 = đỏ; 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

# ------------------------------------------------------------------ hằng số

# Ba loại message đánh thức TCB (D2-131). `Stop`/`TlbFlush` không đánh thức TCB.
WAKE_KINDS = ("Wake", "Reschedule", "Migrate")

# Tên hàm chuyển trạng thái duy nhất (mức `--strict`).
WAKE_STEP = "wake_step"

# Hàm tiêu thụ message trong hộp thư (kernel + sim).
MAIL_CONSUMER = "apply_mail"

# Hàm phát message `Wake` (đường chéo hart).
#
# **KT-154 (N7-2)** — mở rộng **có kiểm soát**: `wake` là API công khai (đường hộp thư cũ);
# `wake_from` là **cửa duy nhất** cho mọi đường sản phẩm (IPC/Reply/Notification/finalise/
# refill) — nó cũng phát `Wake` về `home` khi khác hart. Cả hai **cùng một kênh**; không
# thêm hàm thứ ba nào được phát `Wake`. Định nghĩa lại phạm vi này đã quét lại **toàn bộ**
# `v2/kernel` + `v2/sim` (Luật 2) — xem `v2/agent/JOURNAL/kt154-n7fix.md` §8.
WAKE_PRODUCER = ("wake", "wake_from")

# Hàm phát `Migrate` (push migration).
MIGRATE_PRODUCER = "push_step"

# Tệp định nghĩa kiểu hộp thư của từng tầng.
KERNEL_MAILBOX = "v2/kernel/vifast-core/src/mailbox.rs"
SIM_MAILBOX = "v2/sim/vifast-sim/src/mailbox.rs"
SYNC_MAILBOX = "v2/lib/vifast-sync/src/mailbox.rs"
SYNC_TEST = "v2/lib/vifast-sync/tests/kt30_sync.rs"
KERNEL_SCHED = "v2/kernel/vifast-core/src/core_sched.rs"
SIM_KOBJ = "v2/sim/vifast-sim/src/kobj.rs"

# Thư mục quét.
V2_SCAN = ("v2/kernel", "v2/sim")
V1_PLATFORM_GLOB = "crates/vifast-platform-*/src/platform.rs"
V1_KERNEL_GLOB = "crates/vifast-kernel/src/*.rs"

# Mẫu nhận diện.
RE_FN = re.compile(
    r"^(?P<indent>\s*)(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?"
    r"(?:extern\s+\"[^\"]*\"\s+)?fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)
RE_READY_TRANSITION = re.compile(r"transition_tcb\s*\([^;]*?TcbState::Ready")
RE_POST_MAIL = re.compile(r"\.post_mail\s*\(")
RE_TRY_PUSH = re.compile(r"\.try_push\s*\(")
RE_MAILBOX_MSG = re.compile(r"MailboxMsg::(Wake|Reschedule|Migrate|Stop|TlbFlush)\b")
RE_MSG = re.compile(r"\bMsg::(Wake|Reschedule|Migrate|Stop|VmKick|PowerReq)\b")
RE_IPI_WAKE_POST = re.compile(r"\bIpiKind::Wake\b")
RE_IPI_POST = re.compile(r"\b(?:post|unpost)\s*\(")
RE_DOORBELL = re.compile(r"\bdoorbell\b", re.IGNORECASE)
RE_ARCH_FORBIDDEN = re.compile(
    r"\b(?:TcbState|transition_tcb|MailboxMsg|wake_step|MailboxMsg::Wake)\b"
)


# ------------------------------------------------------------------ đọc/parse Rust

class Fn:
    """Một hàm: tên + khoảng dòng (1-based, bao gồm cả dòng `fn` và dòng `}`)."""

    __slots__ = ("name", "start", "end", "indent", "path")

    def __init__(self, name: str, start: int, end: int, indent: int, path: str) -> None:
        self.name = name
        self.start = start
        self.end = end
        self.indent = indent
        self.path = path

    def __repr__(self) -> str:  # pragma: no cover - chẩn đoán
        return f"Fn({self.name}@{self.path}:{self.start}-{self.end})"


def find_functions(path: Path, text: str) -> List[Fn]:
    """Tìm hàm theo **thụt lề** (mã đã qua rustfmt): dòng `}` cùng mức thụt lề đóng hàm.

    Đủ cho mục đích của tệp này (xác định hàm bao quanh một dòng); không phải parser Rust
    đầy đủ. Hàm không có thân (`;` trong trait) bị bỏ.
    """
    lines = text.splitlines()
    out: List[Fn] = []
    i = 0
    n = len(lines)
    while i < n:
        m = RE_FN.match(lines[i])
        if not m:
            i += 1
            continue
        indent = len(m.group("indent").expandtabs(4))
        name = m.group("name")
        # Tìm dấu `{` mở thân (cùng dòng hoặc vài dòng sau), dừng ở `;` (trait decl).
        j = i
        opened = False
        while j < n and j - i <= 8:
            if "{" in lines[j]:
                opened = True
                break
            if ";" in lines[j]:
                break
            j += 1
        if not opened:
            i += 1
            continue
        # Tìm dòng đóng: `}` có cùng thụt lề (bỏ qua dòng trống/chú thích).
        k = j
        end = None
        while k < n:
            stripped = lines[k].strip()
            if stripped.startswith("}"):
                cur_indent = len(lines[k]) - len(lines[k].lstrip())
                if cur_indent == indent:
                    end = k
                    break
            k += 1
        if end is None:
            # Không đóng được ⇒ fail-closed ở tầng gọi (đánh dấu tới hết tệp).
            end = n - 1
        out.append(Fn(name, i + 1, end + 1, indent, str(path)))
        i = end + 1
    return out



def test_line_ranges(text: str) -> List[Tuple[int, int]]:
    """Khoảng dòng của các khối `#[cfg(test)] mod … { … }` (1-based, bao gồm)."""
    lines = text.splitlines()
    ranges: List[Tuple[int, int]] = []
    i = 0
    n = len(lines)
    while i < n:
        if not re.match(r"^\s*#\[cfg\(test\)\]", lines[i]):
            i += 1
            continue
        # Tìm `mod … {` trong vài dòng kế.
        j = i + 1
        mod_line = None
        while j < n and j - i <= 4:
            if re.match(r"^\s*(?:pub\s+)?mod\s+\w+\s*\{", lines[j]):
                mod_line = j
                break
            if "{" in lines[j] and "mod" in lines[j]:
                mod_line = j
                break
            j += 1
        if mod_line is None:
            i += 1
            continue
        indent = len(lines[mod_line]) - len(lines[mod_line].lstrip())
        depth = 0
        end = n - 1
        for k in range(mod_line, n):
            depth += lines[k].count("{") - lines[k].count("}")
            if depth <= 0 and k > mod_line:
                end = k
                break
        ranges.append((i + 1, end + 1))
        i = end + 1
    return ranges


def is_test_file(path: Path, root: Path) -> bool:
    relp = rel(root, path)
    name = path.name
    return (
        "/tests/" in relp
        or name.endswith("_tests.rs")
        or name.startswith("ktest")
        or name == "tls_tests.rs"
    )


def in_ranges(line: int, ranges: Sequence[Tuple[int, int]]) -> bool:
    return any(a <= line <= b for a, b in ranges)

def enclosing(fns: Sequence[Fn], line: int) -> Optional[Fn]:
    """Hàm bao quanh dòng `line` (1-based); chọn khoảng nhỏ nhất."""
    best: Optional[Fn] = None
    for fn in fns:
        if fn.start <= line <= fn.end:
            if best is None or (fn.end - fn.start) < (best.end - best.start):
                best = fn
    return best


def iter_rs(root: Path, rel_dirs: Iterable[str]) -> Iterable[Path]:
    for rel in rel_dirs:
        base = root / rel
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*.rs")):
            if "target" in p.parts:
                continue
            yield p


def rel(root: Path, p: Path) -> str:
    try:
        return str(p.relative_to(root))
    except ValueError:
        return str(p)


# ------------------------------------------------------------------ kết quả

class Report:
    def __init__(self) -> None:
        self.errors: List[str] = []
        self.warnings: List[str] = []
        self.info: List[str] = []

    def err(self, code: str, msg: str) -> None:
        self.errors.append(f"{code} {msg}")

    def warn(self, code: str, msg: str) -> None:
        self.warnings.append(f"{code} {msg}")

    def note(self, msg: str) -> None:
        self.info.append(msg)

    @property
    def ok(self) -> bool:
        return not self.errors


# ------------------------------------------------------------------ v2

def scan_v2(root: Path, strict: bool) -> Report:
    r = Report()

    # --- mỏ neo (fail-closed) -------------------------------------------------
    anchors = {
        KERNEL_MAILBOX: ("enum MailboxMsg", WAKE_KINDS),
        SIM_MAILBOX: ("enum MailboxMsg", WAKE_KINDS),
        SYNC_MAILBOX: ("enum Msg", WAKE_KINDS),
        KERNEL_SCHED: ("fn wake(", ("fn wake_step(",)),
        SIM_KOBJ: ("fn wake(", ("fn wake_step(",)),
    }
    for relpath, (needle, names) in anchors.items():
        p = root / relpath
        if not p.is_file():
            r.err("E_ANCHOR_MISSING", f"{relpath}: không có tệp mỏ neo")
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        if needle not in text:
            r.err("E_ANCHOR_MISSING", f"{relpath}: thiếu `{needle}`")
        for name in names:
            if name not in text:
                r.err("E_ANCHOR_MISSING", f"{relpath}: thiếu `{name}`")
    if not (root / SYNC_TEST).is_file():
        r.err("E_ANCHOR_MISSING", f"{SYNC_TEST}: thiếu test của kiểu thiết kế")
    else:
        t = (root / SYNC_TEST).read_text(encoding="utf-8", errors="replace")
        for kind in WAKE_KINDS:
            if f"Msg::{kind}" not in t:
                r.err("E_ANCHOR_MISSING", f"{SYNC_TEST}: không dùng `Msg::{kind}`")

    # --- quét mọi tệp nguồn ---------------------------------------------------
    files: List[Tuple[Path, str, List[Fn]]] = []
    test_ranges: Dict[str, List[Tuple[int, int]]] = {}
    for p in iter_rs(root, V2_SCAN):
        text = p.read_text(encoding="utf-8", errors="replace")
        files.append((p, text, find_functions(p, text)))
        test_ranges[str(p)] = test_line_ranges(text)

    # 1) Kênh chéo hart: chỉ `post_mail`/`try_push` mang message hộp thư.
    producers: Dict[str, List[str]] = {k: [] for k in WAKE_KINDS}
    for p, text, fns in files:
        if is_test_file(p, root) or p.name == "mailbox.rs":
            continue
        tr = test_ranges.get(str(p), [])
        lines = text.splitlines()
        for lineno, line in enumerate(lines, 1):
            if in_ranges(lineno, tr):
                continue
            stripped = line.lstrip()
            if stripped.startswith("//") or stripped.startswith("*") or stripped.startswith("/*"):
                continue
            m = RE_MAILBOX_MSG.search(line)
            if not m:
                continue
            kind = m.group(1)
            if kind not in producers:
                continue
            # Bỏ **pattern** (match arm / `if let` / `matches!`); chỉ giữ **constructor**.
            if "=>" in line or "if let" in line or "matches!" in line:
                continue
            if "MailboxMsg::" in line and "{" in line:
                # Struct constructor: phải có `= MailboxMsg::` (let) hoặc là đối số của
                # `post_mail`/`try_push`; pattern `{ tcb, to_hart }` không có `:` giá trị.
                if "= MailboxMsg::" not in line and "post_mail" not in line and "try_push" not in line:
                    continue
            fn = enclosing(fns, lineno)
            fname = fn.name if fn else "<ngoài hàm>"
            producers[kind].append(f"{rel(root, p)}:{lineno} ({fname})")

    # `Wake` và `Migrate` **phải** có chỗ phát ở **cả hai** tầng (kernel + sim); `Reschedule`
    # là kênh có sẵn (hiện chưa có producer trong mã sản phẩm) nên chỉ đòi có nhánh tiêu thụ.
    for kind in ("Wake", "Migrate"):
        if not producers[kind]:
            r.err(
                "E_CHANNEL_MISSING",
                f"không có chỗ phát `MailboxMsg::{kind}` — kênh chéo hart không đủ",
            )
        for layer, prefix in (("kernel", "v2/kernel/"), ("sim", "v2/sim/")):
            if not any(site.startswith(prefix) for site in producers[kind]):
                r.err(
                    "E_CHANNEL_MISSING",
                    f"{layer}: không có chỗ phát `MailboxMsg::{kind}` — tầng này mất kênh",
                )
    # `Wake` chỉ được phát từ `wake`/`wake_from` (một kênh, hai cửa cùng kênh — KT-154);
    # `Migrate` chỉ từ `push_step` (đường push migration).
    for kind, allowed in (("Wake", WAKE_PRODUCER), ("Migrate", MIGRATE_PRODUCER)):
        allowed_set = (allowed,) if isinstance(allowed, str) else tuple(allowed)
        for site in producers[kind]:
            fname = site.rsplit("(", 1)[-1].rstrip(")")
            if fname not in allowed_set:
                r.err(
                    "E_CHANNEL_PRODUCER",
                    f"`MailboxMsg::{kind}` phát từ `{fname}` ({site}) — chỉ "
                    f"{sorted(allowed_set)} được phát",
                )
    for kind in WAKE_KINDS:
        for site in producers[kind]:
            r.note(f"kênh {kind}: {site}")

    # 2) Người tiêu thụ: `Wake`/`Reschedule`/`Migrate` chỉ trong `apply_mail`.
    consumers: Dict[str, List[str]] = {k: [] for k in WAKE_KINDS}
    for p, text, fns in files:
        if p.name == "mailbox.rs" or is_test_file(p, root):
            continue
        tr = test_ranges.get(str(p), [])
        for lineno, line in enumerate(text.splitlines(), 1):
            if in_ranges(lineno, tr):
                continue
            if "=>" not in line:
                continue
            kinds = RE_MAILBOX_MSG.findall(line)
            if not kinds:
                continue
            fn = enclosing(fns, lineno)
            fname = fn.name if fn else "<ngoài hàm>"
            for kind in kinds:
                if kind in consumers:
                    consumers[kind].append(f"{rel(root, p)}:{lineno} ({fname})")
    for kind in WAKE_KINDS:
        if not consumers[kind]:
            r.err("E_CONSUMER_MISSING", f"không có nhánh xử lý `MailboxMsg::{kind}`")
        allowed = {MAIL_CONSUMER}
        if kind == "Migrate":
            allowed.add("drain_mail_fifo")
        for site in consumers[kind]:
            fname = site.rsplit("(", 1)[-1].rstrip(")")
            if fname not in allowed:
                r.err(
                    "E_CONSUMER",
                    f"`MailboxMsg::{kind}` xử lý ở `{fname}` ({site}) — chỉ {sorted(allowed)}",
                )

    # 3) Không doorbell/kênh riêng theo arch.
    for p, text, fns in files:
        relp = rel(root, p)
        if not re.match(r"v2/kernel/vifast-plat-(riscv64|aarch64|x86_64)/", relp):
            continue
        if is_test_file(p, root):
            continue
        tr = test_ranges.get(str(p), [])
        for lineno, line in enumerate(text.splitlines(), 1):
            if in_ranges(lineno, tr):
                continue
            if re.search(r"\b(?:wake_step|MailboxMsg|send_reschedule|wake_hart)\b", line):
                r.err(
                    "E_ARCH_WAKE",
                    f"{relp}:{lineno}: tầng arch có kênh wake riêng",
                )
            if RE_IPI_WAKE_POST.search(line) and RE_IPI_POST.search(line):
                r.err(
                    "E_ARCH_DOORBELL",
                    f"{relp}:{lineno}: arch tự post `IpiKind::Wake` (doorbell riêng)",
                )
            if RE_DOORBELL.search(line) and not p.name.startswith("virtio_"):
                r.err(
                    "E_ARCH_DOORBELL",
                    f"{relp}:{lineno}: `doorbell` ngoài thiết bị virtio",
                )
            if RE_READY_TRANSITION.search(line):
                fn = enclosing(fns, lineno)
                body = ""
                if fn is not None:
                    body = "\n".join(text.splitlines()[fn.start - 1 : fn.end])
                if "is_blocked()" in body or "Sleeping" in body:
                    r.err(
                        "E_ARCH_WAKE",
                        f"{relp}:{lineno}: arch tự chuyển blocked/sleeping → Ready",
                    )

    # 4) Kiểu thiết kế `vifast-sync::Mailbox` có đủ ba loại (đã kiểm ở mỏ neo) — in ánh xạ.
    r.note(
        "kiểu thiết kế: `vifast-sync::Mailbox` (Msg::Wake/Reschedule/Migrate); "
        "kernel dùng `vifast-core::mailbox::MailboxMsg`; sim dùng `vifast-sim::mailbox::MailboxMsg`"
    )

    # 5) `--strict`: mọi chuyển blocked/sleeping → Ready phải nằm trong `wake_step`.
    if strict:
        transitions: List[str] = []
        for p, text, fns in files:
            relp = rel(root, p)
            if is_test_file(p, root):
                continue
            tr = test_ranges.get(str(p), [])
            lines = text.splitlines()
            for lineno, line in enumerate(lines, 1):
                if in_ranges(lineno, tr):
                    continue
                if not RE_READY_TRANSITION.search(line):
                    continue
                fn = enclosing(fns, lineno)
                if fn is None:
                    continue
                body = "\n".join(lines[fn.start - 1 : fn.end])
                # Chỉ tính **đánh thức** (nguồn blocked/sleeping), không tính Running→Ready
                # của `dispatch`/`yield` hay khởi tạo `Inactive→Ready`.
                if "is_blocked()" not in body and "Sleeping" not in body:
                    continue
                # `transition_tcb` là **nguyên thuỷ** chuyển trạng thái (không phải đường wake);
                # `tcb_suspend` là **treo** (Ready + suspended), không phải đánh thức.
                if fn.name in ("transition_tcb", "tcb_suspend"):
                    continue
                transitions.append(f"{relp}:{lineno} ({fn.name})")
        bad = [t for t in transitions if not t.endswith(f"({WAKE_STEP})")]
        for t in transitions:
            r.note(f"chuyển Ready: {t}")
        if bad:
            r.err(
                "E_STRICT_TRANSITION",
                f"{len(bad)} chỗ chuyển `TcbState::Ready` ngoài `{WAKE_STEP}` "
                f"(nợ N7-2, không nằm trong cổng mặc định)",
            )
            for t in bad:
                r.err("E_STRICT_TRANSITION", f"  {t}")

    return r


# ------------------------------------------------------------------ v1

def classify_v1_channel(body: str) -> str:
    """Phân loại kênh đánh thức chéo hart của một `wake_hart`/`send_reschedule`."""
    if "MAINT_KIND" in body or "MAINT_RESCHEDULE" in body:
        return "riscv64:MAINT_KIND+send_ipi"
    if "AP_RESCHED_PENDING" in body or "send_icr" in body or "APIC_MAINT_VECTOR" in body:
        return "x86_64:AP_RESCHED_PENDING+LAPIC"
    if "RESCHED_PENDING" in body and ("GICD" in body or "write32" in body):
        return "aarch64:RESCHED_PENDING+GIC SGI"
    if "vifast_maint::Mailbox" in body or "Mailbox" in body:
        return "mailbox"
    return "không rõ"


def scan_v1(root: Path, strict: bool) -> Report:
    r = Report()
    channels: Dict[str, List[str]] = {}
    for p in sorted(root.glob(V1_PLATFORM_GLOB)):
        text = p.read_text(encoding="utf-8", errors="replace")
        fns = find_functions(p, text)
        lines = text.splitlines()
        # Lượt 1: `send_reschedule` là **cơ chế**; `wake_hart` chỉ là **vỏ** gọi nó.
        per_file: Dict[str, str] = {}
        for fn in fns:
            if fn.name != "send_reschedule":
                continue
            body = "\n".join(lines[fn.start - 1 : fn.end])
            kind = classify_v1_channel(body)
            per_file[kind] = kind
            channels.setdefault(kind, []).append(f"{rel(root, p)}:{fn.start} ({fn.name})")
        # Lượt 2: `wake_hart` — vỏ gọi `send_reschedule` thì quy về cơ chế của tệp; nếu tự
        # dựng kênh (x86_64 LAPIC) thì phân loại theo primitive trong thân.
        for fn in fns:
            if fn.name != "wake_hart":
                continue
            body = "\n".join(lines[fn.start - 1 : fn.end])
            if "send_reschedule(" in body and per_file:
                kind = next(iter(per_file))
            else:
                kind = classify_v1_channel(body)
            channels.setdefault(kind, []).append(f"{rel(root, p)}:{fn.start} ({fn.name})")
    # Hàm đánh thức trong kernel (không phải kênh chéo hart) — chỉ liệt kê.
    kernel_wakes: List[str] = []
    for p in sorted(root.glob(V1_KERNEL_GLOB)):
        text = p.read_text(encoding="utf-8", errors="replace")
        for lineno, line in enumerate(text.splitlines(), 1):
            m = re.match(
                r"\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+"
                r"(make_ready|wake_error|wake_sleepers|wake_budgeters)\b",
                line,
            )
            if m:
                kernel_wakes.append(f"{rel(root, p)}:{lineno} ({m.group(1)})")
    for site in kernel_wakes:
        r.note(f"kernel wake: {site}")
    for kind, sites in sorted(channels.items()):
        for site in sites:
            r.note(f"kênh chéo hart: {kind} — {site}")
    if len(channels) != 1:
        r.err(
            "E_V1_MULTI_CHANNEL",
            f"tìm thấy {len(channels)} cơ chế đánh thức chéo hart: "
            + ", ".join(sorted(channels)),
        )
        for kind, sites in sorted(channels.items()):
            for site in sites:
                r.err("E_V1_MULTI_CHANNEL", f"  {kind}: {site}")
    if not channels:
        r.err("E_V1_NO_CHANNEL", "không tìm thấy kênh đánh thức nào (fail-closed)")
    return r


# ------------------------------------------------------------------ selftest

def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _fixture_v2(base: Path, extra_doorbell: bool = False, missing_wake_step: bool = False) -> None:
    """Cây v2 giả tối thiểu: một hộp thư, một `wake_step`, một `apply_mail`."""
    _write(
        base / KERNEL_MAILBOX,
        "pub enum MailboxMsg {\n    Wake(u32),\n    Reschedule,\n    Migrate { tcb: u32, to_hart: u16 },\n    Stop,\n    TlbFlush(u32),\n}\n",
    )
    _write(
        base / SIM_MAILBOX,
        "pub enum MailboxMsg {\n    Wake(u32),\n    Reschedule,\n    Migrate { tcb: u32, to_hart: u16 },\n    Stop,\n    TlbFlush(u32),\n}\n",
    )
    _write(
        base / SYNC_MAILBOX,
        "pub enum Msg {\n    Wake { hart: u16, tcb: u32 },\n    Reschedule { hart: u16, tcb: u32 },\n    Migrate { hart: u16, tcb: u32 },\n    Stop { hart: u16 },\n    VmKick { hart: u16 },\n    PowerReq { hart: u16, state: u8 },\n}\n",
    )
    _write(
        base / SYNC_TEST,
        "fn t() { let _ = Msg::Wake { hart: 0, tcb: 1 }; let _ = Msg::Reschedule { hart: 0, tcb: 1 }; let _ = Msg::Migrate { hart: 0, tcb: 1 }; }\n",
    )
    wake_step = "" if missing_wake_step else "    fn wake_step(&mut self, tcb: u32) -> u8 {\n        self.transition_tcb(tcb, TcbState::Ready)\n    }\n"
    _write(
        base / KERNEL_SCHED,
        "impl M {\n"
        "    fn wake(&mut self, tcb: u32) -> u8 {\n"
        "        if self.home(tcb) != self.current_hart {\n"
        "            return self.post_mail(self.home(tcb), MailboxMsg::Wake(tcb));\n"
        "        }\n"
        "        self.wake_step(tcb)\n"
        "    }\n"
        + wake_step
        + "    fn apply_mail(&mut self, hart: u16, msg: MailboxMsg) {\n"
        "        match msg {\n"
        "            MailboxMsg::Wake(tcb) => { let _ = self.wake_step(tcb); }\n"
        "            MailboxMsg::Reschedule => {}\n"
        "            MailboxMsg::Migrate { tcb, to_hart } => {}\n"
        "            MailboxMsg::Stop => {}\n"
        "            MailboxMsg::TlbFlush(_) => {}\n"
        "        }\n"
        "    }\n"
        "    fn push_step(&mut self) -> usize {\n"
        "        let msg = MailboxMsg::Migrate { tcb: 1, to_hart: 1 };\n"
        "        self.mail[1].try_push(msg);\n"
        "        1\n"
        "    }\n"
        "}\n",
    )
    _write(
        base / SIM_KOBJ,
        "impl M {\n"
        "    fn wake(&mut self, tcb: u32) -> u8 {\n"
        "        if self.home(tcb) != self.current_hart {\n"
        "            return self.post_mail(self.home(tcb), MailboxMsg::Wake(tcb));\n"
        "        }\n"
        "        self.wake_step(tcb)\n"
        "    }\n"
        "    fn wake_step(&mut self, tcb: u32) -> u8 {\n"
        "        self.transition_tcb(tcb, TcbState::Ready)\n"
        "    }\n"
        "    fn apply_mail(&mut self, hart: u16, msg: MailboxMsg) {\n"
        "        match msg {\n"
        "            MailboxMsg::Wake(tcb) => { let _ = self.wake_step(tcb); }\n"
        "            MailboxMsg::Reschedule => {}\n"
        "            MailboxMsg::Migrate { tcb, to_hart } => {}\n"
        "            MailboxMsg::Stop => {}\n"
        "            MailboxMsg::TlbFlush(_) => {}\n"
        "        }\n"
        "    }\n"
        "    fn push_step(&mut self) -> usize {\n"
        "        let msg = MailboxMsg::Migrate { tcb: 1, to_hart: 1 };\n"
        "        self.mail[1].try_push(msg);\n"
        "        1\n"
        "    }\n"
        "}\n",
    )
    if extra_doorbell:
        _write(
            base / "v2/kernel/vifast-plat-riscv64/src/ipi.rs",
            "fn post_wake(hart: usize) {\n    IPI_MAILBOXES.get(hart).post(IpiKind::Wake);\n}\n",
        )


def _fixture_v1(base: Path, channels: int = 3) -> None:
    _write(
        base / "crates/vifast-platform-riscv64/src/platform.rs",
        "fn send_reschedule(target: usize) -> bool {\n    MAINT_KIND[target].fetch_or(MAINT_RESCHEDULE, Ordering::AcqRel);\n    send_ipi(target);\n    true\n}\n",
    )
    if channels >= 2:
        _write(
            base / "crates/vifast-platform-aarch64/src/platform.rs",
            "fn send_reschedule(target: usize) -> bool {\n    RESCHED_PENDING[target].swap(true, Ordering::SeqCst);\n    write32(GICD + 0xF00, 1);\n    true\n}\n",
        )
    if channels >= 3:
        _write(
            base / "crates/vifast-platform-x86_64/src/platform.rs",
            "fn wake_hart(hart: usize) {\n    AP_RESCHED_PENDING.store(true, Ordering::Release);\n    send_icr(base, APIC_MAINT_VECTOR);\n}\n",
        )
    _write(
        base / "crates/vifast-kernel/src/lib.rs",
        "fn wake_sleepers(&mut self, now: u64) -> usize { 0 }\nfn wake_error(&mut self, tcb: u32, e: u8) {}\n",
    )


def selftest() -> int:
    """Bốn ca dương + ba ca âm; mỗi ca một dòng `ok`/`not ok`."""
    cases: List[Tuple[str, bool, str]] = []
    tmp = Path(tempfile.mkdtemp(prefix="kt151-selftest-"))
    try:
        # (1) v2 sạch ⇒ xanh.
        d = tmp / "v2-clean"
        _fixture_v2(d)
        rep = scan_v2(d, strict=False)
        cases.append(("v2 sạch ⇒ xanh", rep.ok, "; ".join(rep.errors)))

        # (2) v2 thêm doorbell arch ⇒ đỏ.
        d = tmp / "v2-doorbell"
        _fixture_v2(d, extra_doorbell=True)
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "v2 thêm `IpiKind::Wake` ở arch ⇒ đỏ",
                (not rep.ok) and any("E_ARCH_DOORBELL" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (3) v2 thiếu `wake_step` ⇒ đỏ (fail-closed).
        d = tmp / "v2-missing"
        _fixture_v2(d, missing_wake_step=True)
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "v2 thiếu `wake_step` ⇒ đỏ (fail-closed)",
                (not rep.ok) and any("E_ANCHOR_MISSING" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4) v2 `--strict` với transition ngoài `wake_step` ⇒ đỏ.
        d = tmp / "v2-strict"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn reply(&mut self, tcb: u32) -> u8 {\n        if self.tcb_state(tcb).is_blocked() {\n            return self.transition_tcb(tcb, TcbState::Ready);\n        }\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True)
        cases.append(
            (
                "v2 `--strict` transition ngoài `wake_step` ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (5) v1 ba kênh ⇒ đỏ.
        d = tmp / "v1-three"
        _fixture_v1(d, channels=3)
        rep = scan_v1(d, strict=False)
        cases.append(
            (
                "v1 ba kênh ⇒ đỏ",
                (not rep.ok) and any("E_V1_MULTI_CHANNEL" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (6) v1 một kênh ⇒ xanh (đối chứng dương: máy kiểm không phải luôn-đỏ).
        d = tmp / "v1-one"
        _fixture_v1(d, channels=1)
        rep = scan_v1(d, strict=False)
        cases.append(("v1 một kênh ⇒ xanh", rep.ok, "; ".join(rep.errors)))

        # (7) cây rỗng ⇒ đỏ (fail-closed).
        d = tmp / "empty"
        d.mkdir()
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "cây rỗng ⇒ đỏ (fail-closed)",
                (not rep.ok) and any("E_ANCHOR_MISSING" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    fails = 0
    for name, ok, detail in cases:
        if ok:
            print(f"ok - {name}")
        else:
            fails += 1
            print(f"not ok - {name}")
            if detail:
                print(f"#   {detail}")
    print(f"# selftest: {len(cases) - fails}/{len(cases)} ok")
    return 0 if fails == 0 else 1


# ------------------------------------------------------------------ main

def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--root", default=None, help="gốc cây nguồn (mặc định: repo chứa tệp này)")
    ap.add_argument("--strict", action="store_true", help="đòi mọi chuyển Ready nằm trong wake_step")
    ap.add_argument("--json", action="store_true", help="in kết quả dạng JSON")
    ap.add_argument("--selftest", action="store_true", help="chạy 7 ca tự kiểm")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()

    here = Path(__file__).resolve()
    root = Path(args.root).resolve() if args.root else here.parent.parent.parent
    if not root.is_dir():
        print(f"check-wake-paths: không có thư mục {root}", file=sys.stderr)
        return 2

    if (root / "v2/kernel").is_dir():
        rep = scan_v2(root, strict=args.strict)
        mode = "v2"
    elif (root / "crates").is_dir():
        rep = scan_v1(root, strict=args.strict)
        mode = "v1"
    else:
        print(
            f"check-wake-paths: {root} không có `v2/kernel` lẫn `crates` — fail-closed",
            file=sys.stderr,
        )
        return 1

    if args.json:
        print(
            json.dumps(
                {
                    "mode": mode,
                    "ok": rep.ok,
                    "errors": rep.errors,
                    "warnings": rep.warnings,
                    "info": rep.info,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        for line in rep.info:
            print(f"# {line}")
        for line in rep.warnings:
            print(f"W {line}")
        for line in rep.errors:
            print(f"E {line}")
        print(
            f"# check-wake-paths: mode={mode} strict={int(args.strict)} "
            f"errors={len(rep.errors)} warnings={len(rep.warnings)}"
        )
    return 0 if rep.ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
