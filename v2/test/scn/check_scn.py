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
    "debug_get_info": "DebugGetInfo",
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
    "notification",
}

CAP_KEYS = {"cap", "src", "dst", "cnode", "ep", "reply", "notif", "tcb", "u", "sched", "sched_control"}

fails = []


def split_fields(body):
    """Tách `a, b=c, d=E{1}` theo dấu phẩy ở mức ngoài cùng."""
    out, depth, cur = [], 0, ""
    for ch in body:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def check_state(fname, st, sig, state_names):
    """Kiểm `Name` hoặc `Name{field=value,...}` theo enum + field của draft D2-33."""
    if "{" in st:
        name, body = st.split("{", 1)
        body = body.rsplit("}", 1)[0]
    else:
        name, body = st, ""
    name = name.strip()
    if name not in state_names:
        fails.append(f"{fname}: trạng thái {name!r} không có trong enum D2-33")
        return
    declared = {f.split(":")[0].strip() for f in (sig.get("state", {}).get(name, {}) or {}).get("fields", [])}
    for field in split_fields(body) if body else []:
        key = field.split("=")[0].strip()
        if declared and key not in declared:
            fails.append(f"{fname}: {name} không có trường {key!r} (D2-33: {sorted(declared)})")


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
    sig_path = pathlib.Path("v2/docs/abi-call-signatures.toml")
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

    # D2-33: chữ ký method + enum trạng thái lấy từ draft (chưa vào spec/)
    sig = {}
    if sig_path.exists():
        try:
            _, sig = load_toml(sig_path)
        except Exception as exc:  # noqa: BLE001
            print(f"scn-check: draft chữ ký không parse được: {exc}")
            return 1
    sig_methods = sig.get("method", {})
    state_names = set((sig.get("state", {}) or {}).get("names", []) or [])

    def signature_ok(op, params):
        """Trả (ok, lý do). Kiểm đúng chữ ký draft: đủ tham số bắt buộc, không tham số lạ."""
        entry = sig_methods.get(op)
        if entry is None:
            return False, f"method {op} không có chữ ký trong draft D2-33"
        declared = entry.get("params", [])
        required = [x for x in declared if not x.endswith("?")]
        optional = [x[:-1] for x in declared if x.endswith("?")]
        missing = [x for x in required if x not in params]
        if missing:
            return False, f"{op} thiếu tham số bắt buộc {missing}"
        unknown = [k for k in params if k not in required and k not in optional]
        if unknown:
            return False, f"{op} có tham số lạ {unknown}"
        for key, allowed in (entry.get("variants") or {}).items():
            if key in params and params[key] not in allowed:
                return False, f"{op}.{key}={params[key]!r} không thuộc {allowed}"
        return True, ""

    # D2-11: nếu spec đã có chuỗi `args` cho một method thì draft không được nói khác
    def spec_params(obj_name, meth_name):
        obj = objects.get(obj_name) or {}
        for m in (obj.get("method") or {}).values():
            if isinstance(m, dict) and m.get("name") == meth_name and m.get("args"):
                return [t.split(":")[0].strip() for t in m["args"].split(",")]
        return None

    for op, entry in sig_methods.items():
        if "." not in op:
            continue
        obj_name, meth = op.split(".", 1)
        have = spec_params(obj_name, meth)
        if have is None:
            continue
        draft = [x.rstrip("?") for x in entry.get("params", [])]
        for param in have:
            if param not in draft:
                fails.append(f"draft lệch spec: {op} có {param!r} trong spec nhưng không có trong draft")

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

        for st in doc.get("expect_states", []) or []:
            check_state(path.name, st, sig, state_names)
        _ = 0

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
                ok, why = signature_ok(op, params)
                check(ok, f"{path.name}: {why}")
            if expect == "halt":
                check(kind == "syscall" and op in ("exit", "debug_halt"),
                      f"{path.name}: `halt` chỉ dùng cho exit/debug_halt")
            else:
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

    check(not blocked, f"DoD KT-04c: phải 21/21 tệp không blocked, còn {len(blocked)}")
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
