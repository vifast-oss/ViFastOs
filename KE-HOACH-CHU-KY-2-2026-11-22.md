# KẾ HOẠCH THI CÔNG CHU KỲ 2 & KHUNG DÀI HẠN — Hợp nhất lõi, khử nhân bản & chuyển sang sản phẩm (`KT-330..KT-348`)

**Người ban hành**: **Agent 0** (Kiến trúc sư trưởng).
**Ngày ban hành**: `2026-11-22` (mở tại `1559339e`; bổ sung **D2-236..D2-240** tại `5e635474`; duyệt `KT-344` & **D2-241..D2-243** tại `2bf130bf`; **D2-244..D2-245** tại `bfb9a13d`; **hiệu đính nửa sau Chu kỳ 2 & kịch bản chạy đêm tới 09:00 `D2-247..D2-248`** tại `7b60723` sau khi **18/34 task `DONE`** — `KT-335c` `DONE`, T2 76/76 PASS, L1-full 400k PASS).
**Hiệu lực**: Ngay sau khi các agent `git pull origin main`. Thay thế mọi kế hoạch vòng cũ.
**Nguồn sự thật**: [`AGENTS.md`](AGENTS.md), [`v2/AGENTS.md`](v2/AGENTS.md), [`docs/QUYET-DINH-sau-review.md`](docs/QUYET-DINH-sau-review.md), [`spec/abi.toml`](spec/abi.toml), [`v2/agent/DECISIONS.md`](v2/agent/DECISIONS.md) (**D2-229..D2-248**), [`v2/agent/QUEUE.md`](v2/agent/QUEUE.md).

> **Mục tiêu một câu**: Hoàn tất nốt nửa sau Chu kỳ 2 (**18 → 34/34 task**): nâng cấp quy trình & dọn kho sớm (`KT-333d`/`KT-333e`), gộp chính sách mạng giữ `D2-246` (`KT-335b`), đưa **cả 3 kiến trúc (`riscv64`/`aarch64`/`x86_64`) lên chung 1 lõi `vifast-core` + 1 `shell` + 28/28 parity v1 + đóng 7/7 lỗi v1**, tách userland `v2/user/` đóng P20 (`KT-346`), PoC `U1a` (`KT-347`), hợp nhất thư viện boot (`KT-347b`), và chốt cổng nghiệm thu `KT-348` (`v2.0`).

---

## 0. Chẩn đoán đầu vào (cập nhật thực thi tại `7b60723`, 18/34 `DONE`)

Chu kỳ 1 (`KT-01..KT-329`, 25 vòng) và nửa đầu Chu kỳ 2 (**18/34 task `DONE`** — `KT-335c` vừa chốt `DONE` tại `c5996ac8` với 36/36 kiểm sạch + 14/14 dns host test; T2 **76/76 PASS 575,2 s**, **L1-full 400k PASS**, soak `riscv64` 1000/1000 + `aarch64` 1000/1000 PASS) đã đóng xong phần lớn nợ nền tảng. Còn lại **5 mặt trận chính** cho nửa sau Chu kỳ 2:

1. **Đường găng số 1 — `x86_64` (Slot 4: `KT-340b → KT-340d → KT-340c → KT-341`)**: `.data+.bss` sản phẩm đang `96.320 B > 64 KiB` và `main.rs` chạm trần R1 `1.500/1.500` ⇒ `KT-340b` cắt `main.rs`, hạ `.bss < 64 KiB`, dựng `VSpace` động; `KT-340d` đóng lỗi v1 **N5** + `IpiMailbox` đủ 3/3 port; `KT-340c` link `vifast-core::Machine` + `7C-r13` thay scheduler riêng; `KT-341` chạy shell chung.
2. **`aarch64` nối `vifast-core` + shell chung (Slot 3: `KT-338b → KT-339`)**: `KT-338a`/`KT-338c` đã hạ `.bss` về `0 B` sản phẩm / `26.544 B` ktest, `main.rs` 549 dòng, 0 harness marker, `IpiMailbox` SGI 3/3 đường; `KT-338b` đưa nền MMU/`VSpace`/GIC/EL0 sang sản phẩm + link `Machine` + `svc #0`; `KT-339` chạy `procsrv`/`shell` chung.
3. **Mạng dùng chung & sản phẩm `riscv64` 28/28 parity (Slot 2: `KT-335b → KT-342 → KT-343 → KT-345b`)**: `KT-335c` (P0 DNS `D2-246`) đã `DONE` (chạy lại soak đầy `x86_64` xác nhận hết flake ở `KT-348`); `KT-335b` mở `READY` ngay để gộp chính sách DHCP/ARP/ICMP/UDP/DNS giữ `D2-246`; `KT-342` cắt `user.rs` (`1.499 → < 1.100`) & hợp nhất sản phẩm FS+`netsrv`+`exec`; `KT-343`/`KT-345b` đóng 28/28 parity v1.
4. **Quy trình Agent 2 & dọn kho sớm (Slot 1: `KT-333d → KT-333e`)**: `v2/bin/v2` đã phình lên `2.185` dòng; `do_gate_dev` T1 đang giữ `measure-lock` độc quyền cản 4 slot song song trong khi runner T0 không kiểm khóa gây `T-INFRA`; còn 7 runner `LOGDIR` tĩnh + 3 `SOCK` trong `$TMPDIR`; `KT-333e` kéo sớm 1-pass `wake` (−55–60 s/cổng) + dọn 22 phiếu ở root + tách ~7.602 raw `.log` Chu kỳ 1 để giảm ~73,5 % số tệp repo ngay lập tức.
5. **Đợt 3 tiền `v2.0` (`KT-346 → KT-347 ∥ KT-347b → KT-348`)**: tách `v2/user/` đóng P20 3 arch, PoC `U1a`, rút `v2/test/boot/lib/` chung + `nightly --soak --profile product-shell` & `virtio-blk`, chạy cổng nghiệm thu `KT-348` (`R2-v2`).

---

## 1. Quyết định kiến trúc của Agent 0 (ghi trong `v2/agent/DECISIONS.md`)

| Mã | Quyết định | Phạm vi & ranh giới |
|---|---|---|
| **D2-229..235** | **Mở Chu kỳ 2 (`KT-330..KT-348`), NTFS guard, driver chung, 1 lõi `vifast-core`, cửa sổ ABI v1.1** | Giữ nguyên mọi ngưỡng cứng (`.data+.bss ≤ 256 KiB`, `Machine ≤ 196.608 B`, `R1 ≤ 1.500`, `D2-07`, `D2-08`, `D2-17`); 4 worktree song song; chưa mở `v2.0`. |
| **D2-236..240** | **Giải nghẽn `.bss` 3 arch, đẩy `KT-345a` trước `KT-338b`/`KT-340c`, tách task ≤ 3 h, Khung Chu kỳ 3 & Exit Criteria `v2.0`** | Rút `MmioBus`/`DmaPool` (`sync_for_device/cpu`) vào `vifast-virtio`; `budget-x86_64.sh`; đóng N5 + Audit Q1-A; 7 trụ cột Chu kỳ 3 & bảng Exit Criteria (§6). |
| **D2-241** | **BỎ GitHub Actions, vĩnh viễn** *(chỉ đạo người điều phối)* | Xoá `.github/`; ghi luật cấm vào `AGENTS.md` §4.11 & `v2/AGENTS.md` §4. Mọi cổng chạy trên máy đo. |
| **D2-242** | **Duyệt Gói ABI v1.1 (`KT-344` `APPROVED`) & mở khoá `KT-345a`** | Chấp thuận **1C..9B + `7C-r13`**, `abi_version = 1`, `VspaceOutcome` rollback *(đã `DONE` 9/9 — `spec/` khoá lại)*. |
| **D2-243** | **Tinh chỉnh Đợt 0–1 (`KT-336c`, `KT-333c`, `.bss < 64 KiB` `x86_64`)** | `KT-336c` & `KT-333c` đã `DONE`; ràng buộc `.data+.bss < 64 KiB` `x86_64` trước `KT-340c`. |
| **D2-244** | **Hiệu đính DAG Đợt 2 (`KT-338c`, `KT-340b`/`d`), Audit Q1-A 2 tầng & `v2 mutants`** | Đóng Audit Q1-A/`N7` tầng 1 bằng **`IpiMailbox` + `IrqMcsLock` đủ 3/3 port** (`Mailbox<N>` cho `SMP-1` Chu kỳ 3); `v2 mutants` ở Chu kỳ 3 (`v2.1`). |
| **D2-245** | **Quy chế Agent 2 (T0+T1 sub-agent / Lead T2 gộp lô, cấm "DONE một phần", 4 slot cố định, siết `JOURNAL/`)** | Đã phát huy hiệu lực ngay (`KT-335` tách `KT-335b`, 0 `kt*-logs/*.log` mới). |
| **D2-246** | **DNS client `x86_64`: reply hỏng ⇒ đếm `bad_replies` & retry trong ngân sách** *(Agent 1)* | Vá lỗi 1/600 soak (`KT-335c` `DONE` tại `c5996ac8`, 36/36 + 14/14 dns host test); phải bảo toàn hợp đồng này trong `KT-335b`. |
| **D2-247** | **Hiệu đính DAG trên `7b60723`, tách sớm `KT-333e` & vá bao đóng `KT-348`** | Lật **`KT-333d`/`KT-335b`/`KT-338b` → `READY`** ngay cùng `KT-340b` (4 slot song song); nối `KT-335c → KT-335b → KT-342 → KT-348` & `KT-347 → KT-348`; kéo sớm **`KT-333e`** (1-pass `wake` + 22 root `.md` + ~7.602 `.log`) ngay sau `KT-333d`; chốt ranh giới `KT-338b`/`KT-340d`. |
| **D2-248** | **Khóa 2 lớp T0/T1/T2, `wt.sh` cache ấm, `v2 t0`, `boot/lib/env.sh`, `agentctl lint` & Kịch bản chạy đêm đến 09:00** | Gỡ khóa độc quyền ở T1 `do_gate_dev`, thêm kiểm khóa ở runner T0, phủ 7 runner `LOGDIR` & 3 `SOCK`, thêm `soak-x86_64-max`, và chốt lịch 4 đợt tự hành tới 09:00 (§5). |

---

## 2. Lộ trình 4 đợt & đồ thị phụ thuộc (`KT-330..KT-348`, 34 task ≤ 3 h)

```text
[18/34 DONE tại 7b60723 — T2 76/76 PASS 575,2 s | L1-full 400k PASS | rv/a64 soak 1000/1000 PASS]
  DONE: KT-330, KT-331, KT-332, KT-333a, KT-333b, KT-333c, KT-334, KT-335, KT-335c, KT-336a,
        KT-336b, KT-336c, KT-337, KT-338a, KT-338c, KT-340a, KT-344, KT-345a (spec/ khoá lại)
        │
        ▼ (4 SLOT SONG SONG READY NGAY LẬP TỨC — 0 CHỒNG CHÉO TỆP, 0 CHỜ ĐẦU PHIÊN)
  ├── [Slot 1 — Hạ tầng Agent 2 & Dọn kho sớm (ngoài v2/kernel & v2/lib)]
  │     KT-333d [READY] (tách v2/bin/v2 + khóa 2 lớp + env.sh + wt.sh + v2 t0 + agentctl lint + x64-max)
  │       └──► KT-333e (1-pass wake −60s + gom 22 phiếu root + tách ~7.602 raw .log Chu kỳ 1)
  │
  ├── [Slot 2 — Mạng dùng chung & Sản phẩm riscv64 28/28 parity]
  │     KT-335b [READY] (DHCP/ARP/ICMP/UDP/DNS → vifast-net, giữ D2-246)
  │       └──► KT-342 (cắt user.rs < 1.100 + riscv64 sản phẩm FS+netsrv+exec)
  │              └──► KT-343 (25/28 parity) ──► KT-345b (28/28 parity + NetSvc 6A)
  │
  ├── [Slot 3 — aarch64: nền sản phẩm + vifast-core + shell chung]
  │     KT-338b [READY] (MMU/VSpace/EL0 sản phẩm + vifast-core + svc #0 + IPC EL0)
  │       └──► KT-339 (vifast-plat-user đa arch + aarch64 shell)
  │
  └── [Slot 4 — ĐƯỜNG GĂNG SỐ 1 (KHÔNG ĐỂ TRỐNG!) — x86_64 lõi & shell chung]
        KT-340b [READY] (cắt main.rs < 1.100 + .bss sản phẩm < 64 KiB + VSpace động)
          └──► KT-340d (TLB shootdown + IpiMailbox ipi.rs + đóng N5 PCID)
                 └──► KT-340c (x86_64 vifast-core + IPC 7C-r13)
                        └──► (KT-339, KT-340c) ──► KT-341 (x86_64 shell chung)
        │
        ▼
[ĐỢT 3 — Tách Userland v2/user/, Thư viện boot chung & Nghiệm thu Chu kỳ 2]
  ├── (KT-341, KT-342)                   ──► KT-346  (tách v2/user/ + VBREV_ALGO kt134-v5 + P20 3 arch)
  │                                            └──► KT-347  (PoC U1a + hợp đồng .vidl + UEFI/DTB/ACPI)
  ├── (KT-333e, KT-341, KT-345b, KT-346) ──► KT-347b (v2/test/boot/lib/ chung + nightly --soak product-shell/blk)
  └── (đủ bao đóng 33 task)              ──► KT-348  (Cổng nghiệm thu Chu kỳ 2 & Báo cáo R2-v2)
```

---

## 3. Chi tiết từng đợt & bảng task (mọi task ≤ 3 h)

### Đợt 0 — Khép vòng 25, vệ sinh, hạ tầng cổng, quy trình Agent 2 & dọn kho sớm

> **Luật thực thi Đợt 0**: `KT-331`, `KT-332`, `KT-333a..e`, `KT-344` **không** sửa `v2/kernel/**` hay `v2/lib/**` (không làm bẩn `build-inputs`).

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-330** | P0 | **Soak ứng viên cuối vòng 25**: `riscv64` 1.000 + `x86_64` 600 ×2 CPU | `KT-328, KT-329` | 3 h | `DONE` | 0 lỗi cả 3 đợt soak trên `5e635474` (`qemu_foreign=0`, không retry). |
| **KT-331** | P0 | **Guard tên tệp Windows/NTFS + chính sách `JOURNAL/`** | — | 2 h | `DONE` | `check-filenames.py` (5 mã lỗi NTFS, 23/23) nối `v2 rules` + pre-commit; `.gitignore` chặn raw soak log. |
| **KT-332** | P0 | **Đồng bộ tài liệu công khai, cắt gọn `STATE.md` & dọn `BLOCKED.md`** | — | 2 h | `DONE` | `README.md`/`v2/README.md` đồng bộ; `STATE.md` 335,8→3,6 KiB; `BLOCKED.md` dọn 43 mục đóng. |
| **KT-333a** | P1 | **Profile & hạ wall-time T2 (`gate --fast`)** | — | 3 h | `DONE` | T2 hạ từ 766–828 s → **558,9–575,2 s** (`PASS 76/76`); GitHub Actions bỏ vĩnh viễn (`D2-241`). |
| **KT-333b** | P1 | **Dựng `v2 nightly` thật + `deny.toml` + `THIRD-PARTY.md` (D2-21)** | — | 3 h | `DONE` | `v2 nightly` thật (19/19), `deny.toml`, `v2 licenses` (9/9), `THIRD-PARTY.md`. |
| **KT-333c** | P1 | **Khoá độc quyền máy đo + `gpu-x86_64.sh` vào T2 + `v2 nightly --soak`** | `KT-333a, KT-333b, KT-337` | 2 h | `DONE` | `measure-lock.sh` (9/9), logdir PID 6 runner (12/12), `gpu-x86_64.sh` vào T2 (id 79, sửa socket > 104 B), `nightly --soak` (19/19). |
| **KT-333d** | P1 | **Tách `v2/bin/v2` + khóa 2 lớp T0/T1/T2 + `boot/lib/env.sh` + `wt.sh` + `v2 t0` + `agentctl lint` + `x64-max`** | `KT-333c` | 2 h | `READY` | **(D2-245/D2-248)** (1) tách `v2/bin/v2` (2.185 dòng) → `v2/bin/lib/*.sh`; (2) **khóa 2 lớp** (T2/`nightly`/`soak` EXCLUSIVE + pre-flight `< 1 s` dirty/`df < 5 GiB`; bỏ khóa độc quyền toàn thời gian ở `do_gate_dev` T1 + `qemu-slot` max 2; runner T0 kiểm `measure_lock_is_held`); (3) `v2/test/boot/lib/env.sh` phủ 7 runner `LOGDIR` tĩnh & 3 `SOCK` `/tmp` ngắn; (4) `v2/tools/wt.sh` 4 slot tái dùng `target-task/slot-N`; (5) `./v2/bin/v2 t0 <scope>`; (6) `vifast-agentctl lint` + `sync-queue`; (7) thêm `soak-x86_64-max` vào `nightly --soak`; T2 PASS. |
| **KT-333e** | P1 | **Tăng tốc cổng & dọn kho sớm: 1-pass `wake`, 22 phiếu root & tách ~7.602 raw `.log` Chu kỳ 1** | `KT-333d` | 2 h | `BLOCKED` | **(D2-247)** (1) 1-pass combined (`normal` + `--strict`) cho `check-wake-paths.py` (−55–60 s/cổng, giữ selftest 452/452); (2) chuyển 22 phiếu/báo cáo cũ ở root → `v2/agent/archive/reports/` (0 legacy SHA bị đụng); (3) tách ~7.602 raw `.log` Chu kỳ 1 ngoài 165 `scope=` sang nhánh/tag `cycle1-raw-logs`; `verify-numbers.py` (`ok=5 fail=0`) + T2 PASS. |
| **KT-344** | P1 | **RFC Gói duyệt ABI v1.1 (`review/agent0/GOI-ABI-V1_1.md`) — 9 nhóm** | — | 3 h | `DONE` | **`APPROVED 2026-11-22 Agent 0 (D2-242)`**: duyệt 1C..9B + `7C-r13`, `abi_version = 1`. |

### Đợt 1 — Khử nhân bản driver, hợp nhất tầng dùng chung & Áp `spec/` ABI v1.1

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-334** | P1 | **Transport `virtio-pci` (`v2/lib/vifast-virtio`) + `pci`/`virtio_blk` 3 arch + MSRV** | `KT-330` | 3 h | `DONE` | `vifast-virtio` (`sync_for_device/cpu`); `pci`+`blk` −49,3 %; MSRV `1.100.0`. |
| **KT-335** | P1 | **Hợp nhất `virtio_net` 3 arch + TCP client về `v2/lib/vifast-net`** | `KT-334` | 3 h | `DONE` | `netdev.rs` 794 dòng (−42,13 % driver), `tcp_client.rs` 524 dòng, 19 host test, 2 đột biến ĐỎ; tách `KT-335b`. |
| **KT-335c** | P0 | **`x86_64`: DNS `BadFrame` retry trong ngân sách (vá soak 1/600, `D2-246`)** | `KT-335` | 1,5 h | `DONE` | `65e27004` + `f5eb1c82` + `c5996ac8`: `bad_replies` retry trong ngân sách, 2 host test đỏ-trước (dns 14/14), `run-x86_64.sh` 36/36 sạch; xác nhận soak `x86_64` N đầy ở `KT-348`. |
| **KT-335b** | P1 | **Chính sách `DHCP`/`ARP`/`ICMP`/`UDP`/`DNS` về `vifast-net` (bảo toàn `D2-246`)** | `KT-335, KT-335c` | 2 h | `READY` | Hợp nhất 3 trần thời gian DHCP + `NetIo` + `D2-246` (DNS `bad_replies` retry + `bad=<n>`) + `dhcp_frame*` vào `vifast-net`; thiết bị giả `netdev` RX hai chiều; 3 runner xanh y nguyên. |
| **KT-336a** | P1 | **Nối `vifast-sync` (`IrqMcsLock`) vào `x86_64`** | `KT-330` | 3 h | `DONE` | `vifast-plat-x86_64/src/irq_lock.rs` thay spinlock `COM1` + `MH_LOCK`, TAP 34 + 3 đột biến ĐỎ. |
| **KT-336b** | P1 | **Tách `v2/sim/vifast-sim/src/kobj.rs` + đồng bộ checker `wake` + R1 `sim/src/`** | `KT-330` | 3 h | `DONE` | `kobj.rs` 11.964→1.030 dòng (13 module ≤ 1.402); L1-medium 40k & L1-full 400k PASS. |
| **KT-336c** | P1 | **Hoàn tất `riscv64` `IrqMcsLock` (`core_bridge::LOCKED`) & §28 `ObjStore`** | `KT-336a, KT-336b` | 2 h | `DONE` | `vifast-plat-riscv64/src/irq_lock.rs` (`csrrc sstatus,SIE`), TAP 70 + 3 đột biến ĐỎ, §28 `ObjStore`. |
| **KT-337** | P1 | **Hợp nhất `virtio_gpu` + `virtio_input` & mở `virtio-gpu` x86_64** | `KT-334` | 3 h | `DONE` | `gpudev`/`inputdev` −61,4 %; `gpu-x86_64.sh` PPM 23/23 (`-vga none`), đột biến `offset=0` ĐỎ. |
| **KT-345a** | P1 | **Áp `spec/` ABI v1.1 + `vifast-gen` + `VspaceOutcome` rollback (`D2-242`)** | `KT-344` | 3 h | `DONE` | 9/9 nhóm D2-242 ĐẠT (`spec/kernel.toml`, `7C-r13`, `VspaceOutcome`, 3 `.vidl`); `spec/` khoá lại. |

### Đợt 2 — Giải nghẽn `.bss` (< 64 KiB), đóng N5/Q1-A tầng 1, một lõi `vifast-core` 3 arch & sản phẩm `riscv64`

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-338a** | P1 | **`aarch64`: giải phóng `.bss` (`BootMemory`/DMA ra RAM)** | `KT-334, KT-336a` | 3 h | `DONE` | `.data+.bss` **233.704 → 30.632 B (−86,98 %)**; `run-aarch64.sh` 27/27. |
| **KT-338c** | P1 | **`aarch64`: cắt `main.rs` + tách sản phẩm vs `ktest-aarch64` + `IpiMailbox`** | `KT-338a` | 2 h | `DONE` | `main.rs` **1.491 → 549**; marker **97 → 0**, `.bss` sản phẩm **0 B**; `IpiMailbox` SGI 3/3; runner 32/32. |
| **KT-338b** | P1 | **`aarch64`: nền MMU/`VSpace`/EL0 sản phẩm + nối `vifast-core` & IPC EL0 (`svc #0`)** | `KT-338c, KT-345a` | 3 h | `READY` | **(D2-247)** Đưa MMU/`VSpace`/GIC/EL0 sang đường sản phẩm (0 marker); link `vifast-core::Machine` (`.data+.bss ≤ 256 KiB`); nối `svc #0` + `VspaceOutcome` vào `core::syscall`; TAP `core` + `ipc` EL0 + 2 đột biến đỏ *(tách `KT-338d` cho IPC nếu hết khung 3 h)*. |
| **KT-339** | P1 | **Trừu tượng hoá `vifast-plat-user` đa arch + `aarch64` K3.5 (`procsrv` + `shell`)** | `KT-338b` | 3 h | `BLOCKED` | Tách stub syscall theo arch trong `vifast-plat-user`; ảnh sản phẩm `aarch64 --features product-shell` nạp ELF chạy `help`/`echo`/`ps`/`exit` qua PL011 stdin; ca âm ĐỎ; T2 PASS. |
| **KT-340a** | P1 | **`x86_64`: kernel W^X + `budget-x86_64.sh` + tách sản phẩm vs `ktest-x86_64`** | `KT-334, KT-336a` | 3 h | `DONE` | `budget-x86_64.sh` (T2 id 76, 10/10); marker 74→0; kernel W^X TAP 36/36; `.bss` sản phẩm 96.320 B. |
| **KT-340b** | P1 | **`x86_64` (ĐƯỜNG GĂNG): cắt `main.rs` `< 1.100` + hạ `.bss` sản phẩm `< 64 KiB` + `VSpace` động** | `KT-340a` | 3 h | `READY` | **(D2-244)** Cắt `kbin-x86_64/src/main.rs` (`1.500 → < 1.100`); dời bảng trang boot/stack/DMA ra ngoài `.bss` để `.data+.bss` sản phẩm **`< 64 KiB`** (`budget-x86_64.sh`); dựng **`VSpace` động** (`vifast-pte::PageTable` + physmap + đột biến ĐỎ); `run-x86_64.sh` + `-cpu max` + `gpu-x86_64.sh` PASS. |
| **KT-340d** | P1 | **`x86_64` (ĐƯỜNG GĂNG): TLB shootdown đa hart (`IpiMailbox` `ipi.rs`) & đóng lỗi v1 **N5**** | `KT-340b` | 3 h | `BLOCKED` | **(D2-238/D2-244/D2-247)** Cài `invlpg` + IPI shootdown AP→BSP qua `vifast_plat_common::ipi::IpiMailbox` (**tên `ipi.rs`, cấm chữ `doorbell`** — hoàn tất 3/3 port Q1-A tầng 1); kiểm `CR4.PCIDE`/`-cpu max` + đột biến ĐỎ; đóng v1 **N5**; giữ `.bss` sản phẩm `< 64 KiB`. |
| **KT-340c** | P1 | **`x86_64` (ĐƯỜNG GĂNG): nối `vifast-core` & IPC ring 3 (thay scheduler riêng)** | `KT-340d, KT-345a` | 3 h | `BLOCKED` | Link `vifast-core::Machine` (`.data+.bss ≤ 256 KiB`); nối `int 0x80` theo **`7C-r13`** + `VspaceOutcome`; gỡ scheduler trùng lặp; hart phụ `hlt`; TAP `core` + `ipc` ring 3 + `-cpu max` xanh. |
| **KT-341** | P1 | **`x86_64` K3.5 (ĐƯỜNG GĂNG): `procsrv` + `shell` dùng chung `vifast-plat-user`** | `KT-339, KT-340c` | 3 h | `BLOCKED` | Ảnh sản phẩm `x86_64 --features product-shell` chạy cùng nguồn `procsrv`/`shell` qua COM1 stdin; ca âm ĐỎ; cả 3 arch dùng chung 1 shell. |
| **KT-342** | P1 | **Hợp nhất ảnh sản phẩm `riscv64`: gỡ `ktest` khỏi driver + FS, `netsrv`, `exec`** | `KT-335, KT-335b, KT-336a, KT-336c` | 3 h | `BLOCKED` | Cắt sớm `vifast-plat-riscv64/src/user.rs` (`1.499 → < 1.100`); tách feature driver khỏi `ktest`; ảnh sản phẩm chạy đồng thời `ls`/`cat`/`write` + `ping` qua `netsrv` + `run /hello.elf` (`.data+.bss ≤ 256 KiB`, 0 marker); 3 đột biến đỏ. |
| **KT-343** | P1 | **Đóng 11 lệnh `deferred-K7` (`mkdir`, `rm`, `mv`, `ln`, `fsck`, `uname`, `history`, `!!`, `run`, `runfs`, `exec`)** | `KT-342` | 3 h | `BLOCKED` | Lật 11 lệnh → `active` trong `spec/v1-parity.toml` (**25/28 active**); parity `--mode serial` 25/25 ở `smp=1,4`; ca âm từng nhóm. |

### Đợt 3 — Hoàn tất ABI v1.1, tách `v2/user/`, Thư viện boot chung & Nghiệm thu Chu kỳ 2

| Mã | Ưu tiên | Task | Phụ thuộc | Ngân sách | Trạng thái | Tiêu chí xong (DoD) |
|---|---|---|---|---|---|---|
| **KT-345b** | P1 | **Thi hành ABI v1.1: `free`/`df`/`loop` (28/28 parity) + `NetSvc` đa caller** | `KT-342, KT-343, KT-345a` | 3 h | `BLOCKED` | `netsrv` U-mode đa caller theo `socket.vidl` (`6A`); `OP_STATFS` + `MemSvc` (`4C`) đóng `free`/`df`/`loop` (**28/28 active**); parity `--mode serial` 28/28 ở `smp=1,4`. |
| **KT-346** | P1 | **Tách build Userland độc lập (`v2/user/`) khỏi `build.rs` + đóng P20/A1-184 (3 arch)** | `KT-341, KT-342` | 3 h | `BLOCKED` | Build ELF user trong `v2/user/` + `--remap-path-prefix`; thêm `v2/user` vào `VBREV_INPUT_ROOTS` + tăng `VBREV_ALGO` (`kt134-v4` → `kt134-v5`) + `wake_scope.py`; cùng sha256 trên 2 thư mục build cả 3 arch (Linux/macOS). |
| **KT-347** | P2 | **Thiết kế & PoC `U1a` (`no_std+alloc` `vifast-rt`) + hợp đồng `.vidl` `ViScript` + hướng `UEFI`/DTB/ACPI** | `KT-341, KT-346` | 3 h | `BLOCKED` | 1 ứng dụng user `U1a` (stable `1.100.0`) chạy thật qua `exec` từ FS; `v2/docs/u1-uefi-readiness.md` chốt `U1a`/`U1b`, hợp đồng `.vidl` & script kiểm chéo trên máy đo `ViFastOs ↔ ViScript` (không GitHub Actions theo D2-241), và `UEFI`/DTB/ACPI. |
| **KT-347b** | P1 | **Hợp nhất `v2/test/boot/lib/` (`harness-scan`/`budget`/`soak`) + nối `nightly --soak --profile product-shell` & `virtio-blk`** | `KT-333e, KT-341, KT-345b, KT-346` | 2 h | `BLOCKED` | **(D2-245/D2-247)** Rút `v2/test/boot/lib/{harness-scan,budget-common,soak-common}.sh` (1 bộ quét D2-08 chung 3 arch + khử lặp 4.202 dòng); nối `--profile product-shell` (3 arch + `x64-max`) & đợt **soak đĩa thật `virtio-blk`** vào `v2 nightly --soak`; dọn mục cũ `BLOCKED.md`; T2 PASS. |
| **KT-348** | P0 | **Cổng nghiệm thu Chu kỳ 2: Soak sản phẩm 3 arch + đĩa thật + T2 + `v2 nightly --all` + Báo cáo `R2-v2`** | `KT-331, KT-332, KT-333a..e, KT-335b/c, KT-336b/c, KT-337, KT-341, KT-343, KT-345b, KT-346, KT-347, KT-347b` | 3 h | `BLOCKED` | Chạy độc quyền (`dirty=0`, `qemu_foreign=0`): T2 PASS; `v2 nightly --all` (L1 100k × 4 SMP + Miri + `STATUS.md`) PASS; soak **ảnh sản phẩm có shell** 3 arch (`rv 1000` + đĩa thật `virtio-blk`, `a64 1000`, `x64 600 qemu64` + `600 -cpu max`) đều 0 lỗi; đối chiếu **Exit Criteria `v2.0` (§6.2)**; xuất `v2/agent/reports/R2-v2.{md,json}`; trình Agent 0 chốt `v2.0`. |

---

## 4. Vùng xung đột & quy chế vận hành tối ưu cho Agent 2 (**D2-236..D2-248**)

| Vùng tệp / Nút thắt | Rủi ro đã xảy ra | Quy tắc bắt buộc |
|---|---|---|
| **Khóa 2 lớp T0/T1 vs T2** (`D2-248`) | `do_gate_dev` giữ `measure-lock` chặn 4 slot; runner T0 đè lên T2 gây `T-INFRA` | **T2 / `nightly` / `soak`** giữ `measure-lock` EXCLUSIVE + pre-flight `< 1 s` (`dirty=0`, `df ≥ 5 GiB`); **T1 (`gate --dev`)** bỏ khóa độc quyền toàn thời gian, chỉ kiểm `measure_lock_is_held` + `qemu-slot` (max 2); **runner T0** kiểm `measure_lock_is_held`. |
| **4 slot worktree cache ấm** (`D2-248`) | 9 worktree chiếm 12 GiB; build nguội chậm | Tái dùng đúng 4 slot qua **`v2/tools/wt.sh`** (`target-task/slot-<1..4>`, giữ ấm cache giữa các task, chặn mở khi `df < 5 GiB`). |
| **Bẫy `cargo check -p` & R3 SPDX** (`D2-248`) | `cargo check -p` ẩn lỗi `ktest`; chờ 100 s mới thấy lỗi SPDX/`doorbell` | Sub-agent chạy **`./v2/bin/v2 t0 <scope>`** (< 15 s: `fmt` + `SPDX` + `R1` + `E_ARCH_DOORBELL` + build cả `product` & `--features ktest` + host test) trước khi chạy T1. |
| **1 nguồn sự thật sổ sách** (`D2-248`) | Quên lật `READY` (`KT-340b`/`KT-333d`/`KT-338b`); `STATE.md` lệch `QUEUE.md` | Chạy **`vifast-agentctl lint`** (trong `v2 check`) và `sync-queue` sau mỗi đợt merge; cấm "DONE một phần" (D2-245). |
| `v2/bin/v2` (2.185 dòng) & `$TMPDIR`/`SOCK` | Đụng merge liên tục; lỗi `AF_UNIX > 104 B` trên `gpu-x86_64.sh` | **`KT-333d`** tách `v2/bin/v2` → `v2/bin/lib/*.sh` và tạo `v2/test/boot/lib/env.sh` (`LOGDIR` PID + `SOCKDIR` `/tmp` ngắn) phủ toàn bộ runner còn lại. |
| `JOURNAL/` & tệp `[[legacy]]` | 7.602 `.log` thừa làm chậm repo; sửa nhầm `k6-virtio-blk.md` vỡ legacy SHA | Mỗi task chỉ **1 tệp `JOURNAL/kt<n>-<ten>.md`**, 0 `kt*-logs/*.log`; **`KT-333e`** dọn 7.602 `.log` + 22 phiếu root sớm; **CẤM sửa 19 tệp `[[legacy]]`** trong `numbers-policy.toml`. |
| Tệp nóng R1 (`1.500` dòng) | `kbin-x86_64/main.rs` (1.500), `plat-riscv64/user.rs` (1.499), `core_ipc.rs` (1.470), `machine.rs` (1.430) | Cắt `< 1.100` ngay ở đầu **`KT-340b`** & **`KT-342`**; theo dõi `core_ipc.rs`/`machine.rs` trước khi chạm vào. |
| `spec/abi.toml`, `spec/kernel.toml`, `.github/` | Vỡ đóng băng ABI; vi phạm cấm CI | `spec/` khoá cứng sau `KT-345a`; **cấm vĩnh viễn `.github/workflows/**`** (`D2-241`). |

---

## 5. Kịch bản chạy đêm tự hành tối ưu tới 09:00 sáng mai (`D2-248`, 2026-11-23 00:10 → 09:00 JST)

> **Cơ sở định lượng**: `KT-335c` đã chốt **`DONE`** tại `c5996ac8` (không còn 35 phút chờ soak đầu phiên; xác nhận soak `x86_64` N đầy ở `KT-348`). Tốc độ đo được ở phiên chiều–tối 2026-11-22 là **~2 task/giờ** trên 4 slot song song; T2 `gate --fast` = **9,5 phút** (~8,5 phút sau `KT-333e`). Mục tiêu đến **09:00 JST (~8,8 giờ)** là hoàn tất **toàn bộ Đợt 0 + Đợt 1 + Đợt 2 + `KT-333d/e` + `KT-345b`** (**28–30 / 34 task `DONE`**, cả 3 kiến trúc lên chung `vifast-core` + `shell` + 28/28 parity v1 + đóng 7/7 lỗi v1), để sáng mai chỉ còn đúng 4 task Đợt 3 cuối cùng (`KT-346 → KT-347 ∥ KT-347b → KT-348`). *(Nếu phiên chạy kéo tới **09:00 ICT = 11:00 JST**, dùng 2 giờ dự phòng chạy luôn **`KT-346`** để sáng đạt **31/34 `DONE`**!)*

### Lịch 4 đợt nối tiếp + 4 Checkpoint T2 gộp lô (khởi chạy ngay đủ 4 slot `READY`)

| Khung giờ (JST) | Slot 1 (Hạ tầng/Kho) | Slot 2 (`riscv64`/Net) | Slot 3 (`aarch64`) | Slot 4 (**ĐƯỜNG GĂNG** `x86_64`) | Cổng Lead (T2 gộp lô) |
|---|---|---|---|---|---|
| **Đợt Đêm 1**<br>`00:10 – 01:45` | **`KT-333d`** *(READY — **ưu tiên merge #1** để mở khóa 2 lớp, `wt.sh`, `v2 t0`, `agentctl lint` cho Đợt 2–4)* | **`KT-335b`** *(READY — DHCP/ARP/ICMP/UDP/DNS → `vifast-net`, bảo toàn `D2-246` + `bad=<n>`)* | **`KT-338b`** *(READY — MMU/`VSpace`/EL0 sản phẩm + `vifast-core` + `svc #0`; tách `KT-338d` cho IPC nếu chạm 3 h)* | **`KT-340b`** *(READY — cắt `main.rs` < 1.100 + `.bss` sản phẩm < 64 KiB + `VSpace` động)* | **01:45 – 02:00**: Dừng QEMU lẻ ⇒ Merge lô (`KT-333d` trước → `KT-335b` + `KT-340b` + `KT-338b` nếu xong), chạy **T2 #1** (~9,5 p), push `main` |
| **Đợt Đêm 2**<br>`02:00 – 04:00` | **`KT-333e`** *(1-pass `wake` −60 s + gom 22 phiếu root + tách 7.602 `.log` — merge sớm ngay khi xong!)* | **`KT-342`** *(cắt `user.rs` < 1.100 + sản phẩm `riscv64` FS+`netsrv`+`exec`)* | **`KT-339`** *(hoặc chốt nốt `KT-338b`/`KT-338d` rồi mở ngay `KT-339`: `vifast-plat-user` + `aarch64` shell)* | **`KT-340d`** *(TLB shootdown + `IpiMailbox` `ipi.rs` + đóng N5 PCID)* | **04:00 – 04:15**: Merge lô (`KT-333e` trước → `KT-342` + `KT-339` + `KT-340d`), chạy **T2 #2** (~8,5 p), push `main` |
| **Đợt Đêm 3**<br>`04:15 – 06:15` | *(Rảnh ⇒ chuẩn bị nháp `v2/docs/u1-uefi-readiness.md` cho `KT-347` hoặc hỗ trợ `KT-339` nếu còn nối)* | **`KT-343`** *(11 lệnh `deferred-K7` → **25/28 active** parity)* | *(Hoàn tất `KT-339` nếu còn nối từ Đợt 2)* | **`KT-340c`** *(`x86_64` `vifast-core::Machine` + IPC `7C-r13` + gỡ scheduler riêng)* | **06:15 – 06:30**: Merge lô (`KT-343` + `KT-340c`), chạy **T2 #3** (~8,5 p), push `main` |
| **Đợt Đêm 4**<br>`06:30 – 08:25` | *(Rảnh ⇒ rà soát thiết kế `v2/user/` cho `KT-346` & dọn mục đóng ở `BLOCKED.md`)* | **`KT-345b`** *(`free`/`df`/`loop` → **28/28 parity** + `netsrv` đa caller `6A`)* | *(Nếu `KT-341` + `KT-342` xong sớm ⇒ khởi động `KT-346`)* | **`KT-341`** *(`x86_64` `procsrv` + `shell` dùng chung `vifast-plat-user`)* | **08:20 – 08:35**: Merge lô (`KT-345b` + `KT-341`), chạy **T2 #4** (~8,5 p), push `main` |
| **Chốt sáng**<br>`08:35 – 09:00` | **NGỪNG MỞ TASK MỚI TỪ 08:25** | Dọn worktree còn sót, kiểm `df -h`, chạy `vifast-agentctl lint` | Cập nhật `STATE.md` sạch 100 % khớp `QUEUE.md` | Ghi tóm tắt phiên đêm (chỉ trích nguyên văn log, 0 số viết tay) | Push sạch `origin/main` trước **09:00 JST** *(nếu chạy tới 09:00 ICT = 11:00 JST thì mở luôn **`KT-346`** từ 08:35–10:35 rồi chốt T2 #5)* |

### 5 nguyên tắc an toàn bắt buộc cho chạy đêm không giám sát (Fail-Safe)
1. **Giữ `qemu_foreign=0` tuyệt đối khi Lead chạy T2 / `nightly`**: trước khi `KT-333d` được merge, Lead chốt ranh giới T2 #1 bằng cách dừng mọi QEMU sub-agent trước khi chạy `v2 gate --fast`; sau khi `KT-333d` merge, khóa 2 lớp (`measure_lock_is_held` + `qemu-slot`) sẽ cưỡng chế tự động, chấm dứt hoàn toàn `T-INFRA`.
2. **Bảo vệ Đường găng Slot 4**: bất kỳ lúc nào Slot 4 (`KT-340b → KT-340d → KT-340c → KT-341`) hoàn tất 1 bước, Lead ưu tiên merge/mở bước kế tiếp ngay (không để Slot 4 chờ các slot khác).
3. **Rule 2.5 chống treo máy**: task nào gặp cùng 1 lỗi 2 lần hoặc vượt trần 3 h ⇒ dừng ngay, ghi nguyên nhân + log vào `BLOCKED.md` (hoặc tách phần đã xanh thành task con hợp lệ `^KT-[0-9]+[a-z]?$` theo `D2-245`), chuyển slot sang task kế tiếp. **Không** retry để lấy xanh (D2-07).
4. **Cổng cắt giờ 08:25**: không bắt đầu bất kỳ task lớn mới nào trong 35 phút cuối trước mốc 09:00; ưu tiên 100 % bàn giao `main` sạch (`git status -s` rỗng, đã qua T2, `agentctl lint` xanh, đã push `origin/main`).
5. **Canh ổ đĩa trước mỗi đợt**: đảm bảo `df ≥ 8 GiB` đầu mỗi đợt; nếu `< 5 GiB` chạy `wt.sh prune` (hoặc dọn `target-task` của task đã merge) ngay lập tức.

---

## 6. Khung lộ trình dài hạn (Chu kỳ 3 → `v2.0` → `v2.1` → `U1`/`VSH2`/`HW1`) & Tiêu chí phát hành `v2.0` (**D2-240/D2-241/D2-244/D2-245/D2-248**)

### 6.1 Bảy trụ cột chiến lược sau Chu kỳ 2 (Chu kỳ 3 trở đi)

| # | Trụ cột | Mốc đề xuất | Thực trạng hiện tại & việc bắt buộc phải làm |
|---|---|---|---|
| 1 | **SMP Preemptive thật (`S1 → S2 → S3`) & Tầng 2 `Mailbox<N>`** | `SMP-1`, `SMP-2` | Sau khi Chu kỳ 2 thống nhất `IpiMailbox` + `IrqMcsLock` trên 3/3 port (D2-244), `SMP-1` đưa `vifast-core::Machine::dispatch` + timer preempt + tầng 2 `vifast_sync::Mailbox<N>` (có bit `OVERFLOW` rơi về `IpiMailbox`) chạy U-mode thật trên 4 hart cả 3 arch, rồi `PerHart` (`SMP-2`); chỉ mở `S3` khi `vifast-bench` đo chờ khoá > 10 % ở `smp=4`. |
| 2 | **Hợp nhất `Machine` (đóng `D2-83`) & `vifast-objmem` (D2-14)** | `MEM-1`, `MEM-2` | Rút Sv39/PLIC khỏi `vifast-plat-common`, đưa `TableMemory` lên `vifast-pte` (`MEM-1`); tách `core_ipc.rs`/`machine.rs` (< 1.100 dòng); gom 14 `[[bin]]` của `vifast-kbin-riscv64` về 2 mode `product`/`ktest`; chuyển cấp phát đối tượng vào `Untyped` thật qua `vifast-objmem` (`MEM-2`). |
| 3 | **4 nền tảng kiểm chứng/quan sát trong thiết kế v2** | `OBS-1`, `VER-1` | (i) **trace nhị phân 32 B luôn bật** + dump `pmemsave` + `vifast-trace` (**D2-09**); (ii) **mutation testing tự động** `v2/test/mutants.toml` + `v2 mutants` trong `nightly` (gom 28 feature đột biến ad-hoc của `vifast-kbin-x86_64/Cargo.toml` về đây, D2-248); (iii) **`vifast-bench` + `test/perf/history.csv`** dưới `-icount`; (iv) **conformance `.scn` đủ 3 arch** (D2-13) + **MCS refill đa đoạn/`SetTimeoutEp`** (D2-16). |
| 4 | **Driver từ Polling → IRQ `Notification` → Userland + IOMMU** | `DRV-1`, `D2`, `D3` | Chuyển driver VirtIO từ busy-poll sang ngắt thật qua **`IrqControl`/`IrqHandler` + `Notification`** (ABI v1.2); làm nền cho driver userland (`D2`: PCIe MSI-X, NVMe, AHCI) kèm `Frame.MapIo` + **`IOSpace` (VT-d / SMMUv3 / virtio-iommu, `D3`)**. |
| 5 | **An toàn, FPU/SIMD & chuẩn hoá `.vidl`** | `SEC-1`, `IDL-1` | Lưu/khôi phục **ngữ cảnh FPU/SIMD theo TCB** (`fs0..fs31`, NEON `v0..v31`, `fxsave`/`xsave` — bắt buộc trước `U1`); test đối kháng `copy_from_user`/`copy_to_user` + `SMEP`/`SMAP`/`PAN`; fuzz `vifast-net` + `virtio-rng`; chuẩn hoá `proc/fs/net/term.vidl`. |
| 6 | **`U1` hai tầng & Hợp đồng liên repo `ViFastOs ↔ ViScript` (`VSH2`)** | `U1a`, `U1b`, `VSH2` | `U1a` (`no_std + alloc` + `vifast-rt`, stable `1.100.0`) kéo `vis-host-vifast`/ViShell lên sớm; `U1b` (Rust `std` PAL `*-unknown-vifastos`); kiểm chéo định kỳ trên máy đo (`v2 nightly`, không GitHub Actions theo **D2-241**) với [`ViScript`](VISCRIPT.md). |
| 7 | **Khám phá phần cứng thật (`HW1`) & Tinh gọn kho mã nguồn (Pha 3)** | `HW-0`, `UEFI`, `HW1` | **`HW-0`**: phân tích **DTB thật** (`riscv64`/`aarch64`) và **ACPI `MADT`/`FADT` + E820/UEFI** (`x86_64`) thay hằng QEMU. Đóng băng & tách nội bộ `check-wake-paths.py`; tổ chức `v2/tools/{checkers,build,ops}/`; đánh giá gỡ cây v1 đóng băng (`crates/`, `legacy/`, `deploy/`, `python-model/`, `Cargo.toml` v1 — giữ tag `v1-archive`) khỏi `main` sau `v2.0`. |

### 6.2 Bảng Tiêu chí phát hành (Exit Criteria) phân định `v2.0` vs `v2.1` (đối chiếu ở `KT-348`)

| Hạng mục | Bắt buộc cho **`v2.0`** | Chuyển **`v2.1`** / Chu kỳ 3 |
|---|---|---|
| **Ba kiến trúc cùng 1 lõi + 1 shell** (`riscv64`/`aarch64`/`x86_64`, tách sản phẩm/harness D2-08) | **Có** (`KT-338a..KT-342`) | `SMP-1/2` preemptive 4 hart đầy đủ + gom 14 bin `kbin-riscv64` về 2 mode |
| **Tương đương v1 (`spec/v1-parity.toml`)** | **28/28 `active`** (`KT-343` + `KT-345b`, `smp=1` & `smp=4`) | Mở rộng lệnh `ViShell` (`VSH2`) |
| **Đóng đủ 7 lỗi v1 (`KNOWN-ISSUES.md` N1..N7)** | **Có** (đóng **N5** ở `KT-340d` và **Audit Q1-A/N7 tầng 1**: `IpiMailbox` + `IrqMcsLock` đủ 3/3 port qua `KT-336a/336c/338c/340d`) | Tầng 2 `vifast_sync::Mailbox<N>` có `OVERFLOW` bit (`SMP-1`) |
| **Ngân sách & Cổng** (`.data+.bss ≤ 256 KiB` 3 arch, `Machine ≤ 196.608 B`, T2 PASS, `v2 nightly --all` L1 400k + Miri PASS) | **Có** (`budget*.sh`, `KT-333b..e`, `KT-347b`, `KT-348`) | `v2 mutants` (thay 28 feature x64) + `vifast-bench` trong nightly |
| **Soak nghiệm thu** (không retry D2-07, `qemu_foreign=0`, `dirty=0`) | **Ảnh sản phẩm có shell** 3 arch (`rv 1000`, `a64 1000`, `x64 600 qemu64 + 600 -cpu max`) + **soak đĩa thật `virtio-blk`** (`KT-348`) | Soak đa tiến trình `U1`/mạng tải nặng |
| **Tái lập byte (P20) & Vệ sinh kho** | **Cùng sha256 canonical cả 3 arch trên Linux + macOS** (`KT-346`); dọn ~7.602 raw `.log` Chu kỳ 1 + 22 phiếu root (`KT-333e`) + thư viện boot chung (`KT-347b`) | Hội tụ trên host **Windows** (B2); gỡ cây v1 (`crates/`) khỏi `main` |
| **Giấy phép & Chuỗi cung ứng (D2-21)** | **`deny.toml` + `THIRD-PARTY.md` tự sinh** (`KT-333b`), clone/checkout sạch Win/macOS/Linux (`KT-331`) | Nhúng license vào ảnh + ký phát hành (nếu cần) |
| **Động hóa bộ nhớ, FPU/SIMD, `IrqHandler`, `U1`/`VSH2`, `UEFI`/`HW1`** | Có tài liệu định hướng & PoC `U1a` (`KT-347`) | Thi công ở **Chu kỳ 3 (`v2.1+`)** theo §6.1 |

