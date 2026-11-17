# KT-118 — PANIC kernel từ một request U-mode (`proc_fs.rs` tràn mảng)

**Ngày**: 2026-11-18 · **Nguồn**: review toàn dự án (KT-115), phát hiện **nặng** #1.

## Lỗi

`FsSvc::load_path` nhận `len` từ `head >> 40` của **request U-mode** (tối đa 255) và chỉ kẹp theo
`room` (`MAX_PATH` = 48):

```rust
let room = MAX_PATH.saturating_sub(self.path_len);
let len = core::cmp::min(len, room);      // 25 vẫn lọt
...
let word = words[1 + index / 8];          // index = 24 ⇒ words[4] trên [u64; 4]
```

Ba MR word chở **24** byte, nên `len = 25` là **tràn mảng** ⇒ PANIC kernel.

## Vá

```rust
let len = core::cmp::min(len, core::cmp::min(room, REPLY_BYTES));
```

## Bằng chứng (đột biến phía **shell**, không phải phía test)

Dựng bản clone cho shell gửi `OP_PATH | 25<<40` thay vì `count` thật:

```text
# KHÔNG vá:
shell>
PANIC at vifast-kbin-riscv64/src/proc_fs.rs:454:24
PANIC: index out of bounds: the len is 4 but the index is 4
(không có '# product halt' — kernel chết)

# CÓ vá:
# shell: fs loi code=5
# shell: exit code=0
# product halt: shell_exit=0 fault_seen=1 events=2 … fs_replies=3
```

## Mức độ

Reviewer xếp **nặng** (không phải "chặn") vì shell phát hành **không** gửi `len > 24`. Nhưng
`EP_FS` nằm trong cspace **dùng chung** của procsrv và shell, và bất kỳ tiến trình U-mode nào có cap
đó đều làm chết kernel. Vá một dòng là xong.

## Chưa làm

Chưa có **ca kiểm** cho biên `len = 25` (hiện không ca nào chạm). Muốn thêm phải có một tiến trình
U-mode cố ý gửi request sai — đó là **ca âm** đúng nghĩa, nên làm ở K7 cùng bộ ca âm dịch vụ FS.
