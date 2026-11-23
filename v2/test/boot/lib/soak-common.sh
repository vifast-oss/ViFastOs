#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-347b** (D2-245/D2-252 Pha A) — phần **dùng chung** của 3 vòng soak boot
# (`v2/test/boot/{soak.sh,soak-x86_64.sh,soak-aarch64.sh}`).
#
# # Vì sao có tệp này
#
# Ba runner soak chép **cùng** một vòng: chia `1..N` cho `LANES` subshell, mỗi luồng gọi `boot_one`
# tuần tự, `wait` **đúng PID từng luồng** (không `wait` trần — aarch64 có TCP server nền), in
# `# soak: bắt đầu …`, rồi tổng hợp `ok/fail`, kiểm ảnh không đổi giữa soak, in `# soak: BUILD_ID…`,
# và kết luận `0/N` (PASS) / `SMOKE` luôn exit 1 / `N < n ≥ 3/p` ĐỎ. Ba bản lệch nhau ở chi tiết
# (x86_64 thêm `qemu_own`, aarch64 thêm DNS/TCP) nên sửa một lỗi tổng hợp phải sửa ba chỗ.
#
# Tệp này gom phần **giống nhau**: vòng luồng, tổng hợp, neo ảnh/sha, BUILD_ID, kết luận. Runner
# vẫn giữ phần riêng: `boot_one`, `check_boot_log`, build ảnh, cấu hình QEMU, `qemu_guard`, các
# phép kiểm đặc thù arch (DNS/TCP/qemu_own) — gọi qua `SOAK_EXTRA_CHECK`.
#
# # Hợp đồng **không retry** (D2-07)
#
# Mỗi slot `i` được gọi **đúng một lần** (`soak_common_lane_worker`); không có vòng thử lại, không
# `until`, không chạy lại boot đỏ. Flake là `T-INFRA`, không phải lý do chạy lại. `--smoke` **luôn**
# exit 1 (không phải bằng chứng cổng) — `soak_common_finish` giữ nguyên hợp đồng đó.
#
# # Ghi chú
#
# Tệp **không** có trạng thái toàn cục ngoài các biến `SOAK_*` do chính nó đặt, không `exit`, không
# `cd`; an toàn khi `source` dưới `set -u` (bash 3.2 của macOS). Chạy trực tiếp chỉ để tự kiểm:
# `--selftest` (không QEMU: dùng `boot` giả + kết quả giả).
set -u

_SOAK_COMMON_HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "$_SOAK_COMMON_HERE/harness-scan.sh"

# `soak_common_lane_worker <lane> <i> <N> <lanes> <boot_fn>`: gọi `boot_fn <i>` cho các slot của
# luồng này. **Một lần mỗi slot** — không retry (D2-07). Chạy trong subshell `&` của
# `soak_common_run_lanes`, nên `boot_fn` phải định nghĩa **trước** lời gọi (hàm được kế thừa).
soak_common_lane_worker() {
    local _lane="$1" i="$2" n="$3" lanes="$4" boot_fn="$5"
    while [ "$i" -le "$n" ]; do
        "$boot_fn" "$i" || true
        i=$((i + lanes))
    done
}

# `soak_common_run_lanes <N> <lanes> <boot_fn>`: chạy `lanes` luồng song song, chờ **đúng PID**
# từng luồng (không `wait` trần — sẽ chờ cả server nền của aarch64) và đặt `SOAK_WALL` (giây).
soak_common_run_lanes() {
    local n="$1" lanes="$2" boot_fn="$3" lane=1 pid lane_pids=""
    echo "# soak: bắt đầu N=$n lanes=$lanes $(date -u '+%H:%M:%SZ')"
    SECONDS=0
    while [ "$lane" -le "$lanes" ]; do
        soak_common_lane_worker "$lane" "$lane" "$n" "$lanes" "$boot_fn" &
        lane_pids="$lane_pids $!"
        lane=$((lane + 1))
    done
    for pid in $lane_pids; do
        wait "$pid"
    done
    SOAK_WALL="$SECONDS"
}

# `soak_common_image_sha <ảnh>`: sha256 (shasum nếu có, ngược lại sha256sum).
soak_common_image_sha() {
    local image="${1:-}"
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$image" 2>/dev/null | awk '{print $1}'
    else
        sha256sum "$image" 2>/dev/null | awk '{print $1}'
    fi
}

# `soak_common_assert_image_unchanged <sha-đầu> <ảnh>`: 0 ⇔ sha cuối **trùng** sha đầu (ảnh không
# đổi giữa soak); 1 + in lý do nếu khác (bằng chứng mất neo — D2-80). Đặt `SOAK_IMG_SHA_END`.
soak_common_assert_image_unchanged() {
    local start="${1:-}" image="${2:-}"
    SOAK_IMG_SHA_END="$(soak_common_image_sha "$image")"
    if [ -z "$SOAK_IMG_SHA_END" ] || [ "$SOAK_IMG_SHA_END" != "$start" ]; then
        echo "# soak: FAIL — ảnh đổi giữa soak (${start:-?} → ${SOAK_IMG_SHA_END:-?}): bằng chứng không còn neo (D2-80)"
        return 1
    fi
    return 0
}

# `soak_common_counts <results.txt>`: đếm `ok`/`fail`, in dòng `# soak: xong wall=… boot/s`, in
# danh sách boot đỏ (nếu có). Đặt `SOAK_OK_COUNT`, `SOAK_FAIL_COUNT`, `SOAK_TOTAL`, `SOAK_WALL_S`.
soak_common_counts() {
    local results="${1:-}" n="${2:-0}" wall="${3:-${SOAK_WALL:-0}}"
    SOAK_OK_COUNT="$(grep -c '^ok ' "$results" 2>/dev/null || true)"
    SOAK_FAIL_COUNT="$(grep -c '^fail ' "$results" 2>/dev/null || true)"
    case "${SOAK_OK_COUNT:-}" in '' | *[!0-9]*) SOAK_OK_COUNT=0 ;; esac
    case "${SOAK_FAIL_COUNT:-}" in '' | *[!0-9]*) SOAK_FAIL_COUNT=0 ;; esac
    SOAK_TOTAL=$((SOAK_OK_COUNT + SOAK_FAIL_COUNT))
    SOAK_WALL_S="$wall"
    echo "# soak: xong wall=${wall}s ($((n / (wall > 0 ? wall : 1))) boot/s); ok=$SOAK_OK_COUNT fail=$SOAK_FAIL_COUNT tổng=$SOAK_TOTAL/$n"
    if [ "$SOAK_FAIL_COUNT" -ne 0 ]; then
        echo "# soak: danh sách boot đỏ (lý do):"
        grep '^fail ' "$results" | sed 's/^/#   /'
    fi
}

# `soak_common_finish`: tổng hợp cuối **một chỗ** cho 3 runner. Đọc các biến:
#   SOAK_N SOAK_NEED SOAK_SMOKE SOAK_TAG SOAK_SMOKE_MSG SOAK_GIT_HEAD SOAK_IMG_SHA
#   SOAK_BUILD_ID SOAK_IMAGE SOAK_RESULTS SOAK_EXTRA_CHECK (tên hàm, tuỳ chọn; trả 1 ⇒ ĐỎ)
# Trả 0 ⇔ `0/N` (PASS); 1 ⇔ tổng thiếu, ảnh đổi, extra check đỏ, SMOKE, có boot đỏ, hoặc N < NEED.
soak_common_finish() {
    local n="${SOAK_N:-0}" need="${SOAK_NEED:-0}" smoke="${SOAK_SMOKE:-0}"
    local tag="${SOAK_TAG:-soak}" results="${SOAK_RESULTS:-}"
    soak_common_counts "$results" "$n" "${SOAK_WALL:-0}"
    if [ "$SOAK_TOTAL" -ne "$n" ]; then
        echo "# soak: FAIL — chỉ xử lý $SOAK_TOTAL/$n slot (không bao giờ xanh vì không chạy gì)"
        return 1
    fi
    soak_common_assert_image_unchanged "${SOAK_IMG_SHA:-}" "${SOAK_IMAGE:-}" || return 1
    echo "# soak: BUILD_ID=${SOAK_BUILD_ID:-?} sha256=${SOAK_IMG_SHA:-?} git=${SOAK_GIT_HEAD:-?}"
    echo "# soak: $SOAK_OK_COUNT/$n lỗi=$SOAK_FAIL_COUNT — 0/N chỉ là trần (QD §5.3); không retry (D2-07)"
    # Phép kiểm đặc thù arch (DNS/TCP/qemu_own/…) chạy **trước** nhánh SMOKE — đột biến của phép
    # kiểm đó phải ĐỎ kể cả ở smoke (giữ đúng thứ tự cũ của soak-aarch64/soak-x86_64).
    if [ -n "${SOAK_EXTRA_CHECK:-}" ]; then
        "$SOAK_EXTRA_CHECK" || return 1
    fi
    if [ "$smoke" -eq 1 ]; then
        echo "# soak: SMOKE N=$n < $need — KHÔNG phải bằng chứng cổng ${SOAK_SMOKE_MSG:-}"
        return 1
    fi
    if [ "$SOAK_FAIL_COUNT" -ne 0 ]; then
        echo "# ${tag}: FAIL ($SOAK_FAIL_COUNT/$n boot đỏ)"
        return 1
    fi
    if [ "$n" -lt "$need" ]; then
        echo "# ${tag}: FAIL — N=$n < $need (n ≥ 3/p)"
        return 1
    fi
    echo "# soak: 0/$n lỗi (PASS)"
    echo "# ${tag}: PASS"
    return 0
}

# ---------------------------------------------------------------- tự kiểm (không QEMU)
soak_common_selftest() {
    local fails=0 cases=0 tmp calls calls2
    _sc_ok() {
        cases=$((cases + 1))
        echo "ok $cases - $*"
    }
    _sc_bad() {
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - $*"
    }
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/soak-common-selftest.XXXXXX")" || {
        echo "soak-common: không tạo được thư mục tạm" >&2
        return 1
    }

    # (1) chia slot: N=10, 3 luồng ⇒ **đúng một lần** mỗi slot 1..10 (không trùng, không thiếu).
    #     Đây cũng là bằng chứng **không retry** (D2-07): mỗi boot chỉ được gọi một lần.
    calls="$tmp/calls.txt"
    : >"$calls"
    _fake_boot() { printf '%s\n' "$1" >>"$calls"; return 0; }
    soak_common_run_lanes 10 3 _fake_boot >"$tmp/lanes.log" 2>&1
    if [ "$(sort -n "$calls" | tr '\n' ' ')" = "1 2 3 4 5 6 7 8 9 10 " ] &&
        [ "$(wc -l <"$calls" | tr -d ' ')" -eq 10 ]; then
        _sc_ok "chia slot N=10 × 3 luồng: mỗi slot đúng 1 lần (10 lời gọi, không retry)"
    else
        _sc_bad "chia slot sai: '$(sort -n "$calls" | tr '\n' ' ')' ($(wc -l <"$calls") lời gọi)"
    fi
    if grep -q '^# soak: bắt đầu N=10 lanes=3 ' "$tmp/lanes.log" &&
        case "${SOAK_WALL:-x}" in '' | *[!0-9]*) false ;; *) true ;; esac; then
        _sc_ok "run_lanes in dòng bắt đầu + đặt SOAK_WALL (giây) = ${SOAK_WALL:-?}"
    else
        _sc_bad "run_lanes thiếu dòng bắt đầu hoặc SOAK_WALL='${SOAK_WALL:-}'"
    fi

    # (2) tổng hợp: 8 ok + 2 fail ⇒ đếm đúng, in dòng xong + danh sách đỏ.
    printf 'ok 1\nok 2\nok 3\nok 4\nok 5\nok 6\nok 7\nok 8\nfail 9 timeout-20s\nfail 10 panic=1\n' \
        >"$tmp/results.txt"
    soak_common_counts "$tmp/results.txt" 10 40 >"$tmp/counts.log" 2>&1
    if [ "$SOAK_OK_COUNT" -eq 8 ] && [ "$SOAK_FAIL_COUNT" -eq 2 ] && [ "$SOAK_TOTAL" -eq 10 ] &&
        grep -q 'wall=40s (0 boot/s); ok=8 fail=2 tổng=10/10' "$tmp/counts.log" &&
        grep -q '^#   fail 9 timeout-20s' "$tmp/counts.log"; then
        _sc_ok "tổng hợp: ok=8 fail=2 tổng=10/10 + in danh sách boot đỏ"
    else
        _sc_bad "tổng hợp sai: ok=$SOAK_OK_COUNT fail=$SOAK_FAIL_COUNT tổng=$SOAK_TOTAL"
    fi

    # (3) ảnh không đổi / đổi: cùng sha ⇒ 0; khác sha ⇒ 1 + nói rõ mất neo.
    head -c 32 /dev/zero >"$tmp/img-a"
    cp "$tmp/img-a" "$tmp/img-b"
    printf 'x' >>"$tmp/img-b"
    local sha_a sha_b
    sha_a="$(soak_common_image_sha "$tmp/img-a")"
    sha_b="$(soak_common_image_sha "$tmp/img-b")"
    if [ -n "$sha_a" ] && soak_common_assert_image_unchanged "$sha_a" "$tmp/img-a" >/dev/null 2>&1 &&
        ! soak_common_assert_image_unchanged "$sha_a" "$tmp/img-b" >"$tmp/chg.log" 2>&1 &&
        grep -q 'ảnh đổi giữa soak' "$tmp/chg.log"; then
        _sc_ok "neo ảnh: cùng sha ⇒ XANH; đổi sha ⇒ ĐỎ kèm lý do (D2-80)"
    else
        _sc_bad "neo ảnh sai (sha_a=$sha_a sha_b=$sha_b)"
    fi

    # (4) PASS đầy đủ: 0/N với N=NEED, 0 fail ⇒ rc 0 + BUILD_ID + PASS.
    _finish_setup() { # <N> <need> <smoke> <tag> <results>
        SOAK_N="$1"
        SOAK_NEED="$2"
        SOAK_SMOKE="$3"
        SOAK_TAG="$4"
        SOAK_RESULTS="$5"
        SOAK_WALL=40
        SOAK_IMAGE="$tmp/img-a"
        SOAK_IMG_SHA="$sha_a"
        SOAK_BUILD_ID=0123456789abcdef
        SOAK_GIT_HEAD=deadbeef
        SOAK_SMOKE_MSG="K5'"
        SOAK_EXTRA_CHECK=""
    }
    _ok_results() {
        : >"$tmp/all-ok.txt"
        local i=1
        while [ "$i" -le "$1" ]; do
            printf 'ok %s\n' "$i" >>"$tmp/all-ok.txt"
            i=$((i + 1))
        done
    }
    _ok_results 10
    _finish_setup 10 10 0 soak-x86_64.sh "$tmp/all-ok.txt"
    if soak_common_finish >"$tmp/finish-pass.log" 2>&1 &&
        grep -q '^# soak: BUILD_ID=0123456789abcdef sha256=' "$tmp/finish-pass.log" &&
        grep -q '^# soak: 10/10 lỗi=0' "$tmp/finish-pass.log" &&
        grep -q '^# soak: 0/10 lỗi (PASS)' "$tmp/finish-pass.log" &&
        grep -q '^# soak-x86_64.sh: PASS' "$tmp/finish-pass.log"; then
        _sc_ok "kết luận PASS: 0/10, BUILD_ID + sha + git in đúng, tag 'soak-x86_64.sh: PASS'"
    else
        _sc_bad "kết luận PASS sai: $(tail -4 "$tmp/finish-pass.log" | tr '\n' '|')"
    fi
    # (5) **SMOKE luôn exit 1** dù 0/N sạch (giữ nguyên hợp đồng "smoke không phải bằng chứng cổng").
    _finish_setup 10 1000 1 soak-aarch64.sh "$tmp/all-ok.txt"
    if ! soak_common_finish >"$tmp/finish-smoke.log" 2>&1 &&
        grep -q '^# soak: SMOKE N=10 < 1000' "$tmp/finish-smoke.log"; then
        _sc_ok "--smoke: 0/10 sạch nhưng rc=1 + in SMOKE (hợp đồng cũ giữ nguyên)"
    else
        _sc_bad "--smoke trả rc=0 hoặc thiếu dòng SMOKE"
    fi
    # (6) có boot đỏ ⇒ rc 1 + in 'FAIL (1/10 boot đỏ)'.
    _finish_setup 10 10 0 soak.sh "$tmp/results.txt"
    if ! soak_common_finish >"$tmp/finish-fail.log" 2>&1 &&
        grep -q '^# soak.sh: FAIL (2/10 boot đỏ)' "$tmp/finish-fail.log"; then
        _sc_ok "boot đỏ: rc=1 + 'FAIL (2/10 boot đỏ)'"
    else
        _sc_bad "boot đỏ không bị kết luận FAIL"
    fi
    # (7) thiếu slot (chỉ 9/10) ⇒ rc 1 + "chỉ xử lý 9/10" (không bao giờ xanh vì không chạy gì).
    head -9 "$tmp/all-ok.txt" >"$tmp/partial.txt"
    _finish_setup 10 10 0 soak.sh "$tmp/partial.txt"
    if ! soak_common_finish >"$tmp/finish-partial.log" 2>&1 &&
        grep -q 'chỉ xử lý 9/10 slot' "$tmp/finish-partial.log"; then
        _sc_ok "thiếu slot: rc=1 + 'chỉ xử lý 9/10 slot'"
    else
        _sc_bad "thiếu slot mà vẫn PASS"
    fi
    # (8) N < NEED (không smoke) ⇒ rc 1 + "N=5 < 10 (n ≥ 3/p)".
    _ok_results 5
    _finish_setup 5 10 0 soak-x86_64.sh "$tmp/all-ok.txt"
    if ! soak_common_finish >"$tmp/finish-need.log" 2>&1 &&
        grep -q 'N=5 < 10 (n ≥ 3/p)' "$tmp/finish-need.log"; then
        _sc_ok "N < n ≥ 3/p: rc=1 + 'N=5 < 10 (n ≥ 3/p)'"
    else
        _sc_bad "N < NEED mà vẫn PASS"
    fi
    # (9) `SOAK_EXTRA_CHECK` (DNS/TCP/qemu_own) ĐỎ ⇒ rc 1 **trước** nhánh SMOKE (giữ thứ tự cũ).
    _ok_results 10
    _finish_setup 10 1000 1 soak-aarch64.sh "$tmp/all-ok.txt"
    SOAK_EXTRA_CHECK=_fake_extra_bad
    _fake_extra_bad() { echo "# dns server: cục bộ không được dùng"; return 1; }
    if ! soak_common_finish >"$tmp/finish-extra.log" 2>&1 &&
        grep -q '^# dns server:' "$tmp/finish-extra.log" &&
        ! grep -q '^# soak: SMOKE' "$tmp/finish-extra.log"; then
        _sc_ok "extra check đỏ ⇒ rc=1 và chạy TRƯỚC nhánh SMOKE (đúng thứ tự cũ)"
    else
        _sc_bad "extra check đỏ không chặn đúng chỗ"
    fi
    # (10) **không retry khin boot đỏ** (D2-07): boot giả trả 1 cho slot lẻ; vòng luồng vẫn gọi
    #      **đúng 7 lần** cho 7 slot — không chạy lại slot đỏ.
    calls2="$tmp/calls-fail.txt"
    : >"$calls2"
    _fake_boot_fail() { printf '%s\n' "$1" >>"$calls2"; [ $(( $1 % 2 )) -eq 0 ]; }
    soak_common_run_lanes 7 2 _fake_boot_fail >/dev/null 2>&1
    if [ "$(sort -n "$calls2" | tr '\n' ' ')" = "1 2 3 4 5 6 7 " ] &&
        [ "$(wc -l <"$calls2" | tr -d ' ')" -eq 7 ]; then
        _sc_ok "boot đỏ KHÔNG bị chạy lại: 7 slot ⇒ đúng 7 lời gọi (D2-07)"
    else
        _sc_bad "có retry: '$(sort -n "$calls2" | tr '\n' ' ')' ($(wc -l <"$calls2") lời gọi, đòi 7)"
    fi

    rm -rf "$tmp" 2>/dev/null || true
    echo
    if [ "$fails" -gt 0 ]; then
        echo "soak-common.sh --selftest: FAIL ($fails/$cases)" >&2
        return 1
    fi
    echo "soak-common.sh --selftest: PASS ($cases/$cases)"
    return 0
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selftest) soak_common_selftest ;;
        "" | -h | --help)
            echo "dùng: (source) v2/test/boot/lib/soak-common.sh; trực tiếp: --selftest"
            ;;
        *)
            echo "soak-common.sh: cờ lạ '$1' (chỉ --selftest)" >&2
            exit 2
            ;;
    esac
fi
