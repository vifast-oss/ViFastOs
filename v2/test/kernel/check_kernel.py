#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-345a — máy kiểm `spec/kernel.toml` đối chiếu **theo TÊN HẰNG** (D2-242 nhóm 1, `1C`).

Vì sao có tệp này: `spec/kernel.toml` là nguồn sự thật cấu hình kernel (limits/bảng tĩnh/hộp thư/
bit `features`/hình học GDT). Nếu không có máy kiểm thì nó chỉ là tài liệu: số trong spec và số
trong mã có thể lệch âm thầm (đúng lớp lỗi `k6-test-invariants.md` §4 chống).

# Cách kiểm (không phụ thuộc số dòng)

Mỗi giá trị trong spec là `{ value = …, source = "<tệp>#<TÊN_HẰNG>" }`. Với mỗi mục:
  1. tệp nguồn phải tồn tại;
  2. tìm **khai báo** `const <TÊN_HẰNG>` (kể cả `pub const`) trong tệp nguồn;
  3. đọc biểu thức giá trị (`8`, `(INDEX_TSS as u16) * 8`, `MAX_OBJECTS`, …) — hỗ trợ số nguyên,
     tham chiếu hằng cùng tệp, `as <kiểu>`, `+ - * |` và ngoặc;
  4. so với `value`; lệch ⇒ ĐỎ, in đúng tên hằng + hai giá trị.
`source = "<tệp>#text:<chuỗi>"` ⇒ kiểm chuỗi có mặt trong tệp (mục mô tả như `planned`).
`source = "<tệp>"` ⇒ chỉ kiểm tệp tồn tại (mục không có hằng số, vd `sysret_ready`).

Ngoài ra kiểm cấu trúc tối thiểu D2-242 nhóm 1: `status = "APPROVED"`, đủ các mục
`[limits]`/`[limits.core]`/`[limits.plat]`/`[mailbox]`/`[features]`/`[arch.x86_64.gdt]`/
`[reserved]`; hình học GDT tự nhất quán (selector = index × 8, TSS 2 entry, `planned` đúng
thứ tự D2-242 nhóm 5) và bit `features` phân biệt.

Dùng:
    python3 v2/test/kernel/check_kernel.py              # kiểm spec/kernel.toml thật
    python3 v2/test/kernel/check_kernel.py --selftest   # ca âm/dương của chính máy kiểm
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from pathlib import Path

# Gốc repo: `v2/test/kernel/check_kernel.py` ⇒ parents[0]=kernel, [1]=test, [2]=v2, [3]=repo.
ROOT = Path(__file__).resolve().parents[3]
SPEC = ROOT / "spec/kernel.toml"

# Thứ tự ĐÍCH cho `syscall`/`sysret` ở Chu kỳ 3 (D2-242 nhóm 5 = 5A) — CHƯA áp dụng ở v1.1.
PLANNED_GDT_ORDER = [
    "null",
    "kernel_code",
    "kernel_data",
    "user_data",
    "user_code",
    "tss",
]
PLANNED_STAR_63_48 = 16  # `STAR[63:48] = 0x10` (kernel code selector)

RE_LEAF = re.compile(r"^\s*\{")
RE_CONST = re.compile(
    r"(?:^|\s)(?:pub\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([A-Za-z0-9_:<>\[\];&' ]+?)\s*=\s*",
    re.MULTILINE,
)
RE_TOKEN = re.compile(r"\d[\d_]*|[A-Za-z_][A-Za-z0-9_]*|<<|>>|\(|\)|\*|\+|-|\|")


class Fail(Exception):
    """Một lỗi có mã (mã nằm trong thông điệp, dùng cho ca âm của `--selftest`)."""


# ------------------------------------------------------------------ đọc hằng từ mã nguồn


def _strip_line_comments(text: str) -> str:
    out = []
    for line in text.splitlines():
        # Chỉ cần bỏ `//` — mã khai báo hằng của repo không chứa `//` trong chuỗi.
        idx = line.find("//")
        out.append(line[:idx] if idx >= 0 else line)
    return "\n".join(out)


class Consts:
    """Bảng hằng của MỘT tệp nguồn; giải biểu thức giá trị thành số nguyên."""

    def __init__(self, path: Path, text: str) -> None:
        self.path = path
        self.text = text
        self.raw: dict[str, str] = {}
        self._cache: dict[str, int] = {}
        self._busy: set[str] = set()
        for match in RE_CONST.finditer(text):
            name = match.group(1)
            tail = text[match.end() :]
            body = []
            depth = 0
            for ch in tail:
                if ch in "([{":
                    depth += 1
                elif ch in ")]}":
                    depth -= 1
                elif ch == ";" and depth == 0:
                    break
                body.append(ch)
            if not body:
                continue
            self.raw.setdefault(name, "".join(body).strip())

    def has(self, name: str) -> bool:
        return name in self.raw

    def value(self, name: str) -> int:
        if name in self._cache:
            return self._cache[name]
        if name not in self.raw:
            raise Fail(f"E_CONST_MISSING: không thấy hằng {name!r} trong {self.path}")
        if name in self._busy:
            raise Fail(f"E_CONST_CYCLE: hằng {name!r} tham chiếu vòng")
        self._busy.add(name)
        try:
            expr = _strip_line_comments(self.raw[name])
            value = _Expr(self, expr).parse()
        finally:
            self._busy.discard(name)
        self._cache[name] = value
        return value


class _Expr:
    """Bộ giải biểu thức số nguyên nhỏ: `8`, `(INDEX_TSS as u16) * 8`, `A | B`."""

    def __init__(self, consts: Consts, text: str) -> None:
        self.consts = consts
        self.tokens = RE_TOKEN.findall(text)
        self.pos = 0

    def peek(self) -> str | None:
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def take(self, want: str | None = None) -> str:
        tok = self.peek()
        if tok is None:
            raise Fail(f"E_CONST_EXPR: hết biểu thức (muốn {want!r})")
        if want is not None and tok != want:
            raise Fail(f"E_CONST_EXPR: gặp {tok!r}, muốn {want!r}")
        self.pos += 1
        return tok

    def parse(self) -> int:
        value = self.expr()
        if self.peek() is not None:
            raise Fail(f"E_CONST_EXPR: token thừa {self.peek()!r}")
        return value

    def expr(self) -> int:
        value = self.term()
        while self.peek() in ("+", "-", "|", "<<", ">>"):
            op = self.take()
            rhs = self.term()
            if op == "+":
                value += rhs
            elif op == "-":
                value -= rhs
            elif op == "|":
                value |= rhs
            elif op == "<<":
                value <<= rhs
            else:
                value >>= rhs
        return value

    def term(self) -> int:
        value = self.primary()
        while self.peek() == "*":
            self.take("*")
            value *= self.primary()
        return value

    def primary(self) -> int:
        tok = self.peek()
        if tok is None:
            raise Fail("E_CONST_EXPR: thiếu toán hạng")
        if tok == "(":
            self.take("(")
            value = self.expr()
            self.take(")")
        elif tok.isdigit() or (tok.replace("_", "").isdigit() and "_" in tok):
            self.pos += 1
            value = int(tok.replace("_", ""))
        elif tok[0].isalpha() or tok[0] == "_":
            self.pos += 1
            value = self.consts.value(tok)
        else:
            raise Fail(f"E_CONST_EXPR: token lạ {tok!r}")
        # `as <kiểu>` — bỏ qua kiểu (mọi giá trị ở đây đều trong dải u64).
        if self.peek() == "as":
            self.take("as")
            ty = self.take()
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", ty):
                raise Fail(f"E_CONST_EXPR: kiểu ép lạ {ty!r}")
        return value


# ------------------------------------------------------------------ kiểm một tệp spec


def _iter_leaves(doc: dict, prefix: str = ""):
    """Sinh `(đường-dẫn, giá-trị)` cho mọi bảng `{value=..., source=...}`."""
    for key, value in doc.items():
        path = f"{prefix}.{key}" if prefix else str(key)
        if isinstance(value, dict):
            if "value" in value:
                yield path, value
            else:
                yield from _iter_leaves(value, path)


def _scalar_leaves(doc: dict, prefix: str = ""):
    """Sinh trường vô hướng **không** thuộc bảng `{value, source}` (bắt lỗi thiếu source).

    `[arch.x86_64.gdt.planned]` và `[reserved]` được miễn: chúng là **mô tả** (thứ tự đích, danh
    sách nợ) chứ không phải giá trị cấu hình đối chiếu được với một hằng trong mã; `planned` có
    `source` ở cấp bảng, `reserved.keys` có luật riêng (CẤM generator/consumer đọc).
    """
    for key, value in doc.items():
        path = f"{prefix}.{key}" if prefix else str(key)
        if isinstance(value, dict):
            if "value" in value:
                continue
            yield from _scalar_leaves(value, path)
        elif not prefix:
            continue  # khoá cấp gốc (schema/status/name/source) không phải giá trị cấu hình
        elif key in ("note", "source"):
            continue
        else:
            yield path, value


def _source_type(source: str) -> str:
    if "#text:" in source:
        return "text"
    if "#" in source:
        return "const"
    return "file"


def check_spec(path: Path) -> tuple[list[str], int]:
    """Trả `(danh sách lỗi, số giá trị đã đối chiếu)`. Không in, không thoát."""
    fails: list[str] = []
    try:
        raw = path.read_bytes()
    except FileNotFoundError:
        return ([f"E_SPEC_MISSING: không có {path}"], 0)

    if not raw.startswith(b"#"):
        fails.append("E_SPDX: dòng 1 phải là header `#` (SPDX)")
    if b"SPDX-License-Identifier" not in raw:
        fails.append("E_SPDX: thiếu dòng SPDX-License-Identifier")

    try:
        import tomllib
    except ModuleNotFoundError:  # pragma: no cover - môi trường cũ
        raise Fail("E_PYTHON: cần Python ≥3.11 (tomllib)")

    try:
        doc = tomllib.loads(raw.decode("utf-8"))
    except Exception as exc:  # noqa: BLE001 - muốn in mọi lỗi parse
        return ([f"E_TOML: không parse được: {exc}"], 0)

    # --- cấu trúc tối thiểu (D2-242 nhóm 1) ---
    if doc.get("schema") != 1:
        fails.append(f"E_SCHEMA: schema phải = 1, đang {doc.get('schema')!r}")
    if doc.get("status") != "APPROVED":
        fails.append(
            f"E_STATUS: status phải 'APPROVED' (D2-242), đang {doc.get('status')!r}"
        )
    if not isinstance(doc.get("name"), str) or not doc["name"]:
        fails.append("E_NAME: thiếu name")

    required = [
        "limits",
        "limits.core",
        "limits.plat",
        "profiles.mission",
        "profiles.general",
        "fpu",
        "mailbox",
        "features",
        "arch.x86_64.gdt",
        "reserved",
    ]
    for dotted in required:
        node = doc
        for part in dotted.split("."):
            node = node.get(part) if isinstance(node, dict) else None
        if not isinstance(node, dict) or not node:
            fails.append(f"E_SECTION: thiếu mục [{dotted}] (D2-242 nhóm 1)")

    # --- mọi giá trị có `value` phải có `source` và khớp mã ---
    checked = 0
    for where, leaf in _iter_leaves(doc):
        if "source" not in leaf:
            fails.append(f"E_SOURCE_THIEU: {where} có value nhưng thiếu source")
            continue
        source = leaf["source"]
        if not isinstance(source, str) or not source:
            fails.append(f"E_SOURCE_KIEU: {where}.source phải là chuỗi")
            continue
        spec_file, _, ref = source.partition("#")
        target = ROOT / spec_file
        if not target.is_file():
            fails.append(f"E_SOURCE_FILE: {where}: không có tệp nguồn {spec_file}")
            continue
        kind = _source_type(source)
        if kind == "text":
            marker = ref[len("text:") :]
            if marker not in target.read_text(encoding="utf-8", errors="replace"):
                fails.append(
                    f"E_SOURCE_TEXT: {where}: không thấy {marker!r} trong {spec_file}"
                )
            checked += 1
            continue
        if kind == "file":
            checked += 1
            continue
        # kind == "const"
        text = target.read_text(encoding="utf-8", errors="replace")
        consts = Consts(target, text)
        if not consts.has(ref):
            fails.append(f"E_CONST_MISSING: {where}: không thấy hằng {ref!r} trong {spec_file}")
            continue
        value = leaf["value"]
        if not isinstance(value, int) or isinstance(value, bool):
            fails.append(
                f"E_CONST_KIEU: {where}: source #const chỉ so được số nguyên, đang {value!r}"
            )
            continue
        try:
            got = consts.value(ref)
        except Fail as exc:
            fails.append(f"{where}: {exc}")
            continue
        if got != value:
            fails.append(
                f"E_CONST_LECH: {where}: spec value={value} nhưng {ref} trong {spec_file} = {got}"
            )
        checked += 1

    # Trong các mục cấu hình, mọi trường vô hướng phải là `{value, source}` (fail-closed).
    # Miễn: `[arch.x86_64.gdt.planned]` (mô tả thứ tự đích, có `source` ở cấp bảng) và
    # `[reserved]` (danh sách nợ, CẤM sinh/đọc — luật riêng bên dưới).
    scalar_scope = doc.get("limits", {})
    for where, value in _scalar_leaves(scalar_scope):
        fails.append(
            f"E_LEAF_KHONG_SOURCE: {where} = {value!r} phải là bảng {{ value, source }}"
        )
    for section in ("mailbox", "features"):
        for where, value in _scalar_leaves(doc.get(section, {}), section):
            fails.append(
                f"E_LEAF_KHONG_SOURCE: {where} = {value!r} phải là bảng {{ value, source }}"
            )
    gdt_cur = doc.get("arch", {}).get("x86_64", {}).get("gdt", {})
    for where, value in _scalar_leaves(gdt_cur, "arch.x86_64.gdt"):
        if where.startswith("arch.x86_64.gdt.planned"):
            continue
        fails.append(
            f"E_LEAF_KHONG_SOURCE: {where} = {value!r} phải là bảng {{ value, source }}"
        )

    # --- hình học GDT tự nhất quán (D2-242 nhóm 5) ---
    gdt = doc.get("arch", {}).get("x86_64", {}).get("gdt", {})
    if isinstance(gdt, dict):
        def _v(key: str):
            leaf = gdt.get(key)
            return leaf.get("value") if isinstance(leaf, dict) else None

        entries = _v("entries")
        descriptors = _v("descriptors")
        tss_entries = _v("tss_descriptor_entries")
        if isinstance(entries, int) and isinstance(descriptors, list) and isinstance(tss_entries, int):
            if entries != len(descriptors) + (tss_entries - 1):
                fails.append(
                    "E_GDT_ENTRIES: entries phải = số descriptor + (TSS entry − 1) = "
                    f"{len(descriptors) + (tss_entries - 1)}, đang {entries}"
                )
        else:
            fails.append("E_GDT_SHAPE: entries/descriptors/tss_descriptor_entries thiếu")
        idx = {name: _v(f"index_{name}") for name in
               ("null", "kernel_code", "kernel_data", "user_code", "user_data", "tss")}
        sel = {name: _v(f"selector_{name}") for name in
               ("kernel_code", "kernel_data", "user_code", "user_data", "tss")}
        if all(isinstance(v, int) for v in idx.values()):
            if sorted(idx.values()) != list(range(6)):
                fails.append(f"E_GDT_INDEX: index_* phải là 0..5, đang {idx}")
            for name, value in sel.items():
                if isinstance(value, int) and idx.get(name) is not None:
                    if value != idx[name] * 8:
                        fails.append(
                            f"E_GDT_SELECTOR: selector_{name} phải = index_{name} × 8 = "
                            f"{idx[name] * 8}, đang {value}"
                        )
        else:
            fails.append("E_GDT_INDEX: thiếu index_*")
        if _v("sysret_ready") is not False:
            fails.append("E_GDT_SYSRET: sysret_ready phải false ở v1.1 (giữ `int 0x80`)")

    # --- `planned` phải đúng thứ tự đích D2-242 nhóm 5 ---
    planned = doc.get("arch", {}).get("x86_64", {}).get("gdt", {}).get("planned", {})
    if not isinstance(planned, dict):
        fails.append("E_GDT_PLANNED: thiếu [arch.x86_64.gdt.planned]")
    else:
        if planned.get("order") != PLANNED_GDT_ORDER:
            fails.append(
                "E_GDT_PLANNED_ORDER: planned.order phải là "
                f"{PLANNED_GDT_ORDER} (D2-242 nhóm 5), đang {planned.get('order')!r}"
            )
        if planned.get("entries") != 7:
            fails.append(f"E_GDT_PLANNED_ENTRIES: planned.entries phải 7, đang {planned.get('entries')!r}")
        if planned.get("star_63_48") != PLANNED_STAR_63_48:
            fails.append(
                f"E_GDT_PLANNED_STAR: planned.star_63_48 phải {PLANNED_STAR_63_48} (0x10), "
                f"đang {planned.get('star_63_48')!r}"
            )
        if planned.get("trap") != "int 0x80":
            fails.append(
                f"E_GDT_PLANNED_TRAP: planned.trap phải 'int 0x80' ở v1.1, đang {planned.get('trap')!r}"
            )
        if planned.get("min_compat") != 1:
            fails.append(
                f"E_GDT_PLANNED_COMPAT: planned.min_compat phải 1, đang {planned.get('min_compat')!r}"
            )
        # Selector đích: đảo user data (24) trước user code (32), TSS giữ 40.
        want = {"kernel_code": 8, "kernel_data": 16, "user_data": 24, "user_code": 32, "tss": 40}
        for name, value in want.items():
            if planned.get(f"selector_{name}") != value:
                fails.append(
                    f"E_GDT_PLANNED_SELECTOR: planned.selector_{name} phải {value}, "
                    f"đang {planned.get(f'selector_{name}')!r}"
                )

    # --- **D2-261 nhóm 7**: hai profile bộ nhớ + vùng FPU + luật BootArena (`#text:`) ---
    prof = doc.get("profiles") or {}
    mission = prof.get("mission") or {}
    general = prof.get("general") or {}
    if mission.get("store") != "StaticStore":
        fails.append("E_PROFILES_MISSION: [profiles.mission].store phải 'StaticStore'")
    if mission.get("observer") != "NoopObserver":
        fails.append("E_PROFILES_MISSION: [profiles.mission].observer phải 'NoopObserver'")
    if mission.get("size_limits") != "limits.core":
        fails.append("E_PROFILES_MISSION: [profiles.mission].size_limits phải 'limits.core'")
    if general.get("store") != "UntypedStore":
        fails.append("E_PROFILES_GENERAL: [profiles.general].store phải 'UntypedStore'")
    if general.get("observer") != "NoopObserver":
        fails.append("E_PROFILES_GENERAL: [profiles.general].observer phải 'NoopObserver'")
    arena = (general.get("boot_arena") or {}).get("source")
    if not isinstance(arena, str) or "#text:" not in arena:
        fails.append(
            "E_PROFILES_BOOT_ARENA: [profiles.general].boot_arena.source phải là #text: "
            "(cấm chép số đo tay — D2-261 nhóm 7)"
        )
    else:
        spec_file, _, ref = arena.partition("#text:")
        target = ROOT / spec_file
        if not target.is_file():
            fails.append(f"E_PROFILES_BOOT_ARENA: không có tệp nguồn {spec_file}")
        elif ref not in target.read_text(encoding="utf-8", errors="replace"):
            fails.append(f"E_PROFILES_BOOT_ARENA: không thấy luật {ref!r} trong {spec_file}")
    fpu = doc.get("fpu") or {}
    fpu_slots = (fpu.get("slots") or {}).get("value")
    max_tcbs = (((doc.get("limits") or {}).get("core") or {}).get("max_tcbs") or {}).get("value")
    if isinstance(fpu_slots, int) and isinstance(max_tcbs, int):
        if fpu_slots != max_tcbs + 1:
            fails.append(
                f"E_FPU_SLOTS: [fpu].slots phải = max_tcbs + 1 = {max_tcbs + 1}, đang {fpu_slots}"
            )
    else:
        fails.append("E_FPU_SLOTS: thiếu [fpu].slots hoặc [limits.core].max_tcbs")

    # --- `[reserved]`: danh sách tên, không trùng, không rỗng (CẤM sinh/đọc) ---
    reserved = doc.get("reserved", {})
    keys = reserved.get("keys") if isinstance(reserved, dict) else None
    if not isinstance(keys, list) or not keys:
        fails.append("E_RESERVED: [reserved].keys phải là danh sách không rỗng")
    elif len(set(keys)) != len(keys):
        fails.append("E_RESERVED_TRUNG: [reserved].keys có tên trùng")

    # --- `features`: bitmap bit rời nhau ---
    feats = doc.get("features", {})
    bits = []
    if isinstance(feats, dict):
        for name, leaf in feats.items():
            if isinstance(leaf, dict) and "value" in leaf:
                bits.append((name, leaf["value"]))
    if len(bits) != 4:
        fails.append(f"E_FEATURES: phải có đúng 4 bit features, đang {len(bits)}")
    seen = 0
    for name, value in bits:
        if not isinstance(value, int) or value <= 0 or value & (value - 1):
            fails.append(f"E_FEATURES_BIT: features.{name} = {value!r} không phải bit đơn")
        elif seen & value:
            fails.append(f"E_FEATURES_TRUNG: features.{name} = {value} trùng bit khác")
        else:
            seen |= value

    return (fails, checked)


# ------------------------------------------------------------------ selftest


def selftest() -> int:
    try:
        base = SPEC.read_text(encoding="utf-8")
    except FileNotFoundError:
        print(f"not ok 1 - không có {SPEC}")
        return 1

    cases: list[tuple[str, bool, str]] = []

    def add(name: str, ok: bool, detail: str = "") -> None:
        cases.append((name, ok, detail))

    # 1) DƯƠNG: spec thật phải xanh.
    fails, checked = check_spec(SPEC)
    add(
        f"dương: spec/kernel.toml thật xanh ({checked} giá trị)",
        not fails,
        "; ".join(fails)[:400],
    )

    tmp = Path(tempfile.mkdtemp(prefix="kt-kernel-st."))

    def mutate(name: str, old: str, new: str, want: str) -> None:
        """Ca âm: thay `old` → `new`, phải ĐỎ với `want`; không thay được ⇒ ca sai."""
        if old not in base:
            add(name, False, f"không đột biến được (không thấy {old!r})")
            return
        text = base.replace(old, new, 1)
        path = tmp / f"{abs(hash(name))}.toml"
        path.write_text(text, encoding="utf-8")
        fails, _ = check_spec(path)
        joined = "\n".join(fails)
        add(name, any(want in msg for msg in fails), f"muốn {want!r}, nhận: {joined[:300]}")

    # 2) ÂM: status khác APPROVED.
    mutate(
        "âm: status=REVIEW ⇒ E_STATUS",
        '\nstatus = "APPROVED"\nname = "vifast-kernel-config"',
        '\nstatus = "REVIEW"\nname = "vifast-kernel-config"',
        "E_STATUS",
    )
    # 3) ÂM: số trong spec lệch hằng MAX_TCBS.
    mutate(
        "âm: max_tcbs 64→65 ⇒ E_CONST_LECH đúng tên MAX_TCBS",
        "max_tcbs = { value = 64, source = \"v2/kernel/vifast-core/src/machine.rs#MAX_TCBS\" }",
        "max_tcbs = { value = 65, source = \"v2/kernel/vifast-core/src/machine.rs#MAX_TCBS\" }",
        "MAX_TCBS",
    )
    # 4) ÂM: tên hằng không tồn tại trong mã.
    mutate(
        "âm: hằng lạ MAX_TCBS_KHONG_CO ⇒ E_CONST_MISSING",
        "machine.rs#MAX_TCBS",
        "machine.rs#MAX_TCBS_KHONG_CO",
        "E_CONST_MISSING",
    )
    # 5) ÂM: tệp nguồn không tồn tại.
    mutate(
        "âm: tệp nguồn lạ ⇒ E_SOURCE_FILE",
        "v2/kernel/vifast-core/src/machine.rs#MAX_TCBS",
        "v2/kernel/vifast-core/src/khong_co.rs#MAX_TCBS",
        "E_SOURCE_FILE",
    )
    # 6) ÂM: mất cả một mục bắt buộc.
    mutate(
        "âm: xoá [mailbox] ⇒ E_SECTION",
        "[mailbox]",
        "[mailbox_da_xoa]",
        "E_SECTION: thiếu mục [mailbox]",
    )
    # 7) ÂM: một giá trị không có source.
    mutate(
        "âm: giá trị thiếu source ⇒ E_SOURCE_THIEU",
        'max_notifications = { value = 16, source = "v2/kernel/vifast-core/src/machine.rs#MAX_NOTIFICATIONS" }',
        "max_notifications = { value = 16 }",
        "E_SOURCE_THIEU",
    )
    # 8) ÂM: planned order sai (đảo ngược user data/user code về hình học hiện tại).
    mutate(
        "âm: planned.order sai ⇒ E_GDT_PLANNED_ORDER",
        'order = ["null", "kernel_code", "kernel_data", "user_data", "user_code", "tss"]',
        'order = ["null", "kernel_code", "kernel_data", "user_code", "user_data", "tss"]',
        "E_GDT_PLANNED_ORDER",
    )
    # 9) ÂM: `boot_arena` mất `#text:` ⇒ E_PROFILES_BOOT_ARENA (D2-261 nhóm 7).
    mutate(
        "âm: boot_arena mất #text: ⇒ E_PROFILES_BOOT_ARENA",
        "untyped_store.rs#text:",
        "untyped_store.rs#",
        "E_PROFILES_BOOT_ARENA",
    )
    # 10) ÂM: `[fpu].slots` 65→64 ⇒ E_FPU_SLOTS (phải = max_tcbs + 1).
    mutate(
        "âm: [fpu].slots 65→64 ⇒ E_FPU_SLOTS",
        'slots = { value = 65, source = "v2/kernel/vifast-plat-common/src/fpu.rs#FPU_SLOTS" }',
        'slots = { value = 64, source = "v2/kernel/vifast-plat-common/src/fpu.rs#FPU_SLOTS" }',
        "E_FPU_SLOTS",
    )

    for child in tmp.iterdir() if tmp.exists() else []:
        child.unlink(missing_ok=True)
    tmp.rmdir()

    print("TAP version 13")
    print(f"1..{len(cases)}")
    bad = 0
    for index, (name, ok, detail) in enumerate(cases, 1):
        if ok:
            print(f"ok {index} - {name}")
        else:
            bad += 1
            print(f"not ok {index} - {name}" + (f" ({detail})" if detail else ""))
    if bad:
        print(f"# selftest: {bad}/{len(cases)} ca sai kỳ vọng")
        return 1
    print(f"# selftest: {len(cases)}/{len(cases)} ca đúng như kỳ vọng (1 dương + {len(cases) - 1} âm)")
    return 0


# ------------------------------------------------------------------ main


def main() -> int:
    ap = argparse.ArgumentParser(description="Kiểm spec/kernel.toml ↔ mã (D2-242 nhóm 1).")
    ap.add_argument("--file", default=str(SPEC), help="tệp spec (mặc định spec/kernel.toml)")
    ap.add_argument("--selftest", action="store_true", help="ca âm/dương của chính máy kiểm")
    args = ap.parse_args()

    if args.selftest:
        return selftest()

    try:
        fails, checked = check_spec(Path(args.file))
    except Fail as exc:
        print(f"kernel-check: {exc}")
        return 2
    if fails:
        for msg in fails:
            print(f"not ok - {msg}")
        print(f"# {len(fails)} mục sai trong {args.file}")
        return 1
    print(f"kernel-check: PASS ({checked} giá trị đối chiếu theo tên hằng, {args.file})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
