#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-265 — soak boot ảnh **aarch64** K4' (điều kiện "**như K3**" của K4': soak 1.000).
#
# Khuôn: `soak.sh` (riscv64, KT-57). **Cùng bất biến, không nới một ngưỡng nào**:
#   - D2-24 + QD §5.3: máy `ps`, **N = 1.000**, **3 luồng song song**, `n ≥ 3/p` với p = 0,3 %
#     (⇒ n ≥ 1.000); 0 lỗi/N; 0/N chỉ là **trần**, không phải bảo đảm.
#   - D2-72 Q10: timeout **20 s/boot**, chỉ `kill` PID do chính script tạo (AGENTS §9),
#     BUILD_ID = git sha + sha256 ảnh, ghi qemu version + cấu hình; **không retry** (D2-07).
#   - D2-74 điều kiện (c): soak **KHÔNG** `-icount`.
#   - D2-80: mỗi log boot phải có **hash ảnh do kernel tự in** (`# conf image: build=<16 hex>
#     text=<len>`, FNV-1a 64 trên `.text` đã nạp); script băm lại **cùng** section từ ELF
#     (`test/conformance/image_digest.py`) và **từ chối** log không có/lệch hash.
#   - D2-08: ảnh sản phẩm KHÔNG chứa chuỗi `ktest` (kiểm trước khi soak, như `run.sh`).
#
# Bất biến của MỖI boot (fail-closed, không nới):
#   1. đúng 1 `TAP version 13`; đúng 1 `1..18`; đúng **18** dòng `ok N - `, đủ số 1..18,
#      **không** dòng `not ok` nào (KT-269 thêm 3 ca U-mode: 15 → 18);
#   2. đúng 1 `# viFastOs aarch64: boot ok` (marker **nguyên văn**, in sau khi mọi TAP xanh);
#   3. đúng 1 `# conf image: build=<16 hex> text=<len>`, build/text **khớp ELF** (D2-80);
#   4. `# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1` — **bốn** hart (KT-262);
#      kèm bằng chứng từng hart (`# smp: hart=N online|alive`), IPI `# smp: ipi recv=1` đúng một
#      lần, và ba dòng `# tlbi:` (read1 match=1, shootdown acks=3 ok=1, read2 fault=1);
#   5. **KT-269** — đúng **một** dòng `# umode: el0 ran svc=<n> ret=<v>` với `svc=0` (imm16 của
#      `svc #0`) và `ret > 0` (DebugPutStr trả độ dài chuỗi đã in): bằng chứng EL0 **thật sự** chạy
#      và `svc` được xử lý. Ảnh chỉ chạy EL1 (không `eret` xuống EL0) **không** có dòng này ⇒ ĐỎ
#      dù TAP 1..18 vẫn xanh;
#   6. **không** dòng `PANIC` nào (tính cả dòng thụt đầu dòng — fail-closed);
#   7. quá **20 s** ⇒ `kill` đúng PID, boot đó **đỏ** (không retry).
#
# Khác `soak.sh` riscv64 **chỉ ở phần danh tính ảnh**: ảnh aarch64 **chưa** in dòng `# build:`
# của KT-86 (nợ ghi ở `v2/docs/k4-aarch64-boot.md` §7), nên cổng danh tính ở đây là
# `# conf image:` + sha256 ảnh + `git HEAD` ghi trong header. **Không** nới bất biến nào ở trên.
#
# Dùng:
#   bash v2/test/boot/soak-aarch64.sh [NBOOT] [LANES]     # mặc định 1000 3 (đúng cổng K4')
#   bash v2/test/boot/soak-aarch64.sh --smoke N [LANES]   # N < 1.000: chỉ để đỏ trước/kiểm marker;
#                                                         # LUÔN exit 1 (không phải bằng chứng cổng)
#   bash v2/test/boot/soak-aarch64.sh --selftest          # kiểm bộ đọc log trên log tổng hợp:
#                                                         # 1 ca dương + 16 ca âm, KHÔNG mở QEMU
#   bash v2/test/boot/soak-aarch64.sh --help
#
# Biến môi trường: `SOAK_AARCH64_LOGDIR` (mặc định `$TMPDIR/k4-aarch64-soak`).
#
# Exit: 0 = 0 lỗi/N (chỉ ở chế độ cổng, N ≥ 1.000); 1 = có boot đỏ / ảnh đổi giữa soak;
#       2 = lỗi cấu hình (thiếu tool, N < 1.000, LANES ngoài [1,3], build/budget hỏng);
#       3 = TỪ CHỐI ĐO (ảnh không in `# conf image:` khớp ELF) — 0 boot được đếm.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget-aarch64.sh"
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
TARGET="aarch64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-aarch64"
LOGDIR="${SOAK_AARCH64_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/k4-aarch64-soak}"
QEMU_MIN_MAJOR=9 # cùng sàn với riscv64 (D2-72 Q2): QEMU >= 9.2
QEMU_MIN_MINOR=2
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
P_NUM=3        # p = 0,3 % = 3/1000 (D2-24)
P_DEN=1000
TAP_TESTS=18   # KT-269: 18 ca (KT-262 15 + 3 ca U-mode; khớp `1..18` của ảnh và `EXPECT_TESTS` của runner)
SMOKE=0
SELFTEST=0

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "soak-aarch64.sh: $*" >&2
    exit 2
}

usage() {
    sed -n 's/^# \{0,1\}//; 2,47p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,60p'
    echo "soak-aarch64.sh: dùng: soak-aarch64.sh [NBOOT] [LANES] | soak-aarch64.sh --smoke N [LANES] | soak-aarch64.sh --selftest | soak-aarch64.sh --help"
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

    # (1) khung TAP — đúng **một** dòng mỗi mẫu, đủ số 1..18, không `not ok`.
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
    n="$(grep -Fxc '# viFastOs aarch64: boot ok' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# marker '# viFastOs aarch64: boot ok' xuất hiện ${n:-0} lần, cần đúng 1"
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

    # (4) bốn hart + bằng chứng từng hart + ba dòng `# tlbi:` (cùng bộ với `run-aarch64.sh`).
    grep -Eq '^# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 ' "$log" || {
        echo "# thiếu '# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1'"
        bad=1
    }
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
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    for line in \
        "# tlbi: hart=1 read1=" \
        "# tlbi: shootdown " \
        "# tlbi: hart=1 read2 "; do
        n="$(grep -Fc "$line" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    grep -F "# tlbi: hart=1 read1=" "$log" | grep -Fq "match=1" || {
        echo "# dòng read1 của hart 1 không có 'match=1' (hart 1 chưa nạp TLB)"
        bad=1
    }
    grep -F "# tlbi: shootdown " "$log" | grep -Fq "acks=3" || {
        echo "# dòng shootdown không có 'acks=3' (thiếu ack)"
        bad=1
    }
    grep -F "# tlbi: shootdown " "$log" | grep -Fq "ok=1" || {
        echo "# dòng shootdown không có 'ok=1'"
        bad=1
    }
    grep -F "# tlbi: hart=1 read2 " "$log" | grep -Fq "fault=1" || {
        echo "# dòng read2 của hart 1 không có 'fault=1' (TLB của hart 1 chưa bị xoá)"
        bad=1
    }

    # (5) KT-269 — bằng chứng U-mode: đúng **một** dòng `# umode: el0 ran svc=<n> ret=<v>`, với
    #     `svc=0` (imm16 của `svc #0`) và `ret > 0` (DebugPutStr trả độ dài chuỗi đã in). Dòng này
    #     chỉ in khi EL0 **thật sự** chạy và handler `svc` đọc được số hiệu từ `x8` của EL0; ảnh chỉ
    #     chạy EL1 (không `eret`) **không** có ⇒ ĐỎ, dù TAP 1..18 vẫn xanh. Cùng bộ kiểm với
    #     `run-aarch64.sh` (`check_umode`) — không có đường thứ hai lệch kỳ vọng.
    n="$(grep -cE '^# umode: el0 ran svc=[0-9]+ ret=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# umode: el0 ran svc=<n> ret=<v>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -Eq '^# umode: el0 ran svc=0 ret=[1-9][0-9]*$' "$log" || {
        echo "# dòng '# umode: el0 ran svc=0 ret=<len>' không đúng (imm16 phải 0, ret > 0)"
        bad=1
    }

    # (6) không PANIC nào (in cả dòng thụt đầu dòng — fail-closed).
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
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/soak-a64-selftest.XXXXXX")" || die "không tạo được thư mục tạm"
    good="$tmp/good.log"
    cat >"$good" <<'EOF'
TAP version 13
1..18
# conf image: build=0123456789abcdef text=37044
ok 1 - pl011: init sequence applied
ok 2 - vector: 16 slots, VBAR_EL1 installed
ok 3 - brk #0: ec=0x3c, elr matches instruction
ok 4 - mmu: SCTLR_EL1.M=1 after ISB
ok 5 - mmu: identity marker + non-identity probe VA
ok 6 - high: PC at high VA, translate matches PA
ok 7 - physmap: read PA via physmap window
ok 8 - vspace: map Frame 4 KiB + walk + activate + read
ok 9 - vspace: read after unmap faults with FaultKind::Vm
ok 10 - smp: hart 1 online
ok 11 - smp: ipi 0->1
ok 12 - smp: all harts report
ok 13 - tlbi: hart 1 sees unmap
ok 14 - tlbi: ack from all harts
ok 15 - smp: 4 harts online
# smp: hart=1 online
# smp: hart=2 online
# smp: hart=3 online
# smp: ipi recv=1 intid=0
# smp: hart=0 alive
# smp: hart=1 alive
# smp: hart=2 alive
# smp: hart=3 alive
# tlbi: hart=1 read1=0x5653504143450001 magic=0x5653504143450001 match=1 sent=true done=1
# tlbi: shootdown va=0x60000000 targets=0xe expected=3 acks=3 waited=6386 timeout=0 send_failed=0x0 ok=1
# tlbi: hart=1 read2 fault=1 va=0x60000000 access=Read reason=NotMapped esr=0x96000006 far=0x60000000 sent=true done=1 seen=0x3
# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 mpidr=0x80000000,0x80000001,0x80000002,0x80000003
# umode: eret entry=0x70000000 sp=0x70002000 spsr=0x3c0 mode=EL0t code_va=0x70000000 code_pa=0x4008b000 code_bytes=52 stack_va=0x70001000 stack_pa=0x40097000
ok 16 - umode: el0 entered
ok 17 - umode: svc handled
# umode: el0 ran svc=0 ret=12
# umode: svc nr=12 imm=0 elr=0x70000010 ptr=0x70000028 len=12 printed=12 error=0 ret=12
ok 18 - umode: returned to el0
# umode: exit code=0 nr=14 imm=0
# umode: done
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
EOF
    local wb=0123456789abcdef wt=37044

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
    neg "plan sai (1..14)" "$tmp/b.log"

    grep -v '^ok 7 - ' "$good" >"$tmp/c.log" || true
    neg "thiếu 'ok 7'" "$tmp/c.log"

    sed 's/^ok 9 - /not ok 9 - /' "$good" >"$tmp/d.log" || true
    neg "có 'not ok'" "$tmp/d.log"

    grep -v -F '# viFastOs aarch64: boot ok' "$good" >"$tmp/e.log" || true
    neg "thiếu marker" "$tmp/e.log"

    awk '{print} /^# viFastOs aarch64: boot ok$/{print}' "$good" >"$tmp/f.log" || true
    neg "marker in hai lần" "$tmp/f.log"

    sed 's/^# viFastOs aarch64: boot ok$/# viFastOs aarch64: boot ok\nPANIC: giả lập/' \
        "$good" >"$tmp/g.log" || true
    neg "có PANIC" "$tmp/g.log"

    sed 's/^# conf image: build=.*$//' "$good" >"$tmp/h.log" || true
    neg "thiếu '# conf image:'" "$tmp/h.log"

    sed 's/^# conf image: build=[0-9a-f]*/# conf image: build=ffffffffffffffff/' \
        "$good" >"$tmp/i.log" || true
    neg "build lệch ELF" "$tmp/i.log"

    sed 's/ text=37044$/ text=99999/' "$good" >"$tmp/j.log" || true
    neg "text lệch ELF" "$tmp/j.log"

    sed 's/^# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 .*$/# smp: online mask=0x7 alive=0x7 expect=0x7 harts=3 mpidr_ok=1/' \
        "$good" >"$tmp/k.log" || true
    neg "chỉ 3 hart online (mask=0x7)" "$tmp/k.log"

    grep -v -F '# tlbi: shootdown ' "$good" >"$tmp/l.log" || true
    neg "thiếu dòng shootdown" "$tmp/l.log"

    sed 's/acks=3 /acks=2 /' "$good" >"$tmp/m.log" || true
    neg "shootdown thiếu ack (acks=2)" "$tmp/m.log"

    # **KT-269** — ba ca âm cho bất biến U-mode (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# umode: el0 ran ' "$good" >"$tmp/n.log" || true
    neg "thiếu dòng '# umode: el0 ran …' (ảnh không xuống EL0)" "$tmp/n.log"

    sed 's/^# umode: el0 ran svc=0 ret=12$/# umode: el0 ran svc=1 ret=12/' \
        "$good" >"$tmp/o.log" || true
    neg "svc imm16 sai (svc=1 thay vì 0)" "$tmp/o.log"

    sed 's/^# umode: el0 ran svc=0 ret=12$/# umode: el0 ran svc=0 ret=0/' \
        "$good" >"$tmp/p.log" || true
    neg "ret=0 (DebugPutStr không in được gì)" "$tmp/p.log"

    rm -rf "$tmp" 2>/dev/null || true
    echo "# soak-aarch64 --selftest: $((cases - fails))/$cases ca đúng như phải (1 dương + $((cases - 1)) âm)"
    [ "$fails" -eq 0 ] || return 1
    echo "# soak-aarch64 --selftest: PASS"
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
N="${1:-1000}"
LANES="${2:-3}"
case "$N" in '' | *[!0-9]*) die "N phải là số dương (nhận '$N')" ;; esac
case "$LANES" in '' | *[!0-9]*) die "LANES phải là số dương (nhận '$LANES')" ;; esac
[ "$N" -gt 0 ] || die "N phải > 0"
# AGENTS §9: tối đa 3 QEMU song song; D2-24 đúng 3 luồng cho cổng soak.
[ "$LANES" -ge 1 ] && [ "$LANES" -le 3 ] || die "LANES=$LANES ngoài [1,3] (AGENTS §9: tối đa 3 QEMU)"

# D2-24: n ≥ 3/p. p = 3/1000 ⇒ n ≥ ceil(3 · 1000 / 3) = 1000. Số nguyên, không dấu phẩy động.
NEED=$(((3 * P_DEN + P_NUM - 1) / P_NUM))
if [ "$N" -lt "$NEED" ]; then
    if [ "$SMOKE" -ne 1 ]; then
        echo "soak-aarch64.sh: N=$N < n ≥ 3/p = $NEED (p=$P_NUM/$P_DEN = 0,3 %; D2-24, QD §5.3)" >&2
        echo "soak-aarch64.sh: giảm N là NỚI NGƯỠNG — không chạy. Dùng --smoke chỉ để đỏ trước/kiểm marker." >&2
        exit 2
    fi
    echo "# soak: SMOKE N=$N < $NEED — KHÔNG phải bằng chứng cổng K4' (D2-24), luôn exit 1"
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
[ -f "$BUDGET" ] || die "không thấy $BUDGET"
QEMU="$(command -v qemu-system-aarch64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-aarch64"

# QEMU >= 9.2 (cùng sàn riscv64, D2-72 Q2) — thiếu/dưới sàn ⇒ đỏ, không tự hạ.
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

# Cấu hình QEMU **đúng** đường boot của ảnh (k4-aarch64-boot.md §2 + run-aarch64.sh):
# `gic-version=3` bắt buộc từ KT-259 (GICv2 không có redistributor/`ICC_*_EL1`), `-smp 4` từ
# KT-262, `-cpu cortex-a72` như runner. KHÔNG `-icount` (D2-74 điều kiện c).
QEMU_ARGS=(-M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 \
    -kernel "$IMAGE" -no-reboot)

# ------------------------------------------------------------------ build + neo ảnh
echo "# soak-aarch64 KT-265 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2)"
echo "# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)"
echo "# N=$N lanes=$LANES timeout=$((TIMEOUT_DS / 10))s/boot; p=$P_NUM/$P_DEN (0,3 %) ⇒ n ≥ 3/p = $NEED (D2-24)"
if command -v git >/dev/null 2>&1; then
    GIT_HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
    GIT_DIRTY="$(git -C "$REPO_ROOT" status --porcelain -- v2/kernel 2>/dev/null | wc -l | tr -d ' ')"
    echo "# git HEAD=$GIT_HEAD dirty-kernel-paths=$GIT_DIRTY"
else
    GIT_HEAD=unknown
    GIT_DIRTY=unknown
fi

mkdir -p "$LOGDIR" 2>/dev/null || true
# KT-78/OD-K6-6: boot đi qua `qemu_guard.sh` — process group riêng, timeout kill **cả group**,
# trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). 3 luồng = 3 subshell cùng script.
# shellcheck source=/dev/null
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
INFRA_INVALID=0
INFRA_LOG="$LOGDIR/kt265-infra.log"
qg_infra before "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))

echo "# build ảnh aarch64 ($TARGET, release)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-aarch64 --target "$TARGET" --release \
    >"$LOGDIR/kt265-soak-build.log" 2>&1; then
    tail -20 "$LOGDIR/kt265-soak-build.log" >&2
    die "build ảnh aarch64 FAIL (xem $LOGDIR/kt265-soak-build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

# D2-72 Q5: `.data+.sdata+.bss+.sbss` ≤ 262.144 B — fail nếu vượt, không nới.
bash "$BUDGET" "$IMAGE" product || die "budget ảnh aarch64 VƯỢT trần (D2-72 Q5)"

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
# Ảnh aarch64 **chưa** in `# build:` của KT-86 (nợ ở k4-aarch64-boot.md §7), nên cổng danh tính
# ở đây là **một** boot dò: log phải có **đúng một** `# conf image:` khớp ELF. Không có/lệch ⇒
# TỪ CHỐI ĐO (exit 3), **không** đếm một boot soak nào — thà không đo còn hơn đo ảnh không rõ nguồn.
PROBE_LOG="$LOGDIR/kt265-probe.log"
echo "# probe (D2-80): boot MỘT lần để đọc dòng '# conf image:' của ảnh"
if ! qg_boot "$PROBE_LOG" "$TIMEOUT_DS" "$QEMU" "${QEMU_ARGS[@]}"; then
    echo "# soak-aarch64.sh: TỪ CHỐI ĐO — probe boot không thoát (rc=$QG_LAST_RC, pid=$QG_LAST_PID, log=$PROBE_LOG)" >&2
    exit 3
fi
PROBE_BUILD="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$PROBE_LOG" | head -1)"
PROBE_TEXT="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$PROBE_LOG" | head -1)"
if [ "$PROBE_BUILD" != "$WANT_BUILD" ] || [ "$PROBE_TEXT" != "$WANT_TEXT" ]; then
    echo "# soak-aarch64.sh: TỪ CHỐI ĐO — ảnh in '# conf image: build=${PROBE_BUILD:-none} text=${PROBE_TEXT:-none}', ELF là build=$WANT_BUILD text=$WANT_TEXT (D2-80)" >&2
    echo "# soak-aarch64.sh: ảnh KHÔNG tự khai hash khớp ELF ⇒ không có neo, không đo (xem k4-aarch64-boot.md §7)" >&2
    exit 3
fi
echo "# probe: '# conf image:' khớp ELF (build=$WANT_BUILD text=$WANT_TEXT)"

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
while [ "$lane" -le "$LANES" ]; do
    lane_worker "$lane" "$lane" &
    lane=$((lane + 1))
done
wait
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
echo "# soak: BUILD_ID=$WANT_BUILD sha256=$IMG_SHA git=$GIT_HEAD"
echo "# soak: $OK_COUNT/$N lỗi=$FAIL_COUNT — 0/N chỉ là trần (QD §5.3); không retry (D2-07)"
if [ "$SMOKE" -eq 1 ]; then
    echo "# soak: SMOKE N=$N < $NEED — KHÔNG phải bằng chứng cổng K4'"
    exit 1
fi
[ "$FAIL_COUNT" -eq 0 ] || {
    echo "# soak-aarch64.sh: FAIL ($FAIL_COUNT/$N boot đỏ)"
    exit 1
}
[ "$N" -ge "$NEED" ] || {
    echo "# soak-aarch64.sh: FAIL — N=$N < $NEED (n ≥ 3/p)"
    exit 1
}
echo "# soak: 0/$N lỗi (PASS)"
echo "# soak-aarch64.sh: PASS"
exit 0
