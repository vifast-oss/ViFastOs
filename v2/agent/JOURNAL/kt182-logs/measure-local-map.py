#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-182 — đo tác động: bản đồ trợ giúp **cùng tệp** (KT-163) nhận hàm **sản phẩm** cùng tệp.

Bối cảnh
--------
`v2/tools/check-test-teeth.py` lớp (c) chứng nhận một `#[test]` "có răng" theo ba đường:
token trong thân, bản đồ trợ giúp **cùng tệp** (`helper_teeth` + `teeth_of`), bản đồ **toàn
cây** (`build_global_helpers`, KT-170 — chỉ hàm trong phạm vi test), và `HELPER_ALLOW`.

Bản đồ **toàn cây** đã siết về phạm vi test (KT-170). Bản đồ **cùng tệp** thì **chưa**: nó
dựng từ **mọi** `fn` không-phải-`#[test]` trong tệp, kể cả hàm **sản phẩm** nằm ngoài khối
`#[cfg(test)]`. Đó là lỗ hổng KT-182: một test trong `#[cfg(test)] mod tests` gọi hàm sản
phẩm cùng tệp (vd `super::fsck()`) được xanh nhờ `debug_assert!`/`assert!` **của sản phẩm**.

Câu hỏi có số
-------------
Q1. Bao nhiêu `#[test]` hiện **chỉ** có răng nhờ hàm **sản phẩm cùng tệp** (siết ⇒ ĐỎ)?
Q2. Danh sách đầy đủ: `tệp:dòng`, tên test, tên hàm sản phẩm, token khẳng định của hàm đó.
Q3. Trong số đó: **răng thật** (`assert!`/`assert_eq!`/… hoặc `panic!` có điều kiện) vs
    **răng giả** (chỉ `debug_assert*!` hoặc không có token nào — chỉ bắc cầu).
Q4. Nếu siết (chỉ nhận hàm trong phạm vi test): tổng ĐỎ, **hồi quy giả** (test thật sự có
    răng nhưng mất chứng nhận) vs **lỗ hổng thật** (test vốn không có răng).

Cách làm
--------
Import **chính** checker làm module (không sửa nó), dựng lại đúng các bảng `scan()` dựng
(`parse_file`, `build_scope`, `build_global_helpers`, `build_use_aliases`), rồi chạy lại phép
chứng nhận lớp (c) với **hai** bản đồ cùng tệp:

  - `current`  — mọi `fn` không-phải-`#[test]` trong tệp (hành vi KT-163 hiện tại);
  - `scoped`   — chỉ `fn` nằm trong phạm vi test (tệp `tests/` toàn phần, hoặc trong khoảng
                 `#[cfg(test)]`) — đúng luật mà KT-170 đã áp cho bản đồ toàn cây.

Dùng
----
    python3 v2/agent/JOURNAL/kt182-logs/measure-local-map.py [--root R] [--json PATH]

Exit: 0 = đo xong; 2 = dùng sai.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set, Tuple

HERE = Path(__file__).resolve()
REPO = HERE.parents[4]
CHECKER = REPO / "v2" / "tools" / "check-test-teeth.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("check_test_teeth", CHECKER)
    if spec is None or spec.loader is None:  # pragma: no cover
        raise SystemExit(f"không nạp được {CHECKER}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["check_test_teeth"] = mod
    spec.loader.exec_module(mod)
    return mod


# Token **mạnh** (răng thật) vs token **yếu** (`debug_assert*!` — chỉ chạy ở debug, và bị
# chính checker coi là đủ; KT-182 đo riêng để biết "răng" là thật hay giả).
DEBUG_TOKENS: Tuple[str, ...] = ("debug_assert!", "debug_assert_eq!", "debug_assert_ne!")


def real_tokens(mod) -> Tuple[str, ...]:
    return tuple(t for t in mod.ASSERT_TOKENS if t not in DEBUG_TOKENS)


def own_body(mod, pf, f, fns) -> str:
    """Thân **riêng** của `f` (bỏ thân hàm lồng) — đúng như `scan()` tính."""
    m = pf.masked
    body = m[f.body_start : f.body_end]
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


def has_own_token(mod, own_s: str, f) -> bool:
    if any(t in own_s for t in mod.ASSERT_TOKENS):
        return True
    if "panic!" in own_s and mod.CONTROL_FLOW.search(own_s):
        return True
    if f.is_should_panic:
        return True
    return False


def build_local_map(mod, fns, m: str, is_whole: bool, ranges, scoped: bool):
    """Bản đồ cùng tệp: `helper_teeth` + `calls_of` (đúng khuôn `scan()`).

    `scoped=True` ⇒ chỉ hàm trong phạm vi test (đúng luật KT-170 áp cho bản đồ toàn cây).
    """
    helper_teeth: Dict[str, bool] = {}
    calls_of: Dict[str, Set[str]] = {}
    for f in fns:
        if f.is_test:
            continue
        if scoped and not (is_whole or mod.in_ranges(f.start, ranges)):
            continue
        body = m[f.body_start : f.body_end]
        helper_teeth[f.name] = any(t in body for t in mod.ASSERT_TOKENS) or (
            "panic!" in body and bool(mod.CONTROL_FLOW.search(body))
        )
    for f in fns:
        if scoped and not (is_whole or mod.in_ranges(f.start, ranges)):
            continue
        body = m[f.body_start : f.body_end]
        calls_of[f.name] = set(re.findall(mod.CALL_NAME_RE, body))
    return helper_teeth, calls_of


def make_teeth_of(helper_teeth: Dict[str, bool], calls_of: Dict[str, Set[str]]):
    def teeth_of(name: str, seen: Optional[Set[str]] = None) -> bool:
        seen = set() if seen is None else seen
        if name in seen:
            return False
        seen.add(name)
        if helper_teeth.get(name):
            return True
        for c in calls_of.get(name, ()):
            if c in helper_teeth and teeth_of(c, seen):
                return True
        return False

    return teeth_of


def fn_kinds(mod, fns, m: str):
    """`tên -> (real, debug)` **bắc cầu** trên mọi hàm không-phải-`#[test]` trong tệp.

    `real` = có `assert!`/`assert_eq!`/`assert_ne!`/`assert_matches!`/`expect_err`/
    `unwrap_err`, hoặc `panic!` **có điều kiện**; `debug` = có `debug_assert*!`.
    Bắc cầu: hàm gọi hàm khác có răng thì cũng có răng (đúng bao đóng `teeth_of`).
    """
    direct: Dict[str, Tuple[bool, bool]] = {}
    calls: Dict[str, Set[str]] = {}
    for f in fns:
        if f.is_test:
            continue
        body = m[f.body_start : f.body_end]
        real = any(t in body for t in real_tokens(mod)) or (
            "panic!" in body and bool(mod.CONTROL_FLOW.search(body))
        )
        dbg = any(t in body for t in DEBUG_TOKENS)
        direct[f.name] = (real, dbg)
        calls[f.name] = set(re.findall(mod.CALL_NAME_RE, body))
    kind = dict(direct)
    changed = True
    while changed:
        changed = False
        for n in list(kind):
            r, d = kind[n]
            for c in calls.get(n, ()):
                if c not in kind:
                    continue
                cr, cd = kind[c]
                if cr and not r:
                    r = True
                    changed = True
                if cd and not d:
                    d = True
                    changed = True
            kind[n] = (r, d)
    return kind, direct


def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=None)
    ap.add_argument("--json", default=None, help="ghi dữ liệu đầy đủ ra tệp JSON")
    args = ap.parse_args(argv)

    mod = load_checker()
    root = Path(args.root).resolve() if args.root else REPO
    if not (root / "v2").is_dir():
        print(f"không có {root}/v2", file=sys.stderr)
        return 2

    files = mod.iter_rs_files(root)
    parsed = [mod.parse_file(p, root) for p in files]
    parsed = [pf for pf in parsed if pf is not None]
    whole, inline = mod.build_scope(root, parsed)
    g_teeth = mod.build_global_helpers(parsed, whole, inline)
    use_aliases = mod.build_use_aliases(parsed, whole | set(inline))

    rows: List[dict] = []
    n_tests = 0
    n_own = 0
    n_should_panic = 0
    n_allow_no_assert = 0
    n_helper_allow = 0
    n_local = 0
    n_local_test_scope = 0
    n_local_product = 0
    n_global = 0
    n_none = 0
    n_red_current = 0
    n_red_scoped = 0
    n_tests_call_product = 0
    n_tests_call_product_no_own = 0
    n_multi_def_test_scope = 0
    n_collision_names = 0
    collision_list: List[dict] = []

    for pf in parsed:
        p = pf.path
        rel = pf.rel
        m = pf.masked
        is_whole = p in whole
        ranges = inline.get(p, [])
        if not is_whole and not ranges:
            continue
        fns = pf.fns
        top = mod.top_level_fns(fns)

        helper_teeth, calls_of = build_local_map(mod, fns, m, is_whole, ranges, scoped=False)
        teeth_of = make_teeth_of(helper_teeth, calls_of)
        helper_teeth_s, calls_of_s = build_local_map(mod, fns, m, is_whole, ranges, scoped=True)
        teeth_of_s = make_teeth_of(helper_teeth_s, calls_of_s)
        kinds, direct_kinds = fn_kinds(mod, fns, m)

        def in_test_scope(f) -> bool:
            return is_whole or mod.in_ranges(f.start, ranges)

        # Đối chứng cho Q1: test **có gọi** hàm sản phẩm cùng tệp (theo tên) — dù có token
        # riêng hay không. Con số này giải thích vì sao Q1 = 0 mà lỗ hổng vẫn "phủ" nhiều test.
        prod_names = {
            f.name for f in fns if not f.is_test and not in_test_scope(f)
        }
        test_names = {f.name for f in fns if not f.is_test and in_test_scope(f)}
        for name in sorted(prod_names & test_names):
            n_collision_names += 1
            collision_list.append({"file": rel, "name": name})
        defs_in_scope: Dict[str, int] = {}
        for f in fns:
            if f.is_test or not in_test_scope(f):
                continue
            defs_in_scope[f.name] = defs_in_scope.get(f.name, 0) + 1
        n_multi_def_test_scope += sum(1 for v in defs_in_scope.values() if v >= 2)

        for f in top:
            if not f.is_test:
                continue
            if not in_test_scope(f):
                continue
            n_tests += 1
            own_s = own_body(mod, pf, f, fns)
            own_token = has_own_token(mod, own_s, f)
            called = set(re.findall(mod.CALL_NAME_RE, own_s))
            file_aliases = use_aliases.get(p, {})
            called |= {file_aliases[c] for c in called if c in file_aliases}
            hit_prod = called & prod_names
            if hit_prod:
                n_tests_call_product += 1
                if not own_token:
                    n_tests_call_product_no_own += 1

            # --- đường chứng nhận hiện tại (đúng thứ tự `scan()`)
            local_names = sorted(c for c in called if c in helper_teeth and teeth_of(c))
            local_test_scope = [c for c in local_names if c in helper_teeth_s]
            local_product = [c for c in local_names if c not in helper_teeth_s]
            global_names = sorted(c for c in called if c in g_teeth)
            allow_na = (rel, f.name) in mod.ALLOW_NO_ASSERT
            allow_help = sorted(c for c in called if c in mod.HELPER_ALLOW)

            if own_token:
                path = "own"
                n_own += 1
            elif allow_help:
                path = "helper_allow"
                n_helper_allow += 1
            elif local_names:
                path = "local"
                n_local += 1
                if local_test_scope:
                    n_local_test_scope += 1
                if local_product and not local_test_scope:
                    n_local_product += 1
            elif global_names:
                path = "global"
                n_global += 1
            elif allow_na:
                path = "allow_no_assert"
                n_allow_no_assert += 1
            else:
                path = "none"
                n_none += 1
                n_red_current += 1

            # --- mô phỏng siết: bản đồ cùng tệp chỉ nhận hàm trong phạm vi test
            if own_token:
                ok_s = True
            elif allow_help:
                ok_s = True
            elif any(c in helper_teeth_s and teeth_of_s(c) for c in called):
                ok_s = True
            elif global_names:
                ok_s = True
            elif allow_na:
                ok_s = True
            else:
                ok_s = False
            if not ok_s:
                n_red_scoped += 1

            # --- phân loại "răng" của các hàm sản phẩm cùng tệp đã chứng nhận
            prod_detail = []
            for c in local_product:
                r, d = kinds.get(c, (False, False))
                dr, dd = direct_kinds.get(c, (False, False))
                prod_detail.append(
                    {
                        "fn": c,
                        "real_transitive": r,
                        "debug_transitive": d,
                        "real_direct": dr,
                        "debug_direct": dd,
                    }
                )
            if prod_detail:
                if any(x["real_transitive"] for x in prod_detail):
                    teeth_kind = "real"
                elif any(x["debug_transitive"] for x in prod_detail):
                    teeth_kind = "debug-only"
                else:
                    teeth_kind = "none"
            else:
                teeth_kind = ""

            rows.append(
                {
                    "file": rel,
                    "line": pf.line_of(f.start),
                    "test": f.name,
                    "path": path,
                    "own_token": own_token,
                    "should_panic": f.is_should_panic,
                    "local_names": local_names,
                    "local_test_scope": local_test_scope,
                    "local_product": local_product,
                    "global_names": global_names,
                    "allow_no_assert": allow_na,
                    "helper_allow": allow_help,
                    "product_detail": prod_detail,
                    "teeth_kind": teeth_kind,
                    "red_if_scoped": not ok_s,
                    "red_current": path == "none",
                }
            )

    product_only = [
        r
        for r in rows
        if r["path"] == "local" and r["local_product"] and not r["local_test_scope"]
    ]
    newly_red = [r for r in rows if r["red_if_scoped"] and not r["red_current"]]
    false_reg = [r for r in newly_red if r["teeth_kind"] == "real"]
    real_hole = [r for r in newly_red if r["teeth_kind"] in ("debug-only", "none")]

    summary = {
        "root": str(root),
        "n_files": len(parsed),
        "n_whole": len(whole),
        "n_inline_files": len(inline),
        "n_tests": n_tests,
        "n_own": n_own,
        "n_should_panic": sum(1 for r in rows if r["should_panic"]),
        "n_allow_no_assert": n_allow_no_assert,
        "n_helper_allow": n_helper_allow,
        "n_local": n_local,
        "n_local_test_scope": n_local_test_scope,
        "n_local_product": n_local_product,
        "n_global": n_global,
        "n_none": n_none,
        "n_red_current": n_red_current,
        "n_red_scoped": n_red_scoped,
        "n_newly_red": len(newly_red),
        "n_false_regression": len(false_reg),
        "n_real_hole": len(real_hole),
        "n_product_only": len(product_only),
        "n_product_only_real": sum(1 for r in product_only if r["teeth_kind"] == "real"),
        "n_product_only_debug": sum(1 for r in product_only if r["teeth_kind"] == "debug-only"),
        "n_product_only_none": sum(1 for r in product_only if r["teeth_kind"] == "none"),
        "n_tests_call_product": n_tests_call_product,
        "n_tests_call_product_no_own": n_tests_call_product_no_own,
        "n_multi_def_test_scope": n_multi_def_test_scope,
        "n_collision_names": n_collision_names,
        "g_teeth_size": len(g_teeth),
    }

    print("== KT-182: đo bản đồ trợ giúp CÙNG TỆP ==")
    for k, v in summary.items():
        print(f"{k}: {v}")
    print()
    print("== Q1/Q2/Q3: test CHỈ có răng nhờ hàm SẢN PHẨM cùng tệp ==")
    print(f"{'tệp:dòng':<62} {'test':<44} {'hàm sản phẩm':<28} {'răng':<10} token")
    for r in sorted(product_only, key=lambda x: (x["file"], x["line"])):
        fns_s = ", ".join(x["fn"] for x in r["product_detail"])
        toks = "; ".join(
            f"{x['fn']}: real={x['real_direct']} debug={x['debug_direct']}"
            for x in r["product_detail"]
        )
        print(
            f"{r['file'] + ':' + str(r['line']):<62} {r['test']:<44} {fns_s:<28} "
            f"{r['teeth_kind']:<10} {toks}"
        )
    print()
    print("== Tên vừa có định nghĩa SẢN PHẨM vừa có định nghĩa TEST trong cùng tệp ==")
    for c in collision_list:
        print(f"  {c['file']}: {c['name']}")
    print()
    print("== Q4: nếu siết (chỉ hàm trong phạm vi test) ==")
    print(f"tổng ĐỎ sau siết: {n_red_scoped} (hiện tại {n_red_current})")
    print(f"mới ĐỎ: {len(newly_red)} — hồi quy giả {len(false_reg)}, lỗ hổng thật {len(real_hole)}")
    for r in sorted(newly_red, key=lambda x: (x["file"], x["line"])):
        print(
            f"  {r['file']}:{r['line']} {r['test']} — {r['teeth_kind']} "
            f"({', '.join(x['fn'] for x in r['product_detail'])})"
        )

    if args.json:
        # Chỉ ghi phần **có số** cần dẫn chứng: tổng quan, danh sách "chỉ dựa hàm sản phẩm
        # cùng tệp" (Q1–Q3), và 23 test dùng bản đồ cùng tệp (đối chứng dương). Toàn bộ
        # 1019 dòng tái lập được bằng cách chạy lại script — không cần chép vào repo.
        keep = [
            r
            for r in rows
            if r["path"] == "local" or r["red_if_scoped"] or r["red_current"]
        ]
        Path(args.json).write_text(
            json.dumps(
                {
                    "summary": summary,
                    "product_only": product_only,
                    "local_rows": keep,
                    "collisions": collision_list,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"\n# JSON: {args.json} ({len(keep)} dòng)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
