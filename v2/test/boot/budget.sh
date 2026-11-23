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

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# **KT-347b** — đo/ngưỡng/in kết quả ở **một** nguồn dùng chung 3 arch (`lib/budget-common.sh`,
# kéo theo `lib/harness-scan.sh`). Runner chỉ còn phần riêng của riscv64: các section NOLOAD
# per-hart của arch này và fixture `--selftest`.
# shellcheck source=/dev/null
. "$HERE/lib/budget-common.sh"

BUDGET_BYTES="$budget_common_TOTAL" # 262.144 B [TK] — QD §3.5, D2-72 Q5 (một nguồn)

die() { echo "budget: $*" >&2; exit 1; }

# **KT-347b** — phép kiểm số học/ngưỡng nằm ở **một** nguồn `lib/budget-common.sh` (cùng ca âm:
# tên biến thể, `.bss` 300 KiB vượt trần, biên `product` 65.535/65.536, `.corespace`, parse dòng).
selftest() {
    budget_common_selftest
}

if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi

IMAGE="${1:-}"
LABEL="${2:-$(basename "${IMAGE:-?}")}"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

[ -n "$IMAGE" ] || die "thiếu đường dẫn ảnh (dùng: budget.sh <ảnh-elf> [nhãn])"
# **KT-347c/KT-347b** — `cargo` thay bin trong `target/release` không nguyên tử ⇒ **chờ có hạn**
# (20 s) thay vì `[ -f ]` một lần rồi kết luận (TOCTOU); hết hạn ⇒ TỪ CHỐI ĐO.
harness_scan_wait_file "$IMAGE" || die "không thấy ảnh $IMAGE sau ${HARNESS_SCAN_WAIT_MS} ms — TỪ CHỐI ĐO"

LLVM_SIZE="$(budget_common_llvm_size)" || die "KHÔNG đo được (thiếu llvm-size của rustc)"

SIZES="$("$LLVM_SIZE" -A "$IMAGE" 2>/dev/null)" || die "llvm-size lỗi trên $IMAGE"
[ -n "$SIZES" ] || die "llvm-size không trả section nào cho $IMAGE"

MB="$(printf '%s\n' "$SIZES" | budget_common_sum_data_bss)"
STACK="$(printf '%s\n' "$SIZES" | awk '$1==".stack" {s+=$2} END {print s+0}')"
GUARD="$(printf '%s\n' "$SIZES" | awk '$1==".stack_guard" {s+=$2} END {print s+0}')"
TRAP_STACK="$(printf '%s\n' "$SIZES" | awk '$1==".trap_stack" {s+=$2} END {print s+0}')"
# KT-66: stack dispatch syscall per-hart (32 KiB) là NOLOAD ⇒ **báo riêng**, không cộng trần
# `.data+.bss` (D2-72 Q5); `.hart_stacks` là 7 record còn lại của vùng NOLOAD per-hart.
SYSCALL_STACK="$(printf '%s\n' "$SIZES" | awk '$1==".syscall_stack" {s+=$2} END {print s+0}')"
HART_STACKS="$(printf '%s\n' "$SIZES" | awk '$1==".hart_stacks" {s+=$2} END {print s+0}')"
NOLOAD_HART=$((STACK + GUARD + TRAP_STACK + SYSCALL_STACK + HART_STACKS))
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"

# **KT-347b** — cùng **một** định dạng kết quả cho 3 arch (`budget-common.sh`); trường riêng của
# riscv64 (`trap_stack`/`syscall_stack`/`hart_stacks`) in `0` ở arch khác.
budget_common_print_line riscv64 "$budget_common_EM_RISCV" "$LABEL" "$IMAGE" "$MB" "$NOLOAD_HART" \
    "$STACK" "$GUARD" 0 0 0 0 0 "$TRAP_STACK" "$SYSCALL_STACK" "$HART_STACKS"
echo "# sections-seen: $(printf '%s\n' "$SIZES" | budget_common_list_seen)"
echo "# data+bss-sections: $(printf '%s\n' "$SIZES" | budget_common_list_data_bss)"
if [ "$MB" -gt "$BUDGET_BYTES" ]; then
    echo "budget: VƯỢT $MB > $BUDGET_BYTES byte — D2-72 Q5 cấm nới trần (cắt mảng tĩnh sang arena/RAM boot)" >&2
    exit 1
fi
exit 0
