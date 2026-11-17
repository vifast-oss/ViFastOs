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

## Sự cố khi build ảnh test (đã vá)

Lần build đầu, `ktest-con` **FAIL**:

```text
error[E0425]: cannot find function `cmd_truncate` in this scope
  note: found an item that was configured out
     --> init/shell_user.rs:254  #[cfg(shell_fs)] fn cmd_truncate(...)
```

Nguyên nhân: tôi đặt hai nhánh dispatch `jobs`/`truncate` trong chuỗi `if/else` **chính**, nhưng hai
hàm lại bị `#[cfg(shell_fs)]` loại ở ảnh test. **Vá**: chuyển hai nhánh vào `fs_dispatch` (hàm đã có
`#[cfg(shell_fs)]`).

**Bài học**: thêm lệnh vào shell phải sửa **ba** chỗ cùng một `cfg` — hàm, `CMDS`/`help`/banner, **và
chỗ dispatch**. Tôi quên chỗ thứ ba; ma trận bắt được ngay.

## Ảnh canonical cuối

```text
REPRO sha256=503be0f9986502620e369fd056009bf593a7195be58aa14485c4ba099c017f59
      size=205440  inputs=3a2e6b095d6491b1  dirty=0  rev=feb45790afc6
      build=2dc8d3f9e71d47ef  text=130072
# đã ghim: v2/artifacts/503be0f9….bin

bash v2/test/boot/ktest.sh 1 matrix → # KTEST: 21/21 boot sạch · ktest.sh: PASS
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/503be0f9….bin → KET_LUAN=ok
bash v2/test/boot/shell-edge.sh --image v2/artifacts/503be0f9….bin → 8/8 ca PASS
bash v2/test/boot/persist.sh → persist.sh: PASS
```
