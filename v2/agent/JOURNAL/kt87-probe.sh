#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-87 — ca kiểm "cổng nhìn thấy tệp **chưa commit**" (5 ca bắt buộc của KT-87).
#
# Lỗ đã xảy ra hai lần trong một buổi (KT-85; KT-86 + KT-77c): tài liệu mới còn **untracked**
# ⇒ `v2 check` xanh; commit xong ⇒ `v2 check` đỏ. Script này chứng minh cổng đã nhìn thấy tệp
# **sẽ được commit** và dự đoán đúng kết quả sau commit.
#
# Chạy: bash v2/agent/JOURNAL/kt87-probe.sh          # exit 0 = mọi ca đúng kỳ vọng
#
# Ràng buộc:
#   * **KHÔNG** `git add`/`git commit` trên cây thật — mọi thao tác index/commit chỉ trong clone
#     tạm `${TMPDIR:-/tmp}/kt87-clone.*` (A1-177: index dùng chung đã gây nhiễm chéo hai lần).
#   * Tệp thử trên cây thật được `trap` xoá khi thoát (kể cả khi lỗi/ngắt) — không để lại rác.
#   * Băm nội dung tài liệu đích trước/sau: cây thật không bị script làm đổi.
#
# Năm ca của KT-87:
#   (a) tệp thử **untracked** có số ở prose ⇒ luật 1 ĐỎ, mỗi lỗi in trạng thái `untracked`;
#   (b) `git add` tệp thử (trong clone) ⇒ vẫn ĐỎ, in trạng thái `staged`;
#   (c) commit thử (trong clone) ⇒ vẫn ĐỎ, in trạng thái `tracked`;
#   (d) xoá tệp thử ⇒ XANH trở lại (không còn lỗi nào của tệp thử, không để lại rác);
#   (e) cây thật (untracked) == clone tạm (đã commit): **cùng tập lỗi** (file:dòng:mã) — đây là
#       ca quan trọng nhất, chứng minh "cổng nhìn thấy đúng cái sẽ được commit".
# Đối chứng thêm: `--staged-only` trên cùng cây KHÔNG thấy tệp thử — đúng lỗ cũ mà KT-87 vá.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"      # gốc repo (v2/agent/JOURNAL -> 3 cấp)
# Tệp thử **phải khớp glob tài liệu đích** `v2/docs/k6-*.md` của numbers-policy.toml, nếu không
# nó không phải tài liệu đích và luật 1 không có nghĩa vụ quét nó. Đề bài ghi `kt87-probe.md`;
# tên đó KHÔNG khớp glob — script kiểm luôn ca đối chứng đó (xem check_literal_name).
PROBE_REL="v2/docs/k6-kt87-probe.md"
PROBE_LITERAL="v2/docs/kt87-probe.md"

PY=""
for p in python3.12 python3.11 python3; do
    if command -v "$p" >/dev/null 2>&1 && "$p" -c 'import tomllib' >/dev/null 2>&1; then
        PY="$(command -v "$p")"
        break
    fi
done
[ -n "$PY" ] || {
    echo "kt87-probe: cần python ≥ 3.11 có tomllib" >&2
    exit 2
}

TMP="$(mktemp -d "${TMPDIR:-/tmp}/kt87-clone.XXXXXX")" || exit 2
CLONE="$TMP/clone"

cleanup() {
    rm -f "$ROOT/$PROBE_REL" "$ROOT/$PROBE_LITERAL"
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

TAP=0
FAILS=0
ok() { TAP=$((TAP + 1)); echo "ok $TAP - $1"; }
nok() { TAP=$((TAP + 1)); FAILS=$((FAILS + 1)); echo "not ok $TAP - $1"; }

# run_law1 <gốc> [cờ...]: chạy luật 1 của verifier trên <gốc>; in stdout, trả exit của verifier.
run_law1() {
    local root="$1"
    shift
    "$PY" "$ROOT/v2/tools/verify-numbers.py" --root "$root" --law 1 "$@" 2>&1
}

# errors: chỉ giữ dòng lỗi, bỏ nhãn trạng thái `[tracked]/[staged …]/[untracked …]` để so tập lỗi
# giữa hai cây (nhãn là thứ **được phép** khác nhau giữa "chưa commit" và "đã commit").
errors() { grep '^not ok - luat' | sed -E 's/\[(tracked|staged|untracked|no-git)[^]]*\] //' | sort; }
probe_only() { grep -F "$PROBE_REL" || true; }

# snapshot <gốc>: băm nội dung mọi tài liệu đích + registry + verifier (phát hiện cây đổi giữa
# hai lần chạy — task song song đang ghi thì so sánh phải công bằng, không báo động giả).
snapshot() {
    (
        cd "$1" || return 1
        {
            ls -1d v2/docs/k6-*.md v2/agent/reports/*.md 2>/dev/null
            printf '%s\n' \
                v2/test/rules/numbers-policy.toml \
                v2/test/rules/counters.toml \
                v2/test/rules/criteria.toml \
                v2/tools/verify-numbers.py
        } | sort -u | tr '\n' '\0' | xargs -0 shasum -a 256 2>/dev/null |
            shasum -a 256 | awk '{print $1}'
    )
}

REAL_HEAD="$(git -C "$ROOT" rev-parse HEAD)"
REAL_BEFORE="$(snapshot "$ROOT")"
echo "TAP version 13"
echo "# kt87-probe: root=$ROOT head=${REAL_HEAD:0:8}"

# ---------------------------------------------------------------- đối chứng nền
BASE_OUT="$(run_law1 "$ROOT")"
BASE_RC=$?
BASE_ERR="$(printf '%s\n' "$BASE_OUT" | errors)"
echo "# nền (chưa có tệp thử): exit=$BASE_RC lỗi=$(printf '%s\n' "$BASE_ERR" | grep -c . )"

# ---------------------------------------------------------------- cài tệp thử (untracked)
cat >"$ROOT/$PROBE_REL" <<'EOF'
# Ca kiểm KT-87 — tệp thử, phải xoá

Số đo thử ở prose (không nằm trong khối `scope=`): 987654.
EOF
if [ -f "$ROOT/$PROBE_REL" ]; then ok "cài tệp thử untracked $PROBE_REL"; else nok "không tạo được tệp thử"; fi
REAL_WITH_PROBE="$(snapshot "$ROOT")"   # ảnh cây thật **có** tệp thử: mốc so với clone
if [ "$REAL_WITH_PROBE" != "$REAL_BEFORE" ]; then
    ok "ảnh cây thật đổi đúng một tệp thử (mốc so sánh clone)"
else
    nok "ảnh cây thật không đổi sau khi thêm tệp thử (snapshot sai?)"
fi
if [ -z "$(git -C "$ROOT" status --porcelain -- "$PROBE_REL")" ]; then
    nok "tệp thử phải hiện untracked trong git status"
else
    ok "tệp thử đang untracked (git status thấy ??)"
fi

# ---------------------------------------------------------------- (a) untracked ⇒ ĐỎ + nhãn
A_OUT="$(run_law1 "$ROOT")"
A_RC=$?
A_ERR="$(printf '%s\n' "$A_OUT" | errors)"
printf '%s\n' "$A_OUT" >"$TMP/a.out"
if [ "$A_RC" -ne 0 ]; then ok "(a) luật 1 ĐỎ khi tệp thử còn untracked"; else nok "(a) luật 1 vẫn XANH dù có tệp thử untracked"; fi
if printf '%s\n' "$A_OUT" | probe_only | grep -q 'E1_NUMBER_OUTSIDE'; then
    ok "(a) tệp untracked bị bắt đúng mã E1_NUMBER_OUTSIDE"
else
    nok "(a) tệp untracked KHÔNG bị bắt E1_NUMBER_OUTSIDE"
fi
if printf '%s\n' "$A_OUT" | grep -F "$PROBE_REL" | grep -q '\[untracked'; then
    ok "(a) thông điệp in trạng thái [untracked]"
else
    nok "(a) thông điệp KHÔNG ghi untracked"
fi
if printf '%s\n' "$A_OUT" | grep -qF 'tệp chưa commit — cổng vẫn phải xanh trước khi commit'; then
    ok "(a) thông điệp nêu 'tệp chưa commit — cổng vẫn phải xanh trước khi commit'"
else
    nok "(a) thông điệp thiếu câu 'tệp chưa commit — cổng vẫn phải xanh trước khi commit'"
fi
# mọi dòng lỗi phải có nhãn trạng thái (KT-87 yêu cầu *mỗi* lỗi nói nguồn tệp)
if printf '%s\n' "$A_OUT" | grep -E '^not ok - luat' | grep -E ': E[0-9]+_' |
    grep -vqE '\[(tracked|staged|untracked|no-git)[^]]*\]'; then
    nok "(a) có dòng lỗi KHÔNG kèm nhãn trạng thái tệp"
else
    ok "(a) mọi dòng lỗi đều kèm nhãn trạng thái tệp"
fi

# đối chứng lỗ cũ: --staged-only (tập index) không thấy tệp chưa `git add`
OLD_OUT="$(run_law1 "$ROOT" --staged-only)"
if printf '%s\n' "$OLD_OUT" | probe_only | grep -q .; then
    nok "đối chứng --staged-only: vẫn thấy tệp untracked (cờ chẩn đoán sai)"
else
    ok "đối chứng --staged-only: KHÔNG thấy tệp untracked (đúng lỗ cũ mà KT-87 vá)"
fi

# tên trong đề bài không khớp glob đích ⇒ không phải ca kiểm của luật 1 (ghi nhận, không tính lỗi)
cat >"$ROOT/$PROBE_LITERAL" <<'EOF'
# Tên trong đề bài KT-87 (không khớp `v2/docs/k6-*.md`)

Số đo thử: 987655.
EOF
if run_law1 "$ROOT" | grep -qF "$PROBE_LITERAL"; then
    nok "tên $PROBE_LITERAL bị luật 1 quét — ngoài glob đích mà vẫn bị bắt (sai phạm vi)"
else
    ok "tên $PROBE_LITERAL không thuộc tài liệu đích (không khớp glob k6-*.md) — không phải ca kiểm"
fi
rm -f "$ROOT/$PROBE_LITERAL"

# ---------------------------------------------------------------- clone tạm (cùng HEAD, cùng tệp)
git clone -q --shared "$ROOT" "$CLONE" || {
    echo "not ok 0 - không clone được repo" >&2
    exit 2
}
for d in v2/bin v2/tools v2/test v2/docs v2/agent docs/v1 docs/v2; do
    rm -rf "$CLONE/$d"
    mkdir -p "$(dirname "$CLONE/$d")"
    cp -a "$ROOT/$d" "$CLONE/$d"
done
CLONE_HEAD="$(git -C "$CLONE" rev-parse HEAD)"
if [ "$CLONE_HEAD" = "$REAL_HEAD" ]; then
    ok "(clone) HEAD clone == HEAD cây thật (${REAL_HEAD:0:8})"
else
    nok "(clone) HEAD clone ${CLONE_HEAD:0:8} != HEAD cây thật ${REAL_HEAD:0:8}"
fi
if [ "$(snapshot "$CLONE")" = "$REAL_WITH_PROBE" ]; then
    ok "(clone) cùng tệp + cùng nội dung tài liệu đích với cây thật (có tệp thử)"
else
    nok "(clone) nội dung tài liệu đích KHÁC cây thật"
    diff <(snapshot "$ROOT") <(snapshot "$CLONE") | sed 's/^/#   /'
fi

A2_OUT="$(run_law1 "$CLONE")"
A2_ERR="$(printf '%s\n' "$A2_OUT" | errors)"
if [ "$A2_ERR" = "$A_ERR" ]; then
    ok "(clone) bản sao cho cùng tập lỗi với cây thật (cùng tệp untracked)"
else
    nok "(clone) tập lỗi khác cây thật"
    diff <(printf '%s\n' "$A_ERR") <(printf '%s\n' "$A2_ERR") | sed 's/^/#   /'
fi

# ---------------------------------------------------------------- (b) staged ⇒ ĐỎ + nhãn
git -C "$CLONE" add "$PROBE_REL" || nok "(b) không git add được trong clone"
B_OUT="$(run_law1 "$CLONE")"
B_RC=$?
printf '%s\n' "$B_OUT" >"$TMP/b.out"
if [ "$B_RC" -ne 0 ]; then ok "(b) luật 1 ĐỎ khi tệp thử đã git add (staged)"; else nok "(b) XANH dù tệp thử đã staged"; fi
if printf '%s\n' "$B_OUT" | grep -F "$PROBE_REL" | grep -q '\[staged'; then
    ok "(b) thông điệp in trạng thái [staged]"
else
    nok "(b) thông điệp KHÔNG ghi staged"
fi
if [ "$(printf '%s\n' "$B_OUT" | errors)" = "$A_ERR" ]; then
    ok "(b) tập lỗi staged == tập lỗi untracked (chỉ đổi nhãn trạng thái)"
else
    nok "(b) tập lỗi staged khác untracked"
fi

# ---------------------------------------------------------------- (c) committed (clone) ⇒ ĐỎ + nhãn
git -C "$CLONE" -c user.name=kt87 -c user.email=kt87@example.invalid \
    commit -q -m "[KT-87] ca kiểm: tệp thử chưa commit (clone tạm)" || nok "(c) không commit được trong clone"
C_OUT="$(run_law1 "$CLONE")"
C_RC=$?
printf '%s\n' "$C_OUT" >"$TMP/c.out"
if [ "$C_RC" -ne 0 ]; then ok "(c) luật 1 ĐỎ khi tệp thử đã commit"; else nok "(c) XANH sau khi commit tệp thử"; fi
if printf '%s\n' "$C_OUT" | grep -F "$PROBE_REL" | grep -q '\[tracked'; then
    ok "(c) thông điệp in trạng thái [tracked]"
else
    nok "(c) thông điệp KHÔNG ghi tracked"
fi
if [ "$(git -C "$CLONE" rev-parse HEAD^)" = "$REAL_HEAD" ]; then
    ok "(c) commit thử chỉ thêm tệp thử: HEAD^ của clone == HEAD cây thật"
else
    nok "(c) HEAD^ của clone != HEAD cây thật"
fi

# ---------------------------------------------------------------- (e) cây thật == clone tạm
C_ERR="$(printf '%s\n' "$C_OUT" | errors)"
if [ "$A_ERR" = "$C_ERR" ]; then
    ok "(e) **cây thật (untracked) == clone tạm (đã commit)**: cùng tập lỗi file:dòng:mã"
else
    nok "(e) cây thật và clone tạm cho tập lỗi KHÁC nhau"
    diff <(printf '%s\n' "$A_ERR") <(printf '%s\n' "$C_ERR") | sed 's/^/#   /'
fi
echo "# (e) lỗi trên cây thật (untracked):"
printf '%s\n' "$A_ERR" | sed 's/^/#   /'
echo "# (e) lỗi trên clone tạm (đã commit):"
printf '%s\n' "$C_ERR" | sed 's/^/#   /'
# bằng chứng nguyên văn: cùng một tệp, cùng dòng, cùng mã — chỉ **nhãn trạng thái** đổi
echo "# (e) nguyên văn trên cây thật (chưa commit):"
printf '%s\n' "$A_OUT" | grep -E ': E[0-9]+_|^# verify-numbers:' | sed 's/^/#   /'
echo "# (e) nguyên văn trên clone tạm (đã commit):"
printf '%s\n' "$C_OUT" | grep -E ': E[0-9]+_|^# verify-numbers:' | sed 's/^/#   /'
echo "# (b) nguyên văn sau git add (staged):"
printf '%s\n' "$B_OUT" | grep -E ': E[0-9]+_|^# verify-numbers:' | sed 's/^/#   /'
echo "# đối chứng --staged-only (tập index, đúng hành vi trước KT-87):"
printf '%s\n' "$OLD_OUT" | grep -E ': E[0-9]+_|^# verify-numbers:' | sed 's/^/#   /'

# ---------------------------------------------------------------- (d) xoá tệp ⇒ XANH trở lại
# Bỏ commit thử mà KHÔNG `reset --hard`: clone đang giữ các bản sửa **chưa commit** của cây thật
# (overlay ở trên) — `--hard` sẽ revert chúng và ca (d) so sai. Chỉ bỏ tệp thử khỏi HEAD+index.
git -C "$CLONE" reset -q --soft HEAD^ || nok "(d) không reset mềm được clone"
git -C "$CLONE" rm -q --cached "$PROBE_REL" || nok "(d) không bỏ được tệp thử khỏi index clone"
rm -f "$CLONE/$PROBE_REL"
D_OUT="$(run_law1 "$CLONE")"
D_RC=$?
if printf '%s\n' "$D_OUT" | probe_only | grep -q .; then
    nok "(d) clone sau khi xoá tệp vẫn còn lỗi của tệp thử"
else
    ok "(d) clone sau khi xoá tệp: không còn lỗi của tệp thử"
fi
if [ "$(printf '%s\n' "$D_OUT" | errors)" = "$BASE_ERR" ]; then
    ok "(d) clone sau khi xoá tệp == nền (không để lại rác)"
else
    nok "(d) clone sau khi xoá tệp khác nền"
fi
if [ "$BASE_RC" -ne 0 ] || [ "$D_RC" -eq 0 ]; then
    ok "(d) exit sau khi xoá tệp: $D_RC (nền $BASE_RC)"
else
    nok "(d) exit sau khi xoá tệp = $D_RC nhưng nền XANH"
fi

# xoá tệp thử trên **cây thật** rồi chạy lại (ca (d) trên cây thật)
rm -f "$ROOT/$PROBE_REL"
if [ -e "$ROOT/$PROBE_REL" ]; then nok "(d) tệp thử còn trên cây thật"; else ok "(d) tệp thử đã xoá khỏi cây thật"; fi
E_OUT="$(run_law1 "$ROOT")"
E_RC=$?
if printf '%s\n' "$E_OUT" | probe_only | grep -q .; then
    nok "(d) cây thật sau khi xoá tệp vẫn còn lỗi của tệp thử"
else
    ok "(d) cây thật sau khi xoá tệp: không còn lỗi của tệp thử"
fi
if [ "$(printf '%s\n' "$E_OUT" | errors)" = "$BASE_ERR" ]; then
    ok "(d) cây thật sau khi xoá tệp == nền (không để lại rác)"
else
    nok "(d) cây thật sau khi xoá tệp khác nền"
fi
if [ -z "$(git -C "$ROOT" status --porcelain -- "$PROBE_REL")" ]; then
    ok "(d) git status cây thật không còn dấu vết tệp thử"
else
    nok "(d) git status cây thật còn dấu vết tệp thử"
fi

# cây thật không bị script làm đổi nội dung (ngoài tệp thử đã xoá)
REAL_AFTER="$(snapshot "$ROOT")"
if [ "$REAL_AFTER" = "$REAL_BEFORE" ]; then
    ok "cây thật không đổi nội dung tài liệu đích (sha trước == sau)"
else
    nok "cây thật ĐỔI nội dung tài liệu đích trong lúc chạy (task song song?)"
    echo "#   trước=$REAL_BEFORE" ; echo "#   sau  =$REAL_AFTER"
fi

echo "# kt87-probe: ok=$((TAP - FAILS)) fail=$FAILS total=$TAP"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
