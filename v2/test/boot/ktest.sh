#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-44..54 — runner boot các ảnh **TEST** riscv64 K3 (crate `vifast-kbin-riscv64`, feature `ktest`).
#
# D2-08: ảnh test KHÔNG nằm trong ảnh sản phẩm — cùng crate nhưng bin + feature riêng. Mỗi boot chỉ
# kết thúc được một lần bằng PANIC có chủ đích ⇒ **năm** ảnh test (KT-66 thêm `ktest-proc`;
# KT-72 thêm `ktest-vblk` — ảnh virtio-blk cần `-drive`):
#   - `ktest`       : 56 TAP — KT-45 (image + trap) + KT-46 (mmu/walk/guard) + KT-48 (PLIC/UART IRQ)
#                     + KT-47 (timer SBI) + KT-49 (SMP 15..19) + KT-50 (IPI 20..24) + KT-51 (TLB/ASID
#                     25..29) + KT-52 (object layer 30..36) + KT-55 (conformance 5 `.scn` 37:
#                     `# trace:` chuẩn hoá) + KT-56 FaultKind/W^X (38..42) + KT-61 core `no_std`
#                     trên nhân thật (43..52: link/untyped/tcb/configure/sched/IPC/notification/
#                     waitaddr/D2-83/invariants) + KT-63 chuyển cap/finalise IPC (53..55) +
#                     trap lồng fatal (56).
#   - `ktest-guard` : 3 TAP — guard page fault **thật**: load `scause=13` (probe resume) rồi store
#                     `scause=15` fatal `PANIC: guard page fault`.
#   - `ktest-user`  : 29 TAP (KT-62 + KT-67a + KT-67b + KT-68 + KT-69) — U-mode + core: nạp `init` ELF (W^X), `sret`, 10 syscall qua
#                     `Machine` thật (DebugGetInfo 15, Yield 8, Exit 14 + 5 lần ngoài dải ⇒ 19), TAP
#                     kernel-level cho bảng 16 syscall/cap sai/IPC/NB/WaitAddr/Clock/Yield (7..13),
#                     `Exit` không Reply (14); **KT-67a** invocation hẹp (15..18: Retype/Map/Unmap/
#                     method lạ ⇒ 18) trên U-mode thật; **KT-67b** chuyển cap Frame giữa hai tiến
#                     trình U-mode (19: memsrv.elf → client.elf, cap trong cspace B, map/ghi/đọc);
#                     **KT-68** capalloc (20..24); **KT-69** IDL + ring (25..29, `ring_server`/`ring_client`);
#                     kết thúc sạch bằng PANIC có chủ đích.
#   - `ktest-proc`  : 10 TAP (KT-65, **tách ảnh KT-66**) — đúng mười ca cũ 15..24 của ảnh user:
#                     procsrv/shell/con U-mode (1..7) + 3 TAP fault/wx (8..10, PANIC `u-mode fault`).
#                     `ktest-user` trả ~40 KiB headroom nhờ bỏ `proc_demo` + 3 ELF demo khỏi ảnh đó.
#
# **KT-54 — nguồn sự thật duy nhất là `v2/test/boot/expected.txt`** (mỗi ảnh một dòng
# `<ảnh> <kế hoạch TAP> <số dòng ok>`): runner **đọc số từ tệp**, không hardcode; `expected.txt` lệch
# với số TAP mà runner biết tên ⇒ **đỏ** (không cho lệch âm thầm). Ngoài ra runner kiểm:
#   1. `TAP version 13` đúng 1 lần, dòng kế hoạch `1..N` đúng 1 lần, **thứ tự** version < kế hoạch < ok đầu;
#   2. dãy `ok` = **1..N liên tục, không trùng, không thiếu** (so cả dãy theo thứ tự) + số dòng ok khớp tệp;
#   3. không có `not ok` nào;
#   4. **đúng 1 dòng `PANIC`, đúng mẫu cho phép của ảnh** (ảnh chính: trap lồng; guard: guard page
#      fault; user: u-mode fault) — PANIC khác mẫu hoặc PANIC thứ hai ⇒ đỏ;
#   5. mỗi TAP N có **đúng một** dòng `ok N - <tên>` khớp tên trong manifest của runner;
#   6. mọi dòng `# evidence` bắt buộc của ảnh đều có (ảnh chính **98** mục, guard **5**, user **55**,
#      proc **28**); KT-66 thêm kiểm **stack syscall per-hart** (`check_stack_lines`: tập hart
#      0..N-1, `base` phân biệt, `low` trong `[base, top]`, `canary=0`);
#   8. **KT-55**: mỗi ca conformance in `# conf: scn=N name=… steps=K` và đúng **K** dòng
#      `# trace: N …` đúng định dạng; ca 24 (2 hart) in `skipped=smp1` trên `-smp 1`;
#   7. `Platform HART Count` của OpenSBI khớp `-smp` (số hart là **máy ảo** nói, không chỉ kernel tự khai).
#
# D2-72 Q2/Q4/Q5/Q10: `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp N`, matrix **1/2/4**
# cho KT-49, budget riêng cho **mỗi** ảnh test (≤ 262.144 B), timeout 20 s/boot, **chỉ** `kill <pid>`
# do chính script tạo (AGENTS §9) — không `pkill -f qemu-system`.
# D2-74: `-icount shift=6` cố định **chỉ** trong runner này (đo định thời tất định); `run.sh`/soak
# KT-57 giữ dòng D2-72 Q2 không icount.
#
# Dùng:
#   bash v2/test/boot/ktest.sh [NBOOT] [SMP|matrix]     # mặc định 1 boot, 1 hart; `matrix` = 1 2 4
#     vd: bash v2/test/boot/ktest.sh 50 matrix          # 50 boot × 4 ảnh × smp 1/2/4 = 600 boot
#   bash v2/test/boot/ktest.sh --check-log <ảnh> <log> [SMP]   # kiểm MỘT log đã có (không build/boot)
#   bash v2/test/boot/ktest.sh --log-path <ảnh> <SMP> [i]      # in đường dẫn log của runner
#   bash v2/test/boot/ktest.sh --print-expected               # in bảng expected.txt đã đọc
#   bash v2/test/boot/ktest.sh --print-evidence <ảnh> [SMP]    # in ordinal + regex từng evidence bắt buộc
#   bash v2/test/boot/ktest.sh --check-rev <log> [<ảnh>]       # KT-86: cổng danh tính bản dựng trên MỘT log
#   bash v2/test/boot/ktest.sh --help
# Biến môi trường: `KT54_LOGDIR` (mặc định `/tmp/k3-ktest`), `KT54_EXPECTED` (chỉ `--check-log`,
# dùng cho `selftest.sh` — vòng boot thật **luôn** đọc `v2/test/boot/expected.txt`).
#
# Exit: 0 = mọi boot sạch (hoặc `--check-log` xanh); 1 = có boot/log đỏ; 2 = lỗi cấu hình môi trường;
#       3 = **TỪ CHỐI ĐO** (KT-86/D2-125.3: dòng `# build:` không có / lệch HEAD / dirty=1) — cổng đỏ
#           trước phép đo, không slot nào chạy (không có `ok`).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget.sh"
# KT-86 (D2-125.3): cổng danh tính bản dựng — **một** nguồn sự thật cho rev/dirty
# (cùng tệp mà `v2/tools/repro-build.sh` và `build.rs` dùng), không chép lại định nghĩa.
. "$HERE/build-rev.sh"
EXPECTED_FILE="$HERE/expected.txt"
LOGDIR="${KT54_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/k3-ktest}"
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
TARGET="riscv64gc-unknown-none-elf"
IMG_DIR="$V2_ROOT/kernel/target/$TARGET/release"
# Ảnh **sản phẩm** (`src/main.rs`) — ảnh duy nhất in dòng `# build:` (KT-86).
PRODUCT_IMAGE="$IMG_DIR/vifast-kbin-riscv64"
LABELS="ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec" # thứ tự in bảng + thứ tự boot trong một vòng
LABEL_COUNT=0                              # số ảnh (đếm từ LABELS; không hardcode 3/4)
TIMEOUT_DS=200                        # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
QEMU_ICOUNT="-icount shift=6"         # D2-74 (shift CỐ ĐỊNH, không `auto`)
QEMU_MIN_MAJOR=9                      # sàn D2-72 Q2: QEMU >= 9.2
QEMU_MIN_MINOR=2

MODE=matrix
# KT-86: đường dẫn ảnh cho `--check-rev` (rỗng ⇒ cổng chỉ kiểm log + HEAD/dirty).
CHECK_IMAGE=""
CHECK_LABEL=""
CHECK_LOG=""
CHECK_SMP=1
LOGP_LABEL=""
LOGP_SMP=1
LOGP_I=1
PRINT_LABEL=""
PRINT_SMP=1
N_BOOT=1
SMP_ARG=1
SMP_LIST="1"
EXPECT_ERR=""
MISSING="" # token "thiếu …" của lần validate_log gần nhất (toàn cục ⇒ không subshell)
SMP_COUNT=0 # số cấu hình SMP thật sự có trong SMP_LIST (chặn "0 boot mà PASS")
RUN_RESULT="" # PASS|FAIL|TIMEOUT của lần run_one gần nhất (để SKIP chỉ khi TIMEOUT)
ROWS=0     # số dòng bảng đã ghi = số "slot" (ảnh, smp, boot) đã xử lý (boot thật + SKIP)


export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "ktest.sh: $*" >&2
    exit 2
}

# KT-78 (OD-K6-6): **mọi** boot đi qua `qemu_guard.sh` — QEMU trong process group riêng, timeout
# giết cả group, trap EXIT/INT/TERM/HUP dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL).
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
INFRA_INVALID=0

usage() {
    sed -n '3,40p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
}

# ---------------------------------------------------------------- tiện ích

log_path() { printf '%s/%s-smp%s-%s.log' "$LOGDIR" "$1" "$2" "$3"; }

image_path() {
    case "$1" in
        ktest) printf '%s/vifast-kbin-ktest' "$IMG_DIR" ;;
        ktest-guard) printf '%s/vifast-kbin-ktest-guard' "$IMG_DIR" ;;
        ktest-user) printf '%s/vifast-kbin-ktest-user' "$IMG_DIR" ;;
        ktest-proc) printf '%s/vifast-kbin-ktest-proc' "$IMG_DIR" ;;
        ktest-vblk) printf '%s/vifast-kbin-ktest-vblk' "$IMG_DIR" ;;
        ktest-con) printf '%s/vifast-kbin-ktest-con' "$IMG_DIR" ;;
        ktest-exec) printf '%s/vifast-kbin-ktest-exec' "$IMG_DIR" ;;
        *) return 1 ;;
    esac
}

known_label() {
    case " $LABELS " in
        *" $1 "*) return 0 ;;
        *) return 1 ;;
    esac
}

# `expected_field <ảnh> <2|3>`: đọc đúng một trường từ expected.txt (nguồn sự thật duy nhất).
expected_field() {
    awk -v label="$1" -v field="$2" '
        /^[[:space:]]*#/ { next }
        /^[[:space:]]*$/ { next }
        $1 == label { print $field; exit }' "$EXPECTED_FILE"
}

# Đọc + kiểm **định dạng** expected.txt. Trả 1 và đặt `EXPECT_ERR` nếu sai (không `die` để
# `--check-log` còn in được `not ok` có chẩn đoán).
load_expected() {
    local out="" l=""
    EXPECT_ERR=""
    LABEL_COUNT="$(printf '%s\n' $LABELS | wc -l | tr -d ' ')"
    [ -f "$EXPECTED_FILE" ] || {
        EXPECT_ERR="không thấy $EXPECTED_FILE"
        return 1
    }
    if ! out="$(awk '
        /^[[:space:]]*#/ { next }
        /^[[:space:]]*$/ { next }
        {
            if (NF != 3) { printf "FMT dòng %d: cần đúng 3 trường [ảnh] [kế hoạch] [số ok], thấy %d trường\n", NR, NF; bad = 1; next }
            if ($2 !~ /^[0-9]+$/ || $3 !~ /^[0-9]+$/) { printf "FMT dòng %d: trường 2/3 phải là số nguyên\n", NR; bad = 1; next }
            if ($2 + 0 < 1 || $3 + 0 < 1) { printf "FMT dòng %d: kế hoạch và số ok phải >= 1\n", NR; bad = 1; next }
            if ($2 + 0 != $3 + 0) { printf "FMT dòng %d: kế hoạch %d khác số ok %d (runner không cho phép TAP đỏ)\n", NR, $2, $3; bad = 1; next }
            if (seen[$1]++) { printf "FMT dòng %d: nhãn %s bị lặp\n", NR, $1; bad = 1; next }
            printf "LINE %s %s %s\n", $1, $2, $3
        }
        END { exit bad }' "$EXPECTED_FILE")"; then
        EXPECT_ERR="$(printf '%s\n' "$out" | grep '^FMT' | head -3 | tr '\n' ';')"
        [ -n "$EXPECT_ERR" ] || EXPECT_ERR="expected.txt sai định dạng"
        return 1
    fi
    for l in $LABELS; do
        if [ -z "$(expected_field "$l" 2)" ]; then
            EXPECT_ERR="expected.txt thiếu dòng cho ảnh [$l] (runner biết: $LABELS)"
            return 1
        fi
    done
    for l in $(printf '%s\n' "$out" | awk '$1 == "LINE" { print $2 }'); do
        if ! known_label "$l"; then
            EXPECT_ERR="expected.txt có nhãn [$l] mà runner chưa biết (runner biết: $LABELS)"
            return 1
        fi
    done
    return 0
}

# Số TAP mà runner **biết tên** cho từng ảnh phải bằng kế hoạch trong expected.txt ⇒ thêm/bớt TAP
# mà chỉ sửa một bên là đỏ ngay (không lệch âm thầm).
check_manifests() {
    local smp="$1" l="" plan="" n=""
    EXPECT_ERR=""
    for l in $LABELS; do
        plan="$(expected_field "$l" 2)"
        n="$(manifest_for "$l" "$smp" | wc -l | tr -d ' ')"
        if [ "${n:-0}" -ne "$plan" ]; then
            EXPECT_ERR="expected.txt: ảnh [$l] kế hoạch $plan nhưng runner kiểm $n TAP — phải sửa cả hai cùng lúc"
            return 1
        fi
    done
    return 0
}

# D2-80: BUILD_ID ảnh = FNV-1a 64 trên section `.text` (cùng hàm băm với kernel).
image_build() {
    local img="$1" out=""
    out="$(python3.12 "$V2_ROOT/test/conformance/image_digest.py" "$img" 2>/dev/null)" || return 1
    printf '%s' "${out%% *}"
}

now_s() { date +%s; }

# --------------------------------------------------- manifest TAP (tên từng TAP)

ipi_name() {
    case "${1:-}" in '' | *[!0-9]*) return 1 ;; esac
    if [ "$1" -ge 2 ]; then printf '100 ipi delivered'; else printf '100 ipi traps (self)'; fi
}

manifest_main() {
    cat <<'EOF'
ktest image
trap breakpoint
trap ecall sbi
trap illegal inject
mmu on
map layout
walk khop
plic window
uart irq
plic no fake irq
timer 100 ticks monotonic
timer no lost tick
timer irq
timer deadline drift
harts alive __SMP__
per-hart stack/guard
per-hart timer
plic ctx per hart
barrier
__IPI20__
ipi drain khi cho ack
self-target khong gui
coalesce khong mat kind
ipi khong nham timer/external
100 vong shootdown Ok
stale mapping caught
hart ngoai active_on
rollover flush
timeout khong treo
retype vspace
untyped byte invariant
frame 3 sizes
wx violation x-then-w
wx violation device+exec
map/unmap 4 KiB
0 ro
__CONF37__
faultkind vm load
faultkind vm store
faultkind vm insn
wx violation qua ABI
device exec
core link
core untyped boot
core tcb retype
core configure resume suspend
core sched 2 tcb
core ipc round
core notification signal
core waitaddr
core d2-83 boundary
core invariants
core cap transfer
core cap message
core revoke finalise
core slot arena
core sched bind
core fault resume
core tlb flush
mem alloc frame
mem revoke frame
mem out of memory
mem bridge cap
mem deny rights
fs rename truncate double-indirect
fs dirty flush write-back
fs journal replay sau crash
fs crash 7 pha nhat quan
fs fsck phat hien va sua
trap nested panic
EOF
}

manifest_guard() {
    cat <<'EOF'
guard image
guard load fault
guard store fault
EOF
}

manifest_user() {
    cat <<'EOF'
init u-mode
syscall debug_put_str
syscall exit
syscall la ma loi
syscall debug_get_info
syscall yield
syscall 16 duong
syscall sai cap
syscall ipc vong
syscall nb va notification
syscall waitaddr wakeaddr
syscall clock read
syscall yield doi tcb
syscall exit khong reply
invocation untyped retype
invocation frame map
invocation frame unmap va fault
invocation ngoai danh sach
ipc capx hai tien trinh
capalloc cap vong
capalloc mint copy move
capalloc revoke cay delete reparent
capalloc ca am
capalloc bat bien
idl stub sinh tu vidl
ring day
ring rong
ring thu tu va wrap
ring race co kiem soat
pager fault den server va nap trang
pager hit khong doc backend
pager eviction LRU khong mat du lieu
pager hai client chia se mot trang
pager page ngoai file tra ma
pager ca am quyen va khong ghi de
pager bat bien reads==misses
pager giao dung mot lan
EOF
}

# KT-66: ảnh demo **tách** khỏi `ktest-user` (`proc_demo` + 3 ELF U-mode + fault phase KT-56).
# Mười TAP dưới đây = đúng mười ca cũ 15..24 của `ktest-user`, **đánh số lại 1..10** trong ảnh
# mới (TAP của một ảnh phải liên tục 1..N); ánh xạ cũ→mới 15→1 … 24→10 ghi ở `expected.txt`
# + `docs/k35-procsrv.md` §10. Tên TAP giữ nguyên.
manifest_proc() {
    cat <<'EOF'
procsrv/shell/con load u-mode
procsrv recv fault exit
shell help echo
shell ps tu core
shell wait ma thoat
procsrv kill treo tcb
shell exit va procsrv finalise
u-mode kernel read fault
faultkind exit
faultkind wx from u
EOF
}

# KT-72: ảnh **virtio-blk** (6 TAP) — thiết bị thật qua `-drive` (xem `boot_image`/`run_one`).
manifest_vblk() {
    cat <<'EOF'
vblk pci
vblk queue
vblk ghi doc lai
vblk flush
vblk fs tren dia
vblk loi ngoai dai
EOF
}

# KT-73: ảnh **console** (5 TAP) — UART RX IRQ → ring → U-mode + virtio-console hai queue.
manifest_con() {
    cat <<'EOF'
con uart irq ring
con u-mode doc ring
con pci
con ghi doc
con ca am
EOF
}

# KT-74: ảnh **`Tcb.SetTls` + `exec(path)`** (6 TAP) — TLS qua invocation + tp quan sát tại trap,
# ca âm, `exec` giữ pid/cspace + không rò, chuỗi 3 bước, thất bại nguyên tử, TLS reset sau exec.
manifest_exec() {
    cat <<'EOF'
tls dat qua invocation
tls ca am
exec giu pid cspace
exec chain 3 buoc
exec that bai nguyen tu
tls reset sau exec
EOF
}

# Tên TAP 37 của KT-55 phụ thuộc smp: `-smp 1` không chạy ca 24 (cần 2 hart thật).
conf37_name() {
    if [ "${1:-1}" -ge 2 ]; then printf 'conformance 5/5'; else printf 'conformance 4/5 + 1 skipped'; fi
}

# `manifest_for <ảnh> <smp>`: danh sách tên TAP theo thứ tự 1..N (đã thay phần phụ thuộc cấu hình).
manifest_for() {
    case "$1" in
        ktest) manifest_main ;;
        ktest-guard) manifest_guard ;;
        ktest-user) manifest_user ;;
        ktest-proc) manifest_proc ;;
        ktest-vblk) manifest_vblk ;;
        ktest-con) manifest_con ;;
        ktest-exec) manifest_exec ;;
        *) return 1 ;;
    esac | sed -e "s|__SMP__|$2|g" -e "s|__IPI20__|$(ipi_name "$2")|g" \
        -e "s|__CONF37__|$(conf37_name "$2")|g"
}

# ------------------------------------------------------ bằng chứng `# …` bắt buộc

# `evidence_for <ảnh> <smp> [hart-boot]`: mỗi dòng là một regex `grep -E` **bắt buộc** phải khớp.
evidence_for() {
    local label="$1" smp="$2" hart="${3:-}" mask tlb_removed tlb_sent ipi_del ipi_drain ipi_coal tlb_mode tlb_peer
    mask="$(printf '%x' $(((1 << smp) - 1)))"
    if [ "$smp" -ge 2 ]; then
        evid24='^# trace: 24 [0-9]+ shootdown va=0x[0-9a-f]+ targets=0x[0-9a-f]+ acked=0x[0-9a-f]+ completed=1 => ok$'
    else
        evid24='^# conf: scn=24 name=vm-tlb steps=0 skipped=smp1$'
    fi
    if [ "$smp" -ge 2 ]; then
        ipi_mode=cross
        ipi_del="sent=100 delivered=100 traps=100 coalesced=0 spurious=0 kinds=34/33/33 timeout=0"
        ipi_drain="mode=cross mutual=1 boot_ok=1 peer_ok=1 traps_delta=0/0 wait_ticks=[0-9]+/[0-9]+ timeout_ended=1"
        ipi_coal="mode=cross posts=6 coalesced=4 sent_delta=2 pending=0x3 peer_bits=0x3 peer_delivered=2 ok=1"
        tlb_mode=cross
        tlb_sent=$((100 * (smp - 1)))
        tlb_peer="received=100 flushes=100 page=100 asid=0 all=0 implicit=0 prime_wrong=0 verify_wrong=0 service_timeout=0"
        tlb_removed=2
        [ "$smp" -eq 2 ] && tlb_removed=1
    else
        ipi_mode=self-ipi
        ipi_del="sent=100 delivered=0 traps=100 coalesced=0 spurious=0 kinds=0/0/0 timeout=0"
        ipi_drain="mode=self-wait mutual=0 boot_ok=0 peer_ok=0 traps_delta=0/0 wait_ticks=[0-9]+/[0-9]+ timeout_ended=1"
        ipi_coal="mode=mailbox posts=6 coalesced=4 sent_delta=0 pending=0x3 peer_bits=0x3 peer_delivered=2 ok=1"
        tlb_mode=self
        tlb_sent=0
        tlb_peer="received=0 flushes=0 page=0 asid=0 all=0 implicit=0 prime_wrong=0 verify_wrong=0 service_timeout=0"
        tlb_removed=1
    fi
    case "$label" in
        ktest) cat <<'EOF'
^# obj init: arena=0x[0-9a-f]+-0x[0-9a-f]+ bytes=[0-9]+ untyped=2 window=0x40000000 pa0=0x[0-9a-f]+ pa1=0x80000000 physmap=1$
^# retype: vspace=1 root=0x[0-9a-f]+ asid=[1-9][0-9]* pt=3 frame=1 cap_ok=1 walk_empty=1$
^# untyped bytes: cap=[0-9]+ used0=[0-9]+ after_retype=[0-9]+ after_revoke=[0-9]+ free0=[0-9]+ free1=[0-9]+ invariant=1 atomic=1 oom=1$
^# frame: s12=1 s21=1 s30=1 pa4=0x[0-9a-f]+ pa2=0x[0-9a-f]+ pa1g=0x80000000 aligned=1 real4=1 real2=1 window=1 bad13=InvalidArgument atomic=1$
^# wx: x_then_w=WxViolation device_exec=WxViolation atomic=1 maps=1 w=0 x=1$
^# map: va=0x40000000 level=0 pa=0x[0-9a-f]+ satp=1 asid=[1-9][0-9]* sum=1 write=1 read=1 protect=1 unmapped=1 flush=[1-9][0-9]* walk_after=0 kernel=1$
^# leak: cap=[0-9]+ used=0 free=[0-9]+ holes=0 frames=0 pts=0 vspaces=0 maps=0 orphans=0 check=1$
^# mmu: satp=0x[0-9a-f]+ mode=8 root=0x[0-9a-f]+ tables=[0-9]+ used=[0-9]+ arena=0x[0-9a-f]+-0x[0-9a-f]+ kernel_end=0x[0-9a-f]+$
^# mmu on: satp=0x[0-9a-f]+ mode=8 readback=0x[0-9a-f]+ root=0x[0-9a-f]+ arena=0x[0-9a-f]+ alias=1 uart=1 kernel_low=1 kernel_high=1$
^# mmu layout: guard=0x[0-9a-f]+-0x[0-9a-f]+ none_low=1 none_high=1 none_physmap=1 stack=0x[0-9a-f]+-0x[0-9a-f]+ rw_nx=1 trap=0x[0-9a-f]+-0x[0-9a-f]+ rw_nx=1 arena=0x[0-9a-f]+-0x[0-9a-f]+ arena_alias=1 plic_low_2m=1 plic_high_2m=1 physmap_2m_nx=1 uart_dev=1 plic_dev=1 kernel_high_2m=1 all_harts=1$
^# mmu layout: .*physmap_2m_nx=1 uart_dev=1 plic_dev=1 kernel_high_2m=1 all_harts=1$
^# walk khop: pre=post=1 software=8/8 hardware_a=1 hardware_b=1 alias=1 satp_root=0x[0-9a-f]+$
^# plic: base=0x[0-9a-f]+ ndev=[0-9]+ hart=[0-9]+ ctx=[0-9]+ plat_off=1 plat_on=1 threshold=0 priority_uart=1 enable_ctx=1 enable_mmode=0 claim_idle=0$
^# plic map: low=0x[0-9a-f]+-0x[0-9a-f]+ high=0x[0-9a-f]+-0x[0-9a-f]+ low_dev=1 high_dev=1 low_2m=1 high_2m=1 low_nx=1 high_nx=1$
^# uart irq: irqs=[1-9][0-9]* claims=[1-9][0-9]* completes=[1-9][0-9]* traps_delta=[1-9][0-9]* balanced=1 spurious=0 unexpected=0 last_irq=10 last_iir=0x2 ier_off=1 plic_enable=0$
^# plic quiet: claim_idle=0 quiet=1 stats_unchanged=1 external_traps=[1-9][0-9]* interrupts_delta=[1-9][0-9]* spurious=0 unexpected=0$
^# timer arm: now=[0-9]+ first_deadline=[0-9]+ tick_ticks=100000 tick_hz=100 timebase_hz=10000000 sie_stie=1 sstatus_sie=1$
^# timer ticks: ticks=[0-9]+ min_delta_ticks=[0-9]+ max_delta_ticks=[0-9]+ non_monotonic=0 late=0 missed=0 max_late_ticks=[0-9]+ first=[0-9]+ last=[0-9]+ deadline=[0-9]+$
^# timer drift: min_ns=[0-9]+ max_ns=[0-9]+ min_us=[0-9]+ max_us=[0-9]+ tick_ns=10000000 min_bound_ns=9000000 max_bound_ns=11000000 in_range=1$
^# timer irq: interrupts=[0-9]+ scause=0x8000000000000005 code=5 exceptions=[0-9]+ exceptions_before=[0-9]+ sie_stie=1$
^# timer sie: trap_spie_on=[0-9]+ handler_sie_off=[0-9]+ ticks=[0-9]+ sie_stie=1 sret_sie=1$
^# timer disable: sie_stie=0 sstatus_sie=0$
^# smp: boot=[0-9]+ harts=__SMP__ .* timeout=0 .* dtb_magic=1 .* stride=0x1d000 area=0xe8000$
^# harts alive: n=__SMP__ mask=0x__MASK__ alive=0x__MASK__ tickets=1 arrived=0x__MASK__ timeout=0$
^# per-hart stack/guard: stack_ok=0x__MASK__ guard_ok=0x__MASK__ distinct=1 owner_ok=true$
^# per-hart timer: ok=0x__MASK__ total_ticks=[1-9][0-9]* arrived=0x__MASK__ timeout=0$
^# per-hart plic: ok=0x__MASK__ distinct=1 uart_irqs_boot=8 stats_clean=1 arrived=0x__MASK__ timeout=0$
^# barrier: round=5 arrived=0x[0-9a-f]+ missing=0x0 timeout=0 wait_ticks=[0-9]+ slow_hart=1 slow_delayed=[01]$
^# barrier timeout: round=6 expected_missing=0x[0-9a-f]+ arrived=0x[0-9a-f]+ missing=0x[0-9a-f]+ timeout=1 wait_ticks=[0-9]+ \(may khong treo\)$
^# ipi: harts=__SMP__ boot=[0-9]+ peer=-?[0-9]+ mode=__IPI_MODE__ target=100 sie_ok=0x__MASK__ setup_ok=1$
^# ipi delivered: mode=__IPI_MODE__ target=100 __IPI_DELIVERED__$
^# ipi drain: __IPI_DRAIN__ timeout_wait=[0-9]+ timeout_drains=[1-9][0-9]*$
^# ipi self: ok=0x__MASK__ local=[1-9][0-9]* sent_delta=0 traps_delta=0 kinds_tlb=[1-9][0-9]*$
^# ipi coalesce: __IPI_COALESCE__$
^# ipi distinct: ipi_traps=100 interrupts_delta=100 timer_delta=0 plic_delta=0 last_scause=0x8000000000000001 last_code=1 decode_ok=1 ok=1$
^# tlb init: asid=0 generation=0 rollover=0 satp=0x[0-9a-f]+ satp_asid=0 active_on=0x(1|2|4|8) free=255 issued=1$
^# tlb: harts=__SMP__ boot=[0-9]+ peer=-?[0-9]+ mode=__TLB_MODE__ rounds=100 asid_bits=8 asid_count=256$
^# tlb probe: va=0x80800000 pa_a=0x80800000 pa_b=0x80a00000 probe2_va=0x80a00000 pa_c=0x80c00000 pa_d=0x80e00000 leaf_a=1 leaf_b=1 nx=1$
^# shootdown rounds: mode=__TLB_MODE__ rounds=100 ok=100 acked_ok=100 sent=__TLB_SENT__ sent_expected=__TLB_SENT__ self_slot_wrong=0 local_verify_wrong=0 barrier_bad=0 targets=0x[0-9a-f]+ active_on=0x[0-9a-f]+ wait_ticks=[0-9]+$
^# shootdown all: bad=0 served=__SMPM1__ expected_served=__SMPM1__ online=0x__MASK__$
^# shootdown peer: mode=__TLB_MODE__ __TLB_PEER__$
^# stale: asid=0 va=0x80800000 forms=3 stale_seen=3 fresh_ok=3 wrong=0 note=stale-read-without-sfence-is-positive$
^# stale form: name=Page fresh_a=1 stale_seen=1 fresh_b=1 wrong=0$
^# stale form: name=Asid fresh_a=1 stale_seen=1 fresh_b=1 wrong=0$
^# stale form: name=All fresh_a=1 stale_seen=1 fresh_b=1 wrong=0$
^# outside: mode=__TLB_MODE__ removed=0x[0-9a-f]+ removed_count=__TLB_REMOVED__ narrowed=0x[0-9a-f]+ targets=0x[0-9a-f]+ acked=0x[0-9a-f]+ caller_in=[01] removed_acked_before=0x[0-9a-f]+ removed_acked_after=0x[0-9a-f]+ restored=1 restored_acked=0x[0-9a-f]+ outside_wrong=0 ok=1$
^# rollover: generation=1 exhaustions=1 rollover=1 flush=Ok asid=0 satp_asid=0 stale_seen=1 fresh_ok=1 free=255 issued=1 asid_ok=0x__MASK__ online=0x__MASK__ primed=0x4b51c000000000c2 wrong=0$
^# timeout: outcome=Timeout narrowed=0x[0-9a-f]+ targets=0x[0-9a-f]+ acked=0x[0-9a-f]+ missing=0x[0-9a-f]+ waited=[0-9]+ probe=1000000 timeouts=[1-9][0-9]* mailbox_empty=1 recovered=1 wrong=0$
^# tlb asid: ok=0x__MASK__ online=0x__MASK__ phase=0x[0-9a-f]+ arrived=0x__MASK__ timeout=0 satp_asid_boot=0 lease_asid=0 generation=1$
^# tlb: core asid=[0-9]+ requests=[1-9][0-9]* seq=[1-9][0-9]* local=[1-9][0-9]* targets=0 acked=0 pending=0 stale_load=13 stval_ok=1 protect_read=1 restore=1 root_fault=13 harts=1 check=1$
^# slot: arena=0x[0-9a-f]+\+[0-9]+ span=0x[0-9a-f]+ bytes=768 within=1 used0=[0-9]+ used1=[0-9]+ used2=0 grown=1 slot_ok=1 revoke_back=1 check=1$
^# bind: sc=[0-9]+ ntfn=[0-9]+ two_way=1 second=IllegalOperation rebind_clears=1 wrong_target=1 signal_wake=1 other_blocked=1 unbind=1 delete_clears=1 check=1$
^# resume: fault=1 state=Ready regs=0xaa/0xbb running=IllegalOperation regs_kept=1 reply=1 reply_caller=0 cap=InvalidArgument inactive=IllegalOperation exit_kept=1 user_nr_max=15 user_path=0 check=1$
^# conf image: build=[0-9a-f]{16} text=[0-9]+$
^# conf: scn=22 name=vm-map steps=[0-9]+ ok=1$
^# conf: scn=23 name=vm-wx steps=[0-9]+ ok=1$
^# conf: scn=24 name=vm-tlb .*$
^# conf: scn=25 name=vm-fault steps=[0-9]+ ok=1$
^# conf: scn=26 name=vm-shared steps=[0-9]+ ok=1$
^# trace: 22 1 map va=0x[0-9a-f]+ frame=f4 rights=READ\|WRITE attrs=WB => ok$
^# trace: 22 [0-9]+ walk va=0x[0-9a-f]+ => NotMapped$
^# trace: 23 [0-9]+ map .* => WxViolation$
^# trace: 23 [0-9]+ check => ok$
__EVID24__
^# trace: 25 [0-9]+ fault va=0x[0-9a-f]+ access=Write reason=NotMapped => ok$
^# trace: 26 [0-9]+ fault va=0x[0-9a-f]+ access=Write reason=Permission => ok$
^# trace: 26 [0-9]+ check => ok$
^# faultkind: case=load access=Read scause=0xd stval=0x50000000 sepc=0x[0-9a-f]+ spp=1 kind=Vm\{vspace=0, va=0x50000000, access=Read, reason=NotMapped\} src=trap walk=0 ok=1$
^# faultkind: case=store access=Write scause=0xf stval=0x50000000 sepc=0x[0-9a-f]+ spp=1 kind=Vm\{vspace=0, va=0x50000000, access=Write, reason=NotMapped\} src=trap walk=0 ok=1$
^# faultkind: case=insn access=Execute scause=0xc stval=0x50000000 sepc=0x[0-9a-f]+ spp=1 kind=Vm\{vspace=0, va=0x50000000, access=Execute, reason=NotMapped\} src=trap walk=0 ok=1$
^# faultkind: case=wx-abi map=W\|X err=WxViolation abi=4 atomic=1 pte=0 counts=0/0/0 panic=0 ok=1$
^# faultkind: case=device-exec device_count=1 map_rx=WxViolation protect_dev_rx=WxViolation abi=4 atomic=1 check=1 ok=1$
^# faultprobe: va=0x200040 access=Read scause=0xd stval=0x200040 sepc=0x[0-9a-f]+ pte=0x0 reason=NotMapped mapped=0 scratch=0 src=trap$
^# faultprobe: va=0x200040 access=Write scause=0xf stval=0x200040 sepc=0x[0-9a-f]+ pte=0x0 reason=NotMapped mapped=0 scratch=0 src=trap$
^# faultprobe: va=0x1000 access=Write scause=0xf stval=0x1000 sepc=0x[0-9a-f]+ pte=0x[0-9a-f]+ reason=Permission mapped=1 scratch=1 src=trap$
^# core: link addr=0x[0-9a-f]+ bytes=189384 pin=196608 image_cap=262144 parts=29 sum=189384 scalars=[0-9]+ padding=[0-9]+ sum_ok=1 in_image=1$
^# core: untyped id=[0-9]+ base=0x[0-9a-f]+ bytes=65536 used=0 free=65536 allocated=0 gap=0 objects=1 caps=0 check_bytes=1 check_all_bytes=1$
^# core: tcb retype tcb=[0-9]+ slot=1 objects=11 caps=11 used=[0-9]+ state=Inactive check_caps=1 check_bytes=1$
^# core: configure cspace=1 sc=1 priority=3 mcp=10 budget=4 period=8 suspended_ok=1 resume=1 state=Ready check_sched=1 check_sc_refs=1 check_budget=1$
^# core: sched harts=__SMP__ pick0=[0-9]+ pick1=[0-9]+ dispatch0=[0-9]+ dispatch1=[0-9]+ t0=Ready t1=Running suspended=0 check_sched=1$
^# core: ipc call=1 recv=1 reply=1 mr=0xfeed blocked_send=1 blocked_reply=1 t0=Ready ep_len=0 reply_caller=0 sent=[0-9]+ delivered=[0-9]+ check_ipc=1 check_reply_links=1$
^# core: ntfn signal=1 wake=1 word=0x0 badge=0x0 waiter=0 consumed=0x42 poll=0x8 state=Ready check_notifications=1$
^# core: waitaddr frame=0x[0-9a-f]+ label=[0-9]+ offset=64 value=0x1234 woke=1 waiters=0 buckets=0 state=Ready check_waitaddr=1$
^# core: d2-83 frame=NotSupported page_table=NotSupported vspace=NotSupported atomic=1 untyped_used=[0-9]+ obj_frame=[0-9]+ obj_vspace=[0-9]+ obj_health=1 owner=obj$
^# core: invariants caps=1 objects=[0-9]+ used=0 free=65536 deleted=[0-9]+ check_caps=1 check_bytes=1 check_all_bytes=1 check_sched=1 check_sc_refs=1 check_budget=1 check_mailboxes=1 check_ipc=1 check_reply_links=1 check_notifications=1 check_waitaddr=1 check_fault=1 leak=0$
^# ipc: cap-transfer installed=8 failed=1 unwrap=1 caps4=4 len=2 rights=0xb caps3=3 caps1=1 extra=1 badge=0xa5a5 check_ipc=1 check_caps=1$
^# ipc: cap-message direction=call-reply caller_caps=2 reply_caps=2 caller_bound=1 caller_ready=1 rights=0xb reply_caller=0 check_ipc=1 check_reply_links=1$
^# ipc: revoke deleted=[0-9]+ caller=Ready caller_err=Cancelled recv_waiter=Ready notif_waiter=Ready sender=Ready sender_err=Cancelled queue=0 queued=1 pending=0 sent=[0-9]+ dropped=1 check_ipc=1 check_reply_links=1 check_notifications=1$
^# core: phase fails=0 static=189384 pin=196608 image_cap=262144$
^# mem: init untyped=2 u0=[0-9]+ u1=[0-9]+ base0=0x82000000 base1=0x80000000 window=1073741824 cap0=1073741824 cap1=1073741824 physmap=1$
^# mem: alloc cap0=[0-9]+ window=1073741824 base=0x82000000 used=2101248 free=[0-9]+ hole=0 live4=1 live2=1 live1g=0 pa4=0x82200000 pa2=0x82000000 s21=1 s12=1 real4=1 real2=1 live=2101248 stats=2101248 frames=2 allocs=2 check=1 obj=1$
^# mem: revoke used0=2101248 used1=0 holes=0 frames=0 free=1073741824 realloc4=0x82000000 same=1 stats=0 check=1 obj=1$
^# mem: oom window=1073741824 pa1g=0x80000000 second=NotEnoughMemory code=9 small=9 used=1073741824 used_after=1073741824 free=0 atomic=1 panic=0 check=1 obj=1$
^# mem: bridge token=[0-9]+ core_obj=[0-9]+ obj_cap=[0-9]+ key=0x[0-9a-f]+ pa=0x82000000 bits=12 slot=[0-9]+ rights_ok=1 obj_ok=1 resolve=1 used_granted=4096 check_caps=1 check_ipc=1 check_objmem=1$
^# mem: release released=1 used_after=0 free=1073741824 cap_after=0 caps_check=1 obj=1$
^# mem: deny wrong=2 rights=3 invalid=1 used=0 allocs=[0-9]+ atomic=1 obj=1$
^# fs: rename src=[0-9]+ exp=[0-9]+ replaced_gone=1 dir_over=1 used=[0-9]+->[0-9]+ impl=rename-overwrite$
^# fs: truncate used=[0-9]+->[0-9]+ freed=[0-9]+ read_empty=(true|false) reuse=[0-9]+ \(=used_again-used_t0\) used_again=[0-9]+ used_t0=[0-9]+ reuse_ok=(true|false) const:from_blocks=4$
^# fs: double index=143 used=[0-9]+->[0-9]+->[0-9]+ delta=3 \(=3: root\+bang L2\+data\) root=[1-9][0-9]* l2=[1-9][0-9]* data_ptr=[1-9][0-9]* single=0 read=4 data=2CAP fsck=(true|false)$
^# fs: rename_ok=[01] trunc_ok=(true|false) double_ok=(true|false) fsck=(true|false) const:disk_blocks=48 const:direct=10$
^# fs: writeback dirty_marks=[0-9]+->[0-9]+ dirty_now=[0-9]+ writes=8 flushes=1 wrote=(true|false) backend_ok=(true|false) reopen_ok=(true|false) const:page=4096 const:sector=512$
^# fs: journal writes=[0-9]+ issued=[0-9]+ commit_prefix=[0-9]+ \(=writes-3: 2 target \+ 1 clear\) replay=1 new=1 size=7 keep=(true|false) fsck=(true|false) crc_reject=2 crc_applied=0 crc_disk=old reorder_writes=[0-9]+ kept=(true|false) pre=old replay_r=1 new_r=1 post=new keep_r=(true|false)$
^# fs: crash pha=[0-6] writes=[0-9]+ replay=[0-9]+ content=(old|new) size=[0-9]+ found=0 keep=(true|false) clean=(true|false)$
^# fs: crash7 phases=7 writes=[0-9]+ old=[0-9]+ new=[0-9]+ torn=0 consistent=7 threshold=[0-9]+ last_old=[0-9]+ atomic=1 replays=[0-9]+$
^# fs: fsck detect=1 found=[0-9]+ fixed=[0-9]+ unfixed=0 orphan_blocks=1 orphan_inodes=1 bad_nlink=1 bad_blocks=1 used=[0-9]+->[0-9]+->[0-9]+ keep=(true|false) fsck_ok=(true|false) second=0 unfixed_pos=(true|false) unfixed2=[0-9]+ double_blocks2=[0-9]+ second_bad=(true|false) const:disk_blocks=48$
^OpenSBI v[0-9]+\.[0-9]+\.[0-9]+$
^Platform HART Count *: *__SMP__$
EOF
            ;;
        ktest-guard) cat <<'EOF'
^# guard image: hart=[0-9]+ stvec=0x[0-9a-f]+ direct=1 satp=0x[0-9a-f]+ mode=8 root=0x[0-9a-f]+ tables=[0-9]+ used=[0-9]+ arena=0x[0-9a-f]+-0x[0-9a-f]+$
^# mmu map: high=0x[0-9a-f]+ physmap=0x[0-9a-f]+ guard=0x[0-9a-f]+-0x[0-9a-f]+ guard_none=1 alias=1$
^# guard load stats: faults\+[0-9]+ load\+[0-9]+ store\+[0-9]+ recovered\+[0-9]+ fatal\+[0-9]+ last_scause=0xd last_stval=0x[0-9a-f]+ armed=[0-9]+$
^OpenSBI v[0-9]+\.[0-9]+\.[0-9]+$
^Platform HART Count *: *__SMP__$
EOF
            ;;
        ktest-user) cat <<'EOF'
^# user core: static=189384 pin=196608 image_cap=262144 untyped=65536 harts=__SMP__ tcb=[0-9]+ ep=[0-9]+ cspace=[0-9]+ state=Running current=1 check=1$
^# user image: hart=[0-9]+ stvec=0x[0-9a-f]+ direct=1 satp=0x[0-9a-f]+ mode=8 root=0x[0-9a-f]+ obj_arena=0x[0-9a-f]+-0x[0-9a-f]+ untyped=2$
^# init load: elf=1 entry=0x40000000 segs=3 text=1 ro=1 data=1 stack=8 image_end=0x[0-9a-f]+ guard=0x40007000 guard_none=1 wx=0 flags=1 high=1 root=0x[0-9a-f]+ asid=[1-9][0-9]* check=1$
^# init run: entry=0x40000000 u_mode=1 sret_spp=0 ecall_spp=0 entries=1 syscalls=10 exits=1 exit=42$
^# u sstatus: enter=0x[0-9a-f]+ trap=0x[0-9a-f]+ spp=0 spie=0 sie=0 sum=0 uxl=2$
^# user put_str: calls=2 bytes=51 sum_off=1$
^# user exit: code=42 state=Exited exits=1$
^# user badsys: nr=31 err=19 calls=5 panic=0$
^# user getinfo: nr=15 calls=1 err=0 abi_version=1 num_harts=__SMP__ exit=42$
^# user yield: nr=8 calls=1 err=0 budget_left=[0-9]+$
^# sys: nr=15 err=0 value=1 tcb=[0-9]+ state=Running src=core$
^# sys: nr=8 err=0 value=[0-9]+ tcb=[0-9]+ state=Running src=core$
^# sys: nr=31 err=19 value=0 tcb=[0-9]+ state=Running src=core$
^# sys: tap=7 nr=0 out=blocked err=0 tcb=[0-9]+ state=BlockedOnSend src=core$
^# sys: tap=7 nr=13 out=halt err=0 tcb=[0-9]+ state=Running src=core$
^# sys: tap=7 nr=15 out=ok err=0 tcb=[0-9]+ state=Running src=core harts=__SMP__$
^# sys: tap=8 empty=1 wrong_type=2 call_nonep=18 recv_wrong=2 tcb=[0-9]+ state=Running$
^# sys: tap=9 call=1 recv=1 reply=1 mr=0x11/0x22 blocked=1 reply_caller=0 caller=(Ready|Running) state_ok=1$
^# sys: tap=10 nb_send=0 nb_recv_mr=0x55 busy=14 nbsr_mr=0x66 nbwait=0 notif=0x99/0xab/0xcd ok=1$
^# sys: tap=11 wait=1 wake=1 mismatch=5 notmapped=11 state=Ready ok=1$
^# sys: tap=12 nr=11 clock1=[0-9]+ clock2=[0-9]+ mono=1$
^# sys: tap=13 nr=8 tcb_before=[0-9]+ tcb_after=[0-9]+ state_before=Running state_after=Ready ok=1$
^# sys: tap=14 nr=14 err=0 code=42 tcb=[0-9]+ state=BlockedOnSend call=0 fault=Exit\{42\} ep_len=1 sender=[0-9]+ reply=0$
^# sys: kernel phase invariants=1 caps=[0-9]+ objects=[0-9]+$
^# user exit: code=42 state=Exited exits=1$
^# user exitkind: code=42 kind=Exit\{42\} ok=1$
^# stack: noload=1 block=[0-9]+ harts=__SMP__ distinct=1 canary=0 base=0x[0-9a-f]+$
^# inv: prep untyped_slot=13 vspace_slot=14 frame_slot=15 frame_b_slot=18 vspace_ro_slot=19 tcb=[0-9]+ va=0x41000000 registered=1 vspace_key=0x[0-9a-f]+ frame_b_pa=0x[0-9a-f]+ root=0x[0-9a-f]+$
^# inv: retype method=0 ty=Frame size=12 err=0 slot=15 stats=1/[0-9]+$
^# inv: map method=0 va=0x41000000 pa=0x[0-9a-f]+ err=0 stats=1 phys=0x1122334455667788$
^# inv: unmap method=1 va=0x41000000 err=0 stats=1 walk_after=0 objmem=1 caps=1$
^# inv: deny err=18 stats=3 allowed=3 tls_delta=0 last_err=18 syscalls=[0-9]+ entries=2$
^# inv: deny2 ran=1 frame_a_maps=0 frame_b_maps=0 walk_after=0$
^# inv: fault va=0x41000000 kind=Vm\{NotMapped,Read\} ok=1 snap_state=Faulted$
^# capx: pre cspace_b4=empty cspace_b5=empty walk=0 a5=Frame src_alive=1 policy=copy$
^# capx: recv slot=4 ty=Frame rights=0xb obj=[0-9]+ from_obj=[0-9]+ from_rights=0xf installed=1 failed=1$
^# capx: map va=0x41000000 pa=0x[0-9a-f]+ bits=12 walk=1 walk_pa=0x[0-9a-f]+ phys=0x8877665544332211 caps=1 ipc=1 obj=1 src_alive=1$
^# capa: u-mode server_exit=0 client_exit=0 steps=[0-9]+ src_obj=[0-9]+ src_ty=Frame src_rights=0xf src_alive=1$
^# capa: cap slot client4\.\.7=1111 server8=1 copies=[0-9]+ revokes=[0-9]+ deletes=[0-9]+ mints=[0-9]+ moves=[0-9]+ denied=[0-9]+$
^# capa: walk va=0x41000000 valid=1 pa=0x[0-9a-f]+ expected_pa=0x[0-9a-f]+ phys=0x8877665544332211 client_vspace=[0-9]+$
^# capa: pre empty_client=1 src_alive=1 walk_before=0 post caps=1 ipc=1 bytes=1 obj=1$
^# capa: mint badge=0xabc rights=0x3 badge_frame=1 copy_rights=0x3 move_src_empty=1 same_cap=1 parent_ep=1 cleanup=1 caps=1$
^# capa: revoke slot=6 deleted=2 subtree=1 sibling=1 live_before=[0-9]+ live_after=[0-9]+ delete_reparent=1 cross_cnode=1 src_alive=1 caps=1 revokes=[0-9]+$
^# capa: am empty=1 occupied=1 rights=1 range=1 ro_cnode=1 mutate=1 rotate=1 other=1 denied=1 short=1 long=1 arity=10 stale=1 dead_alive=1 caps=1$
^# capa: rfree s=12->12 c=12->12 live=[0-9]+->[0-9]+ cap_high=[0-9]+->[0-9]+ installed\+=1 failed\+=0$
^# capa: invariants caps=1 ipc=1 bytes=1 obj=1 frames=[0-9]+$
^# idl: const:iface=ringchan const:version=1 const:methods=3 attach=0 sync=1 finish=2 seen=0x7 attach_msgs=1 sync_msgs=[1-9][0-9]* finish_msgs=1 sync_acks=[1-9][0-9]* attach_caps=1 labels_ok=1 const:caps_max=1$
^# ring: frame=0x[0-9a-f]+ const:va=0x42000000 size=8 slot=32 head=40 tail=40 seq=40 used=0 check=1 open=1 const:capacity=8 const:slot_bytes=32$
^# ring: sent=40 recv=40 full_err=[1-9][0-9]* full_code=9 full_name=NotEnoughMemory full_head=8 empty_err=3 empty_code_first=14 empty_code_last=14 empty_name=Busy empty_tail=0 first=0 last=39 wrap=head/size=5 const:size=8 head=40 order=1 hdrerr=0 dups=0 lost=0 bad=0 peak=8 const:total=40$
^# ring: prod_sum=820 cons_sum=820 sum_equal=1 slot0=[0-9]+/[0-9]+/[0-9]+/[0-9]+ const:sum=820$
^# ring: map server=1 client=1 same_pa=1 pa=0x[0-9a-f]+ s_pa=0x[0-9a-f]+ c_pa=0x[0-9a-f]+ s_flags=1 c_flags=1 pre_walk=0/0$
^# ring: caps b4=1 b4_ty=Frame b4_alive=1 frame_obj=[0-9]+ installed=[0-9]+->[0-9]+ failed=[0-9]+->[0-9]+ caps=1 ipc=1 obj=1 bytes=1 steps=s[0-9]+/c[0-9]+ exits=0/0 run_steps=[0-9]+$
^# pager: fault kind=Vm va=0x[0-9a-f]+ client=0 access=Read reason=NotMapped core_fault=1 faults=[1-9][0-9]* ready=1 const:page=4096 const:sector=512$
^# pager: cache const:capacity=3 hits=2 misses=7 evictions=4 reads=7 sectors=56 served=9 rejected=1 entries=3 unique=3 \(=entries\(3\) nếu keys_unique\) shared=2 reloads=2 const:clients=4 magic=0x[0-9a-f]+ const:magic_expected=0x[0-9a-f]+$
^# pager: map p2_a=0x[0-9a-f]+ p2_b=0x[0-9a-f]+ p2_same=1 \(=p2_a==p2_b\) hit_delta0=1 \(reads_before=[0-9]+ reads_after=[0-9]+\) mapping_bad=0 walks_ok=[1-9][0-9]*$
^# pager: evict evictions=4 \(=misses\(7\)-min\(capacity\(3\),unique\(3\)\)\) walk_gone=1 \(file=0 page=1 va=0x[0-9a-f]+\) pa_changed=1 \(file=0 page=0 first=0x[0-9a-f]+ reload=0x[0-9a-f]+\) reloads=2 kernel_data_ok=[1-9][0-9]* kernel_data_bad=0 a_done=5 b_done=4$
^# pager: data a_ok=5 a_bytes=20480 \(=a_ok\*4096\) a_bad=0 a_bad_bytes=0 b_ok=4 b_bytes=16384 \(=b_ok\*4096\) b_bad=0 b_bad_bytes=0$
^# pager: reject client=1 page=99 status=5 name=InvalidArgument reads_before=[0-9]+ reads_after=[0-9]+ delta=0 \(=reads_after-reads_before\) exit=0x80000005$
^# pager: exit pager=0 a=0 b=2147483653 steps=[0-9]+ const:starts=3 exits=3 \(=\(pager==0\)\+\(a==0\)\+\(b==0x8000_0000\|status\)\)$
^# pager: neg aux_status=2 name=WrongObjectType over_status=10 name=AlreadyMapped pa_same=1 \(walk=0x[0-9a-f]+ exp=0x[0-9a-f]+\) magic=0x[0-9a-f]+ const:neg_over=10 \(aux=P_SELF là CNode cap, không phải VSpace ⇒ core trả WrongObjectType trước object layer\)$
^# pager: clients=4 deliveries=[1-9][0-9]* faults=[1-9][0-9]* served=[0-9]+ rejected=[0-9]+ expected_va=[1-9][0-9]* distinct_va=[1-9][0-9]* ok=1$
^# ktest-user: taps=37 ok=1$
^OpenSBI v[0-9]+\.[0-9]+\.[0-9]+$
^Platform HART Count *: *__SMP__$
EOF
            ;;
        ktest-vblk) cat <<'EOF'
^# vblk image: hart=[0-9]+ stvec=0x[0-9a-f]+ direct=1 satp=0x[0-9a-f]+ mode=8 tables=[0-9]+ used=[0-9]+ const:disk_bytes=4194304$
^# vblk pci: functions=[0-9]+ bus=[0-9]+ dev=[0-9]+ func=[0-9]+ vendor=0x1af4 device=0x100[12] bar=[0-9]+ kind=mem(32|64) size=[0-9]+ assigned=0x[0-9a-f]+ readback=0x[0-9a-f]+ command=0x[0-9a-f]+$
^# vblk cap: common=0x[0-9a-f]+ notify=0x[0-9a-f]+ device=0x[0-9a-f]+ isr=0x[0-9a-f]+ mul=[0-9]+ cap_bar=[0-9]+ ecam=0x[0-9a-f]+ ecam_identity=1 bar_identity=1 cap_ranges_ok=1 overlap_checked=1 other_bars=[0-9]+ reserved_ranges=[0-9]+$
^# vblk dev: capacity=[1-9][0-9]* blk_size=512 size_max=[0-9]+ seg_max=[0-9]+ num_queues=[1-9][0-9]* queue_max=[0-9]+ queue_size=[3-9][0-9]* device_features=0x[0-9a-f]+ driver_features=0x100000200 flush_feature=1 status=0xf notify_off=[0-9]+ notify_addr=0x[0-9a-f]+ config_generation=[0-9]+$
^# vblk dma: desc=0x[0-9a-f]+ avail=0x[0-9a-f]+ used=0x[0-9a-f]+ req=0x[0-9a-f]+ data=0x[0-9a-f]+ status_pa=0x[0-9a-f]+ identity=1 version1=1 status_readback=0xf$
^# vblk io: const:sectors=8 const:passes=2 writes=16 reads=16 compared=8192 \(=reads\(16\)\*512\) mismatched=0 first_diff=[0-9]+ requests=[0-9]+ completions=[0-9]+ const:first_lba=6000$
^# vblk flush: calls=2 flushes=[0-9]+ completions=2 device_status=0xf driver_ok=1 errors=[0-9]+ timeouts=0$
^# vblk fs: phase=write formatted=1 wrote=69 fsck_clean=1 found=0 fixed=0 unfixed=0 const:payload_len=69$
^# vblk fs: phase=remount read_len=69 content_match=1 fsck_clean=1 const:payload_len=69$
^# vblk magic: lba0_read=1 lba0_zero=1 lba1_read=1 lba1_magic=VF04 lba1_version=4 lba1_total_blocks=8192$
^# vblk loi: capacity=8192 refused=OutOfRange refused_no_request=1 \(=requests_after\([0-9]+\)-requests_before\([0-9]+\)\) forced=Io forced_requests=1 \(=requests_after\([0-9]+\)-requests_before\([0-9]+\)\) recovered=1 device_status=0xf id_mismatch=0 timeouts=0 const:id_timeout_proven=0 errors_before=[0-9]+ errors_after=[0-9]+ errors_delta=1 \(=after-before\) fs_err_refused=Invalid$
^# vblk stats: requests=[0-9]+ reads=[0-9]+ writes=[0-9]+ flushes=[0-9]+ completions=[0-9]+ errors=[0-9]+ notifies=[0-9]+ id_mismatch=0 timeouts=0 capacity=8192$
^OpenSBI v[0-9]+\.[0-9]+\.[0-9]+$
^Platform HART Count *: *__SMP__$
EOF
            ;;
        ktest-exec) cat <<'EOF'
^# exec rig: stvec=0x[0-9a-f]+ untyped=[0-9]+ core_untyped=[0-9]+ images=[0-9]+ cspace=[0-9]+ tcb=[0-9]+ tcb_cptr=[0-9]+ tls_const=0x74310001$
^# tls: set=0x74310001 observed_first=0 observed=0x74310001 method=15 const:method_source=spec traps=[0-9]+ stats=[0-9]+ sys0_calls=[0-9]+ sys0_err=0 exit=1 exit2=1 tcb=0x[0-9a-f]+ tcb_tls=0x74310001 ok=1$
^# tls neg: method_bad=18 arity0=5 arity2=5 cap_untyped=18 cap_ro=3 tls_giu=0x74310001 set_tls_unchanged=1 ok=1$
^# exec: step=1 pid=0 tcb=[0-9]+ cspace_same=1 old_vspace=[0-9]+ new_vspace=[0-9]+ vspace_changed=1 old_entry=0x[0-9a-f]+ new_entry=0x[0-9a-f]+ const:elf_linked_at_base=1 img_differ=1 img_a=[0-9]+ img_b=[0-9]+ freed=[0-9]+ new_pages=[0-9]+ exit=2 objects=[0-9]+ frames=[0-9]+ caps=[0-9]+ .* same=1 check=1 ok=1$
^# exec chain: execs=3 pid=0 tcb=[0-9]+ exits=3,4,1 recycled=[1-9][0-9]* freed=[1-9][0-9]* objects=[0-9]+ frames=[0-9]+ caps=[0-9]+ .* frames_same=1 objects_same=1 caps_same=1 total=[0-9]+ ok=1$
^# exec atomic: path_err=NotFound elf_err=Elf vspace_same=1 entry_same=1 objects=[0-9]+ frames=[0-9]+ caps=[0-9]+ .* frames_same=1 objects_same=1 caps_same=1 execs_total=[0-9]+ ok=1$
^# tls reset: after_exec=0x0 observed=0 exit1=4 set=0xb2 observed2=0xb2 exit2=4 tcb_tls=0xb2 execs_total=[0-9]+ ok=1$
^# exec final: c0 objects=[0-9]+ frames=[0-9]+ caps=[0-9]+ .* releases_clean=1 fail=0 execs_total=[0-9]+$
EOF
            ;;
        ktest-con) cat <<'EOF'
^# con image: hart=[0-9]+ stvec=0x[0-9a-f]+ direct=1 satp=0x[0-9a-f]+ mode=8 root=0x[0-9a-f]+ obj_arena=0x[0-9a-f]+-0x[0-9a-f]+ untyped=[0-9]+ tcb=[0-9]+ elf=[0-9]+$
^# con plic: base=0x[0-9a-f]+ ndev=[0-9]+ hart=[0-9]+ ctx=[0-9]+ threshold=0 priority_uart=1 enable_ctx=1 enable_mmode=0 claim_idle=0$
^# con ring: pa=0x[0-9a-f]+ phys=0x[0-9a-f]+ va=0x43000000 const:slots=32 const:slot_bytes=32 head=0 tail=0 check=1 const:capacity=768$
^# con uart: irqs=[1-9][0-9]* bytes=30 const:payload=30 chunks=[1-9][0-9]* const:chunks_want=2 head=[0-9]+ const:head0=0 tail=0 used=[0-9]+ seq=[0-9]+ empty_polls=[0-9]+ empty_bytes=0 drops=0 timeouts=0 inject_ok=1 inject_ticks=[0-9]+ rx_begin=1 rx_end=1$
^# con u: map=1 exit=0 match=1 head=[0-9]+ tail=[0-9]+ used=0 captured=[1-9][0-9]* prefix="[^"]*"$
^# con pci: vendor=0x1af4 device=0x1003 bus=0 dev=[0-9]+ func=0 bar=4 kind=mem64 size=0x4000 assigned=0x40200000 readback=0x40200000 command=0x6 ecam=0x[0-9a-f]+ other_bars=[0-9]+ reserved=5 overlap=1 caps_in_bar=1 ecam_id=1 bar_id=1 const:assign=0x40200000$
^# con queue: num_queues=64 rx_max=128 rx_size=8 rx_notify_off=[0-9]+ tx_max=128 tx_size=8 tx_notify_off=[0-9]+ notify_mul=4 device_features=0x[0-9a-f]+ driver_features=0x100000000 const:version1=1 multiport=0 status=0xf generations=[0-9]+/[0-9]+ .*$
^# con tx: one=Ok\(14\) two=Ok\(10\) writes=2 write_bytes=24 const:tx_want=24 notifies=[0-9]+ last_tx_used_len=[0-9]+ const:tx_len_note=0..=24 driver_status=0xf$
^# con rx: got=14 const:want=14 ok=1 busy=0 errors=0 timeouts=0 id_mismatch=0 last_rx_used_len=14 armed=1 head=[0-9]+/[0-9]+$
^# con neg: busy=1 const:busy_code=14 read_try=Busy ring_slots_ok=32 const:slots_want=32 ring_bytes_ok=768 const:bytes_want=768 drops=2 const:drops_want=2 full_code=9 const:full_want=9 consumed=768 order_ok=1 head=[0-9]+ tail=[0-9]+ used=0 timeouts=0$
^# con done: fails=0 const:plan=5 .*$
^OpenSBI v[0-9]+\.[0-9]+\.[0-9]+$
^Platform HART Count *: *__SMP__$
EOF
            ;;
        ktest-proc) cat <<'EOF'
^# user core: static=189384 pin=196608 image_cap=262144 untyped=65536 harts=__SMP__ tcb=[0-9]+ ep=[0-9]+ cspace=[0-9]+ state=Running current=1 check=1$
^# init load: elf=1 entry=0x40000000 segs=3 text=1 ro=1 data=1 stack=8 image_end=0x[0-9a-f]+ guard=0x40007000 guard_none=1 wx=0 flags=1 high=1 root=0x[0-9a-f]+ asid=[1-9][0-9]* check=1$
^# init run: entry=0x40000000 u_mode=1 sret_spp=0 ecall_spp=0 entries=1 syscalls=10 exits=1 exit=42$
^# user exit: code=42 state=Exited exits=1$
^# user exitkind: code=42 kind=Exit\{42\} ok=1$
^# proc: load procsrv=\[0x[0-9a-f]+,0x[0-9a-f]+\) asid=[0-9]+ segs=[0-9]+ stack=[0-9]+ ok=1 shell=\[0x[0-9a-f]+,0x[0-9a-f]+\) asid=[0-9]+ segs=[0-9]+ stack=[0-9]+ ok=1 child=\[0x[0-9a-f]+,0x[0-9a-f]+\) asid=[0-9]+ segs=[0-9]+ stack=[0-9]+ ok=1
^# proc: exit recv code=42 state=Inactive faults=[0-9]+ captured=1$
^# proc: recv exit pid=[0-9]+ code=42$
^# shell: ps replies=1 entries=4 snapshot_match=1 captured=1$
^# shell: wait pid=[0-9]+ code=42 captured=1$
^# shell: kill pid=[0-9]+ code=0 suspend_ok=1 victim_suspended=1 captured=1$
^# shell: exit code=0 state=Inactive faults=[0-9]+ steps=[0-9]+ captured=1$
^# proc: sched steps=[0-9]+ loads=3 events=[0-9]+ faults=2 ps=4 invariant=1$
^# proc: sched done ok=1$
^# proc: up proto=wait/kill$
^# shell: up cmds=help/echo/ps/wait/kill/exit$
^# shell: help cmds=6$
^# child: up then exit 42$
^U: init hello from u-mode$
^U: bad syscall err=19 ok$
^# u-mode read: hart=[0-9]+ addr=0x80200000 expect=scause=0xd stval=0x80200000 recover=1$
^# faultkind: case=u-kernel-read scause=0xd stval=0x80200000 recover=1 kind=Vm\{vspace=[0-9]+, va=0x80200000, access=Read, reason=Permission\} ok=1$
^# u-mode fault: scause=0xd stval=0x80200000 sepc=0x[0-9a-f]+ from_u=1 kernel_addr=1 recover=1 faultkind=Vm\{vspace=[0-9]+, va=0x80200000, access=Read, reason=Permission\}$
^# u-mode wx: hart=[0-9]+ exec=data_page expect=scause=0xc reason=Wx fatal=1$
^# u-mode fault: scause=0xc stval=0x[0-9a-f]+ sepc=0x[0-9a-f]+ from_u=1 kernel_addr=0 recover=0 faultkind=Vm\{vspace=[0-9]+, va=0x[0-9a-f]+, access=Execute, reason=Wx\}$
^# stack: noload=1 block=[0-9]+ harts=__SMP__ distinct=1 canary=0 base=0x[0-9a-f]+$
^OpenSBI v[0-9]+\.[0-9]+\.[0-9]+$
^Platform HART Count *: *__SMP__$
^# proc plic: base=0x[0-9a-f]+ ndev=[0-9]+ hart=[0-9]+ ctx=[0-9]+ threshold=0 priority_uart=1 enable_ctx=1 enable_mmode=0 claim_idle=0 const:uart_irq=10$
^# proc ring: pa=0x[0-9a-f]+ phys=0x[0-9a-f]+ va=0x43000000 const:slots=32 const:slot_bytes=32 head=0 tail=0 check=1 mapped_shell=1 const:capacity=768$
^# con uart: irqs=[1-9][0-9]* bytes=45 const:script_bytes=45 lines=6 inject_ok=1 drops=0 timeouts=0 head=45 tail=45 used=0 empty_polls=[0-9]+ const:va=0x43000000 ring_ok=1$
EOF
            ;;
        *) return 1 ;;
    esac |
        sed -e "s|__SMPM1__|$((smp - 1))|g" \
            -e "s|__SMP__|$smp|g" \
            -e "s|__MASK__|$mask|g" \
            -e "s|__IPI_MODE__|$ipi_mode|g" \
            -e "s|__IPI_DELIVERED__|$ipi_del|g" \
            -e "s|__IPI_DRAIN__|$ipi_drain|g" \
            -e "s|__IPI_COALESCE__|$ipi_coal|g" \
            -e "s|__TLB_MODE__|$tlb_mode|g" \
            -e "s|__TLB_SENT__|$tlb_sent|g" \
            -e "s|__TLB_PEER__|$tlb_peer|g" \
            -e "s|__TLB_REMOVED__|$tlb_removed|g" \
            -e "s|__BOOT_HART__|$hart|g" \
            -e "s|__EVID24__|$evid24|g"
}

# Mẫu PANIC **duy nhất** được phép của từng ảnh (đúng 1 dòng, không hơn).
panic_allow_re() {
    case "$1" in
        ktest) printf '%s' '^PANIC: nested trap \(KT-45: trap trong handler = fatal, khong resume\)$' ;;
        ktest-guard) printf '%s' '^PANIC: guard page fault scause=0xf stval=0x[0-9a-f]+ sepc=0x[0-9a-f]+ sstatus=0x[0-9a-f]+$' ;;
        ktest-user) printf '%s' '^PANIC: KT-70 u-mode pha syscall, invocation, capalloc, IDL va ring va pager xong \(37 TAP, 0 fail\)$' ;;
        ktest-proc) printf '%s' '^PANIC: u-mode fault scause=0xc stval=0x[0-9a-f]+ sepc=0x[0-9a-f]+ sstatus=0x[0-9a-f]+ from_u=1$' ;;
        ktest-vblk) printf '%s' '^PANIC: KT-72 vblk xong \(6 TAP, 0 fail\)$' ;;
        ktest-con) printf '%s' '^PANIC: KT-73 con xong \(5 TAP, 0 fail\)$' ;;
        ktest-exec) printf '%s' '^PANIC: KT-74 tls va exec xong \(6 TAP, 0 fail\)$' ;;
        *) return 1 ;;
    esac
}

# ------------------------------------------------------------ bộ kiểm một log

# Khung TAP: version/kế hoạch/thứ tự, dãy ok 1..N liên tục không trùng không thiếu, không `not ok`,
# đúng 1 PANIC đúng mẫu.
check_tap_common() {
    local log="$1" label="$2" plan="" okwant="" vline="" pline="" firstok="" vcount="" pcount="" \
        okcount="" notok="" ptotal="" pbad="" allow="" wantseq="" gotseq=""
    plan="$(expected_field "$label" 2)"
    okwant="$(expected_field "$label" 3)"
    vline="$(grep -n -x -F 'TAP version 13' "$log" | head -1 | cut -d: -f1)"
    pline="$(grep -n -x -F "1..$plan" "$log" | head -1 | cut -d: -f1)"
    firstok="$(grep -nE '^ok [0-9]+' "$log" | head -1 | cut -d: -f1)"
    vcount="$(grep -c -x -F 'TAP version 13' "$log" || true)"
    pcount="$(grep -c -x -F "1..$plan" "$log" || true)"
    okcount="$(grep -cE '^ok [0-9]+' "$log" || true)"
    notok="$(grep -cE '^not ok' "$log" || true)"
    [ "${vcount:-0}" -eq 1 ] || MISSING="$MISSING tap-version=${vcount:-0}"
    [ "${pcount:-0}" -eq 1 ] || MISSING="$MISSING plan-1..$plan=${pcount:-0}"
    if [ -n "${vline:-}" ] && [ -n "${pline:-}" ] && [ -n "${firstok:-}" ]; then
        if [ "$vline" -ge "$pline" ] || [ "$pline" -ge "$firstok" ]; then
            MISSING="$MISSING tap-thu-tu"
        fi
    fi
    [ "${okcount:-0}" -eq "$okwant" ] || MISSING="$MISSING ok-count=${okcount:-0}/$okwant"
    [ "${notok:-0}" -eq 0 ] || MISSING="$MISSING not-ok=${notok:-0}"
    # Dãy số phải đúng 1..N **theo thứ tự**: thiếu (2..37), trùng (1 1 2...), đảo thứ tự đều đỏ.
    gotseq="$(grep -E '^ok [0-9]+' "$log" | sed -e 's/^ok //' -e 's/ .*//' | tr '\n' ' ')"
    wantseq="$(awk -v n="$plan" 'BEGIN { for (i = 1; i <= n; i++) printf "%d ", i }')"
    [ "$gotseq" = "$wantseq" ] || MISSING="$MISSING tap-day-1..$plan"
    # PANIC: tổng đúng 1, không dòng nào lệch mẫu cho phép của ảnh.
    allow="$(panic_allow_re "$label")"
    ptotal="$(grep -cE '^[[:space:]]*PANIC' "$log" || true)"
    pbad="$(grep -E '^[[:space:]]*PANIC' "$log" | grep -Evc -- "$allow" || true)"
    [ "${pbad:-0}" -eq 0 ] || MISSING="$MISSING panic-khac=${pbad:-0}"
    [ "${ptotal:-0}" -eq 1 ] || MISSING="$MISSING panic-tong=${ptotal:-0}/1"
}

# Dòng `# hart …`/`# stack …` per-hart + số dòng `# timer hart=`/`# plic hart=` (ảnh chính).
check_hart_lines() {
    local log="$1" smp="$2" h=""
    h=0
    while [ "$h" -lt "$smp" ]; do
        grep -Eq "^# hart $h alive ticket=[0-9]+ tp=$h " "$log" || MISSING="$MISSING hart$h-alive"
        grep -Eq "^# stack hart=$h .* owner=$h .* frame_trap=1 rw_nx=1 guard_none=1$" "$log" ||
            MISSING="$MISSING hart$h-stack"
        h=$((h + 1))
    done
    # Không chỉ **đếm**: tập hart phải đúng `0..N-1` và mỗi hart đúng một dòng `ok=1` — lặp dòng
    # của hart 0 thay cho hart 1 (cùng số dòng) phải đỏ (reviewer KT-54 mục D).
    local want_ids="" got_ids=""
    h=0
    while [ "$h" -lt "$smp" ]; do
        want_ids="$want_ids$h "
        h=$((h + 1))
    done
    got_ids="$(grep -E '^# timer hart=[0-9]+ .* ok=1$' "$log" |
        sed -n 's/^# timer hart=\([0-9][0-9]*\) .*/\1/p' | sort -n | tr '\n' ' ')"
    [ "$got_ids" = "$want_ids" ] || MISSING="$MISSING timer-hart-lines"
    got_ids="$(grep -E '^# plic hart=[0-9]+ .* ok=1$' "$log" |
        sed -n 's/^# plic hart=\([0-9][0-9]*\) .*/\1/p' | sort -n | tr '\n' ' ')"
    [ "$got_ids" = "$want_ids" ] || MISSING="$MISSING plic-hart-lines"
}

# KT-66: **stack syscall per-hart** — mỗi hart online phải có đúng một dòng
# `# stack hart=K base=… top=… low=… used=… canary=0`, `base` **phân biệt** (không dùng chung
# stack cho smp 2/4), và `low` nằm trong `[base, top]`. Kiểm cả **tập** hart 0..N-1 (không chỉ đếm).
check_stack_lines() {
    local log="$1" smp="$2" h="" want_ids="" got_ids="" rows="" n="" b="" t="" l="" bb="" tt="" ll=""
    h=0
    while [ "$h" -lt "$smp" ]; do
        want_ids="$want_ids$h "
        h=$((h + 1))
    done
    got_ids="$(grep -E '^# stack hart=[0-9]+ ' "$log" |
        sed -n 's/^# stack hart=\([0-9][0-9]*\) .*/\1/p' | sort -n | uniq | tr '\n' ' ')"
    [ "$got_ids" = "$want_ids" ] || MISSING="$MISSING stack-hart-lines"
    local bases="" nb=""
    bases="$(grep -E '^# stack hart=[0-9]+ ' "$log" |
        sed -n 's/^# stack hart=[0-9][0-9]* base=\(0x[0-9a-f][0-9a-f]*\) .*/\1/p')"
    nb="$(printf '%s\n' "$bases" | sed '/^$/d' | sort -u | wc -l | tr -d ' ')"
    [ "${nb:-0}" -eq "$smp" ] || MISSING="$MISSING stack-distinct=${nb:-0}/$smp"
    # `low` phải nằm trong [base, top] và `top-base >= 32768`.
    # **Portable + fail-closed (reviewer KT-66 M3):** KHÔNG dùng `strtonum` (BSD awk không có ⇒
    # awk lỗi, stdout rỗng, biến `bad` rỗng ⇒ PASS giả). Parse bằng `sed` rồi tính bằng bash
    # `16#…`; nếu không parse đúng **đủ `smp` dòng** hoặc hex sai ⇒ thêm MISSING (ĐỎ).
    rows="$(grep -E '^# stack hart=[0-9]+ ' "$log" |
        sed -n 's/^# stack hart=\([0-9][0-9]*\) base=\(0x[0-9a-f]*\) top=\(0x[0-9a-f]*\) low=\(0x[0-9a-f]*\) .*/\1 \2 \3 \4/p')"
    n="$(printf '%s\n' "$rows" | sed '/^$/d' | wc -l | tr -d ' ')"
    [ "${n:-0}" -eq "$smp" ] || MISSING="$MISSING stack-rows=${n:-0}/$smp"
    while read -r h b t l; do
        [ -z "${h:-}" ] && continue
        case "$b$t$l" in
            *[!0-9a-fx]*) MISSING="$MISSING stack-hex=h$h" && continue ;;
        esac
        bb=$((16#${b#0x}))
        tt=$((16#${t#0x}))
        ll=$((16#${l#0x}))
        if [ "$ll" -lt "$bb" ] || [ "$ll" -gt "$tt" ]; then
            MISSING="$MISSING stack-low=h$h"
        fi
        if [ $((tt - bb)) -lt 32768 ]; then
            MISSING="$MISSING stack-block=h$h"
        fi
    done <<EOF
$rows
EOF
}

# KT-55: `# conf:` tự khai số bước trace của từng ca ⇒ số dòng `# trace: <scn> ` phải **bằng**
# đúng số đó (bắt mất/đội dòng mà không cần hardcode số ở runner), và mọi dòng phải đúng định dạng
# `# trace: <scn> <step> <op> [k=v …] => <kết quả>`.
check_trace() {
    local log="$1" smp="$2" scn="" steps="" n="" bad="" conf_count="" got="" want="" dup=""
    # Đúng 5 dòng `# conf:` của 5 ca (không thừa/thiếu/trùng).
    conf_count="$(grep -Ec '^# conf: scn=(22|23|24|25|26) name=' "$log" || true)"
    [ "${conf_count:-0}" -eq 5 ] || MISSING="$MISSING conf-count=${conf_count:-0}/5"
    for scn in 22 23 24 25 26; do
        steps="$(sed -n "s/^# conf: scn=$scn name=[^ ]* steps=\([0-9]*\) .*/\1/p" "$log" | head -1)"
        if [ -z "$steps" ]; then
            MISSING="$MISSING conf$scn"
            continue
        fi
        n="$(grep -c -E "^# trace: $scn " "$log" || true)"
        [ "${n:-0}" -eq "$steps" ] || MISSING="$MISSING trace$scn=${n:-0}/$steps"
        # Ca 24 được phép 0 bước **chỉ** khi smp=1 và conf ghi `skipped=smp1`; còn lại `steps>0`.
        if [ "$steps" -eq 0 ]; then
            if [ "$scn" = 24 ] && [ "$smp" -lt 2 ] &&
                grep -Eq '^# conf: scn=24 name=vm-tlb steps=0 skipped=smp1$' "$log"; then
                continue
            fi
            MISSING="$MISSING trace$scn-empty"
            continue
        fi
        # Tập `step` phải đúng **tập bước conformance** của tệp `.scn` (nguồn độc lập:
        # `check_scn.py --steps <scn>`), không trùng, không thiếu — không chỉ đếm dòng.
        got="$(sed -n "s/^# trace: $scn \([0-9][0-9]*\) .*/\1/p" "$log" | sort -n | tr '\n' ' ')"
        want="$(cd "$V2_ROOT/.." && python3.12 v2/test/scn/check_scn.py --steps "$scn" 2>/dev/null |
            tr ' ' '\n' | sed '/^$/d' | sort -n | tr '\n' ' ')"
        if [ -z "$want" ]; then
            MISSING="$MISSING trace$scn-steps-tool"
        else
            [ "$got" = "$want" ] || MISSING="$MISSING trace$scn-steps"
            [ "$(printf '%s' "$got" | wc -w | tr -d ' ')" -eq "$steps" ] ||
                MISSING="$MISSING trace$scn-count"
        fi
        dup="$(sed -n "s/^# trace: $scn \([0-9][0-9]*\).*/\1/p" "$log" | sort -n | uniq -d | tr '\n' ' ')"
        [ -z "$dup" ] || MISSING="$MISSING trace$scn-dup=$dup"
        bad="$(grep -E "^# trace: $scn " "$log" |
            grep -Evc '^# trace: [0-9]+ [0-9]+ [a-z_]+( [a-z_]+=[^ ]+)* => [^ ]+$' || true)"
        [ "${bad:-0}" -eq 0 ] || MISSING="$MISSING tracefmt$scn=$bad"
    done
    # D2-80: đúng 1 dòng `# conf image:` và BUILD_ID phải khớp ảnh **đang chạy** (tính lại từ ELF).
    local want_build="" got_build="" img=""
    img="$(image_path ktest)"
    want_build="$(image_build "$img")" || want_build=""
    got_build="$(sed -n 's/^# conf image: build=\([0-9a-f][0-9a-f]*\) text=[0-9][0-9]*$/\1/p' "$log" | head -1)"
    [ -n "$want_build" ] || MISSING="$MISSING image-build-tool"
    [ -n "$got_build" ] || MISSING="$MISSING image-build-missing"
    if [ -n "$want_build" ] && [ -n "$got_build" ] && [ "$got_build" != "${want_build#build=}" ]; then
        MISSING="$MISSING image-build-mismatch"
    fi
    [ "$(grep -c '^# conf image: ' "$log" || true)" -eq 1 ] || MISSING="$MISSING image-line-count"
}

# Mọi dòng `# evidence` bắt buộc của ảnh. In chẩn đoán ra stderr khi thiếu (để sửa nhanh).
check_evidence() {
    local log="$1" label="$2" smp="$3" hart="${4:-}" n=0 re=""
    while IFS= read -r re; do
        n=$((n + 1))
        if ! grep -Eq -- "$re" "$log"; then
            MISSING="$MISSING ev$n"
            echo "#   thiếu evidence#$n: $re" >&2
        fi
    done < <(evidence_for "$label" "$smp" "$hart")
}

# `validate_log <ảnh> <log> <smp>`: kiểm một log; trả 0 nếu sạch (MISSING rỗng), 1 nếu thiếu.
# Log QEMU có **CRLF ở dòng của firmware** (`OpenSBI v1.8.1\r`, `Platform HART Count … 2\r`) còn
# dòng kernel là LF ⇒ chuẩn hoá `\r` vào một bản tạm **trước khi** kiểm để mọi mẫu neo `$` đều đúng
# (không sửa log gốc của người đọc; bản tạm xoá ngay sau khi kiểm).
validate_log() {
    local label="$1" log="$2" smp="$3" norm="" rc=0
    MISSING=""
    [ -f "$log" ] || {
        MISSING="log-khong-thay"
        return 1
    }
    norm="$(mktemp "${TMPDIR:-/tmp}/kt54-norm.XXXXXX")" || {
        MISSING="khong-tao-duoc-tmp"
        return 1
    }
    if ! tr -d '\r' <"$log" >"$norm"; then
        rm -f "$norm"
        MISSING="doc-log-loi"
        return 1
    fi
    validate_norm "$label" "$norm" "$smp" || rc=1
    rm -f "$norm"
    return "$rc"
}

# `validate_norm <ảnh> <log-đã-chuẩn-hoá> <smp>`: thân kiểm (mọi `grep` neo `^…$` dùng tệp này).
validate_norm() {
    local label="$1" log="$2" smp="$3" plan="" n=0 name="" cnt="" vplan="" bh=""
    MISSING=""
    plan="$(expected_field "$label" 2)"
    vplan="$(manifest_for "$label" "$smp" | wc -l | tr -d ' ')"
    check_tap_common "$log" "$label" "$smp"
    # Mỗi TAP N: đúng **một** dòng `ok N - <tên>` (khớp manifest) ⇒ bắt cả trùng/thiếu/sai tên.
    n=0
    while IFS= read -r name; do
        n=$((n + 1))
        cnt="$(grep -c -x -F "ok $n - $name" "$log" || true)"
        if [ "${cnt:-0}" -ne 1 ]; then
            MISSING="$MISSING tap$n"
            echo "#   thiếu TAP #$n [$name] (thấy ${cnt:-0} dòng, cần đúng 1)" >&2
        fi
    done < <(manifest_for "$label" "$smp")
    [ "$n" -eq "$vplan" ] || MISSING="$MISSING manifest=$n/$vplan"
    case "$label" in
        ktest)
            check_hart_lines "$log" "$smp"
            check_trace "$log" "$smp"
            check_evidence "$log" "$label" "$smp"
            ;;
        ktest-guard)
            # Hart boot do OpenSBI chọn (có thể khác 0) ⇒ đọc từ log rồi đòi fault/load/store đúng hart đó.
            bh="$(sed -n 's/^# guard image: hart=\([0-9][0-9]*\) .*/\1/p' "$log" | head -1)"
            [ -n "$bh" ] || MISSING="$MISSING guard-image-hart"
            if [ -n "$bh" ]; then
                grep -Eq "^# guard load: hart=$bh cham 0x" "$log" || MISSING="$MISSING guard-load-line"
                grep -Eq "^# guard store: hart=$bh ghi 0x" "$log" || MISSING="$MISSING guard-store-line"
                grep -Eq "^# guard fault load scause=0xd stval=0x[0-9a-f]* sepc=0x[0-9a-f]* len=[24] recovered=1 hart=$bh$" "$log" ||
                    MISSING="$MISSING guard-load-real"
                awk -v h="$bh" '/^Platform HART Count *:/ { gsub(/\r/, ""); if ($NF > h) ok = 1 } END { exit !ok }' "$log" ||
                    MISSING="$MISSING guard-hart-out-of-range"
            fi
            check_evidence "$log" "$label" "$smp" "$bh"
            ;;
        ktest-user)
            check_stack_lines "$log" "$smp"
            check_evidence "$log" "$label" "$smp"
            ;;
        ktest-proc)
            check_stack_lines "$log" "$smp"
            check_evidence "$log" "$label" "$smp"
            ;;
        ktest-vblk)
            check_evidence "$log" "$label" "$smp"
            ;;
        ktest-exec)
            check_evidence "$log" "$label" "$smp"
            ;;
        ktest-con)
            check_evidence "$log" "$label" "$smp"
            ;;
    esac
    [ -z "$MISSING" ] || return 1
    return 0
}

# ------------------------------------------------------------- boot một ảnh

# `boot_image <ảnh> <log> <smp> [tham số QEMU thêm]`: 0 = QEMU tự thoát; 124 = quá 20 s đã kill
# **cả process group**; 125 = không khởi động được. Việc mở QEMU nằm trong `qemu_guard.sh` (KT-78).
boot_image() {
    local image="$1" out="$2" smp="$3"
    shift 3
    qg_boot "$out" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$image" -nographic -cpu rv64 -m 128M -smp "$smp" -no-reboot "$@"
}

# Bảng tổng kết: SUMMARY = các dòng `ảnh|smp|boot|TAP|kết quả|log`.
SUMMARY=""
record_row() {
    SUMMARY="$SUMMARY$1|$2|$3|$4|$5|$6
"
    ROWS=$((ROWS + 1))
}

# `run_one <ảnh> <smp> <boot-i>`: boot + kiểm + ghi bảng; 0 = sạch.
run_one() {
    local label="$1" smp="$2" idx="$3" image="" log="" oks="" t0="" dt="" disk=""
    local con_base="" con_feeder="" con_host=1 con_tx=""
    local -a qextra=()
    image="$(image_path "$label")"
    log="$(log_path "$label" "$smp" "$idx")"
    t0="$(now_s)"
    RUN_RESULT=""
    # KT-72: mỗi boot ảnh vblk nhận một ảnh đĩa raw **4 MiB** mới (8192 sector 512 B) qua
    # `-drive`+`-device virtio-blk-pci`; xoá trước mỗi boot để kết quả tất định (không phụ thuộc
    # lần chạy trước) và để test **format từ đầu** trên thiết bị thật.
    if [ "$label" = "ktest-vblk" ]; then
        disk="$LOGDIR/vblk-smp$smp-boot$idx.img"
        rm -f "$disk"
        if ! dd if=/dev/zero of="$disk" bs=1m count=4 2>/dev/null; then
            echo "not ok - $label smp=$smp boot $idx: không tạo được ảnh đĩa 4 MiB $disk" >&2
            fails=$((fails + 1))
            RUN_RESULT=FAIL
            record_row "$label" "$smp" "$idx" "-" FAIL "$disk"
            return 1
        fi
        qextra=(-drive "file=$disk,format=raw,if=none,id=hd0" -device virtio-blk-pci,drive=hd0)
    fi
    # KT-73: ảnh `ktest-con` cần **virtio-console** trên bus virtio-serial + một `chardev pipe`:
    # `.out` là bằng chứng **độc lập phía host** cho byte guest ghi (tx); `.in` (FIFO) là byte host
    # bơm cho rx. FIFO được tạo mới mỗi boot; bơm **sau** khi guest in marker `# con: rx armed`.
    if [ "$label" = "ktest-con" ]; then
        con_base="$LOGDIR/con-smp$smp-boot$idx"
        rm -f "$con_base.in" "$con_base.out"
        if ! mkfifo "$con_base.in"; then
            echo "not ok - $label smp=$smp boot $idx: không tạo được FIFO $con_base.in" >&2
            fails=$((fails + 1))
            RUN_RESULT=FAIL
            record_row "$label" "$smp" "$idx" "-" FAIL "mkfifo"
            return 1
        fi
        : >"$con_base.out"
        qg_tmp "$con_base.in" "$con_base.out"
        qextra=(-device virtio-serial-pci,id=vser0 \
            -chardev "pipe,id=ch0,path=$con_base" \
            -device virtconsole,bus=vser0.0,chardev=ch0)
    fi
    # KT-73: bơm byte host → `chr` của virtio-console khi guest đã arm `rx` (đồng bộ theo log).
    if [ -n "$con_base" ]; then
        (
            for _ in $(seq 1 400); do
                if grep -qF '# con: rx armed' "$log" 2>/dev/null; then
                    printf 'KT73-CON-RX-OK' >"$con_base.in" 2>/dev/null
                    break
                fi
                sleep 0.05
            done
        ) &
        con_feeder=$!
    fi
    # bash 3.2 + `set -u`: mảng rỗng `"${a[@]}"` bị coi là unbound ⇒ dùng dạng có điều kiện.
    if ! boot_image "$image" "$log" "$smp" ${qextra[@]+"${qextra[@]}"}; then
        dt=$(($(now_s) - t0))
        total_boots=$((total_boots + 1))
        fails=$((fails + 1))
        RUN_RESULT=TIMEOUT
        record_row "$label" "$smp" "$idx" "-" TIMEOUT "$log"
        echo "not ok - $label smp=$smp boot $idx: quá $((TIMEOUT_DS / 10)) s (treo, đã kill pid), log $log"
        return 1
    fi
    [ -n "$con_feeder" ] && wait "$con_feeder" 2>/dev/null
    total_boots=$((total_boots + 1))
    dt=$(($(now_s) - t0))
    oks="$(grep -cE '^ok [0-9]+' "$log" || true)"
    # KT-73: bằng chứng **host** cho hàng `tx` của virtio-console — chardev `.out` phải chứa
    # đúng byte guest ghi (không tin lời khai của guest). Đây là kiểm độc lập của runner.
    if [ -n "$con_base" ]; then
        if grep -qF 'KT73-CON-WRITE0123456789' "$con_base.out" 2>/dev/null; then
            con_tx="con-host-tx=1"
            echo "# con host: tx_file=$con_base.out bytes=$(wc -c <"$con_base.out" | tr -d ' ') ok=1"
        else
            con_host=0
            con_tx="con-host-tx=0"
            echo "not ok - $label: chardev .out thiếu byte guest ghi (bằng chứng host)" >&2
        fi
    fi
    if [ "$con_host" -eq 1 ] && validate_log "$label" "$log" "$smp"; then
        RUN_RESULT=PASS
        clean_boots=$((clean_boots + 1))
        record_row "$label" "$smp" "$idx" "${oks:-0}/$(expected_field "$label" 2)" PASS "$log $con_tx"
        echo "ok - $label smp=$smp boot $idx (${oks:-0} TAP, ${dt}s) log $log"
        return 0
    fi
    fails=$((fails + 1))
    RUN_RESULT=FAIL
    record_row "$label" "$smp" "$idx" "${oks:-0}/$(expected_field "$label" 2)" FAIL "$log $con_tx"
    echo "not ok - $label smp=$smp boot $idx: thiếu$MISSING, log $log"
    return 1
}

print_summary() {
    local l s
    echo "# ================= BẢNG TỔNG KẾT (image × smp × boot) ================="
    printf '# %-11s %-4s %-5s %-9s %-8s %s\n' "image" "smp" "boot" "TAP" "kết quả" "log"
    printf '%s' "$SUMMARY" | while IFS='|' read -r l s b t v p; do
        [ -n "${l:-}" ] || continue
        printf '# %-11s %-4s %-5s %-9s %-8s %s\n' "$l" "$s" "$b" "$t" "$v" "$p"
    done
    echo "# ------------------------- gộp theo (image, smp) -------------------------"
    for l in $LABELS; do
        for s in $SMP_LIST; do
            printf '%s' "$SUMMARY" | awk -F'|' -v l="$l" -v s="$s" '
                $1 == l && $2 == s { t++; if ($5 == "PASS") p++; else if ($5 == "TIMEOUT") to++; else if ($5 == "FAIL") f++ }
                END { if (t > 0) printf "# %-11s smp=%-2s %d/%d boot sạch (FAIL=%d TIMEOUT=%d)\n", l, s, p, t, f + 0, to + 0 }'
        done
    done
    echo "# ======================================================================="
}

# `--selftest-stack`: kiểm **chính** `check_stack_lines` bằng log tổng hợp (không cần QEMU) —
# bắt đúng low ngoài [base,top], block < 32 KiB, thiếu dòng/hart, hex hỏng; log đúng phải xanh.
# Ca "low ngoài" là ca reviewer KT-66 M3 yêu cầu (và chứng minh không còn PASS giả khi awk lỗi).
do_selftest_stack() {
    local dir="" ok=0 fail=0 total=0
    dir="$(mktemp -d "${TMPDIR:-/tmp}/kt66-stack.XXXXXX")" || die "không tạo được thư mục tạm"
    printf '%s\n' \
        '# stack hart=0 base=0x80415000 top=0x8041d000 low=0x8041c178 used=3720 canary=0' \
        '# stack hart=1 base=0x80432000 top=0x8043a000 low=0x8043a000 used=0 canary=0' \
        >"$dir/good.log"
    printf '%s\n' \
        '# stack hart=0 base=0x80415000 top=0x8041d000 low=0x8040f000 used=3720 canary=0' \
        '# stack hart=1 base=0x80432000 top=0x8043a000 low=0x8043a000 used=0 canary=0' \
        >"$dir/low-ngoai.log"
    printf '%s\n' \
        '# stack hart=0 base=0x80415000 top=0x80416000 low=0x80415000 used=3720 canary=0' \
        '# stack hart=1 base=0x80432000 top=0x8043a000 low=0x8043a000 used=0 canary=0' \
        >"$dir/block-nho.log"
    printf '%s\n' \
        '# stack hart=0 base=0x80415000 top=0x8041d000 low=0x8041c178 used=3720 canary=0' \
        >"$dir/thieu-hart.log"
    printf '%s\n' \
        '# stack hart=0 base=0xzz top=0x8041d000 low=0x8041c178 used=3720 canary=0' \
        '# stack hart=1 base=0x80432000 top=0x8043a000 low=0x8043a000 used=0 canary=0' \
        >"$dir/hex-hong.log"
    _st_case() {
        local name="$1" smp="$2" want="$3" f="$4"
        MISSING=""
        check_stack_lines "$f" "$smp"
        total=$((total + 1))
        if [ -z "$want" ]; then
            if [ -z "$MISSING" ]; then
                ok=$((ok + 1))
                echo "ok $total - $name"
            else
                fail=$((fail + 1))
                echo "not ok $total - $name (MISSING=$MISSING, cần xanh)"
            fi
        else
            if printf '%s' "$MISSING" | grep -q -- "$want"; then
                ok=$((ok + 1))
                echo "ok $total - $name"
            else
                fail=$((fail + 1))
                echo "not ok $total - $name (MISSING=$MISSING, cần khớp $want)"
            fi
        fi
    }
    _st_case "log đúng 2 hart (base khác, low trong [base,top], block 32 KiB)" 2 "" "$dir/good.log"
    _st_case "low NGOÀI [base,top] ⇒ ĐỎ (ca M3)" 2 "stack-low" "$dir/low-ngoai.log"
    _st_case "block < 32768 ⇒ ĐỎ" 2 "stack-block" "$dir/block-nho.log"
    _st_case "thiếu dòng hart 1 ⇒ ĐỎ" 2 "stack-rows" "$dir/thieu-hart.log"
    _st_case "hex hỏng ⇒ ĐỎ (fail-closed: không parse đủ dòng)" 2 "stack-rows" "$dir/hex-hong.log"
    rm -rf "$dir"
    echo "# selftest-stack: $ok/$total ca đúng như kỳ vọng"
    [ "$fail" -eq 0 ]
}

# ------------------------------------------------------------------ chế độ

parse_args() {
    case "${1:-}" in
        '') return 0 ;;
        --check-rev)
            # KT-86: kiểm **một** log đã có (không build/boot) — dùng cho ca kiểm + selftest.
            MODE=check-rev
            CHECK_LOG="${2:-}"
            CHECK_IMAGE="${3:-}"
            ;;
        --check-log)
            MODE=check-log
            CHECK_LABEL="${2:-}"
            CHECK_LOG="${3:-}"
            CHECK_SMP="${4:-1}"
            ;;
        --log-path)
            MODE=log-path
            LOGP_LABEL="${2:-}"
            LOGP_SMP="${3:-1}"
            LOGP_I="${4:-1}"
            ;;
        --print-expected) MODE=print-expected ;;
        --selftest-stack) MODE=selftest-stack ;;
        --print-evidence)
            MODE=print-evidence
            PRINT_LABEL="${2:-}"
            PRINT_SMP="${3:-1}"
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*) die "tham số '$1' không hiểu (xem --help)" ;;
        *)
            MODE=matrix
            N_BOOT="${1:-1}"
            SMP_ARG="${2:-1}"
            case "$SMP_ARG" in
                matrix) SMP_LIST="1 2 4" ;;
                *) SMP_LIST="$SMP_ARG" ;;
            esac
            ;;
    esac
}

check_smp_arg() {
    local s="" nsmp=0
    for s in $SMP_LIST; do
        nsmp=$((nsmp + 1))
        case "$s" in '' | *[!0-9]*) die "SMP [$s] không phải số" ;; esac
        [ "$s" -ge 1 ] && [ "$s" -le 8 ] || die "SMP=$s ngoài [1,8] (MAX_HARTS=8)"
    done
    # Danh sách rỗng (" " hoặc "") ⇒ `for` không chạy ⇒ **0 boot mà vẫn PASS** (reviewer KT-54 mục A):
    # fail-closed, không có cấu hình nào để boot thì không được coi là xanh.
    [ "$nsmp" -ge 1 ] || die "SMP_LIST rỗng (SMP='$SMP_ARG') — không có cấu hình nào để boot"
    SMP_COUNT="$nsmp"
}

do_print_expected() {
    local l
    echo "# BẢNG expected.txt ($EXPECTED_FILE) — mỗi ảnh một dòng <ảnh> <kế hoạch TAP> <số dòng ok>"
    printf '# %-11s %-7s %-6s %s\n' "image" "plan" "ok" "TAP runner kiểm tên"
    for l in $LABELS; do
        printf '# %-11s %-7s %-6s %s\n' "$l" "$(expected_field "$l" 2)" "$(expected_field "$l" 3)" \
            "$(manifest_for "$l" 1 | wc -l | tr -d ' ')"
    done
}

# `--print-evidence <ảnh> [SMP]`: in `<ordinal> <regex>` của danh sách evidence bắt buộc — để
# `selftest.sh`/tài liệu tra **ordinal** mà không hardcode (đổi danh sách thì test tự theo).
do_print_evidence() {
    local label="$1" smp="$2" n=0 re=""
    known_label "$label" || die "--print-evidence: nhãn [$label] không biết (dùng: $LABELS)"
    case "$smp" in '' | *[!0-9]*) die "--print-evidence: SMP [$smp] không phải số" ;; esac
    while IFS= read -r re; do
        n=$((n + 1))
        printf '%d\t%s\n' "$n" "$re"
    done < <(evidence_for "$label" "$smp")
}

do_log_path() {
    known_label "$LOGP_LABEL" || die "--log-path: nhãn [$LOGP_LABEL] không biết (dùng: $LABELS)"
    case "$LOGP_SMP" in '' | *[!0-9]*) die "--log-path: SMP [$LOGP_SMP] không phải số" ;; esac
    case "$LOGP_I" in '' | *[!0-9]*) die "--log-path: boot index [$LOGP_I] không phải số" ;; esac
    log_path "$LOGP_LABEL" "$LOGP_SMP" "$LOGP_I"
}

do_check_log() {
    local label="$1" log="$2" smp="$3" ev=""
    known_label "$label" || {
        echo "not ok - check-log: nhãn [$label] không biết (dùng: $LABELS)"
        return 1
    }
    case "$smp" in '' | *[!0-9]*) echo "not ok - check-log: SMP [$smp] không phải số"; return 1 ;; esac
    if ! load_expected; then
        echo "not ok - check-log $label smp=$smp: $EXPECT_ERR"
        return 1
    fi
    if ! check_manifests "$smp"; then
        echo "not ok - check-log $label smp=$smp: $EXPECT_ERR"
        return 1
    fi
    ev="$(evidence_for "$label" "$smp" "" | wc -l | tr -d ' ')"
    if validate_log "$label" "$log" "$smp"; then
        echo "ok - check-log $label smp=$smp (plan=$(expected_field "$label" 2) ok=$(expected_field "$label" 3), $ev evidence) log $log"
        return 0
    fi
    echo "not ok - check-log $label smp=$smp: thiếu$MISSING, log $log"
    return 1
}

# --------------------------------------------------- KT-86: cổng danh tính bản dựng
# KT-86 (D2-125.3): ảnh phải **chứng minh** nó build từ đúng mã nguồn đang đứng ở HEAD **trước khi**
# đo một slot nào. Ảnh test nằm ở các bin riêng (`src/ktest*.rs` — ngoài vùng ghi của KT-86) nên
# không in dòng `# build:`; vì vậy cổng boot **ảnh sản phẩm** (`src/main.rs` — cùng crate, cùng
# `build.rs`, cùng env danh tính trong cùng phiên build) rồi đối chiếu HEAD + trạng thái bẩn.
# Cổng đỏ ⇒ `exit 3` ("TỪ CHỐI ĐO", khác "đo ra đỏ" = 1): **không** đo gì, không in `ok`.
kt86_rev_gate() {
    local log="$LOGDIR/kt86-rev-gate.log" build_log="$LOGDIR/kt86-product-build.log"
    echo "# rev-gate (KT-86): đòi rev=HEAD và dirty=0 — build ảnh sản phẩm để đọc dòng '# build:'"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-riscv64 --target "$TARGET" --release >"$build_log" 2>&1; then
        tail -20 "$build_log" >&2
        vbrev_refuse product-build-failed "log=$build_log"
        return 1
    fi
    if [ ! -f "$PRODUCT_IMAGE" ]; then
        vbrev_refuse product-image-missing "image=$PRODUCT_IMAGE"
        return 1
    fi
    vbrev_summary "$V2_ROOT"
    if ! boot_image "$PRODUCT_IMAGE" "$log" 1; then
        vbrev_refuse probe-boot-failed "log=$log"
        return 1
    fi
    vbrev_gate "$log" "$V2_ROOT" "$PRODUCT_IMAGE"
}

# ------------------------------------------------------- vòng boot thật

run_matrix() {
    case "$N_BOOT" in '' | *[!0-9]*) die "NBOOT [$N_BOOT] không phải số" ;; esac
    [ "$N_BOOT" -ge 1 ] || die "NBOOT phải >= 1"
    check_smp_arg
    load_expected || die "$EXPECT_ERR"
    check_manifests "${SMP_LIST%% *}" || die "$EXPECT_ERR"
    mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"
    # KT-78/OD-K6-6: trạng thái hạ tầng **trước** phép đo (wall của matrix gồm cả build — D2-100).
    INFRA_LOG="$LOGDIR/kt78-infra.log"
    qg_infra before "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))

    command -v cargo >/dev/null 2>&1 || die "không có cargo"
    QEMU="$(command -v qemu-system-riscv64 || true)"
    [ -n "$QEMU" ] || die "không có qemu-system-riscv64"
    [ -f "$EXPECTED_FILE" ] || die "không thấy $EXPECTED_FILE"

    # OpenSBI `fw_dynamic` nạp trực tiếp, không U-Boot/EDK2 (D2-72 Q3).
    SBI="${OPENSBI:-}"
    if [ -z "$SBI" ]; then
        for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
            [ -f "$c" ] && {
                SBI="$c"
                break
            }
        done
    fi
    [ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

    # QEMU version: thiếu hoặc dưới sàn major.minor ⇒ FAIL, không tự hạ (D2-72 Q2).
    local qv qmaj qrest qmin
    qv="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
    [ -n "$qv" ] || die "không đọc được phiên bản QEMU"
    case "$qv" in *.*) ;; *) die "phiên bản QEMU [$qv] không có dạng major.minor" ;; esac
    qmaj="${qv%%.*}"
    qrest="${qv#*.}"
    qmin="${qrest%%.*}"
    case "$qmaj" in '' | *[!0-9]*) die "không đọc được major của QEMU [$qv]" ;; esac
    case "$qmin" in '' | *[!0-9]*) die "không đọc được minor của QEMU [$qv]" ;; esac
    if [ "$qmaj" -lt "$QEMU_MIN_MAJOR" ] ||
        { [ "$qmaj" -eq "$QEMU_MIN_MAJOR" ] && [ "$qmin" -lt "$QEMU_MIN_MINOR" ]; }; then
        die "QEMU $qv < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2) — không tự hạ"
    fi

    echo "# runner KT-54: expected.txt = nguồn sự thật duy nhất; TAP 1..N liên tục; đúng 1 PANIC theo mẫu"
    echo "# qemu $qv (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2) — icount shift=6 (D2-74)"
    echo "# opensbi $SBI"
    local l
    for l in $LABELS; do
        echo "# expected[$l]: plan=$(expected_field "$l" 2) ok=$(expected_field "$l" 3) ten-TAP=$(manifest_for "$l" 1 | wc -l | tr -d ' ')"
    done
    echo "# smp-list: $SMP_LIST — NBOOT=$N_BOOT — timeout=$((TIMEOUT_DS / 10)) s/boot — logdir=$LOGDIR"

    echo "# build ảnh test chính+guard ($TARGET, release, --features ktest)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest --bin vifast-kbin-ktest-guard \
        --features ktest --target "$TARGET" --release >"$LOGDIR/k3-ktest-build.log" 2>&1; then
        tail -20 "$LOGDIR/k3-ktest-build.log" >&2
        die "build ảnh test FAIL — CHƯA có ảnh test (xem $LOGDIR/k3-ktest-build.log)"
    fi
    # KT-72: ảnh **vblk** bật thêm feature `vblk` (PCI ECAM + virtio-blk) — tách khỏi ảnh trên để
    # `.text`/mapping của ảnh `ktest` không đổi (TAP 35 dùng VA 0x4000_0000).
    echo "# build ảnh test vblk ($TARGET, release, --features ktest,vblk)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest-vblk --features ktest,vblk \
        --target "$TARGET" --release >"$LOGDIR/k3-ktest-build-vblk.log" 2>&1; then
        tail -20 "$LOGDIR/k3-ktest-build-vblk.log" >&2
        die "build ảnh test vblk FAIL — CHƯA có ảnh test (xem $LOGDIR/k3-ktest-build-vblk.log)"
    fi
    # KT-73: ảnh **console** (UART RX IRQ → ring → U-mode + virtio-console) — feature `ktest,user,con`.
    echo "# build ảnh test con ($TARGET, release, --features ktest,user,con)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest-con --features ktest,user,con \
        --target "$TARGET" --release >"$LOGDIR/k3-ktest-build-con.log" 2>&1; then
        tail -20 "$LOGDIR/k3-ktest-build-con.log" >&2
        die "build ảnh test con FAIL — CHƯA có ảnh test (xem $LOGDIR/k3-ktest-build-con.log)"
    fi
    # KT-74: ảnh **exec/TLS** (feature `ktest,user,exec`) — tách khỏi `ktest-user`/`ktest-proc`
    # để TAP/`.text` của chúng không đổi.
    echo "# build ảnh test exec ($TARGET, release, --features ktest,user,exec)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest-exec --features ktest,user,exec \
        --target "$TARGET" --release >"$LOGDIR/k3-ktest-build-exec.log" 2>&1; then
        tail -20 "$LOGDIR/k3-ktest-build-exec.log" >&2
        die "build ảnh test exec FAIL — CHƯA có ảnh test (xem $LOGDIR/k3-ktest-build-exec.log)"
    fi
    # KT-53: ảnh user bật thêm feature `user` (module U-mode chỉ có trong ảnh này).
    echo "# build ảnh test user+proc ($TARGET, release, --features ktest,user)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest-user --bin vifast-kbin-ktest-proc --features ktest,user \
        --target "$TARGET" --release >"$LOGDIR/k3-ktest-build-user.log" 2>&1; then
        tail -20 "$LOGDIR/k3-ktest-build-user.log" >&2
        die "build ảnh test user FAIL — CHƯA có ảnh test (xem $LOGDIR/k3-ktest-build-user.log)"
    fi
    for l in $LABELS; do
        [ -f "$(image_path "$l")" ] || die "không thấy ảnh test $(image_path "$l") — CHƯA có ảnh test"
    done

    # Budget: mỗi ảnh test cũng phải ≤ 262.144 B (D2-72 Q5, không tách ngân sách riêng).
    for l in $LABELS; do
        bash "$BUDGET" "$(image_path "$l")" "$l" || die "budget ảnh test [$l] VƯỢT trần"
    done

    # Ảnh test phải mang chuỗi `ktest` (D2-08: tách khỏi ảnh sản phẩm).
    local hits img
    for l in $LABELS; do
        img="$(image_path "$l")"
        if command -v strings >/dev/null 2>&1; then
            hits="$(strings "$img" 2>/dev/null | grep -c ktest || true)"
        else
            hits="$(LC_ALL=C grep -a -o ktest "$img" 2>/dev/null | wc -l | tr -d ' ' || true)"
        fi
        [ "${hits:-0}" -ge 1 ] || die "ảnh [$l] không chứa chuỗi 'ktest' — nhầm ảnh?"
        echo "# image=$l ktest-strings=$hits"
    done

    # KT-86 (D2-125.3): CỔNG DANH TÍNH — chạy **trước** mọi phép đo; đỏ ⇒ exit 3, không slot nào.
    if ! kt86_rev_gate; then
        echo "# ktest.sh: TỪ CHỐI ĐO (KT-86/D2-125.3) — ảnh/cây nguồn không chứng minh được nguồn gốc"
        exit 3
    fi

    local smp="" i="" skip=""
    for smp in $SMP_LIST; do
        check_manifests "$smp" || die "$EXPECT_ERR"
        i=1
        while [ "$i" -le "$N_BOOT" ]; do
            skip=0
            for l in $LABELS; do
                if [ "$skip" -ne 0 ]; then
                    record_row "$l" "$smp" "$i" "-" SKIP "-"
                    continue
                fi
                # Chỉ **TIMEOUT** mới bỏ các ảnh còn lại của cùng (smp, boot) (máy có thể treo/chậm);
                # FAIL của một ảnh KHÔNG cần bỏ ảnh khác — boot độc lập, chạy tiếp để lấy thêm bằng chứng.
                if ! run_one "$l" "$smp" "$i"; then
                    [ "$RUN_RESULT" = TIMEOUT ] && skip=1
                fi
            done
            i=$((i + 1))
        done
    done

    # Không được phép "xanh vì không chạy gì": **mọi slot** (ảnh, smp, boot) phải có dòng trong
    # bảng — slot boot thật hoặc SKIP (chỉ sinh khi ảnh trước trong cùng vòng TIMEOUT).
    local want_rows=$((N_BOOT * SMP_COUNT * LABEL_COUNT))
    [ "$ROWS" -eq "$want_rows" ] ||
        die "chạy THIẾU slot: $ROWS/$want_rows (NBOOT=$N_BOOT × smp=$SMP_COUNT × $LABEL_COUNT ảnh) — không coi là PASS"
    [ "$total_boots" -ge 1 ] || die "không boot lần nào (0/$want_rows) — không coi là PASS"
    # KT-78/OD-K6-6: trạng thái hạ tầng **sau** phép đo; QEMU mồ côi/lạ ⇒ phép đo KHÔNG HỢP LỆ.
    qg_infra after "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
    echo "# infra: invalid=$INFRA_INVALID isolation_bad=$QG_ISO_BAD guard_log=$QG_LOG"
qg_guard_summary "$QG_LOG" >"$LOGDIR/guard-summary.log" 2>/dev/null || true
echo "# guard_summary_log=$LOGDIR/guard-summary.log"
    print_summary
    echo "# KTEST: $clean_boots/$total_boots boot sạch (smp=$SMP_ARG, NBOOT=$N_BOOT, ảnh/boot=$LABELS)"
    [ "$fails" -eq 0 ] || {
        echo "# KTEST: FAIL ($fails lỗi)"
        return 1
    }
    echo "# ktest.sh: PASS"
    return 0
}

# ------------------------------------------------------------------- main

parse_args "$@"
# `KT54_EXPECTED`: chỉ cho `--check-log` (selftest/mutation); vòng boot thật **luôn** đọc
# expected.txt trong repo để không có đường tắt đổi nguồn sự thật.
if [ -n "${KT54_EXPECTED:-}" ]; then
    [ "$MODE" = check-log ] || die "KT54_EXPECTED chỉ dùng cho --check-log (vòng boot thật đọc $HERE/expected.txt)"
    EXPECTED_FILE="$KT54_EXPECTED"
    echo "# KT54_EXPECTED=$EXPECTED_FILE (chỉ --check-log)" >&2
fi

total_boots=0
clean_boots=0
fails=0

case "$MODE" in
    print-expected)
        load_expected || die "$EXPECT_ERR"
        do_print_expected
        ;;
    log-path) do_log_path ;;
    print-evidence) do_print_evidence "$PRINT_LABEL" "$PRINT_SMP" ;;
    selftest-stack) do_selftest_stack || exit 1 ;;
    check-rev)
        # KT-86: 0 = ĐO ĐƯỢC, 3 = TỪ CHỐI ĐO (giữ đúng quy ước mã thoát của runner).
        vbrev_gate "$CHECK_LOG" "$V2_ROOT" "$CHECK_IMAGE" || exit 3
        ;;
    check-log)
        do_check_log "$CHECK_LABEL" "$CHECK_LOG" "$CHECK_SMP" || exit 1
        echo "# check-log: PASS"
        ;;
    matrix)
        run_matrix || exit 1
        ;;
    *) die "chế độ [$MODE] không biết" ;;
esac
exit 0
