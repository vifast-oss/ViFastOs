#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0 OR MIT
# KT-156 — ĐO LẠI SÁU TIÊU CHÍ K6 TRÊN **ẢNH SẢN PHẨM CUỐI CÙNG** (`e7149448…`), bằng chứng để
# đóng K7.
#
# Chép từ `kt129-closure-measure.sh` (KT-129 đã đo đúng sáu tiêu chí này trên ảnh `a0287e4b…`):
# giữ nguyên **cấu trúc và các phép kiểm**, chỉ đổi `SHA`, `LOGDIR`, tên log, và **thêm** hai phần
# mà KT-129 lấy từ log cũ chứ không chạy lại trong script:
#   - F2: đọc lại tệp **sau reboot** trên **ảnh sản phẩm** (KT-77c: `kt77c-boot.sh`, cùng tệp đĩa);
#   - F3: build **lần hai** ảnh **tắt shell** (`--features product-fs`, không `product-shell`) rồi
#         chạy **kịch bản của F2** trên nó và đòi **ĐỎ** (OD-K6-2 tiêu chí 3).
#
# KHÔNG build ảnh sản phẩm ở đây: ảnh đã build bằng pipeline canonical và **ghim** vào
# `v2/artifacts/<sha256>.bin` (D2-122.6) — mọi phép đo trỏ vào tệp đã ghim, không trỏ `target/`.
# (Ảnh ca âm F3 thì phải build, vì nó là **ảnh khác**; nó cũng được ghim.)
#
# Thứ tự chạy theo **ưu tiên** của KT-156: F1 → F6 → F5 → F2 → F3 → F4 (soak cuối, tốn nhất).
# Dùng: bash v2/agent/JOURNAL/kt192-closure-measure.sh
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt192-logs"
mkdir -p "$LOGDIR"
SHA="${KT192_SHA:?dat KT192_SHA}"
FINAL="$ROOT/v2/artifacts/$SHA.bin"
[ -f "$FINAL" ] || {
    echo "kt192: không thấy ảnh $FINAL" >&2
    exit 2
}
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
. "$ROOT/v2/test/boot/build-rev.sh"

infra() { # <nhãn>
    echo "### infra $1 qemu_total=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ') loadavg=$(sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' | sed 's/^ *//;s/ *$//') ncpu=$(sysctl -n hw.ncpu 2>/dev/null)"
}

echo "### KT-156 CLOSURE-MEASURE $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### image=$SHA"
echo "### image_bytes=$(wc -c <"$FINAL" | tr -d ' ')"
echo "### head_now=$(git rev-parse HEAD)"
infra truoc

# ---------------------------------------------------------------- F1: shell> ở 1/2/4 hart
echo
echo "===== F1 BOOT anh san pham smp 1/2/4 $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
. "$ROOT/v2/test/boot/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/kt192-guard.log"
rm -f "$QG_LOG" 2>/dev/null
qg_init || {
    echo "qg_init FAIL"
    exit 2
}
for SMP in 1 2 4; do
    L="$LOGDIR/kt192-boot-smp$SMP.log"
    PAY="$LOGDIR/.pay-boot-$SMP"
    printf 'help\nexit\n' >"$PAY"
    KT77_SEND_DELAY=0.05 qg_boot "$L" 300 "$ROOT/v2/test/parity/stdin_wrap.sh" "$PAY" \
        "$(command -v qemu-system-riscv64)" \
        -machine virt,aia=none,aclint=on -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        -kernel "$FINAL" -nographic -cpu rv64 -m 128M -smp "$SMP" -no-reboot
    echo "### boot smp=$SMP qemu_rc=$? secs=$QG_LAST_SECS log=$L"
    rm -f "$PAY"
    grep -a -m1 -E '^# build:' "$L"
    grep -a -m1 -E '^# product shell:' "$L" | cut -c1-90
    grep -a -m1 -E '^# shell: up' "$L"
    grep -a -m1 -E '^# smp: boot=' "$L" | cut -c1-70
    echo "### smp=$SMP shell_prompt_lines=$(grep -a -c '^shell>' "$L")"
    vbrev_gate "$L" "$ROOT/v2" "$FINAL" || echo "### REV-GATE REFUSE smp=$SMP"
done

# ---------------------------------------------------------------- F6: quét harness (bằng máy)
echo
echo "===== F6 HARNESS SCAN tren anh san pham $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
NEGKT="$LOGDIR/ktest-proc-ca-am.bin"
if [ ! -f "$NEGKT" ]; then
    echo "# build ảnh ca âm ktest-proc (riscv64gc-unknown-none-elf, release, --features ktest,user)"
    cargo build --manifest-path "$ROOT/v2/kernel/Cargo.toml" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest-proc --features ktest,user \
        --target riscv64gc-unknown-none-elf --release >"$LOGDIR/kt192-build-negative.log" 2>&1
    echo "### build_negative_exit=$?"
    cp "$ROOT/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc" "$NEGKT"
fi
echo "### negative_image=$NEGKT bytes=$(wc -c <"$NEGKT" | tr -d ' ')"
FEAT_PRODUCT="product-shell,product-fs" FEAT_HARNESS="ktest,user" \
    bash v2/agent/JOURNAL/kt77b-harness-scan.sh --image "$FINAL" --negative "$NEGKT"
echo "### harness_scan_exit=$?"

# ---------------------------------------------------------------- F5: ngưỡng
echo
echo "===== F5 NGUONG tren anh san pham $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
echo "### image_bytes=$(wc -c <"$FINAL" | tr -d ' ') tran=262144"
bash v2/test/boot/budget.sh "$FINAL" product
echo "### budget_exit=$?"

# ---------------------------------------------------------------- F2: kịch bản shell + đọc lại sau reboot
echo
echo "===== F2 KICH BAN SHELL + DOC LAI SAU REBOOT $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
for SMP in 1 4; do
    bash v2/test/parity/run.sh --mode serial --image "$FINAL" --smp "$SMP" \
        --logdir "$LOGDIR/parity" --log-suffix -kt192
    echo "### parity smp=$SMP exit=$?"
    grep -a -E '^(ok|not ok) [0-9]+ - ' "$LOGDIR/parity/kt77a-serial-custom-smp$SMP-kt192.log" 2>/dev/null |
        grep -a -v 'k3 boot'
    grep -a -E '^# parity: (total|not_ok|KET_LUAN)' "$LOGDIR/parity/kt77a-serial-custom-smp$SMP-kt192.log" 2>/dev/null
done

# F2b: đọc lại tệp **sau reboot** trên **ảnh sản phẩm** (cùng tệp đĩa, hai tiến trình QEMU khác nhau)
echo "--- F2b doc lai sau reboot (anh san pham, cung anh dia) ---"
DISK="$LOGDIR/f2-disk.img"
rm -f "$DISK"
truncate -s 4M "$DISK" || echo "### f2_truncate_FAIL"
printf 'write /p.txt hello77\nwrite /p.txt hello77\ncat /p.txt\ncat /p.txt\nls\nls\nexit\n' \
    >"$LOGDIR/.f2-write.payload"
bash v2/agent/JOURNAL/kt77c-boot.sh "$FINAL" "$DISK" 1 "$LOGDIR/.f2-write.payload" \
    "$LOGDIR/kt192-f2-write-smp1.log" 300
echo "### f2_write_exit=$?"
SHA_W="$(shasum -a 256 "$DISK" | awk '{print $1}')"
printf 'ls\nls\ncat /p.txt\ncat /p.txt\nexit\n' >"$LOGDIR/.f2-read.payload"
bash v2/agent/JOURNAL/kt77c-boot.sh "$FINAL" "$DISK" 1 "$LOGDIR/.f2-read.payload" \
    "$LOGDIR/kt192-f2-reboot-read-smp1.log" 300
echo "### f2_reboot_read_exit=$?"
SHA_R="$(shasum -a 256 "$DISK" | awk '{print $1}')"
echo "### f2_disk_sha_sau_ghi=$SHA_W"
echo "### f2_disk_sha_sau_doc=$SHA_R"
[ "$SHA_W" = "$SHA_R" ] &&
    echo "### f2_dia_khong_doi_giua_hai_boot=1 (boot 2 chi doc)" ||
    echo "### f2_dia_khong_doi_giua_hai_boot=0"
echo "### f2_write_markers:"
grep -a -m3 -E '# shell: (write|cat|ls)' "$LOGDIR/kt192-f2-write-smp1.log"
echo "### f2_reboot_read_markers:"
grep -a -m3 -E '# shell: (cat|ls)' "$LOGDIR/kt192-f2-reboot-read-smp1.log"
echo "### f2_reboot_read_cat_ok=$(grep -a -c '# shell: cat ok bytes=7 first=hello77' "$LOGDIR/kt192-f2-reboot-read-smp1.log")"

# F2c: persist 7 pha (giữ nguyên phép kiểm của KT-129)
echo "--- F2c persist 7 pha ---"
KT75_LOGDIR="$LOGDIR/persist" bash v2/test/boot/persist.sh
echo "### persist_exit=$?"

# ---------------------------------------------------------------- F3: ca âm — ảnh TẮT shell
echo
echo "===== F3 CA AM anh TAT shell $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
# **Sửa so với đề bài KT-156** (đo được, xem `kt192-k7-do-lai.md` §F3): đề bài ghi build ca âm bằng
# `--features product-fs` (không `product-shell`), nhưng trong cây này
# `product-fs = ["product-shell", "vifast-plat-riscv64/block"]`
# (`v2/kernel/vifast-kbin-riscv64/Cargo.toml:29`) ⇒ `product-fs` **kéo theo** `product-shell` và
# build ra **đúng cùng sha** `e7149448…` (đo ở lần chạy đầu: `### noshell_sha=e7149448…` trùng
# `### image=e7149448…`). Ca âm đúng là build **không feature nào** — đúng comment của Cargo.toml:
# "Ca âm của tiêu chí 3 = build **không** feature này (`bash v2/tools/repro-build.sh` mặc định)
#  ⇒ runner parity phải đỏ `exit 5`".
NEGOUT="$(VIFAST_REPRO_LOG="$LOGDIR/kt192-build-noshell.log" \
    bash v2/tools/repro-build.sh 2>&1)"
echo "### build_noshell_exit=$?"
printf '%s\n' "$NEGOUT" | grep -E '^REPRO ' | tail -1
NEGSHA="$(printf '%s\n' "$NEGOUT" | sed -n 's/.* sha256=\([0-9a-f]*\) .*/\1/p' | tail -1)"
NEGIMG="$(printf '%s\n' "$NEGOUT" | sed -n 's/.* image=\(.*\)$/\1/p' | tail -1)"
echo "### noshell_sha=$NEGSHA"
bash v2/tools/pin-artifact.sh "$NEGIMG" 2>&1 | tail -2
NEGPIN="$ROOT/v2/artifacts/$NEGSHA.bin"
echo "### noshell_pinned=$NEGPIN"
# (a) ảnh tắt shell KHÔNG được in `shell>`
PAY="$LOGDIR/.f3-pay"
printf 'help\nexit\n' >"$PAY"
KT77_SEND_DELAY=0.05 qg_boot "$LOGDIR/kt192-f3-noshell-boot.log" 300 \
    "$ROOT/v2/test/parity/stdin_wrap.sh" "$PAY" \
    "$(command -v qemu-system-riscv64)" \
    -machine virt,aia=none,aclint=on -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel "$NEGPIN" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
echo "### f3_noshell_boot_exit=$?"
rm -f "$PAY"
echo "### f3_noshell_shell_prompt_lines=$(grep -a -c '^shell>' "$LOGDIR/kt192-f3-noshell-boot.log")"
grep -a -m1 -E '^# product shell:' "$LOGDIR/kt192-f3-noshell-boot.log" ||
    echo "### f3_khong_co_dong_product_shell=1"
grep -a -m1 -E '^# build:' "$LOGDIR/kt192-f3-noshell-boot.log"
# (b) kịch bản của F2 trên ảnh tắt shell phải **ĐỎ**
bash v2/test/parity/run.sh --mode serial --image "$NEGPIN" --smp 1 \
    --logdir "$LOGDIR/parity" --log-suffix -kt192-noshell
echo "### f3_parity_noshell_exit=$?"
grep -a -E '^# parity: (total|not_ok|KET_LUAN)' \
    "$LOGDIR/parity/kt77a-serial-custom-smp1-kt192-noshell.log" 2>/dev/null

# ---------------------------------------------------------------- F4: ma trận + soak
echo
echo "===== F4 MA TRAN ktest.sh 1 matrix (21 boot) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
KT54_LOGDIR="$LOGDIR/ktest-matrix" bash v2/test/boot/ktest.sh 1 matrix
echo "### matrix_exit=$?"

echo
echo "===== F4 SOAK 1000 3 tren anh da ghim $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
SOAK_LOGDIR="$LOGDIR/soak" bash v2/test/boot/soak.sh \
    --image "$FINAL" --features product-shell,product-fs 1000 3
echo "### soak_exit=$?"

echo
echo "### KT-156 XONG $(date -u +%Y-%m-%dT%H:%M:%SZ)"
infra sau
