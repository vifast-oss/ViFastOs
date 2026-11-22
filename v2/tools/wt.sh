#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245/D2-248 mục 2c) — **4 slot worktree cố định + cache build ấm**.
#
# # Vì sao
#
# Sự cố 2026-11-22: 9 worktree mở cùng lúc làm tràn ổ đĩa 12 GiB, mỗi task build **lạnh** vào
# `target-task/<mã>` rồi xoá ⇒ cùng một cây phải biên dịch lại từ đầu cho task kế. D2-245 chốt
# **đúng 4 slot** (`/tmp/vifast-wt-{1..4}`) + `CARGO_TARGET_DIR` **riêng theo slot** và **giữ lại**
# khi release ⇒ task sau trong cùng slot thừa hưởng build ấm (`target-task/slot-<n>`).
#
# # Lệnh
#
#   eval "$(bash v2/tools/wt.sh acquire 1 task/kt333d-v2lib main)"
#       cấp slot 1: tạo worktree `/tmp/vifast-wt-1` từ `main` trên nhánh mới
#       `task/kt333d-v2lib`; in `export CARGO_TARGET_DIR=<repo>/target-task/slot-1` +
#       `export V2_WT_SLOT=1` + `export V2_WT_PATH=<worktree>`. Từ chối nếu:
#       slot ngoài 1..4 / slot đang bận / nhánh đã được checkout ở worktree khác /
#       nhánh sai `git check-ref-format` / **đĩa trống < 5 GiB**.
#
#   bash v2/tools/wt.sh release <slot>
#       gỡ worktree của slot (giữ **nguyên** `target-task/slot-<n>` để build ấm) — không xoá nhánh
#       (nhánh có thể chưa merge; xoá nhánh là việc của lead sau khi merge).
#
#   bash v2/tools/wt.sh list          # 4 slot: trạng thái, nhánh, đường dẫn, target, đĩa trống
#   bash v2/tools/wt.sh prune         # chỉ dọn thư mục MỒ CÔI (không còn là worktree) + git prune
#   bash v2/tools/wt.sh --selftest    # 10 ca trên repo tạm — KHÔNG chạm repo thật
#
# # Env (mặc định = hành vi thật; selftest dùng để chạy trong thư mục tạm)
#
#   V2_WT_REPO            gốc repo (mặc định: gốc worktree chứa tệp này)
#   V2_WT_BASE            thư mục 4 slot (mặc định `/tmp/vifast-wt`)
#   V2_WT_TARGET_BASE     gốc `CARGO_TARGET_DIR` (mặc định `<repo>/target-task`)
#   V2_WT_MIN_FREE_GIB    ngưỡng đĩa trống trước khi cấp slot (mặc định 5)
#
# Exit: 0 = xong; 2 = dùng sai / từ chối (in lý do); 1 = lỗi git.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
DEFAULT_REPO="$(cd "$V2_ROOT/.." && pwd)"
REPO="${V2_WT_REPO:-$DEFAULT_REPO}"
BASE="${V2_WT_BASE:-/tmp/vifast-wt}"
TARGET_BASE="${V2_WT_TARGET_BASE:-$REPO/target-task}"
MIN_FREE_GIB="${V2_WT_MIN_FREE_GIB:-5}"
SLOTS="1 2 3 4"

wt_die() {
    echo "wt.sh: $*" >&2
    exit 2
}

# **Chuẩn hoá đường dẫn** (bỏ `//`, `/var` → `/private/var` trên macOS): `git worktree list`
# in đường dẫn **vật lý**; nếu ta so bằng chuỗi chưa chuẩn hoá thì mọi slot bị coi là "trống"
# và `prune` xoá nhầm worktree đang dùng. Dùng `pwd -P` (physical).
wt_norm_dir() { # <thư mục> [tạo-nếu-thiếu]
    local d="$1" mk="${2:-0}"
    if [ ! -d "$d" ] && [ "$mk" = "1" ]; then
        mkdir -p "$d" 2>/dev/null || return 1
    fi
    (cd "$d" 2>/dev/null && pwd -P) || printf '%s' "$d"
}

REPO="$(wt_norm_dir "$REPO")" || wt_die "không vào được repo $REPO"
BASE="$(wt_norm_dir "$BASE" 1)" || wt_die "không tạo được $BASE"
TARGET_BASE="$(wt_norm_dir "$TARGET_BASE" 1)" || wt_die "không tạo được $TARGET_BASE"

wt_err() { echo "wt.sh: $*" >&2; }

wt_git() { git -C "$REPO" "$@"; }

wt_slot_ok() {
    case "${1:-}" in
        1 | 2 | 3 | 4) return 0 ;;
        *) return 1 ;;
    esac
}

wt_slot_dir() { printf '%s/%s' "$BASE" "$1"; }
wt_slot_target() { printf '%s/slot-%s' "$TARGET_BASE" "$1"; }

# Dung lượng trống (KiB) của thư mục chứa <đường-dẫn>; rỗng nếu không đọc được.
wt_free_kb() {
    local d="$1"
    while [ -n "$d" ] && [ ! -d "$d" ]; do
        d="$(dirname "$d")"
    done
    df -Pk "$d" 2>/dev/null | awk 'NR==2 { print $4 }'
}

# Trạng thái một slot: in "<trống|bận>\t<nhánh|->\t<đường-dẫn>".
wt_slot_state() {
    local n="$1" dir branch
    dir="$(wt_slot_dir "$n")"
    if [ -e "$dir" ] && wt_git worktree list --porcelain 2>/dev/null | grep -qxF "worktree $dir"; then
        branch="$(wt_git worktree list --porcelain 2>/dev/null |
            awk -v d="worktree $dir" '$0 == d { f = 1; next } f && /^branch / { sub(/^branch refs\/heads\//, ""); print; exit }')"
        printf 'bận\t%s\t%s' "${branch:--}" "$dir"
        return 0
    fi
    printf 'trống\t-\t%s' "$dir"
}

# Nhánh <nhánh> đang được checkout ở worktree nào? In đường dẫn (rỗng = không).
wt_branch_worktree() {
    local branch="$1"
    wt_git worktree list --porcelain 2>/dev/null | awk -v b="branch refs/heads/$branch" '
        /^worktree / { w = $2 }
        $0 == b { print w; exit }'
}

wt_valid_branch() {
    local b="${1:-}"
    [ -n "$b" ] || return 1
    wt_git check-ref-format --branch "$b" >/dev/null 2>&1
}

# ---------------------------------------------------------------- lệnh
wt_acquire() { # <slot> <nhánh> [base]
    local n="${1:-}" branch="${2:-}" base="${3:-HEAD}" dir target free_kb min_kb here
    wt_slot_ok "$n" || wt_die "slot phải là 1..4 (nhận '${n:-}')"
    wt_valid_branch "$branch" || wt_die "nhánh không hợp lệ theo 'git check-ref-format --branch': '${branch:-}'"
    dir="$(wt_slot_dir "$n")"
    target="$(wt_slot_target "$n")"
    min_kb=$((MIN_FREE_GIB * 1024 * 1024))

    # Đĩa phải đủ **trước** khi tạo gì (D2-248: `< 5 GiB` ⇒ từ chối).
    mkdir -p "$BASE" 2>/dev/null || wt_die "không tạo được $BASE"
    free_kb="$(wt_free_kb "$BASE")"
    case "${free_kb:-}" in
        '' | *[!0-9]*) wt_err "không đọc được dung lượng trống của $BASE — vẫn kiểm các điều kiện khác" ;;
        *)
            if [ "$free_kb" -lt "$min_kb" ]; then
                wt_die "đĩa trống $((free_kb / 1024 / 1024)) GiB < ${MIN_FREE_GIB} GiB tại $BASE — TỪ CHỐI cấp slot (D2-248). Dọn: bash v2/tools/wt.sh prune"
            fi
            ;;
    esac

    # Slot đang bận?
    if [ -e "$dir" ]; then
        if wt_git worktree list --porcelain 2>/dev/null | grep -qxF "worktree $dir"; then
            wt_die "slot $n đang bận ($dir) — release trước: bash v2/tools/wt.sh release $n"
        fi
        wt_die "slot $n có thư mục lạ không phải worktree: $dir — kiểm tay rồi xoá (không tự xoá)"
    fi

    # Nhánh đã được checkout ở worktree khác?
    here="$(wt_branch_worktree "$branch")"
    if [ -n "$here" ]; then
        wt_die "nhánh '$branch' đã được checkout tại $here — một nhánh không thể ở hai worktree"
    fi

    # Nhánh đã tồn tại (nhưng chưa checkout) ⇒ gắn vào; chưa có ⇒ tạo mới từ <base>.
    if wt_git show-ref --verify --quiet "refs/heads/$branch"; then
        wt_git worktree add "$dir" "$branch" || return 1
        wt_err "dùng lại nhánh sẵn có '$branch' (không tạo mới)"
    else
        wt_git worktree add -b "$branch" "$dir" "$base" || return 1
    fi
    mkdir -p "$target" || return 1
    echo "# wt.sh: slot $n sẵn sàng — nhánh '$branch', worktree $dir, cache $target (ấm giữa các task)"
    printf "export V2_WT_SLOT='%s'\n" "$n"
    printf "export V2_WT_PATH='%s'\n" "$dir"
    printf "export CARGO_TARGET_DIR='%s'\n" "$target"
    return 0
}

wt_release() { # <slot>
    local n="${1:-}" dir
    wt_slot_ok "$n" || wt_die "slot phải là 1..4 (nhận '${n:-}')"
    dir="$(wt_slot_dir "$n")"
    if [ ! -e "$dir" ]; then
        wt_die "slot $n đang trống (không có $dir)"
    fi
    if ! wt_git worktree list --porcelain 2>/dev/null | grep -qxF "worktree $dir"; then
        wt_die "slot $n có thư mục nhưng KHÔNG phải worktree của $REPO: $dir (không tự xoá)"
    fi
    wt_git worktree remove --force "$dir" || return 1
    # KHÔNG xoá `target-task/slot-<n>`: đó là cache ấm cho task kế trong cùng slot (D2-245).
    echo "# wt.sh: đã gỡ worktree slot $n; GIỮ cache $(wt_slot_target "$n") để build ấm"
    return 0
}

wt_list() {
    local n state free_kb min_kb target
    min_kb=$((MIN_FREE_GIB * 1024 * 1024))
    free_kb="$(wt_free_kb "$BASE")"
    printf '# wt.sh: repo=%s base=%s target-base=%s (đĩa trống %s GiB, ngưỡng %s GiB)\n' \
        "$REPO" "$BASE" "$TARGET_BASE" \
        "$( [ -n "${free_kb:-}" ] && echo "$((free_kb / 1024 / 1024))" || echo '?')" "$MIN_FREE_GIB"
    for n in $SLOTS; do
        target="$(wt_slot_target "$n")"
        if [ -d "$target" ]; then
            printf 'slot %s\t%-6s\t%-40s\t%s\n' "$n" "$(wt_slot_state "$n" | cut -f1)" \
                "$(wt_slot_state "$n" | cut -f2)" "$target (ấm)"
        else
            printf 'slot %s\t%-6s\t%-40s\t%s\n' "$n" "$(wt_slot_state "$n" | cut -f1)" \
                "$(wt_slot_state "$n" | cut -f2)" "$target"
        fi
    done
    return 0
}

wt_prune() {
    local dir removed=0 list
    wt_git worktree prune || return 1
    list="$(wt_git worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p')"
    # Quét **mọi** thư mục con của BASE (kể cả tên không phải slot): cái nào không còn là
    # worktree đã đăng ký ⇒ mồ côi, dọn. Không đụng thư mục đang là worktree.
    [ -d "$BASE" ] || return 0
    for dir in "$BASE"/*; do
        [ -d "$dir" ] || continue
        if printf '%s
' "$list" | grep -qxF "$dir"; then
            continue
        fi
        rm -rf -- "$dir" || wt_err "không xoá được thư mục mồ côi $dir"
        echo "# wt.sh: đã dọn thư mục mồ côi $dir"
        removed=$((removed + 1))
    done
    echo "# wt.sh: prune xong (git worktree prune + $removed thư mục mồ côi)"
    return 0
}

wt_usage() {
    cat <<'EOF'
  eval "$(bash v2/tools/wt.sh acquire <1..4> <nhánh> [base])"   # cấp slot + CARGO_TARGET_DIR ấm
  bash v2/tools/wt.sh release <1..4>                            # gỡ worktree, GIỮ cache target
  bash v2/tools/wt.sh list                                      # 4 slot + đĩa trống
  bash v2/tools/wt.sh prune                                     # dọn thư mục mồ côi
  bash v2/tools/wt.sh --selftest                                # 10 ca trên repo tạm

  Ngưỡng đĩa: V2_WT_MIN_FREE_GIB (mặc định 5). Slot mặc định: /tmp/vifast-wt/<n>.
  Cache: <repo>/target-task/slot-<n> (giữ qua release ⇒ build ấm, D2-245).
EOF
}

# ---------------------------------------------------------------- --selftest (repo tạm)
wt_selftest() {
    local tmp ca=0 fails=0 rc out repo
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/wt-selftest.XXXXXX")" || {
        wt_err "không tạo được thư mục tạm"
        return 2
    }
    tmp="$(cd "$tmp" && pwd -P)" # bỏ '//' + '/var'→'/private/var' để so với git worktree list
    repo="$tmp/repo"
    mkdir -p "$repo" || return 2
    # Repo tạm **hoàn toàn tách biệt** — không chạm repo thật.
    git -C "$repo" init -q || return 2
    git -C "$repo" config user.email "kt333d@example.invalid"
    git -C "$repo" config user.name "kt333d"
    printf 'x\n' >"$repo/a.txt"
    git -C "$repo" add a.txt
    git -C "$repo" commit -qm init
    git -C "$repo" checkout -qb main 2>/dev/null || git -C "$repo" branch -M main
    _w_ok() { ca=$((ca + 1)); echo "ok $ca - $1"; }
    _w_bad() { ca=$((ca + 1)); fails=$((fails + 1)); echo "not ok $ca - $1"; }
    # Chạy wt.sh với repo tạm; `$@` là tham số của lệnh.
    _w() {
        V2_WT_REPO="$repo" V2_WT_BASE="$tmp/wt" V2_WT_TARGET_BASE="$tmp/target-task" \
            V2_WT_MIN_FREE_GIB="${W_MIN:-0}" bash "$0" "$@"
    }

    # (1) cấp slot 1: worktree + cache target được tạo, in đúng 3 dòng export.
    out="$(_w acquire 1 task/mot main 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && [ -e "$tmp/wt/1" ] && [ -d "$tmp/target-task/slot-1" ] &&
        printf '%s' "$out" | grep -q "CARGO_TARGET_DIR='$tmp/target-task/slot-1'" &&
        printf '%s' "$out" | grep -q "V2_WT_SLOT='1'"; then
        _w_ok "cấp slot 1: worktree + CARGO_TARGET_DIR=target-task/slot-1 (cache ấm)"
    else
        _w_bad "cấp slot 1 thất bại (rc=$rc): $out"
    fi
    # (2) slot đang bận ⇒ từ chối.
    out="$(_w acquire 1 task/hai main 2>&1)"
    rc=$?
    if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q "đang bận"; then
        _w_ok "slot đang bận ⇒ TỪ CHỐI (rc=2)"
    else
        _w_bad "slot đang bận KHÔNG bị từ chối (rc=$rc)"
    fi
    # (3) slot ngoài 1..4 ⇒ từ chối.
    out="$(_w acquire 5 task/nam main 2>&1)"
    rc=$?
    if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q "1..4"; then
        _w_ok "slot 5 ⇒ TỪ CHỐI (rc=2)"
    else
        _w_bad "slot 5 KHÔNG bị từ chối (rc=$rc)"
    fi
    # (4) nhánh đã checkout ở slot 1 ⇒ slot 2 cùng nhánh bị từ chối.
    out="$(_w acquire 2 task/mot main 2>&1)"
    rc=$?
    if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q "đã được checkout"; then
        _w_ok "nhánh trùng (đã ở slot 1) ⇒ TỪ CHỐI (rc=2)"
    else
        _w_bad "nhánh trùng KHÔNG bị từ chối (rc=$rc)"
    fi
    # (5) đĩa trống thấp hơn ngưỡng ⇒ từ chối **trước khi** tạo gì.
    out="$(W_MIN=99999999 _w acquire 3 task/ba main 2>&1)"
    rc=$?
    if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q "TỪ CHỐI cấp slot" && [ ! -e "$tmp/wt/3" ]; then
        _w_ok "df < ngưỡng ⇒ TỪ CHỐI, KHÔNG tạo worktree (rc=2)"
    else
        _w_bad "df thấp KHÔNG bị từ chối đúng (rc=$rc, worktree=$([ -e "$tmp/wt/3" ] && echo có || echo không))"
    fi
    # (6) nhánh sai định dạng ⇒ từ chối.
    out="$(_w acquire 3 'nhánh..sai' main 2>&1)"
    rc=$?
    if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q "không hợp lệ"; then
        _w_ok "nhánh sai 'git check-ref-format' ⇒ TỪ CHỐI (rc=2)"
    else
        _w_bad "nhánh sai KHÔNG bị từ chối (rc=$rc)"
    fi
    # (7) list thấy slot 1 bận, slot 2..4 trống.
    out="$(_w list 2>&1)"
    if printf '%s' "$out" | grep -q "slot 1.*bận" && printf '%s' "$out" | grep -q "slot 4.*trống"; then
        _w_ok "list: slot 1 bận, slot 4 trống"
    else
        _w_bad "list sai trạng thái slot: $out"
    fi
    # (8) release slot 1: gỡ worktree nhưng **GIỮ** cache target (build ấm).
    out="$(_w release 1 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$tmp/wt/1" ] && [ -d "$tmp/target-task/slot-1" ] &&
        printf '%s' "$out" | grep -q "GIỮ cache"; then
        _w_ok "release: gỡ worktree, GIỮ target-task/slot-1 (cache ấm)"
    else
        _w_bad "release sai (rc=$rc, worktree=$([ -e "$tmp/wt/1" ] && echo còn || echo mất), cache=$([ -d "$tmp/target-task/slot-1" ] && echo còn || echo mất))"
    fi
    # (9) release slot trống ⇒ rc=2.
    out="$(_w release 4 2>&1)"
    rc=$?
    if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q "đang trống"; then
        _w_ok "release slot trống ⇒ TỪ CHỐI (rc=2)"
    else
        _w_bad "release slot trống KHÔNG bị từ chối (rc=$rc)"
    fi
    # (10) nhánh sẵn có (chưa checkout) được **dùng lại**, không tạo mới.
    git -C "$repo" branch task/san-co main
    out="$(_w acquire 2 task/san-co main 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && [ -e "$tmp/wt/2" ] &&
        printf '%s' "$out" | grep -q "dùng lại nhánh sẵn có"; then
        _w_ok "nhánh sẵn có ⇒ dùng lại (không tạo mới), cấp slot 2"
    else
        _w_bad "nhánh sẵn có không được dùng lại (rc=$rc): $out"
    fi
    _w release 2 >/dev/null 2>&1 || true
    # (11) prune chỉ dọn thư mục mồ côi, không đụng slot đang bận.
    _w acquire 3 task/ba main >/dev/null 2>&1
    mkdir -p "$tmp/wt/mo-coi"
    out="$(_w prune 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$tmp/wt/mo-coi" ] && [ -e "$tmp/wt/3" ]; then
        _w_ok "prune: dọn thư mục mồ côi, GiỮ worktree đang dùng"
    else
        _w_bad "prune sai (rc=$rc): $out"
    fi

    rm -rf "$tmp" 2>/dev/null || true
    echo
    if [ "$fails" -gt 0 ]; then
        echo "wt.sh --selftest: FAIL ($fails/$ca)" >&2
        return 1
    fi
    echo "wt.sh --selftest: PASS ($ca/$ca)"
    return 0
}

# ---------------------------------------------------------------- dispatch
case "${1:-}" in
    acquire)
        shift
        wt_acquire "${1:-}" "${2:-}" "${3:-HEAD}"
        ;;
    release)
        shift
        wt_release "${1:-}"
        ;;
    list)
        wt_list
        ;;
    prune)
        wt_prune
        ;;
    --selftest)
        wt_selftest
        ;;
    "" | -h | --help | help)
        wt_usage
        ;;
    *)
        wt_err "lệnh không biết: $1"
        wt_usage >&2
        exit 2
        ;;
esac
