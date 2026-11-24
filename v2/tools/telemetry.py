#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""telemetry.py — **quy chuẩn telemetry `key=value` không nhạy thứ tự trường** (KT-403, D2-259).

Vì sao có tệp này
-----------------
Log evidence của kernel/harness là các dòng dạng:

    # timer: lapic-window lvt_masked=1 masked_before_unmask=1 dest_bsp=1 order=dest_first
    # product exec: path=/u1a.elf src=fs bytes=16736 check=1

Bộ kiểm đang so chúng bằng **regex dán liền** (vd `attempts=… const:timeout_ms=3000`). Cách đó
phụ thuộc **thứ tự** trường: KT-347b đã phải sửa một regex như vậy khi D2-246 thêm `bad=<n>` vào
giữa. Thiết kế §9.11/D2-259 đòi chuẩn hoá: **đọc trường thành bản đồ**, so **theo tên**, không theo
vị trí. Tệp này là bộ phân tích **duy nhất** cho quy chuẩn đó.

Quy chuẩn
---------
1. Dòng telemetry: `[# ]<tiền tố>: <token>…` — tiền tố = phần trước token đầu tiên có `=`.
2. Mỗi token: `khoá=giá trị`. Khoá `[a-z][a-z0-9_]*` hoặc dạng hằng thiết kế
   `const:<tên>` (giữ quy ước đang dùng: `const:version1=1`, `const:timeout_ms=3000`).
   Giá trị **không rỗng, không khoảng trắng**.
3. **Thứ tự trường không mang nghĩa**: hai dòng cùng tiền tố + cùng bản đồ trường (bất kể thứ tự)
   là **tương đương**. Khoá trùng ⇒ `E_DUP_KEY` (không đoán cái nào thắng).
4. Token không có `=` ⇒ `E_TOKEN`; không có tiền tố ⇒ `E_NO_PREFIX`.

Dùng
----
    python3 v2/tools/telemetry.py --selftest
    python3 v2/tools/telemetry.py --file <log> [--prefix <t>] [--require <k>…]

Exit: 0 xanh; 1 có dòng sai quy chuẩn (khi `--file`/`--selftest`); 2 dùng sai.
"""

from __future__ import annotations

import argparse
import json
import re
import sys

KEY_RE = re.compile(r"^(?:const:)?[a-z][a-z0-9_]*$")
TOKEN_RE = re.compile(r"^([^=\s]+)=(\S+)$")


class TelemetryError(ValueError):
    """Dòng telemetry sai quy chuẩn; `code` là mã lỗi có tên (E_*)."""

    def __init__(self, code, detail=""):
        super().__init__("%s %s" % (code, detail))
        self.code = code
        self.detail = detail


def parse_line(line):
    """Trả `(prefix, fields)`; `fields` là dict (đọc theo tên, thứ tự vô nghĩa)."""
    text = line.rstrip("\n")
    if text.startswith("# "):
        text = text[2:]
    elif text.startswith("#"):
        text = text[1:]
    text = text.strip()
    if not text:
        raise TelemetryError("E_NO_PREFIX", "(dòng rỗng)")
    fields = {}
    prefix_parts = []
    tokens = text.split()
    seen_field = False
    for tok in tokens:
        m = TOKEN_RE.match(tok)
        if not m:
            # `k=` (giá trị rỗng) hoặc dạng lạ đều là token hỏng — **không** nuốt vào tiền tố.
            if "=" in tok or seen_field:
                raise TelemetryError("E_TOKEN", tok)
            prefix_parts.append(tok)
            continue
        seen_field = True
        key, value = m.group(1), m.group(2)
        if not KEY_RE.match(key):
            raise TelemetryError("E_BAD_KEY", key)
        if key in fields:
            raise TelemetryError("E_DUP_KEY", key)
        fields[key] = value
    if not seen_field:
        raise TelemetryError("E_NO_FIELDS", text)
    prefix = " ".join(prefix_parts).rstrip(":")
    if not prefix:
        raise TelemetryError("E_NO_PREFIX", text)
    return prefix, fields


def canonical(fields):
    """Dạng chuẩn hoá (sắp theo khoá) — dùng để so/ghi log tất định."""
    return {k: fields[k] for k in sorted(fields)}


def fields_equal(a, b):
    """So **theo tên trường**, bỏ qua thứ tự; trả False nếu khác tập khoá/giá trị."""
    return canonical(a) == canonical(b)


def has_fields(fields, expected):
    """`expected` ⊆ `fields` (so theo tên; giá trị so nguyên văn)."""
    return all(fields.get(k) == v for k, v in expected.items())


def format_line(prefix, fields):
    """Sinh dòng chuẩn hoá tất định từ bản đồ trường (tất cả trường, sắp theo khoá)."""
    body = " ".join("%s=%s" % (k, v) for k, v in canonical(fields).items())
    return "# %s: %s" % (prefix, body)


# --------------------------------------------------------------------------- selftest

# Bộ ca **inline** (một nguồn, không phụ thuộc tệp ngoài): mỗi ca là
# (dòng, kỳ vọng | None, mã lỗi | None).
CASES = [
    # (a) KHÔNG NHẠY THỨ TỰ: cùng trường, khác thứ tự ⇒ cùng bản đồ.
    ("# timer: a=1 b=2 c=3", {"a": "1", "b": "2", "c": "3"}, None),
    ("# timer: c=3 a=1 b=2", {"a": "1", "b": "2", "c": "3"}, None),
    ("# timer: b=2 c=3 a=1", {"a": "1", "b": "2", "c": "3"}, None),
    # (b) tiền tố nhiều từ + hằng thiết kế `const:`.
    ("# product exec: path=/u1a.elf const:timeout_ms=3000 ok=1",
     {"path": "/u1a.elf", "const:timeout_ms": "3000", "ok": "1"}, None),
    # (c) dòng không có `# ` (harness tự in) vẫn đọc được.
    ("timer: x=1", {"x": "1"}, None),
    # (d) LỖI — trùng khoá (không đoán cái nào thắng).
    ("# timer: a=1 a=2", None, "E_DUP_KEY"),
    # (e) LỖI — khoá hoa (quy chuẩn chữ thường).
    ("# timer: A=1", None, "E_BAD_KEY"),
    # (f) LỖI — giá trị rỗng.
    ("# timer: a=", None, "E_TOKEN"),
    # (g) LỖI — token không `=` sau trường đầu.
    ("# timer: a=1 rác", None, "E_TOKEN"),
    # (h) LỖI — không có tiền tố (dòng chỉ có trường).
    ("a=1 b=2", None, "E_NO_PREFIX"),
    # (i) LỖI — không có trường nào.
    ("# timer:", None, "E_NO_FIELDS"),
]

# Đối chứng hoán vị: mọi hoán vị của 4 trường phải cho cùng bản đồ.
PERMUTE_LINE = "# smp: boot=1 harts=4 timeout=0 dtb_magic=1"


def run_selftest():
    fail = 0
    total = 0

    def say(ok, label, extra=""):
        nonlocal fail, total
        total += 1
        print("%s %d - %s%s" % ("ok" if ok else "not ok", total, label, extra))
        if not ok:
            fail += 1

    for i, (line, want, code) in enumerate(CASES, 1):
        if code is None:
            try:
                prefix, fields = parse_line(line)
                ok = fields_equal(fields, want) and prefix
            except TelemetryError as exc:
                ok = False
                prefix = "lỗi %s" % exc.code
            say(ok, "ca %d: %r" % (i, line))
        else:
            try:
                parse_line(line)
                got = "-"
            except TelemetryError as exc:
                got = exc.code
            say(got == code, "ca %d: %r ⇒ %s" % (i, line, code), " (nhận %s)" % got)

    # (j) hoán vị 4! = 24 thứ tự ⇒ **cùng** bản đồ (định nghĩa "không nhạy thứ tự trường").
    import itertools

    prefix0, fields0 = parse_line(PERMUTE_LINE)
    ok = True
    for perm in itertools.permutations(fields0):
        line = "# %s: %s" % (prefix0, " ".join("%s=%s" % (k, fields0[k]) for k in perm))
        p, f = parse_line(line)
        ok = ok and p == prefix0 and fields_equal(f, fields0)
    say(ok, "24/24 hoán vị trường ⇒ cùng bản đồ (thứ tự vô nghĩa)")

    # (k) sinh dòng chuẩn hoá rồi đọc lại ⇒ trùng khít (round-trip tất định).
    line = format_line("timer", {"b": "2", "a": "1"})
    p, f = parse_line(line)
    say(p == "timer" and f == {"a": "1", "b": "2"}, "round-trip format_line/parse_line")

    # (l) `has_fields` so theo tên, không theo vị trí.
    _, f = parse_line("# timer: order=dest_first dest_bsp=1 lvt_masked=1")
    say(
        has_fields(f, {"lvt_masked": "1", "dest_bsp": "1"}) and not has_fields(f, {"lvt_masked": "0"}),
        "has_fields: so theo tên, không theo vị trí",
    )

    if fail:
        print("== SELFTEST telemetry: SAI — %d/%d lỗi ==" % (fail, total))
        return 1
    print("== SELFTEST telemetry: OK — %d/%d (quy chuẩn key=value không nhạy thứ tự) ==" % (total, total))
    return 0


def main(argv):
    ap = argparse.ArgumentParser(description="quy chuẩn telemetry key=value (KT-403)")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--file", default=None, help="log cần phân tích (mỗi dòng một bản ghi)")
    ap.add_argument("--prefix", default=None, help="chỉ nhận tiền tố này")
    ap.add_argument("--require", action="append", default=[], help="trường bắt buộc (lặp lại được)")
    args = ap.parse_args(argv)
    if args.selftest:
        return run_selftest()
    if not args.file:
        ap.print_help()
        return 2
    bad = 0
    n = 0
    with open(args.file, "r", encoding="utf-8", errors="replace") as fh:
        for lineno, line in enumerate(fh, 1):
            if "=" not in line:
                continue
            # **Lọc tiền tố TRƯỚC khi parse**: log kernel có dòng văn xuôi chứa `=` (không phải
            # bản ghi telemetry). Khi người gọi đã chỉ định tiền tố, chỉ những dòng đó mới phải
            # đúng quy chuẩn — còn lại là log thường, bỏ qua.
            if args.prefix:
                probe = line[2:] if line.startswith("# ") else line
                head = probe.split(":", 1)[0]
                if not head.strip().startswith(args.prefix):
                    continue
            try:
                prefix, fields = parse_line(line)
            except TelemetryError as exc:
                print("E%d %s %s" % (lineno, exc.code, exc.detail), file=sys.stderr)
                bad += 1
                continue
            if args.require and not has_fields(fields, {k: fields.get(k) for k in args.require}):
                print("E%d E_REQUIRED %s" % (lineno, args.require), file=sys.stderr)
                bad += 1
                continue
            n += 1
            print(json.dumps({"prefix": prefix, "fields": canonical(fields)}, ensure_ascii=False))
    print("# telemetry: dòng=%d sai=%d" % (n, bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
