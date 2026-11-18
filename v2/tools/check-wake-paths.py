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
- **`--strict`** (KT-155): **allowlist tường minh** — mọi `transition_tcb(…, TcbState::Ready)`
  trong mã sản phẩm phải hoặc nằm trong một **cửa wake** (`wake_step`/`wake_from`), hoặc có
  tên trong `NON_WAKE_READY_SITES` kèm lý do. Fail-closed **hai chiều**: chỗ lạ trong mã ⇒
  ĐỎ; mục trong allowlist không còn khớp ⇒ ĐỎ. Mức này **xanh** trên v2 sau KT-154/155 và
  **nằm trong `v2 check`** từ KT-155 (thay heuristic "thân hàm có `is_blocked()`/`Sleeping`"
  vốn chỉ phủ 4/16 đường — KT-154 §7).
- **`--strict`** (KT-159, D2-144; **KT-165** mở rộng; **KT-171** vá vòng 3): `wake()` là API
  **CHỈ-DÙNG-CHO-TEST** —
  **mọi** tham chiếu `wake` (lời gọi `wake(`, `Machine::wake`/`::wake`, `use …wake`, `wake` dùng
  làm giá trị) trong **mã sản phẩm** (`vifast-core/src/`, `vifast-sim/src/`,
  `vifast-kbin-riscv64/src/`, **mọi** `vifast-plat-*/src/`) **ngoài** `#[cfg(test)]`/`mod tests`
  ⇒ ĐỎ (`E_STRICT_TEST_ONLY_WAKE`), fail-closed. Bỏ qua **chú thích** (`//`, `///`, `//!`,
  `/* … */`), **literal chuỗi/ký tự**, **định nghĩa** (`fn wake`) và **biến cục bộ** tên `wake`.
  Vì sao: `wake()` = post-only, `wake_from()` = post + `Ready` ngay — gọi `wake()` trong đường
  sản phẩm (đã tự gỡ bookkeeping) làm TCB nằm chờ trong hộp thư trong khi C2/`check_ipc` đã đỏ.

  **KT-171** — reviewer L11 (KT-168 §1.2) phá bản KT-165 bằng **sáu** lớp; hai lớp là **lỗ
  hổng thật** (mã sản phẩm biên dịch `wake`, `--strict` vẫn `errors=0`). Nay:
    - **B8/B9** — "test-only" xác định bằng **đồ thị module** (`test_only_modules`), không
      theo tên/đường dẫn: tệp chỉ test-only khi mọi đường tới nó từ ngữ cảnh test
      (`tests/` cấp crate, `#[cfg(test)]`, `#[cfg(feature = "ktest")]`, bin
      `required-features = ["ktest"]`). `ktest_evil.rs` `mod` từ `main.rs` là **sản phẩm**;
      `src/tests/evil.rs` với `mod tests;` không `#[cfg(test)]` là **sản phẩm**.
    - **B6/B7** — `include!`/`#[path]` được **phân giải**; đích **trong** `v2/kernel`+`v2/sim`
      được **quét**, đích **ngoài** ⇒ ĐỎ (`E_STRICT_WAKE_INCLUDE`, fail-closed).
    - **B10** — miễn local-binding **theo khối** (không theo cả hàm); token `wake` làm **đối
      số macro** không được miễn (macro có thể biến ident thành tên hàm: `$m.$name($t)`).
    - **B4** — quét thân `macro_rules!` trong **`v2/lib/**`** tìm token `wake`
      (`E_STRICT_MACRO_WAKE`), allowlist `MACRO_WAKE_ALLOW` có lý do.

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
import bisect
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
#
# **KT-155 (N7-4)** — thêm **một** cửa cùng kênh: `wake_from_unlink` (như `wake_from` nhưng
# `unlink = true`) cho `wake_addr` — waiter `BlockedOnAddress` phải gỡ bucket WaitAddr **và**
# đánh thức hart `home`. Vẫn **một** kênh (`MailboxMsg::Wake`), ba cửa; đã quét lại toàn bộ
# `v2/kernel` + `v2/sim` (Luật 2) — xem `v2/agent/JOURNAL/kt155-n7-dong.md`.
WAKE_PRODUCER = ("wake", "wake_from", "wake_from_unlink")

# Hàm phát `Migrate` (push migration).
MIGRATE_PRODUCER = "push_step"

# **KT-159 (N7, D2-144)** — `wake()` là API **CHỈ-DÙNG-CHO-TEST**.
#
# Đo được ở D2-144: `wake()` **không có** call site sản phẩm nào — chỉ ảnh test
# (`vifast-kbin-riscv64/src/ktest_inv.rs:51`, `ktest_user.rs:692`) và test sim. Nhưng nó vẫn là
# **bẫy**: `wake()` = **post-only** (khác hart: chỉ gửi `MailboxMsg::Wake`, **không** đổi trạng
# thái ngay) còn `wake_from()` = **post + `Ready` ngay**. Người viết sau gọi `wake()` trong một
# đường sản phẩm (đã tự gỡ bookkeeping) sẽ làm TCB nằm chờ trong hộp thư trong khi bất biến C2
# (`Reply.caller ⇔ BlockedOnReply`) / `check_ipc` đã đỏ. Vì vậy **cưỡng chế bằng máy**:
# lời gọi `wake(` trong `v2/kernel/vifast-core/src/` hoặc `v2/sim/vifast-sim/src/` ngoài
# `#[cfg(test)]`/`mod tests` ⇒ ĐỎ (`E_STRICT_TEST_ONLY_WAKE`), fail-closed.
#
# **KT-165 (L8)** — phạm vi **đầy đủ** sau review L6 (KT-161 §2.3 H1/H2). KT-159 chỉ quét
# `vifast-core/src/` + `vifast-sim/src/`; reviewer đặt `m.wake(t)` trong
# `vifast-kbin-riscv64/src/proc_rt.rs` (H1) và `vifast-plat-riscv64/src/lib.rs` (H2) ⇒ `--strict`
# vẫn `errors=0`. Đó **đúng là nơi** một lời gọi sản phẩm sẽ được viết (ảnh sản phẩm + cầu nối
# arch), nên luật cũ chỉ đúng với người **không cố lách**. Nay quét **mọi** crate sản phẩm: core,
# sim, ảnh kernel (`kbin`), và **mọi** crate `vifast-plat-*`.
#
# Ảnh test (`vifast-kbin-riscv64/src/ktest*.rs`) và `tests/` vẫn được phép gọi `wake()` — đó là
# mục đích duy nhất còn lại của nó (`is_test_file` miễn).
TEST_ONLY_WAKE_SCOPE = (
    "v2/kernel/vifast-core/src/",
    "v2/sim/vifast-sim/src/",
    "v2/kernel/vifast-kbin-riscv64/src/",
)
# Mọi crate `vifast-plat-*` (riscv64, aarch64, common, user, …). Dùng **mẫu** chứ không liệt kê
# cứng: crate plat thứ tư ra đời sau vẫn tự động nằm trong phạm vi (fail-closed theo hướng mở).
RE_TEST_ONLY_WAKE_SCOPE_PLAT = re.compile(r"^v2/kernel/vifast-plat-[a-z0-9_]+/src/")


def in_test_only_wake_scope(relp: str) -> bool:
    """Tệp có thuộc phạm vi luật `wake()` chỉ-dùng-cho-test không (đường dẫn tương đối gốc)."""
    return relp.startswith(TEST_ONLY_WAKE_SCOPE) or bool(
        RE_TEST_ONLY_WAKE_SCOPE_PLAT.match(relp)
    )


# Token `wake` **đứng riêng**: không phải `wake_from`/`wake_from_unlink`/`wake_step`/`wake_addr`
# (sau `wake` là `_`), không phải `MailboxMsg::Wake` (hoa), không phải `rewake` (lookbehind).
#
# **KT-165** — luật cũ chỉ bắt `wake\s*\(` theo **từng dòng**, nên lách được ba cách (KT-161 §2.3):
#   H3 `let f = Machine::wake; f(t)` — không có `(` ngay sau `wake`;
#   H4 `m.wake /*x*/ (t)` — block comment chen giữa tên hàm và `(`;
#   H5 `m.wake` ⏎ `(t)` — lời gọi cắt dòng.
# Nay quét **token** trên toàn văn bản đã **khử nhiễu** (chú thích + literal chuỗi/ký tự), nên cả
# ba đều thành `wake` + khoảng trắng + `(`. Mọi tham chiếu `wake` khác (giá trị, `::wake`, `use`)
# cũng ĐỎ — trừ **định nghĩa** `fn wake` và **biến cục bộ** tên `wake`.
RE_WAKE_TOKEN = re.compile(r"(?<![A-Za-z0-9_])wake(?![A-Za-z0-9_])")
# Định nghĩa hàm (`pub fn wake(`, `unsafe fn wake(`) **không** phải tham chiếu — bỏ qua, nếu không
# chính chỗ khai báo API sẽ làm cổng đỏ.
RE_FN_WAKE_DEF_BEFORE = re.compile(r"\bfn\s+$")
# Tham chiếu đường dẫn tới hàm: `Machine::wake`, `Self::wake`, `<Machine>::wake`, `::wake`.
RE_WAKE_PATH_BEFORE = re.compile(r"::\s*$")
# Khai báo **biến cục bộ** tên `wake` — có thật trong cây (`core_tcb.rs:721` `let mut wake = …`,
# `kobj.rs:8095` `let mut wake: Vec<ObjId>`). Biến cục bộ không phải hàm nên không báo oan.
RE_LOCAL_WAKE_BINDING = re.compile(
    r"\blet\s+(?:mut\s+)?wake\b"
    r"|\bfor\s+wake\s+in\b"
    r"|\(\s*(?:mut\s+)?wake\s*:"
)

# Tệp định nghĩa kiểu hộp thư của từng tầng.
KERNEL_MAILBOX = "v2/kernel/vifast-core/src/mailbox.rs"
SIM_MAILBOX = "v2/sim/vifast-sim/src/mailbox.rs"
SYNC_MAILBOX = "v2/lib/vifast-sync/src/mailbox.rs"
SYNC_TEST = "v2/lib/vifast-sync/tests/kt30_sync.rs"
KERNEL_SCHED = "v2/kernel/vifast-core/src/core_sched.rs"
SIM_KOBJ = "v2/sim/vifast-sim/src/kobj.rs"

# **KT-155** — allowlist tường minh thay heuristic "thân hàm có `is_blocked()`/`Sleeping`".
# Heuristic cũ chỉ phủ **4/16** đường (KT-154 §7): đột biến ở `reply_with` (bỏ `wake_from`,
# gọi `transition_tcb` trực tiếp) **không** bị bắt vì thân hàm không có `is_blocked()`/
# `Sleeping`. Nay mọi `transition_tcb(…, TcbState::Ready)` trong mã sản phẩm phải hoặc nằm
# trong một **cửa wake**, hoặc có tên ở `NON_WAKE_READY_SITES` kèm lý do — fail-closed.
#
# Cửa wake: **chỉ** `wake_step` (thân chuyển trạng thái) và `wake_from` (cửa sản phẩm) —
# khoá theo (tệp, hàm) để hàm trùng tên ở tệp khác không lọt.
WAKE_DOORS = {
    (KERNEL_SCHED, "wake_step"),
    (KERNEL_SCHED, "wake_from"),
    (KERNEL_SCHED, "wake_from_unlink"),
    (SIM_KOBJ, "wake_step"),
    (SIM_KOBJ, "wake_from"),
    (SIM_KOBJ, "wake_from_unlink"),
}

# Chỗ `transition_tcb(…, Ready)` **không** phải wake, khoá (tệp, hàm, thứ tự trong hàm).
# Thứ tự trong hàm (1-based) để một chỗ **mới** trong cùng hàm bị bắt (fail-closed) mà không
# phụ thuộc số dòng (dòng trôi khi sửa tệp). Mục không còn khớp chỗ nào ⇒ ĐỎ (`E_STRICT_STALE`).
NON_WAKE_READY_SITES: Dict[Tuple[str, str, int], str] = {
    # --- kernel: chuyển Running→Ready (dispatch/yield/preempt) và Inactive→Ready (khởi tạo).
    ("v2/kernel/vifast-core/src/core_ipc.rs", "ensure_started", 1): "Inactive→Ready (khởi tạo TCB)",
    ("v2/kernel/vifast-core/src/core_ipc.rs", "reply_with", 1): "Running→Ready của chính replier",
    ("v2/kernel/vifast-core/src/core_sched.rs", "dispatch_step", 1): "Running→Ready (dispatch)",
    ("v2/kernel/vifast-core/src/core_sched.rs", "yield_current", 1): "Running→Ready (yield)",
    ("v2/kernel/vifast-core/src/core_tcb.rs", "transition_tcb", 1): "nguyên thuỷ: preempt Running→Ready",
    ("v2/kernel/vifast-core/src/core_tcb.rs", "tcb_suspend", 1): "treo: Ready + suspended (không chạy)",
    # --- kernel: mã khởi tạo sản phẩm (không phải đường wake của TCB đang chờ).
    ("v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs", "build_rig", 1): "khởi tạo sản phẩm",
    ("v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs", "task_ready", 1): "khởi tạo sản phẩm",
    ("v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs", "build_rig", 1): "khởi tạo sản phẩm",
    ("v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs", "turn", 1): "khởi tạo sản phẩm",
    ("v2/kernel/vifast-plat-riscv64/src/core_bridge.rs", "boot_smoke", 1): "khởi tạo sản phẩm",
    # --- sim: cùng phân loại với kernel (sim là oracle, đối xứng từng đường).
    ("v2/sim/vifast-sim/src/kobj.rs", "transition_tcb", 1): "nguyên thuỷ: preempt Running→Ready",
    ("v2/sim/vifast-sim/src/kobj.rs", "tcb_suspend", 1): "treo: Ready + suspended (không chạy)",
    ("v2/sim/vifast-sim/src/kobj.rs", "dispatch_step", 1): "Running→Ready (dispatch)",
    ("v2/sim/vifast-sim/src/kobj.rs", "yield_current", 1): "Running→Ready (yield)",
    ("v2/sim/vifast-sim/src/kobj.rs", "reply_with", 1): "Running→Ready của chính replier",
    ("v2/sim/vifast-sim/src/kobj.rs", "ensure_started", 1): "Inactive→Ready (khởi tạo TCB)",
}


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



RE_CFG_TEST_ATTR = re.compile(r"^\s*#\[cfg\(test\)\]")
RE_ATTR_LINE = re.compile(r"^\s*#!?\[")
# Mục **có thân khối** — chỉ những mục này mới mở được vùng test. `use`/`const`/`static`/`type`
# là câu lệnh một dòng, **không** có thân, nên `#[cfg(test)]` trên chúng không miễn gì cả.
RE_ITEM_WITH_BODY = re.compile(
    r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?(?:async\s+)?"
    r"(?:extern\s+\"[^\"]*\"\s+)?(?:mod|fn|impl|trait|struct|enum|union)\b"
)


def test_line_ranges(text: str) -> List[Tuple[int, int]]:
    """Khoảng dòng của các khối **test thật** (1-based, bao gồm).

    **KT-165 (L8)** — sửa lỗi H6 của KT-161 §2.3. Bản cũ thấy `#[cfg(test)]` rồi tìm `mod … {`
    trong **4 dòng kế**, nên `#[cfg(test)]` đặt trên một `use` **nuốt luôn** `mod` sản phẩm đứng
    sau ⇒ lời gọi `wake(` trong module sản phẩm được miễn. Nay chỉ nhận khi `#[cfg(test)]` đứng
    **ngay trước** một mục **có thân khối** (`mod`/`fn`/`impl`/`trait`/`struct`/`enum`/`union`),
    bỏ qua dòng thuộc tính kế tiếp và dòng trống (chú thích đã thành dòng trắng). `#[cfg(test)]`
    trên `use`/`const`/`static`/`type` **không** mở vùng test.
    """
    code = strip_rust_noise(text)
    lines = code.splitlines()
    n = len(lines)
    ranges: List[Tuple[int, int]] = []
    i = 0
    while i < n:
        if not RE_CFG_TEST_ATTR.match(lines[i]):
            i += 1
            continue
        # Bỏ qua thuộc tính kế tiếp + dòng trống (chú thích đã thành dòng trắng).
        j = i + 1
        while j < n and (not lines[j].strip() or RE_ATTR_LINE.match(lines[j])):
            j += 1
        if j >= n or not RE_ITEM_WITH_BODY.match(lines[j]):
            # `#[cfg(test)]` trên `use`/`const`/… ⇒ **không** miễn gì (H6).
            i += 1
            continue
        # Tìm `{` mở thân (cùng dòng hoặc vài dòng sau), dừng ở `;` (khai báo không thân).
        k = j
        opened = False
        while k < n and k - j <= 8:
            if "{" in lines[k]:
                opened = True
                break
            if ";" in lines[k]:
                break
            k += 1
        if not opened:
            i = j + 1
            continue
        depth = 0
        end = n - 1
        for m in range(k, n):
            depth += lines[m].count("{") - lines[m].count("}")
            if depth <= 0:
                end = m
                break
        ranges.append((i + 1, end + 1))
        i = end + 1
    return ranges


def is_test_file_heuristic(path: Path, root: Path) -> bool:
    """Heuristic **tên/đường dẫn** — CHỈ dùng cho các mục 1–5 (kênh/arch/transition).

    **KT-171**: đây **không** phải định nghĩa "test-only" của luật `wake`. Reviewer L11
    (KT-168 §1.2 B8/B9) đã phá đúng heuristic này: một module **sản phẩm** tên `ktest_*.rs`
    hoặc đặt dưới `src/tests/` được miễn dù được biên dịch vào ảnh sản phẩm. Luật `wake`
    dùng [`is_test_file`] (đồ thị module) — xem `test_only_modules`.
    """
    relp = rel(root, path)
    name = path.name
    return (
        "/tests/" in relp
        or name.endswith("_tests.rs")
        or name.startswith("ktest")
        or name == "tls_tests.rs"
    )


# ------------------------------------------------------------------ KT-171: test-only thật

# `mod NAME;` (không thân) — khai báo module ngoài.
RE_MOD_DECL = re.compile(
    r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"
)
# Bản MULTILINE trên văn bản **thô** — chỉ để lọc rẻ tệp có khai báo `mod NAME;`.
RE_MOD_DECL_RAW = re.compile(
    r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;", re.M
)
RE_PATH_ATTR_LINE = re.compile(r'^\s*#\[path\s*=\s*"([^"]+)"\s*\]\s*$')
RE_CFG_ATTR_LINE = re.compile(r"^\s*#\[cfg\((.*)\)\]\s*$")

# Cache theo gốc cây (selftest dựng nhiều cây tạm, mỗi cây một khoá).
_TEST_ONLY_CACHE: Dict[str, set] = {}
_MODS_CACHE: Dict[Tuple[str, str], List[Tuple[Path, bool, Path]]] = {}


def _cargo_sections(text: str) -> List[Tuple[str, Dict[str, str]]]:
    """Parse Cargo.toml **tối giản** (Python 3.9 không có `tomllib`).

    Chỉ cần `[lib]`/`[[bin]]` + `path`/`required-features`; bỏ qua phần còn lại.
    """
    out: List[Tuple[str, Dict[str, str]]] = []
    cur_name = ""
    cur: Dict[str, str] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[[") and line.endswith("]]"):
            if cur_name:
                out.append((cur_name, cur))
            cur_name = line[2:-2].strip()
            cur = {}
            continue
        if line.startswith("[") and line.endswith("]"):
            if cur_name:
                out.append((cur_name, cur))
            cur_name = line[1:-1].strip()
            cur = {}
            continue
        if "=" in line:
            k, v = line.split("=", 1)
            cur[k.strip()] = v.strip()
    if cur_name:
        out.append((cur_name, cur))
    return out


def _feature_list(raw: str) -> List[str]:
    return re.findall(r'"([^"]+)"', raw)


def crate_roots(root: Path) -> List[Tuple[Path, bool]]:
    """Crate root trong `v2/kernel` + `v2/sim`: `(tệp, test-only?)`.

    Test-only khi: bin có `required-features` chứa `ktest` (ảnh test kernel), hoặc
    `tests/*.rs` cấp crate (integration test). `src/lib.rs`/`src/main.rs` là **sản phẩm**.
    """
    out: List[Tuple[Path, bool]] = []
    seen: set = set()

    def add(p: Path, is_test: bool) -> None:
        if p.is_file() and p not in seen:
            seen.add(p)
            out.append((p, is_test))

    for base_rel in V2_SCAN:
        base = root / base_rel
        if not base.is_dir():
            continue
        for crate in sorted(p for p in base.iterdir() if p.is_dir()):
            cargo = crate / "Cargo.toml"
            text = ""
            if cargo.is_file():
                try:
                    text = cargo.read_text(encoding="utf-8", errors="replace")
                except OSError:
                    text = ""
            lib_path = "src/lib.rs"
            bins: List[Tuple[str, List[str]]] = []
            for name, kv in _cargo_sections(text):
                if name == "lib" and "path" in kv:
                    lib_path = kv["path"].strip('"')
                elif name == "bin":
                    bp = kv.get("path", "").strip('"')
                    if bp:
                        bins.append((bp, _feature_list(kv.get("required-features", ""))))
            add(crate / lib_path, False)
            for bp, feats in bins:
                add(crate / bp, "ktest" in feats)
            # Cargo tự nhận `src/lib.rs`/`src/main.rs` khi không khai báo.
            add(crate / "src/lib.rs", False)
            add(crate / "src/main.rs", False)
            tests_dir = crate / "tests"
            if tests_dir.is_dir():
                for t in sorted(tests_dir.glob("*.rs")):
                    add(t, True)
    return out


def _attrs_before(lines: Sequence[str], idx: int) -> List[str]:
    """Các dòng `#[…]` liền trước dòng `idx` (bỏ dòng trống), giữ thứ tự nguồn."""
    out: List[str] = []
    j = idx - 1
    while j >= 0:
        s = lines[j].strip()
        if not s:
            j -= 1
            continue
        if s.startswith("#["):
            out.append(s)
            j -= 1
            continue
        break
    out.reverse()
    return out


def _cfg_is_test(cfg: str) -> bool:
    """`#[cfg(…)]` có phải ngữ cảnh **test** không (fail-closed: chỉ nhận dạng chắc chắn)."""
    cfg = cfg.strip()
    if cfg == "test":
        return True
    if cfg == 'feature = "ktest"':
        return True
    if cfg.startswith("all(") and cfg.endswith(")"):
        parts = [x.strip() for x in cfg[4:-1].split(",")]
        return bool(parts) and all(_cfg_is_test(x) for x in parts)
    return False


def _mod_is_test_context(
    attrs: Sequence[str], lineno: int, tr: Sequence[Tuple[int, int]]
) -> bool:
    """`mod` có nằm trong ngữ cảnh test không: trong khối `#[cfg(test)]`, hoặc có thuộc tính
    `#[cfg(test)]`/`#[cfg(feature = "ktest")]` **trực tiếp**."""
    if in_ranges(lineno, tr):
        return True
    for a in attrs:
        m = RE_CFG_ATTR_LINE.match(a)
        if m and _cfg_is_test(m.group(1)):
            return True
    return False


def _mods_of(p: Path, mod_dir: Path) -> List[Tuple[Path, bool, Path]]:
    """Module con khai báo trong `p`: `(tệp đích, mod nằm trong ngữ cảnh test?, thư mục module
    của đích)`.

    **KT-171** — `mod_dir` là thư mục chứa module con của `p` theo **đúng** luật Rust:
      - crate root (`lib.rs`/`main.rs`/`[[bin]] path`) ⇒ thư mục của chính nó;
      - tệp nạp bằng `#[path]` ⇒ thư mục của tệp đích (đây là lý do `ktest_vfs.rs` nạp bằng
        `#[path]` vẫn tìm thấy `ktest_vfs_once.rs` **cạnh nó**, không phải `ktest_vfs/`);
      - module thường `src/foo.rs` ⇒ `src/foo/`; `src/foo/mod.rs` ⇒ `src/foo/`.
    Không có `mod_dir` thì `ktest.rs` (bin root) sẽ bị coi là module `src/ktest/` và 12 module
    con của nó rơi khỏi đồ thị (bị coi là sản phẩm — sai, dù không lộ `wake` trên cây sạch).
    """
    key = (str(p), str(mod_dir))
    cached = _MODS_CACHE.get(key)
    if cached is not None:
        return cached
    out: List[Tuple[Path, bool, Path]] = []
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        _MODS_CACHE[key] = out
        return out
    # Rẻ: chỉ khử nhiễu tệp **có** khai báo `mod NAME;` (đa số tệp không có).
    if not RE_MOD_DECL_RAW.search(text):
        _MODS_CACHE[key] = out
        return out
    # Giữ literal chuỗi để đọc `#[path = "…"]`; chú thích vẫn bị khử.
    code = strip_rust_noise(text, keep_strings=True)
    lines = code.splitlines()
    tr = test_line_ranges(text)
    for idx, line in enumerate(lines):
        m = RE_MOD_DECL.match(line)
        if not m:
            continue
        name = m.group(1)
        attrs = _attrs_before(lines, idx)
        is_test = _mod_is_test_context(attrs, idx + 1, tr)
        target: Optional[Path] = None
        child_dir: Optional[Path] = None
        for a in attrs:
            pm = RE_PATH_ATTR_LINE.match(a)
            if pm:
                target = (p.parent / pm.group(1)).resolve()
                child_dir = target.parent
                break
        if target is None:
            for cand in (mod_dir / (name + ".rs"), mod_dir / name / "mod.rs"):
                if cand.is_file():
                    target = cand.resolve()
                    child_dir = mod_dir / name
                    break
        if target is not None and child_dir is not None:
            out.append((target, is_test, child_dir))
    _MODS_CACHE[key] = out
    return out


def test_only_modules(root: Path) -> set:
    """Tập `relpath` của tệp **thật sự test-only** (KT-171, vá B8/B9).

    Một tệp là test-only **chỉ khi** mọi đường tới nó đều từ ngữ cảnh test:
      (a) `tests/*.rs` cấp crate (integration test), hoặc
      (b) được `mod` trong khối `#[cfg(test)]` / có `#[cfg(test)]` trực tiếp, hoặc
      (c) được `mod` dưới `#[cfg(feature = "ktest")]` (ảnh test kernel), hoặc
      (d) là bin có `required-features` chứa `ktest`.
    **Tên tệp không đủ** (`ktest_evil.rs` là sản phẩm nếu `mod` từ `main.rs`), **thư mục
    `src/tests/` không đủ** (`mod tests;` không `#[cfg(test)]` là sản phẩm). Tệp không tới
    được từ crate root nào ⇒ **sản phẩm** (fail-closed).
    """
    key = str(root)
    cached = _TEST_ONLY_CACHE.get(key)
    if cached is not None:
        return cached
    status: Dict[Path, bool] = {}
    moddir: Dict[Path, Path] = {}
    work: List[Path] = []
    for p, is_test in crate_roots(root):
        if p not in status:
            status[p] = is_test
            moddir[p] = p.parent
            work.append(p)
        elif status[p] and not is_test:
            status[p] = False
            work.append(p)
    while work:
        p = work.pop()
        for child, child_test, child_dir in _mods_of(p, moddir[p]):
            new = status[p] or child_test
            if child not in status:
                status[child] = new
                moddir[child] = child_dir
                work.append(child)
            elif status[child] and not new:
                status[child] = False
                work.append(child)
    out = {rel(root, p) for p, is_test in status.items() if is_test}
    _TEST_ONLY_CACHE[key] = out
    return out


def is_test_file(path: Path, root: Path, test_only: Optional[set] = None) -> bool:
    """Tệp **test-only thật sự** (KT-171) — dùng cho luật `E_STRICT_TEST_ONLY_WAKE`.

    Khác [`is_test_file_heuristic`]: dựa trên **đồ thị module** (`test_only_modules`), không
    dựa tên tệp/đường dẫn. Đây là bản vá B8/B9 của reviewer L11 (KT-168 §1.2).
    """
    if test_only is None:
        test_only = test_only_modules(root)
    return rel(root, path) in test_only


# ------------------------------------------------------------------ KT-171: B6/B7 include!/#[path]

RE_INCLUDE_MACRO = re.compile(r'\binclude!\s*\(\s*"([^"]+)"\s*\)')
RE_PATH_ATTR_ANY = re.compile(r'#\[path\s*=\s*"([^"]+)"\s*\]')


def _inside_scan_tree(root: Path, target: Path) -> Optional[str]:
    """`relpath` nếu `target` nằm trong `v2/kernel`/`v2/sim`, ngược lại `None`."""
    try:
        relp = str(target.relative_to(root))
    except ValueError:
        return None
    if relp.startswith("v2/kernel/") or relp.startswith("v2/sim/"):
        return relp
    return None


def resolve_include_targets(
    root: Path, files: Sequence[Tuple[Path, str, List[Fn]]], test_only: set
) -> Tuple[Dict[str, Tuple[Path, bool]], List[str]]:
    """KT-171 (B6/B7): phân giải `include!("…")` + `#[path = "…"]` và trả tệp đích.

    Reviewer L11 đặt `m.wake(t)` trong tệp **ngoài** phạm vi quét rồi kéo vào bằng `include!`
    hoặc `#[path]` ⇒ `--strict` vẫn `errors=0`. Nay:
      - đích **ngoài** `v2/kernel`+`v2/sim` ⇒ **ĐỎ** (không quét được, fail-closed);
      - đích **trong** cây ⇒ **quét chính tệp đích** (kế thừa trạng thái test-only của tệp
        kéo nó vào; sản phẩm thắng nếu cả hai).
    """
    extra: Dict[str, Tuple[Path, bool]] = {}
    errors: List[str] = []
    resolved: set = set()
    queue: List[Tuple[Path, bool]] = []
    for p, _, _ in files:
        queue.append((p, rel(root, p) in test_only))
    while queue:
        p, is_test = queue.pop()
        if p in resolved:
            continue
        resolved.add(p)
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        # Rẻ: đa số tệp không có `include!`/`#[path]` — khỏi khử nhiễu cả tệp.
        if "include!" not in text and "#[path" not in text:
            continue
        code = strip_rust_noise(text)
        code_keep = strip_rust_noise(text, keep_strings=True)
        relp = rel(root, p)
        found: List[Tuple[str, str, int]] = []
        for m in RE_INCLUDE_MACRO.finditer(code_keep):
            if not code[m.start() : m.end()].strip():
                continue  # nằm trong literal chuỗi
            found.append(("include!", m.group(1), m.start()))
        for m in RE_PATH_ATTR_ANY.finditer(code_keep):
            if not code[m.start() : m.end()].strip():
                continue
            found.append(("#[path]", m.group(1), m.start()))
        for kind, raw, off in found:
            lineno = code_keep.count("\n", 0, off) + 1
            target = (p.parent / raw).resolve()
            inside = _inside_scan_tree(root, target)
            if inside is None:
                errors.append(
                    f"  {relp}:{lineno}: `{kind}` trỏ ra ngoài `v2/kernel`+`v2/sim` "
                    f"(`{raw}`) — không quét được đích (fail-closed, KT-171 B6/B7)"
                )
                continue
            if not target.is_file():
                errors.append(
                    f"  {relp}:{lineno}: `{kind}` trỏ tới `{inside}` không tồn tại "
                    f"(fail-closed, KT-171 B6/B7)"
                )
                continue
            if inside in extra:
                extra[inside] = (target, extra[inside][1] and is_test)
            else:
                extra[inside] = (target, is_test)
            if target not in resolved:
                queue.append((target, is_test))
    return extra, errors


# ------------------------------------------------------------------ KT-171: B4 macro v2/lib

RE_MACRO_RULES = re.compile(r"\bmacro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)")

# Allowlist **có lý do** cho thân `macro_rules!` trong `v2/lib/**` chứa token `wake`.
# Khoá: (relpath, tên macro). Hiện **rỗng** — không có macro nào trong `v2/lib` chạm `wake`.
MACRO_WAKE_ALLOW: Dict[Tuple[str, str], str] = {}


def scan_lib_macros(root: Path, r: Report) -> None:
    """KT-171 (B4): quét thân `macro_rules!` trong `v2/lib/**` tìm token `wake`.

    Reviewer L11 đặt `#[macro_export] macro_rules! wake_call` trong `v2/lib/vifast-abi` (ngoài
    phạm vi quét cũ) với thân `$m.wake($t)`; lời gọi `vifast_abi::wake_call!(m, t)` **không**
    chứa token `wake` nên lọt. Nay thân macro ở **mọi** crate `v2/lib` bị quét; token `wake`
    (không phải `wake_from`/`wake_step`/`wake_addr`) ⇒ ĐỎ, trừ allowlist có lý do.
    """
    base = root / "v2/lib"
    if not base.is_dir():
        return
    for p in sorted(base.rglob("*.rs")):
        if "target" in p.parts:
            continue
        relp = rel(root, p)
        try:
            text = p.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        code = strip_rust_noise(text)
        for m in RE_MACRO_RULES.finditer(code):
            name = m.group(1)
            j = m.end()
            while j < len(code) and code[j] not in "{;":
                j += 1
            if j >= len(code) or code[j] != "{":
                continue
            depth = 0
            end = None
            for k in range(j, len(code)):
                if code[k] == "{":
                    depth += 1
                elif code[k] == "}":
                    depth -= 1
                    if depth == 0:
                        end = k
                        break
            if end is None:
                end = len(code) - 1
            body = code[j : end + 1]
            for wm in RE_WAKE_TOKEN.finditer(body):
                if (relp, name) in MACRO_WAKE_ALLOW:
                    continue
                lineno = code.count("\n", 0, j + wm.start()) + 1
                r.err(
                    "E_STRICT_MACRO_WAKE",
                    f"  {relp}:{lineno}: thân `macro_rules! {name}` chứa token `wake` — "
                    f"macro có thể sinh lời gọi `wake()` trong mã sản phẩm (KT-171 B4); "
                    f"dùng `wake_from`/`wake_step` hoặc thêm allowlist có lý do",
                )


# ------------------------------------------------------------------ KT-171: B10 scope + macro

# Macro **chỉ đọc giá trị** — `wake` làm đối số ở đây là biến cục bộ, không thể thành tên hàm.
SAFE_VALUE_MACROS = frozenset(
    {
        "assert",
        "assert_eq",
        "assert_ne",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "matches",
        "format",
        "write",
        "writeln",
        "println",
        "print",
        "vec",
        "panic",
        "todo",
        "unimplemented",
        "stringify",
        "concat",
    }
)
RE_MACRO_BANG_BEFORE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*!\s*$")


def _delim_pairs(code: str) -> List[Tuple[int, int, str]]:
    """Cặp `(mở, đóng, ký tự mở)` của mọi nhóm ngoặc trong mã đã khử nhiễu."""
    stack: List[Tuple[int, str]] = []
    out: List[Tuple[int, int, str]] = []
    close_of = {"(": ")", "[": "]", "{": "}"}
    for i, ch in enumerate(code):
        if ch in close_of:
            stack.append((i, ch))
        elif ch in (")", "]", "}"):
            if stack and close_of[stack[-1][1]] == ch:
                a, k = stack.pop()
                out.append((a, i, k))
    return out


def _innermost_group(
    groups: Sequence[Tuple[int, int, str]], pos: int, kind: Optional[str] = None
) -> Optional[Tuple[int, int, str]]:
    best: Optional[Tuple[int, int, str]] = None
    for a, b, k in groups:
        if kind is not None and k != kind:
            continue
        if a < pos < b:
            if best is None or (b - a) < (best[1] - best[0]):
                best = (a, b, k)
    return best


def _local_wake_scopes(
    code: str, groups: Sequence[Tuple[int, int, str]]
) -> List[Tuple[int, int, int]]:
    """`(vị trí binding, mở khối, đóng khối)` cho mỗi binding cục bộ tên `wake`.

    **KT-171 (B10)**: bản cũ `_fn_has_local_wake` miễn **cả hàm** khi thấy `let (mut) wake` ở
    **bất kỳ** scope con. Nay miễn **chỉ trong khối** của binding (và các khối con của nó).
    """
    out: List[Tuple[int, int, int]] = []
    for m in RE_LOCAL_WAKE_BINDING.finditer(code):
        start = m.start()
        head = code[start : m.end()].lstrip()
        if head.startswith("for"):
            # `for wake in expr { body }` — scope là **thân** vòng lặp.
            j = m.end()
            depth = 0
            body = None
            while j < len(code):
                c = code[j]
                if c in "([":
                    depth += 1
                elif c in ")]":
                    depth -= 1
                elif c == "{" and depth == 0:
                    body = j
                    break
                j += 1
            if body is None:
                continue
            blk = next(((a, b) for a, b, k in groups if k == "{" and a == body), None)
            if blk is None:
                continue
            out.append((start, blk[0], blk[1]))
        else:
            blk = _innermost_group(groups, start, "{")
            if blk is None:
                continue
            out.append((start, blk[0], blk[1]))
    return out


def _exempt_local_wake(
    pos: int,
    scopes: Sequence[Tuple[int, int, int]],
    groups: Sequence[Tuple[int, int, str]],
) -> bool:
    """`pos` có nằm trong scope của một binding `wake` **đứng trước** nó không."""
    blk = _innermost_group(groups, pos, "{")
    while blk is not None:
        a, b, _ = blk
        for bp, sa, sb in scopes:
            if sa == a and sb == b and bp < pos:
                return True
        blk = _innermost_group(groups, a, "{")
    return False


def _macro_arg_of(
    code: str, pos: int, groups: Sequence[Tuple[int, int, str]]
) -> Optional[str]:
    """Tên macro nếu `pos` nằm trong danh sách đối số của một lời gọi macro.

    **KT-171 (B10)**: `_b10!(m, wake, t)` với thân `$m.$name($t)` biến ident `wake` thành
    **tên hàm** — biến cục bộ `wake` không cứu được. Vì vậy token `wake` là đối số macro
    **không** được miễn theo local-binding (trừ macro chỉ-đọc-giá-trị ở `SAFE_VALUE_MACROS`).
    """
    g = _innermost_group(groups, pos)
    while g is not None:
        a, b, k = g
        m = RE_MACRO_BANG_BEFORE.search(code[max(0, a - 80) : a])
        if m:
            return m.group(1)
        g = _innermost_group(groups, a)
    return None


def in_ranges(line: int, ranges: Sequence[Tuple[int, int]]) -> bool:
    return any(a <= line <= b for a, b in ranges)


RE_RAW_STRING_OPEN = re.compile(r'(?:b|c)?r(#*)"')


def strip_rust_noise(text: str, keep_strings: bool = False) -> str:
    """Khử **chú thích** và **literal chuỗi/ký tự** khỏi mã Rust, giữ nguyên số dòng + số cột.

    **KT-171** — `keep_strings=True`: chỉ khử **chú thích**, giữ nguyên literal chuỗi/ký tự.
    Dùng cho việc phân giải `include!("…")`/`#[path = "…"]` (đường dẫn nằm trong literal, nếu
    khử luôn thì không đọc được). Mặc định `False` (khử cả literal) cho luật token `wake`.

    Vì sao cần (**KT-165**): luật `E_STRICT_TEST_ONLY_WAKE` phải thấy `m.wake /*x*/ (t)` là
    `m.wake  (t)` (H4 — block comment chen giữa tên hàm và `(`), và **không** được báo oan
    `"wake"` trong chuỗi hay `/// wake(tcb)` trong doc-comment. `strip_line_comment` cũ chỉ cắt
    `//` nên không đủ.

    Cách làm: thay **toàn bộ** đoạn chú thích/literal bằng dấu cách, giữ nguyên `\\n` để số dòng
    và số cột không đổi. Block comment của Rust **lồng nhau** nên đếm độ sâu. Literal thô
    (`r"…"`, `r#"…"#`, `br#"…"#`) đóng bằng `"` + đúng số `#`. `'a` (lifetime) **không** phải
    literal ký tự — phân biệt bằng độ dài (một ký tự rồi `'`).
    """
    out: List[str] = []
    i = 0
    n = len(text)

    def blank(seg: str) -> str:
        return "".join("\n" if ch == "\n" else " " for ch in seg)

    while i < n:
        c = text[i]
        # (1) Chú thích dòng `//`, `///`, `//!`.
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            if j < 0:
                j = n
            out.append(blank(text[i:j]))
            i = j
            continue
        # (2) Chú thích khối `/* … */` — Rust cho **lồng**.
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            depth = 0
            j = i
            while j < n:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                    if depth == 0:
                        break
                else:
                    j += 1
            out.append(blank(text[i:j]))
            i = j
            continue
        # (3) Chuỗi thô `r"…"`, `r#"…"#`, `br#"…"#` (không có escape).
        if c in "bcr" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            m = RE_RAW_STRING_OPEN.match(text, i)
            if m:
                close = '"' + m.group(1)
                j = text.find(close, m.end())
                j = n if j < 0 else j + len(close)
                out.append(text[i:j] if keep_strings else blank(text[i:j]))
                i = j
                continue
        # (4) Chuỗi thường `"…"` (có escape `\\`).
        if c == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(text[i:j] if keep_strings else blank(text[i:j]))
            i = j
            continue
        # (5) Literal ký tự `'x'`, `'\\n'` — **không** nhầm với lifetime `'a`.
        if c == "'" and (
            (i + 1 < n and text[i + 1] == "\\")
            or (i + 2 < n and text[i + 2] == "'")
        ):
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == "'":
                    j += 1
                    break
                j += 1
            out.append(text[i:j] if keep_strings else blank(text[i:j]))
            i = j
            continue
        out.append(c)
        i += 1
    return "".join(out)


def _in_use_item(code: str, pos: int) -> bool:
    """Vị trí `pos` có nằm trong một mục `use …;` không (kể cả `use a::{b, c};` nhiều dòng).

    Vì sao cần (**KT-165**, H3): `use vifast_core::Machine::{wake, wake_from};` nhập **hàm**
    `wake` vào phạm vi mà **không** có `::wake` liền nhau — nếu chỉ bắt `::wake` thì lọt. Cắt
    đoạn từ dấu `;` gần nhất phía trước (mục trước đã kết thúc) rồi tìm từ khoá `use`.
    """
    cut = code.rfind(";", 0, pos)
    stmt = code[cut + 1 : pos]
    return bool(re.search(r"\buse\b", stmt))


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

def scan_v2(
    root: Path,
    strict: bool,
    allowlist: Optional[Dict[Tuple[str, str, int], str]] = None,
) -> Report:
    r = Report()
    # **KT-171**: chuẩn hoá gốc (giải symlink) để `rel()`/`relative_to()` khớp nhau giữa
    # đường dẫn từ `iter_rs` và đường dẫn **phân giải** từ `#[path]`/`include!` (macOS:
    # `/var` → `/private/var`; selftest dựng cây trong `tempfile.mkdtemp`).
    root = root.resolve()

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
        if is_test_file_heuristic(p, root) or p.name == "mailbox.rs":
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
        if p.name == "mailbox.rs" or is_test_file_heuristic(p, root):
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
        if is_test_file_heuristic(p, root):
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

    # 5) `--strict` (KT-155): **allowlist tường minh** thay heuristic "thân hàm có
    #    `is_blocked()`/`Sleeping`". Heuristic cũ chỉ phủ 4/16 đường (KT-154 §7) nên đột biến
    #    ở `reply_with` (bỏ `wake_from`, gọi `transition_tcb` trực tiếp) **không** bị bắt.
    #    Nay **mọi** `transition_tcb(…, TcbState::Ready)` trong mã sản phẩm phải hoặc nằm
    #    trong một **cửa wake** (`WAKE_DOORS`), hoặc có tên trong `NON_WAKE_READY_SITES`
    #    (kèm lý do). Fail-closed **hai chiều**:
    #      - chỗ trong mã không có trong allowlist ⇒ ĐỎ (đường wake thứ hai / đột biến);
    #      - mục trong allowlist không còn khớp chỗ nào ⇒ ĐỎ (allowlist mục, phải cập nhật).
    if strict:
        allow = NON_WAKE_READY_SITES if allowlist is None else allowlist
        sites: List[Tuple[str, str, int, int]] = []
        for p, text, fns in files:
            relp = rel(root, p)
            if is_test_file_heuristic(p, root):
                continue
            tr = test_ranges.get(str(p), [])
            lines = text.splitlines()
            counts: Dict[Tuple[str, str], int] = {}
            for lineno, line in enumerate(lines, 1):
                if in_ranges(lineno, tr):
                    continue
                if not RE_READY_TRANSITION.search(line):
                    # Fail-closed: câu lệnh `transition_tcb(…)` **nhiều dòng** mà máy kiểm
                    # (quét theo dòng) không xếp loại được ⇒ ĐỎ, không bỏ qua im lặng. Gom
                    # **đúng** lời gọi theo cân bằng ngoặc (không quét sang câu lệnh kế).
                    if "transition_tcb" in line and not line.lstrip().startswith(
                        ("//", "*", "/*")
                    ):
                        window = "\n".join(lines[lineno - 1 : lineno + 8])
                        pos = window.index("transition_tcb")
                        depth = 0
                        end = None
                        for k in range(pos, len(window)):
                            c = window[k]
                            if c == "(":
                                depth += 1
                            elif c == ")":
                                depth -= 1
                                if depth == 0:
                                    end = k
                                    break
                        call = window[pos : end + 1] if end is not None else window[pos:]
                        if "TcbState::Ready" in call:
                            r.err(
                                "E_STRICT_TRANSITION",
                                f"  {relp}:{lineno}: `transition_tcb` nhiều dòng — "
                                f"máy kiểm không xếp loại được (fail-closed)",
                            )
                    continue
                fn = enclosing(fns, lineno)
                if fn is None:
                    # Fail-closed: không xác định được hàm bao quanh ⇒ không thể xếp loại.
                    r.err(
                        "E_STRICT_TRANSITION",
                        f"  {relp}:{lineno}: `TcbState::Ready` ngoài hàm — không xác định "
                        f"được cửa (fail-closed)",
                    )
                    continue
                key = (relp, fn.name)
                counts[key] = counts.get(key, 0) + 1
                sites.append((relp, fn.name, counts[key], lineno))
        used: set = set()
        bad: List[str] = []
        for relp, fname, ordinal, lineno in sites:
            if (relp, fname) in WAKE_DOORS:
                r.note(f"cửa wake: {relp}:{lineno} ({fname})")
                continue
            key = (relp, fname, ordinal)
            if key in allow:
                used.add(key)
                r.note(f"chuyển Ready không-wake: {relp}:{lineno} ({fname}) — {allow[key]}")
                continue
            bad.append(f"{relp}:{lineno} ({fname})")
        if bad:
            r.err(
                "E_STRICT_TRANSITION",
                f"{len(bad)} chỗ chuyển `TcbState::Ready` ngoài cửa wake "
                f"(allowlist tường minh KT-155)",
            )
            for b in bad:
                r.err("E_STRICT_TRANSITION", f"  {b}")
        # Mục trong allowlist phải còn khớp chỗ nào đó. Chỉ xét tệp **có trong cây** — cây
        # test tối thiểu (selftest) không có đủ tệp của repo thật, không được coi là mục.
        present = {rel(root, p) for p, _, _ in files}
        for key in sorted(allow):
            if key[0] not in present:
                continue
            if key not in used:
                r.err(
                    "E_STRICT_STALE",
                    f"  allowlist có mục không còn khớp: {key[0]} {key[1]} #{key[2]} "
                    f"({allow[key]})",
                )

        # 6) `--strict` (KT-159, D2-144; **KT-165** mở rộng; **KT-171** vá vòng 3):
        #    `wake()` là API **CHỈ-DÙNG-CHO-TEST**. **Mọi** tham chiếu `wake` trong **mã sản
        #    phẩm** ngoài `#[cfg(test)]`/`mod tests` ⇒ ĐỎ, fail-closed.
        #
        #    KT-165 vá **sáu** cách lách của reviewer L6 (KT-161 §2.3):
        #      H1/H2 — phạm vi: nay gồm `vifast-kbin-riscv64/src/` + **mọi** `vifast-plat-*/src/`;
        #      H3 — alias/fn-pointer `Machine::wake` (không có `(`): bắt theo **token**;
        #      H4 — block comment chen giữa `wake /*x*/ (t)`: khử nhiễu trước khi quét;
        #      H5 — gọi cắt dòng `wake` ⏎ `(t)`: quét **toàn văn bản**, không theo dòng;
        #      H6 — `#[cfg(test)]` trên `use` nuốt `mod` sản phẩm: `test_line_ranges` chỉ nhận
        #           `#[cfg(test)]` gắn **trực tiếp** mục có thân khối.
        #
        #    **KT-171** vá **sáu** lớp reviewer L11 đã phá (KT-168 §1.2):
        #      B8/B9 — miễn theo **tên/đường dẫn**: nay `is_test_file` dựa trên **đồ thị module**
        #              (`test_only_modules`): tệp chỉ test-only khi mọi đường tới nó từ ngữ cảnh
        #              test (`tests/` cấp crate, `#[cfg(test)]`, `#[cfg(feature = "ktest")]`,
        #              bin `required-features = ["ktest"]`). Tên `ktest*`/thư mục `src/tests/`
        #              **không** đủ.
        #      B6/B7 — `include!`/`#[path]` trỏ ra ngoài phạm vi: nay **phân giải** và **quét
        #              tệp đích**; đích ngoài `v2/kernel`+`v2/sim` ⇒ ĐỎ (fail-closed).
        #      B10 — miễn local-binding theo **cả hàm**: nay chỉ miễn trong **cùng khối** của
        #              binding; token `wake` làm **đối số macro** không được miễn (macro có thể
        #              biến ident thành tên hàm — `$m.$name($t)`).
        #      B4 — macro ở crate khác: `scan_lib_macros` quét thân `macro_rules!` trong
        #              `v2/lib/**` tìm token `wake` (allowlist `MACRO_WAKE_ALLOW` có lý do).
        #
        #    Bỏ qua: **chú thích** + **literal chuỗi/ký tự** (khử nhiễu), **định nghĩa**
        #    `fn wake`, và **biến cục bộ** tên `wake` (`let mut wake = …` — có thật trong cây).
        #    Vì sao: `wake()` = post-only, `wake_from()` = post + `Ready` ngay — gọi `wake()`
        #    trong đường sản phẩm (đã tự gỡ bookkeeping) làm TCB nằm chờ trong hộp thư trong
        #    khi C2/`check_ipc` đã đỏ. Xem `v2/agent/JOURNAL/kt159-wake2.md` +
        #    `v2/agent/JOURNAL/kt165-wakefix.md` + `v2/agent/JOURNAL/kt171-wakefix3.md`.
        test_only = test_only_modules(root)
        extra, inc_errors = resolve_include_targets(root, files, test_only)
        for e in inc_errors:
            r.err("E_STRICT_WAKE_INCLUDE", e)
        wake_targets: List[Tuple[Path, str, List[Fn], bool]] = []
        added: set = set()
        for p, text, fns in files:
            relp = rel(root, p)
            if not in_test_only_wake_scope(relp):
                continue
            wake_targets.append((p, text, fns, is_test_file(p, root, test_only)))
            added.add(p)
        for relp, (p, is_test) in sorted(extra.items()):
            if p in added:
                continue
            try:
                text = p.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            wake_targets.append((p, text, find_functions(p, text), is_test))
            added.add(p)

        for p, text, fns, is_test in wake_targets:
            relp = rel(root, p)
            if is_test:
                continue
            tr = test_ranges.get(str(p))
            if tr is None:
                tr = test_line_ranges(text)
            code = strip_rust_noise(text)
            # Đầu dòng (offset) để đổi offset token ⇒ số dòng, không phải quét lại từng dòng.
            line_starts = [0]
            for idx, ch in enumerate(code):
                if ch == "\n":
                    line_starts.append(idx + 1)
            groups: Optional[List[Tuple[int, int, str]]] = None
            scopes: List[Tuple[int, int, int]] = []

            for m in RE_WAKE_TOKEN.finditer(code):
                s = m.start()
                lineno = bisect.bisect_right(line_starts, s)
                if in_ranges(lineno, tr):
                    continue
                before = code[max(0, s - 80) : s]
                if RE_FN_WAKE_DEF_BEFORE.search(before):
                    continue  # định nghĩa `fn wake(` — chỗ khai báo API, không phải tham chiếu
                after = code[m.end() :]
                nxt = after.lstrip()[:1]
                if RE_WAKE_PATH_BEFORE.search(before):
                    why = "tham chiếu `…::wake` (alias/fn-pointer), không có `(`"
                elif nxt == "(":
                    why = "gọi `wake(`"
                elif _in_use_item(code, s):
                    why = "`use …wake` (nhập hàm `wake` vào phạm vi)"
                else:
                    if groups is None:
                        groups = _delim_pairs(code)
                        scopes = _local_wake_scopes(code, groups)
                    macro = _macro_arg_of(code, s, groups)
                    if macro is not None and macro not in SAFE_VALUE_MACROS:
                        why = (
                            f"`wake` là đối số của macro `{macro}!` — macro có thể biến "
                            f"ident thành tên hàm (KT-171 B10)"
                        )
                    elif _exempt_local_wake(s, scopes, groups):
                        continue  # biến cục bộ tên `wake` **trong cùng khối**, không phải hàm
                    else:
                        why = "dùng `wake` làm giá trị (không có `(`)"
                r.err(
                    "E_STRICT_TEST_ONLY_WAKE",
                    f"  {relp}:{lineno}: {why} trong mã sản phẩm — `wake()` là API "
                    f"CHỈ-DÙNG-CHO-TEST (D2-144); sản phẩm dùng `wake_from`/"
                    f"`wake_from_unlink`",
                )

        # 7) **KT-171 (B4)** — macro ở crate khác: quét thân `macro_rules!` trong `v2/lib/**`.
        scan_lib_macros(root, r)

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


def _fixture_v2(
    base: Path,
    extra_doorbell: bool = False,
    missing_wake_step: bool = False,
    extra_sched: str = "",
) -> None:
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
        + extra_sched
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
    """Mỗi ca một dòng `ok`/`not ok`.

    15 ca KT-151/155/159 + **11 ca KT-165** (sáu cách lách H1..H6 + năm đối chứng dương) +
    **13 ca KT-171** (sáu lớp B4/B6/B7/B8/B9/B10 của reviewer L11 + bảy đối chứng dương).
    """
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

        # (4) v2 `--strict` với transition ngoài cửa wake ⇒ đỏ.
        d = tmp / "v2-strict"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn reply(&mut self, tcb: u32) -> u8 {\n        if self.tcb_state(tcb).is_blocked() {\n            return self.transition_tcb(tcb, TcbState::Ready);\n        }\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "v2 `--strict` transition ngoài cửa wake ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4b) v2 `--strict` + allowlist khớp ⇒ xanh (đối chứng dương: allowlist không
        # luôn-đỏ). `dispatch_step` là chuyển Running→Ready, có tên trong allowlist.
        d = tmp / "v2-strict-allow"
        _fixture_v2(
            d,
            extra_sched=(
                "    fn dispatch_step(&mut self, hart: u16) -> u8 {\n"
                "        self.transition_tcb(1, TcbState::Ready);\n"
                "        0\n"
                "    }\n"
            ),
        )
        allow_ok = {(KERNEL_SCHED, "dispatch_step", 1): "Running→Ready (dispatch)"}
        rep = scan_v2(d, strict=True, allowlist=allow_ok)
        cases.append(
            (
                "v2 `--strict` + allowlist khớp ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4c) v2 `--strict` + **đột biến**: thêm chỗ thứ hai trong `dispatch_step` (hàm đã
        # có trong allowlist) ⇒ đỏ. Đây là ca chống tái diễn đúng lỗi KT-154 §7: heuristic cũ
        # bỏ sót vì thân hàm không có `is_blocked()`/`Sleeping`.
        d = tmp / "v2-strict-mutate"
        _fixture_v2(
            d,
            extra_sched=(
                "    fn dispatch_step(&mut self, hart: u16) -> u8 {\n"
                "        self.transition_tcb(1, TcbState::Ready);\n"
                "        self.transition_tcb(2, TcbState::Ready);\n"
                "        0\n"
                "    }\n"
            ),
        )
        rep = scan_v2(d, strict=True, allowlist=allow_ok)
        cases.append(
            (
                "v2 `--strict` đột biến trong hàm đã allowlist ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4d) v2 `--strict` + allowlist **mục** (không còn khớp chỗ nào) ⇒ đỏ (fail-closed).
        d = tmp / "v2-strict-stale"
        _fixture_v2(d)
        allow_stale = {(KERNEL_SCHED, "khong_ton_tai", 1): "mục"}
        rep = scan_v2(d, strict=True, allowlist=allow_stale)
        cases.append(
            (
                "v2 `--strict` allowlist mục ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_STALE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4e) v2 `--strict` + `transition_tcb` **nhiều dòng** ⇒ đỏ (fail-closed: máy kiểm
        # quét theo dòng không xếp loại được, không được bỏ qua im lặng).
        d = tmp / "v2-strict-multiline"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn reply(&mut self, tcb: u32) -> u8 {\n        self.transition_tcb(\n            tcb,\n            TcbState::Ready,\n        );\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "v2 `--strict` transition nhiều dòng ⇒ đỏ (fail-closed)",
                (not rep.ok) and any("nhiều dòng" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4f) v2 `--strict` + **đột biến KT-159**: lời gọi `wake(` trong mã sản phẩm
        # (`vifast-core/src/`) ⇒ đỏ `E_STRICT_TEST_ONLY_WAKE`. Đây là luật cưỡng chế D2-144:
        # `wake()` là API chỉ-dùng-cho-test, sản phẩm phải dùng `wake_from`.
        d = tmp / "v2-strict-test-only-wake"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn duong_san_pham(&mut self, tcb: u32) -> u8 {\n        self.wake(tcb);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "v2 `--strict` gọi `wake(` trong mã sản phẩm ⇒ đỏ (E_STRICT_TEST_ONLY_WAKE)",
                (not rep.ok)
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4g) Đối chứng dương: **chỉ** `wake_from(` ⇒ xanh (luật không phải "thấy chữ wake là
        # đỏ"; `wake_from`/`wake_from_unlink`/`wake_step` là đường sản phẩm hợp lệ).
        d = tmp / "v2-strict-wake-from"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn duong_san_pham(&mut self, tcb: u32) -> u8 {\n        self.wake_from(tcb);\n        self.wake_from_unlink(tcb);\n        self.wake_step(tcb, false);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "v2 `--strict` chỉ `wake_from(`/`wake_from_unlink(`/`wake_step(` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4h) `wake(` **trong** `#[cfg(test)] mod tests` ⇒ xanh (test được phép — mục đích duy
        # nhất còn lại của `wake()`).
        d = tmp / "v2-strict-wake-cfg-test"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn f(&mut self) -> u8 {\n        0\n    }\n}\n\n"
            "#[cfg(test)]\nmod tests {\n    fn t(m: &mut M) {\n        m.wake(1);\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "v2 `--strict` `wake(` trong `#[cfg(test)]` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4i) `wake(` **trong chú thích** (`///`, `//`, `//!`) ⇒ xanh — không báo oan
        # doc-comment của chính `wake()`.
        d = tmp / "v2-strict-wake-comment"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "//! Gọi `wake(tcb)` chỉ trong test.\n"
            "impl M {\n"
            "    /// `wake(tcb)` — chỉ dùng cho test.\n"
            "    // self.wake(tcb);\n"
            "    fn f(&mut self) -> u8 {\n"
            "        0\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "v2 `--strict` `wake(` trong chú thích ⇒ xanh (không báo oan)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # ============ KT-165 (L8) — sáu cách lách của reviewer L6 (KT-161 §2.3) ============
        # Mỗi ca: log giả **chứa đúng cách lách** ⇒ ĐỎ `E_STRICT_TEST_ONLY_WAKE`; kèm đối chứng
        # dương để chứng minh luật không phải "thấy chữ `wake` là đỏ".

        # (4j) **H1** — `m.wake(t)` trong **ảnh sản phẩm** `vifast-kbin-riscv64/src/` (đúng nơi
        # một lời gọi sản phẩm sẽ được viết). Trước KT-165: XANH (`errors=0`).
        d = tmp / "v2-strict-h1-kbin"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H1 `wake(` trong `vifast-kbin-riscv64/src/` ⇒ đỏ (KT-165)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4k) **H2** — `m.wake(t)` trong crate plat `vifast-plat-riscv64/src/`. Trước: XANH.
        d = tmp / "v2-strict-h2-plat"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-plat-riscv64/src/lib.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H2 `wake(` trong `vifast-plat-*/src/` ⇒ đỏ (KT-165)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4l) **H2b** — crate plat **mới** (`vifast-plat-aarch64`) cũng nằm trong phạm vi: dùng
        # **mẫu** `vifast-plat-*/src/`, không liệt kê cứng ⇒ crate thứ tư không lọt.
        d = tmp / "v2-strict-h2b-plat-moi"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-plat-aarch64/src/lib.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H2b `wake(` trong crate plat **mới** ⇒ đỏ (mẫu, không liệt kê cứng)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4m) **H3** — alias/fn-pointer: `let g = Machine::wake; g(self, t)` — **không** có `(`
        # ngay sau `wake`. Trước: XANH.
        d = tmp / "v2-strict-h3-alias"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        let g = Machine::wake;\n        g(self, t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H3 alias `let g = Machine::wake;` ⇒ đỏ (không cần `(`)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4n) **H3b** — `use …::{wake, wake_from};` nhập **hàm** vào phạm vi (không có `::wake`
        # liền nhau). Trước: XANH.
        d = tmp / "v2-strict-h3b-use"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "use vifast_core::Machine::{wake, wake_from};\nimpl M {\n    fn f(&mut self) -> u8 {\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H3b `use …::{wake, …}` ⇒ đỏ (nhập hàm vào phạm vi)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4o) **H4** — block comment chen giữa: `self.wake /* L6 */ (t)`. Trước: XANH.
        d = tmp / "v2-strict-h4-block-comment"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake /* L6 */ (t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4 `wake /*x*/ (t)` ⇒ đỏ (block comment chen giữa)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4p) **H5** — lời gọi cắt dòng: `self.wake` ⏎ `(t)`. Trước: XANH.
        d = tmp / "v2-strict-h5-cat-dong"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake\n            (t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H5 `wake` ⏎ `(t)` ⇒ đỏ (gọi cắt dòng)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4q) **H6** — `#[cfg(test)]` trên một `use`, rồi `mod` **sản phẩm** trong ≤ 4 dòng:
        # `test_line_ranges` cũ nuốt cả module ⇒ lời gọi `wake(` được miễn. Trước: XANH.
        d = tmp / "v2-strict-h6-cfg-test-use"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "#[cfg(test)]\nuse super::*;\n\nmod san_pham {\n    fn f(m: &mut M, t: u32) {\n        m.wake(t);\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H6 `#[cfg(test)]` trên `use` không được miễn `mod` sản phẩm ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4r) Đối chứng dương cho **phạm vi mới**: chỉ `wake_from(`/`wake_from_unlink(`/
        # `wake_step(`/`wake_addr(` trong `vifast-kbin-riscv64/src/` + `vifast-plat-*/src/` ⇒
        # XANH. Không có ca này thì "mở rộng phạm vi" có thể chỉ là "đỏ mọi thứ".
        d = tmp / "v2-strict-scope-moi-wake-from"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake_from(t);\n        self.wake_from_unlink(t);\n        self.wake_step(t, false);\n        self.wake_addr(1, 2, 3);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-plat-riscv64/src/lib.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n        self.wake_from(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "phạm vi mới: chỉ `wake_from(`/`wake_step(`/`wake_addr(` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4s) Đối chứng dương cho **H6**: `#[cfg(test)]` gắn **trực tiếp** `mod tests` vẫn được
        # miễn (kể cả khi có `#[cfg(test)]` trên `use` đứng trước) — luật không cấm test.
        d = tmp / "v2-strict-cfg-test-that"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "#[cfg(test)]\nuse super::*;\n\nimpl M {\n    fn f(&mut self) -> u8 {\n        0\n    }\n}\n\n"
            "#[cfg(test)]\nmod tests {\n    fn t(m: &mut M) {\n        m.wake(1);\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "`#[cfg(test)] mod tests` thật vẫn được miễn ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4t) Đối chứng dương cho **biến cục bộ** tên `wake` (có thật trong cây:
        # `core_tcb.rs:721`, `kobj.rs:8095`) — không báo oan.
        d = tmp / "v2-strict-bien-cuc-bo-wake"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_tcb.rs",
            "impl M {\n    fn f(&mut self) -> u8 {\n        let mut wake = [0u32; 4];\n        wake[0] = 1;\n        for &t in &wake[..1] {\n            let _ = t;\n        }\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "biến cục bộ `let mut wake = …` ⇒ xanh (không báo oan)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # ============ KT-171 (L14) — sáu lớp reviewer L11 đã phá (KT-168 §1.2) ============
        # Mỗi ca: log giả **chứa đúng cách lách** ⇒ ĐỎ; kèm đối chứng dương để chứng minh
        # luật không phải "thấy chữ `wake` là đỏ".

        # (4u) **B8** — module **sản phẩm** tên `ktest_evil.rs` + `mod ktest_evil;` trong
        # `main.rs` (ảnh sản phẩm). Trước KT-171: XANH (miễn theo tên `ktest*`).
        d = tmp / "v2-b8-ktest-name"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/Cargo.toml",
            '[package]\nname = "vifast-kbin-riscv64"\n\n'
            '[[bin]]\nname = "vifast-kbin-riscv64"\npath = "src/main.rs"\n',
        )
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/main.rs",
            "mod ktest_evil;\n\nfn main() {}\n",
        )
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B8 module sản phẩm tên `ktest_evil.rs` + `mod` từ `main.rs` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4v) Đối chứng dương cho B8: `#[cfg(feature = "ktest")] mod ktest_ok;` là **test-only
        # thật** (ảnh test kernel) ⇒ XANH.
        d = tmp / "v2-b8b-ktest-cfg"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/main.rs",
            '#[cfg(feature = "ktest")]\nmod ktest_ok;\n\nfn main() {}\n',
        )
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/ktest_ok.rs",
            "impl M {\n    fn test(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                'B8b `#[cfg(feature = "ktest")] mod ktest_ok;` ⇒ xanh (test-only thật)',
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4w) **B9** — module **sản phẩm** dưới `src/tests/` + `mod tests;` **không**
        # `#[cfg(test)]`. Trước KT-171: XANH (miễn theo đường dẫn `/tests/`).
        d = tmp / "v2-b9-src-tests"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "mod tests;\n")
        _write(d / "v2/kernel/vifast-core/src/tests.rs", "mod evil;\n")
        _write(
            d / "v2/kernel/vifast-core/src/tests/evil.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B9 `src/tests/evil.rs` + `mod tests;` không `#[cfg(test)]` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4x) Đối chứng dương cho B9: `#[cfg(test)] mod tests;` ⇒ XANH.
        d = tmp / "v2-b9b-src-tests-cfg"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod tests;\n")
        _write(d / "v2/kernel/vifast-core/src/tests.rs", "mod evil;\n")
        _write(
            d / "v2/kernel/vifast-core/src/tests/evil.rs",
            "impl M {\n    fn test(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B9b `#[cfg(test)] mod tests;` ⇒ xanh (test-only thật)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4y) **B6** — `include!` trỏ **ra ngoài** `v2/kernel`+`v2/sim` (tệp chứa `wake`).
        # Trước KT-171: XANH (không phân giải `include!`).
        d = tmp / "v2-b6-include-outside"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'include!("../../../lib/vifast-abi/src/evil_include.rs");\n',
        )
        _write(
            d / "v2/lib/vifast-abi/src/evil_include.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B6 `include!` trỏ ra ngoài `v2/kernel`+`v2/sim` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4z) **B6b** — `include!` trỏ **trong** `v2/kernel` nhưng **ngoài** `src/` (tệp không
        # nằm trong phạm vi quét trực tiếp): phải **quét tệp đích** ⇒ ĐỎ.
        d = tmp / "v2-b6b-include-inside"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'include!("../inc/evil_inc.rs");\n',
        )
        _write(
            d / "v2/kernel/vifast-core/inc/evil_inc.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B6b `include!` trỏ trong `v2/kernel` (ngoài `src/`) ⇒ quét đích ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4aa) **B7** — `#[path]` trỏ **ra ngoài** `v2/kernel`+`v2/sim`. Trước: XANH.
        d = tmp / "v2-b7-path-outside"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = "../../../lib/vifast-abi/src/evil_path.rs"]\nmod _b7;\n',
        )
        _write(
            d / "v2/lib/vifast-abi/src/evil_path.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B7 `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ab) Đối chứng dương cho B7: `#[path]` trỏ trong `v2/kernel`, đích chỉ dùng
        # `wake_from` ⇒ XANH (luật không phải "có `#[path]` là đỏ").
        d = tmp / "v2-b7b-path-inside"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = "../inc/evil_ok.rs"]\nmod _b7;\n',
        )
        _write(
            d / "v2/kernel/vifast-core/inc/evil_ok.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B7b `#[path]` trỏ trong cây, đích chỉ `wake_from(` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4ac) **B10** — `let mut wake` + macro nhận `$name:ident` biến ident `wake` thành
        # **tên hàm** (`$m.$name($t)`). Trước KT-171: XANH (miễn local-binding theo cả hàm).
        d = tmp / "v2-b10-local-macro"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "macro_rules! _b10 {\n"
            "    ($m:expr, $name:ident, $t:expr) => {\n"
            "        $m.$name($t)\n"
            "    };\n"
            "}\n"
            "impl M {\n"
            "    fn f(&mut self, t: u32) -> u8 {\n"
            "        let mut wake = 0u32;\n"
            "        wake += 1;\n"
            "        _b10!(self, wake, t);\n"
            "        0\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B10 `wake` làm đối số macro `$name:ident` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ad) **B10b** — binding trong **khối con**, token ở **khối cha** ⇒ ĐỎ (trước: XANH
        # vì `_fn_has_local_wake` miễn cả hàm).
        d = tmp / "v2-b10b-scope"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_tcb.rs",
            "impl M {\n    fn f(&mut self) -> u8 {\n        {\n"
            "            let mut wake = [0u32; 4];\n            wake[0] = 1;\n        }\n"
            "        let _ = wake;\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B10b binding ở khối con, token ở khối cha ⇒ đỏ (scope, KT-171)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ae) Đối chứng dương cho B10: binding + dùng **cùng khối** (và trong khối con của
        # nó) ⇒ XANH — không báo oan biến cục bộ có thật trong cây.
        d = tmp / "v2-b10c-cung-khoi"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_tcb.rs",
            "impl M {\n    fn f(&mut self) -> u8 {\n"
            "        let mut wake = [0u32; 4];\n"
            "        if true {\n            wake[0] = 1;\n        }\n"
            "        for &t in &wake[..1] {\n            let _ = t;\n        }\n"
            "        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B10c binding + dùng cùng khối ⇒ xanh (không báo oan)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4af) **B4** — `#[macro_export] macro_rules! wake_call` trong `v2/lib/vifast-abi`
        # (ngoài phạm vi quét cũ), thân `$m.wake($t)`. Trước KT-171: XANH.
        d = tmp / "v2-b4-macro-lib"
        _fixture_v2(d)
        _write(
            d / "v2/lib/vifast-abi/src/lib.rs",
            "#[macro_export]\nmacro_rules! wake_call {\n"
            "    ($m:expr, $t:expr) => {\n        $m.wake($t)\n    };\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B4 `macro_rules!` trong `v2/lib` chứa token `wake` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_MACRO_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ag) Đối chứng dương cho B4: macro trong `v2/lib` chỉ dùng `wake_from` ⇒ XANH.
        d = tmp / "v2-b4b-macro-lib-ok"
        _fixture_v2(d)
        _write(
            d / "v2/lib/vifast-abi/src/lib.rs",
            "#[macro_export]\nmacro_rules! wake_call {\n"
            "    ($m:expr, $t:expr) => {\n        $m.wake_from($t)\n    };\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B4b `macro_rules!` trong `v2/lib` chỉ `wake_from(` ⇒ xanh",
                rep.ok,
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
    ap.add_argument(
        "--strict",
        action="store_true",
        help="allowlist tường minh: mọi chuyển Ready phải qua cửa wake hoặc có trong allowlist",
    )
    ap.add_argument("--json", action="store_true", help="in kết quả dạng JSON")
    ap.add_argument("--selftest", action="store_true", help="chạy 39 ca tự kiểm")
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
