#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — `do_check`: đăng ký 44 bước của `v2 check` (id 1..40, 72, 73, 74, 75, 78;
# **KT-333e** gộp 20+22 ⇒ 45 → 44 bước, 0 luật bị bỏ), chốt `CHECK_STEPS_EXPECTED` fail-closed
# rồi chạy theo làn.
do_check() {
    local fails
    [ -n "$LANE_DIR" ] || lane_reset || return 1
    V2_PY="$(find_python || true)"
    echo "== v2 check (HEAD $(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo '?')) =="
    # Số thứ tự = **đúng thứ tự cũ** của `check`; làn quyết định chạy song song hay không.
    # Làn nền **chỉ** chứa bước chỉ đọc (không ghi `target/`, không sửa cây, không QEMU).
    #
    # **KT-333a (P1)** — cân lại 6 làn nền theo **thời gian đo được** (`JOURNAL/kt333a-t2wall.md`):
    # trước đây `main` = fmt+clippy+luật+selftest build-inputs+wake dep-info (109 s) và các làn
    # A/B/C lệch nhau (105/94/102 s) trong khi tổng công chỉ ~300 s ⇒ 3 làn là **quá ít** cho 12
    # nhân và `main` bị chặn bởi bước không cần tuần tự. Nay:
    #   * `main` chỉ còn bước **ghi `v2/target/`**: `clippy` + `wake theo dep-info` (cargo check);
    #   * `fmt`, `luật cấu trúc`, `selftest build-inputs`, `self-test luật`… là **chỉ đọc** ⇒ làn nền;
    #   * 6 làn nền A..F, mỗi làn ~40–70 s (đo), tổng công giữ nguyên, 0 bước bị bỏ.
    lane_step F 1 "fmt" step_fmt
    lane_step main 2 "clippy" step_clippy
    lane_step F 3 "luật cấu trúc R1..R5 + tên tệp NTFS (KT-331)" step_rules
    lane_step A 4 "self-test luật" step_selftest_rules
    lane_step D 5 "luật máy K6 (KT-80: 1,2,3,7,8)" step_k6
    lane_step D 6 "selftest build-inputs (KT-128/134: a..h)" step_build_inputs
    lane_step E 7 "selftest danh tính ảnh (KT-145: l1..l5)" step_build_identity
    lane_step F 8 "selftest t26 (KT-149: g1..g4)" step_t26
    lane_step F 9 "ghim thuật toán dấu vân tay (KT-134)" step_algo_pin
    lane_step F 10 "selftest ghim thuật toán (KT-134)" step_selftest_algo_pin
    lane_step B 11 "selftest shell-edge (KT-108)" step_shell_edge
    lane_step F 12 "selftest budget (KT-88)" step_budget
    lane_step F 13 "selftest counter-evidence (KT-80)" step_counter_evidence
    lane_step F 14 "selftest task-target (KT-84)" step_task_target
    lane_step F 15 "selftest ba mức L1 (KT-242)" step_selftest_l1_tiers
    lane_step sau 16 "bảng đối chiếu số ↔ khối scope= (KT-134)" step_scope_table
    lane_step F 17 "selftest bảng đối chiếu (KT-134)" step_selftest_scope_table
    lane_step E 18 "selftest wiring (KT-134)" step_wiring
    lane_step E 19 "cổng selftest có cổng gọi (KT-134)" step_wiring_standalone
    # **KT-333e (D2-247)** — bước 20 **gộp** hai lượt `wake paths` + `wake paths --strict` (cũ là
    # id 20 và 22) thành **một** lượt quét: `check-wake-paths.py --combined`. Hai chế độ vẫn chạy
    # **đủ luật** và in **đủ hai bảng** `strict=0`/`strict=1` như trước (log so từng dòng được với
    # bản cũ); chỉ phần **chung** (đọc tệp, token hoá, đồ thị module, luật 1–5) là chạy một lần.
    # Đo: **−38,8 s CPU**/cổng với interpreter cổng (`python3.12`; −49,2 s với `python3` 3.9) —
    # ~92 % trần lý thuyết của phép gộp là một lượt đầy đủ (~42 s CPU). Số bước `check` 45 → 44 là việc **có chủ ý**
    # (không bỏ luật nào) nên `CHECK_STEPS_EXPECTED` cập nhật theo.
    lane_step C 20 "wake paths + --strict (KT-151/155 — KT-333e: một lượt)" step_wake_paths_combined
    lane_step A 21 "selftest wake paths (KT-151)" step_selftest_wake_paths
    lane_step main 23 "wake theo dep-info (KT-217: lớp 2)" step_wake_depinfo
    lane_step C 24 "selftest wake dep-info (KT-217)" step_selftest_wake_depinfo
    lane_step sau 25 "api boundary (KT-241)" step_api_boundary
    lane_step E 26 "test có răng (KT-163)" step_test_teeth
    lane_step F 27 "selftest test có răng (KT-163)" step_selftest_test_teeth
    lane_step F 28 "abi spec" step_abi
    lane_step F 29 "golden trace (D2-43)" step_golden
    lane_step F 30 "kịch bản .scn (KT-04c)" step_scn
    lane_step F 31 "scn fixtures (KT-04c)" step_scn_fixtures
    lane_step E 32 "báo cáo R1-v2 (Khối E)" step_report
    lane_step E 33 "fixture âm của máy kiểm báo cáo" step_report_fixtures
    lane_step F 34 "abi reserved (KT-04b)" step_abi_reserved
    lane_step F 35 "abi fixtures" step_abi_fixtures
    lane_step F 36 "gen-check (KT-05: code + docs sinh từ spec)" step_gen
    lane_step F 37 "gen fixtures" step_gen_fixtures
    lane_step F 38 "idl-gen-check (KT-69: stub sinh từ .vidl)" step_idl
    lane_step F 39 "idl fixtures" step_idl_fixtures
    lane_step sau 40 "build (L0 compile)" step_build
    lane_step E 72 "selftest tên tệp NTFS (KT-331)" step_selftest_filenames
    lane_step main 73 "giấy phép + THIRD-PARTY.md (D2-21)" step_licenses
    lane_step B 74 "selftest cổng giấy phép (D2-21)" step_selftest_licenses
    lane_step B 75 "selftest nightly (kế hoạch + trạng thái)" step_selftest_nightly
    # **KT-407 (B3) + KT-404c** — đồng bộ **3** `Cargo.lock` (`v2/` ↔ `v2/kernel/` ↔ `v2/user/`):
    # cùng phiên bản + nguồn cho **mọi cặp** crate dùng chung. Id 80/81 nằm ngoài dải 1..40 và dải
    # `gate --fast` (41..79) nên phải đếm riêng ở cuối `do_check` (cùng khuôn bước 72).
    lane_step F 80 "đồng bộ 3 Cargo.lock (B3, KT-407/KT-404c)" step_lockfile_sync
    lane_step F 81 "selftest đồng bộ lockfile (B3, KT-407/KT-404c)" step_selftest_lockfile_sync
    # **KT-407** — tự kiểm helper runner dùng chung (`rc=126`/`log_inventory`/1 `trap EXIT`).
    lane_step F 82 "selftest helper runner dùng chung (KT-407)" step_selftest_run_common
    # **KT-404 (B4, D2-259)** — `v2/user/` là workspace Cargo thật: `clippy -D warnings` cho **3
    # target bare-metal** + 8 ca host của lõi bump. Làn `main` vì bước này **ghi** target dir
    # riêng (`<target>/user-workspace`) — cùng làn với `clippy`/`wake dep-info` để không tranh
    # khoá cargo với làn nền (chỉ đọc).
    lane_step main 83 "workspace v2/user: clippy 3 arch + host test (KT-404)" step_user_workspace
    # **KT-409 (D2-260 mục 3)** — `clippy -D warnings` cho **workspace kernel** (`v2/kernel/`):
    # đóng lỗ hổng `exclude = ["kernel"]` của `v2/Cargo.toml` (trước bước này **không** cổng nào
    # quét `clippy` cho 8 crate kernel). Làn `main` vì bước **ghi** `$CARGO_TARGET_DIR`.
    lane_step main 84 "workspace v2/kernel: clippy host-testable (KT-409)" step_clippy_kernel
    # **KT-333c (D2-243 mục 3a)** — bộ tự kiểm khoá độc quyền máy đo (9 ca, thuần shell, **không**
    # chạm khoá thật: `V2_MEASURE_LOCK_DIR` trỏ thư mục tạm của ca). Bắt buộc có bước trong cổng:
    # khoá là thứ **cấm** chạy chồng cổng, nên chính nó phải được kiểm bằng cổng (KT-134).
    lane_step D 78 "selftest khoá máy đo (KT-333c)" step_measure_lock
    # **KT-404d** — khớp boot ABI kernel (`proc_abi.rs`) ↔ userland (`v2/user/programs/*`):
    # 17 `EP_*` + 8 `SLOT_*`/`LOOP_*` + 2 mảng; 1 bước gộp cả `--selftest` (13 ca) lẫn kiểm cây
    # thật. Làn E: chỉ đọc, không ghi `target/`, không QEMU.
    lane_step E 85 "khớp boot ABI kernel ↔ userland (KT-404d)" step_boot_abi
    # **KT-333a** — chốt số bước `check` (fail-closed): bỏ/thêm một dòng `lane_step` mà không cập
    # nhật `CHECK_STEPS_EXPECTED` ⇒ ĐỎ ngay, không chạy bước nào (AGENTS §6.1, D2-235(5)).
    if [ "$(lane_count)" -ne "$CHECK_STEPS_EXPECTED" ]; then
        echo "--- LỖI: check đăng ký $(lane_count) bước, kỳ vọng $CHECK_STEPS_EXPECTED — cập nhật CHECK_STEPS_EXPECTED nếu thêm/bớt bước có chủ ý" >&2
        return 1
    fi
    lane_run || return 1
    # Bước 72 là **bước của `check`** nhưng mang id sau dải 1..40 (dải này đã đầy, id 41..71
    # thuộc `gate --fast`). Không nới `max` lên 72: như trên, việc đó sẽ kéo bước 51 vào lượt
    # đếm của `check`. Đếm riêng bằng `lane_failed`.
    fails="$(lane_fails 40)"
    fails=$((fails + $(lane_failed 72) + $(lane_failed 73) + $(lane_failed 74) + $(lane_failed 75) + $(lane_failed 78) + $(lane_failed 80) + $(lane_failed 81) + $(lane_failed 82) + $(lane_failed 83) + $(lane_failed 84) + $(lane_failed 85)))
    if [ "$fails" -ne 0 ]; then
        echo "v2 check: $fails bước FAIL" >&2
        return 1
    fi
    echo "v2 check: PASS (L1 chạy trong gate --fast)"
    return 0
}
