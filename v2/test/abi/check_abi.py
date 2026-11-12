#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-04a — kiểm `spec/abi.toml` (nguồn sự thật ABI, D2-11) bằng máy.

Bộ kiểm này là **hợp đồng** lấy từ QD §4 và thiết kế §4.20/§4.21/§3.9; nó cố ý
viết TRƯỚC abi.toml để "đỏ trước": file chưa có / thiếu mục ⇒ exit 1.

Dùng: `python3 check_abi.py [--file spec/abi.toml]`  (cần Python ≥3.11 có tomllib)
"""

import sys

SPDX = "SPDX-License-Identifier"

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

CORE_OBJECTS = [
    "Untyped",
    "SchedControl",
    "CNode",
    "Tcb",
    "SchedContext",
    "Endpoint",
    "Notification",
    "Reply",
]

CORE_IDS = {
    "Untyped": 1, "CNode": 2, "Tcb": 3, "SchedContext": 4,
    "Endpoint": 5, "Notification": 6, "Reply": 7, "SchedControl": 8,
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
]

CNODE_METHODS = ["Copy", "Mint", "Move", "Mutate", "Delete", "Revoke", "Rotate"]

fails = []


def check(cond, msg):
    if not cond:
        fails.append(msg)


def main():
    path = "spec/abi.toml"
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--file":
        path = args[1]

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
    check(dbg.get("number") == 15, "DebugGetInfo phải là syscall 15 (D2-29)")

    # --- KT-05 bước 0: chữ ký method trong spec, stable-draft (D2-33/D2-35)
    sig = doc.get("method_signature") or {}
    check(sig.get("stability") == "stable-draft",
          "thiếu [method_signature].stability = 'stable-draft' (KT-05 bước 0)")
    methods = {k: v for k, v in sig.items() if isinstance(v, dict) and "params" in v}
    check(len(methods) == 13, f"phải có 13 chữ ký method, đang {len(methods)}")
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
    check(len((sig.get("state") or {}).get("names", [])) == 11,
          "enum TCB phải đủ 11 trạng thái (D2-33)")
    check(dbg.get("status") == "frozen", "DebugGetInfo đã chốt ⇒ status frozen (D2-29)")
    rc = dbg.get("return_channel", {})
    check(rc.get("a1") == "abi_version" and rc.get("a3") == "min_compat"
          and rc.get("a4") == "features" and rc.get("a5") == "num_harts",
          "DebugGetInfo return_channel phải là a1/a3/a4/a5 (D2-35)")

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
    if "Tcb" in objs:
        methods = objs["Tcb"].get("method", {})
        got = [v.get("name") if isinstance(v, dict) else v for _, v in sorted(methods.items(), key=lambda kv: int(kv[0]))]
        check(got == TCB_METHODS, f"Tcb.method phải đúng danh sách thiết kế; đang {got}")
    if "CNode" in objs:
        methods = objs["CNode"].get("method", {})
        got = [v.get("name") if isinstance(v, dict) else v for _, v in sorted(methods.items(), key=lambda kv: int(kv[0]))]
        check(got == CNODE_METHODS, f"CNode.method sai; đang {got}")

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
    # 2 arch còn lại chỉ giữ số reserved (D2-10)
    for arch in ("aarch64", "x86_64"):
        got = doc.get("calling", {}).get(arch, {})
        check(got.get("reserved") is True, f"{arch}: chỉ reserved tới K4'/K5' (D2-10)")

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
