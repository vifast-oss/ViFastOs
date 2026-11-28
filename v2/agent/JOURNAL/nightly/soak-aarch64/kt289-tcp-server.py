import socket, sys

port = int(sys.argv[1])
token = sys.argv[2].encode()
log = sys.argv[3]


def note(line):
    with open(log, "a") as handle:
        handle.write(line + "\n")


srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
try:
    srv.bind(("127.0.0.1", port))
except OSError as error:
    note("bind_err=%s" % error)
    sys.exit(2)
srv.listen(8)
note("listen port=%d" % port)
srv.settimeout(1.0)
while True:
    try:
        conn, addr = srv.accept()
    except socket.timeout:
        continue
    except OSError:
        break
    note("accept peer=%s:%d" % addr)
    try:
        conn.sendall(token)
        note("sent=%d" % len(token))
        conn.settimeout(5.0)
        total = 0
        while True:
            data = conn.recv(4096)
            if not data:
                break
            total += len(data)
        note("recv=%d" % total)
    except OSError as error:
        note("conn_err=%s" % error)
    finally:
        conn.close()
