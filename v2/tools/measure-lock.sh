#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333c (D2-243 mục 3a) — **khoá độc quyền cấp máy đo** cho mọi phép đo dài (T2/soak/nightly).
#
# # Vì sao có tệp này
#
# Ngày 2026-11-22, hai `v2 gate --fast` chạy **song song** trên cùng máy đo (15:31–15:45):
#   * `git-guard` tranh `.git` (số worktree 77 → 78) ⇒ 5 bước đỏ **oán**;
#   * QEMU chồng tải ⇒ mọi số **thời gian** trong khoảng đó không dùng được;
#   * một con số "−43,9 %" **phải vô hiệu** (bằng chứng không còn neo).
# Cổng có khoá theo tài nguyên rời rạc (làn, `V2_RUN_TMP`, logdir riêng) nhưng **không** có gì cấm
# hai tiến trình cùng đo trên **một máy**. Tệp này là khoá đó: **fail-closed**, phạm vi máy đo.
#
# # Hợp đồng
#
#   measure_lock_acquire <nhãn>   giữ khoá; 0 = giữ được (kể cả lồng/được uỷ quyền),
#                                 1 = TỪ CHỐI (đang có tiến trình khác giữ), 2 = lỗi tạo khoá.
#   measure_lock_release          xoá khoá **chỉ khi** chủ là đúng PID của tiến trình này.
#                                 3 = bỏ qua vì khoá của tiến trình khác (KHÔNG xoá).
#   measure_lock_status           in ai đang giữ, từ bao lâu; 0 = đang giữ, 1 = trống.
#
# Cờ thoát hiểm **duy nhất** (phải in cảnh báo to): `V2_MEASURE_LOCK=off` — chỉ dùng khi chạy
# trên máy khác/không có khoá (bằng chứng **không** hợp lệ nếu có tiến trình khác cùng đo).
#
# # Cơ chế (không phụ thuộc GNU coreutils — macOS bash 3.2)
#
#   * **Nguyên tử**: `mkdir` trên `$TMPDIR/vifast-measure.lock` — chỉ một tiến trình tạo được.
#     Không `flock` (macOS không có), không `ln` hai bước.
#   * **Nội dung khoá** (`owner`, một dòng): `pid:host:start_epoch:nhãn`.
#   * **Stale**: PID trong khoá **không còn sống** (`kill -0`) **hoặc** đã giữ quá **6 giờ**
#     (`V2_MEASURE_LOCK_MAX_AGE`, giây) ⇒ tự phá kèm cảnh báo. Phá bằng `mv` cả thư mục rồi `rm`
#     (không `rm -rf` tại chỗ) để hai tiến trình cùng phá không xoá khoá vừa giành của nhau.
#   * **Owner `owner` đọc không được** (khoá vừa được tạo, tiến trình tạo chưa kịp ghi): **fail-closed**
#     — coi như đang giữ; chỉ phá khi cả thư mục đã quá 6 giờ.
#   * **Lồng trong cùng tiến trình**: `$$` trùng PID chủ ⇒ cho qua (do_gate_dev gọi do_gate_fast).
#   * **Được uỷ quyền**: tiến trình con (soak.sh do `v2 nightly` gọi) nhận
#     `V2_MEASURE_LOCK_DELEGATED_BY=<pid cha>` ⇒ dùng khoá của cha, **không** tạo khoá mới và
#     `release` của con **không** xoá khoá của cha.
#
# # Selftest
#
#   bash v2/tools/measure-lock.sh --selftest     # 8 ca (6 bắt buộc + 2 phụ), KHÔNG chạm khoá thật
#   bash v2/tools/measure-lock.sh status
#   bash v2/tools/measure-lock.sh acquire "<nhãn>"   # giữ tới khi tiến trình thoát (stale tự phá)
#   bash v2/tools/measure-lock.sh release
set -u

ML_SELF="${BASH_SOURCE[0]}"
ML_LOCK_DIR="${V2_MEASURE_LOCK_DIR:-${TMPDIR:-/tmp}/vifast-measure.lock}"
ML_MAX_AGE="${V2_MEASURE_LOCK_MAX_AGE:-21600}" # 6 giờ
ML_HOST="$(uname -n 2>/dev/null || hostname 2>/dev/null || echo unknown)"
# Trạng thái của **phiên hiện tại**: "" (chưa xin) | owner | reentrant | delegated | off
ML_STATE=""
ML_LABEL=""

ml_now() { date +%s 2>/dev/null || echo 0; }

ml_warn() { printf '# CẢNH BÁO (measure-lock): %s\n' "$*" >&2; }

ml_err() { printf 'measure-lock: %s\n' "$*" >&2; }

# Đọc `owner`; đặt ML_OP/ML_OH/ML_OS/ML_OLABEL. Trả 1 nếu không đọc được/không hợp lệ.
ml_read_owner() {
    local line pid host start label
    ML_OP=""
    ML_OH=""
    ML_OS="0"
    ML_OLABEL=""
    [ -f "$ML_LOCK_DIR/owner" ] || return 1
    line="$(cat "$ML_LOCK_DIR/owner" 2>/dev/null || true)"
    [ -n "$line" ] || return 1
    pid="${line%%:*}"
    line="${line#*:}"
    host="${line%%:*}"
    line="${line#*:}"
    start="${line%%:*}"
    label="${line#*:}"
    case "$pid" in '' | *[!0-9]*) return 1 ;; esac
    case "$start" in '' | *[!0-9]*) return 1 ;; esac
    ML_OP="$pid"
    ML_OH="$host"
    ML_OS="$start"
    ML_OLABEL="$label"
    return 0
}

# mtime (epoch) của một đường dẫn — macOS `stat -f`, Linux `stat -c`; 0 nếu không đọc được.
ml_mtime() {
    if stat -f %m "$1" >/dev/null 2>&1; then
        stat -f %m "$1" 2>/dev/null || echo 0
    elif stat -c %Y "$1" >/dev/null 2>&1; then
        stat -c %Y "$1" 2>/dev/null || echo 0
    else
        echo 0
    fi
}

# Phá khoá cũ **nguyên tử**: mv cả thư mục sang tên riêng rồi rm. Trả 0 nếu đã dọn.
ml_break_stale() {
    local stale="$ML_LOCK_DIR.stale.$$.$RANDOM"
    if mv "$ML_LOCK_DIR" "$stale" 2>/dev/null; then
        rm -rf "$stale" 2>/dev/null || true
        return 0
    fi
    return 1
}

ml_refuse() { # <pid> <host> <age> <nhãn> [lý do]
    printf '# TỪ CHỐI ĐO (measure-lock): máy đo đang bận — KHÔNG chạy chồng cổng/soak.\n' >&2
    printf '#   chủ khoá: pid=%s host=%s giữ %ss trước, nhãn="%s"%s\n' \
        "${1:-?}" "${2:-?}" "${3:-?}" "${4:-?}" "${5:-}"
    printf '#   hai cổng chạy song song làm hỏng số đo (sự cố KT-333a, 2026-11-22) ⇒ fail-closed.\n' >&2
    printf '#   khoá: %s (xem: bash v2/tools/measure-lock.sh status)\n' "$ML_LOCK_DIR" >&2
    printf '#   V2_MEASURE_LOCK=off CHỈ dùng khi chạy trên máy khác — không phải để né khoá trên máy đo.\n' >&2
}

# Giữ khoá. 0 = đã giữ (owner/reentrant/delegated/off), 1 = TỪ CHỐI, 2 = lỗi.
measure_lock_acquire() { # <nhãn>
    local label="${1:-khong-nhan}" now tries=0 age alive
    ML_LABEL="$label"
    ML_STATE=""
    if [ "${V2_MEASURE_LOCK:-on}" = "off" ]; then
        ML_STATE=off
        ml_warn "=========================================================="
        ml_warn "V2_MEASURE_LOCK=off — **KHÔNG** giữ khoá độc quyền máy đo!"
        ml_warn "Chỉ dùng trên máy khác/không tranh chấp. Nếu máy này đang có cổng/soak"
        ml_warn "khác chạy thì số đo (đặc biệt mọi số **thời gian**) là KHÔNG HỢP LỆ."
        ml_warn "=========================================================="
        return 0
    fi
    now="$(ml_now)"
    mkdir -p "$(dirname "$ML_LOCK_DIR")" 2>/dev/null || true
    while [ "$tries" -lt 5 ]; do
        tries=$((tries + 1))
        if mkdir "$ML_LOCK_DIR" 2>/dev/null; then
            if ! printf '%s:%s:%s:%s\n' "$$" "$ML_HOST" "$now" "$label" \
                >"$ML_LOCK_DIR/owner" 2>/dev/null; then
                ml_err "tạo được thư mục khoá nhưng KHÔNG ghi được owner — dọn và bỏ cuộc (fail-closed)"
                rm -rf "$ML_LOCK_DIR" 2>/dev/null || true
                return 2
            fi
            ML_STATE=owner
            echo "# measure-lock: ĐÃ GIỮ khoá máy đo (pid=$$ host=$ML_HOST nhãn=\"$label\")"
            return 0
        fi
        # Đã có người giữ.
        if ! ml_read_owner; then
            age=$((now - $(ml_mtime "$ML_LOCK_DIR")))
            [ "$age" -lt 0 ] && age=0
            if [ "$age" -gt "$ML_MAX_AGE" ]; then
                ml_warn "khoá không có owner đọc được và đã cũ (${age}s > ${ML_MAX_AGE}s) ⇒ phá"
                ml_break_stale || ml_warn "phá không được (tranh chấp) — thử lại"
                continue
            fi
            ml_refuse "?" "?" "${age}" "owner không đọc được — tiến trình tạo khoá chưa kịp ghi" \
                " (không phá vì fail-closed)"
            return 1
        fi
        if [ "$ML_OP" = "$$" ]; then
            ML_STATE=reentrant
            echo "# measure-lock: đã giữ khoá sẵn bởi chính tiến trình này (pid=$$, nhãn=\"${ML_OLABEL:-?}\") — lồng, không tạo khoá mới"
            return 0
        fi
        if [ -n "${V2_MEASURE_LOCK_DELEGATED_BY:-}" ] && [ "$V2_MEASURE_LOCK_DELEGATED_BY" = "$ML_OP" ]; then
            ML_STATE=delegated
            echo "# measure-lock: dùng khoá của tiến trình cha pid=$ML_OP (uỷ quyền qua V2_MEASURE_LOCK_DELEGATED_BY) — nhãn=\"${ML_OLABEL:-?}\""
            return 0
        fi
        alive=0
        kill -0 "$ML_OP" 2>/dev/null && alive=1
        age=$((now - ML_OS))
        [ "$age" -lt 0 ] && age=0
        if [ "$alive" -eq 0 ]; then
            ml_warn "phá khoá cũ: pid=$ML_OP KHÔNG còn sống (host=$ML_OH nhãn=\"${ML_OLABEL:-?}\")"
            ml_break_stale || ml_warn "phá không được (tranh chấp) — thử lại"
            continue
        fi
        if [ "$age" -gt "$ML_MAX_AGE" ]; then
            ml_warn "phá khoá cũ: quá hạn ${age}s > ${ML_MAX_AGE}s (pid=$ML_OP còn sống, nhãn=\"${ML_OLABEL:-?}\")"
            ml_break_stale || ml_warn "phá không được (tranh chấp) — thử lại"
            continue
        fi
        ml_refuse "$ML_OP" "$ML_OH" "$age" "$ML_OLABEL" ""
        return 1
    done
    ml_err "không giành được khoá sau $tries lần thử (tranh chấp liên tục) — TỪ CHỐI"
    return 1
}

# Chỉ xoá khi đúng PID của mình. 0 = đã xoá/không có gì để xoá; 3 = bỏ qua (khoá của người khác).
measure_lock_release() {
    if [ ! -d "$ML_LOCK_DIR" ]; then
        ML_STATE=""
        return 0
    fi
    if ! ml_read_owner; then
        ml_warn "release: khoá tồn tại nhưng owner không đọc được — KHÔNG xoá (fail-closed)"
        return 3
    fi
    if [ "$ML_OP" != "$$" ]; then
        ml_warn "release bị bỏ qua: khoá của pid=$ML_OP host=$ML_OH (không phải tôi $$) — KHÔNG xoá"
        return 3
    fi
    rm -f "$ML_LOCK_DIR/owner" 2>/dev/null || true
    if rmdir "$ML_LOCK_DIR" 2>/dev/null; then
        ML_STATE=""
        echo "# measure-lock: đã nhả khoá (pid=$$)"
        return 0
    fi
    # Có thể ai đó vừa tạo lại thư mục (hiếm): không xoá mù.
    ml_warn "release: owner đã xoá nhưng thư mục khoá còn (có tiến trình khác vừa tạo?) — không rmdir"
    return 0
}

# Nhả khoá **chỉ ở shell chính**: trap EXIT được subshell (làn soak) thừa kế; trong subshell
# `$$` vẫn là PID shell cha nên nếu nhả ở đó sẽ xoá khoá của cha (bash 3.2 không có `$BASHPID`).
measure_lock_release_if_top() {
    [ "${BASH_SUBSHELL:-0}" = "0" ] || return 0
    measure_lock_release
}

# 0 = đang có khoá; 1 = trống. In ai giữ, từ bao lâu, sống hay stale.
measure_lock_status() {
    local now age alive
    if [ ! -d "$ML_LOCK_DIR" ]; then
        if [ "${V2_MEASURE_LOCK:-on}" = "off" ]; then
            echo "measure-lock: KHÔNG khoá (V2_MEASURE_LOCK=off) — bằng chứng thời gian không hợp lệ nếu tranh chấp"
        else
            echo "measure-lock: TRỐNG (không ai giữ $ML_LOCK_DIR)"
        fi
        return 1
    fi
    now="$(ml_now)"
    if ! ml_read_owner; then
        age=$((now - $(ml_mtime "$ML_LOCK_DIR")))
        echo "measure-lock: ĐANG GIỮ (owner KHÔNG đọc được; thư mục ${age}s trước; fail-closed)"
        return 0
    fi
    age=$((now - ML_OS))
    [ "$age" -lt 0 ] && age=0
    alive=0
    kill -0 "$ML_OP" 2>/dev/null && alive=1
    if [ "$alive" -eq 1 ] && [ "$age" -le "$ML_MAX_AGE" ]; then
        echo "measure-lock: ĐANG GIỮ pid=$ML_OP host=$ML_OH từ ${age}s trước (nhãn=\"${ML_OLABEL:-?}\"; còn sống)"
    else
        if [ "$alive" -eq 0 ]; then
            echo "measure-lock: STALE pid=$ML_OP không còn sống (host=$ML_OH ${age}s, nhãn=\"${ML_OLABEL:-?}\") — lần acquire sau sẽ tự phá"
        else
            echo "measure-lock: STALE quá hạn ${age}s > ${ML_MAX_AGE}s (pid=$ML_OP còn sống, nhãn=\"${ML_OLABEL:-?}\") — lần acquire sau sẽ tự phá"
        fi
    fi
    return 0
}

# ---------------------------------------------------------------- selftest (không chạm khoá thật)
measure_lock_selftest() {
    local tmp ca=0 fails=0 out rc spid
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/ml-selftest.XXXXXX")" || {
        ml_err "không tạo được thư mục tạm"
        return 2
    }
    # Thư mục khoá **riêng của selftest** — không bao giờ chạm khoá thật của máy đo.
    ML_LOCK_DIR="$tmp/lock"
    : "${ML_LOCK_DIR:?}"
    unset V2_MEASURE_LOCK 2>/dev/null || true
    _ml_ok() { ca=$((ca + 1)); echo "ok $ca - $1"; }
    _ml_bad() { ca=$((ca + 1)); fails=$((fails + 1)); echo "not ok $ca - $1"; }

    # 1) giữ được + status thấy đúng chủ + đúng nhãn.
    out="$(measure_lock_acquire "ca-giu" 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && ml_read_owner && [ "$ML_OP" = "$$" ] && [ "$ML_OLABEL" = "ca-giu" ]; then
        if measure_lock_status | grep -q "ĐANG GIỮ pid=$$"; then
            _ml_ok "dương: acquire giữ khoá (pid=$$) + status in đúng chủ/nhãn"
        else
            _ml_bad "dương: acquire giữ khoá nhưng status không in đúng chủ"
        fi
    else
        _ml_bad "dương: acquire không giữ được khoá (rc=$rc)"
    fi

    # 2) **BẮT BUỘC**: giữ lần hai từ tiến trình KHÁC ⇒ TỪ CHỐI (khoá còn nguyên).
    out="$(V2_MEASURE_LOCK_DIR="$ML_LOCK_DIR" bash "$ML_SELF" acquire "ke-gian" 2>&1)"
    rc=$?
    if [ "$rc" -eq 1 ] && printf '%s' "$out" | grep -q "TỪ CHỐI ĐO"; then
        if ml_read_owner && [ "$ML_OP" = "$$" ]; then
            _ml_ok "âm: giữ khoá lần hai (tiến trình khác) ⇒ TỪ CHỐI, khoá còn của chủ cũ"
        else
            _ml_bad "âm: TỪ CHỐI nhưng khoá đã bị đổi chủ"
        fi
    else
        _ml_bad "âm: giữ khoá lần hai KHÔNG bị từ chối (rc=$rc) — khoá là dấu cao su"
    fi

    # 3) stale theo PID chết ⇒ tự phá kèm cảnh báo.
    measure_lock_release >/dev/null 2>&1
    mkdir -p "$ML_LOCK_DIR" || _ml_bad "không tạo được khoá giả (stale pid)"
    printf '%s:%s:%s:%s\n' "999999" "host-gia" "$(ml_now)" "da-chet" >"$ML_LOCK_DIR/owner"
    out="$(measure_lock_acquire "ca-stale-pid" 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "KHÔNG còn sống" &&
        ml_read_owner && [ "$ML_OP" = "$$" ]; then
        _ml_ok "âm: stale pid=999999 ⇒ tự phá + cảnh báo + giành được khoá"
    else
        _ml_bad "âm: stale pid không được phá (rc=$rc)"
    fi
    measure_lock_release >/dev/null 2>&1

    # 4) quá hạn 6 giờ (PID còn sống) ⇒ tự phá kèm cảnh báo.
    sleep 30 &
    spid=$!
    mkdir -p "$ML_LOCK_DIR" || _ml_bad "không tạo được khoá giả (quá hạn)"
    printf '%s:%s:%s:%s\n' "$spid" "host-gia" "$(($(ml_now) - ML_MAX_AGE - 60))" "qua-han" \
        >"$ML_LOCK_DIR/owner"
    out="$(measure_lock_acquire "ca-qua-han" 2>&1)"
    rc=$?
    kill "$spid" 2>/dev/null || true
    wait "$spid" 2>/dev/null || true
    if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "quá hạn" &&
        ml_read_owner && [ "$ML_OP" = "$$" ]; then
        _ml_ok "âm: quá hạn ${ML_MAX_AGE}s (pid còn sống) ⇒ tự phá + cảnh báo + giành được khoá"
    else
        _ml_bad "âm: quá hạn không được phá (rc=$rc)"
    fi

    # 5) release sai PID ⇒ KHÔNG xoá.
    out="$(V2_MEASURE_LOCK_DIR="$ML_LOCK_DIR" bash "$ML_SELF" release 2>&1)"
    rc=$?
    if [ "$rc" -eq 3 ] && [ -d "$ML_LOCK_DIR" ] && ml_read_owner && [ "$ML_OP" = "$$" ]; then
        _ml_ok "âm: release sai pid ⇒ rc=3, KHÔNG xoá khoá của chủ"
    else
        _ml_bad "âm: release sai pid đã xoá khoá hoặc rc lạ (rc=$rc)"
    fi

    # 6) release đúng PID ⇒ xoá.
    measure_lock_release >/dev/null 2>&1
    rc=$?
    if [ ! -d "$ML_LOCK_DIR" ]; then
        _ml_ok "dương: release đúng pid ⇒ xoá khoá"
    else
        _ml_bad "dương: release đúng pid KHÔNG xoá khoá (rc=$rc)"
    fi

    # 7) owner viết dở (đọc không được) ⇒ fail-closed, KHÔNG phá.
    mkdir -p "$ML_LOCK_DIR" || _ml_bad "không tạo được khoá giả (owner hỏng)"
    printf 'rac-khong-hop-le\n' >"$ML_LOCK_DIR/owner"
    out="$(measure_lock_acquire "ca-owner-hong" 2>&1)"
    rc=$?
    if [ "$rc" -eq 1 ] && printf '%s' "$out" | grep -q "TỪ CHỐI ĐO" && [ -d "$ML_LOCK_DIR" ]; then
        _ml_ok "âm: owner đọc không được ⇒ TỪ CHỐI (fail-closed), không phá"
    else
        _ml_bad "âm: owner hỏng không bị từ chối (rc=$rc)"
    fi
    rm -rf "$ML_LOCK_DIR" 2>/dev/null || true

    # 8) cờ thoát hiểm off ⇒ cho qua kèm cảnh báo to; release không xoá gì.
    mkdir -p "$ML_LOCK_DIR" || _ml_bad "không tạo được khoá giả (off)"
    printf '%s:%s:%s:%s\n' "424242" "host-gia" "$(ml_now)" "chu-khac" >"$ML_LOCK_DIR/owner"
    out="$(V2_MEASURE_LOCK=off measure_lock_acquire "ca-off" 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q "KHÔNG.*giữ khoá" && [ -d "$ML_LOCK_DIR" ]; then
        _ml_ok "dương: V2_MEASURE_LOCK=off ⇒ cho qua + cảnh báo to (không tạo/không xoá khoá)"
    else
        _ml_bad "dương: off không cho qua hoặc không cảnh báo (rc=$rc)"
    fi
    V2_MEASURE_LOCK=off measure_lock_release >/dev/null 2>&1
    if [ -d "$ML_LOCK_DIR" ]; then
        _ml_ok "âm: release khi off ⇒ KHÔNG xoá khoá của người khác"
    else
        _ml_bad "âm: release khi off đã xoá khoá của người khác"
    fi
    rm -rf "$ML_LOCK_DIR" 2>/dev/null || true
    rm -rf "$tmp" 2>/dev/null || true

    echo
    if [ "$fails" -gt 0 ]; then
        echo "measure-lock --selftest: FAIL ($fails/$ca)" >&2
        return 1
    fi
    echo "measure-lock --selftest: PASS ($ca/$ca ca)"
    return 0
}

ml_usage() {
    cat <<'EOF'
  bash v2/tools/measure-lock.sh acquire "<nhãn>"   # giữ khoá tới khi tiến trình thoát
  bash v2/tools/measure-lock.sh release            # chỉ xoá nếu đúng PID của mình
  bash v2/tools/measure-lock.sh status             # ai đang giữ, từ bao lâu
  bash v2/tools/measure-lock.sh --selftest         # 8 ca, không chạm khoá thật
  # env: V2_MEASURE_LOCK_DIR (mặc định $TMPDIR/vifast-measure.lock),
  #      V2_MEASURE_LOCK_MAX_AGE (mặc định 21600 s = 6 giờ),
  #      V2_MEASURE_LOCK=off (thoát hiểm — in cảnh báo to)
EOF
}

# Chỉ chạy CLI khi được **gọi trực tiếp**; khi `source` thì chỉ định nghĩa hàm.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    cmd="${1:-}"
    [ $# -gt 0 ] && shift
    case "$cmd" in
        acquire) measure_lock_acquire "${1:-khong-nhan}" && exit 0 || exit $? ;;
        release) measure_lock_release && exit 0 || exit $? ;;
        status) measure_lock_status ;;
        --selftest) measure_lock_selftest ;;
        "" | -h | --help | help) ml_usage ;;
        *)
            ml_err "lệnh không biết: $cmd"
            ml_usage >&2
            exit 2
            ;;
    esac
fi
