#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""KT-348b — gửi `sendkey a` qua HMP monitor **trước** khi khách hết hạn chờ phím.

# Vì sao có tệp này (đo được ở KT-348)

`soak-aarch64.sh` cũ chờ dòng `# input: ready` bằng `grep` trên tệp log rồi **mới** khởi động
`python3` để nối monitor và gửi `sendkey a`. Đường đó nằm **trong** cửa sổ 5 s của ảnh
(`input::INPUT_WAIT_SECS`): dưới tải của máy đo, đo được các boot có `elapsed_ms` 1.578–3.827,
và boot 861 vượt hẳn 5.076 s ⇒ ảnh hết hạn, QEMU thoát, lần `connect` sau đó nhận
`[Errno 2] No such file or directory` (monitor socket đã bị QEMU xoá khi thoát). Harness cũ
`send_key ... || true` **nuốt** lỗi đó ⇒ boot đỏ trông như `TAP 32 input: key` của nhân.

# Hợp đồng

    monitor-sendkey.py <sock> <log> <ready_timeout_ms> <kbd_log> <boot_index>

1. Nối monitor **ngay** (trước `# input: ready`), chờ banner `(qemu)` — Python khởi động chạy
   **song song** với lúc khách boot, không nằm trong cửa sổ 5 s nữa.
2. Poll tệp log tới khi thấy dòng `# input: ready` (ảnh in **sau** khi đã arm 8 đệm ⇒ gửi an toàn).
3. Gửi `sendkey a` trên **kết nối đã mở sẵn**, đọc prompt, kiểm `Error`.
4. In + ghi một dòng `i=<i> ok ready_ms=<n> send_ms=<n>` hoặc `i=<i> fail reason=<slug>` vào
   `<kbd_log>`; exit 0/1. Mọi lỗi đều **có tên** — không im lặng.

Không nới ngưỡng nào: `ready_timeout_ms` do runner truyền (5 s, bằng `READY_DS` cũ) và cửa sổ 5 s
của ảnh giữ nguyên; tệp này chỉ **rút ngắn đường gửi** và **đặt tên** cho lỗi hạ tầng.
"""

import os
import select
import socket
import sys
import time


def connect_monitor(path, deadline_s=2.0):
    """Nối tới monitor socket; trả (socket, None) hoặc (None, lý_do)."""
    end = time.monotonic() + deadline_s
    last = "unknown"
    while True:
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.settimeout(5.0)
            s.connect(path)
            return s, None
        except OSError as exc:
            last = "connect-%s" % (exc.errno if exc.errno is not None else "?")
        if time.monotonic() >= end:
            return None, last
        time.sleep(0.005)


def read_prompt(sock, timeout_s):
    """Đọc tới `(qemu)`; trả (buffer, None) hoặc (None, 'banner-eof'|'banner-timeout')."""
    sock.settimeout(timeout_s)
    buf = b""
    try:
        while b"(qemu)" not in buf:
            chunk = sock.recv(4096)
            if not chunk:
                return None, "banner-eof"
            buf += chunk
    except socket.timeout:
        return None, "banner-timeout"
    except OSError:
        return None, "banner-error"
    return buf, None


def log_has_ready_line(path):
    """`True` ⇔ tệp log có **đúng** một dòng `# input: ready` (dòng cuối có thể chưa có \\n)."""
    try:
        with open(path, "rb") as fh:
            data = fh.read()
    except OSError:
        return False
    return b"# input: ready\n" in data


def monitor_alive(sock):
    """`False` ⇔ QEMU đã đóng đầu kia (EOF) — phân biệt 'chưa tới ready' với 'QEMU đã thoát'."""
    try:
        ready, _, _ = select.select([sock], [], [], 0)
        if not ready:
            return True
        return sock.recv(1, socket.MSG_PEEK) != b""
    except OSError:
        return False


def finish(code, line, kbd_log):
    try:
        fd = os.open(kbd_log, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
        try:
            os.write(fd, line.encode())
        finally:
            os.close(fd)
    except OSError:
        pass
    sys.stdout.write(line)
    sys.stdout.flush()
    return code


def main():
    if len(sys.argv) != 6:
        sys.stderr.write(__doc__)
        return 2
    sock_path, log_path, timeout_ms, kbd_log, boot_index = sys.argv[1:6]
    prefix = "i=%s " % boot_index
    try:
        ready_timeout = max(0.001, float(timeout_ms) / 1000.0)
    except ValueError:
        return finish(1, prefix + "fail reason=bad-timeout\n", kbd_log)

    sock, why = connect_monitor(sock_path)
    if sock is None:
        return finish(1, prefix + "fail reason=%s\n" % why, kbd_log)
    t_start = time.monotonic()
    banner, why = read_prompt(sock, 5.0)
    if banner is None:
        return finish(1, prefix + "fail reason=%s\n" % why, kbd_log)

    # Chờ dòng `# input: ready` do **khách** in (đã arm 8 đệm). Poll 5 ms, không spawn tiến trình.
    while True:
        if log_has_ready_line(log_path):
            break
        if time.monotonic() - t_start > ready_timeout:
            return finish(1, prefix + "fail reason=ready-timeout\n", kbd_log)
        if not monitor_alive(sock):
            return finish(1, prefix + "fail reason=qemu-exit-before-ready\n", kbd_log)
        time.sleep(0.005)
    ready_ms = int(round((time.monotonic() - t_start) * 1000))

    # Gửi trên kết nối **đã mở sẵn** — cửa sổ còn lại chỉ là một `sendall`.
    t_send = time.monotonic()
    try:
        sock.sendall(b"sendkey a\n")
        resp, why = read_prompt(sock, 2.0)
    except OSError:
        return finish(1, prefix + "fail reason=send-eof\n", kbd_log)
    if resp is None:
        return finish(1, prefix + "fail reason=%s\n" % why, kbd_log)
    if b"Error" in resp or b"error" in resp:
        return finish(1, prefix + "fail reason=monitor-error\n", kbd_log)
    send_ms = int(round((time.monotonic() - t_send) * 1000))
    try:
        sock.close()
    except OSError:
        pass
    return finish(0, prefix + "ok ready_ms=%d send_ms=%d\n" % (ready_ms, send_ms), kbd_log)


if __name__ == "__main__":
    sys.exit(main())
