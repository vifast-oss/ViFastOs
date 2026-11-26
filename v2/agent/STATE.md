# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-27 01:00 JST (Lead: W7b DONE + merge `b76e3bc8`; T2 lần 1 T-INFRA; mở W8a 2 slot + 2 recon) ===
HEAD          : `b76e3bc8` — **W7b DONE & merge**: `bb3c144e` `KT-417b` (`Cap` 32 B + `vm/` vào `vifast-core`),
                `f237cd34`+`6a4a986a` `KT-418c` (hardening `-cpu max`, PAN a64, preempt CPL3), `9a962a7c`+`717f57a1`
                `KT-408d` (cắt điện tất định `persist`), `857bbd19`+`5087f283` `KT-408e` (SSOT TAP `tap-parity.sh`).
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`..`D2-263`, 47 task ≤ 3 h, 4 Chặng).
CỔNG          : **T2 lần 1 @ `b76e3bc8` = FAIL 1/81** (`# gate-steps registered=81 ran=81 skipped=0 failed=1`, 755132 ms):
                chỉ bước `QEMU virtio-gpu x86_64 + ca âm (KT-337/KT-333c)` đỏ vì TAP 33 `# net: dns error=Timeout
                elapsed_ms=6000` (slirp không trả lời) ⇒ **T-INFRA**; chạy lại **riêng bước** trên cùng cây: `0 lỗi`,
                DNS `rtt_ms=19` + đủ GPU marker. **KHÔNG tính T2 xanh** (`D2-07`); T2 xác nhận sẽ chạy trên ứng viên
                sau merge W8a. T2 trước: PASS 81/81 @ `0f26bebc`. Bằng chứng: `JOURNAL/t2-w7b-20261126/`.
KIẾN TRÚC     : (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` — `KT-417`+`KT-417b` DONE → `KT-417d` đang chạy
                (bảng VM vào `KernelStore` + `ArchVmOps` + xoá `[limits.plat]`/`core_foreign.rs`); (2) Lõi Kép
                `Machine<S: KernelStore, O: CoreObserver>` + `PerHartState` **DONE**; (3) `v2/user/` + 6 `.vidl`/`namesrv`
                (`KT-420` đang chạy) + `BootCapLayout` v2 (`KT-420b`); (4) đẩy sạch driver/dịch vụ ra User Space
                (`KT-421`..`KT-423b`); (5) `vifast-ktest` 3 arch + SMP 4 hart 3 arch **DONE**.
TASK          : **389** — **DONE 378** / **READY 3** / **BLOCKED 8** (Đại tu: **34/47 DONE**). `READY`: `KT-417d` ∥ `KT-420`
                (W8a, đang chạy slot 1-2), `KT-417c` (mở ở W8b sau `KT-417d`). `BLOCKED` thuần DAG (8): `KT-419`, `KT-419b`,
                `KT-420b`, `KT-421`, `KT-422`, `KT-423`, `KT-423b`, `KT-424`. **`QUEUE.md` còn in trạng thái cũ** — Agent 0
                cần đồng bộ; Lead không sửa QUEUE.
ĐANG CHẠY    : khung **thấp điểm** (15:45–00:50 UTC, 0,5×, `D2-256`): **W8a** `KT-417d` (slot 1) ∥ `KT-420` (slot 2) —
                2 dev-agent, đỏ-trước + T0, chờ Lead commit hộ rồi T1 trên cây sạch; 2 recon-agent (chỉ đọc) dựng brief
                W8b (`KT-419`/`KT-417c`) + W8c/W8d (`KT-419b`/`KT-420b`) tại `/tmp/briefs/`. Kế: merge W8a → T2 (máy yên)
                → W8b → W8c nếu kịp; **00:50 UTC dừng mở sub-agent mới**.
MÁY (sim)     : `size_of::<Machine>()` **172.424** B (sau `KT-417b`, `Cap` 32 B; nguồn: log T1 `kt417b-cap-vm`
                `corespace=180744 ≥ Machine 172424`) / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
