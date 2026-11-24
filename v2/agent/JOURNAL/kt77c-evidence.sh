#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-77c — bằng chứng chạy được của "FS + virtio-blk trong ảnh sản phẩm" (D2-123.2).
#
# Chạy **một loạt** boot trên QEMU riscv64 bằng `kt77c-boot.sh` (đi qua `qemu_guard.sh`, D2-116):
#   1. đĩa mới ⇒ boot 1: format + `write` + `cat` (ghi trong boot này);
#   2. **cùng tệp đĩa** ⇒ boot 2: chỉ `ls` + `cat` (đọc lại sau reboot — phép đo chính của tiêu chí 1);
#   3. không đĩa (`-drive` vắng) ⇒ volume RAM `volatile=1`, `ls`/`cat` vẫn chạy;
#   4. đĩa **hỏng** (tệp rác) ⇒ shell vẫn lên, FS báo lý do, **không treo**;
#   5. 1/2/4 hart.
#
# Dùng: bash v2/agent/JOURNAL/kt77c-evidence.sh <image> [logdir]
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
BRUN="$HERE/kt77c-boot.sh"
IMAGE="${1:?cần <image>}"
LOGDIR="${2:-$HERE/kt77c-logs}"
WORK="${TMPDIR:-/tmp}/kt77c-evidence"
mkdir -p "$LOGDIR" "$WORK"

# Kịch bản lặp mỗi lệnh **hai lần**: `stdin_wrap.sh` bơm payload 8 lần nhưng shell `exit` ở bản
# sao đầu ⇒ nếu byte đầu bản sao 1 bị mất (nợ K7 mà KT-77b đã ghi) thì bản sao 2 vẫn đủ lệnh.
W=(
    'write /p.txt hello77
write /p.txt hello77
cat /p.txt
cat /p.txt
ls
ls
exit
'
    'ls
ls
cat /p.txt
cat /p.txt
cat /README
cat /README
exit
'
    'ls
ls
cat /README
cat /README
write /ram.txt vol1
write /ram.txt vol1
cat /ram.txt
cat /ram.txt
exit
'
)

run() { # <tên log> <disk|-> <smp> <payload>
    local name="$1" disk="$2" smp="$3" payload="$4" rc
    printf '%s' "$payload" >"$WORK/$name.payload"
    bash "$BRUN" "$IMAGE" "$disk" "$smp" "$WORK/$name.payload" "$LOGDIR/$name.log" 300
    rc=$?
    echo "# evidence: $name rc=$rc log=$LOGDIR/$name.log"
    return 0
}

echo "# evidence: image=$IMAGE sha256=$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
echo "# evidence: bat dau $(date -u +%Y-%m-%dT%H:%M:%SZ)"

# 1+2) đĩa mới → ghi → reboot cùng đĩa → đọc lại
DISK="$WORK/disk-persist.img"
rm -f "$DISK"
truncate -s 4M "$DISK" || exit 2
run kt77c-boot-write-smp1 "$DISK" 1 "${W[0]}"
DISK_SHA_AFTER_WRITE="$(shasum -a 256 "$DISK" | awk '{print $1}')"
echo "# evidence: disk_sha256_sau_boot1=$DISK_SHA_AFTER_WRITE"
run kt77c-boot-reboot-read-smp1 "$DISK" 1 "${W[1]}"
DISK_SHA_AFTER_READ="$(shasum -a 256 "$DISK" | awk '{print $1}')"
echo "# evidence: disk_sha256_sau_boot2=$DISK_SHA_AFTER_READ"
[ "$DISK_SHA_AFTER_WRITE" = "$DISK_SHA_AFTER_READ" ] &&
    echo "# evidence: dia_khong_doi_giua_hai_boot=1 (boot 2 chi doc)" ||
    echo "# evidence: dia_khong_doi_giua_hai_boot=0"

# 2b) đọc lại trên smp 2 và 4 (cùng đĩa — chứng minh hart phụ không phá FS)
run kt77c-boot-reboot-read-smp2 "$DISK" 2 "${W[1]}"
run kt77c-boot-reboot-read-smp4 "$DISK" 4 "${W[1]}"

# 3) không đĩa → volume RAM
run kt77c-boot-nodisk-smp1 - 1 "${W[2]}"

# 4) đĩa hỏng: tệp 4 MiB chứa rác (superblock không hợp lệ ⇒ phải format lại, không treo)
BROKEN="$WORK/disk-broken.img"
rm -f "$BROKEN"
head -c 4194304 /dev/urandom >"$BROKEN" 2>/dev/null || dd if=/dev/urandom of="$BROKEN" bs=65536 count=64 2>/dev/null
run kt77c-boot-disk-broken-smp1 "$BROKEN" 1 "${W[1]}"

# 4b) "không có đĩa" theo nghĩa tệp không tồn tại ⇒ QEMU từ chối gắn đĩa: bỏ qua gắn (dùng '-')
run kt77c-boot-missing-disk-smp1 - 1 "${W[1]}"

echo "# evidence: xong $(date -u +%Y-%m-%dT%H:%M:%SZ)"
# D2-116: đếm QEMU còn sót (toàn máy, chỉ đọc — không `pkill`)
echo "# evidence: qemu_con_sot_total=$(ps -ax -o pid,command 2>/dev/null | grep -c '[q]emu-system')"
