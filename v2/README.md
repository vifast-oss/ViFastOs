# v2/ — ViFastOs v2

Cây làm việc của v2, **tách hẳn** workspace v1 (`../crates/`, tag `v1-archive`, đọc-only).
Luật chung: [`../AGENTS.md`](../AGENTS.md); phần riêng dòng OS: [`AGENTS.md`](AGENTS.md).
Điều khiển: [`agent/QUEUE.md`](agent/QUEUE.md), [`agent/STATE.md`](agent/STATE.md), [`agent/BLOCKED.md`](agent/BLOCKED.md), [`agent/DECISIONS.md`](agent/DECISIONS.md).

## Cây hiện có (vòng 1)

```
v2/
  Cargo.toml        workspace riêng (không phải member của workspace v1)
  LICENSE           tham chiếu ../LICENSE-MIT, ../LICENSE-APACHE (một bản ở gốc — QD §9.3)
  bin/              wrapper `v2` (KT-03) — hiện chưa có
  lib/              thư viện no_std chạy được trên host
    vifast-fs       port từ crates/vifast-fs@ca2eaf5 (D2-26)
    vifast-fsutil   port từ crates/vifast-fsutil@ca2eaf5 (D2-26)
  agent/            QUEUE/STATE/BLOCKED/DECISIONS + JOURNAL/ + reports/
```

Thư viện port giữ `SPDX-License-Identifier` và dòng `PORTED-FROM: crates/<tên>@ca2eaf5` (D2-26);
**không** có path-dependency nào sang `../crates/`.

## Lệnh

```bash
./v2/bin/v2 check          # KT-03 — chưa có
./v2/bin/v2 gate --fast    # KT-03 — chưa có
cargo build                # trong v2/ — dùng khi chưa có wrapper
```

**L1 chưa bật** cho tới KT-08 (chưa có sim) — xem `agent/STATE.md`.
