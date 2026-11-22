#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245/D2-248) — **hằng số** của cổng v2: số bước kỳ vọng, danh sách bước chỉ T2,
# thư mục quét luật cấu trúc. Thêm/bớt bước là việc **có chủ ý** ⇒ phải sửa hằng số ở đây.
#
# Tệp này được `source` từ `v2/bin/v2` (thứ tự: consts → util → lock → lane → steps_* →
# check/gate/nightly → t0). KHÔNG chạy trực tiếp.
# **KT-242 (A0 2026-11-20)** — ba tầng kiểm, tên gọi rõ ràng:
#   T0  trong lúc sửa: agent tự chạy `cargo test -p <crate>` + checker liên quan (không phải lệnh ở đây).
#   T1  `gate --dev`  : fmt/lint + luật + checker + compile-fail + L0 + L1 1.000 + smoke QEMU.
#                       **KHÔNG** phải full gate; không thay tiêu chí nghiệm thu.
#   T2  `gate --fast` : đầy đủ 76 bước (mặc định; `CHECK_STEPS_EXPECTED` + `GATE_FAST_EXTRA_EXPECTED`) — dùng trên ứng viên tích hợp.
# Bước chỉ chạy ở T2: selftest của runner/QEMU/persist/crash/selfbreak/git-guard. Chúng kiểm
# **harness**, không kiểm mã sản phẩm, và chiếm ~350 s trong tổng ~685 s.
V2_TIER="${V2_TIER:-t2}"
V2_T2_ONLY_STEPS="selftest runner ktest (KT-54)|selftest qemu guard (KT-78)|selftest persist đỏ-trước (KT-75)|cắt điện giữa lúc ghi (KT-136)|selfbreak 5 luật K6 (KT-80)|ca âm git-guard (KT-85)|ca âm git-guard song song (R5)|ca âm git-guard b (KT-85b)|QEMU virtio-net (KT-245/246)|kiểm máy --cfg test (KT-256)|QEMU virtio-input (KT-255)|QEMU virtio-gpu (KT-263)|wake dep-info --product-fs (KT-267)|QEMU virtio-gpu x86_64 (KT-337/KT-333c)"
# **KT-333a** — chốt **số bước đăng ký** của từng lệnh. Lý do: bỏ một dòng `lane_step` (hoặc gộp
# sai) thì cổng vẫn có thể xanh vì **không ai đếm** — trái AGENTS §6.1 ("không bỏ âm thầm bước
# nào") và D2-235(5) (trùng id `lane_step` ⇒ `lane_done` coi bước sau đã chạy và bỏ qua im lặng).
# Thêm/bớt bước là việc **có chủ ý** ⇒ phải cập nhật đúng hằng số dưới đây; lệch ⇒ cổng ĐỎ ngay
# trước khi chạy bước nào (fail-closed, không tốn thời gian).
CHECK_STEPS_EXPECTED=45        # `check` (id 1..40 + 72 KT-331 + 73/74/75 KT-333b + 78 KT-333c)
GATE_FAST_EXTRA_EXPECTED=31    # `gate --fast` thêm 31 bước (id 41..64, 66, 67, 69, 71, 76, 77, 79)
T2_ONLY_STEPS_EXPECTED=14      # số bước T1 bỏ (khớp `V2_T2_ONLY_STEPS`)
