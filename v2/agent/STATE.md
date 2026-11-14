# STATE — ViFastOs v2 (vòng 1)

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.

```
HEAD          : `b0e9f91b` ([KT-00] mở KT-48) → `5db8eaae` ([KT-00] **D2-75** của Agent 0: `probe_ndev` không phá trạng thái) — commit `[KT-48]` nằm trên `5db8eaae`, chỉ `v2/`
BUILD_ID      : — (ảnh K3 KT-48: `vifast-kbin-riscv64` 55.560 B, `vifast-kbin-ktest` 74.264 B, `vifast-kbin-ktest-guard` 51.000 B — `JOURNAL/kt48-budget.log`/`kt48-ktest.log`)
Task          : **KT-48 (PLIC + UART IRQ)** — **DONE (`reviewer-kt48` DRIFT nhẹ: code/DoD đạt, **không** UNSAFE; 4 mục đã vá + D2-75)**: PLIC cổ điển `0x0c00_0000` map **2×2 MiB Device RW NX** (priority/pending/enable + context), context S-mode **`2*hart+1`** (M-mode `2*hart` không bật), `ndev` **probe register** (write-1 → read-back → khôi phục 0 → read-back 0, panic nếu sai — D2-75), threshold **0**, priority IRQ 10 **1**; UART0 IRQ 10 bật `IER.TX-empty`, handler `claim → đọc IIR → complete → re-arm IER 0→2` (level-triggered có gateway), tắt `IER` + enable sau **N=8** ngắt; `scause=(1<<63)|9` ⇒ `Event::Interrupt{code:9}` → PLIC (line lạ đếm + complete, **0 IRQ giả**); `Platform::enable_irq/disable_irq` **PLIC thật** (TAP gọi wrapper); TAP **8..10 PLIC/UART** + timer **11..14** (chạy **sau** PLIC — hồi quy cùng boot) + nested **15**; product cũng bật PLIC+UART IRQ (**không icount**); host test **46/46**; ktest **15/15 + 3/3**; product **50/50**; gate **569**; budget **344/352/344 B**
Bước          : 8/8 xong (baseline gate 569 → đỏ trước: `ktest.sh` đòi 3 TAP PLIC/UART + `plan-1..15` → `plat-common::plic` (layout/context/stats/host test) + `trap` mã 9 → arch `plic` (probe ndev/init/claim/complete/IER/gate) + MMU 2×2 MiB + `trap_dispatch` nhánh 9 + `Platform` IRQ thật → ảnh test 15 TAP + product PLIC → host 12 test PLIC (tổng 46) → reviewer DRIFT nhẹ: vá `interrupts_delta` thành hiệu thật, thêm TAP `Platform::enable_irq/disable_irq`, quiet 1 ms khớp docs, probe khôi phục 0 + đọc lại (D2-75), sửa comment TAP → ktest 15/15+3/3 ×2, product 50/50, gate 569) — **đã commit `[KT-48]`; `[KT-48b]` cập nhật HEAD + verdict + push**
Nhánh         : main
Cổng gần nhất : `./v2/bin/v2 gate --fast` — **PASS 569 test/73 suite, 0 failed** (`JOURNAL/kt48-gate-fast.log`); baseline cùng số (`JOURNAL/kt48-baseline-gate-fast.log`). Kernel **không** nằm trong `gate --fast` (`v2/Cargo.toml` `exclude = ["kernel"]`) — kernel kiểm bằng build 3 ảnh + `ktest.sh` (**icount shift=6**, D2-74) + `run.sh` (**không icount**, D2-72 Q2) + host test **46/46** (`JOURNAL/kt48-{host-tests,ktest,ktest-2rounds,ktest-main-raw,ktest-guard-raw,boot50,budget,product-boot-nographic-raw}.log`).
Miri          : KT-48 — **không chạy Miri** trên ảnh `no_std`/asm/MMIO (D2-72 Q12); Miri vẫn áp `lib/` + sim. Phần **logic** PLIC chạy host: `cargo test -p vifast-plat-common` **46/46** = 13 trap KT-45 + 1 API `Platform` KT-47 + 8 MMU KT-46 + 12 timer KT-47 + **12 PLIC KT-48** (hình học đăng ký, context `2*hart+1` cho hart 0..3, word/bit enable, offset threshold/claim, hai cửa sổ phủ `MAX_HARTS=8`, phân loại claim 3 nhánh, claim/complete cân bằng + 0 IRQ giả, limit tắt sau N, `ndev` không giả định 95, priority > threshold).
Việc dở       : **`reviewer-kt48` DRIFT nhẹ (code/DoD đạt, không UNSAFE)**, 4 mục đã vá + D2-75; đã commit `[KT-48]`; `[KT-48b]` cập nhật HEAD + push. Kế tiếp Agent 0 lật KT-48 → KT-49 SMP. `viscript/` + `review/` của tab khác **không chạm** (chỉ stage `v2/`).
```



## KT-48 — PLIC + UART IRQ (claim/complete, context `2*hart+1`, 0 IRQ giả)

**Vào**: READY; HEAD `b0e9f91b`; `HEAD..origin/main = 0`; working tree có `viscript/` của tab khác —
**không chạm**; baseline `gate --fast` **569 PASS** ([`kt48-baseline-gate-fast.log`](JOURNAL/kt48-baseline-gate-fast.log)); baseline ktest KT-47 **12/12 + 3/3**
([`kt48-baseline-ktest.log`](JOURNAL/kt48-baseline-ktest.log)).

**Mục tiêu** (K3-PLAN §4.1 KT-48 + D2-72 Q7/Q8): PLIC cổ điển thật — priority/enable/claim/complete,
context `2*hart+1`, threshold 0, `ndev` đọc từ register; UART0 IRQ 10 bật `IER.TX-empty`, handler
`claim → xử lý → complete`, đếm và tắt sau N; `scause` bit 63 + mã 9 ⇒ `Event::Interrupt` đi vào
PLIC (không nhầm timer 5 / exception); line ngoài dự kiến đếm + complete (không treo); TAP `uart irq >= 1`,
claim/complete cân bằng, context đúng hart, 0 IRQ giả, plic threshold, timer vẫn chạy sau PLIC cùng
boot; 50 boot; gate 569; budget.

**Đỏ trước** ([`kt48-red-ktest.log`](JOURNAL/kt48-red-ktest.log)): `ktest.sh` đòi `1..15` + 3 TAP
PLIC/UART + 4 dòng bằng chứng; ảnh KT-47 chỉ có `1..12` ⇒ `not ok 1 … thiếu plan-1..15
ok8-plic-window ok9-uart-irq ok10-plic-no-fake-irq ok11-timer-monotonic ok12-timer-no-lost-tick
ok13-timer-irq ok14-timer-drift ok15-nested plic-window-evidence plic-map-evidence uart-irq-evidence
plic-quiet-evidence`, `EXIT=1`.

**Kết quả**:
- **`vifast-plat-common::plic`** (mới, `forbid(unsafe_code)`): priority `0x0`, pending `0x1000`,
  enable `0x2000+0x80*ctx`, context `0x20_0000+0x1000*ctx` (threshold `+0`, claim `+4`), max source
  **1023**; `context_of_hart=2*h+1`/`machine_context_of_hart=2*h`; `classify_claim`
  (Uart/Spurious/Unexpected); `UART_IRQ_LIMIT=8`; `PlicStats`/`PLIC_STATS` (traps/claims/
  completions/uart_irqs/spurious/unexpected/last_irq/last_iir(normalize `IIR&0x0f`)/uart_ier_off,
  `balanced()`, `fake_irqs()`).
- **`trap`**: `INTERRUPT_SUPERVISOR_EXTERNAL = 9`; doc phân biệt 5 timer / 9 external / khác fatal.
  **`Platform::enable_irq/disable_irq`** = PLIC thật (set/clear bit context `2*hart+1` + đọc lại).
- **`vifast-plat-riscv64::plic`** (mới, `unsafe` thuộc D2-17): `probe_ndev` (write-1/read-back/
  restore-0/read-back-0, panic nếu sai — **D2-75**), `init` (threshold 0, priority[10]=1, enable
  ctx S-mode, kiểm ctx M-mode=0 + claim rỗng), `claim`/`complete`, `enable_external` (`sie.SEIE`
  rồi `sstatus.SIE`), `IER.TX-empty`/`IIR`, `on_external_interrupt` (claim → IIR → complete →
  re-arm `IER 0→2`; đủ N tắt `IER`+enable; line lạ đếm+complete; không in).
- **`trap_dispatch`**: `Event::Interrupt{code:9}` → PLIC + `TRAP_STATS.record_interrupt(9)` (không
  fatal); mã khác 5/9 vẫn fatal. **`mmu`**: PLIC **2×2 MiB Device RW NX** (`0x0c00_0000` +
  `0x0c20_0000`), UART vẫn 4 KiB.
- **Ảnh test 15 TAP** (8..10 PLIC/UART, 11..14 timer **sau** PLIC, 15 nested); **ảnh product**
  bật PLIC+UART IRQ (không icount), in `# plic:`/`# uart irq:`, sai ⇒ PANIC. `ktest.sh` đòi
  `1..15` + 4 bằng chứng (`# plic:` có `plat_off=1 plat_on=1`, `# plic map:`, `# uart irq:`,
  `# plic quiet:`); `interrupts_delta` là **hiệu thật** từ trước `plic::init`.
- **Host tests**: `tests/plic.rs` mới **12 test**; tổng plat-common **46/46**.
- **Docs**: `k3-riscv64.md` §2/§4/§6/§7/§8.1/§9/§10.6 + **§11 mới** (PLIC register map, D2-75
  probe không phá huỷ, đường ngắt level/gateway/re-arm, TAP + bằng chứng, giới hạn).

**Số đo** (do script in — [`kt48-ktest.log`](JOURNAL/kt48-ktest.log),
[`kt48-boot50.log`](JOURNAL/kt48-boot50.log)): ktest **15/15** + guard **3/3** × **2 vòng**;
`# plic: … ndev=95 ctx=1 plat_off=1 plat_on=1 threshold=0 priority_uart=1 enable_ctx=1
enable_mmode=0 claim_idle=0`; `# uart irq: irqs=8 claims=8 completes=8 traps_delta=8 balanced=1
spurious=0 unexpected=0 last_irq=10 last_iir=0x2 ier_off=1 plic_enable=0`; `# plic quiet: …
external_traps=8 interrupts_delta=8 spurious=0 unexpected=0`; timer `ok 11..14`
`ticks=1000 late=missed=0`, `scause=0x8000000000000005`, `in_range=1`; product **50/50** (không
icount); host **46/46**; budget **344/352/344 B**/262.144; gate **569**.

**Review độc lập**: `reviewer-kt48` **DRIFT nhẹ** — code/DoD đạt, **không UNSAFE**; tự chạy host
46/46, ktest 15/15+3/3, product 50/50, gate 569; 4 mục vá trước commit: `interrupts_delta` thành
**hiệu thật**; TAP 8 **gọi thật** `Platform::enable_irq/disable_irq` (bằng chứng `plat_off=1
plat_on=1`); cửa sổ quiet **1 ms** khớp docs; `probe_ndev` khôi phục 0 + đọc lại 0, panic nếu sai
(**D2-75**) + nit comment TAP. Sau vá chạy lại đủ (46/46, 15+3 ×2, 50/50, gate 569).

**Không làm (đúng phạm vi)**: SMP/PerHart (KT-49), IPI (KT-50), ASID/shootdown (KT-51), object
layer (KT-52), U-mode (KT-53), harness matrix (KT-54), conformance (KT-55), `FaultKind`/W^X
(KT-56), soak/K3-DOD (KT-57), port core (K3b), AIA/MSI; `spec/`, sim, `.scn`, QUEUE/DECISIONS/
AGENTS, `viscript/`, `review/` **không chạm**.

## KT-47 — Timer SBI (`SBI TIME`, tick 100 Hz, deadline tuyệt đối, D2-72 Q6 + D2-74)

**Vào**: READY; HEAD `0a14b8a2`; `HEAD..origin/main = 0`; working tree có `viscript/` của tab khác —
**không chạm**; baseline `gate --fast` **569/73 PASS** ([`kt47-baseline-gate-fast.log`](JOURNAL/kt47-baseline-gate-fast.log)).

**Mục tiêu** (K3-PLAN §4.1 KT-47 + D2-72 Q6): SBI `set_timer` (EID `0x54494D45`), `sie.STIE`, timebase
10 MHz đọc bằng `rdtime`, tick 100 Hz; deadline/hart, monotonic, phát hiện mất tick ("trễ > 1 tick"),
đếm `ticks/late/max_delta`; API `Platform` timer; trap `scause` bit 63 + mã 5 ⇒ `Event::Timer`
(**không** exception); TAP 4 timer; 50 boot; gate 569; budget.

**Đỏ trước** ([`kt47-red-ktest.log`](JOURNAL/kt47-red-ktest.log)): `ktest.sh` đòi `1..12` + 4 TAP
timer + 5 dòng bằng chứng; ảnh KT-46 chỉ có `1..8` ⇒ `not ok 1 … thiếu plan-1..12 ok8-timer-monotonic
ok9-timer-no-lost-tick ok10-timer-irq ok11-timer-drift ok12-nested timer-*-evidence`, `EXIT=1`.

**Kết quả**:
- **`vifast-plat-common::timer`** (mới, `forbid(unsafe_code)`): hằng số `[TK]` (`TIMEBASE_HZ=10_000_000`,
  `TICK_HZ=100`, `TICK_TICKS=100_000`, `TICK_NS=10_000_000`, `TICK_DELTA_MIN/MAX_NS` = 9/11 ms +
  `const` assert), `ticks_to_ns`/`ns_to_ticks`, `classify(last, deadline, now)` (deadline **tuyệt
  đối**, `missed = late_by / TICK_TICKS`, `next = deadline + (missed+1)·TICK_TICKS` luôn > `now`),
  `TimerCounters`/`TIMER_STATS` + `TimerSnapshot` (tick/late/missed/monotonic/max_delta/min_delta/
  max_late + `trap_spie_on`/`handler_sie_off`).
- **`trap`**: `Event::Timer` cho `scause=(1<<63)|5` (**không** phải `Exception`), `TrapAction::Timer`,
  `INTERRUPT_SUPERVISOR_TIMER`; ngắt mã khác vẫn `Event::Interrupt` (KT-48).
- **`Platform`**: `now_ticks`/`now_ns`, `timebase_hz` (đổi tên từ `tick_hz`), `set_timer_deadline`,
  `enable_timer`, `disable_timer` (+ forward `&P`).
- **`vifast-plat-riscv64::timer`** (mới; `unsafe` thuộc D2-17): `rdtime`, `csrr sie/sstatus`, SBI TIME
  ecall (`a7/a6/a0`, `a0/a1` lateout), `enable_timer` (`sie.STIE` **rồi** `sstatus.SIE`), `disable_timer`
  (đảo), `init()` (arm → `set_timer` → bật), `on_timer_interrupt`.
- **`trap_dispatch`**: nhánh `Event::Timer` — đọc `sstatus` trong handler (SIE=0), `record_tick`,
  `record_sstatus`, nạp deadline kế (`u64::MAX` nếu chưa arm), `TRAP_STATS.record_interrupt(5)`,
  resume bằng `sret`; **không** ghi `sstatus` (P2a).
- **Ảnh test**: 12 TAP (8..11 timer + nested 12); tắt timer (`u64::MAX` + `disable_timer`) trước probe
  trap lồng. **Ảnh product**: arm + chờ ≥1 tick (timeout 1 s), in `# timer arm:`/`# timer:`; sai ⇒ PANIC.
- **D2-74 (deviation)**: QEMU TCG **không icount** chỉ phục vụ timer ảo rất thưa (đo được: ~20 ngắt/3 s,
  Δ tick ngoài [9,11] ms — **giới hạn host**, không phải kernel; giống nhau với `wfi`/spin/ecall-poll,
  `aclint=on|off`, `thread=single|multi`). Lead chốt: `-icount` **chỉ** trong `ktest.sh` (D2-74 đính chính: **shift=6 cố định**);
  `run.sh` + soak KT-57 giữ nguyên D2-72 Q2; ngưỡng **[9,11] ms giữ nguyên**. Bằng chứng A/B cùng ảnh:
  [`kt47-qemu-no-icount-vs-icount.log`](JOURNAL/kt47-qemu-no-icount-vs-icount.log); product **không
  icount** vẫn chạy 50/50 ([`kt47-boot50.log`](JOURNAL/kt47-boot50.log)).

**Số đo** (do script in — [`kt47-icount-ktest.log`](JOURNAL/kt47-icount-ktest.log)): ktest **12/12** +
guard **3/3**; `# timer ticks` 100/1.000 tick `late=missed=non_monotonic=0`, `drift in_range=1`;
`interrupts=1000 scause=0x8000000000000005 code=5`, `trap_spie_on=handler_sie_off=1000`, `sret_sie=1`;
host **34/34**; product **50/50**; budget **272/280/272 B**/262.144; gate **569/73**.

**Review độc lập**: `reviewer-kt47` **APPROVE code** — tự chạy host **34/34**, ktest **2 lần liên tiếp** PASS 12/12 + 3/3 (Δ 10,0000–10,0192 ms, `late=missed=non_monotonic=0`, `interrupts=1000 scause=0x8000000000000005`, `trap_spie_on=handler_sie_off=1000`), product **50/50**, gate **569**, product `ktest-strings=0`; tái hiện độc lập flake shift tự động (`min=3.361.400 ns max=16.648.300 ns in_range=0`) ⇒ đổi `shift=6` là thật. 8 mục tài liệu/bằng chứng đã vá trước commit; 5 giới hạn phép đo ghi §10.7 + [`kt47-journal.md`](JOURNAL/kt47-journal.md) §6.

**Không làm (đúng phạm vi)**: PLIC/UART IRQ (KT-48), SMP/PerHart (KT-49), IPI (KT-50), ASID (KT-51),
object layer (KT-52), U-mode (KT-53), harness matrix (KT-54), conformance (KT-55), `FaultKind`/W^X
(KT-56), soak/K3-DOD (KT-57), port core (K3b), đo jitter host (task riêng — D2-74 (d)); `spec/`, sim,
`.scn`, QUEUE/DECISIONS/AGENTS, `viscript/` **không chạm**.

## KT-46 — Sv39 thật + guard page-fault (`scause` 13/15)

**Vào**: READY (`agentctl next --project v2` trả KT-46); HEAD `669e2cbc` ([KT-00] lật KT-45 DONE,
mở KT-46); `HEAD..origin/main = 0`; working tree có `viscript/` + `review/` của tab khác — **không
chạm**; baseline `gate --fast` **569/73 PASS** ([`kt46-baseline-gate-fast.log`](JOURNAL/kt46-baseline-gate-fast.log)).
Trong lúc làm, tab ViScript commit/push `1af01abd` (ST-00) — chỉ `viscript/`, không đụng `v2/`.

**Mục tiêu** (K3-PLAN §4.1 KT-46 + D2-72 Q5/Q7 + D2-73): root page table Sv39 từ arena RAM sau
`__kernel_end`; map kernel identity + nửa cao 2 MiB, physmap NX, MMIO UART/PLIC 4 KiB Device,
`.stack` + `.trap_stack` (NOLOAD) + arena; guard page `NONE` ngay dưới `__stack_bottom`; bật `satp`
hai bước (map xong mới `csrw satp` + `sfence.vma`), walk phần mềm ↔ phần cứng khớp; TAP page-fault
**thật** `scause=13/15`; 50 boot product; gate 569; budget ≤ 262.144 B.

**Đỏ trước** ([`kt46-red-ktest.log`](JOURNAL/kt46-red-ktest.log)): `ktest.sh` đòi `1..8` + `ok 5 - mmu
on` + `ok 6 - map layout` + `ok 7 - walk khop` + bằng chứng `# mmu:`; ảnh KT-45 chỉ có `1..5`, không
có MMU ⇒ `not ok 1 - ktest boot 1: thiếu plan-1..8 ok5-mmu-on ok6-map-layout ok7-walk-khop
ok8-nested mmu-evidence`, `EXIT=1` — đỏ đúng lý do.

**Kết quả**:
- **Logic MMU** (`vifast-plat-common::mmu`, mới, `forbid(unsafe_code)`): bit PTE Sv39
  (`V/R/W/X/U/G/A/D`, `A|D` đặt sẵn), `PteFlags`, `MapError`, `Walk`, `PteMemory`, `is_canonical`,
  `walk_with`, `PageTableBuilder` cấp bảng từ `&mut [u8]` (arena), map **4 KiB/2 MiB/1 GiB**,
  `satp_sv39`/`satp_mode`/`satp_root`; dùng lại `vifast_pte::{walk_shape, Pte, VA_BITS=39,
  PTE_LEVELS=3, PAGE_SIZE_BITS}` (path nội bộ, không crate ngoài). `trap.rs` thêm `GuardStats`/
  `GUARD_STATS` + `report_guard_fault` (`PANIC: guard page fault scause=… stval=… sepc=…` + regs +
  halt), `dump_regs` dùng chung với `report_fatal`.
- **Arch** (`vifast-plat-riscv64::mmu`, mới; `unsafe` thuộc danh sách đóng D2-17): arena **64 KiB**
  từ `__kernel_end` (không `static`); identity image **leaf 2 MiB** + nửa cao **leaf 2 MiB**
  `0xFFFF_FFC0_4000_0000`; NOLOAD `0x8040_0000..arena_end` 4 KiB (guard **NONE**, stack/trap/arena
  RW NX); physmap `0xFFFF_FFC0_0000_0000 + PA` **2 MiB NX** (chunk chứa guard tách 4 KiB + khoét
  guard); UART/PLIC **4 KiB Device**; `csrw satp` + `sfence.vma` **một asm block**, đọc lại `satp`,
  mẫu PTE trước/sau + alias identity ↔ nửa cao ↔ physmap. `trap_dispatch` đi đường guard riêng;
  probe `probe_guard_load` (resume test-only) / `probe_guard_store` (fatal), `read_u64`/`write_u64`.
- **Linker** `ALIGN(2M)` trước `.stack_guard`: guard ở `0x8040_0000`; image 2 MiB không phủ guard;
  NOLOAD nên ảnh không đổi file, budget báo riêng.
- **Ảnh**: product `vifast-kbin-riscv64` in `# mmu: satp=… mode=8 …` + vẫn `ok 1 - k3 boot`; ảnh
  chính **8 TAP** (4 trap KT-45 + `mmu on`/`map layout`/`walk khop` + trap lồng fatal); ảnh guard
  **3 TAP** (guard image + load `scause=0xd` recover + store `scause=0xf` fatal).
- **Bằng chứng**: [`kt46-ktest.log`](JOURNAL/kt46-ktest.log),
  [`kt46-ktest-main-raw.log`](JOURNAL/kt46-ktest-main-raw.log),
  [`kt46-guard-ktest-raw.log`](JOURNAL/kt46-guard-ktest-raw.log),
  [`kt46-host-tests.log`](JOURNAL/kt46-host-tests.log),
  [`kt46-budget.log`](JOURNAL/kt46-budget.log),
  [`kt46-boot50.log`](JOURNAL/kt46-boot50.log),
  [`kt46-boot.log`](JOURNAL/kt46-boot.log),
  [`kt46-gate-fast.log`](JOURNAL/kt46-gate-fast.log).
- **Số đo**: host **21/21**; TAP **8/8 + 3/3**; product **50/50**; budget `.data+.bss` **168/168/176
  B**/262.144 (`.stack=65.536`, `.stack_guard=4.096`, `.trap_stack=16.384` báo riêng); gate
  **569/73 PASS**; `satp=0x8000000000080415 mode=8`, root/arena `0x80415000`, guard
  `0x80400000..0x80401000` (`guard_none=1` cả 3 alias), `kernel_high_2m=1`, `physmap_2m_nx=1`,
  `uart_dev=1`, `plic_dev=1`, `walk khop pre=post=1 software=8/8` — số do script/cargo in, không
  chép tay vào docs.

**Review độc lập**: `reviewer-kt46` **APPROVE** (tự chạy host 21/21, ktest 8+3 TAP,
`satp=0x8000000000080415 mode=8 root=0x80415000 tables=10`, product 50/50, gate 569; grep không còn
vùng dùng chưa map; disasm guard fault đúng `ld`/`sd`). 4 nit đã vá trước commit: `ktest.sh` nhận
`len=2|4`; sửa câu ELF `filesz`; probe/chuỗi guard sau `#[cfg(feature="ktest")]` (product 0 chuỗi
test); comment `lib.rs` + HEAD. Chi tiết
[`kt46-journal.md`](JOURNAL/kt46-journal.md) §6.

**Không làm (đúng phạm vi)**: timer (KT-47), PLIC IRQ/mapping rộng (KT-48), SMP/PerHart (KT-49),
IPI (KT-50), ASID/shootdown (KT-51), object layer (KT-52), U-mode (KT-53), harness/matrix (KT-54),
conformance (KT-55), `FaultKind`/W^X/spec (KT-56), soak/K3-DOD (KT-57), port core (K3b); `spec/`,
sim, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/` **không chạm**.

## KT-45 — trap khung riscv64 (`stvec` direct, trapframe, decode, panic dump)

**Vào**: READY; HEAD `faba5cb8` ([KT-00] D2-73 — Agent 0 điều chỉnh DoD sau báo cáo chặn của
Agent 2); `HEAD..origin/main = 0`; working tree có `viscript/` + `review/` của tab khác — **không
chạm**; baseline `gate --fast` **569/73 PASS** (`JOURNAL/kt45-baseline-gate-fast.log`).

**Mục tiêu** (K3-PLAN §4.1 KT-45 + **D2-73**): `stvec` direct-mode một vector; trapframe 32 thanh
ghi + `sepc/sstatus/scause/stval`; `sp` đổi sang **trap stack** qua `sscratch`; decode `scause`
(mọi mã 0/1/2/3/4/5/6/7/8/9/12/13/15/Unknown) ⇒ `Event::Exception`; panic dump
`scause/sepc/stval` + thanh ghi chính, `DebugHalt` sạch; `Platform` + `set_trap_vector` +
`enable_irq`/`disable_irq` (stub, KT-48); TAP trap + 50 boot; gate 569; docs/JOURNAL/STATE.

**Đỏ trước** ([`kt45-red-ktest.log`](JOURNAL/kt45-red-ktest.log)): `ktest.sh` siết đòi `1..4` +
`ok 2 - trap breakpoint` + `ok 3 - trap ecall sbi` + `ok 4 - trap illegal inject`; ảnh KT-44 chỉ
có `1..1`/`ok 1 - ktest image` ⇒ `not ok 1 - ktest boot 1: thiếu plan-1..4 …`, `EXIT=1` — đỏ đúng
lý do, không phải hạ tầng.

**Phát hiện chặn DoD (D2-73)**: OpenSBI v1.8.1 `delegate_traps()` đặt `MEDELEG = 0xf4b509`
(chỉ cause 0/3/8 + page fault 12/13/15) ⇒ **bit 2 illegal = 0, bit 9 ecall-S = 0**. Probe QEMU
11.1.3 (D2-72 Q2/Q3): `ebreak` → `scause=3`; `.4byte` illegal **không** vào `stvec` (ở lại M-mode);
`ecall` S-mode trả `SBI_ERR_NOT_SUPPORTED` và resume `sepc+4` (M-mode, không qua `stvec`) —
[`kt45-probe-medeleg.log`](JOURNAL/kt45-probe-medeleg.log),
[`kt45-probe-illegal-mmode.log`](JOURNAL/kt45-probe-illegal-mmode.log). Agent 0 ghi **D2-73**:
giữ đủ khung + decode mọi `scause` (host test), TAP thật chỉ `breakpoint` + `ecall-S` (SBI, không
claim trap S-mode), `illegal` = host test + **trap tiêm** (ghi rõ "inject"), page fault thật →
**KT-46**, cấm đổi firmware M-mode. Deviation ghi ở `k3-riscv64.md` §7 + JOURNAL + đây.

**Kết quả**:
- `vifast-plat-common::trap` (mới, `forbid(unsafe_code)`): `TrapFrame` `#[repr(C)]` **288 B**
  (32×8 + 4×8) + offset hằng số + `const` assert `offset_of!`; `ExceptionKind`/`Exception`/`Event`;
  `decode_scause`/`decode_trap`; `handle_trap` (resume 2/4 byte, fatal); `TrapStats`/`TRAP_STATS`;
  `report_fatal`/`report_interrupt` (PANIC + `scause/sepc/stval/sstatus` + `ra/sp/gp/tp` + `a0..a7`);
  `is_nested_trap_sp` (predicate trap lồng, host test biên).
- `vifast-plat-riscv64`: `_start` zero `.bss` rồi đặt `sscratch = __trap_stack_top`;
  `trap_entry` direct-mode: `csrrw` + **`addi sp, sp, -288`** (frame nằm trọn trong `.trap_stack`
  — P1), lưu/khôi phục 32 thanh ghi + 4 CSR (offset là `const` operand từ `plat-common`), **không**
  ghi `sstatus` khi ra (sret tự khôi phục — P2a), phát hiện **trap lồng** từ `sp` bị ngắt trong
  frame ⇒ `nested_trap_halt` PANIC + halt (P2b); `trap_dispatch`; `trap_vector`/
  `install_trap_vector`/`read_trap_vector`; `Platform` impl `set_trap_vector` +
  `enable_irq`/`disable_irq` (luôn `false` — PLIC KT-48); probe `probe_breakpoint`/
  `probe_ecall_sbi` (SBI clobber a0/a1 — P3)/`probe_injected_trap`/`probe_nested_trap` (API an
  toàn để kbin **không** thêm `unsafe` — R5/D2-17).
- 2 ảnh: product cài `stvec`, in `stvec=… direct=true`, TAP giữ `ok 1 - k3 boot`;
  test **5 TAP** — `ok 2 - trap breakpoint` (scause 3, `last_sepc+4` = địa chỉ resume,
  `frame=0x80218ee0` trong `.trap_stack`), `ok 3 - trap ecall sbi` (`a0=SBI_ERR_NOT_SUPPORTED`,
  resume `sepc+4`, `stvec-traps+0`), `ok 4 - trap illegal inject` (scause 2, **tiêm**),
  `ok 5 - trap nested panic` (trap lồng ⇒ `PANIC: nested trap …` + halt, có chủ đích; runner chỉ
  chấp nhận đúng dòng PANIC này).
- Ảnh sản phẩm [`kt45-boot.log`](JOURNAL/kt45-boot.log) `ok 1 - boot 1`, strings `ktest=0`;
  **50 boot** [`kt45-boot50.log`](JOURNAL/kt45-boot50.log) `# 50/50 boot sạch (smp=1)`.
  Ảnh test [`kt45-ktest.log`](JOURNAL/kt45-ktest.log) PASS.
- Budget [`kt45-budget.log`](JOURNAL/kt45-budget.log): `data+bss=80 B`/262.144 **cả hai** ảnh;
  `.stack=65536`, `.stack_guard=4096`, **`.trap_stack=16384`** báo riêng (NOLOAD).
- Fatal path trên QEMU (tạm tiêm `scause=13`, không commit;
  [`kt45-fatal-trap.log`](JOURNAL/kt45-fatal-trap.log)): `PANIC: trap LoadPageFault scause=0xd …`
  + `# regs …`, rồi halt — dump + `DebugHalt` sạch, không treo.
- Trap lồng [`kt45-nested-trap.log`](JOURNAL/kt45-nested-trap.log): `PANIC: nested trap …` +
  halt, không treo/không resume (P2b sau review).
- **Cổng**: `gate --fast` **PASS 569 test/73 suite, 0 failed**
  ([`kt45-gate-fast.log`](JOURNAL/kt45-gate-fast.log)); `git diff spec/` **rỗng**.

**Review độc lập**: `reviewer-kt45` verdict **DRIFT** (không UNSAFE) — P1 (frame ngoài
`.trap_stack`), P2a (`csrw sstatus` sớm), P2b (trap lồng), P3 (SBI clobber a1), P4 (zero `.bss`):
**đã vá hết** trước commit; `reviewer-kt45b` **CONFIRM** (tự verify QEMU `-d in_asm`, `frame`
trong trap stack, 13/13 host, 5 TAP + đúng 1 PANIC, 50/50, gate 569, spec rỗng, unsafe chỉ
`plat-riscv64`); 4 nit còn lại (comment bố cục frame lồng, serial log, "5 TAP", grep sstatus)
đã vá trong `[KT-45b]`.

**Không làm (đúng phạm vi)**: MMU/`satp`/guard page (KT-46), timer (KT-47), PLIC (KT-48), SMP/
PerHart trap stack (KT-49), IPI/TLB (KT-50/51), object layer (KT-52), U-mode/syscall (KT-53);
nested-trap guard (trap lồng ghi đè trap stack) để KT-46/49; `spec/`, sim, `.scn`, QUEUE/
DECISIONS/AGENTS, `viscript/` **không chạm**; `unsafe` chỉ thêm trong `vifast-plat-riscv64`
(D2-17), không `cfg(target_arch)`.

## KT-44 — task đầu K3: `plat-common` + tách ảnh sản phẩm/ktest + đo budget

**Vào**: READY (`agentctl next --project v2` trả KT-44); HEAD `4f7b65e2` ([KT-00] duyệt K3-PLAN,
D2-72); `HEAD..origin/main = 0`; working tree có `viscript/` của tab khác — **không chạm**;
baseline `gate --fast` **569 test/73 suite PASS** (`JOURNAL/kt44-baseline-gate-fast.log`).

**Mục tiêu** (K3-PLAN §4.1 KT-44 + D2-72 Q2/Q3/Q5/Q7/Q12 + D2-08): crate
`v2/kernel/vifast-plat-common` (`no_std`, `#![forbid(unsafe_code)]`) với trait `Platform` tối thiểu
(`put_str`/`halt`/`now`/`tick_hz`/`hart_id`) + `PanicHandler` dùng chung; **tách 2 ảnh** sản phẩm và
`ktest` từ cùng crate bằng **feature** (không `cfg(target_arch)`); linker `.stack` `NOLOAD` +
trang bảo vệ; cổng budget `llvm-size -A` `.data+.sdata+.bss+.sbss ≤ 262.144 B` cho **cả hai** ảnh;
`v2/docs/k3-riscv64.md` (hằng số D2-72, quy trình build/boot, "K3 mới là kernel thật").

**Đỏ trước** ([`kt44-red-ktest.log`](JOURNAL/kt44-red-ktest.log)): `ktest.sh` đòi `ok 1 - ktest image`;
chưa có ảnh test ⇒ `error: the package 'vifast-kbin-riscv64' does not contain this feature: ktest`
+ `build ảnh test FAIL — CHƯA có ảnh test`, `EXIT=1`. Đỏ **đúng lý do** (thiếu ảnh test, không phải
lỗi hạ tầng).

**Kết quả**:
- **2 ảnh từ cùng crate** ([`kt44-build.log`](JOURNAL/kt44-build.log)): `vifast-kbin-riscv64`
  (sản phẩm, 19.216 B) và `vifast-kbin-ktest` (feature `ktest`, `required-features`, 19.080 B);
  `strings`: ảnh sản phẩm **0** chuỗi `ktest`, ảnh test **8** ⇒ tách ảnh thật (D2-08).
- **TAP 1 boot xanh cả hai ảnh**: [`kt44-boot.log`](JOURNAL/kt44-boot.log) `ok 1 - boot 1`
  (`ok 1 - k3 boot`, `# hart=0 tick_hz=10000000`); [`kt44-ktest-boot.log`](JOURNAL/kt44-ktest-boot.log)
  `ok 1 - ktest image`; QEMU **11.1.3** (≥ sàn 9.x), OpenSBI **v1.8.1**, `-machine
  virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1`, timeout 20 s, chỉ `kill <pid>`.
- **Budget** ([`kt44-budget.log`](JOURNAL/kt44-budget.log)): `data+bss = 0 B` cho **cả hai** ảnh
  (trần 262.144 B), `.stack` **65.536 B** + `.stack_guard` **4.096 B** báo riêng (NOLOAD);
  `run.sh`/`ktest.sh` **fail** nếu vượt. `nm`: guard `0x80202000..0x80203000`, `__stack_bottom`
  `0x80203000`, `__stack_top` `0x80213000` — guard nằm **ngay dưới** đáy stack.
  Số **0 B là thật, không phải đọc sai** — chứng minh ở [`kt44-budget-probe.log`](JOURNAL/kt44-budget-probe.log):
  `llvm-objdump -h` của ảnh sản phẩm chỉ có `.text .rodata .bss(0 B) .stack_guard .stack` (**không**
  có `.data/.sdata/.sbss`); `budget.sh` in `# sections-seen` + `# data+bss-sections`; khớp tên theo
  **tiền tố** nên `.data.rel.ro`/`.sbss.*` không bị bỏ sót; `--selftest` 3 fixture; **đối chứng
  dương** probe A (bản sao workspace ở `/tmp/kt44-probe`, cùng linker script) thêm `.data` 16 B +
  `.bss` 1.024 B ⇒ `data+bss=1040`, exit 0; **đối chứng âm** probe B `.bss` 307.200 B ⇒
  `data+bss=307216`, exit 1 (`budget: VƯỢT`).
- **Host test logic dùng chung** (D2-72 Q12): `cargo test -p vifast-plat-common` **3/3**
  (`platform_writer_in_dung_thu_tu`, `panic_handler_in_vi_tri_va_halt_1`,
  `panic_handler_khong_vi_tri_van_halt_1`).
- **Cổng**: `./v2/bin/v2 gate --fast` **PASS 569 test** ([`kt44-gate-fast.log`](JOURNAL/kt44-gate-fast.log))
  — kernel không nằm trong cổng này (`exclude = ["kernel"]`), sim không đỏ.

**Review độc lập**: `reviewer-kt44` **APPROVE** (phạm vi + DoD) — tự chạy lại trên cây hiện tại:
`gate --fast` 569, `run.sh 1 1` PASS, `ktest.sh 1 1` PASS, budget `data+bss=0 / stack=65536 /
guard=4096`, strings **0/8**, `budget.sh --selftest` 3/3; xác nhận **0 B không che lỗi** (`.rodata`
404 B nằm trong PT_LOAD, `.stack_guard/.stack` là `SHT_NOBITS`, chỉ 2 static zero-sized) và
`__stack_guard_end = __stack_bottom = 0x80203000 < __stack_top = 0x80213000`. 2 nit **không chặn**
đã vá trong follow-up **`[KT-44b]`** (lead yêu cầu): (a) sàn QEMU so **major.minor ≥ 9.2**
(trước chỉ so major) + đòi chuỗi version có dấu chấm; (b) `k3-riscv64.md` §1 ghi đúng phạm vi
từng task (KT-46..53) thay vì "KT-45..49"; (c) STATE `HEAD`/việc dở. Bằng chứng cổng phiên bản:
[`kt44-version-gate.log`](JOURNAL/kt44-version-gate.log) (9.1.0/9.1/8.2.0/9 ⇒ FAIL; 9.2/9.2.0/
9.10.0/11.1.3 ⇒ PASS). Ghi chú `kt44-budget.log` cũ hơn `budget.sh` là do lần đọc cũ: bản đã commit có mtime
22:35:11 > `budget.sh` 22:33:49 và chứa `# sections-seen` + selftest.

**Không làm (đúng phạm vi)**: trap/`stvec` (KT-45), MMU/guard fault (KT-46), timer tick (KT-47),
PLIC (KT-48), SMP/HSM (KT-49), IPI/TLB thật (KT-50/51), core ABI (Q13 — K3 **không phải** ABI đầy
đủ); `spec/`, sim semantics, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/` **không chạm**; không thêm
`unsafe` ngoài danh sách đóng D2-17, không `cfg(target_arch)`.

**YC/Follow-up**: (1) KT-45 dựng `stvec`/trapframe trên khung 2 ảnh này; (2) KT-46 dùng
`__stack_guard_*` để bắt guard fault; (3) KT-47 thay stub `now` bằng SBI TIME 100 Hz; (4) KT-54 thay
`ok 1 - ktest image` bằng harness TAP đầy đủ + `expected.txt`; (5) Agent 0 lật QUEUE KT-44 DONE khi
verdict reviewer đạt (D2-37).

## KT-43 — cổng cuối K2: 100k×4 L1 (driver phủ VM) + 17 mutation + Miri + DoD K2

**Vào**: READY; KT-35..42 DONE; HEAD `66d51f89`; `git rev-list --count HEAD..origin/main` = **0**;
`v2/` sạch (chỉ `viscript/`+`review/` của tab khác — không chạm); baseline = KT-42 **569/73**, L1
2.635 ms.

**Mục tiêu** (K2-PLAN §4 + D2-65 Q10): `nightly --l1` **100k seed × smp 1/2/4/8** (400.000 lượt)
với driver L1 **phủ thêm VM** (map/unmap/walk/shootdown/fault; giữ 64 op/seed, cùng seed cùng
trace); **mutation 12 ca §7.2 + 4+ ca VM** (gồm AP→BSP); **Miri** toàn bộ tập K2 + `vifast-pte` +
`vifast-sync`; `gate --fast` ≥ 569 + `check_scn` 26/16 + fixtures âm; **K2-DOD** với số đo thật và
nợ có chủ; câu "K2 không phải kernel thật".

**Kết quả chính**:
- **Driver L1 phủ VM** (`tests/l1_k1c.rs`, test-only): thêm op 19 `map` (Frame 1 GiB qua VSpace
  active mọi hart), 20 `walk+TLB fill`, 21 `unmap+shootdown` (assert `Ok ⇒ acked == targets`,
  mọi hart sạch entry), 22 `vm_touch` fault `NotMapped` → `fault_ep` → `wake`; sau mỗi op có
  `tlb_violations()==0`, cuối mỗi lượt `check_vm()` (8 checker). **Đỏ trước**:
  [`kt43-red-driver-vm.log`](JOURNAL/kt43-red-driver-vm.log) (bỏ 4 nhánh VM ⇒ panic đúng bất biến
  phủ VM).
- **100k×4 PASS**: [`kt43-l1-100k.log`](JOURNAL/kt43-l1-100k.log) — 400.000 lượt, **2 lần/seed**
  (400.000 cặp trace khớp), **0 panic/0 vi phạm TLB-safety** trên **51,2 triệu** op, wall
  **690,69 s**; counters: `ipc_sent` **4.639.586**, `ipc_delivered` 865.478, `ipc_dropped`
  2.918.141; VM map **1.028.942**, walk 286.584, unmap/shootdown 540.715, fault 924.833. Bản chạy
  100k **đầu** dừng ở guard per-seed KT-33 (`seed 67453/smp 2 ipc_sent=1`) — đã chỉnh **ngưỡng
  phủ của driver** (per-seed giữ `sent ≥ 1` + bảo toàn; ngưỡng "không tautology" xuống tổng smp
  `sent > 5×seeds`), **không** nới bất biến model/seed/op; bản hỏng giữ ở
  [`kt43-l1-100k-attempt1-fail.log`](JOURNAL/kt43-l1-100k-attempt1-fail.log).
- **Mutation 17/17** ([`kt43-mutation.log`](JOURNAL/kt43-mutation.log)): 12 ca §7.2 cũ + **V1**
  đảo cấp lá AP→BSP, **V2** bỏ chờ ack khi unmap, **V3** bỏ W^X khi map W, **V4** bỏ flush khi
  deactivate, **V5** bỏ drain hộp thư khi chờ ack (AP→BSP v1). Mỗi ca `exit 101` + `FAILED` đúng
  test đích + khôi phục hash == HEAD. Harness lần đầu 13/17 (flush trễ + M12 cũ sống vì KT-34
  thêm guard `transition_tcb`) — đã sửa harness + định nghĩa lại M12 theo guard hiện hành, chạy
  lại **toàn bộ**; log đầu giữ ở `kt43-mutation-attempt1.log` (không dùng).
- **Miri 146 test/0 failed**: 7 tệp K2 (57) + lib 64 + pte 9 + sync 16 + driver mẫu nhỏ; thêm
  `kt36..kt39` + `vifast-pte` vào danh sách `nightly --only-miri`.
- **Gate PASS 569/0**, L1 **1.427 ms** ([`kt43-gate-fast.log`](JOURNAL/kt43-gate-fast.log)); wall
  505,76 s (wall host — T-INFRA); `.scn` 26 tệp/16 nhóm + 5/5 fixture âm; `git diff spec/` rỗng.
- **K2-DOD** [`reports/K2-DOD.md`](reports/K2-DOD.md): D1..D11 ĐẠT, nợ có chủ (model-only 1 luồng,
  TLB/PTE/IPI thật = K3, `Busy`/`FaultKind` model-only, pager/COW = K3), câu "K2 **không** phải
  kernel thật".

**Review độc lập**: `reviewer-kt43` **DRIFT** (4 mục) → đã vá: **D2-70** (lead) + `ipc_min`/
`ipc_median` + log 10k; K2-DOD final (sửa Miri driver/kt41-42/AP→BSP/diff); lưu harness +
`kt43-mutation-diff.md`; D8 honest (driver smoke không hoàn tất, không claim). Chạy lại
`gate --fast` **569/0**, L1 **1.427 ms**. Đề xuất: reviewer-kt43 xác nhận ⇒ **K2 DONE (đề xuất)**;
Agent 0 lật QUEUE. Không tự lật (D2-37).



## KT-42 — K2: 5 `.scn` VM + một cửa `check_vm` + shared read-only end-to-end

**Vào**: READY (`agentctl next --project v2` trả KT-42) · HEAD `6a9e3ccf` ([KT-00] lật KT-41
DONE, 562 test) · `git rev-list --count HEAD..origin/main` = **0** · working tree có `viscript/`
+ `review/` của tab khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 562
test/71 suite**, L1 **885 ms** ([`JOURNAL/kt42-baseline-gate-fast.log`](JOURNAL/kt42-baseline-gate-fast.log)).

**Mục tiêu** (K2-PLAN §3 KT-42 + §2 Q7/Q10; D2-65, D2-67, D2-68): 5 `.scn` VM (map/unmap
4K+2M+1G + walk; W^X `WxViolation` + bộ đếm nguyên; shootdown 2 hart ack đủ mới `Ok`; VM fault
`NotMapped` + resume; shared read-only 2 VSpace); **một cửa** `check_vm` ở cổng (L0) gộp 8
checker — **không** thay hook per-op; self-test `check_vm` không rỗng (2 lỗi cố ý); docs §16;
Miri; **không** sửa `spec/`/test cũ/21 `.scn` cũ, không unsafe/dependency, không nới ngưỡng.

**Kết quả**:
- **Đỏ trước** (D2-05): [`kt42-red-scn-check.log`](JOURNAL/kt42-red-scn-check.log) (21 vs 26
  tệp, tag/op `vm_*` lạ) + [`kt42-red-scn-runner.log`](JOURNAL/kt42-red-scn-runner.log) và
  `kt42-red-scn-{22..26}.log` (5/5 ca "runner chưa cài op", panic đúng op đầu; kịch bản 24 đỏ
  ở `vm_activate`).
- **5 `.scn` mới** `v2/test/scn/{22-frame-map-unmap-walk,23-wx-violation-counters,
  24-shootdown-2-hart,25-vm-fault-notmapped-resume,26-shared-readonly-frame}.scn` — tag
  `vm-map/vm-wx/vm-tlb/vm-fault/vm-shared`; số 22..26 = số trống tiếp theo, **không** đụng 21
  tệp cũ. `check_scn.py`: 21→26 tệp, 11→16 nhóm, bảng đóng `VM_OPS` cho op model-only (op ABI
  vẫn theo `spec/method_signature`; fixtures âm vẫn bắt đúng thông điệp).
- **Runner** `tests/kt42_scn.rs` (**5 test**): đọc `.scn` (không sửa), rig `harts`/caps (3 cỡ
  Frame, 2 VSpace, `t0` fault_ep/`t1` procsrv), **21** op `vm_*`, so từng bước.
- **`Machine::check_vm`** (`src/kobj.rs`): gộp `check_frames`+`check_pages`+`check_wx`+
  `wx_recount`+`check_vspace`+`check_asid`+`check_tlb`+`check_fault`, tiền tố lỗi theo checker;
  **per-op hook không đổi**; chi phí `O(F log F)` + 2 lần đi cây + `O(entries×walk)` … ghi ở
  §16.2. Unit test dương tính tiêm **2 lỗi** (xoá PTE ngầm; entry TLB mồ côi) — bắt cả hai,
  khôi phục thì sạch.
- **`tests/kt42_vmcheck.rs`** (L0/cổng): đường vàng đầy đủ (3 cỡ Frame, shared read-only 2
  VSpace, TLB 2 hart, shootdown) + `wx_recount` khớp; `vm_check` ở cuối cả 5 kịch bản.
- **Docs** `v2/docs/k2-vm.md` **§16** (kịch bản/nhóm, quy ước runner, `check_vm` + chi phí,
  self-test, shared read-only, log đỏ, cổng/Miri, ranh giới).
- **Cổng**: [`kt42-gate-fast.log`](JOURNAL/kt42-gate-fast.log) **PASS 569/73**, L1 2.635 ms;
  baseline 562/71. `git diff spec/` **rỗng**; không `#[ignore]`/unsafe/dep; 21 `.scn` cũ nguyên.
- **Miri**: `kt42_vmcheck` 1/1 (32,12 s) + `--lib check_vm_` 1/1 (3,37 s)
  ([`kt42-miri.log`](JOURNAL/kt42-miri.log), MIRI1/2_EXIT=0).

**Reviewer `reviewer-kt42`: APPROVE** (tự chạy 5/5 + 1/1, gate 569/73 L1 1.499 ms, scn-check 26/16, fixtures 5/5, spec rỗng; `check_vm` đủ 8 checker, per-op nguyên vẹn +117/−0, self-test 2 lỗi thật; 5 `.scn` 23/27/22/17/18 bước, runner 21 nhánh thật). Đã sửa 3 ghi chú P3 trước commit (đếm 21 op; verdict; lệch chữ per-op dùng bộ checker cũ không `wx_recount` — `check_vm` đầy đủ ở L0/`.scn`/cổng, tương đương phủ, **không nới**); mục số `.scn` 22..26 do Agent 0 chốt **D2-69** (thay G5 "≥30").

**Đề xuất: REVIEW** (chờ Agent 0 xác nhận; không tự DONE, không lật QUEUE — D2-37).
**YC/Follow-up**: (1) KT-43 cổng cuối K2 — 100k×4 + mutation (4 còn lại) + soak + DoD; (2) K3
mới có TLB thật/pager/COW + đưa `FaultKind::Vm`/VM method vào `spec/`; (3) giữ 5 `.scn` VM là
hồi quy cho mọi thay đổi K2 VM sau này.


## KT-41 — K2: VM fault + `FaultKind::Vm` (model-only), `vm_touch`, `check_fault`

**Vào**: READY (`agentctl next --project v2` trả KT-41) · HEAD `f3872df0` ([KT-00] lật KT-40 DONE,
552 test) · `git rev-list --count HEAD..origin/main` = **0** · working tree có `viscript/` +
`review/` của tab khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 552
test/70 suite**, L1 **774 ms** ([`JOURNAL/kt41-baseline-gate-fast.log`](JOURNAL/kt41-baseline-gate-fast.log)).

**Mục tiêu** (K2-PLAN §3 KT-41 + §2 Q6, D2-65 Q6 + D2-56(a)/(c)): `FaultKind::Vm {vspace, va,
access, reason}` **model-only** (không sửa `spec/`); `vm_touch` miss ⇒ `NotMapped`, thiếu quyền
⇒ `Permission`, exec trang W/ghi trang X ⇒ `Wx`; giao fault qua **đường fault K1c** tới
`fault_ep`; procsrv model **chỉ nhận + ghi log**, không cấp trang (không pager ở K2); không
`fault_ep` ⇒ K1c `InvalidArgument`; fault không mất (C2); property 10k; `check_fault`; Miri.

**Không** làm: pager/memory server/COW (K3); `.scn`/`check_vm`/shared read-only (KT-42);
100k×4/mutation/soak (KT-43); `spec/`, QUEUE/DECISIONS/AGENTS/`viscript/`; không unsafe/
dependency; không sửa test cũ ngoài cập nhật cơ học theo D2-65 Q6.

**Kết quả**:
- **Test đỏ trước** (D2-05): [`kt41-red-compile.log`](JOURNAL/kt41-red-compile.log) — **20 lỗi**
  (E0599 ×17 `vm_touch/check_fault`, E0559 ×4 `FaultKind::Vm` bốn trường, E0432 ×3
  `VmFaultReason`), exit 101.
- **Code**: `src/tcb.rs` (`VmFaultReason`, `FaultKind::Vm` bốn trường, `name`/`render`);
  `src/kobj.rs` (`vm_touch` + `vm_touch_reason`, `check_fault` trong `sched_after_op`, field
  `fault_events`, unit test dương tính); `src/procsrv_model.rs` (`fault_log`, doc "chỉ nhận +
  ghi log"); `tests/kt41_vmfault.rs` **(mới, 9 test)**; `tests/kt26_fault.rs` **7 điểm dựng + 1
  assertion `render`** cập nhật cơ học theo D2-65 Q6 (không nới); `v2/bin/v2` thêm `kt41_vmfault`
  vào `nightly --only-miri`.
- **Docs**: `v2/docs/k2-vm.md` **§15** (kiểu dữ liệu, `vm_touch`, bảng `Permission`/`Wx` gồm
  `W+NX`, `va` không căn, `check_fault`, C2, 9 test/property, cập nhật test cũ, ranh giới).
- **Cổng**: [`kt41-check.log`](JOURNAL/kt41-check.log) PASS; [`kt41-gate-fast.log`](JOURNAL/kt41-gate-fast.log)
  **PASS 562 test/71 suite**, L1 1.428 ms (đợt 2 47.758 ms — host load `T-INFRA`); `kt41_vmfault`
  9/9; lib 63/63; `git diff spec/` **rỗng**.
- **Mutation tự kiểm** (nit reviewer): làm tròn `va` trong payload ⇒ test 1 **đỏ** đúng assertion
  ([`kt41-red-va-khong-can.log`](JOURNAL/kt41-red-va-khong-can.log)), khôi phục `cp` + hash khớp,
  9/9 xanh.
- **Miri**: `--test kt41_vmfault` **9/9** (property 40 vòng) + `--lib check_fault` **1/1**
  ([`kt41-miri.log`](JOURNAL/kt41-miri.log), MIRI1/2_EXIT=0).
- **Review độc lập**: `reviewer-kt41` **APPROVE** (tự chạy gate 562/71, kt41 9/9, kt26 24/24,
  spec rỗng; 3 reason không trộn; đúng `inject_fault` K1c; không `fault_ep` ⇒ `InvalidArgument`;
  C2 checker hai chiều; kt26 cơ học không nới; procsrv không thành pager); 4 nit đã sửa trước
  commit (đếm 7+1, suite 562/71, `va` không căn + test/mutation, hàng W+NX + assert). Nhật ký:
  [`JOURNAL/kt41-journal.md`](JOURNAL/kt41-journal.md).

**Đề xuất: REVIEW** (chờ Agent 0 xác nhận; không tự DONE, không lật QUEUE — D2-37).
**YC/Follow-up**: (1) KT-42 gộp `check_fault` vào `check_vm` + 4 `.scn` (fault→procsrv) + shared
read-only; (2) KT-43 100k×4 + mutation K2 + soak; (3) K3 mới đưa `FaultKind` vào `spec/` + pager
thật (D2-56(a), Q6).

## KT-40 — K2: TLB 16 entry/hart + shootdown `TlbFlush` + ack thế hệ + chống deadlock

**Vào**: READY (`agentctl next --project v2` trả KT-40) · HEAD `4ae932e` ([KT-00] lật KT-39 DONE,
536 test) · `git rev-list --count HEAD..origin/main` = **0** · working tree có `viscript/` +
`review/` của tab khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 536
test/68 suite**, L1 **800 ms** ([`JOURNAL/kt40-baseline-gate-fast.log`](JOURNAL/kt40-baseline-gate-fast.log)).

**Mục tiêu** (K2-PLAN §3 KT-40 + §2 Q4/Q5, D2-65 Q4/Q5 + D2-60): TLB **16 entry/hart** khoá
`(gen, asid, va)`, LRU tất định, hit/miss đếm được, `gen` vô hiệu toàn bộ; **một** kind
`TlbFlush{asid, va, gen, seq}` (nới D2-59, `VmKick`/`PowerReq` vẫn K3); unmap/protect sửa PTE ⇒
flush cục bộ + gửi tới `active_on` + **chờ đủ ack** (`acked_mask == active_on`), ack `fetch_max`
(`acked ≠ delivered`); hart chờ ack **phải rút hộp thư của mình** (AP→BSP); oracle TLB-safety;
property 10k; Miri; mutation phải bị bắt.

**Không** làm: VM fault (KT-41), `.scn`/`check_vm` (KT-42), 100k×4 + 4 mutation còn lại (KT-43),
`spec/`, `viscript/`, QUEUE/DECISIONS/AGENTS; không unsafe/dependency/`#[ignore]`; không sửa
test cũ.

**Kết quả**:
- **Test đỏ trước** (D2-05): [`kt40-red-compile.log`](JOURNAL/kt40-red-compile.log) — **E0432**
  (`TlbFlushScope/TLB_ENTRIES_PER_HART`) + **E0599** (`tlb_translate/lookup/info/…`,
  `tlb_shootdown_*`, `check_tlb`, `tlb_violations`), `could not compile` exit 101.
- **Code**: `src/tlb.rs` **(mới)** (16 entry/hart, LRU, 3 scope flush, ack `fetch_max` bằng
  `u64` thường — không unsafe/atomic, 5 unit test) + `src/mailbox.rs` (**đúng một** kind
  `TlbFlush`) + `src/kobj.rs` (9 field, 16 API, `push_mail` gán ack, `apply_mail(TlbFlush)`,
  hook `vspace_unmap/protect` (prepare→PTE→post→wait), flush cục bộ khi đổi/rời VSpace/offline,
  teardown Asid + cạn ASID Generation broadcast, `check_tlb` trong `sched_after_op`,
  `check_mailboxes` thêm `delivered-acked == số TlbFlush trong hàng`, 1 unit test dương tính
  oracle) + `tests/kt40_tlb.rs` **(mới, 10 test)** + `v2/bin/v2` (kt40_tlb vào `nightly --only-miri`).
- **Docs**: `v2/docs/k2-vm.md` **§14** (hình TLB, 3 scope, shootdown/ack, **deviation D2-68**
  không IPI hart gọi + ack ngầm, chống deadlock + `poll`, oracle 4 nhóm, 10 test, **5 mutation**,
  ranh giới).
- **Cổng**: [`kt40-check.log`](JOURNAL/kt40-check.log) PASS; [`kt40-gate-fast.log`](JOURNAL/kt40-gate-fast.log)
  **PASS 552 test/70 suite**, L1 **793 ms**; `kt40_tlb` **10/10**, lib **62/62**, L0 552/0.
- **Miri**: [`kt40-miri.log`](JOURNAL/kt40-miri.log) — `--test kt40_tlb` **10/10** (324,35 s) +
  `--lib tlb::tests` **5/5** + `--lib check_tlb` **1/1**.
- **Mutation** (5 ca, hash khớp, log thật): M1 bỏ wait (`kt40-mutation-bo-wait.log`), M2 bỏ flush
  cục bộ (`kt40-mutation-bo-flush-cuc-bo.log`), M3 ack sai seq (`kt40-mutation-ack-sai-seq.log`),
  M4 bỏ rút hộp thư (`kt40-mutation-bo-rut-hop-thu.log` — đỏ ở AP→BSP tương hỗ, không
  self-target), M5 bỏ flush `deactivate` (`kt40-mutation-bo-flush-deactivate.log`).
- **Review độc lập**: `reviewer-kt40` verdict **DRIFT (không UNSAFE)** — đã xử lý đủ 5 việc
  (4 mutation + log, test hart ngoài `active_on` + AP→BSP tương hỗ, oracle `active_on` + M5,
  Miri + miri list, docs/D2-68); `reviewer-kt40b` **CONFIRM 5/5** (tự chạy 10+5+1+62, gate
  552/70, M2 đỏ thật, M4 đỏ đúng 1 ca deadlock, oracle bắt M5, docs khớp). Sửa nốt 3 nit:
  docstring `vspace_activate/deactivate` hết mâu thuẫn, hàng M3 thêm
  `hart_goi_ngoai_active_on_van_shootdown_dung`, tiền tố `v2/agent/JOURNAL/`. Nhật ký đầy đủ:
  [`JOURNAL/kt40-journal.md`](JOURNAL/kt40-journal.md).

**Đề xuất: REVIEW** (chờ reviewer xác nhận sau DRIFT; không tự DONE, không lật QUEUE — D2-37).
**YC/Follow-up**: (1) KT-41 dùng `vspace_walk`/`tlb_lookup` cho `FaultKind::Vm`; (2) KT-42 gộp
`check_tlb` vào `check_vm` + `.scn` shootdown 2 hart; (3) KT-43 `nightly --l1` 100k×4 + 4 mutation
còn lại của K2-PLAN + soak; (4) K3 thay hộp thư/ack bằng IPI/SBI thật, giao thức không đổi.



## KT-38 — K2: `VSpace` + ASID (8 bit/256/thế hệ) + `generation` + `active_on: HartMask`

**Vào**: READY (`agentctl next --project v2` trả KT-38) · HEAD `d240026` ([KT-00] lật KT-37 DONE,
512 test) · `git rev-list --count HEAD..origin/main` = **0** · working tree có `viscript/` của tab
khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 512 test/67 suite**, L1
1.000 seed **1463 ms** ([`JOURNAL/kt38-baseline-gate-fast.log`](JOURNAL/kt38-baseline-gate-fast.log)).
Trong lúc làm, tab ViScript commit `f158085`/`cc7b096` lên `main` (không đụng `v2/`).

**Mục tiêu** (K2-PLAN §3 KT-38 + §2 Q4/Q9, D2-65/66): `VSpace` object + **một** root PageTable
(model API; spec vẫn `reserved`), `page_table_walk` làm oracle; ASID **8 bit = 256/thế hệ**,
free-list, **không tái dùng trong cùng thế hệ**, cạn ⇒ `generation += 1` + **flush toàn bộ một
lần theo `gen`** (`GetAsid` debug); `active_on: HartMask` cập nhật khi hart chạy/nghỉ,
`active_on ⊆ hart online`, teardown giải phóng ASID + flush theo asid; 10k tạo/huỷ;
`check_vspace`/`check_asid` sau mỗi op; Miri; gate 512+.

**Không** làm: TLB/shootdown/ack/chống deadlock (KT-40), W^X/protect (KT-39), VM fault (KT-41),
`.scn`/`check_vm` (KT-42), `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`;
không `unsafe`, không dependency ngoài workspace, không `#[ignore]`, **không sửa test cũ**.

**Kết quả**:
- **Test đỏ trước** (D2-05): [`kt38-red-compile.log`](JOURNAL/kt38-red-compile.log) — **E0432**
  (`AsidState/FlushScope/HartMask/VSpaceInfo/ASID_COUNT`) + **E0599** (`vspace_create`,
  `vspace_*`, `check_vspace`, `check_asid`), 135 dòng, `could not compile` exit 101 (tái lập bằng
  `git stash` **chỉ** `src/` về KT-37 với test đã viết; đã pop, md5 khôi phục). **Không có đỏ
  ngữ nghĩa** vì KT-38 không đổi API cũ (`Retype(VSpace)` vẫn `NotSupported`).
- **Code**: `src/vspace.rs` **(mới)** (`HartMask` ≤64 bit, `VSpaceData/Info`, `AsidState`,
  `FlushScope/FlushCmd`, `ASID_COUNT = 1<<ASID_BITS`, `fresh_asid_free`, 3 unit test) +
  `kobj.rs` (`Kind::VSpace`, `Meta.space`, 9 trường mới, `vspace_create`/`create_vspace`,
  `asid_alloc`/`push_flush`, 13 API VSpace, `check_vspace` 5 nhóm + `check_asid` 3 nhóm trong
  `sched_after_op`, `finalise_vspace` + `revoke_children` atomic/`HasChildren`, `page_table_attach`
  chặn root VSpace, 3 unit test âm) + `tests/kt38_vspace.rs` **(mới, 7 test)**.
- **Docs**: `v2/docs/k2-vm.md` **§12** (object/root + model API, ASID free-list/về hưu, `active_on`,
  lệnh flush cho KT-40, 5+3 nhóm bất biến, `HasChildren`, ranh giới).
- **Cổng**: [`kt38-check.log`](JOURNAL/kt38-check.log) PASS; [`kt38-gate-fast.log`](JOURNAL/kt38-gate-fast.log)
  **PASS 526 test/68 suite**, L1 **1261 ms**; `kt38_vspace` **7/7** (9,34 s), lib **54/54**,
  L0 **526/0**.
- **Miri**: [`kt38-miri.log`](JOURNAL/kt38-miri.log) — **PASS**: `kt38_vspace` 7/7 (61,29 s) +
  `--lib` lọc `vspace` 5/5 + `check_vspace` 2/2 + `check_asid` 2/2 (3,27 s).
- **Review độc lập**: nhờ lead spawn `reviewer-kt38` (read-only); chờ verdict trước commit.

**Đề xuất: REVIEW** (chờ verdict reviewer; không tự DONE, không lật QUEUE — D2-37).
**YC/Follow-up**: (1) KT-39 W^X đặt trên hook `page_table_map/unmap` + bộ đếm Frame (VSpace đã
uỷ quyền nguyên đường này); (2) KT-40 tiêu thụ `flush_log` (`Asid`/`Generation`) + thêm kind
`(asid, va)` + TLB 16 entry/hart + ack thế hệ; (3) KT-41 dùng `vspace_root`/`vspace_info` cho
`FaultKind::Vm`; (4) KT-42 gộp `check_vspace`/`check_asid` vào `check_vm`; (5) K3: thống nhất
đường tạo VSpace với Frame/PageTable (Retype) khi `spec` mở `VSpace`, hoặc giữ model API.


## KT-37 — K2: `PageTable` 3 cấp (root/cấp 1/cấp 0) + map/unmap + rights/attrs + 7 mã lỗi

**Vào**: READY (`agentctl next --project v2` trả KT-37) · HEAD `eaf30c1` ([KT-00] lật KT-36 DONE,
494 test) · `git rev-list --count HEAD..origin/main` = **0** · working tree có `viscript/` của tab
khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 494 test/66 suite**, L1
**985 ms** ([`JOURNAL/kt37-baseline-gate-fast.log`](JOURNAL/kt37-baseline-gate-fast.log)).

**Mục tiêu** (K2-PLAN §3 KT-37, D2-65 Q2/Q3/Q7): `PageTable` object + cây 3 cấp dùng
`vifast-pte` (VA 39 bit); `map` chọn lá theo **cỡ Frame** (4 KiB cấp 0, 2 MiB cấp 1, 1 GiB cấp
2), thiếu bảng cấp trên ⇒ `MissingPageTable`; trùng ⇒ `AlreadyMapped`; lệch căn ⇒ `Alignment`;
range ⇒ `RangeError`; unmap chưa map ⇒ `NotMapped`; unmap gỡ bảng còn con ⇒ `HasChildren`; lưu
rights/attrs **từng mapping** (không suy từ cap); bộ đếm `maps/mapped_w/mapped_x` toàn cục;
walk ngược VA→PA 100% sau 10.000 op; `check_pages` sau mỗi op; Miri; gate 494+.

**Không** làm: VSpace/ASID/TLB (KT-38/40), W^X/protect (KT-39), VM fault (KT-41), `.scn`/
`check_vm` (KT-42), `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không
`unsafe`, không dependency ngoài workspace, không `#[ignore]`.

**Kết quả**:
- **Test đỏ trước** (D2-05): [`kt37-red-semantics.log`](JOURNAL/kt37-red-semantics.log) (API cũ:
  `Retype(Untyped, PageTable)` trả `NotSupported` ⇒ 0/1) + [`kt37-red-compile.log`](JOURNAL/kt37-red-compile.log)
  (**E0432** `MapAttrs/MapRights/Mapping` + **E0599** 10 method PageTable ⇒ `could not compile`).
- **Code**: `src/pagetable.rs` **(mới)** `MapRights`/`MapAttrs`/`Mapping`/`PageTableInfo`/
  `PageTableData`/layout PTE mô hình + 4 unit test; `kobj.rs`: `Kind::PageTable`, `Meta.pt`,
  `page_tables`/`page_table_objects` (cửa sổ nhanh), `retype_page_table` (4 KiB arena, atomic),
  `page_table_{map,map_obj,unmap,walk,attach,detach,count,info,pte_raw}`, `check_pages` (5 nhóm
  bất biến) trong `sched_after_op`, `finalise_page_table` + `revoke_children` kiểm `HasChildren`
  atomic; `tests/kt37_pagetable.rs` **(mới, 10 test)** gồm bảng chân trị **7 mã lỗi × 3 cỡ**,
  property **10.000 op** (walk ngược 100% + bộ đếm + `check_pages` mỗi op) và 100×10 vòng không
  rò; 4 unit âm KT-37 (`check_pages` bắt `mapped`/Frame counter/`attached`/bảng chết còn dữ
  liệu/PTE sai, `pa` lệch căn ⇒ `Alignment`).
- **Docs**: `v2/docs/k2-vm.md` **§11** (object/cây, layout PTE, API, thứ tự kiểm + 7 mã lỗi, 5
  nhóm bất biến, ranh giới).
- **Cổng**: [`kt37-gate-fast.log`](JOURNAL/kt37-gate-fast.log) **PASS 512 test/67 suite**, L1
  **726 ms**; [`kt37-check.log`](JOURNAL/kt37-check.log) PASS; [`kt37-miri.log`](JOURNAL/kt37-miri.log)
  Miri **10/10** (171,54 s) + lib **4/4** + unit âm **3/3**.
- **Review độc lập**: `reviewer-kt37` **APPROVE code/gate** (tự chạy 10/10 + gate 512/67, L1
  782 ms; cây 3 cấp/7 mã lỗi/rollback/walk 10k/`check_pages`/scope đạt). Nit đã xử lý: **N1**
  detach kiểm căn lề `Alignment`; **N2** `MAX_FRAMES_PER_VSPACE` sau `AlreadyMapped` (khớp
  docs); **N3** bỏ `saturating_sub` → kiểm `mapped > 0` trước khi sửa cây; **N6** journal
  39+8=47; **N7** mục STATE này.

**Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (reviewer APPROVE; không tự DONE, không lật QUEUE
— D2-37). **YC/Follow-up**: (1) KT-38 VSpace/ASID dùng `page_table_walk` làm oracle; (2) KT-39
W^X đặt trên hook `page_table_{map,unmap}` + bộ đếm Frame; (3) KT-42 gộp `check_pages` vào
`check_vm`, thêm `.scn` + shared read-only chính thức; (4) KT-43 thêm `kt37_pagetable` vào
`nightly --only-miri` (Miri đầy đủ).

## KT-36 — K2: `Frame` 3 cỡ (4 KiB/2 MiB/1 GiB) + ownership/Untyped + `GetAddress` + bộ đếm W^X

**Vào**: READY (`agentctl next --project v2` trả KT-36) · HEAD `c987fe1` ([KT-00], 477 test) ·
`git rev-list --count HEAD..origin/main` = 0 · working tree có `viscript/` của tab khác (**không
chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 477 test**, L1 **832 ms**
([`JOURNAL/kt36-baseline-gate-fast.log`](JOURNAL/kt36-baseline-gate-fast.log)).

**Mục tiêu** (K2-PLAN §3 KT-36, D2-65 Q2/Q3/Q7): `FrameMeta { pa, size_bits, live, mapped_w,
mapped_x, maps }`; `Retype(Untyped, Frame, size_bits)` chỉ nhận `{12,21,30}`, kiểm `pa` căn lề +
nằm trong cửa sổ PA của Untyped (lệch ⇒ `Alignment`, cỡ lạ ⇒ `InvalidArgument`); `Frame.GetAddress`;
frame **không cấp byte thật** (1 GiB vẫn 0 byte arena); frame chết ⇒ **không còn mapping** (bộ đếm
0, nếu không ⇒ lỗi bất biến, không im lặng); `check_frames` sớm; **chưa** map vào VSpace/PageTable.

**Không** làm: map/unmap PageTable/VSpace (KT-37/38), rights/attrs khi map, TLB (KT-40), luật W^X
(`WxViolation`, KT-39), `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`,
`v2/bin/v2`; không `unsafe`, không dependency ngoài workspace, không `#[ignore]`.

**Kết quả**:
- **Test đỏ trước** (D2-05): [`kt36-red-semantics.log`](JOURNAL/kt36-red-semantics.log) (API cũ:
  `retype Frame` trả `NotSupported` cho cả `size_bits=13` lẫn 3 cỡ ⇒ 0/2) và
  [`kt36-red-compile.log`](JOURNAL/kt36-red-compile.log) (**E0432** `FrameMeta`/`UNTYPED_PA_WINDOW`
  + **E0599** 9 method mới ⇒ 78 lỗi).
- **Code**: `v2/sim/vifast-sim/src/frame.rs` **(mới)** `FrameMeta`/`FrameArea`/`carve`/
  `UNTYPED_PA_WINDOW = 1 << 30`; `kobj.rs`: `Kind::Frame` (mốc body), `Meta.frame`/`Meta.frame_area`,
  `frame_windows`/`frame_objects`, `add_untyped(base, size)` **nay dùng `base`** (đáy cửa sổ PA),
  `retype` rẽ nhánh `Frame` → `retype_frame` (4 lỗi **atomic**), `frame_pa_overlap` (chặn chồng
  `pa`), `frame_get_address`/`frame_address`/`frame_meta`/`frame_count`/`untyped_frames`/
  `untyped_pa_window`/`frame_note_map`/`frame_note_unmap`, `check_frames` (**7 nhóm** bất biến — khớp D2-66 — chạy cả trong `sched_after_op`
  với cửa sổ nhanh `frame_objects == 0`), `finalise_frame`,
  `revoke_children` từ chối `HasChildren` khi Frame còn mapping + reset watermark cửa sổ;
  `check_bytes`/`untyped_allocated` **bỏ qua** Frame (không tiêu byte arena).
- **Test**: `tests/kt36_frame.rs` **12 test** (cỡ lạ; 3 cỡ × GetAddress/meta/bộ đếm; biên
  `0/2^12/2^21/2^30`; pa lệch 1 ⇒ `Alignment`; 1 GiB không ăn byte; trộn cỡ giảm dần; frame chết
  sạch; revoke ⇒ `HasChildren` atomic; bất biến bộ đếm; ownership/chồng `pa`; **10.000 retype +
  huỷ**; property **10.000 seed**) + **5 unit test âm** `check_frames` + 1 test `FrameArea::carve`.
- **Docs**: `v2/docs/k2-vm.md` **§10** (6 trường, cửa sổ PA 1 GiB, thứ tự kiểm `Retype`,
  `GetAddress`, finalise/`check_frames`, ranh giới không làm).
- **Cổng**: [`kt36-gate-fast.log`](JOURNAL/kt36-gate-fast.log) **PASS 494 test/66 suite (65 suite cũ + `kt36_frame`)**, L1
  **769 ms**; [`kt36-check.log`](JOURNAL/kt36-check.log) PASS; [`kt36-miri.log`](JOURNAL/kt36-miri.log)
  Miri **12/12** (36,10 s) + **5/5** (2,50 s). Hồi quy cổng tự bắt (L1 54.513 ms khi `check_frames` chưa có cửa sổ
  nhanh) đã sửa — xem `JOURNAL/kt36-journal.md` §4.1.

**Quyết định có thể gây tranh luận** (chi tiết `JOURNAL/kt36-journal.md` §3 + `k2-vm.md` §10):
1. `pa` là **nhãn ảo** cấp từ cửa sổ PA 1 GiB của Untyped (`[TK]`), Frame **không** tiêu byte arena
   ⇒ 1 GiB Frame không ăn RSS; hệ quả `base` của `add_untyped` thành đáy cửa sổ và Untyped chồng
   cửa sổ ⇒ `AlreadyMapped`.
2. **Không căn lề ngầm**: `pa = base + watermark` lệch căn ⇒ `Alignment`; trộn cỡ phải cấp **giảm
   dần** (1 GiB → 2 MiB → 4 KiB).
3. `Revoke` Untyped khi Frame còn mapping ⇒ **`HasChildren` atomic** (K2 chưa có ngược
   Frame→PageTable để tự unmap; unmap thật là KT-39).
4. Bất biến bộ đếm KT-36 là `w ≤ maps && x ≤ maps` — **chưa** `w + x ≤ maps` (luật W^X là KT-39).
5. Test 10.000 retype+huỷ = **100 vòng × 100 Frame** (mỗi vòng một `Machine`): bất biến per-op của
   mô hình là O(#object đã từng cấp) nên dồn 10k object vào một mô hình là O(n²) ~56 s mà không
   kiểm thêm gì; vẫn là **10.000** lần retype + 10.000 lần huỷ thật.

**Commit**: `9c54bcf` `[KT-36]`, đã push (`ba3ae01..9c54bcf main -> main`) — **DONE-mã, chờ Agent 0
lật sổ**; `git rev-list --count HEAD..origin/main` = 0 sau push. Gate/Miri chạy lại trên **cây cuối**
(sau 3 nit của reviewer): gate **PASS 494 test**, L1 **769 ms**; Miri **12/12** (36,10 s) + **5/5**
(2,50 s).

**Review trước commit**: `reviewer-kt36` verdict **APPROVE code**; **D2-66** (Agent 0) đã push
(`9b70618`) — chốt `UNTYPED_PA_WINDOW` 1 GiB/`add_untyped(base)`/nhãn PA ảo/bất biến bộ đếm; 3 nit
của reviewer đã sửa **trước** commit: assert tràn `base+1 GiB` ở `add_untyped`, câu "Frame sống
không chồng `pa`" ở `k2-vm.md` §10.2, thống nhất **7 nhóm** bất biến (D2-66). Không lật QUEUE.

## KT-35 — mở K2: crate `vifast-pte` (hình 3 cấp + PTE array) và `v2/docs/k2-vm.md` (D2-65 Q1/Q2/Q8)

**Vào**: READY (D2-65) · HEAD `53417d9` · `git rev-list --count HEAD..origin/main` = 0 · baseline
`gate --fast` **PASS 468 test**, L1 **711 ms** ([`JOURNAL/kt35-baseline-gate-fast.log`](JOURNAL/kt35-baseline-gate-fast.log)).

**Mục tiêu** (KT-35, K2-PLAN §3): (1) crate `v2/lib/vifast-pte` `no_std` + `forbid(unsafe_code)`,
không dependency, không `cfg` theo kiến trúc: `FrameSize{K4,M2,G1} ↔ size_bits{12,21,30}` (lạ ⇒
`InvalidArgument`), `Pte(u64)`, `PteArray<const N>`, `walk_shape` VA 39 bit (3 chỉ số bit 12/21/30);
(2) `v2/docs/k2-vm.md` 7 hằng số `[TK]`, hình 3 cấp, căn lề, bảng 7 mã lỗi dùng lại, Q1–Q8 + mục
"K2 không phải kernel thật", ghi rõ `spec/` không đổi và `kernel.toml` chưa tạo; (3) test đỏ trước;
(4) gate + Miri + log + STATE.

**Không** làm: PageTable walker/map thật (KT-37), Frame/VSpace/TLB (KT-36..KT-41), sửa `spec/`,
`v2/lib/vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không `unsafe`/dep/`#[ignore]`;
không tạo `spec/kernel.toml`; không đổi ABI.

**Kết quả**:
- **Test đỏ trước** (D2-05): crate dựng khung rỗng (chưa API) + `tests/kt35_pte.rs` ⇒ **E0432**
  13 unresolved imports (`walk_shape`, `FrameSize`, `Pte`, `PteArray`, `PteError`, `WalkShape`,
  7 hằng số) ([`JOURNAL/kt35-red-compile.log`](JOURNAL/kt35-red-compile.log)).
- **Crate** `v2/lib/vifast-pte` (6 tệp src/test): `FrameSize::{K4,M2,G1}` +
  `size_bits()`/`size_bytes()`/`from_size_bits()`/`TryFrom<u8>`; `Pte::EMPTY` + `from_raw`/`raw`/
  `is_empty` (u64 thô, **không** gán nghĩa bit — KT-37); `PteArray<N>` không cấp phát,
  `PteArray<512>` = **4096 B**, `get`/`set` trả `Option` (ngoài biên không panic/không ghi);
  `walk_shape(u64) -> WalkShape` trả `[cấp2, cấp1, cấp0]` (9 bit/cấp, bit 30/21/12);
  `PteError::InvalidArgument` model/local (`code_name()` = tên `spec/abi.toml`).
- **Test** `tests/kt35_pte.rs` **9 test**: hằng số Q1; roundtrip cỡ/bits/byte; **size_bits lạ
  0..=255** ⇒ chỉ 12/21/30 OK, còn lại `InvalidArgument`; `va=1<<38 ⇒ [256,0,0]`,
  `u64::MAX ⇒ [511,511,511]` (cắt, không panic); `PteArray<512>` 4096 B + mọi chỉ số biên/`usize::MAX`;
  property **10.000 VA/shape** tất định (LCG) dựng lại bit 12..38; Miri chạy 256 vòng.
- **Docs** [`v2/docs/k2-vm.md`](../docs/k2-vm.md): 7 hằng số `[TK]` (PAGE_SIZE_BITS=[12,21,30],
  PTE_LEVELS=3, VA_BITS=39, ASID_BITS=8, TLB_ENTRIES_PER_HART=16, MAX_FRAMES_PER_VSPACE=4096,
  MAX_MAP_DEPTH=3), hình 3 cấp 9/9/9/12 + bảng lá theo cỡ + căn lề `pa/va % frame_size == 0`,
  bảng **7 mã lỗi dùng lại** (4/6/10/11/12/13/19; `InvalidArgument` 5 là mã chung cho `size_bits`),
  Q1–Q8 theo D2-65, **"K2 không phải kernel thật"**, `spec/` không đổi + `kernel.toml` chưa tạo.
- **Gate** `gate --fast` **PASS 477 test/65 suite**, L1 1.000 seed **607 ms**, fmt/clippy sạch,
  luật R1..R5 OK, gen-check ON ([`JOURNAL/kt35-gate-fast.log`](JOURNAL/kt35-gate-fast.log), chạy
  **sau** fix B1/B2). Hai lần đỏ trước đó: (a) **R2** bắt **chữ** `cfg(target_arch)` trong
  doc-comment (grep thô) — sửa chú thích, log
  [`kt35-gate-fast-fail-r2-doc.log`](JOURNAL/kt35-gate-fast-fail-r2-doc.log); (b) reviewer B1 thiếu
  inner attribute — sửa rồi chạy lại; bản PASS trước review giữ
  [`kt35-gate-fast-pre-review.log`](JOURNAL/kt35-gate-fast-pre-review.log).
- **Miri** `./v2/bin/v2 miri -p vifast-pte` **PASS 9 test**, 0 failed
  ([`JOURNAL/kt35-miri.log`](JOURNAL/kt35-miri.log)); không thêm vào `nightly --only-miri`
  (crate không unsafe/alloc/IO; lệnh rời đã đủ; KT-43 quyết khi chốt cổng K2) — `v2/bin/v2` **không đổi**.
- **Không đổi ABI**: `spec/` nguyên trạng (VSpace 9/PageTable 10/Frame 11 vẫn reserved +
  unsupported), không `.scn`, không `vifast-model`, không `viscript/`; chỉ stage `v2/`.

**Review độc lập**: `reviewer-kt35` verdict **DRIFT** (2 mục chặn) — **đã sửa, chạy lại xanh**:
- **B1 (chặn)**: crate root **thiếu** inner attribute thật — `#![no_std]`/`#![forbid(unsafe_code)]`
  mới chỉ nằm trong doc comment (`grep '^#!'` = 0). Đã thêm **2 dòng đầu `lib.rs`** (DoD Q8/D2-65);
  reviewer đã thử bản 2 attribute + 9/9 test.
- **B2 (chặn nhỏ)**: `k2-vm.md` bảng lá sai — M2 (2 MiB) phủ **một bảng cấp 0** (512 entry =
  `2^9` trang 4 KiB), lá ở cấp 1; G1 (1 GiB) phủ **một bảng cấp 1** (`512 × 512 = 2^18` trang
  4 KiB), lá ở cấp 2 — đã sửa ô bảng cho khớp.
- **Nit**: `SHIFT_L0` suy từ `PAGE_SIZE_BITS[0]` (không chép 12); nguồn design sửa `:353-375` →
  `:353-366` (§4.10 kết thúc trước §4.11); ghi rõ `MAX_FRAMES_PER_VSPACE=4096` /
  `MAX_MAP_DEPTH=3` là `[TK]` do KT-35 chọn, được reviewer-kt35/Agent 0 chấp nhận.
- Sau sửa: `cargo test -p vifast-pte` **9/9**, **Miri 9/9**, **gate 477 PASS**; log cuối
  `kt35-gate-fast.log`/`kt35-miri.log` (bản trước review giữ `*-pre-review.log`).

**Đề xuất**: **KT-35 DONE** (crate + docs + test đỏ + gate + Miri đạt; không nới ngưỡng/không retry
để lấy xanh — lần đỏ R2 là lỗi tooling do chú thích, đã sửa đúng chỗ). **YC mới**: (i) `reviewer-kt35`
xác nhận hình 39 bit/truncation + `PteArray` biên + hằng số Q1; (ii) Agent 0 lật KT-35 DONE và mở
KT-36 (Frame 3 cỡ) khi D2-65 còn hiệu lực; (iii) KT-43 cân nhắc thêm `vifast-pte` vào
`nightly --only-miri` khi có task cài đặt.

**Commit**: `[KT-35]` trên HEAD `41a37d3` (commit **ST-00 ViScript** đến trong lúc chạy — chỉ
chạm `viscript/agent/DECISIONS.md`, **không chạm `v2/`**; baseline/gate của KT-35 chạy ở
`53417d9` + cây `v2/` hiện tại); chỉ stage `v2/` (crate + docs + Cargo.toml/lock + JOURNAL + STATE),
không `git add -A`, không `viscript/`.

## KT-34 — đóng **C2** cuối K1c: cap `Reply` đang active, bất biến liên kết Reply, 100k×4 (D2-63)

**Vào**: READY (D2-63) · HEAD `43c2e3c` · `git rev-list --count HEAD..origin/main` = 0 · baseline
`gate --fast` **PASS 461 test/62 suite**, L1 **38.973 ms** (máy tải; các gate trước 611–747 ms), wall **236,86 s**
([`JOURNAL/kt34-baseline-gate-fast.log`](JOURNAL/kt34-baseline-gate-fast.log)).

**Mục tiêu** (D2-63): (1) `recv`/`call` **từ chối** cap `Reply` đang `caller=Some` (**trước** khi
đổi state/nhận message) — **không** ghi đè caller làm TCB cũ `BlockedOnReply` mất đường Reply;
(2) bất biến C2 `BlockedOnReply ⇔ Reply.caller == Some(tcb)` sau **mỗi** public op; (3) test + fuzz
property (Signal/WakeAddr sau khi xếp hàng **phải** đánh thức, mọi thứ tự bước); (4) 10k×4 rồi
**100k×4**; (5) gate + Miri các test ảnh hưởng; (6) report C2 ĐẠT + đề xuất K1c DONE.

**Không** làm: sửa semantics ngoài phạm vi C2, `spec/`/`.scn`/`vifast-model`/QUEUE/DECISIONS/
AGENTS/`viscript/`; không dependency/`unsafe`/`#[ignore]`/`#[allow]` mới; không nới seed/ngưỡng;
không dùng lại cap Reply active trong driver; không "sửa test cho xanh".

**Kết quả**:
- **Test đỏ trước** (D2-05): [`JOURNAL/kt34-red-reply-reuse.log`](JOURNAL/kt34-red-reply-reuse.log) —
  trên code cũ `Recv` lần hai với cap `Reply` đang active **thành công**, `left: Ok(())` vs
  `right: Err(Busy)` ⇒ đúng lỗi model (ghi đè caller), không phải test-only.
- **Sửa** (`v2/sim/vifast-sim/src/kobj.rs`, model-only `[TK]`, ABI/`spec/` **không đổi**): `recv`
  trả `Busy` khi Reply cap đang `caller=Some` (**trước** `reset_ipc_error`, trước pop/đổi trạng
  thái); `call_msg` lớp 2 (Reply cap của waiter đã bind **qua cap khác** cùng object) **trước**
  `admit_message`; `reply_recv`/`nb_send_recv` kiểm Reply **mới** trước khi gửi reply (ngoại lệ
  `recv_reply == reply`); **`check_reply_links()`** trong `sched_after_op` sau **mỗi** op; cổng
  `transition_tcb` giữ liên kết (vào bind, rời gỡ, ghi đè ⇒ `IllegalOperation`).
- **Test**: `kt25_ipc2` **+5** (Busy + caller cũ giữ nguyên + Reply đúng ⇒ `Ready` + cap dùng lại
  được; lớp 2 qua cap khác; cổng transition bind/gỡ/từ chối ghi đè; `Call` **trước** `Recv` đầu
  vẫn hợp lệ; `ReplyRecv` Reply mới ⇒ Busy trước khi gửi + dùng lại chính Reply hợp lệ);
  `kt27_notification` **+1** + `kt28_waitaddr` **+1** property **6/6 hoán vị** (C2 hai chiều);
  `kt22_mailbox` đổi rig 1 → **3 Reply riêng** (3 `Recv` cần 3 Reply chưa bind — one-shot; kỳ
  vọng FIFO giữ nguyên); driver `l1_k1c` chỉ cấp Reply **rảnh** + `check_reply_links` mỗi op.
- **10k×4 PASS**: 40.000 lượt, **661 s**, `ipc_sent` 144.977/144.871/144.552/145.056; `delivered`
  27.845/28.155/28.177/28.404; `sent>1` mọi lượt, `delivered>0` mọi smp
  ([`JOURNAL/kt34-l1-10k.log`](JOURNAL/kt34-l1-10k.log)).
- **100k×4 PASS**: 400.000 lượt, `ipc_sent` **5.786.296**, `ipc_delivered` 1.123.598, `ipc_dropped` 3.748.173, wall **6306,38 s (105m06,38)**, cùng seed ⇒ cùng trace, `sent>1` mọi lượt, `delivered>0` mọi smp, **0 panic/0 vi phạm `check_reply_links`** ([`JOURNAL/kt34-l1-100k.log`](JOURNAL/kt34-l1-100k.log))
- **Gate cuối PASS 468 test/62 suite**, L1 **275 ms**, wall **239,32 s** (wall host)
  ([`JOURNAL/kt34-gate-fast.log`](JOURNAL/kt34-gate-fast.log)).
- **Miri 3 tập ảnh hưởng PASS**: `kt25_ipc2` 29/111,28 s, `kt27_notification` 27/66,02 s,
  `kt28_waitaddr` 13/115,91 s — 0 failed ([`JOURNAL/kt34-miri-kt25.log`](JOURNAL/kt34-miri-kt25.log),
  [`-kt27`](JOURNAL/kt34-miri-kt27.log), [`-kt28`](JOURNAL/kt34-miri-kt28.log)).
- **Docs**: [`v2/docs/k1c-ipc-semantics.md`](../docs/k1c-ipc-semantics.md) **§15** — quy tắc cap
  `Reply` đang active (`Busy` **model-only**, không đổi ABI), bất biến, bằng chứng, giới hạn.
- **Review độc lập**: `reviewer-kt34` **APPROVE code** — "normal Call/Recv không bị chặn (Recv đầu
  thấy caller=None; chỉ cap đang active mới `Busy`); kt22 1→3 Reply hợp lý one-shot, không phải
  sửa test cho xanh; `.scn` 04/05/07/08 + KT-21/23/26 xanh; vị trí kiểm đúng".

**Follow-up `reviewer-kt34b`** (CHANGES nhỏ, 5 điểm — đã sửa, commit follow-up):
- **F1 (chặn)**: Miri **bổ sung** tệp bị sửa trong KT-34 — `kt22_mailbox` **PASS 10/240,93 s**
  + khuyến nghị `kt21_donation` 13/245,40 s, `kt24_ipc` 13/45,79 s, `kt26_fault` 24/90,99 s,
  `kt29_smp` 13/264,62 s ⇒ **8 tập / 123 test** Miri của đường K1c cho KT-34, 0 failed
  ([`JOURNAL/kt34-miri-kt22_mailbox.log`](JOURNAL/kt34-miri-kt22_mailbox.log), …).
- **F2**: bảng §8 K1C-DOD sửa **29/27/13, tổng 138** (khớp gate 468).
- **F3**: §7 C2 ghi rõ **1 ca đỏ trước + 5 test** (`kt34-red-reply-reuse.log`: `1 failed; 26 filtered out`).
- **F4**: bảng 100k thêm `ipc_pending` cuối lượt (suy ra từ bất biến): 218.449/229.074/232.012/234.990,
  **tổng 914.525** ⇒ 5.786.296 = 1.123.598 + 914.525 + 3.748.173.
- **F5**: ghi chú **HEAD của log gate**: `kt34-gate-fast.log` chạy ở `43c2e3c` + cây KT-34 chưa
  commit (**nội dung `v2/` == `40a8e5a`**); reviewer-kt34b đã chạy lại `gate --fast` tại `40a8e5a`
  — **PASS 468 test**, khớp.

**Đề xuất**: **KT-34 DONE**; **K1c DONE (đề xuất)** — C1/C2/C3/C4 + Miri + gate ĐẠT; nợ **model-only
/ K2 / K3** vẫn mở nhưng **không thuộc K1c** (K1C-DOD §9.1/§9.3/§9.4). Không tự lật QUEUE.

**Commit**: `[KT-34]` (hash xem `git log -1`; chỉ stage `v2/`, `viscript/` không chạm).

**Trạng thái**: **REVIEW** — chờ Agent 0 lật (không tự DONE).

## KT-33 — đóng 2 nợ cuối K1c: **C3 12/12** + Miri `kt25_ipc2` + siết driver (D2-61/62)

**Vào**: READY (agentctl `next --project v2` trả KT-33) · HEAD `16eb801` · `git rev-list --count
HEAD..origin/main` = 0 · baseline `gate --fast` **PASS 461 test/62 suite**, L1 **747 ms**, wall
**223,87 s** ([`JOURNAL/kt33-baseline-gate-fast.log`](JOURNAL/kt33-baseline-gate-fast.log)).

**Mục tiêu** (đúng 4 việc, không sửa semantics): (1) đủ **12/12 mutation** §7.2 — mỗi ca sửa
**một chỗ**, chạy **đúng một** test, log `exit 101` + `test result: FAILED` + assert, khôi phục
hash == `HEAD`; (2) thêm `kt25_ipc2` vào `nightly --only-miri` + chạy lại all; (3) siết driver
`ipc_sent > 1` + `ipc_delivered > 0` + panic `kiem` kèm `seed/smp`, chạy 10k×4 + gate; (4) cập nhật
K1C-DOD (C1/C3/Miri ĐẠT, C2 một phần) + STATE.

**Không** làm: sửa semantics/`spec`/`.scn`/`vifast-model`/QUEUE/DECISIONS/AGENTS/`viscript/`;
không `#[ignore]`/`#[allow]`/unsafe/dependency; không commit đột biến; không nới seed/ngưỡng.

**Kết quả**:
- **Mutation 12/12** §7.2 — M1 (bỏ copy 1/64 MR, `kt24` MR 32 left 0/right 4026531872), M2
  (bỏ `RIGHT_GRANT`, `kt25` caps 4 vs 3), M3 (không bỏ GRANT, rights 4 vs 0), M4 (bỏ unwrap,
  caps 2 vs 1), M5 (Signal không xoá word, 165 vs 0), M6 (bỏ cổng một-waiter, `Ok` vs `Busy`),
  M7 (xếp hàng trước, `BlockedOnAddress` vs `Ready`), M8 (wake mọi khoá, 2 vs 1), M9 (bỏ
  `home=to`, home 0 vs 1), M10 (resume không ghi MR, `[0,0,0,0]` vs `[7,8,9,10]`), M11 (mailbox
  ghi đè, `Ok` vs `Err(Busy)`), M12 (không tách caller, `Some(7)` vs `None`); **12/12** khôi
  phục hash == `HEAD` (kobj `37ff523…`, ipc `2173300…`, mailbox `5747220…`); mỗi ca có **xanh
  trước** + **1 test** (`filtered out`). **M11 dùng `unsafe` chỉ trong mutant tạm** (ghi đè slot
  `Mailbox`) — đã khôi phục, **cây cuối sạch, không `unsafe` mới** (`mailbox.rs` không đổi so HEAD). Log [`JOURNAL/kt33-mutation.log`](JOURNAL/kt33-mutation.log),
  bảng + 2 ghi chú deviation câu chữ §7.2 (M5/M6) ở [`reports/K1C-DOD.md`](reports/K1C-DOD.md) §3b.
- **Miri all PASS**: `v2/bin/v2` thêm **một dòng** `kt25_ipc2`; `nightly --only-miri` **19 tập /
  **300 test** / 0 failed, wall **4141,74 s** (69,03 phút); `kt25_ipc2` **24/24 (89,28 s)**; K1c
  trong Miri **128 test**. Mục §7.1 #3 **ĐẠT**. Log
  [`JOURNAL/kt33-miri-all.log`](JOURNAL/kt33-miri-all.log).
- **Driver siết** (chỉ `tests/l1_k1c.rs`, +32/−18): `ipc_sent > 1` từng lượt (prologue chỉ 1 ⇒
  64 op phải sinh thêm), `ipc_delivered > 0` tổng mỗi smp, panic `kiem` kèm `seed/smp`. **10k×4
  PASS**: 40.000 lượt, **658,60 s**, bộ đếm **trùng khít** KT-32 (144.977/27.850/95.461; …) ⇒
  không đổi hành vi/trace, chỉ mạnh hơn bất biến. Log
  [`JOURNAL/kt33-l1-10k.log`](JOURNAL/kt33-l1-10k.log).
- **Gate cuối PASS 461 test/62 suite**, L1 **653 ms** (⇒ **đạt** ngưỡng 60 s của QD §5.2 cho L1),
  driver L0 **13,37 s**; `real 221,41 s` là **wall host** (không phải ngưỡng cổng; baseline
  223,87 s) — không nới ngưỡng, không retry. Log
  [`JOURNAL/kt33-gate-fast.log`](JOURNAL/kt33-gate-fast.log).
- **Report**: [`reports/K1C-DOD.md`](reports/K1C-DOD.md) cập nhật §2c/§3b/§4b/§5b + §7 (C1 ĐẠT,
  C2 **một phần — không claim**, C3 **ĐẠT 12/12**, C4 ĐẠT theo định nghĩa) + §9 (đóng nợ 2/8/9;
  giữ nợ model-only/K2/K3) + §10 (**K1c giữ REVIEW — C2 nợ; KT-34 đóng C2**).

**Đề xuất**: **KT-33 DONE**; **K1c giữ REVIEW — CHƯA DONE** vì **C2 chỉ đạt một phần** (nợ
Reply cap reuse có thể ghi đè `caller`; **KT-34** đóng C2); C1/C3/C4 + Miri ĐẠT, nợ model-only/K2/K3
vẫn mở ở §9. Không tự lật QUEUE; verdict `reviewer-kt33`: **DRIFT** (đã sửa nhãn K1c DONE→REVIEW +
wall host) — chờ Agent 0 lật `KT-33 DONE`.

**Commit**: `[KT-33]` (hash xem `git log -1`; chỉ stage `v2/`, `viscript/` không chạm).

**Trạng thái**: **REVIEW** — chờ reviewer độc lập + Agent 0 lật (không tự DONE).

## KT-32 — driver L1 **trộn IPC** cho K1c, đóng nợ **C1-100k** (D2-61)

**Vào**: READY · HEAD khi mở `83df94a` (khi commit `efa787f` — hai commit `739fc3e`/`efa787f`
dòng ViScript đến trong lúc chạy, **không chạm `v2/`**) · `git rev-list --count HEAD..origin/main`
= 0 · baseline `gate --fast` PASS **461 test**, wall **233,94 s**, L1 **2.846 ms**
([`JOURNAL/kt32-baseline-gate-fast.log`](JOURNAL/kt32-baseline-gate-fast.log)).

**Mục tiêu**: mở rộng **chỉ** driver [`sim/vifast-sim/tests/l1_k1c.rs`](sim/vifast-sim/tests/l1_k1c.rs)
(test-only) để 100k seed **thật sự** đi qua IPC/Notification/WaitAddr — K1C-DOD §9.2 (nợ C1),
D2-61. Không sửa semantics, không nới seed/ngưỡng/op (giữ `SO_OP = 64`), không `.scn`/spec/
QUEUE/DECISIONS/AGENTS/`viscript/`, không `#[ignore]`/`unsafe`/dependency.

**Kết quả**:
- Driver: giữ **9 op** KT-29 + thêm **10 op** K1c — Send/NBSend/Recv/Call/Reply (Endpoint/Reply
  cap gốc trong CNode), Signal/Wait (Notification + cap `mint` badge), WaitAddr/WakeAddr/Store
  (khoá `(frame, offset)`, `expected = addr_word`). Fallback `san_sang` chọn TCB `Ready|Running`,
  `wake` rồi thử lại, cạn thì ghi nhận — **không** panic/không gọi op sai state; prologue
  `NBSend` bảo đảm `ipc_sent ≥ 1` từ seed đầu. Sau **mỗi** op (kể cả prologue):
  `check_sched/check_harts/check_ipc/check_waitaddr/check_mailboxes/check_caps`; cuối lượt
  `sent == delivered+pending+dropped`; 2 lần/seed + `assert_eq` trace **và** bộ đếm.
- **100k×4 PASS**: 4 dòng `# L1 OK: smp 1/2/4/8`, **400.000 lượt**, wall **6596,88 s**
  (109,95 phút), `ipc_sent` **5.786.297** (14,47/seed), `ipc_delivered` **1.123.586**,
  `ipc_dropped` **3.748.540**, `1 passed; 0 failed`, 0 panic (51,2 triệu lần kiểm) —
  [`JOURNAL/kt32-l1-100k-mixed.log`](JOURNAL/kt32-l1-100k-mixed.log).
- **1k×4** PASS 62,54 s (`kt32-l1-1k-mixed.log`); **10k×4** PASS 503,02 s (`kt32-l1-10k-mixed.log`).
- **Gate cuối**: PASS **461 test**, L1 1.000 seed **269 ms**, wall **124,25 s**, fmt/clippy
  `-D warnings` sạch ([`JOURNAL/kt32-gate-fast.log`](JOURNAL/kt32-gate-fast.log)); driver cũng
  chạy trong L0 (1.000 seed/1 hart) **4,71 s** `1 passed`.
- **Review độc lập**: `reviewer-kt32` (lead spawn, read-only) **APPROVE** — tự chạy 100 seed
  (14,67 ipc_sent/seed), 1k×4 64,03 s (bộ đếm khớp byte-for-byte log tác giả), gate 461; xác
  nhận đủ 10 op/6 checker/tất định/1 tệp test/không ignore-unsafe-dep.
- **Nợ chuyển KT-33**: `ipc_sent>0` gần tautology do prologue (siết `sent>1`/`delivered>0`),
  panic `kiem` chưa kèm seed/smp; **không** claim C2 (dùng lại cap Reply có thể ghi đè `caller`
  — lỗ hổng có sẵn của model).

**Không** làm: đóng K1c (KT-33 còn C3 + Miri), sửa test cũ, đổi QUEUE/DECISIONS.

## KT-31 — cổng cuối K1c: 100k×4 + mutation + Miri + gate + bảng DoD (K1C-PLAN §7.1, D2-60)

**Vào**: READY (agentctl `next --project v2` trả KT-31) · HEAD `0d026bd` · `rev-list --count
HEAD..origin/main` = 0 · baseline `gate --fast` PASS **461 test**, L1 **611 ms**
([`JOURNAL/kt31-baseline-gate-fast.log`](JOURNAL/kt31-baseline-gate-fast.log)).

**Mục tiêu**: chạy **đúng** 5 việc §7.1 (không sửa semantics, không nới seed/ngưỡng, không retry):
(1) `nightly --l1 --seeds 100000 --smp 1,2,4,8`; (2) `gate --fast` cuối; (3) `nightly
--only-miri`; (4) mutation tay 6 bất biến K1c; (5) bảng DoD K1c; thêm (6) parity 10k hồi quy.

**Không** làm: sửa `spec/`/`vifast-model`/`.scn`/QUEUE/DECISIONS/AGENTS/`viscript/`; không
dependency/`unsafe`/`#[ignore]`/`#[allow]`/`#[should_panic]`; không "sửa test cho xanh"; không
lật QUEUE.

**Kết quả**:
- **100k×4 L1 PASS**: 4 dòng `# L1 OK: smp 1/2/4/8, 100000 seed × 64 op`, tổng **400.000 lượt**
  (`# L1 OK: 400000 lượt (smp [1, 2, 4, 8], seeds 100000)`), wall **5043,84 s** (84,06 phút;
  `[ƯL]` plan 20 phút — **T-INFRA** dự toán, không giảm seed). Driver chạy **2 lần/seed** và
  `assert_eq!(a, b)` ⇒ **cùng seed cùng trace**; `check_sched`/`check_harts`/`check_ipc` sau **mỗi**
  op, 0 panic. Log [`JOURNAL/kt31-l1-100k-smp.log`](JOURNAL/kt31-l1-100k-smp.log).
- **Mutation 6/6 đỏ đúng test** (exit 101, `test result: FAILED`, đúng tên test + dòng assert),
  khôi phục `git checkout` → hash == `HEAD:<file>`: IPC cap sai vị trí (`kt25_ipc2::bon_cap...`),
  Notification bỏ OR (`kt27::signal_or...` left 4 right 7), WaitAddr xếp hàng trước
  (`kt28::expected_lech...` BlockedOnAddress vs Ready), Fault resume không ghi MR
  (`kt26::resume_with_regs...` `[0,0,0,0]` vs `[7,8,9,10]`), SMP bỏ guard idle
  (`kt29::push_chi_khi_dich_idle...` left 1 right 0), Sync mailbox ghi đè
  (`kt30_sync::mailbox_day...` `Ok(())` vs `Err(Busy)`). Log
  [`JOURNAL/kt31-mutation.log`](JOURNAL/kt31-mutation.log) — 1 lần chạy (bản đầu gọi thiếu
  `--manifest-path` là lỗi cargo, **không** tính).
- **Miri toàn bộ PASS**: `nightly --only-miri` 18 tập, **276 test**, wall **4058,24 s**; K1c 104
  test (kt24 13, kt26 24, kt27 26, kt28 12, kt29 13, sync 16); **không có `kt25_ipc2`** trong
  danh sách (Miri rời ở KT-25 24/24) — ghi rõ, không claim phủ 100%. Log
  [`JOURNAL/kt31-miri-all.log`](JOURNAL/kt31-miri-all.log).
- **Gate cuối PASS 461 test**, L1 720 ms, wall **254,19 s** ⇒ **T-INFRA** (> 60 s, tải máy,
  không nới ngưỡng). Log [`JOURNAL/kt31-gate-fast.log`](JOURNAL/kt31-gate-fast.log).
- **Parity 10k PASS**: `kt11_diff_model_va_sim_10000_seed` 10.000 seed × 24 op, 15,52 s. Log
  [`JOURNAL/kt31-parity-10k.log`](JOURNAL/kt31-parity-10k.log).
- **Bảng DoD K1c**: [`reports/K1C-DOD.md`](reports/K1C-DOD.md) — C1 **đạt một phần** (100k không
  phủ IPC), C2 đạt (model 1 luồng), C3 **6/6 ca** (không đủ 12 mục §7.2), C4 **ĐẠT**; 60/60 mục
  DoD KT-24..30 theo log từng task; §9 liệt kê phần **chưa** phủ (model-only, K2, K3
  D2-56..60, `spec/kernel.toml` chưa có, mutation chưa đủ 12 mục).

**Đề xuất**: KT-31 **REVIEW** — đề xuất Agent 0 lật `DONE` như **"gate run một phần"** (D2-61) và
mở **KT-32** đóng 3 nợ (C1-100k: driver L1 trộn IPC; C3 đủ 12 mục §7.2; Miri thêm `kt25_ipc2`).
**K1c REVIEW — chưa DONE**. Không tự lật QUEUE.

**Commit**: `[KT-31]` trên `main` (hash xem `git log -1`; không ghi hash trong tệp để khỏi lệch sau commit).

**Trạng thái**: **REVIEW** — reviewer-kt31 verdict **DRIFT** (nhãn DoD overclaim: C1-100k, C3 3/12, Miri thiếu kt25) đã sửa nhãn report/STATE; chờ Agent 0 lật (không tự DONE).

## KT-30 — `vifast-sync`: MCS lock + Mailbox MPSC cố định (K1c, D2-53 Q9)

**Vào**: READY (agentctl `next --project v2` trả KT-30) · HEAD `27f9a8d` · `rev-list --count
HEAD..origin/main` = 0 · working tree có thay đổi `viscript/` của tab khác — **không chạm**, chỉ
stage `v2/`.

**Mục tiêu**: crate mới `v2/lib/vifast-sync` (`no_std`, 0 dependency, `unsafe` trong danh sách
đóng D2-17 + Miri sạch): **MCS queue lock** (BKL, FIFO) + **hộp thư MPSC cố định** (7 loại
message, ack theo bộ đếm thế hệ `AtomicU32`, đầy ⇒ `Busy`); kiểm bằng **stress host có biên +
Miri** (D2-53 Q9(a): **không** loom, **không** explorer).

**Không** làm: loom/explorer/dependency mới; thread trong kernel/sim; sửa IPC/Notification/
WaitAddr/SMP sim; `vifast-model`, `spec/`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không
`#[allow]`/`#[ignore]`.

**Quyết định model-only `[TK]`** (ghi trong `docs/k1c-sync.md`):
1. MCS dùng **pool node cố định trong khoá** (`[Node; MAX_WAITERS=16]`, liên kết bằng chỉ số
   slot+1, **không** con trỏ thô) — vì `Guard` trả theo giá trị sẽ **di chuyển** node đã công bố
   cho node trước ⇒ con trỏ treo/UB (đúng rủi ro (a) của KT-30); `Guard` chỉ giữ `usize`.
2. `Ordering::SeqCst` cho `tail/head/next/locked` (K1c không có mục tiêu hiệu năng; hạ bậc để
   sau, có điều kiện kèm test/loom).
3. Ack `[TK]`: `generation` = số `send` Ok; `ack(gen)` = `fetch_max` (không lùi, kẹp về
   `generation`); `pending(gen)` = `min(gen, generation) - acked`; `Busy` **không** tiêu thế hệ.
4. `Mailbox<N>` đòi `N ≥ 2` (seq của slot mã hoá cả `pos+1` và `pos+N`).
5. Hook test (`test_enqueue`/`test_woken`/`test_release`/`Guard::slot`) là **`pub(crate)` +
   `cfg(test)`** — `pub` sẽ cho code ngoài crate mở khoá hộ slot ⇒ hai `Guard` cùng lúc = UB.

**Kết quả**: đỏ trước E0432+E0425 (`kt30-red-compile.log`); `gate --fast` **PASS 461 test**
(L1 750 ms; wall 199,3 s — **T-INFRA** tải máy); Miri **16/16** (13,5 s); stress **4 luồng ×
500 op** ≈2,6 ms (send Ok 1978–1984 + Busy = 2000, `drain` == `send Ok` từng hart, bộ đếm ME =
2000), **200 lần lặp / 0 fail**; FIFO có điều khiển `[B, C]`; `try_lock` tranh chấp không 2 chủ;
pool cạn 16 slot ⇒ `try_lock()==None` (unit test). Verdict `reviewer-kt30` **DRIFT** đã sửa 2/2
(assertion stress `try_lock` sai bất biến; hook test `pub` → `pub(crate)`+`cfg(test)`) + 4 việc
làm luôn (pool cạn, `free_mask` không tràn, doc `acked ≠ delivered` + `release` quay vòng chờ
`next`, `gate_wall` + T-INFRA). **Chờ Agent 0 lật sổ.**

## KT-29 — SMP mô hình tất định 1/2/4/8 + push migration (K1c, D2-53 Q1/Q2)

**Vào**: READY (agentctl `next --project v2` trả KT-29) · HEAD `dd4761e` ([KT-28] DONE) ·
`rev-list --count HEAD..origin/main` = 0 · baseline `gate --fast` PASS **427 test/56 suite**,
L1 **725 ms** (`JOURNAL/kt29-baseline-gate-fast.log`) · working tree có thay đổi `viscript/` của
tab khác — **không chạm**, chỉ stage `v2/`.

**Mục tiêu**: mô hình **N hart tất định** (không thread/QEMU) 1/2/4/8; mỗi hart công bố
`idle`/`cur_prio`; push migration **cuối hàng mức Ready cao nhất**, **≤1 TCB/hart/bước**, chỉ khi
`published.idle`, đúng affinity, hart đích **nhỏ nhất**; **không** work stealing; wake chéo hart
qua `Mailbox::Wake` + ngưỡng `published.cur_prio`; guard luật 3.

**Không** làm: `vifast-sync`/MCS lock/MPSC (KT-30), IPC/Notification/WaitAddr, `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`/
`#[allow]`/`#[ignore]`; không đổi ABI; không thread thật/QEMU.

**Bước**: (1) baseline + đỏ `tests/kt29_smp.rs` (E0432/E0599) + probe ngữ nghĩa cũ; (2)
`Published`/`last_at` (`sched.rs`), `iter` (`mailbox.rs`) + drain FIFO `Migrate`; (3) `with_harts`,
`current_hart`/`set_current_hart`, `publish`/`published` + guard luật 3, `migrate_count`,
`mailbox_len`; (4) `push_step`/`push_migration`/`apply_migrate` + `post_mail` kiểm Migrate; (5)
wake chéo hart + preempt theo `cur_prio`; (6) `check_smp`/`check_harts` + `sched_after_op`/`dispatch`;
(7) test 11 + fuzz 1.000×64×4 + driver `l1_k1c` + `nightly --l1`; (8) doc `k1c-smp.md`, gate,
Miri, JOURNAL, review, commit.

**Kết quả**: gate PASS **442 test/58 suite**, L1 **767 ms**; Miri `kt29_smp` **13/13** (261,03 s)
(243,85 s); `nightly --l1 --seeds 200 --smp 1,2,4,8` PASS (800 lượt); test đỏ
`JOURNAL/kt29-red-compile.log` + `kt29-red-semantics.log`; chi tiết `JOURNAL/kt29-journal.md`.

## KT-28 — WaitOnAddress: bảng băm `(frame, offset)`, Wait đọc giá trị + xếp hàng một lần, `WakeAddr(n)` (K1c, D2-53 Q3/Q4)

**Vào**: READY (agentctl `next --project v2` trả KT-28) · HEAD `8e36f55` ([KT-27] DONE) ·
`rev-list --count HEAD..origin/main` = 0 · baseline `gate --fast` PASS **413 test/55 suite**
([`JOURNAL/kt28-baseline-gate-fast.log`](JOURNAL/kt28-baseline-gate-fast.log)).
**Phạm vi**: `WAITADDR_BUCKETS = 64` hằng **model `[TK]`** (Q3: **không** tạo `spec/kernel.toml`) +
`waitaddr_bucket((frame, offset))`; `address_waiters: [Vec<(frame, offset, tcb)>; 64]`; `wait_addr`
đọc `addr_word` **và** xếp hàng trong **một** hàm, lệch `expected` ⇒ `InvalidArgument` không xếp
hàng, TCB `suspended` ⇒ `IllegalOperation`; **không timeout** (Q4); `wake_addr(frame, offset, n)`
FIFO, trả số đã đánh thức, `n = u32::MAX` = tất cả; `set_addr_word`/`addr_word`; `check_waitaddr`
sau mỗi public op (đúng bucket/TCB sống + đúng trạng thái/không trùng/không TCB chết/suspended);
`unlink_tcb_queues` + sweep nút chết theo **đúng bucket**; API K1b `tcb_wait_address`/`address_waiter`/
`address_waiters_len` giữ nguyên (KT-18/19/23 xanh), `tcb_wait_address` đi qua `wait_addr`.
**Không** làm: timeout/K6, `Cancelled` khi Frame unmap (K2), SMP/push (KT-29), `vifast-sync` (KT-30),
Notification mới; không sửa `spec/`, `vifast-model`, `.scn`, QUEUE, DECISIONS, AGENTS, `viscript/`;
không dependency/`unsafe`/`#[allow]`/`#[ignore]`; không đổi ABI.
**Bước**: (1) baseline; (2) test đỏ `tests/kt28_waitaddr.rs` (E0432/E0599) + probe; (3) bảng băm +
`addr_words`; (4) `wait_addr`/`wake_addr`/`set_addr_word`/`addr_word`; (5) `check_waitaddr` +
`sched_after_op`; (6) `unlink_tcb_queues`/sweep theo bucket; (7) test 12 + fuzz 1.000×64; (8) doc
`v2/docs/k1c-waitaddr.md` + Miri list; (9) gate + Miri + mutation + log; (10) review độc lập → commit.
**Kết quả**: test đỏ [`JOURNAL/kt28-red-compile.log`](JOURNAL/kt28-red-compile.log) (24 lỗi
E0432/E0599) + [`JOURNAL/kt28-red-semantics.log`](JOURNAL/kt28-red-semantics.log); `check` PASS
([`JOURNAL/kt28-check.log`](JOURNAL/kt28-check.log)); `gate --fast` **PASS 427 test/56 suite**, L1
704 ms ([`JOURNAL/kt28-gate-fast.log`](JOURNAL/kt28-gate-fast.log)); Miri **12/12** + lib **2/2**;
đột biến "xếp hàng trước rồi so" bị bắt ([`JOURNAL/kt28-mutation-red.log`](JOURNAL/kt28-mutation-red.log));
doc model-only `[TK]`: [`v2/docs/k1c-waitaddr.md`](../docs/k1c-waitaddr.md); nhật ký đầy đủ:
[`JOURNAL/kt28-journal.md`](JOURNAL/kt28-journal.md).
**Nợ đã ghi (không chặn, reviewer-kt28 APPROVE 2026-11-13)**: (a) `addr_words` **không** được sweep
khi frame chết — chưa có vòng đời Frame (K2 Frame thật, K3 xem lại); (b) bất biến
`check_waitaddr` **một chiều** (`nút ⇒ TcbState`, không kiểm `TcbState ⇒ nút` — cùng lý do
`check_notifications`/ma trận 11×11 KT-23); (c) `wake_addr` **bỏ** `sched_after_op` nếu `wake_step`
lỗi giữa chừng (hiện không xảy ra: mọi nút đều `BlockedOnAddress` theo bất biến); (d) docs §6:
"giữ khoá = một hàm" **chấp nhận cho K1c**, `WaitAddr` trên TCB `suspended` ⇒ `IllegalOperation`
là **nợ K3 conformance**, giữ `InvalidArgument`, `set_addr_word`/`addr_word` là **model API**.
**Commit**: `ff94538` (đã push `origin/main`, 14 tệp: +3.348/−25 — gồm 12 test `kt28_waitaddr`, 2
unit checker, doc `v2/docs/k1c-waitaddr.md`, `v2/bin/v2` Miri list, log `JOURNAL/kt28-*`).
**Trạng thái**: **REVIEW → chờ Agent 0 lật** (reviewer độc lập `reviewer-kt28` verdict **APPROVE**
2026-11-13); **không** tự DONE, **không** lật QUEUE.

## KT-27 — Notification: word 64 + badge, Signal OR, một waiter, Wait/Poll, bind 1:1 (K1c, D2-53 Q6)

**Vào**: READY (agentctl `next --project v2` trả KT-27) · HEAD `e5350f0` ([KT-00] lật KT-26 DONE) ·
baseline `gate --fast` PASS **386 test/53 suite** ([`JOURNAL/kt27-baseline-gate-fast.log`](JOURNAL/kt27-baseline-gate-fast.log)).
**Phạm vi**: `NotificationState { word: u64, badge: u64, waiter, bound_sc }` (`[TK]`); `Signal`
(`Send`/`NBSend` trên cap Notification, badge của cap) OR word + badge, đánh thức **một** waiter
(giao word/badge rồi xoá); `Wait` (`recv`) / `Poll` (`nb_recv`) trên cap Notification; word ≠ 0 ⇒
trả ngay, word = 0 ⇒ `BlockedOnNotification`/`Busy`; **một** waiter (waiter thứ hai `Busy`, khác v1
"đánh thức tất cả"); bound notification với TCB (`tcb_bind_notification` 1:1, unbind) + `SchedContext.Bind`
1:1; `signal` đánh thức TCB bound đang `BlockedOnRecv` (model-only theo K1C-PLAN DoD 5);
`check_notifications` sau mỗi op; `.scn` 21 qua runner (đọc tệp, không sửa).
**Không** làm: WaitAddr (KT-28), SMP/push (KT-29), `vifast-sync` (KT-30), IPC cap/Reply mới;
không sửa `spec/`, `vifast-model`, `.scn`, QUEUE, DECISIONS, AGENTS, `viscript/`; không
dependency/`unsafe`/`#[allow]`/`#[ignore]`; không đổi ABI.
**Bước**: (1) test đỏ `tests/kt27_notification.rs` + `tests/kt27_scn.rs` (E0432/E0599) + probe
ngữ nghĩa; (2) `NotificationState`/`TcbData.notif`/`NotificationValue`; (3) `notification_signal`/
`wait`/`poll`/`word`/`badge`/`bound_sc`; (4) `recv`/`nb_recv`/`send`/`nb_send` nhánh Notification;
(5) bind TCB 1:1 + `SchedContext.Bind` 1:1 + dọn khi cap/SC chết; (6) `check_notifications` +
`block_current` ghi waiter; (7) `scenario.rs` `Step::{Signal, WaitNotification, PollNotification}`;
(8) `kt27_scn.rs` đọc 21; (9) fuzz 1.000 seed × 64 op; (10) gate + Miri + docs + STATE/JOURNAL.

**Commit**: `093b986` (đã push `origin/main`). **Kết quả**: test đỏ [`JOURNAL/kt27-red-compile.log`](JOURNAL/kt27-red-compile.log) (44 lỗi:
1×E0432 + 43×E0599) + [`JOURNAL/kt27-red-semantics.log`](JOURNAL/kt27-red-semantics.log) (probe
tạm); `gate --fast` **PASS 413 test/55 suite**, L1 738 ms ([`JOURNAL/kt27-gate-fast.log`](JOURNAL/kt27-gate-fast.log));
Miri `kt27_notification` **PASS 26/26** ([`JOURNAL/kt27-miri.log`](JOURNAL/kt27-miri.log)); reviewer-kt27 **APPROVE**
(1 yêu cầu `reset_ipc_error` đã sửa + test); fuzz
**1.000 seed × 64 op** bất biến `check_sched`/`check_ipc`/`check_notifications`/`check_caps`/
`check_all_bytes` sau mỗi bước + cùng seed cùng trace; `.scn` 21 chạy qua runner. Chi tiết DoD 1..8
+ 8 quyết định mô hình + giới hạn K3: [`JOURNAL/kt27-journal.md`](JOURNAL/kt27-journal.md), docs
[`../docs/k1c-notification.md`](../docs/k1c-notification.md).

## KT-26 — Fault + Exit + mô hình procsrv (K1c, D2-53 Q5/Q10, D2-25)

**Vào**: READY (agentctl `next --project v2` trả KT-26) · HEAD `20e137c` ([KT-00] lật KT-25 DONE) ·
baseline `gate --fast` PASS **361 test/51 suite** ([`JOURNAL/kt26-baseline-gate-fast.log`](JOURNAL/kt26-baseline-gate-fast.log)).
**Phạm vi**: `FaultKind` 4 biến thể **model-only** (`Exit` + `Vm{fault_addr, access}` +
`UnknownSyscall{nr}` + `UserException{ip, kind}`; **không** sửa `spec/` — spec là **tập con** tới K3,
Q5); fault ≠ Exit đi **Call + Reply** tới `fault_ep`, procsrv Reply **ghi thanh ghi/MR vào TCB
trước khi `Ready`** (chống mất message r319); `tcb_resume_with_regs` model API (ghi regs một cửa
`ipc::transfer`, không phát lại IPC cũ, giữ `exit_code`); `Exit` đúng D2-25 (4 nhánh); mô hình
procsrv `[TK]` `{code, how}` (`recv_exit`/`wait`/`kill` = `Suspend(Abort)` + Revoke cap TCB).
**Không** làm: Notification (KT-27), WaitAddr (KT-28), SMP/push (KT-29), `vifast-sync` (KT-30);
không sửa `spec/`, `vifast-model`, `.scn`, QUEUE, DECISIONS, AGENTS, `viscript/`; không
dependency/`unsafe`/`#[allow]`/`#[ignore]`; không đổi ABI.
**Bước**: (1) test đỏ `tests/kt26_fault.rs` + `tests/kt26_scn.rs` (E0599/API) + probe ngữ nghĩa;
(2) `tcb.rs` 4 FaultKind; (3) `kobj.rs` `inject_fault`/delivery fault Call+Reply/`tcb_resume_with_regs`/
`tcb_last_fault`/`tcb_regs` + nới `check_mailboxes` cho fault-Call; (4) `procsrv_model.rs`; (5)
`scenario.rs` `Step::{Fault, ReplyFault, ResumeWithRegs}`; (6) `kt26_scn.rs` đọc 13/14/20; (7) fuzz
1.000 seed × 64 op; (8) gate + Miri + mutation "resume không ghi MR"; (9) `v2/docs/k1c-fault.md`,
STATE/JOURNAL, review độc lập rồi commit/push.

**Kết quả**: test đỏ [`JOURNAL/kt26-red-compile.log`](JOURNAL/kt26-red-compile.log) (53 lỗi:
51×E0599 + 2×E0432) + [`JOURNAL/kt26-red-semantics.log`](JOURNAL/kt26-red-semantics.log) (probe
tạm); `gate --fast` **PASS 386 test/53 suite**, L1 713 ms ([`JOURNAL/kt26-gate-fast.log`](JOURNAL/kt26-gate-fast.log));
Miri `kt26_fault` **PASS 24/24** ([`JOURNAL/kt26-miri.log`](JOURNAL/kt26-miri.log));
đột biến "resume không ghi MR" ⇒ 3 test resume **đỏ** rồi khôi phục
([`JOURNAL/kt26-mutation-resume.log`](JOURNAL/kt26-mutation-resume.log)); fuzz **1.000 seed × 64 op**
bất biến sau mỗi bước + cùng seed cùng trace; `.scn` 13/14/20 chạy qua runner. Chi tiết DoD 1..8 +
8 quyết định mô hình + verdict **reviewer-kt26 APPROVE** (và 3 yêu cầu bằng chứng đã sửa):
[`JOURNAL/kt26-journal.md`](JOURNAL/kt26-journal.md); docs [`../docs/k1c-fault.md`](../docs/k1c-fault.md).

## KT-25 — IPC core 2: 4 cap + vòng đời Reply + `ReplyRecv`/NB* (K1c, D2-53 Q6/Q8/Q15)

**Vào**: READY (agentctl `next --project v2` trả KT-25) · HEAD `47cea0d` ([KT-00] lật KT-24 DONE) ·
baseline `gate --fast` PASS **336 test/49 suite** ([`JOURNAL/kt25-baseline-gate-fast.log`](JOURNAL/kt25-baseline-gate-fast.log)).
**Phạm vi**: **4 cap** (GRANT, bỏ GRANT đúng, unwrap badge, receive window; `MessageInfo.caps ≤ 4`);
thất bại chuyển cap **mềm** (D2-53 Q6: message vẫn giao, `caps` = số cài được,
`cap_transfer_failed`, **không** thêm bit MessageInfo); vòng đời `Reply` (bị xoá/revoke ⇒ caller
**`Cancelled`**, D2-53 Q8); `ReplyRecv` + `NBSendRecv`/`NBSendWait` (Q15a: NBSend lỗi **vẫn**
Recv/Wait); donation không rò `SchedContext`.
**Không** làm: Fault/Exit (KT-26), Notification word/badge/`Signal` (KT-27), WaitAddr (KT-28),
SMP/push (KT-29), `vifast-sync` (KT-30); không sửa `spec/`, `vifast-model`, `.scn`, QUEUE,
DECISIONS, AGENTS, `viscript/`; không dependency/`unsafe`/`#[allow]`/`#[ignore]`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt25-red-compile.log`](JOURNAL/kt25-red-compile.log) — `tests/kt25_ipc2.rs` **không biên
  dịch**: **34 lỗi** = **31×`E0599`** (`Message::with_cap_ids`; `Machine::{tcb_set_recv_window,
  reply_recv, nb_send_recv, nb_send_wait, cap_transfer_stats, cap_transfer_failed,
  cap_transfer_installed, cap_unwrapped}`; `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}`)
  + 3×`E0308` phụ thuộc API mới.
- [`JOURNAL/kt25-red-semantics.log`](JOURNAL/kt25-red-semantics.log) — probe **tạm (đã xoá)**: (1)
  `NBSend` message mang cap ⇒ `Err(NotSupported)` (cần giao được + cài cap); (2) `CNode.Delete` cap
  Reply khi caller `BlockedOnReply` ⇒ caller **vẫn treo** + `last_ipc_error = None` ⇒ **deadlock**
  (Q8 cần `Cancelled`).

### Sản phẩm

- `v2/sim/vifast-sim/src/ipc.rs` (+~120 dòng): `CapSnapshot`, `CapPlan`, **`plan_cap_transfer`**
  (unwrap badge tới chính Endpoint / thiếu `RIGHT_GRANT` ⇒ mềm / ô đích có cap ⇒ mềm / cài theo
  vị trí), `MessageInfo::{with_caps, with_extra_caps_unwrapped}`, `Message::{with_cap_ids, caps}`.
- `v2/sim/vifast-sim/src/kobj.rs`: `TcbData.recv_window` + `tcb_set_recv_window`/`tcb_recv_window`;
  `resolve_caps`/`prepare_caps`/`deliver_caps`/`deliver_message` (cap chết ⇒ `InvalidCapability`
  **trước** `msg_id`; window chưa bind ⇒ `InvalidArgument` **trước** khi giao; cài cap con **bỏ
  `RIGHT_GRANT`**, giữ badge; `MessageInfo.caps`/`extra_caps_unwrapped`/`badge` cập nhật);
  `QueuedMessage.snaps`; `recv`/`nb_recv` **peek** kiểm window trước khi pop; `reply_recv`,
  `nb_send_recv`, `nb_send_wait`, `wait_notification_obj`; **vòng đời Reply** (`cancel_reply` +
  hook `delete`/`revoke_children`/Untyped-revoke + `purge_dead_refs`); bộ đếm
  `cap_transfer_{installed,failed,unwrapped}`; `nb_recv` = **poll** (TCB không chạy được ⇒ `Busy`).
- `v2/sim/vifast-sim/src/scenario.rs`: `Step::{ReplyRecv, NBSendRecv, NBSendWait, SendCaps}` +
  `Msg::words`.
- `v2/sim/vifast-sim/tests/kt25_ipc2.rs` (**mới, 24 test**) + `tests/kt25_scn.rs` (**mới, 1 test**).
- `v2/docs/k1c-ipc-semantics.md`: §10 (4 cap/receive window), §11 (vòng đời Reply), §12
  (`ReplyRecv`/NB*), §13 (runner `.scn`), §14 (giới hạn còn lại).
- [`JOURNAL/kt25-journal.md`](JOURNAL/kt25-journal.md): bảng đối chiếu DoD 1..10 + 6 quyết định
  mô hình cần reviewer soi.

### Cổng

| Hạng mục | Kết quả | Log |
|---|---|---|
| `gate --fast` | **PASS 361 test/51 suite** (+25), L1 1.000 seed/999 trace 708 ms, clippy `-D warnings` sạch | [`kt25-gate-fast.log`](JOURNAL/kt25-gate-fast.log) |
| Miri `--test kt25_ipc2` | **PASS 24/24**, 75,99 s | [`kt25-miri.log`](JOURNAL/kt25-miri.log) |

### Review độc lập (reviewer-kt25)

Verdict **DRIFT (nhẹ)** — lõi DoD đạt, 3 điểm phải sửa trước commit; **đã sửa cả 3** (xem
`kt25-journal.md` §4 "Reviewer verdict"): (1) Reply cap chết còn trong Recv waiter sau
`CNode.Delete` ⇒ thêm `clear_dead_reply_in_waiters()` cuối `delete` khi cap là `Reply` (giữ nguyên
ngữ nghĩa KT-18 `fault_ep` chết) + test hồi quy + mutation check; (2) comment lỗi thời
`admit_message`; (3) số liệu cây cuối (gate **361 test/51 suite**, Miri **24/24**) — log
`kt25-gate-fast.log` / `kt25-miri.log` ghi đè bản cuối.


## KT-24 — IPC core 1 (K1c, D2-53 Q7/Q11)

**Vào**: READY (agentctl `next --project v2` trả KT-24) · HEAD `4bcdf81` (K1b DONE, K1c mở) ·
baseline `gate --fast` PASS **321 test/47 suite** ([`JOURNAL/kt24-baseline-gate-fast.log`](JOURNAL/kt24-baseline-gate-fast.log)).
**Phạm vi**: `MessageInfo` 35 bit đúng `spec/abi.toml:40-49`; `Message` **64 MR** + IPC buffer model;
**một** hàm `ipc::transfer` copy MR; `Send/NBSend/Recv/NBRecv`; `no_reply` **hai chiều** (D2-53 Q7);
`Call` trên object không phải Endpoint **không** chặn/không Reply, `NotSupported` (Q11).
**Không** làm: 4 cap/GRANT/badge unwrap/`ReplyRecv`/Notification/WaitAddr/Fault (KT-25..KT-28),
SMP/push (KT-29), `vifast-sync` (KT-30); không sửa `spec/`, `vifast-model`, `.scn`, QUEUE,
DECISIONS, AGENTS, `viscript/`; không dependency/`unsafe`/`#[allow]`/`#[ignore]`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt24-red-compile.log`](JOURNAL/kt24-red-compile.log) — `tests/kt24_ipc.rs` **không biên
  dịch**: **56 lỗi** = 51×`E0599` (method `call_msg`/`send`/`nb_send`/`nb_recv`/`check_ipc`/
  `ipc_sent`/`ipc_delivered`/`ipc_pending`/`ipc_dropped`/`transfer_calls`/`tcb_ipc_*`/
  `endpoint_msg_ids` chưa có) + 1×`E0432` (`vifast_sim::ipc`) + 4×`E0433`.
- [`JOURNAL/kt24-red-semantics.log`](JOURNAL/kt24-red-semantics.log) — probe **tạm (đã xoá)** dùng
  API cũ: (1) server `Recv` không Reply cap + client `Call` ⇒ hiện `IllegalOperation` (cần
  `Cancelled` + message vẫn giao); (2) `EndpointState.last_mr` chỉ giữ **một** word, bị message sau
  đè (`Ok(Some(8738))`) ⇒ 64 MR/message chưa biểu diễn được.

### Sản phẩm

- `v2/sim/vifast-sim/src/ipc.rs` (**mới**, 354 dòng): `MessageInfo` (35 bit, `new`/`unpack` kiểm
  biên `label<2^20`, `caps≤4`, `extra≤0xF`, `length≤64`; `pack`/`bits`), `Message` (`info`,
  `mr:[u64;64]`, `caps`, `badge`, `msg_id`), `IpcArea` (**hai field riêng** `regs:[u64;4]` +
  `buffer:[u64;60]`), `transfer()` — đường copy **duy nhất**, xoá vùng nhận trước khi chép.
- `v2/sim/vifast-sim/src/kobj.rs`: `TcbData.ipc` + `last_ipc_error`; `EndpointState.send` thành
  `Mailbox<QueuedMessage>` (`msg: Option<Box<Message>>`, `sender`, `call`, `fault`, `blocking`);
  `call_msg` + `call` (wrapper 1 MR) + `send` (dispatch Endpoint/Reply/Notification) + `nb_send` +
  `recv` (chép qua `transfer`, bind Reply, nhánh thiếu Reply) + `nb_recv`; `exit` dùng
  `QueuedMessage` fault; `check_mailboxes` nới cho `blocking`/`no_reply`; **`check_ipc()`** (msg_id
  đúng một nơi, `transfer_calls == delivered`, `sent == delivered + pending + dropped`) gọi trong
  `sched_after_op` sau **mỗi** public op (đường nhanh khi `ipc_sent == 0` để L1 không chậm);
  `ipc_dropped` cộng khi `Suspend(Abort)`/finalise TCB/Revoke Untyped chứa Endpoint; accessor
  `ipc_*`, `tcb_ipc_*`, `endpoint_msg_ids`.
- `v2/sim/vifast-sim/src/scenario.rs`: `Msg` (MessageInfo + 64 MR) + `Step::{Send, NBSend, NBRecv}`
  (đủ `name`/`actor`/`execute`/`is_tcb_syscall`).
- `v2/sim/vifast-sim/tests/kt24_ipc.rs` (**mới, 13 test**): layout 35 bit + round-trip 1.000 giá
  trị LCG + biên; 64 MR byte-for-byte + `mr[4]` **chỉ** ở buffer; `transfer` xoá dữ liệu cũ;
  `send` chặn/`nb_send` không chặn/FIFO/`nb_recv` Busy; `recv` chặn + giao thẳng; Call/Reply +
  `send` trên Reply; `no_reply` caller (a1/a2) + kernel thiếu Reply (b1/b2) ⇒ `Cancelled`;
  Call non-Endpoint `NotSupported` (CNode/Tcb/Reply/Untyped) + `Send` sai kiểu; hàng đầy `Busy`;
  fuzz **1.000 seed × 64 op** (Miri giảm `cfg(miri)` 3×8), cùng seed cùng trace; bảo toàn
  message không abort (`dropped == 0`, `sent == delivered + pending`).
- `v2/sim/vifast-sim/tests/kt24_scn.rs` (**mới, 2 test**): runner **đọc tệp** `.scn` 04/06/17 và
  chạy đúng chuỗi bước, 0 lệch; scenario `Step::{Send,NBSend,NBRecv}` qua `run_sequential`.
- `v2/docs/k1c-ipc-semantics.md` (**mới**, model-only `[TK]`): bảng ngữ nghĩa, `no_reply` hai chiều,
  quy ước runner `.scn`, bất biến bảo toàn message, giới hạn KT-24.
- `v2/bin/v2`: thêm `kt24_ipc` vào `nightly --only-miri` (kt24_scn đọc tệp nên **không** đưa vào).

### Đo

| Việc | Số | Log |
|---|---|---|
| Gate | `gate --fast` **336 test/49 suite**, 0 failed; **L1 1.000 seed/999 trace 674 ms** (đo bởi cổng) | [`JOURNAL/kt24-gate-fast.log`](JOURNAL/kt24-gate-fast.log) |
| Test mới | **15** = 13 `kt24_ipc` + 2 `kt24_scn` | cùng log |
| Fuzz IPC | **1.000 seed × 64 op**, `check_sched`+`check_mailboxes`+`check_ipc` sau **mỗi** op, 0 vi phạm; cùng seed ⇒ cùng trace | `cargo test -p vifast-sim --test kt24_ipc` |
| `no_reply` | 4 ca: caller bật cờ (có/không waiter) ⇒ `Ok` ngay không Reply; kernel thiếu Reply (giao thẳng/xếp hàng) ⇒ `Cancelled` + message vẫn giao + cờ bật | cùng log |
| Miri | `v2 miri -p vifast-sim --test kt24_ipc` — **PASS 13/13 (37,77 s)**; 2 warning "integer-to-pointer cast" là của `vifast-objmem` (có từ trước, không do KT-24) | [`JOURNAL/kt24-miri.log`](JOURNAL/kt24-miri.log) |

### DoD KT-24 (K1C-PLAN §5) — trung thực

| # | Tiêu chí | Kết quả |
|---|---|---|
| 1 | `MessageInfo` 35 bit đúng layout + round-trip 1.000 + biên `caps>4`/`len>64` | **ĐẠT** (`message_info_dung_layout_35_bit`, `message_info_roundtrip_1000_va_bien`) |
| 2 | `Message` 64 MR + 4 MR thanh ghi/60 MR IPC buffer **hai vùng riêng**, `mr[4]` chỉ ở buffer | **ĐẠT** (`message_64_mr_hai_vung...`, `transfer_mot_duong...`) |
| 3 | `ipc::transfer` là đường copy **duy nhất** (`transfer_calls == ipc_delivered`) | **ĐẠT** — grep chỉ `kobj::deliver_to` gọi `ipc::transfer`; `check_ipc` kiểm sau mỗi op |
| 4 | `Send`/`NBSend`/`Recv`/`NBRecv` + FIFO + không mất message + hàng đầy `Busy` | **ĐẠT** (`send_nb_send_fifo_va_nb_recv_busy`, `hang_gui_day...`) — **ghi rõ**: K1C-PLAN ghi "hàng gửi không giới hạn" nhưng KT-22 DONE đã chốt `MAILBOX_CAP = 8` + `Busy`; KT-24 **giữ nguyên** KT-22 (không nới/hạ), đã ghi trong `v2/docs/k1c-ipc-semantics.md` §4 |
| 5 | `no_reply` **hai chiều** (Q7) + `.scn` 17 | **ĐẠT** (`no_reply_caller_dat_co...`, `no_reply_kernel_thieu_reply_tra_cancelled`, `kt24_scn`) |
| 6 | Call non-Endpoint **không chặn/không Reply**, `NotSupported`, `TcbState` không đổi | **ĐẠT** (`call_non_endpoint_not_supported...`) |
| 7 | Bảo toàn message `check_ipc` sau **mỗi** op; 1.000 seed × 64 op; cùng seed cùng trace | **ĐẠT** — `ipc_sent == ipc_delivered + ipc_pending` khi không abort (thêm `ipc_dropped` cho đường huỷ hợp lệ D2-25) |
| 8 | `scenario.rs` có `Step::{Send,NBSend,NBRecv}` + `Msg`; runner `.scn` 04/06/17 đọc tệp, 0 lệch | **ĐẠT** (`kt24_scn`) |
| 9 | `gate --fast` PASS không retry; Miri `--test kt24_ipc` PASS; 0 unsafe; `spec/`/`vifast-model`/`.scn` không đổi | **ĐẠT** (gate PASS; `git diff --stat spec/ v2/lib/vifast-model v2/test/scn` rỗng) |

### Giới hạn còn lại (KT-25..KT-30)

Chưa chuyển **cap** (`MessageInfo.caps > 0`/`Message.caps` ⇒ `NotSupported` tới KT-25 — không im
lặng bỏ), chưa GRANT/unwrap badge/receive window/`ReplyRecv`/`NBSendRecv`/`NBSendWait`/vòng đời
`Reply` (KT-25); fault ngoài `Exit` (KT-26); Notification signal/wait (KT-27); WaitAddr/WakeAddr
(KT-28); SMP/push (KT-29); `vifast-sync` (KT-30). IPC buffer model chỉ giữ **word** + metadata,
chưa cấp 4 KiB/căn 16 thật (K2/K3).

## KT-23 — Tích hợp scenario, coverage 100%, Miri, parity, chốt DoD K1b (K1b, D2-49..D2-52)

**Vào**: READY (agentctl `next --project v2` trả KT-23) · HEAD `67aa8bc` · KT-18..KT-22 DONE ·
baseline `gate --fast` PASS **311 test/46 suite** ([`JOURNAL/kt23-baseline-gate-fast.log`](JOURNAL/kt23-baseline-gate-fast.log)).
**Phạm vi**: nối các mảnh K1b vào bộ chạy kịch bản + chứng minh 4 tiêu chí K1b bằng log; **không**
IPC 4 cap/badge/`ReplyRecv`/Notification đầy đủ (K1c), không SMP/push migration (**D2-49(a)** —
hoãn K1c), không sửa `spec/`/`vifast-model`; `.scn` chỉ sửa **một dòng** theo **D2-52**; không
`unsafe`/dependency/`#[allow]`/`#[ignore]`; Miri **không** vào `gate --fast`. Working tree có thay
đổi `viscript/` của tab khác — **không chạm**, chỉ stage `v2/`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt23-red-compile.log`](JOURNAL/kt23-red-compile.log) — `tests/kt23_k1b.rs` **không
  biên dịch**: **30 lỗi** = 27×`E0599` (method mới `k1b_report`/`k1b_audit`/`check_budget`/
  `check_sc_refs`/`forbidden_attempt_count`/`design_coverage`, bước kịch bản mới) + 2×`E0425`
  (`tcb::reserved_count`/`tcb::forbidden_count`) + 1×`E0432` (`tcb::coverage_design`).
- [`JOURNAL/kt23-red-interleave.log`](JOURNAL/kt23-red-interleave.log) — probe **tạm (đã xoá)**
  dùng API **đang có**: interleaver KT-09 lọc "actor **không** runnable" ⇒ TCB `Ready` sẵn sàng
  chạy vẫn bị `Deadlock { remaining: 1 }` — đúng lý do DoD 1 phải sửa `run_interleaved`.

### Sản phẩm

- `v2/sim/vifast-sim/src/scenario.rs`: **6 bước mới** — `TcbSuspend`, `TcbResume`,
  `TcbReadState` (tên DoD; đổi từ `ReadState`), `TcbSetSchedParams` (model API D2-49(b), cần để
  dựng cảnh ngân sách), `SchedTick`, `SchedPick` (= `dispatch`: pick + chuyển ngữ cảnh — `pick`
  thuần chỉ đọc nên không tạo event). `Step::dispatchable`: bước syscall (`Call/Recv/Reply/Exit`)
  chỉ chạy khi actor `Running`, hoặc `Ready` **trong ready queue** + `tcb_has_budget`, hoặc
  `Inactive` + budget (kích hoạt ngầm KT-18) — D2-50: không SC/SC chưa `Configure` **không** chạy;
  bước quản trị (suspend/resume/read/set-params/tick/pick/retype/...) luôn chọn được.
  `run_interleaved` chỉ chọn thread có bước kế `dispatchable` ⇒ tôn trọng `Running`/ready
  queue/SC budget; cùng seed ⇒ cùng trace. `kt09_e2e` chỉ đổi tên `Step::ReadState` →
  `Step::TcbReadState` (3 dòng, cùng ngữ nghĩa).
- `v2/sim/vifast-sim/src/kobj.rs`:
  - **Tách bất biến** (giữ nguyên tổng năng lực kiểm): `check_sched` (queue/current/Ready⇔hàng)
    + **`check_sc_refs`** (donation hai chiều, donee không SC riêng/không chuỗi, donor
    `BlockedOnReply`, `effective_priority` ≤ MCP; **mới**: mọi `Tcb.sched_context`/`donated_sc`
    trỏ object **sống đúng kiểu** `SchedContext`, `SchedContext.bound` cap còn sống Reply/Notification)
    + **`check_budget`** (Running ⇒ có budget; mọi SC sống `ScBudget::check`). `check_sched` gọi cả
    hai ⇒ sau **mỗi** public op vẫn kiểm đủ (không nới).
  - **Đếm cặp cấm**: `forbidden_hits[121]` + `forbidden_attempt_count` ở **một cửa**
    `transition_tcb`; `budget_sleeps` (Running→Sleeping{Budget}) + `budget_refills` (biên refill).
  - **`K1bReport` + `k1b_report()` + `k1b_audit()`** (model-only, không ABI): transition
    `hit/allowed` = **46** cặp thiết kế (`coverage_design`, trừ 2 cặp `Stopping` reserved),
    `hit_table/table_allowed` = 48, `reserved` = 2, forbidden `73/73`, queue counters,
    budget ticks/sleeps/refills/charged, và 5 miền `*_violations` (ảnh chụp lúc dựng report).
- `v2/sim/vifast-sim/src/tcb.rs`: `is_reserved`/`reserved_count` (**2**)/`forbidden_count`
  (**73**)/`coverage_design` (**46**) + 1 unit test (reserved không lẫn vào 46/46).
- **Bug do fuzz KT-23 tìm ra + đã sửa** (regression `exit_fault_giao_thang_cho_recv_waiter`):
  `Exit` mang fault vào `fault_ep` khi endpoint **đang có `Recv` waiter** trước đây vẫn đẩy
  message vào hàng ⇒ vỡ bất biến KT-22 "có message ⇒ không `Recv` waiter" (fuzz bắt 44 lần).
  Nay giao **thẳng** cho waiter (message **không** vào hàng), procsrv `Recv` coi như xong ⇒ TCB
  thoát `Inactive` ngay (D2-25); `Exit` từ `Inactive` **giữ** `Inactive` (cặp danh tính bị cấm).
  Không đổi ABI; nhánh **không** waiter (`.scn` 14/20) giữ nguyên `BlockedOnSend{fault}`.
- `v2/sim/vifast-sim/tests/kt23_k1b.rs` (**mới, 9 test**):
  1. ma trận 11×11 qua cổng: **46/46** cặp thiết kế (100%), **48/48** toàn bảng, **73/73** cặp
     cấm bị thử đúng 1 lần + `IllegalOperation` + state không đổi + không đếm hit;
  2. 4 ca cấm viết **từ lý do thiết kế** (không đọc bảng ra kỳ vọng);
  3. interleaved 64 seed: 64 trace khác nhau, cùng seed ⇒ cùng trace, 12 bước/kịch bản;
  4. ca âm **hết budget** (`Sleeping{Budget}` ⇒ bế tắc; có `SchedTick` refill ⇒ chạy);
  5. ca âm **`Ready + suspended`** ⇒ bế tắc; `TcbResume` rồi chạy;
  6. **fuzz 1.000 seed × 64 bước** (LCG**: retype/call/recv/reply/suspend/resume/tick/pick/
     exit/wait-notify/wait-addr/wake/mail/finalise): **256.000** lần kiểm 4 miền
     (`check_sched`/`check_budget`/`check_sc_refs`/`check_all_bytes` + `check_mailboxes`),
     **0 vi phạm**; ops coverage **45/46** (thiếu `Stopping→Dead` — K1b không có nguồn nhập
     `Stopping`; 46/46 chứng minh ở test ma trận, không hạ tiêu chí); `forbidden_attempts`
     (thao tác hợp lệ) = 0;
  7. **Exit `Busy` snapshot**: `exit_code`=None, `TcbInfo`/`ScInfo`/donation hai chiều/
     `Reply.caller`/`endpoint len-pushed-popped-removed` + danh sách message **không đổi**;
  8. regression exit-fault-waiter (giao thẳng cho `Recv` waiter, `len == 0`, `Inactive`);
  9. chốt 2 cặp `Stopping` reserved / 48 allowed / 73 cấm.
- `v2/bin/v2`: `nightly --only-miri` thêm **kt18_tcb, kt19_sched, kt20_budget, kt21_donation,
  kt22_mailbox** (mỗi tệp tự giảm seed bằng `cfg(miri)`); **không** thêm `kt23_k1b` (1.000 seed —
  không có I/O cần Miri) và **không** thêm test I/O.
- `v2/test/scn/12-suspend-abort-while-reply-wait.scn`: **đúng một dòng** `Defer => ok` →
  `Defer => InvalidArgument` (**D2-52**); không sửa `.scn` khác.
- `v2/docs/k1b-tcb-transition.md`: đóng cảnh báo fixture `Defer` + ghi bằng chứng phủ 121 ô.

### Đo (K1b)

| Việc | Số | Log |
|---|---|---|
| Coverage cặp thiết kế | **46/46 (100%)**; toàn bảng 48/48; cặp cấm **73/73** thử, `forbidden_attempts = 73` | `cargo test -p vifast-sim --test kt23_k1b` → `# KT-23 ma trận: ...` |
| Fuzz 1.000 seed × 64 bước | 256.000 kiểm 4 miền, **0 vi phạm**; ops coverage 45/46 | cùng log → `# KT-23 fuzz: ...` |
| Budget | `budget_sleeps = 542`, `refills = 7056`, `ticks = 12883`, **0** vi phạm | cùng log |
| Interleaver | 64 seed/64 trace khác nhau, 2 ca âm đúng (hết budget, suspended) | cùng log |
| Exit `Busy` | `exit_code=None`, donation/`Reply.caller`/hàng đợi không đổi | cùng log |
| Gate | `gate --fast` **321 test/47 suite** (+10 so KT-22: 9 `kt23_k1b` + 1 unit `tcb`), L1 1.000 seed/999 trace 687 ms | `JOURNAL/kt23-gate-fast.log` |
| Miri | `nightly --only-miri`: **12/12 PASS** (2.608 giây, ~43 phút) — objmem + sim 11 mục, thêm 5 test K1b (không thêm `kt23_k1b`, không test I/O) | `JOURNAL/kt23-miri-only.log` |
| Parity (Q7) | `kt11_diff_sim` 10.000 seed / 0 lệch — **chỉ** Untyped/CNode, **không** phải oracle TCB/scheduler | `JOURNAL/kt23-parity.log` |
| `.scn` | **21 tệp / 11 nhóm**, 0 blocked, fixtures 5 ca âm PASS; fixture 12 sửa 1 dòng (D2-52) | `JOURNAL/kt23-scn.log` |

### K1b DoD (4 dòng — K1B-PLAN §1.1/§5 + D2-49(a)) — trung thực, không tô hồng

| # | Tiêu chí K1b | Kết quả | Bằng chứng |
|---|---|---|---|
| 1 | **TCB + transition + `suspended`**: bảng `ALLOWED` 11×11 một cửa, **100%** cặp ALLOWED, mọi cặp cấm `IllegalOperation` | **ĐẠT** — 46/46 cặp thiết kế (48/48 toàn bảng), 73/73 cặp cấm bị thử | `kt23_k1b::ma_tran_46_cap_va_73_cap_cam` + `kt18_tcb` 18 test |
| 2 | **Scheduler 1 hart + budget (chu kỳ thẳng hàng)**: ready queue 256 mức/bitmap, `pick` O(1), preempt mọi bước, affinity, không đói; **0 vi phạm budget** | **ĐẠT** — 1.000 seed × 64 bước, 0 vi phạm; `budget_sleeps=542`/`refills=7056`/`ticks=12883`; `Running ⇒ còn budget` kiểm sau mỗi bước | `kt23_k1b::fuzz_1000_seed_64_buoc_k1b` + `kt19_sched`/`kt20_budget` |
| 3 | **Hộp thư**: hàng cố định, đầy ⇒ backpressure `Busy`, thứ tự tất định | **ĐẠT** — KT-22 (10 test, `MAILBOX_CAP=8`, `pushed == popped + removed + len`) + KT-23 fuzz `mail_pushed=1742` sau mỗi bước; **bug exit-fault-waiter** fuzz tìm ra đã sửa | `kt22_mailbox` + `kt23_k1b::exit_fault_giao_thang_cho_recv_waiter` + `kt23_k1b::exit_busy_...` |
| 4 | **Push migration** (tiêu chí gốc thiết kế `:874`) | **CHƯA ĐẠT trong K1b** — **hoãn K1c** theo **D2-49(a)**: K1b 1 hart, K1c mới có `smp 1/2/4/8`; `Migrate` bị từ chối rõ `NotSupported`, không im lặng bỏ | D2-49(a) + `kt22_mailbox::migrate_tren_1_hart_bi_tu_choi_not_supported` |

### Giới hạn còn lại (K1c)

- **Không** push migration/2 hart (D2-49(a)); **không** MPSC lock-free thật/loom/stress
  (D2-49(f)); IPC 4 cap/badge/`ReplyRecv`/Notification signal-wait/WaitAddr đầy đủ, `Send` thường
  (nới `check_mailboxes` đang mã hoá `call == fault.is_none()`), fault ngoài `Exit` — K1c.
- **Parity `kt11_diff_sim` chỉ phủ Untyped/CNode** (Q7): là **hồi quy**, **không** phải oracle
  cho TCB/scheduler/MCS; bằng chứng K1b là bất biến + coverage + `check_*` sau mỗi bước.
- 2 cặp `Stopping` của bảng để K1c dùng (K1b **không** có nguồn nhập `Stopping`); cặp
  `Stopping→Dead` chỉ đi được qua cổng trực tiếp (ma trận), không qua thao tác nào.
- `check_sc_refs` chưa kiểm "một SC một TCB" (KT-20 cho phép chia SC; D2-49 ghi model API);
  binding ABI `Tcb.SetSchedParams` để K1c/K3.

## KT-22 — Hộp thư FIFO cố định: backpressure `Busy`, nhiều endpoint, 1 hart (K1b, D2-49(a,f), D2-50)

**Vào**: READY (agentctl `next --project v2` trả KT-22) · HEAD `cde7fcc` · KT-18..KT-21 DONE · baseline
`gate --fast` PASS **298 test/45 suite** ([`JOURNAL/kt22-baseline-gate-fast.log`](JOURNAL/kt22-baseline-gate-fast.log)).
**Phạm vi**: **chỉ mô hình tất định** (D2-49(f)) — không atomic/thread/loom/stress; **1 hart**
(D2-49(a): push migration hoãn K1c ⇒ `Migrate` trả `NotSupported`); không IPC 4 cap/badge/`ReplyRecv`
(K1c), không Notification đầy đủ; không `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS,
`viscript/`; không dependency/`unsafe`/`#[allow]`/`#[ignore]`. Working tree có thay đổi `viscript/`
của tab khác — **không chạm**, chỉ stage `v2/`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt22-red-compile.log`](JOURNAL/kt22-red-compile.log) — `tests/kt22_mailbox.rs` **không
  biên dịch**: **40 lỗi** (39 × `E0599` cho `post_mail`/`process_mail`/`send_mail`/`mail_len`/
  `mail_log`/`hart_stopped`/`endpoint_info`/`endpoint_send_items`, 2 × `E0432` cho
  `vifast_sim::mailbox` + `EndpointInfo`).
- [`JOURNAL/kt22-red-unbounded.log`](JOURNAL/kt22-red-unbounded.log) — probe **tạm (đã xoá)** dùng
  API cũ: 9 `Call` cùng một endpoint đều `Ok`, `endpoint_send_len == 9 > 8` ⇒ hàng gửi `VecDeque`
  **không** có sức chứa, **không** backpressure.

### Sản phẩm

- `v2/sim/vifast-sim/src/mailbox.rs` (**mới**): `MAILBOX_CAP = 8` `[TK]`; `Mailbox<T>` FIFO cố định
  — `try_push` đầy ⇒ **trả lại** message (không ghi đè/panic/mất), `pop_front`, `retain` (cộng
  `removed`), bộ đếm `pushed/popped/removed` + bất biến `pushed == popped + removed + len`;
  `MailboxMsg { Wake, Reschedule, Migrate, Stop }` (§4.17, phần K1b); `HartMailbox` (hàng + nhật ký
  message **đã xử lý** + cờ `stopped`); 3 unit test.
- `v2/sim/vifast-sim/src/kobj.rs`:
  - `EndpointState.send: VecDeque` → `Mailbox<(ObjId, Option<FaultKind>)>` **cố định 8**; `call` đầy
    (và không có server `Recv`) ⇒ `KError::Busy` **trước** khi ghi `last_mr`/đổi trạng thái;
    `exit` có `fault_ep` đầy ⇒ `Busy` **trước** mọi thay đổi (không `exit_code`, không trả donation);
    `unlink_tcb_queues` dùng `retain` (đếm `removed`), `purge_dead_refs` giữ nguyên nghĩa.
  - Model API `[TK]`: `endpoint_info` + `EndpointInfo {capacity,len,pushed,popped,removed,recv_waiter}`,
    `endpoint_send_items` (thứ tự FIFO), `post_mail` (Busy/NotSupported), `process_mail` (drain FIFO
    áp dụng đúng một lần, log theo thứ tự), `send_mail` (= post + process **một** bước — DoD 6),
    `mail_len`, `mail_log`, `hart_stopped`.
  - `check_mailboxes` (bất biến `pushed/popped/removed/len`, `len ≤ cap`, "có message ⇒ không Recv
    waiter", mọi sender ở đúng `BlockedOnSend{ep, call/fault}`) chạy trong `sched_after_op` — sau
    **mỗi** public op, cùng chỗ `check_sched`.
  - `wake` tách `wake_step` (không tự kiểm bất biến) để `process_mail` chỉ kiểm một lần/bước.
- `v2/sim/vifast-sim/src/lib.rs`: `pub mod mailbox`.
- `v2/sim/vifast-sim/tests/kt22_mailbox.rs` (**mới**, 10 test): FIFO + đầy trả lại message; backpressure
  1.000 lần; `retain` đếm `removed`; `Migrate` ⇒ `NotSupported` trên 1 hart; hộp thư hart FIFO +
  đầy `Busy` + `Wake` đánh thức + `Stop` đặt cờ + `send_mail` không lộ trạng thái giữa; endpoint
  đầy ⇒ `Call` `Busy` (TCB vẫn `Ready`) rồi `Recv` giải phóng; nhiều endpoint độc lập; `Exit` fault
  đầy ⇒ `Busy` không đổi gì, có chỗ ⇒ fault vào **cuối** hàng; FIFO bắc cầu Call/fault Exit; invariant
  **1.000 seed × 64 op** (stable; Miri 3 seed × 8 op bằng `cfg(miri)`) — không mất/thêm message,
  `check_sched` xanh sau mỗi op, cùng seed cùng trace.

### Review độc lập (verdict APPROVE — `reviewer-kt22`)

Reviewer tự chạy `kt22_mailbox` **10/10** + `gate --fast` **311 test/46 suite** (L1 1000/999), xác
nhận: `Exit` `Busy` không ghi một phần; bất biến `pushed == popped + removed + len` phủ mọi đường
gỡ; `process_mail` không lấn K1c; scope sạch. Ba điểm nhỏ đã sửa trước commit: (1) xoá re-export
không dùng ở `kobj.rs`; (2) ghi rõ `Busy` của `Call` không hoàn nguyên `ensure_started` (kích hoạt
ngầm pre-existing); (3) ghi chú **K1c**: `check_mailboxes` đang mã hoá `call == fault.is_none()`, sẽ
gãy khi có `Send` thường. Ghi chú **KT-23**: ca `Exit` `Busy` nên snapshot thêm donation/SC +
`Reply.caller`.

### Giới hạn mô hình (ghi rõ, để K1c)

- **Không** phải MPSC lock-free thật: 1 luồng, không `std::sync`/atomic/loom/stress (D2-49(f));
  ack thế hệ chưa có — bất biến chỉ dựa bộ đếm `pushed/popped/removed`.
- **Không** push migration/2 hart (D2-49(a)): `Migrate` bị từ chối ở **cạnh gửi** (`NotSupported`),
  không im lặng bỏ; `Stop` chỉ đặt cờ mô hình (chưa có quy trình dừng hart).
- Message `TlbFlush`/`VmKick`/`PowerReq` (cần TLB/VCpu/Power) để K1c.
- Hàng gửi Endpoint **không** phân biệt 4 cap/badge/`ReplyRecv` — K1c.

## KT-21 — MCS donation: passive server, `Call`/`Reply`, MCP (K1b, D2-49(c), D2-50, D2-16)

**Vào**: READY (agentctl `next --project v2` trả KT-21) · HEAD `d547bbf` · KT-18/19/20 DONE · baseline
`gate --fast` PASS 284 test/44 suite ([`JOURNAL/kt21-baseline-gate-fast.log`](JOURNAL/kt21-baseline-gate-fast.log)).
**Phạm vi**: donation của **mô hình sim**; **không** IPC 4 cap/badge/`ReplyRecv` (K1c), hộp thư
(KT-22), SMP/push migration, Notification đầy đủ, refill sporadic/timeout fault (sau K6 — D2-16);
không `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`;
không `#[allow]`/`#[ignore]`. Working tree có thay đổi `viscript/` của tab khác — **không chạm**, chỉ
stage `v2/`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt21-red-compile.log`](JOURNAL/kt21-red-compile.log) — `tests/kt21_donation.rs` **không
  biên dịch**: **42 × `E0599`** cho API chưa có (`sc_of`, `sched_donated_to`, `sched_donor_of`,
  `donated_sc`, `effective_priority`) + **5 × `E0609`** (`ScInfo.donated_to/donor`,
  `TcbInfo.donated_sc`).
- [`JOURNAL/kt21-red-charge.log`](JOURNAL/kt21-red-charge.log) — tệp dò **tạm (không commit)**, dùng
  API **đang có**: sau `Call`, server passive `Ready` nhưng `tcb_has_budget == false`,
  `sched_pick() == None`, `dispatch == None` ⇒ `Call` **không bao giờ được phục vụ**; `2 tick` sau,
  `SC client consumed == 0` (kỳ vọng 2) ⇒ đúng 2 lỗi KT-21 (server không chạy + client không bị trừ).
  Tệp dò đã xoá sau khi lấy log.

### Sản phẩm

- `v2/sim/vifast-sim/src/mcs.rs`: `ScInfo` thêm `donated_to`/`donor` (model-only `[TK]`);
  `donated_priority(donor_priority, donor_mcp, donee_mcp) = min(...)` + **1 unit test** (kẹp MCP,
  quét 7×5×4 tổ hợp bất biến `p_eff ≤ cả ba`).
- `v2/sim/vifast-sim/src/kobj.rs`:
  - `TcbData.donated_sc: Option<ObjId>`, `SchedData.donated_to/donor: Option<ObjId>` — liên kết
    **hai chiều** (tra O(1), kiểm bất biến); `TcbInfo.donated_sc`.
  - Model API `[TK]`: `sc_of`, `donated_sc`, `sched_donated_to`, `sched_donor_of`,
    `effective_priority`.
  - **Donation tự động** đúng hai chỗ giao message của `Call`: `call` (server đang `Recv`) và
    `recv` (server lấy `Call` đang xếp hàng); `reply` trả SC; `donate_sc` chỉ nhận donor có SC
    **đã `Configure`**, SC **không** bị donate, **không** gắn cho TCB khác ("một SC một TCB" khi
    donate — KT-20 chưa cấm chia SC), donee **không** có SC riêng (D2-49(c)), donor không là donee
    (không chuỗi >1). **Không** từ chối `Call/Recv` vì thiếu SC (giữ KT-20 DoD 4).
  - `charge_tick`/`process_refills` dùng `funding_sc` (SC riêng **hoặc** SC được donate) ⇒ thời
    gian server chạy trừ đúng ngân sách client, hết budget ⇒ **server** `Sleeping{Budget}`, refill
    đánh thức server.
  - `sched_sync`/`check_sched` dùng **priority hiệu lực**; `check_sched` thêm bất biến donation
    (hai chiều khớp, một SC ↔ một donee, donee không SC riêng/không donor, donor
    `BlockedOnReply`, donor ≠ donee, không cùng `Running`, `p_eff ≤ MCP` cả hai bên).
  - Huỷ donation: `transition_tcb` (rời `BlockedOnReply` bằng đường khác `Reply`), `tcb_suspend`,
    `exit`, `finalise_tcb` → `release_donation_of` (donee ⇒ trả SC + đánh thức donor qua
    `cancel_reply_of`; donor ⇒ thu hồi SC, donee `Running` ⇒ `Sleeping{Budget}` ngay);
    `set_sched_params`/`tcb_set_priority` **từ chối** đổi SC/priority của donor đang donate
    (`IllegalOperation`/`Busy`); `purge_dead_refs` dọn liên kết khi một bên chết.
- `v2/sim/vifast-sim/tests/kt21_donation.rs` (**mới, 13 test**): server passive chỉ chạy khi được
  donate (trước `Call` `pick` không chọn dù `Ready`); charge vào SC client/`Sleeping{Budget}` +
  refill + `Reply` không mất; server có SC riêng giữ SC riêng; MCP 3 ca (kế thừa/kẹp 40/không nâng
  20); suspend/exit/**finalise** server; abort/finalise client; `SchedContext.Bind` không tạo
  donation; donation cần SC đã `Configure` + các ca `set_sched_params` bị từ chối; **SC dùng chung
  >1 TCB ⇒ không donate**; **không chuỗi 2 tầng qua guard `donated_sc`** (donee `Call` vào endpoint
  có server `Recv`); **`Tcb.SetPriority` từ chối donor, chấp nhận donee đổi MCP** (priority hiệu
  lực/bucket đổi theo); **Reply nhầm Reply object ⇒ không trả donation** (cảnh báo giới hạn K1c,
  [`JOURNAL/kt21-reply-owner.md`](JOURNAL/kt21-reply-owner.md)); **invariant 1.000 seed × 64 op**
  (stable; `#[cfg(miri)]` 3×8) — mỗi bước `check_sched`, không TCB `Running` nào thiếu SC cấp/thiếu
  budget, donation hai chiều, **14.609 bước có donation** / 64.000 bước (ngưỡng test `≥ 12.000`,
  log tái lập [`JOURNAL/kt21-invariant-donation.log`](JOURNAL/kt21-invariant-donation.log)), cùng
  seed cùng trace.

### Quyết định đã chốt (model-only `[TK]`, ghi rõ để review)

- **Donation tự động, không chuỗi >1** (D2-49(c); K1B-PLAN §5 KT-21 rủi ro (a)); server có SC
  riêng **giữ** SC riêng, không nhận donation.
- **Priority khi donate** `p_eff = min(client.priority, client.mcp, server.mcp)`. KT-18 buộc
  `priority ≤ mcp` cho **mọi** TCB (`kobj.rs:1397`) nên chặn thực tế là **MCP của server**: client
  mức cao hơn MCP server ⇒ **kẹp** xuống `server.mcp` (không chạy vượt MCP); client mức thấp hơn ⇒
  server chạy thấp hơn priority gốc (không được nâng). Priority **gốc** của server không đổi — chỉ
  bucket ready queue/`cur_prio` dùng `p_eff`.
- **Huỷ donation** khi server suspend/exit/finalise ⇒ SC về client + client được **đánh thức**
  (`Reply.caller = None`; mã `Cancelled` đầy đủ thuộc K1c); khi client suspend/exit/finalise ⇒ thu
  hồi SC, server mất ngân sách ⇒ `Sleeping{Budget}` (D2-50).
- **"Một SC một TCB" khi donate**: SC đang gắn cho >1 TCB (KT-20 chưa cấm chia SC) **không** được
  donate — nếu donate, TCB kia đang `Running` sẽ mất ngân sách giữa chừng. SC dùng chung vẫn chạy
  bình thường theo KT-20.
- **K1b không mô hình hoá** server phục vụ nhiều client cùng lúc / `ReplyRecv` / badge — K1c.
- **`Reply` trên Reply object không kiểm chủ sở hữu cap** (model thiếu thông tin): `reply` **chỉ**
  trả donation khi `caller` đúng là **donor**; Reply nhầm ⇒ giữ donation, không đánh thức nhầm
  (test `reply_nham_reply_object_khong_tra_donation`; chi tiết + follow-up K1c:
  [`JOURNAL/kt21-reply-owner.md`](JOURNAL/kt21-reply-owner.md)).
- **Đề xuất (chưa thành D2)**: giữ cách **kẹp** `p_eff = min(client.priority, client.mcp,
  server.mcp)` cho K1b, **model-only `[TK]`**; **K1c** chốt **strict reject** nếu cần. **D2-51
  chưa tồn tại** — đề nghị Agent 0 ghi khi lật sổ; KT-21 **không** tự tạo quyết định.

### Cổng + Miri (không retry — D2-07)

- [`JOURNAL/kt21-gate-fast.log`](JOURNAL/kt21-gate-fast.log) — `gate --fast` **PASS 298 test / 0
  failed** (**45 suite**; KT-20: 284/44 ⇒ **+14 test** = 13 integration + 1 unit `mcs`), **L1 1.000
  seed/999 trace** (550 ms), gen-check ON, `.scn` 21 tệp OK, **không retry**; `kt21_donation` native
  ~4,5 s.
- [`JOURNAL/kt21-miri.log`](JOURNAL/kt21-miri.log) — `v2 miri -p vifast-sim --test kt21_donation`
  **PASS 13/13** (0 failed, 0 ignored, **128,57 s**); 2 cảnh báo `integer-to-pointer cast` **có
  sẵn** từ objmem (`v2/lib/vifast-objmem/src/obj.rs`, D2-14/D2-17), không phát sinh từ KT-21;
  `v2/sim/**` **0 unsafe**. Invariant test dưới Miri giảm còn 3 seed × 8 bước (`#[cfg(miri)]`;
  stable giữ **1.000 × 64**) — không `#[ignore]`, không nới `gate --fast`/L1.
- [`JOURNAL/kt21-invariant-donation.log`](JOURNAL/kt21-invariant-donation.log) — số bước có
  donation in bằng `--nocapture`: **14.609 / 64.000** bước; test assert `≥ 12.000` (stable),
  Miri `> 0`.

### Phạm vi / không chạm

Chỉ `v2/sim/vifast-sim/src/{mcs.rs,kobj.rs}`, `v2/sim/vifast-sim/tests/kt21_donation.rs` (mới),
`v2/agent/{STATE.md,JOURNAL/}`. Không sửa `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS,
`viscript/`; không dependency/`unsafe`; không `#[allow]`/`#[ignore]`.

### Review độc lập + đề xuất

Review độc lập: **lead spawn** (`reviewer-kt21`) vì depth 1/1 — **APPROVE phần mã** (reviewer tự
chạy gate 294/45, L1 1000/999, `kt21_donation` 9/9) kèm **3 điều kiện đã xử lý** ở commit follow-up:
(1) thêm 3 test thật (SC dùng chung >1 TCB ⇒ không donate; guard chuỗi 2 tầng qua `dinfo.donated_sc`;
`tcb_set_priority` từ chối donor) — **thêm cả** test Reply nhầm Reply object; (2) sửa STATE
"D2-51 chưa tồn tại — đề xuất clamp, chờ Agent 0 ghi"; (3) `reply` chỉ trả donation khi
`caller == donor` + test cảnh báo + [`JOURNAL/kt21-reply-owner.md`](JOURNAL/kt21-reply-owner.md);
(4) in/assert số bước donation ra log. **Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (không tự
DONE, không lật QUEUE — D2-37). **MCP (đề xuất, chưa thành D2)**: cách hiểu **MCP khi donate** =
**kẹp** `p_eff = min(client.priority, client.mcp, server.mcp)` cho K1b (**model-only `[TK]`**; KT-18
buộc `priority ≤ mcp` nên chặn thực tế là MCP **server**); K1c chốt **strict reject** nếu cần.
**D2-51 chưa tồn tại** — đề nghị Agent 0 ghi khi lật sổ. **YC/Follow-up**: (1) D2-51 (Agent 0 ghi
khi lật sổ, không phải YC mới); (2) KT-22 hộp thư; (3) KT-23 thêm `kt21_donation` vào
`nightly --only-miri` (129 s).

## KT-20 — MCS budget: timer ảo `tick`, consumed/refill, `Sleeping{Budget}` (K1b, D2-49(b,e,h))

**Vào**: READY (agentctl `next --project v2` trả KT-20) · HEAD `54af1a8` · KT-18/19 DONE · baseline
`gate --fast` PASS 263 test/43 suite ([`JOURNAL/kt20-baseline-gate-fast.log`](JOURNAL/kt20-baseline-gate-fast.log)).
**Phạm vi**: ngân sách MCS của **mô hình sim**; **không** donation (KT-21), hộp thư/push migration
(KT-22/K1c), refill sporadic/timeout fault (**hoãn sau K6** — D2-16, D2-49(e)); không `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`; không
`#[allow]`/`#[ignore]`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt20-red-compile.log`](JOURNAL/kt20-red-compile.log) — `tests/kt20_budget.rs` **không biên dịch**:
  **58 × `E0599`** cho API chưa có (`tick`, `virtual_now`, `sched_info`, `sched_consumed`,
  `sched_remaining`, `tcb_has_budget`) + **`E0432`** (`ScInfo`) + `E0308`.
- [`JOURNAL/kt20-red-mutation-bo-charge.log`](JOURNAL/kt20-red-mutation-bo-charge.log) — đột biến tay
  **bỏ dòng `consumed += 1`** trong `ScBudget::charge` (không commit): test invariant **1.000 seed**
  đỏ đúng lý do (`SC 0 cửa sổ 0: 1+1 > budget 1`) ⇒ bộ kiểm bắt được "quên trừ budget" (DoD 10).

### Sản phẩm

- `v2/sim/vifast-sim/src/mcs.rs` (**mới**): `ScBudget { budget, period, consumed, refill, charged }`
  + `ScInfo`; `configure(now, budget, period)` đặt `refill` = **biên thẳng hàng toàn cục kế tiếp**
  (`next_boundary`, bão hoà `u64`, không wrap); `charge()`, `refill_due(now)`, `has_budget()`,
  `remaining()`, `check(now)`; **8 unit test** (biên toàn cục, overflow ở trần, configure, charge
  đúng biên, refill nhảy nhiều biên, trần `u64` không lặp, `check` bắt budget/period/consumed sai,
  **SC chưa cấu hình ⇒ không có budget**).
- `v2/sim/vifast-sim/src/sched.rs`: `HartSched::pick_where(ok)` — `pick` có điều kiện, vẫn mức cao
  nhất + FIFO, O(1) khi phần tử đầu hợp lệ; **+1 unit test**.
- `v2/sim/vifast-sim/src/kobj.rs`: `SchedData.budget: ScBudget` (thay `budget/period` thô);
  `Machine.now: u64`; `tick(n)` (charge TCB đang `Running` cho `[now, now+1)` → `now += 1` → xử lý
  biên refill: **charge trước, refill sau**), `charge_tick` (hết budget ⇒ `Running → Sleeping{Budget}`
  qua **một cửa** `transition_tcb`), `process_refills` (refill ⇒ `Sleeping{Budget} + !suspended →
  Ready` + enqueue), `enforce_budget` (gắn/gỡ SC khiến TCB `Running` không còn budget ⇒ ngủ ngay);
  `SchedControl.Configure` đặt `consumed = 0` + `refill` biên mới (giữ kiểm `budget > 0`,
  `period > 0`, `budget ≤ period`); model API `virtual_now/tick/sched_info/sched_consumed/
  sched_remaining/tcb_has_budget`; `sched_pick_on` lọc budget; `check_sched` thêm bất biến ngân
  sách (mọi SC sống: `budget`/`period` dương, `budget ≤ period`, `consumed ≤ budget`,
  `refill > now`; `Running` ⇒ còn budget).
- `v2/sim/vifast-sim/tests/kt20_budget.rs` (**mới, 12 test**): tick tất định cùng seed cùng mốc
  refill; Configure 0/oversize bị từ chối, không đổi trạng thái; charge **chỉ** khi `Running` qua
  đường IPC thật (Call/Recv/Reply); hết budget ⇒ `Sleeping{Budget}` đúng biên (không `budget + 1`),
  gỡ khỏi ready queue, timer biên kế; không charge quá budget; refill ⇒ `Ready`, `+suspended` giữ
  `Sleeping{Budget}`, `Resume` không phát lại; `pick`/`dispatch` bỏ qua TCB `Ready` hết budget rồi
  nhận lại sau refill; **không SC / SC chưa `Configure` ⇒ không có budget ⇒ không chạy**, sau
  `Configure` mới chạy, gỡ SC khi `Running` ⇒ ngủ ngay; yield không mất budget/thời gian; gắn SC cạn
  cho TCB `Running` ⇒ ngủ ngay + refill đánh thức nhiều TCB; gỡ SC khi `Sleeping{Budget}` không còn
  biên refill; invariant **1.000 seed × 64 bước ngẫu nhiên** với cửa sổ thẳng hàng.

### Quyết định đã chốt (model-only `[TK]`, ghi rõ)

- **Chu kỳ thẳng hàng toàn cục** (D2-49(e)): `refill` = bội số kế tiếp của `period`; **không** cửa
  sổ trượt/sporadic refill/timeout fault (D2-16 — hoãn sau K6).
- **TCB không SC / SC chưa `SchedControl.Configure` = KHÔNG có budget ⇒ `pick`/`dispatch` không
  chọn** (ngữ nghĩa §4.5 + K1B-PLAN §5 KT-20 DoD 4). TCB muốn chạy phải có SC **đã `Configure`**;
  passive server chạy bằng donation ở KT-21. **Đây là sửa theo verdict `reviewer-kt20` (DRIFT mục
  1)**, không phải "không giới hạn"; Agent 0 sẽ ghi **D2-50** khi lật sổ.
- **Charge trước, refill sau**; cấu hình lại giữa chu kỳ reset `consumed` + tính lại biên (quy ước
  5 trong `mcs.rs`); không dồn budget.

### Sửa theo review `reviewer-kt20` (DRIFT → đã sửa, không UNSAFE)

- **Mục 1 — no-SC ≠ unlimited (đã sửa đúng ngữ nghĩa)**: `ScBudget::has_budget` =
  `is_configured() && consumed < budget`; `ScBudget::remaining` = 0 khi chưa cấu hình;
  `Machine::tcb_has_budget` = **false** khi không SC hoặc SC chưa `Configure` ⇒ `sched_pick_on`
  (và `dispatch_step`) bỏ qua; `enforce_budget` đưa TCB `Running` về `Sleeping{Budget}` khi gỡ SC /
  gắn SC cạn / SC chưa `Configure`; `charge_tick` cũng đưa `Running` không SC/chưa cấu hình về
  `Sleeping{Budget}`. **Không** đổi `call/recv/reply`.
- **Rig test cập nhật (test-only, không đổi kỳ vọng cũ)**: `kt19_sched.rs` — `Rig` thêm
  `SchedControl` (slot 6) + `SchedControl.Configure` SC slot 4 với `SC_MAC_DINH = 64` rồi gắn cho 3
  TCB qua helper `gan_sc_mac_dinh`; `pick_du_256` thêm SchedControl/SchedContext (slot 256/257) và
  gắn cho **256** TCB; TCB "D" của test dispatch và 3 TCB của `RandRig` (1.000 seed) cũng được gắn.
  KT-18 không cần (không `dispatch`; chuyển trạng thái trực tiếp là model API, suite vẫn xanh).
- **Mục 2 — Miri**: reviewer đọc `kt20-miri.log` **giữa lúc tệp đang ghi** nên thấy 11/12 và chưa có
  `test result`; log hoàn chỉnh của lần chạy trước đã có `test result: ok. 12 passed`. Sau khi sửa
  review đã **chạy lại** (số dưới đây) ⇒ PASS đủ 12/12.

### Cổng + Miri (không retry — D2-07)

- [`JOURNAL/kt20-gate-fast.log`](JOURNAL/kt20-gate-fast.log) — `gate --fast` **PASS 284 test / 0 failed**
  (**44 suite**; KT-19: 263/43 ⇒ **+21 test** = 12 integration + 8 unit `mcs` + 1 unit `pick_where`),
  **L1 1.000 seed/999 trace**, gen-check ON, `.scn` 21 tệp OK, **không retry**; `kt20_budget` native
  2,8 s.
- [`JOURNAL/kt20-miri.log`](JOURNAL/kt20-miri.log) — chạy lại sau sửa review:
  `v2 miri -p vifast-sim --test kt20_budget` **PASS 12/12** (0 failed, 0 ignored, **117,12 s**);
  2 cảnh báo `integer-to-pointer cast` **có sẵn** từ objmem (`v2/lib/vifast-objmem/src/obj.rs`,
  D2-14/D2-17), không phát sinh từ KT-20; `v2/sim/**` **0 unsafe**. Invariant test dưới Miri vẫn
  chạy nhưng giảm `N_SEED = 5`, `N_STEP = 16` (`#[cfg(miri)]`; stable giữ **1.000 × 64**) — không
  `#[ignore]`, không nới `gate --fast`/L1.

### Phạm vi / không chạm

Chỉ `v2/sim/vifast-sim/src/{mcs.rs,sched.rs,kobj.rs,lib.rs}` (`lib.rs` +1 dòng `pub mod mcs;`),
`v2/sim/vifast-sim/tests/{kt20_budget.rs,kt19_sched.rs}` (rig test-only gắn SC mặc định),
`v2/agent/{STATE.md,JOURNAL/}`. Không sửa `spec/`, `vifast-model`, `.scn`,
QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`; không `#[allow]`/`#[ignore]`.

### Review độc lập + đề xuất

`reviewer-kt20` (lead spawn vì depth 1/1): **DRIFT** (không UNSAFE) — (1) "TCB không SC = không
giới hạn" trái §4.5/K1B-PLAN DoD 4 ⇒ **đã sửa** sang "không SC/chưa `Configure` = không có budget,
không chạy" + rig test-only gắn SC mặc định + test no-SC/chưa-Configure/cạn budget; (2) Miri "chưa
xong" do đọc log giữa lúc ghi ⇒ log hoàn chỉnh + chạy lại **PASS 12/12, 117,12 s**. Không phát hiện
UNSAFE; `v2/sim/**` 0 unsafe.

**Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (không tự DONE, không lật QUEUE — D2-37).
**YC/Follow-up**: (1) **Agent 0 ghi D2-50** "no SC ≠ unlimited" khi lật sổ (lead đã xác nhận sẽ
ghi); (2) KT-21 donation/passive server (SC đi theo Call/Reply — SC-less server chỉ chạy khi được
donate); (3) KT-23 thêm `kt20_budget` vào `nightly --only-miri` (117 s) và cân nhắc thời lượng Miri.

## KT-19 — Scheduler 1 hart: 256 mức + bitmap, `Ready/Running`, preempt mọi bước (K1b, D2-49(a))

**Vào**: READY (agentctl `next --project v2` trả KT-19) · HEAD `26643ba` · KT-18 DONE · baseline
`gate --fast` PASS 251 test/42 suite ([`JOURNAL/kt19-baseline-gate-fast.log`](JOURNAL/kt19-baseline-gate-fast.log)).
**Phạm vi**: scheduler **1 hart** cho **mô hình TCB** (không phải scheduler harness KT-08 `lib.rs`);
**không** budget/refill (KT-20), donation (KT-21), hộp thư/push migration (KT-22, K1c); không
`spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không thêm dependency/`unsafe`;
không `#[allow]`/`#[ignore]`. Working tree: `viscript/` + tệp ngoài `v2/` có thay đổi của tab khác —
**không chạm**, chỉ stage `v2/`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt19-red-compile.log`](JOURNAL/kt19-red-compile.log) — `tests/kt19_sched.rs` không biên dịch:
  **93 × `E0599`** cho toàn bộ API còn thiếu (`ready_len`, `sched_pick`, `current`, `dispatch`,
  `check_sched`, `block_current`, `wake`, `yield_current`, `set_preempt_every_step`, `sched_idle`,
  `sched_cur_prio`, `ready_contains`).
- [`JOURNAL/kt19-red-queue.log`](JOURNAL/kt19-red-queue.log) — **sau khung** (đã có `HartSched` +
  `check_sched`/`dispatch` nhưng **chưa** nối `transition_tcb` với ready queue): **0/10 test pass**;
  thông điệp đúng dự kiến K1B-PLAN §5 KT-19 #2: `check_sched sau transition Ready: TCB 7 Ready nhưng
  không nằm trong ready queue`; test dispatch đỏ vì chưa TCB nào vào hàng ⇒ `current(0) == None`.
- Test red trước then chốt ở 2 log trên; sau khi nối một cửa `transition_tcb → sched_sync`, toàn bộ xanh.

### Sản phẩm

- `v2/sim/vifast-sim/src/sched.rs` (**mới**, ~300 dòng + 2 unit test): `HartSched {
  queues: [VecDeque<ObjId>; 256], bitmap: [u64; 4], len, current, idle, cur_prio }`;
  `highest_priority`/`pick` **O(1)** (tối đa 4 word, `leading_zeros`, không quét 256 hàng), cùng mức
  FIFO (`VecDeque::front`); `enqueue/remove_at/remove_all/set_current/clear_current`; `check_local`
  (bitmap ⇔ hàng khác rỗng + `len`) và `for_each_queued` (chỉ đi bit bật — bản quét 256 hàng/kiểm
  làm kt19 chậm 15 µs/kiểm, đã tối ưu còn 0,19 µs). Không `HashMap`/thời gian thực ⇒ tất định theo seed.
- `v2/sim/vifast-sim/src/kobj.rs`: `Machine.sched: Vec<HartSched>` (KT-19: `HART_COUNT = 1`, cấu trúc
  theo hart để K1c mở rộng) + `preempt_every_step: bool` (mặc định **tắt** — không đổi ngữ nghĩa KT-18);
  **một cửa**: `transition_tcb` preempt TCB cũ về `Ready` khi vào `Running{hart}` rồi `sched_sync`
  (gỡ khỏi bucket theo `priority`, nhả `current`, đặt `Ready`+`!suspended` vào hàng / `Running` thành
  `current`); hook `set_tcb_suspended`, `tcb_resume` (chỉ enqueue khi thực sự gỡ cờ `suspended`),
  `tcb_set_priority`/`tcb_set_affinity`/`tcb_set_home` (gỡ bucket cũ trước khi đổi). Model API `[TK]`:
  `ready_len`, `ready_len_at`, `ready_contains`, `current`, `sched_idle`, `sched_cur_prio`, `sched_pick`,
  `sched_pick_on`, `dispatch`, `block_current`, `wake`, `yield_current`, `check_sched`,
  `set_preempt_every_step`/`preempt_every_step`, `hart_count`; `TcbInfo` có `snapshot()` dùng chung.
  `sched_after_op(preempt)` gọi sau **mỗi** public op TCB/scheduler (`call/recv/reply/exit/suspend/
  resume/wait_notification/wait_address/set_priority/set_affinity/set_home/delete/revoke/retype` =
  preempt+kiểm; `finalise_tcb/configure/set_fault_ep/set_sched_params/bind_notification` = kiểm).
- `v2/sim/vifast-sim/tests/kt19_sched.rs` (**mới**, **10 test**, 0 ignored): (1) `pick` đủ **256 mức**
  0..=255, mức cao nhất trước, FIFO cùng mức, rút hết bằng `dispatch`+`Suspend(Abort)` (assert đúng
  từng mức); (2) dispatch `Running→Ready→Running`, ≤1 `Running`/hart, Running không trong hàng, tôn
  trọng priority + preempt khi có mức cao hơn; (3) `check_sched` sau **mỗi** public op (tour đủ
  Configure/SetFaultEp/set_sched_params/bind_notification/affinity/home/Ready/Call/Recv/Reply/Suspend/
  Resume/Wait notification/WaitAddr/Exit/finalise); (4) preempt ép **mọi bước**: 2 TCB cùng mức xen kẽ
  40 bước, **không đói** (≥19/40 mỗi TCB); (5) **1.000 seed × 64 op** ngẫu nhiên
  (call/recv/reply/suspend/resume/exit/retype): cùng seed ⇒ cùng trace, bất biến sau **mỗi** op,
  1.000/1.000 trace khác nhau; (6) wake TCB `suspended` **không** enqueue, `Resume` enqueue đúng
  priority; (7) `block_current`/`yield_current` đổi `current`, `block_current(Ready)` bị từ chối;
  (8) finalise TCB đang chạy + Suspend(Abort) + Revoke Untyped ⇒ `ready_len` giảm đúng, `current` được
  thay, không ObjId chết trong hàng; (9) affinity/home mặc định (1/0) + rebucket khi đổi priority;
  (10) phủ "một cửa"/curr.

### Bằng chứng đo (DoD 1-7)

- **256 mức**: test tạo đủ 256 TCB priority 0..=255, `sched_pick()` = mức 255; vòng rút hạ dần assert
  `pick == mức p` và `sched_cur_prio == p` cho **mọi** p (255→0) — O(1) theo 4 word bitmap, FIFO cùng mức.
- **≤1 Running/hart**: mỗi bước `dem_running(...) == 1`; `Running` không nằm trong hàng (`ready_contains`).
- **`Ready` ⇔ hàng + `!suspended`**: `check_sched` bắt mọi lệch (gồm cả `Ready+suspended` trong hàng);
  gọi sau mỗi public op + sau mỗi preempt; test gọi thêm sau từng op trong 1.000 seed (kể cả op lỗi).
- **Preempt/không đói**: 40 bước preempt trên 2 TCB cùng mức cho 20/20 lượt; 1.000 seed × 64 op =
  **64.000 op** (đo phân bố tạm thời: call 5.077 ok/4.137 lỗi, recv 5.476/3.714, reply 1.781/7.223,
  suspend 5.436/3.812, resume 8.936/0, exit 9.340/0, retype 9.068/0) — mọi op đều kiểm bất biến.
- **Gỡ hàng**: `Suspend(Abort)`/`finalise_tcb`/`Revoke Untyped` giảm `ready_len` đúng và thay `current`;
  `check_sched` xác nhận không còn ObjId chết/không Ready trong hàng (revoke 2 TCB ⇒ hàng 2→0).

### Cổng + Miri (không retry — D2-07)

- [`JOURNAL/kt19-gate-fast.log`](JOURNAL/kt19-gate-fast.log) — `gate --fast` **PASS 263 test / 0 failed**
  (43 suite; KT-18 là 251/42 ⇒ **+12 test**: 10 integration + 2 unit `HartSched`), **L1 1.000 seed/999
  trace**, gen-check ON, `.scn` 21 tệp OK, **không retry**; KT-19 test native 2,8 s.
- [`JOURNAL/kt19-miri.log`](JOURNAL/kt19-miri.log) — `v2 miri -p vifast-sim --test kt19_sched` **PASS
  10/10** (0 ignored, 4 m 32 s); 2 cảnh báo `integer-to-pointer cast` **có sẵn** từ objmem
  (`v2/lib/vifast-objmem/src/obj.rs:166,175`, D2-14/D2-17), không phát sinh từ KT-19; `v2/sim/**` **0 unsafe**.
- **Miri cần giảm phạm vi (ghi rõ, chỉ dưới `cfg(miri)`; stable giữ nguyên DoD)**: `check_sched` sau
  **mỗi** op quét toàn bộ TCB sống nên Miri chậm hơn ~1000 lần — đo thật 1.000 seed sẽ >4 h. Trên
  stable vẫn **1.000 seed × 64 op** và **đủ 256 mức**; dưới Miri: **5 seed × 64 op** và **64 mức**
  (`#[cfg(miri)] N_SEED = 5`, `N_PRIO = 64`; log 20 seed = 5 m 46 s trước khi giảm:
  [`JOURNAL/kt19-miri-1000seed-aborted.log`](JOURNAL/kt19-miri-1000seed-aborted.log)). Không `#[ignore]`,
  không nới ngưỡng `gate --fast`/L1.

### Phạm vi / không chạm

Chỉ `v2/sim/vifast-sim/src/{sched.rs,kobj.rs,lib.rs}` (`lib.rs` +1 dòng `pub mod sched;`),
`v2/sim/vifast-sim/tests/kt19_sched.rs`, `v2/agent/{STATE.md,JOURNAL/}`. Không sửa `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`;
không `#[allow]`/`#[ignore]`.

### Review độc lập + đề xuất

Đã báo lead spawn reviewer trước commit (depth 1/1) — lead spawn `reviewer-kt19` (review-only, đọc
diff uncommitted + tự chạy gate/Miri). **Verdict `reviewer-kt19`: APPROVE** (không DRIFT/UNSAFE);
reviewer tự chạy `gate --fast` **263 test/43 suite**, L1 1000/999, `kt19_sched` **10/10**; xác nhận
`pick`/bitmap/FIFO, bất biến Ready⇔queue, ≤1 Running, preempt mặc định tắt, scope sạch.

**Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (không tự DONE, không lật QUEUE — D2-37).
**Follow-up**: KT-20 thêm điều kiện "chỉ pick TCB có SC còn budget" (bổ sung vào `dispatch_step`,
không đổi API KT-19); KT-22/K1c push migration đọc `idle`/`cur_prio` đã công bố; KT-23 thêm
`kt19_sched` vào `nightly --only-miri` và cân nhắc thời lượng Miri (hiện 4 m 32 s với 5 seed/64 mức).

## KT-18 — TCB đầy đủ, bảng `ALLOWED` 11×11, `suspended` (xong mã, chờ verdict reviewer)

**Vào**: READY · HEAD `ecb0e08` · K1a' DONE, K1b mở (D2-49) · baseline `gate --fast` PASS
([`JOURNAL/kt18-baseline-gate-fast.log`](JOURNAL/kt18-baseline-gate-fast.log)).
**Phạm vi**: TCB + vòng đời của **mô hình sim**; **không** scheduler (KT-19), budget (KT-20), donation
(KT-21), hộp thư (KT-22); không `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`.
Bảng `ALLOWED` đặt ở `v2/docs/k1b-tcb-transition.md` (model-only `[TK]`) theo **D2-49(d)**.

### Đỏ trước (D2-05)

- [`JOURNAL/kt18-red-compile.log`](JOURNAL/kt18-red-compile.log) — `tests/kt18_tcb.rs` **không biên dịch**:
  `E0599` (chưa có `transition_tcb`/`tcb_suspend`/`tcb_resume`/`tcb_set_priority`/`tcb_set_affinity`/
  `tcb_set_home`/`set_sched_params`/`tcb_info`/`tcb_bind_notification`/`pair_hit_count`/
  `transition_coverage`/`finalise_tcb`/`endpoint_send_len`/`reply_caller`/`notification_waiter`/
  `tcb_wait_notification`/`tcb_wait_address`), `E0308`/`E0609` (`tcb_read_state` trả `TcbState`, không
  phải `TcbSnapshot` 4 giá trị), `E0559` (`BlockedOnRecv` thiếu `reply`), `E0432` (`ALLOWED`/`TcbSnapshot`/
  `SuspendMode`/`BlockedOnKind` chưa có).
- [`JOURNAL/kt18-red-transition-gate.log`](JOURNAL/kt18-red-transition-gate.log) — đột biến **bỏ cổng**
  (`set_tcb_state` cũ nhận mọi cặp) ⇒ test ma trận 11×11 đỏ đúng lý do: cặp cấm `(0,0)` nhận `Ok(())`
  thay vì `IllegalOperation` (**14 passed, 1 failed**).
- [`JOURNAL/kt18-red-abort-unlink.log`](JOURNAL/kt18-red-abort-unlink.log) — đột biến **Abort không gỡ
  hàng chờ** ⇒ **5 test đỏ**: hàng gửi `Ok(1)` (mong `Ok(0)`), `Reply.caller Ok(Some(7))` (mong `Ok(None)`),
  hàng nhận `Ok(Some(8))`, notification waiter `Ok(Some(7))`, message Exit chưa huỷ (**10 passed, 5 failed**).

### Sản phẩm

- `v2/sim/vifast-sim/src/tcb.rs` (**mới**): `TcbState` (11 tên, `index()` khớp `vifast_abi::TCB_STATES`),
  `TcbSnapshot` (4 giá trị của `Tcb.ReadState`), `BlockedOnKind`, `SuspendMode`, `FaultKind`,
  `SleepReason`, **`pub const ALLOWED: [[bool; 11]; 11]`** (48 cặp ALLOWED/121 ô) + `allowed_count()` +
  `coverage()`; 2 unit test (không cặp danh tính nào ngoài `BlockedOnSend`; chỉ số/`TCB_STATES`).
- `v2/sim/vifast-sim/src/kobj.rs`: `TcbData` đủ **11 trường** (thêm `suspended`, `priority`,
  `max_controlled_priority`, `affinity: u64`, `home: u16`, `sched_context: Option<ObjId>`,
  `bound_notification: Option<ObjId>`); **một cửa `Machine::transition_tcb`** (cặp cấm ⇒
  `IllegalOperation`, **không** đổi state; `pair_hits: [u32; 121]` + `transition_coverage()`);
  `tcb_read_state → TcbSnapshot`; `tcb_suspend`/`tcb_resume`; `finalise_tcb` gọi **trước** `destroy`
  trong `revoke_children` (state `Dead`, gỡ endpoint/Reply/notification/address, trả `sched_context`
  qua hook + `finalise_log`); model API `set_sched_params` (D2-49(b)), `tcb_set_priority`/`tcb_set_affinity`/
  `tcb_set_home`/`tcb_bind_notification`/`tcb_info` (D2-49(c)); `Notification` có waiter + bucket
  WaitOnAddress tối giản (`tcb_wait_notification`/`tcb_wait_address`) chỉ để chứng minh Abort/finalise
  gỡ đúng hàng (Signal/WakeAddr thuộc K1c).
- **Drift D2-33 đã sửa**: `BlockedOnRecv { ep, reply: Option<ObjId> }`, `BlockedOnAddress.offset: u32`,
  `ReadState` trả 4 giá trị.
- **Bảng 48 cặp = 46 cặp suy ra từ thiết kế + 2 cặp `Stopping` reserved** cho KT-19/K1c (không có
  nguồn nhập `Stopping` ở K1b 1 hart); coverage **của bảng qua cổng** ở test ma trận, coverage
  **thao tác** ở test `moi_duong_dung_mot_cua_transition_tcb` — không dùng `(48,48)` làm coverage thao tác.
- `v2/sim/vifast-sim/tests/kt18_tcb.rs` (**mới**, **18 test**, 0 ignored): ma trận 11×11 (mỗi cặp một TCB
  trên **một** `Machine` ⇒ `transition_coverage() == (48, 48)`, cặp cấm `IllegalOperation` + state giữ
  nguyên + hit `0`), Abort gỡ hàng gửi/nhận/Reply/notification/address (đếm hàng giảm đúng 1) và giữ
  `exit_code`, `Defer ⇒ InvalidArgument` (state không đổi, không hit), `Resume` không phát lại
  message/queue + xoá `exit_code`, `Exit` giữ D2-25 (kể cả Exit khi đang chờ gửi ⇒ cặp danh tính 3→3),
  finalise 6 TCB khi Revoke Untyped (mọi hàng chờ rỗng, log 6 dòng, `sched_context` trả lại), và test
  "một cửa" (đường thật `call/recv/reply/exit/suspend/finalise` chỉ đi qua `transition_tcb`).
- `v2/docs/k1b-tcb-transition.md` (**mới**, model-only `[TK]`, D2-49(d)): bảng 11×11 + lý do từng nhóm
  cặp từ thiết kế §4.4/§4.5 + D2-25/D2-33, quy ước mô hình (kích hoạt ngầm, không cặp danh tính,
  `Defer`, `reply` bị bỏ khi `caller = None`), bảng drift đã sửa, việc của KT-19/20/21/K1c/K3.

### Cổng + Miri

- [`JOURNAL/kt18-gate-fast.log`](JOURNAL/kt18-gate-fast.log) — `gate --fast` **PASS**, **251 test / 0 failed**
  (42 suite; KT-17: 231/41 ⇒ +20 test), **L1 1.000 seed/999 trace**, gen-check ON, **không retry** (D2-07).
- [`JOURNAL/kt18-miri.log`](JOURNAL/kt18-miri.log) — `./v2/bin/v2 miri -p vifast-sim --test kt18_tcb`
  **PASS 18/18** (0 failed, 0 ignored); 2 cảnh báo `integer-to-pointer cast` **có sẵn** từ
  `v2/lib/vifast-objmem/src/obj.rs:175` (arena objmem, D2-14/D2-17), không phải code KT-18; `v2/sim/**` 0 `unsafe`.
- [`JOURNAL/kt18-check.log`](JOURNAL/kt18-check.log) — `v2 check` PASS (fmt/clippy/R1..R5/ABI/scn/report/gen-check).

### Review độc lập (`reviewer-kt18`)

`reviewer-kt18` (review-only) **DRIFT nhẹ, không UNSAFE**: tự chạy `gate --fast` **248 test/42 suite**,
L1 1000/999, Miri `kt18_tcb` **15/15**, xác nhận 48/73 cặp, một cửa `transition_tcb`, phạm vi sạch.
**Đã sửa đủ 4 mục:**
1. **Thứ tự `exit()`**: giờ đọc + `require_cap_type(fault_ep)` **trước** khi ghi `exit_code`/gỡ hàng
   chờ ⇒ cap `fault_ep` đã Delete/Revoke trả `InvalidCapability`, TCB **không** đổi trạng thái/hàng
   chờ (test mới `exit_cap_fault_ep_chet_khong_doi_trang_thai`).
2. **Bảng tự chứng minh vòng tròn**: thêm test `cap_cam_theo_thiet_ke_bi_tu_choi` (5 ca cấm viết từ
   lý do thiết kế: `Inactive→Running`, `Sleeping→Running`, `BlockedOnReply→BlockedOnRecv`,
   `Dead→Ready`, `Ready→Sleeping`); phát biểu lại DoD **46 cặp + 2 reserved**, `(48,48)` chỉ là
   coverage **của bảng qua cổng**, không phải coverage thao tác.
3. **Thiếu test `Suspend(Abort)` trên `Sleeping`**: thêm `suspend_abort_tren_sleeping_giu_trang_thai`
   (vẫn `Sleeping`, `suspended`, giữ `exit_code`, không hàng chờ, không hit cặp danh tính).
4. **Ghi chú doc**: `Exit` từ mọi trạng thái sống là quy ước model-only; `Send(reply)` lần hai im
   lặng thành công (reply bị bỏ) — đã ghi trong `k1b-tcb-transition.md` §4; fixture `12-…scn:13`
   `Defer` để **KT-23** (không sửa `.scn` ở KT-18).

### Đề xuất + follow-up

- **Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (review độc lập DRIFT nhẹ đã sửa đủ; không tự DONE,
  không lật QUEUE — D2-37).
- **Follow-up cho KT-23**: `.scn` `12-suspend-abort-while-reply-wait.scn:13` ghi `Tcb.Suspend mode=Defer => ok`
  trong khi thiết kế §4.4 `:282` nói `Defer` **không** dùng cho IPC ⇒ KT-18 trả `InvalidArgument` (DoD 6);
  fixture mới chỉ kiểm tĩnh, khi KT-23 chạy `.scn` trên mô hình phải chốt lại (sửa fixture, không nới ngữ nghĩa).
- `Stopping` chưa có nguồn nhập trong K1b 1 hart (D2-49(a)) — cặp `Ready/Running → Stopping`,
  `Stopping → Dead` có sẵn cho KT-19/K1c; notification `Signal`/`WakeAddr` thuộc K1c; ready queue/refill
  queue hook thuộc KT-19/KT-20.

## DoD K1a' (chốt ở KT-17)

Định nghĩa xong của K1a' (thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:872-873`): *thay store giả
bằng objmem (API closure) + Miri*; các ca Untyped/CNode của KT-04c chạy trên mô hình.
**Đủ 2 dòng, mỗi dòng có bằng chứng đo được:**

| # | Điều kiện DoD | Bằng chứng |
|---|---|---|
| a | Thay store giả bằng objmem + **Miri từ commit đầu** | `vifast-objmem` Miri **40 test** (20 lib + 8 alias + 1 compile-fail + 1 fuzz 200 chuỗi/197,30 s + 10 contract); `./v2/bin/v2 nightly --only-miri` **PASS 365 s, exit 0** (sim `--lib` 13/13, kt16 4/4, kt09_untyped 8/8, kt09_revoke 7/7, kt10_cnode 13/13, kt17_align 3/3); `v2 miri` cũ vẫn xanh 13/13 — [`JOURNAL/kt17-nightly-only-miri.log`](JOURNAL/kt17-nightly-only-miri.log), [`JOURNAL/kt17-miri-old-cmd.log`](JOURNAL/kt17-miri-old-cmd.log) |
| b | Ca Untyped/CNode của KT-04c chạy trên mô hình, không hồi quy | `.scn` Untyped/CNode **9/9** bước khớp, 0 lệch (3 bước setup Frame để K2); **parity 10.000 seed/240.000 thao tác/9.065 trace khác nhau/0 lệch**; `gate --fast` **231 test/41 suite**, L1 1.000 seed/999 trace, `.scn` 21 tệp OK — [`JOURNAL/kt17-kt04c-scn-parity.log`](JOURNAL/kt17-kt04c-scn-parity.log), [`JOURNAL/kt17-gate-fast.log`](JOURNAL/kt17-gate-fast.log) |

> **Cập nhật 06:40 bởi Agent 2**: **KT-17 xong mã — K1a' đủ DoD** (task cuối phase K1a'):
> (1) đóng **2 follow-up KT-16** — `alloc_place_span` cap `align ≤ ARENA_ALIGN` (fail-closed,
> test đỏ `kt17-red-align.log` → xanh), `Machine::retype` Untyped `size_bits ≥ 13` trả
> `Alignment` rõ; `Store::alloc` chốt hợp đồng `0/không lũy thừa 2/> 4096 ⇒ None` + test;
> (2) **`v2 nightly --only-miri`** (D2-47(a), không vào `gate --fast`, không L1 100k/mutation/
> soak; dùng chung `require_miri_toolchain` với `v2 miri` nên thiếu toolchain fail rõ);
> (3) Miri tổng **PASS 365 s/exit 0** (objmem 40 test + sim 48 test); (4) `.scn` Untyped/CNode
> **9/9** + parity **10.000 seed/0 lệch**; (5) `gate --fast` **PASS 231 test/41 suite**
> (KT-16: 227/40; +4 test mới), L1 999 trace, **không retry**. Reviewer `reviewer-kt17`:
> **DRIFT nhẹ (chỉ tài liệu/DoD)** — đã sửa đủ 4 mục. Chi tiết + log: `JOURNAL/kt17-journal.md`,
> `JOURNAL/kt17-*.log`.
>
> **Cập nhật 06:00 bởi Agent 2**: **KT-16 xong mã** — thay **hai** store giả bằng
> `vifast-objmem` (D2-14, D2-47d): (1) `vifast-sim::Store` dùng `OwnedArena` (vùng căn 4096
> do objmem cấp, địa chỉ logic `0x1000+offset` giữ trace/golden tất định); (2) ruột
> `kobj::Machine` — body `Kind` nằm trong `ObjMem`, **mỗi Untyped một arena riêng** nên
> watermark = `Arena::used()` (bỏ `Vec<Object>` + `align_up` viết tay), Revoke Untyped
> `reset` cả cây; `Meta` giữ metadata cho `object_type`/`object_bytes` của object đã chết
> (parity hash cả object chết), `Cap` vẫn `Vec<Cap>`. Thêm `OwnedArena`/`OwnedMem` (feature
> `heap`) vào objmem — **toàn bộ `unsafe` nằm trong objmem**, `v2/sim/**` 0 `unsafe` (R5
> xanh); guard `chua_header` áp cho **cả** `with`/`with_mut` (reviewer KT-15, test đỏ trước
> `Ok(7)` vs `Dead`); bảng kind → ctor **đo 8.192 B = 8 KiB** (không phình); `ObjMem` có
> `id_base` để `id` duy nhất giữa các vùng. Test đỏ trước `kt16_objmem.rs` **10 × E0599**.
> Bằng chứng: **227 test/40 suite**, old test `git diff --stat kt09*/kt10*/kt11*` **rỗng**;
> parity 10.000 seed 0 lệch; **Miri PASS** (objmem 39 test; kt16 4/4; kt09_untyped 8/8;
> kt10_cnode 13/13); `aarch64-unknown-none` xanh (feature off); `gate --fast` PASS không
> retry. Reviewer `reviewer-kt16`: **APPROVE** (không DRIFT/UNSAFE). Chi tiết + log:
> `JOURNAL/kt16-journal.md`, `JOURNAL/kt16-*.log`.
>
> **Follow-up KT-17 (reviewer-kt16) — ĐÃ ĐÓNG ở KT-17**: (1) `alloc_place_span` cap
> `align > ARENA_ALIGN` (fail-closed `None`, không đốt byte) + `Machine::retype` Untyped
> `size_bits ≥ 13` ⇒ `Alignment` rõ ràng (trước: trả `pa` có thể sai căn); (2) `Store::alloc`
> chốt hợp đồng `align 0/không lũy thừa 2/> 4096 ⇒ None` + test đối chứng, doc ghi rõ.
> Không còn việc phải làm cho 2 follow-up này.

> **Cập nhật 04:48 bởi Agent 2**: **KT-15 xong mã** — `with_many_mut` thật theo **D2-48 (a)**:
> bảng cố định `kind → constructor` (256 ô trong `ObjMem`, `place::<T>` đăng ký), giữ `ObjRef` 16 B
> (thêm `into_dyn()` — vẫn 16 B, không vtable) + `KObj::as_any_mut()` (default `None`) để closure
> `[&mut dyn KObj; N]` ghi được kiểu cụ thể. Alias kiểm **từng cặp** trước khi tạo ref (trùng `pa`,
> chồng vùng, ref chết, kind lạ ⇒ `Alias` fail-closed, closure không gọi); `N = 9` **không build**
> (`const { assert!(N <= 8) }`, log `kt15-red-n9.log`). Crate **34 test** (lib 14, alias 8,
> compile_fail 1, fuzz 1, contract 10); **Miri PASS** (alias 8/8; fuzz **200 chuỗi/191,95 s** < 20 phút
> ⇒ giữ 200); fuzz stable **10.000 chuỗi × 32 thao tác** PASS (4,51 s); build `aarch64-unknown-none`
> xanh; `gate --fast` **PASS 218 test** không retry. Reviewer `reviewer-kt15` **DRIFT (nhẹ), không
> UNSAFE** — đã sửa: bỏ claim sai "fail-closed" (ref không mang identity store → thành hợp đồng
> `place`/`reset`), guard `chua_header`/`Arena::chua` (ref store khác/`pa` ngoài arena ⇒ `Alias`),
> `TypeId` + `debug_assert` chống va chạm `kind`, test end-to-end chồng vùng (`Gia`), trùng `id`,
> ZST, mảng trộn 3 kind, sửa số đo STATE. Chi tiết + bảng log: `JOURNAL/kt15-journal.md`.

> **Cập nhật 04:22 bởi Agent 2**: **KT-14 xong mã** — crate `v2/lib/vifast-objmem` (15 test: header 16 B,
> arena thô căn lề 8/16/64 + hết chỗ `None`, `place/with/with_mut/destroy`, `Dead`, id không tái dùng,
> object align 64); Miri 15/15, build `aarch64-unknown-none`, `gate --fast` 199 test/L1 1000 (không retry).
> Reviewer `reviewer-kt14` **DRIFT (nhẹ)** — đã sửa: test alias **fail-closed** (hết tautology), STATE,
> **YC-18** (`ObjRef<dyn KObj>` 16 B thiếu vtable → Agent 0 chốt trước KT-15), ghi chú `UnsafeCell`/
> store identity/cluster id. Chờ Agent 0 lật sổ.

> **Cập nhật 18:20 bởi người điều phối** theo `review/agent0/DUYET-YC-02-05.md`: YC-04/05 **đã đóng** (D2-32/D2-33), xem **Khối F** trong `QUEUE.md`. Không mở lại YC-04/05.

> **Cập nhật 20:00 bởi Agent 0**: KT-30 DONE (29718a6, 461 test, Miri 16/16, D2-60); KT-31 READY — cổng cuối K1c.

> **Cập nhật 03:35 bởi Agent 2**: **KT-13 xong mã** — thêm `v2 miri` (D2-47(a): lệnh rời, **không** trong
> `gate --fast`); Miri PoC 13 test PASS; `check` + `gate --fast` PASS (184 test, không retry). Chờ review
> độc lập + Agent 0 lật sổ (D2-37).
> **Cập nhật 03:41 bởi Agent 2**: `reviewer-kt13` **APPROVE** (tự chạy PoC 13/13 + gate 184 test/L1
> 1000/999); đã sửa 3 nit (`D2-47(a)` citation, timestamp 03:35, câu chữ `nightly`); gate lại PASS
> ([`JOURNAL/kt13-gate-fast-final.log`](JOURNAL/kt13-gate-fast-final.log)). Đề xuất DONE.

## Ghi chú

- **L1 đã bật từ KT-08**: `gate --fast` chạy thêm `# L1 OK: 1000 seed` (sim `vifast-sim`, cùng seed → cùng trace). `gen-check` **đã bật** (KT-05, chạy trong `v2 check`/`gate --fast`).
- `./v2/bin/v2 check` / `gate --fast` **đã có** từ KT-03; `gate` đầy đủ và `nightly` **từ chối** (exit 2) cho tới khi có L2/L3 và sim — không núp bóng cổng rỗng.
- Task `REVIEW` (KT-04d) **không** được coi là đã duyệt; chỉ mở khi có dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `QUEUE.md`. **Không** ghi APPROVED.
- Tag v1: `v1-archive` trên `ca2eaf5` (KT-01). **Không** có `v1-final`; v1 không soak lại.
- Nhánh làm việc: `main`. Mỗi commit `[KT-##]`; chạm `kernel/`, `lib/` phải xanh `--fast` (D2-05). **Push sau mỗi commit xanh**: `git push origin main`.
- Toolchain: `cargo`/`rustc` 1.99.1 ở `~/.cargo/bin` (**không** có trên PATH mặc định). `v2/bin/v2` **tự tìm** cargo (PATH → `~/.cargo/bin` → `/usr/local/cargo/bin` → `/opt/homebrew/bin`) và luôn dùng `--manifest-path v2/Cargo.toml` ⇒ **không bao giờ** chạy workspace v1.
- **D2-27**: KT-04c/KT-04d = **21** kịch bản (20 theo QD §4 + ca "Exit khi procsrv chưa Recv"). Q-01 đóng.
- **D2-28**: KT-02 chỉ port tối đa `vifast-fs`, `vifast-fsutil`, `vifast-elf`.
- **Thứ tự điều phối**: `KT-03` (xong) → **`KT-07`** (K0.5 boot riscv64) → `KT-04a`. KT-07 tạo `v2/kernel/` + `v2/boot/`; **không** tạo `v2/platform/` (cây chuẩn §8 là `kernel/vifast-plat-{riscv64,aarch64,x86_64}`).
- **QUEUE row chưa lật** `KT-02 DONE` / `KT-03 READY`: `vifast-agentctl` chưa tồn tại (KT-06) và Agent 2 **không** được sửa QUEUE — chờ Agent 0. Thực chất phụ thuộc đã DONE và đã kiểm lại.

## Cổng v2 (KT-03) — bộ luật R1..R5

`v2/bin/v2` = wrapper bash; `v2/test/rules/run.sh` = self-test 6 case (5 âm + 1 dương).

| Luật | Nội dung | Nguồn | Case âm |
|---|---|---|---|
| **R1** | Tệp trong `kernel/`, `lib/` ≤ **1.500** dòng | thiết kế §8 (cổng cứng, D2-06) | `case-r1-long` (1.501 dòng) |
| **R2** | Cấm `cfg(target_arch)` trong `kernel/vifast-core/**` + `lib/**` | thiết kế **:701** | `case-r2-cfg-arch` |
| **R3** | SPDX theo loại tệp: `//` cho `.rs/.s/.asm`, `#` cho `.toml/.vis/.vish/.vidl/.sh/.py`, sidecar `.license` cho `.json/.bin/.txt` | QD §9.3 | `case-r3-no-spdx` |
| **R4** | Cấm path-dependency sang `crates/` (v1 đóng băng) | D2-26 | `case-r4-path-dep` |
| **R5** | `unsafe` chỉ trong danh sách đóng: `vifast-sync`, `vifast-objmem`, `kernel/vifast-plat-*`, `boot/` | D2-17, thiết kế §8 | `case-r5-unsafe` |

Ghi chú: bộ **5 luật này là R1..R5** (Agent 2 chọn; không phải 6 luật của thiết kế §8 — gen-check là luật thứ 6, thuộc KT-05). R1 làm ở KT-03 thay vì "giao T05" như `review/C-handoff.md` §315 đề nghị: **chặt hơn**, không nới ngưỡng.

## Hiện trạng đo được (sau KT-03)

| Hạng mục | Hiện trạng | Dẫn chứng |
|---|---|---|
| `HEAD` | xem `git log -1`: KT-04a ở `1766145` (+ commit STATE `a8bb272`); KT-07 ở `bf61901`+`e01a81f` | `git log -1 --oneline` |
| Cổng | `./v2/bin/v2 check` PASS (gồm `abi spec`, `abi reserved`, `scn`); `gate --fast` PASS (thời gian: xem log, không ghi tay — Q12); `gate`/`nightly` exit 2 | log cổng; `v2/bin/v2:255` |
| Luật | self-test **6/6** (`ok 1..6`); R1..R5 có case âm đúng mã luật | `bash v2/test/rules/run.sh` |
| Build/test | `cargo test --manifest-path v2/Cargo.toml --workspace` xanh: fs **55** (52 unit + 3 T26) + elf **20** = **100**, 0 failed; clippy `-D warnings` 0 warning; fmt sạch | trong `gate --fast` |
| Luật 1.500 dòng | tệp lớn nhất `vifast-fs/src/fs.rs` = **1.248** dòng | `wc -l` |
| `spec/` | **có** `spec/abi.toml`: 16 syscall (0..14 + **15 DebugGetInfo** D2-29), dải reserved **16..31**, 19 mã lỗi, object **1..8** (SchedControl 8 = lõi, D2-32) + reserved **9..16** (`[object_reserved]` 17..31); `status = "REVIEW"` | `./v2/bin/v2 check` |
| `v2/bin/v2` | **có** (wrapper), `.gitkeep` giữ thư mục | `ls v2/bin` |

## KT-13 — Bootstrap Miri + `v2 miri` (xong mã, review APPROVE, chờ lật sổ, 2026-11-13 03:35)

**Vào**: READY · HEAD `5ad86e5` · baseline `gate --fast` PASS (L1 1000 seed/999 trace, 556 ms).
**Phạm vi**: chỉ `v2/bin/v2` + `STATE`/`JOURNAL`; **không** đụng 5 luật R1..R5, `nightly` (vẫn stub),
`v2/agent/{QUEUE,DECISIONS}.md`, `spec/`, `AGENTS*`, `viscript/`.

**Đỏ trước (D2-05)**: [`JOURNAL/kt13-red-wrapper.log`](JOURNAL/kt13-red-wrapper.log) — `./v2/bin/v2 miri`
→ `v2: lệnh không biết: miri`, **exit 2**; [`JOURNAL/kt13-red-stable.log`](JOURNAL/kt13-red-stable.log) —
`cargo miri --version` trên stable → *"the 'miri' component … is not available for the
'stable-aarch64-apple-darwin' toolchain"*, exit 1.

**Hiện thực**: `find_rustup` (PATH → `~/.cargo/bin` → `/usr/local/cargo/bin` → `/opt/homebrew/bin`);
`do_miri` kiểm nightly + `miri` + `rust-src` + `cargo +nightly miri --version`, thiếu thì **exit 2**
(thiếu rustup **127**) và **in đúng câu bootstrap** `rustup toolchain install nightly` +
`rustup component add miri rust-src --toolchain nightly`; chạy
`cargo +nightly miri <sub> --manifest-path v2/Cargo.toml` (mặc định `sub=test`, nhận
`test/run/nextest/setup/clean`); usage thêm dòng `miri` ghi rõ D2-47 (lệnh rời, **không** trong
`gate --fast`). Exit code các lệnh cũ giữ nguyên (`gate`/`nightly`/lệnh lạ vẫn exit 2).

**Thiếu toolchain (giả lập bằng PATH + rustup/cargo giả)**: [`JOURNAL/kt13-red-toolchain.log`](JOURNAL/kt13-red-toolchain.log)
— 5 ca: thiếu rustup (127), thiếu nightly (2), thiếu `miri` (2), thiếu `rust-src` (2), `cargo-miri`
hỏng (2); cả 5 đều in câu bootstrap, không im lặng bỏ qua.

**PoC**: [`JOURNAL/kt13-miri-poc.log`](JOURNAL/kt13-miri-poc.log) —
`./v2/bin/v2 miri -p vifast-sim --lib` → **13 passed; 0 failed** (7 unit `lib.rs` + 6 unit `kobj.rs`;
Miri 33,17 s, tổng 50,28 s, EXIT=0), không `#[ignore]`, không cần `-Zmiri-disable-isolation`.

**Toolchain**: [`JOURNAL/kt13-toolchain.log`](JOURNAL/kt13-toolchain.log) — nightly `1.102.0-nightly
(3ef0c22cd)`, `miri 0.1.0`; component `miri` + `rust-src` có.

**Cổng**: [`JOURNAL/kt13-check.log`](JOURNAL/kt13-check.log) `check` **PASS**;
[`JOURNAL/kt13-gate-fast.log`](JOURNAL/kt13-gate-fast.log) `gate --fast` **PASS 184 test / 0 failed**,
L1 **1000 seed / 999 trace** (600 ms), 36,665 s, **không retry** (D2-07); sau 3 nit tài liệu chạy lại trên
cây cuối: [`JOURNAL/kt13-gate-fast-final.log`](JOURNAL/kt13-gate-fast-final.log) **PASS 184 test / 0 failed**,
L1 **1000 seed / 999 trace** (603 ms), 36,559 s.

**Review**: `reviewer-kt13` (độc lập, review-only) **APPROVE** — tự chạy PoC Miri **13/13**, `gate --fast`
**184 test / L1 1000/999**; xác nhận wrapper/exit code/phạm vi đúng. 3 nit tài liệu đã sửa (`D2-47(a)`
citation, timestamp 03:35, câu chữ `nightly`).

**Đề xuất**: **DONE** (review độc lập APPROVE, commit + push xong). Không lật QUEUE (D2-37).

## KT-09 — Untyped trên sim + Retype/Revoke + end-to-end (xong mã, chờ lật sổ)

- **Đỏ trước (D2-05)**: `JOURNAL/kt09-red-runtime.log` — 4 test target không biên dịch (chưa có
  `vifast_sim::kobj`/`scenario`); `JOURNAL/kt09-red-fixtures.log` — fixture `bad-state-value` bị bắt nhưng
  **sai thông điệp** (máy kiểm chỉ kiểm tên trường, chưa kiểm giá trị).
- **Sản phẩm**: `v2/sim/vifast-sim/src/kobj.rs` (Untyped watermark, CNode Revoke con/cháu, TCB/Endpoint/Reply/
  SchedContext; mã lỗi lấy từ `vifast_abi::ERRORS`), `src/scenario.rs` (C5: `expect_states` là hậu điều kiện
  **sau mỗi bước**, `exit` kiểm ngay sau halt trước procsrv `Recv`; scheduler chen giữa bước theo seed),
  4 tệp `tests/kt09_*.rs` (**27 test mới**); `check_scn.py` kiểm **giá trị** `expect_states`;
  `run_fixtures.sh` kiểm **đúng thông điệp** (5 fixture).
- **Cổng**: `gate --fast` PASS; L1 **1000 seed/999 trace** không đổi; golden trace không đổi; 21 `.scn` cũ PASS.
- **Không làm (đúng phạm vi)**: property test cây MDB (KT-10), `vifast-model` (KT-11), Frame/VSpace (K2).
- **KT-09b (sửa review Agent 0)**: `TcbState` đủ **11 variant** D2-33 + unit test so `vifast_abi::TCB_STATES`;
  `check_bytes` siết `allocated ≤ watermark`, từng range `≤ watermark`, `allocated + gap + free == size`
  (+ `untyped_allocated`/`untyped_gap`, test gap căn lề); revoke Untyped **lồng nhau** finalise cả cây con
  (+ test), sửa bug Untyped con `size=0`. Tổng **29 test** KT-09; gate --fast PASS, L1 1000 seed/999 trace.
- **Đề xuất**: Agent 0 lật `KT-09 = DONE` (không có YC mới).

## KT-10 — CNode/MDB test-first (xong mã, 2026-11-12 23:15)

**Vào**: READY, phụ thuộc KT-09 DONE (`3d578c0` + `da7d30f`) · HEAD `f638bd6` · baseline `gate --fast` PASS.

**Phạm vi**: mô hình tối giản trong sim/test; property test cây; Revoke. **Không** làm MDB đầy đủ
production (không `mdb_prev/mdb_next` 48 B), **không** `vifast-model` (KT-11), **không** Frame/VSpace/K2.

**Đỏ trước (D2-05)** (đính chính theo review độc lập F1/F2):
- [`JOURNAL/kt10-red-compile.log`](JOURNAL/kt10-red-compile.log) — 2 test target không biên dịch
  (chưa có `mint`/`cap_info`/`cap_table`/`cap_children`/`live_cap_count`/`check_caps`).
- [`JOURNAL/kt10-red-semantic.log`](JOURNAL/kt10-red-semantic.log) — **4/7** test đỏ đúng ngữ nghĩa:
  `delete` để con **mồ côi** (cha chết vẫn là parent) — 2 test `kt10_cnode` (`delete_*`) + 2 property
  (`cung_seed_cung_cay_tat_dinh`, `property_cay_cap_tat_dinh_giu_bat_bien`). **3/7** còn lại
  (`finalise_cnode_long_nhau_*`, `revoke_untyped_finalise_*`, `property_untyped_revoke_*`) panic ở
  `check_caps` với "cha cùng object" — **rule quá chặt cho con của `Retype`**, không phải lỗi finalise
  (rule đó đã bỏ). Vì vậy: đỏ-trước **chỉ chứng minh lỗi `delete`**; finalise CNode có test xanh
  nhưng **không có log đỏ riêng** — ghi nhận là hạn chế bằng chứng, không nhận "tái lập byte-for-byte"
  (log đỏ sinh trước khi chỉnh test: bỏ rule `check_caps`, kỳ vọng `revoke(None)`, `div_ceil`, unit checker).

**Sản phẩm**:
- `src/kobj.rs`: quyền `RIGHT_*`/`RIGHTS_ALL`, `CapInfo` + `cap_info`/`cap_table`/`live_cap_count`/
  `cap_children`/`cnode_slot_count`, `Machine::mint` (quyền ⊆ nguồn ⇒ `InsufficientRights`; badge chỉ
  Endpoint/Notification; `derive` kiểm trước khi cấp ⇒ lỗi không rò cap), `check_caps` (bất biến cây
  MDB), **`delete` reparent con cho ông** (seL4 `cteDelete`), **finalise CNode** khi Untyped revoke
  giết CNode (xoá cap trong slot + cả cây con, dọn slot) — không còn cap mồ côi/trỏ object chết.
- `tests/kt10_cnode.rs` (**13 test**): copy/mint/revoke, cha–con, cap chết, slot trống/đầy,
  `RangeError`/`SlotOccupied`/`InvalidCapability`/`InsufficientRights`/`InvalidArgument`, phạm vi revoke,
  delete reparent, finalise CNode (kể cả CNode lồng nhau).
- `tests/kt10_property.rs` (**5 test**): generator LCG tự viết + oracle cây trong test;
  **128 seed × 48 bước = 6.144 thao tác** (copy 1.102, mint 1.109, delete 802, revoke 1.141 xoá 1.093 cap,
  lỗi 762) — kiểm bất biến sau **mỗi** thao tác; 64 seed `Untyped.Revoke` finalise; 2 ca chuỗi sâu;
  tái lập cùng seed.
- `src/kobj.rs` unit: **2 test** chứng minh `check_caps` không rỗng (bắt cap mồ côi, slot không đối xứng,
  cap chết còn giữ slot). Tổng **20 test mới**, 0 ignored.

**F3 (ngoài DoD, chuyển KT-11/K1a)**: quyền mới chỉ cưỡng chế ở `Mint` (⊆ quyền nguồn); `Copy`/`Retype`/
`Delete`/`Revoke` **chưa** kiểm quyền; `mint(Some(0))` giữ badge nguồn (không xoá badge). Không sửa code
trong KT-10.

**Cổng**: [`JOURNAL/kt10-gate-fast.log`](JOURNAL/kt10-gate-fast.log) — `gate --fast` PASS,
**161 test / 0 failed**, L1 **1000 seed / 999 trace** (582 ms, đo bởi cổng); golden + 21 `.scn` không đổi.

**Review**: review độc lập `reviewer-kt10` **APPROVE** (source + test; tự chạy gate PASS 161 test,
property 128×48=6144, staged chỉ `v2/`); yêu cầu sửa tài liệu F1/F2 + ghi F3 — đã sửa.

**Đề xuất**: Agent 0 lật `KT-10 = DONE` sau review commit cuối (không có YC mới; KT-11 mở sau khi DONE).

## KT-11 — `vifast-model` Untyped/CNode, F3 quyền, so sim 10.000 seed (xong mã, 2026-11-13 00:50)

**Vào**: READY, phụ thuộc KT-10 DONE (`c78e814`) · HEAD `f98e1e7` · baseline `gate --fast` PASS
161 test / L1 1000 seed ([`JOURNAL/kt11-baseline-gate-fast.log`](JOURNAL/kt11-baseline-gate-fast.log)).

**Phạm vi**: crate **`v2/lib/vifast-model`** (thêm workspace) — mô hình Untyped/CNode **tái sử dụng**,
không test-only; chạy 4 ca `.scn` Untyped/CNode của KT-04c; F3 (review KT-10); parity test với
`vifast-sim` **10.000 seed**. Không MDB 48 B, không Frame/VSpace/K2; không sửa `spec/`/`QUEUE`/
`DECISIONS`/`AGENTS`; không `#[ignore]`, không nới ngưỡng. Review `reviewer-kt11` **DRIFT** → sửa đủ;
Agent 0 ban hành **D2-44..46**.

**Đỏ trước (D2-05)**:
- [`JOURNAL/kt11-red-model-api.log`](JOURNAL/kt11-red-model-api.log) — 3 test target của crate chưa
  biên dịch (`unresolved imports vifast_model::{Machine, KError, SlotRef, ...}`).
- [`JOURNAL/kt11-red-f3-sem.log`](JOURNAL/kt11-red-f3-sem.log) — **3/3 FAILED** trên code KT-10:
  `mint(Some(0))` giữ `Some(7)` (mong `None`); `Retype` qua cap chỉ READ trả `Ok(3)` (mong
  `InsufficientRights`); cap con `Retype` có `rights = 15` (mong `3`).
- [`JOURNAL/kt11-red-f3-copy.log`](JOURNAL/kt11-red-f3-copy.log) — `copy_rights` chưa có (`E0599`).

**Sản phẩm**:
- `v2/lib/vifast-model/src/{object,error,cap,machine}.rs` — `ObjType`/19 `KError` ánh xạ `vifast-abi`;
  Untyped watermark; CNode `copy`/`copy_rights`/`mint`/`move_cap`/`delete`/`revoke`; `retype`;
  finalise Untyped lồng nhau + CNode; `check_bytes`/`check_caps`; lớp `invoke_*` kiểm quyền cap CNode.
- **F3 (D2-45/46)**: `Copy` có `rights?` (⊆ nguồn); `Retype` đòi **WRITE** + cap con **kế thừa quyền**;
  `Mint(Some(0))` **xoá badge**, badge ≠ 0 chỉ Endpoint/Notification; `Delete`/`Revoke` qua cap CNode
  cần **WRITE** (`[TK]`, K1a rà lại). Sim `kobj.rs` sửa 3 điểm: `retype` WRITE/kế thừa,
  `mint Some(0)` xoá badge, thêm `copy_rights`.
- **D2-44 `CNode.Move`** `[TK]`: `dst` đã có cap thì xoá cap đích rồi chuyển (overwrite) — test kiểm
  cap đích chết, cháu reparent cho ông, không mồ côi; `Copy`/`Mint` vẫn `SlotOccupied`.
- `.scn` runner tối giản trong test (`tests/kt11_scn.rs`) cho `01/02/03/16`: **9/9** bước khớp,
  3 setup `Retype Frame` chưa mô hình (K2). **D2-46** sửa fixture `02-cnode-mint-badge.scn`
  (`02:11` badge trên Untyped → `src=ep0`; `02:12` hai Mint cùng `dst` → thêm `c2`, `dst=c2`) —
  sửa dữ liệu test, không đổi ABI, không hard-code 7/9.
- **Parity test** (không re-export `kobj` của sim): **10.000 seed / 240.000 thao tác, 0 lệch**
  (9065 trace khác nhau, 10,5 s), assert `check_caps` + `check_all_bytes` **sau mỗi bước**:
  [`JOURNAL/kt11-diff-10000.log`](JOURNAL/kt11-diff-10000.log). `Move` không có trong sim nên test riêng.
- 19 test model mới + 4 test sim F3 mới, 0 ignored.

**Cổng**: [`JOURNAL/kt11-gate-fast.log`](JOURNAL/kt11-gate-fast.log) — `gate --fast` PASS,
**184 test / 0 failed**, L1 **1000 seed/999 trace** (505 ms); fmt/clippy/R1..R5 sạch.

**Đề xuất**: **REVIEW** (reviewer `reviewer-kt11` DRIFT → đã sửa đủ 5 mục; chờ xác nhận lần cuối;
không tự nhận DONE, không lật QUEUE). **Không còn YC**: D2-44/45/46 đã chốt.

## Việc đang làm — KT-T26 (3 h theo QUEUE/PATCH-BRIEF/thiết kế)

DoD đủ 5 mệnh đề (khôi phục theo review #7 — không được tự rút): (1) host test `vifast-fs`;
(2) **block giả + cache**; (3) `MAX_IO = 256`; (4) đọc **RAW** kiểm read-your-own-write;
(5) đánh **"ai chen giữa"** theo D-03.

**Đã làm**: `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` (3 test, 1.000 seed, `CachedDev` đang ship,
ca âm **mất ghi im lặng** đã đỏ khi đảo kỳ vọng) + `v2/docs/kt-t26-analysis.md` (bảng kẻ chen giữa).
**Chưa làm được trong crate**: tái hiện đầy đủ T26 (`-smp 2`, 50 boot) — **YC-06**.

## KT-04c — `.scn` xong phần viết được; arity BLOCKED (YC-05)

- `v2/docs/scn-format.md` (`[TK]`, `version = 1`, chiếu §9.3), `v2/test/scn/check_scn.py` + 3 fixture âm,
  **21 tệp** `.scn`, gắn vào `v2/bin/v2 check` (bước "scn" + "scn fixtures").
- `18-schedcontext-donation.scn` **đã mở** (SchedControl lên lõi ở KT-04b): `SchedControl.Configure` + `SchedContext.Bind`.
- **Arity + trạng thái**: xong ở KT-04c bằng draft `v2/docs/abi-call-signatures.toml` (D2-33); `check_scn.py`
  kiểm đủ tham số/không lạ/variants và `expect_states` có trường; 21/21, 0 blocked.

## KT-04b — BLOCKED chờ YC-04 (không ghi `spec/`)

- Sản phẩm hợp lệ đã làm: draft `v2/docs/abi-reserved-objects.toml` (9 object reserved + SchedControl, id 8..16,
  `unsupported = true`) + `v2/test/abi/check_reserved.py` gắn vào `v2/bin/v2 check`.
- **YC-04** (BLOCKED): xin Agent 0 (a) cho KT-04b ghi `spec/abi.toml`, hay (b) Agent 0 tự chép draft; kèm câu hỏi
  "VSpace khách = object riêng hay method" và đề nghị lật `QUEUE:102/103/104`.
- Máy kiểm đã vá 2 lỗ (trùng id, ngoài dải) + gỡ bom `status == "REVIEW"`.

## Đã xong — KT-04a (`spec/abi.toml` lõi + bảng ánh xạ)

- `spec/` ở **gốc repo** (AGENTS.md §1/§4.1 — bậc cao hơn cây §8 của thiết kế): `version/abi_version/min_compat = 1`,
  `status = "REVIEW"`, `[frozen]` đủ 5 thứ, `[message]` 64 MR + 4 cap, `[message_info]` 35 bit,
  `[calling.riscv64]` lấy từ code v1 (`a7` số hiệu, `a0/a1` tham số, `a2` MessageInfo, `a3..a6` 4 MR),
  `[object]` lõi **1..8** (SchedControl 8 = lõi theo D2-32), `[object_reserved]` 9..16 + 17..31, `[error]` 19 mã, `[syscall]` 0..15, reserved 16..31.
- Bảng ánh xạ: `v2/docs/abi-v1-v2-mapping.md` (25 crate; 58 hàng = 12→9 số syscall + 27 hàng method (23–24 đích, chưa chuẩn) + 19 bỏ; 22 biến thể KError → 0 + 19).
- Máy kiểm: `v2/test/abi/check_abi.py` (Python ≥3.11 `tomllib`), **gắn vào `v2/bin/v2 check`** — `check` nay có 6 bước.
- Đã đóng: **YC-02** (D2-29: DebugGetInfo = syscall 15), **YC-03** (D2-30/D2-31: đánh số lại; `spec/` ở gốc repo).

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-14 | **KT-44** | 8/8 | `efe128fb` | — | `gate --fast` PASS (**569 test/73 suite**, L1 1.000 seed; baseline cùng số) | **xong mã — chờ Agent 0 lật sổ**; `vifast-plat-common` no_std/`forbid(unsafe)` + trait `Platform`/`PanicHandler`; 2 ảnh bằng feature `ktest` (sản phẩm 0 chuỗi `ktest`, ảnh test 8); budget `data+bss 0 B`/262.144, `.stack` 65.536 + guard 4.096 báo riêng; TAP 1 boot ×2 ảnh; host test 3/3 |
| 2026-11-13 | **KT-30** | 8/8 | `29718a6` | — | `gate --fast` PASS (**461 test/62 suite**, L1 1000 seed/999 trace, 750 ms; wall 199,3 s — **T-INFRA** tải máy) | **xong mã — chờ Agent 0 lật sổ**; `vifast-sync` `no_std` **1.346 dòng** (src 845, test 501), **0 dependency**, 8 khối `unsafe` có `SAFETY`; MCS lock FIFO qua **pool node cố định 16 slot** (không con trỏ thô, `Guard` giữ chỉ số slot), **không reentrant**; `Mailbox<N>` MPSC **7 loại** (`Wake/TlbFlush/Reschedule/Migrate/Stop/VmKick/PowerReq`), đầy ⇒ `Busy` (không mất message đầu), **ack thế hệ** `AtomicU32`; stress 4×500 (≈2,6 ms) + **200 lần / 0 fail**; Miri **16/16**; **reviewer-kt30 DRIFT đã sửa 2/2 chặn** (assertion stress; hook test `pub`→`pub(crate)`+`cfg(test)`); **không** loom/explorer (D2-53 Q9(a)) |
| 2026-11-13 | **KT-28** | 10/10 | `ff94538` | — | `gate --fast` PASS (**427 test/56 suite**, L1 1000 seed/999 trace, 704 ms) | **commit `ff94538` đã push**; WaitOnAddress bảng băm `(frame, offset)` 64 bucket hằng model `[TK]` (không tạo `spec/kernel.toml`), `wait_addr` đọc giá trị + xếp hàng một lần (expected lệch ⇒ `InvalidArgument` không xếp hàng), **không timeout** (D2-53 Q4), `wake_addr(n)` FIFO tối đa n, `check_waitaddr` sau mỗi op, API K1b giữ nguyên; 12 test + fuzz **1.000 seed × 64 op**, Miri **12/12** + lib **2/2**; đột biến "xếp hàng trước rồi so" bị bắt; **reviewer-kt28 APPROVE**; chờ Agent 0 lật sổ |
| 2026-11-13 | **KT-23** | 7/8 | `67aa8bc` | — | `gate --fast` PASS (**321 test/47 suite**, L1 1000 seed/999 trace, 687 ms) | **xong mã — chờ review độc lập rồi mới commit**; coverage **46/46** (bảng 48/48) + **73/73** cặp cấm (`forbidden_attempts=73`), fuzz 1.000×64 = **256.000** kiểm 4 miền/0 vi phạm; **bug fuzz tìm ra đã sửa** (Exit fault giao thẳng Recv waiter); Miri **12/12** (2.608 s); parity 10.000 seed/0 lệch (chỉ Untyped/CNode); `.scn` 21 tệp/11 nhóm, fixture 12 sửa **1 dòng** (D2-52); DoD K1b: 3/4 ĐẠT, push migration **hoãn K1c** (D2-49(a)) |
| 2026-11-13 | **KT-21** | 8/8 | `d547bbf` | — | `gate --fast` PASS (**298 test/45 suite**, L1 1000 seed/999 trace, 550 ms) | donation tự động Call/Reply + passive server + MCP (model-only `[TK]`); **13 test** (invariant **1.000 seed × 64 op**, **14.609** bước có donation), Miri **13/13** (129 s); commit `eae876b` + follow-up `[KT-21]` (3 điều kiện `reviewer-kt21`) đã push; **chờ Agent 0 ghi D2-51 + lật sổ** |
| 2026-11-13 | **KT-18** | 8/8 | `5cccb72` | — | `gate --fast` PASS (**251 test/42 suite**, L1 1000 seed/999 trace) | **commit `5cccb72` đã push**; TCB 11 trường + **một cửa** `transition_tcb` + bảng `ALLOWED` 48 = **46+2 reserved**, 18 test (ma trận 121 cặp, coverage bảng 100%), Miri kt18 **18/18**; drift D2-33 sửa; reviewer DRIFT nhẹ đã sửa; chờ Agent 0 lật sổ |
| 2026-11-13 | **KT-15** | 8/8 | `c0d3163` | — | `gate --fast` PASS (**218 test**, L1 1000 seed/999 trace, 596 ms) | **commit `c0d3163` đã push — reviewer DRIFT nhẹ đã sửa, chờ Agent 0 lật sổ**; `with_many_mut` D2-48(a): bảng `kind→ctor`, alias từng cặp, N=9 không build; Miri PASS (alias 8/8, fuzz 200/191,95 s); fuzz 10.000 chuỗi/4,51 s; `aarch64-none` xanh; guard `chua_header` + `TypeId` |
| 2026-11-13 | **KT-14** | 8/8 | `e1921ab` | — | `gate --fast` PASS (199 test, L1 1000 seed/999 trace, 433 ms) | **DONE** `36c51bc`; `vifast-objmem` 15 test, Miri 15/15, build `aarch64-unknown-none`; reviewer DRIFT nhẹ đã sửa; **YC-18** chốt **D2-48** |
| 2026-11-13 | **KT-13** | 8/8 | `5ad86e5` | — | `check` + `gate --fast` PASS (184 test, L1 1000 seed/999 trace, 603 ms) | **reviewer-kt13 APPROVE — chờ Agent 0 lật sổ**; `v2 miri` (D2-47(a): lệnh rời, không trong `gate --fast`); Miri PoC 13 test PASS; nhánh thiếu toolchain fail rõ |
| 2026-11-13 | **KT-11** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace, **184 test**) | **DONE** `035584b`; `vifast-model` Untyped/CNode + F3; parity sim **10.000 seed/0 lệch**; `.scn` **9/9** (D2-44..46) |
| 2026-11-12 | **KT-10** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace, 161 test) | **xong mã — chờ Agent 0**; CNode/MDB test-first: `mint`, `check_caps`, delete reparent, finalise CNode; **20 test mới**, property 6.144 thao tác |
| 2026-11-12 | **KT-09b** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace) | sửa review Agent 0: `TcbState` đủ 11 variant (test spec), `check_bytes` siết allocated/gap, revoke Untyped lồng nhau; **29 test** |
| 2026-11-12 | **KT-09** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace) | **xong mã — chờ Agent 0 lật sổ** (D2-37); Untyped/Revoke/e2e + C5; **27 test**; fixture âm kiểm thông điệp |
| 2026-11-12 | **KT-12b** | 8/8 | — | — | `gate --fast` PASS (có bước no_std) | **xong mã — chờ Agent 0 lật sổ** (D2-37); no_std + std gating đúng D2-42; gen sinh TRACE_* từ `[trace]` |
| 2026-11-12 | **KT-12** | 8/8 | — | — | `check`+`gate --fast` PASS | **DONE**: `vifast-traceformat` (Record 32 B D2-38, ring, decode_all) + golden nhị phân 200×32 B + cross-check text↔nhị phân |
| 2026-11-12 | **KT-05** | 8/8 | — | — | `check`+`gate --fast` PASS (gen-check ON) | **DONE**: `[method_signature]` stable-draft vào spec; `vifast-gen` sinh code+`docs/v2/abi.md`; gen-check so byte + 3 fixture âm |
| 2026-11-12 | **KT-R1** | 8/8 | — | — | `check` PASS (báo cáo) | **DONE**: `R1-v2.md` 6 mục + `R1-v2.json` 11 khoá (project v2); `check_report.py` nối vào `v2 check`; ghi cả hai trạng thái khi sổ lệch |
| 2026-11-12 | **KT-08** | 8/8 | — | — | `gate --fast` PASS (**L1 1000 seed**) | **DONE**: `v2/sim/vifast-sim` (store giả + scheduler 1 hart, trace băm FNV-1a); `v2 sim` CLI; **L1 1.000 seed bật trong gate --fast**; 7 test unit + 1 test L1 |
| 2026-11-12 | **KT-06** | 8/8 | — | — | `check` PASS (agentctl 9/9 test) | **DONE** (sửa theo review #10): `vifast-agentctl next/list/report/start/done/block` (đọc mặc định; `--write` mới ghi; `start` chỉ READY); parser nhận **KT- và ST-**, chuẩn hoá `**DONE**`, `set_status` giữ escape; `v2 agentctl` nối đúng dispatch; **13/13** test |
| 2026-11-12 | **KT-T26** | 8/8 | — | — | `check` PASS + 3 test T26 | **DONE đèn host** (1.000 seed, RAW, ca âm mất ghi im lặng); **tái hiện đầy đủ BLOCKED** (YC-06: crate không có VFS/smp) |
| 2026-11-12 | **KT-04c** | 8/8 | — | — | `check` PASS (scn 21 tệp) | **DONE đủ**: 21 `.scn` 0 blocked, arity theo draft D2-33, 4 fixture âm, ca 18 mở |
| 2026-11-12 | **KT-04b** | 8/8 | — | — | `check` PASS | **BLOCKED** (YC-04): không được ghi `spec/`; draft id 8..16 + `check_reserved.py` gắn cổng; vá 2 lỗ máy kiểm (trùng id/ngoài dải), sửa 2 sai số KT-04a |
| 2026-11-12 | KT-04b | 1–7/8 | `a8bb272` | — | — | review#5 **DRIFT** → dừng trước `spec/`, chuyển draft + YC-04; đỏ trước: 9 object reserved thiếu |
| 2026-11-12 | **KT-04a** | 8/8 | `545f3b4` | — | `gate --fast` PASS | **DONE**: `spec/abi.toml` (REVIEW) + 3 bảng ánh xạ + `check_abi.py` gắn cổng; 4 điều kiện review #4 xử lý |
| 2026-11-12 | KT-04a | 1–7/8 | `545f3b4` | — | — | review#4 ON-TRACK; đỏ trước `abi-check: KHÔNG có spec/abi.toml`; quy ước gọi lấy từ code v1 (4 MR) |
| 2026-11-12 | **KT-07** | 8/8 | `c9cdc9e` | — | boot 50/50 + `gate --fast` | **DONE**: K0.5 boot riscv64 qua OpenSBI; TAP 1 test; `image 14.512 B`, `.data+.bss = 0 B` (trần 256 KiB), `.stack 64 KiB` NOLOAD báo riêng |
| 2026-11-12 | KT-07 | 1–7/8 | `c9cdc9e` | — | — | review#3 ON-TRACK (3 điều kiện) → đỏ trước (`chưa có crate kbin`); `vifast-plat-riscv64` (unsafe, UART/SBI) + `vifast-kbin-riscv64` (safe, linker 0x80200000); kernel workspace tách khỏi v2 |
| 2026-11-12 | **KT-03** | 8/8 | `785996a` | — | `gate --fast` PASS | **DONE**: wrapper `v2/bin/v2` (check/gate --fast/rules; gate đầy đủ + nightly từ chối), 5 luật R1..R5 + self-test 6/6, `check` PASS, `gate --fast` 4,1 s |
| 2026-11-12 | KT-03 | 1–7/8 | `785996a` | — | — | review#2 ON-TRACK (5 điều kiện) → đỏ trước 6/6 (`chưa có v2/bin/v2`); viết fixture + run.sh; sửa R2/R3/`--manifest-path`; `JOURNAL/review-2026-11-12-2.md` |
| 2026-11-12 | **KT-02** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: workspace `v2/` + `bin/` + port `vifast-fs`(10 tệp, max 1.248 dòng)/`vifast-fsutil`/`vifast-elf`; commit `b4a8693`, `054c3e2` |
| 2026-11-12 | KT-02 | 2–7/8 | `329177e` | — | cổng chưa có | tách 4.543 dòng v1 → 10 tệp; `cargo fix` gỡ 13 import; D2-27/D2-28 nhận trong lúc làm |
| 2026-11-12 | KT-02 | 1/8 | `329177e` | — | cổng chưa có | lập kế hoạch nhỏ 4 bước; review#1 ON-TRACK |
| 2026-11-12 | **KT-01** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: tag annotated `v1-archive` trên `ca2eaf5` (subject "T33a mở"); 7/7 mục KNOWN-ISSUES khớp QD §6; test đỏ **N/A** — commit `19659ee` |
| 2026-11-12 | KT-01 | 3–7/8 | `329177e` | — | cổng chưa có | đỏ trước: `git tag -l v1-archive` rỗng; tạo tag; dán review#1; sửa câu chữ Exit + mở Q-01 |
| 2026-11-12 | KT-01 | 1/8 | `329177e` | — | cổng chưa có | mở vòng 1: đọc 7 tệp + STATE + review subagent trước task đầu |
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 17 dòng task, 5 READY (KT-01, KT-02, KT-04a, KT-T26, KT-R1), KT-04d REVIEW |
