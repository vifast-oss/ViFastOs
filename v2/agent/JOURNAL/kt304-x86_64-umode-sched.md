# KT-304 — U-mode task trong scheduler x86_64 (K5')

Trạng thái: **DONE** — nhánh `task/kt304-x64umode`, base `main` @ `58a1f867`, 2026-11-21.
Sub-agent `x64ring3`. Quyết định: **D2-201** (K5' bước 1), **D2-212** (KT-286 preemption),
**D2-216** (KT-297/298), **D2-219** (KT-301), **D2-198** (uỷ quyền commit nhánh task),
**D2-05** (test đỏ trước), **D2-07** (không retry), **D2-80** (neo ảnh).

> **D2-219 không tồn tại** trong `v2/agent/DECISIONS.md` tại base `58a1f867` (mục cuối là
> **D2-218**). Nhiệm vụ yêu cầu đọc D2-219 ⇒ đã đọc D2-201/D2-212/D2-216 và **ghi lại** việc
> thiếu mục này thay vì tự suy ra nội dung. **Sau khi làm xong**, `main` tiến lên `2828a67f`
> (`[KT-304] vòng 18: D2-219 mở KT-304 …`) — D2-219 nay **có** và nội dung khớp phạm vi đã làm:
> "tác vụ ring 3 được `schedule()` chọn, bị preempt, quay lại ring 3; TAP mới + ca âm", **không**
> nới ngưỡng, **không** sửa `spec/` (cần syscall mới thì dừng và báo — bản này **không** cần).

**Không** tuyên bố K5' đạt.

## 1. Khảo sát trước (bắt buộc — trả lời ba câu hỏi của task)

### 1.1 `switch_frame_ok` (KT-298) có chấp nhận frame ring 3 không? — **KHÔNG**

`vifast-plat-x86_64::sched::switch_frame_ok(rip, cs, rflags, rsp)` (KT-298) kiểm bốn điều kiện,
trong đó điều kiện (2) là **`cs == exc::KERNEL_CS` (`0x08`)**. Frame của tác vụ ring 3 có
`cs = 0x1B` ⇒ bị từ chối **ngay** ở điều kiện (2), trước cả khi xét `rsp`.

Hai điểm nữa:

- Hàm **không nhận `ss`** — mà frame ring 3 có `ss = 0x23` (`USER_SS`), còn frame kernel thật
  (CPU đẩy khi cùng CPL) có ô `ss` **ngoài** frame (QEMU pop thêm) nên không kiểm được. Muốn
  phân biệt hai loại frame phải thêm tham số `ss` và chỉ kiểm nó khi `cs` là user.
- Điều kiện `rsp` hiện tại (khác 0, căn 8, canonical) **không đủ** cho ring 3: `rsp` của frame
  ring 3 là **user rsp**, phải nằm trong trang stack user `[USER_STACK_VA, USER_STACK_TOP]`,
  không phải một `rsp` kernel bất kỳ.

**Đính chính selector**: task viết "`cs=0x23`/`ss=0x1b`" — trong code này **ngược lại**:
`USER_CS = 0x1B` (code, `gdt::SELECTOR_USER_CODE_RPL3`), `USER_SS = 0x23` (data,
`gdt::SELECTOR_USER_DATA_RPL3`), xem `v2/kernel/vifast-plat-x86_64/src/umode.rs:54-57` và
`tests/umode.rs::user_selectors_literal`. Bản làm theo **code** (0x1B/0x23), không theo câu chữ
đã hoán đổi.

Ngoài ra `vifast-kbin-x86_64::sched::frame_ok` chỉ nhận ba slot `MAIN/A/B`; `slot_stack` trả
`None` cho slot khác ⇒ thêm slot `TASK_U` là **bắt buộc**, không chỉ nới hàm thuần.

### 1.2 `TSS.rsp0` có được nạp khi vào ring 3 không? — **có, nhưng chỉ cho đường KT-270**

`vifast-kbin-x86_64::umode::run` gọi `tss::set_rsp0(0, __syscall_stack_top)` **ngay trước**
`asm::enter_user(...)` (KT-270). Vậy đường một chiều `kmain → ring 3 → int 0x80 → Exit` có
`RSP0` = stack syscall 32 KiB.

**Nhưng scheduler thì không**: `sched::run` không đụng `TSS.RSP0`. Nếu `schedule()` `iretq`
xuống ring 3 mà không nạp lại `RSP0`, tick timer từ CPL3 sẽ đẩy frame vào **`__syscall_stack_top`
— đúng stack mà `main` đang chạy** (sau `Exit`, toàn bộ `net::run → smp::run → sched::run` chạy
trên stack syscall; xem chú thích `slot_stack` trong `sched.rs`). Frame của `main` đã lưu ở
`SCHED_RSP[TASK_MAIN]` sẽ bị ghi đè ⇒ hỏng ngữ cảnh, đúng lớp lỗi boot 410 của KT-298.

⇒ Phải cấp **stack kernel riêng** cho tác vụ ring 3 (16 KiB, cùng khuôn `TASK_A_STACK`) và
`tss::set_rsp0(0, u_kernel_stack_top)` **trước** lần `iretq` đầu tiên xuống ring 3.

### 1.3 Frame `iretq` của tác vụ ring 3 — khác frame kernel ở đâu?

Cùng **22 qword** (`FRAME_WORDS`), cùng offset (`rip` 136, `cs` 144, `rflags` 152, `rsp` 160,
`ss` 168), nhưng:

| Ô | Tác vụ kernel (KT-286) | Tác vụ ring 3 (KT-304) |
|---|---|---|
| `cs` | `0x08` (`KERNEL_CS`) | **`0x1B`** (`USER_CS`) |
| `ss` | `0x10` (`KERNEL_DS`) | **`0x23`** (`USER_SS`) |
| `rsp` | `kernel_stack_top - 8` (SysV `%16 == 8`) | **`USER_STACK_TOP - 16`** (user rsp) |
| `rsp` khởi tạo (đầu frame) | `stack_top - 176 - 8` (slack 8) | **`kernel_stack_top - 176`** (không slack) |

Vì sao **không slack** cho ring 3: `iretq` đổi CPL **luôn** pop 5 qword (`SS:RSP:RFLAGS:CS:RIP`)
trên cả QEMU lẫn CPU thật (SDM Vol 3A §5.5), nên frame kết thúc đúng `kernel_stack_top`; sau
`iretq` kernel `rsp = kernel_stack_top`. Khi timer ngắt từ CPL3, CPU đẩy 5 qword vào
`kernel_stack_top - 40`, stub đẩy 17 qword ⇒ frame mới ở **đúng** `kernel_stack_top - 176` —
trùng chỗ frame khởi tạo, không tràn. Slack 8 của tác vụ kernel là để tạo `rsp % 16 == 8` sau
`iretq` cho SysV; tác vụ ring 3 không `call`/`ret` bằng kernel rsp nên không cần.

`rsp` user `USER_STACK_TOP - 16` chừa 8 byte cuối trang cho **bộ đếm** mà tác vụ ring 3 tự
tăng (`USER_COUNTER_VA = USER_STACK_TOP - 8`), kernel đọc lại sau khi scheduler dừng.

## 2. Phạm vi đã làm

| Việc | Kết quả |
|---|---|
| Test host **đỏ-trước** (D2-05) | **ĐỎ** — `cargo test -p vifast-plat-x86_64 --test sched` không biên dịch (`init_user_frame_words`/`TASK_SEQUENCE`/`TASK_U` chưa có; `switch_frame_ok` 4→5 tham số) |
| `vifast-plat-x86_64::sched` | **DONE** — `TASK_U` (slot 3), `TASK_SEQUENCE` 9 bước, `init_user_frame_words`, `switch_frame_ok(rip, cs, rflags, rsp, ss)` có nhánh ring 3 |
| `vifast-kbin-x86_64::umode` | **DONE** — stub `.user` thứ hai `__user_sched_entry` (tăng bộ đếm), `USER_TASK_RSP`, `USER_COUNTER_VA`, `sched_entry_va()` |
| `vifast-kbin-x86_64::sched` | **DONE** — `TASK_U_STACK` 16 KiB, `build_user_frame`, `slot_stack(TASK_U)`, `frame_ok` đọc `ss`, `on_tick` đo preempt/resume, `run` nạp `TSS.RSP0` + zero bộ đếm + in bằng chứng + TAP 24 |
| `vifast-kbin-x86_64::main` | **DONE** — `TAP_TESTS = 26`; bỏ `.p2align 12` trong `.user` (stub thứ hai có thể phát **trước** khối `main.rs`; căn 4 KiB giữa section làm `ASSERT` linker đỏ) |
| `vifast-kbin-x86_64::net` | **DONE** — blk dời `tap_first + 9` → `+ 10` (TAP 25/26) |
| Test host | **10/10** (`cargo test -p vifast-plat-x86_64 --test sched`) |
| Boot QEMU | **TAP 26/26 xanh** — log §3 |
| Runner | **18/18 kiểm sạch + 16/16 ca âm ĐỎ** (thêm ca (p) "tác vụ ring 3 không chạy lại") |
| Soak `--smoke 60 3` | **60/60** — log §5 |
| `./v2/bin/v2 check` | **PASS** |
| Đột biến (i) `no-rsp0` | **ĐỎ** — `# exc: vec=8 … cs=0x1b` (ĐỎ, không marker) |
| Đột biến (ii) `reject-user-cs` | **ĐỎ** — `# sched: BAD frame slot=3 …` (ĐỎ, không marker) |

## 3. Đo được (log QEMU nguyên văn)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: ticks=10 tasks=3
# sched: runs a=283832 b=157811 switches=9 ab=1 ba=2 a_rsp=0x12e168 b_rsp=0x132168 u_rsp=0x136170
# sched: umode task ran n=5015211 ring=3 preempted=2 resumed=1
# sched: umode counts first=4279886 last=5015211 cs=0x1b kernel_stack=0x136220
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - blk: device
ok 26 - blk: sector0
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

| Phép đo | Kết quả |
|---|---|
| tác vụ ring 3 chạy thật | `n=5015211` — bộ đếm do **chính tác vụ** tăng trong vòng lặp `inc [rcx]` ở CPL3 |
| đang ở ring 3 khi bị preempt | `cs=0x1b` — `cs` của frame do CPU đẩy khi timer ngắt từ CPL3 |
| preempt | `preempted=2` — rời U hai lần (`U→A`, `U→main`) |
| chạy lại | `resumed=1`; `last=5015211 > first=4279886` ⇒ bộ đếm **tăng** giữa hai lần preempt (chạy lại thật) |
| stack kernel riêng | `u_rsp=0x136170`, `kernel_stack=0x136220` — frame trong `TASK_U_STACK` 16 KiB, **không** đụng stack syscall của `main` |
| TAP | 26/26 xanh; runner 18/18 + 16 ca âm ĐỎ |

## 4. Đột biến (đều ĐỎ)

| Đột biến | Cách làm | Log nguyên văn | Kết quả |
|---|---|---|---|
| (i) **không nạp `TSS.RSP0`** | `--features no-rsp0` (nạp `RSP0 = 0`) | `# exc: vec=8 err=0x0 rip=0x400000005a cs=0x1b rflags=0x206` + `# exc: frame rsp=0x11bf50 ss=0x23 task_rsp=0x4000001ff0` | halt, **không marker** (ĐỎ) |
| (ii) **`switch_frame_ok` từ chối `cs=0x1B`** | `--features reject-user-cs` | `# sched: BAD frame slot=3 rsp=0x136170` + `# sched: BAD frame fields rip=0x4000000050 cs=0x1b rflags=0x202 task_rsp=0x4000001ff0 ss=0x23` | halt, **không marker** (ĐỎ) |

## 5. Soak `--smoke 60 3`

```text
# soak: xong wall=86s; ok=60 fail=0 tổng=60/60
# guard: boot_end=61 rc_124=0 qemu_own=0
# soak: BUILD_ID=50611635a6b42cee sha256=b4f5f19bfbd7272dd0c3d8d8a276cb469baf0f5755c903a4931ae33bddbff9b5 git=58a1f86768d3bbcaa076d249325db8c91742b1e6
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5'
```

`--smoke` **luôn exit 1** theo thiết kế (N < 600, D2-24) — "xanh" ở đây là **60/60 boot, 0 lỗi**,
`rc_124=0`, `qemu_own=0`, ảnh không đổi giữa soak. **Không** phải bằng chứng cổng K5'.

## 5b. Runner + `v2 check`

```text
# 18/18 kiểm sạch (boot=1, tests=26, exc=5)
# ca âm: 16/16 ĐỎ đúng như phải
# infra: qemu_own=0
```

`./v2/bin/v2 check` — **PASS** (rc=0, 123 s; fmt + clippy `-D warnings` + luật R1..R5 + mọi
selftest).

## 7. Xác nhận

- **Không** push, **không** merge, **không** chạm `main`.
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng, **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common`.
- **Không** tuyên bố K5' đạt.

## 6. Còn gì chưa xong / rủi ro

1. **Một** tác vụ ring 3, **không** IPC giữa các tác vụ ring 3, **không** nhiều hart schedule
   (chỉ BSP), **không** priority, **không** sleep/wake — đúng phạm vi task.
2. Tác vụ ring 3 **không** gọi syscall (không có số hiệu mới — `spec/` không mở trong task này);
   bằng chứng "chạy lại" là **bộ đếm user** kernel đọc trực tiếp, không phải in từ ring 3.
3. `TASK_SEQUENCE` là **bảng cố định** (không phải runqueue): đủ cho phép đo "chọn → preempt →
   chạy lại", **không** phải scheduler sản phẩm.
4. **Không** tuyên bố K5' đạt: K5' còn "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot) và các mục
   ngoài soak (§12.12 của `k5-x86_64-boot.md`).
