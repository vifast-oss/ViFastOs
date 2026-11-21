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
#   1. đúng 1 `TAP version 13`; đúng 1 `1..27`; đúng **27** dòng `ok N - `, đủ số 1..27,
#      **không** dòng `not ok` nào (KT-261 7 ca exception + KT-266 4 ca APIC/timer + KT-270 3 ca
#      U-mode + KT-274 3 ca PCI/net + KT-278 4 ca SMP + KT-286 2 ca preemption + KT-304 1 ca tác
#      vụ ring 3 + KT-310 1 ca **nhiều hart** + KT-288 2 ca virtio-blk = 27);
#   2. đúng 1 `# viFastOs x86_64: boot ok` (marker **nguyên văn**, in sau khi mọi TAP xanh);
#   3. đúng 1 `# conf image: build=<16 hex> text=<len>`, build/text **khớp ELF** (D2-80);
#   4. **KT-261** — đúng **5** dòng `# exc: vec=… err=… rip=… cs=0x8 rflags=…`: `#BP` 3/err 0,
#      `#UD` 6/err 0, `#PF` 14/err 0 + `# exc: cr2=0x700000000000`, `#GP` 13/err 0x38,
#      `isr_unknown` 254/err 0. Ảnh không chạy handler exception ⇒ ĐỎ dù TAP 1..7 xanh;
#   5. **KT-266** — `# apic: id=… version=0x… svr=0x…` + `# apic: ioapic id=… ver=0x… entries=…`
#      + `# timer: hz=… ticks=…` (ticks > 0) + `# timer: eoi=… ticks=…` (ticks > 0);
#   6. **KT-274** — `# pci: ecam base=0x… buses=… functions=<n>` (ECAM thật, không legacy) +
#      `# net: mac=<6 octet> …` (MAC đọc từ thiết bị) + `# net: arp gw=<ip> mac=<mac> sent=<n>
#      recv=<n>` (ARP reply **thật** của slirp, sent/recv ≥ 1);
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
#      cảnh **cả hai** chiều) và `# sched: ticks=<t> tasks=3` (scheduler nhường lại `main`);
#  10b. **KT-304** — `# sched: umode task ran n=<n> ring=3 preempted=<n> resumed=<n>`: tác vụ
#      **ring 3** đã chạy thật (`n > 0`), bị preempt khi đang ở CPL3 (`ring=3`), được
#      `schedule()` chọn **lại** (`resumed ≥ 1`) — thiếu dòng ⇒ ĐỎ;
#  10c. **KT-310** — `# sched: hart=0|1 picked=<task> n=<k>` (mỗi hart **một** dòng) **và**
#      `# sched: multi-hart harts=2 rounds=<r> picks=[<a>, <b>] first=[<x>, <y>] double=0`:
#      hai hart cùng chọn từ **một** hàng đợi toàn cục, chọn **khác** tác vụ ở lần đầu, hart phụ
#      chọn ≥ 1, **không** double-pick — thiếu/lệch ⇒ ĐỎ;
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
#                                                         # 1 ca dương + 27 ca âm, KHÔNG mở QEMU
#   bash v2/test/boot/soak-x86_64.sh --help
#
# Biến môi trường: `SOAK_X86_64_LOGDIR` (mặc định `$TMPDIR/k5-x86_64-soak`).
#
# Exit: 0 = 0 lỗi/N (chỉ ở chế độ cổng, N ≥ 600); 1 = có boot đỏ / ảnh đổi giữa soak;
#       2 = lỗi cấu hình (thiếu tool, N < 600, LANES ngoài [1,3], build hỏng);
#       3 = TỪ CHỐI ĐO (ảnh không in `# conf image:` khớp ELF) — 0 boot được đếm.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
TARGET="x86_64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-x86_64"
LOGDIR="${SOAK_X86_64_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/k5-x86_64-soak}"
QEMU_MIN_MAJOR=9 # cùng sàn với riscv64/aarch64 (D2-72 Q2): QEMU >= 9.2
QEMU_MIN_MINOR=2
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
P_NUM=5        # p = 0,5 % = 5/1000 — K5' chốt N = 600 (D2-149) ⇒ n ≥ 3/p = 600 (D2-24)
P_DEN=1000
TAP_TESTS=27   # KT-261 7 + KT-266 4 + KT-270 3 + KT-274 3 + KT-278 4 + KT-286 2 + KT-304 1 + KT-310 1 + KT-288 2
EXPECT_EXC=5   # KT-261: #BP, #UD, #PF, #GP, isr_unknown
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

    # (1) khung TAP — đúng **một** dòng mỗi mẫu, đủ số 1..25, không `not ok`.
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
    n="$(grep -cE '^# sched: ticks=[0-9]+ tasks=3$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: ticks=<t> tasks=3' xuất hiện ${n:-0} lần, cần đúng 1"
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

    # (10c) **KT-310** — **nhiều hart** schedule: mỗi hart một dòng `picked=`, cộng dòng tổng hợp
    # có `double=0`; hai hart phải chọn **khác** tác vụ ở lần đầu và hart phụ phải chọn ≥ 1.
    n="$(grep -cE '^# sched: hart=0 picked=[abu] n=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: hart=0 picked=<task> n=<k>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# sched: hart=1 picked=[abu] n=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# sched: hart=1 picked=<task> n=<k>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    mh_sum="$(grep -E '^# sched: multi-hart harts=2 rounds=[0-9]+ picks=\[[0-9]+, [0-9]+\] first=\[[abu], [abu]\] double=0$' "$log" || true)"
    if [ -z "$mh_sum" ]; then
        echo "# thiếu '# sched: multi-hart harts=2 … picks=[…] first=[…] double=0' (không có bằng chứng nhiều hart)"
        bad=1
    else
        mh_f0="$(printf '%s\n' "$mh_sum" | sed -n 's/.*first=\[\([abu]\), [abu]\].*/\1/p')"
        mh_f1="$(printf '%s\n' "$mh_sum" | sed -n 's/.*first=\[[abu], \([abu]\)\].*/\1/p')"
        [ "$mh_f0" != "$mh_f1" ] || {
            echo "# multi-hart first=[$mh_f0, $mh_f1] — hai hart chọn **cùng** tác vụ"
            bad=1
        }
        mh_p1="$(printf '%s\n' "$mh_sum" | sed -n 's/.*picks=\[[0-9]*, \([0-9]*\)\].*/\1/p')"
        [ "${mh_p1:-0}" -ge 1 ] || {
            echo "# multi-hart picks[1]=${mh_p1:-0} — hart phụ chưa chọn tác vụ nào"
            bad=1
        }
    fi

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
    local tmp fails=0 cases=0 good
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/soak-x64-selftest.XXXXXX")" || die "không tạo được thư mục tạm"
    good="$tmp/good.log"
    cat >"$good" <<'EOF'
TAP version 13
1..27
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
# sched: ticks=10 tasks=3
# sched: runs a=128618 b=204226 switches=9 ab=1 ba=2 a_rsp=0x12e168 b_rsp=0x132168 u_rsp=0x136170
# sched: umode task ran n=5015211 ring=3 preempted=2 resumed=1
# sched: umode counts first=4279886 last=5015211 cs=0x1b kernel_stack=0x136220
ok 24 - sched: umode task
# sched: hart=1 picked=a n=1
# sched: hart=0 picked=b n=2
# sched: multi-hart harts=2 rounds=7 picks=[3, 4] first=[b, a] double=0
ok 25 - sched: multi-hart
# blk: device vendor=0x1af4 device=0x1001 bar=4 base=0xfe004000 size=0x4000 net_base=0xfe000000 bar_distinct=1 capacity=8 blk_size=512 size_max=0 seg_max=254 queue=8 max=256 status=0x0f features=0x0000010130007e54 driver=0x0000000100000200 flush=1
ok 26 - blk: device
# blk: sector0 hex=5649464153544f532d4b543238382d53 ascii="VIFASTOS-KT288-S" magic=1 capacity=8 status=0x0f dma=0x133000 desc=0x133000 avail=0x133080 used=0x133098 requests=1 completions=1 errors=0 timeouts=0
ok 27 - blk: sector0
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
    neg "plan sai (1..24)" "$tmp/b.log"

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

    sed 's/^# sched: multi-hart .*$/# sched: multi-hart harts=2 rounds=6 picks=[6, 0] first=[a, ?] double=0/' \
        "$good" >"$tmp/ai.log" || true
    neg "hart phụ không chọn tác vụ nào (picks=[6, 0])" "$tmp/ai.log"

    sed 's/^# sched: multi-hart \(.*\)first=\[b, a\] double=0$/# sched: multi-hart \1first=[a, a] double=0/' \
        "$good" >"$tmp/aj.log" || true
    neg "hai hart chọn cùng tác vụ (first=[a, a])" "$tmp/aj.log"

    sed 's/ lma=1$/ lma=0/' "$good" >"$tmp/ad.log" || true
    neg "lma=0 (long mode chưa bật)" "$tmp/ad.log"

    grep -v -F "$HALT_LINE" "$good" >"$tmp/ae.log" || true
    neg "thiếu dòng halt" "$tmp/ae.log"

    rm -rf "$tmp" 2>/dev/null || true
    echo "# soak-x86_64 --selftest: $((cases - fails))/$cases ca đúng như phải (1 dương + $((cases - 1)) âm)"
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
DISK="$LOGDIR/disk0.raw"
QEMU_ARGS=(-M q35 -m 128M -nographic -no-reboot -smp 4 \
    -kernel "$IMAGE" \
    -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
    -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" -device virtio-blk-pci,drive=d0)

# ------------------------------------------------------------------ build + neo ảnh
echo "# soak-x86_64 KT-293 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2)"
echo "# cấu hình: -M q35 -m 128M -nographic -no-reboot -smp 4; KHÔNG icount (D2-74 c)"
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

echo "# build ảnh x86_64 ($TARGET, release)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-x86_64 --target "$TARGET" --release \
    >"$LOGDIR/kt293-soak-build.log" 2>&1; then
    tail -20 "$LOGDIR/kt293-soak-build.log" >&2
    die "build ảnh x86_64 FAIL (xem $LOGDIR/kt293-soak-build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

# D2-08: ảnh sản phẩm không được chứa chuỗi `ktest`.
if command -v strings >/dev/null 2>&1; then
    HITS="$(strings "$IMAGE" 2>/dev/null | grep -c ktest || true)"
else
    HITS="$(LC_ALL=C grep -a -o ktest "$IMAGE" 2>/dev/null | wc -l | tr -d ' ' || true)"
fi
[ "${HITS:-0}" -eq 0 ] || die "ảnh sản phẩm chứa ${HITS} chuỗi 'ktest' (D2-08)"
echo "# image=product ktest-strings=0"

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
trap 'qg_cleanup' EXIT
trap 'qg_cleanup; exit 130' INT
trap 'qg_cleanup; exit 143' TERM
echo "# probe (D2-80): boot MỘT lần để đọc dòng '# conf image:' của ảnh"
if ! qg_boot "$PROBE_LOG" "$TIMEOUT_DS" "$QEMU" "${QEMU_ARGS[@]}"; then
    echo "# soak-x86_64.sh: TỪ CHỐI ĐO — probe boot không thoát (rc=$QG_LAST_RC, pid=$QG_LAST_PID, log=$PROBE_LOG)" >&2
    exit 3
fi
PROBE_BUILD="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$PROBE_LOG" | head -1)"
PROBE_TEXT="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$PROBE_LOG" | head -1)"
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
echo "# infra: invalid=$INFRA_INVALID isolation_bad=$QG_ISO_BAD guard_log=$QG_LOG"
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
