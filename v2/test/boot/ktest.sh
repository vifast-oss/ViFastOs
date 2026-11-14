#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-44/45/46/47/48/49 — boot ảnh **TEST** riscv64 K3 (crate `vifast-kbin-riscv64`, feature `ktest`).
#
# D2-08: ảnh test KHÔNG nằm trong ảnh sản phẩm — cùng crate nhưng bin + feature riêng.
# KT-49: **hai** ảnh test (mỗi boot chỉ kết thúc được một lần bằng PANIC có chủ đích):
#   - `vifast-kbin-ktest`       : 20 TAP — KT-45 (image + 3 trap) + KT-46 (mmu on, map layout,
#                                 walk khop) + KT-48 (3 TAP PLIC + UART IRQ thật) + KT-47 (4 TAP
#                                 timer SBI: 100 tick đơn điệu, 1.000 tick không mất, irq thật,
#                                 drift Δ ∈ [9,11] ms) + **KT-49 SMP** (5 TAP: N harts alive qua
#                                 SBI HSM, stack/guard riêng mỗi hart, timer PerHart, PLIC context
#                                 `2*h+1` per hart, barrier + timeout không treo) + trap lồng fatal.
#   - `vifast-kbin-ktest-guard` : 3 TAP — guard page fault **thật**: load `scause=13` (probe resume)
#                                 rồi store `scause=15` fatal `PANIC: guard page fault`.
# Số TAP kỳ vọng đọc từ `expected.txt` (không hardcode hai nơi).
#
# D2-72 Q2/Q4/Q5/Q10: `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp N`, matrix
# **1/2/4** cho KT-49, budget riêng cho **mỗi** ảnh test (≤ 262.144 B), timeout 20 s/boot, chỉ
# `kill <pid>` do chính script tạo (AGENTS §9).
#
# Dùng: bash v2/test/boot/ktest.sh [NBOOT] [SMP|matrix]   (mặc định 1 boot, 1 hart)
#   vd: bash v2/test/boot/ktest.sh 50 matrix   # 50 boot **mỗi** N=1/2/4, log riêng từng N
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget.sh"
EXPECTED_FILE="$HERE/expected.txt"
TARGET="riscv64gc-unknown-none-elf"
IMG_MAIN="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest"
IMG_GUARD="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest-guard"
N_BOOT="${1:-1}"
SMP_ARG="${2:-1}"
case "$SMP_ARG" in
    matrix) SMP_LIST="1 2 4" ;;
    *) SMP_LIST="$SMP_ARG" ;;
esac
# KT-47: `-icount` để QEMU phục vụ timer ảo (ACLINT/SBI TIME) **đúng hạn** — không icount, QEMU 11
# TCG trên macOS chỉ chạy virtual timer mỗi ~200 ms (đo được: Δ tick 0.3–2.9 s), nên không thể đo
# DoD "Δ tick ∈ [9,11] ms". Dòng lệnh QEMU của D2-72 Q2 (machine/cpu/ram/smp) **giữ nguyên**; chỉ
# thêm icount cho **ảnh test** (đo định thời tất định); `run.sh`/soak giữ nguyên dòng D2-72 Q2.
# **Shift CỐ ĐỊNH = 6** (64 ns/lệnh), KHÔNG dùng shift tự động (`auto`): `auto` tự đổi shift giữa chừng (bám thời
# gian thật) ⇒ thời gian ảo nhảy, Δ tick lệch 3–17 ms (TAP drift đỏ ngẫu nhiên); shift cố định giữ
# Δ = 10 ms ± quantization icount, vẫn trong [9,11] ms. Deviation ghi ở `v2/docs/k3-riscv64.md`
# §10.5 + JOURNAL/STATE + D2-74.
QEMU_ICOUNT="-icount shift=6"
TIMEOUT_DS=200   # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
QEMU_MIN_MAJOR=9 # sàn D2-72 Q2: QEMU >= 9.2
QEMU_MIN_MINOR=2

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() { echo "ktest.sh: $*" >&2; exit 1; }

command -v cargo >/dev/null 2>&1 || die "không có cargo"
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"
[ -f "$EXPECTED_FILE" ] || die "không thấy $EXPECTED_FILE"

# OpenSBI `fw_dynamic` nạp trực tiếp, không U-Boot/EDK2 (D2-72 Q3).
SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && { SBI="$c"; break; }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

# QEMU version: thiếu hoặc dưới sàn major.minor ⇒ FAIL, không tự hạ (D2-72 Q2).
QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
case "$QEMU_VERSION" in *.*) ;; *) die "phiên bản QEMU '$QEMU_VERSION' không có dạng major.minor" ;; esac
QEMU_MAJOR="${QEMU_VERSION%%.*}"
QEMU_REST="${QEMU_VERSION#*.}"
QEMU_MINOR="${QEMU_REST%%.*}"
case "$QEMU_MAJOR" in '' | *[!0-9]*) die "không đọc được major của QEMU '$QEMU_VERSION'" ;; esac
case "$QEMU_MINOR" in '' | *[!0-9]*) die "không đọc được minor của QEMU '$QEMU_VERSION'" ;; esac
if [ "$QEMU_MAJOR" -lt "$QEMU_MIN_MAJOR" ] ||
    { [ "$QEMU_MAJOR" -eq "$QEMU_MIN_MAJOR" ] && [ "$QEMU_MINOR" -lt "$QEMU_MIN_MINOR" ]; }; then
    die "QEMU $QEMU_VERSION < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2) — không tự hạ"
fi
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2)"
echo "# opensbi $SBI"

# Kế hoạch/số `ok` kỳ vọng cho từng ảnh — một nguồn duy nhất `expected.txt`.
expected_field() { awk -v label="$1" -v field="$2" '$1 == label { print $field; exit }' "$EXPECTED_FILE"; }
PLAN_MAIN="$(expected_field ktest 2)"
OK_MAIN="$(expected_field ktest 3)"
PLAN_GUARD="$(expected_field ktest-guard 2)"
OK_GUARD="$(expected_field ktest-guard 3)"
case "${PLAN_MAIN:-}" in '' | *[!0-9]*) die "expected.txt: thiếu kế hoạch TAP cho 'ktest'" ;; esac
case "${OK_MAIN:-}" in '' | *[!0-9]*) die "expected.txt: thiếu số ok cho 'ktest'" ;; esac
case "${PLAN_GUARD:-}" in '' | *[!0-9]*) die "expected.txt: thiếu kế hoạch TAP cho 'ktest-guard'" ;; esac
case "${OK_GUARD:-}" in '' | *[!0-9]*) die "expected.txt: thiếu số ok cho 'ktest-guard'" ;; esac
echo "# expected: ktest plan=$PLAN_MAIN ok=$OK_MAIN; ktest-guard plan=$PLAN_GUARD ok=$OK_GUARD"

echo "# build 2 ảnh test ($TARGET, release, --features ktest)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
    --bin vifast-kbin-ktest --bin vifast-kbin-ktest-guard \
    --features ktest --target "$TARGET" --release >/tmp/k3-ktest-build.log 2>&1; then
    tail -20 /tmp/k3-ktest-build.log >&2
    die "build ảnh test FAIL — CHƯA có ảnh test (xem /tmp/k3-ktest-build.log)"
fi
for IMAGE in "$IMG_MAIN" "$IMG_GUARD"; do
    [ -f "$IMAGE" ] || die "không thấy ảnh test $IMAGE — CHƯA có ảnh test"
done

# Budget: mỗi ảnh test cũng phải ≤ 262.144 B (D2-72 Q5, không tách ngân sách riêng).
bash "$BUDGET" "$IMG_MAIN" ktest || die "budget ảnh test VƯỢT trần"
bash "$BUDGET" "$IMG_GUARD" ktest-guard || die "budget ảnh test guard VƯỢT trần"

# Ảnh test phải mang chuỗi `ktest` (D2-08: tách khỏi ảnh sản phẩm).
for PAIR in "ktest:$IMG_MAIN" "ktest-guard:$IMG_GUARD"; do
    LABEL="${PAIR%%:*}"
    IMAGE="${PAIR#*:}"
    if command -v strings >/dev/null 2>&1; then
        HITS="$(strings "$IMAGE" 2>/dev/null | grep -c ktest || true)"
    else
        HITS="$(LC_ALL=C grep -a -o ktest "$IMAGE" 2>/dev/null | wc -l | tr -d ' ' || true)"
    fi
    [ "${HITS:-0}" -ge 1 ] || die "ảnh $LABEL không chứa chuỗi 'ktest' — nhầm ảnh?"
    echo "# image=$LABEL ktest-strings=$HITS"
done

# boot_once <tag> <image> <smp> <index>: boot QEMU, in đường dẫn log qua stdout (chỉ đường dẫn).
boot_once() {
    local tag="$1" image="$2" smp="$3" index="$4"
    local out="/tmp/k3-ktest-$tag-smp$smp-$index.log"
    "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$image" -nographic \
        -cpu rv64 -m 128M -smp "$smp" -no-reboot >"$out" 2>&1 &
    local pid=$!
    local waited=0
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$TIMEOUT_DS" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null # chỉ kill ĐÚNG pid do script này tạo (AGENTS §9)
        wait "$pid" 2>/dev/null
        echo "$out"
        return 124
    fi
    wait "$pid" 2>/dev/null
    if grep -q '^OpenSBI v' "$out"; then
        echo "# opensbi $(sed -n 's/^OpenSBI \(v[0-9][0-9.]*\).*/\1/p' "$out" | head -1)" >&2
    fi
    echo "$out"
    return 0
}

# tap_numbers_ok <log> <plan>: đúng tập `ok 1..plan`, không thiếu/không lặp/không thừa.
tap_numbers_ok() {
    local out="$1" plan="$2"
    local got want
    got="$(grep -o '^ok [0-9][0-9]*' "$out" | awk '{print $2}' | sort -n | tr '\n' ' ')"
    want="$(seq 1 "$plan" | tr '\n' ' ')"
    [ "$got" = "$want" ]
}

# TAP chung cho cả hai ảnh: plan, đủ số `ok`, không `not ok`, không PANIC ngoài danh sách cho phép.
common_tap_ok() {
    local out="$1" plan="$2" ok_count="$3" allowed_panic_re="$4"
    local missing=""
    grep -q '^TAP version 13$' "$out" || missing="$missing TAP-version"
    grep -q "^1\\.\\.$plan$" "$out" || missing="$missing plan-1..$plan"
    tap_numbers_ok "$out" "$plan" || missing="$missing ok-set-1..$plan"
    local oks
    oks="$(grep -c '^ok ' "$out" || true)"
    [ "${oks:-0}" -eq "$ok_count" ] || missing="$missing ok-count=${oks:-0}/$ok_count"
    if grep -q '^not ok' "$out"; then missing="$missing not-ok"; fi
    local others
    others="$(grep '^PANIC' "$out" | grep -Evc "$allowed_panic_re" || true)"
    [ "${others:-0}" -eq 0 ] || missing="$missing PANIC-khác"
    printf '%s' "$missing"
}

fails=0
total_boots=0
clean_boots=0
for SMP in $SMP_LIST; do
    case "$SMP" in '' | *[!0-9]*) die "SMP '$SMP' không phải số" ;; esac
    [ "$SMP" -ge 1 ] && [ "$SMP" -le 8 ] || die "SMP=$SMP ngoài [1,8] (MAX_HARTS=8)"
    MASK="$(printf '%x' $(((1 << SMP) - 1)))"
    smp_fails=0
    i=1
    while [ "$i" -le "$N_BOOT" ]; do
        total_boots=$((total_boots + 1))
        # ------------------------------------------------------- ảnh chính: 20 TAP (KT-49)
        if out="$(boot_once main "$IMG_MAIN" "$SMP" "$i")"; then :; else
            echo "not ok - ktest smp=$SMP boot $i: quá $((TIMEOUT_DS / 10)) s (treo), log $out"
            fails=$((fails + 1))
            smp_fails=$((smp_fails + 1))
            i=$((i + 1))
            continue
        fi
        missing="$(common_tap_ok "$out" "$PLAN_MAIN" "$OK_MAIN" '^PANIC: nested trap \(KT-45: trap trong handler = fatal, khong resume\)$')"
        # 1..20: KT-45 (1..4), KT-46 (5..7), KT-48 (8..10), KT-47 (11..14), KT-49 (15..19), trap lồng (20).
        grep -q '^ok 1 - ktest image$' "$out" || missing="$missing ok1-image"
        grep -q '^ok 2 - trap breakpoint$' "$out" || missing="$missing ok2-breakpoint"
        grep -q '^ok 3 - trap ecall sbi$' "$out" || missing="$missing ok3-ecall-sbi"
        grep -q '^ok 4 - trap illegal inject$' "$out" || missing="$missing ok4-illegal-inject"
        grep -q '^ok 5 - mmu on$' "$out" || missing="$missing ok5-mmu-on"
        grep -q '^ok 6 - map layout$' "$out" || missing="$missing ok6-map-layout"
        grep -q '^ok 7 - walk khop$' "$out" || missing="$missing ok7-walk-khop"
        grep -q '^ok 8 - plic window$' "$out" || missing="$missing ok8-plic-window"
        grep -q '^ok 9 - uart irq$' "$out" || missing="$missing ok9-uart-irq"
        grep -q '^ok 10 - plic no fake irq$' "$out" || missing="$missing ok10-plic-no-fake-irq"
        grep -q '^ok 11 - timer 100 ticks monotonic$' "$out" || missing="$missing ok11-timer-monotonic"
        grep -q '^ok 12 - timer no lost tick$' "$out" || missing="$missing ok12-timer-no-lost-tick"
        grep -q '^ok 13 - timer irq$' "$out" || missing="$missing ok13-timer-irq"
        grep -q '^ok 14 - timer deadline drift$' "$out" || missing="$missing ok14-timer-drift"
        grep -q "^ok 15 - harts alive $SMP$" "$out" || missing="$missing ok15-harts-alive-$SMP"
        grep -q '^ok 16 - per-hart stack/guard$' "$out" || missing="$missing ok16-stack-guard"
        grep -q '^ok 17 - per-hart timer$' "$out" || missing="$missing ok17-per-hart-timer"
        grep -q '^ok 18 - plic ctx per hart$' "$out" || missing="$missing ok18-plic-ctx"
        grep -q '^ok 19 - barrier$' "$out" || missing="$missing ok19-barrier"
        grep -q '^ok 20 - trap nested panic$' "$out" || missing="$missing ok20-nested"
        # --- bằng chứng KT-45..KT-49 (số đo do chính boot in ra, không viết tay)
        grep -q '^# mmu: .*mode=8' "$out" || missing="$missing mmu-evidence"
        grep -q '^# mmu on: .*alias=1 .*kernel_low=1 kernel_high=1' "$out" ||
            missing="$missing mmu-on-evidence"
        grep -q '^# mmu layout: .*none_low=1 none_high=1 none_physmap=1' "$out" ||
            missing="$missing guard-none-evidence"
        grep -q '^# mmu layout: .*physmap_2m_nx=1 uart_dev=1 plic_dev=1 kernel_high_2m=1 all_harts=1$' "$out" ||
            missing="$missing all-hart-layout-evidence"
        grep -q '^# walk khop: pre=post=1 software=8/8' "$out" ||
            missing="$missing walk-khop-evidence"
        grep -q '^# plic: .*threshold=0 priority_uart=1 enable_ctx=1 enable_mmode=0 claim_idle=0$' "$out" ||
            missing="$missing plic-window-evidence"
        grep -q '^# plic map: .* low_dev=1 high_dev=1 low_2m=1 high_2m=1 low_nx=1 high_nx=1$' "$out" ||
            missing="$missing plic-map-evidence"
        grep -q '^# uart irq: irqs=[1-9][0-9]* claims=[1-9][0-9]* completes=[1-9][0-9]* traps_delta=[1-9][0-9]* balanced=1 spurious=0 unexpected=0 last_irq=10 last_iir=0x2 ier_off=1 plic_enable=0$' "$out" ||
            missing="$missing uart-irq-evidence"
        grep -q '^# plic quiet: claim_idle=0 quiet=1 stats_unchanged=1 external_traps=[1-9][0-9]* interrupts_delta=[1-9][0-9]* spurious=0 unexpected=0$' "$out" ||
            missing="$missing plic-quiet-evidence"
        grep -q '^# timer arm: .*tick_ticks=100000 tick_hz=100 timebase_hz=10000000 sie_stie=1 sstatus_sie=1$' "$out" ||
            missing="$missing timer-arm-evidence"
        grep -q '^# timer ticks: .*non_monotonic=0 late=0 missed=0' "$out" ||
            missing="$missing timer-monotonic-evidence"
        grep -q '^# timer drift: .*in_range=1' "$out" || missing="$missing timer-drift-evidence"
        grep -q '^# timer irq: .*scause=0x8000000000000005 code=5' "$out" ||
            missing="$missing timer-irq-evidence"
        grep -q '^# timer sie: .*sret_sie=1$' "$out" || missing="$missing timer-sie-evidence"
        grep -q '^# timer disable: sie_stie=0 sstatus_sie=0$' "$out" ||
            missing="$missing timer-disable-evidence"
        # --- KT-49: SBI HSM (hart theo DTB qua hart_get_status), N harts alive, per-hart state.
        grep -Eq "^# smp: boot=[0-9]+ harts=$SMP .* timeout=0 .* dtb_magic=1 .* stride=0x15000 area=0xa8000$" "$out" ||
            missing="$missing smp-bringup-evidence"
        grep -q "^# harts alive: n=$SMP mask=0x$MASK alive=0x$MASK tickets=1 arrived=0x$MASK timeout=0$" "$out" ||
            missing="$missing harts-alive-evidence"
        grep -q "^# per-hart stack/guard: stack_ok=0x$MASK guard_ok=0x$MASK distinct=1 owner_ok=true$" "$out" ||
            missing="$missing per-hart-stack-guard-evidence"
        grep -q "^# per-hart timer: ok=0x$MASK total_ticks=[1-9][0-9]* arrived=0x$MASK timeout=0$" "$out" ||
            missing="$missing per-hart-timer-evidence"
        grep -q "^# per-hart plic: ok=0x$MASK distinct=1 uart_irqs_boot=8 stats_clean=1 arrived=0x$MASK timeout=0$" "$out" ||
            missing="$missing per-hart-plic-evidence"
        grep -Eq '^# barrier: round=5 arrived=0x[0-9a-f]+ missing=0x0 timeout=0 wait_ticks=[0-9]+ slow_hart=1 slow_delayed=[01]$' "$out" ||
            missing="$missing barrier-evidence"
        grep -Eq '^# barrier timeout: round=6 expected_missing=0x[0-9a-f]+ arrived=0x[0-9a-f]+ missing=0x[0-9a-f]+ timeout=1 wait_ticks=[0-9]+ \(may khong treo\)$' "$out" ||
            missing="$missing barrier-timeout-evidence"
        # Mỗi hart in đúng MỘT dòng alive (id + ticket + tp) và một dòng stack per-hart.
        hart=0
        while [ "$hart" -lt "$SMP" ]; do
            grep -Eq "^# hart $hart alive ticket=[0-9]+ tp=$hart " "$out" ||
                missing="$missing hart$hart-alive-line"
            grep -Eq "^# stack hart=$hart .* owner=$hart .* frame_trap=1 rw_nx=1 guard_none=1$" "$out" ||
                missing="$missing hart$hart-stack-line"
            hart=$((hart + 1))
        done
        [ "$(grep -cE '^# timer hart=[0-9]+ .* ok=1$' "$out" || true)" -eq "$SMP" ] ||
            missing="$missing timer-hart-lines"
        [ "$(grep -cE '^# plic hart=[0-9]+ .* ok=1$' "$out" || true)" -eq "$SMP" ] ||
            missing="$missing plic-hart-lines"
        # Bằng chứng độc lập từ OpenSBI: máy ảo thật sự có N hart (không chỉ kernel tự khai).
        grep -q "^Platform HART Count *: *$SMP" "$out" || missing="$missing opensbi-hart-count"
        if [ -z "$missing" ]; then
            echo "ok - ktest smp=$SMP boot $i (20 TAP: trap+mmu+plic+timer+smp)"
            clean_boots=$((clean_boots + 1))
        else
            echo "not ok - ktest smp=$SMP boot $i: thiếu$missing, log $out"
            fails=$((fails + 1))
            smp_fails=$((smp_fails + 1))
        fi

        # ------------------------------------------------------- ảnh guard: 3 TAP, store fatal
        total_boots=$((total_boots + 1))
        if out="$(boot_once guard "$IMG_GUARD" "$SMP" "$i")"; then :; else
            echo "not ok - guard ktest smp=$SMP boot $i: quá $((TIMEOUT_DS / 10)) s (treo), log $out"
            fails=$((fails + 1))
            smp_fails=$((smp_fails + 1))
            i=$((i + 1))
            continue
        fi
        missing="$(common_tap_ok "$out" "$PLAN_GUARD" "$OK_GUARD" '^PANIC: guard page fault scause=0xf ')"
        grep -q '^ok 1 - guard image$' "$out" || missing="$missing ok1-guard-image"
        grep -q '^ok 2 - guard load fault$' "$out" || missing="$missing ok2-guard-load"
        grep -q '^ok 3 - guard store fault$' "$out" || missing="$missing ok3-guard-store"
        # KT-49: ảnh guard dùng guard **của hart boot thật** — runner đọc hart từ `# guard image:`
        # rồi đòi fault/load/store đúng hart đó (không ghim hart 0; OpenSBI có thể boot hart khác 0).
        BOOT_HART="$(sed -n 's/^# guard image: hart=\([0-9][0-9]*\) .*/\1/p' "$out" | head -1)"
        [ -n "$BOOT_HART" ] || missing="$missing guard-image-hart"
        if [ -n "$BOOT_HART" ]; then
            grep -q "^# guard load: hart=$BOOT_HART cham 0x" "$out" ||
                missing="$missing guard-load-line"
            grep -q "^# guard store: hart=$BOOT_HART ghi 0x" "$out" ||
                missing="$missing guard-store-line"
            grep -q "^# guard fault load scause=0xd stval=0x[0-9a-f]* sepc=0x[0-9a-f]* len=[24] recovered=1 hart=$BOOT_HART$" "$out" ||
                missing="$missing guard-load-real"
            # hart boot phải nằm trong số hart của máy ảo.
            if ! awk -v h="$BOOT_HART" '/^Platform HART Count *:/ {gsub(/\r/,""); if ($NF > h) ok=1} END {exit !ok}' "$out"; then
                missing="$missing guard-hart-out-of-range"
            fi
        fi
        grep -q '^# guard load stats: .*last_scause=0xd' "$out" ||
            missing="$missing guard-load-stats"
        grep -q '^PANIC: guard page fault scause=0xf stval=0x[0-9a-f]* sepc=0x[0-9a-f]*' "$out" ||
            missing="$missing guard-store-panic"
        grep -q "^Platform HART Count *: *$SMP" "$out" || missing="$missing opensbi-hart-count"
        if [ -z "$missing" ]; then
            echo "ok - guard smp=$SMP boot $i (3 TAP: load 13 + store 15)"
            clean_boots=$((clean_boots + 1))
        else
            echo "not ok - guard smp=$SMP boot $i: thiếu$missing, log $out"
            fails=$((fails + 1))
            smp_fails=$((smp_fails + 1))
        fi
        i=$((i + 1))
    done
    echo "# smp=$SMP: $((N_BOOT * 2 - smp_fails))/$((N_BOOT * 2)) boot ảnh test sạch"
    [ "$smp_fails" -eq 0 ] || echo "# smp=$SMP: FAIL ($smp_fails lỗi)"
done

echo "# KTEST: $clean_boots/$total_boots boot sạch (smp=$SMP_ARG, NBOOT=$N_BOOT)"
[ "$fails" -eq 0 ] || exit 1
echo "# ktest.sh: PASS"
