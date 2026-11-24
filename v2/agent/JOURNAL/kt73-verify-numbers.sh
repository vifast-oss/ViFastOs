#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-73 — **D2-112**: kiểm số theo **từng khẳng định có phạm vi log riêng**.
#
# Vì sao viết lại (reviewer phá bản trước bằng 4 đường):
#   1. hoán số giữa hai ảnh (`ktest-proc 206736 → 202832`) — bản cũ nối **haystack chung** nên số
#      chỉ cần xuất hiện "ở đâu đó" ⇒ PASS;
#   2. bảng ngoại lệ chứa chữ số (`1 2 … 21`) ⇒ mọi số một chữ số đổi được;
#   3. log mutation/break-test nằm trong nguồn ⇒ giá trị "cố tình sai" thành nguồn hợp lệ;
#   4. `STATE.md` chỉ quét khối đầu ⇒ số ngoài khối không bị quét.
#
# Luật hiện tại (D2-112(a)(b)(c)(d)):
#   (a) **Phạm vi theo khẳng định**: mỗi dòng có **số đo** phải nằm trong một khối
#       ```` ```text scope=<FILE> ```` và dòng đó phải **trích nguyên văn** từ `<FILE>` (một tệp,
#       không nối nhiều tệp). Không có "haystack chung" nào để hoán số.
#   (b) **Không ngoại lệ chứa số**: ngoài khối, tài liệu chỉ được có **định danh** — theo **lớp**
#       (hex `0x…`, số hiệu spec dạng `x.y[.z]`, mã task `KT-n`, ngày `YYYY-MM-DD`, tên chip
#       `16550`). Mỗi lớp có lý do ngay trong script; **không** có danh sách số được miễn.
#   (c) **Nguồn**: chỉ log của chính task, **loại** `*red*` (ca âm/đỏ trước), `*mutation*`,
#       `*verify*` (log của script này) và `*break*`. Hai log KT-72 chỉ dùng làm **giá trị tham
#       chiếu** cho câu về sha (khai báo riêng, không phải nguồn số đo).
#   (d) **STATE**: quét **toàn bộ** tệp trừ các mục lịch sử của task khác (`## KT-xx` không phải
#       KT-73) — lý do: số của task khác thuộc log của task đó (D2-109 theo từng task); quét
#       chúng ở đây sẽ buộc phải dùng haystack chung, đúng thứ D2-112(a) cấm. Khối trạng thái và
#       **mọi** mục KT-73 đều bị quét.
#
# Tự phá: `kt73-verify-selfbreak.sh` (5 đường) — xem `kt73-verify-selfbreak.log`.
#
# Dùng: bash v2/agent/JOURNAL/kt73-verify-numbers.sh
set -u

# Gốc repo (hoặc **bản copy** khi tự phá): `KT73_DOCS_ROOT=/tmp/copy` — nhờ đó script tự phá
# **không bao giờ** ghi vào cây thật đang review.
ROOT="${KT73_DOCS_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
J="$ROOT/v2/agent/JOURNAL"
tmp_doc="$(mktemp "${TMPDIR:-/tmp}/kt73-doc.XXXXXX")" || exit 2
WORK="$(mktemp -d "${TMPDIR:-/tmp}/kt73-v.XXXXXX")" || exit 2
trap 'rm -rf "$tmp_doc" "$WORK"' EXIT

# ---------------------------------------------------------------- (c) nguồn log
SOURCES=()
for f in "$J"/kt73-*.log "$J"/kt73-qemu-*.log "$J"/kt73-con-chardev.out; do
    [ -f "$f" ] || continue
    case "$(basename "$f")" in
        *red* | *mutation* | *verify* | *break*) continue ;;
    esac
    SOURCES+=("$f")
done
[ "${#SOURCES[@]}" -gt 0 ] || {
    echo "kt73-verify: không có nguồn log nào" >&2
    exit 2
}
# Tệp tham chiếu **chỉ** cho câu về sha (không phải nguồn số đo của task khác).
REF_SHA="$J/kt72-product-sha.log"

# ---------------------------------------------------------------- (b) **định danh** (không phải số đo)
# D2-112(b): không có "danh sách số được miễn". Chỉ có **lớp cú pháp** là định danh, mỗi lớp một
# lý do; trước khi tìm số, các lớp này bị **gọt** khỏi dòng, nên số còn lại **bắt buộc** nằm trong
# khối trích nguyên văn.
strip_identifiers() {
    sed -E \
        -e 's/^#+ [0-9]+(\.[0-9]+)*\.?//' \
        -e 's/^[[:space:]]*[0-9]+\. //' \
        -e 's/[Ll][0-9]+//g' \
        -e 's/tap[0-9]+//g' \
        -e 's/KT-[0-9]+[a-z]?(\/[0-9]+)*(\.[.][0-9]+)*//g' \
        -e 's/[0-9][0-9a-f]{7,}//g' \
        -e 's/[Dd]2\.\.[A-Za-z0-9]+//g' \
        -e 's/[Dd]2-[0-9]+[A-Za-z]?//g' \
        -e 's/kt[0-9]+//g' \
        -e 's/[Kk][0-9]+//g' \
        -e 's/sha[0-9]+//g' \
        -e 's/vser[0-9.]+//g' \
        -e 's/ch[0-9]+//g' \
        -e 's/_VERSION_[0-9]+//g' \
        -e 's/Agent [0-9]+//g' \
        -e 's/FNV-1a//g' \
        -e 's/TAP [0-9]+//g' \
        -e 's/KT[0-9]+-[A-Z0-9-]+//g' \
        -e 's/v[0-9]+//g' \
        -e 's/[0-9]{4}-[0-9]{2}-[0-9]{2}//g' \
        -e 's/[A-Za-z0-9_./-]+:[0-9]+(-[0-9]+)?//g' \
        -e 's/§[0-9]+(\.[0-9]+)*//g' \
        -e 's/0x[0-9a-fA-F_]+//g' \
        -e 's/16550A?//g' \
        -e 's/8250//g' \
        -e 's/UART[0-9]+//g'
}
# Lý do từng lớp (ghi để người đọc kiểm):
#   KT-n[a]     : mã task (kèm dải `KT-7x/7y`, `KT-7x..7y`)
#   hex >= 8    : giá trị băm / định danh dài in trong tài liệu (số đo ở đây ngắn hơn nhiều)
#   D2-n        : mã quyết định
#   ktN / vN    : tiền tố tên log/tệp và tên dòng sản phẩm
#   Kn          : tên hạng mục K6; shaN : tên thuật toán băm; vserN.N : tên thiết bị QEMU
#   Agent n     : tên vai trò (Agent 2)
#   Ln / tapN   : mức test (L0/L1) và số hiệu TAP (định danh ca kiểm, không phải số đo)
#   "## n."     : số hiệu mục markdown
#   "n. " đầu dòng: số thứ tự mục danh sách
#   YYYY-MM-DD  : ngày
#   file:line   : tham chiếu dòng trong mã nguồn (không phải số đo)
#   §x.y[.z]    : số hiệu spec/mục
#   0x…         : định danh thiết bị/địa chỉ (spec hoặc log)
#   16550[Ａ]    : tên chip UART

# ---------------------------------------------------------------- trích tài liệu
# Mỗi tài liệu: hàm trích nội dung vào $tmp_doc. STATE: bỏ mục lịch sử không phải KT-73.
extract() {
    local label="$1"
    case "$label" in
        k6-console.md) cat "$ROOT/v2/docs/k6-console.md" ;;
        k3b-core.md-21) awk '/^## 21\. KT-73/{f=1} f' "$ROOT/v2/docs/k3b-core.md" ;;
        STATE.md)
            awk '
                /^## KT-/ { hist = ($0 !~ /KT-73/) }
                hist { next }
                { print }
            ' "$ROOT/v2/agent/STATE.md"
            ;;
        kt73-journal.md) cat "$J/kt73-journal.md" ;;
        *) return 1 ;;
    esac
}

# ---------------------------------------------------------------- kiểm một tài liệu
fail=0
total_claims=0
total_tokens=0
total_verbatim=0
echo "# kt73-verify (D2-112): phạm vi log theo TỪNG khẳng định — nguồn ${#SOURCES[@]} tệp, gốc $ROOT"
for label in k6-console.md k3b-core.md-21 STATE.md kt73-journal.md; do
    extract "$label" >"$tmp_doc" || {
        echo "not ok - [$label] không trích được"
        fail=$((fail + 1))
        continue
    }
    if [ ! -s "$tmp_doc" ]; then
        echo "not ok - [$label] trích ra 0 byte"
        fail=$((fail + 1))
        continue
    fi
    doc_fail=0
    doc_claims=0
    doc_verbatim=0
    scope=""
    lineno=0
    while IFS= read -r line; do
        lineno=$((lineno + 1))
        case "$line" in
            '```text scope='*)
                scope="${line#\`\`\`text scope=}"
                scope="${scope% \`\`\`}"
                scope="${scope%% *}"
                if [ ! -f "$J/$scope" ] && [ ! -f "$ROOT/$scope" ] && [ ! -f "$scope" ]; then
                    echo "not ok - [$label] dòng $lineno: scope không tồn tại [$scope]"
                    doc_fail=$((doc_fail + 1))
                fi
                # (c) phạm vi phải là **nguồn hợp lệ** — không được là log đỏ/mutation/break/verify.
                scope_ok=0
                for src in "${SOURCES[@]}"; do
                    [ "$(basename "$src")" = "$scope" ] && scope_ok=1 && break
                done
                if [ "$scope_ok" -eq 0 ]; then
                    echo "not ok - [$label] dòng $lineno: scope [$scope] không phải nguồn hợp lệ"
                    doc_fail=$((doc_fail + 1))
                fi
                continue
                ;;
            '```'*)
                scope=""
                continue
                ;;
        esac
        # Số trong dòng (bỏ `_` để khớp cách log in `0x4020_0000` → `0x40200000`).
        nums="$(printf '%s\n' "$line" | strip_identifiers | sed -e 's/_//g' |
            grep -oE '[0-9][0-9a-fx]*' | sort -u)"
        [ -n "$nums" ] || continue
        if [ -n "$scope" ]; then
            # (a) Khối có phạm vi: dòng phải **nguyên văn** trong đúng tệp đó.
            total_verbatim=$((total_verbatim + 1))
            doc_verbatim=$((doc_verbatim + 1))
            if ! grep -Fqx -- "$line" "$J/$scope" 2>/dev/null && ! grep -Fqx -- "$line" "$ROOT/$scope" 2>/dev/null; then
                echo "not ok - [$label] dòng $lineno: không nguyên văn trong [$scope]"
                doc_fail=$((doc_fail + 1))
            fi
            total_tokens=$((total_tokens + 1))
            continue
        fi
        # Ngoài khối: chỉ định danh theo lớp; số đo ⇒ đỏ.
        total_claims=$((total_claims + 1))
        doc_claims=$((doc_claims + 1))
        bad=""
        for token in $nums; do
            total_tokens=$((total_tokens + 1))
            bad="$bad $token"
        done
        if [ -n "$bad" ]; then
            echo "not ok - [$label] dòng $lineno: số ngoài khối trích (cần đưa vào \`\`\`text scope=…):$bad"
            doc_fail=$((doc_fail + 1))
        fi
    done <"$tmp_doc"
    if [ "$doc_fail" -eq 0 ]; then
        echo "ok - [$label] nguyên_văn=$doc_verbatim dòng_ngoài_khối=$doc_claims thiếu=0"
    else
        echo "not ok - [$label] nguyên_văn=$doc_verbatim dòng_ngoài_khối=$doc_claims thiếu=$doc_fail"
        fail=$((fail + 1))
    fi
done

# Câu về sha: bắt buộc có **giá trị so sánh** từ log KT-72 (không phải nguồn số đo của task).
sha_expect="$(grep -oE '[0-9a-f]{64}' "$REF_SHA" 2>/dev/null | head -1)"
sha16="${sha_expect:0:16}"
if [ -z "$sha16" ]; then
    echo "not ok - thiếu giá trị sha tham chiếu trong $REF_SHA"
    fail=$((fail + 1))
elif ! grep -rqF -- "$sha16" "$ROOT/v2/agent/STATE.md" "$ROOT/v2/docs/k6-console.md" 2>/dev/null; then
    echo "not ok - không thấy giá trị sha KT-72 ($sha16) trong tài liệu"
    fail=$((fail + 1))
else
    echo "ok - sha tham chiếu KT-72 có mặt trong tài liệu ($sha16)"
fi

echo "# kt73-verify: docs=4 khối_nguyên_văn=$total_verbatim dòng_ngoài_khối=$total_claims token=$total_tokens fail=$fail"
[ "$fail" -eq 0 ] || {
    echo "# kt73-verify: FAIL"
    exit 1
}
echo "# kt73-verify: PASS"
exit 0
