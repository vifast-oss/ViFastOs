#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-414b — build + chạy lặp lại được boot stub UEFI aarch64 dưới AAVMF (QEMU).
#
# Cách dùng:
#   bash run.sh --all-cases       # TẤT CẢ ca: xanh + 8 ca âm + chứng minh đỏ-trước (naive)
#   bash run.sh --positive        # chỉ ca xanh
#   bash run.sh --fault <tên>     # một ca âm (desc-size|desc-version|map-size|map-empty|
#                                 #              bad-type|bad-page|bad-attr|no-usable)
#   bash run.sh --naive-fault <tên>  # bản "trước khi vá" + tiêm lỗi ⇒ phải VẪN xanh (đỏ-trước)
#
# Đầu ra: `logs/<tag>/{serial.log,qemu.out,rc,vars.fd}` + rc của script.
# Log serial chứa cả ANSI của firmware; các dòng bằng chứng bắt đầu bằng `# uefi:` / `# bootinfo:`.
# Sau `ExitBootServices` ConOut của firmware đã chết ⇒ bằng chứng `after-ebs=ok` đi qua **ARM
# semihosting** (`SYS_WRITE0`) và QEMU ghi nó ra **stderr** ⇒ nằm trong `logs/<tag>/qemu.out`.
#
# Mã thoát QEMU (`SYS_EXIT_EXTENDED`, `hlt #0xF000`): rc = `(val<<1)|1` — **cùng thang** với
# `v2/boot/uefi-x86_64` (isa-debug-exit):
#   33 = 0x10 xanh (ExitBootServices OK) · 65 = 0x20 fail-closed bảng nhớ · 67 = 0x21 EBS lỗi.
#   rc=124 = watchdog của script (KHÔNG phải kết quả đo hợp lệ).
#
# Yêu cầu môi trường: có `qemu-system-aarch64` + AAVMF của Homebrew; KHÔNG cần mạng.
# Toolchain: **ghim `1.100.0`** (D2-260 mục 4 — KT-414b bỏ fallback `stable` của KT-414). Target
# `aarch64-unknown-uefi` phải đã cài cho toolchain đó (tiền đề hạ tầng của task); script **không**
# cài gì từ mạng và **fail-closed** nếu thiếu rust-std.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
QEMU="${QEMU:-qemu-system-aarch64}"
TRIPLE="aarch64-unknown-uefi"
CARGO_BIN="${CARGO_BIN:-cargo}"
TOOLCHAIN="${VIFAST_UEFI_TOOLCHAIN:-1.100.0}"
TIMEOUT="${VIFAST_UEFI_TIMEOUT:-180}"
CODE_FD="${VIFAST_UEFI_CODE_FD:-/opt/homebrew/share/qemu/edk2-aarch64-code.fd}"
VARS_FD="${VIFAST_UEFI_VARS_FD:-/opt/homebrew/share/qemu/edk2-arm-vars.fd}"
# Tôn trọng `CARGO_TARGET_DIR` của môi trường (máy đo dùng thư mục target dùng chung của slot);
# mặc định `target/` của crate (đã gitignore). **Không** để cargo rơi về `target/` mặc định khi
# biến đã đặt — nếu không, mỗi lần chạy lại build từ đầu và đầy đĩa.
TDIR_ROOT="${CARGO_TARGET_DIR:-$HERE/target}"
ESP="$TDIR_ROOT/esp"
LOGS="$HERE/logs"

# Tên lỗi tiêm → mã lỗi **có tên** mà app phải in (khớp `src/memmap.rs` của mô-đun chung).
# (bash 3.2 của macOS không có mảng kết hợp, nên dùng hàm `case`.)
fault_err() {
    case "$1" in
        desc-size) printf 'map-desc-size' ;;
        desc-version) printf 'map-desc-version' ;;
        map-size) printf 'map-size' ;;
        map-empty) printf 'map-empty' ;;
        bad-type) printf 'map-bad-type' ;;
        bad-page) printf 'map-bad-region' ;;
        bad-attr) printf 'map-bad-attribute' ;;
        # **D2-260** — bảng nhớ **hợp lệ theo UEFI** nhưng không còn vùng RAM usable nào:
        # `memmap::validate` xanh, `BootInfo::validate` (nguồn `Uefi`) fail-closed `missing-memory`.
        no-usable) printf 'missing-memory' ;;
        *) printf '' ;;
    esac
}

FAULTS="desc-size desc-version map-size map-empty bad-type bad-page bad-attr no-usable"

die() {
    echo "FAIL: $*" >&2
    exit 1
}

# Fail-closed nếu toolchain ghim **không** có rust-std cho target UEFI aarch64: đo bằng rc=124
# (watchdog) hay "build lỗi mạng" đều là kết quả **không hợp lệ**, nên từ chối đo ngay.
need_target() {
    local sysroot
    sysroot="$(rustc +"$TOOLCHAIN" --print sysroot 2>/dev/null)" ||
        die "không có toolchain '$TOOLCHAIN' — TỪ CHỐI ĐO (không PASS)"
    [ -d "$sysroot/lib/rustlib/$TRIPLE/lib" ] ||
        die "toolchain $TOOLCHAIN thiếu rust-std cho $TRIPLE — chạy 'rustup target add $TRIPLE --toolchain $TOOLCHAIN' trên máy đo (KHÔNG cài mạng trong task); TỪ CHỐI ĐO"
}

need_env() {
    command -v "$QEMU" >/dev/null 2>&1 || die "thiếu $QEMU"
    command -v "$CARGO_BIN" >/dev/null 2>&1 || die "thiếu $CARGO_BIN"
    [ -r "$CODE_FD" ] || die "thiếu firmware AAVMF code: $CODE_FD — TỪ CHỐI ĐO (không PASS)"
    [ -r "$VARS_FD" ] || die "thiếu firmware AAVMF vars: $VARS_FD — TỪ CHỐI ĐO (không PASS)"
    [ -n "$(fault_err "${1:-}")" ] || [ -z "${1:-}" ] || die "fault lạ: $1"
    need_target
}

# build <tag> <fault> <naive 0|1> — dựng ảnh .efi + dựng ESP (VVFAT) cho lần chạy kế tiếp.
build() {
    local tag="$1" fault="$2" naive="$3" tdir="$TDIR_ROOT/selftest/$tag"
    local log="$TDIR_ROOT/build-$tag.log"
    mkdir -p "$TDIR_ROOT" "$ESP/EFI/BOOT"
    echo "--- build $tag (fault='$fault' naive=$naive)"
    VIFAST_UEFI_FAULT="$fault" VIFAST_UEFI_NAIVE="$naive" \
        RUSTUP_TOOLCHAIN="$TOOLCHAIN" CARGO_TARGET_DIR="$tdir" \
        "$CARGO_BIN" build --release --target "$TRIPLE" --features uefi-image \
        --manifest-path "$HERE/Cargo.toml" >"$log" 2>&1 ||
        { tail -20 "$log" >&2; die "build $tag"; }
    local efi="$tdir/$TRIPLE/release/vifast-boot-aarch64.efi"
    [ -f "$efi" ] || die "build $tag: không thấy $efi"
    cp -f "$efi" "$ESP/EFI/BOOT/BOOTAA64.EFI"
    # AAVMF (edk2-stable202408) tự tạo boot option "UEFI Misc Device" cho đĩa virtio-blk có
    # `\EFI\BOOT\BOOTAA64.EFI` và nạp thẳng (đo được: `BdsDxe: starting Boot0001 "UEFI Misc
    # Device"`). `startup.nsh` là **dự phòng** cho firmware rơi vào UEFI Shell (khuôn OVMF của
    # KT-414) — Shell tự chạy nó ở FS0.
    printf '\\EFI\\BOOT\\BOOTAA64.EFI\r\n' >"$ESP/startup.nsh"
}

# run_case <tag> — chạy QEMU, ghi log, in rc (124 = watchdog: KHÔNG hợp lệ).
run_case() {
    local tag="$1" dir="$LOGS/$tag" pid w rc
    rm -rf "$dir"
    mkdir -p "$dir"
    cp -f "$VARS_FD" "$dir/vars.fd" # vars phải ghi được ⇒ bản sao mỗi lần chạy
    "$QEMU" \
        -machine virt -cpu cortex-a57 -m 256 -smp 1 \
        -display none -monitor none \
        -serial file:"$dir/serial.log" \
        -drive if=pflash,format=raw,readonly=on,file="$CODE_FD" \
        -drive if=pflash,format=raw,file="$dir/vars.fd" \
        -drive if=none,id=esp,file=fat:rw:"$ESP",format=raw \
        -device virtio-blk-pci,drive=esp \
        -net none -no-reboot \
        -semihosting-config enable=on,target=native \
        >"$dir/qemu.out" 2>&1 &
    pid=$!
    w=0
    while kill -0 "$pid" 2>/dev/null && [ "$w" -lt "$TIMEOUT" ]; do
        sleep 1
        w=$((w + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        rc=124
    else
        wait "$pid"
        rc=$?
    fi
    printf '%s' "$rc" >"$dir/rc"
    # `edk2-arm-vars.fd` là **64 MiB** mỗi bản sao (pflash của AAVMF phải đúng cỡ) ⇒ 12 ca là
    # ~768 MiB. Bản sao chỉ cần **trong lúc chạy** (BDS ghi boot option vào đó); sau khi có
    # `serial.log`/`qemu.out`/`rc` thì xoá để log không chiếm đĩa (máy đo còn ~11 GiB).
    rm -f "$dir/vars.fd"
    printf '%s' "$rc"
}

# grep trên log serial (có ANSI của firmware, nên dùng -a; không neo đầu dòng).
has() { grep -aqE -e "$2" "$LOGS/$1/serial.log" 2>/dev/null; }
has_not() { ! has "$1" "$2"; }
# Bằng chứng **sau** `ExitBootServices` nằm trên semihosting ⇒ stderr của QEMU (`qemu.out`).
semi_has() { grep -aqE -e "$2" "$LOGS/$1/qemu.out" 2>/dev/null; }

# expect <tag> <rc> <pattern...> — kiểm rc + các dòng bằng chứng bắt buộc; cộng điều kiện riêng.
expect() {
    local tag="$1" want="$2" pat rc ok=1
    shift 2
    rc="$(cat "$LOGS/$tag/rc" 2>/dev/null)"
    if [ "$rc" != "$want" ]; then
        echo "FAIL $tag: rc=$rc, muốn $want (xem logs/$tag/)" >&2
        ok=0
    fi
    for pat in "$@"; do
        if ! has "$tag" "$pat"; then
            echo "FAIL $tag: log thiếu /$pat/" >&2
            ok=0
        fi
    done
    [ "$ok" = 1 ]
}

# ---------------------------------------------------------------- ca

case_positive() {
    local tag=positive rc
    build "$tag" "" 0
    rc="$(run_case "$tag")"
    expect "$tag" 33 \
        '# uefi: app=vifast-boot-aarch64 v=1 target=aarch64-unknown-uefi' \
        '# uefi: memmap=ok count=[0-9]+ desc_size=[0-9]+ desc_version=1' \
        '# uefi: cfgtable n=[0-9]+ rsdp_pa=0x[0-9a-f]+ dtb_pa=0x[0-9a-f]+' \
        '# uefi: dtb pa=0x[0-9a-f]+ .*check=(ok|none)' \
        '# bootinfo: v=1 arch=aarch64 source=uefi n_harts=1 boot_hart=0' \
        '# bootinfo: ram n=[0-9]+ bytes=0x' \
        '# uefi: handoff .* arch=2 .* rsdp_pa=0x[0-9a-f]+ dtb_pa=0x[0-9a-f]+ ' \
        '# uefi: handoff verify=ok checksum=ok' \
        '# uefi: handoff reserved pa=0x[0-9a-f]+ bytes=352 pages=[0-9]+ invariant=kernel-must-reserve-or-copy' \
        '# uefi: exitbootservices=attempt' \
        || return 1
    semi_has "$tag" '# uefi: after-ebs=ok' ||
        { echo "FAIL $tag: semihosting (qemu.out) thiếu after-ebs=ok" >&2; return 1; }
    echo "PASS $tag (rc=$rc): memmap=ok, handoff verify=ok, ExitBootServices OK, after-ebs=ok (semihosting)"
}

case_fault() { # <fault> [naive]
    local fault="$1" naive="${2:-0}" tag rc err
    if [ "$naive" = 1 ]; then tag="naive-$fault"; else tag="neg-$fault"; fi
    err="$(fault_err "$fault")"
    [ -n "$err" ] || die "fault lạ: $fault"
    build "$tag" "$fault" "$naive"
    rc="$(run_case "$tag")"
    if [ "$naive" = 1 ]; then
        # ĐỎ-TRƯỚC: bản chưa vá (bỏ kiểm) **vẫn xanh** dù bảng nhớ hỏng ⇒ phép kiểm ca âm có
        # giá trị. Đây là bằng chứng đỏ của "trước khi có fix".
        expect "$tag" 33 '# uefi: memmap=ok count=[0-9]+ naive=1' '# uefi: exitbootservices=attempt' ||
            { echo "FAIL $tag: bản naive KHÔNG xanh ⇒ ca âm không chứng minh được gì" >&2; return 1; }
        has_not "$tag" 'fail-closed' || { echo "FAIL $tag: bản naive lại fail-closed" >&2; return 1; }
        semi_has "$tag" '# uefi: after-ebs=ok' ||
            { echo "FAIL $tag: naive thiếu after-ebs=ok (semihosting)" >&2; return 1; }
        echo "RED-PROOF $tag: bảng nhớ hỏng + bản CHƯA VÁ vẫn ExitBootServices (rc=$rc) ⇒ ca âm bắt được lỗi thật"
    else
        expect "$tag" 65 \
            "# uefi: fail-closed err=$err " \
            "# uefi: memmap=FAIL err=$err exitbootservices=not-called" \
            || return 1
        has_not "$tag" '# uefi: handoff verify=ok' || { echo "FAIL $tag: vẫn bàn giao khi bảng nhớ hỏng" >&2; return 1; }
        has_not "$tag" '# uefi: exitbootservices=attempt' || { echo "FAIL $tag: vẫn gọi ExitBootServices" >&2; return 1; }
        semi_has "$tag" '# uefi: fail-closed' || { echo "FAIL $tag: semihosting thiếu fail-closed" >&2; return 1; }
        echo "PASS $tag (rc=$rc): fail-closed err=$err, KHÔNG ExitBootServices"
    fi
}

run_all_cases() {
    local fail=0 f
    case_positive || fail=1
    for f in $FAULTS; do
        case_fault "$f" 0 || fail=1
    done
    # Chỉ cần MỘT ca đỏ-trước là đủ chứng minh bộ ca âm không rỗng; chạy 3 ca đại diện
    # (thêm `no-usable` — ca âm của `BootInfo::validate` theo nguồn `Uefi`, D2-260).
    case_fault desc-size 1 || fail=1
    case_fault bad-attr 1 || fail=1
    case_fault no-usable 1 || fail=1
    if [ "$fail" -ne 0 ]; then
        echo "SELFTEST: ĐỎ" >&2
        return 1
    fi
    echo "SELFTEST: XANH (1 xanh + 8 âm + 3 đỏ-trước)"
    return 0
}

main() {
    case "${1:-}" in
        -h | --help)
            sed -n '2,26p' "$0"
            return 0
            ;;
    esac
    need_env ""
    mkdir -p "$LOGS"
    case "${1:---all-cases}" in
        --all-cases)
            run_all_cases
            ;;
        --positive)
            case_positive
            ;;
        --fault)
            [ $# -eq 2 ] || die "--fault cần đúng một tên: $FAULTS"
            need_env "$2"
            case_fault "$2" 0
            ;;
        --naive-fault)
            [ $# -eq 2 ] || die "--naive-fault cần đúng một tên: $FAULTS"
            need_env "$2"
            case_fault "$2" 1
            ;;
        *)
            die "tham số lạ: $1 (xem --help)"
            ;;
    esac
}

main "$@"
