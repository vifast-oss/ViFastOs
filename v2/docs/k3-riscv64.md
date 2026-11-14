<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# K3 — kernel riscv64 thật trên QEMU `virt` (mốc mở bằng KT-44)

Trạng thái: **KT-46** — MMU **Sv39 thật** (page table từ arena RAM, kernel nửa cao 2 MiB, physmap
NX, MMIO Device, `satp` mode 8) + **guard page-fault thật** (`scause=13/15`) trên nền trap KT-45;
tài liệu hằng số nền tảng + quy trình build/boot/trap/MMU của K3.
Nguồn quyết định: **D2-72** (`v2/agent/DECISIONS.md`) chốt Q1..Q13 của
[`review/agent0/K3-PLAN.md`](../../review/agent0/K3-PLAN.md) §3; chi tiết task ở §4/§4.1 cùng tệp.
Nhãn số liệu theo QD §2.2: `[TK]` thiết kế mới, `[GT]` giả thuyết nền tảng, `[ĐO]` chỉ trong log
do lệnh sinh ra. **Tài liệu này không chứa số đo viết tay** (QD §9 Q12): mọi số đo nằm ở
`v2/agent/JOURNAL/kt45-*.log` + `kt46-*.log` và do `v2/test/boot/*.sh` / `cargo test` in ra.

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
| MMU | **Sv39** (`satp.MODE=8`), 3 cấp × 512 entry, bảng 4 KiB; `VA_BITS=39` từ `vifast-pte`; kernel **identity** + **nửa cao** 2 MiB (image), NOLOAD 4 KiB; physmap RAM **2 MiB NX**; MMIO UART/PLIC **4 KiB Device**; guard page **NONE** | `[TK]` | KT-46 |
| Địa chỉ map | kernel identity `0x8020_0000`; kernel nửa cao `0xFFFF_FFC0_4000_0000`; physmap `0xFFFF_FFC0_0000_0000 + PA`; guard ở `0x8040_0000` (linker `ALIGN(2M)`) | `[TK]` | KT-46 |
| Arena page table | **64 KiB** RAM sau `__kernel_end`, bump allocator trong builder; root + bảng trung gian **không** `static`, không cộng trần `.data+.bss` | `[TK]` | KT-46, D2-72 Q5 |
| IRQ | `Platform::enable_irq`/`disable_irq` **stub** (KT-46 vẫn trả `false`); PLIC thật ở KT-48 | `[TK]` | D2-72 Q7/Q8, KT-45/46 |

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
             __bss_end
             . = ALIGN(2M)                       ← KT-46: tách vùng NOLOAD sang 2 MiB kế tiếp
0x8040_0000  .stack_guard (NOLOAD, 4 KiB)        ← guard page, MMU cưỡng chế NONE
0x8040_1000  .stack       (NOLOAD, 64 KiB)       ← `sp = __stack_top`, mọc xuống đáy
0x8041_1000  .trap_stack  (NOLOAD, 16 KiB)       ← `sscratch = __trap_stack_top`; mọi trap đổi sang đây
             __kernel_end                        ← arena page table (64 KiB) bắt đầu ở đây, KHÔNG là section
```

- **Vì sao `ALIGN(2M)`:** KT-46 map **kernel image** `[0x8020_0000, 0x8040_0000)` bằng **một leaf
  2 MiB** ở nửa cao (yêu cầu "kernel nửa cao 2 MiB"). Nếu guard/stack nằm trong chính leaf đó thì
  không thể đặt riêng guard page `NONE` ở granularity 4 KiB. Đưa vùng NOLOAD sang 2 MiB kế tiếp
  giải quyết cả hai: image 2 MiB, guard 4 KiB.
- `.stack_guard` + `.stack` + `.trap_stack` là **NOLOAD**: chỉ chiếm địa chỉ, không chiếm file ảnh,
  **báo riêng** — không cộng vào trần budget (D2-72 Q5). Guard 4 KiB nằm **ngay dưới**
  `__stack_bottom` (`__stack_guard_end == __stack_bottom`).
- **Arena** (KT-46) bắt đầu ở `__kernel_end` (sau `.trap_stack`), 64 KiB, căn 4 KiB. Root page table
  và mọi bảng trung gian được cấp từ đây (bump allocator), **không** `static`/`static mut`; arena
  được identity-map + nửa cao RW NX. Vì không phải section ELF, nó **không** vào `.data+.bss`.
- `_start` nhận `a0 = hartid`, chép vào `tp`; zero `.bss`; đặt `sp = __stack_top`,
  `sscratch = __trap_stack_top`; gọi `rust_main`. `Riscv64Virt::hart_id` đọc `tp`, `now` đọc `rdtime`.
- `.stack`/`.trap_stack` là section NOLOAD: **không nằm trong `filesz`** của LOAD (mặc dù LOAD
  `memsz` vẫn phủ tới `__kernel_end`) — đây là bài học KT-45; KT-46 **bắt buộc** map cả hai vùng
  trong page table kernel (TAP 6 ghi/đọc qua nửa cao để chứng minh), nếu thiếu thì trap đầu tiên
  sau khi bật `satp` sẽ page-fault vào chính `.trap_stack` chưa map.

## 4. Ba ảnh: sản phẩm + hai ảnh test (D2-08) — tách bằng feature, không `cfg(target_arch)`

| Ảnh | Bin | Feature | TAP | Runner |
|---|---|---|---|---|
| **Sản phẩm** | `vifast-kbin-riscv64` | — (mặc định) | `ok 1 - k3 boot` + `# k3 halt` | `v2/test/boot/run.sh` |
| **Test chính** | `vifast-kbin-ktest` | `ktest` | **8** — 4 TAP trap KT-45 + 3 TAP MMU KT-46 + trap lồng fatal cuối | `v2/test/boot/ktest.sh` |
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
- Ảnh test ở KT-44 chỉ chứng minh tồn tại/tách ảnh; harness TAP nhiều test + `expected.txt` +
  matrix smp `1/2/4` là **KT-54**.

## 5. Build / boot / đo

```bash
bash v2/test/boot/run.sh 1        # ảnh sản phẩm, 1 boot, 1 hart (DoD KT-44/45/46)
bash v2/test/boot/run.sh 50       # 50 boot 0 lỗi (DoD KT-46)
bash v2/test/boot/ktest.sh 1 1    # 2 ảnh test: 8 TAP chính + 3 TAP guard
bash v2/test/boot/ktest.sh 1 2    # matrix smp bắt đầu từ KT-49; KT-46 chỉ 1 hart

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

qemu-system-riscv64 -machine virt,aia=none,aclint=on \
    -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel <ảnh> -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
```

Cổng đo (do script thực thi, không phải người):

- **Budget** — `budget.sh` cộng `.data .sdata .bss .sbss` bằng `llvm-size -A` (đúng cách đo của
  K0.5/KT-07), trần **262.144 B** `[TK]` cho **cả ba ảnh**, in
  `# image=… data+bss=… stack=… guard=… trap_stack=…`; **exit 1** nếu vượt (D2-72 Q5). Page table
  nằm trong arena RAM nên **không** đội `.data+.bss`.
- **Boot** — `run.sh`/`ktest.sh`: timeout **20 s**/boot `[TK]` (D2-72 Q10), chỉ `kill <pid>` do
  chính script tạo (AGENTS §9 — cấm `pkill -f qemu-system`), đòi QEMU ≥ sàn **9.2**, đòi TAP đủ
  (`TAP version 13`, `1..K`, mọi dòng `ok`, marker/PANIC có chủ đích), **không** có PANIC lạ.
- **Cổng repo** — `./v2/bin/v2 gate --fast` **không** build kernel
  (`v2/Cargo.toml` `exclude = ["kernel"]`); kernel kiểm bằng build + `ktest.sh`/`run.sh` + host test
  `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common`.

## 6. `vifast-plat-common` + `vifast-plat-riscv64` (KT-44..46)

- `vifast-plat-common` (`no_std` + `#![forbid(unsafe_code)]`):
  - trait **`Platform`**: `put_str`, `halt`, `now`, `tick_hz`, `hart_id`, `set_trap_vector`,
    `enable_irq`/`disable_irq` (stub, KT-48); **`PanicHandler`** dùng chung; **`PlatformWriter`**.
  - module **`trap`** (KT-45): `TrapFrame` 288 B + offset hằng số + `const` assert `offset_of!`;
    `ExceptionKind`/`Exception`/`Event`; `decode_scause`/`decode_trap`; `handle_trap`;
    `TrapStats`/`TRAP_STATS`; `GuardStats`/`GUARD_STATS`; `report_fatal`/`report_guard_fault`/
    `report_interrupt`; `is_nested_trap_sp`.
  - module **`mmu`** (KT-46): bit PTE Sv39 (`V/R/W/X/U/G/A/D`), `PteFlags`, `MapError`, `Walk`,
    `PteMemory`, `is_canonical`, `walk_with`, `PageTableBuilder` (cấp bảng từ `&mut [u8]`, map
    4 KiB/2 MiB/1 GiB, walk phần mềm), `satp_sv39`/`satp_mode`/`satp_root`; dùng lại
    `vifast_pte::{walk_shape, Pte, VA_BITS=39, PTE_LEVELS=3, PAGE_SIZE_BITS}` — crate nội bộ,
    `no_std`, không dependency ngoài.
- `vifast-plat-riscv64` (asm/MMIO, `unsafe` thuộc danh sách đóng `kernel/vifast-plat-*`, D2-17):
  `_start`, `trap_entry` direct mode, `trap_dispatch`, probe KT-45; **module `mmu`** (KT-46): đọc
  linker symbol, arena từ `__kernel_end`, dựng identity + nửa cao + physmap + MMIO + guard NONE,
  `csrw satp` + `sfence.vma` hai bước, đọc lại `satp`, alias self-test, walk phần mềm
  (`PhysMemory`), `probe_guard_load`/`probe_guard_store`, `read_u64`/`write_u64` cho TAP.
- Phần **logic** chạy host (D2-72 Q12): `cargo test --manifest-path v2/kernel/Cargo.toml
  -p vifast-plat-common` — 13 test trap KT-45 + **8 test MMU KT-46** (map/walk 4K-2M-1G, từ chối
  mapping sai, VA canonical, guard NONE/physmap NX, `satp` mode 8, guard stats + dump). Miri
  **không** áp ảnh kernel `no_std`/asm/MMIO; Miri vẫn áp `lib/` + sim.

## 7. Trap khung (KT-45) + guard page-fault (KT-46, theo D2-73)

- **`stvec` direct mode**: một vector `trap_entry` (`.balign 4`, `MODE=0`), cài bằng
  `Platform::set_trap_vector`; `read_trap_vector` đọc lại để ảnh in bằng chứng.
- **Trap stack riêng**: `_start` đặt `sscratch = __trap_stack_top`. `trap_entry` làm
  `csrrw sp, sscratch, sp` rồi `addi sp, sp, -288` — trapframe 288 B nằm **trọn trong**
  `.trap_stack` (P1 của review KT-45). Lưu `x0..x31` + `sepc/sstatus/scause/stval` (offset là
  `const` operand từ `plat-common`), gọi `trap_dispatch`; khôi phục **chỉ** `sepc` + thanh ghi,
  `sp` **cuối cùng**, rồi `sret`. **Không** ghi `sstatus` khi ra (sret tự khôi phục SIE/SPP — P2a).
- **Trap lồng (P2b)**: nếu sp bị ngắt nằm trong `[__trap_stack_bottom, __trap_stack_top)` ⇒
  `nested_trap_halt` in `PANIC: nested trap …` + `DebugHalt(1)`, không resume. Ảnh test chính dùng
  đúng đường này làm TAP cuối (fatal có chủ đích).
- **Decode**: bit 63 = interrupt; còn lại `Event::Exception` cho **0/1/2/3/4/5/6/7/8/9/12/13/15**
  và `Unknown(mã)` (host test đủ bảng). Chính sách: `breakpoint`/`illegal`/`ecall` resume đúng cỡ
  instruction (2/4 byte); page fault/access fault/misaligned/`Unknown` **fatal**; interrupt bất
  ngờ fatal (timer/PLIC là KT-47/48).
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
| kernel identity — NOLOAD | `0x0000_0000_8040_0000..arena_end` | `= VA` | **4 KiB**; guard **NONE**; `.stack`/`.trap_stack`/arena RW NX |
| kernel nửa cao — image | `0xFFFF_FFC0_4000_0000 + Δ` | `0x8020_0000 + Δ` | **2 MiB** RWX |
| kernel nửa cao — NOLOAD | `0xFFFF_FFC0_4000_0000 + Δ`, `Δ ≥ 0x20_0000` | `= 0x8040_0000 + …` | **4 KiB**; guard **NONE**; RW NX |
| physmap RAM | `0xFFFF_FFC0_0000_0000 + PA` | `PA` | **2 MiB NX** (`V|R|W|A|D`); chunk chứa guard tách 4 KiB và **khoét** guard |
| UART0 (Device) | `0x0000_0000_1000_0000` | `= VA` | **4 KiB** `V|R|W|A|D`, X=0 |
| PLIC (Device) | `0x0000_0000_0c00_0000` | `= VA` | **4 KiB** `V|R|W|A|D`, X=0 |
| guard page | `0x0000_0000_8040_0000..0x8040_1000` | — | **NONE** (không có PTE hợp lệ) ở **cả** identity, nửa cao, physmap |

- Kernel **liên kết thấp** (`linker.ld` `0x8020_0000`) nên nó **chạy** ở identity mapping; nửa cao
  là **alias** để đọc/ghi (và để user root copy entry sau này — K3 chưa có user root). Đây là lựa
  chọn có chủ đích, không phải "quên chạy nửa cao": relocating image sang nửa cao cần linker/entry
  khác và không thuộc KT-46.
- **2 MiB cho image, 4 KiB cho NOLOAD** vì guard page cần granularity 4 KiB; linker đặt vùng NOLOAD
  sang `0x8040_0000` (`ALIGN(2M)`) để leaf 2 MiB của image không phủ guard. Physmap cũng dùng
  leaf 2 MiB (yêu cầu "2 MiB/1 GiB") và khoét guard.
- `.trap_stack`/`.stack` là NOLOAD: **không nằm trong `filesz`** của LOAD (LOAD `memsz` vẫn phủ
  tới `__kernel_end`); KT-46 map **tường minh** cả hai (TAP 6 ghi/đọc qua nửa cao). Thiếu map ⇒
  trap đầu tiên sau `satp` sẽ fault vào trap stack.
- MMIO map **4 KiB** UART/PLIC tại đúng PA (D2-72 Q7); PLIC window rộng hơn (enable/claim) thuộc
  KT-48, sẽ mở rộng mapping khi cần. CLINT không map (timer qua SBI — KT-47).

### 8.2 Arena page table

- Root page table cấp từ **arena RAM** bắt đầu ở `__kernel_end` (sau `.trap_stack`), cỡ **64 KiB**,
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

- Guard page 4 KiB **ngay dưới** `__stack_bottom`; entry **không hợp lệ** ở identity, nửa cao và
  physmap. Chạm vào (dù đọc hay ghi) ⇒ **page fault phần cứng thật** vào `stvec`:
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

## 9. Phạm vi: KT-46 **không** làm gì

| Không làm | Thuộc |
|---|---|
| Timer `set_timer`, tick 100 Hz, deadline/hart, đo drift | KT-47 |
| PLIC enable/claim/complete, UART IRQ 10, mapping PLIC rộng hơn 4 KiB | KT-48 |
| SBI HSM, stack/arena/PerHart theo hart, boot barrier, guard stack AP | KT-49 |
| IPI thật, hộp thư drain, `TlbFlush`/`VmKick` | KT-50 |
| ASID thật, `sfence.vma` theo ASID/VA, shootdown ack + timeout, mutation bỏ flush | KT-51 |
| Object layer `no_std` + Retype nội bộ (VSpace/PageTable/Frame từ Untyped) | KT-52 |
| U-mode, 3 syscall, nạp `init` ELF, user root copy entry nửa cao | KT-53 |
| Harness TAP đầy đủ + `expected.txt` + matrix smp 1/2/4 | KT-54 |
| Conformance 5 `.scn` × sim/QEMU | KT-55 |
| `FaultKind` thật + W^X đầy đủ (tách text/rodata RWX→RX) + một hunk `spec/abi.toml` | KT-56 |
| Soak 1.000 boot + K3-DOD + perf | KT-57 |
| Port core `no_std` (CNode/TCB/IPC/sched) | **K3b** (D2-72 Q13) |
