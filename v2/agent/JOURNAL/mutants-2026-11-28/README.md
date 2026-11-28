# `v2 mutants --all` — nghiệm thu bản vá công cụ (2026-11-28)

- **Lệnh**: `python3.12 v2/tools/mutants.py --root . --all --timeout 600`
- **Cây**: `main @82502f7d` (T2 PASS trước đó; worktree chỉ có thêm tệp **chưa theo dõi** dưới
  `v2/agent/JOURNAL/` sao chép bằng chứng — **không** phải đường build).
- **Thời gian**: 12:13:09 → 14:23:55 UTC (2 h 10 m 46 s).
- **Kết quả** (`mutants-all.log:37`):
  `chạy=36 killed=30 survived=0 wrong-reason=6 stale=0 chưa-nối=0`
- **Vì sao chạy lại**: lần 27/11 dừng giữa chừng vì `mutants.py:480 TypeError: can't concat str to
  bytes` (bug công cụ) + `mut-info-x1 E_WRONG-REASON` (thiếu dấu phẩy `patches.replace` trong
  `mutants.toml`). Lần này **không** còn `TypeError`, `mut-info-x1` **KILLED 88,8 s** và
  `mut-no-x0-out` **KILLED 105,3 s** ⇒ hai bản vá đã được nghiệm thu ở lượt chạy thật.

## 6 mục `E_WRONG-REASON` — giới hạn **ngân sách**, không phải đột biến sống

Cả 6 đều `suite = "ktest-riscv64"` (ma trận 27 boot: `NBOOT × SMP × ảnh`, mỗi boot có ngân sách
timeout riêng; một đột biến làm boot treo ⇒ mọi boot tiêu hết ngân sách ⇒ lượt chạy vượt 600 s):
`mut-leak-unlock`, `mut-no-irq`, `mut-no-lock`, `mut-smp-set-harts-reset-all`,
`mut-smp-no-refresh-published`, `mut-smp-publish-any-hart`.

Bằng chứng phân loại: `rc=124` (timeout của **công cụ**), `marker=KHONG`, `survived=0`. Với
`--timeout 1800` (mặc định) lần trước, cùng nhóm này vẫn bị cắt — nghĩa là ma trận riscv64 dưới
đột biến treo cần **> 1.800 s**, không phải "bộ kiểm không bắt được".

**Nợ phải đóng (không được gọi là `--all` xanh)**: chạy lại **riêng 6 mục** với
`--timeout 2400` (hoặc tách suite riscv64 thành ma trận ngắn hơn **có chủ ý** + ghi lý do) để có
`KILLED` thật; kết quả sẽ bổ sung vào tệp này. Không nới marker/không đổi `expect`.

## Ghi chú kỹ thuật

- `--timeout 600` là **ngân sách công cụ**, không phải ngưỡng cổng: rút ngắn làm phép thử
  **chặt hơn** (timeout ⇒ không được tính KILLED nếu thiếu marker), không phải nới để lấy xanh.
- Lý do dùng 600 thay vì 1800: `mut-leak-unlock` (và 5 mục cùng suite) treo ma trận > 30 phút;
  1800 s chỉ làm lượt chạy dài vô ích mà kết quả không đổi (vẫn timeout).
