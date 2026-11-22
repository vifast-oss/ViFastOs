#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-248 mục 2b) — **môi trường chạy chung cho runner boot** (`v2/test/boot/*.sh`).
#
# # Vì sao có tệp này
#
# Trước KT-333d, mỗi runner tự viết lại **cùng một** biểu thức `LOGDIR` (6 bản) và đặt socket
# monitor ở `$TMPDIR/...` (3 bản `SOCK`). Hai hệ quả thật:
#   * hai lần chạy song song ghi đè log của nhau (đã sửa một phần ở KT-333c bằng `.$$`);
#   * `$TMPDIR` trên macOS là `/var/folders/…/T/` (rất dài) ⇒ `AF_UNIX` **giới hạn 104 byte**
#     ⇒ QEMU không mở được monitor (`Path must be less than 104 bytes`) ⇒ ca âm đỏ vì lý do SAI.
#
# Tệp này gom hai việc đó thành **một** chỗ. Runner `source` nó rồi gọi:
#
#   . "$HERE/lib/env.sh"
#   LOGDIR="$(v2_env_logdir k5-x86_64 KT257_LOGDIR)"      # KT_LOGDIR thắng mọi thứ (như cũ)
#   SOCKDIR="$(v2_env_sockdir kt337)" || die "..."         # thư mục NGẮN cho unix socket
#   SOCK="$(v2_env_sock "$SOCKDIR" mon.sock)" || die "..." # kiểm < 104 byte
#
# # Hợp đồng (giữ **nguyên** hành vi cũ)
#
#   v2_env_logdir <tên> [biến-riêng] [pid|nopid]
#       `KT_LOGDIR` (nếu đặt, không rỗng) → in đúng nó.
#       Ngược lại `<biến-riêng>` (vd. `KT257_LOGDIR`) nếu đặt → in đúng nó.
#       Ngược lại `${V2_RUN_TMP:-${TMPDIR:-/tmp}}/<tên>.<pid>` (`nopid` ⇒ không có `.<pid>`).
#       Đây **chính là** biểu thức cũ của 6 runner — chỉ dời vào một hàm.
#
#   v2_env_sockdir [nhãn]
#       Tạo và in thư mục tạm **ngắn** (ưu tiên `/tmp`; chỉ dùng `$TMPDIR` nếu đường dẫn đủ ngắn),
#       bảo đảm `$thư-mục/<tên-socket>` ≤ **103 byte** (giới hạn `sun_path` của macOS: 104 gồm NUL).
#       Không tạo được thư mục đủ ngắn ⇒ rc 1 + nói rõ `TMPDIR`.
#
#   v2_env_sock <thư-mục> <tên>
#       In đường dẫn socket đầy đủ; **từ chối** (rc 1) nếu > 103 byte — thà đỏ rõ còn hơn QEMU
#       chết mờ (`Path must be less than 104 bytes`).
#
# Tệp này **không** có trạng thái toàn cục, không `exit`, không `cd`; an toàn khi `source` dưới
# `set -u`. Chế độ chạy trực tiếp chỉ để chẩn đoán/tự kiểm: `--selfcheck`, `--print-logdir`,
# `--print-sockdir`.
set -u

# macOS: `sun_path` 104 byte **kể cả** NUL ⇒ chuỗi tối đa 103 ký tự. Linux cho 108 nhưng dùng
# ngưỡng chặt hơn (macOS) cho **cùng** một đường chạy — không có hành vi khác theo arch.
V2_ENV_SOCK_MAX="${V2_ENV_SOCK_MAX:-103}"
# Chỗ dành cho tên tệp socket dài nhất (`mon-999.sock` / `mon-neg.sock` = 12) + đệm.
V2_ENV_SOCK_NAME_RESERVE="${V2_ENV_SOCK_NAME_RESERVE:-16}"

v2_env_err() { echo "env.sh: $*" >&2; }

# Lặp một ký tự N lần — không dùng `seq` (macOS không có sẵn).
v2_env_repeat() { # <ký-tự> <số-lần>
    local s="" i=0 n="$2"
    while [ "$i" -lt "$n" ]; do
        s="$s$1"
        i=$((i + 1))
    done
    printf '%s' "$s"
}

# Xem hợp đồng ở đầu tệp. In đường dẫn; **không** tạo thư mục (giữ đúng hành vi cũ: runner tự
# `mkdir -p` khi cần — `soak.sh`/`persist.sh` đã làm vậy).
v2_env_logdir() { # <tên> [biến-riêng] [pid|nopid]
    local ten="${1:-}" rieng="${2:-}" kieu="${3:-pid}" v=""
    if [ -z "$ten" ]; then
        v2_env_err "v2_env_logdir: thiếu tên thư mục log"
        return 2
    fi
    if [ -n "${KT_LOGDIR:-}" ]; then
        printf '%s' "$KT_LOGDIR"
        return 0
    fi
    if [ -n "$rieng" ]; then
        eval "v=\${$rieng:-}"
        if [ -n "$v" ]; then
            printf '%s' "$v"
            return 0
        fi
    fi
    if [ "$kieu" = "nopid" ]; then
        printf '%s/%s' "${V2_RUN_TMP:-${TMPDIR:-/tmp}}" "$ten"
    else
        printf '%s/%s.%s' "${V2_RUN_TMP:-${TMPDIR:-/tmp}}" "$ten" "$$"
    fi
    return 0
}

# Thư mục tạm **ngắn** cho unix socket. Ưu tiên `/tmp` (luôn ngắn trên macOS/Linux); nếu `/tmp`
# không ghi được thì thử `$TMPDIR` và **chỉ nhận** khi đường dẫn còn chỗ cho tên socket.
v2_env_sockdir() { # [nhãn]
    local tag="${1:-v2}" base d
    case "$tag" in
        '' | *[!A-Za-z0-9_-]*) tag="v2" ;;
    esac
    for base in /tmp "${TMPDIR:-/tmp}"; do
        [ -d "$base" ] && [ -w "$base" ] || continue
        d="$(mktemp -d "$base/v2s-$tag.XXXXXX" 2>/dev/null)" || continue
        if [ "${#d}" -le $((V2_ENV_SOCK_MAX - V2_ENV_SOCK_NAME_RESERVE)) ]; then
            printf '%s' "$d"
            return 0
        fi
        rm -rf "$d" 2>/dev/null || true
    done
    v2_env_err "không tạo được thư mục socket đủ ngắn (< $V2_ENV_SOCK_MAX byte, chừa $V2_ENV_SOCK_NAME_RESERVE cho tên) — TMPDIR='${TMPDIR:-}'"
    return 1
}

# Đường dẫn socket đầy đủ, **từ chối** nếu vượt giới hạn `AF_UNIX` (fail-closed, không để QEMU
# chết mờ). Không tạo tệp.
# **KT-333e (2026-11-23)** — `v2_env_wait_port_free <cổng> [giây tối đa]`: chờ cổng TCP trên
# 127.0.0.1 **trống** rồi trả 0 (hết hạn ⇒ 1). Dùng cho các runner dựng **TCP server thật** trên
# cổng `45678`: cổng này là **hằng số hai phía** (khách trong kernel gọi `10.0.2.2:45678`) nên
# **không** đổi số được; 4 slot chạy đêm đụng nhau ⇒ trước đây ca dương đỏ oan
# (`không dựng được TCP server trên 127.0.0.1:45678`). Không nới bất kỳ ngưỡng bằng chứng nào —
# chỉ **chờ** tài nguyên dùng chung.
v2_env_wait_port_free() {
    local port="${1:?thiếu cổng}" max="${2:-180}" waited=0
    command -v python3 >/dev/null 2>&1 || return 0
    while [ "$waited" -lt "$((max * 10))" ]; do
        if python3 - "$port" >/dev/null 2>&1 <<'PY'
import socket, sys
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", int(sys.argv[1])))
s.close()
PY
        then
            return 0
        fi
        [ "$waited" -eq 0 ] && echo "# chờ cổng $port trống (slot khác đang chạy runner)…" >&2
        sleep 0.1
        waited=$((waited + 1))
    done
    echo "# v2_env_wait_port_free: cổng $port vẫn bận sau ${max}s" >&2
    return 1
}

v2_env_sock() { # <thư-mục> <tên>
    local dir="${1:-}" name="${2:-}" p
    if [ -z "$dir" ] || [ -z "$name" ]; then
        v2_env_err "v2_env_sock: cần <thư-mục> <tên>"
        return 2
    fi
    p="$dir/$name"
    if [ "${#p}" -gt "$V2_ENV_SOCK_MAX" ]; then
        v2_env_err "đường dẫn AF_UNIX dài ${#p} > $V2_ENV_SOCK_MAX byte: $p"
        return 1
    fi
    printf '%s' "$p"
}

# ---------------------------------------------------------------- tự kiểm / chẩn đoán
v2_env_selfcheck() {
    local ca=0 fails=0 out rc d path tmp long
    _e_ok() { ca=$((ca + 1)); echo "ok $ca - $1"; }
    _e_bad() { ca=$((ca + 1)); fails=$((fails + 1)); echo "not ok $ca - $1"; }
    _e_eq() { # <nhãn> <nhận> <mong>
        if [ "$2" = "$3" ]; then _e_ok "$1"; else _e_bad "$1 (nhận '$2', mong '$3')"; fi
    }
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/v2env-sc.XXXXXX")" || {
        v2_env_err "không tạo được thư mục tự kiểm"
        return 2
    }
    long="$tmp/$(v2_env_repeat x 120)"

    # (1) KT_LOGDIR thắng mọi thứ.
    out="$(KT_LOGDIR=/co-dinh/log KT257_LOGDIR=/rieng v2_env_logdir k5-x86_64 KT257_LOGDIR)"
    _e_eq "KT_LOGDIR thắng ⇒ đúng đường ép" "$out" "/co-dinh/log"
    # (2) biến riêng của runner khi không có KT_LOGDIR.
    out="$(KT257_LOGDIR=/rieng v2_env_logdir k5-x86_64 KT257_LOGDIR)"
    _e_eq "KT257_LOGDIR (biến riêng) ⇒ đúng đường ép" "$out" "/rieng"
    # (3) V2_RUN_TMP ⇒ <base>/<tên>.<pid>.
    out="$(unset KT_LOGDIR KT257_LOGDIR; V2_RUN_TMP=/run/tmp v2_env_logdir k5-x86_64 KT257_LOGDIR)"
    _e_eq "V2_RUN_TMP ⇒ <base>/<tên>.<pid>" "$out" "/run/tmp/k5-x86_64.$$"
    # (4) `nopid` cho runner dùng chung thư mục giữa hai pha (persist, KT-242).
    out="$(unset KT_LOGDIR; V2_RUN_TMP=/run/tmp v2_env_logdir kt75-persist "" nopid)"
    _e_eq "nopid ⇒ KHÔNG có .<pid> (persist giữ nguyên hành vi)" "$out" "/run/tmp/kt75-persist"
    # (5) **CA ÂM**: TMPDIR rất dài ⇒ vẫn tạo được thư mục socket NGẮN.
    d="$(TMPDIR="$long" v2_env_sockdir kt 2>/dev/null)"
    rc=$?
    if [ "$rc" -ne 0 ] || [ -z "$d" ]; then
        _e_bad "ca âm TMPDIR dài ${#long} byte: không tạo được SOCKDIR"
    else
        path="$(v2_env_sock "$d" mon-999.sock)"
        if [ "${#path}" -le 103 ]; then
            _e_ok "ca âm TMPDIR dài ${#long} byte ⇒ SOCKDIR ngắn (${#d} byte), socket ${#path} byte < 104"
        else
            _e_bad "ca âm TMPDIR dài: socket ${#path} byte vượt giới hạn"
        fi
        rm -rf "$d" 2>/dev/null || true
    fi
    # (6) bind **thật** một unix socket trên đường dẫn vừa tính (bằng chứng AF_UNIX, không chỉ đếm).
    d="$(TMPDIR="$long/khac" v2_env_sockdir ktb 2>/dev/null)"
    if [ -z "$d" ]; then
        _e_bad "ca âm: không tạo được SOCKDIR cho phép bind"
    else
        path="$(v2_env_sock "$d" mon.sock)"
        if command -v python3 >/dev/null 2>&1; then
            if python3 - "$path" >/dev/null 2>&1 <<'PY'
import socket, sys
s = socket.socket(socket.AF_UNIX)
s.bind(sys.argv[1])
s.close()
PY
            then
                _e_ok "ca âm: bind được AF_UNIX thật trên đường dẫn ngắn khi TMPDIR dài"
            else
                _e_bad "ca âm: bind AF_UNIX thất bại trên $path"
            fi
        else
            _e_bad "ca âm: thiếu python3 để bind AF_UNIX (không bỏ qua phép kiểm)"
        fi
        rm -rf "$d" 2>/dev/null || true
    fi
    # (7) đường dẫn quá dài ⇒ `v2_env_sock` TỪ CHỐI (fail-closed).
    d="$tmp/$(v2_env_repeat z 120)"
    out="$(v2_env_sock "$d" mon.sock 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q "AF_UNIX"; then
        _e_ok "v2_env_sock từ chối đường dẫn > 103 byte (rc=$rc)"
    else
        _e_bad "v2_env_sock KHÔNG từ chối đường dẫn quá dài (rc=$rc)"
    fi
    # (8) socket ngắn hợp lệ ⇒ in đúng, không đổi tên.
    _e_eq "v2_env_sock giữ nguyên <dir>/<tên>" "$(v2_env_sock /tmp mon.sock)" "/tmp/mon.sock"

    rm -rf "$tmp" 2>/dev/null || true
    echo
    if [ "$fails" -gt 0 ]; then
        echo "env.sh --selfcheck: FAIL ($fails/$ca)" >&2
        return 1
    fi
    echo "env.sh --selfcheck: PASS ($ca/$ca)"
    return 0
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selfcheck) v2_env_selfcheck ;;
        --print-logdir)
            shift
            v2_env_logdir "${1:-}" "${2:-}" "${3:-pid}"
            ;;
        --print-sockdir)
            shift
            v2_env_sockdir "${1:-v2}"
            ;;
        "" | -h | --help)
            echo "dùng: (source) v2/test/boot/lib/env.sh; trực tiếp: --selfcheck | --print-logdir <tên> [biến] [pid|nopid] | --print-sockdir [nhãn]"
            ;;
        *)
            v2_env_err "cờ lạ: $1"
            exit 2
            ;;
    esac
fi
