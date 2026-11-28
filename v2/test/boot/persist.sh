#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-75 — runner `test-persist` **7 pha** trên virtio-blk, mỗi pha sống qua **reboot QEMU thật**.
#
# Vì sao runner riêng: một pha = **hai** tiến trình QEMU (ghi rồi thoát; tiến trình **mới** đọc
# lại) ⇒ không vừa mô hình “mỗi ảnh một boot” của `ktest.sh` (`expected.txt`: một plan TAP/boot).
# Ở đây 10 boot chia thành:
#
#   b01  format + ghi P1                 b06  đọc lại P5 + ghi P6
#   b02  đọc lại P1 + ghi P2             b07  đọc lại P6 + ghi P7
#   b03  đọc lại P2 + ghi P3             b08  đọc lại P7 + in `phases=7 ok=7`
#   b04  đọc lại P3 + ghi P4             b09  **ca âm**: ghi `/pneg.txt` KHÔNG flush (mất)
#   b05  đọc lại P4 + ghi P5             b10  **ca âm**: boot mới thấy `/pneg.txt` vắng
#
# Bằng chứng **độc lập phía host** (không tin kernel): sau **mỗi** boot, `vifast-fsutil fsck` +
# `ls` + `stat` + `get` trên **cùng** ảnh đĩa, so **từng byte** với mô hình sinh lại ở
# `persist_host.py`; đồng thời sha256 ảnh phải **không đổi** trong lúc kiểm host (`fsutil_mutated=0`)
# và **không đổi** qua ca âm (không có gì ra thiết bị).
#
# Dùng:
#   bash v2/test/boot/persist.sh                 # chạy đủ 10 boot + kiểm host (PASS/FAIL)
#   bash v2/test/boot/persist.sh --red-selftest  # chứng minh bộ kiểm **không rỗng**: sửa payload
#                                                #   trên đĩa bằng fsutil rồi đòi verify QEMU đỏ
#   bash v2/test/boot/persist.sh --soak N        # **KT-136**: N vòng × 10 boot = 10N boot trên
#                                                #   ảnh đĩa thật; mỗi boot kiểm host; bất biến
#                                                #   "mọi vòng cùng một sha256 ảnh đĩa" (không rò)
#   bash v2/test/boot/persist.sh --crash         # **KT-136/KT-408d**: cắt điện **tất định** giữa
#                                                #   lúc ghi pha 7 (kernel tự halt(77), KHÔNG poll)
#                                                #   rồi kiểm ảnh đĩa + journal replay + phục hồi
#   bash v2/test/boot/persist.sh --crash-nocut   # **KT-408d Q7**: đỏ-trước — cùng chuỗi boot
#                                                #   nhưng KHÔNG cắt (mode=0): pha 7 ghi trọn, bộ
#                                                #   kiểm host PHẢI đỏ `records=13`/`partial=0`
#   KT75_CRASH_AFTER=<1..143> bash … --crash     # điểm cắt (mặc định 72/144 sector của /p7.bin)
#   KT75_IMAGE=<elf> KT75_SKIP_BUILD=1 bash …    # dùng ảnh có sẵn (đỏ trước: ảnh không có persist)
#   KT75_LOGDIR=<dir> bash …                     # nơi để log/ảnh đĩa (mặc định /tmp/kt75-persist)
# Exit 0 = 10/10 boot xanh + 7/7 pha + ca âm đúng (hoặc soak/crash xanh); 1 = có lỗi (in từng
#      dòng `not ok`); 3 = TỪ CHỐI ĐO (KT-86/D2-125.3: `# build:` thiếu/lệch HEAD/dirty=1).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
# **KT-333d (D2-248 mục 2b)** — LOGDIR (và SOCK ngắn) dùng chung `v2/test/boot/lib/env.sh`
# (KT-333c đã làm logdir theo phiên inline ở 6 runner; nay gom một chỗ, không lặp code).
. "$HERE/lib/env.sh"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
TARGET="riscv64gc-unknown-none-elf"
KMANIFEST="$V2_ROOT/kernel/Cargo.toml"
V2MANIFEST="$V2_ROOT/Cargo.toml"
IMAGE="${KT75_IMAGE:-${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release/vifast-kbin-ktest-persist}"
FSUTIL="${KT75_FSUTIL:-$V2_ROOT/target/debug/vifast-fsutil}"
LOGDIR="$(v2_env_logdir kt75-persist KT75_LOGDIR nopid)"
# **KT-333d (D2-248 mục 2b)** — in logdir đã giải rồi thoát (cùng hợp đồng 6 runner còn lại),
# để chứng minh ca âm `$TMPDIR` dài mà không cần build/boot.
if [ "${1:-}" = "--print-logdir" ]; then
    echo "$LOGDIR"
    exit 0
fi
# KT-242 (P3): LOGDIR nay **riêng theo lần chạy** ⇒ phải tạo **trước** lần ghi log đầu tiên
# (`build_everything` ghi `kt75-build-image.log` trước `fresh_disk`). Trước đây đường cố định
# `/tmp/kt75-persist` tồn tại sẵn từ lần chạy trước nên lỗi này bị che.
mkdir -p "$LOGDIR" 2>/dev/null || true
DISK="$LOGDIR/kt75-disk.img"
HOSTPY="$HERE/persist_host.py"
# KT-86 (D2-125.3): cổng danh tính bản dựng — nguồn sự thật duy nhất cho rev/dirty.
. "$HERE/build-rev.sh"
# Ảnh **sản phẩm** không feature — ảnh duy nhất in dòng `# build:` (KT-86).
PRODUCT_IMAGE="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release/vifast-kbin-riscv64"
TIMEOUT_DS=900 # 90 s/boot (pha 7 ghi 144 block ⇒ ~900 request virtio dưới `-icount`)
QEMU_MIN_MAJOR=9
QEMU_MIN_MINOR=2
QEMU_ICOUNT="-icount shift=6" # D2-74 (chỉ runner; run.sh/soak không icount)
# **KT-408d (Q7)** — điểm cắt điện mặc định: 72/144 sector của `/p7.bin` (nửa file, đã đi qua
# bảng gián tiếp đơn nên điểm cắt không rơi vào nhánh đặc biệt nào của FS). Đổi bằng env để đo
# nhiều điểm cắt mà không sửa mã; runner **luôn** kiểm cỡ đĩa đúng `CRASH_AFTER*512 B`.
CRASH_AFTER="${KT75_CRASH_AFTER:-72}"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

FAILS=0
BOOTS=0
OK_BOOTS=0
ROWS=""
PIDS=""

die() {
    echo "persist.sh: $*" >&2
    exit 2
}

# KT-78 (OD-K6-6): mỗi boot đi qua `qemu_guard.sh` — QEMU trong process group riêng, timeout kill
# **cả group**, trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). `pid` của từng boot vẫn
# là PID **do script này tạo** (lấy từ `$!` bên trong guard) ⇒ bằng chứng "7 pha = 10 tiến trình
# QEMU khác nhau" của KT-75 giữ nguyên.
. "$HERE/qemu_guard.sh"
# **KT-411b** — khoá + phát hành ảnh kiểm dùng chung (một tên tệp build cho 13 suite).
. "$HERE/lib/ktest-build.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
INFRA_INVALID=0

PYTHON=""
find_python() {
    local c
    for c in python3.12 python3.11 python3; do
        if command -v "$c" >/dev/null 2>&1; then
            PYTHON="$c"
            return 0
        fi
    done
    return 1
}

bad() {
    echo "not ok - $*"
    FAILS=$((FAILS + 1))
}

build_everything() {
    command -v cargo >/dev/null 2>&1 || die "không có cargo"
    if [ "${KT75_SKIP_BUILD:-0}" != "1" ]; then
        # **KT-411b** (D2-259 mục 2): **một** bin kiểm `vifast-kbin-ktest`, bộ ca chọn bằng
        # feature `suite-*`; `cp` ra **đúng tên ảnh cũ** nên `IMAGE`/`budget.sh` không đổi.
        echo "# build ảnh persist ($TARGET, release, --features ktest,suite-persist)"
        if ! ktest_build_suite suite-persist "$LOGDIR/kt75-build-image.log" "$IMAGE" \
            "$KMANIFEST" "$TARGET"; then
            tail -20 "$LOGDIR/kt75-build-image.log" >&2
            die "build ảnh persist FAIL"
        fi
        echo "# build vifast-fsutil (host)"
        if ! cargo build --manifest-path "$V2MANIFEST" -p vifast-fsutil \
            >"$LOGDIR/kt75-build-fsutil.log" 2>&1; then
            tail -20 "$LOGDIR/kt75-build-fsutil.log" >&2
            die "build vifast-fsutil FAIL"
        fi
    fi
    [ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
    [ -x "$FSUTIL" ] || die "không thấy fsutil $FSUTIL"
    bash "$HERE/budget.sh" "$IMAGE" ktest-persist >"$LOGDIR/kt75-budget.log" 2>&1 ||
        die "budget ảnh persist VƯỢT trần (xem $LOGDIR/kt75-budget.log)"
    grep -E "^# image=" "$LOGDIR/kt75-budget.log" || true
}

# KT-86 (D2-125.3): **trước** 10 boot persist, ảnh phải chứng minh nguồn gốc: in
# `# build: rev=<12 hex> dirty=<0|1>` (từ `const`, D2-102) khớp `git rev-parse HEAD` + cây sạch.
# Ảnh persist là bin riêng (`src/ktest_persist.rs`, ngoài vùng ghi KT-86) nên không in dòng này;
# build + boot **ảnh sản phẩm** (cùng crate/build script/env danh tính) làm chứng. Đỏ ⇒ exit 3
# trước khi ghi/đọc bất kỳ pha nào (không có phép đo nào chạy trên artifact lệch nguồn).
rev_gate() {
    local plog="$LOGDIR/kt86-rev-gate.log" pbuild="$LOGDIR/kt86-product-build.log"
    echo "# rev-gate (KT-86): build + boot MỘT lần ảnh sản phẩm để đọc dòng '# build:'"
    vbrev_summary "$REPO_ROOT"
    if ! cargo build --manifest-path "$KMANIFEST" -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 \
        --target "$TARGET" --release >"$pbuild" 2>&1; then
        tail -20 "$pbuild" >&2
        vbrev_refuse product-build-failed "log=$pbuild"
        return 1
    fi
    [ -f "$PRODUCT_IMAGE" ] || {
        vbrev_refuse product-image-missing "image=$PRODUCT_IMAGE"
        return 1
    }
    if ! qg_boot "$plog" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT \
        -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$PRODUCT_IMAGE" \
        -nographic -cpu rv64 -m 128M -smp 1 -no-reboot; then
        vbrev_refuse probe-boot-failed "log=$plog"
        return 1
    fi
    vbrev_gate "$plog" "$REPO_ROOT" "$PRODUCT_IMAGE"
}

setup_tools() {
    find_python || die "không có python3"
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
    [ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"
    local qv qmaj qrest qmin
    qv="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
    [ -n "$qv" ] || die "không đọc được phiên bản QEMU"
    qmaj="${qv%%.*}"
    qrest="${qv#*.}"
    qmin="${qrest%%.*}"
    case "$qmaj" in '' | *[!0-9]*) die "phiên bản QEMU [$qv] lạ" ;; esac
    case "$qmin" in '' | *[!0-9]*) die "phiên bản QEMU [$qv] lạ" ;; esac
    if [ "$qmaj" -lt "$QEMU_MIN_MAJOR" ] ||
        { [ "$qmaj" -eq "$QEMU_MIN_MAJOR" ] && [ "$qmin" -lt "$QEMU_MIN_MINOR" ]; }; then
        die "QEMU $qv < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR"
    fi
    echo "# qemu $qv (icount shift=6) — opensbi $SBI"
}

fresh_disk() {
    mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"
    # 4 MiB = 8192 sector 512 B = đúng `vifast_fs::TOTAL_BLOCKS`.
    dd if=/dev/zero of="$DISK" bs=4096 count=1024 >/dev/null 2>&1 ||
        die "không tạo được ảnh đĩa 4 MiB $DISK"
}

sha_of() {
    "$PYTHON" -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$1"
}

# `run_boot <tag> <verify> <write> <flags> <mode> <nonce>`: một tiến trình QEMU trên cùng ảnh đĩa.
run_boot() {
    local tag="$1" verify="$2" write="$3" flags="$4" mode="$5" nonce="$6"
    local plan="$LOGDIR/$tag.plan" log="$LOGDIR/$tag.log" before="" after="" pid="" waited=0
    local rc=0
    BOOTS=$((BOOTS + 1))
    "$PYTHON" "$HOSTPY" plan --verify "$verify" --write "$write" --flags "$flags" \
        --mode "$mode" --nonce "$nonce" --out "$plan" >>"$LOGDIR/$tag.plan.log" 2>&1 ||
        die "sinh kịch bản $tag hỏng"
    dd if="$plan" of="$DISK" bs=512 count=1 conv=notrunc >/dev/null 2>&1 ||
        die "ghi kịch bản vào LBA 0 hỏng"
    before="$(sha_of "$DISK")"
    # KT-78: process group riêng + timeout kill cả group (125 = không khởi động được).
    qg_boot "$log" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot \
        -drive "file=$DISK,format=raw,if=none,id=hd0" \
        -device virtio-blk-pci,drive=hd0
    if [ "$?" -ne 0 ]; then
        pid="$QG_LAST_PID"
        bad "$tag: quá $((TIMEOUT_DS / 10)) s (đã kill group pgid=$pid), log $log"
        ROWS="$ROWS$tag|TIMEOUT|$pid|-|-
"
        return 1
    fi
    pid="$QG_LAST_PID"
    waited="$QG_LAST_SECS" # giữ nguyên nghĩa cột `0.1s` và `secs=` của bảng (KT-75)
    after="$(sha_of "$DISK")"
    PIDS="$PIDS $pid"
    LAST_PID="$pid"
    LAST_BEFORE="$before"
    LAST_AFTER="$after"
    ROWS="$ROWS$tag|OK|$pid|$((waited))|$log
"
    echo "# boot: tag=$tag pid=$pid plan_verify=$verify plan_write=$write mode=$mode nonce=$nonce secs=$((waited / 10))"
    return 0
}

# `check_log <tag> <kind> <phase> <verify> <write> <mode> <nonce> [crash_after]`
check_log() {
    local tag="$1" kind="$2" phase="$3" verify="$4" write="$5" mode="$6" nonce="$7"
    local crash_after="${8:-0}"
    local out="$LOGDIR/$tag.check-$kind.log"
    if ! "$PYTHON" "$HOSTPY" check --log "$LOGDIR/$tag.log" --kind "$kind" --phase "$phase" \
        --verify "$verify" --write "$write" --mode "$mode" --nonce "$nonce" \
        --crash-after "$crash_after" >"$out" 2>&1; then
        sed -n '1,40p' "$out" >&2
        bad "$tag: log QEMU không khớp mô hình (kind=$kind phase=$phase)"
        return 1
    fi
    grep -E "^# log-check: " "$out" || true
    return 0
}

# `check_host <tag> <phase> <records>`
check_host() {
    local tag="$1" phase="$2" records="$3" out="$LOGDIR/$tag.host.log"
    if ! "$PYTHON" "$HOSTPY" host --disk "$DISK" --fsutil "$FSUTIL" \
        --phase "$phase" --records "$records" >"$out" 2>&1; then
        sed -n '1,40p' "$out" >&2
        bad "$tag: kiểm host ($phase, records=$records) ĐỎ"
        return 1
    fi
    grep -E "^# host(:| file:)" "$out" | sed -n '1,12p'
    OK_BOOTS=$((OK_BOOTS + 1))
    return 0
}

expect_sha_changed() {
    local tag="$1" want="$2"
    if [ "$want" = "changed" ]; then
        if [ "$LAST_BEFORE" = "$LAST_AFTER" ]; then
            bad "$tag: ảnh đĩa KHÔNG đổi sau boot ghi (dữ liệu không ra thiết bị?)"
            return 1
        fi
        echo "# sha: tag=$tag before=$LAST_BEFORE after=$LAST_AFTER changed=1"
    else
        if [ "$LAST_BEFORE" != "$LAST_AFTER" ]; then
            bad "$tag: ảnh đĩa ĐỔI ở ca âm (lẽ ra không có gì ra thiết bị)"
            return 1
        fi
        echo "# sha: tag=$tag sha=$LAST_AFTER changed=0 (ca âm: không có gì ra thiết bị)"
    fi
    return 0
}

run_sequence() {
    local p=0 nonce_base=1000 tag=""
    # b01: format + ghi P1
    tag=b01
    run_boot "$tag" 0 1 1 0 $((nonce_base + 1)) || return 1
    check_log "$tag" write 1 0 1 0 $((nonce_base + 1)) || true
    expect_sha_changed "$tag" changed || true
    check_host "$tag" 1 1 || true
    # b02..b07: đọc lại P(p-1) + ghi P(p)
    for p in 2 3 4 5 6 7; do
        tag="b0$p"
        run_boot "$tag" $((p - 1)) "$p" 0 0 $((nonce_base + p)) || return 1
        check_log "$tag" verify $((p - 1)) $((p - 1)) "$p" 0 $((nonce_base + p)) || true
        check_log "$tag" write "$p" $((p - 1)) "$p" 0 $((nonce_base + p)) || true
        expect_sha_changed "$tag" changed || true
        check_host "$tag" "$p" $((2 * p - 1)) || true
    done
    # b08: đọc lại P7 + tổng kết `phases=7 ok=7`
    tag=b08
    run_boot "$tag" 7 0 0 0 $((nonce_base + 8)) || return 1
    check_log "$tag" verify-last 7 7 0 0 $((nonce_base + 8)) || true
    expect_sha_changed "$tag" changed || true
    check_host "$tag" 7 14 || true
    # b09: ca âm — ghi KHÔNG flush (cache bay hơi)
    tag=b09
    run_boot "$tag" 0 0 0 1 $((nonce_base + 9)) || return 1
    check_log "$tag" neg-write 0 0 0 1 $((nonce_base + 9)) || true
    expect_sha_changed "$tag" same || true
    check_host "$tag" 7 14 || true
    # b10: ca âm — boot mới thấy `/pneg.txt` vắng
    tag=b10
    run_boot "$tag" 0 0 0 2 $((nonce_base + 10)) || return 1
    check_log "$tag" neg-verify 0 0 0 2 $((nonce_base + 10)) || true
    expect_sha_changed "$tag" same || true
    check_host "$tag" 7 14 || true
    return 0
}

# ---------------------------------------------------------------- KT-136: soak đĩa thật
#
# Agent 0 (phiếu lượt bốn, mục "Thứ tự", việc 2): "KT-136 soak đĩa thật". Nợ K6 số hai là: soak
# 1.000 vòng của KT-57 chạy trên **RAM**, không chạm đĩa thật; `persist.sh` chạm đĩa thật nhưng chỉ
# **10 boot**. Hàm dưới đây chạy **N vòng × 10 boot** trên **cùng một ảnh đĩa thật** (file trên
# host, virtio-blk), mỗi boot kiểm host độc lập (`fsutil fsck` + `ls` + `stat` + `get` so từng
# byte), rồi thêm **một** ca "cắt điện giữa lúc đang ghi".
#
# Bất biến của soak — mỗi vòng phải cho **cùng một** sha256 ảnh đĩa:
#   * Payload là **hàm của số pha**, không của nonce ⇒ hai vòng cùng dãy thao tác phải cho cùng
#     byte trên đĩa. Lệch ⇒ có **tích luỹ** giữa các vòng (block/inode không được trả lại, journal
#     không được dọn, bitmap rò) — đúng loại lỗi mà 10 boot không bắt được nhưng 100 boot bắt được.
#   * Đây là bất biến **chặt hơn** "chạy không lỗi": một FS rò tài nguyên vẫn có thể xanh 100 lần.
soak_run() {
    local cycles="$1" k=1 sha1="" sha="" top="$LOGDIR" cyc="" bad_cycles=0
    case "$cycles" in '' | *[!0-9]*)
        die "soak: số vòng phải là số nguyên dương"
        ;;
    esac
    [ "$cycles" -ge 1 ] || die "soak: số vòng phải >= 1"
    echo "# soak: cycles=$cycles boots_per_cycle=10 tổng_boot=$((cycles * 10))"
    while [ "$k" -le "$cycles" ]; do
        cyc="$top/cycle-$k"
        mkdir -p "$cyc" || die "không tạo được $cyc"
        LOGDIR="$cyc"
        DISK="$LOGDIR/kt75-disk.img"
        fresh_disk
        run_sequence || true
        sha="$(sha_of "$DISK")"
        if [ "$k" -eq 1 ]; then sha1="$sha"; fi
        echo "# soak: cycle=$k boots=$BOOTS sha=$sha"
        if [ "$sha" != "$sha1" ]; then
            bad "soak: cycle=$k sha ảnh đĩa KHÁC cycle=1 ($sha != $sha1) — có tích luỹ/rò giữa các vòng"
            bad_cycles=$((bad_cycles + 1))
        fi
        k=$((k + 1))
    done
    LOGDIR="$top"
    DISK="$LOGDIR/kt75-disk.img"
    echo "# soak: cycles=$cycles sha_all_equal=$((bad_cycles == 0)) bad_cycles=$bad_cycles"
    return 0
}

# `crash_run <cut>`: **cắt điện TẤT ĐỊNH** giữa lúc kernel đang ghi pha 7 (KT-136 → KT-408d/Q7).
#
# Cơ chế cũ (tới KT-408c) poll log mỗi `sleep 0.05` rồi `kill` cả process group ngay sau dòng
# `# persist: phase=6 action=verify`. Kết quả **phụ thuộc tải máy**: poll rơi sau khi pha 7 ghi
# xong ⇒ `/pstate` 13 bản ghi (`records=13` — nợ Q7); rơi quá sớm ⇒ chưa có `/p7.bin`
# (`partial=0`). Cả hai đều không chứng minh được gì về FS sau cắt điện.
#
# Cơ chế hiện tại (`D2-262` Q7): kernel tự cắt (`mode=3`, `crash_after=<cut>`) đúng sector thứ
# `cut` của `/p7.bin`, in `# persist: crash_cut …` rồi `halt(77)` **KHÔNG flush** cache. Runner
# **không poll, không kill**. Bất biến kiểm được (không phụ thuộc thời gian):
#   (1) log: đúng một dòng `crash_cut writes=<cut> pending=P` với `P >= 1` (cache còn block bẩn)
#       và **không** có dòng ghi pha 7 (cắt thật giữa chừng);
#   (2) đĩa: `/p7.bin` = **tiền tố** payload pha 7 với cỡ **đúng** `cut*512 B` (partial=1),
#       `/pstate` **đúng 12** bản ghi, fsck sạch, dữ liệu pha 6 còn nguyên từng byte;
#   (3) journal replay: boot lại trên **bản sao byte-identical** (trước mọi lệnh fsutil chạm bản
#       gốc) phải in `replayed=1` + đọc lại pha 6 xanh;
#   (4) dọn tệp dở (`create` là tạo độc quyền — KT-146 §A6) rồi phục hồi đủ b07r → b08r → b01r.
#
# `crash_run 0` = **đỏ-trước Q7**: kịch bản KHÔNG cắt (`mode=0`) ⇒ pha 7 ghi trọn 144/144 sector —
# đúng trạng thái mà race cũ sinh ra khi kill rơi sau pha 7. Bộ kiểm host **phải ĐỎ** với
# `records=13` + `partial=0` (chứng minh bộ kiểm không rỗng); trả 0 khi tái hiện đúng đỏ ấy.
crash_run() {
    local top="$LOGDIR"
    local d="$top/crash" tag="" rc=0
    local cut="${1:-0}" p7_blocks=144
    case "$cut" in '' | *[!0-9]*)
        die "crash: điểm cắt phải là số nguyên (nhận '$cut')"
        ;;
    esac
    if [ "$cut" -ne 0 ] && { [ "$cut" -lt 1 ] || [ "$cut" -ge "$p7_blocks" ]; }; then
        die "crash: điểm cắt phải trong 1..$((p7_blocks - 1)) sector (nhận $cut)"
    fi
    mkdir -p "$d" || die "không tạo được $d"
    LOGDIR="$d"
    DISK="$LOGDIR/kt75-disk.img"
    fresh_disk
    local p
    for p in 1 2 3 4 5 6; do
        tag="b0$p"
        if [ "$p" -eq 1 ]; then
            run_boot "$tag" 0 1 1 0 $((8000 + p)) || return 1
            check_log "$tag" write 1 0 1 0 $((8000 + p)) || true
            check_host "$tag" 1 1 || true
        else
            run_boot "$tag" $((p - 1)) "$p" 0 0 $((8000 + p)) || return 1
            check_log "$tag" verify $((p - 1)) $((p - 1)) "$p" 0 $((8000 + p)) || true
            check_log "$tag" write "$p" $((p - 1)) "$p" 0 $((8000 + p)) || true
            check_host "$tag" "$p" $((2 * p - 1)) || true
        fi
    done
    # ---- b07: boot cắt điện (mode=3, kernel tự halt(77)) hoặc đỏ-trước (mode=0, KHÔNG cắt)
    tag=b07
    local plan="$LOGDIR/$tag.plan" log="$LOGDIR/$tag.log" mode=3
    [ "$cut" -ne 0 ] || mode=0
    "$PYTHON" "$HOSTPY" plan --verify 6 --write 7 --flags 0 --mode "$mode" \
        --crash-after "$cut" --nonce 8007 --out "$plan" >>"$LOGDIR/$tag.plan.log" 2>&1 ||
        die "sinh kịch bản $tag hỏng"
    dd if="$plan" of="$DISK" bs=512 count=1 conv=notrunc >/dev/null 2>&1 ||
        die "ghi kịch bản vào LBA 0 hỏng"
    BOOTS=$((BOOTS + 1))
    # **Không poll, không `kill`**: chờ QEMU tự thoát vì kernel halt(77). Timeout giữ nguyên
    # `TIMEOUT_DS` như mọi boot khác — hết ngân sách ⇒ phép đo KHÔNG hợp lệ, không phải "đỏ".
    if ! qg_boot "$log" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on \
        -bios "$SBI" -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot \
        -drive "file=$DISK,format=raw,if=none,id=hd0" \
        -device virtio-blk-pci,drive=hd0; then
        bad "$tag: QEMU không thoát sạch (timeout/tín hiệu ngoài) — phép đo KHÔNG hợp lệ (log $log)"
        return 1
    fi
    echo "# crash: boot $tag tự halt(77) sau $cut/$p7_blocks sector — KHÔNG poll, KHÔNG kill (pid=$QG_LAST_PID secs=$((QG_LAST_SECS / 10)))"
    if [ "$cut" -eq 0 ]; then
        # ---- ĐỎ-TRƯỚC Q7: pha 7 ghi trọn ⇒ bộ kiểm host PHẢI bắt `records=13`/`partial=0`.
        if grep -q "^# persist: crash_cut " "$log" 2>/dev/null; then
            bad "crash-red: log có dòng crash_cut dù mode=0 — kernel cắt sai"
            rc=1
        fi
        if grep -q "^# persist: phase=7 action=write " "$log" 2>/dev/null; then
            echo "# crash-red: pha 7 ĐÃ GHI TRỌN 144/144 — đúng trạng thái 'kill trượt' của poll cũ"
        else
            bad "crash-red: pha 7 không ghi xong — không tái hiện được trạng thái cắt trượt"
            rc=1
        fi
        if cp "$DISK" "$LOGDIR/$tag.nocut.img" 2>/dev/null; then
            if "$PYTHON" "$HOSTPY" crash-host --disk "$LOGDIR/$tag.nocut.img" \
                --fsutil "$FSUTIL" --crash-after 72 >"$LOGDIR/$tag.crash-host.log" 2>&1; then
                bad "crash-red: kiểm host XANH trên ảnh pha 7 ghi trọn — bộ kiểm rỗng (Q7)"
                rc=1
            elif grep -q "records=13" "$LOGDIR/$tag.crash-host.log" &&
                grep -q "partial=0" "$LOGDIR/$tag.crash-host.log"; then
                grep -E "^FAIL|^# crash" "$LOGDIR/$tag.crash-host.log" | sed -n '1,12p' || true
                echo "# crash-red: PASS — host bắt đúng 'records=13' + 'partial=0' (đỏ ĐÚNG lý do Q7)"
            else
                sed -n '1,40p' "$LOGDIR/$tag.crash-host.log" >&2
                bad "crash-red: host ĐỎ nhưng KHÔNG đúng lý do records=13/partial=0"
                rc=1
            fi
        else
            bad "crash-red: không tạo được bản sao ảnh đĩa để kiểm"
            rc=1
        fi
        LOGDIR="$top"
        DISK="$LOGDIR/kt75-disk.img"
        return "$rc"
    fi
    # ---- (1) log của lần cắt: đúng một crash_cut, pending>=1, KHÔNG có ghi pha 7
    check_log "$tag" crash 0 6 7 3 8007 "$cut" || rc=1
    # ---- (2) ảnh đĩa: kiểm trên BẢN SAO ⇒ bản gốc giữ nguyên journal cho phép kiểm replay
    local crashimg="$LOGDIR/$tag.crash.img" replayimg="$LOGDIR/$tag.replay.img" savedisk="$DISK"
    if cp "$DISK" "$crashimg" 2>/dev/null; then
        if "$PYTHON" "$HOSTPY" crash-host --disk "$crashimg" --fsutil "$FSUTIL" \
            --crash-after "$cut" >"$LOGDIR/$tag.crash-host.log" 2>&1; then
            grep -E "^# crash" "$LOGDIR/$tag.crash-host.log" || true
            echo "# crash: ảnh đĩa sau cắt điện — fsck sạch, partial=1 (đúng $cut sector), dữ liệu pha 6 còn nguyên"
        else
            sed -n '1,40p' "$LOGDIR/$tag.crash-host.log" >&2
            bad "crash: kiểm ảnh đĩa sau cắt điện ĐỎ (xem $LOGDIR/$tag.crash-host.log)"
            rc=1
        fi
    else
        bad "crash: không tạo được bản sao ảnh đĩa để kiểm"
        rc=1
    fi
    # ---- (3) journal replay trên bản sao byte-identical, TRƯỚC mọi lệnh fsutil chạm bản gốc.
    # Kernel cắt trước `push_all` của transaction sector kế ⇒ trên đĩa còn commit record của
    # transaction cuối đã bền (chưa kịp ghi 0) ⇒ mount lại BẮT BUỘC replay đúng transaction đó.
    if cp "$DISK" "$replayimg" 2>/dev/null &&
        [ "$(sha_of "$DISK")" = "$(sha_of "$replayimg")" ]; then
        DISK="$replayimg"
        if run_boot "$tag-replay" 6 0 0 0 8017; then
            if grep -qE "^# persist: fs mounted=1 formatted=0 replayed=[1-9]" "$LOGDIR/$tag-replay.log"; then
                echo "# crash: journal replay trên bản sao — mount in replayed=1"
            else
                bad "crash: boot replay KHÔNG in 'replayed=1' (journal không được phát lại)"
                rc=1
            fi
            check_log "$tag-replay" verify 6 6 0 0 8017 || rc=1
        else
            bad "crash: boot replay không chạy"
            rc=1
        fi
        DISK="$savedisk"
    else
        bad "crash: không tạo được bản sao replay / bản sao lệch sha256"
        rc=1
    fi
    # ---- (4) dọn tệp dở (bắt buộc: partial=1 ⇒ `/p7.bin` phải CÓ MẶT)
    tag=b07r
    if "$FSUTIL" ls "$DISK" / 2>/dev/null | awk '{print $2}' | grep -qx "p7.bin"; then
        if "$FSUTIL" rm "$DISK" /p7.bin >"$LOGDIR/$tag.don-tep-do.log" 2>&1; then
            echo "# crash: dọn — đã xoá /p7.bin DỞ (create là tạo độc quyền; xoá rồi để kernel tạo lại)"
        else
            bad "crash: KHÔNG xoá được /p7.bin dở (xem $LOGDIR/$tag.don-tep-do.log)"
            rc=1
        fi
    else
        bad "crash: KHÔNG có /p7.bin dở để dọn — partial=1 không được chứng minh"
        rc=1
    fi

    # (2) Phục hồi: đọc lại pha 6 **và** ghi lại pha 7 đều phải xanh sau khi dọn.
    if run_boot "$tag" 6 7 0 0 8107; then
        # KHÔNG gọi `check_log` cho boot này: đây là boot **phục hồi** (đĩa đã có dữ liệu pha
        # 1..6) nên log không khớp mô hình đầy đủ của một boot sạch; kiểm từng dòng TAP bên dưới.
        if grep -qx "ok 1 - persist verify phase 6" "$LOGDIR/$tag.log"; then
            echo "# crash: boot sau cắt điện ĐỌC được dữ liệu đã commit (verify pha 6 xanh)"
        else
            bad "crash: boot sau cắt điện KHÔNG đọc được dữ liệu đã commit (verify pha 6 đỏ)"
            rc=1
        fi
        if grep -qx "ok 2 - persist write phase 7" "$LOGDIR/$tag.log"; then
            echo "# crash: boot sau cắt điện GHI LẠI được pha 7 sau khi dọn tệp dở (không kẹt)"
        else
            bad "crash: boot sau cắt điện KHÔNG ghi lại được pha 7 sau khi dọn tệp dở"
            rc=1
        fi
    else
        rc=1
    fi

    # (2b) Chạy tiếp tới **cuối** chuỗi pha — không chỉ qua pha 7.
    tag=b08r
    if run_boot "$tag" 7 0 0 0 8108; then
        if grep -qx "ok 1 - persist verify phase 7" "$LOGDIR/$tag.log"; then
            echo "# crash: boot sau cắt điện ĐỌC LẠI được pha 7 vừa ghi (verify pha 7 xanh)"
        else
            bad "crash: boot sau cắt điện KHÔNG đọc lại được pha 7 vừa ghi (verify pha 7 đỏ)"
            rc=1
        fi
        if grep -qE "^# persist: phases=7 ok=7 " "$LOGDIR/$tag.log"; then
            echo "# crash: chạy tiếp tới CUỐI — tổng kết phases=7 ok=7"
        else
            bad "crash: KHÔNG chạy tiếp được tới cuối (thiếu '# persist: phases=7 ok=7')"
            rc=1
        fi
        if grep -qE "^# persist: phases=7 ok=7 .*records=15 const:" "$LOGDIR/$tag.log"; then
            # 14 bản ghi của chuỗi chuẩn + **1** bản ghi verify pha 6 LẶP của lần boot bị cắt
            # điện (b07), rồi b07r đọc lại pha 6 lần nữa trước khi ghi pha 7. Con số này là bất
            # biến của chuỗi phục hồi, không phải của "chạy may mắn".
            echo "# crash: tổng kết phases=7 ok=7 + records=15 (14 chuẩn + 1 verify pha 6 của lần cắt)"
        else
            bad "crash: tổng kết phases/records sai (cần '# persist: phases=7 ok=7 … records=15')"
            rc=1
        fi
    else
        rc=1
    fi
    # (3) FS còn **ghi được** sau cắt điện: pha 1 tạo `/p1.txt` (đã bị rename đi ở pha 5).
    tag=b01r
    if run_boot "$tag" 0 1 0 0 8101; then
        if grep -qx "ok 1 - persist write phase 1" "$LOGDIR/$tag.log"; then
            echo "# crash: boot sau cắt điện vẫn GHI được (tạo /p1.txt mới, TAP xanh)"
        else
            bad "crash: boot sau cắt điện KHÔNG ghi được (tạo /p1.txt mới đỏ)"
            rc=1
        fi
    else
        rc=1
    fi
    # (4) fsck lần cuối.
    if "$FSUTIL" fsck "$DISK" >"$LOGDIR/final-fsck.log" 2>&1; then
        echo "# crash: fsck lần cuối sạch"
    else
        bad "crash: fsck lần cuối ĐỎ (xem $LOGDIR/final-fsck.log)"
        rc=1
    fi
    LOGDIR="$top"
    DISK="$LOGDIR/kt75-disk.img"
    return "$rc"
}

# `red_selftest`: chứng minh bộ kiểm **không rỗng** bằng một lần đỏ THẬT.
#
# (1) boot b01 ghi P1 xanh; (2) host `vifast-fsutil put-file` ghi đè `/p1.txt` bằng payload SAI;
# (3) boot đọc lại P1 **phải** đỏ (`match=0`, `not ok`) — cả QEMU lẫn host đều phải bắt.
red_selftest() {
    local tag=b01 wrong="$LOGDIR/wrong.bin" plan="$LOGDIR/red.log" rc=0
    fresh_disk
    run_boot "$tag" 0 1 1 0 7001 || die "red-selftest: boot ghi P1 hỏng"
    check_log "$tag" write 1 0 1 0 7001 || die "red-selftest: boot ghi P1 không khớp mô hình"
    check_host "$tag" 1 1 || die "red-selftest: kiểm host P1 (trước khi phá) ĐỎ"
    "$PYTHON" - "$wrong" <<'PY'
import sys
open(sys.argv[1], "wb").write(bytes([0xAA]) * 512)
PY
    "$FSUTIL" put-file "$DISK" /p1.txt "$wrong" >"$plan" 2>&1 ||
        die "red-selftest: fsutil put-file hỏng"
    echo "# red-selftest: host đã ghi đè /p1.txt bằng 512 B 0xAA (payload SAI)"
    run_boot b02 1 0 0 0 7002 || die "red-selftest: boot đọc lại P1 không chạy"
    if "$PYTHON" "$HOSTPY" check --log "$LOGDIR/b02.log" --kind verify --phase 1 \
        --verify 1 --write 0 --mode 0 --nonce 7002 >>"$plan" 2>&1; then
        bad "red-selftest: bộ kiểm QEMU KHÔNG bắt được payload sai (red mong đợi)"
        rc=1
    else
        echo "# red-selftest: QEMU báo đỏ đúng (xem $LOGDIR/b02.log)"
    fi
    if grep -q "^not ok 1 - persist verify phase 1" "$LOGDIR/b02.log"; then
        echo "# red-selftest: TAP QEMU 'not ok 1 - persist verify phase 1' có mặt"
    else
        bad "red-selftest: TAP QEMU không có 'not ok'"
        rc=1
    fi
    if "$PYTHON" "$HOSTPY" host --disk "$DISK" --fsutil "$FSUTIL" --phase 1 --records 1 \
        >>"$plan" 2>&1; then
        bad "red-selftest: kiểm host KHÔNG bắt được payload sai"
        rc=1
    else
        echo "# red-selftest: host cũng báo đỏ đúng (fsutil get khác mô hình)"
    fi
    if [ "$FAILS" -eq 0 ] && [ "$rc" -eq 0 ]; then
        echo "# red-selftest: PASS (QEMU + host đều phát hiện payload bị đổi trên đĩa)"
        return 0
    fi
    return 1
}

print_summary() {
    echo "# ================= BẢNG BOOT (test-persist) ================="
    printf '%s' "$ROWS" | while IFS='|' read -r t s p d l; do
        [ -n "${t:-}" ] || continue
        printf '# boot=%-4s kết quả=%-8s pid=%-6s 0.1s=%-5s log=%s\n' "$t" "$s" "$p" "$d" "$l"
    done
    echo "# ============================================================"
    echo "# pids:$PIDS (mỗi boot một tiến trình QEMU mới)"
    local distinct
    # D2-102: `pids` phải là số **PID phân biệt** — `sort -u` khử trùng, không đếm dòng.
    distinct="$(printf '%s\n' $PIDS | sed '/^$/d' | sort -u | wc -l | tr -d ' ')"
    if [ "${RUN_KIND:-run}" = "run" ]; then
        # `phase_reboots` phải là **giá trị tính** từ PID thật của cặp boot (ghi/đọc lại), không
        # được là hằng số trong dòng tổng kết (D2-102: dòng tổng kết cũng là evidence). Chỉ đếm
        # cặp mà **cả hai** boot đã chạy xong (`$2 == "OK"`).
        local pairs_ok
        pairs_ok="$(printf '%s' "$ROWS" | awk -F'|' '
            $2 == "OK" && $1 ~ /^b0[1-8]$/ { pid[$1] = $3 }
            END {
                n = 0
                for (phase = 1; phase <= 7; phase++) {
                    w = sprintf("b0%d", phase)
                    v = sprintf("b0%d", phase + 1)
                    if (pid[w] != "" && pid[v] != "" && pid[w] != pid[v]) n++
                }
                print n
            }')"
        echo "# reboot: boots=$BOOTS pids=$distinct phase_reboots=$pairs_ok (7 pha; mỗi pha ghi ở một tiến trình, đọc lại ở tiến trình KHÁC)"
        # Bảng **pha ↔ boot ↔ pid**: bằng chứng từng pha thật sự qua hai tiến trình QEMU khác nhau.
        printf '%s' "$ROWS" | awk -F'|' '
            $2 == "OK" && $1 ~ /^b0[1-8]$/ { pid[$1] = $3 }
            END {
                for (phase = 1; phase <= 7; phase++) {
                    w = sprintf("b0%d", phase)
                    v = sprintf("b0%d", phase + 1)
                    distinct = (pid[w] != "" && pid[v] != "" && pid[w] != pid[v]) ? 1 : 0
                    printf "# phase_reboot: phase=%d write_boot=%s write_pid=%s verify_boot=%s verify_pid=%s distinct_process=%d\n", phase, w, pid[w], v, pid[v], distinct
                }
            }'
    else
        echo "# reboot: boots=$BOOTS pids=$distinct (red-selftest/đỏ trước: không phải kết quả pha)"
    fi
}

main() {
    # Hai dòng `local` riêng: bash 3.2 đánh giá **mọi** vế phải trước khi tạo biến cục bộ, nên
    # `local mode="${1:-run}" arg2="${2:-}"` vẫn đọc `$2` từ phạm vi ngoài (rỗng) — đã đo được:
    # `--soak 2` chạy 10 vòng vì `arg2` rỗng.
    local mode="${1:-run}"
    local arg2="${2:-}"
    RUN_KIND=run
    [ "$mode" = "--red-selftest" ] && RUN_KIND=red
    mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"
    # KT-78/OD-K6-6: hạ tầng **trước** phép đo (wall persist gồm cả build).
    INFRA_LOG="$LOGDIR/kt78-infra.log"
    qg_infra before "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
    setup_tools
    build_everything
    case "$mode" in
        --red-selftest)
            echo "# chế độ: --red-selftest (chứng minh bộ kiểm bắt được đột biến)"
            red_selftest || FAILS=$((FAILS + 1))
            ;;
        --soak)
            # KT-136: soak đĩa thật. `--soak N` = N vòng × 10 boot = 10N boot trên ảnh đĩa thật.
            if ! rev_gate; then
                echo "# persist.sh: TỪ CHỐI ĐO (KT-86/D2-125.3) — không vòng nào chạy"
                exit 3
            fi
            soak_run "${arg2:-10}" || true
            ;;
        --crash)
            # KT-136 / KT-408d (Q7): cắt điện **tất định** giữa lúc đang ghi + phục hồi.
            if ! rev_gate; then
                echo "# persist.sh: TỪ CHỐI ĐO (KT-86/D2-125.3) — không boot nào chạy"
                exit 3
            fi
            RUN_KIND=crash
            crash_run "$CRASH_AFTER" || true
            ;;
        --crash-nocut)
            # KT-408d (Q7) — **đỏ-trước**: cùng chuỗi boot nhưng kịch bản KHÔNG cắt (mode=0).
            # Bộ kiểm host PHẢI đỏ `records=13`/`partial=0`; đây là chứng minh bộ kiểm không rỗng.
            if ! rev_gate; then
                echo "# persist.sh: TỪ CHỐI ĐO (KT-86/D2-125.3) — không boot nào chạy"
                exit 3
            fi
            RUN_KIND=crash
            crash_run 0 || true
            ;;
        run | '')
            # KT-86 (D2-125.3): cổng danh tính **trước** pha đầu tiên; đỏ ⇒ exit 3, không pha nào chạy.
            # (`--red-selftest` là ca kiểm của **bộ kiểm**, không phải phép đo hiệu năng/ngưỡng ⇒ không cổng.)
            if ! rev_gate; then
                echo "# persist.sh: TỪ CHỐI ĐO (KT-86/D2-125.3) — không pha nào chạy"
                exit 3
            fi
            fresh_disk
            run_sequence || true
            # D2-102: KHÔNG in bằng hằng. `phases`/`ok` đọc từ **log b08** (đã qua `check_log` —
            # kernel tự đếm từ nhật ký `/pstate` trên đĩa); `neg` lấy từ kết quả **thật** của hai
            # boot ca âm; `boots`/`host_checks` là bộ đếm của chính runner.
            summary_phases="$(sed -n 's/^# persist: phases=\([0-9][0-9]*\) .*/\1/p' "$LOGDIR/b08.log" 2>/dev/null | head -1)"
            summary_ok="$(sed -n 's/^# persist: phases=[0-9][0-9]* ok=\([0-9][0-9]*\) .*/\1/p' "$LOGDIR/b08.log" 2>/dev/null | head -1)"
            neg=0
            if grep -qx "ok 1 - persist noflush write" "$LOGDIR/b09.log" 2>/dev/null &&
                grep -qx "ok 1 - persist noflush verify" "$LOGDIR/b10.log" 2>/dev/null; then
                neg=1
            fi
            if [ -z "$summary_phases" ] || [ -z "$summary_ok" ]; then
                bad "không đọc được phases/ok từ log b08 — dòng tổng kết không được in bằng hằng"
            elif [ "$FAILS" -eq 0 ]; then
                echo "# persist: phases=$summary_phases ok=$summary_ok boots=$BOOTS neg=$neg host_checks=$OK_BOOTS"
            fi
            ;;
        *) die "tham số '$mode' không hiểu (dùng: --red-selftest | --soak N | --crash | --crash-nocut)" ;;
    esac
    # KT-78/OD-K6-6: hạ tầng **sau** phép đo; QEMU mồ côi/lạ ⇒ phép đo KHÔNG HỢP LỆ.
    qg_infra after "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
    echo "# infra: invalid=$INFRA_INVALID isolation_bad=$QG_ISO_BAD guard_log=$QG_LOG"
qg_guard_summary "$QG_LOG" >"$LOGDIR/guard-summary.log" 2>/dev/null || true
echo "# guard_summary_log=$LOGDIR/guard-summary.log"
    print_summary
    if [ "$FAILS" -ne 0 ]; then
        echo "# persist.sh: FAIL ($FAILS lỗi)"
        return 1
    fi
    # **F7 (audit hạ tầng 2026-11-28)** — hạ tầng KHÔNG HỢP LỆ ⇒ **mã 4** (T-INFRA, D2-116/
    # OD-K6-6) và **không** in PASS. Trước bản vá `INFRA_INVALID` chỉ `+=1` rồi `echo`.
    qg_infra_verdict "$INFRA_INVALID" persist.sh || return 4
    echo "# persist.sh: PASS"
    return 0
}

main "${1:-run}" "${2:-}"
