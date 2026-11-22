#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — **bước Miri và driver L1 của `v2 nightly`** (KT-13/KT-17/KT-29/KT-333b):
# `do_miri`, `run_miri_test`, `run_nightly_l1_driver`, `run_nightly_miri`. Nguyên văn bản cũ.
# ---------------------------------------------------------------- Miri (KT-13/KT-17, D2-47)

# In đúng câu lệnh bootstrap khi thiếu toolchain — không bao giờ im lặng bỏ qua.
miri_bootstrap_hint() {
    echo "v2 miri: bootstrap Miri bằng:" >&2
    echo "    rustup toolchain install nightly" >&2
    echo "    rustup component add miri rust-src --toolchain nightly" >&2
}

# Kiểm toolchain Miri đủ (nightly + miri + rust-src + `cargo miri` chạy được).
# Thiếu ⇒ in câu lệnh bootstrap và exit (127 nếu thiếu rustup, 2 nếu thiếu component).
# Dùng chung cho `v2 miri` (KT-13) và `v2 nightly --only-miri` (KT-17) để hai lệnh không
# bao giờ lệch nhau về điều kiện "đủ toolchain" (fail rõ, không im lặng).
require_miri_toolchain() {
    local ten="${1:-v2 miri}" rustup_bin installed missing
    rustup_bin="$(find_rustup)" || {
        echo "$ten: không tìm thấy rustup (đã thử PATH, ~/.cargo/bin, /usr/local/cargo/bin, /opt/homebrew/bin)" >&2
        miri_bootstrap_hint
        return 127
    }
    if ! "$rustup_bin" toolchain list 2>/dev/null | grep -q '^nightly'; then
        echo "$ten: thiếu toolchain nightly của rustup" >&2
        miri_bootstrap_hint
        return 2
    fi
    installed="$("$rustup_bin" component list --installed --toolchain nightly 2>/dev/null || true)"
    missing=""
    printf '%s\n' "$installed" | grep -qE '^miri(-|$)' || missing="miri"
    printf '%s\n' "$installed" | grep -qE '^rust-src$' || missing="${missing:+$missing }rust-src"
    if [ -n "$missing" ]; then
        echo "$ten: toolchain nightly thiếu component: $missing" >&2
        miri_bootstrap_hint
        return 2
    fi
    # `component list` có thể lệch với thực tế — xác nhận cargo-miri chạy được.
    if ! "$CARGO" +nightly miri --version >/dev/null 2>&1; then
        echo "$ten: 'cargo +nightly miri --version' thất bại dù component đã cài" >&2
        miri_bootstrap_hint
        return 2
    fi
}

do_miri() {
    local sub
    require_miri_toolchain "v2 miri" || exit $?
    # Mặc định `test` (DoD KT-13: `v2 miri -p vifast-sim --lib`); chuyển tiếp nguyên tham số.
    sub="test"
    case "${1:-}" in
        run | r | test | t | nextest | setup | clean)
            sub="$1"
            shift
            ;;
    esac
    case "$sub" in
        setup | clean) exec "$CARGO" +nightly miri "$sub" "$@" ;;
        *) exec "$CARGO" +nightly miri "$sub" --manifest-path "$MANIFEST" "$@" ;;
    esac
}

# Một bước Miri của tập chọn; dừng ngay ở bước đỏ (không retry — D2-07).
run_miri_test() {
    local ten="$1"
    shift
    echo "--- Miri $ten"
    if "$CARGO" +nightly miri test --manifest-path "$MANIFEST" "$@"; then
        echo "--- Miri $ten: OK"
        return 0
    fi
    echo "--- Miri $ten: FAIL" >&2
    return 1
}

# KT-17 (D2-47(a)): CHỈ Miri trên tập chọn; KHÔNG L1 100k/mutation/soak.
#
# Tập chọn: toàn bộ `vifast-objmem` (lib + alias + fuzz 200 chuỗi + contract) và các test
# sim đi qua object store: `--lib`, kt16_objmem, kt09_untyped, kt09_revoke (plan §4 KT-17),
# kt10_cnode, kt17_align. Không chạy `l1` (tránh L1 1.000 seed dưới Miri).
# **KT-23** (K1B-PLAN §5 KT-23 DoD 4): thêm các test K1b đi qua TCB/scheduler/budget/donation/
# hộp thư — kt18_tcb, kt19_sched, kt20_budget, kt21_donation, kt22_mailbox (mỗi tệp tự giảm
# seed/bước bằng `cfg(miri)`); **không** thêm `kt23_k1b` (1.000 seed × 64 bước — keo dài, và
# không có I/O cần Miri kiểm; bất biến của nó chạy trong L0). KHÔNG thêm test I/O.
# **KT-24** (K1C-PLAN §5 KT-24 DoD 9): thêm `kt24_ipc` (MessageInfo/64 MR/ipc::transfer/no_reply;
# fuzz tự giảm seed/bước bằng `cfg(miri)`). KHÔNG thêm `kt24_scn` (đọc tệp — test I/O).
# **KT-26** (K1C-PLAN §5 KT-26 DoD 8): thêm `kt26_fault` (FaultKind 4 biến thể model-only, fault
# Call+Reply, `tcb_resume_with_regs`, Exit D2-25, mô hình procsrv; fuzz 1.000 seed tự giảm còn
# 3 seed × 8 op bằng `cfg(miri)`). KHÔNG thêm `kt26_scn` (đọc tệp — test I/O).
# **KT-27** (K1C-PLAN §5 KT-27 DoD 8): thêm `kt27_notification` (word 64 + badge, Signal OR, một
# waiter, Wait/Poll, bind 1:1, TCB chết; fuzz 1.000 seed tự giảm còn 3 seed × 8 op bằng
# `cfg(miri)`). KHÔNG thêm `kt27_scn` (đọc tệp — test I/O).
# **KT-28** (K1C-PLAN §5 KT-28 DoD 8): thêm `kt28_waitaddr` (bảng băm `(frame, offset)` 64 bucket,
# WaitAddr đọc giá trị + xếp hàng một lần, `WakeAddr(n)`, không timeout; fuzz 1.000 seed tự giảm
# còn 3 seed × 8 op bằng `cfg(miri)`). KHÔNG có tệp `.scn` mới nên không có test I/O để bỏ.
# **KT-29** (K1C-PLAN §5 KT-29 DoD 8) — L1 theo `smp`: driver `tests/l1_k1c.rs` đọc
# `VIFAST_SMP`/`VIFAST_SEEDS`; mặc định 10.000 seed × smp 1,2,4,8. `gate --fast` KHÔNG gọi
# lệnh này (vẫn 1.000 seed/smp 1 qua env mặc định của test `l1_k1c`). 100.000 seed là cổng
# cuối phase §7.1, chạy bằng `--seeds 100000` khi tới cổng — không nhét vào `--fast`.
run_nightly_l1_driver() { # <seeds> <smp> — driver L1 theo smp (KT-29/D2-53 Q13)
    local seeds="$1" smp="$2" t0 t1
    echo "== L1 (KT-29): seeds=$seeds smp=$smp; driver tests/l1_k1c.rs =="
    t0="$(date +%s)"
    if ! VIFAST_SEEDS="$seeds" VIFAST_SMP="$smp" "$CARGO" test --manifest-path "$MANIFEST" \
        -p vifast-sim driver_smp -- --nocapture; then
        echo "L1: FAIL (seeds=$seeds smp=$smp)" >&2
        return 1
    fi
    t1="$(date +%s)"
    echo "L1: PASS ($((t1 - t0)) s; seeds=$seeds smp=$smp)"
}

run_nightly_miri() {
    local t0 t1
    require_miri_toolchain "v2 nightly --only-miri" || return $?
    echo "== Miri (D2-47(a)): tập chọn KT-17/KT-23..KT-43; KHÔNG L1 100k/mutation/soak =="
    t0="$(date +%s)"
    run_miri_test "vifast-objmem (lib + alias + fuzz 200 + contract)" -p vifast-objmem || return 1
    run_miri_test "vifast-sim --lib" -p vifast-sim --lib || return 1
    run_miri_test "vifast-sim kt16_objmem" -p vifast-sim --test kt16_objmem || return 1
    run_miri_test "vifast-sim kt09_untyped" -p vifast-sim --test kt09_untyped || return 1
    run_miri_test "vifast-sim kt09_revoke" -p vifast-sim --test kt09_revoke || return 1
    run_miri_test "vifast-sim kt10_cnode" -p vifast-sim --test kt10_cnode || return 1
    run_miri_test "vifast-sim kt17_align" -p vifast-sim --test kt17_align || return 1
    run_miri_test "vifast-sim kt18_tcb" -p vifast-sim --test kt18_tcb || return 1
    run_miri_test "vifast-sim kt19_sched" -p vifast-sim --test kt19_sched || return 1
    run_miri_test "vifast-sim kt20_budget" -p vifast-sim --test kt20_budget || return 1
    run_miri_test "vifast-sim kt21_donation" -p vifast-sim --test kt21_donation || return 1
    run_miri_test "vifast-sim kt22_mailbox" -p vifast-sim --test kt22_mailbox || return 1
    run_miri_test "vifast-sim kt24_ipc" -p vifast-sim --test kt24_ipc || return 1
    # **KT-33** (D2-61/K1C-DOD §4): thêm `kt25_ipc2` — KT-31 đã ghi nợ "Miri all thiếu kt25_ipc2";
    # tệp tự giảm fuzz còn 3 seed × 8 op bằng `cfg(miri)` như các tệp K1c khác. KHÔNG thêm
    # `kt25_scn` (đọc tệp — test I/O) và không chạy `l1_k1c`.
    run_miri_test "vifast-sim kt25_ipc2" -p vifast-sim --test kt25_ipc2 || return 1
    run_miri_test "vifast-sim kt26_fault" -p vifast-sim --test kt26_fault || return 1
    run_miri_test "vifast-sim kt27_notification" -p vifast-sim --test kt27_notification || return 1
    run_miri_test "vifast-sim kt28_waitaddr" -p vifast-sim --test kt28_waitaddr || return 1
    # **KT-29** (K1C-PLAN §5 KT-29 DoD 9): thêm `kt29_smp` (SMP 1/2/4/8 + push migration; fuzz
    # tự giảm còn 3 seed × 8 op bằng `cfg(miri)`, stable vẫn 1.000 × 64 × 4). KHÔNG thêm
    # `l1_k1c` (1.000 seed — chạy ở gate/nightly --l1, không dưới Miri).
    run_miri_test "vifast-sim kt29_smp" -p vifast-sim --test kt29_smp || return 1
    # **KT-43** (K2-PLAN §4.2 D8 — Miri **toàn bộ** tập ảnh hưởng K2): thêm 4 tệp còn thiếu của
    # K2 (`kt36_frame`, `kt37_pagetable`, `kt38_vspace`, `kt39_wx`) + crate `vifast-pte` (KT-35).
    # Các tệp tự giảm seed/property bằng `cfg(miri)` như KT-36..42 (D2-47(a)); **không** thêm
    # `kt42_scn`/`kt43_scn` (đọc tệp) và **không** chạy `l1_k1c` 1.000 seed dưới Miri (lần chạy
    # mẫu KT-43 có env riêng, log `JOURNAL/kt43-miri-l1.log`).
    run_miri_test "vifast-sim kt36_frame" -p vifast-sim --test kt36_frame || return 1
    run_miri_test "vifast-sim kt37_pagetable" -p vifast-sim --test kt37_pagetable || return 1
    run_miri_test "vifast-sim kt38_vspace" -p vifast-sim --test kt38_vspace || return 1
    run_miri_test "vifast-sim kt39_wx" -p vifast-sim --test kt39_wx || return 1
    run_miri_test "vifast-pte (unit + doctest)" -p vifast-pte || return 1
    # **KT-40** (K2-PLAN §3 KT-40): thêm `kt40_tlb` (TLB 16 entry/hart + shootdown/ack + deadlock;
    # property tự giảm còn 2 hart × 150 vòng và ca cạn 256 ASID tự bỏ bằng `cfg(miri)`).
    run_miri_test "vifast-sim kt40_tlb" -p vifast-sim --test kt40_tlb || return 1
    # **KT-41** (K2-PLAN §3 KT-41; D2-65 Q6): thêm `kt41_vmfault` (FaultKind::Vm model-only, 3
    # reason NotMapped/Permission/Wx, fault_ep/K1c, resume, C2; property tự giảm còn 150 vòng
    # bằng `cfg(miri)`).
    run_miri_test "vifast-sim kt41_vmfault" -p vifast-sim --test kt41_vmfault || return 1
    # **KT-42** (K2-PLAN §3 KT-42): thêm `kt42_vmcheck` — một cửa `check_vm` ở cổng trên trạng
    # thái VM đầy đủ (Frame 3 cỡ, 2 VSpace shared read-only, TLB, shootdown). KHÔNG thêm
    # `kt42_scn` (đọc tệp — test I/O); unit test `check_vm` nằm trong `--lib` đã có ở trên.
    run_miri_test "vifast-sim kt42_vmcheck" -p vifast-sim --test kt42_vmcheck || return 1
    # **KT-30** (K1C-PLAN §5 KT-30 DoD 8): thêm `-p vifast-sync` (MCS lock + Mailbox MPSC; test
    # `kt30_stress` 4 luồng tự gạt bằng `#![cfg(not(miri))]` nên Miri chỉ chạy phần single-thread).
    run_miri_test "vifast-sync (lib + kt30_sync)" -p vifast-sync || return 1
    t1="$(date +%s)"
    echo "Miri: PASS ($((t1 - t0)) s; không mutation/soak/L1 100k)"
}
