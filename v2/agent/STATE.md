# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-26 14:50 JST (W7a khép: KT-417/413c/408c DONE; T2 PASS 81/81 @ 0f26bebc) ===
HEAD          : `0f26bebc` — **W7a khép**: `KT-417` (spec ABI sạch + `[boot_caps]` + 4 invocation core) +
                `KT-413c` (gỡ 2.162 LOC `sched.rs` `x86_64` → `Machine::dispatch(hart)`) + `KT-408c` (mutants
                git-tree 7/7 KILLED + `Machine::now` riscv64 thật + boot-abi 5 `SLOT_EP_*`) + hardening F2/F3/F5.
                Phiên trước: 26 task DONE lô 1..8 `KT-400..KT-404d`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`D2-259`/`D2-260`/`D2-261`,
                42 task ≤ 3 h trong dải `KT-400..KT-424`, 4 Chặng — 100 % đại tu kiến trúc, KHÔNG soak phát hành).
CỔNG          : **T2 = PASS 81/81 @ `0f26bebc`** (648,4 s; **lượt 2**). Lượt 1 đỏ **một** bước `persist`
                (`13` bản ghi `/pstate` thay vì 11–12) = **`T-INFRA` host timing**: 4 lần chạy riêng PASS,
                boot-sau-crash + `fsck` xanh; D2-07 phân loại rồi chạy lại **một** lượt ⇒ PASS. Trước đó
                `KT-417` PASS 81/81 @ `c57895df` (688,3 s). `ktest` 9/9; `gen-check`/`pre-measure`/
                `api-boundary` xanh trên cây sạch. `bench-bkl` (`KT-415`): BKL 1/2/4 hart = 0 % / 39,6 % / 93,5 %.
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (**`KT-417` DONE** → `KT-417b` gỡ `plat-common::obj`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + `PerHartState` (**DONE**: `KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 17 `EP_*` (**`KT-417` DONE** sinh `BootCapLayout` → `KT-420` bỏ 17 `EP_*`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`/`rngsrv`) ra User Space (`KT-419/419b/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (**DONE**: `KT-411/411b/412/413/413b/415` + **`KT-413c`** gỡ hẳn `sched.rs` `x86_64`).
TASK          : **384 task** — **DONE (374 task)** / **0 READY** / **10 BLOCKED** (Đại tu: **32/42 DONE**).
                `KT-417`/`KT-413c`/`KT-408c` **DONE** (dòng `QUEUE.md` còn `READY` — Agent 0 lật khi sync;
                `agentctl done` đòi `--write` = quyền Agent 0, AGENTS §3). **Không còn task READY.**
                **`BLOCKED` thuần theo DAG (10)**: `KT-417b` (1 cây Cap/MDB — planner đo 6,5–7,5 h, đề xuất tách
                `KT-417b-1`/`417b-2`), `KT-417c` (`x86_64` `syscall`/`sysret`), `KT-418c` (VSpace isolation
                `-cpu max`, KHÔNG nới ratchet `B1.1`), `KT-419` (`Irq*` + `MapIo`), `KT-419b` (`IOSpace`),
                `KT-420..KT-424`. **Chờ Agent 0 chốt `Q1..Q7` (`BLOCKED.md` §1b) trước cửa sổ `10:00 UTC`.**
ĐANG CHẠY    : **W7a khép 05:47 UTC**. Ba nhánh merge 0 xung đột: `task/kt413c-sched` `694f9d52`
                (T1 PASS 672,9 s) → `3bf315f5`; `task/kt408c-tooling` `baec4f59` (T1 PASS 832,3 s) → `296a294f`;
                `task/kt417-hardening` `161ed302` (T1 PASS 798,4 s) → `0f26bebc`; T2 PASS 81/81 @ `0f26bebc`.
                Nhật ký: `JOURNAL/kt413c-sched-x64.md`, `JOURNAL/kt408c-tooling.md`, `JOURNAL/kt417-spec-bootcaps.md`
                (+ log bằng chứng cùng thư mục). `cargo test -p vifast-core` **137** xanh.
                **Nợ ghi nhận**: `KT-413c` — TAP 24 đổi nhãn (mất phủ preempt ring-3), soak **chưa chạy thật**
                (chỉ `--selftest 58/58`), 3 host test cũ xoá cùng logic, `-64 KiB` là `.bootstacks` (NOLOAD,
                không phải `.data+.bss`); `KT-408c` — 3 đột biến revive chưa chạy, `--all` chưa chạy lại;
                `persist` crash-injection cần **tất định** (Q7).
                **Peak 2 `06:00–10:00`** chạy nặng: `soak-x86_64` (64 dòng mới) + `mutants --all` + `L1-medium`.
                **Cửa sổ kế (`10:00 UTC`)**: W7b đề xuất `KT-417b-1` ∥ `KT-417c` ∥ `KT-418c` (chờ `Q1..Q3`).
                **Ghi chú kiến trúc (`D2-261`)**: ratchet `B1.1` giữ cứng (`driver_files ≤ 23`, `virtio-rng` đi `rngsrv` User Space ở `KT-423`);
                ngưỡng BKL S3 đã kích hoạt (93,5 % ở 4 hart) ⇒ Khoá `PerHartState` + `HartTier2` là hạng mục #1 hậu `KT-424`.
MÁY (sim)     : `size_of::<Machine>()` **184.712** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
