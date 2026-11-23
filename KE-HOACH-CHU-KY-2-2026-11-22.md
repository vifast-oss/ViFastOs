# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; **D2-236..D2-240** tại `5e635474`; `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **D2-244..D2-245** tại `bfb9a13d`; **D2-247..D2-248** tại `3a8219a`; **D2-250..D2-251** tại `ba0b6b7`; **D2-252** tại `7d37fcee`; **D2-255** tại `14b5de95` sau khi **33/36 task `DONE` & T2 #6 PASS 75/75**).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-255**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md), [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md).

> **Mục tiêu một câu**: Hoàn tất đúng **3 task cuối Chu kỳ 2** (**33 → 36/36 task**) kèm hiệu đính kiến trúc `D2-255`: **`KT-347`** (`exec` từ FS + PoC `U1a` + `u1-uefi-readiness.md`) ∥ **`KT-347b`** (`boot/lib/` + `rustfmt` `v2/user/` + `nightly --soak` + dọn gốc) → **`KT-348`** (`v2.0`, `R2-v2`) để mở **Chu kỳ 3 (20 task `KT-350..KT-366` gồm Pha B+ refactor lõi/HAL/`vifast-kbuild`/tools & Pha C+ tinh gọn kho)**.

---

## 0. Chẩn đoán đầu vào (cập nhật thực thi tại `14b5de95`, 2026-11-24 07:38 JST)

Toàn bộ Đợt 0, Đợt 1, Đợt 2, Đợt 3A và **Đợt 3B** đã hoàn tất sạch (**33/36 task `DONE`**, **T2 #6 PASS 75/75** trên `e1875d53` `qemu_foreign=0`, `vifast-agentctl lint` **0 lỗi**, 0 worktree rò rỉ):
- **Đợt 3B chốt sạch 2 cột mốc cốt lõi**: **`KT-345b`** chốt **28/28 active (`deferred=0`)** parity ở `smp=1,4` + `netsrv` đa caller `6A` (`direct_pushed_proc=1 direct_pushed=1 fwd=0`, rào chắn R1 giữ nguyên `shell.rs` 1.415 / `core_ipc.rs` 1.498); **`KT-346`** (+ **`D2-253`**) tách trọn **`v2/user/`** (8 tệp, 3.291 LOC) và **đóng P20 cả 3 arch** (`VBREV_ALGO = kt134-v5`, byte-identical sha256 qua 2 thư mục, `repro-check.sh --arch` PASS 3/3); **`KT-347c`** vá race TOCTOU cargo trong `ktest.sh`; **`D2-254`** duyệt 3 dòng `_t0_tool` nối 3 thư viện harness của `KT-347b` vào T0.
- **6 phát hiện kiến trúc mới tại `14b5de95` được chốt trong `D2-255`**: (1) biên trang ELF user `riscv64` chỉ còn **168 B** (`USER_STACK_TOP = 0x4001_0000` / `MAX_INIT_PAGES = 16` ⇒ cấp phép mở rộng lên `0x4010_0000` / `32`); (2) nợ `exec` đọc ELF từ FS (`KT-346` §5.1) chuyển vào `KT-347`; (3) `v2/user/` chưa được `rustfmt --check` (lệch 67 dòng ⇒ Lead nối ở vòng merge T2 #7); (4) 3 `vifast-kbin-*/build.rs` lặp 15 hàm (**1.789 LOC** ⇒ rút **`vifast-kbuild`** ở `KT-353b`); (5) `spec/kernel.toml` ghim cứng **`review/agent0/GOI-ABI-V1_1.md`** (bắt buộc giữ nguyên ở `KT-359b`!); (6) tách đủ `kt58`/`kt59`/`kt60` (`9.229` LOC, `KT-359a`) & hấp thu `KT-345c` TCP + `FrameArena` `free` vào `KT-362`.

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229..246** | **Mở Chu kỳ 2, NTFS guard, driver chung, 1 lõi `vifast-core`, giải nghẽn `.bss`, cấm CI GitHub, ABI v1.1 (`7C-r13`), Audit Q1-A 2 tầng, DNS retry `D2-246`** | Đã hoàn tất 100 % trên 3/3 arch; `spec/` khoá cứng trở lại; giữ mọi ngưỡng cứng. |
| **D2-247..251** | **Khóa 2 lớp/`v2/bin/lib/`/`wt.sh`/`v2 t0`/`agentctl lint`; `KT-333e/f`/`KT-342a` (D2-249); ngân sách `product-shell` 3 arch + `KT-339a`/`KT-342b` (D2-250); Chu kỳ 3 (D2-251)** | Đã hoàn tất toàn bộ Đợt 3A (31/36 `DONE`, T2 #5 PASS 75/75). |
| **D2-252..254** | **Refactor 3 Pha** (`D2-252`); **duyệt R5 `v2/user/runtime/*` + chỉ Lead sửa `v2/bin/**`** (`D2-253`); **duyệt 3 dòng `_t0_tool` cho `KT-347b`** (`D2-254`) | Đợt 3B hoàn tất 100 % (`KT-345b` + `KT-346` + `KT-347c`, T2 #6 PASS 75/75, **33/36 `DONE`**); `KT-347b` đang thi công. |
| **D2-255** | **Hiệu đính sau Đợt 3B (`14b5de95`) — biên ELF `riscv64`, `exec` từ FS, `rustfmt` `v2/user/`, `vifast-kbuild` & chốt bảo vệ `GOI-ABI-V1_1.md` ở Pha C** | Áp ngay cho `KT-347`, vòng merge `T2 #7` (`KT-347b`, không phá worktree) và Chu kỳ 3 (`KT-353a/b`, `KT-354`, `KT-359a/b`, `KT-362`). |

---

## 2. Lộ trình 3 task cuối & đồ thị phụ thuộc (`KT-330..KT-348`, 36 task ≤ 3 h)

```text
[33/36 DONE tại 6e00031d — T2 #6 PASS 75/75 | agentctl lint 0 lỗi | 28/28 parity | v2/user/ + P20 3 arch]
  DONE (33): KT-330..KT-333e(+f), KT-334..KT-337, KT-338a..c, KT-339, KT-339a, KT-340a/b/d/c,
             KT-341, KT-342(+a), KT-342b, KT-343, KT-344, KT-345a (spec/ khoá),
             KT-345b (28/28 parity + netsrv 6A), KT-346 (v2/user/ + P20 kt134-v5) [+ KT-347c]
        │
        ▼ (ĐỢT 3C — 2 SLOT READY NGAY, 0 CHỒNG CHÉO TỆP)
  ├── [Slot 1] KT-347  [READY] (exec đọc ELF thật từ FS + PoC U1a + u1-uefi-readiness.md; mở biên 0x4010_0000 nếu cần)
  └── [Slot 2] KT-347b [READY] (boot/lib/ chung + rustfmt v2/user/ + nightly --soak product-shell/blk + dọn gốc & STATE/BLOCKED)
        │
        ▼ (ĐỢT 3D — CỔNG NGHIỆM THU CHU KỲ 2, MÁY ĐO ĐỘC QUYỀN)
  └── (bao đóng 35/35 task, 0 mồ côi) ──► KT-348 (T2 75/75 + nightly --all + soak 3 arch + R2-v2 ⇒ v2.0)
                                             │
                                             ▼ (sau APPROVED A0 chốt tag v2.0)
                                      [CHU KỲ 3 — v2/docs/ke-hoach-chu-ky-3.md (20 task: KT-350..KT-366, Pha B+ & C+)]
```

---

## 3. Chi tiết hiện trạng & 3 task cuối tới `v2.0`

### Đợt 0, Đợt 1, Đợt 2, Đợt 3A & Đợt 3B — ĐÃ HOÀN TẤT 100 % (`33/36 DONE`)

| Nhóm | Task đã `DONE` | Kết quả đo được |
|---|---|---|
| **Đợt 0 & 1** | `KT-330..KT-333e(+f)`, `KT-334..KT-337`, `KT-344`, `KT-345a` | T2 75 bước; 40 GiB đĩa trống; driver chung −42..61 %; `D2-246` 3 arch; ABI v1.1 9/9 (`spec/` khoá). |
| **Đợt 2 & 3A** | `KT-338a/c/b`, `KT-339`, `KT-339a`, `KT-340a/b/d/c`, `KT-341`, `KT-342(+a)`, `KT-342b`, `KT-343` | Cả 3 arch chạy `product-shell` 0 marker (`rv 224.472 B`, `a64 696 B + .corespace`, `x64 51.176 B + .corespace`); đóng **N5** & Audit Q1-A 3/3; sửa gốc `NBSend` (`direct_pushed=1`); T2 #5 PASS 75/75. |
| **Đợt 3B** | **`KT-345b`**, **`KT-346`** *(+`KT-347c`, `D2-253`)* | **Parity 28/28 active (`deferred=0`, `smp=1,4`)** + `netsrv` đa caller `6A`; **`v2/user/`** (8 tệp, 3.291 LOC) + **P20 byte-identical 3 arch (`kt134-v5`)**; R1 giữ nguyên (`shell.rs` 1.415, `core_ipc.rs` 1.498); **T2 #6 PASS 75/75**. |

### Đợt 3C → 3D — 3 task cuối cùng (`KT-347`, `KT-347b`, `KT-348`)

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) + Hiệu đính `D2-252`/`D2-255` |
|---|---|---|---|---|---|---|
| **KT-347** | P2 | **PoC `U1a` (`no_std+alloc` `vifast-rt`) + `exec` đọc ELF từ FS + hợp đồng `.vidl` `ViScript` + `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `READY` | **(D2-255)** (1) Đóng nợ `KT-346` §5.1: seed `/hello.elf` + `/u1a.elf` bằng `vifast-fsutil`, thêm đọc toàn tệp ở `proc_fs`, `proc_exec` đọc ELF thật từ FS thay `include_bytes!`; (2) chạy thật 1 ứng dụng `U1a` (stable `1.100.0`) qua `exec` từ FS (được phép nâng `USER_STACK_TOP: 0x4001_0000 → 0x4010_0000` + `MAX_INIT_PAGES: 16 → 32` và cập nhật 2 ghim `ktest.sh:647,830` nếu vượt 28 KiB); (3) lập `v2/docs/u1-uefi-readiness.md` chốt `U1a`/`U1b`, `.vidl` & kiểm chéo cục bộ `ViFastOs ↔ ViScript` (D2-241), `UEFI`/DTB/ACPI. |
| **KT-347b** | P1 | **Hợp nhất `v2/test/boot/lib/` + `rustfmt` `v2/user/` + `nightly --soak` + dọn gốc tiền `KT-348`** | `KT-333e, KT-339a, KT-341, KT-345b, KT-346` | 2 h | `READY` | **(D2-245/D2-252/D2-255)** Rút `v2/test/boot/lib/{harness-scan,budget-common,soak-common}.sh`; nối `--profile product-shell` (3 arch + `x64-max`) & **soak đĩa thật `virtio-blk`** vào `v2 nightly --soak`; định dạng sạch `v2/user/` & Lead nối `rustfmt --edition 2021 --check` vào `v2/bin/lib/steps_check.sh` (`D2-253`); dọn nợ đã đóng trong `BLOCKED.md`, đồng bộ sạch `STATE.md`, dời `KE-HOACH-CUOI-TUAN-2026-11-{20,21}.md` → `v2/agent/archive/reports/` & `GIAI-THICH-VIFASTOS-CHO-SINH-VIEN.md` → `v2/docs/guides/` (cập nhật `README.md`). |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | bao đóng 35/35 task | 3 h | `BLOCKED` | Máy đo độc quyền (`dirty=0`, `qemu_foreign=0`): T2 75/75 PASS; `v2 nightly --all` (L1 100k × 4 SMP + Miri + `STATUS.md`) PASS; soak `product-shell` 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64` + `600 -cpu max`) đều 0 lỗi; đối chiếu Exit Criteria `v2.0` (§6.2); xuất `R2-v2.{md,json}`; trình Agent 0 chốt `v2.0` & mở Chu kỳ 3. |

---

## 4. Vùng xung đột & quy chế vận hành tối ưu cho Agent 2 (**D2-236..D2-255**)

| Vùng tệp / Nút thắt | Rủi ro đã đo được | Quy tắc bắt buộc |
|---|---|---|
| **Biên ELF user `riscv64` & điểm nóng R1** (`D2-252/D2-255`) | `shell.elf` còn 168 B trước `0x4000_7000`; `core_ipc.rs` (`1.498`), `plat-x86_64/asm.rs` (`1.475`), `machine.rs` (`1.430`), `shell.rs` (`1.415`) | Cấp phép nâng `USER_STACK_TOP → 0x4010_0000` + `MAX_INIT_PAGES → 32` (cập nhật 2 ghim `ktest.sh`) ở `KT-347` hoặc `KT-353a`; xả toàn bộ 5 tệp R1 về `< 950` LOC ngay ở `KT-353a`. |
| **Khóa 2 lớp, T2 sạch & quyền `v2/bin/**`** (`D2-248/D2-253`) | Chạy T2 song song gây T-INFRA; sub-agent sửa `v2/bin/**` | Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; **chỉ Lead chạy T2 (`v2 gate --fast`)** khi `qemu_foreign=0` và **chỉ Lead sửa `v2/bin/**`**. |
| **Hợp đồng `product-shell` & `.corespace`** (`D2-250/D2-252`) | Đặt `Machine` về `.bss` vẫn lọt trần 256 KiB | Giữ chặt `*-product < 65.536 B`, `*-product-shell ≤ 262.144 B` + `.corespace ≥ 184.792 B` (a64/x64) + **0 marker** D2-08; thống nhất `riscv64` + NX ở `KT-353a`. |
| **Sổ sách & 4 slot worktree** (`D2-248/D2-255`) | `STATE.md` chậm HEAD (`W_STATE_HEAD_DRIFT`), `BLOCKED.md` cũ | Dùng `v2/tools/wt.sh` (`df ≥ 5 GiB`), giữ `vifast-agentctl lint` **0 lỗi**, cập nhật sạch `STATE.md`/`BLOCKED.md`, mỗi task 1 tệp `JOURNAL/kt<n>-<ten>.md`, chỉ `kill` PID slot mình (§9). |
| `spec/`, `GOI-ABI-V1_1.md`, 19 `[[legacy]]`, `.github/` | Vỡ đóng băng ABI; vỡ `check_kernel.py`; vỡ SHA legacy; vi phạm cấm CI | Cấm tuyệt đối sửa `spec/abi.toml`/`spec/kernel.toml`, 19 tệp `[[legacy]]`, `.github/workflows/**` (`D2-241`), và **cấm gỡ `review/agent0/GOI-ABI-V1_1.md`** ở Pha C (`KT-359b`). |

---

## 5. Lịch chạy 2 đợt cuối khép Chu kỳ 2 (`v2.0`)

| Đợt | Slot 1 | Slot 2 | Cổng Lead (T2 / Soak) | Kết quả |
|---|---|---|---|---|
| **Đợt 3C** *(~2–3 h, `READY` ngay)* | **`KT-347`** *(`exec` từ FS + PoC `U1a` + `u1-uefi-readiness.md` + nâng `0x4010_0000` nếu cần)* | **`KT-347b`** *(`v2/test/boot/lib/` + `rustfmt` `v2/user/` + `nightly --soak` + dọn gốc & `STATE`/`BLOCKED`)* | Merge lô 2 task → **T2 #7** (`qemu_foreign=0`, ~10 p) → `sync-queue` + push | **35/36 `DONE`** |
| **Đợt 3D (`KT-348`)** *(~2,5 h độc quyền)* | **MÁY ĐO ĐỘC QUYỀN (`dirty=0`, `qemu_foreign=0`)**: T2 75/75 + `v2 nightly --all` | Soak `product-shell` 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600` ×2 CPU) | Xuất **`R2-v2.{md,json}`** qua `check_report.py --all` + `verify-numbers.py`, push | **36/36 `DONE`** → Trình A0 chốt **`v2.0`** & mở **Chu kỳ 3**! |

---

## 6. Lộ trình dài hạn Chu kỳ 3 (`v2.1`, 20 task `KT-350..KT-366`) & Tiêu chí phát hành `v2.0` (**D2-240/D2-250..D2-255**)

### 6.1 Bản thiết kế thi công Chu kỳ 3 (`v2.1` — gồm Pha B+ & Pha C+ `D2-252`/`D2-255`)
Toàn bộ 7 trụ cột chiến lược cùng kế hoạch đại tu kiến trúc lõi/HAL/`vifast-kbuild`/công cụ (**Pha B+**: `KT-353a/b/c`, `KT-354`, `KT-359a`) và tinh gọn cấu trúc kho (**Pha C+**: `KT-359b` — archive rác `JOURNAL/` + `docs/bundles/` + 29 tệp lịch sử `review/` **nhưng giữ cứng `review/agent0/GOI-ABI-V1_1.md`**, gỡ cây mã v1 đóng băng khỏi `main`, giữ nguyên `v1-archive` & `docs/v1/` đọc-only) đã được chốt thành **20 task ≤ 3 h (`KT-350..KT-366`)** tại **[`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md)**. Chu kỳ 3 mở khoá vào `QUEUE.md` ngay sau khi `KT-348` ĐẠT và Agent 0 duyệt `R2-v2` / gắn tag `v2.0`.

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 (`KT-350..KT-366`) |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 ảnh `product-shell` (0 marker, D2-08)** | **Có — ĐÃ ĐẠT** (`rv`, `KT-339a` `a64`, `KT-341` `x64`) | `SMP-1` preemptive 4 hart (`KT-350`/`KT-351`) + hợp nhất HAL/`vifast-plat-common`/`vifast-kbuild` & gom 14 bin `kbin-riscv64` (`KT-353a/b/c`) |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active` — ĐÃ ĐẠT** (`KT-345b`, `smp=1,4`, `deferred=0`) | `KT-345c` (`socket.vidl` TCP đầy đủ) + `ping`/`ifconfig` + `free` DRAM + `ViShell`/`ViScript` (`KT-362`/`KT-363`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có — ĐÃ ĐẠT** (**N5** đóng `KT-340d`; Audit Q1-A/N7 tầng 1 đủ 3/3 port) | Tầng 2 `vifast_sync::Mailbox<N>` + `OVERFLOW` bit + range shootdown (`KT-352`) |
| **Ngân sách & Cổng** (`*-product < 64 KiB`, `*-product-shell ≤ 256 KiB` + `.corespace ≥ 184.792 B`, `Machine ≤ 196.608 B`, T2 75/75, `nightly --all`) | **Có** (`budget*.sh`, `KT-333b..f`, `KT-347b`, `KT-348`) | `.corespace` NOLOAD+NX 3 arch (`KT-353a`) + `v2 mutants` thay 43 `mut-*` + `selfbreak` + trace 32 B + `vifast-bench` (`KT-354`) |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`, `dirty=0`) | **Ảnh `product-shell`** 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) (`KT-348`) | Soak 4 hart U-mode + `VSH2` tải nặng (`KT-366`) |
| **Tái lập byte (P20) & Vệ sinh kho** | **ĐÃ ĐẠT P20 cả 3 arch** (`KT-346`, `kt134-v5`, `v2/user/`); `boot/lib/` + `rustfmt` `v2/user/` (`KT-347b`) | Pha B+/C+: tách `check-wake-paths.py` + `kt58`/`kt59`/`kt60` + `v2/tools/` (`KT-359a`); archive `JOURNAL/`+`docs/bundles/` & gỡ cây mã v1 khỏi `main` (`KT-359b`) |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md`** (`KT-333b`), clone sạch Win/macOS/Linux (`KT-331`) — **ĐÃ ĐẠT** | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **`vifast-objmem`, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | `exec` từ FS + PoC `U1a` + tài liệu định hướng (`KT-347`) | Thi công ở **Chu kỳ 3 (`KT-355..KT-366`)** theo `v2/docs/ke-hoach-chu-ky-3.md` |


