#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-418c** — kịch bản **hardening `-cpu max`** (độc lập, KHÔNG sửa `run-{x86_64,aarch64}.sh`).
#
# # Vì sao có tệp này
#
# Hai runner cũ boot bằng `-cpu qemu64` (x86_64) và `-cpu cortex-a72` (aarch64) — **không** model
# nào có `SMEP`/`SMAP` (x86_64) hay `PAN` (aarch64). Hệ quả đo được:
#
# - `-cpu max` x86_64: `#PF` cô lập `VSpace` của ring 3 **không** khớp hook (handler in `# exc:`
#   chung rồi `halt`) vì `sec_probe::run` (ca âm `SMEP`/`SMAP`) xoá `EXPECT_CR2` — **KT-418c** vá;
# - `-cpu max` aarch64: `msr pan` phát sai mã hoá (`op1 = 0b011`) ⇒ trap `ec=0x00` ⇒ boot chết
#   ngay trước TAP 50 — **KT-418c** vá; trước đó `PAN` **chưa từng** chạy trên mã thật.
#
# Kịch bản này kiểm **đích danh** hành vi hardening trên **phần cứng thật** (QEMU `-cpu max`):
#
# | Leg | Kiểm |
# |---|---|
# | `x86_64 -cpu max` | `SMEP`+`SMAP` bật thật (`cpuid_*=1`, `cr4` bit 20/21), **ca âm thật** `smap_err=0x1`/`smap_cr2`, `smep_err=0x11`/`smep_cr2`, `hardened=1`, `ok 50`; `#PF` cô lập `VSpace` khớp hook (`# vspace: cross fault` + `ok 12`); phủ **preempt ring-3** (`# sched: cpl3 preempted≥1 resumed=1 cs=<đúng cs ring 3>`) |
# | `x86_64 -cpu max,-smap` | đối chứng: cả ảnh chạy **hết** dưới `SMEP` (TAP 50/50 + marker + halt) ⇒ tách bạch "hardening đúng" khỏi nợ `SMAP` bên dưới |
# | `aarch64 -cpu max` | `PAN` bật thật + **ca âm data abort thật** (`# pan: pan_supported=1 … fault_ok=1 no_fault=1 restored=1 ok=1`), `ok 50`, **không** dòng `# trap:` nào, TAP 50/50 + marker + halt |
#
# # Nợ có tên (KHÔNG che bằng ngưỡng)
#
# `-cpu max` x86_64 **chưa** chạy hết boot: sau TAP 13 nó **deadlock** trong
# `IrqMcsLock::lock_irqsave` (khoá `Machine`) khi `CR4.SMAP = 1` — đo được cả `-smp 1` lẫn `-smp 4`,
# còn `-cpu max,-smap` thì **50/50 + marker** (xem `v2/agent/JOURNAL/kt418c-hardening-max.md` §4).
# Đó là lỗi **khác** (ngoài phạm vi KT-418c), đề xuất `KT-418d`. Mặc định kịch bản in
# `# known-blocker:` và vẫn PASS **hợp đồng hardening**; đặt `HARDENING_REQUIRE_FULL=1` để biến
# nợ đó thành **ĐỎ** (dùng khi nợ được đóng).
#
# # Lệnh
#
#     bash v2/test/boot/hardening-max.sh              # 2 leg thật (x86_64, aarch64) — cần QEMU
#     bash v2/test/boot/hardening-max.sh --x64        # chỉ leg x86_64
#     bash v2/test/boot/hardening-max.sh --a64        # chỉ leg aarch64
#     bash v2/test/boot/hardening-max.sh --selftest   # 0 QEMU: ca dương + ca âm cho MỌI phép kiểm
#
# Exit: 0 = hợp đồng hardening đạt; 1 = ĐỎ (in lý do có tên); 2 = dùng sai / thiếu công cụ;
#       3 = `TỪ CHỐI ĐO` (máy bận/thiếu QEMU — T-INFRA, không phải lỗi kernel).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
# Logdir riêng theo phiên (cùng quy ước `lib/env.sh`: cổng tạo `V2_RUN_TMP`, hoặc TMPDIR).
LOGDIR="${HARDENING_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/hardening-max.$$}"
mkdir -p "$LOGDIR" || {
    echo "hardening-max.sh: không tạo được logdir $LOGDIR" >&2
    exit 2
}
X64_CPU="${HARDENING_X64_CPU:-max}"
A64_CPU="${HARDENING_A64_CPU:-max}"
BOOT_SECS="${HARDENING_BOOT_SECS:-25}"
TARGET_X64="x86_64-unknown-none"
TARGET_A64="aarch64-unknown-none"
export PATH="$HOME/.cargo/bin:$PATH"

# ---------------------------------------------------------------- phép kiểm log (thuần, dùng lại
# cho cả `--selftest`). `check_x64 <log>` / `check_a64 <log>` in lý do ĐỎ ra stderr, trả 0/1.
#
# Mọi phép kiểm bám **giá trị đo được** trong log, không bám câu chữ trang trí. Riêng `PAN`:
# `pan_supported=0 … ok=1` (CPU không có PAN) **KHÔNG** được tính là kiểm PAN — đó chính là kiểu
# "xanh giả" mà `-cpu max` phát hiện (bản KT-418 chỉ chạy `cortex-a72`, TAP 50 xanh vô nghĩa).

fail_reason=""

# Ghi lý do ĐỎ ra **stderr** rồi trả 1 — ca âm của `--selftest` in được lý do có tên.
_deny() {
    fail_reason="$1"
    echo "$fail_reason" >&2
    return 1
}

_need() { # <log> <mô tả> <regex ERE>
    if ! grep -Eq -- "$3" "$1"; then
        _deny "$2 (thiếu khớp: $3)"
        return 1
    fi
    return 0
}

# Giá trị một trường `k=v` trên dòng khớp <regex>. Rỗng ⇔ không thấy.
_field() { # <log> <regex dòng> <tên trường>
    grep -E -- "$2" "$1" | tail -1 | tr ' ' '\n' | sed -n "s/^$3=//p" | head -1
}

check_hardening_x64() { # <log> [require_full] [smap_required]
    local log="$1" require_full="${2:-0}" smap_required="${3:-1}" cs_umode cs_cpl3 tap marker halt cr4
    # (1) SMEP + SMAP **bật thật** + hai ca âm **thật** (không phải "hardened nhờ tắt").
    #     `smap_required=0` = leg **đối chứng** `-cpu max,-smap` (bỏ phần SMAP; vẫn đòi SMEP thật).
    if [ "$smap_required" = "1" ] && ! grep -Eq '^# smep/smap: va=0x[0-9a-f]+ cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 smap_deferred=0 .*hardened=1$' "$log"; then
        _deny "thiếu dòng '# smep/smap:' đầy đủ với cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 smap_deferred=0 hardened=1"
        return 1
    fi
    cr4="$(_field "$log" '^# smep/smap: ' cr4)"
    if [ "$smap_required" = "1" ]; then
        if ! grep -Eq '^# smep/smap: va=0x[0-9a-f]+ cpuid_smep=1 .* smep=1 .*smep_err=0x11 smep_cr2=0x[0-9a-f]+ smep_ok=1 hardened=1$' "$log"; then
            _deny "thiếu dòng '# smep/smap:' với SMEP bật thật + ca âm thật (leg đối chứng)"
            return 1
        fi
        if [ -z "$cr4" ] || [ $(( cr4 & 0x100000 )) -ne $((0x100000)) ]; then
            _deny "cr4 không bật SMEP(bit20) ở leg đối chứng: '$cr4'"
            return 1
        fi
        if [ "$(_field "$log" '^# smep/smap: ' smep_err)" != "0x11" ]; then
            _deny "ca âm SMEP không thật ở leg đối chứng: smep_err=$(_field "$log" '^# smep/smap: ' smep_err)"
            return 1
        fi
    else
        # --- leg **đối chứng** (`-cpu max,-smap`): đòi SMEP thật, bỏ các assert SMAP.
        if ! grep -Eq '^# smep/smap: va=0x[0-9a-f]+ cpuid_smep=1 .* smep=1 .*smep_err=0x11 smep_cr2=0x[0-9a-f]+ smep_ok=1 hardened=1$' "$log"; then
            _deny "thiếu dòng '# smep/smap:' với SMEP bật thật + ca âm thật (leg đối chứng)"
            return 1
        fi
        if [ -z "$cr4" ] || [ $(( cr4 & 0x100000 )) -ne $((0x100000)) ]; then
            _deny "cr4 không bật SMEP(bit20) ở leg đối chứng: '$cr4'"
            return 1
        fi
        if [ "$(_field "$log" '^# smep/smap: ' smep_err)" != "0x11" ]; then
            _deny "ca âm SMEP không thật ở leg đối chứng: smep_err=$(_field "$log" '^# smep/smap: ' smep_err)"
            return 1
        fi
    fi
    # --- nhánh **đầy đủ** (cả SMAP): `cr4` có **cả** bit 20 (SMEP) và bit 21 (SMAP) + ca âm SMAP.
    if [ "$smap_required" = "1" ]; then
        if [ -z "$cr4" ] || [ $(( cr4 & 0x300000 )) -ne $((0x300000)) ]; then
            _deny "cr4 không bật đủ SMEP(bit20)+SMAP(bit21): '$cr4'"
            return 1
        fi
        if [ "$(_field "$log" '^# smep/smap: ' smap_err)" != "0x1" ]; then
            _deny "ca âm SMAP không thật: smap_err=$(_field "$log" '^# smep/smap: ' smap_err) (cần 0x1)"
            return 1
        fi
        [ "$(_field "$log" '^# smep/smap: ' smap_ok)" = "1" ] || {
            _deny "smap_ok != 1"
            return 1
        }
    fi
    if [ "$(_field "$log" '^# smep/smap: ' smep_err)" != "0x11" ]; then
        _deny "ca âm SMEP không thật: smep_err=$(_field "$log" '^# smep/smap: ' smep_err) (cần 0x11)"
        return 1
    fi
    [ "$(_field "$log" '^# smep/smap: ' smep_ok)" = "1" ] || {
        _deny "smep_ok != 1"
        return 1
    }
    # (2) TAP 50 (hợp đồng đếm của ảnh) — kiểm **đích danh** ca hardening.
    _need "$log" "thiếu 'ok 50 - smep/smap: bat va ca am'" '^ok 50 - smep/smap: bat va ca am$' || return 1
    # (3) KT-418c: `#PF` cô lập `VSpace` **khớp hook** (đây là lỗ `-cpu max` đã vá).
    _need "$log" "thiếu '# vspace: cross fault … user=1' (hook cô lập VSpace không khớp ⇒ lỗi KT-418b)" \
        '^# vspace: cross fault vec=14 err=0x6 cr2=0x[0-9a-f]+ cs=0x[0-9a-f]+ rip=0x[0-9a-f]+ user=1$' || return 1
    _need "$log" "thiếu 'ok 12 - umode: ring3 entered'" '^ok 12 - umode: ring3 entered$' || return 1
    # (4) KT-418c: phủ **preempt ring-3** — số tick ≥ 1, đã resume, và `cs` của frame **đúng** là
    # selector ring 3 mà `# umode: iretq` công bố (không hardcode `0x1b`/`0x23`: `KT-417c` sẽ đổi).
    _need "$log" "thiếu dòng '# sched: cpl3 preempted=… resumed=1 cs=0x…'" \
        '^# sched: cpl3 preempted=[1-9][0-9]* resumed=1 cs=0x[0-9a-f]+$' || return 1
    cs_umode="$(grep -E '^# umode: iretq ' "$log" | tail -1 | tr ' ' '\n' | sed -n 's/^cs=//p')"
    cs_cpl3="$(grep -E '^# sched: cpl3 ' "$log" | tail -1 | tr ' ' '\n' | sed -n 's/^cs=//p')"
    if [ -z "$cs_umode" ] || [ "$cs_umode" != "$cs_cpl3" ]; then
        _deny "cs của tick CPL3 ($cs_cpl3) khác selector ring 3 công bố ($cs_umode)"
        return 1
    fi
    # (5) Kết thúc boot — **nợ có tên** nếu chưa đạt (xem header).
    tap="$(grep -c '^ok ' "$log")"
    marker="$(grep -c '^# viFastOs x86_64: boot ok$' "$log")"
    halt="$(grep -c '^# viFastOs x86_64: halt$' "$log")"
    if [ "$tap" -ne 50 ] || [ "$marker" -ne 1 ] || [ "$halt" -ne 1 ]; then
        if [ "$require_full" = "1" ]; then
            _deny "boot chưa hoàn tất (ok=$tap/50 marker=$marker halt=$halt) — nợ SMAP KT-418d, HARDENING_REQUIRE_FULL=1"
            return 1
        fi
        echo "# known-blocker: x86_64 -cpu max dừng sau TAP $tap/50 (marker=$marker) — deadlock khoá 'Machine' khi CR4.SMAP=1; nợ KT-418d (đối chứng: -cpu max,-smap đạt 50/50)"
    fi
    return 0
}

check_hardening_a64() { # <log>
    local log="$1" line tap notok marker halt
    # (0) Không có trap **không rõ lý do** — bản KT-418 trap đúng `msr pan` sai mã hoá ở đây
    # (`# trap: slot=4 off=0x200 ec=0x00 …`). Lưu ý: `# trap: other=0` của ảnh là **bộ đếm**
    # (`main.rs:563` in số trap khác; `0` = tốt) nên **không** nằm trong phép kiểm này.
    if grep -Eq '^# trap: .*ec=0x00' "$log"; then
        _deny "có trap không rõ lý do (ec=0x00, mã hoá lệnh sai): $(grep -E '^# trap: .*ec=0x00' "$log" | head -1)"
        return 1
    fi
    # (1) `PAN` **có thật trên CPU** và **ca âm data abort thật** — `pan_supported=0 … ok=1` KHÔNG
    # được tính (đó là xanh giả: runner cũ `cortex-a72` chỉ chạy nhánh này).
    line="$(grep -E '^# pan: ' "$log" | tail -1)"
    if [ -z "$line" ]; then
        _deny "thiếu dòng '# pan:' (TAP 50 không chạy tới)"
        return 1
    fi
    case "$line" in
        *pan_supported=0*)
            _deny "CPU không có PAN (pan_supported=0) — phép kiểm PAN là xanh giả, cần '-cpu $A64_CPU'"
            return 1
            ;;
    esac
    local field
    for field in 'pan_supported=1' 'fault_ok=1' 'no_fault=1' 'restored=1' 'ok=1'; do
        case " $line " in
            *" $field "*) ;;
            *)
                _deny "dòng '# pan:' thiếu '$field': $line"
                return 1
                ;;
        esac
    done
    _need "$log" "thiếu 'ok 50 - pan: el1 khong doc trang el0'" '^ok 50 - pan: el1 khong doc trang el0$' || return 1
    # (2) TAP 50/50 + marker đúng 1 lần + halt (không có TAP đỏ nào).
    tap="$(grep -c '^ok ' "$log")"
    notok="$(grep -c '^not ok' "$log")"
    marker="$(grep -c '^# viFastOs aarch64: boot ok$' "$log")"
    halt="$(grep -c '^# viFastOs aarch64: halt$' "$log")"
    if [ "$notok" -ne 0 ]; then
        _deny "có $notok TAP đỏ: $(grep -E '^not ok' "$log" | head -1)"
        return 1
    fi
    if [ "$tap" -ne 50 ]; then
        _deny "TAP xanh $tap != 50"
        return 1
    fi
    if [ "$marker" -ne 1 ]; then
        _deny "marker 'boot ok' xuất hiện $marker lần (cần đúng 1)"
        return 1
    fi
    if [ "$halt" -ne 1 ]; then
        _deny "thiếu/khác 1 dòng halt ($halt)"
        return 1
    fi
    return 0
}

# ---------------------------------------------------------------- --selftest (0 QEMU)
#
# Mỗi phép kiểm phải **đỏ** trên log bị làm hỏng đúng chỗ và **xanh** trên log tốt. Nhờ vậy
# kịch bản này không thể là "dấu cao su": sửa một trường bằng chứng ⇒ ĐỎ có tên.
selftest() {
    local tmp ca=0 fails=0 good log
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/hardening-selftest.XXXXXX")" || return 2
    _ok() { ca=$((ca + 1)); echo "ok $ca - $1"; }
    _bad() { ca=$((ca + 1)); fails=$((fails + 1)); echo "not ok $ca - $1"; }

    # ---- log x86_64 "tốt" (đủ mọi mẫu mà check_hardening_x64 đòi), 50 dòng `ok`.
    good="$tmp/x64-good.log"
    {
        echo 'TAP version 13'
        echo '1..50'
        echo '# umode: iretq entry=0x100000000f sp=0x1000002000 rflags=0x202 cs=0x1b ss=0x23 code_va=0x1000000000'
        echo '# smep/smap: va=0x1000000000 cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 smap_deferred=0 cr4=0x0000000000300020 smap_err=0x1 smap_cr2=0x1000000000 smap_ok=1 smep_err=0x11 smep_cr2=0x1000000000 smep_ok=1 hardened=1'
        echo 'ok 50 - smep/smap: bat va ca am'
        echo '# vspace: cross fault vec=14 err=0x6 cr2=0x1000003000 cs=0x1b rip=0x1000000027 user=1'
        echo 'ok 12 - umode: ring3 entered'
        echo '# sched: cpl3 preempted=1 resumed=1 cs=0x1b'
        for i in $(seq 1 50); do
            case $i in 12 | 50) continue ;; esac
            echo "ok $i - ca $i"
        done
        echo '# viFastOs x86_64: boot ok'
        echo '# viFastOs x86_64: halt'
    } >"$good"
    if check_hardening_x64 "$good" 1 2>"$tmp/err"; then _ok "x64: log tốt (đủ bằng chứng + boot hoàn tất) ⇒ XANH"; else _bad "x64: log tốt bị ĐỎ oan ($(cat "$tmp/err"))"; fi
    if check_hardening_x64 "$good" 0 2>"$tmp/err"; then _ok "x64: log tốt (mặc định) ⇒ XANH"; else _bad "x64: log tốt (mặc định) bị ĐỎ oan ($(cat "$tmp/err"))"; fi

    # chế độ **đối chứng** (`smap_required=0`): log có SMAP tắt vẫn phải XANH khi `require_full=1`.
    sed 's/smap=1 smap_deferred=0/smap=0 smap_deferred=0/; s/cr4=0x0000000000300020/cr4=0x0000000000100020/; s/smap_err=0x1/smap_err=0x0/' \
        "$good" >"$tmp/x64-control.log" 2>/dev/null
    if check_hardening_x64 "$tmp/x64-control.log" 1 0 2>"$tmp/err"; then
        _ok "x64: log đối chứng (SMAP tắt, SMEP thật) ⇒ XANH ở chế độ đối chứng"
    else
        _bad "x64: log đối chứng bị ĐỎ oan ($(cat "$tmp/err"))"
    fi

    # ---- từng ca âm: sửa **một** chỗ, phải ĐỎ.
    mk() { # <tên> <biểu thức sed>
        log="$tmp/x64-$1.log"
        sed "$2" "$good" >"$log" 2>/dev/null
        if check_hardening_x64 "$log" 1 >"$tmp/out" 2>"$tmp/err"; then
            _bad "x64 ca âm ($1): sửa '$2' mà vẫn XANH"
        else
            _ok "x64 ca âm ($1): ĐỎ — $(head -1 "$tmp/err")"
        fi
    }
    mk 'smap-tat' 's/smap=1 smap_deferred=0/smap=0 smap_deferred=0/'
    mk 'smap-deferred' 's/smap_deferred=0/smap_deferred=1/'
    mk 'smap-err-0' 's/smap_err=0x1/smap_err=0x0/'
    mk 'smep-err-sai' 's/smep_err=0x11/smep_err=0x1/'
    mk 'cr4-thieu-smap' 's/cr4=0x0000000000300020/cr4=0x0000000000100020/'
    mk 'hardened-0' 's/hardened=1$/hardened=0/'
    mk 'mat-cross-fault' '/^# vspace: cross fault/d'
    mk 'mat-ok-12' '/^ok 12 - umode/d'
    mk 'cpl3-preempted-0' 's/preempted=1 resumed=1/preempted=0 resumed=0/'
    mk 'cpl3-cs-lech' 's/preempted=1 resumed=1 cs=0x1b/preempted=1 resumed=1 cs=0x23/'
    mk 'marker-2-lan' 's/^# viFastOs x86_64: halt$/# viFastOs x86_64: boot ok\n# viFastOs x86_64: halt/'
    mk 'thieu-halt' '/^# viFastOs x86_64: halt/d'

    # ---- aarch64: log tốt + ca âm.
    good="$tmp/a64-good.log"
    {
        echo 'TAP version 13'
        echo '1..50'
        echo '# pan: va=0x70000000 pan_supported=1 ec=0x25 far=0x70000000 elr=0x4008f898 fault_ok=1 pan_off_read=0x0000000000000000 no_fault=1 restored=1 ok=1'
        echo 'ok 50 - pan: el1 khong doc trang el0'
        for i in $(seq 1 49); do echo "ok $i - ca $i"; done
        echo '# viFastOs aarch64: boot ok'
        echo '# viFastOs aarch64: halt'
    } >"$good"
    if check_hardening_a64 "$good" 2>"$tmp/err"; then _ok "a64: log tốt (PAN thật + boot hoàn tất) ⇒ XANH"; else _bad "a64: log tốt bị ĐỎ oan ($(cat "$tmp/err"))"; fi
    mk64() {
        log="$tmp/a64-$1.log"
        sed "$2" "$good" >"$log" 2>/dev/null
        if check_hardening_a64 "$log" >"$tmp/out" 2>"$tmp/err"; then
            _bad "a64 ca âm ($1): sửa '$2' mà vẫn XANH"
        else
            _ok "a64 ca âm ($1): ĐỎ — $(head -1 "$tmp/err")"
        fi
    }
    mk64 'pan-khong-co-cpu' 's/pan_supported=1/pan_supported=0/'
    mk64 'pan-fault-ok-0' 's/fault_ok=1/fault_ok=0/'
    mk64 'pan-no-fault-0' 's/no_fault=1/no_fault=0/'
    mk64 'pan-restored-0' 's/restored=1/restored=0/'
    mk64 'pan-ok-0' 's/ ok=1$/ ok=0/'
    mk64 'co-trap' 's/^# viFastOs aarch64: halt$/# trap: slot=4 off=0x200 ec=0x00 il=1 far=0x1 elr=0x2 spsr=0x3c5\n# viFastOs aarch64: halt/'
    mk64 'thieu-ok-50' '/^ok 50 - pan/d'
    mk64 'tap-do' 's/^ok 49 - ca 49$/not ok 49 - ca 49/'
    mk64 'thieu-marker' '/^# viFastOs aarch64: boot ok/d'

    echo "# hardening-max --selftest: $((ca - fails))/$ca ca đúng như phải"
    rm -rf "$tmp"
    [ "$fails" -eq 0 ]
}

# ---------------------------------------------------------------- khởi động QEMU
command -v cargo >/dev/null 2>&1 || {
    echo "# hardening-max: không có cargo" >&2
    exit 2
}
build_image() { # <crate> <target> <log>
    cargo build --manifest-path "$MANIFEST" -p "$1" --target "$2" --release --features ktest \
        >"$3" 2>&1 || {
        tail -20 "$3" >&2
        echo "# hardening-max: build $1 ($2) FAIL — xem $3" >&2
        return 1
    }
}

boot_x64() { # <log> <cpu>
    local log="$1" cpu="$2" qemu disk pid i
    qemu="$(command -v qemu-system-x86_64 || true)"
    [ -n "$qemu" ] || {
        echo "# hardening-max: không có qemu-system-x86_64" >&2
        return 2
    }
    disk="$LOGDIR/disk-x64.raw"
    if [ ! -f "$disk" ]; then
        dd if=/dev/zero of="$disk" bs=512 count=8 2>/dev/null
        printf 'VIFASTOS-KT288-S0' | dd of="$disk" bs=1 seek=0 conv=notrunc 2>/dev/null
    fi
    rm -f "$log"
    "$qemu" -M q35 -cpu "$cpu" -m 128M -nographic -no-reboot -smp 4 \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -drive "file=$disk,if=none,id=d0,format=raw" -device virtio-blk-pci,drive=d0 \
        -kernel "$LOGDIR/vifast-kbin-ktest-x86_64" >"$log" 2>&1 &
    pid=$!
    # Chờ marker rồi mới chờ thoát (không `sleep` mù): ảnh `halt` ⇒ QEMU tự thoát.
    for i in $(seq 1 $((BOOT_SECS * 10))); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    echo "# boot x86_64 cpu=$cpu log=$log lines=$(wc -l <"$log" | tr -d ' ')"
    return 0
}

# **Không** dùng `$(start_tcp_server …)`: command substitution chạy hàm trong **subshell** ⇒
# tiến trình `python3 &` mất cha và bị dọn ⇒ khách nhận `err=Io elapsed_ms=1` (đo được lần đầu).
# Hàm đặt PID vào `TCP_SRV_PID` (biến của shell chính) và trả 0/1.
TCP_SRV_PID=""
start_tcp_server() { # <port> <log>
    local port="$1" log="$2" i
    rm -f "$log"
    python3 - "$port" "$log" >"$LOGDIR/tcp-server.out" 2>&1 <<'PY' &
import socket, sys
port = int(sys.argv[1]); log = sys.argv[2]
def note(l):
    with open(log, "a") as h: h.write(l + "\n")
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
try:
    s.bind(("127.0.0.1", port))
except OSError as e:
    note("bind_err=%s" % e); raise SystemExit(2)
s.listen(1); note("listen")
s.settimeout(60.0)
try:
    c, a = s.accept()
except socket.timeout:
    note("accept_timeout"); raise SystemExit(3)
note("accept")
c.sendall(b"vifast-tcp-ok"); note("sent")
c.settimeout(5.0); total = 0
try:
    while True:
        d = c.recv(4096)
        if not d: break
        total += len(d)
except socket.timeout:
    note("recv_timeout")
note("recv=%d" % total); c.close(); s.close()
PY
    TCP_SRV_PID=$!
    for i in $(seq 1 50); do
        grep -q '^listen' "$log" 2>/dev/null && return 0
        sleep 0.1
    done
    kill "$TCP_SRV_PID" 2>/dev/null
    TCP_SRV_PID=""
    return 1
}

boot_a64() { # <log> <cpu>
    local log="$1" cpu="$2" qemu disk pid i sock srv kbd
    qemu="$(command -v qemu-system-aarch64 || true)"
    [ -n "$qemu" ] || {
        echo "# hardening-max: không có qemu-system-aarch64" >&2
        return 2
    }
    disk="$LOGDIR/disk-a64.raw"
    if [ ! -f "$disk" ]; then
        dd if=/dev/zero of="$disk" bs=512 count=8 2>/dev/null
        printf 'VIFASTOS-KT273-S0' | dd of="$disk" bs=1 seek=0 conv=notrunc 2>/dev/null
    fi
    start_tcp_server 45678 "$LOGDIR/tcp-server.log" || {
        echo "# hardening-max: không dựng được TCP server (port 45678) — T-INFRA" >&2
        return 3
    }
    srv="$TCP_SRV_PID"
    sock="$LOGDIR/monitor.sock"
    rm -f "$sock"
    rm -f "$log"
    "$qemu" -M virt,gic-version=3 -cpu "$cpu" -m 128M -nographic -smp 4 \
        -kernel "$LOGDIR/vifast-kbin-ktest-aarch64" -no-reboot -nic none \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -drive "file=$disk,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0 -device virtio-gpu-pci -device virtio-keyboard-pci \
        -monitor "unix:$sock,server=on,wait=off" >"$log" 2>&1 &
    pid=$!
    # Bàn phím: TAP 32 cần `sendkey a` trong cửa sổ 5 s của ảnh ⇒ dùng lib dùng chung (KT-407).
    python3 "$HERE/lib/monitor-sendkey.py" "$sock" "$log" 8000 "$LOGDIR/kbd.log" 1 \
        >"$LOGDIR/kbd.out" 2>&1 &
    kbd=$!
    for i in $(seq 1 $((BOOT_SECS * 10))); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    kill "$kbd" 2>/dev/null
    kill "$srv" 2>/dev/null
    wait "$kbd" "$srv" 2>/dev/null
    echo "# boot aarch64 cpu=$cpu log=$log lines=$(wc -l <"$log" | tr -d ' ')"
    return 0
}

# ---------------------------------------------------------------- chạy thật
run_x64() {
    local rc=0
    echo "== KT-418c hardening: x86_64 -cpu $X64_CPU =="
    build_image vifast-kbin-x86_64 "$TARGET_X64" "$LOGDIR/build-x64.log" || return 2
    cp "$V2_ROOT/kernel/target/$TARGET_X64/release/vifast-kbin-x86_64" \
        "$LOGDIR/vifast-kbin-ktest-x86_64" 2>/dev/null ||
        cp "${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET_X64/release/vifast-kbin-x86_64" \
            "$LOGDIR/vifast-kbin-ktest-x86_64" || {
        echo "# hardening-max: không copy được ảnh ktest x86_64" >&2
        return 2
    }
    boot_x64 "$LOGDIR/x64-max.log" "$X64_CPU" || return 2
    if check_hardening_x64 "$LOGDIR/x64-max.log" "${HARDENING_REQUIRE_FULL:-0}"; then
        echo "ok 1 - x86_64 -cpu $X64_CPU: SMEP/SMAP bật thật + 2 ca âm thật + cross fault + preempt CPL3"
    else
        echo "not ok 1 - x86_64 -cpu $X64_CPU: $fail_reason"
        rc=1
    fi
    # Đối chứng: cùng ảnh dưới `-cpu $X64_CPU,-smap` ⇒ boot **hết** (tách nợ SMAP khỏi hardening).
    boot_x64 "$LOGDIR/x64-nosmap.log" "$X64_CPU,-smap" || return 2
    if check_hardening_x64 "$LOGDIR/x64-nosmap.log" 1 0; then
        echo "ok 2 - x86_64 -cpu $X64_CPU,-smap: cả ảnh chạy hết 50/50 TAP + marker + halt (đối chứng nợ SMAP)"
    else
        echo "not ok 2 - x86_64 -cpu $X64_CPU,-smap: $fail_reason"
        rc=1
    fi
    return $rc
}

run_a64() {
    local rc=0
    echo "== KT-418c hardening: aarch64 -cpu $A64_CPU =="
    build_image vifast-kbin-aarch64 "$TARGET_A64" "$LOGDIR/build-a64.log" || return 2
    cp "$V2_ROOT/kernel/target/$TARGET_A64/release/vifast-kbin-aarch64" \
        "$LOGDIR/vifast-kbin-ktest-aarch64" 2>/dev/null ||
        cp "${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET_A64/release/vifast-kbin-aarch64" \
            "$LOGDIR/vifast-kbin-ktest-aarch64" || {
        echo "# hardening-max: không copy được ảnh ktest aarch64" >&2
        return 2
    }
    boot_a64 "$LOGDIR/a64-max.log" "$A64_CPU" || return $?
    if check_hardening_a64 "$LOGDIR/a64-max.log"; then
        echo "ok 3 - aarch64 -cpu $A64_CPU: PAN bật thật + data abort thật + 50/50 TAP + marker + halt"
    else
        echo "not ok 3 - aarch64 -cpu $A64_CPU: $fail_reason"
        rc=1
    fi
    return $rc
}

case "${1:-}" in
    --selftest)
        selftest
        exit $?
        ;;
    --x64)
        run_x64
        exit $?
        ;;
    --a64)
        run_a64
        exit $?
        ;;
    '')
        rc=0
        run_x64 || rc=$?
        run_a64 || rc=$?
        echo "# hardening-max: logdir $LOGDIR"
        [ "$rc" -eq 0 ] || exit "$rc"
        exit 0
        ;;
    *)
        echo "hardening-max.sh: tham số lạ '$1' (dùng --selftest|--x64|--a64)" >&2
        exit 2
        ;;
esac
