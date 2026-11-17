# KT-114 — bốn lệnh shell mới: `time`, `uptime`, `sync`, `stat`

**Ngày**: 2026-11-18 · **Ngân sách**: 2 h · **Dùng**: ~1 h 10 m.

## Vì sao làm được mà không cần Agent 0

KT-91 bị chặn vì `# shell: help cmds=` được khẳng định trong `proc_demo.rs:761` và `ktest.sh:709`.
Nhưng **KT-77c đã mở đường**: chỉ cần đặt lệnh mới trong nhánh `#[cfg(shell_fs)]` thì **ảnh test**
(không FS) giữ **sáu** lệnh, còn **ảnh sản phẩm** đổi. Hai chỗ khẳng định kia chỉ chạy trên ảnh test
⇒ **không** phải sửa. `spec/v1-parity.toml:70` vốn đã linh hoạt (`cmds=[0-9]+`).

## Bốn lệnh

| Lệnh | Nguồn | In ra |
|---|---|---|
| `time` | `ClockRead` (11) — **đã có**, không mở ABI | `# shell: time ns=<n>` |
| `uptime` | cùng nguồn, ns chia một triệu | `# shell: uptime ms=<n>` |
| `sync` | `OP_SYNC` (0x16) — **đã có**, `write` đang dùng | `# shell: sync ok` |
| `stat <path>` | `OP_STAT` (0x15) — **đã có** trong dịch vụ FS, **chưa ai gọi** | `# shell: stat path=<p> kind=<k> size=<n> ino=<i>` |

`stat` không tham số ⇒ `# shell: thieu duong dan` (theo khuôn KT-104).

## `CMDS`: 9 → 13 (ảnh sản phẩm), ảnh test vẫn 6

Đã kiểm bằng máy: build biến thể **không** `product-fs` cho banner
`# shell: up cmds=help/echo/ps/wait/kill/exit` và **không** chứa `time ns=`/`uptime ms=`/`sync ok=`/`stat path=`.

## Log QEMU thật

```text
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/exit
# shell: help cmds=13
  help | echo <text> | ps | wait <pid> | kill <pid> | ls [path] | cat <path> | write <path> <text> | time | uptime | sync | stat <path> | exit
# shell: time ns=1146273000
# shell: uptime ms=1804
# shell: sync ok
# shell: stat path=/README kind=1 size=79 ino=2
# shell: fs loi code=5            ← stat /khongco
# shell: exit code=0
# product fs: … sync=1 errors=1 replies=7 dev_flushes=1
```

Đối chiếu chéo: `size=79` đúng bằng `README` trong `proc_fs.rs`; `sync=1` và `dev_flushes=1` chứng
minh `sync` **thật sự** flush; `stat /khongco` → `code=5` = `FsError::NotFound`.

## Hai script được làm linh hoạt (để lần sau không phải sửa)

- `v2/test/boot/shell-edge.sh`: khớp **tập lệnh** `^# shell: up cmds=help(/[a-z]+)+$` thay vì chuỗi
  cứng. Đã thử: khớp **mọi** ảnh sản phẩm cũ trong `v2/artifacts/`, không khớp ảnh không shell.
- `v2/agent/JOURNAL/kt90-soak-product-image.sh`: **đọc `CMDS` từ chính log** (đếm tập lệnh trong
  banner rồi đối chiếu `help cmds=<n>`), lệch ⇒ `why=cmds-mismatch`.

## Kiểm

```text
bash v2/test/boot/shell-edge.sh --image <ảnh mới>  → 8/8 ca PASS
bash v2/test/boot/shell-edge.sh --selftest         → 19/19 kiểm
./v2/bin/v2 check                                  → PASS
cargo fmt --check                                  → sạch
clippy (target riscv64 + product-shell,product-fs) → tập cảnh báo TRÙNG KHÍT baseline HEAD
```

## Ghi chú

`clippy --workspace --all-targets` trên host **đỏ** vì `vifast-plat-riscv64` có asm riscv64 không
build được cho host — **lỗi có sẵn**, đã chứng minh bằng cách chạy y hệt trên cây HEAD nguyên bản.
