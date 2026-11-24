#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-78 — **D2-112**: kiểm số trong tài liệu của task này.
#
# Vì sao cần (D2-112, vá nhiều lần): số đo **không** được viết tay; mỗi số phải trích **nguyên văn**
# từ **một** tệp log **đã ở trong repo**; ngoài khối trích chỉ được có **định danh**, không có số đo.
#
# Luật của script này:
#   (a) **Phạm vi theo khẳng định**: mỗi khối ```` ```text scope=<tên-tệp> ```` chỉ được trích từ
#       **một** tệp `v2/agent/JOURNAL/<tên-tệp>`, và **từng dòng** trong khối phải xuất hiện
#       nguyên văn trong tệp đó (so khớp cả dòng). Không có "haystack chung".
#   (b) **Không số ngoài khối**: trong phần KT-78 của tài liệu được kiểm, ngoài khối `scope=` chỉ
#       được có **định danh trong dấu `backtick`**; một **chữ số trần** ngoài backtick = ĐỎ.
#       (Lý do: nếu cho phép số một chữ số hay số thập phân thì mọi số đo đổi được tuỳ ý — D2-112b.)
#   (c) **Nguồn**: log tên chứa `selfbreak`/`verify` **không** được là nguồn.
#
# Dùng:
#   bash v2/agent/JOURNAL/kt78-verify-numbers.sh [ROOT]
#     ROOT mặc định là gốc repo; tự phá (`kt78-verify-selfbreak.sh`) truyền **bản copy** (D2-113F).
# Exit: 0 = PASS; 1 = có vi phạm; 2 = lỗi môi trường.
set -u

ROOT="${1:-${KT78_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}}"
J="$ROOT/v2/agent/JOURNAL"
[ -d "$J" ] || {
    echo "kt78-verify: không thấy $J" >&2
    exit 2
}

WORK="$(mktemp -d "${TMPDIR:-/tmp}/kt78-verify.XXXXXX")" || exit 2
trap 'rm -rf "$WORK"' EXIT

FAILS=0
BLOCKS=0
BLOCK_LINES=0
PROSE_LINES=0
bad() {
    echo "# kt78-verify: ĐỎ — $*"
    FAILS=$((FAILS + 1))
}

# Phần KT-78 của một tài liệu: `file:start:end`.
#   - `k3b-core.md`: từ dòng `## 25. KT-78` tới dòng `## ` kế tiếp.
#   - `STATE.md`: từ dòng `## KT-78` tới dòng `## ` kế tiếp.
#   - tệp khác: toàn bộ.
section_of() {
    local f="$1" start=1 end=0
    end="$(wc -l <"$f" | tr -d ' ')"
    case "$f" in
        */k3b-core.md)
            start="$(grep -n '^## 25\. KT-78' "$f" | head -1 | cut -d: -f1)"
            ;;
        */STATE.md)
            start="$(grep -n '^## KT-78' "$f" | head -1 | cut -d: -f1)"
            ;;
        *) start=1 ;;
    esac
    [ -n "${start:-}" ] || return 1
    if [ "$start" -gt 1 ]; then
        local next
        next="$(awk -v s="$start" 'NR > s && /^## / { print NR; exit }' "$f")"
        [ -n "$next" ] && end=$((next - 1))
    fi
    echo "$start:$end"
}

# Tách khối `scope=` ra tệp, trả về danh sách "tệp-log|dòng-đầu|số-dòng|tệp-khối".
extract_blocks() { # $1=tài liệu $2=start $3=end $4=nhãn
    local f="$1" s="$2" e="$3" tag="$4" ln=0 inb=0 scope="" blk="" first=0 n=0
    awk -v s="$s" -v e="$e" 'NR >= s && NR <= e { print NR ":" $0 }' "$f" |
        while IFS= read -r rec; do
            ln="${rec%%:*}"
            line="${rec#*:}"
            if [ "$inb" = 0 ]; then
                case "$line" in
                    '```text scope='*)
                        scope="${line#\`\`\`text scope=}"
                        scope="${scope%% *}"
                        scope="${scope%\`\`\`}"
                        inb=1
                        first="$ln"
                        n=0
                        blk="$WORK/$tag-$ln.txt"
                        : >"$blk"
                        ;;
                esac
            else
                case "$line" in
                    '```'*) echo "$scope|$first|$n|$blk" >>"$WORK/blocks.txt"; inb=0 ;;
                    *) printf '%s\n' "$line" >>"$blk"; n=$((n + 1)) ;;
                esac
            fi
        done
}

# (b) số trần ngoài khối (bỏ nội dung trong backtick trước khi tìm chữ số)
check_prose() { # $1=tài liệu $2=start $3=end $4=nhãn
    local f="$1" s="$2" e="$3" tag="$4" out="$WORK/prose-$tag.txt"
    awk -v s="$s" -v e="$e" 'NR >= s && NR <= e { print NR "\t" $0 }' "$f" >"$out"
    # đánh dấu dòng nằm trong khối scope (không kiểm prose cho các dòng đó)
    awk -v s="$s" -v e="$e" '
        function strip(l) {
            gsub(/`[^`]*`/, "", l)             # code span (định danh)
            gsub(/D2-[0-9]+/, "", l)           # mã quyết định
            gsub(/OD-K[0-9]+-[0-9]+/, "", l)
            gsub(/KT-[0-9]+[ab]?/, "", l)
            gsub(/ST-[0-9]+/, "", l)
            gsub(/A1-[0-9]+/, "", l)
            gsub(/[KNSRM][0-9]+[ab]?/, "", l)  # K6, N7, R1, S1, MR4
            gsub(/P[0-9]+/, "", l)             # P20
            gsub(/§[0-9]+(\.[0-9]+)*/, "", l)   # số hiệu tiết
            gsub(/[0-9a-zA-Z_.\/-]+\.(sh|md|log|rs|toml|py|txt):[0-9]+(-[0-9]+)?/, "", l)
            gsub(/[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/, "", l)
            gsub(/v[0-9]+(\.[0-9]+)*/, "", l)       # v2, v2.0
            gsub(/Agent[ \t]+[0-9]+/, "", l)
            gsub(/0x[0-9a-fA-F]+/, "", l)
            gsub(/\]\([^)]*\.(md|sh|log)\)/, "]", l)  # đích link markdown
            gsub(/^#+[ \t]+[0-9]+(\.[0-9]+)*\.[ \t]+/, "", l)
            gsub(/^[ \t]*[0-9]+\. /, "", l)  # số thứ tự danh sách
            return l
        }
        NR >= s && NR <= e {
            line = $0
            if (inb == 1) {                          # đang trong khối `text scope=`
                if (line ~ /^```/) { inb = 0 }
                print NR "\tBLOCK"
                next
            }
            if (inc == 1) {                          # đang trong fence **không** scope= ⇒ kiểm như prose
                if (line ~ /^```/) { inc = 0; print NR "\tBLOCK"; next }
                line = strip(line)
                if (line ~ /[0-9]/) print NR "\t" $0
                next
            }
            if (line ~ /^```text scope=/) {
                inb = 1
                print NR "\tBLOCK"
                next
            }
            # **Chỉ** khối `text scope=` được miễn kiểm chữ số. Mọi fence khác (kể cả ```text) là
            # **prose**: số phải nằm trong backtick, nếu không là ĐỎ — bịt đường phá của reviewer
            # (họ chèn fence ```text chứa `boot_done=999 isolated_0=888` và verifier cũ vẫn PASS).
            if (line ~ /^```/) {
                inc = 1
                print NR "\tBLOCK"
                next
            }
            line = strip(line)
            if (line ~ /[0-9]/) print NR "\t" $0
        }' "$f" >"$WORK/prose-bad.txt"
    while IFS="$(printf '\t')" read -r ln txt; do
        [ -n "${ln:-}" ] || continue
        [ "${txt:-}" = BLOCK ] && continue # đánh dấu dòng nằm trong khối, không phải vi phạm
        PROSE_LINES=$((PROSE_LINES + 1))
        bad "$f:$ln có chữ số **ngoài** khối scope (D2-112b): $txt"
    done <"$WORK/prose-bad.txt"
}

DOCS=""
add_doc() {
    [ -f "$1" ] || {
        bad "không thấy tài liệu $1"
        return 0
    }
    DOCS="$DOCS$1
"
}

# Tài liệu KT-78 (chỉ tệp **của task này**).
add_doc "$ROOT/v2/docs/k6-harness.md"
add_doc "$ROOT/v2/docs/k3b-core.md"
add_doc "$ROOT/v2/agent/STATE.md"
# `kt78-journal.md` **không** nằm trong phạm vi: nó là nhật ký (log tường thuật), cùng quy ước
# KT-76 — số đo trong nhật ký vẫn lấy từ khối `scope=` nhưng ngày/hash là định danh.

: >"$WORK/blocks.txt"
while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in
        */k3b-core.md) grep -q '^## 25\. KT-78' "$f" || {
            bad "$f: thiếu mục '## 25. KT-78'"
            continue
        } ;;
        */STATE.md) grep -q '^## KT-78' "$f" || {
            bad "$f: thiếu mục '## KT-78'"
            continue
        } ;;
    esac
    se="$(section_of "$f")" || {
        bad "$f: không xác định được phần KT-78"
        continue
    }
    s="${se%%:*}"
    e="${se##*:}"
    tag="$(basename "$f")"
    extract_blocks "$f" "$s" "$e" "$tag"
    check_prose "$f" "$s" "$e" "$tag"
done <<EOF
$DOCS
EOF

# (a) từng dòng trong khối phải có **nguyên văn** trong đúng một log
while IFS='|' read -r scope first n blk; do
    [ -n "${scope:-}" ] || continue
    BLOCKS=$((BLOCKS + 1))
    case "$scope" in
        *selfbreak* | *verify*) bad "khối ở tài liệu dùng nguồn bị cấm: $scope" ;;
    esac
    log="$J/$scope"
    if [ ! -f "$log" ]; then
        bad "khối scope=$scope: không có log $log"
        continue
    fi
    miss=""
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        BLOCK_LINES=$((BLOCK_LINES + 1))
        grep -Fqx -- "$line" "$log" || miss="$miss [$line]"
    done <"$blk"
    [ -z "$miss" ] || bad "khối scope=$scope không trích nguyên văn từ $scope:$miss"
done <"$WORK/blocks.txt"

# Khối trạng thái đầu `STATE.md`: **không** được chứa số đo (số đo chỉ ở mục KT-78 có `scope=`).
if [ -f "$ROOT/v2/agent/STATE.md" ]; then
    awk '
        function strip(l) {
            gsub(/`[^`]*`/, "", l); gsub(/D2-[0-9]+/, "", l); gsub(/OD-K[0-9]+-[0-9]+/, "", l)
            gsub(/KT-[0-9]+[ab]?/, "", l); gsub(/ST-[0-9]+/, "", l); gsub(/A1-[0-9]+/, "", l)
            gsub(/[KNSRM][0-9]+[ab]?/, "", l); gsub(/P[0-9]+/, "", l)
            gsub(/§[0-9]+(\.[0-9]+)*/, "", l); gsub(/0x[0-9a-fA-F]+/, "", l)
            return l
        }
        NR == 1 && $0 ~ /^```/ { inb = 1; next }
        inb == 1 && $0 ~ /^```/ { exit }
        inb == 1 && strip($0) ~ /[0-9]/ { print NR "\t" $0 }
    ' "$ROOT/v2/agent/STATE.md" >"$WORK/state-head-bad.txt"
    while IFS="$(printf '\t')" read -r ln txt; do
        [ -n "${ln:-}" ] || continue
        bad "STATE.md:$ln (khối trạng thái đầu) có số đo ngoài khối scope: $txt"
    done <"$WORK/state-head-bad.txt"
fi

echo "# kt78-verify: docs=$(printf '%s' "$DOCS" | grep -c . ) blocks=$BLOCKS block_lines=$BLOCK_LINES prose_violations=$PROSE_LINES"
if [ "$FAILS" -eq 0 ]; then
    echo "# kt78-verify: PASS"
    exit 0
fi
echo "# kt78-verify: FAIL ($FAILS vi phạm)"
exit 1
