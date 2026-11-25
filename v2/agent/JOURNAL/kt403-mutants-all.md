# KT-403 — lượt `v2 mutants --all` đầu tiên (2026-11-25, Giờ Cao điểm 1)

- **Lệnh**: `./v2/bin/v2 mutants --all` (manifest `v2/test/mutants.toml`, `wired=24/24`)
- **Cây**: `main` @ `16a03de2` (D2-260), `dirty=1` (chỉ tệp untracked có sẵn + log của chính lượt chạy)
- **Máy**: `mps` (macOS aarch64, 12 core), QEMU 11.1.3, toolchain ghim `1.100.0`
- **Thời gian**: 2026-11-25 10:37–11:29 JST (01:37–02:29 UTC), ~52 phút
- **Log đầy đủ**: [`kt403-mutants-all-20261125.log`](kt403-mutants-all-20261125.log)
- **Kết quả**: `chạy=24 killed=17 survived=0 wrong-reason=7 stale=0 chưa-nối=0`, `rc=1`

## Bảng verdict (nguyên văn từ log)

| # | Mục | Verdict | Thời gian | Ghi chú |
|---:|---|---|---:|---|
| 1 | `selfbreak` | KILLED | 46,2 s | script tự bảo vệ cây thật |
| 2 | `mut-big-bss` | **E_WRONG-REASON** | 11,6 s | đỏ đúng lý do ngân sách, marker suite không khớp |
| 3 | `mut-df-round-up` | **E_WRONG-REASON** | 105,7 s | rev-gate `no-git` (bản sao không có `.git`) |
| 4 | `mut-drop-nb-no-waiter` | KILLED | 6,1 s | `test result: FAILED` |
| 5 | `mut-harness-marker` | KILLED | 81,6 s | 2/37 kiểm đỏ |
| 6 | `mut-info-r12` | KILLED | 278,2 s | |
| 7 | `mut-info-x1` | KILLED | 82,6 s | build-patch |
| 8 | `mut-ktest-driver` | KILLED | 0,4 s | đồ thị feature |
| 9 | `mut-leak-unlock` | **E_WRONG-REASON** | 108,8 s | rev-gate `no-git` |
| 10 | `mut-machine-bss` | KILLED | 79,3 s | 1/37 kiểm đỏ |
| 11 | `mut-net-one-caller` | KILLED | 220,4 s | `product-riscv64: fails=5` |
| 12 | `mut-no-int80` | KILLED | 277,9 s | 3/37 kiểm đỏ |
| 13 | `mut-no-ipi-invlpg` | KILLED | 279,1 s | |
| 14 | `mut-no-irq` | **E_WRONG-REASON** | 108,4 s | rev-gate `no-git` |
| 15 | `mut-no-irq-restore` | KILLED | 80,0 s | |
| 16 | `mut-no-lock` | **E_WRONG-REASON** | 107,9 s | rev-gate `no-git` |
| 17 | `mut-no-netsrv` | **E_WRONG-REASON** | 108,5 s | rev-gate `no-git` |
| 18 | `mut-no-shootdown` | KILLED | 280,0 s | |
| 19 | `mut-no-svc` | KILLED | 81,0 s | |
| 20 | `mut-no-wait-drain` | KILLED | 267,0 s | |
| 21 | `mut-no-x0-out` | KILLED | 41,2 s | build-patch |
| 22 | `mut-pt-bss` | **E_WRONG-REASON** | 6,1 s | đỏ đúng lý do ngân sách, marker suite không khớp |
| 23 | `mut-shared-peer` | KILLED | 259,0 s | |
| 24 | `mut-state-before-cap` | KILLED | 4,9 s | `test result: FAILED` |

**Không có `SURVIVED`**: 17/17 mục chạy được đều bị bộ kiểm bắt (KILLED). 7 mục
`E_WRONG-REASON` chia đúng **hai** nguyên nhân dưới đây — cả hai là lỗ hổng của **công cụ/manifest**,
không phải mã kernel.

## Phát hiện 1 — rev-gate KT-86 chặn 5 mục riscv64 (lỗ hổng `mutants.py`)

`_copy_tree` trong [`v2/tools/mutants.py`](../../tools/mutants.py) **cố ý bỏ `.git`**
(`ignore_patterns("target", ".git")`). Suite `ktest-riscv64` chạy `v2/test/boot/ktest.sh`, mà
`ktest.sh` gọi cổng `rev-gate` (KT-86/D2-125.3) đòi `rev=HEAD` + `dirty=0`; bản sao không có git
⇒ `REV-GATE: REFUSE reason=no-git` ⇒ runner `exit 3` **trước khi chạy QEMU**. Marker suite
(`^not ok |^FAIL|không khớp`) không khớp dòng `TỪ CHỐI ĐO` ⇒ `E_WRONG-REASON`.

Ảnh hưởng: `mut-df-round-up`, `mut-leak-unlock`, `mut-no-irq`, `mut-no-lock`, `mut-no-netsrv`
(5/5 mục dùng suite `ktest-riscv64`).

**Đề xuất vá** (không nới cổng): sau khi áp đột biến, tạo git tree tạm **trong bản sao**
(`git init && git add -A && git commit`) và copy `.gitignore` gốc để `target/` không làm bẩn;
khi đó `rev=HEAD`, `dirty=0`, dấu vân tay build-inputs của ảnh = của cây đã commit ⇒ rev-gate
chấp nhận **thật**, không cần miễn trừ. Cần một task riêng (chạm `v2/tools/mutants.py` + test
công cụ), không gộp vào KT-401/407/414.

## Phát hiện 2 — 2 mục ngân sách đỏ đúng lý do nhưng thiếu marker riêng (lỗ hổng manifest)

- `mut-big-bss` (aarch64): `budget-aarch64: ảnh SẢN PHẨM VƯỢT 98304 > 65536 byte`
- `mut-pt-bss` (x86_64): `budget-x86_64: VƯỢT TRẦN 76128 ≥ 65536 byte`

Cả hai bị **kiểm ngân sách** bắt trước khi boot — đúng mục đích đột biến — nhưng marker chung của
suite `ktest-aarch64`/`ktest-x86_64` không phủ dòng `budget-*`. **Đề xuất**: thêm `marker` riêng
cho hai mục trong `v2/test/mutants.toml`, ví dụ `marker = "budget-(aarch64|x86_64): .*VƯỢT"`.

## Đề xuất cho Agent 0

Mở **một** task vá công cụ đột biến (gộp cả hai phát hiện, ~1–2 h, 0 chạm `v2/kernel/`):
sửa `v2/tools/mutants.py` (git tree tạm cho bản sao) + `v2/test/mutants.toml` (2 marker ngân sách),
rồi chạy lại `v2 mutants --all` kỳ vọng **24/24 KILLED**. Đây là điều kiện để log đột biến dùng
được làm bằng chứng K6 (hiện 7/24 mục chưa có verdict hợp lệ).
