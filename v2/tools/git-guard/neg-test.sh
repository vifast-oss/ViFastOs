#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# v2/tools/git-guard/neg-test.sh — KT-85, sinh **bằng chứng ca âm** cho hook chặn commit.
#
# Chạy đúng ba ca âm đề bài đòi (D2-123 mục (4)(d)) + hai ca dương để chứng minh hook
# **không** phá người dùng, trong một **worktree tạm** (không bao giờ commit lên nhánh chính):
#
#   CA-1  không đặt VIFAST_ROLE            → commit BỊ TỪ CHỐI  (fail-closed)
#   CA-2  VIFAST_ROLE=agent2               → commit BỊ TỪ CHỐI
#   CA-3  VIFAST_ROLE=agent2               → push   BỊ TỪ CHỐI  (remote tạm trong /tmp)
#   DƯƠNG-1 VIFAST_ROLE=lead               → commit THÀNH CÔNG
#   DƯƠNG-2 VIFAST_ROLE=lead               → push   THÀNH CÔNG (chỉ tới bare repo /tmp)
#   KHÔNG-PHÁ git status / git log vẫn chạy bình thường (hook không tham gia)
#   NO-VERIFY `git commit --no-verify` vẫn bỏ qua hook — quyền của người, có chủ ý
#
# An toàn:
#   * Chỉ dùng `git worktree add <tmp> -b <nhánh tạm>`; commit nằm trên nhánh tạm, bị xoá
#     cùng worktree ⇒ không để lại commit nào trên nhánh chính, không sửa lịch sử.
#   * **Tên nhánh tạm là duy nhất cho từng lần chạy** (`task/kt85neg-<pid>-<epoch>`), không
#     phải hằng số `task/kt85neg` (R5 của Agent 0, lượt 4). Vì sao: hai lần chạy song song —
#     kể cả hai lần do hai agent khác nhau bấm — trước đây tranh nhau **cùng một** tên nhánh;
#     lần thứ hai `git worktree add -b` hỏng, và tệ hơn: trap của lần thứ nhất `git branch -D`
#     xoá nhánh của lần thứ hai. Ca `--selftest-parallel` chạy đúng hai lần song song và đòi
#     **cả hai** PASS.
#   * `git add` **chỉ** chạy trong index của worktree tạm (mỗi worktree có index riêng —
#     đây chính là cơ chế vá A1-177). Script kiểm fingerprint index của checkout chính
#     trước/sau và in kết quả.
#   * Push chỉ tới `git init --bare` trong thư mục tạm (không mạng, không remote thật).
#   * Kết thúc: `git worktree remove --force`, `git branch -D`, xoá bare repo, `worktree prune`.
#
# Dùng:
#   v2/tools/git-guard/neg-test.sh                  # tự tạo thư mục tạm, tự dọn
#   v2/tools/git-guard/neg-test.sh --dir /tmp/kt85neg   # đúng tên thư mục đề bài
#   v2/tools/git-guard/neg-test.sh --keep           # giữ worktree khi có ca hỏng
#   v2/tools/git-guard/neg-test.sh --no-cleanup     # giữ lại để soi (nhớ dọn tay)
#   v2/tools/git-guard/neg-test.sh --selftest-parallel  # hai lần chạy song song, cả hai phải PASS
#
# Mã thoát: 0 = cả năm ca đúng như kỳ vọng; 1 = có ca sai; 2 = lỗi môi trường.

set -u

GUARD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$GUARD_DIR" rev-parse --show-toplevel 2>/dev/null)" || {
    echo "neg-test.sh: không tìm thấy gốc repo từ $GUARD_DIR" >&2
    exit 2
}

WT=''
# Rỗng = **tự sinh tên duy nhất** sau khi phân tích tham số (xem `--selftest-parallel`).
BRANCH=''
KEEP=0
NO_CLEANUP=0
SELFTEST_PARALLEL=0
MESSAGE_PREFIX='[KT-85]'

while [ $# -gt 0 ]; do
    case "$1" in
        --dir)
            [ $# -ge 2 ] || { echo "neg-test.sh: --dir cần một đường dẫn" >&2; exit 2; }
            WT="$2"
            shift
            ;;
        --branch)
            [ $# -ge 2 ] || { echo "neg-test.sh: --branch cần một tên" >&2; exit 2; }
            BRANCH="$2"
            shift
            ;;
        --selftest-parallel) SELFTEST_PARALLEL=1 ;;
        --keep) KEEP=1 ;;
        --no-cleanup) NO_CLEANUP=1 ;;
        -h | --help) sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "neg-test.sh: tham số lạ [$1]" >&2; exit 2 ;;
    esac
    shift
done

# Tên nhánh tạm **duy nhất cho từng lần chạy** (R5). `$$` là pid của shell này, `date +%s` là
# epoch giây; hai lần chạy song song luôn khác pid nên không bao giờ trùng tên. `--branch` vẫn
# ghi đè được (dùng cho ca tự kiểm của chính script).
if [ -z "$BRANCH" ]; then
    BRANCH="task/kt85neg-$$-$(date +%s)"
fi

# ---------------------------------------------------------------- ca hai lần chạy song song (R5)
#
# Vì sao có ca này: tên nhánh tạm từng là hằng số `task/kt85neg`. Hai lần chạy song song (hai
# agent, hoặc `gate --fast` của hai worktree) tranh nhau **cùng một** tên: lần thứ hai
# `git worktree add -b` hỏng vì nhánh đã tồn tại, và trap của lần thứ nhất `git branch -D` xoá
# nhánh mà lần thứ hai đang dùng. Ca này chạy **đúng hai lần song song** và đòi cả hai PASS —
# nếu tên nhánh quay lại thành hằng số, ca này đỏ.
if [ "$SELFTEST_PARALLEL" -eq 1 ]; then
    _p1_log="$(mktemp "${TMPDIR:-/tmp}/kt85neg-par1.XXXXXX")" || exit 2
    _p2_log="$(mktemp "${TMPDIR:-/tmp}/kt85neg-par2.XXXXXX")" || exit 2
    bash "${BASH_SOURCE[0]}" >"$_p1_log" 2>&1 &
    _p1=$!
    bash "${BASH_SOURCE[0]}" >"$_p2_log" 2>&1 &
    _p2=$!
    wait "$_p1"; _rc1=$?
    wait "$_p2"; _rc2=$?
    _b1="$(sed -n 's/^nhánh tạm *: *//p' "$_p1_log" | head -n 1)"
    _b2="$(sed -n 's/^nhánh tạm *: *//p' "$_p2_log" | head -n 1)"
    _bad=0
    if [ "$_rc1" -ne 0 ]; then
        echo "not ok - selftest-parallel: lần 1 exit $_rc1 (mong 0)" >&2
        tail -n 20 "$_p1_log" >&2
        _bad=1
    fi
    if [ "$_rc2" -ne 0 ]; then
        echo "not ok - selftest-parallel: lần 2 exit $_rc2 (mong 0)" >&2
        tail -n 20 "$_p2_log" >&2
        _bad=1
    fi
    if [ -z "$_b1" ] || [ -z "$_b2" ]; then
        echo "not ok - selftest-parallel: không đọc được tên nhánh tạm từ log con" >&2
        _bad=1
    elif [ "$_b1" = "$_b2" ]; then
        echo "not ok - selftest-parallel: hai lần chạy dùng CÙNG tên nhánh [$_b1]" >&2
        _bad=1
    fi
    if [ "$_bad" -eq 0 ]; then
        echo "ok - selftest-parallel: hai lần chạy song song đều PASS, nhánh tạm khác nhau"
        echo "     lần 1: $_b1"
        echo "     lần 2: $_b2"
    fi
    rm -f "$_p1_log" "$_p2_log"
    exit "$_bad"
fi

if [ -z "$WT" ]; then
    WT="$(mktemp -d "${TMPDIR:-/tmp}/kt85neg.XXXXXX")" || exit 2
    rmdir "$WT" 2>/dev/null || true
fi
# `git worktree add` đòi đích không tồn tại (hoặc rỗng).
if [ -e "$WT" ] && [ -n "$(ls -A "$WT" 2>/dev/null)" ]; then
    echo "neg-test.sh: [$WT] đã tồn tại và không rỗng — chọn --dir khác hoặc dọn trước" >&2
    exit 2
fi
rmdir "$WT" 2>/dev/null || true

BARE="$(mktemp -d /tmp/kt85remote.XXXXXX)" || exit 2

CASE_FAIL=0
CASE_TOTAL=0
CLEANED=0

# Dọn lần hai (im lặng) khi thoát bất thường — nhánh tạm/worktree tạm không được sống sót.
cleanup_trap() {
    [ "$NO_CLEANUP" -eq 1 ] && return 0
    if [ "$KEEP" -eq 1 ] && [ "$CASE_FAIL" -ne 0 ]; then return 0; fi
    if [ -n "$WT" ]; then
        git -C "$REPO" worktree remove --force "$WT" >/dev/null 2>&1 || true
        rm -rf "$WT" 2>/dev/null || true
    fi
    git -C "$REPO" branch -D "$BRANCH" >/dev/null 2>&1 || true
    git -C "$REPO" worktree prune >/dev/null 2>&1 || true
    if [ -n "$BARE" ]; then rm -rf "$BARE" 2>/dev/null || true; fi
}
trap cleanup_trap EXIT INT TERM

hash_stdin() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 | awk '{print $1}'
    else
        sha256sum | awk '{print $1}'
    fi
}

say() { printf '%s\n' "$*"; }
rule() { say '----------------------------------------------------------------------'; }
head_case() {
    CASE_TOTAL=$((CASE_TOTAL + 1))
    rule
    say "CA $1 — $2"
    rule
}
verdict() {
    # $1 = PASS/FAIL, $2 = mô tả chính
    if [ "$1" = "PASS" ]; then
        say "KẾT LUẬN: PASS — $2"
    else
        say "KẾT LUẬN: FAIL — $2"
        CASE_FAIL=$((CASE_FAIL + 1))
    fi
}

# Kỳ vọng hook TỪ CHỐI: mã thoát khác 0 **và** thông điệp đúng của ta (không phải lỗi cờ).
expect_deny() {
    local hook="$1" rc="$2" out="$3" want
    want="vifast-guard($hook): TỪ CHỐI"
    case "$out" in
        *"$want"*)
            if [ "$rc" -ne 0 ]; then
                verdict PASS "hook $hook từ chối, mã thoát $rc, thông điệp đúng"
            else
                verdict FAIL "có thông điệp từ chối nhưng mã thoát 0"
            fi
            ;;
        *)
            verdict FAIL "không thấy thông điệp từ chối của hook $hook (mã thoát $rc) — đầu ra ở trên"
            ;;
    esac
}

expect_allow() {
    local rc="$1" out="$2" want="$3"
    case "$out" in
        *"$want"*) : ;;
        *)
            verdict FAIL "không thấy dòng cho qua [$want] — đầu ra ở trên"
            return
            ;;
    esac
    if [ "$rc" -eq 0 ]; then
        verdict PASS "hook cho qua, lệnh thành công"
    else
        verdict FAIL "hook cho qua nhưng lệnh vẫn lỗi (mã thoát $rc)"
    fi
}

# ------------------------------------------------------------------ tiền kiểm
say "vifast-guard neg-test — KT-85 / D2-123 mục (4)"
say "thời điểm      : $(date '+%Y-%m-%d %H:%M:%S %Z')"
say "repo           : $REPO"
say "worktree tạm   : $WT"
say "nhánh tạm      : $BRANCH"
say "remote tạm     : $BARE (bare, chỉ trong /tmp)"
say "git            : $(git --version)"
say "shell          : $BASH_VERSION"
say
say "--- tiền kiểm: hook đã cài đúng chưa"
pre="$("$GUARD_DIR/install.sh" --check 2>&1)"
pre_rc=$?
say "$pre"
if [ "$pre_rc" -ne 0 ]; then
    say "neg-test.sh: hook CHƯA cài đúng — chạy v2/tools/git-guard/install.sh trước" >&2
    exit 2
fi
say

HEAD_BEFORE="$(git -C "$REPO" rev-parse HEAD)"
INDEX_BEFORE="$(git -C "$REPO" ls-files -s | hash_stdin)"
WT_COUNT_BEFORE="$(git -C "$REPO" worktree list | wc -l | tr -d ' ')"
say "HEAD checkout chính trước : $HEAD_BEFORE"
say "fingerprint index chính   : $INDEX_BEFORE"
say "số worktree trước         : $WT_COUNT_BEFORE"
say

# ------------------------------------------------------------------ worktree tạm
say "--- tạo worktree tạm (nhánh $BRANCH cắt từ HEAD $HEAD_BEFORE)"
git -C "$REPO" worktree add "$WT" -b "$BRANCH" "$HEAD_BEFORE" 2>&1 | tail -n 2
if [ ! -d "$WT" ]; then
    say "neg-test.sh: tạo worktree tạm THẤT BẠI" >&2
    exit 2
fi
say "toplevel tạm              : $(git -C "$WT" rev-parse --show-toplevel)"
say "git-common-dir (từ tạm)   : $(git -C "$WT" rev-parse --path-format=absolute --git-common-dir)"
say "đường hook git sẽ dùng    : $(git -C "$WT" rev-parse --git-path hooks/pre-commit)"
say "                           (worktree liên kết dùng hook ở common dir ⇒ guard áp cho cả tạm)"
say

PROBE='kt85-ca-am-probe.txt'
printf 'KT-85 ca am: tep nay chi song trong worktree tam, se bi xoa.\n' >"$WT/$PROBE"
git -C "$WT" add -- "$PROBE" 2>&1
say "đã sửa 1 tệp trong tạm    : $PROBE (git add chỉ trong index của tạm)"
say

# ------------------------------------------------------------------ CA-1
head_case 1 "không đặt VIFAST_ROLE ⇒ commit phải bị từ chối (fail-closed)"
say '$ env -u VIFAST_ROLE git -C <tạm> commit -m "<KT-85> ca am: khong dat bien"'
out="$(env -u VIFAST_ROLE git -C "$WT" commit -m "$MESSAGE_PREFIX ca am: khong dat VIFAST_ROLE" 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny pre-commit "$rc" "$out"
say

# ------------------------------------------------------------------ CA-2
head_case 2 "VIFAST_ROLE=agent2 ⇒ commit phải bị từ chối"
say '$ VIFAST_ROLE=agent2 git -C <tạm> commit -m "<KT-85> ca am: vai agent2"'
out="$(env VIFAST_ROLE=agent2 git -C "$WT" commit -m "$MESSAGE_PREFIX ca am: vai agent2" 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny pre-commit "$rc" "$out"
say

# ------------------------------------------------------------------ DƯƠNG-1
head_case "D1" "VIFAST_ROLE=lead ⇒ commit THÀNH CÔNG (hook không phá người dùng)"
say '$ VIFAST_ROLE=lead git -C <tạm> commit -m "<KT-85> ca duong: vai lead"'
out="$(env VIFAST_ROLE=lead git -C "$WT" commit -m "$MESSAGE_PREFIX ca duong: vai lead" 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_allow "$rc" "$out" 'vifast-guard(pre-commit): CHO QUA'
POS_SHA="$(git -C "$WT" rev-parse HEAD)"
say "commit tạm: $POS_SHA (chỉ trên nhánh $BRANCH, sẽ xoá cùng worktree)"
say

# ------------------------------------------------------------------ CA-3
git init --bare -q "$BARE" 2>&1
head_case 3 "VIFAST_ROLE=agent2 ⇒ push phải bị từ chối (remote tạm $BARE)"
say '$ VIFAST_ROLE=agent2 git -C <tạm> push <bare tạm> HEAD:refs/heads/kt85probe'
out="$(env VIFAST_ROLE=agent2 git -C "$WT" push "$BARE" HEAD:refs/heads/kt85probe 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny pre-push "$rc" "$out"
say "kiểm thêm: bare tạm KHÔNG có ref kt85probe (push thật sự không xảy ra)"
if git -C "$BARE" rev-parse --verify -q refs/heads/kt85probe >/dev/null 2>&1; then
    verdict FAIL "bare tạm CÓ ref kt85probe — push đã chạy dù bị hook từ chối"
else
    verdict PASS "bare tạm không có ref nào từ lần push bị từ chối"
fi
say

# ------------------------------------------------------------------ DƯƠNG-2
head_case "D2" "VIFAST_ROLE=lead ⇒ push THÀNH CÔNG (chỉ tới bare repo trong /tmp)"
say '$ VIFAST_ROLE=lead git -C <tạm> push <bare tạm> HEAD:refs/heads/kt85probe'
out="$(env VIFAST_ROLE=lead git -C "$WT" push "$BARE" HEAD:refs/heads/kt85probe 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_allow "$rc" "$out" 'vifast-guard(pre-push): CHO QUA'
ref_sha="$(git -C "$BARE" rev-parse --verify -q refs/heads/kt85probe 2>/dev/null || true)"
say "ref ở bare tạm: kt85probe = ${ref_sha:-(không có)}"
if [ "$ref_sha" = "$POS_SHA" ]; then
    verdict PASS "bare tạm nhận đúng commit $POS_SHA ⇒ push của lead chạy thật"
else
    verdict FAIL "bare tạm không nhận đúng commit (mong $POS_SHA, có ${ref_sha:-(không có)})"
fi
say


# ------------------------------------------------------------------ KHÔNG-PHÁ
head_case "P" "hook không phá git status và git log (vẫn chạy y như trước)"
st_out="$(git -C "$WT" status --porcelain 2>&1)"
st_rc=$?
lg_out="$(git -C "$WT" log -1 --oneline 2>&1)"
lg_rc=$?
main_st_rc=0
git -C "$REPO" status --porcelain >/dev/null 2>&1 || main_st_rc=$?
main_lg_rc=0
git -C "$REPO" log -1 --oneline >/dev/null 2>&1 || main_lg_rc=$?
say "git -C <tạm> status --porcelain   → rc=$st_rc ($(printf '%s' "$st_out" | wc -l | tr -d ' ') dòng)"
say "git -C <tạm> log -1 --oneline     → rc=$lg_rc: ${lg_out:-(rỗng)}"
say "git -C <chính> status --porcelain → rc=$main_st_rc"
say "git -C <chính> log -1 --oneline   → rc=$main_lg_rc"
if [ "$st_rc" -eq 0 ] && [ "$lg_rc" -eq 0 ] && [ -n "$lg_out" ] &&
    [ "$main_st_rc" -eq 0 ] && [ "$main_lg_rc" -eq 0 ]; then
    verdict PASS "git status/git log chạy bình thường ở cả checkout chính lẫn worktree"
else
    verdict FAIL "git status/git log bị ảnh hưởng (rc: status=$st_rc log=$lg_rc main=$main_st_rc/$main_lg_rc)"
fi
say

# ------------------------------------------------------------------ NO-VERIFY
head_case "N" "quyền của NGƯỜI: commit --no-verify vẫn bỏ qua hook (chủ ý)"
printf 'dong thu hai cho ca --no-verify\n' >>"$WT/$PROBE"
git -C "$WT" add -- "$PROBE" 2>&1
say "đã sửa thêm 1 dòng trong tạm: $PROBE (để có thay đổi chờ commit)"
say '$ VIFAST_ROLE=agent2 git -C <tạm> commit --no-verify -m "<KT-85> ca no-verify"'
out="$(env VIFAST_ROLE=agent2 git -C "$WT" commit --no-verify -m "$MESSAGE_PREFIX ca: nguoi dung --no-verify" 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
if [ "$rc" -eq 0 ]; then
    verdict PASS "--no-verify bỏ qua hook đúng như tài liệu — hook không chặn quyền của người"
else
    verdict FAIL "--no-verify bị chặn (rc=$rc) — trái yêu cầu 4"
fi
say "Lưu ý: commit này chỉ nằm trên nhánh tạm $BRANCH, bị xoá cùng worktree ở bước dọn."
say

# ------------------------------------------------------------------ dọn + bất biến
rule
say "DỌN — không để lại commit/di trú trên nhánh chính"
if [ "$NO_CLEANUP" -eq 1 ]; then
    say "[  ! ] --no-cleanup: GIỮ $WT (nhánh $BRANCH) — phải dọn tay:"
    say "        git -C \"$REPO\" worktree remove --force \"$WT\" && git -C \"$REPO\" branch -D $BRANCH && rm -rf \"$BARE\""
elif [ "$KEEP" -eq 1 ] && [ "$CASE_FAIL" -ne 0 ]; then
    say "[  ! ] --keep + có ca hỏng: GIỮ $WT để soi (nhánh $BRANCH)"
else
    git -C "$REPO" worktree remove --force "$WT" 2>&1 && CLEANED=1
    git -C "$REPO" branch -D "$BRANCH" 2>&1
    rm -rf "$BARE"
    git -C "$REPO" worktree prune 2>&1
    CLEANED=1
fi
say

HEAD_AFTER="$(git -C "$REPO" rev-parse HEAD)"
INDEX_AFTER="$(git -C "$REPO" ls-files -s | hash_stdin)"
WT_COUNT_AFTER="$(git -C "$REPO" worktree list | wc -l | tr -d ' ')"
BRANCH_LEFT="$(git -C "$REPO" branch --list "$BRANCH" | tr -d ' ')"

say "HEAD checkout chính sau   : $HEAD_AFTER"
say "fingerprint index chính   : $INDEX_AFTER"
say "số worktree sau           : $WT_COUNT_AFTER"
say "nhánh tạm còn lại         : ${BRANCH_LEFT:-(không có)}"
say

rule
say "BẤT BIẾN"
if [ "$HEAD_AFTER" = "$HEAD_BEFORE" ]; then
    verdict PASS "HEAD checkout chính không đổi ($HEAD_BEFORE)"
else
    verdict FAIL "HEAD checkout chính ĐỔI ($HEAD_BEFORE → $HEAD_AFTER)"
fi
if [ "$INDEX_AFTER" = "$INDEX_BEFORE" ]; then
    verdict PASS "index checkout chính không bị nhiễm (fingerprint giữ nguyên — vá A1-177)"
else
    verdict FAIL "index checkout chính ĐỔI — có gì đó đã stage vào checkout chính"
fi
if [ -z "$BRANCH_LEFT" ]; then
    verdict PASS "nhánh tạm $BRANCH đã xoá — không còn commit thử nào"
else
    verdict FAIL "nhánh tạm $BRANCH còn tồn tại"
fi
if [ "$WT_COUNT_AFTER" = "$WT_COUNT_BEFORE" ]; then
    verdict PASS "số worktree trở về như trước ($WT_COUNT_BEFORE)"
else
    verdict FAIL "số worktree KHÁC trước ($WT_COUNT_BEFORE → $WT_COUNT_AFTER)"
fi

rule
if [ "$CASE_FAIL" -eq 0 ]; then
    say "TỔNG KẾT: PASS — $CASE_TOTAL ca đều đúng kỳ vọng (ba ca âm bị từ chối, hai ca lead" \
        "cho qua, hai ca phụ: không phá git status/log + tôn trọng --no-verify của người)"
    exit 0
fi
say "TỔNG KẾT: FAIL — $CASE_FAIL/$CASE_TOTAL ca sai kỳ vọng"
exit 1
