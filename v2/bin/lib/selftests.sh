#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-248 mục 2) — **self-test công cụ T0** của quy trình Agent 2.
#
# `v2 t0 --selftest` gọi `run_t0_tooling`: chạy self-test của các công cụ **không** nằm trong
# `v2 check`/`gate --fast` (vì chúng là công cụ quy trình, không phải mã sản phẩm), nhưng vẫn phải
# có cổng gọi (KT-134 `check-selftest-wiring.py` quét `v2/bin/v2` + `v2/bin/lib/*.sh`):
#   * `v2/tools/measure-lock.sh --selftest`  — khoá máy đo + `measure_lock_is_held` (lớp 2).
#   * `v2/tools/wt.sh --selftest`            — 4 slot worktree + `target-task/slot-<n>` + `df`.
#   * `v2/test/boot/lib/env.sh --selfcheck`  — LOGDIR/SOCK ngắn (< 104 B) kể cả `$TMPDIR` dài.
#   * `v2/bin/vifast-agentctl lint --selftest` / `sync-queue --selftest` — sổ QUEUE/STATE.
#
# Không QEMU, không build kernel. Trả 0 ⇔ mọi công cụ PASS.

run_t0_tooling() {
    local rc=0 n=0
    _t0_tool() { # <nhãn> <lệnh...>
        local label="$1"
        shift
        n=$((n + 1))
        echo "--- T0 tooling $n: $label"
        if "$@"; then
            echo "--- T0 tooling $n: OK"
        else
            echo "--- T0 tooling $n: FAIL" >&2
            rc=1
        fi
    }
    _t0_tool "measure-lock (khoá máy đo + is_held)" bash "$V2_ROOT/tools/measure-lock.sh" --selftest
    # **KT-403 (D2-259)** — 4 công cụ mới: luật ratchet B1.1, manifest đột biến, chuẩn telemetry,
    # giải mã trace 32 B. Mỗi công cụ có **đỏ-trước cho chính nó** (D2-113F).
    local v2py=""
    v2py="$(find_python 2>/dev/null || true)"
    if [ -n "$v2py" ]; then
        _t0_tool "B1.1 ratchet (đỏ-trước 5 ca: dep mới/tệp driver mới)" \
            "$v2py" "$V2_ROOT/tools/check-api-boundary.py" --root "$REPO_ROOT" --ratchet-selftest
        _t0_tool "v2 mutants (manifest 24 + đỏ-trước 7 ca)" \
            "$v2py" "$V2_ROOT/tools/mutants.py" --root "$REPO_ROOT" --selftest
        _t0_tool "telemetry key=value (14 ca, không nhạy thứ tự trường)" \
            "$v2py" "$V2_ROOT/tools/telemetry.py" --selftest
        _t0_tool "vifast-trace (bản ghi 32 B + header VFT1)" \
            "$v2py" "$V2_ROOT/tools/vifast-trace.py" --selftest
        _t0_tool "vifast-bench (schema CSV + -icount, không QEMU)" \
            bash "$V2_ROOT/bin/vifast-bench" --selftest
    else
        echo "--- T0 tooling KT-403: BỎ QUA (không thấy python ≥3.11 có tomllib)" >&2
        rc=1
    fi
    _t0_tool "wt.sh (4 slot worktree + cache ấm + df)" bash "$V2_ROOT/tools/wt.sh" --selftest
    # **KT-348a** — bất biến "boot bị tiến trình NGOÀI giết không được chấm là lỗi kernel":
    # `qg_boot` đọc mã thoát, `128..191` ⇒ rc=126 + `ext_sig=`/`killer=`; soak phải TỪ CHỐI ĐO (D2-116).
    _t0_tool "guard_extsig (phân loại SIGTERM ngoài, rc=126)" bash "$V2_ROOT/test/boot/guard_extsig_selftest.sh" 
    _t0_tool "boot/lib/env.sh (LOGDIR + SOCK ngắn)" bash "$V2_ROOT/test/boot/lib/env.sh" --selfcheck
    _t0_tool "boot/lib/harness-scan.sh (23 marker D2-08 + ca âm)" bash "$V2_ROOT/test/boot/lib/harness-scan.sh" --selftest
    _t0_tool "boot/lib/budget-common.sh (3 ngưỡng + parse + nhãn)" bash "$V2_ROOT/test/boot/lib/budget-common.sh" --selftest
    _t0_tool "boot/lib/soak-common.sh (vòng soak + không retry)" bash "$V2_ROOT/test/boot/lib/soak-common.sh" --selftest
    _t0_tool "agentctl lint" bash "$V2_ROOT/bin/vifast-agentctl" lint --selftest
    _t0_tool "agentctl sync-queue" bash "$V2_ROOT/bin/vifast-agentctl" sync-queue --selftest
    if [ "$rc" -eq 0 ]; then
        echo "# T0 tooling: PASS ($n/$n)"
    else
        echo "# T0 tooling: FAIL" >&2
    fi
    return "$rc"
}
