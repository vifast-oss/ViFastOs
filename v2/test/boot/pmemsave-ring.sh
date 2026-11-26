#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-408b** — bằng chứng `pmemsave` **đầu-cuối**: QEMU ghi RAM ra tệp, rồi kiểm vùng
# `.corespace` của ring trace 32 B (`RING`/`HEADS`/`COUNTS`) **trong chính tệp đó** (`D2-09`).
#
# Vì sao có tệp này
# ----------------
# `KT-408` (ring trace 32 B trong `.corespace` NOLOAD) chỉ có bằng chứng **byte-level ở test host**
# (`kt408_ring_trace_32_b`); nợ chuyển tiếp của nó là "chứng minh ring **có thật trong RAM ảnh**,
# đọc được bằng `pmemsave`". Khi chạy đầu-cuối lần đầu (`KT-408b`) hoá ra `RING` **không có trong
# ảnh**: LTO + DSE xoá `static` nội bộ vì bare-metal không ai đọc (`llvm-nm` chỉ thấy
# `HEADS`/`COUNTS`, `.corespace` = `Machine` + 128 B). Vá `#[used]` trong
# `v2/lib/vifast-sync/src/kernel_ring.rs` rồi runner này chứng minh lại từ đầu.
#
# Cách đo (không nới ngưỡng nào)
# -----------------------------
# 1. build `vifast-kbin-ktest` (`--features suite-base`, release, `lto = true` — **đúng** cấu hình
#    ảnh sản phẩm) rồi đọc `RING`/`HEADS`/`COUNTS` bằng `llvm-nm` của rustc hiện hành;
# 2. QEMU (`-machine virt,aia=none,aclint=on -bios OpenSBI -kernel ảnh`, `qemu_guard.sh` theo
#    `D2-116`) mở **monitor HMP** trên unix socket + serial ra tệp; kernel riscv64 **identity-map**
#    `0x8020_0000` (`plat-riscv64/src/mmu.rs`) ⇒ `PA == VA` của ký hiệu;
# 3. `pmemsave_ring.py` chờ **hai** mốc serial (`ok 42` = **trước** pha core, `ok 59` = **cuối**
#    pha core — nơi `Machine` chạy thật và hook trace ghi), mỗi mốc gửi
#    `pmemsave <pa> <n> <tệp>` cho `RING`/`HEADS`/`COUNTS` ⇒ QEMU **tự ghi** RAM ra tệp;
# 4. kiểm tệp: vùng `[__corespace_start, __corespace_end)` chứa cả ba ký hiệu; `count =
#    min(head, PER_HART)`; `head` **tăng** giữa hai ảnh chụp (ring sống); bản ghi 32 B giải mã
#    được bằng `v2/tools/vifast-trace.py` (đúng `spec/abi.toml [trace]`);
# 5. lưu **log + tệp mẫu** vào `v2/test/trace/` (bằng chứng thường trực, không phải `/tmp`).
#
# Dùng:
#   bash v2/test/boot/pmemsave-ring.sh [OUT_DIR]     # mặc định OUT_DIR = v2/test/trace
# Exit: 0 = PASS (đủ 5 mục trên); 1 = ĐỎ; 3 = thiếu hạ tầng (không có nm/qemu/OpenSBI/python).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO="$(cd "$V2_ROOT/.." && pwd)"
OUT="${1:-$V2_ROOT/test/trace}"
TARGET="riscv64gc-unknown-none-elf"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
IMG_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release"
IMAGE="$IMG_DIR/vifast-kbin-ktest"
TIMEOUT_DS=200 # 20 s/boot — cùng ngân sách `ktest.sh` (D2-72 Q10), KHÔNG nới
RING_BYTES=8192 # 8 hart × 32 bản ghi × 32 B (`kernel_ring::CAP * record_bytes`)
PER_HART=32
BLOB_BYTES=$((RING_BYTES + 128)) # RING | HEADS (8×u64) | COUNTS (8×u64) — một `pmemsave` nguyên tử

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() { echo "pmemsave-ring.sh: $*" >&2; exit 1; }
sha256_of() { shasum -a 256 "$1" | cut -d' ' -f1; }
refuse() { echo "pmemsave-ring.sh: TỪ CHỐI ĐO — $*" >&2; exit 3; }

mkdir -p "$OUT" || die "không tạo được $OUT"
LOG="$OUT/pmemsave-riscv64.log"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/kt408b.XXXXXX")" || die "không tạo được thư mục tạm"
SERIAL="$SCRATCH/serial.log"
SOCK="$SCRATCH/mon.sock" # tên ngắn: unix socket macOS giới hạn 104 byte
QLOG="$SCRATCH/qemu.log"
PY_PID=""
trap 'if [ -n "$PY_PID" ]; then kill "$PY_PID" 2>/dev/null || true; fi' EXIT INT TERM HUP

# Ghi cả ra log thường trực (bằng chứng) lẫn stdout.
: >"$LOG"
exec > >(tee -a "$LOG") 2>&1
echo "# SPDX-License-Identifier: MIT OR Apache-2.0"
echo "# KT-408b — pmemsave đầu-cuối: ring trace 32 B trong .corespace (D2-09/D2-252)"
echo "# ngày $(date -u '+%Y-%m-%dT%H:%M:%SZ') UTC · máy $(uname -sm) · host $(hostname -s 2>/dev/null || echo '?')"
echo "# worktree $(pwd) · commit $(git -C "$REPO" rev-parse HEAD 2>/dev/null || echo '?')"
echo "# logdir $OUT · scratch $SCRATCH"

command -v cargo >/dev/null 2>&1 || refuse "không có cargo"
command -v python3 >/dev/null 2>&1 || refuse "không có python3"
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || refuse "không có qemu-system-riscv64"
SYSROOT="$(rustc --print sysroot 2>/dev/null)" || refuse "không đọc được rustc sysroot"
NM="$(ls "$SYSROOT"/lib/rustlib/*/bin/llvm-nm 2>/dev/null | head -1)"
[ -x "${NM:-/nonexistent}" ] || refuse "không có llvm-nm trong sysroot rustc"
SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && { SBI="$c"; break; }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || refuse "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"
echo "# qemu $("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p') · opensbi $SBI"
echo "# toolchain $(rustc -V)"
echo "# target $TARGET · features suite-base · profile release (lto = true)"

# ---------------------------------------------------------------- 1. build ảnh
echo "# build ảnh test base ($TARGET, release, --features suite-base)"
BUILD_LOG="$SCRATCH/build.log"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --bin vifast-kbin-ktest \
    --features suite-base --target "$TARGET" --release >"$BUILD_LOG" 2>&1; then
    tail -20 "$BUILD_LOG" >&2
    die "build ảnh FAIL ($BUILD_LOG)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
BUILD_ID="$(grep -m1 -o 'KT-89 build identity .*' "$BUILD_LOG" || true)"
[ -n "$BUILD_ID" ] || die "không đọc được dòng danh tính bản dựng (# build:) trong $BUILD_LOG"
echo "# build: $BUILD_ID"
echo "# image $(sha256_of "$IMAGE") ($(wc -c <"$IMAGE" | tr -d ' ') B)"

# ---------------------------------------------------------------- 2. ký hiệu
SYMS="$SCRATCH/syms.txt"
"$NM" --defined-only "$IMAGE" >"$SYMS" 2>/dev/null || die "llvm-nm FAIL"
sym() { awk -v n="$1" '$3 ~ n {print $1}' "$SYMS" | head -1; }
RING_PA="$(sym 'kernel_ring4RING$')"
HEADS_PA="$(sym 'kernel_ring5HEADS$')"
COUNTS_PA="$(sym 'kernel_ring6COUNTS$')"
CS_START="$(sym '__corespace_start$')"
CS_END="$(sym '__corespace_end$')"
for v in RING_PA HEADS_PA COUNTS_PA CS_START CS_END; do
    [ -n "${!v}" ] || die "thiếu ký hiệu $v trong ảnh — ring KHÔNG có trong RAM (xem $SYMS)"
done
hex2dec() { printf '%d' "0x$1"; }
RING_D="$(hex2dec "$RING_PA")"
HEADS_D="$(hex2dec "$HEADS_PA")"
COUNTS_D="$(hex2dec "$COUNTS_PA")"
CS_START_D="$(hex2dec "$CS_START")"
CS_END_D="$(hex2dec "$CS_END")"
echo "# sym: RING=0x$RING_PA HEADS=0x$HEADS_PA COUNTS=0x$COUNTS_PA"
echo "# sym: .corespace=[0x$CS_START, 0x$CS_END) = $((CS_END_D - CS_START_D)) B"
in_cs() {
    local a="$1" n="$2" name="$3"
    if [ "$a" -lt "$CS_START_D" ] || [ $((a + n)) -gt "$CS_END_D" ]; then
        die "$name (0x$(printf '%x' "$a") + $n B) KHÔNG nằm trong .corespace — sai section (D2-252)"
    fi
}
in_cs "$RING_D" "$RING_BYTES" RING
in_cs "$HEADS_D" 64 HEADS
in_cs "$COUNTS_D" 64 COUNTS
[ $((HEADS_D + 64)) -eq "$COUNTS_D" ] || die "HEADS/COUNTS không liền nhau (0x$HEADS_PA/0x$COUNTS_PA) — sửa runner"
echo "# sym: RING ⊂ HEADS ⊂ COUNTS đều trong .corespace ⇒ ring ở đúng arena NOLOAD"

# ---------------------------------------------------------------- 3. boot + pmemsave
. "$HERE/qemu_guard.sh"
QG_LOG="$SCRATCH/qemu-guard.log"
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

echo "# boot QEMU (+monitor HMP) — pmemsave vùng liền nhau RING|HEADS|COUNTS ($BLOB_BYTES B, VM stop/cont)"
echo "# tại hai mốc serial: 'ok 42' (trước pha core) và 'ok 59' (cuối pha core)"
echo "# cmd: $QEMU -machine virt,aia=none,aclint=on -bios $SBI -kernel $IMAGE -display none"
echo "#      -serial file:$SERIAL -monitor unix:$SOCK,server=on,wait=off -no-reboot"
echo "#      -cpu rv64 -m 128M -smp 1 -icount shift=6"
python3 "$HERE/pmemsave_ring.py" \
    --sock "$SOCK" --serial "$SERIAL" --out "$SCRATCH" \
    --blob "$RING_D:$BLOB_BYTES" --ring-size "$RING_BYTES" \
    --heads-size 64 --counts-size 64 \
    --marker '^ok 42 ' --tag a --marker '^ok 59 ' --tag b --timeout 20 &
PY_PID=$!

qg_boot "$QLOG" "$TIMEOUT_DS" "$QEMU" \
    -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" \
    -display none -serial "file:$SERIAL" -monitor "unix:$SOCK,server=on,wait=off" \
    -no-reboot -cpu rv64 -m 128M -smp 1 -icount shift=6
BOOT_RC=$?
echo "# boot rc=$BOOT_RC (124 = hết ngân sách $((TIMEOUT_DS / 10)) s — ảnh test kết thúc bằng PANIC có chủ đích)"
case "$BOOT_RC" in
0 | 124) ;;
*) die "boot rc=$BOOT_RC không hợp lệ (125/126 = hạ tầng) — xem $QLOG" ;;
esac

if ! wait "$PY_PID"; then
    PY_PID=""
    die "pmemsave_ring.py FAIL — xem log phía trên (serial: $SERIAL)"
fi
PY_PID=""
echo "# serial: $(wc -l <"$SERIAL" | tr -d ' ') dòng · $(grep -c '^ok ' "$SERIAL") dòng ok · $(grep -c '^not ok' "$SERIAL") dòng not ok"

# ---------------------------------------------------------------- 4. kiểm tệp dump
echo "# kiểm tệp pmemsave: count = min(head, $PER_HART), head tăng giữa hai ảnh chụp, bản ghi 32 B"
python3 - "$SCRATCH" "$RING_BYTES" "$PER_HART" <<'PY' || die "kiểm cấu trúc ring trong tệp pmemsave FAIL"
import json
import struct
import sys

scratch, ring_bytes, per_hart = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
meta = {}
for tag in ("a", "b"):
    with open("%s/meta-%s.json" % (scratch, tag), encoding="utf-8") as fh:
        meta[tag] = json.load(fh)
    ring = open("%s/ring-%s.bin" % (scratch, tag), "rb").read()
    assert len(ring) == ring_bytes, "ring-%s.bin %d B != %d" % (tag, len(ring), ring_bytes)
    heads, counts = meta[tag]["heads"]["values"], meta[tag]["counts"]["values"]
    assert len(heads) == 8 and len(counts) == 8, "HEADS/COUNTS phải 8 hart"
    n, head = counts[0], heads[0]
    assert n == min(head, per_hart), "COUNTS[0]=%d != min(HEADS[0]=%d, %d)" % (n, head, per_hart)
    assert all(c == 0 for c in counts[1:]), "smp=1 mà hart khác có bản ghi: %r" % (counts,)
    meta[tag]["n"] = n
    meta[tag]["head"] = head
    last = "-" if n == 0 else "%d B" % ((n - 1) * 32)
    print("# dump-%s: head=%d count=%d kept=%d B · bản ghi cuối @%s" % (tag, head, n, n * 32, last))

assert meta["b"]["head"] > meta["a"]["head"], (
    "HEADS[0] không tăng (a=%d, b=%d) — ring không sống giữa hai ảnh chụp"
    % (meta["a"]["head"], meta["b"]["head"])
)
print("# liveness: head %d → %d (+%d bản ghi giữa hai mốc)"
      % (meta["a"]["head"], meta["b"]["head"], meta["b"]["head"] - meta["a"]["head"]))

# Bản ghi 32 B: t_ns:u64 hart:u16 kind:u16 tcb:u32 a:u64 b:u64 (little-endian)
ring = open("%s/ring-b.bin" % scratch, "rb").read()
n = meta["b"]["n"]
assert n > 0, "COUNTS[0]=0 ở ảnh chụp cuối — hook trace không ghi bản ghi nào (D2-09)"
prev = -1
first = -1
kinds = set()
for i in range(n):
    t_ns, hart, kind, tcb, a, b = struct.unpack_from("<QHHIQQ", ring, i * 32)
    assert hart == 0, "bản ghi %d: hart=%d (smp=1)" % (i, hart)
    assert 1 <= kind <= 7, "bản ghi %d: kind=%d ngoài 1..7" % (i, kind)
    assert t_ns >= prev, "bản ghi %d: t_ns lùi (%d < %d)" % (i, t_ns, prev)
    if i == 0:
        first = t_ns
    prev = t_ns
    kinds.add(kind)
# **KT-408c** — in **đúng** mốc đầu (trước đây hằng `0`, che mất đúng thứ cần chứng minh) và đòi
# `t_ns` **tăng thật** giữa hai bản ghi: `KT-408b` §6.4 đo được `t_ns=[0..0]` (mọi bản ghi cùng
# mốc) vì `Machine::now` không được cấp tick. Đỏ-trước: trước bản vá, dòng `assert` dưới ĐỎ.
print("# records: n=%d hart=0 kinds=%s t_ns=[%d..%d]" % (n, sorted(kinds), first, prev))
assert n < 2 or prev > first, (
    "t_ns KHÔNG tăng (mọi bản ghi cùng mốc %d) — `Machine::now` không được cấp tick (KT-408c)"
    % first
)

# Thân giải mã cho `vifast-trace.py` = đúng n*32 B của ảnh chụp b.
with open("%s/records-b.bin" % scratch, "wb") as fh:
    fh.write(ring[: n * 32])
print("# ghi %s/records-b.bin (%d B)" % (scratch, n * 32))
PY

echo "# giải mã bằng công cụ chính thức: python3 v2/tools/vifast-trace.py <records-b.bin>"
if ! python3 "$V2_ROOT/tools/vifast-trace.py" "$SCRATCH/records-b.bin" >"$SCRATCH/decoded.txt" 2>&1; then
    cat "$SCRATCH/decoded.txt"
    die "vifast-trace.py KHÔNG giải mã được tệp pmemsave (định dạng 32 B lệch?)"
fi
head -6 "$SCRATCH/decoded.txt"
echo "# … ($(wc -l <"$SCRATCH/decoded.txt" | tr -d ' ') dòng giải mã)"

# ---------------------------------------------------------------- 5. lưu bằng chứng
SAMPLE_RING="$OUT/pmemsave-riscv64-ring.bin"
SAMPLE_HC="$OUT/pmemsave-riscv64-heads-counts.bin"
cp "$SCRATCH/ring-b.bin" "$SAMPLE_RING" || die "không lưu được $SAMPLE_RING"
cat "$SCRATCH/heads-b.bin" "$SCRATCH/counts-b.bin" >"$SAMPLE_HC" || die "không lưu được $SAMPLE_HC"
printf '# SPDX-License-Identifier: MIT OR Apache-2.0\n' >"$SAMPLE_RING.license"
printf '# SPDX-License-Identifier: MIT OR Apache-2.0\n' >"$SAMPLE_HC.license"
echo "# lưu mẫu: $SAMPLE_RING ($(wc -c <"$SAMPLE_RING" | tr -d ' ') B) sha256=$(sha256_of "$SAMPLE_RING")"
echo "# lưu mẫu: $SAMPLE_HC ($(wc -c <"$SAMPLE_HC" | tr -d ' ') B: 8×u64 HEADS rồi 8×u64 COUNTS)"
echo "# — bố cục bản ghi: t_ns:u64 | hart:u16 kind:u16 tcb:u32 | a:u64 | b:u64 (32 B, LE, spec/abi.toml [trace])"
HEAD_B="$(python3 - "$SCRATCH/meta-b.json" <<'PY'
import json
import sys

print(json.load(open(sys.argv[1]))["heads"]["values"][0])
PY
)"
N_B="$(python3 - "$SCRATCH/meta-b.json" "$PER_HART" <<'PY'
import json
import sys

print(min(json.load(open(sys.argv[1]))["heads"]["values"][0], int(sys.argv[2])))
PY
)"
echo "# == pmemsave-ring: PASS (RING/HEADS/COUNTS trong .corespace; 2 ảnh chụp head 0→$HEAD_B bản ghi;"
echo "#    $N_B bản ghi 32 B giải mã bằng vifast-trace.py) =="
exit 0
