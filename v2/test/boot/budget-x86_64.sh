#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-340a — cổng ngân sách kernel **x86_64** (D2-236, khuôn `budget-aarch64.sh` / KT-265).
#
# Cộng `.data + .sdata + .bss + .sbss` bằng `llvm-size -A` (đúng cách đo của K0.5/KT-07), trần
# **262.144 B** [TK] (QD §3.5, bất biến I7) — **không nới**. `.stack`/`.stack_guard` (nếu linker
# còn khai) và **`.smp_stacks`** (arena stack per-hart NOLOAD của KT-276) báo **RIÊNG**, không cộng
# vào trần: chúng là NOLOAD, không chiếm `filesz` của LOAD segment.
#
# Khớp tên theo **tiền tố** `^.(data|sdata|bss|sbss)` để không bỏ sót `.data.rel.ro`, `.sbss.*`…
# (một section writable mới mà bỏ sót = ngân sách rỗng, đúng lỗi cần chặn).
#
# **Khác `budget.sh` riscv64** (chỉ thêm, không bớt):
#   1. kiểm `e_machine` = **62** (x86-64) trước khi đo — trỏ nhầm ảnh riscv64/aarch64 là **từ
#      chối**, không đo (fail-closed);
#   2. báo `.smp_stacks` (NOLOAD per-hart) + `.user` (khối mã ring 3, không phải writable data)
#      riêng như `.stack`/`.stack_guard`;
#   3. `--selftest` có fixture **đúng hình dạng ảnh x86_64** (`.note.Xen`, `.user`, `.smp_stacks`)
#      + cổng kiến trúc đọc `e_machine` từ **ELF thật** (không chỉ hàm thuần) + ca âm/không-ELF.
#
# **KT-340b** (D2-243/D2-244) — thêm **hai** thứ, không nới ngưỡng nào:
#   4. báo **riêng** `.bootpt` (8 bảng trang boot + arena bảng trang động, NOLOAD) và
#      `.bootstacks` (4 stack IST1 + stack user, NOLOAD) — chúng đã ra khỏi `.bss` ở KT-340b;
#      vẫn là NOLOAD nên **không** cộng vào trần, nhưng phải hiện trong dòng `# image=` để người
#      đọc thấy chúng không biến mất khỏi phép đo;
#   **KT-341**: báo thêm `.corespace` (NOLOAD — `Machine` 184.792 B của `KT-340c`/`KT-341`) cùng
#   nhóm với `.bootpt`/`.bootstacks`; nó **không** được cộng vào `.data+.bss` nên trần 65.536 B
#   của ảnh sản phẩm vẫn đo đúng phần dữ liệu tĩnh còn lại.
#   5. **trần riêng của ảnh SẢN PHẨM: < 64 KiB (65.536 B)** — ràng buộc D2-243 cho `x86_64`
#      (`KT-340c` link `vifast-core::Machine` 184.792 B vào cùng `.data+.bss`; 96.320 B cũ +
#      184.792 B = 281.112 B > 262.144 B). Nhãn kết thúc bằng `-product` ⇒ áp trần này; ảnh
#      `ktest`/`gpu` vẫn chỉ chịu trần I7 **262.144 B**. Đây là **siết thêm**, không phải nới.
#
# Dùng: bash v2/test/boot/budget-x86_64.sh <ảnh-elf> [nhãn]
#       bash v2/test/boot/budget-x86_64.sh --selftest     # tự kiểm phép cộng + cổng trên fixture
set -u

BUDGET_BYTES=$((256 * 1024)) # 262.144 B [TK] — QD §3.5, D2-72 Q5
PRODUCT_BUDGET_BYTES=$((64 * 1024)) # 65.536 B — D2-243/D2-244 cho ảnh **sản phẩm** x86_64
EM_X86_64=62                 # ELF `e_machine` của x86-64 (ELF for the x86-64 architecture)

die() { echo "budget-x86_64: $*" >&2; exit 1; }

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

# `e_machine` của ELF (đọc 2 byte LE tại offset 0x12). Rỗng nếu tệp không phải ELF.
machine_of() {
    local f="$1" magic
    [ -f "$f" ] || return 0
    magic="$(od -An -c -j0 -N4 "$f" 2>/dev/null | tr -d ' ')"
    case "$magic" in
        177ELF*) ;;
        *) return 0 ;;
    esac
    od -An -tu2 -j18 -N2 "$f" 2>/dev/null | tr -d ' '
}

# `machine_ok <e_machine>`: 0 ⇔ đúng x86-64. Rỗng/khác ⇒ 1 (từ chối đo).
machine_ok() {
    [ "${1:-}" = "$EM_X86_64" ]
}

# Trần áp cho **nhãn** ảnh: ảnh **sản phẩm** (`*-product`) chịu trần riêng **65.536 B** của
# D2-243/D2-244; mọi ảnh khác (`ktest`, `gpu`, …) chỉ chịu trần I7 **262.144 B**. Hàm **thuần**
# để `--selftest` kiểm được (nhãn `ktest` **không** bị áp nhầm trần sản phẩm).
label_budget() {
    case "${1:-}" in
        *-product | product) echo "$PRODUCT_BUDGET_BYTES" ;;
        *) echo "$BUDGET_BYTES" ;;
    esac
}

selftest() {
    local fails=0 s

    # Fixture 1 — **hình dạng thật** của ảnh x86_64 (đo trên ảnh KT-340a trước khi tách):
    # `.text`/`.rodata`/`.note.Xen`/`.user` **không** cộng; `.data` 4408 + `.bss` 173032
    # = 177440; `.smp_stacks` NOLOAD 557056 báo riêng.
    local f1
    f1="$(cat <<'EOF'

section                size         addr
.text               135168    1048576
.rodata              10348    1183744
.note.Xen               20    1194092
.data                 4408    1196032
.user                   95    1204224
.bss                173032    1208320
.smp_stacks         557056    1384448
Total               880127
EOF
)"
    s="$(printf '%s' "$f1" | sum_data_bss)"
    if [ "$s" -eq 177440 ]; then
        echo "ok 1 - cộng .data+.bss của ảnh x86_64 = 177440 (dưới trần $BUDGET_BYTES)"
    else
        echo "not ok 1 - cộng .data+.bss: được $s, đòi 177440"
        fails=$((fails + 1))
    fi
    # `.smp_stacks` **không** được cộng (NOLOAD per-hart, báo riêng).
    s="$(printf '%s' "$f1" | awk '$1==".smp_stacks" {s+=$2} END {print s+0}')"
    if [ "$s" -eq 557056 ]; then
        echo "ok 2 - .smp_stacks báo riêng = 557056 (không cộng vào trần)"
    else
        echo "not ok 2 - .smp_stacks: được $s, đòi 557056"
        fails=$((fails + 1))
    fi
    # `.user` (khối mã ring 3, PROGBITS) cũng phải báo riêng — không nằm trong `.data/.bss`.
    s="$(printf '%s' "$f1" | awk '$1==".user" {s+=$2} END {print s+0}')"
    if [ "$s" -eq 95 ]; then
        echo "ok 3 - .user báo riêng = 95 (khối mã, không phải writable data)"
    else
        echo "not ok 3 - .user: được $s, đòi 95"
        fails=$((fails + 1))
    fi

    # Fixture 2 — tên biến thể `.data.rel.ro` + `.sdata` + `.sbss.*` phải được cộng: 8+24+32+64 = 128.
    local f2 s2
    f2="$(cat <<'EOF'

section                size         addr
.text                  1712    1048576
.rodata                 404    1183744
.data                     8    1196032
.data.rel.ro             24    1196040
.sdata                   32    1196064
.bss                      0    1196096
.sbss                    64    1196096
.sbss.extra               0    1196160
Total                 71800
EOF
)"
    s2="$(printf '%s' "$f2" | sum_data_bss)"
    if [ "$s2" -eq 128 ]; then
        echo "ok 4 - cộng tên biến thể (.data.rel.ro/.sdata/.sbss) = 128"
    else
        echo "not ok 4 - tên biến thể: được $s2, đòi 128"
        fails=$((fails + 1))
    fi

    # Fixture 3 — 300 KiB `.bss` phải VƯỢT trần (đèn đỏ của cổng hoạt động).
    local f3 s3
    f3="$(cat <<'EOF'

section                size         addr
.text                  1712    1048576
.rodata                 404    1183744
.bss                 307200    1196032
.smp_stacks         557056    1507328
Total               866372
EOF
)"
    s3="$(printf '%s' "$f3" | sum_data_bss)"
    if [ "$s3" -gt "$BUDGET_BYTES" ]; then
        echo "ok 5 - .bss 300 KiB ⇒ $s3 > $BUDGET_BYTES (cổng sẽ FAIL)"
    else
        echo "not ok 5 - .bss 300 KiB không bị coi là vượt: $s3"
        fails=$((fails + 1))
    fi

    # Fixture 4 — `.smp_stacks` khổng lồ **không** được kéo tổng vượt trần (NOLOAD không tính).
    local f4 s4
    f4="$(cat <<'EOF'

section                size         addr
.text                  1712    1048576
.data                     8    1196032
.bss                   1024    1196040
.smp_stacks        4194304    1197056
Total               4197048
EOF
)"
    s4="$(printf '%s' "$f4" | sum_data_bss)"
    if [ "$s4" -eq 1032 ] && [ "$s4" -le "$BUDGET_BYTES" ]; then
        echo "ok 6 - .smp_stacks 4 MiB không kéo tổng: data+bss=$s4 ≤ $BUDGET_BYTES"
    else
        echo "not ok 6 - .smp_stacks bị cộng nhầm: data+bss=$s4"
        fails=$((fails + 1))
    fi

    # Fixture 5 — cổng kiến trúc trên **ELF thật**: 62 (x86-64) nhận; 183 (AArch64), 243
    # (RISC-V), rỗng **từ chối**. Dựng header 20 byte bằng `printf` (e_machine tại offset 18).
    local tmp
    tmp="$(mktemp "${TMPDIR:-/tmp}/budget-x64-selftest.XXXXXX")" || die "không tạo được tệp tạm"
    if printf '\177ELF\002\001\001\000\000\000\000\000\000\000\000\000\001\000\076\000' >"$tmp" &&
        [ "$(machine_of "$tmp")" = "$EM_X86_64" ]; then
        echo "ok 7 - e_machine đọc từ ELF thật = 62 (x86-64) ⇒ nhận"
    else
        echo "not ok 7 - không đọc được e_machine=62 từ ELF thật (được '$(machine_of "$tmp")')"
        fails=$((fails + 1))
    fi
    if machine_ok "$EM_X86_64"; then
        echo "ok 8 - machine_ok(62) ⇒ nhận"
    else
        echo "not ok 8 - machine_ok(62) bị từ chối"
        fails=$((fails + 1))
    fi
    # 183 = AArch64, 243 = RISC-V tại cùng offset ⇒ phải bị **từ chối** (không đo nhầm arch).
    local bad=0 other
    for other in '\267\000' '\363\000'; do
        printf '\177ELF\002\001\001\000\000\000\000\000\000\000\000\000\001\000%b' "$other" >"$tmp"
        if machine_ok "$(machine_of "$tmp")"; then bad=1; fi
    done
    if machine_ok "" || machine_ok 183 || machine_ok 243 || [ "$bad" -ne 0 ]; then
        echo "not ok 9 - e_machine aarch64/riscv64/rỗng mà cổng kiến trúc vẫn nhận"
        fails=$((fails + 1))
    else
        echo "ok 9 - e_machine 183/243/rỗng ⇒ TỪ CHỐI (không đo nhầm ảnh arch khác)"
    fi

    # Fixture 6 — tệp **không phải ELF** phải bị từ chối (đường `od` thật, không chỉ hàm thuần).
    printf 'không phải ELF\n' >"$tmp"
    if bash "${BASH_SOURCE[0]}" "$tmp" selftest >/dev/null 2>&1; then
        echo "not ok 10 - tệp không phải ELF mà budget vẫn đo"
        fails=$((fails + 1))
    else
        echo "ok 10 - tệp không phải ELF ⇒ TỪ CHỐI"
    fi
    rm -f "$tmp" 2>/dev/null || true

    # Ca 11 — **trần riêng ảnh sản phẩm** (KT-340b/D2-243): nhãn `*-product` ⇒ 65.536 B; nhãn
    # `x86_64-ktest`/`x86_64-gpu` ⇒ 262.144 B. Đây là cổng **siết thêm**, không nới trần I7.
    local bp bk
    bp="$(label_budget x86_64-product)"
    bk="$(label_budget x86_64-ktest)"
    if [ "$bp" -eq 65536 ] && [ "$bk" -eq 262144 ]; then
        echo "ok 11 - trần ảnh sản phẩm=$bp < trần I7=$bk (nhãn ktest không bị áp nhầm)"
    else
        echo "not ok 11 - trần theo nhãn: product=$bp ktest=$bk (đòi 65536/262144)"
        fails=$((fails + 1))
    fi

    [ "$fails" -eq 0 ] || return 1
    echo "# budget-x86_64 --selftest: PASS"
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n 's/^# \{0,1\}//; 2,20p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,30p'
    echo "budget-x86_64.sh: dùng: budget-x86_64.sh <ảnh-elf> [nhãn] | budget-x86_64.sh --selftest | budget-x86_64.sh --help"
    exit 0
fi
if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi

IMAGE="${1:-}"
LABEL="${2:-$(basename "${IMAGE:-?}")}"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

[ -n "$IMAGE" ] || die "thiếu đường dẫn ảnh (dùng: budget-x86_64.sh <ảnh-elf> [nhãn])"
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

# Cổng kiến trúc **trước** khi đo: trần `.data+.bss` của ảnh này chỉ có nghĩa với ảnh x86_64.
MACHINE="$(machine_of "$IMAGE")"
machine_ok "$MACHINE" || die "ảnh '$IMAGE' không phải ELF x86_64 (e_machine=${MACHINE:-<không phải ELF>}, cần $EM_X86_64) — TỪ CHỐI ĐO"

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
# KT-276: arena stack **per-hart** (guard 4 KiB + stack 64 KiB × 8) là NOLOAD ⇒ báo **riêng**,
# không cộng trần `.data+.bss` (D2-72 Q5).
SMP_STACKS="$(printf '%s\n' "$SIZES" | awk '$1==".smp_stacks" {s+=$2} END {print s+0}')"
# Mã ring 3 (`.user`, PROGBITS) — không phải writable data, báo riêng để người đọc thấy đủ.
USER_SEC="$(printf '%s\n' "$SIZES" | awk '$1==".user" {s+=$2} END {print s+0}')"
# **KT-340b**: bảng trang boot + arena (NOLOAD) và stack cố định (NOLOAD) đã ra khỏi `.bss` —
# vẫn báo riêng (không cộng trần) để không ai tưởng chúng đã biến mất khỏi ảnh.
BOOTPT="$(printf '%s\n' "$SIZES" | awk '$1==".bootpt" {s+=$2} END {print s+0}')"
BOOTSTACKS="$(printf '%s\n' "$SIZES" | awk '$1==".bootstacks" {s+=$2} END {print s+0}')"
# **KT-341** — `vifast-core::Machine` (184.792 B) ra `.corespace` NOLOAD: báo **riêng** như
# `.bootpt`/`.bootstacks` (không cộng trần, nhưng phải hiện để không ai tưởng nó biến mất).
CORESPACE="$(printf '%s\n' "$SIZES" | awk '$1==".corespace" {s+=$2} END {print s+0}')"
NOLOAD=$((STACK + GUARD + SMP_STACKS + BOOTPT + BOOTSTACKS + CORESPACE))
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"

LABEL_BUDGET="$(label_budget "$LABEL")"
echo "# image=$LABEL path=$IMAGE file=$IMG_BYTES arch=x86_64(e_machine=$MACHINE) data+bss=$MB budget=$BUDGET_BYTES label_budget=$LABEL_BUDGET stack=$STACK guard=$GUARD smp_stacks=$SMP_STACKS bootpt=$BOOTPT bootstacks=$BOOTSTACKS corespace=$CORESPACE user=$USER_SEC noload=$NOLOAD"
echo "# sections-seen: $(printf '%s\n' "$SIZES" | list_seen)"
echo "# data+bss-sections: $(printf '%s\n' "$SIZES" | list_data_bss)"
if [ "$MB" -gt "$BUDGET_BYTES" ]; then
    echo "budget-x86_64: VƯỢT $MB > $BUDGET_BYTES byte — D2-72 Q5 cấm nới trần (cắt mảng tĩnh sang arena/RAM boot)" >&2
    exit 1
fi
# **KT-340b** — trần riêng ảnh **sản phẩm** (D2-243/D2-244): `< 64 KiB`, cổng chặn của KT-340c.
if [ "$MB" -ge "$LABEL_BUDGET" ]; then
    echo "budget-x86_64: VƯỢT TRẦN ẢNH SẢN PHẨM $MB ≥ $LABEL_BUDGET byte (nhãn $LABEL) — D2-243/D2-244: .data+.bss ảnh sản phẩm x86_64 phải < 64 KiB trước KT-340c" >&2
    exit 1
fi
exit 0
