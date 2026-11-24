# KẾ HOẠCH TỔNG THỂ CHU KỲ 3 (`v2.1` — `KT-350..KT-366`, 21 task ≤ 3 h) & ĐỊNH HƯỚNG CHU KỲ 4 (`v2.2`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng) — theo **D2-251, D2-252, D2-255, D2-256 & D2-257** (`2026-11-24`, đã được người điều phối chỉ đạo tối ưu toàn diện).
**Trạng thái**: **THIẾT KẾ TỐI ƯU ĐÃ CHỐT (`D2-257`), MỞ KHOÁ NGAY SAU `KT-348` / TAG `v2.0`** *(chưa nạp vào `v2/agent/QUEUE.md` để bảng hàng đợi Chu kỳ 2 khép sạch 38/38 task sau `KT-348a` + `KT-348b` → `KT-348`)*.
**Nguồn sự thật**: [`AGENTS.md`](../../AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](../../docs/QUYET-DINH-sau-review.md), [`v2/agent/DECISIONS.md`](../agent/DECISIONS.md) (**D2-240..D2-257**), [`KE-HOACH-CHU-KY-2-2026-11-22.md`](../../KE-HOACH-CHU-KY-2-2026-11-22.md), [`v2/docs/u1-uefi-readiness.md`](u1-uefi-readiness.md).
**Nhãn số liệu (QD §2.2 & §9 Q12)**: mọi hằng số thiết kế/ngân sách trong kế hoạch này mang nhãn `[TK]` (mặc định), giả thuyết `[GT]`, ước lượng công việc `[ƯL]`; tuyệt đối không chứa số đo hiệu năng viết tay.

> **Mục tiêu chiến lược một câu**: Nâng cấp ViFastOs từ nền tảng `v2.0` lên **`v2.1` với Kiến trúc Lõi Kép Đa hình (`Machine<S: KernelStore>`)**: vừa **bảo toàn 100 % chế độ tĩnh bất biến `StaticStore` (`ViFastOs-Mission`: `0 unsafe`, `0` con trỏ, `Copy`, `≤ 196.608 B` cho hệ thống an toàn cao / hàng không vũ trụ / kiểm thử snapshot)**, vừa **mở khoá chế độ động theo RAM thật `UntypedStore` (`ViFastOs-General`: đặt thân object + slot CNode 48 B trực tiếp vào `Untyped` qua `vifast-objmem` + `BootInfo` dynamic arena theo CPU/RAM lúc boot cho Desktop/Server/VMM)**; đồng thời hoàn tất **SMP preemptive 4 hart cả 3 arch**, **hợp nhất HAL/`vifast-plat-common`/`vifast-kbuild`**, **tinh gọn kho ~62 % dung lượng**, **ABI v1.2 (`syscall`/`sysret`, IRQ `Notification`, `Tcb.Exec`, `IOSpace`)**, **`v2 mutants` + trace 32 B + `vifast-bench`**, và chạy thật **`ViScript`/`VSH2` + `U1b`/UEFI + tiền trạm phần cứng thật `HW1-Pre`**.

---

## 0. Nâng cấp kiến trúc cốt lõi ở bản `D2-257` (So với bản cũ `D2-251..D2-255`)

Qua rà soát sâu mã nguồn thực tế tại `da75f5b` (`vifast-core`, `vifast-objmem`, `vifast-sim`, `spec/kernel.toml`, `check_kernel.py`, `u1-uefi-readiness.md`), bản kế hoạch mới khắc phục triệt để **4 điểm mù kiến trúc & vận hành**:

### 0.1 Từ "Thay thế cứng" sang **Kiến trúc Lõi Kép Đa hình (`Machine<S: KernelStore = StaticStore>`)** (`KT-355a` + `KT-355b`)
Bản cũ (`k3b-core.md` §28.5 & `KT-355` cũ) dự định *xoá* bảng tĩnh của `Machine` để thay cứng bằng `vifast-objmem`, nhưng vẫn giữ `objects: [Meta; 256]` và `caps: [Cap; 512]`. Cách cũ mắc **3 nhược điểm lớn**: (i) làm mất tính chất `#[derive(Clone, Copy)]` không con trỏ của `Machine` (mất cấu hình nhúng an toàn cao & snapshot tức thì); (ii) vẫn bị chặn cứng ở **object thứ 257 / cap thứ 513 / CNode > 192 slot** dù máy có 64 GiB RAM; (iii) đụng trực tiếp vào các hằng `MAX_*` đang bị `spec/kernel.toml` ghim cứng trước khi mở `spec/`.

**Giải pháp tối ưu (`D2-257`)** — tách thành 2 bước sạch (mỗi bước ≤ 3 h), **100 % một nguồn logic `vifast-core` (`#![no_std]`, `#![forbid(unsafe_code)]`)**:

| Thành phần | Chế độ 1: **`ViFastOs-Mission`** (`Machine<StaticStore>`) | Chế độ 2: **`ViFastOs-General`** (`Machine<UntypedStore>`) | Chế độ Kiểm thử: **`SimHeapStore`** |
|---|---|---|---|
| **Nơi cài đặt Store** | `v2/kernel/vifast-core/src/store.rs` (`0 unsafe`, mặc định `S = StaticStore`) | `v2/kernel/vifast-plat-common/src/untyped_store.rs` + `vifast-objmem` (nằm sẵn trong allowlist R5 `D2-17` ⇒ **0 sửa `util.sh`!**) | `v2/sim/vifast-sim` (bọc `OwnedMem` + `Vec` trên host) |
| **Thân Object (`TCB`, `SC`, `Endpoint`, `Reply`, `Notification`)** | Mảng tĩnh `.corespace` (`MAX_TCBS = 64`…, khớp nguyên `spec/kernel.toml` `[limits.core]`) | Đặt trực tiếp trong RAM vật lý **`Untyped`** tại `pa + BODY_OFFSET` qua `physmap` & `ObjMem::{place,with,with_mut,with_many_mut,destroy}` | Đặt trong `OwnedMem` của `vifast-objmem` |
| **Ô `Slot` 48 B của `CNode`** | Cửa sổ tĩnh `SlotMem` (`4.096` word) | Đặt trực tiếp trong vùng **`Untyped`** của chính `CNode` tại `pa + CNODE_BASE_BYTES` (xoá trần `192` word/Region!) | Mảng động theo `slots` của CNode |
| **Sổ cái `Meta`, `Cap`, bảng băm `WaitAddr`, `PerHart`** | Mảng tĩnh cố định (`256 Meta`, `512 Cap`, `64 bucket`, `8 hart`) | **Boot-Time Dynamic Arena**: cắt 1 lần lúc khởi động từ `BootInfo` (`DTB`/`ACPI` dung lượng RAM & `N_harts` ở `KT-357`), co giãn tuyến tính theo RAM vật lý | `Vec` không biên trên máy chủ |
| **Hàng đợi Scheduler & Waiter** | **Danh sách liên kết xâm nhập (`Intrusive ObjId`)** nằm ngay trong `TcbData`/`WaitNode` (bỏ mảng cứng `[u16; MAX_OBJECTS + 1]`) | Dùng chung 100 % mã liên kết xâm nhập của `vifast-core` | Dùng chung 100 % mã `vifast-core` |
| **Đặc tính & Ứng dụng đích** | `#[derive(Clone, Copy)]`, `0` con trỏ, chống lật bit / biểu quyết 3 máy (TMR) cho tàu vũ trụ, vệ tinh, y tế, công nghiệp, snapshot L1 | Không giới hạn cứng luồng/object/cap — bung hết 100 % RAM cho Desktop, Server, `ViShell`, máy ảo Linux (`VCpu`) | Chấm dứt nợ bảo trì kép `sim` ↔ `core` |

### 0.2 Rào chắn cơ học bảo vệ các ghim của `spec/kernel.toml` (`check_kernel.py`)
Trước khi `KT-360` được phép mở `spec/` theo `APPROVED` của Agent 0, mọi task ở Đợt C3-1..C3-5 **bắt buộc bảo toàn đường dẫn tệp và tên hằng** đang được `spec/kernel.toml` ghim cứng:
1. **`v2/kernel/vifast-core/src/machine.rs`** (`#MAX_HARTS`, `#MAX_OBJECTS`, `#MAX_CAPS`, `#MAX_UNTYPEDS`, `#MAX_TCBS`, `#MAX_SCHED_CONTEXTS`, `#MAX_ENDPOINTS`, `#MAX_REPLIES`, `#MAX_NOTIFICATIONS`, `#MAX_QUEUED_MESSAGES`): ở `KT-353a`, giữ **`machine.rs` làm tệp cha** và đặt mô-đun con vào `v2/kernel/vifast-core/src/machine/{init,lifecycle}.rs` (**cấm** đổi thành `machine/mod.rs` vì sẽ làm `check_kernel.py` đỏ `E_SOURCE_FILE`!).
2. **`v2/kernel/vifast-plat-common/src/obj/mod.rs`** (`#MAX_OBJECTS`, `#MAX_CAPS`, `#MAX_UNTYPEDS`, `#MAX_MAPPINGS`, `#MAX_FLUSHES`): ở `KT-353b`, khi dời toàn bộ mã Sv39/PLIC sang `vifast-plat-riscv64`, **giữ nguyên bảng đối tượng VM kiến trúc-độc-lập tại `plat-common/src/obj/mod.rs`** cùng 5 hằng `MAX_*`.
3. **`v2/kernel/vifast-plat-x86_64/src/gdt.rs`**, **`v2/kernel/vifast-core/src/{mailbox,syscall}.rs`**, **`v2/sim/vifast-sim/src/mailbox.rs`** và **`review/agent0/GOI-ABI-V1_1.md`**: giữ nguyên vị trí & hằng số (chỉ cập nhật `gdt.rs` đồng thời với `spec/kernel.toml` ở `KT-360`; bắt buộc giữ `GOI-ABI-V1_1.md` ở `KT-359b`).

### 0.3 Tái bố trí Sóng thi công **0 % chồng chéo tệp** & Khớp lịch giờ DeepSeek (`D2-256`)
- **Khử xung đột ngầm của bản cũ**: bản cũ xếp `KT-353a` và `KT-350` chạy cùng đợt (cùng sửa `machine.rs` 1.430 LOC & `plat-riscv64`), và xếp `KT-354` cùng đợt `KT-359a` (cùng chạm `v2/tools/` & `v2/bin/lib/`). Bản mới sắp lại thành **8 đợt phát triển Thấp điểm + 1 đợt nghiệm thu Cao điểm**: mọi task chạy song song trong cùng một đợt chạm **các cây thư mục hoàn toàn rời nhau**, và `SMP-1a` (`KT-350`) chạy *sau* khi `KT-353b` đã hợp nhất `CoreBridge`/`IrqMcsLock` (viết 1 lần cho cả 3 arch thay vì sửa lặp 3 nơi!).
- **Mở song song Đợt C3-8 (`KT-363 ∥ KT-364 ∥ KT-365`)**: gỡ phụ thuộc giả `KT-364 → KT-365` (`HW1-Pre` PCIe/NVMe/IOMMU chỉ phụ thuộc `KT-357` ACPI/DTB và `KT-361` IRQ driver, không phụ thuộc Rust `std` `U1b`), rút ngắn 1 đợt chờ trên đường găng.
- **Đóng trọn khoảng trống `KT-347` / `KT-347b`**: nâng biên ELF user `riscv64` (`0x4010_0000`, `MAX_INIT_PAGES = 32`, gỡ trần `Scratch<24 KiB>`) ở `KT-353a`; bổ sung runner lái UART shell thật `product-{aarch64,x86_64}.sh` sau khi có RX IRQ ở `KT-357`.

---

## 1. Điều kiện mở khoá Chu kỳ 3

1. Khép sạch Chu kỳ 2 (`38/38 task DONE`): `KT-348a` (sửa treo `x86_64` TAP 10) và `KT-348b` (sửa `soak-aarch64.sh` mất log/trap/sendkey) `DONE`, kéo theo **`KT-348` ĐẠT** (`T2` 75/75 PASS, `v2 nightly --all` PASS, soak `product-shell` 3 arch + `virtio-blk` 0 lỗi, `dirty=0`, `qemu_foreign=0`).
2. Agent 0 duyệt `v2/agent/reports/R2-v2.{md,json}`, gắn tag **`v2.0`**, và nạp bảng §3 dưới đây vào `v2/agent/QUEUE.md` với 4 slot khởi chạy đầu tiên của Đợt C3-1 (`KT-353a`, `KT-357`, `KT-354`, `KT-359a`).
3. Mọi luật hiến định giữ nguyên không nới: `.data+.bss ≤ 262.144 B` [TK] (`*-product < 65.536 B`, `*-product-shell` có `.corespace ≥ 184.792 B` NOLOAD+NX), `size_of::<Machine<StaticStore>>() ≤ 196.608 B` [TK], `R1 ≤ 1.500` dòng/tệp, `D2-07` không retry, `D2-08` 0 marker ở ảnh sản phẩm, `D2-17` danh sách crate `unsafe` đóng, `D2-241` cấm GitHub Actions, `D2-256` lịch vận hành theo khung giờ DeepSeek.

---

## 2. Đồ thị Sóng thi công 0 chồng chéo tệp & Ánh xạ lịch DeepSeek (`KT-350..KT-366`, 21 task ≤ 3 h)

```text
[TAG v2.0 — sau KT-348a + KT-348b → KT-348 ĐẠT & APPROVED A0]
  │
  ▼ ĐỢT C3-1 [Thấp điểm 0,5× | 4 Slot song song | 0 chồng tệp | 0 sửa spec/]
  ├── [Slot 1 — Lõi/ASM/Shell] KT-353a (Xả R1 < 950 giữ machine.rs cha + asm/{trap,syscall,fpu} + biên ELF rv 0x4010_0000/>24K + KT-342c + .corespace NOLOAD+NX 3 arch)
  ├── [Slot 2 — Phần cứng Boot] KT-357  (BootInfo v1 + DTB thật rv/a64 + ACPI MADT/FADT/E820 x64 + UART RX IRQ a64 + runner product-{a64,x64}.sh)
  ├── [Slot 3 — Đột biến/Đo]    KT-354  (OBS-1/VER-1: mutants.toml + v2 mutants + trace ring 32 B D2-09 + vifast-bench -icount)
  └── [Slot 4 — Tools/Parity]   KT-359a (v2/tools/{checkers/wake,build,ops,crates} + tách kt58/59/60 9.229 LOC + run-common.sh + .scn 3 arch)
        └──► [Cao điểm: Lead gộp lô + gỡ 43 feature mut-* & nối caller v2/bin/lib/ + chạy T2 + repro-check 3 arch]
  │
  ▼ ĐỢT C3-2 [Thấp điểm 0,5× | 3 Slot song song | 0 chồng tệp | 0 sửa spec/]
  ├── [Slot 1 — Trait Bộ nhớ]   KT-355a (MEM-2a: Trait an toàn KernelStore + StaticStore mặc định Copy ≤196 KiB + hàng đợi xâm nhập + Meta Option<u32> 48 B)
  ├── [Slot 2 — Hợp nhất HAL]   KT-353b (Tái kiến trúc vifast-plat-common giữ obj/mod.rs + vifast-kbuild gom 3 build.rs + dời Sv39/PLIC sang rv + 9 crate forbid_unsafe)
  └── [Slot 3 — Tinh gọn Kho]   KT-359b (Pha C+: tag cycle1-raw-artifacts & docs-bundles-archive giữ cứng GOI-ABI-V1_1.md + gỡ cây mã v1 + gốc Cargo.toml → v2)
        └──► [Cao điểm: Lead chạy T2 + l1-tier.sh --tier medium]
  │
  ▼ ĐỢT C3-3 [Thấp điểm 0,5× | 2 Slot song song | 0 chồng tệp | 0 sửa spec/]
  ├── [Slot 1 — Đường găng SMP] KT-350  (SMP-1a: Preemptive 4 hart vifast-core trên riscv64 & aarch64 qua CoreBridge/IrqMcsLock chung)
  └── [Slot 2 — Untyped Động]   KT-355b (MEM-2b: UntypedStore bọc vifast-objmem cho body & slot CNode + BootInfo dynamic arena mở trần >64 TCB/>256 Obj + SimHeapStore)
  │
  ▼ ĐỢT C3-4 [Thấp điểm 0,5× | 2 Slot song song | 0 chồng tệp | 0 sửa spec/]
  ├── [Slot 1 — Đường găng SMP] KT-351  (SMP-1b: x86_64 4 hart vifast-core + hấp thu KT-340e gỡ 2.162 LOC sched.rs harness + PCID/ASID)
  └── [Slot 2 — Chuẩn hoá RV]   KT-353c (Gom vifast-kbin-riscv64 14 [[bin]] → 2 mode product/ktest < 18.000 LOC đồng nhất a64/x64)
  │
  ▼ ĐỢT C3-5 [Thấp điểm 0,5× | 2 Slot song song | 0 chồng tệp | 0 sửa spec/]
  ├── [Slot 1 — Đường găng SMP] KT-352  (SMP-2: Mailbox<N> tầng 2 + OVERFLOW bit trên IpiMailbox + VSpace::free_frame/destroy + range TLB shootdown)
  └── [Slot 2 — Gói duyệt ABI]  KT-358  (RFC GOI-ABI-V1_2.md 7 nhóm gồm cả [profiles] Mission/General — KHÔNG sửa spec/, trình Agent 0 APPROVED)
        └──► [Cao điểm: Lead T2 + vifast-bench đo tranh khoá 4 hart + Agent 0 thẩm định & ghi APPROVED cho KT-358]
  │
  ▼ ĐỢT C3-6 [Thấp điểm 0,5× — chỉ mở KT-360 sau APPROVED A0 | 2 Slot song song | 0 chồng tệp]
  ├── [Slot 1 — Mở ABI v1.2]    KT-360  (Áp spec/ ABI & Kernel v1.2 + vifast-gen + x86_64 syscall/sysret trên asm/syscall.rs & gdt.rs → khoá lại spec/)
  └── [Slot 2 — FPU & Cứng hoá] KT-356  (SEC-1: FPU/SIMD theo TCB trên asm/fpu.rs + SMEP/SMAP/PAN/SUM + virtio-rng + fuzz vifast-net)
  │
  ▼ ĐỢT C3-7 [Thấp điểm 0,5× | 2 Slot song song | 0 chồng tệp]
  ├── [Slot 1 — Driver Ngắt]    KT-361  (DRV-1: VirtIO blk/net/gpu/input chuyển sang IrqHandler + Notification 3 arch + PoC driver U-mode Frame.MapIo)
  └── [Slot 2 — Runtime Chuẩn]  KT-362  (U1a-Full: vifast-rt cấp trang Frame/Untyped thật + 5 .vidl + KT-345c TCP + ping/ifconfig + free DRAM + Tcb.Exec(path))
  │
  ▼ ĐỢT C3-8 [Thấp điểm 0,5× | 3 Slot song song | 0 chồng tệp]
  ├── [Slot 1 — Hệ sinh thái]   KT-363  (VSH2: Port vis-host-vifast / ViShell lên U1a-Full + script kiểm chéo cục bộ viscript-vifast.sh D2-241)
  ├── [Slot 2 — Rust std/UEFI]  KT-364  (U1b & UEFI: PoC Rust std PAL *-unknown-vifastos + boot/uefi-{x86_64,aarch64} OVMF/AAVMF nạp BootInfo v1)
  └── [Slot 3 — Tiền trạm HW1]  KT-365  (HW1-Pre: PCIe MSI-X + NVMe/AHCI + IOSpace virtio-iommu/SMMUv3/VT-d cô lập DMA + checklist máy thật)
  │
  ▼ ĐỢT C3-9 [Cao điểm 2× — Máy đo độc quyền dirty=0, qemu_foreign=0, 0 token LLM]
  └── [Nghiệm thu v2.1]         KT-366  (T2 + nightly --all [L1 400k StaticStore & UntypedStore + Miri + mutants + bench] + soak 4 hart/VSH2/blk ⇒ R2-v2_1 ⇒ tag v2.1)
```

---

## 3. Bảng đặc tả 21 task Chu kỳ 3 (`KT-350..KT-366`, mọi task ≤ 3 h)

> Mọi mã task tuân thủ nghiêm ngặt biểu thức chính quy `^KT-[0-9]+[a-z]?$` của `commit-msg` và `vifast-agentctl lint`. Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; chỉ Lead sửa `v2/bin/**` và chạy **T2 (`v2 gate --fast`)**.

### Nhóm I — Đại tu Nền tảng, Khám phá Phần cứng, Quan sát & Tinh gọn Kho (Đợt C3-1 & C3-2, 0 sửa `spec/`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) & Ranh giới tệp |
|---|---|---|---|---|---|
| **KT-353a** | P0 | **Pha B1 — Xả R1 lõi/asm/shell `< 950` LOC (bảo toàn `machine.rs`) + biên ELF `riscv64` `0x4010_0000` + `KT-342c` + `.corespace` NOLOAD+NX 3 arch** | `KT-348` | 3 h | **(D2-252/D2-255/D2-257)** (1) Tách 5 tệp nóng về `< 950` LOC: `core_ipc.rs` + `core_ipc/{send,recv,reply,queue}.rs`, **`machine.rs` (BẮT BUỘC giữ tệp cha `machine.rs` chứa `MAX_*` cho `check_kernel.py`!)** + `machine/{init,lifecycle}.rs`, `core_tcb.rs` + `core_tcb/*.rs`, `vifast-plat-x86_64/src/asm.rs` + `asm/{trap,syscall,fpu}.rs` (để `KT-356` và `KT-360` không đụng tệp!), và `v2/user/programs/shell.rs` (L1 digest trùng 100 %); (2) mở biên ELF user `riscv64` (`USER_STACK_TOP = 0x4010_0000`, `MAX_INIT_PAGES = 32`, nâng giới hạn nạp ELF `proc_exec` > 24 KiB cho `ViShell`, cập nhật đúng 2 ghim `guard=0x400f7000` ở `ktest.sh:647,830`); (3) đồng bộ `KT-342c` (`NBSend` keep-committed ở mọi đường chuyển trạng thái + parity sim); (4) đưa `Machine` `riscv64` sang `.corespace` NOLOAD (`.data+.bss` giảm về `~40 KiB`) và bật **W^X NX** toàn ảnh + `.corespace` cả 3 arch (đóng mục `BLOCKED.md` §2 W^X toàn ảnh `x86_64`) kèm đột biến thực thi ĐỎ; T2 PASS. |
| **KT-357** | P1 | **HW-0: `BootInfo` v1 + phân tích `DTB` thật (`rv`/`a64`) + `ACPI` `MADT`/`FADT`/E820 (`x64`) + UART RX IRQ + runner `product-{a64,x64}.sh`** | `KT-348` | 3 h | **(D2-257 / `u1-uefi-readiness.md` §4)** (1) Định nghĩa struct có phiên bản `BootInfo` v1 dùng chung 3 arch (bản đồ vùng RAM usable, `N_harts` thực tế, địa chỉ GIC/APIC/PLIC/UART); (2) parse FDT/DTB từ `kmain(dtb)` trên `riscv64`/`aarch64` và `MADT`/`FADT`/E820 trên `x86_64` thay hằng cứng QEMU (fail-closed có tên khi bảng hỏng); (3) bật **IRQ UART RX thật** (PL011 trên `aarch64`, COM1 trên `x86_64`) và thêm `product-{aarch64,x86_64}.sh` bơm lệnh thật vào ảnh `product-shell` (đóng nợ `KT-347b`!); cô lập trong mô-đun mới để 0 xung đột với `KT-353a`; T2 PASS. |
| **KT-354** | P1 | **Pha B4 — OBS-1 & VER-1: `v2 mutants` (gỡ 43 `mut-*` + `selfbreak`) + trace 32 B (`D2-09`) + `vifast-bench`** | `KT-348` | 3 h | **(D2-252/D2-255)** (1) Dựng `v2/test/mutants.toml` + công cụ `v2 mutants` thay toàn bộ **43 feature đột biến `mut-*` + `selfbreak`** (Lead gỡ sạch khỏi `Cargo.toml` & nối vào `v2 nightly --all` khi merge); (2) nối ring trace nhị phân 32 B luôn bật (`FixedRing`) + dump `pmemsave` + `vifast-trace` (`D2-09`); (3) dựng `vifast-bench` + `v2/test/perf/history.csv` dưới QEMU `-icount` đo IPC round-trip & tỷ lệ chờ khoá SMP (làm thước đo định lượng mở `S3`); T2 PASS. |
| **KT-359a** | P1 | **Pha B5 — Tổ chức `v2/tools/` + package `checkers/wake/` (11.046 LOC) + tách `kt58`/`kt59`/`kt60` (9.229 LOC) + `run-common.sh` + `.scn` 3 arch** | `KT-348` | 3 h | **(D2-252/D2-255)** (1) Tổ chức `v2/tools/{checkers,build,ops,crates}/` (Lead cập nhật đường dẫn gọi trong `v2/bin/lib/` lúc merge theo `D2-253`); (2) tách `check-wake-paths.py` thành package `v2/tools/checkers/wake/{lexer,parser,rules,scope,cli,selftest}.py` (giữ **461/461** selftest); (3) tách cả **3 khối test parity sim** `kt{60,58,59}_core_parity.rs` (`9.229` LOC) theo miền; (4) rút `v2/test/boot/lib/run-common.sh`, gọn `shell-edge.sh`, chạy bộ `.scn` conformance chung 3 arch (`D2-13`) & rà soát Windows repro (`B2`); 0 chạm `v2/kernel/` hay `v2/user/`; T2 PASS. |
| **KT-355a** | P0 | **MEM-2a: Trait an toàn `KernelStore` & Chế độ `ViFastOs-Mission` (`StaticStore` mặc định, `0 unsafe`, `Copy`, `≤ 196.608 B`)** | `KT-353a` | 3 h | **(D2-257 — Trụ cột Lõi Kép phần 1)** Trong `v2/kernel/vifast-core` (`#![forbid(unsafe_code)]`, 0 phụ thuộc ngoài, 0 sửa `spec/`): (1) thiết kế trait an toàn `KernelStore` trừu tượng hoá truy cập sổ cái `Meta`/`Cap`, cửa sổ slot `CNode`, các thân đối tượng (`TcbData`, `ScData`, `EndpointData`, `ReplyData`, `NotificationData`, `QueuedMsg`) và bảng `WaitOnAddress`; (2) chuyển hàng đợi ready của `HartSched` từ mảng cố định `[u16; MAX_OBJECTS + 1]` sang **danh sách liên kết xâm nhập (`sched_prev`/`sched_next` trong `TcbData`)**, đồng thời mở rộng `Meta.untyped` từ `Option<u8>` lên `Option<u32>` bằng đóng gói trường sao cho `size_of::<Meta>() == 48 B` không đổi; (3) tham số hoá `pub struct Machine<S: KernelStore = StaticStore>` (`pub type MissionMachine = Machine<StaticStore>;`) giữ nguyên `#[derive(Clone, Copy)]`, `size_of::<MissionMachine>() ≤ 196.608 B` và giữ nguyên toàn bộ hằng `MAX_*` trong `machine.rs` (`check_kernel.py` xanh, 0 sửa nơi gọi ở 3 arch); 3 đột biến đỏ + L1-medium 10k + T2 PASS. |
| **KT-353b** | P0 | **Pha B2 — Tái kiến trúc `vifast-plat-common` (giữ `obj/mod.rs`), rút `vifast-kbuild` (3 `build.rs`), hợp nhất HAL 3 arch & khoá 9 crate `forbid(unsafe_code)`** | `KT-353a` | 3 h | **(D2-252/D2-255/D2-257)** (1) Rút vào `vifast-plat-common` các khối dùng chung 3 arch: `ArchIrq`, `IrqMcsLock<T, A>`, `CoreBridge<A>`, `ElfUserLoader`, `ProductShellHost`, `ImageHash`/`WxReport` (**giữ nguyên `plat-common/src/obj/mod.rs` và 5 hằng `MAX_*` cho `check_kernel.py`**); (2) rút crate host `v2/kernel/vifast-kbuild` (`#![forbid(unsafe_code)]`, 0 dep ngoài) cắt 3 `vifast-kbin-*/build.rs` từ `1.789 → ~500` LOC; (3) dời toàn bộ mã đặc thù Sv39 (`mmu`/`plic`/`trap`/`timer`/`tlb`/`user`) sang `vifast-plat-riscv64` (`0` từ khoá `sv39`/`plic`/`satp`/`sbi` trong `plat-common`) và đưa `TableMemory` lên `vifast-pte`; (4) bật `#![forbid(unsafe_code)]` cho 9 crate 0-`unsafe` (`vifast-abi`, `vifast-idl`, `vifast-ring`, `vifast-traceformat`, `vifast-fsutil`, `vifast-kbin-aarch64`, `vifast-kbin-x86_64`, `vifast-sim`, `vifast-kbuild`); cắt giảm **`≥ 4.500` LOC** lặp; 0 chạm `vifast-core`; T2 PASS. |
| **KT-359b** | P1 | **Pha C+ — Tinh gọn cấu trúc & dung lượng Repository (archive rác `JOURNAL/` & `docs/bundles/`, giữ cứng `GOI-ABI-V1_1.md`, gỡ cây mã v1 đóng băng)** | `KT-359a` | 2 h | **(D2-252/D2-255 — sau tag `v2.0`)**: (1) lưu trữ ~1.120 tệp rác Chu kỳ 1 không phải `.md` trong `v2/agent/JOURNAL/` (ngoài `summary.txt`, `scope=`, `counters.toml`, liên kết sống) vào tag annotated **`cycle1-raw-artifacts`**; (2) lưu trữ `docs/bundles/` & 29 tệp lịch sử `review/` vào tag annotated **`docs-bundles-archive`** — **BẮT BUỘC GIỮ NGUYÊN `review/agent0/GOI-ABI-V1_1.md`**, mọi RFC đang mở, `docs/v1/` đọc-only (§4.7), `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md` (§4.6); (3) gỡ cây mã v1 đóng băng (`crates/`, `legacy/`, `deploy/`, `python-model/`, `scripts/` v1 — vẫn lưu bất biến tại tag `v1-archive`), sửa fixture `case-r4-path-dep`, cập nhật `README.md`, dời báo cáo tuần cũ vào `v2/agent/archive/reports/`, đưa `Cargo.toml` gốc trỏ sang workspace `v2/` (cắt ~50 % số tệp & ~62 % dung lượng); `check_kernel.py` + `verify-numbers.py` (`ok=5 fail=0`) + T2 PASS. |

### Nhóm II — Mở khoá Bộ nhớ Động `UntypedStore`, Đa nhân SMP 4 Hart & Chuẩn hoá `riscv64` (Đợt C3-3..C3-5, 0 sửa `spec/`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) & Ranh giới tệp |
|---|---|---|---|---|---|
| **KT-350** | P0 | **SMP-1a: Preemptive 4 hart `vifast-core::Machine` trên `riscv64` & `aarch64`** | `KT-353b, KT-355a` | 3 h | Thêm `Machine::set_harts(n)` (đọc từ `BootInfo` khi có) + `PerHart` active TCB trên nền `CoreBridge<A>` và `IrqMcsLock<T, A>` dùng chung; 4 hart đồng thời chạy tác vụ U-mode preemptive qua ngắt timer trên cả `riscv64` và `aarch64` (`0` starvation, `0` double-pick, giữ đúng luật đọc `published` §4.16); 3 đột biến đỏ; T2 PASS. |
| **KT-355b** | P1 | **MEM-2b: Chế độ `ViFastOs-General` (`UntypedStore` bọc `vifast-objmem` + `BootInfo` Dynamic Arena mở trần `> 64 TCB / > 256 Obj`) & `SimHeapStore`** | `KT-353b, KT-355a, KT-357` | 3 h | **(D2-257 — Trụ cột Lõi Kép phần 2)** Cài đặt `UntypedStore` trong `vifast-plat-common/src/untyped_store.rs` + `vifast-objmem` (nằm sẵn trong allowlist R5 `D2-17`, 0 sửa `util.sh`, giữ `vifast-core` 100 % `forbid(unsafe_code)`): (1) đặt thân đối tượng lõi (`TCB`, `SC`, `Endpoint`, `Reply`, `Notification`) và mảng slot `CNode` 48 B trực tiếp vào vùng RAM vật lý **`Untyped`** qua `physmap` & `ObjMem` (`place`/`with`/`with_mut`/`with_many_mut`/`destroy` idempotent), xoá trần `192` slot/Region; (2) cắt **Boot-Time Dynamic Arena** từ `BootInfo` (`DTB`/`ACPI` cỡ RAM & `N_harts` của `KT-357`) cho bảng `Meta`/`Cap`/`WaitAddr` co giãn theo RAM thật; (3) thêm `SimHeapStore` chạy vi sai trực tiếp trên `vifast_core::Machine` + đóng nợ `KT-74` ca lỗi-muộn `ObjKind::Untyped`; chứng minh trên cùng mã lõi: `StaticStore` chặn an toàn ở TCB thứ 65 (`NotEnoughMemory`) còn `UntypedStore` tạo & `Revoke` sạch **≥ 512 TCB / ≥ 1.024 Cap** trả watermark về `0`; L1-medium 40k + Miri + T2 PASS. |
| **KT-351** | P0 | **SMP-1b: `x86_64` 4 hart `vifast-core::Machine` & hấp thu `KT-340e` (gỡ 2.162 LOC harness `sched.rs`)** | `KT-350` | 3 h | Đưa cả 4 hart `x86_64` vào `vifast-core::Machine` qua `CoreBridge` chung (`init` sau `smp::run`, `resolve_addr` thật); chuyển trọn độ phủ preemption/priority/sleep 4 hart sang `vifast-core`, gỡ bỏ 2.162 dòng harness tạm `sched.rs` (`KT-340e` từ `D2-250`) và đối xứng bộ ca âm `# sched:`; bật & kiểm `PCID`/`ASID` khi CPU hỗ trợ; chỉ chạm nhánh `x86_64` (0 xung đột với `KT-353c`); T2 PASS. |
| **KT-353c** | P1 | **Pha B3 — Gom `vifast-kbin-riscv64` 14 `[[bin]]` → 2 mode (`product`/`ktest`)** | `KT-350, KT-353b` | 3 h | **(D2-252)** Chuẩn hoá `vifast-kbin-riscv64` (`32.356` LOC / 14 `[[bin]]`) về đúng mô hình 2 mode `product` (có lựa chọn profile `mission` `StaticStore` vs `general` `UntypedStore`) và `ktest` như `aarch64`/`x86_64`, gom 32 tệp `ktest*.rs` + 29 `init/*.rs` thừa về 1 suite `ktest` gọn (`< 18.000` LOC); giữ 100 % độ phủ matrix & parity; chỉ chạm `kbin-riscv64`; T2 PASS. |
| **KT-352** | P1 | **SMP-2: Tầng 2 `vifast_sync::Mailbox<N>` trên `IpiMailbox` + `VSpace::free_frame`/range TLB shootdown** | `KT-351` | 3 h | **(D2-244)** Cài payload `Wake{hart,tcb}`/`Migrate`/`TlbFlush{asid,va}` dùng `vifast_sync::Mailbox<N>` đặt trên `IpiMailbox` kèm cờ `OVERFLOW` tự rơi về full-rescan/full-flush khi đầy (bảo đảm toán học **không bao giờ mất đánh thức**); bổ sung `VSpace::free_frame`/`destroy` + TLB shootdown theo dải & đa yêu cầu bay trên cả 3 arch; đo lại tranh khoá 4 hart bằng `vifast-bench`; 3 đột biến đỏ; T2 PASS. |
| **KT-358** | P0 | **RFC Gói duyệt ABI & Cấu hình Kernel v1.2 (`review/agent0/GOI-ABI-V1_2.md`) — KHÔNG sửa `spec/`** | `KT-351, KT-355b, KT-357` | 3 h | **(D2-251/D2-257)** Soạn gói thẩm định 7 nhóm trình Agent 0 (0 chạm `spec/`): (1) `IrqControl`/`IrqHandler` + `Notification`; (2) `Frame.MapIo`/`UnmapIo` + `IOSpace` (IOMMU); (3) `Tcb.Exec` + mở các method `VSpace`/`PageTable` (`install_at = "Chu-ky-3"`); (4) bố trí GDT `planned` (`sysret_ready = true`) cho `syscall`/`sysret` `x86_64` giữ nguyên quy ước thanh ghi `7C-r13`; (5) phương án `CNode.Mutate`/`Rotate`; (6) MCS refill đa đoạn / `SetTimeoutEp` (`D2-16`); **(7) khai báo chính thức 2 profile `[profiles]` (`mission = "StaticStore"`, `general = "UntypedStore"`) và quy tắc Boot Arena trong `spec/kernel.toml`**. **Chỉ mở `KT-360` sau khi có dòng `APPROVED` riêng của Agent 0.** |

### Nhóm III — Thi hành ABI v1.2, Bảo mật FPU/SIMD, Driver Ngắt, Hệ sinh thái `ViScript` & Tiền trạm Máy thật (Đợt C3-6..C3-9)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) & Ranh giới tệp |
|---|---|---|---|---|---|
| **KT-360** | P0 | **Áp `spec/` ABI & Kernel v1.2 + `vifast-gen` + bật `x86_64` `syscall`/`sysret` (chờ `APPROVED` A0)** | `KT-352, KT-358` *(APPROVED A0)* | 3 h | Áp chính xác các mục đã được Agent 0 `APPROVED` ở `KT-358` vào `spec/abi.toml` & `spec/kernel.toml`; bật cặp lệnh phần cứng `syscall`/`sysret` (`swapgs` + per-CPU GS + đảo GDT `planned` trong `gdt.rs` & `asm/syscall.rs`, giữ hợp đồng thanh ghi `7C-r13`!); sinh lại `vifast-abi`, kiểm `check_abi.py` + `check_kernel.py` + `vifast-gen --check`, **khoá cứng `spec/` trở lại ngay sau commit**; 0 đụng `asm/fpu.rs`; T2 PASS. |
| **KT-356** | P1 | **SEC-1: Lưu/khôi phục FPU/SIMD theo TCB + cứng hoá `SMEP`/`SMAP`/`PAN`/`SUM` + `virtio-rng` + fuzz `vifast-net`** | `KT-352` | 3 h | Lưu/khôi phục eager có theo dõi bẩn cho ngữ cảnh FPU/SIMD theo từng TCB (`fs0..fs31`, NEON `v0..v31`, `fxsave`/`xsave` trong `asm/fpu.rs` với cỡ đọc từ `BootInfo` / CPUID, kiểm chứng 2 tác vụ U-mode tính dấu phẩy động song song trên 4 hart không lẫn thanh ghi); bật & kiểm ca âm phần cứng `SMEP`/`SMAP` (`x64`), `PAN`/`PXN` (`a64`), `SUM=0` (`rv`) + test đối kháng; thêm driver `virtio-rng` & target fuzz `vifast-net`; T2 PASS. |
| **KT-361** | P1 | **DRV-1: Chuyển VirtIO (`blk`/`net`/`gpu`/`input`) sang IRQ `Notification` + PoC driver U-mode không đặc quyền (`D2`)** | `KT-360` | 3 h | Thay vòng quay busy-poll của các driver VirtIO bằng `IrqControl`/`IrqHandler` + `Notification` trên cả 3 arch; chạy thật PoC 1 driver thiết bị không đặc quyền ở User space dùng `Frame.MapIo` (Device Untyped) + `IrqHandler.Ack`; đột biến bỏ `Ack` IRQ ⇒ ĐỎ; T2 PASS. |
| **KT-362** | P1 | **U1a-Full: `vifast-rt` trên `Frame`/`Untyped` thật + 5 `.vidl` chuẩn + `KT-345c` TCP + `ping`/`ifconfig`/`free` + `Tcb.Exec(path)` 3 arch** | `KT-355b, KT-356, KT-360` | 3 h | **(D2-255/D2-257 / `u1-uefi-readiness.md` §2)** (1) Nâng cấp allocator của `vifast-rt` từ arena tĩnh 8 KiB lên cấp trang `Frame`/`Untyped` động thật; (2) sinh & nối đủ 5 giao diện `.vidl` (`proc`, `fs`, `net`, `term`, `socket.vidl` — đóng `KT-345c` TCP thật `connect`/`send`/`recv`/`poll`/`close`); (3) mở `ping`, `ifconfig`, `free` (hiển thị cả thống kê DRAM `FrameArena` lẫn `UntypedStore` / `StaticStore`) và `Tcb.Exec(path)` nạp ELF bất kỳ từ FS trên ảnh `product-shell` cả 3 arch; T2 PASS. |
| **KT-363** | P1 | **VSH2: Port `vis-host-vifast` / `ViShell` (`ViScript`) lên `U1a-Full` & kiểm chéo cục bộ liên repo (`D2-241`)** | `KT-362` | 3 h | Biên dịch & chạy tương tác thật `ViShell` (`ViScript`) làm shell trên ảnh `product-shell` ViFastOs (`UntypedStore` profile) qua `vifast-rt` + `.vidl`; cài đặt `v2/test/crosscheck/viscript-vifast.sh` (fail-closed theo khoá đầu vào + golden dương/âm, chạy trên máy đo cục bộ qua `measure-lock`, **0 GitHub Actions** theo `D2-241`); T2 PASS. |
| **KT-364** | P2 | **U1b & UEFI: PoC Rust `std` PAL (`*-unknown-vifastos`) + Boot UEFI (`OVMF`/`AAVMF`) nạp `BootInfo` v1** | `KT-357, KT-362` | 3 h | Chạy PoC ứng dụng Rust `std` (`U1b`: thread/mutex trên `WaitAddr`, đồng hồ vDSO, fs, stdout) với target JSON & PAL tối thiểu; dựng `boot/uefi-{x86_64,aarch64}` (PE/COFF `efi_main`) boot qua `OVMF`/`AAVMF` (`--firmware uefi`), chụp memory map ngay trước `ExitBootServices` nạp vào `BootInfo` v1 mà **không đổi 1 byte** đường boot `-kernel` mặc định; T2 PASS. |
| **KT-365** | P2 | **HW1-Pre: PCIe MSI-X + NVMe/AHCI + `IOSpace` (`virtio-iommu`/`SMMUv3`/`VT-d`) & Checklist Phần cứng thật** | `KT-357, KT-361` | 3 h | *(Chạy song song với `KT-363` & `KT-364`!)* Dựng `IOSpace` cô lập DMA phần cứng trên QEMU (`virtio-iommu`/`VT-d`/`SMMUv3`, kèm ca âm thiết bị DMA ghi ngoài bảng trang IOMMU bị chặn ngay); chạy PoC PCIe MSI-X + đọc/ghi block NVMe/AHCI; lập hồ sơ kiểm định phần cứng vật lý `HW1` cho cả 2 profile (`Mission` bo mạch nhúng/vũ trụ & `General` PC/Workstation); T2 PASS. |
| **KT-366** | P0 | **Cổng nghiệm thu Chu kỳ 3 (`v2.1`): `nightly --all` (2 Store + `mutants` + `bench`) + Soak 4 hart/`VSH2`/UEFI + Báo cáo `R2-v2_1`** | bao đóng đủ 20/20 task `KT-350..KT-365` | 3 h | **Chạy ở Khung giờ Cao điểm DeepSeek (`D2-256`, 0 token LLM, máy đo độc quyền `dirty=0`, `qemu_foreign=0`)**: T2 PASS; `v2 nightly --all` (`L1-full` 400k cho **cả `StaticStore` lẫn `UntypedStore`** + Miri + `v2 mutants` thật + `vifast-bench` + `STATUS.md`) PASS; `v2 nightly --soak --profile all` (ảnh `product-shell` lái UART thật cả 3 arch 4 hart + `virtio-blk` + `VSH2` + crosscheck `ViScript`) đều 0 lỗi (không retry `D2-07`); xuất `v2/agent/reports/R2-v2_1.{md,json}` qua `check_report.py` + `verify-numbers.py`; trình Agent 0 chốt tag **`v2.1`**. |

---

## 4. Tầm nhìn nối tiếp sau `v2.1` — Khung định hướng Chu kỳ 4 (`v2.2` / `HW1` / Đa cụm & Desktop)

Nhờ nền tảng **Dual-Profile `KernelStore` + `BootInfo`/UEFI + IRQ Userland Driver + `U1b` Rust `std`** hoàn tất ở `v2.1`, Chu kỳ 4 (`v2.2`) sẽ mở rộng hoàn toàn ở tầng Plug-in và User space mà **không bao giờ phải thiết kế lại `vifast-core` hay phá vỡ ABI lõi**:

1. **`HW1` — Đưa lên phần cứng vật lý thật (Bare-Metal Bring-up)**:
   - Profile **`ViFastOs-General` (`UntypedStore`)** boot qua USB UEFI trên PC `x86_64` (NVMe + xHCI USB bàn phím + GOP framebuffer) và bo mạch `riscv64` / `aarch64` thật.
   - Profile **`ViFastOs-Mission` (`StaticStore`)** đóng gói ảnh tĩnh bất biến kèm kiểm tra băm định kỳ `Machine` (`TMR-ready` chống lật bit bức xạ) cho bo mạch điều khiển thời gian thực / không gian.
2. **`S3` & `F1` — Mở rộng Đa cụm (Clustered Multikernel) & IPC Fastpath dựa trên số đo `vifast-bench`**:
   - Nếu số đo `vifast-bench` ở `KT-352`/`KT-366` cho thấy tỷ lệ chờ khoá > 10 % [ƯL] khi nâng số nhân lên 8–64 hart/NUMA: khởi tạo **mỗi cụm L3/NUMA một thực thể `Machine<UntypedStore>` riêng** (`id = (cluster_id << 56) | counter` đã quy hoạch từ thiết kế §4.2), giao tiếp chéo cụm qua `Mailbox<N>` phi khoá.
   - Bổ sung nhánh **`F1` (`ipc_fastpath`)** cho đường nóng `Call` / `ReplyRecv` ≤ 4 thanh ghi MR không kèm cap, đối chiếu vi sai 1-1 với hàm chuẩn `ipc::transfer`.
3. **`H1..H2` (Ảo hoá `VCpu`) & `G1..G5` (Môi trường Desktop + `ViFastKey`)**:
   - Cài đặt đối tượng `VCpu` + `VSpace` khách (`riscv64` H-extension → `aarch64` EL2 → `x86_64` VT-x) để boot máy ảo Linux khách trên nền `ViFastOs-General`.
   - Mở dòng đồ hoạ Desktop (`D-08` đã hết điều kiện khoá): Compositor cửa sổ U-mode, `ViTerm` (`term.vidl`), bộ gõ tiếng Việt `ViFastKey` (`G4`), và âm thanh thời gian thực trên MCS refill đa đoạn (`G5`).
