# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-26 16:00 JST (Agent 0 D2-262/D2-263: chốt Q1..Q7 + KT-408e soak SSOT; mở đủ 4/4 slot W7b) ===
HEAD          : `08dc3bd` (mã/bằng chứng Peak 2 đang ghim `becc1756`; Lead pull sau khi xong Peak 2) — **W7a khép (`0f26bebc` T2 PASS 81/81)
                + Agent 0 `D2-262`/`D2-263`**: đồng bộ `KT-417`/`KT-413c`/`KT-408c` `DONE`, chốt `Q1..Q7`, tách chuẩn
                `KT-417b`/`417d`, `KT-420`/`420b`, `KT-423`/`423b`, mở `KT-408d` (cắt điện tất định `persist`) &
                `KT-408e` (SSOT `tap-parity.sh` + probe fail-fast `soak-{aarch64,x86_64}.sh`), chỉ thị Lead đóng điểm mù `step_l0_kernel`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`..`D2-263`,
                47 task ≤ 3 h trong dải `KT-400..KT-424`, 4 Chặng — 100 % đại tu kiến trúc, KHÔNG soak phát hành).
CỔNG          : **T2 = PASS 81/81 @ `0f26bebc`** (648,4 s). Peak 2 @ `becc1756`: `soak` riscv64 1000×3 PASS (255 s);
                `soak-aarch64` FAIL 1000/1000 (971 s) do lệch harness `TAP_TESTS` 46/47 vs 50 (không retry, đóng triệt để
                ở `KT-408e`); `L1-medium` 40k → `mutants --all` → Miri đang chạy (bằng chứng hợp lệ, tái dùng §6.3).
                `vifast-core` **137** + `vifast-ktest` **16** = **153 test host** PASS (0,18 s) — **Lead nối vào `step_l0_kernel` đầu `10:00 UTC`**.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (**`KT-417` DONE** → `KT-417b`
                `Cap` 32 B + dời `vm/` → `KT-417d` bảng VM trong `KernelStore` + xoá `[limits.plat]`/`core_foreign.rs`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + `PerHartState` (**DONE**: `KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + 6 `.vidl`/`namesrv` (`KT-420`) + `BootCapLayout` v2 bỏ 17 `EP_*` (`KT-420b`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`/`rngsrv` → `driver_files=0`,
                rồi chứng minh mở rộng bằng `nvmesrv` 0 dòng kernel ở `KT-423b`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (**DONE**: `KT-411/411b/412/413/413b/413c/415`).
TASK          : **389 task** — **DONE (374 task)** / **4 READY** / **11 BLOCKED** (Đại tu: **32/47 DONE**).
                **`READY` ngay — Đợt `W7b` (đủ 4/4 slot song song, 0 % chồng tệp, `D2-262`/`D2-263`)**:
                `KT-417b` (P0: `Cap` 56→32 B −12 KiB + dời `plat-common::obj` → `vifast-core/src/vm/` + Q6),
                `KT-418c` (P1: vá `-cpu max` `VSpace` `TF=1` + `hardening-max.sh` + `cfg(target_os="none")` `a64 fpu.rs`),
                `KT-408d` (P1: cắt điện tất định `MODE_CRASH_MID_WRITE` cho `persist.sh --crash`, Q7),
                `KT-408e` (P1: SSOT `lib/tap-parity.sh` + đồng bộ/siết TAP 47..50 + probe fail-fast `soak-{aarch64,x86_64}.sh`).
                **`BLOCKED` thuần theo DAG (11)**: `KT-417d`, `KT-417c`, `KT-419`, `KT-419b`, `KT-420`, `KT-420b`,
                `KT-421`, `KT-422`, `KT-423`, `KT-423b`, `KT-424`. **0 chờ duyệt.**
ĐANG CHẠY    : **Peak 2 (`06:00–10:00 UTC`, 0 token LLM, `D2-256`)**: `L1-medium` 40k → `v2 mutants --all` → Miri
                trên `becc1756` (KHÔNG chạy lại `nightly --soak` 1000×3 trong thời Đại tu, `D2-259`/`D2-263`).
                **Đầu khung `10:00 UTC` (0,5×)**: Lead `git pull --ff-only`, nối `-p vifast-core -p vifast-ktest` vào
                `step_l0_kernel` (+ T1 + journal), rồi mở **đủ 4/4 slot Đợt `W7b`** (`KT-417b` ∥ `KT-418c` ∥ `KT-408d` ∥ `KT-408e`).
                Sau merge W7b: xác nhận nhanh `soak-x86_64` (đóng nợ 64 dòng `# sched:` `KT-413c`) + `soak-aarch64 --smoke 2 1`,
                rồi sang `W8a` (`KT-417d` ∥ `KT-420`), `W8b` (`KT-419` ∥ `KT-417c`), `W8c` (`KT-419b`), `W8d` (`KT-420b`),
                `W9` (`KT-421` ∥ `KT-422` ∥ `KT-423`), `W9b` (`KT-423b`), `W10` (`KT-424`).
                **Ghi chú kiến trúc (`D2-261`..`D2-263`)**: ratchet `B1.1` giữ cứng (`driver_files ≤ 23` → 0);
                hạng mục #1 hậu `KT-424` là Khoá `PerHartState` + `HartSched::next` theo `KernelStore` (`store.rs:373`)
                + `HartTier2` + `ipc_fastpath` `F1`.
MÁY (sim)     : `size_of::<Machine>()` **184.712** (`Cap` 56 B → `KT-417b` đóng gói 32 B thu hồi 12.288 B) / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
