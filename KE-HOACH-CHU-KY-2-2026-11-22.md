# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; **D2-236..D2-240** tại `5e635474`; `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **D2-244..D2-245** tại `bfb9a13d`; **D2-247..D2-248** tại `3a8219a`; **D2-250..D2-251** tại `ba0b6b7`; **D2-252** tại `ff042c63` sau khi **31/36 task `DONE` & T2 #5 PASS 75/75**).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-252**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md), [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md).

> **Mục tiêu một câu**: Hoàn tất đúng **5 task cuối Chu kỳ 2** (**31 → 36/36 task**) kèm rào chắn Pha A (`D2-252`): **`KT-345b`** (28/28 parity + `netsrv` đa caller `6A`) ∥ **`KT-346`** (`v2/user/` + P20 3 arch) → **`KT-347`** (PoC `U1a`) ∥ **`KT-347b`** (`boot/lib/` + dọn gốc) → **`KT-348`** (`v2.0`, `R2-v2`) để mở **Chu kỳ 3 (20 task `KT-350..KT-366` gồm Pha B refactor lõi/HAL/tools & Pha C tinh gọn kho)**.

---

## 0. Chẩn đoán đầu vào (cập nhật thực thi tại `ff042c63`, 2026-11-23 21:10 JST)

Toàn bộ Đợt 0, Đợt 1, Đợt 2 và Đợt 3A đã hoàn tất sạch (**31/36 task `DONE`**, **T2 #5 PASS 75/75 `649,2 s`** trên máy rảnh `qemu_foreign=0`, `vifast-agentctl lint` **0 lỗi**, 0 worktree rò rỉ):
- **Cả 3/3 kiến trúc đều chạy ảnh sản phẩm `product-shell` 0 marker trên 1 `vifast-core::Machine` + 1 nguồn `procsrv`/`shell`**: `riscv64` (`224.472 B`, FS+`netsrv`+`exec`, **25/28 active** parity `smp=1,4`, lỗi `NBSend` đã sửa gốc `direct_pushed=1`), `aarch64` (`696 B` + `.corespace` NOLOAD, TAP 46/46), `x86_64` (`51.176 B` + `.corespace` NOLOAD + `procsrv`, TAP 47/47, gpu 48).
- **Điểm nóng R1 cần rào chắn ngay (Pha A `D2-252`)**: `core_ipc.rs` (`1.498/1.500`), `plat-x86_64/asm.rs` (`1.475`), `machine.rs` (`1.430`), `shell.rs` (`1.414`) ⇒ `KT-345b` đặt lệnh mới vào `shell_cmds.rs`, không phình `shell.rs` và không chạm `core_ipc.rs`; xả toàn bộ R1 + tái kiến trúc HAL/kho trong Pha B & C ngay sau tag `v2.0`.

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229..240** | **Mở Chu kỳ 2, NTFS guard, driver chung, 1 lõi `vifast-core`, giải nghẽn `.bss`, Khung Chu kỳ 3 & Exit Criteria `v2.0`** | Giữ mọi ngưỡng cứng (`.data+.bss ≤ 256 KiB`, `Machine ≤ 196.608 B`, `R1 ≤ 1.500`, `D2-07/08/17`). |
| **D2-241..246** | **Cấm GitHub Actions; duyệt & áp ABI v1.1 (`7C-r13`, `VspaceOutcome`); Audit Q1-A 2 tầng; quy chế Agent 2; DNS retry `D2-246`** | Đã hoàn tất 100 % trên 3/3 arch; `spec/` khoá cứng trở lại. |
| **D2-247..251** | **Khóa 2 lớp/`v2/bin/lib/`/`wt.sh`/`v2 t0`/`agentctl lint`; `KT-333e/f`/`KT-342a` (D2-249); ngân sách `product-shell` 3 arch + `KT-339a`/`KT-342b` (D2-250); Chu kỳ 3 (D2-251)** | Đã hoàn tất toàn bộ Đợt 3A (31/36 `DONE`, T2 #5 PASS 75/75). |
| **D2-252** | **Khắc hợp đồng `NBSend`/`.corespace` & Kế hoạch Refactor Kiến trúc — Cấu trúc Kho 3 Pha** *(đã duyệt)* | Pha A (rào chắn R1 `KT-345b`, tách `v2/user/` `KT-346`, dọn tệp gốc `KT-347b`); Pha B (`KT-353a/b/c`, `KT-354`, `KT-359a`); Pha C sau `v2.0` (`KT-359b`: archive rác `JOURNAL/` + `docs/bundles/`, gỡ cây mã v1 đóng băng khỏi `main`, giữ `v1-archive` & `docs/v1/` đọc-only). |

---

## 2. Lộ trình 5 task cuối & đồ thị phụ thuộc (`KT-330..KT-348`, 36 task ≤ 3 h)

```text
[31/36 DONE tại ff042c63 — T2 #5 PASS 75/75 (649,2 s) | agentctl lint 0 lỗi | 3/3 arch product-shell]
  DONE (31): KT-330..KT-333e(+f), KT-334..KT-337, KT-338a..c, KT-339, KT-339a,
             KT-340a/b/d/c, KT-341, KT-342(+a), KT-342b, KT-343 (25/28), KT-344, KT-345a (spec/ khoá)
        │
        ▼ (ĐỢT 3B — 2 SLOT READY NGAY, 0 CHỒNG CHÉO TỆP)
  ├── [Slot 1] KT-345b [READY] (free/df/loop vào shell_cmds.rs → 28/28 parity + netsrv đa caller 6A)
  └── [Slot 2] KT-346  [READY] (tách trọn userland → v2/user/ + VBREV_ALGO kt134-v5 + đóng P20 3 arch)
        │
        ▼ (ĐỢT 3C — 2 SLOT SONG SONG TIỀN NGHIỆM THU)
  ├── (KT-341, KT-346)                            ──► KT-347  (PoC U1a + .vidl ViScript + UEFI/DTB/ACPI)
  └── (KT-333e, KT-339a, KT-341, KT-345b, KT-346) ──► KT-347b (boot/lib/ chung + nightly --soak product-shell/blk + dọn gốc)
        │
        ▼ (ĐỢT 3D — CỔNG NGHIỆM THU CHU KỲ 2, MÁY ĐO ĐỘC QUYỀN)
  └── (bao đóng 35/35 task, 0 mồ côi)             ──► KT-348  (T2 75/75 + nightly --all + soak 3 arch + R2-v2 ⇒ v2.0)
                                                         │
                                                         ▼ (sau APPROVED A0 chốt tag v2.0)
                                                  [CHU KỲ 3 — v2/docs/ke-hoach-chu-ky-3.md (20 task: KT-350..KT-366, Pha B + C)]
```

---

## 3. Chi tiết hiện trạng & 5 task cuối tới `v2.0`

### Đợt 0, Đợt 1, Đợt 2 & Đợt 3A — ĐÃ HOÀN TẤT 100 % (`31/36 DONE`)

| Nhóm | Task đã `DONE` | Kết quả đo được |
|---|---|---|
| **Đợt 0 & 1** | `KT-330..KT-333e(+f)`, `KT-334..KT-337`, `KT-344`, `KT-345a` | T2 75 bước; repo `2.874` tệp; 40 GiB đĩa trống; driver chung −42..61 %; `D2-246` 3 arch; ABI v1.1 9/9 (`spec/` khoá). |
| **Đợt 2 & 3A** | `KT-338a/c/b`, `KT-339`, **`KT-339a`**, `KT-340a/b/d/c`, **`KT-341`**, `KT-342(+a)`, **`KT-342b`**, **`KT-343`** | Cả 3 arch chạy `product-shell` 0 marker (`rv 224.472 B`, `a64 696 B + .corespace`, `x64 51.176 B + .corespace`); đóng **N5** & Audit Q1-A 3/3; `user.rs` 704 + sửa gốc `NBSend` (`direct_pushed=1`); parity **25/28** (`smp=1,4`); **T2 #5 PASS 75/75**. |

### Đợt 3B → 3D — 5 task cuối cùng (`KT-345b`, `KT-346`, `KT-347`, `KT-347b`, `KT-348`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) + Rào chắn Pha A (`D2-252`) |
|---|---|---|---|---|---|---|
| **KT-345b** | P1 | **Thi hành ABI v1.1: `free`/`df`/`loop` (28/28 parity) + `NetSvc` đa caller** | `KT-342, KT-342b, KT-343, KT-345a` | 3 h | `READY` | `netsrv` U-mode đa caller theo `socket.vidl` (`6A`); `OP_STATFS` + `MemSvc` (`4C`) đóng `free`/`df`/`loop` (**28/28 active**, `smp=1,4`). **Rào chắn R1**: viết 3 lệnh mới vào `shell_cmds.rs`, không phình `shell.rs` (`1.414/1.500`), không chạm `core_ipc.rs` (`1.498/1.500`). |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184 (3 arch)** | `KT-339a, KT-341, KT-342` | 3 h | `READY` | Chuyển trọn nguồn userland sản phẩm (`vifast-plat-user` + `kbin-riscv64/init/*_prod_user.rs`) vào `v2/user/` + `--remap-path-prefix`; tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + `build-rev.algo.pin` + `wake_scope.py`; cùng sha256 trên 2 thư mục build cả 3 arch (Linux/macOS); `exec` nạp ELF từ FS chạy thật. |
| **KT-347** | P2 | **Thiết kế & PoC `U1a` (`no_std+alloc` `vifast-rt`) + hợp đồng `.vidl` `ViScript` + hướng `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `BLOCKED` | 1 ứng dụng user `U1a` (stable `1.100.0`) chạy thật qua `exec` từ FS; `v2/docs/u1-uefi-readiness.md` chốt `U1a`/`U1b`, hợp đồng `.vidl` & kiểm chéo cục bộ `ViFastOs ↔ ViScript` (D2-241), và `UEFI`/DTB/ACPI. |
| **KT-347b** | P1 | **Hợp nhất `v2/test/boot/lib/` + nối `nightly --soak --profile product-shell` & `virtio-blk` + dọn tệp gốc** | `KT-333e, KT-339a, KT-341, KT-345b, KT-346` | 2 h | `BLOCKED` | **(D2-245/D2-252)** Rút `v2/test/boot/lib/{harness-scan,budget-common,soak-common}.sh`; nối `--profile product-shell` (3 arch + `x64-max`) & **soak đĩa thật `virtio-blk`** vào `v2 nightly --soak`; dọn `BLOCKED.md`, dời `KE-HOACH-CUOI-TUAN-2026-11-{20,21}.md` → `v2/agent/archive/reports/` & `GIAI-THICH-VIFASTOS-CHO-SINH-VIEN.md` → `v2/docs/guides/` (cập nhật `README.md`). |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | bao đóng 35/35 task | 3 h | `BLOCKED` | Máy đo độc quyền (`dirty=0`, `qemu_foreign=0`): T2 75/75 PASS; `v2 nightly --all` (L1 100k × 4 SMP + Miri + `STATUS.md`) PASS; soak `product-shell` 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64` + `600 -cpu max`) đều 0 lỗi; đối chiếu Exit Criteria `v2.0` (§6.2); xuất `R2-v2.{md,json}`; trình Agent 0 chốt `v2.0` & mở Chu kỳ 3. |

---

## 4. Vùng xung đột & quy chế vận hành tối ưu cho Agent 2 (**D2-236..D2-252**)

| Vùng tệp / Nút thắt | Rủi ro đã đo được | Quy tắc bắt buộc |
|---|---|---|
| **Điểm nóng R1 (`1.500` dòng)** (`D2-252`) | `core_ipc.rs` (`1.498`), `plat-x86_64/asm.rs` (`1.475`), `machine.rs` (`1.430`), `shell.rs` (`1.414`) | `KT-345b` viết lệnh vào `shell_cmds.rs`, không chạm `core_ipc.rs`; xả triệt để toàn bộ điểm nóng về `< 950` LOC ngay ở `KT-353a`. |
| **Khóa 2 lớp & T2 sạch** (`D2-248/D2-249`) | Chạy T2 song song 4 slot gây T-INFRA | Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; **chỉ Lead chạy T2 (`v2 gate --fast`)** khi `qemu_foreign=0`. |
| **Hợp đồng `product-shell` & `.corespace`** (`D2-250/D2-252`) | Đặt `Machine` về `.bss` vẫn lọt trần 256 KiB | Giữ chặt `*-product < 65.536 B`, `*-product-shell ≤ 262.144 B` + `.corespace ≥ 184.792 B` (a64/x64) + **0 marker** D2-08; thống nhất `riscv64` + NX ở `KT-353a`. |
| **Sổ sách & 4 slot worktree** (`D2-248`) | Lệch `STATE.md`/`QUEUE.md`; `kill` nhầm PID | Dùng `v2/tools/wt.sh` (`df ≥ 5 GiB`), giữ `vifast-agentctl lint` **0 lỗi**, chạy `sync-queue`, mỗi task 1 tệp `JOURNAL/kt<n>-<ten>.md`, chỉ `kill` PID slot mình (§9). |
| `spec/`, 19 tệp `[[legacy]]`, `.github/` | Vỡ đóng băng ABI; vỡ SHA legacy; vi phạm cấm CI | Cấm tuyệt đối sửa `spec/abi.toml`/`spec/kernel.toml`, 19 tệp `[[legacy]]` trong `numbers-policy.toml`, và `.github/workflows/**` (`D2-241`). |

---

## 5. Lịch chạy 3 đợt cuối khép Chu kỳ 2 (`v2.0`)

| Đợt | Slot 1 | Slot 2 | Cổng Lead (T2 / Soak) | Kết quả |
|---|---|---|---|---|
| **Đợt 3B** *(~2–3 h)* | **`KT-345b`** *(28/28 parity + `netsrv` đa caller `6A`, viết vào `shell_cmds.rs`)* | **`KT-346`** *(tách `v2/user/` + `VBREV_ALGO kt134-v5` + P20 3 arch)* | Merge lô 2 task → **T2 #6** (`qemu_foreign=0`, ~10 p) → `sync-queue` + push | **33/36 `DONE`** |
| **Đợt 3C** *(~2–3 h)* | **`KT-347`** *(PoC `U1a` + `u1-uefi-readiness.md` + `.vidl` `ViScript`)* | **`KT-347b`** *(`v2/test/boot/lib/` + `nightly --soak --profile product-shell` & `virtio-blk` + dọn gốc)* | Merge lô 2 task → **T2 #7** (`qemu_foreign=0`, ~10 p) → `sync-queue` + push | **35/36 `DONE`** |
| **Đợt 3D (`KT-348`)** *(~2,5 h độc quyền)* | **MÁY ĐO ĐỘC QUYỀN (`dirty=0`, `qemu_foreign=0`)**: T2 75/75 + `v2 nightly --all` | Soak `product-shell` 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600` ×2 CPU) | Xuất **`R2-v2.{md,json}`** qua `check_report.py --all` + `verify-numbers.py`, push | **36/36 `DONE`** → Trình A0 chốt **`v2.0`** & mở **Chu kỳ 3**! |

---

## 6. Lộ trình dài hạn Chu kỳ 3 (`v2.1`, 20 task `KT-350..KT-366`) & Tiêu chí phát hành `v2.0` (**D2-240/D2-250..D2-252**)

### 6.1 Bản thiết kế thi công Chu kỳ 3 (`v2.1` — gồm Pha B & Pha C `D2-252`)
Toàn bộ 7 trụ cột chiến lược cùng kế hoạch đại tu kiến trúc lõi/HAL/công cụ (**Pha B**: `KT-353a/b/c`, `KT-354`, `KT-359a`) và tinh gọn cấu trúc kho (**Pha C**: `KT-359b` — archive rác `JOURNAL/` + `docs/bundles/`, gỡ cây mã v1 đóng băng khỏi `main`, giữ nguyên `v1-archive` & `docs/v1/` đọc-only) đã được cụ thể hoá thành **20 task ≤ 3 h (`KT-350..KT-366`)** tại **[`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md)**. Chu kỳ 3 mở khoá vào `QUEUE.md` ngay sau khi `KT-348` ĐẠT và Agent 0 duyệt `R2-v2` / gắn tag `v2.0`.

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 (`KT-350..KT-366`) |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 ảnh `product-shell` (0 marker, D2-08)** | **Có — ĐÃ ĐẠT** (`rv`, `KT-339a` `a64`, `KT-341` `x64`) | `SMP-1` preemptive 4 hart (`KT-350`/`KT-351`) + hợp nhất HAL `vifast-plat-common` & gom 14 bin `kbin-riscv64` (`KT-353a/b/c`) |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active`** (`KT-343` 25/28 ĐẠT + `KT-345b`, `smp=1,4`) | Lệnh `ping`/`ifconfig` + `ViShell`/`ViScript` (`KT-362`/`KT-363`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có — ĐÃ ĐẠT** (**N5** đóng `KT-340d`; Audit Q1-A/N7 tầng 1 đủ 3/3 port) | Tầng 2 `vifast_sync::Mailbox<N>` + `OVERFLOW` bit + range shootdown (`KT-352`) |
| **Ngân sách & Cổng** (`*-product < 64 KiB`, `*-product-shell ≤ 256 KiB` + `.corespace ≥ 184.792 B`, `Machine ≤ 196.608 B`, T2 75/75, `nightly --all`) | **Có** (`budget*.sh`, `KT-333b..f`, `KT-347b`, `KT-348`) | `.corespace` NOLOAD+NX 3 arch (`KT-353a`) + `v2 mutants` thay 44 feature đột biến + trace 32 B + `vifast-bench` (`KT-354`) |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`, `dirty=0`) | **Ảnh `product-shell`** 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) (`KT-348`) | Soak 4 hart U-mode + `VSH2` tải nặng (`KT-366`) |
| **Tái lập byte (P20) & Vệ sinh kho** | **Cùng sha256 canonical cả 3 arch trên Linux + macOS** (`KT-346`); kho `2.874` tệp + `boot/lib/` (`KT-347b`) | Pha B/C: tách `check-wake-paths.py` + `v2/tools/` (`KT-359a`); archive `JOURNAL/`+`docs/bundles/` & gỡ cây mã v1 khỏi `main` (`KT-359b`) |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md`** (`KT-333b`), clone sạch Win/macOS/Linux (`KT-331`) — **ĐÃ ĐẠT** | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **`vifast-objmem`, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | PoC `U1a` + tài liệu định hướng (`KT-347`) | Thi công ở **Chu kỳ 3 (`KT-355..KT-366`)** theo `v2/docs/ke-hoach-chu-ky-3.md` |

