#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-149 (D2-129) — harness T26 "read-your-own-write" trên **ảnh sản phẩm v2**.
#
# Vì sao có tệp này: tiêu chí N2 của v2 (D2-129) đòi **50 boot** ảnh sản phẩm
# (`--features product-shell,product-fs`, `-smp 2`) với kịch bản T26 — ghi rồi đọc lại **ngay**,
# so **từng byte** — và **0 đỏ**. Trước tệp này phép đo ấy chỉ làm bằng tay: không có cổng nào
# đếm được "boot nào đỏ", không có ca âm chứng minh harness **bắt được** log đỏ.
#
# Kịch bản T26 trên ảnh sản phẩm (không thêm lệnh shell, không đổi ngữ nghĩa FS):
#   1. chờ prompt `shell>` (shell U-mode sống);
#   2. gửi `write /t26 <payload>` — shell ghi theo khối 24 B rồi `TRUNC` + `SYNC`;
#   3. gửi `cat /t26` **ngay** — shell đọc lại theo khối 24 B và in **nội dung thật**;
#   4. so **nguyên văn** chuỗi byte `\n<payload>\n# shell: cat ok bytes=<n> first=<8 byte đầu>\n`
#      trong log serial — lệch một byte là đỏ (không so "chứa chuỗi con" rời rạc);
#   5. gửi `exit`, đòi `# product halt:` (thoát sạch, không treo).
#
# Định nghĩa "đỏ" của **một boot** (fail-closed, không nới):
#   - không thấy `shell>` trong hạn; hoặc
#   - không thấy `# shell: write /t26 bytes=<n> sync=ok`; hoặc
#   - không thấy **đúng một** chuỗi `\n<payload>\n# shell: cat ok bytes=<n> first=<8>\n`; hoặc
#   - log có `PANIC`, `# shell: ?`, `fs loi`, `dong qua dai`, `# shell: thieu`; hoặc
#   - không thấy `# product fs: medium=` (dịch vụ FS không lên) hoặc `# product halt:`.
#
# `--check-image` dùng **`build-identity.sh`** (KT-145): ảnh phải tự khai ĐÚNG `algo=` hiện hành;
# ảnh sai `algo=` (hoặc dạng cũ thiếu `algo=`) bị **TỪ CHỐI** — không đo trên ảnh không rõ danh tính.
#
# `--selftest` (bắt buộc theo luật repo: harness mới phải có ca âm): dựng log giả trong `mktemp -d`
# — một log **đỏ giả** (đổi một byte payload) phải bị bắt **ĐỎ**, một log **xanh giả** phải được
# nhận **XANH**, cộng hai ca đỏ giả khác (thiếu `# product halt:`, có `# shell: ?`). Không cần
# QEMU/ảnh cho selftest.
#
# Dùng:
#   bash v2/test/boot/t26.sh --check-image <ảnh.bin>          # chỉ kiểm danh tính ảnh
#   bash v2/test/boot/t26.sh --image <ảnh.bin> --boots N      # N boot T26 (mặc định N=50)
#   bash v2/test/boot/t26.sh --selftest                       # ca âm trên log giả
#   bash v2/test/boot/t26.sh --help
#
# Biến môi trường: `T26_LOGDIR` (mặc định `/tmp/kt149-t26`), `OPENSBI`.
#
# Exit: 0 = 0 boot đỏ (và selftest PASS nếu chạy); 1 = có boot đỏ / selftest FAIL;
#       2 = lỗi cấu hình (thiếu ảnh/QEMU/OpenSBI, ảnh sai danh tính, N < 1).
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"

# KT-145: cổng danh tính ảnh (tự nạp `build-rev.sh`).
# shellcheck source=/dev/null
. "$HERE/build-identity.sh"
# KT-78/OD-K6-6: mọi lần mở QEMU đi qua guard (process group riêng + timeout cứng + dọn).
# shellcheck source=/dev/null
. "$HERE/qemu_guard.sh"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

# Payload T26: 48 byte ASCII in được, **không** chứa `\n`/khoảng trắng (shell tách path/text ở
# khoảng trắng đầu; `cat` in nguyên byte). 48 B = 2 khối 24 B của giao thức FS ⇒ đúng kênh T26
# "ghi nhiều khúc rồi đọc lại ngay". `write /t26 ` = 11 B + 48 B = 59 B ≤ `MAX_LINE` 64 của shell.
PAYLOAD="T26-0123456789-abcdefghijklmnopqrstuvwxyz-ABCDEF"
PAYLOAD_LEN=${#PAYLOAD}
FIRST8="${PAYLOAD:0:8}"

LOGDIR="${T26_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/kt149-t26}"
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
QEMU_TDS=600 # trần QEMU 60 s/boot (qg_boot); harness tự kill sớm hơn khi chờ quá hạn
BOOT_POLLS=150
POLL_S=0.2
QGB_PID=""
LOG=""
FIFO=""

die() {
    echo "t26.sh: $*" >&2
    exit 2
}

usage() {
    sed -n 's/^# \{0,1\}//; 2,52p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,60p'
    echo "t26.sh: dùng: t26.sh --check-image <ảnh> | --image <ảnh> [--boots N] | --selftest | --help"
}

# ------------------------------------------------------------------ kiểm một log
# t26_check_log <log> <payload> -> 0 = XANH, 1 = ĐỎ (in lý do ra stdout, một dòng `# t26: ...`).
# Dùng chung cho boot thật và cho `--selftest` (cùng một hàm khẳng định ⇒ selftest chứng minh
# đúng cái harness dùng, không phải bản sao).
t26_check_log() {
    local log="$1" payload="$2"
    if [ ! -f "$log" ]; then
        echo "# t26: FAIL (không có log: $log)"
        return 1
    fi
    if ! command -v python3 >/dev/null 2>&1; then
        echo "# t26: FAIL (thiếu python3 — không so được từng byte)"
        return 1
    fi
    python3 - "$log" "$payload" <<'PY'
import sys

log_path, payload = sys.argv[1], sys.argv[2].encode()
try:
    data = open(log_path, "rb").read()
except OSError as exc:
    print(f"# t26: FAIL (không đọc được log: {exc})")
    sys.exit(1)

n = len(payload)
first8 = payload[:8]
need = b"\n" + payload + b"\n# shell: cat ok bytes=" + str(n).encode() + b" first=" + first8 + b"\n"
write_ok = b"# shell: write /t26 bytes=" + str(n).encode() + b" sync=ok"
forbidden = [b"PANIC", b"# shell: ?", b"fs loi", b"dong qua dai", b"# shell: thieu"]

if write_ok not in data:
    print("# t26: FAIL (thiếu dòng write ok: %s)" % write_ok.decode())
    sys.exit(1)
count = data.count(need)
if count != 1:
    print("# t26: FAIL (chuỗi cat đúng byte xuất hiện %d lần, cần đúng 1)" % count)
    sys.exit(1)
for token in forbidden:
    if token in data:
        print("# t26: FAIL (log có dấu hiệu hỏng: %s)" % token.decode())
        sys.exit(1)
if b"# product fs: medium=" not in data:
    print("# t26: FAIL (thiếu '# product fs: medium=' — dịch vụ FS không lên)")
    sys.exit(1)
if b"# product halt:" not in data:
    print("# t26: FAIL (thiếu '# product halt:' — shell không thoát sạch)")
    sys.exit(1)
print("# t26: PASS (payload=%d B, cat khớp từng byte, halt sạch)" % n)
sys.exit(0)
PY
}

# ------------------------------------------------------------------ selftest
t26_selftest() {
    local scratch rc=0 bad=0
    scratch="$(mktemp -d "${TMPDIR:-/tmp}/kt149-t26-selftest.XXXXXX")" || {
        echo "# t26-selftest: FAIL (mktemp)" >&2
        return 1
    }
    # shellcheck disable=SC2064
    trap "rm -rf '$scratch'" RETURN 2>/dev/null || true

    # (g1) ĐỐI CHỨNG DƯƠNG: log đúng ⇒ phải XANH. Thiếu ca này thì một checker luôn-đỏ cũng qua.
    {
        printf '# product fs: medium=ram blocks=96 used=2 total=66 volatile=1\n'
        printf 'shell>\n'
        printf '# shell: write /t26 bytes=%s sync=ok\n' "$PAYLOAD_LEN"
        printf 'shell>\n'
        printf '%s\n' "$PAYLOAD"
        printf '# shell: cat ok bytes=%s first=%s\n' "$PAYLOAD_LEN" "$FIRST8"
        printf 'shell>\n'
        printf '# product halt: shell_exit=0\n'
    } >"$scratch/green.log"
    if ! t26_check_log "$scratch/green.log" "$PAYLOAD" >"$scratch/green.out" 2>&1; then
        echo "# t26-selftest (g1) FAIL: log XANH giả mà checker báo đỏ: $(cat "$scratch/green.out")"
        bad=1
    fi

    # (g2) CA ÂM 1: đổi **một byte** của payload trong log ⇒ phải ĐỎ (đúng kênh T26).
    local wrong="${PAYLOAD%?}G"
    {
        printf '# product fs: medium=ram blocks=96 used=2 total=66 volatile=1\n'
        printf 'shell>\n'
        printf '# shell: write /t26 bytes=%s sync=ok\n' "$PAYLOAD_LEN"
        printf 'shell>\n'
        printf '%s\n' "$wrong"
        printf '# shell: cat ok bytes=%s first=%s\n' "$PAYLOAD_LEN" "$FIRST8"
        printf 'shell>\n'
        printf '# product halt: shell_exit=0\n'
    } >"$scratch/red-byte.log"
    if t26_check_log "$scratch/red-byte.log" "$PAYLOAD" >"$scratch/red-byte.out" 2>&1; then
        echo "# t26-selftest (g2) FAIL: log lệch MỘT byte mà checker nhận xanh"
        bad=1
    fi

    # (g3) CA ÂM 2: thiếu `# product halt:` (shell treo/không thoát) ⇒ phải ĐỎ.
    {
        printf '# product fs: medium=ram blocks=96 used=2 total=66 volatile=1\n'
        printf 'shell>\n'
        printf '# shell: write /t26 bytes=%s sync=ok\n' "$PAYLOAD_LEN"
        printf 'shell>\n'
        printf '%s\n' "$PAYLOAD"
        printf '# shell: cat ok bytes=%s first=%s\n' "$PAYLOAD_LEN" "$FIRST8"
    } >"$scratch/red-halt.log"
    if t26_check_log "$scratch/red-halt.log" "$PAYLOAD" >"$scratch/red-halt.out" 2>&1; then
        echo "# t26-selftest (g3) FAIL: log thiếu '# product halt:' mà checker nhận xanh"
        bad=1
    fi

    # (g4) CA ÂM 3: log có `# shell: ?` (lệnh lạ) ⇒ phải ĐỎ.
    {
        printf '# product fs: medium=ram blocks=96 used=2 total=66 volatile=1\n'
        printf 'shell>\n'
        printf '# shell: write /t26 bytes=%s sync=ok\n' "$PAYLOAD_LEN"
        printf 'shell>\n'
        printf '%s\n' "$PAYLOAD"
        printf '# shell: cat ok bytes=%s first=%s\n' "$PAYLOAD_LEN" "$FIRST8"
        printf '# shell: ?\n'
        printf '# product halt: shell_exit=0\n'
    } >"$scratch/red-cmd.log"
    if t26_check_log "$scratch/red-cmd.log" "$PAYLOAD" >"$scratch/red-cmd.out" 2>&1; then
        echo "# t26-selftest (g4) FAIL: log có '# shell: ?' mà checker nhận xanh"
        bad=1
    fi

    rm -rf "$scratch"
    [ "$bad" -eq 0 ] || rc=1
    if [ "$rc" -eq 0 ]; then
        echo "# t26-selftest: PASS (g1 xanh, g2/g3/g4 đỏ)"
    else
        echo "# t26-selftest: FAIL"
    fi
    return "$rc"
}

# ------------------------------------------------------------------ boot thật
kill_own_qemu() {
    local pg
    [ -n "${QG_REG:-}" ] && [ -f "$QG_REG" ] || return 0
    while IFS= read -r pg; do
        [ -n "$pg" ] || continue
        qg_kill_group "$pg"
    done <"$QG_REG"
    return 0
}

# AGENTS §9 / D2-134: tối đa 3 QEMU toàn máy, tối đa 1 QEMU của chính task này. Trước khi mở
# QEMU, đếm `qemu-system` toàn máy; ≥ 3 thì chờ 20 s rồi đếm lại (không `pkill`, không giết lạ).
wait_qemu_slot() {
    local n
    while :; do
        n="$(pgrep -f qemu-system 2>/dev/null | wc -l | tr -d ' ')"
        [ "${n:-0}" -lt 3 ] && return 0
        echo "# t26: cho QEMU (dang co $n >= 3) — sleep 20"
        sleep 20
    done
}

wait_re() {
    local re="$1" max="$2" i=0
    while [ "$i" -lt "$max" ]; do
        grep -qE "$re" "$LOG" 2>/dev/null && return 0
        kill -0 "$QGB_PID" 2>/dev/null || break
        sleep "$POLL_S"
        i=$((i + 1))
    done
    grep -qE "$re" "$LOG" 2>/dev/null
}

# boot_one <số> -> 0 = xanh, 1 = đỏ. In `# boot <n>: PASS/FAIL (lý do)`.
boot_one() {
    local index="$1" qemu="$2" sbi="$3" rc=0
    LOG="$LOGDIR/boot-$index.log"
    FIFO="$LOGDIR/boot-$index.fifo"
    rm -f "$LOG" "$FIFO" 2>/dev/null
    : >"$LOG" || die "không ghi được $LOG"
    mkfifo "$FIFO" || die "không tạo được FIFO $FIFO"
    exec 8<>"$FIFO" || die "không mở được FIFO $FIFO"
    export T26_FIFO="$FIFO"
    qg_boot "$LOG" "$QEMU_TDS" /bin/bash -c 'exec "$@" <"$T26_FIFO"' bash "$qemu" \
        -machine virt,aia=none,aclint=on -bios "$sbi" -kernel "$IMAGE" \
        -nographic -cpu rv64 -m 128M -smp 2 -no-reboot &
    QGB_PID=$!

    if ! wait_re '^shell>$' "$BOOT_POLLS"; then
        echo "# boot $index: FAIL (không thấy 'shell>' sau $BOOT_POLLS vòng poll)"
        rc=1
    else
        printf 'write /t26 %s\n' "$PAYLOAD" >&8
        if ! wait_re "^# shell: write /t26 bytes=$PAYLOAD_LEN sync=ok$" "$BOOT_POLLS"; then
            echo "# boot $index: FAIL (không thấy dòng write ok)"
            rc=1
        else
            printf 'cat /t26\n' >&8
            if ! wait_re "^# shell: cat ok bytes=$PAYLOAD_LEN first=$FIRST8$" "$BOOT_POLLS"; then
                echo "# boot $index: FAIL (không thấy dòng cat ok)"
                rc=1
            fi
        fi
        printf 'exit\n' >&8
        if ! wait_re '^# product halt:' "$BOOT_POLLS"; then
            echo "# boot $index: FAIL (không thấy '# product halt:')"
            rc=1
        fi
    fi
    exec 8>&-
    kill_own_qemu
    wait "$QGB_PID" 2>/dev/null || true
    QGB_PID=""

    if [ "$rc" -eq 0 ]; then
        if t26_check_log "$LOG" "$PAYLOAD" >"$LOGDIR/boot-$index.check" 2>&1; then
            echo "# boot $index: PASS ($(sed -n '1p' "$LOGDIR/boot-$index.check"))"
        else
            echo "# boot $index: FAIL ($(sed -n '1p' "$LOGDIR/boot-$index.check"))"
            rc=1
        fi
    fi
    return "$rc"
}

run_boots() {
    local nboots="$1" qemu sbi c i red=0 green=0
    [ -f "$IMAGE" ] || die "không thấy ảnh: $IMAGE"
    if ! vbrev_image_identity "$IMAGE"; then
        echo "T26-GATE: REFUSE reason=$VBREV_IMG_REASON image=$IMAGE algo=${VBREV_IMG_ALGO:-<none>} current_algo=$VBREV_ALGO" >&2
        exit 2
    fi
    echo "T26-GATE: ACCEPT image=$IMAGE algo=$VBREV_IMG_ALGO inputs=$VBREV_IMG_INPUTS dirty=$VBREV_IMG_DIRTY"

    qemu="$(command -v qemu-system-riscv64 2>/dev/null || true)"
    [ -n "$qemu" ] || die "không có qemu-system-riscv64"
    sbi="${OPENSBI:-}"
    if [ -z "$sbi" ]; then
        for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
            [ -f "$c" ] && {
                sbi="$c"
                break
            }
        done
    fi
    [ -f "${sbi:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

    mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"
    qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
    qg_infra before "$LOGDIR/infra.log" || true

    echo "# t26 KT-149 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    echo "# image: $IMAGE"
    echo "# payload: $PAYLOAD ($PAYLOAD_LEN B)"
    echo "# cấu hình: -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 2 -no-reboot (không icount)"
    echo "# log: $LOGDIR"

    i=1
    while [ "$i" -le "$nboots" ]; do
        wait_qemu_slot
        if boot_one "$i" "$qemu" "$sbi"; then
            green=$((green + 1))
        else
            red=$((red + 1))
        fi
        i=$((i + 1))
    done
    qg_infra after "$LOGDIR/infra.log" || true
    echo "# t26: boots=$nboots green=$green red=$red"
    [ "$red" -eq 0 ]
}

# ------------------------------------------------------------------ đối số
IMAGE=""
NBOOTS=50
MODE=""
while [ $# -gt 0 ]; do
    case "$1" in
        --check-image)
            [ "$#" -ge 2 ] || die "--check-image cần đường dẫn ảnh"
            MODE="check-image"
            IMAGE="$2"
            shift 2
            ;;
        --image)
            [ "$#" -ge 2 ] || die "--image cần đường dẫn ảnh"
            MODE="boots"
            IMAGE="$2"
            shift 2
            ;;
        --boots)
            [ "$#" -ge 2 ] || die "--boots cần số"
            NBOOTS="$2"
            shift 2
            ;;
        --selftest)
            MODE="selftest"
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) die "tham số không biết: $1" ;;
    esac
done

case "$NBOOTS" in '' | *[!0-9]*) die "--boots phải là số nguyên dương" ;; esac
[ "$NBOOTS" -ge 1 ] || die "--boots phải >= 1"

case "$MODE" in
    check-image)
        if vbrev_image_identity "$IMAGE"; then
            echo "T26-GATE: ACCEPT image=$IMAGE algo=$VBREV_IMG_ALGO inputs=$VBREV_IMG_INPUTS dirty=$VBREV_IMG_DIRTY"
            exit 0
        fi
        echo "T26-GATE: REFUSE reason=$VBREV_IMG_REASON image=$IMAGE algo=${VBREV_IMG_ALGO:-<none>} current_algo=$VBREV_ALGO"
        exit 1
        ;;
    boots) run_boots "$NBOOTS" ;;
    selftest) t26_selftest ;;
    *)
        usage >&2
        exit 2
        ;;
esac
