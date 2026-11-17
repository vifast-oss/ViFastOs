# KT-125 — thêm hai lệnh: `truncate <path> <size>` và `jobs`

**Ngày**: 2026-11-18 · **Ngân sách**: 45 phút · **Dùng**: ~25 phút.

## Vì sao làm được mà không cần Agent 0

Cùng lý do KT-114: đặt trong nhánh `#[cfg(shell_fs)]` ⇒ ảnh **test** giữ sáu lệnh, `proc_demo.rs` và
`ktest.sh` **không** phải sửa. `spec/v1-parity.toml` vốn đã linh hoạt (`cmds=[0-9]+`).

## Hai lệnh

| Lệnh | Nguồn | In ra |
|---|---|---|
| `truncate <path> <size>` | `OP_TRUNC` (0x14) — **đã có**, `write` đang dùng | `# shell: truncate path=<p> size=<n>` |
| `jobs` | **cùng** reply `PS_REQ` mà `ps` dùng — không thêm đường kernel | `# shell: jobs ready=<n> running=<n> blocked=<n> other=<n>` |

`jobs` giải mã trạng thái bằng **cùng bốn bit** như `cmd_ps` (bài học KT-105).

## `CMDS`: 13 → 15 (ảnh sản phẩm), ảnh test vẫn 6

## Log QEMU thật

```text
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/truncate/jobs/exit
# shell: help cmds=15
  help | echo <text> | ps | wait <pid> | kill <pid> | ls [path] | cat <path> | write <path> <text> | time | uptime | sync | stat <path> | truncate <path> <size> | jobs | exit
# shell: jobs ready=3 running=0 blocked=0 other=0
# shell: truncate path=/README size=10
# shell: stat path=/README kind=1 size=10 ino=2      ← truncate có hiệu lực thật
# shell: fs loi code=5                                ← truncate /khongco
# shell: thieu kich thuoc                             ← truncate /README (thiếu cỡ)
# shell: exit code=0
```

`jobs ready=3` khớp `ps n=3` (procsrv + shell + victim) — hai lệnh đọc **cùng** nguồn nên khớp là
đương nhiên; giá trị nằm ở chỗ `jobs` **đếm theo trạng thái**, thứ `ps` không làm.

## Kiểm

```text
./v2/bin/v2 check → PASS
bash v2/test/boot/shell-edge.sh --image <ảnh cũ> → 8/8 ca PASS (harness đọc CMDS từ log nên không gãy)
```

## Còn lại của nhóm A (theo KT-109)

`history`, `!!` — cần **trạng thái lịch sử** trong shell (buffer + con trỏ). Chưa làm: đây là tính
năng mới thật sự, không chỉ là nối opcode có sẵn, nên đáng một task riêng.
