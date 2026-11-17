#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-88 (D2-123.3) — BƯỚC 5: ĐỌC LẠI TỆP SAU REBOOT, trên **cùng một tệp đĩa**, hai boot QEMU
# riêng biệt, ảnh sản phẩm CUỐI (canonical, `product-shell,product-fs`) boot từ artifacts/<sha>.bin:
#   boot 1: `write /p.txt hello77` rồi `exit`  → format + ghi + flush + halt sạch
#   boot 2: `cat /p.txt` rồi `exit`            → đọc lại từ đĩa, KHÔNG ghi gì (`write=0`)
# Chứng minh: (i) boot 2 in `# shell: cat ok bytes=7 first=hello77`;
#             (ii) sha256 **tệp đĩa** không đổi giữa hai boot;
#             (iii) log boot 2 có `# product fs: ... write=0`.
# Đo bằng qemu_guard.sh (KT-78, không sửa) + stdin_wrap.sh (KT-77a) — chỉ giết PID của mình.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt88-logs"
RDIR="$LOGDIR/persist-2boot"
V2_ROOT="$ROOT/v2"
mkdir -p "$RDIR"
SHA=38714b799fd27964785eba9aec2d8f591063d694f8a7bb9d9672f3abbd9decb2
IMG="$ROOT/v2/artifacts/$SHA.bin"
DISK="$RDIR/kt88-persist.img"
GUARD="$ROOT/v2/test/boot/qemu_guard.sh"
WRAP="$ROOT/v2/test/parity/stdin_wrap.sh"
B1="$RDIR/boot1.log"
B2="$RDIR/boot2.log"
SBI=/opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin
QEMU="$(command -v qemu-system-riscv64)"
[ -x "$IMG" ] || { echo "khong thay anh $IMG"; exit 2; }
[ -f "$SBI" ] || { echo "khong thay opensbi"; exit 2; }
[ -x "$WRAP" ] || { echo "khong thay stdin_wrap.sh"; exit 2; }
. "$GUARD"
. "$ROOT/v2/test/boot/build-rev.sh"
qg_init || { echo "qg_init FAIL"; exit 2; }

echo "### KT-88 buoc 5 (doc lai sau reboot) $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### HEAD=$(git rev-parse HEAD)"
echo "### image_sha256=$(shasum -a 256 "$IMG" | awk '{print $1}') image_bytes=$(wc -c <"$IMG" | tr -d ' ')"
echo "### image_build_line=$(LC_ALL=C grep -a -o -m1 '# build: rev=[0-9a-f]* dirty=[01]' "$IMG")"
echo "### disk=$DISK (moi: 1024 block x 4096 B, toan so 0)"
echo "### qemu_con_sot_truoc=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"

rm -f "$DISK" "$B1" "$B2"
dd if=/dev/zero of="$DISK" bs=4096 count=1024 >/dev/null 2>&1 || { echo "dd FAIL"; exit 2; }
echo "### disk_shasum_luc_dau=$(shasum -a 256 "$DISK" | awk '{print $1}')"

QARGS=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMG" -nographic \
    -cpu rv64 -m 128M -smp 1 -no-reboot \
    -drive "file=$DISK,format=raw,if=none,id=hd0" -device virtio-blk-pci,drive=hd0)

send_boot() { # $1=log $2=payload
    local log="$1" payload="$2" pay="$RDIR/.payload.$$"
    printf '%b' "$payload" >"$pay"
    KT77_SEND_DELAY=0.05 qg_boot "$log" 300 "$WRAP" "$pay" "$QEMU" "${QARGS[@]}"
    local rc=$?
    rm -f "$pay"
    return "$rc"
}

echo
echo "===== BOOT 1: 'write /p.txt hello77' + exit $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
send_boot "$B1" 'write /p.txt hello77\nexit\n'
RC1=$?
echo "### boot1_qemu_rc=$RC1 pid=$QG_LAST_PID secs=$QG_LAST_SECS log=$B1"
SHA1="$(shasum -a 256 "$DISK" | awk '{print $1}')"
echo "### disk_sha256_sau_boot1=$SHA1"
echo "----- boot1 markers:"
grep -a -E '^# (build:|product fs:|product halt:|shell: write|shell: up)' "$B1" | head -8
echo "----- vbrev_gate:"
vbrev_gate "$B1" "$V2_ROOT" "$IMG" || echo "----- REV-GATE REFUSE"

echo
echo "===== BOOT 2: 'cat /p.txt' + exit (cung tep dia) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
send_boot "$B2" 'cat /p.txt\nexit\n'
RC2=$?
echo "### boot2_qemu_rc=$RC2 pid=$QG_LAST_PID secs=$QG_LAST_SECS log=$B2"
SHA2="$(shasum -a 256 "$DISK" | awk '{print $1}')"
echo "### disk_sha256_sau_boot2=$SHA2"
echo "----- boot2 markers:"
grep -a -E '^# (build:|product fs:|product halt:|shell: cat|shell: up)' "$B2" | head -8
echo "----- vbrev_gate:"
vbrev_gate "$B2" "$V2_ROOT" "$IMG" || echo "----- REV-GATE REFUSE"

echo
echo "===== KET LUAN buoc 5 ====="
echo "### disk_sha_khong_doi=$([ "$SHA1" = "$SHA2" ] && echo 1 || echo 0)"
grep -a -q '^# shell: cat ok bytes=7 first=hello77$' "$B2" && echo "### boot2_doc_lai_hello77=1" || echo "### boot2_doc_lai_hello77=0"
W2="$(grep -a -o -m1 '# product fs: medium=[a-z]* .*write=[0-9]*' "$B2" | sed -n 's/.*write=\([0-9]*\).*/\1/p')"
echo "### boot2_write_counter=${W2:-<khong-thay>}"
echo "### qemu_con_sot_sau=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
echo "### ket thuc $(date -u +%Y-%m-%dT%H:%M:%SZ)"
