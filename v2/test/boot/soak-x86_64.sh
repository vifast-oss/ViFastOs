#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-293 — soak boot ảnh **x86_64** K5' (tiêu chí "**như K3**" của K5': soak **600** boot, D2-149).
#
# Khuôn: `soak-aarch64.sh` (KT-265) + `soak.sh` (riscv64, KT-57). **Cùng bất biến, không nới một
# ngưỡng nào**:
#   - D2-24 + QD §5.3: máy `ps`, **3 luồng song song**, `n ≥ 3/p`; 0 lỗi/N; 0/N chỉ là **trần**,
#     không phải bảo đảm.
#   - **K5' chốt N = 600** (D2-149: "N1 đóng khi K5' có 600 boot thật"). Với N = 600, bất biến
#     `n ≥ 3/p` (D2-24) đòi **p = 0,5 %** (3/600) ⇒ `P_NUM=5`, `P_DEN=1000`, `NEED = 600`. Đây
#     **không** phải nới ngưỡng: N do chính K5' quy định; K3/K4' dùng p = 0,3 % ⇒ N = 1.000, còn
#     K5' dùng p = 0,5 % ⇒ N = 600. Script **từ chối** N < 600 ở chế độ cổng (exit 2).
#   - D2-72 Q10: timeout **20 s/boot**, chỉ `kill` PID do chính script tạo (AGENTS §9),
#     BUILD_ID = git sha + sha256 ảnh, ghi qemu version + cấu hình; **không retry** (D2-07).
#   - D2-74 điều kiện (c): soak **KHÔNG** `-icount`.
#   - D2-80: mỗi log boot phải có **hash ảnh do kernel tự in** (`# conf image: build=<16 hex>
#     text=<len>`, FNV-1a 64 trên `.text` đã nạp); script băm lại **cùng** section từ ELF
#     (`test/conformance/image_digest.py`) và **từ chối** log không có/lệch hash.
#   - D2-08: ảnh sản phẩm KHÔNG chứa chuỗi `ktest` (kiểm trước khi soak, như `run.sh`).
#
# Bất biến của MỖI boot (fail-closed, không nới):
#   1. đúng 1 `TAP version 13`; đúng 1 `1..36`; đúng **36** dòng `ok N - `, đủ số 1..36,
#      **không** dòng `not ok` nào (KT-261 7 ca exception + KT-266 4 ca APIC/timer + KT-270 3 ca
#      U-mode + KT-274 3 ca PCI/net + KT-278 4 ca SMP + KT-286 2 ca preemption + KT-304 1 ca tác
#      vụ ring 3 + KT-310 1 ca **nhiều hart** + KT-288 2 ca virtio-blk + KT-316 1 ca khoá COM1 +
#      KT-318 2 ca scheduler (priority tĩnh + sleep/wake) + **KT-326 1 ca DHCP** +
#      **KT-328 2 ca mạng (ICMP + DNS)** + **KT-336a 1 ca khoá MCS bọc IRQ** = 34);
#   2. đúng 1 `# viFastOs x86_64: boot ok` (marker **nguyên văn**, in sau khi mọi TAP xanh);
#   3. đúng 1 `# conf image: build=<16 hex> text=<len>`, build/text **khớp ELF** (D2-80);
#   4. **KT-261** — đúng **6** dòng `# exc: vec=… err=… rip=… cs=0x8 rflags=…`: `#BP` 3/err 0,
#      `#UD` 6/err 0, `#PF` 14/err 0 + `# exc: cr2=0x700000000000`, `#GP` 13/err 0x38,
#      isr_unknown 254/err 0, và **`#PF` của ca W^X** 14/err **0x3** (`# exc: cr2=<địa chỉ .text>`),
#      `isr_unknown` 254/err 0. Ảnh không chạy handler exception ⇒ ĐỎ dù TAP 1..7 xanh;
#   5. **KT-266** — `# apic: id=… version=0x… svr=0x…` + `# apic: ioapic id=… ver=0x… entries=…`
#      + `# timer: hz=… ticks=…` (ticks > 0) + `# timer: eoi=… ticks=…` (ticks > 0);
#   6. **KT-274** — `# pci: ecam base=0x… buses=… functions=<n>` (ECAM thật, không legacy) +
#      `# net: mac=<6 octet> …` (MAC đọc từ thiết bị) + `# net: arp gw=<ip> mac=<mac> sent=<n>
#      recv=<n>` (ARP reply **thật** của slirp, sent/recv ≥ 1) + **KT-326** `# net: dhcp
#      lease=<secs> ip=10.0.2.15 mask=255.255.255.0 gw=10.0.2.2 dns=10.0.2.3
#      server=10.0.2.2 attempts=<n> elapsed_us=<n> filter=cookie:1 xid:1` (lease **thật** của
#      slirp qua DISCOVER→OFFER→REQUEST→ACK; bộ lọc đã chặn cookie sai **và** xid lạ) + TAP 31 in
#      **cùng** lease/ip/gw/dns + **KT-328** `# net: icmp target=10.0.2.2 reply=1 id=0x4b54
#      seq=1 payload=32 rtt_ms=<n> attempts=<n> elapsed_ms=<n> filter=id:1 seq:1 payload:1` (echo
#      **thật** của slirp, `id`/`seq` khớp, bộ lọc chặn reply bị đột biến) + `# net: dns
#      name=localhost a=127.0.0.1 ttl=<n> id=0x4b55 rcode=0 an=1 reply_len=<n> rtt_ms=<n>
#      elapsed_ms=<n> attempts=<n> const:timeout_ms=3000 filter=id:1 qr:1 rcode:1` (truy vấn `A`
#      **thật** tới `10.0.2.3:53`, `RCODE=0`, địa chỉ **đúng hằng** `127.0.0.1`);
#   7. **KT-270** — đúng **một** dòng `# umode: ring3 ran nr=12 ret=<len>` với `ret > 0`: ring 3
#      **thật sự** chạy và `int 0x80` được xử lý. Ảnh chỉ chạy CPL0 **không** có dòng này ⇒ ĐỎ;
#   8. **KT-288** — `# blk: device … vendor=0x1af4 … bar_distinct=1 capacity=8 … status=0x0f` +
#      `# blk: sector0 hex=<32 hex> … magic=1 … errors=0 timeouts=0`; hex phải khớp **16 byte đầu
#      của đĩa tạm** (script băm lại từ tệp bằng `od`) ⇒ "đọc được sector 0" không thể là đọc ra
#      số 0 rồi tự khen;
#   9. **KT-278** — `# smp: hart=1|2|3 online` (ba hart phụ vào long mode), `# smp: hart=0|1|2|3
#      alive`, `# smp: tss bases=[…] … distinct=Ok(()) … tr=0x28 type=0x8b ok=1` (TSS per-hart),
#      `# smp: ipi recv=1 intid=0x30` (IPI tới hart 1), `# smp: online mask=0xf alive=0xf expect=0xf
#      harts=4` (bốn hart cùng online **và** cùng báo alive);
#  10. **KT-286** — `# sched: switch a→b n=<k>`, `# sched: switch b→a n=<k>` (timer chuyển ngữ
#      cảnh **cả hai** chiều) và `# sched: ticks=<t> tasks=4` (scheduler nhường lại `main`);
#  10b. **KT-304** — `# sched: umode task ran n=<n> ring=3 preempted=<n> resumed=<n>`: tác vụ
#      **ring 3** đã chạy thật (`n > 0`), bị preempt khi đang ở CPL3 (`ring=3`), được
#      `schedule()` chọn **lại** (`resumed ≥ 1`) — thiếu dòng ⇒ ĐỎ;
#  10c. **KT-310**/**KT-315** (sửa **KT-313**/**KT-317**) — `# sched: hart=0|1|2|3 picked=<task>
#      n=<k>` (mỗi hart **một** dòng) **và** `# sched: multi-hart harts=4 rounds=<r>
#      picks=[<a>, <b>, <c>, <d>] first=[<w>, <x>, <y>, <z>] diff=<n> double=0`: **bốn** hart cùng
#      chọn từ **một** hàng đợi toàn cục, mọi hart chọn ≥ 1, **có** lượt hart phụ chọn tác vụ
#      **khác** tác vụ BSP **đang chạy** (`diff ≥ 1`), **không** double-pick — thiếu/lệch ⇒ ĐỎ.
#      **KT-313** bỏ `first[0] != first[i]` của KT-310/KT-315: nó **phụ thuộc thời điểm** (hàng
#      đợi FIFO **bốn** tác vụ quay vòng ⇒ hart phụ kịp **bốn** lượt trước lượt đầu của BSP thì
#      `first=[a, a, …]` dù `double=0`); soak KT-313 đo **5/600** boot đỏ vì nó — cùng lớp lỗi
#      `last > first` của KT-308. `first=[…]` vẫn in để **chẩn đoán**, **không** phải điều kiện;
#  10d. **KT-316** — đúng **một** dòng `# com: lock ok=1 contended=<n>`: khoá COM1 **đã được
#      dùng** (`ok=1` đòi `acquired > 0` **và** `overlap = 0`). Dòng bị **xen ký tự** (không
#      khoá) không khớp mẫu ⇒ ĐỎ;
#  10e. **KT-318** — priority **tĩnh** + sleep/wake: đúng **một** dòng `# sched: prio first=b
#      viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]` (lượt chọn **đầu** là tác vụ ưu tiên cao nhất —
#      FIFO sẽ là `a` — **không** vi phạm, pha kết thúc **bình thường**) **và** đúng **một** dòng
#      `# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]` (**cả bốn** tác vụ đã thức, kể
#      cả `u` với `sleep(0)`; **không** lượt nào chọn tác vụ đang ngủ; mỗi tác vụ chạy → ngủ →
#      chạy lại). Hai giá trị là **hằng** vì pha là **hàm thuần của tick** — **không** đòi một
#      cửa sổ thời gian nào (bài học KT-308/KT-313, đã lặp ba lần trong tuần);
#  11. `lma=1` (long mode **thật**, đọc lại `IA32_EFER.LMA`) + đúng 1 `# viFastOs x86_64: halt`
#      (thoát sạch, không triple-fault);
#  12. **không** dòng `PANIC` nào (tính cả dòng thụt đầu dòng — fail-closed);
#  13. quá **20 s** ⇒ `kill` đúng PID, boot đó **đỏ** (không retry).
#
# **Vì sao ảnh x86_64 phải in `# conf image:` (KT-293)**: D2-80 áp cho **mọi** ảnh, nhưng
# KT-257..KT-288 chỉ in dòng ấy trên riscv64/aarch64. KT-293 thêm `vifast_plat_x86_64::image`
# (FNV-1a 64 trên `[__text_start, __text_end)`, khuôn `vifast_plat_aarch64::image` của KT-265) và
# in ngay sau plan TAP. Không có dòng ấy thì bất biến "hash ảnh khớp ELF mỗi boot" **không đo
# được** ⇒ script sẽ TỪ CHỐI ĐO (exit 3) thay vì đo ảnh không rõ nguồn.
#
# Dùng:
#   bash v2/test/boot/soak-x86_64.sh [NBOOT] [LANES]     # mặc định 600 3 (đúng cổng K5')
#   bash v2/test/boot/soak-x86_64.sh --smoke N [LANES]   # N < 600: chỉ để đỏ trước/kiểm marker;
#                                                         # LUÔN exit 1 (không phải bằng chứng cổng)
#   bash v2/test/boot/soak-x86_64.sh --selftest          # kiểm bộ đọc log trên log tổng hợp:
#                                                         # 2 ca dương + 47 ca âm, KHÔNG mở QEMU
#   bash v2/test/boot/soak-x86_64.sh --help
#
# Biến môi trường: `SOAK_X86_64_LOGDIR`/`KT_LOGDIR` (mặc định `$V2_RUN_TMP` hoặc
#   `$TMPDIR/k5-x86_64-soak.<pid>`);
#   `V2_X64_CPU` — model CPU của QEMU cho **mọi** boot (mặc định `qemu64` = model QEMU chọn cho
#   `-M q35`, giữ nguyên để số đo cũ **tái lập được**; `max` = tập tính năng rộng nhất, **KT-323**).
#   Model đã dùng in ở dòng `# cấu hình:` và `# infra:`.
#
# Exit: 0 = 0 lỗi/N (chỉ ở chế độ cổng, N ≥ 600); 1 = có boot đỏ / ảnh đổi giữa soak;
#       2 = lỗi cấu hình (thiếu tool, N < 600, LANES ngoài [1,3], build hỏng);
#       3 = TỪ CHỐI ĐO (ảnh không in `# conf image:` khớp ELF) — 0 boot được đếm.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
# **KT-333d (D2-248 mục 2b)** — LOGDIR (và SOCK ngắn) dùng chung `v2/test/boot/lib/env.sh`
# (KT-333c đã làm logdir theo phiên inline ở 6 runner; nay gom một chỗ, không lặp code).
. "$HERE/lib/env.sh"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
TARGET="x86_64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-x86_64"
# **KT-340a** (D2-08) — soak chạy **ảnh ktest** (hợp đồng TAP 34 của soak là hợp đồng của ảnh
# kiểm); ảnh **sản phẩm** được build **không** feature `ktest` và kiểm riêng (0 harness marker +
# ngân sách) ở đầu soak. Hai bản sao trong `$LOGDIR` để không đè nhau trong `target/`.
# **KT-333c** (D2-243 mục 3b) — logdir **riêng theo phiên**: `$V2_RUN_TMP` hoặc
# `$TMPDIR/k5-x86_64-soak.<pid>`; ép bằng `KT_LOGDIR`/`SOAK_X86_64_LOGDIR`.
LOGDIR="$(v2_env_logdir k5-x86_64-soak SOAK_X86_64_LOGDIR)"
if [ "${1:-}" = "--print-logdir" ]; then
    echo "$LOGDIR"
    exit 0
fi
# **KT-340a** (D2-08) — đường dẫn **hai** ảnh (sau khi có `$LOGDIR`): sản phẩm (không feature) và
# ktest (`--features ktest`). Hai bản sao để build không đè nhau trong `target/`.
PRODUCT_IMAGE="$LOGDIR/vifast-kbin-x86_64-product"
KTEST_IMAGE="$LOGDIR/vifast-kbin-ktest-x86_64"
PRODUCT_HARNESS_MARKERS=(
    'TAP version 13' 'not ok' '# TAP ' 'ok 1 - ' 'ktest'
    '# smp:' '# net:' '# umode:' '# sched:' '# blk:' '# lock:' '# apic:' '# timer:' '# wx:'
)
QEMU_MIN_MAJOR=9 # cùng sàn với riscv64/aarch64 (D2-72 Q2): QEMU >= 9.2
QEMU_MIN_MINOR=2
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
P_NUM=5        # p = 0,5 % = 5/1000 — K5' chốt N = 600 (D2-149) ⇒ n ≥ 3/p = 600 (D2-24)
P_DEN=1000
TAP_TESTS=43   # KT-340c thêm 6 ca core (38..43); KT-261 7 + KT-266 4 + KT-270 3 + KT-274 3 + KT-278 4 + KT-286 2 + KT-304 1 + KT-310 1 + KT-288 2 + KT-316 1 + KT-318 2 + KT-326 1 + KT-328 2 + KT-336a 1 + KT-340a 2 + KT-340d 1
EXPECT_EXC=6   # KT-261: #BP, #UD, #PF, #GP, isr_unknown + #PF (P|W) của ca W^X
EXPECT_PF_ADDR=0x700000000000
MARKER="# viFastOs x86_64: boot ok"
HALT_LINE="# viFastOs x86_64: halt"
# **KT-288** — hex 16 byte đầu của sector 0. Giá trị dưới đây là của chữ ký
# `VIFASTOS-KT288-S0` (dùng cho `--selftest`, không có đĩa thật); đường chạy thật **ghi đè**
# bằng `od` trên đĩa tạm.
WANT_SECTOR0_HEX=5649464153544f532d4b543238382d53
WANT_CAPACITY=8
SMOKE=0
SELFTEST=0

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "soak-x86_64.sh: $*" >&2
    exit 2
}

usage() {
    sed -n 's/^# \{0,1\}//; 2,80p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,80p'
    echo "soak-x86_64.sh: dùng: soak-x86_64.sh [NBOOT] [LANES] | soak-x86_64.sh --smoke N [LANES] | soak-x86_64.sh --selftest | soak-x86_64.sh --help"
}

# ------------------------------------------------------------------ bộ đọc log (một boot)
# `check_boot_log <log> <want_build> <want_text>`: 0 ⇔ log đạt **mọi** bất biến ở đầu tệp.
# In lý do đỏ ra stdout (dòng `#`). Dùng **cùng** hàm cho soak thật và cho `--selftest` ⇒ không có
# đường thứ hai lệch nhau về kỳ vọng.
check_boot_log() {
    local log="$1" want_build="$2" want_text="$3" bad=0 n got_build got_text i line

    [ -f "$log" ] || {
        echo "# không có log $log"
        return 1
    }

    # (1) khung TAP — đúng **một** dòng mỗi mẫu, đủ số 1..$TAP_TESTS, không `not ok`.
    n="$(grep -c -x -F 'TAP version 13' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# 'TAP version 13' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -c -x -F "1..$TAP_TESTS" "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# plan '1..$TAP_TESTS' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -c '^ok [0-9][0-9]* - ' "$log" || true)"
    [ "${n:-0}" -eq "$TAP_TESTS" ] || {
        echo "# có ${n:-0} dòng 'ok', cần $TAP_TESTS"
        bad=1
    }
    n="$(grep -c '^not ok ' "$log" || true)"
    [ "${n:-0}" -eq 0 ] || {
        echo "# có ${n:-0} dòng 'not ok'"
        bad=1
    }
    i=1
    while [ "$i" -le "$TAP_TESTS" ]; do
        grep -q "^ok $i - " "$log" || {
            echo "# thiếu 'ok $i - ...'"
            bad=1
        }
        i=$((i + 1))
    done

    # (2) marker **nguyên văn**, đúng **một** lần (0 = ảnh không báo boot; 2 = in lặp).
    n="$(grep -Fxc "$MARKER" "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# marker '$MARKER' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (3) D2-80 — hash ảnh do kernel tự in, khớp ELF đang chạy.
    n="$(grep -cE '^# conf image: build=[0-9a-f]{16} text=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# conf image:' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    got_build="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$log" | head -1)"
    got_text="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$log" | head -1)"
    [ "$got_build" = "$want_build" ] || {
        echo "# image-build=${got_build:-none} lệch ELF ($want_build)"
        bad=1
    }
    [ "$got_text" = "$want_text" ] || {
        echo "# image-text=${got_text:-none} lệch ELF ($want_text)"
        bad=1
    }

    # (4) **KT-261** — đúng 5 exception, đúng vector + mã lỗi + `cs=0x8`, `#PF` có `cr2` đúng.
    n="$(grep -c '^# exc: vec=' "$log" || true)"
    [ "${n:-0}" -eq "$EXPECT_EXC" ] || {
        echo "# có ${n:-0} dòng '# exc: vec=', cần $EXPECT_EXC"
        bad=1
    }
    grep -qE '^# exc: vec=3 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=3 err=0x0 … cs=0x8' (int3)"
        bad=1
    }
    grep -qE '^# exc: vec=6 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=6 err=0x0 … cs=0x8' (ud2)"
        bad=1
    }
    grep -qE '^# exc: vec=14 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=14 err=0x0 … cs=0x8' (page fault)"
        bad=1
    }
    grep -qxF "# exc: cr2=$EXPECT_PF_ADDR" "$log" || {
        echo "# thiếu '# exc: cr2=$EXPECT_PF_ADDR' (CR2 của #PF)"
        bad=1
    }
    grep -qE '^# exc: vec=13 err=0x38 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=13 err=0x38 … cs=0x8' (gp)"
        bad=1
    }
    grep -qE '^# exc: vec=254 err=0x0 rip=0x[0-9a-f]+ cs=0x8 rflags=0x[0-9a-f]+$' "$log" || {
        echo "# thiếu '# exc: vec=254 err=0x0 … cs=0x8' (isr_unknown)"
        bad=1
    }

    # (5) **KT-266** — APIC + timer: bốn dòng bằng chứng, mỗi dòng một lớp lỗi khác nhau.
    n="$(grep -cE '^# apic: id=[0-9]+ version=0x[0-9a-f]+ svr=0x[0-9a-f]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# apic: id=… version=0x… svr=0x…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# apic: ioapic id=[0-9]+ ver=0x[0-9a-f]+ entries=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# apic: ioapic id=… ver=0x… entries=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# timer: hz=[0-9]+ ticks=[0-9]+' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# timer: hz=… ticks=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# timer: hz=' "$log" | grep -qE 'ticks=0( |$)' && {
        echo "# timer ticks=0 (timer không chạy)"
        bad=1
    }
    n="$(grep -cE '^# timer: eoi=[0-9]+ ticks=[0-9]+' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# timer: eoi=… ticks=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# timer: eoi=' "$log" | grep -qE 'ticks=0( |$)' && {
        echo "# timer eoi ticks=0 (handler không đếm tick)"
        bad=1
    }

    # (6) **KT-274** — PCI ECAM + virtio-net + ARP: ba dòng bằng chứng.
    n="$(grep -cE '^# pci: ecam base=0x[0-9a-f]+ buses=[0-9]+\.\.[0-9]+ functions=[1-9][0-9]*$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# pci: ecam base=0x… buses=… functions=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net: mac=([0-9a-f]{2}:){5}[0-9a-f]{2} ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: mac=aa:bb:cc:dd:ee:ff …' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net: arp gw=[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+ mac=([0-9a-f]{2}:){5}[0-9a-f]{2} sent=[1-9][0-9]* recv=[1-9][0-9]*' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: arp gw=… mac=… sent=… recv=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    # **KT-326** — DHCP lease **thật** + bộ lọc chặn **cả** cookie sai **và** xid lạ; TAP 31 in
    # cùng lease/ip/gw/dns (nhãn TAP cũng là bằng chứng, không chỉ số hiệu).
    n="$(grep -cE '^# net: dhcp lease=[1-9][0-9]* ip=10\.0\.2\.15 mask=255\.255\.255\.0 gw=10\.0\.2\.2 dns=10\.0\.2\.3 server=10\.0\.2\.2 attempts=[1-6] elapsed_us=[0-9]+ filter=cookie:1 xid:1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: dhcp lease=… ip=10.0.2.15 … filter=cookie:1 xid:1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 31 - net: dhcp lease=[1-9][0-9]* ip=10\.0\.2\.15 gw=10\.0\.2\.2 dns=10\.0\.2\.3$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 31 - net: dhcp lease=… ip=10.0.2.15 gw=10.0.2.2 dns=10.0.2.3' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    # **KT-328** — ICMP echo **thật** (reply của slirp) + DNS `A` **thật** với địa chỉ **đúng hằng**
    # `127.0.0.1`; cả hai kèm phép tự kiểm bộ lọc (`filter=…:1`) nên đột biến bỏ một phép kiểm
    # làm dòng không khớp ⇒ ĐỎ. Cùng khuôn đếm "đúng 1 lần" như DHCP: một dòng hai lần là log bị
    # ghép, không phải bằng chứng.
    n="$(grep -cE '^# net: icmp target=10\.0\.2\.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=[0-9]+ attempts=[1-3] elapsed_ms=[0-9]+ filter=id:1 seq:1 payload:1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 … filter=id:1 seq:1 payload:1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 32 - net: icmp echo reply target=10\.0\.2\.2 id=0x4b54 seq=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 32 - net: icmp echo reply target=10.0.2.2 id=0x4b54 seq=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net: dns name=localhost a=127\.0\.0\.1 ttl=[0-9]+ id=0x4b55 rcode=0 an=1 reply_len=[0-9]+ rtt_ms=[0-9]+ elapsed_ms=[0-9]+ attempts=[1-2] const:timeout_ms=3000 filter=id:1 qr:1 rcode:1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: dns name=localhost a=127.0.0.1 … rcode=0 … filter=id:1 qr:1 rcode:1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 33 - net: dns a=127\.0\.0\.1 rcode=0$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 33 - net: dns a=127.0.0.1 rcode=0' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (7) **KT-270** — bằng chứng U-mode: đúng **một** dòng `# umode: ring3 ran nr=12 ret=<len>`
    #     với `ret > 0`. Dòng này chỉ in khi ring 3 **thật sự** chạy và handler `int 0x80` đọc
    #     được số hiệu từ `rax` của ring 3; ảnh chỉ chạy CPL0 **không** có ⇒ ĐỎ.
    n="$(grep -cE '^# umode: ring3 ran nr=[0-9]+ ret=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# umode: ring3 ran nr=<n> ret=<v>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -qE '^# umode: ring3 ran nr=12 ret=[1-9][0-9]*$' "$log" || {
        echo "# dòng '# umode: ring3 ran nr=12 ret=<len>' không đúng (nr phải 12, ret > 0)"
        bad=1
    }

    # (8) **KT-288** — virtio-blk: thiết bị thật + sector 0 khớp **16 byte đầu của đĩa tạm**.
    n="$(grep -cE '^# blk: device ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# blk: device ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    line="$(grep -E '^# blk: device ' "$log" || true)"
    for want in "vendor=0x1af4" "bar_distinct=1" "capacity=$WANT_CAPACITY " "status=0x0f"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# blk: device' không có '$want'"
                bad=1
                ;;
        esac
    done
    n="$(grep -cE '^# blk: sector0 hex=[0-9a-f]{32} ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# blk: sector0 hex=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    line="$(grep -E '^# blk: sector0 ' "$log" || true)"
    for want in "hex=$WANT_SECTOR0_HEX" "magic=1" "errors=0" "timeouts=0"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# blk: sector0' không có '$want'"
                bad=1
                ;;
        esac
    done

    # (9) **KT-278** — SMP 4 hart + IPI + TSS per-hart.
    for h in 1 2 3; do
        n="$(grep -Fxc "# smp: hart=$h online" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '# smp: hart=$h online' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    for h in 0 1 2 3; do
        n="$(grep -Fxc "# smp: hart=$h alive" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '# smp: hart=$h alive' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    n="$(grep -cE '^# smp: tss bases=\[0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+\] ist1=\[0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+, 0x[0-9a-f]+\] distinct=Ok\(\(\)\) ist1_distinct=Ok\(\(\)\) tr=0x28 type=0x8b ok=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# smp: tss bases=[…] … distinct=Ok(()) … tr=0x28 type=0x8b ok=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -Fxc '# smp: ipi recv=1 intid=0x30' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# smp: ipi recv=1 intid=0x30' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# smp: online mask=0xf alive=0xf expect=0xf harts=4 ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# smp: online mask=0xf alive=0xf expect=0xf harts=4 …' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (10) **KT-286** — preemption: chuyển **cả hai** chiều + scheduler nhường lại `main`.
    n="$(grep -cE '^# sched: switch a→b n=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: switch a→b n=<k>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# sched: switch b→a n=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: switch b→a n=<k>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# sched: ticks=[0-9]+ tasks=4$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: ticks=<t> tasks=4' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# sched: ticks=' "$log" | grep -qE 'ticks=0 ' && {
        echo "# sched ticks=0 (timer không chạy trong lúc scheduler bật)"
        bad=1
    }
    # **KT-304** — tác vụ ring 3: chạy thật, bị preempt khi ở CPL3, được chọn lại.
    n="$(grep -cE '^# sched: umode task ran n=[1-9][0-9]* ring=3 preempted=[2-9][0-9]* resumed=[1-9][0-9]*$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: umode task ran n=… ring=3 preempted=… resumed=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (10c) **KT-310**/**KT-315** (sửa **KT-313**/**KT-317**) — **nhiều hart** schedule: mỗi hart
    # một dòng `picked=`, cộng dòng tổng hợp có `harts=4` + `diff=<n>` + `double=0`; **mọi** hart
    # phải chọn ≥ 1 và phải **có** lượt hart phụ chọn tác vụ **khác** tác vụ BSP **đang chạy**
    # (`diff ≥ 1`), **không** double-pick.
    #
    # **KT-313** — bỏ điều kiện `first[0] != first[i]` của KT-310/KT-315: nó **phụ thuộc thời
    # điểm** (hàng đợi FIFO **bốn** tác vụ quay vòng ⇒ hart phụ kịp **bốn** lượt trước lượt đầu
    # của BSP thì tác vụ `a` về lại đầu hàng đợi ⇒ `first=[a, a, …]` dù `double=0`). Soak KT-313
    # đo **5/600** boot đỏ vì điều kiện ấy — cùng lớp lỗi `last > first` của KT-308. `first=[…]`
    # vẫn in để **chẩn đoán**, **không** phải điều kiện.
    for h in 0 1 2 3; do
        n="$(grep -cE "^# sched: hart=$h picked=[abuc] n=[0-9]+$" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '# sched: hart=$h picked=<task> n=<k>' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    mh_sum="$(grep -E '^# sched: multi-hart harts=4 rounds=[0-9]+ picks=\[[0-9]+, [0-9]+, [0-9]+, [0-9]+\] first=\[[abuc], [abuc], [abuc], [abuc]\] diff=[0-9]+ double=[0-9]+$' "$log" || true)"
    if [ -z "$mh_sum" ]; then
        echo "# thiếu '# sched: multi-hart harts=4 … picks=[…] first=[…] diff=<n> double=<n>' (không có bằng chứng bốn hart)"
        bad=1
    else
        mh_p1="$(printf '%s\n' "$mh_sum" | sed -n 's/.*picks=\[[0-9]*, \([0-9]*\), [0-9]*, [0-9]*\].*/\1/p')"
        mh_p2="$(printf '%s\n' "$mh_sum" | sed -n 's/.*picks=\[[0-9]*, [0-9]*, \([0-9]*\), [0-9]*\].*/\1/p')"
        mh_p3="$(printf '%s\n' "$mh_sum" | sed -n 's/.*picks=\[[0-9]*, [0-9]*, [0-9]*, \([0-9]*\)\].*/\1/p')"
        for pair in "1:$mh_p1" "2:$mh_p2" "3:$mh_p3"; do
            [ "${pair#*:}" -ge 1 ] || {
                echo "# multi-hart picks[${pair%%:*}]=${pair#*:} — hart ${pair%%:*} chưa chọn tác vụ nào"
                bad=1
            }
        done
        mh_diff="$(printf '%s\n' "$mh_sum" | sed -n 's/.*diff=\([0-9]*\) double=.*/\1/p')"
        [ "${mh_diff:-0}" -ge 1 ] || {
            echo "# multi-hart diff=${mh_diff:-0} — hart phụ chưa chọn tác vụ **khác** tác vụ BSP đang chạy"
            bad=1
        }
        mh_dbl="$(printf '%s\n' "$mh_sum" | sed -n 's/.*double=\([0-9]*\)$/\1/p')"
        [ "${mh_dbl:-0}" -eq 0 ] || {
            echo "# multi-hart double=${mh_dbl:-0} — có double-pick (hai hart cùng một tác vụ)"
            bad=1
        }
    fi

    # (10d) **KT-316** — khoá COM1: đúng **một** dòng `# com: lock ok=1 contended=<n>`. `ok=1`
    # đòi khoá **đã được dùng** và **chưa từng** chồng lấn; dòng bị xen ký tự không khớp mẫu.
    n="$(grep -cE '^# com: lock ok=1 contended=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# com: lock ok=1 contended=<n>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (10f) **KT-336a** — khoá MCS bọc lưu/tắt ngắt: **loại trừ hai ngữ cảnh** (`denied=1`,
    # `overlap=0`, `in_cs=0`) + **ngắt bị hoãn** (`during == before`, `delivered=1`, `after >
    # before`, `if_inside=0`). Hai dòng đo hai bất biến độc lập; thiếu một dòng ⇒ ĐỎ.
    n="$(grep -cE '^# lock: mcs acquired=[1-9][0-9]* contended=[0-9]+ denied=1 overlap=0 in_cs=0 ok=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# lock: mcs acquired=… denied=1 overlap=0 in_cs=0 ok=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# lock: irq deferred=1 delivered=1 before=[0-9]+ during=[0-9]+ after=[0-9]+ if_before=1 if_inside=0 ok=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# lock: irq deferred=1 delivered=1 … if_inside=0 ok=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 34 - lock: mcs irq guard$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 34 - lock: mcs irq guard' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    # **KT-340a** — W^X kernel: bằng chứng + hai TAP (35/36).
    n="$(grep -cE '^# wx: cr0_wp=1 text_end=0x[0-9a-f]+ .* wx_pages=[0-9]+ pt=0x[0-9a-f]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# wx: cr0_wp=1 … wx_pages=… pt=0x…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# wx: text write addr=0x[0-9a-f]+ err=0x3 cr2=0x[0-9a-f]+ unchanged=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# wx: text write … err=0x3 … unchanged=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 35 - wx: kernel sections$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 35 - wx: kernel sections' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 36 - wx: text write faults$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 36 - wx: text write faults' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # **KT-340d** — TLB shootdown đa hart + `IpiMailbox` 3/3 port (TAP 37): hart 1 đọc lại VA sau
    # `TlbFlush` phải thấy mapping **mới** (`remap_obs` = frame B), và BSP phải phục vụ `TlbFlush`
    # của hart 1 **trong lúc chờ ACK** (`own_wait=acked`) — đúng lỗi v1 **N5**. Ba dòng, mỗi dòng
    # **đúng một** lần: "đã gửi" và "đã ACK" là hai số khác nhau nên không thể xanh nhờ một dòng.
    n="$(grep -cE '^# tlb: shootdown ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# tlb: shootdown ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# tlb: shootdown .*stale=0xa1a1111122223333 remap_obs=0xb2b2444455556666 .*chain_read=0xd4d4aaaabbbbcccc .*peer_wait=acked own_wait=acked remap_ok=1 chain_ok=1 restored=1 ok=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# tlb: shootdown … remap_obs=<mới> … own_wait=acked … ok=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^ok 37 - tlb: shootdown ap->bsp$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng 'ok 37 - tlb: shootdown ap->bsp' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# ipi: mailbox posted=[1-9][0-9]* coalesced=[0-9]+ sent=[1-9][0-9]* drained=[1-9][0-9]* empty=[0-9]+ acked=[1-9][0-9]* invalid=0 local=[0-9]+ timeouts=0 busy=[0-9]+ vector=0x30 target=1 online=0x[0-9a-f]+ ok=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# ipi: mailbox posted=… sent=… acked=… invalid=0 timeouts=0 vector=0x30 ok=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# ipi: pcid supported=[01] pge_supported=[01] cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# ipi: pcid … cr4_pcid=0 cr4_pge=0 full_flush=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # **KT-340c** — `vifast-core` trong ảnh + `7C-r13` + sáu TAP core (38..43). Ba dòng bằng chứng
    # dưới đây là ba bất biến **độc lập**: (i) `Machine` nằm trong `.bss` ảnh và qua pin D2-84;
    # (ii) `MessageInfo` trả về ở `r13` (kernel đối chiếu vùng nhận + mã thoát ring 3); (iii)
    # `RFLAGS.IF` tắt trong vùng găng và **khôi phục** sau khi nhả khoá.
    n="$(grep -cE '^# core: addr=0x[0-9a-f]+ size=[0-9]+ pin=196608 in_image=1 harts=[1-8] objects=[0-9]+ caps=[0-9]+ healthy=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# core: … pin=196608 in_image=1 … healthy=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# core 7c-r13: recv_err=0 exit=0x4 info_seen_expected=0x80000341 mr3_expected=0x34100003 area_info=0x80000341 area_mr3=0x34100003$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# core 7c-r13: … area_info=0x80000341 area_mr3=0x34100003' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# core lock: irq_before=1 irq_inside=0 irq_after=1 acquired=[1-9][0-9]* overlap=0 in_cs=0 ok=1$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# core lock: irq_before=1 irq_inside=0 irq_after=1 … ok=1' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    for want in \
        "ok 38 - core: Machine linked in image, pin + invariants" \
        "ok 39 - core: Tcb.Configure priority + vspace rollback" \
        "ok 40 - ipc: ring3 send 8 MR -> endpoint recv (MR4..MR7 buffer)" \
        "ok 41 - core: 7c-r13 MessageInfo from r13 (r12 keeps MR3)" \
        "ok 42 - core: Tcb.Suspend(Defer) safe point + Resume entry" \
        "ok 43 - lock: RFLAGS.IF masked in critical section, restored after"; do
        n="$(grep -Fxc "$want" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done

    # (10e) **KT-318** — priority **tĩnh** + sleep/wake: hai dòng bằng chứng với giá trị **tuyệt
    # đối**. `first=b` là điều kiện priority (FIFO chọn `a`); `hard=0` là điều kiện **không** bị
    # trần cứng cắt (`hard=1` ⇔ pha hết bằng chứng giữa chừng — đột biến `no-wake`); `wakes=4` là
    # điều kiện sleep/wake (cả bốn tác vụ, kể cả `u` với `sleep(0)`, đã thức lại). Pha là **hàm
    # thuần của tick** nên ba giá trị này là **hằng**, không phải "một cửa sổ đo được".
    n="$(grep -Fxc '# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -Fxc '# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (11) long mode **thật** + halt sạch.
    grep -q 'lma=1' "$log" || {
        echo "# thiếu 'lma=1' (long mode chưa bật thật)"
        bad=1
    }
    n="$(grep -Fxc "$HALT_LINE" "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng halt '$HALT_LINE' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (12) không PANIC nào (in cả dòng thụt đầu dòng — fail-closed).
    n="$(grep -cE '^[[:space:]]*PANIC' "$log" || true)"
    [ "${n:-0}" -eq 0 ] || {
        echo "# có ${n:-0} dòng PANIC"
        bad=1
    }
    return "$bad"
}

# ------------------------------------------------------------------ --selftest (không QEMU)
# Log **tổng hợp** đúng khuôn ảnh in ra (chỉ các dòng bộ đọc dùng tới). Ca dương phải XANH; mỗi ca
# âm phải ĐỎ. Nếu một ca âm lại XANH thì bộ đọc là "dấu cao su" và soak đang PASS giả.
selftest() {
    local tmp fails=0 cases=0 pos=1 good
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/soak-x64-selftest.XXXXXX")" || die "không tạo được thư mục tạm"
    good="$tmp/good.log"
    cat >"$good" <<'EOF'
TAP version 13
1..43
# conf image: build=0123456789abcdef text=69632
# viFastOs x86_64: long mode cr0=0x0000000080000011 cr4=0x0000000000000020 cr3=0x0000000000116000 efer=0x0000000000000d00 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x116000
# viFastOs x86_64: mmio map 0x80000000..0x100000000 pages=1024 pcd=1
# viFastOs x86_64: gdt base=0x113268 limit=55 tr=0x28 tss=0x1370a4 ist1=0x11c000
# viFastOs x86_64: idt base=0x136098 limit=4095 entries=256 df_ist=1
ok 1 - gdt: loaded
ok 2 - idt: loaded
# exc: vec=3 err=0x0 rip=0x100431 cs=0x8 rflags=0x6
ok 3 - exc: int3
# exc: vec=6 err=0x0 rip=0x100440 cs=0x8 rflags=0x10006
ok 4 - exc: ud2
# exc: vec=14 err=0x0 rip=0x100450 cs=0x8 rflags=0x10006
# exc: cr2=0x700000000000
ok 5 - exc: page fault
# exc: vec=13 err=0x38 rip=0x100464 cs=0x8 rflags=0x10006
ok 6 - exc: gp
# exc: vec=254 err=0x0 rip=0x100472 cs=0x8 rflags=0x6
ok 7 - exc: unknown
# apic: id=0 version=0x14 svr=0x1ff
ok 8 - apic: lapic
# apic: ioapic id=0 ver=0x20 entries=24
ok 9 - apic: ioapic
# timer: hz=84 ticks=3 window_pit=28299
ok 10 - timer: ticks
# timer: eoi=3 ticks=3 spurious=0
ok 11 - timer: eoi
# umode: iretq entry=0x4000000000 sp=0x4000002000 rflags=0x202 cs=0x1b ss=0x23 code_va=0x4000000000 code_pa=0x114000 code_bytes=71 stack_va=0x4000001000 stack_pa=0x121000 syscall_stack=0x12a000
# umode: int80 nr=12 cs=0x1b rip=0x4000000025 user=1
# umode: ring3 ran nr=12 ret=14
ok 12 - umode: ring3
# umode: int80 ret error=0 value=14 printed=14 ptr=0x4000000039 len=14
ok 13 - umode: ret
# umode: exit code=0 nr=14
ok 14 - umode: exit
# pci: ecam base=0xb0000000 buses=0..255 functions=7
ok 15 - pci: ecam
# pci: dev 00:02.0 vendor=0x1af4 device=0x1000 class=0x0200 network hdr=0x00
# net: mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020
ok 16 - net: device
# net: arp gw=10.0.2.2 mac=52:55:0a:00:02:02 sent=1 recv=1 elapsed_us=1999 errors=0 timeouts=0
ok 17 - net: arp
# smp: hart=1 online
# smp: hart=2 online
# smp: hart=3 online
ok 18 - smp: online
# smp: tss bases=[0x1370a4, 0x13710c, 0x137174, 0x1371dc] ist1=[0x11c000, 0x11d000, 0x11e000, 0x11f000] distinct=Ok(()) ist1_distinct=Ok(()) tr=0x28 type=0x8b ok=1
ok 19 - smp: tss
# smp: ipi recv=1 intid=0x30
ok 20 - smp: ipi
# smp: hart=1 alive
# smp: hart=2 alive
# smp: hart=3 alive
# smp: hart=0 alive
# smp: online mask=0xf alive=0xf expect=0xf harts=4 ap_ticks=10 waited_online_us=10563 waited_alive_us=159968
ok 21 - smp: alive
# sched: switch a→b n=1
ok 22 - sched: a→b
# sched: switch b→a n=2
ok 23 - sched: b→a
# sched: ticks=10 tasks=4
# sched: runs a=128618 b=204226 c=5910 switches=9 ab=1 ba=2 a_rsp=0x12e168 b_rsp=0x132168 u_rsp=0x13a1a0 c_rsp=0x136170
# sched: umode task ran n=5015211 ring=3 preempted=2 resumed=1
# sched: umode counts first=4279886 last=5015211 cs=0x1b kernel_stack=0x136220
ok 24 - sched: umode task
# sched: hart=1 picked=b n=2
# sched: hart=2 picked=u n=3
# sched: hart=3 picked=c n=4
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=8 picks=[2, 2, 2, 2] first=[a, b, u, c] diff=3 double=0
ok 25 - sched: multi-hart
# blk: device vendor=0x1af4 device=0x1001 bar=4 base=0xfe004000 size=0x4000 net_base=0xfe000000 bar_distinct=1 capacity=8 blk_size=512 size_max=0 seg_max=254 queue=8 max=256 status=0x0f features=0x0000010130007e54 driver=0x0000000100000200 flush=1
ok 26 - blk: device
# blk: sector0 hex=5649464153544f532d4b543238382d53 ascii="VIFASTOS-KT288-S" magic=1 capacity=8 status=0x0f dma=0x133000 desc=0x133000 avail=0x133080 used=0x133098 requests=1 completions=1 errors=0 timeouts=0
ok 27 - blk: sector0
# com: lock ok=1 contended=3
ok 28 - com: lock
# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]
# sched: sleep wakes=4 viol=0 ticks=8 picks=[2, 2, 2, 2]
ok 29 - sched: priority
ok 30 - sched: sleep wake
# net: dhcp lease=86400 ip=10.0.2.15 mask=255.255.255.0 gw=10.0.2.2 dns=10.0.2.3 server=10.0.2.2 attempts=2 elapsed_us=7507 filter=cookie:1 xid:1
ok 31 - net: dhcp lease=86400 ip=10.0.2.15 gw=10.0.2.2 dns=10.0.2.3
# net: icmp target=10.0.2.2 reply=1 id=0x4b54 seq=1 payload=32 rtt_ms=0 attempts=1 elapsed_ms=1 filter=id:1 seq:1 payload:1
ok 32 - net: icmp echo reply target=10.0.2.2 id=0x4b54 seq=1
# net: arp dns=52:55:0a:00:02:03 ip=10.0.2.3
# net: dns name=localhost a=127.0.0.1 ttl=0 id=0x4b55 rcode=0 an=1 reply_len=43 rtt_ms=20 elapsed_ms=22 attempts=1 const:timeout_ms=3000 filter=id:1 qr:1 rcode:1
ok 33 - net: dns a=127.0.0.1 rcode=0
# lock: mcs acquired=2 contended=1 denied=1 overlap=0 in_cs=0 ok=1
# lock: irq deferred=1 delivered=1 before=2 during=2 after=3 if_before=1 if_inside=0 ok=1
ok 34 - lock: mcs irq guard
# exc: vec=14 err=0x3 rip=0x1002b0 cs=0x8 rflags=0x10002
# exc: cr2=0x100040
# wx: cr0_wp=1 text_end=0x123000 rodata_end=0x124000 data_end=0x151000 text=0x0000000000100063 rodata=0x0000000000121063 data=0x8000000000124063 low=0x8000000000008063 wx_pages=1 pt=0x116000
# wx: text write addr=0x100040 err=0x3 cr2=0x100040 unchanged=1
# wx: data write addr=0x124100 written=1 restored=1
ok 35 - wx: kernel sections
ok 36 - wx: text write faults
# core: addr=0x1570c0 size=184792 pin=196608 in_image=1 harts=1 objects=10 caps=10 healthy=1
# core buffer: va=0x1000005000 mr_offset=80 extra_offset=560 configure_offset=608 harts=1
# core configure: priority_old=4 priority_now=9 err_cfg=0 err_vspace=14 err_entry=0 vspace=0 configure=2
# core ipc: send_err=0 len=8 mr0=0x340c0000 mr3=0x340c0003 mr4=0x340c0004 mr7=0x340c0007 server=Ready
# core 7c-r13: recv_err=0 exit=0x4 info_seen_expected=0x80000341 mr3_expected=0x34100003 area_info=0x80000341 area_mr3=0x34100003
# core suspend: switched=1 state=Inactive resume_entry=0x10000001c6 resume_ok=1 exit=0x4 deferred=1 resume_stat=1
# core lock: irq_before=1 irq_inside=0 irq_after=1 acquired=14 overlap=0 in_cs=0 ok=1
ok 38 - core: Machine linked in image, pin + invariants
ok 39 - core: Tcb.Configure priority + vspace rollback
ok 40 - ipc: ring3 send 8 MR -> endpoint recv (MR4..MR7 buffer)
ok 41 - core: 7c-r13 MessageInfo from r13 (r12 keeps MR3)
ok 42 - core: Tcb.Suspend(Defer) safe point + Resume entry
ok 43 - lock: RFLAGS.IF masked in critical section, restored after
# tlb: shootdown va=0x1000006000 stale=0xa1a1111122223333 remap_obs=0xb2b2444455556666 expect=(0xa1a1111122223333,0xb2b2444455556666) adopt_ack=1 remap_ack=2 remap_seq=2 chain_va=0x1000007000 chain_obs=0xd4d4aaaabbbbcccc chain_ack=3 chain_seq=3 chain_read=0xd4d4aaaabbbbcccc expect_chain=(0xc3c3777788889999,0xd4d4aaaabbbbcccc) peer_wait=acked own_wait=acked remap_ok=1 chain_ok=1 restored=1 ok=1
# ipi: mailbox posted=6 coalesced=0 sent=6 drained=6 empty=2 acked=6 invalid=0 local=0 timeouts=0 busy=0 vector=0x30 target=1 online=0xf ok=1
# ipi: pcid supported=0 pge_supported=1 cr4_pcid=0 cr4_pge=0 full_flush=1 reason=no-pcid-per-va-invlpg
ok 37 - tlb: shootdown ap->bsp
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
EOF
    local wb=0123456789abcdef wt=69632

    # Ca dương: log đúng khuôn **phải** XANH (nếu không, bộ đọc từ chối cả log tốt).
    cases=$((cases + 1))
    if check_boot_log "$good" "$wb" "$wt" >"$tmp/p.out" 2>&1; then
        echo "ok $cases - ca dương: log đúng khuôn ⇒ XANH"
    else
        echo "not ok $cases - ca dương: log đúng khuôn mà bộ đọc ĐỎ ($(head -1 "$tmp/p.out"))"
        fails=$((fails + 1))
    fi

    # Mỗi ca âm: biến đổi log tốt rồi đòi bộ đọc **ĐỎ**.
    neg() { # $1 = tên ca, $2 = tệp log đã biến đổi
        cases=$((cases + 1))
        if check_boot_log "$2" "$wb" "$wt" >"$tmp/n.out" 2>&1; then
            echo "not ok $cases - ca âm ($1): bộ đọc vẫn XANH"
            fails=$((fails + 1))
        else
            echo "ok $cases - ca âm ($1) ⇒ ĐỎ ($(head -1 "$tmp/n.out"))"
        fi
    }

    grep -v -x -F 'TAP version 13' "$good" >"$tmp/a.log" || true
    neg "thiếu 'TAP version 13'" "$tmp/a.log"

    sed "s/^1\.\.$TAP_TESTS\$/1..$((TAP_TESTS - 1))/" "$good" >"$tmp/b.log" || true
    neg "plan sai (1..$((TAP_TESTS - 1)))" "$tmp/b.log"

    grep -v '^ok 7 - ' "$good" >"$tmp/c.log" || true
    neg "thiếu 'ok 7'" "$tmp/c.log"

    sed 's/^ok 9 - /not ok 9 - /' "$good" >"$tmp/d.log" || true
    neg "có 'not ok'" "$tmp/d.log"

    grep -v -F "$MARKER" "$good" >"$tmp/e.log" || true
    neg "thiếu marker" "$tmp/e.log"

    awk '{print} /^# viFastOs x86_64: boot ok$/{print}' "$good" >"$tmp/f.log" || true
    neg "marker in hai lần" "$tmp/f.log"

    sed 's/^# viFastOs x86_64: boot ok$/# viFastOs x86_64: boot ok\nPANIC: giả lập/' \
        "$good" >"$tmp/g.log" || true
    neg "có PANIC" "$tmp/g.log"

    grep -v -F '# conf image: ' "$good" >"$tmp/h.log" || true
    neg "thiếu '# conf image:'" "$tmp/h.log"

    sed 's/^# conf image: build=[0-9a-f]*/# conf image: build=ffffffffffffffff/' \
        "$good" >"$tmp/i.log" || true
    neg "build lệch ELF" "$tmp/i.log"

    sed 's/ text=69632$/ text=99999/' "$good" >"$tmp/j.log" || true
    neg "text lệch ELF" "$tmp/j.log"

    grep -v '^# exc: ' "$good" >"$tmp/k.log" || true
    neg "thiếu dòng '# exc:'" "$tmp/k.log"

    sed 's/^# exc: vec=3 /# exc: vec=4 /' "$good" >"$tmp/l.log" || true
    neg "sai vector (#BP in thành vec=4)" "$tmp/l.log"

    grep -v -F '# exc: cr2=' "$good" >"$tmp/m.log" || true
    neg "thiếu '# exc: cr2='" "$tmp/m.log"

    grep -v '^# apic: ' "$good" >"$tmp/n.log" || true
    neg "thiếu dòng '# apic:'" "$tmp/n.log"

    grep -v '^# timer: ' "$good" >"$tmp/o.log" || true
    neg "thiếu dòng '# timer:'" "$tmp/o.log"

    sed 's/^# timer: hz=84 ticks=3 /# timer: hz=84 ticks=0 /' "$good" >"$tmp/p2.log" || true
    neg "timer ticks=0" "$tmp/p2.log"

    grep -v -F '# pci: ecam ' "$good" >"$tmp/q.log" || true
    neg "thiếu dòng '# pci: ecam '" "$tmp/q.log"

    grep -v -F '# net: mac=' "$good" >"$tmp/r.log" || true
    neg "thiếu dòng '# net: mac='" "$tmp/r.log"

    grep -v -F '# net: arp ' "$good" >"$tmp/s.log" || true
    neg "thiếu dòng '# net: arp '" "$tmp/s.log"

    grep -v -F '# umode: ring3 ran ' "$good" >"$tmp/t.log" || true
    neg "thiếu dòng '# umode: ring3 ran …' (ảnh không xuống ring 3)" "$tmp/t.log"

    sed 's/^# umode: ring3 ran nr=12 ret=14$/# umode: ring3 ran nr=13 ret=14/' \
        "$good" >"$tmp/u.log" || true
    neg "nr sai (13 thay vì 12)" "$tmp/u.log"

    grep -v -F '# blk: device ' "$good" >"$tmp/v.log" || true
    neg "thiếu dòng '# blk: device '" "$tmp/v.log"

    sed 's/^# blk: sector0 hex=[0-9a-f]*/# blk: sector0 hex=00000000000000000000000000000000/' \
        "$good" >"$tmp/w.log" || true
    neg "sector0 hex sai (đọc ra số 0)" "$tmp/w.log"

    grep -v -F '# smp: tss bases=' "$good" >"$tmp/x.log" || true
    neg "thiếu dòng '# smp: tss bases='" "$tmp/x.log"

    grep -v -F '# smp: ipi recv=' "$good" >"$tmp/y.log" || true
    neg "thiếu dòng '# smp: ipi recv='" "$tmp/y.log"

    sed 's/^# smp: online mask=0xf alive=0xf /# smp: online mask=0x7 alive=0x7 /' \
        "$good" >"$tmp/z.log" || true
    neg "chỉ 3 hart alive (mask=0x7)" "$tmp/z.log"

    grep -v -F '# sched: switch a→b ' "$good" >"$tmp/aa.log" || true
    neg "thiếu '# sched: switch a→b'" "$tmp/aa.log"

    grep -v -F '# sched: switch b→a ' "$good" >"$tmp/ab.log" || true
    neg "thiếu '# sched: switch b→a'" "$tmp/ab.log"

    grep -v -F '# sched: ticks=' "$good" >"$tmp/ac.log" || true
    neg "thiếu '# sched: ticks='" "$tmp/ac.log"

    grep -v -F '# sched: umode task ran ' "$good" >"$tmp/af.log" || true
    neg "thiếu '# sched: umode task ran …' (tác vụ ring 3 không chạy lại)" "$tmp/af.log"

    sed 's/^\(# sched: umode task ran .*\) resumed=[0-9][0-9]*$/\1 resumed=0/' \
        "$good" >"$tmp/ag.log" || true
    neg "tác vụ ring 3 resumed=0 (không chạy lại)" "$tmp/ag.log"

    grep -v -F '# sched: hart=1 picked=' "$good" >"$tmp/ah.log" || true
    neg "chỉ BSP schedule (mất '# sched: hart=1 picked=')" "$tmp/ah.log"

    grep -v -F '# sched: hart=3 picked=' "$good" >"$tmp/ak.log" || true
    neg "chỉ 3 hart schedule (mất '# sched: hart=3 picked=')" "$tmp/ak.log"

    sed 's/^# sched: multi-hart .*$/# sched: multi-hart harts=2 rounds=6 picks=[6, 6, 0, 0] first=[a, b, ?, ?] double=0/' \
        "$good" >"$tmp/ai.log" || true
    neg "chỉ 2 hart schedule (harts=2, picks=[6, 6, 0, 0])" "$tmp/ai.log"

    # **KT-313** — `diff=0`: hart phụ **có** chọn nhưng **chưa bao giờ** chọn tác vụ **khác** tác
    # vụ BSP đang chạy. Ca âm của **bất biến mới** thay `first[0] != first[i]`; nếu bộ đọc vẫn
    # XANH thì bản sửa KT-313 đã **bỏ mất** phép kiểm thay vì thay nó.
    sed 's/^# sched: multi-hart \(.*\) diff=[0-9][0-9]* double=\([0-9][0-9]*\)$/# sched: multi-hart \1 diff=0 double=\2/' \
        "$good" >"$tmp/aj.log" || true
    neg "hart phụ chưa chọn tác vụ khác tác vụ BSP (diff=0)" "$tmp/aj.log"

    # **KT-313** — `double=1`: có double-pick (hai hart cùng một tác vụ). Phải ĐỎ **dù** `diff`
    # khác 0 — chứng minh điều kiện `double == 0` vẫn còn răng sau khi thay điều kiện.
    sed 's/^# sched: multi-hart \(.*\) double=0$/# sched: multi-hart \1 double=1/' \
        "$good" >"$tmp/al.log" || true
    neg "có double-pick (double=1)" "$tmp/al.log"

    sed 's/ lma=1$/ lma=0/' "$good" >"$tmp/ad.log" || true
    neg "lma=0 (long mode chưa bật)" "$tmp/ad.log"

    grep -v -F "$HALT_LINE" "$good" >"$tmp/ae.log" || true
    neg "thiếu dòng halt" "$tmp/ae.log"

    # **KT-316** — dòng `# com: lock` bị **xen ký tự** (không khoá): ghép với dòng khác ⇒ mẫu
    # `^# com: lock ok=1 contended=<n>$` không còn khớp ⇒ ĐỎ.
    sed 's/^\(# com: lock ok=1 contended=[0-9]*\)$/\1# sched: ticks=10 tasks=3/' \
        "$good" >"$tmp/ak.log" || true
    neg "dòng '# com: lock' bị xen ký tự (không khoá)" "$tmp/ak.log"

    # **KT-318** — **bỏ priority**: `first=b` → `first=a` — đúng log của ảnh `--features no-prio`
    # (chọn FIFO, đầu hàng đợi là `a`) ⇒ ĐỎ. Nếu bộ đọc vẫn XANH thì "priority tĩnh" chỉ là bảng in.
    sed 's/^\(# sched: prio .*\)first=b /\1first=a /' "$good" >"$tmp/am.log" || true
    neg "priority bị bỏ (first=a)" "$tmp/am.log"

    # **KT-318** — **`sleep(0)` không thức**: `wakes=4` → `wakes=3` + `u` chỉ 1 lượt + pha chạm
    # trần cứng — đúng log của ảnh `--features no-wake` ⇒ ĐỎ.
    sed 's/^# sched: sleep .*$/# sched: sleep wakes=3 viol=0 ticks=48 picks=[3, 3, 3, 1]/' \
        "$good" >"$tmp/an.log" || true
    neg "sleep(0) không thức (wakes=3)" "$tmp/an.log"

    # **KT-318** — pha bị **trần cứng** cắt: `ticks=8 hard=0` → `ticks=48 hard=1` (bằng chứng
    # **chưa** đủ nhưng pha đã hết) ⇒ ĐỎ.
    sed 's/^\(# sched: prio .*\)ticks=8 hard=0 /\1ticks=48 hard=1 /' \
        "$good" >"$tmp/ao.log" || true
    neg "pha bị trần cứng cắt (hard=1)" "$tmp/ao.log"

    # **KT-326** — mất dòng `# net: dhcp` (ảnh không có lease DHCP thật): bộ đọc phải ĐỎ, không
    # thể "xanh nhờ TAP 31". Đây là ca âm trực tiếp của bất biến DHCP ở mục 6.
    grep -v -F '# net: dhcp ' "$good" >"$tmp/ap.log" || true
    neg "thiếu dòng '# net: dhcp '" "$tmp/ap.log"

    # **KT-328** — mất dòng `# net: icmp` (ảnh không có echo thật): bộ đọc phải ĐỎ, không thể
    # "xanh nhờ TAP 32". Ca âm **tự khẳng định đã phá** (`grep -c` trước/sau) — mẫu không khớp
    # thì ca ĐỎ, không im lặng xanh (luật 2 của `k6-test-invariants.md`).
    icmp_before="$(grep -c -F '# net: icmp ' "$good" || true)"
    grep -v -F '# net: icmp ' "$good" >"$tmp/aq.log" || true
    icmp_after="$(grep -c -F '# net: icmp ' "$tmp/aq.log" || true)"
    if [ "${icmp_before:-0}" -lt 1 ] || [ "${icmp_after:-0}" -ne 0 ]; then
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - ca âm (KT-328): KHÔNG đột biến được dòng '# net: icmp' (trước=${icmp_before:-0} sau=${icmp_after:-0})"
    else
        neg "thiếu dòng '# net: icmp '" "$tmp/aq.log"
    fi

    # **KT-328** — mất dòng `# net: dns`: cùng lý do, phải ĐỎ.
    dns_before="$(grep -c -F '# net: dns ' "$good" || true)"
    grep -v -F '# net: dns ' "$good" >"$tmp/ar.log" || true
    dns_after="$(grep -c -F '# net: dns ' "$tmp/ar.log" || true)"
    if [ "${dns_before:-0}" -lt 1 ] || [ "${dns_after:-0}" -ne 0 ]; then
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - ca âm (KT-328): KHÔNG đột biến được dòng '# net: dns' (trước=${dns_before:-0} sau=${dns_after:-0})"
    else
        neg "thiếu dòng '# net: dns '" "$tmp/ar.log"
    fi

    # **KT-336a** — mất dòng `# lock: irq` (ngắt không được đo): phải ĐỎ. Ca âm **tự khẳng
    # định đã phá** (`grep -c` trước/sau) theo luật 2 của `k6-test-invariants.md`.
    lockirq_before="$(grep -c -F '# lock: irq ' "$good" || true)"
    grep -v -F '# lock: irq ' "$good" >"$tmp/as.log" || true
    lockirq_after="$(grep -c -F '# lock: irq ' "$tmp/as.log" || true)"
    if [ "${lockirq_before:-0}" -lt 1 ] || [ "${lockirq_after:-0}" -ne 0 ]; then
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - ca âm (KT-336a): KHÔNG đột biến được dòng '# lock: irq' (trước=${lockirq_before:-0} sau=${lockirq_after:-0})"
    else
        neg "thiếu dòng '# lock: irq '" "$tmp/as.log"
    fi

    # **KT-336a** — ngắt **không** bị hoãn (`deferred=1` → `deferred=0`) — đúng ca đột biến
    # `mut-no-irq` (bỏ `cli`). Phải ĐỎ, nếu không thì phép đo ngắt chỉ là chữ in.
    sed 's/^\(# lock: irq \)deferred=1 /\1deferred=0 /' "$good" >"$tmp/at.log" || true
    if [ "$(grep -cE '^# lock: irq deferred=0 ' "$tmp/at.log" || true)" -ne 1 ]; then
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - ca âm (KT-336a): KHÔNG đột biến được 'deferred=1' → 'deferred=0'"
    else
        neg "ngắt không bị hoãn (deferred=0)" "$tmp/at.log"
    fi

    # **KT-340d** — ba ca âm của bộ đọc mới: mất dòng `# tlb:` (không có phép đo shootdown),
    # `remap_obs` **quay về dữ liệu cũ** (đúng khuôn đột biến `mut-no-shootdown` /
    # `mut-no-ipi-invlpg`), và `own_wait=timeout` (đúng khuôn đột biến `mut-no-wait-drain`, lỗi v1
    # N5). Mỗi ca **phải ĐỎ** — nếu không thì TAP 37 chỉ là chữ in.
    grep -v '^# tlb: shootdown ' "$good" >"$tmp/au.log" || true
    neg "thiếu dòng '# tlb: shootdown '" "$tmp/au.log"

    sed 's/remap_obs=0xb2b2444455556666/remap_obs=0xa1a1111122223333/' "$good" >"$tmp/av.log" || true
    if [ "$(grep -c 'remap_obs=0xa1a1111122223333' "$tmp/av.log" || true)" -ne 1 ]; then
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - ca âm (KT-340d): KHÔNG đột biến được 'remap_obs' về dữ liệu cũ"
    else
        neg "hart nhận vẫn thấy dữ liệu cũ (remap_obs)" "$tmp/av.log"
    fi

    sed 's/own_wait=acked/own_wait=timeout/' "$good" >"$tmp/aw.log" || true
    if [ "$(grep -c 'own_wait=timeout' "$tmp/aw.log" || true)" -ne 1 ]; then
        cases=$((cases + 1))
        fails=$((fails + 1))
        echo "not ok $cases - ca âm (KT-340d): KHÔNG đột biến được 'own_wait='"
    else
        neg "BSP không phục vụ TlbFlush khi chờ ACK (N5)" "$tmp/aw.log"
    fi

    grep -v '^# ipi: mailbox ' "$good" >"$tmp/ax.log" || true
    neg "thiếu dòng '# ipi: mailbox '" "$tmp/ax.log"

    # **KT-313** — ca **dương** hồi quy: đúng khuôn soak đo được ở **5/600** boot
    # (`picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 double=0`) **phải XANH**. Trước KT-313 khuôn
    # này ĐỎ vì `first[0] != first[i]`; nếu giờ vẫn ĐỎ thì bản sửa chưa vào (hoặc đã bỏ mất phép
    # kiểm). Đếm riêng (`pos`) để không trộn với số ca âm.
    cases=$((cases + 1))
    pos=$((pos + 1))
    sed 's/^# sched: multi-hart .*$/# sched: multi-hart harts=4 rounds=6 picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 double=0/' \
        "$good" >"$tmp/pos.log" || true
    if check_boot_log "$tmp/pos.log" "$wb" "$wt" >"$tmp/pos.out" 2>&1; then
        echo "ok $cases - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH"
    else
        echo "not ok $cases - ca dương (KT-313): khuôn soak first=[a, a, …] diff=1 mà bộ đọc ĐỎ ($(head -1 "$tmp/pos.out"))"
        fails=$((fails + 1))
    fi

    rm -rf "$tmp" 2>/dev/null || true
    echo "# soak-x86_64 --selftest: $((cases - fails))/$cases ca đúng như phải ($pos dương + $((cases - pos)) âm)"
    [ "$fails" -eq 0 ] || return 1
    echo "# soak-x86_64 --selftest: PASS"
    return 0
}

# ------------------------------------------------------------------ đối số
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    usage
    exit 0
fi
if [ "${1:-}" = "--selftest" ]; then
    SELFTEST=1
    shift
fi
if [ "${1:-}" = "--smoke" ]; then
    SMOKE=1
    shift
    [ $# -ge 1 ] || die "--smoke cần N (ví dụ: --smoke 3 3)"
fi
N="${1:-600}"
LANES="${2:-3}"
case "$N" in '' | *[!0-9]*) die "N phải là số dương (nhận '$N')" ;; esac
case "$LANES" in '' | *[!0-9]*) die "LANES phải là số dương (nhận '$LANES')" ;; esac
[ "$N" -gt 0 ] || die "N phải > 0"
# AGENTS §9: tối đa 3 QEMU song song; D2-24 đúng 3 luồng cho cổng soak.
[ "$LANES" -ge 1 ] && [ "$LANES" -le 3 ] || die "LANES=$LANES ngoài [1,3] (AGENTS §9: tối đa 3 QEMU)"

# D2-24: n ≥ 3/p. p = 5/1000 ⇒ n ≥ ceil(3 · 1000 / 5) = 600 (K5' chốt N = 600, D2-149).
# Số nguyên, không dấu phẩy động.
NEED=$(((3 * P_DEN + P_NUM - 1) / P_NUM))
if [ "$N" -lt "$NEED" ]; then
    if [ "$SMOKE" -ne 1 ]; then
        echo "soak-x86_64.sh: N=$N < n ≥ 3/p = $NEED (p=$P_NUM/$P_DEN = 0,5 %; D2-24, K5'/D2-149)" >&2
        echo "soak-x86_64.sh: giảm N là NỚI NGƯỠNG — không chạy. Dùng --smoke chỉ để đỏ trước/kiểm marker." >&2
        exit 2
    fi
    echo "# soak: SMOKE N=$N < $NEED — KHÔNG phải bằng chứng cổng K5' (D2-24), luôn exit 1"
fi

# ------------------------------------------------------------------ --selftest (không cần tool/QEMU)
if [ "$SELFTEST" -eq 1 ]; then
    selftest
    exit $?
fi

# ------------------------------------------------------------------ công cụ
command -v cargo >/dev/null 2>&1 || die "không có cargo"
command -v python3.12 >/dev/null 2>&1 || die "không có python3.12 (cần cho image_digest.py)"
[ -f "$DIGEST" ] || die "không thấy $DIGEST"
QEMU="$(command -v qemu-system-x86_64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-x86_64"

# QEMU >= 9.2 (cùng sàn riscv64/aarch64, D2-72 Q2) — thiếu/dưới sàn ⇒ đỏ, không tự hạ.
QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
case "$QEMU_VERSION" in *.*) ;; *) die "phiên bản QEMU '$QEMU_VERSION' không có dạng major.minor" ;; esac
QEMU_MAJOR="${QEMU_VERSION%%.*}"
QEMU_REST="${QEMU_VERSION#*.}"
QEMU_MINOR="${QEMU_REST%%.*}"
case "$QEMU_MAJOR" in '' | *[!0-9]*) die "không đọc được major của QEMU '$QEMU_VERSION'" ;; esac
case "$QEMU_MINOR" in '' | *[!0-9]*) die "không đọc được minor của QEMU '$QEMU_VERSION'" ;; esac
if [ "$QEMU_MAJOR" -lt "$QEMU_MIN_MAJOR" ] ||
    { [ "$QEMU_MAJOR" -eq "$QEMU_MIN_MAJOR" ] && [ "$QEMU_MINOR" -lt "$QEMU_MIN_MINOR" ]; }; then
    die "QEMU $QEMU_VERSION < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2) — không tự hạ"
fi

# Cấu hình QEMU **đúng** đường boot của ảnh (k5-x86_64-boot.md + run-x86_64.sh):
# `-M q35` (ECAM qua MCFG), `-smp 4` (KT-278), `virtio-net-pci` trên slirp (KT-274),
# `virtio-blk-pci` trên đĩa tạm (KT-288). KHÔNG `-icount` (D2-74 điều kiện c).
# `snapshot=on` **bắt buộc**: 3 luồng soak dùng **cùng** tệp đĩa, thiếu nó QEMU báo
# `Failed to get "write" lock` và 2/3 boot đỏ (bài học KT-273 bên aarch64).
# **KT-323** — `-cpu "$X64_CPU"`: mặc định `qemu64` (model QEMU chọn cho `-M q35`; cùng model với
# mọi số đo cũ ⇒ tái lập được — AGENTS §6.3); `V2_X64_CPU=max` để đo trên tập tính năng rộng nhất.
# **Cùng** biến, **cùng** ngữ nghĩa với `run-x86_64.sh` — nếu hai script lệch model thì bằng chứng
# runner và soak không còn so sánh được. Probe **và** mọi boot dùng chung `QEMU_ARGS`.
X64_CPU="${V2_X64_CPU:-qemu64}"
DISK="$LOGDIR/disk0.raw"
QEMU_ARGS=(-M q35 -cpu "$X64_CPU" -m 128M -nographic -no-reboot -smp 4 \
    -kernel "$IMAGE" \
    -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
    -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" -device virtio-blk-pci,drive=d0)

# ------------------------------------------------------------------ build + neo ảnh
echo "# soak-x86_64 KT-293 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2)"
echo "# cấu hình: -M q35 -cpu $X64_CPU -m 128M -nographic -no-reboot -smp 4; KHÔNG icount (D2-74 c)"
echo "# N=$N lanes=$LANES timeout=$((TIMEOUT_DS / 10))s/boot; p=$P_NUM/$P_DEN (0,5 %) ⇒ n ≥ 3/p = $NEED (D2-24, K5'/D2-149)"
if command -v git >/dev/null 2>&1; then
    GIT_HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
    GIT_DIRTY="$(git -C "$REPO_ROOT" status --porcelain -- v2/kernel 2>/dev/null | wc -l | tr -d ' ')"
    echo "# git HEAD=$GIT_HEAD dirty-kernel-paths=$GIT_DIRTY"
else
    GIT_HEAD=unknown
    GIT_DIRTY=unknown
fi

mkdir -p "$LOGDIR" 2>/dev/null || true
# **KT-288** — đĩa tạm: 8 sector 512 B, 17 byte đầu `VIFASTOS-KT288-S0` (8 byte đầu khớp
# `virtio_blk::SECTOR0_MAGIC`). Soak đòi **cùng** hex 16 byte đầu trong mỗi log.
dd if=/dev/zero of="$DISK" bs=512 count=8 2>/dev/null || die "không tạo được đĩa tạm $DISK"
printf 'VIFASTOS-KT288-S0' | dd of="$DISK" bs=1 seek=0 conv=notrunc 2>/dev/null ||
    die "không ghi được chữ ký vào $DISK"
WANT_SECTOR0_HEX="$(od -An -tx1 -N16 "$DISK" 2>/dev/null | tr -d ' \n')"
[ "${#WANT_SECTOR0_HEX}" -eq 32 ] || die "không đọc được 16 byte đầu của $DISK"
DISK_BYTES="$(wc -c <"$DISK" | tr -d ' ')"
WANT_CAPACITY=$((DISK_BYTES / 512))
[ "$WANT_CAPACITY" -eq 8 ] || die "đĩa tạm $DISK_BYTES byte, cần 4096"

# KT-78/OD-K6-6: boot đi qua `qemu_guard.sh` — process group riêng, timeout kill **cả group**,
# trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). 3 luồng = 3 subshell cùng script.
# shellcheck source=/dev/null
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
INFRA_INVALID=0
INFRA_LOG="$LOGDIR/kt293-infra.log"
qg_infra before "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
# **KT-333c** (D2-243 mục 3a) — khoá **độc quyền cấp máy đo**, giữ từ trước build tới hết soak;
# nhả chỉ ở shell chính (làn soak là subshell, `$$` vẫn là PID cha).
# shellcheck source=/dev/null
. "$V2_ROOT/tools/measure-lock.sh"
measure_lock_acquire "soak x86_64 N=$N lanes=$LANES" || exit 2
trap 'qg_cleanup; measure_lock_release_if_top' EXIT
trap 'qg_cleanup; measure_lock_release_if_top; exit 130' INT
trap 'qg_cleanup; measure_lock_release_if_top; exit 143' TERM

# `harness_hits <ảnh>`: tổng số chuỗi harness (grep -F) trong ảnh — ảnh sản phẩm phải 0.
harness_hits() {
    local img="$1" pat n total_hits=0
    if command -v strings >/dev/null 2>&1; then
        for pat in "${PRODUCT_HARNESS_MARKERS[@]}"; do
            n="$(strings -a "$img" 2>/dev/null | grep -Fc -- "$pat" || true)"
            total_hits=$((total_hits + ${n:-0}))
        done
    else
        for pat in "${PRODUCT_HARNESS_MARKERS[@]}"; do
            n="$(LC_ALL=C grep -a -o -F -- "$pat" "$img" 2>/dev/null | wc -l | tr -d ' ' || true)"
            total_hits=$((total_hits + ${n:-0}))
        done
    fi
    echo "$total_hits"
}

echo "# build ảnh SẢN PHẨM x86_64 ($TARGET, release, KHÔNG feature)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-x86_64 --target "$TARGET" --release \
    >"$LOGDIR/kt293-soak-build.log" 2>&1; then
    tail -20 "$LOGDIR/kt293-soak-build.log" >&2
    die "build ảnh sản phẩm x86_64 FAIL (xem $LOGDIR/kt293-soak-build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh sản phẩm $IMAGE"
cp "$IMAGE" "$PRODUCT_IMAGE" || die "không copy được ảnh sản phẩm"
echo "# build ảnh KTEST x86_64 ($TARGET, release, --features ktest)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-x86_64 --target "$TARGET" --release \
    --features ktest >"$LOGDIR/kt340a-soak-build-ktest.log" 2>&1; then
    tail -20 "$LOGDIR/kt340a-soak-build-ktest.log" >&2
    die "build ảnh ktest x86_64 FAIL (xem $LOGDIR/kt340a-soak-build-ktest.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh ktest $IMAGE"
cp "$IMAGE" "$KTEST_IMAGE" || die "không copy được ảnh ktest"

# **KT-340a** (D2-08): ảnh **sản phẩm** 0 harness marker, kèm **ca âm** (ảnh ktest phải > 0 —
# OD-K6-5 luật 3: bộ đếm 0 mà không có ca âm là dấu cao su).
PROD_HITS="$(harness_hits "$PRODUCT_IMAGE")"
KTEST_HITS="$(harness_hits "$KTEST_IMAGE")"
[ "${PROD_HITS:-1}" -eq 0 ] || die "ảnh sản phẩm chứa ${PROD_HITS} chuỗi harness (D2-08)"
[ "${KTEST_HITS:-0}" -gt 0 ] || die "ca âm HỎNG: ảnh ktest cũng 0 harness marker (${KTEST_HITS}) — bộ quét không có răng"
echo "# image=product harness-strings=0 (ktest=$KTEST_HITS ⇒ bộ quét có răng)"
# Ngân sách `.data+.bss` của **cả hai** ảnh (D2-72 Q5/D2-236, e_machine=62) — không nới.
for pair in "x86_64-product:$PRODUCT_IMAGE" "x86_64-ktest:$KTEST_IMAGE"; do
    label="${pair%%:*}"
    img="${pair#*:}"
    if ! bash "$HERE/budget-x86_64.sh" "$img" "$label" >"$LOGDIR/budget-$label.log" 2>&1; then
        cat "$LOGDIR/budget-$label.log"
        die "ngân sách ảnh $label vượt trần (xem $LOGDIR/budget-$label.log)"
    fi
    grep -E '^# image=' "$LOGDIR/budget-$label.log" || true
done

# Soak đo **ảnh kiểm** (TAP 34 là hợp đồng của soak).
IMAGE="$KTEST_IMAGE"

# D2-80: BUILD_ID (FNV-1a 64 trên `.text`, kernel tự in) + sha256 (toàn ảnh) + kích thước.
DIGEST_OUT="$(python3.12 "$DIGEST" "$IMAGE" 2>/dev/null)" || die "image_digest.py lỗi trên $IMAGE"
WANT_BUILD="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/^build=\([0-9a-f]*\).*/\1/p')"
WANT_TEXT="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/.* text=\([0-9]*\)$/\1/p')"
[ -n "$WANT_BUILD" ] && [ -n "$WANT_TEXT" ] || die "không đọc được BUILD_ID từ $DIGEST_OUT"
if command -v shasum >/dev/null 2>&1; then
    IMG_SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
else
    IMG_SHA="$(sha256sum "$IMAGE" | awk '{print $1}')"
fi
[ -n "$IMG_SHA" ] || die "không tính được sha256 ảnh"
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"
echo "# image=product bytes=$IMG_BYTES sha256=$IMG_SHA"
echo "# BUILD_ID (D2-80) = build=$WANT_BUILD text=$WANT_TEXT"
if [ "$GIT_DIRTY" != "0" ] && [ "$GIT_DIRTY" != "unknown" ]; then
    PATCH_SHA="$(
        git -C "$REPO_ROOT" diff -- v2/kernel 2>/dev/null | shasum -a 256 | awk '{print $1}'
    )"
    echo "# git dirty: v2/kernel có $GIT_DIRTY tệp khác HEAD; sha256 patch=$PATCH_SHA"
fi

# --------------------------------------------------- cổng danh tính ảnh (D2-80)
# Một boot dò: log phải có **đúng một** `# conf image:` khớp ELF **và** đạt mọi bất biến của một
# boot sạch. Không có/lệch ⇒ TỪ CHỐI ĐO (exit 3), **không** đếm một boot soak nào — thà không đo
# còn hơn đo ảnh không rõ nguồn.
PROBE_LOG="$LOGDIR/kt293-probe.log"
trap 'qg_cleanup; measure_lock_release_if_top' EXIT
trap 'qg_cleanup; measure_lock_release_if_top; exit 130' INT
trap 'qg_cleanup; measure_lock_release_if_top; exit 143' TERM
echo "# probe (D2-80): boot MỘT lần để đọc dòng '# conf image:' của ảnh"
if ! qg_boot "$PROBE_LOG" "$TIMEOUT_DS" "$QEMU" "${QEMU_ARGS[@]}"; then
    echo "# soak-x86_64.sh: TỪ CHỐI ĐO — probe boot không thoát (rc=$QG_LAST_RC, pid=$QG_LAST_PID, log=$PROBE_LOG)" >&2
    exit 3
fi
PROBE_BUILD="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$PROBE_LOG" | head -1)"
PROBE_TEXT="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$PROBE_LOG" | head -1)"
# **KT-323** — chẩn đoán khi model CPU sai: QEMU thoát ngay (`unable to find CPU model '<tên>'`) và
# log **không** có TAP/`# conf image:`; nếu không in lý do thì thông điệp "ảnh không khai hash" ở
# dưới gây lạc hướng. Chỉ **đọc** log, không mở thêm QEMU nào.
if [ -z "$PROBE_BUILD" ] && grep -qiE 'unable to find cpu model|invalid cpu model' "$PROBE_LOG" 2>/dev/null; then
    echo "# soak-x86_64.sh: QEMU TỪ CHỐI model CPU (V2_X64_CPU='$X64_CPU') — xem $PROBE_LOG" >&2
fi
if [ "$PROBE_BUILD" != "$WANT_BUILD" ] || [ "$PROBE_TEXT" != "$WANT_TEXT" ]; then
    echo "# soak-x86_64.sh: TỪ CHỐI ĐO — ảnh in '# conf image: build=${PROBE_BUILD:-none} text=${PROBE_TEXT:-none}', ELF là build=$WANT_BUILD text=$WANT_TEXT (D2-80)" >&2
    echo "# soak-x86_64.sh: ảnh KHÔNG tự khai hash khớp ELF ⇒ không có neo, không đo" >&2
    exit 3
fi
echo "# probe: '# conf image:' khớp ELF (build=$WANT_BUILD text=$WANT_TEXT)"
if ! PROBE_WHY="$(check_boot_log "$PROBE_LOG" "$WANT_BUILD" "$WANT_TEXT" 2>&1)"; then
    echo "# soak-x86_64.sh: TỪ CHỐI ĐO — probe boot không đạt bất biến của một boot sạch:" >&2
    printf '%s\n' "$PROBE_WHY" | sed 's/^/#   /' >&2
    exit 3
fi
echo "# probe: log đạt mọi bất biến của một boot sạch (TAP $TAP_TESTS/$TAP_TESTS + marker + halt)"

# ------------------------------------------------------------------ log
RUNDIR="$LOGDIR/run-$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$RUNDIR" || die "không tạo được $RUNDIR"
RESULTS="$RUNDIR/results.txt"
: >"$RESULTS" || die "không ghi được $RESULTS"
echo "# log: $RUNDIR/boot-<i>.log (mỗi boot một tệp; kết quả: $RESULTS)"

# ------------------------------------------------------------------ một boot
# boot_one <i>: chạy 1 boot, kiểm **mọi** bất biến, ghi `ok i` hoặc `fail i <lý do>`. Trả 0/1.
boot_one() {
    local i="$1" out="$RUNDIR/boot-$i.log" why=""

    # KT-78: QEMU trong process group riêng + timeout kill cả group (124 = quá 20 s).
    qg_boot "$out" "$TIMEOUT_DS" "$QEMU" "${QEMU_ARGS[@]}"
    if [ "$?" -ne 0 ]; then
        printf 'fail %s timeout-%ss\n' "$i" "$((TIMEOUT_DS / 10))" >>"$RESULTS"
        echo "boot $i/$N FAIL: quá $((TIMEOUT_DS / 10)) s (treo) pid=$QG_LAST_PID log=$out" >&2
        return 1
    fi

    why="$(check_boot_log "$out" "$WANT_BUILD" "$WANT_TEXT" 2>&1)" || {
        printf 'fail %s %s\n' "$i" "$(printf '%s' "$why" | tr '\n' ';' | tr ' ' ',')" >>"$RESULTS"
        echo "boot $i/$N FAIL: $why log=$out" >&2
        tail -5 "$out" | sed 's/^/    /' >&2
        return 1
    }
    printf 'ok %s\n' "$i" >>"$RESULTS"
    echo "boot $i/$N ok"
    return 0
}

# ------------------------------------------------------------------ luồng
lane_worker() {
    local lane="$1" i="$2"
    while [ "$i" -le "$N" ]; do
        boot_one "$i" || true
        i=$((i + LANES))
    done
}

echo "# soak: bắt đầu N=$N lanes=$LANES $(date -u '+%H:%M:%SZ')"
SECONDS=0
lane=1
LANE_PIDS=""
while [ "$lane" -le "$LANES" ]; do
    lane_worker "$lane" "$lane" &
    LANE_PIDS="$LANE_PIDS $!"
    lane=$((lane + 1))
done
for lane_pid in $LANE_PIDS; do
    wait "$lane_pid"
done
WALL="$SECONDS"

# ------------------------------------------------------------------ ảnh không được đổi giữa soak
if command -v shasum >/dev/null 2>&1; then
    IMG_SHA_END="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
else
    IMG_SHA_END="$(sha256sum "$IMAGE" | awk '{print $1}')"
fi
IMG_CHANGED=0
[ "$IMG_SHA_END" = "$IMG_SHA" ] || IMG_CHANGED=1

# ------------------------------------------------------------------ tổng
# KT-78/OD-K6-6: trạng thái hạ tầng **sau** phép đo; QEMU mồ côi/lạ ⇒ phép đo KHÔNG HỢP LỆ.
qg_infra after "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
echo "# infra: cpu=$X64_CPU invalid=$INFRA_INVALID isolation_bad=$QG_ISO_BAD guard_log=$QG_LOG"
qg_guard_summary "$QG_LOG" >"$LOGDIR/guard-summary.log" 2>/dev/null || true
echo "# guard_summary_log=$LOGDIR/guard-summary.log"
RC124="$(grep -c 'rc=124' "$QG_LOG" 2>/dev/null || true)"
QEMU_OWN="$(qg_own)"
echo "# guard: boot_end=$(grep -c '^# boot_end' "$QG_LOG" 2>/dev/null || true) rc_124=${RC124:-0} qemu_own=$QEMU_OWN"

OK_COUNT="$(grep -c '^ok ' "$RESULTS" || true)"
FAIL_COUNT="$(grep -c '^fail ' "$RESULTS" || true)"
TOTAL=$((OK_COUNT + FAIL_COUNT))
echo "# soak: xong wall=${WALL}s ($((N / (WALL > 0 ? WALL : 1))) boot/s); ok=$OK_COUNT fail=$FAIL_COUNT tổng=$TOTAL/$N"
if [ "$FAIL_COUNT" -ne 0 ]; then
    echo "# soak: danh sách boot đỏ (lý do):"
    grep '^fail ' "$RESULTS" | sed 's/^/#   /'
fi
if [ "$TOTAL" -ne "$N" ]; then
    echo "# soak: FAIL — chỉ xử lý $TOTAL/$N slot (không bao giờ xanh vì không chạy gì)"
    exit 1
fi
if [ "$IMG_CHANGED" -ne 0 ]; then
    echo "# soak: FAIL — ảnh đổi giữa soak ($IMG_SHA → $IMG_SHA_END): bằng chứng không còn neo (D2-80)"
    exit 1
fi
if [ "$QEMU_OWN" != "0" ]; then
    echo "# soak: FAIL — còn $QEMU_OWN QEMU của chính script sống sau soak (AGENTS §9)"
    exit 1
fi
echo "# soak: BUILD_ID=$WANT_BUILD sha256=$IMG_SHA git=$GIT_HEAD"
echo "# soak: $OK_COUNT/$N lỗi=$FAIL_COUNT — 0/N chỉ là trần (QD §5.3); không retry (D2-07)"
if [ "$SMOKE" -eq 1 ]; then
    echo "# soak: SMOKE N=$N < $NEED — KHÔNG phải bằng chứng cổng K5'"
    exit 1
fi
[ "$FAIL_COUNT" -eq 0 ] || {
    echo "# soak-x86_64.sh: FAIL ($FAIL_COUNT/$N boot đỏ)"
    exit 1
}
[ "$N" -ge "$NEED" ] || {
    echo "# soak-x86_64.sh: FAIL — N=$N < $NEED (n ≥ 3/p)"
    exit 1
}
echo "# soak: 0/$N lỗi (PASS)"
echo "# soak-x86_64.sh: PASS"
exit 0
