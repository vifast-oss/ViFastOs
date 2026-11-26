#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245/D2-248) — `do_gate_dev` (T1) + `do_gate_fast` (T2).
#
# **Khoá 2 lớp (D2-248 mục 1)**: T2 giữ `measure-lock` EXCLUSIVE + pre-flight `< 1 s` (cây bẩn
# hoặc `df < 5 GiB` ⇒ TỪ CHỐI trước khi chạy bước nào). T1 **không** giành khoá: chỉ kiểm
# `measure_lock_is_held` và **nhường ngay** nếu T2/nightly/soak đang giữ (T1 không phải phép đo).
# **KT-242** — T1: kiểm tích hợp nhanh. Chạy `do_gate_fast` với `V2_TIER=t1` (bỏ các bước
# harness nặng) + một smoke QEMU thật, rồi in bảng tổng có nhãn **T1** rõ ràng.
do_gate_dev() {
    # **KT-333d (D2-248 mục 1b, khoá 2 lớp)** — T1 **KHÔNG** giữ khoá độc quyền nữa: T1 là kiểm
    # tích hợp nhanh (không phải phép đo thời gian), nên nếu giữ khoá toàn thời gian thì 4 slot
    # sub-agent không thể chạy `gate --dev` song song (sự cố 2026-11-22). Lớp 2 chỉ **kiểm**:
    # đang có T2/nightly/soak giữ khoá ⇒ **nhường ngay** (không chạy chồng lên phép đo).
    # `do_gate_fast` bên dưới thấy `V2_TIER=t1` nên cũng không giành khoá.
    v2_lock_t1_guard || return 1
    V2_TIER=t1
    export V2_TIER
    do_gate_fast || return 1
    # Smoke QEMU: 7 ảnh test × 1 boot, smp=1. Đây là bằng chứng "ảnh vẫn boot được", **không**
    # thay ma trận QEMU của T2. QEMU ⇒ làn `sau` (đuôi; trong T1 các làn đuôi khác đã bỏ qua).
    # **KT-250** — id **70** (không phải 58): xem lịch sử — trùng id thì `lane_done` coi bước này
    # đã chạy và **bỏ qua im lặng** QEMU smoke. **KT-333a** kiểm trùng id + số bước tự động.
    lane_step sau 70 "QEMU smoke (T1: 7 ảnh × 1 boot)" step_qemu_smoke
    # **KT-333a** — T1 đăng ký toàn bộ bước của T2 + bước 70 (QEMU smoke); đúng 14 bước bị bỏ
    # theo `V2_T2_ONLY_STEPS` (in rõ `BỎ QUA` + `# gate-steps … skipped=14`, không bỏ âm thầm).
    if [ "$(lane_count)" -ne $((CHECK_STEPS_EXPECTED + GATE_FAST_EXTRA_EXPECTED + 1)) ]; then
        echo "--- LỖI: gate --dev đăng ký $(lane_count) bước, kỳ vọng $((CHECK_STEPS_EXPECTED + GATE_FAST_EXTRA_EXPECTED + 1))" >&2
        return 1
    fi
    lane_run || return 1
    # **KT-00 (2026-11-27)** — audit test: T1 "bỏ đúng 14 bước" chưa có máy kiểm (hằng
    # `T2_ONLY_STEPS_EXPECTED` khai mà không dùng). Đếm tệp `.skip` thật ⇒ lệch là ĐỎ.
    nskip_t1="$(find "$LANE_DIR" -maxdepth 1 -name '*.skip' 2>/dev/null | wc -l | tr -d ' ')"
    if [ "$nskip_t1" -ne "$T2_ONLY_STEPS_EXPECTED" ]; then
        echo "--- LỖI: T1 bỏ $nskip_t1 bước, kỳ vọng $T2_ONLY_STEPS_EXPECTED (D2-256 fail-closed)" >&2
        return 1
    fi
    # Trần id **76** = bước cuối của `gate --fast` (KT-340a) + bước 72 của `check` (KT-331).
    # Bản cũ ghi 70 nên **bỏ sót** bước 71 khi đếm — nay đếm đủ, không nới ngưỡng nào.
    [ "$(lane_fails 999)" -eq 0 ] || return 1
    return 0
}

# **KT-333d (D2-248 mục 1a)** — pre-flight **< 1 s** của T2, chạy **trước** bước nào: cây bẩn
# (`dirty != 0`) hoặc đĩa trống `< 5 GiB` ⇒ TỪ CHỐI ngay. Lý do: sự cố 2026-11-22 — `REV-GATE`
# vẫn chạy tiếp vài phút trên cây bẩn rồi mới đỏ, và một đợt tràn ổ đĩa 12 GiB. Không nới ngưỡng:
# cả hai điều kiện là điều kiện **đo được**, không phải "để cổng xanh".
# Chỉ áp cho T2 (T1 được phép chạy trên cây đang sửa — đó là mục đích của T1).
V2_PREFLIGHT_MIN_FREE_GIB="${V2_PREFLIGHT_MIN_FREE_GIB:-5}"
v2_preflight_t2() {
    local dirty df_kb min_kb
    [ "${V2_TIER:-t2}" = "t1" ] && return 0
    # **KT-333e (2026-11-23)** — "bẩn" = có **thay đổi theo dõi** (sửa/đã stage) ⇒ ứng viên đo
    # **không** khớp commit. Tệp **chưa theo dõi** (`??`) **không** vào ứng viên (cũng không vào
    # `dirty=` của `REV-STATE`), nên chỉ **cảnh báo** — chặn vì chúng sẽ làm T2 đỏ oan ngay ở
    # pre-flight (kho phiếu cũ ở gốc repo). Vẫn in danh sách để lead dọn.
    dirty="$(git -C "$REPO_ROOT" status --porcelain --untracked-files=no 2>/dev/null || true)"
    if [ -n "$dirty" ]; then
        echo "--- T2 pre-flight: TỪ CHỐI — cây làm việc **bẩn** (dirty != 0). Commit/stash rồi chạy lại (D2-248)." >&2
        printf '%s\n' "$dirty" | sed -n '1,10p' | sed 's/^/    /' >&2
        return 1
    fi
    local untracked
    untracked="$(git -C "$REPO_ROOT" status --porcelain --untracked-files=normal 2>/dev/null | grep -c '^??' || true)"
    if [ "${untracked:-0}" -gt 0 ]; then
        echo "--- T2 pre-flight: CẢNH BÁO — $untracked tệp **chưa theo dõi** (không vào ứng viên đo; xem \`git status --porcelain | grep '^??'\`)" >&2
    fi
    min_kb=$((V2_PREFLIGHT_MIN_FREE_GIB * 1024 * 1024))
    df_kb="$(df -Pk "$REPO_ROOT" 2>/dev/null | awk 'NR==2 { print $4 }')"
    case "${df_kb:-}" in
        '' | *[!0-9]*) echo "--- T2 pre-flight: không đọc được dung lượng trống (df) — bỏ qua kiểm đĩa (không phải nới ngưỡng đo)" >&2 ;;
        *)
            if [ "$df_kb" -lt "$min_kb" ]; then
                echo "--- T2 pre-flight: TỪ CHỐI — đĩa trống $((df_kb / 1024 / 1024)) GiB < ${V2_PREFLIGHT_MIN_FREE_GIB} GiB (D2-248)." >&2
                echo "    dọn: bash v2/tools/wt.sh prune   (hoặc xoá target-task của task đã merge)" >&2
                return 1
            fi
            ;;
    esac
    return 0
}

do_gate_fast() {
    local fails
    # KT-333c (D2-243 mục 3a) — **fail-closed** ngay đầu T2: hai `gate --fast` song song làm
    # git-guard tranh `.git`, QEMU chồng tải, số đo không dùng được. Khoá theo PID + 6 h stale.
    # **KT-333d (D2-248)** — khoá 2 lớp: chỉ T2 giữ khoá + pre-flight; T1 (V2_TIER=t1) bỏ cả hai
    # (T1 đã kiểm `measure_lock_is_held` và nhường ngay ở `do_gate_dev`).
    if [ "${V2_TIER:-t2}" = "t1" ]; then
        :
    else
        v2_preflight_t2 || return 1
        measure_lock_enter "gate --fast (T2)" || return 1
    fi
    [ -n "$LANE_DIR" ] || lane_reset || return 1
    do_check || return 1
    # **KT-333a (P1)** — tài nguyên từng làn đuôi (chạy **song song với nhau**, mỗi làn tuần tự):
    #
    #   main  — bước ghi `target/` (L0, L0 kernel, L1) hoặc bước **đo** (pre-measure/pin-artifact/
    #           algo-strict/`--cfg test`): tuần tự, KHÔNG đặt cạnh bước khác (D2-116/D2-141: L0 có
    #           binary nhạy thời gian; bước đo bị nhiễu dưới tải).
    #   sau   — **tuần tự, chạy một mình** (shell chính, trước các làn song song):
    #           `49 selftest persist đỏ-trước` → `50 cắt điện giữa lúc ghi`. 50 là bước **nhạy
    #           tải** (D2-151): dòng `verify pha 6` vừa in thì phải `kill` group **ngay**, host
    #           chậm ⇒ kill rơi sau pha 7 ⇒ `records=13` đỏ oan. Vì vậy 49/50 **không** chạy cạnh
    #           QEMU nào khác (đây là lý do vẫn giữ một pha tuần tự ngắn); 49 trước 50 vì cùng
    #           `$V2_RUN_TMP/kt75-persist`.
    #   sau2  — `44 selftest runner ktest` + `45 selftest qemu guard`: **cùng một ảnh sản phẩm**
    #           `vifast-kbin-riscv64` do 44 dựng ở rev-gate, nên 45 phải **sau** 44 ⇒ cùng làn.
    #           44 sửa `v2/test/boot/expected.txt` (~1 s, trả lại + sha256) — không bước nào ở
    #           làn đuôi khác đọc tệp đó (đã `grep` mọi runner QEMU: chỉ `ktest.sh`/`selftest.sh`).
    #   sau3  — `51 selfbreak`: **chỉ đọc**: băm `v2/bin/v2`, `v2/tools/verify-numbers.py`,
    #           `v2/docs/k6-machine-rules.md`, `v2/test/rules/**` trước/sau + dựng bản copy bằng
    #           `git ls-files`/`git show` (đọc index/HEAD worktree chính). Không bước nào ở làn
    #           đuôi khác ghi bốn đường đó hay index chính (git-guard chỉ `git add` trong
    #           **worktree tạm** — `neg-test.sh:25`) ⇒ chạy song song an toàn.
    #   sau4  — ba ca âm git-guard (52,53,54): chỉ dùng `git worktree add` + nhánh tạm + bare repo
    #           trong `mktemp`; cùng làn để không hai tiến trình cùng sửa worktree/branch của repo.
    #   sau5  — QEMU aarch64 (56) + virtio-net (57): **cùng bind `127.0.0.1:45678`** (server TCP
    #           của runner) ⇒ bắt buộc tuần tự trong **một** làn; 56 trước 57 như cũ.
    #   sau6  — QEMU x86_64 (59) + virtio-input (60) + virtio-gpu (63): mỗi bước ghi
    #           `$V2_RUN_TMP`/`kernel/target` riêng, không bind cổng host; cargo tự khoá
    #           `kernel/target` nên build xen kẽ **không** đua tệp.
    #
    # QEMU đồng thời tối đa = 3 (sau2, sau5, sau6) — đúng trần AGENTS §9; selfbreak/git-guard
    # không chạy QEMU.
    lane_step main 41 "L0 test" step_l0
    lane_step main 42 "L0 kernel workspace (KT-148)" step_l0_kernel
    lane_step main 43 "L1 (sim 1.000 seed)" step_l1
    lane_step sau2 44 "selftest runner ktest (KT-54)" step_selftest_ktest
    lane_step sau2 45 "selftest qemu guard (KT-78)" step_guard
    lane_step main 46 "selftest pre-measure (KT-95)" step_pre_measure
    lane_step main 47 "selftest pin-artifact (KT-84)" step_pin_artifact
    lane_step main 48 "algo= nghiêm ngặt ở đường đo (KT-145)" step_algo_strict
    lane_step sau 49 "selftest persist đỏ-trước (KT-75)" step_persist_red
    lane_step sau 50 "cắt điện giữa lúc ghi (KT-136)" step_persist_crash
    lane_step sau3 51 "selfbreak 5 luật K6 (KT-80)" step_selfbreak
    lane_step sau4 52 "ca âm git-guard (KT-85)" step_neg1
    lane_step sau4 53 "ca âm git-guard song song (R5)" step_neg2
    lane_step sau4 54 "ca âm git-guard b (KT-85b)" step_neg3
    lane_step D 55 "selftest validator net (KT-245)" step_net_red
    lane_step sau5 56 "QEMU aarch64 (K4' bước 3)" step_aarch64
    lane_step sau5 57 "QEMU virtio-net (KT-245/246)" step_net_run
    lane_step main 58 "kiểm máy --cfg test (KT-256)" step_no_cfg_test
    lane_step sau6 59 "QEMU x86_64 (K5' bước 1)" step_x86_64
    lane_step sau6 60 "QEMU virtio-input (KT-255)" step_input_run
    lane_step B 61 "selftest validator input (KT-255)" step_input_red
    lane_step B 62 "selftest validator cfg-test (KT-256)" step_no_cfg_test_red
    lane_step sau6 63 "QEMU virtio-gpu (KT-263)" step_gpu_run
    # **KT-333c** — id **79**, làn **sau6** (xem chú thích làn: bước **mở QEMU + ghi
    # `kernel/target`** không được nằm ở làn nền chỉ-đọc; đo được 21 s nên vào T2, và T1 bỏ qua).
    lane_step sau6 79 "QEMU virtio-gpu x86_64 + ca âm không GPU (KT-337/KT-333c)" step_gpu_x86_64
    lane_step C 64 "selftest validator gpu (KT-263)" step_gpu_red
    lane_step B 66 "selftest soak aarch64 (KT-265)" step_a64_soak_selfcheck
    lane_step D 67 "selftest budget aarch64 (KT-265)" step_a64_budget
    lane_step D 76 "selftest budget x86_64 (KT-340a)" step_x64_budget
    lane_step D 77 "spec/kernel.toml ↔ mã (D2-242 nhóm 1)" step_kernel
    lane_step C 69 "wake dep-info --product-fs (KT-267)" step_wake_depinfo_product_fs
    lane_step D 71 "selftest soak x86_64 (KT-293)" step_x64_soak_selfcheck
    # **KT-333a / KT-345a / KT-333c / KT-407 / KT-409 / KT-404d** — chốt tổng số bước T2 =
    # `CHECK_STEPS_EXPECTED` (50: `check`, gồm 80/81/82 B3 + run-common, 83/84 workspace, 85 boot
    # ABI) + 31 (`gate --fast`) = 81. Bỏ một dòng `lane_step` (hoặc trùng id) ⇒ ĐỎ ngay trước khi
    # chạy bước nào.
    if [ "$(lane_count)" -ne $((CHECK_STEPS_EXPECTED + GATE_FAST_EXTRA_EXPECTED)) ]; then
        echo "--- LỖI: gate --fast đăng ký $(lane_count) bước, kỳ vọng $((CHECK_STEPS_EXPECTED + GATE_FAST_EXTRA_EXPECTED)) — cập nhật GATE_FAST_EXTRA_EXPECTED nếu thêm/bớt bước có chủ ý" >&2
        return 1
    fi
    lane_run || return 1
    fails="$(lane_fails 999)"
    [ "$fails" -eq 0 ] || return 1
    return 0
}
