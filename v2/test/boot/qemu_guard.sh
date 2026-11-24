#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-78 — OD-K6-6 / D2-116: **mọi** lần mở QEMU trong harness phải đi qua tệp này.
#
# Vì sao có tệp này (D2-116): ngày 2026-11-17 hai `qemu-system-riscv64` **PPID 1** chạy 2h58/2h55
# ở 100 % CPU mỗi cái, đốt hai core đúng lúc gate/matrix/soak/persist đang đo ⇒ mọi số **thời gian**
# trong khoảng đó không dùng được. Nguyên nhân: script cha chết (SIGINT/SIGKILL) thì `kill $pid`
# không bao giờ chạy, QEMU bị `launchd`/`init` nhận nuôi và sống tiếp.
#
# Hợp đồng (không đổi ngữ nghĩa cổng: số boot/seed/ngưỡng/expected.txt giữ nguyên):
#   `qg_init`                      bật trap + watchdog; gọi **một lần** ngay sau khi source.
#   `qg_boot <log> <tds> <bin> [args…]`
#                                  chạy QEMU trong **process group riêng**; trả:
#                                    0   = QEMU tự thoát (như `wait "$pid"` cũ)
#                                    124 = quá `<tds>` đơn vị 0,1 s ⇒ đã kill **cả group**
#                                    125 = không khởi động được / không tạo được group riêng
#                                    126 = QEMU bị **tín hiệu từ tiến trình NGOÀI harness** giết
#                                          (KT-348a): `QG_LAST_SIG` = số hiệu, `QG_LAST_KILLER` =
#                                          pid người gửi đọc từ log QEMU. **Không** phải boot
#                                          treo ⇒ phép đo **không hợp lệ** (D2-116), không được
#                                          tính là boot đỏ của kernel.
#   `qg_tmp <tệp>…`                đăng ký tệp tạm để trap `rm` khi thoát.
#   `qg_infra <tag> [tệp]`         in (và ghi thêm vào `[tệp]`) dòng hạ tầng:
#                                  `qemu_total= qemu_ppid1= qemu_own= … loadavg= ncpu= verdict=`.
#   `qg_own`                       số QEMU **của chính script này** còn sống (theo PGID đã đăng ký).
#   `qg_cleanup`                   kill mọi group còn sống + `rm` tệp tạm; trap gọi tự động.
#
# Cơ chế (không có `setsid`/`timeout` trên macOS, bash 3.2):
#   1. **Process group riêng**: `perl -e 'setpgrp(0,0); exec @ARGV'` ⇒ QEMU là **leader** của group
#      riêng, `PGID == PID == $!`; dọn bằng `kill -TERM -- -$pgid` rồi `-KILL` — không `pkill -f`
#      (AGENTS §9) và **không** đụng tiến trình khác: `qg_kill_group` chỉ giết group có QEMU
#      (`qg_group_has_qemu`) **và** chỉ group do chính script này đăng ký.
#   2. **Timeout cứng**: vòng `sleep 0.1` đếm tới `<tds>` (đúng cách cũ), nhưng khi hết hạn thì
#      kill **cả group** (QEMU + con nó sinh), không chỉ `kill $pid`.
#   3. **Cha chết (kể cả SIGKILL)**: watchdog là subshell **disown** bám PID của script; script
#      biến mất (hoặc thành zombie) ⇒ watchdog đọc registry PGID và kill từng group ⇒ **0 QEMU
#      còn sống**. Đây là đường duy nhất SIGKILL cha không kịp chạy trap.
#   4. **Trap EXIT/INT/TERM/HUP**: `qg_cleanup` kill mọi group đã đăng ký + `rm` tệp tạm.
#   5. **Hạ tầng**: `qg_infra` đếm QEMU toàn máy + số PPID 1 + `loadavg` + `ncpu`; nếu có QEMU lạ
#      (`qemu_foreign>0`) hoặc QEMU mồ côi (`qemu_ppid1>0`) hoặc load 1 phút > ncpu ⇒ in
#      `verdict=KHONG_HOP_LE` + `WARNING` + `# MEASUREMENT_INVALID` và tăng `QG_INFRA_BAD`.
#
# D2-102: mọi giá trị trong dòng evidence lấy từ **biến đo**, không có chuỗi hằng viết sẵn.
set -u

QG_INIT=0
QG_ME=""
QG_REG=""
QG_LOG=""
QG_GROUPS=""
QG_FILES=""
QG_NBOOT=0
QG_INFRA_BAD=0
QG_ISO_BAD=0
QG_CLEANED=0
QG_PERL=""
QG_LAST_PID=""
QG_LAST_PGID=""
QG_LAST_SECS=0
QG_LAST_RC=0
# **KT-348a** — phân loại **ai** kết thúc QEMU: 0 = QEMU tự thoát, >0 = tín hiệu; `QG_LAST_KILLER`
# = pid tiến trình **ngoài harness** đã gửi tín hiệu (đọc từ log QEMU, rỗng nếu không đọc được).
# Dùng để tách "boot treo" (124 — lỗi kernel/harness, boot ĐỎ) khỏi "bị giết từ ngoài" (126 —
# phép đo KHÔNG HỢP LỆ, D2-116); trước KT-348a cả hai đều trả 0/124 lẫn lộn nên một SIGTERM
# ngoài bị tính là boot đỏ của kernel (sự cố KT-348: 1/600 đỏ, QEMU bị `signal 15 from pid 73084`).
QG_LAST_SIG=0
QG_LAST_KILLER=""
QG_LAST_TDS=""
QG_LAST_LOG=""
QG_START_ERR="" # **KT-348b**: lý do `qg_start` trả 125 (có tên, không nuốt lỗi)
QG_BINFO=""
QG_FORCE_NO_SETPGRP="${QG_FORCE_NO_SETPGRP:-}" # **chỉ** cho ca âm D2-108B (guard_selftest.sh)
QG_KILL_WAIT_DS=20 # 2 s chờ SIGTERM trước khi SIGKILL

# ---------------------------------------------------------------- tiện ích
qg_ncpu() {
    if command -v sysctl >/dev/null 2>&1; then
        sysctl -n hw.ncpu 2>/dev/null && return 0
    fi
    if command -v nproc >/dev/null 2>&1; then
        nproc 2>/dev/null && return 0
    fi
    getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1
}

# `l1 l5 l15` — một dòng, đúng thứ tự.
qg_loadavg() {
    if [ -r /proc/loadavg ]; then
        awk '{print $1, $2, $3}' /proc/loadavg
        return 0
    fi
    sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' |
        awk '{print $1, $2, $3}'
}

# Có ít nhất một `qemu-system*` trong group `$1`? (dùng trước MỌI `kill -- -PGID`)
qg_group_has_qemu() {
    [ -n "${1:-}" ] || return 1
    case "$1" in *[!0-9]*) return 1 ;; esac
    ps -ax -o pgid=,comm= 2>/dev/null |
        awk -v g="$1" '$1 == g && $0 ~ /qemu-system/ { f = 1 } END { exit(f ? 0 : 1) }'
}

# Kill **group** `$1`: TERM → chờ ≤ 2 s → KILL. Không đụng group không có QEMU.
qg_kill_group() {
    local pg="${1:-}" i=0
    [ -n "$pg" ] || return 0
    case "$pg" in *[!0-9]*) return 0 ;; esac
    qg_group_has_qemu "$pg" || return 0
    kill -TERM -- "-$pg" 2>/dev/null || true
    while [ "$i" -lt "$QG_KILL_WAIT_DS" ]; do
        qg_group_has_qemu "$pg" || return 0
        sleep 0.1
        i=$((i + 1))
    done
    kill -KILL -- "-$pg" 2>/dev/null || true
    return 0
}

qg_tmp() {
    local f
    for f in "$@"; do
        [ -n "$f" ] || continue
        QG_FILES="$QG_FILES$f
"
    done
}

# `qg_parse_killer <log>` — pid người gửi tín hiệu, đọc từ dòng QEMU in khi bị giết:
#   `qemu-system-x86_64: terminating on signal 15 from pid 73084 (<unknown process>)`
# In pid ra stdout; rỗng nếu log không có dòng đó (QEMU khác/bị SIGKILL ⇒ không in gì).
# Chỉ **đọc** log, không mở QEMU nào (D2-102: giá trị lấy từ đo, không phải hằng viết sẵn).
qg_parse_killer() {
    local f="${1:-}"
    [ -n "$f" ] && [ -f "$f" ] || return 0
    sed -n 's/^qemu-system[^:]*: terminating on signal [0-9][0-9]* from pid \([0-9][0-9]*\).*/\1/p' "$f" |
        tail -1
    return 0
}

# `qg_parse_sig <log>` — số hiệu tín hiệu QEMU khai trong log (rỗng nếu không có).
qg_parse_sig() {
    local f="${1:-}"
    [ -n "$f" ] && [ -f "$f" ] || return 0
    sed -n 's/^qemu-system[^:]*: terminating on signal \([0-9][0-9]*\) from pid .*/\1/p' "$f" | tail -1
    return 0
}

# ---------------------------------------------------------------- init + trap
qg_cleanup() {
    [ "$QG_CLEANED" = 1 ] && return 0
    QG_CLEANED=1
    local pg f
    if [ -n "$QG_GROUPS" ]; then
        printf '%s' "$QG_GROUPS" | while IFS= read -r pg; do
            [ -n "$pg" ] || continue
            qg_kill_group "$pg"
        done
    fi
    if [ -n "$QG_FILES" ]; then
        printf '%s' "$QG_FILES" | while IFS= read -r f; do
            [ -n "$f" ] || continue
            rm -f -- "$f" 2>/dev/null || true
        done
    fi
    # Registry là tệp **dùng chung** giữa các luồng của cùng script (soak 3 lane): chỉ thân script
    # chính được xoá; watchdog cũng xoá sau khi xử lý xong. Subshell (lane) KHÔNG xoá.
    if [ "${BASH_SUBSHELL:-0}" = 0 ] && [ -n "$QG_REG" ]; then
        rm -f -- "$QG_REG" 2>/dev/null || true
    fi
    return 0
}

qg_init() {
    [ "$QG_INIT" = 1 ] && return 0
    QG_ME=$$
    QG_DIR="${QG_DIR:-${TMPDIR:-/tmp}}"
    QG_LOG="${QG_LOG:-}"
    QG_REG="$QG_DIR/qg-$$.reg"
    QG_GROUPS=""
    QG_FILES=""
    QG_NBOOT=0
    QG_INFRA_BAD=0
    QG_ISO_BAD=0
    QG_CLEANED=0
    QG_PERL="$(command -v perl 2>/dev/null || true)"
    if [ -z "$QG_PERL" ]; then
        echo "qemu_guard: KHÔNG có perl — không tạo được process group riêng (OD-K6-6)" >&2
        return 2
    fi
    : >"$QG_REG" 2>/dev/null || {
        echo "qemu_guard: không ghi được registry $QG_REG" >&2
        return 2
    }
    if [ -n "$QG_LOG" ]; then
        mkdir -p "$(dirname "$QG_LOG")" 2>/dev/null || true
        printf '# qemu_guard: pid=%s date_utc=%s\n' "$QG_ME" "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
            >>"$QG_LOG" 2>/dev/null || true
    fi
    # Watchdog: bám PID của script. Script chết — kể cả SIGKILL (trap không chạy) — thì nó đọc
    # registry và kill từng group. `disown` để `wait` trần của script không chờ nó (soak.sh:277).
    (
        while :; do
            st="$(ps -o stat= -p "$QG_ME" 2>/dev/null)" || break
            case "$st" in *Z*) break ;; esac
            sleep 0.2
        done
        if [ -f "$QG_REG" ]; then
            while IFS= read -r pg; do
                [ -n "$pg" ] || continue
                qg_kill_group "$pg"
            done <"$QG_REG"
        fi
        rm -f -- "$QG_REG" 2>/dev/null
    ) </dev/null >/dev/null 2>&1 &
    QG_WD=$!
    disown "$QG_WD" 2>/dev/null || true
    trap 'qg_cleanup; exit 130' INT
    trap 'qg_cleanup; exit 143' TERM
    trap 'qg_cleanup; exit 129' HUP
    trap 'qg_cleanup' EXIT
    QG_INIT=1
    return 0
}

# ---------------------------------------------------------------- một boot
# `qg_start <log> <tds> <bin> [args…]` — **khởi động** QEMU trong process group riêng, KHÔNG chờ.
#
# Tách khỏi `qg_boot` ở KT-136: ca "cắt điện giữa lúc đang ghi" cần **giết** QEMU ở một thời điểm
# chọn trước, mà `qg_boot` luôn chờ tới khi tiến trình tự thoát (hoặc hết ngân sách). Hai hàm dùng
# **cùng** một đường khởi động ⇒ không có đường thứ hai lệch nhau về setpgrp/đo pgid.
#
# Đặt `QG_LAST_PID`, `QG_LAST_PGID`, `QG_LAST_TDS`, `QG_LAST_LOG`. Trả 0 = đã khởi động;
# 125 = tham số/môi trường hỏng (không có tiến trình nào được tạo).
qg_start() {
    local log="${1:-}" tds="${2:-}" bin="${3:-}" pid="" pg="" v=0
    shift 3 2>/dev/null || true
    QG_LAST_PID=""
    QG_LAST_PGID=""
    QG_LAST_SECS=0
    QG_LAST_RC=0
    QG_LAST_SIG=0
    QG_LAST_KILLER=""
    QG_LAST_TDS="$tds"
    QG_LAST_LOG="$log"
    case "$tds" in '' | *[!0-9]*) return 125 ;; esac
    [ -n "$log" ] || return 125
    [ -n "$bin" ] || return 125
    [ -x "$bin" ] || return 125
    [ "$QG_INIT" = 1 ] || return 125
    # **KT-348b** — mở/tỉa tệp log **trước** khi exec QEMU và **fail-closed có tên** nếu FS từ chối.
    # Lỗi đo được (KT-348, soak aarch64 vòng 16): đĩa đầy ⇒ redirect `>"$log"` của tiến trình con
    # thất bại (`qemu_guard.sh: line 225: …: No space left on device`), QEMU **không chạy**, nhưng
    # `qg_start` vẫn trả 0 (chỉ thấy `$!` của một shell con đã chết) ⇒ boot bị báo `# không có log`
    # mà **không** có lý do. Kiểm ngay tại đây để lỗi thành rc=125 + `QG_START_ERR` có errno.
    QG_START_ERR=""
    local _log_err=""
    if ! _log_err="$({ : >"$log"; } 2>&1)"; then
        QG_START_ERR="log-open: ${_log_err:-unknown}"
        echo "qemu_guard: KHÔNG mở được log '$log': $QG_START_ERR" >&2
        return 125
    fi
    # Process group riêng: perl đặt pgid = pid của chính nó rồi `exec` QEMU ⇒ QEMU **là leader**
    # của group riêng, `$!` = PID = PGID. macOS không có `setsid`, bash 3.2 không có `set -m` sạch.
    # Perl ghi `pid`/`pgid` **đo bằng getpgrp** ra tệp trước khi exec ⇒ boot thoát nhanh (0,1 s) vẫn
    # có bằng chứng, không phụ thuộc việc `ps` kịp nhìn thấy tiến trình.
    # Tên tệp **duy nhất theo tiến trình**: soak chạy 3 lane là 3 subshell cùng `$$` và cùng
    # `QG_NBOOT` ⇒ nếu đặt tên theo `$$-$QG_NBOOT` thì ba lane ghi đè lên nhau (đo được lỗi:
    # lane đọc pgid của lane khác ⇒ cảnh báo giả "KHÔNG ở group riêng"). `mktemp` bảo đảm duy nhất.
    QG_BINFO="$(mktemp "$QG_DIR/qg-binfo.XXXXXX" 2>/dev/null || true)"
    qg_tmp "$QG_BINFO"
    QG_BINFO="$QG_BINFO" QG_FORCE_NO_SETPGRP="$QG_FORCE_NO_SETPGRP" "$QG_PERL" -e '
        # Ca âm D2-108B: `QG_FORCE_NO_SETPGRP=1` bỏ setpgrp để chứng minh bộ đếm
        # isolated_0/isolation_bad **có thể** khác 0 (chỉ guard_selftest.sh dùng).
        unless ($ENV{QG_FORCE_NO_SETPGRP}) {
            setpgrp(0, 0) or die "qemu_guard: setpgrp: $!";
        }
        if (my $f = $ENV{QG_BINFO}) {
            if (open(my $fh, ">", $f)) {
                printf $fh "# qg_binfo pid=%d pgid=%d\n", $$, getpgrp(0);
                close $fh;
            }
        }
        exec @ARGV or die "qemu_guard: exec $ARGV[0]: $!";
    ' -- "$bin" "$@" >"$log" 2>&1 &
    pid=$!
    QG_LAST_PID="$pid"
    QG_GROUPS="$QG_GROUPS$pid
"
    printf '%s\n' "$pid" >>"$QG_REG" 2>/dev/null || true
    QG_NBOOT=$((QG_NBOOT + 1))
    # Đo PGID **thật**: chờ tới khi `pgid == pid` (tối đa 1 s). Không được đọc một lần ngay sau
    # `&` — lúc đó perl **chưa kịp** `setpgrp` (đo được: pgid = group của cha, báo động giả). Đây
    # là phép **đo**, không phải giả định; QEMU thoát quá nhanh ⇒ `isolated=unknown` (D2-102).
    while [ "$v" -lt 10 ]; do
        if [ -f "$QG_BINFO" ]; then
            pg="$(sed -n 's/^# qg_binfo pid=[0-9][0-9]* pgid=\([0-9][0-9]*\)$/\1/p' "$QG_BINFO" | head -1)"
        fi
        [ -n "$pg" ] && break
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
        v=$((v + 1))
    done
    [ -n "$pg" ] || pg="$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ')"
    if [ -n "$pg" ] && [ "$pg" = "$pid" ]; then
        QG_LAST_PGID="$pid"
    elif [ -n "$pg" ]; then
        QG_LAST_PGID="$pg"
        QG_ISO_BAD=$((QG_ISO_BAD + 1))
        echo "qemu_guard: CẢNH BÁO pgid=$pg != pid=$pid — QEMU KHÔNG ở group riêng (OD-K6-6)" >&2
    fi
    qg_guard_note boot_start
    QG_START_SPENT="$v" # số bước 0,1 s đã tiêu để đo PGID — `qg_boot` trừ vào ngân sách
    return 0
}

# `qg_boot <log> <tds> <bin> [args…]` — xem hợp đồng ở đầu tệp.
qg_boot() {
    local waited=0 t0=0 tmax=0
    qg_start "$@" || return $?
    local pid="$QG_LAST_PID" tds="$QG_LAST_TDS"
    waited="${QG_START_SPENT:-0}" # giữ đúng ngân sách timeout (phần đã tiêu để đo PGID)
    # Ngân sách = `tds` bước 0,1 s (đúng như vòng cũ). Thêm **trần tuyệt đối** theo `SECONDS`
    # (2× danh nghĩa + 5 s) để boot không thể treo vô hạn kể cả khi `sleep` bị kéo dài:
    # KHÔNG nới ngưỡng (chỉ siết), và boot bình thường (~1 s) không bao giờ chạm trần.
    t0=$SECONDS
    tmax=$(((tds / 10) * 2 + 5))
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$tds" ] &&
        [ $((SECONDS - t0)) -lt "$tmax" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    QG_LAST_SECS="$waited"
    if kill -0 "$pid" 2>/dev/null; then
        qg_kill_group "$pid"
        wait "$pid" 2>/dev/null
        QG_LAST_RC=124
        qg_guard_note boot_end
        rm -f -- "$QG_BINFO" 2>/dev/null || true
        return 124
    fi
    local st=0
    wait "$pid" 2>/dev/null
    st=$?
    # **KT-348a** — `wait` trả 128+N khi QEMU bị **tín hiệu** kết thúc. Nhánh này chỉ chạy khi QEMU
    # đã chết **trước** ngân sách `tds` ⇒ tín hiệu **không phải** của guard (guard gửi TERM/KILL
    # trong nhánh 124 ở trên) ⇒ người gửi là tiến trình **ngoài** phép đo (agent khác, `pkill`, OOM…).
    # Trước KT-348a nhánh này trả **0** ⇒ soak chấm boot đó là **boot đỏ của kernel** (sự cố KT-348:
    # 599/600, log dừng sau TAP 9, QEMU bị `signal 15 from pid 73084`). Không nới ngưỡng nào: chỉ
    # **phân loại đúng nguyên nhân** (D2-116 — phép đo bị nhiễu ⇒ KHÔNG HỢP LỆ, không phải boot đỏ).
    if [ "$st" -ge 128 ] && [ "$st" -le 191 ]; then
        QG_LAST_SIG=$((st - 128))
        QG_LAST_KILLER="$(qg_parse_killer "$QG_LAST_LOG")"
        # Số hiệu trong log QEMU là nguồn chính; `wait` là dự phòng (SIGKILL thì QEMU không in gì).
        local lsig
        lsig="$(qg_parse_sig "$QG_LAST_LOG")"
        [ -n "$lsig" ] && QG_LAST_SIG="$lsig"
        QG_LAST_RC=126
        qg_guard_note boot_end
        rm -f -- "$QG_BINFO" 2>/dev/null || true
        return 126
    fi
    QG_LAST_RC=0
    qg_guard_note boot_end
    rm -f -- "$QG_BINFO" 2>/dev/null || true
    return 0
}

# `qg_boot_kill <log> <tds> <delay_ds> <bin> [args…]` — khởi động rồi **giết cả process group**
# sau `delay_ds` bước 0,1 s (KT-136: "cắt điện" giữa lúc kernel đang ghi xuống đĩa).
#
# Trả 137 = đã giết (đúng ca đang đo); 0 = QEMU tự thoát **trước** khi hết `delay_ds` (ca đo
# KHÔNG hợp lệ — không có gì bị cắt); 125 = tham số hỏng. Người gọi **phải** coi 0 là lỗi của
# phép đo, không phải thành công: không cắt được thì không chứng minh được gì.
qg_boot_kill() {
    local delay="${3:-}" i=0
    case "$delay" in '' | *[!0-9]*) return 125 ;; esac
    qg_start "$@" || return $?
    local pid="$QG_LAST_PID"
    while [ "$i" -lt "$delay" ] && kill -0 "$pid" 2>/dev/null; do
        sleep 0.1
        i=$((i + 1))
    done
    QG_LAST_SECS="$i"
    if kill -0 "$pid" 2>/dev/null; then
        qg_kill_group "$pid"
        wait "$pid" 2>/dev/null
        QG_LAST_RC=137
        qg_guard_note boot_end
        rm -f -- "$QG_BINFO" 2>/dev/null || true
        return 137
    fi
    wait "$pid" 2>/dev/null
    QG_LAST_RC=0
    qg_guard_note boot_end
    rm -f -- "$QG_BINFO" 2>/dev/null || true
    return 0
}

# Ghi **bằng chứng** process group ra tệp riêng (không chèn dòng lạ vào log QEMU: validator của
# cổng giữ nguyên ngữ nghĩa — yêu cầu 6 của KT-78).
qg_guard_note() {
    local kind="${1:-?}" iso=unknown
    [ -n "$QG_LOG" ] || return 0
    if [ -n "$QG_LAST_PID" ] && [ "$QG_LAST_PGID" = "$QG_LAST_PID" ]; then iso=1; fi
    if [ -n "$QG_LAST_PGID" ] && [ "$QG_LAST_PGID" != "$QG_LAST_PID" ]; then iso=0; fi
    case "$kind" in
        boot_start)
            printf '# boot_start seq=%s pid=%s pgid=%s isolated=%s timeout_ds=%s log=%s\n' \
                "$QG_NBOOT" "$QG_LAST_PID" "${QG_LAST_PGID:-unknown}" "$iso" \
                "${QG_LAST_TDS:-?}" "${QG_LAST_LOG:-}" >>"$QG_LOG" 2>/dev/null || true
            ;;
        *)
            # **KT-348a** — thêm `ext_sig=`/`killer=` (0/"" khi QEMU tự thoát hoặc guard tự kill).
            # Đây là **bằng chứng phân loại**: `rc=126 ext_sig=15 killer=73084` ⇒ phép đo bị nhiễu
            # từ ngoài (D2-116), **không** phải boot treo. `qg_guard_summary` chỉ đọc `isolated=`/`rc=`
            # nên định dạng cũ vẫn parse được (thêm trường ở cuối, không đổi trường cũ).
            printf '# boot_end seq=%s pid=%s pgid=%s isolated=%s rc=%s ext_sig=%s killer=%s secs=%s.%s log=%s\n' \
                "$QG_NBOOT" "$QG_LAST_PID" "${QG_LAST_PGID:-unknown}" "$iso" "$QG_LAST_RC" \
                "$QG_LAST_SIG" "${QG_LAST_KILLER:-none}" "$((QG_LAST_SECS / 10))" "$((QG_LAST_SECS % 10))" \
                "${QG_LAST_LOG:-}" \
                >>"$QG_LOG" 2>/dev/null || true
            ;;
    esac
    return 0
}

qg_own() {
    local n=0 pg
    [ "$QG_INIT" = 1 ] || {
        echo 0
        return 0
    }
    if [ -n "$QG_GROUPS" ]; then
        while IFS= read -r pg; do
            [ -n "$pg" ] || continue
            qg_group_has_qemu "$pg" && n=$((n + 1))
        done <<EOF
$QG_GROUPS
EOF
    fi
    echo "$n"
}

# ---------------------------------------------------------------- tổng hợp guard log
# `qg_guard_summary [tệp]` — tổng hợp bằng awk trên log guard (D2-106: nguồn + công thức in kèm).
# Dùng sau mỗi lần soak/persist/matrix để có dòng `guard_summary` **do script sinh**, thay vì tự đếm.
qg_guard_summary() {
    local f="${1:-$QG_LOG}"
    [ -n "$f" ] || return 1
    [ -f "$f" ] || return 1
    printf '# guard_summary nguon=%s\n' "${QG_SUMMARY_SRC:-$f}"
    printf '# cong_thuc: boot_end = dem dong bat dau bang "# boot_end"; isolated_X = so dong co isolated=X; rc_X = so dong co rc=X\n'
    printf '# sinh_lai: QG_SUMMARY_SRC=<nguon> bash v2/agent/JOURNAL/kt78-collect-guard.sh --src <nguon> --dst <dich>\n'
    awk '
        /^# boot_end/ {
            for (i = 1; i <= NF; i++) {
                if ($i ~ /^isolated=/) { k = $i; sub(/^isolated=/, "", k); iso[k]++ }
                if ($i ~ /^rc=/) { r = $i; sub(/^rc=/, "", r); rc[r]++ }
            }
            n++
        }
        END {
            printf "# guard_summary boot_end=%d isolated_1=%d isolated_0=%d isolated_unknown=%d rc_0=%d rc_124=%d\n", \
                n, iso["1"] + 0, iso["0"] + 0, iso["unknown"] + 0, rc["0"] + 0, rc["124"] + 0
        }
    ' "$f"
}

# ---------------------------------------------------------------- hạ tầng
# `qg_infra <tag> [tệp]` — in dòng `# infra <tag>: …` ra stdout và (nếu có) nối vào `[tệp]`.
# Trả 0 nếu phép đo HỢP LỆ, 1 nếu KHÔNG HỢP LỆ (có QEMU lạ/mồ côi hoặc load bất thường).
qg_infra() {
    local tag="${1:-?}" out="${2:-}" total="" ppid1="" own="" foreign="" l1="" l5="" l15=""
    local ncpu="" verdict=OK why="" line=""
    ncpu="$(qg_ncpu)"
    read -r total ppid1 <<EOF
$(ps -ax -o ppid=,comm= 2>/dev/null | awk '$0 ~ /qemu-system/ { n++; if ($1 == 1) z++ } END { print n + 0, z + 0 }')
EOF
    : "${total:=0}" "${ppid1:=0}"
    own="$(qg_own)"
    foreign=$((total - own))
    [ "$foreign" -lt 0 ] && foreign=0
    read -r l1 l5 l15 <<EOF
$(qg_loadavg)
EOF
    : "${l1:=0}" "${l5:=0}" "${l15:=0}"
    case "$ncpu" in '' | *[!0-9]*) ncpu=1 ;; esac
    if [ "$QG_ISO_BAD" -gt 0 ]; then
        verdict=KHONG_HOP_LE
        why="isolation_bad=$QG_ISO_BAD"
    elif [ "$ppid1" -gt 0 ]; then
        verdict=KHONG_HOP_LE
        why="qemu_ppid1=$ppid1"
    elif [ "$foreign" -gt 0 ]; then
        verdict=KHONG_HOP_LE
        why="qemu_foreign=$foreign"
    elif awk -v l="$l1" -v n="$ncpu" 'BEGIN { exit(l > n ? 0 : 1) }'; then
        verdict=KHONG_HOP_LE
        why="loadavg_1m=$l1>ncpu=$ncpu"
    fi
    [ "$verdict" = OK ] || QG_INFRA_BAD=$((QG_INFRA_BAD + 1))
    line="# infra $tag: qemu_total=$total qemu_ppid1=$ppid1 qemu_own=$own qemu_foreign=$foreign loadavg_1m=$l1 loadavg_5m=$l5 loadavg_15m=$l15 ncpu=$ncpu boot_done=$QG_NBOOT verdict=$verdict"
    [ -n "$why" ] && line="$line ly_do=$why"
    printf '%s\n' "$line"
    if [ -n "$out" ]; then
        printf '%s\n' "$line" >>"$out" 2>/dev/null || true
    fi
    if [ -n "$QG_LOG" ]; then
        printf '%s\n' "$line" >>"$QG_LOG" 2>/dev/null || true
    fi
    if [ "$verdict" != OK ]; then
        printf 'WARNING: phép đo [%s] KHÔNG HỢP LỆ (%s) — số THỜI GIAN không dùng để kết luận (D2-116/OD-K6-6)\n' \
            "$tag" "$why" >&2
        printf '# MEASUREMENT_INVALID tag=%s ly_do=%s\n' "$tag" "$why"
        return 1
    fi
    return 0
}
