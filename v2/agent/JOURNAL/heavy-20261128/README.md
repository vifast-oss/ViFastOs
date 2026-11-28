# Chạy nặng trên cây hợp nhất `2d9135f3` — 2026-11-28 (15:33–16:14 UTC)

Bằng chứng nặng **duy nhất** đạt trên HEAD sau khi hợp nhất 3 nhánh + hotfix vòng 2 (thay cho
`JOURNAL/heavy-20261127/` vốn đo trên `44e60e67` — **không** tái dùng được, §6.3).

- **Commit**: `2d9135f3` (cây đo sạch `dirty=0`; ảnh in `# build: algo=kt134-v5
  inputs=d428c8149c3e86e9` — khớp `# measure-key` của T2 `5f48f92a`).
- **Toolchain**: Rust `1.100.0` (ghim).
- **Máy**: 12 core, không QEMU lạ (`qemu_foreign=0`), không agents chạy nền.
- **Thứ tự** (một tiến trình, không lồng build): hardening-max → product 3 arch → soak-aarch64
  smoke → soak-x86_64 600 → l1-tier full.

| Phép đo | Kết quả | Log |
|---|---|---|
| `hardening-max.sh` | **PASS** (`ok 1` x86_64 `-cpu max` SMEP/SMAP + 2 ca âm + cross fault + preempt CPL3; `ok 2` `-cpu max,-smap` đối chứng 50/50 TAP; `ok 3` aarch64 `-cpu max` PAN + data abort) | `hardening-max.log` |
| `product-riscv64.sh` | **PASS** `checks=11 fails=0`, `REV-GATE ACCEPT` | `product-riscv64.log` |
| `product-aarch64.sh` | **PASS** `checks=10 fails=0` | `product-aarch64.log` |
| `product-x86_64.sh` | **PASS** `checks=11 fails=0`, `# conf image` khớp ELF | `product-x86_64.log` |
| `soak-aarch64.sh --smoke 2 1` | **2/2 boot sạch**, `o=2 fail=0`; **rc=1 theo hợp đồng SMOKE** (script luôn exit 1 để không ai dùng smoke làm bằng chứng cổng K4' — xem dòng `# soak: SMOKE N=2 < 1000 — KHÔNG phải bằng chứng cổng K4'`) | `soak-aarch64-smoke.log` |
| `soak-x86_64.sh 600 3` | **PASS** 600/600 boot sạch, `lỗi=0`, wall ~14,7 phút, `BUILD_ID=2dcb9ff1…` | `soak-x86_64-600.log` |
| `l1-tier.sh --tier full` | **PASS** 4 cấu hình `smp=1,2,4,8` × 100.000 seed = **400k**, 24 lô, wall **1395 s**, mọi lô xanh, digest đủ | `l1-tier-full.log` + `l1-full-logs/` |

## Ghi chú trung thực

1. **Soak aarch64 chỉ smoke 2 boot** (không phải soak dài): bằng chứng aarch64 nặng vẫn là nợ; lần
   chạy này chỉ chứng minh ảnh aarch64 mới **boot sạch** sau thay đổi.
2. `rc=1` của soak-aarch64 smoke **không phải đỏ**: đó là hợp đồng của chế độ SMOKE (exit 1 có chủ
   ý). Đừng trích dòng này làm "soak aarch64 PASS".
3. Không chạy `v2 nightly --all` (Miri + mutants + soak) trong đợt này; mutants đã chạy riêng
   (`JOURNAL/mutants-2026-11-28/`), Miri còn nợ.
4. Bằng chứng này đo trên `2d9135f3` — **trước** nhánh `KT-424` (`syscall` cho `Switched`). Nhánh đó
   chạm `coretest.rs`/`umode.rs`/`run-x86_64.sh` ⇒ bằng chứng nặng này **không** tự động áp cho HEAD
   sau khi merge `KT-424`; cần chạy lại nhóm liên quan (soak + l1) trên HEAD cuối — ghi rõ trong
   STATE.
