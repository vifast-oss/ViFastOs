# KT-424 #0 — bằng chứng chạy cho nhánh `Switched` **từ cửa `syscall`** (và syscall sau `Exit`)

- **Nhánh**: `task/kt424-evidence` (worktree slot 1), dựa trên `main @2d9135f3`.
- **Vì sao**: review đối kháng hotfix `KT-417c` (`/tmp/audits/hotfix-review.md`) để lại 2 MAJOR **chỉ
  thiếu bằng chứng chạy**, không sai mã:
  - **MAJOR-2**: nhánh `SyscallOutcome::Switched` **từ cửa `syscall`** chưa từng chạy — pha core
    trước đây gọi `Tcb.Suspend(Defer)` bằng `int 0x80`, nên `resume_rip_offset(via_syscall)` (X1)
    chỉ là code chết trên thực tế.
  - **MAJOR-1b**: chưa có một `syscall` **sau** `Exit` — phép đo `GS.base` tại cửa vào (X2 vòng 2)
    chỉ chứng minh trên lượt ring 3 **trước** `Exit`.
- **Cách đóng**: cho chương trình ring 3 **pha core** gọi đúng một syscall `Tcb.Suspend(Defer)`
  bằng `syscall` thay `int 0x80`. Pha core chạy **sau** `# umode: exit` ⇒ cùng một dòng bằng chứng
  đóng cả MAJOR-2 (Switched từ cửa nhanh) lẫn MAJOR-1b (syscall sau Exit).

## Thay đổi

| Tệp | Nội dung |
|---|---|
| `vifast-kbin-x86_64/src/coretest.rs` | call `(c2)` `Tcb.Suspend(self, Defer)` đổi `int 0x80` → `syscall`; các call khác của pha core giữ `int 0x80` để không mất phủ cửa ngắt |
| `vifast-kbin-x86_64/src/umode.rs` | nhánh `Switched` in `# umode: switched via=<syscall\|int80> nr=<n> entry=0x<hex> gs=0x<hex>`: `via` chứng minh nhánh X1 đã chạy, `gs` là `GS.base` **đo tại cửa vào** (phải bằng `gs_scratch=` của `# syscall: msr`) |
| `v2/test/boot/run-x86_64.sh` | `check_switched_syscall`: đúng 1 dòng, `via=syscall`, `entry` trong dải mã user, `gs` khớp `gs_scratch`, dòng **sau** `# umode: exit`; ca âm **(mm)** đổi `via=syscall` → `via=int80` phải ĐỎ đúng lý do; dòng tổng kết hiển thị 38 → 39 |
| `vifast-plat-x86_64/tests/syscall_frame_gs.rs` | test `core_phase_must_switch_via_syscall` (`include_str!`) chốt `mov r10, {defer}` + `syscall` và cấm quay lại `int 0x80` |

## Bằng chứng phải có (điền sau mỗi tầng)

| Phép kiểm | Kết quả | Log |
|---|---|---|
| T0 `cargo test -p vifast-plat-x86_64` (host, kể cả 2 test `include_str!`) | **PASS**, `syscall_frame_gs` **9/9** (thêm `core_phase_must_switch_via_syscall`) | `/tmp/kt424-t0.log` |
| T0 `cargo check -p vifast-kbin-x86_64 --target x86_64-unknown-none --release --features ktest` | PASS (11 cảnh báo có sẵn) | cùng log |
| T1 `gate --dev` | (điền sau) | |
| T2 `gate --fast` trên cây hợp nhất | (điền sau) | |

## Rủi ro / kiểm soát

- `syscall` clobber `RCX`/`R11` theo kiến trúc; chương trình pha core **không** dùng hai thanh ghi
  này sau call `(c2)` (chỉ `r14` + `rax`/`rdi` cho `Exit`) ⇒ không đổi ngữ nghĩa.
- Stub `syscall` lấy RIP resume từ `RCX` (X1); `entry` do core giao vẫn qua `user_rip_ok()` trước khi
  ghi ⇒ nếu entry ngoài cửa sổ mã user, kernel `halt(7)` **có tên**.
- Ca âm (mm) + test `include_str!` giữ cho bằng chứng không bị "trả lại `int 0x80`" mà vẫn xanh.
- Không chạm `spec/`, không đổi `Tcb`/ABI; chỉ đổi **đường gọi** trong chương trình kiểm và thêm
  dòng log.
