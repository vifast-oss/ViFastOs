#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-79 (OD-K6-3) — BUILD TÁI LẬP BYTE ảnh riscv64: bọc `cargo build` của ảnh sản phẩm
# bằng đúng **ba** đầu vào đã chứng minh là nguồn gây khác sha (xem `v2/docs/k6-repro.md` §3):
#
#   1. `--remap-path-prefix=<gốc-cây>/=/vifast` — che đường dẫn tuyệt đối. Lý do đo được: crate
#      `v2/lib/vifast-alloc` nằm **ngoài** workspace `v2/kernel`, nên cargo truyền đường dẫn
#      **tuyệt đối** cho rustc; `panic!`/bounds-check của crate đó nhúng nguyên văn đường dẫn vào
#      `.rodata` (panic location), dài/ngắn theo chỗ checkout ⇒ `.rodata` đổi cỡ, mọi tham chiếu
#      `auipc/addi` tới `.rodata` trong `.text` đổi theo (log: `JOURNAL/kt79-baseline-diff.log`).
#      KHÔNG remap bằng tiền tố tương đối: rustc so khớp tiền tố **nguyên văn**, không chuẩn hoá
#      `..` theo cwd (đo ở `JOURNAL/kt79-remap-relative.log`) ⇒ script này tính đường dẫn tuyệt
#      đối của chính cây đang build.
#   2. `-C strip=symbols` — bỏ `.symtab`/`.strtab`. Crate ngoài workspace có **metadata hash**
#      phụ thuộc đường dẫn tuyệt đối (đo: `libvifast_alloc-<hash>` khác nhau mỗi chỗ checkout),
#      nên tên symbol mang hash đường dẫn; strip cắt hẳn nguồn đó. `.text` digest (D2-80) vẫn
#      đọc được vì section header còn nguyên — kiểm bằng `v2/test/conformance/image_digest.py`.
#   3. `CARGO_INCREMENTAL=0`, `SOURCE_DATE_EPOCH=0`, `LC_ALL=C`, toolchain **ghim** ở
#      `rust-toolchain.toml` (1.99.1) — biến môi trường/thời gian không được rò vào ảnh.
#
# Dùng:
#   bash v2/tools/repro-build.sh [--root DIR] [--bin NAME] [--package NAME]
#                                [--target-dir DIR] [--break] [--quiet]
#   --target-dir DIR : bắt buộc KHÁC cây nguồn (mặc định: thư mục tạm mới). Trỏ vào cây nguồn
#                      ⇒ fail-closed (A1-175: không được ghi đè ảnh phát hành trong repo).
#   --break : CỐ TÌNH tắt remap (giữ strip) — dùng cho ca âm của `v2/tools/repro-check.sh`.
#             Cũng nhận qua env `VIFAST_REPRO_BREAK=1`.
#   In dòng cuối `REPRO machine=… root=… sha256=… size=… build=… text=…` để script khác so.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_ROOT="$(cd "$HERE/../.." && pwd)"

ROOT="$DEFAULT_ROOT"
BIN="vifast-kbin-riscv64"
PACKAGE="vifast-kbin-riscv64"
TARGET="riscv64gc-unknown-none-elf"
TARGET_DIR=""
BREAK="${VIFAST_REPRO_BREAK:-0}"
QUIET=0
REDIRECT="/vifast"

die() { echo "repro-build: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --root) [ -n "${2:-}" ] || die "--root cần thư mục"; ROOT="$2"; shift 2 ;;
        --bin) [ -n "${2:-}" ] || die "--bin cần tên"; BIN="$2"; shift 2 ;;
        --package) [ -n "${2:-}" ] || die "--package cần tên"; PACKAGE="$2"; shift 2 ;;
        --target-dir) [ -n "${2:-}" ] || die "--target-dir cần thư mục"; TARGET_DIR="$2"; shift 2 ;;
        --break) BREAK=1; shift ;;
        --quiet) QUIET=1; shift ;;
        -h | --help)
            sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) die "tham số không biết: $1" ;;
    esac
done

ROOT="$(cd "$ROOT" 2>/dev/null && pwd -P)" || die "không có thư mục gốc"
[ -f "$ROOT/v2/kernel/Cargo.toml" ] || die "không thấy $ROOT/v2/kernel/Cargo.toml"
[ -f "$ROOT/v2/kernel/rust-toolchain.toml" ] || [ -f "$ROOT/v2/rust-toolchain.toml" ] ||
    die "không thấy file ghim toolchain của dòng OS (v2/rust-toolchain.toml hoặc v2/kernel/rust-toolchain.toml)"

# cargo/rustc có thể không nằm trên PATH (máy ps: ~/.cargo/bin) — cùng cách tìm như v2/bin/v2.
find_cargo() {
    # Ưu tiên rustup shim trong $HOME/.cargo/bin: trên máy có cargo của distro (ps: 1.93.1)
    # thì `command -v cargo` sẽ trúng bản distro và **bỏ qua** toolchain đã ghim.
    local c
    for c in "$HOME/.cargo/bin/cargo" "/usr/local/cargo/bin/cargo" "/opt/homebrew/bin/cargo"; do
        [ -x "$c" ] && { echo "$c"; return 0; }
    done
    if command -v cargo >/dev/null 2>&1; then command -v cargo; return 0; fi
    return 1
}
CARGO="$(find_cargo)" || die "không tìm thấy cargo"
export PATH="$(dirname "$CARGO"):$PATH"

# Ghim toolchain **tường minh**: không dựa vào `rust-toolchain.toml` ở gốc repo (A1-174: file đó
# sẽ phủ cả crates/viscript của dòng khác). Nếu thiếu toolchain hoặc thiếu rustup thì cảnh báo —
# script KHÔNG tự hạ/đổi ngưỡng, chỉ nói rõ môi trường không đúng.
REPRO_TOOLCHAIN="${VIFAST_REPRO_TOOLCHAIN:-1.99.1}"
if command -v rustup >/dev/null 2>&1; then
    if rustup toolchain list 2>/dev/null | grep -q "^${REPRO_TOOLCHAIN}"; then
        export RUSTUP_TOOLCHAIN="$REPRO_TOOLCHAIN"
    else
        echo "# repro-build: CẢNH BÁO không thấy toolchain $REPRO_TOOLCHAIN qua rustup — dùng mặc định" >&2
    fi
else
    echo "# repro-build: CẢNH BÁO không có rustup — không ép được toolchain $REPRO_TOOLCHAIN" >&2
fi

# Đường dẫn tuyệt đối **đã chuẩn hoá** của gốc cây: đây là tiền tố remap. Trên macOS `/tmp` là
# symlink tới `/private/tmp` — cargo/rustc canonicalise, nên phải dùng `pwd -P` (đo: remap bằng
# `/tmp/...` KHÔNG khớp, xem `JOURNAL/kt79-remap-relative.log`).
if [ "$BREAK" = "1" ]; then
    REMAP=""
    REMAP_NOTE="break: KHÔNG remap (ca âm)"
else
    REMAP="--remap-path-prefix=$ROOT=$REDIRECT"
    REMAP_NOTE="remap $ROOT -> $REDIRECT"
fi

export RUSTFLAGS="$REMAP -C strip=symbols ${VIFAST_REPRO_RUSTFLAGS:-}"

# ---------------------------------------------------------------------------------------------
# KT-86 (D2-125.3) — **danh tính bản dựng** đi cùng ảnh: `rev` 12 hex + cờ `dirty`.
#
# Định nghĩa rev/dirty nằm ở **một** chỗ: `v2/test/boot/build-rev.sh` (harness dùng đúng tệp đó),
# nên script build và cổng đo không thể lệch định nghĩa. Ở đây chỉ **truyền** giá trị vào build
# qua env (`build.rs` đọc rồi `cargo:rustc-env`) để ảnh in `# build: rev=… dirty=…` lúc boot.
#
# Không có git (bản sao cây nguồn của `repro-check.sh` chép **không** kèm `.git`) ⇒ `rev=unknown`,
# `dirty=1`: ảnh **nói thật** là không biết nguồn, và cổng đo của harness sẽ **từ chối** ảnh đó.
# Script **không** tự bịa rev — bịa đúng là lớp lỗi đang chặn (D2-108A).
. "$HERE/../test/boot/build-rev.sh"
BUILD_REV="$(vbrev_rev12 "$ROOT")"
BUILD_DIRTY="$(vbrev_dirty "$ROOT")"
if [ -z "$BUILD_REV" ]; then
    BUILD_REV="unknown"
    echo "# repro-build: CẢNH BÁO không đọc được git rev ở $ROOT — ảnh mang rev=unknown, cổng đo sẽ TỪ CHỐI" >&2
fi
export VIFAST_BUILD_REV="$BUILD_REV"
export VIFAST_BUILD_DIRTY="$BUILD_DIRTY"
# D2-127: danh tính **quyết định** là dấu vân tay **nội dung build-inputs** (không phải rev).
# Cùng nguồn sự thật với harness: `vbrev_inputs_calc` của `v2/test/boot/build-rev.sh`.
if vbrev_inputs_calc "$ROOT"; then
    BUILD_INPUTS="$VBREV_CALC_SHORT"
    BUILD_INPUTS_COUNT="$VBREV_CALC_COUNT"
else
    BUILD_INPUTS="unknown"
    BUILD_INPUTS_COUNT="0"
    echo "# repro-build: CẢNH BÁO không tính được dấu vân tay build-inputs (${VBREV_CALC_ERROR:-unknown}) — ảnh mang inputs=unknown, cổng đo sẽ TỪ CHỐI" >&2
fi
export VIFAST_BUILD_INPUTS="$BUILD_INPUTS"
# KT-143: tên **định nghĩa** dấu vân tay cũng vào ảnh (dòng `# build: algo=…`), để một log tự khai
# nó được tính bằng công thức nào. `VBREV_ALGO` là nguồn sự thật duy nhất (build-rev.sh).
export VIFAST_BUILD_ALGO="$VBREV_ALGO"
export VIFAST_BUILD_INPUTS_COUNT="$BUILD_INPUTS_COUNT"

# A1-175: **cấm** dùng `target/` của repo cho build canonical — nó là nơi ảnh phát hành
# (`v2/kernel/target/<target>/release/<bin>`) do KT-75/76/78 soak/đo. Mặc định = thư mục tạm
# riêng; nếu người dùng trỏ vào trong cây nguồn thì **fail-closed, không build**.
if [ -z "$TARGET_DIR" ]; then
    TARGET_DIR="$(mktemp -d "${TMPDIR:-/tmp}/vifast-repro-target.XXXXXX")" || die "mktemp target dir"
fi
mkdir -p "$TARGET_DIR" || die "không tạo được target dir: $TARGET_DIR"
TARGET_DIR="$(cd "$TARGET_DIR" && pwd -P)" || die "không vào được target dir: $TARGET_DIR"
case "$TARGET_DIR" in
    "$ROOT" | "$ROOT"/*)
        die "CARGO_TARGET_DIR=$TARGET_DIR nằm TRONG cây nguồn ($ROOT) — cấm (A1-175: sẽ ghi đè ảnh phát hành). Dùng thư mục tạm."
        ;;
esac
export CARGO_TARGET_DIR="$TARGET_DIR"

# A1-175: ảnh phát hành trong repo phải KHÔNG đổi byte. Ghi sha trước khi build; so lại sau.
REL_IMAGE="$ROOT/v2/kernel/target/$TARGET/release/$BIN"
sha_of() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'; else sha256sum "$1" | awk '{print $1}'; fi; }
REL_BEFORE=""
[ -f "$REL_IMAGE" ] && REL_BEFORE="$(sha_of "$REL_IMAGE")"


# 4) `RUSTC_WRAPPER=v2/tools/rustc-canon.rs` — chuẩn hoá `\\` → `/` và thay `-C metadata=…`
#    (cargo băm đường dẫn/RUSTFLAGS vào hash crate) bằng hằng số; không có bước này, hai đường
#    dẫn khác nhau có thể cho **crate khác sở hữu** một monomorphization dùng chung ⇒ lệch layout
#    `.text` (đo được: bản không wrapper cho 1924266c… ở đường ngắn và c1072d72… ở đường dài).
WRAP_DIR="${TMPDIR:-/tmp}/vifast-kt79-wrapper"
mkdir -p "$WRAP_DIR" || die "không tạo được $WRAP_DIR"
if [ ! -x "$WRAP_DIR/rustc-canon" ] || [ "$ROOT/v2/tools/rustc-canon.rs" -nt "$WRAP_DIR/rustc-canon" ]; then
    rustc -O "$ROOT/v2/tools/rustc-canon.rs" -o "$WRAP_DIR/rustc-canon" 2>"$WRAP_DIR/build.err" ||
        die "biên dịch rustc-canon FAIL (xem $WRAP_DIR/build.err)"
fi
export RUSTC_WRAPPER="$WRAP_DIR/rustc-canon"

[ "$QUIET" = "1" ] || {
    echo "# repro-build: máy $(uname -sm), gốc=$ROOT"
    echo "# repro-build: $(rustc -V) | $("$CARGO" -V) | toolchain=$(rustup show active-toolchain 2>/dev/null | tr '\n' ' ')"
    echo "# repro-build: RUSTFLAGS=[$RUSTFLAGS] ($REMAP_NOTE)"
    echo "# repro-build: CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-<mặc định $ROOT/v2/kernel/target>}"
    echo "# repro-build: build identity rev=$BUILD_REV dirty=$BUILD_DIRTY (KT-86; dirty = thay đổi chưa commit trong v2/kernel+v2/lib)"
}

t0="$(date +%s)"
"$CARGO" build --manifest-path "$ROOT/v2/kernel/Cargo.toml" -p "$PACKAGE" --bin "$BIN" \
    --target "$TARGET" --release ${VIFAST_REPRO_CARGO_EXTRA:-} >"${VIFAST_REPRO_LOG:-/dev/null}" 2>&1 ||
    {
        tail -20 "${VIFAST_REPRO_LOG:-/dev/null}" >&2 2>/dev/null
        die "cargo build FAIL (bin=$BIN log=${VIFAST_REPRO_LOG:-/dev/null})"
    }
t1="$(date +%s)"

IMAGE="$CARGO_TARGET_DIR/$TARGET/release/$BIN"
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
if [ -f "$REL_IMAGE" ]; then
    REL_AFTER="$(sha_of "$REL_IMAGE")"
    [ "$REL_BEFORE" = "$REL_AFTER" ] ||
        die "A1-175: ảnh phát hành trong repo ĐÃ ĐỔI ($REL_BEFORE -> $REL_AFTER) — dừng, không dùng kết quả"
fi
if command -v shasum >/dev/null 2>&1; then
    SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
else
    SHA="$(sha256sum "$IMAGE" | awk '{print $1}')"
fi
SIZE="$(wc -c <"$IMAGE" | tr -d ' ')"
DIGEST="$(python3 "$HERE/../test/conformance/image_digest.py" "$IMAGE" 2>/dev/null || echo "build=? text=?")"
# KT-86: ảnh **phải** thật sự mang đúng danh tính vừa tính. Nếu build script bị cargo dùng lại
# (cache) ⇒ ảnh mang rev cũ ⇒ **dừng, không dùng kết quả** (đúng lớp lỗi KT-76: "build sạch" thật
# ra là no-op). Nhờ vậy `repro-build.sh` không bao giờ xuất một ảnh có nguồn gốc khác dòng REPRO.
# D2-127: danh tính **quyết định** là dấu vân tay build-inputs (16 hex), KHÔNG phải rev 12 hex.
vbrev_image_has "$IMAGE" "$VBREV_ALGO" "$BUILD_INPUTS" "$BUILD_DIRTY" ||
    die "KT-89: ảnh KHÔNG mang danh tính build algo=$VBREV_ALGO inputs=$BUILD_INPUTS dirty=$BUILD_DIRTY (build script bị cargo dùng lại?) — không dùng kết quả"
echo "REPRO machine=$(uname -sm | tr ' ' '-') root=$ROOT sha256=$SHA size=$SIZE wall_s=$((t1 - t0)) algo=$VBREV_ALGO inputs=$BUILD_INPUTS dirty=$BUILD_DIRTY rev=$BUILD_REV $DIGEST image=$IMAGE"
