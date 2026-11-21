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
# `check_log` trên log tốt với 11 kỳ vọng **sai** (marker sai, thiếu một `ok`, có `not ok`, plan
# sai, log rỗng, thiếu dòng hart 1 online, IPI in hai lần, thiếu dòng `# tlbi:`, thiếu ack
# shootdown, **hash ảnh lệch ELF**, **thiếu dòng `# umode:`**) và đòi **cả 11 phải ĐỎ**; cộng
# **ca âm (l) của KT-273** — boot **không có đĩa** phải in `# blk: no device` và TAP phải ĐỎ;
# cộng **ca âm (m) của KT-275** — boot **không có mạng** phải in `# net: no device` và TAP phải
# ĐỎ; cộng **hai ca âm của KT-277** — thiếu dòng `# wx:` (W^X) và thiếu dòng `# dma:` (cache
# maintenance) đều phải ĐỎ (**15 ca âm**). Nhờ vậy mỗi lần chạy runner vừa chứng minh ảnh boot
# sạch, vừa chứng minh bộ kiểm **không phải dấu cao su** — không có đường "PASS giả".
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs aarch64: boot ok`)
#   EXPECT_TESTS    số ca TAP (mặc định 27 — KT-253 thêm 2 ca nửa cao/physmap, KT-254 thêm 2 ca
#                   VSpace/fault, KT-259 thêm 3 ca SMP: hart 1 online, IPI 0->1, cả hai hart báo,
#                   KT-262 thêm 3 ca TLBI/smp 4, KT-269 thêm 3 ca U-mode: EL0 vào được, `svc`
#                   được xử lý, EL0 chạy tiếp sau `svc` rồi `Exit`, **KT-273 thêm 3 ca PCI/blk**:
#                   `pci: ecam`, `pci: virtio-blk found`, `blk: sector0`, **KT-275 thêm 4 ca
#                   mạng**: `net: device`, `net: dhcp`, `net: arp`, `net: ping`, **KT-277 thêm 2
#                   ca**: `wx: text RX (write faults), data RW`, `dma: cache maintenance
#                   before/after DMA`)
# cộng **hai ca âm của KT-277** — thiếu dòng `# wx:` (W^X) và thiếu dòng `# dma:` (cache
# maintenance) đều phải ĐỎ (14 ca âm). Nhờ vậy mỗi lần chạy runner vừa chứng minh ảnh boot sạch,
# vừa chứng minh bộ kiểm **không phải dấu cao su** — không có đường "PASS giả".
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs aarch64: boot ok`)
#   EXPECT_TESTS    số ca TAP (mặc định 23 — KT-253 thêm 2 ca nửa cao/physmap, KT-254 thêm 2 ca
#                   VSpace/fault, KT-259 thêm 3 ca SMP: hart 1 online, IPI 0->1, cả hai hart báo,
#                   KT-262 thêm 3 ca TLBI/smp 4, KT-269 thêm 3 ca U-mode: EL0 vào được, `svc`
#                   được xử lý, EL0 chạy tiếp sau `svc` rồi `Exit`, **KT-273 thêm 3 ca PCI/blk**:
#                   `pci: ecam`, `pci: virtio-blk found`, `blk: sector0`, **KT-277 thêm 2 ca**:
#                   `wx: text RX (write faults), data RW`, `dma: cache maintenance before/after DMA`)
#   EXPECT_TEXT_VMA VMA của `.text` (mặc định 0x40080000)
#   KT250_LOGDIR    thư mục log (mặc định $TMPDIR/k4-aarch64)
#
# **KT-259**: QEMU chạy **2 hart** (`-smp 2`) và **GICv3** (`-M virt,gic-version=3`) — mặc định
# của QEMU `virt` là GICv2 (`arm,cortex-a15-gic`, đo bằng `-M virt,dumpdtb` + `dtc`), không có
# redistributor/`ICC_*_EL1` nên IPI `SGI 0` không thể chạy. Ngoài TAP, runner còn đòi **đúng một**
# dòng bằng chứng cho mỗi hart (`# smp: hart=1 online`, `# smp: ipi recv=1`, `# smp: hart=0 alive`,
# `# smp: hart=1 alive`) — TAP nói phép đo đúng, các dòng ấy nói **hart nào** đã in.
#
# **KT-262**: `-smp 4` (bốn hart) + **shootdown `TLBI`**. Runner đòi thêm:
#   - `# smp: hart=2 online`, `# smp: hart=3 online`, `# smp: hart=2 alive`, `# smp: hart=3 alive`
#     — đúng **một** lần mỗi dòng (một ảnh chỉ chạy 2 hart vẫn in `ok 12` nhưng **không** có các
#     dòng này ⇒ ĐỎ);
#   - `# tlbi: shootdown … acks=<N-1> expected=<N-1> ok=1` — **bằng chứng ack** của shootdown;
#   - `# tlbi: hart=1 read1=… match=1` — hart 1 **đã nạp** entry vào TLB trước khi `unmap`;
#   - `# tlbi: hart=1 read2 … fault=1` — hart 1 **fault** sau `unmap` ⇒ TLB của nó đã bị xoá.
#
# **Không** tuyên bố K4' đạt: K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000) — bước sau.
# Runner này là B3.8 + B3.9 + **B3.6** (bốn hart, một ảnh, TAP 15 ca sau KT-262).
#
# **KT-265**: ảnh in thêm `# conf image: build=<16 hex> text=<len>` (FNV-1a 64 trên `.text` đã
# nạp, D2-80) — runner băm lại **cùng** section từ ELF (`test/conformance/image_digest.py`) và
# đòi log khớp **từng trường**; lệch/thiếu ⇒ boot đó ĐỎ. Đây là bất biến `soak-aarch64.sh` dùng
# chung, nên runner và soak không thể lệch nhau về định nghĩa "ảnh nào".
#
# **KT-273**: QEMU nhận thêm một **đĩa tạm** có nội dung **biết trước**
# (`-drive file=<tmp>,if=none,id=d0,format=raw,snapshot=on -device virtio-blk-pci,drive=d0`;
# 8 sector 512 B, 17 byte đầu `VIFASTOS-KT273-S0`). `snapshot=on` để QEMU **không** giữ khoá ghi
# trên tệp đĩa — soak chạy 3 luồng song song trên **cùng** tệp (nếu thiếu, QEMU báo
# `Failed to get "write" lock`, đo được khi dựng KT-273). và `-nic none` để **topology PCI tất định** (chỉ host bridge +
# virtio-blk). Runner đòi thêm ba dòng bằng chứng:
#   - `# pci: ecam … class=0x060000 … empty=0xffff …` — ECAM map đúng và giải mã thật;
#   - `# pci: virtio-blk found … vendor=0x1af4 … assigned=… readback=…` — BAR gán rồi **đọc lại**
#     đúng;
#   - `# blk: sector0 hex=<32 hex> … magic=1` — hex phải khớp **16 byte đầu của đĩa tạm** (runner
#     băm lại từ tệp bằng `od`), nên "đọc được sector 0" không thể là đọc ra số 0.
#
# **KT-275**: QEMU nhận thêm **card mạng** slirp
# (`-netdev user,id=n0 -device virtio-net-pci,netdev=n0`; `-nic none` vẫn giữ để **không** có NIC
# mặc định thứ hai — topology PCI tất định: host bridge + virtio-net + virtio-blk). Runner đòi
# thêm bốn dòng bằng chứng:
#   - `# net: device … vendor=0x1af4 … mac=<6 octet> …` — MAC **đọc từ `virtio_net_config`**,
#     BAR gán rồi đọc lại đúng;
#   - `# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2` — DHCP **thật**
#     của slirp (cùng khuôn `net.sh` riscv64);
#   - `# net: arp gw=<mac>` — ARP reply **thật** của slirp (MAC khác 0/broadcast);
#   - `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — ICMP echo **thật** qua gateway.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
TARGET="aarch64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-aarch64"
# KT-265: bản băm `.text` từ tệp ELF — nguồn đối chiếu của dòng `# conf image:` do kernel in.
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
N_BOOT=1
TIMEOUT_DS=200 # 20 s/boot (cùng ngân sách `run.sh` riscv64)
LOGDIR="${KT250_LOGDIR:-${TMPDIR:-/tmp}/k4-aarch64}"
EXPECT_MARKER="${EXPECT_MARKER:-# viFastOs aarch64: boot ok}"
EXPECT_TESTS="${EXPECT_TESTS:-27}"
EXPECT_TEXT_VMA="${EXPECT_TEXT_VMA:-0x40080000}"
# **KT-262**: số hart QEMU (`-smp`) và số hart phụ phải ack shootdown = `EXPECT_SMP - 1`.
EXPECT_SMP="${EXPECT_SMP:-4}"
EXPECT_ACKS=$((EXPECT_SMP - 1))

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

# ---------------------------------------------------------------- đĩa tạm (KT-273)
# Nội dung **biết trước**: 8 sector 512 B, 17 byte đầu là chữ ký `VIFASTOS-KT273-S0` (8 byte đầu
# khớp `virtio_blk::SECTOR0_MAGIC`). Ảnh in hex 16 byte đầu của sector 0; runner băm lại **cùng**
# 16 byte từ tệp ⇒ "đọc được sector 0" không thể là đọc ra số 0 rồi tự khen.
DISK="$LOGDIR/disk0.raw"
make_disk() {
    dd if=/dev/zero of="$DISK" bs=512 count=8 2>/dev/null || return 1
    printf 'VIFASTOS-KT273-S0' | dd of="$DISK" bs=1 seek=0 conv=notrunc 2>/dev/null || return 1
    return 0
}
make_disk || die "không tạo được đĩa tạm $DISK"
WANT_SECTOR0_HEX="$(od -An -tx1 -N16 "$DISK" 2>/dev/null | tr -d ' \n')"
[ "${#WANT_SECTOR0_HEX}" -eq 32 ] || die "không đọc được 16 byte đầu của $DISK"

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

# ---------------------------------------------------------------- kiểm dòng `# smp:` (KT-259/262)
# Các dòng **bằng chứng hart** phải có **đúng một lần** mỗi dòng. Đây là phần "mỗi hart in hart id
# của mình" của tiêu chí KT-259/262: TAP nói phép đo đúng, các dòng này nói **hart nào** đã in —
# một ảnh chỉ chạy hart 0 mà in `ok 10` vẫn bị bắt ở đây.
#
# So **chuỗi con** (`grep -Fc`, không phải `-Fxc`): dòng IPI của ảnh là
# `# smp: ipi recv=1 intid=0` — mang thêm INTID đọc từ `ICC_IAR1_EL1` sau đúng tiền tố mà KT-259
# yêu cầu. Vẫn đòi **đúng một** dòng khớp, nên in lặp (ca âm g) vẫn đỏ.
#
# **KT-262**: thêm `online`/`alive` cho hart 2 và hart 3 — bốn hart phải cùng in, không chỉ hai.
check_smp() {
    local log="$1" bad=0 line n
    for line in \
        "# smp: hart=1 online" \
        "# smp: hart=2 online" \
        "# smp: hart=3 online" \
        "# smp: ipi recv=1" \
        "# smp: hart=0 alive" \
        "# smp: hart=1 alive" \
        "# smp: hart=2 alive" \
        "# smp: hart=3 alive"; do
        n="$(grep -Fc "$line" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# tlbi:` (KT-262)
# Ba dòng **bằng chứng shootdown** — đây là phần "TLB của hart khác đã bị xoá" của B3.6:
#
#   `# tlbi: hart=1 read1=… match=1`   hart 1 **đã nạp** entry vào TLB (nếu không, "fault" sau
#                                      `unmap` không chứng minh được gì);
#   `# tlbi: shootdown … acks=N expected=N ok=1`
#                                      **mọi** hart phụ đã ack (N = `EXPECT_SMP - 1`);
#   `# tlbi: hart=1 read2 … fault=1`   hart 1 **fault** sau `unmap` ⇒ TLB của nó đã sạch.
#
# Đòi **đúng một** dòng mỗi loại: in lặp nghĩa là shootdown chạy hai lần (hoặc log bịa).
check_tlbi() {
    local log="$1" bad=0 n line
    for line in \
        "# tlbi: hart=1 read1=" \
        "# tlbi: shootdown " \
        "# tlbi: hart=1 read2 "; do
        n="$(grep -Fc "$line" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    # Nội dung từng dòng: `match=1` (hart 1 đã nạp TLB), `acks=N expected=N ok=1` (đủ ack),
    # `fault=1` (hart 1 fault sau unmap). So **chuỗi con** nên thêm trường vẫn khớp.
    grep -F "# tlbi: hart=1 read1=" "$log" | grep -Fq "match=1" || {
        echo "# dòng read1 của hart 1 không có 'match=1' (hart 1 chưa nạp TLB)"
        bad=1
    }
    # Ba trường của dòng shootdown kiểm **riêng** (thứ tự in là `expected=` rồi `acks=`, và
    # đổi thứ tự in không được làm bộ kiểm mù).
    local sd
    sd="$(grep -F "# tlbi: shootdown " "$log" || true)"
    case "$sd" in
        *"expected=$EXPECT_ACKS"*) ;;
        *)
            echo "# dòng shootdown không có 'expected=$EXPECT_ACKS'"
            bad=1
            ;;
    esac
    case "$sd" in
        *"acks=$EXPECT_ACKS"*) ;;
        *)
            echo "# dòng shootdown không có 'acks=$EXPECT_ACKS' (thiếu ack)"
            bad=1
            ;;
    esac
    case "$sd" in
        *"ok=1"*) ;;
        *)
            echo "# dòng shootdown không có 'ok=1'"
            bad=1
            ;;
    esac
    grep -F "# tlbi: hart=1 read2 " "$log" | grep -Fq "fault=1" || {
        echo "# dòng read2 của hart 1 không có 'fault=1' (TLB của hart 1 chưa bị xoá)"
        bad=1
    }
    return "$bad"
}

# ---------------------------------------------------------------- kiểm hash ảnh (KT-265 / D2-80)
# `check_digest <log>`: 0 ⇔ log có **đúng một** `# conf image: build=<16 hex> text=<len>` và cả
# hai trường khớp ELF đang chạy (`WANT_BUILD`/`WANT_TEXT` do `image_digest.py` băm từ tệp).
# Đây là bất biến **neo ảnh**: sha256 tệp không chứng minh được đoạn mã **đang chạy** là đoạn mã
# trong tệp; dòng kernel tự in thì chứng minh được.
check_digest() {
    local log="$1" bad=0 n got_build got_text
    n="$(grep -cE '^# conf image: build=[0-9a-f]{16} text=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# conf image:' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    got_build="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$log" | head -1)"
    got_text="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$log" | head -1)"
    if [ "$got_build" != "$WANT_BUILD" ]; then
        echo "# image-build=${got_build:-none} lệch ELF ($WANT_BUILD)"
        bad=1
    fi
    if [ "$got_text" != "$WANT_TEXT" ]; then
        echo "# image-text=${got_text:-none} lệch ELF ($WANT_TEXT)"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# umode:` (KT-269)
# **Bằng chứng U-mode**: đúng **một** dòng `# umode: el0 ran svc=<n> ret=<v>` — nó chỉ được in
# khi handler `svc` **thật sự** đọc được số hiệu syscall từ `x8` của EL0 và trả kết quả qua
# `x1` (quy ước `[calling.aarch64]`). Ảnh chỉ chạy EL1 (không `eret` xuống EL0) sẽ **không** có
# dòng này ⇒ ĐỎ, dù TAP 1..15 vẫn xanh.
check_umode() {
    local log="$1" bad=0 n
    n="$(grep -cE '^# umode: el0 ran svc=[0-9]+ ret=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# umode: el0 ran svc=<n> ret=<v>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # `svc #0` ⇒ imm16 = 0; `ret` phải > 0 (DebugPutStr trả độ dài chuỗi đã in).
    grep -Eq '^# umode: el0 ran svc=0 ret=[1-9][0-9]*$' "$log" || {
        echo "# dòng '# umode: el0 ran svc=0 ret=<len>' không đúng (imm16 phải 0, ret > 0)"
        bad=1
    }
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# pci:` (KT-273)
# Ba dòng **bằng chứng PCI** — đây là phần "ECAM map đúng + BAR gán rồi đọc lại" của KT-273:
#
#   `# pci: ecam … class=0x060000 … empty=0xffff …`
#       host bridge **phải** là class `0x060000` (PCI class code "bridge/host") và một slot
#       **trống** phải trả `0xffff`. Hai điều kiện ấy **cùng đúng** chỉ khi ECAM trỏ đúng chỗ và
#       giải mã thật — ECAM trỏ sai thì một trong hai sai.
#   `# pci: virtio-blk found … vendor=0x1af4 … assigned=… readback=…`
#       BAR gán xong phải **đọc lại** đúng địa chỉ đã ghi (nếu không, thiết bị không nhận BAR).
#
# Đòi **đúng một** dòng mỗi loại: in lặp nghĩa là scan chạy hai lần (hoặc log bịa).
check_pci() {
    local log="$1" bad=0 n line assigned readback
    n="$(grep -cE '^# pci: ecam ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# pci: ecam ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# pci: ecam ' "$log" || true)"
    case "$line" in
        *"class=0x060000"*) ;;
        *)
            echo "# dòng '# pci: ecam' không có 'class=0x060000' (host bridge class)"
            bad=1
            ;;
    esac
    case "$line" in
        *"empty=0xffff"*) ;;
        *)
            echo "# dòng '# pci: ecam' không có 'empty=0xffff' (slot trống phải trả 0xffff)"
            bad=1
            ;;
    esac
    n="$(grep -cE '^# pci: virtio-blk found ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# pci: virtio-blk found ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# pci: virtio-blk found ' "$log" || true)"
    case "$line" in
        *"vendor=0x1af4"*) ;;
        *)
            echo "# dòng virtio-blk không có 'vendor=0x1af4' (vendor của virtio)"
            bad=1
            ;;
    esac
    assigned="$(printf '%s\n' "$line" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    readback="$(printf '%s\n' "$line" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$assigned" ] || [ "$assigned" != "$readback" ]; then
        echo "# BAR đọc lại (${readback:-none}) khác địa chỉ đã gán (${assigned:-none})"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# blk:` (KT-273)
# `# blk: sector0 hex=<32 hex> … magic=1` — hex phải khớp **16 byte đầu của đĩa tạm** (runner
# băm lại từ tệp bằng `od`) và `magic=1` chỉ khi 8 byte đầu khớp `VIFASTOS`. Nhờ vậy "đọc được
# sector 0" không thể là đọc ra số 0 rồi tự khen.
check_blk() {
    local log="$1" bad=0 n line
    n="$(grep -cE '^# blk: sector0 hex=[0-9a-f]{32} ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# blk: sector0 hex=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# blk: sector0 ' "$log" || true)"
    case "$line" in
        *"hex=$WANT_SECTOR0_HEX"*) ;;
        *)
            echo "# sector0 hex không khớp 16 byte đầu của đĩa tạm ($WANT_SECTOR0_HEX)"
            bad=1
            ;;
    esac
    case "$line" in
        *"magic=1"*) ;;
        *)
            echo "# sector0 không có 'magic=1' (chữ ký VIFASTOS không khớp)"
            bad=1
            ;;
    esac
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# net:` (KT-275)
# Bốn dòng **bằng chứng mạng** — đây là phần "virtio-net + DHCP + ARP + ICMP thật" của KT-275:
#
#   `# net: device … vendor=0x1af4 … mac=<6 octet> … assigned=… readback=…`
#       MAC **đọc từ `virtio_net_config`** (không hardcode) và BAR gán xong phải **đọc lại**
#       đúng địa chỉ đã ghi.
#   `# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2`
#       DHCP **thật** của slirp — cùng khuôn `net.sh` riscv64 (KT-246), nên hai arch không lệch
#       nhau về định nghĩa "lấy được địa chỉ".
#   `# net: arp gw=<mac>` — ARP reply **thật**; MAC toàn 0/broadcast là giá trị vô nghĩa.
#   `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — ICMP echo **thật** qua gateway.
#
# Đòi **đúng một** dòng mỗi loại: in lặp nghĩa là bước mạng chạy hai lần (hoặc log bịa).
check_net() {
    local log="$1" bad=0 n line assigned readback
    n="$(grep -cE '^# net: device ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: device ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# net: device ' "$log" || true)"
    case "$line" in
        *"vendor=0x1af4"*) ;;
        *)
            echo "# dòng '# net: device' không có 'vendor=0x1af4' (vendor của virtio)"
            bad=1
            ;;
    esac
    # MAC phải là **6 octet hex** — MAC rỗng/toàn 0 là dấu hiệu đọc sai `virtio_net_config`.
    printf '%s\n' "$line" | grep -Eq 'mac=([0-9a-f]{2}:){5}[0-9a-f]{2}( |$)' || {
        echo "# dòng '# net: device' không có 'mac=<6 octet>'"
        bad=1
    }
    printf '%s\n' "$line" | grep -Eq 'mac=(00:){5}00( |$)' && {
        echo "# MAC toàn 0 — không phải MAC đọc từ thiết bị"
        bad=1
    }
    assigned="$(printf '%s\n' "$line" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    readback="$(printf '%s\n' "$line" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$assigned" ] || [ "$assigned" != "$readback" ]; then
        echo "# BAR mạng đọc lại (${readback:-none}) khác địa chỉ đã gán (${assigned:-none})"
        bad=1
    fi
    n="$(grep -cE '^# net: dhcp ip=10\.0\.2\.15 gw=10\.0\.2\.2 mask=255\.255\.255\.0 server=10\.0\.2\.2$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    n="$(grep -cE '^# net: arp gw=[0-9a-f]{2}(:[0-9a-f]{2}){5}$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: arp gw=<mac>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # MAC toàn 0 / broadcast là giá trị **vô nghĩa** cho một host — driver từ chối, nên log có nó
    # nghĩa là phép kiểm của driver bị bỏ (cùng bất biến `arp_parse_reply`).
    grep -E '^# net: arp gw=' "$log" | grep -Eq 'gw=(00:){5}00$' && {
        echo "# ARP trả MAC toàn 0 — không phải reply hợp lệ"
        bad=1
    }
    grep -E '^# net: arp gw=' "$log" | grep -Eq 'gw=(ff:){5}ff$' && {
        echo "# ARP trả MAC broadcast — không phải reply hợp lệ"
        bad=1
    }
    n="$(grep -cE '^# net: ping 10\.0\.2\.2 ok n=3 rtt=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: ping 10.0.2.2 ok n=3 rtt=<ms>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    [ "$bad" -eq 0 ]
}

# ---------------------------------------------------------------- kiểm dòng `# wx:` (KT-277)
# `# wx: text=RX data=RW ok=1 text_bytes=… data_bytes=… guard_pages=… guard_bytes=… … fault_ec=0x25
#        fault_access=Write fault_va=… text_unchanged=1 data_rw=1 …`
#
# Đây là phần "**W^X thật**" của KT-277:
#   - `text=RX`/`data=RW`/`ok=1` — quyền **đọc lại từ bảng trang** (không phải hằng viết tay);
#   - `fault_ec=0x25` + `fault_access=Write` — một lần **ghi** vào `.text` từ EL1 đã sinh data
#     abort quyền; ảnh map `.text` RW sẽ **không** fault ⇒ `ok=0` ⇒ ĐỎ;
#   - `text_unchanged=1` — 8 byte của `.text` **không đổi** sau lần ghi bị từ chối;
#   - `data_rw=1` — ghi vào `.data` thì **thành công** (không phải "RX cả ảnh").
#
# **Guard không map** được kiểm bằng **số trang**: `# mmu: map kernel=… pages=N` phải bằng
# `(text_bytes + data_bytes - guard_bytes) / 4096`. Nếu 8 trang guard bị map, `N` lớn hơn đúng 8
# ⇒ ĐỎ. Đây là phép kiểm QEMU-quan-sát-được cho bất biến "guard **không** map".
check_wx() {
    local log="$1" bad=0 n line want text_bytes data_bytes guard_bytes mmu_line mmu_pages want_pages
    n="$(grep -cE '^# wx: ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# wx: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# wx: ' "$log" || true)"
    for want in "text=RX" "data=RW" "ok=1" "fault_access=Write" "text_unchanged=1" "data_rw=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# wx:' không có '$want'"
                bad=1
                ;;
        esac
    done
    # Một lần **ghi** vào `.text` phải là data abort **cùng EL** (`EC=0x25`). Nhận `0x21`
    # (instruction abort) làm phương án dự phòng nhưng vẫn đòi **có** fault — không có fault
    # nghĩa là `.text` ghi được.
    case "$line" in
        *"fault_ec=0x25"* | *"fault_ec=0x21"*) ;;
        *)
            echo "# dòng '# wx:' không có 'fault_ec=0x25'/'0x21' (ghi vào .text KHÔNG fault)"
            bad=1
            ;;
    esac
    text_bytes="$(printf '%s\n' "$line" | sed -n 's/.* text_bytes=\([0-9][0-9]*\).*/\1/p')"
    data_bytes="$(printf '%s\n' "$line" | sed -n 's/.* data_bytes=\([0-9][0-9]*\).*/\1/p')"
    guard_bytes="$(printf '%s\n' "$line" | sed -n 's/.* guard_bytes=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$text_bytes" ] || [ "$text_bytes" -le 0 ]; then
        echo "# dòng '# wx:' thiếu 'text_bytes=' dương (vùng RX rỗng)"
        bad=1
    fi
    if [ -z "$data_bytes" ] || [ "$data_bytes" -le 0 ]; then
        echo "# dòng '# wx:' thiếu 'data_bytes=' dương (vùng RW rỗng)"
        bad=1
    fi
    if [ -z "$guard_bytes" ] || [ "$guard_bytes" -le 0 ]; then
        echo "# dòng '# wx:' thiếu 'guard_bytes=' dương (không có trang guard nào bị loại)"
        bad=1
    fi
    # **Guard không map**: số trang map vùng ảnh phải bằng (text + data - guard) / 4096.
    mmu_line="$(grep -E '^# mmu: map kernel=' "$log" || true)"
    mmu_pages="$(printf '%s\n' "$mmu_line" | sed -n 's/.* pages=\([0-9][0-9]*\).*/\1/p')"
    if [ -n "$text_bytes" ] && [ -n "$data_bytes" ] && [ -n "$guard_bytes" ]; then
        want_pages=$(((text_bytes + data_bytes - guard_bytes) / 4096))
        if [ -z "$mmu_pages" ]; then
            echo "# thiếu dòng '# mmu: map kernel=… pages=…' để đối chiếu số trang"
            bad=1
        elif [ "$mmu_pages" -ne "$want_pages" ]; then
            echo "# số trang map vùng ảnh ($mmu_pages) khác (text+data-guard)/4096 = $want_pages — guard có thể đã bị map"
            bad=1
        fi
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# dma:` (KT-277)
# `# dma: cache-clean ok=1 lines=… bytes=… ops=… inval_lines=… inval_bytes=… inval_ops=… …`
#
# `lines`/`inval_lines` là bộ đếm của **chính vòng lặp phát `dc`** (không phải hằng viết tay):
# bỏ lời gọi `dc civac` ⇒ `lines=0` ⇒ ĐỎ. `lines × line == bytes` chốt luôn phép tính dòng.
#
# **Trung thực về QEMU TCG**: TCG **không** mô hình hoá cache, nên **hiệu ứng** của `dc` không
# quan sát được ở đây — tầng này chỉ chứng minh **lệnh đã chạy** (số dòng + số byte). Bằng chứng
# "địa chỉ từng dòng" nằm ở test host `v2/kernel/vifast-plat-aarch64/tests/cache.rs`.
check_dma() {
    local log="$1" bad=0 n line lines bytes line_bytes inval_lines
    n="$(grep -cE '^# dma: ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# dma: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# dma: ' "$log" || true)"
    case "$line" in
        *"cache-clean ok=1"*) ;;
        *)
            echo "# dòng '# dma:' không có 'cache-clean ok=1'"
            bad=1
            ;;
    esac
    lines="$(printf '%s\n' "$line" | sed -n 's/.* lines=\([0-9][0-9]*\).*/\1/p')"
    bytes="$(printf '%s\n' "$line" | sed -n 's/.* bytes=\([0-9][0-9]*\).*/\1/p')"
    line_bytes="$(printf '%s\n' "$line" | sed -n 's/.* line=\([0-9][0-9]*\).*/\1/p')"
    inval_lines="$(printf '%s\n' "$line" | sed -n 's/.* inval_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$lines" ] || [ "$lines" -le 0 ]; then
        echo "# dòng '# dma:' có lines=${lines:-none} — không có lệnh 'dc civac' nào chạy"
        bad=1
    fi
    if [ -z "$inval_lines" ] || [ "$inval_lines" -le 0 ]; then
        echo "# dòng '# dma:' có inval_lines=${inval_lines:-none} — không có lệnh 'dc ivac' nào chạy"
        bad=1
    fi
    if [ -z "$bytes" ] || [ "$bytes" -le 0 ]; then
        echo "# dòng '# dma:' có bytes=${bytes:-none} — vùng DMA rỗng"
        bad=1
    fi
    if [ -n "$lines" ] && [ -n "$bytes" ] && [ -n "$line_bytes" ] && [ "$line_bytes" -gt 0 ]; then
        if [ $((lines * line_bytes)) -ne "$bytes" ]; then
            echo "# dòng '# dma:' lines×line=$((lines * line_bytes)) khác bytes=$bytes"
            bad=1
        fi
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm TAP của một log
# `check_log <log> <marker> <tests>`: 0 ⇔ log đạt TAP + marker + dòng `# smp:` + `# tlbi:` +
# `# umode:` + `# pci:` + `# blk:` + `# net:` + hash ảnh. In lý do đỏ ra stdout.
# `# umode:` + `# pci:` + `# blk:` + `# wx:` + `# dma:` + hash ảnh. In lý do đỏ ra stdout.
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
    check_smp "$log" || bad=1
    check_tlbi "$log" || bad=1
    check_umode "$log" || bad=1
    check_pci "$log" || bad=1
    check_blk "$log" || bad=1
    check_net "$log" || bad=1
    check_wx "$log" || bad=1
    check_dma "$log" || bad=1
    check_digest "$log" || bad=1
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

    # (f) thiếu dòng bằng chứng hart 1 online — ảnh chỉ chạy hart 0
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# smp: hart=1 online' "$good" >"$tmp/f.log" 2>/dev/null || true
    if check_log "$tmp/f.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/f.out" 2>&1; then
        echo "not ok $n - ca âm (f): thiếu '# smp: hart=1 online' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (f): thiếu dòng hart 1 online ⇒ ĐỎ ($(head -1 "$tmp/f.out"))"
    fi

    # (g) dòng IPI xuất hiện **hai** lần — hart 1 nhận hai IPI (hoặc in lặp)
    n=$((n + 1))
    cases=$((cases + 1))
    awk '{print} /^# smp: ipi recv=1/{print}' "$good" >"$tmp/g.log" 2>/dev/null || true
    if check_log "$tmp/g.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/g.out" 2>&1; then
        echo "not ok $n - ca âm (g): '# smp: ipi recv=1' hai lần mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (g): IPI in hai lần ⇒ ĐỎ ($(head -1 "$tmp/g.out"))"
    fi

    # (h) **KT-262** — thiếu **cả** dòng `# tlbi:` (shootdown không chạy / không ack)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# tlbi: ' "$good" >"$tmp/h.log" 2>/dev/null || true
    if check_log "$tmp/h.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/h.out" 2>&1; then
        echo "not ok $n - ca âm (h): thiếu dòng '# tlbi:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (h): thiếu dòng '# tlbi:' ⇒ ĐỎ ($(head -1 "$tmp/h.out"))"
    fi

    # (i) **KT-262** — shootdown **thiếu ack** (`acks=N-1` thay vì `acks=N`)
    n=$((n + 1))
    cases=$((cases + 1))
    sed "s/acks=$EXPECT_ACKS /acks=$((EXPECT_ACKS - 1)) /" \
        "$good" >"$tmp/i.log" 2>/dev/null || true
    if check_log "$tmp/i.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/i.out" 2>&1; then
        echo "not ok $n - ca âm (i): thiếu ack shootdown mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (i): thiếu ack shootdown ⇒ ĐỎ ($(head -1 "$tmp/i.out"))"
    fi

    # (j) **KT-265** — hash ảnh trong log **lệch** ELF (ảnh khác/không tự khai đúng)
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^# conf image: build=[0-9a-f]*/# conf image: build=ffffffffffffffff/' \
        "$good" >"$tmp/j.log" 2>/dev/null || true
    if check_log "$tmp/j.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/j.out" 2>&1; then
        echo "not ok $n - ca âm (j): hash ảnh lệch ELF mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (j): hash ảnh lệch ELF ⇒ ĐỎ ($(head -1 "$tmp/j.out"))"
    fi

    # (k) **KT-269** — thiếu **cả** dòng `# umode:` (ảnh không `eret` xuống EL0 / không xử lý `svc`)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# umode: ' "$good" >"$tmp/k.log" 2>/dev/null || true
    if check_log "$tmp/k.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/k.out" 2>&1; then
        echo "not ok $n - ca âm (k): thiếu dòng '# umode:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (k): thiếu dòng '# umode:' ⇒ ĐỎ ($(head -1 "$tmp/k.out"))"
    fi

    # (l) **KT-273** — **không có đĩa**: ảnh phải in `# blk: no device` và TAP phải ĐỎ.
    # Đây là ca âm **thật** (boot QEMU lần hai, không `-drive`), không phải biến đổi log: nó
    # chứng minh driver **fail-closed** khi thiếu thiết bị, thay vì đọc ra số 0 rồi in `ok`.
    n=$((n + 1))
    cases=$((cases + 1))
    out="$LOGDIR/boot-nodisk.log"
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none; then
        echo "not ok $n - ca âm (l): boot không có đĩa bị treo/timeout"
        fails=$((fails + 1))
    elif check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/l.out" 2>&1; then
        echo "not ok $n - ca âm (l): không có đĩa mà kiểm vẫn XANH"
        fails=$((fails + 1))
    elif ! grep -Fq '# blk: no device' "$out"; then
        echo "not ok $n - ca âm (l): thiếu dòng '# blk: no device'"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (l): không có đĩa ⇒ '# blk: no device' + TAP ĐỎ ($(head -1 "$tmp/l.out"))"
    fi

    # (m) **KT-275** — **không có mạng**: ảnh phải in `# net: no device` và TAP phải ĐỎ.
    # Ca âm **thật** (boot QEMU lần ba, có đĩa nhưng **không** `-netdev`/`-device virtio-net-pci`):
    # chứng minh driver mạng **fail-closed** khi thiếu thiết bị, thay vì in `ok` cho một card
    # không tồn tại. Đĩa vẫn có để `blk` xanh — nếu không, ảnh dừng ở `blk` và **không** in dòng
    # `# net:` nào, tức ca âm sẽ đỏ vì lý do sai.
    n=$((n + 1))
    cases=$((cases + 1))
    out="$LOGDIR/boot-nonet.log"
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0; then
        echo "not ok $n - ca âm (m): boot không có mạng bị treo/timeout"
        fails=$((fails + 1))
    elif check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/m.out" 2>&1; then
        echo "not ok $n - ca âm (m): không có mạng mà kiểm vẫn XANH"
        fails=$((fails + 1))
    elif ! grep -Fq '# net: no device' "$out"; then
        echo "not ok $n - ca âm (m): thiếu dòng '# net: no device'"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (m): không có mạng ⇒ '# net: no device' + TAP ĐỎ ($(head -1 "$tmp/m.out"))"
    fi

    # (n) **KT-277** — thiếu **cả** dòng `# wx:` (ảnh không tách `.text` RX / `.data` RW, hoặc
    # không thử ghi vào `.text`). TAP 22 vẫn có thể `ok` nếu ảnh tự khai — dòng bằng chứng mới là
    # thứ bộ kiểm đòi, nên thiếu nó phải ĐỎ.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# wx: ' "$good" >"$tmp/m.log" 2>/dev/null || true
    if check_log "$tmp/m.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/m.out" 2>&1; then
        echo "not ok $n - ca âm (n): thiếu dòng '# wx:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (n): thiếu dòng '# wx:' ⇒ ĐỎ ($(head -1 "$tmp/m.out"))"
    fi

    # (o) **KT-277** — thiếu **cả** dòng `# dma:` (ảnh không chạy cache maintenance trước DMA).
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# dma: ' "$good" >"$tmp/n.log" 2>/dev/null || true
    if check_log "$tmp/n.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/n.out" 2>&1; then
        echo "not ok $n - ca âm (o): thiếu dòng '# dma:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (o): thiếu dòng '# dma:' ⇒ ĐỎ ($(head -1 "$tmp/n.out"))"
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

# **KT-265 / D2-80** — hash ảnh mà kernel **tự in** phải khớp bản băm lại từ tệp ELF. Băm **cùng**
# section `.text` bằng `image_digest.py` (cùng hằng FNV-1a 64 với `vifast_plat_aarch64::image`).
command -v python3.12 >/dev/null 2>&1 || die "không có python3.12 (cần cho image_digest.py)"
[ -f "$DIGEST" ] || die "không thấy $DIGEST"
DIGEST_OUT="$(python3.12 "$DIGEST" "$IMAGE" 2>/dev/null)" || die "image_digest.py lỗi trên $IMAGE"
WANT_BUILD="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/^build=\([0-9a-f]*\).*/\1/p')"
WANT_TEXT="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/.* text=\([0-9]*\)$/\1/p')"
[ -n "$WANT_BUILD" ] && [ -n "$WANT_TEXT" ] || die "không đọc được BUILD_ID từ $DIGEST_OUT"
echo "# BUILD_ID (D2-80) = build=$WANT_BUILD text=$WANT_TEXT"

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
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0; then
        echo "not ok $total - boot $i: quá $((TIMEOUT_DS / 10)) s (treo, pid=$QG_LAST_PID), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    if check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$LOGDIR/tap-$i.log" 2>&1; then
        echo "ok $total - boot $i: TAP $EXPECT_TESTS/$EXPECT_TESTS + marker 1 lần + hash ảnh khớp ELF"
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
    total=$((total + 13))
    total=$((total + 14))
fi

echo "# $((total - fails))/$total kiểm sạch (boot=$N_BOOT, tests=$EXPECT_TESTS)"
echo "# infra: guard_log=$QG_LOG qemu_own=$(qg_own)"
[ "$fails" -eq 0 ] || exit 1
exit 0
