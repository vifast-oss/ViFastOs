# KT-417c — hotfix audit đối kháng W8b (X1/X2/X3) + T1 PASS

- **Task**: `KT-417c` (đã merge ở `075b0639`), phần hotfix theo phát hiện của audit mã W8b
  (`/tmp/audits/code-audit.md` §X1/§X2/§X3 + X6).
- **Nhánh**: `task/kt417c-fix` (worktree slot 3), commit `c73eb77a`, dựa trên `4f6acb02`.
- **Ngày**: 2026-11-27 (soạn bản vá) → 2026-11-28 (chốt T1, sau sự cố server LLM làm agent dừng giữa lượt).
- **Bằng chứng thô**: `v2/agent/JOURNAL/kt417c-fix/` (log T0, log boot thật, log ca âm).

## 1. Ba lỗi MAJOR mà cổng không bắt

| Mã | Lỗi | Vì sao nguy hiểm |
|---|---|---|
| **X1** | Handler nhánh `Switched` ghi `frame + SYSCALL_FRAME_RIP` (= **136**) cho **cả hai** cửa | Frame cửa `syscall` chỉ **136 B**; `frame+136` đúng bằng `rsp0` = 8 B đầu `__ist1_stacks` hart 0 ⇒ đè stack `#DF`; đồng thời bỏ qua `entry` core giao ⇒ resume sai điểm vào |
| **X2** | Cặp `swapgs` chỉ nằm trong thân stub; đường `Exit` không quay lại đuôi stub | `GS.base` lật vĩnh viễn ⇒ `syscall` kế ghi `gs:[8]` vào **VA 8** ⇒ `#PF` CPL0 trên stack user, dưới SMAP ⇒ `#DF` (**leo thang**, kernel chạy trên bộ nhớ user) |
| **X3** | `cs` của cửa nhanh là hằng `sysret_cs()` | `is_user_cs(cs)` luôn đúng ⇒ `user=1`, TAP 12/14 mất nghĩa "đã vào từ ring 3" — bằng chứng tautology |
| X6 (bonus) | `retss` chỉ in, `IA32_GS_BASE` chưa từng đọc lại | `sysretq` `#GP` sau khi đã đổi stack, hoặc stub ghi nhầm địa chỉ |

## 2. Mô hình chọn

- **X1**: `resume_rip_offset(via_syscall)` — một nguồn cho "ô resume của cửa nào": `int 0x80` →
  ô `RIP` (136, CPU đẩy, `iretq` đọc lại); `syscall` → ô **`RCX`** (104, có thật trong 136 B,
  `sysretq` đọc lại ngay trước khi về user). Thêm const-assert mọi ô handler ghi nằm trong
  `SYSCALL_FAST_FRAME_BYTES` và chốt runtime `frame ∈ [rsp0−136, rsp0)`; entry resume phải nằm
  trong cửa sổ mã user.
- **X2**: `enter_user` — nơi **duy nhất** mọi `iretq` về CPL3 đi qua (`umode::run`,
  `coretest::start_core_phase`, `shellpath`) — **đọc lại** `IA32_GS_BASE`, `swapgs` **đúng một
  lần có điều kiện** nếu khác `USER_GS_BASE = 0`, đọc lại lần hai và `halt(5)` có tên nếu vẫn
  lệch. Cửa vào `int 0x80` kiểm bất biến ngược (`GS.base` phải là 0) và `halt(6)` nếu lệch.
  `# syscall: ran … ug=<GS.base đo trước iretq gần nhất>` in bằng chứng.
- **X3**: stub truyền `SYSCALL_FAST_NO_CS = 0` (sentinel; 0 không phải selector hợp lệ);
  `user_from_entry(via_syscall, cs, rip)` — `int 0x80` dùng `cs` CPU đẩy, `syscall` đòi sentinel
  **và** `rip` = `RCX` CPU ghi nằm trong `[USER_CODE_VA, +CODE_BYTES)` hoặc
  `[VSPACE_CODE_VA, +CODE_BYTES)`. `resume` cũng đòi entry trong mã user.

Không đổi đặc tả `spec/`, không nới ngưỡng, không đổi số ca TAP (50), `int 0x80` giữ song song.

## 3. Tệp sửa

| Tệp | Δ | Nội dung |
|---|---|---|
| `vifast-plat-x86_64/src/umode.rs` | +157 | `SYSCALL_FAST_NO_CS`, `user_rip_ok`, `user_from_entry`, `SYSCALL_FAST_FRAME_BYTES`, `resume_rip_offset`, 9 const-assert bố cục frame |
| `vifast-plat-x86_64/src/asm/trap.rs` | +50 | `LAST_USER_GS` + `last_user_gs()`, chuẩn hoá cặp `swapgs` trong `enter_user`, `halt(5)` |
| `vifast-plat-x86_64/src/syscall_fast.rs` | +79 | `USER_GS_BASE`, `needs_swap_before_user`, `user_gs_ok`, `gs_is_kernel`, `readback()` trả 5 MSR |
| `vifast-plat-x86_64/src/asm/syscall.rs` | +11 | đẩy sentinel thay `sysret_cs()`, giữ `rip` = `[rbx+RCX]` |
| `vifast-kbin-x86_64/src/umode.rs` | +111 | kiểm `retss`/`gsbase` là PANIC, in `rip=`/`ug=`, chốt bố cục frame + entry user, `halt(6/7)` có tên |
| `vifast-plat-x86_64/tests/syscall_frame_gs.rs` | **mới** 358 dòng | 6 test host: bố cục 136 B, canary `rsp0`, một cặp `swapgs` trong stub, GS khép ở **mọi** đường ra, không-CS ⇒ đo RIP |
| `v2/test/boot/run-x86_64.sh` | +224/−83 | pin `cs=0x23`/`retss=0x1b`/`star=0x0010000800000000`, kiểm `rip` trong `code_va..+code_bytes`, **6 ca âm mới (gg)..(ll)** tự khẳng định đã đột biến được; tổng selftest 31→37 |

## 4. Bằng chứng

| Phép kiểm | Kết quả | Log |
|---|---|---|
| cargo test `vifast-plat-x86_64` | **22/22 tệp PASS**, `syscall_frame_gs` 6/6 | `kt417c-fix/t0-hosttests.log` |
| clippy `-D warnings` host + target x86_64 | sạch (kbin 11 cảnh báo **có sẵn**, không đổi) | `kt417c-fix/t0-clippy-check.log` |
| boot thật QEMU 11.1.3, ảnh `--features ktest` | `# syscall: msr … retcs=0x23 retss=0x1b gsbase=0x0`; `# syscall: ran nr=12 ret=14 err=0 cs=0x0 rip=0x100000006e ac=0 user=1 ug=0x0`; `# sched: cpl3 preempted=1 resumed=1 cs=0x23`; `# umode: int80 … cs=0x23`; **TAP 50 ok / 0 not_ok** | `kt417c-fix/boot-1-hotfix.log`, `boot-evidence.txt` |
| ca âm trên log thật (X3) | 6/6 đột biến **ĐỎ**, 2 dòng tốt **XANH** trước | `kt417c-fix/red-neg-check-x3.log` |
| đỏ-trước (trước bản vá) | `fast_frame_resume_slot…`, `canary_at_rsp0…`, `enter_user_must_restore_gs…` **ĐỎ**; `check_syscall` **XANH** với log giả | `kt417c-fix/red-frame-gs.log`, `red-check-syscall.log` |
| **T1 `gate --dev`** | **PASS** `registered=82 ran=68 skipped=14 failed=0`, 668,8 s, 11:16:29–11:27:39 UTC | `/tmp/t1-kt417c-hotfix-c73eb77a.log` |

## 5. Ghi chú / nợ

1. **T2 chưa chạy cho hotfix này** — phải chạy trên `main` sau merge (đầu vào mã đổi: `v2/kernel/`,
   `v2/test/boot/run-x86_64.sh`).
2. Fixture `good.log` của `soak-x86_64.sh --selftest` còn dòng mẫu `# syscall: ran … cs=0x23`
   (định dạng **trước** hotfix). Không parser nào của soak đọc dòng này (chỉ là log tổng hợp cho
   `check_log`), nên **không** ảnh hưởng kết quả; ghi lại để lần sửa fixture sau đồng bộ.
3. Trạng thái `KT-417c` chính thức vẫn chờ **Agent 0** chốt `DONE` + `D2-264` sau khi T2 xanh
   (lead không tự sửa `QUEUE.md`).
