# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; bổ sung **D2-236..D2-240** tại `5e635474`; **duyệt `KT-344` & tinh chỉnh D2-241..D2-243** tại `2bf130bf` sau khi Đợt 0–1 hoàn tất 10/11 task).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-243**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md).

> **Mục tiêu một câu**: Khép soak vòng 25 (`KT-330`), chấm dứt trạng thái "mỗi kiến trúc một nhánh rời / mỗi tính năng một `ktest-*` riêng" — **khử nhân bản driver trên 3 kiến trúc** (`v2/lib/vifast-virtio` + `vifast-net`), **giải nghẽn `.bss` & đóng lỗi v1 N5 + Audit Q1-A đủ 3 port**, **nối `vifast-core` + `shell` dùng chung lên `aarch64` và `x86_64`**, **hợp nhất ảnh sản phẩm `riscv64`** (FS + Net + Exec, đạt **28/28** lệnh parity v1), **hạ thời gian cổng & dựng `v2 nightly` thật trên máy đo (không GitHub Actions — D2-241)**, **áp gói ABI v1.1 đã duyệt (`KT-344` `APPROVED` → `KT-345a` `READY`, `7C-r13`)**, và chốt khung **Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`**.

---

## 0. Chẩn đoán đầu vào (căn cứ từ mã nguồn `5e635474`, cập nhật thực thi tại `2bf130bf`)

Chu kỳ 1 (`KT-01..KT-329`, 25 vòng đã hoàn tất) đạt kỷ luật kiểm chứng rất cao (T2 `gate --fast` PASS 68/68, soak 3 arch `9e18880d` 0 lỗi, K6 đóng, luật `wake` đóng, 3 arch đều boot thật trên QEMU). Đối chiếu trực tiếp `Cargo.toml`, `spec/abi.toml`, cây nguồn và cổng đo lộ ra **10 điểm nghẽn cấu trúc** (7 gốc ở D2-229 + 3 phát hiện bổ sung ở D2-236..D2-238; đến `2bf130bf` đã đóng các mục 7, 8, 10 và phần lớn mục 6):

1. **`aarch64` (K4') và `x86_64` (K5') chưa nối `vifast-core`**: `vifast-kbin-aarch64` và `vifast-kbin-x86_64` chưa phụ thuộc `vifast-core`, `vifast-elf`, `vifast-vfs`, `vifast-fs`; cả hai vẫn là 1 binary selftest duy nhất (chưa tách ảnh sản phẩm vs harness theo D2-08). `x86_64` đang phát triển một bộ lập lịch độc lập tách rời `vifast-core::Machine`.
2. **Nghẽn toán học `.data+.bss ≤ 256 KiB` khi nối `vifast-core` (D2-236/D2-243)**: `aarch64` đang dùng **233.704 / 262.144 B (89,2 %)** do `BootMemory::ARENA` (`44` frame = `180.224 B`) và 4 trang DMA (`16 KiB`) trong `.bss`; trên `x86_64`, `KT-337` đo ảnh mặc định = **177.440 B** (`--features gpu` = **186.080 B**) — trừ 4 `TaskStack` test (`65.536 B`) vẫn còn **111.904 B** (riêng asm `.bss` ở `main.rs:388-443` chiếm `81.920 B`: 7 bảng trang + 4 IST1 + `__user_stack` + `__syscall_stack`, cộng các `DMA_PAGE`). Cộng `Machine` (`184.792 B`) sẽ vượt trần I7 trên cả hai arch nếu không hạ `.data+.bss` bản sản phẩm **< 64 KiB** trước khi link `Machine`.
3. **Xung đột thanh ghi trong `[calling.x86_64]` & thiếu mục đa arch ở `spec/abi.toml` (D2-237/D2-242)**: `[calling.x86_64]` đặt `return_value = "rdx"` **trùng** `message_info = "rdx"`, thiếu `return_message_info`, và đặt `r12` vừa `MR3` vừa `preserved`; `spec/abi.toml` thiếu `[debug_info].return_channel` và `[object_size]` cho `aarch64`/`x86_64`, và `VSpace`/`PageTable`/`Frame` còn ghi `reserved/unsupported=true`. *(Đã duyệt phương án sửa tại **D2-242**, `KT-345a` `READY`.)*
4. **`x86_64` thiếu `VSpace` động + W^X kernel + TLB shootdown & lỗi v1 N5 chưa đóng (D2-238)**: `vifast-plat-x86_64` chưa có `vspace.rs`/`wx.rs`/`tlb.rs`/`shootdown.rs`; lỗi v1 **N5** (`x86 AP→BSP + PCID`, `KNOWN-ISSUES.md` #5) hoãn sang K5' sẽ đóng tại `KT-340b`.
5. **Phân mảnh 14 binary trên `riscv64`**: `vifast-kbin-riscv64` có 13 binary `ktest-*`; các feature `net`/`gpu`/`input`/`vcon` của `vifast-plat-riscv64` vẫn kéo theo `ktest`; ảnh sản phẩm `product-fs` chưa có `exec`/`netsrv`/mạng và còn 14/28 lệnh `deferred-K7` trong `spec/v1-parity.toml`.
6. **Nhân bản driver lớn & crate mồ côi (`vifast-sync`, `vifast-objmem`) + bẫy `SIM_KOBJ`**: `KT-334`/`KT-337` đã rút `pci`/`virtio_blk`/`virtio_gpu`/`virtio_input` về `v2/lib/vifast-virtio`; `KT-336b` đã tách `kobj.rs` (11.964→1.030 dòng); `KT-336a` đã nối `IrqMcsLock` cho `x86_64`. Còn lại: `KT-335` (`virtio_net` + TCP) và **`KT-336c`** (bổ sung phần `riscv64` `IrqMcsLock` + `Mailbox` & tài liệu `ObjStore` bị sót sau `KT-336a/b`).
7. **Khép soak vòng 25 + `-cpu max` (`KT-330` `DONE`)**: đã đạt 0 lỗi trên `5e635474` (`riscv64` 0/1000, `x86_64` 0/600 `qemu64`, `x86_64` 0/600 `-cpu max`, `qemu_foreign=0`).
8. **Cổng T2 phình thời gian, `v2 nightly`, `deny.toml` & khoá độc quyền máy đo**: `KT-333a`/`KT-333b` đã hạ T2 xuống **~560 s (PASS 72/72)** và dựng `v2 nightly` + `deny.toml` + `THIRD-PARTY.md`; **`D2-241`** cấm vĩnh viễn GitHub Actions (`AGENTS.md` §4.11); **`KT-333c`** bổ sung khoá độc quyền máy đo chống chạy chồng cổng + nối `gpu-x86_64.sh` vào T2 + chuẩn bị `v2 nightly --soak` cho `KT-348`.
9. **Build userland qua `build.rs` + `include_bytes!` không mở rộng sang `U1`/`VSH2`** và là gốc nợ P20/A1-184 (đóng tại `KT-346`).
10. **Vệ sinh repo, sổ sách & tương thích đa nền tảng (`KT-331`/`KT-332` `DONE`)**: guard NTFS đã vào `v2 rules` + pre-commit; `STATE.md`/`BLOCKED.md`/`README.md` đã cắt gọn và đồng bộ; `v2/kernel/Cargo.toml` đã nâng `rust-version = "1.100.0"` (`KT-334`).

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
| **D2-242** | **Duyệt Gói ABI v1.1 (`KT-344` `APPROVED`) & mở khoá `KT-345a`** | Chấp thuận **1C, 2C, 3.1A+3.2A, 4C, 5A, 6A, 8C, 9B** và tinh chỉnh Nhóm 7 thành **`7C-r13`** (`return_message_info = "r13"` — một hợp đồng thanh ghi thống nhất cho cả `int 0x80` lẫn `syscall`/`sysret` sau này); giữ **`abi_version = 1`, `min_compat = 1`**; `KT-345a` làm luôn `VspaceOutcome` + rollback `TcbData.vspace` để `KT-338b`/`KT-340c` dùng đúng chữ ký ngay từ đầu. |
| **D2-243** | **Tinh chỉnh Chu kỳ 2 sau Đợt 0–1 (`KT-336c`, `KT-333c`, `.bss < 64 KiB` `x86_64`, đồng bộ `D2-241`)** | Bổ sung **`KT-336c`** (`READY`, `riscv64` `IrqMcsLock` + `Mailbox` + `k3b-core.md` `ObjStore` bị sót sau `KT-336a/b`); bổ sung **`KT-333c`** (`READY`, khoá độc quyền `v2/bin/v2`/`soak*.sh` + nối `gpu-x86_64.sh` vào T2 + `v2 nightly --soak` sản phẩm/đĩa thật); chặn cứng ảnh sản phẩm `x86_64` `.data+.bss < 64 KiB` ở `KT-340a`/`KT-340b` trước `KT-340c`; đồng bộ toàn bộ kế hoạch theo `D2-241`. |

---

## 2. Lộ trình 4 đợt & đồ thị phụ thuộc (`KT-330..KT-348`, 27 task ≤ 3 h)

```text
[VÒNG 25 — ĐÃ DONE TẠI 5e635474: KT-328 (x86_64 ICMP+DNS) & KT-329 (Suspend Defer + Resume)]
        │
        ▼
[ĐỢT 0 — 6/6 DONE (KT-330, KT-331, KT-332, KT-333a*, KT-333b, KT-344 APPROVED D2-242)]
  * KT-333a: tối ưu T2 561 s PASS 72/72 giữ nguyên; GitHub Actions BỎ theo D2-241.
  * KT-333c [READY] (D2-243): khoá độc quyền máy đo v2/soak + nối gpu-x86_64.sh vào T2 + nightly --soak sản phẩm/đĩa thật
        │                                                           │
        │ (sau KT-330)                                              │ (ĐÃ APPROVED D2-242)
        ▼                                                           ▼
[ĐỢT 1 — Khử nhân bản & hợp nhất tầng dùng chung]              KT-345a [READY] (áp spec/ + vifast-gen
  ├── KT-334  [DONE]  (v2/lib/vifast-virtio + pci/blk + MSRV 1.100) │ + 7C-r13 + VspaceOutcome rollback;
  │     ├──► KT-335  [READY] (virtio_net 3 arch + TCP → vifast-net) │   tiền đề của KT-338b & KT-340c)
  │     └──► KT-337  [DONE]  (virtio_gpu/input + GPU x86_64 PPM)    │
  ├── KT-336a [DONE]  (x86_64 IrqMcsLock COM1/MH_LOCK)              │
  ├── KT-336b [DONE]  (tách sim/src/kobj.rs + SIM_KOBJ + R1 sim/)   │
  └── KT-336c [READY] (riscv64 IrqMcsLock + Mailbox + k3b-core.md ObjStore — đóng sót KT-336a/b)
        │                                                           │
        ▼                                                           ▼
[ĐỢT 2 — Giải nghẽn .bss (< 64 KiB), đóng N5/Q1-A, một lõi 3 arch & Hợp nhất ảnh sản phẩm riscv64]
  ├── (KT-334, KT-336a) ──► KT-338a [READY/đang chạy] (aarch64: dời .bss < 64 KiB + product/ktest + Mailbox)
  │                           └──► (KT-338a, KT-345a) ──► KT-338b (aarch64: vifast-core + IPC EL0)
  │                                                         └──► KT-339 (vifast-plat-user + aarch64 shell)
  ├── (KT-334, KT-336a) ──► KT-340a [READY/đang chạy] (x86_64: VSpace + W^X + budget-x86_64 + product/ktest)
  │                           └──► KT-340b (x86_64: TLB shootdown AP→BSP/Mailbox + đóng N5 + .bss sản phẩm < 64 KiB)
  │                                 └──► (KT-340b, KT-345a) ──► KT-340c (x86_64: vifast-core + IPC r13)
  │                                                               └──► (KT-339, KT-340c) ──► KT-341 (x86_64 shell)
  └── (KT-335, KT-336a, KT-336c) ──► KT-342 (riscv64: gỡ ktest khỏi driver + ảnh sản phẩm FS+Net+Exec)
                                       └──► KT-343 (đóng 11 lệnh K7 không cần ABI gồm run/runfs/exec → 25/28 active)
        │
        ▼
[ĐỢT 3 — Hoàn tất ABI v1.1 (28/28), tách Userland v2/user/ & Nghiệm thu Chu kỳ 2]
  ├── (KT-342, KT-343, KT-345a) ──► KT-345b (free/df/loop → 28/28 parity + NetSvc đa caller 6A)
  ├── (KT-341, KT-342)          ──► KT-346  (tách v2/user/ + tăng VBREV_ALGO kt134-v5 + đóng P20 3 arch)
  │                                   └──► KT-347 (PoC U1a + hợp đồng .vidl ViScript + UEFI/DTB/ACPI)
  └── (KT-331, KT-332, KT-333a, KT-333b, KT-333c, KT-336b, KT-336c, KT-337, KT-341, KT-343, KT-345b, KT-346)
                                ──► KT-348  (Cổng nghiệm thu Chu kỳ 2: soak sản phẩm 3 arch + đĩa thật
                                             + T2 + nightly --all + Exit Criteria v2.0 + báo cáo R2-v2)
```

---

## 3. Chi tiết từng đợt & bảng task (mọi task ≤ 3 h)

### Đợt 0 — Khép vòng 25, vệ sinh, hạ tầng cổng & RFC ABI v1.1

> **Luật thực thi Đợt 0**: `KT-331`, `KT-332`, `KT-333a`, `KT-333b`, `KT-333c`, `KT-344` **không** sửa `v2/kernel/**` hay `v2/lib/**` (không làm bẩn `build-inputs`). Khi chạy **`KT-330`** (soak), máy đo chạy **độc quyền** (`qemu_foreign=0`, D2-116).

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-330** | P0 | **Soak ứng viên cuối vòng 25**: `riscv64` 1.000 + `x86_64` 600 (`qemu64`) + `x86_64` 600 (`-cpu max`) | `KT-328, KT-329` | 3 h | `DONE` | 0 lỗi cả 3 đợt soak trên `5e635474` (`rc_124=0`, `qemu_foreign=0` hai mẫu, `invalid=0`, không retry D2-07); chỉ commit tóm tắt `v2/agent/JOURNAL/kt330-soak/summary.md` (D2-230). |
| **KT-331** | P0 | **Guard tên tệp Windows/NTFS + chính sách `JOURNAL/`** | — | 2 h | `DONE` | `v2/tools/check-filenames.py` (5 mã lỗi NTFS, selftest 23/23) nối vào `v2 rules` + `git-guard/pre-commit`; `.gitignore` chặn raw `run-soak-*/boot-*.log`. |
| **KT-332** | P0 | **Đồng bộ tài liệu công khai, cắt gọn `STATE.md` & dọn `BLOCKED.md`** | — | 2 h | `DONE` | `README.md`/`v2/README.md`/báo cáo tuần đồng bộ; `STATE.md` 335,8→3,6 KiB + `archive/STATE-chu-ky-1.md`; `BLOCKED.md` dọn 43 mục đóng. |
| **KT-333a** | P1 | **Profile & hạ wall-time T2 (`gate --fast`)** | — | 3 h | `DONE` | T2 hạ từ 766–828 s → **558,9–561,4 s** (`PASS 72/72`, 0 bước bỏ); thêm `CHECK_STEPS_EXPECTED` + `lane_fails 999`; ~~GitHub Actions~~ **BỎ vĩnh viễn theo D2-241**. |
| **KT-333b** | P1 | **Dựng `v2 nightly` thật + `deny.toml` + `THIRD-PARTY.md` (D2-21)** | — | 3 h | `DONE` | `v2 nightly` thật (selftest 14/14, sinh `v2/agent/reports/STATUS.md`); `deny.toml` + `v2 licenses` (selftest 9/9) + `THIRD-PARTY.md` tự sinh (bước 73/74/75). |
| **KT-333c** | P1 | **Khoá độc quyền máy đo (`v2/bin/v2` & `soak*.sh`) + nối `gpu-x86_64.sh` vào T2 + chuẩn bị `v2 nightly --soak`** | `KT-333a, KT-333b, KT-337` | 2 h | `READY` | **(D2-243)** (1) Khoá nguyên tử cấp máy đo (lockfile PID + kiểm sống, fail-closed + `--selftest` ca âm ĐỎ) cho `v2 gate --fast`/`nightly`/`soak*.sh`; (2) nối `gpu-x86_64.sh` + thêm ca âm "không GPU" vào làn `sau6` T2 (kiểm trùng `lane_step`, cập nhật `CHECK_STEPS_EXPECTED`); (3) mở rộng `v2 nightly --soak` cho ảnh sản phẩm có shell + đĩa thật `virtio-blk` sẵn sàng cho `KT-348`; không sửa `v2/kernel`/`v2/lib`; T2 PASS. |
| **KT-344** | P1 | **RFC Gói duyệt ABI v1.1 (`review/agent0/GOI-ABI-V1_1.md`) — 9 nhóm, KHÔNG sửa `spec/`** | — | 3 h | `DONE` | **`APPROVED 2026-11-22 Agent 0 (D2-242)`**: trình đủ 9 nhóm, 21 phương án (2.300 dòng, commit `e2533939`); duyệt **1C, 2C, 3.1A+3.2A, 4C, 5A, 6A, `7C-r13`, 8C, 9B**, giữ `abi_version = 1`, `min_compat = 1`. Mở khoá `KT-345a`. |

### Đợt 1 — Khử nhân bản driver, hợp nhất tầng dùng chung & Áp `spec/` ABI v1.1

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-334** | P1 | **Transport `virtio-pci` (`MmioBus`/`DmaPool` + cache hook) trong `v2/lib/vifast-virtio` + `pci`/`virtio_blk` 3 arch + MSRV** | `KT-330` | 3 h | `DONE` | `v2/lib/vifast-virtio` (`#![forbid(unsafe_code)]`, `sync_for_device/cpu` bảo toàn `dc civac` `aarch64`); `pci`+`virtio_blk` 4.407→2.235 dòng (−49,3%); `rust-version = "1.100.0"`; T2 PASS 72/72. |
| **KT-335** | P1 | **Hợp nhất `virtio_net` 3 arch + chuyển TCP/client state machine về `v2/lib/vifast-net`** | `KT-334` | 3 h | `READY` | Xoá 2 bản `tcp.rs` nhân bản trong `vifast-plat-{riscv64,aarch64}`; chuyển toàn bộ state machine TCP/DHCP/ARP/ICMP/DNS dùng trait `Clock` vào `vifast-net` (≥ 25 host test + ca âm timeout/reset/checksum); 3 arch dùng transport `KT-334` (giữ đủ TAP ICMP+DNS từ `KT-328`); `net.sh` + aarch64 TAP + x86_64 TAP + T2 PASS. |
| **KT-336a** | P1 | **Nối `vifast-sync` (`IrqMcsLock`) vào `x86_64`** | `KT-330` | 3 h | `DONE` | `vifast-plat-x86_64/src/irq_lock.rs` thay spinlock `COM1` + `MH_LOCK`, TAP 34 + 3 đột biến ĐỎ. *(Phần `riscv64` + `Mailbox` tách sang `KT-336c` theo D2-243.)* |
| **KT-336b** | P1 | **Tách `v2/sim/vifast-sim/src/kobj.rs` + đồng bộ checker `wake` + R1 `sim/src/`** | `KT-330` | 3 h | `DONE` | `kobj.rs` 11.964→1.030 dòng thành 13 module (≤ 1.402 dòng); R1 phủ `sim/vifast-sim/src`; đồng bộ 6 chỗ `SIM_KOBJ` (selftest 452/452 + 70/70); L1 smoke digest `237f3fa69d2bf1ea` bất biến. *(Mục `ObjStore` ở `k3b-core.md` gộp vào `KT-336c`.)* |
| **KT-336c** | P1 | **Hoàn tất `riscv64` `IrqMcsLock` (`core_bridge::LOCKED`) + `vifast_sync::Mailbox` (wake/IPI) & tài liệu `ObjStore`** | `KT-336a, KT-336b` | 2 h | `READY` | **(D2-243)** Thay `core_bridge::LOCKED` (`riscv64`) bằng `IrqMcsLock` (`sstatus.SIE`); nối `vifast_sync::Mailbox` cho kênh wake/IPI `riscv64` (đủ 3/3 port cùng `KT-338a`/`KT-340b` đóng Audit Q1-A/`N7`); đột biến bỏ SIE/khoá/Mailbox ĐỎ; viết mục `ObjStore` & `slotmem→vifast-objmem` trong `v2/docs/k3b-core.md`; matrix + T2 PASS. |
| **KT-337** | P1 | **Hợp nhất `virtio_gpu` + `virtio_input` qua `MmioBus`/`DmaPool` & mở `virtio-gpu` x86_64** | `KT-334` | 3 h | `DONE` | `gpudev`/`inputdev` lên `v2/lib/vifast-virtio` (3.282→1.267 dòng, −61,4%); `gpu-x86_64.sh` PPM 23/23 (`-vga none`), đột biến `offset=0` ĐỎ; T2 PASS 72/72. *(Nối vào T2 + ca âm không GPU giao `KT-333c`.)* |
| **KT-345a** | P1 | **Áp `spec/` ABI v1.1 + `vifast-gen` + `VspaceOutcome` rollback theo D2-242 (tiền đề cho `KT-338b`/`KT-340c`)** | `KT-344` | 3 h | `READY` | **(ĐÃ MỞ KHOÁ — D2-242, `abi_version = 1`, `min_compat = 1`)**: tạo `spec/kernel.toml` (`status = "APPROVED"`, 1C/5A) + `check_kernel.py` nối vào `v2/bin/v2`; cập nhật `spec/abi.toml` (`[ipc_buffer]` 2C, `[cspace]` + `Tcb.Configure.errors` 3.1A/3.2A, `[calling.x86_64]` **`7C-r13`** với `return_message_info = "r13"` + đồng bộ `calling.rs`/`tests/host.rs`, `[debug_info]`/`[object_size]` 8C, `Frame`→`frozen` + `VSpace`/`PageTable`→`core` 9B); tạo `spec/protocols/{socket,meminfo,fsstat}.vidl` (4C/6A) + mở `gen_idl.py`; đổi `SyscallEnv::tcb_vspace -> VspaceOutcome` + rollback `TcbData.vspace` ở `vifast-core`/`plat-riscv64` + đột biến ĐỎ; `vifast-gen`/`vifast-idl` + cập nhật checker + T2 PASS. Sau `DONE`, `spec/` khoá cứng trở lại. |

### Đợt 2 — Giải nghẽn `.bss` (< 64 KiB), đóng N5/Q1-A, một lõi `vifast-core` 3 arch & Hợp nhất sản phẩm `riscv64`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-338a** | P1 | **`aarch64`: giải phóng `.bss` (`BootMemory`/DMA ra RAM) + tách sản phẩm vs `ktest-aarch64` (`D2-08`) + `Mailbox`** | `KT-334, KT-336a` | 3 h | `READY` | Chuyển `BootMemory::ARENA` (180 KiB) + 4 trang DMA tĩnh ra RAM ngoài `.bss` (qua physmap như `KT-279`) để `.data+.bss` < 64 KiB (đủ chỗ cho `Machine`); tách ảnh sản phẩm (0 harness marker, `harness-scan` + ca âm) vs `ktest-aarch64`; nối `vifast_sync::Mailbox` cho SGI/shootdown (bước 2 đóng Q1-A); `budget-aarch64.sh` + TAP cũ giữ nguyên; T2 PASS. |
| **KT-338b** | P1 | **`aarch64`: nối `vifast-core` & IPC EL0 (`svc #0`)** | `KT-338a, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` vào `aarch64` (`.data+.bss ≤ 256 KiB` đo bằng `budget-aarch64.sh`); nối `svc #0` theo `[calling.aarch64]` + `[debug_info]`/`[object_size]` (`KT-345a`) và `SyscallEnv::tcb_vspace -> VspaceOutcome` (D2-242) vào `core::syscall` (`DebugPutStr`/`DebugGetInfo`/`Exit`/`Yield`/`Send`/`Recv`/`Call`/`Reply`); TAP cũ + TAP `core` + `ipc` EL0 + 2 đột biến đỏ; T2 PASS. |
| **KT-339** | P1 | **Trừu tượng hoá `vifast-plat-user` đa arch + `aarch64` K3.5 (`procsrv` + `shell`)** | `KT-338b` | 3 h | `BLOCKED` | Tách stub syscall theo arch trong `vifast-plat-user` để cùng nguồn Rust `procsrv`/`shell` biên dịch được cho `riscv64` & `aarch64`; ảnh sản phẩm `aarch64 --features product-shell` nạp ELF bằng `vifast-elf`, chạy thật `help`/`echo`/`ps`/`exit` qua PL011 stdin; ca âm không `product-shell` báo `khong_co_shell`; T2 PASS. |
| **KT-340a** | P1 | **`x86_64`: `VSpace` động + kernel W^X + `budget-x86_64.sh` + tách sản phẩm vs `ktest-x86_64` (`D2-08`)** | `KT-334, KT-336a` | 3 h | `READY` | Dựng `VSpace` động (physmap + `vifast-pte::PageTable`), tách W^X toàn ảnh (`.text` RX / `.data` RW, đột biến ghi `.text` sinh `#PF`); thêm `v2/test/boot/budget-x86_64.sh` (`e_machine = 62`, trần 262.144 B, báo riêng `.smp_stacks`, nối `--selftest` vào T2); tách binary sản phẩm (0 harness marker, bỏ 64 KiB `TaskStack` test). **Lưu ý D2-243**: nếu còn kịp nhánh thì dời luôn bảng trang boot/IST1/`__syscall_stack`/DMA page (`~80+ KiB`) ra `.smp_stacks`/physmap để `.data+.bss < 64 KiB` (nếu không kịp thì **bắt buộc hoàn tất ở `KT-340b`**); TAP cũ + `-cpu max` xanh; T2 PASS. |
| **KT-340b** | P1 | **`x86_64`: TLB shootdown đa hart (`invlpg` + IPI AP→BSP qua `Mailbox`) + đóng v1 **N5** + hạ `.bss` sản phẩm `< 64 KiB`** | `KT-340a` | 3 h | `BLOCKED` | Cài `invlpg` + IPI shootdown AP→BSP qua `vifast_sync::Mailbox` (cùng `KT-336c`/`KT-338a` hoàn tất 3/3 port đóng thật sự Audit Q1-A/`N7`); kiểm chứng `CR4.PCIDE`/TLB cũ trên `-cpu max` kèm đột biến bỏ shootdown/bỏ ack ⇒ ĐỎ; đóng chính thức lỗi v1 **N5** (`KNOWN-ISSUES.md` #5); **chặn cứng D2-243**: xác nhận `.data+.bss` ảnh sản phẩm `x86_64` **< 64 KiB** qua `budget-x86_64.sh` trước khi mở `KT-340c`; T2 PASS. |
| **KT-340c** | P1 | **`x86_64`: nối `vifast-core` & IPC ring 3 (thay scheduler riêng)** | `KT-340b, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` (`.data+.bss ≤ 256 KiB` đo bởi `budget-x86_64.sh`); nối `int 0x80` theo `[calling.x86_64]` v1.1 (`D2-242` **`7C-r13`**: `rax`=error, `rdx`=value, **`r13`=MessageInfo**, `r10/r8/r9/r12`=MR0..3) + `SyscallEnv::tcb_vspace -> VspaceOutcome` vào `core::syscall`; gỡ state machine scheduler trùng lặp ở `vifast-plat-x86_64/src/sched.rs` (giữ switch `iretq` 22-qword + TSS/IST1 + `McsLock` bọc IRQ); hart phụ `hlt` khi rảnh; TAP cũ + `core` + `ipc` ring 3 + `-cpu max` xanh; T2 PASS. |
| **KT-341** | P1 | **`x86_64` K3.5: `procsrv` + `shell` dùng chung `vifast-plat-user`** | `KT-339, KT-340c` | 3 h | `BLOCKED` | Ảnh sản phẩm `x86_64 --features product-shell` nạp `procsrv`/`shell` cùng nguồn Rust với `riscv64`/`aarch64`, chạy thật `help`/`echo`/`ps`/`exit` qua COM1 stdin; ca âm không `product-shell` ĐỎ; cả 3 arch dùng chung 1 codebase shell; T2 PASS. |
| **KT-342** | P1 | **Hợp nhất ảnh sản phẩm `riscv64`: gỡ `ktest` khỏi driver + chạy đồng thời FS, `netsrv`, `exec`** | `KT-335, KT-336a, KT-336c` | 3 h | `BLOCKED` | Tách nhãn feature driver (`net-drv`/`gpu-drv`/`input-drv`/`vcon-drv`) khỏi `ktest` trong `vifast-plat-riscv64`; tích hợp FS + `netsrv` U-mode + `exec` vào ảnh sản phẩm (bộ nhớ động Untyped/physmap, `.data+.bss ≤ 256 KiB`, 0 harness marker, giữ `IrqMcsLock` + `Mailbox` của `KT-336c`); 1 boot sản phẩm chạy cả `ls`/`cat`/`write` + `ping` qua `netsrv` + `run /hello.elf`; 3 đột biến đỏ; T2 PASS. |
| **KT-343** | P1 | **Đóng 11 lệnh `deferred-K7` không cần mở ABI (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`, `run`, `runfs`, `exec`)** | `KT-342` | 3 h | `BLOCKED` | Chuyển 11 lệnh trên từ `deferred-K7` → `active` trong `spec/v1-parity.toml` (**25/28 active**, không sửa `spec/abi.toml`); runner parity `--mode serial` xanh 25/25 trên ảnh sản phẩm ở cả `smp=1` và `smp=4`; ca âm từng nhóm lệnh mới; T2 PASS. |

### Đợt 3 — Hoàn tất tính năng ABI v1.1, tách build Userland & Nghiệm thu Chu kỳ 2

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-345b** | P1 | **Thi hành tính năng ABI v1.1: `free`/`df`/`loop` (28/28 parity) + `NetSvc` đa caller** | `KT-342, KT-343, KT-345a` | 3 h | `BLOCKED` | Nâng `netsrv` U-mode phục vụ ≥ 2 caller đồng thời theo `spec/protocols/socket.vidl` (`D2-242` `6A`: bảng phiên theo `(badge, handle)`, đầy ⇒ `Busy` 14, sai `badge`/`handle` ⇒ `InvalidCapability` 1, **kernel `NetSvc` giữ đơn caller**); cài `OP_STATFS` + dịch vụ `MemSvc` (`4C`) đóng 3 lệnh cuối `free`, `df`, `loop` trong `spec/v1-parity.toml` (**28/28 = 100 % active**); parity `--mode serial` xanh 28/28 ở `smp=1` và `smp=4`; đột biến đỏ + T2 PASS. |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184 (3 arch)** | `KT-341, KT-342` | 3 h | `BLOCKED` | Chuyển build ELF user ra cây `v2/user/` riêng kèm `--remap-path-prefix`, đóng gói ELF vào ảnh đĩa FS (`vifast-fsutil`) hoặc initrd chuẩn thay `include_bytes!` trong `build.rs`; thêm `v2/user` vào `VBREV_INPUT_ROOTS` (`build-rev.sh`) kèm **tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `build-rev.algo.pin`** và `wake_scope.py`; `repro-check.sh` chứng minh **cùng sha256 trên 2 thư mục build khác nhau cho ảnh sản phẩm canonical cả 3 arch** (Linux/macOS; Windows giữ nợ `v2.1` theo B2); shell + `exec` nạp ELF từ FS chạy thật; T2 PASS. |
| **KT-347** | P2 | **Thiết kế & PoC `U1a` (`no_std+alloc` `vifast-rt`) + hợp đồng `.vidl` `ViScript` + hướng `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `BLOCKED` | Chạy thật 1 ứng dụng user mẫu dùng `vifast-alloc` + lớp runtime tối thiểu (`U1a`, biên dịch bằng stable `1.100.0`) qua `exec` từ FS; lập `v2/docs/u1-uefi-readiness.md` chốt phân tầng `U1a`/`U1b`, hợp đồng `.vidl` & kịch bản kiểm chéo định kỳ trên máy đo cho liên repo `ViFastOs ↔ ViScript` (`VSH2`, không GitHub Actions theo D2-241), và phương án `UEFI` + khám phá phần cứng `DTB`/`ACPI` cho Chu kỳ 3; T2 PASS. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | `KT-331, KT-332, KT-333a, KT-333b, KT-333c, KT-336b, KT-336c, KT-337, KT-341, KT-343, KT-345b, KT-346` | 3 h | `BLOCKED` | Chạy độc quyền trên ứng viên tích hợp cuối: T2 PASS; `v2 nightly --all` (L1 100k × smp 1/2/4/8 + Miri + sinh `STATUS.md`) PASS; soak **ảnh sản phẩm có shell** (`riscv64` `--features product-shell,product-fs` + net/exec 1.000 kèm đợt **soak đĩa thật `virtio-blk`**, `aarch64` product-shell 1.000, `x86_64` product-shell 600 `qemu64` + 600 `-cpu max`) đều 0 lỗi (`qemu_foreign=0`, không retry); đối chiếu đủ **Bảng Exit Criteria `v2.0` (§6.2)**; xuất `v2/agent/reports/R2-v2.{md,json}` qua `check_report.py --all` + `verify-numbers.py`; xác nhận clone/checkout sạch Windows/macOS/Linux; trình Agent 0 chốt Chu kỳ 2 & quyết định mở `v2.0`. |

---

## 4. Vùng xung đột & quy tắc chạy song song (bài học Chu kỳ 1 + rà soát D2-236..D2-243)

| Vùng tệp nhạy cảm | Rủi ro | Quy tắc bắt buộc |
|---|---|---|
| `v2/bin/v2` (ID bước `lane_step` & chạy chồng cổng) | Trùng ID làm `lane_done` bỏ qua bước; chạy 2 `gate --fast` song song làm lệch worktree & sai số đo (`KT-333a`) | Chỉ **1 task mỗi vòng** thêm bước vào `v2/bin/v2` (chú ý **`KT-333c`** vs **`KT-345a`** không sửa `v2/bin/v2` song song); kiểm trùng `lane_step` trên cả `do_gate_fast`/`do_gate_dev`; **`KT-333c`** khoá độc quyền máy đo bằng lockfile nguyên tử. |
| GitHub Actions / `.github/` | Sinh bằng chứng không hợp lệ (không QEMU/máy đo) mà vẫn xanh | **Cấm vĩnh viễn** tạo `.github/workflows/**` (`D2-241`, `AGENTS.md` §4.11). Mọi chạy định kỳ dùng script + bộ hẹn giờ trên máy đo. |
| `v2/tools/check-wake-paths.py` (`SIM_KOBJ`) & `wake_scope.py` | Thêm `v2/user/` (`KT-346`) làm gãy/lọt luật `wake` | `KT-336b` đã đồng bộ `SIM_KOBJ_FILES` (14 tệp); **`KT-346`** bắt buộc cập nhật `WORKSPACES`/`NON_CRATE_SOURCE_TREES` không để thoái lui hợp đồng `D2-190`/`D2-205`. |
| `v2/test/boot/build-rev.sh` & `build-rev.algo.pin` | Thêm `v2/user` mà không tăng `VBREV_ALGO` bị `check-algo-pin.sh` từ chối | Chỉ **`KT-346`** được sửa `VBREV_INPUT_ROOTS` và bắt buộc tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `build-rev.algo.pin` trong **cùng commit** (D2-239). |
| `v2/lib/vifast-sync` (`McsLock`) | `McsLock::lock()` không tự tắt IRQ ⇒ self-deadlock nếu dùng trong cả IRQ & normal | Bắt buộc bọc guard lưu/tắt IRQ (`cli`/`sstatus.SIE`/`DAIF`) ở lớp `vifast-plat-*` cho mọi khoá đụng ngắt (`KT-336a` đã xong `x86_64`, **`KT-336c`** hoàn tất `riscv64` — D2-238/D2-243). |
| `v2/kernel/vifast-core/src/machine.rs` & pin `196.608` | Hai nhánh cùng sửa struct hoặc pin | Không để 2 worktree sửa `vifast-core` cùng lúc nếu đều đổi layout `Machine`/`TcbData`. |
| `v2/agent/QUEUE.md`, `v2/agent/STATE.md` | Xung đột merge, agent ghi vượt quyền | **Chỉ lead (Agent 1)** cập nhật `QUEUE.md` (lật `DONE`/`READY` đúng DAG D2-235) và làm mới khối trạng thái đầu `STATE.md` khi merge về `main`. |
| `v2/test/boot/*` & chạy QEMU song song | `qemu_foreign > 0` làm soak `MEASUREMENT_INVALID` | **Soak (`KT-348`) luôn chạy một mình trên máy đo**; các task dùng QEMU khác tối đa 3 QEMU tổng toàn máy, cấm `pkill -f qemu-system`. |
| `spec/abi.toml`, `spec/kernel.toml` | Vỡ đóng băng ABI | **Chỉ `KT-345a`** được sửa `spec/` đúng 9 mục duyệt tại **D2-242**. Ngay sau khi `KT-345a` `DONE`, `spec/` khoá cứng trở lại theo D2-234. |

---

## 5. Hướng dẫn khởi động ngay cho các Agent sau khi `git pull`

```bash
git pull origin main
git status -s                                              # phải sạch trên cả Linux/macOS/Windows
cargo run -q --manifest-path v2/Cargo.toml -p vifast-agentctl -- report --project v2
bash v2/tools/status.sh
```

1. **Ưu tiên số 1 trên đường găng — `KT-345a` (`READY`, đã mở khoá theo D2-242)**: áp đúng 9 mục đã duyệt (`1C, 2C, 3.1A+3.2A, 4C, 5A, 6A, 7C-r13, 8C, 9B`, giữ `abi_version = 1`, `min_compat = 1`) + đổi `SyscallEnv::tcb_vspace -> VspaceOutcome` kèm rollback để mở ngay `KT-338b` & `KT-340c`. Lưu ý **không** để `KT-345a` sửa `v2/bin/v2` song song cùng `KT-333c`.
2. **Đợt 1 còn lại & vá sót Đợt 0–1 (`READY` song song)**: **`KT-335`** (`virtio_net` + TCP vào `vifast-net`), **`KT-336c`** (`riscv64` `IrqMcsLock` + `Mailbox` + `k3b-core.md` `ObjStore` — đủ 3/3 port đóng Audit Q1-A), và **`KT-333c`** (khoá độc quyền `v2/bin/v2`/`soak*.sh` + nối `gpu-x86_64.sh` vào T2 + chuẩn bị `v2 nightly --soak` cho `KT-348`).
3. **Đợt 2 đang chạy (`KT-338a` ∥ `KT-340a`)**: đảm bảo ảnh sản phẩm cả `aarch64` và `x86_64` đều hạ `.data+.bss < 64 KiB` trước khi link `vifast-core::Machine` (`184.792 B`) ở `KT-338b`/`KT-340c` (với `x86_64`, hoàn tất ngay trong `KT-340a` nếu còn kịp nhánh hoặc chặn cứng ở `KT-340b` theo D2-243).
4. **Dọn sổ trạng thái**: Agent 2 làm mới `v2/agent/STATE.md` (gỡ các mục đã `DONE` khỏi `NỢ MỞ`) ở lần merge kế tiếp.
5. Mỗi task hoàn tất phải có: **test đỏ trước** + **đột biến bắt buộc** + **ca âm** + **T1/T2 theo `AGENTS.md` §6.1** + **commit có nhãn `[KT-###]`** (không retry, không nới ngưỡng, không commit raw `boot-*.log` của soak, không tạo `.github/workflows/**` theo D2-241).

---

## 6. Khung lộ trình dài hạn (Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`) & Tiêu chí phát hành `v2.0` (**D2-240/D2-241**)

### 6.1 Bảy trụ cột chiến lược sau Chu kỳ 2 (Chu kỳ 3 trở đi)

| # | Trụ cột | Mốc đề xuất | Thực trạng hiện tại & việc bắt buộc phải làm |
|---|---|---|---|
| 1 | **SMP Preemptive thật (`S1 → S2 → S3`)** | `SMP-1`, `SMP-2` | Ảnh sản phẩm `riscv64` hiện vẫn chạy vòng hợp tác `proc_rt` trên 1 hart (`rust_secondary_main` park vô hạn) với 1 BKL. Cần đưa `vifast-core::Machine::dispatch` + timer preempt + `vifast-sync::Mailbox` + `McsLock` bọc IRQ chạy U-mode thật trên 4 hart ở cả 3 arch (`SMP-1`), rồi tách trạng thái nóng sang `PerHart` (`SMP-2`); chỉ mở `S3` khi `vifast-bench` đo chờ khoá > 10 % ở `smp=4`. |
| 2 | **Hợp nhất `Machine` (đóng `D2-83`) & `vifast-objmem` (D2-14)** | `MEM-1`, `MEM-2` | Kernel còn 2 `Machine` song song (`vifast-core::Machine` 184,8 KiB + `vifast-plat-common::obj::Machine` 64 mappings) và trần tĩnh `MAX_OBJECTS=256`, `MAX_TCBS=64` sẽ chặn `U1`/`VSH2`/`G2`. Cần rút Sv39/PLIC khỏi `vifast-plat-common`, đưa `TableMemory` lên `vifast-pte` (`MEM-1`), và chuyển cấp phát đối tượng vào `Untyped` thật qua `vifast-objmem` để `.data+.bss` < 64 KiB và quy mô theo RAM (`MEM-2`). |
| 3 | **4 nền tảng kiểm chứng/quan sát trong thiết kế v2** | `OBS-1`, `VER-1` | Hoàn thiện những mảnh đã cam kết nhưng còn trống: (i) **trace nhị phân 32 B luôn bật** + dump `pmemsave` + `vifast-trace` (**D2-09**, §9.11); (ii) **mutation testing tự động** `v2/test/mutants.toml` + `v2 mutants` trong `nightly` (§9.9); (iii) **`vifast-bench` + `test/perf/history.csv`** dưới `-icount` làm cổng dữ liệu cho `syscall`/`sysret`, `S2/S3`, `F1` (§9.10); (iv) **conformance `.scn` đủ 3 arch** (D2-13) + **MCS refill đa đoạn/`SetTimeoutEp`** (D2-16). |
| 4 | **Driver từ Polling → IRQ `Notification` → Userland + IOMMU** | `DRV-1`, `D2`, `D3` | Chuyển driver VirtIO từ busy-poll sang ngắt thật qua **`IrqControl`/`IrqHandler` + `Notification`** (mở ở ABI v1.2); làm nền cho driver userland (`D2`: PCIe MSI-X, NVMe, AHCI) kèm `Frame.MapIo` + **`IOSpace` (VT-d / SMMUv3 / virtio-iommu, `D3`)** để cô lập DMA. |
| 5 | **An toàn, FPU/SIMD & chuẩn hoá `.vidl`** | `SEC-1`, `IDL-1` | Lưu/khôi phục **ngữ cảnh FPU/SIMD theo TCB** (`fs0..fs31`, NEON `v0..v31`, `fxsave`/`xsave` — bắt buộc trước `U1` để tránh hỏng thanh ghi giữa 2 tiến trình Rust/ViScript); bộ test đối kháng `copy_from_user`/`copy_to_user` + bật `SMEP`/`SMAP`/`PAN` + chính sách KPTI tuỳ chọn; fuzz `vifast-net` + nguồn entropy `virtio-rng`; chuẩn hoá `proc.vidl`, `fs.vidl`, `net.vidl`, `term.vidl` trong `spec/protocols/`. |
| 6 | **`U1` hai tầng & Hợp đồng liên repo `ViFastOs ↔ ViScript` (`VSH2`)** | `U1a`, `U1b`, `VSH2` | Chia `U1` thành **`U1a`** (`no_std + alloc` + `vifast-rt` qua `.vidl`, chạy ngay trên **stable `1.100.0`** để kéo `vis-host-vifast`/ViShell lên sớm) và **`U1b`** (Rust `std` PAL target `*-unknown-vifastos`); chốt hợp đồng phiên bản `abi_version`/`.vidl` và **script kiểm chéo định kỳ trên máy đo** (`v2 nightly` / bộ hẹn giờ nội bộ, không dùng GitHub Actions theo **D2-241**) giữa repo `ViFastOs` và repo [`ViScript`](VISCRIPT.md). |
| 7 | **Khám phá phần cứng thật (`HW1`) & Bền vững công cụ** | `HW-0`, `UEFI`, `HW1` | Trước `UEFI`/`D2`/`HW1`, bổ sung mốc **`HW-0`**: phân tích **DTB thật** (`riscv64`/`aarch64`) và **ACPI `MADT`/`FADT` + bản đồ bộ nhớ E820/UEFI** (`x86_64`) thay hằng số QEMU `virt` gán cứng. Đồng thời **đóng băng bề mặt parser Python** (`check-wake-paths.py` đã ~498 KiB — ưu tiên ép bất biến bằng hệ kiểu Rust như `NonReadyTarget`) và quyết định chính sách lưu trữ/nén 8.985 raw log Chu kỳ 1 trong `JOURNAL/` trước khi công khai `v2.0`. |

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 shell** (`riscv64`/`aarch64`/`x86_64`, tách sản phẩm/harness D2-08) | **Có** (`KT-338a..KT-342`) | `SMP-1/2` preemptive 4 hart đầy đủ trên ảnh sản phẩm |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active`** (`KT-343` + `KT-345b`, `smp=1` & `smp=4`) | Mở rộng lệnh `ViShell` (`VSH2`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có** (đóng nốt **N5** ở `KT-340b` và **Audit Q1-A/N7** đủ 3 port qua `KT-336a`/`336c`/`338a`/`340b`) | — |
| **Ngân sách & Cổng** (`.data+.bss ≤ 256 KiB` cả 3 arch, `Machine ≤ 196.608 B`, T2 PASS, `v2 nightly --all` L1 400k + Miri PASS) | **Có** (`budget.sh`, `budget-aarch64.sh`, `budget-x86_64.sh`, `KT-333b`, `KT-333c`, `KT-348`) | `v2 mutants` + `vifast-bench` tự động trong nightly |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`) | **Ảnh sản phẩm có shell** 3 arch (`rv 1000`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) + **soak đĩa thật `virtio-blk`** (`KT-348`) | Soak đa tiến trình `U1`/mạng tải nặng |
| **Tái lập byte (P20)** | **Đạt cho ảnh sản phẩm canonical cả 3 arch trên Linux + macOS** (`KT-346`) | Hội tụ trên host **Windows** (B2 / D2-122.4) |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md` tự sinh** (`KT-333b`), clone/checkout sạch Win/macOS/Linux (`KT-331`) | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **Động hóa bộ nhớ, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | Có tài liệu định hướng & PoC `U1a` (`KT-347`) | Thi công ở **Chu kỳ 3 (`v2.1+`)** theo §6.1 |

