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

## 6 mục `E_WRONG-REASON` — **không phải** đột biến sống, mà là **lỗi khoá build** (đã vá)

Cả 6 đều `suite = "ktest-riscv64"` (`mut-leak-unlock`, `mut-no-irq`, `mut-no-lock`,
`mut-smp-set-harts-reset-all`, `mut-smp-no-refresh-published`, `mut-smp-publish-any-hart`).

**Nguyên nhân thật (đo được, 28/11 17:00 UTC)**: `ktest_build_lock()` (`v2/test/boot/lib/
ktest-build.sh`) vòng `mkdir "$KTEST_BUILD_LOCK"` trên `<repo>/v2/kernel/target/.ktest-build.lock`.
Bản sao worktree/mutant **không có** `target/` ⇒ `mkdir` trả **ENOENT** (không phải EEXIST) ⇒ vòng
chờ đủ **900 s** rồi `return 1` **trước khi build/boot** (log `# infra before … boot_done=0`, không
có QEMU). Vì vậy `--timeout 600` cắt thành timeout; tăng lên 2400 chỉ kéo dài vô ích — **không
phải** "ma trận 27 boot chậm" như giả thuyết ban đầu.

**Vá** (`a842091d`): `mkdir -p "$(dirname "$KTEST_BUILD_LOCK")"` một lần trước vòng; không tạo được
thì `return 1` (fail-closed). Chi tiết + probe cô lập: `JOURNAL/ktest-build-lock-fix.md`.

**Kết quả sau vá** (cùng 6 mục, `--timeout 900`, trên cây có vá):

| Mục | Kết quả |
|---|---|
| `mut-leak-unlock` | **KILLED 161,6 s** |
| `mut-no-irq` | **KILLED 160,3 s** |
| `mut-no-lock` | **KILLED 158,7 s** |
| `mut-smp-set-harts-reset-all` | **KILLED 160,5 s** |
| `mut-smp-no-refresh-published` | **KILLED 159,3 s** |
| `mut-smp-publish-any-hart` | **KILLED 158,7 s** |

⇒ **6/6 KILLED**, 0 `SURVIVED`, 0 `E_WRONG-REASON`.

## Lượt `--all` đầy đủ trên HEAD cuối (`f5f39f6d`) — **36/36 KILLED**

- **Lệnh**: `python3.12 v2/tools/mutants.py --root . --all --timeout 900`
- **Thời gian**: 18:17:33 → 19:42:06 UTC (84 m 33 s).
- **Kết quả** (`mutants-all-final-36-killed.log:37`):
  `mutants: chạy=36 killed=36 survived=0 wrong-reason=0 stale=0 chưa-nối=0` — **lượt `--all` sạch đầu
  tiên** sau khi vá hai lỗi công cụ (`f1c59c5d`) + lỗi khoá build (`a842091d`).
- Đây là bằng chứng mutation trên cây hợp nhất cuối (cùng `build-inputs=b15fc2f60a7d436b` với T2
  `cc405ead`/`f5f39f6d`); không còn nợ `E_WRONG-REASON`.

## Ghi chú kỹ thuật

- `--timeout 600` là **ngân sách công cụ**, không phải ngưỡng cổng: rút ngắn làm phép thử
  **chặt hơn** (timeout ⇒ không được tính KILLED nếu thiếu marker), không phải nới để lấy xanh.
- Lý do dùng 600 thay vì 1800: `mut-leak-unlock` (và 5 mục cùng suite) treo ma trận > 30 phút;
  1800 s chỉ làm lượt chạy dài vô ích mà kết quả không đổi (vẫn timeout).
