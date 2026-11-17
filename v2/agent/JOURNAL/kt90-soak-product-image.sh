#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-90 — SOAK 1.000 BOOT TRÊN ẢNH **PHÁT HÀNH** (product-shell + product-fs).
#
# Vì sao có script này (bằng chứng cho khoảng trống KT-88):
#   `v2/test/boot/soak.sh:172` build `vifast-kbin-riscv64` **KHÔNG** `--features`, nên "0/1000"
#   của KT-88 là **ảnh sản phẩm không feature** (188.680 B, sha256 9dfd2ca9…, in `# k3 halt`):
#   không shell, không FS. Ảnh **phát hành** là `v2/artifacts/291502d2….bin` (205.440 B,
#   `rev=cf96eb9b589d`, `product-shell` + `product-fs`) — chưa được soak 1.000 vòng.
#
# Script này **KHÔNG sửa harness**: nó đọc `soak.sh` để lấy **nguyên văn** bộ cờ QEMU (ghi lại
# trong log), rồi boot ảnh phát hành với đúng bộ cờ đó (`-machine virt,aia=none,aclint=on
# -cpu rv64 -m 128M -smp 1 -no-reboot`, KHÔNG `-icount` — D2-72 Q2 / D2-74 c).
#
# Mỗi vòng (fail-closed, **không retry** — D2-07):
#   1. chờ `shell>` (timeout 20 s như D2-72 Q10) ⇒ chứng minh U-mode shell đã lên;
#   2. gửi `help\n` qua UART (stdin FIFO của QEMU) ⇒ đòi `# shell: help cmds=<n>`
#      (chứng minh shell **thật sự chạy** mỗi vòng, không chỉ boot tới chỗ chờ);
#   3. gửi `exit\n` ⇒ đòi `# shell: exit code=0` + `# product halt:` + QEMU **tự thoát** rc=0;
#   4. kiểm danh tính ảnh phát hành: đúng 1 `# build: rev=cf96eb9b589d dirty=0`,
#      đúng 1 `# product shell: … const:input_mode=serial const:inject=0`,
#      ≥1 `# product fs: medium=…`, `rx_bytes>0` (byte thật đi qua UART), 0 `PANIC`;
#   5. giữ nguyên `TAP version 13` / `1..1` / `ok 1 - k3 boot` như soak.sh kiểm (ảnh phát hành
#      vẫn in chúng trước khi handoff).
#
# D2-116: mỗi QEMU chạy trong **process group riêng** (`perl setpgrp`); chỉ kill **group của
# chính mình**, không `pkill -f qemu`. Có watchdog dọn khi tiến trình cha chết (kể cả SIGKILL).
#
# Không build gì (đo ảnh có sẵn) ⇒ không chạm `v2/kernel/target/` (KT-89 có thể đang build).
#
# Dùng:
#   bash kt90-soak-product-image.sh --dry          # MỘT vòng khô, log đầy đủ + in chứng cứ
#   bash kt90-soak-product-image.sh --pilot 6      # pilot (KHÔNG phải phép đo báo cáo)
#   bash kt90-soak-product-image.sh 1000 3         # phép đo (N LANES; LANES ≤ 3 — AGENTS §9)
#
# Exit: 0 = 0 lỗi/N (đủ N); 1 = có vòng đỏ / thiếu vòng / ảnh đổi; 2 = lỗi cấu hình/preflight;
#       3 = TỪ CHỐI ĐO (ảnh sai sha256 / thiếu marker danh tính ảnh phát hành).
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
LOGROOT="$HERE/kt90-logs"
# KT-99: mặc định vẫn là ảnh của phép đo KT-88; `--image <path>` để soak ảnh khác (vd ảnh mới
# `dd9f7050…`). Khi cho ảnh, sha/bytes/dòng build **suy ra từ chính ảnh** — không đoán.
ART="$REPO/v2/artifacts/291502d25c50b4e74476f08663bbe915d6b96f3d9961c66d9fb3f80d5a250353.bin"
WANT_SHA="291502d25c50b4e74476f08663bbe915d6b96f3d9961c66d9fb3f80d5a250353"
WANT_BYTES=205440
WANT_REV="cf96eb9b589d"
WANT_BUILD_LINE="# build: rev=${WANT_REV} dirty=0"
GIVEN_IMAGE=""
for _arg in "$@"; do
    case "$_arg" in
        --image=*) GIVEN_IMAGE="${_arg#--image=}" ;;
    esac
done
if [ -n "$GIVEN_IMAGE" ]; then
    [ -f "$GIVEN_IMAGE" ] || { echo "kt90: không thấy ảnh $GIVEN_IMAGE" >&2; exit 2; }
    ART="$GIVEN_IMAGE"
    WANT_SHA="$(shasum -a 256 "$ART" | awk '{print $1}')"
    WANT_BYTES="$(wc -c <"$ART" | tr -d ' ')"
    WANT_BUILD_LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: inputs=[0-9a-f]* dirty=[01]' -e '# build: rev=[0-9a-f]* dirty=[01]' "$ART" 2>/dev/null || true)"
    WANT_REV="$(printf '%s' "$WANT_BUILD_LINE" | sed -n 's/.*rev=\([0-9a-f]*\).*/\1/p')"
    [ -n "$WANT_BUILD_LINE" ] || { echo "kt90: ảnh không có dòng '# build:' — không đo" >&2; exit 3; }
fi
SOAK_SH="$REPO/v2/test/boot/soak.sh"

# Ngưỡng thời gian (số vòng poll × bước poll), KHÔNG nới trong lúc đo.
T_SHELL_POLLS=667 # 667 × 0.03 s ≈ 20 s/boot tới `shell>` (D2-72 Q10)
P_SHELL=0.03
T_HELP_POLLS=100 # 100 × 0.05 s = 5 s chờ phản hồi `help`
T_HALT_POLLS=100 # 100 × 0.05 s = 5 s chờ `# product halt:`
T_GONE_POLLS=80  # 80 × 0.05 s = 4 s chờ QEMU tự thoát sau halt
P_SLOW=0.05

N=1000
LANES=3
MODE=soak
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

usage() {
    sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

die() {
    echo "kt90: $*" >&2
    exit 2
}
refuse() {
    echo "kt90: TỪ CHỐI ĐO — $*" >&2
    exit 3
}

case "${1:-}" in
    --help | -h)
        usage
        exit 0
        ;;
    --dry)
        MODE=dry
        N=1
        LANES=1
        ;;
    --pilot)
        MODE=pilot
        N="${2:-6}"
        LANES="${3:-3}"
        ;;
    *)
        N="${1:-1000}"
        LANES="${2:-3}"
        ;;
esac
case "$N" in '' | *[!0-9]*) die "N phải là số (nhận '$N')" ;; esac
case "$LANES" in '' | *[!0-9]*) die "LANES phải là số (nhận '$LANES')" ;; esac
[ "$N" -gt 0 ] || die "N phải > 0"
[ "$LANES" -ge 1 ] && [ "$LANES" -le 3 ] || die "LANES=$LANES ngoài [1,3] (AGENTS §9: tối đa 3 QEMU)"

# ------------------------------------------------------------------ công cụ
PERL="$(command -v perl 2>/dev/null || true)"
[ -n "$PERL" ] || die "không có perl (cần cho process group riêng — D2-116)"
QEMU="$(command -v qemu-system-riscv64 2>/dev/null || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"
QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
case "$QEMU_VERSION" in *.*) ;; *) die "QEMU version '$QEMU_VERSION' lạ" ;; esac

# QEMU >= 9.2 (sàn D2-72 Q2) — kiểm bằng số nguyên, không tự hạ.
QM="${QEMU_VERSION%%.*}"
QR="${QEMU_VERSION#*.}"
Qm="${QR%%.*}"
case "$QM" in '' | *[!0-9]*) die "QEMU major lạ '$QEMU_VERSION'" ;; esac
case "$Qm" in '' | *[!0-9]*) die "QEMU minor lạ '$QEMU_VERSION'" ;; esac
{ [ "$QM" -gt 9 ] || { [ "$QM" -eq 9 ] && [ "$Qm" -ge 2 ]; }; } ||
    die "QEMU $QEMU_VERSION < sàn 9.2 (D2-72 Q2)"

SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && {
            SBI="$c"
            break
        }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

sha256_of() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        sha256sum "$1" | awk '{print $1}'
    fi
}
NOW() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }

qemu_count() { # in "<total> <ppid1>" — mọi QEMU trên máy (đo, không phải hằng)
    ps -ax -o ppid=,comm= 2>/dev/null |
        awk '$0 ~ /qemu-system/ { n++; if ($1 == 1) z++ } END { print n + 0, z + 0 }'
}

# ------------------------------------------------------------------ preflight
mkdir -p "$LOGROOT" || die "không tạo được $LOGROOT"
# N1 (reviewer KT-90): KHÔNG ghi đè preflight của phép đo trước — mỗi lần chạy một tệp riêng,
# và giữ nguyên `kt90-preflight.log` đã commit làm bằng chứng của phép đo 1.000.
PRE="$LOGROOT/kt90-preflight-$(date -u +%Y%m%dT%H%M%SZ)-$$.log"
: >"$PRE"
say() {
    printf '%s\n' "$*"
    printf '%s\n' "$*" >>"$PRE"
}

say "# KT-90 preflight — $(NOW)"
say "# script=$HERE/kt90-soak-product-image.sh mode=$MODE N=$N lanes=$LANES"
say "# qemu=$QEMU_VERSION ($QEMU) opensbi=$SBI"
say "# git HEAD=$(command git -C "$REPO" rev-parse HEAD 2>/dev/null || echo unknown)"

[ -f "$ART" ] || refuse "không thấy ảnh phát hành $ART"
GOT_SHA="$(sha256_of "$ART")"
GOT_BYTES="$(wc -c <"$ART" | tr -d ' ')"
say "# ảnh phát hành: path=$ART"
say "# ảnh phát hành: sha256=$GOT_SHA bytes=$GOT_BYTES"
[ "$GOT_SHA" = "$WANT_SHA" ] || refuse "sha256 ảnh '$GOT_SHA' ≠ '$WANT_SHA' — KHÔNG dùng ảnh khác"
[ "$GOT_BYTES" = "$WANT_BYTES" ] || refuse "bytes ảnh '$GOT_BYTES' ≠ '$WANT_BYTES'"
say "# sha256+bytes khớp ảnh phát hành (KT-88 §0: 291502d2…0353, 205.440 B)"

# Marker danh tính **trong ảnh**: phải có `# build: rev=… dirty=0` và chuỗi product shell/fs.
HAS_BUILD="$(LC_ALL=C grep -a -c -x -F "$WANT_BUILD_LINE" "$ART" 2>/dev/null || true)"
HAS_PSHELL="$(LC_ALL=C grep -a -c "product shell" "$ART" 2>/dev/null || true)"
HAS_PFS="$(LC_ALL=C grep -a -c "# product fs: medium=" "$ART" 2>/dev/null || true)"
say "# grep -a trong ảnh: build-line=$HAS_BUILD product-shell-strings=$HAS_PSHELL product-fs-fmt=$HAS_PFS"
[ "${HAS_BUILD:-0}" -ge 1 ] || refuse "ảnh KHÔNG chứa '$WANT_BUILD_LINE'"
[ "${HAS_PSHELL:-0}" -ge 1 ] || refuse "ảnh KHÔNG chứa chuỗi 'product shell' (thiếu feature)"
[ "${HAS_PFS:-0}" -ge 1 ] || refuse "ảnh KHÔNG chứa '# product fs: medium=' (thiếu feature)"

# Bộ cờ QEMU: lấy **nguyên văn** từ soak.sh (đọc, không sửa) và đối chiếu ổn định.
[ -f "$SOAK_SH" ] || die "không thấy $SOAK_SH"
SOAK_ARGS_LINE="$(grep -n -A1 '^QEMU_ARGS=(' "$SOAK_SH" | tr '\n' ' ')"
say "# soak.sh nguồn cờ: $SOAK_ARGS_LINE"
case "$SOAK_ARGS_LINE" in
    *"-machine virt,aia=none,aclint=on"*"-bios"*"-kernel"*"-nographic"*"-cpu rv64 -m 128M -smp 1 -no-reboot"*) ;;
    *) refuse "cờ QEMU trong soak.sh khác kỳ vọng — không tự đoán" ;;
esac
# Chỉ kiểm PHẦN CỜ, không kiểm chú thích (dòng nguồn có ghi "KHÔNG -icount" trong comment).
SOAK_ARGS_ONLY="${SOAK_ARGS_LINE%%#*}"
case "$SOAK_ARGS_ONLY" in *icount*) refuse "soak.sh có icount (D2-74 c cấm) — dừng" ;; esac

# Bộ cờ dùng để boot: **bản cứng** trong script này (KHÔNG suy ra từ soak.sh — N2 của reviewer).
# soak.sh được **đối chiếu** ở khối `case` phía trên; nếu hai bên lệch, script TỪ CHỐI ĐO.
QARGS=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$ART" -nographic \
    -cpu rv64 -m 128M -smp 1 -no-reboot)
QARGS_TXT="-machine virt,aia=none,aclint=on -bios $SBI -kernel $ART -nographic -cpu rv64 -m 128M -smp 1 -no-reboot"
say "# QEMU_ARGS (bản cứng của script; soak.sh chỉ đối chiếu): $QARGS_TXT"
# N2b: đối chiếu **từng cờ** với phần cờ của soak.sh — lệch ⇒ TỪ CHỐI ĐO (không tự đoán).
for _tok in -machine -bios -kernel -nographic -cpu -m -smp -no-reboot; do
    case "$SOAK_ARGS_ONLY" in
        *"$_tok"*) ;;
        *) refuse "soak.sh thiếu cờ '$_tok' mà script dùng — không tự đoán" ;;
    esac
done
case "$SOAK_ARGS_ONLY" in
    *"virt,aia=none,aclint=on"*"rv64"*"128M"*"smp 1"*) ;;
    *) refuse "giá trị cờ của soak.sh khác bản cứng — không tự đoán" ;;
esac

read -r Q_BEFORE Q1_BEFORE <<EOF
$(qemu_count)
EOF
say "# infra before: qemu_con_sot_total=$Q_BEFORE qemu_ppid1=$Q1_BEFORE loadavg=$(sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' | awk '{print $1,$2,$3}') ncpu=$(sysctl -n hw.ncpu 2>/dev/null)"
if [ "${Q_BEFORE:-0}" -gt 0 ]; then
    echo "kt90: CẢNH BÁO có QEMU lạ trước phép đo (total=$Q_BEFORE) — số wall có thể nhiễu" >&2
fi
[ "$MODE" = dry ] || say "# (wall của soak KHÔNG gồm preflight; không build — đo ảnh có sẵn)"

# ------------------------------------------------------------------ thư mục run
TS="$(date +%Y%m%d-%H%M%S)"
RUN="$LOGROOT/run-$MODE-$TS-$$"
mkdir -p "$RUN/pids" || die "không tạo được $RUN"
RESULT_MERGED="$RUN/results.txt"
: >"$RESULT_MERGED"
say "# run dir: $RUN"
say "# $MODE bắt đầu $(NOW)"

# Tệp cấu hình của run (đọc lại được sau khi kết thúc).
{
    echo "# KT-90 $MODE — $(NOW)"
    echo "# N=$N lanes=$LANES"
    echo "# image=$ART sha256=$WANT_SHA bytes=$WANT_BYTES rev=$WANT_REV"
    echo "# QEMU_ARGS=$QARGS_TXT"
    echo "# qemu=$QEMU_VERSION opensbi=$SBI"
    echo "# timeouts: shell<=20s help<=5s halt<=5s gone<=4s (poll shell 30ms, khác 50ms)"
    echo "# git HEAD=$(command git -C "$REPO" rev-parse HEAD 2>/dev/null || echo unknown)"
    if [ -n "$WANT_REV" ]; then
        echo "# git diff $WANT_REV..HEAD -- v2/kernel v2/lib:"
        command git -C "$REPO" diff --stat "$WANT_REV"..HEAD -- v2/kernel v2/lib 2>/dev/null | sed 's/^/#   /'
    else
        echo "# git diff: ảnh dùng định dạng inputs= (không có rev) — bỏ qua so lịch sử"
    fi
    echo "# head-of-diff-end"
} >"$RUN/config.txt"

# ------------------------------------------------------------------ D2-116: group riêng + kill đúng PID mình
STOP="$RUN/stop"
kill_boot() { # $1 = pid (leader group). Chỉ kill nếu đúng là QEMU.
    local p="${1:-}" pg="" i=0
    [ -n "$p" ] || return 0
    case "$p" in *[!0-9]*) return 0 ;; esac
    kill -0 "$p" 2>/dev/null || return 0
    ps -o command= -p "$p" 2>/dev/null | grep -q 'qemu-system-riscv64' || return 0
    pg="$(ps -o pgid= -p "$p" 2>/dev/null | tr -d ' ')"
    [ -n "$pg" ] || pg="$p"
    kill -TERM -- "-$pg" 2>/dev/null || kill -TERM "$p" 2>/dev/null || true
    while [ "$i" -lt 20 ]; do
        kill -0 "$p" 2>/dev/null || return 0
        sleep 0.1
        i=$((i + 1))
    done
    kill -KILL -- "-$pg" 2>/dev/null || kill -KILL "$p" 2>/dev/null || true
    return 0
}

XD_PID=""
cleanup() {
    local f
    : >"$STOP" 2>/dev/null || true
    for f in "$RUN"/pids/lane-*; do
        [ -f "$f" ] && kill_boot "$(cat "$f" 2>/dev/null)"
    done
    [ -n "$XD_PID" ] && kill "$XD_PID" 2>/dev/null
    return 0
}
trap 'cleanup' EXIT INT TERM HUP

# Watchdog: cha chết (kể cả SIGKILL) ⇒ đọc registry pid của từng lane và kill group QEMU.
(
    me=$$
    while :; do
        st="$(ps -o stat= -p "$me" 2>/dev/null)" || break
        case "$st" in '' | *Z*) break ;; esac
        sleep 0.3
    done
    : >"$STOP" 2>/dev/null || true
    for f in "$RUN"/pids/lane-*; do
        [ -f "$f" ] && kill_boot "$(cat "$f" 2>/dev/null)"
    done
) </dev/null >/dev/null 2>&1 &
XD_PID=$!
disown "$XD_PID" 2>/dev/null || true

# ------------------------------------------------------------------ chờ marker trong log
# Dùng biến động: LOG (đường dẫn log của boot hiện tại). Đặt POLLS = số vòng đã chờ.
wait_re() { # $1 ERE, $2 max polls, $3 poll secs, $4 pid
    local i=0
    while [ "$i" -lt "$2" ]; do
        if grep -qE "$1" "$LOG" 2>/dev/null; then
            POLLS=$i
            return 0
        fi
        kill -0 "$4" 2>/dev/null || break
        sleep "$3"
        i=$((i + 1))
    done
    POLLS=$i
    grep -qE "$1" "$LOG" 2>/dev/null
}

wait_gone() { # $1 max polls, $2 pid
    local i=0
    while [ "$i" -lt "$1" ]; do
        kill -0 "$2" 2>/dev/null || {
            GONE_POLLS=$i
            return 0
        }
        sleep "$P_SLOW"
        i=$((i + 1))
    done
    GONE_POLLS=$i
    return 1
}

# ------------------------------------------------------------------ MỘT vòng
# boot_one <i> <lane>: 0 = vòng xanh (đủ bằng chứng), 1 = đỏ. Ghi 1 dòng kết quả.
boot_one() {
    local i="$1" lane="$2"
    local log="$RUN/boot-$i.log" fifo="$RUN/fifo-$i" binfo="$RUN/pginfo-$i"
    local res="$RUN/results-lane-$lane.txt" evid="$RUN/evidence-lane-$lane.txt"
    local pid="" pgid="" rc=0 why="" help="" fsmed="" rx="" killed=0
    local shell_polls=0 help_polls=0 halt_polls=0 gone_polls=0
    local c_build c_pshell c_pfs c_panic c_tap c_plan c_ok1 c_halt c_exit0

    LOG="$log"
    : >"$log" 2>/dev/null || true
    rm -f "$fifo" "$binfo" 2>/dev/null || true
    mkfifo "$fifo" 2>/dev/null || {
        printf 'fail %s infra no-fifo\n' "$i" >>"$res"
        echo "boot $i/$N FAIL: không tạo được FIFO" >&2
        return 1
    }
    # RDWR: `open` của QEMU không chặn; byte bơm vào được giữ trong pipe tới khi QEMU đọc.
    exec 8<>"$fifo" || {
        printf 'fail %s infra no-fifo-open\n' "$i" >>"$res"
        return 1
    }

    # D2-116: setpgrp **trước** exec ⇒ $! = PID = PGID; ghi pid/pid thật ra binfo (đo).
    KT90_BINFO="$binfo" KT90_FIFO="$fifo" "$PERL" -e '
        setpgrp(0, 0) or die "kt90: setpgrp: $!";
        if (my $f = $ENV{KT90_BINFO}) {
            if (open(my $fh, ">", $f)) { printf $fh "%d %d\n", $$, getpgrp(0); close $fh; }
        }
        open(STDIN, "<", $ENV{KT90_FIFO}) or die "kt90: stdin fifo: $!";
        exec @ARGV or die "kt90: exec: $!";
    ' -- "$QEMU" "${QARGS[@]}" >"$log" 2>&1 &
    pid=$!
    echo "$pid" >"$RUN/pids/lane-$lane"
    [ -f "$binfo" ] && read -r pid_in binfo_pg <"$binfo" && pgid="$binfo_pg"
    [ -n "${pgid:-}" ] || pgid="$pid"

    # (1) chờ shell> — ảnh phát hành handoff sang U-mode rồi shell in prompt.
    if ! wait_re '^shell>$' "$T_SHELL_POLLS" "$P_SHELL" "$pid"; then
        shell_polls=$POLLS
        why="$why,timeout-shell"
    else
        shell_polls=$POLLS
    fi

    # (2) gửi `help` qua UART rồi chờ đúng phản hồi của shell (bằng chứng shell CHẠY).
    if [ -z "$why" ]; then
        printf 'help\n' >&8 2>/dev/null || true
        if wait_re '^# shell: help cmds=[0-9]+$' "$T_HELP_POLLS" "$P_SLOW" "$pid"; then
            help_polls=$POLLS
            help="$(sed -n 's/^# shell: help cmds=\([0-9]*\)$/\1/p' "$log" | head -1)"
        else
            help_polls=$POLLS
            why="$why,no-help-response"
        fi
    fi

    # (3) gửi `exit` ⇒ shell thoát 0, kernel in `# product halt:` rồi halt(0).
    if kill -0 "$pid" 2>/dev/null; then
        printf 'exit\n' >&8 2>/dev/null || true
        if wait_re '^# product halt:' "$T_HALT_POLLS" "$P_SLOW" "$pid"; then
            halt_polls=$POLLS
        else
            halt_polls=$POLLS
            why="$why,no-halt"
        fi
    fi

    # (4) QEMU phải **tự** thoát; quá hạn ⇒ kill group của chính mình (ghi rõ).
    if ! wait_gone "$T_GONE_POLLS" "$pid"; then
        killed=1
        kill_boot "$pid"
        wait "$pid" 2>/dev/null
        why="$why,killed-after-halt"
    else
        gone_polls=$GONE_POLLS
        wait "$pid" 2>/dev/null
    fi
    rc=$?
    exec 8>&- 2>/dev/null || true
    rm -f "$fifo" "$binfo" 2>/dev/null || true
    rm -f "$RUN/pids/lane-$lane" 2>/dev/null || true

    # ---------------------------------------------------------- kiểm bằng chứng (fail-closed)
    c_build="$(grep -c -x -F "$WANT_BUILD_LINE" "$log" 2>/dev/null || true)"
    c_pshell="$(grep -cE '^# product shell: .*const:input_mode=serial const:inject=0$' "$log" 2>/dev/null || true)"
    c_pfs="$(grep -cE '^# product fs: medium=[a-z-]+ ' "$log" 2>/dev/null || true)"
    c_panic="$(grep -cE '^[[:space:]]*PANIC' "$log" 2>/dev/null || true)"
    c_tap="$(grep -c -x -F 'TAP version 13' "$log" 2>/dev/null || true)"
    c_plan="$(grep -c -x -F '1..1' "$log" 2>/dev/null || true)"
    c_ok1="$(grep -c -x -F 'ok 1 - k3 boot' "$log" 2>/dev/null || true)"
    c_halt="$(grep -c -x -F '# k3 halt' "$log" 2>/dev/null || true)"
    c_exit0="$(grep -c -x -F '# shell: exit code=0' "$log" 2>/dev/null || true)"
    fsmed="$(sed -n 's/^# product fs: medium=\([a-z-]*\).*/\1/p' "$log" | head -1)"
    rx="$(sed -n 's/^# product halt:.* rx_bytes=\([0-9]*\) .*/\1/p' "$log" | head -1)"
    : "${rx:=0}"

    [ "${c_build:-0}" -eq 1 ] || why="$why,build-line=${c_build:-0}"
    [ "${c_pshell:-0}" -eq 1 ] || why="$why,product-shell=${c_pshell:-0}"
    [ "${c_pfs:-0}" -ge 1 ] || why="$why,product-fs=${c_pfs:-0}"
    [ "${c_panic:-0}" -eq 0 ] || why="$why,panic=${c_panic:-0}"
    [ "${c_tap:-0}" -eq 1 ] || why="$why,tap=${c_tap:-0}"
    [ "${c_plan:-0}" -eq 1 ] || why="$why,plan=${c_plan:-0}"
    [ "${c_ok1:-0}" -eq 1 ] || why="$why,ok1=${c_ok1:-0}"
    [ "${c_halt:-0}" -eq 0 ] || why="$why,halt-marker=${c_halt:-0}" # ảnh phát hành KHÔNG halt sau boot
    [ "${c_exit0:-0}" -eq 1 ] || why="$why,exit0=${c_exit0:-0}"
    [ "$rx" -gt 0 ] || why="$why,rx-bytes=$rx"
    [ "$rc" -eq 0 ] || why="$why,qemu-rc=$rc"

    if [ -n "$why" ]; then
        printf 'fail %s lane=%s why=%s shell_polls=%s help_polls=%s halt_polls=%s gone_polls=%s rx=%s qemu_rc=%s log=boot-%s.log\n' \
            "$i" "$lane" "${why#,}" "$shell_polls" "$help_polls" "$halt_polls" "$gone_polls" "$rx" "$rc" "$i" >>"$res"
        tail -8 "$log" | sed 's/^/    /' >&2
        echo "boot $i/$N FAIL:${why} lane=$lane" >&2
        return 1
    fi

    printf 'ok %s lane=%s help=%s fs=%s rx=%s qemu_rc=%s shell_polls=%s help_polls=%s halt_polls=%s gone_polls=%s killed=%s\n' \
        "$i" "$lane" "$help" "$fsmed" "$rx" "$rc" "$shell_polls" "$help_polls" "$halt_polls" "$gone_polls" "$killed" >>"$res"
    # Bản gọn 3 dòng chứng cứ danh tính ảnh + shell, để grep nhanh sau khi đo.
    {
        printf '# boot %s | %s\n' "$i" "$(grep -m1 -x -F "$WANT_BUILD_LINE" "$log")"
        printf '# boot %s | %s\n' "$i" "$(grep -m1 -E '^# product shell: ' "$log")"
        printf '# boot %s | %s\n' "$i" "$(grep -m1 -E '^# product fs: medium=' "$log")"
        printf '# boot %s | %s | help=%s | rx=%s\n' "$i" "$(grep -m1 -E '^# shell: help cmds=' "$log")" "$help" "$rx"
    } >>"$evid" 2>/dev/null || true
    echo "boot $i/$N ok lane=$lane help=$help rx=$rx"
    return 0
}

# ------------------------------------------------------------------ lane
lane_worker() {
    local lane="$1" i="$1"
    while [ "$i" -le "$N" ]; do
        [ -f "$STOP" ] && break
        boot_one "$i" "$lane" || true
        i=$((i + LANES))
    done
}

# ------------------------------------------------------------------ chạy
SECONDS=0
lane=1
while [ "$lane" -le "$LANES" ]; do
    lane_worker "$lane" &
    lane=$((lane + 1))
done
wait
WALL=$SECONDS
: >"$STOP" 2>/dev/null || true

# ------------------------------------------------------------------ tổng hợp
for f in "$RUN"/results-lane-*.txt; do
    [ -f "$f" ] && cat "$f" >>"$RESULT_MERGED"
done
for f in "$RUN"/evidence-lane-*.txt; do
    [ -f "$f" ] && cat "$f" >>"$RUN/evidence.txt"
done
rm -f "$RUN"/results-lane-*.txt "$RUN"/evidence-lane-*.txt 2>/dev/null || true

OK_COUNT="$(grep -c '^ok ' "$RESULT_MERGED" 2>/dev/null || true)"
FAIL_COUNT="$(grep -c '^fail ' "$RESULT_MERGED" 2>/dev/null || true)"
TOTAL=$((OK_COUNT + FAIL_COUNT))
TO_COUNT="$(grep -c 'why=.*timeout-shell' "$RESULT_MERGED" 2>/dev/null || true)"
PANIC_COUNT="$(grep -c 'why=.*panic=' "$RESULT_MERGED" 2>/dev/null || true)"
HELP_COUNT="$(grep -c '^ok .* help=[0-9][0-9]*' "$RESULT_MERGED" 2>/dev/null || true)"
RX_MIN="$(grep '^ok ' "$RESULT_MERGED" 2>/dev/null | sed -n 's/.* rx=\([0-9]*\) .*/\1/p' | sort -n | head -1)"
: "${RX_MIN:=0}"

GOT_SHA_END="$(sha256_of "$ART")"
IMG_CHANGED=0
[ "$GOT_SHA_END" = "$WANT_SHA" ] || IMG_CHANGED=1

# Chờ QEMU của mình thoát hết (tối đa 5 s) rồi đếm lại (D2-116).
w=0
while [ "$w" -lt 50 ]; do
    n_own=0
    for f in "$RUN"/pids/lane-*; do
        [ -f "$f" ] || continue
        p="$(cat "$f" 2>/dev/null)"
        [ -n "$p" ] && kill -0 "$p" 2>/dev/null && n_own=$((n_own + 1))
    done
    [ "$n_own" -eq 0 ] && break
    sleep 0.1
    w=$((w + 1))
done
read -r Q_AFTER Q1_AFTER <<EOF
$(qemu_count)
EOF

SUM="$RUN/summary.txt"
{
    echo "# KT-90 $MODE xong — $(NOW)"
    echo "# wall=${WALL}s tổng=$TOTAL/$N ok=$OK_COUNT fail=$FAIL_COUNT (${WALL}s)"
    [ "$WALL" -gt 0 ] && echo "# tốc độ=$((TOTAL / WALL)) boot/s (3 lane nếu LANES=3)"
    echo "# timeout-shell=$TO_COUNT panic=$PANIC_COUNT help-response=${HELP_COUNT}/$TOTAL rx_bytes_min=$RX_MIN"
    echo "# ảnh: sha256=$GOT_SHA_END (đầu $WANT_SHA) changed=$IMG_CHANGED bytes=$GOT_BYTES"
    echo "# infra after: qemu_con_sot_total=$Q_AFTER qemu_ppid1=$Q1_AFTER (trước: $Q_BEFORE/$Q1_BEFORE)"
    echo "# run=$RUN"
} | tee "$SUM"
[ "$FAIL_COUNT" -gt 0 ] && grep '^fail ' "$RESULT_MERGED" | sed 's/^/#   /'

# ------------------------------------------------------------------ chứng cứ mẫu
echo "# --- 3 dòng chứng cứ mẫu (evidence.txt) ---"
grep -m 12 '' "$RUN/evidence.txt" 2>/dev/null | sed 's/^/#   /'
echo "# --- ảnh: dòng shell>/help/halt của vòng 1 ---"
grep -nE '^shell>$|^# shell: help cmds=|^# shell: exit code=0|^# product halt:|^# k3 shell: handoff' "$RUN/boot-1.log" 2>/dev/null | sed 's/^/#   boot-1.log:/'

# ------------------------------------------------------------------ kết luận
if [ "$TOTAL" -ne "$N" ]; then
    echo "# kt90: FAIL — chỉ $TOTAL/$N vòng (dừng giữa chừng?)"
    exit 1
fi
if [ "$IMG_CHANGED" -ne 0 ]; then
    echo "# kt90: FAIL — ảnh đổi giữa soak ($WANT_SHA → $GOT_SHA_END)"
    exit 1
fi
if [ "$FAIL_COUNT" -ne 0 ]; then
    echo "# kt90: FAIL ($FAIL_COUNT/$N vòng đỏ)"
    exit 1
fi
if [ "$Q_AFTER" -gt 0 ]; then
    echo "# kt90: CẢNH BÁO — còn $Q_AFTER QEMU trên máy sau soak (không phải của script này: own=0)"
fi
echo "# kt90: $OK_COUNT/$N vòng xanh — mỗi vòng có 'shell>' + phản hồi 'help' qua UART + halt sạch"
echo "# kt90: PASS"
exit 0
