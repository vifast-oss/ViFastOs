#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-347b** (D2-245/D2-252 Pha A) — phần **dùng chung** của 3 cổng ngân sách kernel
# (`v2/test/boot/{budget.sh,budget-aarch64.sh,budget-x86_64.sh}`).
#
# # Vì sao có tệp này
#
# Ba cổng ngân sách chép **cùng** logic: cộng `.data/.sdata/.bss/.sbss` từ `llvm-size -A`, đọc
# `e_machine` của ELF, phân nhánh **nhãn** (`*-product` < 65.536 B; `*-product-shell` ≤ 262.144 B;
# còn lại 262.144 B), cổng `.corespace ≥ 184.792 B` (D2-252) và in một dòng `# image=…`. Ba bản
# chép lệch nhau một chút (tên trường, cỡ `.corespace` chỉ có ở aarch64/x86_64) nên **cùng một
# ảnh** cho ba dòng khác nhau — người đọc phải nhớ mỗi arch in gì.
#
# Tệp này gom: **đo** (cộng section, đọc `e_machine`, đọc `.corespace`), **luật nhãn** (3 ngưỡng
# cứng, không nới), **in kết quả cùng một định dạng** cho 3 arch, và **parse** dòng `# image=…`
# (để ca kiểm/runner đọc lại đúng số). Runner vẫn giữ phần riêng: `e_machine` của arch, cờ
# `--product`/`--product-shell`, fixture `--selftest` đặc thù arch, thông điệp lỗi.
#
# # Ngưỡng — **giữ nguyên tuyệt đối** (D2-72 Q5, D2-243/D2-244, D2-250, D2-252)
#
#   budget_common_TOTAL          262.144 B  trần hiến định `.data+.bss` (mọi ảnh)
#   budget_common_PRODUCT         65.536 B  ảnh sản phẩm tối giản `*-product`: **nhỏ hơn** (strict)
#   budget_common_PRODUCT_SHELL  262.144 B  ảnh `*-product-shell`: **≤**
#   budget_common_MACHINE        172.424 B  `.corespace` NOLOAD phải ≥ `size_of::<Machine>()`
#
# Không có hàm nào nới ngưỡng; ba hằng số trên là nguồn duy nhất, runner không khai lại.
#
# # Định dạng kết quả (giống nhau cho cả 3 arch)
#
#   # image=<nhãn> path=<ảnh> file=<byte> arch=<arch>(e_machine=<n>) data+bss=<byte>
#     budget=<trần-I7> label_budget=<trần-theo-nhãn> stack=<n> guard=<n> smp_stacks=<n>
#     bootpt=<n> bootstacks=<n> corespace=<n> user=<n> trap_stack=<n> syscall_stack=<n>
#     hart_stacks=<n> noload=<n>
#
# Trường nào arch không có section thì in `0` (không bỏ trường) — cùng một khuôn để so ba arch.
#
# # Ghi chú
#
# Tệp **không** có trạng thái toàn cục, không `exit`, không `cd`; an toàn khi `source` dưới
# `set -u` (bash 3.2 của macOS). Mọi phép **đọc tệp ảnh** đều chờ có hạn (TOCTOU: `cargo` thay bin
# trong `target/release`, xem `harness-scan.sh`), hết hạn ⇒ fail-closed.
# Chạy trực tiếp chỉ để tự kiểm: `--selftest`.
set -u

_BUDGET_COMMON_HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Một nguồn chờ-tệp + quét marker: `harness-scan.sh` cùng thư mục (nguồn 23 tiền tố D2-08).
# shellcheck source=/dev/null
. "$_BUDGET_COMMON_HERE/harness-scan.sh"

# ---- ngưỡng cứng (nguồn duy nhất; KHÔNG nới) ----
budget_common_TOTAL=$((256 * 1024))          # 262.144 B [TK] — QD §3.5, D2-72 Q5
budget_common_PRODUCT=$((64 * 1024))         # 65.536 B — D2-243/D2-244 (ảnh sản phẩm tối giản)
budget_common_PRODUCT_SHELL=$((256 * 1024))  # 262.144 B — D2-250 (ảnh product-shell)
budget_common_MACHINE=172424                 # D2-84/D2-252 + KT-417b: size_of::<vifast_core::Machine>()
# `e_machine` của 3 arch (ELF header, offset 0x12, LE 2 byte) — dùng cho ca kiểm/runner.
budget_common_EM_X86_64=62
budget_common_EM_AARCH64=183
budget_common_EM_RISCV=243

# `budget_common_sum_data_bss` (stdin từ `llvm-size -A`): tổng `.data/.sdata/.bss/.sbss` và hậu tố
# (`.data.rel.ro`, `.sbss.*`…). Khớp theo **tiền tố** để không bỏ sót section writable mới.
budget_common_sum_data_bss() {
    awk '$1 ~ /^\.(data|sdata|bss|sbss)(\.|$)/ {s+=$2} END {print s+0}'
}

# Danh sách section **đã cộng** (kèm cỡ) — để người đọc thấy tổng không phải số rỗng.
budget_common_list_data_bss() {
    awk '$1 ~ /^\.(data|sdata|bss|sbss)(\.|$)/ {printf "%s=%d ", $1, $2}'
}

# Mọi section `llvm-size` thấy (bỏ 2 dòng tiêu đề, bỏ dòng `Total`).
budget_common_list_seen() {
    awk 'NR > 2 && $1 ~ /^\./ {printf "%s=%d ", $1, $2}'
}

# `budget_common_section <sizes> <tên>`: cỡ một section trong biến `$SIZES` (0 nếu vắng).
budget_common_section() {
    printf '%s\n' "${1:-}" | awk -v name="${2:-}" '$1==name {s+=$2} END {print s+0}'
}

# `budget_common_machine_of <ảnh>`: `e_machine` (2 byte LE tại 0x12); rỗng nếu không phải ELF.
budget_common_machine_of() {
    local f="${1:-}" magic
    harness_scan_wait_file "$f" || return 0
    magic="$(od -An -c -j0 -N4 "$f" 2>/dev/null | tr -d ' ')"
    case "$magic" in
        177ELF*) ;;
        *) return 0 ;;
    esac
    od -An -tu2 -j18 -N2 "$f" 2>/dev/null | tr -d ' '
}

# `budget_common_machine_ok <e_machine> <e_machine-mong-đợi>`: 0 ⇔ khớp (rỗng/khác ⇒ 1).
budget_common_machine_ok() {
    [ -n "${1:-}" ] && [ "${1:-}" = "${2:-<không>}" ]
}

# `budget_common_llvm_size`: in đường dẫn `llvm-size` của rustc hiện hành (rc 1 nếu không có).
budget_common_llvm_size() {
    command -v rustc >/dev/null 2>&1 || return 1
    local sysroot host
    sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
    [ -n "$sysroot" ] && [ -n "$host" ] || return 1
    [ -x "$sysroot/lib/rustlib/$host/bin/llvm-size" ] || return 1
    printf '%s' "$sysroot/lib/rustlib/$host/bin/llvm-size"
}

# `budget_common_corespace_bytes <ảnh>`: cỡ `.corespace` (NOLOAD chứa `Machine`), 0 nếu không có.
budget_common_corespace_bytes() {
    local image="${1:-}" llvm
    llvm="$(budget_common_llvm_size)" || {
        echo 0
        return 0
    }
    harness_scan_wait_file "$image" || {
        echo 0
        return 1
    }
    "$llvm" -A "$image" 2>/dev/null | awk '$1==".corespace" {s+=$2} END {print s+0}'
}

# `budget_common_label_budget <nhãn>`: trần áp cho nhãn.
budget_common_label_budget() {
    case "${1:-}" in
        *-product-shell | product-shell) echo "$budget_common_PRODUCT_SHELL" ;;
        *-product | product) echo "$budget_common_PRODUCT" ;;
        *) echo "$budget_common_TOTAL" ;;
    esac
}

# `budget_common_budget_ok <byte> <nhãn>`: 0 ⇔ trong trần của nhãn. `*-product` **nhỏ hơn**
# 65.536 (strict); `*-product-shell`/`ktest`/khác **≤** 262.144.
budget_common_budget_ok() {
    local bytes="${1:-0}" label="${2:-}" limit
    limit="$(budget_common_label_budget "$label")"
    case "$label" in
        *-product | product) [ "$bytes" -lt "$limit" ] ;;
        *) [ "$bytes" -le "$limit" ] ;;
    esac
}

# `budget_common_label_needs_marker_scan <nhãn>`: 0 ⇔ `*-product-shell` (nhãn duy nhất phải quét
# D2-08 0 marker ở cổng ngân sách; `*-product` có bộ quét riêng của runner; `ktest` **phải** có).
budget_common_label_needs_marker_scan() {
    case "${1:-}" in
        *-product-shell | product-shell) return 0 ;;
        *) return 1 ;;
    esac
}

# `budget_common_corespace_gate <corespace> [machine]`: 0 ⇔ arena NOLOAD chứa vừa `Machine`.
budget_common_corespace_gate() {
    local corespace="${1:-0}" machine="${2:-$budget_common_MACHINE}"
    [ "$corespace" -ge "$machine" ]
}

# `budget_common_print_line <arch> <e_machine> <nhãn> <ảnh> <data+bss> <noload>
#                          [stack] [guard] [smp_stacks] [bootpt] [bootstacks] [corespace]
#                          [user] [trap_stack] [syscall_stack] [hart_stacks]`
# In **một** dòng `# image=…` cùng khuôn cho 3 arch. `noload` do runner tính (giữ đúng công thức
# cũ của arch: x86_64 gồm `.corespace`, aarch64 thì không) — tệp này không đổi số của ai.
budget_common_print_line() {
    local arch="${1:-?}" machine="${2:-0}" label="${3:-?}" path="${4:-?}"
    local mb="${5:-0}" noload="${6:-0}"
    local stack="${7:-0}" guard="${8:-0}" smp="${9:-0}" bootpt="${10:-0}" bootstacks="${11:-0}"
    local corespace="${12:-0}" user="${13:-0}" trap="${14:-0}" syscall="${15:-0}" hart="${16:-0}"
    local file label_budget
    file=0
    if [ -f "$path" ]; then
        file="$(wc -c <"$path" 2>/dev/null | tr -d ' ' || true)"
    fi
    case "${file:-}" in '' | *[!0-9]*) file=0 ;; esac
    label_budget="$(budget_common_label_budget "$label")"
    printf '# image=%s path=%s file=%s arch=%s(e_machine=%s) data+bss=%s budget=%s label_budget=%s stack=%s guard=%s smp_stacks=%s bootpt=%s bootstacks=%s corespace=%s user=%s trap_stack=%s syscall_stack=%s hart_stacks=%s noload=%s\n' \
        "$label" "$path" "$file" "$arch" "$machine" "$mb" "$budget_common_TOTAL" "$label_budget" \
        "$stack" "$guard" "$smp" "$bootpt" "$bootstacks" "$corespace" "$user" \
        "$trap" "$syscall" "$hart" "$noload"
}

# `budget_common_field <dòng> <khoá>`: in giá trị `khoá=…` đầu tiên (rỗng nếu không có).
# Dùng `awk` để khoá ở **đầu dòng** (`image=…`) và khoá có `+` (`data+bss=…`) đều đọc đúng.
budget_common_field() {
    printf '%s\n' "${1:-}" | awk -v k="${2:-}" '{
        for (i = 1; i <= NF; i++) {
            if (index($i, k "=") == 1) {
                print substr($i, length(k) + 2)
                exit
            }
        }
    }'
}

# `budget_common_parse_line <dòng>`: đọc dòng `# image=… data+bss=… budget=…`; in
# `image=… data+bss=… budget=… corespace=…`; rc 1 nếu thiếu trường bắt buộc hoặc số không hợp lệ.
budget_common_parse_line() {
    local line="${1:-}" image mb budget corespace
    image="$(printf '%s\n' "$line" | sed -n 's/^# image=\([^[:space:]]*\).*/\1/p')"
    mb="$(budget_common_field "$line" data+bss)"
    budget="$(budget_common_field "$line" budget)"
    corespace="$(budget_common_field "$line" corespace)"
    [ -n "$image" ] && [ -n "$mb" ] && [ -n "$budget" ] || return 1
    case "$mb$budget${corespace:-0}" in
        *[!0-9]*) return 1 ;;
    esac
    printf 'image=%s data+bss=%s budget=%s corespace=%s\n' "$image" "$mb" "$budget" "${corespace:-0}"
}

# `budget_common_check_line <dòng> <nhãn-mong-đợi>`: 0 ⇔ dòng parse được, `image` đúng nhãn,
# `budget` đúng trần của nhãn và `data+bss` trong trần đó. Ca kiểm "nhãn sai ⇒ ĐỎ".
budget_common_check_line() {
    local line="${1:-}" want="${2:-}" parsed image mb budget corespace
    parsed="$(budget_common_parse_line "$line")" || return 1
    image="$(budget_common_field "$parsed" image)"
    mb="$(budget_common_field "$parsed" data+bss)"
    budget="$(budget_common_field "$parsed" budget)"
    corespace="$(budget_common_field "$parsed" corespace)"
    [ "$image" = "$want" ] || return 1
    [ "$budget" = "$(budget_common_label_budget "$want")" ] || return 1
    budget_common_budget_ok "$mb" "$want" || return 1
    # D2-252: nhãn product-shell phải có `.corespace ≥ 184.792` NOLOAD.
    if budget_common_label_needs_marker_scan "$want"; then
        budget_common_corespace_gate "${corespace:-0}" || return 1
    fi
    return 0
}

# ---------------------------------------------------------------- tự kiểm (không QEMU)
budget_common_selftest() {
    local fails=0 cases=0 s a b line parsed tmp
    _bc_ok() {
        cases=$((cases + 1))
        echo "ok $cases - $*"
    }
    _bc_bad() {
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - $*"
    }
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/budget-common-selftest.XXXXXX")" || {
        echo "budget-common: không tạo được thư mục tạm" >&2
        return 1
    }

    # (1) cộng section: `.data` 8 + `.data.rel.ro` 24 + `.sdata` 32 + `.bss` 0 + `.sbss` 64 +
    # `.sbss.extra` 0 = 128; `.text`/`.rodata`/`.smp_stacks` **không** cộng.
    s="$(printf '%s\n' '.text 1712' '.rodata 404' '.data 8' '.data.rel.ro 24' '.sdata 32' \
        '.bss 0' '.sbss 64' '.sbss.extra 0' '.smp_stacks 557056' | budget_common_sum_data_bss)"
    if [ "$s" -eq 128 ]; then
        _bc_ok "cộng .data+.bss (kể tên biến thể) = 128; NOLOAD không cộng"
    else
        _bc_bad "cộng .data+.bss = $s (đòi 128)"
    fi
    # (2) danh sách section đã cộng + danh sách đã thấy tách đúng.
    a="$(printf '%s\n' '.data 8' '.bss 1024' '.smp_stacks 4096' | budget_common_list_data_bss)"
    b="$(printf '%s\n' '' 'section size addr' '.text 1712' '.data 8' '.bss 1024' 'Total 2744' |
        budget_common_list_seen)"
    if [ "$a" = ".data=8 .bss=1024 " ] && [ "$b" = ".text=1712 .data=8 .bss=1024 " ]; then
        _bc_ok "list_data_bss/list_seen đúng (không kéo Total/NOLOAD)"
    else
        _bc_bad "list_data_bss='$a' list_seen='$b'"
    fi

    # (3) **ba ngưỡng cứng** — đúng hợp đồng D2-243/D2-250/D2-72 Q5.
    if [ "$(budget_common_label_budget x86_64-product)" -eq 65536 ] &&
        [ "$(budget_common_label_budget x86_64-product-shell)" -eq 262144 ] &&
        [ "$(budget_common_label_budget x86_64-ktest)" -eq 262144 ]; then
        _bc_ok "nhãn: product=65.536, product-shell=262.144, ktest=262.144"
    else
        _bc_bad "ngưỡng theo nhãn sai: product=$(budget_common_label_budget x86_64-product) product-shell=$(budget_common_label_budget x86_64-product-shell) ktest=$(budget_common_label_budget x86_64-ktest)"
    fi
    # (4) biên của hai kiểu so sánh: product **strict** <; product-shell **≤**.
    if budget_common_budget_ok 65535 x-product && ! budget_common_budget_ok 65536 x-product &&
        budget_common_budget_ok 262144 x-product-shell && ! budget_common_budget_ok 262145 x-product-shell; then
        _bc_ok "biên: product 65.535 XANH / 65.536 ĐỎ; product-shell 262.144 XANH / 262.145 ĐỎ"
    else
        _bc_bad "biên ngưỡng sai (product 65.536 phải ĐỎ, product-shell 262.144 phải XANH)"
    fi
    # (5) **nhãn sai ⇒ ĐỎ** (dùng `budget_common_check_line`, không chỉ hàm thuần).
    line="# image=x86_64-product-shell path=/tmp/a file=1 arch=x86_64(e_machine=62) data+bss=204800 budget=262144 label_budget=262144 stack=0 guard=0 smp_stacks=0 bootpt=0 bootstacks=0 corespace=184792 user=0 trap_stack=0 syscall_stack=0 hart_stacks=0 noload=184792"
    if budget_common_check_line "$line" x86_64-product-shell; then
        _bc_ok "parse + check: dòng product-shell 204.800 B, .corespace=184.792 ⇒ XANH"
    else
        _bc_bad "parse + check từ chối dòng product-shell hợp lệ"
    fi
    if ! budget_common_check_line "$line" x86_64-ktest; then
        _bc_ok "nhãn sai ⇒ ĐỎ (check_line đòi đúng nhãn)"
    else
        _bc_bad "nhãn sai mà check_line vẫn XANH"
    fi
    if ! budget_common_check_line "# image=x86_64-product-shell path=/tmp/a file=1 data+bss=204800 budget=262144 corespace=0" x86_64-product-shell; then
        _bc_ok "D2-252: .corespace=0 (Machine rơi vào .bss) ⇒ ĐỎ"
    else
        _bc_bad "D2-252: .corespace=0 mà check_line vẫn XANH"
    fi
    # (6) dòng hỏng (thiếu `budget=`) ⇒ parse **ĐỎ**, không đọc ra số rỗng.
    if ! budget_common_parse_line "# image=product path=/tmp/a file=1 data+bss=1024" >/dev/null 2>&1; then
        _bc_ok "dòng thiếu 'budget=' ⇒ parse TỪ CHỐI"
    else
        _bc_bad "dòng thiếu 'budget=' mà parse vẫn nhận"
    fi
    # (7) `.corespace` gate: 172.424 ⇒ nhận; 172.423/0 ⇒ từ chối (KT-417b: `Cap` 56→32 B).
    if budget_common_corespace_gate 172424 && ! budget_common_corespace_gate 172423 &&
        ! budget_common_corespace_gate 0; then
        _bc_ok ".corespace gate: ≥172.424 nhận; 172.423/0 từ chối"
    else
        _bc_bad ".corespace gate sai"
    fi
    # (8) `e_machine` đọc từ ELF thật (62) + tệp không phải ELF ⇒ rỗng.
    printf '\177ELF\002\001\001\000\000\000\000\000\000\000\000\000\001\000\076\000' >"$tmp/elf64"
    printf 'không phải ELF\n' >"$tmp/notelf"
    if [ "$(budget_common_machine_of "$tmp/elf64")" = "62" ] &&
        [ -z "$(budget_common_machine_of "$tmp/notelf")" ]; then
        _bc_ok "machine_of: ELF thật=62; tệp không ELF ⇒ rỗng (từ chối đo)"
    else
        _bc_bad "machine_of: elf='$(budget_common_machine_of "$tmp/elf64")' notelf='$(budget_common_machine_of "$tmp/notelf")'"
    fi
    # (9) **cùng một định dạng** cho 3 arch: đủ trường, `label_budget` đúng nhãn, `arch=` đúng.
    local rv a64 x64
    rv="$(budget_common_print_line riscv64 243 product /tmp/rv 223696 950272 65536 4096 0 0 0 0 0 16384 32768 831488)"
    a64="$(budget_common_print_line aarch64 183 aarch64-product-shell /tmp/a64 155872 561152 0 0 557056 0 0 184792 0 0 0 0)"
    x64="$(budget_common_print_line x86_64 62 x86_64-product-shell /tmp/x64 177440 811008 0 0 557056 262144 20480 184792 95 0 0 0)"
    if printf '%s\n' "$rv" | grep -q '^# image=product ' &&
        printf '%s\n' "$a64" | grep -q '^# image=aarch64-product-shell ' &&
        printf '%s\n' "$x64" | grep -q '^# image=x86_64-product-shell ' &&
        [ "$(budget_common_field "$rv" arch)" = "riscv64(e_machine=243)" ] &&
        [ "$(budget_common_field "$a64" label_budget)" = "262144" ] &&
        [ "$(budget_common_field "$x64" data+bss)" = "177440" ] &&
        [ "$(budget_common_field "$rv" corespace)" = "0" ] &&
        [ "$(budget_common_field "$x64" corespace)" = "184792" ]; then
        _bc_ok "3 arch in cùng khuôn: image/path/file/arch/data+bss/budget/label_budget/…/noload"
    else
        _bc_bad "định dạng 3 arch lệch: rv='$rv' a64='$a64' x64='$x64'"
    fi
    # (10) `budget_common_field` không khớp khoá con (`data` không lấy `data+bss`), không có ⇒ rỗng.
    if [ "$(budget_common_field '# image=p data+bss=12 budget=262144' data+bss)" = "12" ] &&
        [ -z "$(budget_common_field '# image=p data+bss=12' corespace)" ]; then
        _bc_ok "field: đọc đúng khoá, khoá vắng ⇒ rỗng"
    else
        _bc_bad "field đọc sai khoá"
    fi

    rm -rf "$tmp" 2>/dev/null || true
    echo
    if [ "$fails" -gt 0 ]; then
        echo "budget-common.sh --selftest: FAIL ($fails/$cases)" >&2
        return 1
    fi
    echo "budget-common.sh --selftest: PASS ($cases/$cases)"
    return 0
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selftest) budget_common_selftest ;;
        "" | -h | --help)
            echo "dùng: (source) v2/test/boot/lib/budget-common.sh; trực tiếp: --selftest"
            ;;
        *)
            echo "budget-common.sh: cờ lạ '$1' (chỉ --selftest)" >&2
            exit 2
            ;;
    esac
fi
