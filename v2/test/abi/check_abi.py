#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-04a — kiểm `spec/abi.toml` (nguồn sự thật ABI, D2-11) bằng máy.

Bộ kiểm này là **hợp đồng** lấy từ QD §4 và thiết kế §4.20/§4.21/§3.9; nó cố ý
viết TRƯỚC abi.toml để "đỏ trước": file chưa có / thiếu mục ⇒ exit 1.

Dùng: `python3 check_abi.py [--file spec/abi.toml]`  (cần Python ≥3.11 có tomllib)
"""

import re
import sys
from pathlib import Path

SPDX = "SPDX-License-Identifier"

# Gốc repo (tệp nằm ở `v2/test/abi/check_abi.py`) — để cross-check artifact do `vifast-gen` sinh.
ROOT = Path(__file__).resolve().parents[3]

SYSCALLS = {
    0: "Call",
    1: "Send",
    2: "NBSend",
    3: "Recv",
    4: "NBRecv",
    5: "ReplyRecv",
    6: "NBSendRecv",
    7: "NBSendWait",
    8: "Yield",
    9: "WaitAddr",
    10: "WakeAddr",
    11: "ClockRead",
    12: "DebugPutStr",
    13: "DebugHalt",
    14: "Exit",
    15: "DebugGetInfo",
}

ERRORS = [
    "InvalidCapability",
    "WrongObjectType",
    "InsufficientRights",
    "WxViolation",
    "InvalidArgument",
    "Alignment",
    "SlotOccupied",
    "SlotEmpty",
    "NotEnoughMemory",
    "AlreadyMapped",
    "NotMapped",
    "MissingPageTable",
    "HasChildren",
    "Busy",
    "Cancelled",
    "Timeout",
    "IllegalOperation",
    "NotSupported",
    "RangeError",
]

# **D2-242 nhóm 9 (9B)**: `Frame` → `frozen`, `VSpace`/`PageTable` → `core` ⇒ 11 object lõi;
# reserved còn 5 (`12..16`).
CORE_OBJECTS = [
    "Untyped",
    "SchedControl",
    "CNode",
    "Tcb",
    "SchedContext",
    "Endpoint",
    "Notification",
    "Reply",
    "VSpace",
    "PageTable",
    "Frame",
]

CORE_IDS = {
    "Untyped": 1, "CNode": 2, "Tcb": 3, "SchedContext": 4,
    "Endpoint": 5, "Notification": 6, "Reply": 7, "SchedControl": 8,
    "VSpace": 9, "PageTable": 10, "Frame": 11,
}

TCB_METHODS = [
    "Configure",
    "SetFaultEp",
    "ReadState",
    "Suspend",
    "SetPriority",
    "SetMcp",
    "SetAffinity",
    "SetSchedParams",
    "SetTimeoutEp",
    "ReadRegs",
    "WriteRegs",
    "CopyRegs",
    "Resume",
    "BindNotification",
    "UnbindNotification",
    "SetTls",
    "Exec",
]

CNODE_METHODS = ["Copy", "Mint", "Move", "Mutate", "Delete", "Revoke", "Rotate"]

# ---------------------------------------------------------------------------------------------
# **D2-261 nhóm 8 (8A)** — `[boot_caps]`: bố cục cap lúc boot. Nguồn **thứ hai** để đối chiếu là
# `proc_abi.rs` đang chạy (kernel) — spec không được lệch khỏi mã. Hai **không gian chỉ số** tách
# bảng: `cspace` (slot cap) và `proc` (ô chương trình `MAX_PROCS`).
# ---------------------------------------------------------------------------------------------
BOOT_CAPS_CSPACE = {
    "fault": ("EP_FAULT", 1),
    "proc": ("EP_PROC", 2),
    "uart": ("EP_UART", 3),
    "reply": ("EP_REPLY", 4),
    "ps": ("EP_PS", 5),
    "sched_control": ("EP_CTL", 6),
    "fs": ("EP_FS", 7),
    "events": ("EP_EVENTS", 8),
    "net": ("EP_NET", 9),
    "net_reply": ("EP_NET_REPLY", 15),
    "net_kern": ("EP_NET_KERN", 16),
    "net_kern_reply": ("EP_NET_KERN_REPLY", 17),
    "exec": ("EP_EXEC", 18),
    "mem": ("EP_MEM", 21),
    "loop_ep": ("EP_LOOP", 22),
    "net_client": ("EP_NET_CLIENT", 26),
    "net_client_reply": ("EP_NET_CLIENT_REPLY", 27),
    "self_cspace": ("SLOT_CSPACE", 20),
    "loop_arena": ("LOOP_ARENA", 23),
    "loop_tcb": ("LOOP_TCB_SLOT", 24),
    "loop_sc": ("LOOP_SC_SLOT", 25),
}
BOOT_CAPS_ARRAYS = {
    "tcb": ("TCB_SLOTS", [10, 11, 12, 13, 14]),
    "sc": ("SC_SLOTS", [30, 31, 32, 33, 34]),
}
BOOT_CAPS_PROC = {
    "procsrv": ("SLOT_PROCSRV", 0),
    "shell": ("SLOT_SHELL", 1),
}
PROC_ABI = ROOT / "v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs"


def _proc_const(text: str, name: str):
    """Đọc `pub const <name>: usize|u8 = <số>;` hoặc `[usize; N] = [..];` của `proc_abi.rs`."""
    match = re.search(rf"pub const {name}\s*:\s*[^=]+=\s*([^;]+);", text)
    if not match:
        return None
    body = match.group(1).strip()
    if body.startswith("["):
        return [int(v) for v in re.findall(r"\d+", body)]
    digits = re.match(r"\d+", body)
    return int(digits.group(0)) if digits else None


def check_boot_caps(doc: dict, proc_abi: Path = PROC_ABI) -> None:
    """Kiểm `[boot_caps]` ↔ `proc_abi.rs` + artifact sinh (`D2-261` nhóm 8).

    `proc_abi` tách tham số để `--selftest` mô phỏng được tệp vắng (F2b).
    """
    bc = doc.get("boot_caps") or {}
    check(isinstance(bc, dict) and bool(bc), "thiếu [boot_caps] (D2-261 nhóm 8)")
    if not isinstance(bc, dict):
        return
    check(bc.get("boot_abi") == 1, f"[boot_caps].boot_abi phải = 1, đang {bc.get('boot_abi')!r}")
    check(bc.get("none_cspace") == 0xFFFF,
          f"[boot_caps].none_cspace phải = 0xFFFF, đang {bc.get('none_cspace')!r}")
    check(bc.get("none_proc") == 0xFF,
          f"[boot_caps].none_proc phải = 0xFF, đang {bc.get('none_proc')!r}")
    space = bc.get("cspace") or {}
    # **KT-417 hardening F2b**: `proc_abi.rs` là **nguồn thứ hai** của hợp đồng; tệp vắng hoặc
    # không đọc được thì KHÔNG được bỏ qua phép đối chiếu (trước đây `if code:` nuốt lỗi ⇒ spec
    # lệch kernel vẫn xanh khi tệp vắng).
    try:
        code = proc_abi.read_text(encoding="utf-8")
    except OSError as exc:
        check(False, f"không đọc được {proc_abi} để đối chiếu [boot_caps] với kernel: {exc}")
        return
    if not code.strip():
        check(False, f"{proc_abi} rỗng — không đối chiếu được [boot_caps] với kernel")
        return
    for field, (const, want) in BOOT_CAPS_CSPACE.items():
        leaf = space.get(field) or {}
        check(leaf.get("value") == want,
              f"[boot_caps.cspace].{field} phải = {want}, đang {leaf.get('value')!r}")
        check(str(leaf.get("source", "")).endswith("#" + const),
              f"[boot_caps.cspace].{field}.source phải trỏ proc_abi.rs#{const}")
        check(_proc_const(code, const) == want,
              f"proc_abi.rs {const} phải = {want}, đang {_proc_const(code, const)}")
    for field, (const, want) in BOOT_CAPS_ARRAYS.items():
        leaf = space.get(field) or {}
        check(leaf.get("value") == want,
              f"[boot_caps.cspace].{field} phải = {want}, đang {leaf.get('value')!r}")
        check(str(leaf.get("source", "")).endswith("#" + const),
              f"[boot_caps.cspace].{field}.source phải trỏ proc_abi.rs#{const}")
        check(_proc_const(code, const) == want,
              f"proc_abi.rs {const} phải = {want}, đang {_proc_const(code, const)}")
    proc = bc.get("proc") or {}
    for field, (const, want) in BOOT_CAPS_PROC.items():
        leaf = proc.get(field) or {}
        check(leaf.get("value") == want,
              f"[boot_caps.proc].{field} phải = {want}, đang {leaf.get('value')!r}")
        check(str(leaf.get("source", "")).endswith("#" + const),
              f"[boot_caps.proc].{field}.source phải trỏ proc_abi.rs#{const}")
        check(_proc_const(code, const) == want,
              f"proc_abi.rs {const} phải = {want}, đang {_proc_const(code, const)}")
    test_child = ((proc.get("test") or {}).get("child") or {})
    product_netsrv = ((proc.get("product") or {}).get("netsrv") or {})
    check(test_child.get("value") == 2 and
          str(test_child.get("source", "")).endswith("#SLOT_CHILD"),
          f"[boot_caps.proc.test].child phải = 2 + source #SLOT_CHILD, đang {test_child!r}")
    check(product_netsrv.get("value") == 2 and
          str(product_netsrv.get("source", "")).endswith("#SLOT_NETSRV"),
          f"[boot_caps.proc.product].netsrv phải = 2 + source #SLOT_NETSRV, đang {product_netsrv!r}")
    check(test_child.get("value") == product_netsrv.get("value"),
          "child/netsrv phải CÙNG ô 2 (khác ảnh — D2-261 nhóm 8)")
    check(_proc_const(code, "SLOT_CHILD") == 2, "proc_abi.rs SLOT_CHILD phải = 2")
    check(_proc_const(code, "SLOT_NETSRV") == 2, "proc_abi.rs SLOT_NETSRV phải = 2")
    generated = ROOT / "v2/lib/vifast-abi/src/generated.rs"
    if generated.exists():
        text = generated.read_text(encoding="utf-8")
        for needle in (
            "pub const BOOT_ABI: u32 = 1;",
            "pub const BOOT_CAPS_NONE_CSPACE: u16 = 0xFFFF;",
            "pub const BOOT_CAPS_NONE_PROC: u8 = 0xFF;",
            "pub struct BootCapLayout {",
            "pub const BOOT_CAPS_TEST: BootCapLayout = BootCapLayout {",
            "pub const BOOT_CAPS_PRODUCT: BootCapLayout = BootCapLayout {",
        ):
            check(needle in text, f"generated.rs: thiếu {needle!r} (D2-261 nhóm 8)")

fails = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def selftest() -> int:
    """Ca dương (spec thật xanh) + ca âm (đột biến ⇒ đỏ **đúng lý do**) — KT-345a/D2-242."""
    import shutil
    import subprocess
    import tempfile

    base = ROOT / "spec/abi.toml"
    try:
        text = base.read_text(encoding="utf-8")
    except FileNotFoundError:
        print(f"not ok 1 - không có {base}")
        return 1
    tmp = Path(tempfile.mkdtemp(prefix="kt-abi-st."))
    cases: list[tuple[str, bool, str]] = []
    me = str(Path(__file__).resolve())

    def run(path: Path, proc_abi: Path | None = None) -> tuple[int, str]:
        argv = [sys.executable, me, "--file", str(path)]
        if proc_abi is not None:
            argv += ["--proc-abi", str(proc_abi)]
        proc = subprocess.run(argv, capture_output=True, text=True)
        return proc.returncode, proc.stdout + proc.stderr

    rc, out = run(base)
    cases.append(("dương: spec/abi.toml thật xanh", rc == 0, out[:300]))

    def mutate(name: str, old: str, new: str, want: str) -> None:
        if old not in text:
            cases.append((name, False, f"không đột biến được (không thấy {old!r})"))
            return
        path = tmp / f"{abs(hash(name))}.toml"
        path.write_text(text.replace(old, new, 1), encoding="utf-8")
        rc, out = run(path)
        cases.append((name, rc == 1 and want in out,
                      f"muốn rc=1 + {want!r}, nhận rc={rc}: {out[:300]}"))

    mutate(
        "âm: Tcb.Configure.errors thiếu 'Busy' ⇒ đỏ",
        '"InvalidArgument", "Busy"]',
        '"InvalidArgument"]',
        "Tcb.Configure.errors",
    )
    mutate(
        "âm: ipc_buffer.extra_caps_offset 560→564 ⇒ đỏ",
        "extra_caps_offset = 560",
        "extra_caps_offset = 564",
        "extra_caps_offset phải",
    )
    mutate(
        "âm: x86_64 return_message_info r13→r11 ⇒ đỏ (7C-r13)",
        'return_message_info = "r13"',
        'return_message_info = "r11"',
        "return_message_info phải = 'r13'",
    )
    mutate(
        "âm: Frame stability frozen→reserved ⇒ đỏ (9B)",
        'Frame = { id = 11, stability = "frozen"',
        'Frame = { id = 11, stability = "reserved"',
        "Frame.stability",
    )
    mutate(
        "âm: DebugGetInfo return_channel đổi vai trò value ⇒ đỏ (8C)",
        'return_channel = { value = "abi_version"',
        'return_channel = { value = "min_compat"',
        "return_channel",
    )
    mutate(
        "âm: object_size aarch64 trỏ alias khác ⇒ đỏ (8C)",
        'aarch64 = { same_as = "model" }',
        'aarch64 = { same_as = "riscv64" }',
        "same_as phải = 'model'",
    )
    mutate(
        "âm: x86_64 clobbered mất r13 ⇒ đỏ (7C-r13)",
        'clobbered = ["rax", "rdx", "r13", "r10", "r8", "r9", "r12"]',
        'clobbered = ["rax", "rdx", "r10", "r8", "r9", "r12"]',
        "clobbered phải",
    )
    mutate(
        "âm: [boot_caps].boot_abi 1→2 ⇒ đỏ (D2-261 nhóm 8)",
        "boot_abi = 1",
        "boot_abi = 2",
        "[boot_caps].boot_abi",
    )
    mutate(
        "âm: [boot_caps.cspace].fault 1→9 ⇒ đỏ (D2-261 nhóm 8)",
        "fault = { value = 1,",
        "fault = { value = 9,",
        ".cspace].fault",
    )

    # --- **KT-417 hardening**: case âm cho F2a/F2b/F5 -----------------------------
    # F2a — `gen.py` phải **TỪ CHỐI** khoá lạ trong `[boot_caps*]`; trước bản vá nó bỏ im
    # lặng (khoá không vào `generated.rs`, không lỗi) ⇒ gõ sai tên trường vẫn xanh. Cây chép
    # đủ 3 tệp để "thêm một khoá lạ" là **khác biệt duy nhất** so với bản đang commit.
    gen_py = ROOT / "v2/tools/vifast-gen/gen.py"
    gen_root = tmp / "gen"
    (gen_root / "spec").mkdir(parents=True)
    (gen_root / "v2/lib/vifast-abi/src").mkdir(parents=True)
    (gen_root / "docs").mkdir(parents=True)
    shutil.copy2(base, gen_root / "spec/abi.toml")
    shutil.copy2(ROOT / "v2/lib/vifast-abi/src/generated.rs",
                 gen_root / "v2/lib/vifast-abi/src/generated.rs")
    shutil.copy2(ROOT / "docs/abi.md", gen_root / "docs/abi.md")

    def run_gen(spec_text: str) -> tuple[int, str]:
        (gen_root / "spec/abi.toml").write_text(spec_text, encoding="utf-8")
        proc = subprocess.run([sys.executable, str(gen_py), "--check", "--root", str(gen_root)],
                              capture_output=True, text=True)
        return proc.returncode, proc.stdout + proc.stderr

    rc, out = run_gen(text)
    cases.append(("dương: vifast-gen --check trên cây chép = PASS (đối chứng F2a)",
                  rc == 0, out[:300]))

    def mutate_gen(name: str, old: str, new: str) -> None:
        if old not in text:
            cases.append((name, False, f"không đột biến được (không thấy {old!r})"))
            return
        rc, out = run_gen(text.replace(old, new, 1))
        cases.append((name, rc == 1 and "khoá lạ" in out,
                      f"muốn rc=1 + 'khoá lạ', nhận rc={rc}: {out[:300]}"))

    mutate_gen(
        "âm F2a: khoá lạ trong [boot_caps] ⇒ gen TỪ CHỐI (không bỏ im lặng)",
        "boot_abi = 1\n",
        "boot_abi = 1\nboot_abii = 2\n",
    )
    mutate_gen(
        "âm F2a: khoá lạ trong [boot_caps.cspace] ⇒ gen TỪ CHỐI",
        'fault = { value = 1, source = "v2/kernel',
        'faul = { value = 1, source = "x" }\nfault = { value = 1, source = "v2/kernel',
    )
    mutate_gen(
        "âm F2a: khoá lạ trong [boot_caps.proc] ⇒ gen TỪ CHỐI",
        "procsrv = { value = 0,",
        'procsr = { value = 0, source = "x" }\nprocsrv = { value = 0,',
    )
    mutate_gen(
        "âm F2a: khoá lạ trong [boot_caps.proc.test] ⇒ gen TỪ CHỐI",
        "child = { value = 2,",
        'chil = { value = 2, source = "x" }\nchild = { value = 2,',
    )
    mutate_gen(
        "âm F2a: khoá lạ trong [boot_caps.proc.product] ⇒ gen TỪ CHỐI",
        'netsrv = { value = 2, source = "v2/kernel',
        'netrsv = { value = 2, source = "x" }\nnetsrv = { value = 2, source = "v2/kernel',
    )

    # F2b — `proc_abi.rs` là **nguồn thứ hai**; vắng tệp thì phép đối chiếu KHÔNG được bỏ qua.
    rc, out = run(base, tmp / "selftest-khong-co-proc_abi.rs")
    cases.append((
        "âm F2b: proc_abi.rs vắng ⇒ abi-check ĐỎ (không bỏ qua đối chiếu)",
        rc == 1 and "proc_abi" in out,
        f"muốn rc=1 + nhắc proc_abi, nhận rc={rc}: {out[:300]}",
    ))

    # F5 — `Tcb.Exec` (16, `install_at = "KT-420"`) chưa cài: canh **cả** `unsupported`
    # lẫn mốc `install_at`; trước bản vá danh sách chỉ có 3 object VM nên Exec rơi khỏi lưới.
    mutate(
        "âm F5: Tcb.Exec mất unsupported = true ⇒ đỏ",
        '16 = { name = "Exec", unsupported = true, install_at = "KT-420"',
        '16 = { name = "Exec", install_at = "KT-420"',
        "Tcb.Exec",
    )
    mutate(
        "âm F5: Tcb.Exec install_at KT-420→KT-421 ⇒ đỏ",
        'install_at = "KT-420"',
        'install_at = "KT-421"',
        "Tcb.Exec",
    )

    shutil.rmtree(tmp, ignore_errors=True)

    print("TAP version 13")
    print(f"1..{len(cases)}")
    bad = 0
    for i, (name, ok, detail) in enumerate(cases, 1):
        if ok:
            print(f"ok {i} - {name}")
        elif detail:
            print(f"not ok {i} - {name} ({detail})")
        else:
            print(f"not ok {i} - {name}")
        bad += 0 if ok else 1
    if bad:
        print(f"# selftest: {bad}/{len(cases)} ca sai kỳ vọng")
        return 1
    print(f"# selftest: {len(cases)}/{len(cases)} ca đúng như kỳ vọng "
          f"(dương: spec thật + gen đối chứng; còn lại là âm)")
    return 0


def main():
    path = "spec/abi.toml"
    proc_abi = PROC_ABI
    args = sys.argv[1:]
    if args == ["--selftest"]:
        return selftest()
    while args:
        flag = args.pop(0)
        if flag == "--file" and args:
            path = args.pop(0)
        elif flag == "--proc-abi" and args:
            # Chỉ dùng cho `--selftest` (mô phỏng `proc_abi.rs` vắng — F2b); cổng không truyền cờ này.
            proc_abi = Path(args.pop(0))
        else:
            print(f"abi-check: đối số lạ {flag!r} (dùng --file | --proc-abi | --selftest)")
            return 2

    try:
        with open(path, "rb") as fh:
            raw = fh.read()
    except FileNotFoundError:
        print(f"abi-check: KHÔNG có {path}")
        return 1

    try:
        import tomllib
    except ModuleNotFoundError:
        print("abi-check: cần Python ≥3.11 (tomllib)")
        return 2

    try:
        doc = tomllib.loads(raw.decode("utf-8"))
    except Exception as exc:  # noqa: BLE001 - muốn in mọi lỗi parse
        print(f"abi-check: TOML không parse được: {exc}")
        return 1

    check(raw.startswith(b"#"), "thiếu header comment (SPDX) ở dòng 1")
    check(SPDX.encode() in raw, "thiếu dòng SPDX-License-Identifier")

    # --- version -------------------------------------------------------
    check(doc.get("status") in ("REVIEW", "APPROVED"),
          "status phải REVIEW (chưa duyệt) hoặc APPROVED (đã duyệt, D2-22)")
    check(isinstance(doc.get("golden"), str) and doc["golden"], "thiếu golden file (D2-15)")
    check(doc.get("abi_version") == 1, "abi_version phải = 1")
    check(doc.get("min_compat") == 1, "min_compat phải = 1")
    frozen = doc.get("frozen", {})
    check(frozen.get("id_reuse") == "never", "thiếu luật ID không tái dùng")
    for key in ("wire_syscalls", "message_layout", "error_table", "object_layout"):
        check(frozen.get(key) is True, f"frozen.{key} phải true (5 thứ đóng băng, D2-15)")

    # --- syscall 0..15, dải 16..31 reserved ---------------------------
    sc = {int(k): v for k, v in doc.get("syscall", {}).items()}
    check(set(sc.keys()) == set(SYSCALLS.keys()), f"syscall phải là 0..15, đang có {sorted(sc)}")
    for num, name in SYSCALLS.items():
        got = sc.get(num, {})
        got_name = got.get("name") if isinstance(got, dict) else got
        check(str(got_name) == name, f"syscall {num} phải tên {name}, đang {got_name!r}")
    res = doc.get("syscall_reserved", {})
    check(res.get("from") == 16 and res.get("to") == 31,
          "dải reserved phải 16..31 (15 đã cấp cho DebugGetInfo, D2-29)")

    # --- 19 mã lỗi ----------------------------------------------------
    err = doc.get("error", {})
    names = [v.get("name") if isinstance(v, dict) else v for _, v in sorted(err.items(), key=lambda kv: int(kv[0]))]
    check(names == ERRORS, f"19 mã lỗi phải đúng thứ tự QD §4; đang {names}")
    check(len(err) == 19, f"phải có đúng 19 mã lỗi, đang {len(err)}")
    reserved = doc.get("syscall_reserved", {})
    objs_reserved = doc.get("object_reserved", {})
    check(objs_reserved.get("from") == 17 and objs_reserved.get("to") == 31,
          "object reserved phải 17..31 (D2-32: 9..16 đã cấp tên)")
    dbg = doc.get("debug_info", {})
    tr = doc.get("trace", {})
    check(tr.get("record_bytes") == 32, "trace.record_bytes phải = 32 (D2-40)")
    check(tr.get("endianness") == "little", "trace phải little-endian (D2-40)")
    fields = tr.get("fields", [])
    want = [("t_ns", 0, 8), ("hart", 8, 2), ("kind", 10, 2), ("tcb", 12, 4), ("a", 16, 8), ("b", 24, 8)]
    got = [(f.get("name"), f.get("offset"), f.get("bytes")) for f in fields]
    check(got == want, f"trace.fields sai offset/bytes: {got}")
    kinds = tr.get("kind", {})
    check([kinds.get(str(i)) for i in range(1, 8)] == ["syscall","ipc","sched","fault","store","run","idle"],
          "trace.kind phải 1..7 đúng tên (D2-40)")
    hdr = tr.get("header", {})
    check(hdr.get("magic") == "VFT1" and hdr.get("version") == 1, "trace.header magic/version sai (D2-40)")

    check(dbg.get("number") == 15, "DebugGetInfo phải là syscall 15 (D2-29)")

    # --- **D2-242 nhóm 2 (2C)**: vùng IPC buffer đóng băng (extra caps) ---
    ipc = doc.get("ipc_buffer") or {}
    for key, want in (
        ("bytes", 4096),
        ("align", 16),
        ("mr_offset", 80),
        ("mr_words", 60),
        ("extra_caps_offset", 560),
        ("extra_caps_max", 4),
        ("extra_caps_dst_offset", 600),
        ("configure_offset", 608),
    ):
        check(ipc.get(key) == want,
              f"[ipc_buffer].{key} phải = {want} (D2-242 nhóm 2), đang {ipc.get(key)!r}")
    check(ipc.get("configure_offset_status") == "[TK]",
          "[ipc_buffer].configure_offset phải giữ [TK] — CHƯA đóng băng (D2-242 nhóm 2)")

    # --- **D2-242 nhóm 3 (3.1A + 3.2A)**: kênh lỗi Configure + luật ô 0 CSpace ---
    cs = doc.get("cspace") or {}
    check(cs.get("slot0") == "CS_SELF", "[cspace].slot0 phải = 'CS_SELF' (D2-242 nhóm 3)")
    check(cs.get("cptr_zero") == "absent", "[cspace].cptr_zero phải = 'absent' (D2-242 nhóm 3)")
    check(cs.get("vspace_zero") == "clear",
          "[cspace].vspace_zero phải = 'clear' (KT-325: vspace? = 0 ⇒ xoá)")
    cfg = (doc.get("method_signature") or {}).get("Tcb.Configure") or {}
    check(cfg.get("errors") == [
        "InvalidCapability", "WrongObjectType", "InsufficientRights", "InvalidArgument", "Busy"
    ], f"Tcb.Configure.errors phải đúng 5 mã D2-242 nhóm 3, đang {cfg.get('errors')!r}")

    # --- **D2-242 nhóm 8 (8C)**: object_size = MỘT model + 3 alias bằng nhau ---
    osz = doc.get("object_size") or {}
    model = osz.get("model") or {}
    check(isinstance(model, dict) and bool(model),
          "[object_size].model phải là bộ cỡ object (D2-242 nhóm 8)")
    arches = ("riscv64", "aarch64", "x86_64")
    for arch in arches:
        alias = osz.get(arch) or {}
        check(set(alias) == {"same_as"},
              f"[object_size].{arch} phải là alias `same_as` (D2-242 nhóm 8), đang {sorted(alias)}")
        check(alias.get("same_as") == "model",
              f"[object_size].{arch}.same_as phải = 'model', đang {alias.get('same_as')!r}")
    resolved = {}
    for arch in arches:
        entry = osz.get(arch) or {}
        resolved[arch] = model if entry.get("same_as") == "model" else osz.get(entry.get("same_as"))
    check(resolved.get("riscv64") == model and resolved.get("aarch64") == model
          and resolved.get("x86_64") == model,
          "ba arch phải cùng một bộ cỡ `model` (D2-242 nhóm 8)")

    # --- KT-05 bước 0: chữ ký method trong spec, stable-draft (D2-33/D2-35)
    sig = doc.get("method_signature") or {}
    check(sig.get("stability") == "stable-draft",
          "thiếu [method_signature].stability = 'stable-draft' (KT-05 bước 0)")
    # KT-67b: **không** pin số viết tay (pin 13 cũ đã lỗi thời khi spec mở theo D2-99).
    # Số kỳ vọng **suy ra** từ spec: mọi khoá `[method_signature."X.Y"]` phải là một entry có
    # `params` (đúng quy ước D2-33); so **tập** để bắt cả entry thiếu `params` lẫn entry lạ.
    sig_keys = sorted(k for k in sig if isinstance(k, str) and "." in k)
    methods = {k: v for k, v in sig.items() if isinstance(v, dict) and "params" in v}
    check(sorted(methods) == sig_keys,
          "mọi [method_signature.\"X.Y\"] phải có `params` (spec có "
          f"{len(sig_keys)} khoá, thấy {len(methods)} chữ ký)")
    # Nguồn thứ hai (cùng pipeline): số phần tử của `METHOD_SIGNATURES` do `vifast-gen` sinh.
    # Gen-check so **byte**; đây so **số lượng** để máy kiểm ABI tự đứng một mình vẫn có răng.
    generated = (ROOT / "v2/lib/vifast-abi/src/generated.rs")
    if generated.exists():
        text = generated.read_text(encoding="utf-8")
        marker = "METHOD_SIGNATURES:"
        line = next((ln for ln in text.splitlines() if marker in ln), "")
        count = ""
        for part in line.split(";"):
            digits = "".join(ch for ch in part if ch.isdigit())
            if digits:
                count = digits
        check(count != "" and int(count) == len(sig_keys),
              f"generated.rs phải có đúng {len(sig_keys)} chữ ký (METHOD_SIGNATURES), đang {count or '?'}")
    objs = doc.get("object", {})
    for name, entry in methods.items():
        obj, _, meth = name.partition(".")
        check(obj in objs, f"chữ ký {name}: object {obj!r} không có trong [object]")
        if obj in objs:
            names = {m.get("name") for m in (objs[obj].get("method") or {}).values()}
            check(meth in names, f"chữ ký {name}: method {meth!r} không có trong [object.{obj}]")
    if "Tcb.ReadState" in methods:
        check(methods["Tcb.ReadState"].get("returns") ==
              ["state", "suspended", "blocked_on_kind", "exit_code"],
              "Tcb.ReadState phải trả 4 giá trị (D2-33)")
    if "Untyped.Retype" in methods:
        check("type" in methods["Untyped.Retype"].get("params", []),
              "Untyped.Retype phải có tham số 'type' (D2-33)")
    # D2-40: [trace] là nguồn sự thật cho bản ghi 32 B
    tr = doc.get("trace") or {}
    check(tr.get("record_bytes") == 32, "[trace].record_bytes phải 32 (D2-40)")
    check(tr.get("endianness") == "little", "[trace].endianness phải little (D2-40)")
    fields = tr.get("fields") or []
    check(len(fields) == 6, f"[trace] phải có 6 trường, đang {len(fields)}")
    total = 0
    for f in fields:
        check(f.get("offset") == total, f"[trace] {f.get('name')} offset {f.get('offset')} phải {total}")
        total += f.get("bytes", 0)
    check(total == 32, f"[trace] tổng bytes phải 32, đang {total}")
    check(len(tr.get("kind") or {}) == 7, "[trace].kind phải đủ 7 (1..7)")
    check(len((sig.get("state") or {}).get("names", [])) == 11,
          "enum TCB phải đủ 11 trạng thái (D2-33)")
    check(dbg.get("status") == "frozen", "DebugGetInfo đã chốt ⇒ status frozen (D2-29)")
    # **D2-242 nhóm 8 (8C)** — `return_channel` khai theo **VAI TRÒ**; tên thanh ghi suy từ
    # `[calling.<arch>]` (`return_value` + `mr_registers[i]`) cho **cả ba** arch.
    rc = dbg.get("return_channel", {})
    check(rc.get("value") == "abi_version",
          f"DebugGetInfo return_channel.value phải 'abi_version', đang {rc.get('value')!r}")
    check(rc.get("mr") == ["min_compat", "features", "num_harts"],
          f"DebugGetInfo return_channel.mr phải ['min_compat','features','num_harts'], đang {rc.get('mr')!r}")
    check([rc.get("value"), *(rc.get("mr") or [])] == dbg.get("returns", []),
          "return_channel phải phủ đúng `returns` theo thứ tự (value + mr)")
    roles = rc.get("mr") or []
    calling = doc.get("calling") or {}
    for arch in ("riscv64", "aarch64", "x86_64"):
        entry = calling.get(arch) or {}
        value_reg = entry.get("return_value")
        mr_regs = (entry.get("mr_registers") or [])[: len(roles)]
        check(isinstance(value_reg, str) and bool(value_reg),
              f"{arch}: thiếu return_value — không suy được kênh DebugGetInfo")
        check(len(mr_regs) == len(roles),
              f"{arch}: mr_registers phải có ≥ {len(roles)} thanh ghi cho DebugGetInfo")
        check(value_reg not in mr_regs,
              f"{arch}: kênh value {value_reg!r} trùng MR của DebugGetInfo — hai kênh không cùng thanh ghi")

    # --- object lõi ---------------------------------------------------
    objs = doc.get("object", {})
    # --- id object/method: KHÔNG trùng, đúng dải (lỗ review #5 bắt được) ---
    ids = {}
    for oid_name, obj in objs.items():
        oid = obj.get("id")
        check(isinstance(oid, int), f"object {oid_name} thiếu id số nguyên")
        if not isinstance(oid, int):
            continue
        check(1 <= oid <= 31, f"object {oid_name} id={oid} ngoài dải 1..31")
        check(oid not in ids, f"object id {oid} trùng giữa {ids.get(oid)} và {oid_name}")
        ids[oid] = oid_name
    for oname, obj in objs.items():
        mids = {}
        for key, m in obj.get("method", {}).items():
            mid = int(key)
            check(0 <= mid < (1 << 20), f"{oname}.method[{mid}] vượt label 20 bit")
            check(mid not in mids, f"{oname}.method id {mid} trùng")
            mids[mid] = True
            if isinstance(m, dict):
                check(bool(m.get("name")), f"{oname}.method[{mid}] thiếu name")

    for name in CORE_OBJECTS:
        check(name in objs, f"thiếu object lõi {name}")
    for name, want in CORE_IDS.items():
        got = objs.get(name, {}).get("id")
        check(got == want, f"{name}.id phải = {want} (D2-32), đang {got}")
    # **D2-242 nhóm 9 (9B)** — trạng thái ba object VM (11 object lõi, 5 reserved).
    for name, want_stability in (("VSpace", "core"), ("PageTable", "core"), ("Frame", "frozen")):
        entry = objs.get(name) or {}
        check(entry.get("stability") == want_stability,
              f"{name}.stability phải = {want_stability!r} (D2-242 nhóm 9), đang {entry.get('stability')!r}")
        check(entry.get("unsupported") is not True,
              f"{name} KHÔNG còn `unsupported = true` ở cấp object (D2-242 nhóm 9)")
    # **D2-242 nhóm 9 (9B) + KT-417 hardening F5**: method chưa cài phải giữ `unsupported`,
    # đúng mốc `install_at`. `Tcb.Exec` (id 16, `install_at = "KT-420"`) trước đây **rơi khỏi
    # lưới** vì danh sách chỉ có 3 object VM ⇒ ca "Exec thành cài" vẫn xanh.
    for obj_name, missing, install_at in (
        ("VSpace", ["GetAsid"], "Chu-ky-3"),
        ("PageTable", ["Map", "Unmap"], "Chu-ky-3"),
        ("Frame", ["GetAddress", "MapIo", "UnmapIo"], "Chu-ky-3"),
        ("Tcb", ["Exec"], "KT-420"),
    ):
        by_name = {
            m.get("name"): m
            for m in ((objs.get(obj_name) or {}).get("method") or {}).values()
            if isinstance(m, dict)
        }
        for meth in missing:
            m = by_name.get(meth)
            check(m is not None, f"{obj_name}.{meth} phải có trong [object.{obj_name}]")
            if m is None:
                continue
            check(m.get("unsupported") is True,
                  f"{obj_name}.{meth} chưa cài ⇒ phải unsupported = true")
            check(m.get("install_at") == install_at,
                  f"{obj_name}.{meth} phải install_at = {install_at!r} "
                  f"(D2-242 nhóm 9 / D2-261 nhóm 4), đang {m.get('install_at')!r}")
    if "Tcb" in objs:
        methods = objs["Tcb"].get("method", {})
        got = [v.get("name") if isinstance(v, dict) else v for _, v in sorted(methods.items(), key=lambda kv: int(kv[0]))]
        check(got == TCB_METHODS, f"Tcb.method phải đúng danh sách thiết kế; đang {got}")
    if "CNode" in objs:
        methods = objs["CNode"].get("method", {})
        got = [v.get("name") if isinstance(v, dict) else v for _, v in sorted(methods.items(), key=lambda kv: int(kv[0]))]
        check(got == CNODE_METHODS, f"CNode.method sai; đang {got}")

    # --- **D2-261 nhóm 8 (8A)**: `[boot_caps]` ↔ `proc_abi.rs` + artifact sinh ---
    check_boot_caps(doc, proc_abi)

    # --- MessageInfo + MR + receive window ---------------------------
    mi = doc.get("message_info", {})
    check(mi.get("bits") == 35, "MessageInfo 35 bit (thiết kế §4.6: u20+u3+u1+u4+u7)")
    check(mi.get("label_bits", 0) > 0, "MessageInfo phải có label_bits")
    check(mi.get("length_bits", 0) > 0, "MessageInfo phải có length_bits")
    msg = doc.get("message", {})
    check(msg.get("max_words") == 64, f"message.max_words phải = 64 (QD §3.13), đang {msg.get('max_words')}")
    check(msg.get("max_caps") == 4, f"message.max_caps phải = 4 (QD §3.13), đang {msg.get('max_caps')}")
    check(msg.get("words_in_registers") == 4, "4 MR trong thanh ghi (quy ước v1)")

    # --- quy ước gọi riscv64 -----------------------------------------
    call = doc.get("calling", {}).get("riscv64", {})
    check(call.get("syscall_number") == "a7", "riscv64: số syscall phải ở a7")
    check(call.get("return_error") == "a0", "riscv64: trả lỗi ở a0")
    check(call.get("return_value") == "a1", "riscv64: trả value ở a1")
    check(call.get("args") == ["a0", "a1"], "riscv64: 2 tham số ở a0/a1 (đúng quy ước v1)")
    check(call.get("message_info") == "a2", "riscv64: MessageInfo ở a2 (đúng quy ước v1)")
    check(call.get("mr_registers") == ["a3", "a4", "a5", "a6"],
          "riscv64: MR0..MR3 ở a3..a6 (4 MR, đúng quy ước v1)")
    check(bool(call.get("ipc_buffer")), "riscv64: phải khai báo ipc_buffer (receive window)")
    # **KT-269 / D2-206**: `[calling.aarch64]` đã MỞ — kiểm **đối xứng** từng trường với
    # riscv64 (cùng vai trò, chỉ đổi tên thanh ghi + cơ chế chuyển `svc #0`). Ba trường
    # `trap`/`stack`/`clobbered` là bổ sung tường minh của aarch64 (riscv64 đóng băng).
    a64 = doc.get("calling", {}).get("aarch64", {})
    check(a64.get("reserved") is not True, "aarch64: đã mở (D2-206) ⇒ KHÔNG còn reserved = true")
    check(a64.get("syscall_number") == "x8", "aarch64: số syscall phải ở x8")
    check(a64.get("return_error") == "x0", "aarch64: trả lỗi ở x0")
    check(a64.get("return_value") == "x1", "aarch64: trả value ở x1")
    check(a64.get("args") == ["x0", "x1"], "aarch64: 2 tham số ở x0/x1 (đối xứng a0/a1)")
    check(a64.get("message_info") == "x2", "aarch64: MessageInfo ở x2 (đối xứng a2)")
    check(a64.get("mr_registers") == ["x3", "x4", "x5", "x6"],
          "aarch64: MR0..MR3 ở x3..x6 (đối xứng a3..a6)")
    check(a64.get("trap") == "svc #0", "aarch64: cơ chế chuyển phải là 'svc #0' (thay ecall)")
    check(bool(a64.get("stack")), "aarch64: phải khai báo quy ước ngăn xếp (SP_EL0/SP_EL1)")
    check(bool(a64.get("clobbered")), "aarch64: phải khai báo thanh ghi bị clobber")
    check(bool(a64.get("ipc_buffer")), "aarch64: phải khai báo ipc_buffer (receive window)")
    check(bool(a64.get("preserved")), "aarch64: phải khai báo preserved")
    check(bool(a64.get("note")), "aarch64: phải có note giải thích")
    # Đối xứng **cấu trúc**: cùng tập khoá với riscv64 + đúng ba trường bổ sung.
    extra = {"trap", "stack", "clobbered"}
    check(set(a64) == set(call) | extra,
          "aarch64: trường phải đối xứng riscv64 + {trap,stack,clobbered}; lệch "
          f"{sorted(set(a64) ^ (set(call) | extra))}")
    # **KT-270 / D2-207**: `[calling.x86_64]` đã MỞ — kiểm **đối xứng** từng trường với riscv64
    # (cùng vai trò, chỉ đổi tên thanh ghi + cơ chế chuyển `int 0x80`). Ba trường
    # `trap`/`stack`/`clobbered` là bổ sung tường minh (như aarch64).
    x64 = doc.get("calling", {}).get("x86_64", {})
    check(x64.get("reserved") is not True, "x86_64: đã mở (D2-207) ⇒ KHÔNG còn reserved = true")
    check(x64.get("syscall_number") == "rax", "x86_64: số syscall phải ở rax")
    check(x64.get("return_error") == "rax", "x86_64: trả lỗi ở rax")
    check(x64.get("return_value") == "rdx", "x86_64: trả value ở rdx")
    check(x64.get("args") == ["rdi", "rsi"], "x86_64: 2 tham số ở rdi/rsi (đối xứng a0/a1)")
    check(x64.get("message_info") == "rdx", "x86_64: MessageInfo ở rdx (đối xứng a2)")
    check(x64.get("mr_registers") == ["r10", "r8", "r9", "r12"],
          "x86_64: MR0..MR3 ở r10/r8/r9/r12 (đối xứng a3..a6)")
    check(x64.get("trap") == "int 0x80", "x86_64: cơ chế chuyển phải là 'int 0x80' (D2-207)")
    check(bool(x64.get("stack")), "x86_64: phải khai báo quy ước ngăn xếp (RSP user/TSS RSP0)")
    check(bool(x64.get("clobbered")), "x86_64: phải khai báo thanh ghi bị clobber")
    check(bool(x64.get("ipc_buffer")), "x86_64: phải khai báo ipc_buffer (receive window)")
    check(bool(x64.get("preserved")), "x86_64: phải khai báo preserved")
    check(bool(x64.get("note")), "x86_64: phải có note giải thích")
    # Đối xứng **cấu trúc**: cùng tập khoá với riscv64 + ba trường bổ sung + **`return_message_info`**
    # (D2-242 nhóm 7 = 7C-r13 — riscv64/aarch64 tái dùng thanh ghi `message_info` làm kênh trả,
    # x86_64 không thể vì `rdx` đã là `return_value`).
    extra_x64 = extra | {"return_message_info"}
    check(set(x64) == set(call) | extra_x64,
          "x86_64: trường phải đối xứng riscv64 + {trap,stack,clobbered,return_message_info}; lệch "
          f"{sorted(set(x64) ^ (set(call) | extra_x64))}")
    # **7C-r13**: MessageInfo trả về ở `r13` (KHÔNG phải r11 — CPU ghi RFLAGS vào r11 khi bật
    # `syscall`/`sysret` ở Chu kỳ 3), khác `message_info`/`return_value`, và phải nằm trong clobbered.
    check(x64.get("return_message_info") == "r13",
          f"x86_64: return_message_info phải = 'r13' (7C-r13), đang {x64.get('return_message_info')!r}")
    check(x64.get("return_message_info") != x64.get("message_info"),
          "x86_64: return_message_info phải KHÁC message_info (7C-r13)")
    check(x64.get("return_message_info") != x64.get("return_value"),
          "x86_64: return_message_info phải KHÁC return_value (7C-r13)")
    want_clobbered = ["rax", "rdx", "r13", "r10", "r8", "r9", "r12"]
    check(x64.get("clobbered") == want_clobbered,
          f"x86_64: clobbered phải {want_clobbered} (7C-r13), đang {x64.get('clobbered')!r}")
    check(x64.get("preserved") == ["rsp", "rbx", "rbp", "r14", "r15"],
          f"x86_64: preserved phải rsp/rbx/rbp/r14/r15 (7C-r13), đang {x64.get('preserved')!r}")
    for reg in [x64.get("return_error"), x64.get("return_value"), x64.get("return_message_info"),
                *(x64.get("mr_registers") or [])]:
        check(reg in (x64.get("clobbered") or []),
              f"x86_64: {reg!r} là kênh ra/MR ⇒ phải nằm trong clobbered (7C-r13)")
    for reg in x64.get("preserved") or []:
        check(reg not in (x64.get("clobbered") or []),
              f"x86_64: {reg!r} vừa preserved vừa clobbered (7C-r13)")
    # Vai trò từng trường: 2 tham số, 4 MR, error/value khác nhau, MR không trùng tham số.
    check(len(x64.get("args") or []) == 2, "x86_64: phải có đúng 2 thanh ghi tham số")
    check(len(x64.get("mr_registers") or []) == 4, "x86_64: phải có đúng 4 MR")
    check(x64.get("return_error") != x64.get("return_value"),
          "x86_64: error và value phải ở hai thanh ghi khác nhau")
    for reg in x64.get("mr_registers") or []:
        check(reg not in (x64.get("args") or []),
              f"x86_64: MR {reg} không được trùng thanh ghi tham số")
    # Nguồn thứ hai (cùng pipeline): `vifast-gen` phải sinh hằng cho **cả ba** arch đã mở.
    if generated.exists():
        text = generated.read_text(encoding="utf-8")
        check('CALLING_ARCHES: [&str; 3] = ["aarch64", "riscv64", "x86_64"]' in text,
              "generated.rs: CALLING_ARCHES phải có đủ aarch64/riscv64/x86_64")
        for const, want in (
            ("X86_64_SYSCALL_NUMBER_REG", "rax"),
            ("X86_64_MESSAGE_INFO_REG", "rdx"),
            ("X86_64_RETURN_ERROR_REG", "rax"),
            ("X86_64_RETURN_VALUE_REG", "rdx"),
            ("X86_64_TRAP", "int 0x80"),
        ):
            check(f'pub const {const}: &str = "{want}";' in text,
                  f"generated.rs: thiếu {const} = {want!r}")
        check('pub const X86_64_MR_REGS: [&str; 4] = ["r10", "r8", "r9", "r12"];' in text,
              "generated.rs: X86_64_MR_REGS phải là r10/r8/r9/r12")
        # D2-242 nhóm 7: hằng `MessageInfo` trả về = r13 + clobbered đủ 7 + preserved 5.
        check('pub const X86_64_RETURN_MESSAGE_INFO_REG: &str = "r13";' in text,
              "generated.rs: X86_64_RETURN_MESSAGE_INFO_REG phải = 'r13' (7C-r13)")
        check('pub const X86_64_CLOBBERED_REGS: [&str; 7] = ["rax", "rdx", "r13", "r10", "r8", "r9", "r12"];' in text,
              "generated.rs: X86_64_CLOBBERED_REGS phải đủ 7 thanh ghi (7C-r13)")
        check('pub const X86_64_PRESERVED_REGS: [&str; 5] = ["rsp", "rbx", "rbp", "r14", "r15"];' in text,
              "generated.rs: X86_64_PRESERVED_REGS phải 5 thanh ghi (7C-r13)")
        # D2-242 nhóm 2: hằng IPC buffer.
        for const, want in (
            ("IPC_BUFFER_BYTES", 4096), ("IPC_BUFFER_ALIGN", 16),
            ("IPC_BUFFER_MR_OFFSET", 80), ("IPC_BUFFER_MR_WORDS", 60),
            ("IPC_BUFFER_EXTRA_CAPS_OFFSET", 560), ("IPC_BUFFER_EXTRA_CAPS_MAX", 4),
            ("IPC_BUFFER_EXTRA_CAPS_DST_OFFSET", 600), ("IPC_BUFFER_CONFIGURE_OFFSET", 608),
        ):
            check(f"pub const {const}: usize = {want};" in text,
                  f"generated.rs: thiếu {const} = {want} (D2-242 nhóm 2)")
        # D2-242 nhóm 8: kênh DebugGetInfo suy từ [calling.*].
        check('pub const DEBUG_INFO_RETURN_VALUE_ROLE: &str = "abi_version";' in text,
              "generated.rs: thiếu DEBUG_INFO_RETURN_VALUE_ROLE")
        check('pub const DEBUG_INFO_RETURN_MR_ROLES: [&str; 3] = ["min_compat", "features", "num_harts"];' in text,
              "generated.rs: thiếu DEBUG_INFO_RETURN_MR_ROLES")
        for const, want in (
            ("RISCV64_DEBUG_INFO_RETURN_VALUE_REG", "a1"),
            ("AARCH64_DEBUG_INFO_RETURN_VALUE_REG", "x1"),
            ("X86_64_DEBUG_INFO_RETURN_VALUE_REG", "rdx"),
        ):
            check(f'pub const {const}: &str = "{want}";' in text,
                  f"generated.rs: {const} phải = {want!r} (suy từ [calling.*])")
        # D2-242 nhóm 9: 11 object lõi, 5 reserved.
        check("pub const CORE_OBJECTS: [(&str, u16); 11]" in text,
              "generated.rs: CORE_OBJECTS phải 11 object (D2-242 nhóm 9)")
        check("pub const RESERVED_OBJECTS: [(&str, u16); 5]" in text,
              "generated.rs: RESERVED_OBJECTS phải còn 5 object (D2-242 nhóm 9)")

    # --- kích thước/căn lề -------------------------------------------
    slot = doc.get("object_size", {}).get("slot", {})
    check(slot.get("bytes") == 48, f"slot cap phải 48 B [TK] (QD §3.9), đang {slot.get('bytes')}")
    check(slot.get("align_bytes", 0) > 0, "slot phải có align_bytes")

    if fails:
        for msg in fails:
            print(f"not ok - {msg}")
        print(f"# {len(fails)} mục sai trong {path}")
        return 1
    print(f"abi-check: PASS ({path})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
