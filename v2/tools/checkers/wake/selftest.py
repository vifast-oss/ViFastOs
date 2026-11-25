#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-407 — 461 ca tự kiểm của máy kiểm `wake` (tách từ `v2/tools/check-wake-paths.py`).

Nội dung ca **nguyên văn** bản gốc; chỉ đổi ba đường dẫn `__file__`-tương-đối (tệp này nằm sâu
hơn hai cấp) và hai vòng quét call-site (nay quét `core.py` thay cho `check-wake-paths.py`).
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

from . import core as _core

# Nạp **toàn bộ** namespace core (kể cả tên private) — ca tự kiểm dùng lại `_mods_of`,
# `_transition_calls`, `_test_char_ranges`, `_lex`, … đúng như bản gốc một tệp.
globals().update({k: v for k, v in vars(_core).items() if not k.startswith("__")})

_TOOLS_DIR = Path(__file__).resolve().parents[2]  # v2/tools
_REPO_ROOT = Path(__file__).resolve().parents[4]  # gốc repo
# Hai tệp được quét call-site: `core.py` (lớp 1, nay là tệp cài đặt) + lớp 2 dep-info.
_SELFTEST_SCAN_FILES = (
    Path(__file__).resolve().parent / "core.py",
    _TOOLS_DIR / "check-wake-depinfo.py",
)

# ------------------------------------------------------------------ selftest

def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _state_write_rs(missing_wake_step: bool = False) -> str:
    """`state_write.rs` giả theo **API-SPEC KT-241** (D2-194).

    Có đủ thứ mỏ neo kiểu cần: `enum NonReadyTarget` **10 biến thể, không `Ready`**,
    `into_state`/`write_state` **private**, cửa thông thường `transition_tcb`, **5 cửa
    `Ready`** (`wake_step`, `preempt_current`, `yield_to_ready`, `start_tcb`,
    `suspend_to_ready`), `tcb_yield` (gọi `yield_to_ready`, **không** gọi `write_state`), và
    `block_current(to: NonReadyTarget)`.
    """
    wake_step = (
        ""
        if missing_wake_step
        else (
            "    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {\n"
            "        self.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
        )
    )
    return (
        "pub enum NonReadyTarget {\n"
        "    Inactive,\n"
        "    Running { hart: u16 },\n"
        "    BlockedOnSend { ep: u32, call: bool, fault: Option<u8> },\n"
        "    BlockedOnRecv { ep: u32, reply: Option<u32> },\n"
        "    BlockedOnReply { reply: u32 },\n"
        "    BlockedOnNotification { ntfn: u32 },\n"
        "    BlockedOnAddress { frame: u32, offset: u32 },\n"
        "    Sleeping { reason: u8 },\n"
        "    Stopping,\n"
        "    Dead,\n"
        "}\n"
        "\n"
        "impl NonReadyTarget {\n"
        "    fn into_state(self) -> TcbState {\n"
        "        match self {\n"
        "            NonReadyTarget::Inactive => TcbState::Inactive,\n"
        "            NonReadyTarget::Running { hart } => TcbState::Running { hart },\n"
        "            NonReadyTarget::BlockedOnSend { ep, call, fault } => TcbState::BlockedOnSend { ep, call, fault },\n"
        "            NonReadyTarget::BlockedOnRecv { ep, reply } => TcbState::BlockedOnRecv { ep, reply },\n"
        "            NonReadyTarget::BlockedOnReply { reply } => TcbState::BlockedOnReply { reply },\n"
        "            NonReadyTarget::BlockedOnNotification { ntfn } => TcbState::BlockedOnNotification { ntfn },\n"
        "            NonReadyTarget::BlockedOnAddress { frame, offset } => TcbState::BlockedOnAddress { frame, offset },\n"
        "            NonReadyTarget::Sleeping { reason } => TcbState::Sleeping { reason },\n"
        "            NonReadyTarget::Stopping => TcbState::Stopping,\n"
        "            NonReadyTarget::Dead => TcbState::Dead,\n"
        "        }\n"
        "    }\n"
        "    pub(crate) fn is_block_target(self) -> bool {\n"
        "        matches!(self, NonReadyTarget::Sleeping { .. })\n"
        "    }\n"
        "}\n"
        "\n"
        "impl M {\n"
        "    fn write_state(&mut self, tcb: u32, to: TcbState) -> u8 {\n"
        "        let _ = (tcb, to);\n"
        "        0\n"
        "    }\n"
        "    pub fn transition_tcb(&mut self, tcb: u32, to: NonReadyTarget) -> u8 {\n"
        "        self.write_state(tcb, to.into_state())\n"
        "    }\n"
        + wake_step
        + "    pub(crate) fn preempt_current(&mut self, tcb: u32) -> u8 {\n"
        "        self.write_state(tcb, TcbState::Ready)\n"
        "    }\n"
        "    pub(crate) fn yield_to_ready(&mut self, tcb: u32) -> u8 {\n"
        "        self.write_state(tcb, TcbState::Ready)\n"
        "    }\n"
        "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
        "        self.yield_to_ready(tcb)\n"
        "    }\n"
        "    pub fn start_tcb(&mut self, tcb: u32) -> u8 {\n"
        "        self.write_state(tcb, TcbState::Ready)\n"
        "    }\n"
        "    pub(crate) fn suspend_to_ready(&mut self, tcb: u32) -> u8 {\n"
        "        self.write_state(tcb, TcbState::Ready)\n"
        "    }\n"
        "    pub fn block_current(&mut self, hart: u16, to: NonReadyTarget) -> u8 {\n"
        "        let _ = hart;\n"
        "        self.transition_tcb(1, to)\n"
        "    }\n"
        "}\n"
    )


def _fixture_v2(
    base: Path,
    extra_doorbell: bool = False,
    missing_wake_step: bool = False,
    extra_sched: str = "",
) -> None:
    """Cây v2 giả tối thiểu: một hộp thư, một `state_write` (API KT-241), một `apply_mail`.

    **KT-241** — `wake_step` **rời** `core_sched.rs`/`kobj.rs` sang `state_write.rs` (API-SPEC
    §3). `core_sched.rs` / tệp miền `kobj` (KT-336b) giữ `fn wake` (cửa sản phẩm phát `Wake`), `apply_mail`,
    `push_step`; chúng gọi `self.wake_step(tcb)` như trước.
    """
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
    _write(base / KERNEL_STATE_WRITE, _state_write_rs(missing_wake_step))
    _write(base / SIM_STATE_WRITE, _state_write_rs(missing_wake_step))
    _write(
        base / KERNEL_SCHED,
        "impl M {\n"
        "    fn wake(&mut self, tcb: u32) -> u8 {\n"
        "        if self.home(tcb) != self.current_hart {\n"
        "            return self.post_mail(self.home(tcb), MailboxMsg::Wake(tcb));\n"
        "        }\n"
        "        self.wake_step(tcb)\n"
        "    }\n"
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
        base / SIM_KOBJ_ROOT,
        "impl M {\n"
        "    fn wake(&mut self, tcb: u32) -> u8 {\n"
        "        if self.home(tcb) != self.current_hart {\n"
        "            return self.post_mail(self.home(tcb), MailboxMsg::Wake(tcb));\n"
        "        }\n"
        "        self.wake_step(tcb)\n"
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
    `#[path]` sang kernel, hai đối chứng dương trong `v2/lib`). Tổng **157** ca.
    **10 ca KT-181** (D2-164: thân macro ở `init/`, `vifast-plat-user/`, `tests/` của kernel/sim
    được kiểm token `wake` — bốn ca ĐỎ + bốn đối chứng `wake_from` **cùng thư mục** + một đối
    chứng nhiễu chuỗi + một ca ngoài phạm vi `v2/tools`) +
    **16 ca KT-202** (reviewer L40: `include!`/`#[path]`/`mod` trong **đối số lời gọi macro** và
    `$…:<frag>` passthrough — repro `:item` + `include!` ở **cả hai** chế độ, `:item`→`:tt`,
    `include!` trần, `:item`/`:stmt`/`:block`/`:meta`, `#[path]`/`mod` trong lời gọi, biến thể
    lồng trong `( … )`, bốn đối chứng dương + một ca `:expr` xanh có lý do). Tổng **157** ca.
    **22 ca KT-230** (reviewer L54 — đường lọt #14: `macro_rules!` **lồng** che tên macro "an
    toàn"; repro một tầng + hai tầng + hai biến thể `.txt` (`v2/lib`, `v2/kernel`) + đối chứng
    âm + sáu đối chứng dương). Tổng **281** ca.
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

        # --- **KT-267 (b)** — miễn luật arch `doorbell` theo **ngữ nghĩa**, không theo tên tệp.
        # Trước bản này `p.name.startswith("virtio_")` miễn **mọi** tệp `virtio_*.rs`; một tệp
        # `virtio_evil.rs` **không** phải driver virtio dùng `doorbell` ⇒ XANH (nợ §7.3(b)).
        # Nay chỉ miễn khi tệp **thật sự** là driver virtio (dùng `SplitQueue`/`VqLayout` hoặc
        # định nghĩa kiểu `Virtio*`/`impl … VirtioKind`), marker đọc trên **mã** đã khử chú thích.
        _KT267B = "v2/kernel/vifast-plat-riscv64/src/"
        _kt267b_cases = (
            (
                "âm: `virtio_evil.rs` không phải driver, `doorbell` trong **mã**",
                _KT267B + "virtio_evil.rs",
                "pub fn evil() -> u32 { let doorbell = 1u32; doorbell }\n",
                True,
            ),
            (
                "âm: `virtio_evil.rs` không phải driver, `doorbell` trong **chú thích**",
                _KT267B + "virtio_evil.rs",
                "// doorbell riêng\npub fn evil() -> u32 { 1 }\n",
                True,
            ),
            (
                "âm: marker `SplitQueue`/`VqLayout` chỉ trong **chú thích** ⇒ vẫn đỏ",
                _KT267B + "virtio_evil2.rs",
                "// SplitQueue VqLayout\n"
                "pub fn evil() -> u32 { let doorbell = 1u32; doorbell }\n",
                True,
            ),
            (
                "dương: driver virtio thật (virtqueue `SplitQueue`/`VqLayout`) ⇒ xanh",
                _KT267B + "virtio_ok.rs",
                "use vifast_virtio::queue::{SplitQueue, VqLayout};\n"
                "// doorbell\n"
                "pub fn ok() -> u32 { let _ = VqLayout::new(8); 1 }\n",
                False,
            ),
            (
                "dương: kiểu `Virtio*` ⇒ xanh",
                _KT267B + "virtio_ok2.rs",
                "pub struct VirtioThing { pub n: u32 }\n// doorbell\npub fn ok() -> u32 { 1 }\n",
                False,
            ),
        )
        for _i, (_n, _relp, _text, _red) in enumerate(_kt267b_cases):
            d = tmp / f"kt267b-{_i}"
            _fixture_v2(d)
            _write(d / _relp, _text)
            rep = scan_v2(d, strict=False, extra_doors=set())
            hits = [e for e in rep.errors if "E_ARCH_DOORBELL" in e]
            cases.append(
                (
                    f"KT-267 (b) {_n} ⇒ {'đỏ' if _red else 'xanh'}",
                    bool(hits) == _red,
                    "; ".join(rep.errors),
                )
            )
        # Đối chứng dương **trên tệp THẬT**: năm driver virtio thật phải được nhận là driver
        # (nếu không, luật arch báo oan trên cây thật ⇒ `v2 check` đỏ). Ca này chết nếu ai đó
        # đổi marker ngữ nghĩa sang thứ các driver thật **không** có.
        _real_root = _REPO_ROOT
        _real_virtio = (
            "v2/kernel/vifast-plat-riscv64/src/virtio_net.rs",
            "v2/kernel/vifast-plat-riscv64/src/virtio_input.rs",
            "v2/kernel/vifast-plat-riscv64/src/virtio_gpu.rs",
            "v2/kernel/vifast-plat-riscv64/src/virtio_blk.rs",
            "v2/kernel/vifast-plat-riscv64/src/virtio_console.rs",
        )
        _real_missing = [r for r in _real_virtio if not (_real_root / r).is_file()]
        _real_not_driver = [
            r
            for r in _real_virtio
            if (_real_root / r).is_file()
            and not _is_virtio_driver((_real_root / r).read_text(encoding="utf-8", errors="replace"))
        ]
        cases.append(
            (
                "KT-267 (b) đối chứng dương trên tệp THẬT: 5 driver virtio được nhận",
                not _real_missing and not _real_not_driver,
                f"missing={_real_missing} not_driver={_real_not_driver}",
            )
        )

        # (4) v2 `--strict` với transition ngoài cửa wake ⇒ đỏ.
        d = tmp / "v2-strict"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_ipc.rs",
            "impl M {\n    fn reply(&mut self, tcb: u32) -> u8 {\n        if self.tcb_state(tcb).is_blocked() {\n            return self.transition_tcb(tcb, TcbState::Ready);\n        }\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` transition ngoài cửa wake ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4b) **KT-241** — `extra_doors` khớp ⇒ xanh (đối chứng dương: allowlist cửa không
        # luôn-đỏ). Thay ca cũ "allowlist `NON_WAKE_READY_SITES` khớp ⇒ xanh": allowlist đó
        # **đã xoá** (dưới API mới không còn chỗ `transition_tcb(…, Ready)` hợp lệ ngoài cửa).
        # Nay allowlist là **cửa được gọi `write_state`**; ca này chứng minh nó được **tư vấn**
        # (không phải hardcode "luôn đỏ").
        d = tmp / "v2-strict-allow"
        _fixture_v2(d)
        _write(
            d / KERNEL_STATE_WRITE,
            _state_write_rs()
            + "impl M {\n"
            "    fn dispatch_step(&mut self, tcb: u32) -> u8 {\n"
            "        self.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors={(KERNEL_STATE_WRITE, "dispatch_step")})
        cases.append(
            (
                "v2 `--strict` + extra_doors khớp cửa ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4c) **KT-241** — `write_state` trong hàm **không** phải cửa ⇒ đỏ. Thay ca cũ "đột
        # biến trong hàm đã allowlist": nay `tcb_yield` **phải** đi qua `yield_to_ready`; gọi
        # thẳng `write_state` là bỏ kiểm điều kiện nguồn ⇒ ĐỎ `E_STRICT_STATE_WRITE`.
        d = tmp / "v2-strict-mutate"
        _fixture_v2(d)
        _write(
            d / KERNEL_STATE_WRITE,
            _state_write_rs().replace(
                "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
                "        self.yield_to_ready(tcb)\n"
                "    }\n",
                "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
                "        self.write_state(tcb, TcbState::Ready)\n"
                "    }\n",
            ),
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` `write_state` trong `tcb_yield` (phải qua `yield_to_ready`) ⇒ đỏ",
                (not rep.ok) and any("E_STRICT_STATE_WRITE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4c2) **KT-252 (WB-20)** — "cửa thứ bảy" qua **TRÙNG TÊN**: `impl Kt252Door { fn
        # wake_step { self.write_state(tcb, TcbState::Ready) } }` trong `state_write.rs`. Tên
        # khớp `STATE_WRITE_CALLERS` nhưng **kiểu khác** `write_state` ⇒ phải ĐỎ. Trước bản vá
        # luật chỉ so tên ⇒ XANH (đo: `cargo check` RC=0, cả ba lượt kiểm XANH).
        d = tmp / "v2-strict-kt252-impl-khac"
        _fixture_v2(d)
        _write(
            d / KERNEL_STATE_WRITE,
            _state_write_rs()
            + "pub struct Kt252Door;\n"
            "impl Kt252Door {\n"
            "    pub fn wake_step(m: &mut M, tcb: u32) -> u8 {\n"
            "        m.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` `fn wake_step` trùng tên trong `impl` KHÁC ⇒ đỏ (KT-252)",
                (not rep.ok) and any("E_STRICT_STATE_WRITE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4c3) **KT-252** — trùng tên trong **module con**: `mod kt252 { fn wake_step }` ⇒ ĐỎ.
        d = tmp / "v2-strict-kt252-mod"
        _fixture_v2(d)
        _write(
            d / KERNEL_STATE_WRITE,
            _state_write_rs()
            + "pub mod kt252 {\n"
            "    pub fn wake_step(m: &mut super::M, tcb: u32) -> u8 {\n"
            "        m.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` `fn wake_step` trùng tên trong module con ⇒ đỏ (KT-252)",
                (not rep.ok) and any("E_STRICT_STATE_WRITE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4c4) **KT-252** — trùng tên trong **trait impl** ⇒ ĐỎ.
        d = tmp / "v2-strict-kt252-trait"
        _fixture_v2(d)
        _write(
            d / KERNEL_STATE_WRITE,
            _state_write_rs()
            + "pub struct Kt252Door;\n"
            "pub trait Kt252Trait {\n"
            "    fn wake_step(m: &mut M, tcb: u32) -> u8;\n"
            "}\n"
            "impl Kt252Trait for Kt252Door {\n"
            "    fn wake_step(m: &mut M, tcb: u32) -> u8 {\n"
            "        m.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` `fn wake_step` trùng tên trong trait impl ⇒ đỏ (KT-252)",
                (not rep.ok) and any("E_STRICT_STATE_WRITE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4c5) **KT-252** — trùng tên ở **free fn** mức module ⇒ ĐỎ.
        d = tmp / "v2-strict-kt252-free"
        _fixture_v2(d)
        _write(
            d / KERNEL_STATE_WRITE,
            _state_write_rs()
            + "pub fn wake_step(m: &mut M, tcb: u32) -> u8 {\n"
            "    m.write_state(tcb, TcbState::Ready)\n"
            "}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` `fn wake_step` trùng tên ở free fn ⇒ đỏ (KT-252)",
                (not rep.ok) and any("E_STRICT_STATE_WRITE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4c6) **KT-252 đối chứng DƯƠNG** — sáu cửa thật (method của `impl M`, cùng kiểu với
        # `write_state`) vẫn XANH. Thiếu vế này thì một luật "luôn đỏ" cũng qua được (4c2)..(4c5).
        d = tmp / "v2-strict-kt252-duong"
        _fixture_v2(d)
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` sáu cửa thật (method của `impl M`) ⇒ xanh (KT-252 đối chứng dương)",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4d) **KT-241** — mục `extra_doors` **mục** (không còn khớp chỗ nào) ⇒ đỏ
        # (`E_STRICT_STALE`). Giữ tinh thần "allowlist mục phải cập nhật" của KT-155.
        d = tmp / "v2-strict-stale"
        _fixture_v2(d)
        rep = scan_v2(d, strict=True, extra_doors={(KERNEL_STATE_WRITE, "khong_ton_tai")})
        cases.append(
            (
                "v2 `--strict` extra_doors mục ⇒ đỏ",
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
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "v2 `--strict` transition nhiều dòng ⇒ đỏ (token-tree, KT-231)",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "`#[cfg(test)] mod tests` thật vẫn được miễn ⇒ xanh",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # (4t) Đối chứng dương cho **biến cục bộ** tên `wake` (có thật trong cây:
        # `core_tcb.rs:721`, một tệp miền `kobj` — không báo oan.
        d = tmp / "v2-strict-bien-cuc-bo-wake"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/core_tcb.rs",
            "impl M {\n    fn f(&mut self) -> u8 {\n        let mut wake = [0u32; 4];\n        wake[0] = 1;\n        for &t in &wake[..1] {\n            let _ = t;\n        }\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "B8 module sản phẩm tên `ktest_evil.rs` + `mod` từ `main.rs` ⇒ đỏ (KT-171)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4v) **KT-227 P0-4** — `#[cfg(feature = "ktest")] mod ktest_ok;` **KHÔNG** còn được
        # coi là test-only: không có bằng chứng `ktest` không bao giờ bật trong build sản phẩm
        # ⇒ fail-closed, coi là sản phẩm ⇒ `wake` trong tệp đó ĐỎ (đường lọt B3b của KT-224).
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
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                'B8b `#[cfg(feature = "ktest")] mod ktest_ok;` ⇒ đỏ (fail-closed, KT-227 P0-4)',
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "KT-172 A: `ktest_evil.rs` + `mod` từ `main.rs`, `transition_tcb(…, Ready)` "
                "trực tiếp ⇒ đỏ `E_STRICT_TRANSITION`",
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # (4ai) **KT-227 P0-4** — cùng tệp, nhưng `#[cfg(feature = "ktest")] mod` **KHÔNG**
        # còn là test-only (fail-closed) ⇒ `transition_tcb(…, Ready)` trong đó ĐỎ ở `--strict`.
        # Không có ca này thì "đỏ" ở (4ah) có thể chỉ là "đỏ mọi thứ".
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
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                'KT-172 A2: `#[cfg(feature = "ktest")] mod ktest_evil;` ⇒ đỏ (KT-227 P0-4)',
                (not rep.ok) and any("E_STRICT_TRANSITION" in e for e in rep.errors),
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
            rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
            rep = scan_v2(d, strict=True, extra_doors=set())
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
            rep = scan_v2(d, strict=True, extra_doors=set())
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

        # --- (e2) **KT-219** (L49, đường lọt #13): vùng test phải kết thúc ở **cuối mục**,
        # không được kéo sang mục kế. `#[cfg(test)]` trên nhánh `match` `A => (),` (kết thúc
        # bằng `,`, không có thân `{…}`) từng bị quét tiếp sang nhánh kế và lấy thân `{…}` của
        # nhánh **sản phẩm** làm thân mục test ⇒ nuốt `m.wake(t)`; cả hai lớp dùng chung
        # `_test_char_ranges` nên **cùng XANH** trong khi `cargo check` RC=0.
        _wake_case(
            "KT-219 nhánh match test không thân + nhánh sản phẩm wake",
            "pub fn evil(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A => (),\n"
            "        B => {\n"
            "            m.wake(t);\n"
            "        }\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            True,
        )
        _wake_case(
            "KT-219 nhánh match test có guard + nhánh sản phẩm wake",
            "pub fn evil(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A if sel > 0 => (),\n"
            "        B => { m.wake(t); }\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            True,
        )
        _wake_case(
            "KT-219 nhánh match test không thân + nhánh sản phẩm wake (feature ktest)",
            "pub fn evil(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(feature = \"ktest\")]\n"
            "        A => (),\n"
            "        B => { m.wake(t); }\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            True,
        )
        _wake_case(
            "KT-219 nhánh match test không thân + nhánh sản phẩm wake (đảo thứ tự)",
            "pub fn evil(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        B => { m.wake(t); }\n"
            "        #[cfg(test)]\n"
            "        A => (),\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            True,
        )
        _wake_case(
            "KT-219 nhánh match test không thân + nhánh sản phẩm wake (guard sản phẩm)",
            "pub fn evil(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A => (),\n"
            "        B if sel > 1 => { m.wake(t); }\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            True,
        )
        _wake_case(
            "KT-219 nhánh match test không thân + nhánh sản phẩm wake (macro)",
            "pub fn evil(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A => (),\n"
            "        B => { let _ = m.wake(t); }\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            True,
        )
        # Đối chứng dương: nhánh match **thật sự** test ⇒ vẫn miễn.
        _wake_case(
            "KT-219 nhánh match test có thân chứa wake ⇒ xanh",
            "pub fn f(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A => { m.wake(t); }\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            False,
        )
        _wake_case(
            "KT-219 nhánh match test không thân chứa wake ⇒ xanh",
            "pub fn f(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A => m.wake(t),\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            False,
        )
        _wake_case(
            "KT-219 nhánh match test có guard chứa wake ⇒ xanh",
            "pub fn f(m: &mut M, t: u32, sel: u32) {\n"
            "    match sel {\n"
            "        #[cfg(test)]\n"
            "        A if sel > 0 => m.wake(t),\n"
            "        _ => (),\n"
            "    }\n"
            "}\n",
            False,
        )
        # Các dạng mục **không thân** khác: vùng test không được kéo sang mục kế.
        _wake_case(
            "KT-219 cfg(test) use + fn sản phẩm wake",
            "#[cfg(test)]\nuse crate::M::wake;\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) const + fn sản phẩm wake",
            "#[cfg(test)]\nconst X: u32 = 0;\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) static + fn sản phẩm wake",
            "#[cfg(test)]\nstatic X: u32 = 0;\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) mod tests; + fn sản phẩm wake",
            "#[cfg(test)]\nmod tests;\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) fn không thân (trait) + fn sản phẩm wake",
            "trait T {\n    #[cfg(test)]\n    fn f();\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) field struct + fn sản phẩm wake",
            "struct S {\n    #[cfg(test)]\n    a: u32,\n    b: u32,\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) biến thể enum + fn sản phẩm wake",
            "enum E {\n    #[cfg(test)]\n    A,\n    B,\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) if + fn sản phẩm wake",
            "pub fn f(c: bool) {\n    #[cfg(test)]\n    if c { let _ = 1; }\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) if let + fn sản phẩm wake",
            "pub fn f(o: Option<u32>) {\n    #[cfg(test)]\n    if let Some(x) = o { let _ = x; }\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) if let chứa wake ⇒ xanh",
            "pub fn f(o: Option<u32>, m: &mut M, t: u32) {\n    #[cfg(test)]\n"
            "    if let Some(_x) = o { m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) let tuple + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    let x = (1, 2);\n    let _ = x;\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) let closure nhiều tham số + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    let g = |a: u32, b: u32| a + b;\n    let _ = g;\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) use nhiều nhánh + fn sản phẩm wake",
            "#[cfg(test)]\nuse crate::{M, wake};\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) macro_rules + fn sản phẩm wake",
            "#[cfg(test)]\nmacro_rules! m { () => { 1 }; }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) lời gọi macro ngoặc nhọn + fn sản phẩm wake",
            "#[cfg(test)]\nfoo! { 1 }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) lời gọi macro ngoặc tròn + fn sản phẩm wake",
            "#[cfg(test)]\nfoo!(1);\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) impl + fn sản phẩm wake",
            "#[cfg(test)]\nimpl X { fn f(&self) {} }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) trait + fn sản phẩm wake",
            "#[cfg(test)]\ntrait T { fn f(&self); }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) struct + fn sản phẩm wake",
            "#[cfg(test)]\nstruct S { a: u32 }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) enum + fn sản phẩm wake",
            "#[cfg(test)]\nenum E { A }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) union + fn sản phẩm wake",
            "#[cfg(test)]\nunion U { a: u32 }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) extern + fn sản phẩm wake",
            "#[cfg(test)]\nextern \"C\" { fn foo(); }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) unsafe + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    unsafe { let _ = 1; }\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) while + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    while false { let _ = 1; }\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) for + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    for _i in 0..1 { let _ = 1; }\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) match + fn sản phẩm wake",
            "pub fn f(x: u32) {\n    #[cfg(test)]\n    match x { _ => {} }\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) let match + fn sản phẩm wake",
            "pub fn f(x: u32) {\n    #[cfg(test)]\n    let y = match x { _ => 1 };\n    let _ = y;\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) let struct literal + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    let s = S { a: 1 };\n    let _ = s;\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) let array + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    let a = [1, 2];\n    let _ = a;\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) return block + fn sản phẩm wake",
            "pub fn f() -> u32 {\n    #[cfg(test)]\n    return { 1 };\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) async block + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    let _ = async { 1 };\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        _wake_case(
            "KT-219 cfg(test) const block + fn sản phẩm wake",
            "pub fn f() {\n    #[cfg(test)]\n    let _ = const { 1 };\n}\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
            True,
        )
        # Đối chứng dương cho các dạng mục **thật sự** test.
        _wake_case(
            "KT-219 cfg(test) fn có thân chứa wake ⇒ xanh",
            "#[cfg(test)]\nfn helper(m: &mut M, t: u32) { m.wake(t); }\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) fn generic nhiều tham số chứa wake ⇒ xanh",
            "#[cfg(test)]\nfn helper<T, U>(m: &mut M, t: u32) { m.wake(t); }\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) mod tests chứa wake ⇒ xanh",
            "#[cfg(test)]\nmod tests {\n    fn f(m: &mut M, t: u32) { m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) use nhập wake ⇒ xanh",
            "#[cfg(test)]\nuse crate::M::wake;\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) let gọi wake ⇒ xanh",
            "pub fn f(m: &mut M, t: u32) {\n    #[cfg(test)]\n    let _ = m.wake(t);\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) impl chứa wake ⇒ xanh",
            "#[cfg(test)]\nimpl X {\n    fn f(m: &mut M, t: u32) { m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) trait default method chứa wake ⇒ xanh",
            "#[cfg(test)]\ntrait T {\n    fn f(m: &mut M, t: u32) { m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) if chứa wake ⇒ xanh",
            "pub fn f(c: bool, m: &mut M, t: u32) {\n    #[cfg(test)]\n    if c { m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) match chứa wake ⇒ xanh",
            "pub fn f(x: u32, m: &mut M, t: u32) {\n    #[cfg(test)]\n    match x { _ => { m.wake(t); } }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) loop chứa wake ⇒ xanh",
            "pub fn f(m: &mut M, t: u32) {\n    #[cfg(test)]\n    loop { m.wake(t); }\n}\n",
            False,
        )
        _wake_case(
            "KT-219 cfg(test) unsafe chứa wake ⇒ xanh",
            "pub fn f(m: &mut M, t: u32) {\n    #[cfg(test)]\n    unsafe { m.wake(t); }\n}\n",
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
        rep = scan_v2(d, strict=True, extra_doors=set())
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
            rep = scan_v2(d, strict=True, extra_doors=set())
            got = any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors)
            cases.append(
                (
                    f"KT-209/L2 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        # --- (g) **KT-181** (D2-164) — thân `macro_rules!` ở **mọi** tệp nguồn sản phẩm
        # (`v2/lib`+`v2/kernel`+`v2/sim`) phải được kiểm **cả hai** luật. Trước KT-181 luật
        # token `wake` chỉ chạy ở `v2/lib` (`scan_lib_macros`); `v2/kernel`+`v2/sim` chỉ có
        # luật include (`scan_kernel_macro_includes`) ⇒ macro trong `init/`, `tests/`,
        # `vifast-plat-user/` chứa token `wake` **lọt**. Mỗi ca ĐỎ kèm **đối chứng**
        # `wake_from` trong **cùng** thư mục (không báo oan) + một đối chứng nhiễu chuỗi.
        def _kt181_case(name: str, relpath: str, body: str, expect_red: bool) -> None:
            d = tmp / ("kt181-" + name)
            _fixture_v2(d)
            _write(
                d / relpath,
                "#[macro_export]\nmacro_rules! kt181_m {\n"
                "    ($m:expr, $t:expr) => {\n        " + body + "\n    };\n}\n",
            )
            rep = scan_v2(d, strict=True, extra_doors=set())
            got = any("E_STRICT_MACRO_WAKE" in e for e in rep.errors)
            cases.append(
                (
                    f"KT-181 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
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
            rep = scan_v2(d, strict=True, extra_doors=set())
            got = any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors)
            cases.append(
                (
                    f"KT-209/L3 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _kt181_case(
            "init/ macro `wake`",
            "v2/kernel/vifast-kbin-riscv64/init/evil.rs",
            "$m.wake($t)",
            True,
        )
        _kt181_case(
            "init/ macro `wake_from` (đối chứng)",
            "v2/kernel/vifast-kbin-riscv64/init/ok.rs",
            "$m.wake_from($t)",
            False,
        )
        _kt181_case(
            "init/ macro chuỗi \"wake\" (đối chứng nhiễu)",
            "v2/kernel/vifast-kbin-riscv64/init/ok_str.rs",
            'let _ = "wake";',
            False,
        )
        _kt181_case(
            "vifast-plat-user macro `wake`",
            "v2/user/runtime/lib.rs",
            "$m.wake($t)",
            True,
        )
        _kt181_case(
            "vifast-plat-user macro `wake_from` (đối chứng)",
            "v2/user/runtime/lib.rs",
            "$m.wake_from($t)",
            False,
        )
        _kt181_case(
            "vifast-plat-common/tests macro `wake`",
            "v2/kernel/vifast-plat-common/tests/evil.rs",
            "$m.wake($t)",
            True,
        )
        _kt181_case(
            "vifast-plat-common/tests macro `wake_from` (đối chứng)",
            "v2/kernel/vifast-plat-common/tests/ok.rs",
            "$m.wake_from($t)",
            False,
        )
        _kt181_case(
            "sim/tests macro `wake`",
            "v2/sim/vifast-sim/tests/evil.rs",
            "$m.wake($t)",
            True,
        )
        _kt181_case(
            "sim/tests macro `wake_from` (đối chứng)",
            "v2/sim/vifast-sim/tests/ok.rs",
            "$m.wake_from($t)",
            False,
        )
        _kt181_case(
            "v2/tools macro `wake` (ngoài phạm vi, lý do ở hợp đồng)",
            "v2/tools/vifast-agentctl/src/lib.rs",
            "$m.wake($t)",
            False,
        )

        # --- (h) **KT-202** (reviewer L40) — hai đường lọt **sống**:
        #   (1) `include!`/`#[path]`/`mod` nằm trong **đối số lời gọi macro** (nhóm sau `!`) —
        #       `_walk_items` cũ bỏ **mù** nhóm đó nên `resolve_include_targets` không bao giờ
        #       thấy; (2) `$…:<frag>` passthrough — `_check_macro_body` cũ chỉ từ chối `:tt`.
        # Repro: `macro_rules! pass { ($i:item) => { $i }; } pass!(include! {"…"});` —
        # `cargo check` RC=0, checker `errors=0` **cả hai** chế độ. Mỗi ca ĐỎ kèm đối chứng
        # dương (lời gọi macro không include/path/mod ⇒ xanh).
        def _kt202_case(name: str, src: str, expect_red: bool, strict: bool = True) -> None:
            d = tmp / ("kt202-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/kt202_evil.rs", src)
            rep = scan_v2(d, strict=strict, extra_doors=set())
            got = not rep.ok
            cases.append(
                (
                    f"KT-202 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
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
        _kt202_case(
            "repro `:item` + `include!` trong lời gọi (chế độ thường)",
            'macro_rules! pass { ($i:item) => { $i }; }\n'
            'pass!(include! {"../../../lib/evil.rs"});\n',
            True,
            strict=False,
        )
        _kt202_case(
            "repro `:item` + `include!` trong lời gọi (--strict)",
            'macro_rules! pass { ($i:item) => { $i }; }\n'
            'pass!(include! {"../../../lib/evil.rs"});\n',
            True,
            strict=True,
        )
        _kt202_case(
            "`:item` → `:tt` vẫn đỏ (không hồi quy)",
            'macro_rules! pass { ($i:tt) => { $i }; }\n'
            'pass!(include! {"../../../lib/evil.rs"});\n',
            True,
        )
        _kt202_case(
            "`include!` trần (không macro) vẫn đỏ",
            'include! {"../../../lib/evil.rs"};\n',
            True,
        )
        _kt202_case(
            "`:item` passthrough trong thân (không lời gọi)",
            "macro_rules! pass { ($i:item) => { $i }; }\n",
            True,
        )
        _kt202_case(
            "`include!` trong lời gọi macro (thân không passthrough)",
            "macro_rules! pass { () => {}; }\n"
            'pass!(include! {"../../../lib/evil.rs"});\n',
            True,
        )
        _kt202_case(
            "`#[path]` trong lời gọi macro",
            "macro_rules! pass { () => {}; }\n"
            'pass!(#[path = "evil.rs"] mod evil;);\n',
            True,
        )
        _kt202_case(
            "`mod` trong lời gọi macro",
            "macro_rules! pass { () => {}; }\npass!(mod evil;);\n",
            True,
        )
        _kt202_case(
            "`:stmt` passthrough",
            "macro_rules! pass { ($s:stmt) => { $s }; }\n",
            True,
        )
        _kt202_case(
            "`:block` passthrough",
            "macro_rules! pass { ($b:block) => { $b }; }\n",
            True,
        )
        _kt202_case(
            "`:meta` passthrough",
            "macro_rules! pass { ($m:meta) => { $m }; }\n",
            True,
        )
        _kt202_case(
            "`:expr` passthrough ⇒ xanh (cây thật `bail` dùng `:expr`; lý do ở hợp đồng §4)",
            "macro_rules! pass { ($e:expr) => { $e }; }\n",
            False,
        )
        _kt202_case(
            "đối chứng dương: `assert_eq!`/`vec!`/`format!`",
            'fn f() { assert_eq!(1, 1); let _ = vec![1, 2]; let _ = format!("x"); }\n',
            False,
        )
        _kt202_case(
            "đối chứng dương: `#[cfg(test)]` trong lời gọi macro",
            "macro_rules! pass { () => {}; }\npass!(#[cfg(test)] fn f() {});\n",
            False,
        )
        _kt202_case(
            "biến thể lồng trong `( … )` (chỉ sửa `_walk_items` sẽ bỏ sót)",
            "macro_rules! pass { ($e:expr) => { $e }; }\n"
            'fn f() { let _ = (pass!(include! {"../../../lib/evil.rs"})); }\n',
            True,
        )
        _kt202_case(
            "đối chứng dương: lời gọi macro lồng trong `( … )` không include/path/mod",
            "macro_rules! pass { ($e:expr) => { $e }; }\n"
            "fn f() { let _ = (pass!(1 + 1)); }\n",
            False,
        )

        # --- (i) **KT-211** (reviewer L44, KT-208 §B1) — `include!` ở **vị trí biểu thức**
        # trong nhóm `( … )`/`[ … ]` (KHÔNG phải `{ … }`, KHÔNG phải đối số lời gọi macro).
        # `_walk_items` chỉ đệ quy `{ … }` ⇒ `on_include` **không bao giờ** được gọi; đích
        # `include!` không được phân giải, không bị từ chối. Sáu biến thể, `cargo check`
        # RC=0, checker `errors=0` **cả hai** chế độ (reviewer L44 đo trực tiếp). Cùng lỗ
        # với `#[path]` trên `mod` trong khối lồng `( … )` (`_walk_items` không thấy `{ … }`
        # bên trong `( … )`). Nay: **quét token toàn cục** cho `include!` + `#[path]` (mọi
        # tầng nhóm, mọi loại nhóm), bỏ qua thân `macro_rules!` (đã bị `_check_macro_body`
        # từ chối riêng). Mỗi ca ĐỎ kèm **đối chứng dương** (include_str!/include_bytes!/
        # stringify!(include)/include! hợp lệ trong cây/`#[path]` hợp lệ).
        def _kt211_case(
            name: str,
            relpath: str,
            src: str,
            expect_red: bool,
            extra_files: Optional[dict] = None,
            strict: bool = True,
            msg_contains: Optional[str] = None,
            line: Optional[int] = None,
            code: str = "E_STRICT_WAKE_INCLUDE",
        ) -> None:
            d = tmp / ("kt211-" + name)
            _fixture_v2(d)
            _write(d / "v2/lib/evil_expr.rs", "m.wake(t)\n")
            for rp, text in (extra_files or {}).items():
                _write(d / rp, text)
            _write(d / relpath, src)
            rep = scan_v2(d, strict=strict, extra_doors=set())
            hits = [e for e in rep.errors if code in e]
            got = bool(hits)
            ok = got == expect_red
            # **KT-225** (reviewer L52 KT-223 §D) — ca B29/B30 trước đây chỉ assert **mã lỗi**
            # nên khi phá fixture (đổi non-literal → literal, bỏ lồng) vẫn PASS ⇒ **không răng**.
            # Nay assert thêm **nội dung** thông điệp / **dòng** của lỗi để ca chết khi mất
            # đúng lý do.
            if ok and expect_red and msg_contains is not None:
                ok = any(msg_contains in e for e in hits)
            if ok and expect_red and line is not None:
                ok = any(f":{line}:" in e for e in hits)
            cases.append(
                (
                    f"KT-211 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    ok,
                    "; ".join(rep.errors),
                )
            )

        _KT211_CORE = "v2/kernel/vifast-core/src/kt211_evil.rs"
        _KT211_EVIL = "../../../lib/evil_expr.rs"
        _kt211_case(
            "B01 `sink(include!{…})` — đối số lời gọi HÀM trong `( … )`",
            _KT211_CORE,
            "fn kt211_sink<T>(_: T) {}\n"
            "pub fn kt211_b01() {\n"
            f'    kt211_sink(include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B02 `[include!{…}]` — mảng `[ … ]`",
            _KT211_CORE,
            "pub fn kt211_b02() {\n"
            f'    let _ = [include! {{"{_KT211_EVIL}"}}];\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B03 `(0u8, include!{…})` — tuple `( … )`",
            _KT211_CORE,
            "pub fn kt211_b03() {\n"
            f'    let _ = (0u8, include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B23 `(include!{…})` — trần trong `( … )`",
            _KT211_CORE,
            "pub fn kt211_b23() {\n"
            f'    let _ = (include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B24 `(|| include!{…})` — thân closure trong `( … )`",
            _KT211_CORE,
            "pub fn kt211_b24() {\n"
            f'    let _mut_f = (|| include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B25 `Some(include!{…})` — đối số hàm/biến thể",
            _KT211_CORE,
            "pub fn kt211_b25() {\n"
            f'    let _ = Some(include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B04 đối chứng: `{ include!{…} }` — khối `{ … }` vẫn ĐỎ",
            _KT211_CORE,
            "pub fn kt211_b04() {\n"
            f'    let _ = {{ include! {{"{_KT211_EVIL}"}} }};\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B05 đối chứng: `include!` trần câu lệnh vẫn ĐỎ",
            _KT211_CORE,
            "pub fn kt211_b05() {\n"
            f'    let _ = include! {{"{_KT211_EVIL}"}};\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B26 đối chứng dương: `include_str!` XANH",
            _KT211_CORE,
            "pub fn kt211_ok_str() {\n"
            f'    let _ = include_str!("{_KT211_EVIL}");\n'
            "}\n",
            False,
        )
        _kt211_case(
            "B26 đối chứng dương: `include_bytes!` XANH",
            _KT211_CORE,
            "pub fn kt211_ok_bytes() {\n"
            f'    let _ = include_bytes!("{_KT211_EVIL}");\n'
            "}\n",
            False,
        )
        _kt211_case(
            "B26 đối chứng dương: `stringify!(include)` XANH",
            _KT211_CORE,
            "pub fn kt211_ok_stringify() {\n"
            "    let _ = stringify!(include);\n"
            "}\n",
            False,
        )
        _kt211_case(
            "B26 đối chứng dương: `include!` hợp lệ TRONG cây, trong `( … )` XANH",
            _KT211_CORE,
            "pub fn kt211_ok_include() {\n"
            '    let _ = (include! {"kt211_ok_expr.rs"});\n'
            "}\n",
            False,
            {"v2/kernel/vifast-core/src/kt211_ok_expr.rs": "1u8\n"},
        )
        _kt211_case(
            "B28 `#[path]` trên `mod` trong khối lồng `( … )` — đích ngoài cây",
            _KT211_CORE,
            "pub fn kt211_path_leak() {\n"
            "    let _ = ({\n"
            '        #[path = "../../../lib/evil_mod.rs"]\n'
            "        mod evil_mod;\n"
            "        1u8\n"
            "    });\n"
            "}\n",
            True,
            {"v2/lib/evil_mod.rs": "m.wake(t)\n"},
        )
        _kt211_case(
            "B28 đối chứng dương: `#[path]` trên `mod` trong khối lồng `( … )` — đích trong cây",
            _KT211_CORE,
            "pub fn kt211_path_ok() {\n"
            "    let _ = ({\n"
            '        #[path = "kt211_ok_mod.rs"]\n'
            "        mod ok_mod;\n"
            "        1u8\n"
            "    });\n"
            "}\n",
            False,
            {"v2/kernel/vifast-core/src/kt211_ok_mod.rs": "pub fn ok() {}\n"},
        )
        _kt211_case(
            "B27 `include!` trong `( … )` ở `v2/lib` — đích ngoài `v2/lib`",
            "v2/lib/vifast-abi/src/lib.rs",
            "fn kt211_sink<T>(_: T) {}\n"
            "pub fn kt211_lib() {\n"
            '    kt211_sink(include! {"../../../test/evil.rs"});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B27 `include!` trong `( … )` ở `v2/sim` — đích ngoài cây",
            "v2/sim/vifast-sim/src/lib.rs",
            "fn kt211_sink<T>(_: T) {}\n"
            "pub fn kt211_sim() {\n"
            '    kt211_sink(include! {"../../../test/evil.rs"});\n'
            "}\n",
            True,
        )
        _kt211_case(
            "B29 `include!` không literal trong `( … )` — fail-closed",
            _KT211_CORE,
            "pub fn kt211_b29() {\n"
            '    let _ = (include! {concat!("../../../lib/", "evil_expr.rs")});\n'
            "}\n",
            True,
            # **KT-225** — assert **lý do** (không phải literal), không chỉ mã lỗi: phá fixture
            # (đổi `concat!` → literal trỏ ra ngoài) ⇒ thông điệp khác ⇒ ca ĐỎ.
            msg_contains="không phải literal chuỗi",
        )
        _kt211_case(
            "B30 `include!` trong `( … )` lồng 2 tầng nhóm",
            _KT211_CORE,
            "pub fn kt211_b30() {\n"
            "    let _ = (\n"
            f'        [include! {{"{_KT211_EVIL}"}}],\n'
            "        0u8,\n"
            "    );\n"
            "}\n",
            True,
            # **KT-225** — assert **dòng** của `include!` lồng (dòng 3): phá fixture (bỏ lồng,
            # đưa `include!` về dòng 2) ⇒ ca ĐỎ.
            line=3,
        )

        # --- **KT-225** (reviewer L52 KT-223 §C/§7) — bốn **báo oan** + đối chứng âm ---------
        # Bốn dạng **mã hợp lệ** từng bị ĐỎ oan; nay phải XANH. Mỗi dạng kèm **đối chứng âm**
        # (`include!` thật vẫn ĐỎ) để không nới lỏng luật.
        _kt211_case(
            "KT-225 FP1 `stringify!(include!)` — token stream, không phải lời gọi ⇒ xanh",
            _KT211_CORE,
            "pub fn kt225_fp1() {\n"
            "    let _ = stringify!(include!);\n"
            "}\n",
            False,
        )
        _kt211_case(
            "KT-225 FP1b `stringify!(include!())` — token stream ⇒ xanh",
            _KT211_CORE,
            "pub fn kt225_fp1b() {\n"
            "    let _ = stringify!(include!());\n"
            "}\n",
            False,
        )
        _kt211_case(
            "KT-225 FP2 `include != 0` (biến) — `!=` không phải `include!` ⇒ xanh",
            _KT211_CORE,
            "pub fn kt225_fp2(include: u8) {\n"
            "    if include != 0 {}\n"
            "}\n",
            False,
        )
        _kt211_case(
            "KT-225 FP3 `self.include != 0` (trường) ⇒ xanh",
            _KT211_CORE,
            "pub struct Kt225S { include: u8 }\n"
            "pub fn kt225_fp3(s: &Kt225S) {\n"
            "    if s.include != 0 {}\n"
            "}\n",
            False,
        )
        _kt211_case(
            "KT-225 FP4 `include!(\"…\",)` trailing comma — cú pháp hợp lệ ⇒ xanh",
            _KT211_CORE,
            "pub fn kt225_fp4() {\n"
            '    let _ = include!("kt211_ok_expr.rs",);\n'
            "}\n",
            False,
            {"v2/kernel/vifast-core/src/kt211_ok_expr.rs": "1u8\n"},
        )
        _kt211_case(
            "KT-225 NEG1 `include!(…, )` đích ngoài cây vẫn ĐỎ (không nới)",
            _KT211_CORE,
            "pub fn kt225_neg1() {\n"
            f'    let _ = include!("{_KT211_EVIL}",);\n'
            "}\n",
            True,
        )
        _kt211_case(
            "KT-225 NEG2 `pass!(include!{…})` (L40) vẫn ĐỎ",
            _KT211_CORE,
            "macro_rules! pass225 { ($i:item) => { $i }; }\n"
            f'pass225!(include! {{"{_KT211_EVIL}"}});\n',
            True,
            code="E_STRICT_MACRO_INCLUDE",
        )
        _kt211_case(
            "KT-225 NEG3 `pass!(include!())` vẫn ĐỎ",
            _KT211_CORE,
            "macro_rules! pass225b { () => {}; }\n"
            "pass225b!(include!());\n",
            True,
            code="E_STRICT_MACRO_INCLUDE",
        )
        _kt211_case(
            "KT-225 NEG4 `include != 0` trong thân macro ⇒ xanh (không phải include!)",
            _KT211_CORE,
            "macro_rules! kt225_m { () => { if include != 0 {} } }\n"
            "pub fn kt225_neg4() { kt225_m!(); }\n",
            False,
        )
        _kt211_case(
            "KT-225 NEG5 `concat!(include!(…))` — literal, không chở wake ⇒ xanh",
            _KT211_CORE,
            "pub fn kt225_neg5() {\n"
            '    let _ = concat!(include!("kt211_ok_expr.rs"));\n'
            "}\n",
            False,
            {"v2/kernel/vifast-core/src/kt211_ok_expr.rs": '"x"\n'},
        )
        _kt211_case(
            "KT-225 NEG6 `stringify!(include!{…})` — không mở rộng ⇒ xanh",
            _KT211_CORE,
            "pub fn kt225_neg6() {\n"
            f'    let _ = stringify!(include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            False,
        )
        _kt211_case(
            "KT-225 SHADOW `macro_rules! stringify` che tên an toàn ⇒ ĐỎ",
            _KT211_CORE,
            "macro_rules! stringify { ($x:expr) => { $x } }\n"
            "pub fn kt225_shadow() {\n"
            f'    let _ = stringify!(include!("{_KT211_EVIL}"));\n'
            "}\n",
            True,
            code="E_STRICT_MACRO_INCLUDE",
        )
        # **KT-225** — đường lọt lớp 1 `.txt` (reviewer L52 KT-223 §3): `macro_rules!` định
        # nghĩa trong tệp **không phải `.rs`** được `include!` ở vị trí item. Trước KT-225
        # `scan_macro_bodies` chỉ `rglob("*.rs")` ⇒ thân macro không bị quét ⇒ XANH cả hai
        # chế độ. Nay `scan_macro_bodies` nhận `extra_files` (mọi tệp được `include!` vào cây)
        # ⇒ `_check_macro_body` bắt `include!` trong thân ⇒ ĐỎ.
        _kt211_case(
            "KT-225 TXT `macro_rules!` trong `.txt` được include! ⇒ ĐỎ",
            "v2/kernel/vifast-core/src/kt225_txt.rs",
            "pub fn kt225_txt() {\n"
            "    let _ = evil_txt225!();\n"
            "}\n",
            True,
            {
                "v2/kernel/vifast-core/src/lib.rs": 'include!("macro_def225.txt");\nmod kt225_txt;\n',
                "v2/kernel/vifast-core/src/macro_def225.txt": (
                    'macro_rules! evil_txt225 { () => { include!("../../../lib/evil_expr.rs") } }\n'
                ),
            },
            code="E_STRICT_MACRO_INCLUDE",
        )

        # --- **KT-267 (a)** — `:expr` passthrough: đo xem `:expr` có chở được
        # `include!`/`#[path]`/`mod` từ **tệp trong phạm vi** không. Đo trên cây thật
        # (`JOURNAL/kt267-attack/`): A1 (`include!` trong đối số, macro **khác tệp**) và A2
        # (block chứa `#[path]`+`mod` trong đối số) đều **ĐỎ cả ba lượt**; `mod`/`#[path]`
        # **trực tiếp** trong đối số `:expr` là **lỗi cú pháp** (rustc: "no rules expected
        # keyword `mod`" / "expected expression, found keyword `mod`"). ⇒ `:expr` **không**
        # chở được từ tệp trong phạm vi; nợ §7.3(a) **đóng**. Bốn ca dưới canh bất biến: nếu
        # mai kia luật "đối số lời gọi" bị nới, chúng **XANH** ⇒ selftest đỏ.
        _kt211_case(
            "KT-267 (a) `:expr` + `include!` trong đối số, macro **khác tệp** ⇒ ĐỎ",
            "v2/kernel/vifast-core/src/kt267_expr_call.rs",
            "use crate::kt267_expr_def::kt267_pass;\n"
            "pub fn kt267_evil() {\n"
            f'    let _ = kt267_pass!(include! {{"{_KT211_EVIL}"}});\n'
            "}\n",
            True,
            {
                "v2/kernel/vifast-core/src/kt267_expr_def.rs": (
                    "macro_rules! kt267_pass { ($e:expr) => { $e }; }\n"
                    "pub(crate) use kt267_pass;\n"
                ),
            },
            code="E_STRICT_MACRO_INCLUDE",
        )
        _kt211_case(
            "KT-267 (a) `:expr` + block chứa `#[path]` trong đối số ⇒ ĐỎ",
            "v2/kernel/vifast-core/src/kt267_expr_block.rs",
            "macro_rules! kt267_pass { ($e:expr) => { $e }; }\n"
            "pub fn kt267_evil() {\n"
            "    let _ = kt267_pass!({\n"
            '        #[path = "kt267_evil_mod.rs"]\n'
            "        mod evil;\n"
            "        1u8\n"
            "    });\n"
            "}\n",
            True,
            {"v2/kernel/vifast-core/src/kt267_evil_mod.rs": "pub fn f() -> u8 { 1 }\n"},
            code="E_STRICT_MACRO_INCLUDE",
        )
        _kt211_case(
            "KT-267 (a) `:expr` + block chứa `mod` trong đối số ⇒ ĐỎ",
            "v2/kernel/vifast-core/src/kt267_expr_mod.rs",
            "macro_rules! kt267_pass { ($e:expr) => { $e }; }\n"
            "pub fn kt267_evil() {\n"
            "    let _ = kt267_pass!({\n"
            "        mod evil { pub fn f() -> u8 { 1 } }\n"
            "        evil::f()\n"
            "    });\n"
            "}\n",
            True,
            code="E_STRICT_MACRO_MOD",
        )
        _kt211_case(
            "KT-267 (a) đối chứng dương: `:expr` + block biểu thức thường ⇒ xanh",
            "v2/kernel/vifast-core/src/kt267_expr_ok.rs",
            "macro_rules! kt267_pass { ($e:expr) => { $e }; }\n"
            "pub fn kt267_ok() -> u8 {\n"
            "    kt267_pass!({\n"
            "        let x = 1u8;\n"
            "        x + 1\n"
            "    })\n"
            "}\n",
            False,
            code="E_STRICT_MACRO_INCLUDE",
        )

        # --- (4b) **KT-213** — gốc phân giải `#[path]` của tệp **non-mod-rs** ---------------
        # Reviewer L47 (KT-212) dựng đường lọt #11: `resolve_include_targets` truyền `p.parent`
        # làm gốc cho **mọi** tệp, nên `#[path]` nằm trong **inline module** của `src/foo.rs`
        # giải theo `src/<inline>/` thay vì `src/foo/<inline>/` (rustc). Hệ quả: `#[path]` trỏ
        # ra **ngoài `src/`** (ngoài phạm vi luật `wake`) vẫn XANH trong khi rustc biên dịch
        # tệp thật. Cùng gốc sai ở `_mods_of` (nó truyền `mod_dir` làm `cur_dir` ⇒ `#[path]`
        # **mức mục** trong `src/foo.rs` giải thành `src/foo/a.rs` thay vì `src/a.rs`).
        _EVIL_BODY = (
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n"
        )

        def _kt213_case(
            name: str,
            foo_src: str,
            files: dict,
            expect_red: bool,
            strict: bool = True,
        ) -> None:
            d = tmp / ("kt213-" + name)
            _fixture_v2(d)
            _write(d / "v2/kernel/vifast-core/src/lib.rs", "mod foo;\n")
            _write(d / "v2/kernel/vifast-core/src/foo.rs", foo_src)
            for relp, body in files.items():
                _write(d / relp, body)
            rep = scan_v2(d, strict=strict, extra_doors=set())
            got = not rep.ok
            cases.append(
                (
                    f"KT-213 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _kt213_case(
            "repro #11 `#[path]` trong inline module tệp non-mod-rs → ngoài `src/` (thường)",
            'mod inner {\n    #[path = "../../../evil.rs"]\n    mod baz;\n}\n',
            {
                "v2/kernel/vifast-core/src/foo/inner/notes.txt": "x\n",
                "v2/kernel/vifast-core/evil.rs": _EVIL_BODY,
                "v2/kernel/evil.rs": "pub fn decoy() {}\n",
            },
            True,
            strict=False,
        )
        _kt213_case(
            "repro #11 `#[path]` trong inline module tệp non-mod-rs → ngoài `src/` (--strict)",
            'mod inner {\n    #[path = "../../../evil.rs"]\n    mod baz;\n}\n',
            {
                "v2/kernel/vifast-core/src/foo/inner/notes.txt": "x\n",
                "v2/kernel/vifast-core/evil.rs": _EVIL_BODY,
                "v2/kernel/evil.rs": "pub fn decoy() {}\n",
            },
            True,
            strict=True,
        )
        _kt213_case(
            "đối chứng dương: `#[path]` trong inline module tệp non-mod-rs → tệp THẬT trong cây",
            'mod inner {\n    #[path = "ok.rs"]\n    mod baz;\n}\n',
            {"v2/kernel/vifast-core/src/foo/inner/ok.rs": "pub fn ok() {}\n"},
            False,
        )
        _kt213_case(
            "đối chứng dương: `#[path]` mức mục trong tệp non-mod-rs → tệp cạnh tệp",
            '#[path = "ok.rs"]\nmod baz;\n',
            {"v2/kernel/vifast-core/src/ok.rs": "pub fn ok() {}\n"},
            False,
        )
        _kt213_case(
            "`#[path]` mức mục trong tệp non-mod-rs → ngoài `src/` (không hồi quy)",
            '#[path = "../evil.rs"]\nmod baz;\n',
            {"v2/kernel/vifast-core/evil.rs": _EVIL_BODY},
            True,
        )
        _kt213_case(
            "đối chứng dương: `#[path]` trên inline module trong tệp non-mod-rs",
            '#[path = "sub"]\nmod inner {\n    #[path = "ok.rs"]\n    mod baz;\n}\n',
            {"v2/kernel/vifast-core/src/sub/ok.rs": "pub fn ok() {}\n"},
            False,
        )

        # `_mods_of` phải dùng **hai** gốc: `#[path]` mức mục theo thư mục tệp, `mod NAME;`
        # theo thư mục module của tệp. Trước KT-213 cả hai dùng `mod_dir` ⇒ `#[path]` sai;
        # nếu chỉ đổi `cur_dir` thành `p.parent` thì `mod NAME;` lại sai.
        d = tmp / "kt213-mods-of"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/lib.rs", "mod foo;\n")
        _write(d / "v2/kernel/vifast-core/src/foo.rs", "mod bar;\n")
        _write(d / "v2/kernel/vifast-core/src/foo/bar.rs", "pub fn bar() {}\n")
        mods = _mods_of(
            d / "v2/kernel/vifast-core/src/foo.rs", d / "v2/kernel/vifast-core/src/foo"
        )
        cases.append(
            (
                "KT-213 `mod bar;` trong tệp non-mod-rs ⇒ `src/foo/bar.rs`",
                any(str(t).endswith("src/foo/bar.rs") for t, _, _ in mods),
                f"{mods}",
            )
        )
        _write(d / "v2/kernel/vifast-core/src/baz.rs", '#[path = "bar.rs"]\nmod bar;\n')
        _write(d / "v2/kernel/vifast-core/src/bar.rs", "pub fn bar() {}\n")
        mods = _mods_of(
            d / "v2/kernel/vifast-core/src/baz.rs", d / "v2/kernel/vifast-core/src/baz"
        )
        cases.append(
            (
                "KT-213 `#[path]` mức mục trong tệp non-mod-rs ⇒ `src/bar.rs`",
                any(str(t).endswith("src/bar.rs") for t, _, _ in mods),
                f"{mods}",
            )
        )

        # --- (4c) **KT-215** — bộ lọc thư mục build `target/` -------------------------------
        # Reviewer L47 (KT-212) dựng đường lọt #12: cả **năm** chỗ quét dùng
        # `"target" in p.parts`, khớp **bất kỳ** thành phần `target` nào — kể cả `src/target/`.
        # `mod target;` + `src/target/evil.rs` chứa `wake` **biên dịch được** (`cargo check`
        # RC=0) mà checker **XANH cả hai chế độ**. Nay `_is_build_dir` chỉ bỏ qua `target`
        # đứng **trước** `src`.
        def _kt215_case(name: str, files: dict, expect_red: bool, strict: bool) -> None:
            d = tmp / ("kt215-" + name)
            _fixture_v2(d)
            for relp, body in files.items():
                _write(d / relp, body)
            rep = scan_v2(d, strict=strict, extra_doors=set())
            got = not rep.ok
            cases.append(
                (
                    f"KT-215 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _KT215_EVIL = (
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.wake(t);\n        0\n    }\n}\n"
        )
        for _strict in (False, True):
            _kt215_case(
                "repro #12 `src/target/evil.rs` chứa `wake`"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/lib.rs": "mod target;\n",
                    "v2/kernel/vifast-core/src/target/mod.rs": "mod evil;\n",
                    "v2/kernel/vifast-core/src/target/evil.rs": _KT215_EVIL,
                },
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt215_case(
                "đối chứng dương: tệp trong `v2/kernel/target/` (build dir) bị bỏ qua"
                + (" (--strict)" if _strict else " (thường)"),
                {"v2/kernel/target/kt215-probe/evil.rs": _KT215_EVIL},
                False,
                _strict,
            )
        # Bảng `_is_build_dir`: chỉ bỏ qua `target` **trước** `src` (thư mục build thật).
        _root = tmp / "kt215-builddir"
        for _relp, _want in (
            ("v2/kernel/target/x.rs", True),
            ("v2/kernel/vifast-core/target/x.rs", True),
            ("v2/kernel/vifast-core/src/target/x.rs", False),
            ("v2/kernel/vifast-core/src/foo/target/x.rs", False),
            ("v2/lib/vifast-abi/src/target/x.rs", False),
            ("v2/kernel/vifast-core/src/foo.rs", False),
        ):
            _got = _is_build_dir(_root, _root / _relp)
            cases.append(
                (
                    f"KT-215 `_is_build_dir({_relp})` ⇒ {_want}",
                    _got == _want,
                    f"{_got}",
                )
            )

        # **KT-215** — cùng lớp: miễn luật 1/2 theo **tên** `mailbox.rs` khớp mọi thư mục.
        # Nay so **đúng relpath** của hai tệp định nghĩa; tệp cùng tên ở thư mục khác là sản phẩm.
        d = tmp / "kt215-mailbox-name"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/sub/mailbox.rs",
            "impl M {\n    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        self.post_mail(1, MailboxMsg::Wake(t));\n        0\n    }\n}\n",
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-215 `src/sub/mailbox.rs` (cùng tên, khác thư mục) phát `MailboxMsg::Wake` "
                "⇒ đỏ `E_CHANNEL_PRODUCER`",
                (not rep.ok) and any("E_CHANNEL_PRODUCER" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # **KT-230** (reviewer L54, đường lọt #14) — `macro_rules!` **lồng** che tên macro
        # "an toàn". `_shadowed_safe_macros` bản KT-177 chỉ thấy `macro_rules!` mức mục
        # (`_walk_items` bỏ qua thân macro) ⇒ `matches` vẫn "an toàn" ⇒ `let wake = …;
        # matches!(m, wake, t)` được miễn local-binding ⇒ **lọt cả hai lớp** (cargo RC=0).
        # Nay `_collect_safe_macro_defs` đệ quy **mọi** tầng nhóm, và lớp 1 nhận thêm
        # `extra_files` (đích `include!`/`#[path]`, kể cả tệp không `.rs`).
        _KT230_NESTED = (
            "macro_rules! kt230_outer {\n"
            "    () => {\n"
            "        #[macro_export]\n"
            "        macro_rules! matches {\n"
            "            ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };\n"
            "        }\n"
            "    };\n"
            "}\n"
            "kt230_outer!();\n"
        )
        _KT230_NESTED2 = (
            "macro_rules! kt230_l1 {\n"
            "    () => {\n"
            "        macro_rules! kt230_l2 {\n"
            "            () => {\n"
            "                #[macro_export]\n"
            "                macro_rules! matches {\n"
            "                    ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };\n"
            "                }\n"
            "            };\n"
            "        }\n"
            "        kt230_l2!();\n"
            "    };\n"
            "}\n"
            "kt230_l1!();\n"
        )
        _KT230_CALL = (
            "impl M {\n"
            "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        let wake = 0u32;\n"
            "        let _ = wake;\n"
            "        let _ = matches!(self, wake, t);\n"
            "        0\n"
            "    }\n"
            "}\n"
        )
        _KT230_CALL_NO_BIND = (
            "impl M {\n"
            "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        let _ = matches!(self, wake, t);\n"
            "        0\n"
            "    }\n"
            "}\n"
        )

        def _kt230_case(name: str, files: dict, expect_red: bool, strict: bool) -> None:
            d = tmp / ("kt230-" + name)
            _fixture_v2(d)
            for relp, body in files.items():
                _write(d / relp, body)
            rep = scan_v2(d, strict=strict, extra_doors=set())
            got = not rep.ok
            cases.append(
                (
                    f"KT-230 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        for _strict in (False, True):
            _kt230_case(
                "repro #14 `macro_rules! matches` LỒNG trong macro khác + `let wake` + "
                "`matches!(m, wake, t)`"
                + (" (--strict)" if _strict else " (thường)"),
                {"v2/kernel/vifast-core/src/evil.rs": _KT230_NESTED + _KT230_CALL},
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "repro #14 lồng **hai** tầng (macro sinh macro sinh macro)"
                + (" (--strict)" if _strict else " (thường)"),
                {"v2/kernel/vifast-core/src/evil.rs": _KT230_NESTED2 + _KT230_CALL},
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "repro #14 biến thể `.txt` trong `v2/lib` (đích `include!` không `.rs`)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/lib/vifast-abi/src/lib.rs": 'include!("macros.txt");\n',
                    "v2/lib/vifast-abi/src/macros.txt": _KT230_NESTED,
                    "v2/kernel/vifast-core/src/evil.rs": _KT230_CALL,
                },
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "repro #14 biến thể `.txt` trong `v2/kernel` (đích `include!` vào `extra`)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/macros.txt": _KT230_NESTED,
                    "v2/kernel/vifast-core/src/evil.rs": 'include!("macros.txt");\n' + _KT230_CALL,
                },
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng âm: bỏ `let wake = …` ⇒ vẫn đỏ (không hồi quy)"
                + (" (--strict)" if _strict else " (thường)"),
                {"v2/kernel/vifast-core/src/evil.rs": _KT230_NESTED + _KT230_CALL_NO_BIND},
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng dương: `matches!(t, wake)` với macro THẬT + `let wake` ⇒ xanh "
                "(ca KT-171 B10 phải giữ)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/ok.rs": (
                        "impl M {\n"
                        "    fn hop_le(&mut self, t: u32) -> u8 {\n"
                        "        let wake = 0u32;\n"
                        "        let _ = wake;\n"
                        "        let _ = matches!(t, wake);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                False,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng dương: `stringify!(wake)` + `let wake` ⇒ xanh"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/ok.rs": (
                        "impl M {\n"
                        "    fn hop_le(&mut self) -> u8 {\n"
                        "        let wake = 0u32;\n"
                        "        let _ = wake;\n"
                        "        let _ = stringify!(wake);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                False,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng dương: `concat!(\"wa\", \"ke\")` ⇒ xanh"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/ok.rs": (
                        "impl M {\n"
                        "    fn hop_le(&mut self) -> u8 {\n"
                        "        let _ = concat!(\"wa\", \"ke\");\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                False,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng dương: `assert!(wake)` với `let wake = true` ⇒ xanh"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/ok.rs": (
                        "impl M {\n"
                        "    fn hop_le(&mut self) -> u8 {\n"
                        "        let wake = true;\n"
                        "        assert!(wake);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                False,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng dương: `macro_rules! matches` mức mục trong `v2/lib` (KT-177) ⇒ đỏ"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/lib/vifast-abi/src/lib.rs": (
                        "#[macro_export]\n"
                        "macro_rules! matches {\n"
                        "    ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };\n"
                        "}\n"
                    ),
                    "v2/kernel/vifast-core/src/evil.rs": _KT230_CALL,
                },
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt230_case(
                "đối chứng dương: macro tên KHÔNG an toàn (`evil!`) ⇒ đỏ"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/evil.rs": (
                        "macro_rules! evil {\n"
                        "    ($m:expr, $x:ident, $t:expr) => { $m.$x($t) };\n"
                        "}\n"
                        "impl M {\n"
                        "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                        "        let wake = 0u32;\n"
                        "        let _ = wake;\n"
                        "        let _ = evil!(self, wake, t);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                True,
                _strict,
            )

        # **KT-231 → KT-241** (reviewer L54, đường lọt #15) — `transition_tcb(…, Ready)` ẩn
        # bằng **alias kiểu**: `use crate::TcbState as S;` + `m.transition_tcb(t, S::Ready)`.
        # Bản KT-155 khớp **văn bản** `TcbState::Ready` trên **một dòng** ⇒ XANH cả hai lớp.
        #
        # **KT-241** — dưới API mới (`to: NonReadyTarget`) dạng này **không biên dịch được**;
        # checker giữ nó **ĐỎ** như **chốt chống đảo kiểu** (`…::Ready` là alias của
        # `TcbState`/biến thể bịa — `NonReadyTarget` không có `Ready`). Các ca cũ được **cập
        # nhật sang luật mới**, không xoá:
        #   - `S::Ready`/`Self::Ready`/path-qualified ⇒ vẫn ĐỎ (`ready_alias`/`tcbstate`);
        #   - `TcbState::Blocked` (đối chứng dương cũ) ⇒ nay **ĐỎ** (`tcbstate`): mọi
        #     `TcbState::*` làm đối số `transition_tcb` là **đảo kiểu**;
        #   - biến gán từ `TcbState::Ready` ⇒ **XANH** ở luật transition (kiểu đã loại trừ) —
        #     nhưng ca **cặp** dưới đây chứng minh **mỏ neo** bắt được nếu chữ ký bị đảo;
        #   - `TcbState::Ready` trong hàm **từng có tên** trong `NON_WAKE_READY_SITES` ⇒ vẫn
        #     **ĐỎ**: allowlist đó đã xoá, không còn chỗ nào được miễn.
        def _kt231_case(
            name: str,
            files: dict,
            expect_red: bool,
            strict: bool,
            extra_doors: Optional[set] = None,
        ) -> None:
            d = tmp / ("kt231-" + name)
            _fixture_v2(d)
            for relp, body in files.items():
                _write(d / relp, body)
            rep = scan_v2(
                d, strict=strict, extra_doors=extra_doors if extra_doors is not None else set()
            )
            got = not rep.ok
            cases.append(
                (
                    f"KT-231 {name} ⇒ {'đỏ' if expect_red else 'xanh'}",
                    got == expect_red,
                    "; ".join(rep.errors),
                )
            )

        _KT231_ALIAS = (
            "use crate::TcbState as S;\n"
            "impl M {\n"
            "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
            "        let _ = self.transition_tcb(t, S::Ready);\n"
            "        0\n"
            "    }\n"
            "}\n"
        )
        for _strict in (False, True):
            _kt231_case(
                "repro #15 alias `TcbState as S` + `S::Ready`"
                + (" (--strict)" if _strict else " (thường)"),
                {"v2/kernel/vifast-core/src/evil.rs": _KT231_ALIAS},
                True,
                _strict,
            )
        for _strict in (False, True):
            _kt231_case(
                "`Self::Ready` (alias qua Self)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/evil.rs": (
                        "impl M {\n"
                        "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                        "        let _ = self.transition_tcb(t, Self::Ready);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                True,
                _strict,
            )
        # **Cập nhật KT-241**: ca cũ "biến gán từ `TcbState::Ready` ⇒ ĐỎ" (dataflow-lite) nay
        # **XANH** ở luật transition — đúng chủ trương "đơn giản hoá theo kiểu": kiểu
        # `NonReadyTarget` đã loại trừ `Ready`, nên `let r = TcbState::Ready; transition_tcb(t, r)`
        # **không biên dịch được**. Ca **cặp** ngay dưới chứng minh **mỏ neo** là thứ bắt được
        # nếu ai đó đảo chữ ký (thay vì tin vào dataflow-lite).
        for _strict in (False, True):
            _kt231_case(
                "biến gán từ `TcbState::Ready` ⇒ xanh ở luật transition (kiểu đã loại trừ)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/evil.rs": (
                        "impl M {\n"
                        "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                        "        let r = TcbState::Ready;\n"
                        "        let _ = self.transition_tcb(t, r);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                False,
                _strict,
            )
        # **Cập nhật KT-241**: đối chứng dương cũ "`TcbState::Blocked` ngoài cửa ⇒ xanh" nay
        # **ĐỎ** — mọi `TcbState::*` làm đối số `transition_tcb` là dấu hiệu **đảo kiểu**.
        for _strict in (False, True):
            _kt231_case(
                "`TcbState::Blocked` làm đối số ⇒ đỏ (đảo kiểu)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/ok.rs": (
                        "impl M {\n"
                        "    fn hop_le(&mut self, t: u32) -> u8 {\n"
                        "        let _ = self.transition_tcb(t, TcbState::Blocked);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                True,
                _strict,
            )
        # **Cập nhật KT-241**: ca cũ "`TcbState::Ready` trong hàm có mục allowlist ⇒ xanh" nay
        # **ĐỎ** — `NON_WAKE_READY_SITES` đã xoá; `extra_doors` chỉ miễn **cửa gọi
        # `write_state`**, không miễn `transition_tcb(…, TcbState::*)`.
        for _strict in (False, True):
            _kt231_case(
                "`TcbState::Ready` trong hàm từng allowlist ⇒ vẫn đỏ (allowlist đã xoá)"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/allow.rs": (
                        "impl M {\n"
                        "    fn cho_phep(&mut self, t: u32) -> u8 {\n"
                        "        let _ = self.transition_tcb(t, TcbState::Ready);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                True,
                _strict,
                extra_doors={(KERNEL_STATE_WRITE, "cho_phep")},
            )
        for _strict in (False, True):
            _kt231_case(
                "đối chứng dương: biến **tham số** `to` (block_current) ⇒ xanh, không báo oan"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/ok.rs": (
                        "impl M {\n"
                        "    fn block_current(&mut self, t: u32, to: NonReadyTarget) -> u8 {\n"
                        "        let _ = self.transition_tcb(t, to);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                False,
                _strict,
            )
        for _strict in (False, True):
            _kt231_case(
                "path-qualified `vifast_core::TcbState::Ready` ngoài cửa ⇒ đỏ"
                + (" (--strict)" if _strict else " (thường)"),
                {
                    "v2/kernel/vifast-core/src/evil.rs": (
                        "impl M {\n"
                        "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                        "        let _ = self.transition_tcb(t, vifast_core::TcbState::Ready);\n"
                        "        0\n"
                        "    }\n"
                        "}\n"
                    )
                },
                True,
                _strict,
            )

        # **KT-241 — mỏ neo kiểu + luật `write_state`** (D2-194 §3). Mỗi ca dựng **đúng một**
        # vi phạm trên cây fixture (đã có `state_write.rs` hợp lệ) và assert **mã lỗi cụ thể**
        # — không nhận Đỏ vì lý do khác (`E_LEX_*`/cây rỗng) làm bằng chứng.
        def _kt241_case(
            name: str,
            expect: str,
            strict: bool = False,
            files: Optional[dict] = None,
            mutate: Optional[dict] = None,
            extra_doors: Optional[set] = None,
        ) -> None:
            """`expect ∈ {"green", "anchor-missing", "anchor-type", "state-write",
            "transition"}`."""
            d = tmp / ("kt241-" + name)
            _fixture_v2(d)
            for relp, body in (files or {}).items():
                _write(d / relp, body)
            for relp, (old, new) in (mutate or {}).items():
                p = d / relp
                text = p.read_text(encoding="utf-8")
                if old not in text:
                    cases.append(
                        (
                            f"KT-241 {name} ⇒ {expect}",
                            False,
                            f"ca không dựng được: `{old[:40]}…` không có trong {relp}",
                        )
                    )
                    return
                _write(p, text.replace(old, new))
            rep = scan_v2(
                d, strict=strict, extra_doors=extra_doors if extra_doors is not None else set()
            )
            if expect == "green":
                ok = rep.ok
            else:
                code = {
                    "anchor-missing": "E_ANCHOR_MISSING",
                    "anchor-type": "E_ANCHOR_TYPE",
                    "anchor-api": "E_ANCHOR_API",
                    "nonready-conv": "E_STRICT_NONREADY_CONV",
                    "state-write": "E_STRICT_STATE_WRITE",
                    "transition": "E_STRICT_TRANSITION",
                }[expect]
                ok = (not rep.ok) and any(code in e for e in rep.errors)
            cases.append(
                (
                    f"KT-241 {name} ⇒ {expect}" + (" (--strict)" if strict else ""),
                    ok,
                    "; ".join(rep.errors),
                )
            )

        # --- mỏ neo: thiếu / sai kiểu / sai mức công khai --------------------------------
        d = tmp / "kt241-anchor-no-file"
        _fixture_v2(d)
        (d / KERNEL_STATE_WRITE).unlink()
        rep = scan_v2(d, strict=False, extra_doors=set())
        cases.append(
            (
                "KT-241 mỏ neo: xoá `state_write.rs` ⇒ E_ANCHOR_MISSING",
                (not rep.ok) and any("E_ANCHOR_MISSING" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )
        _kt241_case(
            "mỏ neo: `NonReadyTarget` có biến thể `Ready` ⇒ E_ANCHOR_TYPE",
            "anchor-type",
            mutate={KERNEL_STATE_WRITE: ("    Dead,\n}", "    Dead,\n    Ready,\n}")},
        )
        _kt241_case(
            "mỏ neo: `transition_tcb` tham số thứ hai `TcbState` (đảo kiểu) ⇒ E_ANCHOR_TYPE",
            "anchor-type",
            mutate={
                KERNEL_STATE_WRITE: (
                    "pub fn transition_tcb(&mut self, tcb: u32, to: NonReadyTarget) -> u8 {",
                    "pub fn transition_tcb(&mut self, tcb: u32, to: TcbState) -> u8 {",
                )
            },
        )
        _kt241_case(
            "mỏ neo: `block_current` tham số thứ hai `TcbState` ⇒ E_ANCHOR_TYPE",
            "anchor-type",
            mutate={
                KERNEL_STATE_WRITE: (
                    "pub fn block_current(&mut self, hart: u16, to: NonReadyTarget) -> u8 {",
                    "pub fn block_current(&mut self, hart: u16, to: TcbState) -> u8 {",
                )
            },
        )
        _kt241_case(
            "mỏ neo: `write_state` thành `pub(crate)` ⇒ E_ANCHOR_API",
            "anchor-api",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    fn write_state(&mut self, tcb: u32, to: TcbState) -> u8 {",
                    "    pub(crate) fn write_state(&mut self, tcb: u32, to: TcbState) -> u8 {",
                )
            },
        )
        _kt241_case(
            "mỏ neo: `into_state` thành `pub` ⇒ E_ANCHOR_API",
            "anchor-api",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    fn into_state(self) -> TcbState {",
                    "    pub fn into_state(self) -> TcbState {",
                )
            },
        )
        _kt241_case(
            "E1: `impl From<TcbState> for NonReadyTarget` ⇒ E_STRICT_NONREADY_CONV",
            "nonready-conv",
            files={
                KERNEL_STATE_WRITE: _state_write_rs()
                + "impl From<TcbState> for NonReadyTarget {\n"
                "    fn from(_s: TcbState) -> NonReadyTarget {\n"
                "        NonReadyTarget::Dead\n"
                "    }\n"
                "}\n"
            },
        )
        _kt241_case(
            "E1: `pub fn tu_state(s: TcbState) -> NonReadyTarget` ⇒ E_STRICT_NONREADY_CONV",
            "nonready-conv",
            files={
                KERNEL_STATE_WRITE: _state_write_rs()
                + "pub fn tu_state(s: TcbState) -> NonReadyTarget {\n"
                "    let _ = s;\n"
                "    NonReadyTarget::Dead\n"
                "}\n"
            },
        )
        # **E2** — `TryFrom` cùng lỗ hổng (orphan rule cho phép trong crate).
        _kt241_case(
            "E2: `impl TryFrom<TcbState> for NonReadyTarget` ⇒ E_STRICT_NONREADY_CONV",
            "nonready-conv",
            files={
                KERNEL_STATE_WRITE: _state_write_rs()
                + "impl TryFrom<TcbState> for NonReadyTarget {\n"
                "    type Error = ();\n"
                "    fn try_from(_s: TcbState) -> Result<Self, ()> {\n"
                "        Ok(NonReadyTarget::Dead)\n"
                "    }\n"
                "}\n"
            },
        )
        # **E1 (chiều ngược)** — `Into<NonReadyTarget> for TcbState`.
        _kt241_case(
            "E1: `impl Into<NonReadyTarget> for TcbState` ⇒ E_STRICT_NONREADY_CONV",
            "nonready-conv",
            files={
                KERNEL_STATE_WRITE: _state_write_rs()
                + "impl Into<NonReadyTarget> for TcbState {\n"
                "    fn into(self) -> NonReadyTarget {\n"
                "        let _ = self;\n"
                "        NonReadyTarget::Dead\n"
                "    }\n"
                "}\n"
            },
        )
        # **E1 (inherent trên đích)** — `impl NonReadyTarget { fn from_state(s: TcbState) -> Self }`.
        _kt241_case(
            "E1: `impl NonReadyTarget { fn from_state(s: TcbState) -> Self }` ⇒ ĐỎ",
            "nonready-conv",
            files={
                KERNEL_STATE_WRITE: _state_write_rs()
                + "impl NonReadyTarget {\n"
                "    fn from_state(_s: TcbState) -> Self {\n"
                "        NonReadyTarget::Dead\n"
                "    }\n"
                "}\n"
            },
        )
        # **E1 (inherent trên nguồn)** — `impl TcbState { fn to_target(self) -> NonReadyTarget }`.
        _kt241_case(
            "E1: `impl TcbState { fn to_target(self) -> NonReadyTarget }` ⇒ ĐỎ",
            "nonready-conv",
            files={
                KERNEL_STATE_WRITE: _state_write_rs()
                + "impl TcbState {\n"
                "    fn to_target(self) -> NonReadyTarget {\n"
                "        let _ = self;\n"
                "        NonReadyTarget::Dead\n"
                "    }\n"
                "}\n"
            },
        )
        # **E1 (phạm vi `v2/lib`)** — impl nằm ngoài `v2/kernel`+`v2/sim` vẫn phải ĐỎ.
        _kt241_case(
            "E1: `impl From<TcbState> for NonReadyTarget` trong `v2/lib` ⇒ ĐỎ (phạm vi rộng)",
            "nonready-conv",
            files={
                "v2/lib/vifast-abi/src/evil_conv.rs": (
                    "impl From<TcbState> for NonReadyTarget {\n"
                    "    fn from(_s: TcbState) -> NonReadyTarget {\n"
                    "        NonReadyTarget::Dead\n"
                    "    }\n"
                    "}\n"
                )
            },
        )
        # **E5** — `pub(super)` cũng là nới ranh giới.
        _kt241_case(
            "E5: `pub(super) fn write_state` ⇒ E_ANCHOR_API",
            "anchor-api",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    fn write_state(&mut self, tcb: u32, to: TcbState) -> u8 {",
                    "    pub(super) fn write_state(&mut self, tcb: u32, to: TcbState) -> u8 {",
                )
            },
        )
        # **E5** — `pub fn into_state` (đã có ca `pub`, thêm `pub(crate)`).
        _kt241_case(
            "E5: `pub(crate) fn into_state` ⇒ E_ANCHOR_API",
            "anchor-api",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    fn into_state(self) -> TcbState {",
                    "    pub(crate) fn into_state(self) -> TcbState {",
                )
            },
        )
        # **E7** — đột biến mỏ neo: đổi `transition_tcb` về `TcbState` ⇒ ĐỎ (đối chứng âm).
        _kt241_case(
            "E7: đột biến mỏ neo — `transition_tcb` về `TcbState` ⇒ E_ANCHOR_TYPE",
            "anchor-type",
            mutate={
                KERNEL_STATE_WRITE: (
                    "pub fn transition_tcb(&mut self, tcb: u32, to: NonReadyTarget) -> u8 {",
                    "pub fn transition_tcb(&mut self, tcb: u32, to: TcbState) -> u8 {",
                )
            },
        )
        _kt241_case(
            "mỏ neo: `core_tcb.rs` còn `fn transition_tcb(` (cửa cũ) ⇒ E_ANCHOR_TYPE",
            "anchor-type",
            files={
                "v2/kernel/vifast-core/src/core_tcb.rs": (
                    "impl M {\n"
                    "    pub fn transition_tcb(&mut self, tcb: u32, to: TcbState) -> u8 {\n"
                    "        let _ = (tcb, to);\n"
                    "        0\n"
                    "    }\n"
                    "}\n"
                )
            },
        )
        _kt241_case(
            "mỏ neo: thiếu cửa `suspend_to_ready` ⇒ E_ANCHOR_MISSING",
            "anchor-missing",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    pub(crate) fn suspend_to_ready(&mut self, tcb: u32) -> u8 {\n"
                    "        self.write_state(tcb, TcbState::Ready)\n"
                    "    }\n",
                    "",
                )
            },
        )

        # --- luật `write_state`/`into_state` --------------------------------------------
        _kt241_case(
            "luật: `write_state` ngoài `state_write.rs` ⇒ E_STRICT_STATE_WRITE",
            "state-write",
            files={
                "v2/kernel/vifast-core/src/evil.rs": (
                    "impl M {\n"
                    "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                    "        self.write_state(t, TcbState::Ready)\n"
                    "    }\n"
                    "}\n"
                )
            },
        )
        _kt241_case(
            "luật: `into_state` ngoài `state_write.rs` ⇒ E_STRICT_STATE_WRITE",
            "state-write",
            files={
                "v2/kernel/vifast-core/src/evil.rs": (
                    "impl M {\n"
                    "    fn duong_san_pham(&mut self, t: NonReadyTarget) -> u8 {\n"
                    "        let _ = t.into_state();\n"
                    "        0\n"
                    "    }\n"
                    "}\n"
                )
            },
        )
        _kt241_case(
            "luật: `write_state` trong `tcb_yield` (phải qua `yield_to_ready`) ⇒ ĐỎ",
            "state-write",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
                    "        self.yield_to_ready(tcb)\n"
                    "    }\n",
                    "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
                    "        self.write_state(tcb, TcbState::Ready)\n"
                    "    }\n",
                )
            },
        )
        _kt241_case(
            "luật: `write_state` dùng làm giá trị ⇒ ĐỎ",
            "state-write",
            mutate={
                KERNEL_STATE_WRITE: (
                    "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
                    "        self.yield_to_ready(tcb)\n"
                    "    }\n",
                    "    pub fn tcb_yield(&mut self, tcb: u32) -> u8 {\n"
                    "        let f = Self::write_state;\n"
                    "        let _ = f;\n"
                    "        self.yield_to_ready(tcb)\n"
                    "    }\n",
                )
            },
        )
        _kt241_case(
            "đối chứng dương: `state_write.rs` hợp lệ (5 cửa + `tcb_yield` qua `yield_to_ready`) ⇒ xanh",
            "green",
        )
        _kt241_case(
            "đối chứng dương: `write_state` trong cửa `start_tcb` ⇒ xanh",
            "green",
            strict=True,
        )
        _kt241_case(
            "đối chứng dương: `into_state` trong `transition_tcb` ⇒ xanh",
            "green",
            strict=True,
        )
        # **Cặp chứng minh mỏ neo thay dataflow-lite**: cùng văn bản "biến gán từ
        # `TcbState::Ready`" — XANH khi chữ ký đúng, ĐỎ `E_ANCHOR_TYPE` khi chữ ký bị đảo.
        _kt241_case(
            "cặp: biến gán từ `TcbState::Ready` + chữ ký bị đảo ⇒ E_ANCHOR_TYPE",
            "anchor-type",
            files={
                "v2/kernel/vifast-core/src/evil.rs": (
                    "impl M {\n"
                    "    fn duong_san_pham(&mut self, t: u32) -> u8 {\n"
                    "        let r = TcbState::Ready;\n"
                    "        let _ = self.transition_tcb(t, r);\n"
                    "        0\n"
                    "    }\n"
                    "}\n"
                )
            },
            mutate={
                KERNEL_STATE_WRITE: (
                    "pub fn transition_tcb(&mut self, tcb: u32, to: NonReadyTarget) -> u8 {",
                    "pub fn transition_tcb(&mut self, tcb: u32, to: TcbState) -> u8 {",
                )
            },
        )

        # **KT-239 → KT-241 (WB-16/WB-17, D2-190 §2)** — repro BỀN cho 6 dạng WB-16 + 7 biến
        # thể WB-17. Mỗi biến thể dựng đúng mã journal (kt234-interfn.md, kt232-fnptr.md),
        # chạy ở **cả hai** chế độ, qua một lệnh (`check-wake-paths.py --selftest`).
        #
        # **KT-241 đổi kỳ vọng** (D2-194 §3): dưới API `to: NonReadyTarget`, **cả 13 dạng**
        # WB-16/WB-17 **bị chặn ở biên dịch** (`E0308`/`E0631`) — đó là cách đóng của KT-241,
        # không phải checker. Vì vậy:
        #   - ca gốc: checker **XANH** (đúng — kiểu đã loại trừ `Ready`; checker **không** cần
        #     và **không được** mô phỏng lại hệ thống kiểu);
        #   - ca **cặp** `-anchor`: **cùng văn bản**, nhưng chữ ký `transition_tcb` bị **đảo**
        #     về `TcbState` ⇒ **ĐỎ `E_ANCHOR_TYPE`**. Cặp này chứng minh **mỏ neo kiểu** là thứ
        #     bắt được đường lọt cũ nếu ai đó đảo API — thay cho `_var_may_be_ready` đã bỏ.
        #   - WB-17 f6 (UFCS) vẫn **ĐỎ `E_STRICT_TRANSITION`** vì đối số thứ ba là
        #     `TcbState::Ready` (đảo kiểu ngay trong văn bản).
        # Vì sao không dựng cây kernel thật + cargo check ở đây: selftest của lớp 1 là kiểm máy
        # **không cần cargo** (fixture tối thiểu + `_fixture_v2`); tính "biên dịch được" đã được
        # journal gốc đo trên cây thật và giữ nguyên trong `kt239-repro-harden.md` +
        # `kt241-checker.md`. Ca tự kiểm assert **mã lỗi cụ thể**, không nhận Đỏ vì lý do khác
        # (neo thiếu/cây rỗng/lex) làm bằng chứng.
        _KT239_EVIL = "v2/kernel/vifast-core/src/evil.rs"

        def _kt239_case(
            name: str,
            body: str,
            expect: str,
            strict: bool,
            relp: str = _KT239_EVIL,
            calls: Optional[List[Tuple[str, str]]] = None,
            need: str = "transition_tcb",
            site: Optional[Tuple[int, str]] = None,
            append: bool = False,
            label: Optional[str] = None,
            revert_sig: bool = False,
        ) -> None:
            """Dựng fixture, chạy `scan_v2`, rồi **chứng minh nguyên nhân** — không chỉ
            assert mã lỗi:

            - `calls`: phân loại **bắt buộc** của `_transition_calls` trên đúng văn bản
              (`[(kind, desc)]`). Đây là mắt xích chứng minh checker **thấy đúng đối số**:
              WB-16 ⇒ `("ok", …)` (biến/call/const — kiểu đã loại trừ); UFCS ⇒
              `("tcbstate", "TcbState::Ready")`; WB-17 fn-pointer ⇒ `[]` (không có lời gọi).
            - `need`: chuỗi **phải** có trong văn bản — chặn ca rỗng nghĩa.
            - `site`: với ca Đỏ transition, `(dòng, tên hàm)` mà thông điệp lỗi **phải** nêu.
            - `revert_sig`: đảo chữ ký `transition_tcb` trong `state_write.rs` về `TcbState`
              (ca **cặp** chứng minh mỏ neo).
            """
            d = tmp / ("kt239-" + name)
            _fixture_v2(d)
            if revert_sig:
                swp = d / KERNEL_STATE_WRITE
                swp.write_text(
                    swp.read_text(encoding="utf-8").replace(
                        "pub fn transition_tcb(&mut self, tcb: u32, to: NonReadyTarget) -> u8 {",
                        "pub fn transition_tcb(&mut self, tcb: u32, to: TcbState) -> u8 {",
                    ),
                    encoding="utf-8",
                )
            old = (d / relp).read_text(encoding="utf-8") if (d / relp).is_file() else ""
            _write(d / relp, (old + body) if append else body)
            rep = scan_v2(d, strict=strict, extra_doors=set())
            why: List[str] = []
            if need and need not in body:
                why.append(f"văn bản ca thiếu `{need}`")
            if calls is not None:
                got = [(k, dsc) for _, k, dsc, _s in _transition_calls(body, relp)]
                if got != list(calls):
                    why.append(f"phân loại lời gọi {got} ≠ kỳ vọng {list(calls)}")
            if expect == "red-transition":
                ok = (not rep.ok) and all(
                    e.startswith("E_STRICT_TRANSITION") for e in rep.errors
                )
                ok = ok and any("đảo ngược" in e or "không chứng minh" in e for e in rep.errors)
                if site is not None:
                    _ln, _fn = site
                    ok = ok and any(f"{relp}:{_ln}" in e and _fn in e for e in rep.errors)
            elif expect == "anchor-type":
                ok = (not rep.ok) and any("E_ANCHOR_TYPE" in e for e in rep.errors)
            else:  # "green" — đối chứng dương: phải XANH, không báo oan
                ok = rep.ok
            cases.append(
                (
                    f"KT-239 {label or name} ⇒ "
                    + {
                        "red-transition": "đỏ E_STRICT_TRANSITION",
                        "anchor-type": "đỏ E_ANCHOR_TYPE (mỏ neo bắt đảo kiểu)",
                    }.get(expect, "xanh (kiểu đã loại trừ)")
                    + (" (--strict)" if strict else " (thường)"),
                    ok and not why,
                    "; ".join(rep.errors + why),
                )
            )

        # --- WB-16 (6 dạng, journal kt234-interfn.md §2/§3.4) ---------------------------
        # Mỗi mục: (văn bản, phân loại `_transition_calls` bắt buộc, chuỗi phải có).
        _KT239_WB16 = {
            # f1 — helper cùng tệp: `Ready` ở call site `helper(…, Ready)`, lời gọi
            # `transition_tcb(t, s)` trong `helper` chỉ thấy `var s` ⇒ `("ok", "s")`.
            "wb16-f1-helper-cung-tep": (
                "fn helper(m: &mut M, t: u32, s: TcbState) -> u8 {\n"
                "    let _ = m.transition_tcb(t, s);\n"
                "    0\n"
                "}\n"
                "impl M {\n"
                "    fn kt239_f1(&mut self, t: u32) -> u8 {\n"
                "        let _ = helper(self, t, TcbState::Ready);\n"
                "        0\n"
                "    }\n"
                "}\n",
                [("ok", "s")],
                "transition_tcb",
            ),
            # f2 — helper ở TỆP KHÁC: call site `helper(…, Ready)` không chứa token
            # `transition_tcb`; tệp helper thấy `("ok", "s")`.
            "wb16-f2-helper-tep-khac": (
                "impl M {\n"
                "    fn kt239_f2(&mut self, t: u32) -> u8 {\n"
                "        let _ = crate::kt239_f2_helper::helper(self, t, TcbState::Ready);\n"
                "        0\n"
                "    }\n"
                "}\n",
                [],
                "TcbState::Ready",
            ),
            # f3 — hàm TRẢ VỀ Ready: đối số thứ hai là `ready_state()` (call) ⇒ `("ok", …)`.
            "wb16-f3-ham-tra-ve-ready": (
                "fn ready_state() -> TcbState {\n"
                "    TcbState::Ready\n"
                "}\n"
                "impl M {\n"
                "    fn kt239_f3(&mut self, t: u32) -> u8 {\n"
                "        let _ = self.transition_tcb(t, ready_state());\n"
                "        0\n"
                "    }\n"
                "}\n",
                [("ok", "ready_state")],
                "transition_tcb",
            ),
            # f4 — const: `R` là const ngoài hàm ⇒ `("ok", "R")`.
            "wb16-f4-const": (
                "const R: TcbState = TcbState::Ready;\n"
                "impl M {\n"
                "    fn kt239_f4(&mut self, t: u32) -> u8 {\n"
                "        let _ = self.transition_tcb(t, R);\n"
                "        0\n"
                "    }\n"
                "}\n",
                [("ok", "R")],
                "transition_tcb",
            ),
            # f5 — static: như const.
            "wb16-f5-static": (
                "static R: TcbState = TcbState::Ready;\n"
                "impl M {\n"
                "    fn kt239_f5(&mut self, t: u32) -> u8 {\n"
                "        let _ = self.transition_tcb(t, R);\n"
                "        0\n"
                "    }\n"
                "}\n",
                [("ok", "R")],
                "transition_tcb",
            ),
            # f6 — closure: IIFE `(|s| transition_tcb(t, s))(Ready)` — lời gọi trong
            # closure chỉ thấy `("ok", "s")`.
            "wb16-f6-closure": (
                "impl M {\n"
                "    fn kt239_f6(&mut self, t: u32) -> u8 {\n"
                "        let _ = (|s: TcbState| self.transition_tcb(t, s))(TcbState::Ready);\n"
                "        0\n"
                "    }\n"
                "}\n",
                [("ok", "s")],
                "transition_tcb",
            ),
        }
        for _name, (_body, _calls, _need) in _KT239_WB16.items():
            for _strict in (False, True):
                _kt239_case(
                    _name,
                    _body,
                    "green",
                    _strict,
                    calls=_calls,
                    need=_need,
                )
                # Ca **cặp**: cùng văn bản, chữ ký bị đảo ⇒ mỏ neo bắt.
                _kt239_case(
                    _name + "-anchor",
                    _body,
                    "anchor-type",
                    _strict,
                    calls=_calls,
                    need=_need,
                    revert_sig=True,
                    label=_name + " (chữ ký bị đảo)",
                )
        # f2 cần thêm tệp helper (không thì `crate::…` chỉ là văn bản, vẫn XANH nhưng
        # thiếu đúng cấu trúc journal). Tệp helper chứa lời gọi `transition_tcb(t, s)`.
        for _strict in (False, True):
            d = tmp / ("kt239-wb16-f2-helper-tep-khac-full-" + ("s" if _strict else "t"))
            _fixture_v2(d)
            _f2_site = (
                "impl M {\n"
                "    fn kt239_f2(&mut self, t: u32) -> u8 {\n"
                "        let _ = crate::kt239_f2_helper::helper(self, t, TcbState::Ready);\n"
                "        0\n"
                "    }\n"
                "}\n"
            )
            _f2_helper = (
                "fn helper(m: &mut M, t: u32, s: TcbState) -> u8 {\n"
                "    let _ = m.transition_tcb(t, s);\n"
                "    0\n"
                "}\n"
            )
            _write(d / _KT239_EVIL, _f2_site)
            _write(d / "v2/kernel/vifast-core/src/kt239_f2_helper.rs", _f2_helper)
            rep = scan_v2(d, strict=_strict, extra_doors=set())
            # Nguyên nhân XANH phải đúng: call site **không** có token `transition_tcb`
            # (nên luật transition không thấy), còn tệp helper thấy `("ok", "s")`.
            _why: List[str] = []
            if "TcbState::Ready" not in _f2_site:
                _why.append("call site thiếu `TcbState::Ready`")
            if "transition_tcb" in _f2_site:
                _why.append("call site lại chứa `transition_tcb` (ca không còn đúng dạng)")
            if [
                (k, dsc)
                for _, k, dsc, _s in _transition_calls(
                    _f2_helper, "v2/kernel/vifast-core/src/kt239_f2_helper.rs"
                )
            ] != [("ok", "s")]:
                _why.append("tệp helper không được phân loại `ok s`")
            cases.append(
                (
                    "KT-239 wb16-f2-helper-tep-khac (đủ 2 tệp) ⇒ xanh (kiểu đã loại trừ)"
                    + (" (--strict)" if _strict else " (thường)"),
                    rep.ok and not _why,
                    "; ".join(rep.errors + _why),
                )
            )

        # --- WB-17 (7 biến thể, journal kt232-fnptr.md §2) ------------------------------
        # f1/f2/f3/f5a/f5b/f7: `transition_tcb` xuất hiện nhưng **không** phải lời gọi
        # (`(` ngay sau định danh) ⇒ `_transition_calls` trả **rỗng**. Dưới API KT-241 dạng
        # fn-pointer **không biên dịch được** (`E0631`/`E0308`); checker XANH, ca `-anchor`
        # chứng minh mỏ neo bắt nếu chữ ký bị đảo.
        _KT239_WB17_ALIVE = {
            # f1 — `let tt = Machine::transition_tcb;` (dùng làm giá trị, không `(` sau
            # định danh) ⇒ `_transition_calls` bỏ qua.
            "wb17-f1-fn-pointer-tran": (
                "impl M {\n"
                "    fn kt239_f1(&mut self, t: u32) -> u8 {\n"
                "        let tt = Machine::transition_tcb;\n"
                "        let _ = tt;\n"
                "        0\n"
                "    }\n"
                "}\n"
            ),
            # f2 — fn-pointer có kiểu tường minh.
            "wb17-f2-fn-pointer-kieu": (
                "impl M {\n"
                "    fn kt239_f2(&mut self, t: u32) -> u8 {\n"
                "        let tt: fn(&mut M, u32, TcbState) -> u8 = Machine::transition_tcb;\n"
                "        let _ = tt;\n"
                "        0\n"
                "    }\n"
                "}\n"
            ),
            # f3 — truyền qua tham số hàm trợ giúp `fn call(f: fn(…), …)`.
            "wb17-f3-qua-tham-so-ham": (
                "fn call(f: fn(&mut M, u32, TcbState) -> u8, m: &mut M, t: u32) -> u8 {\n"
                "    let _ = f;\n"
                "    0\n"
                "}\n"
                "impl M {\n"
                "    fn kt239_f3(&mut self, t: u32) -> u8 {\n"
                "        call(Machine::transition_tcb, self, t)\n"
                "    }\n"
                "}\n"
            ),
            # f5a — generic `F: Fn(…)`.
            "wb17-f5a-generic": (
                "fn call_generic<F>(f: F, m: &mut M, t: u32) -> u8\n"
                "where\n"
                "    F: Fn(&mut M, u32, TcbState) -> u8,\n"
                "{\n"
                "    let _ = f;\n"
                "    0\n"
                "}\n"
                "impl M {\n"
                "    fn kt239_f5(&mut self, t: u32) -> u8 {\n"
                "        call_generic(Machine::transition_tcb, self, t)\n"
                "    }\n"
                "}\n"
            ),
            # f5b — trait object `&dyn Fn(…)`.
            "wb17-f5b-trait-object": (
                "impl M {\n"
                "    fn kt239_f5b(&mut self, t: u32) -> u8 {\n"
                "        let f: fn(&mut M, u32, TcbState) -> u8 = Machine::transition_tcb;\n"
                "        let g: &dyn Fn(&mut M, u32, TcbState) -> u8 = &f;\n"
                "        let _ = g;\n"
                "        0\n"
                "    }\n"
                "}\n"
            ),
            # f7 — `let f = <Machine>::transition_tcb;` (đường dẫn turbofish, không `(`).
            "wb17-f7-turbofish": (
                "impl M {\n"
                "    fn kt239_f7(&mut self, t: u32) -> u8 {\n"
                "        let f = <Machine>::transition_tcb;\n"
                "        let _ = f;\n"
                "        0\n"
                "    }\n"
                "}\n"
            ),
        }
        for _name, _body in _KT239_WB17_ALIVE.items():
            for _strict in (False, True):
                _kt239_case(_name, _body, "green", _strict, calls=[])
                _kt239_case(
                    _name + "-anchor",
                    _body,
                    "anchor-type",
                    _strict,
                    calls=[],
                    revert_sig=True,
                    label=_name + " (chữ ký bị đảo)",
                )
        # f6 — UFCS `M::transition_tcb(m, t, Ready)`: đọc args[2] ⇒ ĐỎ `E_STRICT_TRANSITION`
        # đúng nguyên nhân (đối số thứ ba là `TcbState::Ready` — đảo kiểu).
        # `calls=[("tcbstate", "TcbState::Ready")]` là **chốt chống hồi quy của bản vá KT-239**:
        # hoàn nguyên `segs_of(args[1])` ⇒ phân loại thành `("ok", "t")` ⇒ ca này Đỏ.
        _KT239_UFCS = (
            "use crate::Machine as M2;\n"
            "impl M {\n"
            "    fn kt239_f6(&mut self, t: u32) -> u8 {\n"
            "        let _ = M2::transition_tcb(self, t, TcbState::Ready);\n"
            "        0\n"
            "    }\n"
            "}\n"
        )
        for _strict in (False, True):
            _kt239_case(
                "wb17-f6-UFCS-doc-dung-args2",
                _KT239_UFCS,
                "red-transition",
                _strict,
                calls=[("tcbstate", "TcbState::Ready")],
                site=(4, "kt239_f6"),
            )

        # --- Đối chứng ÂM: thẳng `TcbState::Ready` ⇒ ĐỎ đúng mã (mỗi chế độ 1 ca) -----
        # Dùng **cùng đường dẫn** `evil.rs` như các ca "xanh": cùng một tệp, cùng một fixture
        # — Đỏ khi Ready là literal, Xanh khi rơi vào dạng đã ghi ⇒ chứng minh các ca Xanh kia
        # **không** xanh vì tệp bị bỏ qua ngoài phạm vi quét.
        _KT239_NEG = (
            "impl M {\n"
            "    fn am(&mut self, t: u32) -> u8 {\n"
            "        let _ = self.transition_tcb(t, TcbState::Ready);\n"
            "        0\n"
            "    }\n"
            "}\n"
        )
        for _strict in (False, True):
            _kt239_case(
                "doi-chung-am-thang-Ready",
                _KT239_NEG,
                "red-transition",
                _strict,
                calls=[("tcbstate", "TcbState::Ready")],
                site=(3, "am"),
            )

        # --- Đối chứng DƯƠNG: cửa thông thường + block_current + biến thể khác --------
        # Mỗi mục: (văn bản, tệp, phân loại `_transition_calls` bắt buộc).
        _KT239_POS = {
            # Cửa thông thường `transition_tcb` với `NonReadyTarget::*` ngoài `state_write.rs`
            # ⇒ XANH (đây là đường sản phẩm **duy nhất** còn lại: `block_current` truyền `to`).
            "duong-cua-thong-thuong": (
                "impl M {\n"
                "    fn block_current(&mut self, t: u32, to: NonReadyTarget) -> u8 {\n"
                "        let _ = self.transition_tcb(t, to);\n"
                "        let _ = self.transition_tcb(t, NonReadyTarget::Dead);\n"
                "        0\n"
                "    }\n"
                "}\n",
                "v2/kernel/vifast-core/src/ok.rs",
                [("ok", "to"), ("ok", "NonReadyTarget::Dead")],
            ),
            # block_current tham số `to: NonReadyTarget` ⇒ XANH (không báo oan).
            "duong-block-current-tham-so": (
                "impl M {\n"
                "    fn block_current(&mut self, t: u32, to: NonReadyTarget) -> u8 {\n"
                "        let _ = self.transition_tcb(t, to);\n"
                "        0\n"
                "    }\n"
                "}\n",
                "v2/kernel/vifast-core/src/ok.rs",
                [("ok", "to")],
            ),
            # `NonReadyTarget::<BiếnThể>` ⇒ XANH — gồm **cả** dạng UFCS
            # `M2::transition_tcb(self, t, NonReadyTarget::Dead)`: chứng minh nhánh UFCS đọc
            # args[2] mà **không** biến mọi lời gọi UFCS thành Đỏ (chống overfit).
            "duong-bien-the-khac": (
                "use crate::Machine as M2;\n"
                "impl M {\n"
                "    fn hop_le(&mut self, t: u32) -> u8 {\n"
                "        let _ = self.transition_tcb(t, NonReadyTarget::Stopping);\n"
                "        let _ = self.transition_tcb(t, NonReadyTarget::Sleeping { reason: 1 });\n"
                "        let _ = M2::transition_tcb(self, t, NonReadyTarget::Dead);\n"
                "        0\n"
                "    }\n"
                "}\n",
                "v2/kernel/vifast-core/src/ok.rs",
                [
                    ("ok", "NonReadyTarget::Stopping"),
                    ("ok", "NonReadyTarget::Sleeping"),
                    ("ok", "NonReadyTarget::Dead"),
                ],
            ),
        }
        for _name, (_body, _relp, _calls) in _KT239_POS.items():
            for _strict in (False, True):
                _kt239_case(
                    _name + "-" + ("s" if _strict else "t"),
                    _body,
                    "green",
                    _strict,
                    relp=_relp,
                    calls=_calls,
                    append=True,
                    label=_name,
                )

        # **KT-241 — tham số `tr` chết**: `_check_wake_text(relp, text, tr, safe_macros, r)`
        # nhận `tr = test_line_ranges(text)` nhưng **bỏ qua** và tự tính
        # `_test_char_ranges(code, relp)`. Hai cách phân vùng ⇒ người đọc tưởng caller điều
        # khiển được vùng test. Nay **một** nơi sở hữu: bỏ tham số `tr` khỏi chữ ký và khỏi
        # **cả** call site (lớp 1 + lớp 2). Ba ca dưới chứng minh:
        #   (a) hai cách phân vùng **thật sự khác nhau** trên cùng văn bản;
        #   (b) kết quả theo cách của **callee** (`_test_char_ranges`) — ĐỎ;
        #   (c) chữ ký không còn `tr` và không call site nào truyền `test_line_ranges`.
        _PART = (
            "#[cfg(test)] mod tests { fn h() {} }\n"
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n"
        )
        _wake_line = 2
        _line_covers = in_ranges(_wake_line, test_line_ranges(_PART))
        _char_covers = in_ranges(
            _PART.index("m.wake"), _test_char_ranges(strip_rust_noise(_PART), "x.rs")
        )
        d = tmp / "kt241-partition"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/evil.rs", _PART)
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "KT-241 phân vùng: hai cách khác nhau, kết quả theo callee (ĐỎ)",
                _line_covers
                and not _char_covers
                and (not rep.ok)
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                f"line_covers={_line_covers} char_covers={_char_covers}; "
                + "; ".join(rep.errors),
            )
        )
        _sig_params = list(inspect.signature(_check_wake_text).parameters)
        cases.append(
            (
                "KT-241 `tr`: `_check_wake_text` không còn tham số `tr` (4 tham số)",
                _sig_params == ["relp", "text", "safe_macros", "r"],
                str(_sig_params),
            )
        )
        _here = _TOOLS_DIR
        _calls: List[str] = []
        _bad_calls: List[str] = []
        for _fp in _SELFTEST_SCAN_FILES:
            _f = _fp.name
            if not _fp.is_file():
                continue
            for _ln in _fp.read_text(encoding="utf-8").splitlines():
                _st = _ln.strip()
                if "_check_wake_text(" not in _ln or "def _check_wake_text" in _ln:
                    continue
                if _st.startswith("#"):  # chú thích, không phải call site
                    continue
                _calls.append(f"{_f}: {_st}")
                if "test_line_ranges" in _ln:
                    _bad_calls.append(f"{_f}: {_st}")
        cases.append(
            (
                # **KT-258** — thêm **một** call site ở lớp 1 (tập đích ngoài `V2_SCAN`,
                # WB-21): 5 = lớp 1 ×4 + lớp 2 ×1. Không phải nới luật — cùng một hàm luật,
                # thêm một tập đích; ca vẫn khẳng định **không** call site nào truyền
                # `test_line_ranges`.
                "KT-241 `tr`: 5 call site (lớp 1 ×4 + lớp 2 ×1), không truyền `test_line_ranges`",
                len(_calls) == 5 and not _bad_calls,
                f"{len(_calls)} call site; sai: {_bad_calls}",
            )
        )

        # ======================================================================
        # **KT-242** — khép bốn lệch hợp đồng ảnh hưởng luật `wake` (review độc lập
        # `JOURNAL/kt242-wake-review.md`). Mỗi mục có **đối chứng dương** để chứng minh
        # bản vá là **siết**, không phải "luôn đỏ".
        # ======================================================================

        # --- N1/N2: `write_state` quy cho **hàm trong cùng** (token-tree) -------------
        _N1_IMPL = (
            "    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {\n"
            "        impl M {\n"
            "            pub fn kt242_door(&mut self, tcb: u32) -> u8 {\n"
            "                self.write_state(tcb, TcbState::Ready)\n"
            "            }\n"
            "        }\n"
            "        self.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
        )
        _N1_FN = (
            "    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {\n"
            "        fn kt242_door(m: &mut M, tcb: u32) -> u8 {\n"
            "            m.write_state(tcb, TcbState::Ready)\n"
            "        }\n"
            "        let _ = kt242_door;\n"
            "        self.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
        )
        _N1_OLD = (
            "    pub(crate) fn wake_step(&mut self, tcb: u32) -> u8 {\n"
            "        self.write_state(tcb, TcbState::Ready)\n"
            "    }\n"
        )
        for _n1_name, _n1_new in (("impl", _N1_IMPL), ("fn", _N1_FN)):
            d = tmp / ("kt242-n1-" + _n1_name)
            _fixture_v2(d)
            _swp = d / KERNEL_STATE_WRITE
            _swp.write_text(
                _swp.read_text(encoding="utf-8").replace(_N1_OLD, _n1_new, 1),
                encoding="utf-8",
            )
            for _strict in (False, True):
                rep = scan_v2(d, strict=_strict, extra_doors=set())
                cases.append(
                    (
                        f"KT-242 N1/N2 `{_n1_name}` lồng trong cửa gọi `write_state` ⇒ đỏ "
                        f"E_STRICT_STATE_WRITE"
                        + (" (--strict)" if _strict else " (thường)"),
                        (not rep.ok)
                        and any("E_STRICT_STATE_WRITE" in e for e in rep.errors)
                        and any("kt242_door" in e for e in rep.errors),
                        "; ".join(rep.errors),
                    )
                )

        # Đối chứng dương: **6 cửa thật** vẫn được nhận **đúng cửa** (bản vá không làm đỏ
        # cửa thật, và không quy nhầm sang hàm khác).
        d = tmp / "kt242-n1-doors"
        _fixture_v2(d)
        _sw_text = (d / KERNEL_STATE_WRITE).read_text(encoding="utf-8")
        _r = Report()
        _used = _check_state_write(
            KERNEL_STATE_WRITE,
            _sw_text,
            _test_char_ranges(strip_rust_noise(_sw_text), KERNEL_STATE_WRITE),
            None,
            _r,
        )
        _expected = {(KERNEL_STATE_WRITE, _n) for _n in STATE_WRITE_CALLERS}
        cases.append(
            (
                "KT-242 N1 đối chứng dương: 6 cửa thật vẫn được nhận đúng cửa",
                _r.ok and _used == _expected,
                f"used={sorted(_used)}; errors={_r.errors}",
            )
        )

        # --- N5: vùng test theo **ký tự**, không theo dòng ---------------------------
        # Cùng-dòng: `#[cfg(test)]` trên một mục **có thân** rồi mã sản phẩm **cùng dòng**.
        # `test_line_ranges` cũ phủ **cả** dòng ⇒ miễn; `_test_char_ranges` chỉ phủ mục test.
        _N5_PART = "#[cfg(test)]\nfn helper() {} "
        _N5_CASES = (
            (
                "luật 1 (post_mail MailboxMsg::Wake)",
                "v2/kernel/vifast-core/src/sub/mailbox.rs",
                _N5_PART + "fn evil(&mut self) { self.post_mail(0, MailboxMsg::Wake(1)); }\n",
                "E_CHANNEL_PRODUCER",
            ),
            (
                "luật 2 (consumer MailboxMsg::Wake)",
                "v2/kernel/vifast-core/src/evil2.rs",
                _N5_PART
                + "fn evil(msg: MailboxMsg) { match msg { MailboxMsg::Wake(t) => "
                + "{ let _ = t; } _ => {} } }\n",
                "E_CONSUMER",
            ),
            (
                "luật 3 (arch IpiKind::Wake)",
                "v2/kernel/vifast-plat-riscv64/src/ipi.rs",
                _N5_PART
                + "fn post_wake(hart: usize) { IPI_MAILBOXES.get(hart).post(IpiKind::Wake); }\n",
                "E_ARCH_DOORBELL",
            ),
        )
        for _n5_name, _n5_relp, _n5_text, _n5_code in _N5_CASES:
            d = tmp / ("kt242-n5-" + _n5_code.lower())
            _fixture_v2(d)
            _write(d / _n5_relp, _n5_text)
            for _strict in (False, True):
                rep = scan_v2(d, strict=_strict, extra_doors=set())
                cases.append(
                    (
                        f"KT-242 N5 {_n5_name} cùng dòng `#[cfg(test)]` ⇒ đỏ {_n5_code}"
                        + (" (--strict)" if _strict else " (thường)"),
                        (not rep.ok) and any(_n5_code in e for e in rep.errors),
                        "; ".join(rep.errors),
                    )
                )

        # Đối chứng dương: `#[cfg(test)] mod t { … }` **vẫn** được miễn (không báo oan).
        d = tmp / "kt242-n5-pos"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil3.rs",
            "#[cfg(test)]\nmod t { fn evil(m: &mut M, t: u32) { let _ = m.wake(t); } }\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "KT-242 N5 đối chứng dương: `#[cfg(test)] mod t { … }` vẫn XANH",
                rep.ok,
                "; ".join(rep.errors),
            )
        )
        # Đối chứng dương: R1 (luật token `wake`) **vẫn** bắt mã cùng dòng (không hồi quy).
        d = tmp / "kt242-n5-r1"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/evil4.rs",
            _N5_PART + "pub fn evil(m: &mut M, t: u32) { let _ = m.wake(t); }\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "KT-242 N5 đối chứng dương: R1 cùng dòng `#[cfg(test)]` vẫn ĐỎ",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                "; ".join(rep.errors),
            )
        )

        # --- N3: `[[bin]] path` **ngoài `src/`** ------------------------------------
        d = tmp / "kt242-n3-bin"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/Cargo.toml",
            '[package]\nname = "vifast-core"\n\n[[bin]]\nname = "kt242-bin"\n'
            'path = "bin/kt242_bin.rs"\n',
        )
        _write(
            d / "v2/kernel/vifast-core/bin/kt242_bin.rs",
            "fn main() { let mut m = M; let _ = m.wake(0); }\n",
        )
        for _strict in (False, True):
            rep = scan_v2(d, strict=_strict, extra_doors=set())
            cases.append(
                (
                    "KT-242 N3 `[[bin]] path` ngoài `src/` chứa `wake` ⇒ đỏ "
                    "E_STRICT_TEST_ONLY_WAKE"
                    + (" (--strict)" if _strict else " (thường)"),
                    (not rep.ok)
                    and any(
                        "E_STRICT_TEST_ONLY_WAKE" in e and "bin/kt242_bin.rs" in e
                        for e in rep.errors
                    ),
                    "; ".join(rep.errors),
                )
            )
        # Đối chứng dương: `tests/*.rs` cấp crate **vẫn** được miễn (test-only).
        d = tmp / "kt242-n3-tests"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/Cargo.toml",
            '[package]\nname = "vifast-core"\n',
        )
        _write(
            d / "v2/kernel/vifast-core/tests/kt242_ok.rs",
            "fn t(m: &mut M) { let _ = m.wake(0); }\n",
        )
        rep = scan_v2(d, strict=True, extra_doors=set())
        cases.append(
            (
                "KT-242 N3 đối chứng dương: `tests/*.rs` cấp crate vẫn XANH",
                rep.ok,
                "; ".join(rep.errors),
            )
        )

        # --- N4: tệp `.RS` (hoa) — FS không phân biệt hoa/thường ---------------------
        d = tmp / "kt242-n4-rs"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-core/src/kt242_rs.RS",
            "pub fn evil(m: &mut M, t: u32) { let _ = m.wake(t); }\n",
        )
        for _strict in (False, True):
            rep = scan_v2(d, strict=_strict, extra_doors=set())
            cases.append(
                (
                    "KT-242 N4 tệp `.RS` (hoa) chứa `wake` ⇒ đỏ E_STRICT_TEST_ONLY_WAKE"
                    + (" (--strict)" if _strict else " (thường)"),
                    (not rep.ok)
                    and any(
                        "E_STRICT_TEST_ONLY_WAKE" in e and "kt242_rs.RS" in e
                        for e in rep.errors
                    ),
                    "; ".join(rep.errors),
                )
            )

        # --- Chốt chống tái diễn: **không** còn thành phần quyết định theo DÒNG ---------
        # (a) `test_line_ranges` **không** còn call site sản xuất nào (chỉ còn trong chính
        #     `selftest` để chứng minh hai cách phân vùng khác nhau).
        _here2 = _TOOLS_DIR
        _tlr_bad: List[str] = []
        for _fp in _SELFTEST_SCAN_FILES:
            _f = _fp.name
            if not _fp.is_file():
                continue
            for _ln in _fp.read_text(encoding="utf-8").splitlines():
                _st = _ln.strip()
                if "test_line_ranges(" not in _ln or "def test_line_ranges" in _ln:
                    continue
                if _st.startswith("#"):
                    continue
                # Bản thân `selftest` được phép (ca chứng minh hai cách khác nhau).
                if _f == "core.py" and "_line_covers" in _ln:
                    continue
                # Dòng **docstring** (nhắc lịch sử trong dấu backtick) không phải call site.
                if "`" in _ln:
                    continue
                _tlr_bad.append(f"{_f}: {_st}")
        cases.append(
            (
                "KT-242 N5 chốt: `test_line_ranges` không còn call site sản xuất",
                not _tlr_bad,
                f"call site: {_tlr_bad}",
            )
        )
        # (b) `_check_state_write` **không** còn tham số `fns` (nguồn quy sai hàm).
        _sw_params = list(inspect.signature(_check_state_write).parameters)
        cases.append(
            (
                "KT-242 N1 chốt: `_check_state_write` không còn tham số `fns`",
                _sw_params == ["relp", "text", "tr", "extra_doors", "r"],
                str(_sw_params),
            )
        )
        # (c) `in_test_only_wake_scope` **có** tham số `root` (phạm vi = mẫu ∪ crate root).
        # **KT-256** — thêm `scope` (tầng gọi truyền phạm vi đã dựng); `root` vẫn phải có.
        _scope_params = list(inspect.signature(in_test_only_wake_scope).parameters)
        cases.append(
            (
                "KT-242 N3 chốt: `in_test_only_wake_scope` nhận `root` (crate root thật)",
                "root" in _scope_params and _scope_params[:2] == ["relp", "root"],
                str(_scope_params),
            )
        )

        # --- N6: đường dẫn không phân giải được ⇒ lỗi cổng RÕ, không traceback --------
        d = tmp / "kt242-n6-nul"
        _fixture_v2(d)
        _write(d / "v2/kernel/vifast-core/src/evil5.rs", 'include!("\\0");\n')
        try:
            rep = scan_v2(d, strict=False)
            _n6_ok = (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors)
            _n6_detail = "; ".join(rep.errors)
        except Exception as _e:  # noqa: BLE001 - chính là điều ca này cấm
            _n6_ok = False
            _n6_detail = f"CRASH {type(_e).__name__}: {_e}"
        cases.append(
            (
                "KT-242 N6 `include!(\"\\0\")` ⇒ đỏ E_STRICT_WAKE_INCLUDE (không crash)",
                _n6_ok,
                _n6_detail,
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

        # ================================================================ KT-256 (L2, D2-188)
        # **Một** định nghĩa phạm vi sinh từ `cargo metadata` (`v2/tools/wake_scope.py`) dùng cho
        # **cả hai** checker. Các ca dưới là **đột biến hai chiều**: chứng minh (i) **ba mẫu cũ**
        # không khớp tệp đó, (ii) định nghĩa mới khớp, (iii) checker **ĐỎ** đúng mã lỗi. Thêm ba
        # ca cho **lưới an toàn** (`E_SCOPE_REDUCTION`, `E_SCOPE_LIB_CORE`, `E_SCOPE_METADATA`) để
        # chúng không phải "luật luôn xanh".
        _OLD_TOKEN_PAT = re.compile(r"^v2/(?:kernel|sim)/[^/]+/(?:src/.*\.rs|build\.rs)$")
        _OLD_MACRO_ROOTS = ("v2/lib", "v2/kernel", "v2/sim")

        def _scope256(sroot, pkgs, devdeps=()):
            """Blob `cargo metadata` giả (không chạy cargo) cho các ca KT-256."""
            packages = []
            for name, d, deps in pkgs:
                dep_objs = [
                    {"name": dep.split("/")[-1], "path": str(sroot / dep), "kind": None}
                    for dep in deps
                ]
                for owner, dep in devdeps:
                    if owner == name:
                        dep_objs.append(
                            {
                                "name": dep.split("/")[-1],
                                "path": str(sroot / dep),
                                "kind": "dev",
                            }
                        )
                packages.append(
                    {
                        "name": name,
                        "manifest_path": str(sroot / d / "Cargo.toml"),
                        "targets": [
                            {
                                "name": name.replace("-", "_"),
                                "kind": ["lib"],
                                "src_path": str(sroot / d / "src/lib.rs"),
                            }
                        ],
                        "dependencies": dep_objs,
                    }
                )
            return wake_scope.scope_from_metadata(
                sroot,
                [
                    (
                        "host",
                        {
                            "target_directory": str(sroot / "v2/target"),
                            "packages": packages,
                        },
                    )
                ],
            )

        # Bộ crate tối thiểu của `_fixture_v2` (để `check_no_reduction` không báo oan).
        _FIX_PKGS = [
            ("vifast-sync", "v2/lib/vifast-sync", ()),
            ("vifast-core", "v2/kernel/vifast-core", ()),
            ("vifast-sim", "v2/sim/vifast-sim", ()),
        ]

        # --- T1: tệp sản phẩm NGOÀI `src/` của crate ảnh sản phẩm (chương trình U-mode) ------
        d = tmp / "kt256-t1-init"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-kbin-riscv64/init/evil.rs",
            "pub fn f(m: &mut M, t: u32) { m.wake(t); }\n",
        )
        relp = "v2/kernel/vifast-kbin-riscv64/init/evil.rs"
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-256/L2 T1: `wake` trong `init/` (sản phẩm, ngoài `src/`) ⇒ ĐỎ; mẫu cũ KHÔNG khớp",
                (not rep.ok)
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors)
                and _OLD_TOKEN_PAT.match(relp) is None,
                f"errors={rep.errors}",
            )
        )

        # --- T2: shim `vifast-plat-user` (cây KHÔNG phải crate, đóng vào ảnh) ----------------
        d = tmp / "kt256-t2-platuser"
        _fixture_v2(d)
        _write(
            d / "v2/user/runtime/lib.rs",
            "pub fn f(m: &mut M, t: u32) { m.wake(t); }\n",
        )
        relp = "v2/user/runtime/lib.rs"
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-256/L2 T2: `wake` trong shim `vifast-plat-user/lib.rs` ⇒ ĐỎ; mẫu cũ KHÔNG khớp",
                (not rep.ok)
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors)
                and _OLD_TOKEN_PAT.match(relp) is None,
                f"errors={rep.errors}",
            )
        )

        # --- T3: crate ở thư mục gốc MỚI (`v2/drivers/…`) — thân macro chứa `wake` -----------
        d = tmp / "kt256-t3-drivers"
        _fixture_v2(d)
        _write(
            d / "v2/drivers/vifast-x/src/lib.rs",
            "macro_rules! door { () => { m.wake(t) } }\n",
        )
        relp = "v2/drivers/vifast-x/src/lib.rs"
        sc = _scope256(d, _FIX_PKGS + [("vifast-x", "v2/drivers/vifast-x", ())])
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-256/L2 T3: crate ở gốc MỚI `v2/drivers/…` — thân macro `wake` ⇒ ĐỎ; "
                "`MACRO_SCAN_ROOTS` cũ mù",
                (not rep.ok)
                and any("E_STRICT_MACRO_WAKE" in e for e in rep.errors)
                and not relp.startswith(tuple(r + "/" for r in _OLD_MACRO_ROOTS)),
                f"errors={rep.errors}",
            )
        )

        # --- T4: `v2/tools/<crate>` phụ thuộc crate bảo vệ ⇒ NÂNG ⇒ macro scan bắt -----------
        d = tmp / "kt256-t4-tool"
        _fixture_v2(d)
        _write(
            d / "v2/tools/tool/src/lib.rs",
            "macro_rules! door { () => { m.wake(t) } }\n",
        )
        relp = "v2/tools/tool/src/lib.rs"
        sc = _scope256(d, _FIX_PKGS + [("tool", "v2/tools/tool", ("v2/kernel/vifast-core",))])
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-256/L2 T4: `v2/tools` phụ thuộc crate bảo vệ ⇒ nâng ⇒ thân macro `wake` ĐỎ",
                (not rep.ok)
                and any("E_STRICT_MACRO_WAKE" in e for e in rep.errors)
                and sc.by_name["tool"].status == "protected"
                and "nâng" in sc.by_name["tool"].reason
                and not relp.startswith(tuple(r + "/" for r in _OLD_MACRO_ROOTS)),
                f"errors={rep.errors} tool={sc.by_name['tool'].status}",
            )
        )

        # --- T5: lưới "không giảm phạm vi" — metadata thiếu crate có thật trên đĩa -----------
        d = tmp / "kt256-t5-reduction"
        _fixture_v2(d)
        sc = _scope256(
            d,
            [("vifast-sync", "v2/lib/vifast-sync", ()), ("vifast-sim", "v2/sim/vifast-sim", ())],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-256/L2 T5: metadata bỏ sót crate có thật ⇒ `E_SCOPE_REDUCTION` (không giảm phạm vi)",
                (not rep.ok) and any("E_SCOPE_REDUCTION" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- T6: DEBT-KT227-1 — `v2/lib` phụ thuộc `vifast-core` (normal) ⇒ ĐỎ ---------------
        d = tmp / "kt256-t6-libcore"
        _fixture_v2(d)
        sc = _scope256(
            d,
            _FIX_PKGS + [("vifast-model", "v2/lib/vifast-model", ("v2/kernel/vifast-core",))],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-256/DEBT-KT227-1: crate `v2/lib` phụ thuộc `vifast-core` (normal) ⇒ "
                "`E_SCOPE_LIB_CORE` (buộc mở rộng luật token)",
                (not rep.ok) and any("E_SCOPE_LIB_CORE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # Đối chứng dương: **dev**-dependency thì KHÔNG báo (dev không làm `wake()` gọi được từ
        # mã sản phẩm) — thiếu vế này thì một luật "luôn đỏ" cũng qua được ca trên.
        sc = _scope256(
            d,
            _FIX_PKGS + [("vifast-model", "v2/lib/vifast-model", ())],
            devdeps=[("vifast-model", "v2/kernel/vifast-core")],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-256/DEBT-KT227-1 đối chứng dương: `v2/lib` chỉ **dev**-dep `vifast-core` ⇒ XANH",
                rep.ok,
                f"errors={rep.errors}",
            )
        )

        # --- T7: không dựng được metadata trên cây CÓ manifest ⇒ ĐỎ (fail-closed) ------------
        d = tmp / "kt256-t7-meta"
        _fixture_v2(d)
        sc = wake_scope.ProtectedScope()
        sc.error = "cargo metadata trả 0 crate (giả lập)"
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-256/L2 T7: không dựng được phạm vi từ metadata ⇒ `E_SCOPE_METADATA` (fail-closed)",
                (not rep.ok) and any("E_SCOPE_METADATA" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- T8: chính sách `v2/lib` ngoài luật token (DEBT-KT227-1) + phạm vi dự phòng ------
        d = tmp / "kt256-t8-policy"
        _fixture_v2(d)
        cases.append(
            (
                "KT-256/DEBT-KT227-1: `v2/lib` NGOÀI luật token; `v2/kernel`+`v2/sim` TRONG "
                "(phạm vi dự phòng của cây fixture)",
                in_test_only_wake_scope("v2/kernel/vifast-core/src/lib.rs", d)
                and in_test_only_wake_scope("v2/sim/vifast-sim/src/kobj/sched.rs", d)
                # KT-336b: phạm vi token tính theo **crate**, nên tệp miền mới (không chỉ
                # `kobj.rs`) cũng phải thuộc phạm vi bảo vệ.
                and not in_test_only_wake_scope("v2/lib/vifast-abi/src/lib.rs", d),
                "phạm vi token sai chính sách",
            )
        )

        # ================================================================ KT-258 (WB-21/WB-22)
        # **Phạm vi mới của KT-256** (sinh từ `cargo metadata`) rộng hơn **vòng lặp** của lớp
        # tĩnh (`V2_SCAN` = `v2/kernel` + `v2/sim`): crate trong **bao đóng sản phẩm** ở gốc mới
        # (`v2/drivers/…`) là `contains`=True nhưng `token_scope`=False (workspace="closure") và
        # không được `iter_rs` duyệt ⇒ luật token **không bao giờ** chạy trên nó. Đo được đường
        # lọt **cả ba** lượt kiểm khi crate đó là path-dependency **tuỳ chọn** của
        # `vifast-kbin-riscv64` (chỉ bật bởi `product-fs`): `JOURNAL/kt258-attack/`.
        # Các ca dưới là **đột biến hai chiều**: trước vá XANH, sau vá ĐỎ.

        def _scope258(sroot, host_pkgs, kernel_pkgs, closure_pkgs=()):
            """Blob `cargo metadata` giả có **đơn vị `closure`** (path-dep ngoài workspace)."""

            def _unit(name, pkgs):
                packages = []
                for pname, d, deps in pkgs:
                    dep_objs = [
                        {"name": dep.split("/")[-1], "path": str(sroot / dep), "kind": None}
                        for dep in deps
                    ]
                    packages.append(
                        {
                            "name": pname,
                            "manifest_path": str(sroot / d / "Cargo.toml"),
                            "targets": [
                                {
                                    "name": pname.replace("-", "_"),
                                    "kind": ["lib"],
                                    "src_path": str(sroot / d / "src/lib.rs"),
                                }
                            ],
                            "dependencies": dep_objs,
                        }
                    )
                return (
                    name,
                    {"target_directory": str(sroot / "v2/target"), "packages": packages},
                )

            units = [_unit("host", host_pkgs), _unit("kernel", kernel_pkgs)]
            if closure_pkgs:
                units.append(_unit("closure", closure_pkgs))
            return wake_scope.scope_from_metadata(sroot, units)

        _HOST_PKGS = [
            ("vifast-sync", "v2/lib/vifast-sync", ()),
            ("vifast-sim", "v2/sim/vifast-sim", ()),
        ]
        _KERNEL_PKGS = [("vifast-core", "v2/kernel/vifast-core", ("v2/drivers/evil",))]
        _CLOSURE_PKGS = [("vifast-evil", "v2/drivers/evil", ())]

        # --- U1: crate trong bao đóng sản phẩm ở GỐC MỚI gọi `wake` ⇒ ĐỎ ---------------------
        d = tmp / "kt258-u1-closure"
        _fixture_v2(d)
        _write(
            d / "v2/drivers/evil/src/lib.rs",
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
        )
        relp = "v2/drivers/evil/src/lib.rs"
        sc = _scope258(d, _HOST_PKGS, _KERNEL_PKGS, _CLOSURE_PKGS)
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-21 U1: `wake` trong crate ở GỐC MỚI thuộc **bao đóng sản phẩm** ⇒ ĐỎ "
                "(trước vá: XANH — `token_scope`=False + ngoài `V2_SCAN`)",
                (not rep.ok)
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors)
                and sc.token_scope(relp)
                and _OLD_TOKEN_PAT.match(relp) is None,
                f"errors={rep.errors}",
            )
        )
        rep = scan_v2(d, strict=True, scope=sc, extra_doors=set())
        cases.append(
            (
                "KT-258/WB-21 U1s: như U1 ở `--strict` ⇒ ĐỎ (cùng tập đích, hai chế độ)",
                (not rep.ok) and any("E_STRICT_TEST_ONLY_WAKE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- U2: đối chứng dương — cùng crate, KHÔNG có `wake` ⇒ XANH ------------------------
        d = tmp / "kt258-u2-clean"
        _fixture_v2(d)
        _write(
            d / "v2/drivers/evil/src/lib.rs",
            "pub fn evil(m: &mut M, t: u32) { let _ = (m, t); }\n",
        )
        sc = _scope258(d, _HOST_PKGS, _KERNEL_PKGS, _CLOSURE_PKGS)
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-21 U2 đối chứng dương: crate bao đóng sản phẩm KHÔNG có `wake` ⇒ XANH",
                rep.ok,
                f"errors={rep.errors}",
            )
        )

        # --- U3: crate bao đóng **không** thuộc sản phẩm/oracle ⇒ ngoài luật token (XANH) ----
        d = tmp / "kt258-u3-hostonly"
        _fixture_v2(d)
        _write(
            d / "v2/foo/evil/src/lib.rs",
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
        )
        sc = _scope258(
            d,
            _HOST_PKGS + [("vifast-model", "v2/lib/vifast-model", ("v2/foo/evil",))],
            _KERNEL_PKGS,
            [("vifast-evil", "v2/foo/evil", ())],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-21 U3: crate bao đóng **không** thuộc sản phẩm/oracle ⇒ ngoài luật "
                "token (XANH) — phạm vi không mở rộng mù",
                rep.ok,
                f"errors={rep.errors}",
            )
        )

        # --- U4: `v2/lib` trong bao đóng sản phẩm — chính sách KT-256 giữ ngoài luật token ----
        d = tmp / "kt258-u4-libclosure"
        _fixture_v2(d)
        _write(
            d / "v2/lib/vifast-x/src/lib.rs",
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
        )
        sc = _scope258(
            d,
            _HOST_PKGS,
            [("vifast-core", "v2/kernel/vifast-core", ("v2/lib/vifast-x",))],
            [("vifast-x", "v2/lib/vifast-x", ())],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-21 U4: `v2/lib` trong bao đóng sản phẩm vẫn NGOÀI luật token "
                "(chính sách KT-256; canh bằng `E_SCOPE_LIB_CORE`)",
                rep.ok and not sc.token_scope("v2/lib/vifast-x/src/lib.rs"),
                f"errors={rep.errors}",
            )
        )
        sc = _scope258(
            d,
            _HOST_PKGS,
            [("vifast-core", "v2/kernel/vifast-core", ("v2/lib/vifast-x",))],
            [("vifast-x", "v2/lib/vifast-x", ("v2/kernel/vifast-core",))],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-21 U4b: `v2/lib` trong bao đóng sản phẩm mà phụ thuộc `vifast-core` "
                "⇒ `E_SCOPE_LIB_CORE` (ca canh có răng)",
                (not rep.ok) and any("E_SCOPE_LIB_CORE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- U5: WB-22 — `include!` trong crate bao đóng sản phẩm trỏ RA NGOÀI cây ⇒ ĐỎ ------
        d = tmp / "kt258-u5-include"
        _fixture_v2(d)
        _write(
            d / "v2/drivers/evil/src/lib.rs",
            'include!("../../../agent/evil_src.rs");\n',
        )
        _write(
            d / "v2/agent/evil_src.rs",
            "pub fn evil(m: &mut M, t: u32) { m.wake(t); }\n",
        )
        sc = _scope258(d, _HOST_PKGS, _KERNEL_PKGS, _CLOSURE_PKGS)
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-22 U5: `include!` trong crate bao đóng sản phẩm trỏ ra ngoài "
                "`v2/kernel`+`v2/sim` ⇒ ĐỎ `E_STRICT_WAKE_INCLUDE` (trước vá: XANH)",
                (not rep.ok) and any("E_STRICT_WAKE_INCLUDE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # Đối chứng dương: cùng crate, `include!` trỏ tới tệp **trong** `v2/kernel` ⇒ XANH.
        d = tmp / "kt258-u5b-include-ok"
        _fixture_v2(d)
        _write(
            d / "v2/drivers/evil/src/lib.rs",
            'include!("../../../kernel/vifast-core/src/kt258_ok.rs");\n',
        )
        _write(
            d / "v2/kernel/vifast-core/src/kt258_ok.rs",
            "pub fn ok(m: &mut M, t: u32) { let _ = (m, t); }\n",
        )
        sc = _scope258(d, _HOST_PKGS, _KERNEL_PKGS, _CLOSURE_PKGS)
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-22 U5b đối chứng dương: `include!` trong crate bao đóng trỏ tới tệp "
                "TRONG `v2/kernel` ⇒ XANH (luật không phải luôn-đỏ)",
                rep.ok,
                f"errors={rep.errors}",
            )
        )

        # --- U6: WB-23 — path-dependency ra NGOÀI gốc repo ⇒ từ chối rõ ---------------------
        d = tmp / "kt258-u6-outside"
        _fixture_v2(d)
        sc = _scope258(
            d,
            _HOST_PKGS,
            [("vifast-core", "v2/kernel/vifast-core", ("/tmp/kt258-outside-crate",))],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-23 U6: path-dependency ra NGOÀI gốc repo ⇒ `E_SCOPE_OUTSIDE` "
                "(từ chối rõ, không bỏ im lặng khỏi phạm vi)",
                (not rep.ok) and any("E_SCOPE_OUTSIDE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # Đối chứng dương: path-dependency **trong** repo ⇒ không có `E_SCOPE_OUTSIDE`.
        d = tmp / "kt258-u6b-inside"
        _fixture_v2(d)
        _write(d / "v2/foo/evil/src/lib.rs", "pub fn ok() {}\n")
        sc = _scope258(
            d,
            _HOST_PKGS,
            [("vifast-core", "v2/kernel/vifast-core", ("v2/foo/evil",))],
            [("vifast-evil", "v2/foo/evil", ())],
        )
        rep = scan_v2(d, strict=False, scope=sc)
        cases.append(
            (
                "KT-258/WB-23 U6b đối chứng dương: path-dependency TRONG repo ⇒ không "
                "`E_SCOPE_OUTSIDE` (luật không phải luôn-đỏ)",
                not any("E_SCOPE_OUTSIDE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # ================================================================ KT-264 (nợ §7.3)
        # **Luật cấu trúc theo phạm vi**: trước KT-264 `scan_structure` chỉ chạy `V2_SCAN` và
        # `_mods_of` **bỏ im lặng** cạnh `mod` không phân giải được ⇒ crate ở **gốc mới** (và
        # `v2/lib`) có `mod` graph lạ / tệp mồ côi mà **không** luật nào thấy. Các ca dưới là
        # **đột biến hai chiều**: trước vá XANH, sau vá ĐỎ; mỗi ca có **đối chứng dương** để
        # chứng minh luật không phải luôn-đỏ.
        _EVIL = "v2/drivers/evil"
        _EVIL_MANIFEST = '[package]\nname = "vifast-evil"\nversion = "0.1.0"\n'
        _NET = "v2/lib/vifast-net"
        _NET_MANIFEST = '[package]\nname = "vifast-net"\nversion = "0.1.0"\n'
        _HOST_NET = _HOST_PKGS + [("vifast-net", _NET, ())]

        def _evil_scope(sroot):
            return _scope258(sroot, _HOST_PKGS, _KERNEL_PKGS, _CLOSURE_PKGS)

        # --- V1: tệp MỒ CÔI trong crate ở gốc mới ⇒ `E_STRUCT_ORPHAN` ----------------------
        d = tmp / "kt264-v1-orphan"
        _fixture_v2(d)
        _write(d / _EVIL / "Cargo.toml", _EVIL_MANIFEST)
        _write(d / _EVIL / "src/lib.rs", "pub fn ok() -> u8 { 0 }\n")
        _write(d / _EVIL / "src/orphan.rs", "pub fn not_compiled() -> u8 { 1 }\n")
        rep = scan_v2(d, strict=False, scope=_evil_scope(d))
        cases.append(
            (
                "KT-264 V1: tệp `.rs` **mồ côi** trong crate ở GỐC MỚI ⇒ `E_STRUCT_ORPHAN` "
                "(trước vá: XANH — `scan_structure` chỉ chạy `V2_SCAN`)",
                (not rep.ok)
                and any("E_STRUCT_ORPHAN" in e and "orphan.rs" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # --- V2: đối chứng dương — mọi tệp trong `mod` graph ⇒ không `E_STRUCT_ORPHAN` ------
        d = tmp / "kt264-v2-clean"
        _fixture_v2(d)
        _write(d / _EVIL / "Cargo.toml", _EVIL_MANIFEST)
        _write(d / _EVIL / "src/lib.rs", "mod inner;\npub fn ok() -> u8 { 0 }\n")
        _write(d / _EVIL / "src/inner.rs", "pub fn f() -> u8 { 1 }\n")
        rep = scan_v2(d, strict=False, scope=_evil_scope(d))
        cases.append(
            (
                "KT-264 V2 đối chứng dương: mọi tệp trong `mod` graph ⇒ không "
                "`E_STRUCT_ORPHAN` (luật không phải luôn-đỏ)",
                not any("E_STRUCT_ORPHAN" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- V3: cạnh `mod` KHÔNG phân giải được ⇒ `E_STRUCT_MOD_MISSING` ------------------
        d = tmp / "kt264-v3-missing"
        _fixture_v2(d)
        _write(d / _EVIL / "Cargo.toml", _EVIL_MANIFEST)
        _write(
            d / _EVIL / "src/lib.rs",
            "#[cfg(any())]\nmod ghost;\npub fn ok() -> u8 { 0 }\n",
        )
        rep = scan_v2(d, strict=False, scope=_evil_scope(d))
        cases.append(
            (
                "KT-264 V3: `mod ghost;` **không** phân giải được (nhánh cfg tắt) trong crate "
                "ở GỐC MỚI ⇒ `E_STRUCT_MOD_MISSING` (trước vá: XANH — `_mods_of` bỏ im lặng)",
                (not rep.ok)
                and any("E_STRUCT_MOD_MISSING" in e and "ghost" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # --- V4: đối chứng dương — `mod ghost;` có `ghost.rs` ⇒ không `E_STRUCT_MOD_MISSING` -
        d = tmp / "kt264-v4-present"
        _fixture_v2(d)
        _write(d / _EVIL / "Cargo.toml", _EVIL_MANIFEST)
        _write(d / _EVIL / "src/lib.rs", "mod ghost;\npub fn ok() -> u8 { 0 }\n")
        _write(d / _EVIL / "src/ghost.rs", "pub fn g() -> u8 { 1 }\n")
        rep = scan_v2(d, strict=False, scope=_evil_scope(d))
        cases.append(
            (
                "KT-264 V4 đối chứng dương: `mod ghost;` có `ghost.rs` ⇒ không "
                "`E_STRUCT_MOD_MISSING` (luật không phải luôn-đỏ)",
                not any("E_STRUCT_MOD_MISSING" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- V5: cạnh `#[path]` RA NGOÀI crate ⇒ `E_STRUCT_MOD_OUTSIDE` --------------------
        d = tmp / "kt264-v5-outside"
        _fixture_v2(d)
        _write(d / _EVIL / "Cargo.toml", _EVIL_MANIFEST)
        _write(d / "v2/kernel/vifast-plat-common/src/evil.rs", "pub fn e() -> u8 { 0 }\n")
        _write(
            d / _EVIL / "src/lib.rs",
            '#[path = "../../../kernel/vifast-plat-common/src/evil.rs"]\nmod evil;\n',
        )
        rep = scan_v2(d, strict=False, scope=_evil_scope(d))
        cases.append(
            (
                "KT-264 V5: `#[path]` trỏ **ra ngoài crate** (vẫn trong `v2/kernel`) ⇒ "
                "`E_STRUCT_MOD_OUTSIDE` (trước vá: XANH — `resolve_include_targets` chỉ chặn "
                "ra ngoài `v2/kernel`+`v2/sim`)",
                (not rep.ok)
                and any("E_STRUCT_MOD_OUTSIDE" in e and "evil" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # --- V6: đối chứng dương — `#[path]` TRONG crate ⇒ không `E_STRUCT_MOD_OUTSIDE` -----
        d = tmp / "kt264-v6-inside"
        _fixture_v2(d)
        _write(d / _EVIL / "Cargo.toml", _EVIL_MANIFEST)
        _write(d / _EVIL / "src/evil.rs", "pub fn e() -> u8 { 0 }\n")
        _write(d / _EVIL / "src/lib.rs", '#[path = "evil.rs"]\nmod evil;\n')
        rep = scan_v2(d, strict=False, scope=_evil_scope(d))
        cases.append(
            (
                "KT-264 V6 đối chứng dương: `#[path]` trỏ **trong** crate ⇒ không "
                "`E_STRUCT_MOD_OUTSIDE` (luật không phải luôn-đỏ)",
                not any("E_STRUCT_MOD_OUTSIDE" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- V7: `mod` hỏng trong `v2/lib` (ngoài `V2_SCAN`) ⇒ `E_LEX_STRUCT` --------------
        d = tmp / "kt264-v7-lib-lex"
        _fixture_v2(d)
        _write(d / _NET / "Cargo.toml", _NET_MANIFEST)
        _write(d / _NET / "src/lib.rs", "pub fn ok() -> u8 { 0 }\n")
        _write(d / _NET / "src/bad.rs", "#[cfg(any())]\nmod 123;\n")
        rep = scan_v2(d, strict=False, scope=_scope258(d, _HOST_NET, _KERNEL_PKGS, _CLOSURE_PKGS))
        cases.append(
            (
                "KT-264 V7: khai báo `mod` **hỏng** trong `v2/lib` (ngoài `V2_SCAN`) ⇒ "
                "`E_LEX_STRUCT` (trước vá: XANH — `scan_structure` chỉ chạy `V2_SCAN`)",
                (not rep.ok)
                and any("E_LEX_STRUCT" in e and "bad.rs" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # --- V8: đối chứng dương — `v2/lib` sạch ⇒ không `E_LEX_STRUCT` --------------------
        d = tmp / "kt264-v8-lib-clean"
        _fixture_v2(d)
        _write(d / _NET / "Cargo.toml", _NET_MANIFEST)
        _write(d / _NET / "src/lib.rs", "pub fn ok() -> u8 { 0 }\n")
        _write(d / _NET / "src/good.rs", "pub fn g() -> u8 { 1 }\n")
        rep = scan_v2(d, strict=False, scope=_scope258(d, _HOST_NET, _KERNEL_PKGS, _CLOSURE_PKGS))
        cases.append(
            (
                "KT-264 V8 đối chứng dương: `v2/lib` sạch ⇒ không `E_LEX_STRUCT` "
                "(luật không phải luôn-đỏ)",
                not any("E_LEX_STRUCT" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- V9: `src/bin/*.rs` là crate root TỰ NHẬN ⇒ không mồ côi -----------------------
        d = tmp / "kt264-v9-srcbin"
        _fixture_v2(d)
        _write(d / _NET / "Cargo.toml", _NET_MANIFEST)
        _write(d / _NET / "src/lib.rs", "pub fn ok() -> u8 { 0 }\n")
        _write(d / _NET / "src/bin/tool.rs", "fn main() {}\n")
        rep = scan_v2(d, strict=False, scope=_scope258(d, _HOST_NET, _KERNEL_PKGS, _CLOSURE_PKGS))
        cases.append(
            (
                "KT-264 V9 đối chứng dương: `src/bin/*.rs` là crate root **tự nhận** của cargo "
                "⇒ không `E_STRUCT_ORPHAN` (thiếu ca này thì luật báo oan trên cây thật)",
                not any("E_STRUCT_ORPHAN" in e and "src/bin" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- V10: đích `include!` dưới `src/` KHÔNG phải mồ côi ----------------------------
        d = tmp / "kt264-v10-include"
        _fixture_v2(d)
        _write(d / _NET / "Cargo.toml", _NET_MANIFEST)
        _write(
            d / _NET / "src/lib.rs",
            'pub mod inner {\n    include!("body.rs");\n}\n',
        )
        _write(d / _NET / "src/body.rs", "pub fn f() -> u8 { 0 }\n")
        rep = scan_v2(d, strict=False, scope=_scope258(d, _HOST_NET, _KERNEL_PKGS, _CLOSURE_PKGS))
        cases.append(
            (
                "KT-264 V10 đối chứng dương: tệp dưới `src/` nạp bằng `include!` **không** "
                "phải mồ côi (được biên dịch)",
                not any("E_STRUCT_ORPHAN" in e and "body.rs" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        # --- V11: crate có manifest mà KHÔNG có crate root ⇒ `E_STRUCT_NO_ROOT` ------------
        d = tmp / "kt264-v11-noroot"
        _fixture_v2(d)
        _write(d / _NET / "Cargo.toml", _NET_MANIFEST)
        _write(d / _NET / "src/foo.rs", "pub fn f() -> u8 { 0 }\n")
        rep = scan_v2(d, strict=False, scope=_scope258(d, _HOST_NET, _KERNEL_PKGS, _CLOSURE_PKGS))
        cases.append(
            (
                "KT-264 V11: crate **có** manifest mà **không** có crate root ⇒ "
                "`E_STRUCT_NO_ROOT` (fail-closed, không im lặng bỏ qua)",
                (not rep.ok)
                and any("E_STRUCT_NO_ROOT" in e and _NET in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # --- V12: đối chứng dương — có `src/lib.rs` ⇒ không `E_STRUCT_NO_ROOT` -------------
        d = tmp / "kt264-v12-root"
        _fixture_v2(d)
        _write(d / _NET / "Cargo.toml", _NET_MANIFEST)
        _write(d / _NET / "src/lib.rs", "pub fn ok() -> u8 { 0 }\n")
        rep = scan_v2(d, strict=False, scope=_scope258(d, _HOST_NET, _KERNEL_PKGS, _CLOSURE_PKGS))
        cases.append(
            (
                "KT-264 V12 đối chứng dương: crate có `src/lib.rs` ⇒ không `E_STRUCT_NO_ROOT` "
                "(luật không phải luôn-đỏ)",
                not any("E_STRUCT_NO_ROOT" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )
        # --- KT-333e (D2-247) — `--combined`: **một lượt quét, cùng toàn bộ luật** ----------
        # Bằng chứng "cắt thời gian **không** mất độ phủ": trên **cùng** cây, report của lượt
        # gộp phải **bằng đúng** report của lượt riêng tương ứng — so cả `errors`, `warnings`,
        # `info` (không chỉ `ok`). Chạy cho cây xanh, cây đỏ ở luật arch (`doorbell`), cây đỏ ở
        # luật token, và cây có `extra_doors` (luật **chỉ** `--strict`) để chứng minh chế độ
        # gộp **không** rò lỗi strict sang bảng thường.

        def _combined_matches(d: Path, extra_doors=None) -> Tuple[bool, str]:
            rn = scan_v2(d, strict=False, extra_doors=extra_doors)
            rs = scan_v2(d, strict=True, extra_doors=extra_doors)
            cn, cs = scan_v2(d, strict=True, extra_doors=extra_doors, combined=True)
            same = (
                rn.errors == cn.errors
                and rs.errors == cs.errors
                and rn.warnings == cn.warnings
                and rs.warnings == cs.warnings
                and rn.info == cn.info
                and rs.info == cs.info
            )
            return same, (
                ""
                if same
                else f"normal_gộp={cn.errors} normal_riêng={rn.errors} | "
                f"strict_gộp={cs.errors} strict_riêng={rs.errors}"
            )

        d = tmp / "kt333e-combined-clean"
        _fixture_v2(d)
        ok, detail = _combined_matches(d)
        cn, cs = scan_v2(d, strict=True, combined=True)
        cases.append(
            (
                "KT-333e: `--combined` trên cây xanh ⇒ **hai** bảng xanh, khớp từng dòng với "
                "hai lượt riêng",
                ok and cn.ok and cs.ok,
                detail,
            )
        )

        d = tmp / "kt333e-combined-doorbell"
        _fixture_v2(d, extra_doorbell=True)
        ok, detail = _combined_matches(d)
        cn, cs = scan_v2(d, strict=True, combined=True)
        cases.append(
            (
                "KT-333e: `--combined` + `IpiKind::Wake` ở arch ⇒ **cả hai** bảng đỏ "
                "`E_ARCH_DOORBELL`, khớp hai lượt riêng (luật arch không bị gộp làm mất)",
                ok
                and (not cn.ok)
                and (not cs.ok)
                and any("E_ARCH_DOORBELL" in e for e in cn.errors)
                and any("E_ARCH_DOORBELL" in e for e in cs.errors),
                detail,
            )
        )

        # **KT-267 (b)** — chữ `doorbell` trong `vifast-plat-*/` (**không** phải driver virtio)
        # là ĐỎ ở **cả** chế độ; `--combined` phải báo y hệt hai lượt riêng (bài học KT-338c).
        d = tmp / "kt333e-combined-doorbell-word"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-plat-riscv64/src/dbg_evil.rs",
            "pub fn f() -> u32 {\n    // doorbell riêng theo arch\n    1\n}\n",
        )
        ok, detail = _combined_matches(d)
        cn, cs = scan_v2(d, strict=True, combined=True)
        cases.append(
            (
                "KT-333e: `--combined` + chữ `doorbell` trong `vifast-plat-*/` ⇒ cả hai bảng "
                "đỏ `E_ARCH_DOORBELL`, khớp hai lượt riêng (KT-267(b)/KT-338c)",
                ok
                and any("E_ARCH_DOORBELL" in e for e in cn.errors)
                and any("E_ARCH_DOORBELL" in e for e in cs.errors),
                detail,
            )
        )

        d = tmp / "kt333e-combined-wake-call"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-plat-riscv64/src/bad_wake.rs",
            "pub fn f(m: &mut M) {\n    m.wake(1);\n}\n",
        )
        ok, detail = _combined_matches(d)
        cn, cs = scan_v2(d, strict=True, combined=True)
        cases.append(
            (
                "KT-333e: `--combined` + `wake(` trong mã sản phẩm ⇒ cả hai bảng đỏ "
                "`E_STRICT_TEST_ONLY_WAKE`, khớp hai lượt riêng (luật token không bị mất)",
                ok
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in cn.errors)
                and any("E_STRICT_TEST_ONLY_WAKE" in e for e in cs.errors),
                detail,
            )
        )

        d = tmp / "kt333e-combined-extra-doors"
        _fixture_v2(d)
        ed = {(KERNEL_STATE_WRITE, "khong_ton_tai")}
        ok, detail = _combined_matches(d, extra_doors=ed)
        cn, cs = scan_v2(d, strict=True, extra_doors=ed, combined=True)
        cases.append(
            (
                "KT-333e: `--combined` giữ `extra_doors` **chỉ** ở bảng strict "
                "(`E_STRICT_STALE` không rò sang bảng thường)",
                ok
                and cn.ok
                and (not cs.ok)
                and any("E_STRICT_STALE" in e for e in cs.errors),
                detail,
            )
        )

        # --- KT-333e — ca âm cho các luật trước đây **không** có ca trong selftest ----------
        # (rà ngày 2026-11-23: 38 mã lỗi; 5 mã không xuất hiện trong `selftest()` —
        #  `E_LEX_UNKNOWN` là nhánh **không thể tới** (`punct` nuốt mọi ký tự, có
        #  `# pragma: no cover`), bốn mã còn lại thêm ca ở đây.)
        d = tmp / "kt333e-lex-unterminated-char"
        _fixture_v2(d)
        _write(
            d / "v2/kernel/vifast-sync/src/bad_char.rs",
            "pub fn f() -> u32 {\n    let x = '1;\n    1\n}\n",
        )
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-333e ca âm: literal ký tự hỏng (`'1`) trong tệp thuộc phạm vi ⇒ "
                "`E_LEX_UNTERMINATED_CHAR` (fail-closed, không lặng lẽ bỏ tệp)",
                any("E_LEX_UNTERMINATED_CHAR" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        d = tmp / "kt333e-channel-missing"
        _fixture_v2(d)
        for _relp in (KERNEL_SCHED, SIM_KOBJ_ROOT):
            _t = (d / _relp).read_text(encoding="utf-8")
            _t = _t.replace("MailboxMsg::Wake(tcb));", "MailboxMsg::Stop);")
            (d / _relp).write_text(_t, encoding="utf-8")
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-333e ca âm: không còn chỗ phát `MailboxMsg::Wake` ⇒ `E_CHANNEL_MISSING` "
                "ở **cả** kernel và sim (kênh chéo hart không được mất âm thầm)",
                (not rep.ok)
                and sum(1 for e in rep.errors if "E_CHANNEL_MISSING" in e) >= 3,
                f"errors={rep.errors}",
            )
        )

        d = tmp / "kt333e-consumer-missing"
        _fixture_v2(d)
        for _relp in (KERNEL_SCHED, SIM_KOBJ_ROOT):
            _t = (d / _relp).read_text(encoding="utf-8")
            # Xoá **hẳn** nhánh `Wake` (không thay bằng mẫu khác vẫn chứa `MailboxMsg::Wake`).
            _t = _t.replace(
                "            MailboxMsg::Wake(tcb) => { let _ = self.wake_step(tcb); }\n", ""
            )
            (d / _relp).write_text(_t, encoding="utf-8")
        rep = scan_v2(d, strict=False)
        cases.append(
            (
                "KT-333e ca âm: không còn nhánh tiêu thụ `MailboxMsg::Wake` ⇒ "
                "`E_CONSUMER_MISSING` (mỗi loại phải có đúng một chỗ xử lý)",
                (not rep.ok) and any("E_CONSUMER_MISSING" in e for e in rep.errors),
                f"errors={rep.errors}",
            )
        )

        d = tmp / "kt333e-v1-nochannel"
        _write(d / "crates/vifast-kernel/src/lib.rs", "fn f() -> u32 { 0 }\n")
        rep = scan_v1(d, strict=False)
        cases.append(
            (
                "KT-333e ca âm: v1 không có kênh đánh thức nào ⇒ `E_V1_NO_CHANNEL` "
                "(fail-closed, không coi 'không thấy gì' là xanh)",
                (not rep.ok) and any("E_V1_NO_CHANNEL" in e for e in rep.errors),
                f"errors={rep.errors}",
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


