#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245/D2-248 mục 2f) — **soak của `v2 nightly`**: N nghiệm thu từng ảnh, kế hoạch
# theo N/luồng, một bước soak fail-closed (`TỪ CHỐI ĐO` không phải smoke) + `soak-x86_64-max`
# (`V2_X64_CPU=max`, 600 × 3 — KT-323). Soak **không** nằm trong T2: chỉ `nightly --soak/--all`.
#
# **KT-347b (D2-245/D2-252 Pha A)** — `--profile` chọn **bộ bước soak** (một nguồn duy nhất ở
# `nightly_soak_kinds`), in rõ profile, và **từ chối tên lạ** (fail-closed, có tên):
#
#   (không nêu)     4 bước cũ (giữ **nguyên** hành vi trước KT-347b): riscv64 tối giản,
#                   aarch64, x86_64, x86_64 `-cpu max`.
#   product-shell   3 arch **ảnh sản phẩm có shell**: riscv64 `soak.sh --features product-shell`,
#                   aarch64 (bước cũ build+kiểm `product-shell`), x86_64 (bước cũ build+kiểm
#                   `product-shell`). Không gồm nhánh `-cpu max` (profile riêng bên dưới).
#   soak-x86_64-max riêng nhánh x86_64 `-cpu max` (600 × 3, KT-323).
#   virtio-blk      soak **đĩa thật**: riscv64 `--features product-shell,product-fs --disk`
#                   (1.000 boot, mỗi boot gắn `virtio-blk-pci` + đòi `# product fs: medium=virtio-blk`).
#   all             cả 5 bước trên (gộp 2 profile đầu + đĩa thật) — chỉ khi nêu **tường minh**.
#
# Mọi ngưỡng N giữ nguyên: riscv64/aarch64 **1.000**, x86_64 (**cả** `-cpu max`) **600** × 3 luồng
# (D2-24/D2-149/D2-239). `--smoke`/`--n` vẫn là **hạ N có ghi rõ**, không phải bằng chứng nghiệm thu.
nightly_soak_profile() { # <rỗng|product-shell|soak-x86_64-max|virtio-blk|all>
    printf '%s' "${NIGHTLY_PROFILE:-}"
}
nightly_soak_profile_label() { # nhãn để in log (rỗng = mặc định cũ)
    local p
    p="$(nightly_soak_profile)"
    if [ -z "$p" ]; then
        printf '%s' 'mặc định (4 bước KT-348, trước KT-347b)'
    else
        printf '%s' "$p"
    fi
}
# Danh sách **bước** theo profile — **một** nguồn cho plan/run/selftest/status.
nightly_soak_kinds() {
    case "$(nightly_soak_profile)" in
        product-shell) printf '%s\n' soak-riscv64-product-shell soak-aarch64 soak-x86_64 ;;
        soak-x86_64-max) printf '%s\n' soak-x86_64-max ;;
        virtio-blk) printf '%s\n' soak-riscv64-virtio-blk ;;
        all) printf '%s\n' soak-riscv64-product-shell soak-aarch64 soak-x86_64 \
            soak-x86_64-max soak-riscv64-virtio-blk ;;
        '') printf '%s\n' soak-riscv64 soak-aarch64 soak-x86_64 soak-x86_64-max ;;
        *) return 1 ;;
    esac
}
nightly_soak_full_n() { # <kind> -> N nghiệm thu của arch
    case "$1" in
        soak-riscv64) printf '1000' ;;
        soak-riscv64-product-shell) printf '1000' ;;
        soak-riscv64-virtio-blk) printf '1000' ;;
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
        soak-riscv64-product-shell) printf '%s' 'soak.sh' ;;
        soak-riscv64-virtio-blk) printf '%s' 'soak.sh' ;;
        soak-aarch64) printf '%s' 'soak-aarch64.sh' ;;
        soak-x86_64) printf '%s' 'soak-x86_64.sh' ;;
        soak-x86_64-max) printf '%s' 'soak-x86_64.sh' ;;
        *) printf '' ;;
    esac
}
# **KT-347b** — tham số **riêng** của bước (đặt **trước** N/LANES và trước `--smoke`).
nightly_soak_kind_args() {
    case "$1" in
        soak-riscv64-product-shell) printf '%s' '--features product-shell' ;;
        soak-riscv64-virtio-blk) printf '%s' '--features product-shell,product-fs --disk' ;;
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
# 0 = N/luồng đúng mức nghiệm thu của **mọi bước trong profile** (KT-348).
nightly_soak_is_full() {
    local k n full
    [ "$NIGHTLY_SOAK_RUNS" -eq 3 ] || return 1
    for k in $(nightly_soak_kinds); do
        n="$(nightly_soak_n "$k")"
        full="$(nightly_soak_full_n "$k")"
        [ "$n" -eq "$full" ] || return 1
    done
    return 0
}
nightly_soak_is_smoke() { # 1 = có bước nào dưới N đầy
    local k
    for k in $(nightly_soak_kinds); do
        [ "$(nightly_soak_n "$k")" -lt "$(nightly_soak_full_n "$k")" ] && return 0
    done
    return 1
}
# Một hàng kế hoạch soak: đúng N thật + ước lượng **co theo N/luồng** (không phải cam kết).
nightly_plan_soak_row() { # <kind> <nhãn arch> <est đầy (s)> [tiền-tố-env]
    local kind="$1" arch="$2" base="$3" envp="${4:-}" n full runs script est cmd smoke=""
    local kargs
    n="$(nightly_soak_n "$kind")"
    full="$(nightly_soak_full_n "$kind")"
    runs="$NIGHTLY_SOAK_RUNS"
    script="$(nightly_soak_script "$kind")"
    kargs="$(nightly_soak_kind_args "$kind")"
    est=$(((base * n * runs) / (full * 3)))
    [ "$est" -lt 1 ] && est=1
    cmd="${envp}bash v2/test/boot/$script ${kargs}${kargs:+ }$n $runs"
    if [ "$n" -lt "$full" ]; then
        cmd="${envp}bash v2/test/boot/$script ${kargs}${kargs:+ }--smoke $n $runs"
        smoke="; SMOKE — không phải bằng chứng nghiệm thu"
    fi
    printf 'soak\tSoak %s ảnh sản phẩm (N=%s × %s luồng%s)\t%s\t%s\t%s\n' \
        "$arch" "$n" "$runs" "$smoke" "$kind" "$est" "$cmd"
}
nightly_plan_soak_rows() {
    local k
    for k in $(nightly_soak_kinds); do
        case "$k" in
            soak-riscv64) nightly_plan_soak_row soak-riscv64 riscv64 320 ;;
            soak-riscv64-product-shell) nightly_plan_soak_row soak-riscv64-product-shell "riscv64 product-shell" 340 ;;
            soak-aarch64) nightly_plan_soak_row soak-aarch64 aarch64 260 ;;
            soak-x86_64) nightly_plan_soak_row soak-x86_64 x86_64 220 ;;
            # **KT-333d** — `-cpu max` (KT-323) là **kịch bản riêng** của nightly, không nhân đôi T2.
            soak-x86_64-max) nightly_plan_soak_row soak-x86_64-max "x86_64 -cpu max" 220 "V2_X64_CPU=max " ;;
            # **KT-347b** — soak **đĩa thật** `virtio-blk` (profile `virtio-blk`).
            soak-riscv64-virtio-blk) nightly_plan_soak_row soak-riscv64-virtio-blk "riscv64 + đĩa thật virtio-blk" 400 ;;
        esac
    done
}

# **KT-333c (D2-243 mục 3c)** — một bước soak: N **đầy** của arch (hoặc `--n`) + `--runs` luồng.
# N dưới mức nghiệm thu ⇒ chạy `--smoke` (script **luôn exit 1** theo hợp đồng "smoke không phải
# bằng chứng cổng"); nightly chỉ nhận khi log chứng minh **0 lỗi/N** và **không** có dòng FAIL.
# Logdir riêng từng arch (`KT_LOGDIR`), không ghi đè log của cổng/soak khác (D2-243 mục 3b).
nightly_soak_step() { # <script> <kind> <logdir> <n|rỗng> <runs>
    local script="$1" kind="$2" d="$3" n="$4" runs="$5" full rc tmp kargs
    full="$(nightly_soak_full_n "$kind")"
    kargs="$(nightly_soak_kind_args "$kind")"
    [ -n "$n" ] || n="$full"
    case "$n" in '' | *[!0-9]*) echo "v2 nightly: N soak phải là số dương (nhận '$n')" >&2; return 2 ;; esac
    [ "$n" -gt 0 ] || { echo "v2 nightly: N soak phải > 0" >&2; return 2; }
    mkdir -p "$d" 2>/dev/null || true
    if [ "$n" -ge "$full" ]; then
        # Đúng mức nghiệm thu: chạy chế độ cổng thật, mã thoát là kết quả.
        # shellcheck disable=SC2086
        KT_LOGDIR="$d" bash "$script" $kargs "$n" "$runs"
        return $?
    fi
    tmp="$d/smoke.log"
    # shellcheck disable=SC2086
    KT_LOGDIR="$d" bash "$script" $kargs --smoke "$n" "$runs" >"$tmp" 2>&1
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
        mutants)
            # **KT-403 r2 (D2-259)** — `v2 mutants --all` theo manifest; mỗi đột biến chạy trên
            # **bản sao** (D2-113F) và phân loại KILLED/SURVIVED/E_*. `find_python` fail-closed.
            local py
            py="$(find_python)" || {
                echo "# nightly: cần python ≥3.11 có tomllib cho v2 mutants" >&2
                return 2
            }
            "$py" "$V2_ROOT/tools/mutants.py" --root "$REPO_ROOT" --all
            ;;
        soak-riscv64) nightly_soak_step "$V2_ROOT/test/boot/soak.sh" soak-riscv64 "$2/soak-riscv64" "$nsoak" "$nruns" ;;
        soak-riscv64-product-shell)
            nightly_soak_step "$V2_ROOT/test/boot/soak.sh" soak-riscv64-product-shell \
                "$2/soak-riscv64-product-shell" "$nsoak" "$nruns"
            ;;
        soak-riscv64-virtio-blk)
            echo "# nightly: soak riscv64 đĩa thật virtio-blk (N=$nsoak runs=$nruns; --features product-shell,product-fs --disk)"
            nightly_soak_step "$V2_ROOT/test/boot/soak.sh" soak-riscv64-virtio-blk \
                "$2/soak-riscv64-virtio-blk" "$nsoak" "$nruns"
            ;;
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
