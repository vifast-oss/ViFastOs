# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-22 15:05 (Chu kỳ 2, Đợt 0) ===
HEAD          : `99ee94df` (Chu kỳ 2, Đợt 0+1 xong; Đợt 2 đang chạy). Cây sạch.
                `KT-330` **DONE**. Cây sạch.
CỔNG          : **T2** `gate --fast` **PASS 72/72** trên `32b60c49` (**561,4 s**, 0 FAIL).
SOAK          : KT-330 ĐẠT trên `5e635474`: riscv64 **0/1000**, x86_64 **0/600** (`qemu64`), x86_64
                **0/600** (`-cpu max`), `qemu_foreign=0` cả ba. Vòng 26–27 **đổi cả ba ảnh** ⇒ cần soak lại
                trên ứng viên cuối (`KT-348`).
                Tóm tắt: `v2/agent/JOURNAL/kt330-soak/summary.md` (D2-230: không commit raw log).
AARCH64       : boot + vector + MMU + nửa cao/physmap + `VSpace` + smp 4 + TLBI + soak + budget +
                U-mode EL0 + PCI ECAM **256 bus** + virtio-blk/net/gpu/input + W^X + cache DMA.
                **Nợ `.bss`**: 233.704/262.144 B ⇒ **KT-338a** giải phóng `BootMemory`/DMA ra RAM.
X86_64        : boot + GDT/TSS/IDT + 5 exception + APIC + PIT + ring 3 + PCI ECAM + virtio-net
                (ARP/DHCP/**ICMP**/**DNS**) + SMP 4 hart + IPI + preemption + **bốn hart schedule**
                + **khoá COM1** + **priority + sleep/wake** + `-cpu max` + virtio-blk;
                TAP **33/33**, runner **32/32**. **Nợ**: chưa có `VSpace` động/W^X/shootdown
                (**KT-340a/340b**), chưa `budget-x86_64.sh`, scheduler riêng (**KT-340c**).
RISCV64       : IPC U-mode + MR4..MR63 + net server + `Tcb.Configure` đầy đủ (kể cả **xoá**
                `vspace?`) + `Tcb.Resume` + `Tcb.Suspend` **Abort + Defer** + `NetPing` + cap
                transfer; `ktest-ipc` **11 TAP**; matrix **27/27**.
TASK          : **KT-243..KT-337 DONE** (95 task). **Chu kỳ 2**: Đợt 0 xong (`KT-330..KT-333b`),
                Đợt 1 xong (`KT-334`, `KT-336a`, `KT-336b`, `KT-337`); `KT-335` **READY** (chưa làm);
                Đợt 2 `KT-338a`/`KT-340a` **đang chạy**; `KT-344` **REVIEW** (RFC 9 nhóm) — **chờ
                Agent 0 `APPROVED`**, `KT-345a` **vẫn khoá cứng**.
ĐANG CHẠY    : `KT-338a` (aarch64 giải phóng `.bss` 233.704 B) ∥ `KT-340a` (x86_64 `budget`,
                tách ảnh sản phẩm D2-08, kernel W^X, `VSpace` động).
MÁY (sim)     : `size_of::<Machine>()` **184.792** / pin **196.608** (headroom **11.816**).
NỢ MỞ         : P20; K5' đầy đủ (`syscall`/`sysret`, virtio-gpu x86_64, MSI-X, hotplug);
                `[calling.x86_64]` **ba lỗi thật** (D2-237) chờ `KT-344`/`KT-345a`;
                `spec/kernel.toml` **chưa tồn tại**; `[object_size]` thiếu cho aarch64/x86_64;
                `VSpace`/`PageTable`/`Frame` còn `stability = "reserved"` dù **đã dùng thật**;
                driver VirtIO-PCI nhân bản ba arch (`KT-334/335/337`); `sim/src/kobj.rs` 11.443
                dòng (`KT-336b`); aarch64/x86_64 chưa nối `vifast-core` (`KT-338b`/`KT-340c`).
HẠN           : Chu kỳ 2 theo `KE-HOACH-CHU-KY-2-2026-11-22.md`. Không mở v2.0 tới `KT-348`.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
