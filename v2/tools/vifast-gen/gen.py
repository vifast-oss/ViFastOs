#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""`vifast-gen` — sinh code Rust + `docs/v2/abi.md` TỪ `spec/abi.toml` (KT-05, D2-11).

Không chép tay: mọi bảng lấy từ spec. `--check` sinh vào thư mục tạm rồi so BYTE với tệp đã commit;
lệch ⇒ exit 1 (sửa tay bị bắt). Cần Python ≥3.11 (tomllib).
"""
from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path


def die(msg: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"vifast-gen: {msg}", file=sys.stderr)
    raise SystemExit(1)


def load(spec: Path) -> dict:
    try:
        with spec.open("rb") as fh:
            doc = tomllib.load(fh)
    except FileNotFoundError:
        die(f"không có {spec}")
    except tomllib.TOMLDecodeError as exc:
        die(f"{spec}: TOML hỏng ({exc})")
    return doc


def gen_rs(doc: dict) -> str:
    syscalls = {int(k): v for k, v in doc.get("syscall", {}).items()}
    if sorted(syscalls) != list(range(16)):
        die(f"syscall phải 0..15, đang {sorted(syscalls)}")
    errors = {int(k): v for k, v in doc.get("error", {}).items()}
    if sorted(errors) != list(range(1, 20)):
        die(f"error phải 1..19, đang {sorted(errors)}")
    objects = doc.get("object", {})
    core = sorted(((v["id"], name) for name, v in objects.items()
                   if v.get("stability") in {"core", "frozen"}))
    reserved = sorted((v["id"], name) for name, v in objects.items() if v.get("stability") == "reserved")
    sig = doc.get("method_signature", {})
    if sig.get("stability") != "stable-draft":
        die("spec thiếu [method_signature].stability = 'stable-draft' (KT-05 bước 0)")
    methods = {k: v for k, v in sig.items() if isinstance(v, dict) and "params" in v}
    states = sig.get("state", {}).get("names", [])
    out = []
    out.append("// SPDX-License-Identifier: MIT OR Apache-2.0")
    out.append("// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen từ spec/abi.toml — ĐỪNG SỬA TAY (gen-check sẽ bắt).")
    out.append("#![allow(clippy::all)]")
    out.append("")
    out.append("#[rustfmt::skip]")
    out.append(f"pub const ABI_VERSION: u32 = {int(doc.get('abi_version', doc.get('version', 1)))};")
    out.append("#[rustfmt::skip]")
    # KT-62: `min_compat` cũng là hằng ABI (`DebugGetInfo` trả nó) — cũng phải sinh từ spec (D2-11).
    out.append(f"pub const MIN_COMPAT: u32 = {int(doc.get('min_compat', 1))};")
    out.append("#[rustfmt::skip]")
    out.append(f"pub const SYSCALL_COUNT: usize = {len(syscalls)};")
    out.append("#[rustfmt::skip]")
    out.append("pub const SYSCALLS: [(&str, u16); SYSCALL_COUNT] = [")
    for num in sorted(syscalls):
        out.append(f'    ("{syscalls[num]["name"]}", {num}),')
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append(f"pub const ERROR_COUNT: usize = {len(errors)};")
    # KT-62/D2-88(c): tên + số hiệu syscall cũng là hằng ABI sinh từ `[syscall]` — core/plat
    # không chép tay `SYS_YIELD = 8`… Tên hằng đọc được (NBSendRecv → SYS_NBSEND_RECV).
    sys_aliases = {
        "NBSend": "NB_SEND",
        "NBRecv": "NB_RECV",
        "NBSendRecv": "NB_SEND_RECV",
        "NBSendWait": "NB_SEND_WAIT",
        "ReplyRecv": "REPLY_RECV",
        "WaitAddr": "WAIT_ADDR",
        "WakeAddr": "WAKE_ADDR",
        "ClockRead": "CLOCK_READ",
        "DebugPutStr": "DEBUG_PUT_STR",
        "DebugHalt": "DEBUG_HALT",
        "DebugGetInfo": "DEBUG_GET_INFO",
    }
    for num in sorted(syscalls):
        name = syscalls[num]["name"]
        const = "SYS_" + sys_aliases.get(name, name.upper())
        out.append("#[rustfmt::skip]")
        out.append(f"pub const {const}: u64 = {num};")
    out.append("#[rustfmt::skip]")
    out.append("pub const ERRORS: [(&str, u16); ERROR_COUNT] = [")
    for num in sorted(errors):
        out.append(f'    ("{errors[num]["name"]}", {num}),')
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append("pub const CORE_OBJECTS: [(&str, u16); %d] = [" % len(core))
    for oid, name in core:
        out.append(f'    ("{name}", {oid}),')
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append("pub const RESERVED_OBJECTS: [(&str, u16); %d] = [" % len(reserved))
    for oid, name in reserved:
        out.append(f'    ("{name}", {oid}),')
    out.append("];")
    # KT-68: **số method** theo object (`[object].<Tên>.method`) — invocation `CNode.*` phải
    # dùng đúng số spec, không chép tay (D2-11). Chỉ sinh cho object có method; số phải liền
    # mạch 0..N (lệch ⇒ dừng, fail-closed, không tự đánh số).
    for obj_name in sorted(objects):
        methods_of = objects[obj_name].get("method") or {}
        if not methods_of:
            continue
        ids = sorted(int(k) for k in methods_of)
        if ids != list(range(len(ids))):
            die(f"[object].{obj_name}.method phải liền mạch 0..N, đang {ids}")
        const = f"{obj_name.upper()}_METHODS"
        out.append("#[rustfmt::skip]")
        out.append(f"pub const {const}: [(&str, u16); {len(ids)}] = [")
        for num in ids:
            out.append(f'    ("{methods_of[str(num)]["name"]}", {num}),')
        out.append("];")
        # KT-69/D2-102(c): **hằng số method theo tên** sinh từ spec để U-mode/kernel không chép
        # tay số (`FRAME_MAP = 0`, …). U-mode dùng `vifast-abi` rlib thay vì literal.
        for num in ids:
            method_name = methods_of[str(num)]["name"].upper()
            out.append("#[rustfmt::skip]")
            out.append(f"pub const {obj_name.upper()}_{method_name}: u32 = {num};")
    # KT-58: kích thước/căn lề object từ `[object_size]` (QD §3.9) — core `no_std` dùng
    # hằng sinh, không hardcode 48/64/4096 trong `vifast-core`.
    osz = doc.get("object_size") or {}
    slot = osz.get("slot") or {}
    if slot:
        out.append("#[rustfmt::skip]")
        out.append(f"pub const SLOT_BYTES: usize = {int(slot['bytes'])};")
        out.append("#[rustfmt::skip]")
        out.append(f"pub const SLOT_ALIGN_BYTES: usize = {int(slot['align_bytes'])};")
    for key, name in (
        ("default_align_bytes", "DEFAULT_ALIGN_BYTES"),
        ("page_bytes", "PAGE_BYTES"),
        ("untyped_min_bytes", "UNTYPED_MIN_BYTES"),
    ):
        if key in osz:
            out.append("#[rustfmt::skip]")
            out.append(f"pub const {name}: usize = {int(osz[key])};")
    # **D2-242 nhóm 8 (8C)** — `[object_size]` khai **một** bộ `model` + ba alias
    # `same_as = "model"`. Ba arch phải giống nhau; lệch ⇒ `die` (không sinh hằng nửa vời).
    model = resolve_object_size(osz)
    for key in sorted(model):
        const = "CNODE_BASE_BYTES" if key == "cnode_base" else f"OBJ_{key.upper()}_BYTES"
        out.append("#[rustfmt::skip]")
        out.append(f"pub const {const}: usize = {int(model[key])};")
    # **D2-242 nhóm 2 (2C)** — hằng vùng IPC buffer (đóng băng vùng extra caps).
    ipc = doc.get("ipc_buffer") or {}
    for key, const in (
        ("bytes", "IPC_BUFFER_BYTES"),
        ("align", "IPC_BUFFER_ALIGN"),
        ("mr_offset", "IPC_BUFFER_MR_OFFSET"),
        ("mr_words", "IPC_BUFFER_MR_WORDS"),
        ("extra_caps_offset", "IPC_BUFFER_EXTRA_CAPS_OFFSET"),
        ("extra_caps_max", "IPC_BUFFER_EXTRA_CAPS_MAX"),
        ("extra_caps_dst_offset", "IPC_BUFFER_EXTRA_CAPS_DST_OFFSET"),
        ("configure_offset", "IPC_BUFFER_CONFIGURE_OFFSET"),
    ):
        if key not in ipc:
            die(f"spec thiếu [ipc_buffer].{key} (D2-242 nhóm 2)")
        label = " // [TK] — CHƯA đóng băng (D2-242 nhóm 2)" if key == "configure_offset" else ""
        out.append("#[rustfmt::skip]")
        out.append(f"pub const {const}: usize = {int(ipc[key])};{label}")
    out.append("#[rustfmt::skip]")
    out.append("/// (method, params, returns) — từ `[method_signature]` (stable-draft).")
    out.append("#[rustfmt::skip]")
    out.append("pub const METHOD_SIGNATURES: [(&str, &[&str], &[&str]); %d] = [" % len(methods))
    for name in sorted(methods):
        params = ", ".join(f'"{p}"' for p in methods[name].get("params", []))
        rets = ", ".join(f'"{r}"' for r in methods[name].get("returns", []))
        out.append(f'    ("{name}", &[{params}], &[{rets}]),')
    out.append("];")
    tr = doc.get("trace") or {}
    if tr:
        out.append(f"pub const TRACE_RECORD_BYTES: usize = {int(tr.get('record_bytes', 0))};")
        out.append("#[rustfmt::skip]")
        out.append("pub const TRACE_FIELDS: [(&str, usize, usize); %d] = [" % len(tr.get("fields") or []))
        for f in tr.get("fields") or []:
            out.append(f'    ("{f["name"]}", {int(f["offset"])}, {int(f["bytes"])}),')
        out.append("];")
    names = ", ".join(f'"{n}"' for n in states)
    out.append("#[rustfmt::skip]")
    out.append(f"pub const TCB_STATES: [&str; {len(states)}] = [{names}];")
    # KT-56 / D2-56a: FaultKind đầy đủ (mở spec MỘT lần — D2-72 Q1). Sinh từ
    # `[method_signature.fault_kind]` để sửa tay / lệch spec bị gen-check bắt.
    fk = sig.get("fault_kind") or {}
    fk_names = fk.get("names") or []
    if fk_names:
        out.append("#[rustfmt::skip]")
        out.append(f"pub const FAULT_KIND_COUNT: usize = {len(fk_names)};")
        out.append("#[rustfmt::skip]")
        out.append(
            "pub const FAULT_KINDS: [(&str, &[&str]); %d] = [" % len(fk_names)
        )
        for name in fk_names:
            fields = ", ".join(
                f'"{f}"' for f in (fk.get(name) or {}).get("fields", [])
            )
            out.append(f'    ("{name}", &[{fields}]),')
        out.append("];")
    # **KT-269 / D2-206**: quy ước gọi sinh từ `[calling.*]` — chỉ arch **đã mở** (có
    # `syscall_number`). Nhờ vậy U-mode/kernel dùng hằng sinh thay vì chép tay tên thanh ghi
    # (D2-11); `reserved = true` (arch chưa mở) **không** sinh gì.
    gen_calling(out, doc)
    gen_debug_info(out, doc)
    gen_boot_caps(out, doc)
    out.append("")
    return "\n".join(out)


def gen_calling(out: list[str], doc: dict) -> None:
    """Sinh hằng quy ước gọi cho mọi arch **đã mở** trong `[calling.*]`."""
    calling = doc.get("calling") or {}
    opened = sorted(
        arch
        for arch, entry in calling.items()
        if isinstance(entry, dict) and entry.get("syscall_number")
    )
    if not opened:
        return
    arches = ", ".join(f'"{arch}"' for arch in opened)
    out.append("#[rustfmt::skip]")
    out.append(f"pub const CALLING_ARCHES: [&str; {len(opened)}] = [{arches}];")
    for arch in opened:
        entry = calling[arch]
        prefix = arch.upper()
        for key, const in (
            ("syscall_number", "SYSCALL_NUMBER_REG"),
            ("message_info", "MESSAGE_INFO_REG"),
            ("return_error", "RETURN_ERROR_REG"),
            ("return_value", "RETURN_VALUE_REG"),
            ("return_message_info", "RETURN_MESSAGE_INFO_REG"),
            ("trap", "TRAP"),
        ):
            value = entry.get(key)
            if isinstance(value, str):
                out.append("#[rustfmt::skip]")
                out.append(f'pub const {prefix}_{const}: &str = "{value}";')
        for key, const in (
            ("args", "ARG_REGS"),
            ("mr_registers", "MR_REGS"),
            ("preserved", "PRESERVED_REGS"),
            ("clobbered", "CLOBBERED_REGS"),
        ):
            value = entry.get(key)
            if isinstance(value, list):
                items = ", ".join(f'"{item}"' for item in value)
                out.append("#[rustfmt::skip]")
                out.append(f"pub const {prefix}_{const}: [&str; {len(value)}] = [{items}];")


def resolve_object_size(osz: dict) -> dict:
    """Giải `[object_size]` theo D2-242 nhóm 8: MỘT bộ `model` + 3 alias `same_as`.

    Lệch ⇒ `die` (fail-closed): spec sai không được sinh hằng nửa vời.
    """
    if not osz:
        die("spec thiếu [object_size]")
    model = osz.get("model")
    if not isinstance(model, dict) or not model:
        die("[object_size].model phải là bảng cỡ object (D2-242 nhóm 8)")
    if "same_as" in model:
        die("[object_size].model không được là alias (phải là bộ cỡ thật)")
    arches = ("riscv64", "aarch64", "x86_64")
    for arch in arches:
        entry = osz.get(arch)
        if not isinstance(entry, dict):
            die(f"thiếu [object_size].{arch} = {{ same_as = \"model\" }} (D2-242 nhóm 8)")
        if set(entry) != {"same_as"}:
            die(
                f"[object_size].{arch} phải là alias `same_as` (D2-242 nhóm 8), "
                f"đang {sorted(entry)}"
            )
        if entry["same_as"] != "model":
            die(f"[object_size].{arch}.same_as phải = 'model', đang {entry['same_as']!r}")
    # Alias trỏ đúng model ⇒ ba arch bằng nhau theo cấu trúc; kiểm thêm khoá lạ ở cấp [object_size].
    known = {"slot", "default_align_bytes", "page_bytes", "untyped_min_bytes", "model", *arches}
    for key in osz:
        if key not in known:
            die(f"[object_size].{key} lạ — sửa bằng D2 mới (D2-242 nhóm 8)")
    return model


def gen_debug_info(out: list[str], doc: dict) -> None:
    """**D2-242 nhóm 8 (8C)** — kênh trả `DebugGetInfo` khai theo **VAI TRÒ**, sinh per-arch.

    `return_channel = { value = "<role>", mr = ["<role>", …] }`; tên thanh ghi lấy từ
    `[calling.<arch>].return_value` và `[calling.<arch>].mr_registers[i]`. Thiếu/lệch ⇒ `die`.
    """
    dbg = doc.get("debug_info") or {}
    channel = dbg.get("return_channel")
    if not isinstance(channel, dict) or "value" not in channel or "mr" not in channel:
        die("[debug_info].return_channel phải là { value = …, mr = [...] } (D2-242 nhóm 8)")
    returns = dbg.get("returns") or []
    roles = [channel["value"], *channel["mr"]]
    if roles != returns:
        die(
            "[debug_info].return_channel phải phủ đúng `returns` theo thứ tự "
            f"(value + mr), đang {roles} vs {returns}"
        )
    calling = doc.get("calling") or {}
    opened = sorted(
        arch for arch, entry in calling.items()
        if isinstance(entry, dict) and entry.get("syscall_number")
    )
    if not opened:
        die("không có arch nào mở trong [calling.*] để suy [debug_info].return_channel")
    out.append("#[rustfmt::skip]")
    out.append(f'pub const DEBUG_INFO_RETURN_VALUE_ROLE: &str = "{channel["value"]}";')
    out.append("#[rustfmt::skip]")
    mr_roles = ", ".join(f'"{role}"' for role in channel["mr"])
    out.append(
        f"pub const DEBUG_INFO_RETURN_MR_ROLES: [&str; {len(channel['mr'])}] = [{mr_roles}];"
    )
    for arch in opened:
        entry = calling[arch]
        value_reg = entry.get("return_value")
        mr_regs = entry.get("mr_registers")
        if not isinstance(value_reg, str) or not value_reg:
            die(f"[calling.{arch}] thiếu return_value — không suy được kênh value")
        if not isinstance(mr_regs, list) or len(mr_regs) < len(channel["mr"]):
            die(
                f"[calling.{arch}].mr_registers phải có ≥ {len(channel['mr'])} thanh ghi "
                f"cho DebugGetInfo, đang {mr_regs}"
            )
        if value_reg in mr_regs[: len(channel["mr"])]:
            die(
                f"[calling.{arch}]: return_value {value_reg!r} trùng một MR của DebugGetInfo "
                f"— hai kênh không thể cùng thanh ghi"
            )
        prefix = arch.upper()
        out.append("#[rustfmt::skip]")
        out.append(f'pub const {prefix}_DEBUG_INFO_RETURN_VALUE_REG: &str = "{value_reg}";')
        regs = ", ".join(f'"{reg}"' for reg in mr_regs[: len(channel["mr"])])
        out.append(
            f"pub const {prefix}_DEBUG_INFO_RETURN_MR_REGS: "
            f"[&str; {len(channel['mr'])}] = [{regs}];"
        )


# ---------------------------------------------------------------------------------------------
# **D2-261 nhóm 8 (8A)** — `[boot_caps]` → `BootCapLayout` (`#[repr(C)]`) + 2 profile.
# Thứ tự trường là **hợp đồng ABI bố cục** (sửa ⇒ `boot_abi` mới); không suy ra từ tên khoá.
# ---------------------------------------------------------------------------------------------

BOOT_CAPS_CSPACE_ORDER = [
    "fault", "proc", "uart", "reply", "ps", "sched_control", "fs", "events",
    "net", "net_reply", "net_kern", "net_kern_reply", "exec", "mem", "loop_ep",
    "net_client", "net_client_reply",
]
BOOT_CAPS_TAIL_ORDER = ["self_cspace", "tcb", "sc", "loop_arena", "loop_tcb", "loop_sc"]
BOOT_CAPS_PROC_ORDER = ["procsrv", "shell"]


def _boot_val(table, key: str, where: str):
    """Đọc một leaf `{ value, source }` của `[boot_caps.*]`; sai hình dạng ⇒ `die`."""
    leaf = table.get(key) if isinstance(table, dict) else None
    if not isinstance(leaf, dict) or "value" not in leaf or "source" not in leaf:
        die(f"[boot_caps].{where} phải là {{ value, source }} (D2-261 nhóm 8)")
    value = leaf["value"]
    if isinstance(value, list):
        if not value or not all(isinstance(v, int) and not isinstance(v, bool) for v in value):
            die(f"[boot_caps].{where} phải là mảng số nguyên không rỗng")
        return list(value)
    if not isinstance(value, int) or isinstance(value, bool):
        die(f"[boot_caps].{where} phải là số nguyên")
    return value


def gen_boot_caps(out: list[str], doc: dict) -> None:
    """Sinh `BootCapLayout` + `BOOT_CAPS_TEST`/`BOOT_CAPS_PRODUCT` từ `[boot_caps]`."""
    bc = doc.get("boot_caps")
    if not isinstance(bc, dict) or not bc:
        die("spec thiếu [boot_caps] (D2-261 nhóm 8)")
    if bc.get("boot_abi") != 1:
        die(f"[boot_caps].boot_abi phải = 1, đang {bc.get('boot_abi')!r}")
    none_cspace = bc.get("none_cspace")
    none_proc = bc.get("none_proc")
    if not isinstance(none_cspace, int) or not 0 < none_cspace <= 0xFFFF:
        die("[boot_caps].none_cspace phải là số dương ≤ 0xFFFF")
    if not isinstance(none_proc, int) or not 0 < none_proc <= 0xFF:
        die("[boot_caps].none_proc phải là số dương ≤ 0xFF")
    cspace = bc.get("cspace") or {}
    values = {
        key: _boot_val(cspace, key, f"cspace.{key}")
        for key in BOOT_CAPS_CSPACE_ORDER + BOOT_CAPS_TAIL_ORDER
    }
    tcb_slots = values["tcb"]
    sc_slots = values["sc"]
    if len(tcb_slots) != 5 or len(sc_slots) != 5:
        die("[boot_caps.cspace].tcb/.sc phải là mảng 5 phần tử (D2-261 nhóm 8)")
    proc = bc.get("proc") or {}
    proc_vals = {key: _boot_val(proc, key, f"proc.{key}") for key in BOOT_CAPS_PROC_ORDER}
    test_child = _boot_val(proc.get("test") or {}, "child", "proc.test.child")
    product_netsrv = _boot_val(proc.get("product") or {}, "netsrv", "proc.product.netsrv")
    for key in BOOT_CAPS_CSPACE_ORDER + ["self_cspace", "loop_arena", "loop_tcb", "loop_sc"]:
        if not 0 < values[key] <= 0xFFFE:
            die(f"[boot_caps].cspace.{key} phải trong 1..0xFFFE (0xFFFF là sentinel vắng)")

    def arr(items: list[int]) -> str:
        return "[" + ", ".join(str(v) for v in items) + "]"

    out.append("#[rustfmt::skip]")
    out.append(f"pub const BOOT_ABI: u32 = {int(bc['boot_abi'])};")
    out.append("#[rustfmt::skip]")
    out.append(f"pub const BOOT_CAPS_NONE_CSPACE: u16 = 0x{none_cspace:04X};")
    out.append("#[rustfmt::skip]")
    out.append(f"pub const BOOT_CAPS_NONE_PROC: u8 = 0x{none_proc:02X};")
    out.append("/// **D2-261 nhóm 8** — bố cục cap lúc boot (`#[repr(C)]`; thứ tự trường là ABI).")
    out.append("#[rustfmt::skip]")
    out.append("#[repr(C)]")
    out.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    out.append("pub struct BootCapLayout {")
    for key in BOOT_CAPS_CSPACE_ORDER + BOOT_CAPS_TAIL_ORDER:
        if key in ("tcb", "sc"):
            out.append(f"    /// Slot cap (u16), sentinel `BOOT_CAPS_NONE_CSPACE`.")
            out.append(f"    pub {key}: [u16; 5],")
        else:
            out.append(f"    pub {key}: u16,")
    for key in BOOT_CAPS_PROC_ORDER:
        out.append(f"    pub {key}: u8,")
    out.append("    /// Ảnh test `child` trong ô chương trình (sentinel `BOOT_CAPS_NONE_PROC`).")
    out.append("    pub child: u8,")
    out.append("    /// Ảnh sản phẩm `netsrv` trong **cùng** ô 2 (khác ảnh — D2-261 nhóm 8).")
    out.append("    pub netsrv: u8,")
    out.append("}")

    def emit_profile(name: str, child: int, netsrv: int) -> None:
        out.append("#[rustfmt::skip]")
        out.append(f"pub const {name}: BootCapLayout = BootCapLayout {{")
        for key in BOOT_CAPS_CSPACE_ORDER + BOOT_CAPS_TAIL_ORDER:
            if key in ("tcb", "sc"):
                out.append(f"    {key}: {arr(values[key])},")
            else:
                out.append(f"    {key}: {values[key]},")
        for key in BOOT_CAPS_PROC_ORDER:
            out.append(f"    {key}: {proc_vals[key]},")
        out.append(f"    child: {child},")
        out.append(f"    netsrv: {netsrv},")
        out.append("};")

    emit_profile("BOOT_CAPS_TEST", test_child, none_proc)
    emit_profile("BOOT_CAPS_PRODUCT", none_proc, product_netsrv)


def gen_md(doc: dict) -> str:
    syscalls = {int(k): v for k, v in doc.get("syscall", {}).items()}
    errors = {int(k): v for k, v in doc.get("error", {}).items()}
    objects = {name: v for name, v in doc.get("object", {}).items()}
    out = ["<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->",
           "# ABI v2 — tài liệu sinh tự động",
           "",
           "SINH TỰ ĐỘNG bởi `v2/tools/vifast-gen` từ `spec/abi.toml` — **đừng sửa tay** (`gen-check` sẽ bắt).",
           f"`status = {doc.get('status', '?')!r}` · `abi_version = {int(doc.get('abi_version', doc.get('version', 1)))}`",
           "",
           "## Syscall (0..15)", "",
           "| # | Tên | Tham số | Trả |", "|---|---|---|---|"]
    for num in sorted(syscalls):
        sc = syscalls[num]
        out.append(f"| {num} | `{sc['name']}` | {sc.get('args','—')} | {sc.get('ret','—')} |")
    out += ["", "## Mã lỗi (1..19)", "", "| # | Tên | Ghi chú |", "|---|---|---|"]
    for num in sorted(errors):
        e = errors[num]
        out.append(f"| {num} | `{e['name']}` | {e.get('detail','')} |")
    out += ["", "## Object", "", "| ID | Tên | Ổn định |", "|---|---|---|"]
    for name in sorted(objects, key=lambda n: objects[n]["id"]):
        o = objects[name]
        out.append(f"| {o['id']} | `{name}` | {o.get('stability','?')} |")
    sig = {k: v for k, v in doc.get("method_signature", {}).items() if isinstance(v, dict) and "params" in v}
    out += ["", "## Chữ ký method (stable-draft, D2-33)", "", "| Method | Tham số | Trả |", "|---|---|---|"]
    for name in sorted(sig):
        params = ", ".join(sig[name].get("params", [])) or "—"
        rets = ", ".join(sig[name].get("returns", [])) or "—"
        out.append(f"| `{name}` | {params} | {rets} |")
    # **KT-269 / D2-206**: quy ước gọi của arch **đã mở** — sinh từ `[calling.*]`, không chép tay.
    calling = doc.get("calling") or {}
    opened = sorted(
        arch
        for arch, entry in calling.items()
        if isinstance(entry, dict) and entry.get("syscall_number")
    )
    if opened:
        out += ["", "## Quy ước gọi (`[calling.*]`, D2-206)", "",
                "| Arch | Số hiệu | Tham số | MessageInfo | MR | Trả lỗi | Trả value | Chuyển |",
                "|---|---|---|---|---|---|---|---|"]
        for arch in opened:
            entry = calling[arch]
            args = ", ".join(entry.get("args", [])) or "—"
            mrs = ", ".join(entry.get("mr_registers", [])) or "—"
            out.append(
                f"| `{arch}` | `{entry.get('syscall_number', '—')}` | {args} | "
                f"`{entry.get('message_info', '—')}` | {mrs} | "
                f"`{entry.get('return_error', '—')}` | `{entry.get('return_value', '—')}` | "
                f"`{entry.get('trap', '—')}` |"
            )
        out += ["", "Arch còn `reserved = true` (chưa mở) không có mặt trong bảng này; `—` = spec "
                "không khai báo trường ấy (riscv64 đóng băng, không được thêm trường)."]
    fk = doc.get("method_signature", {}).get("fault_kind") or {}
    fk_names = fk.get("names") or []
    if fk_names:
        out += ["", "## `FaultKind` (D2-56a, `[method_signature.fault_kind]`)", "",
                "| Biến thể | Trường |", "|---|---|"]
        for name in fk_names:
            fields = ", ".join((fk.get(name) or {}).get("fields", [])) or "—"
            out.append(f"| `{name}` | {fields} |")
    tr = doc.get("trace")
    if tr:
        out += ["", "## Trace (D2-40, `[trace]`)", "",
                f"`record_bytes = {tr.get('record_bytes')}`, `{tr.get('endianness')}`; header `{tr.get('header')}`", "",
                "| Trường | Offset | Bytes | Kiểu |", "|---|---|---|---|"]
        for f in tr.get("fields") or []:
            out.append(f"| `{f['name']}` | {f['offset']} | {f['bytes']} | `{f['ty']}` |")
    bc = doc.get("boot_caps") or {}
    if isinstance(bc, dict) and bc:
        cspace = bc.get("cspace") or {}
        proc = bc.get("proc") or {}
        out += ["", "## Boot caps (`[boot_caps]`, D2-261 nhóm 8)", "",
                f"`boot_abi = {bc.get('boot_abi')}` · sentinel `none_cspace = 0x"
                f"{int(bc.get('none_cspace', 0)):04X}`, `none_proc = 0x"
                f"{int(bc.get('none_proc', 0)):02X}`", "",
                "| Trường (cspace) | Slot |", "|---|---:|"]
        for key in BOOT_CAPS_CSPACE_ORDER + BOOT_CAPS_TAIL_ORDER:
            leaf = cspace.get(key) or {}
            if "value" in leaf:
                value = leaf["value"]
                shown = ", ".join(str(v) for v in value) if isinstance(value, list) else str(value)
                out.append(f"| `{key}` | {shown} |")
        out += ["", "| Trường (proc) | Ô |", "|---|---:|"]
        for key in BOOT_CAPS_PROC_ORDER:
            leaf = proc.get(key) or {}
            out.append(f"| `{key}` | {leaf.get('value', '—')} |")
        for label, table in (("test", proc.get("test") or {}), ("product", proc.get("product") or {})):
            for key, leaf in table.items():
                if isinstance(leaf, dict) and "value" in leaf:
                    out.append(f"| `{label}.{key}` | {leaf['value']} |")
        out += ["", "`BOOT_CAPS_TEST` đặt `child` vào ô 2 và `netsrv = NONE_PROC`; "
                "`BOOT_CAPS_PRODUCT` ngược lại — hai ảnh dùng **cùng** ô 2 (D2-261 nhóm 8)."]
    out.append("")
    return "\n".join(out)


def targets(root: Path) -> list[tuple[Path, str]]:
    return [
        (root / "v2/lib/vifast-abi/src/generated.rs", "rs"),
        (root / "docs/abi.md", "md"),
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--spec", default=None, help="mặc định <root>/spec/abi.toml")
    ap.add_argument("--check", action="store_true", help="so byte, không ghi")
    args = ap.parse_args()
    root = Path(args.root).resolve()
    spec = Path(args.spec) if args.spec else root / "spec/abi.toml"
    doc = load(spec)
    rc = 0
    for path, kind in targets(root):
        text = gen_rs(doc) if kind == "rs" else gen_md(doc)
        if args.check:
            have = path.read_text() if path.exists() else ""
            if have != text:
                print(f"gen-check: LỆCH {path.relative_to(root)} (sinh lại bằng `v2/bin/vifast-gen gen`)",
                      file=sys.stderr)
                rc = 1
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
            print(f"vifast-gen: ghi {path.relative_to(root)}")
    if args.check and rc == 0:
        print("gen-check: PASS (code + docs khớp spec/abi.toml)")
    return rc


if __name__ == "__main__":
    sys.exit(main())
