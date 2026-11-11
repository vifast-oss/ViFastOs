#!/bin/sh
# Chạy toàn bộ cổng chất lượng của ViFastOs trong một lệnh.
#
#   ./scripts/gate.sh            # build + QEMU 3/3×3 + persist + harness + docs + fmt
#   ./scripts/gate.sh --quick    # bỏ QEMU/Persist (chỉ build + harness + docs + fmt)
#   ./scripts/gate.sh --with-tcp # thêm kiểm tra TCP echo qua hostfwd
#   ./scripts/gate.sh --with-input  # thêm kiểm tra bàn phím virtio
#
# Trả mã 0 khi mọi bước xanh. Dùng cho các vòng phát triển liên tục và cho
# các đợt soak dài: mỗi bước in PASS/FAIL rõ ràng.
set -u

QUICK=0
SMOKE=0
TCP=0
INPUT=0
for arg in "$@"; do
    case "$arg" in
        --smoke) SMOKE=1 ;;
        --quick) QUICK=1 ;;
        --with-tcp) TCP=1 ;;
        --with-input) INPUT=1 ;;
    esac
done

cd "$(dirname "$0")/.." || exit 2
PATH="$HOME/.cargo/bin:$PATH"
export PATH
FAILED=""

step() {
    name="$1"
    shift
    printf '\n=== %s ===\n' "$name"
    if "$@"; then
        printf '[gate] %s: PASS\n' "$name"
    else
        printf '[gate] %s: FAIL\n' "$name"
        FAILED="$FAILED $name"
    fi
}

if [ "$SMOKE" = "1" ]; then
    # r322: vòng lặp NHANH (~90 s) để dùng sau mỗi lần sửa — không thay cổng
    # đầy đủ. Gồm: build, host test kernel, đặc tả harness, MỘT boot QEMU, fmt.
    # Chỉ build khi mã nguồn đã đổi (BUILD_ID tính giống build.sh nhưng chỉ
    # trên crates/scripts/Cargo.*). Nhờ vậy chạy --smoke liên tiếp rất nhanh.
    if command -v sha256sum >/dev/null 2>&1; then
        _h8() { sha256sum | cut -c1-8; }
    else
        _h8() { shasum -a 256 | cut -c1-8; }
    fi
    want_id="$(git rev-parse --short HEAD 2>/dev/null || printf nogit0)-$(git diff HEAD -- crates scripts Cargo.toml Cargo.lock 2>/dev/null | _h8)"
    have_id="$(cat target/vifast-build-id.txt 2>/dev/null || printf none)"
    if [ "$want_id" = "$have_id" ] && [ -f target/vifast-build-id.txt ]; then
        printf '\n=== build ===\n[gate] build: SKIP (BUILD_ID không đổi: %s)\n' "$want_id"
    else
        step "build" ./scripts/build.sh
    fi
    step "kernel-test" cargo test -p vifast-kernel
    step "harness" python3 scripts/test-harness-spec.py
    step "qemu-1" python3 scripts/test-qemu.py --arch riscv64
    step "fmt" cargo fmt --check
    printf '\n=== gate summary (smoke) ===\n'
    if [ -n "$FAILED" ]; then
        printf 'FAILED steps:%s\n' "$FAILED"
        exit 1
    fi
    printf 'all steps passed\n'
    exit 0
fi
step "build" ./scripts/build.sh
# Host test của kernel chạy tường minh (audit #3 P1-4): bước fuzz chỉ chạy
# --ignored, nên nếu chỉ dựa vào đó thì các test thường không nằm trong cổng.
step "kernel-test" cargo test -p vifast-kernel
# Fuzz chỉ có nghĩa khi đúng 2 test #[ignore] còn tồn tại; đổi tên chúng thì
# "0 passed" vẫn xanh, nên chặn bằng cách đếm số test liệt kê được.
# 289.5: sàn test là số ĐO ĐƯỢC (scripts/test-floor.json), không phải hằng số
# viết tay. Trước đây chốt cứng `-ge 138` trong khi thực tế 245 — xoá 107 test
# vẫn xanh. Nay giảm bất kỳ crate nào là ĐỎ.
step "tests-present" python3 scripts/test-floor.py --check
step "fuzz" cargo test -p vifast-kernel --release -- --ignored

if [ "$QUICK" -eq 0 ]; then
    QEMU_ARGS=""
    if [ "$TCP" -eq 1 ]; then
        QEMU_ARGS="$QEMU_ARGS --tcp-check"
    fi
    if [ "$INPUT" -eq 1 ]; then
        QEMU_ARGS="$QEMU_ARGS --input-check"
    fi
    step "qemu" sh -c "python3 scripts/test-qemu.py --repeat 3 $QEMU_ARGS"
    # r259 (T21/D19): bất biến sched ở ranh giới user — bản xanh phải in
    # BOUNDARY OK, bản mutation (D18) phải in đúng một BOUNDARY BAD.
    step "boundary" sh -c "python3 scripts/test-qemu.py --feature test-check-boundary && python3 scripts/test-qemu.py --feature test-check-boundary-mutation"
    step "persist" python3 scripts/test-persist.py
    step "docs" sh -c 'python3 scripts/gen-docs.py >/dev/null && python3 scripts/check-docs.py'
fi
step "harness" python3 scripts/test-harness-spec.py
step "fmt" cargo fmt --check

printf '\n=== gate summary ===\n'
if [ -n "$FAILED" ]; then
    printf 'FAILED steps:%s\n' "$FAILED"
    exit 1
fi
if [ "$QUICK" -eq 1 ]; then
    printf 'all executed steps passed (--quick: QEMU and persistence were SKIPPED)\n'
    exit 0
fi
printf 'all steps passed\n'
exit 0
