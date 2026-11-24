#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-78 — **tự phá** script kiểm số (`kt78-verify-numbers.sh`) trên **bản copy** (D2-113F).
#
# Vì sao bản copy: KT-73 đã gây race khi tự phá trên cây thật (script khôi phục tài liệu đúng lúc
# verifier đọc). Từ đó (D2-113F): mọi ca tự phá chạy trên **bản copy**, và phải chứng minh **md5 tài
# liệu thật không đổi** trước/sau.
#
# Sáu đường phá (mỗi đường phải làm verifier **ĐỎ**):
#   1. đổi **một chữ số** trong khối `scope=` (số đo bịa)                        → khối không khớp log;
#   2. thêm **một chữ số trần** vào prose (ngoài khối, ngoài backtick)           → D2-112b;
#   3. trỏ một khối sang **log khác** (hoán nguồn)                               → khối không khớp log;
#   4. đổi một dòng trong khối thành câu khác nhưng vẫn "có vẻ" đúng             → khối không khớp log;
#   5. chèn fence ```text KHÔNG `scope=` chứa số đo bịa (đường reviewer đã phá bản trước);
#   6. thêm số **thập phân** viết tay vào prose (D2-117: KT-76 từng bị phá đúng đường này).
#
# Dùng: bash v2/agent/JOURNAL/kt78-verify-selfbreak.sh
# Exit: 0 = cả bốn đường bị bắt (và cây thật không đổi); 1 = có đường lọt.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
VERIFY="$HERE/kt78-verify-numbers.sh"
LOGDIR_SRC="$HERE"
DOCS=(
    "v2/docs/k6-harness.md"
    "v2/docs/k3b-core.md"
    "v2/agent/STATE.md"
    "v2/agent/JOURNAL/kt78-journal.md"
)

[ -f "$VERIFY" ] || {
    echo "kt78-selfbreak: không thấy $VERIFY" >&2
    exit 2
}
WORK="$(mktemp -d "${TMPDIR:-/tmp}/kt78-selfbreak.XXXXXX")" || exit 2
trap 'rm -rf "$WORK"' EXIT
FAILS=0
CASES=0
ok() {
    CASES=$((CASES + 1))
    echo "ok $CASES - $*"
}
no() {
    CASES=$((CASES + 1))
    echo "not ok $CASES - $*"
    FAILS=$((FAILS + 1))
}

# md5 toàn bộ tài liệu thật **trước** khi phá (D2-113F).
md5_real() {
    (
        cd "$ROOT" || exit 1
        for f in "${DOCS[@]}"; do
            [ -f "$f" ] && md5 -q "$f" 2>/dev/null || md5sum "$f" 2>/dev/null | awk '{print $1}'
        done
    )
}
BEFORE="$(md5_real)"

# Bản copy: tài liệu + **bản sao log** (verifier đọc `$ROOT/v2/agent/JOURNAL/<scope>`).
cp_tree() { # $1 = tên ca
    local d="$WORK/$1"
    mkdir -p "$d/v2/docs" "$d/v2/agent/JOURNAL" || return 1
    cp "$ROOT/v2/docs/k6-harness.md" "$d/v2/docs/" 2>/dev/null
    cp "$ROOT/v2/docs/k3b-core.md" "$d/v2/docs/" 2>/dev/null
    cp "$ROOT/v2/agent/STATE.md" "$d/v2/agent/" 2>/dev/null
    cp "$ROOT/v2/agent/JOURNAL/kt78-journal.md" "$d/v2/agent/JOURNAL/" 2>/dev/null
    # log nguồn (chỉ log của KT-78, không cần log task khác)
    cp "$LOGDIR_SRC"/kt78-*.log "$d/v2/agent/JOURNAL/" 2>/dev/null
    mkdir -p "$d/v2/agent/JOURNAL/kt78-guard" 2>/dev/null
    cp "$LOGDIR_SRC"/kt78-guard/*.log "$d/v2/agent/JOURNAL/kt78-guard/" 2>/dev/null
    echo "$d"
}

run_case() { # $1=tên $2=mô-tả $3=đường-dẫn-copy $4=lệnh-phá
    local name="$1" desc="$2" dir="$3" mut="$4"
    if ! (cd "$dir" && eval "$mut"); then
        no "$name: không thực hiện được phép phá ($desc)"
        return 1
    fi
    if bash "$VERIFY" "$dir" >"$WORK/$name.out" 2>&1; then
        no "$name: verifier vẫn XANH sau khi phá — $desc"
        sed -n '1,20p' "$WORK/$name.out" | sed 's/^/#   /'
        return 1
    fi
    ok "$name: verifier ĐỎ đúng như kỳ vọng — $desc"
    return 0
}

# ------------------------------------------------------------------ đường 1: đổi chữ số trong khối
D="$(cp_tree mut1)"
run_case mut1 "đổi một chữ số trong khối scope (số đo bịa)" "$D" '
    f=v2/docs/k3b-core.md
    perl -0pi -e "s/# KTEST: 21\/21 boot sạch/# KTEST: 22\/21 boot sạch/g" "$f"
' || true

# ------------------------------------------------------------------ đường 2: chữ số trần trong prose
D="$(cp_tree mut2)"
run_case mut2 "thêm chữ số trần vào prose (ngoài khối)" "$D" '
    f=v2/docs/k6-harness.md
    printf "\nKiểm lại 3 lần trên máy.\n" >>"$f"
' || true

# ------------------------------------------------------------------ đường 3: hoán nguồn log
D="$(cp_tree mut3)"
run_case mut3 "trỏ một khối scope sang log khác (hoán nguồn)" "$D" '
    f=v2/docs/k3b-core.md
    perl -0pi -e "s/scope=kt78-ktest-matrix\.log/scope=kt78-gate.log/" "$f"
' || true

# ------------------------------------------------------------------ đường 4: sửa một dòng trong khối
D="$(cp_tree mut4)"
run_case mut4 "đổi một dòng trong khối thành câu khác" "$D" '
    f=v2/docs/k3b-core.md
    perl -0pi -e "s/boot_done=21 verdict=OK/boot_done=21 verdict=OK_DA_SUA/" "$f"
' || true

# ------------------------------------------- đường 5: fence ```text KHÔNG scope= chứa số đo
# Đây đúng là đường reviewer phá được bản trước: fence thường bị bỏ qua ⇒ chèn số bịa vẫn PASS.
D="$(cp_tree mut5)"
run_case mut5 "chèn fence \`\`\`text KHÔNG scope= chứa số đo bịa" "$D" '
    f=v2/docs/k6-harness.md
    printf "\n\x60\x60\x60text\nboot_done=999 isolated_0=888\n\x60\x60\x60\n" >>"$f"
' || true

# ------------------------------- đường 6: số **thập phân** viết tay trong prose (D2-117)
# KT-76 từng bị phá đúng đường này (miễn mẫu `^\d+\.\d+$`); verifier KT-78 **không** miễn.
D="$(cp_tree mut6)"
run_case mut6 "thêm số thập phân viết tay vào prose (D2-117)" "$D" '
    f=v2/docs/k6-harness.md
    printf "\n\u1ea2nh vuot 262.144 byte.\n" >>"$f"
' || true

# ------------------------------------------------------------------ cây thật không đổi
AFTER="$(md5_real)"
if [ "$BEFORE" = "$AFTER" ]; then
    ok "md5 tài liệu thật KHÔNG đổi trước/sau tự phá (D2-113F)"
else
    no "md5 tài liệu thật ĐỔI — tự phá đã ghi vào cây thật"
fi

echo "# kt78-selfbreak: cases=$CASES vi_pham=$FAILS"
if [ "$FAILS" -eq 0 ]; then
    echo "# kt78-selfbreak: PASS (sáu đường phá đều bị bắt; cây thật không đổi)"
    exit 0
fi
echo "# kt78-selfbreak: FAIL"
exit 1
