#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-44 — cổng ngân sách kernel riscv64 K3 (D2-72 Q5).
#
# Cộng `.data + .sdata + .bss + .sbss` bằng `llvm-size -A` (đúng cách đo của K0.5/KT-07),
# trần **262.144 B** cho **cả hai** ảnh (sản phẩm và ktest), `.stack`/`.stack_guard` báo RIÊNG
# vì NOLOAD. In `# image=… data+bss=… stack=…`, in **mọi section llvm-size thấy** và đúng những
# section đã cộng vào tổng (không giấu chỗ khớp), rồi **fail** (exit 1) nếu vượt trần — không nới.
#
# Khớp tên theo **tiền tố** `^.(data|sdata|bss|sbss)` để không bỏ sót `.data.rel.ro`, `.sbss.*`…
# (một section writable mới mà bỏ sót = ngân sách rỗng, đúng lỗi cần chặn).
#
# Dùng: bash v2/test/boot/budget.sh <ảnh-elf> [nhãn]
#       bash v2/test/boot/budget.sh --selftest     # tự kiểm phép cộng trên fixture
set -u

BUDGET_BYTES=$((256 * 1024)) # 262.144 B [TK] — QD §3.5, D2-72 Q5

die() { echo "budget: $*" >&2; exit 1; }

# Cộng `.data/.sdata/.bss/.sbss` (và hậu tố `.data.rel.ro`, `.sbss.*`…) từ đầu ra `llvm-size -A`.
sum_data_bss() {
    awk '$1 ~ /^\.(data|sdata|bss|sbss)(\.|$)/ {s+=$2} END {print s+0}'
}

# Danh sách section đã cộng, kèm cỡ — để người đọc thấy tổng không phải số rỗng.
list_data_bss() {
    awk '$1 ~ /^\.(data|sdata|bss|sbss)(\.|$)/ {printf "%s=%d ", $1, $2}'
}

# Mọi section llvm-size thấy (bỏ 2 dòng tiêu đề, bỏ dòng Total).
list_seen() {
    awk 'NR > 2 && $1 ~ /^\./ {printf "%s=%d ", $1, $2}'
}

selftest() {
    local fails=0
    # Fixture 1: không có `.data`/`.sbss`; `.bss` 1.024 B ⇒ tổng 1.032 (dưới trần).
    local f1
    f1="$(cat <<'EOF'

section                size         addr
.text                  1712   2149580800
.rodata                 404   2149584896
.data                     8   2149588992
.bss                   1024   2149588992
.stack_guard           4096   2149588992
.stack                65536   2149593088
Total                 71780
EOF
)"
    local s1
    s1="$(printf '%s' "$f1" | sum_data_bss)"
    if [ "$s1" -eq 1032 ]; then
        echo "ok 1 - cộng .data+.bss = 1032"
    else
        echo "not ok 1 - cộng .data+.bss: được $s1, đòi 1032"
        fails=$((fails + 1))
    fi
    # Fixture 2: tên biến thể `.data.rel.ro` + `.sdata` + `.sbss.*` phải được cộng;
    # tổng 8+24+32+64 = 128 (không được bỏ sót 4 section này).
    local f2 s2
    f2="$(cat <<'EOF'

section                size         addr
.text                  1712   2149580800
.rodata                 404   2149584896
.data                     8   2149588992
.data.rel.ro             24   2149589000
.sdata                   32   2149589024
.bss                      0   2149589056
.sbss                    64   2149589056
.sbss.extra               0   2149589120
Total                 71800
EOF
)"
    s2="$(printf '%s' "$f2" | sum_data_bss)"
    if [ "$s2" -eq 128 ]; then
        echo "ok 2 - cộng tên biến thể (.data.rel.ro/.sdata/.sbss) = 128"
    else
        echo "not ok 2 - tên biến thể: được $s2, đòi 128"
        fails=$((fails + 1))
    fi
    # Fixture 3: 300 KiB `.bss` phải VƯỢT trần (đèn đỏ của cổng hoạt động).
    local f3 s3
    f3="$(cat <<'EOF'

section                size         addr
.text                  1712   2149580800
.rodata                 404   2149584896
.bss                 307200   2149588992
.stack                65536   2149593088
Total                375052
EOF
)"
    s3="$(printf '%s' "$f3" | sum_data_bss)"
    if [ "$s3" -gt "$BUDGET_BYTES" ]; then
        echo "ok 3 - .bss 300 KiB ⇒ $s3 > $BUDGET_BYTES (cổng sẽ FAIL)"
    else
        echo "not ok 3 - .bss 300 KiB không bị coi là vượt: $s3"
        fails=$((fails + 1))
    fi
    [ "$fails" -eq 0 ] || return 1
    echo "# budget --selftest: PASS"
}

if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi

IMAGE="${1:-}"
LABEL="${2:-$(basename "${IMAGE:-?}")}"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

[ -n "$IMAGE" ] || die "thiếu đường dẫn ảnh (dùng: budget.sh <ảnh-elf> [nhãn])"
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

command -v rustc >/dev/null 2>&1 || die "không có rustc (không tìm được llvm-size)"
SYSROOT="$(rustc --print sysroot 2>/dev/null || true)"
HOST="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
LLVM_SIZE="$SYSROOT/lib/rustlib/$HOST/bin/llvm-size"
[ -x "$LLVM_SIZE" ] || die "KHÔNG đo được (thiếu llvm-size ở $LLVM_SIZE)"

SIZES="$("$LLVM_SIZE" -A "$IMAGE" 2>/dev/null)" || die "llvm-size lỗi trên $IMAGE"
[ -n "$SIZES" ] || die "llvm-size không trả section nào cho $IMAGE"

MB="$(printf '%s\n' "$SIZES" | sum_data_bss)"
STACK="$(printf '%s\n' "$SIZES" | awk '$1==".stack" {s+=$2} END {print s+0}')"
GUARD="$(printf '%s\n' "$SIZES" | awk '$1==".stack_guard" {s+=$2} END {print s+0}')"
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"

echo "# image=$LABEL path=$IMAGE file=$IMG_BYTES data+bss=$MB budget=$BUDGET_BYTES stack=$STACK guard=$GUARD"
echo "# sections-seen: $(printf '%s\n' "$SIZES" | list_seen)"
echo "# data+bss-sections: $(printf '%s\n' "$SIZES" | list_data_bss)"
if [ "$MB" -gt "$BUDGET_BYTES" ]; then
    echo "budget: VƯỢT $MB > $BUDGET_BYTES byte — D2-72 Q5 cấm nới trần (cắt mảng tĩnh sang arena/RAM boot)" >&2
    exit 1
fi
exit 0
