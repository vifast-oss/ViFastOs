> **ĐÃ HỦY BỎ (SUPERSEDED — D2-259, 2026-11-24)**: Theo chỉ đạo của người điều phối, toàn bộ kế hoạch phát hành cũ (`v2.0`/`v2.1`/`KT-348`) đã bị huỷ bỏ để tập trung 100 % vào đại tu kiến trúc. Kế hoạch duy nhất hiện hành là [`KE-HOACH-KIEN-TRUC.md`](../../../KE-HOACH-KIEN-TRUC.md). Tệp này chỉ giữ làm tư liệu lịch sử (đọc-only).

# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348b`, 38 task)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; **D2-236..D2-240** tại `5e635474`; `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **D2-244..D2-245** tại `bfb9a13d`; **D2-247..D2-248** tại `3a8219a`; **D2-250..D2-251** tại `ba0b6b7`; **D2-252** tại `7d37fcee`; **D2-255** tại `14b5de95`; **D2-256** tại `c1c068a`; **D2-257** tại `da75f5b`; **D2-258** tại `899cbcb` sau khi **`KT-348a` + `KT-348b` `DONE` 37/38 task, `KT-348` mở `READY`** & tinh chỉnh thực tế 26 task Chu kỳ 3).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-258**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md), [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md).

> **Mục tiêu một câu**: Đợt 3D (**`KT-348a`** & **`KT-348b`**) đã về đích sạch (**37/38 `DONE`** tại `899cbcb`, x64 `600/600` & a64 `1000/1000` PASS) ⇒ chạy ngay **`KT-348` (`READY`)** độc quyền ở khung **Cao điểm DeepSeek (0 token LLM, `df ≥ 15 GiB`)** sinh `R2-v2` (công bố rõ ranh giới soak `D2-258`) → chốt tag **`v2.0`** → mở khoá **Chu kỳ 3 (`v2.1` — 26 task ≤ 3 h `KT-350..KT-366` với Kiến trúc Lõi Kép `Machine<S: KernelStore>` khớp 100 % cỡ ABI `512 B`/`64 B` & dọn kho sớm ngay Đợt 1)**.

---

## 0. Chẩn đoán đầu vào (cập nhật thực thi tại `899cbcb`, 2026-11-24)

Toàn bộ Đợt 0..Đợt 3D đã hoàn tất (**37/38 task `DONE`**; chỉ còn duy nhất **`KT-348` `READY`**):
- **Đợt 3D (`KT-348a` + `KT-348b`) về đích 100 % (`899cbcb`)**:
  - **`KT-348a` (`DONE`)**: chứng minh gốc rễ treo 1/600 là **tín hiệu `SIGTERM` từ tiến trình ngoài** (không phải lỗi timer); sửa `qg_boot` trả `rc=126` ⇒ `TỪ CHỐI ĐO` + gia cố `ioapic_init`/`timer_init` theo `D2-257` + bất biến `# timer: lapic-window` + `guard_extsig_selftest` 7/7 nối T0; xác nhận soak `x86_64` **600/600** PASS (`ok=600 fail=0`). Ghi nhận nợ mới **`N-7`** (2 thư mục build lệch `BUILD_ID`/`.text` digest) chuyển vào **`KT-353d`**.
  - **`KT-348b` (`DONE`)**: chứng minh gốc rễ 7/1000 boot mất log là **`ENOSPC` (đĩa đầy)** + `qg_start` vẫn trả 0, còn timeout phím là HMP nối muộn; thêm fail-closed `rc=125`, `soak_a64_log_inventory`, `monitor-sendkey.py` nối HMP trước, gộp đúng 1 `trap EXIT`; xác nhận soak `aarch64` **1000/1000** PASS (`0` lỗi, `sendkey ok=1000`). Bổ sung điều kiện chặn **`df ≥ 15 GiB`** trước `KT-348` và chuyển phần đồng bộ `rc=126`/`log_inventory`/1 `trap` trên các runner còn lại sang **`KT-359a`**.
- **Tinh chỉnh thực tế Chu kỳ 2 & Chu kỳ 3 (`D2-258`)**:
  1. **Công bố trung thực ranh giới soak `KT-348`**: ở `v2.0` (`KT-348`), `riscv64` soak thật ảnh `product-shell` + đĩa `virtio-blk`; `aarch64`/`x86_64` kiểm build + budget + `D2-08` + TAP ảnh `product-shell` và soak `ktest` trên máy đo macOS (Mach-O DNS interpose) — **bắt buộc ghi rõ ranh giới này + nợ `N-7` trong `R2-v2.md`** (runner UART shell thật & Linux `LD_PRELOAD` mở ở `KT-357b`).
  2. **Tinh chỉnh thực tế Chu kỳ 3 (`D2-258`)**: nâng cấp [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md) thành **26 task ≤ 3 h (`KT-350..KT-366`)** sau kiểm toán mã nguồn: vá bẫy cỡ ABI `512 B`/`64 B` (`TcbCore ≤ 496 B`, `EndpointCore ≤ 48 B`), bao phủ đủ 4 bảng tĩnh (`finalise_log`/`ipc_msgs`/`wait_nodes`/`regions`), bảo toàn `Meta == 48 B` & trần `184.832 B`, kéo sớm dọn kho `KT-359b` (Đợt 1) & UEFI `KT-364b` (Đợt 7), tách gọn 5 task quá tải (`KT-355a/b/c`, `KT-353d/b`, `KT-357a/b`, `KT-364a/b`, `KT-365a/b`), và hấp thu nợ `N-7` (`KT-353d`) + harness `rc=126`/`log_inventory` (`KT-359a`).

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229..251** | **Chu kỳ 2 nền tảng**: NTFS guard, driver chung, 1 lõi `vifast-core`, giải nghẽn `.bss`, cấm CI GitHub (`D2-241`), ABI v1.1 (`7C-r13`), Audit Q1-A 2 tầng, DNS retry (`D2-246`), khóa 2 lớp/`wt.sh`/`v2 t0`/`agentctl lint`, ngân sách `product-shell` 3 arch (`D2-250`) | Đã hoàn tất 100 % trên 3/3 arch; `spec/` khoá cứng. |
| **D2-252..255** | **Refactor 3 Pha (`D2-252`), R5 `v2/user/runtime/*` (`D2-253`), `_t0_tool` (`D2-254`), hiệu đính Đợt 3B (`D2-255`)** | Đợt 3B & Đợt 3C hoàn tất 100 % (T2 #7 PASS 75/75, `exec` từ FS, `vifast-rt` PoC, 3 thư viện harness chung). |
| **D2-256** | **Quy chuẩn lịch vận hành theo khung giờ Cao điểm (2×) / Thấp điểm (0,5×) DeepSeek (`AGENTS.md` §6.4)** | Thấp điểm: phát triển 4 slot T0+T1; Cao điểm (`01:00–04:00` & `06:00–10:00 UTC` T2–T6): chạy nặng cục bộ T2/soak/`nightly` với 0/tối thiểu token LLM. |
| **D2-257** | **Vá DAG `KT-348a`/`KT-348b` → `KT-348` & Định hướng Lõi Kép `Machine<S: KernelStore>`** | `KT-348a` + `KT-348b` đã hoàn tất (`899cbcb`, **37/38 `DONE`**); **`KT-348` mở `READY`**. |
| **D2-258** | **Tinh chỉnh thực tế kế hoạch Chu kỳ 3 (`v2.1`, 26 task ≤ 3 h) & ranh giới trung thực `KT-348`** | Vá bẫy cỡ ABI `512/64 B` (`TcbCore`/`EndpointCore`), phủ 4 bảng tĩnh trong `KernelStore`, bảo toàn `Meta == 48 B`/`184.832 B`, kéo sớm `KT-359b`/`KT-364b`, tách gọn 5 task quá tải, hấp thu nợ `N-7`/`rc=126` & đồng bộ `STATE.md`. |

---

## 2. Lộ trình khép Chu kỳ 2 (`37 → 38/38 task`) & Đồ thị phụ thuộc

```text
[37/38 DONE tại 899cbcb — KT-348a (x64 600/600) + KT-348b (a64 1000/1000) DONE | 28/28 parity | vifast-rt + exec FS]
        │
        ▼ ĐỢT 3E — CỔNG NGHIỆM THU CHU KỲ 2 [Khung Cao điểm DeepSeek 2× 06:00–10:00 UTC | Máy đo độc quyền dirty=0, qemu_foreign=0, df≥15 GiB, 0 token LLM]
  └── (bao đóng đủ 37/37 task, 0 mồ côi) ──► KT-348 [READY] (T2 75/75 + nightly --all + soak 3 arch & virtio-blk + R2-v2 ghi rõ ranh giới ⇒ v2.0)
                                                 │
                                                 ▼ (sau APPROVED A0 chốt tag v2.0)
                                          [CHU KỲ 3 — v2/docs/ke-hoach-chu-ky-3.md (26 task ≤ 3 h: KT-350..KT-366, D2-258)]
```

---

## 3. Chi tiết task cuối cùng tới `v2.0` (`KT-348`, sau khi `KT-348a`/`KT-348b` `DONE`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) & Ràng buộc `D2-257`/`D2-258` |
|---|---|---|---|---|---|---|
| **KT-348a** | P0 | **`x86_64` treo 1/600 ở TAP 10 (tín hiệu ngoài SIGTERM)** | `KT-341, KT-347b` | 3 h | `DONE` | **`899cbcb`**: `qg_boot` trả `rc=126` (`TỪ CHỐI ĐO`), `guard_extsig_selftest` 7/7 nối T0, gia cố `ioapic_init`/`timer_init` + bất biến `lapic-window`, soak `x86_64` **600/600** PASS; nợ `N-7` digest `.text` chuyển `KT-353d`. |
| **KT-348b** | P0 | **`aarch64` soak 7/1000 mất log (`ENOSPC`) + timeout phím** | `KT-338b, KT-347b` | 3 h | `DONE` | **`899cbcb`**: `qg_start` mở log trước `exec` (`rc=125`), `soak_a64_log_inventory`, `monitor-sendkey.py` nối HMP trước, gộp 1 `trap EXIT`, soak `aarch64` **1000/1000** PASS; nợ đồng bộ runner chuyển `KT-359a`. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | bao đóng đủ 37/37 task (`… KT-347, KT-347b, KT-348a, KT-348b`) | 3 h | `READY` | **Chạy ở khung Cao điểm DeepSeek (`D2-256`, 0 token LLM, máy đo độc quyền `dirty=0`, `qemu_foreign=0`, đĩa trống `≥ 15 GiB`)**: T2 75/75 PASS; `v2 nightly --all` (L1 100k × 4 SMP + Miri + `STATUS.md`) PASS; soak `riscv64` `product-shell` 1.000 + đĩa thật `virtio-blk`, `aarch64` 1.000, `x86_64` 600 `qemu64` + 600 `-cpu max` đều 0 lỗi (không retry `D2-07`; **ghi rõ trong `R2-v2` ranh giới `D2-258` + nợ `N-7`**: `a64`/`x64` kiểm build+budget+D2-08+TAP ảnh `product-shell` và soak `ktest`, runner UART shell thật + Linux `LD_PRELOAD` nằm ở `KT-357b`); đối chiếu Exit Criteria `v2.0` (§6.2); xuất `R2-v2.{md,json}` qua `check_report.py --all` + `verify-numbers.py`; trình Agent 0 chốt `v2.0` & mở Chu kỳ 3 (`26 task`). |

---

## 4. Vùng xung đột & quy chế vận hành bắt buộc (**D2-236..D2-258**)

| Vùng tệp / Nút thắt | Rủi ro đã đo được | Quy tắc bắt buộc |
|---|---|---|
| **Lịch giờ DeepSeek (`D2-256`) & đĩa trống (`KT-348b`)** | Đốt token 2× khi chạy dài; `ENOSPC` làm mất log soak | Chạy độc quyền **`KT-348`** trong khung **Cao điểm 2 (`06:00–10:00 UTC`, 0 token LLM)** sau khi xác nhận **`df ≥ 15 GiB`**, `dirty=0`, `qemu_foreign=0`. |
| **Khóa 2 lớp, T2 sạch & quyền `v2/bin/**`** (`D2-248/D2-253`) | Chạy chồng QEMU gây `T-INFRA`; sub-agent sửa `v2/bin/**` | Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; **chỉ Lead chạy T2 (`v2 gate --fast`)** khi `qemu_foreign=0` và **chỉ Lead sửa `v2/bin/**`**. |
| **Đồng bộ `STATE.md` & DAG `QUEUE.md`** (`D2-248/D2-258`) | `W_STATE_HEAD_DRIFT` / `W_STATE_COUNT` | Giữ `STATE.md` sạch 0 drift (`KT-243..KT-348b (124 task)`), giữ `vifast-agentctl lint` **0 lỗi**. |
| **Bảo vệ ghim `spec/` & `GOI-ABI-V1_1.md`** (`D2-255/D2-257`) | Đổi tên `machine.rs` → `machine/mod.rs` hoặc xoá `GOI-ABI-V1_1.md` làm đỏ `check_kernel.py` | Cấm tuyệt đối sửa `spec/abi.toml`/`spec/kernel.toml`, 19 tệp `[[legacy]]`, `.github/workflows/**` (`D2-241`), và bảo toàn 6 tệp bị `spec/kernel.toml` ghim. |

---

## 5. Lịch thực thi đợt cuối khép Chu kỳ 2 theo khung giờ DeepSeek (`D2-256`)

| Đợt & Khung giờ | Công việc chạy độc quyền | Cổng Lead | Kết quả |
|---|---|---|---|
| **Đợt 3E (`KT-348`)** *(Khung Cao điểm 2×: `06:00–10:00 UTC` hoặc `01:00–04:00 UTC`, 0 token LLM)* | **MÁY ĐO ĐỘC QUYỀN (`dirty=0`, `qemu_foreign=0`, `df ≥ 15 GiB`)**: T2 75/75 + `v2 nightly --all` + `v2 nightly --soak --profile all` (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600` × 2 CPU) | Sinh **`R2-v2.{md,json}`** (ghi rõ ranh giới `D2-258` & nợ `N-7`), `check_report.py --all` + `verify-numbers.py`, push | **38/38 `DONE`** → Trình A0 chốt **`v2.0`** & mở **Chu kỳ 3 (26 task)**! |

---

## 6. Lộ trình Chu kỳ 3 (`v2.1`, 26 task `KT-350..KT-366`) & Bảng Exit Criteria `v2.0` vs `v2.1`

### 6.1 Bản thiết kế thực tế & tối ưu Chu kỳ 3 (`v2.1` — `D2-258`)
Toàn bộ kế hoạch Chu kỳ 3 được tinh chỉnh thành **26 task ≤ 3 h (`KT-350..KT-366`, 9 đợt phát triển 0 % chồng chéo tệp + 1 đợt nghiệm thu)** tại **[`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md)**:
- **Lõi Kép 3 bước sạch (`KT-355a` → `KT-355b` → `KT-355c`)**: tách `TcbCore ≤ 496 B` & `EndpointCore ≤ 48 B` khớp 100 % `spec/abi.toml` `[object_size]`, bao phủ đủ 4 bảng tĩnh (`finalise_log`/`ipc_msgs`/`wait_nodes`/`regions`), giữ `Meta == 48 B` và `Machine<StaticStore> ≤ 184.832 B` (`ViFastOs-Mission`), đồng thời mở `UntypedStore` + `BootInfo` Dynamic Arena (`ViFastOs-General`, `≥ 512 TCB / ≥ 1.024 Cap`).
- **Tối ưu đường găng & quy mô ≤ 3 h/task**: kéo sớm dọn kho `KT-359b` lên Đợt 1 và UEFI `KT-364b` lên Đợt 7; tách gọn `KT-353d`/`KT-353b`, `KT-357a`/`KT-357b` (gồm runner `product-{a64,x64}.sh` & Linux `LD_PRELOAD`), `KT-364a`/`KT-364b`, `KT-365a`/`KT-365b`.

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** (`KT-348`) | Chuyển sang **`v2.1`** / Chu kỳ 3 (`KT-350..KT-366`) |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + ảnh `product-shell` (0 marker, D2-08)** | **ĐÃ ĐẠT** (`rv`, `KT-339a` `a64`, `KT-341` `x64`) | `SMP-1` preemptive 4 hart (`KT-350`/`KT-351`), UART RX IRQ + runner `product-{a64,x64}.sh` (`KT-357b`), hợp nhất HAL/`vifast-kbuild` & gom 14 bin `rv` (`KT-353a/d/b/c`) |
| **Mô hình bộ nhớ `vifast-core`** | Bảng tĩnh `.corespace` (`Static`, `184.792 B ≤ 184.832 B`) | **Lõi Kép `Machine<S: KernelStore>`**: giữ `StaticStore` (`Mission`, `Meta == 48 B`) + mở `UntypedStore` & `BootInfo` Arena phủ đủ 100 % bảng (`General`, `≥ 512 TCB`, `KT-355a/b/c`) |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active` — ĐÃ ĐẠT** (`KT-345b`, `smp=1,4`, `deferred=0`) | `KT-345c` (`socket.vidl` TCP đầy đủ) + `ping`/`ifconfig`/`free` DRAM + `Tcb.Exec(path)` + `ViShell` (`KT-362`/`KT-363`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **ĐÃ ĐẠT** (**N5** đóng `KT-340d`; Audit Q1-A/N7 tầng 1 đủ 3/3 port) | Tầng 2 `vifast_sync::Mailbox<N>` + cờ `OVERFLOW` + range TLB shootdown (`KT-352`) |
| **Ngân sách & Cổng** | `*-product < 64 KiB`, `*-product-shell ≤ 256 KiB` + `.corespace ≥ 184.792 B`, T2 75/75, `nightly --all` (`KT-348`) | `.corespace` NOLOAD+NX 3 arch (`KT-353a`) + `v2 mutants` thay 43 `mut-*` + trace 32 B + `vifast-bench` (`KT-354`) |
| **Soak nghiệm thu** (`D2-07` không retry, `qemu_foreign=0`, `dirty=0`) | 0 lỗi trên `rv 1000` `product-shell` + `virtio-blk`, `a64 1000` (`ktest`), `x64 600 qemu64 + 600 -cpu max` (`ktest`) trên máy đo macOS (ghi rõ ranh giới `D2-258` trong `R2-v2`) | Soak `product-shell` lái UART thật **3/3 arch** 4 hart + Linux `LD_PRELOAD` (`KT-357b`) + `VSH2` + UEFI + crosscheck `ViScript` (`KT-366`) |
| **Tái lập byte (P20), Vệ sinh kho & Runtime** | **ĐÃ ĐẠT** P20 3 arch (`KT-346`), `exec` từ FS + PoC `vifast-rt` (`KT-347`), `boot/lib/` chung (`KT-347b`) | Pha C+ dọn kho ngay Đợt 1 (`KT-359b`) + Pha B5 (`KT-359a`); FPU/SIMD (`KT-356`); UEFI sớm (`KT-364b`); IRQ driver (`KT-361`); Rust `std` (`KT-364a`); IOMMU/NVMe (`KT-365a/b`) |
