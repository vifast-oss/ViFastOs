# Lô Thấp điểm chính — 2026-11-25 10:00 UTC → 2026-11-26 01:00 UTC

- **Mục tiêu** (chỉ đạo người điều phối): đang giờ thấp điểm (0,5×) ⇒ triển khai **task song song**
  tới **giờ cao điểm kế tiếp thì dừng** (`D2-256` / `AGENTS.md` §6.4). Khung: `10:00 UTC 25/11`
  → `01:00 UTC 26/11` (15 h liên tục, T3). Cao điểm kế: `01:00–04:00 UTC 26/11` (T4).
- **Cây**: `main` @ `c33ad729` (bằng chứng Cao điểm 2; mã `a61921b4`), `dirty=0`.
- **Máy**: `mps` (macOS aarch64, 12 core), QEMU 11.1.3, toolchain ghim `1.100.0`.
- **Luật áp**: sub-agent chỉ **T0 + T1**; Lead merge + **T2** gộp lô; không nới ngưỡng; không retry
  lấy xanh; tái dùng bằng chứng §6.3; **không** GitHub Actions (§4.11).

## 1. Bố trí 4 slot (`v2/tools/wt.sh`, D2-245)

| Slot | Task | Nhánh | Worktree | Cache build |
|---:|---|---|---|---|
| 1 | `KT-404` — workspace Cargo thật `v2/user/` + `vifast-kbuild` + `N-7` + 9 crate `forbid(unsafe_code)` | `task/kt404-user-workspace` | `/private/tmp/vifast-wt/1` | `target-task/slot-1` |
| 2 | `KT-408` — trait `KernelStore` + `Machine<S,O>` + ring trace 32 B kernel | `task/kt408-kernelstore-ring` | `/private/tmp/vifast-wt/2` | `target-task/slot-2` |
| 3 | `KT-402b` — harness `aarch64 -dtb` + `x86_64 fw_cfg/table-loader` + ca âm 3 arch | `task/kt402b-bootinfo-harness` | `/private/tmp/vifast-wt/3` | `target-task/slot-3` |
| 4 | `KT-414b` — UEFI `aarch64` qua AAVMF + `BootSource::Uefi` | `task/kt414b-uefi-aarch64` | `/private/tmp/vifast-wt/4` | `target-task/slot-4` |

- **Khoá T1**: `/tmp/vifast-t1-lock.sh` — chỉ **một** `gate --dev` chạy trên máy tại một thời điểm
  (T1 có QEMU smoke + L1 1.000 seed; §9 tối đa 3 QEMU song song, thực tế giữ 1 để số đo không nhiễu).
- **Xung đột tệp đã biết**: `KT-402b` và `KT-414b` cùng chạm `v2/test/boot/run-aarch64.sh` (khác hunk:
  `-dtb` vs `--firmware uefi`) — Lead giải khi merge. Các cặp còn lại 0 chồng tệp theo `KE-HOACH-KIEN-TRUC.md`.
- **Tiền đề `KT-414b`**: `aarch64-unknown-uefi` + `x86_64-unknown-uefi` đã cài cho `1.100.0` (đã kiểm);
  AAVMF `edk2-aarch64-code.fd` có sẵn trong `/opt/homebrew/share/qemu/`.

## 2. Tiến độ

| Mốc (UTC) | Việc | Kết quả |
|---|---|---|
| 10:00 | Cấp 4 slot + khởi chạy 4 sub-agent | 4/4 chạy |
| 10:04–11:20 | 4 sub-agent làm task (T0 + T1 qua khoá) | 4/4 xong, 0 commit (đúng luật: agent không commit) |
| 11:20–11:35 | Lead commit từng nhánh + merge `--no-ff` vào `main` | 4/4 merge sạch (`7b970aea`), 0 xung đột |
| 11:35 | **T2 lần 1** (`gate --fast`) trên ứng viên gộp | **ĐỎ** đúng 1 bước `wake dep-info`: `forbid(unsafe_code)` (KT-404) × `link_section` (KT-408) |
| 11:45 | **Merge fix**: chuyển kho ring `.corespace` `vifast-traceformat` → `vifast-sync` (D2-17) | `f3cf3c97`; `v2 check` PASS; kernel workspace `cargo check` RC=0 |
| 11:55 | **T2 lần 2** trên ứng viên đã vá | **PASS 79/79**, 647,3 s |
| 12:05 | `L1-medium` 10k (KT-408) | xem §3 |
| 01:00 (26/11) | **Dừng** theo lịch | — |

## 3. Bằng chứng

### 3.1 T1 từng nhánh (sub-agent, qua khoá `/tmp/vifast-t1-lock.sh`)

| Task | T1 | Ghi chú |
|---|---|---|
| `KT-404` | **PASS** 705,6 s (`registered=80 ran=66 skipped=14 failed=0`) | chạy trên bản sao sạch (worktree bẩn làm `api boundary`/`vbrev` fail-closed) |
| `KT-408` | 46/47 — đỏ duy nhất `báo cáo R1-v2` | đỏ do bản sao git hỏng; `check_report.py --all` **PASS** trên worktree thật |
| `KT-402b` | 583,7 s — 64 làn OK, 1 đỏ `selftest pre-measure` | đỏ do `refuse 7 tree-dirty` trên worktree bẩn; PASS trên cây sạch |
| `KT-414b` | **PASS** 652,8 s | — |

### 3.2 T2 trên ứng viên gộp

- **Lần 1** (`7b970aea`): **ĐỎ** — `wake theo dep-info` → `cargo check` kernel workspace RC=101:
  `error: usage of the unsafe link_section attribute` (`vifast-traceformat/src/kernel_ring.rs`).
  Nguyên nhân: KT-404 bật `#![forbid(unsafe_code)]` cho `vifast-traceformat`; KT-408 đặt kho ring
  (cần `link_section`) trong chính crate đó. Hai nhánh xanh riêng lẻ, gộp mới lộ.
- **Merge fix** (`f3cf3c97`): `kernel_ring.rs` → `vifast-sync` (crate thấp nhất trong danh sách
  đóng D2-17 mà `vifast-core` được phụ thuộc); `vifast-traceformat` giữ `forbid(unsafe_code)`;
  `vifast-core` thêm dep `vifast-sync`. 0 đổi định dạng 32 B / `size_of::<Machine>()` / `.corespace`.
- **Lần 2** (`f3cf3c97`): **PASS 79/79**, `step-time-total 647273 ms`; log `/tmp/t2-thapdiem-20261125-r2.log`.

### 3.3 Kiểm phụ sau vá

`cargo check` kernel workspace **RC=0**; `vifast-core` **112/112**, `vifast-sync` **19/19**,
`vifast-traceformat` **9/9**; `v2 check` **PASS** (84,1 s); `check-lockfile-sync` **OK**;
`check_report.py --all` **PASS**.

### 3.4 L1-medium (KT-408)

`bash v2/tools/l1-tier.sh --tier medium` — **PASS**: 4 cấu hình smp × 6 lô = **24 task**, `wall 167 s`,
phủ đủ 10.000 seed/cấu hình (`== l1-tier: PASS (mọi lô xanh, phủ đủ seed) ==`); log
`/tmp/l1-medium-thapdiem-20261125.log`.
