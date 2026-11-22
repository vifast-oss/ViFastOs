#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-04b — kiểm phần object reserved của ABI: số không trùng + `NotSupported`.

Hợp đồng lấy từ thiết kế §4.21 (bảng object) và QD §1 Q-B ("object chưa cài chỉ giữ
số reserved, method chưa cài trả NotSupported"). Viết TRƯỚC khi viết bảng reserved
để "đỏ trước".

**D2-242 nhóm 9 (9B)** — `VSpace`/`PageTable`/`Frame` đã nâng lên `core`/`frozen`; object
reserved còn **5** (`12..16`: `IrqControl`, `IrqHandler`, `IOSpace`, `VCpu`, `PowerControl`).

Dùng: `python3 check_reserved.py [--file spec/abi.toml]`
      `python3 check_reserved.py --selftest`
"""

import sys
from pathlib import Path

SPDX = "SPDX-License-Identifier"

# Object reserved theo đúng thứ tự thiết kế §4.21 (sau Reply), kèm method mong đợi.
# **D2-242 nhóm 9 (9B)**: `VSpace`/`PageTable`/`Frame` → `core`/`frozen` (đã cài hoặc là đích
# `Retype`) ⇒ còn **5** object reserved `12..16` (IrqControl..PowerControl).
RESERVED = {
    "IrqControl": ["GetLine", "GetMsi"],
    "IrqHandler": ["SetNotification", "Ack", "Clear", "SetAffinity"],
    "IOSpace": ["Flush", "Attach"],
    "VCpu": ["Run", "ReadRegs", "WriteRegs", "InjectIrq", "SetTsc", "Configure"],
    "PowerControl": ["HartOffline", "HartOnline", "Suspend"],
}

ID_FROM = 12
ID_TO = 16

fails = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def method_names(obj):
    methods = obj.get("method", {})
    return [v.get("name") if isinstance(v, dict) else v
            for _, v in sorted(methods.items(), key=lambda kv: int(kv[0]))]


def selftest() -> int:
    """Ca dương (spec thật xanh) + ca âm (đột biến ⇒ đỏ **đúng lý do**) cho chính máy kiểm."""
    import subprocess
    import tempfile

    base = Path(__file__).resolve().parents[3] / "spec/abi.toml"
    try:
        text = base.read_text(encoding="utf-8")
    except FileNotFoundError:
        print(f"not ok 1 - không có {base}")
        return 1
    tmp = Path(tempfile.mkdtemp(prefix="kt-reserved-st."))
    cases: list[tuple[str, bool, str]] = []

    def run(path: Path) -> tuple[int, str]:
        proc = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--file", str(path)],
            capture_output=True,
            text=True,
        )
        return proc.returncode, proc.stdout + proc.stderr

    rc, out = run(base)
    cases.append(("dương: spec/abi.toml thật xanh, đúng 5 object reserved", rc == 0, out[:300]))

    def mutate(name: str, old: str, new: str, want: str) -> None:
        if old not in text:
            cases.append((name, False, f"không đột biến được (không thấy {old!r})"))
            return
        path = tmp / f"{abs(hash(name))}.toml"
        path.write_text(text.replace(old, new, 1), encoding="utf-8")
        rc, out = run(path)
        cases.append((name, rc == 1 and want in out, f"muốn rc=1 + {want!r}, nhận rc={rc}: {out[:300]}"))

    mutate(
        "âm: IrqControl stability → core ⇒ thiếu object reserved",
        'IrqControl = { id = 12, stability = "reserved"',
        'IrqControl = { id = 12, stability = "core"',
        "thiếu object reserved IrqControl",
    )
    mutate(
        "âm: IrqControl id 12 → 17 ⇒ ngoài dải reserved",
        'IrqControl = { id = 12,',
        'IrqControl = { id = 17,',
        "ngoài dải reserved",
    )
    mutate(
        "âm: method IOSpace.Attach mất unsupported ⇒ phải NotSupported",
        '1 = { name = "Attach", unsupported = true }',
        '1 = { name = "Attach" }',
        "Attach) phải unsupported = true",
    )
    mutate(
        "âm: object_reserved 17..31 → 18..31",
        "from = 17\nto = 31",
        "from = 18\nto = 31",
        "object_reserved phải 17..31",
    )

    for child in tmp.iterdir():
        child.unlink()
    tmp.rmdir()

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
    print(f"# selftest: {len(cases)}/{len(cases)} ca đúng như kỳ vọng (1 dương + {len(cases) - 1} âm)")
    return 0


def main():
    path = "spec/abi.toml"
    args = sys.argv[1:]
    if args == ["--selftest"]:
        return selftest()
    if len(args) == 2 and args[0] == "--file":
        path = args[1]

    try:
        with open(path, "rb") as fh:
            raw = fh.read()
    except FileNotFoundError:
        print(f"reserved-check: KHÔNG có {path}")
        return 1

    try:
        import tomllib
    except ModuleNotFoundError:
        print("reserved-check: cần Python ≥3.11 (tomllib)")
        return 2

    try:
        doc = tomllib.loads(raw.decode("utf-8"))
    except Exception as exc:  # noqa: BLE001
        print(f"reserved-check: TOML không parse được: {exc}")
        return 1

    check(SPDX.encode() in raw, "thiếu dòng SPDX-License-Identifier")

    objs = doc.get("object", {})
    core = {n: o for n, o in objs.items() if o.get("stability") in ("frozen", "core")}
    reserved = {n: o for n, o in objs.items() if o.get("stability") == "reserved"}
    if not objs:
        print(f"reserved-check: {path} không có [object] nào")
        return 1

    # 1) đủ **5** object reserved (12..16) theo D2-242 nhóm 9
    for name in RESERVED:
        check(name in reserved, f"thiếu object reserved {name}")

    # 2) method đúng danh sách + đánh số từ 0 liền mạch
    for name, want in RESERVED.items():
        obj = reserved.get(name)
        if obj is None:
            continue
        got = method_names(obj)
        check(got == want, f"{name}.method sai: muốn {want}, đang {got}")
        ids = sorted(int(k) for k in obj.get("method", {}))
        check(ids == list(range(len(want))), f"{name}.method phải đánh số 0..{len(want)-1}, đang {ids}")

    # 3) object id không trùng, nằm trong dải reserved
    all_ids = {}
    for name, obj in objs.items():
        oid = obj.get("id")
        check(isinstance(oid, int), f"{name} thiếu id số nguyên")
        if not isinstance(oid, int):
            continue
        check(oid not in all_ids, f"object id {oid} trùng giữa {all_ids.get(oid)} và {name}")
        all_ids[oid] = name
    for name in RESERVED:
        if name not in reserved:
            continue
        oid = reserved[name].get("id")
        if isinstance(oid, int):
            check(ID_FROM <= oid <= ID_TO, f"{name}.id={oid} ngoài dải reserved {ID_FROM}..{ID_TO}")

    # 4) mọi entry reserved phải trả NotSupported
    for name, obj in reserved.items():
        check(obj.get("unsupported") is True, f"{name} phải có unsupported = true (trả NotSupported)")
        for key, m in obj.get("method", {}).items():
            if isinstance(m, dict):
                check(m.get("unsupported") is True,
                      f"{name}.method[{key}] ({m.get('name')}) phải unsupported = true")

    # 5) nếu tệp có cả core (bản đầy đủ) thì Reply phải = 7
    if core:
        check(core.get("Reply", {}).get("id") == 7, "core phải giữ Reply = 7 (KT-04a)")
    if core:
        check(core.get("SchedControl", {}).get("id") == 8, "SchedControl phải là object LÕI id 8 (D2-32)")
        check("SchedControl" not in reserved, "SchedControl KHÔNG được vừa reserved vừa có chữ ký (D2-32)")

    # 7) (bỏ) khối so draft cũ: đường dẫn tự tính không tồn tại nên không bao giờ chạy —
    #    KT-04b đã hợp nhất draft vào spec/abi.toml (D2-32), spec là nguồn sự thật duy nhất.
    import os

    # 6) dải reserved của ABI vẫn bao trùm id đã dùng
    rng = doc.get("object_reserved", {})
    check(rng.get("from") == 17 and rng.get("to") == 31, "object_reserved phải 17..31 (D2-32)")

    if fails:
        for msg in fails:
            print(f"not ok - {msg}")
        print(f"# {len(fails)} mục sai trong {path}")
        return 1
    print(f"reserved-check: PASS ({len(reserved)} object reserved, {path})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
