#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — **hàm bước của `v2 check`** (fmt, clippy, luật, checker, ABI/scn/report…)
# và `check_k6_rules` (5 luật máy). Thân hàm **nguyên văn** bản cũ trong `v2/bin/v2`; tách tệp
# không đổi id bước, thứ tự, làn hay phép kiểm nào.
# ---------------------------------------------------------------- các bước (hàm, không tham số)
#
# Mỗi bước là **một hàm** để bảng làn chỉ cần giữ tên hàm — không `eval`, không nối chuỗi lệnh
# (đường dẫn có dấu cách vẫn đúng). Thân hàm là **nguyên văn** lệnh cũ.

step_py() { # <script> [tham số...] — chặn sớm khi thiếu python ≥3.11 (giữ nguyên thông điệp cũ)
    if [ -z "${V2_PY:-}" ]; then
        echo "cần python ≥3.11 có tomllib (đã thử python3.12/3.11/3)"
        return 1
    fi
    "$V2_PY" "$@"
}

# --- `check` ---
step_fmt() {
    # `cargo fmt --all` **chỉ** phủ crate của workspace `v2/kernel` — nó **không** thấy cây
    # `v2/user/` (userland biên dịch bằng `$RUSTC` trực tiếp trong từng `build.rs`, không phải
    # crate cargo). **D2-255 mục 3** yêu cầu kiểm luôn cây đó ⇒ gọi `rustfmt --check` tường minh.
    # Đo trước khi thêm bước này: `v2/user/**/*.rs` lệch **67 dòng** `rustfmt --edition 2021`.
    "$CARGO" fmt --all --check --manifest-path "$MANIFEST" || return 1
    local f
    for f in "$V2_ROOT"/user/runtime/*.rs "$V2_ROOT"/user/programs/*.rs "$V2_ROOT"/user/rt/*.rs; do
        [ -f "$f" ] || continue
        "$(dirname "$CARGO")/rustfmt" --edition 2021 --check "$f" || {
            echo "rustfmt lệch: ${f#"$V2_ROOT"/} (chạy: rustfmt --edition 2021 $f)" >&2
            return 1
        }
    done
}
step_clippy() { "$CARGO" clippy --manifest-path "$MANIFEST" --workspace --all-targets -- -D warnings; }
step_rules() { bash "$V2_ROOT/bin/v2" rules --root "$V2_ROOT"; }
step_selftest_rules() { bash "$V2_ROOT/test/rules/run.sh"; }
# **KT-331 / D2-230** — ca âm ĐỎ **đúng mã** của máy kiểm tên tệp Windows/NTFS + ca
# dương XANH + ca âm/dương end-to-end của hook `pre-commit` (repo tạm, không QEMU).
step_selftest_filenames() { step_py "$V2_ROOT/tools/check-filenames.py" --selftest; }
# **KT-333b (D2-21, D2-37(c))** — cổng giấy phép: allowlist `deny.toml` (QD §9.3) + kiểm
# `Cargo.lock` ↔ `cargo metadata` + nguồn registry + `THIRD-PARTY.md` khớp bản sinh (không drift).
step_licenses() { step_py "$V2_ROOT/tools/check-licenses.py" --root "$REPO_ROOT" --check; }
step_selftest_licenses() { step_py "$V2_ROOT/tools/check-licenses.py" --selftest; }
# `v2 nightly --selftest` (KT-333b): kế hoạch + đọc/sinh STATUS.md; KHÔNG chạy sim (nên gọi được
# trong `v2 check`, giữ luật KT-134 "script tự kiểm phải có cổng gọi").
step_selftest_nightly() { bash "$V2_ROOT/bin/v2" nightly --selftest; }
# **KT-333c** — tự kiểm khoá máy đo (9 ca: giữ/giữ lại ⇒ TỪ CHỐI, stale pid, quá hạn, release sai
# pid, release đúng pid, owner hỏng, `off`, release khi `off`). Thuần shell, không QEMU.
step_measure_lock() { bash "$V2_ROOT/tools/measure-lock.sh" --selftest; }
step_k6() { check_k6_rules; }
step_build_inputs() { bash "$V2_ROOT/test/boot/build-rev.sh" --selftest; }
step_build_identity() { bash "$V2_ROOT/test/boot/build-identity.sh" --selftest; }
step_t26() { bash "$V2_ROOT/test/boot/t26.sh" --selftest; }
step_algo_pin() { bash "$V2_ROOT/tools/check-algo-pin.sh" --strict; }
step_selftest_algo_pin() { bash "$V2_ROOT/tools/check-algo-pin.sh" --selftest; }
step_shell_edge() { bash "$V2_ROOT/test/boot/shell-edge.sh" --selftest; }
step_budget() { bash "$V2_ROOT/test/boot/budget.sh" --selftest; }
step_counter_evidence() { bash "$V2_ROOT/test/rules/counter-evidence.sh" --selftest; }
step_task_target() { bash "$V2_ROOT/tools/task-target.sh" --selftest; }
step_selftest_l1_tiers() { bash "$V2_ROOT/tools/l1-tier.sh" --selftest; }
step_scope_table() { step_py "$V2_ROOT/tools/check-scope-table.py" --root "$REPO_ROOT"; }
step_selftest_scope_table() { step_py "$V2_ROOT/tools/check-scope-table.py" --selftest; }
step_wiring() { step_py "$V2_ROOT/tools/check-selftest-wiring.py" --selftest; }
step_wiring_standalone() { step_py "$V2_ROOT/tools/check-selftest-wiring.py" --standalone-red; }
# **KT-333e (D2-247)** — một lượt quét cho **cả** chế độ thường và `--strict` (trước là hai bước
# 20/22, hai lượt đọc cùng cây). `--combined` chạy **đủ** hai bộ luật trong một lượt và in hai bảng
# `strict=0`/`strict=1`; đỏ nếu **một trong hai** đỏ. Bản riêng (`step_wake_paths`/`--strict` vẫn
# gọi được thủ công) giữ nguyên để so sánh/đối chứng.
step_wake_paths_combined() { step_py "$V2_ROOT/tools/check-wake-paths.py" --combined; }
step_selftest_wake_paths() { step_py "$V2_ROOT/tools/check-wake-paths.py" --selftest; }
step_wake_depinfo() { step_py "$V2_ROOT/tools/check-wake-depinfo.py" --root "$REPO_ROOT"; }
step_selftest_wake_depinfo() { step_py "$V2_ROOT/tools/check-wake-depinfo.py" --selftest; }
step_api_boundary() { step_py "$V2_ROOT/tools/check-api-boundary.py" --root "$REPO_ROOT"; }
step_test_teeth() { step_py "$V2_ROOT/tools/check-test-teeth.py"; }
step_selftest_test_teeth() { step_py "$V2_ROOT/tools/check-test-teeth.py" --selftest; }
# **KT-345a**: chạy kèm `--selftest` của máy kiểm (D2-242 nhóm 8/9). Không thêm bước mới —
# `check-selftest-wiring.py` (KT-134) đòi **lời gọi thật** kèm cờ cho mọi script tự kiểm.
step_abi() {
    step_py "$V2_ROOT/test/abi/check_abi.py" --file "$REPO_ROOT/spec/abi.toml" &&
        step_py "$V2_ROOT/test/abi/check_abi.py" --selftest
}
step_golden() { (cd "$REPO_ROOT" && step_py v2/test/trace/gen_golden.py --check); }
step_scn() { (cd "$REPO_ROOT" && step_py v2/test/scn/check_scn.py); }
step_scn_fixtures() { (cd "$REPO_ROOT" && PYTHON="$V2_PY" bash v2/test/scn/run_fixtures.sh); }
step_report() { (cd "$REPO_ROOT" && step_py v2/test/report/check_report.py --all); }
step_report_fixtures() { (cd "$REPO_ROOT" && PYTHON="$V2_PY" bash v2/test/report/run_fixtures.sh); }
step_abi_reserved() {
    step_py "$V2_ROOT/test/abi/check_reserved.py" --file "$REPO_ROOT/spec/abi.toml" &&
        step_py "$V2_ROOT/test/abi/check_reserved.py" --selftest
}
step_abi_fixtures() { (cd "$REPO_ROOT" && PYTHON="$V2_PY" bash v2/test/abi/run_fixtures.sh); }
step_gen() { (cd "$REPO_ROOT" && ./v2/bin/vifast-gen --check); }
step_gen_fixtures() { (cd "$REPO_ROOT" && bash v2/test/gen/run_fixtures.sh); }
step_idl() { (cd "$REPO_ROOT" && ./v2/bin/vifast-idl --check); }
step_idl_fixtures() { (cd "$REPO_ROOT" && bash v2/test/gen/run_idl_fixtures.sh); }
# **KT-345a (D2-242 nhóm 1)** — `spec/kernel.toml` ↔ mã, đối chiếu **theo tên hằng** (không phụ
# thuộc số dòng). Chạy **cả `--selftest`** ngay tại bước này: `check-selftest-wiring.py` (KT-134)
# đòi mọi script tự kiểm phải có **lời gọi thật** kèm cờ trong `v2/bin/v2`; chỉ gọi kiểm thường
# sẽ sinh `E_ORPHAN` ở step 19. Dùng `step_py` để chọn Python ≥3.11 (máy đo: `python3` = 3.9,
# **không** có `tomllib`).
step_kernel() {
    step_py "$V2_ROOT/test/kernel/check_kernel.py" &&
        step_py "$V2_ROOT/test/kernel/check_kernel.py" --selftest
}
step_build() { "$CARGO" build --manifest-path "$MANIFEST" --workspace; }

# KT-80 / OD-K6-5 — 5 luật máy; logic nằm ở `v2/tools/verify-numbers.py` (một công cụ dùng
# chung, không copy-paste 4 verifier cũ). `--law`/`--self-test`/`--root` chuyển tiếp nguyên văn.
check_k6_rules() {
    local py
    if ! py="$(find_python)"; then
        echo "check-rules: cần python ≥ 3.11 có tomllib (đã thử python3.12/3.11/3)" >&2
        return 1
    fi
    "$py" "$V2_ROOT/tools/verify-numbers.py" --root "$REPO_ROOT" "$@"
}
