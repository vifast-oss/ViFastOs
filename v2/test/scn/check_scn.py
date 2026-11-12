#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-04c — kiểm tĩnh kịch bản `.scn` (chiếu văn bản của thiết kế §9.3).

Đỏ trước: viết khi `v2/test/scn/*.scn` chưa có tệp nào.

Dùng: `python3 check_scn.py [--dir v2/test/scn] [--spec spec/abi.toml]`
"""

import pathlib
import re
import sys

SPDX = "SPDX-License-Identifier"

# 15 syscall của spec, tên snake_case -> tên trong abi.toml
SYSCALL_NAMES = {
    "call": "Call",
    "send": "Send",
    "nb_send": "NBSend",
    "recv": "Recv",
    "nb_recv": "NBRecv",
    "reply_recv": "ReplyRecv",
    "nb_send_recv": "NBSendRecv",
    "nb_send_wait": "NBSendWait",
    "yield": "Yield",
    "wait_addr": "WaitAddr",
    "wake_addr": "WakeAddr",
    "clock_read": "ClockRead",
    "debug_put_str": "DebugPutStr",
    "debug_halt": "DebugHalt",
    "exit": "Exit",
}

# 9 nhóm phủ theo QD §4 (cột "phủ" của bảng 21 kịch bản)
TAG_GROUPS = {
    "cap-copy",
    "cap-revoke",
    "call-reply",
    "reply-deleted",
    "suspend-blocked",
    "kill-blocked",
    "exit",
    "four-cap",
    "no-reply",
    "donation",
}

CAP_KEYS = {"cap", "src", "dst", "cnode", "ep", "reply", "notif", "tcb", "u", "sched"}

fails = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def load_toml(path):
    import tomllib

    raw = pathlib.Path(path).read_bytes()
    return raw, tomllib.loads(raw.decode("utf-8"))


def parse_step(line):
    """Trả (op_kind, op_name, params, expect) hoặc raise ValueError."""
    if "=>" not in line:
        raise ValueError("thiếu `=> <kết quả>`")
    body, expect = line.rsplit("=>", 1)
    expect = expect.strip()
    toks = body.split()
    if not toks:
        raise ValueError("bước rỗng")
    if toks[0] == "invoke":
        if len(toks) < 2 or "." not in toks[1]:
            raise ValueError("invoke phải là `invoke Object.Method`")
        kind, name = "invoke", toks[1]
        rest = toks[2:]
    else:
        kind, name, rest = "syscall", toks[0], toks[1:]
    params = {}
    for tok in rest:
        if "=" not in tok:
            raise ValueError(f"tham số không phải k=v: {tok!r}")
        k, v = tok.split("=", 1)
        params[k] = v
    return kind, name, params, expect


def main():
    scn_dir = pathlib.Path("v2/test/scn")
    spec_path = pathlib.Path("spec/abi.toml")
    draft_path = pathlib.Path("v2/docs/abi-reserved-objects.toml")
    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--dir":
            scn_dir = pathlib.Path(args[i + 1]); i += 2
        elif args[i] == "--spec":
            spec_path = pathlib.Path(args[i + 1]); i += 2
        else:
            print(f"tham số lạ: {args[i]}")
            return 2

    try:
        _, spec = load_toml(spec_path)
    except FileNotFoundError:
        print(f"scn-check: KHÔNG có {spec_path}")
        return 1

    syscall_by_name = {v["name"] for v in spec.get("syscall", {}).values()}
    errors = {v["name"] for v in spec.get("error", {}).values()}
    objects = dict(spec.get("object", {}))
    if draft_path.exists():
        try:
            _, draft = load_toml(draft_path)
            objects.update(draft.get("object", {}))
        except Exception:  # noqa: BLE001 - draft hỏng thì bỏ qua, spec vẫn là chuẩn
            pass

    def method_exists(name):
        if "." not in name:
            return False
        obj_name, meth = name.split(".", 1)
        obj = objects.get(obj_name)
        if not obj:
            return False
        return any((m.get("name") if isinstance(m, dict) else m) == meth
                   for m in obj.get("method", {}).values())

    files = sorted(scn_dir.glob("*.scn")) if scn_dir.exists() else []
    check(len(files) == 21, f"phải có đúng 21 tệp .scn, đang có {len(files)}")

    names, tags_seen, blocked, has_exit_case = set(), set(), [], False

    for path in files:
        try:
            raw, doc = load_toml(path)
        except Exception as exc:  # noqa: BLE001
            fails.append(f"{path.name}: TOML không parse được: {exc}")
            continue

        check(SPDX.encode() in raw, f"{path.name}: thiếu SPDX")
        name = doc.get("name")
        check(name == path.stem, f"{path.name}: name={name!r} phải trùng tên tệp")
        check(doc.get("version") == 1, f"{path.name}: version phải = 1")
        check(name not in names, f"{path.name}: name {name!r} trùng")
        names.add(name)

        if doc.get("blocked"):
            blocked.append((path.name, doc["blocked"]))
            check(not doc.get("steps") and not doc.get("setup"),
                  f"{path.name}: tệp blocked không được có bước")
            continue

        for tag in doc.get("tags", []):
            check(tag in TAG_GROUPS, f"{path.name}: tag lạ {tag!r}")
            tags_seen.add(tag)

        caps = doc.get("caps", {})
        steps = list(doc.get("setup", []))
        for thread in doc.get("thread", []):
            steps.extend(thread.get("steps", []))

        for line in steps:
            try:
                kind, op, params, expect = parse_step(line)
            except ValueError as exc:
                fails.append(f"{path.name}: bước {line!r}: {exc}")
                continue
            if kind == "syscall":
                want = SYSCALL_NAMES.get(op)
                check(want is not None, f"{path.name}: syscall lạ {op!r}")
                if want is not None:
                    check(want in syscall_by_name, f"{path.name}: {op} không có trong spec/abi.toml")
            else:
                check(method_exists(op), f"{path.name}: {op} không có trong spec/abi.toml (hoặc draft)")
            check(expect == "ok" or expect in errors,
                  f"{path.name}: kết quả {expect!r} không phải ok/19 mã lỗi")
            for k, v in params.items():
                if k in CAP_KEYS and v not in caps:
                    fails.append(f"{path.name}: cap {v!r} dùng ở {op} chưa khai trong caps")

        if path.name == "20-exit-procsrv-chua-recv.scn":
            has_exit_case = any(parse_step(l)[0] == "syscall" and parse_step(l)[1] == "exit"
                                for l in steps if "=>" in l)

    check(has_exit_case, "thiếu ca bắt buộc 20-exit-procsrv-chua-recv với bước exit")
    missing = TAG_GROUPS - tags_seen
    check(not missing, f"thiếu nhóm phủ: {sorted(missing)}")

    for name, reason in blocked:
        print(f"# blocked: {name} — {reason}")

    if fails:
        for msg in fails:
            print(f"not ok - {msg}")
        print(f"# {len(fails)} mục sai")
        return 1
    print(f"scn-check: PASS ({len(files)} tệp, {len(blocked)} blocked, {len(tags_seen)} nhóm)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
