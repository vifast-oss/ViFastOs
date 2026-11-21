#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-257 (K5' bước 1) + **KT-261** (K5' bước 2: GDT/TSS/IDT + handler exception) + **KT-266**
# (K5' bước 3: APIC + timer) + **KT-270** (K5' bước 4: U-mode ring 3 + `int 0x80`) + **KT-274**
# (K5' bước 5: PCI ECAM + virtio-net + ARP) + **KT-276** (K5' bước 6: SMP 2 hart) + **KT-278**
# (K5' bước 7: SMP **4 hart** + **IPI** + **TSS per-hart**) + **KT-286** (K5' bước 8:
# **preemption** — hai tác vụ kernel + timer chuyển ngữ cảnh) + **KT-288** (K5' bước 9:
# **virtio-blk** — BAR leaf riêng + virtqueue + đọc **sector 0** của đĩa QEMU thật) + **KT-304**
# (**tác vụ ring 3 trong scheduler** — `schedule()` chọn nó, tick preempt nó, nó chạy lại ở
# ring 3) + **KT-308** (sửa **giả định sai** của phép kiểm KT-304: `last > first` không được
# bảo đảm dưới tải — xem dòng `# sched: umode progress`) — runner boot **x86_64**. Khuôn:
# `run-aarch64.sh` (KT-250) + `qemu_guard.sh` (KT-78: process group + timeout + dọn con).
#
# **KT-274** đổi **máy**: `-M q35` (có ECAM qua bảng ACPI `MCFG` @ `0xB000_0000`) + một thiết bị
# `virtio-net-pci` trên `-netdev user` (slirp). `-M pc` (i440fx) **không** có ECAM ⇒ ảnh rơi xuống
# legacy `0xCF8`/`0xCFC` và TAP `pci: ecam` ĐỎ — đó là hợp đồng của runner này, không phải lỗi.
#
# Việc runner làm, theo thứ tự:
#   1. **build** ảnh `vifast-kbin-x86_64` cho `x86_64-unknown-none` (release);
#   2. **kiểm ELF** (T-objdump của thiết kế): entry == symbol `_start`; `.text` nạp đúng
#      `0x0010_0000`; **multiboot2 header** 24 byte ở đầu `.text` đúng magic/arch/len/checksum/end
#      tag; **PVH note** (`.note.Xen`, type 18) trỏ đúng `_start` — đây là đường QEMU 11.1.3
#      thật sự dùng cho `-kernel` (xem đầu `linker.ld`);
#   3. **boot** QEMU `-M q35 -m 128M -nographic -kernel <ELF> -no-reboot -smp 4` +
#      `virtio-net-pci` trên `-netdev user` + **`virtio-blk-pci` trên đĩa tạm** (KT-288) qua
#      `qemu_guard.sh`, kiểm
#      **TAP** (`TAP version 13`, `1..N`, đủ N dòng `ok`, 0 dòng `not ok`), marker **đúng một
#      lần**, `lma=1` (long mode thật), dòng `halt`, không `PANIC`, **và** các dòng `# exc:` của
#      KT-261: đúng **5** exception, đúng vector + mã lỗi + `cs`, `#PF` có `cr2` đúng;
#      KT-266: `# apic: id=… version=… svr=…`, `# apic: ioapic id=… ver=… entries=…`,
#      `# timer: hz=… ticks=…` (ticks > 0), `# timer: eoi=… ticks=…`;
#      KT-270: đúng **một** dòng `# umode: ring3 ran nr=12 ret=<len>` (ring 3 thật sự chạy và
#      `int 0x80` được xử lý);
#      KT-274: `# pci: ecam base=… buses=… functions=…` (ECAM thật, không phải legacy),
#      `# net: mac=aa:bb:…` (MAC đọc từ `virtio_net_config`), `# net: arp gw=… mac=… sent=… recv=…`
#      (ARP reply **thật** của slirp);
#      KT-276: `# smp: hart=1 online`, `# smp: hart=0 alive`, `# smp: hart=1 alive` (hart phụ vào
#      long mode qua trampoline real mode + LAPIC riêng, cả hai hart báo alive);
#      **KT-278**: `# smp: hart=1|2|3 online` (ba hart phụ), `# smp: hart=0|1|2|3 alive`,
#      `# smp: tss bases=[…] … distinct=Ok(()) … tr=0x28 type=0x8b ok=1` (TSS **per-hart**, bốn
#      địa chỉ rời nhau), `# smp: ipi recv=1 intid=0x30` (IPI fixed-delivery hart 0 → hart 1),
#      `# smp: online mask=0xf alive=0xf … harts=4` (bốn hart cùng online **và** cùng báo alive);
#      **KT-286**: `# sched: switch a→b n=<k>`, `# sched: switch b→a n=<k>` (timer **thật sự**
#      chuyển ngữ cảnh theo **cả hai** chiều) và `# sched: ticks=<t> tasks=3` (tổng kết sau khi
#      scheduler nhường lại `main`);
#      **KT-304**: `# sched: umode task ran n=<n> ring=3 preempted=<n> resumed=<n>` — tác vụ
#      **ring 3** đã chạy thật (`n > 0`), bị preempt khi đang ở CPL3 (`ring=3` ở **cả hai** lần),
#      được `schedule()` chọn **lại** (`resumed ≥ 1`);
#      **KT-308**: `# sched: umode progress first=<n> last=<n> delta=<n>` — `first > 0` (đã chạy
#      trước lần preempt đầu) và `last >= first` / `delta >= 0` (bộ đếm **không giảm**).
#      **Không** đòi `last > first`: PIT 100 Hz, host deschedule ≥ 10 ms đúng lúc `iretq` ⇒ tick
#      kế pending ⇒ 0 vòng là **hợp lệ** (soak KT-308: 1/600 boot, boot 331);
#      **KT-288**: `# blk: device … vendor=0x1af4 … bar_distinct=1 capacity=<n> … status=0x0f`
#      (thiết bị thật, BAR **rời** BAR net, driver ở DRIVER_OK, dung lượng đọc từ thiết bị) và
#      `# blk: sector0 hex=<32 hex> … magic=1` (hex phải khớp **16 byte đầu của đĩa tạm** — runner
#      băm lại từ tệp bằng `od`, nên "đọc được sector 0" không thể là đọc ra số 0 rồi tự khen);
#   4. **ca âm** (chạy MẶC ĐỊNH): (a) `EXPECT_MARKER` sai trên log tốt; (b) **ảnh thật** in marker
#      sai (build `--features wrong-marker`) boot ra log sai; (c) log mất marker; (d) log rỗng;
#      (e) marker in **hai** lần; (f) **mất dòng `# exc:`** (KT-261); (g) **sai vector** (KT-261);
#      (h) **mất dòng `# timer:`** (KT-266); (i) **mất dòng `# apic:`** (KT-266);
#      (j) **mất dòng `# umode:`** (KT-270); (k) **không có mạng** — boot **thật** không có
#      `-device virtio-net-pci` ⇒ ảnh in `# net: no device` và TAP 16 ĐỎ (KT-274);
#      (l) **mất dòng `# smp:`** (KT-276) — ảnh không đánh thức được hart phụ;
#      (m) **mất dòng `# smp: ipi recv=`** (KT-278) — IPI không tới hart đích;
#      (n) **mất dòng `# sched:`** (KT-286) — timer không chuyển ngữ cảnh;
#      (o) **không có đĩa** (KT-288) — boot **thật** không có `-drive`/`-device virtio-blk-pci`
#          ⇒ ảnh in `# blk: no device` và TAP 25 ĐỎ;
#      (p) **tác vụ ring 3 không chạy lại** (KT-304) — log tốt bị sửa `resumed=0` (bị preempt
#          nhưng `schedule()` không chọn lại) ⇒ phải ĐỎ;
#      (q) **preempt không ở ring 3** (KT-308) — log tốt bị sửa `cs=0x1b` → `cs=0x8` ở dòng
#          `# sched: umode counts` ⇒ phải ĐỎ;
#      (r) **bộ đếm giảm** (KT-308) — log tốt bị sửa `# sched: umode progress` thành
#          `first=100 last=50 delta=-50` ⇒ phải ĐỎ;
#      (s) **chưa chạy trước lần preempt đầu** (KT-308) — log tốt bị sửa thành `first=0` ⇒ ĐỎ.
#      Cả **mười chín** phải **ĐỎ** — nếu một ca xanh thì bộ kiểm là dấu cao su.
#
# Dùng: bash v2/test/boot/run-x86_64.sh [NBOOT]
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs x86_64: boot ok`)
#   EXPECT_TESTS    số ca TAP (mặc định 26 — KT-261: 7 ca exception; KT-266: 4 ca APIC/timer;
#                   KT-270: 3 ca U-mode; KT-274: 3 ca PCI/net; KT-278: 4 ca SMP; KT-286: 2 ca
#                   preemption; KT-304: 1 ca tác vụ ring 3; KT-288: 2 ca virtio-blk)
#   EXPECT_EXC      số dòng `# exc: vec=` (mặc định 5 — một cho mỗi exception có chủ đích)
#   EXPECT_PF_ADDR  `cr2` của ca `#PF` (mặc định 0x700000000000)
#   EXPECT_TEXT_VMA VMA của `.text` (mặc định 0x100000)
#   KT257_LOGDIR    thư mục log (mặc định $TMPDIR/k5-x86_64)
#
# **Không** tuyên bố K5' đạt: K5' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot, D2-149) —
# runner này là **bước 1..9** (một ảnh, GDT/TSS/IDT + 5 exception, LAPIC + IOAPIC + timer, U-mode
# ring 3 + `int 0x80`, PCI/virtio-net/ARP, **4 hart + IPI + TSS per-hart**, preemption hai tác vụ,
# **virtio-blk đọc sector 0**, marker + halt sạch). **Không** IPC từ ring 3, **không** virtio-gpu
# trên x86_64, **không** MSI-X, **không** hotplug, **không** `-cpu max`, **không**
# `syscall`/`sysret`, **không** soak 600 boot.
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
# KT-261: TAP + dòng `# exc:` là **hợp đồng** giữa ảnh và runner.
EXPECT_TESTS="${EXPECT_TESTS:-26}"
EXPECT_EXC="${EXPECT_EXC:-5}"
EXPECT_PF_ADDR="${EXPECT_PF_ADDR:-0x700000000000}"
HALT_LINE="# viFastOs x86_64: halt"
# **KT-274**: `-M q35` (ECAM qua `MCFG`) + một `virtio-net-pci` trên slirp. Một chỗ quyết định:
# mọi lần boot của runner dùng **cùng** máy + **cùng** thiết bị, trừ ca âm (k) cố ý bỏ thiết bị.
# **KT-278**: `-smp 4` — BSP (hart 0) + **ba** hart phụ (hart 1..3). Ảnh đánh thức từng hart phụ
# bằng `INIT`+`SIPI` **tuần tự**; runner không có ca âm "1 hart" vì ảnh **đòi** mask `0xf` online
# (TAP 18) — thiếu `-smp 4` thì QEMU chỉ có hart 0 và TAP 18 ĐỎ (đó là hành vi đúng, không phải
# ca âm riêng).
QEMU_MACHINE=(-M q35 -m 128M -nographic -no-reboot -smp 4)
QEMU_NET=(-netdev user,id=n0 -device virtio-net-pci,netdev=n0)

# Ngân sách riêng cho boot **không mạng**: ảnh dừng ở TAP 16 đỏ (`cli; hlt`) nên QEMU **không**
# tự thoát; 10 s là dư cho phần in (đo được ~2 s) mà không phí 20 s như boot thường.
NONET_TIMEOUT_DS=100
# Ngân sách cho boot **không đĩa**: ảnh chạy hết net + SMP + preemption + tác vụ ring 3 rồi
# mới tới TAP 25 đỏ (`cli; hlt`) — đo được ~4 s, 15 s là dư mà không phí 20 s như boot thường.
NODISK_TIMEOUT_DS=150
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

# ---------------------------------------------------------------- đĩa tạm (KT-288)
# Nội dung **biết trước**: 8 sector 512 B, 17 byte đầu là chữ ký `VIFASTOS-KT288-S0` (8 byte đầu
# khớp `virtio_blk::SECTOR0_MAGIC`). Ảnh in hex 16 byte đầu của sector 0; runner băm lại **cùng**
# 16 byte từ tệp bằng `od` ⇒ "đọc được sector 0" không thể là đọc ra số 0 rồi tự khen (khuôn
# KT-273 của `run-aarch64.sh`).
DISK="$LOGDIR/disk0.raw"
DISK_SECTORS=8
make_disk() {
    dd if=/dev/zero of="$DISK" bs=512 count="$DISK_SECTORS" 2>/dev/null || return 1
    printf 'VIFASTOS-KT288-S0' | dd of="$DISK" bs=1 seek=0 conv=notrunc 2>/dev/null || return 1
    return 0
}
make_disk || die "không tạo được đĩa tạm $DISK"
WANT_SECTOR0_HEX="$(od -An -tx1 -N16 "$DISK" 2>/dev/null | tr -d ' \n')"
[ "${#WANT_SECTOR0_HEX}" -eq 32 ] || die "không đọc được 16 byte đầu của $DISK"
# Dung lượng theo sector 512 B — **đo từ tệp** (không hardcode 8): ảnh đọc `capacity` từ
# `virtio_blk_config`, runner đối chiếu với đĩa nó vừa tạo.
DISK_BYTES="$(wc -c <"$DISK" | tr -d ' ')"
WANT_CAPACITY=$((DISK_BYTES / 512))
[ "$WANT_CAPACITY" -eq "$DISK_SECTORS" ] || die "đĩa tạm $DISK_BYTES byte, cần $((DISK_SECTORS * 512))"

# **KT-288**: một đĩa `virtio-blk-pci` trên tệp tạm 8 sector. Một chỗ quyết định: mọi lần boot
# của runner dùng **cùng** máy + **cùng** thiết bị, trừ ca âm (k) cố ý bỏ mạng và ca âm (o) cố ý
# bỏ đĩa. `format=raw` + `if=none` + `id=d0` đúng khuôn KT-273.
QEMU_DISK=(-drive "file=$DISK,if=none,id=d0,format=raw" -device virtio-blk-pci,drive=d0)

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
    local hdr text_vma text_type start sym_start mb pvh note_type tramp_start tramp_end

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

    # **KT-276** — trang trampoline SMP: `__trampoline_start` phải **64 KiB-aligned** (điều kiện
    # để `offset label` 16-bit trong đoạn real mode bằng đúng offset trong trang) và trang phải
    # đúng **4 KiB** (một trang SIPI). Sai một trong hai ⇒ hart phụ chạy vào rác.
    tramp_start="$("$OBJDUMP" -t "$img" 2>/dev/null | awk '$NF=="__trampoline_start"{print $1; exit}')"
    tramp_end="$("$OBJDUMP" -t "$img" 2>/dev/null | awk '$NF=="__trampoline_end"{print $1; exit}')"
    if [ -z "$tramp_start" ] || [ -z "$tramp_end" ]; then
        echo "# không thấy __trampoline_start/__trampoline_end (trang trampoline SMP)"
        bad=1
    elif [ "$((0x$tramp_start % 0x10000))" -ne 0 ]; then
        echo "# __trampoline_start=0x$tramp_start không 64 KiB-aligned (offset 16-bit real mode sẽ sai)"
        bad=1
    elif [ "$((0x$tramp_end - 0x$tramp_start))" -ne 4096 ]; then
        echo "# trang trampoline $((0x$tramp_end - 0x$tramp_start)) byte, cần 4096"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# umode:` (KT-270)
# **Bằng chứng U-mode**: đúng **một** dòng `# umode: ring3 ran nr=<n> ret=<v>` — nó chỉ được in
# khi handler `int 0x80` **thật sự** đọc được số hiệu syscall từ `rax` của ring 3 và trả kết quả
# qua `rdx` (quy ước `[calling.x86_64]`). Ảnh chỉ chạy CPL0 (không `iretq` xuống ring 3) sẽ
# **không** có dòng này ⇒ ĐỎ, dù TAP 1..11 vẫn xanh.
check_umode() {
    local log="$1" bad=0 n
    n="$(grep -cE '^# umode: ring3 ran nr=[0-9]+ ret=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# umode: ring3 ran nr=<n> ret=<v>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # `DebugPutStr` = syscall 12; `ret` phải > 0 (số byte chuỗi user đã in).
    grep -Eq '^# umode: ring3 ran nr=12 ret=[1-9][0-9]*$' "$log" || {
        echo "# dòng '# umode: ring3 ran nr=12 ret=<len>' không đúng (nr phải 12, ret > 0)"
        bad=1
    }
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# blk:` (KT-288)
# Hai dòng **bằng chứng đĩa** — đây là phần "virtio-blk đọc sector 0 thật" của KT-288:
#
#   `# blk: device … vendor=0x1af4 … bar_distinct=1 capacity=<n> … status=0x0f`
#       Thiết bị **thật** (vendor virtio), BAR **rời** BAR của virtio-net (leaf riêng — trên
#       x86_64 firmware gán BAR nên "riêng" là bất biến kiểm được), dung lượng **đọc từ
#       `virtio_blk_config`** khớp đĩa runner vừa tạo, và `device_status` đọc lại đủ bốn bit
#       `ACK|DRIVER|FEATURES_OK|DRIVER_OK` (`0x0f`).
#   `# blk: sector0 hex=<32 hex> … magic=1`
#       Hex phải khớp **16 byte đầu của đĩa tạm** (runner băm lại từ tệp bằng `od`) và `magic=1`
#       chỉ khi 8 byte đầu khớp `VIFASTOS`. Nhờ vậy "đọc được sector 0" không thể là đọc ra số 0
#       rồi tự khen. `errors=0`/`timeouts=0` chốt là request hoàn tất thật, không phải timeout.
check_blk() {
    local log="$1" bad=0 n line
    n="$(grep -cE '^# blk: device ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# blk: device ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# blk: device ' "$log" || true)"
    case "$line" in
        *"vendor=0x1af4"*) ;;
        *)
            echo "# dòng blk: device không có 'vendor=0x1af4' (vendor của virtio)"
            bad=1
            ;;
    esac
    case "$line" in
        *"bar_distinct=1"*) ;;
        *)
            echo "# dòng blk: device không có 'bar_distinct=1' (BAR blk đè BAR net?)"
            bad=1
            ;;
    esac
    case "$line" in
        *"capacity=$WANT_CAPACITY "*) ;;
        *)
            echo "# dòng blk: device không có 'capacity=$WANT_CAPACITY' (dung lượng đĩa tạm)"
            bad=1
            ;;
    esac
    case "$line" in
        *"status=0x0f"*) ;;
        *)
            echo "# dòng blk: device không có 'status=0x0f' (chưa DRIVER_OK đầy đủ)"
            bad=1
            ;;
    esac
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
    case "$line" in
        *"errors=0"*) ;;
        *)
            echo "# sector0 không có 'errors=0' (request đĩa có lỗi)"
            bad=1
            ;;
    esac
    case "$line" in
        *"timeouts=0"*) ;;
        *)
            echo "# sector0 không có 'timeouts=0' (poll used quá hạn)"
            bad=1
            ;;
    esac
    return "$bad"
}

# ---------------------------------------------------------------- kiểm log của một lần boot
# `check_log <log> <marker> <tests>`: 0 ⇔ log đạt TAP + marker + `# exc:` + `# umode:` + `# blk:`.
# In lý do đỏ ra stdout.
#
# KT-261 thêm **ba** lớp kiểm mới, mỗi lớp bắt một lớp lỗi khác nhau:
#
#   1. **TAP** — ảnh phải phát `TAP version 13`, plan `1..N`, **đủ** N dòng `ok i - `, **0** dòng
#      `not ok`. Một ca kiểm trong ảnh sai ⇒ runner đỏ, không chỉ log.
#   2. **`# exc:`** — đúng `EXPECT_EXC` dòng, và **từng vector** phải có mặt với mã lỗi đúng:
#      `#BP` 3/err 0, `#UD` 6/err 0, `#PF` 14/err 0 + `cr2` đúng, `#GP` 13/err 0x38 (selector
#      ngoài GDT), `isr_unknown` 255/err 0. Đây là bằng chứng **handler chạy thật** với **đúng**
#      vector — không phải chỉ "ảnh in được một dòng nào đó".
#   3. **`cs=0x8`** — exception đến từ kernel code, không phải từ một selector rác.
check_log() {
    local log="$1" marker="$2" tests="$3" bad=0 mn okn notn i excn

    [ -f "$log" ] || {
        echo "# không có log $log"
        return 1
    }
    # ---- TAP
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
    # ---- KT-261: dòng `# exc:` — đúng số lượng + đúng vector/mã lỗi/`cs`
    excn="$(grep -c '^# exc: vec=' "$log" || true)"
    if [ "${excn:-0}" -ne "$EXPECT_EXC" ]; then
        echo "# có $excn dòng '# exc: vec=', cần $EXPECT_EXC"
        bad=1
    fi
    # `#BP` (int3): vector 3, không mã lỗi.
    grep -qE '^# exc: vec=3 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=3 err=0x0 … cs=0x8' (int3)"
        bad=1
    }
    # `#UD` (ud2): vector 6, không mã lỗi.
    grep -qE '^# exc: vec=6 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=6 err=0x0 … cs=0x8' (ud2)"
        bad=1
    }
    # `#PF`: vector 14, `P = 0` (err 0x0 khi đọc địa chỉ chưa map), `cr2` đúng địa chỉ đã đọc.
    grep -qE '^# exc: vec=14 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=14 err=0x0 … cs=0x8' (page fault)"
        bad=1
    }
    grep -qxF "# exc: cr2=$EXPECT_PF_ADDR" "$log" || {
        echo "# thiếu '# exc: cr2=$EXPECT_PF_ADDR' (CR2 của #PF)"
        bad=1
    }
    # `#GP`: vector 13, mã lỗi = selector ngoài GDT (`0x38`).
    grep -qE '^# exc: vec=13 err=0x38 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=13 err=0x38 … cs=0x8' (gp)"
        bad=1
    }
    # `isr_unknown`: sentinel **254** (KT-266 dời từ 255 vì 255 là vector thật của spurious) —
    # mọi vector không có stub riêng vẫn **thấy được**.
    grep -qE '^# exc: vec=254 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=254 err=0x0 … cs=0x8' (isr_unknown)"
        bad=1
    }
    # ---- KT-266: APIC + timer — bốn dòng bằng chứng, mỗi dòng một lớp lỗi khác nhau.
    # LAPIC: ID/version đọc từ MMIO + SVR đọc lại (bit 8 = enable, vector spurious 255).
    grep -qE '^# apic: id=[0-9]+ version=0x[0-9a-f]+ svr=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# apic: id=… version=0x… svr=0x…' (LAPIC)"
        bad=1
    }
    # IOAPIC: ID/version + số redirection entry (chứng minh có IOAPIC thật).
    grep -qE '^# apic: ioapic id=[0-9]+ ver=0x[0-9a-f]+ entries=[0-9]+$' "$log" || {
        echo "# thiếu '# apic: ioapic id=… ver=0x… entries=…' (IOAPIC)"
        bad=1
    }
    # Timer: `hz` đo được + `ticks` **> 0** (không hardcode số tick chính xác).
    grep -qE '^# timer: hz=[0-9]+ ticks=[0-9]+' "$log" || {
        echo "# thiếu '# timer: hz=… ticks=…' (timer)"
        bad=1
    }
    if grep -qE '^# timer: hz=[0-9]+ ticks=0( |$)' "$log"; then
        echo "# timer ticks=0 (timer không chạy)"
        bad=1
    fi
    # EOI: số EOI phải bằng số tick (handler ghi EOI mỗi tick).
    grep -qE '^# timer: eoi=[0-9]+ ticks=[0-9]+' "$log" || {
        echo "# thiếu '# timer: eoi=… ticks=…' (EOI)"
        bad=1
    }
    # ---- KT-274: PCI + virtio-net + ARP — ba dòng bằng chứng, mỗi dòng một lớp lỗi khác nhau.
    # ECAM: đường truy cập **phải** là `ecam` (q35) và đã thấy ≥ 1 function. Log rơi xuống
    # `# pci: legacy …` (hoặc không có dòng nào) ⇒ ĐỎ: TAP 15 của ảnh cũng đỏ, nhưng runner phải
    # bắt được **từ log** chứ không chỉ tin TAP.
    grep -qE '^# pci: ecam base=0x[0-9a-f]+ buses=[0-9]+\.\.[0-9]+ functions=[1-9][0-9]*$' "$log" || {
        echo "# thiếu '# pci: ecam base=0x… buses=… functions=…' (ECAM thật)"
        bad=1
    }
    # MAC: đọc từ `virtio_net_config`, dạng `aa:bb:cc:dd:ee:ff` (không phải hằng viết tay).
    grep -qE '^# net: mac=([0-9a-f]{2}:){5}[0-9a-f]{2} ' "$log" || {
        echo "# thiếu '# net: mac=aa:bb:cc:dd:ee:ff …' (MAC từ thiết bị)"
        bad=1
    }
    # ARP: gateway + MAC trả lời + đã **gửi** và **nhận** ≥ 1 frame (reply thật của slirp).
    grep -qE '^# net: arp gw=[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+ mac=([0-9a-f]{2}:){5}[0-9a-f]{2} sent=[1-9][0-9]* recv=[1-9][0-9]*' "$log" || {
        echo "# thiếu '# net: arp gw=… mac=… sent=… recv=…' (ARP reply thật)"
        bad=1
    }
    # ---- KT-270: dòng `# umode:` — ring 3 thật sự chạy + `int 0x80` được xử lý.
    check_umode "$log" || bad=1
    # ---- **KT-288**: hai dòng `# blk:` — thiết bị + sector 0 khớp đĩa tạm.
    check_blk "$log" || bad=1
    # ---- KT-276 + **KT-278**: SMP **4 hart** — mỗi dòng một lớp lỗi khác nhau.
    # `hart=N online` cho **cả ba** hart phụ: trampoline real mode chạy thật trên từng hart.
    for h in 1 2 3; do
        grep -qxF "# smp: hart=$h online" "$log" || {
            echo "# thiếu '# smp: hart=$h online' (hart phụ $h chưa vào long mode)"
            bad=1
        }
    done
    # `hart=N alive` cho **cả bốn** hart: LAPIC riêng của từng hart chạy (timer đánh thức `hlt`).
    for h in 0 1 2 3; do
        grep -qxF "# smp: hart=$h alive" "$log" || {
            echo "# thiếu '# smp: hart=$h alive' (hart $h không báo alive)"
            bad=1
        }
    done
    # **KT-278** — TSS **per-hart**: bốn địa chỉ TSS rời nhau (`distinct=Ok(())`), `tr=0x28`,
    # descriptor **busy** (`type=0x8b` — CPU đổi `0x89`→`0x8b` khi `ltr`), `ok=1`.
    grep -qE '^# smp: tss bases=\[0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+\] ist1=\[0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+\] distinct=Ok\(\(\)\) ist1_distinct=Ok\(\(\)\) tr=0x28 type=0x8b ok=1$' "$log" || {
        echo "# thiếu '# smp: tss bases=[…] … distinct=Ok(()) … tr=0x28 type=0x8b ok=1' (TSS per-hart)"
        bad=1
    }
    # **KT-278** — IPI: hart 1 **nhận** đúng **một** IPI vector `0x30` (đếm trong handler ngắt).
    grep -qxF '# smp: ipi recv=1 intid=0x30' "$log" || {
        echo "# thiếu '# smp: ipi recv=1 intid=0x30' (IPI không tới hart 1)"
        bad=1
    }
    # **KT-278** — bốn hart **cùng** online **và** cùng báo alive trên **cùng** mặt nạ `0xf`.
    grep -qE '^# smp: online mask=0xf alive=0xf expect=0xf harts=4 ' "$log" || {
        echo "# thiếu '# smp: online mask=0xf alive=0xf … harts=4' (bốn hart chưa cùng online+alive)"
        bad=1
    }
    # ---- **KT-286**: preemption — ba dòng bằng chứng, mỗi dòng một lớp lỗi khác nhau.
    # Chuyển **A→B**: timer đã thật sự lưu ngữ cảnh A và nạp ngữ cảnh B.
    grep -qE '^# sched: switch a→b n=[0-9]+$' "$log" || {
        echo "# thiếu '# sched: switch a→b n=<k>' (timer chưa chuyển A→B)"
        bad=1
    }
    # Chuyển **B→A**: chiều ngược lại — một chiều thôi thì chỉ là "chạy B rồi dừng".
    grep -qE '^# sched: switch b→a n=[0-9]+$' "$log" || {
        echo "# thiếu '# sched: switch b→a n=<k>' (timer chưa chuyển B→A)"
        bad=1
    }
    # Tổng kết: scheduler đã nhường lại `main` và đếm được tick.
    grep -qE '^# sched: ticks=[0-9]+ tasks=3$' "$log" || {
        echo "# thiếu '# sched: ticks=<t> tasks=3' (scheduler không nhường lại main)"
        bad=1
    }
    if grep -qE '^# sched: ticks=0 ' "$log"; then
        echo "# sched ticks=0 (timer không chạy trong lúc scheduler bật)"
        bad=1
    fi
    # ---- **KT-304** + **KT-308**: tác vụ **ring 3** trong scheduler — chạy thật, bị preempt
    # khi ở CPL3 **cả hai** lần, được `schedule()` chọn **lại**, đã chạy trước lần preempt đầu
    # và bộ đếm **không giảm**. Năm điều kiện tách bạch: `n > 0` (đã chạy), `ring=3` (cs lúc
    # preempt = 0x1B ở **cả hai** lần), `preempted ≥ 2` (rời U hai lần), `resumed ≥ 1` (được
    # chọn lại), `first > 0` + `last >= first` (bộ đếm đơn điệu). Thiếu dòng ⇒ ĐỎ, không thể
    # "xanh nhờ TAP 24".
    grep -qE '^# sched: umode task ran n=[1-9][0-9]* ring=3 preempted=[2-9][0-9]* resumed=[1-9][0-9]*$' "$log" || {
        echo "# thiếu '# sched: umode task ran n=… ring=3 preempted=… resumed=…' (tác vụ ring 3 không chạy lại)"
        bad=1
    }
    # **KT-308** — `cs` của lần preempt **cuối** phải là ring 3 (`0x1B`). `ring=3` ở dòng trên
    # đã đòi **cả hai** lần; dòng này là bằng chứng trực tiếp của lần cuối (ca âm (q) sửa
    # `cs=0x1b` → `cs=0x8` phải bị bắt).
    grep -qE '^# sched: umode counts first=[0-9]+ last=[0-9]+ cs=0x1b kernel_stack=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# sched: umode counts … cs=0x1b …' (lần preempt cuối không ở ring 3)"
        bad=1
    }
    # **KT-308** — dòng bằng chứng định lượng: `first > 0` (tác vụ **đã chạy** trước lần preempt
    # đầu) và `last >= first` / `delta >= 0` (bộ đếm **không giảm**). **Không** đòi
    # `last > first`: PIT 100 Hz, host deschedule ≥ 10 ms đúng lúc `iretq` ⇒ tick kế pending ⇒
    # 0 vòng là hợp lệ (soak KT-308: 1/600 boot, boot 331; 13/600 boot `last - first < 100.000`).
    u_prog="$(grep -E '^# sched: umode progress first=[0-9]+ last=[0-9]+ delta=-?[0-9]+$' "$log" || true)"
    if [ -z "$u_prog" ]; then
        echo "# thiếu '# sched: umode progress first=… last=… delta=…' (không có bằng chứng bộ đếm)"
        bad=1
    else
        u_first="$(printf '%s\n' "$u_prog" | sed -n 's/.*first=\([0-9]*\).*/\1/p')"
        u_last="$(printf '%s\n' "$u_prog" | sed -n 's/.*last=\([0-9]*\).*/\1/p')"
        u_delta="$(printf '%s\n' "$u_prog" | sed -n 's/.*delta=\(-\{0,1\}[0-9]*\).*/\1/p')"
        if [ "${u_first:-0}" -le 0 ]; then
            echo "# umode progress first=${u_first:-0} (tác vụ chưa chạy trước lần preempt đầu)"
            bad=1
        fi
        if [ "${u_last:-0}" -lt "${u_first:-0}" ]; then
            echo "# umode progress last=${u_last:-0} < first=${u_first:-0} (bộ đếm giảm)"
            bad=1
        fi
        if [ "${u_delta:-0}" -lt 0 ]; then
            echo "# umode progress delta=${u_delta:-0} < 0 (bộ đếm giảm)"
            bad=1
        fi
    fi
    # ---- marker + long mode + halt
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
    # `check_log` giờ cần số ca TAP — ca âm dùng **đúng** kỳ vọng của lượt chạy thật.

    # (a) marker sai
    n=$((n + 1))
    cases=$((cases + 1))
    if check_log "$good" "# viFastOs x86_64: boot BAD" "$EXPECT_TESTS" >"$tmp/a.log" 2>&1; then
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
        if check_log "$LOGDIR/boot-bad.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/b.log" 2>&1; then
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
    if check_log "$tmp/c.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/c.out" 2>&1; then
        echo "not ok $n - ca âm (c): mất marker mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (c): mất marker ⇒ ĐỎ ($(head -1 "$tmp/c.out"))"
    fi

    # (d) log rỗng (ảnh không in gì — triple fault hoặc treo)
    n=$((n + 1))
    cases=$((cases + 1))
    : >"$tmp/d.log"
    if check_log "$tmp/d.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/d.out" 2>&1; then
        echo "not ok $n - ca âm (d): log rỗng mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (d): log rỗng ⇒ ĐỎ ($(head -1 "$tmp/d.out"))"
    fi

    # (e) marker in **hai** lần (ảnh in lặp — runner phải bắt)
    n=$((n + 1))
    cases=$((cases + 1))
    { cat "$good"; printf '%s\n' "$EXPECT_MARKER"; } >"$tmp/e.log" 2>/dev/null || true
    if check_log "$tmp/e.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/e.out" 2>&1; then
        echo "not ok $n - ca âm (e): marker 2 lần mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (e): marker 2 lần ⇒ ĐỎ ($(head -1 "$tmp/e.out"))"
    fi

    # (f) **mất dòng `# exc:`** (KT-261) — ảnh không chạy handler exception (hoặc handler im lặng).
    #     Đây là ca âm trực tiếp của tiêu chí "in `# exc: vec=…`": nếu runner vẫn XANH khi log
    #     không có một dòng `# exc:` nào thì toàn bộ phần KT-261 của bộ kiểm là dấu cao su.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^# exc: ' "$good" >"$tmp/f.log" 2>/dev/null || true
    if check_log "$tmp/f.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/f.out" 2>&1; then
        echo "not ok $n - ca âm (f): mất dòng '# exc:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (f): mất dòng '# exc:' ⇒ ĐỎ ($(head -1 "$tmp/f.out"))"
    fi

    # (g) **sai vector** (KT-261) — `#BP` in thành `vec=4`. Bắt lớp lỗi "handler chạy nhưng đọc
    #     sai vector" (ví dụ stub đẩy nhầm số): log vẫn có `# exc:` và TAP vẫn đủ, chỉ vector sai.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^# exc: vec=3 /# exc: vec=4 /' "$good" >"$tmp/g.log" 2>/dev/null || true
    if check_log "$tmp/g.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/g.out" 2>&1; then
        echo "not ok $n - ca âm (g): sai vector mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (g): sai vector ⇒ ĐỎ ($(head -1 "$tmp/g.out"))"
    fi

    # (h) **mất dòng `# timer:`** (KT-266) — timer không chạy hoặc handler không đếm.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^# timer: ' "$good" >"$tmp/h.log" 2>/dev/null || true
    if check_log "$tmp/h.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/h.out" 2>&1; then
        echo "not ok $n - ca âm (h): mất dòng '# timer:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (h): mất dòng '# timer:' ⇒ ĐỎ ($(head -1 "$tmp/h.out"))"
    fi

    # (i) **mất dòng `# apic:`** (KT-266) — LAPIC/IOAPIC không được đọc.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^# apic: ' "$good" >"$tmp/i.log" 2>/dev/null || true
    if check_log "$tmp/i.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/i.out" 2>&1; then
        echo "not ok $n - ca âm (i): mất dòng '# apic:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (i): mất dòng '# apic:' ⇒ ĐỎ ($(head -1 "$tmp/i.out"))"
    fi

    # (j) **KT-270** — thiếu **cả** dòng `# umode:` (ảnh không `iretq` xuống ring 3 / không xử
    #     lý `int 0x80`). Đây là ca âm trực tiếp của tiêu chí "in `# umode: ring3 ran …`".
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# umode: ' "$good" >"$tmp/j.log" 2>/dev/null || true
    if check_log "$tmp/j.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/j.out" 2>&1; then
        echo "not ok $n - ca âm (j): thiếu dòng '# umode:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (j): thiếu dòng '# umode:' ⇒ ĐỎ ($(head -1 "$tmp/j.out"))"
    fi

    # (k) **KT-274** — "không có mạng": boot **thật** không có `-device virtio-net-pci` ⇒ ảnh in
    #     `# net: no device` rồi TAP 16 đỏ (không marker). Ca này đòi **cả hai** điều kiện: có
    #     dòng evidence **và** `check_log` đỏ — thiếu dòng đó nghĩa là ảnh chết trước khi tới
    #     bước mạng, và khi đó ca âm không chứng minh được gì.
    n=$((n + 1))
    cases=$((cases + 1))
    if [ -f "$LOGDIR/boot-nonet.log" ]; then
        if grep -qxF '# net: no device' "$LOGDIR/boot-nonet.log" &&
            ! check_log "$LOGDIR/boot-nonet.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/k.log" 2>&1; then
            echo "ok $n - ca âm (k): không có mạng ⇒ '# net: no device' + TAP đỏ ($(head -1 "$tmp/k.log"))"
        else
            echo "not ok $n - ca âm (k): thiếu mạng mà kiểm vẫn XANH (hoặc thiếu dòng '# net: no device')"
            fails=$((fails + 1))
        fi
    else
        echo "not ok $n - ca âm (k): không có log boot không mạng ($LOGDIR/boot-nonet.log)"
        fails=$((fails + 1))
    fi

    # (l) **KT-276** — mất **cả** ba dòng `# smp:` (ảnh không đánh thức được hart phụ / hart phụ
    # không vào long mode). Đây là ca âm trực tiếp của tiêu chí "in `# smp: hart=1 online` …".
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^# smp: ' "$good" >"$tmp/l.log" 2>/dev/null || true
    if check_log "$tmp/l.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/l.out" 2>&1; then
        echo "not ok $n - ca âm (l): mất dòng '# smp:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (l): mất dòng '# smp:' ⇒ ĐỎ ($(head -1 "$tmp/l.out"))"
    fi

    # (m) **KT-278** — mất dòng `# smp: ipi recv=` (IPI không tới hart đích). Đây là ca âm trực
    #     tiếp của tiêu chí "in `# smp: ipi recv=1 intid=0x30`": nếu runner vẫn XANH khi log không
    #     có dòng ấy thì phần IPI của bộ kiểm là dấu cao su.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^# smp: ipi recv=' "$good" >"$tmp/m.log" 2>/dev/null || true
    if check_log "$tmp/m.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/m.out" 2>&1; then
        echo "not ok $n - ca âm (m): mất dòng '# smp: ipi recv=' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (m): mất dòng '# smp: ipi recv=' ⇒ ĐỎ ($(head -1 "$tmp/m.out"))"
    fi

    # (n) **KT-286** — mất **cả** ba dòng `# sched:` (timer không chuyển ngữ cảnh / scheduler
    #     không nhường lại `main`). Đây là ca âm trực tiếp của tiêu chí "in `# sched: switch …`".
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^# sched: ' "$good" >"$tmp/n.log" 2>/dev/null || true
    if check_log "$tmp/n.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/n.out" 2>&1; then
        echo "not ok $n - ca âm (n): mất dòng '# sched:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (n): mất dòng '# sched:' ⇒ ĐỎ ($(head -1 "$tmp/n.out"))"
    fi

    # (o) **KT-288** — "không có đĩa": boot **thật** không có `-drive`/`-device virtio-blk-pci` ⇒
    #     ảnh in `# blk: no device` rồi TAP 25 đỏ (không marker). Ca này đòi **cả hai** điều kiện:
    #     có dòng evidence **và** `check_log` đỏ — thiếu dòng đó nghĩa là ảnh chết trước khi tới
    #     bước đĩa, và khi đó ca âm không chứng minh được gì.
    n=$((n + 1))
    cases=$((cases + 1))
    if [ -f "$LOGDIR/boot-nodisk.log" ]; then
        if grep -qxF '# blk: no device' "$LOGDIR/boot-nodisk.log" &&
            ! check_log "$LOGDIR/boot-nodisk.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/o.log" 2>&1; then
            echo "ok $n - ca âm (o): không có đĩa ⇒ '# blk: no device' + TAP đỏ ($(head -1 "$tmp/o.log"))"
        else
            echo "not ok $n - ca âm (o): thiếu đĩa mà kiểm vẫn XANH (hoặc thiếu dòng '# blk: no device')"
            fails=$((fails + 1))
        fi
    else
        echo "not ok $n - ca âm (o): không có log boot không đĩa ($LOGDIR/boot-nodisk.log)"
        fails=$((fails + 1))
    fi

    # (p) **KT-304** — tác vụ ring 3 **không chạy lại**: log tốt bị sửa `resumed=0` (bị preempt
    #     nhưng `schedule()` không chọn lại). Đây là ca âm trực tiếp của tiêu chí "nó chạy lại ở
    #     ring 3": nếu runner vẫn XANH khi `resumed=0` thì phép kiểm mới là dấu cao su.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^\(# sched: umode task ran .*\) resumed=[0-9][0-9]*$/\1 resumed=0/' "$good" >"$tmp/p.log" 2>/dev/null || true
    if check_log "$tmp/p.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/p.out" 2>&1; then
        echo "not ok $n - ca âm (p): tác vụ ring 3 không chạy lại mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (p): tác vụ ring 3 không chạy lại ⇒ ĐỎ ($(head -1 "$tmp/p.out"))"
    fi

    # (q) **KT-308** — preempt **không** ở ring 3: log tốt bị sửa `cs=0x1b` → `cs=0x8` ở dòng
    #     `# sched: umode counts`. Nếu runner vẫn XANH thì "ring 3" chỉ là chữ in, không phải
    #     điều kiện được kiểm.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^\(# sched: umode counts .*\) cs=0x1b /\1 cs=0x8 /' "$good" >"$tmp/q.log" 2>/dev/null || true
    if check_log "$tmp/q.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/q.out" 2>&1; then
        echo "not ok $n - ca âm (q): preempt không ở ring 3 (cs=0x8) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (q): preempt không ở ring 3 (cs=0x8) ⇒ ĐỎ ($(head -1 "$tmp/q.out"))"
    fi

    # (r) **KT-308** — bộ đếm **giảm**: log tốt bị sửa dòng `# sched: umode progress` thành
    #     `first=100 last=50 delta=-50`. Bộ đếm đơn điệu là bất biến thật; giảm ⇒ ĐỎ.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^# sched: umode progress .*$/# sched: umode progress first=100 last=50 delta=-50/' "$good" >"$tmp/r.log" 2>/dev/null || true
    if check_log "$tmp/r.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/r.out" 2>&1; then
        echo "not ok $n - ca âm (r): bộ đếm giảm (last < first) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (r): bộ đếm giảm (last < first) ⇒ ĐỎ ($(head -1 "$tmp/r.out"))"
    fi

    # (s) **KT-308** — tác vụ **chưa chạy** trước lần preempt đầu: log tốt bị sửa thành
    #     `first=0`. `first > 0` là bằng chứng "đã chạy thật"; thiếu nó thì `n > 0` có thể là
    #     bộ đếm cũ (dương tính giả) ⇒ ĐỎ.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^# sched: umode progress .*$/# sched: umode progress first=0 last=100 delta=100/' "$good" >"$tmp/s.log" 2>/dev/null || true
    if check_log "$tmp/s.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/s.out" 2>&1; then
        echo "not ok $n - ca âm (s): first=0 (chưa chạy trước preempt đầu) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (s): first=0 (chưa chạy trước preempt đầu) ⇒ ĐỎ ($(head -1 "$tmp/s.out"))"
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
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" "${QEMU_MACHINE[@]}" \
        -kernel "$IMAGE" "${QEMU_NET[@]}" "${QEMU_DISK[@]}"; then
        echo "not ok $total - boot $i: quá $((TIMEOUT_DS / 10)) s (treo, pid=$QG_LAST_PID), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    if check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$LOGDIR/log-$i.log" 2>&1; then
        echo "ok $total - boot $i: TAP $EXPECT_TESTS/$EXPECT_TESTS + $EXPECT_EXC dòng '# exc:' + marker 1 lần + halt sạch"
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
        if ! qg_boot "$LOGDIR/boot-bad.log" "$TIMEOUT_DS" "$QEMU" "${QEMU_MACHINE[@]}" \
            -kernel "$BAD_IMAGE" "${QEMU_NET[@]}" "${QEMU_DISK[@]}"; then
            echo "# cảnh báo: ảnh marker sai không thoát (pid=$QG_LAST_PID) — ca âm (b) vẫn dùng log"
        fi
    fi
else
    echo "# cảnh báo: build ảnh marker sai FAIL (xem $LOGDIR/build-bad.log)"
fi

# ---- ca âm (k) **KT-274**: boot **thật** không có mạng. `-nic none` tắt cả NIC mặc định của
#      QEMU (q35 tự thêm e1000 khi không có `-net*`), nên đây đúng là "không có thiết bị mạng".
#      Ảnh in `# net: no device` rồi TAP 16 đỏ ⇒ `cli; hlt` (QEMU không tự thoát) nên dùng ngân
#      sách riêng ngắn hơn.
echo "# boot không mạng (ca âm k) — -nic none, không có thiết bị mạng"
if ! qg_boot "$LOGDIR/boot-nonet.log" "$NONET_TIMEOUT_DS" "$QEMU" "${QEMU_MACHINE[@]}" \
    -nic none -kernel "$IMAGE" "${QEMU_DISK[@]}"; then
    echo "# cảnh báo: boot không mạng không tự thoát (pid=$QG_LAST_PID) — ca âm (k) vẫn dùng log"
fi

# ---- ca âm (o) **KT-288**: boot **thật** không có đĩa. Mạng vẫn có để ảnh chạy qua TAP 15..23;
#      tới bước blk thì `pci::init_for(VirtioKind::Blk)` trả `NoDevice` ⇒ ảnh in
#      `# blk: no device` rồi TAP 25 ĐỎ ⇒ `cli; hlt` (QEMU không tự thoát) nên dùng ngân sách
#      riêng ngắn hơn. Đây là ca âm **thật** (boot QEMU lần hai, không `-drive`), không phải biến
#      đổi log: nó chứng minh driver **fail-closed** khi thiếu thiết bị, thay vì đọc ra số 0 rồi
#      in `ok`.
echo "# boot không đĩa (ca âm o) — không có -drive/-device virtio-blk-pci"
if ! qg_boot "$LOGDIR/boot-nodisk.log" "$NODISK_TIMEOUT_DS" "$QEMU" "${QEMU_MACHINE[@]}" \
    -kernel "$IMAGE" "${QEMU_NET[@]}"; then
    echo "# cảnh báo: boot không đĩa không tự thoát (pid=$QG_LAST_PID) — ca âm (o) vẫn dùng log"
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
    total=$((total + 16))
fi

echo "# $((total - fails))/$total kiểm sạch (boot=$N_BOOT, tests=$EXPECT_TESTS, exc=$EXPECT_EXC)"
echo "# infra: guard_log=$QG_LOG qemu_own=$(qg_own)"
[ "$fails" -eq 0 ] || exit 1
exit 0
