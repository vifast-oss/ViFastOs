#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""vifast-trace.py — giải mã **dump trace nhị phân 32 B** (D2-09/D2-40, KT-403).

Định dạng (nguồn sự thật: `spec/abi.toml [trace]`, `D2-40`)
-----------------------------------------------------------
Bản ghi 32 B, little-endian, không padding:

    t_ns:u64  hart:u16  kind:u16  tcb:u32  a:u64  b:u64

kind: 1 syscall, 2 ipc, 3 sched, 4 fault, 5 store, 6 run, 7 idle.

Dump có header (khi ghi ra tệp / vùng `pmemsave` của QEMU):

    magic "VFT1" (4 B) | version:u32 | count:u32 | hash:u64 (FNV-1a 64 trên chuỗi bản ghi)

Tệp **không** header (golden thô `v2/test/trace/golden-sim-seed7.bin`) cũng đọc được: độ dài chia
hết 32 ⇒ giải mã trực tiếp. Header sai magic/version/count/hash ⇒ lỗi có tên (fail-closed).

Dùng
----
    python3 v2/tools/vifast-trace.py <tệp> [--json] [--limit N] [--kind ipc]
    python3 v2/tools/vifast-trace.py --selftest
"""

from __future__ import annotations

import argparse
import json
import struct
import sys

RECORD = struct.Struct("<QHHIQQ")
RECORD_BYTES = 32
HEADER = struct.Struct("<4sIIQ")
HEADER_BYTES = HEADER.size
MAGIC = b"VFT1"
VERSION = 1
KIND = {
    1: "syscall",
    2: "ipc",
    3: "sched",
    4: "fault",
    5: "store",
    6: "run",
    7: "idle",
}


class TraceError(Exception):
    def __init__(self, code, detail=""):
        super().__init__("%s %s" % (code, detail))
        self.code = code
        self.detail = detail


def fnv1a64(data):
    h = 0xCBF29CE484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def decode(payload):
    """Trả `(header | None, records)`. `header` = dict magic/version/count/hash."""
    header = None
    body = payload
    if payload[:4] == MAGIC:
        if len(payload) < HEADER_BYTES:
            raise TraceError("E_TRUNC", "header %d B" % len(payload))
        magic, version, count, digest = HEADER.unpack_from(payload, 0)
        if magic != MAGIC:
            raise TraceError("E_MAGIC", repr(magic))
        if version != VERSION:
            raise TraceError("E_VERSION", str(version))
        body = payload[HEADER_BYTES:]
        got = fnv1a64(body)
        if got != digest:
            raise TraceError("E_HASH", "0x%016x != 0x%016x" % (got, digest))
        if count != len(body) // RECORD_BYTES:
            raise TraceError("E_COUNT", "header=%d, thân=%d" % (count, len(body) // RECORD_BYTES))
        header = {"magic": "VFT1", "version": version, "count": count, "hash": digest}
    if len(body) % RECORD_BYTES:
        raise TraceError("E_TRUNC", "%d B lẻ %d" % (len(body), len(body) % RECORD_BYTES))
    records = []
    for off in range(0, len(body), RECORD_BYTES):
        t_ns, hart, kind, tcb, a, b = RECORD.unpack_from(body, off)
        if kind not in KIND:
            raise TraceError("E_KIND", "kind=%d @%d" % (kind, off))
        records.append(
            {
                "t_ns": t_ns,
                "hart": hart,
                "kind": kind,
                "kind_name": KIND[kind],
                "tcb": None if tcb == 0xFFFFFFFF else tcb,
                "a": a,
                "b": b,
            }
        )
    return header, records


def render(rec):
    tcb = "-" if rec["tcb"] is None else str(rec["tcb"])
    return "t=%d hart=%d kind=%s tcb=%s a=%d b=%d" % (
        rec["t_ns"],
        rec["hart"],
        rec["kind_name"],
        tcb,
        rec["a"],
        rec["b"],
    )


def run_selftest():
    import tempfile
    import os

    fail = 0
    total = 0

    def say(ok, label, extra=""):
        nonlocal fail, total
        total += 1
        print("%s %d - %s%s" % ("ok" if ok else "not ok", total, label, extra))
        if not ok:
            fail += 1

    body = b""
    for i in range(3):
        body += RECORD.pack(100 + i, 0, (6, 7, 2)[i], 1 if i else 0xFFFFFFFF, i, i * 2)
    dump = MAGIC + struct.pack("<IIQ", VERSION, 3, fnv1a64(body)) + body
    header, recs = decode(dump)
    say(header["count"] == 3 and len(recs) == 3, "dump có header ⇒ 3 bản ghi")
    say(recs[0]["kind_name"] == "run" and recs[2]["kind_name"] == "ipc", "ánh xạ kind 6/7/2")
    say(recs[0]["tcb"] is None and recs[1]["tcb"] == 1, "tcb=0xFFFFFFFF ⇔ None")
    say(decode(body)[1] == recs, "dump THÔ (không header) ⇒ cùng bản ghi")
    cases = [
        ("E_HASH", dump[:-1] + bytes([dump[-1] ^ 0xFF]), "hash sai ⇒ E_HASH"),
        ("E_TRUNC", body + b"\x00", "dump THÔ lẻ 1 B ⇒ E_TRUNC"),
        ("E_VERSION", MAGIC + struct.pack("<IIQ", 2, 0, fnv1a64(b"")), "version 2 ⇒ E_VERSION"),
        ("E_COUNT", MAGIC + struct.pack("<IIQ", 1, 9, fnv1a64(body)) + body, "count lệch ⇒ E_COUNT"),
    ]
    for want, data, label in cases:
        try:
            decode(data)
            got = "-"
        except TraceError as exc:
            got = exc.code
        say(got == want, "%s (nhận %s)" % (label, got))
    # Tệp tạm + CLI đường ống: golden thô 200 bản ghi đọc được end-to-end.
    path = os.path.join(tempfile.mkdtemp(prefix="kt403-trace-"), "g.vft")
    with open(path, "wb") as fh:
        fh.write(dump)
    with open(path, "rb") as fh:
        h2, r2 = decode(fh.read())
    say(h2 == header and len(r2) == 3, "đọc lại từ tệp (round-trip)")
    os.remove(path)
    os.rmdir(os.path.dirname(path))
    if fail:
        print("== SELFTEST vifast-trace: SAI — %d/%d ==" % (fail, total))
        return 1
    print("== SELFTEST vifast-trace: OK — %d/%d (bản ghi 32 B, header VFT1) ==" % (total, total))
    return 0


def main(argv):
    ap = argparse.ArgumentParser(description="giải mã dump trace nhị phân 32 B (D2-09/D2-40)")
    ap.add_argument("file", nargs="?", help="tệp dump (header VFT1 hoặc thô)")
    ap.add_argument("--json", action="store_true", help="in JSON từng bản ghi")
    ap.add_argument("--limit", type=int, default=0, help="chỉ in N bản ghi đầu")
    ap.add_argument("--kind", default=None, help="chỉ in kind này (syscall/ipc/…)")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args(argv)
    if args.selftest:
        return run_selftest()
    if not args.file:
        ap.print_help()
        return 2
    try:
        with open(args.file, "rb") as fh:
            payload = fh.read()
        header, records = decode(payload)
    except OSError as exc:
        print("vifast-trace: %s" % exc, file=sys.stderr)
        return 2
    except TraceError as exc:
        print("vifast-trace: ĐỎ %s %s" % (exc.code, exc.detail), file=sys.stderr)
        return 1
    shown = 0
    for rec in records:
        if args.kind and rec["kind_name"] != args.kind:
            continue
        if args.limit and shown >= args.limit:
            break
        print(json.dumps(rec, ensure_ascii=False) if args.json else render(rec))
        shown += 1
    head = "header=VFT1v%d count=%d" % (header["version"], header["count"]) if header else "header=thô"
    print("# vifast-trace: %s records=%d in=%d shown=%d" % (head, len(records), len(payload), shown))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
