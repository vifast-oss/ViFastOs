#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-203 (review L41) — phép đo ĐỘC LẬP cho KT-182.

Không import `v2/tools/check-test-teeth.py`, không dùng `measure-local-map.py` của tác giả.
Tự che bình luận/chuỗi, tự tìm `fn`, tự dựng phạm vi test, tự dựng hai bản đồ trợ giúp
(cùng tệp "cũ" = mọi hàm; cùng tệp "mới" = chỉ hàm trong phạm vi test) và bản đồ toàn cây.

Câu hỏi:
  Q1. Có `#[test]` nào **chỉ** được chứng nhận nhờ hàm **sản phẩm cùng tệp** không?
  Q2. Bao nhiêu `#[test]`? Bao nhiêu gọi tên hàm sản phẩm cùng tệp? Bao nhiêu dựa trợ giúp
      cùng tệp trong phạm vi test?
  Q3. Sau khi siết, test nào đổi từ XANH sang ĐỎ? (đối chứng ngược: báo oan)
  Q4. Hàm `#[cfg(test)]` đặt trên **item** (không phải `mod`) — checker có bỏ sót không?
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set, Tuple

ASSERT_TOKENS: Tuple[str, ...] = (
    "assert!",
    "assert_eq!",
    "assert_ne!",
    "assert_matches!",
    "debug_assert!",
    "debug_assert_eq!",
    "debug_assert_ne!",
    "expect_err",
    "unwrap_err",
)
CONTROL_FLOW = re.compile(r"\b(?:if|match|else|while|for|loop)\b")
CALL_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*(?:::\s*<[^;{}()]*?>)?\s*[(!]")
FN_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")
TEST_ATTR_RE = re.compile(r"^(?:[A-Za-z_][A-Za-z0-9_]*::)*test$")
SHOULD_PANIC_RE = re.compile(r'^should_panic\s*\(\s*expected\s*=\s*"[^"]+"')
USE_ALIAS_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s+as\s+([A-Za-z_][A-Za-z0-9_]*)")

SKIP_DIRS = {"target", "compile-fail"}
SKIP_PREFIXES = ("v2/test/fixtures/",)


# ------------------------------------------------------------------ che bình luận/chuỗi


def mask(src: str) -> str:
    """Che nội dung bình luận + chuỗi/ký tự bằng dấu cách, giữ nguyên độ dài và số dòng."""
    out = list(src)
    n = len(src)
    i = 0
    while i < n:
        c = src[i]
        if c == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
            continue
        if c == "/" and i + 1 < n and src[i + 1] == "*":
            depth = 1
            j = i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            for k in range(i, j):
                if src[k] != "\n":
                    out[k] = " "
            i = j
            continue
        # raw string: r"…", r#"…"#, br#"…"#, cr#"…"#
        m = re.match(r'(?:b|c)?r(#*)"', src[i:])
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            hashes = m.group(1)
            close = '"' + hashes
            j = src.find(close, i + m.end())
            j = n if j < 0 else j + len(close)
            for k in range(i, j):
                if src[k] != "\n":
                    out[k] = " "
            i = j
            continue
        # string thường: "…" (kể cả b"…", c"…")
        if c == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            for k in range(i + 1, max(i + 1, j - 1)):
                if src[k] != "\n":
                    out[k] = " "
            i = j
            continue
        # ký tự: 'x' / '\n' — nhưng KHÔNG phải lifetime 'a
        if c == "'":
            mm = re.match(r"'(\\.|[^\\'])'", src[i:])
            if mm:
                for k in range(i + 1, i + mm.end() - 1):
                    if src[k] != "\n":
                        out[k] = " "
                i += mm.end()
                continue
            i += 1
            continue
        i += 1
    return "".join(out)


# ------------------------------------------------------------------ tìm hàm


class Fn:
    __slots__ = ("name", "start", "body_start", "body_end", "attrs")

    def __init__(self, name, start, body_start, body_end, attrs):
        self.name = name
        self.start = start
        self.body_start = body_start
        self.body_end = body_end
        self.attrs = attrs

    @property
    def is_test(self) -> bool:
        return any(TEST_ATTR_RE.match(a.strip()) for a in self.attrs)

    @property
    def is_should_panic(self) -> bool:
        return any(a.strip().startswith("should_panic") for a in self.attrs)

    @property
    def should_panic_has_expected(self) -> bool:
        return any(SHOULD_PANIC_RE.match(a.strip()) for a in self.attrs)


def match_brace(m: str, open_pos: int) -> int:
    depth = 0
    k = open_pos
    n = len(m)
    while k < n:
        o = m.find("{", k)
        c = m.find("}", k)
        if c < 0:
            return -1
        if 0 <= o < c:
            depth += 1
            k = o + 1
        else:
            depth -= 1
            if depth == 0:
                return c
            k = c + 1
    return -1


def collect_attrs(m: str, pos: int) -> List[str]:
    attrs: List[str] = []
    i = pos
    while True:
        j = i
        while j > 0 and m[j - 1] in " \t\n":
            j -= 1
        if j == 0 or m[j - 1] != "]":
            break
        depth = 0
        k = j - 1
        while k >= 0:
            if m[k] == "]":
                depth += 1
            elif m[k] == "[":
                depth -= 1
                if depth == 0:
                    break
            k -= 1
        if k < 0:
            break
        h = k - 1
        while h >= 0 and m[h] in " \t\n":
            h -= 1
        if h < 0 or m[h] != "#":
            break
        attrs.append(m[k + 1 : j - 1].strip())
        i = h
    attrs.reverse()
    return attrs


def find_fns(m: str) -> List[Fn]:
    out: List[Fn] = []
    for mt in FN_RE.finditer(m):
        j = mt.end()
        depth = 0
        found = False
        while j < len(m):
            ch = m[j]
            if ch == "-" and j + 1 < len(m) and m[j + 1] == ">":
                j += 2
                continue
            if ch in "([<":
                depth += 1
            elif ch in ")]>":
                depth -= 1
            elif ch == "{" and depth <= 0:
                found = True
                break
            elif ch == ";" and depth <= 0:
                break
            j += 1
        if not found:
            continue
        k = match_brace(m, j)
        if k < 0:
            continue
        out.append(Fn(mt.group(1), mt.start(), j, k, collect_attrs(m, mt.start())))
    return out


def top_level(fns: Sequence[Fn]) -> List[Fn]:
    return [
        f
        for f in fns
        if not any(g.body_start < f.start < g.body_end for g in fns if g is not f)
    ]


# ------------------------------------------------------------------ phạm vi test


CFG_RE = re.compile(r"#\s*\[\s*cfg\s*\(")
MOD_RE = re.compile(r"mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([{;])")


def cfg_items(m: str) -> List[Tuple[str, str, Optional[Tuple[int, int]]]]:
    """`#[cfg(…test…)]` + item theo sau: ('mod_inline'|'mod_file'|'item', tên, khoảng)."""
    out: List[Tuple[str, str, Optional[Tuple[int, int]]]] = []
    for mt in CFG_RE.finditer(m):
        i = mt.end() - 1
        depth = 0
        j = i
        while j < len(m):
            if m[j] == "(":
                depth += 1
            elif m[j] == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        if j >= len(m):
            continue
        cfg_text = m[i + 1 : j]
        if not re.search(r"\btest\b", cfg_text):
            continue
        if re.search(r"\bnot\s*\(\s*test\s*\)", cfg_text):
            continue
        k = j + 1
        while k < len(m) and m[k] in " \t\n":
            k += 1
        if k >= len(m) or m[k] != "]":
            continue
        k += 1
        # bỏ qua thuộc tính khác
        while True:
            while k < len(m) and m[k] in " \t\n":
                k += 1
            if k < len(m) and m[k] == "#":
                k2 = k + 1
                while k2 < len(m) and m[k2] in " \t\n":
                    k2 += 1
                if k2 < len(m) and m[k2] == "[":
                    d2 = 0
                    k3 = k2
                    while k3 < len(m):
                        if m[k3] == "[":
                            d2 += 1
                        elif m[k3] == "]":
                            d2 -= 1
                            if d2 == 0:
                                break
                        k3 += 1
                    k = k3 + 1
                    continue
            break
        mm = MOD_RE.match(m, k)
        if mm:
            name = mm.group(1)
            if mm.group(2) == ";":
                out.append(("mod_file", name, None))
            else:
                b = mm.end() - 1
                e = match_brace(m, b)
                if e >= 0:
                    out.append(("mod_inline", name, (b, e)))
            continue
        # item khác (fn / impl / struct / …) — ghi nhận để báo cáo, không dùng cho scope hẹp
        im = re.match(r"(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:async\s+)?(fn|impl|struct|enum|trait|const|static|type|use|mod)\b", m[k:])
        if im:
            kind = im.group(1)
            # tên gần đúng
            tail = m[k : k + 200]
            nm = re.search(r"\b(?:fn|struct|enum|trait|const|static|type)\s+([A-Za-z_][A-Za-z0-9_]*)", tail)
            out.append(("item", nm.group(1) if nm else kind, None))
    return out


def in_ranges(pos: int, ranges: Sequence[Tuple[int, int]]) -> bool:
    return any(a <= pos <= b for a, b in ranges)


class Parsed:
    __slots__ = ("path", "rel", "masked", "fns", "line_starts")

    def __init__(self, path, rel, masked, fns, line_starts):
        self.path = path
        self.rel = rel
        self.masked = masked
        self.fns = fns
        self.line_starts = line_starts

    def line_of(self, pos: int) -> int:
        import bisect

        return bisect.bisect_right(self.line_starts, pos)


def iter_files(root: Path) -> List[Path]:
    base = root / "v2"
    out = []
    for p in sorted(base.rglob("*.rs")):
        rel = str(p.relative_to(root))
        if any(part in SKIP_DIRS for part in p.relative_to(root).parts):
            continue
        if any(rel.startswith(pre) for pre in SKIP_PREFIXES):
            continue
        out.append(p)
    return out


def parse(p: Path, root: Path) -> Parsed:
    src = p.read_text(encoding="utf-8", errors="replace")
    m = mask(src)
    starts = [0]
    idx = m.find("\n")
    while idx >= 0:
        starts.append(idx + 1)
        idx = m.find("\n", idx + 1)
    return Parsed(p, str(p.relative_to(root)), m, find_fns(m), starts)


def build_scope(parsed: Sequence[Parsed]) -> Tuple[Set[Path], Dict[Path, List[Tuple[int, int]]], List[dict]]:
    whole: Set[Path] = set()
    inline: Dict[Path, List[Tuple[int, int]]] = {}
    item_cfg: List[dict] = []
    for pf in parsed:
        for kind, name, rng in cfg_items(pf.masked):
            if kind == "mod_inline" and rng is not None:
                inline.setdefault(pf.path, []).append(rng)
            elif kind == "mod_file":
                for cand in (pf.path.parent / f"{name}.rs", pf.path.parent / name / "mod.rs"):
                    if cand.exists():
                        whole.add(cand)
            elif kind == "item":
                item_cfg.append({"file": pf.rel, "item": name})
        # `#![cfg(test)]` ở đầu tệp ⇒ cả tệp là phạm vi test
        if re.search(r"#!\s*\[\s*cfg\s*\([^)]*\btest\b", pf.masked):
            whole.add(pf.path)
    for pf in parsed:
        relparts = tuple(pf.rel.split("/"))
        if "tests" in relparts[:-1]:
            whole.add(pf.path)
    return whole, inline, item_cfg


# ------------------------------------------------------------------ bản đồ trợ giúp


def body_of(pf: Parsed, f: Fn) -> str:
    return pf.masked[f.body_start : f.body_end]


def own_body(pf: Parsed, f: Fn, fns: Sequence[Fn]) -> str:
    body = body_of(pf, f)
    own = list(body)
    for g in fns:
        if g is f:
            continue
        if f.body_start < g.start < f.body_end:
            a = g.body_start - f.body_start
            b = g.body_end - f.body_start
            for k in range(max(0, a), min(len(own), b + 1)):
                if own[k] != "\n":
                    own[k] = " "
    return "".join(own)


def direct_teeth(body: str) -> bool:
    return any(t in body for t in ASSERT_TOKENS) or (
        "panic!" in body and bool(CONTROL_FLOW.search(body))
    )


def build_local(pf: Parsed, is_whole: bool, ranges, scoped: bool):
    helper: Dict[str, bool] = {}
    calls: Dict[str, Set[str]] = {}
    for f in pf.fns:
        if f.is_test:
            continue
        if scoped and not (is_whole or in_ranges(f.start, ranges)):
            continue
        helper[f.name] = direct_teeth(body_of(pf, f))
    for f in pf.fns:
        if scoped and not (is_whole or in_ranges(f.start, ranges)):
            continue
        calls[f.name] = set(CALL_RE.findall(body_of(pf, f)))
    return helper, calls


def teeth_fn(helper: Dict[str, bool], calls: Dict[str, Set[str]]):
    def teeth(name: str, seen: Optional[Set[str]] = None) -> bool:
        seen = set() if seen is None else seen
        if name in seen:
            return False
        seen.add(name)
        if helper.get(name):
            return True
        for c in calls.get(name, ()):
            if c in helper and teeth(c, seen):
                return True
        return False

    return teeth


def build_global(parsed: Sequence[Parsed], whole, inline) -> Set[str]:
    direct: Dict[str, List[bool]] = {}
    calls: Dict[str, List[Set[str]]] = {}
    for pf in parsed:
        is_whole = pf.path in whole
        ranges = inline.get(pf.path, [])
        if not is_whole and not ranges:
            continue
        for f in pf.fns:
            if f.is_test:
                continue
            if not (is_whole or in_ranges(f.start, ranges)):
                continue
            b = body_of(pf, f)
            direct.setdefault(f.name, []).append(direct_teeth(b))
            calls.setdefault(f.name, []).append(set(CALL_RE.findall(b)))
    teeth = {n for n, ds in direct.items() if all(ds)}
    changed = True
    while changed:
        changed = False
        for n, ds in direct.items():
            if n in teeth:
                continue
            ok = True
            for i, has in enumerate(ds):
                if has:
                    continue
                if not any(c in teeth for c in calls[n][i]):
                    ok = False
                    break
            if ok:
                teeth.add(n)
                changed = True
    return teeth


def use_aliases(parsed: Sequence[Parsed], scope: Set[Path]) -> Dict[Path, Dict[str, str]]:
    out: Dict[Path, Dict[str, str]] = {}
    for pf in parsed:
        if pf.path not in scope:
            continue
        al: Dict[str, str] = {}
        for mt in re.finditer(r"\buse\s+([^;]+);", pf.masked):
            for am in USE_ALIAS_RE.finditer(mt.group(1)):
                al[am.group(2)] = am.group(1)
        if al:
            out[pf.path] = al
    return out


# ------------------------------------------------------------------ đo


def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=None)
    ap.add_argument("--json", default=None)
    args = ap.parse_args(argv)
    root = Path(args.root).resolve() if args.root else Path(__file__).resolve().parents[4]
    if not (root / "v2").is_dir():
        print(f"không có {root}/v2", file=sys.stderr)
        return 2

    parsed = [parse(p, root) for p in iter_files(root)]
    whole, inline, item_cfg = build_scope(parsed)
    g_teeth = build_global(parsed, whole, inline)
    aliases = use_aliases(parsed, whole | set(inline))

    rows: List[dict] = []
    n_tests = 0
    n_own = 0
    n_local = 0
    n_local_test_scope = 0
    n_local_product = 0
    n_global = 0
    n_none = 0
    n_call_product = 0
    n_call_product_no_own = 0
    n_newly_red = 0
    product_only: List[dict] = []
    newly_red: List[dict] = []

    for pf in parsed:
        is_whole = pf.path in whole
        ranges = inline.get(pf.path, [])
        if not is_whole and not ranges:
            continue
        fns = pf.fns
        top = top_level(fns)
        helper_old, calls_old = build_local(pf, is_whole, ranges, scoped=False)
        teeth_old = teeth_fn(helper_old, calls_old)
        helper_new, calls_new = build_local(pf, is_whole, ranges, scoped=True)
        teeth_new = teeth_fn(helper_new, calls_new)

        def in_scope(f: Fn) -> bool:
            return is_whole or in_ranges(f.start, ranges)

        prod_names = {f.name for f in fns if not f.is_test and not in_scope(f)}
        test_scope_names = {f.name for f in fns if not f.is_test and in_scope(f)}

        for f in top:
            if not f.is_test or not in_scope(f):
                continue
            n_tests += 1
            own_s = own_body(pf, f, fns)
            own_token = direct_teeth(own_s) or f.is_should_panic
            called = set(CALL_RE.findall(own_s))
            fa = aliases.get(pf.path, {})
            called |= {fa[c] for c in called if c in fa}
            hit_prod = called & prod_names
            if hit_prod:
                n_call_product += 1
                if not own_token:
                    n_call_product_no_own += 1

            local_old = sorted(c for c in called if c in helper_old and teeth_old(c))
            local_new = sorted(c for c in called if c in helper_new and teeth_new(c))
            global_hit = sorted(c for c in called if c in g_teeth)

            if own_token:
                path = "own"
                n_own += 1
            elif local_old:
                path = "local"
                n_local += 1
                if any(c in helper_new for c in local_old):
                    n_local_test_scope += 1
                if all(c not in helper_new for c in local_old):
                    n_local_product += 1
            elif global_hit:
                path = "global"
                n_global += 1
            else:
                path = "none"
                n_none += 1

            ok_new = own_token or bool(local_new) or bool(global_hit)
            if not ok_new:
                n_newly_red += 1
                newly_red.append(
                    {
                        "file": pf.rel,
                        "line": pf.line_of(f.start),
                        "test": f.name,
                        "own_token": own_token,
                        "local_old": local_old,
                        "local_new": local_new,
                        "global_hit": global_hit,
                        "hit_prod": sorted(hit_prod),
                    }
                )
            if path == "local" and all(c not in helper_new for c in local_old):
                product_only.append(
                    {
                        "file": pf.rel,
                        "line": pf.line_of(f.start),
                        "test": f.name,
                        "helpers": local_old,
                        "hit_prod": sorted(hit_prod),
                    }
                )
            rows.append(
                {
                    "file": pf.rel,
                    "line": pf.line_of(f.start),
                    "test": f.name,
                    "path": path,
                    "own_token": own_token,
                    "local_old": local_old,
                    "local_new": local_new,
                    "global_hit": global_hit,
                    "hit_prod": sorted(hit_prod),
                }
            )

    summary = {
        "root": str(root),
        "n_files": len(parsed),
        "n_whole": len(whole),
        "n_inline_files": len(inline),
        "n_tests": n_tests,
        "n_own": n_own,
        "n_local": n_local,
        "n_local_test_scope": n_local_test_scope,
        "n_local_product": n_local_product,
        "n_global": n_global,
        "n_none": n_none,
        "n_call_product": n_call_product,
        "n_call_product_no_own": n_call_product_no_own,
        "n_newly_red": n_newly_red,
        "n_product_only": len(product_only),
        "g_teeth_size": len(g_teeth),
        "n_item_cfg_test": len(item_cfg),
    }
    print("== KT-203: phép đo ĐỘC LẬP (không dùng checker/script tác giả) ==")
    for k, v in summary.items():
        print(f"{k}: {v}")
    print()
    print("== Q1: test CHỈ có răng nhờ hàm SẢN PHẨM cùng tệp ==")
    for r in sorted(product_only, key=lambda x: (x["file"], x["line"])):
        print(f"  {r['file']}:{r['line']} {r['test']} — {r['helpers']} (prod: {r['hit_prod']})")
    print()
    print("== Q3: test đổi XANH → ĐỎ sau khi siết ==")
    for r in sorted(newly_red, key=lambda x: (x["file"], x["line"])):
        print(
            f"  {r['file']}:{r['line']} {r['test']} — own={r['own_token']} "
            f"local_old={r['local_old']} local_new={r['local_new']} global={r['global_hit']}"
        )
    print()
    print("== Q4: item `#[cfg(test)]` (không phải `mod`) — checker bỏ sót khỏi phạm vi ==")
    for it in item_cfg:
        print(f"  {it['file']}: {it['item']}")

    if args.json:
        Path(args.json).write_text(
            json.dumps(
                {
                    "summary": summary,
                    "product_only": product_only,
                    "newly_red": newly_red,
                    "item_cfg_test": item_cfg,
                    "rows": rows,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"\n# JSON: {args.json}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
