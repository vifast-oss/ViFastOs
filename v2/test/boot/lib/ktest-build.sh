#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-411b** (D2-259 mục 2) — khoá + phát hành ảnh kiểm dùng chung cho 5 runner.
#
# Vì sao cần: sau khi gom 14 `[[bin]]` → 2, **mọi** suite của ảnh kiểm dựng ra **cùng một tên
# tệp** `target/<target>/release/vifast-kbin-ktest` rồi `cp` ra tên ảnh riêng. Trong T2 các làn
# `sau` (ktest.sh), `sau5` (net.sh), `sau6` (input.sh/gpu.sh) chạy **song song** và cùng ghi tệp
# đó ⇒ có khoảng đua giữa "cargo trả về" và "cp": làn khác kịp dựng suite của nó thì `cp` của làn
# này chép **nhầm ảnh**. Trước KT-411b không có đua vì mỗi suite một tên bin riêng.
#
# Cách chặn (không nới cổng nào, không đổi ảnh):
#   * `ktest_build_lock` — khoá `mkdir` (bash 3.2/macOS **không** có `flock`), trần chờ 900 s;
#     hết hạn ⇒ **ĐỎ** (không treo, không bỏ qua).
#   * `ktest_publish` — `cp` **nguyên tử**: ghi tệp tạm cùng thư mục rồi `mv` (đổi tên trong cùng
#     FS là nguyên tử) ⇒ người đọc song song (`guard_selftest.sh`, `conformance/run.sh`) không bao
#     giờ thấy tệp ghi dở.
#   * `ktest_build_suite` — gói **build + phát hành trong cùng một khoá**.
#
# Không đổi: bộ feature của từng suite, tên ảnh phát ra, `expected.txt`, `budget.sh`.

# Khoá đặt cạnh target dir (mỗi slot/worktree một khoá; `CARGO_TARGET_DIR` là cùng nguồn với
# `IMG_DIR` của các runner).
KTEST_BUILD_LOCK="${KTEST_BUILD_LOCK:-${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/.ktest-build.lock}"
KTEST_BUILD_LOCK_WAIT_DS="${KTEST_BUILD_LOCK_WAIT_DS:-9000}" # 900 s
KTEST_BUILD_LOCK_STALE="${KTEST_BUILD_LOCK_STALE:-600}"     # 600 s (khoá không có `pid`)

# Dọn khoá **mồ côi** (tiến trình giữ khoá đã chết — Ctrl-C/SIGKILL giữa lúc build). Không có
# bước này thì một khoá sót làm mọi lượt T2 sau chờ 900 s rồi ĐỎ. Chỉ xoá đúng thư mục khoá đã
# tính (`*/.ktest-build.lock`) — không xoá đường dẫn nào khác.
ktest_build_reap() {
    local pid="" age="" mtime=""
    pid="$(cat "$KTEST_BUILD_LOCK/pid" 2>/dev/null || true)"
    if [ -n "$pid" ] && ! kill -0 "$pid" 2>/dev/null; then
        echo "ktest-build-lock: dọn khoá của pid $pid (tiến trình đã chết)" >&2
        case "$KTEST_BUILD_LOCK" in
            */.ktest-build.lock) rm -rf "$KTEST_BUILD_LOCK" ;;
        esac
        return 0
    fi
    # Không có `pid` (chết giữa `mkdir` và ghi `pid`) ⇒ dùng tuổi khoá, cùng khuôn `measure-lock.sh`.
    mtime="$(stat -f %m "$KTEST_BUILD_LOCK" 2>/dev/null || stat -c %Y "$KTEST_BUILD_LOCK" 2>/dev/null || echo 0)"
    if [ "${mtime:-0}" -gt 0 ]; then
        age=$(($(date +%s) - mtime))
        if [ "$age" -gt "$KTEST_BUILD_LOCK_STALE" ]; then
            echo "ktest-build-lock: dọn khoá mồ côi (${age}s > ${KTEST_BUILD_LOCK_STALE}s)" >&2
            case "$KTEST_BUILD_LOCK" in
                */.ktest-build.lock) rm -rf "$KTEST_BUILD_LOCK" ;;
            esac
        fi
    fi
    return 0
}

ktest_build_lock() {
    local n=0
    while ! mkdir "$KTEST_BUILD_LOCK" 2>/dev/null; do
        ktest_build_reap
        n=$((n + 1))
        if [ "$n" -ge "$KTEST_BUILD_LOCK_WAIT_DS" ]; then
            echo "ktest-build-lock: hết hạn chờ khoá $KTEST_BUILD_LOCK ($((KTEST_BUILD_LOCK_WAIT_DS / 10)) s)" >&2
            return 1
        fi
        sleep 0.1
    done
    printf '%s\n' "$$" >"$KTEST_BUILD_LOCK/pid" 2>/dev/null || true
    return 0
}

ktest_build_unlock() {
    rm -f "$KTEST_BUILD_LOCK/pid" 2>/dev/null || true
    rmdir "$KTEST_BUILD_LOCK" 2>/dev/null || true
}

# `cp` nguyên tử: tệp tạm **cùng thư mục** (cùng FS ⇒ `mv` là `rename(2)`, nguyên tử).
ktest_publish() { # $1=nguồn $2=đích
    local src="$1" dst="$2" tmp="$2.tmp.$$"
    if ! cp "$src" "$tmp"; then
        rm -f "$tmp"
        return 1
    fi
    if ! mv "$tmp" "$dst"; then
        rm -f "$tmp"
        return 1
    fi
    return 0
}

# `ktest_build_suite <suite> <log> <đích> <manifest> <target>`: build **một** suite rồi phát hành
# ra `<đích>` — hai bước nằm trong **cùng** một khoá. Trả 0 = ảnh đã ở `<đích>`.
ktest_build_suite() {
    local suite="$1" blog="$2" dst="$3" manifest="$4" target="$5" src="" rc=0
    src="$(dirname "$dst")/vifast-kbin-ktest"
    ktest_build_lock || return 1
    if ! cargo build --manifest-path "$manifest" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-ktest --features "ktest,$suite" \
        --target "$target" --release >"$blog" 2>&1; then
        ktest_build_unlock
        return 1
    fi
    ktest_publish "$src" "$dst" || rc=1
    ktest_build_unlock
    return "$rc"
}
