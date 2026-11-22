# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-22 20:30 (Chu kỳ 2, Đợt 0+1+2) ===
HEAD          : `5255a773` (Chu kỳ 2; đã rebase lên A0 `db3b5eea`). Cây sạch.
CỔNG          : **T2** `gate --fast` **PASS 74/74** trên `3a5e2604` (**563,98 s**, 0 FAIL). Bước 76
                (budget x86_64) + bước 77 (`spec/kernel.toml`) mới. **CI GitHub đã BỎ** (D2-241).
SOAK          : KT-330 ĐẠT trên `5e635474` (riscv64 **0/1000**, x86_64 **0/600** ×2 CPU).
                Vòng 26–27 **đổi cả ba ảnh** ⇒ soak lại trên ứng viên cuối là việc của `KT-348`;
                lượt này chỉ chạy **smoke** 3 arch (`soak*.sh --smoke 60 3`).
AARCH64       : boot + vector + MMU + nửa cao/physmap + `VSpace` + smp 4 + TLBI + soak + budget +
                U-mode EL0 + PCI ECAM **256 bus** + virtio-blk/net/gpu/input + W^X + cache DMA.
                **`.data+.bss` 233.704 → 30.632 B (−86,98 %)** (KT-338a: arena 180.224 B ra RAM
                thật + 4 trang DMA thành frame động). Runner **27/27** (tests=35, 24/24 ca âm).
X86_64        : boot + GDT/TSS/IDT + 5 exception + APIC + PIT + ring 3 + PCI ECAM + virtio-net
                (ARP/DHCP/ICMP/DNS) + SMP 4 hart + IPI + preemption + bốn hart schedule + khoá COM1
                + priority + sleep/wake + `-cpu max` + virtio-blk + **virtio-gpu 2D** (PPM 23 kiểm)
                + **kernel W^X** + **tách ảnh sản phẩm/ktest (D2-08: 74 → 0 marker)**.
                Runner **36/36** (tests=36, 32/32 ca âm). `.data+.bss` sản phẩm **96.320 B**.
                **Nợ**: `VSpace` động + TLB shootdown (`KT-340b`, kèm cổng `.bss < 64 KiB`),
                scheduler riêng (`KT-340c` — **READY**).
RISCV64       : IPC U-mode + MR4..MR63 + net server + `Tcb.Configure` đầy đủ (kể cả **xoá**
                `vspace?`) + `Tcb.Resume` + `Tcb.Suspend` **Abort + Defer** + `NetPing` + cap
                transfer + **`IrqMcsLock` bọc `sstatus.SIE`** (KT-336c, TAP 70 khoá/ngắt).
                `ktest-ipc` **11 TAP**; TAP kernel **71**; matrix **27/27**.
TASK          : **KT-243..KT-345a DONE** (102 task). Chu kỳ 2: Đợt 0+1 xong; **`KT-333c`** (khoá máy
                đo + `gpu-x86_64.sh` vào T2 + `nightly --soak`), **`KT-335`** (net transport + TCP
                client, −42,13 %) **DONE**; **READY**: `KT-335b`, `KT-338c` (đang chạy), `KT-340b`,
                `KT-333d`; **BLOCKED**: `KT-336c` (xong), `KT-340c`/`KT-340d`/`KT-338b` theo DAG D2-244.
MÁY (sim)     : `size_of::<Machine>()` **184.792** / pin **196.608** (headroom **11.816**;
                `VspaceOutcome` **không** thêm byte nào — `khop=1 duoi_pin=1`).
HẠN           : Chu kỳ 2 theo `KE-HOACH-CHU-KY-2-2026-11-22.md`. Không mở v2.0 tới `KT-348`.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
