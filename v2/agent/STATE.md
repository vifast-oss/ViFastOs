# STATE — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.
**Lịch sử KT-01..KT-329** (vòng 1 → vòng 25) đã tách sang [`archive/STATE-chu-ky-1.md`](archive/STATE-chu-ky-1.md) — **đọc-only**, giữ nguyên văn.

Khối dưới là **nguồn trạng thái hiện tại duy nhất**: lead và sub-agent đọc/ghi **trong** khối.
Cập nhật bằng cách thay nội dung giữa hai dòng mốc; **không** đổi định dạng hai dòng mốc.

```
=== KHỐI TRẠNG THÁI HIỆN TẠI (duy nhất) — cập nhật 2026-11-25 21:30 JST (KT-409 phần 1 DONE; KT-409b + KT-410 chạy song song) ===
HEAD          : `0fbdaab6` (merge KT-409 phần 1: clippy kernel + gỡ dev-dep + TableMemory). Cây sạch; `KT-400`, `KT-401`, `KT-401b`, `KT-402`, `KT-402b`, `KT-403`, `KT-404`, `KT-405`, `KT-406`, `KT-407`, `KT-408`, `KT-414`, `KT-414b` DONE trên `main`.
KẾ HOẠCH      : **DUY NHẤT [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)** (`KT-400..KT-424` + `KT-401b/402b/414b`,
                28 task ≤ 3 h, 4 Chặng, 10 Đợt — 100 % tối ưu kiến trúc, KHÔNG mục tiêu phát hành hay soak phát hành).
CỔNG          : **T2 = PASS 79/79 tại `f3cf3c97`** (4 task Thấp điểm chính + merge fix, 647,3 s, 2026-11-25);
                trước đó **T2 = PASS 75/75 tại `a61921b4`** (674,9 s), `26287331` (640,5 s), `bdfe97dd` (788,5 s), `d39b86b9` (757,2 s), `6c1accd5` (621,8 s) và `6cf4640c` (W1).
                `KT-348a` (x64 600/600, `guard_extsig_selftest` 7/7 nối T0) & `KT-348b` (a64 1000/1000, 48/48) `DONE` tại `899cbcb`.
                Từ `D2-259`: không soak phát hành; kiểm định theo T0/T1/T2 + `L1-medium` + Miri + `v2 mutants` + `vifast-bench`.
                **Lô Cao điểm 2 (2026-11-25, 0 token LLM)**: `L1-medium` **PASS** (40k seed, 682 s) + Miri **PASS**
                (384 test, 7.211 s) + `vifast-bench` smoke **PASS** (`build_id=b090cfd83fe82db3`) tại `19f0a4c3`
                — [`JOURNAL/cao-diem-2-20261125.md`](JOURNAL/cao-diem-2-20261125.md).
KIẾN TRÚC     : Đích 5 tầng: (1) 1 Cây Capability/MDB duy nhất trong `vifast-core` (gỡ `plat-common::obj` ở `KT-417`);
                (2) Lõi Kép `Machine<S: KernelStore, O: CoreObserver>` + tách `PerHartState` (`KT-405/408/410`);
                (3) `v2/user/` Cargo workspace thật + `BootCapLayout` bỏ 16 `EP_*` (`KT-404/420`);
                (4) Đẩy sạch driver/dịch vụ (`netsrv`/`blksrv`/`fssrv`/`gpusrv`/`inputsrv`) ra User Space (`KT-403/419/421..423`);
                (5) `vifast-ktest` dùng chung 3 arch + SMP 4 hart 3 arch (`KT-411/412/413/415`) + gỡ `vifast-model` (`KT-410`).
TASK          : **371 task / 356 DONE / 2 READY / 13 BLOCKED** — `KT-243..KT-348b` đã khép (126 task).
                Đã `DONE` (9/29): `KT-400`, `KT-402`, `KT-403`, `KT-406`, `KT-402b`, `KT-404`, `KT-408`, `KT-414b`
                (lô Thấp điểm chính 1, T2 PASS 79/79 @ `f3cf3c97`) + **`KT-409` phần 1** (`0fbdaab6`: bước 84 clippy kernel
                host+3 target, gỡ dev-dep ngược, `TableMemory` lên `vifast-pte`; phần 2 tách `KT-409b`).
                **`READY` (2 task)**: `KT-409b` (HAL phần 2: Sv39/PLIC + trait dùng chung + cắt LOC), `KT-410` (`UntypedStore` + `BootInfo` Arena + `SimHeapStore` + gỡ `vifast-model`).
                **`BLOCKED` theo DAG (13 task)**: `KT-411..KT-413`, `KT-415..KT-424`.
ĐANG CHẠY    : **2 slot song song** — `KT-409b` (slot 1, `task/kt409b-hal-r2`) + `KT-410` (slot 2,
                `task/kt410-general-store`), base `0fbdaab6`; sub-agent chỉ T0+T1, Lead merge + T2.
                Lô 1 (4 task) đã khép: T2 **PASS 79/79** @ `f3cf3c97` (647,3 s) + `L1-medium` **PASS** (24 lô, 167 s);
                T2 lần 1 bắt đúng 1 lỗi gộp (`forbid(unsafe_code)` × `link_section`) ⇒ merge fix kho ring sang `vifast-sync`.
                Nhật ký lô: `JOURNAL/thap-diem-chinh-20261125.md`; khoá T1 `/tmp/vifast-t1-lock.sh` (1 T1/máy).
                Ghi chú: `v2 mutants --all` 2026-11-25 (17 KILLED / 7 E_WRONG-REASON — `JOURNAL/kt403-mutants-all.md`);
                Miri đã chạy đủ tập chọn tại `19f0a4c3`; target UEFI `1.100.0` đã cài.
                Nợ công cụ: `vifast-bench` thiếu `~/.cargo/bin` trong `PATH`; manifest nightly bị SIGTERM thiếu `ended`.
MÁY (sim)     : `size_of::<Machine>()` **184.712** / trần KT-312 **184.832** / pin **196.608**.
=== HẾT KHỐI HIỆN TẠI — mọi mục dưới đây là LỊCH SỬ ===
```
