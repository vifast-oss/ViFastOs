# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-22 19:15 (Chu kỳ 2, Đợt 0+1 xong) ===
HEAD          : `d0c5817c` (Chu kỳ 2; A0 đã duyệt ABI v1.1 ở `1fa7553d`). Cây sạch.
CỔNG          : **T2** `gate --fast` **PASS** (72/72 trên `32b60c49`, **561,4 s**); sau `KT-340a`
                phải đo lại (thêm bước 76 ⇒ 73 bước). **CI GitHub đã BỎ** (D2-241).
SOAK          : KT-330 ĐẠT trên `5e635474`: riscv64 **0/1000**, x86_64 **0/600** (`qemu64`),
                x86_64 **0/600** (`-cpu max`), `qemu_foreign=0` cả ba. Vòng 26–27 **đổi cả ba ảnh**
                ⇒ phải soak lại trên ứng viên cuối ở `KT-348`.
                Tóm tắt: `v2/agent/JOURNAL/kt330-soak/summary.md` (D2-230: không commit raw log).
AARCH64       : boot + vector + MMU + nửa cao/physmap + `VSpace` + smp 4 + TLBI + soak + budget +
                U-mode EL0 + PCI ECAM **256 bus** + virtio-blk/net/gpu/input + W^X + cache DMA.
                **Nợ `.bss`** 233.704/262.144 B ⇒ **`KT-338a` đang chạy**.
X86_64        : boot + GDT/TSS/IDT + 5 exception + APIC + PIT + ring 3 + PCI ECAM + virtio-net
                (ARP/DHCP/ICMP/DNS) + SMP 4 hart + IPI + preemption + bốn hart schedule + khoá COM1
                + priority + sleep/wake + `-cpu max` + virtio-blk + **virtio-gpu 2D (KT-337)**.
                **`KT-340a` xong**: `budget-x86_64.sh`, **tách ảnh sản phẩm/ktest (D2-08)**,
                **kernel W^X** (TAP 35/36); `.data+.bss` sản phẩm **96.320 B**.
                **Còn nợ**: `VSpace` động + TLB shootdown (`KT-340b`), scheduler riêng (`KT-340c`),
                `.bss` sản phẩm **< 64 KiB** là **cổng chặn** của `KT-340b`.
RISCV64       : IPC U-mode + MR4..MR63 + net server + `Tcb.Configure` đầy đủ (kể cả **xoá**
                `vspace?`) + `Tcb.Resume` + `Tcb.Suspend` **Abort + Defer** + `NetPing` + cap
                transfer; `ktest-ipc` **11 TAP**; matrix **27/27**.
TASK          : **KT-243..KT-337 DONE** (96 task). **Chu kỳ 2**: Đợt 0 xong; Đợt 1 xong
                (`KT-334/336a/336b/337`), `KT-335` **READY**; **`KT-344` DONE/APPROVED** (D2-242)
                ⇒ **`KT-345a` MỞ KHOÁ**; `KT-336c`/`KT-333c` **READY** (D2-243);
                `KT-338b/340b/340c` **BLOCKED** theo DAG.
ĐANG CHẠY    : `KT-338a` (aarch64 `.bss`) ∥ `KT-345a` (spec ABI v1.1 + `VspaceOutcome` rollback) ∥
                `KT-336c` (riscv64 `IrqMcsLock` + `Mailbox` + tài liệu `ObjStore`).
MÁY (sim)     : `size_of::<Machine>()` **184.792** / pin **196.608** (headroom **11.816**).
HẠN           : Chu kỳ 2 theo `KE-HOACH-CHU-KY-2-2026-11-22.md`. Không mở v2.0 tới `KT-348`.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
