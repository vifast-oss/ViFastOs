# AGENTS.md — dòng ViFastOs v2 (tóm tắt)

Bản đầy đủ: [`../AGENTS.md`](../AGENTS.md). Tệp này chỉ thêm phần riêng của `v2/`.
Nguồn sự thật của vòng vá: [`../docs/QUYET-DINH-sau-review.md`](../docs/QUYET-DINH-sau-review.md) (**QD**).

## 1. Thứ tự nguồn sự thật (áp cho v2)

1. **Code + harness** trong `v2/`: crate chạy được, sim, `.scn`, TAP, log cổng.
2. **`spec/`** — `spec/abi.toml`, `spec/kernel.toml`: một nguồn sự thật duy nhất; code sinh từ spec không sửa tay (D2-11).
3. [`agent/DECISIONS.md`](agent/DECISIONS.md) — D-01..D-08 (chỉ v1) + **D2-01..D2-76**.
4. Tài liệu thiết kế [`../docs/v2/ViFastOs-v2-thiet-ke.md`](../docs/v2/ViFastOs-v2-thiet-ke.md) — tham chiếu, không phải hiện trạng.

QD thắng tài liệu thiết kế. Số đo chỉ do `./v2/bin/v2 nightly` sinh.

## 2. Tài liệu thiết kế và điều khiển

| Tệp | Vai trò |
|---|---|
| [`../docs/v2/ViFastOs-v2-thiet-ke.md`](../docs/v2/ViFastOs-v2-thiet-ke.md) | thiết kế đầy đủ (mục 3 kiến trúc, 4 kernel, 9 kiểm thử, 10 lộ trình K0→K6, 11 quy trình) |
| [`../docs/v1/KNOWN-ISSUES.md`](../docs/v1/KNOWN-ISSUES.md) | 7 lỗi v1 mang sang v2 + ca kiểm tương ứng |
| [`agent/QUEUE.md`](agent/QUEUE.md) | 49 dòng: vòng 1 + K1a' + K1b + K1c (DONE) + **K2** (KT-35..KT-43) |
| [`agent/DECISIONS.md`](agent/DECISIONS.md) | D-01..D-08 (v1) + D2-01..D2-76 |
| [`agent/STATE.md`](agent/STATE.md) | HEAD, BUILD_ID, task, bước — Agent 2 cập nhật sau mỗi bước |
| [`agent/BLOCKED.md`](agent/BLOCKED.md) | task bị chặn + câu hỏi cho Agent 0 |
| `agent/JOURNAL/`, `agent/reports/` | nhật ký ngày, `R<n>-v2.md` + `R<n>-v2.json` |

## 3. Trạng thái duyệt hiện tại

- **KT-04d = APPROVED 2026-11-12 Agent 0**: đã duyệt checklist 5 mục/method + 21 kịch bản ABI + draft chữ ký (D2-35..43). **KT-05 đã mở** (`vifast-gen` + gen-check); sau duyệt chỉ Agent 0 sửa `spec/`. T04 cũ = KT-04d.
- Câu hỏi O1 của vòng review (**Exit** vào ABI bằng gì) **đã chốt** trong QD §1 Q-A: syscall **14 `Exit(code)`**: lưu `exit_code`, TCB `BlockedOnSend` trên `fault_ep` mang `FaultKind::Exit{code}` (không Reply); procsrv `Recv` xong mới `Inactive`; `wait`/`kill` là giao thức procsrv. Không mở lại.
- `./v2/bin/v2 check` và `./v2/bin/v2 gate --fast` **đã có từ KT-03** (`gate --fast` PASS); `gen-check` bật từ KT-05, L1 từ KT-08.

## 4. Riêng của dòng OS

- Lộ trình không ngày: K0 → **K0.5 boot riscv64** → K1a (sim store giả + CNode/MDB) → K1a' (objmem + Miri) → K1b (TCB, scheduler, MCS donation) → K1c (IPC/Reply/Notification/WaitAddr/Exit) → K2 VM → K3 riscv64 đủ → K3.5 procsrv + shell Rust → K6 tương đương v1 → K4'/K5' (aarch64, x86_64, UEFI).
- **Chỉ riscv64 tới K6** (D2-10); 2 arch còn lại chỉ giữ số reserved trong `abi.toml`.
- Trace **luôn bật**, không có cờ tắt (D2-09). Test kernel **không** nằm trong boot sản phẩm (D2-08).
- Cổng cứng `.data + .bss ≤ 256 KiB` [TK], đo ở K0.5 (QD §3.5).
- **L1 đã bật** từ KT-08 trong `gate --fast`: sim 1.000 seed; nightly 100.000 seed.
- Cấm `pkill -f qemu-system` toàn cục: chỉ `kill <pid>` của chính task. Tối đa 3 QEMU song song.
- Ngân sách 3 giờ/task; vượt thì dừng, ghi tiến độ, chuyển task không phụ thuộc.

## 5. Lệnh

```bash
./v2/bin/v2 check
./v2/bin/v2 gate --fast
./v2/bin/v2 nightly
```

Lệnh nằm ở `v2/bin/v2`; có thể `export PATH="$PWD/v2/bin:$PATH"` để gõ `v2 …`.

Commit: `[KT-##] <tóm tắt>`. Push sau mỗi commit xanh: `git push origin main`. Duyệt: dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `agent/QUEUE.md`.
