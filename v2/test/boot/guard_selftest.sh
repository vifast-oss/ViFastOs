#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-78 — ca âm bắt buộc cho harness QEMU (OD-K6-6, D2-116, D2-108B).
#
# Hai ca âm (a)(b) của KT-78, mỗi ca có **log thật**:
#   (a) giết **tiến trình cha** khi QEMU đang chạy ⇒ **0 QEMU còn sống sau 5 s**:
#         a1 SIGKILL cha (trap không kịp chạy) ⇒ watchdog phải dọn;
#         a2 SIGINT  cha (trap chạy)              ⇒ `qg_cleanup` phải dọn;
#         a3 SIGTERM cha (trap chạy)              ⇒ như trên.
#   (b) boot **treo** (QEMU ăn chương trình không bao giờ thoát: `j .` 4 byte) ⇒ timeout cứng phải
#       kích hoạt, **không** treo vô hạn, exit code **124** phân biệt được với boot sạch (0).
#
# **Đỏ trước** (D2-05): cùng driver, `--mode old` dùng **đúng pattern cũ** của
# `ktest.sh:1007-1027`/`soak.sh:204-217`/`persist.sh:153-170` (không process group, không trap,
# không watchdog) ⇒ SIGKILL/SIGINT cha **để lại** QEMU `PPID 1` — tái hiện D2-116. `--mode guard`
# chạy cùng kịch bản qua `qemu_guard.sh` ⇒ 0 QEMU.
#
# Vì sao cần ca âm thứ hai (D2-108B): bộ đếm `qemu_ppid1=0` chỉ có nghĩa nếu **có** một ca làm nó
# khác 0 — đó là `old-sigkill`/`old-sigint` trong log này (`qemu_orphan=1`).
#
# Dùng:
#   bash v2/test/boot/guard_selftest.sh [--mode old|guard|both] [--logdir DIR]
# Exit: 0 = mọi ca cho đúng kết quả kỳ vọng (kể cả ca đỏ-trước của `old`); 1 = có ca lệch; 2 = lỗi môi trường.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
GUARD="$HERE/qemu_guard.sh"
MODE=both
# **KT-333f (2026-11-23)** — logdir **ra NGOÀI repo** mặc định: bước này chạy trong T2 **mỗi lượt**
# và sinh `selftest-<stamp>.log` ⇒ nằm trong repo thì mỗi lượt T2 lại thêm 1 tệp `??` (đo được:
# 36 tệp chỉ trong 2 ngày), làm `git status` ồn và `pre-flight T2` cảnh báo. Đặt dưới
# `${TMPDIR:-/tmp}/v2-kt78-<pid>` **có dấu thời gian** (không ghi đè log cũ); vẫn ghi **đường dẫn
# tuyệt đối** ra stdout để bằng chứng truy được. `KT78_LOGDIR`/`--logdir` vẫn thắng nếu được đặt.
LOGDIR="${KT78_LOGDIR:-${TMPDIR:-/tmp}/v2-kt78-$$}"
TARGET="riscv64gc-unknown-none-elf"
# **KT-342a (D2-249)** — ca `clean-boot` cần một **boot tự kết thúc** (TAP đủ rồi `halt`). Ảnh
# **sản phẩm** từ `KT-342` có `shell` chờ **đầu vào** ⇒ treo tới hết `timeout` (`exit=124`, đo
# được 2026-11-23). Ảnh **ktest** tự in TAP `1..N` + `# k3 halt` rồi thoát ⇒ đúng thứ ca này cần
# đo (guard/PGID/exit 0), không phụ thuộc shell. Vẫn nhận `KT78_IMAGE` để đổi ảnh khi cần.
# **KT-407** — tôn trọng `CARGO_TARGET_DIR` (cùng quy ước cargo; xem `run-aarch64.sh`).
IMG_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release"
IMAGE="${KT78_IMAGE:-$IMG_DIR/vifast-kbin-ktest-base}"
[ -f "$IMAGE" ] || IMAGE="$IMG_DIR/vifast-kbin-riscv64"
# Timeout "5 s" của tiêu chí KT-78 — đo **thời gian thật** từ lúc gửi signal tới lúc QEMU mất.
LIMIT_MS=5000

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) MODE="${2:-}"; shift 2 ;;
        --logdir) LOGDIR="${2:-}"; shift 2 ;;
        -h | --help)
            sed -n '2,40p' "$0"
            exit 0
            ;;
        *) echo "guard_selftest: tham số lạ $1" >&2; exit 2 ;;
    esac
done
case "$MODE" in old | guard | both) ;; *) echo "guard_selftest: --mode lạ [$MODE]" >&2; exit 2 ;; esac
[ -f "$GUARD" ] || {
    echo "guard_selftest: không thấy $GUARD" >&2
    exit 2
}
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || {
    echo "guard_selftest: không có qemu-system-riscv64" >&2
    exit 2
}
command -v python3 >/dev/null 2>&1 || {
    echo "guard_selftest: cần python3 (đặt lại SIGINT về DEFAULT trước exec)" >&2
    exit 2
}
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
[ -f "${SBI:-/nonexistent}" ] || {
    echo "guard_selftest: không thấy OpenSBI fw_dynamic (đặt OPENSBI=…)" >&2
    exit 2
}

mkdir -p "$LOGDIR" || exit 2
STAMP="$(date -u '+%Y%m%dT%H%M%SZ')"
WORK="$LOGDIR/run-$STAMP"
mkdir -p "$WORK" || exit 2
TRANSCRIPT="$LOGDIR/selftest-$STAMP.log" # có dấu thời gian: chạy lại KHÔNG ghi đè log đã dùng làm bằng chứng
: >"$TRANSCRIPT" || exit 2
CASES=0
OKS=0
BASE_QEMU=0
LAST_QEMU_PID=""
LAUNCHED_PID=""

say() {
    printf '%s\n' "$*"
    printf '%s\n' "$*" >>"$TRANSCRIPT"
}

now_ms() {
    python3 -c 'import time; print(int(time.time()*1000))'
}

# Đếm QEMU **toàn máy** (không `pgrep -f` để không khớp chính lệnh).
qemu_total() {
    ps -ax -o comm= 2>/dev/null | grep -c 'qemu-system' || true
}
# **KT-342a (D2-249, 2026-11-23 08:05)** — đếm QEMU **có cửa sổ lắng** ngắn: sau khi wrapper `timeout`
# trả về, tiến trình QEMU con có thể còn vài chục ms trong bảng tiến trình (SIGTERM vừa gửi, chưa
# reap) ⇒ đếm ngay lập tức cho `left = baseline + 1` **không** có nghĩa "còn QEMU sống".
# Hàm này chờ tối đa `$2` ms (mặc định 1.000) cho tới khi số QEMU ≤ `$1`, trả về **số cuối cùng**.
# Bất biến vẫn nguyên: quá hạn mà vẫn còn ⇒ ca ĐỎ (không nới ngưỡng, chỉ bỏ **race** đo).
qemu_wait_le() { # $1=max, $2=timeout_ms
    local max="${1:-0}" limit="${2:-1000}" waited=0 n
    n="$(qemu_total)"
    while [ "$n" -gt "$max" ] && [ "$waited" -lt "$limit" ]; do
        sleep 0.025
        waited=$((waited + 25))
        n="$(qemu_total)"
    done
    printf '%s\n' "$n"
}

qemu_ppid1() {
    ps -ax -o ppid=,comm= 2>/dev/null | awk '$0 ~ /qemu-system/ && $1 == 1 { n++ } END { print n + 0 }'
}

# **T-INFRA (KT-402, D2-07)** — danh sách pid QEMU **toàn máy** để ca timeout so **tập trước/sau**
# thay vì so tổng với baseline: làn QEMU song song của cổng có thể xuất hiện/biến mất giữa hai lần
# đếm, làm tổng lệch dù guard chạy đúng (đã tái hiện 2 lượt T2 ngày 2026-11-25).
qemu_pids() {
    ps -ax -o pid=,comm= 2>/dev/null | awk '$0 ~ /qemu-system/ { print $1 }' | sort -n
}

# Kill **đúng PID do driver này ghi lại** (AGENTS §9: không `pkill -f`), chỉ dùng để dọn ca `old`
# vốn cố tình để lại QEMU mồ côi.
force_kill_pid() {
    local p="${1:-}" i=0
    [ -n "$p" ] || return 0
    case "$p" in *[!0-9]*) return 0 ;; esac
    kill -0 "$p" 2>/dev/null || return 0
    kill -TERM "$p" 2>/dev/null || true
    while [ "$i" -lt 20 ]; do
        kill -0 "$p" 2>/dev/null || return 0
        sleep 0.1
        i=$((i + 1))
    done
    kill -KILL "$p" 2>/dev/null || true
}

# Chờ tới khi QEMU `$1` biến mất; in số ms đã chờ (hoặc 999999 nếu quá `$2` ms).
# Đo bằng **bước 50 ms** (không fork `python3` mỗi vòng): `ms` = số bước × 50 (±50 ms).
wait_gone_ms() {
    local p="$1" limit="$2" steps=0 max=0
    max=$((limit / 50 + 1))
    while kill -0 "$p" 2>/dev/null; do
        steps=$((steps + 1))
        if [ "$steps" -ge "$max" ]; then
            echo 999999
            return 0
        fi
        sleep 0.05
    done
    echo $((steps * 50))
}

case_dir() {
    local d="$WORK/$1"
    mkdir -p "$d/qgdir" || return 1
    echo "$d"
}

# ---------------------------------------------------------------- wrapper: pattern CŨ
# Chép **đúng** pattern hiện hành trước KT-78: `"$QEMU" … & pid=$!`, `kill $pid` khi timeout, không
# trap, không process group, không watchdog.
write_old_wrapper() {
    local dir="$1" qlog="$2" ready="$3" info="$4"
    cat >"$dir/wrapper.sh" <<EOF
#!/usr/bin/env bash
# pattern CŨ (trước KT-78) — tái hiện D2-116
set -u
QEMU="$QEMU"
"\$QEMU" -S -nographic -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot >"$qlog" 2>&1 &
pid=\$!
printf '# wrapper_cu: pid=%s pgid=%s qemu=%s\n' "\$\$" "\$(ps -o pgid= -p "\$\$" | tr -d ' ')" "\$pid" >"$info"
: >"$ready"
sleep 600 & wait \$!
EOF
    chmod +x "$dir/wrapper.sh"
}

# wrapper TREO kiểu cũ: chương trình không bao giờ thoát + timeout kiểu cũ (`kill $pid`).
write_old_hang_wrapper() {
    local dir="$1" qlog="$2" tds="$3"
    cat >"$dir/wrapper.sh" <<EOF
#!/usr/bin/env bash
# pattern CŨ + khách treo — đo nhánh timeout (đối chứng: nhánh này vốn đã kill được con trực tiếp)
set -u
QEMU="$QEMU"
"\$QEMU" -bios "$dir/hang.bin" -nographic -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot >"$qlog" 2>&1 &
pid=\$!
waited=0
while kill -0 "\$pid" 2>/dev/null && [ "\$waited" -lt "$tds" ]; do
    sleep 0.1
    waited=\$((waited + 1))
done
if kill -0 "\$pid" 2>/dev/null; then
    kill "\$pid" 2>/dev/null
    wait "\$pid" 2>/dev/null
    exit 124
fi
wait "\$pid" 2>/dev/null
exit 0
EOF
    chmod +x "$dir/wrapper.sh"
}

# ---------------------------------------------------------------- wrapper: qemu_guard.sh
# `$1`=dir `$2`=log QEMU `$3`=tds `$4`=kiểu (`S` = CPU đóng băng, `hang` = khách treo) `$5`=ảnh
write_guard_wrapper() {
    local dir="$1" qlog="$2" tds="$3" kind="$4" image="${5:-}" boot=""
    if [ "$kind" = hang ]; then
        boot='qg_boot "$LOGFILE" "$TDS" "$QEMU" -bios "$HANG" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot'
    elif [ "$kind" = s_only ]; then
        boot='qg_boot "$LOGFILE" "$TDS" "$QEMU" -S -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot'
    elif [ "$kind" = iso_bad ]; then
        boot='qg_boot "$LOGFILE" "$TDS" "$QEMU" -kernel "$IMAGE" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot
rc=$?
printf "# iso_bad rc=%s isolation_bad=%s\n" "$rc" "$QG_ISO_BAD" >"$INFOFILE"
qg_infra after-iso-bad >>"$INFOFILE" 2>&1 || true
exit $rc'
    elif [ "$kind" = clean ]; then
        boot='qg_boot "$LOGFILE" "$TDS" "$QEMU" -kernel "$IMAGE" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot'
    else
        boot='qg_boot "$LOGFILE" "$TDS" "$QEMU" -S -kernel "$IMAGE" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot'
    fi
    {
        echo '#!/usr/bin/env bash'
        printf '# KT-78: process group riêng + trap + timeout cứng + watchdog (kind=%s)\n' "$kind"
        echo 'set -u'
        printf '. %q\n' "$GUARD"
        printf 'QG_DIR=%q\n' "$dir/qgdir"
        printf 'QG_LOG=%q\n' "$dir/qg.log"
        printf 'LOGFILE=%q\n' "$qlog"
        printf 'TDS=%q\n' "$tds"
        printf 'QEMU=%q\n' "$QEMU"
        if [ "$kind" = hang ]; then
            printf 'HANG=%q\n' "$dir/hang.bin"
        else
            printf 'IMAGE=%q\n' "$image"
        fi
        printf 'INFOFILE=%q\n' "$dir/info.txt"
        if [ "$kind" = iso_bad ]; then
            # Ca âm D2-108B: bỏ setpgrp ⇒ QEMU ở group của wrapper. **Mặc định** 1 nhưng cho
            # override từ ngoài (`QG_FORCE_NO_SETPGRP=0` ⇒ ca phải ĐỎ) để tái lập **từ ngoài**.
            echo 'QG_FORCE_NO_SETPGRP=${QG_FORCE_NO_SETPGRP:-1}'
        fi
        echo 'qg_init || exit 2'
        echo 'printf '"'"'# wrapper: pid=%s pgid=%s\n'"'"' "$$" "$(ps -o pgid= -p $$ | tr -d '"'"' '"'"')" >"$INFOFILE"'
        printf '%s\n' "$boot"
        [ "$kind" = iso_bad ] || echo 'exit $?'
    } >"$dir/wrapper.sh"
    chmod +x "$dir/wrapper.sh"
}

# Chạy wrapper trong **nền** với SIGINT = DEFAULT (bash đặt SIGINT=ignore cho job nền, nên phải
# đặt lại trước exec — nếu không, `kill -INT` không bao giờ chạm tới được trap).
launch_bg() {
    local wrapper="$1" out="$2" reset_int="${3:-0}"
    if [ "$reset_int" = 1 ]; then
        python3 -c 'import os, signal, sys
signal.signal(signal.SIGINT, signal.SIG_DFL)
os.execv("/bin/bash", ["/bin/bash"] + sys.argv[1:])' "$wrapper" >"$out" 2>&1 &
    else
        bash "$wrapper" >"$out" 2>&1 &
    fi
    LAUNCHED_PID=$!
}

# Chờ bằng chứng "QEMU đã chạy" (READY của wrapper cũ / `boot_start` trong guard log).
# In `pid=<qemu pid>` hoặc rỗng nếu quá 10 s.
wait_boot_up() {
    local dir="$1" mode="$2" i=0 pid=""
    while [ "$i" -lt 200 ]; do
        if [ "$mode" = old ]; then
            if [ -f "$dir/ready" ] && [ -s "$dir/info.txt" ]; then
                pid="$(sed -n 's/.* qemu=\([0-9][0-9]*\).*/\1/p' "$dir/info.txt" | head -1)"
                [ -n "$pid" ] && {
                    echo "$pid"
                    return 0
                }
            fi
        else
            if [ -f "$dir/qg.log" ]; then
                pid="$(sed -n 's/^# boot_start .*pid=\([0-9][0-9]*\).*/\1/p' "$dir/qg.log" | head -1)"
                [ -n "$pid" ] && {
                    echo "$pid"
                    return 0
                }
            fi
        fi
        sleep 0.05
        i=$((i + 1))
    done
    echo ""
}

record() { # record <tên> <kết quả 0/1> <mô tả>
    CASES=$((CASES + 1))
    if [ "$2" = 0 ]; then
        OKS=$((OKS + 1))
        say "ok $CASES - $1: $3"
    else
        say "not ok $CASES - $1: $3"
    fi
}

say "# KT-78 guard_selftest — $(date -u '+%Y-%m-%dT%H:%M:%SZ') mode=$MODE"
say "# qemu=$QEMU"
say "# opensbi=$SBI"
say "# image=$IMAGE exists=$([ -f "$IMAGE" ] && echo 1 || echo 0)"
say "# logdir=$WORK"
BASE_QEMU="$(qemu_total)"
say "# baseline: qemu_total=$BASE_QEMU qemu_ppid1=$(qemu_ppid1) ncpu=$(sysctl -n hw.ncpu 2>/dev/null || echo '?') loadavg=$(sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' || echo '?')"
if [ "$BASE_QEMU" -ne 0 ]; then
    say "# T-INFRA: đã có $BASE_QEMU QEMU lạ trước khi đo — số dưới đây so với baseline, không so với 0"
fi

# ============================================================ (a) cha chết
# old-sigkill / old-sigint: ĐỎ TRƯỚC — QEMU phải còn sống với PPID 1.
run_old_kill() { # $1=tag $2=signal
    local tag="$1" sig="$2" dir pid wp wpid left ppidms ppid1
    dir="$(case_dir "old-$tag")" || return 1
    write_old_wrapper "$dir" "$dir/qemu.log" "$dir/ready" "$dir/info.txt"
    wp="$dir/wrapper.sh"
    launch_bg "$wp" "$dir/wrapper.out" 1
    wpid="$LAUNCHED_PID"
    pid="$(wait_boot_up "$dir" old)"
    if [ -z "$pid" ]; then
        say "not ok - old-$tag: QEMU không lên trong 10 s"
        kill -9 "$wpid" 2>/dev/null
        LAST_QEMU_PID=""
        record "old-$tag" 1 "QEMU không lên trong 10 s"
        return 1
    fi
    LAST_QEMU_PID="$pid"
    alive="$(ps -o pid=,ppid=,pgid=,stat= -p "$pid" 2>/dev/null | tr '\n' ' ')"
    say "# old-$tag: wrapper=$wpid qemu=$pid alive_before=[$alive] pgid_wrapper=$(sed -n 's/.*pgid=\([0-9]*\).*/\1/p' "$dir/info.txt" | head -1)"
    if [ -z "$alive" ]; then
        say "not ok - old-$tag: QEMU KHÔNG sống ngay trước khi gửi signal ⇒ ca không có giá trị"
        force_kill_pid "$pid"
        kill -9 "$wpid" 2>/dev/null
        LAST_QEMU_PID=""
        record "old-$tag" 1 "QEMU không sống trước khi gửi signal"
        return 1
    fi
    kill -"$sig" "$wpid" 2>/dev/null
    sleep 1
    if kill -0 "$pid" 2>/dev/null; then
        ppid1="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')"
        say "# old-$tag: SAU kill -$sig cha — QEMU pid=$pid ppid=$ppid1 (mồ côi: tái hiện D2-116)"
        printf '# qemu-con-song pid=%s ppid=%s ps=%s\n' "$pid" "$ppid1" \
            "$(ps -o pid=,ppid=,pgid=,stat= -p "$pid" 2>/dev/null | tr '\n' ' ')" >"$dir/qemu-con-song.txt"

        record "old-$tag" 0 "ĐỎ TRƯỚC tái hiện: qemu_con_song=1 qemu_ppid=$ppid1 (D2-116)"
    else
        record "old-$tag" 1 "kỳ vọng còn QEMU mồ côi nhưng đã mất — pattern cũ không tái hiện được"
    fi
    # Dọn: chỉ kill đúng PID do chính ca này ghi lại (AGENTS §9).
    force_kill_pid "$pid"
    wait "$wpid" 2>/dev/null
    left="$(qemu_total)"
    [ "$left" -le "$BASE_QEMU" ] || say "# old-$tag: còn $left QEMU sau dọn"
    LAST_QEMU_PID=""
    return 0
}

if [ "$MODE" = old ] || [ "$MODE" = both ]; then
    run_old_kill sigkill KILL
    run_old_kill sigint INT
fi

# guard-*: cùng kịch bản, qua qemu_guard.sh ⇒ phải 0 QEMU trong ≤ 5 s.
run_guard_kill() { # $1=tag $2=signal $3=reset_int
    local tag="$1" sig="$2" reset="$3" dir pid wp wpid gone left rc alive
    dir="$(case_dir "guard-$tag")" || return 1
    if [ -f "$IMAGE" ]; then
        write_guard_wrapper "$dir" "$dir/qemu.log" 600 S "$IMAGE"
    else
        write_guard_wrapper "$dir" "$dir/qemu.log" 600 s_only ""
    fi
    wp="$dir/wrapper.sh"
    launch_bg "$wp" "$dir/wrapper.out" "$reset"
    wpid="$LAUNCHED_PID"
    pid="$(wait_boot_up "$dir" guard)"
    if [ -z "$pid" ]; then
        say "not ok - guard-$tag: QEMU không lên trong 10 s"
        kill -9 "$wpid" 2>/dev/null
        record "guard-$tag" 1 "QEMU không lên trong 10 s"
        return 1
    fi
    LAST_QEMU_PID="$pid"
    alive="$(ps -o pid=,ppid=,pgid=,stat= -p "$pid" 2>/dev/null | tr '\n' ' ')"
    say "# guard-$tag: wrapper=$wpid qemu=$pid alive_before=[$alive] pgid_wrapper=$(sed -n 's/.*pgid=\([0-9]*\).*/\1/p' "$dir/info.txt" | head -1)"
    if [ -z "$alive" ]; then
        say "not ok - guard-$tag: QEMU KHÔNG sống ngay trước khi gửi signal ⇒ ca không có giá trị"
        force_kill_pid "$pid"
        kill -9 "$wpid" 2>/dev/null
        LAST_QEMU_PID=""
        record "guard-$tag" 1 "QEMU không sống trước khi gửi signal"
        return 1
    fi
    kill -"$sig" "$wpid" 2>/dev/null
    gone="$(wait_gone_ms "$pid" "$LIMIT_MS")"
    wait "$wpid" 2>/dev/null
    rc=$?
    left="$(qemu_total)"
    if [ "$gone" = 999999 ]; then
        say "# guard-$tag: QEMU pid=$pid VẪN SỐNG sau ${LIMIT_MS} ms — $(ps -o pid=,ppid=,pgid=,stat= -p "$pid" 2>/dev/null)"
        force_kill_pid "$pid"
        record "guard-$tag" 1 "qemu_con_song=1 sau ${LIMIT_MS} ms — guard KHÔNG dọn"
    else
        say "# guard-$tag: qemu_gone_ms=$gone wrapper_rc=$rc qemu_total=$left baseline=$BASE_QEMU"
        # **T-INFRA (KT-402, D2-07)** — bất biến của guard là **QEMU của ca này** chết trong
        # `LIMIT_MS`, KHÔNG phải "toàn máy không còn QEMU nào": làn QEMU song song của cổng có thể
        # xuất hiện giữa hai lần đếm ⇒ `qemu_total` lệch baseline dù guard chạy đúng (đã tái hiện
        # 2 lượt T2 ngày 2026-11-25). `gone != 999999` đã chứng minh pid của ca đã chết; số toàn
        # cục chỉ để truy vết, không dùng để kết luận.
        record "guard-$tag" 0 "QEMU của ca chết sau ${gone} ms (<${LIMIT_MS} ms); wrapper_rc=$rc; toàn cục=$left baseline=$BASE_QEMU"
    fi
    LAST_QEMU_PID=""
    return 0
}

if [ "$MODE" = guard ] || [ "$MODE" = both ]; then
    run_guard_kill sigkill KILL 0
    run_guard_kill sigint INT 1
    run_guard_kill sigterm TERM 0
fi

# ============================================================ (b) boot treo
# Sinh khách **không bao giờ thoát**: 4 byte `6f 00 00 00` = `j .` (JAL x0, 0) — QEMU nạp bằng
# `-bios` dạng raw tại 0x8000_0000 và quay vòng mãi mãi. Không cần toolchain, không cần kernel.
printf '\x6f\x00\x00\x00' >"$WORK/hang.bin"
say "# hang.bin bytes=$(wc -c <"$WORK/hang.bin" | tr -d ' ') hex=$(od -An -tx1 "$WORK/hang.bin" | tr -d ' \n') (= 'j .' quay vô hạn)"

run_hang() { # $1=tag $2=mode(old|guard) $3=tds
    local tag="$1" mode="$2" tds="$3" dir t0 t1 dt rc left
    dir="$(case_dir "$tag")" || return 1
    cp "$WORK/hang.bin" "$dir/hang.bin"
    if [ "$mode" = old ]; then
        write_old_hang_wrapper "$dir" "$dir/qemu.log" "$tds"
    else
        write_guard_wrapper "$dir" "$dir/qemu.log" "$tds" hang ""
    fi
    before_pids="$(qemu_pids | tr '\n' ' ')"
    t0="$(now_ms)"
    bash "$dir/wrapper.sh" >"$dir/wrapper.out" 2>&1
    rc=$?
    t1="$(now_ms)"
    dt=$((t1 - t0))
    # **T-INFRA (KT-402)** — chờ tối đa 1 s cho QEMU **mới của ca này** biến mất (SIGTERM vừa gửi,
    # chưa reap), rồi so **tập pid trước/sau** thay vì so tổng với baseline (làn song song làm tổng
    # đổi giữa hai lần đếm — xem `qemu_pids`).
    left_new=""
    for _ in $(seq 1 40); do
        left_new="$(qemu_pids | awk -v b="$before_pids" \
            'BEGIN { n = split(b, a, " "); for (i = 1; i <= n; i++) keep[a[i]] = 1 }
             !($1 in keep) { print }')"
        [ -z "$left_new" ] && break
        sleep 0.025
    done
    left="$(qemu_total)"
    say "# $tag: exit=$rc elapsed_ms=$dt timeout_ds=$tds qemu_total=$left baseline=$BASE_QEMU new_qemu=[$(printf '%s' "$left_new" | tr '\n' ' ')]"
    if [ "$rc" = 124 ] && [ -z "$left_new" ]; then
        record "$tag" 0 "timeout kích hoạt: exit=124 sau ${dt} ms (không treo vô hạn), QEMU của ca đã chết"
    elif [ "$rc" = 0 ]; then
        record "$tag" 1 "boot treo nhưng exit=0 — timeout KHÔNG kích hoạt"
    elif [ "$rc" = 124 ]; then
        record "$tag" 1 "timeout kích hoạt nhưng còn QEMU mới: $(printf '%s' "$left_new" | tr '\n' ' ')"
    else
        record "$tag" 1 "exit=$rc (kỳ vọng 124) sau ${dt} ms"
    fi
    return 0
}

if [ "$MODE" = guard ] || [ "$MODE" = both ]; then
    # Nhánh timeout **cũ** vẫn kill được con trực tiếp ⇒ đối chứng để thấy (b) không phải lỗi cũ;
    # điều mới là timeout **kill cả group** + không để lại gì.
    run_hang old-timeout old 20
    run_hang guard-timeout guard 20
fi

# ============================================================ boot sạch + bất biến PGID
run_clean_boot() {
    local dir rc tds n log
    dir="$(case_dir "clean-boot")" || return 1
    if [ ! -f "$IMAGE" ]; then
        say "# clean-boot: KHÔNG có ảnh sản phẩm $IMAGE — bỏ ca (nêu rõ, không tính ok)"
        return 0
    fi
    write_guard_wrapper "$dir" "$dir/qemu.log" 200 clean "$IMAGE"
    bash "$dir/wrapper.sh" >"$dir/wrapper.out" 2>&1
    rc=$?
    log="$dir/qemu.log"
    # **KT-342a (D2-249)** — chấp nhận **hai** ảnh: sản phẩm (`# k3 halt` + `ok 1 - k3 boot`) hoặc
    # ktest (`# ktest halt` + `ok 1 - ktest image`). Điều ca này **thật sự** đo là `exit=0` +
    # `isolated=1` của guard wrapper, không phải hình dạng TAP của một ảnh cụ thể.
    if [ "$rc" = 0 ] &&
        grep -qE '^# (k3|ktest) halt$' "$log" &&
        grep -qE '^ok 1 - (k3 boot|ktest image)$' "$log"; then
        n="$(sed -n 's/^# boot_end .*isolated=\([0-9a-z]*\).*/\1/p' "$dir/qg.log" | wc -l | tr -d ' ')"
        say "# clean-boot: exit=0 (QEMU tự thoát) isolated_line=$(sed -n 's/^# boot_end .*isolated=\([0-9a-z]*\).*/\1/p' "$dir/qg.log" | tr '\n' ' ') lines=$n"
        if grep -q '^# boot_end .*isolated=1' "$dir/qg.log"; then
            record "clean-boot" 0 "boot sạch exit=0, QEMU ở process group riêng (isolated=1 trong guard log)"
        else
            record "clean-boot" 1 "boot sạch nhưng guard log KHÔNG chứng minh isolated=1"
        fi
    else
        say "# clean-boot: exit=$rc — tail log:" >>"$TRANSCRIPT"
        tail -5 "$log" >>"$TRANSCRIPT" 2>/dev/null
        record "clean-boot" 1 "boot sạch exit=$rc (kỳ vọng 0)"
    fi
}
if [ "$MODE" = guard ] || [ "$MODE" = both ]; then
    run_clean_boot
fi

# ============================================================ (c) hạ tầng: QEMU mồ côi ⇒ KHÔNG HỢP LỆ
# D2-108B: bộ đếm `qemu_ppid1` chỉ có nghĩa nếu có ca làm nó **khác 0** ⇒ phép đo bị đánh dấu.
#
# **T-INFRA (KT-407, D2-260 mục 5)** — bản cũ so **cứng** `qemu_ppid1=1` toàn cục: chỉ cần một làn
# QEMU song song của cổng đã để lại QEMU mồ côi (hoặc chính ca này chạy khi máy có QEMU PPID 1 khác)
# là `qemu_ppid1=2` ⇒ ca ĐỎ oan dù `qg_infra` vẫn kết luận đúng. Bất biến thật là **của ca này**:
#   (i)   PID do ca tạo có `ppid == 1` (đúng là QEMU mồ côi);
#   (ii)  `verdict == KHONG_HOP_LE`;
#   (iii) `after_ppid1 >= before_ppid1 + 1` — đo **trước** khi tạo mồ côi, không so cứng toàn cục;
#   (iv)  PID của ca **chết** sau `force_kill_pid` (khuôn `6cf4640c`: khẳng định PID của ca, không
#         so tổng toàn máy).
run_infra_orphan() {
    local dir pid verdict line before_ppid1 after_ppid1 ppid gone
    dir="$(case_dir "infra-orphan")" || return 1
    before_ppid1="$(qemu_ppid1)"
    (
        "$QEMU" -S -nographic -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot \
            >"$dir/qemu.log" 2>&1 &
        echo $! >"$dir/qemu.pid"
        exit 0
    )
    sleep 0.5
    pid="$(cat "$dir/qemu.pid" 2>/dev/null)"
    if [ -z "$pid" ] || ! kill -0 "$pid" 2>/dev/null; then
        record "infra-orphan" 1 "không tạo được QEMU mồ côi (pid=${pid:-none})"
        return 0
    fi
    LAST_QEMU_PID="$pid"
    ppid="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')"
    say "# infra-orphan: QEMU pid=$pid ppid=$ppid before_ppid1=$before_ppid1 (tạo mồ côi có kiểm soát)"
    line="$(bash "$HERE/infra.sh" orphan /dev/null 2>&1)"
    say "# infra-orphan: $line"
    verdict="$(printf '%s' "$line" | sed -n 's/.*verdict=\([A-Za-z_]*\).*/\1/p')"
    # `line` gồm **ba** dòng (stdout `# infra …`, stderr `WARNING …`, stdout
    # `# MEASUREMENT_INVALID …`) ⇒ chỉ lấy dòng đầu, nếu không `after_ppid1` thành "2\n2\n2"
    # và phép so số hỏng (đã tái hiện: ca ĐỎ oan dù mọi giá trị đúng).
    after_ppid1="$(printf '%s\n' "$line" | sed -n 's/.*qemu_ppid1=\([0-9][0-9]*\).*/\1/p' | head -1)"
    printf '%s\n' "$line" >"$dir/infra-orphan.txt"
    # (iv) khuôn `6cf4640c`: khẳng định **PID của ca** chết sau khi dọn, không so tổng toàn máy.
    force_kill_pid "$pid"
    gone="$(wait_gone_ms "$pid" 2000)"
    say "# infra-orphan: pid=$pid gone_ms=$gone sau force_kill_pid"
    if [ "$ppid" = 1 ] && [ "$verdict" = KHONG_HOP_LE ] &&
        [ -n "$after_ppid1" ] && [ "$after_ppid1" -ge $((before_ppid1 + 1)) ] &&
        [ "$gone" != 999999 ]; then
        record "infra-orphan" 0 "QEMU của ca ppid=1; verdict=$verdict; qemu_ppid1 $before_ppid1→$after_ppid1 (≥ +1); pid chết sau ${gone} ms"
    else
        record "infra-orphan" 1 "ppid=$ppid (kỳ vọng 1) verdict=$verdict qemu_ppid1 $before_ppid1→${after_ppid1:-?} (kỳ vọng ≥ $((before_ppid1 + 1))) gone_ms=$gone (kỳ vọng != 999999)"
    fi
    LAST_QEMU_PID=""
    return 0
}
if [ "$MODE" = guard ] || [ "$MODE" = both ]; then
    run_infra_orphan
fi

# ============================================================ (c2) ca âm bộ đếm isolation (D2-108B)
# Chạy QEMU **không** qua `setpgrp` ⇒ PGID == group của wrapper ⇒ guard phải ghi `isolated=0`,
# tăng `QG_ISO_BAD`, và `qg_infra` phải kết luận `KHONG_HOP_LE ly_do=isolation_bad=…`.
# Đây là ca âm cho ba bộ đếm `isolated_0`/`isolation_bad`/`isolated_unknown` (reviewer KT-78, D2-108B)
# — không có ca này thì `0` của chúng không chứng minh gì.
run_iso_bad() {
    local dir rc log info
    dir="$(case_dir "iso-bad")" || return 1
    if [ ! -f "$IMAGE" ]; then
        say "# iso-bad: KHÔNG có ảnh sản phẩm $IMAGE — bỏ ca (nêu rõ, không tính ok)"
        return 0
    fi
    write_guard_wrapper "$dir" "$dir/qemu.log" 200 iso_bad "$IMAGE"
    bash "$dir/wrapper.sh" >"$dir/wrapper.out" 2>&1
    rc=$?
    log="$dir/qg.log"
    info="$dir/info.txt"
    say "# iso-bad: wrapper_rc=$rc boot_start=[$(grep '^# boot_start' "$log" 2>/dev/null | head -1)]"
    if grep -q '^# boot_start .* isolated=0' "$log" 2>/dev/null &&
        grep -q '^# iso_bad rc=0 isolation_bad=1$' "$info" 2>/dev/null &&
        grep -q 'verdict=KHONG_HOP_LE ly_do=isolation_bad=1$' "$info" 2>/dev/null; then
        record "iso-bad" 0 "QEMU KHÔNG qua setpgrp ⇒ isolated=0, isolation_bad=1, infra KHONG_HOP_LE (bộ đếm có ca âm — D2-108B)"
    else
        say "# iso-bad: info=[$(tr '\n' ' ' <"$info" 2>/dev/null)]"
        record "iso-bad" 1 "guard KHÔNG phát hiện QEMU ngoài process group riêng"
    fi
    return 0
}
if [ "$MODE" = guard ] || [ "$MODE" = both ]; then
    run_iso_bad
fi

# ============================================================ tổng kết
FINAL_QEMU="$(qemu_total)"
FINAL_PPID1="$(qemu_ppid1)"
say "# tổng kết: qemu_total=$FINAL_QEMU qemu_ppid1=$FINAL_PPID1 baseline=$BASE_QEMU"
if [ "$FINAL_QEMU" -ne "$BASE_QEMU" ]; then
    say "# T-INFRA: toàn cục còn $FINAL_QEMU QEMU (baseline=$BASE_QEMU) — có QEMU ngoài ca, không dùng để kết luận"
fi
say "# guard_selftest: cases=$CASES ok=$OKS logdir=$WORK"
say "# transcript=$TRANSCRIPT"
if [ "$OKS" -eq "$CASES" ]; then
    say "# guard_selftest: PASS"
    exit 0
fi
say "# guard_selftest: FAIL ($((CASES - OKS))/$CASES ca lệch)"
exit 1
