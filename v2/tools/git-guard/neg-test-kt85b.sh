#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# v2/tools/git-guard/neg-test-kt85b.sh — KT-85b: **bằng chứng ca âm** cho hai hook mới
# (`reference-transaction` + `commit-msg`), thi hành D2-125.2 và vá A1-183.
#
# Vì sao dùng **bản clone tạm** thay vì worktree tạm của repo thật:
#   * `refs/heads/main` đang được checkout ở checkout chính ⇒ git **không** cho worktree thứ hai
#     checkout cùng nhánh (`fatal: 'main' is already checked out`). Muốn thử cherry-pick/merge/
#     reset **vào chính refs/heads/main** thì phải có một repo mà `main` là nhánh đang đứng.
#   * Clone cục bộ (`git clone --local`, hardlink object, không mạng) cho đúng thứ đó: cùng lịch
#     sử, cùng tên nhánh, nhưng **mọi thay đổi chỉ nằm trong /tmp**. Repo thật không thể bị đổi
#     ngay cả khi hook hỏng — đó là điều kiện để phép thử này an toàn tuyệt đối.
#   * Ngoài clone, script còn thử **trên repo thật** một phép không thể làm đổi lịch sử:
#     bật bảo vệ cho một nhánh nháp (`refs/heads/kt85b-probe`) rồi `git update-ref` nhánh đó từ
#     worktree tạm — hook phải từ chối, nhánh nháp không được tạo. `refs/heads/main` của repo
#     thật chỉ được **đọc** (rev-parse) trước/sau.
#
# Các ca (mỗi ca in lệnh, đầu ra, mã thoát, kết luận):
#   CA-A  VIFAST_ROLE=agent2  git cherry-pick <sha>  trên nhánh main  → TỪ CHỐI, main không đổi
#   CA-B  VIFAST_ROLE=agent2  git merge --no-ff      trên nhánh main  → TỪ CHỐI, main không đổi
#   CA-C  VIFAST_ROLE=agent2  git reset --hard HEAD~1 trên nhánh main → TỪ CHỐI, main không đổi
#   CA-D  commit-msg: agent2 + message không nhãn → TỪ CHỐI (pre-commit chặn trước, ghi rõ);
#                    lead + message không nhãn → TỪ CHỐI bởi commit-msg (chứng minh hook nhãn chạy);
#                    lead + "[KT-00] …" → CHO QUA; lead + "Merge …" → CHO QUA (miễn trừ)
#   DƯƠNG lead cherry-pick / lead merge trên main → CHO QUA (hook không phá lead)
#   PROBE  repo thật: update-ref nhánh bảo vệ bằng agent2 → TỪ CHỐI, ref không đổi
#   VERSION git giả 2.27 → install.sh KHÔNG cài reference-transaction, ba hook còn lại vẫn cài
#   BẤT BIẾN main + HEAD + fingerprint index của repo thật không đổi; worktree tạm đã dọn
#
# Dùng:
#   v2/tools/git-guard/neg-test-kt85b.sh                 # tự tạo thư mục tạm, tự dọn
#   v2/tools/git-guard/neg-test-kt85b.sh --dir /tmp/kt85bneg
#   v2/tools/git-guard/neg-test-kt85b.sh --keep          # giữ lại khi có ca hỏng
#   v2/tools/git-guard/neg-test-kt85b.sh --no-cleanup    # giữ nguyên để soi (nhớ dọn tay)
#
# Mã thoát: 0 = mọi ca đúng kỳ vọng; 1 = có ca sai; 2 = lỗi môi trường.

set -u

GUARD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$GUARD_DIR" rev-parse --show-toplevel 2>/dev/null)" || {
    echo "neg-test-kt85b.sh: không tìm thấy gốc repo từ $GUARD_DIR" >&2
    exit 2
}

BASE=''
KEEP=0
NO_CLEANUP=0

while [ $# -gt 0 ]; do
    case "$1" in
        --dir)
            [ $# -ge 2 ] || {
                echo "neg-test-kt85b.sh: --dir cần một đường dẫn" >&2
                exit 2
            }
            BASE="$2"
            shift
            ;;
        --keep) KEEP=1 ;;
        --no-cleanup) NO_CLEANUP=1 ;;
        -h | --help)
            sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "neg-test-kt85b.sh: tham số lạ [$1]" >&2
            exit 2
            ;;
    esac
    shift
done

if [ -z "$BASE" ]; then
    BASE="/tmp/kt85bneg.$$"
fi
if [ -e "$BASE" ]; then
    echo "neg-test-kt85b.sh: [$BASE] đã tồn tại — chọn --dir khác hoặc dọn trước" >&2
    exit 2
fi
mkdir -p "$BASE" || exit 2
CLONE="$BASE/repo"
TINY="$BASE/tinyrepo"
FAKEBIN="$BASE/fakebin"
PROBE_WT="$BASE/wt-probe"

CASE_FAIL=0
CASE_TOTAL=0
CLEANED=0

cleanup_trap() {
    [ "$NO_CLEANUP" -eq 1 ] && return 0
    if [ "$KEEP" -eq 1 ] && [ "$CASE_FAIL" -ne 0 ]; then return 0; fi
    if [ -d "$PROBE_WT" ]; then
        git -C "$REPO" worktree remove --force "$PROBE_WT" >/dev/null 2>&1 || true
        rm -rf "$PROBE_WT" 2>/dev/null || true
    fi
    git -C "$REPO" worktree prune >/dev/null 2>&1 || true
    rm -rf "$BASE" 2>/dev/null || true
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
    if [ "$1" = "PASS" ]; then
        say "KẾT LUẬN: PASS — $2"
    else
        say "KẾT LUẬN: FAIL — $2"
        CASE_FAIL=$((CASE_FAIL + 1))
    fi
}

# Hook phải từ chối: mã thoát != 0 **và** đúng thông điệp của ta (không phải lỗi cờ).
expect_deny() {
    local hook="$1" rc="$2" out="$3" what="$4"
    if [ "$rc" -eq 0 ]; then
        verdict FAIL "lệnh KHÔNG bị từ chối (rc=0) — hook $hook không chặn $what"
        return
    fi
    case "$out" in
        *"vifast-guard($hook): TỪ CHỐI"*)
            verdict PASS "$what bị hook $hook từ chối (rc=$rc), thông điệp đúng"
            ;;
        *)
            verdict FAIL "rc=$rc nhưng KHÔNG thấy thông điệp từ chối của hook $hook — đầu ra ở trên"
            ;;
    esac
}

expect_allow() {
    local rc="$1" out="$2" want="$3" what="$4"
    if [ "$rc" -ne 0 ]; then
        verdict FAIL "$what bị chặn oan (rc=$rc) — đầu ra ở trên"
        return
    fi
    case "$out" in
        *"$want"*) verdict PASS "$what được cho qua, dòng cho qua đúng" ;;
        *) verdict FAIL "rc=0 nhưng KHÔNG thấy dòng cho qua [$want] — đầu ra ở trên" ;;
    esac
}

# ------------------------------------------------------------------ tiền kiểm
say "vifast-guard neg-test-kt85b — KT-85b / D2-125.2 (vá A1-183)"
say "thời điểm      : $(date '+%Y-%m-%d %H:%M:%S %Z')"
say "repo thật      : $REPO"
say "git            : $(git --version)"
say "thư mục tạm    : $BASE (clone cục bộ — không mạng, không đụng repo thật)"
say

say "--- tiền kiểm: bốn hook đã cài đúng ở repo thật chưa"
pre="$("$GUARD_DIR/install.sh" --check 2>&1)"
pre_rc=$?
say "$pre"
if [ "$pre_rc" -ne 0 ]; then
    say "neg-test-kt85b.sh: hook CHƯA cài đúng — chạy v2/tools/git-guard/install.sh trước" >&2
    exit 2
fi
say

MAIN_BEFORE="$(git -C "$REPO" rev-parse refs/heads/main)"
HEAD_BEFORE="$(git -C "$REPO" rev-parse HEAD)"
INDEX_BEFORE="$(git -C "$REPO" ls-files -s | hash_stdin)"
WT_BEFORE="$(git -C "$REPO" worktree list | wc -l | tr -d ' ')"
say "BẤT BIẾN — chụp trước (repo thật)"
say "  refs/heads/main   : $MAIN_BEFORE"
say "  HEAD              : $HEAD_BEFORE"
say "  fingerprint index : $INDEX_BEFORE"
say "  số worktree       : $WT_BEFORE"
say

# ------------------------------------------------------------------ clone tạm + cài hook
say "--- tạo clone cục bộ tại $CLONE (hardlink object, không mạng)"
git clone --quiet --local "$REPO" "$CLONE" 2>&1 | tail -n 3
if [ ! -d "$CLONE/.git" ]; then
    say "neg-test-kt85b.sh: tạo clone tạm THẤT BẠI" >&2
    exit 2
fi
# **KT-242 (A0 duyệt 2026-11-20)** — khi cổng chạy trong **worktree trên nhánh phát triển**,
# `git clone` từ worktree chỉ mang nhánh đang đứng ⇒ clone **không** có `refs/heads/main`, và
# test đỏ vì lý do môi trường chứ không phải hook. Fixture tự tạo `main` trong clone trỏ đúng
# commit `main` của repo thật: vẫn kiểm **đúng luật bảo vệ** (cherry-pick/merge/reset vào chính
# `refs/heads/main`), **không** tắt guard, **không** ép nhánh phát triển phải tên `main`.
if ! git -C "$CLONE" rev-parse --verify --quiet refs/heads/main >/dev/null; then
    git -C "$CLONE" branch --quiet main "$MAIN_BEFORE" || {
        say "neg-test-kt85b.sh: không tạo được refs/heads/main trong clone" >&2
        exit 2
    }
    say "  (clone không có main — đã tạo main = $MAIN_BEFORE, đúng commit của repo thật)"
fi
say "  nhánh đang đứng : $(git -C "$CLONE" rev-parse --abbrev-ref HEAD)"
say "  main của clone  : $(git -C "$CLONE" rev-parse refs/heads/main)"
say
say "--- cài hook vào clone bằng install.sh --repo (cùng bản gốc, cùng sha256)"
inst="$(env -u VIFAST_ROLE "$GUARD_DIR/install.sh" --repo "$CLONE" 2>&1)"
inst_rc=$?
say "$inst"
if [ "$inst_rc" -ne 0 ]; then
    say "neg-test-kt85b.sh: cài hook vào clone THẤT BẠI" >&2
    exit 2
fi
say

M0="$(git -C "$CLONE" rev-parse refs/heads/main)"
SUBJ0="$(git -C "$CLONE" log -1 --format=%s main)"
main_unchanged() {
    local now
    now="$(git -C "$CLONE" rev-parse refs/heads/main)"
    if [ "$now" = "$M0" ]; then
        verdict PASS "refs/heads/main của clone KHÔNG đổi ($M0)"
    else
        verdict FAIL "refs/heads/main ĐỔI ($M0 → $now) — hook đã không chặn"
    fi
}

# Đưa clone về đúng trạng thái $M0 (chỉ dùng khi dọn dẹp; vai lead nên hook cho qua).
restore_clone() {
    env VIFAST_ROLE=lead git -C "$CLONE" reset --hard "$M0" >/dev/null 2>&1 || true
    git -C "$CLONE" clean -fdq >/dev/null 2>&1 || true
    env VIFAST_ROLE=lead git -C "$CLONE" cherry-pick --abort >/dev/null 2>&1 || true
    env VIFAST_ROLE=lead git -C "$CLONE" merge --abort >/dev/null 2>&1 || true
    env VIFAST_ROLE=lead git -C "$CLONE" rebase --abort >/dev/null 2>&1 || true
    git -C "$CLONE" checkout --quiet main >/dev/null 2>&1 || true
    env VIFAST_ROLE=lead git -C "$CLONE" reset --hard "$M0" >/dev/null 2>&1 || true
}
show_clone_state() {
    local dirty
    dirty="$(git -C "$CLONE" status --porcelain | head -n 5)"
    say "  trạng thái clone sau ca: ${dirty:-(sạch — không có thay đổi chờ)}"
}

# ------------------------------------------------------------------ chuẩn bị một commit để cherry-pick
say "--- chuẩn bị: một commit trên nhánh task/kt85b-cp (KHÔNG phải main)"
git -C "$CLONE" checkout --quiet -b task/kt85b-cp "$M0" 2>&1
printf 'KT-85b ca am: tep chi song trong clone tam.\n' >"$CLONE/kt85b-ca-am.txt"
git -C "$CLONE" add -- kt85b-ca-am.txt 2>&1
out="$(env VIFAST_ROLE=lead git -C "$CLONE" commit -m '[KT-85b] ca am: tao commit de cherry-pick' 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
CP_SHA="$(git -C "$CLONE" rev-parse HEAD)"
say "commit để thử: $CP_SHA (nhánh task/kt85b-cp — sẽ bị xoá cùng thư mục tạm)"
if [ "$rc" -ne 0 ] || [ -z "$CP_SHA" ]; then
    say "neg-test-kt85b.sh: không tạo được commit chuẩn bị" >&2
    exit 2
fi
git -C "$CLONE" checkout --quiet main 2>&1
say "  đã quay lại nhánh main của clone, HEAD = $(git -C "$CLONE" rev-parse --short HEAD)"
say

# ------------------------------------------------------------------ CA-A cherry-pick
head_case A "VIFAST_ROLE=agent2 git cherry-pick <sha> trên nhánh main ⇒ phải bị TỪ CHỐI (A1-183)"
say "\$ VIFAST_ROLE=agent2 git -C <clone> cherry-pick $CP_SHA"
out="$(env VIFAST_ROLE=agent2 git -C "$CLONE" cherry-pick "$CP_SHA" 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny reference-transaction "$rc" "$out" "cherry-pick (agent2)"
main_unchanged
say "  subject main : $(git -C "$CLONE" log -1 --format=%s main)"
if [ "$(git -C "$CLONE" log -1 --format=%s main)" = "$SUBJ0" ]; then
    verdict PASS "subject của main vẫn là '$SUBJ0' — commit lạ không vào được"
else
    verdict FAIL "subject của main đã đổi — có commit lạ đã vào main"
fi
# Dọn trạng thái sequencer do cherry-pick bị huỷ (vai lead: chỉ dọn, không đổi nội dung main).
env VIFAST_ROLE=lead git -C "$CLONE" cherry-pick --abort >/dev/null 2>&1 || true
restore_clone
show_clone_state
say "  (đã dọn bằng vai lead — chỉ để ca sau bắt đầu từ cây sạch, không đổi ref)"
say

# ------------------------------------------------------------------ CA-B merge
head_case B "VIFAST_ROLE=agent2 git merge --no-ff task/kt85b-cp trên nhánh main ⇒ phải bị TỪ CHỐI"
say "\$ VIFAST_ROLE=agent2 git -C <clone> merge --no-ff task/kt85b-cp -m '...'"
out="$(env VIFAST_ROLE=agent2 git -C "$CLONE" merge --no-ff task/kt85b-cp -m '[KT-85b] ca am: merge bang vai agent2' 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny reference-transaction "$rc" "$out" "merge (agent2)"
main_unchanged
env VIFAST_ROLE=lead git -C "$CLONE" merge --abort >/dev/null 2>&1 || true
restore_clone
show_clone_state
say "  (đã dọn bằng vai lead — chỉ để ca sau bắt đầu từ cây sạch, không đổi ref)"
say

# ------------------------------------------------------------------ CA-C reset --hard
head_case C "VIFAST_ROLE=agent2 git reset --hard HEAD~1 trên nhánh main ⇒ phải bị TỪ CHỐI"
say "\$ VIFAST_ROLE=agent2 git -C <clone> reset --hard HEAD~1"
out="$(env VIFAST_ROLE=agent2 git -C "$CLONE" reset --hard HEAD~1 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny reference-transaction "$rc" "$out" "reset --hard (agent2)"
main_unchanged
say "  HEAD của clone vẫn là : $(git -C "$CLONE" rev-parse --short HEAD) (mong $(printf '%s' "$M0" | cut -c1-7))"
say
say "  CA C-b — GIỚI HẠN ĐÃ BIẾT (ghi lại trung thực, không phải lỗi của hook):"
say "  'git reset --hard' ghi index + cây làm việc TRƯỚC khi cập nhật ref, nên khi ref bị"
say "  từ chối thì **ref (lịch sử) vẫn nguyên** nhưng cây làm việc/index có thể đã ở bản của"
say "  commit đích. Đó là lý do lịch sử được bảo vệ còn 'cây bẩn' thì không — người/agent sau"
say "  phải tự đưa cây về bằng vai lead."
show_clone_state
restore_clone
say "  sau khi dọn bằng vai lead (VIFAST_ROLE=lead git reset --hard $M0):"
show_clone_state
MAIN_STILL="$(git -C "$CLONE" rev-parse refs/heads/main)"
if [ "$MAIN_STILL" = "$M0" ]; then
    verdict PASS "dù cây có bị đổi tạm thời, refs/heads/main vẫn nguyên $M0 và cây đã về sạch bằng vai lead"
else
    verdict FAIL "sau khi dọn, refs/heads/main đổi ($M0 → $MAIN_STILL)"
fi
say

# ------------------------------------------------------------------ CA-D commit-msg
head_case D "hook commit-msg: nhãn [KT-##]/[ST-##] bắt buộc, trừ message git tự sinh"

# D1: agent2 + message không nhãn ⇒ pre-commit chặn trước (pre-commit chạy TRƯỚC commit-msg).
say "\$ VIFAST_ROLE=agent2 git -C <clone> commit -m 'khong co nhan'"
out="$(env VIFAST_ROLE=agent2 git -C "$CLONE" commit --allow-empty -m 'khong co nhan' 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny pre-commit "$rc" "$out" "commit agent2 thiếu nhãn (pre-commit chặn trước)"
say "  ghi chú: pre-commit chạy TRƯỚC commit-msg trong thứ tự hook của git, nên với vai"
say "           agent2 ta thấy thông điệp của pre-commit; phần kiểm nhãn chứng minh ở D2/D3."
say

# D2: lead + message không nhãn ⇒ pre-commit cho qua, commit-msg phải chặn.
say "\$ VIFAST_ROLE=lead git -C <clone> commit --allow-empty -m 'khong co nhan'"
out="$(env VIFAST_ROLE=lead git -C "$CLONE" commit --allow-empty -m 'khong co nhan' 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_deny commit-msg "$rc" "$out" "commit của lead nhưng thiếu nhãn"
say "  (đây là ca chứng minh commit-msg chạy độc lập với pre-commit)"
say

# D3: lead + nhãn đúng ⇒ cho qua (dùng nhánh task, không đụng main).
# Cây phải sạch để `--allow-empty` tạo commit RỖNG đúng nghĩa (không lẫn thay đổi của ca trước).
restore_clone
D3_DIRTY="$(git -C "$CLONE" status --porcelain | head -n 3)"
say "  kiểm cây trước D3: ${D3_DIRTY:-(sạch)}"
git -C "$CLONE" checkout --quiet -b task/kt85b-msg "$M0" 2>&1
say "\$ VIFAST_ROLE=lead git -C <clone> commit -m '[KT-00] thu'"
out="$(env VIFAST_ROLE=lead git -C "$CLONE" commit --allow-empty -m '[KT-00] thu' 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_allow "$rc" "$out" 'vifast-guard(commit-msg): CHO QUA' "commit của lead có nhãn [KT-00]"
say "  commit tạo ra: $(git -C "$CLONE" log -1 --format='%h %s') (nhánh task/kt85b-msg)"
say

# D4: lead + message "Merge …" tự sinh ⇒ miễn trừ.
git -C "$CLONE" checkout --quiet main 2>&1
say "\$ VIFAST_ROLE=lead git -C <clone> merge --no-ff task/kt85b-msg (message 'Merge branch …')"
out="$(env VIFAST_ROLE=lead git -C "$CLONE" merge --no-ff task/kt85b-msg 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
MBEFORE="$M0"
expect_allow "$rc" "$out" 'vifast-guard(commit-msg): CHO QUA' 'merge của lead (message Merge tự sinh, miễn nhãn)'
MERGE_SHA="$(git -C "$CLONE" rev-parse refs/heads/main)"
say "  main của clone sau merge : ${MERGE_SHA:0:7} — subject: $(git -C "$CLONE" log -1 --format=%s main)"
if [ "$MERGE_SHA" != "$MBEFORE" ]; then
    verdict PASS "merge của lead THẬT SỰ đổi main của clone (${MBEFORE:0:7} → ${MERGE_SHA:0:7}) ⇒ hook không chặn oan vai lead"
else
    verdict FAIL "merge của lead không đổi main — hook chặn oan hoặc merge không chạy"
fi
say

# ------------------------------------------------------------------ DƯƠNG lead cherry-pick
head_case "P" "DƯƠNG: VIFAST_ROLE=lead cherry-pick trên nhánh main ⇒ phải CHO QUA (không chặn oan)"
M_NOW="$(git -C "$CLONE" rev-parse refs/heads/main)"
say "\$ VIFAST_ROLE=lead git -C <clone> cherry-pick $CP_SHA"
out="$(env VIFAST_ROLE=lead git -C "$CLONE" cherry-pick "$CP_SHA" 2>&1)"
rc=$?
say "$out"
say "mã thoát: $rc"
expect_allow "$rc" "$out" 'vifast-guard(reference-transaction): CHO QUA' "cherry-pick của lead"
M_POS="$(git -C "$CLONE" rev-parse refs/heads/main)"
say "  main của clone : ${M_NOW:0:7} → ${M_POS:0:7}"
if [ "$M_POS" != "$M_NOW" ]; then
    verdict PASS "lead cherry-pick vào main THÀNH CÔNG ($(git -C "$CLONE" log -1 --format=%s main))"
else
    verdict FAIL "lead cherry-pick không đổi main"
fi
say

# ------------------------------------------------------------------ PROBE trên repo thật (không thể đổi lịch sử)
head_case "R" "PROBE repo thật: update-ref nhánh bảo vệ bằng vai agent2 ⇒ TỪ CHỐI, không ref nào đổi"
PROBE_REF='refs/heads/kt85b-probe'
say "  (nhánh nháp $PROBE_REF chưa tồn tại; bật bảo vệ nó qua VIFAST_PROTECTED_REFS để thử"
say "   cơ chế 'danh sách cấu hình' ngay trên repo thật — cơ chế này chỉ THÊM ref bảo vệ)"
git -C "$REPO" worktree add --quiet --detach "$PROBE_WT" "$HEAD_BEFORE" 2>&1 | tail -n 2
if [ -d "$PROBE_WT" ]; then
    say "  worktree tạm: $PROBE_WT (detached HEAD, không checkout nhánh nào)"
    say "\$ VIFAST_ROLE=agent2 VIFAST_PROTECTED_REFS=$PROBE_REF git -C <wt> update-ref $PROBE_REF $HEAD_BEFORE"
    out="$(env VIFAST_ROLE=agent2 VIFAST_PROTECTED_REFS="$PROBE_REF" \
        git -C "$PROBE_WT" update-ref "$PROBE_REF" "$HEAD_BEFORE" 2>&1)"
    rc=$?
    say "$out"
    say "mã thoát: $rc"
    expect_deny reference-transaction "$rc" "$out" "update-ref nhánh bảo vệ từ worktree tạm"
    if git -C "$REPO" rev-parse --verify -q "$PROBE_REF" >/dev/null 2>&1; then
        verdict FAIL "nhánh nháp $PROBE_REF ĐÃ được tạo dù hook từ chối — hook không có tác dụng"
    else
        verdict PASS "nhánh nháp $PROBE_REF KHÔNG được tạo ⇒ update-ref thật sự bị huỷ"
    fi
    say "  kiểm thêm: cùng lệnh với vai lead (đáng lẽ qua) — để chứng minh từ chối là do VAI,"
    say "  không phải do lệnh sai; sau đó xoá nhánh nháp ngay."
    out="$(env VIFAST_ROLE=lead VIFAST_PROTECTED_REFS="$PROBE_REF" \
        git -C "$PROBE_WT" update-ref "$PROBE_REF" "$HEAD_BEFORE" 2>&1)"
    rc=$?
    say "  \$ VIFAST_ROLE=lead … update-ref $PROBE_REF … → mã thoát: $rc"
    say "  ${out:-(không có đầu ra)}"
    if [ "$rc" -eq 0 ]; then
        verdict PASS "vai lead update-ref được (hook chỉ chặn vai khác) — minh chứng đối chứng"
        env VIFAST_ROLE=lead git -C "$REPO" update-ref -d "$PROBE_REF" >/dev/null 2>&1 || true
        say "  đã xoá nhánh nháp bằng vai lead: còn không? $(git -C "$REPO" rev-parse --verify -q "$PROBE_REF" || echo 'không còn')"
    else
        verdict FAIL "vai lead cũng bị chặn (rc=$rc) — hook chặn nhầm"
    fi
    git -C "$REPO" worktree remove --force "$PROBE_WT" >/dev/null 2>&1 || true
    rm -rf "$PROBE_WT" 2>/dev/null || true
else
    verdict FAIL "không tạo được worktree tạm cho PROBE"
fi
say

# ------------------------------------------------------------------ VERSION gate
head_case "V" "git < 2.28 ⇒ install.sh KHÔNG cài reference-transaction (hook không tồn tại từ git 2.28)"
REAL_GIT="$(command -v git)"
mkdir -p "$FAKEBIN"
cat >"$FAKEBIN/git" <<EOF
#!/bin/sh
# git giả: chỉ nói dối phiên bản, mọi lệnh khác chuyển cho git thật ($REAL_GIT).
if [ "\${1-}" = "--version" ]; then
    echo "git version 2.27.0"
    exit 0
fi
exec "$REAL_GIT" "\$@"
EOF
chmod 0755 "$FAKEBIN/git"
git init --quiet "$TINY" 2>&1
say "  git giả: $FAKEBIN/git → giả '2.27.0', còn lại gọi $REAL_GIT"
say "  repo nháp: $TINY (git init, không cần commit)"
sup="$(PATH="$FAKEBIN:$PATH" "$GUARD_DIR/install.sh" --print-support 2>&1)"
say "$sup"
inst_old="$(PATH="$FAKEBIN:$PATH" "$GUARD_DIR/install.sh" --repo "$TINY" 2>&1)"
rc=$?
say "$inst_old"
say "mã thoát: $rc"
case "$inst_old" in
    *'[SKIP] reference-transaction'*)
        verdict PASS "install.sh nhận ra git cũ và KHÔNG cài reference-transaction (in [SKIP] + lý do)"
        ;;
    *)
        verdict FAIL "install.sh không in [SKIP] reference-transaction cho git 2.27"
        ;;
esac
if [ "$rc" -ne 0 ]; then
    verdict FAIL "install.sh trả rc=$rc khi git cũ — phải là 0 (ba hook còn lại vẫn cài được)"
fi
if [ -f "$TINY/.git/hooks/reference-transaction" ]; then
    verdict FAIL "vẫn cài reference-transaction trên git giả 2.27 (hook sẽ không bao giờ chạy)"
else
    verdict PASS "không có tệp .git/hooks/reference-transaction trong repo nháp (đúng: git cũ)"
fi
for h in pre-commit pre-push commit-msg; do
    if [ -x "$TINY/.git/hooks/$h" ]; then
        verdict PASS "$h vẫn được cài trên git cũ (đúng: không liên quan phiên bản)"
    else
        verdict FAIL "$h KHÔNG được cài trên git cũ (sai: hook này không phụ thuộc phiên bản)"
    fi
done
chk_old="$(PATH="$FAKEBIN:$PATH" "$GUARD_DIR/install.sh" --repo "$TINY" --check 2>&1)"
rc=$?
say "  --check trên repo nháp (git giả 2.27): rc=$rc"
printf '%s\n' "$chk_old" | tail -n 8
if [ "$rc" -eq 0 ]; then
    verdict PASS "--check trên git cũ trả 0 (thiếu reference-transaction là ĐÚNG, không phải lỗi)"
else
    verdict FAIL "--check trên git cũ trả rc=$rc — coi 'thiếu hook do git cũ' là lỗi là sai"
fi
say

# ------------------------------------------------------------------ dọn + bất biến
rule
say "DỌN — không để lại gì trên repo thật"
if [ "$NO_CLEANUP" -eq 1 ]; then
    say "[  ! ] --no-cleanup: GIỮ $BASE — phải dọn tay:"
    say "        git -C \"$REPO\" worktree remove --force \"$PROBE_WT\" ; rm -rf \"$BASE\""
elif [ "$KEEP" -eq 1 ] && [ "$CASE_FAIL" -ne 0 ]; then
    say "[  ! ] --keep + có ca hỏng: GIỮ $BASE để soi"
else
    rm -rf "$BASE" 2>/dev/null && CLEANED=1
    git -C "$REPO" worktree prune >/dev/null 2>&1 || true
    say "  đã xoá $BASE (clone + repo nháp + git giả + worktree tạm)"
fi
say

MAIN_AFTER="$(git -C "$REPO" rev-parse refs/heads/main)"
HEAD_AFTER="$(git -C "$REPO" rev-parse HEAD)"
INDEX_AFTER="$(git -C "$REPO" ls-files -s | hash_stdin)"
WT_AFTER="$(git -C "$REPO" worktree list | wc -l | tr -d ' ')"
PROBE_LEFT="$(git -C "$REPO" rev-parse --verify -q refs/heads/kt85b-probe || true)"
say "BẤT BIẾN — chụp sau (repo thật)"
say "  refs/heads/main   : $MAIN_AFTER   (trước: $MAIN_BEFORE)"
say "  HEAD              : $HEAD_AFTER   (trước: $HEAD_BEFORE)"
say "  fingerprint index : $INDEX_AFTER   (trước: $INDEX_BEFORE)"
say "  số worktree       : $WT_AFTER   (trước: $WT_BEFORE)"
say

rule
say "BẤT BIẾN — kết luận"
if [ "$MAIN_AFTER" = "$MAIN_BEFORE" ]; then
    verdict PASS "refs/heads/main của repo THẬT không đổi ($MAIN_BEFORE)"
else
    verdict FAIL "refs/heads/main của repo THẬT ĐỔI ($MAIN_BEFORE → $MAIN_AFTER)"
fi
if [ "$HEAD_AFTER" = "$HEAD_BEFORE" ]; then
    verdict PASS "HEAD checkout chính không đổi"
else
    verdict FAIL "HEAD checkout chính ĐỔI ($HEAD_BEFORE → $HEAD_AFTER)"
fi
if [ "$INDEX_AFTER" = "$INDEX_BEFORE" ]; then
    verdict PASS "fingerprint index checkout chính không đổi (không bị nhiễm)"
else
    verdict FAIL "index checkout chính ĐỔI — có gì đó đã stage vào checkout chính"
fi
if [ -z "$PROBE_LEFT" ]; then
    verdict PASS "không còn nhánh nháp nào ($PROBE_REF đã xoá)"
else
    verdict FAIL "nhánh nháp $PROBE_REF còn sót"
fi
if [ "$WT_AFTER" = "$WT_BEFORE" ]; then
    verdict PASS "số worktree trở về như trước ($WT_BEFORE)"
else
    verdict FAIL "số worktree KHÁC trước ($WT_BEFORE → $WT_AFTER) — worktree tạm chưa dọn sạch"
fi
if [ "$NO_CLEANUP" -eq 0 ] && [ "$KEEP" -eq 0 ] && [ "$CLEANED" -eq 0 ]; then
    verdict FAIL "thư mục tạm chưa được xoá"
fi

rule
if [ "$CASE_FAIL" -eq 0 ]; then
    say "TỔNG KẾT: PASS — $CASE_TOTAL ca đều đúng kỳ vọng."
    say "  Bốn ca âm bắt buộc (cherry-pick / merge / reset / commit-msg thiếu nhãn) đều BỊ TỪ CHỐI;"
    say "  hai ca dương (lead cherry-pick, lead merge) đi qua; PROBE trên repo thật bị chặn mà"
    say "  không ref nào đổi; bất biến main/HEAD/index/worktree giữ nguyên."
    exit 0
fi
say "TỔNG KẾT: FAIL — $CASE_FAIL/$CASE_TOTAL ca sai kỳ vọng"
exit 1
