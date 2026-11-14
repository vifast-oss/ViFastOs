# KT-45 — trap khung riscv64 (`stvec` direct, trapframe, decode, panic dump)

Ngày: 2026-11-14. Agent 2 dòng OS. HEAD vào: `faba5cb8` ([KT-00] D2-73). Tham chiếu:
`review/agent0/K3-PLAN.md` §4.1 KT-45 (gốc) + **D2-73** (điều chỉnh DoD theo ràng buộc
firmware) + D2-72 Q2/Q3/Q5/Q12 + `v2/docs/k3-riscv64.md` §7.

## 1. Vào / baseline / đỏ trước

- `agentctl next --project v2` ⇒ **KT-45**; QUEUE `READY`; `git rev-list --count HEAD..origin/main`
  = **0**; working tree có `viscript/` của tab khác — **không chạm**, chỉ stage `v2/`.
- Baseline `./v2/bin/v2 gate --fast` ⇒ **PASS 569 test/73 suite**, L1 1.000 seed 1.396 ms
  ([`kt45-baseline-gate-fast.log`](kt45-baseline-gate-fast.log)).
- **Đỏ trước** ([`kt45-red-ktest.log`](kt45-red-ktest.log)): `ktest.sh` được siết đòi
  `1..4` + `ok 2 - trap breakpoint` + `ok 3 - trap ecall sbi` + `ok 4 - trap illegal inject`;
  ảnh test KT-44 chỉ có `1..1`/`ok 1 - ktest image` ⇒
  `not ok 1 - ktest boot 1: thiếu plan-1..4 ok2-breakpoint ok3-ecall-sbi ok4-illegal-inject`,
  `EXIT=1`, **đúng lý do** (chưa có 3 TAP trap), không phải lỗi hạ tầng. Sau vòng review P2b,
  `ktest.sh` thêm **TAP 5** (trap lồng); red-before ở trên vẫn là log đỏ thật của bộ 4 TAP ban
  đầu — TAP 5 chỉ được thêm **sau** khi code đã xanh + review, không dùng để "đỏ giả".

## 2. Phát hiện chặn DoD và bằng chứng (D2-73)

DoD gốc đòi 3 TAP **trap phần cứng**: `ecall` từ S-mode, `breakpoint`, `illegal instruction`.
Probe QEMU 11.1.3 + OpenSBI v1.8.1 `fw_dynamic` (đúng D2-72 Q2/Q3) cho thấy **2/3 bất khả thi**:

- OpenSBI v1.8.1 `delegate_traps()` (`lib/sbi/sbi_hart.c:212-250`) đặt
  `MEDELEG = BIT(0) | BIT(3) | BIT(8)` (+ page fault 12/13/15 ở platform có
  `mfaults_delegation`); in ra `MEDELEG = 0x0000000000f4b509`. **Bit 2 (illegal) = 0**,
  **bit 9 (ecall từ S) = 0** — ecall-S phải ở M-mode cho SBI.
- Probe thật ([`kt45-probe-medeleg.log`](kt45-probe-medeleg.log)): `ebreak` → vào `stvec` S-mode,
  `scause=3`, `stval=sepc`; `.4byte 0x2`/`0xffffffff`/`0x0` → **không** vào `stvec` (ở lại M-mode);
  `ecall` a7=0xdeadbeef từ S → trả `a0=0xfffffffffffffffe` (`SBI_ERR_NOT_SUPPORTED`) và resume
  `sepc+4`, **không** qua `stvec` ([`kt45-probe-illegal-mmode.log`](kt45-probe-illegal-mmode.log):
  illegal treo ở M-mode, không có log `stvec`).
- FWFT (EID 0x46574654, feature 0) bật delegation misaligned được nhưng QEMU xử lý unaligned
  native ⇒ không sinh trap; FWFT **không** có feature illegal/ecall-S.

⇒ Báo lead; lead chốt **A** và Agent 0 ghi **D2-73** (`faba5cb8`): giữ đủ khung + decode mọi
`scause` (host test), TAP thật chỉ `breakpoint` + `ecall-S` (SBI, không claim trap S-mode),
`illegal` bằng host test + **trap tiêm** (ghi rõ "inject"), page-fault thật chuyển KT-46, **cấm**
đổi firmware M-mode. Deviation này được ghi ở đây + `STATE.md` + `v2/docs/k3-riscv64.md` §7.

## 3. Việc đã làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-common/src/trap.rs` (mới) | `TrapFrame` `#[repr(C)]` 288 B (32×8 + 4×8), offset hằng số + `const` assert `offset_of!`; `ExceptionKind`/`Exception`/`Event`; `decode_scause`/`decode_trap`; `handle_trap` (resume/fatal); `TrapStats`/`TRAP_STATS`; `report_fatal`/`report_interrupt` |
| `v2/kernel/vifast-plat-common/src/lib.rs` | `pub mod trap`; `Platform` + `set_trap_vector`, `enable_irq`/`disable_irq` (stub KT-48) + impl cho `&P` |
| `v2/kernel/vifast-plat-common/tests/host.rs` | 12 host test: bố cục trapframe, decode 0/1/2/3/4/5/6/7/8/9/12/13/15/Unknown, interrupt bit 63, resume 2/4 byte, fatal giữ `sepc`, dump `PANIC` + regs + halt(1), stats, stub IRQ |
| `v2/kernel/vifast-plat-riscv64/src/lib.rs` | `_start` đặt `sscratch=__trap_stack_top`; `trap_entry` direct-mode (lưu/khôi phục 32 thanh ghi + 4 CSR, offset là `const` operand từ `plat-common`); `trap_dispatch`; `trap_vector`/`install_trap_vector`/`read_trap_vector`; `probe_breakpoint`/`probe_ecall_sbi`/`probe_injected_trap`; `Platform` impl mới |
| `v2/kernel/vifast-kbin-riscv64/src/main.rs` | ảnh sản phẩm cài `stvec` direct-mode, in lại `stvec`/`direct=`, TAP giữ `ok 1 - k3 boot` |
| `v2/kernel/vifast-kbin-riscv64/src/ktest.rs` | ảnh test 4 TAP: image, breakpoint (thật), ecall sbi (thật qua M-mode), illegal **inject**; kiểm `TRAP_STATS` (`scause`, `sepc` tại trap, `sepc+4` = địa chỉ resume) |
| `v2/kernel/vifast-kbin-riscv64/linker.ld` | `.trap_stack (NOLOAD, 16 KiB)` + `__trap_stack_bottom/top`; `__kernel_end` sau trap stack |
| `v2/test/boot/budget.sh` | báo riêng `trap_stack=` (NOLOAD), không đổi trần |
| `v2/test/boot/ktest.sh` | siết kỳ vọng đúng 4 TAP (đỏ trước ở §1) |
| `v2/docs/k3-riscv64.md` | §2 hằng trap/IRQ, §3 `.trap_stack`, §6 Platform/trap module, §7 trap khung + deviation D2-73, §8 phạm vi KT-45 |

## 3.1 Vá review độc lập (P1..P4) — cùng ngày

Reviewer `reviewer-kt45` verdict **DRIFT** (không UNSAFE), 2 lỗi thật + 2 nit; đã vá **trước
commit**:

- **P1 (blocking)** — trapframe ghi ngoài `.trap_stack` (thiếu `addi sp, sp, -288`): thêm
  `addi sp, sp, -{tf_size}` ngay sau `csrrw sp, sscratch, sp`; frame 288 B (chia hết 16) nằm trọn
  trong `.trap_stack`; `sp` vẫn khôi phục **cuối cùng** bằng `ld sp, {x2}(sp)`. Log TAP có
  `frame=0x80218ee0` (trong `0x80215000..0x80219000`). Sửa comment `linker.ld` + docs §7: KT-46
  **phải map** `.stack`/`.trap_stack` (NOLOAD, không thuộc PT_LOAD) và arena bắt đầu từ
  `__kernel_end` sau vùng này.
- **P2a** — `csrw sstatus` trước khi khôi phục thanh ghi/`sp` có thể bật interrupt khi frame chưa
  thoát: **bỏ hẳn** khôi phục `sstatus`; `sret` tự lấy `SIE` từ `SPIE` + privilege từ `SPP`
  (chuẩn bị KT-47). `sstatus` vẫn được **lưu** trong trapframe để dump/kiểm.
- **P2b** — trap lồng: thêm phát hiện trong asm (đọc `sp` bị ngắt từ frame; nếu nằm trong
  `[__trap_stack_bottom, __trap_stack_top)` ⇒ `nested_trap_halt` in
  `PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)` + `DebugHalt(1)`, không
  resume/không treo). Thêm predicate `is_nested_trap_sp` (plat-common) + host test biên; **TAP 5**
  `ok 5 - trap nested panic` trên QEMU ([`kt45-nested-trap.log`](kt45-nested-trap.log)); runner chỉ
  chấp nhận **đúng** dòng PANIC đó (không nới cổng PANIC chung).
- **P3** — `probe_ecall_sbi` thêm `lateout("a1") _` (SBI clobber a0/a1).
- **P4** — zero `.bss` tường minh ở `_start` (`__bss_start..__bss_end`, chỉ t0/t1), không dựa vào
  "QEMU tình cờ zero RAM"; `TRAP_STATS` được đảm bảo 0 trước dùng.

Sau vá: host **13/13**; `ktest.sh` **5 TAP** PASS; `run.sh 50 1` **50/50**; budget 80 B; gate
**569/73**; fatal path chạy lại ([`kt45-fatal-trap.log`](kt45-fatal-trap.log)) — frame/halt sạch.

## 3.2 Nit reviewer-kt45b (confirm) — cùng ngày

`reviewer-kt45b` **CONFIRM** (tự verify QEMU `-d in_asm`: `csrrw` → `addi sp,-288` → 8 lệnh lưu;
`frame=0x80218ee0` ∈ `[0x80215000, 0x80219000)`; `ld sp,16(sp)` → `sret`; 13/13 host; 5 TAP +
đúng 1 PANIC; 50/50 boot; gate 569; spec rỗng; unsafe chỉ plat-riscv64). Vá 4 nit trước commit:

1. Comment `nested_trap_halt` + docs §7: nói **đúng** bố cục — `sscratch` reset về top trước khi
   phát hiện nên frame lồng ở **đúng slot** `top-288` và **đè** frame ngoài; vô hại vì halt ngay.
2. Thêm [`kt45-serial.log`](kt45-serial.log) — copy **nguyên dòng serial** của QEMU (`TAP version`
   → hết), chứ không chỉ output runner; chứa `frame=0x80218ee0` + PANIC nested.
3. `ktest.sh` + `ktest.rs` header: **5 TAP** (không còn "4 TAP").
4. Docs §7 P2a: kiểm `grep 'csrw sstatus'` trong `trap_entry` = **0**; lần còn lại ở
   `vifast-plat-riscv64` là restore của **probe tiêm**, không phải đường trap thật.

## 4. Kết quả đo (do script in)

- **Host test** `cargo test -p vifast-plat-common`: **13 passed / 0 failed** (12 cũ + predicate
  trap lồng).
- **Ảnh test** [`kt45-ktest.log`](kt45-ktest.log) + serial [`kt45-serial.log`](kt45-serial.log):
  `ok 1 - ktest image + trap + nested`; serial QEMU có đủ:
  - `# trap Breakpoint scause=0x3 … frame=0x80218ee0` + `ok 2 - trap breakpoint`;
  - `# ecall sbi: a0=0xfffffffffffffffe … stvec-traps+0` + `ok 3 - trap ecall sbi`;
  - `# trap IllegalInstruction scause=0x2 …` + `# illegal inject (KHÔNG phải trap phần cứng —
    MEDELEG bit 2 = 0, D2-73)` + `ok 4 - trap illegal inject`;
  - `ok 5 - trap nested panic` + `PANIC: nested trap (KT-45: trap trong handler = fatal, khong
    resume)` — boot dừng **có chủ đích**, runner chấp nhận đúng dòng này, không có PANIC khác.
- **Ảnh sản phẩm** [`kt45-boot.log`](kt45-boot.log): `ok 1 - boot 1`, `stvec=0x80200020
  direct=true`, strings `ktest=0`.
- **50 boot** (sau vá) [`kt45-boot50.log`](kt45-boot50.log): `# 50/50 boot sạch (smp=1)`,
  `run.sh: PASS`, 0 `not ok`.
- **Budget** [`kt45-budget.log`](kt45-budget.log): `data+bss=80 B`/262.144 cho **cả hai** ảnh;
  `.stack=65536`, `.stack_guard=4096`, `.trap_stack=16384` báo riêng (NOLOAD).
- **Fatal trap trên QEMU** (tạm tiêm `scause=13`, không commit; [`kt45-fatal-trap.log`](kt45-fatal-trap.log)):
  `PANIC: trap LoadPageFault scause=0xd sepc=… stval=0x100000 sstatus=…` + `# regs ra/sp/gp/tp` +
  `# regs a0..a7`, rồi halt — chứng minh dump + `DebugHalt` sạch (không treo, không quay lại).
- **Cổng** [`kt45-gate-fast.log`](kt45-gate-fast.log): `./v2/bin/v2 gate --fast` PASS **569 test**
  (không hồi quy sim); kernel không nằm trong cổng (`v2/Cargo.toml` `exclude = ["kernel"]`).

## 5. Giới hạn / bàn giao

- **Không** có trap thật cho `illegal`/`ecall-S` với firmware D2-72; đã ghi D2-73. TAP illegal là
  **tiêm**, không được trích dẫn như trap phần cứng.
- Trap lồng **đã** bị chặn: fatal (PANIC + halt), không resume; **không** hỗ trợ unwind/resume
  lồng. Trap stack hiện **một** cho hart 0; `PerHart` là KT-49. `.stack`/`.trap_stack` là NOLOAD,
  không thuộc PT_LOAD — KT-46 phải map cả hai trước khi bật `satp`.
- Page fault thật (`scause` 13/15) + guard page → **KT-46**. Harness TAP đầy đủ (`expected.txt`,
  smp 1/2/4) → **KT-54**.
- Review độc lập: chờ verdict trước khi commit (quy trình lead).
