# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; bổ sung **D2-236..D2-240** tại `5e635474`; duyệt `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **tối ưu quy trình & tái tổ chức kho D2-244..D2-245** tại `bfb9a13d` sau khi `KT-345a` ABI v1.1 + 14/31 task đã `DONE`).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-245**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md).

> **Mục tiêu một câu**: Khép soak vòng 25 (`KT-330`), chấm dứt trạng thái "mỗi kiến trúc một nhánh rời / mỗi tính năng một `ktest-*` riêng" — **khử nhân bản driver trên 3 kiến trúc** (`v2/lib/vifast-virtio` + `vifast-net`), **giải nghẽn `.bss` & đóng lỗi v1 N5 + Audit Q1-A 2 tầng (`IpiMailbox` + `IrqMcsLock` đủ 3 port)**, **nối `vifast-core` + `shell` dùng chung lên `aarch64` và `x86_64`**, **hợp nhất ảnh sản phẩm `riscv64`** (FS + Net + Exec, đạt **28/28** lệnh parity v1), **áp xong ABI v1.1 (`KT-345a` `DONE`)**, **tối ưu quy trình Agent 2 & dọn kho tiền `v2.0` (`KT-333c/d`, `KT-347b`)**, và chốt khung **Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`**.

---

## 0. Chẩn đoán đầu vào (căn cứ từ mã nguồn `5e635474`, cập nhật thực thi tại `bfb9a13d`)

Chu kỳ 1 (`KT-01..KT-329`, 25 vòng đã hoàn tất) đạt kỷ luật kiểm chứng rất cao (T2 `gate --fast` PASS 68/68, soak 3 arch `9e18880d` 0 lỗi, K6 đóng, luật `wake` đóng, 3 arch đều boot thật trên QEMU). Đối chiếu trực tiếp `Cargo.toml`, `spec/abi.toml`, cây nguồn và cổng đo lộ ra **10 điểm nghẽn cấu trúc** (7 gốc ở D2-229 + 3 phát hiện bổ sung ở D2-236..D2-238; đến `bfb9a13d` đã đóng các mục 3, 7, 8, 10 và phần lớn mục 2, 6):

1. **`aarch64` (K4') và `x86_64` (K5') chưa nối `vifast-core`**: `vifast-kbin-aarch64` và `vifast-kbin-x86_64` chưa phụ thuộc `vifast-core`, `vifast-elf`, `vifast-vfs`, `vifast-fs`; `x86_64` đang dùng bộ lập lịch độc lập tách rời `vifast-core::Machine`. (`KT-340a` đã tách ảnh sản phẩm vs `ktest-x86_64`; `aarch64` tách ở `KT-338c`.)
2. **Nghẽn toán học `.data+.bss ≤ 256 KiB` khi nối `vifast-core` (D2-236/D2-243/D2-244)**: `aarch64` đã giải xong ở `KT-338a` (**233.704 → 30.632 B**); trên `x86_64`, `KT-340a` hạ ảnh sản phẩm từ 177.440 → **96.320 B** (còn asm `.bss` ở `main.rs` 81.920 B + DMA pages) — cộng `Machine` (`184.792 B`) sẽ vượt 262.144 B nên `KT-340b` bắt buộc hạ `< 64 KiB` trước khi link `Machine` ở `KT-340c`.
3. **Xung đột thanh ghi trong `[calling.x86_64]` & thiếu mục đa arch ở `spec/abi.toml` (D2-237/D2-242)**: **ĐÃ ĐÓNG** 9/9 nhóm tại **`KT-345a`** (`7C-r13`, `spec/kernel.toml`, `[ipc_buffer]`, `VspaceOutcome`, `[object_size]` alias, 3 `.vidl`, `Frame` frozen / `VSpace`+`PageTable` core; `spec/` khoá lại).
4. **`x86_64` thiếu `VSpace` động + TLB shootdown & lỗi v1 N5 chưa đóng (D2-238/D2-244)**: `KT-340a` đã xong kernel W^X + `budget-x86_64.sh` + tách sản phẩm/ktest; còn `VSpace` động (`KT-340b`) và TLB shootdown + đóng lỗi v1 **N5** (`KT-340d`).
5. **Phân mảnh 14 binary trên `riscv64`**: `vifast-kbin-riscv64` có 13 binary `ktest-*`; các feature `net`/`gpu`/`input`/`vcon` của `vifast-plat-riscv64` vẫn kéo theo `ktest`; ảnh sản phẩm `product-fs` chưa có `exec`/`netsrv`/mạng và còn 14/28 lệnh `deferred-K7` trong `spec/v1-parity.toml` (đóng qua `KT-342`/`KT-343`/`KT-345b`; gom 14 bin ở Pha 3).
6. **Nhân bản driver lớn & crate mồ côi (`vifast-sync`, `vifast-objmem`) + bẫy `SIM_KOBJ`**: `KT-334`/`KT-337` đã rút `pci`/`virtio_blk`/`virtio_gpu`/`virtio_input` về `v2/lib/vifast-virtio`; `KT-336b` tách `kobj.rs` (11.964→1.030 dòng); `KT-336a`/`KT-336c` đóng `IrqMcsLock` trên `x86_64` & `riscv64` + §28 `ObjStore`. Còn lại: `KT-335` (`virtio_net` + TCP) và nối `IpiMailbox` trên `aarch64` (`KT-338c`) + `x86_64` (`KT-340d`) để đóng tầng 1 Audit Q1-A (`D2-244`).
7. **Khép soak vòng 25 + `-cpu max` (`KT-330` `DONE`)**: đã đạt 0 lỗi trên `5e635474` (`riscv64` 0/1000, `x86_64` 0/600 `qemu64`, `x86_64` 0/600 `-cpu max`, `qemu_foreign=0`).
8. **Cổng T2 phình thời gian, `v2 nightly`, `deny.toml` & quy trình Agent 2**: `KT-333a`/`KT-333b` đã hạ T2 xuống **~564 s (PASS 74/74)** và dựng `v2 nightly` + `deny.toml` + `THIRD-PARTY.md`; **`D2-241`** cấm vĩnh viễn GitHub Actions (`AGENTS.md` §4.11); **`KT-333c`** + **`KT-333d`** hoàn thiện lock máy đo, tách `v2/bin/v2`, pre-flight, `wt.sh`, `agentctl lint`, `mktemp -d`.
9. **Build userland qua `build.rs` + `include_bytes!` không mở rộng sang `U1`/`VSH2`** và là gốc nợ P20/A1-184 (đóng tại `KT-346`).
10. **Vệ sinh repo, sổ sách & tương thích đa nền tảng (`KT-331`/`KT-332` `DONE`, `KT-347b` tiền `v2.0`)**: guard NTFS đã vào `v2 rules` + pre-commit; `STATE.md`/`BLOCKED.md`/`README.md` đã cắt gọn; MSRV `1.100.0` (`KT-334`); **`KT-347b`** sẽ dọn ~8.000 raw `.log` Chu kỳ 1 + phiếu cũ + rút `soak`/`budget` chung + 1-pass `wake` ngay trước `KT-348`.

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229** | **Mở Chu kỳ 2 (`KT-330..KT-348`), sửa 4 tệp lỗi Windows & đối soát `QUEUE.md`** | Đổi tên 4 tệp lỗi NTFS (giữ 100% blob), đối soát 7 dòng `DONE` sót vòng 17/21; **giữ nguyên mọi ngưỡng cứng** (`.data+.bss ≤ 256 KiB`, `Machine ≤ 196.608 B`, `R1 ≤ 1.500`, `D2-07`, `D2-08`, `D2-17`); **chưa mở `v2.0`**. |
| **D2-230** | **Chính sách nhật ký `JOURNAL/` & phòng chống tên tệp lỗi NTFS** | Cấm commit raw `run-soak-*/boot-*.log` kể từ Chu kỳ 2; chỉ commit tóm tắt (`summary.txt`, bảng chứng cứ, sha256, `build-inputs`, log mẫu lỗi nếu đỏ). Bắt buộc kiểm tên tệp NTFS ở `git-guard` và `v2 check` (`KT-331`). |
| **D2-231** | **Hợp nhất tầng driver VirtIO-PCI & mạng dùng chung 3 arch** | Rút logic lặp của `pci`, `virtio_blk`, `virtio_net`, `virtio_gpu`, `virtio_input` về lớp transport thuần (`MmioBus` + `DmaPool`, `forbid(unsafe_code)`); giữ adapter `unsafe` tối giản ở `vifast-plat-*` (D2-17). Đưa state machine TCP/client về `v2/lib/vifast-net`; xoá 2 bản `tcp.rs` nhân bản. |
| **D2-232** | **Đóng trạng thái "crate mồ côi" (`vifast-sync`, `vifast-objmem`) & áp R1 cho `sim/src/`** | Nối `vifast-sync` vào `v2/kernel`; định rõ trait `ObjStore` & đường chuyển `slotmem`→`objmem`; tách `v2/sim/vifast-sim/src/kobj.rs` về ≤ 1.500 dòng/tệp và mở R1 phủ `v2/sim/vifast-sim/src/`. |
| **D2-233** | **Một lõi `vifast-core` & một userland `vifast-plat-user` cho cả 3 kiến trúc** | **Cấm** phát triển thêm scheduler riêng ngoài `vifast-core`. `aarch64` và `x86_64` tách ảnh sản phẩm vs harness (`D2-08`), nối `vifast-core`, chạy chung 1 codebase `procsrv`/`shell` qua `vifast-plat-user`. Hợp nhất ảnh sản phẩm `riscv64` (FS+`netsrv`+`exec`) và tách build userland sang `v2/user/`. |
| **D2-234** | **Một cửa sổ duy nhất cho ABI v1.1 (`KT-344` → `APPROVED` → `KT-345a`)** | Cấm sửa `spec/abi.toml` ở mọi task Chu kỳ 2 trừ **`KT-345a`** (riêng `spec/v1-parity.toml` được cập nhật trạng thái lệnh không đổi ABI ở `KT-343`/`KT-345b`). Chỉ mở `KT-345a` sau dòng `APPROVED <ngày> Agent 0` cho `KT-344` trong `QUEUE.md`. |
| **D2-235** | **Quy chế điều phối song song & lật sổ Chu kỳ 2** | Tối đa **4 worktree song song** (D2-198); sub-agent commit trên nhánh `task/kt<n>-<tên>`, chỉ Agent 1 (lead) merge/push `main`. Lead được lật `BLOCKED` → `READY` theo DAG **trừ `KT-345a`** (chỉ Agent 0 mở). `KT-330` và `KT-348` chạy **độc quyền** (`qemu_foreign=0`). |
| **D2-236** | **Giải nghẽn `.bss` 3 arch trước khi nối `vifast-core` & thêm `budget-x86_64.sh`** | `KT-338a` dời `BootMemory::ARENA` (180 KiB) + 4 trang DMA `aarch64` ra RAM ngoài `.bss` (< 64 KiB trước khi link `Machine`); `KT-340a` tách 64 KiB `TaskStack` test khỏi ảnh sản phẩm `x86_64` và thêm **`budget-x86_64.sh`** (`e_machine = 62`, trần 262.144 B) vào T2. |
| **D2-237** | **Mở rộng `KT-344` lên 9 nhóm & đẩy `KT-345a` (spec/gen) lên TRƯỚC `KT-338b`/`KT-340c`** | Sửa xung đột `rdx`/`r12` ở `[calling.x86_64]`, bổ sung `[debug_info].return_channel` + `[object_size]` 3 arch, nâng trạng thái `VSpace`/`PageTable`/`Frame`. Tách `KT-345a` (spec/gen — tiền đề của `KT-338b`/`KT-340c`) và `KT-345b` (`free`/`df`/`loop` + `NetSvc` đa caller). |
| **D2-238** | **Chèn `KT-340a`/`KT-340b` (`VSpace`/W^X/shootdown + đóng v1 `N5`), tách task > 3 h & sửa bẫy `KT-336`/DAG `KT-348`** | Tách `KT-333a/b`, `KT-336a/b`, `KT-338a/b`, `KT-340a/b/c` để mọi task ≤ 3 h (`AGENTS.md` §10). Bắt buộc bọc lưu/tắt IRQ khi dùng `McsLock` cho tài nguyên đụng ngắt; nối `vifast_sync::Mailbox` đủ 3 port để đóng thật sự Audit Q1-A (`N7`); đồng bộ `SIM_KOBJ` trong checker `wake`; vá đủ DAG `KT-348`; lật `KT-330` → `READY`. |
| **D2-239** | **Chốt chi tiết `KT-334` (`vifast-virtio` + cache DMA), `KT-343` (25→28/28), `KT-346` (`VBREV_ALGO`) & soak `KT-348`** | Đặt `MmioBus`/`DmaPool` (có `sync_for_device/cpu` bảo toàn `dc civac` `aarch64`) trong `v2/lib/vifast-virtio`; `KT-343` lật 11 lệnh (gồm `run`/`runfs`/`exec` → 25/28; `KT-345b` → 28/28); `KT-346` thêm `v2/user` vào `build-rev.sh` + tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `wake_scope.py` + kiểm tái lập 3 arch Linux/macOS; `KT-348` soak **ảnh sản phẩm có shell** 3 arch + đợt **soak đĩa thật `virtio-blk`**. |
| **D2-240** | **Khung lộ trình dài hạn Chu kỳ 3 & Bảng Exit Criteria phát hành `v2.0` vs `v2.1`** | Chốt 7 trụ cột Chu kỳ 3 (`SMP-1/2`, `MEM-1/2`, nền kiểm chứng D2-09/mutants/`vifast-bench`/conformance 3 arch, IRQ `Notification` → IOMMU, FPU/SIMD + `copy_from_user` + `.vidl`, `U1a`/`U1b` + liên repo `ViScript`, DTB/ACPI cho `HW1` + đóng băng parser Python) và tiêu chí tường minh mở `v2.0` (§6). |
| **D2-241** | **BỎ GitHub Actions, vĩnh viễn** *(chỉ đạo người điều phối)* | Xoá `.github/` vĩnh viễn; ghi luật cấm vào `AGENTS.md` §4.11 & `v2/AGENTS.md` §4. Mọi cổng và kiểm định kỳ (gồm liên repo `ViScript`) chạy bằng **script + bộ hẹn giờ trên máy đo**, không dùng dịch vụ CI ngoài. |
| **D2-242** | **Duyệt Gói ABI v1.1 (`KT-344` `APPROVED`) & mở khoá `KT-345a`** | Chấp thuận **1C, 2C, 3.1A+3.2A, 4C, 5A, 6A, 8C, 9B** và tinh chỉnh Nhóm 7 thành **`7C-r13`** (`return_message_info = "r13"` — một hợp đồng thanh ghi thống nhất cho cả `int 0x80` lẫn `syscall`/`sysret` sau này); giữ **`abi_version = 1`, `min_compat = 1`**; `KT-345a` làm luôn `VspaceOutcome` + rollback `TcbData.vspace` *(đã `DONE` 9/9 nhóm)*. |
| **D2-243** | **Tinh chỉnh Chu kỳ 2 sau Đợt 0–1 (`KT-336c`, `KT-333c`, `.bss < 64 KiB` `x86_64`, đồng bộ `D2-241`)** | Bổ sung **`KT-336c`** (`DONE`, `riscv64` `IrqMcsLock` + `k3b-core.md` `ObjStore`), **`KT-333c`** (`READY`, khoá máy đo + `gpu-x86_64` T2 + `nightly --soak`); chặn cứng ảnh sản phẩm `x86_64` `.data+.bss < 64 KiB` trước `KT-340c`; đồng bộ `D2-241`. |
| **D2-244** | **Hiệu đính DAG Đợt 2 (`KT-338c`, `KT-340b`/`d`), kiến trúc 2 tầng Audit Q1-A & chốt `v2 mutants`** | Tách `KT-340b` (cũ) → `KT-340b` (`READY`, cắt `main.rs` + `.bss < 64 KiB` + `VSpace` động) & `KT-340d` (shootdown + `IpiMailbox` + đóng N5); tách `KT-338c` (`READY`, cắt `main.rs` + tách product/ktest `aarch64` + `IpiMailbox`) trước `KT-338b`; đóng Audit Q1-A (`N7`) tầng 1 bằng **`IpiMailbox` + `IrqMcsLock` đủ 3/3 port** (`Mailbox<N>` cho `SMP-1` Chu kỳ 3); xác nhận `v2 mutants` ở Chu kỳ 3 (`v2.1`). |
| **D2-245** | **Tối ưu quy trình Agent 2 & tái tổ chức kho mã nguồn 3 pha (`KT-333d`, `KT-347b`)** | Ban hành quy chế **Sub-agent T0+T1 / Lead T2 gộp lô + cấm "DONE một phần" + 4 slot worktree cố định + siết `JOURNAL/`**; thêm **`KT-333d`** (tách `v2/bin/v2` sang `v2/bin/lib/*.sh`, pre-flight fail-fast, `wt.sh`, `agentctl lint`, `mktemp -d`) và **`KT-347b`** (rút `soak`/`budget` chung, 1-pass `wake`, gom phiếu cũ, tách lưu trữ ~8.000 raw `.log` Chu kỳ 1). |

---

## 2. Lộ trình 4 đợt & đồ thị phụ thuộc (`KT-330..KT-348`, 31 task ≤ 3 h)

```text
[ĐỢT 0 + ĐỢT 1 + ABI v1.1 — 14/31 DONE (T2 74/74 PASS 563,98 s tại 3a5e2604)]
  DONE: KT-330, KT-331, KT-332, KT-333a*, KT-333b, KT-334, KT-336a, KT-336b, KT-336c,
        KT-337, KT-338a, KT-340a, KT-344 (APPROVED), KT-345a (ABI v1.1 9/9 — spec/ khoá lại)
        │
        ▼ (4 SLOT SONG SONG READY NGAY — 0 CHỒNG CHÉO TỆP)
  ├── [Slot 1 — Hạ tầng cổng & Quy trình Agent 2 (ngoài v2/kernel & v2/lib)]
  │     KT-333c [READY] (lock máy đo + gpu-x86_64 T2 + nightly --soak)
  │       └──► KT-333d (tách v2/bin/v2 → lib/*.sh + pre-flight + wt.sh + agentctl lint + mktemp -d)
  │
  ├── [Slot 2 — Mạng dùng chung & Hợp nhất sản phẩm riscv64]
  │     KT-335  [READY] (virtio_net 3 arch + TCP → vifast-net)
  │       └──► KT-342 (cắt user.rs < 1.100 + riscv64 sản phẩm FS+netsrv+exec)
  │              └──► KT-343 (25/28 parity) ──► KT-345b (28/28 parity + NetSvc đa caller 6A)
  │
  ├── [Slot 3 — aarch64: product/ktest + lõi vifast-core + shell chung]
  │     KT-338c [READY] (cắt main.rs < 1.100 + tách product/ktest D2-08 + IpiMailbox)
  │       └──► KT-338b (aarch64 vifast-core + IPC EL0) ──► KT-339 (vifast-plat-user + aarch64 shell)
  │
  └── [Slot 4 — x86_64: .bss < 64 KiB + VSpace + đóng N5 + lõi vifast-core + shell chung]
        KT-340b [READY] (cắt main.rs < 1.100 + .bss sản phẩm < 64 KiB + VSpace động)
          └──► KT-340d (TLB shootdown AP→BSP qua IpiMailbox + đóng N5 PCID)
                 └──► KT-340c (x86_64 vifast-core + IPC r13)
                        └──► (KT-339, KT-340c) ──► KT-341 (x86_64 shell chung)
        │
        ▼
[ĐỢT 3 — Tách Userland v2/user/, Dọn kho tiền v2.0 & Nghiệm thu Chu kỳ 2]
  ├── (KT-341, KT-342)           ──► KT-346  (tách v2/user/ + VBREV_ALGO kt134-v5 + đóng P20 3 arch)
  │                                    └──► KT-347  (PoC U1a + hợp đồng .vidl ViScript + UEFI/DTB/ACPI)
  ├── (KT-333d, KT-345b, KT-346) ──► KT-347b (rút soak/budget chung + 1-pass wake + gom phiếu cũ + archive raw .log)
  └── (KT-331, KT-332, KT-333a..d, KT-336b/c, KT-337, KT-341, KT-343, KT-345b, KT-346, KT-347b)
                                 ──► KT-348  (Cổng nghiệm thu Chu kỳ 2: soak sản phẩm 3 arch + đĩa thật
                                              + T2 + nightly --all + Exit Criteria v2.0 + báo cáo R2-v2)
```

---

## 3. Chi tiết từng đợt & bảng task (mọi task ≤ 3 h)

### Đợt 0 — Khép vòng 25, vệ sinh, hạ tầng cổng & Quy trình Agent 2

> **Luật thực thi Đợt 0**: `KT-331`, `KT-332`, `KT-333a..d`, `KT-344` **không** sửa `v2/kernel/**` hay `v2/lib/**` (không làm bẩn `build-inputs`).

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-330** | P0 | **Soak ứng viên cuối vòng 25**: `riscv64` 1.000 + `x86_64` 600 ×2 CPU | `KT-328, KT-329` | 3 h | `DONE` | 0 lỗi cả 3 đợt soak trên `5e635474` (`qemu_foreign=0`, không retry); tóm tắt `JOURNAL/kt330-soak/summary.md`. |
| **KT-331** | P0 | **Guard tên tệp Windows/NTFS + chính sách `JOURNAL/`** | — | 2 h | `DONE` | `check-filenames.py` (5 mã lỗi NTFS, selftest 23/23) nối `v2 rules` + pre-commit; `.gitignore` chặn raw soak log. |
| **KT-332** | P0 | **Đồng bộ tài liệu công khai, cắt gọn `STATE.md` & dọn `BLOCKED.md`** | — | 2 h | `DONE` | `README.md`/`v2/README.md` đồng bộ; `STATE.md` 335,8→3,6 KiB; `BLOCKED.md` dọn 43 mục đóng. |
| **KT-333a** | P1 | **Profile & hạ wall-time T2 (`gate --fast`)** | — | 3 h | `DONE` | T2 hạ từ 766–828 s → **558,9–563,98 s** (`PASS 74/74`); ~~GitHub Actions~~ **BỎ vĩnh viễn theo D2-241**. |
| **KT-333b** | P1 | **Dựng `v2 nightly` thật + `deny.toml` + `THIRD-PARTY.md` (D2-21)** | — | 3 h | `DONE` | `v2 nightly` thật (selftest 14/14, sinh `STATUS.md`); `deny.toml` + `v2 licenses` (9/9) + `THIRD-PARTY.md`. |
| **KT-333c** | P1 | **Khoá độc quyền máy đo (`v2/bin/v2` & `soak*.sh`) + nối `gpu-x86_64.sh` vào T2 + chuẩn bị `v2 nightly --soak`** | `KT-333a, KT-333b, KT-337` | 2 h | `READY` | **(D2-243)** Lockfile PID nguyên tử fail-closed cho `gate --fast`/`nightly`/`soak*.sh`; nối `gpu-x86_64.sh` (+ ca âm không GPU) vào làn `sau6` T2; mở rộng `v2 nightly --soak` cho sản phẩm + đĩa thật `KT-348`; T2 PASS. |
| **KT-333d** | P1 | **Mô-đun hoá `v2/bin/v2` + pre-flight fail-fast + `wt.sh` (4 slot) + `agentctl lint` + cô lập `$TMPDIR`** | `KT-333c` | 2 h | `BLOCKED` | **(D2-245)** Tách `v2/bin/v2` (1.919 dòng) thành dispatcher + `v2/bin/lib/{lock,lanes,check,gate,nightly}.sh`; pre-flight < 1 s (dirty / `df < 5 GiB`); `v2/tools/wt.sh` quản 4 slot worktree cố định; **`vifast-agentctl lint`** (bắt `BLOCKED` đủ phụ thuộc `DONE`, vòng lặp, mã task sai mẫu `commit-msg`, lệch `STATE.md`, nối `v2 check`); đổi `$TMPDIR/k5-x86_64` sang `mktemp -d`; T2 PASS. |
| **KT-344** | P1 | **RFC Gói duyệt ABI v1.1 (`review/agent0/GOI-ABI-V1_1.md`) — 9 nhóm** | — | 3 h | `DONE` | **`APPROVED 2026-11-22 Agent 0 (D2-242)`**: 9 nhóm, 21 phương án; duyệt 1C..9B + **`7C-r13`**, `abi_version = 1`. |

### Đợt 1 — Khử nhân bản driver, hợp nhất tầng dùng chung & Áp `spec/` ABI v1.1

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-334** | P1 | **Transport `virtio-pci` trong `v2/lib/vifast-virtio` + `pci`/`virtio_blk` 3 arch + MSRV** | `KT-330` | 3 h | `DONE` | `v2/lib/vifast-virtio` (`sync_for_device/cpu` bảo toàn `dc civac`); `pci`+`blk` −49,3 %; MSRV `1.100.0`. |
| **KT-335** | P1 | **Hợp nhất `virtio_net` 3 arch + chuyển TCP/client state machine về `v2/lib/vifast-net`** | `KT-334` | 3 h | `READY` | Xoá 2 bản `tcp.rs` nhân bản; chuyển state machine TCP/DHCP/ARP/ICMP/DNS dùng `Clock` vào `vifast-net` (≥ 25 host test + ca âm); rút 4.004 dòng `virtio_net.rs` 3 arch về transport `KT-334`; `net.sh` + aarch64/x86_64 TAP + T1/T2 PASS. |
| **KT-336a** | P1 | **Nối `vifast-sync` (`IrqMcsLock`) vào `x86_64`** | `KT-330` | 3 h | `DONE` | `vifast-plat-x86_64/src/irq_lock.rs` thay spinlock `COM1` + `MH_LOCK`, TAP 34 + 3 đột biến ĐỎ. |
| **KT-336b** | P1 | **Tách `v2/sim/vifast-sim/src/kobj.rs` + đồng bộ checker `wake` + R1 `sim/src/`** | `KT-330` | 3 h | `DONE` | `kobj.rs` 11.964→1.030 dòng (13 module ≤ 1.402); R1 phủ `sim/`; 6 chỗ `SIM_KOBJ` đồng bộ; L1 digest bất biến. |
| **KT-336c** | P1 | **Hoàn tất `riscv64` `IrqMcsLock` (`core_bridge::LOCKED`) & tài liệu §28 `ObjStore`** | `KT-336a, KT-336b` | 2 h | `DONE` | `vifast-plat-riscv64/src/irq_lock.rs` (`csrrc sstatus,SIE`), TAP 70 + 3 đột biến ĐỎ, §28 `ObjStore` trong `k3b-core.md`; chốt D2-244 giữ `IpiMailbox` bitmask cho tầng 1 Q1-A. |
| **KT-337** | P1 | **Hợp nhất `virtio_gpu` + `virtio_input` & mở `virtio-gpu` x86_64** | `KT-334` | 3 h | `DONE` | `gpudev`/`inputdev` −61,4 %; `gpu-x86_64.sh` PPM 23/23 (`-vga none`), đột biến `offset=0` ĐỎ. |
| **KT-345a** | P1 | **Áp `spec/` ABI v1.1 + `vifast-gen` + `VspaceOutcome` rollback theo D2-242** | `KT-344` | 3 h | `DONE` | **9/9 nhóm D2-242 ĐẠT**: `spec/kernel.toml` + `check_kernel.py` (T2 id 77), `[ipc_buffer]`, `VspaceOutcome` rollback, 3 `.vidl`, **`7C-r13`**, alias `[object_size]`, `Frame` frozen / `VSpace`+`PageTable` core; `abi_version = 1`; `spec/` khoá cứng trở lại. |

### Đợt 2 — Giải nghẽn `.bss` (< 64 KiB), đóng N5/Q1-A tầng 1, một lõi `vifast-core` 3 arch & sản phẩm `riscv64`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-338a** | P1 | **`aarch64`: giải phóng `.bss` (`BootMemory`/DMA ra RAM)** | `KT-334, KT-336a` | 3 h | `DONE` | `.data+.bss` **233.704 → 30.632 B (−86,98 %)**, arena 180 KiB + 4 trang DMA ra RAM thật identity-map; `run-aarch64.sh` 27/27. |
| **KT-338c** | P1 | **`aarch64`: tách `main.rs` `< 1.100` dòng + tách sản phẩm vs `ktest-aarch64` (`D2-08`) + doorbell `IpiMailbox`** | `KT-338a` | 2 h | `READY` | **(D2-244)** Cắt `kbin-aarch64/src/main.rs` (`1.491 → < 1.100` dòng); tách sản phẩm (0 harness marker, `harness-scan` + ca âm, `.data+.bss < 64 KiB`) vs `ktest-aarch64`; nối `vifast_plat_common::ipi::IpiMailbox` cho SGI (bước 2 Q1-A tầng 1); đột biến ĐỎ; `run-aarch64.sh` 27/27 + T1/T2 PASS. |
| **KT-338b** | P1 | **`aarch64`: nối `vifast-core` & IPC EL0 (`svc #0`)** | `KT-338c, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` vào `aarch64` (`.data+.bss ≤ 256 KiB` đo bằng `budget-aarch64.sh`); nối `svc #0` theo `[calling.aarch64]` + `[debug_info]`/`[object_size]` (`KT-345a`) và `SyscallEnv::tcb_vspace -> VspaceOutcome` vào `core::syscall`; TAP `core` + `ipc` EL0 + 2 đột biến đỏ; T2 PASS. |
| **KT-339** | P1 | **Trừu tượng hoá `vifast-plat-user` đa arch + `aarch64` K3.5 (`procsrv` + `shell`)** | `KT-338b` | 3 h | `BLOCKED` | Tách stub syscall theo arch trong `vifast-plat-user` để cùng nguồn Rust `procsrv`/`shell` chạy được trên `riscv64` & `aarch64`; ảnh sản phẩm `aarch64 --features product-shell` nạp ELF bằng `vifast-elf`, chạy `help`/`echo`/`ps`/`exit` qua PL011 stdin; ca âm không `product-shell` ĐỎ; T2 PASS. |
| **KT-340a** | P1 | **`x86_64`: kernel W^X + `budget-x86_64.sh` + tách sản phẩm vs `ktest-x86_64` (`D2-08`)** | `KT-334, KT-336a` | 3 h | `DONE` | `budget-x86_64.sh` (T2 id 76, 10/10); tách sản phẩm/ktest (marker 74→0); kernel W^X (`cr0_wp=1`, ghi `.text` `#PF`, TAP 36/36); `.data+.bss` sản phẩm 96.320 B *(phần `VSpace` động + hạ `< 64 KiB` tách sang `KT-340b`)*. |
| **KT-340b** | P1 | **`x86_64`: tách `main.rs` `< 1.100` dòng + hạ `.bss` sản phẩm `< 64 KiB` + dựng `VSpace` động** | `KT-340a` | 3 h | `READY` | **(D2-244)** Cắt `kbin-x86_64/src/main.rs` (`1.500 → < 1.100` dòng); dời bảng trang boot / stack cố định / DMA page ra ngoài `.bss` để `.data+.bss` sản phẩm **`< 64 KiB`** qua `budget-x86_64.sh`; dựng **`VSpace` động** (`vifast-pte::PageTable` + physmap, map/unmap + đột biến ĐỎ); `run-x86_64.sh` + `-cpu max` + `gpu-x86_64.sh` + T1/T2 PASS. |
| **KT-340d** | P1 | **`x86_64`: TLB shootdown đa hart (`invlpg` + IPI AP→BSP qua `IpiMailbox`) & đóng lỗi v1 **N5** (`PCID`/`-cpu max`)** | `KT-340b` | 3 h | `BLOCKED` | **(D2-238/D2-244)** Cài `invlpg` + IPI shootdown AP→BSP dùng chung `vifast_plat_common::ipi::IpiMailbox` (hoàn tất **3/3 port** đóng Audit Q1-A/`N7` tầng 1); kiểm chứng `CR4.PCIDE`/TLB cũ trên `-cpu max` + đột biến bỏ shootdown/ack ĐỎ; đóng chính thức lỗi v1 **N5** (`KNOWN-ISSUES.md` #5); giữ `.bss` sản phẩm `< 64 KiB`; T2 PASS. |
| **KT-340c** | P1 | **`x86_64`: nối `vifast-core` & IPC ring 3 (thay scheduler riêng)** | `KT-340d, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` (`.data+.bss ≤ 256 KiB` đo bởi `budget-x86_64.sh`); nối `int 0x80` theo `[calling.x86_64]` v1.1 (**`7C-r13`**: `rax`=error, `rdx`=value, **`r13`=MessageInfo**, `r10/r8/r9/r12`=MR0..3) + `VspaceOutcome`; gỡ scheduler trùng lặp; hart phụ `hlt`; TAP `core` + `ipc` ring 3 + `-cpu max` xanh; T2 PASS. |
| **KT-341** | P1 | **`x86_64` K3.5: `procsrv` + `shell` dùng chung `vifast-plat-user`** | `KT-339, KT-340c` | 3 h | `BLOCKED` | Ảnh sản phẩm `x86_64 --features product-shell` chạy cùng nguồn `procsrv`/`shell` qua COM1 stdin; ca âm ĐỎ; T2 PASS. |
| **KT-342** | P1 | **Hợp nhất ảnh sản phẩm `riscv64`: gỡ `ktest` khỏi driver + chạy đồng thời FS, `netsrv`, `exec`** | `KT-335, KT-336a, KT-336c` | 3 h | `BLOCKED` | Tách sớm `vifast-plat-riscv64/src/user.rs` (`1.499 → < 1.100` dòng); tách nhãn feature driver khỏi `ktest`; tích hợp FS + `netsrv` U-mode + `exec` vào ảnh sản phẩm (`.data+.bss ≤ 256 KiB`, 0 harness marker, giữ `IrqMcsLock` + `IpiMailbox`); 1 boot chạy cả `ls`/`cat`/`write` + `ping` + `run /hello.elf`; 3 đột biến đỏ; T2 PASS. |
| **KT-343** | P1 | **Đóng 11 lệnh `deferred-K7` không cần mở ABI (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`, `run`, `runfs`, `exec`)** | `KT-342` | 3 h | `BLOCKED` | Chuyển 11 lệnh sang `active` trong `spec/v1-parity.toml` (**25/28 active**); parity `--mode serial` xanh 25/25 ở `smp=1` và `smp=4`; ca âm từng nhóm; T2 PASS. |

### Đợt 3 — Hoàn tất ABI v1.1, tách `v2/user/`, Dọn kho tiền `v2.0` & Nghiệm thu Chu kỳ 2

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-345b** | P1 | **Thi hành tính năng ABI v1.1: `free`/`df`/`loop` (28/28 parity) + `NetSvc` đa caller** | `KT-342, KT-343, KT-345a` | 3 h | `BLOCKED` | `netsrv` U-mode phục vụ ≥ 2 caller đồng thời theo `spec/protocols/socket.vidl` (`6A`: bảng phiên theo `(badge, handle)`, đầy ⇒ `Busy` 14, sai `badge`/`handle` ⇒ `InvalidCapability` 1, **kernel `NetSvc` đơn caller**); `OP_STATFS` + `MemSvc` (`4C`) đóng `free`/`df`/`loop` (**28/28 active**); parity `--mode serial` 28/28 ở `smp=1,4`; T2 PASS. |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184 (3 arch)** | `KT-341, KT-342` | 3 h | `BLOCKED` | Chuyển build ELF user ra `v2/user/` kèm `--remap-path-prefix`, đóng gói ELF vào ảnh FS/initrd; thêm `v2/user` vào `VBREV_INPUT_ROOTS` + tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `build-rev.algo.pin` + `wake_scope.py`; `repro-check.sh` chứng minh **cùng sha256 trên 2 thư mục build cho 3 arch** (Linux/macOS); T2 PASS. |
| **KT-347** | P2 | **Thiết kế & PoC `U1a` (`no_std+alloc` `vifast-rt`) + hợp đồng `.vidl` `ViScript` + hướng `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `BLOCKED` | Chạy thật 1 ứng dụng user `U1a` (stable `1.100.0`) qua `exec` từ FS; lập `v2/docs/u1-uefi-readiness.md` chốt `U1a`/`U1b`, hợp đồng `.vidl` & script kiểm chéo trên máy đo `ViFastOs ↔ ViScript` (không GitHub Actions theo D2-241), và `UEFI`/DTB/ACPI cho Chu kỳ 3; T2 PASS. |
| **KT-347b** | P1 | **Dọn kho & tăng tốc cổng tiền `v2.0`: `soak`/`budget` chung, 1-pass `wake`, gom phiếu cũ & archive raw `.log` Chu kỳ 1** | `KT-333d, KT-345b, KT-346` | 2 h | `BLOCKED` | **(D2-245)** (1) Rút `v2/test/boot/lib/{soak,budget}-common.sh` khử lặp 4.202 dòng `soak*.sh`/`budget*.sh`; (2) thêm chế độ 1-pass combined (`normal` + `--strict`) cho `check-wake-paths.py` (giữ selftest 452/452); (3) gom phiếu/báo cáo vòng cũ ở root (ngoài legacy SHA của `verify-numbers.py`) vào `v2/agent/archive/reports/`; (4) tách ~8.000 raw `.log` Chu kỳ 1 trong `JOURNAL/` không được `verify-numbers.py` tham chiếu sang nhánh/tag lưu trữ `cycle1-raw-logs` (giữ 100 % `summary.txt`, `*.md`, tệp có `scope=`); `verify-numbers.py` (`ok=5 fail=0`) + T2 PASS. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | `KT-331, KT-332, KT-333a..d, KT-336b/c, KT-337, KT-341, KT-343, KT-345b, KT-346, KT-347b` | 3 h | `BLOCKED` | Chạy độc quyền trên ứng viên tích hợp cuối: T2 PASS; `v2 nightly --all` (L1 100k × smp 1/2/4/8 + Miri + `STATUS.md`; `v2 mutants` ở Chu kỳ 3 theo D2-240/D2-244) PASS; soak **ảnh sản phẩm có shell** 3 arch + **soak đĩa thật `virtio-blk`** đều 0 lỗi (`qemu_foreign=0`, không retry); đối chiếu **Exit Criteria `v2.0` (§6.2)**; xuất `v2/agent/reports/R2-v2.{md,json}` qua `check_report.py --all` + `verify-numbers.py`; trình Agent 0 chốt Chu kỳ 2 & quyết định mở `v2.0`. |

---

## 4. Vùng xung đột & quy chế vận hành tối ưu cho Agent 2 (**D2-236..D2-245**)

| Vùng tệp / Nút thắt | Rủi ro đã xảy ra | Quy tắc bắt buộc |
|---|---|---|
| **Phân tầng T0/T1 vs T2** (`AGENTS.md` §6.1/§6.3) | 2 `gate --fast` song song (`KT-333a`), đo dưới tải `loadavg=12` (`KT-340a`) | **Sub-agent chỉ chạy T0 + T1 (`v2 gate --dev`)** trên worktree; **chỉ Lead chạy T2 (`v2 gate --fast`)** trên ứng viên gộp lô (1–3 task sạch) lúc máy rảnh (trừ thay đổi bắt buộc T2 ngay theo §6.1). |
| **4 slot worktree & ổ đĩa** | 9 worktree chiếm ~12 GiB `target/`, còn 438 MiB (`KT-338a`) | Tái dùng cố định **4 slot** (`/tmp/vifast-wt-{1..4}`) gắn `CARGO_TARGET_DIR` riêng theo slot (hoặc `sccache`); tự dọn worktree khi merge `DONE`; chặn mở mới khi `df < 5 GiB` (`wt.sh` ở `KT-333d`). |
| **Cấm "DONE một phần" & đồng bộ sổ** | Sót DoD đẩy miệng sang task sau (`KT-336a/b/c`, `KT-338a`, `KT-340a`); quên mở `KT-340b` | Hết 3 h mà còn sót mục DoD ⇒ **tách ngay task con tường minh** trong `QUEUE.md`, không ghi "DONE một phần"; dùng **`vifast-agentctl lint`** (`KT-333d`) bắt tự động quên mở `READY`, ID sai mẫu `commit-msg`, lệch `STATE.md`. |
| `v2/bin/v2` (1.919 dòng) | Đụng merge liên tục giữa `KT-333c`/`340a`/`345a`; trùng ID `lane_step` | Chỉ **1 task mỗi vòng** chạm `v2/bin/v2`; **`KT-333c`** thêm lockfile máy đo, tiếp nối **`KT-333d`** tách sang `v2/bin/lib/*.sh` + pre-flight fail-fast < 1 s. |
| Đường dẫn tạm & GitHub Actions | `$TMPDIR/k5-x86_64` đụng nhau khi chạy song song; CI ngoài sinh bằng chứng sai | Đổi mọi đường tạm sang `mktemp -d` (`KT-333d`); **cấm vĩnh viễn `.github/workflows/**`** (`D2-241`, `AGENTS.md` §4.11). |
| Chính sách `JOURNAL/` | 9.014 tệp / 55 MiB raw `.log` làm chậm `git`/grep; `KT-336c` thêm 10 `.log` mới | Kể từ sau `bfb9a13d`, mỗi task chỉ commit **1 tệp `JOURNAL/kt<n>-<ten>.md`**, không tạo thư mục `kt*-logs/*.log` trừ trường hợp có `scope=` của `verify-numbers.py`; **`KT-347b`** archive ~8.000 raw `.log` Chu kỳ 1. |
| 3 tệp chạm trần R1 (`1.500` dòng) | `kbin-x86_64/main.rs` (1.500), `plat-riscv64/user.rs` (1.499), `kbin-aarch64/main.rs` (1.491) | Cắt sớm về `< 1.100` dòng ngay ở đầu **`KT-340b`**, **`KT-338c`**, **`KT-342`** trước khi thêm tính năng mới. |
| `spec/abi.toml`, `spec/kernel.toml` | Vỡ đóng băng ABI | **`KT-345a` đã `DONE` — `spec/` khoá cứng trở lại** cho toàn bộ phần còn lại của Chu kỳ 2 (`D2-234`). |

---

## 5. Hướng dẫn khởi động ngay cho các Agent sau khi `git pull`

```bash
git pull origin main
git status -s                                              # phải sạch trên cả Linux/macOS/Windows
cargo run -q --manifest-path v2/Cargo.toml -p vifast-agentctl -- report --project v2
bash v2/tools/status.sh
```

1. **4 slot worktree `READY` ngay — 0 chồng chéo tệp (chạy song song 4 slot)**:
   - **Slot 1**: **`KT-333c`** (lock máy đo + `gpu-x86_64` T2 + `nightly --soak`) → nối tiếp **`KT-333d`** (tách `v2/bin/v2` + pre-flight + `wt.sh` + `agentctl lint`).
   - **Slot 2**: **`KT-335`** (`virtio_net` + TCP vào `vifast-net`) → **`KT-342`** → **`KT-343`** → **`KT-345b`**.
   - **Slot 3**: **`KT-338c`** (`aarch64` cắt `main.rs` + tách product/ktest + `IpiMailbox`) → **`KT-338b`** → **`KT-339`** *(nếu `KT-338b` đã mở worktree trước khi kéo `D2-244`, KHÔNG huỷ — chạy nốt rồi nối `KT-338c` ngay sau)*.
   - **Slot 4**: **`KT-340b`** (`x86_64` cắt `main.rs` + hạ `.bss < 64 KiB` + `VSpace` động) → **`KT-340d`** → **`KT-340c`** → **`KT-341`**.
2. **Áp quy chế D2-245 ngay**: sub-agent chỉ chạy **T0 + T1 (`v2 gate --dev`)**; chỉ Lead chạy **T2 (`v2 gate --fast`)** trên ứng viên gộp lô lúc máy rảnh; không tạo `kt*-logs/*.log`; không ghi "DONE một phần".
3. **Làm mới `v2/agent/STATE.md`**: ở lần merge kế tiếp, đồng bộ dòng `READY`/`BLOCKED` theo `QUEUE.md` mới.

---

## 6. Khung lộ trình dài hạn (Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`) & Tiêu chí phát hành `v2.0` (**D2-240/D2-241/D2-244/D2-245**)

### 6.1 Bảy trụ cột chiến lược sau Chu kỳ 2 (Chu kỳ 3 trở đi)

| # | Trụ cột | Mốc đề xuất | Thực trạng hiện tại & việc bắt buộc phải làm |
|---|---|---|---|
| 1 | **SMP Preemptive thật (`S1 → S2 → S3`) & Tầng 2 `Mailbox<N>`** | `SMP-1`, `SMP-2` | Sau khi Chu kỳ 2 thống nhất doorbell `IpiMailbox` + `IrqMcsLock` trên 3/3 port (D2-244), `SMP-1` đưa `vifast-core::Machine::dispatch` + timer preempt + tầng 2 `vifast_sync::Mailbox<N>` (mang payload có bit `OVERFLOW` rơi về `IpiMailbox`) chạy U-mode thật trên 4 hart ở cả 3 arch, rồi tách sang `PerHart` (`SMP-2`); chỉ mở `S3` khi `vifast-bench` đo chờ khoá > 10 % ở `smp=4`. |
| 2 | **Hợp nhất `Machine` (đóng `D2-83`) & `vifast-objmem` (D2-14)** | `MEM-1`, `MEM-2` | Rút Sv39/PLIC khỏi `vifast-plat-common`, đưa `TableMemory` lên `vifast-pte` (`MEM-1`); gom 14 `[[bin]]` của `vifast-kbin-riscv64` về 2 mode `product`/`ktest` như `aarch64`/`x86_64`; chuyển cấp phát đối tượng vào `Untyped` thật qua `vifast-objmem` để `.data+.bss` < 64 KiB và quy mô theo RAM (`MEM-2`). |
| 3 | **4 nền tảng kiểm chứng/quan sát trong thiết kế v2** | `OBS-1`, `VER-1` | Hoàn thiện những mảnh đã cam kết nhưng còn trống: (i) **trace nhị phân 32 B luôn bật** + dump `pmemsave` + `vifast-trace` (**D2-09**, §9.11); (ii) **mutation testing tự động** `v2/test/mutants.toml` + `v2 mutants` trong `nightly` (§9.9 — xác nhận ở Chu kỳ 3 theo D2-240/D2-244); (iii) **`vifast-bench` + `test/perf/history.csv`** dưới `-icount` (§9.10); (iv) **conformance `.scn` đủ 3 arch** (D2-13) + **MCS refill đa đoạn/`SetTimeoutEp`** (D2-16). |
| 4 | **Driver từ Polling → IRQ `Notification` → Userland + IOMMU** | `DRV-1`, `D2`, `D3` | Chuyển driver VirtIO từ busy-poll sang ngắt thật qua **`IrqControl`/`IrqHandler` + `Notification`** (mở ở ABI v1.2); làm nền cho driver userland (`D2`: PCIe MSI-X, NVMe, AHCI) kèm `Frame.MapIo` + **`IOSpace` (VT-d / SMMUv3 / virtio-iommu, `D3`)** để cô lập DMA. |
| 5 | **An toàn, FPU/SIMD & chuẩn hoá `.vidl`** | `SEC-1`, `IDL-1` | Lưu/khôi phục **ngữ cảnh FPU/SIMD theo TCB** (`fs0..fs31`, NEON `v0..v31`, `fxsave`/`xsave` — bắt buộc trước `U1`); bộ test đối kháng `copy_from_user`/`copy_to_user` + bật `SMEP`/`SMAP`/`PAN` + chính sách KPTI tuỳ chọn; fuzz `vifast-net` + nguồn entropy `virtio-rng`; chuẩn hoá `proc.vidl`, `fs.vidl`, `net.vidl`, `term.vidl` trong `spec/protocols/`. |
| 6 | **`U1` hai tầng & Hợp đồng liên repo `ViFastOs ↔ ViScript` (`VSH2`)** | `U1a`, `U1b`, `VSH2` | Chia `U1` thành **`U1a`** (`no_std + alloc` + `vifast-rt` qua `.vidl`, chạy ngay trên **stable `1.100.0`** để kéo `vis-host-vifast`/ViShell lên sớm) và **`U1b`** (Rust `std` PAL target `*-unknown-vifastos`); chốt hợp đồng phiên bản `abi_version`/`.vidl` và **script kiểm chéo định kỳ trên máy đo** (`v2 nightly` / bộ hẹn giờ nội bộ, không dùng GitHub Actions theo **D2-241**) giữa repo `ViFastOs` và repo [`ViScript`](VISCRIPT.md). |
| 7 | **Khám phá phần cứng thật (`HW1`) & Tinh gọn kho mã nguồn (Pha 3)** | `HW-0`, `UEFI`, `HW1` | Trước `UEFI`/`D2`/`HW1`, bổ sung mốc **`HW-0`**: phân tích **DTB thật** (`riscv64`/`aarch64`) và **ACPI `MADT`/`FADT` + bản đồ bộ nhớ E820/UEFI** (`x86_64`) thay hằng số QEMU `virt` gán cứng. Đồng thời **đóng băng & tách nội bộ `check-wake-paths.py`** (10.769 dòng — ưu tiên ép bất biến bằng hệ kiểu Rust) và đánh giá gỡ cây v1 đóng băng (`crates/`, `legacy/`, `deploy/`, `python-model/`, đã lưu tại tag `v1-archive`) khỏi nhánh `main` sau `v2.0`. |

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 shell** (`riscv64`/`aarch64`/`x86_64`, tách sản phẩm/harness D2-08) | **Có** (`KT-338a..KT-342`) | `SMP-1/2` preemptive 4 hart đầy đủ + gom 14 bin `kbin-riscv64` về 2 mode |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active`** (`KT-343` + `KT-345b`, `smp=1` & `smp=4`) | Mở rộng lệnh `ViShell` (`VSH2`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có** (đóng **N5** ở `KT-340d` và **Audit Q1-A/N7 tầng 1**: `IpiMailbox` + `IrqMcsLock` đủ 3/3 port qua `KT-336a/336c/338c/340d`) | Tầng 2 `vifast_sync::Mailbox<N>` có `OVERFLOW` bit (`SMP-1`) |
| **Ngân sách & Cổng** (`.data+.bss ≤ 256 KiB` cả 3 arch, `Machine ≤ 196.608 B`, T2 PASS, `v2 nightly --all` L1 400k + Miri PASS) | **Có** (`budget*.sh`, `KT-333b/c/d`, `KT-347b`, `KT-348`) | `v2 mutants` + `vifast-bench` tự động trong nightly (D2-240/D2-244) |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`) | **Ảnh sản phẩm có shell** 3 arch (`rv 1000`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) + **soak đĩa thật `virtio-blk`** (`KT-348`) | Soak đa tiến trình `U1`/mạng tải nặng |
| **Tái lập byte (P20) & Vệ sinh kho** | **Cùng sha256 canonical cả 3 arch trên Linux + macOS** (`KT-346`); dọn raw `.log` Chu kỳ 1 + phiếu cũ (`KT-347b`) | Hội tụ trên host **Windows** (B2); đánh giá gỡ cây v1 (`crates/`) khỏi `main` |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md` tự sinh** (`KT-333b`), clone/checkout sạch Win/macOS/Linux (`KT-331`) | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **Động hóa bộ nhớ, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | Có tài liệu định hướng & PoC `U1a` (`KT-347`) | Thi công ở **Chu kỳ 3 (`v2.1+`)** theo §6.1 |

