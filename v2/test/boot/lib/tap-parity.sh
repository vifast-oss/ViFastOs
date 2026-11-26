#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-408e** (D2-263) — **SSOT read-only** cho số ca TAP của ảnh kiểm, dùng chung cho **mọi**
# runner: khẳng định `soak-<arch>.sh:TAP_TESTS == run-<arch>.sh:EXPECT_TESTS ==
# kbin-<arch>/src/main.rs:TAP_TESTS` cho **cả** `aarch64` lẫn `x86_64`.
#
# # Vì sao có tệp này
#
# Peak 2 `2026-11-26` (`becc1756`) cho thấy cùng một ảnh kiểm mà **ba** nơi khai số ca TAP tách
# rời: `kbin-*/src/main.rs::TAP_TESTS` = `50`, `run-*.sh::EXPECT_TESTS` = `50`, nhưng
# `soak-aarch64.sh::TAP_TESTS` = `46` / `soak-x86_64.sh::TAP_TESTS` = `47`. `soak-aarch64.sh
# --smoke 1 1` đỏ **1.000/1.000** boot (971 s) vì `1..46` không có trong log — trong khi
# `--selftest` của chính soak vẫn XANH, vì nó chỉ chấm trên **fixture tổng hợp do nó tự dựng**.
# Tức là: selftest xanh không chứng minh soak khớp ảnh thật. Đây là lỗ hổng **nhân bản SSOT**,
# không phải một con số sai lẻ — vá tay `46→50`/`47→50` sẽ lệch lại ở task TAP kế tiếp.
#
# Tệp này đọc **mã thật** (không hardcode số nào, không đọc fixture) và **fail-closed**:
#   - thiếu tệp / không tìm thấy khai báo / khai báo không parse được / khai báo trùng ⇒ ĐỎ;
#   - ba khai báo lệch nhau (kể cả ±1) ⇒ ĐỎ, in đủ ba giá trị + lý do;
#   - soak quên **gọi** `tap_parity_cases` trong `--selftest` ⇒ ĐỎ (không cho tháo khoá im lặng).
#
# # Dùng
#
#   . "$HERE/lib/tap-parity.sh"        # trong `soak-*.sh`
#   tap_parity_check                   # 0 ⇔ soak==run==kbin (đọc cây thật), in lý do ĐỎ
#   tap_parity_cases <số-ca-đầu>    # 1 ca dương + 10 ca âm trên **bản sao tạm**; đặt
#                                      # TAP_PARITY_CASES / TAP_PARITY_FAILS để caller cộng tổng
#
# Tệp chỉ **đọc**; bản sao cho ca âm nằm trong `mktemp -d` và bị xoá khi xong. Không sửa `run-*`,
# không ghi vào repo, không chạy QEMU, không cần quyền đặc biệt. Số ca TAP là **hằng** của mỗi
# ảnh — đổi nó là đổi hợp đồng TAP, phải đồng bộ cả ba nơi (đây chính là điều tệp này cưỡng chế).
#
# rc: hàm trả 0 = XANH, 1 = ĐỎ (không `exit` — để runner tự quyết định).
set -u

# Thư mục `v2/test/boot/lib` + gốc repo, chốt **một lần** lúc source (không phụ thuộc `cwd`).
TAP_PARITY_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAP_PARITY_ROOT="$(cd "$TAP_PARITY_LIB_DIR/../../../.." && pwd)"

# `tap_parity_paths <gốc>`: đúng **2 cặp + 2** `kbin/*/src/main.rs`. Không quét thêm tệp nào —
# `gpu-x86_64.sh`/`product-*.sh` có kỳ vọng TAP khác do feature `gpu`, không thuộc SSOT này.
tap_parity_paths() {
    printf '%s\n' \
        "$1/v2/test/boot/soak-aarch64.sh" \
        "$1/v2/test/boot/soak-x86_64.sh" \
        "$1/v2/test/boot/run-aarch64.sh" \
        "$1/v2/test/boot/run-x86_64.sh" \
        "$1/v2/kernel/vifast-kbin-aarch64/src/main.rs" \
        "$1/v2/kernel/vifast-kbin-x86_64/src/main.rs"
}

# `_tp_ok_int <nhãn> <giá-trị>`: phải là số nguyên 1..999 (từ chối `abc`, rỗng, `46+1`, `0x32`,
# số âm). In lý do ra stderr khi hỏng.
_tp_ok_int() {
    local label="$1" v="$2"
    case "$v" in
        '' | *[!0-9]*)
            echo "# tap-parity: $label='${v}' không phải số nguyên (fail-closed)" >&2
            return 1
            ;;
    esac
    if [ "$v" -lt 1 ] || [ "$v" -gt 999 ]; then
        echo "# tap-parity: $label=$v ngoài [1,999] (fail-closed)" >&2
        return 1
    fi
    return 0
}

# `_tp_read <tệp> <loại>`: loại = soak | run | kbin. In **một** số ra stdout; lý do ĐỎ ra stderr.
# Parser **chặt**: đúng **một** dòng khai báo thuộc đúng khuôn của loại đó.
_tp_read() {
    local f="$1" kind="$2" hits="" n="" v="" pat1="" pat2=""

    [ -f "$f" ] || {
        echo "# tap-parity: thiếu tệp $f (fail-closed)" >&2
        return 1
    }

    case "$kind" in
        soak)
            # `TAP_TESTS=46` + comment cuối dòng được phép.
            hits="$(grep -E '^[[:space:]]*TAP_TESTS=' "$f" || true)"
            if [ -z "$hits" ]; then
                echo "# tap-parity: $f không có dòng 'TAP_TESTS=<n>'" >&2
                return 1
            fi
            n="$(printf '%s\n' "$hits" | grep -c '')"
            if [ "${n:-0}" -ne 1 ]; then
                echo "# tap-parity: $f có ${n:-0} dòng 'TAP_TESTS=', cần đúng 1" >&2
                return 1
            fi
            v="$(printf '%s\n' "$hits" |
                sed -nE 's/^[[:space:]]*TAP_TESTS=([0-9]+)([[:space:]]*#.*)?$/\1/p')"
            if [ -z "$v" ]; then
                echo "# tap-parity: $f có dòng 'TAP_TESTS=' nhưng không phải 'TAP_TESTS=<số>[ # …]'" >&2
                return 1
            fi
            ;;
        run)
            # Runner dùng `${EXPECT_TESTS:-50}` (cho phép ghi đè khi chạy tay) — nhận cả hai khuôn.
            pat1="$(grep -E '^[[:space:]]*EXPECT_TESTS="\$\{EXPECT_TESTS:-[0-9]+\}"[[:space:]]*$' "$f" || true)"
            pat2="$(grep -E '^[[:space:]]*EXPECT_TESTS=[0-9]+[[:space:]]*$' "$f" || true)"
            n=0
            [ -n "$pat1" ] && n=$((n + 1))
            [ -n "$pat2" ] && n=$((n + 1))
            if [ "$n" -ne 1 ]; then
                echo "# tap-parity: $f có $n dòng 'EXPECT_TESTS' hợp khuôn, cần đúng 1" >&2
                return 1
            fi
            if [ -n "$pat1" ]; then
                v="$(printf '%s\n' "$pat1" | sed -nE 's/.*EXPECT_TESTS:-([0-9]+)\}.*/\1/p')"
            else
                v="$(printf '%s\n' "$pat2" | sed -nE 's/^[[:space:]]*EXPECT_TESTS=([0-9]+)[[:space:]]*$/\1/p')"
            fi
            ;;
        kbin)
            # Hợp khuôn: hằng số thuần, hoặc `<n> + cfg!(feature = "gpu") as usize` (ảnh GPU cộng
            # thêm ca; không bật feature ⇒ đúng `<n>`). Mọi dạng khác ⇒ ĐỎ, không đoán.
            pat1="$(grep -E '^pub const TAP_TESTS: usize = [0-9]+;[[:space:]]*$' "$f" || true)"
            pat2="$(grep -E '^pub const TAP_TESTS: usize = [0-9]+ \+ cfg!\(feature = "gpu"\) as usize;[[:space:]]*$' "$f" || true)"
            n=0
            [ -n "$pat1" ] && n=$((n + 1))
            [ -n "$pat2" ] && n=$((n + 1))
            if [ "$n" -ne 1 ]; then
                echo "# tap-parity: $f có $n dòng 'pub const TAP_TESTS' hợp khuôn, cần đúng 1" >&2
                return 1
            fi
            if [ -n "$pat1" ]; then
                v="$(printf '%s\n' "$pat1" |
                    sed -nE 's/^pub const TAP_TESTS: usize = ([0-9]+);[[:space:]]*$/\1/p')"
            else
                v="$(printf '%s\n' "$pat2" |
                    sed -nE 's/^pub const TAP_TESTS: usize = ([0-9]+) \+ cfg!.*/\1/p')"
            fi
            ;;
        *)
            echo "# tap-parity: loại '${kind}' không hợp lệ (soak|run|kbin)" >&2
            return 1
            ;;
    esac

    _tp_ok_int "$kind($f)" "$v" || return 1
    printf '%s\n' "$v"
    return 0
}

# `tap_parity_check [gốc-repo]` — khẳng định soak==run==kbin cho **cả** aarch64 và x86_64, và
# rằng mỗi soak **có gọi** `tap_parity_cases` trong `--selftest`. 0 = XANH; 1 = ĐỎ.
tap_parity_check() {
    local root="${1:-$TAP_PARITY_ROOT}" bad=0 arch soak run kbin
    local a_soak="" a_run="" a_kbin="" b_soak="" b_run="" b_kbin=""

    for arch in aarch64 x86_64; do
        soak=""
        run=""
        kbin=""
        if ! soak="$(_tp_read "$root/v2/test/boot/soak-$arch.sh" soak)"; then soak=""; bad=1; fi
        if ! run="$(_tp_read "$root/v2/test/boot/run-$arch.sh" run)"; then run=""; bad=1; fi
        if ! kbin="$(_tp_read "$root/v2/kernel/vifast-kbin-$arch/src/main.rs" kbin)"; then kbin=""; bad=1; fi
        case "$arch" in
            aarch64) a_soak="$soak" a_run="$run" a_kbin="$kbin" ;;
            *) b_soak="$soak" b_run="$run" b_kbin="$kbin" ;;
        esac
        # soak phải **gọi** parity trong `--selftest`; mất lời gọi = tháo khoá im lặng ⇒ ĐỎ.
        if ! grep -qE '^[[:space:]]*tap_parity_cases([[:space:]]|$)' \
            "$root/v2/test/boot/soak-$arch.sh" 2>/dev/null; then
            echo "# tap-parity: soak-$arch.sh KHÔNG gọi tap_parity_cases trong --selftest (mất khoá)"
            bad=1
        fi
    done

    # Ba giá trị phải **bằng nhau** từng arch; in đủ ba khi lệch (không chỉ nói "lệch").
    if [ -n "$a_soak" ] && [ -n "$a_run" ] && [ -n "$a_kbin" ] &&
        { [ "$a_soak" != "$a_run" ] || [ "$a_run" != "$a_kbin" ]; }; then
        echo "# tap-parity: aarch64 LỆCH soak.TAP_TESTS=$a_soak run.EXPECT_TESTS=$a_run kbin.TAP_TESTS=$a_kbin"
        bad=1
    fi
    if [ -n "$b_soak" ] && [ -n "$b_run" ] && [ -n "$b_kbin" ] &&
        { [ "$b_soak" != "$b_run" ] || [ "$b_run" != "$b_kbin" ]; }; then
        echo "# tap-parity: x86_64 LỆCH soak.TAP_TESTS=$b_soak run.EXPECT_TESTS=$b_run kbin.TAP_TESTS=$b_kbin"
        bad=1
    fi

    [ "$bad" -eq 0 ] || return 1
    printf '# tap-parity: soak==run==kbin (aarch64=%s x86_64=%s)\n' "$a_soak" "$b_soak"
    return 0
}

# `_tp_copy_tree <đích> [gốc]` — bản sao **chỉ 6 tệp** của SSOT, đúng cây thư mục.
_tp_copy_tree() {
    local dst="$1" root="${2:-$TAP_PARITY_ROOT}" p rel
    mkdir -p "$dst/v2/test/boot" "$dst/v2/kernel/vifast-kbin-aarch64/src" \
        "$dst/v2/kernel/vifast-kbin-x86_64/src" || return 1
    while IFS= read -r p; do
        rel="${p#"$root"/}"
        cp "$p" "$dst/$rel" || return 1
    done < <(tap_parity_paths "$root")
    return 0
}

# `_tp_mutate <gốc> <đường-dẫn> <biểu-thức sed -E>` — sửa **một** tệp trong bản sao.
_tp_mutate() {
    local root="$1" rel="$2" expr="$3" f="$1/$2" t="$1/$2.new"
    sed -E "$expr" "$f" >"$t" || return 1
    mv "$t" "$f" || return 1
    return 0
}

# `tap_parity_cases [số-ca-đầu]` — 1 ca dương trên **cây thật** + 10 ca âm trên bản sao tạm:
# lệch +1/−1 ở soak/run/kbin (6 ca), thiếu tệp, parse hỏng, khai báo trùng, mất call-site.
# In `ok <n> - tap-parity: …` / `not ok <n> - …`; đặt TAP_PARITY_CASES / TAP_PARITY_FAILS.
tap_parity_cases() {
    local base="${1:-1}" root="$TAP_PARITY_ROOT" tmp good out v d n cases=0 fails=0

    TAP_PARITY_CASES=0
    TAP_PARITY_FAILS=0
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/tap-parity-selftest.XXXXXX")" || {
        echo "not ok $base - tap-parity: không tạo được thư mục tạm"
        TAP_PARITY_CASES=1
        TAP_PARITY_FAILS=1
        return 1
    }
    good="$tmp/good"
    if ! _tp_copy_tree "$good" "$root"; then
        echo "not ok $base - tap-parity: không sao chép được 6 tệp SSOT (cây thật)"
        rm -rf "$tmp" 2>/dev/null || true
        TAP_PARITY_CASES=1
        TAP_PARITY_FAILS=1
        return 1
    fi

    # (1) ca dương: cây thật phải XANH — nếu không, chính repo đang lệch SSOT.
    cases=1
    if out="$(tap_parity_check "$root" 2>&1)"; then
        echo "ok $((base)) - tap-parity: cây thật soak==run==kbin ($(printf '%s\n' "$out" | head -1 | sed 's/^# tap-parity: //'))"
    else
        echo "not ok $((base)) - tap-parity: cây thật ĐỎ (lệch SSOT thật!) — $(printf '%s\n' "$out" | head -1)"
        fails=$((fails + 1))
    fi

    # `_tp_neg <tên> <hàm-sửa>`: bản sao mới từ `good`, hàm sửa nó, rồi **đòi** parity ĐỎ có lý do.
    _tp_neg() {
        local name="$1" mutate="$2" dir="$tmp/case-$((cases + 1))"
        cases=$((cases + 1))
        n=$((base + cases - 1))
        rm -rf "$dir" 2>/dev/null || true
        if ! cp -R "$good" "$dir" 2>/dev/null; then
            echo "not ok $n - tap-parity: ca âm ($name) — không tạo được bản sao"
            fails=$((fails + 1))
            return 0
        fi
        if ! "$mutate" "$dir"; then
            echo "not ok $n - tap-parity: ca âm ($name) — harness không sửa được bản sao"
            fails=$((fails + 1))
            return 0
        fi
        if out="$(tap_parity_check "$dir" 2>&1)"; then
            echo "not ok $n - tap-parity: ca âm ($name) — parity vẫn XANH"
            fails=$((fails + 1))
            return 0
        fi
        printf '%s\n' "$out" | grep -q '^# tap-parity: ' || {
            echo "not ok $n - tap-parity: ca âm ($name) — ĐỎ nhưng không nêu lý do parity"
            fails=$((fails + 1))
            return 0
        }
        echo "ok $n - tap-parity: ca âm ($name) ⇒ ĐỎ ($(printf '%s\n' "$out" | grep '^# tap-parity: ' | head -1 | sed 's/^# tap-parity: //'))"
        return 0
    }

    # ---- 6 ca lệch ±1 (đọc giá trị **thật** rồi +1/−1, không hardcode 50).
    _mut_soak_a64_up() {
        v="$(_tp_read "$1/v2/test/boot/soak-aarch64.sh" soak)" || return 1
        _tp_mutate "$1" "v2/test/boot/soak-aarch64.sh" "s/^(TAP_TESTS=)[0-9]+/\1$((v + 1))/"
    }
    _mut_soak_x64_down() {
        v="$(_tp_read "$1/v2/test/boot/soak-x86_64.sh" soak)" || return 1
        _tp_mutate "$1" "v2/test/boot/soak-x86_64.sh" "s/^(TAP_TESTS=)[0-9]+/\1$((v - 1))/"
    }
    _mut_run_a64_up() {
        v="$(_tp_read "$1/v2/test/boot/run-aarch64.sh" run)" || return 1
        _tp_mutate "$1" "v2/test/boot/run-aarch64.sh" "s/(EXPECT_TESTS:-)[0-9]+/\1$((v + 1))/"
    }
    _mut_run_x64_down() {
        v="$(_tp_read "$1/v2/test/boot/run-x86_64.sh" run)" || return 1
        _tp_mutate "$1" "v2/test/boot/run-x86_64.sh" "s/(EXPECT_TESTS:-)[0-9]+/\1$((v - 1))/"
    }
    _mut_kbin_a64_up() {
        v="$(_tp_read "$1/v2/kernel/vifast-kbin-aarch64/src/main.rs" kbin)" || return 1
        _tp_mutate "$1" "v2/kernel/vifast-kbin-aarch64/src/main.rs" \
            "s/^(pub const TAP_TESTS: usize = )[0-9]+;/\1$((v + 1));/"
    }
    _mut_kbin_x64_down() {
        v="$(_tp_read "$1/v2/kernel/vifast-kbin-x86_64/src/main.rs" kbin)" || return 1
        _tp_mutate "$1" "v2/kernel/vifast-kbin-x86_64/src/main.rs" \
            "s/^(pub const TAP_TESTS: usize = )[0-9]+( \+ cfg!)/\1$((v - 1))\2/"
    }
    # ---- 4 ca fail-closed: thiếu tệp / parse hỏng / khai báo trùng / mất call-site.
    _mut_missing() { rm -f "$1/v2/test/boot/soak-aarch64.sh"; }
    _mut_unparseable() {
        _tp_mutate "$1" "v2/test/boot/soak-aarch64.sh" 's/^(TAP_TESTS=)[0-9]+/\1abc/'
    }
    _mut_dup() { printf 'TAP_TESTS=50\n' >>"$1/v2/test/boot/soak-x86_64.sh"; }
    _mut_nocall() {
        local f="$1/v2/test/boot/soak-x86_64.sh"
        grep -v -E '^[[:space:]]*tap_parity_cases([[:space:]]|$)' "$f" >"$f.new" || return 1
        mv "$f.new" "$f"
    }

    _tp_neg "soak-aarch64 lệch +1" _mut_soak_a64_up
    _tp_neg "soak-x86_64 lệch -1" _mut_soak_x64_down
    _tp_neg "run-aarch64 lệch +1" _mut_run_a64_up
    _tp_neg "run-x86_64 lệch -1" _mut_run_x64_down
    _tp_neg "kbin-aarch64 lệch +1" _mut_kbin_a64_up
    _tp_neg "kbin-x86_64 lệch -1" _mut_kbin_x64_down
    _tp_neg "thiếu tệp soak-aarch64.sh (fail-closed)" _mut_missing
    _tp_neg "TAP_TESTS không parse được (abc)" _mut_unparseable
    _tp_neg "TAP_TESTS khai báo trùng" _mut_dup
    _tp_neg "soak-x86_64 mất lời gọi tap_parity_cases" _mut_nocall

    rm -rf "$tmp" 2>/dev/null || true
    TAP_PARITY_CASES="$cases"
    TAP_PARITY_FAILS="$fails"
    [ "$fails" -eq 0 ]
}
