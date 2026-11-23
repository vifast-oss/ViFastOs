#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-265 — cổng ngân sách kernel **aarch64** K4' (khuôn `budget.sh` riscv64 / KT-44, D2-72 Q5).
#
# Cộng `.data + .sdata + .bss + .sbss` bằng `llvm-size -A` (đúng cách đo của K0.5/KT-07), trần
# **262.144 B** [TK] (QD §3.5, bất biến I7) — **không nới**. `.stack`/`.stack_guard` (nếu linker
# còn khai) và **`.smp_stacks`** (arena stack per-hart NOLOAD của KT-259) báo **RIÊNG**, không cộng
# vào trần: chúng là NOLOAD, không chiếm `filesz` của ảnh.
#
# Khớp tên theo **tiền tố** `^.(data|sdata|bss|sbss)` để không bỏ sót `.data.rel.ro`, `.sbss.*`…
# (một section writable mới mà bỏ sót = ngân sách rỗng, đúng lỗi cần chặn).
#
# **Khác `budget.sh` riscv64** (chỉ thêm, không bớt):
#   1. kiểm `e_machine` = **183** (AArch64) trước khi đo — trỏ nhầm ảnh riscv64/x86_64 là **từ
#      chối**, không đo (fail-closed);
#   2. báo `.smp_stacks` (NOLOAD per-hart) riêng như `.stack`/`.stack_guard`;
#   3. `--selftest` có fixture **đúng hình dạng ảnh aarch64** (`.text.vectors`, `.smp_stacks`).
#
# **KT-338c** (D2-08/D2-236) — trần **`64 KiB`** cho ảnh **sẢN PHẨM**: trước khi link
# `vifast-core::Machine` (`KT-338b`), ảnh sản phẩm aarch64 phải dưới **65.536 B** `.data+.bss`;
# ảnh **kiểm** giữ trần cũ `262.144 B` (D2-72 Q5). Cờ `--product` chọn trần chặt hơn — **không**
# nới ngưỡng nào, chỉ thêm một cổng **chặt hơn** cho ảnh sản phẩm.
#
# **KT-339a** (D2-250) — **hai nhãn** cho hai artefact aarch64:
#   - `aarch64-product` (ảnh tối giản, **không** `product-shell`): trần chặt **65.536 B** (`--product`);
#   - `aarch64-product-shell` (gắn `vifast-core::Machine` + `procsrv`/`shell`, 0 harness marker):
#     trần hiến định **262.144 B** + `Machine` **phải** nằm trong `.corespace` NOLOAD (thêm
#     `--product-shell`, hoặc nhãn chứa `product-shell`). Ngưỡng của nhãn **cũ không đổi**.
#
# Dùng: bash v2/test/boot/budget-aarch64.sh <ảnh-elf> [nhãn] [--product|--product-shell]
#       bash v2/test/boot/budget-aarch64.sh --selftest     # tự kiểm phép cộng + cổng trên fixture
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# **KT-347b** — đo/ngưỡng/nhãn/in kết quả ở **một** nguồn dùng chung 3 arch:
# `budget-common.sh` (kéo theo `harness-scan.sh` cho 23 tiền tố D2-08). Runner chỉ còn phần riêng
# của aarch64: `e_machine`=183, cờ `--product`/`--product-shell`, fixture `--selftest`.
# shellcheck source=/dev/null
. "$HERE/lib/budget-common.sh"

BUDGET_BYTES="$budget_common_TOTAL"  # 262.144 B [TK] — QD §3.5, D2-72 Q5 (ảnh kiểm; một nguồn)
# **KT-338c** — trần ảnh sản phẩm (D2-236/D2-244): `< 64 KiB` trước khi link `Machine`.
PRODUCT_BUDGET_BYTES="$budget_common_PRODUCT" # 65.536 B
# **KT-339a** (D2-250) — trần ảnh sản phẩm có shell: `KERNEL_STATIC_BUDGET` (D2-72 Q5), 256 KiB.
PRODUCT_SHELL_BUDGET_BYTES="$budget_common_PRODUCT_SHELL" # 262.144 B
# `size_of::<vifast_core::Machine>()` (D2-84/D2-252) — `.corespace` phải chứa **ít nhất** chừng
# này khi ảnh là `product-shell`: model **không** được rơi vào `.data+.bss` (một nguồn dùng chung).
MACHINE_BYTES="$budget_common_MACHINE"
# **D2-08** — 23 tiền tố harness nằm ở **một** nguồn `lib/harness-scan.sh` (trước KT-347b chép ở
# `run-aarch64.sh`/`budget-aarch64.sh`).
EM_AARCH64="$budget_common_EM_AARCH64" # 183 — ELF `e_machine` của AArch64 (arch-specific)

die() { echo "budget-aarch64: $*" >&2; exit 1; }

# `machine_ok <e_machine>`: ràng buộc **arch** của runner (AArch64 = 183) với phép so dùng chung.
machine_ok() { budget_common_machine_ok "${1:-}" "$EM_AARCH64"; }

selftest() {
    local fails=0 s

    # Fixture 1 — **hình dạng thật** của ảnh aarch64 (đo trên ảnh KT-262): `.text.vectors` là
    # TEXT (không cộng), `.smp_stacks` NOLOAD 544 KiB (không cộng) ⇒ tổng 208 + 155664 = 155872.
    local f1
    f1="$(cat <<'EOF'

section                size         addr
.text                37044   1074266112
.text.vectors         2072   1074305024
.rodata              11264   1074311168
.data                  208   1074323456
.bss                155664   1074327552
.smp_stacks         557056   1074487296
Total               763308
EOF
)"
    s="$(printf '%s' "$f1" | budget_common_sum_data_bss)"
    if [ "$s" -eq 155872 ]; then
        echo "ok 1 - cộng .data+.bss của ảnh aarch64 = 155872 (dưới trần $BUDGET_BYTES)"
    else
        echo "not ok 1 - cộng .data+.bss: được $s, đòi 155872"
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

    # Fixture 3 — tên biến thể `.data.rel.ro` + `.sdata` + `.sbss.*` phải được cộng: 8+24+32+64 = 128.
    local f3 s3
    f3="$(cat <<'EOF'

section                size         addr
.text                  1712   1074266112
.rodata                 404   1074305024
.data                     8   1074323456
.data.rel.ro             24   1074323464
.sdata                   32   1074323488
.bss                      0   1074323520
.sbss                    64   1074323520
.sbss.extra               0   1074323584
Total                 71800
EOF
)"
    s3="$(printf '%s' "$f3" | budget_common_sum_data_bss)"
    if [ "$s3" -eq 128 ]; then
        echo "ok 3 - cộng tên biến thể (.data.rel.ro/.sdata/.sbss) = 128"
    else
        echo "not ok 3 - tên biến thể: được $s3, đòi 128"
        fails=$((fails + 1))
    fi

    # Fixture 4 — 300 KiB `.bss` phải VƯỢT trần (đèn đỏ của cổng hoạt động).
    local f4 s4
    f4="$(cat <<'EOF'

section                size         addr
.text                  1712   1074266112
.rodata                 404   1074305024
.bss                 307200   1074323456
.smp_stacks         557056   1074487296
Total               866372
EOF
)"
    s4="$(printf '%s' "$f4" | budget_common_sum_data_bss)"
    if [ "$s4" -gt "$BUDGET_BYTES" ]; then
        echo "ok 4 - .bss 300 KiB ⇒ $s4 > $BUDGET_BYTES (cổng sẽ FAIL)"
    else
        echo "not ok 4 - .bss 300 KiB không bị coi là vượt: $s4"
        fails=$((fails + 1))
    fi

    # Fixture 5 — `.smp_stacks` khổng lồ **không** được kéo tổng vượt trần (NOLOAD không tính).
    local f5 s5
    f5="$(cat <<'EOF'

section                size         addr
.text                  1712   1074266112
.data                     8   1074323456
.bss                   1024   1074323464
.smp_stacks        4194304   1074487296
Total               4197048
EOF
)"
    s5="$(printf '%s' "$f5" | budget_common_sum_data_bss)"
    if [ "$s5" -eq 1032 ] && [ "$s5" -le "$BUDGET_BYTES" ]; then
        echo "ok 5 - .smp_stacks 4 MiB không kéo tổng: data+bss=$s5 ≤ $BUDGET_BYTES"
    else
        echo "not ok 5 - .smp_stacks bị cộng nhầm: data+bss=$s5"
        fails=$((fails + 1))
    fi

    # Fixture 6 — cổng kiến trúc: 183 (AArch64) nhận, 243 (RISC-V) và rỗng **từ chối**.
    if machine_ok "$EM_AARCH64"; then
        echo "ok 6 - e_machine=$EM_AARCH64 (AArch64) ⇒ nhận"
    else
        echo "not ok 6 - e_machine=$EM_AARCH64 bị từ chối"
        fails=$((fails + 1))
    fi
    if machine_ok 243 || machine_ok "" || machine_ok 62; then
        echo "not ok 7 - e_machine riscv64/x86_64/rỗng mà cổng kiến trúc vẫn nhận"
        fails=$((fails + 1))
    else
        echo "ok 7 - e_machine 243/62/rỗng ⇒ TỪ CHỐI (không đo nhầm ảnh arch khác)"
    fi

    # Fixture 8 — tệp **không phải ELF** phải bị từ chối (đường `od` thật, không chỉ hàm thuần).
    local tmp bad
    # Fixture 8 — **KT-338c**: trần **64 KiB** của ảnh sản phẩm. `.bss` 96 KiB phải VƯỢT trần
    # sản phẩm (đèn đỏ của cổng `--product` hoạt động), còn 26 KiB thì không.
    local s8a s8b
    s8a="$(printf '%s\n' '.data 232' '.bss 98304' | budget_common_sum_data_bss)"
    s8b="$(printf '%s\n' '.data 232' '.bss 26120' | budget_common_sum_data_bss)"
    if [ "$s8a" -gt "$PRODUCT_BUDGET_BYTES" ] && [ "$s8b" -lt "$PRODUCT_BUDGET_BYTES" ]; then
        echo "ok 8 - trần sản phẩm 64 KiB: .bss 96 KiB ⇒ $s8a > $PRODUCT_BUDGET_BYTES (ĐỎ), 26 KiB ⇒ $s8b (xanh)"
    else
        echo "not ok 8 - trần sản phẩm 64 KiB: 96 KiB ⇒ $s8a, 26 KiB ⇒ $s8b (cần vượt/không vượt)"
        fails=$((fails + 1))
    fi

    tmp="$(mktemp "${TMPDIR:-/tmp}/budget-a64-selftest.XXXXXX")" || die "không tạo được tệp tạm"
    printf 'không phải ELF\n' >"$tmp"
    if bash "${BASH_SOURCE[0]}" "$tmp" selftest >/dev/null 2>&1; then
        echo "not ok 9 - tệp không phải ELF mà budget vẫn đo"
        fails=$((fails + 1))
    else
        echo "ok 9 - tệp không phải ELF ⇒ TỪ CHỐI"
    fi
    rm -f "$tmp" 2>/dev/null || true

    # Fixture 10 (**KT-339a**) — **hai nhãn tách nhau**: .data+.bss 200 KiB (204.800 B) **vượt**
    # trần ảnh tối giản 65.536 nhưng **dưới** trần product-shell 262.144 ⇒ nhãn product-shell
    # XANH, nhãn product ĐỎ. Đây là phép tách nhãn của D2-250, không phải nới ngưỡng.
    local s10
    s10="$(printf '%s\n' '.data 360' '.bss 204440' | budget_common_sum_data_bss)"
    if [ "$s10" -le "$PRODUCT_SHELL_BUDGET_BYTES" ] && [ "$s10" -gt "$PRODUCT_BUDGET_BYTES" ]; then
        echo "ok 10 - tách nhãn: data+bss=$s10 ≤ $PRODUCT_SHELL_BUDGET_BYTES (product-shell) và > $PRODUCT_BUDGET_BYTES (product ĐỎ)"
    else
        echo "not ok 10 - tách nhãn: data+bss=$s10 (cần trong ($PRODUCT_BUDGET_BYTES, $PRODUCT_SHELL_BUDGET_BYTES])"
        fails=$((fails + 1))
    fi

    # Fixture 11 (**KT-339a**) — `Machine` **phải** ở `.corespace` NOLOAD: 184.792 ⇒ nhận,
    # 0 (model rơi vào `.bss`) ⇒ **từ chối** — đúng đột biến (i).
    if budget_common_corespace_gate "$MACHINE_BYTES" "$MACHINE_BYTES"; then
        echo "ok 11 - .corespace=$MACHINE_BYTES chứa vừa Machine $MACHINE_BYTES ⇒ nhận"
    else
        echo "not ok 11 - .corespace=$MACHINE_BYTES bị từ chối"
        fails=$((fails + 1))
    fi
    if budget_common_corespace_gate 0 "$MACHINE_BYTES"; then
        echo "not ok 12 - .corespace=0 (Machine trong .bss) mà cổng vẫn nhận"
        fails=$((fails + 1))
    else
        echo "ok 12 - .corespace=0 ⇒ TỪ CHỐI (Machine không được nằm trong .data+.bss)"
    fi

    # Fixture 13 (**KT-339a**) — bộ quét D2-08 **có răng**: tệp sạch ⇒ 0, tệp có 1 marker ⇒ > 0.
    local clean dirty hits_clean hits_dirty
    clean="$(mktemp "${TMPDIR:-/tmp}/budget-a64-clean.XXXXXX")" || die "không tạo được tệp tạm"
    dirty="$(mktemp "${TMPDIR:-/tmp}/budget-a64-dirty.XXXXXX")" || die "không tạo được tệp tạm"
    printf 'viFastOs product shell\nshell>\n# shell: exit code=0\n' >"$clean"
    printf 'viFastOs product shell\n# trap: slot=4\n' >"$dirty"
    hits_clean="$(harness_scan_count_strings "$clean")"
    hits_dirty="$(harness_scan_count_strings "$dirty")"
    if [ "$hits_clean" -eq 0 ] && [ "$hits_dirty" -gt 0 ]; then
        echo "ok 13 - quét D2-08: tệp sạch=0, tệp có marker=$hits_dirty ⇒ bộ quét có răng"
    else
        echo "not ok 13 - quét D2-08: sạch=$hits_clean (cần 0), có marker=$hits_dirty (cần > 0)"
        fails=$((fails + 1))
    fi
    rm -f "$clean" "$dirty" 2>/dev/null || true


    [ "$fails" -eq 0 ] || return 1
    echo "# budget-aarch64 --selftest: PASS"
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n 's/^# \{0,1\}//; 2,20p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,30p'
    echo "budget-aarch64.sh: dùng: budget-aarch64.sh <ảnh-elf> [nhãn] | budget-aarch64.sh --selftest | budget-aarch64.sh --help"
    exit 0
fi
if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi

# **KT-338c**: cờ `--product` (đặt ở đâu cũng được) chọn trần **64 KiB** của ảnh sản phẩm.
# **KT-339a**: `--product-shell` (hoặc nhãn chứa `product-shell`) chọn trần **256 KiB** + kiểm
# `.corespace` + 0 harness marker; hai nhãn **tách nhau**, ngưỡng cũ không đổi.
PRODUCT_MODE=0
PRODUCT_SHELL_MODE=0
ARGS=()
for arg in "$@"; do
    case "$arg" in
        --product) PRODUCT_MODE=1 ;;
        --product-shell) PRODUCT_SHELL_MODE=1 ;;
        *) ARGS+=("$arg") ;;
    esac
done
IMAGE="${ARGS[0]:-}"
LABEL="${ARGS[1]:-$(basename "${IMAGE:-?}")}"
case "$LABEL" in
    *product-shell*) PRODUCT_SHELL_MODE=1 ;;
esac
if [ "$PRODUCT_MODE" -eq 1 ]; then
    BUDGET_BYTES="$PRODUCT_BUDGET_BYTES"
fi
if [ "$PRODUCT_SHELL_MODE" -eq 1 ]; then
    BUDGET_BYTES="$PRODUCT_SHELL_BUDGET_BYTES"
fi

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

[ -n "$IMAGE" ] || die "thiếu đường dẫn ảnh (dùng: budget-aarch64.sh <ảnh-elf> [nhãn])"
# **KT-347c/KT-347b** — `cargo` thay bin trong `target/release` không nguyên tử ⇒ **chờ có hạn**
# (20 s) thay vì `[ -f ]` một lần rồi kết luận (TOCTOU); hết hạn ⇒ TỪ CHỐI ĐO.
harness_scan_wait_file "$IMAGE" || die "không thấy ảnh $IMAGE sau ${HARNESS_SCAN_WAIT_MS} ms — TỪ CHỐI ĐO"

# Cổng kiến trúc **trước** khi đo: trần `.data+.bss` của ảnh này chỉ có nghĩa với ảnh aarch64.
MACHINE="$(budget_common_machine_of "$IMAGE")"
machine_ok "$MACHINE" || die "ảnh '$IMAGE' không phải ELF AArch64 (e_machine=${MACHINE:-<không phải ELF>}, cần $EM_AARCH64) — TỪ CHỐI ĐO"

LLVM_SIZE="$(budget_common_llvm_size)" || die "KHÔNG đo được (thiếu llvm-size của rustc)"

SIZES="$("$LLVM_SIZE" -A "$IMAGE" 2>/dev/null)" || die "llvm-size lỗi trên $IMAGE"
[ -n "$SIZES" ] || die "llvm-size không trả section nào cho $IMAGE"

MB="$(printf '%s\n' "$SIZES" | budget_common_sum_data_bss)"
STACK="$(printf '%s\n' "$SIZES" | awk '$1==".stack" {s+=$2} END {print s+0}')"
GUARD="$(printf '%s\n' "$SIZES" | awk '$1==".stack_guard" {s+=$2} END {print s+0}')"
# KT-259: arena stack **per-hart** (guard 4 KiB + stack 64 KiB × 8) là NOLOAD ⇒ báo **riêng**,
# không cộng trần `.data+.bss` (D2-72 Q5).
SMP_STACKS="$(printf '%s\n' "$SIZES" | awk '$1==".smp_stacks" {s+=$2} END {print s+0}')"
NOLOAD=$((STACK + GUARD + SMP_STACKS))
# **KT-339a** — arena NOLOAD chứa `vifast_core::Machine` (báo **riêng** như `.bootpt` x86_64).
CORESPACE="$(printf '%s\n' "$SIZES" | awk '$1==".corespace" {s+=$2} END {print s+0}')"
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"

# **KT-347b** — cùng **một** định dạng kết quả cho 3 arch (`budget-common.sh`).
budget_common_print_line aarch64 "$MACHINE" "$LABEL" "$IMAGE" "$MB" "$NOLOAD" \
    "$STACK" "$GUARD" "$SMP_STACKS" 0 0 "$CORESPACE" 0 0 0 0
echo "# sections-seen: $(printf '%s\n' "$SIZES" | budget_common_list_seen)"
echo "# data+bss-sections: $(printf '%s\n' "$SIZES" | budget_common_list_data_bss)"
if [ "$MB" -gt "$BUDGET_BYTES" ]; then
    if [ "$PRODUCT_MODE" -eq 1 ]; then
        echo "budget-aarch64: ảnh SẢN PHẨM VƯỢT $MB > $BUDGET_BYTES byte (trần 64 KiB, D2-236/D2-244) — cắt mảng tĩnh sang arena/RAM boot" >&2
    else
        echo "budget-aarch64: VƯỢT $MB > $BUDGET_BYTES byte — D2-72 Q5 cấm nới trần (cắt mảng tĩnh sang arena/RAM boot)" >&2
    fi
    exit 1
fi
# **KT-339a** — nhãn `product-shell`: model **phải** trong `.corespace` NOLOAD, không trong
# `.data+.bss` (D2-250: đó là toàn bộ lý do trần 256 KiB đủ). Đột biến `mut-machine-bss` rơi vào
# đây (`.corespace=0`, `.data+.bss` + `size_of::<Machine>()`).
if [ "$PRODUCT_SHELL_MODE" -eq 1 ] && ! budget_common_corespace_gate "$CORESPACE" "$MACHINE_BYTES"; then
    echo "budget-aarch64: ảnh SẢN PHẨM-SHELL có .corespace=$CORESPACE < $MACHINE_BYTES ⇒ Machine KHÔNG ở arena NOLOAD (nằm trong .data+.bss=$MB) — D2-250 cấm; cắt model về .corespace, KHÔNG nới trần" >&2
    exit 1
fi
# **KT-339a** — nhãn `product-shell`: D2-08, **0 harness marker** (bộ quét 23 tiền tố + ca âm ở selftest).
if [ "$PRODUCT_SHELL_MODE" -eq 1 ]; then
    HITS="$(harness_scan_count_strings "$IMAGE")"
    if [ "$HITS" -ne 0 ]; then
        echo "budget-aarch64: ảnh SẢN PHẨM-SHELL còn $HITS harness marker (D2-08 yêu cầu 0) — ĐỎ" >&2
        exit 1
    fi
    echo "# d2-08: 0 harness marker (23 tiền tố)"
fi
exit 0
