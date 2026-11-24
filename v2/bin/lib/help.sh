#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-333d (D2-245) — `usage()` của `v2/bin/v2` (nguyên văn + mục `t0` của KT-333d).
usage() {
    cat <<'EOF'
v2 — cổng dòng OS ViFastOs v2 (vòng 1)

  ./v2/bin/v2 check            fmt + clippy + 5 luật cấu trúc + self-test luật + build
  ./v2/bin/v2 gate --dev       **T1** kiểm tích hợp nhanh: check + L0 + L1 1.000 + smoke QEMU
                               (bỏ các selftest harness nặng). KHÔNG phải full gate.
  ./v2/bin/v2 gate --fast      **T2** full gate: như check + L0 (cargo test --workspace) + mọi
                               selftest harness. BẮT BUỘC trên ứng viên tích hợp
  ./v2/bin/v2 gate             CHƯA CÓ trong vòng 1 (cần L2/L3)
  ./v2/bin/v2 t0 [crate ...] [--crate <tên>] [--no-test] [--selftest]
                               KT-333d/D2-248: **T0** — tầng kiểm nhanh trong lúc sửa
                               (fmt + clippy + `cargo test -p` trên crate bị chạm). **KHÔNG phải
                               cổng**: T1 = `gate --dev`, T2 = `gate --fast`; T0 chỉ **cảnh báo**
                               khi T2/nightly/soak đang giữ khoá máy đo (khoá 2 lớp, D2-248).
  ./v2/bin/v2 nightly --all | --l1 <seed> | --only-miri | --soak [--n N --runs K] | --status [--soak]
                               | --dry-run | --selftest
                               KT-333b: nightly thật (AGENTS.md §6). Không có mặc định chạy —
                               `v2 nightly` trần là lỗi dùng (exit 2). Xem chi tiết bằng
                               `./v2/bin/v2 nightly --help`; `--dry-run` in kế hoạch KHÔNG chạy.
                               L1-full = 100.000 seed × smp 1,2,4,8 (hợp đồng K6, chia lô qua
                               l1-tier.sh); Miri (KT-17/D2-47(a)); đột biến (selfbreak KT-80);
                               soak 3 arch (D2-24/D2-149). Mỗi lần chạy sinh
                               `v2/agent/reports/STATUS.md` + log tóm tắt máy sinh (QD Q12).
  ./v2/bin/v2 licenses [--check|--generate|--selftest]
                               KT-333b/D2-21: allowlist giấy phép QD §9.3 (`deny.toml`),
                               kiểm mọi dep trong `Cargo.lock`/metadata + nguồn registry,
                               sinh `THIRD-PARTY.md` từ `cargo metadata` (không NOTICE).
  ./v2/bin/v2 miri [test|run|setup|clean] [tuỳ chọn]
                               Miri qua `cargo +nightly miri` trên v2/Cargo.toml; mặc định `test`
                               (D2-47: lệnh rời, KHÔNG nằm trong gate --fast)
  ./v2/bin/v2 rules [--root D] chạy 5 luật cấu trúc + kiểm tên tệp Windows/NTFS (E1)
                               trên D (mặc định v2/); E1 kiểm **tệp đã theo dõi Git**
  ./v2/bin/v2 check-rules [--law N] [--self-test] [--root R]
                               KT-80 / OD-K6-5: 5 luật máy (1, 2, 3, 7, 8) — số do script sinh từ
                               log đã commit; log commit trước báo cáo; bộ đếm "0" phải có ca âm;
                               bản trích tiêu chí đủ cột; ca phá cũ là ca hồi quy. Được
                               `v2 check` / `gate --fast` gọi tự động.
  ./v2/bin/v2 mutants [id] [--list|--lint|--all|--selftest]   chạy đột biến (manifest v2/test/mutants.toml)
  ./v2/bin/vifast-trace <tệp> [--json] [--kind K]            giải mã dump trace nhị phân 32 B (D2-09)
  ./v2/bin/vifast-bench --smoke [--arch A] [--csv F]          đo smoke dưới QEMU -icount (D2-259)
  ./v2/bin/v2 status [--json]
                               KT-102: một màn hình trạng thái (HEAD, dấu vân tay build-inputs,
                               ảnh mới nhất, đếm task). Chỉ đọc, không build.
  ./v2/bin/v2 check-repro [--negative]
                               KT-79 / OD-K6-3: build ảnh sản phẩm ở **2 đường dẫn tuyệt đối
                               khác nhau** ⇒ phải CÙNG sha256. `--negative` = ca âm (tắt
                               --remap-path-prefix) ⇒ phải ĐỎ. KHÔNG nằm trong gate --fast
                               (2 build lạnh ~2 phút); xem v2/docs/k6-repro.md

Luật: R1 tệp kernel/lib/sim/vifast-sim/src ≤1.500 dòng (KT-336b); R2 cấm cfg(target_arch) trong kernel/vifast-core + lib;
      R3 SPDX theo loại tệp; R4 cấm path-dependency sang crates/ (D2-26);
      R5 unsafe chỉ trong danh sách đóng (D2-17).
L1 đã bật từ KT-08: 1.000 seed trong `gate --fast`; `v2 nightly --all` chạy L1-full 400k (K6) +
Miri + đột biến + soak — KHÔNG tự chạy (phải nêu cờ; AGENTS.md §6).
Giấy phép: `v2 licenses --check` nằm trong `v2 check` (D2-21, D2-37(c)); `THIRD-PARTY.md` máy sinh.
Bootstrap Miri (khi `v2 miri` báo thiếu toolchain): rustup component add miri rust-src --toolchain nightly
EOF
}
