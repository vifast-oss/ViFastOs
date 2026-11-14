# K3-PLAN — Kernel thật riscv64: platform, trap, MMU, SMP, init, ktest

- **Trạng thái:** KẾ HOẠCH, chờ Agent 0 chốt **Q1..Q13** (§3). Chưa task nào được mở.
- **Mốc:** K3 trong thiết kế §10.1 (dòng 877): `plat-common + plat-riscv64; init tối thiểu;
  ktest + conformance`; cổng `TAP 100% smp 1/2/4 [TK]`, `soak boot 0 lỗi / N = 1.000` trên máy
  `ps` `[TK]` (D2-24), `perf đầu tiên`.
- **Người viết:** planner K3 (subagent); không sửa code/QUEUE/DECISIONS/AGENTS/spec, không đụng
  `viscript/`. Tệp duy nhất tạo: `review/agent0/K3-PLAN.md`.
- **Nhãn** (QD §2.2): `[TK]` hằng số/quyết định kỹ thuật; `[GT]` giả thuyết hoặc dữ kiện môi
  trường chưa xác nhận trên máy `ps`; `[ƯL]` ước lượng cần đo. Số đo thật chỉ do
  `./v2/bin/v2 nightly` / script QEMU sinh (QD §9 Q12).

## 0. Nguồn đã đọc (đúng giới hạn, không đọc thêm)

| Nguồn | Vùng đọc |
|---|---|
| `docs/v2/ViFastOs-v2-thiet-ke.md` | 512–551 (§5, §5.1–§5.4); 353–375 (§4.10); 862–885 (§10.1, dòng 876–877); grep `K3\|riscv64\|OpenSBI\|PLIC\|ktest\|conformance`; 484–485 (IrqControl/IrqHandler); 726–742 (L3); 770–790 (§9.3) |
| `docs/QUYET-DINH-sau-review.md` | grep `K3\|riscv64\|QEMU\|soak\|conformance\|TAP`; dòng 25 (Q-C), 28 (Q-F), 63 (D-06), 187, 195–201, 316 (D2-24) |
| `v2/agent/DECISIONS.md` | chỉ D2-08, D2-09, D2-10, D2-24, D2-53, D2-56..D2-60, D2-64, D2-65, D2-68, D2-71 |
| `v2/agent/reports/K2-DOD.md` | §7, §8, §9 |
| Code hiện có | `ls v2/`, `ls v2/kernel`, `cat v2/kernel/Cargo.toml`, `cat v2/Cargo.toml`, `ls v2/bin`, `grep 'riscv' v2/KERNEL*` (rỗng), `wc -l` + đọc `plat-riscv64/src/lib.rs` (71 dòng), `kbin/src/main.rs` (27 dòng), `v2/test/boot/run.sh` (105 dòng), grep `boot\|gate` trong `v2/bin/v2` |
| Môi trường (không phải repo) | `qemu-system-riscv64 --version`; banner OpenSBI; DTB `virt` bằng `dtc`; `ls review/agent0` (chỉ kiểm path) |

**Không đọc:** toàn bộ `QUEUE.md` (chỉ grep KT-07), `spec/abi.toml`, `v2/docs/k*.md`, `v2/sim/`,
`v2/lib/` ngoài phần đầu 2 crate, `docs/v1/`, `viscript/`.

Dữ kiện **có** trong nguồn: K0.5 DONE (KT-07, 50 boot 0 lỗi); K2 DONE phase-level (D2-71);
`spec/` chỉ có `abi.toml`; `v2/kernel` chỉ có `vifast-kbin-riscv64` (27 dòng) + `vifast-plat-riscv64`
(71 dòng); **chưa có** `vifast-plat-common`, core `no_std`, `vifast-scenario`, `vifast-ktest`,
`vifast-conformance`, `vifast-dtb`; `v2/sim/vifast-sim` 16.742 dòng (std/alloc) + `vifast-model`
1.441 dòng (std) là **toàn bộ ngữ nghĩa kernel hiện có**, chưa port `no_std`.

Dữ kiện **không** có (ghi `[GT]`): QEMU/OpenSBI trên **máy `ps`**; wall soak; nội dung
`spec/kernel.toml` tương lai; `vifast-dtb` có ở K3 không; hành vi `ps` dưới 3 QEMU; Miri trên
crate `no_std`.

## 1. Phạm vi K3 đo được

### 1.1 Làm trong K3

1. **`vifast-plat-common`**: vòng trap chung (trap → `Event` → `handle` → `Decision`), khung
   `Platform` (console/timer/IPI/IRQ claim-complete), drain hộp thư khi chờ khoá, watchdog, dump
   trace khi panic; `#![no_std]`, `unsafe` chỉ trong crate đóng D2-17 nếu buộc.
2. **`vifast-plat-riscv64`**: `_start` BSP, `stvec`, trapframe, `satp`/`sfence.vma`, `sret`,
   `mhartid`/`time`, SBI TIME/IPI/HSM/SRST, UART 16550, PLIC.
3. **Boot OpenSBI nạp trực tiếp** (`-bios fw_dynamic`): không UEFI, không U-Boot (Q3); console
   `DebugPutStr`/`DebugHalt` dùng được trước và sau MMU.
4. **Trap/exception thật**: ecall (U/S), breakpoint, illegal, page/access fault; trap từ U-mode
   không sập kernel; panic trong trap không treo im lặng.
5. **Timer thật**: SBI `set_timer` + `sie.STIE`, tick 100 Hz `[TK]` (Q6), `time` monotonic,
   deadline theo hart, phát hiện trễ.
6. **PLIC thật**: priority/enable/claim/complete, context S-mode `2*hart+1` (đo được), UART0 IRQ
   10; IRQ → `Event` → `handle`.
7. **Paging thật Sv39**: PTE 64 bit trong RAM, walk phần cứng, `satp` + `sfence.vma`, kernel page
   table **nửa cao** + **physmap** trang lớn NX, copy entry nửa cao vào root user (design §4.10),
   guard page dưới stack BSP.
8. **SMP thật**: BSP + AP qua SBI HSM; stack/arena/PerHart/hộp thư theo hart; `HartMask`; boot
   barrier; `MAX_HARTS = 8` `[TK]`.
9. **IPI thật** cho mailbox + shootdown: SBI IPI, drain trong trap, `TlbFlush` thật; hart gọi
   flush cục bộ + ack ngầm (giữ D2-68), chờ ack có timeout.
10. **`init` tối thiểu**: ELF user nạp bằng `vifast-elf`, U-mode, đúng 3 syscall `DebugPutStr`,
    `DebugHalt`, `DebugGetInfo`; fault của init được báo và halt sạch.
11. **`ktest` + conformance QEMU**: ảnh test riêng (D2-08) xuất TAP; 5 `.scn` nền tảng chạy **cả
    sim và QEMU**, so khớp sau chuẩn hoá.
12. **Đo**: `.data+.bss` từng ảnh, boot time, latency trap/IPI/shootdown `[ĐO]`; soak 1.000 boot
    trên `ps` (D2-24).

### 1.2 KHÔNG làm trong K3 (chống trượt phạm vi)

- **Không** port core (CNode/MDB/TCB/Endpoint/Notification/Reply/scheduler/MCS/IPC) sang `no_std`:
  ngữ nghĩa đó vẫn ở sim/model (Q13). K3 **không** hứa ABI đầy đủ trên QEMU.
- **Không** pager/COW/cấp trang khi fault (K2-DOD §8.4; design §4.10 xếp sau).
- **Không** `Busy` thật (cần IPC thật — Q13); **không** thêm `Busy` vào `spec/`.
- **Không** Retype **qua cap/syscall** (chỉ Retype nội bộ lúc boot cho VSpace/PageTable/Frame).
- **Không** `IrqControl.GetLine`/`IrqHandler` đầy đủ (chỉ PLIC nội bộ + 1 line UART); MSI/AIA hoãn.
- **Không** UEFI/aarch64/x86_64 (D2-10, QD Q-F(1)); **không** Sstc (SBI TIME trước).
- **Không** đổi `gate --fast` để nhét QEMU: L0/L1 vẫn host, QEMU chạy script riêng, `--fast` < 60 s.
- **Không** nới 256 KiB, không giảm seed L1, không `#[ignore]`, không retry lấy xanh.
- **Không** đụng `viscript/`, `docs/v1/`.

### 1.3 “K3 mới là kernel thật” — nghĩa chính xác

- K2 **model-only**: “TLB”, “IPI”, “ack” là bản ghi/bộ đếm trong **một** luồng CPU (K2-DOD §8.1;
  D2-65 Q10: “K2 không phải kernel thật; K3 mới QEMU”). K3 thay bằng phần cứng: MMU/TLB, IPI,
  timer, PLIC, SMP thật.
- Nhưng **“kernel thật” ở K3 = platform kernel thật**, chưa phải **toàn bộ ngữ nghĩa**: core IPC/
  TCB/CNode vẫn model-only tới khi port (Q13). Hệ quả:
  - Cổng K3 **không thể** đạt bằng sim: mọi tiêu chí TAP/soak/conformance phải có log QEMU.
  - Cổng K3 **không thay** cổng sim: L1 100.000 seed × smp 1/2/4/8 vẫn giữ (§5.3).
  - “K2 model-only không thay thế K3”: K3 có thể **đỏ** dù K2 xanh; K2 xanh không là bằng chứng K3.
- K3.5 (`procsrv + shell`) **không thể** bắt đầu chỉ bằng K3 — cần task port core đầu tiên
  (KT-58, ngoài K3). Nếu Agent 0 muốn K3 gồm core port thì ngân sách K3 phải lập lại (Q13).

### 1.4 Số liệu nền `[GT: đo bằng QEMU cục bộ, phải xác nhận trên ps]`

| Hạng mục | Giá trị đo | Ghi chú |
|---|---|---|
| QEMU / OpenSBI | `11.1.3` / `v1.8.1` (fw_dynamic đi kèm) | banner + `--version` |
| `timebase-frequency` | `10.000.000` (10 MHz) | DTB `virt`, `-smp 4` |
| PLIC | `0x0c00_0000`, cửa sổ `0x60_0000`, `ndev = 95` | không có APLIC/IMSIC mặc định |
| Context PLIC S-mode | `2*hart + 1` (hart0→1, hart1→3, …) | `interrupts-extended` + phandle intc `cpu@0` = 0x08 |
| UART0 / CLINT / test | `0x1000_0000` IRQ 10 / `0x0200_0000` / `0x0010_0000` | 16550A; `sifive,clint0`; `test@100000` |
| MMU trong DTB | `riscv,sv57` | kernel **vẫn chọn Sv39** (D2-65 Q2) |
| AIA mặc định | không node `aplic`/`imsic` | `virt` mặc định = PLIC cổ điển (Q8) |

`.data + .bss` của K0.5 đã có cổng 256 KiB trong `v2/test/boot/run.sh` (đo bằng `llvm-size -A`,
cộng `.data .sdata .bss .sbss`, `.stack` báo riêng vì NOLOAD). K3 **tái dùng đúng cách đo**, không
viết lại, không đổi ngưỡng.

## 2. Ranh giới K2 → K3: model-only → thật

| Hạng mục | K2 (model-only) | K3 (thật) | Bằng chứng bắt buộc |
|---|---|---|---|
| TLB | 16 entry/hart trong `vifast-sim/src/tlb.rs`, xoá bằng lệnh model | TLB phần cứng; chỉ `sfence.vma` mới xoá | test “map đè không flush ⇒ sai bị bắt”; mutation **bỏ `sfence.vma`** làm test đỏ |
| PTE / walk | `PageTable` model + `walk_shape` của `vifast-pte` | PTE 64 bit trong RAM, walk bằng MMU; walk phần mềm chỉ để assert | TAP “walk phần cứng khớp phần mềm” 3 cấp; PTE lạ ⇒ fault thật |
| IPI | kind `TlbFlush`/`Wake` trong hộp thư model | SBI IPI tới hartmask; drain trong trap | 100 IPI 2 hart, đếm đủ, không mất |
| Shootdown | ack bằng bộ đếm, cùng luồng | chờ ack thật + timeout; hart gọi flush cục bộ + ack ngầm (D2-68) | 100 vòng unmap 2 hart `Ok`; mutation “bỏ chờ ack” bị bắt; hart gọi **ngoài** `active_on` |
| `FaultKind` | `NotMapped/Permission/Wx` model-only (`vm_touch`) | decode `scause` thật (12/13/15 page fault; 1/5/7 access fault; 2 illegal; 3 breakpoint) → cùng enum; `spec/abi.toml` mở **một lần** (D2-56a, Q1) | bảng cause→FaultKind; 3 ca fault thật khớp 3 biến thể; `git diff spec/` chỉ hunk duyệt |
| `Busy` | model-only (D2-64) | **vẫn model-only ở K3** (không IPC thật) | ghi nợ K3.5; **không** thêm mã vào spec |
| VSpace tạo | API model `VSpace::new` + objmem heap | **Retype nội bộ** từ Untyped boot arena (`vifast-objmem` `no_std`) cho VSpace/PageTable/Frame; Retype qua cap **vẫn model-only** | TAP tạo VSpace/PT/Frame từ Untyped, map user, walk; tổng byte không đổi |
| Nửa cao + physmap | ghi chú (K2-DOD §8.5) | kernel page table nửa cao + physmap 1G/2M NX; copy entry nửa cao sang root user | đọc/ghi qua physmap; user root có entry nửa cao; NX trên physmap |
| Pager/COW | không có | **hoãn K6**, không làm | ghi trong K3-DOD; fault không cấp trang |
| User mode | chưa có | `init` U-mode + 3 syscall | TAP init in marker + halt; trap U-mode không sập kernel |
| Core (CNode/TCB/IPC/sched) | `vifast-sim` 16.742 dòng std | **vẫn model-only**; K3 không port (Q13) | K3-DOD nêu rõ “K3 không phải kernel ABI đầy đủ” |

Ghi chú bắt buộc: (1) “K2 xong = K3 xong” là sai — checker K2 vẫn chạy sim nhưng K3 phải chứng
minh bằng phần cứng; (2) `FaultKind` là **chỗ duy nhất** `spec/abi.toml` được mở (nếu Q1 = Có),
`spec/kernel.toml` **không** mở; (3) Retype nội bộ ≠ Retype ABI: không thêm chữ ký/mã lỗi, nếu cần
thì dừng và hỏi Agent 0.

## 3. Q1..Q13 Agent 0 phải chốt

### Q1 — Có mở `spec/` ở K3 không?

- **Đề xuất:** **không mở `spec/kernel.toml`**; hằng số K3 để `v2/docs/k3-riscv64.md` (tiền lệ
  D2-53 Q3, D2-65 Q1). **Có mở `spec/abi.toml` đúng một lần** thêm `FaultKind`
  (`NotMapped/Permission/Wx/Unknown`) theo D2-56(a), Agent 2 soạn REVIEW, Agent 0 APPROVED
  (D2-22); **không** thêm `Busy`.
- **Căn cứ:** K3 chỉ cần `FaultKind` thật; hạn mức nội bộ chưa bị user ABI phụ thuộc; mở
  `kernel.toml` bây giờ tạo nghĩa vụ đóng băng sớm.
- **Chặn:** KT-46/49/51 dùng hằng số `v2/docs/k3-riscv64.md`; KT-56 chặn bởi hunk `abi.toml`.
  Nếu mở `kernel.toml`: thêm 1 task 3 h (spec + gen + gen-check) **trước KT-44**.

### Q2 — QEMU version và máy ảo nào?

- **Đề xuất:** QEMU **≥ 9.2** `[TK]`, hiện `11.1.3` `[GT]`; máy **`virt`** (không `virt-<version>`),
  `-cpu rv64`, `-m 128M`, `-smp 1|2|4`, `-bios <fw_dynamic>`, `-nographic`, `-no-reboot`,
  `-machine virt,aia=none,aclint=on`. Ghi `# qemu <version>` vào TAP/log; thiếu hoặc khác major
  ⇒ **FAIL**, không tự hạ.
- **Căn cứ:** K0.5 đã chốt `virt` + fw_dynamic; `-cpu rv64` tránh phụ thuộc CPU host; `aia=none`
  khoá PLIC cổ điển (Q8); sàn ≥ 9.2 `[TK]` để thuộc tính `aia`/`aclint` và PLIC 95 nguồn ổn định
  như bản đo 11.1.3.
- **Chặn:** KT-46, KT-48, KT-54, KT-57. `-cpu max` chỉ ở K5' (design §10.1), không dùng ở K3.

### Q3 — OpenSBI nạp trực tiếp hay U-Boot?

- **Đề xuất:** **OpenSBI fw_dynamic nạp trực tiếp** qua `-bios`, không U-Boot/EDK2; pin OpenSBI
  đi kèm QEMU (`v1.8.1` `[GT]`), log phiên bản; không tải fw khác.
- **Căn cứ:** D2-10 + QD Q-F(1) “tới K6 chỉ riscv64, OpenSBI nạp trực tiếp”; UEFI là đường sau K6.
- **Chặn:** KT-44, KT-49, KT-57. Nếu U-Boot: +task riêng + DTB do U-Boot truyền, ≥3 h, flake tăng.

### Q4 — Số hart mặc định; `MAX_HARTS`?

- **Đề xuất:** boot **tất cả hart trong DTB** (`-smp N`), trần `MAX_HARTS = 8` `[TK]`; sản phẩm
  mặc định `-smp 1`, matrix kiểm `1/2/4`; BSP bật AP bằng SBI HSM sau khi dựng arena.
- **Căn cứ:** design §5.2 arena theo hart; mô hình K1c dùng N ∈ {1,2,4,8} (D2-53 Q1); K3 gate
  chỉ đòi 1/2/4.
- **Chặn:** KT-49, KT-51, KT-57. Cố định 4 hart sẽ bỏ khả năng so sánh 1 hart.

### Q5 — `.data + .bss ≤ 256 KiB` đo thế nào; áp cho ảnh nào?

- **Đề xuất:** giữ cách K0.5: `llvm-size -A`, cộng `.data .sdata .bss .sbss`, trần **262.144
  byte**, `.stack` báo riêng (NOLOAD); áp cho **cả ảnh sản phẩm và ảnh ktest** (ktest cũng
  ≤ 256 KiB, chuỗi test trong `.rodata`); in `# image=… data+bss=… stack=…`.
- **Căn cứ:** QD §3.5 + KT-07 đã có cổng cứng; D2-24/QD §9 Q8 không cho nới.
- **Chặn:** KT-44, KT-49, KT-57. Nếu vượt: cắt mảng tĩnh sang arena/RAM boot; **không** tăng trần,
  không tách ngân sách riêng cho ktest.

### Q6 — Timer tick bao nhiêu; SBI TIME hay Sstc?

- **Đề xuất:** **SBI TIME** (`set_timer`) + `sie.STIE`, tick **10 ms (100 Hz)** `[TK]`, `time`
  10 MHz monotonic; mỗi hart một deadline; timer heap tối thiểu 1 phần tử/hart. Sstc hoãn.
- **Căn cứ:** DTB `timebase-frequency = 10 MHz`; SBI TIME có trên mọi OpenSBI; Sstc không đảm bảo
  trên `-cpu rv64`.
- **Chặn:** KT-47, KT-50/51 (timeout chờ ack), KT-57. 1 kHz làm tăng trap; Sstc cần +1 task.

### Q7 — Địa chỉ PLIC/UART/CLINT lấy từ đâu?

- **Đề xuất:** hardcode hằng số nền tảng QEMU `virt` trong `vifast-plat-riscv64`
  (`UART0=0x1000_0000` IRQ 10, `PLIC=0x0c00_0000`, `CLINT=0x0200_0000`, `test=0x0010_0000`), kèm
  **kiểm tra DTB magic** ở `a1` nếu OpenSBI truyền (chỉ log/cảnh báo, không parse đầy đủ); context
  S-mode `2*hart+1`, `ndev` đọc từ register, không giả định 95.
- **Căn cứ:** chưa có `vifast-dtb`; K3 không nên vừa viết DTB parser vừa bring-up; địa chỉ là dữ
  kiện nền tảng, không phải hằng số thiết kế (ghi chú sẵn trong `plat-riscv64/src/lib.rs`).
- **Chặn:** KT-46, KT-48, KT-49. Parse DTB đầy đủ ⇒ +crate +1 task 3 h.

### Q8 — AIA (APLIC/IMSIC) hay PLIC cổ điển?

- **Đề xuất:** **PLIC cổ điển**, khoá `aia=none`; AIA hoãn tới K4'/K5' hoặc khi có phần cứng;
  `IrqControl.GetLine` (nếu làm) map trực tiếp line PLIC.
- **Căn cứ:** DTB `virt` mặc định **không** có APLIC/IMSIC (đo được); D2-10 chỉ riscv64 tới K6;
  design §5.4 liệt kê cả hai nhưng K3 không cần.
- **Chặn:** KT-48, KT-56, KT-57. AIA cần IMSIC/hart + `aia=aplic-imsic`, +≥2 task.

### Q9 — `ktest` trong kernel hay user?

- **Đề xuất:** **ktest in-kernel trong ảnh test riêng** (`vifast-kbin-ktest`, feature `ktest`,
  không nằm trong ảnh sản phẩm — D2-08), TAP qua `DebugPutStr`; `init` U-mode chỉ chứng minh đường
  user + 3 syscall. `vifast-ktest` **user process** (design §9.3 dòng 788) **hoãn tới K3.5** khi
  core ABI được port (Q13), vì `Step::Call/Send/Recv` cần IPC thật.
- **Căn cứ:** K3 chưa có core `no_std`; kịch bản user cần syscall vượt quá 3 syscall K3; D2-08
  buộc tách ảnh.
- **Chặn:** KT-54, KT-55, KT-57. User-process ngay ở K3 ⇒ cần port core trước (+≥30 h, Q13).

### Q10 — Soak 1.000 boot trên máy `ps` chạy thế nào?

- **Đề xuất:** `v2/test/boot/soak.sh N=1000 LANES=3` trên **máy `ps`**, 3 luồng QEMU (D-06/D2-24),
  log riêng mỗi boot, timeout **20 s/boot** (chỉ kill PID của chính nó), BUILD_ID = git sha +
  sha256 ảnh, ghi qemu/OpenSBI version + cấu hình; pass = đủ `TAP version 13`, `1..K` đúng, mọi
  dòng `ok`, `# k3 halt`, không `PANIC`; **0 lỗi/N=1000**, không retry, không đếm artifact, 0/N chỉ
  là trần (QD §5.3). Không chạy song song với build/gate.
- **Căn cứ:** QD D-06 + D2-24 (p = 0,3 % ⇒ n ≥ 3/p = 1.000, 3 luồng); K0.5 có mẫu `run.sh` 50 boot.
- **Chặn:** KT-57. N nhỏ hơn 1.000 là **nới ngưỡng** (cấm); 1 luồng hợp lệ nhưng wall dài hơn.

### Q11 — Conformance `.scn` chạy cả sim và QEMU (design §9.3) tới đâu?

- **Đề xuất:** K3 chỉ chạy **tập nền tảng**: 5 `.scn` (map/unmap/walk, W^X, shootdown 2 hart, VM
  fault NotMapped, shared read-only) qua **scenario-lite backend** dùng chung IR
  `vifast-scenario`; backend sim = model hiện có, backend QEMU = ktest in-kernel; chuẩn hoá trace
  (bỏ timestamp/thứ tự hart nếu được phép), so **chuỗi event/kết quả**; 5/5 khớp. Kịch bản đầy đủ
  (`Call/Send/Recv/ReplyRecv/...`) hoãn tới K3.5.
- **Căn cứ:** design §9.3 (dòng 786–788) + §9.2 “Platform: conformance trap/syscall/fault/W^X”;
  D2-13 “`.scn` chạy cả sim và QEMU”; 26 `.scn` K2 đa số cần IPC.
- **Chặn:** KT-55, cổng K3 §5.2(4). Đòi toàn bộ `.scn` ở K3 ⇒ phụ thuộc Q13.

### Q12 — Miri còn áp cho phần `no_std` kernel không?

- **Đề xuất:** **không** chạy Miri trên ảnh/kernel `no_std` (asm/MMIO/S-mode không hỗ trợ); **giữ**
  Miri cho `lib/` + sim như hiện tại (D2-47/D2-65; K2: 146 test), **mở rộng** cho crate logic chạy
  host (`vifast-pte`, `vifast-elf`, phần logic `plat-common` tách khỏi asm); kernel kiểm bằng
  `cargo build --target riscv64gc-unknown-none-elf` + TAP QEMU. Ghi miễn trừ **có lý do** trong
  K3-DOD, không coi là nới ngưỡng.
- **Căn cứ:** Miri không hỗ trợ inline asm/S-mode; D2-17 giới hạn `unsafe`; K2-DOD §8.6 ghi nợ.
- **Chặn:** KT-44, KT-57. Đòi Miri trên kernel ⇒ tách crate + host shim, +2 task.

### Q13 (bổ sung, bắt buộc) — Port core kernel có thuộc K3 không?

- **Đề xuất:** **Không**. K3 chỉ platform kernel; core (CNode/MDB/TCB/Endpoint/Notification/Reply/
  scheduler/MCS/IPC) vẫn model-only; **K3.5 mở bằng task port core đầu tiên** (KT-58, ngoài tệp
  này) trước `procsrv`/shell. Cổng K3 **không** đo ngữ nghĩa IPC.
- **Căn cứ:** phạm vi K3 design §10.1 chỉ nêu plat-common/plat-riscv64/init/ktest; code hiện có:
  core 16.742 dòng std trong sim, 0 dòng `no_std`; 14 task × ≤3 h không đủ port.
- **Chặn:** Q9, Q11, KT-52/53/54/55/57 và cách viết K3-DOD. Nếu K3 gồm core port: +30–40 h,
  +8–10 task, cổng K3 thêm ABI `.scn`; phải tách K3a/K3b. **Không** được vừa giữ cổng hiện tại
  vừa hứa ABI đầy đủ.

## 4. Bảng task KT-44..KT-57 (14 task, mỗi task ≤ 3 h)

| Task | Nội dung | Phụ thuộc | Giờ | DoD đo được (tóm tắt) | Rủi ro chính |
|---|---|---|---|---|---|
| KT-44 | `plat-common` + tách ảnh sản phẩm/ktest + đo budget | KT-43, KT-07 | 2,5 | 2 ảnh build; ảnh sản phẩm không chuỗi ktest; budget in ra; TAP 1 boot | chuỗi test đội `.data` |
| KT-45 | Trap khung: `stvec`, trapframe, decode, panic/trace | KT-44 | 3 | TAP 3 exception (ecall/breakpoint/illegal); 50 boot 0 lỗi | sai offset trapframe |
| KT-46 | Sv39 thật + nửa cao + physmap + `satp` + guard page | KT-45 | 3 | MMU bật; walk khớp; guard fault bị bắt; 50 boot | treo không log |
| KT-47 | Timer SBI: tick 100 Hz, monotonic, deadline/hart | KT-45/46 | 2,5 | 100 tick đơn điệu, không mất; drift ≤ 1 % | sai đơn vị ms/tick |
| KT-48 | PLIC + UART IRQ: enable/claim/complete | KT-46 | 3 | IRQ UART ≥ 1; claim/complete đúng; 0 IRQ giả | claim sai context |
| KT-49 | SMP: SBI HSM, stack/arena/PerHart/hộp thư | KT-46/47 | 3 | hart 0..N-1 sống với N=1/2/4; barrier đúng | stack AP chồng |
| KT-50 | IPI thật + mailbox drain + kind `TlbFlush`/`VmKick` | KT-49 | 2,5 | 100 IPI 2 hart không mất; drain khi chờ khoá | self-target lặp |
| KT-51 | ASID thật + `sfence.vma` + shootdown ack + timeout | KT-46/50 | 3 | 100 vòng shootdown `Ok`; mutation bỏ flush bị bắt; hart ngoài `active_on` | deadlock chờ ack |
| KT-52 | Object layer tối thiểu `no_std` + Retype nội bộ | KT-46 | 3 | tạo VSpace/PT/Frame từ Untyped; map user; byte không đổi | objmem khác host |
| KT-53 | U-mode + 3 syscall + nạp `init` ELF | KT-46/52 | 3 | init in TAP; trap U-mode không sập; 50 boot | thiếu `sfence.vma` |
| KT-54 | ktest TAP + runner QEMU smp 1/2/4 + `expected.txt` | KT-45..53 | 2,5 | TAP 100 % ở 1/2/4; số test khớp | đếm test theo hart |
| KT-55 | Conformance 5 `.scn` × sim/QEMU, chuẩn hoá trace | KT-51/52/54 | 3 | 5/5 khớp; 0 khác biệt ngoài trường cho phép | trace khác định dạng |
| KT-56 | `FaultKind` thật + W^X + `spec/abi.toml` một lần | KT-46/51/53 | 3 | cause→FaultKind 3/3; W^X fault; spec diff 1 hunk | lẫn `Permission`/`NotMapped` |
| KT-57 | Cổng K3: soak 1.000, matrix, 100k L1, perf, K3-DOD | tất cả | 3 | soak 0/1000; TAP 100 % 1/2/4; 100k L1; budget; perf `[ĐO]` | flake/`ps` bận |

Tổng: **40 h** `[ƯL]` (14 × 2,5–3 h), chưa gồm Q13 nếu đổi phạm vi. KT-57 có wall soak 25–60
phút `[ƯL]`, không tính vào 3 h công.

**Thứ tự tối ưu:** làn A `44→45→46→{47∥48∥49}→50→51`; làn B `52→53` (sau 46, song song 47/48/49);
làn C `54` dựng runner sau 45, hoàn thiện sau 53; `55` sau 51/52/54; `56` sau 46/51/53; `57` cuối,
chạy một mình. Tối đa 3 làn (AGENTS §9: 3 QEMU song song), soak không song song build/gate.
Cổng mỗi task: `./v2/bin/v2 gate --fast` + smoke QEMU của task (50 boot hoặc TAP riêng); task
chạm `kernel/` phải xanh `--fast` (D2-05), lỗi logic phải có test đỏ trước.

### 4.1 Chi tiết từng task (việc / test đỏ trước / DoD + bằng chứng)

- **KT-44 (2,5 h):** tạo `v2/kernel/vifast-plat-common` (`no_std`, trait `Platform`), tách ảnh
  sản phẩm/ktest, linker `.stack` NOLOAD + guard, `v2/docs/k3-riscv64.md` (không đụng `spec/`).
  *Đỏ trước:* `ktest.sh` đòi `ok 1 - ktest image` (chưa có ảnh). *DoD:* 2 ảnh build; `strings` ảnh
  sản phẩm không có `ktest`; budget ≤ 262.144; `run.sh 1` xanh. *Chứng:* `kt44-{build,budget,boot}.log`.
- **KT-45 (3 h):** `stvec` direct, trapframe 32 thanh ghi + `sepc/sstatus/scause/stval`, decode,
  panic dump `scause/sepc`, `Event::Exception`. *Đỏ trước:* 3 TAP `trap ecall/breakpoint/illegal`
  (chưa `stvec` ⇒ reset). *DoD:* 3/3 TAP; 50 boot 0 lỗi; trap S-mode không treo. *Chứng:* `kt45-*.log`.
- **KT-46 (3 h):** root/kernel page table trong RAM từ arena, map kernel nửa cao (2M/1G), physmap
  NX, MMIO UART/PLIC 4K, bật `satp` Sv39 + `sfence.vma`, guard page, copy entry nửa cao. *Đỏ trước:*
  TAP `mmu on`, `physmap rw`, `guard page fault` (trước `satp` guard đỏ). *DoD:* walk phần cứng =
  phần mềm; guard đúng `scause`; 50 boot; budget. *Chứng:* `kt46-{ktest,boot50,budget}.log`.
- **KT-47 (2,5 h):** SBI `set_timer` (EID 0x54494D45), `sie.STIE`, `time` 10 MHz, tick 100 Hz,
  đếm tick/hart, phát hiện trễ. *Đỏ trước:* TAP `100 ticks monotonic`, `no lost tick` (tick=0).
  *DoD:* Δ tick ∈ [9,11] ms `[ĐO]`; 1.000 tick không mất. *Chứng:* `kt47-ktest.log` (có Δ).
- **KT-48 (3 h):** PLIC priority/enable/claim/complete, context `2*hart+1`, threshold 0; UART0 IRQ
  10, bật IER TX-empty; handler đếm, tắt sau N; `Event::Irq(line)`. *Đỏ trước:* TAP `uart irq >= 1`
  (chưa enable ⇒ 0). *DoD:* claim/complete cân bằng, context đúng hart 0..3, 50 boot. *Chứng:*
  `kt48-{ktest,boot50}.log`.
- **KT-49 (3 h):** arena/stack (guard) theo hart, PerHart, SBI HSM `hart_start` cho 1..N-1, boot
  barrier, `HartMask`, hart lỗi → log không panic. *Đỏ trước:* TAP `4 harts alive` (chỉ hart 0).
  *DoD:* N=1/2/4 đủ hart, mỗi hart in id; 50 boot mỗi N; budget. *Chứng:* `kt49-smp{1,2,4}.log`.
- **KT-50 (2,5 h):** SBI IPI (EID 0x735049) hartmask; trap ID mềm drain hộp thư; kind `VmKick` +
  `TlbFlush`; đếm sent/delivered; drain khi chờ khoá. *Đỏ trước:* TAP `100 ipi delivered` (0).
  *DoD:* 100/100 không mất/nhân đôi, 2 hart, 50 boot. *Chứng:* `kt50-ipi.log`.
- **KT-51 (3 h):** ASID 8 bit + generation (D2-65 Q4), `sfence.vma` (asid,va)/asid/all, `TlbFlush`
  thật qua IPI, local flush + ack ngầm (D2-68), timeout bằng timer, rollover flush toàn bộ. *Đỏ
  trước:* TAP `stale mapping caught`, `shootdown 100 rounds`; mutation bỏ `sfence.vma` đỏ. *DoD:*
  100 vòng 2 hart `Ok`; hart ngoài `active_on`; rollover; 50 boot. *Chứng:* `kt51-tlb.log`,
  `kt51-mutation-bỏ-sfence.log`.
- **KT-52 (3 h):** `vifast-objmem` (`no_std`) trên Untyped boot, body VSpace/PageTable/Frame
  (`vifast-pte`), Retype **nội bộ** (không ABI), check tổng byte/không rò, VSpace user đầu tiên.
  *Đỏ trước:* TAP `retype vspace`, `untyped byte invariant` (chưa có Retype). *DoD:* 1 VSpace +
  ≥3 PT + ≥1 Frame; byte trước/sau khớp; map/unmap 4 KiB; 0 rò. *Chứng:* `kt52-obj.log`,
  `git diff spec/` rỗng.
- **KT-53 (3 h):** nạp `init` ELF (`vifast-elf`), map segment W^X, trapframe U-mode, `sret`;
  ecall → 3 syscall 12/13/`DebugGetInfo`; fault U-mode log + halt; copy entry nửa cao. *Đỏ trước:*
  TAP `init u-mode`, `syscall debug_put_str` (chưa `sret`). *DoD:* init in marker + 3 syscall;
  U-mode không đọc được địa chỉ kernel; 50 boot. *Chứng:* `kt53-init.log`.
- **KT-54 (2,5 h):** harness TAP trong ảnh test (`1..K`, `ok/not ok`, chẩn đoán); `ktest.sh` chạy
  `-smp 1/2/4`, đòi TAP 100 %, so số test `expected.txt`, log riêng, timeout 20 s, kill đúng PID.
  *Đỏ trước:* runner đòi `# tests 14` (thiếu test). *DoD:* TAP 100 % ×3 cấu hình; log version.
  *Chứng:* `kt54-smp{1,2,4}.log`, `expected.txt`.
- **KT-55 (3 h):** IR `vifast-scenario` rút gọn (`Retype/Map/Unmap/Touch/ReadClock`), backend sim
  và QEMU (ktest in-kernel), trace cùng `vifast-traceformat`, chuẩn hoá (bỏ timestamp, sắp
  `(hart,step)`), 5 ca: map/unmap/walk, W^X, shootdown 2 hart, VM fault NotMapped, shared read-only.
  *Đỏ trước:* `kt55-diff.log` đòi 0 khác biệt (QEMU chưa có). *DoD:* 5/5 khớp; khác biệt chỉ ở
  trường cho phép; sim xanh + QEMU đỏ = lỗi platform (design §9.3). *Chứng:* `kt55-*`.
- **KT-56 (3 h):** bảng `scause`→`FaultKind` (`NotMapped` 13/15, `Permission` do U/S, `Wx` do PTE
  X+W, `Unknown` còn lại); fault user → log + halt (không pager); W^X ở PTE; Agent 2 REVIEW
  `spec/abi.toml` hunk `FaultKind`, Agent 0 APPROVED (D2-56a). *Đỏ trước:* 3 TAP `fault
  notmapped/permission/wx` (rơi `Unknown`). *DoD:* 3/3; W^X fault thật; spec diff 1 hunk; gen-check
  xanh. *Chứng:* `kt56-fault.log`, `kt56-spec.diff`.
- **KT-57 (3 h):** chạy đủ §5.2; `soak.sh 1000 3` trên `ps`; perf boot→halt, trap, IPI, shootdown
  `[ĐO]`; viết `v2/agent/reports/K3-DOD.md` (nêu Q13: K3 không phải ABI đầy đủ); cập nhật STATE.
  *Đỏ trước:* `soak.sh 3` đỏ khi thiếu marker. *DoD:* soak 0/1000, 3 luồng, BUILD_ID + sha256 +
  version; TAP 100 % 1/2/4; 100k L1 xanh; budget 2 ảnh; conformance 5/5; perf có số. *Chứng:*
  `kt57-soak.log`, `kt57-smp*.log`, `kt57-l1-100k.log`, `K3-DOD.md`.

## 5. Cổng K3 — không nới ngưỡng

### 5.1 Mỗi task (bắt buộc)

1. `./v2/bin/v2 gate --fast` xanh (fmt, clippy, luật cấu trúc, L0, gen-check, L1 1.000 seed).
2. Smoke QEMU của task: 50 boot (`run.sh 50`) hoặc `ktest.sh` TAP 100 %; log lưu
   `v2/agent/JOURNAL/` + `v2/agent/reports/`.
3. Test đỏ trước có log; không `#[ignore]`, không `#[should_panic]` để né, không retry xanh.
4. `git diff spec/` rỗng, trừ KT-56 (đúng hunk đã duyệt).

### 5.2 Cổng K3 (đủ 9 điều kiện mới DONE)

| # | Điều kiện | Ngưỡng (không nới) |
|---|---|---|
| 1 | `gate --fast` | xanh, 0 test đỏ, L1 1.000 seed |
| 2 | TAP ktest | **100 %** ở `smp 1/2/4`; số test khớp `expected.txt`, không thiếu/thừa |
| 3 | Soak boot | **0 lỗi / N = 1.000** trên **`ps`**, 3 luồng (D2-24, QD §5.3); 0/N chỉ là trần |
| 4 | Conformance | **5/5** `.scn` nền tảng khớp sim ↔ QEMU; khác biệt ngoài trường cho phép = 0 |
| 5 | `.data + .bss` | **≤ 262.144 byte** cho **cả** ảnh sản phẩm và ktest; `.stack` báo riêng |
| 6 | Sim L1 100k | `./v2/bin/v2 nightly` (100.000 seed × smp 1/2/4/8) **0 vi phạm** — giữ nguyên |
| 7 | Mutation | 4 mutation bị giết: bỏ `sfence.vma`; bỏ chờ ack; bỏ flush cục bộ; bỏ drain hộp thư |
| 8 | Perf đầu tiên | có `[ĐO]` boot→halt, trap, IPI, shootdown; **chưa đặt ngưỡng perf**, chỉ baseline |
| 9 | Tài liệu | `K3-DOD.md` ghi rõ K3 = platform kernel thật, **không** ABI đầy đủ (Q13); nợ `Busy`/pager/COW/core port |

- **Flake:** T-INFRA, ghi log, chạy lại **tối đa một lần** chỉ để phân loại; không bao giờ PASS
  (D2-07, QD §5.3).
- **QEMU:** tối đa 3 song song; chỉ kill PID do chính task tạo (AGENTS §9); không `pkill -f qemu`.
- **Thời gian:** `gate --fast` < 60 s; wall host > 60 s là T-INFRA (K2-DOD §8.6), không giảm
  invariant; L3/soak ngoài `--fast`, chạy script riêng.

### 5.3 Những thứ KHÔNG được làm để qua cổng

- Không tăng trần 256 KiB, không tách ngân sách cho ktest, không bỏ `.sdata/.sbss`.
- Không giảm N soak, không giảm seed L1, không đổi `p`, không đếm artifact.
- Không bỏ cấu hình smp nào, không “chạy 4 hart thay 1 hart”.
- Không dùng kết quả sim thay log QEMU; không dùng log K0.5 thay K3.
- Không sửa `.scn`/kỳ vọng/`expected.txt` để khớp; nếu kỳ vọng sai, sửa **sau** D2 và ghi lý do.

## 6. Rủi ro

### 6.1 Ba rủi ro lớn nhất

1. **Boot/QEMU flake + OpenSBI version drift** — soak 1.000 là tiêu chí cứng; QEMU/OpenSBI trên
   `ps` khác máy lập kế hoạch `[GT]`, một boot treo làm cổng đỏ. Giảm thiểu: pin QEMU ≥ 9.2 +
   fw_dynamic đi kèm, log version/sha256/BUILD_ID, timeout 20 s/boot, 3 luồng, T-INFRA có log;
   **không** tăng N hay retry xanh.
2. **Paging thật sai (map kernel/nửa cao/physmap)** — bật `satp` khi thiếu map ⇒ treo không log.
   Giảm thiểu: bật MMU 2 bước (identity → nửa cao), map UART sớm, guard page, walk đối chiếu,
   giữ đường halt SRST.
3. **IPI/shootdown deadlock thật** — chờ ack khi không drain hộp thư (lỗi AP→BSP v1) hoặc
   self-target. Giảm thiểu: giữ D2-68 (local flush + ack ngầm), drain trong vòng chờ, timeout
   timer, test hart gọi ngoài `active_on`, 4 mutation.

### 6.2 Bảng rủi ro đầy đủ

| # | Rủi ro | Mức | Giảm thiểu | Task |
|---|---|---|---|---|
| R1 | QEMU/OpenSBI khác trên `ps`; boot flake | Cao | pin version, log fingerprint, timeout, 3 luồng, T-INFRA | KT-57 |
| R2 | Paging thật sai → treo không log | Cao | bật MMU 2 bước, map sớm, guard, walk đối chiếu | KT-46 |
| R3 | IPI/shootdown deadlock | Cao | local flush + ack ngầm, drain, timeout, mutation | KT-50/51 |
| R4 | Vượt 256 KiB do arena/PerHart/PLIC table | Trung | arena từ RAM boot, mảng tĩnh tối thiểu, đo mỗi task | KT-44/49/57 |
| R5 | Thời gian soak `ps` (25–60 phút `[ƯL]`) | Trung | chạy một mình, 3 luồng, log theo lô; không mốc ngày | KT-57 |
| R6 | `no_std` + Miri không chạy được kernel | Trung | Miri cho lib/logic host; kernel kiểm build + QEMU; miễn trừ có lý do | KT-44/57 |
| R7 | Conformance lệch sim ↔ QEMU | Trung | chỉ tập nền tảng; chuẩn hoá trace; chốt format sớm | KT-55 |
| R8 | Q13 sai (K3 bị đòi ABI đầy đủ) | Cao | chốt Q13 trước KT-44; K3-DOD nêu rõ; K3.5 mở bằng port core | KT-57 |
| R9 | Retype nội bộ thành ABI (thêm mã/chữ ký) | Trung | `git diff spec/` rỗng; review D2-04/D2-22 | KT-52/56 |

## 7. Điều plan này KHÔNG làm, `[GT]` cần chốt, và tự kiểm

- **Không** sửa code/QUEUE/DECISIONS/AGENTS/spec, không mở task, không viết `.scn`, không chạy
  soak thật.
- **Không** cam kết ngày/giờ (QD §5.1): 40 h `[ƯL]` là công ước lượng, soak là wall-clock.
- **Không** thay Agent 0 quyết định: Q1..Q13 là đề xuất; Agent 0 chốt bằng D2 + APPROVED.

| `[GT]` | Cần chốt | Trước task |
|---|---|---|
| G1 | QEMU + OpenSBI trên `ps` đúng version/đường dẫn fw_dynamic | KT-44 |
| G2 | `ps` đủ RAM/CPU cho 3 QEMU song song; wall soak thực tế | KT-57 |
| G3 | `vifast-elf` nạp được ELF user 64-bit (không cần sửa crate) | KT-53 |
| G4 | `vifast-objmem` `no_std` chạy trên target riscv64 | KT-52 |
| G5 | Miri trên crate `no_std` không asm (nếu Q12 mở rộng) | KT-57 |
| G6 | Không có `spec/kernel.toml` ⇒ hằng số K3 ở `v2/docs/k3-riscv64.md` | KT-44 |
| G7 | `-cpu rv64` đủ extension K3 cần (Sv39, SBI TIME/IPI/HSM) | KT-46 |

**Tự kiểm:** path `review/agent0/K3-PLAN.md`; tệp duy nhất **plan này** tạo; **432 dòng**
(`wc -l`, trong khoảng 350–500). 14 task KT-44..KT-57; 13 câu Q1..Q13. Các thay đổi khác trong
`git status` (nhánh ViScript, snapshot, log) là **có sẵn từ trước**, plan này không chạm: đã kiểm
`git status --porcelain -- review/agent0/K3-PLAN.md` chỉ có đúng dòng `?? review/agent0/K3-PLAN.md`.
