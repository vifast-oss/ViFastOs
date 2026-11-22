# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-22 22:30 (Chu kỳ 2, Đợt 0+1+2) ===
HEAD          : `daab76ef` (Chu kỳ 2). Cây sạch. `main` = **ứng viên ĐÃ QUA T2**.
CỔNG          : **T2** `gate --fast` **PASS 76/76, 0 FAIL** trên `89c8ae50` (**575,2 s**).
                Runner: riscv64 matrix **27/27**, aarch64 **32/32** (tests=35, 25 ca âm),
                x86_64 **36/36** (tests=36, exc=6). **CI GitHub đã BỎ** (D2-241).
SOAK          : KT-330 ĐẠT trên `5e635474` (số cũ). Cả ba ảnh **đã đổi** ⇒ soak **full** là việc
                của `KT-348`; lượt này chỉ smoke 3 arch (**60/60 lỗi=0** mỗi arch) trên `3a5e2604`.
AARCH64       : boot + vector + MMU + nửa cao/physmap + `VSpace` + smp 4 + TLBI + soak + budget +
                U-mode EL0 + PCI ECAM 256 bus + virtio-blk/net/gpu/input + W^X + cache DMA.
                **`.data+.bss` 233.704 → 30.632 B** (KT-338a: arena ra RAM thật, DMA động);
                **`main.rs` 1.491 → 549** + **tách sản phẩm/ktest (D2-08)** + **`IpiMailbox` SGI**
                (KT-338c): sản phẩm **0 marker**, **`.data+.bss` 0 B** (trần 65.536), ktest 26.544.
                **Nợ**: ảnh sản phẩm **chưa có** MMU/VSpace/U-mode ⇒ `KT-338b`.
X86_64        : boot + GDT/TSS/IDT + 5 exception + APIC + PIT + ring 3 + PCI ECAM + virtio-net
                (ARP/DHCP/ICMP/DNS) + SMP 4 hart + IPI + preemption + bốn hart schedule + khoá COM1
                + priority + sleep/wake + `-cpu max` + virtio-blk + **virtio-gpu 2D** (PPM 23 kiểm)
                + **kernel W^X** + tách ảnh sản phẩm/ktest (74 → 0 marker). Runner **36/36**.
                **Nợ**: `.data+.bss` sản phẩm **96.320 B > 64 KiB**, `VSpace` động + shootdown
                ⇒ `KT-340b` (`READY`), rồi `KT-340d`/`KT-340c`.
RISCV64       : IPC U-mode + MR4..MR63 + net server + `Tcb.Configure` đầy đủ (kể cả xoá
                `vspace?`) + `Tcb.Resume` + `Tcb.Suspend` Abort + Defer + `NetPing` + cap transfer
                + **`IrqMcsLock` bọc `sstatus.SIE`** (KT-336c, TAP 70) + **net transport + TCP
                client dùng chung** (KT-335, driver −42,13 %). `ktest-ipc` 11 TAP; matrix 27/27.
TASK          : **KT-243..KT-345a DONE** (104 task). Chu kỳ 2: Đợt 0+1 xong; Đợt 2 xong
                `KT-338a/c`, `KT-340a`; **READY**: `KT-335b`, `KT-340b`, `KT-333d`;
                **BLOCKED**: `KT-338b`, `KT-336c`(xong), `KT-340c/d`, `KT-342..KT-348`.
ĐANG CHẠY    : không (mọi sub-agent đã đóng; lead vừa đo cổng cuối).
MÁY (sim)     : `size_of::<Machine>()` **184.792** / pin **196.608** (headroom 11.816).
HẠN           : Chu kỳ 2 theo `KE-HOACH-CHU-KY-2-2026-11-22.md`. Không mở v2.0 tới `KT-348`.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
