#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-04b — kiểm phần object reserved của ABI: số không trùng + `NotSupported`.

Hợp đồng lấy từ thiết kế §4.21 (bảng object) và QD §1 Q-B ("object chưa cài chỉ giữ
số reserved, method chưa cài trả NotSupported"). Viết TRƯỚC khi viết bảng reserved
để "đỏ trước".

Dùng: `python3 check_reserved.py [--file spec/abi.toml]`
"""

import sys

SPDX = "SPDX-License-Identifier"

# Object reserved theo đúng thứ tự thiết kế §4.21 (sau Reply), kèm method mong đợi.
RESERVED = {
    "VSpace": ["GetAsid"],                       # method id 1 TRỐNG: VSpace khách = bit GUEST (D2-32)
    "PageTable": ["Map", "Unmap"],
    "Frame": ["Map", "Unmap", "GetAddress", "MapIo", "UnmapIo"],
    "IrqControl": ["GetLine", "GetMsi"],
    "IrqHandler": ["SetNotification", "Ack", "Clear", "SetAffinity"],
    "IOSpace": ["Flush", "Attach"],
    "VCpu": ["Run", "ReadRegs", "WriteRegs", "InjectIrq", "SetTsc", "Configure"],
    "PowerControl": ["HartOffline", "HartOnline", "Suspend"],
}

ID_FROM = 9
ID_TO = 16

fails = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def method_names(obj):
    methods = obj.get("method", {})
    return [v.get("name") if isinstance(v, dict) else v
            for _, v in sorted(methods.items(), key=lambda kv: int(kv[0]))]


def main():
    path = "spec/abi.toml"
    args = sys.argv[1:]
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

    # 1) đủ 8 object reserved (9..16) theo thiết kế
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
