# KT-49 — SMP: SBI HSM + PerHart (K3) — nhật ký

Người làm: **Agent 2 dòng OS** (phiên KT-49). HEAD vào: `808695a0` (`[KT-00]` mở KT-49; parent code
KT-48 `aec27f73`). Working tree có `viscript/` của tab khác — **không chạm**; chỉ stage `v2/`.
`git rev-list --count HEAD..origin/main` = **0** trước khi làm.

Nguồn: `review/agent0/K3-PLAN.md` KT-49 (§4.1) + **D2-72 Q4** (hart theo DTB, `MAX_HARTS = 8`,
matrix `1/2/4`), Q7 (DTB chỉ kiểm magic), Q5 (budget 262.144 B), Q12 (Miri không áp kernel);
D2-74 (icount chỉ trong `ktest.sh`).

## 1. Baseline (trước khi sửa)

- `./v2/bin/v2 gate --fast` → **PASS 569 test** ([`kt49-baseline-gate-fast.log`](kt49-baseline-gate-fast.log)).
- `bash v2/test/boot/ktest.sh 1 1` (ảnh KT-48) → PASS 15 TAP chính + 3 TAP guard
  ([`kt49-baseline-ktest.log`](kt49-baseline-ktest.log)).
- Working tree `v2/` sạch; `viscript/` + `review/` + `out-b.log`… của tab khác **không chạm**.

## 2. Đỏ trước ([`kt49-red-ktest.log`](kt49-red-ktest.log))

Dựng worktree tạm ở `aec27f73` (KT-48), **chỉ chép runner mới** (`ktest.sh` + `expected.txt`) rồi
chạy `ktest.sh 1 1` trên ảnh KT-48:

```
not ok - ktest smp=1 boot 1: thiếu plan-1..20 ok-set-1..20 ok-count=15/20 ok15-harts-alive-1
  ok16-stack-guard ok17-per-hart-timer ok18-plic-ctx ok19-barrier ok20-nested
  all-hart-layout-evidence smp-bringup-evidence harts-alive-evidence … hart0-alive-line
  hart0-stack-line timer-hart-lines plic-hart-lines
not ok - guard smp=1 boot 1: thiếu guard-load-real
# KTEST: 0/2 boot sạch (smp=1, NBOOT=1)
```

Log ảnh cũ: [`kt49-red-main-raw.log`](kt49-red-main-raw.log), [`kt49-red-guard-raw.log`](kt49-red-guard-raw.log).
Đỏ vì **thiếu tính năng** (chưa HSM/PerHart/TAP 15..19), không phải do hạ/nới ngưỡng. Worktree đã
xoá (`git worktree remove --force`, prune).

## 3. Thay đổi

### 3.1 `vifast-plat-common` (không `unsafe`)

- **`src/smp.rs` (mới)**: `MAX_HARTS = 8` `[TK]`; hằng số record per-hart `HART_GUARD_BYTES =
  0x1000`, `HART_STACK_BYTES = 0x10000`, `HART_TRAP_STACK_BYTES = 0x4000`, `HART_STRIDE =
  0x1_5000`, `HART_AREA_BYTES = 0xA_8000`; offset/`guard_owner`/`is_guard_offset` thuần; `HartMask`
  (`AtomicU64`, insert/remove/contains/bits/count/lowest/highest); hằng **SBI HSM** (EID `0x48534D`,
  FID 0/1/2), `HartStatus`/`start_decision` (hart `-3 INVALID_PARAM` ⇒ **Reject**)/`hart_start_ok`
  (`0` hoặc `-6`); `const` assert bố cục (record không chồng, ≤ 2 MiB).
- **`timer.rs`**: `TIMER_STATS: PerHartTimerStats` (`slots: [TimerCounters; MAX_HARTS]`, `get(hart)`,
  `try_get`, `total_ticks`, `total_late`) — KT-47 là một bộ đếm toàn cục.
- **`plic.rs`**: `PLIC_STATS: PerHartPlicStats` (`get(hart)`, `total_claims`, `total_uart_irqs`).
- **`tests/smp.rs` (mới, 9 test)**: bố cục record không chồng/căn trang, guard owner, `HartMask`
  + hart ngoài dải, phân loại trạng thái HSM + `start_decision` (từ chối hart ngoài DTB, không
  panic), `PerHartTimerStats`/`PerHartPlicStats` không lẫn. **Host test: 46 → 55**.

### 3.2 `vifast-plat-riscv64` (asm/CSR/SBI, `unsafe` thuộc D2-17)

- **`lib.rs`**:
  - `_start` tính `sp`/`sscratch` từ **record của hart trong `a0`** (`HART_STRIDE` bằng dịch/cộng,
    không cần M), zero `.bss`, lưu `a1` (DTB) vào `SMP_DTB` sau zero, gọi `rust_main`.
  - `_start_secondary` (mới, 4-byte aligned): `tp ← hartid`, cùng phép tính record, `sscratch`,
    `call rust_secondary_main` (bin cung cấp).
  - `trap_entry`: `sscratch = frame + TF_SIZE` (trap top **của hart này**, không symbol toàn cục);
    phát hiện trap lồng trong `[trap_top − HART_TRAP_STACK_BYTES, trap_top)`; khôi phục `sscratch`
    theo `sp + TF_SIZE`. `trap_dispatch` ghi `smp::note_trap_frame(hart, frame)` mỗi trap.
  - `debug_lock_do`/`debug_put_str_atomic` (khoá UART cho dòng alive của từng hart),
    `park_forever`.
- **`smp.rs` (mới)**: `SMP_DTB`; `dtb_magic_ok` đọc FDT qua **physmap** (QEMU đặt FDT ở
  `0x87e0_0000`, ngoài identity-map); `hart_start`/`hart_status` (ecall HSM); `init` (probe
  `1..MAX_HARTS`, start hart `STOPPED`, từ chối hart ngoài DTB, chờ `ONLINE ⊇ PRESENT` timeout
  2 s); `secondary_setup` (kiểm opaque `"KT49"`, `mmu::adopt_satp`, `stvec`, online); `release`/
  `finish`/`wait_release`; ticket; `note_trap_frame`; barrier `begin/join/wait_arrivals/release`
  (`released` **tăng đơn điệu**, không reset); `SmpReport`.
- **`mmu.rs`**: import `__hart_area_base` + hằng `smp`; `Layout` kiểm linker khớp hằng (guard hart 0
  ở base, `__kernel_end == base + HART_AREA_BYTES`, panic nếu lệch); `is_guard_page`/`guard_owner`;
  `stack_range_of`/`trap_stack_range_of`/`guard_range_of`/`stack_top_of`/`trap_top_of`/`hart_area_base`;
  map mọi trang NOLOAD trừ **guard từng hart** (identity + nửa cao), khoét **mọi** guard khỏi
  physmap; `adopt_satp`; `handle_guard_fault` chỉ resume load fault nếu `guard_owner(stval) ==
  hart_id()` và in `hart=`; `is_guard_fault` theo owner.
- **`timer.rs`**: mọi thao tác dùng `TIMER_STATS.get(hart_id())` (`stats()`).
- **`plic.rs`**: `stats() = PLIC_STATS.get(hart_id())`; `init_hart()` (threshold 0 cho context
  `2*h+1`, đọc priority, enable round-trip IRQ UART trên context của hart, kiểm context M-mode
  `2*h` tắt, claim rỗng) + `PlicHartReport::is_configured` + `context_enabled`.

### 3.3 Ảnh test / sản phẩm / linker

- **`linker.ld`**: vùng NOLOAD `ALIGN(2M)` = **8 record** `guard|stack|trap stack`
  (`__hart_area_base`, `.stack_guard`/`.stack`/`.trap_stack` = hart 0 như cũ, `.hart_stacks` =
  7×0x1_5000, `__kernel_end = base + 0xA8000`). NOLOAD ⇒ không cộng budget.
- **`ktest.rs` → 20 TAP**: 1..14 giữ nguyên (KT-45/46/47/48; TAP 6 mở rộng kiểm **mọi** hart:
  guard NONE 3 alias + stack/trap RW NX + owner, `all_harts=1`), **15..19 = SMP** (gọi
  `ktest_smp::smp_phase()`), **20 = trap lồng** (đổi số từ 15). Plan `1..20`.
- **`ktest_smp.rs` (mới)**: `rust_secondary_main` (setup → alive line → guard turn → timer 100 tick
  → plic ctx → barrier 5/6 → park) + `smp_phase` trên BSP: `smp::init`, log `# smp:` + từng hart bị
  từ chối, release, TAP 15..19. Barrier: vòng 5 có hart 1 trễ **5 ms**, vòng 6 kỳ vọng +1 bit hart
  **không online** ⇒ timeout **200 ms** rồi release (không treo). Mọi pha có timeout.
- **`main.rs` (product)**: sau timer, `smp::init()` + in `# smp:` (kèm `dtb/dtb_magic`); hart lỗi
  chỉ log `# smp: WARN` rồi **tiếp tục** (hart lỗi không panic cả máy); `rust_secondary_main` =
  setup + park.
- **`ktest_guard.rs`**: thêm `rust_secondary_main` (setup + park; ảnh guard không gọi `smp::init`).
- **`ktest.sh`**: đọc `expected.txt` (`ktest 20 20`, `ktest-guard 3 3`) + sidecar `.license`;
  kiểm plan/tập `ok 1..K`/đếm `ok`; **matrix** `SMP=matrix` → `1 2 4`, log riêng
  `/tmp/k3-ktest-{main,guard}-smp<N>-<i>.log`; kiểm TAP 15 = `harts alive N`, `# smp:` khớp N,
  `# harts alive`/`# stack hart`/`# timer hart`/`# plic hart`/`# barrier`, và **OpenSBI
  `Platform HART Count = N`** (bằng chứng độc lập).
- **`run.sh`**: kiểm `# smp: … harts=N … timeout=0 … dtb_magic=1` trên ảnh sản phẩm.

## 4. Kết quả đo (do script in — không chép tay vào docs)

### 4.1 Matrix 50 boot **mỗi** N ([`kt49-boot50-matrix.log`](kt49-boot50-matrix.log))

```
# smp=1: 100/100 boot ảnh test sạch
# smp=2: 100/100 boot ảnh test sạch
# smp=4: 100/100 boot ảnh test sạch
# KTEST: 300/300 boot sạch (smp=matrix, NBOOT=50)
# ktest.sh: PASS
```

(300 = 50 main + 50 guard cho **mỗi** N=1/2/4; 0 `not ok`, 0 PANIC ngoài dòng có chủ đích.)
Sau khi tách `ktest_smp.rs` (chỉ đổi tổ chức tệp, không đổi ngữ nghĩa), chạy lại `ktest.sh 10
matrix` → **60/60 sạch** ([`kt49-ktest-postsplit-10x3.log`](kt49-ktest-postsplit-10x3.log)) và
`ktest.sh 1 matrix` → 6/6.

Bằng chứng SMP theo N (trích log đại diện — `kt49-ktest-smp{1,2,4}.log`):

- `# smp: … harts=1/2/4 … present=0x1/0x3/0xf … timeout=0 … dtb_magic=1 base=0x80400000
  stride=0x15000 area=0xa8000`; hart ngoài DTB bị từ chối: `-smp 1` → 7 dòng `status=-3 … ngoai DTB
  — tu choi start`; `-smp 4` → 4 dòng.
- `# harts alive: n=N mask/alive=0x… tickets=1 arrived=0x… timeout=0`; mỗi hart in
  `# hart h alive ticket=T tp=h …`.
- `# stack hart=h … owner=h frame_trap=1 rw_nx=1 guard_none=1` cho **từng** hart (guard riêng
  `0x8040_0000`, `0x8041_5000`, `0x8042_a000`, `0x8043_f000`; frame trong trap stack của chính
  hart: `0x80414ee0`, `0x80429ee0`, `0x8043eee0`, `0x80453ee0`).
- `# timer hart=h ticks=100 … ok=1` cho mọi hart; ở N=4, hart phụ có thể có `late/missed > 0`
  (log đại diện `kt49-ktest-smp4.log`: hart 3 `late=1 missed=2`; giá trị đổi theo lượt chạy) — hạn
  chế **chia lượt vCPU của icount**; TAP 17 chỉ đòi ≥100 tick + slot riêng, **không** đòi drift
  per-hart (drift [9,11] ms chỉ đo trên BSP — D2-74; ghi §12.8.4).
- `# plic hart=0..3 ctx=1/3/5/7 mmode_ctx=0/2/4/6 threshold=0 priority_uart=1 enable_on=1
  enable_off=1 mmode_off=1 ok=1`; `uart_irqs_boot=8 stats_clean=1` (boot hart có thể ≠ 0 — xem §5.5).
- `# barrier: round=5 arrived=0x… timeout=0 wait_ticks=… slow_hart=1 slow_delayed=1` (hart 1 trễ
  **5 ms** thật; N=4 `wait_ticks≈676.7k` = ~67,7 ms ảo — gồm trễ 5 ms + chia lượt vCPU);
  `# barrier timeout: round=6 … timeout=1 wait_ticks=2000xxx (may khong treo)`.

### 4.2 Ảnh sản phẩm — **không icount** (D2-72 Q2)

- `run.sh 50 1` → **50/50** ([`kt49-product-boot50-smp1.log`](kt49-product-boot50-smp1.log)).
- `run.sh 50 4` → **50/50** ([`kt49-product-boot50-smp4.log`](kt49-product-boot50-smp4.log)); log
  boot có `# smp: boot=0 harts=4 … timeout=0 dtb_magic=1`.

### 4.3 Host / budget / cổng

- `cargo test -p vifast-plat-common` → **55/55** ([`kt49-host-tests.log`](kt49-host-tests.log)).
- Budget (`llvm-size -A`, [`kt49-budget.log`](kt49-budget.log)): product `data+bss=1752`/`file=64400`;
  ktest `2648`/`98872`; ktest-guard `1600`/`52376`; trần **262144** — tất cả **dưới trần**;
  `.stack`/`.stack_guard`/`.trap_stack`/`.hart_stacks` NOLOAD báo riêng.
- `./v2/bin/v2 gate --fast` → **PASS 569 test** ([`kt49-gate-fast.log`](kt49-gate-fast.log)); fmt,
  clippy, luật cấu trúc R1..R5 (đã tách `ktest_smp.rs` để ≤ 1500 dòng; sidecar
  `expected.txt.license` cho R3), gen-check, L0, L1 1.000 seed.

## 5. Bug gặp trong lúc làm (đã sửa + ghi lại)

1. **Đọc DTB fault**: `a1 = 0x87e00000` (FDT cuối RAM) **ngoài identity-map** ⇒ load page fault
   khi kiểm magic. Sửa: đọc qua **physmap** (`mmu::physmap_alias`) — D2-72 Q7 vẫn chỉ kiểm magic,
   không thêm mapping, không parse FDT.
2. **Barrier release bị reset**: bản đầu `barrier_begin` đặt `released = 0`; BSP sang vòng mới
   trước khi AP đọc release cũ ⇒ AP chờ hết timeout, `smp=2/4` đỏ TAP 18/19 (arrived thiếu).
   Sửa: `released` **tăng đơn điệu** theo vòng, không reset. Sau sửa 300/300 boot sạch.
3. **Boot hart không tự online**: `ONLINE` chỉ AP set ⇒ `harts=0` ở `smp=1`; sửa `smp::init` set
   bit boot hart (hart boot đã chạy S-mode).
4. **Vòng 6 timeout quá ngắn (200 µs)**: dưới icount, vCPU được chia lượt thô, hart chậm có thể
   tới muộn ~100 ms ảo ⇒ AP chưa kịp join. Tăng timeout vòng 6 lên **200 ms** (vẫn bounded, vẫn
   chứng minh "không treo"); **không** nới ngưỡng DoD nào (DoD không đặt ngưỡng thời gian barrier).
5. **Boot hart không phải 0**: ảnh sản phẩm `-smp 4` **không icount** có lần OpenSBI chọn
   `Domain0 Boot HART: 2`; kernel đã hỗ trợ từ đầu (mọi record/`ONLINE`/ticket dùng `a0`/`hart_id()`
   thật, `_start` tính `sp`/`sscratch` theo hart). Reviewer bắt thêm: ảnh **guard** còn hardcode
   `mmu::guard_range()` (hart 0) — đã sửa dùng `guard_range_of(hart_id())` + runner đọc hart boot từ
   `# guard image: hart=N`; TAP 1/2 đòi `guard_owner == hart_id()`. Tái hiện được boot hart ≠ 0
   **không icount** (`-smp 4`, 6 lần: 1 lần `Domain0 Boot HART: 3`) — ảnh guard vẫn 3/3 TAP, fault
   line `hart=3` ([`kt49-guard-boothart3.log`](kt49-guard-boothart3.log)). TAP 18/evidence cũng đổi
   từ hardcode hart 0 sang **hart boot** (`uart_irqs_boot=…`); runner chỉ đòi `harts=N`. Ảnh sản
   phẩm: hart lỗi chỉ log `# smp: WARN` (không panic cả máy), `run.sh` đỏ nếu `harts≠N`.

## 6. Review độc lập `reviewer-kt49` — **DRIFT**, 4 mục đã vá trước commit

Reviewer tự chạy host 55/55, `ktest.sh 1 matrix` 6/6, `run.sh 2 4` 2/2, gate 569, đọc lịch sử
300/300; kết luận **DRIFT** (không UNSAFE), DoD chính xanh, yêu cầu 4 mục:

1. **Sai đơn vị "hart chậm 5 ms"**: `SLOW_DELAY_TICKS` là `TICK_TICKS/20` = **0,5 ms** nhưng docs/
   journal ghi 5 ms. Chọn **làm đúng 5 ms** (`TICK_TICKS/2` = 50.000 tick) thay vì sửa chữ xuống
   0,5 ms (0,5 ms nằm trong nhiễu icount, không chứng minh "hart chậm"); comment + docs §12.5/
   §12.8.3 + journal đã theo số thật. Sau vá: N=4 `barrier round=5 wait_ticks≈676.7k` (trước
   ~450k) — trễ có tác dụng.
2. **Ảnh guard hardcode hart 0**: `ktest_guard.rs` dùng `mmu::guard_range()` (hart 0) và runner
   ghim `hart=0`, mâu thuẫn boot-hart tổng quát. Sửa: guard image dùng
   `mmu::guard_range_of(hart_id())`, TAP 1/2 đòi `guard_owner == hart_id()`; `# guard image: hart=N`
   + `# guard load:/store: hart=N`; `ktest.sh` đọc hart từ `# guard image:` và đòi fault line đúng
   hart đó (không ghim 0), kiểm hart nằm trong `Platform HART Count`. **Tái hiện boot hart ≠ 0**:
   ảnh guard `-smp 4` **không icount**, 6 lần có 1 lần `Domain0 Boot HART: 3` → vẫn **3/3 TAP**,
   fault `hart=3` ([`kt49-guard-boothart3.log`](kt49-guard-boothart3.log)).
3. **Docs thiếu giới hạn timer PerHart dưới icount**: thêm §12.8.4 — N=4 slot timer hart phụ có
   thể `late/missed > 0` do **chia lượt vCPU của icount**; TAP 17 chỉ đòi ≥100 tick + đúng slot,
   **không** đòi drift per-hart; drift [9,11] ms chỉ đo trên BSP (D2-74); chưa tách biến ⇒ task
   riêng.
4. **Nit**: comment `main.rs` còn nói PANIC nếu present-but-offline (code chỉ WARN) — đã sửa; §12.1
   "hart boot (hart 0)" → cho phép khác 0 (đo được `Domain0 Boot HART: 2/3`).

Sau vá: host **55/55**, `ktest.sh 10 matrix` **60/60** ([`kt49-ktest-postdrift-10x3.log`](kt49-ktest-postdrift-10x3.log)),
`ktest.sh 1 matrix` 6/6, gate **569**, budget dưới trần; product 50/50 smp1 + 50/50 smp4 (log đã
lưu) — không hồi quy KT-44..48 (TAP 1..14 giữ nguyên + 300/300 lịch sử).

## 7. Không làm (đúng phạm vi)

IPI (KT-50), ASID/TLB shootdown (KT-51), object layer (KT-52), U-mode (KT-53), harness TAP đầy đủ/
conformance (KT-54/55), `FaultKind`/W^X (KT-56), soak 1.000/K3-DOD (KT-57), port core (K3b),
hotplug/suspend/scheduler. `spec/`, sim, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`, `review/`
**không chạm**; không thêm dependency; không `cfg(target_arch)`; `unsafe` chỉ trong
`kernel/vifast-plat-*`.
