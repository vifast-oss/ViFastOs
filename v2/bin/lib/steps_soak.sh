#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245/D2-248 mục 2f) — **soak của `v2 nightly`**: N nghiệm thu từng ảnh, kế hoạch
# theo N/luồng, một bước soak fail-closed (`TỪ CHỐI ĐO` không phải smoke) + `soak-x86_64-max`
# (`V2_X64_CPU=max`, 600 × 3 — KT-323). Soak **không** nằm trong T2: chỉ `nightly --soak/--all`.
nightly_soak_full_n() { # <kind> -> N nghiệm thu của arch
    case "$1" in
        soak-riscv64) printf '1000' ;;
        soak-aarch64) printf '1000' ;;
        soak-x86_64) printf '600' ;;
        # **KT-333d (D2-248 mục 2f, KT-323)** — cùng N nghiệm thu 600 × 3 nhưng chạy QEMU
        # `-cpu max` (tập tính năng rộng nhất). Là **kịch bản riêng của nightly**, KHÔNG vào T2.
        soak-x86_64-max) printf '600' ;;
        *) printf '0' ;;
    esac
}
nightly_soak_script() { # <kind> -> runner
    case "$1" in
        soak-riscv64) printf '%s' 'soak.sh' ;;
        soak-aarch64) printf '%s' 'soak-aarch64.sh' ;;
        soak-x86_64) printf '%s' 'soak-x86_64.sh' ;;
        soak-x86_64-max) printf '%s' 'soak-x86_64.sh' ;;
        *) printf '' ;;
    esac
}
nightly_soak_n() { # <kind> -> N sẽ chạy (--n nếu có, ngược lại N đầy của arch)
    if [ -n "$NIGHTLY_SOAK_N" ]; then
        printf '%s' "$NIGHTLY_SOAK_N"
    else
        nightly_soak_full_n "$1"
    fi
}
# 0 = N/luồng đúng mức nghiệm thu của **mọi** arch (KT-348).
nightly_soak_is_full() {
    local k n full
    [ "$NIGHTLY_SOAK_RUNS" -eq 3 ] || return 1
    for k in soak-riscv64 soak-aarch64 soak-x86_64 soak-x86_64-max; do
        n="$(nightly_soak_n "$k")"
        full="$(nightly_soak_full_n "$k")"
        [ "$n" -eq "$full" ] || return 1
    done
    return 0
}
nightly_soak_is_smoke() { # 1 = có arch nào dưới N đầy
    local k
    for k in soak-riscv64 soak-aarch64 soak-x86_64 soak-x86_64-max; do
        [ "$(nightly_soak_n "$k")" -lt "$(nightly_soak_full_n "$k")" ] && return 0
    done
    return 1
}
# Một hàng kế hoạch soak: đúng N thật + ước lượng **co theo N/luồng** (không phải cam kết).
nightly_plan_soak_row() { # <kind> <nhãn arch> <est đầy (s)> [tiền-tố-env]
    local kind="$1" arch="$2" base="$3" envp="${4:-}" n full runs script est cmd smoke=""
    n="$(nightly_soak_n "$kind")"
    full="$(nightly_soak_full_n "$kind")"
    runs="$NIGHTLY_SOAK_RUNS"
    script="$(nightly_soak_script "$kind")"
    est=$(((base * n * runs) / (full * 3)))
    [ "$est" -lt 1 ] && est=1
    cmd="${envp}bash v2/test/boot/$script $n $runs"
    if [ "$n" -lt "$full" ]; then
        cmd="${envp}bash v2/test/boot/$script --smoke $n $runs"
        smoke="; SMOKE — không phải bằng chứng nghiệm thu"
    fi
    printf 'soak\tSoak %s ảnh sản phẩm (N=%s × %s luồng%s)\t%s\t%s\t%s\n' \
        "$arch" "$n" "$runs" "$smoke" "$kind" "$est" "$cmd"
}
nightly_plan_soak_rows() {
    nightly_plan_soak_row soak-riscv64 riscv64 320
    nightly_plan_soak_row soak-aarch64 aarch64 260
    nightly_plan_soak_row soak-x86_64 x86_64 220
    # **KT-333d** — `-cpu max` (KT-323) là **kịch bản riêng** của nightly, không nhân đôi T2.
    nightly_plan_soak_row soak-x86_64-max "x86_64 -cpu max" 220 "V2_X64_CPU=max "
}

# **KT-333c (D2-243 mục 3c)** — một bước soak: N **đầy** của arch (hoặc `--n`) + `--runs` luồng.
# N dưới mức nghiệm thu ⇒ chạy `--smoke` (script **luôn exit 1** theo hợp đồng "smoke không phải
# bằng chứng cổng"); nightly chỉ nhận khi log chứng minh **0 lỗi/N** và **không** có dòng FAIL.
# Logdir riêng từng arch (`KT_LOGDIR`), không ghi đè log của cổng/soak khác (D2-243 mục 3b).
nightly_soak_step() { # <script> <kind> <logdir> <n|rỗng> <runs>
    local script="$1" kind="$2" d="$3" n="$4" runs="$5" full rc tmp
    full="$(nightly_soak_full_n "$kind")"
    [ -n "$n" ] || n="$full"
    case "$n" in '' | *[!0-9]*) echo "v2 nightly: N soak phải là số dương (nhận '$n')" >&2; return 2 ;; esac
    [ "$n" -gt 0 ] || { echo "v2 nightly: N soak phải > 0" >&2; return 2; }
    mkdir -p "$d" 2>/dev/null || true
    if [ "$n" -ge "$full" ]; then
        # Đúng mức nghiệm thu: chạy chế độ cổng thật, mã thoát là kết quả.
        KT_LOGDIR="$d" bash "$script" "$n" "$runs"
        return $?
    fi
    tmp="$d/smoke.log"
    KT_LOGDIR="$d" bash "$script" --smoke "$n" "$runs" >"$tmp" 2>&1
    rc=$?
    cat "$tmp"
    [ "$rc" -eq 3 ] && return 3 # TỪ CHỐI ĐO (danh tính ảnh) — không phải smoke
    grep -q '# soak: FAIL' "$tmp" && return 1
    if ! grep -qF "# soak: $n/$n lỗi=0" "$tmp"; then
        echo "# nightly: smoke N=$n KHÔNG có bằng chứng '0/$n lỗi' (xem $tmp)" >&2
        return 1
    fi
    if ! grep -q '# soak: SMOKE' "$tmp"; then
        echo "# nightly: smoke N=$n thiếu dòng '# soak: SMOKE' (xem $tmp)" >&2
        return 1
    fi
    echo "# nightly: smoke N=$n × $runs luồng — 0 lỗi; KHÔNG phải bằng chứng nghiệm thu KT-348"
    return 0
}

nightly_step_exec() { # <kiểu> <logdir> <seeds> <smp> <soak N|rỗng> <soak runs>
    local nsoak="${5:-}" nruns="${6:-$NIGHTLY_SOAK_RUNS}"
    case "$1" in
        l1-tier-full)
            bash "$V2_ROOT/tools/l1-tier.sh" --tier full --logdir "$2/l1"
            ;;
        l1-driver) run_nightly_l1_driver "$3" "$4" ;;
        miri) run_nightly_miri ;;
        mutation) bash "$V2_ROOT/test/rules/selfbreak.sh" ;;
        soak-riscv64) nightly_soak_step "$V2_ROOT/test/boot/soak.sh" soak-riscv64 "$2/soak-riscv64" "$nsoak" "$nruns" ;;
        soak-aarch64) nightly_soak_step "$V2_ROOT/test/boot/soak-aarch64.sh" soak-aarch64 "$2/soak-aarch64" "$nsoak" "$nruns" ;;
        soak-x86_64) nightly_soak_step "$V2_ROOT/test/boot/soak-x86_64.sh" soak-x86_64 "$2/soak-x86_64" "$nsoak" "$nruns" ;;
        soak-x86_64-max)
            # **KT-333d (D2-248 mục 2f)** — in rõ `cpu=max` (KT-323) TRƯỚC khi chạy; soak-x86_64.sh
            # cũng in `# cấu hình: … -cpu max …` nhưng dòng này để log nightly tự mô tả.
            echo "# nightly: soak x86_64 cpu=max (KT-323; N=$nsoak runs=$nruns)"
            V2_X64_CPU=max nightly_soak_step "$V2_ROOT/test/boot/soak-x86_64.sh" soak-x86_64-max "$2/soak-x86_64-max" "$nsoak" "$nruns"
            ;;
        *) echo "v2 nightly: kiểu bước lạ '$1'" >&2; return 2 ;;
    esac
}
