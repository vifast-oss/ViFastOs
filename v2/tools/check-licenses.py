#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-333b (D2-21, QD §9.3) — cổng giấy phép + sinh `THIRD-PARTY.md` từ `cargo metadata`.

Vì sao có tệp này
-----------------
`D2-21` chốt: giấy phép MIT OR Apache-2.0 cho mã của repo, kiểm bằng **allowlist QD §9.3**,
`THIRD-PARTY.md` **sinh tự động**, nhúng license vào ảnh, **không** tạo `NOTICE`. `D2-37(c)` chốt
thêm: `deny.toml` (allowlist QD §9.3) + bước kiểm "không dependency ngoài" phải nằm trong
`v2 check` **trước** dependency ngoài đầu tiên. Tệp này là bước kiểm đó: nó **đọc `deny.toml`**
làm nguồn sự thật duy nhất (cùng tệp `cargo-deny` dùng được khi có công cụ), đối chiếu
`Cargo.lock` ↔ `cargo metadata`, và sinh/kiểm `THIRD-PARTY.md` tất định.

Ba phép kiểm (fail-closed, không bỏ qua)
----------------------------------------
1. **Giấy phép**: mọi gói trong `cargo metadata` (workspace + ngoài) phải có biểu thức SPDX mà
   **ít nhất một nhánh OR** nằm trong allowlist `[licenses].allow` của `deny.toml`. Thiếu giấy
   phép ⇒ ĐỎ (`E_LICENSE_MISSING`). `MIT OR Apache-2.0` hợp lệ theo đúng nghĩa SPDX OR.
2. **Nguồn**: gói không có `source` = đường dẫn/workspace (hợp lệ). Gói từ registry phải khớp
   **đúng** một URL trong `[sources].allow-registry` (chuẩn hoá `registry+`/`sparse+`, gộp
   crates.io git-index ↔ sparse-index vì là **cùng một registry**); gói `git+…` chỉ hợp lệ nếu
   khớp `[sources].allow-git` (mặc định **rỗng** = cấm mọi git dependency).
3. **Khớp lock**: tập `(name, version)` của `Cargo.lock` phải **bằng đúng** tập của metadata —
   lệch một gói là ĐỎ (`E_LOCK_MISMATCH`), không đoán.

`THIRD-PARTY.md` (gốc repo): bảng `tên · phiên bản · giấy phép · nguồn`, sắp xếp tất định, không
chứa thời gian/commit ⇒ cùng cây ⇒ cùng byte. `--check` so byte với tệp đã commit; lệch ⇒
`E_THIRD_PARTY_DRIFT` (CI phát hiện ngay, không có "số đo viết tay").

Dùng
----
    python3.12 v2/tools/check-licenses.py --root . --check      # cổng (mặc định)
    python3.12 v2/tools/check-licenses.py --root . --generate   # ghi THIRD-PARTY.md
    python3.12 v2/tools/check-licenses.py --selftest            # 9 ca (âm + dương), không cargo

Exit: 0 = xanh; 1 = có vi phạm (in mã `E_*`); 2 = dùng sai / thiếu công cụ.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

try:  # Python >= 3.11; fallback tối thiểu cho 3.9 (máy macOS mặc định)
    import tomllib  # type: ignore
except ImportError:  # pragma: no cover - chỉ chạy trên python cũ
    tomllib = None


# ------------------------------------------------------------------ mã lỗi (in ra để ca âm khớp)

E_DENY_MISSING = "E_DENY_MISSING"
E_LICENSE_MISSING = "E_LICENSE_MISSING"
E_LICENSE_NOT_ALLOWED = "E_LICENSE_NOT_ALLOWED"
E_LICENSE_EXPR = "E_LICENSE_EXPR"
E_SOURCE_REGISTRY_DENIED = "E_SOURCE_REGISTRY_DENIED"
E_SOURCE_GIT_DENIED = "E_SOURCE_GIT_DENIED"
E_SOURCE_UNKNOWN = "E_SOURCE_UNKNOWN"
E_LOCK_MISMATCH = "E_LOCK_MISMATCH"
E_THIRD_PARTY_DRIFT = "E_THIRD_PARTY_DRIFT"
E_METADATA = "E_METADATA"
E_CARGO_MISSING = "E_CARGO_MISSING"
E_USAGE = "E_USAGE"

# crates.io có hai dạng index (git-index cũ + sparse-index mới) nhưng là **một** registry.
CRATES_IO_INDEXES = {
    "https://github.com/rust-lang/crates.io-index",
    "https://index.crates.io",
}

THIRD_PARTY = "THIRD-PARTY.md"
DENY = "deny.toml"
LOCK = os.path.join("v2", "Cargo.lock")
MANIFEST = os.path.join("v2", "Cargo.toml")


# ------------------------------------------------------------------ TOML (có fallback 3.9)

def _die_config(path: str, line: int, msg: str) -> None:
    raise SystemExit(f"check-licenses: {path}:{line}: {msg}")


def load_toml(path: str) -> dict:
    """Đọc TOML; fallback chỉ hỗ trợ tập con `Cargo.lock`/`deny.toml` dùng (fail-closed)."""
    raw = open(path, encoding="utf-8").read()
    if tomllib is not None:
        return tomllib.loads(raw)

    data: dict = {}
    cur = data
    arr_key = None
    in_array = False
    for lineno, line in enumerate(raw.splitlines(), 1):
        s = line.strip()
        if in_array:
            if s.endswith("]"):
                in_array = False
            continue
        if not s or s.startswith("#"):
            continue
        m = re.fullmatch(r"\[\[([A-Za-z0-9_.-]+)\]\]", s)
        if m:
            key = m.group(1)
            arr = data.setdefault(key, [])
            if not isinstance(arr, list):
                _die_config(path, lineno, f"'{key}' vừa bảng vừa mảng")
            cur = {}
            arr.append(cur)
            arr_key = key
            continue
        m = re.fullmatch(r"\[([A-Za-z0-9_.-]+)\]", s)
        if m:
            cur = data.setdefault(m.group(1), {})
            arr_key = None
            continue
        m = re.fullmatch(r'([A-Za-z0-9_.-]+)\s*=\s*(.+)', s)
        if not m:
            _die_config(path, lineno, "không hiểu dòng (fallback TOML)")
        key, val = m.group(1), m.group(2).strip()
        if val == "[" or (val.startswith("[") and not val.endswith("]")):
            in_array = True
            val = "[]"
        if val in ("true", "false"):
            cur[key] = val == "true"
        elif re.fullmatch(r"-?\d+", val):
            cur[key] = int(val)
        elif re.fullmatch(r'"[^"]*"', val):
            cur[key] = val[1:-1]
        elif re.fullmatch(r"\[[^\[\]]*\]", val):
            items = [x.strip() for x in val[1:-1].split(",") if x.strip()]
            out = []
            for it in items:
                if not re.fullmatch(r'"[^"]*"', it):
                    _die_config(path, lineno, "mảng chỉ nhận chuỗi (fallback TOML)")
                out.append(it[1:-1])
            cur[key] = out
        else:
            # bỏ qua mảng nhiều dòng kiểu `dependencies = [\n "a",\n]` — chỉ cần name/version.
            if val.startswith("["):
                in_array = True
                cur[key] = []
                continue
            _die_config(path, lineno, "giá trị không hỗ trợ (fallback TOML)")
    return data


# ------------------------------------------------------------------ SPDX expression

_TOKEN = re.compile(r"\s*(\(|\)|AND|OR|WITH|[A-Za-z0-9][A-Za-z0-9.+-]*)")


def _tokenize(expr: str):
    out = []
    pos = 0
    while pos < len(expr):
        m = _TOKEN.match(expr, pos)
        if not m:
            raise ValueError(f"không đọc được biểu thức SPDX tại vị trí {pos}: {expr!r}")
        tok = m.group(1)
        out.append(tok)
        pos = m.end()
        if pos < len(expr) and not expr[pos].isspace() and not _TOKEN.match(expr, pos):
            raise ValueError(f"ký tự lạ trong biểu thức SPDX: {expr[pos]!r} ({expr!r})")
    return out


class _Spdx:
    """Parser nhỏ cho biểu thức SPDX: OR thấp nhất, rồi AND, rồi WITH, rồi ngoặc/atom."""

    def __init__(self, expr: str, allow_lower: set):
        self.expr = expr
        self.allow = allow_lower
        self.tokens = _tokenize(expr)
        self.i = 0

    def peek(self):
        return self.tokens[self.i] if self.i < len(self.tokens) else None

    def take(self):
        tok = self.peek()
        self.i += 1
        return tok

    def parse_or(self) -> bool:
        val = self.parse_and()
        while self.peek() == "OR":
            self.take()
            rhs = self.parse_and()
            val = val or rhs
        return val

    def parse_and(self) -> bool:
        val = self.parse_with()
        while self.peek() == "AND":
            self.take()
            rhs = self.parse_with()
            val = val and rhs
        return val

    def parse_with(self) -> bool:
        val = self.parse_atom()
        if self.peek() == "WITH":
            self.take()
            exc = self.take()
            if exc is None or exc in ("(", ")", "AND", "OR", "WITH"):
                raise ValueError(f"WITH thiếu ngoại lệ trong {self.expr!r}")
            # Ngoại lệ chỉ hợp lệ nếu **nguyên cụm** có trong allowlist (hiện allowlist rỗng
            # ngoại lệ ⇒ mọi `WITH` đều ĐỎ, đúng chính sách fail-closed).
            combo = f"{val[1]} WITH {exc}".lower() if isinstance(val, tuple) else ""
            return isinstance(val, tuple) and combo in self.allow
        return val[0] if isinstance(val, tuple) else val

    def parse_atom(self) -> bool:
        tok = self.take()
        if tok == "(":
            val = self.parse_or()
            if self.take() != ")":
                raise ValueError(f"thiếu ')' trong {self.expr!r}")
            return val
        if tok is None or tok in ("AND", "OR", "WITH", ")"):
            raise ValueError(f"thiếu toán hạng trong {self.expr!r}")
        return (tok.lower() in self.allow, tok)


def license_allowed(expr: str, allow: list) -> bool:
    allow_lower = {a.lower() for a in allow}
    parser = _Spdx(expr, allow_lower)
    val = parser.parse_or()
    if parser.peek() is not None:
        raise ValueError(f"biểu thức SPDX thừa token {parser.peek()!r}: {expr!r}")
    return bool(val)


# ------------------------------------------------------------------ nguồn

def norm_source(src: str) -> str:
    s = src.strip()
    for prefix in ("registry+", "sparse+", "git+"):
        if s.startswith(prefix):
            s = s[len(prefix):]
            break
    s = s.split("#", 1)[0].split("?", 1)[0].rstrip("/")
    return s.lower()


def source_kind(src: str) -> str:
    if src.startswith("registry+") or src.startswith("sparse+"):
        return "registry"
    if src.startswith("git+"):
        return "git"
    return "unknown"


def registry_allowed(url: str, allow: list) -> bool:
    nu = norm_source(url)
    allowed = {norm_source(a) for a in allow}
    if nu in allowed:
        return True
    # crates.io: git-index và sparse-index là cùng registry (cargo chọn index theo config).
    if nu in CRATES_IO_INDEXES and (allowed & CRATES_IO_INDEXES):
        return True
    return False


def git_allowed(url: str, allow: list) -> bool:
    nu = norm_source(url)
    return nu in {norm_source(a) for a in allow}


# ------------------------------------------------------------------ audit

def _load_metadata(root: str, metadata_path: str | None) -> dict:
    if metadata_path:
        with open(metadata_path, encoding="utf-8") as fh:
            return json.load(fh)
    cargo = shutil.which("cargo")
    if not cargo:
        for cand in (
            os.path.expanduser("~/.cargo/bin/cargo"),
            "/usr/local/cargo/bin/cargo",
            "/opt/homebrew/bin/cargo",
        ):
            if os.path.isfile(cand) and os.access(cand, os.X_OK):
                cargo = cand
                break
    if not cargo:
        raise SystemExit(f"{E_CARGO_MISSING}: không tìm thấy cargo (đã thử PATH, ~/.cargo/bin)")
    env = dict(os.environ)
    tc = os.path.join(root, "v2", "rust-toolchain.toml")
    if not env.get("RUSTUP_TOOLCHAIN") and os.path.isfile(tc):
        for line in open(tc, encoding="utf-8"):
            m = re.match(r'^\s*channel\s*=\s*"([^"]+)"', line)
            if m:
                env["RUSTUP_TOOLCHAIN"] = m.group(1)
                break
    proc = subprocess.run(
        [cargo, "metadata", "--format-version", "1", "--locked",
         "--manifest-path", os.path.join(root, MANIFEST)],
        capture_output=True, text=True, env=env, timeout=600,
    )
    if proc.returncode != 0:
        raise SystemExit(f"{E_METADATA}: `cargo metadata --locked` đỏ:\n{proc.stderr.strip()[:2000]}")
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{E_METADATA}: cargo metadata trả JSON hỏng: {exc}")


def audit(root: str, metadata_path: str | None = None) -> tuple[list, dict]:
    """Trả (lỗi, thông tin). Mỗi lỗi: (mã, mô tả)."""
    errors: list = []
    info: dict = {"packages": [], "workspace": set(), "allow": [], "allow_registry": [],
                  "allow_git": [], "external": 0}

    deny_path = os.path.join(root, DENY)
    if not os.path.isfile(deny_path):
        errors.append((E_DENY_MISSING, f"không có `{DENY}` ở gốc repo"))
        return errors, info
    deny = load_toml(deny_path)
    allow = (deny.get("licenses") or {}).get("allow") or []
    if not isinstance(allow, list) or not allow:
        errors.append((E_DENY_MISSING, f"`{DENY}` thiếu `[licenses].allow` (allowlist QD §9.3)"))
    sources = deny.get("sources") or {}
    allow_registry = sources.get("allow-registry") or []
    allow_git = sources.get("allow-git") or []
    if not isinstance(allow_registry, list) or not allow_registry:
        errors.append((E_DENY_MISSING,
                       f"`{DENY}` thiếu `[sources].allow-registry` (fail-closed: cần allowlist tường minh)"))
    if not isinstance(allow_git, list):
        errors.append((E_DENY_MISSING, f"`{DENY}` `[sources].allow-git` phải là mảng"))
        allow_git = []
    info.update(allow=allow, allow_registry=allow_registry, allow_git=allow_git)
    if errors:
        return errors, info

    lock_path = os.path.join(root, LOCK)
    if not os.path.isfile(lock_path):
        errors.append((E_LOCK_MISMATCH, f"không có `{LOCK}`"))
        return errors, info
    lock = load_toml(lock_path)
    lock_set = {(p.get("name"), p.get("version")) for p in lock.get("package", [])}
    if None in lock_set or any(not n or not v for n, v in lock_set):
        errors.append((E_LOCK_MISMATCH, f"`{LOCK}` có mục thiếu name/version"))
        return errors, info

    meta = _load_metadata(root, metadata_path)
    pkgs = meta.get("packages") or []
    ws = set(meta.get("workspace_members") or [])
    meta_set = {(p.get("name"), p.get("version")) for p in pkgs}
    info["packages"] = pkgs
    info["workspace"] = ws
    info["external"] = sum(1 for p in pkgs if p.get("source"))

    for name, version in sorted(lock_set - meta_set):
        errors.append((E_LOCK_MISMATCH, f"`{LOCK}` có `{name} {version}` nhưng metadata không có"))
    for name, version in sorted(meta_set - lock_set):
        errors.append((E_LOCK_MISMATCH, f"metadata có `{name} {version}` nhưng `{LOCK}` không có"))

    for p in sorted(pkgs, key=lambda x: (x.get("name") or "", x.get("version") or "")):
        name = p.get("name") or "?"
        version = p.get("version") or "?"
        expr = p.get("license")
        if not expr or not str(expr).strip():
            hint = " (có `license-file` — vẫn phải khai báo giấy phép SPDX)" if p.get("license_file") else ""
            errors.append((E_LICENSE_MISSING, f"`{name} {version}` thiếu trường giấy phép SPDX{hint}"))
        else:
            try:
                ok = license_allowed(str(expr), allow)
            except ValueError as exc:
                errors.append((E_LICENSE_EXPR, f"`{name} {version}`: {exc}"))
            else:
                if not ok:
                    errors.append((E_LICENSE_NOT_ALLOWED,
                                   f"`{name} {version}`: giấy phép `{expr}` ngoài allowlist QD §9.3 "
                                   f"({', '.join(allow)})"))
        src = p.get("source")
        if not src:
            continue
        kind = source_kind(src)
        if kind == "registry" and not registry_allowed(src, allow_registry):
            errors.append((E_SOURCE_REGISTRY_DENIED,
                           f"`{name} {version}`: registry `{src}` ngoài `allow-registry` "
                           f"({', '.join(allow_registry)})"))
        elif kind == "git" and not git_allowed(src, allow_git):
            errors.append((E_SOURCE_GIT_DENIED,
                           f"`{name} {version}`: git-source `{src}` ngoài `allow-git` "
                           f"({', '.join(allow_git) or 'rỗng'})"))
        elif kind == "unknown":
            errors.append((E_SOURCE_UNKNOWN, f"`{name} {version}`: source lạ `{src}`"))
    return errors, info


# ------------------------------------------------------------------ THIRD-PARTY.md

def render_third_party(info: dict) -> str:
    pkgs = info["packages"]
    ws = info["workspace"]
    lines = [
        "<!-- SINH TỰ ĐỘNG bởi `v2/tools/check-licenses.py` từ `cargo metadata` — KHÔNG SỬA TAY. -->",
        "<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->",
        "",
        "# Third-party — ViFastOs v2",
        "",
        "Tệp này do máy sinh (`./v2/bin/v2 licenses --generate`); `./v2/bin/v2 check` đỏ nếu lệch.",
        "Nguồn: `cargo metadata --locked` của workspace `v2/Cargo.toml`; allowlist: `deny.toml`",
        "(QD §9.3, D2-21). Không tạo `NOTICE` (D2-21).",
        "",
        "| Gói | Phiên bản | Giấy phép | Nguồn |",
        "|---|---|---|---|",
    ]
    for p in sorted(pkgs, key=lambda x: ((x.get("name") or ""), (x.get("version") or ""))):
        name = p.get("name") or "?"
        version = p.get("version") or "?"
        expr = p.get("license") or "—"
        src = p.get("source")
        if not src and p.get("id") in ws:
            source = "workspace (nội bộ)"
        elif not src:
            source = "path (nội bộ)"
        else:
            source = src
        lines.append(f"| `{name}` | `{version}` | `{expr}` | `{source}` |")
    total = len(pkgs)
    external = info["external"]
    lines += [
        "",
        f"Tổng: **{total}** gói (nội bộ/workspace: **{total - external}**, từ registry/git: **{external}**).",
        "",
    ]
    return "\n".join(lines)


# ------------------------------------------------------------------ lệnh

def _in_allowlist_verbatim(info: dict) -> str:
    return ", ".join(info["allow"])


def cmd_check(root: str) -> int:
    errors, info = audit(root)
    if errors:
        for code, msg in errors:
            print(f"{code} {msg}", file=sys.stderr)
        print(f"check-licenses: FAIL ({len(errors)} vi phạm)", file=sys.stderr)
        return 1
    third = os.path.join(root, THIRD_PARTY)
    expected = render_third_party(info)
    if not os.path.isfile(third):
        print(f"{E_THIRD_PARTY_DRIFT} thiếu `{THIRD_PARTY}` — chạy `./v2/bin/v2 licenses --generate`",
              file=sys.stderr)
        return 1
    got = open(third, encoding="utf-8").read()
    if got != expected:
        print(f"{E_THIRD_PARTY_DRIFT} `{THIRD_PARTY}` lệch bản sinh từ `cargo metadata` — "
              f"chạy `./v2/bin/v2 licenses --generate`", file=sys.stderr)
        return 1
    total = len(info["packages"])
    print(f"check-licenses: PASS ({total} gói: {total - info['external']} nội bộ + "
          f"{info['external']} registry/git; allowlist: {_in_allowlist_verbatim(info)}; "
          f"registry: {', '.join(info['allow_registry'])}; git: {', '.join(info['allow_git']) or 'rỗng'}; "
          f"0 vi phạm; {THIRD_PARTY}: khớp)")
    return 0


def cmd_generate(root: str) -> int:
    errors, info = audit(root)
    if errors:
        for code, msg in errors:
            print(f"{code} {msg}", file=sys.stderr)
        print("check-licenses: KHÔNG sinh THIRD-PARTY.md khi còn vi phạm", file=sys.stderr)
        return 1
    path = os.path.join(root, THIRD_PARTY)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(render_third_party(info))
    total = len(info["packages"])
    print(f"check-licenses: đã ghi `{THIRD_PARTY}` ({total} gói, {info['external']} từ registry/git); "
          f"allowlist: {_in_allowlist_verbatim(info)}")
    return 0


# ------------------------------------------------------------------ tự kiểm (9 ca, không cargo)

SELFTEST_ALLOW = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC",
                  "0BSD", "Zlib", "Unicode-3.0", "Unicode-DFS-2016"]


def _fixture(root: str, deny: dict | None, lock: dict, meta: dict, third: str | None = None):
    os.makedirs(os.path.join(root, "v2"), exist_ok=True)
    import json as _json

    with open(os.path.join(root, LOCK), "w", encoding="utf-8") as fh:
        lines = []
        for p in lock["package"]:
            lines.append("[[package]]")
            lines.append(f'name = "{p["name"]}"')
            lines.append(f'version = "{p["version"]}"')
            if p.get("source"):
                lines.append(f'source = "{p["source"]}"')
            lines.append("")
        fh.write("\n".join(lines))
    with open(os.path.join(root, "meta.json"), "w", encoding="utf-8") as fh:
        _json.dump(meta, fh)
    if deny is not None:
        with open(os.path.join(root, DENY), "w", encoding="utf-8") as fh:
            fh.write("[licenses]\nallow = [" + ", ".join(f'"{a}"' for a in deny["allow"]) + "]\n\n")
            fh.write("[sources]\n")
            fh.write("unknown-registry = \"deny\"\nunknown-git = \"deny\"\n")
            fh.write("allow-registry = [" + ", ".join(f'"{a}"' for a in deny["registry"]) + "]\n")
            fh.write("allow-git = [" + ", ".join(f'"{a}"' for a in deny["git"]) + "]\n")
    if third is not None:
        with open(os.path.join(root, THIRD_PARTY), "w", encoding="utf-8") as fh:
            fh.write(third)


def _pkg(name, version, license, source=None, wid=None):
    return {"name": name, "version": version, "license": license, "source": source,
            "id": wid or f"path+file:///{name}#{name}@{version}"}


def _meta(pkgs):
    ids = [p["id"] for p in pkgs]
    return {"packages": pkgs, "workspace_members": ids, "workspace_default_members": ids}


def cmd_selftest() -> int:
    """9 ca: 3 dương + 6 âm; ca âm đỏ **đúng mã** (không phải đỏ vô tội vạ)."""

    CASES = [
        ("dương: workspace `MIT OR Apache-2.0` + registry crates.io ⇒ XANH", []),
        ("dương: `GPL-3.0-only OR MIT` XANH, `GPL AND MIT` ĐỎ (đúng nghĩa SPDX)",
         [E_LICENSE_NOT_ALLOWED]),
        ("âm: `GPL-3.0-only` ⇒ E_LICENSE_NOT_ALLOWED", [E_LICENSE_NOT_ALLOWED]),
        ("âm: thiếu license ⇒ E_LICENSE_MISSING", [E_LICENSE_MISSING]),
        ("âm: registry lạ ⇒ E_SOURCE_REGISTRY_DENIED", [E_SOURCE_REGISTRY_DENIED]),
        ("âm: git dependency ⇒ E_SOURCE_GIT_DENIED", [E_SOURCE_GIT_DENIED]),
        ("âm: Cargo.lock lệch metadata ⇒ E_LOCK_MISMATCH", [E_LOCK_MISMATCH]),
        ("âm: thiếu deny.toml ⇒ E_DENY_MISSING", [E_DENY_MISSING]),
    ]

    def fixture(case: int, root: str) -> tuple:
        allow = {"allow": SELFTEST_ALLOW,
                 "registry": ["https://github.com/rust-lang/crates.io-index"], "git": []}
        crates = "registry+https://github.com/rust-lang/crates.io-index"
        if case == 0:
            _fixture(root, allow,
                     {"package": [{"name": "vifast-abi", "version": "0.1.0"}]},
                     _meta([_pkg("vifast-abi", "0.1.0", "MIT OR Apache-2.0", wid="ws-abi")]))
        elif case == 1:
            _fixture(root, allow,
                     {"package": [{"name": "a", "version": "1.0.0"},
                                  {"name": "b", "version": "1.0.0"},
                                  {"name": "c", "version": "1.0.0"}]},
                     _meta([_pkg("a", "1.0.0", "GPL-3.0-only OR MIT"),
                            _pkg("b", "1.0.0", "MIT AND Apache-2.0"),
                            _pkg("c", "1.0.0", "GPL-3.0-only AND MIT")]))
        elif case == 2:
            _fixture(root, allow,
                     {"package": [{"name": "evil", "version": "1.0.0", "source": crates}]},
                     _meta([_pkg("evil", "1.0.0", "GPL-3.0-only", source=crates)]))
        elif case == 3:
            _fixture(root, allow,
                     {"package": [{"name": "nolicense", "version": "2.0.0"}]},
                     _meta([_pkg("nolicense", "2.0.0", None)]))
        elif case == 4:
            _fixture(root, allow,
                     {"package": [{"name": "far", "version": "1.0.0",
                                   "source": "registry+https://evil.example/index"}]},
                     _meta([_pkg("far", "1.0.0", "MIT",
                                 source="registry+https://evil.example/index")]))
        elif case == 5:
            _fixture(root, allow,
                     {"package": [{"name": "fromgit", "version": "1.0.0",
                                   "source": "git+https://github.com/x/y#abcdef"}]},
                     _meta([_pkg("fromgit", "1.0.0", "MIT",
                                 source="git+https://github.com/x/y#abcdef")]))
        elif case == 6:
            _fixture(root, allow,
                     {"package": [{"name": "a", "version": "1.0.0"},
                                  {"name": "ghost", "version": "9.9.9"}]},
                     _meta([_pkg("a", "1.0.0", "MIT")]))
        else:
            _fixture(root, None,
                     {"package": [{"name": "a", "version": "1.0.0"}]},
                     _meta([_pkg("a", "1.0.0", "MIT")]))
        return audit(root, metadata_path=os.path.join(root, "meta.json"))

    print("TAP version 13")
    print(f"1..{len(CASES) + 1}")
    fails = 0
    n = 0
    for i, (name, want_codes) in enumerate(CASES):
        n += 1
        tmp = tempfile.mkdtemp(prefix="kt333b-lic-")
        try:
            errors, _info = fixture(i, tmp)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
        got = [c for c, _m in errors]
        # `GPL AND MIT` chỉ sinh đúng **một** lỗi; các ca khác cũng đúng một mã.
        if got == want_codes:
            print(f"ok {n} - {name}")
        else:
            print(f"not ok {n} - {name} (mã nhận {got or 'không có'}, muốn {want_codes or 'không có'})")
            fails += 1

    # Ca dương cuối: `THIRD-PARTY.md` tất định; lệch byte ⇒ đúng `E_THIRD_PARTY_DRIFT`.
    n += 1
    tmp = tempfile.mkdtemp(prefix="kt333b-lic-")
    try:
        _fixture(tmp,
                 {"allow": SELFTEST_ALLOW,
                  "registry": ["https://github.com/rust-lang/crates.io-index"], "git": []},
                 {"package": [{"name": "a", "version": "1.0.0"}]},
                 _meta([_pkg("a", "1.0.0", "MIT")]))
        _errors, info = audit(tmp, metadata_path=os.path.join(tmp, "meta.json"))
        first = render_third_party(info)
        second = render_third_party(info)
        with open(os.path.join(tmp, THIRD_PARTY), "w", encoding="utf-8") as fh:
            fh.write(first)
        code_ok = cmd_check_quiet(tmp)
        with open(os.path.join(tmp, THIRD_PARTY), "a", encoding="utf-8") as fh:
            fh.write("| `them` | `9.9.9` | `MIT` | `fake` |\n")
        code_drift = cmd_check_quiet(tmp)
        if first == second and code_ok == "OK" and code_drift == E_THIRD_PARTY_DRIFT:
            print(f"ok {n} - dương: THIRD-PARTY.md tất định; lệch byte ⇒ E_THIRD_PARTY_DRIFT")
        else:
            print(f"not ok {n} - dương: THIRD-PARTY.md tất định; lệch byte ⇒ E_THIRD_PARTY_DRIFT "
                  f"(deterministic={first == second}, check={code_ok}, drift={code_drift})")
            fails += 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    total = len(CASES) + 1
    if fails:
        print(f"check-licenses --selftest: FAIL ({fails}/{total})", file=sys.stderr)
        return 1
    print(f"check-licenses --selftest: PASS ({total}/{total})")
    return 0


def cmd_check_quiet(root: str) -> str:
    """Chạy `--check` nhưng trả **mã lỗi đầu tiên** thay vì in/exit (dùng cho selftest)."""
    meta = os.path.join(root, "meta.json")
    errors, info = audit(root, metadata_path=meta if os.path.isfile(meta) else None)
    if errors:
        return errors[0][0]
    third = os.path.join(root, THIRD_PARTY)
    expected = render_third_party(info)
    if not os.path.isfile(third):
        return E_THIRD_PARTY_DRIFT
    if open(third, encoding="utf-8").read() != expected:
        return E_THIRD_PARTY_DRIFT
    return "OK"


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(add_help=True, description=__doc__)
    ap.add_argument("--root", default=None, help="gốc repo (mặc định: hai cấp trên tệp này)")
    ap.add_argument("--check", action="store_true", help="cổng: allowlist + nguồn + lock + THIRD-PARTY khớp")
    ap.add_argument("--generate", action="store_true", help="ghi THIRD-PARTY.md từ cargo metadata")
    ap.add_argument("--selftest", action="store_true", help="9 ca tự kiểm (không cargo/sim)")
    ap.add_argument("--metadata", default=None, help="(chẩn đoán) đọc metadata JSON thay vì chạy cargo")
    args = ap.parse_args(argv)

    root = args.root or os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    root = os.path.abspath(root)
    if not os.path.isdir(os.path.join(root, "v2")):
        print(f"{E_USAGE}: `{root}` không phải gốc repo (thiếu v2/)", file=sys.stderr)
        return 2
    if args.selftest:
        return cmd_selftest()
    modes = [bool(args.check), bool(args.generate)]
    if sum(modes) != 1:
        print(f"{E_USAGE}: chọn đúng một trong --check/--generate/--selftest", file=sys.stderr)
        return 2
    if args.generate:
        return cmd_generate(root)
    if args.metadata:
        errors, info = audit(root, metadata_path=args.metadata)
        if errors:
            for code, msg in errors:
                print(f"{code} {msg}", file=sys.stderr)
            return 1
        print("check-licenses: PASS (metadata chẩn đoán)")
        return 0
    return cmd_check(root)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
