# KT-47 — Timer SBI (K3): tick 100 Hz, timebase 10 MHz, deadline tuyệt đối/hart

Ngày: 2026-11-15 (Asia/Tokyo). Người làm: Agent 2 dòng OS. Task: **KT-47** (`v2/agent/QUEUE.md`),
theo K3-PLAN §4.1 + D2-72 Q6, deviation **D2-74**.

## 0. Trạng thái khi vào

- HEAD `0a14b8a2` (`[KT-00] D2-74: -icount chỉ trong ktest.sh …`); `git rev-list --count
  HEAD..origin/main = 0`; working tree có `viscript/` của tab khác — **không chạm**, chỉ stage `v2/`.
- KT-44/45/46 DONE; KT-47 READY. Baseline `gate --fast`: **569 test / 73 suite PASS**
  ([`kt47-baseline-gate-fast.log`](kt47-baseline-gate-fast.log)).

## 1. Đỏ trước

Sửa `v2/test/boot/ktest.sh` đòi `1..12` + 4 TAP timer + 5 dòng bằng chứng timer, chạy khi **chưa**
có code timer ⇒ [`kt47-red-ktest.log`](kt47-red-ktest.log):
`not ok 1 - ktest boot 1: thiếu plan-1..12 ok8-timer-monotonic ok9-timer-no-lost-tick
ok10-timer-irq ok11-timer-drift ok12-nested timer-arm-evidence timer-monotonic-evidence
timer-drift-evidence timer-irq-evidence timer-sie-evidence`, `EXIT=1`. Đỏ đúng lý do (ảnh KT-46 chỉ
có `1..8`).

## 2. Việc đã làm

- **`vifast-plat-common::timer`** (mới, `#![forbid(unsafe_code)]`): hằng số `[TK]` `TIMEBASE_HZ =
  10_000_000`, `TICK_HZ = 100`, `TICK_TICKS = 100_000`, `TICK_NS = 10_000_000`,
  `TICK_DELTA_MIN_NS/MAX_NS = 9/11 ms` (+ `const` assert); `ticks_to_ns`/`ns_to_ticks`;
  `classify(last, deadline, now) -> TickSample` — deadline **tuyệt đối**, `missed = late_by /
  TICK_TICKS`, `next = deadline + (missed+1)·TICK_TICKS` (luôn > `now`, không cộng dồn từ `now`);
  `TimerCounters`/`TIMER_STATS` + `TimerSnapshot` (`ticks/late/missed/non_monotonic/max_delta/
  min_delta/max_late` + `trap_spie_on`/`handler_sie_off`).
- **`vifast-plat-common::trap`**: `Event::Timer` cho `scause = (1<<63)|5` (**không** phải
  `Exception`), `TrapAction::Timer`, `INTERRUPT_SUPERVISOR_TIMER`; ngắt mã khác vẫn
  `Event::Interrupt` (KT-48).
- **`Platform`** (KT-47): `now_ticks`/`now_ns`, `timebase_hz` (đổi tên từ `tick_hz` cho khỏi lẫn
  `TICK_HZ=100`), `set_timer_deadline`, `enable_timer`, `disable_timer` (+ forward cho `&P`).
- **`vifast-plat-riscv64::timer`** (mới, `unsafe` trong danh sách đóng D2-17): `now_ticks`
  (`rdtime`), `read_sie`/`read_sstatus`, `set_timer_deadline` (SBI TIME EID `0x5449_4D45`, FID 0),
  `enable_timer` (`sie.STIE` **rồi** `sstatus.SIE`, đọc lại), `disable_timer` (đảo thứ tự),
  `init()` (arm `now + TICK_TICKS` → `set_timer` → bật), `on_timer_interrupt`.
- **`trap_dispatch`**: nhánh `Event::Timer` — đọc `sstatus` trong handler (SIE=0), `record_tick`,
  `record_sstatus`, nạp deadline kế (`u64::MAX` nếu chưa arm), `TRAP_STATS.record_interrupt(5)`,
  **resume** bằng `sret`. `trap_entry`/handler **không** ghi `sstatus` (P2a).
- **Ảnh test chính**: 12 TAP — 8..11 timer (100 tick monotonic, 1.000 tick không mất, irq thật
  `scause=0x8000000000000005`, drift Δ ∈ [9,11] ms), nested thành **12**; tắt timer
  (`set_timer(u64::MAX)` + `disable_timer`) trước probe trap lồng.
- **Ảnh sản phẩm**: arm timer + chờ **≥1 tick** (timeout 1 s) + in `# timer arm:`/`# timer:`,
  `timer_ok` kiểm `sie_stie=1`, `sstatus_sie=1`, `sret_sie=1`, `ticks ≥ 1`, monotonic, disable đúng
  — sai ⇒ `PANIC`. **Không icount** (điều kiện (b) D2-74).
- **Host tests**: `tests/timer.rs` mới **12 test** (hằng số/đổi đơn vị, `classify` đúng hẹn / trễ <
  1 tick / trễ trọn 1 tick ⇒ mất tick / ngắt sớm / saturate, 1.000 tick + jitter không mất,
  phát hiện mất tick + tự căn deadline tuyệt đối, monotonic, bằng chứng `sstatus.SPIE/SIE`,
  arm/reset) + **1 test API `Platform`** KT-47 (`now_ns`/`timebase_hz`/`set_timer_deadline`/
  `enable_timer`/`disable_timer`). Tổng `vifast-plat-common`: **34/34** (13 trap + 1 API + 8 MMU +
  12 timer).

## 3. Phát hiện quan trọng — QEMU TCG không icount phục vụ timer ảo rất thưa (D2-74)

Đo trên máy này (QEMU 11.1.3/macOS, `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1`):
timer ảo (`QEMU_CLOCK_VIRTUAL`/ACLINT) chỉ được main loop của QEMU xử lý khi vCPU nhường, nên ngắt
đến **trễ cộng thêm một khoảng lớn, thất thường** so với deadline; 3 s chỉ được ~20 ngắt ⇒ 4 TAP
timer đỏ dù kernel/handler đúng. Không phải lỗi kernel: giống nhau khi (a) busy-spin vs `wfi`
(wfi có tác dụng — user-time QEMU giảm mạnh), (b) gọi ecall M-mode trong vòng chờ, (c) `aclint=on`
hay `off`, (d) `thread=single` hay `multi`. Cùng ảnh + `-icount shift=6` (cố định): 1.000 tick cho Δ
9,98–10,02 ms, `late=missed=0` ⇒ đo đúng **toán deadline** của kernel.

Đối chứng A/B trên **cùng ảnh** (final): [`kt47-qemu-no-icount-vs-icount.log`](kt47-qemu-no-icount-vs-icount.log)
(A: `not ok 8..11`; B: `ok 8..11`, `in_range=1`).

**Quyết định**: lead chốt **phương án 1**, Agent 0 ghi **D2-74** (`0a14b8a2`):
`-icount` **chỉ trong `ktest.sh`**; `run.sh` + soak KT-57 giữ nguyên D2-72 Q2; **ngưỡng [9,11] ms
giữ nguyên**; điều kiện (a) ghi deviation `k3-riscv64.md` §10 + JOURNAL/STATE, (b) giữ bằng chứng
**không-icount** (product arm timer + ≥1 tick, log riêng), (c) soak **không** icount, (d) đo jitter
thật ⇒ task riêng, không đổi D2.

**Tinh chỉnh khi chạy thật (D2-74 đính chính 2026-11-15, `7a2e12d6`)**: shift tự động (`auto`)
**flaky** — QEMU tự đổi shift giữa chừng (bám thời gian thật) làm thời gian ảo nhảy: 100 tick đầu
đúng 99.952–100.048 tick, nhưng trong 1.000 tick xuất hiện Δ ~3,4 ms và ~16,6 ms ⇒ `ok 11 - timer
deadline drift` đỏ ngẫu nhiên (đã tái hiện, kể cả reviewer:
[`kt47-icount-auto-flake.log`](kt47-icount-auto-flake.log) + raw). `ktest.sh` dùng **`-icount
shift=6`** (64 ns/lệnh, **cố định**) ⇒ Δ = 10 ms ± quantization icount, ổn định giữa các lần chạy
(**2 lần liên tiếp PASS**, [`kt47-icount-ktest-run1.log`](kt47-icount-ktest-run1.log) +
[`…-run2.log`](kt47-icount-ktest-run2.log); đo thử `shift=4/6/8` đều `in_range=1`; chọn 6 vì wall
~3,7 s vừa đủ nhanh và quantization nhỏ). Vẫn chỉ `ktest.sh`, vẫn không nới ngưỡng.

## 4. Kết quả đo (do script/cargo in — log, không chép tay vào tài liệu)

| Hạng mục | Kết quả | Log |
|---|---|---|
| Host test `vifast-plat-common` | **34/34** (13 trap + 1 API + 8 MMU + 12 timer) | [`kt47-host-tests.log`](kt47-host-tests.log) |
| Build 3 ảnh | OK (product + ktest + ktest-guard) | [`kt47-build.log`](kt47-build.log) |
| Budget `.data+.bss` | product **272 B**, ktest **280 B**, ktest-guard **272 B** / 262.144 | [`kt47-budget.log`](kt47-budget.log) |
| TAP ảnh test (`-icount shift=6`, D2-74) | **12/12** chính + **3/3** guard × **2 lần liên tiếp**; `ok 8..11 timer`, `in_range=1` | [`kt47-icount-ktest-run1.log`](kt47-icount-ktest-run1.log), [`kt47-icount-ktest-run2.log`](kt47-icount-ktest-run2.log), raw [`kt47-icount-ktest-main-raw.log`](kt47-icount-ktest-main-raw.log), [`kt47-icount-ktest-guard-raw.log`](kt47-icount-ktest-guard-raw.log) |
| Flake `auto` (đối chứng, reviewer tái hiện) | Δ 3,36–16,65 ms ⇒ TAP drift đỏ | [`kt47-icount-auto-flake.log`](kt47-icount-auto-flake.log) |
| Product 50 boot (**không icount**) | **50/50** sạch | [`kt47-boot50.log`](kt47-boot50.log) |
| Product 1 boot raw (không icount) | `# timer arm:` + `# timer: ticks=1 … sret_sie=1` + `ok 1 - k3 boot` | [`kt47-product-boot-nographic-raw.log`](kt47-product-boot-nographic-raw.log) |
| A/B không-icount vs icount (cùng ảnh) | A: timer TAP đỏ; B: xanh, `in_range=1` | [`kt47-qemu-no-icount-vs-icount.log`](kt47-qemu-no-icount-vs-icount.log) |
| Đỏ trước | `ktest.sh` thiếu 4 TAP timer + `plan-1..12` | [`kt47-red-ktest.log`](kt47-red-ktest.log) |
| Baseline gate | **569/73 PASS** | [`kt47-baseline-gate-fast.log`](kt47-baseline-gate-fast.log) |
| Gate cuối | xem [`kt47-gate-fast.log`](kt47-gate-fast.log) | — |

Bằng chứng timer trong 1 boot icount (trích log, số do ảnh in): `# timer arm: … tick_ticks=100000
tick_hz=100 timebase_hz=10000000 sie_stie=1 sstatus_sie=1`; `# timer ticks: ticks=1000 …
non_monotonic=0 late=0 missed=0`; `# timer irq: interrupts=1000 scause=0x8000000000000005 code=5
exceptions=2 exceptions_before=2 sie_stie=1`; `# timer sie: trap_spie_on=1000
handler_sie_off=1000 ticks=1000 sie_stie=1 sret_sie=1`; `# timer drift: … in_range=1`; `# timer
disable: sie_stie=0 sstatus_sie=0`; `ok 12 - trap nested panic` + `PANIC: nested trap …`.

## 5. Ghi chú kỹ thuật / bài học

- **`sstatus.SIE` trong trapframe luôn = 0**: hardware làm `SPIE ← SIE`, `SIE ← 0` **trước** khi
  asm đọc `sstatus`, nên bằng chứng "bị ngắt khi đang bật ngắt" là **`SPIE = 1`** (field
  `trap_spie_on`), không phải `SIE`. Bản nháp đầu kiểm `SIE` ⇒ `trap_sie_on=0` dù mọi thứ đúng.
- **Không dùng `wfi` trong vòng chờ tick của test**: dưới `-icount`, thời gian ảo tiến theo lệnh
  đã chạy; `wfi` làm thời gian ảo đứng ⇒ tick trễ. Busy-spin (`spin_loop`) + deadline tuyệt đối.
- **Deadline tuyệt đối**: `next = deadline + (missed+1)·TICK_TICKS` giữ Δ tick không trôi; nếu
  `missed > 0` thì nhảy qua các deadline bị bỏ **một lần** (không dồn interrupt).
- **Đếm tick == số ngắt**: `TRAP_STATS.interrupts` và `TIMER_STATS.ticks` phải bằng nhau; handler
  **không in** (in trong handler làm trễ/tăng jitter).
- Product **không icount** nên `# timer: delta_ns` của lần boot đó có thể lớn (QEMU phục vụ timer
  thảo); ảnh sản phẩm chỉ đòi **≥1 tick**, monotonic, SIE/STIE đúng chỗ — **không** dùng để đo
  drift (D2-74).
- `rg`/grep product: ảnh sản phẩm **0** chuỗi `ktest`; ảnh test có chuỗi `ktest` (D2-08).

## 6. Review độc lập

`reviewer-kt47` **APPROVE code** (tự chạy host **34/34**; ktest **2 lần liên tiếp** PASS 12/12 + 3/3
với Δ 10,0000–10,0192 ms, `late=missed=non_monotonic=0`, `interrupts=1000 scause=0x8…005`,
`trap_spie_on=handler_sie_off=1000`; product **50/50** boot; gate **569**; product `ktest-strings=0`;
tái hiện độc lập flake shift tự động: `min=3.361.400 ns max=16.648.300 ns in_range=0` ⇒ lý do đổi
`shift=6` là thật, không phải nới ngưỡng). 8 mục tài liệu/bằng chứng đã vá trước commit.

**Giới hạn phép đo** (reviewer nêu, đã ghi `k3-riscv64.md` §10.7): (1) Δ chỉ đúng dưới icount —
không chứng minh jitter host; (2) `late_by ≤ vài trăm µs` nên boot TAP **chưa phân biệt** deadline
tuyệt đối vs `now + TICK` (phân biệt nhờ host test `classify` + thiết kế); (3) `shift=6` chưa test
dưới tải host nặng; (4) `smp>1` + icount chưa kiểm (KT-49/KT-54); (5) jitter thật = task riêng
(D2-74 (d)).

## 7. Không làm (đúng phạm vi)

PLIC/UART IRQ (KT-48), SMP/PerHart (KT-49), IPI (KT-50), ASID/shootdown (KT-51), object layer
(KT-52), U-mode (KT-53), harness matrix (KT-54), conformance (KT-55), `FaultKind`/W^X/spec (KT-56),
soak/K3-DOD (KT-57), port core (K3b); `spec/`, sim, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`
**không chạm**.
