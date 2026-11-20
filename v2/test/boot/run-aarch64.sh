#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-250 — runner boot **aarch64** (K4' bước 3, B3.8 phần đầu). Khuôn: `run.sh` (riscv64) +
# `qemu_guard.sh` (KT-78: process group + timeout + dọn con).
#
# Việc runner làm, theo thứ tự:
#   1. **build** ảnh `vifast-kbin-aarch64` cho `aarch64-unknown-none` (release);
#   2. **kiểm ELF** (T-objdump của thiết kế): `.text` nạp đúng `0x4008_0000`; `.text.vectors` là
#      `TEXT` **không** phải `BSS` (bất biến của B3.2); 16 stub `__vec_i` nằm đúng
#      `__vector_base + i*0x80`;
#   3. **boot** QEMU `virt` N lần qua `qemu_guard.sh`, kiểm **TAP** (`TAP version 13`, `1..N`,
#      đúng N dòng `ok`, không `not ok`, không `PANIC`) + marker **đúng một lần**;
#   4. in `ok`/`not ok` cho từng kiểm, exit 0 ⇔ mọi kiểm xanh.
#
# Dùng: bash v2/test/boot/run-aarch64.sh [NBOOT]
#
# **Ca âm chạy MẶC ĐỊNH** (không cần cờ): sau lần boot đầu, runner chạy lại **cùng** bộ kiểm
# `check_log` trên log tốt với 5 kỳ vọng **sai** (marker sai, thiếu một `ok`, có `not ok`, plan
# sai, log rỗng) và đòi **cả 5 phải ĐỎ**. Nhờ vậy mỗi lần chạy runner vừa chứng minh ảnh boot
# sạch, vừa chứng minh bộ kiểm **không phải dấu cao su** — không có đường "PASS giả".
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs aarch64: boot ok`)
#   EXPECT_TESTS    số ca TAP (mặc định 7 — KT-253 thêm 2 ca nửa cao/physmap)
#   EXPECT_TEXT_VMA VMA của `.text` (mặc định 0x40080000)
#   KT250_LOGDIR    thư mục log (mặc định $TMPDIR/k4-aarch64)
#
# **Không** tuyên bố K4' đạt: K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000) — bước sau.
# Runner này chỉ là B3.8 **phần đầu** (một hart, một ảnh, TAP 7 ca sau KT-253).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
TARGET="aarch64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-aarch64"
N_BOOT=1
TIMEOUT_DS=200 # 20 s/boot (cùng ngân sách `run.sh` riscv64)
LOGDIR="${KT250_LOGDIR:-${TMPDIR:-/tmp}/k4-aarch64}"
EXPECT_MARKER="${EXPECT_MARKER:-# viFastOs aarch64: boot ok}"
EXPECT_TESTS="${EXPECT_TESTS:-7}"
EXPECT_TEXT_VMA="${EXPECT_TEXT_VMA:-0x40080000}"

case "${1:-}" in
'' | *[!0-9]*) ;;
*) N_BOOT="$1" ;;
esac

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "run-aarch64.sh: $*" >&2
    exit 1
}

mkdir -p "$LOGDIR" 2>/dev/null || true

# ---------------------------------------------------------------- objdump (nhiều nguồn)
# macOS không có `llvm-objdump` trong PATH mặc định; `llvm-tools` của rustup thì có. Thử lần lượt
# và **fail-closed** nếu không có cái nào (không bỏ qua kiểm ELF).
find_objdump() {
    local c
    for c in llvm-objdump rust-objdump; do
        if command -v "$c" >/dev/null 2>&1; then
            command -v "$c"
            return 0
        fi
    done
    local sysroot host
    sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
    if [ -n "$sysroot" ] && [ -n "$host" ] && [ -x "$sysroot/lib/rustlib/$host/bin/llvm-objdump" ]; then
        echo "$sysroot/lib/rustlib/$host/bin/llvm-objdump"
        return 0
    fi
    if command -v xcrun >/dev/null 2>&1; then
        c="$(xcrun --find llvm-objdump 2>/dev/null || true)"
        if [ -n "$c" ] && [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    fi
    if command -v objdump >/dev/null 2>&1; then
        command -v objdump
        return 0
    fi
    return 1
}

# ---------------------------------------------------------------- kiểm ELF (T-objdump)
# `check_elf <image>`: 0 ⇔ mọi kiểm ELF xanh. In lý do đỏ ra stdout (dòng `#`).
check_elf() {
    local img="$1" bad=0
    local hdr text_vma vec_vma vec_type base addr want i

    hdr="$("$OBJDUMP" -h "$img" 2>/dev/null)" || {
        echo "# không đọc được header ELF bằng $OBJDUMP"
        return 1
    }
    text_vma="$(printf '%s\n' "$hdr" | awk '$2==".text"{print $4}')"
    vec_vma="$(printf '%s\n' "$hdr" | awk '$2==".text.vectors"{print $4}')"
    vec_type="$(printf '%s\n' "$hdr" | awk '$2==".text.vectors"{print $5}')"

    if [ -z "$text_vma" ]; then
        echo "# không thấy section .text"
        bad=1
    elif [ "$((0x$text_vma))" != "$((EXPECT_TEXT_VMA))" ]; then
        echo "# .text VMA=$text_vma, cần $EXPECT_TEXT_VMA"
        bad=1
    fi

    # Bất biến B3.2: bảng vector **không** nằm trong `.bss` (`.bss` là NOLOAD ⇒ không có trong ảnh).
    if [ -z "$vec_vma" ]; then
        echo "# không thấy section .text.vectors (bảng vector phải ở section riêng)"
        bad=1
    else
        if [ "$vec_type" != "TEXT" ]; then
            echo "# .text.vectors type=$vec_type, cần TEXT (KHÔNG được BSS)"
            bad=1
        fi
        if [ "$((0x$vec_vma % 0x800))" -ne 0 ]; then
            echo "# .text.vectors VMA=$vec_vma không căn 0x800 (VBAR_EL1 chỉ giữ bit [63:11])"
            bad=1
        fi
    fi

    # 16 stub đúng `base + i*0x80` — kiểm phía **asm**, không chỉ phía Rust.
    local syms
    syms="$("$OBJDUMP" -t "$img" 2>/dev/null)"
    base="$(printf '%s\n' "$syms" | awk '$NF=="__vector_base"{print $1}')"
    if [ -z "$base" ]; then
        echo "# không thấy symbol __vector_base"
        bad=1
    else
        i=0
        while [ "$i" -lt 16 ]; do
            addr="$(printf '%s\n' "$syms" | awk -v s="__vec_$i" '$NF==s{print $1}')"
            want="$(printf '%016x' $((0x$base + i * 0x80)))"
            if [ "$addr" != "$want" ]; then
                echo "# __vec_$i=$addr, cần $want (base=$base)"
                bad=1
            fi
            i=$((i + 1))
        done
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm TAP của một log
# `check_log <log> <marker> <tests>`: 0 ⇔ log đạt TAP + marker. In lý do đỏ ra stdout.
check_log() {
    local log="$1" marker="$2" tests="$3" bad=0
    local okn notn mn i

    [ -f "$log" ] || {
        echo "# không có log $log"
        return 1
    }
    grep -qx 'TAP version 13' "$log" || {
        echo "# thiếu dòng 'TAP version 13'"
        bad=1
    }
    grep -qx "1\.\.$tests" "$log" || {
        echo "# thiếu plan '1..$tests'"
        bad=1
    }
    okn="$(grep -c '^ok [0-9][0-9]* - ' "$log" || true)"
    notn="$(grep -c '^not ok ' "$log" || true)"
    if [ "${okn:-0}" -ne "$tests" ]; then
        echo "# có $okn dòng 'ok', cần $tests"
        bad=1
    fi
    if [ "${notn:-0}" -ne 0 ]; then
        echo "# có $notn dòng 'not ok'"
        bad=1
    fi
    i=1
    while [ "$i" -le "$tests" ]; do
        grep -q "^ok $i - " "$log" || {
            echo "# thiếu 'ok $i - ...'"
            bad=1
        }
        i=$((i + 1))
    done
    # Marker **nguyên văn**, đúng **một** lần (0 lần = ảnh không báo boot; 2 lần = in lặp).
    mn="$(grep -Fxc "$marker" "$log" || true)"
    if [ "${mn:-0}" -ne 1 ]; then
        echo "# marker '$marker' xuất hiện ${mn:-0} lần, cần đúng 1"
        bad=1
    fi
    if grep -q '^PANIC' "$log"; then
        echo "# log có dòng PANIC"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- ca âm (bắt buộc)
# Chạy **cùng** `check_log` trên log tốt với kỳ vọng **sai** ⇒ phải ĐỎ. Nếu một ca âm lại XANH thì
# bộ kiểm là "dấu cao su" và runner đang PASS giả.
selftest_negative() {
    local good="$1" start="${2:-1}" fails=0 n="$2" cases=0
    local tmp="$LOGDIR/selftest"
    n=$((start - 1))
    mkdir -p "$tmp" 2>/dev/null || true

    # (a) marker sai
    n=$((n + 1))
    cases=$((cases + 1))
    if check_log "$good" "# viFastOs aarch64: boot BAD" "$EXPECT_TESTS" >"$tmp/a.log" 2>&1; then
        echo "not ok $n - ca âm (a): marker sai mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (a): marker sai ⇒ ĐỎ ($(head -1 "$tmp/a.log"))"
    fi

    # (b) thiếu một dòng `ok` (mô phỏng ảnh in thiếu ca)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^ok 3 - ' "$good" >"$tmp/b.log" 2>/dev/null || true
    if check_log "$tmp/b.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/b.out" 2>&1; then
        echo "not ok $n - ca âm (b): thiếu 'ok 3' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (b): thiếu 'ok 3' ⇒ ĐỎ ($(head -1 "$tmp/b.out"))"
    fi

    # (c) có `not ok`
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^ok 2 - /not ok 2 - /' "$good" >"$tmp/c.log" 2>/dev/null || true
    if check_log "$tmp/c.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/c.out" 2>&1; then
        echo "not ok $n - ca âm (c): có 'not ok' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (c): có 'not ok' ⇒ ĐỎ ($(head -1 "$tmp/c.out"))"
    fi

    # (d) plan sai (1..N-1 thay vì 1..N) — dùng $EXPECT_TESTS, không hard-code số ca
    n=$((n + 1))
    cases=$((cases + 1))
    sed "s/^1\.\.$EXPECT_TESTS\$/1..$((EXPECT_TESTS - 1))/" "$good" >"$tmp/d.log" 2>/dev/null || true
    if check_log "$tmp/d.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/d.out" 2>&1; then
        echo "not ok $n - ca âm (d): plan sai mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (d): plan sai ⇒ ĐỎ ($(head -1 "$tmp/d.out"))"
    fi

    # (e) log rỗng (ảnh không in gì — treo trước khi in)
    n=$((n + 1))
    cases=$((cases + 1))
    : >"$tmp/e.log"
    if check_log "$tmp/e.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/e.out" 2>&1; then
        echo "not ok $n - ca âm (e): log rỗng mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (e): log rỗng ⇒ ĐỎ ($(head -1 "$tmp/e.out"))"
    fi

    echo "# ca âm: $((cases - fails))/$cases ĐỎ đúng như phải"
    [ "$fails" -eq 0 ]
}

# ---------------------------------------------------------------- khởi động
command -v cargo >/dev/null 2>&1 || die "không có cargo"
QEMU="$(command -v qemu-system-aarch64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-aarch64"
OBJDUMP="$(find_objdump)" || die "không có objdump (llvm-objdump/rust-objdump/xcrun/objdump)"

QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
echo "# qemu $QEMU_VERSION  objdump $OBJDUMP"
echo "# target $TARGET  image $IMAGE"

# `qemu_guard.sh` (KT-78): process group riêng + timeout kill cả group + trap dọn con.
# **Không** dùng `perl -e 'alarm N; exec @ARGV'`: QEMU bỏ qua SIGALRM (đo 2026-11-21).
. "$HERE/qemu_guard.sh"
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?)"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null

echo "# build ảnh aarch64 ($TARGET, release)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-aarch64 --target "$TARGET" --release \
    >"$LOGDIR/build.log" 2>&1; then
    tail -20 "$LOGDIR/build.log" >&2
    die "build ảnh aarch64 FAIL (xem $LOGDIR/build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

fails=0
total=0

# ---- kiểm 1: ELF (T-objdump)
total=$((total + 1))
if check_elf "$IMAGE" >"$LOGDIR/elf.log" 2>&1; then
    echo "ok $total - elf: .text tại $EXPECT_TEXT_VMA, .text.vectors TEXT (không BSS), 16 stub đúng offset"
else
    echo "not ok $total - elf: $(head -3 "$LOGDIR/elf.log" | tr '\n' ' ')"
    fails=$((fails + 1))
fi

# ---- kiểm 2..: boot N lần
i=1
while [ "$i" -le "$N_BOOT" ]; do
    total=$((total + 1))
    out="$LOGDIR/boot-$i.log"
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt -cpu cortex-a72 -m 128M -nographic \
        -kernel "$IMAGE" -no-reboot; then
        echo "not ok $total - boot $i: quá $((TIMEOUT_DS / 10)) s (treo, pid=$QG_LAST_PID), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    if check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$LOGDIR/tap-$i.log" 2>&1; then
        echo "ok $total - boot $i: TAP $EXPECT_TESTS/$EXPECT_TESTS + marker 1 lần"
    else
        echo "not ok $total - boot $i: $(head -3 "$LOGDIR/tap-$i.log" | tr '\n' ' '), log $out"
        fails=$((fails + 1))
    fi
    i=$((i + 1))
done

# ---- ca âm (bắt buộc, chạy **mặc định**): bộ kiểm phải ĐỎ khi kỳ vọng sai
good="$LOGDIR/boot-1.log"
if [ ! -f "$good" ]; then
    echo "not ok $((total + 1)) - ca âm: không có log tốt để chạy (boot 1 không chạy được)"
    fails=$((fails + 1))
    total=$((total + 1))
else
    # Ca âm đánh số **tiếp** dãy của runner để log TAP không trùng số hiệu.
    selftest_negative "$good" "$((total + 1))" || fails=$((fails + 1))
    total=$((total + 5))
fi

echo "# $((total - fails))/$total kiểm sạch (boot=$N_BOOT, tests=$EXPECT_TESTS)"
echo "# infra: guard_log=$QG_LOG qemu_own=$(qg_own)"
[ "$fails" -eq 0 ] || exit 1
exit 0
