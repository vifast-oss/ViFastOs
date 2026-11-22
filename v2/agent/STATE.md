# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-22 15:05 (Chu kỳ 2, Đợt 0) ===
HEAD          : `ee8f2ca2` (Agent 0: **D2-236..D2-240**, `KT-330..KT-348` thành **25 task ≤ 3 h**) +
                `KT-330` **DONE**. Cây sạch.
CỔNG          : **T2** `gate --fast` **PASS 68/68** trên ứng viên vòng 25.
SOAK          : **KT-330 ĐẠT** — riscv64 **0/1000**, x86_64 **0/600** (`qemu64`), x86_64 **0/600**
                (`-cpu max`); mỗi đợt `qemu_foreign=0` hai mẫu, `invalid=0`, `rc_124=0`.
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
TASK          : **KT-243..KT-330 DONE** (88 task). **Chu kỳ 2**: `KT-331..KT-333`, `KT-333a/b`,
                `KT-344` **READY**; `KT-334..KT-348` **BLOCKED** theo DAG. **`KT-345a` khoá cứng**
                — chỉ Agent 0 mở sau `APPROVED` cho `KT-344`.
ĐANG CHẠY    : **KT-344** (RFC ABI v1.1, **9 nhóm** theo D2-237) — nhánh `task/kt344-rfcabi`,
                trạng thái đích **REVIEW**.
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
