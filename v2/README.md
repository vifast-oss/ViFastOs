# v2/ — ViFastOs v2

Cây làm việc của v2, **tách hẳn** workspace v1 (`../crates/`, tag `v1-archive`, đọc-only).
Luật chung: [`../AGENTS.md`](../AGENTS.md); phần riêng dòng OS: [`AGENTS.md`](AGENTS.md).
Điều khiển: [`agent/QUEUE.md`](agent/QUEUE.md), [`agent/STATE.md`](agent/STATE.md),
[`agent/BLOCKED.md`](agent/BLOCKED.md), [`agent/DECISIONS.md`](agent/DECISIONS.md).

## Cây hiện có (sau KT-330)

```text
v2/
  Cargo.toml           workspace riêng (không phải member của workspace v1)
  rust-toolchain.toml  ghim Rust 1.100.0 (bản sao cho workspace kernel ở kernel/)
  bin/                 lệnh cổng `v2` + `vifast-gen` + `vifast-idl`
  kernel/              vifast-core + vifast-plat-{common,riscv64,aarch64,x86_64,user}
                       + vifast-kbin-{riscv64,aarch64,x86_64} (ảnh sản phẩm & ảnh test)
  lib/                 thư viện no_std dùng chung: abi, alloc, elf, fs, fsutil, idl,
                       model, net, objmem, pte, ring, scenario, sync, traceformat,
                       vfs, virtio
  sim/                 vifast-sim — mô phỏng tất định
  test/                boot/ parity/ scn/ abi/ rules/ conformance/ report/ trace/
                       fixtures/ gen/ + runner QEMU
  tools/               checker, git-guard, build/repro, L1 tier, ghim thuật toán…
  docs/                tài liệu kỹ thuật K1..K6
  agent/               QUEUE/STATE/BLOCKED/DECISIONS + JOURNAL/ + reports/ + archive/
```

Thư viện port từ v1 giữ `SPDX-License-Identifier` và dòng `PORTED-FROM: crates/<tên>@ca2eaf5` (D2-26);
**không** có path-dependency nào sang `../crates/`.

## Lệnh

```bash
export PATH="$HOME/.cargo/bin:$PATH"
./v2/bin/v2 check                  # fmt + clippy + luật + checker (không chạy QEMU)
./v2/bin/v2 gate --dev             # T1: kiểm tích hợp nhanh — KHÔNG phải full gate
./v2/bin/v2 gate --fast            # T2: cổng đầy đủ trên ứng viên tích hợp
./v2/bin/v2 status                 # HEAD, dấu vân tay build-inputs, ảnh mới nhất
./v2/bin/v2 check-rules [--law N]  # 5 luật máy K6 (1, 2, 3, 7, 8)
bash v2/tools/l1-tier.sh --tier smoke|medium|full
```

- Ba tầng kiểm T0/T1/T2: `AGENTS.md` §6.1; ba mức L1: §6.2; tái dùng bằng chứng: §6.3.
- `gate --fast` là **T2** (cổng đầy đủ hiện có) — chạy trên ứng viên tích hợp / bàn giao.
- `v2 nightly` đầy đủ (L1 100k/mutation/soak) **đang được dựng** (`KT-333b`); hiện có `--only-miri`
  và `--l1`.
- Toolchain ghim `1.100.0` ở `v2/rust-toolchain.toml` + `v2/kernel/rust-toolchain.toml`;
  `v2/kernel/Cargo.toml` còn `rust-version = "1.99.1"` — **lệch đã biết**, `KT-334` đồng bộ (D2-231).

## Trạng thái

- Khối trạng thái hiện tại: [`agent/STATE.md`](agent/STATE.md).
- Lịch sử KT-01..KT-329: [`agent/archive/STATE-chu-ky-1.md`](agent/archive/STATE-chu-ky-1.md).
- Lịch sử mục BLOCKED đã đóng: [`agent/archive/BLOCKED-chu-ky-1.md`](agent/archive/BLOCKED-chu-ky-1.md).
- Kế hoạch hiện hành (Đại tu Kiến trúc `KT-400..KT-424`, `D2-259`): [`../KE-HOACH-KIEN-TRUC.md`](../KE-HOACH-KIEN-TRUC.md).

## Cổng chạy trên máy — **không** GitHub Actions

Repo **không** dùng `.github/workflows/**` và sẽ không dùng (D2-241). Cổng:
`./v2/bin/v2 check` → `gate --dev` (T1) → `gate --fast` (T2) → `nightly`. Lý do: bước QEMU nhiều
phút + ngưỡng thời gian/đo đạc chỉ đúng trên máy đo; CI ngoài sinh bằng chứng không hợp lệ mà vẫn
xanh. Chạy định kỳ: script + hẹn giờ cục bộ.
