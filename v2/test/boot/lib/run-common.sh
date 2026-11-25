#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-407 — helper dùng chung cho **mọi** runner QEMU, hấp thu nợ `KT-348a`/`KT-348b`
# (D2-257 mục 1, D2-258 mục 3). Không chạy trực tiếp; `source` từ runner:
#
#   . "$HERE/lib/run-common.sh"
#
# Ba nợ được đồng bộ
# ------------------
# 1. **`rc=126`** — "harness không gửi được phím" (T-INFRA, **không** phải lỗi nhân), phân biệt
#    với `124` (treo, đã kill cả group) và `125` (không khởi động được QEMU/mở log). Bản cũ
#    `run-aarch64.sh` chờ `# input: ready` bằng `grep` rồi **mới** khởi động `python3` nối monitor
#    ⇒ dưới tải, `connect` rơi vào cửa sổ 5 s của ảnh và hết hạn (KT-348b: boot 861, 5076 ms).
#    `run_common_sendkey_start` khởi động `lib/monitor-sendkey.py` **ngay** sau `qg_start` — nó nối
#    HMP trước, poll log bằng `read(2)`, gửi trên kết nối đã mở sẵn.
# 2. **`run_common_log_inventory`** — đếm **từng slot** log boot: phải **có mặt** và **khác rỗng**,
#    nêu tên tệp + lý do lấy từ `results.txt` khi thiếu (KT-348: 7 boot 814..823 mất tệp vì
#    `ENOSPC` mà không ai đếm).
# 3. **`run_common_trap`** — **một** bộ trap `EXIT/INT/TERM/HUP` duy nhất. Bản cũ đặt trap ở 3 chỗ
#    ⇒ hai trap sau **ghi đè** trap trước, `stop_*_server`/`qg_cleanup` không bao giờ chạy trên
#    đường `die`/`exit 3` (KT-348b). Gọi lại chỉ **thay** hàm dọn, không chồng trap.
#
# Hợp đồng
# --------
#   run_common_trap <hàm dọn>            # cài đúng một bộ trap; hàm dọn phải tự gọi qg_cleanup
#   run_common_log_inventory <n> <dir>   # 0 = đủ n + khác rỗng; 1 = thiếu/rỗng (đã in lý do)
#   run_common_sendkey_start <sock> <log> <ready_timeout_ms> <kbd_log> <i> [python]
#   run_common_sendkey_wait <kbd_log> [pid]   # 0 = đã gửi; 126 = không gửi được (có lý do)
#
# Biến môi trường (tuỳ chọn):
#   RUN_COMMON_LOG_PREFIX      tiền tố tệp log (mặc định `boot-`)
#   RUN_COMMON_LOG_RESULTS     tệp `results.txt` (mặc định `<dir>/results.txt`)
#   RUN_COMMON_LOG_INFRA_STOP  tệp `INFRA-STOP` (mặc định `<dir>/INFRA-STOP`)
#   RUN_COMMON_LOG_TAG         nhãn trong dòng FAIL (mặc định `log`)
#   RUN_COMMON_SENDKEY_HELPER  đường dẫn helper (mặc định `lib/monitor-sendkey.py` cạnh tệp này)
#
# Tự kiểm: `bash v2/test/boot/lib/run-common.sh --selftest` (11 ca, không QEMU).
# Exit: 0 = xanh; 1 = có ca lệch; 2 = dùng sai.

# ---------------------------------------------------------------- (3) một trap duy nhất
RUN_COMMON_CLEANUP=""

# `run_common_trap <hàm dọn>` — cài **một** bộ trap. Gọi lại chỉ thay hàm dọn.
run_common_trap() {
    local fn="${1:-}"
    if [ -z "$fn" ]; then
        echo "run-common: run_common_trap cần tên hàm dọn" >&2
        return 2
    fi
    RUN_COMMON_CLEANUP="$fn"
    trap "$fn" EXIT
    trap "$fn; exit 130" INT
    trap "$fn; exit 143" TERM
    trap "$fn; exit 129" HUP
    return 0
}

# ---------------------------------------------------------------- (2) đếm log theo từng slot
# `run_common_log_inventory <n> <dir>` — in một dòng tổng kết; thiếu/rỗng ⇒ in tối đa 10 slot kèm
# lý do (lấy từ `results.txt` dòng `fail <i> <lý do>`) và trả 1.
run_common_log_inventory() {
    local n="${1:-}" dir="${2:-}"
    local prefix="${RUN_COMMON_LOG_PREFIX:-boot-}"
    local results="${RUN_COMMON_LOG_RESULTS:-$dir/results.txt}"
    local infra_stop="${RUN_COMMON_LOG_INFRA_STOP:-$dir/INFRA-STOP}"
    local tag="${RUN_COMMON_LOG_TAG:-log}"
    local i=0 miss=0 empty=0 r="" printed=0 f=""
    case "$n" in '' | *[!0-9]*)
        echo "run-common: run_common_log_inventory cần <n> là số (nhận '$n')" >&2
        return 2
        ;;
    esac
    [ -n "$dir" ] || {
        echo "run-common: run_common_log_inventory cần <dir>" >&2
        return 2
    }
    while [ "$i" -lt "$n" ]; do
        i=$((i + 1))
        f="$dir/$prefix$i.log"
        if [ ! -f "$f" ]; then
            miss=$((miss + 1))
        elif [ ! -s "$f" ]; then
            empty=$((empty + 1))
        fi
    done
    if [ "$miss" -eq 0 ] && [ "$empty" -eq 0 ]; then
        echo "# log: có đủ $n/$n tệp $prefix<i>.log, tất cả khác rỗng (mỗi boot một tệp)"
        return 0
    fi
    echo "# $tag: FAIL — log boot: thiếu $miss tệp, rỗng $empty tệp (trên $n slot)"
    i=0
    while [ "$i" -lt "$n" ]; do
        i=$((i + 1))
        f="$dir/$prefix$i.log"
        if [ -f "$f" ] && [ -s "$f" ]; then continue; fi
        r="$(sed -n "s/^fail $i //p" "$results" 2>/dev/null | head -1)"
        if [ -z "$r" ] && [ -f "$infra_stop" ]; then
            r="chưa chạy — soak dừng sớm vì hạ tầng (fail-closed)"
        fi
        if [ -f "$f" ]; then
            [ -n "$r" ] || r="tệp rỗng — không có dòng 'fail $i' trong results.txt"
            printf '#   log rỗng %s/%s%s.log (lý do: %s)\n' "$dir" "$prefix" "$i" "$r"
        else
            [ -n "$r" ] || r="không rõ — harness không ghi được log (không có dòng 'fail $i')"
            printf '#   không có log %s/%s%s.log (lý do: %s)\n' "$dir" "$prefix" "$i" "$r"
        fi
        printed=$((printed + 1))
        if [ "$printed" -ge 10 ]; then
            echo "#   … (còn $((miss + empty - printed)) slot thiếu/rỗng nữa, không in hết)"
            break
        fi
    done
    return 1
}

# ---------------------------------------------------------------- (1) gửi phím qua monitor
RUN_COMMON_SENDKEY_PID=""
RUN_COMMON_KEYS_ERR=""

# `run_common_sendkey_start <sock> <log> <ready_timeout_ms> <kbd_log> <i> [python]`
# Khởi động helper ở **nền** (nối monitor ngay, poll log, gửi `sendkey a`). Đặt
# `RUN_COMMON_SENDKEY_PID`. Trả 0 = đã khởi động; 2 = thiếu helper/python.
run_common_sendkey_start() {
    local sock="${1:-}" log="${2:-}" timeout_ms="${3:-5000}" kbd_log="${4:-}" idx="${5:-1}"
    local py="${6:-${PYTHON:-python3}}"
    local helper="${RUN_COMMON_SENDKEY_HELPER:-}"
    if [ -z "$helper" ]; then
        helper="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/monitor-sendkey.py"
    fi
    RUN_COMMON_KEYS_ERR=""
    if [ ! -f "$helper" ]; then
        RUN_COMMON_KEYS_ERR="helper-missing"
        return 2
    fi
    if ! command -v "$py" >/dev/null 2>&1; then
        RUN_COMMON_KEYS_ERR="python-missing"
        return 2
    fi
    "$py" "$helper" "$sock" "$log" "$timeout_ms" "$kbd_log" "$idx" >/dev/null 2>&1 &
    RUN_COMMON_SENDKEY_PID=$!
    return 0
}

# `run_common_sendkey_wait <kbd_log> [pid]` — chờ helper xong.
# Trả 0 = đã gửi phím (`i=<i> ok …`); 126 = harness không gửi được, `RUN_COMMON_KEYS_ERR` có lý do.
run_common_sendkey_wait() {
    local kbd_log="${1:-}" pid="${2:-${RUN_COMMON_SENDKEY_PID:-}}"
    local krc=0
    RUN_COMMON_KEYS_ERR=""
    if [ -n "$pid" ]; then
        wait "$pid" 2>/dev/null
        krc=$?
    fi
    if [ "$krc" -eq 0 ] && grep -q '^i=[0-9][0-9]* ok ' "$kbd_log" 2>/dev/null; then
        return 0
    fi
    RUN_COMMON_KEYS_ERR="$(sed -n 's/.* reason=\([^ ]*\).*/\1/p' "$kbd_log" 2>/dev/null | head -1)"
    [ -n "$RUN_COMMON_KEYS_ERR" ] || RUN_COMMON_KEYS_ERR="no-result-rc$krc"
    return 126
}

# ---------------------------------------------------------------- tự kiểm (không QEMU)
run_common_selftest() {
    local cases=0 oks=0 tmp="" rc=0 out="" py=""
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/run-common-selftest.XXXXXX")" || {
        echo "run-common: không tạo được thư mục tạm" >&2
        return 2
    }
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT

    _rc_record() { # <tên> <0/1> <chi tiết>
        cases=$((cases + 1))
        if [ "$2" -eq 0 ]; then
            oks=$((oks + 1))
            echo "ok $cases - $1: $3"
        else
            echo "not ok $cases - $1: $3"
        fi
    }

    # (a) trap: cài một bộ, gọi lại chỉ thay hàm dọn.
    _rc_cleanup_a() { :; }
    _rc_cleanup_b() { :; }
    run_common_trap _rc_cleanup_a
    out="$(trap -p EXIT)"
    if printf '%s' "$out" | grep -q '_rc_cleanup_a' && [ "$RUN_COMMON_CLEANUP" = _rc_cleanup_a ]; then
        _rc_record "trap: cài một bộ EXIT gọi hàm dọn" 0 "$out"
    else
        _rc_record "trap: cài một bộ EXIT gọi hàm dọn" 1 "trap=[$out] cleanup=$RUN_COMMON_CLEANUP"
    fi
    run_common_trap _rc_cleanup_b
    out="$(trap -p EXIT)"
    if printf '%s' "$out" | grep -q '_rc_cleanup_b' && ! printf '%s' "$out" | grep -q '_rc_cleanup_a'; then
        _rc_record "trap: gọi lại THAY hàm dọn (không chồng trap)" 0 "$out"
    else
        _rc_record "trap: gọi lại THAY hàm dọn (không chồng trap)" 1 "trap=[$out]"
    fi
    # Trap thật sự chạy khi thoát (subshell).
    (
        _rc_cleanup_marker() { printf 'ran\n' >"$tmp/trap-ran"; }
        run_common_trap _rc_cleanup_marker
        exit 0
    )
    if [ -f "$tmp/trap-ran" ]; then
        _rc_record "trap: hàm dọn CHẠY khi thoát" 0 "marker=$(cat "$tmp/trap-ran")"
    else
        _rc_record "trap: hàm dọn CHẠY khi thoát" 1 "không thấy marker"
    fi
    # Ca trap ở trên đã **thay** trap EXIT của selftest ⇒ cài lại để dọn thư mục tạm.
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT

    # (b) inventory: đủ 3/3 ⇒ XANH.
    mkdir -p "$tmp/logs"
    local i=1
    while [ "$i" -le 3 ]; do
        printf '# boot %s\n' "$i" >"$tmp/logs/boot-$i.log"
        i=$((i + 1))
    done
    printf 'ok 1\nfail 2 start-log-open:_No_space_left_on_device\nok 3\n' >"$tmp/logs/results.txt"
    if run_common_log_inventory 3 "$tmp/logs" >"$tmp/inv-ok.out" 2>&1 &&
        grep -q 'có đủ 3/3 tệp boot-<i>.log' "$tmp/inv-ok.out"; then
        _rc_record "inventory: đủ 3/3 ⇒ XANH" 0 "$(head -1 "$tmp/inv-ok.out")"
    else
        _rc_record "inventory: đủ 3/3 ⇒ XANH" 1 "$(head -1 "$tmp/inv-ok.out")"
    fi
    # (c) thiếu 1 tệp ⇒ ĐỎ + tên tệp + lý do từ results.txt.
    rm -f "$tmp/logs/boot-2.log"
    if run_common_log_inventory 3 "$tmp/logs" >"$tmp/inv-miss.out" 2>&1; then
        _rc_record "inventory: thiếu 1 tệp ⇒ ĐỎ" 1 "vẫn XANH (mất bằng chứng im lặng)"
    elif grep -q 'thiếu 1 tệp, rỗng 0 tệp' "$tmp/inv-miss.out" &&
        grep -q 'boot-2.log (lý do: start-log-open:_No_space_left_on_device)' "$tmp/inv-miss.out"; then
        _rc_record "inventory: thiếu 1 tệp ⇒ ĐỎ + tên + lý do" 0 "$(grep 'không có log' "$tmp/inv-miss.out" | head -1)"
    else
        _rc_record "inventory: thiếu 1 tệp ⇒ ĐỎ + tên + lý do" 1 "$(head -2 "$tmp/inv-miss.out" | tr '\n' '|')"
    fi
    # (d) tệp RỖNG cũng là mất bằng chứng ⇒ ĐỎ + tên tệp.
    : >"$tmp/logs/boot-2.log"
    if run_common_log_inventory 3 "$tmp/logs" >"$tmp/inv-empty.out" 2>&1; then
        _rc_record "inventory: tệp RỖNG ⇒ ĐỎ" 1 "vẫn XANH"
    elif grep -q 'rỗng 1 tệp' "$tmp/inv-empty.out" &&
        grep -q 'log rỗng .*boot-2.log (lý do: ' "$tmp/inv-empty.out"; then
        _rc_record "inventory: tệp RỖNG ⇒ ĐỎ + tên + lý do" 0 "$(grep 'log rỗng' "$tmp/inv-empty.out" | head -1)"
    else
        _rc_record "inventory: tệp RỖNG ⇒ ĐỎ + tên + lý do" 1 "$(head -2 "$tmp/inv-empty.out" | tr '\n' '|')"
    fi
    # (e) n không phải số ⇒ dùng sai (rc 2), không im lặng.
    run_common_log_inventory abc "$tmp/logs" >/dev/null 2>&1
    rc=$?
    if [ "$rc" -eq 2 ]; then
        _rc_record "inventory: <n> không phải số ⇒ rc=2" 0 "rc=$rc"
    else
        _rc_record "inventory: <n> không phải số ⇒ rc=2" 1 "rc=$rc"
    fi

    # (f) sendkey: HMP giả + log có `# input: ready` ⇒ rc=0, `i=… ok`.
    py="$(command -v python3 2>/dev/null || true)"
    if [ -z "$py" ]; then
        _rc_record "sendkey: HMP giả ⇒ gửi được" 1 "thiếu python3"
    else
        printf '# input: ready\n' >"$tmp/guest.log"
        "$py" - "$tmp/mon.sock" <<'PY' >"$tmp/mon.out" 2>&1 &
import os, socket, sys, time
path = sys.argv[1]
try:
    os.unlink(path)
except OSError:
    pass
srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
srv.bind(path)
srv.listen(1)
srv.settimeout(10.0)
try:
    conn, _ = srv.accept()
except socket.timeout:
    sys.exit(1)
conn.settimeout(10.0)
conn.sendall(b"QEMU 11.1.3 monitor - type 'help' for more information\r\n(qemu) ")
buf = b""
try:
    while b"sendkey a" not in buf:
        chunk = conn.recv(4096)
        if not chunk:
            break
        buf += chunk
except socket.timeout:
    pass
conn.sendall(b"(qemu) ")
time.sleep(0.05)
conn.close()
srv.close()
PY
        local monpid=$!
        sleep 0.2
        run_common_sendkey_start "$tmp/mon.sock" "$tmp/guest.log" 5000 "$tmp/kbd.log" 7 "$py"
        rc=$?
        if [ "$rc" -eq 0 ]; then
            run_common_sendkey_wait "$tmp/kbd.log"
            rc=$?
        fi
        wait "$monpid" 2>/dev/null
        if [ "$rc" -eq 0 ] && grep -q '^i=7 ok ' "$tmp/kbd.log" 2>/dev/null; then
            _rc_record "sendkey: HMP giả ⇒ gửi được (rc=0)" 0 "$(cat "$tmp/kbd.log")"
        else
            _rc_record "sendkey: HMP giả ⇒ gửi được (rc=0)" 1 "rc=$rc kbd=[$(cat "$tmp/kbd.log" 2>/dev/null)] err=$RUN_COMMON_KEYS_ERR"
        fi
        # (g) không có monitor ⇒ rc=126 + reason=connect-…
        run_common_sendkey_start "$tmp/khong-co.sock" "$tmp/guest.log" 500 "$tmp/kbd-miss.log" 8 "$py"
        rc=$?
        if [ "$rc" -eq 0 ]; then
            run_common_sendkey_wait "$tmp/kbd-miss.log"
            rc=$?
        fi
        if [ "$rc" -eq 126 ] && printf '%s' "$RUN_COMMON_KEYS_ERR" | grep -q '^connect-'; then
            _rc_record "sendkey: không có monitor ⇒ rc=126 + reason" 0 "reason=$RUN_COMMON_KEYS_ERR"
        else
            _rc_record "sendkey: không có monitor ⇒ rc=126 + reason" 1 "rc=$rc reason=$RUN_COMMON_KEYS_ERR"
        fi
        # (h) helper thiếu ⇒ rc=2 + reason=helper-missing (không im lặng).
        RUN_COMMON_SENDKEY_HELPER="$tmp/khong-co-helper.py" \
            run_common_sendkey_start "$tmp/mon.sock" "$tmp/guest.log" 500 "$tmp/kbd-x.log" 9 "$py"
        rc=$?
        if [ "$rc" -eq 2 ] && [ "$RUN_COMMON_KEYS_ERR" = helper-missing ]; then
            _rc_record "sendkey: thiếu helper ⇒ rc=2 + helper-missing" 0 "rc=$rc"
        else
            _rc_record "sendkey: thiếu helper ⇒ rc=2 + helper-missing" 1 "rc=$rc reason=$RUN_COMMON_KEYS_ERR"
        fi
        # (i) kbd_log có `fail` ⇒ wait trả 126 + reason (không nuốt lỗi).
        printf 'i=10 fail reason=ready-timeout\n' >"$tmp/kbd-fail.log"
        run_common_sendkey_wait "$tmp/kbd-fail.log" ""
        rc=$?
        if [ "$rc" -eq 126 ] && [ "$RUN_COMMON_KEYS_ERR" = ready-timeout ]; then
            _rc_record "sendkey: kbd_log fail ⇒ rc=126 + reason" 0 "reason=$RUN_COMMON_KEYS_ERR"
        else
            _rc_record "sendkey: kbd_log fail ⇒ rc=126 + reason" 1 "rc=$rc reason=$RUN_COMMON_KEYS_ERR"
        fi
    fi

    echo "# run-common --selftest: $oks/$cases ca đúng như phải"
    [ "$oks" -eq "$cases" ] || return 1
    echo "# run-common --selftest: PASS"
    return 0
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selftest) run_common_selftest ;;
        -h | --help)
            sed -n '2,45p' "$0"
            exit 0
            ;;
        *)
            echo "run-common.sh: dùng: run-common.sh --selftest | --help (hoặc source từ runner)" >&2
            exit 2
            ;;
    esac
fi
