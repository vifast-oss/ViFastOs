# KẾ HOẠCH THI CÔNG CHU KỲ 3 (`v2.1` — `KT-350..KT-366`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng) — theo **D2-251** (`2026-11-23`).
**Trạng thái**: **THIẾT KẾ ĐÃ CHỐT, CHỜ MỞ KHOÁ SAU `KT-348` / `v2.0`** *(chưa nạp vào `v2/agent/QUEUE.md` để giữ bảng hàng đợi Chu kỳ 2 sạch 100 %)*.
**Nguồn sự thật**: [`AGENTS.md`](../../AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](../../docs/QUYET-DINH-sau-review.md), [`v2/agent/DECISIONS.md`](../agent/DECISIONS.md) (**D2-240..D2-251**), [`KE-HOACH-CHU-KY-2-2026-11-22.md`](../../KE-HOACH-CHU-KY-2-2026-11-22.md).

> **Mục tiêu một câu**: Đưa ViFastOs từ nền tảng `v2.0` (3 kiến trúc chung 1 lõi `vifast-core` + 1 `product-shell` + 28/28 parity v1) lên **`v2.1` sẵn sàng hệ sinh thái & phần cứng thật**: **SMP preemptive 4 hart thật cả 3 arch**, **`vifast-objmem` + FPU/SIMD + DTB/ACPI**, **ABI v1.2 (IRQ `Notification` + `syscall`/`sysret` + `Tcb.Exec`)**, **`v2 mutants` + trace 32 B + `vifast-bench`**, và chạy thật **`ViScript`/`VSH2` + `U1b`/UEFI**.

---

## 1. Điều kiện mở khoá Chu kỳ 3

1. `KT-348` ĐẠT (`T2` 75/75 PASS, `v2 nightly --all` PASS, soak `product-shell` 3 arch + `virtio-blk` 0 lỗi, `dirty=0`, `qemu_foreign=0`).
2. Agent 0 duyệt `v2/agent/reports/R2-v2.{md,json}`, gắn tag `v2.0`, và chép bảng §3 dưới đây vào `v2/agent/QUEUE.md` với 4 slot khởi chạy đầu tiên (`KT-350`, `KT-353`, `KT-354`, `KT-357`).
3. Mọi luật hiến định giữ nguyên không nới: `.data+.bss ≤ 262.144 B` (`*-product < 65.536 B`), `size_of::<Machine>() ≤ 196.608 B`, `R1 ≤ 1.500` dòng/tệp, `D2-07` không retry, `D2-08` 0 marker ở ảnh sản phẩm, `D2-17` danh sách crate `unsafe` đóng, `D2-241` cấm GitHub Actions.

---

## 2. Đồ thị phụ thuộc 4 đợt (`KT-350..KT-366`, 17 task ≤ 3 h)

```text
[TAG v2.0 — sau KT-348 ĐẠT & APPROVED A0]
  │
  ▼ [ĐỢT C3-1 — SMP 4 Hart, Bộ nhớ, R1 Lõi & Công cụ Kiểm chứng (0 đổi spec/)]
  ├── [Slot 1] KT-353 (MEM-1: cắt core_ipc/machine < 1.100, rút Sv39/PLIC, gom 14 bin riscv64)
  │              └──► KT-355 (MEM-2: vifast-objmem cấp phát động từ Untyped)
  ├── [Slot 2] KT-354 (OBS-1/VER-1: v2 mutants thật gom 28 feature x64 + trace 32 B + vifast-bench)
  │              └──► KT-359 (VER-2: .scn conformance 3 arch + v2/tools/ + Windows B2 + đánh giá gỡ v1)
  ├── [Slot 3] KT-357 (HW-0: DTB thật rv/a64 + ACPI MADT/FADT/E820 x64 + UART RX IRQ a64)
  │              └──► KT-358 (RFC ABI v1.2 — KHÔNG sửa spec/, chờ APPROVED A0)
  └── [Slot 4 — ĐƯỜNG GĂNG] KT-350 (SMP-1a: preemptive 4 hart vifast-core rv + a64)
                              └──► KT-351 (SMP-1b: x64 4 hart core + hấp thu KT-340e gỡ sched.rs harness)
                                     └──► KT-352 (SMP-2: Mailbox<N> tầng 2 + free_frame + range shootdown)
                                            └──► KT-356 (SEC-1: FPU/SIMD theo TCB + SMEP/SMAP/PAN + rng + fuzz)
  │
  ▼ [ĐỢT C3-3 — ABI v1.2, Driver Ngắt, U1a/VSH2 & U1b/UEFI (chỉ mở KT-360 sau APPROVED A0 cho KT-358)]
  ├── (KT-352, KT-355, KT-356, KT-358 APPROVED) ──► KT-360 (Áp spec/ ABI v1.2 + syscall/sysret x64)
  │     ├──► KT-361 (DRV-1: VirtIO polling → IRQ Notification + driver userland nền D2)
  │     ├──► KT-362 (U1a-Full: vifast-rt chuẩn + .vidl + ping/ifconfig + Tcb.Exec(path))
  │     │      └──► KT-363 (VSH2: port vis-host-vifast/ViShell + kiểm chéo cục bộ ViFastOs↔ViScript)
  │     └──► KT-364 (U1b + UEFI: Rust std PAL *-unknown-vifastos + UEFI OVMF/AAVMF)
  │
  ▼ [ĐỢT C3-4 — Chuẩn bị Phần cứng thật & Nghiệm thu v2.1]
  ├── (KT-357, KT-361, KT-364) ──► KT-365 (HW1-Pre: PCIe MSI-X + NVMe/AHCI + IOSpace IOMMU)
  └── (bao đóng KT-350..KT-365)──► KT-366 (Cổng nghiệm thu Chu kỳ 3: nightly + mutants + bench + soak ⇒ R2-v2_1)
```

---

## 3. Bảng 17 task Chu kỳ 3 (`KT-350..KT-366`, mọi task ≤ 3 h)

> Mọi mã task tuân thủ mẫu `^KT-[0-9]+[a-z]?$` của `git-guard/commit-msg` & `vifast-agentctl lint`.

### Đợt C3-1 — SMP Preemptive 4 Hart, Hợp nhất Lõi & Nền tảng Quan sát (0 sửa `spec/`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|
| **KT-350** | P0 | **SMP-1a: Preemptive 4 hart `vifast-core::Machine` trên `riscv64` & `aarch64`** | `KT-348` | 3 h | Thêm `Machine::set_harts` + `PerHart` active TCB; 4 hart đồng thời chạy U-mode preemptive qua timer tick & `IrqMcsLock` trên `riscv64` và `aarch64` (0 starvation, 0 double-pick); 3 đột biến đỏ; T2 PASS. |
| **KT-351** | P0 | **SMP-1b: `x86_64` 4 hart `vifast-core::Machine` & hấp thu `KT-340e` (gỡ `sched.rs` harness)** | `KT-350` | 3 h | Đưa 4 hart `x86_64` vào `vifast-core::Machine` (`init` sau `smp::run`, `resolve_addr` thật); chuyển đủ độ phủ preemption/priority/sleep 4 hart sang `vifast-core`, gỡ 2.162 dòng harness `sched.rs` (`KT-340e` từ D2-250) và cập nhật đối xứng bộ ca âm `# sched:` kèm văn bản D2; kiểm `PCID`/`ASID` khi CPU hỗ trợ; T2 PASS. |
| **KT-352** | P1 | **SMP-2: Tầng 2 `vifast_sync::Mailbox<N>` trên `IpiMailbox` + `free_frame`/range TLB shootdown** | `KT-351` | 3 h | Cài payload `Wake{hart,tcb}`/`Migrate`/`TlbFlush{asid,va}` dùng `vifast_sync::Mailbox<N>` nằm trên `IpiMailbox` với bit `OVERFLOW` rơi về full-rescan/full-flush (D2-244, không bao giờ mất wake); bổ sung `VSpace::free_frame`/`destroy` + shootdown theo dải & đa yêu cầu bay (đóng nợ `KT-340d`); 3 đột biến đỏ; T2 PASS. |
| **KT-353** | P1 | **MEM-1: Tách `core_ipc.rs`/`machine.rs` `< 1.100`, đóng `D2-83` & gom 14 bin `kbin-riscv64`** | `KT-348` | 3 h | Tách `vifast-core/src/core_ipc.rs` (`1.471`) & `machine.rs` (`1.430`) về `< 1.100` dòng (L1 digest trùng tuyệt đối); rút Sv39/PLIC khỏi `vifast-plat-common`, đưa `TableMemory` lên `vifast-pte` (đóng `D2-83`); gom 14 `[[bin]]` của `vifast-kbin-riscv64` về 2 mode `product`/`ktest` như `aarch64`/`x86_64`; T2 PASS. |
| **KT-354** | P1 | **OBS-1 & VER-1: `v2 mutants` thật + trace nhị phân 32 B (`D2-09`) + `vifast-bench`** | `KT-348` | 3 h | Dựng `v2/test/mutants.toml` + `v2 mutants` gom 28 feature đột biến ad-hoc của `vifast-kbin-x86_64/Cargo.toml` về khung chung và nối vào `v2 nightly --all` (thay `selfbreak` tạm); dựng ring trace nhị phân 32 B luôn bật + dump `pmemsave` + tool `vifast-trace` (`D2-09`); dựng `vifast-bench` + `v2/test/perf/history.csv` dưới `-icount` (đo tranh khoá SMP làm căn cứ mở `S3`); T2 PASS. |

### Đợt C3-2 — Động hoá Bộ nhớ, An toàn FPU/SIMD, Phần cứng `DTB`/`ACPI` & RFC ABI v1.2

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|
| **KT-355** | P1 | **MEM-2: `vifast-objmem` cấp phát đối tượng thật từ `Untyped` (D2-14)** | `KT-353` | 3 h | Chuyển cấp phát đối tượng lõi từ bảng tĩnh sang `Untyped` thật qua `vifast-objmem` (theo §28 `ObjStore` ở `k3b-core.md`), giữ `size_of::<Machine>() ≤ 196.608 B`, đóng nợ `KT-74` ca lỗi-muộn `ObjKind::Untyped`; L1-medium 40k + Miri + T2 PASS. |
| **KT-356** | P1 | **SEC-1: Lưu/khôi phục FPU/SIMD theo TCB + cứng hoá `copy_user` (`SMEP`/`SMAP`/`PAN`) + `virtio-rng` + fuzz** | `KT-352` | 3 h | Lưu/khôi phục ngữ cảnh FPU/SIMD khi chuyển ngữ cảnh (`fs0..fs31`, NEON `v0..v31`, `fxsave`/`xsave`) kèm kiểm chứng 2 tác vụ user tính song song không lẫn thanh ghi (đo vào ngân sách `Machine`); bật & kiểm ca âm `SMEP`/`SMAP`/`PAN`/`SUM` + test đối kháng `copy_from_user`/`copy_to_user`; thêm `virtio-rng` + target fuzz `vifast-net`; T2 PASS. |
| **KT-357** | P1 | **HW-0: Phân tích `DTB` thật (`rv`/`a64`) + `ACPI` `MADT`/`FADT`/E820 (`x64`) + UART RX IRQ thật** | `KT-348` | 3 h | Parse FDT/DTB từ `kmain(dtb)` trên `riscv64`/`aarch64` (đóng nợ `kbin-aarch64/main.rs:635`) và `MADT`/`FADT`/E820 trên `x86_64` để lấy RAM/CPU/GIC/APIC/UART thay hằng cứng QEMU (fail-closed khi DTB/ACPI hỏng); thay ring script bằng **IRQ UART RX thật** trên `aarch64` (đóng nợ `KT-339` #3); T2 PASS. |
| **KT-358** | P0 | **RFC Gói duyệt ABI v1.2 (`review/agent0/GOI-ABI-V1_2.md`) — KHÔNG sửa `spec/`** | `KT-352, KT-355, KT-357` | 3 h | Trình Agent 0 thẩm định 6 nhóm (0 chạm `spec/`): (1) `IrqControl`/`IrqHandler` + `Notification`; (2) `Frame.MapIo` + `IOSpace` (IOMMU); (3) `Tcb.Exec(path)` + các method `VSpace`/`PageTable` (`install_at = "Chu-ky-3"`); (4) bố trí GDT `planned` (`sysret_ready = true`) cho `syscall`/`sysret` `x86_64`; (5) phương án `CNode.Mutate`/`Rotate`; (6) MCS refill đa đoạn/`SetTimeoutEp` (`D2-16`). **Chỉ mở `KT-360` sau `APPROVED` riêng của Agent 0.** |
| **KT-359** | P2 | **VER-2 & Vệ sinh Pha 3: `.scn` conformance 3 arch + Windows B2 + `v2/tools/` + đánh giá gỡ v1** | `KT-354` | 3 h | Chạy cùng bộ `.scn` conformance trên `sim` + cả 3 arch (`D2-13`); tổ chức lại `v2/tools/{checkers,build,ops}/` & tách nội bộ `check-wake-paths.py`; đánh giá Windows repro (`B2`) & lập đề xuất chính thức gỡ cây v1 đóng băng (`crates/`, `legacy/`, `deploy/`, `python-model/`, vẫn giữ nguyên vẹn tại tag `v1-archive`) khỏi `main`; T2 PASS. |

### Đợt C3-3 — Thi hành ABI v1.2, Driver Ngắt, `U1a`/`VSH2` & `U1b`/UEFI

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|
| **KT-360** | P0 | **Áp `spec/` ABI v1.2 + `vifast-gen` + `x86_64` `syscall`/`sysret` (chờ `APPROVED` A0)** | `KT-358` *(chờ APPROVED A0)* | 3 h | Áp đúng các mục đã duyệt ở `KT-358`; bật `syscall`/`sysret` (`swapgs` + per-CPU GS + GDT `planned`) trên `x86_64` (giữ hợp đồng thanh ghi `7C-r13` đã thống nhất từ `D2-242`!); sinh lại `vifast-abi`/`vifast-idl`, khoá `spec/` trở lại; T2 PASS. |
| **KT-361** | P1 | **DRV-1: Chuyển VirtIO (`blk`/`net`/`gpu`/`input`) sang IRQ `Notification` + PoC driver userland (`D2`)** | `KT-360` | 3 h | Thay busy-poll của driver VirtIO bằng `IrqControl`/`IrqHandler` + `Notification` trên cả 3 arch; chạy PoC 1 driver không đặc quyền ở U-mode dùng `Frame.MapIo` + `IrqHandler`; đột biến bỏ ack IRQ ⇒ ĐỎ; T2 PASS. |
| **KT-362** | P1 | **U1a-Full: `vifast-rt` sản phẩm + chuẩn `.vidl` + `ping`/`ifconfig` & `Tcb.Exec(path)` trong shell** | `KT-356, KT-360` | 3 h | Hoàn thiện crate `vifast-rt` (`no_std + alloc`, ổn định trên stable) + bộ `.vidl` chuẩn (`proc`/`fs`/`net`/`term`/`socket.vidl`); bổ sung lệnh `ping`/`ifconfig` thật trong `shell.rs` (đóng nợ `KT-77a`) và `Tcb.Exec(path)` nạp ELF theo đường dẫn FS (đóng nợ `KT-74`); T2 PASS. |
| **KT-363** | P1 | **VSH2: Port `vis-host-vifast` / `ViShell` (`ViScript`) lên `U1a` & kiểm chéo liên repo trên máy đo** | `KT-362` | 3 h | Biên dịch & chạy thật `ViShell` (`ViScript`) trên ảnh `product-shell` ViFastOs qua `vifast-rt` + `.vidl`; dựng script kiểm chéo định kỳ chạy cục bộ trên máy đo (`v2 nightly`, không GitHub Actions theo `D2-241`) xác nhận hợp đồng 2 repo; T2 PASS. |
| **KT-364** | P2 | **U1b & UEFI: PoC Rust `std` PAL (`*-unknown-vifastos`) + Boot UEFI (`OVMF`/`AAVMF`)** | `KT-357, KT-362` | 3 h | Chạy PoC Rust `std` (`U1b`) với target JSON & PAL tối thiểu; boot ảnh `x86_64`/`aarch64` qua UEFI (`OVMF`/`AAVMF`) đọc bảng bộ nhớ/ACPI/DTB từ firmware thật; T2 PASS. |

### Đợt C3-4 — Phần cứng thật (`HW1-Pre`) & Cổng Nghiệm thu `v2.1`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|
| **KT-365** | P2 | **HW1-Pre: PCIe MSI-X + NVMe/AHCI + `IOSpace` (`virtio-iommu`/`SMMUv3`/`VT-d`)** | `KT-357, KT-361, KT-364` | 3 h | Dựng `IOSpace` bảo vệ DMA (ca âm DMA ngoài bảng trang IOMMU bị chặn); chạy PoC PCIe MSI-X + NVMe/AHCI trên QEMU; chốt bảng kiểm mang lên phần cứng thật (`HW1`); T2 PASS. |
| **KT-366** | P0 | **Cổng nghiệm thu Chu kỳ 3 (`v2.1`): `nightly --all` (`mutants` + `bench`) + Soak 4 hart/`VSH2` + `R2-v2_1`** | `KT-350..KT-365` | 3 h | Máy đo độc quyền (`dirty=0`, `qemu_foreign=0`): T2 PASS; `v2 nightly --all` (L1 400k + Miri + `v2 mutants` thật + `vifast-bench` + `STATUS.md`) PASS; soak 4 hart U-mode + `VSH2` 3 arch 0 lỗi; xuất `v2/agent/reports/R2-v2_1.{md,json}` trình Agent 0 chốt **`v2.1`**. |
