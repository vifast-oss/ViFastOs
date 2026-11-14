# KT-46 — Sv39 thật + guard page-fault (K3)

- **Vào**: READY; HEAD `669e2cbc` ([KT-00] lật KT-45 DONE, mở KT-46); `HEAD..origin/main = 0`;
  working tree có `viscript/` + `review/` của tab khác — **không chạm**, chỉ stage `v2/`.
- **Nguồn**: `review/agent0/K3-PLAN.md` §4.1 KT-46; **D2-72** (Q5/Q7), **D2-73**; `v2/docs/k3-riscv64.md`
  §2/§3/§7; KT-45 (trap frame, trap stack) + KT-44 (arena/linker/budget).
- **Ngân sách**: ≤ 3 h; task chỉ `v2/` (kernel + test + docs + STATE/JOURNAL); không timer/PLIC/SMP/
  IPI/ASID/U-mode/W^X (task sau); `unsafe` chỉ trong `kernel/vifast-plat-*` (R5/D2-17); không
  `cfg(target_arch)`; không dependency ngoài (dùng path `v2/lib/vifast-pte`).

## 0. Baseline

`./v2/bin/v2 gate --fast` — **PASS 569 test/73 suite, 0 failed** tại HEAD `669e2cbc`
([`kt46-baseline-gate-fast.log`](kt46-baseline-gate-fast.log)).
Kernel **không** nằm trong `gate --fast` (`v2/Cargo.toml` `exclude = ["kernel"]`); kiểm bằng
build riscv64 + `ktest.sh`/`run.sh` + `cargo test -p vifast-plat-common`.

## 1. Đỏ trước

`ktest.sh` được siết đòi `1..8` + `ok 5 - mmu on` + `ok 6 - map layout` + `ok 7 - walk khop` +
bằng chứng `# mmu:`; ảnh KT-45 hiện tại chỉ có `1..5` và không có MMU:

```
not ok 1 - ktest boot 1: thiếu plan-1..8 ok5-mmu-on ok6-map-layout ok7-walk-khop ok8-nested mmu-evidence
# 0/1 boot ảnh test sạch (smp=1)
EXIT=1
```

Log: [`kt46-red-ktest.log`](kt46-red-ktest.log) — đỏ **đúng lý do** (thiếu TAP MMU, không phải
lỗi hạ tầng).

## 2. Thiết kế đã làm

### 2.1 Bảng + walk (logic thuần, host-testable)

- `vifast-plat-common/src/mmu.rs` (mới, `#![forbid(unsafe_code)]`): bit PTE Sv39
  (`V/R/W/X/U/G/A/D`, `A|D` đặt sẵn), `PteFlags`, `MapError`, `Walk`, `PteMemory`, `is_canonical`,
  `walk_with`, `PageTableBuilder` (arena `&mut [u8]`, bump 4 KiB/bảng, map 4 KiB/2 MiB/1 GiB, từ
  chối unaligned/trùng/không canonical/cờ sai/hết arena), `satp_sv39`/`satp_mode`/`satp_root`.
- Hình học dùng lại `vifast-pte`: `VA_BITS = 39`, `PTE_LEVELS = 3`, `PAGE_SIZE_BITS = [12,21,30]`,
  `walk_shape`, `Pte` — thêm path-dependency nội bộ `vifast-pte` trong `vifast-plat-common`
  (`v2/kernel/Cargo.lock` +3 dòng, **không** crate ngoài).
- Guard: `trap.rs` thêm `GuardStats`/`GUARD_STATS` (faults/load/store/recovered/fatal/last_*) và
  `report_guard_fault` in `PANIC: guard page fault scause=… stval=… sepc=…` + regs + `DebugHalt(1)`.
  `dump_regs` tách dùng chung cho `report_fatal`/`report_guard_fault` (output fatal KT-45 giữ nguyên).

### 2.2 Arena + bảng thật trên QEMU

- `vifast-plat-riscv64/src/mmu.rs` (mới): đọc symbol linker; **arena 64 KiB** từ `__kernel_end`
  (sau `.trap_stack`), root + trung gian cấp từ arena (không `static`); map:
  - kernel **image** `0x8020_0000..0x8040_0000` bằng **leaf 2 MiB** RWX ở identity **và** nửa cao
    `0xFFFF_FFC0_4000_0000`;
  - vùng NOLOAD `0x8040_0000..arena_end` (guard 4 KiB + `.stack` 64 KiB + `.trap_stack` 16 KiB +
    arena 64 KiB) bằng leaf 4 KiB: **guard không map** (NONE), stack/trap/arena RW NX; nửa cao
    tương ứng;
  - **physmap** `0xFFFF_FFC0_0000_0000 + PA` cho RAM `0x8000_0000..0x8800_0000`, **2 MiB NX**;
    chunk 2 MiB chứa guard tách 4 KiB và **khoét** guard (không cửa sau qua physmap);
  - **MMIO** UART `0x1000_0000` + PLIC `0x0c00_0000`, **4 KiB Device** RW NX.
- **Linker** `ALIGN(2M)` trước `.stack_guard`: guard ở `0x8040_0000`, image 2 MiB không phủ guard
  (đổi từ layout KT-45: guard/stack/trap dịch sang 2 MiB kế tiếp, NOLOAD nên ảnh không đổi file).
- **Bật `satp` hai bước**: dựng xong toàn bộ bảng → lấy mẫu PTE 8 VA → `csrw satp` (mode 8) +
  `sfence.vma` trong một asm block → đọc lại `satp` → đọc lại PTE sau khi bật (phải bằng trước) →
  alias self-test ghi/đọc qua identity/nửa cao/physmap. Chỉ panic nếu bảng/`satp` sai (không bật MMU
  với bảng dở).
- **Guard handler**: `trap_dispatch` phát hiện page fault (12/13/15) có `stval` trong guard; mặc định
  fatal `report_guard_fault`; **probe test** `probe_guard_load` bật cờ một lần cho load fault
  (`scause=13`) để resume đúng 1 instruction 4 B; `probe_guard_store` (`scause=15`) luôn fatal.

### 2.3 Hai ảnh test (vì sao)

Mỗi boot chỉ kết thúc được **một** lần bằng PANIC có chủ đích. Ảnh `vifast-kbin-ktest` giữ trap
lồng KT-45 làm TAP cuối; guard store fault (`scause=15`) cần ảnh riêng
`vifast-kbin-ktest-guard` (cùng feature `ktest`, cùng linker, cùng trần budget) để dump + halt sạch.
Ảnh guard kiểm thêm load fault `scause=13` trước đó bằng probe resume **chỉ trong test**; chính sách
mặc định vẫn fatal.

## 3. Bằng chứng (số đo do script/cargo in)

| Hạng mục | Kết quả | Log |
|---|---|---|
| Host test logic | **21 test PASS** (13 trap KT-45 + 8 MMU KT-46), 1 ignored doc-test | [`kt46-host-tests.log`](kt46-host-tests.log) |
| Build 3 ảnh | product 53.176 B; ktest 61.336 B; ktest-guard 50.552 B | [`kt46-build.log`](kt46-build.log) |
| Budget `.data+.bss` | **168 B / 168 B / 176 B** ≤ 262.144 B (cả 3 ảnh); `.stack=65.536`, `.stack_guard=4.096`, `.trap_stack=16.384` báo riêng | [`kt46-budget.log`](kt46-budget.log) |
| TAP ảnh chính | **8/8**: image, breakpoint thật, ecall-SBI, illegal tiêm, **mmu on**, **map layout**, **walk khop**, trap lồng fatal | [`kt46-ktest.log`](kt46-ktest.log), [`kt46-ktest-main-raw.log`](kt46-ktest-main-raw.log) |
| TAP ảnh guard | **3/3**: guard image, **load fault `scause=0xd`** (recover, `stval` trong guard), **store fault `scause=0xf`** fatal (`PANIC: guard page fault …` + regs + halt) | [`kt46-guard-ktest-raw.log`](kt46-guard-ktest-raw.log) |
| Product 50 boot | **50/50 sạch** (`# 50/50 boot sạch (smp=1)`) | [`kt46-boot50.log`](kt46-boot50.log), [`kt46-boot.log`](kt46-boot.log) |
| `satp`/map runtime | `satp=0x8000000000080415 mode=8`, root/arena `0x80415000`, guard `0x80400000..0x80401000` (`guard_none=1`), `kernel_high_2m=1`, `physmap_2m_nx=1`, `uart_dev=1`, `plic_dev=1`, alias=1 | raw logs trên |
| Gate --fast | **PASS 569 test/73 suite, 0 failed** | [`kt46-gate-fast.log`](kt46-gate-fast.log) |

Ghi chú bằng chứng:
- Load guard: `# guard fault load scause=0xd stval=0x80400000 sepc=… len=4 recovered=1`.
- Store guard: `PANIC: guard page fault scause=0xf stval=0x80400008 sepc=… sstatus=…` + `# regs …`
  rồi halt — **page fault thật**, không phải trap tiêm.
- `walk khop: pre=post=1 software=8/8` — PTE đọc lại sau `satp` bằng trước, walk phần mềm trả đúng
  PTE, alias hardware ghi/đọc khớp.

## 4. Deviation / quyết định đã ghi

1. **Hai ảnh test** (thay vì một) chỉ vì mỗi boot chỉ có một PANIC kết thúc có chủ đích; D2-08 vẫn
   giữ (ảnh test không nằm trong ảnh sản phẩm, tách bằng feature).
2. **Load guard fault được resume một lần** trong ảnh guard bằng cờ probe; chính sách kernel mặc
   định là fatal (store fault chứng minh). Không hạ chuẩn guard: entry vẫn NONE ở cả 3 alias.
3. **Kernel chạy identity**, nửa cao là alias (do linker/entry của K3 link thấp). Yêu cầu "map
   kernel nửa cao" đạt (leaf 2 MiB, đọc/ghi qua alias trong TAP); chưa chạy PC từ nửa cao — việc
   relocate/entry nửa cao không thuộc KT-46.
4. **Linker `ALIGN(2M)`** dịch vùng NOLOAD sang `0x8040_0000` để image map được leaf 2 MiB mà guard
   vẫn NONE 4 KiB. Ảnh NOLOAD nên file ảnh không đổi; budget không đổi (báo riêng).
5. **W^X đầy đủ** chưa làm (image RWX) — KT-56; KT-46 chỉ bắt buộc physmap/stack NX.

## 5. Không làm (đúng phạm vi)

Timer (KT-47), PLIC IRQ + mapping PLIC rộng hơn 4 KiB (KT-48), SMP/PerHart (KT-49), IPI (KT-50),
ASID/shootdown/mutation bỏ `sfence.vma` (KT-51), object layer/Retype (KT-52), U-mode/3 syscall
(KT-53), harness `expected.txt` + matrix (KT-54), conformance `.scn` (KT-55), `FaultKind`/W^X/spec
(KT-56), soak 1.000/K3-DOD (KT-57), port core (K3b). Không sửa `spec/`, sim, `.scn`, QUEUE,
DECISIONS, AGENTS, `viscript/`.

## 6. Review độc lập

Theo quy trình: **chưa commit trước verdict**. `reviewer-kt46` (độc lập, read-only) tự chạy host
**21/21**, `ktest.sh` **8+3 TAP**, `satp=0x8000000000080415 mode=8 root=0x80415000 tables=10`,
product **50/50**, gate **569**; grep không còn vùng dùng mà chưa map; disasm guard fault đúng
`ld`/`sd` tại `0x80400000`/`0x80400008`; arena 10 bảng/40 KiB đủ.

- **Verdict: APPROVE** (không DRIFT logic map/guard/arena/`satp`).
- **4 nit đã vá trước commit**:
  1. `ktest.sh` không ghim `len=4` nữa — nhận `len=2|4` (assembler có thể nén `c.ld`).
  2. Sửa câu ELF: `.stack`/`.trap_stack` là NOLOAD ⇒ **không nằm trong `filesz`** của LOAD (LOAD
     `memsz` vẫn phủ tới `__kernel_end`), không phải "không thuộc PT_LOAD"; sửa ở
     `v2/docs/k3-riscv64.md` §3/§8 + comment `linker.ld`.
  3. `probe_guard_load`/`probe_guard_store`/`guard_recovery_armed` + nhánh resume + chuỗi
     `# guard fault load …` đặt sau `#[cfg(feature = "ktest")]`; feature `ktest` của kbin bật
     feature arch. Ảnh sản phẩm: **0** chuỗi `ktest`, **0** chuỗi `guard fault load`; chính sách
     guard của product luôn fatal.
  4. Sửa comment lỗi thời `lib.rs` ("K3 chưa bật MMU") + cập nhật HEAD/STATE.
- **Sau vá**: host **21/21**, `ktest.sh` PASS (8+3), product **50/50**, budget **168/176/168 B**,
  gate **569/73 PASS** ([`kt46-gate-fast.log`](kt46-gate-fast.log)).
