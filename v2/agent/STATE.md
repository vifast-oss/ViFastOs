# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-23 08:20 (đợt 3A, 4 slot) ===
HEAD          : `7494aaeb`. Cây sạch; `.log` chưa theo dõi dưới `JOURNAL/` **0** (nợ `KT-333f` đã đóng: tag `cycle1-raw-logs-untracked`).
CỔNG          : **T2 #5 = PASS 75/75** (`registered=75 ran=75 skipped=0 failed=0`, 649.221 ms) trên `7494aaeb`, máy rảnh `qemu_foreign=0` — **ứng viên tích hợp hợp lệ** cho `v2.0`.
                Runner hiện tại: riscv64 matrix **27/27**; aarch64 **37/37** (tests=**46**, ca âm 27/27);
                x86_64 **37/37** (tests=**47**, exc=6, ca âm **32/32**, gpu **48**). **CI GitHub đã BỎ** (D2-241).
SOAK          : `nightly --soak` trên `d5f84596`: riscv64 **1000/1000**, aarch64 **1000/1000**,
                x86_64 **599/600** ⇒ bắt lỗi **thật** DNS `BadFrame` ⇒ **D2-246** (`KT-335c`).
                Ba ảnh **đã đổi tiếp** (`KT-338b/339/340b/340d`) ⇒ soak đầy đủ lại ở `KT-348`.
AARCH64       : boot + vector + MMU + nửa cao + `VSpace` + smp4 + TLBI + soak + U-mode EL0 + PCI +
                virtio-blk/net/gpu/input + W^X + cache DMA + `IpiMailbox`; **sản phẩm `.data+.bss` 0 B**;
                `main.rs` **549**; **`vifast-core::Machine` + IPC EL0** (`KT-338b`, TAP 41);
                **`procsrv` + `shell` cùng nguồn EL0** (`KT-339`, TAP 44). Nợ: sản phẩm **chưa** link
                core/`procsrv` (ngân sách) ⇒ `KT-341`.
X86_64        : boot + GDT/TSS/IDT + 5 exception + APIC/PIT + ring 3 + PCI ECAM + virtio-net +
                SMP4 + IPI + preemption + COM1 lock + priority + sleep/wake + `-cpu max` + virtio-blk
                + virtio-gpu 2D + **W^X** + tách ảnh sản phẩm/ktest; **`.data+.bss` sản phẩm
                96.320 → 43.072 B** (`KT-340b`); **`main.rs` 1.500 → 1.004**; **`VSpace` động**;
                **TLB shootdown đa hart + `IpiMailbox`** + đóng v1 **N5** (`KT-340d`, TAP 37).
                Nợ: **`vifast-core` + `7C-r13` + gỡ scheduler tự viết** ⇒ `KT-340c` (đang chạy).
RISCV64       : IPC U-mode + MR4..MR63 + net server + `Tcb.Configure/Resume/Suspend` + `NetPing` +
                cap transfer + `IrqMcsLock` (`KT-336c`) + chính sách mạng chung (`KT-335b`).
                Nợ: **ảnh sản phẩm hợp nhất FS+netsrv+exec** ⇒ `KT-342` (đang chạy).
TASK          : (336 task) **KT-243..KT-345a** xong; đêm nay thêm **KT-338c, KT-335c, KT-338b, KT-340b,
                KT-333d, KT-335b, KT-339, KT-340d, KT-333e, KT-333f**; **đợt 3A**: `KT-343` (parity **25/28**),
                `KT-339a` (aarch64 product-shell, TAP 46), `KT-341`+`b` (x86_64 product-shell 51.176 B, TAP 47),
                `KT-342b` (`user.rs` 704 dòng, gốc rễ `NBSend`), và **`KT-340c` / `KT-342` / `KT-342a`** đã DONE.
                **READY kế**: `KT-345b` (28/28 — phụ thuộc KT-342b+KT-343), `KT-346` (v2/user + P20), `KT-347`, `KT-347b`;
                **`KT-348`** = nghiệm thu máy độc quyền (soak 3 arch + `nightly --all`).
ĐANG CHẠY    : `KT-340c` (x86_64 core/7C-r13, slot 4 ĐƯỜNG GĂNG), `KT-342` (riscv64 sản phẩm).
MÁY (sim)     : `size_of::<Machine>()` **184.792** / pin **196.608** (headroom 11.816).
ĐĨA           : **40 GiB** trống (đã dọn 75 worktree cũ + tag `cycle1-raw-logs`).
HẠN           : **09:00 sáng 2026-11-23 JST** (D2-247/D2-248). Cắt mở task mới **08:25**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
