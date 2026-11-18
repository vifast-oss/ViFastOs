#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-77b** (OD-K6-2 tiêu chí 6) — kiểm **bằng máy** rằng ảnh sản phẩm **không chứa mã harness**,
# và rằng máy kiểm này **bắt được** ảnh có harness (ca âm).
#
# Vì sao quét chuỗi chứ không quét symbol: ảnh canonical của D2-122.2 được build bằng
# `-C strip=symbols` (không còn `.symtab`), nên detector phải chạy được trên **đúng tệp sẽ boot**.
# Mọi marker dưới đây là chuỗi **do mã test phát ra** (TAP/màn hình/báo lỗi), nằm trong `.rodata`
# của ảnh test và **không** nằm trong ảnh sản phẩm:
#   ktest_        tên module `src/ktest_*.rs` (đường dẫn panic-location cũng nhúng)
#   proc_demo     module harness của ảnh `ktest-proc` (`src/proc_demo.rs`)
#   "# ktest halt" dòng kết thúc do harness in
#   "not ok"      dạng TAP đỏ — harness là nơi duy nhất dùng; ảnh sản phẩm chỉ in `ok 1 - k3 boot`
#   "# core: link" bằng chứng core của ảnh test (sản phẩm in `# core: product`)
#   ktest-<x>     tên ảnh test trong thông điệp
#
# **`probe` (D2-123(1))** — không dùng literal `probe` trần (PLIC in sẵn
# `plic: probe khong khoi phuc priority irq` ⇒ dương tính giả). Kiểm hai lớp:
#   (a) chuỗi **đủ điều kiện** `vifast_plat_riscv64::probe`, `probe::on_trap`,
#       `probe_fault_entry`, `src/probe.rs` — chỉ hiện khi ảnh còn symtab (ảnh canonical đã strip
#       nên thường 0);
#   (b) **feature-graph** (luôn kiểm được trên ảnh stripped): `cargo tree -e features` của crate
#       arch phải **không** có `ktest` cho cấu hình sản phẩm (thiếu `ktest` ⇒ không biên dịch
#       `probe`, `trap_dispatch` không có nhánh probe), và **phải** có `ktest` cho cấu hình ảnh
#       test (ca âm của bộ đếm này).
#
# Luật (OD-K6-5 luật 3): mọi bộ đếm báo 0 **phải** có ca âm làm nó khác 0 — vì vậy script **bắt
# buộc** một `--negative <ảnh-ktest>` và tự đỏ nếu ca âm không bắt được marker nào.
#
# Dùng: bash v2/agent/JOURNAL/kt77b-harness-scan.sh \
#           --image artifacts/<sha>.bin --negative <ảnh có harness>
# Exit: 0 = sản phẩm sạch **và** ca âm bị bắt; 1 = vi phạm; 2 = lỗi dùng/thiếu tệp.
set -u

# KT-129: `cargo tree` (lớp feature-graph) cần `cargo` trong PATH. Thiếu PATH ⇒ `feat_tree` trả
# rỗng ⇒ `ktest=0 umode=0` ⇒ ca 3/4 ĐỎ **oan** (đã xảy ra khi chạy từ shell không có ~/.cargo/bin).
# Các script khác của repo đều tự đặt PATH; script này trước đây không.
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
IMG=""
NEG=""

usage() {
    sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

die() {
    echo "harness-scan: $*" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --image) IMG="${2:-}"; shift 2 ;;
        --negative) NEG="${2:-}"; shift 2 ;;
        -h | --help) usage; exit 0 ;;
        *) die "tham số lạ: $1" ;;
    esac
done
[ -n "$IMG" ] || die "cần --image <ảnh sản phẩm>"
[ -n "$NEG" ] || die "cần --negative <ảnh có harness> (OD-K6-5 luật 3: bộ đếm 0 phải có ca âm)"
[ -f "$IMG" ] || die "không thấy ảnh $IMG"
[ -f "$NEG" ] || die "không thấy ảnh ca âm $NEG"

# Marker = chuỗi **cố định** (grep -F) do mã test phát ra. Không dùng regex để không phải lo ký tự.
MARKERS=(
    'ktest_'
    'proc_demo'
    '# ktest halt'
    'not ok'
    '# core: link'
    'ktest-ktest'
    'ktest-proc'
    'ktest-user'
    'ktest-con'
    'ktest-exec'
    'ktest-guard'
    # Lớp (a): chuỗi đủ điều kiện của `probe` (symbol/đường dẫn module) — xem đầu tệp.
    'vifast_plat_riscv64::probe'
    'probe::on_trap'
    'probe_fault_entry'
    'src/probe.rs'
)

sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }
size_of() { wc -c <"$1" | tr -d ' '; }

# Đếm **số dòng** chứa marker (grep -a: coi tệp là text, ảnh là binary nhưng marker ASCII).
count_marker() { # $1=tệp, $2=marker
    LC_ALL=C grep -a -F -c -- "$2" "$1" 2>/dev/null || true
}

scan() { # $1=tệp → in mỗi marker một dòng "# marker <text> hits=<n>"; trả tổng hits qua biến SCAN_HITS
    local f="$1" m n total=0
    for m in "${MARKERS[@]}"; do
        n="$(count_marker "$f" "$m")"
        case "$n" in '' | *[!0-9]*) n=0 ;; esac
        [ "$n" -gt 0 ] && echo "# marker '$m' hits=$n"
        total=$((total + n))
    done
    SCAN_HITS="$total"
}

fails=0
echo "# kt77b-harness-scan: image=$IMG sha256=$(sha_of "$IMG") bytes=$(size_of "$IMG")"
echo "# kt77b-harness-scan: markers=${#MARKERS[@]} (chuỗi do mã test phát ra; ảnh canonical đã strip)"
scan "$IMG"
prod_hits="$SCAN_HITS"
echo "# scan product: hits=$prod_hits"
if [ "$prod_hits" -eq 0 ]; then
    echo "ok 1 - anh san pham khong chua ma harness (hits=0)"
else
    echo "not ok 1 - anh san pham CHUA ma harness (hits=$prod_hits)"
    fails=$((fails + 1))
fi

echo "# kt77b-harness-scan: negative=$NEG sha256=$(sha_of "$NEG") bytes=$(size_of "$NEG")"
scan "$NEG"
neg_hits="$SCAN_HITS"
echo "# scan negative: hits=$neg_hits"
if [ "$neg_hits" -gt 0 ]; then
    echo "ok 2 - ca am: anh co harness bi bat (hits=$neg_hits > 0)"
else
    echo "not ok 2 - ca am: may kiem KHONG bat duoc anh co harness (hits=0)"
    fails=$((fails + 1))
fi

# ---------------------------------------------------------------- lớp (b): feature-graph `probe`
FEAT_PRODUCT="${FEAT_PRODUCT:-product-shell}"
FEAT_HARNESS="${FEAT_HARNESS:-ktest,user}"
feat_tree() { # $1 = danh sách feature của crate kbin
    (cd "$ROOT/v2/kernel" && RUSTUP_TOOLCHAIN="${VIFAST_REPRO_TOOLCHAIN:-1.99.1}" \
        cargo tree -p vifast-kbin-riscv64 --features "$1" --target riscv64gc-unknown-none-elf \
        -e features -i vifast-plat-riscv64 --locked --offline 2>/dev/null) || true
}
feat_count() { # $1 = feature; trả số dòng `vifast-plat-riscv64 feature "<x>"`
    LC_ALL=C grep -a -F -c "vifast-plat-riscv64 feature \"$1\"" "$2" 2>/dev/null || true
}

echo "# kt77b-harness-scan: feature-graph product=$FEAT_PRODUCT harness=$FEAT_HARNESS (crate vifast-plat-riscv64)"
PT="$(mktemp "${TMPDIR:-/tmp}/kt77b-feat-p.XXXXXX")"
HT="$(mktemp "${TMPDIR:-/tmp}/kt77b-feat-h.XXXXXX")"
feat_tree "$FEAT_PRODUCT" >"$PT"
feat_tree "$FEAT_HARNESS" >"$HT"
p_ktest="$(feat_count ktest "$PT")"
p_umode="$(feat_count umode "$PT")"
h_ktest="$(feat_count ktest "$HT")"
case "$p_ktest" in '' | *[!0-9]*) p_ktest=0 ;; esac
case "$p_umode" in '' | *[!0-9]*) p_umode=0 ;; esac
case "$h_ktest" in '' | *[!0-9]*) h_ktest=0 ;; esac
echo "# feat product: ktest=$p_ktest umode=$p_umode"
echo "# feat harness: ktest=$h_ktest"
if [ "$p_ktest" -eq 0 ] && [ "$p_umode" -gt 0 ]; then
    echo "ok 3 - probe gate: cau hinh san pham KHONG bat 'ktest' (probe khong duoc bien dich), co 'umode'"
else
    echo "not ok 3 - probe gate: cau hinh san pham bat 'ktest' (probe vao anh) hoac thieu 'umode' (ktest=$p_ktest umode=$p_umode)"
    fails=$((fails + 1))
fi
if [ "$h_ktest" -gt 0 ]; then
    echo "ok 4 - ca am cua probe gate: cau hinh anh test BAT 'ktest' (ktest=$h_ktest > 0)"
else
    echo "not ok 4 - ca am cua probe gate: cau hinh anh test KHONG bat 'ktest' (bo dem khong the khac 0)"
    fails=$((fails + 1))
fi
rm -f "$PT" "$HT" 2>/dev/null

if [ "$fails" -eq 0 ]; then
    echo "# kt77b-harness-scan: KET_LUAN=ok (product=0, negative>0, probe_gate=ok)"
    exit 0
fi
echo "# kt77b-harness-scan: KET_LUAN=FAIL (fails=$fails)"
exit 1
