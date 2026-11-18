# KT-140 — log ca âm `persist` (`--red-selftest`), chép vào repo

**Vì sao có thư mục này:** `v2/docs/k6-persist.md` §3.1 khẳng định bộ đếm của **ca âm** —
"`match` bằng không và `mismatched` khác không" — nhưng trước KT-140 **không khối nào** trong tài
liệu trích hai giá trị ấy. Log gốc nằm ở `/tmp` (không vào git), nên khẳng định đó là **lời khai
không có bằng chứng trong repo**. Agent 0 yêu cầu chạy lại ca âm và đưa dòng log vào tài liệu thành
khối `scope-red=`.

**Lệnh đã chạy (đúng một lệnh QEMU, theo yêu cầu của Agent 0):**

```bash
bash v2/test/boot/persist.sh --red-selftest
```

- Ngày chạy: 2026-11-18 (giờ máy, Asia/Tokyo), bắt đầu `2026-11-18T04:53:21Z`.
- Exit code: **0** (`# persist.sh: PASS`).
- `LOGDIR` mặc định của runner: `/tmp/kt75-persist` (không đặt `KT75_LOGDIR` để giữ **đúng** lệnh
  Agent 0 cho phép).
- Ảnh: `vifast-kbin-ktest-persist`, `file=221248 data+bss=27352 budget=262144` (dòng `# image=` trong
  `run.log`; **không** nới trần).

**Cơ chế ca âm** (`red_selftest` trong `v2/test/boot/persist.sh`): boot `b01` ghi pha 1 xanh; host
ghi đè `/p1.txt` bằng 512 B `0xAA` (**payload sai**) qua `vifast-fsutil put-file`; boot `b02` đọc
lại pha 1 ⇒ bộ kiểm **phải** đỏ. Đây là ca chứng minh bộ kiểm **không rỗng** (D2-108B), không phải
phép đo hiệu năng.

**Dòng log được trích vào `k6-persist.md`** (nguyên văn, từ `b02.log` dòng 69):

```text
# persist: phase=1 action=verify files=1 bytes=512 read=512 absent=0 match=0 sha_prefix=799edf40 mismatched=510 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
```

`match=0` và `mismatched=510` (510/512 byte lệch — đúng **hai** byte của payload gốc tình cờ bằng
`0xAA`; kiểm được bằng công thức `payload(1, 512)` trong `v2/test/boot/persist_host.py`) — đúng
khẳng định của §3.1. TAP tương ứng: `not ok 1 - persist verify phase 1: byte đọc lại LỆCH / băm lệch
/ nhật ký pha hỏng`.

**Tệp trong thư mục:**

| Tệp | Nội dung |
|---|---|
| `run.log` | stdout đầy đủ của `persist.sh --red-selftest` (gồm `# image=`, `# red-selftest: PASS`) |
| `b01.log` | log QEMU boot ghi pha 1 (xanh) — `wrote=512 recorded=1 fsync=1 durable=1` |
| `b02.log` | log QEMU boot đọc lại pha 1 **sau khi payload bị phá** — `match=0 mismatched=510` |
| `b01.plan`, `b02.plan` | kịch bản runner ghi vào LBA đầu tiên trước mỗi boot |
| `b01.plan.log`, `b02.plan.log` | log `log-check` của runner cho từng boot |
| `b01.check-write.log`, `b01.host.log` | kiểm host **trước** khi phá payload (phải xanh) |
| `kt75-budget.log` | ngân sách ảnh persist (không nới trần) |

**Không** chép `b02.host.log`/`b02.check-verify.log` của lần chạy này vì `red_selftest` ghi kết quả
kiểm host **sau khi phá** vào `run.log` (biến `$plan`), không vào `b02.host.log`; tệp cùng tên trong
`/tmp/kt75-persist` là **log cũ** của lần chạy đủ 10 boot (07:51), không phải của ca này.
