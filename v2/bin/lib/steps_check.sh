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
    # `cargo fmt --all` của `$MANIFEST` **chỉ** phủ crate của workspace host `v2/` — nó **không**
    # thấy cây `v2/user/`. **D2-255 mục 3** yêu cầu kiểm luôn cây đó.
    #
    # **KT-404 (B4, D2-259)**: từ KT-404 `v2/user/` là **workspace Cargo thật** nên dùng
    # `cargo fmt --all` của chính nó (phủ `runtime/`, `rt/`, `programs/`) thay cho vòng
    # `rustfmt --check` liệt kê tay ba thư mục (vòng cũ phải sửa mỗi lần thêm thư mục).
    "$CARGO" fmt --all --check --manifest-path "$MANIFEST" || return 1
    "$CARGO" fmt --all --check --manifest-path "$V2_ROOT/user/Cargo.toml" || return 1
}

# **KT-404 (B4, D2-259)** — `v2/user/` là **workspace Cargo độc lập** (`vifast-urt` = shim ABI,
# `vifast-rt` = runtime `no_std + alloc`, `vifast-programs` = 6 `[[bin]]` chương trình U-mode).
#
# Vì sao bước này tồn tại: trước KT-404 cây `v2/user/` **không** nằm trong workspace cargo nào
# (ba `build.rs` gọi `$RUSTC` trực tiếp) ⇒ `clippy -D warnings` của cổng **chưa bao giờ** chạy
# trên nó, kể cả `runtime/lib.rs` có `unsafe` (D2-253) và `rt/lib.rs` có `#[panic_handler]`.
# Đo được khi mở bước này: **4 cảnh báo `-D warnings` có sẵn** (`clippy::write_with_newline` ở
# `rt/lib.rs`, 2 `unused_imports` `SYS_EXIT`, `clippy::unnecessary_cast` ở `shell.rs`) — đã sửa.
#
# Lint **cả 3 target bare-metal** với đúng bộ `--cfg` của **ảnh sản phẩm** (`shell_fs`,
# `shell_net`, `shell_exec`, `product_net`, `rt_alloc`) ⇒ lint đúng biến thể chạy thật, không
# lint bản test. `CARGO_TARGET_DIR` **riêng** (`<target>/user-workspace`) để không đụng target
# của workspace host và không làm bẩn phép đo nào.
step_user_workspace() {
    local user_manifest="$V2_ROOT/user/Cargo.toml" t
    local target_dir="${CARGO_TARGET_DIR:-$V2_ROOT/target}/user-workspace"
    local flags="--cfg rt_alloc --cfg shell_fs --cfg shell_net --cfg shell_exec --cfg product_net"
    for t in riscv64gc-unknown-none-elf aarch64-unknown-none x86_64-unknown-none; do
        RUSTFLAGS="$flags" CARGO_TARGET_DIR="$target_dir" \
            "$CARGO" clippy --manifest-path "$user_manifest" --workspace --target "$t" \
            -- -D warnings || return 1
    done
    # 8 ca host của lõi bump allocator (`v2/user/runtime/bump.rs`) — **cùng** lệnh với
    # `v2/test/boot/kt347-u1a.sh` (một nguồn ca, không chép lại). Vì sao `rustc --test` chứ
    # không `cargo test`: `bump` chỉ được biên dịch trong lib khi `--cfg rt_alloc`, nên một test
    # target của cargo phải `#[path]` trỏ vào `../bump.rs` — và `check-wake-paths.py` (luật
    # `E_STRICT_WAKE_INCLUDE`, KT-171 B6/B7) **fail-closed** với mọi `#[path]` trỏ ra ngoài
    # `v2/kernel`+`v2/sim`. Giữ đúng khuôn `kt347-u1a.sh` là cách duy nhất không nới luật đó.
    local rustc_bin
    rustc_bin="$(dirname "$CARGO")/rustc"
    [ -x "$rustc_bin" ] || rustc_bin="$(command -v rustc)"
    "$rustc_bin" --edition 2021 --test "$V2_ROOT/user/runtime/bump.rs" \
        -o "$target_dir/bump-tests" || return 1
    "$target_dir/bump-tests" || return 1
}
step_clippy() { "$CARGO" clippy --manifest-path "$MANIFEST" --workspace --all-targets -- -D warnings; }
# **KT-409 (D2-260 mục 3)** — `clippy -D warnings` cho **workspace kernel** (`v2/kernel/`).
#
# Vì sao bước này tồn tại: `step_clippy` ở trên chỉ lint `$MANIFEST` = `v2/Cargo.toml`, mà tệp đó
# có `exclude = ["kernel"]` ⇒ trước KT-409 **không cổng nào** (check/T1/T2) quét `clippy` cho 8
# crate kernel (~62 k LOC). `v2 t0` có quét nhưng T0 **không phải cổng** (AGENTS §6.1) nên lỗ
# hổng là thật: `D2-260` mục 3 đo được **15 lỗi `clippy` có sẵn** trong `vifast-core` (dọn ở
# KT-405) và 5+7+9 lỗi ở các crate `plat-*` (dọn ở chính task này).
#
# Chỉ **crate host-testable** — danh sách loại trừ là **đo được**, không phải suy đoán:
#   * `vifast-plat-riscv64`: asm RISC-V (`asm!("…", in("a7") …)`) ⇒ host báo
#     `invalid register a7: unknown register`;
#   * `vifast-kbin-riscv64`: cùng lý do asm RISC-V;
#   * `vifast-kbin-aarch64`: ảnh bare-metal ⇒ `unwinding panics are not supported without std` +
#     `duplicate lang item panic_impl`;
#   * `vifast-kbin-x86_64`: `global_asm!`/`boot` chỉ dùng được trên target x86_64 ⇒ host báo
#     `unused import` (lint sai bản chất, không phải lỗi mã).
# Dùng `--workspace --exclude` (không liệt kê `-p`) để crate host-testable **thêm sau** tự động
# được quét — hướng fail-closed, không phải danh sách phải nhớ cập nhật.
#
# Làn `main` (không phải làn nền): bước này **ghi** `$CARGO_TARGET_DIR` như `step_clippy`; làn nền
# chỉ chứa bước chỉ đọc (xem `lane.sh`).
#
# **(2) lint trên target thật của 3 crate arch** — phần host ở trên **không** thấy mã chỉ biên dịch
# được cho bare-metal. Đo được ngay trong task này: bỏ `use crate::asm;` khỏi
# `vifast-plat-x86_64/src/wx.rs` thì host **xanh** (import thừa) nhưng ảnh x86_64 **ĐỎ**
# `E0433: undeclared crate or module asm` — T1 bắt được ở bước QEMU, tức muộn và đắt. Lint theo
# target bắt ngay tại `check`. Đo trước khi nối: `plat-riscv64` **4** cảnh báo, `plat-x86_64` **4**,
# `plat-aarch64` **0** — đã dọn hết (xem `JOURNAL/kt409-hal-clippy.md` §8).
# `--lib` (không `--all-targets`): test của crate bare-metal cần `std` nên không dựng được cho
# target `*-unknown-none`; phần test đã do lượt host ở trên phủ.
step_clippy_kernel() {
    "$CARGO" clippy --manifest-path "$V2_ROOT/kernel/Cargo.toml" --workspace \
        --exclude vifast-plat-riscv64 \
        --exclude vifast-kbin-riscv64 \
        --exclude vifast-kbin-aarch64 \
        --exclude vifast-kbin-x86_64 \
        --all-targets -- -D warnings || return 1
    local t
    for t in riscv64gc-unknown-none-elf:riscv64 aarch64-unknown-none:aarch64 x86_64-unknown-none:x86_64; do
        "$CARGO" clippy --manifest-path "$V2_ROOT/kernel/Cargo.toml" \
            --target "${t%%:*}" -p "vifast-plat-${t##*:}" --lib -- -D warnings || return 1
    done
}
step_rules() { bash "$V2_ROOT/bin/v2" rules --root "$V2_ROOT"; }
step_selftest_rules() { bash "$V2_ROOT/test/rules/run.sh"; }
# **KT-331 / D2-230** — ca âm ĐỎ **đúng mã** của máy kiểm tên tệp Windows/NTFS + ca
# dương XANH + ca âm/dương end-to-end của hook `pre-commit` (repo tạm, không QEMU).
step_selftest_filenames() { step_py "$V2_ROOT/tools/check-filenames.py" --selftest; }
# **KT-333b (D2-21, D2-37(c))** — cổng giấy phép: allowlist `deny.toml` (QD §9.3) + kiểm
# `Cargo.lock` ↔ `cargo metadata` + nguồn registry + `THIRD-PARTY.md` khớp bản sinh (không drift).
step_licenses() { step_py "$V2_ROOT/tools/check-licenses.py" --root "$REPO_ROOT" --check; }
step_selftest_licenses() { step_py "$V2_ROOT/tools/check-licenses.py" --selftest; }
# **KT-407 (B3, KE-HOACH-KIEN-TRUC nút thắt 11)** — hai workspace Cargo (`v2/` và `v2/kernel/`)
# phải cùng phiên bản + cùng nguồn cho **mọi** crate dùng chung; trôi âm thầm ⇒ sim/oracle và
# kernel chạy hai phiên bản khác nhau mà cổng vẫn xanh. Fail-closed khi thiếu/hỏng lockfile.
step_lockfile_sync() { step_py "$V2_ROOT/tools/check-lockfile-sync.py" --root "$REPO_ROOT"; }
step_selftest_lockfile_sync() { step_py "$V2_ROOT/tools/check-lockfile-sync.py" --selftest; }
# **KT-407** — helper dùng chung của runner QEMU (`rc=126` + `log_inventory` + 1 `trap EXIT`):
# tự kiểm 11 ca thuần shell, **không** QEMU. Bắt buộc có bước trong cổng (KT-134: script tự kiểm
# phải có cổng gọi thật).
step_selftest_run_common() { bash "$V2_ROOT/test/boot/lib/run-common.sh" --selftest; }
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
