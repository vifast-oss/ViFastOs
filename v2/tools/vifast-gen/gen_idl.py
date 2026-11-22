#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""`vifast-idl` — sinh stub Rust `no_std` từ tệp IDL tối thiểu (`.vidl`) — KT-69.

Vì sao có generator riêng (không nhét vào `gen.py`): `spec/abi.toml` là nguồn sự thật của **ABI
lõi** (syscall/error/object), còn IDL của một interface dịch vụ **không thuộc ABI** — KT-69 không
mở `spec/` (D2-99). Generator đọc **hai** thư mục IDL:

    v2/lib/vifast-idl/idl/*.vidl      → v2/lib/vifast-idl/src/generated.rs (+ generated_pager.rs)
    spec/protocols/*.vidl            → v2/lib/vifast-idl/src/generated_<tên>.rs

**D2-242 nhóm 6 (6A)**: `socket.vidl` (đa caller `NetSvc`) + `meminfo.vidl`/`fsstat.vidl`
(nhóm 4C) nằm ở `spec/protocols/`; `ring`/`pager` **giữ nguyên** đường cũ tới `KT-346`.

**Nguồn số method**: `spec/abi.toml` **không có** bảng method cho interface mới, nên method id
được sinh **tuần tự từ tệp IDL** (0..N-1 theo thứ tự khai báo). Điều này được ghi ngay trong tệp
sinh và trong docs — không giả vờ là số spec (D2-102c).

Stub sinh ra là **thuần encode/decode**: không `unsafe`, không `ecall`, không phụ thuộc crate
nào. Chương trình U-mode ghép stub với shim syscall; nhờ vậy stub test được trên host.

Dùng:
    ./v2/bin/vifast-idl gen       # ghi generated.rs
    ./v2/bin/vifast-idl --check   # so byte, lệch ⇒ exit 1 (gen-check của cổng)
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

TYPES = {"u32", "u64"}
CAP_TYPES = {"frame", "buffer"}
RETURNS = {"u32", "u64", "unit"}
IDENT = re.compile(r"^[a-z][a-z0-9_]*$")


def die(msg: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"vifast-idl: {msg}", file=sys.stderr)
    raise SystemExit(1)


class Method:
    def __init__(self, name: str) -> None:
        self.name = name
        self.params: list[tuple[str, str]] = []
        self.cap: tuple[str, str] | None = None
        self.ret = "unit"

    @property
    def camel(self) -> str:
        return "".join(part.capitalize() for part in self.name.split("_"))


class Iface:
    def __init__(self, name: str, version: int) -> None:
        self.name = name
        self.version = version
        self.methods: list[Method] = []


def parse(path: Path) -> Iface:
    try:
        lines = path.read_text().splitlines()
    except FileNotFoundError:
        die(f"không có {path}")
    iface: Iface | None = None
    current: Method | None = None
    for lineno, raw in enumerate(lines, 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        keyword = parts[0]
        where = f"{path.name}:{lineno}"
        if keyword == "interface":
            if iface is not None:
                die(f"{where}: chỉ được MỘT dòng interface")
            if len(parts) != 4 or parts[2] != "version":
                die(f"{where}: cú pháp 'interface <tên> version <số>'")
            if not IDENT.match(parts[1]):
                die(f"{where}: tên interface không hợp lệ: {parts[1]!r}")
            try:
                version = int(parts[3])
            except ValueError:
                die(f"{where}: version phải là số nguyên")
            if version <= 0:
                die(f"{where}: version phải > 0")
            iface = Iface(parts[1], version)
            current = None
            continue
        if iface is None:
            die(f"{where}: dòng đầu tiên (sau chú thích) phải là 'interface'")
        if keyword == "method":
            if len(parts) != 2 or not IDENT.match(parts[1]):
                die(f"{where}: cú pháp 'method <tên>' (chữ thường, [a-z0-9_])")
            if any(m.name == parts[1] for m in iface.methods):
                die(f"{where}: method trùng tên: {parts[1]}")
            current = Method(parts[1])
            iface.methods.append(current)
        elif keyword == "param":
            if current is None:
                die(f"{where}: 'param' phải nằm trong một method")
            if len(parts) != 3:
                die(f"{where}: cú pháp 'param <tên> <u32|u64>'")
            if not IDENT.match(parts[1]):
                die(f"{where}: tên tham số không hợp lệ: {parts[1]!r}")
            if parts[2] not in TYPES:
                die(f"{where}: kiểu {parts[2]!r} không hỗ trợ (chỉ {sorted(TYPES)})")
            if any(p[0] == parts[1] for p in current.params):
                die(f"{where}: tham số trùng tên: {parts[1]}")
            current.params.append((parts[1], parts[2]))
        elif keyword == "cap":
            if current is None:
                die(f"{where}: 'cap' phải nằm trong một method")
            if len(parts) != 3:
                die(f"{where}: cú pháp 'cap <tên> <frame|buffer>'")
            if current.cap is not None:
                die(f"{where}: mỗi method tối đa MỘT cap/buffer")
            if not IDENT.match(parts[1]):
                die(f"{where}: tên cap không hợp lệ: {parts[1]!r}")
            if parts[2] not in CAP_TYPES:
                die(f"{where}: loại cap {parts[2]!r} không hỗ trợ (chỉ {sorted(CAP_TYPES)})")
            current.cap = (parts[1], parts[2])
        elif keyword == "returns":
            if current is None:
                die(f"{where}: 'returns' phải nằm trong một method")
            if len(parts) != 2 or parts[1] not in RETURNS:
                die(f"{where}: cú pháp 'returns <u32|u64|unit>'")
            current.ret = parts[1]
        else:
            die(f"{where}: từ khoá lạ {keyword!r}")
    if iface is None:
        die(f"{path.name}: thiếu dòng 'interface'")
    if not iface.methods:
        die(f"{path.name}: interface phải có ít nhất một method")
    if len(iface.methods) > 32:
        die(f"{path.name}: tối đa 32 method (label 20 bit dành 1 bit reply)")
    for index, method in enumerate(iface.methods):
        if len(method.params) > 4:
            die(f"{path.name}: method {method.name} có {len(method.params)} tham số > 4 (MR0..MR3)")
    if not any(method.cap is not None for method in iface.methods) and not any(
        line.strip() == "# caps=none" for line in path.read_text().splitlines()
    ):
        die(
            f"{path.name}: interface không có method mang cap thì phải ghi rõ `# caps=none` "
            "(lý do cap do kernel cài lúc spawn)"
        )
    return iface


def gen_rs(iface: Iface, source_rel: str) -> str:
    out: list[str] = []
    add = out.append
    add("// SPDX-License-Identifier: MIT OR Apache-2.0")
    add(f"// SINH TỰ ĐỘNG bởi v2/tools/vifast-gen/gen_idl.py từ {source_rel}")
    add("// — ĐỪNG SỬA TAY (`./v2/bin/vifast-idl --check` sẽ bắt).")
    add("//")
    add("// Nguồn số method: `spec/abi.toml` KHÔNG có bảng method cho interface mới, nên method id")
    add("// sinh TUẦN TỰ từ tệp IDL (0..N-1 theo thứ tự khai báo) — không chép tay số nào (D2-102c).")
    add("// Stub thuần encode/decode: không `unsafe`, không syscall, không phụ thuộc crate.")
    add("#![allow(clippy::all)]")
    add("")
    # `#[rustfmt::skip]`: tệp sinh không được rustfmt viết lại (nếu không `cargo fmt --check`
    # của cổng và gen-check sẽ đánh nhau); mọi item đều mang thuộc tính này.
    add("#[rustfmt::skip]")
    add(f'pub const IFACE_NAME: &str = "{iface.name}";')
    add(f"pub const IFACE_VERSION: u32 = {iface.version};")
    add(f"pub const METHOD_COUNT: u32 = {len(iface.methods)};")
    add("")
    add("/// Bit reply trong `label` 20 bit (bit 19) — reply của một method cùng số method.")
    add("pub const REPLY_BIT: u32 = 0x8_0000;")
    add("/// Mặt nạ `label` 20 bit (`spec/abi.toml` `[message_info].label_bits = 20`).")
    add("pub const LABEL_MASK: u32 = 0xF_FFFF;")
    add("/// Dịch bit `caps` của `MessageInfo` (spec `[message_info].caps:20..22`).")
    add("pub const INFO_CAPS_SHIFT: u32 = 20;")
    add("/// Dịch bit `length` của `MessageInfo` (spec `[message_info].length:28..34`).")
    add("pub const INFO_LENGTH_SHIFT: u32 = 28;")
    add("")
    for index, method in enumerate(iface.methods):
        add(f"/// Method `{method.name}` — id {index} (sinh tuần tự từ IDL).")
        add(f"pub const METHOD_{method.name.upper()}: u32 = {index};")
    add("")
    add("/// Đóng gói `MessageInfo` 35 bit từ `label`/`caps`/`length` (không đổi layout ABI).")
    add("pub const fn encode_info(label: u32, caps: u8, length: u8) -> u64 {")
    add("    (label as u64 & LABEL_MASK as u64)")
    add("        | ((caps as u64 & 0x7) << INFO_CAPS_SHIFT)")
    add("        | ((length as u64 & 0x7F) << INFO_LENGTH_SHIFT)")
    add("}")
    add("")
    add("/// Số scalar (MR) của từng method — decode kiểm **chính xác** (D2-104: thiếu/thừa ⇒ lỗi).")
    add("pub const fn arity(method: u32) -> Option<u8> {")
    add("    match method {")
    for index, method in enumerate(iface.methods):
        add(f"        METHOD_{method.name.upper()} => Some({len(method.params)}),")
    add("        _ => None,")
    add("    }")
    add("}")
    add("")
    add("/// Số cap của từng method (0 hoặc 1; cap đi qua receive/send window, không phải MR).")
    add("pub const fn cap_count(method: u32) -> Option<u8> {")
    add("    match method {")
    for index, method in enumerate(iface.methods):
        caps = 1 if method.cap else 0
        add(f"        METHOD_{method.name.upper()} => Some({caps}),")
    add("        _ => None,")
    add("    }")
    add("}")
    add("")
    add("/// Request đã giải mã từ `label` + `length` + `MR0..MR3`.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Request {")
    for method in iface.methods:
        fields = ", ".join(f"{name}: {ty}" for name, ty in method.params)
        if fields:
            add(f"    {method.camel} {{ {fields} }},")
        else:
            add(f"    {method.camel},")
    add("}")
    add("")
    # encoder
    for method in iface.methods:
        args = ", ".join(f"{name}: {ty}" for name, ty in method.params)
        caps = 1 if method.cap else 0
        add(f"/// Client encode `{method.name}`: trả `(MessageInfo, MR0..MR3)`.")
        add(f"pub fn encode_{method.name}({args}) -> (u64, [u64; 4]) {{")
        add(f"    let mr = [0u64; 4];" if not method.params else "    let mut mr = [0u64; 4];")
        for index, (name, _ty) in enumerate(method.params):
            add(f"    mr[{index}] = {name} as u64;")
        add(f"    (encode_info(METHOD_{method.name.upper()}, {caps}, {len(method.params)}), mr)")
        add("}")
        add("")
    # decoder — interface **không** method nào có tham số thì `mr` không được đọc; đặt `_mr` để
    # không sinh cảnh báo `unused_variables` (clippy `-D warnings` của cổng).
    uses_mr = any(method.params for method in iface.methods)
    uses_u32 = any(
        ty == "u32" for method in iface.methods for (_name, ty) in method.params
    ) or any(method.ret == "u32" for method in iface.methods)
    add("/// Server/client decode request; `None` = label lạ, arity sai, hoặc tham số tràn kiểu.")
    mr_param = "mr" if uses_mr else "_mr"
    add(f"pub fn decode(label: u32, length: u8, {mr_param}: &[u64; 4]) -> Option<Request> {{")
    add("    if label & REPLY_BIT != 0 {")
    add("        return None;")
    add("    }")
    add("    match label {")
    for method in iface.methods:
        add(f"        METHOD_{method.name.upper()} => {{")
        add(f"            if length != {len(method.params)} {{")
        add("                return None;")
        add("            }")
        fields: list[str] = []
        for index, (name, ty) in enumerate(method.params):
            if ty == "u32":
                fields.append(f"{name}: u32_of(mr[{index}])?")
            else:
                fields.append(f"{name}: mr[{index}]")
        if fields:
            add(f"            Some(Request::{method.camel} {{ " + ", ".join(fields) + " })")
        else:
            add(f"            Some(Request::{method.camel})")
        add("        }")
    add("        _ => None,")
    add("    }")
    add("}")
    add("")
    # Chỉ sinh `u32_of` khi interface **có** kiểu `u32`; nếu không nó là dead code (`-D warnings`).
    if uses_u32:
        add("/// `u32` không mất bit: giá trị > `u32::MAX` ⇒ `None` (fail-closed, không cắt ngầm).")
        add("fn u32_of(value: u64) -> Option<u32> {")
        add("    if value <= u32::MAX as u64 {")
        add("        Some(value as u32)")
        add("    } else {")
        add("        None")
        add("    }")
        add("}")
        add("")
    # reply encoders per method
    for method in iface.methods:
        label = f"METHOD_{method.name.upper()} | REPLY_BIT"
        add(f"/// Reply của `{method.name}`: `returns {method.ret}`.")
        if method.ret == "unit":
            add(f"pub fn encode_{method.name}_reply() -> (u64, [u64; 4]) {{")
            add(f"    (encode_info({label}, 0, 0), [0u64; 4])")
            add("}")
            add("")
            add(f"pub fn decode_{method.name}_reply(label: u32, length: u8) -> Option<()> {{")
            add(f"    if label == {label} && length == 0 {{")
            add("        Some(())")
            add("    } else {")
            add("        None")
            add("    }")
            add("}")
        else:
            add(f"pub fn encode_{method.name}_reply(value: {method.ret}) -> (u64, [u64; 4]) {{")
            add(f"    (encode_info({label}, 0, 1), [value as u64, 0, 0, 0])")
            add("}")
            add("")
            add(f"pub fn decode_{method.name}_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<{method.ret}> {{")
            add(f"    if label != {label} || length != 1 {{")
            add("        return None;")
            add("    }")
            if method.ret == "u32":
                add("    u32_of(mr[0])")
            else:
                add("    Some(mr[0])")
            add("}")
        add("")
    add("/// Reply chung (mọi method): trả `(method, value)`; `length = 0` cho `unit`.")
    add("pub fn decode_reply(label: u32, length: u8, mr: &[u64; 4]) -> Option<(u32, u64)> {")
    add("    if label & REPLY_BIT == 0 {")
    add("        return None;")
    add("    }")
    add("    let method = label & !REPLY_BIT;")
    add("    let want = arity(method)?;")
    add("    match want {")
    add("        0 => {")
    add("            if length == 0 {")
    add("                Some((method, 0))")
    add("            } else {")
    add("                None")
    add("            }")
    add("        }")
    add("        _ => {")
    add("            if length == 1 {")
    add("                Some((method, mr[0]))")
    add("            } else {")
    add("                None")
    add("            }")
    add("        }")
    add("    }")
    add("}")
    add("")
    text = "\n".join(out)
    # Mọi item công khai mang `#[rustfmt::skip]` (xem chú thích ở đầu): chèn trước dòng bắt đầu
    # bằng `pub const`/`pub fn`/`pub enum`/`pub trait` nếu dòng trước chưa phải thuộc tính.
    skipped: list[str] = []
    for line in text.splitlines():
        starter = line.startswith(
            ("pub const ", "pub fn ", "pub enum ", "pub trait ", "pub struct ", "pub mod ")
        )
        if starter and (not skipped or not skipped[-1].startswith("#[rustfmt::skip]")):
            skipped.append("#[rustfmt::skip]")
        skipped.append(line)
    return "\n".join(skipped) + "\n"


def targets(root: Path) -> list[tuple[Path, Path]]:
    """Cặp `(tệp IDL, tệp sinh)` — một interface một tệp sinh.

    `ring`/`pager` giữ nguyên đường cũ (KT-69/KT-70, tới KT-346); **D2-242 nhóm 6 (6A)** mở thêm
    `spec/protocols/`: `socket` (nhóm 6), `meminfo`/`fsstat` (nhóm 4C).
    """
    idl = root / "v2/lib/vifast-idl/idl"
    src = root / "v2/lib/vifast-idl/src"
    spec = root / "spec/protocols"
    return [
        (idl / "ring.vidl", src / "generated.rs"),
        (idl / "pager.vidl", src / "generated_pager.rs"),
        (spec / "socket.vidl", src / "generated_socket.rs"),
        (spec / "meminfo.vidl", src / "generated_meminfo.rs"),
        (spec / "fsstat.vidl", src / "generated_fsstat.rs"),
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("command", nargs="?", default="gen", choices=["gen"])
    ap.add_argument("--root", default=".")
    ap.add_argument("--idl", default=None, help="chỉ sinh MỘT tệp IDL (mặc định: mọi target)")
    ap.add_argument("--check", action="store_true", help="so byte, không ghi (gen-check)")
    args = ap.parse_args()
    root = Path(args.root).resolve()
    pairs = targets(root)
    if args.idl:
        idl = Path(args.idl)
        pairs = [(idl, root / "v2/lib/vifast-idl/src/generated.rs")]
    rc = 0
    for idl, path in pairs:
        iface = parse(idl)
        try:
            source_rel = idl.relative_to(root).as_posix()
        except ValueError:
            source_rel = idl.as_posix()
        text = gen_rs(iface, source_rel)
        if args.check:
            have = path.read_text() if path.exists() else ""
            if have != text:
                print(
                    f"vifast-idl: LỆCH {path.relative_to(root)} "
                    "(sinh lại bằng `./v2/bin/vifast-idl gen`)",
                    file=sys.stderr,
                )
                rc = 1
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
            print(f"vifast-idl: ghi {path.relative_to(root)}")
    if args.check and rc == 0:
        print("vifast-idl: PASS (stub khớp IDL)")
    return rc


if __name__ == "__main__":
    sys.exit(main())
