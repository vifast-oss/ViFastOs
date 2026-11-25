#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-57 — soak boot ảnh **SẢN PHẨM** riscv64 K3 (cổng K3 §5.2 điều kiện 3).
#
# Nguồn ràng buộc:
#   - D2-24 + QD §5.3: máy `ps`, **N = 1.000**, **3 luồng song song**, `n ≥ 3/p` với p = 0,3 %
#     (⇒ n ≥ 1.000); 0 lỗi/N; 0/N chỉ là **trần**, không phải bảo đảm.
#   - D2-72 Q10 (K3 Q10): `soak.sh 1000 3`, timeout **20 s/boot**, chỉ `kill` PID do chính script
#     tạo (AGENTS §9), BUILD_ID = git sha + sha256 ảnh, ghi qemu/OpenSBI version + cấu hình;
#     pass = đủ `TAP version 13`, `1..K` đúng, mọi dòng `ok`, `# k3 halt`, không `PANIC`;
#     **không retry** (D2-07), **không đếm artifact**.
#   - D2-74 điều kiện (c): soak **KHÔNG** `-icount` (giữ đúng dòng QEMU của D2-72 Q2).
#   - D2-80: mỗi log boot phải có **hash ảnh do kernel tự in** (`# conf image: build=<16 hex>
#     text=<len>`, FNV-1a 64 trên `.text` đã nạp); script băm lại **cùng** section từ ELF
#     (`test/conformance/image_digest.py`) và **từ chối** log không có/lệch hash.
#   - D2-08: ảnh sản phẩm KHÔNG chứa chuỗi `ktest` (kiểm trước khi soak, như `run.sh`).
#
# Bất biến của MỖI boot (fail-closed, không nới):
#   1. đúng 1 `TAP version 13`; đúng 1 `1..1`; đúng 1 `ok 1 - k3 boot`; đúng 1 `# k3 halt`;
#   2. `# k3 image=product` (đúng ảnh sản phẩm, không phải ảnh test);
#   3. đúng 1 `# conf image: build=<16 hex> text=<len>`, build/text **khớp ELF** (D2-80);
#   4. `# smp: boot=0 harts=1 … online=0x1 … timeout=0 … dtb_magic=1` (đúng `-smp 1`, Q4);
#      `Platform HART Count : 1` của **OpenSBI** — bằng chứng ngoài kernel;
#   5. **không** dòng `PANIC` nào;
#   6. quá **20 s** ⇒ `kill` đúng PID, boot đó **đỏ** (không retry).
#
# Dùng:
#   bash v2/test/boot/soak.sh [NBOOT] [LANES]        # mặc định 1000 3 (đúng cổng K3)
#   bash v2/test/boot/soak.sh --smoke N [LANES]      # N < 1.000: chỉ để đỏ trước/kiểm marker;
#                                                    # LUÔN exit 1 (không phải bằng chứng cổng)
#   bash v2/test/boot/soak.sh --image <ảnh> [NBOOT] [LANES]
#                                                    # soak **ảnh có sẵn** (vd ảnh phát hành đã ghim
#                                                    # trong `v2/artifacts/`), KHÔNG build lại
#   bash v2/test/boot/soak.sh --features product-shell,product-fs --disk [NBOOT] [LANES]
#                                                    # **KT-347b** soak **đĩa thật**: gắn
#                                                    # `virtio-blk-pci` (đĩa 4 MiB, snapshot=on) và
#                                                    # đòi `# product fs: medium=virtio-blk` mỗi boot
#   bash v2/test/boot/soak.sh --help
#
# Biến môi trường: `SOAK_LOGDIR`/`KT_LOGDIR` (mặc định `$V2_RUN_TMP` hoặc `$TMPDIR/k3-soak.<pid>`),
#   `OPENSBI` (đường dẫn fw_dynamic).
#
# Exit: 0 = 0 lỗi/N (chỉ ở chế độ cổng, N ≥ 1.000); 1 = có boot đỏ / ảnh đổi giữa soak;
#       2 = lỗi cấu hình (thiếu tool, N < 1.000, LANES ngoài [1,3], build/budget hỏng);
#       3 = TỪ CHỐI ĐO (KT-86/D2-125.3: `# build:` thiếu/lệch HEAD/dirty=1) — 0 boot được đếm.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
# **KT-333d (D2-248 mục 2b)** — LOGDIR (và SOCK ngắn) dùng chung `v2/test/boot/lib/env.sh`
# (KT-333c đã làm logdir theo phiên inline ở 6 runner; nay gom một chỗ, không lặp code).
. "$HERE/lib/env.sh"
# **KT-347b** — vòng soak/tổng hợp/BUILD_ID/kết luận ở **một** nguồn `lib/soak-common.sh` (kéo theo
# `lib/harness-scan.sh` cho D2-08). Runner chỉ giữ `boot_one`/`boot_one_shell`, build, QEMU, guard.
# shellcheck source=/dev/null
. "$HERE/lib/soak-common.sh"
# **KT-407** — helper dùng chung: `rc=126` + `log_inventory` + **một** `trap EXIT` (nợ KT-348b).
# shellcheck source=/dev/null
. "$HERE/lib/run-common.sh"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget.sh"
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
# KT-86 (D2-125.3): cổng danh tính bản dựng — nguồn sự thật duy nhất cho rev/dirty.
. "$HERE/build-rev.sh"
# KT-145: cổng **danh tính ảnh** (`vbrev_image_identity`) — tách khỏi `build-rev.sh` để định nghĩa
# dấu vân tay (được ghim bằng `check-algo-pin.sh`) không phải tăng `VBREV_ALGO` vì một hàm tiêu thụ.
# shellcheck source=/dev/null
. "$HERE/build-identity.sh"
TARGET="riscv64gc-unknown-none-elf"
# **KT-407** — tôn trọng `CARGO_TARGET_DIR` (cùng quy ước cargo; xem `run-aarch64.sh`).
IMG_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release"
IMAGE="$IMG_DIR/vifast-kbin-riscv64"
# Ảnh **sản phẩm không feature** — ảnh duy nhất in dòng `# build:` (KT-86; phải đặt **sau** TARGET).
PRODUCT_IMAGE="$IMG_DIR/vifast-kbin-riscv64"
# **KT-333c** (D2-243 mục 3b) — logdir **riêng theo phiên**: `$V2_RUN_TMP` (cổng tạo) hoặc
# `$TMPDIR/k3-soak.<pid>`; ép bằng `KT_LOGDIR`/`SOAK_LOGDIR`. Hai soak song song không ghi đè log.
LOGDIR="$(v2_env_logdir k3-soak SOAK_LOGDIR)"
if [ "${1:-}" = "--print-logdir" ]; then
    echo "$LOGDIR"
    exit 0
fi
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
QEMU_MIN_MAJOR=9 # sàn D2-72 Q2: QEMU >= 9.2
QEMU_MIN_MINOR=2
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
P_NUM=3        # p = 0,3 % = 3/1000 (D2-24)
P_DEN=1000
SMOKE=0

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "soak.sh: $*" >&2
    exit 2
}

# KT-78 (OD-K6-6): boot đi qua `qemu_guard.sh` — process group riêng, timeout kill **cả group**,
# trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). Soak 3 luồng = 3 subshell cùng script:
# mỗi luồng giữ danh sách PGID của mình; registry trên đĩa dùng chung để watchdog dọn **tất cả**.
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
INFRA_INVALID=0

usage() {
    sed -n 's/^# \{0,1\}//; 2,48p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,60p'
    echo "soak.sh: dùng: soak.sh [NBOOT] [LANES] | soak.sh --smoke N [LANES] | soak.sh --check-image <ảnh.bin> | soak.sh --help"
}

# ------------------------------------------------------------------ đối số
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    usage
    exit 0
fi
# KT-96: soak **ảnh có sẵn** — không build lại. Mặc định (không cờ) giữ nguyên hành vi cũ.
GIVEN_IMAGE=""
if [ "${1:-}" = "--image" ]; then
    [ "$#" -ge 2 ] || die "--image cần đường dẫn ảnh"
    GIVEN_IMAGE="$2"
    shift 2
    [ -f "$GIVEN_IMAGE" ] || die "không thấy ảnh: $GIVEN_IMAGE"
fi
# **KT-145** (Agent 0 phiếu lượt 5 §A4): `--check-image` chỉ kiểm **danh tính** ảnh rồi thoát —
# không build, không mở QEMU. Dùng cho ca hồi quy `v2/test/boot/algo-strict.sh`.
if [ "${1:-}" = "--check-image" ]; then
    [ "$#" -ge 2 ] || die "--check-image cần đường dẫn ảnh"
    CHECK_IMAGE="$2"
    shift 2
    if vbrev_image_identity "$CHECK_IMAGE"; then
        echo "REV-GATE: ACCEPT image=$CHECK_IMAGE algo=$VBREV_IMG_ALGO inputs=$VBREV_IMG_INPUTS dirty=$VBREV_IMG_DIRTY"
        exit 0
    fi
    echo "REV-GATE: REFUSE reason=$VBREV_IMG_REASON image=$CHECK_IMAGE algo=${VBREV_IMG_ALGO:-<none>} current_algo=$VBREV_ALGO"
    exit 1
fi
# **KT-135 (C2 của Agent 0, 2026-11-18)**: `--features` cho phép soak **đúng ảnh phát hành**
# (`product-shell,product-fs`). Mặc định (không cờ) giữ nguyên ảnh **không shell** — bằng chứng cũ
# không mất giá trị. Ảnh có shell không tự thoát, nên chế độ này phải **lái shell** (xem
# `boot_one_shell`): chờ `shell>`, gửi `help`, gửi `exit`, đòi `# product halt:` + QEMU rc=0.
SOAK_FEATURES=""
if [ "${1:-}" = "--features" ]; then
    [ "$#" -ge 2 ] || die "--features cần danh sách feature (ví dụ: product-shell,product-fs)"
    SOAK_FEATURES="$2"
    shift 2
fi
SHELL_MODE=0
case ",$SOAK_FEATURES," in
    *,product-shell,*) SHELL_MODE=1 ;;
esac
# **KT-347b (D2-252 Pha A)** — `--disk`: soak **đĩa thật** `virtio-blk` (profile `virtio-blk` của
# `v2 nightly --soak`). Mặc định **không** đĩa ⇒ hành vi cũ không đổi.
DISK_MODE=0
if [ "${1:-}" = "--disk" ]; then
    DISK_MODE=1
    shift
fi
if [ "${1:-}" = "--smoke" ]; then
    SMOKE=1
    shift
    [ $# -ge 1 ] || die "--smoke cần N (ví dụ: --smoke 3 3)"
fi
# KT-96: ảnh cho trước phải được gán **trước** `QEMU_ARGS` (dòng dưới dùng `$IMAGE`).
if [ -n "$GIVEN_IMAGE" ]; then
    IMAGE="$GIVEN_IMAGE"
    PRODUCT_IMAGE="$GIVEN_IMAGE"
fi
N="${1:-1000}"
LANES="${2:-3}"
case "$N" in '' | *[!0-9]*) die "N phải là số dương (nhận '$N')" ;; esac
case "$LANES" in '' | *[!0-9]*) die "LANES phải là số dương (nhận '$LANES')" ;; esac
[ "$N" -gt 0 ] || die "N phải > 0"
# AGENTS §9: tối đa 3 QEMU song song; D2-24 đúng 3 luồng cho cổng K3.
[ "$LANES" -ge 1 ] && [ "$LANES" -le 3 ] || die "LANES=$LANES ngoài [1,3] (AGENTS §9: tối đa 3 QEMU)"

# D2-24: n ≥ 3/p. p = 3/1000 ⇒ n ≥ ceil(3 · 1000 / 3) = 1000. Tính bằng số nguyên để không
# sai số dấu phẩy động (0,003 không biểu diễn chính xác trong awk/float).
NEED=$(((3 * P_DEN + P_NUM - 1) / P_NUM))
if [ "$N" -lt "$NEED" ]; then
    if [ "$SMOKE" -ne 1 ]; then
        echo "soak.sh: N=$N < n ≥ 3/p = $NEED (p=$P_NUM/$P_DEN = 0,3 %; D2-24, QD §5.3)" >&2
        echo "soak.sh: giảm N là NỚI NGƯỠNG — không chạy. Dùng --smoke chỉ để đỏ trước/kiểm marker." >&2
        exit 2
    fi
    echo "# soak: SMOKE N=$N < $NEED — KHÔNG phải bằng chứng cổng K3 (D2-24), luôn exit 1"
fi

# **KT-333c** (D2-243 mục 3a) — khoá **độc quyền cấp máy đo** (fail-closed): soak là phép đo dài;
# chạy chồng `gate`/soak khác làm QEMU chồng tải và mọi số wall/boot/s vô nghĩa. `v2 nightly` giữ
# khoá ở tiến trình cha và uỷ quyền qua `V2_MEASURE_LOCK_DELEGATED_BY` (con không tạo khoá mới).
# shellcheck source=/dev/null
. "$V2_ROOT/tools/measure-lock.sh"
measure_lock_acquire "soak riscv64 N=$N lanes=$LANES" || exit 2
# **KT-407** — một lời gọi duy nhất (`run_common_trap` cài EXIT/INT/TERM/HUP).
_soak_rv_cleanup() {
    qg_cleanup 2>/dev/null || true
    measure_lock_release_if_top 2>/dev/null || true
}
run_common_trap _soak_rv_cleanup

# ------------------------------------------------------------------ công cụ
command -v cargo >/dev/null 2>&1 || die "không có cargo"
command -v python3.12 >/dev/null 2>&1 || die "không có python3.12 (cần cho image_digest.py)"
[ -f "$DIGEST" ] || die "không thấy $DIGEST"
[ -f "$BUDGET" ] || die "không thấy $BUDGET"
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

# QEMU >= 9.2 (D2-72 Q2) — thiếu/dưới sàn ⇒ đỏ, không tự hạ.
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

# **KT-347b** — đĩa thật cho soak `virtio-blk`: 4 MiB raw (cùng hình dạng `product-riscv64.sh`),
# `snapshot=on` để 3 luồng dùng **cùng** tệp mà không tranh khoá ghi (bài học KT-273).
SOAK_DISK_ARGS=()
if [ "$DISK_MODE" -eq 1 ]; then
    DISK="$LOGDIR/disk0.raw"
    dd if=/dev/zero of="$DISK" bs=1024 count=4096 2>/dev/null || die "không tạo được đĩa tạm $DISK"
    [ "$(wc -c <"$DISK" | tr -d ' ')" -eq 4194304 ] || die "đĩa tạm $DISK không phải 4 MiB"
    SOAK_DISK_ARGS=(-drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on"
        -device virtio-blk-pci,drive=d0)
fi

QEMU_ARGS=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" -nographic \
    -cpu rv64 -m 128M -smp 1 -no-reboot ${SOAK_DISK_ARGS[@]+"${SOAK_DISK_ARGS[@]}"}) # D2-72 Q2; KHÔNG -icount (D2-74 c)

# ------------------------------------------------------------------ build + neo ảnh
echo "# soak KT-57 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2); opensbi $SBI"
echo "# cấu hình: -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot; KHÔNG icount (D2-74 c)"
if [ "$DISK_MODE" -eq 1 ]; then
    echo "# cấu hình: + virtio-blk-pci (đĩa thật 4 MiB trong $LOGDIR, snapshot=on; KT-347b)"
fi
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
# KT-86: log boot của lần dò danh tính (cổng trước phép đo).
REV_GATE_LOG="$LOGDIR/kt86-rev-gate.log"
INFRA_LOG="$LOGDIR/kt78-infra.log"
# KT-78/OD-K6-6: trạng thái hạ tầng **trước** phép đo (wall soak gồm cả build).
qg_infra before "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))

if [ -n "$GIVEN_IMAGE" ]; then
    # KT-96: soak.sh đo "boot tới **halt**". Ảnh **có shell** không tự thoát ⇒ mọi boot sẽ
    # `timeout` ⇒ 0/N là **giả**. Từ chối sớm thay vì để người đọc hiểu sai.
    if [ "$SHELL_MODE" -ne 1 ] && LC_ALL=C grep -a -q -m1 '# product shell:' "$IMAGE" 2>/dev/null; then
        echo "soak.sh: ảnh '$IMAGE' CÓ shell (không tự thoát) — soak.sh không đo được ảnh này." >&2
        echo "soak.sh: dùng script gửi 'exit' qua UART (xem v2/agent/JOURNAL/kt90-soak-product-image.sh)" >&2
        exit 2
    fi
    echo "# ảnh cho trước (không build): $IMAGE"
elif [ -n "$SOAK_FEATURES" ]; then
    # **KT-135 (C2 của Agent 0)**: khi soak **ảnh phát hành**, phải build bằng **pipeline canonical**
    # (`repro-build.sh`: remap + strip) — nếu build bằng `cargo build` trần thì sha KHÁC ảnh phát
    # hành, và "soak ảnh phát hành" sẽ là soak một ảnh khác. Đây đúng yêu cầu "phải ra đúng sha ảnh
    # phát hành".
    echo "# build ảnh sản phẩm bằng pipeline canonical ($TARGET, release, features=$SOAK_FEATURES)"
    REPRO_OUT="$(VIFAST_REPRO_CARGO_EXTRA="--features $SOAK_FEATURES" \
        VIFAST_REPRO_LOG="$LOGDIR/kt57-soak-build.log" bash "$REPO_ROOT/v2/tools/repro-build.sh" 2>&1)" || {
        printf '%s\n' "$REPRO_OUT" | tail -20 >&2
        die "repro-build ảnh sản phẩm FAIL (xem $LOGDIR/kt57-soak-build.log)"
    }
    printf '%s\n' "$REPRO_OUT" | grep -E '^REPRO ' | tail -1
    IMAGE="$(printf '%s\n' "$REPRO_OUT" | sed -n 's/.* image=\(.*\)$/\1/p' | tail -1)"
    [ -n "$IMAGE" ] && [ -f "$IMAGE" ] || die "repro-build không trả đường dẫn ảnh"
    PRODUCT_IMAGE="$IMAGE"
    # Neo: sha ảnh build ra phải trùng ảnh phát hành đã ghim (nếu có ghim).
    BUILT_SHA="$(shasum -a 256 "$IMAGE" 2>/dev/null | awk '{print $1}')"
    [ -n "$BUILT_SHA" ] || BUILT_SHA="$(sha256sum "$IMAGE" | awk '{print $1}')"
    PINNED="$REPO_ROOT/v2/artifacts/$BUILT_SHA.bin"
    if [ -f "$PINNED" ]; then
        echo "# neo ảnh: sha=$BUILT_SHA TRÙNG ảnh đã ghim $PINNED"
    else
        echo "# neo ảnh: sha=$BUILT_SHA (chưa ghim; ghim bằng v2/tools/pin-artifact.sh nếu cần)"
    fi
else
    echo "# build ảnh sản phẩm ($TARGET, release)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 \
        --target "$TARGET" --release >"$LOGDIR/kt57-soak-build.log" 2>&1; then
        tail -20 "$LOGDIR/kt57-soak-build.log" >&2
        die "build ảnh sản phẩm FAIL (xem $LOGDIR/kt57-soak-build.log)"
    fi
fi
[ -f "$IMAGE" ] || die "không thấy ảnh sản phẩm $IMAGE"
# **KT-145** (Agent 0 phiếu lượt 5 §A4): soak là **đường đo** ⇒ chỉ nhận ảnh tự khai ĐÚNG định
# nghĩa dấu vân tay hiện hành. Ảnh dạng cũ (thiếu `algo=`) hoặc `algo=` khác ⇒ TỪ CHỐI **trước**
# khi boot, không để phép đo chạy rồi mới phát hiện. (Chỉ `status.sh`/`pin-artifact.sh` được nhận
# dạng cũ — chúng hiển thị/ghim ảnh lịch sử, không đo.)
if ! vbrev_image_identity "$IMAGE"; then
    vbrev_refuse "$VBREV_IMG_REASON" "image=$IMAGE algo=${VBREV_IMG_ALGO:-<none>} current_algo=$VBREV_ALGO"
    die "ảnh '$IMAGE' KHÔNG cùng định nghĩa dấu vân tay ($VBREV_IMG_REASON) — soak từ chối (KT-145)"
fi
# KT-135: `QEMU_ARGS` được dựng **trước** khối build (dòng ~177) nên nếu nhánh canonical đổi
# `IMAGE` thì phải dựng lại — nếu không, soak sẽ boot ảnh cũ trong `target/`.
QEMU_ARGS=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" -nographic \
    -cpu rv64 -m 128M -smp 1 -no-reboot ${SOAK_DISK_ARGS[@]+"${SOAK_DISK_ARGS[@]}"})

# D2-72 Q5: `.data+.sdata+.bss+.sbss` ≤ 262.144 B — fail nếu vượt, không nới.
bash "$BUDGET" "$IMAGE" product || die "budget ảnh sản phẩm VƯỢT trần (D2-72 Q5)"

# D2-08: ảnh sản phẩm không được chứa chuỗi `ktest` — quét ở **một** nguồn `lib/harness-scan.sh`
# (cùng cách đếm dòng `strings` như trước; tệp vắng ⇒ rc 1 chứ không "0 hit" giả).
HITS="$(harness_scan_count_strings "$IMAGE" 'ktest')" || die "không đọc được ảnh $IMAGE (TOCTOU)"
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

# --------------------------------------------------- KT-86: cổng danh tính bản dựng
# KT-86 (D2-125.3): **trước** phép đo, ảnh phải chứng minh nguồn gốc — in `# build: rev=<12 hex>
# dirty=<0|1>` (từ `const`, D2-102) khớp `git rev-parse HEAD` và cây nguồn sạch. Ảnh test nằm ở
# bin riêng (`src/ktest*.rs`, ngoài vùng ghi KT-86) nên không in dòng này; soak boot **ảnh sản
# phẩm** (`src/main.rs`, cùng crate/build script/env danh tính) làm chứng. Cổng đỏ ⇒ exit 3.
echo "# rev-gate (KT-86): build + boot MỘT lần ảnh sản phẩm để đọc dòng '# build:'"
vbrev_summary "$REPO_ROOT"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64 \
    --target "$TARGET" --release ${SOAK_FEATURES:+--features "$SOAK_FEATURES"} >"$LOGDIR/kt86-product-build.log" 2>&1; then
    tail -20 "$LOGDIR/kt86-product-build.log" >&2
    vbrev_refuse product-build-failed "log=$LOGDIR/kt86-product-build.log"
    echo "# soak.sh: TỪ CHỐI ĐO (KT-86/D2-125.3)"
    exit 3
fi
if ! qg_boot "$REV_GATE_LOG" "$TIMEOUT_DS" "$QEMU" \
    -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$PRODUCT_IMAGE" \
    -nographic -cpu rv64 -m 128M -smp 1 -no-reboot ${SOAK_DISK_ARGS[@]+"${SOAK_DISK_ARGS[@]}"}; then
    # KT-96: ảnh **có shell** không tự thoát ⇒ QEMU bị timeout giết ⇒ `qg_boot` trả != 0 dù boot
    # thành công. Với ảnh cho trước, chấp nhận nếu log **đã có** dòng `# build:` (mục đích của
    # lần dò này là đọc dòng đó); `vbrev_gate` ngay dưới vẫn kiểm byte ảnh.
    # KT-100: chỉ chấp nhận **timeout** (ảnh có shell không tự thoát), KHÔNG chấp nhận crash:
    # log có `PANIC` hoặc `Fault` ⇒ vẫn là `probe-boot-failed`.
    # KT-135: ảnh **có shell** (chế độ `--features`) cũng không tự thoát — cùng lý do với ảnh cho
    # trước ở KT-96. Vẫn chỉ chấp nhận **timeout**, không chấp nhận crash (điều kiện PANIC bên dưới).
    if { [ -n "$GIVEN_IMAGE" ] || [ "$SHELL_MODE" -eq 1 ]; } &&
        LC_ALL=C grep -a -q -m1 '# build:' "$REV_GATE_LOG" 2>/dev/null &&
        ! LC_ALL=C grep -a -qE 'PANIC|FaultKind' "$REV_GATE_LOG" 2>/dev/null; then
        echo "# rev-gate: ảnh có shell không tự thoát (timeout) — đã có dòng '# build:', tiếp tục"
    else
        vbrev_refuse probe-boot-failed "log=$REV_GATE_LOG"
        echo "# soak.sh: TỪ CHỐI ĐO (KT-86/D2-125.3)"
        exit 3
    fi
fi
if ! vbrev_gate "$REV_GATE_LOG" "$REPO_ROOT" "$PRODUCT_IMAGE"; then
    echo "# soak.sh: TỪ CHỐI ĐO (KT-86/D2-125.3) — không đếm một boot soak nào"
    exit 3
fi

# ------------------------------------------------------------------ log
mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"
RUNDIR="$LOGDIR/run-$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$RUNDIR" || die "không tạo được $RUNDIR"
RESULTS="$RUNDIR/results.txt"
: >"$RESULTS" || die "không ghi được $RESULTS"
echo "# log: $RUNDIR/boot-<i>.log (mỗi boot một tệp; kết quả: $RESULTS)"

# ------------------------------------------------------------------ một boot
# boot_one <i>: chạy 1 boot, kiểm 6 bất biến, ghi `ok i` hoặc `fail i <lý do>`. Trả 0/1.
boot_one() {
    local i="$1" out="$RUNDIR/boot-$i.log" pid="" waited=0 why="" got_build="" got_text="" n=0
    local tap="" plan="" ok1="" halt="" img=""

    # KT-78: QEMU trong process group riêng + timeout kill cả group (125 = không khởi động được).
    qg_boot "$out" "$TIMEOUT_DS" "$QEMU" "${QEMU_ARGS[@]}"
    if [ "$?" -ne 0 ]; then
        pid="$QG_LAST_PID"
        printf 'fail %s timeout-%ss\n' "$i" "$((TIMEOUT_DS / 10))" >>"$RESULTS"
        echo "boot $i/$N FAIL: quá $((TIMEOUT_DS / 10)) s (treo) pid=$pid log=$out" >&2
        return 1
    fi

    # (1) TAP khung — đúng một dòng mỗi mẫu, theo đúng thứ tự version < plan < ok.
    tap="$(grep -c -x -F 'TAP version 13' "$out" || true)"
    plan="$(grep -c -x -F '1..1' "$out" || true)"
    ok1="$(grep -c -x -F 'ok 1 - k3 boot' "$out" || true)"
    halt="$(grep -c -x -F '# k3 halt' "$out" || true)"
    [ "${tap:-0}" -eq 1 ] || why="$why tap-version=${tap:-0}"
    [ "${plan:-0}" -eq 1 ] || why="$why plan-1..1=${plan:-0}"
    [ "${ok1:-0}" -eq 1 ] || why="$why ok1=${ok1:-0}"
    [ "${halt:-0}" -eq 1 ] || why="$why halt=${halt:-0}"
    [ "$(grep -cE '^not ok' "$out" || true)" -eq 0 ] || why="$why not-ok"
    # "mọi dòng `ok`" đúng — không có `ok` lạ/thiếu số.
    n="$(grep -cE '^ok [0-9]+' "$out" || true)"
    [ "${n:-0}" -eq 1 ] || why="$why ok-count=${n:-0}/1"
    # (2) đúng ảnh sản phẩm.
    grep -q '^# k3 image=product' "$out" || why="$why image-product"
    # (3) D2-80 — hash ảnh do kernel tự in, khớp ELF đang chạy.
    n="$(grep -cE '^# conf image: build=[0-9a-f]+ text=[0-9]+$' "$out" || true)"
    [ "${n:-0}" -eq 1 ] || why="$why image-line=${n:-0}/1"
    got_build="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$out" | head -1)"
    got_text="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$out" | head -1)"
    [ "$got_build" = "$WANT_BUILD" ] || why="$why image-build=${got_build:-none}/$WANT_BUILD"
    [ "$got_text" = "$WANT_TEXT" ] || why="$why image-text=${got_text:-none}/$WANT_TEXT"
    # (4) đúng cấu hình một hart + bằng chứng firmware ngoài kernel.
    grep -Eq '^# smp: boot=0 harts=1 .*online=0x1.* timeout=0 .*dtb_magic=1' "$out" ||
        why="$why smp-config"
    grep -Eq '^Platform HART Count *: *1' "$out" || why="$why fw-hart-count"
    grep -q '^OpenSBI v[0-9]' "$out" || why="$why opensbi"
    # (5) không PANIC nào (in cả dòng thụt đầu dòng — fail-closed).
    n="$(grep -cE '^[[:space:]]*PANIC' "$out" || true)"
    [ "${n:-0}" -eq 0 ] || why="$why panic=${n:-0}"

    if [ -n "$why" ]; then
        printf 'fail %s %s\n' "$i" "$(printf '%s' "$why" | tr ' ' ',')" >>"$RESULTS"
        echo "boot $i/$N FAIL:$why log=$out" >&2
        tail -5 "$out" | sed 's/^/    /' >&2
        return 1
    fi
    printf 'ok %s\n' "$i" >>"$RESULTS"
    echo "boot $i/$N ok"
    return 0
}

# ------------------------------------------------------------------ vòng cho ảnh CÓ shell
# **KT-135 (C2 của Agent 0)**: ảnh phát hành không tự thoát — nó chờ lệnh. Vòng soak phải **lái**
# shell và đòi **ba** dấu hiệu mỗi vòng: `shell>` (U-mode đã lên), `# shell: help cmds=` (shell
# thật sự nhận và xử lý lệnh), `# shell: exit code=0` + `# product halt:` (thoát sạch). Thiếu bất
# kỳ dấu hiệu nào ⇒ vòng đó ĐỎ. Đây là điều kiện Agent 0 đặt: "thiếu một dấu hiệu ⇒ đỏ".
P_SHELL=0.03
P_HELP=0.05
T_SHELL_POLLS=667 # 667 × 0,03 s ≈ 20 s tới `shell>` (D2-72 Q10)
T_HELP_POLLS=100  # 100 × 0,05 s = 5 s chờ phản hồi `help`
T_HALT_POLLS=200  # 200 × 0,05 s = 10 s chờ `# product halt:` + QEMU thoát

soak_wait_re() { # $1 ERE, $2 max polls, $3 poll secs, $4 pid, $5 log
    local i=0
    while [ "$i" -lt "$2" ]; do
        grep -qE "$1" "$5" 2>/dev/null && return 0
        kill -0 "$4" 2>/dev/null || break
        sleep "$3"
        i=$((i + 1))
    done
    grep -qE "$1" "$5" 2>/dev/null
}

boot_one_shell() {
    local i="$1" out="$RUNDIR/boot-$i.log" fifo="$RUNDIR/fifo-$i" pid="" why="" n=""
    rm -f "$fifo" 2>/dev/null || true
    mkfifo "$fifo" 2>/dev/null || {
        printf 'fail %s infra no-fifo\n' "$i" >>"$RESULTS"
        return 1
    }
    # RDWR: `open` của QEMU không chặn; byte bơm vào nằm trong pipe tới khi QEMU đọc.
    exec 8<>"$fifo" || {
        printf 'fail %s infra no-fifo-open\n' "$i" >>"$RESULTS"
        return 1
    }
    : >"$out"
    "$QEMU" "${QEMU_ARGS[@]}" <"$fifo" >"$out" 2>&1 &
    pid=$!

    # (1) `shell>` — U-mode shell đã lên.
    soak_wait_re '^shell>$' "$T_SHELL_POLLS" "$P_SHELL" "$pid" "$out" || why="$why,timeout-shell"
    # (2) `help` — shell nhận và xử lý lệnh thật.
    if [ -z "$why" ]; then
        printf 'help\n' >&8 2>/dev/null || true
        soak_wait_re '^# shell: help cmds=[0-9]+$' "$T_HELP_POLLS" "$P_HELP" "$pid" "$out" ||
            why="$why,no-help-response"
    fi
    # (3) `exit` — thoát sạch: shell in mã thoát, kernel in dòng halt, QEMU tự thoát.
    if [ -z "$why" ]; then
        printf 'exit\n' >&8 2>/dev/null || true
        soak_wait_re '^# shell: exit code=0$' "$T_HALT_POLLS" "$P_HELP" "$pid" "$out" ||
            why="$why,no-exit-line"
        soak_wait_re '^# product halt:' "$T_HALT_POLLS" "$P_HELP" "$pid" "$out" ||
            why="$why,no-halt-line"
        local g=0
        while [ "$g" -lt "$T_HALT_POLLS" ]; do
            kill -0 "$pid" 2>/dev/null || break
            sleep "$P_HELP"
            g=$((g + 1))
        done
        kill -0 "$pid" 2>/dev/null && why="$why,qemu-khong-thoat"
    fi
    exec 8>&-
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    else
        wait "$pid" 2>/dev/null
        [ "$?" -eq 0 ] || why="$why,qemu-rc-khac-0"
    fi
    rm -f "$fifo" 2>/dev/null || true

    # (4) danh tính ảnh: dòng `# build:` do kernel in phải khớp ảnh đang chạy.
    # KT-143: dòng danh tính nay có `algo=` — đòi **đúng** dạng mới, không nhận dạng cũ, vì soak
    # chỉ đo ảnh vừa build từ cây hiện tại.
    n="$(grep -cE '^# build: algo=[0-9A-Za-z._-]+ inputs=[0-9a-f]+ dirty=0$' "$out" || true)"
    [ "${n:-0}" -eq 1 ] || why="$why build-line=${n:-0}/1"
    # **KT-347b** — soak đĩa thật: mỗi boot phải mở được FS trên `virtio-blk` (không chỉ "có -drive").
    if [ "$DISK_MODE" -eq 1 ]; then
        grep -qE '^# product fs: medium=virtio-blk ' "$out" || why="$why no-virtio-blk-fs"
    fi
    # (5) không PANIC (fail-closed, tính cả dòng thụt đầu dòng).
    n="$(grep -cE '^[[:space:]]*PANIC' "$out" || true)"
    [ "${n:-0}" -eq 0 ] || why="$why panic=${n:-0}"

    if [ -n "$why" ]; then
        printf 'fail %s %s\n' "$i" "$(printf '%s' "$why" | tr ' ' ',')" >>"$RESULTS"
        echo "boot $i/$N FAIL:$why log=$out" >&2
        tail -5 "$out" | sed 's/^/    /' >&2
        return 1
    fi
    printf 'ok %s\n' "$i" >>"$RESULTS"
    echo "boot $i/$N ok"
    return 0
}

# ------------------------------------------------------------------ luồng
# **KT-347b** — vòng luồng ở **một** nguồn `lib/soak-common.sh` (mỗi slot đúng 1 lần, không retry).
soak_boot_dispatch() {
    if [ "$SHELL_MODE" -eq 1 ]; then boot_one_shell "$1" || true; else boot_one "$1" || true; fi
}
soak_common_run_lanes "$N" "$LANES" soak_boot_dispatch
WALL="$SOAK_WALL"

# ------------------------------------------------------------------ tổng
# KT-78/OD-K6-6: trạng thái hạ tầng **sau** phép đo; QEMU mồ côi/lạ ⇒ phép đo KHÔNG HỢP LỆ.
qg_infra after "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
echo "# infra: invalid=$INFRA_INVALID isolation_bad=$QG_ISO_BAD guard_log=$QG_LOG"
qg_guard_summary "$QG_LOG" >"$LOGDIR/guard-summary.log" 2>/dev/null || true
echo "# guard_summary_log=$LOGDIR/guard-summary.log"

# **KT-347b** — đếm/neo ảnh/BUILD_ID/kết luận ở **một** nguồn `lib/soak-common.sh` (SMOKE **luôn**
# exit 1, không retry — giữ nguyên hợp đồng cũ).
SOAK_N="$N"
SOAK_NEED="$NEED"
SOAK_SMOKE="$SMOKE"
SOAK_TAG="soak.sh"
SOAK_SMOKE_MSG="K3"
SOAK_GIT_HEAD="$GIT_HEAD"
SOAK_IMG_SHA="$IMG_SHA"
SOAK_BUILD_ID="$WANT_BUILD"
SOAK_IMAGE="$IMAGE"
SOAK_RESULTS="$RESULTS"
SOAK_WALL="$WALL"
soak_common_finish
exit $?
