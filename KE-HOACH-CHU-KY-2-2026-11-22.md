# KẾ HOẠCH THI CÔNG CHU KỲ 2 — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (tại `HEAD` `1559339e`, ngay khi vòng 25 `KT-328`/`KT-329` đang chạy).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ sau khi khép vòng 25 (`KT-328`/`KT-329`).
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-235**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md).

> **Mục tiêu một câu**: Hoàn tất nốt vòng 25 (`KT-328`/`KT-329`) rồi chấm dứt trạng thái "mỗi kiến trúc một nhánh rời / mỗi tính năng một `ktest-*` riêng" — **khử nhân bản driver trên 3 kiến trúc**, **nối `vifast-core` + `shell` dùng chung lên `aarch64` và `x86_64`**, **hợp nhất ảnh sản phẩm `riscv64`** (FS + Net + Exec chạy đồng thời), **hạ thời gian cổng & dựng `v2 nightly` thật**, và **mở một cửa sổ duy nhất cho ABI v1.1** để tiến sang `U1` (Rust `std`), `VSH2` (ViShell) và `v2.0`.

---

## 0. Chẩn đoán đầu vào (căn cứ từ mã nguồn `HEAD` `1559339e`)

Chu kỳ 1 (`KT-01..KT-327`, đang chạy nốt vòng 25 `KT-328`/`KT-329`) đạt kỷ luật kiểm chứng rất cao (T2 `gate --fast` PASS 68/68, soak 3 arch `9e18880d` 0 lỗi, K6 đóng, luật `wake` đóng, 3 arch đều boot thật trên QEMU). Tuy nhiên đối chiếu trực tiếp `Cargo.toml`, cây nguồn và cổng đo lộ ra **7 điểm nghẽn cấu trúc** buộc phải xử lý ngay sau vòng 25:

1. **`aarch64` (K4') và `x86_64` (K5') chưa nối `vifast-core`**: `vifast-kbin-aarch64` và `vifast-kbin-x86_64` đều không phụ thuộc `vifast-core`, `vifast-elf`, `vifast-vfs`, `vifast-fs`; cả hai vẫn là 1 binary selftest duy nhất (chưa tách ảnh sản phẩm vs harness theo D2-08). Đặc biệt `x86_64` đang phát triển một bộ lập lịch độc lập (`vifast-plat-x86_64/src/sched.rs` + `vifast-kbin-x86_64/src/sched.rs`) tách rời `vifast-core::Machine`.
2. **Phân mảnh 14 binary trên `riscv64`**: `vifast-kbin-riscv64` có 13 binary `ktest-*`; các feature `net`/`gpu`/`input`/`vcon` của `vifast-plat-riscv64` vẫn kéo theo `ktest`; ảnh sản phẩm `product-fs` chưa có `exec`/`netsrv`/mạng và còn 14/28 lệnh `deferred-K7` trong `spec/v1-parity.toml`.
3. **Nhân bản driver lớn & crate mồ côi**: `virtio_net.rs`, `virtio_blk.rs`, `pci.rs`, `virtio_gpu.rs`, `virtio_input.rs`, `tcp.rs` bị chép lặp giữa 3 `vifast-plat-*` (~11.000+ dòng). Trong khi đó `vifast-sync` có **0 consumer** trong `v2/kernel` và `v2/sim`; `vifast-objmem` chỉ dùng ở `vifast-sim`; `v2/sim/vifast-sim/src/kobj.rs` phình lên 11.443 dòng (ngoài phạm vi R1).
4. **Cần khép soak sau vòng 25 + `-cpu max`**: `KT-328` (`x86_64`) và `KT-329` (`riscv64`) đang đổi ảnh nên cần 1 đợt soak khép cuối vòng 25 kèm soak 600 boot `-cpu max` cho `x86_64` (`KT-330`) trước khi vào Đợt 1 tái cấu trúc.
5. **Cổng T2 phình thời gian & thiếu `nightly`/CI**: `gate --fast` đã lên ~13,8 phút; `v2 nightly` đầy đủ vẫn trả `exit 2`; chưa có CI host và chưa có `deny.toml` (D2-21).
6. **Build userland qua `build.rs` + `include_bytes!` không mở rộng được sang `U1`/`VSH2`** và là gốc nợ P20/A1-184 (đường dẫn host lọt vào ELF user).
7. **Vệ sinh repo, sổ sách & tương thích đa nền tảng**:
   - 4 tệp trong `v2/agent/JOURNAL/kt219-logs/cases/` chứa `:`/`>` làm gãy `git clone` trên Windows (Agent 0 đã đổi tên trực tiếp trong commit mở Chu kỳ 2 để gỡ tắc ngay; `KT-331` dựng máy kiểm chống tái diễn).
   - 7 task vòng 17/21 (`KT-300..KT-302`, `KT-313..KT-316`) bị sót lật `DONE` ở `QUEUE.md` và `KT-303` có dấu `|` chưa thoát khiến `vifast-agentctl next` bị lệch (Agent 0 đã đối soát sửa trực tiếp).
   - `v2/agent/JOURNAL/` tích tụ gần 9.000 tệp raw log; `README.md`, `v2/README.md`, `STATE.md`, `BLOCKED.md`, `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md` §5 lệch hiện trạng; `v2/kernel/Cargo.toml` ghi `rust-version = "1.99.1"` lệch `1.100.0`.

---

## 1. Quyết định kiến trúc của Agent 0 (ghi vào `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229** | **Mở Chu kỳ 2 (`KT-330..KT-348`), giữ vòng 25 (`KT-328`/`KT-329`) & sửa 4 tệp lỗi Windows** | Không gián đoạn `KT-328`/`KT-329` đang chạy (D2-227/D2-228); Agent 0 đổi tên 4 tệp `kt219-logs/cases/` chứa `:`/`>` (giữ nguyên 100% blob) và đối soát lật `DONE` cho 7 dòng sót ở `QUEUE.md`; **giữ nguyên mọi ngưỡng cứng** (`.data+.bss ≤ 256 KiB`, `Machine ≤ 196.608 B`, `R1 ≤ 1.500`, `D2-07`, `D2-08`, `D2-17`); **chưa mở `v2.0`**. |
| **D2-230** | **Chính sách nhật ký `JOURNAL/` & phòng chống tên tệp lỗi NTFS** | Cấm commit raw `run-soak-*/boot-*.log` hàng trăm/nghìn tệp vào Git kể từ Chu kỳ 2; chỉ commit tóm tắt (`summary.txt`, bảng chứng cứ, sha256, `build-inputs`, log mẫu lỗi nếu đỏ). Bắt buộc kiểm tên tệp tương thích NTFS ở `git-guard` và `v2 check` (`KT-331`). |
| **D2-231** | **Hợp nhất tầng driver VirtIO-PCI & mạng dùng chung 3 arch** | Rút logic lặp lại của `pci`, `virtio_blk`, `virtio_net`, `virtio_gpu`, `virtio_input` về lớp transport thuần qua trait `MmioBus` + `DmaPool` (`forbid(unsafe_code)`); chỉ giữ adapter MMIO/DMA `unsafe` tối giản trong `vifast-plat-*` (D2-17). Đưa toàn bộ state machine TCP/client mạng về `v2/lib/vifast-net`; xoá hai bản `tcp.rs` nhân bản trong `vifast-plat-{riscv64,aarch64}`. |
| **D2-232** | **Đóng trạng thái "crate mồ côi" (`vifast-sync`, `vifast-objmem`) & áp R1 cho `sim/src/`** | Nối `vifast-sync` thật vào `v2/kernel` (thay spinlock tự chế trên `x86_64`, đóng Audit Q1-A). Phân định rõ `slotmem` (tĩnh K3–K6) vs `vifast-objmem` (lộ trình động khi chạm headroom) qua trait `ObjStore` chung. Tách `v2/sim/vifast-sim/src/kobj.rs` về ≤ 1.500 dòng/tệp và áp R1 cho `v2/sim/vifast-sim/src/`. |
| **D2-233** | **Một lõi `vifast-core` & một userland `vifast-plat-user` cho cả 3 kiến trúc** | **Cấm** phát triển thêm scheduler riêng bên ngoài `vifast-core`. `aarch64` và `x86_64` phải tách ảnh sản phẩm vs harness (D2-08), nối `vifast-core`, và chạy chung 1 mã nguồn `procsrv`/`shell` qua `vifast-plat-user` (tách tầng asm `ecall`/`svc`/`int 0x80`). |
| **D2-234** | **Một cửa sổ duy nhất cho ABI v1.1 (`KT-344` → `APPROVED` → `KT-345`)** | Mọi task Chu kỳ 2 trừ `KT-345` **cấm sửa `spec/abi.toml`** (riêng `spec/v1-parity.toml` được phép cập nhật trạng thái lệnh không đổi ABI ở `KT-342`/`KT-343`). Toàn bộ 6 nhóm nợ chạm `spec/` (`spec/kernel.toml`, `extra_caps_unwrapped`, kênh lỗi VSpace/ô 0 CSpace, `MemInfo`/`STATFS`, GDT `syscall`/`sysret` x86_64, IDL `NetSvc` đa caller — `Suspend(Defer)` đã giải xong không cần sửa `spec/` theo D2-228) gom vào **một RFC duy nhất (`KT-344`)**; chỉ mở `KT-345` sau dòng `APPROVED <ngày> Agent 0` trong `QUEUE.md`. |
| **D2-235** | **Quy chế điều phối song song & lật sổ Chu kỳ 2** | Tối đa **4 worktree song song** do Agent 1 (lead) điều phối như D2-198; sub-agent commit trên nhánh `task/kt<n>-<tên>`, chỉ lead merge/push `main`. Khi phụ thuộc trong DAG đã `DONE`, lead được lật trạng thái `BLOCKED` → `READY` cho các task Chu kỳ 2 **trừ `KT-345`** (bắt buộc chờ `APPROVED Agent 0`). `KT-330` và `KT-348` (soak) chạy **độc quyền trên máy đo** (`qemu_foreign=0`). |

---

## 2. Lộ trình 4 đợt & đồ thị phụ thuộc (`KT-328..KT-348`)

```text
[VÒNG 25 ĐANG CHẠY — giữ nguyên, hoàn tất trước khi vào Đợt 1]
  KT-328 (x86_64 ICMP + DNS)  ∥  KT-329 (Tcb.Suspend Defer + Resume, không sửa spec/)
        │
        ▼
[ĐỢT 0 — Khép vòng 25, vệ sinh & hạ tầng cổng]
  (KT-328, KT-329) ──► KT-330 (soak cuối vòng 25: rv 1000 + x64 600 + x64 -cpu max 600)
  READY ngay (ngoài v2/kernel & v2/lib, chạy song song được):
    KT-331 (guard NTFS + chính sách JOURNAL)
    KT-332 (đồng bộ README/STATE/BLOCKED/báo cáo)
    KT-333 (tối ưu T2 + v2 nightly + deny.toml + CI host)
    KT-344 (RFC Gói ABI v1.1 — thuần tài liệu, đầu ra REVIEW chờ Agent 0)
        │
        ▼  (sau khi KT-330 khóa bằng chứng soak vòng 25)
[ĐỢT 1 — Khử nhân bản & hợp nhất tầng dùng chung]
  ├── KT-334 (MmioBus/DmaPool + hợp nhất pci & virtio_blk 3 arch + MSRV 1.100.0)
  │     ├──► KT-335 (hợp nhất virtio_net 3 arch + TCP/client state machine vào vifast-net)
  │     └──► KT-337 (hợp nhất virtio_gpu/input + mở virtio-gpu x86_64 PPM)
  └── KT-336 (nối vifast-sync vào kernel + hướng objmem + tách sim/src/kobj.rs)
        │
        ▼
[ĐỢT 2 — Một lõi vifast-core cho 3 arch & Hợp nhất ảnh sản phẩm riscv64]
  ├── (KT-334, KT-336) ──► KT-338 (aarch64: tách product/ktest + nối vifast-core + IPC EL0)
  │                          └──► KT-339 (vifast-plat-user đa arch + aarch64 procsrv/shell)
  ├── (KT-334, KT-336) ──► KT-340 (x86_64: tách product/ktest + nối vifast-core thay sched riêng)
  │                          └──► (KT-339, KT-340) ──► KT-341 (x86_64 procsrv/shell dùng chung)
  └── (KT-335)         ──► KT-342 (riscv64: gỡ ktest khỏi driver + ảnh sản phẩm FS+Net+Exec)
                             └──► KT-343 (đóng 8 lệnh deferred-K7 không cần ABI → 22/28 active)
        │
        ▼
[ĐỢT 3 — Cửa sổ ABI v1.1, tách Userland & Nghiệm thu Chu kỳ 2]
  ├── (KT-344 + APPROVED Agent 0) ──► KT-345 (áp ABI v1.1 + vifast-gen + free/df + NetSvc đa caller)
  ├── (KT-341, KT-342)            ──► KT-346 (tách build userland v2/user/ khỏi build.rs + P20/A1-184)
  │                                     └──► KT-347 (PoC U1 runtime + chốt hướng UEFI)
  └── (KT-341, KT-342, KT-343, KT-346) ──► KT-348 (Cổng nghiệm thu Chu kỳ 2: soak 3 arch + T2 + nightly + R2-v2)
```

---

## 3. Chi tiết từng đợt & bảng task

### Đợt 0 — Khép vòng 25, vệ sinh & hạ tầng cổng

> **Luật thực thi Đợt 0**: `KT-331`, `KT-332`, `KT-333`, `KT-344` **không** sửa `v2/kernel/**` hay `v2/lib/**` (không xung đột với `KT-328`/`KT-329`, không làm bẩn `build-inputs`). Khi chạy **`KT-330`** (soak), máy đo phải chạy **độc quyền** (`qemu_foreign=0`, D2-116).

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-330** | P0 | **Soak ứng viên cuối vòng 25**: `riscv64` 1.000 + `x86_64` 600 (`qemu64`) + `x86_64` 600 (`-cpu max`) | `KT-328, KT-329` | 3 h | `BLOCKED` | 0 lỗi cả 3 đợt soak (`rc_124=0`, `qemu_foreign=0` hai mẫu, `invalid=0`, không retry D2-07); chỉ commit `summary.txt` + `build-inputs`/`BUILD_ID` (D2-230); cập nhật `STATE.md`. |
| **KT-331** | P0 | **Guard tên tệp Windows/NTFS + chính sách `JOURNAL/`** | — | 2 h | `READY` | Thêm kiểm tên tệp tương thích NTFS (cấm `[<>:"\|?*]`, tên thiết bị Windows, dấu chấm/trắng cuối tên) vào `v2/tools/git-guard/pre-commit` và `v2/bin/v2` `rules`; có `--selftest` ca âm ĐỎ đúng mã + ca dương XANH; thêm `.gitignore` chặn raw `run-soak-*/boot-*.log`; **không** sửa `v2/kernel`/`v2/lib`. |
| **KT-332** | P0 | **Đồng bộ tài liệu công khai, cắt gọn `STATE.md` & dọn `BLOCKED.md`** | — | 2 h | `READY` | Cập nhật `README.md`, `v2/README.md` khớp thực tế sau vòng 25 (không số đo viết tay); đính chính đầu trang + §5 của `BAO-CAO-CUOI-TUAN-KT243-2026-11-22.md`; tách lịch sử KT-01..KT-329 của `STATE.md` sang `v2/agent/archive/STATE-chu-ky-1.md` (`STATE.md` < 30 KiB); dọn mục cũ đã đóng ở `BLOCKED.md`; `v2 check` + `v2 check-rules` PASS (không chạm tệp legacy bị ghim sha hay `docs/v1/`). |
| **KT-333** | P1 | **Hạ wall-time T2 (`gate --fast`), dựng `v2 nightly` thật, `deny.toml` & CI host** | — | 3 h | `READY` | Profile 68 bước T2 và tối ưu làn chạy song song (giảm wall-time, **0 bước bị bỏ**); cài `v2 nightly` (`--all`/`--l1`/`--only-miri`/`--soak`/`--status`/`--dry-run`/`--selftest`, sinh `STATUS.md` máy-đo-được); thêm `deny.toml` allowlist MIT/Apache-2.0 + bước kiểm không registry dep ngoài allowlist + ca âm (D2-21); thêm `.github/workflows/v2-host-ci.yml`; **không** sửa `v2/kernel`/`v2/lib`; T2 PASS. |

### Đợt 1 — Khử nhân bản driver & hợp nhất tầng dùng chung

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-334** | P1 | **Lớp transport `virtio-pci` dùng chung (`MmioBus`/`DmaPool`) cho `pci` + `virtio_blk` 3 arch + MSRV** | `KT-330` | 3 h | `BLOCKED` | Trích logic lặp của `pci.rs` và `virtio_blk.rs` về lớp dùng chung (`forbid(unsafe_code)`); 3 crate `vifast-plat-*` chỉ còn adapter MMIO/DMA mỏng (D2-17); đồng bộ `v2/kernel/Cargo.toml` `rust-version = "1.100.0"`; tổng dòng `pci.rs`+`virtio_blk.rs` giảm ≥ 45%; host test + đột biến đỏ; TAP 3 arch + T2 PASS. |
| **KT-335** | P1 | **Hợp nhất `virtio_net` 3 arch + chuyển TCP/client state machine về `v2/lib/vifast-net`** | `KT-334` | 3 h | `BLOCKED` | Xoá 2 bản `tcp.rs` nhân bản trong `vifast-plat-{riscv64,aarch64}`; chuyển toàn bộ state machine TCP/DHCP/ARP/ICMP/DNS dùng trait `Clock` vào `vifast-net` (≥ 25 host test + ca âm timeout/reset/checksum); 3 arch dùng chung transport `KT-334` (giữ đủ TAP ICMP+DNS của `KT-328`); `net.sh` + aarch64 TAP + x86_64 TAP + T2 PASS. |
| **KT-336** | P1 | **Nối `vifast-sync` vào kernel + chốt `vifast-objmem`/`slotmem` + tách `sim/src/kobj.rs`** | `KT-330` | 3 h | `BLOCKED` | `vifast-sync` được dùng thật trong `v2/kernel` (thay spinlock tự chế trên `x86_64`, đóng Audit Q1-A) kèm đột biến đỏ; tài liệu hoá trait `ObjStore` & đường chuyển `slotmem`→`objmem` trong `v2/docs/k3b-core.md`; tách `v2/sim/vifast-sim/src/kobj.rs` (11.443 dòng) về các module ≤ 1.500 dòng và mở rộng R1 phủ `v2/sim/vifast-sim/src/`; Miri + L0 + L1 + parity + T2 PASS. |
| **KT-337** | P1 | **Hợp nhất `virtio_gpu` + `virtio_input` qua `MmioBus`/`DmaPool` & mở `virtio-gpu` x86_64** | `KT-334` | 3 h | `BLOCKED` | Rút `virtio_gpu` và `virtio_input` về transport chung; giữ xanh `gpu.sh`/`input.sh` (`riscv64`) và `aarch64` (PPM 23/23); bật `virtio-gpu` 2D trên `x86_64` có bằng chứng `screendump` PPM + đột biến `offset=0` đỏ ở PPM; T2 PASS. |

### Đợt 2 — Một lõi `vifast-core` cho 3 arch & Hợp nhất ảnh sản phẩm `riscv64`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-338** | P1 | **`aarch64`: tách ảnh sản phẩm vs harness (`D2-08`) + nối `vifast-core` & IPC EL0** | `KT-334, KT-336` | 3 h | `BLOCKED` | Tách binary sản phẩm (0 harness marker, kiểm bằng `harness-scan` + ca âm) vs `ktest-aarch64`; link `vifast-core` và nối `svc #0` (`[calling.aarch64]`) vào `core::syscall` (`DebugPutStr`/`DebugGetInfo`/`Exit`/`Yield`/`Send`/`Recv`/`Call`/`Reply`); TAP cũ giữ nguyên + TAP `core` + `ipc` EL0 + 2 đột biến đỏ; `.data+.bss ≤ 256 KiB`; T2 PASS. |
| **KT-339** | P1 | **Trừu tượng hoá `vifast-plat-user` đa arch + `aarch64` K3.5 (`procsrv` + `shell`)** | `KT-338` | 3 h | `BLOCKED` | Tách stub syscall theo arch trong `vifast-plat-user` để cùng nguồn Rust `procsrv`/`shell` biên dịch được cho `riscv64` & `aarch64`; ảnh sản phẩm `aarch64 --features product-shell` nạp ELF bằng `vifast-elf`, chạy thật `help`/`echo`/`ps`/`exit` qua PL011 stdin; ca âm không `product-shell` báo `khong_co_shell`; T2 PASS. |
| **KT-340** | P1 | **`x86_64`: tách ảnh sản phẩm vs harness (`D2-08`) + nối `vifast-core` & IPC ring 3 (thay scheduler riêng)** | `KT-334, KT-336` | 3 h | `BLOCKED` | Tách binary sản phẩm (0 harness marker) vs `ktest-x86_64`; chuyển quản lý TCB/priority/sleep/IPC ring 3 (`int 0x80`, `[calling.x86_64]`) sang `vifast-core` + `vifast-sync`, gỡ state machine scheduler trùng lặp trong `vifast-plat-x86_64/src/sched.rs` (giữ switch `iretq` 22-qword + TSS/IST1); hart phụ quay về `hlt` khi rảnh; TAP cũ + `core` + `ipc` ring 3 + `-cpu max` xanh; `.data+.bss ≤ 256 KiB`; T2 PASS. |
| **KT-341** | P1 | **`x86_64` K3.5: `procsrv` + `shell` dùng chung `vifast-plat-user`** | `KT-339, KT-340` | 3 h | `BLOCKED` | Ảnh sản phẩm `x86_64 --features product-shell` nạp `procsrv`/`shell` cùng nguồn Rust với `riscv64`/`aarch64`, chạy thật `help`/`echo`/`ps`/`exit` qua COM1 stdin; ca âm không `product-shell` ĐỎ; cả 3 arch dùng chung 1 codebase shell; T2 PASS. |
| **KT-342** | P1 | **Hợp nhất ảnh sản phẩm `riscv64`: gỡ `ktest` khỏi driver + chạy đồng thời FS, `netsrv`, `exec`** | `KT-335` | 3 h | `BLOCKED` | Tách nhãn feature driver (`net-drv`/`gpu-drv`/`input-drv`/`vcon-drv`) khỏi `ktest` trong `vifast-plat-riscv64`; tích hợp FS + `netsrv` U-mode + `exec` vào ảnh sản phẩm (bộ nhớ động Untyped/physmap, `.data+.bss ≤ 256 KiB`, 0 harness marker); 1 boot sản phẩm chạy cả `ls`/`cat`/`write` + `ping` qua `netsrv` + `run /hello.elf`; 3 đột biến đỏ; T2 PASS. |
| **KT-343** | P1 | **Đóng 8 lệnh `deferred-K7` không cần mở ABI (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`)** | `KT-342` | 3 h | `BLOCKED` | Chuyển 8 lệnh trên từ `deferred-K7` → `active` trong `spec/v1-parity.toml` (**22/28 active**, không sửa `spec/abi.toml`); runner parity `--mode serial` xanh 22/22 trên ảnh sản phẩm ở cả `smp=1` và `smp=4`; ca âm cho mỗi nhóm lệnh mới; T2 PASS. |

### Đợt 3 — Cửa sổ ABI v1.1 có kiểm soát, tách build Userland & Nghiệm thu Chu kỳ 2

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-344** | P1 | **RFC Gói duyệt ABI v1.1 (`review/agent0/GOI-ABI-V1_1.md`) — KHÔNG sửa `spec/`** | — | 3 h | `READY` | Trình đủ 6 nhóm chạm `spec/` (`spec/kernel.toml`, `extra_caps_unwrapped`, kênh lỗi VSpace/ô 0 CSpace, `MemInfo`/`STATFS`, GDT `syscall`/`sysret` x86_64, socket IDL `NetSvc` đa caller — `Suspend(Defer)` đã chốt không cần mở `spec/` ở D2-228), mỗi mục ≥ 2 phương án + đánh giá `min_compat = 1` + `.scn` nháp. Hoàn tất chuyển **`REVIEW`** chờ Agent 0 ghi `APPROVED`. |
| **KT-345** | P1 | **Thi hành Gói ABI v1.1 sau `APPROVED Agent 0` (`spec/`, `vifast-gen`, `free`/`df`, `NetSvc` đa caller)** | `KT-344` | 3 h | `BLOCKED` | **Chỉ mở sau dòng `APPROVED <ngày> Agent 0` cho `KT-344` trong `QUEUE.md`**. Áp đúng mục duyệt vào `spec/`, chạy `vifast-gen`, nâng `NetSvc` phục vụ ≥ 2 caller đồng thời, đóng `free`/`df` (và `loop` nếu đủ điều kiện); `check_abi`/`check_reserved`/`vifast-gen --check`/`check_scn` + đột biến đỏ + T2 PASS. |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184** | `KT-341, KT-342` | 3 h | `BLOCKED` | Chuyển build ELF user ra cây/workspace `v2/user/` riêng kèm `--remap-path-prefix`, đóng gói ELF vào ảnh đĩa FS (`vifast-fsutil`) hoặc initrd chuẩn thay `include_bytes!` thủ công trong `build.rs`; `repro-check.sh` chứng minh **cùng sha256 trên 2 thư mục build khác nhau** cho ảnh sản phẩm canonical (Linux/macOS); shell + `exec` nạp ELF từ FS chạy thật; T2 PASS. |
| **KT-347** | P2 | **Thiết kế & PoC bước 1 `U1` (runtime `alloc`/thread/fs/process) + chốt hướng `UEFI` (`vifast-boot`)** | `KT-341, KT-346` | 3 h | `BLOCKED` | Biên dịch và chạy thật 1 ứng dụng user mẫu dùng `vifast-alloc` + lớp runtime tối thiểu qua `exec` từ FS (đặt nền `U1` & `VSH2` ViShell); lập `v2/docs/u1-uefi-readiness.md` trình phương án `UEFI` (3 arch vs giữ OpenSBI cho `riscv64`) để Agent 0 chốt ở Chu kỳ 3; T2 PASS. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak 3 arch + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | `KT-341, KT-342, KT-343, KT-346` | 3 h | `BLOCKED` | Chạy tuần tự độc quyền trên ứng viên tích hợp cuối: T2 PASS; `v2 nightly --all` (L1 100k × smp 1/2/4/8 + Miri + sinh `STATUS.md`) PASS; soak `riscv64` 1.000 + `aarch64` 1.000 + `x86_64` 600 (`qemu64`) + `x86_64` 600 (`-cpu max`) đều 0 lỗi (`qemu_foreign=0`, không retry); xuất `v2/agent/reports/R2-v2.{md,json}` qua `check_report.py --all` + `verify-numbers.py`; xác nhận clone/checkout sạch Windows/macOS/Linux; trình Agent 0 chốt Chu kỳ 2 & quyết định mở `v2.0`. |

---

## 4. Vùng xung đột & quy tắc chạy song song (bài học Chu kỳ 1)

| Vùng tệp nhạy cảm | Rủi ro | Quy tắc bắt buộc |
|---|---|---|
| `v2/bin/v2` (ID bước `lane_step`) | Trùng ID làm `lane_done` bỏ qua bước im lặng | Chỉ **1 task mỗi vòng** thêm bước vào `v2/bin/v2`; trước commit chạy `grep -n "lane_step" v2/bin/v2` kiểm trùng ID trên cả `do_gate_fast` và `do_gate_dev`. |
| `v2/kernel/vifast-core/src/machine.rs` & pin `196.608` | Hai nhánh cùng sửa struct hoặc pin | Không để 2 worktree sửa `vifast-core` cùng lúc nếu đều đổi layout `Machine`/`TcbData`. |
| `v2/agent/QUEUE.md`, `v2/agent/STATE.md` | Xung đột merge, agent ghi vượt quyền | **Chỉ lead (Agent 1)** cập nhật `QUEUE.md` (lật `DONE`/`READY` đúng DAG D2-235) và khối trạng thái đầu `STATE.md` khi merge về `main`. |
| `v2/test/boot/*` & chạy QEMU song song | `qemu_foreign > 0` làm soak `MEASUREMENT_INVALID` | **Soak (`KT-330`, `KT-348`) luôn chạy một mình trên máy đo**; các task dùng QEMU khác tối đa 3 QEMU tổng toàn máy, cấm `pkill -f qemu-system`. |
| `spec/abi.toml`, `spec/kernel.toml` | Vỡ đóng băng ABI | **Khoá cứng** ở mọi task trừ `KT-345` (chỉ sau `APPROVED Agent 0`). Phát hiện thiếu ABI ⇒ `BLOCKED` và ghi vào `KT-344`, không tự vá lách. |

---

## 5. Hướng dẫn khởi động ngay cho các Agent sau khi `git pull`

```bash
git pull origin main
git status -s                                              # phải sạch trên cả Linux/macOS/Windows
cargo run -q --manifest-path v2/Cargo.toml -p vifast-agentctl -- report --project v2
bash v2/tools/status.sh
```

1. **Agent 1 (Lead)**: Tiếp tục hoàn tất & merge vòng 25 (**`KT-328`** ∥ **`KT-329`**) đang chạy. Song song có thể giao ngay **`KT-331`**, **`KT-332`**, **`KT-333`**, **`KT-344`** (tất cả đều nằm **ngoài** `v2/kernel` và `v2/lib`, không xung đột nhánh `KT-328`/`KT-329`).
2. Ngay khi `KT-328` và `KT-329` đã `DONE` trên `main`: chạy độc quyền **`KT-330`** (soak cuối vòng 25 + `-cpu max` 600), rồi lập tức mở song song Đợt 1 (**`KT-334`** ∥ **`KT-336`**).
3. Ngay khi **`KT-344`** hoàn tất và chuyển `REVIEW`: báo **Agent 0** duyệt một lần để mở **`KT-345`** (không chặn Đợt 1 và Đợt 2).
4. Mỗi task hoàn tất phải có: **test đỏ trước** + **đột biến bắt buộc** + **ca âm** + **T1/T2 theo `AGENTS.md` §6.1** + **commit có nhãn `[KT-###]`** (không retry, không nới ngưỡng, không commit raw `boot-*.log` của soak).
