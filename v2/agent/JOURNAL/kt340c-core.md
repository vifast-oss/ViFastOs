# KT-340c — x86_64: nối `vifast-core::Machine` + IPC ring 3 qua `int 0x80` (`7C-r13`)

- **Task**: `KT-340c` (D2-233 / D2-237 / D2-242 / D2-244), ≤ 3 h, nhánh `task/kt340c-core`, base
  `main` @ `bb1d4abe`, worktree `/tmp/kt340c-s1`.
- **Phạm vi**: `v2/kernel/vifast-plat-x86_64` (mới `core_bridge.rs`, `syscall_env.rs`, sửa
  `asm.rs`, `irq_lock.rs`, `lib.rs`, `Cargo.toml`), `v2/kernel/vifast-kbin-x86_64` (mới
  `coretest.rs`, sửa `main.rs`, `umode.rs`, `net.rs`, `sched.rs`, `Cargo.toml`),
  `v2/test/boot/{run,soak,gpu,budget}-x86_64.sh`. **Không** chạm `spec/`, `v2/agent/{STATE,QUEUE,
  DECISIONS}.md`, `v2/bin/v2`, `.github/**`, `main`.
- **Kết quả một dòng**: x86_64 có **một** `Machine` (184.792 B, `static` trong `.bss` **ảnh
  kiểm**, bọc `IrqMcsLock` lưu/khôi phục `RFLAGS.IF`), **mọi** `int 0x80` từ CPL3 đi qua
  `vifast_core::syscall::dispatch`, `MessageInfo` trả về ở **`r13`** (`7C-r13`, D2-242: 80/560/608
  = `mr`/`extra caps`/`Tcb.Configure`); TAP **37 → 43**; runner **36/36 kiểm sạch** (32/32 ca âm
  ĐỎ, `tests=43`, `exc=6`); `.data+.bss` sản phẩm **43.072 B** (trần 65.536), ktest **251.024 B**
  (trần 262.144); 2 đột biến đều ĐỎ đúng TAP. **Scheduler**: xem §6 — gỡ **toàn bộ** state
  machine syscall viết tay, **giữ** mô hình scheduler preemption của harness (0 % mã `sched.rs`
  bị gỡ) + **task con đề xuất** `KT-340e` cho phần còn lại, đúng lệnh "cấm DONE một phần".

---

## 1. Khảo sát trước khi sửa (x86_64 vs aarch64/riscv64 · "scheduler cũ" là gì)

### 1.1 Ba trục có sẵn / còn thiếu trên x86_64

| Lớp | aarch64 `KT-338b` (đã xong) | riscv64 `KT-336c` (đã xong) | x86_64 **trước** `KT-340c` |
|---|---|---|---|
| `Machine` trong ảnh | `vifast-plat-aarch64/src/core_bridge.rs` | `vifast-plat-riscv64/src/core_bridge.rs` | **không có** — `vifast-core` chưa được link |
| `SyscallEnv` | `syscall_env.rs` (đọc user `EL0_*`, `eret` hook) | `syscall_env.rs` (`SUM`, `sepc`) | **không có** |
| Khoá lá | `IrqMcsLock` (`DAIF.I`) | `IrqMcsLock` (`sstatus.SIE`) | `McsLock` **không** tắt ngắt (IRQ chỉ bọc từ ngoài) |
| `int 0x80` từ CPL3 | — (aarch64: `svc #0`) | — (riscv64: `ecall`) | **có handler** nhưng **thủ công**: `umode::syscall_dispatch` tự đọc chuỗi user in ra (`umode::print_user`, trần 256 B), tự `Exit`, tự chặn số hiệu lạ; **không** CSpace/TCB/Endpoint, không invocation nào |
| `MessageInfo` trả về | `x2` (D2-242) | `a1` (D2-242) | **chưa có** — chưa có message nào |
| Scheduler | **không có** (một tác vụ EL0 một chiều) | `KT-290`: scheduler U-mode | **có** — `plat-x86_64/src/sched.rs` (786 dòng, mô hình thuần) + `kbin-x86_64/src/sched.rs` (1.376 dòng, 3 pha + switch `iretq` 22-qword) |

### 1.2 "Scheduler cũ" trên x86_64 là **cái gì** (trả lời câu hỏi khảo sát)

Hai tầng, **không** phải một scheduler của hệ điều hành:

1. `v2/kernel/vifast-plat-x86_64/src/sched.rs` — **mô hình thuần** (không `unsafe`, test host
   được): `TASK_SLOTS` (a/b/c/u), `TASK_SEQUENCE` (chuỗi lượt cố định cho pha 1), `switch_frame`
   (22 qword), `mh_ok` (bất biến "hart khác chọn tác vụ khác"), `prio_pick`/`sleep_ok` (pha ưu
   tiên + sleep, TAP 29/30).
2. `v2/kernel/vifast-kbin-x86_64/src/sched.rs` — **ứng dụng kiểm preemption**: timer LAPIC →
   `on_tick` chọn tác vụ theo mô hình trên → `switch_iretq` (lưu/khôi phục 22 qword + `iretq`).
   Ba pha: chuỗi A/B/U (TAP 22–24), bốn hart (TAP 25), ưu tiên + sleep (TAP 29/30).

**Kết luận khảo sát**: đây là **harness kiểm chuyển ngữ cảnh**, không phải scheduler của nhân.
Từ `KT-280`/`KT-290`, "ai chạy tiếp" ở tầng nhân do `vifast-core` quyết (hàng đợi TCB theo hart,
D2-49(d)); x86_64 **chưa** nối core nên **chưa** có scheduler thật — chỉ có harness. Vì vậy bước 4
của task ("gỡ scheduler tự viết") áp dụng đúng nghĩa: **gỡ state machine syscall viết tay** (đã
làm, §6.1) và **phần scheduler còn lại là harness** — không thể xoá mà không đổi hợp đồng TAP
22–25/29/30 của runner (32 ca âm đang bám vào **nguyên văn** các dòng `# sched:`); đề xuất task
con `KT-340e` thay **từng pha** bằng TCB thật của core (§6.3).

### 1.3 `Machine` cần gì trên x86_64 (và đã có sẵn gì)

| Nhu cầu của `vifast-core` | Nguồn có sẵn của x86_64 | Việc phải làm |
|---|---|---|
| `SyscallEnv` (arch đọc/ghi bộ nhớ user; D2-83) | `umode::{VSPACE_CODE_VA, VSPACE_STACK_VA}`, `asm::{read_u64,write_u64}`, identity map | `syscall_env.rs` (`X86_64Env`, 3 vùng user đã đăng ký + `ipc_buffer_words` fail-closed) |
| Khoá tắt ngắt khi vào core | `asm::{cli, sti, interrupts_enabled}`, `vifast_sync::McsLock` | `irq_lock.rs`: `IrqMcsLock` + `IrqState` lưu/khôi phục **`RFLAGS.IF`** |
| `static Machine` **trong ảnh** | `linker.ld` (`__text_start`, `__kernel_end`), identity map `VA == PA`, `BootArena` | `core_bridge.rs`: `MaybeUninit` + `init_zeroed` **tại chỗ** (không temp trên stack — bài học KT-61) |
| `num_harts` **thật** (D2-88(a)) | `core_bridge::harts()` (đo lúc `init`, BSP = 1 ở pha rig) | — (SMP boot ở TAP 18–21, **sau** pha rig) |
| `tcb_vspace` (arch giữ danh tính VSpace) | `VSpace` **động** của `KT-340b` | hook trả `Failed(Busy)` cho VSpace ≠ 0 ⇒ core rollback + lỗi 14 (đo ở TAP 39) |
| Điểm vào sau `Suspend` (D2-83: core không ghi `RIP`) | `asm::enter_user`, frame `iretq` | hook `tcb_resume` + `take_resume`; chương trình ring 3 thứ hai có nhãn `__user_core_after_suspend` |

**Vì sao ảnh sản phẩm không mang core**: `size_of::<Machine>()` = **184.792 B** > trần
`.data+.bss` sản phẩm x86_64 **65.536 B** (D2-243/D2-244, `budget-x86_64.sh`). `vifast-core` là
`optional` trong `vifast-kbin-x86_64/Cargo.toml`, bật **chỉ** bởi feature `ktest`; ảnh sản phẩm
giữ 43.072 B **không đổi**. Việc mang core vào ảnh sản phẩm thuộc `KT-341` (shell sản phẩm) —
ghi ở §8.

---

## 2. Nối core vào x86_64 (kiến trúc)

### 2.1 Tệp mới/sửa (crate arch)

| Tệp | Dòng | Nội dung |
|---|---:|---|
| `vifast-plat-x86_64/src/core_bridge.rs` | 264 | `static mut MACHINE: MaybeUninit<Machine>` + `IrqMcsLock<()>`; `set_image_bounds`/`in_image_region` (identity map, **không** đổi VA→PA); `CoreReport` (addr, size, pin 196.608, in_image, check_caps/bytes/all_bytes, harts); `init(base, bytes, harts)` = `write_bytes(0)` + `init_zeroed` **tại chỗ** + `add_untyped` + checks; `LockReport`/`lock_stats`/`lock_ok`; 2 test host |
| `vifast-plat-x86_64/src/syscall_env.rs` | 315 | `X86_64Env`: `put_str` (COM1 khoá + `asm::com1_write_byte`), `resolve_addr` ⇒ `NotMapped`, `now_ns` (PIT kênh 2), `features` = `PUT_STR|DEBUG_HALT`, `num_harts`, `tcb_resume`/`take_resume`, `tcb_vspace` (0 ⇒ `Applied`, ≠ 0 ⇒ `Failed(Busy)`), `ipc_header` (magic + receive window + send caps + extra caps + `Tcb.Configure`), `ipc_buffer_words` (fail-closed ngoài 3 vùng user); `LAYOUT_EVIDENCE = (80, 560, 608)`; 4 test host |
| `vifast-plat-x86_64/src/asm.rs` | +~60 | `com1_write_byte` (chờ `LSR.THRE`, dùng chung `serial::TX_WAIT_LIMIT`), `read_pit2_latch`; stub `vifast_isr_syscall` **11 tham số**: đẩy `frame`, `rip`, `cs`, `r12` (MR3), `r9` (MR2) theo SysV, gọi `syscall_dispatch`, chỉ ghi `rax`/`rdx` lại frame (các kênh trả khác do handler ghi — `7C-r13`) |
| `vifast-plat-x86_64/src/irq_lock.rs` | +40 | `irq_enabled()`; nhánh `mut-no-irq-restore` (về sớm trước `sti`) |
| `vifast-plat-x86_64/Cargo.toml` | +8 | feature `core = ["dep:vifast-core"]`, `mut-no-irq-restore`; dep optional |

`cargo test -p vifast-plat-x86_64 --features core` (host): **xanh**, gồm `core_bridge` 2 ca +
`syscall_env` 4 ca + `LAYOUT_EVIDENCE == (80, 560, 608)`.

### 2.2 Ảnh kiểm: `coretest.rs` (mới, 1.079 dòng) — rig + 6 TAP + chương trình ring 3

- **Rig** (10 ô cap): `Untyped` (4 frame), `Tcb` lượt 1, `Tcb` server (`Recv`-blocked), `Tcb` pha
  core, `Tcb` người gửi (`Send`-blocked trên EP2), `Endpoint` ×2, `CNode` gốc, `Frame` IPC buffer
  (object đại diện PA), `VSpace` đại diện (ngoại lai, D2-83).
- **IPC buffer** ở `VSPACE_BASE + 0x5000` (sau `peer` 0x3000/`probe` 0x4000; **không** đè guard
  `+0x2000`): header 10 word (magic + receive window 12..15), extra caps 560, `Tcb.Configure`
  608, MR4..MR7 ở 80 — đúng `spec/abi.toml [ipc_buffer]`.
- **Pha core**: `VSpace` **mới** (id 3) map mã `.user` + stack + buffer; `TSS.RSP0` =
  `__syscall_stack_top`; chặn server `Recv` và người gửi `Send` **trước** `iretq`; đổi TCB hiện
  hành sang TCB core; `asm::enter_user`.
- **Điểm hội tụ** `finish()`: khôi phục `CR3` gốc, in 8 dòng `# core …`, phát TAP 38..43, kiểm
  `check_caps`/`check_bytes`/`check_all_bytes`/`check_ipc`, rồi **trả về** cho handler gọi
  `net::run` — chuỗi boot TAP 15.. không đổi thứ tự.

### 2.3 `umode.rs`: một cửa syscall duy nhất (D2-79)

`syscall_dispatch(nr, a0, a1, mi, mr0..mr3, cs, rip, frame)`:

1. **Đọc bộ nhớ user đã rời khỏi ảnh**: `print_user` + `MAX_USER_PRINT` + `USER_BYTES_PRINTED`
   **bị xoá**; `DebugPutStr` do `SyscallEnv::put_str` của arch đọc (D2-83), ngân sách do **core**
   kiểm (`MAX_DEBUG_PUT`) — không còn đường in thứ hai có thể lệch.
2. Bảng syscall riêng của ảnh **bị xoá**: `match nr { DebugPutStr, Exit, _ }` cũ thay bằng **một**
   câu `core_bridge::with_machine(|m| vifast_core::syscall::dispatch(m, tcb, 0, nr, args, &mut X86_64Env))`.
3. **`7C-r13`**: `Return` ⇒ ghi `info` vào frame `R13`, `mr[0..3]` vào `R10/R8/R9/R12`
   (`# mc r12` = MR3); `Switched` ⇒ `coretest::resume_after_switch` (gọi `Tcb.Resume`, đối chiếu
   `take_resume`, ghi `RIP` của frame = `entry`); `Exit` pha core ⇒ `record_exit` + `finish`;
   `Exit` lượt 1 ⇒ giữ TAP 14 (`user && vspace_ok`) rồi `start_core_phase`.

---

## 3. Sáu ca kiểm (TAP 38..43) — **kernel** khẳng định, không tin lời ring 3

| TAP | Ca | Bằng chứng kernel (đọc từ `Machine`, không từ log của chương trình) |
|---|---|---|
| 38 | core link | `size_of::<Machine>()` 184.792 ≤ pin 196.608, `in_image_region(addr,size)`, `healthy()` = `check_caps ∧ check_bytes ∧ check_all_bytes` |
| 39 | (a) `Tcb.Configure` | ưu tiên **4 → 9** sau invocation từ ring 3; `vspace?` ⇒ arch `Failed(Busy)` ⇒ core rollback + lỗi **14**; `vspace` trở lại **0** |
| 40 | (b) IPC 1 message | vùng nhận của server: `info.length = 8`, MR0..3 = hằng thanh ghi, MR4..7 = hằng **chỉ có trong buffer**; server `Ready` |
| 41 | (d) **`7C-r13`** | `dispatch` trả `info` ở **`r13`** + MR3 ở **`r12`**; hai nguồn: mã thoát ring 3 (tự so `r13`/`r12`) **và** vùng nhận của TCB do kernel đọc (`area_info = 0x80000341`, `area_mr3 = 0x34100003`) |
| 42 | (c) `Suspend(Defer)` | `dispatch` trả `Switched` ở **điểm an toàn** (TCB `Inactive`), `Tcb.Resume` trả `entry`, `take_resume` = `entry` ⇒ ring 3 chạy tiếp **đúng** `__user_core_after_suspend` (nhãn được căn 2 B — `TCB_ENTRY_ALIGN = 2`) |
| 43 | khoá/`RFLAGS.IF` | `irq_before=1`, **`irq_inside=0`** trong `with_machine`, `irq_after=1`; 14 lần lấy khoá, `overlap=0`, `in_cs=0` |

Log nguyên văn (runner `boot-1.log`, ảnh ktest `--features ktest`, `-smp 4`):

```text
# core phase: entry=0x1000000067 after=0x10000001c6 buf_va=0x1000005000 buf_pa=0x202000 root=0x20e000 boot_root=0x223000 tcb=4
# core resume: tcb=4 state_after_switch=0x0 entry=0x10000001c6 match=1
# core: exit code=0x4
# core: addr=0x1470c0 size=184792 pin=196608 in_image=1 harts=1 objects=10 caps=10 healthy=1
# core buffer: va=0x1000005000 mr_offset=80 extra_offset=560 configure_offset=608 harts=1
# core roots: boot=0x223000 core=0x20e000
# core configure: priority_old=4 priority_now=9 err_cfg=0 err_vspace=14 err_entry=0 vspace=0 configure=2
# core ipc: send_err=0 len=8 mr0=0x340c0000 mr3=0x340c0003 mr4=0x340c0004 mr7=0x340c0007 server=Ready
# core 7c-r13: recv_err=0 exit=0x4 info_seen_expected=0x80000341 mr3_expected=0x34100003 area_info=0x80000341 area_mr3=0x34100003
# core suspend: switched=1 state=Inactive resume_entry=0x10000001c6 resume_ok=1 exit=0x4 deferred=1 resume_stat=1
# core lock: irq_before=1 irq_inside=0 irq_after=1 acquired=14 overlap=0 in_cs=0 ok=1
ok 38 - core: Machine linked in image, pin + invariants
ok 39 - core: Tcb.Configure priority + vspace rollback
ok 40 - ipc: ring3 send 8 MR -> endpoint recv (MR4..MR7 buffer)
ok 41 - core: 7c-r13 MessageInfo from r13 (r12 keeps MR3)
ok 42 - core: Tcb.Suspend(Defer) safe point + Resume entry
ok 43 - lock: RFLAGS.IF masked in critical section, restored after
# core: done
```

Mã thoát `0x4` = **chỉ** bit 2 (đã chạy tiếp sau `Resume`); bit 0 (`r13` sai), bit 1 (`r12` sai),
bit 3 (một syscall lỗi) đều **0** — ring 3 tự khẳng định `7C-r13` bằng **thanh ghi**, kernel
khẳng định lại bằng vùng nhận của TCB.

---

## 4. Đột biến (cả hai ĐỎ — log nguyên văn)

### (i) `mut-info-r12` — đọc `MessageInfo` ở `r12` thay `r13` (D2-05, đột biến bắt buộc i)

`cargo build … --features ktest,mut-info-r12`; boot QEMU:

```text
# core 7c-r13: recv_err=0 exit=0x7 info_seen_expected=0x80000341 mr3_expected=0x34100003 area_info=0x80000341 area_mr3=0x34100003
ok 38 - core: Machine linked in image, pin + invariants
ok 39 - core: Tcb.Configure priority + vspace rollback
ok 40 - ipc: ring3 send 8 MR -> endpoint recv (MR4..MR7 buffer)
not ok 41 - core: 7c-r13 MessageInfo from r13 (r12 keeps MR3)
# TAP 41 FAIL — dừng (không in marker)
```

`exit=0x7` = bit 0 **và** bit 1 **và** bit 2: chương trình ring 3 đọc `r13` không thấy
`MessageInfo`, `r12` không còn MR3 — đột biến bị bắt **từ phía chương trình** trước khi kernel
từ chối. Ca 40 **vẫn xanh** (vùng nhận của TCB không đổi) — chứng minh TAP 41 đo đúng **kênh
thanh ghi**, không phải "có message là đủ".

### (ii) `mut-no-irq-restore` — `IrqState::restore` không `sti` (đột biến bắt buộc ii)

`cargo build … --features ktest,mut-no-irq-restore`; boot QEMU:

```text
# core lock: irq_before=1 irq_inside=0 irq_after=0 acquired=14 overlap=0 in_cs=0 ok=0
ok 41 - core: 7c-r13 MessageInfo from r13 (r12 keeps MR3)
ok 42 - core: Tcb.Suspend(Defer) safe point + Resume entry
not ok 43 - lock: RFLAGS.IF masked in critical section, restored after
# TAP 43 FAIL — dừng (không in marker)
```

`irq_before=1, irq_inside=0, irq_after=0` — chỉ **một** vế (`irq_inside`) là chưa đủ: phép đo
đòi **khôi phục**, nên "tắt ngắt rồi quên bật" không thể xanh.

---

## 5. Số đo

| Đại lượng | Trước (base `bb1d4abe`) | Sau |
|---|---:|---:|
| TAP x86_64 (`run-x86_64.sh`, không `gpu`) | 37 | **43** |
| TAP x86_64 (`gpu-x86_64.sh`, `--features gpu`) | 38 | **44** (gpu dời 38 → 44, `tap_first+29`) |
| Runner `run-x86_64.sh` | 36/36 (32 ca âm) | **36/36 kiểm sạch** (boot=1, **tests=43**, **exc=6**), 32/32 ca âm ĐỎ, `qemu_own=0` |
| `soak-x86_64.sh --selftest` | 53/53 (2 dương + 51 âm) | **53/53** (fixture `1..43` + 6 TAP mới + 3 dòng `# core`) |
| `size_of::<Machine>()` | — | **184.792 B** (pin 196.608 — D2-84) |
| `.data+.bss` ảnh **sản phẩm** | 43.072 B | **43.072 B** (trần **65.536** — không đổi) |
| `.data+.bss` ảnh **ktest** | 131.768 B | **251.024 B** (trần 262.144, headroom 11.120) |
| `.bootstacks` (NOLOAD, báo riêng) | 20.480 B | **86.016 B** (thêm 4 stack tác vụ 16 KiB — xem §6.4) |
| `.text` ảnh ktest | 200.704 B | 200.704 B (không đổi — `# conf image text=200704`) |
| `.user` (một trang 4 KiB) | 471 B | **472 B** (chương trình core ~330 B + căn 2 B) |
| Tệp nguồn | — | mới 3 tệp (1.658 dòng), sửa 13 tệp (+453 / −131) |

Kiểm chéo kích thước: `Machine` 184.792 + các static cũ 66.232 = 251.024 — khớp **từng byte**
với `budget-x86_64.sh` (`.data=11.328 + .bss=239.696`).

---

## 6. "Gỡ scheduler tự viết" — gỡ **cái gì**, còn **cái gì** (số tuyệt đối, không "một phần")

### 6.1 Đã gỡ: **100 %** state machine syscall viết tay trong ảnh

| Thứ bị gỡ | Vị trí | Dòng |
|---|---|---:|
| `print_user` (đọc chuỗi user bằng con trỏ, trần 256 B) | `kbin-x86_64/src/umode.rs` | −48 |
| `MAX_USER_PRINT` + `USER_BYTES_PRINTED` (hai nguồn ngân sách) | nt | −8 |
| Bảng syscall riêng: `DebugPutStr` / `Exit` / nhánh lạ (tự `write_byte`, tự `halt`, tự đếm byte) | nt | −76 |
| **Tổng đường cũ** | | **−132** |
| Đường mới: **một** `core_bridge::with_machine(dispatch)` + phân phối frame `7C-r13` | nt | +175 (gồm 60 dòng chú thích hợp đồng thanh ghi) |

Hệ quả: `DebugPutStr`/`Exit` **không còn** là "syscall riêng của ảnh" — chúng là syscall **của
core** như mọi syscall khác (D2-79); đột biến (i) chứng minh đường trả `MessageInfo` không thể
"in đúng mà không có core".

### 6.2 Chưa gỡ: **0 %** mô hình scheduler preemption của harness — và **vì sao**

`plat-x86_64/src/sched.rs` (786 dòng) + `kbin-x86_64/src/sched.rs` (1.376 dòng) **giữ nguyên**
(2.162 dòng). Lý do kỹ thuật, không phải thiếu thời gian:

1. Đây là **harness kiểm chuyển ngữ cảnh**, không phải scheduler của nhân (§1.2): nó **không**
   quyết định tác vụ nào của hệ chạy — nó tạo 4 tác vụ giả A/B/C/U và **ép** chuỗi lượt để đo
   `switch_iretq` (22 qword), preemption timer, SMP, ưu tiên, sleep.
2. Hợp đồng bằng chứng: 32 ca âm của runner bám **nguyên văn** các dòng `# sched:` (ví dụ
   `# sched: prio first=b viol=0 ticks=8 hard=0 picks=[2, 2, 2, 2]`). Thay "ai chạy tiếp" bằng
   `Machine::dispatch(hart)` **đổi giá trị** các dòng ấy (scheduler của core chọn theo ưu tiên
   thật: `u` 4 > `c` 3 > `b` 2 > `a` 1 ⇒ `picks` khác `[2,2,2,2]`) ⇒ phải sửa **kỳ vọng** runner
   — việc cần Agent 0 duyệt, không được tự làm trong task này (§4.2 AGENTS).
3. An toàn: gỡ nửa vời mô hình (giữ switch, bỏ chuỗi) sẽ làm 6 TAP 22–25/29/30 **mất nguồn sự
   thật** mà không có nguồn thay — đúng loại "DONE một phần" bị cấm.

### 6.3 Đề xuất **task con `KT-340e`** (bắt buộc, kèm tiêu chí đo được)

> **`KT-340e` — thay 3 pha `sched.rs` bằng TCB thật của `vifast-core` (≤ 3 h)**
> - **Bước 1 (an toàn, không đổi TAP)**: dựng 4 TCB trong `Machine` (a/b/c/u, ưu tiên 1/2/3/4,
>   `entry` = nhãn `__user_sched_entry`/`__sched_a/b/c`), **ánh xạ** `ObjId ↔ TaskSlot` do
>   `core_bridge` giữ; mỗi tick, `on_tick` hỏi `Machine` "hart này chạy TCB nào" **nhưng vẫn in
>   bằng chứng theo mô hình cũ** trong một feature `sched-model-core` (đỏ-trước: TAP 25 phải
>   đỏ khi tắt `Machine` — chứng minh phép so là thật).
> - **Bước 2 (đổi hợp đồng, cần Agent 0 duyệt)**: đổi kỳ vọng `# sched:` sang `tcb=<ObjId>`
>   (cùng 6 TAP, cùng 32 ca âm được cập nhật **một lần**, có D2 mới), rồi **xoá** `TASK_SEQUENCE`
>   + `prio_pick`/`sleep_ok` khỏi `plat-x86_64/src/sched.rs` (mỗi lần xoá một hàm, TAP tương ứng
>   đỏ-trước).
> - **Bước 3**: `plat-x86_64/src/sched.rs` còn **đúng** `init_frame_words` + `switch_frame_ok`
>   (switch `iretq` 22 qword — arch giữ, D2-83) ⇒ mục tiêu ≤ 300 dòng; xoá `kbin` phase 1/2/3
>   tương ứng; cập nhật `soak`/`run` fixture.
> - **Tiêu chí xong**: 0 dòng `TASK_SEQUENCE`/`prio_pick`/`sleep_ok`; TAP 22–25/29/30 phát bằng
>   chứng từ `Machine`; 32 ca âm vẫn ĐỎ; `.data+.bss` ktest ≤ 262.144 sau khi thêm TCB.

### 6.4 Vì sao `.bootstacks` tăng 65.536 B (và vì sao **không** phải nới trần)

Nối `Machine` (184.792 B) đẩy `.data+.bss` ảnh kiểm lên **316.560 B** > trần **262.144**. Bốn
stack tác vụ của harness (4 × 16 KiB = 65.536 B) là **stack** — đúng loại D2-243 cho phép tách
section NOLOAD kiểu `.smp_stacks` (KT-276) — nên chuyển sang `.bootstacks` (cùng section với 4
stack IST1 + trang stack user của KT-340b). `budget-x86_64.sh` **báo riêng** `.bootstacks=86.016`
⇒ không byte nào bị giấu; trần **không** đổi. Ảnh sản phẩm không bị ảnh hưởng (không có
`sched.rs`). Nếu sau này muốn đưa 4 stack ấy về `.data+.bss`, phải **gỡ** `Machine` khỏi ảnh kiểm
hoặc tách harness — không nới trần.

---

## 7. Cổng đã chạy

| Cổng | Kết quả |
|---|---|
| `cargo build -p vifast-kbin-x86_64` (không feature, `x86_64-unknown-none --release`) | xanh |
| `cargo build … --features ktest` | xanh |
| `cargo test -p vifast-plat-x86_64 --features core` (host) | xanh — `core_bridge` 2 + `syscall_env` 4 |
| `bash v2/test/boot/run-x86_64.sh` | **36/36 kiểm sạch**, 32/32 ca âm ĐỎ, `qemu_own=0` |
| `bash v2/test/boot/budget-x86_64.sh <sản phẩm>` | `data+bss=43072 ≤ 65536` |
| `bash v2/test/boot/budget-x86_64.sh <ktest>` | `data+bss=251024 ≤ 262144` |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **PASS** 53/53 |
| `bash v2/test/boot/budget-x86_64.sh --selftest` | **PASS** 11/11 |
| `./v2/bin/v2 check` | **EXIT=0** (`TỔNG check ≈ 101 s`) |
| `./v2/bin/v2 gate --dev` (T1) | xem §7.1 |
| `cargo fmt --manifest-path v2/kernel/Cargo.toml --all` | đã chạy |

### 7.1 `gate --dev` (T1)

**PASS** — `gate-steps registered=76 ran=62 skipped=14 failed=0`, `TỔNG gate --dev (T1) 644.856 ms`,
chạy trên cây **sạch** tại commit `7b527258` (bằng chứng T1 gắn với commit đó; sửa journal sau
đó là commit tài liệu riêng, không chạm mã). Hai bước bị T1 bỏ qua in rõ `BỎ QUA (T1 — chỉ chạy ở
T2)` (trong đó có `QEMU virtio-gpu` — T2).

```text
# gate-steps registered=76 ran=62 skipped=14 failed=0
# step-time-total   644856 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
--- QEMU virtio-gpu x86_64 + ca âm không GPU (KT-337/KT-333c): BỎ QUA (T1 — chỉ chạy ở T2)
measure-lock --selftest: PASS (13/13 ca)
```

`T2` (`gate --fast`) **chưa** chạy — task này chỉ được yêu cầu T0+T1 (lệnh của người điều phối);
thay đổi chạm boot/linker/layout ảnh nên ứng viên tích hợp **phải** chạy `gate --fast` trước khi
vào `main` — ghi ở §9.

---

## 8. Vấp và cách sửa (ghi lại để lần sau không lặp)

1. **`dispatch` trả `Switched` rồi `Tcb.Resume` lỗi 5** — `entry` = 0 vì `Tcb.Configure(entry?)`
   từ ring 3 bị **Alignment (6)**: `__user_core_after_suspend` rơi ở offset **lẻ** (0x1c5) mà
   `TCB_ENTRY_ALIGN = 2` (RISC-V có lệnh nén). Sửa: `.balign 2, 0x90` trước nhãn. Bài học: nhãn
   `.user` không tự căn — mọi điểm vào do `Tcb.Configure` nhận phải `balign` tường minh.
2. **`priority_now=4` dù `err_cfg=0`** — `fill_user_buffer` ở `prepare` điền `priority = 4`
   (giá trị **cũ**, để chứng minh "trước = 4") và tôi quên ghi lại **9** ở `start_core_phase`;
   phép đo vì thế đọc đúng 4. Sửa: `set_configure_word(buf_pa, 3, PRIORITY_SET)` — bài học: mọi
   trường buffer do ring 3 "yêu cầu" phải được kernel ghi **sau** lần điền sơ bộ.
3. **`tcb_vspace` trả `u32` (`ObjId`), không phải `u64`** — `unwrap_or(NO_U64)` làm suy luận
   kiểu sai; sửa `NO_TCB`/`u32`. Bài học: `ObjId`/`CapId` là `u32`; đừng "tiện tay" dùng `u64`.
4. **`BootArena::alloc_frame` cần `TableMemory` trong scope** — lỗi biên dịch rõ ràng, chỉ cần
   `use vifast_plat_x86_64::mmu::TableMemory`.
5. **`# core suspend … exit=4` vs regex `exit=0x4`** — runner đỏ vì **một** dòng, TAP 38–43 vẫn
   xanh; sửa `writeln!` sang hex (`0x{:x}`) thay vì nới regex — bài học: bằng chứng máy đọc phải
   **một** định dạng, đừng để runner đoán hai kiểu.
6. **`.data+.bss` ktest 316.560 > 262.144** — không nới trần; tách 4 stack harness sang
   `.bootstacks` NOLOAD (§6.4), `budget` báo riêng. Bài học: D2-243 đã có sẵn lối cho **stack**.

---

## 9. Giới hạn & nợ (ghi thẳng)

1. **Scheduler**: 0 % mô hình harness bị gỡ (2.162 dòng) — có **task con `KT-340e`** (§6.3) với
   tiêu chí đo được; **không** gọi là "đã gỡ scheduler".
2. **SMP + core**: `Machine` được khởi tạo với `harts = 1` (BSP) ở pha rig, **trước** SMP boot
   (TAP 18–21). `num_harts` mà core thấy vì thế là 1 trong suốt pha core; khi `KT-341` mang core
   vào ảnh sản phẩm (hoặc khi scheduler thật chạy đa hart) phải `init` **sau** `smp::run` hoặc
   thêm `Machine::set_harts` — hiện **chưa** có, ghi nợ.
3. **`resolve_addr` = `NotMapped`**: `DebugPutStr` chỉ in từ buffer/IPC buffer đã đăng ký
   (`user_range_ok` fail-closed); syscall nào cần dịch địa chỉ user tuỳ ý (map/unmap) **chưa** hỗ
   trợ — thuộc `KT-341` nếu shell cần.
4. **Pha core chạy trên MỘT hart** (BSP). Bốn hart đã online nhưng chỉ BSP vào pha core; TCB
   `Running{hart:0}`. Đa hart + core là việc của `KT-340e`/`KT-341`.
5. **TAP 14 của lượt 1 vẫn là "ring 3 chạy tiếp"** (không đổi hợp đồng cũ); pha core **không**
   thêm TAP mới ngoài 6 ca — giữ đúng "TAP 37 → 43".
6. **Ảnh sản phẩm không có core** (65.536 B < 184.792 B) — chủ ý, ghi ở §2; `KT-341` quyết định
   ngân sách. Nếu ai đó bật `ktest` cho ảnh sản phẩm, `budget` sẽ đỏ ngay (fail-closed).
