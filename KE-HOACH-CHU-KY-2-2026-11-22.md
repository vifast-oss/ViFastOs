# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở ban đầu tại `1559339e`; **hiệu đính bổ sung D2-236..D2-240** tại `HEAD` `5e635474` sau khi vòng 25 `KT-328`/`KT-329` đã `DONE`).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-240**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md).

> **Mục tiêu một câu**: Khép soak vòng 25 (`KT-330`), chấm dứt trạng thái "mỗi kiến trúc một nhánh rời / mỗi tính năng một `ktest-*` riêng" — **khử nhân bản driver trên 3 kiến trúc** (`v2/lib/vifast-virtio` + `vifast-net`), **giải nghẽn `.bss` & đóng lỗi v1 N5 + Audit Q1-A**, **nối `vifast-core` + `shell` dùng chung lên `aarch64` và `x86_64`**, **hợp nhất ảnh sản phẩm `riscv64`** (FS + Net + Exec, đạt **28/28** lệnh parity v1), **hạ thời gian cổng & dựng `v2 nightly` thật**, **mở một cửa sổ ABI v1.1 duy nhất (`KT-344` → `APPROVED` → `KT-345a`)**, và chốt khung **Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`**.

---

## 0. Chẩn đoán đầu vào (căn cứ từ mã nguồn `HEAD` `5e635474`)

Chu kỳ 1 (`KT-01..KT-329`, 25 vòng đã hoàn tất) đạt kỷ luật kiểm chứng rất cao (T2 `gate --fast` PASS 68/68, soak 3 arch `9e18880d` 0 lỗi, K6 đóng, luật `wake` đóng, 3 arch đều boot thật trên QEMU). Đối chiếu trực tiếp `Cargo.toml`, `spec/abi.toml`, cây nguồn và cổng đo lộ ra **10 điểm nghẽn cấu trúc** (7 gốc ở D2-229 + 3 phát hiện bổ sung ở D2-236..D2-238) buộc phải xử lý dứt điểm trong Chu kỳ 2:

1. **`aarch64` (K4') và `x86_64` (K5') chưa nối `vifast-core`**: `vifast-kbin-aarch64` và `vifast-kbin-x86_64` chưa phụ thuộc `vifast-core`, `vifast-elf`, `vifast-vfs`, `vifast-fs`; cả hai vẫn là 1 binary selftest duy nhất (chưa tách ảnh sản phẩm vs harness theo D2-08). `x86_64` đang phát triển một bộ lập lịch độc lập tách rời `vifast-core::Machine`.
2. **Nghẽn toán học `.data+.bss ≤ 256 KiB` khi nối `vifast-core` (D2-236)**: `aarch64` đã dùng **233.704 / 262.144 B (89,2 %)** do `BootMemory::ARENA` (`44` frame = `180.224 B`) và 4 trang DMA (`16 KiB`) nằm trong `.bss`; `x86_64` chưa có `budget-x86_64.sh` và giữ 4 `TaskStack` tĩnh (`64 KiB`) trong `.bss`. Cộng `Machine` (`184.792 B`) sẽ vượt trần I7 nếu không dời arena/DMA ra RAM ngoài `.bss` trước.
3. **Xung đột thanh ghi trong `[calling.x86_64]` & thiếu mục đa arch ở `spec/abi.toml` (D2-237)**: `[calling.x86_64]` đặt `return_value = "rdx"` **trùng** `message_info = "rdx"` (không trả đồng thời được cả 2 trong `Recv`/`ReplyRecv`), thiếu `return_message_info`, và đặt `r12` vừa là `MR3` vừa nằm trong `preserved` trong khi `clobbered` chỉ ghi `["rax", "rdx"]`. Ngoài ra `spec/abi.toml` thiếu `[debug_info].return_channel` và `[object_size]` cho `aarch64`/`x86_64`, và `VSpace`/`PageTable`/`Frame` vẫn ghi `reserved/unsupported=true`.
4. **`x86_64` thiếu `VSpace` động + W^X kernel + TLB shootdown & lỗi v1 N5 chưa đóng (D2-238)**: `vifast-plat-x86_64` chưa có `vspace.rs`/`wx.rs`/`tlb.rs`/`shootdown.rs` (kernel vẫn identity-map RWX, user chỉ có 2 trang tĩnh); lỗi v1 **N5** (`x86 AP→BSP + PCID`, `KNOWN-ISSUES.md` #5) hoãn sang K5' nhưng chưa có task đóng.
5. **Phân mảnh 14 binary trên `riscv64`**: `vifast-kbin-riscv64` có 13 binary `ktest-*`; các feature `net`/`gpu`/`input`/`vcon` của `vifast-plat-riscv64` vẫn kéo theo `ktest`; ảnh sản phẩm `product-fs` chưa có `exec`/`netsrv`/mạng và còn 14/28 lệnh `deferred-K7` trong `spec/v1-parity.toml` (gồm cả `run`/`runfs`/`exec`).
6. **Nhân bản driver lớn & crate mồ côi (`vifast-sync`, `vifast-objmem`) + bẫy `SIM_KOBJ`**: `pci.rs`, `virtio_blk.rs`, `virtio_net.rs`, `virtio_gpu.rs`, `virtio_input.rs`, `tcp.rs` lặp giữa 3 `vifast-plat-*` (~11.000+ dòng). `vifast-sync` có **0 consumer** trong `v2/kernel`/`v2/sim` (và `McsLock::lock()` không tự tắt IRQ); `vifast-objmem` chỉ dùng ở `vifast-sim`; `v2/sim/vifast-sim/src/kobj.rs` dài 11.443 dòng và bị ghim cứng `SIM_KOBJ` tại 6 chỗ trong `check-wake-paths.py`.
7. **Cần khép soak sau vòng 25 + `-cpu max`**: `KT-328` (`x86_64`) và `KT-329` (`riscv64`) đã `DONE` và đổi ảnh (`aarch64` không đổi tệp nào từ `9e18880d`) ⇒ chạy độc quyền **`KT-330`** (`READY` ngay) trước Đợt 1.
8. **Cổng T2 phình thời gian & thiếu `nightly`/CI/`deny.toml`/`THIRD-PARTY.md`**: `gate --fast` ~13 phút; `v2 nightly` đầy đủ trả `exit 2`; chưa có CI host, `deny.toml` và `THIRD-PARTY.md` (D2-21).
9. **Build userland qua `build.rs` + `include_bytes!` không mở rộng sang `U1`/`VSH2`** và là gốc nợ P20/A1-184 (đường dẫn host lọt vào ELF user; `repro-build.sh` mới ghim `riscv64`; `build-rev.sh` chưa gồm `v2/user/`).
10. **Vệ sinh repo, sổ sách & tương thích đa nền tảng**: 4 tệp `kt219-logs/cases/` lỗi Windows đã được Agent 0 sửa trực tiếp tại `a2378b43` (`KT-331` dựng máy kiểm chống tái diễn); `JOURNAL/` tích tụ gần 9.000 raw log; `README.md`/`v2/README.md`/`STATE.md`/`BLOCKED.md` cần đồng bộ (`KT-332`); `v2/kernel/Cargo.toml` ghi `rust-version = "1.99.1"` lệch `1.100.0`.

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
| **D2-237** | **Mở rộng `KT-344` lên 9 nhóm & đẩy `KT-345a` (spec/gen) lên TRƯỚC `KT-338b`/`KT-340c`** | Sửa xung đột `rdx`/`r12` ở `[calling.x86_64]` (tách `return_message_info`, làm rõ `clobbered`/`preserved`), bổ sung `[debug_info].return_channel` + `[object_size]` 3 arch, nâng trạng thái `VSpace`/`PageTable`/`Frame`. Tách `KT-345a` (spec/gen — tiền đề của `KT-338b`/`KT-340c`, chỉ Agent 0 mở) và `KT-345b` (`free`/`df`/`loop` + `NetSvc` đa caller). |
| **D2-238** | **Chèn `KT-340a`/`KT-340b` (`VSpace`/W^X/shootdown + đóng v1 `N5`), tách task > 3 h & sửa bẫy `KT-336`/DAG `KT-348`** | Tách `KT-333a/b`, `KT-336a/b`, `KT-338a/b`, `KT-340a/b/c` để mọi task ≤ 3 h (`AGENTS.md` §10). Bắt buộc bọc lưu/tắt IRQ khi dùng `McsLock` cho tài nguyên đụng ngắt; nối `vifast_sync::Mailbox` đủ 3 port (`KT-336a`/`338a`/`340b`) để đóng thật sự Audit Q1-A (`N7`); đồng bộ `SIM_KOBJ` trong checker `wake` khi tách `kobj.rs` (`KT-336b`); vá đủ phụ thuộc DAG `KT-348`; lật `KT-330` → `READY`. |
| **D2-239** | **Chốt chi tiết `KT-334` (`vifast-virtio` + cache DMA), `KT-343` (25→28/28), `KT-346` (`VBREV_ALGO`) & soak `KT-348`** | Đặt `MmioBus`/`DmaPool` (có `sync_for_device/cpu` bảo toàn `dc civac` `aarch64`) trong `v2/lib/vifast-virtio`; `KT-343` lật 11 lệnh (gồm `run`/`runfs`/`exec` → 25/28; `KT-345b` → 28/28); `KT-346` thêm `v2/user` vào `build-rev.sh` + tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `wake_scope.py` + kiểm tái lập 3 arch Linux/macOS; `KT-348` soak **ảnh sản phẩm có shell** 3 arch + đợt **soak đĩa thật `virtio-blk`**. |
| **D2-240** | **Khung lộ trình dài hạn Chu kỳ 3 & Bảng Exit Criteria phát hành `v2.0` vs `v2.1`** | Chốt 7 trụ cột Chu kỳ 3 (`SMP-1/2`, `MEM-1/2`, nền kiểm chứng D2-09/mutants/`vifast-bench`/conformance 3 arch, IRQ `Notification` → IOMMU, FPU/SIMD + `copy_from_user` + `.vidl`, `U1a`/`U1b` + liên repo `ViScript`, DTB/ACPI cho `HW1` + đóng băng parser Python) và tiêu chí tường minh mở `v2.0` (§6). |

---

## 2. Lộ trình 4 đợt & đồ thị phụ thuộc (`KT-330..KT-348`, 25 task ≤ 3 h)

```text
[VÒNG 25 — ĐÃ DONE TẠI 5e635474: KT-328 (x86_64 ICMP+DNS) & KT-329 (Suspend Defer + Resume)]
        │
        ▼
[ĐỢT 0 — Khép soak vòng 25, vệ sinh, hạ tầng cổng & RFC ABI v1.1 (tất cả READY ngay)]
  Độc quyền khi đo soak (qemu_foreign=0):
    KT-330 (soak cuối vòng 25: rv 1000 + x64 600 qemu64 + x64 600 -cpu max)
  Song song (ngoài v2/kernel & v2/lib, không chạy QEMU trùng KT-330):
    KT-331  (guard NTFS + chính sách JOURNAL)
    KT-332  (đồng bộ README/STATE/BLOCKED/báo cáo)
    KT-333a (profile/tối ưu T2 + CI host)
    KT-333b (v2 nightly thật + deny.toml + THIRD-PARTY.md)
    KT-344  (RFC Gói ABI v1.1 — 9 nhóm, KHÔNG sửa spec/, đầu ra REVIEW chờ Agent 0)
        │                                                           │
        │ (sau KT-330)                                              │ (sau APPROVED Agent 0)
        ▼                                                           ▼
[ĐỢT 1 — Khử nhân bản & hợp nhất tầng dùng chung]              KT-345a (áp spec/ + vifast-gen)
  ├── KT-334  (MmioBus/DmaPool+sync trong v2/lib/vifast-virtio      │   [CẤM TỰ MỞ — tiền đề
  │            + hợp nhất pci & virtio_blk 3 arch + MSRV 1.100.0)   │    của KT-338b & KT-340c]
  │     ├──► KT-335  (virtio_net 3 arch + TCP/client vào vifast-net)│
  │     └──► KT-337  (virtio_gpu/input + mở virtio-gpu x86_64 PPM)  │
  ├── KT-336a (McsLock bọc IRQ + Mailbox vào kernel — bước 1 Q1-A)  │
  └── KT-336b (tách sim/src/kobj.rs + sửa SIM_KOBJ wake + R1 + ObjStore)
        │                                                           │
        ▼                                                           ▼
[ĐỢT 2 — Giải nghẽn .bss, đóng N5/Q1-A, một lõi 3 arch & Hợp nhất ảnh sản phẩm riscv64]
  ├── (KT-334, KT-336a) ──► KT-338a (aarch64: dời .bss ra RAM + tách product/ktest + Mailbox)
  │                           └──► (KT-338a, KT-345a) ──► KT-338b (aarch64: vifast-core + IPC EL0)
  │                                                         └──► KT-339 (vifast-plat-user + aarch64 shell)
  ├── (KT-334, KT-336a) ──► KT-340a (x86_64: VSpace + W^X + budget-x86_64 + tách product/ktest)
  │                           └──► KT-340b (x86_64: TLB shootdown AP→BSP/Mailbox + đóng N5)
  │                                 └──► (KT-340b, KT-345a) ──► KT-340c (x86_64: vifast-core + IPC ring 3)
  │                                                               └──► (KT-339, KT-340c) ──► KT-341 (x86_64 shell)
  └── (KT-335, KT-336a) ──► KT-342 (riscv64: gỡ ktest khỏi driver + ảnh sản phẩm FS+Net+Exec)
                              └──► KT-343 (đóng 11 lệnh K7 không cần ABI gồm run/runfs/exec → 25/28 active)
        │
        ▼
[ĐỢT 3 — Hoàn tất ABI v1.1 (28/28), tách Userland v2/user/ & Nghiệm thu Chu kỳ 2]
  ├── (KT-342, KT-343, KT-345a) ──► KT-345b (free/df/loop → 28/28 parity + NetSvc đa caller)
  ├── (KT-341, KT-342)          ──► KT-346  (tách v2/user/ + tăng VBREV_ALGO kt134-v5 + đóng P20 3 arch)
  │                                   └──► KT-347 (PoC U1a + hợp đồng .vidl ViScript + UEFI/DTB/ACPI)
  └── (KT-331, KT-332, KT-333a, KT-333b, KT-336b, KT-337, KT-341, KT-343, KT-345b, KT-346)
                                ──► KT-348  (Cổng nghiệm thu Chu kỳ 2: soak sản phẩm 3 arch + đĩa thật
                                             + T2 + nightly --all + Exit Criteria v2.0 + báo cáo R2-v2)
```

---

## 3. Chi tiết từng đợt & bảng task (mọi task ≤ 3 h)

### Đợt 0 — Khép vòng 25, vệ sinh, hạ tầng cổng & RFC ABI v1.1

> **Luật thực thi Đợt 0**: `KT-331`, `KT-332`, `KT-333a`, `KT-333b`, `KT-344` **không** sửa `v2/kernel/**` hay `v2/lib/**` (không làm bẩn `build-inputs`). Khi chạy **`KT-330`** (soak), máy đo phải chạy **độc quyền** (`qemu_foreign=0`, D2-116).

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-330** | P0 | **Soak ứng viên cuối vòng 25**: `riscv64` 1.000 + `x86_64` 600 (`qemu64`) + `x86_64` 600 (`-cpu max`) | `KT-328, KT-329` | 3 h | `READY` | 0 lỗi cả 3 đợt soak (`rc_124=0`, `qemu_foreign=0` hai mẫu, `invalid=0`, không retry D2-07; `aarch64` không đổi từ `9e18880d`); chỉ commit `summary.txt` + `build-inputs`/`BUILD_ID` (D2-230); cập nhật `STATE.md`. |
| **KT-331** | P0 | **Guard tên tệp Windows/NTFS + chính sách `JOURNAL/`** | — | 2 h | `READY` | Thêm kiểm tên tệp tương thích NTFS (cấm `[<>:"\|?*]`, tên thiết bị Windows, dấu chấm/trắng cuối tên) vào `v2/tools/git-guard/pre-commit` và `v2/bin/v2` `rules`; `--selftest` ca âm ĐỎ đúng mã + ca dương XANH; `.gitignore` chặn raw `run-soak-*/boot-*.log`; **không** sửa `v2/kernel`/`v2/lib`. |
| **KT-332** | P0 | **Đồng bộ tài liệu công khai, cắt gọn `STATE.md` & dọn `BLOCKED.md`** | — | 2 h | `READY` | Cập nhật `README.md`, `v2/README.md` khớp thực tế sau vòng 25 (không số đo viết tay); đính chính đầu trang + §5 của `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md`; tách lịch sử KT-01..KT-329 của `STATE.md` sang `v2/agent/archive/STATE-chu-ky-1.md` (`STATE.md` < 30 KiB); dọn mục đã đóng ở `BLOCKED.md`; `v2 check` + `v2 check-rules` PASS. |
| **KT-333a** | P1 | **Profile & hạ wall-time T2 (`gate --fast`) + dựng CI host** | — | 3 h | `READY` | Profile 68 bước T2 và tối ưu làn chạy song song (giảm wall-time, **0 bước bị bỏ**); thêm `.github/workflows/v2-host-ci.yml` (chạy `v2 check` + L0 + L1-smoke); kiểm trùng ID `lane_step` trên cả `do_gate_fast`/`do_gate_dev`; **không** sửa `v2/kernel`/`v2/lib`; T2 PASS. |
| **KT-333b** | P1 | **Dựng `v2 nightly` thật + `deny.toml` + `THIRD-PARTY.md` (D2-21)** | — | 3 h | `READY` | Cài `v2 nightly` (`--all`/`--l1`/`--only-miri`/`--soak`/`--status`/`--dry-run`/`--selftest`, sinh `STATUS.md` máy-đo-được); thêm `deny.toml` allowlist MIT/Apache-2.0 + bước kiểm không registry dep ngoài allowlist + ca âm + sinh tự động `THIRD-PARTY.md` (D2-21, không `NOTICE`); **không** sửa `v2/kernel`/`v2/lib`; T2 PASS. |
| **KT-344** | P1 | **RFC Gói duyệt ABI v1.1 (`review/agent0/GOI-ABI-V1_1.md`) — 9 nhóm, KHÔNG sửa `spec/`** | — | 3 h | `READY` | Trình đủ **9 nhóm** (D2-234/D2-237): (1) `spec/kernel.toml`, (2) `extra_caps_unwrapped`, (3) kênh lỗi VSpace/ô 0 CSpace, (4) `MemInfo`/`STATFS`, (5) sửa xung đột `[calling.x86_64]` (`return_value`/`message_info` cùng `rdx`, tách `return_message_info`, `r12` vs `preserved`/`clobbered`) + GDT `syscall`/`sysret`, (6) `[debug_info].return_channel` + `[object_size]` 3 arch, (7) trạng thái `VSpace`/`PageTable`/`Frame` trong `[object]`, (8) socket IDL `NetSvc` đa caller, (9) đánh giá `min_compat = 1` + `.scn` nháp. Hoàn tất chuyển **`REVIEW`** chờ Agent 0 ký `APPROVED`. |

### Đợt 1 — Khử nhân bản driver, hợp nhất tầng dùng chung & Áp `spec/` ABI v1.1

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-334** | P1 | **Transport `virtio-pci` (`MmioBus`/`DmaPool` + cache hook) trong `v2/lib/vifast-virtio` + `pci`/`virtio_blk` 3 arch + MSRV** | `KT-330` | 3 h | `BLOCKED` | Đặt `MmioBus`/`DmaPool` (**bắt buộc** có `sync_for_device`/`sync_for_cpu` giữ `dc civac`/`dsb` của `aarch64` KT-277) và transport `pci`/`virtio_blk` thuần (`forbid(unsafe_code)`) trong **`v2/lib/vifast-virtio`**; 3 crate `vifast-plat-*` chỉ còn adapter MMIO/DMA/cache mỏng (D2-17); đồng bộ `v2/kernel/Cargo.toml` `rust-version = "1.100.0"`; tổng dòng `pci.rs`+`virtio_blk.rs` giảm ≥ 45%; host test + Miri + đột biến đỏ; TAP 3 arch + T2 PASS. |
| **KT-335** | P1 | **Hợp nhất `virtio_net` 3 arch + chuyển TCP/client state machine về `v2/lib/vifast-net`** | `KT-334` | 3 h | `BLOCKED` | Xoá 2 bản `tcp.rs` nhân bản trong `vifast-plat-{riscv64,aarch64}`; chuyển toàn bộ state machine TCP/DHCP/ARP/ICMP/DNS dùng trait `Clock` vào `vifast-net` (≥ 25 host test + ca âm timeout/reset/checksum); 3 arch dùng transport `KT-334` (giữ đủ TAP ICMP+DNS từ `KT-328`); `net.sh` + aarch64 TAP + x86_64 TAP + T2 PASS. |
| **KT-336a** | P1 | **Nối `vifast-sync` (`McsLock` bọc IRQ + `Mailbox`) vào `v2/kernel` — bước 1 đóng Audit Q1-A (`N7`)** | `KT-330` | 3 h | `BLOCKED` | Thay spinlock tự chế (`core_bridge::LOCKED` `riscv64`, `COM1`/scheduler `x86_64`) bằng `vifast_sync::McsLock` **bọc guard lưu/tắt IRQ** phía platform (`cli`/`sstatus.SIE`/`DAIF`, cấm gọi trực tiếp không tắt IRQ); nối `vifast_sync::Mailbox` cho kênh wake/IPI `riscv64` (bước 1 đóng Q1-A; hoàn tất 3/3 port ở `KT-338a`/`KT-340b`); đột biến bỏ khoá / tái nhập IRQ ⇒ ĐỎ; T2 PASS. |
| **KT-336b** | P1 | **Tách `v2/sim/vifast-sim/src/kobj.rs` (11.443 dòng) + đồng bộ checker `wake` + R1 `sim/src/` + `ObjStore`** | `KT-330` | 3 h | `BLOCKED` | Tách `kobj.rs` về module theo miền ≤ 1.500 dòng/tệp; cập nhật đồng bộ hằng `SIM_KOBJ` ở `check-wake-paths.py` (6 vị trí) & `check-wake-depinfo.py` + `CHECKER-WAKE-CONTRACT.md`, chứng minh **0 thoái lui hợp đồng D2-190/D2-205**; mở rộng R1 phủ `v2/sim/vifast-sim/src/`; tài liệu hoá `ObjStore` & đường chuyển `slotmem`→`vifast-objmem` trong `v2/docs/k3b-core.md`; Miri + L0 + L1 + parity + T2 PASS. |
| **KT-337** | P1 | **Hợp nhất `virtio_gpu` + `virtio_input` qua `MmioBus`/`DmaPool` & mở `virtio-gpu` x86_64** | `KT-334` | 3 h | `BLOCKED` | Rút `virtio_gpu` và `virtio_input` về `v2/lib/vifast-virtio` (giữ `sync_for_device/cpu`); giữ xanh `gpu.sh`/`input.sh` (`riscv64`) + `aarch64` (PPM 23/23); bật `virtio-gpu` 2D trên `x86_64` có `screendump` PPM thật + đột biến `offset=0` đỏ ở PPM; T2 PASS. |
| **KT-345a** | P1 | **Áp `spec/` ABI v1.1 + `vifast-gen` sau `APPROVED Agent 0` (tiền đề cho `KT-338b`/`KT-340c`)** | `KT-344` | 2 h | `BLOCKED` | **CẤM TỰ MỞ — chỉ Agent 0 mở sau dòng `APPROVED <YYYY-MM-DD> Agent 0` cho `KT-344` trong `QUEUE.md`**. Áp đúng mục duyệt vào `spec/abi.toml` + tạo `spec/kernel.toml`, chạy `vifast-gen`, cập nhật `check_abi.py`/`check_reserved.py`/`check_scn.py` + đột biến đỏ + T2 PASS. |

### Đợt 2 — Giải nghẽn `.bss`, đóng N5/Q1-A, một lõi `vifast-core` 3 arch & Hợp nhất sản phẩm `riscv64`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-338a** | P1 | **`aarch64`: giải phóng `.bss` (`BootMemory`/DMA ra RAM) + tách sản phẩm vs `ktest-aarch64` (`D2-08`) + `Mailbox`** | `KT-334, KT-336a` | 3 h | `BLOCKED` | Chuyển `BootMemory::ARENA` (180 KiB) + 4 trang DMA tĩnh ra RAM ngoài `.bss` (qua physmap như `KT-279`) để `.data+.bss` < 64 KiB (đủ chỗ cho `Machine`); tách ảnh sản phẩm (0 harness marker, `harness-scan` + ca âm) vs `ktest-aarch64`; nối `vifast_sync::Mailbox` cho SGI/shootdown (bước 2 đóng Q1-A); `budget-aarch64.sh` + TAP cũ giữ nguyên; T2 PASS. |
| **KT-338b** | P1 | **`aarch64`: nối `vifast-core` & IPC EL0 (`svc #0`)** | `KT-338a, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` vào `aarch64` (`.data+.bss ≤ 256 KiB` đo bằng `budget-aarch64.sh`); nối `svc #0` theo `[calling.aarch64]` + `[debug_info]`/`[object_size]` (`KT-345a`) vào `core::syscall` (`DebugPutStr`/`DebugGetInfo`/`Exit`/`Yield`/`Send`/`Recv`/`Call`/`Reply`); TAP cũ + TAP `core` + `ipc` EL0 + 2 đột biến đỏ; T2 PASS. |
| **KT-339** | P1 | **Trừu tượng hoá `vifast-plat-user` đa arch + `aarch64` K3.5 (`procsrv` + `shell`)** | `KT-338b` | 3 h | `BLOCKED` | Tách stub syscall theo arch trong `vifast-plat-user` để cùng nguồn Rust `procsrv`/`shell` biên dịch được cho `riscv64` & `aarch64`; ảnh sản phẩm `aarch64 --features product-shell` nạp ELF bằng `vifast-elf`, chạy thật `help`/`echo`/`ps`/`exit` qua PL011 stdin; ca âm không `product-shell` báo `khong_co_shell`; T2 PASS. |
| **KT-340a** | P1 | **`x86_64`: `VSpace` động + kernel W^X + `budget-x86_64.sh` + tách sản phẩm vs `ktest-x86_64` (`D2-08`)** | `KT-334, KT-336a` | 3 h | `BLOCKED` | Dựng `VSpace` động (physmap + `vifast-pte::PageTable`), tách W^X toàn ảnh (`.text` RX / `.data` RW, đột biến ghi `.text` sinh `#PF`); thêm `v2/test/boot/budget-x86_64.sh` (`e_machine = 62`, trần 262.144 B, báo riêng `.smp_stacks`, nối `--selftest` vào T2); tách binary sản phẩm (0 harness marker, không kéo 64 KiB `TaskStack` test vào `.bss`) vs `ktest-x86_64`; TAP cũ + `-cpu max` xanh; T2 PASS. |
| **KT-340b** | P1 | **`x86_64`: TLB shootdown đa hart (`invlpg` + IPI AP→BSP qua `Mailbox`) & đóng lỗi v1 **N5** (`PCID`/`-cpu max`)** | `KT-340a` | 3 h | `BLOCKED` | Cài `invlpg` + IPI shootdown AP→BSP qua `vifast_sync::Mailbox` (hoàn tất 3/3 port đóng thật sự Audit Q1-A/`N7`); kiểm chứng `CR4.PCIDE`/TLB cũ trên `-cpu max` (flush theo PCID hoặc tắt PCID có kiểm chứng) kèm đột biến bỏ shootdown/bỏ ack ⇒ ĐỎ; đóng chính thức lỗi v1 **N5** (`KNOWN-ISSUES.md` #5); T2 PASS. |
| **KT-340c** | P1 | **`x86_64`: nối `vifast-core` & IPC ring 3 (thay scheduler riêng)** | `KT-340b, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` (`.data+.bss ≤ 256 KiB` đo bởi `budget-x86_64.sh`); nối `int 0x80` theo `[calling.x86_64]` v1.1 (`KT-345a`: tách `return_message_info`, hết xung đột `rdx`/`r12`) vào `core::syscall`; gỡ state machine scheduler trùng lặp ở `vifast-plat-x86_64/src/sched.rs` (giữ switch `iretq` 22-qword + TSS/IST1 + `McsLock` bọc IRQ); hart phụ `hlt` khi rảnh; TAP cũ + `core` + `ipc` ring 3 + `-cpu max` xanh; T2 PASS. |
| **KT-341** | P1 | **`x86_64` K3.5: `procsrv` + `shell` dùng chung `vifast-plat-user`** | `KT-339, KT-340c` | 3 h | `BLOCKED` | Ảnh sản phẩm `x86_64 --features product-shell` nạp `procsrv`/`shell` cùng nguồn Rust với `riscv64`/`aarch64`, chạy thật `help`/`echo`/`ps`/`exit` qua COM1 stdin; ca âm không `product-shell` ĐỎ; cả 3 arch dùng chung 1 codebase shell; T2 PASS. |
| **KT-342** | P1 | **Hợp nhất ảnh sản phẩm `riscv64`: gỡ `ktest` khỏi driver + chạy đồng thời FS, `netsrv`, `exec`** | `KT-335, KT-336a` | 3 h | `BLOCKED` | Tách nhãn feature driver (`net-drv`/`gpu-drv`/`input-drv`/`vcon-drv`) khỏi `ktest` trong `vifast-plat-riscv64`; tích hợp FS + `netsrv` U-mode + `exec` vào ảnh sản phẩm (bộ nhớ động Untyped/physmap, `.data+.bss ≤ 256 KiB`, 0 harness marker); 1 boot sản phẩm chạy cả `ls`/`cat`/`write` + `ping` qua `netsrv` + `run /hello.elf`; 3 đột biến đỏ; T2 PASS. |
| **KT-343** | P1 | **Đóng 11 lệnh `deferred-K7` không cần mở ABI (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`, `run`, `runfs`, `exec`)** | `KT-342` | 3 h | `BLOCKED` | Chuyển 11 lệnh trên từ `deferred-K7` → `active` trong `spec/v1-parity.toml` (**25/28 active**, không sửa `spec/abi.toml`); runner parity `--mode serial` xanh 25/25 trên ảnh sản phẩm ở cả `smp=1` và `smp=4`; ca âm từng nhóm lệnh mới; T2 PASS. |

### Đợt 3 — Hoàn tất tính năng ABI v1.1, tách build Userland & Nghiệm thu Chu kỳ 2

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-345b** | P1 | **Thi hành tính năng ABI v1.1: `free`/`df`/`loop` (28/28 parity) + `NetSvc` đa caller** | `KT-342, KT-343, KT-345a` | 3 h | `BLOCKED` | Nâng `NetSvc` phục vụ ≥ 2 caller đồng thời theo IDL đã duyệt; đóng 3 lệnh cuối `free`, `df`, `loop` trong `spec/v1-parity.toml` (**28/28 = 100 % active**); parity `--mode serial` xanh 28/28 ở `smp=1` và `smp=4`; đột biến đỏ + T2 PASS. |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184 (3 arch)** | `KT-341, KT-342` | 3 h | `BLOCKED` | Chuyển build ELF user ra cây `v2/user/` riêng kèm `--remap-path-prefix`, đóng gói ELF vào ảnh đĩa FS (`vifast-fsutil`) hoặc initrd chuẩn thay `include_bytes!` trong `build.rs`; thêm `v2/user` vào `VBREV_INPUT_ROOTS` (`build-rev.sh`) kèm **tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `build-rev.algo.pin`** và `wake_scope.py`; `repro-check.sh` chứng minh **cùng sha256 trên 2 thư mục build khác nhau cho ảnh sản phẩm canonical cả 3 arch** (Linux/macOS; Windows giữ nợ `v2.1` theo B2); shell + `exec` nạp ELF từ FS chạy thật; T2 PASS. |
| **KT-347** | P2 | **Thiết kế & PoC `U1a` (`no_std+alloc` `vifast-rt`) + hợp đồng `.vidl` `ViScript` + hướng `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `BLOCKED` | Chạy thật 1 ứng dụng user mẫu dùng `vifast-alloc` + lớp runtime tối thiểu (`U1a`, biên dịch bằng stable `1.100.0`) qua `exec` từ FS; lập `v2/docs/u1-uefi-readiness.md` chốt phân tầng `U1a`/`U1b`, hợp đồng `.vidl` liên repo `ViFastOs ↔ ViScript` (`VSH2`), và phương án `UEFI` + khám phá phần cứng `DTB`/`ACPI` cho Chu kỳ 3; T2 PASS. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | `KT-331, KT-332, KT-333a, KT-333b, KT-336b, KT-337, KT-341, KT-343, KT-345b, KT-346` | 3 h | `BLOCKED` | Chạy độc quyền trên ứng viên tích hợp cuối: T2 PASS; `v2 nightly --all` (L1 100k × smp 1/2/4/8 + Miri + sinh `STATUS.md`) PASS; soak **ảnh sản phẩm có shell** (`riscv64` `--features product-shell,product-fs` + net/exec 1.000 kèm đợt **soak đĩa thật `virtio-blk`**, `aarch64` product-shell 1.000, `x86_64` product-shell 600 `qemu64` + 600 `-cpu max`) đều 0 lỗi (`qemu_foreign=0`, không retry); đối chiếu đủ **Bảng Exit Criteria `v2.0` (§6.2)**; xuất `v2/agent/reports/R2-v2.{md,json}` qua `check_report.py --all` + `verify-numbers.py`; xác nhận clone/checkout sạch Windows/macOS/Linux; trình Agent 0 chốt Chu kỳ 2 & quyết định mở `v2.0`. |

---

## 4. Vùng xung đột & quy tắc chạy song song (bài học Chu kỳ 1 + rà soát D2-236..D2-239)

| Vùng tệp nhạy cảm | Rủi ro | Quy tắc bắt buộc |
|---|---|---|
| `v2/bin/v2` (ID bước `lane_step`) | Trùng ID làm `lane_done` bỏ qua bước im lặng | Chỉ **1 task mỗi vòng** thêm bước vào `v2/bin/v2`; trước commit chạy `grep -n "lane_step" v2/bin/v2` kiểm trùng ID trên cả `do_gate_fast` và `do_gate_dev`. |
| `v2/tools/check-wake-paths.py` (`SIM_KOBJ`) & `wake_scope.py` | Tách `kobj.rs` (`KT-336b`) hoặc thêm `v2/user/` (`KT-346`) làm gãy/lọt luật `wake` | Cập nhật đồng bộ 6 vị trí `SIM_KOBJ` + `check-wake-depinfo.py` + `CHECKER-WAKE-CONTRACT.md` trong cùng `KT-336b`; cập nhật `WORKSPACES`/`NON_CRATE_SOURCE_TREES` trong `KT-346`; không để thoái lui hợp đồng `D2-190`/`D2-205`. |
| `v2/test/boot/build-rev.sh` & `build-rev.algo.pin` | Thêm `v2/user` mà không tăng `VBREV_ALGO` bị `check-algo-pin.sh` từ chối | Chỉ **`KT-346`** được sửa `VBREV_INPUT_ROOTS` và bắt buộc tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + cập nhật `build-rev.algo.pin` trong **cùng commit** (D2-239). |
| `v2/lib/vifast-sync` (`McsLock`) | `McsLock::lock()` không tự tắt IRQ ⇒ self-deadlock nếu dùng trong cả IRQ & normal | Bắt buộc bọc guard lưu/tắt IRQ (`cli`/`sstatus.SIE`/`DAIF`) ở lớp `vifast-plat-*` cho mọi khoá đụng ngắt (`COM1`, `core_bridge`, scheduler — D2-238). |
| `v2/kernel/vifast-core/src/machine.rs` & pin `196.608` | Hai nhánh cùng sửa struct hoặc pin | Không để 2 worktree sửa `vifast-core` cùng lúc nếu đều đổi layout `Machine`/`TcbData`. |
| `v2/agent/QUEUE.md`, `v2/agent/STATE.md` | Xung đột merge, agent ghi vượt quyền | **Chỉ lead (Agent 1)** cập nhật `QUEUE.md` (lật `DONE`/`READY` đúng DAG D2-235, **trừ `KT-345a`**) và khối trạng thái đầu `STATE.md` khi merge về `main`. |
| `v2/test/boot/*` & chạy QEMU song song | `qemu_foreign > 0` làm soak `MEASUREMENT_INVALID` | **Soak (`KT-330`, `KT-348`) luôn chạy một mình trên máy đo**; các task dùng QEMU khác tối đa 3 QEMU tổng toàn máy, cấm `pkill -f qemu-system`. |
| `spec/abi.toml`, `spec/kernel.toml` | Vỡ đóng băng ABI | **Khoá cứng** ở mọi task trừ **`KT-345a`** (chỉ sau `APPROVED Agent 0`). Phát hiện thiếu ABI ⇒ `BLOCKED` và ghi vào `KT-344`, không tự vá lách. |

---

## 5. Hướng dẫn khởi động ngay cho các Agent sau khi `git pull`

```bash
git pull origin main
git status -s                                              # phải sạch trên cả Linux/macOS/Windows
cargo run -q --manifest-path v2/Cargo.toml -p vifast-agentctl -- report --project v2
bash v2/tools/status.sh
```

1. **Agent 1 (Lead) — chạy độc quyền `KT-330` trước**: vòng 25 (`KT-328`/`KT-329`) đã `DONE`, nên **`KT-330`** đang **`READY`**. Chạy độc quyền `KT-330` (soak `riscv64` 1.000 + `x86_64` 600 `qemu64` + 600 `-cpu max`, `qemu_foreign=0`), không mở QEMU song song.
2. **Đợt 0 song song (ngoài giờ chạy QEMU của `KT-330`, ngoài `v2/kernel`/`v2/lib`)**: giao tối đa 4 worktree cho **`KT-331`**, **`KT-332`**, **`KT-333a`**, **`KT-333b`**, và ưu tiên cao **`KT-344`** (RFC ABI v1.1 **9 nhóm**).
3. **Đẩy sớm `KT-344` → `REVIEW`**: ngay khi `KT-344` xong, báo **Agent 0** duyệt một lần để mở **`KT-345a`** song song với Đợt 1 — nhờ đó `KT-338b` và `KT-340c` ở Đợt 2 không bị chờ ABI.
4. **Đợt 1 ngay sau `KT-330`**: mở song song **`KT-334`** ∥ **`KT-336a`** ∥ **`KT-336b`** (tối đa 4 worktree tính cả Đợt 0 còn lại).
5. Mỗi task hoàn tất phải có: **test đỏ trước** + **đột biến bắt buộc** + **ca âm** + **T1/T2 theo `AGENTS.md` §6.1** + **commit có nhãn `[KT-###]`** (không retry, không nới ngưỡng, không commit raw `boot-*.log` của soak).

---

## 6. Khung lộ trình dài hạn (Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`) & Tiêu chí phát hành `v2.0` (**D2-240**)

### 6.1 Bảy trụ cột chiến lược sau Chu kỳ 2 (Chu kỳ 3 trở đi)

| # | Trụ cột | Mốc đề xuất | Thực trạng hiện tại & việc bắt buộc phải làm |
|---|---|---|---|
| 1 | **SMP Preemptive thật (`S1 → S2 → S3`)** | `SMP-1`, `SMP-2` | Ảnh sản phẩm `riscv64` hiện vẫn chạy vòng hợp tác `proc_rt` trên 1 hart (`rust_secondary_main` park vô hạn) với 1 BKL. Cần đưa `vifast-core::Machine::dispatch` + timer preempt + `vifast-sync::Mailbox` + `McsLock` bọc IRQ chạy U-mode thật trên 4 hart ở cả 3 arch (`SMP-1`), rồi tách trạng thái nóng sang `PerHart` (`SMP-2`); chỉ mở `S3` khi `vifast-bench` đo chờ khoá > 10 % ở `smp=4`. |
| 2 | **Hợp nhất `Machine` (đóng `D2-83`) & `vifast-objmem` (D2-14)** | `MEM-1`, `MEM-2` | Kernel còn 2 `Machine` song song (`vifast-core::Machine` 184,8 KiB + `vifast-plat-common::obj::Machine` 64 mappings) và trần tĩnh `MAX_OBJECTS=256`, `MAX_TCBS=64` sẽ chặn `U1`/`VSH2`/`G2`. Cần rút Sv39/PLIC khỏi `vifast-plat-common`, đưa `TableMemory` lên `vifast-pte` (`MEM-1`), và chuyển cấp phát đối tượng vào `Untyped` thật qua `vifast-objmem` để `.data+.bss` < 64 KiB và quy mô theo RAM (`MEM-2`). |
| 3 | **4 nền tảng kiểm chứng/quan sát trong thiết kế v2** | `OBS-1`, `VER-1` | Hoàn thiện những mảnh đã cam kết nhưng còn trống: (i) **trace nhị phân 32 B luôn bật** + dump `pmemsave` + `vifast-trace` (**D2-09**, §9.11); (ii) **mutation testing tự động** `v2/test/mutants.toml` + `v2 mutants` trong `nightly` (§9.9); (iii) **`vifast-bench` + `test/perf/history.csv`** dưới `-icount` làm cổng dữ liệu cho `syscall`/`sysret`, `S2/S3`, `F1` (§9.10); (iv) **conformance `.scn` đủ 3 arch** (D2-13) + **MCS refill đa đoạn/`SetTimeoutEp`** (D2-16). |
| 4 | **Driver từ Polling → IRQ `Notification` → Userland + IOMMU** | `DRV-1`, `D2`, `D3` | Chuyển driver VirtIO từ busy-poll sang ngắt thật qua **`IrqControl`/`IrqHandler` + `Notification`** (mở ở ABI v1.2); làm nền cho driver userland (`D2`: PCIe MSI-X, NVMe, AHCI) kèm `Frame.MapIo` + **`IOSpace` (VT-d / SMMUv3 / virtio-iommu, `D3`)** để cô lập DMA. |
| 5 | **An toàn, FPU/SIMD & chuẩn hoá `.vidl`** | `SEC-1`, `IDL-1` | Lưu/khôi phục **ngữ cảnh FPU/SIMD theo TCB** (`fs0..fs31`, NEON `v0..v31`, `fxsave`/`xsave` — bắt buộc trước `U1` để tránh hỏng thanh ghi giữa 2 tiến trình Rust/ViScript); bộ test đối kháng `copy_from_user`/`copy_to_user` + bật `SMEP`/`SMAP`/`PAN` + chính sách KPTI tuỳ chọn; fuzz `vifast-net` + nguồn entropy `virtio-rng`; chuẩn hoá `proc.vidl`, `fs.vidl`, `net.vidl`, `term.vidl` trong `v2/lib/vifast-idl`. |
| 6 | **`U1` hai tầng & Hợp đồng liên repo `ViFastOs ↔ ViScript` (`VSH2`)** | `U1a`, `U1b`, `VSH2` | Chia `U1` thành **`U1a`** (`no_std + alloc` + `vifast-rt` qua `.vidl`, chạy ngay trên **stable `1.100.0`** để kéo `vis-host-vifast`/ViShell lên sớm) và **`U1b`** (Rust `std` PAL target `*-unknown-vifastos`); chốt hợp đồng phiên bản `abi_version`/`.vidl` và CI tích hợp định kỳ giữa repo `ViFastOs` và repo [`ViScript`](VISCRIPT.md). |
| 7 | **Khám phá phần cứng thật (`HW1`) & Bền vững công cụ** | `HW-0`, `UEFI`, `HW1` | Trước `UEFI`/`D2`/`HW1`, bổ sung mốc **`HW-0`**: phân tích **DTB thật** (`riscv64`/`aarch64`) và **ACPI `MADT`/`FADT` + bản đồ bộ nhớ E820/UEFI** (`x86_64`) thay hằng số QEMU `virt` gán cứng. Đồng thời **đóng băng bề mặt parser Python** (`check-wake-paths.py` đã ~498 KiB — ưu tiên ép bất biến bằng hệ kiểu Rust như `NonReadyTarget`) và quyết định chính sách lưu trữ/nén 8.985 raw log Chu kỳ 1 trong `JOURNAL/` trước khi công khai `v2.0`. |

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 shell** (`riscv64`/`aarch64`/`x86_64`, tách sản phẩm/harness D2-08) | **Có** (`KT-338a..KT-342`) | `SMP-1/2` preemptive 4 hart đầy đủ trên ảnh sản phẩm |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active`** (`KT-343` + `KT-345b`, `smp=1` & `smp=4`) | Mở rộng lệnh `ViShell` (`VSH2`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có** (đóng nốt **N5** ở `KT-340b` và **Audit Q1-A/N7** đủ 3 port qua `KT-336a`/`338a`/`340b`) | — |
| **Ngân sách & Cổng** (`.data+.bss ≤ 256 KiB` cả 3 arch, `Machine ≤ 196.608 B`, T2 PASS, `v2 nightly --all` L1 400k + Miri PASS) | **Có** (`budget.sh`, `budget-aarch64.sh`, `budget-x86_64.sh`, `KT-333b`, `KT-348`) | `v2 mutants` + `vifast-bench` tự động trong nightly |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`) | **Ảnh sản phẩm có shell** 3 arch (`rv 1000`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) + **soak đĩa thật `virtio-blk`** (`KT-348`) | Soak đa tiến trình `U1`/mạng tải nặng |
| **Tái lập byte (P20)** | **Đạt cho ảnh sản phẩm canonical cả 3 arch trên Linux + macOS** (`KT-346`) | Hội tụ trên host **Windows** (B2 / D2-122.4) |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md` tự sinh** (`KT-333b`), clone/checkout sạch Win/macOS/Linux (`KT-331`) | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **Động hóa bộ nhớ, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | Có tài liệu định hướng & PoC `U1a` (`KT-347`) | Thi công ở **Chu kỳ 3 (`v2.1+`)** theo §6.1 |

