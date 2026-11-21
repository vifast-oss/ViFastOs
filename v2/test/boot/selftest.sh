#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-54 — tự kiểm **runner** `ktest.sh` bằng mutation của **harness** (không đụng kernel).
#
# Vì sao cần: một runner TAP chỉ đáng tin khi nó **đỏ** đúng lúc, **đúng lý do**, và **không xanh
# khi không chạy gì**. Tệp này dựng 21 ca quanh một log thật:
#   • 1 ca XANH (log thật qua, không báo động giả);
#   • 19 ca ĐỎ — mỗi ca phá **đúng một** bất biến, và phải đỏ **kèm đúng token lý do** (không chấp
#     nhận "exit ≠ 0 vô tội vạ"):
#       (a) `expected.txt`: bớt 1 TAP (N→N-1, N đọc từ tệp) / mất dòng ảnh `ktest-user`;
#       (b) log: một dòng `ok` → `not ok` / xoá 1 dòng `ok` / nhân đôi 1 dòng `ok`;
#       (c) log: xoá 1 dòng `# evidence` / xoá dòng kế hoạch `1..N`;
#       (d) log: thêm PANIC thứ hai / PANIC đúng mẫu **thêm đuôi lạ** / PANIC **lạ**;
#       (e) **0 boot** (`ktest.sh 1 " "`) phải `die`, không được "0/0 boot sạch + PASS";
#       (f) `Platform HART Count` sai (1→10/2→12) phải đỏ (mẫu phải neo **cả dòng**);
#       (g) evidence bị thêm đuôi lạ (`# mmu on: … EXTRA`) phải đỏ (mẫu neo cả dòng);
#       (h) lặp dòng per-hart của hart 0 thay hart 1 (`# timer hart=`/`# hart … alive`) phải đỏ
#           (kiểm **tập hart**, không chỉ đếm dòng);
#       (k) **KT-280**: xoá dòng `# ipc: umode call ok reply=…` của ảnh `ktest-ipc` (do chính
#           chương trình U-mode in) phải đỏ — dòng `# ipc:` là evidence **bắt buộc**;
#       (l) **KT-282**: xoá dòng `# ipc: mr count=8 mr7=…` của ảnh `ktest-ipc` (MR4..MR7 qua
#           **IPC buffer**) phải đỏ — dòng `# ipc: mr` là evidence **bắt buộc**;
#       (m) **KT-290**: xoá dòng `# ipc: configure ok va=…` của ảnh `ktest-ipc` (`Tcb.Configure`
#           qua **invocation** từ U-mode) phải đỏ — dòng `# ipc: configure` là evidence **bắt buộc**;
#       (o) **KT-309**: xoá dòng `# ipc caps: sent=1 received=1 slot=5 rights=0x…` của ảnh
#           `ktest-ipc` (**cap transfer qua IPC buffer** — kernel in sau khi **copy** cap `Frame`
#           sang CSpace server) phải đỏ — dòng `# ipc caps:` là evidence **bắt buộc**;
#       (n) **KT-306**: xoá dòng `# netping: label=0x291 reply=0 rtt_ms=…` của ảnh `ktest-netsrv`
#           (do chính chương trình U-mode in sau syscall `Call` trên `EP_KERN`) phải đỏ — dòng
#           `# netping:` là evidence **bắt buộc**;
#       (r) **KT-319**: xoá dòng `# ipc: suspend state=Inactive suspended=0 calls=2 …` của ảnh
#           `ktest-ipc` (`Tcb.Suspend` — kernel in sau khi đo **độc lập** `Tcb.ReadState` của
#           đích bị treo) phải đỏ — dòng `# ipc: suspend` là evidence **bắt buộc**;
#   • 1 ca kiểm `expected.txt` **nguyên vẹn** sau mutation (sha256 trước = sau).
#
# Mọi mutation chạy trên **bản sao** log trong thư mục tạm; riêng ca (a) phá **chính**
# `expected.txt` rồi trả lại ngay (`trap` + sha256 + `cmp` chứng minh). Không sửa kernel/ảnh.
#
# Dùng: bash v2/test/boot/selftest.sh [log-ảnh-chính] [smp]   (mặc định: tự boot 1 vòng smp=1)
#   vd: bash v2/test/boot/selftest.sh /tmp/k3-ktest/ktest-smp2-1.log 2
# Exit 0 = mọi ca đúng như kỳ vọng; 1 = có ca sai (in `not ok`).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KTEST="$HERE/ktest.sh"
EXPECTED="$HERE/expected.txt"

SMP="${2:-1}"
WORK=""
RESTORE_NEEDED=0
fail=0
total=0

sha_of() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        sha256sum "$1" | cut -d' ' -f1
    fi
}

cleanup() {
    # Ca (a) phá chính `expected.txt`: trả lại **luôn**, kể cả khi bị Ctrl-C / lỗi giữa chừng.
    if [ "${RESTORE_NEEDED:-0}" -eq 1 ] && [ -f "$WORK/expected.orig" ]; then
        cp "$WORK/expected.orig" "$EXPECTED"
    fi
    [ -n "${WORK:-}" ] && [ -d "$WORK" ] && rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

die() { echo "selftest.sh: $*" >&2; exit 1; }

[ -f "$KTEST" ] || die "không thấy $KTEST"
[ -f "$EXPECTED" ] || die "không thấy $EXPECTED"

# Ca (a) phá **chính** `expected.txt` trong ~1 s ⇒ cấm chạy song song với một vòng boot đang đọc
# tệp đó (một boot đọc nhầm "ktest 36 36" sẽ đỏ oan, tệ hơn: `check_manifests` làm cả vòng dừng).
if command -v pgrep >/dev/null 2>&1 &&
    pgrep -f 'test/boot/ktest\.sh' >/dev/null 2>&1; then
    die "đang có 'ktest.sh' chạy — chạy selftest một mình (mutation expected.txt ~1 s)"
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/kt54-selftest.XXXXXX")" || die "không tạo được thư mục tạm"
cp "$EXPECTED" "$WORK/expected.orig" || die "không sao được expected.txt"
SHA_BEFORE="$(sha_of "$EXPECTED")"
# KT-55: số TAP đọc **từ** expected.txt (không hardcode) — thêm/bớt TAP không làm selftest cũ.
PLAN="$(awk '$1 == "ktest" { print $2; exit }' "$WORK/expected.orig")"
[ -n "$PLAN" ] || die "expected.txt không có dòng 'ktest N M'"
PLAN_M1=$((PLAN - 1))

LOG="${1:-}"
if [ -z "$LOG" ]; then
    echo "# chưa có log — boot 1 vòng (ktest.sh 1 $SMP) để lấy log thật"
    if ! bash "$KTEST" 1 "$SMP"; then
        echo "not ok 1 - không lấy được log thật (ktest.sh 1 $SMP đỏ)"
        exit 1
    fi
    LOG="$(bash "$KTEST" --log-path ktest "$SMP" 1)" || die "không lấy được đường dẫn log"
fi
[ -f "$LOG" ] || die "không thấy log $LOG"
echo "# log kiểm: $LOG (smp=$SMP); expected.txt sha256=$(printf '%s' "$SHA_BEFORE" | cut -c1-16)…"

# --- helpers -------------------------------------------------------------

out_of() { # <biến nhận> <lệnh...>  — chạy, lưu stdout+stderr, trả mã thoát
    local __var="$1"
    shift
    local __outs __rc
    __outs="$("$@" 2>&1)"
    __rc=$?
    eval "$__var=\$__outs"
    return $__rc
}

# case_green <mô tả> <lệnh...>: exit 0 và **không** có dòng `not ok`.
case_green() {
    local desc="$1" out="" rc=0
    shift
    total=$((total + 1))
    out_of out "$@"
    rc=$?
    if [ "$rc" -eq 0 ] && ! printf '%s\n' "$out" | grep -q '^not ok'; then
        echo "ok $total - $desc"
    else
        echo "not ok $total - $desc (exit $rc, muốn 0 + không 'not ok')"
        printf '%s\n' "$out" | sed 's/^/#   /' | head -20
        fail=$((fail + 1))
    fi
}

# case_red <token-lý-do> <mô tả> <lệnh...>: exit ≠ 0, có `not ok`, **và** dòng `not ok` phải chứa
# đúng token lý do (chứng minh đỏ vì đúng cái vừa phá, không phải đỏ ngẫu nhiên).
case_red() {
    local token="$1" desc="$2" out="" rc=0 first=""
    shift 2
    total=$((total + 1))
    out_of out "$@"
    rc=$?
    first="$(printf '%s\n' "$out" | grep '^not ok' | head -1)"
    if [ "$rc" -ne 0 ] && [ -n "$first" ] && printf '%s' "$first" | grep -qF -- "$token"; then
        echo "ok $total - $desc"
        echo "#   runner đỏ vì [$token]: $(printf '%s' "$first" | cut -c1-150)"
    else
        echo "not ok $total - $desc (exit $rc; cần 'not ok' chứa [$token])"
        printf '%s\n' "$out" | sed 's/^/#   /' | head -15
        fail=$((fail + 1))
    fi
}

# case_die <token> <mô tả> <lệnh...>: lỗi **cấu hình** — exit ≠ 0 + thông điệp `ktest.sh:` chứa token
# (đường `die`, không có dòng `not ok`).
case_die() {
    local token="$1" desc="$2" out="" rc=0
    shift 2
    total=$((total + 1))
    out_of out "$@"
    rc=$?
    if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qF -- "$token"; then
        echo "ok $total - $desc"
        echo "#   runner die vì [$token]: $(printf '%s\n' "$out" | head -1 | cut -c1-150)"
    else
        echo "not ok $total - $desc (exit $rc; cần die với [$token])"
        printf '%s\n' "$out" | sed 's/^/#   /' | head -10
        fail=$((fail + 1))
    fi
}

# `mut_diff <tệp> <mô tả>`: mutation phải **thật sự** làm tệp khác bản gốc — nếu không, ca test
# "đỏ" sẽ vô nghĩa (log không đổi mà runner vẫn xanh thì ca đó không chứng minh gì).
mut_diff() {
    cmp -s "$1" "$2" && die "mutation KHÔNG áp dụng: $3 — dừng để sửa test (không chạy tiếp)"
    return 0
}

# `ev_idx <ảnh> <smp> <chuỗi con của mẫu>`: **ordinal** của mục evidence (tra từ chính runner, không
# hardcode — đổi danh sách thì test tự theo).
ev_idx() {
    bash "$KTEST" --print-evidence "$1" "$2" | awk -F'\t' -v pat="$3" 'index($2, pat) { print $1; exit }'
}

# --- ca 1: XANH — log thật phải qua -------------------------------------
case_green "log thật $LOG qua runner (không báo động giả)" \
    bash "$KTEST" --check-log ktest "$LOG" "$SMP"

# --- ca (a): expected.txt bớt 1 TAP của ảnh chính (N → N−1) --------------
# Phá **chính** tệp nguồn sự thật rồi trả lại ngay: chứng minh runner đọc số từ tệp.
sed "s/^ktest $PLAN $PLAN$/ktest $PLAN_M1 $PLAN_M1/" "$WORK/expected.orig" >"$EXPECTED" ||
    die "không sửa được expected.txt"
RESTORE_NEEDED=1
case_red "kế hoạch $PLAN_M1 nhưng runner kiểm $PLAN TAP" \
    "expected.txt bớt 1 TAP (ktest $PLAN $PLAN → $PLAN_M1 $PLAN_M1)" \
    bash "$KTEST" --check-log ktest "$LOG" "$SMP"
cp "$WORK/expected.orig" "$EXPECTED" || die "KHÔNG trả lại được expected.txt"
RESTORE_NEEDED=0

# --- ca (a'): expected.txt mất hẳn một ảnh ------------------------------
grep -v '^ktest-user ' "$WORK/expected.orig" >"$WORK/expected-no-user.txt"
mut_diff "$WORK/expected-no-user.txt" "$WORK/expected.orig" "expected.txt mất dòng ktest-user"
case_red "thiếu dòng cho ảnh [ktest-user]" \
    "expected.txt mất dòng ảnh 'ktest-user'" \
    env KT54_EXPECTED="$WORK/expected-no-user.txt" bash "$KTEST" --check-log ktest "$LOG" "$SMP"

# --- ca (b): một dòng `ok` thành `not ok` -------------------------------
awk '/^ok / && !d { sub(/^ok /, "not ok "); d = 1 } { print }' "$LOG" >"$WORK/log-notok.log"
mut_diff "$WORK/log-notok.log" "$LOG" "log: dòng ok → not ok"
case_red "not-ok=1" \
    "log: dòng 'ok' đầu đổi thành 'not ok'" \
    bash "$KTEST" --check-log ktest "$WORK/log-notok.log" "$SMP"

# --- ca (b'): xoá một dòng `ok` (thiếu TAP) -----------------------------
awk '/^ok / && !d { d = 1; next } { print }' "$LOG" >"$WORK/log-missing.log"
mut_diff "$WORK/log-missing.log" "$LOG" "log: xoá 1 dòng ok"
case_red "tap-day-1.." \
    "log: xoá 1 dòng 'ok' (thiếu TAP)" \
    bash "$KTEST" --check-log ktest "$WORK/log-missing.log" "$SMP"

# --- ca (b''): nhân đôi một dòng `ok` (trùng TAP) ----------------------
awk '/^ok / && !d { print; print; d = 1; next } { print }' "$LOG" >"$WORK/log-dup.log"
mut_diff "$WORK/log-dup.log" "$LOG" "log: nhân đôi 1 dòng ok"
case_red "ok-count=$((PLAN + 1))/" \
    "log: nhân đôi 1 dòng 'ok' (trùng số TAP)" \
    bash "$KTEST" --check-log ktest "$WORK/log-dup.log" "$SMP"

# --- ca (c): thiếu một dòng `# evidence` --------------------------------
grep -v '^# obj init:' "$LOG" >"$WORK/log-noevidence.log"
mut_diff "$WORK/log-noevidence.log" "$LOG" "log: xoá evidence # obj init:"
case_red "ev1" \
    "log: xoá dòng evidence '# obj init:'" \
    bash "$KTEST" --check-log ktest "$WORK/log-noevidence.log" "$SMP"

# --- ca (c'): thiếu dòng kế hoạch TAP ----------------------------------
grep -v '^1\.\.' "$LOG" >"$WORK/log-noplan.log"
mut_diff "$WORK/log-noplan.log" "$LOG" "log: xoá dòng kế hoạch 1..N"
case_red "plan-1..$PLAN=0" \
    "log: xoá dòng kế hoạch '1..N'" \
    bash "$KTEST" --check-log ktest "$WORK/log-noplan.log" "$SMP"

# --- ca (d): PANIC thứ hai (dù đúng mẫu cho phép) ----------------------
{
    cat "$LOG"
    echo "PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)"
} >"$WORK/log-2panic.log"
mut_diff "$WORK/log-2panic.log" "$LOG" "log: thêm PANIC thứ 2"
case_red "panic-tong=2/1" \
    "log: thêm PANIC thứ 2 (đúng mẫu cho phép vẫn phải đỏ: đúng 1)" \
    bash "$KTEST" --check-log ktest "$WORK/log-2panic.log" "$SMP"

# --- ca (d'): PANIC đúng mẫu nhưng có ĐUÔI LẠ (mẫu phải neo cuối dòng) ---
awk '/^PANIC: nested trap/ { print $0 " EXTRA"; next } { print }' "$LOG" >"$WORK/log-panic-tail.log"
mut_diff "$WORK/log-panic-tail.log" "$LOG" "log: PANIC + đuôi lạ"
case_red "panic-khac=1" \
    "log: PANIC đúng mẫu nhưng thêm đuôi lạ (mẫu phải khớp cả dòng)" \
    bash "$KTEST" --check-log ktest "$WORK/log-panic-tail.log" "$SMP"

# --- ca (d''): PANIC lạ (không khớp mẫu nào) --------------------------
awk '/^PANIC: nested trap/ { print "PANIC: boom"; next } { print }' "$LOG" >"$WORK/log-panic-bad.log"
mut_diff "$WORK/log-panic-bad.log" "$LOG" "log: PANIC lạ"
case_red "panic-khac=1" \
    "log: PANIC lạ thay cho PANIC có chủ đích" \
    bash "$KTEST" --check-log ktest "$WORK/log-panic-bad.log" "$SMP"

# --- ca (e): 0 boot KHÔNG được là PASS (reviewer KT-54 mục A) ----------
case_die "SMP_LIST rỗng" \
    "0 boot (ktest.sh 1 \" \") phải die, không '0/0 boot sạch + PASS'" \
    bash "$KTEST" 1 " "

# --- ca (f): Platform HART Count sai (reviewer KT-54 mục B) ------------
EV_HART="$(ev_idx ktest "$SMP" 'Platform HART Count')"
[ -n "$EV_HART" ] || die "không tra được ordinal evidence 'Platform HART Count'"
awk -v s="$SMP" '!d && /^Platform HART Count/ { sub(": *" s, ": 1" s); d = 1 } { print }' \
    "$LOG" >"$WORK/log-hartcount.log"
mut_diff "$WORK/log-hartcount.log" "$LOG" "log: Platform HART Count sai"
case_red "ev$EV_HART" \
    "log: Platform HART Count $SMP → 1$SMP phải đỏ (mẫu neo cả dòng)" \
    bash "$KTEST" --check-log ktest "$WORK/log-hartcount.log" "$SMP"

# --- ca (g): evidence thêm đuôi lạ (reviewer KT-54 mục C) -------------
EV_MMUON="$(ev_idx ktest "$SMP" '# mmu on:')"
[ -n "$EV_MMUON" ] || die "không tra được ordinal evidence '# mmu on:'"
sed 's/^# mmu on: \(.*\)$/# mmu on: \1 EXTRA/' "$LOG" >"$WORK/log-mmuon-tail.log"
mut_diff "$WORK/log-mmuon-tail.log" "$LOG" "log: # mmu on: + đuôi lạ"
case_red "ev$EV_MMUON" \
    "log: '# mmu on: …' thêm đuôi lạ phải đỏ (mẫu neo cả dòng)" \
    bash "$KTEST" --check-log ktest "$WORK/log-mmuon-tail.log" "$SMP"

# --- ca (h): lặp dòng per-hart (reviewer KT-54 mục D) -----------------
if [ "$SMP" -ge 2 ]; then
    awk '/^# timer hart=1 /{ print "# timer hart=0 ticks=100 first=110005234 late=0 missed=0 non_monotonic=0 ok=1"; next } { print }' \
        "$LOG" >"$WORK/log-hartdup-timer.log"
mut_diff "$WORK/log-hartdup-timer.log" "$LOG" "log: lặp # timer hart=0"
    case_red "timer-hart-lines" \
        "log: lặp '# timer hart=0' thay cho hart 1 (kiểm tập hart, không chỉ đếm)" \
        bash "$KTEST" --check-log ktest "$WORK/log-hartdup-timer.log" "$SMP"
    awk '/^# hart 1 alive /{ print "# hart 0 alive ticket=0 tp=0 stack=0x80401000-0x80411000 trap=0x80411000-0x80415000 guard=0x80400000-0x80401000"; next } { print }' \
        "$LOG" >"$WORK/log-hartdup-alive.log"
mut_diff "$WORK/log-hartdup-alive.log" "$LOG" "log: lặp # hart 0 alive"
    case_red "hart1-alive" \
        "log: lặp '# hart 0 alive' thay cho hart 1" \
        bash "$KTEST" --check-log ktest "$WORK/log-hartdup-alive.log" "$SMP"
else
    total=$((total + 2))
    echo "ok $total - (bỏ qua 2 ca lặp per-hart: smp=$SMP < 2, log không có hart 1)"
fi

# --- ca (i): check_stack_lines kiểm THẬT low/block (KT-66 reviewer M3) --
# Ca "low ngoài [base,top] ⇒ ĐỎ" + "block nhỏ ⇒ ĐỎ" + "thiếu dòng ⇒ ĐỎ" + "hex hỏng ⇒ ĐỎ" +
# "log đúng ⇒ xanh" chạy trên log **tổng hợp** (không cần QEMU), trong `ktest.sh --selftest-stack`.
total=$((total + 1))
if bash "$KTEST" --selftest-stack >"$WORK/stack-selftest.log" 2>&1; then
    echo "ok $total - check_stack_lines: 5/5 ca (low ngoài, block nhỏ, thiếu dòng, hex hỏng đều ĐỎ)"
else
    echo "not ok $total - check_stack_lines selftest ĐỎ (xem $WORK/stack-selftest.log)"
    cat "$WORK/stack-selftest.log"
    fail=$((fail + 1))
fi

# --- ca (k): dòng `# ipc:` của ảnh KT-280 (IPC từ U-mode) --------------
# **KT-280** (D2-210): ảnh thứ tám `ktest-ipc` có **hai** dòng do **chính chương trình U-mode** in
# (`# ipc: umode call ok reply=…` của client, `# ipc: server ran nr=…` của server). Xoá đúng dòng
# của client ⇒ runner phải đỏ với token `ev<ordinal>` của dòng đó — chứng minh dòng `# ipc:` là
# **bắt buộc**, không phải trang trí. Log lấy từ cùng LOGDIR của runner; chưa có thì boot một vòng
# (đường không-tham-số của selftest vốn đã boot `ktest.sh 1 $SMP`).
IPC_LOG="$(bash "$KTEST" --log-path ktest-ipc "$SMP" 1)"
if [ ! -f "$IPC_LOG" ]; then
    echo "# ca (k): chưa có log ktest-ipc ($IPC_LOG) — boot một vòng matrix để lấy"
    bash "$KTEST" 1 "$SMP" >/dev/null 2>&1 || die "không lấy được log ktest-ipc"
fi
[ -f "$IPC_LOG" ] || die "không thấy log ktest-ipc $IPC_LOG"
IPC_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: umode call ok reply=')"
[ -n "$IPC_EV" ] || die "không tra được ordinal evidence '# ipc: umode call ok reply=' của ktest-ipc"
grep -v -F '# ipc: umode call ok reply=' "$IPC_LOG" >"$WORK/log-ipc-no-call.log"
mut_diff "$WORK/log-ipc-no-call.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: umode call ok reply='"
case_red "ev$IPC_EV" \
    "log ktest-ipc: xoá dòng '# ipc: umode call ok reply=' (evidence bắt buộc của KT-280)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-call.log" "$SMP"

# --- ca (l): dòng `# ipc: mr` của ảnh KT-282 (MR4..MR63 qua IPC buffer) --------------
# **KT-282** (D2-211): dòng `# ipc: mr count=8 mr7=0x…` do **chính chương trình U-mode** in sau
# khi đọc **IPC buffer** của nó (kernel ghi MR4..MR7 khi resume). Xoá đúng dòng đó ⇒ runner phải
# đỏ với token `ev<ordinal>` — chứng minh dòng `# ipc: mr` là evidence **bắt buộc**, không phải
# trang trí (đột biến M1/M2/M3 của KT-282 làm dòng này sai/thiếu).
MR_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: mr count=8 mr7=')"
[ -n "$MR_EV" ] || die "không tra được ordinal evidence '# ipc: mr count=8 mr7=' của ktest-ipc"
grep -v -F '# ipc: mr count=8 mr7=' "$IPC_LOG" >"$WORK/log-ipc-no-mr.log"
mut_diff "$WORK/log-ipc-no-mr.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: mr count=8 mr7='"
case_red "ev$MR_EV" \
    "log ktest-ipc: xoá dòng '# ipc: mr count=8 mr7=' (evidence bắt buộc của KT-282)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-mr.log" "$SMP"

# --- ca (m): dòng `# ipc: configure` của ảnh KT-290 (Tcb.Configure qua invocation) ----------
# **KT-290**: dòng `# ipc: configure ok va=0x…` do **chính chương trình U-mode** in sau khi
# `Call` trên cap TCB với `label = Tcb.Configure` trả `error = 0`. Xoá đúng dòng đó ⇒ runner phải
# đỏ với token `ev<ordinal>` — chứng minh dòng `# ipc: configure` là evidence **bắt buộc**.
CFG_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: configure ok va=')"
[ -n "$CFG_EV" ] || die "không tra được ordinal evidence '# ipc: configure ok va=' của ktest-ipc"
grep -v -F '# ipc: configure ok va=' "$IPC_LOG" >"$WORK/log-ipc-no-cfg.log"
mut_diff "$WORK/log-ipc-no-cfg.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: configure ok va='"
case_red "ev$CFG_EV" \
    "log ktest-ipc: xoá dòng '# ipc: configure ok va=' (evidence bắt buộc của KT-290)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-cfg.log" "$SMP"

# --- ca (o): dòng `# ipc caps:` của ảnh KT-309 (cap transfer qua IPC buffer) ----------
# **KT-309**: dòng `# ipc caps: sent=1 received=1 slot=5 rights=0x…` do **kernel** in sau khi
# **copy** cap `Frame` từ CSpace client sang CSpace server qua **vùng dự phòng** của IPC buffer.
# Xoá đúng dòng đó ⇒ runner phải đỏ với token `ev<ordinal>` — chứng minh dòng `# ipc caps:` là
# evidence **bắt buộc**, không phải trang trí (đột biến M1/M2/M3 của KT-309 làm dòng này
# sai/thiếu: thiếu `RIGHT_GRANT` ⇒ 3, `n > 4` ⇒ 5, slot đích bận ⇒ 7).
CAP_EV="$(ev_idx ktest-ipc "$SMP" '# ipc caps: sent=1 received=1 slot=5 rights=0x')"
[ -n "$CAP_EV" ] || die "không tra được ordinal evidence '# ipc caps: sent=1 received=1 slot=5 rights=0x' của ktest-ipc"
grep -v -F '# ipc caps: sent=1 received=1 slot=5 rights=0x' "$IPC_LOG" >"$WORK/log-ipc-no-caps.log"
mut_diff "$WORK/log-ipc-no-caps.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc caps: sent=1 received=1 slot=5 rights=0x'"
case_red "ev$CAP_EV" \
    "log ktest-ipc: xoá dòng '# ipc caps: sent=1 received=1 slot=5 rights=0x' (evidence bắt buộc của KT-309)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-caps.log" "$SMP"

# --- ca (p): dòng `# ipc: configure2` của ảnh KT-311 (Tcb.Configure kênh đầy đủ) ----------
# **KT-311**: dòng `# ipc: configure2 entry=0x… prio=4 cspace=… vspace=0 calls=2 denied=5 ok=1`
# do **kernel** in sau khi đo **độc lập** `TcbData.entry`/`priority`/`cspace` của TCB client
# (client đã đổi `cspace` sang `cnode_alt` rồi trả về). Xoá đúng dòng đó ⇒ runner phải đỏ với
# token `ev<ordinal>` — chứng minh dòng `# ipc: configure2` là evidence **bắt buộc**, không phải
# trang trí (đột biến M1/M2/M3 của KT-311 làm dòng này sai/thiếu).
CFG2_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: configure2 entry=0x')"
[ -n "$CFG2_EV" ] || die "không tra được ordinal evidence '# ipc: configure2 entry=0x' của ktest-ipc"
grep -v -F '# ipc: configure2 entry=0x' "$IPC_LOG" >"$WORK/log-ipc-no-cfg2.log"
mut_diff "$WORK/log-ipc-no-cfg2.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: configure2 entry=0x'"
case_red "ev$CFG2_EV" \
    "log ktest-ipc: xoá dòng '# ipc: configure2 entry=0x' (evidence bắt buộc của KT-311)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-cfg2.log" "$SMP"

# --- ca (q): dòng `# ipc: resume` của ảnh KT-314 (Tcb.Resume tiêu thụ TcbData.entry) ----------
# **KT-314**: dòng `# ipc: resume ok entry=0x… state=Running` do **kernel** in sau khi đo **độc
# lập** `TcbData.entry` + trạng thái **thật** của TCB đích (client vừa `Resume` nó từ U-mode).
# Xoá đúng dòng đó ⇒ runner phải đỏ với token `ev<ordinal>` — chứng minh dòng `# ipc: resume` là
# evidence **bắt buộc**, không phải trang trí (đột biến M1/M2/M3 của KT-314 làm dòng này
# sai/thiếu: `entry = 0` ⇒ 5, tác vụ `Running` ⇒ 17, bỏ kiểm trạng thái ⇒ chạy lại tác vụ đã
# `Exit`).
RESUME_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: resume ok entry=0x')"
[ -n "$RESUME_EV" ] || die "không tra được ordinal evidence '# ipc: resume ok entry=0x' của ktest-ipc"
grep -v -F '# ipc: resume ok entry=0x' "$IPC_LOG" >"$WORK/log-ipc-no-resume.log"
mut_diff "$WORK/log-ipc-no-resume.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: resume ok entry=0x'"
case_red "ev$RESUME_EV" \
    "log ktest-ipc: xoá dòng '# ipc: resume ok entry=0x' (evidence bắt buộc của KT-314)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-resume.log" "$SMP"

# --- ca (q2): dòng `# ipc: vspace` của ảnh KT-320 (`Tcb.Configure` tham số `vspace?`) ----------
# **KT-320**: dòng `# ipc: vspace ok obj=… applied=… match=1 root=0x… asid=…` do **kernel** in sau
# khi đo **độc lập** `TcbData.vspace` (client vừa ghi từ U-mode) **và** kết quả arch **tiêu thụ**
# VSpace khi vào U-mode. Xoá đúng dòng đó ⇒ runner phải đỏ với token `ev<ordinal>` — chứng minh
# dòng `# ipc: vspace` là evidence **bắt buộc**, không phải trang trí (ba đột biến của KT-320 làm
# nó sai/thiếu: bỏ kiểm loại ⇒ nhận `Frame`; bỏ kiểm mask lạ ⇒ nhận mask lạ; bỏ hook arch ⇒
# `applied`/`obj` không khớp).
VSPACE_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: vspace ok obj=')"
[ -n "$VSPACE_EV" ] || die "không tra được ordinal evidence '# ipc: vspace ok obj=' của ktest-ipc"
grep -v -F '# ipc: vspace ok obj=' "$IPC_LOG" >"$WORK/log-ipc-no-vspace.log"
mut_diff "$WORK/log-ipc-no-vspace.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: vspace ok obj='"
case_red "ev$VSPACE_EV" \
    "log ktest-ipc: xoá dòng '# ipc: vspace ok obj=' (evidence bắt buộc của KT-320)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-vspace.log" "$SMP"

# --- ca (r): dòng `# ipc: suspend` của ảnh KT-319 (Tcb.Suspend giữ entry để Resume chạy lại) ----
# **KT-319**: dòng `# ipc: suspend state=Inactive suspended=0 calls=2 resumed=Running` do **kernel**
# in sau khi đo **độc lập** `Tcb.ReadState` của TCB đích thứ ba (client vừa `Suspend` nó từ U-mode)
# + **đúng hai** `Tcb.Suspend` thành công (đích `Ready` và đích `Running`) + đích `Running` vẫn
# `Running` sau cặp `Suspend`/`Resume`. Xoá đúng dòng đó ⇒ runner phải đỏ với token `ev<ordinal>` —
# chứng minh dòng `# ipc: suspend` là evidence **bắt buộc**, không phải trang trí (đột biến của
# KT-319 làm dòng này sai/thiếu: bỏ kiểm trạng thái ⇒ đích `Inactive` "treo" lại; bỏ kiểm `mode`
# ⇒ nhận `Defer` và tác vụ **không có đường đánh thức**).
SUSPEND_EV="$(ev_idx ktest-ipc "$SMP" '# ipc: suspend state=Inactive')"
[ -n "$SUSPEND_EV" ] || die "không tra được ordinal evidence '# ipc: suspend state=Inactive' của ktest-ipc"
grep -v -F '# ipc: suspend state=Inactive' "$IPC_LOG" >"$WORK/log-ipc-no-suspend.log"
mut_diff "$WORK/log-ipc-no-suspend.log" "$IPC_LOG" "log ktest-ipc: xoá dòng '# ipc: suspend state=Inactive'"
case_red "ev$SUSPEND_EV" \
    "log ktest-ipc: xoá dòng '# ipc: suspend state=Inactive' (evidence bắt buộc của KT-319)" \
    bash "$KTEST" --check-log ktest-ipc "$WORK/log-ipc-no-suspend.log" "$SMP"


# --- ca (n): dòng `# netping:` của ảnh KT-306 (NetPing là syscall thật) --------------
# **KT-306** (D2-219): dòng `# netping: label=0x291 reply=0 rtt_ms=…` do **chính chương trình
# U-mode** in sau khi syscall `Call` trên `EP_KERN` trả kết quả của **dịch vụ kernel** `NetPing`
# (`net_svc::NetSvc` — kiểm quyền rồi mới chạm driver). Xoá đúng dòng đó ⇒ runner phải đỏ với
# token `ev<ordinal>` — chứng minh dòng `# netping:` là evidence **bắt buộc**, không phải trang trí.
NETSRV_LOG="$(bash "$KTEST" --log-path ktest-netsrv "$SMP" 1)"
if [ ! -f "$NETSRV_LOG" ]; then
    echo "# ca (n): chưa có log ktest-netsrv ($NETSRV_LOG) — boot một vòng matrix để lấy"
    bash "$KTEST" 1 "$SMP" >/dev/null 2>&1 || die "không lấy được log ktest-netsrv"
fi
[ -f "$NETSRV_LOG" ] || die "không thấy log ktest-netsrv $NETSRV_LOG"
NP_EV="$(ev_idx ktest-netsrv "$SMP" '# netping: label=0x291 reply=0 rtt_ms=')"
[ -n "$NP_EV" ] || die "không tra được ordinal evidence '# netping: label=0x291 reply=0 rtt_ms=' của ktest-netsrv"
grep -v -F '# netping: label=0x291 reply=0 rtt_ms=' "$NETSRV_LOG" >"$WORK/log-netsrv-no-netping.log"
mut_diff "$WORK/log-netsrv-no-netping.log" "$NETSRV_LOG" "log ktest-netsrv: xoá dòng '# netping: label=0x291 reply=0 rtt_ms='"
case_red "ev$NP_EV" \
    "log ktest-netsrv: xoá dòng '# netping: label=0x291 reply=0 rtt_ms=' (evidence bắt buộc của KT-306)" \
    bash "$KTEST" --check-log ktest-netsrv "$WORK/log-netsrv-no-netping.log" "$SMP"

# --- ca cuối: expected.txt trả lại **nguyên vẹn** (sha256 + cmp) ------
total=$((total + 1))
SHA_AFTER="$(sha_of "$EXPECTED")"
if [ "$SHA_BEFORE" = "$SHA_AFTER" ] && cmp -s "$WORK/expected.orig" "$EXPECTED"; then
    echo "ok $total - expected.txt nguyên vẹn sau mutation: sha256 $(printf '%s' "$SHA_AFTER" | cut -c1-16)… (trước = sau) + cmp"
else
    echo "not ok $total - expected.txt ĐÃ ĐỔI sau mutation (sha256 trước $SHA_BEFORE / sau $SHA_AFTER)"
    fail=$((fail + 1))
fi

echo "# selftest: $((total - fail))/$total ca đúng như kỳ vọng"
[ "$fail" -eq 0 ] || exit 1
echo "# selftest.sh: PASS"
