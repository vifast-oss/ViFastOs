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
    out.append(f"pub const SYSCALL_COUNT: usize = {len(syscalls)};")
    out.append("#[rustfmt::skip]")
    out.append("pub const SYSCALLS: [(&str, u16); SYSCALL_COUNT] = [")
    for num in sorted(syscalls):
        out.append(f'    ("{syscalls[num]["name"]}", {num}),')
    out.append("];")
    out.append("#[rustfmt::skip]")
    out.append(f"pub const ERROR_COUNT: usize = {len(errors)};")
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
    out.append("")
    return "\n".join(out)


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
    tr = doc.get("trace")
    if tr:
        out += ["", "## Trace (D2-40, `[trace]`)", "",
                f"`record_bytes = {tr.get('record_bytes')}`, `{tr.get('endianness')}`; header `{tr.get('header')}`", "",
                "| Trường | Offset | Bytes | Kiểu |", "|---|---|---|---|"]
        for f in tr.get("fields") or []:
            out.append(f"| `{f['name']}` | {f['offset']} | {f['bytes']} | `{f['ty']}` |")
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
