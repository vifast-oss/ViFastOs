#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-245 + KT-246 (D2-196) — runner ảnh **test virtio-net** riscv64 (`vifast-kbin-ktest-net`).
#
# Vì sao runner riêng (không mở rộng `ktest.sh`): ảnh này cần **thiết bị mạng** trong dòng QEMU
# (`-netdev user,id=n0 -device virtio-net-pci,netdev=n0`) và có **hai ca âm** riêng (không có
# `-netdev`/`-device` ⇒ `net: no device`; có thiết bị nhưng **không có mạng** ⇒ `net: no link`).
# Nhét vào `ktest.sh` sẽ phải sửa manifest/expected.txt của 21 boot matrix cũ — rủi ro xung đột với
# các nhánh khác (KT-243..KT-251).
#
# Hợp đồng (fail-closed, không nới ngưỡng):
#   1. **Ca dương** (`-netdev user`): log phải có `TAP version 13` + `1..11` đúng một
#      lần, mười một dòng `ok 1..11` đúng **thứ tự** và đúng **tên**, **không** `not ok`, đúng
#      **một** dòng `PANIC: KT-268 net xong (11 TAP, 0 fail)`, và các dòng bằng chứng:
#        - MAC thật `52:54:00:…` (OUI QEMU) đọc từ thiết bị;
#        - `net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2`;
#        - `net: arp gw=<mac>` (MAC gateway khác 0, không broadcast);
#        - `net: ping 10.0.2.2 ok n=3 rtt=<ms>`;
#        - `# net rx:` có `oper=2` + `sender_ip=10.0.2.2` + `target_mac_khop=1`;
#        - **KT-260** `net: udp echo ok len=15 rtt=<ms>` + `# net udp echo:` có
#          `target=10.0.2.3:53 via=dns-question-echo … echoed=1` (libslirp **không** có echo UDP
#          ở `10.0.2.2:7`; fallback KT-260 dùng reply DNS làm bằng chứng UDP hai chiều);
#        - **KT-260** `net: dns example.com a=<ip> ttl=<n>` + `# net dns:` có `an=1`;
#        - **KT-260** hai dòng timeout (fail-closed, không treo): `# net udp echo:
#          target=10.0.2.2:7 result=Timeout elapsed_ms=3000 …` (một lần thử, 3 s) và
#          `# net dns: target=192.0.2.1:53 result=Timeout elapsed_ms=6000 attempts=2 …`
#          (**KT-294**: hai lần thử × 3 s — ngân sách **mỗi lần** vẫn `const:timeout_ms=3000`,
#          không nới ngưỡng; `attempts=2` là bằng chứng retransmit **thật**);
#        - **KT-268** `net: tcp connect 10.0.2.2:45678 ok rtt=<ms> state=established` +
#          `# net tcp connect:` (server TCP **thật** trên host: slirp ánh xạ `10.0.2.2` → loopback
#          của host — xem ghi chú `hostfwd` dưới đây);
#        - **KT-268** `net: tcp recv=13 first=vifast-tcp-ok` (đúng chuỗi **server gửi**) +
#          `# net tcp data:` có `sent=15 send=ok close=ok`; log **server** phải có `accept`,
#          `sent=13`, `recv=15` (bằng chứng hai chiều khách ↔ host);
#        - `# net stats:` có `errors=0 timeouts=0 id_mismatch=0`.
#   2. **Ca âm "không thiết bị"**: **cùng** ảnh, **không** `-netdev`/`-device` ⇒ log phải có
#      `net: no device` + `not ok 1 - net device`, **không** có `ok 1`, **không** có dòng PANIC
#      "xong". Runner coi ca âm **đạt** khi ảnh đỏ đúng cách; ảnh PASS giả ⇒ runner **đỏ**.
#   3. **Ca âm "không mạng"**: cùng ảnh, `-netdev socket,id=n0,listen=127.0.0.1:0` (thiết bị có
#      mặt, **không** peer nào trả lời) ⇒ log phải có `net: no link` + `not ok 3 - net dhcp`,
#      **không** có `ok 3`, **không** có dòng PANIC "xong". Chứng minh DHCP **fail-closed** chứ
#      không treo và không PASS giả.
#   4. `--red-selftest`: chứng minh bộ kiểm **không rỗng** — sửa log tốt (xoá `ok 3`/`ok 8`/`ok 9`/
#      `ok 10`/`ok 11`, đổi MAC, đổi `oper`, đổi `ip=`, đổi `len=`, đổi `a=`, đổi `first=`, đổi
#      `recv=`, đổi `send=ok`, xoá dòng timeout, đổi `attempts=`, thêm `not ok`, xoá `# net stats:`) rồi đòi
#      validator **từ chối** từng bản; và hai log ca âm "PASS giả" phải bị từ chối.
#
# **Ghi chú `hostfwd` (KT-268)**: đề bài gợi ý `-netdev user,hostfwd=tcp::<port>-:<port>`, nhưng
# `hostfwd` của QEMU là chiều **host → guest** (`hostfwd=[tcp]:[hostaddr]:hostport-[guestaddr]:
# guestport`), còn ta cần **guest → host**; đặt cùng cổng thì QEMU báo "Could not set up host
# forwarding rule" vì chính server đang giữ cổng đó. Đường **thật** cho guest → host của slirp là
# ánh xạ có sẵn `10.0.2.2` → `127.0.0.1` của host (libslirp `sotranslate_out4`), nên runner chỉ cần
# dựng server trên `127.0.0.1:$TCP_PORT` và khách nối `10.0.2.2:$TCP_PORT`. Bằng chứng hai chiều:
# log server có `accept peer=127.0.0.1:<cổng tạm>`, `sent=13`, `recv=15`.
#
# Dùng:
#   bash v2/test/boot/net.sh                 # build + ca dương + hai ca âm (PASS/FAIL)
#   bash v2/test/boot/net.sh --red-selftest  # chỉ tự kiểm validator (không build, không QEMU)
#   bash v2/test/boot/net.sh --check-log <log> [positive|negative|nolink]   # kiểm MỘT log đã có
#   KT245_SKIP_BUILD=1 bash v2/test/boot/net.sh                      # dùng ảnh có sẵn
#   KT245_LOGDIR=<dir> bash v2/test/boot/net.sh                      # nơi để log
# Exit: 0 = ca dương xanh + hai ca âm đỏ đúng; 1 = có lỗi; 2 = lỗi cấu hình môi trường.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
TARGET="riscv64gc-unknown-none-elf"
KMANIFEST="$V2_ROOT/kernel/Cargo.toml"
IMAGE="${KT245_IMAGE:-${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release/vifast-kbin-ktest-net}"
# **KT-281** (D2-210, N1 phần 3) — ảnh **net server U-mode**. Runner này chỉ chạy **ca âm**
# "không thiết bị" của nó (`# netsrv: no device` + `not ok 1`); ca dương (2 TAP) nằm trong
# `ktest.sh` matrix vì ảnh đó là ảnh thứ **chín** của matrix.
NETSRV_IMAGE="${KT281_IMAGE:-${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release/vifast-kbin-ktest-netsrv}"
LOGDIR="${KT245_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/kt245-net}"
BUDGET="$HERE/budget.sh"
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
QEMU_ICOUNT="-icount shift=6" # D2-74 (chỉ runner; run.sh/soak không icount)
QEMU_MIN_MAJOR=9
QEMU_MIN_MINOR=2
# Số TAP của ảnh — runner đòi đúng mười một dòng `ok 1..11` đúng **tên** (thêm/bớt TAP phải sửa
# cả kernel, tệp này và `# net stats` trong cùng commit).
TAP_COUNT=11
# Dòng kết thúc chủ đích của ảnh (khớp `ktest_net.rs`).
PANIC_OK='^PANIC: KT-268 net xong \(11 TAP, 0 fail\)$'
# OUI mà QEMU gán cho card slirp — MAC **đọc từ thiết bị**, hằng này chỉ để đối chiếu nguồn gốc.
MAC_PREFIX="52:54:00:"
# Giá trị DHCP mà slirp của QEMU cấp (dữ kiện nền tảng QEMU, không phải số đo của kernel).
DHCP_LINE='^# net: dhcp ip=10\.0\.2\.15 gw=10\.0\.2\.2 mask=255\.255\.255\.0 server=10\.0\.2\.2$'
PING_LINE='^# net: ping 10\.0\.2\.2 ok n=3 rtt=[0-9]+$'
ARP_LINE='^# net: arp gw=[0-9a-f]{2}(:[0-9a-f]{2}){5}$'
# **KT-260** — UDP echo: payload `vifast-udp-echo` dài 15 byte; "gương" là reply DNS (fallback vì
# libslirp không có echo UDP ở 10.0.2.2:7). Dòng timeout chứng minh fail-closed đúng 3 s.
UDP_ECHO_LINE='^# net: udp echo ok len=15 rtt=[0-9]+$'
UDP_ECHO_TIMEOUT='^# net udp echo: target=10\.0\.2\.2:7 result=Timeout elapsed_ms=3000 const:timeout_ms=3000$'
UDP_ECHO_DNS='^# net udp echo: target=10\.0\.2\.3:53 via=dns-question-echo payload_len=15 reply_len=[0-9]+ rcode=[0-9]+ echoed=1 rtt_ms=[0-9]+ elapsed_ms=[0-9]+ const:timeout_ms=3000$'
# **KT-260** — DNS: `example.com` A; TTL do resolver trả (không hardcode).
DNS_A_LINE='^# net: dns example\.com a=([0-9]{1,3}\.){3}[0-9]{1,3} ttl=[0-9]+$'
# **KT-294** — `attempts=1` (lần đầu được) hoặc `attempts=2` (lần đầu timeout, gửi lại **cùng
# `id`**); `attempts=3` là **retransmit vô hạn** ⇒ ĐỎ. Ngân sách **mỗi lần thử** vẫn 3000 ms.
DNS_REPLY='^# net dns: name=example\.com id=0x4b55 rcode=0 an=1 reply_len=[0-9]+ rtt_ms=[0-9]+ elapsed_ms=[0-9]+ attempts=(1|2) const:timeout_ms=3000$'
# Đích **không có** server ⇒ **cả hai** lần thử timeout: `elapsed_ms=6000` (2 × 3000) và
# `attempts=2`. Đây là bằng chứng retransmit **thật** trên phần cứng, không phải suy luận từ mã.
DNS_TIMEOUT='^# net dns: target=192\.0\.2\.1:53 result=Timeout elapsed_ms=6000 attempts=2 const:timeout_ms=3000$'
# **KT-268** — TCP: server **thật** trên host. Runner dựng `python3` lắng nghe
# `127.0.0.1:$TCP_PORT`; khách nối `10.0.2.2:$TCP_PORT` (slirp ánh xạ 10.0.2.2 → loopback host —
# xem ghi chú `hostfwd` ở đầu tệp). Server gửi đúng `$TCP_TOKEN` (13 byte) rồi chờ khách gửi
# `$TCP_SEND_LEN` byte + FIN; log server là bằng chứng **hai chiều**.
TCP_PORT=45678
TCP_TOKEN="vifast-tcp-ok"
TCP_TOKEN_LEN=${#TCP_TOKEN}
TCP_SEND_LEN=15 # `vifast-tcp-send`
TCP_CONNECT_LINE="^# net: tcp connect 10\.0\.2\.2:${TCP_PORT} ok rtt=[0-9]+ state=established$"
TCP_RECV_LINE="^# net: tcp recv=${TCP_TOKEN_LEN} first=${TCP_TOKEN}$"
TCP_DATA_LINE="^# net tcp data: recv=${TCP_TOKEN_LEN} sent=${TCP_SEND_LEN} send=ok close=ok state=(fin-wait-2|time-wait|closing|last-ack|closed) retransmits=[0-9]+ elapsed_ms=[0-9]+ const:timeout_ms=3000$"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

FAILS=0
CHECKS=0

die() {
    echo "net.sh: $*" >&2
    exit 2
}

bad() {
    echo "not ok - $*"
    FAILS=$((FAILS + 1))
}

ok() {
    CHECKS=$((CHECKS + 1))
    echo "ok $CHECKS - $*"
}

# KT-78 (OD-K6-6): **mọi** boot đi qua `qemu_guard.sh` — QEMU trong process group riêng, timeout
# giết cả group, trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL).
. "$HERE/qemu_guard.sh"
# **KT-411b** — khoá + phát hành ảnh kiểm dùng chung (một tên tệp build cho 13 suite).
. "$HERE/lib/ktest-build.sh"
# **KT-333e** — helper dùng chung (`v2_env_wait_port_free`) từ `lib/env.sh` (KT-333d).
[ -f "$HERE/lib/env.sh" ] && . "$HERE/lib/env.sh"

# ---------------------------------------------------------------- tìm QEMU + OpenSBI
find_qemu() {
    QEMU="$(command -v qemu-system-riscv64 || true)"
    [ -n "$QEMU" ] || die "không có qemu-system-riscv64"
    local version major minor
    version="$("$QEMU" --version 2>/dev/null | head -1 | sed -n 's/.*version \([0-9][0-9]*\)\.\([0-9][0-9]*\).*/\1 \2/p')"
    major="${version%% *}"
    minor="${version##* }"
    case "$major" in '' | *[!0-9]*) die "không đọc được phiên bản QEMU" ;; esac
    if [ "$major" -lt "$QEMU_MIN_MAJOR" ] ||
        { [ "$major" -eq "$QEMU_MIN_MAJOR" ] && [ "$minor" -lt "$QEMU_MIN_MINOR" ]; }; then
        die "QEMU $major.$minor < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2)"
    fi
    SBI="${OPENSBI:-}"
    if [ -z "$SBI" ]; then
        for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
            if [ -f "$c" ]; then
                SBI="$c"
                break
            fi
        done
    fi
    [ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"
    echo "# qemu $version ($QEMU)"
    echo "# opensbi $SBI"
}

# ---------------------------------------------------------------- build
build_image() {
    command -v cargo >/dev/null 2>&1 || die "không có cargo"
    if [ "${KT245_SKIP_BUILD:-0}" != "1" ]; then
        # **KT-411b** (D2-259 mục 2): **một** bin kiểm `vifast-kbin-ktest`, bộ ca chọn bằng
        # feature `suite-*`; `cp` ra **đúng tên ảnh cũ** nên `IMAGE`/`budget.sh` không đổi.
        echo "# build ảnh net ($TARGET, release, --features ktest,suite-net)"
        if ! ktest_build_suite suite-net "$LOGDIR/kt245-build-image.log" "$IMAGE" \
            "$KMANIFEST" "$TARGET"; then
            tail -20 "$LOGDIR/kt245-build-image.log" >&2
            die "build ảnh net FAIL"
        fi
    fi
    if [ "${KT245_SKIP_BUILD:-0}" != "1" ]; then
        echo "# build ảnh netsrv ($TARGET, release, --features ktest,suite-netsrv) — KT-281"
        if ! ktest_build_suite suite-netsrv "$LOGDIR/kt281-build-netsrv.log" "$NETSRV_IMAGE" \
            "$KMANIFEST" "$TARGET"; then
            tail -20 "$LOGDIR/kt281-build-netsrv.log" >&2
            die "build ảnh netsrv FAIL"
        fi
    fi
    [ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
    [ -f "$NETSRV_IMAGE" ] || die "không thấy ảnh $NETSRV_IMAGE"
    if ! bash "$BUDGET" "$NETSRV_IMAGE" ktest-netsrv >"$LOGDIR/kt281-budget.log" 2>&1; then
        cat "$LOGDIR/kt281-budget.log" >&2
        die "budget ảnh netsrv VƯỢT trần"
    fi
    grep '^# image=' "$LOGDIR/kt281-budget.log" || true
    # D2-72 Q5: trần `.data+.bss` 262.144 B — ảnh mới cũng chịu trần, không nới.
    if ! bash "$BUDGET" "$IMAGE" ktest-net >"$LOGDIR/kt245-budget.log" 2>&1; then
        cat "$LOGDIR/kt245-budget.log" >&2
        die "budget ảnh net VƯỢT trần"
    fi
    grep '^# image=' "$LOGDIR/kt245-budget.log" || true
}

# ---------------------------------------------------------------- boot
# `boot <log> [tham số QEMU thêm]`: 0 = QEMU tự thoát; 124 = quá 20 s đã kill cả group.
boot() {
    local out="$1"
    shift
    qg_boot "$out" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot "$@"
}

# `boot_netsrv <log> [tham số QEMU thêm]`: như [`boot`] nhưng nạp ảnh **netsrv** (KT-281).
boot_netsrv() {
    local out="$1"
    shift
    qg_boot "$out" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$NETSRV_IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot "$@"
}

# ---------------------------------------------------------------- validator
# Mọi phép kiểm đi qua `bad` (tăng `FAILS` **toàn cục**) và validator trả **hiệu số** ⇒ không có
# đường "kiểm đỏ mà vẫn trả 0" (bẫy đã tự gặp ở bản đầu: `bad` tăng biến toàn cục còn validator
# trả biến cục bộ ⇒ log xấu vẫn được chấp nhận).
LOG=""

# `check_tap <số> <tên>`: đòi đúng **một** dòng `ok <số> - <tên>`.
check_tap() {
    local number="$1" name="$2" line count
    line="ok $number - $name"
    count="$(grep -cxF "$line" "$LOG")"
    if [ "$count" != 1 ]; then
        bad "dòng '$line' xuất hiện $count lần (đòi 1)"
    fi
}

# `check_grep <mô tả> <regex>`: đòi **ít nhất một** dòng khớp.
check_grep() {
    local what="$1" pattern="$2"
    if ! grep -qE "$pattern" "$LOG"; then
        bad "không thấy $what (mẫu: $pattern)"
    fi
}

# `check_count <mô tả> <regex> <số đòi>`: đòi **đúng** số dòng khớp.
check_count() {
    local what="$1" pattern="$2" want="$3" count
    count="$(grep -cE "$pattern" "$LOG")"
    if [ "$count" != "$want" ]; then
        bad "$what xuất hiện $count lần (đòi $want)"
    fi
}

# `validate_positive <log>`: 0 = log đạt **mọi** điều kiện của ca dương.
validate_positive() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca dương: không có log $LOG"
        return 1
    fi
    # 1) TAP version + kế hoạch, đúng một lần.
    check_count "ca dương: 'TAP version 13'" '^TAP version 13$' 1
    check_count "ca dương: dòng kế hoạch '1..$TAP_COUNT'" "^1\\.\\.$TAP_COUNT\$" 1
    # 2) Chín dòng `ok N - <tên>` đúng thứ tự, đúng tên, mỗi dòng đúng một lần.
    check_tap 1 "net device"
    check_tap 2 "net mac"
    check_tap 3 "net dhcp"
    check_tap 4 "net arp"
    check_tap 5 "net ping"
    check_tap 6 "net tx"
    check_tap 7 "net rx"
    check_tap 8 "net udp echo"
    check_tap 9 "net dns a"
    check_tap 10 "net tcp connect"
    check_tap 11 "net tcp data"
    # 3) Không `not ok` nào.
    check_count "ca dương: dòng 'not ok'" '^not ok' 0
    # 4) Đúng **một** dòng PANIC kết thúc, đúng mẫu.
    check_count "ca dương: dòng PANIC kết thúc" "$PANIC_OK" 1
    # 5) Bằng chứng MAC **thật** của QEMU (đọc từ thiết bị, không hardcode trong ảnh).
    check_grep "MAC thật '${MAC_PREFIX}…' trong '# net dev:'" \
        "^# net dev: mac=${MAC_PREFIX}[0-9a-f]{2}:[0-9a-f]{2}:[0-9a-f]{2}( |\$)"
    # 6) DHCP: địa chỉ/mặt nạ/gateway/server **đúng giá trị slirp cấp**.
    check_grep "dòng 'net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2'" \
        "$DHCP_LINE"
    # 7) ARP: MAC gateway đọc được, khác 0 và không broadcast.
    check_grep "dòng 'net: arp gw=<mac>'" "$ARP_LINE"
    check_count "ca dương: ARP gateway toàn 0/broadcast" \
        '^# net: arp gw=(00:00:00:00:00:00|ff:ff:ff:ff:ff:ff)$' 0
    # 8) ICMP echo: đủ 3 reply.
    check_grep "dòng 'net: ping 10.0.2.2 ok n=3 rtt=<ms>'" "$PING_LINE"
    # 9) Bằng chứng RX: ARP reply của slirp (oper=2, sender 10.0.2.2, target MAC khớp).
    check_grep "ARP reply 'oper=2 sender_ip=10.0.2.2' trong '# net rx:'" \
        '^# net rx: frame=[0-9]+ oper=2 ethertype=0x806 sender_ip=10\.0\.2\.2( |$)'
    check_grep "ARP reply trỏ đúng MAC của ta (target_mac_khop=1)" \
        '^# net rx: .*target_mac_khop=1( |$)'
    # 10) Bộ đếm sạch: không lỗi, không timeout, không lệch used->id.
    check_grep "'# net stats:' có errors=0 timeouts=0 id_mismatch=0" \
        '^# net stats: .*errors=0 timeouts=0 id_mismatch=0( |$)'
    # 11) **KT-260** — UDP echo: payload 15 byte quay lại **đúng** qua "gương" DNS, và dòng
    #     timeout 3 s của `udp_echo` tới 10.0.2.2:7 (libslirp không có echo UDP).
    check_grep "dòng 'net: udp echo ok len=15 rtt=<ms>'" "$UDP_ECHO_LINE"
    check_grep "dòng timeout UDP 'target=10.0.2.2:7 result=Timeout elapsed_ms=3000'" \
        "$UDP_ECHO_TIMEOUT"
    check_grep "dòng 'net udp echo:' via=dns-question-echo … echoed=1" "$UDP_ECHO_DNS"
    # 12) **KT-260** — DNS: bản ghi A của example.com + dòng timeout 3 s trên TEST-NET-1.
    check_grep "dòng 'net: dns example.com a=<ip> ttl=<n>'" "$DNS_A_LINE"
    check_grep "dòng 'net dns:' rcode=0 an=1 cho example.com" "$DNS_REPLY"
    check_grep "dòng timeout DNS 'target=192.0.2.1:53 result=Timeout elapsed_ms=3000'" \
        "$DNS_TIMEOUT"
    # 13) **KT-268** — TCP: mở tới **server thật trên host** (ánh xạ 10.0.2.2 của slirp), nhận **đúng** chuỗi
    #     server gửi, gửi lại payload và đóng bằng FIN.
    check_grep "dòng 'net: tcp connect 10.0.2.2:45678 ok rtt=<ms> state=established'" \
        "$TCP_CONNECT_LINE"
    check_grep "dòng 'net: tcp recv=13 first=vifast-tcp-ok'" "$TCP_RECV_LINE"
    check_grep "dòng 'net tcp data:' recv=13 sent=15 send=ok close=ok" "$TCP_DATA_LINE"
    # 14) Hai ca âm **không** được lẫn vào log ca dương.
    check_count "ca dương: dòng 'net: no device'" 'net: no device' 0
    check_count "ca dương: dòng 'net: no link'" 'net: no link' 0
    check_count "ca dương: dòng 'net: no udp'" 'net: no udp' 0
    check_count "ca dương: dòng 'net: no dns'" 'net: no dns' 0
    check_count "ca dương: dòng 'net: no tcp'" 'net: no tcp' 0
    return $((FAILS - before))
}

# `validate_negative <log>`: 0 = ảnh **đỏ đúng cách** khi không có thiết bị mạng.
validate_negative() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca âm: không có log $LOG"
        return 1
    fi
    # 1) Ảnh phải in đúng dòng fail-closed.
    check_grep "dòng fail-closed 'net: no device'" 'net: no device'
    # 2) TAP 1 phải **đỏ**.
    check_count "ca âm: 'not ok 1 - net device'" '^not ok 1 - net device' 1
    # 3) **Không** được có `ok 1` — đây là chỗ bắt "PASS giả".
    check_count "ca âm: 'ok 1 - net device'" '^ok 1 - net device' 0
    # 4) **Không** được có dòng kết thúc "xong" (nghĩa là đã đi hết 11 TAP).
    check_count "ca âm: dòng PANIC 'xong'" "$PANIC_OK" 0
    return $((FAILS - before))
}

# `validate_nolink <log>`: 0 = ảnh **đỏ đúng cách** khi có thiết bị nhưng **không có mạng**.
validate_nolink() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca không mạng: không có log $LOG"
        return 1
    fi
    # 1) Thiết bị **phải** được nhận (nếu không, ca này trùng ca "không thiết bị").
    check_count "ca không mạng: 'ok 1 - net device'" '^ok 1 - net device' 1
    check_count "ca không mạng: 'ok 2 - net mac'" '^ok 2 - net mac' 1
    # 2) DHCP phải **fail-closed** với dòng cố định.
    check_grep "dòng fail-closed 'net: no link'" 'net: no link'
    check_count "ca không mạng: 'not ok 3 - net dhcp'" '^not ok 3 - net dhcp' 1
    # 3) **Không** được có `ok 3` — đây là chỗ bắt "PASS giả".
    check_count "ca không mạng: 'ok 3 - net dhcp'" '^ok 3 - net dhcp' 0
    # 4) **Không** được có dòng kết thúc "xong".
    check_count "ca không mạng: dòng PANIC 'xong'" "$PANIC_OK" 0
    return $((FAILS - before))
}

# `validate_netsrv_negative <log>`: 0 = ảnh **netsrv** đỏ **đúng cách** khi không có thiết bị mạng.
#
# **KT-281** — ca âm thường trực của dòng `# netsrv:`. Ảnh phải in đúng dòng fail-closed
# `# netsrv: no device`, TAP 1 **đỏ**, **không** có `ok 1`, **không** có dòng PANIC "xong".
validate_netsrv_negative() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "netsrv ca âm: không có log $LOG"
        return 1
    fi
    # 1) Ảnh phải in đúng dòng fail-closed.
    check_grep "dòng fail-closed 'netsrv: no device'" '^# netsrv: no device$'
    # 2) TAP 1 phải **đỏ**.
    check_count "netsrv ca âm: 'not ok 1 - netsrv: server ran'" '^not ok 1 - netsrv: server ran' 1
    # 3) **Không** được có `ok 1` — đây là chỗ bắt "PASS giả".
    check_count "netsrv ca âm: 'ok 1 - netsrv: server ran'" '^ok 1 - netsrv: server ran' 0
    # 4) **Không** được có dòng kết thúc "xong" (nghĩa là đã đi hết 3 TAP).
    check_count "netsrv ca âm: dòng PANIC 'xong'" \
        '^PANIC: KT-306 netping xong \(3 TAP, 0 fail\)$' 0
    # 5) **Không** được có dòng do chương trình U-mode in (nếu có, ảnh đã chạy được dịch vụ mạng
    #    mà không có thiết bị ⇒ bằng chứng giả).
    check_count "netsrv ca âm: dòng '# netsrv: ping'" '^# netsrv: ping ' 0
    check_count "netsrv ca âm: dòng '# netsrv: client got reply'" '^# netsrv: client got reply=' 0
    # 6) **KT-306** — không có dòng `# netping:` nào (syscall `NetPing` không được chạy khi thiếu
    #    thiết bị) và không có `ok 3 - netping: via ipc`.
    check_count "netsrv ca âm: dòng '# netping:'" '^# netping: ' 0
    check_count "netsrv ca âm: dòng '# netping kern:'" '^# netping kern: ' 0
    check_count "netsrv ca âm: 'ok 3 - netping: via ipc'" '^ok 3 - netping: via ipc$' 0
    return $((FAILS - before))
}

# ---------------------------------------------------------------- TCP server trên host (KT-268)
# Vì sao `python3` chứ không `socat`/`nc`: hai công cụ đó **có thể không có** trên máy đo; python3
# là thứ runner đã đòi cho các bước khác. Server ghi log từng mốc (`listen`/`accept`/`sent`/`recv`)
# để runner có bằng chứng **hai chiều** khách ↔ host, không chỉ tin log của khách.
TCP_SERVER_PID=""

write_tcp_server() {
    cat >"$LOGDIR/kt268-tcp-server.py" <<'PY'
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
srv.listen(1)
note("listen port=%d" % port)
srv.settimeout(60.0)
try:
    conn, addr = srv.accept()
except socket.timeout:
    note("accept_timeout")
    sys.exit(3)
note("accept peer=%s:%d" % addr)
conn.sendall(token)
note("sent=%d" % len(token))
conn.settimeout(5.0)
total = 0
try:
    while True:
        data = conn.recv(4096)
        if not data:
            break
        total += len(data)
except socket.timeout:
    note("recv_timeout")
note("recv=%d" % total)
conn.close()
srv.close()
PY
}

# `start_tcp_server <log>`: dựng server, chờ dòng `listen` (tối đa 5 s). 0 = sẵn sàng.
start_tcp_server() {
    local srv_log="$1" i
    command -v python3 >/dev/null 2>&1 || {
        echo "net.sh: không có python3 để dựng TCP server" >&2
        return 1
    }
    write_tcp_server
    # **KT-333e** — cổng `45678` là hằng số **hai phía** (khách trong kernel gọi
    # `10.0.2.2:45678`) ⇒ **chờ** cổng trống (4 slot chạy đêm) thay vì đỏ oan.
    if command -v v2_env_wait_port_free >/dev/null 2>&1; then
        v2_env_wait_port_free "$TCP_PORT" 180 || return 1
    fi
    rm -f "$srv_log"
    python3 "$LOGDIR/kt268-tcp-server.py" "$TCP_PORT" "$TCP_TOKEN" "$srv_log" &
    TCP_SERVER_PID=$!
    for i in $(seq 1 50); do
        if grep -q '^listen ' "$srv_log" 2>/dev/null; then
            return 0
        fi
        if grep -q '^bind_err=' "$srv_log" 2>/dev/null; then
            echo "net.sh: TCP server không bind được 127.0.0.1:$TCP_PORT — port đang bận?" >&2
            cat "$srv_log" >&2
            return 1
        fi
        sleep 0.1
    done
    echo "net.sh: TCP server không lắng nghe sau 5 s" >&2
    return 1
}

# `wait_tcp_server <log>`: chờ server xong (tối đa 20 s) và đòi đủ ba mốc bằng chứng.
wait_tcp_server() {
    local srv_log="$1" i
    for i in $(seq 1 200); do
        if grep -qE '^(recv=|accept_timeout|bind_err)' "$srv_log" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done
    if [ -n "$TCP_SERVER_PID" ]; then
        if kill -0 "$TCP_SERVER_PID" 2>/dev/null; then
            kill "$TCP_SERVER_PID" 2>/dev/null || true
            echo "net.sh: server TCP vẫn chạy sau 20 s — đã kill" >&2
            TCP_SERVER_PID=""
            return 1
        fi
        wait "$TCP_SERVER_PID" 2>/dev/null || true
        TCP_SERVER_PID=""
    fi
    grep -q '^accept ' "$srv_log" || {
        echo "net.sh: server TCP không nhận kết nối từ khách" >&2
        return 1
    }
    grep -qx "sent=$TCP_TOKEN_LEN" "$srv_log" || {
        echo "net.sh: server TCP không gửi đủ $TCP_TOKEN_LEN byte" >&2
        return 1
    }
    grep -qx "recv=$TCP_SEND_LEN" "$srv_log" || {
        echo "net.sh: server TCP không nhận đúng $TCP_SEND_LEN byte từ khách" >&2
        return 1
    }
    return 0
}

stop_tcp_server() {
    if [ -n "$TCP_SERVER_PID" ]; then
        kill "$TCP_SERVER_PID" 2>/dev/null || true
        wait "$TCP_SERVER_PID" 2>/dev/null || true
        TCP_SERVER_PID=""
    fi
}

# ---------------------------------------------------------------- ba ca
run_positive() {
    local log="$LOGDIR/kt245-positive.log"
    local srv_log="$LOGDIR/kt268-tcp-server.log"
    echo "# ca dương: -netdev user,id=n0 -device virtio-net-pci,netdev=n0 (TCP server host 127.0.0.1:$TCP_PORT qua ánh xạ 10.0.2.2 của slirp)"
    if ! start_tcp_server "$srv_log"; then
        bad "ca dương: không dựng được TCP server trên 127.0.0.1:$TCP_PORT"
        stop_tcp_server
        return 1
    fi
    if ! boot "$log" -netdev user,id=n0 -device virtio-net-pci,netdev=n0; then
        bad "ca dương: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        stop_tcp_server
        return 1
    fi
    if ! wait_tcp_server "$srv_log"; then
        bad "ca dương: server TCP không có đủ bằng chứng hai chiều (accept/sent/recv)"
        return 1
    fi
    if validate_positive "$log"; then
        ok "ca dương: 11/11 TAP + MAC thật + DHCP + ARP + ping + ARP reply + UDP echo + DNS A + TCP connect/data + bộ đếm sạch ($log)"
        return 0
    fi
    return 1
}

run_negative() {
    local log="$LOGDIR/kt245-negative.log"
    echo "# ca âm: cùng ảnh, KHÔNG -netdev/-device"
    if ! boot "$log"; then
        bad "ca âm: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_negative "$log"; then
        ok "ca âm: ảnh in 'net: no device' + TAP 1 đỏ, không PASS giả ($log)"
        return 0
    fi
    return 1
}

run_nolink() {
    local log="$LOGDIR/kt246-nolink.log"
    # `-netdev socket` **không** peer: thiết bị có mặt, link "up" với guest, nhưng **không** ai trả
    # lời DHCP ⇒ đúng ca "thiếu mạng" mà D2-196 mục (3) đòi.
    echo "# ca không mạng: -netdev socket,id=n0,listen=127.0.0.1:0 (không peer)"
    if ! boot "$log" -netdev socket,id=n0,listen=127.0.0.1:0 -device virtio-net-pci,netdev=n0; then
        bad "ca không mạng: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_nolink "$log"; then
        ok "ca không mạng: ảnh in 'net: no link' + TAP 3 đỏ, không treo, không PASS giả ($log)"
        return 0
    fi
    return 1
}

# **KT-281** — ca âm "không thiết bị" của ảnh **net server U-mode**: cùng ảnh, **không**
# `-netdev`/`-device` ⇒ `# netsrv: no device` + `not ok 1`, không treo, không PASS giả.
run_netsrv_negative() {
    local log="$LOGDIR/kt281-netsrv-negative.log"
    echo "# netsrv ca âm: cùng ảnh netsrv, KHÔNG -netdev/-device"
    if ! boot_netsrv "$log"; then
        bad "netsrv ca âm: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_netsrv_negative "$log"; then
        ok "netsrv ca âm: ảnh in 'netsrv: no device' + TAP 1 đỏ, không PASS giả ($log)"
        return 0
    fi
    return 1
}

# ---------------------------------------------------------------- tự kiểm validator
# `expect_accept <mô tả> <hàm> <log>` / `expect_reject …`: chạy validator và đòi kết quả.
# Lỗi do **ca cố ý xấu** không tính vào `FAILS` cuối (chỉ tính khi kỳ vọng sai) — nếu không, dòng
# tổng kết sẽ báo "N lỗi" dù validator đúng.
expect_accept() {
    local what="$1" fn="$2" log="$3" before="$FAILS"
    if "$fn" "$log" >/dev/null 2>&1; then
        ok "selftest: $what được chấp nhận"
    else
        bad "selftest: $what bị TỪ CHỐI (validator quá chặt/sai)"
    fi
    FAILS="$before"
}

expect_reject() {
    local what="$1" fn="$2" log="$3" before="$FAILS"
    if "$fn" "$log" >/dev/null 2>&1; then
        bad "selftest: $what KHÔNG bị bắt (validator rỗng)"
    else
        ok "selftest: $what bị từ chối"
    fi
    FAILS="$before"
}

red_selftest() {
    local good="$LOGDIR/kt245-selftest-good.log"
    local neg="$LOGDIR/kt245-selftest-neg.log"
    local nolink="$LOGDIR/kt245-selftest-nolink.log"
    local tmp="$LOGDIR/kt245-selftest-mut.log"
    # Log **tốt** dựng tay: đúng mười một TAP + đủ bằng chứng (không phải log QEMU thật — chỉ để
    # kiểm validator, không dùng làm bằng chứng KT-245/246/260/268).
    cat >"$good" <<'EOF'
TAP version 13
1..11
# net dev: mac=52:54:00:12:34:56 status=0x1 mtu_raw=65535
ok 1 - net device
ok 2 - net mac
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
ok 3 - net dhcp
# net: arp gw=52:55:0a:00:02:02
ok 4 - net arp
# net: ping 10.0.2.2 ok n=3 rtt=1
ok 5 - net ping
# net tx: frame=42 (=14 ethernet + 28 arp) result=ok
ok 6 - net tx
# net rx: frame=64 oper=2 ethertype=0x806 sender_ip=10.0.2.2 sender_mac=52:55:0a:00:02:02 target_mac_khop=1
ok 7 - net rx
# net udp echo: target=10.0.2.2:7 result=Timeout elapsed_ms=3000 const:timeout_ms=3000
# net: arp dns=52:55:0a:00:02:03 ip=10.0.2.3 elapsed_ms=0
# net udp echo: target=10.0.2.3:53 via=dns-question-echo payload_len=15 reply_len=33 rcode=0 echoed=1 rtt_ms=9 elapsed_ms=9 const:timeout_ms=3000
# net: udp echo ok len=15 rtt=9
ok 8 - net udp echo
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=61 rtt_ms=10 elapsed_ms=10 attempts=1 const:timeout_ms=3000
# net: dns example.com a=104.20.23.154 ttl=300
ok 9 - net dns a
# net dns: target=192.0.2.1:53 result=Timeout elapsed_ms=6000 attempts=2 const:timeout_ms=3000
# net: tcp connect 10.0.2.2:45678 ok rtt=2 state=established
# net tcp connect: iss=0x4b540001 snd_una=0x4b540002 rcv_nxt=0x20000001 retransmits=0 elapsed_ms=2 const:timeout_ms=3000 const:port=45678
ok 10 - net tcp connect
# net: tcp recv=13 first=vifast-tcp-ok
# net tcp data: recv=13 sent=15 send=ok close=ok state=fin-wait-2 retransmits=0 elapsed_ms=3 const:timeout_ms=3000
ok 11 - net tcp data
# net stats: tx_frames=12 tx_bytes=1218 rx_frames=10 rx_bytes=1772 notifies=23 errors=0 timeouts=0 id_mismatch=0
PANIC: KT-268 net xong (11 TAP, 0 fail)
EOF
    expect_accept "log tốt dựng tay" validate_positive "$good"
    # Hai mươi lăm đột biến phải bị **từ chối** — nếu không, validator rỗng.
    local name
    for name in xoa_ok3 doi_mac doi_oper doi_ip them_not_ok xoa_stats doi_ping_n \
        xoa_ok8 xoa_ok9 doi_udp_len doi_echoed doi_dns_a doi_dns_ttl doi_rcode \
        xoa_udp_timeout xoa_dns_timeout doi_attempts doi_dns_timeout_attempts \
        xoa_ok10 xoa_ok11 doi_tcp_port doi_tcp_recv \
        doi_tcp_first doi_tcp_send doi_tcp_state; do
        case "$name" in
            xoa_ok3) grep -v '^ok 3 - net dhcp$' "$good" >"$tmp" ;;
            doi_mac) sed 's/mac=52:54:00:/mac=02:00:00:/' "$good" >"$tmp" ;;
            doi_oper) sed 's/oper=2/oper=1/' "$good" >"$tmp" ;;
            doi_ip) sed 's/ip=10.0.2.15/ip=10.0.2.99/' "$good" >"$tmp" ;;
            them_not_ok) sed 's/^ok 2 - net mac$/not ok 2 - net mac/' "$good" >"$tmp" ;;
            xoa_stats) grep -v '^# net stats:' "$good" >"$tmp" ;;
            doi_ping_n) sed 's/ok n=3 rtt=/ok n=2 rtt=/' "$good" >"$tmp" ;;
            xoa_ok8) grep -v '^ok 8 - net udp echo$' "$good" >"$tmp" ;;
            xoa_ok9) grep -v '^ok 9 - net dns a$' "$good" >"$tmp" ;;
            doi_udp_len) sed 's/udp echo ok len=15/udp echo ok len=14/' "$good" >"$tmp" ;;
            doi_echoed) sed 's/echoed=1/echoed=0/' "$good" >"$tmp" ;;
            doi_dns_a) sed 's/a=104.20.23.154/a=1.2.3/' "$good" >"$tmp" ;;
            doi_dns_ttl) sed 's/ttl=300/ttl=x/' "$good" >"$tmp" ;;
            doi_rcode) sed 's/rcode=0 an=1/rcode=3 an=1/' "$good" >"$tmp" ;;
            xoa_udp_timeout) grep -v '^# net udp echo: target=10.0.2.2:7 result=Timeout' "$good" >"$tmp" ;;
            xoa_dns_timeout) grep -v '^# net dns: target=192.0.2.1:53 result=Timeout' "$good" >"$tmp" ;;
            # **KT-294**: `attempts=3` = retransmit vô hạn ⇒ phải bị từ chối.
            doi_attempts) sed 's/ attempts=1 const:timeout_ms=3000$/ attempts=3 const:timeout_ms=3000/' "$good" >"$tmp" ;;
            # **KT-294**: đột biến "bỏ retransmit" ⇒ dòng timeout chỉ có `attempts=1`/3000 ms.
            doi_dns_timeout_attempts) sed 's/elapsed_ms=6000 attempts=2 const:timeout_ms=3000$/elapsed_ms=3000 attempts=1 const:timeout_ms=3000/' "$good" >"$tmp" ;;
            xoa_ok10) grep -v '^ok 10 - net tcp connect$' "$good" >"$tmp" ;;
            xoa_ok11) grep -v '^ok 11 - net tcp data$' "$good" >"$tmp" ;;
            doi_tcp_port) sed 's/10.0.2.2:45678/10.0.2.2:45679/' "$good" >"$tmp" ;;
            doi_tcp_recv) sed 's/tcp recv=13/tcp recv=12/' "$good" >"$tmp" ;;
            doi_tcp_first) sed 's/first=vifast-tcp-ok/first=vifast-tcp-NO/' "$good" >"$tmp" ;;
            doi_tcp_send) sed 's/send=ok close=ok/send=Timeout close=ok/' "$good" >"$tmp" ;;
            doi_tcp_state) sed 's/state=fin-wait-2/state=syn-sent/' "$good" >"$tmp" ;;
        esac
        expect_reject "đột biến '$name'" validate_positive "$tmp"
    done
    # Ca âm "không thiết bị": log đỏ đúng cách ⇒ chấp nhận; log "PASS giả" (có `ok 1`) ⇒ từ chối.
    cat >"$neg" <<'EOF'
TAP version 13
1..11
# net pci: err=NoDevice stage=nodevice
# net: no device
not ok 1 - net device: không tìm/khởi tạo được PCI
# ktest halt
EOF
    expect_accept "log ca âm 'không thiết bị' đỏ đúng cách" validate_negative "$neg"
    sed 's/^not ok 1 - net device.*$/ok 1 - net device/' "$neg" >"$tmp"
    expect_reject "ca âm 'không thiết bị' PASS giả (có 'ok 1')" validate_negative "$tmp"
    # Ca âm "không mạng": DHCP fail-closed ⇒ chấp nhận; DHCP "PASS giả" ⇒ từ chối.
    cat >"$nolink" <<'EOF'
TAP version 13
1..11
ok 1 - net device
ok 2 - net mac
# net: no link
not ok 3 - net dhcp: không nhận được DHCP OFFER/ACK trong 3 s
# ktest halt
EOF
    expect_accept "log ca 'không mạng' đỏ đúng cách" validate_nolink "$nolink"
    sed 's/^not ok 3 - net dhcp.*$/ok 3 - net dhcp/' "$nolink" >"$tmp"
    expect_reject "ca 'không mạng' PASS giả (có 'ok 3')" validate_nolink "$tmp"
    # **KT-281** — ca âm của ảnh **net server U-mode**: log đỏ đúng cách ⇒ chấp nhận; ba bản
    # "PASS giả"/thiếu bằng chứng ⇒ **từ chối** (validator không rỗng).
    local netsrv="$LOGDIR/kt281-selftest-netsrv-neg.log"
    cat >"$netsrv" <<'EOF'
TAP version 13
1..2
# netsrv pci: err=NoDevice stage=nodevice
# netsrv: no device
not ok 1 - netsrv: server ran: khong tim thay thiet bi mang
# ktest halt
EOF
    expect_accept "netsrv ca âm 'không thiết bị' đỏ đúng cách" validate_netsrv_negative "$netsrv"
    sed 's/^not ok 1 - netsrv: server ran.*$/ok 1 - netsrv: server ran/' "$netsrv" >"$tmp"
    expect_reject "netsrv ca âm PASS giả (có 'ok 1')" validate_netsrv_negative "$tmp"
    grep -v '^# netsrv: no device$' "$netsrv" >"$tmp"
    expect_reject "netsrv ca âm thiếu dòng '# netsrv: no device'" validate_netsrv_negative "$tmp"
    cat "$netsrv" >"$tmp"
    echo 'PANIC: KT-281 netsrv xong (2 TAP, 0 fail)' >>"$tmp"
    expect_reject "netsrv ca âm có dòng PANIC 'xong' (PASS giả)" validate_netsrv_negative "$tmp"
    cat "$netsrv" >"$tmp"
    echo '# netsrv: ping 10.0.2.2 ok rtt=0' >>"$tmp"
    expect_reject "netsrv ca âm có dòng '# netsrv: ping' (bằng chứng giả)" validate_netsrv_negative "$tmp"
    rm -f "$tmp" "$good" "$neg" "$nolink" "$netsrv"
}

# ---------------------------------------------------------------- main
MODE=run
CHECK_LOG=""
CHECK_KIND=positive
case "${1:-}" in
    --red-selftest) MODE=selftest ;;
    --check-log)
        MODE=check
        CHECK_LOG="${2:-}"
        CHECK_KIND="${3:-positive}"
        [ -n "$CHECK_LOG" ] || die "--check-log cần đường dẫn log"
        ;;
    "" | --run) MODE=run ;;
    -h | --help)
        sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) die "tham số không biết: $1" ;;
esac

if [ "$MODE" = selftest ]; then
    red_selftest
    echo "# net.sh --red-selftest: $CHECKS kiểm, $FAILS lỗi"
    [ "$FAILS" = 0 ] || exit 1
    exit 0
fi

if [ "$MODE" = check ]; then
    case "$CHECK_KIND" in
        positive) validate_positive "$CHECK_LOG" ;;
        negative) validate_negative "$CHECK_LOG" ;;
        nolink) validate_nolink "$CHECK_LOG" ;;
        netsrv-negative) validate_netsrv_negative "$CHECK_LOG" ;;
        *) die "--check-log: kiểu phải là positive|negative|nolink|netsrv-negative" ;;
    esac
    rc=$?
    echo "# net.sh --check-log $CHECK_KIND $CHECK_LOG: rc=$rc"
    exit "$rc"
fi

find_qemu
build_image
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

run_positive || true
run_negative || true
run_nolink || true
run_netsrv_negative || true
qg_infra "kt245-net" "$LOGDIR/kt245-infra.log" || true

echo "# net.sh: $CHECKS kiểm đạt, $FAILS lỗi (log: $LOGDIR)"
if [ "$FAILS" != 0 ]; then
    exit 1
fi
exit 0
