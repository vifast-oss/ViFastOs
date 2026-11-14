<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K3 — kernel riscv64 thật trên QEMU `virt` (mốc mở bằng KT-44)

Trạng thái: **KT-45** — khung trap riscv64 (`stvec` direct, trapframe, decode `scause`, panic dump)
trên nền KT-44; tài liệu hằng số nền tảng + quy trình build/boot/trap của K3.
Nguồn quyết định: **D2-72** (`v2/agent/DECISIONS.md`) chốt Q1..Q13 của
[`review/agent0/K3-PLAN.md`](../../review/agent0/K3-PLAN.md) §3; chi tiết task ở §4/§4.1 cùng tệp.
Nhãn số liệu theo QD §2.2: `[TK]` thiết kế mới, `[GT]` giả thuyết nền tảng, `[ĐO]` chỉ trong log
do lệnh sinh ra. **Tài liệu này không chứa số đo viết tay** (QD §9 Q12): mọi số đo nằm ở
`v2/agent/JOURNAL/kt45-{budget,boot,boot50,ktest,gate-fast}.log` và do `v2/test/boot/*.sh` in ra.

## 1. K3 mới là kernel thật

- **K2** là mô hình (`vifast-sim` + `vifast-pte`), **không** phải kernel chạy trên phần cứng (D2-71).
- **K3** (KT-44..57) là lần đầu kernel riscv64 chạy **thật** trên QEMU `virt` qua OpenSBI: trap,
  MMU Sv39 (KT-46), timer SBI (KT-47), PLIC/UART IRQ (KT-48), SMP (KT-49), IPI (KT-50),
  TLB/shootdown (KT-51), object layer (KT-52), U-mode (KT-53).
- K3 **không** phải kernel ABI đầy đủ (D2-72 **Q13**): core CNode/TCB/IPC/scheduler vẫn model-only;
  port core `no_std` là phase **K3b**, mở bằng task port core đầu tiên (KT-58) trước `procsrv`/shell.
- Cổng `.data + .bss ≤ 256 KiB` `[TK]` (QD §3.5) **giữ nguyên** ở K3, áp cho **cả hai** ảnh
  (D2-72 Q5) — không nới, không tách ngân sách riêng cho ảnh test.

## 2. Hằng số nền tảng (không mở `spec/kernel.toml` — D2-72 Q1)

| Hạng mục | Giá trị | Nhãn | Nguồn |
|---|---|---|---|
| QEMU | sàn **≥ 9.2**; bản trên máy đo `11.1.3` | `[TK]` / `[GT]` | D2-72 Q2 |
| Máy ảo | `-machine virt,aia=none,aclint=on` (không `virt-<version>`) | `[TK]` | D2-72 Q2/Q8 |
| CPU / RAM / hart | `-cpu rv64 -m 128M -smp 1` (matrix kiểm `1/2/4`), `MAX_HARTS = 8` | `[TK]` | D2-72 Q2/Q4 |
| Firmware | OpenSBI `fw_dynamic` nạp **trực tiếp** qua `-bios`, không U-Boot/EDK2; bản đi kèm QEMU `v1.8.1` | `[GT]` | D2-72 Q3 |
| Nạp kernel | S-mode tại `0x8020_0000`, `a0 = hartid`, `a1 = địa chỉ DTB` | `[GT]` | K0.5 (KT-07) |
| UART0 | `0x1000_0000` (16550A), IRQ **10** | `[GT]` | D2-72 Q7 |
| PLIC | `0x0c00_0000`, cửa sổ `0x60_0000`; `ndev` **đọc từ register**, không giả định; context S-mode `2*hart+1` | `[GT]` | D2-72 Q7 |
| CLINT/ACLINT | `0x0200_0000` (`aclint=on` ⇒ IPI/timer qua ACLINT) | `[GT]` | D2-72 Q7 |
| Trang test | `0x0010_0000` (`test@100000`) | `[GT]` | D2-72 Q7 |
| Timer | **SBI TIME** `set_timer` + `sie.STIE`, timebase **10 MHz**, tick **100 Hz**; Sstc hoãn | `[TK]` | D2-72 Q6 |
| Ngắt | **PLIC cổ điển**, AIA (APLIC/IMSIC) hoãn K4'/K5' | `[TK]` | D2-72 Q8 |
| DTB | chỉ kiểm magic nếu OpenSBI truyền ở `a1` (log/cảnh báo, **không** parse đầy đủ) | `[TK]` | D2-72 Q7 |
| Soak | `1000 boot × 3 luồng` trên máy `ps`, timeout **20 s/boot**, `0/1000`, không retry | `[TK]` | D2-72 Q10 |
| Trap | `stvec` **direct mode** (một vector, `MODE=0`); trap stack riêng **16 KiB**; trapframe **288 B** = 32×8 (x0..x31) + 4×8 (`sepc/sstatus/scause/stval`) | `[TK]` | D2-73, KT-45 |
| IRQ | `Platform::enable_irq`/`disable_irq` **stub** (KT-45 luôn trả `false`); PLIC thật ở KT-48 | `[TK]` | D2-72 Q7/Q8, KT-45 |

Ghi chú kiểm chứng: `run.sh`/`ktest.sh` in `# qemu <version>` và `# opensbi <version>` **từ log
boot thật**; QEMU dưới sàn **9.2** (so cả major.minor) ⇒ **FAIL**, không tự hạ (D2-72 Q2). OpenSBI banner
(`Platform Timer Device : aclint-mtimer @ …Hz`, `Platform IPI Device : aclint-mswi`) là log
`[ĐO]`, dùng để đối chiếu timebase ở KT-47. Cửa sổ PLIC `0x60_0000` theo K3-PLAN §1; OpenSBI
domain region phủ `0x0c00_0000–0x0c3f_ffff` `[GT]` — KT-48 đọc `ndev` từ register, không giả định.

## 3. Bản đồ ảnh và linker script

```
0x8020_0000  .text        KEEP(.text.entry) — `_start` trong vifast-plat-riscv64
             .rodata      (chuỗi TAP, chẩn đoán)
             .data / .sdata
             .bss / .sbss / COMMON
             .stack_guard (NOLOAD, 4 KiB)   ← trang bảo vệ, MMU của KT-46 mới cưỡng chế
             .stack       (NOLOAD, 64 KiB)  ← `sp = __stack_top`, mọc xuống đáy
             .trap_stack  (NOLOAD, 16 KiB)  ← `sscratch = __trap_stack_top`; mọi trap đổi sang đây
             __kernel_end
```

- `.stack` + `.stack_guard` là **NOLOAD**: chỉ chiếm địa chỉ, không chiếm file ảnh, và **báo
  riêng** — không cộng vào trần budget (D2-72 Q5).
- `_start` nhận `a0 = hartid` của OpenSBI, chép vào `tp` (`mv tp, a0`), đặt `sp = __stack_top` rồi
  `call rust_main`, và đặt `sscratch = __trap_stack_top` (KT-45). `Riscv64Virt::hart_id` đọc lại
  `tp`, `now` đọc `rdtime` — không cần `static mut`.
- Trang bảo vệ 4 KiB nằm **ngay dưới** `__stack_bottom` (`__stack_guard_end`). KT-44 mới chỉ dành
  chỗ + đặt symbol; ánh xạ quyền `NONE` và fault thật thuộc **KT-46**.
- `.stack`/`.stack_guard`/`.trap_stack` đều `NOLOAD`: `budget.sh` báo **riêng** cả ba
  (`# image=… stack=… guard=… trap_stack=…`), không cộng vào trần `data+bss` (D2-72 Q5).

## 4. Hai ảnh: sản phẩm và test (D2-08) — tách bằng feature, không `cfg(target_arch)`

| Ảnh | Bin | Feature | Marker TAP | Runner |
|---|---|---|---|---|
| **Sản phẩm** | `vifast-kbin-riscv64` | — (mặc định) | `ok 1 - k3 boot` + `# k3 halt` | `v2/test/boot/run.sh` |
| **Test** | `vifast-kbin-ktest` | `ktest` | `ok 1 - ktest image` + `# ktest halt` | `v2/test/boot/ktest.sh` |

- Cùng crate `v2/kernel/vifast-kbin-riscv64`, cùng `linker.ld`; bin test khai
  `required-features = ["ktest"]` nên **build mặc định không sinh ảnh test**.
- `run.sh` **fail** nếu ảnh sản phẩm còn **một** chuỗi `ktest`; `ktest.sh` **fail** nếu ảnh test
  **không** có chuỗi đó ⇒ chứng minh tách ảnh thật, không chỉ đổi tên.
- `PanicHandler` + trait `Platform` nằm ở `vifast-plat-common` nên **hai ảnh dùng chung một
  handler**, không chép đôi code.
- Ảnh test ở KT-44 chỉ chứng minh tồn tại/tách ảnh; harness TAP nhiều test + `expected.txt` +
  matrix smp `1/2/4` là **KT-54**.

## 5. Build / boot / đo

```bash
bash v2/test/boot/run.sh 1        # ảnh sản phẩm, 1 boot, 1 hart (DoD của KT-44)
bash v2/test/boot/run.sh 1 2      # 1 boot, 2 hart (matrix 1/2/4 dùng từ KT-49)
bash v2/test/boot/ktest.sh 1 1    # ảnh test: KT-45 đòi 5 TAP (image + 3 trap + nested)

bash v2/test/boot/budget.sh \
  v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 product
```

Lệnh thô bên trong (không cần nhớ, chỉ để tra):

```bash
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
    --bin vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf --release
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
    --bin vifast-kbin-ktest --features ktest --target riscv64gc-unknown-none-elf --release

qemu-system-riscv64 -machine virt,aia=none,aclint=on \
    -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel <ảnh> -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
```

Cổng đo (do script thực thi, không phải người):

- **Budget** — `budget.sh` cộng `.data .sdata .bss .sbss` bằng `llvm-size -A` (đúng cách đo của
  K0.5/KT-07), trần **262.144 B** `[TK]` cho **cả hai** ảnh, in
  `# image=… data+bss=… stack=… guard=…`; **exit 1** nếu vượt (D2-72 Q5).
- **Boot** — `run.sh`/`ktest.sh`: timeout **20 s**/boot `[TK]` (D2-72 Q10), chỉ `kill <pid>` do
  chính script tạo (AGENTS §9 — cấm `pkill -f qemu-system`), đòi QEMU ≥ sàn **9.2**, đòi TAP đủ
  `TAP version 13` + `1..1` + marker `ok` + marker halt và **không** có dòng `PANIC`.
- **Cổng repo** — `./v2/bin/v2 gate --fast` **không** build kernel
  (`v2/Cargo.toml` `exclude = ["kernel"]`); kernel kiểm bằng `cargo build --target
  riscv64gc-unknown-none-elf` + log boot QEMU.

## 6. `vifast-plat-common` (KT-44)

- `no_std` + `#![forbid(unsafe_code)]`; trait **`Platform`**: `put_str`, `halt`, `now`, `tick_hz`,
  `hart_id`, và từ KT-45: `set_trap_vector`, `enable_irq`/`disable_irq` (stub, KT-48);
  **`PanicHandler`** dùng chung; **`PlatformWriter`** = `core::fmt::Write` trên `put_str` (in số
  không cần `alloc`).
- Module **`trap`** (KT-45, thuần logic — test host): `TrapFrame` `#[repr(C)]` + offset hằng số
  (asm dùng làm `const` operand, `const` assert chặn lệch bố cục), `ExceptionKind`/`Exception`/
  `Event`, `decode_scause`/`decode_trap`, `handle_trap` (resume/fatal), `TrapStats` +
  `TRAP_STATS`, `report_fatal`/`report_interrupt`.
- `now`/`tick_hz` ở K3 là **stub đồng hồ monotonic** (`rdtime`, 10 MHz); timer thật (tick 100 Hz,
  deadline mỗi hart) là **KT-47**. `halt` không in marker — từng ảnh tự in dòng halt của mình.
- Phần asm/MMIO ở `vifast-plat-riscv64` (danh sách đóng `kernel/vifast-plat-*`, D2-17); mọi phần
  logic ở `vifast-plat-common`.
- Logic đó chạy được **trên host** (D2-72 Q12): `cargo test --manifest-path v2/kernel/Cargo.toml
  -p vifast-plat-common`. Miri **không** áp ảnh kernel `no_std`/asm/MMIO; Miri vẫn áp `lib/` + sim.

## 7. Trap khung (KT-45, theo D2-73)

- **`stvec` direct mode**: một vector duy nhất `trap_entry` (`.balign 4`, `MODE=0`), cài bằng
  `Platform::set_trap_vector`; `read_trap_vector` đọc lại để ảnh in bằng chứng.
- **Trap stack riêng**: `_start` đặt `sscratch = __trap_stack_top`. `trap_entry` làm
  `csrrw sp, sscratch, sp` (sp = đỉnh trap stack, sscratch = `sp` bị ngắt) rồi
  `addi sp, sp, -288` — trapframe 288 B nằm **trọn trong** `.trap_stack`, không ghi ra ngoài
  vùng stack (bẫy P1 của review KT-45). Sau khi lưu `x0..x31` + `sepc/sstatus/scause/stval`,
  gọi `trap_dispatch`; khôi phục **chỉ** `sepc` + thanh ghi, `sp` **cuối cùng**, rồi `sret`.
  **Không** ghi `sstatus`: `sret` tự khôi phục `SIE` từ `SPIE` và privilege từ `SPP`; ghi sớm có
  thể bật interrupt khi frame chưa thoát (chuẩn bị KT-47). Kiểm chứng: `grep -n 'csrw sstatus'`
  trong đoạn `trap_entry` = **0**; lần xuất hiện duy nhất còn lại trong `vifast-plat-riscv64` là
  restore `sstatus` của **probe tiêm** sau khi trap tiêm quay lại — không phải đường trap thật.
  Không hardcode offset trong asm:
  offset là `const` operand lấy từ `vifast-plat-common::trap`; `const` assert + host test đối
  chiếu `offset_of!`.
- **Trap lồng (P2b)**: sau khi lưu frame, asm đọc `sp` bị ngắt từ frame; nếu nằm trong
  `[__trap_stack_bottom, __trap_stack_top)` thì đó là trap xảy ra **trong handler** ⇒ gọi
  `nested_trap_halt` in `PANIC: nested trap …` + `DebugHalt(1)`, **không** khôi phục/resume frame
  cũ. Bố cục: `sscratch` đã reset về `__trap_stack_top` trước khi phát hiện, nên frame lồng ở
  **đúng slot** `top-288` và **đè** frame ngoài — vô hại vì halt ngay. Host test
  `is_nested_trap_sp` kiểm predicate; TAP 5 trên QEMU kiểm hành vi thật (boot dừng có chủ đích).
- **Decode**: bit 63 = interrupt (`Event::Interrupt { code }`); còn lại `Event::Exception` với
  `ExceptionKind` cho **0/1/2/3/4/5/6/7/8/9/12/13/15** và `Unknown(mã)` (host test đủ bảng).
  `stval` của `illegal` là instruction lỗi; `breakpoint`/page fault giữ `stval` phần cứng.
- **Chính sách KT-45**: `breakpoint`/`illegal`/`ecall` ⇒ resume (cộng `sepc` đúng **2 hay 4**
  byte; `ecall` luôn 4, `breakpoint`/`illegal` đọc halfword tại `sepc` vì instruction đã fetch
  được); page fault/access fault/misaligned/`Unknown` ⇒ **fatal**: `report_fatal` in
  `PANIC: trap <Kind> scause=… sepc=… stval=… sstatus=…` + `ra/sp/gp/tp` + `a0..a7`, rồi
  `DebugHalt(1)`; interrupt bất ngờ ⇒ `PANIC: unexpected interrupt …` + halt (timer/PLIC là
  KT-47/48). `TRAP_STATS` giữ `scause/sepc/stval/sstatus` **tại thời điểm trap** (không phải sepc
  sau resume) để TAP kiểm được.
- **TAP (D2-73)**: `ktest.sh` đòi **5 TAP** — `ok 1 - ktest image`,
  `ok 2 - trap breakpoint` (trap thật, `scause=3`, resume đúng `sepc`),
  `ok 3 - trap ecall sbi` (`ecall` **từ S-mode**: OpenSBI xử lý ở M-mode, trả
  `SBI_ERR_NOT_SUPPORTED`, resume đúng `sepc` — **không** claim trap S-mode),
  `ok 4 - trap illegal inject` (**tiêm**: đặt `scause=2`/`stval`/`sepc` bằng CSR rồi nhảy
  `trap_entry`; **không** claim phần cứng), và `ok 5 - trap nested panic` (**trap lồng** có chủ
  đích: `PANIC: nested trap …` + halt, không treo/không resume; runner chỉ chấp nhận **đúng**
  dòng PANIC này).
- **Vì sao phải tiêm (bằng chứng)**: OpenSBI v1.8.1 `delegate_traps()` đặt `MEDELEG` chỉ gồm
  cause 0/3/8 (+ page fault 12/13/15 khi platform có `mfaults_delegation`); **bit 2 (illegal) và
  bit 9 (ecall-S) = 0**. Probe QEMU thật: `ebreak` → `scause=3`; `.4byte` illegal không vào
  `stvec` (ở lại M-mode); `ecall` S-mode trả `SBI_ERR_NOT_SUPPORTED`. Log:
  `v2/agent/JOURNAL/kt45-probe-medeleg.log`, `kt45-probe-illegal-mmode.log`. D2-73 chốt giữ đủ
  khung + TAP thật (a)/(b) và illegal bằng host test + trap tiêm; **cấm** đổi firmware M-mode
  (trái D2-72 Q3). Page-fault thật (`scause` 13/15) chuyển **KT-46** (guard page).
- **Đo**: `kt45-{red-ktest,ktest,boot,boot50,budget,gate-fast,fatal-trap,nested-trap}.log` trong
  `v2/agent/JOURNAL/` (số đo do script in, không chép tay).

## 8. Phạm vi: KT-45 **không** làm gì

| Không làm | Thuộc |
|---|---|
| Page-fault thật (`scause` 13/15), guard page fault | KT-46 |
| Timer `set_timer`, tick 100 Hz, deadline/hart, đo drift | KT-47 |
| PLIC enable/claim/complete, UART IRQ 10 | KT-48 |
| SBI HSM, stack/arena/PerHart theo hart, barrier | KT-49 |
| IPI thật, hộp thư drain, `TlbFlush`/`VmKick` | KT-50 |
| ASID thật, `sfence.vma`, shootdown ack + timeout | KT-51 |
| Object layer `no_std` + Retype nội bộ | KT-52 |
| U-mode, 3 syscall, nạp `init` ELF | KT-53 |
| Harness TAP đầy đủ + `expected.txt` + smp matrix | KT-54 |
| Conformance 5 `.scn` × sim/QEMU | KT-55 |
| `FaultKind` thật + một hunk `spec/abi.toml` | KT-56 |
| Soak 1.000 boot + K3-DOD + perf | KT-57 |
| Port core `no_std` (CNode/TCB/IPC/sched) | **K3b** (D2-72 Q13) |
