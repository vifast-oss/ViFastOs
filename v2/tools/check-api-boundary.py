#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""check-api-boundary.py — KT-241: chứng minh **ranh giới API** `NonReadyTarget` bằng compile-fail.

Vì sao có tệp này
-----------------
D2-187/D2-194 thu hẹp API để `TcbState::Ready` **không thể** đi vào cửa chuyển trạng thái thông
thường. Đặc tả thi hành: `v2/agent/JOURNAL/kt241-spec/API-SPEC.md`. Các đường lọt phải chết ở
**mức kiểu**, không phải bằng phân tích liên hàm:

  - **WB-16** — mang `TcbState` (kể cả `Ready`) tới `transition_tcb` qua helper cùng tệp, helper
    tệp khác, hàm trả về `TcbState`, `const`, `static`, closure (6 dạng);
  - **WB-17** — dùng `transition_tcb` làm **giá trị**: fn-pointer, fn-pointer có kiểu, truyền qua
    tham số, generic `F: Fn(…, TcbState)`, trait object, UFCS, `<Machine>::…` (7 dạng);
  - **primitive thô** — module **anh em** gọi `write_state` (private) trực tiếp hoặc qua wrapper;
  - **đường chuyển đổi bị cấm** — `From`/`TryFrom`/`into_state` từ ngoài module.

Cơ chế
------
1. **Trước khi chèn**: `git status --porcelain` của **đúng** các tệp sẽ chạm (lib.rs + tệp probe).
   Đã bẩn ⇒ **từ chối chạy** (fail-closed), không ghi đè.
2. Chèn tệp probe + dòng `mod …;` vào lib.rs, chạy `cargo check` với `--message-format=json`.
3. Khẳng định **đúng mã lỗi + đúng thông điệp + đúng `tệp:dòng`** cho từng ca; chẩn đoán lạ
   (sai nguyên nhân) ⇒ ĐỎ. Dòng kỳ vọng tính từ marker `// CASE:<id>` trong chính tệp probe,
   nên không có số dòng viết tay.
4. **Ca dương**: tệp probe dùng đúng API phải biên dịch RC=0 — nếu không, harness ĐỎ (không được
   "chặn bằng cách làm hỏng biên dịch").
5. **Khôi phục** trong `finally` + `atexit` + handler `SIGINT`/`SIGTERM`; sau khôi phục kiểm lại
   `git status` sạch cho đúng các tệp đó; không sạch ⇒ ĐỎ.

Chạy
----
    python3 v2/tools/check-api-boundary.py --root <REPO_ROOT>
    python3 v2/tools/check-api-boundary.py --stub-selftest   # 3 lượt trên crate stub trong /tmp
    python3 v2/tools/check-api-boundary.py --verbose

Exit: 0 = xanh; 1 = đỏ; 2 = dùng sai / từ chối (tệp đã bẩn).

**Trạng thái**: đã nối vào `./v2/bin/v2 check` (KT-241). Nhật ký: `v2/agent/JOURNAL/kt241-probe.md`.
"""

from __future__ import annotations

import argparse
import atexit
import fnmatch
import glob
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

# --------------------------------------------------------------------------- hằng số

PROBE_FILE = "kt241_probe.rs"
HELPER_FILE = "kt241_probe_helper.rs"

ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")

# Dòng `use` khác nhau giữa core (gốc crate) và sim (`kobj`). Thân probe dùng chung.
CORE_IMPORTS = (
    "use crate::{FaultKind, KError, Machine, NonReadyTarget, ObjId, SleepReason, TcbState};"
)
CORE_HELPER_IMPORTS = "use crate::{KError, Machine, ObjId, TcbState};"
SIM_IMPORTS = (
    "use crate::kobj::{FaultKind, KError, Machine, NonReadyTarget, ObjId, SleepReason, TcbState};"
)
SIM_HELPER_IMPORTS = "use crate::kobj::{KError, Machine, ObjId, TcbState};"

# --------------------------------------------------------------------------- nguồn probe

# Ca âm: mỗi biểu thức bị từ chối nằm trên **một** dòng, cuối dòng có marker `// CASE:<id>`.
# Harness tính dòng kỳ vọng từ marker ⇒ không có số dòng viết tay, không lệch khi sửa tệp.
NEGATIVE_BODY = r"""
// ===========================================================================
// WB-16 — 6 dạng mang `TcbState` (kể cả `Ready`) tới cửa chuyển trạng thái.
// ===========================================================================

fn wb16_f1(m: &mut Machine, t: ObjId, s: TcbState) -> Result<(), KError> {
    m.transition_tcb(t, s) // CASE:wb16-f1
}

fn wb16_f2_call(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    crate::kt241_probe_helper::wb16_f2(m, t, TcbState::Ready)
}

fn wb16_f3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    fn ready_state() -> TcbState {
        TcbState::Ready
    }
    m.transition_tcb(t, ready_state()) // CASE:wb16-f3
}

const WB16_R: TcbState = TcbState::Ready;

fn wb16_f4(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb(t, WB16_R) // CASE:wb16-f4
}

static WB16_S: TcbState = TcbState::Ready;

fn wb16_f5(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.transition_tcb(t, WB16_S) // CASE:wb16-f5
}

fn wb16_f6(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let mut f = |s: TcbState| m.transition_tcb(t, s); // CASE:wb16-f6
    f(TcbState::Ready)
}

// ===========================================================================
// WB-17 — 7 dạng dùng `transition_tcb` làm giá trị.
// ===========================================================================

fn wb17_f1(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    let tt = Machine::transition_tcb;
    tt(m, t, TcbState::Ready) // CASE:wb17-f1
}

fn wb17_f2() {
    let _tt: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = Machine::transition_tcb; // CASE:wb17-f2
}

fn call_tt(
    f: fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>,
    m: &mut Machine,
    t: ObjId,
) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}

fn wb17_f3(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call_tt(Machine::transition_tcb, m, t) // CASE:wb17-f3
}

fn call_generic<F: Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError>>(
    f: F,
    m: &mut Machine,
    t: ObjId,
) -> Result<(), KError> {
    f(m, t, TcbState::Ready)
}

fn wb17_f5a(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    call_generic(Machine::transition_tcb, m, t) // CASE:wb17-f5a
}

fn wb17_f5b() {
    let _f: &dyn Fn(&mut Machine, ObjId, TcbState) -> Result<(), KError> = &Machine::transition_tcb; // CASE:wb17-f5b
}

fn wb17_f6(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    Machine::transition_tcb(m, t, TcbState::Ready) // CASE:wb17-f6
}

fn wb17_f7(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    <Machine>::transition_tcb(m, t, TcbState::Ready) // CASE:wb17-f7
}

// ===========================================================================
// Primitive thô — module ANH EM với `state_write`; `write_state` private.
// ===========================================================================

fn raw_direct(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.write_state(t, TcbState::Ready) // CASE:raw-direct
}

pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.write_state(t, TcbState::Ready) // CASE:raw-wrapper
}

pub fn typed_wrapper(m: &mut Machine, t: ObjId, s: TcbState) -> Result<(), KError> {
    m.transition_tcb(t, s) // CASE:typed-wrapper
}

// ===========================================================================
// Đường chuyển đổi bị cấm — không `From`/`TryFrom` công khai, `into_state` private.
// ===========================================================================

fn conv_from() {
    let _ = NonReadyTarget::from(TcbState::Ready); // CASE:conv-from
}

fn conv_into() {
    let _: NonReadyTarget = TcbState::Ready.into(); // CASE:conv-into
}

fn conv_try_into() {
    let _: Result<NonReadyTarget, _> = TcbState::Ready.try_into(); // CASE:conv-tryinto
}

fn conv_into_state() {
    let target = NonReadyTarget::Inactive;
    let _ = target.into_state(); // CASE:conv-into-state
}
"""

# WB-16 f2 — helper ở **tệp khác** (biên dịch chéo tệp không đổi kiểu).
HELPER_BODY = r"""
pub fn wb16_f2(m: &mut Machine, t: ObjId, s: TcbState) -> Result<(), KError> {
    m.transition_tcb(t, s) // CASE:wb16-f2
}
"""

# Ca dương — dùng **đúng** API mới; phải biên dịch RC=0. Dựng đủ 10 biến thể để chốt hình dạng
# enum (không `#[non_exhaustive]`, đúng tên trường) + 5 cửa `Ready` công khai.
POSITIVE_BODY = r"""
pub fn positive(
    m: &mut Machine,
    t: ObjId,
    ep: ObjId,
    reply: ObjId,
    ntfn: ObjId,
    frame: ObjId,
    reason: SleepReason,
) -> Result<(), KError> {
    m.transition_tcb(t, NonReadyTarget::Inactive)?;
    m.transition_tcb(t, NonReadyTarget::Running { hart: 0 })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnSend { ep, call: true, fault: None })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnSend { ep, call: false, fault: Some(FaultKind::Exit { code: 0 }) })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnRecv { ep, reply: None })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnRecv { ep, reply: Some(reply) })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnReply { reply })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnNotification { ntfn })?;
    m.transition_tcb(t, NonReadyTarget::BlockedOnAddress { frame, offset: 0 })?;
    m.transition_tcb(t, NonReadyTarget::Sleeping { reason })?;
    m.transition_tcb(t, NonReadyTarget::Stopping)?;
    m.transition_tcb(t, NonReadyTarget::Dead)?;
    m.start_tcb(t)?;
    m.tcb_yield(t)?;
    let _ = m.block_current(0, NonReadyTarget::Sleeping { reason })?;
    Ok(())
}
"""

# --------------------------------------------------------------------------- crate stub (/tmp)

STUB_CARGO = r"""[package]
name = "vifast-core"
version = "0.1.0"
edition = "2021"

[lib]
name = "vifast_core"
path = "src/lib.rs"

[workspace]
"""

STUB_LIB = r"""#![no_std]

mod machine;
mod state_write;

pub use machine::{FaultKind, KError, Machine, ObjId, SleepReason, TcbState};
pub use state_write::NonReadyTarget;
"""

STUB_MACHINE = r"""#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcbState {
    Inactive,
    Ready,
    Running { hart: u16 },
    BlockedOnSend { ep: ObjId, call: bool, fault: Option<FaultKind> },
    BlockedOnRecv { ep: ObjId, reply: Option<ObjId> },
    BlockedOnReply { reply: ObjId },
    BlockedOnNotification { ntfn: ObjId },
    BlockedOnAddress { frame: ObjId, offset: u32 },
    Sleeping { reason: SleepReason },
    Stopping,
    Dead,
}

pub type ObjId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KError {
    InvalidArgument,
    IllegalOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    Exit { code: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepReason {
    Timeout,
    Budget,
}

pub struct Machine {
    _p: u8,
}
"""

STUB_STATE_WRITE = r"""use crate::machine::{FaultKind, KError, Machine, ObjId, SleepReason, TcbState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonReadyTarget {
    Inactive,
    Running { hart: u16 },
    BlockedOnSend { ep: ObjId, call: bool, fault: Option<FaultKind> },
    BlockedOnRecv { ep: ObjId, reply: Option<ObjId> },
    BlockedOnReply { reply: ObjId },
    BlockedOnNotification { ntfn: ObjId },
    BlockedOnAddress { frame: ObjId, offset: u32 },
    Sleeping { reason: SleepReason },
    Stopping,
    Dead,
}

impl NonReadyTarget {
    fn into_state(self) -> TcbState {
        match self {
            NonReadyTarget::Inactive => TcbState::Inactive,
            NonReadyTarget::Running { hart } => TcbState::Running { hart },
            NonReadyTarget::BlockedOnSend { ep, call, fault } => {
                TcbState::BlockedOnSend { ep, call, fault }
            }
            NonReadyTarget::BlockedOnRecv { ep, reply } => TcbState::BlockedOnRecv { ep, reply },
            NonReadyTarget::BlockedOnReply { reply } => TcbState::BlockedOnReply { reply },
            NonReadyTarget::BlockedOnNotification { ntfn } => {
                TcbState::BlockedOnNotification { ntfn }
            }
            NonReadyTarget::BlockedOnAddress { frame, offset } => {
                TcbState::BlockedOnAddress { frame, offset }
            }
            NonReadyTarget::Sleeping { reason } => TcbState::Sleeping { reason },
            NonReadyTarget::Stopping => TcbState::Stopping,
            NonReadyTarget::Dead => TcbState::Dead,
        }
    }

    pub(crate) fn is_block_target(self) -> bool {
        matches!(self, NonReadyTarget::BlockedOnSend { .. })
    }
}

impl Machine {
    fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        let _ = (tcb, to);
        Ok(())
    }

    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError> {
        self.write_state(tcb, to.into_state())
    }

    pub fn start_tcb(&mut self, tcb: ObjId) -> Result<(), KError> {
        let _ = tcb;
        Ok(())
    }

    pub fn tcb_yield(&mut self, tcb: ObjId) -> Result<(), KError> {
        let _ = tcb;
        Ok(())
    }

    pub fn block_current(&mut self, hart: u16, to: NonReadyTarget) -> Result<ObjId, KError> {
        let _ = (hart, to);
        Ok(0)
    }
}
"""

# Đột biến của stub — dùng cho **đối chứng âm của harness** (assertion phải có răng).
MUT_TRANSITION_OLD = """    pub fn transition_tcb(&mut self, tcb: ObjId, to: NonReadyTarget) -> Result<(), KError> {
        self.write_state(tcb, to.into_state())
    }"""
MUT_TRANSITION_NEW = """    pub fn transition_tcb(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {
        self.write_state(tcb, to)
    }"""
MUT_WRITE_OLD = "    fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {"
MUT_WRITE_NEW = (
    "    pub(crate) fn write_state(&mut self, tcb: ObjId, to: TcbState) -> Result<(), KError> {"
)

# --------------------------------------------------------------------------- bảng ca

# Mã lỗi kỳ vọng + mẩu thông điệp (khớp trên `rendered` + `message` + `children`).
# Nhóm mã cho đường chuyển đổi: đặc tả KT-241 cho phép nhóm `E0599`/`E0624`/`E0277`; đo thực tế
# trên stub cho `NonReadyTarget::from(..)` là `E0308` (impl phản xạ `From<T> for T` của core),
# nên nhóm gồm cả `E0308` — thông điệp vẫn phải nói đúng "expected `NonReadyTarget`, found
# `TcbState`" (không có chuyển đổi), không phải lỗi khác.
CASES = [
    # --- WB-16: 6 dạng ---
    ("wb16-f1", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "helper cùng tệp"),
    ("wb16-f2", "helper", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "helper tệp khác"),
    ("wb16-f3", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "hàm trả về `TcbState`"),
    ("wb16-f4", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "`const` kiểu `TcbState`"),
    ("wb16-f5", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "`static` kiểu `TcbState`"),
    ("wb16-f6", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "closure tham số `TcbState`"),
    # --- WB-17: 7 dạng ---
    ("wb17-f1", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "fn-pointer qua `let`"),
    ("wb17-f2", "probe", {"E0308"}, ["expected fn pointer, found fn item"],
     "fn-pointer có kiểu"),
    ("wb17-f3", "probe", {"E0308"}, ["expected fn pointer, found fn item"],
     "truyền qua tham số `fn(.., TcbState)`"),
    ("wb17-f5a", "probe", {"E0631"}, ["type mismatch in function arguments"],
     "generic `F: Fn(.., TcbState)`"),
    ("wb17-f5b", "probe", {"E0631"}, ["type mismatch in function arguments"],
     "trait object `&dyn Fn(.., TcbState)`"),
    ("wb17-f6", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "UFCS `Machine::transition_tcb`"),
    ("wb17-f7", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "`<Machine>::transition_tcb`"),
    # --- primitive thô ---
    ("raw-direct", "probe", {"E0624"}, ["method `write_state` is private"],
     "module anh em gọi `write_state` trực tiếp"),
    ("raw-wrapper", "probe", {"E0624"}, ["method `write_state` is private"],
     "wrapper `pub fn evil` gọi `write_state`"),
    ("typed-wrapper", "probe", {"E0308"}, ["expected `NonReadyTarget`, found `TcbState`"],
     "wrapper nhận `TcbState` rồi gọi `transition_tcb`"),
    # --- đường chuyển đổi bị cấm ---
    ("conv-from", "probe", {"E0308", "E0599", "E0277"},
     ["expected `NonReadyTarget`, found `TcbState`",
      "no function or associated item named `from`",
      "the trait bound"], "`NonReadyTarget::from(TcbState::Ready)`"),
    ("conv-into", "probe", {"E0277", "E0599"},
     ["the trait bound `NonReadyTarget: From<TcbState>` is not satisfied",
      "the trait bound"], "`TcbState::Ready.into()`"),
    ("conv-tryinto", "probe", {"E0277", "E0599"},
     ["the trait bound `NonReadyTarget: TryFrom<TcbState>` is not satisfied",
      "the trait bound"], "`TcbState::Ready.try_into()`"),
    ("conv-into-state", "probe", {"E0624", "E0599"},
     ["method `into_state` is private", "no method named `into_state`"],
     "`into_state()` từ ngoài module"),
]


# --------------------------------------------------------------------------- kiểu dữ liệu


class HarnessError(Exception):
    """Lỗi dùng sai / lỗi nội bộ của harness."""


class Refuse(HarnessError):
    """Từ chối chạy: tệp sẽ chạm đã bẩn (fail-closed)."""


class RestoreError(HarnessError):
    """Khôi phục không sạch — ĐỎ."""


class Case:
    __slots__ = ("cid", "where", "codes", "patterns", "note")

    def __init__(self, cid, where, codes, patterns, note=""):
        self.cid = cid
        self.where = where
        self.codes = set(codes)
        self.patterns = list(patterns)
        self.note = note


CASES = [Case(*row) for row in CASES]


class Diag:
    __slots__ = ("code", "file", "line", "col", "message", "text")

    def __init__(self, code, file, line, col, message, text):
        self.code = code
        self.file = file
        self.line = line
        self.col = col
        self.message = message
        self.text = text


class CargoRun:
    __slots__ = ("rc", "errors", "stderr", "secs")

    def __init__(self, rc, errors, stderr, secs):
        self.rc = rc
        self.errors = errors
        self.stderr = stderr
        self.secs = secs


class Analysis:
    __slots__ = ("missing", "unexpected", "seen", "lines")

    def __init__(self, missing, unexpected, seen, lines):
        self.missing = missing
        self.unexpected = unexpected
        self.seen = seen
        self.lines = lines


class TargetResult:
    __slots__ = ("name", "preflight", "negative", "positive", "analysis", "red", "reason", "secs")

    def __init__(self, name):
        self.name = name
        self.preflight = None
        self.negative = None
        self.positive = None
        self.analysis = None
        self.red = False
        self.reason = ""
        self.secs = 0.0


class Target:
    __slots__ = (
        "name", "manifest", "package", "lib_rs", "probe_dir", "cwd", "git_root",
        "probe_sources", "positive_source", "mod_lines",
    )

    def __init__(self, name, manifest, package, lib_rs, imports, helper_imports, cwd, git_root):
        self.name = name
        self.manifest = manifest
        self.package = package
        self.lib_rs = lib_rs
        self.probe_dir = os.path.dirname(lib_rs)
        self.cwd = cwd
        self.git_root = git_root
        self.probe_sources = {
            PROBE_FILE: imports + "\n" + NEGATIVE_BODY,
            HELPER_FILE: helper_imports + "\n" + HELPER_BODY,
        }
        self.positive_source = imports + "\n" + POSITIVE_BODY
        self.mod_lines = {
            PROBE_FILE: "mod kt241_probe;",
            HELPER_FILE: "mod kt241_probe_helper;",
        }


# --------------------------------------------------------------------------- tiện ích I/O


def read_text(path):
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def write_text(path, text):
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def find_cargo():
    c = shutil.which("cargo")
    if c:
        return c
    for p in (
        os.path.expanduser("~/.cargo/bin/cargo"),
        "/usr/local/cargo/bin/cargo",
        "/opt/homebrew/bin/cargo",
    ):
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


def toolchain_env(repo_root):
    """Môi trường cargo: ghim toolchain theo `v2/rust-toolchain.toml` (như `v2/bin/v2`)."""
    env = dict(os.environ)
    env.setdefault("CARGO_TERM_COLOR", "never")
    if not env.get("RUSTUP_TOOLCHAIN"):
        tc = os.path.join(repo_root, "v2", "rust-toolchain.toml")
        if os.path.isfile(tc):
            for line in read_text(tc).splitlines():
                m = re.match(r'\s*channel\s*=\s*"([^"]+)"', line)
                if m:
                    env["RUSTUP_TOOLCHAIN"] = m.group(1)
                    break
    return env


def find_marker_line(source, cid):
    marker = "// CASE:" + cid
    # Marker phải ở **cuối dòng** — nếu chỉ dùng `in`, `conv-into` khớp cả `conv-into-state`.
    hits = [i + 1 for i, line in enumerate(source.splitlines()) if line.rstrip().endswith(marker)]
    if len(hits) != 1:
        raise HarnessError(
            "marker %r xuất hiện %d lần trong nguồn probe (cần đúng 1)" % (marker, len(hits))
        )
    return hits[0]


# --------------------------------------------------------------------------- khôi phục


_GUARDS = []


def _restore_all():
    for g in list(_GUARDS):
        try:
            g.restore()
        except Exception as exc:  # noqa: BLE001 — khôi phục phải báo, không được nuốt
            print("check-api-boundary: LỖI khôi phục: %s" % exc, file=sys.stderr)


def _on_signal(signum, _frame):
    _restore_all()
    sys.exit(128 + signum)


atexit.register(_restore_all)
for _sig in (signal.SIGINT, signal.SIGTERM):
    try:
        signal.signal(_sig, _on_signal)
    except (ValueError, OSError):
        pass


class ProbeGuard:
    """Chèn tệp probe + dòng `mod` vào lib.rs, và **khôi phục** dù lỗi giữa đường."""

    def __init__(self, target):
        self.t = target
        self.original = None
        self.active = False
        self.written = []
        _GUARDS.append(self)

    def touched(self):
        return [self.t.lib_rs] + [
            os.path.join(self.t.probe_dir, n) for n in (PROBE_FILE, HELPER_FILE)
        ]

    def dirty(self):
        if not self.t.git_root:
            return ""
        rel = [os.path.relpath(p, self.t.git_root) for p in self.touched()]
        r = subprocess.run(
            ["git", "-C", self.t.git_root, "status", "--porcelain", "--"] + rel,
            capture_output=True,
            text=True,
        )
        if r.returncode != 0:
            raise HarnessError("git status lỗi (RC=%d): %s" % (r.returncode, r.stderr.strip()))
        return r.stdout.strip()

    def install(self, files):
        dirty = self.dirty()
        if dirty:
            raise Refuse(
                "tệp sẽ chạm đã bẩn — từ chối chạy (fail-closed), không ghi đè:\n" + dirty
            )
        self.original = read_text(self.t.lib_rs)
        for name, content in files.items():
            write_text(os.path.join(self.t.probe_dir, name), content)
            self.written.append(name)
        with open(self.t.lib_rs, "a", encoding="utf-8", newline="") as f:
            f.write("\n" + "\n".join(self.t.mod_lines[n] for n in files) + "\n")
        self.active = True

    def restore(self):
        if not self.active:
            return
        write_text(self.t.lib_rs, self.original)
        for name in self.written:
            p = os.path.join(self.t.probe_dir, name)
            if os.path.exists(p):
                os.remove(p)
        self.active = False
        self.written = []
        dirty = self.dirty()
        if dirty:
            raise RestoreError("sau khôi phục, tệp vẫn bẩn — ĐỎ:\n" + dirty)


# --------------------------------------------------------------------------- cargo


CARGO = None


def parse_diagnostics(stdout):
    out = []
    seen = set()
    for line in stdout.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("reason") != "compiler-message":
            continue
        d = msg.get("message") or {}
        if d.get("level") != "error":
            continue
        code = (d.get("code") or {}).get("code")
        spans = [s for s in d.get("spans", []) if s.get("is_primary")]
        sp = spans[0] if spans else {}
        text = d.get("message", "")
        for child in d.get("children", []):
            text += "\n" + child.get("message", "")
        rendered = ANSI_RE.sub("", d.get("rendered") or "")
        text = ANSI_RE.sub("", text) + "\n" + rendered
        diag = Diag(
            code=code,
            file=sp.get("file_name"),
            line=sp.get("line_start"),
            col=sp.get("column_start"),
            message=d.get("message", ""),
            text=text,
        )
        key = (diag.code, diag.file, diag.line, diag.message)
        if key in seen:
            continue
        seen.add(key)
        out.append(diag)
    return out


def cargo_check(target, env, timeout):
    cmd = [
        CARGO,
        "check",
        "--manifest-path",
        target.manifest,
        "-p",
        target.package,
        "--message-format=json",
        "--color=never",
    ]
    t0 = time.time()
    try:
        p = subprocess.run(
            cmd, cwd=target.cwd, env=env, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return CargoRun(124, [], "timeout sau %.0f s" % timeout, time.time() - t0)
    except OSError as exc:
        return CargoRun(127, [], str(exc), time.time() - t0)
    return CargoRun(p.returncode, parse_diagnostics(p.stdout), p.stderr, time.time() - t0)


# --------------------------------------------------------------------------- phân tích


def analyze_negative(target, errors):
    lines = {}
    for c in CASES:
        src = target.probe_sources[HELPER_FILE if c.where == "helper" else PROBE_FILE]
        lines[c.cid] = find_marker_line(src, c.cid)
    by_loc = {(c.where, lines[c.cid]): c for c in CASES}
    seen = {c.cid: [] for c in CASES}
    unexpected = []
    for e in errors:
        base = os.path.basename(e.file or "")
        where = "probe" if base == PROBE_FILE else ("helper" if base == HELPER_FILE else None)
        if where is None:
            unexpected.append(e)
            continue
        c = by_loc.get((where, e.line))
        if c is None or e.code not in c.codes:
            unexpected.append(e)
            continue
        seen[c.cid].append(e)
    missing = []
    for c in CASES:
        if not any(any(p in e.text for p in c.patterns) for e in seen[c.cid]):
            missing.append(c)
    return Analysis(missing=missing, unexpected=unexpected, seen=seen, lines=lines)


def format_analysis(a):
    out = []
    if a.missing:
        out.append("    THIẾU %d ca (không thấy lỗi đúng mã + thông điệp + tệp:dòng):" % len(a.missing))
        for c in a.missing:
            out.append(
                "      - %-16s %s:%-3d kỳ vọng %s / %r"
                % (c.cid, c.where, a.lines[c.cid], sorted(c.codes), c.patterns[0])
            )
    if a.unexpected:
        out.append("    LẠ %d chẩn đoán (sai nguyên nhân / ngoài tệp probe):" % len(a.unexpected))
        for e in a.unexpected[:20]:
            out.append("      - %s %s:%s: %s" % (e.code, e.file, e.line, e.message))
        if len(a.unexpected) > 20:
            out.append("      ... (+%d)" % (len(a.unexpected) - 20))
    return "\n".join(out)


def api_missing_hint(errors):
    for e in errors:
        text = e.text or ""
        if e.code in ("E0432", "E0433") and "NonReadyTarget" in text:
            return (
                "    GỢI Ý: API KT-241 CHƯA CÓ trong cây (A/B chưa hợp nhất) — thiếu "
                "`NonReadyTarget`; đây là ĐỎ đúng (ranh giới chưa tồn tại)."
            )
        if e.code == "E0599" and "transition_tcb" in text:
            return (
                "    GỢI Ý: API KT-241 CHƯA CÓ trong cây — `transition_tcb` chưa nhận "
                "`NonReadyTarget`; đây là ĐỎ đúng."
            )
    return ""


# --------------------------------------------------------------------------- chạy một target


def run_target(target, include_positive, env, timeout, verbose):
    res = TargetResult(target.name)
    t0 = time.time()
    pre = cargo_check(target, env, timeout)
    res.preflight = pre
    if pre.rc != 0:
        res.red = True
        res.reason = "preflight"
        res.secs = time.time() - t0
        return res
    guard = ProbeGuard(target)
    try:
        guard.install(
            {
                PROBE_FILE: target.probe_sources[PROBE_FILE],
                HELPER_FILE: target.probe_sources[HELPER_FILE],
            }
        )
        neg = cargo_check(target, env, timeout)
        res.negative = neg
        res.analysis = analyze_negative(target, neg.errors)
        if include_positive:
            guard.restore()
            guard.install({PROBE_FILE: target.positive_source})
            res.positive = cargo_check(target, env, timeout)
    finally:
        guard.restore()
    res.secs = time.time() - t0
    res.red = bool(res.analysis.missing or res.analysis.unexpected)
    if include_positive and res.positive is not None:
        res.red = res.red or res.positive.rc != 0 or bool(res.positive.errors)
    return res


def print_target_result(res, verbose):
    pre = res.preflight
    print(
        "--- %s: preflight cargo check ... %s (%.1f s)"
        % (res.name, "OK" if pre.rc == 0 else "FAIL", pre.secs)
    )
    if pre.rc != 0:
        print("    cây KHÔNG xanh trước khi chèn probe (RC=%d) — không đo được ranh giới:" % pre.rc)
        if pre.stderr.strip():
            print(indent(pre.stderr[-4000:]))
        return
    neg = res.negative
    a = res.analysis
    ok_neg = not (a.missing or a.unexpected)
    print(
        "--- %s: ca âm (%d ca) ... %s (%.1f s)"
        % (res.name, len(CASES), "OK" if ok_neg else "FAIL", neg.secs)
    )
    if not ok_neg:
        print(format_analysis(a))
        hint = api_missing_hint(neg.errors)
        if hint:
            print(hint)
    if res.positive is not None:
        pos = res.positive
        ok_pos = pos.rc == 0 and not pos.errors
        print(
            "--- %s: ca dương (API dùng được) ... %s (%.1f s)"
            % (res.name, "OK" if ok_pos else "FAIL", pos.secs)
        )
        if not ok_pos:
            print("    RC=%d; %d lỗi — API mới KHÔNG dùng được:" % (pos.rc, len(pos.errors)))
            for e in pos.errors[:20]:
                print("      - %s %s:%s: %s" % (e.code, e.file, e.line, e.message))
            if pos.stderr.strip():
                print(indent(pos.stderr[-2000:]))
    if verbose:
        print("    (verbose) %d chẩn đoán ca âm:" % len(neg.errors))
        for e in neg.errors:
            print("      - %s %s:%s:%s %s" % (e.code, e.file, e.line, e.col, e.message))


def indent(text):
    return "\n".join("    " + line for line in text.splitlines())


# --------------------------------------------------------------------------- chế độ thật


def real_targets(repo):
    return [
        Target(
            name="vifast-core",
            manifest=os.path.join(repo, "v2", "kernel", "Cargo.toml"),
            package="vifast-core",
            lib_rs=os.path.join(repo, "v2", "kernel", "vifast-core", "src", "lib.rs"),
            imports=CORE_IMPORTS,
            helper_imports=CORE_HELPER_IMPORTS,
            cwd=repo,
            git_root=repo,
        ),
        Target(
            name="vifast-sim",
            manifest=os.path.join(repo, "v2", "Cargo.toml"),
            package="vifast-sim",
            lib_rs=os.path.join(repo, "v2", "sim", "vifast-sim", "src", "lib.rs"),
            imports=SIM_IMPORTS,
            helper_imports=SIM_HELPER_IMPORTS,
            cwd=repo,
            git_root=repo,
        ),
    ]


def run_real(args):
    repo = os.path.abspath(args.root)
    env = toolchain_env(repo)
    t0 = time.time()
    print("== KT-241 api boundary (compile-fail) — %s ==" % repo)
    ok = True
    for target in real_targets(repo):
        res = run_target(target, True, env, args.timeout, args.verbose)
        ok = ok and not res.red
        print_target_result(res, args.verbose)
    total = time.time() - t0
    print("== KT-241 api boundary: %s (%.1f s) ==" % ("OK" if ok else "ĐỎ", total))
    return 0 if ok else 1


# --------------------------------------------------------------------------- stub selftest


def write_stub(stub_dir):
    os.makedirs(os.path.join(stub_dir, "src"), exist_ok=True)
    write_text(os.path.join(stub_dir, "Cargo.toml"), STUB_CARGO)
    write_text(os.path.join(stub_dir, "src", "lib.rs"), STUB_LIB)
    write_text(os.path.join(stub_dir, "src", "machine.rs"), STUB_MACHINE)
    write_text(os.path.join(stub_dir, "src", "state_write.rs"), STUB_STATE_WRITE)


def mutate_stub(stub_dir, kind):
    path = os.path.join(stub_dir, "src", "state_write.rs")
    src = read_text(path)
    if kind == "transition_tcb_to_tcbstate":
        if MUT_TRANSITION_OLD not in src:
            raise HarnessError("stub: không thấy khối transition_tcb để đột biến")
        src = src.replace(MUT_TRANSITION_OLD, MUT_TRANSITION_NEW)
    elif kind == "write_state_pub_crate":
        if MUT_WRITE_OLD not in src:
            raise HarnessError("stub: không thấy dòng write_state để đột biến")
        src = src.replace(MUT_WRITE_OLD, MUT_WRITE_NEW)
    else:
        raise HarnessError("stub: kiểu đột biến lạ %r" % kind)
    write_text(path, src)


def stub_target(stub_dir):
    return Target(
        name="stub",
        manifest=os.path.join(stub_dir, "Cargo.toml"),
        package="vifast-core",
        lib_rs=os.path.join(stub_dir, "src", "lib.rs"),
        imports=CORE_IMPORTS,
        helper_imports=CORE_HELPER_IMPORTS,
        cwd=stub_dir,
        git_root=None,
    )


def run_stub_selftest(args):
    repo = os.path.abspath(args.root)
    env = toolchain_env(repo)
    stub_dir = tempfile.mkdtemp(prefix="kt241-api-probe-", dir="/tmp")
    t0 = time.time()
    print("== KT-241 api boundary — STUB SELFTEST (%s) ==" % stub_dir)
    try:
        target = stub_target(stub_dir)

        # (a) stub ĐÚNG ⇒ XANH (cả ca âm lẫn ca dương).
        write_stub(stub_dir)
        a = run_target(target, True, env, args.timeout, args.verbose)
        ok_a = not a.red
        print("--- (a) stub ĐÚNG: transition_tcb(to: NonReadyTarget), write_state private")
        print(
            "    RC ca âm=%s, RC ca dương=%s, verdict=%s (kỳ vọng XANH) => %s"
            % (
                a.negative.rc if a.negative else "?",
                a.positive.rc if a.positive else "?",
                "XANH" if ok_a else "ĐỎ",
                "OK" if ok_a else "SAI",
            )
        )
        if not ok_a:
            if a.analysis is not None:
                print(format_analysis(a.analysis))
            if a.positive is not None and (a.positive.rc != 0 or a.positive.errors):
                for e in a.positive.errors[:10]:
                    print("      - %s %s:%s: %s" % (e.code, e.file, e.line, e.message))

        # (b) ĐỔI KIỂU NGƯỢC: transition_tcb nhận TcbState ⇒ ca WB-16/17 hết đỏ ⇒ harness ĐỎ.
        write_stub(stub_dir)
        mutate_stub(stub_dir, "transition_tcb_to_tcbstate")
        b = run_target(target, False, env, args.timeout, args.verbose)
        missing_b = {c.cid for c in b.analysis.missing}
        ok_b = (
            b.red
            and any(x.startswith("wb16-") for x in missing_b)
            and any(x.startswith("wb17-") for x in missing_b)
        )
        print("--- (b) ĐỔI KIỂU NGƯỢC: transition_tcb(to: TcbState)")
        print(
            "    RC=%s, verdict=%s (kỳ vọng ĐỎ), thiếu %d ca WB-16/17 => %s"
            % (
                b.negative.rc if b.negative else "?",
                "ĐỎ" if b.red else "XANH",
                len(missing_b),
                "OK" if ok_b else "SAI",
            )
        )

        # (c) NỚI QUYỀN: write_state pub(crate) ⇒ ca E0624 hết đỏ ⇒ harness ĐỎ.
        write_stub(stub_dir)
        mutate_stub(stub_dir, "write_state_pub_crate")
        c = run_target(target, False, env, args.timeout, args.verbose)
        missing_c = {x.cid for x in c.analysis.missing}
        ok_c = c.red and {"raw-direct", "raw-wrapper"} <= missing_c
        print("--- (c) NỚI QUYỀN: write_state -> pub(crate)")
        print(
            "    RC=%s, verdict=%s (kỳ vọng ĐỎ), thiếu %d ca E0624 => %s"
            % (
                c.negative.rc if c.negative else "?",
                "ĐỎ" if c.red else "XANH",
                len(missing_c),
                "OK" if ok_c else "SAI",
            )
        )

        ok = ok_a and ok_b and ok_c
        total = time.time() - t0
        print(
            "== STUB SELFTEST: %s (%.1f s) =="
            % ("OK — assertion có răng (3/3)" if ok else "SAI — assertion KHÔNG có răng", total)
        )
        return 0 if ok else 1
    finally:
        if args.keep:
            print("# giữ stub: %s" % stub_dir)
        else:
            shutil.rmtree(stub_dir, ignore_errors=True)


# --------------------------------------------------------------------------- B1.1 ratchet
#
# **KT-403 (D2-259)** — luật ratchet kiến trúc `B1.1` (một chiều, chỉ siết):
#   (1) `vifast-core` / `vifast-plat-*` **không** được thêm dependency mới sang
#       `vifast-net` / `vifast-fs` / `vifast-vfs` — driver & dịch vụ phải ra User Space (`B1.2`);
#   (2) tập tệp driver trong `v2/kernel/` (`virtio_*`, `*tcp*`, `*_svc`) **đóng băng**:
#       thêm tệp mới khớp mẫu ⇒ ĐỎ.
#
# Danh sách đóng băng ở `v2/test/baseline/b1.1-ratchet.toml`; **chỉ được giảm** (xoá mục =
# siết thêm, hợp lệ; thêm mục = nới ratchet, phải qua Agent 0). Mục *stale* (mục đã biến mất
# khỏi cây) chỉ in cảnh báo, không đỏ. Baseline thiếu/hỏng ⇒ **ĐỎ fail-closed** (không đoán).

RATCHET_REL = "v2/test/baseline/b1.1-ratchet.toml"
RATCHET_KEYS = ("targets", "sources", "driver_patterns", "dep_edges", "driver_files")


class RatchetError(Exception):
    """Baseline B1.1 thiếu/hỏng — luôn ĐỎ fail-closed (không có chế độ 'bỏ qua')."""


def _import_tomllib():
    try:
        import tomllib  # type: ignore[import-not-found]
    except Exception:  # pragma: no cover - môi trường < 3.11
        raise RatchetError("cần python ≥ 3.11 có `tomllib` để đọc baseline B1.1")
    return tomllib


def _read_crate_name(manifest):
    try:
        with open(manifest, "r", encoding="utf-8") as fh:
            text = fh.read()
    except OSError:
        return None
    m = re.search(r'^name\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return m.group(1) if m else None


def _source_matches(crate, sources):
    return any(fnmatch.fnmatchcase(crate, pat) for pat in sources)


def scan_dep_edges(root, data):
    """Cạnh phụ thuộc `<crate nguồn> -> <đích>` vào các crate bị ratchet.

    Đọc `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` **và** biến thể
    `[target.'cfg(…)'.dependencies]` (thêm dev-dep cũng là mở cửa sau ⇒ phải bắt).
    """
    targets = set(data["targets"])
    sources = list(data["sources"])
    out = set()
    pattern = os.path.join(root, "v2", "kernel", "*", "Cargo.toml")
    for manifest in sorted(glob.glob(pattern)):
        crate = _read_crate_name(manifest)
        if crate is None or not _source_matches(crate, sources):
            continue
        section = ""
        try:
            with open(manifest, "r", encoding="utf-8") as fh:
                lines = fh.read().splitlines()
        except OSError as exc:
            raise RatchetError("không đọc được %s: %s" % (manifest, exc))
        for line in lines:
            stripped = line.strip()
            head = re.match(r"^\[([^\]]+)\]$", stripped)
            if head:
                section = head.group(1).strip()
                # `[dependencies.vifast-net]` — bảng con của một dependency.
                if section.startswith("dependencies.") or section.endswith(".dependencies"):
                    parts = section.split(".")
                    if len(parts) >= 2 and parts[-1] in targets and "dependencies" in parts[:-1]:
                        out.add("%s -> %s" % (crate, parts[-1]))
                continue
            if "dependencies" not in section:
                continue
            dep = re.match(r'^([A-Za-z0-9_-]+)\s*=', stripped)
            if dep and dep.group(1) in targets:
                out.add("%s -> %s" % (crate, dep.group(1)))
    return out


def scan_driver_files(root, data):
    """Tệp driver hiện có trong `v2/kernel/` khớp mẫu đóng băng (đường dẫn tương đối gốc repo)."""
    patterns = list(data["driver_patterns"])
    exts = {".rs", ".S", ".s", ".asm"}
    out = set()
    kernel = os.path.join(root, "v2", "kernel")
    if not os.path.isdir(kernel):
        raise RatchetError("không thấy %s/v2/kernel" % root)
    for dirpath, _dirnames, filenames in os.walk(kernel):
        for name in filenames:
            stem, ext = os.path.splitext(name)
            if ext not in exts:
                continue
            if any(fnmatch.fnmatchcase(stem, pat) for pat in patterns):
                rel = os.path.relpath(os.path.join(dirpath, name), root)
                out.add(rel.replace(os.sep, "/"))
    return out


def load_ratchet(root):
    path = os.path.join(root, RATCHET_REL)
    if not os.path.isfile(path):
        raise RatchetError("B1.1-BASELINE thiếu %s — fail-closed" % RATCHET_REL)
    tomllib = _import_tomllib()
    try:
        with open(path, "rb") as fh:
            data = tomllib.load(fh)
    except Exception as exc:  # noqa: BLE001 - mọi lỗi parse đều là baseline hỏng
        raise RatchetError("B1.1-BASELINE không parse được %s: %s" % (RATCHET_REL, exc))
    if data.get("version") != 1:
        raise RatchetError("B1.1-BASELINE version=%r (cần 1)" % (data.get("version"),))
    for key in RATCHET_KEYS:
        if not isinstance(data.get(key), list):
            raise RatchetError("B1.1-BASELINE thiếu mảng `%s`" % key)
    for key in ("targets", "sources", "driver_patterns", "dep_edges", "driver_files"):
        if not all(isinstance(v, str) for v in data[key]):
            raise RatchetError("B1.1-BASELINE `%s` phải là mảng chuỗi" % key)
    return data


def check_ratchet(root, verbose=False):
    """Trả `(problems, stale, stats)`. `problems` khác rỗng ⇔ luật B1.1 ĐỎ."""
    data = load_ratchet(root)
    frozen_edges = set(data["dep_edges"])
    found_edges = scan_dep_edges(root, data)
    frozen_files = set(data["driver_files"])
    found_files = scan_driver_files(root, data)

    problems = []
    for edge in sorted(found_edges - frozen_edges):
        problems.append("B1.1-DEP %s" % edge)
    for path in sorted(found_files - frozen_files):
        problems.append("B1.1-DRIVER %s" % path)
    stale = ["dep %s" % e for e in sorted(frozen_edges - found_edges)]
    stale += ["file %s" % f for f in sorted(frozen_files - found_files)]

    print(
        "# B1.1 ratchet: dep_edges=%d (đóng băng %d, mới %d) driver_files=%d (đóng băng %d, mới %d) stale=%d"
        % (
            len(found_edges),
            len(frozen_edges),
            len(found_edges - frozen_edges),
            len(found_files),
            len(frozen_files),
            len(found_files - frozen_files),
            len(stale),
        )
    )
    if verbose:
        for item in stale:
            print("# B1.1 stale (đã biến mất, có thể xoá khỏi baseline): %s" % item)
    for p in problems:
        print(p)
    if problems:
        print(
            "# B1.1 ĐỎ: %d vi phạm ratchet — chỉ được GIẢM; thêm dependency/tệp driver phải qua Agent 0"
            % len(problems),
            file=sys.stderr,
        )
    else:
        print("# B1.1: XANH (0 vi phạm, ratchet một chiều)")
    return problems, stale, (len(found_edges), len(found_files))


def run_ratchet(args):
    try:
        problems, _stale, _stats = check_ratchet(args.root, args.verbose)
    except RatchetError as exc:
        print("check-api-boundary: ĐỎ — %s" % exc, file=sys.stderr)
        return 1
    return 1 if problems else 0


# --------------------------------------------------------------------------- B1.1 selftest
#
# **D2-113F (đỏ-trước)**: luật mới phải chứng minh nó **bắt được** vi phạm, và **không** báo oan
# trên cây hợp lệ. Mỗi ca dựng một cây mini trong `mktemp -d` (không bao giờ sửa cây thật).

MINI_BASELINE_HEAD = """\
# SPDX-License-Identifier: MIT OR Apache-2.0
version = 1
frozen_at = "2026-11-24"
targets = ["vifast-net", "vifast-fs", "vifast-vfs"]
sources = ["vifast-core", "vifast-plat-*"]
driver_patterns = ["virtio_*", "*tcp*", "*_svc"]
"""


def _mini_write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(text)


def _mini_manifest(crate, deps):
    body = "[package]\nname = \"%s\"\nversion = \"0.1.0\"\n\n[dependencies]\n" % crate
    for dep in deps:
        body += "%s = { path = \"../../lib/%s\" }\n" % (dep, dep)
    return body


def _mini_baseline(edges, files):
    out = MINI_BASELINE_HEAD + "\ndep_edges = [\n"
    out += "".join('    "%s",\n' % e for e in edges)
    out += "]\n\ndriver_files = [\n"
    out += "".join('    "%s",\n' % f for f in files)
    out += "]\n"
    return out


def _mini_tree(base, edges, files):
    """Dựng `v2/kernel/{crate}/Cargo.toml` + tệp driver + baseline mini."""
    kernel = os.path.join(base, "v2", "kernel")
    crates = {}
    for edge in edges:
        src, dst = [p.strip() for p in edge.split("->")]
        crates.setdefault(src, set()).add(dst)
    # `vifast-core` luôn có mặt (đối chứng dương: crate nguồn không phụ thuộc gì).
    crates.setdefault("vifast-core", set())
    for crate, deps in crates.items():
        _mini_write(
            os.path.join(kernel, crate, "Cargo.toml"),
            _mini_manifest(crate, sorted(deps)),
        )
    for rel in files:
        _mini_write(os.path.join(base, rel), "// SPDX-License-Identifier: MIT OR Apache-2.0\n")
    _mini_write(
        os.path.join(base, RATCHET_REL),
        _mini_baseline(sorted(edges), sorted(files)),
    )


def _mini_run(base, verbose=False):
    data = load_ratchet(base)
    frozen_edges = set(data["dep_edges"])
    frozen_files = set(data["driver_files"])
    found_edges = scan_dep_edges(base, data)
    found_files = scan_driver_files(base, data)
    problems = ["B1.1-DEP %s" % e for e in sorted(found_edges - frozen_edges)]
    problems += ["B1.1-DRIVER %s" % f for f in sorted(found_files - frozen_files)]
    return problems, len(frozen_edges - found_edges) + len(frozen_files - found_files)


def run_ratchet_selftest(args):
    """5 ca: dương XANH, 3 ca âm ĐỎ-đúng-mã, ca "giảm" vẫn XANH. Exit 0 ⇔ đúng hết."""
    base0 = tempfile.mkdtemp(prefix="kt403-b1.1-", dir="/tmp")
    total = 0
    fail = 0

    print("== B1.1 ratchet — SELFTEST (đỏ-trước, cây mini trong %s) ==" % base0)

    def run_case(tag, label, want_red, codes):
        nonlocal total, fail
        total += 1
        base = os.path.join(base0, tag)
        try:
            problems, stale = _mini_run(base)
            got_codes = [p.split(" ")[0] for p in problems]
            ok = (bool(problems) == want_red) and all(c in got_codes for c in codes)
            extra = " stale=%d" % stale if want_red is False else ""
            print(
                "%s %d - %s (đỏ=%s, mã=%s%s) => %s"
                % (
                    "ok" if ok else "not ok",
                    total,
                    label,
                    bool(problems),
                    ",".join(got_codes) if got_codes else "-",
                    extra,
                    "ĐÚNG" if ok else "SAI",
                )
            )
            if not ok:
                for p in problems:
                    print("#    vi phạm: %s" % p)
                fail += 1
        except RatchetError as exc:
            ok = want_red and "B1.1-BASELINE" in codes
            print(
                "%s %d - %s (ngoại lệ %s) => %s"
                % ("ok" if ok else "not ok", total, label, exc, "ĐÚNG" if ok else "SAI")
            )
            if not ok:
                fail += 1

    # (1) ĐỐI CHỨNG DƯƠNG — cây hợp lệ, baseline khớp: XANH.
    _mini_tree(
        os.path.join(base0, "ok"),
        ["vifast-plat-riscv64 -> vifast-net"],
        ["v2/kernel/vifast-plat-riscv64/src/virtio_net.rs"],
    )
    run_case("ok", "cây hợp lệ (baseline khớp) ⇒ XANH", False, [])

    # (2) CA ÂM — thêm dependency mới net/fs/vfs vào `plat-*`: ĐỎ `B1.1-DEP`.
    _mini_tree(
        os.path.join(base0, "dep"),
        ["vifast-plat-riscv64 -> vifast-net"],
        ["v2/kernel/vifast-plat-riscv64/src/virtio_net.rs"],
    )
    _mini_write(
        os.path.join(base0, "dep", "v2", "kernel", "vifast-plat-riscv64", "Cargo.toml"),
        _mini_manifest("vifast-plat-riscv64", ["vifast-net", "vifast-fs"]),
    )
    run_case("dep", "thêm dep `vifast-fs` vào plat-riscv64 ⇒ ĐỎ B1.1-DEP", True, ["B1.1-DEP"])

    # (3) CA ÂM — thêm tệp driver mới khớp mẫu: ĐỎ `B1.1-DRIVER`.
    _mini_tree(
        os.path.join(base0, "driver"),
        ["vifast-plat-riscv64 -> vifast-net"],
        ["v2/kernel/vifast-plat-riscv64/src/virtio_net.rs"],
    )
    _mini_write(
        os.path.join(base0, "driver", "v2", "kernel", "vifast-plat-riscv64", "src", "virtio_junk.rs"),
        "// đột biến\n",
    )
    run_case("driver", "thêm tệp `virtio_junk.rs` ⇒ ĐỎ B1.1-DRIVER", True, ["B1.1-DRIVER"])

    # (4) RATCHET CHỈ GIẢM — xoá dep + tệp khỏi cây (baseline còn mục): vẫn XANH, chỉ stale.
    _mini_tree(
        os.path.join(base0, "shrink"),
        ["vifast-plat-riscv64 -> vifast-net"],
        ["v2/kernel/vifast-plat-riscv64/src/virtio_net.rs"],
    )
    _mini_write(
        os.path.join(base0, "shrink", "v2", "kernel", "vifast-plat-riscv64", "Cargo.toml"),
        _mini_manifest("vifast-plat-riscv64", []),
    )
    os.remove(os.path.join(base0, "shrink", "v2", "kernel", "vifast-plat-riscv64", "src", "virtio_net.rs"))
    problems, stale = _mini_run(os.path.join(base0, "shrink"))
    total += 1
    ok = not problems and stale == 2
    print(
        "%s %d - giảm dep+tệp (ratchet một chiều) ⇒ XANH, stale=2 => %s"
        % ("ok" if ok else "not ok", total, "ĐÚNG" if ok else "SAI")
    )
    if not ok:
        fail += 1
        for p in problems:
            print("#    vi phạm: %s" % p)

    # (5) CA ÂM — baseline mất/hỏng ⇒ ĐỎ fail-closed `B1.1-BASELINE` (không đoán là xanh).
    _mini_tree(
        os.path.join(base0, "nobase"),
        ["vifast-plat-riscv64 -> vifast-net"],
        [],
    )
    os.remove(os.path.join(base0, "nobase", RATCHET_REL))
    run_case("nobase", "thiếu baseline ⇒ ĐỎ fail-closed B1.1-BASELINE", True, ["B1.1-BASELINE"])

    shutil.rmtree(base0, ignore_errors=True)
    if fail:
        print("== SELFTEST B1.1: SAI — %d/%d ca sai kỳ vọng ==" % (fail, total))
        return 1
    print("== SELFTEST B1.1: OK — %d/%d ca có răng (dương XANH, âm ĐỎ đúng mã) ==" % (total, total))
    return 0


# --------------------------------------------------------------------------- main


def main(argv):
    ap = argparse.ArgumentParser(
        description="KT-241 — harness compile-fail cho ranh giới API NonReadyTarget"
    )
    ap.add_argument("--root", default=None, help="gốc repo (mặc định: suy từ vị trí script)")
    ap.add_argument(
        "--stub-selftest",
        action="store_true",
        help="chạy 3 lượt trên crate stub trong /tmp (đối chứng âm của harness)",
    )
    ap.add_argument("--verbose", action="store_true", help="in mọi chẩn đoán")
    ap.add_argument("--keep", action="store_true", help="giữ thư mục stub sau khi chạy")
    ap.add_argument(
        "--ratchet",
        action="store_true",
        help="chỉ chạy luật ratchet kiến trúc B1.1 (thuần đọc tệp, không cần cargo)",
    )
    ap.add_argument(
        "--ratchet-selftest",
        action="store_true",
        help="đỏ-trước cho luật B1.1: 5 ca trên cây mini trong /tmp",
    )
    ap.add_argument("--timeout", type=float, default=300.0, help="timeout mỗi lần cargo check (giây)")
    args = ap.parse_args(argv)

    if args.root is None:
        args.root = os.path.dirname(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        )
    args.root = os.path.abspath(args.root)
    if not os.path.isdir(os.path.join(args.root, "v2")):
        print("check-api-boundary: không thấy %s/v2" % args.root, file=sys.stderr)
        return 2

    if args.ratchet_selftest:
        return run_ratchet_selftest(args)
    if args.ratchet:
        return run_ratchet(args)

    # **KT-403** — luật `B1.1` chạy **trước** (thuần đọc tệp, rẻ); vi phạm ratchet ⇒ ĐỎ ngay,
    # không đốt thời gian `cargo check` cho một cây đã sai ranh giới kiến trúc.
    rc_ratchet = run_ratchet(args)
    if rc_ratchet != 0:
        return 1

    global CARGO
    CARGO = find_cargo()
    if not CARGO:
        print("check-api-boundary: không tìm thấy cargo", file=sys.stderr)
        return 2

    try:
        if args.stub_selftest:
            return run_stub_selftest(args)
        return run_real(args)
    except Refuse as exc:
        print("check-api-boundary: TỪ CHỐI — %s" % exc, file=sys.stderr)
        return 2
    except RestoreError as exc:
        print("check-api-boundary: ĐỎ — %s" % exc, file=sys.stderr)
        return 1
    except HarnessError as exc:
        print("check-api-boundary: LỖI HARNESS — %s" % exc, file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
