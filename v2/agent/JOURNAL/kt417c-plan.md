# KT-417c — kế hoạch thi hành (W8b, Slot 3)

Nền: `main` @ `afb2556b` (W8a đã merge). Nhánh `task/kt417c-syscall-sysret`, worktree slot 3.
Ngân sách 3 h; mốc dừng an toàn 2 h 45.

## 11 bước

| # | Việc | Đỏ-trước |
|---|---|---|
| 0 | Đọc nền: `gdt.rs`, `asm/syscall.rs`, `umode.rs`, `trap.rs` (cpl3_*), `check_kernel.py`, `spec/kernel.toml`, `run/soak/hardening-max.sh` | — |
| 1 | `check_kernel.py`: đảo `E_GDT_SYSRET` sang "phải true" + **2 ca âm** (bật cờ mà GDT chưa đảo; `false`) | ✅ `red-kernel-selftest.log` |
| 2 | `spec/kernel.toml` §gdt: index/selector đảo + `sysret_ready=true` — **chưa** sửa `gdt.rs` ⇒ `E_CONST_LECH` | ✅ `red-const-lech.log` |
| 3 | `gdt.rs`: `INDEX_USER_DATA=3`/`INDEX_USER_CODE=4` + doc; `syscall_fast` hằng (`STAR`/`FMASK`/vector/GS scratch); test host literal đảo | |
| 4 | `syscall_fast.rs` (plat): GS scratch per-hart + `star_value`/`fmask_value`/`init_syscall_msrs` (EFER đọc-sửa-ghi) | |
| 5 | Stub asm `vifast_isr_syscall_fast` (`swapgs` → stack GS → frame cùng bố cục → `call syscall_dispatch` → `sysretq`) | ✅ `red-syscall-line.log` |
| 6 | Nối: BSP (`main.rs`) + AP (`smp.rs`) gọi `init_syscall_msrs`; ring 3 chạy lại `DebugPutStr` qua `syscall` | |
| 7 | `kbin/umode.rs`: in `# syscall: ran nr=12 ret=<n> cs=0x23` khi cửa là `syscall` (đọc ô `vector` của frame) | |
| 8 | `run-x86_64.sh`: `check_syscall()` + `check_cpl3()` (assert `cs` == `cs` của `# umode: iretq`, không hardcode) | ✅ |
| 9 | `soak-x86_64.sh` log giả + `hardening-max.sh` log giả `cs=0x23`/`ss=0x1b` + **đảo** đột biến `cpl3-cs-lech` | ✅ |
| 10 | Test host mới `tests/syscall_fast.rs` (thứ tự `planned`, công thức `sysret`, GS scratch, `FMASK` có `AC`) | |
| 11 | `cargo fmt` + T0 (`cargo test` các crate + `check_kernel --selftest` + `v2 check`) → stage → báo Lead | |

**0 TAP mới** (giữ `TAP_TESTS=50`): kiểm `syscall`/`cpl3` bằng `grep` trong `check_log`
(`# syscall:` + `# sched: cpl3`), nên `expected.txt`/`tap-parity` không đổi.

**Giữ `int 0x80`**: stub cũ **không** sửa; hai cửa cùng gọi `syscall_dispatch` ⇒ `mut-no-int80`
vẫn ĐỎ (hook `cfg` nằm trong `shellpath::dispatch`, trước mọi phân nhánh).
