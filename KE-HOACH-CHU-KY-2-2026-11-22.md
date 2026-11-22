# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; **D2-236..D2-240** tại `5e635474`; `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **D2-244..D2-245** tại `bfb9a13d`; **D2-247..D2-248** tại `3a8219a`; **nước rút cuối Chu kỳ 2 & thiết kế Chu kỳ 3 `D2-250..D2-251`** tại `c04aabd` sau khi **25/36 task `DONE` + 2 task (`KT-340c`, `KT-342`) đã gộp mã/T1 chờ T2 #3**).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-251**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md), [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md).

> **Mục tiêu một câu**: Hoàn tất nốt **9 task cuối Chu kỳ 2** (**27 → 36/36 task**): chốt T2 #3 (`KT-340c` + `KT-342`), chạy 4 slot song song đưa **cả 3/3 kiến trúc lên ảnh sản phẩm `product-shell` 0 marker (`KT-339a`, `KT-341`) + đóng nợ `KT-342b` + 25→28/28 parity (`KT-343`, `KT-345b`)**, tách userland `v2/user/` đóng P20 (`KT-346`), PoC `U1a` (`KT-347`), hợp nhất thư viện boot (`KT-347b`), và chốt cổng nghiệm thu **`KT-348` (`v2.0`)** để mở **Chu kỳ 3 (`KT-350..KT-366`)**.

---

## 0. Chẩn đoán đầu vào (cập nhật thực thi tại `c04aabd`, 2026-11-23 05:50 JST)

Phiên đêm `00:00 – 05:38 JST` đạt bước nhảy xuất sắc (**+28 commit, 9 task hoàn thành trong ~5,5 h — sớm hơn ~3 h so với kế hoạch D2-248**):
- **Đã `DONE` hoàn chỉnh (25/36)**: toàn bộ Đợt 0 (`KT-330..KT-333e`, `KT-344`), toàn bộ Đợt 1 (`KT-334..KT-337`, `KT-345a`), và phần lớn Đợt 2 (`KT-338a/b/c`, `KT-339`, `KT-340a/b/d`). Repo đã cắt từ **10.373 → 2.827 tệp tracked (−72,8 %)**, tag `cycle1-raw-logs` đã push, `v2/bin/v2` **2.185 → 161 dòng** (14 mảnh), T2 **75 bước** (−38,8 s CPU `wake`), đĩa trống **40 GiB**, đóng lỗi v1 **N5** + **3/3 port Audit Q1-A tầng 1**.
- **Đã gộp mã + T1 PASS, chờ Lead chạy T2 #3 (2/36)**: **`KT-340c`** (`x86_64` `Machine` + `7C-r13` + gỡ 100 % syscall viết tay, TAP 43/43) và **`KT-342`** (`riscv64` gỡ `ktest` khỏi 6/6 driver + 1 boot chạy đồng thời FS + `netsrv` + `exec`, 0 marker).
- **9 task cuối cần hoàn tất để chốt `v2.0` (`KT-339a`, `KT-341`, `KT-342b`, `KT-343`, `KT-345b`, `KT-346`, `KT-347`, `KT-347b`, `KT-348`)**:
  1. **Ngân sách `product-shell` 3 arch (`D2-249`)**: giữ trần chặt `< 65.536 B` cho ảnh tối giản `*-product`, áp trần hiến định `262.144 B` cho ảnh `*-product-shell` (0 marker) ⇒ mở **`KT-339a`** (`aarch64 --features product-shell`) song song **`KT-341`** (`x86_64 --features product-shell`).
  2. **Đóng 2 nợ của `KT-342` (`KT-342b`)**: cắt `plat-riscv64/user.rs` (`1.499 → < 1.100`) và điều tra/sửa lỗi IPC `nb_send` mất message (`kt342-riscv.md` §9.4) trước khi `KT-345b` nâng `netsrv` đa caller.
  3. **Parity 28/28 + `v2/user/` P20 + `U1a` + Thư viện boot + Nghiệm thu `KT-348`**: `KT-343` (25/28) → `KT-345b` (28/28) ∥ `KT-346` (P20 3 arch) → `KT-347` ∥ `KT-347b` → **`KT-348` (`R2-v2`, `v2.0`)**.

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229..235** | **Mở Chu kỳ 2 (`KT-330..KT-348`), NTFS guard, driver chung, 1 lõi `vifast-core`, cửa sổ ABI v1.1** | Giữ nguyên mọi ngưỡng cứng (`.data+.bss ≤ 256 KiB`, `Machine ≤ 196.608 B`, `R1 ≤ 1.500`, `D2-07`, `D2-08`, `D2-17`); 4 worktree song song; chưa mở `v2.0`. |
| **D2-236..240** | **Giải nghẽn `.bss` 3 arch, đẩy `KT-345a` trước `KT-338b`/`KT-340c`, tách task ≤ 3 h, Khung Chu kỳ 3 & Exit Criteria `v2.0`** | Rút `MmioBus`/`DmaPool` (`sync_for_device/cpu`) vào `vifast-virtio`; `budget-x86_64.sh`; đóng N5 + Audit Q1-A; 7 trụ cột Chu kỳ 3 & bảng Exit Criteria (§6). |
| **D2-241** | **BỎ GitHub Actions, vĩnh viễn** *(chỉ đạo người điều phối)* | Xoá `.github/`; ghi luật cấm vào `AGENTS.md` §4.11 & `v2/AGENTS.md` §4. Mọi cổng chạy trên máy đo. |
| **D2-242** | **Duyệt Gói ABI v1.1 (`KT-344` `APPROVED`) & mở khoá `KT-345a`** | Chấp thuận **1C..9B + `7C-r13`**, `abi_version = 1`, `VspaceOutcome` rollback *(đã `DONE` 9/9 — `spec/` khoá lại)*. |
| **D2-243..245** | **Tinh chỉnh Đợt 0–2, Audit Q1-A 2 tầng (`IpiMailbox` + `IrqMcsLock`) & Quy chế Agent 2** | Đã hoàn tất 100 % trên cả 3/3 port; `v2 mutants` & `Mailbox<N>` tầng 2 thuộc Chu kỳ 3. |
| **D2-246** | **DNS client: reply hỏng ⇒ đếm `bad_replies` & retry trong ngân sách** *(Agent 1)* | Đã vá ở `KT-335c` và lan ra đủ **3/3 arch** qua `KT-335b`. |
| **D2-247..248** | **Tách sớm `KT-333e`, khóa 2 lớp T0/T1/T2, `wt.sh`, `v2 t0`, `env.sh`, `agentctl lint` & lịch chạy đêm** | Đã hoàn tất 100 % (`KT-333d`, `KT-333e`, `KT-335b`, `KT-338b`, `KT-339`, `KT-340b/d/c`, `KT-342`). |
| **D2-249** | **`KT-342a`: sản phẩm boot được không cần NIC (`NoDevice`) & `guard_selftest.sh` `LOGDIR` ra `/tmp`** *(Agent 1)* | Đã chốt tại `c04aabd` (cùng `KT-333f` `b476135` dọn 36 `??` `kt78-guard` vào `cycle1-raw-logs-untracked`). |
| **D2-250** | **Nước rút cuối Chu kỳ 2: ngân sách `product-shell` (`≤ 256 KiB` vs bare `< 64 KiB`), mở `KT-339a` & `KT-342b`, triage `KT-340e` & xác nhận `KT-333f`/`KT-342a`** | Đưa **cả 3/3 arch** lên `product-shell` 0 marker; cắt `user.rs` + sửa lỗi IPC `nb_send`; chuyển `KT-340e` sang `KT-351`/`SMP-1b`; `KT-333f`/`KT-342a` đã xong tại `b476135`+`c04aabd`. |
| **D2-251** | **Ban hành Bản thiết kế thi công Chu kỳ 3 (`v2.1` — `KT-350..KT-366`)** | 17 task ≤ 3 h chia 4 đợt tại [`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md); mở khoá sau khi `KT-348` ĐẠT & chốt `v2.0`. |

---

## 2. Lộ trình nước rút cuối & đồ thị phụ thuộc (`KT-330..KT-348`, 36 task ≤ 3 h)

```text
[25/36 DONE + 2/36 gộp mã/T1 chờ T2 #3 tại b2f6e06 — 05:50 JST]
  DONE (25): KT-330, KT-331, KT-332, KT-333a..e, KT-334, KT-335, KT-335b, KT-335c,
             KT-336a..c, KT-337, KT-338a..c, KT-339, KT-340a/b/d, KT-344, KT-345a (spec/ khoá)
  CHỜ T2 #3 (2): KT-340c [READY, T1 PASS], KT-342 [READY, T1 PASS]  ──► Lead chạy T2 #3 (~8,5 p) ⇒ 27/36 DONE
        │
        ▼ (ĐỢT 3A — 4 SLOT SONG SONG FULL, 0 CHỒNG CHÉO TỆP)
  ├── [Slot 1] KT-342b (cắt user.rs < 1.100 + sửa lỗi IPC nb_send mất msg) ─────────┐
  ├── [Slot 2] KT-343  (11 lệnh deferred-K7 → 25/28 parity, smp=1,4) ───────────────┤
  ├── [Slot 3] KT-339a [READY NGAY] (aarch64 --features product-shell, 0 marker) ──┼──► Đợt 3B:
  └── [Slot 4 — ĐƯỜNG GĂNG] KT-341  (x86_64  --features product-shell, 0 marker) ──┘    ├── KT-345b (28/28 + netsrv 6A)
                                                                                         └── KT-346  (v2/user/ + P20 3 arch)
        │
        ▼ (ĐỢT 3C — 2 SLOT SONG SONG TIỀN NGHIỆM THU)
  ├── (KT-341, KT-346)                           ──► KT-347  (PoC U1a + .vidl ViScript + UEFI/DTB/ACPI)
  └── (KT-333e, KT-339a, KT-341, KT-345b, KT-346)──► KT-347b (v2/test/boot/lib/ + nightly --soak product-shell/blk)
        │
        ▼ (ĐỢT 3D — CỔNG NGHIỆM THU CHU KỲ 2, MÁY ĐO ĐỘC QUYỀN)
  └── (bao đóng 35/35 task, 0 mồ côi)            ──► KT-348  (T2 75/75 + nightly --all + soak 3 arch + R2-v2 ⇒ v2.0)
                                                        │
                                                        ▼ (sau APPROVED A0 chốt v2.0)
                                                 [CHU KỲ 3 — v2/docs/ke-hoach-chu-ky-3.md (KT-350..KT-366)]
```

---

## 3. Chi tiết từng đợt & bảng task (mọi task ≤ 3 h — tổng 36 task Chu kỳ 2)

### Đợt 0 & Đợt 1 — ĐÃ HOÀN TẤT 100 % (`18/18 DONE`)

| Mã | Task | Trạng thái | Kết quả đo được |
|---|---|---|---|
| **KT-330..KT-332** | Soak vòng 25 + NTFS guard (`check-filenames.py`) + đồng bộ tài liệu/`STATE.md`/`BLOCKED.md` | `DONE` | 0/1000 & 0/600 ×2; 23/23 NTFS; `STATE` 335,8→3,6 KiB. |
| **KT-333a..KT-333e** | T2 lanes + `v2 nightly`/`deny.toml` + `measure-lock` + tách `v2/bin/v2` (2.185→161) + khóa 2 lớp/`wt.sh`/`v2 t0`/`agentctl lint` + 1-pass `wake` (−38,8 s) + tag `cycle1-raw-logs` (repo 10.373→2.827) | `DONE` | T2 75 bước; `check-wake` 461/461; 40 GiB đĩa trống. |
| **KT-334..KT-337** | `vifast-virtio` (pci/blk/net/gpu/input) + `vifast-net` (TCP/DHCP/ARP/ICMP/UDP/DNS + `D2-246` 3 arch) + `IrqMcsLock` x64/rv + tách `kobj.rs` (13 module) + `virtio-gpu` x64 PPM 23/23 | `DONE` | Driver −42..61 %; 190 net host test; L1-full 400k PASS. |
| **KT-344, KT-345a** | RFC ABI v1.1 (`APPROVED` D2-242) + áp `spec/` 9/9 nhóm (`kernel.toml`, `7C-r13`, `VspaceOutcome`, 3 `.vidl`) | `DONE` | `spec/` khoá cứng trở lại; `Machine` 184.792 B (Δ=0). |

### Đợt 2 & Đợt 3 — Hiện trạng & 9 task cuối tới `v2.0`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) / Kết quả |
|---|---|---|---|---|---|---|
| **KT-338a/c/b, KT-339** | P1 | `aarch64`: `.bss` 0 B sản phẩm + `main.rs` 549 + `IpiMailbox` + `Machine`/EL0 `svc #0` + `vifast-plat-user` `procsrv`/`shell` (TAP 44/44) | — | — | `DONE` | `run-aarch64.sh` 34/34 (32 ca âm); còn đưa sang ảnh `product-shell` ở `KT-339a`. |
| **KT-339a** | P1 | **Ảnh sản phẩm `aarch64 --features product-shell`: `Machine` + `procsrv`/`shell` EL0 (0 marker, ≤ 256 KiB)** | `KT-339` | 3 h | `READY` | **(D2-250)** `budget-aarch64.sh` giữ `aarch64-product < 65.536 B` & `aarch64-product-shell ≤ 262.144 B`; 0 harness marker; chạy `help`/`echo`/`ps`/`exit` qua PL011 + ca âm + 2 đột biến đỏ. |
| **KT-340a/b/d** | P1 | `x86_64`: W^X + `budget-x86_64.sh` + `.bss` sản phẩm `43.072 B < 64 KiB` + `main.rs` 1.004 + `VSpace` động + TLB shootdown `IpiMailbox` (`ipi.rs`) + đóng **N5** | — | — | `DONE` | TAP 37/37, 32/32 ca âm; hoàn tất 3/3 port Audit Q1-A tầng 1. |
| **KT-340c** | P1 | **`x86_64`: nối `vifast-core` & IPC ring 3 (`7C-r13`, gỡ syscall tự viết)** | `KT-340d, KT-345a` | 3 h | `READY` *(chờ T2 #3)* | Mã + T1 PASS tại `b5ad0a0` (TAP 43/43, ktest 251.024 B, 2 đột biến đỏ); Lead chạy T2 #3 ⇒ `DONE` (`KT-340e` chuyển Chu kỳ 3 `KT-351`). |
| **KT-341** | P1 | **`x86_64` K3.5 (ĐƯỜNG GĂNG): ảnh sản phẩm `x86_64 --features product-shell` (`Machine` + `procsrv`/`shell` chung)** | `KT-339, KT-340c` | 3 h | `BLOCKED` | **(D2-250)** `budget-x86_64.sh` giữ `x86_64-product < 65.536 B` & `x86_64-product-shell ≤ 262.144 B`; shim `7C-r13` trong `vifast-plat-user`; 0 marker; chạy `help`/`echo`/`ps`/`exit` qua COM1 + ca âm + 2 đột biến đỏ. |
| **KT-342** | P1 | **Hợp nhất ảnh sản phẩm `riscv64`: gỡ `ktest` khỏi driver + FS, `netsrv`, `exec`** | `KT-335, KT-335b, KT-336a, KT-336c` | 3 h | `READY` *(chờ T2 #3)* | Mã + T1 PASS tại `55d35b4` (`product-riscv64.sh` 1 boot chạy FS+`netsrv`+`exec`, 0 marker, 224.472 B); Lead chạy T2 #3 ⇒ `DONE`. |
| **KT-342b** | P1 | **Đóng nợ `KT-342`: cắt `plat-riscv64/user.rs` `< 1.100` + sửa lỗi IPC `nb_send` mất message (`netsrv`→`shell`)** | `KT-342` | 2 h | `BLOCKED` | **(D2-250)** Tách `user.rs` (`1.499 → < 1.100`); test đỏ-trước tái hiện & sửa gốc lỗi `kt342-riscv.md` §9.4 để `netsrv` trả lời `shell` trực tiếp qua IPC U-mode; `Machine` 184.792 B không đổi; matrix 27/27 + `product-riscv64.sh` PASS. |
| **KT-343** | P1 | **Đóng 11 lệnh `deferred-K7` (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`, `run`, `runfs`, `exec`)** | `KT-342` | 3 h | `BLOCKED` | Lật 11 lệnh → `active` trong `spec/v1-parity.toml` (**25/28 active**); parity `--mode serial` 25/25 ở `smp=1,4`; ca âm từng nhóm. |
| **KT-345b** | P1 | **Thi hành ABI v1.1: `free`/`df`/`loop` (28/28 parity) + `NetSvc` đa caller** | `KT-342, KT-342b, KT-343, KT-345a` | 3 h | `BLOCKED` | `netsrv` U-mode đa caller theo `socket.vidl` (`6A`); `OP_STATFS` + `MemSvc` (`4C`) đóng `free`/`df`/`loop` (**28/28 active**); parity `--mode serial` 28/28 ở `smp=1,4`. |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184 (3 arch)** | `KT-339a, KT-341, KT-342` | 3 h | `BLOCKED` | Build ELF user trong `v2/user/` + `--remap-path-prefix`; thêm `v2/user` vào `VBREV_INPUT_ROOTS` + tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + `wake_scope.py`; cùng sha256 trên 2 thư mục build cả 3 arch (Linux/macOS). |
| **KT-347** | P2 | **Thiết kế & PoC `U1a` (`no_std+alloc` `vifast-rt`) + hợp đồng `.vidl` `ViScript` + hướng `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `BLOCKED` | 1 ứng dụng user `U1a` (stable `1.100.0`) chạy thật qua `exec` từ FS; `v2/docs/u1-uefi-readiness.md` chốt `U1a`/`U1b`, hợp đồng `.vidl` & script kiểm chéo trên máy đo `ViFastOs ↔ ViScript` (không GitHub Actions theo D2-241), và `UEFI`/DTB/ACPI. |
| **KT-347b** | P1 | **Hợp nhất `v2/test/boot/lib/` (`harness-scan`/`budget`/`soak`) + nối `nightly --soak --profile product-shell` & `virtio-blk`** | `KT-333e, KT-339a, KT-341, KT-345b, KT-346` | 2 h | `BLOCKED` | **(D2-245/D2-247/D2-250)** Rút `v2/test/boot/lib/{harness-scan,budget-common,soak-common}.sh`; nối `--profile product-shell` (3 arch + `x64-max`) & đợt **soak đĩa thật `virtio-blk`** vào `v2 nightly --soak`; dọn `BLOCKED.md`; T2 PASS. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | bao đóng 35/35 task | 3 h | `BLOCKED` | Chạy độc quyền (`dirty=0`, `qemu_foreign=0`): T2 75/75 PASS; `v2 nightly --all` (L1 100k × 4 SMP + Miri + `STATUS.md`) PASS; soak **`product-shell`** 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64` + `600 -cpu max`) đều 0 lỗi; đối chiếu **Exit Criteria `v2.0` (§6.2)**; xuất `R2-v2.{md,json}`; trình Agent 0 chốt `v2.0` & mở Chu kỳ 3. |

---

## 4. Vùng xung đột & quy chế vận hành tối ưu cho Agent 2 (**D2-236..D2-250**)

| Vùng tệp / Nút thắt | Rủi ro đã đo được | Quy tắc bắt buộc |
|---|---|---|
| **Khóa 2 lớp & cổng TCP `45678`** (`D2-248`) | 4 slot tranh cổng `45678`; `qemu_foreign > 0` làm số thời gian T2 vô hiệu | Sub-agent chỉ chạy **T0 (`v2 t0`) + T1 (`v2 gate --dev`)**; **chỉ Lead chạy T2 (`v2 gate --fast`)** lúc `qemu_foreign=0`; `v2_env_wait_port_free` trong `env.sh` tuần tự hoá cổng `45678`. |
| **Ngân sách `.data+.bss` `product-shell`** (`D2-249`) | `*-product < 64 KiB` chặn `Machine` 184,8 KiB vào ảnh `product-shell` | Giữ chặt `*-product < 65.536 B`; tách riêng nhãn **`*-product-shell ≤ 262.144 B`** + kiểm D2-08 **0 marker** trên `aarch64` (`KT-339a`) & `x86_64` (`KT-341`). |
| **Sổ `STATE.md` & `vifast-agentctl lint`** | Dòng `TASK` thiếu `(N task)` ở dòng đầu ⇒ `E_STATE_COUNT` | Lead giữ mẫu `(N task)` ngay trên dòng `TASK :` đầu tiên và chạy `./v2/bin/vifast-agentctl lint` sau mỗi đợt merge. |
| **4 slot worktree & kỷ luật PID (§9)** | `kill` nhầm PID QEMU của slot khác (`KT-335b`) | Tái dùng đúng 4 slot qua **`v2/tools/wt.sh`** (`df ≥ 5 GiB`); chỉ `kill` đúng PID do chính slot mình tạo; mỗi task **1 tệp `JOURNAL/kt<n>-<ten>.md`**. |
| **Tệp nóng R1 (`1.500` dòng)** | `plat-riscv64/user.rs` (1.499), `core_ipc.rs` (1.471), `machine.rs` (1.430) | **`KT-342b`** cắt `user.rs < 1.100` ngay trước `KT-345b`; `core_ipc.rs`/`machine.rs` tách ở Chu kỳ 3 `KT-353`. |
| `spec/`, 19 tệp `[[legacy]]`, `.github/` | Vỡ đóng băng ABI; vỡ SHA legacy; vi phạm cấm CI | Cấm tuyệt đối sửa `spec/abi.toml`/`spec/kernel.toml`, 19 tệp `[[legacy]]` trong `numbers-policy.toml`, và `.github/workflows/**` (`D2-241`). |

---

## 5. Lịch chạy nước rút cuối Chu kỳ 2 (`2026-11-23 05:50 JST` → chốt `v2.0`)

> **Tốc độ thực tế đã đo**: 9 task hoàn thành trong 5,5 h (~1,6–2,0 task/h trên 4 slot); T2 `gate --fast` ~8,5 p trên máy rảnh sau `KT-333e`. Với đúng **2 task chờ T2 #3 + 9 task còn lại**, toàn bộ Chu kỳ 2 có thể hoàn tất trong **~6–8 giờ máy đo** (đạt **31/36 `DONE` trước 09:00 JST**, và khép trọn **36/36 `DONE` + `R2-v2`** trong ngày `2026-11-23`).

| Giai đoạn | Khung giờ (JST) | Slot 1 | Slot 2 (`riscv64`) | Slot 3 (`aarch64`) | Slot 4 (**ĐƯỜNG GĂNG** `x86_64`) | Cổng Lead (T2 / Soak) |
|---|---|---|---|---|---|---|
| **Chốt T2 #3** | `05:45 – 05:55` | *(dừng QEMU lẻ)* | *(dừng QEMU lẻ)* | *(chuẩn bị `KT-339a`)* | *(dừng QEMU lẻ)* | Chạy **T2 #3** (`KT-340c` + `KT-342`, `qemu_foreign=0`), lật cả 2 `DONE`, sửa `STATE.md` (`E_STATE_COUNT`), push ⇒ **27/36 `DONE`** |
| **Đợt 3A**<br>*(4 slot full)* | `05:55 – 07:45` | **`KT-342b`** *(cắt `user.rs` + sửa lỗi `nb_send`)* | **`KT-343`** *(25/28 parity `smp=1,4`)* | **`KT-339a`** *(`aarch64 product-shell` 0 marker)* | **`KT-341`** *(`x86_64 product-shell` 0 marker)* | **07:45 – 08:00**: Merge lô 4 task → chạy **T2 #4** (~8,5 p), push ⇒ **31/36 `DONE`** *(cả 3 arch chạy `product-shell`!)* |
| **Chốt sáng 09:00** *(tuỳ chọn)* | `08:00 – 09:00` | *(Nếu chốt bàn giao 09:00 JST: ngừng mở task mới sau 08:25, dọn worktree, `agentctl lint`, push sạch)* | **Nếu chạy liên tục / tới 09:00 ICT (11:00 JST)**: mở ngay **`KT-345b`** (28/28 parity + `netsrv` `6A`) | *(Hỗ trợ chuẩn bị `KT-347b`)* | **Mở ngay `KT-346`** (`v2/user/` + `VBREV_ALGO kt134-v5` + P20 3 arch) | **~10:15**: Merge `KT-345b` + `KT-346` → **T2 #5**, push ⇒ **33/36 `DONE`** |
| **Đợt 3C** | `~10:30 – 12:30` | **`KT-347b`** *(`boot/lib/` chung + `nightly --soak --profile product-shell` & `virtio-blk` + dọn `BLOCKED.md`/`.log`)* | *(Nghỉ / rà `R2-v2`)* | *(Nghỉ)* | **`KT-347`** *(PoC `U1a` + `u1-uefi-readiness.md` + `.vidl` `ViScript`)* | **~12:30**: Merge `KT-347` + `KT-347b` → **T2 #6**, push ⇒ **35/36 `DONE`** |
| **Đợt 3D (`KT-348`)** | `~13:00 – 15:30` | **MÁY ĐO ĐỘC QUYỀN (`dirty=0`, `qemu_foreign=0`)** | T2 75/75 PASS | `v2 nightly --all` (L1 400k + Miri + `STATUS.md`) | Soak `product-shell` 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600` ×2 CPU) | Xuất **`R2-v2.{md,json}`**, push ⇒ **36/36 `DONE`** → trình Agent 0 chốt **`v2.0`** & mở **Chu kỳ 3**! |

---

## 6. Lộ trình dài hạn Chu kỳ 3 (`v2.1`, `KT-350..KT-366`) & Tiêu chí phát hành `v2.0` (**D2-240/D2-250/D2-251**)

### 6.1 Bản thiết kế thi công Chu kỳ 3 (`v2.1`)
Toàn bộ 7 trụ cột chiến lược đã được cụ thể hoá thành **17 task ≤ 3 h (`KT-350..KT-366`, 4 đợt)** kèm đồ thị phụ thuộc, ngân sách, cổng duyệt ABI v1.2 (`KT-358`) và kế hoạch dọn cây v1 tại tài liệu riêng: **[`v2/docs/ke-hoach-chu-ky-3.md`](v2/docs/ke-hoach-chu-ky-3.md)**. Chu kỳ 3 chỉ mở khoá vào `QUEUE.md` sau khi `KT-348` ĐẠT và Agent 0 duyệt `R2-v2` / gắn tag `v2.0`.

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 (`KT-350..KT-366`) |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 ảnh `product-shell` (0 marker, D2-08)** | **Có** (`KT-338a..KT-342b`: `rv` + `KT-339a` `a64` + `KT-341` `x64`) | `SMP-1` preemptive 4 hart (`KT-350`/`KT-351`, hấp thu `KT-340e`) + gom 14 bin `kbin-riscv64` (`KT-353`) |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active`** (`KT-343` + `KT-345b`, `smp=1` & `smp=4`) | Lệnh `ping`/`ifconfig` + `ViShell`/`ViScript` (`KT-362`/`KT-363`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có — ĐÃ ĐẠT** (**N5** đóng ở `KT-340d`; **Audit Q1-A/N7 tầng 1**: `IpiMailbox` + `IrqMcsLock` đủ 3/3 port) | Tầng 2 `vifast_sync::Mailbox<N>` có `OVERFLOW` bit + range shootdown (`KT-352`) |
| **Ngân sách & Cổng** (`*-product < 64 KiB`, `*-product-shell ≤ 256 KiB`, `Machine ≤ 196.608 B`, T2 75/75, `nightly --all`) | **Có** (`budget*.sh`, `KT-333b..e`, `KT-347b`, `KT-348`) | `v2 mutants` thật (thay 28 feature x64) + trace 32 B + `vifast-bench` (`KT-354`) |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`, `dirty=0`) | **Ảnh `product-shell`** 3 arch (`rv 1000` + `virtio-blk`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) (`KT-348`) | Soak 4 hart U-mode + `VSH2` tải nặng (`KT-366`) |
| **Tái lập byte (P20) & Vệ sinh kho** | **Cùng sha256 canonical cả 3 arch trên Linux + macOS** (`KT-346`); kho gọn **2.827 tệp** (`KT-333e`, tag `cycle1-raw-logs`) + `boot/lib/` (`KT-347b`) | Hội tụ Windows (B2) + `.scn` 3 arch + đánh giá gỡ cây v1 (`KT-359`) |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md`** (`KT-333b`), clone sạch Win/macOS/Linux (`KT-331`) — **ĐÃ ĐẠT** | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **`vifast-objmem`, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | PoC `U1a` + tài liệu định hướng (`KT-347`) | Thi công ở **Chu kỳ 3 (`KT-355..KT-366`)** theo `v2/docs/ke-hoach-chu-ky-3.md` |

