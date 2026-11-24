# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-24 14:50 JST (D2-258, trước Đợt 3E KT-348) ===
HEAD          : `899cbcb`. Cây sạch; `.log` chưa theo dõi dưới `JOURNAL/` **0** (nợ `KT-333f` đã đóng).
CỔNG          : **T2 #7 = PASS 75/75** (`84adb4a2`); cộng thêm `KT-348a` (`da81cd4` + `a1502c4`, `guard_extsig_selftest` 7/7
                nối T0) và `KT-348b` (`0715216`, `soak-aarch64 --selftest` 48/48). Chu kỳ 2: **37/38 DONE** — còn đúng `KT-348`.
                Runner: riscv64 matrix **27/27**; aarch64 **37/37** (tests=**46**); x86_64 **37/37** (tests=**47**, gpu **48**).
SOAK          : **`KT-348a` DONE**: x86_64 **600/600** (`ok=600 fail=0 rc_124=0 rc_126=0`, gốc = tín hiệu ngoài SIGTERM +
                gia cố `lapic-window`). **`KT-348b` DONE**: aarch64 **1000/1000** (`0` lỗi, `sendkey ok=1000`, gốc = `ENOSPC` +
                HMP nối muộn + 3 `trap EXIT`). Ranh giới `D2-258`: `v2.0` soak `product-shell` thật ở `rv`, `a64`/`x64` soak `ktest`.
AARCH64       : boot + MMU + `VSpace` + smp4 + TLBI + EL0 + PCI + virtio-blk/net/gpu/input + W^X + DMA + `IpiMailbox` +
                `vifast-core::Machine` + **`product-shell`** (`KT-339a`, `.data+.bss` 696 B, `.corespace` 184.792 B, 0 marker, TAP 46);
                soak `1000/1000` PASS (`KT-348b`). Nợ: UART RX IRQ + runner shell thật + Linux `LD_PRELOAD` ⇒ `KT-357b`.
X86_64        : boot + GDT/TSS/IDT + APIC/PIT (`lapic-window` `KT-348a`) + ring 3 + PCI + virtio-net/blk/gpu + SMP4 + W^X +
                `VSpace` động + TLB shootdown + đóng v1 **N5** + `vifast-core` (`7C-r13`) + **`product-shell`** (`KT-341`,
                `.data+.bss` 51.176 B, `.corespace` 188.416 B, 0 marker, TAP 47); soak `600/600` PASS (`KT-348a`).
                Nợ: `N-7` digest `.text` P20 (`KT-353d`), W^X toàn ảnh (`KT-353a`), `SMP-1b`/`KT-340e` (`KT-351`), UART RX (`KT-357b`).
RISCV64       : `product-shell` hợp nhất FS + `netsrv` đa caller `6A` (`KT-342b`) + `exec` đọc ELF từ FS (`KT-347`, `.data+.bss`
                249.048 B) + **28/28 parity** (`KT-345b`, `smp=1,4`) + P20 `kt134-v5` (`KT-346`). Nợ: biên ELF + `.corespace` (`KT-353a`).
TASK          : **KT-243..KT-348b** DONE (124 task) — Chu kỳ 2 **37/38 DONE** (tổng 339 DONE).
                **READY duy nhất (Đợt 3E)**: **`KT-348`** (nghiệm thu `v2.0` → mở Chu kỳ 3: **26 task** `KT-350..KT-366`, `D2-258`).
                **BLOCKED**: không có (**0** task).
ĐANG CHẠY    : Không có — sẵn sàng chạy độc quyền **`KT-348`** (T2 + `nightly --all` + soak 3 arch & `virtio-blk`, `df ≥ 15 GiB`)
                trong khung **Cao điểm 2 (`06:00–10:00 UTC`, 0 token LLM, `D2-256`)**.
MÁY (sim)     : `size_of::<Machine>()` **184.792** / trần KT-312 **184.832** / pin **196.608**.
ĐĨA           : Kiểm `df ≥ 15 GiB` trước khi chạy `KT-348` (`KT-348b` phát hiện `ENOSPC` ở lượt thử 1).
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
