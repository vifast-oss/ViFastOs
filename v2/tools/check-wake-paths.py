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
  làm giá trị) trong **mã sản phẩm** (**mọi** crate dưới `v2/kernel/<crate>/` và
  `v2/sim/<crate>/`: toàn bộ `src/**` cộng `build.rs` — **mẫu**, không liệt kê cứng; KT-209/L2)
  **ngoài** `#[cfg(test)]`/`mod tests`
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

  **KT-172** — KT-171 chỉ vá **luật `wake`**; **luật 1–5** (kênh `MailboxMsg`, consumer, arch,
  `E_STRICT_TRANSITION`) vẫn miễn theo **tên/đường dẫn** (`is_test_file_heuristic`), nên module
  **sản phẩm** tên `ktest_evil.rs` lọt **luật KT-155** — đúng cách reviewer L11 đã phá. Nay:
    - **bốn call site** của luật 1–5 dùng [`is_test_file`] (đồ thị module); `test_only` tính
      **một lần** ở đầu `scan_v2`, dùng chung cho cả luật 1–5 và luật `wake`;
    - `is_test_file_heuristic` **xoá** ⇒ không còn hai định nghĩa "test-only" lệch nhau;
    - hệ quả đã biết: `proc_demo.rs` (chỉ `mod` từ `ktest_proc.rs`, bin `required-features =
      ["ktest","user"]`) thành **test-only** ⇒ hai mục `NON_WAKE_READY_SITES` của nó **bỏ**
      (giữ lại sẽ là allowlist mục `E_STRICT_STALE`, và nếu tệp thành sản phẩm thì mục cũ sẽ
      **âm thầm miễn** hai chỗ chuyển `Ready` — bỏ mục là hướng fail-closed).

  **KT-177** — reviewer L16 (KT-174 §2) phá bản KT-171/KT-174 bằng **bốn** lớp nữa; cả bốn
  đều là "mã sản phẩm biên dịch `wake`, `--strict` vẫn `errors=0`". Nay:
    - **H4** — `include!` **lồng hai lớp** qua tệp test-only: `resolve_include_targets` thành
      **fixpoint/worklist** (trạng thái `True → False` đơn điệu, xử lý lại khi đổi) ⇒ "sản
      phẩm thắng" lan **hết** chuỗi include, không dừng ở lớp đầu.
    - **H6** — `macro_rules!` trong cây **che** tên trong `SAFE_VALUE_MACROS` (`concat!`…) ⇒
      tên đó **không** còn an toàn (`_shadowed_safe_macros`), nên `wake` làm đối số macro bị
      bắt thay vì rơi xuống miễn trừ biến cục bộ.
    - **H7** — thân `macro_rules!` trong `v2/lib/**` chứa `include!`/`#[path]` ⇒ ĐỎ
      (`E_STRICT_MACRO_INCLUDE`, fail-closed): đường dẫn giải theo **tệp gọi**, không phân
      giải tĩnh an toàn được.
    - **H8** — literal chuỗi Rust **đầy đủ** (`RE_STR_LIT`: `r#"…"#`, `br#"…"#`, …) cho
      `include!`/`#[path]`; bản KT-174 chỉ thêm `r?` nên dạng **có `#`** vẫn lọt.

  **KT-184** (D2-160 + D2-161) — **luật cứng `include!`/`#[path]` ở MỌI chế độ**:
    - **D2-161** — khối phân giải `include!`/`#[path]` + phát lỗi `E_STRICT_WAKE_INCLUDE`
      trước đây nằm **trong** `if strict:` (dòng ~1542), nên `v2 check` **thường** (và
      `gate`) không bắt được `include!(concat!(…))` hay `#[path]` trỏ ra ngoài cây. Nay
      chạy **ngoài** `if strict:` — mọi `include!`/`#[path]` trong `v2/kernel`+`v2/sim`
      phải **literal** và **trỏ trong cây**, kiểm ở **mọi** chế độ. Mã lỗi giữ tên lịch sử
      `E_STRICT_WAKE_INCLUDE` (KT-171) để không phá các ca/allowlist đã có.
    - **D2-161** — thêm kiểm `#[path]` **không literal** (`#[path = concat!(…)]`): trước
      KT-184 chỉ `include!` không literal bị bắt, `#[path]` không phân giải được thì **lọt**
      hoàn toàn (tệp đích không được quét).
    - **D2-160** — H4 đã đóng bằng fixpoint (KT-177/G4); KT-184 chỉ **thêm ca** ba lớp và
      một dạng hai lớp `#[path]`+`include!` để khoá hành vi, không đổi thuật toán.

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
#
# **KT-209/L2** — reviewer L40 (KT-201 §D2 L2) dựng `v2/kernel/vifast-newcrate/src/lib.rs` và
# `v2/sim/vifast-sim2/src/lib.rs` chứa `m.wake(t)` ⇒ checker **XANH**; và
# `v2/kernel/vifast-core/build.rs` chứa `m.wake(t)` ⇒ **XANH** (chỉ `src/` được quét). Hợp đồng
# §1 lại nói "`v2/kernel/**`" — **sai** so với hành vi. Nay phạm vi là **mẫu**, không liệt kê
# cứng: **mọi** crate dưới `v2/kernel/<crate>/` và `v2/sim/<crate>/` — toàn bộ `src/**` **cộng**
# `build.rs` — kể cả crate ra đời sau. `tests/` cấp crate vẫn ngoài phạm vi (test-only, do
# `is_test_file` miễn; chúng không khớp mẫu này).
RE_TEST_ONLY_WAKE_SCOPE = re.compile(r"^v2/(?:kernel|sim)/[^/]+/(?:src/.*\.rs|build\.rs)$")


def in_test_only_wake_scope(relp: str) -> bool:
    """Tệp có thuộc phạm vi luật `wake()` chỉ-dùng-cho-test không (đường dẫn tương đối gốc)."""
    return bool(RE_TEST_ONLY_WAKE_SCOPE.match(relp))


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
    #
    # **KT-172** — hai mục `proc_demo.rs` (`build_rig` #1, `task_ready` #1) đã **bỏ**. Vì sao:
    # `proc_demo.rs` **chỉ** được `mod proc_demo;` từ `ktest_proc.rs` — bin có
    # `required-features = ["ktest", "user"]` ⇒ **test-only thật** theo đồ thị module. Từ KT-172
    # luật 5 dùng `is_test_file` (đồ thị) nên tệp này **không còn được quét**; giữ mục lại thì
    # `E_STRICT_STALE` đỏ (allowlist mục) — và tệ hơn, nếu mai kia `proc_demo.rs` được `mod` từ
    # `main.rs` (thành **sản phẩm**) thì mục cũ sẽ **âm thầm miễn** hai chỗ chuyển `Ready`.
    # Bỏ mục ⇒ hướng fail-closed: thành sản phẩm là `E_STRICT_TRANSITION` đỏ ngay.
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


def cfg_is_test_tokens(nodes: Sequence[object]) -> bool:
    """`#[cfg(…)]` có phải ngữ cảnh **test** không (chỉ nhận dạng chắc chắn)."""
    # `#[cfg(test)]`: vị từ nằm trong **một** nhóm `( … )`.
    if len(nodes) == 1 and isinstance(nodes[0], Group) and nodes[0].open.val == "(":
        nodes = nodes[0].children
    nodes = [n for n in nodes if not _is_punct(n, ",")]
    if len(nodes) == 1 and _is_ident(nodes[0], "test"):
        return True
    if (
        len(nodes) == 3
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
        return bool(parts) and all(cfg_is_test_tokens(p) for p in parts)
    return False


def _attrs_test_context(attrs: Sequence[Attr], line: int, tr: Sequence[Tuple[int, int]]) -> bool:
    """`mod` có nằm trong ngữ cảnh test không: trong khối test, hoặc có `#[cfg(test)]` trực tiếp."""
    if in_ranges(line, tr):
        return True
    for a in attrs:
        if a.name == "cfg" and cfg_is_test_tokens(a.args):
            return True
    return False


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
) -> None:
    """Duyệt **cấu trúc** mức mục: thuộc tính, `mod`, `macro_rules!`, `include!`.

    - Chỉ đệ quy vào nhóm `{ … }` (thân khối/inline module); nhóm đứng sau `!` là **lời gọi
      macro** ⇒ bỏ qua (token trong lời gọi không phải mục Rust cho tới khi mở rộng).
    - `macro_rules!` ⇒ gọi `on_macro` với **thân** rồi bỏ qua thân (luật macro xử lý riêng).
    - `mod NAME;` / `mod NAME { … }` ⇒ gọi `on_mod`; inline module đệ quy với thư mục con.
    """
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
                        on_mod(name_tok, pending, False, None, cur_dir, in_test)
                    i = j + 1
                    pending = []
                    continue
                if j < n and isinstance(nodes[j], Group) and nodes[j].open.val == "{":
                    # **KT-196/F2**: ngữ cảnh test theo **item/token**, không theo dòng.
                    _mod_test = in_test or any(
                        a.name == "cfg" and cfg_is_test_tokens(a.args) for a in pending
                    )
                    if on_mod is not None:
                        on_mod(name_tok, pending, True, nodes[j], cur_dir, in_test)
                    # **KT-199/F4b** (reviewer L35): inline module có `#[path = "sub"]` thì
                    # thư mục con là `sub`, KHÔNG phải `cur_dir/NAME` (rustc dùng `#[path]`).
                    _child_dir = cur_dir / name_tok.val
                    for _a in pending:
                        try:
                            _edges, _ = attr_path_edges(_a, relp)
                        except LexError:
                            continue
                        if _edges:
                            _child_dir = (cur_dir / _edges[0][0]).resolve()
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


def _protected_files(root: Path) -> Iterable[Path]:
    for base_rel in ("v2/kernel", "v2/sim", "v2/lib"):
        base = root / base_rel
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*.rs")):
            if "target" in p.parts:
                continue
            yield p


def scan_lexical(root: Path, r: Report) -> None:
    """**D2-166 §3C** — lỗi từ vựng trong tệp thuộc phạm vi bảo vệ ⇒ ĐỎ (không bỏ qua)."""
    for p in _protected_files(root):
        _, errs = parse_file(p)
        for e in errs:
            r.err(e.code, f"  {rel(root, Path(e.relp))}:{e.line}:{e.col}: {e.msg}")


def scan_structure(root: Path, r: Report) -> None:
    """**D2-166 §3C** — cấu trúc liên quan tới cạnh nguồn không phân giải được ⇒ ĐỎ."""
    for base_rel in ("v2/kernel", "v2/sim"):
        base = root / base_rel
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*.rs")):
            if "target" in p.parts:
                continue
            tree, errs = parse_file(p)
            if tree is None or errs:
                continue
            relp = rel(root, p)
            errs2: List[LexError] = []
            _walk_items(tree, relp, p.parent, errors=errs2)
            for e in errs2:
                r.err(e.code, f"  {relp}:{e.line}:{e.col}: {e.msg}")


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
    tr = test_line_ranges(text)
    relp = str(p)

    def on_mod(
        name_tok: Tok,
        attrs: Sequence[Attr],
        inline: bool,
        group: Optional[Group],
        cur_dir: Path,
        in_test: bool = False,
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
        target: Optional[Path] = None
        child_dir: Optional[Path] = None
        for a in attrs:
            try:
                edges, _ = attr_path_edges(a, relp)
            except LexError:
                continue
            if edges:
                val, _certain, _tok = edges[0]
                target = (cur_dir / val).resolve()
                child_dir = target.parent
                break
        if target is None:
            for cand in (cur_dir / (name_tok.val + ".rs"), cur_dir / name_tok.val / "mod.rs"):
                if cand.is_file():
                    target = cand.resolve()
                    child_dir = cur_dir / name_tok.val
                    break
        if target is not None and child_dir is not None:
            out.append((target, is_test, child_dir))

    _walk_items(tree, relp, mod_dir, on_mod=on_mod, errors=[])
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
    queue: List[Tuple[Path, bool]] = []
    for p, _, _ in files:
        queue.append((p, rel(root, p) in test_only))
    # **KT-209/L3** — `include!`/`#[path]` trong `v2/lib` ngoài macro cũng phải phân giải.
    # Tệp lib là **sản phẩm** (không có ngữ cảnh test-only trong `test_only_modules`, vốn chỉ
    # dựng đồ thị module cho kernel/sim) ⇒ `is_test=False`.
    for p in iter_rs(root, ("v2/lib",)):
        queue.append((p, False))
    while queue:
        p, is_test = queue.pop()
        if p in status:
            # Đã xử lý với trạng thái **mạnh hơn hoặc bằng** ⇒ không cần lại. Chỉ xử lý lại
            # đúng một lần khi `True → False` (sản phẩm thắng) để lan xuống con.
            if not status[p] or is_test:
                continue
            status[p] = False
        else:
            status[p] = is_test
        tree, errs = parse_file(p)
        if tree is None or errs:
            # Lỗi từ vựng đã được `scan_lexical` báo; không phân giải tiếp trên cây thiếu.
            continue
        relp = rel(root, p)
        found: List[Tuple[str, str, Tok, Path]] = []
        problems: List[Tuple[object, str]] = []

        def on_attr(attr: Attr, cur_dir: Path) -> None:
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

        def on_include(group: Group, cur_dir: Path) -> None:
            ch = group.children
            if len(ch) == 1 and isinstance(ch[0], Tok) and ch[0].kind == "str":
                # `include!` giải đường dẫn theo **thư mục tệp gọi**, không theo inline module.
                found.append(("include!", ch[0].val, ch[0], p.parent))
            else:
                problems.append(
                    (group.open, "`include!` không phải literal chuỗi — không phân giải được đích")
                )

        inline_path_toks: set = set()

        def on_mod(
            name_tok: Tok,
            attrs: Sequence[Attr],
            inline: bool,
            group: Optional[Group],
            cur_dir: Path,
            in_test: bool = False,
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

        _walk_items(
            tree,
            relp,
            p.parent,
            on_attr=on_attr,
            on_mod=on_mod,
            on_include=on_include,
            errors=[],
        )
        for tok, msg in problems:
            if isinstance(tok, LexError):
                continue
            line = tok.line if isinstance(tok, Tok) else 1
            errors.append(f"  {relp}:{line}: {msg} (fail-closed, KT-194)")
        src_tree = _source_tree(relp)
        for kind, raw, tok, base in found:
            target = (base / raw).resolve()
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
                # Đích trong `v2/lib`: phân giải **tiếp** (`include!` lồng không trốn được)
                # nhưng **không** vào `extra` — `extra` là tập tệp bị luật `wake` quét, mà mã
                # `v2/lib` không có `Machine` nên `wake()` không thể xuất hiện ở đó (cạnh sang
                # kernel/sim đã bị chặn ở trên).
                if target not in status or status[target]:
                    queue.append((target, False))
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
            if target not in status or (status[target] and not new):
                queue.append((target, new))
    return extra, errors


# ------------------------------------------------------------------ KT-171: B4 macro v2/lib

# **KT-194** — `_macro_bodies`/`RE_MACRO_RULES` cũ đã **xoá**: thân macro lấy từ token-tree
# (`_walk_items` gọi `on_macro` với nhóm thân). Tên macro là **định danh** (Unicode, raw ident).

# Allowlist **có lý do** cho thân `macro_rules!` trong `v2/lib/**` chứa token `wake`.
# Khoá: (relpath, tên macro). Hiện **rỗng** — không có macro nào trong `v2/lib` chạm `wake`.
MACRO_WAKE_ALLOW: Dict[Tuple[str, str], str] = {}


def _check_macro_body(relp: str, name: str, body: Group, r: Report) -> None:
    """**KT-189/B2d+T3c**, **KT-191**, **KT-194** — thân `macro_rules!` chứa cấu trúc có thể
    sinh `include!`/`#[path]`/`mod` ⇒ fail-closed.

    Nhận dạng **theo cấu trúc token**, không theo từ khoá:
      - `include` + `!` (không phải `include_str!`, không phải biến `include`);
      - thuộc tính `#[path]` (kể cả `#[r#path]`) và `cfg_attr(…, path = …)` **lồng**;
      - `mod NAME;` / `mod NAME { … }` (kể cả raw ident `mod r#x;` ⇒ `E_STRICT_MACRO_INCLUDE`);
      - `$…:tt` (passthrough) ⇒ `E_STRICT_MACRO_TT`.

    **Giảm báo oan có chủ đích** (D2-166 §3D): `std::path::Path`, `$path:expr`, `modulo(1)`,
    `include_str!` **không** còn bị coi là dấu hiệu — chúng không phải thuộc tính/macro/module.
    Hạn chế bảo thủ còn giữ: **mọi** thuộc tính trong thân macro có tên `path`/`cfg_attr` đều
    ĐỎ (không đánh giá `cfg`), và `$…:tt` ĐỎ vì có thể chở `mod x;` từ lời gọi.
    """

    def check_list(nodes: Sequence[object]) -> None:
        for i, n in enumerate(nodes):
            if isinstance(n, Group):
                check_list(n.children)
                continue
            assert isinstance(n, Tok)
            if n.kind == "ident" and n.val == "include":
                if i + 1 < len(nodes) and _is_punct(nodes[i + 1], "!"):
                    r.err(
                        "E_STRICT_MACRO_INCLUDE",
                        f"  {relp}:{n.line}: thân `macro_rules! {name}` chứa `include!` — "
                        f"đường dẫn giải theo **tệp gọi**, không phân giải tĩnh được "
                        f"(fail-closed, KT-194)",
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
                            f"  {relp}:{nodes[j].open.line}: thân `macro_rules! {name}` chứa "
                            f"thuộc tính không đọc được — không phân giải tĩnh được "
                            f"(fail-closed, KT-194)",
                        )
                        continue
                    if attr.name == "path":
                        r.err(
                            "E_STRICT_MACRO_INCLUDE",
                            f"  {relp}:{attr.tok.line}: thân `macro_rules! {name}` chứa "
                            f"`#[path]` — đường dẫn giải theo **tệp gọi**, không phân giải "
                            f"tĩnh được (fail-closed, KT-194)",
                        )
                    elif attr.name == "cfg_attr":
                        try:
                            edges, probs = attr_path_edges(attr, relp)
                        except LexError:
                            edges, probs = [], [(attr.tok, "không đọc được")]
                        if edges or probs:
                            r.err(
                                "E_STRICT_MACRO_INCLUDE",
                                f"  {relp}:{attr.tok.line}: thân `macro_rules! {name}` chứa "
                                f"`cfg_attr(…, path = …)` — đường dẫn giải theo **tệp gọi**, "
                                f"không phân giải tĩnh được (fail-closed, KT-194)",
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
                                f"  {relp}:{n.line}: thân `macro_rules! {name}` khai "
                                f"`mod r#…;` — không phân giải tĩnh được (fail-closed, KT-194)",
                            )
                        else:
                            r.err(
                                "E_STRICT_MACRO_MOD",
                                f"  {relp}:{n.line}: thân `macro_rules! {name}` khai `mod …;` — "
                                f"đồ thị module không thấy module này nên không xác định được "
                                f"test-only (fail-closed, KT-194)",
                            )
                    elif j < len(nodes) and isinstance(nodes[j], Group) and nodes[j].open.val == "{":
                        r.err(
                            "E_STRICT_MACRO_MOD",
                            f"  {relp}:{n.line}: thân `macro_rules! {name}` khai `mod … {{…}}` — "
                            f"đồ thị module không thấy module này nên không xác định được "
                            f"test-only (fail-closed, KT-194)",
                        )
                elif j < len(nodes) and _is_punct(nodes[j], "$"):
                    r.err(
                        "E_STRICT_MACRO_INCLUDE",
                        f"  {relp}:{n.line}: thân `macro_rules! {name}` khai `mod $…;` — "
                        f"không phân giải tĩnh được (fail-closed, KT-194)",
                    )
            if n.kind == "punct" and n.val == ":":
                if (
                    i + 1 < len(nodes)
                    and isinstance(nodes[i + 1], Tok)
                    and nodes[i + 1].kind == "ident"
                    and nodes[i + 1].val == "tt"
                ):
                    r.err(
                        "E_STRICT_MACRO_TT",
                        f"  {relp}:{n.line}: thân `macro_rules! {name}` dùng `$…:tt` "
                        f"(passthrough) — có thể chở `mod`/`include!` từ **lời gọi** "
                        f"(fail-closed, KT-194)",
                    )

    check_list(body.children)


def scan_lib_macros(root: Path, r: Report) -> None:
    """KT-171 (B4): quét thân `macro_rules!` trong `v2/lib/**` tìm token `wake`.

    Reviewer L11 đặt `#[macro_export] macro_rules! wake_call` trong `v2/lib/vifast-abi` (ngoài
    phạm vi quét cũ) với thân `$m.wake($t)`; lời gọi `vifast_abi::wake_call!(m, t)` **không**
    chứa token `wake` nên lọt. Nay thân macro ở **mọi** crate `v2/lib` bị quét; token `wake`
    (không phải `wake_from`/`wake_step`/`wake_addr`) ⇒ ĐỎ, trừ allowlist có lý do.

    **KT-177/G5** (reviewer L16 §2.7, H7): thân macro chứa `include!`/`#[path]` mà **không**
    có token `wake` thì bản KT-171 vẫn xanh — `resolve_include_targets` chỉ lặp trên
    `v2/kernel`+`v2/sim` nên `include!` trong `v2/lib` **không** được phân giải, và đích nằm
    trong `v2/lib` nên cũng không bị quét token. Đường dẫn `include!` trong thân macro giải
    theo **tệp gọi** (đã kiểm bằng `rustc` độc lập — reviewer L16 §2.7), không theo tệp định
    nghĩa ⇒ **không thể phân giải tĩnh an toàn** ⇒ fail-closed: mọi `include!`/`#[path]` trong
    thân `macro_rules!` của `v2/lib` là ĐỎ.
    """
    base = root / "v2/lib"
    if not base.is_dir():
        return
    for p in sorted(base.rglob("*.rs")):
        if "target" in p.parts:
            continue
        tree, errs = parse_file(p)
        if tree is None or errs:
            continue
        relp = rel(root, p)

        def on_macro(name_tok: Tok, body: Group, cur_dir: Path) -> None:
            name = name_tok.val
            for tok in _iter_tokens(body.children):
                if tok.kind == "ident" and tok.val == "wake":
                    if (relp, name) in MACRO_WAKE_ALLOW:
                        continue
                    r.err(
                        "E_STRICT_MACRO_WAKE",
                        f"  {relp}:{tok.line}: thân `macro_rules! {name}` chứa token `wake` — "
                        f"macro có thể sinh lời gọi `wake()` trong mã sản phẩm (KT-171 B4); "
                        f"dùng `wake_from`/`wake_step` hoặc thêm allowlist có lý do",
                    )
            _check_macro_body(relp, name, body, r)

        _walk_items(tree, relp, p.parent, on_macro=on_macro, errors=[])


def scan_kernel_macro_includes(root: Path, r: Report) -> None:
    """**KT-187/B2+B3** (reviewer L25): `include!`/`#[path]` trong thân `macro_rules!` của
    `v2/kernel`/`v2/sim` — **fail-closed**.

    Reviewer L25 dựng được hai đường lọt thật: `macro_rules!` định nghĩa ở thư mục A (decoy
    sạch) nhưng **gọi** ở thư mục B; rustc giải `include!`/`#[path]` trong thân macro theo
    **tệp gọi** (đã kiểm bằng `rustc`), còn `resolve_include_targets` giải theo **tệp định
    nghĩa** ⇒ đích thật không bao giờ được quét. Không phân giải tĩnh an toàn được ⇒ ĐỎ.
    """
    for base_rel in ("v2/kernel", "v2/sim"):
        base = root / base_rel
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*.rs")):
            if "target" in p.parts:
                continue
            tree, errs = parse_file(p)
            if tree is None or errs:
                continue
            relp = rel(root, p)

            def on_macro(name_tok: Tok, body: Group, cur_dir: Path) -> None:
                _check_macro_body(relp, name_tok.val, body, r)

            _walk_items(tree, relp, p.parent, on_macro=on_macro, errors=[])


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


def _shadowed_safe_macros(
    root: Path, files: Sequence[Tuple[Path, str, List[Fn]]]
) -> set:
    """**KT-177/G6** (reviewer L16 §2.6, H6): tên trong `SAFE_VALUE_MACROS` bị **che**.

    `SAFE_VALUE_MACROS` tin **tên** macro là "chỉ đọc giá trị", nhưng `macro_rules! concat`
    trong cây **che** macro chuẩn cùng tên và có thể biến ident thành **tên hàm**
    (`$m.$name($t)`). Khi đó `wake` là đối số macro "an toàn" ⇒ nhánh `SAFE_VALUE_MACROS` bỏ
    qua ⇒ rơi xuống `_exempt_local_wake` ⇒ biến cục bộ `wake` cùng khối miễn ⇒ **lọt**.

    Nay: quét **toàn cây** (`v2/kernel` + `v2/sim` + `v2/lib`) tìm `macro_rules! <tên>` trùng
    tên an toàn; tên nào bị che thì **không** còn an toàn (fail-closed). Chỉ **siết**: trên cây
    sạch hiện không có `macro_rules!` nào trùng tên an toàn (chỉ `sctlr_setters`, `bail`).
    """
    out: set = set()
    texts: List[Tuple[Path, str]] = [(p, t) for p, t, _ in files]
    base = root / "v2/lib"
    if base.is_dir():
        for p in sorted(base.rglob("*.rs")):
            if "target" in p.parts:
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

        def on_macro(name_tok: Tok, body: Group, cur_dir: Path) -> None:
            if name_tok.val in SAFE_VALUE_MACROS:
                out.add(name_tok.val)

        _walk_items(tree, str(p), p.parent, on_macro=on_macro, errors=[])
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


def _test_char_ranges(code: str, relp: str) -> List[Tuple[int, int]]:
    """**KT-198/F2b** (reviewer L35) — khoảng **ký tự** của các mục test, theo **token-tree**.

    Bản cũ `test_line_ranges` quyết định theo **dòng**: `#[cfg(test)] fn h() { … } pub fn evil(…)`
    cùng dòng ⇒ cả `evil` bị coi là test ⇒ luật `wake` bỏ qua ⇒ **lọt** (cargo RC=0). Nay mỗi
    `#[cfg(test)]` chỉ đánh dấu **đúng mục nó gắn vào** (từ mục tới `}` khớp).
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
                k = i + 1
                while k < n and not (
                    isinstance(nodes[k], Group) and nodes[k].open.val == "{"
                ):
                    if _is_punct(nodes[k], ";"):
                        break
                    k += 1
                if k < n and isinstance(nodes[k], Group) and nodes[k].open.val == "{":
                    is_test = in_test or any(
                        a.name == "cfg" and cfg_is_test_tokens(a.args) for a in pending
                    )
                    if is_test:
                        out.append((node.start, nodes[k].close.end))
                    walk(nodes[k].children, is_test, depth + 1)
                    i = k + 1
                    pending = []
                    continue
            pending = []
            i += 1

    walk(tree, False, 0)
    return out


def _check_wake_text(
    relp: str,
    text: str,
    tr: Sequence[Tuple[int, int]],
    safe_macros: set,
    r: Report,
) -> None:
    """**KT-155/KT-165/KT-171/KT-194** — luật token `wake` trên **một** tệp sản phẩm.

    Tách khỏi `scan_v2` để dùng chung cho **hai** đường:
      (i) mọi tệp sản phẩm ở `--strict` (KT-155);
      (ii) tệp **test-only bị kéo vào sản phẩm** bằng `#[path]`/`include!` ở chế độ thường
           (D2-166 §3C — không âm thầm bỏ qua cạnh nguồn ẩn; trước KT-194 đường này chỉ
           được luật `--strict` bắt nên chế độ thường xanh).
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

    # **KT-194/D2-166 §3C** — lỗi từ vựng/cấu trúc trong tệp thuộc phạm vi bảo vệ phải **ĐỎ**;
    # không được trả về tập token/cạnh thiếu rồi tiếp tục xanh. Chạy **trước** mọi luật để
    # chẩn đoán `E_LEX_*` không bị lẫn với vi phạm chính sách `E_STRICT_*`.
    scan_lexical(root, r)
    scan_structure(root, r)

    # --- quét mọi tệp nguồn ---------------------------------------------------
    files: List[Tuple[Path, str, List[Fn]]] = []
    test_ranges: Dict[str, List[Tuple[int, int]]] = {}
    for p in iter_rs(root, V2_SCAN):
        text = p.read_text(encoding="utf-8", errors="replace")
        files.append((p, text, find_functions(p, text)))
        test_ranges[str(p)] = test_line_ranges(text)

    # **KT-172** — "test-only" cho **mọi** luật (1–5) là **đồ thị module**
    # (`test_only_modules`), không theo tên/đường dẫn. Trước KT-172 bốn call site dưới đây
    # dùng `is_test_file_heuristic`, nên module **sản phẩm** tên `ktest_evil.rs` (hoặc đặt
    # dưới `src/tests/`) được miễn cả **luật KT-155** (`E_STRICT_TRANSITION`) — đúng lỗ hổng
    # reviewer L11 đã phá cho luật `wake` (KT-168 §1.2 B8/B9). Tính **một lần** cho cả lượt
    # quét (hàm có cache theo gốc cây, nhưng truyền vào để không phụ thuộc cache toàn cục).
    test_only = test_only_modules(root)

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
    extra, inc_errors = resolve_include_targets(root, files, test_only)
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
        extra, inc_errors = resolve_include_targets(root, files, test_only)
    for e in inc_errors:
        r.err("E_STRICT_WAKE_INCLUDE", e)

    # 1) Kênh chéo hart: chỉ `post_mail`/`try_push` mang message hộp thư.
    producers: Dict[str, List[str]] = {k: [] for k in WAKE_KINDS}
    for p, text, fns in files:
        if is_test_file(p, root, test_only) or p.name == "mailbox.rs":
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
        if p.name == "mailbox.rs" or is_test_file(p, root, test_only):
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
        if is_test_file(p, root, test_only):
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
            if is_test_file(p, root, test_only):
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
        #    **KT-172**: `test_only` đã tính **một lần** ở đầu `scan_v2` (dùng chung cho luật
        #    1–5 và luật `wake`) — không tính lại, không để hai định nghĩa "test-only" lệch nhau.
        #    **KT-184/D2-161**: `extra` + lỗi `include!`/`#[path]` đã được phân giải **ngoài**
        #    `if strict:` (chạy ở mọi chế độ) — khối này chỉ **dùng lại** `extra`, không phân
        #    giải lần nữa (một nguồn sự thật cho cả hai chế độ).
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

        # **KT-177/G6** (H6): tên macro "an toàn" bị `macro_rules!` trong cây **che** thì
        # không còn an toàn — `concat!` bị che có thể biến ident `wake` thành **tên hàm**.
        safe_macros = SAFE_VALUE_MACROS - _shadowed_safe_macros(root, files)

        for p, text, fns, is_test in wake_targets:
            relp = rel(root, p)
            if is_test:
                continue
            tr = test_ranges.get(str(p))
            if tr is None:
                tr = test_line_ranges(text)
            _check_wake_text(relp, text, tr, safe_macros, r)

    # **KT-194/D2-166 §3C** — tệp **test-only** bị kéo vào **sản phẩm** bằng `#[path]`/
    # `include!` là tệp **ẩn** với đồ thị module. Ở `--strict` luật `wake` đã bắt; ở chế độ
    # thường trước KT-194 **xanh** (đúng ba repro D2-166 §2: `#[r#path]`, escape, `cfg_attr`
    # lồng). Nay chạy **cùng** luật token `wake` trên đúng các tệp ẩn đó ⇒ fail-closed ở mọi
    # chế độ, không nới ngưỡng, không đổi ngữ nghĩa `--strict`.
    if not strict:
        safe_macros = SAFE_VALUE_MACROS - _shadowed_safe_macros(root, files)
        # **KT-195/F1** (reviewer L34): điều kiện cũ `relp not in orig_test_only` khiến khối này
        # **chỉ chạy khi `_mods_of` phân giải SAI** — quan hệ ngược. Khi `#[path]` nằm trong
        # `lib.rs` (phân giải đúng), tệp đích đã thành **sản phẩm** trong `test_only_modules`
        # ⇒ không còn trong `orig_test_only` ⇒ bị bỏ qua ⇒ XANH. Nay quét **mọi** tệp sản phẩm
        # trong `extra` (tệp bị kéo vào bằng `#[path]`/`include!`), không phụ thuộc vị trí.
        for relp, (p, is_test) in sorted(extra.items()):
            if is_test:
                continue
            try:
                text = p.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            tr = test_ranges.get(str(p))
            if tr is None:
                tr = test_line_ranges(text)
            _check_wake_text(relp, text, tr, safe_macros, r)

    # 7) **KT-171 (B4)** + **KT-187 (B2/B3)** — quét thân `macro_rules!`. Chạy ở **mọi** chế
    #    độ: D2-161 đòi `include!`/`#[path]` literal + trong cây ở **cả ba** chế độ, và
    #    `include!`/`#[path]` trong thân macro giải theo **tệp gọi** nên phải fail-closed.
    scan_lib_macros(root, r)
    scan_kernel_macro_includes(root, r)

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
    **13 ca KT-171** (sáu lớp B4/B6/B7/B8/B9/B10 của reviewer L11 + bảy đối chứng dương) +
    **7 ca KT-172** (bốn call site luật 1–5 chuyển sang đồ thị module: hai đột biến
    `ktest_evil.rs`/`src/tests/evil.rs` + hai đối chứng dương + ba ca luật 1/2/3) +
    **10 ca KT-177** (bốn lớp còn nợ của reviewer L16: H4/H6/H7/H8 + bốn đối chứng dương
    + hai ca H9 cho dấu ngoặc của `include!`) +
    **8 ca KT-184** (D2-160: H4 **ba lớp** + một dạng **hai lớp** `#[path]`+`include!`, mỗi
    dạng kèm đối chứng dương; D2-161: ba đột biến `include!`/`#[path]` không literal / ra
    ngoài cây chạy ở **chế độ thường** + một đối chứng dương) +
    **12 ca KT-187/189/191** (`cfg_attr(path)`, `#[path]`/`include!` trong thân macro mọi
    delimiter, tên macro Unicode, `mod r#…;`, `$…:tt`, `#[/*c*/path]`) +
    **31 ca KT-194** (D2-166 §5 — ba tầng: **13 ca lexer** raw ident/chuỗi/raw/escape/comment
    lồng/char-lifetime/delimiter/đầu vào hỏng/độ sâu; **9 ca tập cạnh nguồn** xác nhận **đúng
    cạnh** cho `#[path]`/`#[r#path]`/escape/`cfg_attr` lồng/`include!` `[]`+`{}`/`mod` Unicode/
    macro Unicode + đối chứng nhiễu comment-chuỗi; **9 ca chính sách** ba repro mới + đối chứng
    hợp lệ + năm đầu vào hỏng `E_LEX_*`) +
    **13 ca KT-209** (L40 KT-201 §D2 L2/L3: **7 ca phạm vi `wake` theo mẫu** — crate kernel
    mới, crate sim mới, `build.rs` kernel/sim, tệp con sâu, hai đối chứng dương `wake_from`;
    **6 ca `include!`/`#[path]` trong `v2/lib`** — sang kernel, ra ngoài cây, lồng hai lớp,
    `#[path]` sang kernel, hai đối chứng dương trong `v2/lib`). Tổng **131** ca.
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

        # ============ KT-172 (L15) — luật 1–5 dùng **đồ thị module**, không theo tên ============
        # Bối cảnh: KT-171 vá **luật `wake`** (B8/B9) nhưng bốn call site của luật 1–5 vẫn dùng
        # `is_test_file_heuristic` (miễn theo tên `ktest*`/`/tests/`). Hệ quả: module **sản phẩm**
        # tên `ktest_evil.rs` lọt **luật KT-155** (`E_STRICT_TRANSITION`) — đúng cách reviewer L11
        # đã phá. KT-172 đổi cả bốn sang `is_test_file` (đồ thị module) và **xoá** heuristic.
        #
        # Mỗi ca đột biến dưới đây là **đúng** cách phá của L11: tệp mang tên `ktest*` (hoặc nằm
        # dưới `src/tests/`) nhưng được `mod` từ **crate root sản phẩm** ⇒ phải bị quét.

        # (4ah) **Đột biến A (KT-172)** — `ktest_evil.rs` + `mod ktest_evil;` trong `main.rs`
        # (ảnh sản phẩm), chuyển `TcbState::Ready` **trực tiếp** (không qua `wake_from`).
        # Trước KT-172: XANH (`errors=0`) vì tên `ktest*` được miễn.
        d = tmp / "v2-kt172-a-ktest-evil-transition"
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
            "impl M {\n    fn duong_san_pham(&mut self, tcb: u32) -> u8 {\n"
            "        self.transition_tcb(tcb, TcbState::Ready)\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "KT-172 A: `ktest_evil.rs` + `mod` từ `main.rs`, `transition_tcb(…, Ready)` "
                "trực tiếp ⇒ đỏ `E_STRICT_TRANSITION`",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ai) Đối chứng dương cho A: cùng tệp, nhưng `#[cfg(feature = "ktest")] mod` ⇒
        # **test-only thật** (ảnh test kernel, D2-08) ⇒ XANH. Không có ca này thì "đỏ" ở (4ah)
        # có thể chỉ là "đỏ mọi thứ".
        d = tmp / "v2-kt172-a2-ktest-cfg"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/main.rs",
            '#[cfg(feature = "ktest")]\nmod ktest_evil;\n\nfn main() {}\n',
        )
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs",
            "impl M {\n    fn test(&mut self, tcb: u32) -> u8 {\n"
            "        self.transition_tcb(tcb, TcbState::Ready)\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                'KT-172 A2: `#[cfg(feature = "ktest")] mod ktest_evil;` ⇒ xanh (test-only thật)',
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4aj) **Đột biến B (KT-172)** — `vifast-core/src/tests/evil.rs` + `mod tests;` trong
        # `lib.rs` **không** `#[cfg(test)]`, chuyển `Ready` trực tiếp. Trước KT-172: XANH vì
        # đường dẫn chứa `/tests/`.
        d = tmp / "v2-kt172-b-src-tests-transition"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "mod tests;\n")
        _write(d / "v2/kernel/vifast-core/src/tests.rs", "mod evil;\n")
        _write(
            d / "v2/kernel/vifast-core/src/tests/evil.rs",
            "impl M {\n    fn duong_san_pham(&mut self, tcb: u32) -> u8 {\n"
            "        self.transition_tcb(tcb, TcbState::Ready)\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "KT-172 B: `src/tests/evil.rs` + `mod tests;` không `#[cfg(test)]`, "
                "`transition_tcb(…, Ready)` trực tiếp ⇒ đỏ `E_STRICT_TRANSITION`",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ak) Đối chứng dương cho B: `#[cfg(test)] mod tests;` ⇒ XANH.
        d = tmp / "v2-kt172-b2-src-tests-cfg"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod tests;\n")
        _write(d / "v2/kernel/vifast-core/src/tests.rs", "mod evil;\n")
        _write(
            d / "v2/kernel/vifast-core/src/tests/evil.rs",
            "impl M {\n    fn test(&mut self, tcb: u32) -> u8 {\n"
            "        self.transition_tcb(tcb, TcbState::Ready)\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "KT-172 B2: `#[cfg(test)] mod tests;` ⇒ xanh (test-only thật)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4al) **Luật 1** (kênh `MailboxMsg`) cũng phải quét module sản phẩm tên `ktest*`:
        # phát `MailboxMsg::Wake` từ hàm **không** phải cửa wake ⇒ đỏ `E_CHANNEL_PRODUCER`.
        d = tmp / "v2-kt172-c-luat1-producer"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/main.rs",
            "mod ktest_evil;\n\nfn main() {}\n",
        )
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.post_mail(1, MailboxMsg::Wake(t));\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-172 C: luật 1 — `ktest_evil.rs` (sản phẩm) phát `MailboxMsg::Wake` "
                "ngoài cửa wake ⇒ đỏ `E_CHANNEL_PRODUCER`",
                (not rep.ok) and any("E_CHANNEL_PRODUCER" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4am) **Luật 2** (consumer): xử lý `MailboxMsg::Wake` ngoài `apply_mail` ⇒ đỏ
        # `E_CONSUMER`.
        d = tmp / "v2-kt172-d-luat2-consumer"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/main.rs",
            "mod ktest_evil;\n\nfn main() {}\n",
        )
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs",
            "impl M {\n    fn duong_san_pham(&mut self, m: MailboxMsg) {\n"
            "        match m {\n            MailboxMsg::Wake(t) => { let _ = t; }\n"
            "            _ => {}\n        }\n    }\n}\n",
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-172 D: luật 2 — `ktest_evil.rs` (sản phẩm) xử lý `MailboxMsg::Wake` "
                "ngoài `apply_mail` ⇒ đỏ `E_CONSUMER`",
                (not rep.ok) and any("E_CONSUMER" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4an) **Luật 3** (arch): tệp tên `ktest*` trong crate `vifast-plat-*` dùng
        # `wake_step` ⇒ đỏ `E_ARCH_WAKE` (trước KT-172: miễn theo tên).
        d = tmp / "v2-kt172-e-luat3-arch"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-plat-riscv64/src/lib.rs",
            "mod ktest_evil;\n",
        )
        _write(
            d / "v2/kernel/vifast-plat-riscv64/src/ktest_evil.rs",
            "fn duong_san_pham(m: &mut M, t: u32) -> u8 {\n    m.wake_step(t)\n}\n",
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-172 E: luật 3 — `ktest_evil.rs` (sản phẩm) trong `vifast-plat-*` dùng "
                "`wake_step` ⇒ đỏ `E_ARCH_WAKE`",
                (not rep.ok) and any("E_ARCH_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # ============ KT-177 (L19) — bốn lớp còn nợ của luật `wake` (reviewer L16, KT-174) ============
        # Bối cảnh: KT-174 (lead, `c5f1ab00`) vá H5/H1/H2/H3. Reviewer L16 §2.4/§2.6/§2.7/§2.8
        # còn **bốn** lớp: H4 (`include!` lồng hai lớp), H6 (macro che tên "an toàn"), H7
        # (`include!` trong thân macro `v2/lib`), H8 (raw string **có `#`** tới tệp test-only
        # trong cây). Mỗi ca: đột biến **đúng** cách L16 đã phá ⇒ ĐỎ; kèm đối chứng dương.

        # (4ak) **H4** — `include!` lồng **hai lớp** qua tệp test-only:
        # `core_ipc.rs` (sản phẩm) → `evil_a.rs` (test-only) → `evil_b.rs` (test-only, có
        # `self.wake(t)`). Trước KT-177: XANH — `resolved` chặn lan "sản phẩm thắng" xuống con.
        d = tmp / "v2-h4-nested-include"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            "#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n",
        )
        _write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", 'include!("evil_a.rs");\n')
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4 `include!` lồng hai lớp qua tệp test-only ⇒ đỏ (KT-177)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4al) Đối chứng dương cho H4: cùng lồng hai lớp nhưng đích chỉ `wake_from(` ⇒ XANH
        # (fixpoint không phải "thấy include! lồng là đỏ").
        d = tmp / "v2-h4b-nested-ok"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            "#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n",
        )
        _write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", 'include!("evil_a.rs");\n')
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4b `include!` lồng hai lớp, đích chỉ `wake_from(` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4am) **H6** — `macro_rules! concat` trong cây **che** macro chuẩn cùng tên; thân
        # biến ident thành **tên hàm** (`$m.$name($t)`), `wake` là đối số macro "an toàn" và
        # có biến cục bộ `wake` cùng khối. Trước KT-177: XANH.
        d = tmp / "v2-h6-shadow-safe-macro"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil_macro.rs",
            "macro_rules! concat {\n"
            "    ($m:expr, $name:ident, $t:expr) => {\n"
            "        $m.$name($t)\n"
            "    };\n"
            "}\n"
            "impl M {\n"
            "    fn f(&mut self, t: u32) -> u8 {\n"
            "        let mut wake = 0u32;\n"
            "        wake += 1;\n"
            "        concat!(self, wake, t);\n"
            "        0\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H6 `macro_rules! concat` che tên an toàn + biến `wake` ⇒ đỏ (KT-177)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4an) Đối chứng dương cho H6: **không** có macro che tên ⇒ `assert_eq!(wake, 1)` với
        # biến cục bộ `wake` vẫn XANH (miễn trừ macro chỉ-đọc-giá-trị còn hiệu lực).
        d = tmp / "v2-h6b-safe-macro-ok"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_tcb.rs",
            "impl M {\n    fn f(&mut self) -> u8 {\n"
            "        let mut wake = 0u32;\n"
            "        wake += 1;\n"
            "        assert_eq!(wake, 1);\n"
            "        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H6b `assert_eq!(wake, 1)` không bị che ⇒ xanh (không báo oan)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4ao) **H7** — thân `macro_rules!` trong `v2/lib` chứa `include!` (thân **không** có
        # token `wake`). Trước KT-177: XANH — `scan_lib_macros` chỉ tìm token `wake`, và
        # `resolve_include_targets` không quét `v2/lib`.
        d = tmp / "v2-h7-macro-include"
        _fixture_v2(d)
        _write(
            d / "v2/lib/vifast-abi/src/lib.rs",
            "#[macro_export]\nmacro_rules! evil_inc {\n"
            "    () => {\n        include!(\"evil_inc_body.rs\")\n    };\n}\n",
        )
        _write(d / "v2/lib/vifast-abi/src/evil_inc_body.rs", "0\n")
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H7 thân `macro_rules!` trong `v2/lib` chứa `include!` ⇒ đỏ (KT-177)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ap) Đối chứng dương cho H7: macro `v2/lib` dùng `concat!` (không `include!`) ⇒ XANH.
        d = tmp / "v2-h7b-macro-ok"
        _fixture_v2(d)
        _write(
            d / "v2/lib/vifast-abi/src/lib.rs",
            "#[macro_export]\nmacro_rules! ok_macro {\n"
            "    ($a:expr) => {\n        concat!($a, \"!\")\n    };\n}\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H7b macro `v2/lib` dùng `concat!`, không `include!` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4aq) **H8** — `#[path = r#"…"#]` (chuỗi thô **có `#`**) trỏ tới tệp test-only **trong
        # cây**. Bản KT-174 chỉ thêm `r?` nên dạng có `#` vẫn lọt: `_mods_of` bỏ cạnh sản phẩm
        # ⇒ tệp ở lại test-only. Trước KT-177: XANH.
        d = tmp / "v2-h8-raw-hash-path"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_b;\n")
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = r#"evil_b.rs"#]\nmod evil_b_product;\n',
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                'H8 `#[path = r#"…"#]` tới tệp test-only trong cây ⇒ đỏ (KT-177)',
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ar) Đối chứng dương cho H8: `#[path = r#"…"#]` tới tệp **sản phẩm** chỉ dùng
        # `wake_from(` ⇒ XANH (raw string có `#` không tự nó làm đỏ).
        d = tmp / "v2-h8b-raw-hash-ok"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil_ok.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = r#"evil_ok.rs"#]\nmod evil_ok_product;\n',
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                'H8b `#[path = r#"…"#]` tới tệp sản phẩm chỉ `wake_from(` ⇒ xanh',
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4as) **H9** (cùng gốc G1/G2, ngoài bốn lớp bắt buộc) — `include!` dùng dấu ngoặc
        # **vuông**/nhọn: `include!["evil_b.rs"]` / `include!{"evil_b.rs"}`. rustc **chấp
        # nhận** cả hai (`cargo check` RC=0); bản KT-171/174 chỉ khớp `(`, nên lọt hoàn toàn.
        for _delim, _open, _close in (("vuông", "[", "]"), ("nhọn", "{", "}")):
            d = tmp / f"v2-h9-include-delim-{_open}"
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_b;\n")
            _write(
                d / "v2/kernel/vifast-core/src/evil_b.rs",
                "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                "        self.wake(t);\n        0\n    }\n}\n",
            )
            _write(
                d / "v2/kernel/vifast-core/src/core_ipc.rs",
                f'include!{_open}"evil_b.rs"{_close};\n',
            )
            rep = scan_v2(d, strict=True, allowlist={})
            cases.append(
                (
                    f"H9 `include!{_open}…{_close}` (ngoặc {_delim}) tới tệp test-only ⇒ đỏ "
                    f"(KT-177)",
                    (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                    "; ".join(rep.errors),
                )
            )

        # ============ KT-184 (D2-160 + D2-161) — H4 ba lớp + luật cứng ở MỌI chế độ ============
        # **D2-160**: H4 đã đóng bằng fixpoint (KT-177/G4). Thêm ca **ba lớp** (mới) và một
        # dạng **hai lớp** khác (`#[path]` + `include!`) để khoá hành vi lan "sản phẩm thắng"
        # qua **nhiều** lớp, không chỉ hai lớp của ca 4ak.
        # **D2-161**: `include!`/`#[path]` phải **literal** và **trỏ trong cây**, kiểm ở
        # **chế độ thường** (`strict=False`). Trước KT-184 khối phân giải nằm trong
        # `if strict:` nên `v2 check` thường (và `gate`) **không** bắt được ba đột biến dưới.

        # (4at) H4 **ba lớp**: `core_ipc.rs` (sản phẩm) → `evil_a.rs` → `evil_b.rs` →
        # `evil_c.rs` (cả ba test-only qua `#[cfg(test)] mod`, `evil_c` gọi `wake`).
        d = tmp / "v2-h4-three-layer"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            "#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n"
            "#[cfg(test)]\nmod evil_c;\n",
        )
        _write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
        _write(d / "v2/kernel/vifast-core/src/evil_b.rs", 'include!("evil_c.rs");\n')
        _write(
            d / "v2/kernel/vifast-core/src/evil_c.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", 'include!("evil_a.rs");\n')
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4 ba lớp `include!` qua tệp test-only ⇒ đỏ (KT-184/D2-160)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4au) Đối chứng dương cho ba lớp: đích chỉ `wake_from(` ⇒ XANH (fixpoint không phải
        # "thấy `include!` lồng ba lớp là đỏ").
        d = tmp / "v2-h4b-three-layer-ok"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            "#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n"
            "#[cfg(test)]\nmod evil_c;\n",
        )
        _write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
        _write(d / "v2/kernel/vifast-core/src/evil_b.rs", 'include!("evil_c.rs");\n')
        _write(
            d / "v2/kernel/vifast-core/src/evil_c.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", 'include!("evil_a.rs");\n')
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4b ba lớp, đích chỉ `wake_from(` ⇒ xanh (KT-184)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4av) H4 **hai lớp** dạng **khác**: `#[path]` nạp tệp sản phẩm `evil_a.rs`, rồi
        # `evil_a.rs` `include!("evil_b.rs")` (test-only, có `wake`) ⇒ sản phẩm thắng phải lan
        # qua **cả hai** cơ chế (`#[path]` rồi `include!`).
        d = tmp / "v2-h4-path-include"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_b;\n")
        _write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = "evil_a.rs"]\nmod evil_a;\n',
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4 `#[path]` (sản phẩm) + `include!` (test-only) hai lớp ⇒ đỏ (KT-184)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4aw) Đối chứng dương cho (4av): đích chỉ `wake_from(` ⇒ XANH.
        d = tmp / "v2-h4b-path-include-ok"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_b;\n")
        _write(d / "v2/kernel/vifast-core/src/evil_a.rs", 'include!("evil_b.rs");\n')
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = "evil_a.rs"]\nmod evil_a;\n',
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "H4b `#[path]` + `include!` hai lớp, đích chỉ `wake_from(` ⇒ xanh (KT-184)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4ax) **D2-161** — `include!(concat!(…))` **không literal** trong mã sản phẩm, chạy
        # ở **chế độ thường**. Trước KT-184: XANH (khối nằm trong `if strict:`).
        d = tmp / "v2-d2161-include-nonliteral"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'include!(concat!("evil", "_b.rs"));\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "D2-161 `include!(concat!(…))` ở chế độ thường ⇒ đỏ (KT-184)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ay) **D2-161** — `#[path = r#"…"#]` trỏ **ra ngoài** `v2/kernel`+`v2/sim`, chạy ở
        # **chế độ thường**. Trước KT-184: XANH.
        d = tmp / "v2-d2161-path-outside"
        _fixture_v2(d)
        _write(
            d / "v2/lib/vifast-abi/src/evil_out.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = r#"../../../lib/vifast-abi/src/evil_out.rs"#]\nmod evil_out;\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "D2-161 `#[path]` ra ngoài cây ở chế độ thường ⇒ đỏ (KT-184)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4az) **D2-161** — `#[path = concat!(…)]` **không literal**, chạy ở **chế độ thường**.
        # Trước KT-184: XANH (chỉ `include!` không literal bị bắt).
        d = tmp / "v2-d2161-path-nonliteral"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil_x.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = concat!("evil", "_x.rs")]\nmod evil_x;\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "D2-161 `#[path = concat!(…)]` ở chế độ thường ⇒ đỏ (KT-184)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ba) **KT-187/B1** — `#[cfg_attr(not(test), path = "…")]` trỏ tệp test-only có
        # `wake` ⇒ ĐỎ. Trước KT-187: XANH (mọi regex đòi `#[path` **liền**).
        d = tmp / "v2-b1-cfgattr-path"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            "#[cfg(test)]\nmod evil_b;\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/evil_b.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[cfg_attr(not(test), path = "evil_b.rs")]\nmod evil_b_product;\n',
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "B1 `#[cfg_attr(not(test), path = …)]` tới tệp test-only ⇒ đỏ (KT-187)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bb) **KT-187/B2** — `#[path]` trong thân `macro_rules!` ⇒ ĐỎ ở **chế độ thường**
        # (rustc giải theo tệp gọi, checker không phân giải tĩnh an toàn được).
        d = tmp / "v2-b2-macro-path"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! evil_mod { () => { #[path = "evil_b.rs"] mod evil; } }\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "B2 `#[path]` trong thân macro ⇒ đỏ ở chế độ thường (KT-187)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bc) **KT-187/B3** — `include!` trong thân `macro_rules!` ⇒ ĐỎ ở **chế độ thường**.
        d = tmp / "v2-b3-macro-include"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! evil_inc { () => { include!("evil_b.rs"); } }\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "B3 `include!` trong thân macro ⇒ đỏ ở chế độ thường (KT-187)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4be) **KT-189/B2c** — `macro_rules!` dùng **ngoặc tròn** cho thân:
        # `macro_rules! evil ( () => ( #[path = "…"] mod x; ); );`. Bản trước dừng ở `;` ⇒ bỏ qua.
        d = tmp / "v2-b2c-macro-paren"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! evil ( () => ( #[path = "evil_b.rs"] mod evil; ); );\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "B2c `#[path]` trong macro ngoặc tròn ⇒ đỏ (KT-189)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bf) **KT-189/B3a** — `include!` trong macro ngoặc tròn ⇒ ĐỎ.
        d = tmp / "v2-b3a-macro-paren-inc"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! evil ( () => ( include!("evil_b.rs"); ); );\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "B3a `include!` trong macro ngoặc tròn ⇒ đỏ (KT-189)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bg) **KT-189/B2d** — `cfg_attr(…, path = …)` trong thân macro ⇒ ĐỎ.
        d = tmp / "v2-b2d-macro-cfgattr"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! evil ( () => ( #[cfg_attr(not(test), path = "evil_b.rs")] mod x; ); );\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "B2d `cfg_attr(path)` trong thân macro ⇒ đỏ (KT-189)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bh) **KT-189/T3c** — `mod NAME;` trong thân macro ⇒ ĐỎ (`E_STRICT_MACRO_MOD`).
        d = tmp / "v2-t3c-macro-mod"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! evil ( () => ( mod evil3; ); );\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "T3c `mod …;` trong thân macro ⇒ đỏ (KT-189)",
                (not rep.ok) and any("E_STRICT_MACRO_MOD" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bj) **KT-191** — tên macro **Unicode** (`macro_rules! é`) ⇒ ĐỎ (bản ASCII-only bỏ qua).
        d = tmp / "v2-kt191-unicode-macro"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! é { () => ( #[path = "evil_b.rs"] mod evil; ); }\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-191 tên macro Unicode ⇒ đỏ (KT-191)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bk) **KT-191** — `mod $n;` (metavariable) trong thân macro ⇒ ĐỎ.
        d = tmp / "v2-kt191-mod-metavar"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! m { ($n:ident) => ( mod $n; ); }\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-191 `mod $n;` trong macro ⇒ đỏ (KT-191)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bl) **KT-191** — `mod r#x;` (raw ident) trong thân macro ⇒ ĐỎ.
        d = tmp / "v2-kt191-mod-raw"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! m { () => ( mod r#x; ); }\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-191 `mod r#x;` trong macro ⇒ đỏ (KT-191)",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bm) **KT-191** — passthrough `$($t:tt)*` ⇒ ĐỎ (có thể chở `mod x;` từ lời gọi).
        d = tmp / "v2-kt191-passthrough"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! pass { ($($t:tt)*) => ( $($t)* ); }\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-191 passthrough `$…:tt` ⇒ đỏ (KT-191)",
                (not rep.ok) and any("E_STRICT_MACRO_TT" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bn) **KT-191** — `#[/*c*/path]` (chú thích chen giữa) ⇒ ĐỎ (bộ lọc khử chú thích trước).
        d = tmp / "v2-kt191-comment-path"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[/*c*/path = "evil_b.rs"]\nmod evil_c;\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-191 `#[/*c*/path]` ⇒ đỏ (KT-191)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4bo) Đối chứng dương cho D2-161: `#[path]` **literal**, trỏ **trong** cây, đích chỉ
        # `wake_from(` ⇒ XANH ở chế độ thường (luật không phải "có `#[path]` là đỏ").
        d = tmp / "v2-d2161-path-ok"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil_ok.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[path = "evil_ok.rs"]\nmod evil_ok_product;\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "D2-161b `#[path]` literal trong cây ở chế độ thường ⇒ xanh (KT-184)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # ================================================================ KT-194
        # **D2-166 §5** — ba tầng test: (a) lexer; (b) **tập cạnh nguồn** (xác nhận **đúng
        # cạnh**, không chỉ exit code); (c) chính sách + cây thật (ba repro mới + đối chứng
        # hợp lệ). Ba repro là ba đường lọt D2-166 §2: `#[r#path]`, escape chuỗi, `cfg_attr`
        # lồng — trước KT-194 checker cũ (regex trên văn bản thô) bỏ lọt cả ba.

        # --- (a) lexer -------------------------------------------------------
        def _lex(text: str):
            return tokenize(text, "<lex>")

        toks, errs = _lex("r#path")
        cases.append(
            (
                "KT-194a raw ident `r#path` ⇒ ident `path` (raw=True)",
                (not errs)
                and len(toks) == 1
                and toks[0].kind == "ident"
                and toks[0].val == "path"
                and toks[0].raw is True,
                f"{toks} {errs}",
            )
        )

        toks, errs = _lex('"a\\x2eb"')
        cases.append(
            (
                "KT-194a chuỗi escape `\\x2e` ⇒ giải mã `.`",
                (not errs) and len(toks) == 1 and toks[0].kind == "str" and toks[0].val == "a.b",
                f"{toks} {errs}",
            )
        )

        toks, errs = _lex('"\\u{2e}"')
        cases.append(
            (
                "KT-194a chuỗi escape `\\u{2e}` ⇒ giải mã `.`",
                (not errs) and len(toks) == 1 and toks[0].val == ".",
                f"{toks} {errs}",
            )
        )

        toks, errs = _lex('r#"a"b"#')
        cases.append(
            (
                'KT-194a chuỗi thô `r#"a"b"#` ⇒ giữ nguyên dấu `"`',
                (not errs) and len(toks) == 1 and toks[0].kind == "str" and toks[0].val == 'a"b',
                f"{toks} {errs}",
            )
        )

        toks, errs = _lex("/* /* */ */")
        cases.append(
            (
                "KT-194a chú thích khối lồng ⇒ không token, không lỗi",
                (not errs) and not toks,
                f"{toks} {errs}",
            )
        )

        toks, errs = _lex("'a' 'b")
        cases.append(
            (
                "KT-194a char `'a'` vs lifetime `'b`",
                (not errs)
                and len(toks) == 2
                and toks[0].kind == "char"
                and toks[1].kind == "lifetime"
                and toks[1].val == "b",
                f"{toks} {errs}",
            )
        )

        toks, errs = _lex("(]")
        try:
            build_tree(toks, "<lex>")
            delim_ok = False
            delim_detail = "không lỗi"
        except LexError as e:
            delim_ok = e.code == "E_LEX_UNBALANCED"
            delim_detail = str(e)
        cases.append(("KT-194a delimiter lệch `(]` ⇒ E_LEX_UNBALANCED", delim_ok, delim_detail))

        toks, errs = _lex('"abc')
        cases.append(
            (
                "KT-194a chuỗi chưa đóng ⇒ E_LEX_UNTERMINATED_STRING",
                bool(errs) and errs[0].code == "E_LEX_UNTERMINATED_STRING",
                f"{errs}",
            )
        )

        toks, errs = _lex("/* abc")
        cases.append(
            (
                "KT-194a chú thích chưa đóng ⇒ E_LEX_UNTERMINATED_COMMENT",
                bool(errs) and errs[0].code == "E_LEX_UNTERMINATED_COMMENT",
                f"{errs}",
            )
        )

        toks, errs = _lex('"a\\qb"')
        cases.append(
            (
                "KT-194a escape không hỗ trợ `\\q` ⇒ E_LEX_INVALID_ESCAPE",
                bool(errs) and errs[0].code == "E_LEX_INVALID_ESCAPE",
                f"{errs}",
            )
        )

        toks, errs = _lex("(" * (MAX_TREE_DEPTH + 2))
        try:
            build_tree(toks, "<lex>")
            depth_ok = False
            depth_detail = "không lỗi"
        except LexError as e:
            depth_ok = e.code == "E_LEX_DEPTH"
            depth_detail = str(e)
        cases.append(
            ("KT-194a lồng ngoặc quá MAX_TREE_DEPTH ⇒ E_LEX_DEPTH", depth_ok, depth_detail)
        )

        inner = 'path = "p"'
        for _ in range(MAX_ATTR_DEPTH + 2):
            inner = "cfg_attr(x, " + inner + ")"
        toks, errs = _lex("#[" + inner + "]")
        tree = build_tree(toks, "<lex>") if not errs else []
        attr = parse_attr(tree[1], "<lex>") if len(tree) > 1 else None
        try:
            attr_path_edges(attr, "<lex>")
            adepth_ok = False
            adepth_detail = "không lỗi"
        except LexError as e:
            adepth_ok = e.code == "E_LEX_DEPTH"
            adepth_detail = str(e)
        cases.append(
            (
                "KT-194a `cfg_attr` lồng quá MAX_ATTR_DEPTH ⇒ E_LEX_DEPTH",
                adepth_ok,
                adepth_detail,
            )
        )

        toks, errs = _lex("#[1 = 2]")
        tree = build_tree(toks, "<lex>") if not errs else []
        try:
            parse_attr(tree[1], "<lex>")
            attr_ok = False
            attr_detail = "không lỗi"
        except LexError as e:
            attr_ok = e.code == "E_LEX_ATTR"
            attr_detail = str(e)
        cases.append(
            ("KT-194a tên thuộc tính không phải định danh ⇒ E_LEX_ATTR", attr_ok, attr_detail)
        )

        # --- (b) tập cạnh nguồn: xác nhận **đúng cạnh**, không chỉ exit code ---------
        def _edges_of(d: Path):
            # `scan_v2` chuẩn hoá gốc bằng `.resolve()`; ở đây cũng vậy để `relative_to()`
            # khớp trên macOS (`/var` → `/private/var`).
            d = d.resolve()
            files = [
                (p, p.read_text(encoding="utf-8", errors="replace"), [])
                for p in iter_rs(d, V2_SCAN)
            ]
            return resolve_include_targets(d, files, test_only_modules(d))

        def _edge_case(name: str, attr_text: str, want: str) -> None:
            d = tmp / ("kt194b-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_t;\n")
            _write(d / "v2/kernel/vifast-core/src/evil_t.rs", "pub fn f() {}\n")
            _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", attr_text + "\nmod evil_p;\n")
            extra, errs = _edges_of(d)
            got = extra.get(want)
            cases.append(
                (
                    f"KT-194b {name} ⇒ đúng cạnh `{want}` (sản phẩm)",
                    (not errs) and got is not None and got[1] is False,
                    f"extra={sorted(extra)} errs={errs}",
                )
            )

        _edge_case("path", '#[path = "evil_t.rs"]', "v2/kernel/vifast-core/src/evil_t.rs")
        _edge_case("r#path", '#[r#path = "evil_t.rs"]', "v2/kernel/vifast-core/src/evil_t.rs")
        _edge_case("escape", '#[path = "evil_t\\x2ers"]', "v2/kernel/vifast-core/src/evil_t.rs")
        _edge_case(
            "cfg_attr lồng",
            '#[cfg_attr(all(not(test)), cfg_attr(not(test), path = "evil_t.rs"))]',
            "v2/kernel/vifast-core/src/evil_t.rs",
        )

        for delim, text in (
            ("[]", 'include!["evil_t.rs"];'),
            ("{}", 'include!{"evil_t.rs"}'),
        ):
            d = tmp / ("kt194b-include-" + ("sq" if delim == "[]" else "br"))
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_t;\n")
            _write(d / "v2/kernel/vifast-core/src/evil_t.rs", "pub fn f() {}\n")
            _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", text + "\n")
            extra, errs = _edges_of(d)
            want = "v2/kernel/vifast-core/src/evil_t.rs"
            got = extra.get(want)
            cases.append(
                (
                    f"KT-194b `include!` delimiter `{delim}` ⇒ đúng cạnh",
                    (not errs) and got is not None and got[1] is False,
                    f"extra={sorted(extra)} errs={errs}",
                )
            )

        d = tmp / "kt194b-mod-unicode"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "mod xin_chào;\n")
        _write(d / "v2/kernel/vifast-core/src/xin_chào.rs", "pub fn f() {}\n")
        mods = _mods_of(
            d / "v2/kernel/vifast-core/src/lib.rs", d / "v2/kernel/vifast-core/src"
        )
        cases.append(
            (
                "KT-194b `mod` tên Unicode ⇒ đúng cạnh",
                any(str(t).endswith("xin_chào.rs") for t, _, _ in mods),
                f"{mods}",
            )
        )

        d = tmp / "kt194b-macro-unicode"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            'macro_rules! xin_chào [ () => { include!("x.rs"); } ];\n',
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-194b `macro_rules!` tên Unicode, thân `[]` ⇒ đỏ E_STRICT_MACRO_INCLUDE",
                (not rep.ok) and any("E_STRICT_MACRO_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        d = tmp / "kt194b-noise"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '// #[path = "evil_t.rs"]\nconst S: &str = "#[path = \\"evil_t.rs\\"]";\n',
        )
        extra, errs = _edges_of(d)
        cases.append(
            (
                "KT-194b `#[path]` trong comment/chuỗi ⇒ không cạnh",
                (not errs) and not extra,
                f"extra={sorted(extra)} errs={errs}",
            )
        )

        # --- (c) chính sách + cây thật: ba repro mới + đối chứng hợp lệ -----------
        def _repro(name: str, attr_text: str) -> None:
            d = tmp / ("kt194c-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", "#[cfg(test)]\nmod evil_t;\n")
            _write(
                d / "v2/kernel/vifast-core/src/evil_t.rs",
                "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                "        self.wake(t);\n        0\n    }\n}\n",
            )
            _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", attr_text + "\nmod evil_p;\n")
            rep = scan_v2(d, strict=True, allowlist={})
            cases.append(
                (
                    f"KT-194c repro {name} ⇒ đỏ E_STRICT_TEST_ONLY_WAKE",
                    (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                    "; ".join(rep.errors),
                )
            )

        _repro("r#path", '#[r#path = "evil_t.rs"]')
        _repro("escape", '#[path = "evil_t\\x2ers"]')
        _repro(
            "cfg_attr lồng",
            '#[cfg_attr(all(not(test)), cfg_attr(not(test), path = "evil_t.rs"))]',
        )

        # --- (d) **KT-196/F2** (A0 2026-11-19 §2): ngữ cảnh test theo **item/token-tree**,
        # không theo dòng. `#[cfg(test)]` chỉ áp cho **mục kế tiếp**, không lan sang mục cùng dòng.
        def _scope_case(name: str, src: str, expect_test: bool) -> None:
            d = tmp / ("kt196-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", src)
            _write(d / "v2/kernel/vifast-core/src/evil_t.rs", "pub fn f() {}\n")
            mods = _mods_of(
                d / "v2/kernel/vifast-core/src/lib.rs", d / "v2/kernel/vifast-core/src"
            )
            got = [is_test for t, is_test, _ in mods if str(t).endswith("evil_t.rs")]
            cases.append(
                (
                    f"KT-196/F2 {name} ⇒ {'test' if expect_test else 'sản phẩm'}",
                    bool(got) and got[0] == expect_test,
                    f"{mods}",
                )
            )

        _scope_case("cùng dòng sau fn test", "#[cfg(test)] fn h() {} mod evil_t;\n", False)
        _scope_case("khác dòng sau fn test", "#[cfg(test)]\nfn h() {}\nmod evil_t;\n", False)
        _scope_case("thuộc tính trực tiếp", "#[cfg(test)]\nmod evil_t;\n", True)
        # Ca lồng: tệp đích nằm trong thư mục module con (`src/tests/`).
        d = tmp / "kt196-long"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            "#[cfg(test)]\nmod tests {\n    mod evil_t;\n}\n",
        )
        _write(d / "v2/kernel/vifast-core/src/tests/evil_t.rs", "pub fn f() {}\n")
        mods = _mods_of(
            d / "v2/kernel/vifast-core/src/lib.rs", d / "v2/kernel/vifast-core/src"
        )
        got = [is_test for t, is_test, _ in mods if str(t).endswith("tests/evil_t.rs")]
        cases.append(
            (
                "KT-196/F2 trong mod test lồng ⇒ test",
                bool(got) and got[0] is True,
                f"{mods}",
            )
        )
        _scope_case(
            "chú thích + dòng trống xen giữa",
            "#[cfg(test)]\nfn h() {}\n\n// chú thích\nmod evil_t;\n",
            False,
        )
        _scope_case(
            "nhiều thuộc tính trước mod test",
            "#[cfg(test)]\n#[allow(dead_code)]\nmod evil_t;\n",
            True,
        )

        # --- (e) **KT-198/F2b** (reviewer L35): luật `wake` dùng ngữ cảnh test theo
        # **token-tree**, không theo dòng. `#[cfg(test)]` chỉ miễn **đúng mục nó gắn vào**.
        def _wake_case(name: str, src: str, expect_red: bool) -> None:
            d = tmp / ("kt198-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", src)
            rep = scan_v2(d, strict=True, allowlist={})
            got = any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors)
            cases.append(
                (
                    f"KT-198/F2b {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _wake_case(
            "hàm sản phẩm cùng dòng sau `}` test",
            "#[cfg(test)]\nfn h() { let _ = 1; } pub fn evil() { let _ = m.wake(t); }\n",
            True,
        )
        _wake_case(
            "wake trong mod test thật",
            "#[cfg(test)]\nmod tests {\n    fn f() { let _ = m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "wake trong fn test thật",
            "#[cfg(test)]\nfn f() { let _ = m.wake(t); }\n",
            False,
        )

        # --- (f) **KT-199/F4b/F4c** (reviewer L35): inline module có `#[path]` dùng đúng
        # thư mục `#[path]`; `#[path]` trỏ thư mục trên `mod` **không** thân ⇒ lỗi rõ.
        d = tmp / "kt199-f4b"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/lib.rs",
            '#[cfg(test)]\n#[path = "sub"]\nmod m {\n    mod child;\n}\n',
        )
        _write(
            d / "v2/kernel/vifast-core/src/sub/child.rs",
            "pub fn f() { let _ = m.wake(t); }\n",
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "KT-199/F4b inline module `#[path]` + con test-only ⇒ xanh (không báo oan)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        d = tmp / "kt199-f4c"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", '#[path = "sub"]\nmod m;\n')
        (d / "v2/kernel/vifast-core/src/sub").mkdir(parents=True, exist_ok=True)
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "KT-199/F4c `#[path]` trỏ thư mục trên `mod` không thân ⇒ đỏ E_STRICT_WAKE_INCLUDE",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        d = tmp / "kt194c-ok"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil_ok.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n",
        )
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            '#[r#path = "evil_ok.rs"]\nmod evil_ok_p;\n',
        )
        rep = scan_v2(d, strict=True, allowlist={})
        cases.append(
            (
                "KT-194c đối chứng `#[r#path]` tới tệp sản phẩm `wake_from` ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        def _broken(name: str, text: str, code: str) -> None:
            d = tmp / ("kt194c-broken-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/core_ipc.rs", text)
            rep = scan_v2(d, strict=False)
            cases.append(
                (
                    f"KT-194c đầu vào hỏng ({name}) ⇒ đỏ {code}",
                    (not rep.ok) and any(code in e for e in rep.errors),
                    "; ".join(rep.errors),
                )
            )

        _broken("chuỗi chưa đóng", 'const S: &str = "abc;\n', "E_LEX_UNTERMINATED_STRING")
        _broken("comment chưa đóng", "/* abc\n", "E_LEX_UNTERMINATED_COMMENT")
        _broken("delimiter lệch", "fn f() { (]\n", "E_LEX_UNBALANCED")
        _broken("escape sai", 'const S: &str = "a\\qb";\n', "E_LEX_INVALID_ESCAPE")
        _broken("quá sâu", "(" * (MAX_TREE_DEPTH + 2), "E_LEX_DEPTH")

        # ============ KT-209 (L40 KT-201 §D2 L2/L3) — hai lỗ tiềm ẩn ============
        # **L2** — phạm vi luật `wake` theo **mẫu**, không liệt kê cứng: mọi crate
        # `v2/kernel/<crate>/src/**` + `build.rs` và `v2/sim/<crate>/src/**` + `build.rs`.
        # Trước bản này: crate mới / `build.rs` ⇒ XANH (reviewer L40 dựng được).
        def _scope_case(name: str, relpath: str, src: str, expect_red: bool) -> None:
            d = tmp / ("kt209-l2-" + name)
            _fixture_v2(d)
            _write(d / relpath, src)
            rep = scan_v2(d, strict=True, allowlist={})
            got = any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors)
            cases.append(
                (
                    f"KT-209/L2 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _WAKE_SRC = (
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n"
        )
        _OK_SRC = (
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake_from(t);\n        0\n    }\n}\n"
        )
        _scope_case(
            "crate kernel MỚI", "v2/kernel/vifast-newcrate/src/lib.rs", _WAKE_SRC, True
        )
        _scope_case("crate sim MỚI", "v2/sim/vifast-sim2/src/lib.rs", _WAKE_SRC, True)
        _scope_case("build.rs vifast-core", "v2/kernel/vifast-core/build.rs", _WAKE_SRC, True)
        _scope_case("build.rs vifast-sim", "v2/sim/vifast-sim/build.rs", _WAKE_SRC, True)
        _scope_case(
            "tệp con sâu trong crate MỚI",
            "v2/kernel/vifast-newcrate/src/sub/deep.rs",
            _WAKE_SRC,
            True,
        )
        _scope_case(
            "ĐỐI CHỨNG DƯƠNG crate kernel MỚI (wake_from)",
            "v2/kernel/vifast-newcrate/src/lib.rs",
            _OK_SRC,
            False,
        )
        _scope_case(
            "ĐỐI CHỨNG DƯƠNG build.rs (wake_from)",
            "v2/kernel/vifast-core/build.rs",
            _OK_SRC,
            False,
        )

        # **L3** — `include!`/`#[path]` trong `v2/lib` **ngoài** thân macro được phân giải;
        # đích phải nằm **trong `v2/lib`** (fail-closed). Trước bản này: worklist chỉ seed
        # `v2/kernel`+`v2/sim` ⇒ `include!` trong `v2/lib` XANH.
        def _lib_case(name: str, lib_src: str, files: dict, expect_red: bool) -> None:
            d = tmp / ("kt209-l3-" + name)
            _fixture_v2(d)
            for relpath, text in files.items():
                _write(d / relpath, text)
            _write(d / "v2/lib/vifast-abi/src/lib.rs", lib_src)
            rep = scan_v2(d, strict=True, allowlist={})
            got = any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors)
            cases.append(
                (
                    f"KT-209/L3 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _lib_case(
            "include! lib → kernel (đích sạch) — cạnh nguồn bị kiểm",
            'include!("../../../kernel/vifast-core/src/evil.rs");\n',
            {"v2/kernel/vifast-core/src/evil.rs": "pub fn clean() {}\n"},
            True,
        )
        _lib_case(
            "include! lib → ngoài cây",
            'include!("../../../../tmp/evil_outside.rs");\n',
            {},
            True,
        )
        _lib_case(
            "include! lib → lib (đối chứng dương)",
            'include!("kt209_ok.rs");\n',
            {"v2/lib/vifast-abi/src/kt209_ok.rs": "pub fn ok() {}\n"},
            False,
        )
        _lib_case(
            "include! LỒNG lib → lib → ngoài cây",
            'include!("kt209_nested_a.rs");\n',
            {
                "v2/lib/vifast-abi/src/kt209_nested_a.rs": (
                    'include!("../../../../tmp/evil_outside.rs");\n'
                ),
            },
            True,
        )
        _lib_case(
            "#[path] lib → kernel",
            '#[path = "../../../kernel/vifast-core/src/evil.rs"]\nmod evil;\n',
            {"v2/kernel/vifast-core/src/evil.rs": "pub fn clean() {}\n"},
            True,
        )
        _lib_case(
            "#[path] lib → lib (đối chứng dương)",
            '#[path = "kt209_ok.rs"]\nmod kt209_ok;\n',
            {"v2/lib/vifast-abi/src/kt209_ok.rs": "pub fn ok() {}\n"},
            False,
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
    ap.add_argument("--selftest", action="store_true", help="chạy 64 ca tự kiểm")
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
