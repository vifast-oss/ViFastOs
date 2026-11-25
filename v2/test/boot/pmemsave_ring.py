#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""pmemsave_ring.py — chụp kho ring trace `.corespace` **trong lúc QEMU đang chạy** (KT-408b).

Vì sao có tệp này
-----------------
`KT-408` đặt ring trace 32 B vào section **NOLOAD `.corespace`** (`D2-09`/`D2-252`) nhưng bằng
chứng "ring có thật trong RAM ảnh" mới chỉ ở **test host**; `KT-408b` phải chứng minh **đầu-cuối**:
QEMU ghi RAM ra tệp (`pmemsave`), rồi kiểm `RING`/`HEADS`/`COUNTS` trong chính tệp đó.

Tệp này **không** tự mở QEMU: runner `pmemsave-ring.sh` mở QEMU qua `qemu_guard.sh` (`D2-116`,
process group riêng + timeout), còn tệp này chạy **song song** để:

1. chờ một **mốc serial** (regex) do khách in ra ⇒ khách đã chạy được tới đó;
2. nối **monitor HMP** trên unix socket, `stop` VM (ảnh chụp **nguyên tử**: nếu để VM chạy giữa
   ba lần đọc thì `COUNTS`/`HEADS`/`RING` lệch nhau — đo được `heads=9, counts=14`), gửi **một**
   `pmemsave <pa> <size> <tệp>` cho vùng **liền nhau** `RING|HEADS|COUNTS` (8.320 B), rồi `cont`;
   QEMU tự ghi RAM vật lý ra tệp (kernel riscv64 identity-map `0x8020_0000` ⇒ `PA == VA`);
3. tách blob thành `ring-<tag>.bin`/`heads-<tag>.bin`/`counts-<tag>.bin` và ghi `meta-<tag>.json`.

Hai ảnh chụp (`--marker` hai lần) để chứng minh ring **sống**: `head` (tổng số bản ghi đã ghi)
tăng giữa hai mốc, còn `count = min(head, PER_HART)`.

Dùng (chỉ runner gọi):

    python3 v2/test/boot/pmemsave_ring.py --sock S --serial F --out DIR \
        --blob PA:SIZE --ring-size 8192 --heads-size 64 --counts-size 64 \
        --marker '^ok 42 ' --tag a --marker '^ok 59 ' --tag b --timeout 20
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import socket
import struct
import sys
import time

PROMPT = b"(qemu)"


def die(msg: str) -> None:
    print("# pmemsave_ring.py: %s" % msg, file=sys.stderr)
    sys.exit(1)


def wait_marker(serial: str, pattern: str, timeout: float) -> bool:
    """Chờ `pattern` xuất hiện trong tệp serial; `True` nếu thấy, `False` nếu hết hạn."""
    rx = re.compile(pattern, re.MULTILINE)
    t0 = time.time()
    while time.time() - t0 < timeout:
        try:
            with open(serial, "r", encoding="utf-8", errors="replace") as fh:
                if rx.search(fh.read()):
                    return True
        except FileNotFoundError:
            pass
        time.sleep(0.002)
    return False


class Monitor:
    """Một kết nối HMP ngắn: gửi lệnh, đọc tới prompt."""

    def __init__(self, path: str, timeout: float = 5.0):
        self.path = path
        self.timeout = timeout
        self.sock: socket.socket | None = None

    def __enter__(self) -> "Monitor":
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(self.timeout)
        try:
            s.connect(self.path)
        except OSError as exc:
            die("không nối được monitor %s: %s" % (self.path, exc))
        self.sock = s
        self._read_prompt()
        return self

    def __exit__(self, *_exc) -> None:
        if self.sock is not None:
            try:
                self.sock.close()
            except OSError:
                pass

    def _read_prompt(self) -> bytes:
        assert self.sock is not None
        buf = b""
        try:
            while PROMPT not in buf:
                chunk = self.sock.recv(4096)
                if not chunk:
                    break
                buf += chunk
        except socket.timeout:
            pass
        return buf

    def cmd(self, line: str) -> bytes:
        assert self.sock is not None
        self.sock.sendall((line + "\n").encode())
        return self._read_prompt()


def dump(m: Monitor, pa: int, size: int, path: str) -> None:
    """Một `pmemsave` **nguyên tử**: `stop` → `pmemsave` → `cont` (xem `dump_atomic`)."""
    m.cmd("stop")
    out = m.cmd('pmemsave 0x%x %d "%s"' % (pa, size, path))
    if b"Error" in out or b"error" in out or b"Could not" in out:
        die("monitor báo lỗi khi pmemsave 0x%x: %r" % (pa, out))
    if not os.path.isfile(path):
        die("pmemsave không tạo tệp %s" % path)
    got = os.path.getsize(path)
    if got != size:
        die("pmemsave ghi %d B, cần %d B (%s)" % (got, size, path))
    m.cmd("cont")


def read_u64s(path: str) -> list[int]:
    with open(path, "rb") as fh:
        raw = fh.read()
    if len(raw) % 8:
        die("%s: %d B không chia hết 8" % (path, len(raw)))
    return list(struct.unpack("<%dQ" % (len(raw) // 8), raw))


def sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sock", required=True)
    ap.add_argument("--serial", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--blob", required=True, help="PA:SIZE của RING|HEADS|COUNTS liền nhau")
    ap.add_argument("--ring-size", type=int, default=8192)
    ap.add_argument("--heads-size", type=int, default=64)
    ap.add_argument("--counts-size", type=int, default=64)
    ap.add_argument("--marker", action="append", required=True)
    ap.add_argument("--tag", action="append", required=True)
    ap.add_argument("--timeout", type=float, default=20.0)
    args = ap.parse_args()

    blob_pa, _, blob_size_s = args.blob.partition(":")
    blob_pa = int(blob_pa, 0)
    blob_size = int(blob_size_s, 0)
    ring_pa = blob_pa
    ring_size = args.ring_size
    heads_pa = blob_pa + args.ring_size
    counts_pa = heads_pa + args.heads_size
    if args.ring_size + args.heads_size + args.counts_size != blob_size:
        die("blob %d B != ring %d + heads %d + counts %d"
            % (blob_size, args.ring_size, args.heads_size, args.counts_size))
    if len(args.marker) != len(args.tag):
        die("--marker/--tag phải cùng số lượng")
    os.makedirs(args.out, exist_ok=True)

    for marker, tag in zip(args.marker, args.tag):
        if not wait_marker(args.serial, marker, args.timeout):
            die("hết %.1f s mà chưa thấy mốc serial %r" % (args.timeout, marker))
        t0 = time.time()
        blob_bin = os.path.join(args.out, "blob-%s.bin" % tag)
        ring_bin = os.path.join(args.out, "ring-%s.bin" % tag)
        heads_bin = os.path.join(args.out, "heads-%s.bin" % tag)
        counts_bin = os.path.join(args.out, "counts-%s.bin" % tag)
        with Monitor(args.sock) as m:
            dump(m, blob_pa, blob_size, blob_bin)
        dt_ms = (time.time() - t0) * 1000.0
        with open(blob_bin, "rb") as fh:
            blob = fh.read()
        ring_raw = blob[: args.ring_size]
        heads_raw = blob[args.ring_size : args.ring_size + args.heads_size]
        counts_raw = blob[args.ring_size + args.heads_size :]
        for path, raw in ((ring_bin, ring_raw), (heads_bin, heads_raw), (counts_bin, counts_raw)):
            with open(path, "wb") as fh:
                fh.write(raw)
        heads = read_u64s(heads_bin)
        counts = read_u64s(counts_bin)
        per_hart = ring_size // 32 // max(1, len(heads))
        meta = {
            "tag": tag,
            "marker": marker,
            "dump_ms": round(dt_ms, 1),
            "blob": {"pa": "0x%x" % blob_pa, "size": blob_size, "sha256": sha256(blob_bin)},
            "ring": {"pa": "0x%x" % ring_pa, "size": ring_size, "sha256": sha256(ring_bin)},
            "heads": {"pa": "0x%x" % heads_pa, "size": args.heads_size, "values": heads},
            "counts": {"pa": "0x%x" % counts_pa, "size": args.counts_size, "values": counts},
            "per_hart": per_hart,
        }
        meta_path = os.path.join(args.out, "meta-%s.json" % tag)
        with open(meta_path, "w", encoding="utf-8") as fh:
            json.dump(meta, fh, indent=2, sort_keys=True)
            fh.write("\n")
        print(
            '# pmemsave: tag=%s marker=%s dump_ms=%.1f blob_pa=0x%x blob_bytes=%d sha256=%s'
            % (tag, marker.strip(), dt_ms, blob_pa, blob_size, meta["blob"]["sha256"])
        )
        print(
            "# pmemsave: tag=%s vùng liền nhau RING[0x%x+%d] HEADS[0x%x+%d] COUNTS[0x%x+%d]"
            " · VM stop/cont ⇒ nguyên tử"
            % (tag, ring_pa, ring_size, heads_pa, args.heads_size, counts_pa, args.counts_size)
        )
        print(
            "# pmemsave: tag=%s heads=[%s] counts=[%s]"
            % (tag, " ".join(str(v) for v in heads), " ".join(str(v) for v in counts))
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
