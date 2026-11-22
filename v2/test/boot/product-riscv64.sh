#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-342** — runner ảnh **SẢN PHẨM** riscv64 hợp nhất: **một** boot làm **đồng thời**
# FS đọc/ghi (`ls`/`cat`/`write`), `netsrv` U-mode (`ping` qua kernel `NetSvc`) và `exec`
# (`run /hello.elf`) — **không** cần `ktest` (D2-08, D2-233, D2-243).
#
# # Vì sao có runner riêng (không nhét vào `run.sh`)
#
# `run.sh`/`soak.sh` đo **ảnh sản phẩm tối thiểu** (boot + `# k3 halt`, TAP `1..1`) — đó là phép
# đo soak 1.000 boot. Ảnh **hợp nhất** (`--features product-all`) là đầu vào khác (FS + mạng +
# exec, cần `-drive` + `-netdev`, thời gian boot dài hơn), nên tách runner để **không** đổi một
# dòng nào của `run.sh`/`soak.sh`/`ktest.sh` (D2-109/D2-111: mỗi ảnh một ngưỡng, không nới trần).
#
# # Việc runner làm (theo thứ tự, fail-closed)
#
#   1. **Cổng đồ thị feature** (đột biến (i) của KT-342): build `--features product-all` và đòi
#      **không** crate nào trong đồ thị bật feature `ktest` (harness). Bật lại `net = ["ktest"]`
#      kiểu cũ ⇒ đỏ **trước khi boot tới marker**.
#   2. **Budget**: `budget.sh <ảnh> product` (trần I7 262.144 B, D2-72 Q5).
#   3. **D2-08**: ảnh sản phẩm **không** chứa chuỗi harness (`ktest`, `# harness`, `not ok`,
#      `TAP_TESTS`). `TAP version 13`/`ok 1 - k3 boot` là **smoke sản phẩm** của K3, không phải
#      harness (đã có ở `run.sh`/`soak.sh` — giữ nguyên).
#   4. Boot **một** lần: `-drive` virtio-blk (FS bền) + `-netdev user`/virtio-net (netsrv), bơm
#      kịch bản qua UART **từ ngoài** (FIFO), chờ **evidence của khách** giữa các lệnh (không
#      `sleep` mù). Mọi dòng evidence phải xuất hiện **đúng** mẫu; thiếu bất kỳ dòng nào ⇒ đỏ.
#   5. **Ca âm** (chạy mặc định, không cờ):
#      - `--negative feature-graph`: build `product-all,mut-ktest-driver` ⇒ cổng đồ thị **phải**
#        đỏ (đột biến (i)).
#      - `--negative no-netsrv`: build `product-all,mut-no-netsrv` ⇒ boot lên nhưng **thiếu**
#        evidence `# netsrv:`/`# product net:` ⇒ các phép kiểm netsrv **phải** đỏ (đột biến (ii)).
#      - `--negative no-net`: **cùng** ảnh, KHÔNG có `-netdev` ⇒ kernel PANIC có tên
#        (`pci net: khong tim thay thiet bi`), **không** tới `# product halt:`.
#
# Dùng:
#   bash v2/test/boot/product-riscv64.sh [--image <ảnh>] [--features <danh-sách>]
#                                        [--negative feature-graph|no-netsrv|no-net]
#                                        [--smp N] [--print-logdir] [--help]
#
# Exit: 0 = mọi phép kiểm/đúng ca âm; 1 = có phép kiểm đỏ; 2 = lỗi cấu hình/môi trường;
#       3 = TỪ CHỐI ĐO (KT-86/D2-125.3: log thiếu/lệch `# build:` — không có bằng chứng).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
# shellcheck source=/dev/null
. "$HERE/lib/env.sh"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget.sh"
# KT-86 (D2-125.3): cổng danh tính bản dựng — một nguồn sự thật cho rev/dirty.
# shellcheck source=/dev/null
. "$HERE/build-rev.sh"
TARGET="riscv64gc-unknown-none-elf"
IMG_DIR="$V2_ROOT/kernel/target/$TARGET/release"
IMAGE=""
FEATURES="product-all"
SMP=1
NEG=""
TIMEOUT_DS=400 # 40 s/boot (FS + DHCP/ARP + ping + exec; soak giữ 20 s cho ảnh tối thiểu)
READY_DS=200   # 20 s tới dấu `shell>` đầu tiên
LOGDIR="$(v2_env_logdir kt342-product KT342_LOGDIR)"

die() {
    echo "product-riscv64.sh: $*" >&2
    exit 2
}
bad() {
    FAILS=$((FAILS + 1))
    echo "not ok $((CHECKS + FAILS)) - $*"
}
good() { echo "ok $((CHECKS + FAILS)) - $*"; }

usage() {
    cat <<'EOF'
Dùng: bash v2/test/boot/product-riscv64.sh [--image <ảnh>] [--features <ds>]
                                           [--negative feature-graph|no-netsrv|no-net]
                                           [--smp N] [--print-logdir] [--help]
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --image) IMAGE="${2:-}"; shift 2 ;;
        --features) FEATURES="${2:-}"; shift 2 ;;
        --negative) NEG="${2:-}"; shift 2 ;;
        --smp) SMP="${2:-}"; shift 2 ;;
        --print-logdir) echo "$LOGDIR"; exit 0 ;;
        -h | --help) usage; exit 0 ;;
        *) die "tham số lạ: $1" ;;
    esac
done
case "$SMP" in '' | *[!0-9]*) die "--smp phải là số" ;; esac
case "$NEG" in '' | feature-graph | no-netsrv | no-net) ;; *) die "--negative phải là feature-graph|no-netsrv|no-net" ;; esac
# Đột biến (ii) là một **feature đột biến** của cùng crate: runner tự bật nó khi build, để ca âm
# tái lập được bằng **một** lệnh (không sửa tay Cargo.toml).
if [ "$NEG" = "no-netsrv" ]; then
    case ",$FEATURES," in
        *,mut-no-netsrv,*) ;;
        *) FEATURES="$FEATURES,mut-no-netsrv" ;;
    esac
fi
[ -f "$MANIFEST" ] || die "không thấy $MANIFEST"
[ -f "$BUDGET" ] || die "không thấy $BUDGET"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
mkdir -p "$LOGDIR" || die "không tạo được logdir $LOGDIR"
CHECKS=0
FAILS=0

# ---------------------------------------------------------------- 1) build + cổng đồ thị feature
feature_graph_guard() { # $1 = danh sách feature
    local out
    # **Vì sao `-i vifast-plat-riscv64`**: `cargo tree -e features` (chiều xuôi) chỉ in cạnh
    # feature khi nó **dẫn tới** một phụ thuộc; feature `ktest = []` của crate arch không dẫn tới
    # đâu nên **vắng** ở chiều xuôi (đo được: cùng lệnh, cùng manifest). Chiều **ngược** in đúng
    # `vifast-plat-riscv64 feature "ktest"` khi ảnh sản phẩm (hoặc bin test) bật nó.
    out="$(cargo tree --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --target "$TARGET" \
        --features "$1" -e features -i vifast-plat-riscv64 2>&1)" || {
        echo "$out" >&2
        return 2
    }
    if printf '%s\n' "$out" | grep -q 'vifast-plat-riscv64 feature "ktest"'; then
        echo "# product-riscv64: ĐỒ THỊ FEATURE bật harness 'ktest' trên crate arch" >&2
        printf '%s\n' "$out" | grep 'feature "ktest"' | head -3 >&2
        return 1
    fi
    return 0
}

if [ "$NEG" = "feature-graph" ]; then
    # **Đột biến (i)**: tái lập nhãn cũ `net = ["ktest"]` ⇒ cổng đồ thị PHẢI đỏ.
    if feature_graph_guard "$FEATURES,mut-ktest-driver"; then
        bad "feature-graph: đồ thị product-all,mut-ktest-driver KHÔNG chứa ktest (đột biến không có răng)"
    else
        good "feature-graph: mut-ktest-driver bị bắt (ktest trong đồ thị feature) — đỏ đúng lý do"
    fi
    echo
    echo "# product-riscv64: negative=feature-graph checks=$CHECKS fails=$FAILS"
    [ "$FAILS" -eq 0 ] || exit 1
    exit 0
fi

# D2-245/D2-248: sub-agent không được chồng phép đo. Runner này **chỉ đọc** khoá (không giữ);
# Lead đang T2/soak ⇒ TỪ CHỐI ĐO (exit 2). Ca `feature-graph` ở trên không mở QEMU nên không cần.
# shellcheck source=/dev/null
. "$V2_ROOT/tools/measure-lock.sh"
if measure_lock_is_held; then
    echo "# product-riscv64.sh: TỪ CHỐI ĐO — máy đo đang giữ khoá (T2/soak của Lead)" >&2
    exit 2
fi

if [ -z "$IMAGE" ]; then
    echo "# build ảnh sản phẩm hợp nhất ($TARGET, release, features=$FEATURES)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 \
        --target "$TARGET" --release --features "$FEATURES" >"$LOGDIR/kt342-build.log" 2>&1; then
        tail -20 "$LOGDIR/kt342-build.log" >&2
        die "build ảnh sản phẩm FAIL (xem $LOGDIR/kt342-build.log)"
    fi
    IMAGE="$IMG_DIR/vifast-kbin-riscv64"
fi
[ -x "$IMAGE" ] || die "không thấy ảnh $IMAGE"

if feature_graph_guard "$FEATURES"; then
    good "feature-graph: không crate nào bật 'ktest' (driver KHÔNG còn là điều kiện harness)"
else
    bad "feature-graph: đồ thị feature bật 'ktest' trên ảnh sản phẩm"
fi

# ---------------------------------------------------------------- 2) budget + D2-08
if bash "$BUDGET" "$IMAGE" product >"$LOGDIR/kt342-budget.log" 2>&1; then
    good "budget: $(sed -n 's/^# image=product //p' "$LOGDIR/kt342-budget.log" | head -1)"
else
    bad "budget: ảnh sản phẩm VƯỢT trần (xem $LOGDIR/kt342-budget.log)"
    sed -n '1,3p' "$LOGDIR/kt342-budget.log" >&2
fi

# D2-08: danh sách marker **harness** (khác `run.sh` chỉ grep 'ktest'); `TAP version`/`ok 1 - k3
# boot` là smoke sản phẩm K3, KHÔNG nằm trong danh sách.
HARNESS_MARKERS=('ktest' '# harness' 'not ok' 'TAP_TESTS')
MARKER_HITS=0
for marker in "${HARNESS_MARKERS[@]}"; do
    n="$(LC_ALL=C grep -a -o -F "$marker" "$IMAGE" 2>/dev/null | wc -l | tr -d ' ')"
    MARKER_HITS=$((MARKER_HITS + n))
done
if [ "$MARKER_HITS" -eq 0 ]; then
    good "markers: 0 harness marker (${#HARNESS_MARKERS[@]} mẫu) — D2-08"
else
    bad "markers: $MARKER_HITS harness marker trong ảnh sản phẩm (D2-08)"
fi

# ---------------------------------------------------------------- 3) boot + kịch bản serial
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"
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
[ -f "${SBI:-/nonexistent}" ] || die "không thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

# `grep -a` khớp dòng `$2` trong log `$1` (không dùng `sleep` mù: chờ evidence của khách).
has() { LC_ALL=C grep -a -q -E "$2" "$1" 2>/dev/null; }

report_check() { # <nhãn> <log> <regex>
    if has "$2" "$3"; then good "$1"; else bad "$1 (thiếu /$3/)"; fi
}

boot_and_feed() { # $1 = log, $2 = 1 nếu bật netdev
    local log="$1" with_net="$2" fifo pid waited rc=0
    fifo="$LOGDIR/fifo.$$"
    rm -f "$fifo"
    mkfifo "$fifo" || die "không tạo được FIFO"
    local disk="$LOGDIR/disk.$$.img"
    dd if=/dev/zero of="$disk" bs=1024 count=4096 >/dev/null 2>&1 || die "không tạo được đĩa"
    local args=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" -nographic
        -cpu rv64 -m 128M -smp "$SMP" -no-reboot
        -drive "file=$disk,if=none,id=d0,format=raw" -device "virtio-blk-pci,drive=d0")
    if [ "$with_net" = 1 ]; then
        args+=(-netdev user,id=n0 -device virtio-net-pci,netdev=n0)
    fi
    : >"$log"
    (exec "$QEMU" "${args[@]}" <"$fifo" >"$log" 2>&1) &
    pid=$!
    exec 9<>"$fifo"
    # 1) chờ `shell>` đầu tiên (U-mode đã lên).
    waited=0
    while [ "$waited" -lt "$READY_DS" ] && ! has "$log" '^shell>$'; do
        if ! kill -0 "$pid" 2>/dev/null; then break; fi
        sleep 0.1
        waited=$((waited + 1))
    done
    if ! has "$log" '^shell>$'; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        exec 9>&-
        rm -f "$fifo" "$disk"
        return 1
    fi
    # 2) gửi từng lệnh, chờ **evidence của khách** rồi mới gửi lệnh kế (không `sleep` mù).
    send_wait() { # <lệnh> <regex evidence>
        printf '%s\n' "$1" >&9
        local n=0
        while [ "$n" -lt 200 ]; do
            has "$log" "$2" && return 0
            kill -0 "$pid" 2>/dev/null || return 1
            sleep 0.05
            n=$((n + 1))
        done
        return 1
    }
    send_wait 'ls' '^# shell: ls n=[0-9]+ path=/' || rc=1
    send_wait 'write /note.txt hello' '^# shell: write /note.txt bytes=5 sync=ok$' || rc=1
    send_wait 'cat /note.txt' '^# shell: cat ok bytes=5 first=hello$' || rc=1
    if [ "$with_net" = 1 ]; then
        send_wait 'ping 10.0.2.2' '^# shell: ping 10\.0\.2\.2 rtt_ms=[0-9]+ err=0$' || rc=1
    fi
    send_wait 'run /hello.elf' '^# product exec: path=/hello\.elf ' || rc=1
    # 3) chờ ảnh tự halt (hello `Exit(0)` ⇒ `# product halt:` + QEMU rc=0), hạn cứng.
    waited=0
    while [ "$waited" -lt "$TIMEOUT_DS" ]; do
        if ! kill -0 "$pid" 2>/dev/null; then break; fi
        sleep 0.1
        waited=$((waited + 1))
    done
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null && rc=$?
    exec 9>&-
    rm -f "$fifo" "$disk"
    return "$rc"
}

check_log() { # $1 = log, $2 = nhóm (product|no-netsrv)
    local log="$1" group="${2:-product}"
    report_check "boot: image=product (kernel thật)" "$log" '^# k3 image=product '
    report_check "boot: shell U-mode lên (cmds=…/ping/run)" "$log" '^# shell: up cmds=help(/[a-z]+)+$'
    report_check "fs: volume virtio-blk thật" "$log" '^# product fs: medium=virtio-blk '
    report_check "fs: ls đọc thư mục gốc" "$log" '^# shell: ls n=[0-9]+ path=/$'
    report_check "fs: write + fsync" "$log" '^# shell: write /note\.txt bytes=5 sync=ok$'
    report_check "fs: cat đọc lại đúng byte" "$log" '^# shell: cat ok bytes=5 first=hello$'
    if [ "$group" = "product" ]; then
        report_check "net: netsrv U-mode chạy" "$log" '^# netsrv: ping 10\.0\.2\.2 rtt_ms=[0-9]+ err=0$'
        report_check "net: shell nhận reply của netsrv" "$log" '^# shell: ping 10\.0\.2\.2 rtt_ms=[0-9]+ err=0$'
        report_check "net: kernel ping thật (served=1 sent=1 received=1)" "$log" '^# product net: served=1 denied=0 failed=0 err=0 rtt_ms=[0-9]+ sent=1 received=1 '
        report_check "net: tổng kết replies=1" "$log" '^# product halt net: replies=1 '
    fi
    report_check "exec: bảng ảnh + exec nguyên tử" "$log" '^# product exec: path=/hello\.elf .*check=1$'
    report_check "exec: ảnh đích chạy thật (RUN OK)" "$log" '^RUN OK$'
    report_check "exec: dấu evidence của ảnh đích" "$log" '^# hello: run ok$'
    report_check "halt: shell_exit=0 (thoát sạch)" "$log" '^# product halt: shell_exit=0 '
    if [ "$group" = "product" ]; then
        report_check "halt: execs=1" "$log" '^# product halt exec: execs=1$'
    fi
    # Bất biến âm: không PANIC, không `not ok`, đúng một smoke `ok 1 - k3 boot`.
    if has "$log" '^PANIC'; then bad "log: có PANIC"; else good "log: không PANIC"; fi
    if has "$log" '^not ok'; then bad "log: có 'not ok'"; else good "log: không 'not ok'"; fi
    local ok_count
    ok_count="$(LC_ALL=C grep -a -c '^ok 1 - k3 boot$' "$log" 2>/dev/null || true)"
    if [ "${ok_count:-0}" -eq 1 ]; then good "log: đúng một 'ok 1 - k3 boot'"; else bad "log: 'ok 1 - k3 boot' = ${ok_count:-0} (mong 1)"; fi
}

# ---------------------------------------------------------------- ca âm: no-net (fail-closed)
if [ "$NEG" = "no-net" ]; then
    log="$LOGDIR/kt342-no-net.log"
    boot_and_feed "$log" 0 || true
    # **KT-342a (D2-249)** — hợp đồng **đổi có văn bản**: không có NIC ⇒ OS **vẫn boot** và in
    # `# product net: no device`; chỉ *có thiết bị mà hỏng* mới PANIC. Lý do: `guard_selftest.sh`
    # ca clean-boot (boot ảnh sản phẩm **không** `-netdev`) đã **đỏ** vì PANIC ⇒ một OS không được
    # chết khi máy không có card mạng.
    if has "$log" '^# product net: no device'; then
        good "no-net: báo 'no device' rõ ràng (chạy tiếp, không PANIC)"
    else
        bad "no-net: thiếu '# product net: no device'"
    fi
    if has "$log" '^PANIC'; then bad "no-net: vẫn PANIC (phải chỉ báo no device)"; else good "no-net: không PANIC"; fi
    if has "$log" '^# netsrv:'; then bad "no-net: vẫn có '# netsrv:' (ca âm không có răng)"; else good "no-net: không có '# netsrv:' (đúng: không NIC)"; fi
    echo
    echo "# product-riscv64: negative=no-net checks=$CHECKS fails=$FAILS log=$log"
    [ "$FAILS" -eq 0 ] || exit 1
    exit 0
fi

# ---------------------------------------------------------------- dương tính
log="$LOGDIR/kt342-product.log"
boot_and_feed "$log" 1 || true

if [ "$NEG" = "no-netsrv" ]; then
    # **Đột biến (ii)**: boot phải lên tới `# product halt:` nhưng **thiếu** đường netsrv.
    if has "$log" '^# product halt:'; then good "no-netsrv: boot vẫn lên (chỉ mất đường netsrv)"; else bad "no-netsrv: boot không tới halt (đỏ vì lý do SAI)"; fi
    if has "$log" '^# netsrv:'; then bad "no-netsrv: vẫn có '# netsrv:' (đột biến không có răng)"; else good "no-netsrv: thiếu '# netsrv:'"; fi
    if has "$log" '^# product net: served='; then bad "no-netsrv: vẫn phục vụ request (đột biến không có răng)"; else good "no-netsrv: kernel không phục vụ ping"; fi
    if has "$log" '^# shell: ping 10\.0\.2\.2 rtt_ms='; then bad "no-netsrv: shell vẫn nhận reply (đột biến không có răng)"; else good "no-netsrv: shell không nhận reply"; fi
    # Các nhánh còn lại (FS + exec) vẫn phải xanh ⇒ chứng minh đột biến chỉ cắt đúng đường netsrv.
    check_log "$log" no-netsrv
    echo
    echo "# product-riscv64: negative=no-netsrv checks=$CHECKS fails=$FAILS log=$log"
    [ "$FAILS" -eq 0 ] || exit 1
    exit 0
fi

check_log "$log" product
echo
echo "# product-riscv64: checks=$CHECKS fails=$FAILS image=$IMAGE log=$log"
[ "$FAILS" -eq 0 ] || exit 1

# **KT-86/D2-125.3** — cổng danh tính bản dựng: log phải có `# build:` khớp HEAD + dirty=0 và ảnh
# phải chứa đúng chuỗi danh tính. Chạy **sau** các phép kiểm để log chẩn đoán vẫn in, nhưng
# exit = 3 (TỪ CHỐI ĐO) ⇒ **không** lần chạy nào trên cây bẩn được tính là bằng chứng.
if vbrev_gate "$log" "v2/kernel/vifast-kbin-riscv64" "$IMAGE"; then
    echo "# product-riscv64: rev-gate ACCEPT (D2-125.3)"
else
    echo "# product-riscv64: TỪ CHỐI ĐO (KT-86/D2-125.3) — log/ảnh không chứng minh nguồn gốc" >&2
    exit 3
fi
exit 0
