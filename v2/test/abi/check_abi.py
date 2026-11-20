#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-04a — kiểm `spec/abi.toml` (nguồn sự thật ABI, D2-11) bằng máy.

Bộ kiểm này là **hợp đồng** lấy từ QD §4 và thiết kế §4.20/§4.21/§3.9; nó cố ý
viết TRƯỚC abi.toml để "đỏ trước": file chưa có / thiếu mục ⇒ exit 1.

Dùng: `python3 check_abi.py [--file spec/abi.toml]`  (cần Python ≥3.11 có tomllib)
"""

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
    # x86_64 **vẫn** chỉ giữ số reserved (D2-10) — chưa mở.
    x64 = doc.get("calling", {}).get("x86_64", {})
    check(x64.get("reserved") is True, "x86_64: chỉ reserved tới K5' (D2-10)")

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
