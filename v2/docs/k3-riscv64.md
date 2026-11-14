<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K3 — kernel riscv64 thật trên QEMU `virt` (mốc mở bằng KT-44)

Trạng thái: **KT-49** — **SMP thật**: SBI HSM `hart_start` cho hart `1..N-1`, `N = 1/2/4`,
**PerHart** (stack + `.trap_stack` + guard riêng mỗi hart, `TP`/`sscratch`/`stvec` per-hart, bộ đếm
timer/PLIC theo hart, PLIC context `2*h+1`), `HartMask` + barrier có timeout (hart lỗi/chậm **không
treo máy**), trên nền trap KT-45 + MMU Sv39 KT-46 + timer SBI KT-47 + PLIC/UART IRQ KT-48; tài liệu
hằng số nền tảng + quy trình build/boot/trap/MMU/timer/PLIC/SMP của K3.
Nguồn quyết định: **D2-72** (`v2/agent/DECISIONS.md`) chốt Q1..Q13 của
[`review/agent0/K3-PLAN.md`](../../review/agent0/K3-PLAN.md) §3; chi tiết task ở §4/§4.1 cùng tệp.
Nhãn số liệu theo QD §2.2: `[TK]` thiết kế mới, `[GT]` giả thuyết nền tảng, `[ĐO]` chỉ trong log
do lệnh sinh ra. **Tài liệu này không chứa số đo viết tay** (QD §9 Q12): mọi số đo nằm ở
`v2/agent/JOURNAL/kt45-*.log`, `kt46-*.log`, `kt47-*.log`, `kt48-*.log`, `kt49-*.log` và do
`v2/test/boot/*.sh` / `cargo test` in ra.

## 1. K3 mới là kernel thật

- **K2** là mô hình (`vifast-sim` + `vifast-pte`), **không** phải kernel chạy trên phần cứng (D2-71).
- **K3** (KT-44..57) là lần đầu kernel riscv64 chạy **thật** trên QEMU `virt` qua OpenSBI: trap,
  MMU Sv39 (KT-46), timer SBI (KT-47), PLIC/UART IRQ (KT-48), SMP (KT-49), IPI (KT-50),
  TLB/shootdown (KT-51), object layer (KT-52), U-mode (KT-53).
- K3 **không** phải kernel ABI đầy đủ (D2-72 **Q13**): core CNode/TCB/IPC/scheduler vẫn model-only;
  port core `no_std` là phase **K3b**, mở bằng task port core đầu tiên (KT-58) trước `procsrv`/shell.
- Cổng `.data + .bss ≤ 256 KiB` `[TK]` (QD §3.5) **giữ nguyên** ở K3, áp cho **cả ba** ảnh
  (sản phẩm + 2 ảnh test; D2-72 Q5) — không nới, không tách ngân sách riêng cho ảnh test.

## 2. Hằng số nền tảng (không mở `spec/kernel.toml` — D2-72 Q1)

| Hạng mục | Giá trị | Nhãn | Nguồn |
|---|---|---|---|
| QEMU | sàn **≥ 9.2**; bản trên máy đo `11.1.3` | `[TK]` / `[GT]` | D2-72 Q2 |
| Máy ảo | `-machine virt,aia=none,aclint=on` (không `virt-<version>`) | `[TK]` | D2-72 Q2/Q8 |
| CPU / RAM / hart | `-cpu rv64 -m 128M -smp N`; sản phẩm mặc định `-smp 1`, **matrix 1/2/4**; `MAX_HARTS = 8` | `[TK]` | D2-72 Q2/Q4, KT-49 §12 |
| SMP / PerHart | **SBI HSM** `hart_start`/`hart_get_status` (EID `0x48534D`, FID 0/2); hart boot `0`; hart `1..N-1` start với `start_addr=_start_secondary`, `opaque="KT49"`; hart ngoài DTB (`hart_get_status = -3`) **bị từ chối** + log, không panic | `[TK]` | D2-72 Q4, KT-49 §12 |
| Vùng NOLOAD per-hart | **8 record** `guard 4 KiB \| stack 64 KiB \| trap stack 16 KiB` (`HART_STRIDE = 0x1_5000`, tổng `0xA_8000`); guard mỗi hart **NONE** ở cả identity/nửa cao/physmap; arena page table **64 KiB** sau `__kernel_end` | `[TK]` | KT-49 §12 |
| Bộ đếm theo hart | `TIMER_STATS`/`PLIC_STATS` là **PerHart** (`get(hart)`), handler chỉ cộng slot của hart đang chạy; `TRAP_STATS`/`GUARD_STATS` vẫn gộp toàn cục | `[TK]` | KT-49 §12 |
| Firmware | OpenSBI `fw_dynamic` nạp **trực tiếp** qua `-bios`, không U-Boot/EDK2; bản đi kèm QEMU `v1.8.1` | `[GT]` | D2-72 Q3 |
| Nạp kernel | S-mode tại `0x8020_0000`, `a0 = hartid`, `a1 = địa chỉ DTB` | `[GT]` | K0.5 (KT-07) |
| UART0 | `0x1000_0000` (16550A), IRQ **10** | `[GT]` | D2-72 Q7 |
| PLIC | `0x0c00_0000` (cổ điển), cửa sổ `0x60_0000`; **KT-48 map 2×2 MiB** (`0x0c00_0000` + `0x0c20_0000`); `ndev` **probe từ register** (ghi 1/đọc lại priority), không giả định 95; context S-mode `2*hart+1` | `[GT]` / `[TK]` | D2-72 Q7/Q8, KT-48 §11 |
| CLINT/ACLINT | `0x0200_0000` (`aclint=on` ⇒ IPI/timer qua ACLINT) | `[GT]` | D2-72 Q7 |
| Trang test | `0x0010_0000` (`test@100000`) | `[GT]` | D2-72 Q7 |
| Timer | **SBI TIME** `set_timer` (EID `0x5449_4D45`) + `sie.STIE`, timebase **10 MHz** (`rdtime`), tick **100 Hz** = 100.000 tick timebase = 10 ms; Sstc hoãn. **KT-47 đã cài** — xem §10 | `[TK]` | D2-72 Q6 |
| Deadline timer | **tuyệt đối** (bội số `TICK_TICKS`, không cộng dồn từ `now`) ⇒ không trôi; một deadline/hart; `missed = late_by / TICK_TICKS` ⇔ mất tick khi trễ trọn một chu kỳ | `[TK]` | KT-47, D2-72 Q6 |
| Tick accounting | `ticks/late/missed/non_monotonic/max_delta/min_delta/max_late` + bằng chứng `sstatus.SPIE/SIE` (`TimerCounters`/`TIMER_STATS`) | `[TK]` | KT-47 |
| Đo drift | Δ tick **∈ [9,11] ms** đo bằng `rdtime`; **chỉ `ktest.sh`** thêm `-icount shift=6` (QEMU TCG không icount phục vụ timer ảo thưa ⇒ đo độ trễ main loop, không đo toán deadline) — **deviation D2-74**, xem §10.5 | `[TK]` | D2-74 |
| Ngắt | **PLIC cổ điển**, AIA (APLIC/IMSIC) hoãn K4'/K5' | `[TK]` | D2-72 Q8 |
| DTB | chỉ kiểm magic nếu OpenSBI truyền ở `a1` (log/cảnh báo, **không** parse đầy đủ) | `[TK]` | D2-72 Q7 |
| Soak | `1000 boot × 3 luồng` trên máy `ps`, timeout **20 s/boot**, `0/1000`, không retry | `[TK]` | D2-72 Q10 |
| Trap | `stvec` **direct mode** (một vector, `MODE=0`, `stvec` là CSR **per-hart**); mỗi hart **trap stack riêng 16 KiB** (`sscratch` per-hart); trapframe **288 B** = 32×8 (x0..x31) + 4×8 (`sepc/sstatus/scause/stval`) | `[TK]` | D2-73, KT-45/49 |
| MMU | **Sv39** (`satp.MODE=8`), 3 cấp × 512 entry, bảng 4 KiB; `VA_BITS=39` từ `vifast-pte`; kernel **identity** + **nửa cao** 2 MiB (image), NOLOAD 4 KiB; physmap RAM **2 MiB NX**; MMIO UART **4 KiB** + PLIC **2×2 MiB Device** (KT-48); guard page **NONE** | `[TK]` | KT-46/48 |
| Địa chỉ map | kernel identity `0x8020_0000`; kernel nửa cao `0xFFFF_FFC0_4000_0000`; physmap `0xFFFF_FFC0_0000_0000 + PA`; vùng NOLOAD per-hart ở `0x8040_0000` (linker `ALIGN(2M)`): guard hart `h` tại `base + h*0x1_5000` | `[TK]` | KT-46/49 |
| Arena page table | **64 KiB** RAM sau `__kernel_end`, bump allocator trong builder; root + bảng trung gian **không** `static`, không cộng trần `.data+.bss` | `[TK]` | KT-46, D2-72 Q5 |
| IRQ | `Platform::enable_irq`/`disable_irq` = **PLIC thật** (KT-48): set/clear bit enable của context `2*hart+1` + đọc lại; cổng CPU `sie.SEIE` + `sstatus.SIE` do `plic::enable_external` (đối xứng `timer::enable_timer`); KT-49: mỗi hart tự đặt threshold/enable trên **context của mình** (`plic::init_hart`) | `[TK]` | D2-72 Q7/Q8, KT-48 §11, KT-49 §12 |
| UART IRQ | UART0 **IRQ 10**; `IER.TX-empty` (mức cao khi THR rỗng); handler `claim → đọc IIR → complete → re-arm`; tự tắt `IER` + enable sau **N = 8** ngắt (`UART_IRQ_LIMIT`) | `[TK]` | KT-48 §11 |

Ghi chú kiểm chứng: `run.sh`/`ktest.sh` in `# qemu <version>` và `# opensbi <version>` **từ log
boot thật**; QEMU dưới sàn **9.2** (so cả major.minor) ⇒ **FAIL**, không tự hạ (D2-72 Q2). OpenSBI banner
(`Platform Timer Device : aclint-mtimer @ …Hz`, `Platform IPI Device : aclint-mswi`) là log
`[ĐO]`, dùng để đối chiếu timebase ở KT-47 (§10.1). Cửa sổ PLIC `0x60_0000` theo K3-PLAN §1; OpenSBI
domain region phủ `0x0c00_0000–0x0c3f_ffff` `[GT]` — KT-48 map **2×2 MiB** trong cửa sổ đó (đủ
context 15 của `MAX_HARTS = 8`) và đọc `ndev` từ register, không giả định 95 (§11).

## 3. Bản đồ ảnh và linker script

```
0x8020_0000  .text        KEEP(.text.entry) — `_start` trong vifast-plat-riscv64
             .rodata      (chuỗi TAP, chẩn đoán)
             .data / .sdata
             .bss / .sbss / COMMON
             __bss_end
             . = ALIGN(2M)                       ← KT-46: tách vùng NOLOAD sang 2 MiB kế tiếp
0x8040_0000  record hart 0 (NOLOAD, 0x1_5000):
  0x8040_0000  .stack_guard (4 KiB)              ← guard hart 0, MMU cưỡng chế NONE
  0x8040_1000  .stack       (64 KiB)              ← `sp` hart 0 (`__stack_top`), mọc xuống guard
  0x8041_1000  .trap_stack  (16 KiB)              ← `sscratch` hart 0; trapframe 288 B trong vùng này
  0x8041_5000  .hart_stacks (NOLOAD, 7 × 0x1_5000) ← record hart 1..7 cùng khuôn (guard | stack | trap stack)
  0x804A_8000  __kernel_end                        ← arena page table (64 KiB) bắt đầu ở đây, KHÔNG là section
```

Record hart `h` ở `__hart_area_base + h*HART_STRIDE`; guard của hart `h` là **trang đầu record**
(nên tràn stack của hart nào chạm đúng guard hart đó). `__stack_*`/`__trap_stack_*` giữ nghĩa cũ
= **hart 0** (KT-46 dùng); `mmu::stack_range_of(h)`/`guard_range_of(h)`/`trap_stack_range_of(h)`
tính cho mọi `h`. `mmu::init` **panic** nếu linker lệch hằng số `plat-common::smp` (không chạy với
stack hart này đè guard hart khác).

- **Vì sao `ALIGN(2M)`:** KT-46 map **kernel image** `[0x8020_0000, 0x8040_0000)` bằng **một leaf
  2 MiB** ở nửa cao (yêu cầu "kernel nửa cao 2 MiB"). Nếu guard/stack nằm trong chính leaf đó thì
  không thể đặt riêng guard page `NONE` ở granularity 4 KiB. Đưa vùng NOLOAD sang 2 MiB kế tiếp
  giải quyết cả hai: image 2 MiB, guard 4 KiB.
- `.stack_guard` + `.stack` + `.trap_stack` là **NOLOAD**: chỉ chiếm địa chỉ, không chiếm file ảnh,
  **báo riêng** — không cộng vào trần budget (D2-72 Q5). Guard 4 KiB nằm **ngay dưới**
  `__stack_bottom` (`__stack_guard_end == __stack_bottom`).
- **Arena** (KT-46) bắt đầu ở `__kernel_end` (sau record hart 7), 64 KiB, căn 4 KiB. Root page table
  và mọi bảng trung gian được cấp từ đây (bump allocator), **không** `static`/`static mut`; arena
  được identity-map + nửa cao RW NX. Vì không phải section ELF, nó **không** vào `.data+.bss`.
- `_start` nhận `a0 = hartid`, chép vào `tp`; tính `sp`/`sscratch` từ **record của hart đó**
  (`__hart_area_base + hart*HART_STRIDE`), zero `.bss`, lưu `a1` (DTB) rồi gọi `rust_main`.
  `_start_secondary` (SBI HSM) làm cùng phép tính với `a0/a1` do `hart_start` truyền, rồi gọi
  `rust_secondary_main` của ảnh. `Riscv64Virt::hart_id` đọc `tp`, `now` đọc `rdtime`.
- `.stack`/`.trap_stack` là section NOLOAD: **không nằm trong `filesz`** của LOAD (mặc dù LOAD
  `memsz` vẫn phủ tới `__kernel_end`) — đây là bài học KT-45; KT-46 **bắt buộc** map cả hai vùng
  trong page table kernel (TAP 6 ghi/đọc qua nửa cao để chứng minh), nếu thiếu thì trap đầu tiên
  sau khi bật `satp` sẽ page-fault vào chính `.trap_stack` chưa map.

## 4. Ba ảnh: sản phẩm + hai ảnh test (D2-08) — tách bằng feature, không `cfg(target_arch)`

| Ảnh | Bin | Feature | TAP | Runner |
|---|---|---|---|---|
| **Sản phẩm** | `vifast-kbin-riscv64` | — (mặc định) | `ok 1 - k3 boot` + `# k3 halt`, kèm bằng chứng `# plic:`/`# uart irq:`/`# timer arm:`/`# timer:`/`# smp:` (PLIC + UART IRQ thật, timer 100 Hz + ≥1 tick, **SMP bring-up `N` hart**, đều **không icount**) | `v2/test/boot/run.sh` |
| **Test chính** | `vifast-kbin-ktest` | `ktest` | **20** — 4 TAP trap KT-45 + 3 TAP MMU KT-46 + **3 TAP PLIC/UART IRQ KT-48** + **4 TAP timer KT-47** (chạy **sau** PLIC) + **5 TAP SMP KT-49** (harts alive, stack/guard per-hart, timer per-hart, PLIC ctx per-hart, barrier) + trap lồng fatal cuối | `v2/test/boot/ktest.sh` |
| **Test guard** | `vifast-kbin-ktest-guard` | `ktest` | **3** — guard image + load `scause=13` (resume probe) + store `scause=15` fatal | `v2/test/boot/ktest.sh` |

- Cùng crate `v2/kernel/vifast-kbin-riscv64`, cùng `linker.ld`; các bin test khai
  `required-features = ["ktest"]` nên **build mặc định không sinh ảnh test**.
- **Vì sao hai ảnh test:** mỗi boot chỉ kết thúc được **một** lần bằng PANIC có chủ đích. Ảnh chính
  dùng lần đó cho **trap lồng** (KT-45 P2b); guard **store fault** (`scause=15`) cần một boot riêng
  để dump + halt sạch. Ảnh guard kiểm thêm **load fault** (`scause=13`) trước đó bằng probe resume
  **chỉ trong test** (cờ `GUARD_RECOVER_LOAD`); chính sách mặc định của kernel vẫn là **fatal**.
- `run.sh` **fail** nếu ảnh sản phẩm còn **một** chuỗi `ktest`; `ktest.sh` **fail** nếu ảnh test
  **không** có chuỗi đó ⇒ chứng minh tách ảnh thật, không chỉ đổi tên.
- **Probe guard chỉ có trong ảnh test**: `probe_guard_load`/`probe_guard_store` nằm sau
  `#[cfg(feature = "ktest")]` ở `vifast-plat-riscv64` (feature `ktest` của kbin bật feature arch).
  Ảnh sản phẩm build **không** feature nên không có probe/chuỗi test; chính sách guard của nó
  **luôn fatal** (`report_guard_fault` + halt).
- `PanicHandler` + trait `Platform` + logic trap/MMU thuần nằm ở `vifast-plat-common` nên **các ảnh
  dùng chung một handler**, không chép đôi code.
- KT-49: ảnh test chính **20 TAP** (`expected.txt` đọc kế hoạch/số `ok`) và `ktest.sh` chạy
  **matrix `-smp 1|2|4`** (`bash ktest.sh 50 matrix`, log riêng từng N); harness TAP đầy đủ cho
  **toàn bộ** `.scn`/conformance vẫn là **KT-54/55**.
- KT-47: ảnh **sản phẩm** arm timer + chờ **≥1 tick** (timeout 1 s) rồi in `# timer: ticks=…` —
  bằng chứng **boot thật, không icount** (D2-74 điều kiện (b)); ảnh test đo drift **tất định** dưới
  `-icount shift=6` (D2-74, §10.5).
- KT-48: **cả hai** ảnh bật PLIC + UART IRQ thật (product **không icount** ⇒ bằng chứng PLIC trên
  đường boot thật; test thêm `# plic:`/`# uart irq:`/`# plic quiet:` và TAP 8..10); handler tự tắt
  `IER` + enable sau N ngắt nên log/TAP không bị ngập (§11).
- KT-49: **cả hai** ảnh bring-up SMP thật (SBI HSM); ảnh test thêm TAP 15..19 + `# smp:`/`# hart h
  alive`/`# stack hart=h`/`# timer hart=h`/`# plic hart=h`/`# barrier:`; ảnh sản phẩm in `# smp:`
  và **PANIC** nếu present-mà-không-online. Chạy `run.sh N 1|2|4` để kiểm ảnh sản phẩm nhiều hart
  (**không icount**, D2-72 Q2).

## 5. Build / boot / đo

```bash
bash v2/test/boot/run.sh 1        # ảnh sản phẩm, 1 boot, 1 hart (D2-72 Q2, KHÔNG icount)
bash v2/test/boot/run.sh 50       # 50 boot 0 lỗi (DoD KT-46/47)
bash v2/test/boot/ktest.sh 1 1    # 2 ảnh test: 20 TAP chính + 3 TAP guard (`-icount shift=6`)
bash v2/test/boot/ktest.sh 1 matrix  # matrix smp 1/2/4 (KT-49), log riêng từng N
bash v2/test/boot/ktest.sh 50 matrix # 50 boot **mỗi** N (DoD KT-49)
bash v2/test/boot/run.sh 50 1     # ảnh sản phẩm 1 hart (không icount)
bash v2/test/boot/run.sh 50 4     # ảnh sản phẩm 4 hart (SMP thật, không icount)

bash v2/test/boot/budget.sh \
  v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 product
bash v2/test/boot/budget.sh \
  v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest ktest
bash v2/test/boot/budget.sh \
  v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-guard ktest-guard
```

Lệnh thô bên trong (không cần nhớ, chỉ để tra):

```bash
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
    --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --release
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
    --bin vifast-kbin-ktest --bin vifast-kbin-ktest-guard --features ktest \
    --target riscv64gc-unknown-none-elf --release

# Ảnh sản phẩm (run.sh) — đúng dòng D2-72 Q2:
qemu-system-riscv64 -machine virt,aia=none,aclint=on \
    -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel <ảnh> -nographic -cpu rv64 -m 128M -smp 1 -no-reboot

# Ảnh test (ktest.sh, D2-74): thêm -icount shift=6 để timer ảo QEMU bắn đúng hạn.
qemu-system-riscv64 -icount shift=6 -machine virt,aia=none,aclint=on \
    -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel <ảnh test> -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
```

Cổng đo (do script thực thi, không phải người):

- **Budget** — `budget.sh` cộng `.data .sdata .bss .sbss` bằng `llvm-size -A` (đúng cách đo của
  K0.5/KT-07), trần **262.144 B** `[TK]` cho **cả ba ảnh**, in
  `# image=… data+bss=… stack=… guard=… trap_stack=…`; **exit 1** nếu vượt (D2-72 Q5). Page table
  nằm trong arena RAM nên **không** đội `.data+.bss`.
- **Boot** — `run.sh`/`ktest.sh`: timeout **20 s**/boot `[TK]` (D2-72 Q10), chỉ `kill <pid>` do
  chính script tạo (AGENTS §9 — cấm `pkill -f qemu-system`), đòi QEMU ≥ sàn **9.2**, đòi TAP đủ
  (`TAP version 13`, `1..K`, mọi dòng `ok`, marker/PANIC có chủ đích), **không** có PANIC lạ.
- **SMP** — `ktest.sh` kiểm `Platform HART Count` của **OpenSBI** khớp `-smp N` (bằng chứng độc lập
  ngoài kernel), `# smp: … harts=N … timeout=0`, `# harts alive: n=N … tickets=1`, số dòng
  `# hart h alive`/`# stack hart=h`/`# timer hart=h`/`# plic hart=h` = N; `run.sh` kiểm
  `# smp: … harts=N … timeout=0` trên ảnh sản phẩm.
- **Cổng repo** — `./v2/bin/v2 gate --fast` **không** build kernel
  (`v2/Cargo.toml` `exclude = ["kernel"]`); kernel kiểm bằng build + `ktest.sh`/`run.sh` + host test
  `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common`.

## 6. `vifast-plat-common` + `vifast-plat-riscv64` (KT-44..49)

- `vifast-plat-common` (`no_std` + `#![forbid(unsafe_code)]`):
  - trait **`Platform`**: `put_str`, `halt`, `now_ticks`/`now_ns`, `timebase_hz`, `hart_id`,
    `set_trap_vector`, `enable_irq`/`disable_irq` (**PLIC thật** từ KT-48), và **KT-47**:
    `set_timer_deadline`, `enable_timer`, `disable_timer`; **`PanicHandler`** dùng chung;
    **`PlatformWriter`**.
  - module **`trap`** (KT-45): `TrapFrame` 288 B + offset hằng số + `const` assert `offset_of!`;
    `ExceptionKind`/`Exception`/`Event`; `decode_scause`/`decode_trap`; `handle_trap`;
    `TrapStats`/`TRAP_STATS`; `GuardStats`/`GUARD_STATS`; `report_fatal`/`report_guard_fault`/
    `report_interrupt`; `is_nested_trap_sp`.
  - module **`mmu`** (KT-46): bit PTE Sv39 (`V/R/W/X/U/G/A/D`), `PteFlags`, `MapError`, `Walk`,
    `PteMemory`, `is_canonical`, `walk_with`, `PageTableBuilder` (cấp bảng từ `&mut [u8]`, map
    4 KiB/2 MiB/1 GiB, walk phần mềm), `satp_sv39`/`satp_mode`/`satp_root`; dùng lại
    `vifast_pte::{walk_shape, Pte, VA_BITS=39, PTE_LEVELS=3, PAGE_SIZE_BITS}` — crate nội bộ,
    `no_std`, không dependency ngoài.
  - module **`plic`** (KT-48): hình học PLIC cổ điển (priority `0x0`, pending `0x1000`, enable
    `0x2000 + 0x80*ctx`, context `0x20_0000 + 0x1000*ctx`, threshold `+0`, claim `+4`), context
    `2*hart+1`, `ndev` probe/limit, phân loại claim (UART/spurious/line lạ), `PlicStats` +
    **`PerHartPlicStats`**/`PLIC_STATS.get(hart)` (traps/claims/completions/uart_irqs/spurious/
    unexpected/last_irq/last_iir — **theo hart** từ KT-49).
  - module **`timer`** (KT-47): hằng số `[TK]` `TIMEBASE_HZ=10_000_000`, `TICK_HZ=100`,
    `TICK_TICKS=100_000`, `TICK_NS=10_000_000`, `TICK_DELTA_MIN_NS/MAX_NS` = 9/11 ms;
    `ticks_to_ns`/`ns_to_ticks`; `classify(last, deadline, now) -> TickSample` (delta, late_by,
    `missed`, `next_deadline` — thuần, test host); `TimerCounters` + `TimerSnapshot`
    (tick/late/missed/monotonic/max_delta/min_delta/max_late + `trap_spie_on`/`handler_sie_off`),
    **`PerHartTimerStats`**/`TIMER_STATS.get(hart)` (theo hart từ KT-49).
  - module **`smp`** (KT-49): `MAX_HARTS=8`, hằng số record `guard|stack|trap stack`
    (`HART_STRIDE=0x1_5000`, `HART_AREA_BYTES=0xA_8000`) + offset/guard-owner thuần, `HartMask`
    (`AtomicU64`), hằng SBI HSM + `classify_hart_status`/`start_decision`/`hart_start_ok`
    (hart ngoài DTB `-3` ⇒ `Reject`, không panic).
- `vifast-plat-riscv64` (asm/MMIO, `unsafe` thuộc danh sách đóng `kernel/vifast-plat-*`, D2-17):
  `_start`, `trap_entry` direct mode, `trap_dispatch`, probe KT-45; **module `mmu`** (KT-46): đọc
  linker symbol, arena từ `__kernel_end`, dựng identity + nửa cao + physmap + MMIO + guard NONE,
  `csrw satp` + `sfence.vma` hai bước, đọc lại `satp`, alias self-test, walk phần mềm
  (`PhysMemory`), `probe_guard_load`/`probe_guard_store`, `read_u64`/`write_u64` cho TAP.
  **Module `timer`** (KT-47): `now_ticks` (`rdtime`), `set_timer_deadline` (SBI TIME ecall),
  `enable_timer`/`disable_timer` (`sie.STIE`/`sstatus.SIE`), `read_sie`/`read_sstatus`, `init`
  (arm + bật), `on_timer_interrupt` (đếm tick + trả deadline kế cho `trap_dispatch`).
  **Module `plic`** (KT-48/49): MMIO `plic_read`/`plic_write`, `probe_ndev`, `init` (threshold 0,
  priority 1, enable context `2*hart+1`), `init_hart` (mỗi hart tự threshold + enable round-trip
  trên **context của mình**, kiểm context M-mode `2*h`), `claim`/`complete`, `enable_irq`/
  `disable_irq` + `irq_enabled`, `enable_external`/`disable_external` (`sie.SEIE` + `sstatus.SIE`),
  UART `IER`/`IIR` (`uart_enable_tx_irq`/`uart_disable_tx_irq`), và `on_external_interrupt`
  (claim → IIR → complete → re-arm / tắt sau N) — bộ đếm `PLIC_STATS.get(hart)`.
  **Module `smp`** (KT-49): `_start_secondary` (asm ở `lib`), `hart_start`/`hart_status` (SBI HSM
  `ecall`), `init` (probe + start + chờ online), `secondary_setup` (adopt `satp`, `stvec`, online),
  `release`/`finish`/`wait_release`, `take_ticket`, `note_trap_frame`, barrier
  (`barrier_begin`/`join`/`wait_arrivals`/`release`, timeout) — `unsafe` thuộc D2-17.
- Phần **logic** chạy host (D2-72 Q12): `cargo test --manifest-path v2/kernel/Cargo.toml
  -p vifast-plat-common` — 13 test trap KT-45 + **1 test API `Platform` KT-47** (`now_ns`/
  `timebase_hz`/ba method timer) + **8 test MMU KT-46** (map/walk 4K-2M-1G, từ chối mapping sai,
  VA canonical, guard NONE/physmap NX, `satp` mode 8, guard stats + dump) + **12 test timer KT-47**
  (hằng số/đổi đơn vị, `classify` đúng hẹn/trễ/mất tick/ngắt sớm/saturate, 1.000 tick không mất,
  phát hiện mất tick + tự căn deadline, monotonic, bằng chứng `sstatus`, arm/reset) + **12 test
  PLIC KT-48** (hình học đăng ký, context `2*hart+1` cho hart 0..3, word/bit enable, offset
  threshold/claim, hai cửa sổ phủ `MAX_HARTS = 8`, phân loại claim, claim/complete cân bằng +
  0 IRQ giả, limit tắt sau N, `ndev` không giả định 95, priority > threshold) + **9 test SMP
  KT-49** (`tests/smp.rs`: bố cục record không chồng/lệch, guard owner, `HartMask` + hart ngoài
  dải, phân loại trạng thái HSM + từ chối hart ngoài DTB, `PerHartTimerStats`/`PerHartPlicStats`
  không lẫn) = **55 test**.
  Miri **không** áp ảnh kernel `no_std`/asm/MMIO; Miri vẫn áp `lib/` + sim.

## 7. Trap khung (KT-45) + guard page-fault (KT-46, theo D2-73)

- **`stvec` direct mode**: một vector `trap_entry` (`.balign 4`, `MODE=0`), cài bằng
  `Platform::set_trap_vector`; `read_trap_vector` đọc lại để ảnh in bằng chứng.
- **Trap stack riêng**: `_start`/`_start_secondary` đặt `sscratch =` đỉnh trap stack **của hart
  đang chạy**. `trap_entry` làm `csrrw sp, sscratch, sp` rồi `addi sp, sp, -288` — trapframe 288 B
  nằm **trọn trong** `.trap_stack` của hart đó (P1 của review KT-45; KT-49 làm per-hart). Lưu `x0..x31` + `sepc/sstatus/scause/stval` (offset là
  `const` operand từ `plat-common`), gọi `trap_dispatch`; khôi phục **chỉ** `sepc` + thanh ghi,
  `sp` **cuối cùng**, rồi `sret`. **Không** ghi `sstatus` khi ra (sret tự khôi phục SIE/SPP — P2a).
- **Trap lồng (P2b)**: nếu sp bị ngắt nằm trong trap stack **của hart đang chạy**
  (`[trap_top − 16 KiB, trap_top)`, suy từ `sp` frame — KT-49) ⇒ `nested_trap_halt` in
  `PANIC: nested trap …` + `DebugHalt(1)`, không resume. Ảnh test chính dùng
  đúng đường này làm TAP cuối (fatal có chủ đích).
- **Decode**: bit 63 = interrupt; còn lại `Event::Exception` cho **0/1/2/3/4/5/6/7/8/9/12/13/15**
  và `Unknown(mã)` (host test đủ bảng). Chính sách: `breakpoint`/`illegal`/`ecall` resume đúng cỡ
  instruction (2/4 byte); page fault/access fault/misaligned/`Unknown` **fatal**.
  **KT-47**: `scause = (1<<63) | 5` ⇒ `Event::Timer` (**không** phải exception) — handler đếm tick,
  nạp deadline kế rồi **resume** bằng `sret`. **KT-48**: `scause = (1<<63) | 9` ⇒
  `Event::Interrupt { code: 9 }` đi vào PLIC **claim → xử lý → complete** (không fatal); ngắt mã
  khác 5/9 (vd IPI 1 — KT-50) vẫn fatal.
- **`sstatus.SIE` (P2a, KT-47)**: `trap_entry`/handler **không** ghi `sstatus`. Hardware khi trap:
  `SPIE ← SIE`, `SIE ← 0`, `SPP ← 1` ⇒ trapframe có `SPIE=1` (bị ngắt khi đang bật ngắt) và
  `SIE=0`; handler chạy với `SIE=0` nên **không** trap lồng; `sret` khôi phục `SIE ← SPIE`.
  Cổng `SIE` chỉ bật/tắt ở `timer::enable_timer`/`disable_timer` (`sie.STIE` trước rồi
  `sstatus.SIE`, đảo thứ tự khi tắt) — TAP `timer irq` đếm `trap_spie_on`/`handler_sie_off` bằng
  `ticks` và `sret_sie=1`.
- **Guard page fault (KT-46)**: trước `trap_dispatch`, nếu exception là page fault (12/13/15) và
  `stval ∈ [__stack_guard_start, __stack_guard_end)` thì đi đường riêng:
  - **mặc định fatal**: `GUARD_STATS` ghi nhận, `report_guard_fault` in
    `PANIC: guard page fault scause=0x… stval=0x… sepc=0x…` + `ra/sp/gp/tp` + `a0..a7` rồi
    `DebugHalt(1)` — dump + halt sạch, không treo.
  - **probe resume chỉ trong test**: `probe_guard_load` bật cờ `GUARD_RECOVER_LOAD` **một lần** cho
    đúng load page fault (`scause=13`) để ảnh guard kiểm được cả 13 và 15 trong một boot; handler
    in `# guard fault load scause=0xd …`, cộng `sepc` đúng 4 byte rồi resume. Mọi guard fault khác
    (và store fault) vẫn fatal.
  - Guard page là **NONE** ở cả identity, nửa cao và physmap ⇒ không có cửa sau; TAP 6 (ảnh chính)
    kiểm cả ba; ảnh guard kiểm fault **thật** vào `stvec`.
- **Vì sao page fault thật phải chờ KT-46**: OpenSBI v1.8.1 `delegate_traps()` đặt
  `MEDELEG = 0xf4b509`; bit 2 (illegal) và bit 9 (ecall-S) **không** delegate, nhưng page fault
  12/13/15 **được** delegate — D2-73 chuyển chứng minh `scause=13/15` sang KT-46 guard page. TAP:
  `breakpoint` thật, `ecall-S` qua SBI + resume, `illegal` **tiêm** (ghi rõ inject), trap lồng
  fatal; guard load/store thật.
- **Đo/log**: KT-45 `kt45-*`; KT-46 `v2/agent/JOURNAL/kt46-{red-ktest,host-tests,build,ktest,
  ktest-main-raw,guard-ktest-raw,boot,boot50,budget,gate-fast}.log` (số đo do script in, không chép
  tay — QD §9 Q12).

## 8. MMU Sv39 thật (KT-46)

### 8.1 Bố cục ánh xạ

QEMU `virt`, RAM vật lý `0x8000_0000..0x8800_0000` (`-m 128M`); `Δ = VA − 0x8020_0000`.

| Vùng | VA | PA | Leaf / quyền |
|---|---|---|---|
| kernel identity — image | `0x0000_0000_8020_0000..0x0000_0000_8040_0000` | `= VA` | **2 MiB** `V|R|W|X|A|D` (W^X đầy đủ KT-56) |
| kernel identity — NOLOAD | `0x0000_0000_8040_0000..arena_end` | `= VA` | **4 KiB**; 8 guard per-hart **NONE**; stack/trap stack của **mọi** hart + arena RW NX |
| kernel nửa cao — image | `0xFFFF_FFC0_4000_0000 + Δ` | `0x8020_0000 + Δ` | **2 MiB** RWX |
| kernel nửa cao — NOLOAD | `0xFFFF_FFC0_4000_0000 + Δ`, `Δ ≥ 0x20_0000` | `= 0x8040_0000 + …` | **4 KiB**; guard **NONE**; RW NX |
| physmap RAM | `0xFFFF_FFC0_0000_0000 + PA` | `PA` | **2 MiB NX** (`V|R|W|A|D`); chunk chứa guard tách 4 KiB và **khoét guard của từng hart** |
| UART0 (Device) | `0x0000_0000_1000_0000` | `= VA` | **4 KiB** `V|R|W|A|D`, X=0 |
| PLIC — cửa sổ thấp (Device) | `0x0000_0000_0c00_0000` | `= VA` | **2 MiB** `V|R|W|A|D`, X=0 (priority/pending/enable) |
| PLIC — cửa sổ cao (Device) | `0x0000_0000_0c20_0000` | `= VA` | **2 MiB** `V|R|W|A|D`, X=0 (threshold/claim) |
| guard per-hart | `base + h*0x1_5000 .. +0x1000`, hart 0 = `0x8040_0000` | — | **NONE** (không PTE hợp lệ) ở **cả** identity, nửa cao, physmap |

- Kernel **liên kết thấp** (`linker.ld` `0x8020_0000`) nên nó **chạy** ở identity mapping; nửa cao
  là **alias** để đọc/ghi (và để user root copy entry sau này — K3 chưa có user root). Đây là lựa
  chọn có chủ đích, không phải "quên chạy nửa cao": relocating image sang nửa cao cần linker/entry
  khác và không thuộc KT-46.
- **2 MiB cho image, 4 KiB cho NOLOAD** vì guard page cần granularity 4 KiB; linker đặt vùng NOLOAD
  sang `0x8040_0000` (`ALIGN(2M)`) để leaf 2 MiB của image không phủ guard. Physmap cũng dùng
  leaf 2 MiB (yêu cầu "2 MiB/1 GiB") và khoét guard.
- stack/trap stack per-hart là NOLOAD: **không nằm trong `filesz`** của LOAD (LOAD `memsz` vẫn
  phủ tới `__kernel_end`); KT-46 map **tường minh** cả hai (TAP 6 ghi/đọc qua nửa cao), KT-49 map
  cho **mọi** hart. Thiếu map ⇒ trap đầu tiên sau `satp` sẽ fault vào trap stack.
- MMIO map **4 KiB UART** + **2×2 MiB PLIC** tại đúng PA (D2-72 Q7; KT-48 mở rộng từ 4 KiB để phủ
  enable `0x2000 + 0x80*ctx` và context `0x20_0000 + 0x1000*ctx` cho `MAX_HARTS = 8` — §11).
  CLINT không map (timer qua SBI — KT-47).

### 8.2 Arena page table

- Root page table cấp từ **arena RAM** bắt đầu ở `__kernel_end` (sau record hart 7), cỡ **64 KiB**,
  zero từng bảng 4 KiB. Root = phần tử đầu arena; bảng trung gian cấp tăng dần (bump allocator).
- **Không** `static`/`static mut` cho bảng; không cộng vào trần `.data+.bss` (D2-72 Q5). Arena
  identity-map + nửa cao RW NX; nằm trong vùng NOLOAD 4 KiB nên cũng không đổi file ảnh.
- Nếu arena hết hoặc mapping sai (unaligned/trùng/không canonical/VA ngoài hình), `mmu::init` panic
  qua `PanicHandler` — **không** bật `satp` với bảng dở.

### 8.3 Bật `satp` hai bước + walk khớp

1. Dựng **xong** toàn bộ bảng: identity (image 2 MiB + NOLOAD 4 KiB), nửa cao, physmap NX, MMIO
   Device, guard NONE. Chưa đụng `satp`.
2. Lấy mẫu PTE của 8 VA (kernel identity/nửa cao, physmap, UART, PLIC, `.stack`, `.trap_stack`,
   arena) rồi `csrw satp` (`mode=8 | PPN(root)`) + `sfence.vma` trong **một** asm block.
3. Đọc lại `satp` bằng `csrr` (mode 8 + đúng PPN root); đọc lại **cùng 8 PTE** sau khi bật — phải
   **bằng** mẫu trước; walk phần mềm (`walk_sw`) phải trả đúng PTE đó.
4. **Hardware walk thật**: ghi qua identity rồi đọc qua nửa cao **và** physmap (và ngược lại) với
   một biến `.bss`; nếu MMU không hoạt động/alias sai thì giá trị không khớp. TAP 5/7 in
   `satp`, root, alias và số mẫu khớp.
5. Không có bước "bật thử rồi sửa": mọi mapping phải xong trước `satp`; sau đó chỉ đọc/kiểm, không
   sửa bảng (tránh quên `sfence.vma` — nợ mutation của KT-51).

### 8.4 Guard page và bằng chứng fault thật

- Mỗi hart một guard 4 KiB **ngay dưới** stack của hart đó (`guard_owner(pa)` trả hart); entry
  **không hợp lệ** ở identity, nửa cao và physmap. Chạm vào (dù đọc hay ghi) ⇒ **page fault phần cứng thật** vào `stvec`:
  - đọc ⇒ `scause=13` (`LoadPageFault`), `stval` = địa chỉ trong guard;
  - ghi ⇒ `scause=15` (`StorePageFault`), `stval` = địa chỉ trong guard.
- Handler guard in bằng chứng + (mặc định) `PANIC: guard page fault …` + thanh ghi rồi
  `DebugHalt(1)`. Ảnh `vifast-kbin-ktest-guard` kiểm **cả hai** `scause` trong một boot: load
  `scause=13` qua probe resume-một-lần, store `scause=15` fatal (dump + halt). Đây là page fault
  **thật**, không phải trap tiêm — khác `illegal` của D2-73.
- Guard cũng **NONE** qua physmap (chunk 2 MiB chứa guard được tách 4 KiB) nên không có cửa sau
  đọc/ghi trang bảo vệ bằng alias vật lý.

### 8.5 Phạm vi KT-46 (không làm)

Xem bảng §9. KT-46 **không** timer, PLIC IRQ, SMP/PerHart, IPI, ASID/shootdown, object layer/
Retype, U-mode, W^X đầy đủ, pager/COW, soak 1.000 — các task sau.

## 9. Phạm vi: KT-49 **không** làm gì

| Không làm | Thuộc |
|---|---|
| IPI thật (SBI IPI/ACLINT MSWI), hộp thư drain, `TlbFlush`/`VmKick` | KT-50 |
| Hotplug/suspend hart, `hart_stop`, scheduler đa hart/preemption | K3b + task riêng |
| PLIC ngắt theo hart **thật** cho nguồn khác UART (mỗi hart chỉ init context + round-trip IRQ của mình) | KT-50+ |
| ASID thật, `sfence.vma` theo ASID/VA, shootdown ack + timeout, mutation bỏ flush | KT-51 |
| AIA (APLIC/IMSIC), MSI, EOI/priority động | K4'/K5' (D2-72 Q8) — task riêng |
| Object layer `no_std` + Retype nội bộ (VSpace/PageTable/Frame từ Untyped) | KT-52 |
| U-mode, 3 syscall, nạp `init` ELF, user root copy entry nửa cao | KT-53 |
| Harness TAP **đầy đủ** cho mọi ảnh/`.scn` (KT-49 mới có `expected.txt` + matrix cho 2 ảnh test K3) | KT-54 |
| Conformance 5 `.scn` × sim/QEMU | KT-55 |
| `FaultKind` thật + W^X đầy đủ (tách text/rodata RWX→RX) + một hunk `spec/abi.toml` | KT-56 |
| Soak 1.000 boot + K3-DOD + perf (**soak KHÔNG icount** — D2-74 điều kiện (c)) | KT-57 |
| Port core `no_std` (CNode/TCB/IPC/sched) | **K3b** (D2-72 Q13) |
| Đo **jitter timer thật** (không icount) / preemption thật theo timer | task riêng — D2-74 điều kiện (d) |

## 10. Timer SBI (KT-47) — tick 100 Hz, timebase 10 MHz, deadline tuyệt đối

Nguồn quyết định: **D2-72 Q6** (SBI TIME, 100 Hz, timebase 10 MHz, Sstc hoãn) và **D2-74**
(deviation `-icount shift=6` **chỉ** trong `ktest.sh`, §10.5). Task: K3-PLAN §4.1 KT-47.

### 10.1 Ràng buộc và hằng số `[TK]`

| Hằng số | Giá trị | Nghĩa |
|---|---|---|
| `timer::TIMEBASE_HZ` | **10.000.000** | timebase QEMU `virt`; `rdtime` (Zicntr) đếm theo đơn vị này |
| `timer::TICK_HZ` | **100** | tick kernel = 10 ms |
| `timer::TICK_TICKS` | **100.000** | một tick tính bằng đơn vị timebase (`TIMEBASE_HZ / TICK_HZ`) |
| `timer::TICK_NS` | **10.000.000** | một tick tính bằng ns |
| `timer::TICK_DELTA_MIN_NS` / `TICK_DELTA_MAX_NS` | **9.000.000** / **11.000.000** | DoD KT-47: Δ tick ∈ [9,11] ms — **không nới** |

- **SBI TIME**: EID `0x5449_4D45`, FID `0` = `sbi_set_timer(uint64 deadline)`; deadline **tuyệt
  đối**, đơn vị timebase. `ecall` từ S-mode đi M-mode/OpenSBI (SBI bảo toàn mọi thanh ghi trừ
  `a0`/`a1`). Không dùng CLINT trực tiếp (KT-46 chỉ map UART/PLIC), **không** Sstc (D2-72 Q6).
- **`sie.STIE`** (bit 5) bật nguồn ngắt S-mode timer; **`sstatus.SIE`** (bit 1) mở cổng.
- Một **deadline/hart**: deadline nằm trong slot **PerHart** `TIMER_STATS.get(hart)` (KT-49);
  `sbi_set_timer`/`sie.STIE` là CSR per-hart.
- OpenSBI banner `Platform Timer Device : aclint-mtimer @ 10000000Hz` là log `[ĐO]` để đối chiếu
  timebase — không chép tay vào tài liệu (QD §9 Q12).

### 10.2 Deadline tuyệt đối + phân loại tick

Handler **không** đặt `deadline = now + TICK_TICKS` (kiểu tương đối sẽ cộng dồn độ trễ). Nó dùng
`timer::classify(last, deadline, now)`:

```text
delta       = now − last                          (khoảng cách giữa hai lần quan sát)
late_by     = now − deadline        (0 nếu ngắt đến sớm hơn deadline)
missed      = late_by / TICK_TICKS  (số deadline bị bỏ; 0 ⇔ chỉ là độ trễ ngắt)
next        = deadline + (missed+1)·TICK_TICKS    (luôn > now)
```

- `missed > 0` ⇔ **mất tick** ("trễ trọn một chu kỳ"): deadline kế không thể giao riêng nữa nên
  các deadline bị bỏ được **nhảy qua một lần** (không dồn interrupt).
- `next` là **bội số tuyệt đối** của deadline cũ ⇒ sai số không cộng dồn (không trôi), khác hẳn
  cách cộng dồn từ `now`.
- `TimerCounters` cộng dồn `ticks`, `late`, `missed`, `non_monotonic` (đếm `now <= last`),
  `max_delta`/`min_delta` (đo drift), `max_late`; `record_sstatus` đếm bằng chứng `sstatus`.
- Hàm `classify` **thuần** (không asm/atomic) nên host test phủ cả trường hợp trễ/mất tick mà phần
  cứng khó tạo (`tests/timer.rs`).

### 10.3 API `Platform` + cài đặt arch

`vifast-plat-common::Platform` (KT-47 thêm; chữ ký ổn định cho KT-48+):

| Method | Hợp đồng |
|---|---|
| `now_ticks() -> u64` | `rdtime` thô, đơn vị timebase; monotonic |
| `now_ns() -> u64` | quy đổi từ `now_ticks` (`ticks_to_ns`, 10 MHz ⇒ chính xác tuyệt đối) |
| `timebase_hz() -> u64` | 10 MHz (tên cũ `tick_hz` đã đổi cho khỏi lẫn với `TICK_HZ = 100`) |
| `set_timer_deadline(deadline)` | `sbi_set_timer`; deadline tương lai cũng **xoá** ngắt đang chờ; `u64::MAX` = tắt hữu hiệu |
| `enable_timer() -> bool` | `sie.STIE = 1` **rồi** `sstatus.SIE = 1`; `true` ⇔ đọc lại thấy cả hai |
| `disable_timer()` | `sstatus.SIE = 0` **rồi** `sie.STIE = 0` (đảo thứ tự enable) |

Cài đặt asm/CSR/SBI nằm ở `vifast-plat-riscv64::timer` (`unsafe` thuộc danh sách đóng D2-17):
`now_ticks`, `read_sie`, `read_sstatus`, `set_timer_deadline`, `enable_timer`, `disable_timer`,
`init()` (arm deadline đầu `now + TICK_TICKS` → `set_timer` → bật cổng), `on_timer_interrupt`
(đếm tick + trả deadline kế). Toàn bộ *ngữ nghĩa* (hằng số, `classify`, bộ đếm) ở plat-common để
chạy host (D2-72 Q12).

### 10.4 `sstatus.SIE` khi vào/ra trap (P2a) + đường trap timer

1. **Bật**: chỉ `enable_timer` ghi `sstatus.SIE = 1`, **sau** khi `set_timer(deadline)` đã nạp ⇒
   không có cửa sổ "SIE bật nhưng chưa có deadline". `sie.STIE` bật **trước** `sstatus.SIE`.
2. **Vào trap** (hardware): `SPIE ← SIE`, `SIE ← 0`, `SPP ← 1`; `trap_entry` lưu trapframe rồi
   gọi `trap_dispatch` — **không** ghi `sstatus` (KT-45 P2a).
3. **Trong handler**: `SIE = 0` (không trap lồng). `Event::Timer` ⇒ `on_timer_interrupt` ghi
   `# timer …`? **Không**: handler chỉ đếm + nạp deadline kế, **không in** (in trong handler làm
   trễ tick); `TRAP_STATS.record_interrupt(scause)` ghi `last_scause = (1<<63)|5`.
4. **Ra trap**: asm khôi phục thanh ghi + `sepc`, **không** ghi `sstatus`; `sret` khôi phục
   `SIE ← SPIE` ⇒ sau trap ngắt lại bật. Bằng chứng: `trap_spie_on == ticks` (SPIE=1 trong
   trapframe), `handler_sie_off == ticks` (SIE=0 trong handler), `sret_sie=1` (đọc sau khi về),
   `disabled_sie=0`/`sie_stie=0` sau `disable_timer`.
5. **Tắt trước test trap lồng**: `set_timer_deadline(u64::MAX)` (dập ngắt đang chờ) rồi
   `disable_timer()` — nếu không, timer có thể chen vào probe trap lồng và đổi dòng PANIC.

### 10.5 Đo drift: `-icount shift=6` **chỉ** trong `ktest.sh` (deviation D2-74)

- Với **đúng dòng D2-72 Q2** (không icount), QEMU TCG trên máy đo phục vụ **timer ảo**
  (`QEMU_CLOCK_VIRTUAL`/ACLINT) rất thưa: thời điểm ngắt đến trễ hơn deadline một khoảng lớn và
  thất thường (main loop của QEMU chỉ xử lý timer khi vCPU nhường), nên Δ tick đo được **không**
  phản ánh toán deadline của kernel. Đây là giới hạn của **host/harness**, không phải của kernel:
  cùng một ảnh, cùng kernel, chỉ đổi cách QEMU cấp thời gian ảo.
- Vì vậy **D2-74**: `v2/test/boot/ktest.sh` thêm **`-icount`** (ảnh **test**) ⇒ thời gian ảo tiến
  theo lệnh đã chạy, timer bắn đúng hạn, Δ tick đo bằng `rdtime` nằm trong [9,11] ms và **1.000
  tick không mất**. Ngưỡng **[9,11] ms giữ nguyên** — không nới.
- **Shift icount CỐ ĐỊNH = 6** (64 ns/lệnh), **không** dùng shift tự động (`auto`): `auto` tự đổi
  shift giữa chừng để bám thời gian thật ⇒ thời gian ảo nhảy, Δ tick có lần lệch hẳn khỏi 10 ms
  (đo được: có tick ~3,4 ms và ~16,6 ms trong cùng một lần chạy) ⇒ TAP `deadline drift` đỏ **ngẫu
  nhiên**. Shift cố định giữ Δ = 10 ms ± quantization của icount (mỗi lệnh = 64 ns), vẫn trong
  [9,11] ms và **ổn định** giữa các lần chạy (2 lần liên tiếp PASS). Đây là nội dung **D2-74**
  (đính chính 2026-11-15): vẫn chỉ `ktest.sh`, vẫn không nới ngưỡng.
- **`run.sh` (ảnh sản phẩm) và soak KT-57 KHÔNG icount** (đúng D2-72 Q2): ảnh sản phẩm arm timer
  và chờ **≥1 tick** (timeout 1 s) rồi in `# timer: ticks=…` — bằng chứng boot thật vẫn chạy; vì
  QEMU phục vụ timer ảo thưa, `delta_ns`/`late` của lần boot **không icount** có thể lớn (log
  riêng) và **không** dùng để đo drift.
- Phép đo icount chứng minh: toán deadline tuyệt đối, đếm tick/không mất tick, decode
  `scause=0x8000000000000005` thành `Event::Timer`, đường `sie.STIE`/`sstatus.SIE` và `sret`.
  Nó **không** đo jitter timer thật của host — việc đó (nếu cần) là **task riêng**, không đổi D2
  (D2-74 điều kiện (d)).
- Số đo thật: `v2/agent/JOURNAL/kt47-icount-ktest-run1.log` + `kt47-icount-ktest-run2.log` (2 lần
  liên tiếp, có icount), `kt47-boot50.log` (product, không icount),
  `kt47-qemu-no-icount-vs-icount.log` (A/B cùng ảnh) — tài liệu **không** chép số tay (QD §9 Q12).

### 10.6 TAP timer + bằng chứng (ảnh test chính, `1..20` — timer `11..14`)

| TAP | Điều kiện đo |
|---|---|
| `ok 11 - timer 100 ticks monotonic` | ≥100 ngắt timer **thật** qua `stvec`; `non_monotonic = 0` (mỗi tick `rdtime` tăng); `now > last` |
| `ok 12 - timer no lost tick` | 1.000 tick liên tiếp, `late = missed = 0`, `non_monotonic = 0` |
| `ok 13 - timer irq` | `TRAP_STATS.interrupts ≥ 1.000`, `last_scause = 0x8000000000000005`, `last_code = 5`, **0** exception phát sinh trong lúc chạy; `trap_spie_on = handler_sie_off = ticks`, `sie_stie=1`, `sret_sie=1` |
| `ok 14 - timer deadline drift` | `ticks_to_ns(min_delta)` ≥ 9 ms và `ticks_to_ns(max_delta)` ≤ 11 ms, `late = 0` |

Bằng chứng in ra (runner `grep` đúng các dòng này, `ktest.sh`): `# timer arm: … tick_ticks=100000
tick_hz=100 timebase_hz=10000000 sie_stie=1 sstatus_sie=1`, `# timer ticks: … non_monotonic=0
late=0 missed=0`, `# timer irq: … scause=0x8000000000000005 code=5`, `# timer drift: … in_range=1`,
`# timer sie: … sret_sie=1`, `# timer disable: sie_stie=0 sstatus_sie=0`. Ảnh sản phẩm in
`# timer arm: …` + `# timer: ticks=…` (≥1 tick, không icount).

### 10.7 Không thuộc KT-47

PLIC/UART IRQ (KT-48), SMP/PerHart (KT-49), IPI (KT-50), ASID/shootdown (KT-51), object layer
(KT-52), U-mode/syscall (KT-53), harness matrix/`expected.txt` (KT-54), conformance (KT-55),
`FaultKind`/W^X (KT-56), soak/K3-DOD (KT-57), preemption thật + đo jitter host (task riêng —
D2-74 (d)). Xem bảng §9.

**Giới hạn của phép đo KT-47** (reviewer nêu — ghi để không hiểu quá phạm vi bằng chứng):

1. Δ tick **chỉ đúng dưới icount**: số đo chứng minh toán deadline/đếm tick/đường trap của kernel,
   **không** chứng minh jitter timer thật của host (QEMU không icount phục vụ timer ảo thưa).
2. `late_by` đo được rất nhỏ (≤ vài trăm µs) nên **chưa phân biệt** được deadline tuyệt đối với
   cách `now + TICK`: cả hai đều cho Δ trong [9,11] ms khi drift nhỏ. Việc phân biệt dựa vào
   **host test** `classify` (trễ/mất tick/saturate) + thiết kế tuyệt đối, không dựa vào boot TAP.
3. `shift=6` **chưa** kiểm dưới tải host nặng: guest time là tất định, nhưng wall time tăng theo
   tải (timeout 20 s/boot vẫn còn dư).
4. `smp > 1` + icount **đã kiểm ở KT-49** (§12: matrix 1/2/4, per-hart timer 100 tick/hart).
5. Đo **jitter thật** (không icount) là **task riêng** — D2-74 điều kiện (d); không đổi D2 này.

## 11. PLIC + UART IRQ (KT-48) — claim/complete, context `2*hart+1`, 0 IRQ giả

Nguồn quyết định: **D2-72 Q7/Q8** (địa chỉ hardcode, `ndev` đọc từ register, context `2*hart+1`;
PLIC **cổ điển**, AIA hoãn K4'/K5'), task K3-PLAN §4.1 KT-48. Code: `vifast-plat-common::plic`
(logic an toàn, test host) + `vifast-plat-riscv64::plic` (MMIO/CSR, `unsafe` thuộc danh sách đóng
D2-17); `Platform::enable_irq`/`disable_irq` giờ là **PLIC thật**; `trap_dispatch` có nhánh
`Event::Interrupt { code: 9 }`.

### 11.1 Hằng số `[TK]` (PLIC cổ điển)

| Hằng số | Giá trị | Nghĩa |
|---|---|---|
| `PLIC_BASE` | `0x0c00_0000` | dữ kiện nền tảng QEMU `virt` (D2-72 Q7) |
| cửa sổ map | **2 × 2 MiB** | `0x0c00_0000..0x0c20_0000` (priority/pending/enable) + `0x0c20_0000..0x0c40_0000` (threshold/claim), Device RW NX; phủ context 15 của `MAX_HARTS = 8` |
| `context_of_hart(h)` | `2*h+1` | context **S-mode**; M-mode `2*h` **không** bật (TAP kiểm `enable_mmode=0`) |
| priority UART | **1** | `priority[10]` (PLIC chỉ forward nguồn priority > threshold) |
| threshold | **0** | context S-mode |
| IRQ UART0 | **10** | D2-72 Q7 |
| `IER.TX-empty` | `0x02` | mức cao khi THR rỗng ⇒ nguồn **level** thật, không phải trap tiêm |
| `UART_IRQ_LIMIT` | **8** | số ngắt thật thu rồi handler tự tắt (`IER` + enable) — không ngập |
| `PLIC_MAX_SOURCES` | **1023** | trần probe `ndev` (id hợp lệ `1..=1023`) |

### 11.2 Bản đồ đăng ký + cách đọc `ndev`

```text
0x0000 + 4*irq            priority[irq]          (IRQ 1..=ndev)
0x1000 + 4*(irq/32)       pending                (chỉ đọc)
0x2000 + 0x80*ctx + 4*w   enable word w của context
0x20_0000 + 0x1000*ctx    +0 threshold, +4 claim (đọc = claim, ghi = complete)
```

PLIC spec **không** có thanh ghi `ndev`; QEMU chỉ hiện thực priority cho nguồn `1..=ndev` và trả 0
cho nguồn không tồn tại ⇒ `probe_ndev` ghi 1, đọc lại, dừng ở nguồn đầu tiên không giữ giá trị
(không giả định 95 — D2-72 Q7). Probe là **chẩn đoán không phá huỷ** theo **D2-75** (2026-11-15):
mỗi nguồn `write-1 → read-back → ghi 0 → đọc lại 0`, panic nếu khôi phục thất bại; K3 **sở hữu**
PLIC nên trả priority 0 là an toàn và `init` đặt lại `priority[10] = 1` sau probe. Probe được phép
ở **cả ảnh sản phẩm**. Giá trị `ndev` của lần boot nằm trong log `# plic:`
(`v2/agent/JOURNAL/kt48-*.log`), **không** chép tay vào tài liệu (QD §9 Q12).

### 11.3 Đường ngắt UART (level → claim → complete → re-arm)

1. `plic::init`: probe `ndev` (panic nếu không phủ IRQ 10), `threshold = 0`, `priority[10] = 1`,
   **set** bit enable của context `2*hart+1`; đọc lại kiểm context S-mode bật, **context M-mode
   cùng hart = 0**, claim lúc chưa bật nguồn = 0.
2. Bật `IER.TX-empty` (ghi **đúng** 0x02 để RX/line khác không sinh IRQ) **rồi** `sie.SEIE` +
   `sstatus.SIE` (`plic::enable_external` — nguồn trước, cổng sau, đối xứng
   `timer::enable_timer`; P2a: trap entry/handler **không** ghi `sstatus`).
3. Trap `scause = (1<<63)|9` → `trap_dispatch` → `plic::on_external_interrupt`:
   **claim** context hiện tại → `PLIC_STATS.begin_claim` → line 10: đọc `IIR` (`0x2` = TX-empty),
   đếm, **complete**, rồi **re-arm** `IER` `0 → 2`. PLIC cổ điển là **level-triggered có gateway**:
   sau claim, nguồn còn mức cao **không** tự pending lại (gateway chờ nguồn xuống thấp) ⇒ nếu chỉ
   claim/complete thì TX-empty chỉ cho **đúng một** ngắt; re-arm tạo **level transition** để có
   ngắt kế. Đủ `UART_IRQ_LIMIT` ngắt: tắt `IER` + clear enable (không ngập) rồi complete.
4. Line **lạ**: đếm `unexpected` + **complete** (không treo, K3-PLAN §4.1). Claim 0: spurious —
   **không** complete (PLIC spec).
5. Handler **không in** (in làm trễ/ngập log); TAP đọc `PLIC_STATS` sau. `trap_dispatch` ghi
   `TRAP_STATS.record_interrupt(scause 9)` cho mỗi ngắt external; hardware đã xoá `sstatus.SIE`
   khi vào trap nên không lồng.

### 11.4 TAP (ảnh test chính, `1..15`) + bằng chứng

| TAP | Điều kiện đo |
|---|---|
| `ok 8 - plic window` | 2 cửa sổ PLIC walk thấy leaf **2 MiB Device RW NX** đúng PA; `ndev` **đọc từ register** > 10; context = `2*hart+1`; threshold 0; priority[10] = 1; enable context S-mode = 1, **context M-mode = 0**; claim rỗng = 0; **`Platform::disable_irq(10)` → đọc lại tắt, `Platform::enable_irq(10)` → đọc lại bật** (wrapper thật trên HW) |
| `ok 9 - uart irq` | `IER.TX-empty` + `sie.SEIE` bật; `uart_irqs = UART_IRQ_LIMIT (= 8 `[TK]`) ≥ 1`; `claims == completes`; `last_irq = 10`; `last_iir = 0x2` (TX-empty); handler đã tắt `IER` + enable; `traps_delta == claims`; 0 exception |
| `ok 10 - plic no fake irq` | **0 IRQ giả**: `spurious = unexpected = 0`; claim sau khi tắt = 0; `external_traps == claims + spurious`; `interrupts_delta == external_traps`; cổng external tắt **im lặng** ~1 ms; 0 exception |
| `ok 11..14 - timer …` | 4 TAP timer KT-47 chạy **sau** PLIC trong **cùng boot** ⇒ hồi quy timer sau khi bật PLIC (100 tick đơn điệu, 1.000 tick không mất, `scause=0x8…005`, drift [9,11] ms) |
| `ok 20 - trap nested panic` | trap lồng fatal như KT-45 (boot kết thúc `PANIC` có chủ đích); TAP 15..19 là SMP KT-49 (§12) |

Bằng chứng in ra (runner `grep` đúng các dòng, `ktest.sh` — số đo thật nằm trong log, không chép
tay): `# plic: … ndev=… ctx=… plat_off=1 plat_on=1 threshold=0 priority_uart=1 enable_ctx=1
enable_mmode=0 claim_idle=0`; `# plic map: … low_dev=1 high_dev=1 low_2m=1 high_2m=1 low_nx=1 high_nx=1`;
`# uart irq: irqs=… claims=… completes=… traps_delta=… balanced=1 spurious=0 unexpected=0
last_irq=10 last_iir=0x2 ier_off=1 plic_enable=0`; `# plic quiet: claim_idle=0 quiet=1
stats_unchanged=1 external_traps=… interrupts_delta=… spurious=0 unexpected=0`. Ảnh **sản phẩm** cũng in `# plic:` + `# uart irq:` (boot thật, **không**
icount) và `PANIC` nếu sai — cùng một handler/PLIC code với ảnh test.

### 11.5 Vì sao "context đúng hart"

- Context S-mode = `2*hart+1` (D2-72 Q7): hart 0..3 ⇒ 1/3/5/7 — phủ bằng **host test**
  `tests/plic.rs`; lần boot hart 0 in `ctx=1`.
- Init chỉ bật bit enable của **đúng context hiện tại** và kiểm **context M-mode cùng hart = 0**;
  handler claim/complete trên **cùng** context. Sai context ⇒ claim trả 0 (spurious) và TAP 9/10
  đỏ — không có đường "may mắn".
- `smp > 1` đã kiểm ở **KT-49** (§12): mỗi hart tự init context `2*h+1` của mình và TAP 18 đòi
  đủ `N` dòng `# plic hart=h … ctx=2h+1 … ok=1`, context M-mode `2*h` tắt.

### 11.6 Giới hạn phép đo

1. KT-48 chỉ **1 hart**; KT-49 đã boot `N = 1/2/4` và TAP 18 kiểm context `2*h+1` của **từng hart
   trong cùng một boot** (bằng chứng trong log `kt49-*`), nên giới hạn "chỉ hart 0" **hết**.
2. Probe `ndev` dựa vào hành vi QEMU (priority read-back); phần cứng khác cần DTB/khác (chưa có
   `vifast-dtb`, D2-72 Q7).
3. Chỉ **UART0 line 10**; IPI (KT-50), AIA/MSI, EOI/priority động **chưa** làm.
4. "0 IRQ giả" là bằng chứng **đếm** (`spurious`/`unexpected` = 0 + claim rỗng + im lặng sau khi
   tắt cổng), không phải phép đo độc lập của phần cứng; claim sai context sẽ lộ ở `# plic:`/TAP 9
   (claim 0 ⇒ spurious ⇒ đỏ).
5. Timer sau PLIC đo dưới `-icount shift=6` (D2-74) — cùng giới hạn §10.7; ảnh sản phẩm **không**
   icount chỉ đòi ≥1 tick (và PLIC/IRQ không phụ thuộc icount).

## 12. SMP — SBI HSM + PerHart (KT-49)

Nguồn quyết định: **D2-72 Q4** (hart theo DTB, `MAX_HARTS = 8` `[TK]`, matrix `1/2/4`, BSP bật AP
bằng SBI HSM sau khi dựng arena), **D2-72 Q7** (DTB chỉ kiểm magic), task K3-PLAN §4.1 KT-49. Code:
`vifast-plat-common::smp` (hằng số bố cục, `HartMask`, phân loại trạng thái SBI HSM — test host
được) + `vifast-plat-riscv64::smp` (SBI `ecall`, barrier, trạng thái runtime) + asm entry/trap ở
`vifast-plat-riscv64::lib` + linker script vùng NOLOAD per-hart + pha test SMP trong
`vifast-kbin-riscv64/src/ktest_smp.rs` (tách khỏi `ktest.rs` theo R1 ≤ 1.500 dòng).

### 12.1 Vì sao hart thứ cấp cần SBI HSM

OpenSBI `fw_dynamic` với `-smp N` chỉ nhảy vào S-mode ở **hart boot** (hart 0 — OpenSBI banner
`Domain0 Boot HART`); các hart còn lại nằm trong firmware ở trạng thái `STOPPED`. Chúng chỉ chạy khi
hart boot gọi **`sbi_hart_start(hartid, start_addr, opaque)`** (EID `0x48534D`, FID `0`); SBI spec
§8.1: hart được đánh thức vào S-mode với `satp = 0` (Bare), `a0 = hartid`, `a1 = opaque`, ngắt tắt.
Vì vậy:

1. Hart boot làm xong MMU (`mmu::init`) **trước** khi start AP — AP thừa hưởng cùng root page table
   qua `csrw satp` + `sfence.vma` (`mmu::adopt_satp`) khi vào.
2. `_start_secondary` (asm) nhận `a0 = hartid`, `a1 = opaque`; `tp ← a0`; `sp`/`sscratch` tính từ
   **record per-hart**; gọi `rust_secondary_main` của ảnh (bin cung cấp).
3. `smp::secondary_setup(opaque)` kiểm `opaque == "KT49"`, adopt `satp`, cài `stvec` (CSR per-hart),
   set bit `ONLINE` của mình rồi chờ `PHASE` (park bằng **spin**, không `wfi`: K3 chưa có IPI để
   đánh thức — KT-50). BSP phát `release()` khi vào pha SMP.
4. Hart lỗi (start trả lỗi, không online, timeout) ⇒ **log + report**, **không panic cả máy** —
   kể cả ảnh sản phẩm (chỉ in `# smp: WARN …` rồi tiếp tục; `run.sh` đỏ nếu `harts` khác `-smp N`
   nên cấu hình hỏng không bị che). Ảnh **test** để TAP 15 đỏ và các pha sau chỉ chạy trên hart
   online.

### 12.2 Bố cục NOLOAD per-hart `[TK]`

| Hằng số (`plat-common::smp`) | Giá trị | Nghĩa |
|---|---|---|
| `MAX_HARTS` | **8** | trần hart K3 (D2-72 Q4); matrix kiểm `1/2/4` |
| `HART_GUARD_BYTES` | **0x1000** | guard page mỗi hart (NONE) |
| `HART_STACK_BYTES` | **0x10000** | stack mỗi hart, `sp` ở đỉnh, mọc xuống guard của hart đó |
| `HART_TRAP_STACK_BYTES` | **0x4000** | trap stack mỗi hart; trapframe 288 B nằm trọn trong vùng |
| `HART_STRIDE` | **0x1_5000** | `guard + stack + trap stack` (record của một hart) |
| `HART_AREA_BYTES` | **0xA_8000** | `MAX_HARTS × HART_STRIDE` — vùng NOLOAD tổng |
| `ARENA_BYTES` | **64 KiB** | page table (KT-46), bắt đầu ở `__kernel_end` (sau record hart 7) |

```text
record hart h = [base + h*0x1_5000, base + (h+1)*0x1_5000)
  +0x0000 guard  (4 KiB, NONE ở identity / nửa cao / physmap)
  +0x1000 stack  (64 KiB, RW NX; đáy = guard của chính hart này)
  +0x11000 trap stack (16 KiB, RW NX; `sscratch` = đỉnh)
```

- Linker: `.stack_guard`/`.stack`/`.trap_stack` giữ nghĩa **hart 0**; `.hart_stacks` cấp địa chỉ
  cho 7 record còn lại; `__kernel_end` sau record hart 7. `mmu::init` **panic** nếu symbol linker
  không khớp hằng số `plat-common::smp` (`guard_offset`/`stack_top_offset`/`trap_top_offset`/
  `HART_AREA_BYTES`) — chặn rủi ro "stack AP chồng guard hart khác" của K3-PLAN.
- `mmu::map_kernel` map **mọi** trang NOLOAD trừ **guard của từng hart** (4 KiB leaf RW NX, cả
  identity và nửa cao); `mmu::map_physmap` khoét **mọi** guard khỏi chunk 2 MiB ⇒ không có cửa sau
  qua alias vật lý. `mmu::guard_owner(pa)` trả hart sở hữu guard.
- `trap_entry` đổi `sp` sang trap stack **của hart đang chạy** (tính `addi sp, sp, -TF_SIZE` từ
  `sscratch`), đặt lại `sscratch = frame + TF_SIZE`, và phát hiện trap lồng bằng
  `[trap_top − HART_TRAP_STACK_BYTES, trap_top)` — **không** dùng symbol toàn cục một hart như
  KT-45; `trapframe` ghi vào slot `smp::last_trap_frame(hart)` để TAP chứng minh `.trap_stack`
  per-hart.

### 12.3 Hart theo DTB: probe `hart_get_status`, từ chối hart ngoài máy

`plat-common::smp` phân loại mã `sbi_hart_get_status`:

| Mã SBI HSM | `HartStatus` | Quyết định của BSP |
|---|---|---|
| `0 STARTED` | `Started` | `AlreadyStarted` — không start lại, chờ online |
| `1 STOPPED` | `Stopped` | **`Start`** — gọi `sbi_hart_start(h, _start_secondary, "KT49")` |
| `2/3` pending | `Pending` | `AlreadyStarted` — đang chuyển, không start lại |
| `4/5/6` suspend | `Suspended` | `Reject` (K3 không hotplug/suspend) |
| `-3 INVALID_PARAM` | `Invalid` | **`Reject`** — hart **không có trong DTB/domain**; log `# smp: hart h … ngoai DTB — tu choi start`, **không panic cả máy** |
| lỗi khác | `Error(code)` | log mã, không start hart đó |

- Đây là "hart theo DTB" **không cần parse FDT**: firmware (OpenSBI) đã đọc DTB/`-smp` và trả
  `-3` cho hart ngoài máy; D2-72 Q7 vẫn giữ "chỉ kiểm magic DTB". Matrix 1/2/4 kiểm cả hai phía:
  `-smp 1` ⇒ hart 1..7 bị từ chối; `-smp 4` ⇒ chỉ hart 4..7 bị từ chối.
- **DTB magic**: QEMU `virt` đặt FDT ở **cuối RAM** (ngoài miền kernel identity-map), nên
  `dtb_magic_ok` đọc 4 byte đầu qua **physmap** (`mmu::physmap_alias`) — không thêm mapping mới,
  không tin `a1` mù. `a1` được `_start` lưu vào `SMP_DTB` **sau** khi zero `.bss`.
- `smp::init` chờ `ONLINE ⊇ PRESENT` với timeout 2 s; hết hạn ⇒ `timed_out = true`, log hart thiếu,
  trả report — **không treo**. `hart_start` trả `0` hoặc `-6 ALREADY_AVAILABLE` ⇒ coi là thành
  công; `PRESENT` gồm hart probe được (kể cả hart start lỗi, để TAP báo thiếu online).

### 12.4 Bộ đếm **PerHart** (timer/PLIC) — không lẫn giữa các hart

- `timer::TIMER_STATS` (KT-47) và `plic::PLIC_STATS` (KT-48) đổi từ một bộ đếm toàn cục thành
  **PerHart**: `TIMER_STATS.get(hart)`/`PLIC_STATS.get(hart)` là slot riêng của hart (mảng
  `[T; MAX_HARTS]`, `const`-init; `unsafe` không cần). Handler timer/external chạy trên hart nào
  cộng slot `hart_id()` của hart đó; `sbi_set_timer`/`sie.STIE`/`sstatus.SIE` là CSR per-hart nên
  deadline/nhịp của hart này không đụng hart kia.
- TAP 17 đòi mỗi hart `ticks ≥ 100`, `non_monotonic = 0`, `first` đúng bằng `rdtime` lúc **hart đó**
  arm (`snap.first == arm.now`), `armed = 1` ⇒ slot là của chính hart (không phải số gộp).
- TAP 18 đòi hart **boot** (hart chạy các TAP KT-48) vẫn giữ `uart_irqs = UART_IRQ_LIMIT` trong
  khi các hart khác `PLIC_STATS.get(h).traps = 0` (không claim external nào) ⇒ không lẫn. Dùng
  `boot` chứ không hardcode hart 0: OpenSBI có thể chọn boot hart khác 0 (log ảnh sản phẩm
  `-smp 4` không icount từng ghi `Domain0 Boot HART: 2`); kernel xử lý theo `a0` của lần boot.
- `TRAP_STATS`/`GUARD_STATS` **vẫn gộp toàn cục** (atomic): TAP 2..14 của KT-45..48 chạy **trước**
  pha SMP và SMP chỉ so **delta**/slot riêng; đây là phạm vi có chủ đích, ghi ở §12.8.5.

### 12.5 Barrier có timeout — "hart chậm không treo"

API (`vifast-plat-riscv64::smp`, dựng trên `HartMask` của `plat-common`):

```text
barrier_begin(round, expected)   BSP: xoá arrival, đặt expected, công bố round
barrier_join(round, timeout)     mọi hart: chờ round được công bố, set bit arrival của mình
barrier_wait_arrivals(timeout)   BSP: chờ arrivals == expected, trả (mask, timed_out)
barrier_release(round)           BSP: phát (released = round, TĂNG ĐƠN ĐIỆU)
barrier_wait_release(round)      hart: chờ released ≥ round
```

- `released` **không reset về 0** mỗi vòng (bug đo được khi boot `smp=2/4`: BSP sang vòng mới xoá
  release cũ trước khi AP kịp đọc ⇒ AP chờ hết timeout). Nó là số vòng tăng đơn điệu.
- Mỗi vòng barrier đều có timeout (1 s cho vòng thường, **200 ms** cho vòng "có hart không tồn
  tại") và mọi pha đều có timeout riêng (online 2 s, timer 2 s) ⇒ hart lỗi/chậm làm TAP đỏ, **không**
  làm treo cả máy.
- Vòng 5: hart `SLOW_HART = 1` (nếu online) **trễ 5 ms** trước khi join; BSP vẫn thấy đủ arrival
  (`timeout=0`) trong biên 1 s — bằng chứng barrier chờ hart chậm.
- Vòng 6: `expected = online | bit(hart đầu tiên không online)` — hart đó **không tồn tại** nên
  BSP phải `timeout=1`, `missing = bit đó`, rồi `release` và đi tiếp tới TAP 20 (không treo).
- Hart thứ cấp **park bằng spin** (không `wfi`): K3 chưa có IPI để đánh thức `wfi` (KT-50). Trong
  pha SMP, mọi hart chạy đúng chuỗi `alive → guard → timer → PLIC → barrier` với barrier giữa các
  pha; BSP cũng là một thành viên (hart boot).

### 12.6 TAP 15..19 (ảnh test chính) + runner matrix

| TAP | Điều kiện đo |
|---|---|
| `ok 15 - harts alive N` | SBI HSM start đủ hart `PRESENT`; `ONLINE = PRESENT`, `harts = N`; DTB magic hợp lệ; **mỗi hart in một dòng** `# hart h alive ticket=T tp=h …` (ticket là hoán vị `0..N-1`); `# harts alive: n=N … tickets=1` |
| `ok 16 - per-hart stack/guard` | mỗi hart `h`: stack/trap stack RW NX leaf 4 KiB, guard NONE ở identity + nửa cao + physmap, `guard_owner = h`, **tự chạm guard của mình** ⇒ load page fault `scause=13` thật, handler resume đúng **một** instruction, trapframe nằm trong `.trap_stack` của hart `h` (`frame_trap=1`) |
| `ok 17 - per-hart timer` | mỗi hart tự arm SBI TIME, nhận **100 tick** trong slot PerHart của mình, `first = rdtime` lúc arm, `non_monotonic = 0`; số dòng `# timer hart=h … ok=1` = N |
| `ok 18 - plic ctx per hart` | mỗi hart context `2*h+1`, context M-mode `2*h` **không** bật, threshold 0, priority UART 1, enable IRQ UART round-trip (bật → đọc lại 1, tắt → đọc lại 0) trên **context của hart**; các context khác nhau; `PLIC_STATS` PerHart không lẫn; số dòng `# plic hart=h … ok=1` = N |
| `ok 19 - barrier` | vòng 5: đủ arrival, `timeout=0`, hart chậm trễ thật (`slow_delayed=1`); vòng 6: `timeout=1`, `missing` đúng hart không online, `wait_ticks < 1 s`, máy tiếp tục |
| `ok 20 - trap nested panic` | trap lồng vẫn fatal (KT-45 P2b) — boot kết thúc `PANIC: nested trap …` |

- `expected.txt` (`v2/test/boot/expected.txt` + sidecar `.license` theo R3) là nguồn duy nhất của
  kế hoạch/số `ok` (KT-49: ktest 20/20, ktest-guard 3/3); `ktest.sh` kiểm `1..K`, tập `ok 1..K`
  không thiếu/lặp, đếm số dòng `ok`. Pha SMP test nằm ở `vifast-kbin-riscv64/src/ktest_smp.rs`.
- Matrix: `bash v2/test/boot/ktest.sh 50 matrix` chạy **50 boot mỗi N=1/2/4** (main + guard), log
  riêng `/tmp/k3-ktest-{main,guard}-smp<N>-<i>.log`, tổng kết theo N; `ktest.sh` kiểm thêm banner
  OpenSBI `Platform HART Count : N` (bằng chứng độc lập rằng máy ảo thật sự có N hart).
- Ảnh **sản phẩm**: `# smp: … harts=N … timeout=0`; hart lỗi chỉ log `# smp: WARN` (không panic cả
  máy), `run.sh N 1|2|4` kiểm `harts=N` nên thiếu hart vẫn đỏ — không icount (D2-72 Q2).
- Ảnh **guard** dùng guard của **hart boot thật** (`mmu::guard_range_of(hart_id())`, in
  `# guard image: hart=N`); runner đọc hart đó và đòi dòng fault/load/store đúng hart — không ghim
  hart 0 (đã tái hiện `Domain0 Boot HART: 3` với `-smp 4` không icount, ảnh guard vẫn 3/3 TAP).

### 12.7 Ngân sách và ảnh hưởng kích thước

- Vùng per-hart là **NOLOAD**: không vào `.data/.bss`, không cộng trần 262.144 B (D2-72 Q5);
  `.hart_stacks` báo riêng trong `budget.sh` (`.stack`/`.trap_stack` của hart 0 vẫn như cũ).
- Bộ đếm PerHart + trạng thái SMP là `static` nhỏ trong `.bss` (8 slot `TimerCounters`/`PlicStats`
  + mask/record). Page table per-hart vẫn cấp từ **arena RAM** (64 KiB) — thêm leaf 4 KiB cho
  stack/trap stack các hart nhưng vẫn **không** đội `.data+.bss`; `mmu::init` panic nếu arena/bố cục
  vượt 2 MiB (kiểm tuyến tính giữ nguyên từ KT-46).
- Số đo budget thật của cả ba ảnh nằm trong `v2/agent/JOURNAL/kt49-*.log` (QD §9 Q12 — tài liệu
  không chép số tay).

### 12.8 Giới hạn phép đo (đọc kèm §12.7)

1. **Không IPI** (KT-50): AP park bằng spin; chưa có "đánh thức hart đang ngủ", preemption hay
   scheduler đa hart. Matrix chỉ chứng minh bring-up + trạng thái per-hart, không chứng minh IPC.
2. **PLIC per hart** mới kiểm *đăng ký context + round-trip enable* trên IRQ UART; chưa giao ngắt
   thật cho nhiều hart cùng lúc (nguồn UART chỉ một line; KT-50 mới có IPI/line khác).
3. **Barrier timeout** dùng `rdtime` + spin, không dùng timer ngắt; "hart chậm" là hart trễ 5 ms,
   không phải hart bị treo thật. Hart **không tồn tại** được mô phỏng bằng bit kỳ vọng không có
   hart tương ứng (đúng ngữ nghĩa "expected mà không tới").
4. **QEMU icount chia lượt vCPU thô**: một hart có thể tới barrier muộn hàng chục ms ảo (đo được
   trong log `kt49-*`); vì vậy vòng timeout dùng 200 ms — **không** phải nới ngưỡng DoD nào (DoD
   KT-49 không đặt ngưỡng thời gian barrier; chỉ đòi "hart chậm không treo").
   Cùng lý do, khi `N=4` slot timer **của hart phụ** có thể có `late/missed > 0` (log `kt49-*`;
   giá trị đổi theo lượt chạy) — đây là hạn chế **chia lượt vCPU của icount**, không phải lỗi
   PerHart: TAP 17 chỉ đòi **≥ 100 tick + đúng slot của hart** (`first = rdtime` lúc hart đó arm,
   `non_monotonic = 0`), **không** đòi drift per-hart. Drift Δ tick ∈ [9,11] ms (D2-74) chỉ đo trên
   **BSP** ở TAP 14; chưa tách được biến "scheduling icount" khỏi "toán deadline per-hart" — muốn
   đo jitter timer per-hart là **task riêng**, không mở lại ngưỡng KT-47/49.
5. **`TRAP_STATS`/`GUARD_STATS` vẫn gộp toàn cục**: TAP 2..14 chạy trước pha SMP; trong pha SMP chỉ
   dùng **delta** và slot PerHart của timer/PLIC. Muốn thống kê trap theo hart từng phần ⇒ task sau.
6. **Chưa kiểm `MAX_HARTS = 8`**: matrix `1/2/4` (D2-72 Q4); `-smp 8` vượt yêu cầu K3 nhưng code
   hỗ trợ tới 8 (hằng số + `mmu::init` panic nếu linker lệch); soak 1.000 boot là KT-57.
7. **Không hotplug/suspend/`hart_stop`**: hart ngoài DTB bị từ chối; hart online không tắt giữa
   chừng (K3 chưa cần).

### 12.9 Không thuộc KT-49

IPI thật + hộp thư drain + `VmKick`/`TlbFlush` (KT-50), ASID/TLB shootdown (KT-51), object layer
(KT-52), U-mode/syscall (KT-53), harness TAP đầy đủ/conformance (KT-54/55), `FaultKind`/W^X
(KT-56), soak 1.000 + K3-DOD (KT-57), port core `no_std` (K3b — D2-72 Q13), hotplug/suspend,
scheduler/preemption đa hart. Xem bảng §9.
