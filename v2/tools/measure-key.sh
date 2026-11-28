#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **F12 của audit hạ tầng (2026-11-28)** — **KHOÁ ĐO** của T2 + **từ chối** tệp `??` trong đường
# build. Trước bản vá, `do_gate_fast` **không** in `commit=`/`build-inputs=`/`dirty=`/
# `toolchain=`/`seed=` (AGENTS §6.3 đòi chúng làm **khoá đầu vào** của mọi phép đo), và
# `v2_preflight_t2` chỉ `CẢNH BÁO` khi có tệp `??` ⇒ một runner **untracked** trong `v2/test` có
# thể làm đỏ/xanh cổng mà log T2 vẫn ghi `dirty=0`, `HEAD` sạch; bằng chứng tái dùng (§6.3) bị coi
# là hợp lệ trong khi cây đo **khác commit**.
#
# # Hợp đồng
#
#   measure-key.sh [--gate-key|--check|--print] [<gốc-repo>]
#
#   --gate-key   (mặc định) = `--check` rồi `--print`: đúng thứ `do_gate_fast` gọi. Exit 1 nếu
#                **từ chối** (có `??` trong đường build, hoặc không đọc được `git status`).
#   --check      Chỉ kiểm `??`: 0 = không có tệp chưa theo dõi trong
#                `v2/{bin,test,tools,kernel,sim,user}`; 1 = TỪ CHỐI (in danh sách tệp + lý do).
#                Không đọc được trạng thái git ⇒ **fail-closed** = 1 (không chạy tiếp).
#   --print      In **một** khối khoá đo (một dòng `# measure-key:`), lấy dữ liệu từ
#                `v2/test/boot/build-rev.sh` — nguồn sự thật **duy nhất** cho `build-inputs`/
#                `dirty` (không tự băm lại, không bịa định nghĩa):
#                  # measure-key: commit=<full> build-inputs=<hex|unknown> dirty=<0|1> untracked=<n> toolchain=<...> seeds=1000
#                Kèm `# measure-key-override:` **khi** có biến môi trường đổi điều kiện đo
#                (`V2_TEST_JOBS/THREADS/SERIAL/ONE/SERIAL_ALL`, `V2_MEASURE_LOCK=off`,
#                `V2_PREFLIGHT_MIN_FREE_GIB`, `RUSTUP_TOOLCHAIN` lệch ghim) — F13/F11 của audit.
#   --selftest   Ca tự kiểm thuần bash (không QEMU, không build): fixture git trong thư mục tạm.
#
# Không in/không sửa ngưỡng nào: `seeds=1000` là **L1-smoke** (D2-256) mà `step_l1` đòi đúng
# dòng `# L1 OK: 1000 seed, <n> trace`; đổi seed là đổi phép đo ⇒ phải sửa ở đó, không ở đây.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
export V2_ROOT REPO_ROOT

# Nguồn sự thật của `build-inputs`/`dirty` (KT-89/D2-127/KT-134). `set -u` ở trên đã bật; tệp chỉ
# định nghĩa hàm + hằng, không chạy gì ở tầng ngoài.
# shellcheck source=/dev/null
. "$V2_ROOT/test/boot/build-rev.sh"

# Bề mặt = **đường build**: sửa một tệp ở đây đổi ảnh/phép kiểm, nên tệp `??` ở đây làm cây đo
# khác commit. Cố ý **không** gồm `v2/{agent,docs,artifacts}` (harness ghi log trong lúc đo —
# `build-rev.sh` cũng trừ chúng) và không gồm gốc repo (kho phiếu cũ).
MK_BUILD_ROOTS="v2/bin v2/test v2/tools v2/kernel v2/sim v2/user"
# L1-smoke (D2-256) — giá trị `step_l1` đòi trong dòng `# L1 OK: 1000 seed, …`.
MK_SEEDS_SMOKE=1000
# Số tệp tối đa in ra khi TỪ CHỐI (đủ để dọn, không làm ngập log).
MK_LIST_MAX=20

# `-c core.quotepath=false`: tên tệp không-ASCII (vd. tiếng Việt) in **nguyên văn** — nếu không,
# `git status` bọc chúng thành `"v2/test/boot/runner-l\341\272\241.sh"` và người đọc log không dọn được.
mk_git() { git -C "$1" -c core.quotepath=false --no-optional-locks "${@:2}" 2>/dev/null; }

# `mk_untracked <gốc> [pathspec…]` — số tệp **chưa theo dõi** (`--untracked-files=all` để đếm
# từng tệp, không gộp thư mục). Trả 1 nếu **không đọc được** trạng thái (caller fail-closed).
mk_untracked() {
    local root="$1" out
    shift || true
    out="$(git -C "$root" -c core.quotepath=false --no-optional-locks status --porcelain \
        --untracked-files=all --ignore-submodules=all -- "$@" 2>/dev/null)" || return 1
    printf '%s\n' "$out" | grep -c '^??' || true
}

mk_head() {
    local head
    head="$(vbrev_head "$1" 2>/dev/null || true)"
    case "$head" in
        [0-9a-f][0-9a-f]*) printf '%s' "$head" ;;
        *) printf 'unknown' ;;
    esac
}

# `mk_toolchain <gốc>` — toolchain **hiệu lực**: `RUSTUP_TOOLCHAIN` nếu có, ngược lại `channel` ghim
# trong `v2/rust-toolchain.toml` (cùng cách `v2/bin/lib/util.sh` làm — không đọc bản thứ hai).
mk_toolchain() {
    local root="$1" channel=""
    [ -f "$root/v2/rust-toolchain.toml" ] &&
        channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$root/v2/rust-toolchain.toml" | head -1)"
    if [ -n "${RUSTUP_TOOLCHAIN:-}" ]; then
        printf '%s' "$RUSTUP_TOOLCHAIN"
    elif [ -n "$channel" ]; then
        printf '%s' "$channel"
    else
        printf 'unknown'
    fi
}

# `mk_overrides` — in các biến **đổi điều kiện đo** đang được đặt (F11/F13): im lặng nếu không có.
mk_overrides() {
    local line="" v
    for v in V2_TEST_JOBS V2_TEST_THREADS V2_TEST_SERIAL V2_TEST_ONE V2_TEST_SERIAL_ALL \
        V2_MEASURE_LOCK V2_PREFLIGHT_MIN_FREE_GIB V2_ALLOW_MEASURE_OVERRIDE; do
        eval "_mkv=\${$v-__unset__}"
        if [ "$_mkv" != "__unset__" ]; then
            line="$line $v=$_mkv"
        fi
    done
    unset _mkv 2>/dev/null || true
    # `RUSTUP_TOOLCHAIN` chỉ là **override** khi nó lệch bản ghim (util.sh tự export bản ghim nên
    # trùng là bình thường, không phải ngoại lệ).
    if [ -n "${RUSTUP_TOOLCHAIN:-}" ] && [ -f "$1/v2/rust-toolchain.toml" ]; then
        v="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$1/v2/rust-toolchain.toml" | head -1)"
        [ "$RUSTUP_TOOLCHAIN" = "$v" ] || line="$line RUSTUP_TOOLCHAIN=$RUSTUP_TOOLCHAIN"
    fi
    # `CARGO_TARGET_DIR` đổi **sha ảnh** cùng commit (nợ P20/F14) ⇒ phải vào khoá đo khi có đặt.
    if [ -n "${CARGO_TARGET_DIR:-}" ]; then
        line="$line CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
    fi
    [ -n "$line" ] || return 0
    printf '# measure-key-override:%s\n' "$line"
    return 0
}

# `mk_check <gốc>` — TỪ CHỐI (1) khi có `??` trong đường build; 0 = sạch.
mk_check() {
    local root="$1" n all list
    if ! vbrev_toplevel "$root" >/dev/null 2>&1; then
        echo "--- T2 pre-flight: TỪ CHỐI — không phải git repo / không đọc được git tại $root (fail-closed, F12)" >&2
        return 1
    fi
    if ! all="$(mk_untracked "$root")"; then
        echo "--- T2 pre-flight: TỪ CHỐI — không đọc được 'git status' tại $root (fail-closed, F12)" >&2
        return 1
    fi
    # shellcheck disable=SC2086 # MK_BUILD_ROOTS là pathspec, cố ý tách theo khoảng trắng
    if ! n="$(mk_untracked "$root" $MK_BUILD_ROOTS)"; then
        echo "--- T2 pre-flight: TỪ CHỐI — không đọc được 'git status' cho đường build tại $root (fail-closed, F12)" >&2
        return 1
    fi
    case "$(printf '%s' "$n" | tr -d '[:space:]')" in
        '' | *[!0-9]*) n=0 ;;
    esac
    if [ "$n" -gt 0 ]; then
        echo "--- T2 pre-flight: TỪ CHỐI — $n tệp **chưa theo dõi** nằm trong đường build ($MK_BUILD_ROOTS):" >&2
        # shellcheck disable=SC2086
        list="$(mk_git "$root" status --porcelain --untracked-files=all --ignore-submodules=all \
            -- $MK_BUILD_ROOTS | grep '^??' | sed -e 's/^?? //' | head -n "$MK_LIST_MAX")"
        printf '%s\n' "$list" | sed 's/^/    /' >&2
        echo "    … (tối đa $MK_LIST_MAX dòng; xem đủ: git status --porcelain | grep '^??')" >&2
        echo "--- Fix: git add (hoặc chuyển ra ngoài repo) rồi chạy lại. Tệp ?? đổi phép đo mà" >&2
        echo "--- KHÔNG vào commit/dirty ⇒ bằng chứng T2 không neo được vào commit (AGENTS §6.3, F12)." >&2
        return 1
    fi
    return 0
}

# `mk_print <gốc>` — khối khoá đo **một dòng** (+ dòng override khi có). Luôn 0: `build-inputs`
# `unknown` là dữ liệu **thành thật** (build-rev fail-closed), không phải lý do tự ý đỏ cổng —
# `v2_preflight_t2`/`mk_check` mới là chỗ từ chối.
mk_print() {
    local root="$1" head inputs="unknown" dirty="1" untracked="?" n=""
    head="$(mk_head "$root")"
    if vbrev_inputs_calc "$root" >/dev/null 2>&1; then
        inputs="$VBREV_CALC_SHORT"
    else
        inputs="unknown"
    fi
    dirty="$(vbrev_dirty "$root")"
    case "$dirty" in 0 | 1) ;; *) dirty=1 ;; esac
    if n="$(mk_untracked "$root")"; then
        case "$(printf '%s' "$n" | tr -d '[:space:]')" in
            '' | *[!0-9]*) untracked="unknown" ;;
            *) untracked="$n" ;;
        esac
    else
        untracked="unknown"
    fi
    printf '# measure-key: commit=%s build-inputs=%s dirty=%s untracked=%s toolchain=%s seeds=%s\n' \
        "$head" "$inputs" "$dirty" "$untracked" "$(mk_toolchain "$root")" "$MK_SEEDS_SMOKE"
    if [ "$inputs" = "unknown" ]; then
        printf '# measure-key-note: build-inputs=unknown (%s) — ảnh/log không neo được vào nội dung nguồn (build-rev fail-closed)\n' \
            "${VBREV_CALC_ERROR:-unknown}"
    fi
    mk_overrides "$root"
    return 0
}

# ------------------------------------------------------------------ tự kiểm (không QEMU/build)
mk_selftest() {
    local tmp fails=0 cases=0 fix rc out d
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/v2-measure-key-selftest.XXXXXX")" || {
        echo "measure-key: không tạo được thư mục tạm" >&2
        return 1
    }
    _mk_ok() {
        cases=$((cases + 1))
        echo "ok $cases - $*"
    }
    _mk_bad() {
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - $*"
    }
    fix="$tmp/repo"
    mkdir -p "$fix/v2/bin" "$fix/v2/test/boot" "$fix/v2/tools" \
        "$fix/v2/kernel/k/src" "$fix/v2/sim/s/src" "$fix/v2/user/programs" "$fix/v2/agent" \
        "$fix/v2/docs" || {
        rm -rf "$tmp"
        return 1
    }
    (
        cd "$fix" || exit 1
        git init -q .
        git config user.email "f12@example.invalid"
        git config user.name "f12"
        printf '#!/bin/sh\necho v2\n' >v2/bin/v2
        printf '#!/bin/sh\n' >v2/test/boot/ktest.sh
        printf '#!/bin/sh\n' >v2/tools/run.sh
        printf 'pub fn k() {}\n' >v2/kernel/k/src/lib.rs
        printf 'fn s() {}\n' >v2/sim/s/src/lib.rs
        printf 'fn u() {}\n' >v2/user/programs/main.rs
        printf 'log\n' >v2/agent/JOURNAL.md
        printf 'doc\n' >v2/docs/x.md
        printf '[toolchain]\nchannel = "1.99.9"\n' >v2/rust-toolchain.toml
        git add -A
        git commit -qm "fixture sạch"
    ) || {
        rm -rf "$tmp"
        return 1
    }

    # (1) cây sạch ⇒ `--check` 0.
    if bash "$HERE/measure-key.sh" --check "$fix" >"$tmp/c1.log" 2>&1; then
        _mk_ok "cây sạch (không ??): --check = 0"
    else
        _mk_bad "cây sạch mà --check khác 0: $(tr '\n' '|' <"$tmp/c1.log")"
    fi

    # (2) `??` ở **cả 6 gốc đường build** ⇒ TỪ CHỐI (đủ mặt — chống bỏ sót một gốc).
    for d in v2/bin v2/test v2/tools v2/kernel v2/sim v2/user; do
        printf 'lạ\n' >"$fix/$d/untracked-moi.sh"
        if bash "$HERE/measure-key.sh" --check "$fix" >"$tmp/c2.log" 2>&1; then
            _mk_bad "?? trong $d KHÔNG bị từ chối (lỗ hổng F12)"
        else
            if grep -q "TỪ CHỐI" "$tmp/c2.log" && grep -q "untracked-moi.sh" "$tmp/c2.log"; then
                _mk_ok "?? trong $d ⇒ TỪ CHỐI + nêu tên tệp"
            else
                _mk_bad "?? trong $d bị từ chối nhưng thiếu lý do/tên tệp: $(tr '\n' '|' <"$tmp/c2.log")"
            fi
        fi
        rm -f "$fix/$d/untracked-moi.sh"
    done

    # (3) `??` NGOÀI đường build (`v2/agent`, `v2/docs`, gốc repo) ⇒ **không** chặn (đúng phạm vi:
    # harness ghi log trong lúc đo), nhưng vẫn đếm ở `untracked=`.
    printf 'nhiễu\n' >"$fix/v2/agent/noise.md"
    printf 'nhiễu\n' >"$fix/v2/docs/noise.md"
    printf 'nhiễu\n' >"$fix/phieu.txt"
    if bash "$HERE/measure-key.sh" --check "$fix" >"$tmp/c3.log" 2>&1; then
        _mk_ok "?? ngoài đường build (v2/agent, v2/docs, gốc) KHÔNG chặn (đúng phạm vi build-rev)"
    else
        _mk_bad "?? ngoài đường build bị chặn oan: $(tr '\n' '|' <"$tmp/c3.log")"
    fi

    # (4) khối khoá đo: đúng **thứ tự khoá** của F12 + `untracked=3` (đếm cả 3 tệp ngoài đường build).
    out="$(bash "$HERE/measure-key.sh" --print "$fix" 2>/dev/null)" || true
    printf '%s\n' "$out" >"$tmp/c4.log"
    if printf '%s\n' "$out" | grep -qE '^# measure-key: commit=[0-9a-f]{40} build-inputs=(unknown|[0-9a-f]{16}) dirty=[01] untracked=3 toolchain=[^ ]+ seeds=1000$'; then
        _mk_ok "khối khoá đo đúng định dạng: commit=<40 hex> build-inputs=<hex|unknown> dirty=<0|1> untracked=3 toolchain=… seeds=1000"
    else
        _mk_bad "khối khoá đo sai định dạng: $(grep '^# measure-key' "$tmp/c4.log" || echo '<không có>')"
    fi

    # (5) `dirty` **nhất quán** với build-rev (nguồn sự thật duy nhất), không tự tính lại.
    #     Fixture không có cargo workspace ⇒ build-rev fail-closed về `dirty=1`; điều phải kiểm là
    #     khoá đo **lấy đúng** giá trị đó.
    if [ "$(vbrev_dirty "$fix")" = "1" ]; then
        if printf '%s\n' "$out" | grep -q 'dirty=1'; then
            _mk_ok "dirty lấy từ build-rev (fixture không cargo metadata ⇒ fail-closed dirty=1)"
        else
            _mk_bad "dirty không khớp build-rev: $(grep '^# measure-key' "$tmp/c4.log" || echo '<không có>')"
        fi
    elif printf '%s\n' "$out" | grep -q "dirty=$(vbrev_dirty "$fix")"; then
        _mk_ok "dirty lấy từ build-rev (= $(vbrev_dirty "$fix"))"
    else
        _mk_bad "dirty không khớp build-rev"
    fi

    # (6) biến override phải **ghi ra** (F11/F13): đủ 5 biến `V2_TEST_*` + cờ thoát hiểm + ngưỡng đĩa.
    out="$(V2_TEST_JOBS=8 V2_TEST_THREADS=2 V2_TEST_SERIAL="kt30_stress" V2_TEST_ONE="vifast_core" \
        V2_TEST_SERIAL_ALL=1 V2_MEASURE_LOCK=off V2_PREFLIGHT_MIN_FREE_GIB=3 \
        bash "$HERE/measure-key.sh" --print "$fix" 2>/dev/null)" || true
    printf '%s\n' "$out" >"$tmp/c6.log"
    miss=""
    for v in V2_TEST_JOBS=8 V2_TEST_THREADS=2 V2_TEST_SERIAL=kt30_stress V2_TEST_ONE=vifast_core \
        V2_TEST_SERIAL_ALL=1 V2_MEASURE_LOCK=off V2_PREFLIGHT_MIN_FREE_GIB=3; do
        printf '%s\n' "$out" | grep -q "measure-key-override:.*$v" || miss="$miss $v"
    done
    if [ -z "$miss" ]; then
        _mk_ok "dòng override ghi đủ V2_TEST_JOBS/THREADS/SERIAL/ONE/SERIAL_ALL + V2_MEASURE_LOCK + V2_PREFLIGHT_MIN_FREE_GIB"
    else
        _mk_bad "override thiếu:$miss"
    fi

    # (7) **không** đặt biến ⇒ **không** có dòng override (không nhiễu log).
    out="$(bash "$HERE/measure-key.sh" --print "$fix" 2>/dev/null)" || true
    if printf '%s\n' "$out" | grep -q 'measure-key-override:'; then
        _mk_bad "không đặt biến mà vẫn in dòng override: $(printf '%s' "$out" | grep 'measure-key-override:' | tr '\n' '|')"
    else
        _mk_ok "không đặt biến override ⇒ không in dòng override"
    fi

    # (8) không phải git repo ⇒ TỪ CHỐI (fail-closed), không im lặng cho chạy tiếp.
    mkdir -p "$tmp/notrepo"
    if bash "$HERE/measure-key.sh" --check "$tmp/notrepo" >"$tmp/c8.log" 2>&1; then
        _mk_bad "thư mục không phải git repo mà --check = 0 (phải fail-closed)"
    else
        if grep -q 'TỪ CHỐI' "$tmp/c8.log"; then
            _mk_ok "không đọc được git ⇒ TỪ CHỐI (fail-closed)"
        else
            _mk_bad "không đọc được git nhưng thiếu thông điệp TỪ CHỐI: $(tr '\n' '|' <"$tmp/c8.log")"
        fi
    fi

    # (9) `--gate-key` = check + print: cây bẩn ⇒ rc 1 **và** không in khoá; cây sạch ⇒ rc 0 + khoá.
    rm -f "$fix/v2/agent/noise.md" "$fix/v2/docs/noise.md" "$fix/phieu.txt"
    if bash "$HERE/measure-key.sh" --gate-key "$fix" >"$tmp/c9a.log" 2>&1; then
        if grep -q '^# measure-key: commit=' "$tmp/c9a.log"; then
            _mk_ok "--gate-key trên cây sạch: rc=0 + in khối khoá đo"
        else
            _mk_bad "--gate-key trên cây sạch mà không in khối khoá đo"
        fi
    else
        _mk_bad "--gate-key trên cây sạch trả != 0: $(tr '\n' '|' <"$tmp/c9a.log")"
    fi
    printf 'lạ\n' >"$fix/v2/test/boot/untracked-moi.sh"
    if bash "$HERE/measure-key.sh" --gate-key "$fix" >"$tmp/c9b.log" 2>&1; then
        _mk_bad "--gate-key trên cây có ?? trong đường build trả 0 (phải TỪ CHỐI)"
    else
        if grep -q '^# measure-key: commit=' "$tmp/c9b.log"; then
            _mk_bad "--gate-key bị TỪ CHỐI mà vẫn in khoá đo (log gây nhầm)"
        else
            _mk_ok "--gate-key bị TỪ CHỐI: rc != 0 và KHÔNG in khoá đo"
        fi
    fi
    rm -f "$fix/v2/test/boot/untracked-moi.sh"

    rm -rf "$tmp" 2>/dev/null || true
    echo "# measure-key --selftest: $((cases - fails))/$cases ca đúng như phải"
    if [ "$fails" -ne 0 ]; then
        echo "# measure-key --selftest: FAIL ($fails/$cases)" >&2
        return 1
    fi
    echo "# measure-key --selftest: PASS"
    return 0
}

# ------------------------------------------------------------------ dispatch
MODE="${1:---gate-key}"
case "$MODE" in
    --check | --print | --gate-key) shift || true ;;
    --selftest | -h | --help) ;;
    *)
        # Cho phép `measure-key.sh <gốc>` như tài liệu hợp đồng ghi.
        case "$MODE" in
            /* | ./* | ../*) MODE="--gate-key" ;;
            *)
                echo "measure-key: tham số lạ [$MODE] (dùng --gate-key | --check | --print | --selftest)" >&2
                exit 2
                ;;
        esac
        ;;
esac
ROOT="${1:-$REPO_ROOT}"
case "$MODE" in
    --selftest)
        mk_selftest || exit 1
        exit 0
        ;;
    -h | --help)
        sed -n '2,60p' "$0"
        exit 0
        ;;
    --check)
        mk_check "$ROOT" || exit 1
        exit 0
        ;;
    --print)
        mk_print "$ROOT"
        exit 0
        ;;
    --gate-key)
        mk_check "$ROOT" || exit 1
        mk_print "$ROOT"
        exit 0
        ;;
esac
