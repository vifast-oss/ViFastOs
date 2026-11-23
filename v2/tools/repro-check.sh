#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-79 (OD-K6-3) — KIỂM build tái lập byte: dựng **hai bản sao y hệt** của cùng một cây nguồn
# ở **hai đường dẫn tuyệt đối khác nhau** (một ngắn, một dài), build ảnh sản phẩm bằng
# `repro-build.sh` cho từng bản, rồi so **sha256**. Cùng sha ⇒ PASS; khác ⇒ ĐỎ.
#
# Ca âm (bắt buộc theo OD-K6-3): `--negative` chạy đúng hai build đó nhưng **tắt**
# `--remap-path-prefix` ⇒ đường dẫn tuyệt đối bị nhúng vào `.rodata` ⇒ sha PHẢI khác. Script in
# `RED` và trả **1** — đó là hành vi đúng của ca âm (log: `v2/agent/JOURNAL/kt79-negative.log`).
#
# Dùng:
#   bash v2/tools/repro-check.sh [-q] [--arch riscv64|aarch64|x86_64] [--negative] [--keep]
#                               [--build-script PATH]
#   v2 check-repro [--negative]        (cùng nghĩa, qua `v2/bin/v2`)
#
# Thời gian đo được (máy ps, 12 core, 2026-11-17): 2 build lạnh ~2m10s ⇒ **KHÔNG** nằm trong
# `gate --fast` (chỉ ~60 s); dùng lệnh riêng, xem `v2/docs/k6-repro.md` §5.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd -P)"
BUILD="$HERE/repro-build.sh"
MODE="normal"
# **KT-346** — arch của ảnh sản phẩm canonical cần chứng minh tái lập byte (mặc định riscv64).
ARCH="riscv64"
KEEP=0
QUIET=0

die() { echo "repro-check: $*" >&2; exit 2; }

while [ $# -gt 0 ]; do
    case "$1" in
        --negative) MODE="negative"; shift ;;
        --arch) [ -n "${2:-}" ] || die "--arch cần tên"; ARCH="$2"; shift 2 ;;
        --keep) KEEP=1; shift ;;
        -q | --quiet) QUIET=1; shift ;;
        --build-script) [ -n "${2:-}" ] || die "--build-script cần đường dẫn"; BUILD="$2"; shift 2 ;;
        -h | --help)
            sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) die "tham số không biết: $1" ;;
    esac
done
[ -f "$BUILD" ] || die "không thấy $BUILD"

# ---- hai gốc: ngắn và dài (> 48 ký tự) để đường dẫn tuyệt đối khác nhau rõ rệt ----------------
WORK="$(mktemp -d "${TMPDIR:-/tmp}/kt79-repro.XXXXXXXX")" || die "mktemp"
case "$ARCH" in
    riscv64) TARGET_TRIPLE="riscv64gc-unknown-none-elf" ; BIN_NAME="vifast-kbin-riscv64" ;;
    aarch64) TARGET_TRIPLE="aarch64-unknown-none" ; BIN_NAME="vifast-kbin-aarch64" ;;
    x86_64) TARGET_TRIPLE="x86_64-unknown-none" ; BIN_NAME="vifast-kbin-x86_64" ;;
    *) die "arch không biết: $ARCH (dùng riscv64|aarch64|x86_64)" ;;
esac
RELEASE_IMAGE="$REPO_ROOT/v2/kernel/target/$TARGET_TRIPLE/release/$BIN_NAME"
VIFAST_REPRO_BUILD_ARGS="--arch $ARCH ${VIFAST_REPRO_BUILD_ARGS:-}"
export VIFAST_REPRO_BUILD_ARGS
sha_of() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'; else sha256sum "$1" | awk '{print $1}'; fi; }
# A1-175: build canonical **không bao giờ** được đổi ảnh phát hành trong repo. Ghi sha hiện tại
# (nếu có) và so lại ở cuối; đổi ⇒ FAIL (fail-closed).
RELEASE_BEFORE=""
[ -f "$RELEASE_IMAGE" ] && RELEASE_BEFORE="$(sha_of "$RELEASE_IMAGE")"
if [ -n "$RELEASE_BEFORE" ]; then
    echo "# repro-check: ảnh phát hành (không được đổi) sha256=$RELEASE_BEFORE"
fi
# Ca âm của luật A1-175: trỏ target dir vào trong cây nguồn ⇒ repro-build phải từ chối, không build.
if bash "$BUILD" --root "$REPO_ROOT" --target-dir "$REPO_ROOT/v2/kernel/target" >/dev/null 2>"$WORK/target-guard.err"; then
    echo "FAIL: A1-175 — repro-build đã chấp nhận target dir nằm trong cây nguồn (phải fail-closed)"
    exit 1
fi
grep -q "A1-175" "$WORK/target-guard.err" || {
    echo "FAIL: A1-175 — repro-build từ chối nhưng không nói lý do A1-175: $(cat "$WORK/target-guard.err")"
    exit 1
}
echo "# repro-check: ca âm A1-175 OK — target dir trong cây nguồn bị từ chối (không build)"
SHORT="$WORK/repro-a"
LONG="$WORK/repro-a-very-long-directory-name-for-absolute-path-embedding-0123456789"
mkdir -p "$SHORT" "$LONG" || die "mkdir"

cleanup() {
    if [ "$KEEP" = "1" ]; then
        echo "# repro-check: giữ $WORK"
    else
        rm -rf "$WORK"
    fi
}
trap cleanup EXIT

# Bản sao y hệt hai lần của cùng một cây nguồn. Chép **v2/** (trừ mọi `target/`) +
# `rust-toolchain.toml` + `.cargo/`; `v2/Cargo.toml` là workspace của các crate `v2/lib/*`
# (`version.workspace = true`) nên **phải** có, thiếu là cargo báo "error inheriting `edition`".
copy_tree() {
    local dst="$1"
    mkdir -p "$dst"
    (
        cd "$REPO_ROOT" || exit 1
        set --
        [ -f rust-toolchain.toml ] && set -- "$@" rust-toolchain.toml
        [ -d .cargo ] && set -- "$@" .cargo
        # Chỉ **đầu vào build** (không chép v2/test/boot — vùng của task khác đang sửa song song;
        # ảnh sản phẩm không cần các script đó). **KT-346**: `v2/user` là nguồn userland sản phẩm
        # (build.rs của cả ba kbin đọc nó) nên **phải** có trong bản sao; `v2/rust-toolchain.toml`
        # là tệp ghim toolchain của dòng OS (nằm trong `VBREV_INPUT_FILES`).
        tar -cf - --exclude target --exclude '*/target' "$@" \
            v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml v2/kernel v2/lib v2/user \
            v2/tools v2/test/conformance
    ) | (
        cd "$dst" || exit 1
        tar -xf -
    ) || die "copy cây -> $dst FAIL"
    # **KT-346** — mỗi bản sao phải là **repo git thật**: dấu vân tay build-inputs băm blob **ở
    # HEAD** (`v2/test/boot/build-rev.sh`), nên bản sao **không có `.git`** chỉ sinh được
    # `inputs=unknown dirty=1` — đúng lỗ hổng "bằng chứng yếu" mà KT-346 đóng. `git init` +
    # commit nội dung vừa chép cho bản sao **danh tính riêng**: hai bản sao cùng nội dung ⇒ cùng
    # blob OID ⇒ cùng dấu vân tay (và `dirty=0`), nên ảnh phải mang `inputs` **thật**.
    (
        cd "$dst" || exit 1
        git init -q . || exit 1
        git config user.email "repro-check@example.invalid" || exit 1
        git config user.name "repro-check" || exit 1
        git config commit.gpgsign false || exit 1
        git add -A || exit 1
        GIT_AUTHOR_DATE="1970-01-01T00:00:00Z" GIT_COMMITTER_DATE="1970-01-01T00:00:00Z" \
            git commit -qm "baseline (repro-check)" || exit 1
    ) || die "tạo repo git cho bản sao -> $dst FAIL"
}

copy_tree "$SHORT"
copy_tree "$LONG"
# Hai bản sao phải giống nhau từng byte (nếu không, "cùng mã nguồn" là giả định sai).
# **KT-346** — bỏ `.git/` khỏi phép so: mỗi bản sao có **repo git riêng** (để danh tính có
# `dirty=0`/`inputs` thật); nội dung `.git` (index/log/reflog) không phải đầu vào build.
DIFF_FILES="$(cd "$SHORT" && find . -path ./.git -prune -o -type f -print | LC_ALL=C sort)"
DIFF_FILES_LONG="$(cd "$LONG" && find . -path ./.git -prune -o -type f -print | LC_ALL=C sort)"
[ "$DIFF_FILES" = "$DIFF_FILES_LONG" ] || die "hai bản sao khác danh sách tệp"
SAME_CONTENT=1
while IFS= read -r f; do
    cmp -s "$SHORT/$f" "$LONG/$f" || {
        SAME_CONTENT=0
        echo "# repro-check: khác nội dung: $f" >&2
        break
    }
done <<EOF
$DIFF_FILES
EOF
[ "$SAME_CONTENT" = "1" ] || die "hai bản sao khác nội dung"
FILES_N="$(printf '%s\n' "$DIFF_FILES" | grep -c . )"

HEAD_SHA="$(cd "$REPO_ROOT" && git rev-parse HEAD 2>/dev/null || echo '?')"
DIRTY_N="$(cd "$REPO_ROOT" && git status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
if [ "$QUIET" = "0" ]; then
    echo "# repro-check: mode=$MODE arch=$ARCH head=$HEAD_SHA dirty_paths=$DIRTY_N files=$FILES_N"
    echo "# repro-check: gốc ngắn = $SHORT"
    echo "# repro-check: gốc dài  = $LONG"
fi

BREAK_ENV=""
[ "$MODE" = "negative" ] && BREAK_ENV="VIFAST_REPRO_BREAK=1"
run_one() {
    local root="$1" tdir="$2"
    env $BREAK_ENV bash "$BUILD" --root "$root" --target-dir "$tdir" ${VIFAST_REPRO_BUILD_ARGS:-} 2>&1 |
        grep '^REPRO '
}

LOG_A="$WORK/a.log"
LOG_B="$WORK/b.log"
run_one "$SHORT" "$WORK/target-a" >"$LOG_A" || die "build 1 FAIL (xem $WORK/a.log)"
run_one "$LONG" "$WORK/target-b" >"$LOG_B" || die "build 2 FAIL (xem $WORK/b.log)"
[ -s "$LOG_A" ] || die "build 1 không in dòng REPRO"
[ -s "$LOG_B" ] || die "build 2 không in dòng REPRO"

SHA_A="$(sed -n 's/.* sha256=\([0-9a-f]\{64\}\).*/\1/p' "$LOG_A")"
SHA_B="$(sed -n 's/.* sha256=\([0-9a-f]\{64\}\).*/\1/p' "$LOG_B")"
SIZE_A="$(sed -n 's/.* size=\([0-9][0-9]*\).*/\1/p' "$LOG_A")"
SIZE_B="$(sed -n 's/.* size=\([0-9][0-9]*\).*/\1/p' "$LOG_B")"
DIG_A="$(sed -n 's/.* \(build=[0-9a-f]* text=[0-9]*\).*/\1/p' "$LOG_A")"
DIG_B="$(sed -n 's/.* \(build=[0-9a-f]* text=[0-9]*\).*/\1/p' "$LOG_B")"
MACHINE="$(uname -sm)"
# cargo/rustc có thể không nằm trên PATH (máy ps: ~/.cargo/bin) — giống repro-build.sh.
if ! command -v rustc >/dev/null 2>&1; then
    for c in "$HOME/.cargo/bin" "/usr/local/cargo/bin" "/opt/homebrew/bin"; do
        [ -x "$c/rustc" ] && { PATH="$c:$PATH"; export PATH; break; }
    done
fi

if [ -n "$RELEASE_BEFORE" ]; then
    RELEASE_AFTER="$(sha_of "$RELEASE_IMAGE")"
    if [ "$RELEASE_BEFORE" != "$RELEASE_AFTER" ]; then
        echo "FAIL: A1-175 — ảnh phát hành trong repo bị build repro ghi đè ($RELEASE_BEFORE -> $RELEASE_AFTER)"
        exit 1
    fi
    echo "# repro-check: ảnh phát hành KHÔNG đổi (sha256=$RELEASE_AFTER)"
fi
echo "# repro-check: máy [$MACHINE] | $(rustc -V 2>/dev/null || echo 'rustc ?') | toolchain=$(rustup show active-toolchain 2>/dev/null | tr '\n' ' ')"
echo "# repro-check: [1] $SHORT"
echo "#              sha256=$SHA_A size=$SIZE_A $DIG_A"
echo "# repro-check: [2] $LONG"
echo "#              sha256=$SHA_B size=$SIZE_B $DIG_B"

if [ "$MODE" = "negative" ]; then
    if [ "$SHA_A" = "$SHA_B" ]; then
        echo "# repro-check: KHÔNG ĐẠT ca âm — tắt remap mà sha vẫn giống ⇒ kiểm tra không chứng minh được gì"
        exit 1
    fi
    echo "RED: ca âm đạt — tắt --remap-path-prefix, hai đường dẫn cho sha KHÁC ($SHA_A vs $SHA_B)"
    echo "# đây là kết quả MONG ĐỢI của '--negative'; lần chạy thường (không cờ) mới phải xanh"
    exit 1
fi

if [ "$SHA_A" != "$SHA_B" ]; then
    echo "FAIL: cùng mã nguồn, hai đường dẫn khác nhau ⇒ sha KHÁC"
    [ "$KEEP" = "1" ] || echo "# dùng --keep để giữ cây build và so tiếp; xem v2/docs/k6-repro.md §3"
    exit 1
fi
[ "$SIZE_A" = "$SIZE_B" ] || {
    echo "FAIL: sha giống nhưng size khác ($SIZE_A vs $SIZE_B) — không thể"
    exit 1
}
echo "PASS: build tái lập byte — 2 đường dẫn tuyệt đối khác nhau, cùng sha256=$SHA_A size=$SIZE_A ($DIG_A)"
echo "# OD-K6-3 cần thêm vế **2 máy**: chạy cùng lệnh này trên máy thứ hai rồi so sha256 (k6-repro.md §6)"
exit 0
