#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-348a** — ca kiểm cho **phân loại nguyên nhân kết thúc QEMU** trong `qemu_guard.sh` (D2-116).
#
# # Vì sao có tệp này (sự cố KT-348)
#
# Nghiệm thu KT-348 (`v2 nightly --soak --profile product-shell`, x86_64 N=600 × 3 luồng) có
# **1/600** boot đỏ: log dừng sau TAP 9 (`# timer: lapic …`), dòng cuối là
# `qemu-system-x86_64: terminating on signal 15 from pid 73084 (<unknown process>)`.
# Đo lại log guard của lượt đó: `rc_124=0`, `max secs=5.1` (ngưỡng 20 s) ⇒ **guard KHÔNG hết hạn**:
# QEMU bị **tiến trình ngoài** giết ở giây 1,1 — nhưng `qg_boot` trả **0** (vì `wait` không được
# đọc mã thoát) nên soak chấm đó là **boot đỏ của kernel**. Một SIGTERM từ ngoài (agent khác,
# `pkill`, OOM…) trở thành "lỗi kernel không tất định" và đốt cả một vòng nghiệm thu.
#
# # Hợp đồng mới (`qemu_guard.sh`, KT-348a)
#
#   QEMU tự thoát              ⇒ `qg_boot` = 0,  `ext_sig=0 killer=none`
#   guard tự kill (quá `tds`)   ⇒ `qg_boot` = 124, `ext_sig=0`  (boot **treo** — vẫn là ĐỎ của kernel)
#   **tín hiệu từ NGOÀI**       ⇒ `qg_boot` = **126**, `ext_sig=<N> killer=<pid>` ⇒ soak **TỪ CHỐI ĐO**
#
# # Đỏ-trước (D2-05, đúng khuôn `guard_selftest.sh --mode old|guard`)
#
#   `--mode old`   : đúng đoạn mã **trước KT-348a** (`wait` rồi trả 0 bất kể mã thoát) ⇒ ca
#                    `ext-sig` **ĐỎ** (rc=0 thay vì 126) — chứng minh ca kiểm có răng.
#   `--mode guard` : qua `qemu_guard.sh` hiện tại ⇒ `ext-sig` XANH.
#   `--mode both`  : mặc định — chạy cả hai, đòi `old` đỏ **và** `guard` xanh.
#
# Không cần ảnh khách/không cần QEMU: "khách" là `/bin/sleep` (guard chỉ quan tâm mã thoát của
# tiến trình trong process group riêng). Không đụng QEMU nào khác trên máy (AGENTS §9).
#
# Dùng: bash v2/test/boot/guard_extsig_selftest.sh [--mode old|guard|both] [--logdir DIR]
# Exit: 0 = mọi ca đúng kỳ vọng; 1 = có ca lệch; 2 = lỗi môi trường.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GUARD="$HERE/qemu_guard.sh"
MODE=both
LOGDIR="${KT348A_LOGDIR:-${TMPDIR:-/tmp}/v2-kt348a-$$}"
SLEEP_BIN="$(command -v sleep 2>/dev/null || echo /bin/sleep)"
CASES=0
FAILS=0

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) MODE="${2:-}"; shift 2 ;;
        --logdir) LOGDIR="${2:-}"; shift 2 ;;
        -h | --help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "guard_extsig_selftest: tham số lạ $1" >&2
            exit 2
            ;;
    esac
done
case "$MODE" in old | guard | both) ;; *) echo "guard_extsig_selftest: --mode lạ [$MODE]" >&2; exit 2 ;; esac
[ -f "$GUARD" ] || {
    echo "guard_extsig_selftest: không thấy $GUARD" >&2
    exit 2
}
[ -x "$SLEEP_BIN" ] || {
    echo "guard_extsig_selftest: không có $SLEEP_BIN" >&2
    exit 2
}
mkdir -p "$LOGDIR" 2>/dev/null || {
    echo "guard_extsig_selftest: không tạo được logdir $LOGDIR" >&2
    exit 2
}

ok_case() {
    CASES=$((CASES + 1))
    echo "ok $CASES - $*"
}
bad_case() {
    CASES=$((CASES + 1))
    FAILS=$((FAILS + 1))
    echo "not ok $CASES - $*"
}

# ---------------------------------------------------------------------------------------------
# Ca 1: đọc **pid người gửi** + số hiệu tín hiệu từ log QEMU (nguồn: dòng thật của sự cố KT-348)
# ---------------------------------------------------------------------------------------------
# shellcheck source=/dev/null
. "$GUARD" 2>/dev/null || true
REAL="$LOGDIR/ket348-boot-7.log"
cat >"$REAL" <<'EOF'
ok 9 - apic: ioapic
# timer: lapic count0=980000 count1=945000 runs=1 ioapic_route=1 div_ok=1 nominal_hz=100 pit=0x2e79->0x29a8
qemu-system-x86_64: terminating on signal 15 from pid 73084 (<unknown process>)
EOF
k="$(qg_parse_killer "$REAL" 2>/dev/null || true)"
s="$(qg_parse_sig "$REAL" 2>/dev/null || true)"
if [ "$k" = "73084" ] && [ "$s" = "15" ]; then
    ok_case "log thật của KT-348: killer=73084 sig=15 (đọc từ dòng 'terminating on signal 15 from pid 73084')"
else
    bad_case "parse killer/sig sai: killer='$k' sig='$s' (đòi 73084/15)"
fi
# Ca 1b: log **không** có tín hiệu (boot sạch) ⇒ rỗng, không được bịa pid.
printf 'ok 47 - shell: exit\n# viFastOs x86_64: halt\n' >"$LOGDIR/clean.log"
if [ -z "$(qg_parse_killer "$LOGDIR/clean.log" 2>/dev/null || true)" ]; then
    ok_case "log boot sạch: killer rỗng (không bịa pid khi QEMU tự thoát)"
else
    bad_case "log boot sạch mà parse ra killer khác rỗng"
fi

# ---------------------------------------------------------------------------------------------
# Ca 2/3/4: phân loại ba đường kết thúc — dùng driver **tự viết** (old = mã trước KT-348a)
# ---------------------------------------------------------------------------------------------
# `drv_old_boot <log> <tds> <bin> [args…]`: đúng đoạn cũ — không đọc mã thoát, luôn trả 0 khi QEMU
# tự chết (đây là lỗi KT-348: SIGTERM ngoài ⇒ rc=0 ⇒ soak chấm là boot đỏ của kernel).
drv_old_boot() {
    local log="$1" tds="$2" bin="$3" pid i=0
    shift 3
    "$bin" "$@" >"$log" 2>&1 &
    pid=$!
    while [ "$i" -lt "$tds" ] && kill -0 "$pid" 2>/dev/null; do
        sleep 0.1
        i=$((i + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null
        return 124
    fi
    wait "$pid" 2>/dev/null
    return 0
}

# `run_ext_kill <driver>`: boot "khách" = sleep 30, giết **từ ngoài** bằng SIGTERM sau 0,3 s.
# In ra "<rc>|<ext_sig>|<killer>|<dòng boot_end có ext_sig>".
run_ext_kill() {
    local drv="$1" d="$LOGDIR/$1" out rc sig="" killer="" line=""
    rm -rf "$d" 2>/dev/null || true
    mkdir -p "$d" 2>/dev/null || true
    out="$d/boot.log"
    (
        QG_DIR="$d"
        QG_LOG="$d/qg.log"
        if [ "$drv" = "old" ]; then
            drv_old_boot "$out" 200 "$SLEEP_BIN" 30
        else
            qg_init || exit 125
            qg_boot "$out" 200 "$SLEEP_BIN" 30
        fi
    ) &
    local sub=$!
    sleep 0.3
    # "kẻ giết từ ngoài": gửi TERM tới đúng tiến trình `sleep` của khách (không phải guard).
    local gpid
    gpid="$(pgrep -P 1 -f "$SLEEP_BIN 30" 2>/dev/null | head -1)"
    [ -n "$gpid" ] || gpid="$(pgrep -f "^$SLEEP_BIN 30$" 2>/dev/null | head -1)"
    [ -n "$gpid" ] || gpid="$(ps -ax -o pid=,command= | awk -v b="$SLEEP_BIN 30" '$0 ~ b { print $1; exit }')"
    if [ -n "$gpid" ]; then
        kill -TERM "$gpid" 2>/dev/null || true
    fi
    wait "$sub" 2>/dev/null
    rc=$?
    # `qg_boot` chạy trong **subshell** ở trên (để giết được từ ngoài trong lúc nó chờ), nên biến
    # `QG_LAST_SIG`/`QG_LAST_KILLER` của subshell không trả về đây — nguồn đọc là **dòng log guard**
    # (`# boot_end … rc=126 ext_sig=15 killer=…`), tức đúng thứ `soak-x86_64.sh` dùng làm bằng chứng.
    line="$(grep '^# boot_end' "$d/qg.log" 2>/dev/null | tail -1)"
    sig="$(printf '%s' "$line" | sed -n 's/.* ext_sig=\([0-9][0-9]*\) .*/\1/p')"
    killer="$(printf '%s' "$line" | sed -n 's/.* killer=\([0-9A-Za-z_-][0-9A-Za-z_-]*\) .*/\1/p')"
    printf '%s|%s|%s|%s\n' "$rc" "${sig:-0}" "${killer:-}" "$line"
}

if [ "$MODE" = "old" ] || [ "$MODE" = "both" ]; then
    r="$(run_ext_kill old)"
    rc="${r%%|*}"
    if [ "$rc" = "0" ]; then
        ok_case "ĐỎ-TRƯỚC (--mode old): QEMU bị SIGTERM ngoài vẫn trả rc=0 ⇒ soak chấm 'boot đỏ của kernel' (đúng lỗi KT-348)"
    else
        bad_case "ĐỎ-TRƯỚC (--mode old) trả rc=$rc — mã cũ phải là 0 (ca kiểm không có răng?)"
    fi
fi

if [ "$MODE" = "guard" ] || [ "$MODE" = "both" ]; then
    r="$(run_ext_kill guard)"
    rc="${r%%|*}"
    rest="${r#*|}"
    sig="${rest%%|*}"
    rest="${rest#*|}"
    killer="${rest%%|*}"
    line="${rest#*|}"
    if [ "$rc" = "126" ]; then
        ok_case "tín hiệu NGOÀI ⇒ qg_boot trả 126 (không phải 0/124)"
    else
        bad_case "tín hiệu ngoài trả rc=$rc (đòi 126)"
    fi
    if [ "$sig" = "15" ]; then
        ok_case "QG_LAST_SIG=15 (đọc từ mã thoát 128+15 của wait)"
    else
        bad_case "QG_LAST_SIG='$sig' (đòi 15)"
    fi
    if printf '%s' "$line" | grep -q 'rc=126 ext_sig=15'; then
        ok_case "log guard có '# boot_end … rc=126 ext_sig=15' (bằng chứng phân loại trong log)"
    else
        bad_case "log guard thiếu 'rc=126 ext_sig=15': '$line'"
    fi
    if printf '%s' "$line" | grep -q 'killer=[0-9][0-9]*'; then
        ok_case "log guard ghi killer=<pid> của tiến trình ngoài"
    else
        # `sleep` không in dòng 'terminating on signal' của QEMU ⇒ killer rỗng là **đúng**;
        # ca này chỉ đòi killer=none, không đòi pid bịa ra.
        if printf '%s' "$line" | grep -q 'killer=none'; then
            ok_case "khách không in dòng của QEMU ⇒ killer=none (không bịa pid; ca 1 đã kiểm đường parse thật)"
        else
            bad_case "log guard thiếu killer= (rỗng cũng phải là 'none'): '$line'"
        fi
    fi
fi

# ---------------------------------------------------------------------------------------------
# Ca F7 (audit hạ tầng 2026-11-28): hạ tầng KHÔNG HỢP LỆ ⇒ runner phải kết thúc **mã 4** và
# **không** in PASS. Trước bản vá, `INFRA_INVALID` chỉ được `+=1` rồi `echo` (0 nhánh quyết định)
# ⇒ log vừa có `# MEASUREMENT_INVALID` vừa có `# <runner>: PASS`, mã thoát 0.
#
# Cách kiểm **không cần QEMU**: cắt **nguyên văn** đoạn mã quyết định cuối của từng runner (sed
# theo mốc), chạy nó trong subshell với `qg_infra` **giả lập** trả 1. Đoạn mã là chính dòng trong
# tệp thật ⇒ nếu ai bỏ dòng `qg_infra_verdict` (hoặc đổi mốc), ca này ĐỎ.
# ---------------------------------------------------------------------------------------------
_f7_stub_env() {
    INFRA_INVALID=0
    INFRA_LOG="$LOGDIR/f7-infra.log"
    : >"$INFRA_LOG" 2>/dev/null || true
    QG_ISO_BAD=0
    QG_LOG="$LOGDIR/f7-qg.log"
    : >"$QG_LOG" 2>/dev/null || true
    LOGDIR="$LOGDIR/f7-logs"
    mkdir -p "$LOGDIR" 2>/dev/null || true
    fails=0
    FAILS=0
    total_boots=1 clean_boots=1 ROWS=1 want_rows=1
    SMP_ARG=1 SMP=1 N_BOOT=1 LABELS="ktest" SMP_COUNT=1 LABEL_COUNT=1
    N=2 NEED=0 SMOKE=0 GIT_HEAD=deadbeef IMG_SHA=abc WANT_BUILD=1
    IMAGE=/dev/null RESULTS=/dev/null SOAK_WALL=1 WALL=1
}
# hạ tầng giả lập KHÔNG HỢP LỆ: `qg_infra` thật trả 1 đúng trong trường hợp này.
qg_infra() {
    echo "# infra $1 (giả lập): qemu_total=2 qemu_ppid1=0 qemu_own=0 qemu_foreign=1 verdict=KHONG_HOP_LE ly_do=qemu_foreign=1"
    return 1
}
qg_guard_summary() { return 0; }
print_summary() { return 0; }
soak_common_finish() { echo "# soak_common_finish (stub): PASS"; return 0; }

# `f7_tail_case <tệp> <mốc-đầu> <mốc-cuối> <nhãn>` — 0 ⇔ runner trả 4 + in KHÔNG HỢP LỆ + không PASS.
f7_tail_case() {
    local file="$1" start="$2" end="$3" label="$4" body="" out="" rc=0
    case "$file" in
        /*) ;;
        *) file="$HERE/$file" ;;
    esac
    body="$(sed -n "/$start/,/$end/p" "$file" 2>/dev/null)"
    if [ -z "$body" ]; then
        bad_case "F7 $label: KHÔNG thấy đoạn '$start' … '$end' trong ${file##*/} — mốc đã đổi ⇒ ca không kiểm gì (sửa ca theo mã)"
        return 0
    fi
    out="$(
        _f7_stub_env
        _f7_tail() { eval "$body"; }
        _f7_tail
    )" 2>&1
    rc=$?
    if [ "$rc" -ne 4 ]; then
        bad_case "F7 $label: INFRA_INVALID=1 mà exit=$rc (đòi 4 — T-INFRA; bản cũ = 0)"
    elif printf '%s\n' "$out" | grep -qE '^# [A-Za-z0-9_.-]+: PASS$'; then
        bad_case "F7 $label: trả 4 nhưng vẫn in dòng PASS (log tự mâu thuẫn)"
    elif ! printf '%s\n' "$out" | grep -q 'KHÔNG HỢP LỆ — không tính là PASS (T-INFRA)'; then
        bad_case "F7 $label: trả 4 nhưng thiếu dòng '# … KHÔNG HỢP LỆ — không tính là PASS (T-INFRA)'"
    else
        ok_case "F7 $label: INFRA_INVALID=1 ⇒ exit=4 + 'KHÔNG HỢP LỆ … (T-INFRA)', không in PASS"
    fi
}

if [ "$(qg_infra_verdict 0 'thử')" = "" ]; then
    ok_case "F7 qg_infra_verdict 0 ⇒ rc 0, không in gì (không đỏ oan khi hạ tầng hợp lệ)"
else
    bad_case "F7 qg_infra_verdict 0 vẫn in ra dòng (phải im lặng)"
fi
out="$(qg_infra_verdict 2 'run.sh' 2>&1)"
rc=$?
if [ "$rc" -eq 4 ] && printf '%s' "$out" | grep -q 'infra_invalid=2'; then
    ok_case "F7 qg_infra_verdict 2 ⇒ rc 4 + infra_invalid=2 (mã riêng của T-INFRA, khác đỏ sản phẩm 1)"
else
    bad_case "F7 qg_infra_verdict 2 ⇒ rc=$rc (đòi 4): $out"
fi
f7_tail_case ktest.sh 'qg_infra after "\$INFRA_LOG"' 'echo "# ktest.sh: PASS"' ktest.sh
f7_tail_case persist.sh 'qg_infra after "\$INFRA_LOG"' 'echo "# persist.sh: PASS"' persist.sh
f7_tail_case run.sh 'qg_infra after "\$INFRA_LOG"' 'qg_infra_verdict' run.sh
f7_tail_case soak.sh 'qg_infra after "\$INFRA_LOG"' 'qg_infra_verdict' soak.sh
f7_tail_case soak-aarch64.sh 'qg_infra after "\$INFRA_LOG"' 'qg_infra_verdict' soak-aarch64.sh
f7_tail_case soak-x86_64.sh 'qg_infra after "\$INFRA_LOG"' 'qg_infra_verdict' soak-x86_64.sh

# **F7 + AGENTS §9** — ngân sách QEMU "lạ" phải theo số QEMU song song được phép: ngoài cổng là 1
# (đo độc quyền); trong cổng T1/T2 là 3. Nếu ai bỏ `qg_foreign_budget` (quay lại `foreign > 0`),
# T2 sẽ đỏ oan mỗi lượt có ≥2 làn QEMU — ca này giữ đúng ngân sách đó.
_budget_case() { # <foreign_allow> <mong-đợi>
    local got
    got="$(V2_INFRA_FOREIGN_ALLOW="$1" qg_foreign_budget)"
    if [ "$got" = "$2" ]; then
        ok_case "F7 qg_foreign_budget: foreign_allow=$1 ⇒ '$2'"
    else
        bad_case "F7 qg_foreign_budget: foreign_allow=$1 ⇒ '$got' (đòi '$2')"
    fi
}
_budget_case 0 0
_budget_case 2 2
_budget_case '' 0
_budget_case xyz 0

echo
if [ "$FAILS" -gt 0 ]; then
    echo "guard_extsig_selftest: FAIL ($FAILS/$CASES ca sai) — logdir=$LOGDIR" >&2
    exit 1
fi
echo "guard_extsig_selftest: PASS ($CASES/$CASES ca) mode=$MODE — logdir=$LOGDIR"
exit 0
