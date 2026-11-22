#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-248 mục 1) — **lớp khoá trong `v2/bin/v2`**: `measure_lock_enter` /
# `measure_lock_leave_all` (KT-333c) + `v2_lock_t1_guard` của khoá 2 lớp.
#
# Lớp 1 (độc quyền): T2 `gate --fast` / `nightly` / `soak*.sh` giữ `measure-lock` EXCLUSIVE.
# Lớp 2 (chỉ kiểm): T1 `gate --dev` và T0 `v2 t0` **không** giành khoá; chúng chỉ hỏi
# `measure_lock_is_held` (định nghĩa trong `v2/tools/measure-lock.sh`, không phá stale, không
# tạo khoá). T1 nhường ngay khi lớp 1 đang giữ; T0 chỉ cảnh báo.

# ---------------------------------------------------------------- lớp 2: T1 nhường ngay

# **KT-333c** — vào/ra khoá độc quyền máy đo. Lồng được (`gate --dev` gọi `gate --fast`): chỉ
# tiến trình **ngoài cùng** giữ khoá; các lớp trong chỉ đếm độ sâu. `release` chạy trong trap EXIT
# của shell chính; **subshell làn nền không được nhả khoá của cha** (`BASH_SUBSHELL > 0` ⇒ bỏ qua,
# nếu không làn đầu tiên xong sẽ xoá khoá — bài học bash 3.2 không có `BASHPID`).
MEASURE_LOCK_DEPTH=0
measure_lock_enter() { # <nhãn>
    local nhan="$1"
    if [ "$MEASURE_LOCK_DEPTH" -gt 0 ]; then
        MEASURE_LOCK_DEPTH=$((MEASURE_LOCK_DEPTH + 1))
        return 0
    fi
    measure_lock_acquire "$nhan" || return 1
    MEASURE_LOCK_DEPTH=1
    trap 'measure_lock_leave_all' EXIT
    trap 'measure_lock_leave_all; exit 130' INT
    trap 'measure_lock_leave_all; exit 143' TERM
    trap 'measure_lock_leave_all; exit 129' HUP
    return 0
}
measure_lock_leave_all() {
    [ "${BASH_SUBSHELL:-0}" = "0" ] || return 0
    [ "$MEASURE_LOCK_DEPTH" -gt 0 ] || return 0
    MEASURE_LOCK_DEPTH=0
    measure_lock_release || true
}

# **KT-333d (D2-248 mục 1b)** — **lớp 2** của khoá: T1 **không giành** khoá độc quyền (T1 không đo
# thời gian). Nếu lớp 1 (T2/nightly/soak) đang giữ thì T1 **nhường ngay** — chạy chồng sẽ làm hỏng
# số đo của lớp 1 và QEMU chồng tải. Trả 0 = được chạy; 1 = nhường (đã in lý do).
# Đọc trạng thái bằng `measure_lock_is_held` (không tạo khoá, không phá stale).
v2_lock_t1_guard() {
    local label=""
    measure_lock_is_held || return 0
    label="$(measure_lock_holder_label 2>/dev/null || true)"
    case "$label" in
        *"gate --dev"*)
            # Giai đoạn chuyển tiếp: T1 của **mã trước KT-333d** vẫn giữ khoá suốt lượt chạy. Hai
            # T1 không phải phép đo (không ngưỡng thời gian) ⇒ chạy song song được, tối đa 2 QEMU
            # (AGENTS §9 cho 3). Sau khi merge, không T1 nào giành khoá nên nhánh này thành vô hại.
            echo "--- T1: một T1 KHÁC (mã trước KT-333d) đang giữ khoá — T1 không phải phép đo ⇒ tiếp tục (không chồng T2/nightly/soak)." >&2
            return 0
            ;;
    esac
    echo "--- T1: có cổng/soak KHÁC đang giữ khoá máy đo ⇒ **nhường ngay** (D2-248 — T1 không giành khoá, không chạy chồng T2/nightly/soak)." >&2
    measure_lock_status >&2 || true
    return 1
}

# **KT-333d** — T0 (`v2 t0`) là tầng **nhanh nhất**, KHÔNG phải cổng: chỉ **cảnh báo** khi lớp 1
# đang chạy (vì số đo thời gian/clippy dưới tải có thể nhiễu), KHÔNG chặn — T0 không tạo bằng chứng.
v2_lock_t0_warn() {
    measure_lock_is_held || return 0
    echo "# CẢNH BÁO T0: có cổng/soak khác đang giữ khoá máy đo — T0 chỉ cảnh báo, KHÔNG chặn (D2-248)." >&2
    measure_lock_status >&2 || true
    echo "#   (T0 không phải cổng; số thời gian dưới tải chỉ để tham khảo.)" >&2
    return 0
}
