import socket, struct, sys

port = int(sys.argv[1])
log = sys.argv[2]
mode = sys.argv[3] if len(sys.argv) > 3 else "ok"
answer_ip = sys.argv[4] if len(sys.argv) > 4 else "192.0.2.123"
ttl = int(sys.argv[5]) if len(sys.argv) > 5 else 3600


def note(line):
    with open(log, "a") as handle:
        handle.write(line + "\n")


def parse_qname(data, off):
    labels = []
    while True:
        if off >= len(data):
            return None, off
        ln = data[off]
        off += 1
        if ln == 0:
            break
        if ln & 0xC0:
            return None, off
        labels.append(data[off:off + ln].decode("latin-1"))
        off += ln
    return ".".join(labels), off


srv = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
try:
    srv.bind(("0.0.0.0", port))
except OSError as error:
    note("bind_err=%s" % error)
    sys.exit(2)
note("listen port=%d mode=%s answer=%s ttl=%d" % (port, mode, answer_ip, ttl))
srv.settimeout(1.0)
while True:
    try:
        data, addr = srv.recvfrom(2048)
    except socket.timeout:
        continue
    except OSError:
        break
    if len(data) < 12:
        continue
    tid, flags, qd, an, ns, ar = struct.unpack("!HHHHHH", data[:12])
    qname, off = parse_qname(data, 12)
    if qname is None or off + 4 > len(data):
        note("bad_query len=%d" % len(data))
        continue
    qtype, qclass = struct.unpack("!HH", data[off:off + 4])
    qend = off + 4
    note("query id=0x%04x name=%s type=%d class=%d from=%s:%d"
         % (tid, qname, qtype, qclass, addr[0], addr[1]))
    if qname != "example.com" or qtype != 1:
        rcode = 3
        answer = b""
        ancount = 0
    elif mode == "nxdomain":
        rcode = 3
        answer = b""
        ancount = 0
    elif mode == "wrongip":
        rcode = 0
        answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, ttl, 4) + socket.inet_aton("203.0.113.9")
        ancount = 1
    else:
        rcode = 0
        answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, ttl, 4) + socket.inet_aton(answer_ip)
        ancount = 1
    resp = struct.pack("!HHHHHH", tid, 0x8180 | rcode, 1, ancount, 0, 0) + data[12:qend] + answer
    srv.sendto(resp, addr)
    note("reply id=0x%04x rcode=%d an=%d len=%d" % (tid, rcode, ancount, len(resp)))
