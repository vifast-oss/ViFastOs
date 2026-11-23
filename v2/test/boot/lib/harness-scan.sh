#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-347b** (D2-245/D2-252 Pha A) — **một nguồn** cho bộ quét harness marker **D2-08** của
# runner boot (`v2/test/boot/*.sh`).
#
# # Vì sao có tệp này
#
# Trước KT-347b, danh sách **23 tiền tố** marker D2-08 được chép nguyên văn ở **≥ 4 tệp**
# (`budget-x86_64.sh`, `budget-aarch64.sh`, `run-x86_64.sh`, `run-aarch64.sh`), còn logic quét
# (`grep -a -o -F` / `strings | grep -Fc` / `for pat …`) được chép ở **6 tệp**. Thêm một marker
# mới (hoặc sửa một marker) phải sửa 4–6 chỗ; bỏ sót một chỗ là bộ quét **rỗng một lỗ** mà không
# ai thấy. Tệp này gom **danh sách + logic** vào một chỗ; runner chỉ còn **gọi**, không chép lại.
#
# # Hợp đồng (giữ **nguyên** hành vi cũ)
#
#   HARNESS_SCAN_MARKERS / HARNESS_SCAN_COUNT
#       23 tiền tố do **mã kiểm** phát ra (D2-08). Ảnh sản phẩm (`*-product`, `*-product-shell`)
#       phải **0** hit; ảnh `ktest` phải **> 0** (ca âm chứng minh bộ đếm có răng).
#
#   harness_scan_count <tệp> [marker…]
#       Tổng số **lần xuất hiện** (`grep -a -o -F | wc -l`) — đúng cách đếm cũ của
#       `budget-x86_64.sh`/`product-riscv64.sh`. Không nêu marker ⇒ dùng **23 tiền tố**.
#       Tệp chưa tồn tại ⇒ **chờ có hạn** rồi mới kết luận (xem dưới), hết hạn ⇒ in `0` + rc 1
#       (fail-closed: không bao giờ "0 marker" vì tệp vắng mặt).
#
#   harness_scan_count_strings <tệp> [marker…]
#       Tổng số **dòng** `strings` khớp mỗi marker (`strings -a | grep -Fc`) — đúng cách đếm cũ của
#       `budget-aarch64.sh`/`soak-x86_64.sh`. Không có `strings` ⇒ lùi về `harness_scan_count`.
#
#   harness_scan_require_zero <tệp> <nhãn>
#       rc 0 ⇔ 0 hit; rc 1 + in lý do (kèm số hit) nếu > 0 **hoặc** tệp không đọc được.
#
#   harness_scan_selftest
#       Tự kiểm **không QEMU**: 23 marker; tệp sạch ⇒ 0; chèn 1 marker ⇒ > 0 (ca âm **ĐỎ**);
#       đếm lần-xuất-hiện vs đếm dòng `strings`; danh sách rút gọn (4 marker của `product-riscv64`)
#       vẫn đúng; **TOCTOU**: tệp xuất hiện muộn (cargo đang thay bin trong `target/`) vẫn được
#       chờ tới khi đọc được; tệp vắng hết hạn ⇒ rc 1 **không** phải "0 marker".
#
# # Chống TOCTOU (bài học `KT-347c` ở `ktest.sh`)
#
# `cargo` thay tệp bin trong `target/release` **không nguyên tử**: `[ -f ]` có thể đúng ngay trước
# lúc tệp bị thay và sai ngay sau đó. Vì vậy **không** dùng `[ -f ]` một lần rồi kết luận: mọi hàm
# quét chờ tối đa `HARNESS_SCAN_WAIT_MS` (mặc định 20.000 ms, cùng ngân sách `wait_image` của
# `ktest.sh`) cho tới khi tệp **đọc được**, rồi mới đếm. Hết hạn ⇒ rc 1 (fail-closed).
#
# # Ghi chú
#
# Tệp **không** có trạng thái toàn cục, không `exit`, không `cd`; an toàn khi `source` dưới `set -u`
# (bash 3.2 của macOS). Chạy trực tiếp chỉ để tự kiểm/chẩn đoán: `--selftest`.
set -u

# 23 tiền tố harness — **nguồn sự thật duy nhất** (D2-08). Thứ tự giữ nguyên bản cũ để log của
# bộ quét không đổi thứ tự khi so sánh.
HARNESS_SCAN_MARKERS=(
    'TAP version 13' 'not ok' '# TAP ' 'ok 1 - '
    '# smp:' '# net:' '# umode:' '# blk:' '# gpu:' '# input:' '# pci:' '# tlbi:' '# wx:' '# dma:'
    '# ipi:' '# vspace:' '# high:' '# mmu:' '# brk:' '# trap:' '# vector:' '# fault:' '# physmap:'
)
HARNESS_SCAN_COUNT=23
# Ngân sách chờ tệp đọc được (ms) — cùng mức 20 s của `wait_image` trong `ktest.sh` (KT-347c).
HARNESS_SCAN_WAIT_MS="${HARNESS_SCAN_WAIT_MS:-20000}"

# `harness_scan_wait_file <tệp> [max_ms]`: 0 ⇔ tệp tồn tại **và** đọc được (`-s`); chờ theo bước
# 50 ms tới hạn. Dùng `sleep 0.05` (macOS/BSD sleep chấp nhận phần lẻ).
harness_scan_wait_file() {
    local f="${1:-}" max="${2:-$HARNESS_SCAN_WAIT_MS}" waited=0
    [ -n "$f" ] || return 1
    while [ "$waited" -le "$max" ]; do
        [ -f "$f" ] && [ -r "$f" ] && [ -s "$f" ] && return 0
        [ "$waited" -ge "$max" ] && break
        sleep 0.05 2>/dev/null || sleep 1
        waited=$((waited + 50))
    done
    [ -f "$f" ] && [ -r "$f" ] && [ -s "$f" ]
}

# `harness_scan_count <tệp> [marker…]` — đếm **lần xuất hiện** trên ảnh nhị phân.
harness_scan_count() {
    local f="${1:-}"
    [ $# -ge 1 ] && shift
    local -a pats
    if [ "$#" -ge 1 ]; then
        pats=("$@")
    else
        pats=("${HARNESS_SCAN_MARKERS[@]}")
    fi
    if ! harness_scan_wait_file "$f"; then
        echo 0
        return 1
    fi
    local total=0 pat n
    for pat in "${pats[@]}"; do
        n="$(LC_ALL=C grep -a -o -F -- "$pat" "$f" 2>/dev/null | wc -l | tr -d ' ' || true)"
        case "$n" in '' | *[!0-9]*) n=0 ;; esac
        total=$((total + n))
    done
    echo "$total"
    return 0
}

# `harness_scan_count_strings <tệp> [marker…]` — đếm **dòng** `strings` khớp (cách cũ của
# budget-aarch64/soak*); không có `strings` ⇒ lùi về `harness_scan_count`.
harness_scan_count_strings() {
    local f="${1:-}"
    [ $# -ge 1 ] && shift
    local -a pats
    if [ "$#" -ge 1 ]; then
        pats=("$@")
    else
        pats=("${HARNESS_SCAN_MARKERS[@]}")
    fi
    if ! harness_scan_wait_file "$f"; then
        echo 0
        return 1
    fi
    local total=0 pat n
    if command -v strings >/dev/null 2>&1; then
        for pat in "${pats[@]}"; do
            n="$(strings -a "$f" 2>/dev/null | grep -F -c -- "$pat" 2>/dev/null || true)"
            case "$n" in '' | *[!0-9]*) n=0 ;; esac
            total=$((total + n))
        done
    else
        total="$(harness_scan_count "$f" "${pats[@]}")"
    fi
    echo "$total"
    return 0
}

# `harness_scan_require_zero <tệp> <nhãn>` — 0 ⇔ 0 hit; > 0 hoặc tệp không đọc được ⇒ 1 + lý do.
harness_scan_require_zero() {
    local f="${1:-}" label="${2:-image}" hits
    if ! hits="$(harness_scan_count "$f")"; then
        echo "harness-scan: KHÔNG đọc được '$f' sau ${HARNESS_SCAN_WAIT_MS} ms — TỪ CHỐI kết luận '0 marker' ($label)" >&2
        return 1
    fi
    if [ "${hits:-1}" -ne 0 ]; then
        echo "harness-scan: D2-08 VI PHẠM — '$label' chứa $hits harness marker (cần 0; ${HARNESS_SCAN_COUNT} tiền tố, xem $f)" >&2
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------- tự kiểm (không QEMU)
harness_scan_selftest() {
    local fails=0 cases=0 tmp clean dirty hits a b
    _hs_ok() {
        cases=$((cases + 1))
        echo "ok $cases - $*"
    }
    _hs_bad() {
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - $*"
    }
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/harness-scan-selftest.XXXXXX")" || {
        echo "harness-scan: không tạo được thư mục tạm" >&2
        return 1
    }
    clean="$tmp/clean.bin"
    dirty="$tmp/dirty.bin"
    printf '# viFastOs x86_64: boot ok\n# shell: prompt\n# viFastOs x86_64: halt\n' >"$clean"
    cp "$clean" "$dirty"

    # (1) danh sách 23 tiền tố — thiếu một marker là một lỗ (đếm phần tử mới bắt được).
    if [ "${#HARNESS_SCAN_MARKERS[@]}" -eq 23 ] && [ "$HARNESS_SCAN_COUNT" -eq 23 ]; then
        _hs_ok "danh sách marker = ${#HARNESS_SCAN_MARKERS[@]} tiền tố (D2-08 đòi 23)"
    else
        _hs_bad "danh sách marker = ${#HARNESS_SCAN_MARKERS[@]} (hằng $HARNESS_SCAN_COUNT), đòi 23"
    fi
    # (2) tệp sạch ⇒ 0 hit ở **cả hai** cách đếm.
    a="$(harness_scan_count "$clean")"
    b="$(harness_scan_count_strings "$clean")"
    if [ "$a" = "0" ] && [ "$b" = "0" ]; then
        _hs_ok "ảnh sạch: 0 marker (đếm lần-xuất-hiện + đếm dòng strings)"
    else
        _hs_bad "ảnh sạch mà bộ quét báo count=$a strings=$b"
    fi
    # (3) ca âm: chèn marker ⇒ phải > 0 (bộ đếm báo 0 mà không có ca âm là dấu cao su, OD-K6-5).
    printf 'not ok 1 - marker chen vao\n' >>"$dirty"
    a="$(harness_scan_count "$dirty")"
    b="$(harness_scan_count_strings "$dirty")"
    if [ "${a:-0}" -gt 0 ] && [ "${b:-0}" -gt 0 ]; then
        _hs_ok "ca âm: chèn 1 marker ⇒ count=$a strings=$b (> 0, có răng)"
    else
        _hs_bad "ca âm: chèn 1 marker mà count=$a strings=$b"
    fi
    # (4) ca âm của **cổng**: tệp bẩn ⇒ `require_zero` ĐỎ; tệp sạch ⇒ XANH.
    if ! harness_scan_require_zero "$dirty" "dirty" >/dev/null 2>&1 &&
        harness_scan_require_zero "$clean" "clean" >/dev/null 2>&1; then
        _hs_ok "require_zero: tệp sạch ⇒ XANH, tệp có marker ⇒ ĐỎ (fail-closed)"
    else
        _hs_bad "require_zero nhận nhầm tệp bẩn hoặc từ chối tệp sạch"
    fi
    # (5) **hai cách đếm khác nhau có chủ đích**: 2 lần cùng một marker trên **một dòng** ⇒
    #     đếm lần-xuất-hiện = 2, đếm dòng strings = 1. Đổi công thức là đổi hành vi của runner cũ
    #     (`budget-x86_64` đếm `-o`, `budget-aarch64` đếm dòng) — ca này giữ đúng cả hai.
    printf '# trap: a=1 # trap: a=2\n' >"$tmp/twice.bin"
    a="$(harness_scan_count "$tmp/twice.bin")"
    b="$(harness_scan_count_strings "$tmp/twice.bin")"
    if [ "$a" -eq 2 ] && [ "$b" -eq 1 ]; then
        _hs_ok "hai công thức giữ nguyên: occurrences=$a dòng-strings=$b trên cùng tệp"
    else
        _hs_bad "công thức đếm lệch: occurrences=$a (đòi 2) dòng-strings=$b (đòi 1)"
    fi
    # (6) danh sách rút gọn (cách của product-riscv64/soak cũ) vẫn quét đúng: 'ktest' + 3 marker.
    printf 'ktest\n' >"$tmp/narrow.bin"
    a="$(harness_scan_count "$tmp/narrow.bin" 'ktest' '# harness' 'not ok' 'TAP_TESTS')"
    b="$(harness_scan_count "$tmp/narrow.bin")"
    if [ "$a" -eq 1 ] && [ "$b" -eq 0 ]; then
        _hs_ok "danh sách rút gọn: 'ktest'=1, bộ 23 tiền tố=0 ⇒ truyền danh sách có tác dụng"
    else
        _hs_bad "danh sách rút gọn sai: rút gọn=$a (đòi 1) bộ-23=$b (đòi 0)"
    fi
    # (7) TOCTOU: tệp xuất hiện **sau** khi hàm bắt đầu ⇒ vẫn được chờ và đọc đúng hit.
    ( sleep 0.3; printf '# vspace: late\n' >"$tmp/late.bin" ) &
    local late_pid=$!
    a="$(HARNESS_SCAN_WAIT_MS=5000 harness_scan_count "$tmp/late.bin")"
    wait "$late_pid" 2>/dev/null || true
    if [ "$a" -eq 1 ]; then
        _hs_ok "TOCTOU: tệp xuất hiện muộn 0,3 s vẫn được chờ và đếm đúng (hit=$a)"
    else
        _hs_bad "TOCTOU: tệp xuất hiện muộn bị bỏ qua (hit=$a, đòi 1)"
    fi
    # (8) tệp vắng hết hạn ⇒ rc 1 + in 0, **không** kết luận "0 marker" cho một ảnh không đọc được.
    local out rc
    out="$(HARNESS_SCAN_WAIT_MS=0 harness_scan_count "$tmp/khong-co.bin" 2>/dev/null)"
    rc=$?
    if [ "$rc" -ne 0 ] && [ "$out" = "0" ]; then
        _hs_ok "tệp vắng: in 0 nhưng rc=$rc (không có '0 marker' giả)"
    else
        _hs_bad "tệp vắng: rc=$rc out=$out (đòi rc≠0, out=0)"
    fi

    rm -rf "$tmp" 2>/dev/null || true
    echo
    if [ "$fails" -gt 0 ]; then
        echo "harness-scan.sh --selftest: FAIL ($fails/$cases)" >&2
        return 1
    fi
    echo "harness-scan.sh --selftest: PASS ($cases/$cases)"
    return 0
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selftest) harness_scan_selftest ;;
        "" | -h | --help)
            echo "dùng: (source) v2/test/boot/lib/harness-scan.sh; trực tiếp: --selftest"
            ;;
        *)
            echo "harness-scan.sh: cờ lạ '$1' (chỉ --selftest)" >&2
            exit 2
            ;;
    esac
fi
