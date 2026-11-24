# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348b`, 38 task)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; **D2-236..D2-240** tại `5e635474`; `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **D2-244..D2-245** tại `bfb9a13d`; **D2-247..D2-248** tại `3a8219a`; **D2-250..D2-251** tại `ba0b6b7`; **D2-252** tại `7d37fcee`; **D2-255** tại `14b5de95`; **D2-256** tại `c1c068a`; **D2-257** tại `da75f5b` sau khi **Đợt 3C `DONE` 35/38 task, T2 #7 PASS 75/75 & mở `KT-348a`/`KT-348b`**).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-257**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md), [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md).

> **Mục tiêu một câu**: Xử lý tận gốc **2 lỗi thật vừa lộ dưới soak nghiệm thu** (**`KT-348a`** treo 1/600 `x86_64` ở TAP 10 ∥ **`KT-348b`** mất log 7/1000 + timeout phím ở `soak-aarch64.sh` — **không nới ngưỡng, vá chặt bao đóng DAG**) trong khung **Thấp điểm DeepSeek (0,5×)** → chạy **`KT-348`** (`v2.0`, `R2-v2`) độc quyền ở khung **Cao điểm DeepSeek (0 token LLM)** → mở khoá **Chu kỳ 3 (`v2.1` — 21 task `KT-350..KT-366` với Kiến trúc Lõi Kép `Machine<S: KernelStore>`: `StaticStore` + `UntypedStore` + `BootInfo` Dynamic Arena)**.

---

## 0. Chẩn đoán đầu vào (cập nhật thực thi tại `da75f5b`, 2026-11-24)

Toàn bộ Đợt 0, Đợt 1, Đợt 2, Đợt 3A, Đợt 3B và **Đợt 3C (`KT-347` + `KT-347b`)** đã hoàn tất sạch (**35/38 task `DONE`**, **T2 #7 PASS 75/75** trên `84adb4a2`):
- **Đợt 3C về đích 100 %**: **`KT-347`** dựng `vifast-rt` PoC (`v2/user/rt/`, `bump.rs` an toàn), chạy thật `/u1a.elf` (`16.736 B`) qua `exec` đọc trực tiếp từ đĩa FS (gỡ `include_bytes!`), và ban hành [`v2/docs/u1-uefi-readiness.md`](v2/docs/u1-uefi-readiness.md); **`KT-347b`** hợp nhất 3 thư viện harness (`harness-scan.sh`, `budget-common.sh`, `soak-common.sh`, `−326` dòng ở 7 runner), nối `rustfmt --check` cho `v2/user/**` vào `steps_check.sh`, và mở `v2 nightly --soak --profile`.
- **Phát hiện tại đợt chạy thử nghiệm thu `KT-348` (`da75f5b`) & khoanh vùng của Agent 0 (`D2-257`)**:
  1. **Lỗ hổng DAG cần vá ngay**: tại `da75f5b`, `KT-348a` và `KT-348b` được thêm ở trạng thái `READY` nhưng `KT-348` vẫn để `READY` và thiếu `KT-348a, KT-348b` trong cột phụ thuộc ⇒ **`D2-257` lật `KT-348` về `BLOCKED` phụ thuộc đủ `KT-348a, KT-348b`** (0 task mồ côi, chặn nhặt nhầm `KT-348` sớm).
  2. **Khoanh vùng gốc rễ `KT-348a` (`x86_64` treo 1/600 sau `ok 9 - apic: ioapic`, `# timer: lapic …`)**: trong `v2/kernel/vifast-kbin-x86_64/src/main.rs`, `measure_timer()` (`:653-664`) lặp `asm::hlt()` ngay sau `asm::sti()` (`:975`) — nếu không có ngắt đánh thức, `hlt` ngủ vĩnh viễn không chạm `HZ_WINDOW_SPIN_BOUND`! Kiểm tra ngay 3 điểm nghi vấn trong `timer_init()` (`:524-597`): (i) dòng `:590` ghi `ioapic_write(reg, VEC_TIMER)` **mở mask trước** khi dòng `:591` ghi `ioapic_write(reg + 1, 0)` đặt Destination LAPIC ID (trong khi `ioapic_init()` chưa từng xoá `reg + 1`); (ii) cửa sổ probe `LVT_TIMER` không mask (`:524-552`) cùng vector `32` khi `IF=0` có thể để lại trạng thái `IRR`/`ISR` hoặc tranh cạnh với IOAPIC pin 0 / PIC 8259 / MADT ISO `IRQ0→GSI2`; (iii) tái hiện với `-d int` để chốt bằng chứng trước khi sửa.
  3. **Khoanh vùng gốc rễ `KT-348b` (`aarch64` soak: 7/1000 boot không có log + 1 boot `not ok 32 - input: key` timeout `5076 ms`)**: phát hiện trực tiếp 2 lỗi trong `v2/test/boot/soak-aarch64.sh`: (i) dòng `:1546` `trap 'rm -rf "$SOCKDIR" ...' EXIT` **ghi đè làm mất toàn bộ `trap EXIT` ở dòng `:1512`** (`stop_soak_dns_server; stop_soak_tcp_server; qg_cleanup`); (ii) dòng `:1614` `send_key "$sock" || true` nuốt im lỗi kết nối/ghi socket HMP dưới tải 3 luồng, đồng thời cần kiểm tra `DNS_WRAPPER` (`DYLD_INSERT_LIBRARIES` / `exec` lỗi trước khi mở stdout hoặc race tạo log trong `qg_start`). Tuyệt đối không nới ngưỡng 5 s.
  4. **Tối ưu toàn diện Chu kỳ 3 (`D2-257`)**: nâng cấp [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md) lên **21 task (`KT-350..KT-366`)** với **Kiến trúc Lõi Kép Đa hình `Machine<S: KernelStore = StaticStore>`** (`KT-355a` giữ nguyên `StaticStore` `0 unsafe` `Copy` cho nhúng/vũ trụ + `KT-355b` mở `UntypedStore` trên `vifast-objmem` & `BootInfo` dynamic arena xoá bỏ cả trần `64 TCB` lẫn `256 Obj / 512 Cap / 192 CNode slot`), khoá rào chắn `check_kernel.py` (`machine.rs` tệp cha, `plat-common/src/obj/mod.rs`), và xếp sóng 0 % chồng chéo tệp theo lịch DeepSeek (`D2-256`).

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229..251** | **Chu kỳ 2 nền tảng**: NTFS guard, driver chung, 1 lõi `vifast-core`, giải nghẽn `.bss`, cấm CI GitHub (`D2-241`), ABI v1.1 (`7C-r13`), Audit Q1-A 2 tầng, DNS retry (`D2-246`), khóa 2 lớp/`wt.sh`/`v2 t0`/`agentctl lint`, ngân sách `product-shell` 3 arch (`D2-250`) | Đã hoàn tất 100 % trên 3/3 arch; `spec/` khoá cứng. |
| **D2-252..255** | **Refactor 3 Pha (`D2-252`), R5 `v2/user/runtime/*` (`D2-253`), `_t0_tool` (`D2-254`), hiệu đính Đợt 3B (`D2-255`)** | Đợt 3B & Đợt 3C hoàn tất 100 % (**35/38 `DONE`**, T2 #7 PASS 75/75, `exec` từ FS, `vifast-rt` PoC, 3 thư viện harness chung). |
| **D2-256** | **Quy chuẩn lịch vận hành theo khung giờ Cao điểm (2×) / Thấp điểm (0,5×) DeepSeek (`AGENTS.md` §6.4)** | Thấp điểm: phát triển 4 slot T0+T1; Cao điểm (`01:00–04:00` & `06:00–10:00 UTC` T2–T6): chạy nặng cục bộ T2/soak/`nightly` với 0/tối thiểu token LLM. |
| **D2-257** | **Vá DAG `KT-348a`/`KT-348b` → `KT-348` khép Chu kỳ 2 & Nâng cấp Kế hoạch Chu kỳ 3 (`v2.1`, 21 task) với Kiến trúc Lõi Kép `KernelStore` (`StaticStore` + `UntypedStore`)** | Áp ngay cho `KT-348a`, `KT-348b`, `KT-348` và toàn bộ [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md). |

---

## 2. Lộ trình khép Chu kỳ 2 (`35 → 38/38 task`) & Đồ thị phụ thuộc

```text
[35/38 DONE tại da75f5b — T2 #7 PASS 75/75 | 28/28 parity | v2/user/ + vifast-rt + exec FS | boot/lib/ chung]
        │
        ▼ ĐỢT 3D — SỬA GỐC 2 LỖI SOAK [Khung Thấp điểm DeepSeek 0,5× | 2 Slot song song | 0 chồng tệp]
  ├── [Slot 1] KT-348a [READY] (x86_64 treo 1/600 ở TAP 10 sau ioapic/lapic — định vị -d int, sửa thứ tự IOAPIC/LVT/IRQ0, 0 nới ngưỡng)
  └── [Slot 2] KT-348b [READY] (aarch64 soak 7/1000 mất log + 1 timeout phím — sửa ghi đè trap EXIT, wrapper/log & HMP sendkey, 0 nới 5 s)
        │      └──► Lead gộp lô + cập nhật sạch STATE.md + chạy T2 #8 (75/75 PASS) trước biên Cao điểm 10 phút
        │
        ▼ ĐỢT 3E — CỔNG NGHIỆM THU CHU KỲ 2 [Khung Cao điểm DeepSeek 2× | Máy đo độc quyền dirty=0, qemu_foreign=0, 0 token LLM]
  └── (bao đóng đủ 37/37 task, 0 mồ côi) ──► KT-348 [BLOCKED] (T2 75/75 + nightly --all + soak 3 arch + virtio-blk + R2-v2 ⇒ v2.0)
                                                 │
                                                 ▼ (sau APPROVED A0 chốt tag v2.0)
                                          [CHU KỲ 3 — v2/docs/ke-hoach-chu-ky-3.md (21 task: KT-350..KT-366, Dual-Profile KernelStore)]
```

---

## 3. Chi tiết 3 task cuối cùng tới `v2.0` (`KT-348a`, `KT-348b`, `KT-348`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) & Khoanh vùng `D2-257` |
|---|---|---|---|---|---|---|
| **KT-348a** | P0 | **`x86_64` treo 1/600 ở TAP 10 (sau `ok 9 - apic: ioapic`, `# timer: lapic …`), QEMU bị guard kill bằng SIGTERM** | `KT-341, KT-347b` | 3 h | `READY` | **(D2-257)** Tái hiện có `-d int` / log trạng thái APIC; kiểm tra `timer_init()`/`measure_timer()` trong `vifast-kbin-x86_64/src/main.rs` (ghi `reg + 1` Destination **trước** khi bỏ mask `reg` ở `:590-591`, dọn trạng thái sau probe `LVT_TIMER` `:524-552`, kiểm tra định tuyến IRQ0/PIC/IOAPIC trước `sti(); hlt()`); viết ca đỏ-trước; **không** nới ngưỡng; soak `x86_64` 600/600 (`qemu64` & `-cpu max`) xanh; journal `kt348a-x64hang.md`. |
| **KT-348b** | P0 | **`aarch64` soak: 7/1000 boot KHÔNG có log + 1 boot `not ok 32 - input: key` (timeout 5076 ms)** | `KT-338b, KT-347b` | 3 h | `READY` | **(D2-257)** Chỉ ra gốc rễ có bằng chứng cho **cả hai** hiện tượng trong `soak-aarch64.sh`/`qemu_guard.sh` (sửa lỗi dòng `:1546` ghi đè mất `trap EXIT` của `:1512`; kiểm tra `DNS_WRAPPER` `exec`/flush gây 7 tệp log rỗng; xử lý lỗi trả về của `send_key` thay vì `|| true` ở `:1614` và đồng bộ HMP monitor); **không** nới ngưỡng 5 s hay 20 s; ca tự kiểm đỏ-trước; soak `aarch64` 1000/1000 xanh; journal `kt348b-a64log.md`. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | bao đóng đủ 37/37 task (`… KT-347, KT-347b, KT-348a, KT-348b`) | 3 h | `BLOCKED` | **Chạy ở khung Cao điểm DeepSeek (`D2-256`, 0 token LLM, máy đo độc quyền `dirty=0`, `qemu_foreign=0`)**: T2 75/75 PASS; `v2 nightly --all` (L1 100k × 4 SMP + Miri + `STATUS.md`) PASS; soak `product-shell` 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64` + `600 -cpu max`) đều 0 lỗi (không retry `D2-07`); đối chiếu Exit Criteria `v2.0` (§6.2); xuất `R2-v2.{md,json}` qua `check_report.py --all` + `verify-numbers.py`; trình Agent 0 chốt `v2.0` & mở Chu kỳ 3 (`21 task`). |

---

## 4. Vùng xung đột & quy chế vận hành bắt buộc (**D2-236..D2-257**)

| Vùng tệp / Nút thắt | Rủi ro đã đo được | Quy tắc bắt buộc |
|---|---|---|
| **Lịch giờ DeepSeek (`D2-256`)** | Đốt token 2× khi chờ QEMU / mở sub-agent sát giờ Cao điểm | Kiểm `date -u '+%u %H:%M UTC'`: **Thấp điểm (0,5×)** chạy `KT-348a ∥ KT-348b` (T0+T1), cắt trước `00:50` / `05:50 UTC`; **Cao điểm (2×)** chạy độc quyền `KT-348` bằng script cục bộ (0 token LLM). |
| **Khóa 2 lớp, T2 sạch & quyền `v2/bin/**`** (`D2-248/D2-253`) | Chạy chồng QEMU gây `T-INFRA`; sub-agent sửa `v2/bin/**` | Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; **chỉ Lead chạy T2 (`v2 gate --fast`)** khi `qemu_foreign=0` và **chỉ Lead sửa `v2/bin/**`**. |
| **Đồng bộ `STATE.md` & DAG `QUEUE.md`** (`D2-248/D2-257`) | `W_STATE_HEAD_DRIFT` (`STATE.md` còn ghi `KT-340c`/`KT-342` đang chạy); `KT-348` thiếu dep | Lead cập nhật sạch toàn bộ khối `STATE.md` ở vòng merge `KT-348a`/`KT-348b`; giữ `vifast-agentctl lint` **0 lỗi**. |
| **Bảo vệ ghim `spec/` & `GOI-ABI-V1_1.md`** (`D2-255/D2-257`) | Đổi tên `machine.rs` → `machine/mod.rs` hoặc xoá `GOI-ABI-V1_1.md` làm đỏ `check_kernel.py` | Cấm tuyệt đối sửa `spec/abi.toml`/`spec/kernel.toml`, 19 tệp `[[legacy]]`, `.github/workflows/**` (`D2-241`), và bảo toàn 6 tệp bị `spec/kernel.toml` ghim. |

---

## 5. Lịch thực thi 2 đợt cuối khép Chu kỳ 2 theo khung giờ DeepSeek (`D2-256`)

| Đợt & Khung giờ | Slot 1 | Slot 2 | Cổng Lead | Kết quả |
|---|---|---|---|---|
| **Đợt 3D** *(Giờ Thấp điểm 0,5×: `04:00–05:50 UTC` hoặc `10:00–01:00 UTC`)* | **`KT-348a`** *(sửa gốc treo `x86_64` TAP 10 + smoke/soak xác nhận)* | **`KT-348b`** *(sửa gốc `soak-aarch64.sh` mất log/trap/sendkey + smoke/soak xác nhận)* | Merge 2 nhánh + cập nhật sạch `STATE.md` + chạy **T2 #8** (`75/75 PASS`) trước mốc biên 10 phút | **37/38 `DONE`**, mở khoá `KT-348` → `READY` |
| **Đợt 3E (`KT-348`)** *(Giờ Cao điểm 2×: `06:00–10:00 UTC` hoặc `01:00–04:00 UTC`, 0 token LLM)* | **MÁY ĐO ĐỘC QUYỀN (`dirty=0`, `qemu_foreign=0`)**: T2 75/75 + `v2 nightly --all` | `v2 nightly --soak --profile all` (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600` × 2 CPU) | Sinh **`R2-v2.{md,json}`**, `check_report.py --all` + `verify-numbers.py`, push | **38/38 `DONE`** → Trình A0 chốt **`v2.0`** & mở **Chu kỳ 3 (21 task)**! |

---

## 6. Lộ trình Chu kỳ 3 (`v2.1`, 21 task `KT-350..KT-366`) & Bảng Exit Criteria `v2.0` vs `v2.1`

### 6.1 Bản thiết kế tối ưu Chu kỳ 3 (`v2.1` — Kiến trúc Lõi Kép `KernelStore` `D2-257`)
Toàn bộ kế hoạch Chu kỳ 3 đã được viết lại tối ưu thành **21 task ≤ 3 h (`KT-350..KT-366`, 8 đợt phát triển 0 chồng chéo tệp + 1 đợt nghiệm thu)** tại **[`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md)**, nổi bật với **Kiến trúc Lõi Kép Đa hình `Machine<S: KernelStore = StaticStore>`**:
- **`KT-355a` (`ViFastOs-Mission` — `StaticStore`)**: giữ nguyên đặc tính `0 unsafe`, `0` con trỏ, `#[derive(Clone, Copy)]`, `≤ 196.608 B` cho hệ thống an toàn cao / tàu vũ trụ / kiểm thử snapshot.
- **`KT-355b` (`ViFastOs-General` — `UntypedStore` + `BootInfo` Dynamic Arena)**: đặt thân object và slot `CNode` 48 B trực tiếp vào RAM `Untyped` qua `vifast-objmem` kết hợp arena động theo thông số CPU/RAM đọc từ `DTB`/`ACPI` (`KT-357`), xoá bỏ triệt để mọi trần cứng (`64 TCB / 256 Obj / 512 Cap / 192 CNode slot`) cho Desktop, Server và `ViShell`.

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** (`KT-348`) | Chuyển sang **`v2.1`** / Chu kỳ 3 (`KT-350..KT-366`) |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + ảnh `product-shell` (0 marker, D2-08)** | **ĐÃ ĐẠT** (`rv`, `KT-339a` `a64`, `KT-341` `x64`) | `SMP-1` preemptive 4 hart (`KT-350`/`KT-351`), runner UART thật `product-{a64,x64}.sh` (`KT-357`), hợp nhất HAL/`vifast-kbuild` & gom 14 bin `rv` (`KT-353a/b/c`) |
| **Mô hình bộ nhớ `vifast-core`** | Bảng tĩnh `.corespace` (`Static`, `184.792 B ≤ 196.608 B`) | **Lõi Kép `Machine<S: KernelStore>`**: giữ `StaticStore` (`Mission`) + mở `UntypedStore` & `BootInfo` Arena (`General`, `> 512 TCB / > 1.024 Cap`, `KT-355a/b`) |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active` — ĐÃ ĐẠT** (`KT-345b`, `smp=1,4`, `deferred=0`) | `KT-345c` (`socket.vidl` TCP đầy đủ) + `ping`/`ifconfig`/`free` DRAM + `Tcb.Exec(path)` + `ViShell` (`KT-362`/`KT-363`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **ĐÃ ĐẠT** (**N5** đóng `KT-340d`; Audit Q1-A/N7 tầng 1 đủ 3/3 port) | Tầng 2 `vifast_sync::Mailbox<N>` + cờ `OVERFLOW` + range TLB shootdown (`KT-352`) |
| **Ngân sách & Cổng** | `*-product < 64 KiB`, `*-product-shell ≤ 256 KiB` + `.corespace ≥ 184.792 B`, T2 75/75, `nightly --all` (`KT-348`) | `.corespace` NOLOAD+NX 3 arch (`KT-353a`) + `v2 mutants` thay 43 `mut-*` + trace 32 B + `vifast-bench` (`KT-354`) |
| **Soak nghiệm thu** (`D2-07` không retry, `qemu_foreign=0`, `dirty=0`) | 0 lỗi trên `rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max` (sau khi sửa gốc `KT-348a`/`KT-348b`) | Soak 4 hart U-mode cả 3 arch + `VSH2` + crosscheck `ViScript` (`KT-366`) |
| **Tái lập byte (P20), Vệ sinh kho & Runtime** | **ĐÃ ĐẠT** P20 3 arch (`KT-346`), `exec` từ FS + PoC `vifast-rt` (`KT-347`), `boot/lib/` chung (`KT-347b`) | Pha B5/C+ cắt ~62 % dung lượng kho (`KT-359a/b`); FPU/SIMD (`KT-356`); IRQ driver (`KT-361`); Rust `std` + UEFI (`KT-364`); NVMe/IOMMU (`KT-365`) |
