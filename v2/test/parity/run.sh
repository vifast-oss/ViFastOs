#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-77a — chạy kịch bản serial của `spec/v1-parity.toml` trên QEMU riscv64, ghi TAP.
#
# Hai chế độ (đo được, không retry để xanh):
#   --mode transcript  (mặc định) boot ảnh **một** lần, đối chiếu đầu ra shell với `expect` của
#                      từng lệnh `active`. Nguồn byte vào shell là **script nội bộ của kernel**
#                      (`proc_demo::build_script`, `v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:270`),
#                      nên chế độ này đo "shell có lệnh + in đúng dạng", KHÔNG đo đường serial ngoài.
#   --mode serial      mỗi lệnh `active` một lần boot; byte của kịch bản được bơm vào **stdin QEMU**
#                      qua `stdin_wrap.sh` (FIFO + trễ 50 ms). Attribution theo `serial_min_count`:
#                      `help`/`ps` phải xuất hiện ≥2 lần (kernel cũng tự chạy một lần), `echo`/`kill`
#                      dùng chuỗi đặc trưng (K77A / pid 999). Đây là chế độ sẽ dùng cho ảnh sản phẩm
#                      ở KT-77b.
#
# Ca âm (D2-108B):
#   --negative wrong-expect  gửi `zzz` (lệnh sai) với expect không thể khớp ⇒ shell in `# shell: ?`
#                            ⇒ `not ok`, exit 3, có hạn thời gian (không treo).
#   --negative no-send       KHÔNG gửi byte nào ⇒ expect của kịch bản không xuất hiện đủ số lần
#                            ⇒ `timeout`, exit 4.
#   --image product          ảnh sản phẩm **chưa có shell** ⇒ không có `# shell: up` ⇒ mọi mục
#                            `not ok` + exit 5 (ca âm "shell không in gì").
#
# Exit: 0 = mọi mục active OK; 2 = lỗi cấu hình/dùng; 3 = có mục không khớp (`not ok`);
#       4 = timeout (không thấy đầu ra của kịch bản trong hạn); 5 = không thấy shell trong log.
#
# D2-116/OD-K6-6: mọi boot đi qua `v2/test/boot/qemu_guard.sh` **không sửa** (process group riêng,
# timeout cứng, dọn khi cha chết) + `qg_infra` trước/sau + đếm QEMU còn sót cuối script.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
TOML="$ROOT/spec/v1-parity.toml"
BOOT_DIR="$V2_ROOT/test/boot"
GUARD="$BOOT_DIR/qemu_guard.sh"
WRAP="$HERE/stdin_wrap.sh"
TARGET="riscv64gc-unknown-none-elf"
IMG_DIR="$V2_ROOT/kernel/target/$TARGET/release"
LOGDIR="${KT77_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/kt77a-parity}"
MODE=transcript
IMAGE_ARG=ktest-proc
SMP=1
TIMEOUT_DS=200 # 20 s/boot (cùng ngưỡng harness D2-72 Q10)
SEND_DELAY_MS=50
SUFFIX=""
NEG=""
EXTRA=""
EXTRA_ONLY=0

usage() {
    cat <<'EOF'
Dùng: bash v2/test/parity/run.sh [--mode transcript|serial] [--image ktest-proc|product|<path>]
                                 [--negative wrong-expect|no-send] [--smp N]
                                 [--send-delay-ms N] [--timeout-ds N] [--logdir DIR] [--log-suffix S]
                                 [--extra TEP-TOML] [--extra-only]  # KT-343: bổ sung/lọc mục active (tuỳ chọn)
EOF
}

die() {
    echo "run.sh: $*" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) MODE="${2:-}"; shift 2 ;;
        --image) IMAGE_ARG="${2:-}"; shift 2 ;;
        --negative) NEG="${2:-}"; shift 2 ;;
        --smp) SMP="${2:-}"; shift 2 ;;
        --send-delay-ms) SEND_DELAY_MS="${2:-}"; shift 2 ;;
        --timeout-ds) TIMEOUT_DS="${2:-}"; shift 2 ;;
        --logdir) LOGDIR="${2:-}"; shift 2 ;;
        --log-suffix) SUFFIX="${2:-}"; shift 2 ;;
        --extra) EXTRA="${2:-}"; shift 2 ;;
        --extra-only) EXTRA_ONLY=1; shift ;;
        -h | --help) usage; exit 0 ;;
        *) die "tham số lạ: $1" ;;
    esac
done
case "$MODE" in transcript | serial) ;; *) die "--mode phải là transcript|serial" ;; esac
case "$NEG" in '' | wrong-expect | no-send) ;; *) die "--negative phải là wrong-expect|no-send" ;; esac
[ -f "$TOML" ] || die "không thấy $TOML"
if [ -n "$EXTRA" ]; then
    [ -f "$EXTRA" ] || die "không thấy tệp --extra $EXTRA"
fi
[ -f "$GUARD" ] || die "không thấy $GUARD (KT-78)"
[ -x "$WRAP" ] || die "không thấy $WRAP (helper của KT-77a)"
case "$SMP" in '' | *[!0-9]*) die "--smp phải là số" ;; esac
case "$SEND_DELAY_MS" in '' | *[!0-9]*) die "--send-delay-ms phải là số (ms)" ;; esac
case "$TIMEOUT_DS" in '' | *[!0-9]*) die "--timeout-ds phải là số (deciseconds)" ;; esac

case "$IMAGE_ARG" in
    ktest-proc) IMAGE="$IMG_DIR/vifast-kbin-ktest-proc"; IMAGE_KIND=ktest-proc ;;
    product) IMAGE="$IMG_DIR/vifast-kbin-riscv64"; IMAGE_KIND=product ;;
    *) IMAGE="$IMAGE_ARG"; IMAGE_KIND=custom ;;
esac
[ -x "$IMAGE" ] || die "không thấy ảnh $IMAGE (build bằng: bash v2/test/boot/ktest.sh 1 1)"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"
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
[ -f "${SBI:-/nonexistent}" ] || die "không thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

mkdir -p "$LOGDIR" 2>/dev/null || die "không tạo được logdir $LOGDIR"
TAG="$(printf '%s-%s-smp%s' "$MODE" "$IMAGE_KIND" "$SMP")"
[ -n "$NEG" ] && TAG="$TAG-neg-$NEG"
TAG="$TAG$SUFFIX"
LOG="$LOGDIR/kt77a-$TAG.log"
QMULOG="$LOGDIR/kt77a-qemu-$TAG.log"
INFRA_LOG="$LOGDIR/kt77a-infra.log"
: >"$LOG" 2>/dev/null || die "không ghi được $LOG"

# D2-116: boot đi qua guard của KT-78, **không sửa** tệp đó.
. "$GUARD"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/kt77a-guard-$TAG.log"
rm -f "$QG_LOG" 2>/dev/null
qg_init || die "qemu_guard: không khởi tạo được (OD-K6-6)"

note() { printf '%s\n' "$*" | tee -a "$LOG"; }

note "# run.sh: task=KT-77a mode=$MODE image=$IMAGE_KIND smp=$SMP negative=${NEG:-none}"
note "# run.sh: toml=$TOML"
note "# run.sh: image=$IMAGE qemu=$QEMU opensbi=$SBI"
note "# run.sh: guard=$GUARD (khong sua) boot_timeout=$((TIMEOUT_DS / 10))s send_delay_ms=$SEND_DELAY_MS"
IMG_SHA="$(shasum -a 256 "$IMAGE" 2>/dev/null | awk '{print $1}')"
IMG_BYTES="$(wc -c <"$IMAGE" 2>/dev/null | tr -d ' ')"
IMG_FROZEN=0
if [ -f "$ROOT/artifacts/$IMG_SHA.bin" ]; then IMG_FROZEN=1; fi
note "# parity: image_path=$IMAGE image_sha256=$IMG_SHA image_bytes=$IMG_BYTES"
note "# parity: artifacts_bin=artifacts/$IMG_SHA.bin co=$IMG_FROZEN (dong bang artifacts/<sha>.bin thuoc KT-84)"

qg_infra before "$INFRA_LOG" | tee -a "$LOG"

# ---------------------------------------------------------------- đọc spec (awk: TOML một tầng)
# Tệp do KT-77a viết theo dạng cố định: khoá = giá trị **một dòng**, chuỗi literal '...'.
# Máy này python3 3.9 không có `tomllib` nên không dùng parser TOML ngoài.
PARSED="$LOGDIR/.parsed.$$"
awk '
function flush() { if (tx == "") tx = expect; if (name != "") printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n", name, send, expect, tms, mc, status, tx }
/^\[\[command\]\]$/ { flush(); name=""; send=""; expect=""; tms="2000"; mc="1"; status=""; tx=""; next }
/^[a-z_]+ *= / {
    k=$0; sub(/ *= .*$/, "", k);
    v=$0; sub(/^[a-z_]+ *= /, "", v);
    if (v ~ /^\047.*\047$/) v=substr(v,2,length(v)-2);
    if (k=="name") name=v; else if (k=="send") send=v; else if (k=="expect") expect=v;
    else if (k=="timeout_ms") tms=v; else if (k=="serial_min_count") mc=v;
    else if (k=="status") status=v; else if (k=="transcript_expect") tx=v;
}
END { flush() }
' "$TOML" >"$PARSED" || die "không đọc được $TOML"
SPEC_TOTAL="$(wc -l <"$PARSED" | tr -d ' ')"
[ "$SPEC_TOTAL" -gt 0 ] || die "spec không có mục nào"
ACTIVE="$(awk -F'\t' '$6=="active"' "$PARSED" | wc -l | tr -d ' ')"
DEFERRED="$(awk -F'\t' '$6=="deferred-K7"' "$PARSED" | wc -l | tr -d ' ')"
[ $((ACTIVE + DEFERRED)) -eq "$SPEC_TOTAL" ] || die "spec có status lạ (tổng=$SPEC_TOTAL active=$ACTIVE deferred=$DEFERRED)"
# **KT-343** — `--extra`: tệp TOML **cùng schema** để bổ sung mục active (dùng khi cần đo riêng
# một nhóm mà không sửa `spec/`); `--extra-only` chỉ chạy nhóm đó. Mọi mục extra mặc định `active`.
EXTRA_TOTAL=0
if [ -n "$EXTRA" ]; then
    PARSED_EXTRA="$LOGDIR/.parsed-extra.$$"
    awk '
    function flush() { if (tx == "") tx = expect; if (name != "") printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n", name, send, expect, tms, mc, (status == "" ? "active" : status), tx }
    /^\[\[command\]\]$/ { flush(); name=""; send=""; expect=""; tms="2000"; mc="1"; status=""; tx=""; next }
    /^[a-z_]+ *= / {
        k=$0; sub(/ *= .*$/, "", k);
        v=$0; sub(/^[a-z_]+ *= /, "", v);
        if (v ~ /^\047.*\047$/) v=substr(v,2,length(v)-2);
        if (k=="name") name=v; else if (k=="send") send=v; else if (k=="expect") expect=v;
        else if (k=="timeout_ms") tms=v; else if (k=="serial_min_count") mc=v;
        else if (k=="status") status=v; else if (k=="transcript_expect") tx=v;
    }
    END { flush() }
    ' "$EXTRA" >"$PARSED_EXTRA" || die "không đọc được $EXTRA"
    EXTRA_TOTAL="$(wc -l <"$PARSED_EXTRA" | tr -d ' ')"
    [ "$EXTRA_TOTAL" -gt 0 ] || die "--extra không có mục nào"
    cat "$PARSED_EXTRA" >>"$PARSED"
fi
if [ "$EXTRA_ONLY" = 1 ]; then
    [ -n "$EXTRA" ] || die "--extra-only cần --extra TEP-TOML"
    cp "$PARSED_EXTRA" "$PARSED" || die "không chép được parsed-extra"
    SPEC_TOTAL=0
    ACTIVE=0
    DEFERRED=0
fi
TOTAL=$((SPEC_TOTAL + EXTRA_TOTAL))
ACTIVE=$((ACTIVE + EXTRA_TOTAL))
[ "$TOTAL" -gt 0 ] || die "không có mục nào"
note "# parity: spec_total=$SPEC_TOTAL spec_active=$((ACTIVE - EXTRA_TOTAL)) deferred=$DEFERRED extra=$EXTRA_TOTAL total=$TOTAL (nguon: $TOML${EXTRA:+, extra=$EXTRA})"

QARGS=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" -nographic \
    -cpu rv64 -m 128M -smp "$SMP" -no-reboot)

# ---------------------------------------------------------------- một boot (transcript)
boot_once() { # $1=log đích
    qg_boot "$1" "$TIMEOUT_DS" "$QEMU" "${QARGS[@]}"
}

# ---------------------------------------------------------------- một boot + bơm byte (serial)
# Guard khởi động `stdin_wrap.sh` (tệp thực thi) trong process group riêng; wrapper `exec` QEMU
# với stdin = FIFO và bơm payload sau `SEND_DELAY_MS`. Guard không bị sửa.
RC=0
boot_send() { # $1=log đích, $2=nội dung payload
    PAY="$LOGDIR/.payload.$$"
    printf '%s' "$2" >"$PAY" || die "không ghi được payload $PAY"
    KT77_SEND_DELAY="$(awk -v ms="$SEND_DELAY_MS" 'BEGIN { printf "%.3f", ms / 1000 }')" \
        qg_boot "$1" "$TIMEOUT_DS" "$WRAP" "$PAY" "$QEMU" "${QARGS[@]}"
    RC=$?
    rm -f "$PAY" 2>/dev/null
}

shell_up() { grep -q '^# shell: up ' "$1"; }
count_re() { grep -Ec "$1" "$2" 2>/dev/null || true; }

# ---------------------------------------------------------------- chạy
fails=0
timeouts=0
noshell=0
n_ok=0
n_active_seen=0
id=0

if [ "$MODE" = transcript ]; then
    note "# parity: mode=transcript input=kernel-script const:proc_demo.rs:270 (khong do duong serial ngoai)"
    boot_once "$QMULOG"
    RC=$?
    note "# parity: qemu_rc=$RC log=$QMULOG"
    cat "$QMULOG" >>"$LOG"
    exec 3<"$PARSED" || die "không mở được $PARSED"
    if shell_up "$QMULOG" && [ "$RC" -eq 0 ]; then
        while IFS=$'\t' read -r name send expect tms mc status tx <&3; do
            [ "$status" = active ] || continue
            [ -n "$tx" ] || tx="$expect"
            id=$((id + 1))
            n_active_seen=$((n_active_seen + 1))
            if grep -Eq "$tx" "$QMULOG"; then
                note "ok $id - $name"
                n_ok=$((n_ok + 1))
            else
                note "not ok $id - $name: expect khong khop (${tx})"
                fails=$((fails + 1))
            fi
        done
    else
        noshell=$((noshell + 1))
        while IFS=$'\t' read -r name send expect tms mc status tx <&3; do
            [ "$status" = active ] || continue
            id=$((id + 1))
            n_active_seen=$((n_active_seen + 1))
            note "not ok $id - $name: khong thay shell trong log (qemu_rc=$RC)"
            fails=$((fails + 1))
        done
    fi
    exec 3<&-
else
    exec 3<"$PARSED" || die "không mở được $PARSED"
    while IFS=$'\t' read -r name send expect tms mc status tx <&3; do
        [ "$status" = active ] || continue
        id=$((id + 1))
        n_active_seen=$((n_active_seen + 1))
        send_dec="${send//\\n/$'\n'}"
        if [ "$NEG" = no-send ]; then
            payload=""
        elif [ "$NEG" = wrong-expect ]; then
            # Dòng hợp lệ ngắn (chứng minh byte đã vào shell) + dòng sai ⇒ shell in "# shell: ?".
            payload="echo K77A"$'\n'"zzz"$'\n'
        else
            payload="$send_dec"
        fi
        if [ "$NEG" = wrong-expect ]; then
            expect_chk='^# shell: echo KHONG-BAO-GIO$' # không thể khớp
        else
            expect_chk="$expect"
        fi
        slog="$LOGDIR/kt77a-serial-$id-$name-$TAG.log"
        boot_send "$slog" "$payload"
        rc="$RC"
        cat "$slog" >>"$LOG"
        note "# parity: serial name=$name payload_bytes=$(printf '%s' "$payload" | wc -c | tr -d ' ') min_count=$mc qemu_rc=$rc log=$slog"
        if ! shell_up "$slog"; then
            note "not ok $id - $name: khong thay shell trong log (qemu_rc=$rc)"
            fails=$((fails + 1))
            noshell=$((noshell + 1))
            continue
        fi
        ok=0
        got=0
        waited=0
        while :; do
            got="$(count_re "$expect_chk" "$slog")"
            case "$got" in '' | *[!0-9]*) got=0 ;; esac
            if [ "$got" -ge "$mc" ]; then
                ok=1
                break
            fi
            [ "$waited" -ge "$tms" ] && break
            sleep 0.05
            waited=$((waited + 50))
        done
        if [ "$ok" = 1 ]; then
            note "ok $id - $name (khop=$got >= $mc, waited_ms=$waited)"
            n_ok=$((n_ok + 1))
            continue
        fi
        # Phân loại ca âm/không đạt: có dấu hiệu byte của ta vào shell chưa?
        # (a) shell in "# shell: ?" cho dòng sai, hoặc (b) dòng hợp lệ đi kèm đã chạy.
        reject="$(count_re '^# shell: \?$' "$slog")"
        case "$reject" in '' | *[!0-9]*) reject=0 ;; esac
        if grep -q '^# shell: echo K77A$' "$slog"; then
            reject=$((reject + 1))
        fi
        if [ "$reject" -gt 0 ]; then
            note "not ok $id - $name: byte da vao shell (co ${reject} dong 'shell: ?') nhung expect khong khop sau ${tms} ms (${expect_chk})"
            fails=$((fails + 1))
        else
            note "not ok $id - $name: timeout sau ${tms} ms — expect khong xuat hien du ${mc} lan (${expect_chk})"
            timeouts=$((timeouts + 1))
            fails=$((fails + 1))
        fi
    done
    exec 3<&-
fi

note "# parity: total=$TOTAL ok=$n_ok deferred=$DEFERRED active=$ACTIVE spec_total=$SPEC_TOTAL extra=$EXTRA_TOTAL (active_da_chay=$n_active_seen)"
note "# parity: not_ok=$fails timeout=$timeouts khong_co_shell=$noshell qemu_rc=$RC"

qg_infra after "$INFRA_LOG" | tee -a "$LOG"
# D2-116(a): đếm QEMU còn sống sau khi script dọn (chỉ của ta; không `pkill`).
live="$(ps -ax -o pid,ppid,command 2>/dev/null | grep -c '[q]emu-system' || true)"
own="$(qg_own)"
note "# parity: qemu_con_sot_total=${live:-0} qemu_cua_script=${own:-0}"
rm -f "$PARSED" 2>/dev/null

if [ "$noshell" -gt 0 ]; then
    note "# parity: KET_LUAN=khong_co_shell (exit 5)"
    exit 5
fi
if [ "$timeouts" -gt 0 ]; then
    note "# parity: KET_LUAN=timeout (exit 4)"
    exit 4
fi
if [ "$fails" -gt 0 ]; then
    note "# parity: KET_LUAN=not_ok (exit 3)"
    exit 3
fi
note "# parity: KET_LUAN=ok (exit 0)"
exit 0
