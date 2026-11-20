#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-257 — runner boot **x86_64** (K5' bước 1). Khuôn: `run-aarch64.sh` (KT-250) + `qemu_guard.sh`
# (KT-78: process group + timeout + dọn con).
#
# Việc runner làm, theo thứ tự:
#   1. **build** ảnh `vifast-kbin-x86_64` cho `x86_64-unknown-none` (release);
#   2. **kiểm ELF** (T-objdump của thiết kế): entry == symbol `_start`; `.text` nạp đúng
#      `0x0010_0000`; **multiboot2 header** 24 byte ở đầu `.text` đúng magic/arch/len/checksum/end
#      tag; **PVH note** (`.note.Xen`, type 18) trỏ đúng `_start` — đây là đường QEMU 11.1.3
#      thật sự dùng cho `-kernel` (xem đầu `linker.ld`);
#   3. **boot** QEMU `-m 128M -nographic -kernel <ELF> -no-reboot` qua `qemu_guard.sh`, kiểm
#      marker **đúng một lần**, `lma=1` (long mode thật), dòng `halt`, không `PANIC`;
#   4. **ca âm** (chạy MẶC ĐỊNH): (a) `EXPECT_MARKER` sai trên log tốt; (b) **ảnh thật** in marker
#      sai (build `--features wrong-marker`) boot ra log sai; (c) log mất marker; (d) log rỗng;
#      (e) marker in **hai** lần. Cả năm phải **ĐỎ** — nếu một ca xanh thì bộ kiểm là dấu cao su.
#
# Dùng: bash v2/test/boot/run-x86_64.sh [NBOOT]
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs x86_64: boot ok`)
#   EXPECT_TEXT_VMA VMA của `.text` (mặc định 0x100000)
#   KT257_LOGDIR    thư mục log (mặc định $TMPDIR/k5-x86_64)
#
# **Không** tuyên bố K5' đạt: K5' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot, D2-149) —
# runner này chỉ là **bước 1** (một ảnh, một CPU, marker + halt sạch).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
TARGET="x86_64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-x86_64"
N_BOOT=1
TIMEOUT_DS=200 # 20 s/boot (cùng ngân sách `run-aarch64.sh`)
LOGDIR="${KT257_LOGDIR:-${TMPDIR:-/tmp}/k5-x86_64}"
EXPECT_MARKER="${EXPECT_MARKER:-# viFastOs x86_64: boot ok}"
EXPECT_TEXT_VMA="${EXPECT_TEXT_VMA:-0x100000}"
HALT_LINE="# viFastOs x86_64: halt"
# 24 byte multiboot2 header (magic, arch=0, len=24, checksum, end tag {0,0,8}) — hex little-endian.
MB2_HEX="d65052e8000000001800000012afad170000000008000000"
# 16 byte đầu của PVH note: namesz=4, descsz=4, type=18, "Xen\0".
PVH_NOTE_HEX="04000000040000001200000058656e00"

case "${1:-}" in
'' | *[!0-9]*) ;;
*) N_BOOT="$1" ;;
esac

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "run-x86_64.sh: $*" >&2
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

# `section_hex <img> <section> <nbytes>` — in hex (không dấu cách) của `nbytes` đầu section.
#
# Dùng `objdump -s` (không cần file offset). Định dạng thật của llvm-objdump:
#
#     <blank>
#     <path>:\tfile format elf64-x86-64
#     Contents of section .text:
#      100000 d65052e8 00000000 18000000 12afad17  .PR.............
#
# nên **chỉ** lấy dòng có cột 1 là địa chỉ hex, và **chỉ** lấy cột 2..5 khớp đúng 8 chữ số hex —
# cột ASCII cuối dòng (`.PR....`) không được lọt vào. Dòng cuối section có thể chỉ 1–3 word.
section_hex() {
    local img="$1" sec="$2" nbytes="$3" want words
    want=$((nbytes * 2))
    words="$("$OBJDUMP" -s -j "$sec" "$img" 2>/dev/null |
        awk '$1 ~ /^[0-9a-f]+$/ {
                 for (i = 2; i <= 5 && i <= NF; i++)
                     if ($i ~ /^[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]$/)
                         printf "%s", $i
             }')"
    printf '%s' "$words" | cut -c1-"$want"
}

# ---------------------------------------------------------------- kiểm ELF (T-objdump)
# `check_elf <image>`: 0 ⇔ mọi kiểm ELF xanh. In lý do đỏ ra stdout (dòng `#`).
check_elf() {
    local img="$1" bad=0
    local hdr text_vma text_type start sym_start mb pvh note_type

    hdr="$("$OBJDUMP" -h "$img" 2>/dev/null)" || {
        echo "# không đọc được header ELF bằng $OBJDUMP"
        return 1
    }
    text_vma="$(printf '%s\n' "$hdr" | awk '$2==".text"{print $4}')"
    text_type="$(printf '%s\n' "$hdr" | awk '$2==".text"{print $5}')"

    if [ -z "$text_vma" ]; then
        echo "# không thấy section .text"
        bad=1
    elif [ "$((0x$text_vma))" != "$((EXPECT_TEXT_VMA))" ]; then
        echo "# .text VMA=$text_vma, cần $EXPECT_TEXT_VMA"
        bad=1
    elif [ "$text_type" != "TEXT" ]; then
        echo "# .text type=$text_type, cần TEXT"
        bad=1
    fi

    # Entry: `objdump -f` phải bằng **địa chỉ symbol `_start`** (linker script đặt `ENTRY(_start)`).
    start="$("$OBJDUMP" -f "$img" 2>/dev/null | sed -n 's/^start address: 0x0*\([0-9a-fA-F]*\)$/\1/p')"
    sym_start="$("$OBJDUMP" -t "$img" 2>/dev/null | awk '$NF=="_start"{print $1}')"
    if [ -z "$start" ]; then
        echo "# không đọc được start address"
        bad=1
    elif [ -z "$sym_start" ]; then
        echo "# không thấy symbol _start"
        bad=1
    elif [ "$((0x$start))" != "$((0x$sym_start))" ]; then
        echo "# entry=0x$start, _start=0x$sym_start — entry phải trỏ đúng _start"
        bad=1
    fi

    # Multiboot2 header: 24 byte đầu `.text` (linker đặt `.multiboot` trước `.text.entry`).
    mb="$(section_hex "$img" .text 24)"
    if [ "$mb" != "$MB2_HEX" ]; then
        echo "# multiboot2 header sai: '$mb', cần '$MB2_HEX'"
        bad=1
    fi

    # PVH note: section riêng + desc (4 byte) == `_start` (32-bit).
    note_type="$(printf '%s\n' "$hdr" | awk '$2==".note.Xen"{print $5}')"
    if [ -z "$note_type" ]; then
        echo "# không thấy section .note.Xen (đường boot PVH của QEMU)"
        bad=1
    else
        pvh="$(section_hex "$img" .note.Xen 20)"
        if [ "${pvh%????????}" != "$PVH_NOTE_HEX" ]; then
            echo "# PVH note header sai: '${pvh%????????}', cần '$PVH_NOTE_HEX'"
            bad=1
        fi
        local desc want_le
        desc="${pvh#????????????????????????????????}" # 16 byte đầu = 32 hex
        # Desc là **little-endian** (4 byte) ⇒ so chuỗi hex với bản LE của `_start`, không đổi
        # sang số (đổi sang số sẽ đọc ngược byte và báo đỏ giả).
        want_le="$(printf '%08x' "$((0x$sym_start))" | sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/')"
        if [ "${#pvh}" -ne 40 ]; then
            echo "# PVH note chỉ đọc được ${#pvh} hex (cần 40) — section hỏng?"
            bad=1
        elif [ "$desc" != "$want_le" ]; then
            echo "# PVH note entry(LE)=$desc, _start(LE)=$want_le — note phải trỏ đúng _start"
            bad=1
        fi
    fi
    # QEMU chỉ đọc note trong **PT_NOTE**; section có mà không có segment là im lặng không boot.
    if ! "$OBJDUMP" -p "$img" 2>/dev/null | grep -q '^ *NOTE '; then
        echo "# không có PT_NOTE segment (QEMU không thấy PVH note)"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm log của một lần boot
# `check_log <log> <marker>`: 0 ⇔ log đạt. In lý do đỏ ra stdout.
check_log() {
    local log="$1" marker="$2" bad=0 mn

    [ -f "$log" ] || {
        echo "# không có log $log"
        return 1
    }
    # Marker **nguyên văn**, đúng **một** lần (0 lần = ảnh không báo boot; 2 lần = in lặp).
    mn="$(grep -Fxc "$marker" "$log" || true)"
    if [ "${mn:-0}" -ne 1 ]; then
        echo "# marker '$marker' xuất hiện ${mn:-0} lần, cần đúng 1"
        bad=1
    fi
    # Long mode **thật**: `IA32_EFER.LMA` đọc lại sau khi bật (bit 10) — không chỉ tin asm.
    grep -q 'lma=1' "$log" || {
        echo "# thiếu 'lma=1' (long mode chưa bật thật)"
        bad=1
    }
    # Dòng halt in **trước** ACPI shutdown: chứng minh thoát sạch, không triple-fault.
    grep -qxF "$HALT_LINE" "$log" || {
        echo "# thiếu dòng halt '$HALT_LINE'"
        bad=1
    }
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
    if check_log "$good" "# viFastOs x86_64: boot BAD" >"$tmp/a.log" 2>&1; then
        echo "not ok $n - ca âm (a): marker sai mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (a): marker sai ⇒ ĐỎ ($(head -1 "$tmp/a.log"))"
    fi

    # (b) ảnh **thật** in marker sai (build `--features wrong-marker`) — log do QEMU sinh, không
    #     phải log bị sửa tay. Đây là ca âm mạnh nhất: nó chứng minh runner đỏ vì **ảnh**, không
    #     phải vì kỳ vọng.
    n=$((n + 1))
    cases=$((cases + 1))
    if [ -f "$LOGDIR/boot-bad.log" ]; then
        if check_log "$LOGDIR/boot-bad.log" "$EXPECT_MARKER" >"$tmp/b.log" 2>&1; then
            echo "not ok $n - ca âm (b): ảnh marker sai mà kiểm vẫn XANH"
            fails=$((fails + 1))
        else
            echo "ok $n - ca âm (b): ảnh marker sai ⇒ ĐỎ ($(head -1 "$tmp/b.log"))"
        fi
    else
        echo "not ok $n - ca âm (b): không có log ảnh marker sai ($LOGDIR/boot-bad.log)"
        fails=$((fails + 1))
    fi

    # (c) log mất marker (mô phỏng ảnh treo trước khi in)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF "$EXPECT_MARKER" "$good" >"$tmp/c.log" 2>/dev/null || true
    if check_log "$tmp/c.log" "$EXPECT_MARKER" >"$tmp/c.out" 2>&1; then
        echo "not ok $n - ca âm (c): mất marker mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (c): mất marker ⇒ ĐỎ ($(head -1 "$tmp/c.out"))"
    fi

    # (d) log rỗng (ảnh không in gì — triple fault hoặc treo)
    n=$((n + 1))
    cases=$((cases + 1))
    : >"$tmp/d.log"
    if check_log "$tmp/d.log" "$EXPECT_MARKER" >"$tmp/d.out" 2>&1; then
        echo "not ok $n - ca âm (d): log rỗng mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (d): log rỗng ⇒ ĐỎ ($(head -1 "$tmp/d.out"))"
    fi

    # (e) marker in **hai** lần (ảnh in lặp — runner phải bắt)
    n=$((n + 1))
    cases=$((cases + 1))
    { cat "$good"; printf '%s\n' "$EXPECT_MARKER"; } >"$tmp/e.log" 2>/dev/null || true
    if check_log "$tmp/e.log" "$EXPECT_MARKER" >"$tmp/e.out" 2>&1; then
        echo "not ok $n - ca âm (e): marker 2 lần mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (e): marker 2 lần ⇒ ĐỎ ($(head -1 "$tmp/e.out"))"
    fi

    echo "# ca âm: $((cases - fails))/$cases ĐỎ đúng như phải"
    [ "$fails" -eq 0 ]
}

# ---------------------------------------------------------------- khởi động
command -v cargo >/dev/null 2>&1 || die "không có cargo"
QEMU="$(command -v qemu-system-x86_64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-x86_64"
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

echo "# build ảnh x86_64 ($TARGET, release)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-x86_64 --target "$TARGET" --release \
    >"$LOGDIR/build.log" 2>&1; then
    tail -20 "$LOGDIR/build.log" >&2
    die "build ảnh x86_64 FAIL (xem $LOGDIR/build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

fails=0
total=0

# ---- kiểm 1: ELF (T-objdump)
total=$((total + 1))
if check_elf "$IMAGE" >"$LOGDIR/elf.log" 2>&1; then
    echo "ok $total - elf: entry=_start, .text tại $EXPECT_TEXT_VMA, multiboot2 + PVH note đúng"
else
    echo "not ok $total - elf: $(head -3 "$LOGDIR/elf.log" | tr '\n' ' ')"
    fails=$((fails + 1))
fi

# ---- kiểm 2..: boot N lần
i=1
while [ "$i" -le "$N_BOOT" ]; do
    total=$((total + 1))
    out="$LOGDIR/boot-$i.log"
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -m 128M -nographic \
        -kernel "$IMAGE" -no-reboot; then
        echo "not ok $total - boot $i: quá $((TIMEOUT_DS / 10)) s (treo, pid=$QG_LAST_PID), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    if check_log "$out" "$EXPECT_MARKER" >"$LOGDIR/log-$i.log" 2>&1; then
        echo "ok $total - boot $i: marker 1 lần + lma=1 + halt sạch"
    else
        echo "not ok $total - boot $i: $(head -3 "$LOGDIR/log-$i.log" | tr '\n' ' '), log $out"
        fails=$((fails + 1))
    fi
    i=$((i + 1))
done

# ---- ca âm (b): build **ảnh thật** in marker sai rồi boot nó. Target dir riêng để không ghi đè
#      ảnh tốt; `--features wrong-marker` chỉ đổi chuỗi marker, mọi thứ khác giữ nguyên.
BAD_IMAGE="$LOGDIR/image-bad"
echo "# build ảnh marker sai (--features wrong-marker) cho ca âm (b)"
if cargo build --manifest-path "$MANIFEST" -p vifast-kbin-x86_64 --target "$TARGET" --release \
    --features wrong-marker --target-dir "$LOGDIR/target-bad" >"$LOGDIR/build-bad.log" 2>&1; then
    cp "$LOGDIR/target-bad/$TARGET/release/vifast-kbin-x86_64" "$BAD_IMAGE" 2>/dev/null || true
    if [ -f "$BAD_IMAGE" ]; then
        if ! qg_boot "$LOGDIR/boot-bad.log" "$TIMEOUT_DS" "$QEMU" -m 128M -nographic \
            -kernel "$BAD_IMAGE" -no-reboot; then
            echo "# cảnh báo: ảnh marker sai không thoát (pid=$QG_LAST_PID) — ca âm (b) vẫn dùng log"
        fi
    fi
else
    echo "# cảnh báo: build ảnh marker sai FAIL (xem $LOGDIR/build-bad.log)"
fi

# ---- ca âm (bắt buộc, chạy **mặc định**): bộ kiểm phải ĐỎ khi kỳ vọng/ảnh sai
good="$LOGDIR/boot-1.log"
if [ ! -f "$good" ]; then
    echo "not ok $((total + 1)) - ca âm: không có log tốt để chạy (boot 1 không chạy được)"
    fails=$((fails + 1))
    total=$((total + 1))
else
    # Ca âm đánh số **tiếp** dãy của runner để log không trùng số hiệu.
    selftest_negative "$good" "$((total + 1))" || fails=$((fails + 1))
    total=$((total + 5))
fi

echo "# $((total - fails))/$total kiểm sạch (boot=$N_BOOT)"
echo "# infra: guard_log=$QG_LOG qemu_own=$(qg_own)"
[ "$fails" -eq 0 ] || exit 1
exit 0
