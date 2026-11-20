#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-242 — **ba mức L1** cho driver `l1_k1c`, chạy **chia lô** trên nhiều tiến trình.
#
# Vì sao có công cụ này (số đo: `v2/agent/JOURNAL/kt242-l1-profile.md`):
#   ~92 % thời gian L1 là **kiểm bất biến** (mô hình tự kiểm sau mỗi public op + driver kiểm
#   sau mỗi op), chỉ ~8 % là mô phỏng thật. Nên **không** có chỗ nào để "tối ưu" mà **không**
#   nới ngưỡng — cách duy nhất giảm thời gian **wall** là chạy **song song**. Công cụ này chia
#   tập seed thành lô (`VIFAST_SHARD=i/n`) và chạy nhiều tiến trình, **không** đổi seed, **không**
#   đổi `SO_OP`, **không** đổi bất biến, **không** đổi ngưỡng.
#
# Ba mức (A0 duyệt, KT-242):
#   smoke    1.000 seed   × smp 1            — kiểm nhanh sau khi sửa driver
#   medium  10.000 seed   × smp 1,2,4,8      — kiểm trước khi báo cáo
#   full   100.000 seed   × smp 1,2,4,8      — hợp đồng K6: 4 × 100k = **400k seed**
#
# **KHÔNG** đổi mặc định của `gate --fast`: cổng vẫn chạy L1 1.000 seed của `tests/l1.rs`
# (`l1_1000_seed_tat_dinh`), **không** gọi driver `l1_k1c` và **không** chạy mức nào ở đây.
# `v2 check` chỉ gọi `--selftest` (27 ca, < 1 s, **không** chạy sim) — đúng luật KT-134 "script
# tự kiểm phải có cổng gọi". Còn **chạy** một mức thì **không** nằm trong cổng nào: nó chạy hàng
# chục tiến trình, phải chạy **một mình** (AGENTS.md §6: không chạy song song với cổng khác).
#
# Dùng:
#   bash v2/tools/l1-tier.sh --tier smoke
#   bash v2/tools/l1-tier.sh --tier medium --jobs 8
#   bash v2/tools/l1-tier.sh --tier full --logdir /tmp/l1-full
#   bash v2/tools/l1-tier.sh --tier smoke --dry-run      # in kế hoạch, không chạy
#   bash v2/tools/l1-tier.sh --selftest                  # tự kiểm 4 nhóm ca (không chạy sim)
#
# Exit: 0 = mọi lô xanh **và** hợp các lô phủ đúng tập seed; 1 = có lô đỏ / phủ sai;
#       2 = dùng sai; 127 = không tìm thấy cargo.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
MANIFEST="$V2_ROOT/Cargo.toml"

die() {
    echo "l1-tier: $*" >&2
    exit 2
}

# ---------------------------------------------------------------------------
# Bảng ba mức — **một** nơi định nghĩa (script là chủ sở hữu; driver chỉ nhận env).
# ---------------------------------------------------------------------------
tier_seeds() {
    case "${1:-}" in
        smoke) echo 1000 ;;
        medium) echo 10000 ;;
        full) echo 100000 ;;
        *) return 1 ;;
    esac
}

tier_smp() {
    case "${1:-}" in
        smoke) echo "1" ;;
        medium | full) echo "1,2,4,8" ;;
        *) return 1 ;;
    esac
}

tier_mo_ta() {
    case "${1:-}" in
        smoke) echo "kiểm nhanh sau khi sửa driver" ;;
        medium) echo "kiểm trước khi báo cáo" ;;
        full) echo "hợp đồng K6: 4 × 100k = 400k seed" ;;
        *) return 1 ;;
    esac
}

# Số seed của lô `i/n` trong `0..seeds` — **cùng công thức** với `so_seed_cua_lo` của driver
# (`v2/sim/vifast-sim/tests/l1_k1c.rs`). Lệch công thức ⇒ tổng phủ sai ⇒ script đỏ.
so_seed_cua_lo() {
    local seeds="$1" i="$2" n="$3"
    if [ "$i" -ge "$seeds" ]; then
        echo 0
    else
        echo $(((seeds - i + n - 1) / n))
    fi
}

so_core() {
    local n
    n="$( (sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 2) | head -1)"
    case "$n" in
        '' | *[!0-9]*) n=2 ;;
    esac
    echo "$n"
}

# AGENTS.md §9: sim dùng tối đa `số core − 1`.
jobs_mac_dinh() {
    local c
    c="$(so_core)"
    if [ "$c" -le 1 ]; then
        echo 1
    else
        echo $((c - 1))
    fi
}

# ---------------------------------------------------------------------------
# Tự kiểm (không chạy sim) — 4 nhóm ca.
# ---------------------------------------------------------------------------
selftest() {
    local fails=0 ca=0
    kiem() { # kiem <mô tả> <thực> <mong>
        ca=$((ca + 1))
        if [ "$2" = "$3" ]; then
            echo "  ok   $1: $2"
        else
            echo "  FAIL $1: nhận '$2', mong '$3'" >&2
            fails=$((fails + 1))
        fi
    }

    echo "== nhóm 1: bảng ba mức =="
    kiem "smoke seeds" "$(tier_seeds smoke)" "1000"
    kiem "medium seeds" "$(tier_seeds medium)" "10000"
    kiem "full seeds" "$(tier_seeds full)" "100000"
    kiem "smoke smp" "$(tier_smp smoke)" "1"
    kiem "medium smp" "$(tier_smp medium)" "1,2,4,8"
    kiem "full smp" "$(tier_smp full)" "1,2,4,8"
    kiem "mức lạ bị từ chối" "$(tier_seeds khong_co >/dev/null 2>&1; echo $?)" "1"

    echo "== nhóm 2: chia lô phủ đúng tập seed (không sót, không trùng) =="
    local seeds i n tong
    for seeds in 1000 10000 100000; do
        for n in 1 3 6 22; do
            tong=0
            i=0
            while [ "$i" -lt "$n" ]; do
                tong=$((tong + $(so_seed_cua_lo "$seeds" "$i" "$n")))
                i=$((i + 1))
            done
            kiem "seeds=$seeds n=$n tổng phủ" "$tong" "$seeds"
        done
    done
    kiem "lô rỗng khi i >= seeds" "$(so_seed_cua_lo 10 10 4)" "0"
    kiem "lô 3/4 của 10 seed (3, 7)" "$(so_seed_cua_lo 10 3 4)" "2"

    echo "== nhóm 3: digest XOR gộp được (giao hoán + kết hợp) =="
    # XOR của các lô phải bằng digest chạy một tiến trình — kiểm bằng giá trị tổng hợp.
    local a b c d full
    a=$((0x1111)); b=$((0x2222)); c=$((0x4444)); d=$((0x8888))
    full=$((a ^ b ^ c ^ d))
    kiem "XOR 4 lô (thứ tự xuôi)" "$((a ^ b ^ c ^ d))" "$full"
    kiem "XOR 4 lô (thứ tự đảo)" "$((d ^ c ^ b ^ a))" "$full"
    kiem "XOR với 0 không đổi" "$((full ^ 0))" "$full"
    kiem "XOR hai lần cùng lô = 0" "$((a ^ a))" "0"

    echo "== nhóm 4: clamp jobs =="
    kiem "jobs mặc định >= 1" "$([ "$(jobs_mac_dinh)" -ge 1 ] && echo yes)" "yes"
    kiem "jobs mặc định <= core-1" "$([ "$(jobs_mac_dinh)" -le "$(($(so_core) - 1))" ] && echo yes)" "yes"

    echo
    if [ "$fails" -eq 0 ]; then
        echo "l1-tier --selftest: PASS ($ca/$ca)"
        return 0
    fi
    echo "l1-tier --selftest: FAIL ($fails/$ca)" >&2
    return 1
}

# ---------------------------------------------------------------------------
# Tham số
# ---------------------------------------------------------------------------
TIER="smoke"
JOBS=""
SMP=""
SEEDS=""
LOGDIR=""
DRY=0
DIGEST=1

while [ $# -gt 0 ]; do
    case "$1" in
        --tier)
            [ $# -ge 2 ] || die "--tier cần giá trị (smoke|medium|full)"
            TIER="$2"
            shift 2
            ;;
        --jobs)
            [ $# -ge 2 ] || die "--jobs cần giá trị"
            JOBS="$2"
            shift 2
            ;;
        --smp)
            [ $# -ge 2 ] || die "--smp cần giá trị (ví dụ 1,2,4,8)"
            SMP="$2"
            shift 2
            ;;
        --seeds)
            [ $# -ge 2 ] || die "--seeds cần giá trị"
            SEEDS="$2"
            shift 2
            ;;
        --logdir)
            [ $# -ge 2 ] || die "--logdir cần giá trị"
            LOGDIR="$2"
            shift 2
            ;;
        --no-digest)
            DIGEST=0
            shift
            ;;
        --dry-run)
            DRY=1
            shift
            ;;
        --selftest)
            selftest
            exit $?
            ;;
        -h | --help)
            sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            die "tham số lạ '$1' (nhận: --tier, --jobs, --smp, --seeds, --logdir, --no-digest, --dry-run, --selftest)"
            ;;
    esac
done

[ -n "$(tier_seeds "$TIER" || true)" ] || die "mức '$TIER' không hợp lệ (nhận: smoke, medium, full)"
[ -n "$SEEDS" ] || SEEDS="$(tier_seeds "$TIER")"
[ -n "$SMP" ] || SMP="$(tier_smp "$TIER")"
[ -n "$JOBS" ] || JOBS="$(jobs_mac_dinh)"

case "$SEEDS" in
    '' | *[!0-9]*) die "--seeds phải là số dương (nhận '$SEEDS')" ;;
esac
[ "$SEEDS" -gt 0 ] || die "--seeds phải dương"
case "$JOBS" in
    '' | *[!0-9]*) die "--jobs phải là số dương (nhận '$JOBS')" ;;
esac
[ "$JOBS" -gt 0 ] || die "--jobs phải dương"
case "$SMP" in
    '' | *[!0-9,]*) die "--smp phải là danh sách số cách nhau bởi dấu phẩy (nhận '$SMP')" ;;
esac

N_SMP="$(printf '%s' "$SMP" | tr ',' '\n' | grep -c .)"
[ "$N_SMP" -gt 0 ] || die "--smp rỗng"

# Số lô mỗi cấu hình smp: đủ để lấp `JOBS` worker và có dư để cân bằng tải (mỗi cấu hình smp
# có chi phí khác nhau: smp 8 chậm hơn smp 1 ~1,4×). Không vượt quá số seed.
K=$(((2 * JOBS + N_SMP - 1) / N_SMP))
[ "$K" -lt 1 ] && K=1
[ "$K" -gt "$SEEDS" ] && K="$SEEDS"

TONG_TASK=$((N_SMP * K))

echo "== l1-tier: mức $TIER ($(tier_mo_ta "$TIER")) =="
echo "   seeds=$SEEDS smp=$SMP jobs=$JOBS lô/smp=$K tổng_task=$TONG_TASK digest=$DIGEST"
echo "   core=$(so_core) (AGENTS.md §9: sim dùng tối đa core-1)"

if [ "$DRY" -eq 1 ]; then
    echo "   --dry-run: kế hoạch"
    for s in $(printf '%s' "$SMP" | tr ',' ' '); do
        i=0
        while [ "$i" -lt "$K" ]; do
            echo "     VIFAST_SEEDS=$SEEDS VIFAST_SMP=$s VIFAST_SHARD=$i/$K"
            i=$((i + 1))
        done
    done
    exit 0
fi

# ---------------------------------------------------------------------------
# Tìm cargo + ghim toolchain (cùng cách `v2/bin/v2` — KT-190)
# ---------------------------------------------------------------------------
find_cargo() {
    if command -v cargo >/dev/null 2>&1; then
        command -v cargo
        return 0
    fi
    local c
    for c in "$HOME/.cargo/bin/cargo" "/usr/local/cargo/bin/cargo" "/opt/homebrew/bin/cargo"; do
        if [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    done
    return 1
}

CARGO="$(find_cargo)" || {
    echo "l1-tier: không tìm thấy cargo (đã thử PATH, ~/.cargo/bin, /usr/local/cargo/bin, /opt/homebrew/bin)" >&2
    exit 127
}
export PATH="$(dirname "$CARGO"):$PATH"

if [ -z "${RUSTUP_TOOLCHAIN:-}" ] && [ -f "$V2_ROOT/rust-toolchain.toml" ]; then
    _tc="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$V2_ROOT/rust-toolchain.toml" | head -1)"
    if [ -n "$_tc" ]; then
        export RUSTUP_TOOLCHAIN="$_tc"
    fi
fi

# ---------------------------------------------------------------------------
# Build **một lần** rồi chạy binary trực tiếp: nhiều tiến trình `cargo test` song song sẽ
# tranh khoá target-dir và làm sai số đo.
# ---------------------------------------------------------------------------
echo "   build: cargo test -p vifast-sim --test l1_k1c --no-run"
BUILD_OUT="$("$CARGO" test --manifest-path "$MANIFEST" -p vifast-sim --test l1_k1c --no-run --message-format=json 2>/dev/null)" || {
    echo "l1-tier: build đỏ" >&2
    exit 1
}
# Chỉ lấy artifact của **test target** `l1_k1c` — `--message-format=json` cũng phát artifact
# của lib và của bin `vifast-sim`; lấy nhầm bin thì mọi lô chết ngay với "tham số lạ --nocapture".
BIN="$(printf '%s\n' "$BUILD_OUT" | grep '"name":"l1_k1c"' | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | grep -v '^$' | tail -1)"
[ -n "$BIN" ] && [ -x "$BIN" ] || {
    echo "l1-tier: không tìm thấy binary test 'l1_k1c' sau khi build" >&2
    exit 1
}
# Fail-closed: binary phải thật là test binary của driver (có tên test), không phải bin khác.
"$BIN" --list 2>/dev/null | grep -q 'driver_smp_cung_seed_cung_trace' || {
    echo "l1-tier: '$BIN' không phải test binary của driver l1_k1c" >&2
    exit 1
}
echo "   bin: $BIN"

if [ -z "$LOGDIR" ]; then
    LOGDIR="$REPO_ROOT/target-task/kt242-l1/$TIER-$(date +%Y%m%d-%H%M%S)"
fi
mkdir -p "$LOGDIR" || die "không tạo được logdir '$LOGDIR'"
echo "   log: $LOGDIR"

# ---------------------------------------------------------------------------
# Chạy pool (bash 3.2: không có `wait -n`, throttle bằng `jobs -pr`)
# ---------------------------------------------------------------------------
T0="$(date +%s)"
for s in $(printf '%s' "$SMP" | tr ',' ' '); do
    i=0
    while [ "$i" -lt "$K" ]; do
        while [ "$(jobs -pr | wc -l | tr -d ' ')" -ge "$JOBS" ]; do
            sleep 0.2
        done
        (
            log="$LOGDIR/smp${s}-lo${i}cua${K}.log"
            if [ "$DIGEST" -eq 1 ]; then
                VIFAST_L1_DIGEST=1 VIFAST_SEEDS="$SEEDS" VIFAST_SMP="$s" VIFAST_SHARD="$i/$K" \
                    "$BIN" --nocapture >"$log" 2>&1
            else
                VIFAST_SEEDS="$SEEDS" VIFAST_SMP="$s" VIFAST_SHARD="$i/$K" \
                    "$BIN" --nocapture >"$log" 2>&1
            fi
            echo $? >"$LOGDIR/smp${s}-lo${i}cua${K}.rc"
        ) &
        i=$((i + 1))
    done
done
wait
T1="$(date +%s)"
WALL=$((T1 - T0))

# ---------------------------------------------------------------------------
# Gộp + kiểm phủ
# ---------------------------------------------------------------------------
python3 - "$LOGDIR" "$SEEDS" "$SMP" "$K" "$WALL" <<'PY'
import os
import re
import sys

logdir, seeds, smp_list, k, wall = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
smps = [s for s in smp_list.split(",") if s]

ok_re = re.compile(
    r"^# L1 OK: smp (\d+), (\d+) seed × (\d+) op \(.*?ipc_min (\d+), ipc_median (\d+); "
    r"ipc_sent (\d+), ipc_delivered (\d+), ipc_dropped (\d+); "
    r"vm map (\d+), walk (\d+), unmap (\d+), fault (\d+), shootdown (\d+)\)"
)
dig_re = re.compile(r"^# L1 DIGEST: smp (\d+) shard (\d+)/(\d+) seeds (\d+) digest ([0-9a-f]{16})")

agg = {}
digest = {}
fails = []
for s in smps:
    agg[s] = dict(seeds=0, sent=0, deliv=0, drop=0, map=0, walk=0, unmap=0, fault=0, shoot=0, mn=None)
    digest[s] = 0

for name in sorted(os.listdir(logdir)):
    if not name.endswith(".log"):
        continue
    path = os.path.join(logdir, name)
    text = open(path, encoding="utf-8", errors="replace").read()
    if "test result: ok" not in text:
        fails.append(f"{name}: không có 'test result: ok'")
        continue
    m = None
    for line in text.splitlines():
        mm = ok_re.match(line)
        if mm:
            m = mm
        dm = dig_re.match(line)
        if dm:
            digest[dm.group(1)] ^= int(dm.group(5), 16)
    if m is None:
        fails.append(f"{name}: không thấy dòng '# L1 OK: smp ...'")
        continue
    s = m.group(1)
    a = agg[s]
    a["seeds"] += int(m.group(2))
    a["mn"] = int(m.group(4)) if a["mn"] is None else min(a["mn"], int(m.group(4)))
    a["sent"] += int(m.group(6))
    a["deliv"] += int(m.group(7))
    a["drop"] += int(m.group(8))
    a["map"] += int(m.group(9))
    a["walk"] += int(m.group(10))
    a["unmap"] += int(m.group(11))
    a["fault"] += int(m.group(12))
    a["shoot"] += int(m.group(13))

print()
print("== gộp ==")
print(f"{'smp':>4} {'seed':>8} {'ipc_min':>8} {'ipc_sent':>10} {'delivered':>10} {'dropped':>9} "
      f"{'map':>7} {'walk':>6} {'unmap':>6} {'fault':>6} {'shoot':>6} {'digest':>18}")
for s in smps:
    a = agg[s]
    print(f"{s:>4} {a['seeds']:>8} {str(a['mn']):>8} {a['sent']:>10} {a['deliv']:>10} {a['drop']:>9} "
          f"{a['map']:>7} {a['walk']:>6} {a['unmap']:>6} {a['fault']:>6} {a['shoot']:>6} "
          f"{digest[s]:016x}")

# Kiểm phủ: mỗi cấu hình smp phải có **đúng** `seeds` seed, không sót không trùng.
for s in smps:
    if agg[s]["seeds"] != seeds:
        fails.append(f"smp {s}: phủ {agg[s]['seeds']}/{seeds} seed — sót hoặc trùng lô")

print()
print(f"== wall {wall} s ({wall // 60} m {wall % 60} s), {len(smps)} cấu hình smp × {k} lô = {len(smps) * k} task ==")
if fails:
    print(f"== l1-tier: FAIL ({len(fails)} lỗi) ==")
    for f in fails:
        print(f"   - {f}")
    sys.exit(1)
print("== l1-tier: PASS (mọi lô xanh, phủ đủ seed) ==")
PY
RC=$?
exit "$RC"
