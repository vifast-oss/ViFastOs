#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-407 — phần **cài đặt** của máy kiểm `wake` (tách từ `v2/tools/check-wake-paths.py`).

Tệp gốc 11.046 dòng được tách thành package `v2/tools/checkers/wake/`:

  * `core.py`     — hằng số, lexer/token-tree, phân tích Rust, luật `wake`, quét v2/v1;
  * `selftest.py` — 461 ca tự kiểm (giữ nguyên số ca và nội dung);
  * `cli.py`      — `main()` + argparse;
  * `__main__.py` — `python -m checkers.wake`.

`v2/tools/check-wake-paths.py` giữ vai trò **shim**: re-export toàn bộ namespace `core` (kể cả
tên private) để `check-wake-depinfo.py` nạp tệp đó như module và dùng lại luật — không chép luật.
"""

from __future__ import annotations

import argparse
import bisect
import inspect
import json
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

# **KT-256 (L2, D2-188)** — phạm vi bảo vệ là **MỘT** định nghĩa sinh từ `cargo metadata`
# (`v2/tools/wake_scope.py`), dùng chung với lớp dep-info.
# **KT-407** — tệp nằm ở `v2/tools/checkers/wake/core.py` ⇒ `wake_scope.py` ở `parents[2]`.
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import wake_scope  # noqa: E402  (đường dẫn phải sửa trước khi import)

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
#
# **KT-209/L2** — reviewer L40 (KT-201 §D2 L2) dựng `v2/kernel/vifast-newcrate/src/lib.rs` và
# `v2/sim/vifast-sim2/src/lib.rs` chứa `m.wake(t)` ⇒ checker **XANH**; và
# `v2/kernel/vifast-core/build.rs` chứa `m.wake(t)` ⇒ **XANH** (chỉ `src/` được quét). Hợp đồng
# §1 lại nói "`v2/kernel/**`" — **sai** so với hành vi. Nay phạm vi là **mẫu**, không liệt kê
# cứng: **mọi** crate dưới `v2/kernel/<crate>/` và `v2/sim/<crate>/` — toàn bộ `src/**` **cộng**
# `build.rs` — kể cả crate ra đời sau. `tests/` cấp crate vẫn ngoài phạm vi (test-only, do
# `is_test_file` miễn; chúng không khớp mẫu này).
# **KT-242 (N4)** — `re.IGNORECASE`: trên macOS/Windows rustc nạp `src/x.RS` cho `mod x;`
# (đã đo: `cargo check` RC=0), nên **đuôi** phải so không phân biệt hoa/thường. `IGNORECASE`
# áp cả phần thư mục — chấp nhận được: chỉ **mở rộng** phạm vi (fail-closed), không thu hẹp.
# (Mẫu `RE_TEST_ONLY_WAKE_SCOPE` mang `IGNORECASE` đã **xoá** ở KT-256; phạm vi nay do
# `wake_scope` quyết định theo **crate**, không theo đuôi tệp — đuôi hoa/thường vẫn được duyệt
# bởi `_iter_rs_ci`.)
#
# **KT-256 (L2, D2-188)** — **xoá** mẫu `RE_TEST_ONLY_WAKE_SCOPE` + `_crate_root_relps`: phạm vi
# luật token nay là **view** `token_scope()` của **một** định nghĩa sinh từ `cargo metadata`
# (`v2/tools/wake_scope.py`), dùng chung với lớp dep-info. Hệ quả (đều **mở rộng**, đã đo 0 token
# `wake` nên không báo oan): tệp sản phẩm **ngoài `src/`** của crate ảnh sản phẩm (chương trình
# U-mode `vifast-kbin-riscv64/init/*.rs`, shim `vifast-plat-user/lib.rs`) vào phạm vi; crate thêm
# vào workspace ở thư mục gốc **mới** cũng vào phạm vi (mẫu cũ mù).
# **DEBT-KT227-1** — `v2/lib` **vẫn** ngoài luật token (chính sách: luật áp ảnh sản phẩm + sim);
# điều kiện làm nó an toàn được canh bằng `E_SCOPE_LIB_CORE` (`wake_scope.check_lib_core_dependency`).


def in_test_only_wake_scope(
    relp: str,
    root: Optional[Path] = None,
    scope: Optional["wake_scope.ProtectedScope"] = None,
) -> bool:
    """Tệp có thuộc phạm vi luật `wake()` chỉ-dùng-cho-test không (đường dẫn tương đối gốc).

    **KT-256 (L2)** — phạm vi là **view** `token_scope()` của định nghĩa duy nhất
    (`wake_scope.protected_scope`), sinh từ `cargo metadata` của `v2/Cargo.toml` +
    `v2/kernel/Cargo.toml` + bao đóng dependency nội bộ. Không truyền `root` ⇒ mẫu dự phòng cũ
    (giữ tương thích cho caller cũ/selftest). `scope` cho phép tầng gọi truyền phạm vi đã dựng
    (selftest chèn metadata, `scan_v2` dựng một lần).
    """
    if scope is not None:
        return scope.token_scope(relp)
    if root is None:
        return wake_scope.fallback_token_scope(relp)
    return wake_scope.protected_scope(root).token_scope(relp)


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
# một tệp miền của `kobj` (KT-336b) `let mut wake: Vec<ObjId>`). Biến cục bộ không phải hàm
# nên không báo oan.
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
# **KT-336b (D2-232/D2-238)** — `kobj.rs` đã tách thành **một bộ** tệp theo miền (mỗi tệp
# ≤ 1.500 dòng). Mọi mỏ neo/thoái lui của `kobj` phải kiểm trên **cả bộ**, không ghim một tệp:
# `fn wake(` + `block_current` nằm ở miền `sched`, còn `transition_tcb` **cấm** xuất hiện ở
# **bất kỳ** tệp nào. Thêm/bớt tệp miền ⇒ cập nhật danh sách này (checker fail-closed: thiếu
# tệp nào trong danh sách mà `fn wake(` không còn ở tệp nào khác ⇒ ĐỎ).
SIM_KOBJ_ROOT = "v2/sim/vifast-sim/src/kobj.rs"
SIM_KOBJ_FILES = (
    SIM_KOBJ_ROOT,
    "v2/sim/vifast-sim/src/kobj/budget.rs",
    "v2/sim/vifast-sim/src/kobj/caps.rs",
    "v2/sim/vifast-sim/src/kobj/clock.rs",
    "v2/sim/vifast-sim/src/kobj/fault.rs",
    "v2/sim/vifast-sim/src/kobj/ipc_ops.rs",
    "v2/sim/vifast-sim/src/kobj/notification.rs",
    "v2/sim/vifast-sim/src/kobj/object.rs",
    "v2/sim/vifast-sim/src/kobj/pagetable.rs",
    "v2/sim/vifast-sim/src/kobj/retype.rs",
    "v2/sim/vifast-sim/src/kobj/sched.rs",
    "v2/sim/vifast-sim/src/kobj/tcb.rs",
    "v2/sim/vifast-sim/src/kobj/tlb.rs",
    "v2/sim/vifast-sim/src/kobj/vspace.rs",
)
# Ghi chú: **KHÔNG** đưa `kobj/state_write.rs` vào danh sách này — đó là nơi `transition_tcb`
# **phải** nằm (KT-241), còn `SIM_KOBJ_FILES` là tập "tệp cũ không được còn `fn transition_tcb`".

# **KT-215** — tệp định nghĩa `MailboxMsg` được **miễn** luật 1/2 (kênh chuyển message): enum
# của nó khớp `RE_MAILBOX_MSG` nhưng không phải chỗ **phát**/xử lý. Trước KT-215 điều kiện là
# `p.name == "mailbox.rs"` — khớp **mọi** tệp cùng tên ở bất kỳ thư mục nào, nên một tệp
# `src/sub/mailbox.rs` chứa `post_mail(…, MailboxMsg::Wake(t))` được miễn luật 1 (kênh wake thứ
# hai). Nay so **đúng relpath** của hai tệp định nghĩa trong phạm vi quét (`v2/kernel`+`v2/sim`).
MAILBOX_DEFS = (KERNEL_MAILBOX, SIM_MAILBOX)

# **KT-241 (D2-194 §3)** — API thu hẹp: `transition_tcb` nhận `NonReadyTarget` (enum 10 biến
# thể, **không có `Ready`**), primitive ghi thô `write_state` là **private** trong module
# `state_write`, và `Ready` chỉ vào được qua **năm cửa ngữ nghĩa**. Hệ quả cho checker:
#
#   1. Đối số thứ hai của `transition_tcb` **không thể** là `Ready` (kiểu loại trừ) ⇒ **bỏ**
#      suy luận `_var_may_be_ready` và bỏ phân tích "đối số thứ hai có phải Ready không".
#      Còn lại **một** kiểm tối thiểu: đối số thứ hai là `TcbState::*` ⇒ dấu hiệu ai đó đã
#      **đảo kiểu** ⇒ ĐỎ (`E_STRICT_TRANSITION`).
#   2. `NON_WAKE_READY_SITES` (allowlist 15 mục "chuyển Ready không-wake") **xoá**: nó tồn tại
#      để miễn những chỗ `transition_tcb(…, TcbState::Ready)` hợp lệ **ngoài** cửa wake. Dưới
#      API mới **không còn chỗ nào như vậy** — `Ready` chỉ vào qua 5 cửa trong `state_write.rs`,
#      và mọi `TcbState::*` làm đối số `transition_tcb` đều là đảo kiểu. Giữ allowlist rỗng
#      hay allowlist cũ đều là **bề mặt tấn công** (một mục allowlist là một chỗ được miễn).
#   3. Luật thay thế trực tiếp: `write_state`/`into_state` **chỉ** được xuất hiện trong
#      `state_write.rs`; trong đó `write_state` **chỉ** được gọi bởi các hàm ở
#      `STATE_WRITE_CALLERS` (cửa thông thường + 5 cửa `Ready`). Ngoài ra ⇒ ĐỎ
#      (`E_STRICT_STATE_WRITE`).
#   4. **Mỏ neo kiểu** (fail-closed, `_check_type_anchors`): đọc **định nghĩa** thay vì tin
#      tên hàm — chữ ký `transition_tcb`/`block_current` phải là `NonReadyTarget`,
#      `NonReadyTarget` **không** có biến thể `Ready`, `write_state`/`into_state` **private**,
#      không `From`/`TryFrom`/hàm công khai nào chuyển `TcbState` → `NonReadyTarget`, và
#      `core_tcb.rs` / **cả bộ** tệp `kobj` (KT-336b) **không** còn `fn transition_tcb(`
#      (cửa cũ đã chuyển đi).
#
# Tệp `state_write.rs` của hai crate (API-SPEC §2.1/§2.2).
KERNEL_STATE_WRITE = "v2/kernel/vifast-core/src/state_write.rs"
SIM_STATE_WRITE = "v2/sim/vifast-sim/src/kobj/state_write.rs"
STATE_WRITE_FILES = (KERNEL_STATE_WRITE, SIM_STATE_WRITE)

# **Năm cửa `Ready`** (API-SPEC §3) + `tcb_yield` (cửa công khai gọi `yield_to_ready`).
# Mọi tên ở đây **phải** có `fn <tên>(` trong `state_write.rs` (mỏ neo `E_ANCHOR_MISSING`).
READY_DOORS = (
    "wake_step",
    "preempt_current",
    "yield_to_ready",
    "tcb_yield",
    "start_tcb",
    "suspend_to_ready",
)

# Hàm **được gọi trực tiếp** `write_state` trong `state_write.rs`:
#   - `transition_tcb` — **cửa thông thường** (door 0, API-SPEC §3): gọi
#     `write_state(tcb, to.into_state())`;
#   - 5 cửa `Ready` gọi `write_state(tcb, TcbState::Ready)`.
# `tcb_yield` **không** có mặt: nó phải đi qua `yield_to_ready` (có kiểm điều kiện nguồn);
# gọi thẳng `write_state` là bỏ kiểm ⇒ ĐỎ. Đây là điểm khác biệt **có chủ đích** so với
# `READY_DOORS` (một cửa ngữ nghĩa, hai mức ghi).
STATE_WRITE_CALLERS = (
    "transition_tcb",
    "wake_step",
    "preempt_current",
    "yield_to_ready",
    "start_tcb",
    "suspend_to_ready",
)

# Tên cũ `WAKE_DOORS` giữ nguyên (lớp 2 `import`), **đổi nghĩa**: tập `(tệp, hàm)` được phép
# gọi `write_state`. Khoá theo **tệp `state_write.rs`** để hàm trùng tên ở tệp khác không lọt.
WAKE_DOORS = {(f, fn) for f in STATE_WRITE_FILES for fn in STATE_WRITE_CALLERS}

# Tệp **cũ** chứa `transition_tcb` — sau KT-241 **không** được còn định nghĩa ở đây.
# Core: `core_tcb.rs:389` (API-SPEC §3.1); sim: **cả bộ** tệp `kobj` (KT-336b tách module —
# `transition_tcb` chỉ được nằm ở `kobj/state_write.rs`). `core_sched.rs` chỉ chứa
# `wake_step` (cũng đã chuyển đi) — không phải nơi định nghĩa `transition_tcb`.
KERNEL_TCB = "v2/kernel/vifast-core/src/core_tcb.rs"
LEGACY_TRANSITION_FILES = (KERNEL_TCB, *SIM_KOBJ_FILES)


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
# **KT-267 (b)** — miễn luật arch `doorbell` theo **ngữ nghĩa**, không theo **tên tệp**.
# Trước bản này: `p.name.startswith("virtio_")` ⇒ **bất kỳ** tệp `virtio_*.rs` nào (kể cả
# `virtio_evil.rs` **không** phải driver) được miễn. Nay: chỉ miễn khi tệp **thật sự** là driver
# virtio — dùng virtqueue của crate `vifast_virtio` (`SplitQueue`/`VqLayout`) hoặc định nghĩa
# kiểu `Virtio*`/`impl … VirtioKind`. Marker đọc trên **mã** (đã khử chú thích/chuỗi) nên
# **không** thể giả bằng comment.
RE_VIRTIO_DRIVER = re.compile(
    r"\b(?:SplitQueue|VqLayout)\b"
    r"|\b(?:struct|enum|trait|union)\s+Virtio[A-Za-z0-9_]*\b"
    r"|\bimpl\b[^\n{;]*\bVirtio[A-Za-z0-9_]*\b"
)
RE_ARCH_FORBIDDEN = re.compile(
    r"\b(?:TcbState|transition_tcb|MailboxMsg|wake_step|MailboxMsg::Wake)\b"
)


def _is_virtio_driver(text: str) -> bool:
    """**KT-267 (b)** — tệp có phải driver virtio **thật** không (kiểm **ngữ nghĩa**).

    Không dựa vào tên tệp: `virtio_evil.rs` không dùng virtqueue/kiểu virtio ⇒ **không** miễn.
    Marker đọc trên mã đã khử chú thích/chuỗi (`strip_rust_noise`) ⇒ comment không đánh lừa được.
    """
    return bool(RE_VIRTIO_DRIVER.search(strip_rust_noise(text)))


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


# ------------------------------------------------------------------ KT-194: lexer + token-tree
#
# **D2-166** — thay lớp so khớp **văn bản thô** bằng **lexer + token-tree + bộ đọc cấu trúc
# tối thiểu**. Mỗi token giữ **vị trí nguồn** (`file`, `line`, `col`, `start`, `end`) để:
#   - báo đúng `file:dòng`;
#   - phân biệt **lỗi từ vựng** (`E_LEX_*`) với **vi phạm chính sách** (`E_STRICT_*`);
#   - đối chiếu repro với chẩn đoán.
# Đầu vào không hiểu (chuỗi/comment chưa đóng, delimiter lệch, escape sai, quá sâu) ⇒ **lỗi**,
# không trả về tập token/cạnh thiếu rồi tiếp tục xanh (D2-166 §3C).

MAX_TREE_DEPTH = 128
MAX_ATTR_DEPTH = 32


class LexError(Exception):
    """Lỗi từ vựng/cấu trúc — **không** phải vi phạm chính sách."""

    __slots__ = ("relp", "line", "col", "code", "msg")

    def __init__(self, relp: str, line: int, col: int, code: str, msg: str) -> None:
        self.relp = relp
        self.line = line
        self.col = col
        self.code = code
        self.msg = msg
        super().__init__(f"{relp}:{line}:{col}: {code} {msg}")


class Tok:
    """Một token Rust kèm **vị trí nguồn** (1-based line/col; offset byte trong tệp)."""

    __slots__ = ("kind", "val", "raw", "file", "line", "col", "start", "end")

    def __init__(
        self,
        kind: str,
        val: str,
        raw: object,
        file: str,
        line: int,
        col: int,
        start: int,
        end: int,
    ) -> None:
        self.kind = kind
        self.val = val
        self.raw = raw
        self.file = file
        self.line = line
        self.col = col
        self.start = start
        self.end = end

    def __repr__(self) -> str:  # pragma: no cover - chẩn đoán
        return f"Tok({self.kind},{self.val!r}@{self.line}:{self.col})"


class Group:
    """Nhóm ngoặc lồng nhau (token-tree)."""

    __slots__ = ("open", "close", "children")

    def __init__(self, open_tok: Tok, close_tok: Optional[Tok], children: List[object]) -> None:
        self.open = open_tok
        self.close = close_tok
        self.children = children

    def __repr__(self) -> str:  # pragma: no cover - chẩn đoán
        return f"Group({self.open.val}…{len(self.children)} con)"


class Attr:
    """Thuộc tính `#[…]` đã đọc theo **cấu trúc** (tên + token đối số)."""

    __slots__ = ("name", "tok", "args", "group")

    def __init__(self, name: str, tok: Tok, args: List[object], group: Optional[Group]) -> None:
        self.name = name
        self.tok = tok
        self.args = args
        self.group = group


# Token hoá bằng **một** regex chính (C-speed) + xử lý tay cho chú thích khối lồng nhau,
# chuỗi thô và giải mã escape. Thứ tự nhánh là quan trọng: `rawstr` trước `rawident`,
# `bytestr` trước `ident`, `char` trước `lifetime`.
_RE_LEX = re.compile(
    r"""
    (?P<ws>[ \t\r\n\f\v]+)
  | (?P<line>//[^\n]*)
  | (?P<block>/\*)
  | (?P<rawstr>(?:b|c)?r(?P<rh>\#*)"(?P<rb>.*?)"(?P=rh))
  | (?P<bytestr>[bc]"(?:[^"\\]|\\.)*")
  | (?P<str>"(?:[^"\\]|\\.)*")
  | (?P<char>'(?:[^'\\]|\\x[0-9A-Fa-f]{2}|\\u\{[0-9A-Fa-f]{1,6}\}|\\[nrt\\0'"]|\\)')
  | (?P<lifetime>'[^\W\d]\w*)
  | (?P<rawident>r\#[^\W\d]\w*)
  | (?P<ident>[^\W\d]\w*)
  | (?P<num>[0-9][0-9A-Za-z_]*(?:\.[0-9][0-9A-Za-z_]*)?)
  | (?P<punct>.)
    """,
    re.X | re.S | re.UNICODE,
)
_RE_RAW_START = re.compile(r'(?:b|c)?r(#*)"')
_SIMPLE_ESC = {"n": "\n", "r": "\r", "t": "\t", "\\": "\\", "0": "\0", "'": "'", '"': '"'}
_HEX = "0123456789abcdefABCDEF"
_PAIRS = {"(": ")", "[": "]", "{": "}"}


def _decode_string(
    body: str, relp: str, line: int, col: int, byte: bool = False
) -> Tuple[Optional[str], Optional[LexError]]:
    """Giải mã escape của chuỗi Rust; escape sai/không hỗ trợ ⇒ **lỗi** (D2-166 §3C)."""
    out: List[str] = []
    i = 0
    n = len(body)
    while i < n:
        c = body[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        if i + 1 >= n:
            return None, LexError(relp, line, col, "E_LEX_INVALID_ESCAPE", "escape ở cuối chuỗi")
        e = body[i + 1]
        if e in _SIMPLE_ESC:
            out.append(_SIMPLE_ESC[e])
            i += 2
            continue
        if e == "x":
            h = body[i + 2 : i + 4]
            if len(h) != 2 or any(ch not in _HEX for ch in h):
                return None, LexError(
                    relp, line, col, "E_LEX_INVALID_ESCAPE", "`\\x` cần đúng 2 chữ số hex"
                )
            out.append(chr(int(h, 16)))
            i += 4
            continue
        if e == "u" and not byte:
            if i + 2 >= n or body[i + 2] != "{":
                return None, LexError(
                    relp, line, col, "E_LEX_INVALID_ESCAPE", "`\\u` cần dạng `\\u{…}`"
                )
            j = body.find("}", i + 3)
            if j < 0:
                return None, LexError(
                    relp, line, col, "E_LEX_INVALID_ESCAPE", "`\\u{…}` chưa đóng"
                )
            h = body[i + 3 : j]
            if not h or len(h) > 6 or any(ch not in _HEX for ch in h):
                return None, LexError(
                    relp, line, col, "E_LEX_INVALID_ESCAPE", "`\\u{…}` không hợp lệ"
                )
            cp = int(h, 16)
            if cp > 0x10FFFF or 0xD800 <= cp <= 0xDFFF:
                return None, LexError(
                    relp, line, col, "E_LEX_INVALID_ESCAPE", "`\\u{…}` ngoài Unicode"
                )
            out.append(chr(cp))
            i = j + 1
            continue
        if e == "\n":
            i += 2
            while i < n and body[i] in " \t\r\n":
                i += 1
            continue
        if e == "\r" and i + 2 < n and body[i + 2] == "\n":
            i += 3
            while i < n and body[i] in " \t\r\n":
                i += 1
            continue
        return None, LexError(
            relp, line, col, "E_LEX_INVALID_ESCAPE", f"escape không hỗ trợ `\\{e}`"
        )
    return "".join(out), None


def tokenize(text: str, relp: str) -> Tuple[List[Tok], List[LexError]]:
    """Token hoá **đầy đủ** một tệp Rust; trả `(token, lỗi từ vựng)`.

    Không bao giờ "đoán" khi gặp đầu vào hỏng: chuỗi/comment chưa đóng, escape sai, ký tự
    không nhận dạng ⇒ ghi lỗi và dừng (không trả token thiếu rồi tiếp tục xanh).
    """
    toks: List[Tok] = []
    errs: List[LexError] = []
    pos = 0
    line = 1
    col = 1
    n = len(text)
    while pos < n:
        m = _RE_LEX.match(text, pos)
        if m is None:  # pragma: no cover - `punct` nuốt mọi ký tự
            errs.append(
                LexError(relp, line, col, "E_LEX_UNKNOWN", f"ký tự không nhận dạng: {text[pos]!r}")
            )
            break
        kind = m.lastgroup or ""
        seg = m.group(0)
        start_line, start_col = line, col
        end = m.end()
        # --- chuỗi thô: regex đã khớp trọn; nếu có mở `r#*"` mà không khớp ⇒ chưa đóng.
        if kind != "rawstr" and _RE_RAW_START.match(text, pos):
            errs.append(
                LexError(
                    relp,
                    start_line,
                    start_col,
                    "E_LEX_UNTERMINATED_STRING",
                    "chuỗi thô `r#\"…\"#` chưa đóng hoặc sai số `#`",
                )
            )
            break
        if kind == "ws" or kind == "line":
            pass
        elif kind == "block":
            depth = 0
            j = pos
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
            if depth != 0:
                errs.append(
                    LexError(
                        relp,
                        start_line,
                        start_col,
                        "E_LEX_UNTERMINATED_COMMENT",
                        "chú thích khối `/*` chưa đóng",
                    )
                )
                break
            seg = text[pos:j]
            end = j
        elif kind == "rawstr":
            toks.append(
                Tok("str", m.group("rb"), seg, relp, start_line, start_col, pos, end)
            )
        elif kind == "bytestr":
            val, err = _decode_string(seg[2:-1], relp, start_line, start_col, byte=True)
            if err is not None:
                errs.append(err)
                break
            toks.append(Tok("bytestr", val or "", seg, relp, start_line, start_col, pos, end))
        elif kind == "str":
            val, err = _decode_string(seg[1:-1], relp, start_line, start_col)
            if err is not None:
                errs.append(err)
                break
            toks.append(Tok("str", val or "", seg, relp, start_line, start_col, pos, end))
        elif kind == "char":
            _, err = _decode_string(seg[1:-1], relp, start_line, start_col)
            if err is not None:
                errs.append(err)
                break
            toks.append(Tok("char", seg, seg, relp, start_line, start_col, pos, end))
        elif kind == "lifetime":
            toks.append(Tok("lifetime", seg[1:], seg, relp, start_line, start_col, pos, end))
        elif kind == "rawident":
            toks.append(Tok("ident", seg[2:], True, relp, start_line, start_col, pos, end))
        elif kind == "ident":
            toks.append(Tok("ident", seg, False, relp, start_line, start_col, pos, end))
        elif kind == "num":
            toks.append(Tok("num", seg, seg, relp, start_line, start_col, pos, end))
        elif kind == "punct":
            ch = seg
            if ch in "([{":
                toks.append(Tok("open", ch, ch, relp, start_line, start_col, pos, end))
            elif ch in ")]}":
                toks.append(Tok("close", ch, ch, relp, start_line, start_col, pos, end))
            elif ch == '"':
                errs.append(
                    LexError(
                        relp,
                        start_line,
                        start_col,
                        "E_LEX_UNTERMINATED_STRING",
                        "chuỗi chưa đóng",
                    )
                )
                break
            elif ch == "'":
                errs.append(
                    LexError(
                        relp,
                        start_line,
                        start_col,
                        "E_LEX_UNTERMINATED_CHAR",
                        "literal ký tự/lifetime không hợp lệ",
                    )
                )
                break
            else:
                toks.append(Tok("punct", ch, ch, relp, start_line, start_col, pos, end))
        # cập nhật dòng/cột theo đoạn đã tiêu thụ
        nl = seg.count("\n")
        if nl:
            line += nl
            col = len(seg) - seg.rfind("\n")
        else:
            col += len(seg)
        pos = end
    return toks, errs


def build_tree(tokens: Sequence[Tok], relp: str) -> List[object]:
    """Dựng token-tree; delimiter lệch/quá sâu ⇒ `LexError` (không bỏ qua)."""
    root: List[object] = []
    stack: List[Tuple[Optional[Group], List[object]]] = [(None, root)]
    for t in tokens:
        if t.kind == "open":
            g = Group(t, None, [])
            stack[-1][1].append(g)
            stack.append((g, g.children))
            if len(stack) > MAX_TREE_DEPTH:
                raise LexError(
                    relp,
                    t.line,
                    t.col,
                    "E_LEX_DEPTH",
                    f"lồng ngoặc quá {MAX_TREE_DEPTH} tầng",
                )
        elif t.kind == "close":
            if len(stack) == 1:
                raise LexError(
                    relp, t.line, t.col, "E_LEX_UNBALANCED", f"`{t.val}` đóng mà không có ngoặc mở"
                )
            g = stack.pop()[0]
            assert g is not None
            if _PAIRS[g.open.val] != t.val:
                raise LexError(
                    relp,
                    t.line,
                    t.col,
                    "E_LEX_UNBALANCED",
                    f"`{g.open.val}` mở tại {g.open.line}:{g.open.col} nhưng đóng bằng `{t.val}`",
                )
            g.close = t
        else:
            stack[-1][1].append(t)
    if len(stack) != 1:
        g = stack[-1][0]
        assert g is not None
        raise LexError(
            relp,
            g.open.line,
            g.open.col,
            "E_LEX_UNBALANCED",
            f"`{g.open.val}` chưa đóng",
        )
    return root


_PARSE_CACHE: Dict[Tuple[str, int, int], Tuple[Optional[List[object]], List[LexError]]] = {}


def parse_file(p: Path) -> Tuple[Optional[List[object]], List[LexError]]:
    """Đọc + token hoá + dựng cây một tệp; cache theo `(đường dẫn, mtime, kích thước)`."""
    try:
        st = p.stat()
    except OSError:
        return None, []
    key = (str(p), st.st_mtime_ns, st.st_size)
    cached = _PARSE_CACHE.get(key)
    if cached is not None:
        return cached
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None, []
    toks, errs = tokenize(text, str(p))
    tree: Optional[List[object]] = None
    if not errs:
        try:
            tree = build_tree(toks, str(p))
        except LexError as e:
            errs.append(e)
    res = (tree, errs)
    _PARSE_CACHE[key] = res
    return res


def _is_punct(n: object, val: str) -> bool:
    return isinstance(n, Tok) and n.kind == "punct" and n.val == val


def _is_ident(n: object, val: Optional[str] = None) -> bool:
    return isinstance(n, Tok) and n.kind == "ident" and (val is None or n.val == val)


def _iter_tokens(nodes: Sequence[object]) -> Iterable[Tok]:
    for n in nodes:
        if isinstance(n, Group):
            yield from _iter_tokens(n.children)
        else:
            assert isinstance(n, Tok)
            yield n


def _split_commas(nodes: Sequence[object]) -> List[List[object]]:
    parts: List[List[object]] = []
    cur: List[object] = []
    for n in nodes:
        if _is_punct(n, ","):
            parts.append(cur)
            cur = []
        else:
            cur.append(n)
    parts.append(cur)
    return parts


def parse_attr(g: Group, relp: str) -> Attr:
    """Đọc tên thuộc tính theo **cấu trúc**; tên không phải định danh ⇒ lỗi cấu trúc."""
    if not (isinstance(g, Group) and g.open.val == "["):
        raise LexError(relp, g.open.line, g.open.col, "E_LEX_ATTR", "thuộc tính không mở bằng `[`")
    ch = g.children
    if not ch or not isinstance(ch[0], Tok) or ch[0].kind != "ident":
        raise LexError(
            relp, g.open.line, g.open.col, "E_LEX_ATTR", "tên thuộc tính không phải định danh"
        )
    return Attr(ch[0].val, ch[0], ch[1:], g)


def _attr_from_tokens(nodes: Sequence[object], relp: str) -> Tuple[Optional[Attr], List[Tuple[object, str]]]:
    """Đọc một nhánh thuộc tính trong `cfg_attr(…, <nhánh>)`."""
    if not nodes:
        return None, [(None, "nhánh thuộc tính rỗng")]
    first = nodes[0]
    if isinstance(first, Tok) and first.kind == "ident":
        return Attr(first.val, first, list(nodes[1:]), None), []
    tok = first if isinstance(first, Tok) else first.open
    return None, [(tok, "tên thuộc tính trong `cfg_attr` không phải định danh — không phân giải được")]


def attr_path_edges(
    attr: Attr, relp: str, depth: int = 0
) -> Tuple[List[Tuple[str, bool, Tok]], List[Tuple[object, str]]]:
    """Cạnh `path` của một thuộc tính.

    Trả `(cạnh, vấn_đề)`:
      - cạnh = `(giá_trị_đã_giải_mã, chắc_chắn?, token)`;
      - `chắc_chắn=True` cho `#[path = …]`, `False` cho `cfg_attr(…, path = …)`;
      - vấn đề = `(token, thông_điệp)` cho dạng **không phân giải được** (fail-closed).
    `cfg_attr` **lồng** được đệ quy có giới hạn; quá `MAX_ATTR_DEPTH` ⇒ `LexError`.
    """
    edges: List[Tuple[str, bool, Tok]] = []
    problems: List[Tuple[object, str]] = []
    if depth > MAX_ATTR_DEPTH:
        raise LexError(
            relp,
            attr.tok.line,
            attr.tok.col,
            "E_LEX_DEPTH",
            f"`cfg_attr` lồng quá {MAX_ATTR_DEPTH} tầng",
        )
    if attr.name == "path":
        rest = attr.args
        if (
            len(rest) == 2
            and _is_punct(rest[0], "=")
            and isinstance(rest[1], Tok)
            and rest[1].kind == "str"
        ):
            edges.append((rest[1].val, True, rest[1]))
        else:
            problems.append(
                (attr.tok, "`path` không phải literal chuỗi — không phân giải được đích")
            )
        return edges, problems
    if attr.name == "cfg_attr":
        # `cfg_attr(…)`: đối số nằm trong **một** nhóm `( … )`.
        args = attr.args
        if len(args) == 1 and isinstance(args[0], Group) and args[0].open.val == "(":
            args = args[0].children
        parts = _split_commas(args)
        if len(parts) < 2:
            problems.append((attr.tok, "`cfg_attr` thiếu nhánh thuộc tính"))
            return edges, problems
        for part in parts[1:]:
            sub, sub_problems = _attr_from_tokens(part, relp)
            problems.extend(sub_problems)
            if sub is None:
                continue
            e2, p2 = attr_path_edges(sub, relp, depth + 1)
            edges.extend(e2)
            problems.extend(p2)
        return edges, problems
    return edges, problems


def cfg_is_test_tokens(nodes: Sequence[object], ktest_is_test: bool = False) -> bool:
    """`#[cfg(…)]` có phải ngữ cảnh **test** không (chỉ nhận dạng chắc chắn).

    **KT-227 P0-4** — `#[cfg(feature = "ktest")]` **không** còn được coi là test theo mặc định.
    Vì sao: đường lọt B3b của KT-224 — `#[cfg(feature = "ktest")] mod evil;` + tệp riêng chứa
    `m.wake(t)` lọt **cả hai** lớp: lớp 2 không nạp (feature tắt), lớp 1 miễn vì coi module là
    test-only. Không có bằng chứng `ktest` **không bao giờ** bật trong build sản phẩm (nó là
    feature của `vifast-kbin-riscv64`/`vifast-plat-riscv64`) ⇒ **fail-closed**: coi là sản phẩm.
    `ktest_is_test=True` chỉ dùng cho ngữ cảnh **khối** (`_test_char_ranges`) nếu cần giữ miễn
    theo khối; hiện không nơi nào bật (đo 0 báo oan trên cây thật).
    """
    # `#[cfg(test)]`: vị từ nằm trong **một** nhóm `( … )`.
    if len(nodes) == 1 and isinstance(nodes[0], Group) and nodes[0].open.val == "(":
        nodes = nodes[0].children
    nodes = [n for n in nodes if not _is_punct(n, ",")]
    if len(nodes) == 1 and _is_ident(nodes[0], "test"):
        return True
    if (
        ktest_is_test
        and len(nodes) == 3
        and _is_ident(nodes[0], "feature")
        and _is_punct(nodes[1], "=")
        and isinstance(nodes[2], Tok)
        and nodes[2].kind == "str"
        and nodes[2].val == "ktest"
    ):
        return True
    if (
        len(nodes) == 2
        and _is_ident(nodes[0], "all")
        and isinstance(nodes[1], Group)
        and nodes[1].open.val == "("
    ):
        parts = _split_commas(nodes[1].children)
        return bool(parts) and all(cfg_is_test_tokens(p, ktest_is_test) for p in parts)
    return False


def _attrs_test_context(attrs: Sequence[Attr], line: int, tr: Sequence[Tuple[int, int]]) -> bool:
    """`mod` có nằm trong ngữ cảnh test không: trong khối test, hoặc có `#[cfg(test)]` trực tiếp."""
    if in_ranges(line, tr):
        return True
    for a in attrs:
        if a.name == "cfg" and cfg_is_test_tokens(a.args):
            return True
    return False


def _safe_resolve(base: Path, raw: str) -> Optional[Path]:
    """`(base / raw).resolve()` — trả `None` khi đường dẫn **không phân giải được**.

    **KT-242 (N6)** — `Path.resolve()` ném `ValueError` khi đường dẫn chứa NUL
    (`include!("\\0")`) và `OSError` khi quá dài / vòng symlink. Trước bản này lỗi đó **thoát
    ra ngoài** thành traceback ⇒ vi phạm D2-166 §3C ("**không âm thầm bỏ qua** … ⇒ lỗi **rõ** +
    exit khác 0"): exit khác 0 nhưng **không rõ** và không có mã lỗi. Nay trả `None` để caller
    phát lỗi cổng có **vị trí nguồn** (`E_STRICT_WAKE_INCLUDE`).
    """
    try:
        return (base / raw).resolve()
    except (ValueError, OSError):
        return None


def _walk_items(
    nodes: Sequence[object],
    relp: str,
    cur_dir: Path,
    on_attr: Optional[object] = None,
    on_mod: Optional[object] = None,
    on_macro: Optional[object] = None,
    on_include: Optional[object] = None,
    errors: Optional[List[LexError]] = None,
    depth: int = 0,
    in_test: bool = False,
    mod_dir: Optional[Path] = None,
) -> None:
    """Duyệt **cấu trúc** mức mục: thuộc tính, `mod`, `macro_rules!`, `include!`.

    - Chỉ đệ quy vào nhóm `{ … }` (thân khối/inline module); nhóm đứng sau `!` là **lời gọi
      macro** ⇒ bỏ qua (token trong lời gọi không phải mục Rust cho tới khi mở rộng). **KT-202**
      quét đối số lời gọi bằng `_scan_macro_calls` (đệ quy **mọi** nhóm) — không dùng
      `_walk_items` vì nó chỉ đệ quy `{…}` nên bỏ sót lời gọi lồng trong `( … )`/`[…]`.
    - `macro_rules!` ⇒ gọi `on_macro` với **thân** rồi bỏ qua thân (luật macro xử lý riêng).
    - `mod NAME;` / `mod NAME { … }` ⇒ gọi `on_mod`; inline module đệ quy với thư mục con.

    **KT-213** — `cur_dir` là gốc của **mức hiện tại** (`#[path]` ở mức mục giải theo đây);
    `mod_dir` là **thư mục module của tệp** (chỉ khác `cur_dir` ở mức gốc của tệp non-mod-rs:
    `src/foo.rs` ⇒ `src/foo/`). Khi vào **inline module không có `#[path]`**, thư mục con là
    `mod_dir/NAME` — đúng Rust Reference ("path starts with a directory with the name of the
    non-mod-rs module"). Trước KT-213 dùng `cur_dir/NAME` nên `#[path]` trong inline module của
    tệp non-mod-rs lệch một tầng `foo/` so với rustc (đường lọt #11).
    """
    if mod_dir is None:
        mod_dir = cur_dir
    if depth > MAX_TREE_DEPTH:
        if errors is not None:
            errors.append(LexError(relp, 1, 1, "E_LEX_DEPTH", "cây token quá sâu"))
        return
    pending: List[Attr] = []
    i = 0
    n = len(nodes)
    while i < n:
        node = nodes[i]
        if isinstance(node, Group):
            prev = nodes[i - 1] if i > 0 else None
            if _is_punct(prev, "!"):
                i += 1
                pending = []
                continue
            if node.open.val == "{":
                _walk_items(
                    node.children,
                    relp,
                    cur_dir,
                    on_attr,
                    on_mod,
                    on_macro,
                    on_include,
                    errors,
                    depth + 1,
                    in_test,
                    mod_dir,
                )
            i += 1
            pending = []
            continue
        assert isinstance(node, Tok)
        if node.kind == "punct" and node.val == "#":
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
            if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "[":
                try:
                    attr = parse_attr(nodes[j], relp)
                except LexError as e:
                    if errors is not None:
                        errors.append(e)
                    i = j + 1
                    pending = []
                    continue
                pending.append(attr)
                if on_attr is not None:
                    on_attr(attr, cur_dir)
                i = j + 1
                continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "macro_rules":
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
            if j < n and isinstance(nodes[j], Tok) and nodes[j].kind == "ident":
                name_tok = nodes[j]
                j += 1
                if j < n and isinstance(nodes[j], Group):
                    if on_macro is not None:
                        on_macro(name_tok, nodes[j], cur_dir)
                    i = j + 1
                    pending = []
                    continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "include":
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
                if j < n and isinstance(nodes[j], Group):
                    if on_include is not None:
                        on_include(nodes[j], cur_dir)
                    i = j + 1
                    pending = []
                    continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "mod":
            j = i + 1
            if j < n and isinstance(nodes[j], Tok) and nodes[j].kind == "ident":
                name_tok = nodes[j]
                j += 1
                if j < n and _is_punct(nodes[j], ";"):
                    if on_mod is not None:
                        on_mod(name_tok, pending, False, None, cur_dir, in_test, mod_dir)
                    i = j + 1
                    pending = []
                    continue
                if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "{":
                    # **KT-196/F2**: ngữ cảnh test theo **item/token**, không theo dòng.
                    _mod_test = in_test or any(
                        a.name == "cfg" and cfg_is_test_tokens(a.args) for a in pending
                    )
                    if on_mod is not None:
                        on_mod(name_tok, pending, True, nodes[j], cur_dir, in_test, mod_dir)
                    # **KT-199/F4b** (reviewer L35): inline module có `#[path = "sub"]` thì
                    # thư mục con là `sub`, KHÔNG phải `cur_dir/NAME` (rustc dùng `#[path]`).
                    # **KT-213** — không có `#[path]` thì thư mục con là `mod_dir/NAME` (gốc
                    # module của **tệp**), không phải `cur_dir/NAME`: với tệp non-mod-rs
                    # `src/foo.rs`, rustc dùng `src/foo/NAME/`, còn `cur_dir` mức gốc là `src/`.
                    _child_dir = mod_dir / name_tok.val
                    for _a in pending:
                        try:
                            _edges, _ = attr_path_edges(_a, relp)
                        except LexError:
                            continue
                        if _edges:
                            # **KT-242 (N6)** — `_safe_resolve`: đường dẫn không phân giải
                            # được (NUL/quá dài) ⇒ giữ `mod_dir/NAME` thay vì ném ra ngoài.
                            _res = _safe_resolve(cur_dir, _edges[0][0])
                            if _res is not None:
                                _child_dir = _res
                            break
                    _walk_items(
                        nodes[j].children,
                        relp,
                        _child_dir,
                        on_attr,
                        on_mod,
                        on_macro,
                        on_include,
                        errors,
                        depth + 1,
                        _mod_test,
                        _child_dir,
                    )
                    i = j + 1
                    pending = []
                    continue
            if errors is not None:
                errors.append(
                    LexError(
                        relp,
                        node.line,
                        node.col,
                        "E_LEX_STRUCT",
                        "khai báo `mod` không có định danh hợp lệ — không phân giải được cạnh nguồn",
                    )
                )
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "pub":
            j = i + 1
            if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "(":
                j += 1
            i = j
            continue
        pending = []
        i += 1


def _walk_all(
    nodes: Sequence[object],
    relp: str,
    cur_dir: Path,
    on_include: object,
    on_path_attr: object,
    inline_path_toks: set,
    depth: int = 0,
    mod_dir: Optional[Path] = None,
    in_macro_args: bool = False,
) -> None:
    """**KT-211** (reviewer L44, KT-208 §B1) — quét **mọi** token của tệp, **mọi** tầng nhóm,
    **mọi** loại nhóm `{`/`(`/`[`, tìm `include!` và `#[path]`.

    Vì sao **không** dùng `_walk_items`: nó chỉ đệ quy nhóm `{ … }` (để không nhầm `mod` trong
    biểu thức thành khai báo module). Hệ quả đo được (reviewer L44): `include!` ở **vị trí
    biểu thức** trong `( … )`/`[ … ]` — `sink(include!{…})`, `[include!{…}]`,
    `(0u8, include!{…})`, `(include!{…})`, `(|| include!{…})`, `Some(include!{…})` — **không
    bao giờ** được `on_include` gọi ⇒ đích không phân giải, không bị từ chối; `cargo check`
    RC=0, checker `errors=0` **cả hai** chế độ. Cùng lỗ với `#[path]` trên `mod` trong khối
    lồng `( … )` (`({ #[path = "…"] mod m; 1 })` — `cargo check` RC=0, checker XANH).

    Khác `_walk_items`:
      - đệ quy **mọi** nhóm (kể cả nhóm sau `!` — lời gọi macro; `_check_macro_passthrough`
        vẫn từ chối `include!`/`#[path]`/`mod` trong **đối số** lời gọi macro, nên đây là
        lớp **thêm**, không thay thế);
      - **bỏ qua** thân `macro_rules!` (đã bị `_check_macro_body` từ chối riêng — tránh báo
        trùng và tránh phân giải sai theo tệp định nghĩa);
      - `mod` **không** được xử lý như khai báo module ở đây (chỉ có nghĩa ở mức item —
        `_walk_items` giữ đường cấu trúc); `cur_dir` **chỉ** đổi khi vào thân **inline
        module** để `#[path]` lồng phân giải đúng như rustc (đã kiểm bằng `rustc`:
        `#[path = "sub"] mod m { #[path = "y.rs"] mod n; }` ⇒ `sub/y.rs`).

    `include!` phân giải theo **thư mục tệp chứa** (callback `on_include` capture `p.parent`),
    **không** theo `cur_dir` — đúng ngữ nghĩa rustc (đã kiểm: `mod m { include!("x.rs") }` ⇒
    `x.rs` cạnh tệp gốc).

    **KT-213** — `cur_dir` là gốc của **mức hiện tại** (`#[path]` ở mức mục giải theo đây);
    `mod_dir` là **thư mục module của tệp** (chỉ khác `cur_dir` ở mức gốc của tệp non-mod-rs:
    `src/foo.rs` ⇒ `src/foo/`). Inline module **không** `#[path]` ⇒ thư mục con là
    `mod_dir/NAME` (đúng Rust Reference); inline module **có** `#[path]` ⇒ `cur_dir/<path>`.

    **KT-225** (reviewer L52 KT-223 — bốn báo oan) — hai luật nhận dạng:
      - `include` + `!` **chỉ** là `include!` khi sau `!` là **nhóm phân cách** `{`/`(`/`[`
        hoặc **literal chuỗi** (cú pháp macro hợp lệ). `include != 0` có `!` + `=` ⇒ **không**
        phải lời gọi macro (trước KT-225 bị ĐỎ oan).
      - `in_macro_args=True` khi đang ở trong **đối số lời gọi macro** (nhóm sau `!`): token
        bên trong là **token stream**, không phải mã cho tới khi macro mở rộng. Không sinh
        cạnh `include!`/`#[path]`/`mod` ở đây — `_check_macro_passthrough` xử lý riêng
        (fail-closed cho macro **không** an toàn; `stringify!`/`concat!` được miễn vì không
        mở rộng đối số thành mã). Trước KT-225 `stringify!(include!)` bị ĐỎ oan.
    """
    if mod_dir is None:
        mod_dir = cur_dir
    if depth > MAX_TREE_DEPTH:
        return
    pending: List[Attr] = []
    i = 0
    n = len(nodes)
    while i < n:
        node = nodes[i]
        if isinstance(node, Group):
            # **KT-211** — đệ quy **mọi** nhóm, kể cả nhóm sau `!` (lời gọi macro) và nhóm
            # `( … )`/`[ … ]`. Thân `macro_rules!` đã bị nhánh `macro_rules` bên dưới bỏ qua.
            # **KT-225** — nhóm sau `!` là **đối số lời gọi macro**: đánh dấu `in_macro_args`
            # để **không** sinh cạnh `include!`/`#[path]`/`mod` từ token stream (báo oan
            # `stringify!(include!)`); `_check_macro_passthrough` xử lý riêng.
            _prev = nodes[i - 1] if i > 0 else None
            _walk_all(
                node.children,
                relp,
                cur_dir,
                on_include,
                on_path_attr,
                inline_path_toks,
                depth + 1,
                mod_dir,
                in_macro_args or _is_punct(_prev, "!"),
            )
            i += 1
            pending = []
            continue
        assert isinstance(node, Tok)
        if node.kind == "punct" and node.val == "#":
            if in_macro_args:
                # **KT-225** — thuộc tính trong đối số lời gọi macro là token stream;
                # `_check_macro_passthrough` xử lý riêng (trừ `stringify!`/`concat!`).
                pending = []
                i += 1
                continue
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
            if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "[":
                try:
                    attr = parse_attr(nodes[j], relp)
                except LexError:
                    # Lỗi từ vựng/cấu trúc đã được `scan_lexical`/`scan_structure` báo.
                    i = j + 1
                    pending = []
                    continue
                pending.append(attr)
                on_path_attr(attr, cur_dir)
                i = j + 1
                continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "macro_rules":
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
            if j < n and isinstance(nodes[j], Tok) and nodes[j].kind == "ident":
                j += 1
                if j < n and isinstance(nodes[j], Group):
                    # **Bỏ qua thân `macro_rules!`** — `_check_macro_body` từ chối
                    # `include!`/`#[path]`/`mod` trong thân (fail-closed) và `include!` trong
                    # thân giải theo **tệp gọi**, không phân giải tĩnh được.
                    i = j + 1
                    pending = []
                    continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "include":
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
                # **KT-225** — chỉ coi là `include!` khi cú pháp macro hợp lệ: sau `!` là
                # nhóm phân cách `{`/`(`/`[` hoặc literal chuỗi. `include != 0` có `!` + `=`
                # ⇒ **không** phải lời gọi macro (trước KT-225 bị ĐỎ oan).
                if j < n and (
                    isinstance(nodes[j], Group)
                    or (isinstance(nodes[j], Tok) and nodes[j].kind == "str")
                ):
                    if not in_macro_args:
                        on_include(nodes, j, node)
                    # Xử lý tiếp **đối số** (nhóm) ở vòng lặp — `include!` lồng vẫn được thấy.
                    i = j
                    pending = []
                    continue
                pending = []
                i = j
                continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "mod":
            if in_macro_args:
                # **KT-225** — `mod` trong đối số lời gọi macro là token stream;
                # `_check_macro_passthrough` xử lý riêng (trừ `stringify!`/`concat!`).
                pending = []
                i += 1
                continue
            j = i + 1
            if j < n and isinstance(nodes[j], Tok) and nodes[j].kind == "ident":
                name_tok = nodes[j]
                j += 1
                if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "{":
                    # Inline module: `cur_dir` đổi theo `#[path]` (rustc) — KT-199/F4b.
                    # **KT-213** — không có `#[path]` thì thư mục con là `mod_dir/NAME` (gốc
                    # module của **tệp**), không phải `cur_dir/NAME`: với tệp non-mod-rs
                    # `src/foo.rs`, rustc dùng `src/foo/NAME/`, còn `cur_dir` mức gốc là `src/`.
                    child_dir = mod_dir / name_tok.val
                    _child_set = False
                    for _a in pending:
                        try:
                            _edges, _ = attr_path_edges(_a, relp)
                        except LexError:
                            continue
                        for _val, _certain, _tok in _edges:
                            inline_path_toks.add(_tok.start)
                        if _edges and not _child_set:
                            # **KT-242 (N6)** — `_safe_resolve` (xem `_walk_items`).
                            _res = _safe_resolve(cur_dir, _edges[0][0])
                            if _res is not None:
                                child_dir = _res
                            _child_set = True
                    _walk_all(
                        nodes[j].children,
                        relp,
                        child_dir,
                        on_include,
                        on_path_attr,
                        inline_path_toks,
                        depth + 1,
                        child_dir,
                    )
                    i = j + 1
                    pending = []
                    continue
            pending = []
            i += 1
            continue
        if node.kind == "ident" and node.val == "pub":
            j = i + 1
            if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "(":
                j += 1
            i = j
            continue
        pending = []
        i += 1


def _is_build_dir(root: Path, p: Path) -> bool:
    """`p` nằm trong thư mục **build của cargo** (`target/`) ⇒ bỏ qua khi quét.

    **KT-215** (reviewer L47 KT-212 — đường lọt #12) — trước bản này cả **năm** chỗ quét dùng
    `"target" in p.parts`, khớp **bất kỳ** thành phần `target` nào, kể cả `src/target/`. Hệ quả:
    `mod target;` + `src/target/evil.rs` chứa `wake` **biên dịch được** (`cargo check` RC=0) mà
    checker **XANH cả hai chế độ** — tệp không vào `files`, không vào `_protected_files`, không
    vào `scan_structure`, không vào `extra` (worklist seed từ `files`).

    Nay chỉ bỏ qua khi `target` đứng **trước** `src` trong đường dẫn **tương đối gốc** — đúng
    thư mục build thật (`v2/kernel/target/`, `v2/target/`, `<crate>/target/`), **không** bỏ qua
    `src/**/target/**`. Đường dẫn không nằm dưới `root` ⇒ **không** bỏ qua (fail-closed: quét
    thừa còn hơn bỏ sót).
    """
    try:
        parts = p.relative_to(root).parts
    except ValueError:
        return False
    if "target" not in parts:
        return False
    ti = parts.index("target")
    if "src" not in parts:
        return True
    return ti < parts.index("src")


def _iter_rs_ci(base: Path, root: Path) -> List[Path]:
    """Mọi tệp `.rs` dưới `base`, đuôi **không phân biệt hoa/thường**, đã lọc thư mục build.

    **KT-242 (N4)** — `rglob("*.rs")` so khớp **phân biệt** hoa/thường, nhưng rustc trên
    macOS/Windows nạp `src/x.RS` cho `mod x;` (đã đo: `cargo check` RC=0) ⇒ tệp **sản phẩm**
    không được quét (lớp 1 XANH, lớp 2 ĐỎ). Nay duyệt bằng `os.scandir` — **cắt sớm** thư mục
    build (`target/`) nên không chậm hơn `rglob`, và so `suffix.lower() == ".rs"`.

    Trả danh sách **đã sắp xếp** (thứ tự ổn định như `sorted(rglob(...))` trước đây).
    """
    out: List[Path] = []
    seen: set = set()
    stack: List[Path] = [base]
    while stack:
        d = stack.pop()
        try:
            st = d.stat()
        except OSError:
            continue
        key = (st.st_dev, st.st_ino)
        if key in seen:  # symlink vòng
            continue
        seen.add(key)
        try:
            entries = list(os.scandir(d))
        except OSError:
            continue
        for e in entries:
            p = Path(e.path)
            try:
                if e.is_dir(follow_symlinks=True):
                    if not _is_build_dir(root, p):
                        stack.append(p)
                    continue
                if not e.is_file(follow_symlinks=True):
                    continue
            except OSError:
                continue
            if p.suffix.lower() == ".rs":
                out.append(p)
    return sorted(out)


def _scope_rs_files(root: Path, scope: "wake_scope.ProtectedScope") -> Iterable[Path]:
    """**KT-264** — mọi tệp `.rs` trong **phạm vi bảo vệ** (`scope.contains`), không chỉ `V2_SCAN`.

    Nợ §7.3 của KT-258: `scan_lexical`/`scan_structure` chỉ chạy `v2/kernel`+`v2/sim`(+`v2/lib`
    cho lexical) trong khi **phạm vi định nghĩa** rộng hơn (crate trong **bao đóng sản phẩm** ở
    gốc mới, cây nguồn không-phải-crate). Hệ quả: một crate trong bao đóng sản phẩm có **lỗi từ
    vựng** hoặc **`mod` graph lạ** không bị lớp tĩnh bắt. Hàm này là **một** nguồn cho cả hai
    luật (một vòng lặp, không hai định nghĩa phạm vi).

    Duyệt `scope.roots()` (crate bảo vệ ∪ cây nguồn không-phải-crate) rồi lọc bằng
    `scope.contains` — nhờ vậy `v2/kernel/vifast-plat-user` (shim không phải crate) vẫn được
    quét, và **không** thu hẹp so với `_protected_files` cũ (`check_no_reduction` bảo đảm mọi
    tệp `.rs` dưới `LEGACY_ROOTS` còn trong phạm vi).
    """
    seen: set = set()
    for base_rel in scope.roots():
        base = root / base_rel
        if not base.is_dir():
            continue
        for p in _iter_rs_ci(base, root):
            if p in seen or _is_build_dir(root, p):
                continue
            if not scope.contains(rel(root, p)):
                continue
            seen.add(p)
            yield p


def scan_lexical(root: Path, r: Report, scope: "wake_scope.ProtectedScope") -> None:
    """**D2-166 §3C** — lỗi từ vựng trong tệp thuộc phạm vi bảo vệ ⇒ ĐỎ (không bỏ qua).

    **KT-264** — phạm vi lấy từ `wake_scope.protected_scope` (nợ §7.3), không còn hằng
    `("v2/kernel", "v2/sim", "v2/lib")`: crate ở gốc mới trong bao đóng sản phẩm cũng phải
    được kiểm từ vựng, nếu không `_mods_of`/`resolve_include_targets` **âm thầm** bỏ cạnh của
    tệp đó (chúng `continue` khi `parse_file` lỗi) ⇒ mù cấu trúc.
    """
    for p in _scope_rs_files(root, scope):
        _, errs = parse_file(p)
        for e in errs:
            r.err(e.code, f"  {rel(root, Path(e.relp))}:{e.line}:{e.col}: {e.msg}")


def scan_structure(root: Path, r: Report, scope: "wake_scope.ProtectedScope") -> None:
    """**D2-166 §3C** — cấu trúc liên quan tới cạnh nguồn không phân giải được ⇒ ĐỎ.

    **KT-264** — phạm vi lấy từ `wake_scope.protected_scope` (nợ §7.3), không còn hằng
    `V2_SCAN`: khai báo `mod` hỏng trong crate ở gốc mới trước bản này **không bao giờ** được
    kiểm. Ba bất biến **cấu trúc** khác (cạnh `mod` không phân giải / ra ngoài crate / tệp mồ
    côi) nằm ở [`scan_structure_scope`] — chạy **sau** `resolve_include_targets` để biết đích
    `include!` (tệp được `include!` **không** phải mồ côi).
    """
    for p in _scope_rs_files(root, scope):
        tree, errs = parse_file(p)
        if tree is None or errs:
            continue
        relp = rel(root, p)
        errs2: List[LexError] = []
        _walk_items(tree, relp, p.parent, errors=errs2)
        for e in errs2:
            r.err(e.code, f"  {relp}:{e.line}:{e.col}: {e.msg}")


def _crate_mod_walk(
    root: Path, crate_dir: Path
) -> Tuple[set, List[Tuple[str, int, str, Optional[Path]]], bool]:
    """Đi `mod` graph của **một** crate từ crate root.

    Trả `(tập tệp tới được, cạnh mod, có tệp không phân tích được?)`. Mỗi cạnh là
    `(relp, dòng, tên mod, tệp đích hoặc None)` — `None` = **không phân giải được** (fail-closed).

    **KT-264** — dùng `_crate_roots_in` (cùng định nghĩa crate root với `_module_graph`) và
    `_mods_of`-tương đương để **một** nguồn cho "cạnh `mod` giải đi đâu". Khác `_mods_of`:
    giữ cả cạnh **không** phân giải được (để phát `E_STRUCT_MOD_MISSING`) và **không** lọc
    theo `is_file()` (để phát `E_STRUCT_MOD_OUTSIDE` cho `#[path]` trỏ ra ngoài crate).
    """
    crate_dir = crate_dir.resolve()
    reachable: set = set()
    edges: List[Tuple[str, int, str, Optional[Path]]] = []
    moddir: Dict[Path, Path] = {}
    work: List[Path] = []
    parse_fail = False
    for p, _is_test in _crate_roots_in(root, [crate_dir]):
        if p not in reachable:
            reachable.add(p)
            moddir[p] = p.parent
            work.append(p)
    while work:
        p = work.pop()
        relp = rel(root, p)
        tree, errs = parse_file(p)
        if tree is None or errs:
            # Lỗi từ vựng đã được `scan_lexical` báo (ĐỎ) — không thêm `E_STRUCT_ORPHAN` oan
            # cho con của tệp này, nhưng **không** im lặng: `parse_fail` chặn luật mồ côi.
            parse_fail = True
            continue

        def on_mod(
            name_tok: Tok,
            attrs: Sequence[Attr],
            inline: bool,
            group: Optional[Group],
            cur_dir: Path,
            in_test: bool = False,
            mod_dir: Optional[Path] = None,
        ) -> None:
            if inline:
                return
            plain_base = mod_dir if mod_dir is not None else cur_dir
            target: Optional[Path] = None
            # **KT-264** — `child_dir` phải đúng luật Rust (cùng `_mods_of`): `#[path]` ⇒ thư
            # mục tệp đích; `mod NAME;` ⇒ `plain_base/NAME` (tệp non-mod-rs `src/foo.rs` có
            # con ở `src/foo/`). Trước bản này lấy `target.parent` cho **mọi** cạnh ⇒
            # `kobj.rs` (`mod state_write;` ⇒ `src/kobj/state_write.rs`) bị coi là
            # `src/state_write.rs` ⇒ **báo oan** `E_STRUCT_MOD_MISSING` + `E_STRUCT_ORPHAN`
            # trên cây thật (đo được: 2 lỗi).
            child_dir: Optional[Path] = None
            for a in attrs:
                try:
                    a_edges, _ = attr_path_edges(a, relp)
                except LexError:
                    continue
                if a_edges:
                    # **KT-242 (N6)** — đường dẫn không phân giải được ⇒ cạnh `None`
                    # (`E_STRUCT_MOD_MISSING`), không ném ra ngoài.
                    target = _safe_resolve(cur_dir, a_edges[0][0])
                    if target is not None:
                        child_dir = target.parent
                    break
            if target is None:
                for cand in (
                    plain_base / (name_tok.val + ".rs"),
                    plain_base / name_tok.val / "mod.rs",
                ):
                    if cand.is_file():
                        target = cand.resolve()
                        child_dir = plain_base / name_tok.val
                        break
            edges.append((relp, name_tok.line, name_tok.val, target))
            if target is not None and target not in reachable:
                reachable.add(target)
                moddir[target] = child_dir if child_dir is not None else target.parent
                work.append(target)

        _walk_items(
            tree,
            relp,
            p.parent,
            on_mod=on_mod,
            errors=[],
            mod_dir=moddir.get(p) or p.parent,
        )
    return reachable, edges, parse_fail


def scan_structure_scope(
    root: Path,
    r: Report,
    scope: "wake_scope.ProtectedScope",
    include_targets: set,
) -> None:
    """**KT-264** — luật **cấu trúc** theo **phạm vi** (nợ §7.3 của KT-258, D2-166 §3C).

    Ba bất biến, mỗi bất biến **một** mã lỗi (fail-closed, không allowlist):

      - `E_STRUCT_MOD_MISSING` — `mod NAME;` **không** phân giải được tệp đích. Trước bản này
        `_mods_of` **bỏ im lặng** cạnh đó (chỉ thêm khi `is_file()`), nên một `mod` nằm trong
        vùng `#[cfg]` tắt (cargo RC=0) là **cạnh nguồn ẩn** với mọi luật khác.
      - `E_STRUCT_MOD_OUTSIDE` — cạnh `mod`/`#[path]` trỏ **ra ngoài thư mục crate**. Cạnh như
        vậy biên dịch **cùng một tệp** vào hai crate với hai phân loại test/sản phẩm khác nhau
        ⇒ `is_test` của luật token có thể sai. `resolve_include_targets` chỉ chặn đích ra ngoài
        `v2/kernel`+`v2/sim`, **không** chặn đích ra ngoài **crate** (đo: S2b).
      - `E_STRUCT_ORPHAN` — tệp `.rs` dưới `<crate>/src/` **không** thuộc `mod` graph và
        **không** phải đích `include!`. Tệp mồ côi là chỗ `mod` graph của checker và của rustc
        **lệch nhau**: một `mod` thêm sau sẽ biên dịch nó mà không ai kiểm cạnh nguồn (đo: S5).

    Phạm vi: **mọi** crate bảo vệ (`scope.protected()`), không chỉ `V2_SCAN`. Tệp ngoài `src/`
    (`build.rs`, `init/*.rs` do `build.rs` biên dịch riêng, shim `vifast-plat-user`) **không**
    thuộc luật mồ côi — chúng không nằm trong `mod` graph của crate theo đúng luật Rust.
    """
    for c in scope.protected():
        crate_dir = root / c.dir_rel
        if not crate_dir.is_dir():
            continue
        # **KT-264** — `mod` graph chỉ có nghĩa khi crate **có manifest trên đĩa**: crate root
        # (`src/lib.rs`/`src/main.rs`/`[[bin]]`/`src/bin/*.rs`) do `Cargo.toml` quyết định.
        # Cây **fixture** của selftest dựng metadata **giả** (`scope_from_metadata`) và không
        # có `Cargo.toml` ⇒ bỏ qua (không phải cây cargo thật). Trên cây thật **mọi** crate
        # trong `cargo metadata` đều có manifest (D2-188) ⇒ luật luôn chạy.
        if not (root / c.manifest_rel).is_file():
            continue
        roots = _crate_roots_in(root, [crate_dir])
        # **Fail-closed**: crate **có** manifest mà **không** có crate root ⇒ cargo cũng không
        # build được, nhưng im lặng bỏ qua là sai (D2-188 §2) ⇒ ĐỎ `E_STRUCT_NO_ROOT`.
        if not roots:
            r.err(
                "E_STRUCT_NO_ROOT",
                f"  {c.dir_rel}: crate **có** `{c.manifest_rel}` nhưng **không** có crate "
                f"root (`src/lib.rs`/`src/main.rs`/`[[bin]]`/`src/bin/*.rs`/`tests/*.rs`) — "
                f"không dựng được `mod` graph (fail-closed, KT-264)",
            )
            continue
        # Chỉ có `tests/*.rs` (integration test) ⇒ **không** có `mod` graph của lib/bin để
        # kiểm mồ côi (`tests/*.rs` là crate **riêng**, không định nghĩa cây module của lib).
        # Cargo cũng không build được crate chỉ có `tests/` ⇒ không có mã nào để bảo vệ.
        if not any(p.parent != (crate_dir / "tests") for p, _t in roots):
            continue
        reachable, edges, parse_fail = _crate_mod_walk(root, crate_dir)
        for relp, line, name, target in edges:
            if target is None:
                r.err(
                    "E_STRUCT_MOD_MISSING",
                    f"  {relp}:{line}: `mod {name};` không phân giải được tệp đích — cạnh "
                    f"nguồn ẩn (fail-closed, KT-264)",
                )
                continue
            try:
                target.relative_to(crate_dir.resolve())
            except ValueError:
                r.err(
                    "E_STRUCT_MOD_OUTSIDE",
                    f"  {relp}:{line}: `mod {name};` trỏ **ra ngoài crate** `{c.dir_rel}` "
                    f"(`{rel(root, target)}`) — cùng một tệp biên dịch vào hai crate "
                    f"(fail-closed, KT-264)",
                )
        if parse_fail:
            # Crate có tệp không phân tích được ⇒ `E_LEX_*` đã ĐỎ; luật mồ côi sẽ báo oan
            # (con của tệp hỏng không vào graph). Không im lặng: cổng vẫn ĐỎ.
            continue
        src_prefix = c.dir_rel + "/src/"
        for p in _iter_rs_ci(crate_dir, root):
            if _is_build_dir(root, p):
                continue
            if p in reachable:
                continue
            relp = rel(root, p)
            if relp in include_targets:
                continue
            if not relp.startswith(src_prefix):
                continue
            r.err(
                "E_STRUCT_ORPHAN",
                f"  {relp}: tệp `.rs` **không** thuộc `mod` graph của crate `{c.dir_rel}` và "
                f"không phải đích `include!` — mồ côi (fail-closed, KT-264)",
            )


# ------------------------------------------------------------------ KT-171: test-only thật

# **KT-172** — `is_test_file_heuristic` (miễn theo **tên** `ktest*`/`*_tests.rs`/`tls_tests.rs`
# và theo **đường dẫn** `/tests/`) đã **xoá**. Nó là đúng lỗ hổng reviewer L11 đã phá cho luật
# `wake` (KT-168 §1.2 B8/B9): một module **sản phẩm** tên `ktest_evil.rs` hoặc đặt dưới
# `src/tests/` được miễn dù được biên dịch vào ảnh sản phẩm. KT-171 vá luật `wake`; KT-172 vá
# **nốt** bốn call site của luật 1–5 (kênh `MailboxMsg`, consumer, arch, `E_STRICT_TRANSITION`)
# sang [`is_test_file`] (đồ thị module). Không còn đường nào miễn theo tên ⇒ không còn hai định
# nghĩa "test-only" lệch nhau.

# **KT-194** — các regex `RE_MOD_DECL*`, `RE_PATH_ATTR_LINE`, `RE_CFG_ATTR_LINE`,
# `RE_STR_LIT` của lớp từ vựng cũ đã **xoá**; `_mods_of` đọc bằng token-tree
# (`_walk_items` + `attr_path_edges`). Giữ tên cũ trong lịch sử journal, không giữ mã.

# Cache theo gốc cây (selftest dựng nhiều cây tạm, mỗi cây một khoá).
_TEST_ONLY_CACHE: Dict[str, set] = {}
_MODS_CACHE: Dict[Tuple[str, str], List[Tuple[Path, bool, Path]]] = {}
# **KT-213** — đồ thị module dùng chung cho `test_only_modules` **và** gốc phân giải `#[path]`
# (`module_dirs`). Một lần dựng, hai nơi đọc ⇒ không có hai định nghĩa `mod_dir` lệch nhau.
_GRAPH_CACHE: Dict[str, Tuple[Dict[Path, bool], Dict[Path, Path]]] = {}


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


def _crate_roots_in(root: Path, crate_dirs: Sequence[Path]) -> List[Tuple[Path, bool]]:
    """Crate root của **các crate trong `crate_dirs`**: `(tệp, test-only?)`.

    Test-only khi: bin có `required-features` chứa `ktest` (ảnh test kernel), hoặc
    `tests/*.rs` cấp crate (integration test). `src/lib.rs`/`src/main.rs` là **sản phẩm**.

    **KT-264** — tách khỏi [`crate_roots`] để luật **cấu trúc** dùng **cùng** một định nghĩa
    "crate root" cho crate **ngoài `V2_SCAN`** (một nguồn, không chép luật — cùng lý do KT-213
    tách `_module_graph` khỏi `test_only_modules`). Thêm hai dạng crate root **tự nhận** của
    cargo mà bản cũ bỏ sót: `src/bin/*.rs` và `src/bin/*/main.rs` (không cần `[[bin]]` trong
    `Cargo.toml`). Thiếu chúng thì `v2/lib/vifast-elf/src/bin/vifast-elf-validate.rs` bị coi là
    **mồ côi** ⇒ luật `E_STRUCT_ORPHAN` báo oan trên cây thật (đo được: 1 ca).
    """
    out: List[Tuple[Path, bool]] = []
    seen: set = set()

    def add(p: Path, is_test: bool) -> None:
        if p.is_file() and p not in seen:
            seen.add(p)
            out.append((p, is_test))

    for crate in crate_dirs:
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
        # **KT-264** — `src/bin/*.rs` / `src/bin/*/main.rs` là crate root **tự nhận** của cargo.
        src_bin = crate / "src/bin"
        if src_bin.is_dir():
            for b in sorted(src_bin.glob("*.rs")):
                add(b, False)
            for b in sorted(src_bin.glob("*/main.rs")):
                add(b, False)
        tests_dir = crate / "tests"
        if tests_dir.is_dir():
            for t in sorted(tests_dir.glob("*.rs")):
                add(t, True)
            # **KT-407** — Cargo cũng nhận `tests/<tên>/main.rs` là integration test (cùng
            # khuôn `src/bin/*/main.rs`). Thiếu dạng này thì tệp test bị coi là **sản phẩm**
            # ⇒ luật `wake` báo `E_CONSUMER` oan trên chính harness parity (đã tái hiện).
            for t in sorted(tests_dir.glob("*/main.rs")):
                add(t, True)
    return out


def crate_roots(root: Path) -> List[Tuple[Path, bool]]:
    """Crate root trong `v2/kernel` + `v2/sim`: `(tệp, test-only?)` (xem [`_crate_roots_in`])."""
    dirs: List[Path] = []
    for base_rel in V2_SCAN:
        base = root / base_rel
        if not base.is_dir():
            continue
        dirs.extend(sorted(p for p in base.iterdir() if p.is_dir()))
    return _crate_roots_in(root, dirs)


def _mods_of(p: Path, mod_dir: Path) -> List[Tuple[Path, bool, Path]]:
    """Module con khai báo trong `p`: `(tệp đích, mod nằm trong ngữ cảnh test?, thư mục module
    của đích)`.

    **KT-194/D2-166** — đọc bằng **token-tree**, không regex trên văn bản thô:
      - `mod NAME;` / `mod NAME { … }` nhận theo **cấu trúc** (raw ident `r#x` ⇒ `x`);
      - `#[path = "…"]` lấy literal **đã giải mã escape** (`"a\x2eb"` ⇒ `a.b`);
      - `#[r#path = "…"]` là **cùng** thuộc tính `path` (raw ident được chuẩn hoá);
      - `cfg_attr(…, path = …)` **lồng** được đệ quy có giới hạn; cạnh **có điều kiện** vẫn
        được thu thập (bảo thủ: có thể là sản phẩm) — xem `attr_path_edges`;
      - `mod` trong **lời gọi macro** / thân `macro_rules!` **không** phải khai báo module
        (luật macro riêng xử lý).

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
    tree, errs = parse_file(p)
    if tree is None or errs:
        _MODS_CACHE[key] = out
        return out
    try:
        text = p.read_text(encoding="utf-8", errors="replace")
    except OSError:
        _MODS_CACHE[key] = out
        return out
    # **KT-242 (N5)** — `tr = test_line_ranges(text)` ở đây là **chết** (không dùng); bỏ để
    # không còn chỗ nào *trông như* `test_line_ranges` còn quyết định luật.
    relp = str(p)

    def on_mod(
        name_tok: Tok,
        attrs: Sequence[Attr],
        inline: bool,
        group: Optional[Group],
        cur_dir: Path,
        in_test: bool = False,
        mod_dir: Optional[Path] = None,
    ) -> None:
        # **KT-196/F2** (A0 2026-11-19 §2): ngữ cảnh test quyết định theo **item/token-tree**,
        # KHÔNG theo dòng văn bản. `#[cfg(test)] fn h() {} mod evil;` — `mod evil` không có
        # thuộc tính nào và không nằm trong khối test ⇒ sản phẩm.
        is_test = in_test or any(
            a.name == "cfg" and cfg_is_test_tokens(a.args) for a in attrs
        )
        if inline:
            # Inline module: không có tệp đích; con của nó được duyệt với `cur_dir/name`.
            return
        # **KT-213** — hai gốc khác nhau, đúng như rustc:
        #   - `#[path = "…"]` giải theo **mức hiện tại** (`cur_dir`: thư mục tệp ở mức gốc,
        #     thư mục inline module khi lồng trong `mod NAME { … }`);
        #   - `mod NAME;` không `#[path]` giải theo **thư mục module của tệp** (`mod_dir`:
        #     `src/foo/` với tệp non-mod-rs `src/foo.rs`), rồi `NAME.rs`/`NAME/mod.rs`.
        plain_base = mod_dir if mod_dir is not None else cur_dir
        target: Optional[Path] = None
        child_dir: Optional[Path] = None
        for a in attrs:
            try:
                edges, _ = attr_path_edges(a, relp)
            except LexError:
                continue
            if edges:
                val, _certain, _tok = edges[0]
                # **KT-242 (N6)** — đường dẫn không phân giải được ⇒ bỏ cạnh này; lỗi cổng
                # do `resolve_include_targets` phát (có vị trí nguồn).
                _res = _safe_resolve(cur_dir, val)
                if _res is None:
                    break
                target = _res
                child_dir = target.parent
                break
        if target is None:
            for cand in (
                plain_base / (name_tok.val + ".rs"),
                plain_base / name_tok.val / "mod.rs",
            ):
                if cand.is_file():
                    target = cand.resolve()
                    child_dir = plain_base / name_tok.val
                    break
        if target is not None and child_dir is not None:
            out.append((target, is_test, child_dir))

    # **KT-213** — `cur_dir` mức gốc là **thư mục tệp** (`#[path]` ở mức mục giải theo đây);
    # `mod_dir` là thư mục module của tệp (chỉ khác ở tệp non-mod-rs). Trước KT-213 truyền
    # thẳng `mod_dir` làm `cur_dir` ⇒ `#[path]` mức mục trong `src/foo.rs` giải thành
    # `src/foo/a.rs` thay vì `src/a.rs` (sai một tầng).
    _walk_items(tree, relp, p.parent, on_mod=on_mod, errors=[], mod_dir=mod_dir)
    _MODS_CACHE[key] = out
    return out


def _module_graph(root: Path) -> Tuple[Dict[Path, bool], Dict[Path, Path]]:
    """Đồ thị module từ crate root: `(test-only?, thư mục module)` cho từng tệp.

    **KT-213** — tách khỏi `test_only_modules` để `resolve_include_targets` dùng **cùng**
    một định nghĩa `mod_dir` (gốc phân giải `#[path]`), không chép lần hai. `mod_dir` là
    thư mục mà rustc dùng làm gốc cho `#[path]` **bên trong inline module** của tệp:
      - crate root / `mod.rs` / tệp nạp bằng `#[path]` ⇒ thư mục của tệp;
      - tệp non-mod-rs `src/foo.rs` (nạp bằng `mod foo;`) ⇒ `src/foo/`.
    """
    key = str(root)
    cached = _GRAPH_CACHE.get(key)
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
    _GRAPH_CACHE[key] = (status, moddir)
    return status, moddir


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
    status, _ = _module_graph(root)
    out = {rel(root, p) for p, is_test in status.items() if is_test}
    _TEST_ONLY_CACHE[key] = out
    return out


def module_dirs(root: Path) -> Dict[Path, Path]:
    """**KT-213** — thư mục module **đúng như rustc** cho mọi tệp tới được từ crate root.

    Đây là gốc phân giải `#[path]` **bên trong inline module** (Rust Reference, Items >
    Modules > The path attribute): mod-rs ⇒ thư mục tệp; non-mod-rs ⇒ `<thư mục>/<stem>/`.
    Trước KT-213 `resolve_include_targets` dùng thẳng `p.parent` cho **mọi** tệp, nên
    `#[path]` trong inline module của `src/foo.rs` lệch đúng một tầng `foo/` so với rustc.
    """
    return _module_graph(root)[1]


def _default_mod_dir(p: Path) -> Path:
    """**KT-213** — `mod_dir` cho tệp **không** tới được từ crate root của cargo.

    `module_dirs` phủ **mọi** tệp được biên dịch như module của crate cargo (đồ thị `mod`/
    `#[path]` từ `crate_roots`). Tệp ngoài đồ thị là **crate root** do `build.rs` biên dịch
    trực tiếp (`v2/kernel/*/init/*.rs`, `v2/kernel/vifast-plat-user/lib.rs`, `build.rs`) hoặc
    tệp chết — với cả hai, gốc phân giải `#[path]` là **thư mục của chính tệp** (rustc coi
    crate root là mod-rs). Đây cũng là hành vi cũ trước KT-213, nên không thêm báo oan.
    """
    return p.parent


def is_test_file(path: Path, root: Path, test_only: Optional[set] = None) -> bool:
    """Tệp **test-only thật sự** (KT-171) — dùng cho **mọi** luật (1–5 và luật `wake`).

    Dựa trên **đồ thị module** (`test_only_modules`), không dựa tên tệp/đường dẫn. Đây là bản
    vá B8/B9 của reviewer L11 (KT-168 §1.2). **KT-172**: luật 1–5 cũng dùng hàm này (trước đó
    dùng `is_test_file_heuristic`, nay đã xoá) — một định nghĩa "test-only" duy nhất.
    """
    if test_only is None:
        test_only = test_only_modules(root)
    return rel(root, path) in test_only


# ------------------------------------------------------------------ KT-171: B6/B7 include!/#[path]

# **KT-194** — `RE_INCLUDE_MACRO`/`RE_PATH_ATTR_ANY`/`RE_CFG_ATTR_PATH` của lớp từ vựng cũ
# đã **xoá**; `resolve_include_targets` đọc bằng token-tree (`_walk_items` + `attr_path_edges`).


def _inside_scan_tree(root: Path, target: Path) -> Optional[str]:
    """`relpath` nếu `target` nằm trong `v2/kernel`/`v2/sim`, ngược lại `None`."""
    try:
        relp = str(target.relative_to(root))
    except ValueError:
        return None
    if relp.startswith("v2/kernel/") or relp.startswith("v2/sim/"):
        return relp
    return None


def _inside_lib_tree(root: Path, target: Path) -> Optional[str]:
    """**KT-209/L3** — `relpath` nếu `target` nằm trong `v2/lib`, ngược lại `None`."""
    try:
        relp = str(target.relative_to(root))
    except ValueError:
        return None
    if relp.startswith("v2/lib/"):
        return relp
    return None


def _source_tree(relp: str) -> str:
    """**KT-209/L3** — cây nguồn của một tệp: `kernel` / `sim` / `lib` / `""`."""
    for name in ("kernel", "sim", "lib"):
        if relp.startswith(f"v2/{name}/"):
            return name
    return ""


def resolve_include_targets(
    root: Path, files: Sequence[Tuple[Path, str, List[Fn]]], test_only: set
) -> Tuple[Dict[str, Tuple[Path, bool]], List[str]]:
    """KT-171 (B6/B7): phân giải `include!("…")` + `#[path = "…"]` và trả tệp đích.

    Reviewer L11 đặt `m.wake(t)` trong tệp **ngoài** phạm vi quét rồi kéo vào bằng `include!`
    hoặc `#[path]` ⇒ `--strict` vẫn `errors=0`. Nay:
      - đích **ngoài** `v2/kernel`+`v2/sim` ⇒ **ĐỎ** (không quét được, fail-closed);
      - đích **trong** cây ⇒ **quét chính tệp đích** (kế thừa trạng thái test-only của tệp
        kéo nó vào; sản phẩm thắng nếu cả hai).

    **KT-177/G4** (reviewer L16 §2.4, H4): bản KT-171 dùng `resolved` để **không xử lý lại** một
    tệp. Với `include!` **lồng hai lớp** (`core_sched.rs` (sản phẩm) → `evil_a.rs` (test-only) →
    `evil_b.rs` (test-only)), `evil_a` được xử lý **trước** với `is_test=True` ⇒ `evil_b` nhận
    `True`; khi `core_sched.rs` include `evil_a` thì `evil_a` đã `resolved` nên **không** lan
    `False` xuống `evil_b` ⇒ `evil_b` ở lại test-only ⇒ lọt. Nay là **fixpoint/worklist** như
    `test_only_modules`: trạng thái mỗi tệp chỉ đi `True → False` (đơn điệu), và khi nó đổi thì
    tệp được **xử lý lại** để lan tiếp xuống con. Không có vòng lặp vô hạn vì trạng thái đơn điệu.

    **KT-209/L3** (reviewer L40 KT-201 §D2 L3): trước bản này worklist chỉ seed từ
    `v2/kernel`+`v2/sim`, nên `include!`/`#[path]` trong `v2/lib` **ngoài** thân macro **không**
    được phân giải ⇒ đích ngoài cây không bị quét, và hợp đồng §4 chỉ ghi `#[path]`. Nay seed
    thêm **mọi** tệp `v2/lib/**/*.rs`; nguồn `v2/lib` có luật riêng: đích phải nằm **trong
    `v2/lib`** (fail-closed). Đích sang `v2/kernel`/`v2/sim` bị **ĐỎ** — `include!`/`#[path]`
    của lib kéo mã tầng khác vào crate lib, và luật `wake` chỉ quét kernel/sim nên không được
    để cạnh nguồn ẩn. Đích trong `v2/lib` được phân giải **tiếp** (include! lồng không trốn
    được) nhưng **không** vào `extra`: `extra` là tập tệp bị luật `wake` quét, mà mã `v2/lib`
    không có `Machine` nên `wake()` không thể xuất hiện ở đó (cạnh sang kernel/sim đã bị chặn).
    """
    extra: Dict[str, Tuple[Path, bool]] = {}
    errors: List[str] = []
    status: Dict[Path, bool] = {}
    # **KT-213** — mỗi mục hàng đợi mang **gốc phân giải** `#[path]` của tệp (thư mục module
    # đúng như rustc), không phải `p.parent` cho mọi tệp. Xem `module_dirs`/`_default_mod_dir`.
    queue: List[Tuple[Path, bool, Path]] = []
    moddir = module_dirs(root)
    for p, _, _ in files:
        queue.append((p, rel(root, p) in test_only, moddir.get(p) or _default_mod_dir(p)))
    # **KT-209/L3** — `include!`/`#[path]` trong `v2/lib` ngoài macro cũng phải phân giải.
    # Tệp lib là **sản phẩm** (không có ngữ cảnh test-only trong `test_only_modules`, vốn chỉ
    # dựng đồ thị module cho kernel/sim) ⇒ `is_test=False`.
    for p in iter_rs(root, ("v2/lib",)):
        queue.append((p, False, moddir.get(p) or _default_mod_dir(p)))
    # **KT-213** — `(tệp, mod_dir)` đã xử lý. Cùng một tệp có thể được nạp bằng hai cạnh khác
    # nhau (`mod foo;` ⇒ `src/foo/`, `#[path = "foo.rs"] mod bar;` ⇒ `src/`) và `#[path]` bên
    # trong inline module giải **khác nhau** theo từng cạnh ⇒ phải xử lý **mỗi cặp** một lần.
    done: set = set()
    while queue:
        p, is_test, p_mod_dir = queue.pop()
        key = (p, p_mod_dir)
        if p in status:
            # Đã xử lý với trạng thái **mạnh hơn hoặc bằng** ⇒ không cần lại. Chỉ xử lý lại
            # đúng một lần khi `True → False` (sản phẩm thắng) để lan xuống con.
            if key in done and (not status[p] or is_test):
                continue
            status[p] = status[p] and is_test
        else:
            status[p] = is_test
        done.add(key)
        tree, errs = parse_file(p)
        if tree is None or errs:
            # Lỗi từ vựng đã được `scan_lexical` báo; không phân giải tiếp trên cây thiếu.
            continue
        relp = rel(root, p)
        found: List[Tuple[str, str, Tok, Path]] = []
        problems: List[Tuple[object, str]] = []

        def on_path_attr(attr: Attr, cur_dir: Path) -> None:
            try:
                edges, probs = attr_path_edges(attr, relp)
            except LexError as e:
                # **KT-195/F3** (reviewer L34): KHÔNG nuốt lỗi cấu trúc — `E_LEX_DEPTH` phải
                # nổi lên thành lỗi cổng (D2-166 §3C: quá độ sâu ⇒ lỗi rõ + exit khác 0).
                errors.append(f"  {e.relp}:{e.line}:{e.col}: {e.code} {e.msg} (fail-closed, KT-195 F3)")
                return
            for val, certain, tok in edges:
                kind = "#[path]" if certain else "#[cfg_attr(path)]"
                found.append((kind, val, tok, cur_dir))
            problems.extend(probs)

        def on_include(nodes: Sequence[object], j: int, include_tok: Tok) -> None:
            """**KT-211** — đối số `include!` lấy từ **token kế tiếp**, ở **bất kỳ** vị trí.

            Nhóm `{…}`/`(…)`/`[…]` phải chứa **đúng một** literal chuỗi; chuỗi literal trần
            (không nhóm) cũng nhận (phòng thủ). Mọi dạng khác ⇒ **ĐỎ** fail-closed — kể cả
            `include!` **không có** đối số (không phân giải được đích).
            """
            if j >= len(nodes):
                problems.append(
                    (include_tok, "`include!` không có đối số — không phân giải được đích")
                )
                return
            arg = nodes[j]
            if isinstance(arg, Group):
                ch = arg.children
                # **KT-225** — `include!("x",)` (trailing comma) là cú pháp Rust hợp lệ;
                # trước KT-225 bị ĐỎ oan vì đòi **đúng một** token.
                if (
                    len(ch) == 2
                    and isinstance(ch[0], Tok)
                    and ch[0].kind == "str"
                    and _is_punct(ch[1], ",")
                ):
                    ch = ch[:1]
                if len(ch) == 1 and isinstance(ch[0], Tok) and ch[0].kind == "str":
                    # `include!` giải đường dẫn theo **thư mục tệp gọi**, không theo inline module.
                    found.append(("include!", ch[0].val, ch[0], p.parent))
                else:
                    problems.append(
                        (arg.open, "`include!` không phải literal chuỗi — không phân giải được đích")
                    )
                return
            if isinstance(arg, Tok) and arg.kind == "str":
                found.append(("include!", arg.val, arg, p.parent))
                return
            problems.append(
                (include_tok, "`include!` không phải literal chuỗi — không phân giải được đích")
            )

        inline_path_toks: set = set()

        def on_mod(
            name_tok: Tok,
            attrs: Sequence[Attr],
            inline: bool,
            group: Optional[Group],
            cur_dir: Path,
            in_test: bool = False,
            mod_dir: Optional[Path] = None,
        ) -> None:
            if not inline:
                return
            for a in attrs:
                try:
                    edges, _ = attr_path_edges(a, relp)
                except LexError:
                    continue
                for _val, _certain, tok in edges:
                    inline_path_toks.add(tok.start)

        # **KT-211** (reviewer L44, KT-208 §B1) — quét **toàn bộ token** của tệp (mọi tầng
        # nhóm, mọi loại nhóm `{`/`(`/`[`) sinh cạnh `include!`/`#[path]`. Đường cấu trúc
        # `_walk_items` chỉ đệ quy `{ … }` nên bỏ sót `include!` ở **vị trí biểu thức** trong
        # `( … )`/`[ … ]` và `#[path]` trên `mod` trong khối lồng `( … )` — reviewer L44 dựng
        # **6** biến thể, `cargo check` RC=0, checker `errors=0` **cả hai** chế độ.
        # **KT-213** — hai gốc: `cur_dir` mức gốc là **thư mục tệp** (`p.parent`) cho `#[path]`
        # mức mục; `mod_dir` là **thư mục module** của tệp (`p_mod_dir`) cho inline module
        # **không** `#[path]` (`src/foo.rs` ⇒ `src/foo/NAME/`). `_walk_all` nhận `mod_dir` cùng
        # nghĩa với `_walk_items` để inline module lồng nhau vẫn đúng.
        _walk_all(
            tree,
            relp,
            p.parent,
            on_include,
            on_path_attr,
            inline_path_toks,
            mod_dir=p_mod_dir,
        )
        # **KT-213** — `mod` giữ đường cấu trúc (`_walk_items`): thu thập `inline_path_toks`
        # cho `#[path]` trên inline module. **KT-211** — KHÔNG truyền `on_attr`/`on_include`
        # (cạnh `include!`/`#[path]` **chỉ** do `_walk_all` sinh; tránh trùng).
        _walk_items(
            tree,
            relp,
            p.parent,
            on_mod=on_mod,
            errors=[],
            mod_dir=p_mod_dir,
        )
        for tok, msg in problems:
            if isinstance(tok, LexError):
                continue
            line = tok.line if isinstance(tok, Tok) else 1
            errors.append(f"  {relp}:{line}: {msg} (fail-closed, KT-194)")
        src_tree = _source_tree(relp)
        for kind, raw, tok, base in found:
            # **KT-242 (N6)** — đường dẫn không phân giải được (NUL/quá dài/vòng symlink)
            # ⇒ **lỗi cổng rõ** thay vì traceback (D2-166 §3C).
            target = _safe_resolve(base, raw)
            if target is None:
                errors.append(
                    f"  {relp}:{tok.line}: `{kind}` có đường dẫn không phân giải được "
                    f"({raw!r}) — fail-closed (KT-242 N6)"
                )
                continue
            if src_tree == "lib":
                # **KT-209/L3** — nguồn `v2/lib`: đích phải nằm **trong `v2/lib`**. Đích sang
                # `v2/kernel`/`v2/sim` (mã tầng khác) hoặc ra ngoài cây ⇒ ĐỎ: `include!`/
                # `#[path]` của lib kéo mã lạ vào crate lib, và luật `wake` chỉ quét kernel/sim
                # nên không được để cạnh nguồn ẩn.
                inside = _inside_lib_tree(root, target)
                if inside is None:
                    errors.append(
                        f"  {relp}:{tok.line}: `{kind}` trỏ ra ngoài `v2/lib` "
                        f"(`{raw}`) — không quét được đích (fail-closed, KT-209 L3)"
                    )
                    continue
                if target.is_dir():
                    if tok.start in inline_path_toks:
                        continue
                    errors.append(
                        f"  {relp}:{tok.line}: `{kind}` trỏ tới thư mục `{inside}` nhưng `mod` "
                        f"không có thân — không phân giải được (fail-closed, KT-199 F4c)"
                    )
                    continue
                if not target.is_file():
                    errors.append(
                        f"  {relp}:{tok.line}: `{kind}` trỏ tới `{inside}` không tồn tại "
                        f"(fail-closed, KT-209 L3)"
                    )
                    continue
                # Đích trong `v2/lib`: phân giải **tiếp** (`include!` lồng không trốn được).
                #
                # **KT-230** — đích `v2/lib` **vào `extra`** (trước đây cố ý bỏ, lý do "mã
                # `v2/lib` không có `Machine` nên `wake()` không thể xuất hiện"). Lý do đó
                # **sai** theo hai nghĩa, và bỏ đích khỏi `extra` làm lớp 1 mù:
                #   (i) tệp đích có thể chứa token `wake` (B12–B15 của KT-224: `.txt`,
                #       symlink, `target/`, dấu cách — lớp 1 XANH, chỉ lớp 2 bắt);
                #   (ii) tệp đích **không phải `.rs`** có thể định nghĩa `macro_rules!` che
                #       tên macro "an toàn" (đường lọt #14, biến thể `.txt`) — `_shadowed_safe_macros`
                #       chỉ thấy nếu đích nằm trong `extra`.
                # Nay fail-closed: đích `v2/lib` là **sản phẩm** (`is_test=False`) và được quét
                # như mọi đích `include!`/`#[path]` khác.
                # **KT-213** — đích của `#[path]`/`include!` có `mod_dir` = thư mục của **tệp
                # đích** (rustc: tệp nạp bằng `#[path]` và tệp `include!` đều giải `#[path]`
                # trong inline module theo thư mục tệp — đã đo bằng rustc 1.100).
                prev = extra.get(inside)
                extra[inside] = (target, False if prev is None else (prev[1] and False))
                if (target, target.parent) not in done or status.get(target, True):
                    queue.append((target, False, target.parent))
                continue
            inside = _inside_scan_tree(root, target)
            if inside is None:
                errors.append(
                    f"  {relp}:{tok.line}: `{kind}` trỏ ra ngoài `v2/kernel`+`v2/sim` "
                    f"(`{raw}`) — không quét được đích (fail-closed, KT-171 B6/B7)"
                )
                continue
            if target.is_dir():
                # **KT-196/F4**: `#[path = "sub"] mod m { … }` — inline module, `sub` là **thư
                # mục** chứa module con, KHÔNG phải tệp nguồn ⇒ bỏ qua (không phải cạnh tệp).
                # **KT-199/F4c** (reviewer L35): nếu `mod` **không** có thân thì `#[path]` trỏ
                # thư mục là **sai cú pháp** ⇒ phải báo lỗi, không được im lặng.
                if tok.start in inline_path_toks:
                    continue
                errors.append(
                    f"  {relp}:{tok.line}: `{kind}` trỏ tới thư mục `{inside}` nhưng `mod` "
                    f"không có thân — không phân giải được (fail-closed, KT-199 F4c)"
                )
                continue
            if not target.is_file():
                errors.append(
                    f"  {relp}:{tok.line}: `{kind}` trỏ tới `{inside}` không tồn tại "
                    f"(fail-closed, KT-171 B6/B7)"
                )
                continue
            prev = extra.get(inside)
            new = is_test if prev is None else (prev[1] and is_test)
            extra[inside] = (target, new)
            # **KT-177/G4**: lan xuống con khi trạng thái đổi `True → False` (hoặc lần đầu).
            # **KT-213** — `mod_dir` của đích là thư mục **tệp đích** (xem chú thích nhánh lib).
            if (target, target.parent) not in done or status.get(target, True):
                queue.append((target, new, target.parent))
    return extra, errors


# ------------------------------------------------------------------ KT-171/KT-181: thân macro

# **KT-194** — `_macro_bodies`/`RE_MACRO_RULES` cũ đã **xoá**: thân macro lấy từ token-tree
# (`_walk_items` gọi `on_macro` với nhóm thân). Tên macro là **định danh** (Unicode, raw ident).

# Allowlist **có lý do** cho thân `macro_rules!` chứa token `wake`.
# Khoá: (relpath, tên macro). Hiện **rỗng** — không có macro nào trong cây chạm `wake`.
MACRO_WAKE_ALLOW: Dict[Tuple[str, str], str] = {}

# **KT-181** (D2-164) — gốc cây nguồn **sản phẩm** mà thân `macro_rules!` phải được kiểm
# **cả hai** luật (token `wake` + `include!`/`#[path]`/`mod`/`:tt`). Trước KT-181 hai luật nằm
# ở hai hàm với phạm vi **khác nhau**: `scan_lib_macros` quét `v2/lib/**` (cả hai luật) còn
# `scan_kernel_macro_includes` quét `v2/kernel/**`+`v2/sim/**` (**chỉ** luật include). Hệ quả:
# thân macro ở `v2/kernel/vifast-kbin-riscv64/init/`, `v2/kernel/*/tests/`,
# `v2/kernel/vifast-plat-user/` chứa token `wake` **không** bị luật token bắt (luật `wake`
# toàn văn bản chỉ phủ `v2/kernel/vifast-*/src/` + `v2/sim/vifast-sim/src/`).
#
# **Ngoài** danh sách (lý do ghi ở `CHECKER-WAKE-CONTRACT.md` §1/§4):
#   - `v2/test/fixtures/**` — fixture **cố tình sai** của các máy kiểm khác;
#   - `v2/tools/**` — công cụ phát triển, **không** link vào ảnh sản phẩm.
#
# **KT-256 (L2, D2-188)** — **xoá** hằng `MACRO_SCAN_ROOTS`: gốc quét nay là
# `wake_scope.protected_scope(root).macro_roots()` — thư mục crate **bảo vệ** (sinh từ
# `cargo metadata`) ∪ cây nguồn không-phải-crate (`vifast-plat-user`). Hệ quả (đều **mở rộng**):
# crate thêm vào workspace ở thư mục gốc **mới** (`v2/drivers/<crate>`) được quét (mẫu cũ mù);
# crate `v2/tools/<crate>` **phụ thuộc** crate bảo vệ được **nâng** lên bảo vệ (đồng nhất với lớp
# dep-info, `_classify_scope`) nên cũng được quét. `v2/tools` **không** phụ thuộc sản phẩm vẫn
# ngoài phạm vi (như cũ).


# **KT-202** — fragment `$…:<frag>` chở được `include!`/`mod`/`#[path]` từ **lời gọi**.
# `:expr` **không** nằm trong danh sách (xem lý do đầy đủ ở hợp đồng §4):
#   (i) `:expr` không chở được `mod` (item) hay `#[path]` (thuộc tính) — chỉ chở được `include!`
#       ở vị trí biểu thức;
#   (ii) `include!` trong **đối số lời gọi** đã bị `_check_macro_passthrough` bắt ở **tệp gọi**
#        (KT-202 vá 1) — kể cả khi macro dùng `:expr`;
#   (iii) cây thật có `macro_rules! bail` (`v2/kernel/vifast-plat-riscv64/src/user.rs`) dùng
#        `$pages:expr, $e:expr` hợp lệ — từ chối `:expr` sẽ báo oan và làm `v2 check` ĐỎ.
MACRO_PASSTHROUGH_FRAGS = frozenset({"tt", "item", "stmt", "block", "meta"})

# **KT-225** (reviewer L52 KT-223 — báo oan `stringify!(include!)`) — macro **không mở rộng**
# đối số thành mã: `stringify!` chỉ chuỗi hoá token; `concat!` chỉ nhận literal (đã đo bằng
# `rustc`: `concat!(include!("x"))` mở rộng `include!` nhưng kết quả **bắt buộc** là literal
# chuỗi ⇒ không thể chở `wake`). Đối số của hai macro này **không** phải mã ⇒ miễn
# `_check_macro_passthrough` và miễn sinh cạnh trong `_walk_all`.
SAFE_TOKEN_MACROS = frozenset({"stringify", "concat"})


def _check_macro_passthrough(
    relp: str,
    where: str,
    nodes: Sequence[object],
    r: Report,
    shadowed_safe: frozenset = frozenset(),
) -> None:
    """**KT-189/B2d+T3c**, **KT-191**, **KT-194**, **KT-202** — token có thể sinh
    `include!`/`#[path]`/`mod` ⇒ fail-closed.

    Dùng chung cho **hai** nơi:
      - thân `macro_rules!` (`_check_macro_body`);
      - **đối số lời gọi macro** (`_scan_macro_calls`, KT-202).

    Nhận dạng **theo cấu trúc token**, không theo từ khoá:
      - `include` + `!` (không phải `include_str!`, không phải biến `include`);
      - thuộc tính `#[path]` (kể cả `#[r#path]`) và `cfg_attr(…, path = …)` **lồng**;
      - `mod NAME;` / `mod NAME { … }` (kể cả raw ident `mod r#x;` ⇒ `E_STRICT_MACRO_INCLUDE`).

    **Giảm báo oan có chủ đích** (D2-166 §3D): `std::path::Path`, `$path:expr`, `modulo(1)`,
    `include_str!` **không** bị coi là dấu hiệu — chúng không phải thuộc tính/macro/module.
    Hạn chế bảo thủ còn giữ: **mọi** thuộc tính trong phạm vi có tên `path`/`cfg_attr` đều
    ĐỎ (không đánh giá `cfg`).
    """

    def check_list(nodes: Sequence[object]) -> None:
        for i, n in enumerate(nodes):
            if isinstance(n, Group):
                # **KT-225** — đối số của `stringify!`/`concat!` là token stream, không phải
                # mã; `include!`/`#[path]`/`mod` trong đó **không** được mở rộng ⇒ miễn.
                _prev = nodes[i - 1] if i > 0 else None
                if (
                    _is_punct(_prev, "!")
                    and i >= 2
                    and isinstance(nodes[i - 2], Tok)
                    and nodes[i - 2].kind == "ident"
                    and nodes[i - 2].val in SAFE_TOKEN_MACROS
                    and nodes[i - 2].val not in shadowed_safe
                ):
                    continue
                check_list(n.children)
                continue
            assert isinstance(n, Tok)
            if n.kind == "ident" and n.val == "include":
                if i + 1 < len(nodes) and _is_punct(nodes[i + 1], "!"):
                    # **KT-225** — chỉ coi là `include!` khi cú pháp macro hợp lệ: sau `!`
                    # là nhóm phân cách hoặc literal chuỗi. `include != 0` có `!` + `=`
                    # ⇒ không phải lời gọi macro (trước KT-225 bị ĐỎ oan).
                    if i + 2 < len(nodes) and (
                        isinstance(nodes[i + 2], Group)
                        or (isinstance(nodes[i + 2], Tok) and nodes[i + 2].kind == "str")
                    ):
                        r.err(
                            "E_STRICT_MACRO_INCLUDE",
                            f"  {relp}:{n.line}: {where} chứa `include!` — "
                            f"đường dẫn giải theo **tệp gọi**, không phân giải tĩnh được "
                            f"(fail-closed, KT-194/KT-202)",
                        )
            if n.kind == "punct" and n.val == "#":
                j = i + 1
                if j < len(nodes) and _is_punct(nodes[j], "!"):
                    j += 1
                if j < len(nodes) and isinstance(nodes[j], Group) and nodes[j].open.val == "[":
                    try:
                        attr = parse_attr(nodes[j], relp)
                    except LexError:
                        r.err(
                            "E_STRICT_MACRO_INCLUDE",
                            f"  {relp}:{nodes[j].open.line}: {where} chứa "
                            f"thuộc tính không đọc được — không phân giải tĩnh được "
                            f"(fail-closed, KT-194/KT-202)",
                        )
                        continue
                    if attr.name == "path":
                        r.err(
                            "E_STRICT_MACRO_INCLUDE",
                            f"  {relp}:{attr.tok.line}: {where} chứa "
                            f"`#[path]` — đường dẫn giải theo **tệp gọi**, không phân giải "
                            f"tĩnh được (fail-closed, KT-194/KT-202)",
                        )
                    elif attr.name == "cfg_attr":
                        try:
                            edges, probs = attr_path_edges(attr, relp)
                        except LexError:
                            edges, probs = [], [(attr.tok, "không đọc được")]
                        if edges or probs:
                            r.err(
                                "E_STRICT_MACRO_INCLUDE",
                                f"  {relp}:{attr.tok.line}: {where} chứa "
                                f"`cfg_attr(…, path = …)` — đường dẫn giải theo **tệp gọi**, "
                                f"không phân giải tĩnh được (fail-closed, KT-194/KT-202)",
                            )
            if n.kind == "ident" and n.val == "mod":
                j = i + 1
                if j < len(nodes) and isinstance(nodes[j], Tok) and nodes[j].kind == "ident":
                    raw = bool(nodes[j].raw)
                    j += 1
                    if j < len(nodes) and _is_punct(nodes[j], ";"):
                        if raw:
                            r.err(
                                "E_STRICT_MACRO_INCLUDE",
                                f"  {relp}:{n.line}: {where} khai "
                                f"`mod r#…;` — không phân giải tĩnh được (fail-closed, KT-194/KT-202)",
                            )
                        else:
                            r.err(
                                "E_STRICT_MACRO_MOD",
                                f"  {relp}:{n.line}: {where} khai `mod …;` — "
                                f"đồ thị module không thấy module này nên không xác định được "
                                f"test-only (fail-closed, KT-194/KT-202)",
                            )
                    elif j < len(nodes) and isinstance(nodes[j], Group) and nodes[j].open.val == "{":
                        r.err(
                            "E_STRICT_MACRO_MOD",
                            f"  {relp}:{n.line}: {where} khai `mod … {{…}}` — "
                            f"đồ thị module không thấy module này nên không xác định được "
                            f"test-only (fail-closed, KT-194/KT-202)",
                        )
                elif j < len(nodes) and _is_punct(nodes[j], "$"):
                    r.err(
                        "E_STRICT_MACRO_INCLUDE",
                        f"  {relp}:{n.line}: {where} khai `mod $…;` — "
                        f"không phân giải tĩnh được (fail-closed, KT-194/KT-202)",
                    )

    check_list(nodes)


def _check_macro_body(
    relp: str, name: str, body: Group, r: Report, shadowed_safe: frozenset = frozenset()
) -> None:
    """**KT-189/B2d+T3c**, **KT-191**, **KT-194**, **KT-202** — thân `macro_rules!` chứa cấu
    trúc có thể sinh `include!`/`#[path]`/`mod` ⇒ fail-closed.

    Hai lớp:
      1. `include!`/`#[path]`/`cfg_attr(path)`/`mod` — dùng chung `_check_macro_passthrough`
         (cùng logic với **đối số lời gọi macro**, KT-202 vá 1);
      2. `$…:<frag>` passthrough với `frag ∈ MACRO_PASSTHROUGH_FRAGS` ⇒ `E_STRICT_MACRO_TT`
         (KT-202 vá 2 — trước đây chỉ `:tt`).
    """
    where = f"thân `macro_rules! {name}`"
    _check_macro_passthrough(relp, where, body.children, r, shadowed_safe)

    def check_frag(nodes: Sequence[object]) -> None:
        for i, n in enumerate(nodes):
            if isinstance(n, Group):
                check_frag(n.children)
                continue
            assert isinstance(n, Tok)
            if n.kind == "punct" and n.val == ":":
                if (
                    i + 1 < len(nodes)
                    and isinstance(nodes[i + 1], Tok)
                    and nodes[i + 1].kind == "ident"
                    and nodes[i + 1].val in MACRO_PASSTHROUGH_FRAGS
                ):
                    r.err(
                        "E_STRICT_MACRO_TT",
                        f"  {relp}:{n.line}: {where} dùng `$…:{nodes[i + 1].val}` "
                        f"(passthrough) — có thể chở `mod`/`include!`/`#[path]` từ **lời gọi** "
                        f"(fail-closed, KT-194/KT-202)",
                    )

    check_frag(body.children)


def _scan_macro_calls(
    nodes: Sequence[object], relp: str, r: Report, shadowed_safe: frozenset = frozenset()
) -> None:
    """**KT-202** (reviewer L40) — quét **đối số lời gọi macro** ở **mọi** tầng nhóm.

    Vì sao không dùng `_walk_items`: nó chỉ đệ quy vào nhóm `{ … }` (để không nhầm `mod` trong
    biểu thức thành khai báo module). Lời gọi macro nằm trong `( … )`/`[…]` — ví dụ
    `let _ = (pass!(include! {"…"}));` — **không** bao giờ được thấy. Đã đo: dạng lồng này
    `cargo check` biên dịch được và checker **xanh** nếu chỉ sửa `_walk_items`
    (`kt181-logs/kt202-nested-checker.log`). Nay quét **toàn cây token**.

    Không đệ quy tiếp vào nhóm của **lời gọi macro** sau khi đã gọi `_check_macro_passthrough`:
    hàm đó đã quét đệ quy toàn bộ nhóm (kể cả lời gọi lồng bên trong) ⇒ tránh báo trùng.
    """
    for i, n in enumerate(nodes):
        if isinstance(n, Group):
            prev = nodes[i - 1] if i > 0 else None
            if _is_punct(prev, "!"):
                # **KT-225** — `stringify!`/`concat!` không mở rộng đối số thành mã ⇒
                # `include!`/`#[path]`/`mod` trong đó không phải dấu hiệu (báo oan
                # `stringify!(include!)` trước KT-225).
                _name = ""
                if i >= 2 and isinstance(nodes[i - 2], Tok) and nodes[i - 2].kind == "ident":
                    _name = nodes[i - 2].val
                if _name not in SAFE_TOKEN_MACROS or _name in shadowed_safe:
                    _check_macro_passthrough(relp, "lời gọi macro", n.children, r, shadowed_safe)
            elif (
                i >= 3
                and isinstance(nodes[i - 1], Tok)
                and nodes[i - 1].kind == "ident"
                and _is_punct(nodes[i - 2], "!")
                and isinstance(nodes[i - 3], Tok)
                and nodes[i - 3].kind == "ident"
                and nodes[i - 3].val == "macro_rules"
            ):
                # Thân `macro_rules!` — `_check_macro_body` đã quét đệ quy; bỏ qua để tránh
                # báo trùng khi thân chứa lời gọi macro có `include!`/`#[path]`/`mod`.
                continue
            else:
                _scan_macro_calls(n.children, relp, r, shadowed_safe)


def scan_macro_bodies(
    root: Path,
    r: Report,
    extra_files: Optional[Iterable[Path]] = None,
    shadowed_safe: frozenset = frozenset(),
    scope: Optional["wake_scope.ProtectedScope"] = None,
) -> None:
    """**KT-171/B4**, **KT-187/B2+B3**, **KT-181**, **KT-256/L2** — quét thân `macro_rules!` ở
    **mọi** tệp nguồn sản phẩm với **cả hai** luật:

    1. **token `wake`** trong thân ⇒ `E_STRICT_MACRO_WAKE`. Reviewer L11 đặt
       `#[macro_export] macro_rules! wake_call` trong `v2/lib/vifast-abi` (ngoài phạm vi quét
       cũ) với thân `$m.wake($t)`; lời gọi `vifast_abi::wake_call!(m, t)` **không** chứa token
       `wake` nên lọt. Token `wake` (không phải `wake_from`/`wake_step`/`wake_addr`) ⇒ ĐỎ,
       trừ `MACRO_WAKE_ALLOW` có lý do.
    2. **`include!`/`#[path]`/`cfg_attr(path)`/`mod`/`$…:<frag>`** trong thân ⇒ `_check_macro_body`
       (fail-closed). Reviewer L25 dựng được hai đường lọt thật: `macro_rules!` định nghĩa ở
       thư mục A (decoy sạch) nhưng **gọi** ở thư mục B; rustc giải `include!`/`#[path]` trong
       thân macro theo **tệp gọi** (đã kiểm bằng `rustc`), còn `resolve_include_targets` giải
       theo **tệp định nghĩa** ⇒ đích thật không bao giờ được quét. Không phân giải tĩnh an
       toàn được ⇒ ĐỎ.

    **KT-181** (D2-164): trước bản này luật (1) chỉ chạy ở `v2/lib` (`scan_lib_macros`), còn
    `v2/kernel`+`v2/sim` chỉ có luật (2) (`scan_kernel_macro_includes`). Hệ quả: thân macro ở
    `v2/kernel/vifast-kbin-riscv64/init/`, `v2/kernel/*/tests/`, `v2/kernel/vifast-plat-user/`
    chứa token `wake` **không** bị luật (1) bắt (luật `wake` toàn văn bản chỉ phủ
    `v2/kernel/vifast-*/src/` + `v2/sim/vifast-sim/src/`). Nay **một** hàm, **một** phạm vi,
    **hai** luật — không còn khoảng trống giữa hai hàm.

    **KT-256 (L2, D2-188)** — gốc quét là `scope.macro_roots()` (định nghĩa duy nhất sinh từ
    `cargo metadata`), không còn hằng `MACRO_SCAN_ROOTS`. `scope=None` ⇒ dựng từ `root`.
    """
    if scope is None:
        scope = wake_scope.protected_scope(root)
    seen: set = set()

    def scan_one(p: Path) -> None:
        if p in seen:
            return
        seen.add(p)
        tree, errs = parse_file(p)
        if tree is None or errs:
            return
        relp = rel(root, p)

        def on_macro(name_tok: Tok, body: Group, cur_dir: Path) -> None:
            name = name_tok.val
            for tok in _iter_tokens(body.children):
                if tok.kind == "ident" and tok.val == "wake":
                    if (relp, name) in MACRO_WAKE_ALLOW:
                        continue
                    r.err(
                        "E_STRICT_MACRO_WAKE",
                        f"  {relp}:{tok.line}: thân `macro_rules! {name}` chứa token "
                        f"`wake` — macro có thể sinh lời gọi `wake()` trong mã sản phẩm "
                        f"(KT-171 B4, KT-181); dùng `wake_from`/`wake_step` hoặc thêm "
                        f"allowlist có lý do",
                    )
            _check_macro_body(relp, name, body, r, shadowed_safe)

        _walk_items(tree, relp, p.parent, on_macro=on_macro, errors=[])

        # **KT-202** — quét **đối số lời gọi macro** ở **mọi** tầng nhóm: `include!`/
        # `#[path]`/`mod` trong đối số không bao giờ được `resolve_include_targets` thấy
        # (nhóm sau `!` bị bỏ qua). Reviewer L40: `macro_rules! pass { ($i:item) => { $i }; }
        # pass!(include! {"…"});` ⇒ `cargo check` biên dịch được, checker **xanh** cả hai
        # chế độ. Dạng lồng trong `( … )` cũng lọt nếu chỉ sửa `_walk_items` (đã đo) ⇒ quét
        # **toàn cây token** bằng `_scan_macro_calls`.
        _scan_macro_calls(tree, relp, r, shadowed_safe)

    for base_rel in scope.macro_roots():
        base = root / base_rel
        if not base.is_dir():
            continue
        # **KT-242 (N4)** — đuôi `.rs` **không phân biệt hoa/thường** (`_iter_rs_ci`).
        for p in _iter_rs_ci(base, root):
            if _is_build_dir(root, p):
                continue
            scan_one(p)

    # **KT-225** (reviewer L52 KT-223 — đường lọt lớp 1 `.txt`) — tệp **không phải `.rs`**
    # được `include!` vào cây (nằm trong `extra` của `resolve_include_targets`) cũng phải
    # quét thân `macro_rules!`: trước KT-225 `rglob("*.rs")` bỏ sót, nên
    # `macro_rules! evil { () => { include!("…") } }` trong `src/macro_def.txt` (được
    # `include!("macro_def.txt")` ở vị trí item) **không** bị `_check_macro_body` bắt ⇒
    # checker tĩnh XANH cả hai chế độ trong khi `cargo check` RC=0 và mã sản phẩm gọi `wake`.
    for p in sorted(extra_files or ()):
        if _is_build_dir(root, p):
            continue
        scan_one(p)


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


def _collect_safe_macro_defs(nodes: Sequence[object], out: set) -> None:
    """**KT-230** — thu **mọi** `macro_rules! <tên-an-toàn>` trong cây token, **mọi tầng**.

    Vì sao đệ quy **mọi** nhóm (không chỉ `{…}` như `_walk_items`): đường lọt #14 (reviewer
    L54, KT-229) định nghĩa `macro_rules! matches` **lồng trong thân** một `macro_rules!` khác
    rồi gọi macro ngoài để expansion sinh định nghĩa. `_walk_items` **bỏ qua thân macro** ⇒
    bản KT-177 không thấy ⇒ tên `matches` vẫn "an toàn" ⇒ `let wake = …; matches!(m, wake, t)`
    được miễn theo local-binding ⇒ **lọt cả hai lớp** (cargo RC=0).

    Nay quét **toàn cây token** — kể cả thân `macro_rules!`, đối số lời gọi macro, mọi
    delimiter `{}`/`()`/`[]` — fail-closed: thấy tên an toàn ở **bất kỳ** đâu là coi như bị che.
    """
    i = 0
    n = len(nodes)
    while i < n:
        node = nodes[i]
        if isinstance(node, Group):
            _collect_safe_macro_defs(node.children, out)
            i += 1
            continue
        if node.kind == "ident" and node.val == "macro_rules":
            j = i + 1
            if j < n and _is_punct(nodes[j], "!"):
                j += 1
            if j < n and isinstance(nodes[j], Tok) and nodes[j].kind == "ident":
                if nodes[j].val in SAFE_VALUE_MACROS:
                    out.add(nodes[j].val)
                j += 1
                if j < n and isinstance(nodes[j], Group):
                    # Đệ quy **thân** macro: định nghĩa lồng (đường lọt #14) nằm ở đây.
                    _collect_safe_macro_defs(nodes[j].children, out)
                    i = j + 1
                    continue
            i += 1
            continue
        i += 1


def _shadowed_safe_macros(
    root: Path,
    files: Sequence[Tuple[Path, str, List[Fn]]],
    extra_files: Optional[Iterable[Path]] = None,
) -> set:
    """**KT-177/G6** (reviewer L16 §2.6, H6) + **KT-230** (reviewer L54, đường lọt #14): tên
    trong `SAFE_VALUE_MACROS` bị **che**.

    `SAFE_VALUE_MACROS` tin **tên** macro là "chỉ đọc giá trị", nhưng `macro_rules! concat`
    trong cây **che** macro chuẩn cùng tên và có thể biến ident thành **tên hàm**
    (`$m.$name($t)`). Khi đó `wake` là đối số macro "an toàn" ⇒ nhánh `SAFE_VALUE_MACROS` bỏ
    qua ⇒ rơi xuống `_exempt_local_wake` ⇒ biến cục bộ `wake` cùng khối miễn ⇒ **lọt**.

    Nay: quét **toàn cây** (`v2/kernel` + `v2/sim` + `v2/lib` + tệp `include!`/`#[path]` đích)
    tìm `macro_rules! <tên>` trùng tên an toàn; tên nào bị che thì **không** còn an toàn
    (fail-closed). Chỉ **siết**: trên cây sạch hiện không có `macro_rules!` nào trùng tên an
    toàn (chỉ `sctlr_setters`, `bail`).

    **KT-230** — hai lỗ của bản KT-177:
      - `_walk_items` **bỏ qua thân `macro_rules!`** ⇒ định nghĩa **lồng** (macro sinh macro)
        không bị thấy. Nay [`_collect_safe_macro_defs`] đệ quy **mọi** nhóm, mọi tầng.
      - chỉ quét `.rs` ⇒ tệp `.txt`/`.RS`/không đuôi được `include!` vào `v2/lib`/`v2/kernel`
        định nghĩa macro che tên an toàn **không** bị thấy (lớp 2 may bắt được vì dùng danh
        sách tệp rustc nạp; lớp 1 thì không). Nay nhận thêm `extra_files` — lớp 1 truyền
        đích `include!`/`#[path]` đã phân giải.
    """
    out: set = set()
    texts: List[Tuple[Path, str]] = [(p, t) for p, t, _ in files]
    base = root / "v2/lib"
    if base.is_dir():
        # **KT-242 (N4)** — đuôi `.rs` **không phân biệt hoa/thường** (`_iter_rs_ci`).
        for p in _iter_rs_ci(base, root):
            if _is_build_dir(root, p):
                continue
            try:
                texts.append((p, p.read_text(encoding="utf-8", errors="replace")))
            except OSError:
                continue
    for p in sorted(extra_files or ()):
        if _is_build_dir(root, p):
            continue
        try:
            texts.append((p, p.read_text(encoding="utf-8", errors="replace")))
        except OSError:
            continue
    for p, text in texts:
        if "macro_rules!" not in text:
            continue
        tree, errs = parse_file(p)
        if tree is None or errs:
            continue
        _collect_safe_macro_defs(tree, out)
    return out


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
    """Mọi tệp `.rs` dưới các thư mục — **KT-242 (N4)**: đuôi không phân biệt hoa/thường."""
    for relp_dir in rel_dirs:
        base = root / relp_dir
        if not base.is_dir():
            continue
        for p in _iter_rs_ci(base, root):
            if _is_build_dir(root, p):
                continue
            yield p


def rel(root: Path, p: Path) -> str:
    try:
        return str(p.relative_to(root))
    except ValueError:
        return str(p)


# ------------------------------------------------------------------ kết quả

class _Tee:
    """**KT-333e** — report chuyển tiếp: ghi **cùng một** lỗi/cảnh báo/ghi chú vào nhiều
    `Report` (dùng cho luật **chung** ở chế độ gộp một lượt quét). Không có trạng thái riêng."""

    def __init__(self, *reps: "Report") -> None:
        self._reps = reps

    def err(self, code: str, msg: str) -> None:
        for rep in self._reps:
            rep.err(code, msg)

    def warn(self, code: str, msg: str) -> None:
        for rep in self._reps:
            rep.warn(code, msg)

    def note(self, msg: str) -> None:
        for rep in self._reps:
            rep.note(msg)


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

def _item_end(toks: Sequence[Tok], start: int) -> Optional[int]:
    """Chỉ số token **cuối** của mục bắt đầu tại `start` (`;` hoặc `}` khớp, mức 0)."""
    n = len(toks)
    depth = 0
    i = start
    while i < n:
        t = toks[i]
        if t.val in "([{":
            depth += 1
        elif t.val in ")]}":
            depth -= 1
            if depth == 0 and t.val == "}":
                return i
        elif t.val == ";" and depth == 0:
            return i
        i += 1
    return None


# **KT-242 (N5)** — cache kết quả phân vùng test theo `(relp, độ dài, hash)`.
# `scan_v2` cần vùng test cho **mọi** luật, còn `_check_wake_text` (R1) tự tính lại; trước bản
# này mỗi tệp bị token hoá **hai** lần. Cache chỉ là bộ nhớ đệm thuần (không đổi ngữ nghĩa):
# khoá gồm `relp` + độ dài + hash nội dung, nên hai nội dung khác nhau không thể trùng khoá
# trong thực tế; selftest dựng tệp mới ở đường dẫn mới nên không lẫn.
_TEST_RANGE_CACHE: Dict[Tuple[str, int, int], List[Tuple[int, int]]] = {}


def _test_char_ranges(code: str, relp: str) -> List[Tuple[int, int]]:
    """Như [`_test_char_ranges_uncached`], có **cache** theo `(relp, độ dài, hash)`."""
    key = (relp, len(code), hash(code))
    cached = _TEST_RANGE_CACHE.get(key)
    if cached is not None:
        return cached
    out = _test_char_ranges_uncached(code, relp)
    _TEST_RANGE_CACHE[key] = out
    return out


def _test_char_ranges_uncached(code: str, relp: str) -> List[Tuple[int, int]]:
    """**KT-198/F2b** (reviewer L35) — khoảng **ký tự** của các mục test, theo **token-tree**.

    Bản cũ `test_line_ranges` quyết định theo **dòng**: `#[cfg(test)] fn h() { … } pub fn evil(…)`
    cùng dòng ⇒ cả `evil` bị coi là test ⇒ luật `wake` bỏ qua ⇒ **lọt** (cargo RC=0). Nay mỗi
    `#[cfg(test)]` chỉ đánh dấu **đúng mục nó gắn vào** (từ mục tới `}` khớp).

    **KT-219** (L49, đường lọt #13) — bản KT-198 chỉ dừng vòng quét ở `;` và `{`, nên với mục
    **không có thân `{…}`** mà kết thúc bằng **dấu `,`** (nhánh `match` `A => (),`, field struct,
    biến thể enum) vùng test chạy tiếp sang mục kế và lấy thân `{…}` của **mục sản phẩm** làm
    thân của mục test. Đo được: `#[cfg(test)] A => (),` + nhánh `B => { m.wake(t); }` ⇒
    `_test_char_ranges` trả vùng phủ **cả** `m.wake(t)` ⇒ **cả hai lớp** (lớp 2 dùng lại chính
    hàm này) XANH trong khi `cargo check` RC=0. Nay vùng test kết thúc ở **cuối mục thật**:
    thân `{…}` nếu có, ngược lại `;` hoặc `,` **cùng mức**. Dấu `,` chỉ là mốc kết thúc khi đã
    thấy `=>` (nhánh `match`) — `,` còn xuất hiện trong generic params (`fn f<T, U>`),
    `use a::b, c::d;`, field struct, biến thể enum: các ngữ cảnh đó **không chứa được lời gọi
    `wake`**, cắt sớm chỉ làm vùng test **hẹp** hơn (báo oan), không làm rộng (lọt).
    """
    toks, errs = tokenize(code, relp)
    if errs:
        return []
    try:
        tree = build_tree(toks, relp)
    except LexError:
        return []
    out: List[Tuple[int, int]] = []

    def walk(nodes: Sequence[object], in_test: bool, depth: int) -> None:
        if depth > MAX_TREE_DEPTH:
            return
        pending: List[Attr] = []
        i = 0
        n = len(nodes)
        while i < n:
            node = nodes[i]
            if isinstance(node, Group):
                prev = nodes[i - 1] if i > 0 else None
                if _is_punct(prev, "!"):
                    i += 1
                    pending = []
                    continue
                if node.open.val == "{":
                    walk(node.children, in_test, depth + 1)
                i += 1
                pending = []
                continue
            if node.kind == "punct" and node.val == "#":
                j = i + 1
                if j < n and _is_punct(nodes[j], "!"):
                    j += 1
                if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "[":
                    try:
                        attr = parse_attr(nodes[j], relp)
                    except LexError:
                        i = j + 1
                        pending = []
                        continue
                    pending.append(attr)
                    i = j + 1
                    continue
                pending = []
                i += 1
                continue
            if node.kind == "ident":
                # **KT-219** — tìm **cuối mục** đúng: thân `{…}` (mục có thân) **hoặc** `;`/`,`
                # cùng mức (mục không thân). Trước bản này vòng quét chỉ dừng ở `;` và `{`,
                # nên `#[cfg(test)]` trên **nhánh `match`** `A => (),` (kết thúc bằng `,`)
                # quét tiếp sang nhánh kế và lấy thân `{…}` của nhánh **sản phẩm** làm thân
                # của mục test ⇒ nuốt `m.wake(t)` (đường lọt #13, lọt **cả hai** lớp vì
                # `check-wake-depinfo.py` dùng lại chính hàm này).
                k = i + 1
                body: Optional[int] = None
                term: Optional[int] = None
                seen_arrow = False
                while k < n:
                    nd = nodes[k]
                    if isinstance(nd, Group):
                        if nd.open.val == "{":
                            body = k
                            break
                        k += 1
                        continue
                    if _is_punct(nd, ";"):
                        term = k
                        break
                    if _is_punct(nd, ",") and seen_arrow:
                        term = k
                        break
                    if (
                        _is_punct(nd, "=")
                        and k + 1 < n
                        and _is_punct(nodes[k + 1], ">")
                    ):
                        seen_arrow = True
                    k += 1
                is_test = in_test or any(
                    a.name == "cfg" and cfg_is_test_tokens(a.args) for a in pending
                )
                if body is not None:
                    if is_test:
                        out.append((node.start, nodes[body].close.end))
                    walk(nodes[body].children, is_test, depth + 1)
                    i = body + 1
                    pending = []
                    continue
                if term is not None:
                    if is_test:
                        out.append((node.start, nodes[term].end))
                    i = term + 1
                    pending = []
                    continue
            pending = []
            i += 1

    walk(tree, False, 0)
    return out


def _check_wake_text(
    relp: str,
    text: str,
    safe_macros: set,
    r: Report,
) -> None:
    """**KT-155/KT-165/KT-171/KT-194** — luật token `wake` trên **một** tệp sản phẩm.

    Tách khỏi `scan_v2` để dùng chung cho **hai** đường:
      (i) mọi tệp sản phẩm ở `--strict` (KT-155);
      (ii) tệp **test-only bị kéo vào sản phẩm** bằng `#[path]`/`include!` ở chế độ thường
           (D2-166 §3C — không âm thầm bỏ qua cạnh nguồn ẩn; trước KT-194 đường này chỉ
           được luật `--strict` bắt nên chế độ thường xanh).

    **KT-241** — **bỏ** tham số `tr` (`test_line_ranges`). Trước đây caller truyền
    `tr = test_line_ranges(text)` nhưng hàm **bỏ qua** và tự tính `_test_char_ranges(code, relp)`
    (phân vùng theo **token-tree**). Hai cách phân vùng khác nhau ⇒ người đọc tưởng
    `test_line_ranges` còn hiệu lực trong luật `wake`, và một caller mới có thể tưởng mình
    đang điều khiển vùng test. Nay **một** nơi sở hữu phân vùng: hàm này tự tính
    `_test_char_ranges`; chữ ký không còn tham số để truyền sai. Ca selftest
    "KT-241 phân vùng" dựng đúng ca hai cách phân vùng **khác nhau** và khẳng định kết quả
    theo cách của callee.
    """
    code = strip_rust_noise(text)
    # **KT-198/F2b**: ngữ cảnh test theo **token-tree** (khoảng ký tự), không theo dòng.
    test_ranges = _test_char_ranges(code, relp)
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
        if in_ranges(s, test_ranges):
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
            if macro is not None and macro not in safe_macros:
                why = (
                    f"`wake` là đối số của macro `{macro}!` — macro có thể biến "
                    f"ident thành tên hàm (KT-171 B10"
                    + (
                        ""
                        if macro not in SAFE_VALUE_MACROS
                        else f"; tên `{macro}` bị `macro_rules!` trong cây che — "
                        f"KT-177 H6"
                    )
                    + ")"
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


def _walk_nodes(nodes: Sequence[object]) -> Iterable[Tuple[object, Sequence[object], int]]:
    """Duyệt **mọi** tầng token-tree: `(node, danh sách anh em, chỉ số)`.

    Cần "anh em" (không chỉ node) để phân biệt **định nghĩa** (`fn write_state`) với **lời
    gọi** (`self.write_state(`) và **dùng làm giá trị** (`let f = Self::write_state;`).
    """
    for i, n in enumerate(nodes):
        yield n, nodes, i
        if isinstance(n, Group):
            yield from _walk_nodes(n.children)


def _transition_calls(text: str, relp: str) -> List[Tuple[int, str, str, int]]:
    """**KT-241** — mọi lời gọi `transition_tcb(…)`; **chỉ** kiểm đối số thứ hai có phải
    `TcbState`/`Ready` không (chốt chống **đảo kiểu**), không phân tích Ready/không-Ready.

    Trả `[(dòng, dạng, mô_tả, offset_ký_tự)]` — `offset` là vị trí **ký tự** của token
    `transition_tcb` trong `text`, để caller lọc vùng test theo **khoảng ký tự**
    (`_test_char_ranges`) thay vì theo dòng (**KT-242 N5**). Dạng:
      - `"tcbstate"` — đối số thứ hai là/chứa `TcbState` ⇒ **ĐỎ**: dưới API KT-241
        (`to: NonReadyTarget`) dạng này **không biên dịch được** ⇒ có người đã **đảo kiểu**
        `transition_tcb` về `TcbState`;
      - `"ready_alias"` — đối số thứ hai kết thúc bằng/chứa `Ready` nhưng không phải
        `TcbState::Ready` (`S::Ready`, `Self::Ready`, `NonReadyTarget::Ready`) ⇒ **ĐỎ**:
        `NonReadyTarget` **không có** biến thể `Ready`, nên đây là alias của `TcbState` hoặc
        biến thể bịa ⇒ cùng lớp "đảo kiểu";
      - `"ok"` — mọi dạng khác (`NonReadyTarget::*`, biến, lời gọi hàm, closure, …) ⇒ **XANH**:
        kiểu `NonReadyTarget` đã bảo đảm không phải `Ready`; kiểm thêm là **suy luận lại điều
        hệ thống kiểu đã bảo đảm** (D2-194 §3).

    Vì sao **bỏ** `_var_may_be_ready`: nó tồn tại để trả lời "biến này có thể là `Ready`
    không" — câu hỏi **không còn nghĩa** khi tham số là `NonReadyTarget`. Giữ nó là giữ một
    mô hình dataflow-lite **không đầy đủ** (không mô hình hoá call/const/closure/liên hàm) làm
    người đọc tưởng checker chứng minh được nhiều hơn thực tế.

    Vì sao vẫn quét token-tree (không regex văn bản): đường lọt #15 (reviewer L54, KT-229) —
    `RE_READY_TRANSITION` khớp **một dòng**; `use crate::TcbState as S;` +
    `m.transition_tcb(t, S::Ready)` XANH cả hai lớp. Token-tree thấy đúng **đối số thứ hai**,
    không phụ thuộc alias/xuống dòng/đường dẫn, và nhận đúng UFCS (`M::transition_tcb(m, t, s)`
    — đối số trạng thái là args[2]).
    """
    toks, errs = tokenize(text, relp)
    if errs:
        return []
    try:
        tree = build_tree(toks, relp)
    except LexError:
        return []
    out: List[Tuple[int, str, str, int]] = []

    def segs_of(arg: Sequence[object]) -> List[str]:
        """Các **đoạn định danh** của một đường dẫn biến thể (`a::b::C` ⇒ `[a, b, C]`)."""
        segs: List[str] = []
        i = 0
        n = len(arg)
        while i < n:
            t = arg[i]
            if isinstance(t, Group):
                break
            if t.kind == "ident":
                segs.append(t.val)
                i += 1
                continue
            if t.kind == "punct" and t.val == ":" and i + 1 < n and _is_punct(arg[i + 1], ":"):
                i += 2
                continue
            break
        return segs

    def walk(nodes: Sequence[object]) -> None:
        for i, node in enumerate(nodes):
            if isinstance(node, Group):
                walk(node.children)
                continue
            if not (node.kind == "ident" and node.val == "transition_tcb"):
                continue
            if (
                i + 1 >= len(nodes)
                or not isinstance(nodes[i + 1], Group)
                or nodes[i + 1].open.val != "("
            ):
                continue
            prev = nodes[i - 1] if i > 0 else None
            if prev is not None and getattr(prev, "kind", None) == "ident" and prev.val == "fn":
                continue  # định nghĩa `fn transition_tcb(`
            # **KT-239 (WB-17 biến thể 6/UFCS)** — `M::transition_tcb(m, t, Ready)`: định danh
            # đứng sau `::` là lời gọi hàm liên kết, KHÔNG phải method `.transition_tcb(`.
            # Đối số thứ hai ở văn bản (`t`) là ObjId, trạng thái thật nằm ở args[2].
            is_ufcs = (
                i >= 2
                and _is_punct(nodes[i - 1], ":")
                and _is_punct(nodes[i - 2], ":")
            )
            args = _split_commas(nodes[i + 1].children)
            if is_ufcs:
                # UFCS method `Type::transition_tcb(m, t, state)`: args[0]=receiver.
                if len(args) < 3:
                    out.append((node.line, "ok", "UFCS thiếu đối số trạng thái (args[2])", node.start))
                    continue
                arg = args[2]
            else:
                if len(args) < 2:
                    out.append((node.line, "ok", "thiếu đối số thứ hai", node.start))
                    continue
                arg = args[1]
            segs = segs_of(arg)
            # Quét **cả** đối số (kể cả trong nhóm `{ … }`/`( … )`) để không lọt
            # `transition_tcb(t, { TcbState::Ready })`; `NonReadyTarget` không có `Ready`
            # nên không có dương tính giả trên mã hợp lệ.
            flat = list(_iter_tokens(arg))
            has_tcb = any(t.kind == "ident" and t.val == "TcbState" for t in flat)
            has_ready = any(t.kind == "ident" and t.val == "Ready" for t in flat)
            if has_tcb:
                out.append((node.line, "tcbstate", "::".join(segs) or "biểu thức chứa `TcbState`", node.start))
            elif has_ready:
                out.append(
                    (
                        node.line,
                        "ready_alias",
                        "::".join(segs) or "biểu thức chứa `Ready`",
                        node.start,
                    )
                )
            else:
                out.append((node.line, "ok", "::".join(segs) or "biểu thức", node.start))

    walk(tree)
    return out


# ------------------------------------------------------------------ KT-241: mỏ neo kiểu
#
# **D2-194 §3** — "đơn giản hoá theo kiểu, **không tin tên hàm**". Sau khi `transition_tcb`
# nhận `NonReadyTarget`, luật "đối số thứ hai có phải `Ready`" **không còn nghĩa**. Nhưng
# **không** được thay bằng "thấy `transition_tcb(` là mặc nhiên an toàn": nếu ai đó **đảo
# kiểu** (đổi `to: NonReadyTarget` về `to: TcbState`, thêm `impl From<TcbState> for
# NonReadyTarget`, nới `write_state` thành `pub(crate)`, …) thì đường lọt cũ sống dậy mà
# checker vẫn XANH. Vì vậy checker đọc **định nghĩa** (chữ ký, biến thể enum, mức công khai)
# và **ĐỎ khi thiếu** (`E_ANCHOR_MISSING`) hoặc **sai** (`E_ANCHOR_TYPE`) — fail-closed.

_MODIFIERS = {"async", "unsafe", "const", "extern", "default"}


def _fn_visibility(prev_tokens: Sequence[object]) -> str:
    """Mức công khai của một `fn` suy từ các token **đứng trước** nó cùng mức.

    Trả `"pub"` cho `pub fn`, `pub(crate) fn`, `pub(super) fn`, `pub(in path) fn` (và các
    biến thể có modifier `async`/`unsafe`/`const`/`extern`/`default` chen giữa); `"private"`
    cho `fn`.
    """
    i = len(prev_tokens) - 1
    while i >= 0:
        t = prev_tokens[i]
        if (
            isinstance(t, Group)
            and t.open.val == "("
            and i >= 1
            and _is_ident(prev_tokens[i - 1], "pub")
        ):
            return "pub"
        if isinstance(t, Tok) and t.kind == "ident" and t.val in _MODIFIERS:
            i -= 1
            continue
        if isinstance(t, Tok) and t.kind == "str":  # `extern "C"`
            i -= 1
            continue
        break
    if i >= 0 and _is_ident(prev_tokens[i], "pub"):
        return "pub"
    return "private"


def _fn_defs(
    nodes: Sequence[object],
    out: List[Tuple[str, Group, List[object], List[object], int]],
) -> None:
    """Thu **mọi** định nghĩa `fn` trong token-tree.

    Mỗi mục: `(tên, nhóm tham số, token trước `fn` cùng mức, token sau nhóm tham số tới
    `{`/`;`, dòng)`. Dùng cho mỏ neo kiểu (chữ ký + mức công khai + kiểu trả về).
    """
    for i, n in enumerate(nodes):
        if isinstance(n, Group):
            _fn_defs(n.children, out)
            continue
        if not _is_ident(n, "fn"):
            continue
        if i + 1 >= len(nodes) or not _is_ident(nodes[i + 1]):
            continue
        name = nodes[i + 1].val
        j = i + 2
        while j < len(nodes) and not (
            isinstance(nodes[j], Group) and nodes[j].open.val == "("
        ):
            j += 1
        if j >= len(nodes):
            continue
        params = nodes[j]
        assert isinstance(params, Group)
        after: List[object] = []
        k = j + 1
        while k < len(nodes):
            x = nodes[k]
            if isinstance(x, Group) and x.open.val == "{":
                break
            if _is_punct(x, ";"):
                break
            after.append(x)
            k += 1
        out.append((name, params, list(nodes[:i]), after, n.line))


def _param_types(params: Group) -> List[Optional[List[object]]]:
    """Kiểu của từng tham số **sau `self`** (`None` cho tham số không có `:`).

    **KT-241** — "tham số thứ hai" trong đặc tả là tham số thứ hai **của lời gọi**
    (`transition_tcb(tcb, to)`), tức tham số thứ ba của **method** (`&mut self, tcb, to`).
    Vì vậy `self`/`&self`/`&mut self`/`self: …` bị **loại** khỏi danh sách: nếu không, mỏ neo
    sẽ đọc `tcb: ObjId` làm "tham số thứ hai" và **ĐỎ oan** trên mọi method.
    """
    out: List[Optional[List[object]]] = []
    for part in _split_commas(params.children):
        if not part:
            continue
        if any(t.kind == "ident" and t.val == "self" for t in _iter_tokens(part)):
            continue  # `self`, `&self`, `&mut self`, `self: Box<Self>`
        idx = None
        for k, x in enumerate(part):
            if _is_punct(x, ":"):
                idx = k
                break
        out.append(None if idx is None else list(part[idx + 1 :]))
    return out


def _type_has(tokens: Optional[Sequence[object]], name: str) -> bool:
    """Kiểu có chứa định danh `name` không (`&mut NonReadyTarget` ⇒ có)."""
    if not tokens:
        return False
    return any(t.kind == "ident" and t.val == name for t in _iter_tokens(tokens))


def _type_text(tokens: Optional[Sequence[object]]) -> str:
    """Chuỗi kiểu để đưa vào thông điệp lỗi."""
    if not tokens:
        return "?"
    return " ".join(t.val for t in _iter_tokens(tokens))


def _find_enum(nodes: Sequence[object], name: str) -> Optional[Group]:
    """Thân `{ … }` của `enum <name>` ở **bất kỳ** tầng nào."""
    for i, n in enumerate(nodes):
        if isinstance(n, Group):
            g = _find_enum(n.children, name)
            if g is not None:
                return g
            continue
        if (
            _is_ident(n, "enum")
            and i + 2 < len(nodes)
            and _is_ident(nodes[i + 1], name)
            and isinstance(nodes[i + 2], Group)
            and nodes[i + 2].open.val == "{"
        ):
            return nodes[i + 2]
    return None


def _check_type_anchors(
    root: Path,
    files: Sequence[Tuple[Path, str, List[Fn]]],
    r: Report,
) -> None:
    """**KT-241** — mỏ neo **kiểu** (fail-closed): đọc **định nghĩa**, không tin tên hàm.

    Kiểm cho **cả hai** crate (core, sim):
      1. `state_write.rs` tồn tại, có `enum NonReadyTarget` **không** biến thể `Ready`;
      2. `transition_tcb` có tham số thứ hai kiểu `NonReadyTarget` (không `TcbState`);
      3. `block_current` có tham số thứ hai kiểu `NonReadyTarget`;
      4. `write_state` và `into_state` là `fn` **private** (không `pub`, không `pub(crate)`);
      5. `state_write.rs` có đủ 5 cửa `Ready` + `tcb_yield` + `transition_tcb`;
      6. `core_tcb.rs` / **cả bộ** tệp `kobj` (KT-336b) **không** còn `fn transition_tcb(`
         (cửa cũ đã chuyển đi);
      7. không `impl From/TryFrom/Into<TcbState> for NonReadyTarget` và không `pub fn` nào
         nhận `TcbState` trả `NonReadyTarget` (trong **mọi** tệp của crate).
    """
    by_rel = {rel(root, p): (p, text, fns) for p, text, fns in files}
    for sw in STATE_WRITE_FILES:
        p = root / sw
        if not p.is_file():
            r.err("E_ANCHOR_MISSING", f"{sw}: không có tệp mỏ neo `state_write` (KT-241)")
            continue
        tree, errs = parse_file(p)
        if tree is None or errs:
            r.err(
                "E_ANCHOR_MISSING",
                f"{sw}: không token hoá được để kiểm mỏ neo kiểu (KT-241)",
            )
            continue
        defs: List[Tuple[str, Group, List[object], List[object], int]] = []
        _fn_defs(tree, defs)
        by_name: Dict[str, List[Tuple[str, Group, List[object], List[object], int]]] = {}
        for d in defs:
            by_name.setdefault(d[0], []).append(d)

        # 1) enum NonReadyTarget — **không** biến thể `Ready`.
        enum_body = _find_enum(tree, "NonReadyTarget")
        if enum_body is None:
            r.err("E_ANCHOR_MISSING", f"{sw}: thiếu `enum NonReadyTarget` (KT-241)")
        elif any(_is_ident(c, "Ready") for c in enum_body.children):
            r.err(
                "E_ANCHOR_TYPE",
                f"{sw}: `NonReadyTarget` có biến thể `Ready` — API đã bị đảo (KT-241)",
            )

        # 2) `transition_tcb(tcb, to: NonReadyTarget)`.
        ds = by_name.get("transition_tcb", [])
        if not ds:
            r.err("E_ANCHOR_MISSING", f"{sw}: thiếu `fn transition_tcb` (KT-241)")
        for _n, params, _prev, _after, line in ds:
            types = _param_types(params)
            if len(types) < 2 or not _type_has(types[1], "NonReadyTarget"):
                got = "?" if len(types) < 2 else _type_text(types[1])
                r.err(
                    "E_ANCHOR_TYPE",
                    f"{sw}:{line}: `transition_tcb` tham số thứ hai phải là `NonReadyTarget` "
                    f"(đang là `{got}`) — API KT-241 (D2-194)",
                )

        # 3) `write_state`/`into_state` **private**.
        for fname in ("write_state", "into_state"):
            ds = by_name.get(fname, [])
            if not ds:
                r.err("E_ANCHOR_MISSING", f"{sw}: thiếu `fn {fname}` (KT-241)")
                continue
            for _n, _params, prev, _after, line in ds:
                if _fn_visibility(prev) != "private":
                    r.err(
                        "E_ANCHOR_API",
                        f"{sw}:{line}: `{fname}` phải là `fn` **private** (không `pub`/"
                        f"`pub(crate)`/`pub(super)`) — ranh giới KT-241 (D2-187 §3)",
                    )

        # 4) Đủ cửa: 5 cửa `Ready` + `tcb_yield` + cửa thông thường.
        for door in READY_DOORS + ("transition_tcb",):
            if door not in by_name:
                r.err("E_ANCHOR_MISSING", f"{sw}: thiếu cửa `fn {door}` (KT-241)")

        # 5) `block_current` — tham số thứ hai `NonReadyTarget`, **chỉ** ở tệp **định nghĩa**
        #    API (`core_sched.rs` / một tệp miền của `kobj` — KT-336b: `sched.rs`).
        #
        #    **KT-241 (lead, sau hợp nhất)** — bản C quét **mọi** tệp của crate, nên báo oan
        #    `kt59_core_parity.rs:720/987`: đó là **trait impl** của harness parity, nhận `CS`
        #    (kiểu trạng thái trừu tượng) rồi đổi qua `sim_nonready()`/`core_nonready()` —
        #    adapter hợp lệ, không phải định nghĩa API. Mỏ neo chỉ có nghĩa ở **nơi định nghĩa**;
        #    nếu `block_current` bị chuyển sang tệp khác thì `E_ANCHOR_MISSING` bắt (fail-closed).
        #    Tệp **định nghĩa** API ở cây thật là `core_sched.rs` / tệp miền `kobj` (KT-336b);
        #    cây fixture của
        #    selftest đặt `block_current` **trong** `state_write.rs` ⇒ kiểm **cả hai** (đúng
        #    nơi định nghĩa, không quét `tests/`).
        bc_files = (sw, KERNEL_SCHED) if sw == KERNEL_STATE_WRITE else (sw, *SIM_KOBJ_FILES)
        found_bc = False
        for relp in bc_files:
            fp = root / relp
            if not fp.is_file():
                continue
            tree2, errs2 = parse_file(fp)
            if tree2 is None or errs2:
                continue
            defs2: List[Tuple[str, Group, List[object], List[object], int]] = []
            _fn_defs(tree2, defs2)
            for name, params, _prev, _after, line in defs2:
                if name != "block_current":
                    continue
                found_bc = True
                types = _param_types(params)
                if len(types) < 2 or not _type_has(types[1], "NonReadyTarget"):
                    got = "?" if len(types) < 2 else _type_text(types[1])
                    r.err(
                        "E_ANCHOR_TYPE",
                        f"{relp}:{line}: `block_current` tham số thứ hai phải là "
                        f"`NonReadyTarget` (đang là `{got}`) — API KT-241 (D2-194 §1(d))",
                    )
        if not found_bc:
            r.err(
                "E_ANCHOR_MISSING",
                f"{bc_files[0]}: thiếu `fn block_current` (KT-241)",
            )

    # 6) Tệp cũ **không** được còn định nghĩa `transition_tcb`.
    for legacy in LEGACY_TRANSITION_FILES:
        p = root / legacy
        if not p.is_file():
            continue
        tree, errs = parse_file(p)
        if tree is None or errs:
            continue
        defs: List[Tuple[str, Group, List[object], List[object], int]] = []
        _fn_defs(tree, defs)
        for name, _params, _prev, _after, line in defs:
            if name == "transition_tcb":
                r.err(
                    "E_ANCHOR_TYPE",
                    f"{legacy}:{line}: còn `fn transition_tcb` — cửa cũ phải chuyển sang "
                    f"`state_write.rs` (KT-241)",
                )

def _check_nonready_conv_file(relp: str, text: str, r: Report) -> None:
    """**KT-241 (E1/E2)** — cấm **mọi** phép chuyển `TcbState` ⇄ `NonReadyTarget` ngoài
    `into_state` private.

    Vì sao cần luật **riêng**, không chỉ dựa vào mỏ neo chữ ký: orphan rule **cho phép**
    `impl From<TcbState> for NonReadyTarget` **bên trong** `vifast-core` (cả hai kiểu đều
    local) ⇒ `cargo check` **RC=0** ⇒ `NonReadyTarget::from(TcbState::Ready)` /
    `TcbState::Ready.into()` biên dịch được ⇒ **đường vòng tái tạo** qua ranh giới kiểu mà
    **không** đụng `transition_tcb`. API-SPEC §1 nói rõ: "**Không** có `From`/`TryFrom` công
    khai nào từ `TcbState` sang kiểu này — đó là toàn bộ ranh giới."

    Bắt **bốn** dạng:
      1. `impl From<TcbState> for NonReadyTarget` / `TryFrom<…>` / `Into<NonReadyTarget> for
         TcbState` — trait impl có **cả hai** kiểu trong đầu `impl`;
      2. `impl NonReadyTarget { fn …(s: TcbState) -> Self }` — inherent impl trên **đích**,
         method nhận **nguồn** trả `Self`;
      3. `impl TcbState { fn …(self) -> NonReadyTarget }` — inherent impl trên **nguồn**,
         method trả **đích**;
      4. `pub fn …(s: TcbState) -> NonReadyTarget` — hàm **công khai** (free fn hoặc trong
         impl bất kỳ) chuyển nguồn → đích.
    """
    toks, lerrs = tokenize(text, relp)
    if lerrs:
        return
    try:
        tree = build_tree(toks, relp)
    except LexError:
        return
    _check_nonready_conv_tree(relp, tree, r)


def _check_nonready_conv_tree(relp: str, tree: Sequence[object], r: Report) -> None:
    """Phần **thuần** của [`_check_nonready_conv_file`] trên token-tree đã dựng (dùng cache
    `parse_file` ở đường quét cả cây — tránh token hoá lần thứ ba)."""
    for n, sibs, i in _walk_nodes(tree):
        if not _is_ident(n, "impl"):
            continue
        head: List[object] = []
        j = i + 1
        while j < len(sibs) and not (
            isinstance(sibs[j], Group) and sibs[j].open.val == "{"
        ):
            head.append(sibs[j])
            j += 1
        body = sibs[j] if j < len(sibs) and isinstance(sibs[j], Group) else None
        idents = {t.val for t in _iter_tokens(head) if t.kind == "ident"}
        conv = idents & {"From", "TryFrom", "Into"}
        if conv and "TcbState" in idents and "NonReadyTarget" in idents:
            r.err(
                "E_STRICT_NONREADY_CONV",
                f"  {relp}:{n.line}: `impl …{sorted(conv)[0]}…` chuyển `TcbState` ⇄ "
                f"`NonReadyTarget` — ranh giới KT-241 (chỉ `into_state` private được đổi; "
                f"API-SPEC §1)",
            )
            continue
        if body is None or "for" in idents:
            continue
        defs: List[Tuple[str, Group, List[object], List[object], int]] = []
        _fn_defs(body.children, defs)
        if "NonReadyTarget" in idents:
            # inherent impl trên **đích**: method nhận `TcbState` trả `Self`
            for name, params, _prev, after, line in defs:
                types = _param_types(params)
                if not any(_type_has(t, "TcbState") for t in types):
                    continue
                if _type_has(after, "Self"):
                    r.err(
                        "E_STRICT_NONREADY_CONV",
                        f"  {relp}:{line}: `impl NonReadyTarget` có `fn {name}` nhận "
                        f"`TcbState` trả `Self` — ranh giới KT-241 (API-SPEC §1)",
                    )
        elif "TcbState" in idents:
            # inherent impl trên **nguồn**: method trả `NonReadyTarget`
            for name, _params, _prev, after, line in defs:
                if _type_has(after, "NonReadyTarget"):
                    r.err(
                        "E_STRICT_NONREADY_CONV",
                        f"  {relp}:{line}: `impl TcbState` có `fn {name}` trả "
                        f"`NonReadyTarget` — ranh giới KT-241 (API-SPEC §1)",
                    )
    # Hàm **công khai** (free fn hoặc trong impl bất kỳ) nhận `TcbState` trả `NonReadyTarget`.
    # API-SPEC §1: "**không** … hàm công khai nào trả `NonReadyTarget` từ `TcbState`".
    all_defs: List[Tuple[str, Group, List[object], List[object], int]] = []
    _fn_defs(tree, all_defs)
    for name, params, prev, after, line in all_defs:
        if _fn_visibility(prev) != "pub":
            continue
        if not _type_has(after, "NonReadyTarget"):
            continue
        types = _param_types(params)
        if any(_type_has(t, "TcbState") for t in types):
            r.err(
                "E_STRICT_NONREADY_CONV",
                f"  {relp}:{line}: `pub fn {name}` nhận `TcbState` trả `NonReadyTarget` — "
                f"ranh giới KT-241 (API-SPEC §1)",
            )


def _check_nonready_conv(root: Path, r: Report) -> None:
    """Áp [`_check_nonready_conv_file`] trên **toàn bộ** `v2/kernel/**` + `v2/sim/**` +
    `v2/lib/**`.

    **Vì sao rộng hơn phạm vi luật `wake`**: `v2/lib` hiện **không** phụ thuộc `vifast-core`
    (nên không thể viết `impl From<TcbState> …`), nhưng nếu mai kia có phụ thuộc thì impl sẽ
    nằm ở đó — quét sẵn là **fail-closed**, không phải báo oan (không có `TcbState`/
    `NonReadyTarget` trong `v2/lib` thì không có gì để bắt).
    """
    for p in iter_rs(root, ("v2/kernel", "v2/sim", "v2/lib")):
        # `parse_file` có cache theo `(đường dẫn, mtime, kích thước)` — không token hoá lại
        # lần thứ ba cho cùng tệp (hai lượt trước: `_transition_calls`, `_check_state_write`).
        tree, errs = parse_file(p)
        if tree is None or errs:
            continue
        _check_nonready_conv_tree(rel(root, p), tree, r)


def _fn_body_groups(nodes: Sequence[object], out: List[Tuple[str, Group]]) -> None:
    """Thu `(tên, nhóm thân)` của **mọi** `fn` **có thân**, ở **mọi** tầng token-tree.

    **KT-242 (N1/N2)** — cần biết **hàm trong cùng** chứa một token. `find_functions` (theo
    dòng + thụt lề) **nhảy qua** hàm lồng (`i = end + 1`), nên không dùng được cho việc này.
    """
    i = 0
    n = len(nodes)
    while i < n:
        node = nodes[i]
        if isinstance(node, Group):
            _fn_body_groups(node.children, out)
            i += 1
            continue
        if _is_ident(node, "fn") and i + 1 < n and _is_ident(nodes[i + 1]):
            name = nodes[i + 1].val
            j = i + 2
            while j < n and not (isinstance(nodes[j], Group) and nodes[j].open.val == "("):
                j += 1
            if j < n:
                k = j + 1
                while k < n:
                    x = nodes[k]
                    if isinstance(x, Group) and x.open.val == "{":
                        out.append((name, x))
                        break
                    if _is_punct(x, ";"):
                        break
                    k += 1
        i += 1


def _innermost_fn_tree(tree: Sequence[object], tok: Tok) -> Optional[str]:
    """Tên **hàm trong cùng** chứa `tok` (token-tree); `None` nếu ngoài mọi hàm.

    **KT-242 (N1/N2)** — đường lọt "cửa thứ bảy": `impl Machine { pub fn kt242_door(…) {
    self.write_state(tcb, TcbState::Ready) } }` **lồng trong thân** `wake_step`; mã sản phẩm
    gọi `m.kt242_door(t)` ⇒ TCB về `Ready` **không qua điều kiện nguồn nào**. `cargo check`
    RC=0; lớp 1 thường/`--strict`/lớp 2 **đều XANH** (đo: `JOURNAL/kt242-wake-repro/logs/r3a/`).

    Gốc: `_check_state_write` quy `write_state` cho hàm qua `enclosing(fns, n.line)`, mà
    `find_functions` **bỏ qua** hàm lồng ⇒ `write_state` trong `fn`/`impl` lồng bị quy cho
    **cửa ngoài** ⇒ thuộc `STATE_WRITE_CALLERS`. Nay đọc **token-tree**: hàm **trong cùng**
    thắng, nên hàm lồng **không** phải cửa ⇒ ĐỎ. Đây là **siết**, không nới: 6 cửa thật vẫn
    XANH (ca selftest đối chứng dương).
    """
    best: Optional[Tuple[str, int]] = None
    groups: List[Tuple[str, Group]] = []
    _fn_body_groups(tree, groups)
    for name, body in groups:
        if body.close is None:
            continue
        if body.open.start <= tok.start <= body.close.end:
            span = body.close.end - body.open.start
            if best is None or span < best[1]:
                best = (name, span)
    return best[0] if best else None


def _impl_groups(nodes: Sequence[object], out: List[Tuple[List[object], Group]]) -> None:
    """Thu `(token đầu `impl`, nhóm thân `{…}`)` của **mọi** `impl` ở **mọi** tầng token-tree.

    **KT-252 (WB-20)** — cần biết một `fn` là method của **kiểu nào** để phân biệt cửa thật
    (`impl Machine { fn wake_step }`) với hàm **trùng tên** (`impl Kt252Door { fn wake_step }`,
    `mod kt252 { fn wake_step }`, free `fn wake_step`). Trước bản này luật chỉ so **tên hàm**.
    """
    i = 0
    n = len(nodes)
    while i < n:
        node = nodes[i]
        if isinstance(node, Group):
            _impl_groups(node.children, out)
            i += 1
            continue
        if _is_ident(node, "impl"):
            j = i + 1
            header: List[object] = []
            body: Optional[Group] = None
            while j < n:
                x = nodes[j]
                if isinstance(x, Group) and x.open.val == "{":
                    body = x
                    break
                header.append(x)
                j += 1
            if body is not None:
                out.append((header, body))
        i += 1


def _impl_type_from_header(header: Sequence[object]) -> Optional[str]:
    """Kiểu **được impl** từ token đầu `impl` (bỏ generic `<…>` ở mức 0).

    `impl Machine` ⇒ `Machine`; `impl<T> Machine<T>` ⇒ `Machine`;
    `impl Kt252Trait for Kt252Door` ⇒ `Kt252Door` (kiểu **đứng sau `for`**).
    """
    depth = 0
    idents: List[str] = []
    for t in header:
        if isinstance(t, Group):
            continue
        if _is_punct(t, "<"):
            depth += 1
            continue
        if _is_punct(t, ">"):
            depth = max(0, depth - 1)
            continue
        if depth > 0:
            continue
        if _is_ident(t, "for"):
            idents = []
            continue
        if isinstance(t, Tok) and t.kind == "ident" and t.val not in (
            "impl",
            "where",
            "dyn",
            "mut",
            "const",
            "unsafe",
            "extern",
            "default",
        ):
            idents.append(t.val)
    return idents[-1] if idents else None


def _enclosing_impl_type(tree: Sequence[object], tok: Tok) -> Optional[str]:
    """Kiểu của `impl` **trong cùng** chứa `tok` (khoảng nhỏ nhất thắng); `None` nếu ngoài impl."""
    best: Optional[Tuple[int, Optional[str]]] = None
    groups: List[Tuple[List[object], Group]] = []
    _impl_groups(tree, groups)
    for header, body in groups:
        if body.close is None:
            continue
        if body.open.start <= tok.start <= body.close.end:
            span = body.close.end - body.open.start
            if best is None or span < best[0]:
                best = (span, _impl_type_from_header(header))
    return best[1] if best else None


def _fn_def_token(tree: Sequence[object], name: str) -> Optional[Tok]:
    """Token của **định nghĩa** `fn <name>` đầu tiên (không phải lời gọi)."""
    for n, sibs, i in _walk_nodes(tree):
        if isinstance(n, Tok) and n.kind == "ident" and n.val == name:
            prev = sibs[i - 1] if i > 0 else None
            if prev is not None and _is_ident(prev, "fn"):
                return n
    return None


def _check_state_write(
    relp: str,
    text: str,
    tr: Sequence[Tuple[int, int]],
    extra_doors: Optional[set],
    r: Report,
) -> set:
    """**KT-241** — `write_state`/`into_state` chỉ được sống trong `state_write.rs`.

    Luật thay thế **trực tiếp** cho `E_STRICT_TRANSITION` cũ (allowlist `NON_WAKE_READY_SITES`):
      - ngoài `state_write.rs`: **mọi** xuất hiện `write_state`/`into_state` ⇒ ĐỎ
        (`E_STRICT_STATE_WRITE`) — primitive ghi thô là private, không tệp nào khác được chạm;
      - trong `state_write.rs`: `write_state` chỉ được **gọi** bởi `STATE_WRITE_CALLERS`
        (cửa thông thường `transition_tcb` + 5 cửa `Ready`); dùng làm **giá trị**
        (`let f = Self::write_state;`) ⇒ ĐỎ; `into_state` được phép (không thể sinh `Ready`).

    **KT-242 (N1/N2)** — "hàm gọi" là **hàm trong cùng theo token-tree**
    ([`_innermost_fn_tree`]), **không** phải hàm theo dòng/thụt lề (`enclosing`): hàm lồng
    trong thân một cửa **không** phải cửa ⇒ ĐỎ. Tham số `fns` (từ `find_functions`) **bỏ** —
    nó là nguồn của lỗi quy sai; tham số `tr` nay là **khoảng ký tự** (`_test_char_ranges`),
    không phải khoảng dòng (KT-242 N5).

    Trả tập `(tệp, hàm)` của `extra_doors` **đã dùng** (để kiểm mục allowlist mục —
    `E_STRICT_STALE`).
    """
    used: set = set()
    toks, errs = tokenize(text, relp)
    if errs:
        return used
    try:
        tree = build_tree(toks, relp)
    except LexError:
        return used
    in_sw = relp in STATE_WRITE_FILES
    allowed = set(STATE_WRITE_CALLERS)
    if extra_doors:
        allowed |= {fn for (f, fn) in extra_doors if f == relp}
    # **KT-252 (WB-20)** — cửa phải là method của **cùng kiểu** với `write_state` (thực tế:
    # `impl Machine`). Trước bản này luật chỉ so **tên hàm**, nên một `fn wake_step` **trùng tên**
    # ở `impl` khác / module con / free fn vẫn được coi là cửa ⇒ ghi `Ready` không qua điều kiện
    # nguồn nào (đo: `cargo check` RC=0, cả ba lượt kiểm XANH — `JOURNAL/kt252-attack/`).
    owner: Optional[str] = None
    wdef = _fn_def_token(tree, "write_state")
    if wdef is not None:
        owner = _enclosing_impl_type(tree, wdef)
    for n, sibs, i in _walk_nodes(tree):
        if not (isinstance(n, Tok) and n.kind == "ident" and n.val in ("write_state", "into_state")):
            continue
        if in_ranges(n.start, tr):
            continue  # mã test — cùng quy ước miễn của mọi luật khác
        prev = sibs[i - 1] if i > 0 else None
        nxt = sibs[i + 1] if i + 1 < len(sibs) else None
        is_def = prev is not None and _is_ident(prev, "fn")
        is_call = isinstance(nxt, Group) and nxt.open.val == "("
        if not in_sw:
            r.err(
                "E_STRICT_STATE_WRITE",
                f"  {relp}:{n.line}: `{n.val}` xuất hiện ngoài `state_write.rs` — primitive "
                f"ghi thô là **private** (KT-241, D2-194 §3)",
            )
            continue
        if n.val == "into_state":
            if not is_def and not is_call:
                r.err(
                    "E_STRICT_STATE_WRITE",
                    f"  {relp}:{n.line}: `into_state` dùng làm giá trị — chỉ được **gọi** "
                    f"trong `state_write.rs` (KT-241)",
                )
            continue
        if is_def:
            continue
        if not is_call:
            r.err(
                "E_STRICT_STATE_WRITE",
                f"  {relp}:{n.line}: `write_state` dùng làm giá trị — chỉ được **gọi trực "
                f"tiếp** trong cửa (KT-241)",
            )
            continue
        # **KT-242 (N1/N2)** — hàm **trong cùng** theo token-tree, không theo dòng/thụt lề.
        fn_name = _innermost_fn_tree(tree, n)
        if fn_name is None:
            r.err(
                "E_STRICT_STATE_WRITE",
                f"  {relp}:{n.line}: `write_state` ngoài hàm — không xác định được cửa "
                f"(fail-closed, KT-241)",
            )
            continue
        if fn_name in allowed:
            # **KT-252 (WB-20)** — tên khớp **chưa đủ**: hàm phải là method của **cùng kiểu**
            # với `write_state` (`impl Machine`). `impl Kt252Door { fn wake_step }`, trait impl,
            # `mod kt252 { fn wake_step }`, free `fn wake_step` đều **không** phải cửa.
            impl_ty = _enclosing_impl_type(tree, n)
            if owner is None or impl_ty != owner:
                r.err(
                    "E_STRICT_STATE_WRITE",
                    f"  {relp}:{n.line}: `write_state` trong `{fn_name}` nhưng **không phải "
                    f"method của `{owner or '?'}`** (kiểu: `{impl_ty or 'không có impl'}`) — "
                    f"cửa phải là method của **cùng kiểu** với `write_state`; hàm **trùng tên** "
                    f"ở impl khác / module con / free fn **không** phải cửa (KT-252, WB-20)",
                )
                continue
            used.add((relp, fn_name))
            continue
        r.err(
            "E_STRICT_STATE_WRITE",
            f"  {relp}:{n.line}: `write_state` trong `{fn_name}` — chỉ "
            f"{len(STATE_WRITE_CALLERS)} hàm được gọi trực tiếp (cửa thông thường + 5 cửa "
            f"`Ready`; `tcb_yield` phải qua `yield_to_ready`); hàm **lồng trong thân** một "
            f"cửa **không** phải cửa (KT-241, KT-242 N1/N2)",
        )
    return used


def _scope_token_targets(
    root: Path,
    scope: "wake_scope.ProtectedScope",
    files: Sequence[Tuple[Path, str, List[Fn]]],
    test_only: set,
) -> List[Tuple[Path, str, List[Fn], bool]]:
    """**KT-258 (WB-21)** — tệp thuộc `token_scope` nhưng **ngoài** `V2_SCAN`.

    Luật token `wake` trước KT-258 chỉ chạy trên `files` = `v2/kernel` + `v2/sim`. Phạm vi
    **định nghĩa** (`wake_scope.token_scope`) rộng hơn: crate trong **bao đóng sản phẩm** ở gốc
    mới (`v2/drivers/…`) là mã sản phẩm nhưng **không bao giờ** được quét ⇒ đường lọt WB-21
    (đo: `JOURNAL/kt258-attack/logs/p1c-feature-gated-closure/`). Hàm này trả đúng các tệp còn
    thiếu để **cả hai** chế độ dùng chung một tập đích (một nguồn, không chép luật).
    """
    known = {p for p, _t, _f in files}
    out: List[Tuple[Path, str, List[Fn], bool]] = []
    for base_rel in scope.roots():
        base = root / base_rel
        if not base.is_dir():
            continue
        for p in _iter_rs_ci(base, root):
            if p in known or _is_build_dir(root, p):
                continue
            relp = rel(root, p)
            if not scope.token_scope(relp):
                continue
            try:
                text = p.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            out.append((p, text, find_functions(p, text), is_test_file(p, root, test_only)))
    return out


def scan_v2(
    root: Path,
    strict: bool,
    extra_doors: Optional[set] = None,
    scope: Optional["wake_scope.ProtectedScope"] = None,
    combined: bool = False,
):
    """Quét cây v2.

    **KT-333e (D2-247)** — `combined=True`: **một** lượt quét cho **cả hai** chế độ. Phần
    chung (đọc tệp, token hoá, đồ thị module, `include!`/`#[path]`, luật 1–5, luật cấu trúc)
    chạy **một lần**; hai chế độ chỉ khác nhau ở **khối luật `wake`** (khối `strict` và khối
    thường) và kiểm `extra_doors`. Trả `(report_chế_độ_thường, report_chế_độ_strict)`; mỗi
    report **giống hệt** report của lượt quét riêng tương ứng (cùng luật, cùng tập lỗi) —
    đây là điều kiện của phép cắt thời gian: **không** bỏ/nới luật nào.
    """
    r = Report()
    # **KT-171**: chuẩn hoá gốc (giải symlink) để `rel()`/`relative_to()` khớp nhau giữa
    # đường dẫn từ `iter_rs` và đường dẫn **phân giải** từ `#[path]`/`include!` (macOS:
    # `/var` → `/private/var`; selftest dựng cây trong `tempfile.mkdtemp`).
    root = root.resolve()

    # **KT-256 (L2, D2-188)** — phạm vi bảo vệ: **một** định nghĩa sinh từ `cargo metadata`
    # (`wake_scope`), dùng cho **cả hai** checker. Dựng **một lần** cho cả lượt quét.
    #   - `scope.error` (không dựng được metadata trên cây **có** workspace manifest) ⇒ ĐỎ
    #     `E_SCOPE_METADATA`: không được quét-rồi-PASS với phạm vi rỗng (fail-closed).
    #   - `check_no_reduction` ⇒ `E_SCOPE_REDUCTION`: định nghĩa mới **không** được hẹp hơn ba
    #     mẫu cũ ở bất kỳ tệp nào (lưới an toàn di trú, KT-256).
    #   - `check_lib_core_dependency` ⇒ `E_SCOPE_LIB_CORE`: **DEBT-KT227-1** — `v2/lib` ngoài luật
    #     token chỉ an toàn khi không crate `v2/lib` nào phụ thuộc `vifast-core` (normal dep).
    if scope is None:
        scope = wake_scope.protected_scope(root)
    if scope.error:
        r.err("E_SCOPE_METADATA", f"  {scope.error}")
    for e in scope.check_no_reduction(root):
        r.err("E_SCOPE_REDUCTION", e)
    for e in scope.check_lib_core_dependency():
        r.err("E_SCOPE_LIB_CORE", e)
    # **KT-258 (WB-23)** — path-dependency ra **ngoài gốc repo** ⇒ từ chối rõ (D2-188 §2).
    for e in scope.check_outside_dependencies():
        r.err("E_SCOPE_OUTSIDE", e)

    # --- mỏ neo (fail-closed) -------------------------------------------------
    # **KT-241** — `wake_step` **rời** `core_sched.rs`/`kobj.rs` sang `state_write.rs`
    # (API-SPEC §3: "Tất cả nằm trong `state_write.rs`"). Giữ mỏ neo `fn wake_step(` ở tệp
    # cũ sẽ **ĐỎ oan** sau khi A/B hợp nhất; mỏ neo của cửa chuyển sang
    # `_check_type_anchors` (đọc `state_write.rs`). Ở đây chỉ còn `fn wake(` — cửa sản phẩm
    # phát `Wake` vẫn nằm trong `core_sched.rs` / một tệp miền của `kobj` (KT-336b: `sched.rs`).
    anchors = {
        KERNEL_MAILBOX: ("enum MailboxMsg", WAKE_KINDS),
        SIM_MAILBOX: ("enum MailboxMsg", WAKE_KINDS),
        SYNC_MAILBOX: ("enum Msg", WAKE_KINDS),
        KERNEL_SCHED: ("fn wake(", ()),
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
    # **KT-336b (D2-232/D2-238)** — sim: `fn wake(` nằm trong **một** tệp miền của bộ `kobj`
    # (hiện là `sched.rs`). Kiểm **hợp** cả bộ thay vì ghim `kobj.rs`; tệp không tồn tại thì bỏ
    # qua, nhưng nếu **không tệp nào** còn `fn wake(` ⇒ ĐỎ (fail-closed).
    sim_texts = []
    for relpath in SIM_KOBJ_FILES:
        p = root / relpath
        if p.is_file():
            sim_texts.append(p.read_text(encoding="utf-8", errors="replace"))
    if not any("fn wake(" in t for t in sim_texts):
        r.err(
            "E_ANCHOR_MISSING",
            f"{SIM_KOBJ_ROOT}: thiếu `fn wake(` trong cả bộ tệp `kobj` (KT-336b)",
        )
    if not (root / SYNC_TEST).is_file():
        r.err("E_ANCHOR_MISSING", f"{SYNC_TEST}: thiếu test của kiểu thiết kế")
    else:
        t = (root / SYNC_TEST).read_text(encoding="utf-8", errors="replace")
        for kind in WAKE_KINDS:
            if f"Msg::{kind}" not in t:
                r.err("E_ANCHOR_MISSING", f"{SYNC_TEST}: không dùng `Msg::{kind}`")

    # **KT-194/D2-166 §3C** — lỗi từ vựng/cấu trúc trong tệp thuộc phạm vi bảo vệ phải **ĐỎ**;
    # không được trả về tập token/cạnh thiếu rồi tiếp tục xanh. Chạy **trước** mọi luật để
    # chẩn đoán `E_LEX_*` không bị lẫn với vi phạm chính sách `E_STRICT_*`.
    # **KT-264** — phạm vi lấy từ `wake_scope.protected_scope` (nợ §7.3 của KT-258), không
    # còn hằng `V2_SCAN`: crate ở gốc mới trong bao đóng sản phẩm cũng phải được kiểm từ vựng
    # + cấu trúc. Ba bất biến `E_STRUCT_*` chạy **sau** `resolve_include_targets` (cần biết
    # đích `include!` để không báo oan tệp mồ côi).
    scan_lexical(root, r, scope)
    scan_structure(root, r, scope)

    # --- quét mọi tệp nguồn ---------------------------------------------------
    files: List[Tuple[Path, str, List[Fn]]] = []
    # **KT-242 (N5)** — vùng test là **khoảng KÝ TỰ** (`_test_char_ranges`, token-tree), không
    # phải khoảng **dòng** (`test_line_ranges`). `test_line_ranges` lan cả dòng: đo được
    # `#[cfg(test)]\nfn helper() {} fn evil(…) { … }` ⇒ vùng `(1,2)` phủ **cả** dòng 2 ⇒ luật
    # 1/2/3 (và R2/R3) miễn mã sản phẩm **cùng dòng** một mục test. Hợp đồng §2 nói "không lan
    # sang mục cùng dòng" — sai cho tới bản này.
    test_ranges: Dict[str, List[Tuple[int, int]]] = {}
    line_starts: Dict[str, List[int]] = {}
    for p in iter_rs(root, V2_SCAN):
        text = p.read_text(encoding="utf-8", errors="replace")
        files.append((p, text, find_functions(p, text)))
        test_ranges[str(p)] = _test_char_ranges(strip_rust_noise(text), rel(root, p))
        _ls = [0]
        for _idx, _ch in enumerate(text):
            if _ch == "\n":
                _ls.append(_idx + 1)
        line_starts[str(p)] = _ls

    # **KT-241** — mỏ neo **kiểu** (fail-closed): đọc **định nghĩa** `state_write.rs` thay vì
    # tin tên hàm. Chạy **trước** mọi luật chính sách để chẩn đoán "API bị đảo" không lẫn với
    # vi phạm cụ thể (`E_STRICT_*`). Thiếu định nghĩa ⇒ `E_ANCHOR_MISSING`; sai kiểu/mức công
    # khai ⇒ `E_ANCHOR_TYPE`.
    _check_type_anchors(root, files, r)
    # **KT-241 (E1/E2)** — cấm mọi phép chuyển `TcbState` ⇄ `NonReadyTarget` ngoài `into_state`
    # private. Phạm vi **rộng hơn** luật `wake`: `v2/kernel` + `v2/sim` + `v2/lib` (orphan rule
    # cho phép `impl From<TcbState> for NonReadyTarget` **bên trong** crate ⇒ `cargo check`
    # RC=0 ⇒ đường vòng tái tạo qua ranh giới kiểu).
    _check_nonready_conv(root, r)

    # **KT-172** — "test-only" cho **mọi** luật (1–5) là **đồ thị module**
    # (`test_only_modules`), không theo tên/đường dẫn. Trước KT-172 bốn call site dưới đây
    # dùng `is_test_file_heuristic`, nên module **sản phẩm** tên `ktest_evil.rs` (hoặc đặt
    # dưới `src/tests/`) được miễn cả **luật KT-155** (`E_STRICT_TRANSITION`) — đúng lỗ hổng
    # reviewer L11 đã phá cho luật `wake` (KT-168 §1.2 B8/B9). Tính **một lần** cho cả lượt
    # quét (hàm có cache theo gốc cây, nhưng truyền vào để không phụ thuộc cache toàn cục).
    test_only = test_only_modules(root)
    # **KT-258 (WB-21)** — luật token phải chạy trên **mọi** tệp thuộc `token_scope`, không chỉ
    # `V2_SCAN`: crate trong bao đóng sản phẩm ở gốc mới (`v2/drivers/…`) nằm trong phạm vi
    # **định nghĩa** nhưng ngoài vòng lặp cũ ⇒ trước bản này luật token **không bao giờ** chạy
    # trên nó (đường lọt WB-21). Tính **một lần**, dùng cho cả hai chế độ.
    scope_targets = _scope_token_targets(root, scope, files, test_only)

    # **KT-174/H5** (reviewer L16): tệp được `include!` từ **mã sản phẩm** là mã sản phẩm,
    # dù đồ thị `mod` nói nó test-only. Trước bản vá này, `include!("evil_b.rs")` từ
    # `core_sched.rs` (sản phẩm) + `#[cfg(test)] mod evil_b;` ⇒ `--strict` vẫn `errors=0`
    # trong khi ảnh sản phẩm biên dịch `wake`. **Sản phẩm thắng.**
    #
    # **KT-184/D2-161** — luật cứng `include!`/`#[path]` chạy ở **MỌI** chế độ, không chỉ
    # `--strict`. Trước KT-184 khối phân giải + phát lỗi nằm **trong** `if strict:` (dòng
    # ~1542) nên `v2 check` thường (và `gate`) **không** bắt được `include!(concat!(…))`
    # hay `#[path]` trỏ ra ngoài cây — đúng lỗ hổng D2-161 đóng. Nay:
    #   - mọi `include!`/`#[path]` trong `v2/kernel`+`v2/sim` phải **literal** và **trỏ
    #     trong cây**; đích ngoài cây / không tồn tại / không literal ⇒ ĐỎ ở mọi chế độ;
    #   - `extra` (tệp đích + trạng thái test-only) vẫn được khối `--strict` dùng lại.
    # **KT-258 (WB-22)** — worklist `include!`/`#[path]` phải seed **cả** tệp ngoài `V2_SCAN`
    # (crate trong bao đóng sản phẩm): trước bản này `include!("…")` trong crate ở gốc mới
    # **không** được phân giải ⇒ đích ngoài cây không bị kiểm (đo: p1e). Đích của crate ngoài
    # `v2/kernel`+`v2/sim` bị **từ chối rõ** (`E_STRICT_WAKE_INCLUDE`) — fail-closed, đúng
    # D2-188 §2 ("kiểm tra, hoặc từ chối rõ vì ngoài hợp đồng").
    _scope_seed = [(p, t, f) for p, t, f, _i in scope_targets]
    extra, inc_errors = resolve_include_targets(root, list(files) + _scope_seed, test_only)
    # **KT-194/D2-166 §3C** — giữ tập test-only **gốc** trước khi "sản phẩm thắng": tệp
    # test-only bị kéo vào sản phẩm là tệp **ẩn** với đồ thị module, phải kiểm `wake` ở
    # **cả** chế độ (xem khối `if not strict:` dưới).
    orig_test_only = set(test_only)
    _product_included = {relp for relp, (_, t) in extra.items() if not t}
    if _product_included:
        test_only = set(test_only) - _product_included
        # Trạng thái test-only đổi ⇒ phân giải lại: lần hai thấy nhiều tệp **sản phẩm**
        # hơn nên bắt được nhiều `include!`/`#[path]` không literal hơn (đơn điệu,
        # fail-closed). Đây đúng là lần phân giải mà khối `--strict` đã dùng trước KT-184.
        extra, inc_errors = resolve_include_targets(
            root, list(files) + _scope_seed, test_only
        )
    for e in inc_errors:
        r.err("E_STRICT_WAKE_INCLUDE", e)

    # **KT-264** — luật **cấu trúc** theo **phạm vi** (nợ §7.3): cạnh `mod` không phân giải
    # được / trỏ ra ngoài crate / tệp `.rs` mồ côi trong crate bảo vệ. Chạy ở **mọi** chế độ
    # (như `include!`/`#[path]` từ KT-184), sau `extra` để biết đích `include!`.
    scan_structure_scope(root, r, scope, set(extra))

    # 1) Kênh chéo hart: chỉ `post_mail`/`try_push` mang message hộp thư.
    producers: Dict[str, List[str]] = {k: [] for k in WAKE_KINDS}
    for p, text, fns in files:
        if rel(root, p) in MAILBOX_DEFS or is_test_file(p, root, test_only):
            continue
        tr = test_ranges.get(str(p), [])
        ls = line_starts.get(str(p), [0])
        lines = text.splitlines()
        for lineno, line in enumerate(lines, 1):
            stripped = line.lstrip()
            if stripped.startswith("//") or stripped.startswith("*") or stripped.startswith("/*"):
                continue
            m = RE_MAILBOX_MSG.search(line)
            if not m:
                continue
            # **KT-242 (N5)** — miễn theo **vị trí của match**, không theo cả dòng.
            if in_ranges(ls[lineno - 1] + m.start(), tr):
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
        if rel(root, p) in MAILBOX_DEFS or is_test_file(p, root, test_only):
            continue
        tr = test_ranges.get(str(p), [])
        ls = line_starts.get(str(p), [0])
        for lineno, line in enumerate(text.splitlines(), 1):
            if "=>" not in line:
                continue
            fn = enclosing(fns, lineno)
            fname = fn.name if fn else "<ngoài hàm>"
            for _m in RE_MAILBOX_MSG.finditer(line):
                # **KT-242 (N5)** — miễn theo **vị trí của match**, không theo cả dòng.
                if in_ranges(ls[lineno - 1] + _m.start(), tr):
                    continue
                kind = _m.group(1)
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
        if is_test_file(p, root, test_only):
            continue
        tr = test_ranges.get(str(p), [])
        ls = line_starts.get(str(p), [0])
        # **KT-267 (b)** — miễn `doorbell` theo **ngữ nghĩa** (một lần/tệp), không theo tên tệp.
        virtio_driver = _is_virtio_driver(text)

        def _exempt(col: int) -> bool:
            """**KT-242 (N5)** — vị trí (dòng, cột) có nằm trong vùng test không."""
            return in_ranges(ls[lineno - 1] + col, tr)

        for lineno, line in enumerate(text.splitlines(), 1):
            _m_arch = re.search(r"\b(?:wake_step|MailboxMsg|send_reschedule|wake_hart)\b", line)
            if _m_arch and not _exempt(_m_arch.start()):
                r.err(
                    "E_ARCH_WAKE",
                    f"{relp}:{lineno}: tầng arch có kênh wake riêng",
                )
            _m_ipi = RE_IPI_WAKE_POST.search(line)
            if _m_ipi and RE_IPI_POST.search(line) and not _exempt(_m_ipi.start()):
                r.err(
                    "E_ARCH_DOORBELL",
                    f"{relp}:{lineno}: arch tự post `IpiKind::Wake` (doorbell riêng)",
                )
            _m_door = RE_DOORBELL.search(line)
            if _m_door and not virtio_driver and not _exempt(_m_door.start()):
                r.err(
                    "E_ARCH_DOORBELL",
                    f"{relp}:{lineno}: `doorbell` ngoài thiết bị virtio",
                )
            _m_ready = RE_READY_TRANSITION.search(line)
            if _m_ready and not _exempt(_m_ready.start()):
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

    # 5) **KT-241 (D2-194 §3)** — API thu hẹp: `transition_tcb` nhận `NonReadyTarget`, nên
    #    đối số thứ hai **không thể** là `Ready`. Luật cũ (allowlist `NON_WAKE_READY_SITES` +
    #    `_var_may_be_ready`) **bị thay thế trực tiếp** bằng hai luật dưới đây. Chạy ở **mọi**
    #    chế độ (KT-231 giữ nguyên yêu cầu: repro phải ĐỎ cả lớp một thường).
    #
    #    5a) `transition_tcb(…)` với đối số thứ hai là/chứa `TcbState` hoặc `…::Ready` ⇒ ĐỎ.
    #        Đây **không** còn là "phân tích Ready/không-Ready" (kiểu đã loại trừ `Ready`);
    #        nó là **chốt chống đảo kiểu**: nếu ai đó đổi `to: NonReadyTarget` về
    #        `to: TcbState`, mọi call-site cũ hiện nguyên hình. Không còn cửa/allowlist nào
    #        được miễn — dưới API mới **không** có chỗ hợp lệ nào truyền `TcbState`.
    for p, text, fns in files:
        relp = rel(root, p)
        if is_test_file(p, root, test_only):
            continue
        tr = test_ranges.get(str(p), [])
        for lineno, kind, desc, _start in _transition_calls(text, relp):
            # **KT-242 (N5)** — miễn theo **offset ký tự** của token, không theo dòng.
            if in_ranges(_start, tr):
                continue
            if kind == "ok":
                continue
            fn = enclosing(fns, lineno)
            where = f" trong `{fn.name}`" if fn is not None else ""
            if kind == "tcbstate":
                r.err(
                    "E_STRICT_TRANSITION",
                    f"  {relp}:{lineno}: `transition_tcb(…, {desc})`{where} — đối số thứ hai "
                    f"kiểu `TcbState`; API KT-241 đã thu hẹp sang `NonReadyTarget` ⇒ kiểu bị "
                    f"đảo ngược (fail-closed, D2-194 §3)",
                )
            else:  # ready_alias
                r.err(
                    "E_STRICT_TRANSITION",
                    f"  {relp}:{lineno}: `transition_tcb(…, {desc})`{where} — đối số thứ hai "
                    f"là `…::Ready` (alias/đường dẫn lạ); `NonReadyTarget` **không có** biến "
                    f"thể `Ready` ⇒ không chứng minh được (fail-closed, KT-241)",
                )

    #    5b) `write_state`/`into_state` **chỉ** sống trong `state_write.rs`; trong đó
    #        `write_state` **chỉ** được gọi bởi `STATE_WRITE_CALLERS`. Đây là luật **thay thế
    #        trực tiếp** cho `E_STRICT_TRANSITION` cũ: trước KT-241, "chỗ chuyển `Ready` hợp lệ
    #        ngoài cửa wake" được miễn bằng allowlist; nay **không** chỗ nào ngoài `state_write.rs`
    #        được chạm primitive ghi thô, nên allowlist **không còn cần** và đã bị xoá.
    used_doors: set = set()
    for p, text, fns in files:
        relp = rel(root, p)
        if is_test_file(p, root, test_only):
            continue
        tr = test_ranges.get(str(p), [])
        used_doors |= _check_state_write(relp, text, tr, extra_doors, r)

    # **KT-333e** — chế độ gộp: chụp lại report **chung** (mọi luật đã chạy ở trên) làm gốc cho
    # report chế độ **thường**; từ đây chỉ còn luật phụ thuộc chế độ (wake + extra_doors) nên
    # hai report mới tách nhau. Chụp **sau** mọi luật chung ⇒ không lệch một khẳng định nào.
    normal_rep: Optional[Report] = None
    if combined:
        normal_rep = Report()
        normal_rep.errors = list(r.errors)
        normal_rep.warnings = list(r.warnings)
        normal_rep.info = list(r.info)

    # Mục `extra_doors` (chỉ selftest dùng) phải còn khớp chỗ nào đó — giữ tinh thần
    # `E_STRICT_STALE` của allowlist cũ (mục allowlist mục ⇒ ĐỎ). Chỉ xét tệp **có trong cây**.
    if (strict or combined) and extra_doors:
        present = {rel(root, p) for p, _, _ in files}
        for key in sorted(extra_doors):
            if key[0] not in present:
                continue
            if key not in used_doors:
                r.err(
                    "E_STRICT_STALE",
                    f"  extra_doors có mục không còn khớp: {key[0]} {key[1]}",
                )

    if strict or combined:
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
        #      B4 — macro ở crate khác: `scan_macro_bodies` quét thân `macro_rules!` trong
        #              `v2/lib`+`v2/kernel`+`v2/sim` tìm token `wake` (allowlist
        #              `MACRO_WAKE_ALLOW` có lý do). **KT-181**: trước bản này chỉ `v2/lib`
        #              được quét token; `v2/kernel`+`v2/sim` chỉ có luật include.
        #
        #    Bỏ qua: **chú thích** + **literal chuỗi/ký tự** (khử nhiễu), **định nghĩa**
        #    `fn wake`, và **biến cục bộ** tên `wake` (`let mut wake = …` — có thật trong cây).
        #    Vì sao: `wake()` = post-only, `wake_from()` = post + `Ready` ngay — gọi `wake()`
        #    trong đường sản phẩm (đã tự gỡ bookkeeping) làm TCB nằm chờ trong hộp thư trong
        #    khi C2/`check_ipc` đã đỏ. Xem `v2/agent/JOURNAL/kt159-wake2.md` +
        #    `v2/agent/JOURNAL/kt165-wakefix.md` + `v2/agent/JOURNAL/kt171-wakefix3.md`.
        #    **KT-172**: `test_only` đã tính **một lần** ở đầu `scan_v2` (dùng chung cho luật
        #    1–5 và luật `wake`) — không tính lại, không để hai định nghĩa "test-only" lệch nhau.
        #    **KT-184/D2-161**: `extra` + lỗi `include!`/`#[path]` đã được phân giải **ngoài**
        #    `if strict:` (chạy ở mọi chế độ) — khối này chỉ **dùng lại** `extra`, không phân
        #    giải lần nữa (một nguồn sự thật cho cả hai chế độ).
        wake_targets: List[Tuple[Path, str, List[Fn], bool]] = []
        added: set = set()
        for p, text, fns in files:
            relp = rel(root, p)
            # **KT-242 (N3)** — truyền `root`: phạm vi = mẫu ∪ **crate root thật** (bin/lib
            # khai báo ngoài `src/`).
            if not in_test_only_wake_scope(relp, root, scope):
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
        # **KT-258 (WB-21)** — tệp trong `token_scope` nhưng ngoài `V2_SCAN` (crate ở gốc mới
        # trong bao đóng sản phẩm) cũng là đích của luật token.
        for p, text, fns, is_test in scope_targets:
            if p in added:
                continue
            wake_targets.append((p, text, fns, is_test))
            added.add(p)

        # **KT-177/G6** (H6): tên macro "an toàn" bị `macro_rules!` trong cây **che** thì
        # không còn an toàn — `concat!` bị che có thể biến ident `wake` thành **tên hàm**.
        safe_macros = SAFE_VALUE_MACROS - _shadowed_safe_macros(
            root,
            files,
            extra_files=[p for p, _ in extra.values()] + [p for p, _t, _f, _i in scope_targets],
        )

        for p, text, fns, is_test in wake_targets:
            relp = rel(root, p)
            if is_test:
                continue
            # **KT-241** — không truyền vùng test: `_check_wake_text` **tự** tính
            # `_test_char_ranges` (một nơi sở hữu phân vùng; tham số `tr` cũ bị bỏ qua).
            _check_wake_text(relp, text, safe_macros, r)

    # **KT-194/D2-166 §3C** — tệp **test-only** bị kéo vào **sản phẩm** bằng `#[path]`/
    # `include!` là tệp **ẩn** với đồ thị module. Ở `--strict` luật `wake` đã bắt; ở chế độ
    # thường trước KT-194 **xanh** (đúng ba repro D2-166 §2: `#[r#path]`, escape, `cfg_attr`
    # lồng). Nay chạy **cùng** luật token `wake` trên đúng các tệp ẩn đó ⇒ fail-closed ở mọi
    # chế độ, không nới ngưỡng, không đổi ngữ nghĩa `--strict`.
    if not strict or combined:
        # **KT-333e** — chế độ gộp: report đích của khối **thường** là `normal_rep` (khối
        # `strict` ở trên đã ghi vào `r`). Hai khối vẫn chạy **nguyên văn** như hai lượt riêng.
        target_rep = normal_rep if combined else r
        safe_macros = SAFE_VALUE_MACROS - _shadowed_safe_macros(
            root,
            files,
            extra_files=[p for p, _ in extra.values()] + [p for p, _t, _f, _i in scope_targets],
        )
        # **KT-215** — **đóng F2** (hợp đồng §4): luật `wake` chạy ở **cả** chế độ thường cho
        # **mọi** tệp sản phẩm trong phạm vi, không chỉ tệp bị kéo vào bằng `#[path]`/`include!`
        # (`extra`). Đường lọt #12 (`src/target/evil.rs`) là module **bình thường** — không bao
        # giờ vào `extra` — nên trước bản này chế độ thường XANH trong khi `--strict` ĐỎ. Hai
        # chế độ nay cùng một tập đích cho luật `wake`; `--strict` chỉ còn thêm allowlist
        # `transition_tcb` (`E_STRICT_TRANSITION`/`E_STRICT_STALE`).
        scanned: set = set()
        for p, text, _fns in files:
            relp = rel(root, p)
            # **KT-242 (N3)** — truyền `root` (xem trên).
            if not in_test_only_wake_scope(relp, root, scope):
                continue
            if is_test_file(p, root, test_only):
                continue
            _check_wake_text(relp, text, safe_macros, target_rep)
            scanned.add(p)
        # **KT-195/F1** (reviewer L34): điều kiện cũ `relp not in orig_test_only` khiến khối này
        # **chỉ chạy khi `_mods_of` phân giải SAI** — quan hệ ngược. Khi `#[path]` nằm trong
        # `lib.rs` (phân giải đúng), tệp đích đã thành **sản phẩm** trong `test_only_modules`
        # ⇒ không còn trong `orig_test_only` ⇒ bị bỏ qua ⇒ XANH. Nay quét **mọi** tệp sản phẩm
        # trong `extra` (tệp bị kéo vào bằng `#[path]`/`include!`), không phụ thuộc vị trí.
        for relp, (p, is_test) in sorted(extra.items()):
            if is_test or p in scanned:
                continue
            try:
                text = p.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            _check_wake_text(relp, text, safe_macros, target_rep)
        # **KT-258 (WB-21)** — cùng tập đích với `--strict`: tệp trong `token_scope` ngoài
        # `V2_SCAN` (crate ở gốc mới trong bao đóng sản phẩm).
        for p, text, _fns, is_test in scope_targets:
            if is_test or p in scanned:
                continue
            _check_wake_text(rel(root, p), text, safe_macros, target_rep)
            scanned.add(p)

    # 7) **KT-171 (B4)** + **KT-187 (B2/B3)** + **KT-181** — quét thân `macro_rules!` ở **mọi**
    #    tệp nguồn sản phẩm (`v2/lib`+`v2/kernel`+`v2/sim`) với **cả hai** luật (token `wake` +
    #    `include!`/`#[path]`/`mod`/`:tt`). Chạy ở **mọi** chế độ: D2-161 đòi `include!`/`#[path]`
    #    literal + trong cây ở **cả ba** chế độ, và `include!`/`#[path]` trong thân macro giải
    #    theo **tệp gọi** nên phải fail-closed.
    # **KT-225** — tên macro "an toàn" bị `macro_rules!` trong cây **che** thì không còn an
    # toàn (cùng lý do KT-177/G6 cho `wake`): `macro_rules! stringify { ($x:expr) => { $x } }`
    # + `stringify!(include!("…"))` sẽ mở rộng `include!` thật. Truyền tập bị che xuống
    # `scan_macro_bodies` để **không** miễn nhầm.
    _shadowed_safe = frozenset(
        _shadowed_safe_macros(root, files, extra_files=[p for p, _ in extra.values()])
    )
    # **KT-333e** — luật chung cuối cùng (thân macro) chạy **một lần** nhưng phải vào **cả hai**
    # report ở chế độ gộp (`_Tee` chỉ chuyển tiếp `err`/`warn`/`note`).
    macro_rep = _Tee(r, normal_rep) if combined else r
    scan_macro_bodies(
        root,
        macro_rep,
        extra_files=[p for p, _ in extra.values()],
        shadowed_safe=_shadowed_safe,
        scope=scope,
    )

    if combined:
        assert normal_rep is not None
        return normal_rep, r
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


