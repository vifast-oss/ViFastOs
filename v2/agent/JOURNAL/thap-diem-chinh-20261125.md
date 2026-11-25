# Lô Thấp điểm chính — 2026-11-25 10:00 UTC → 2026-11-26 01:00 UTC

- **Mục tiêu** (chỉ đạo người điều phối): đang giờ thấp điểm (0,5×) ⇒ triển khai **task song song**
  tới **giờ cao điểm kế tiếp thì dừng** (`D2-256` / `AGENTS.md` §6.4). Khung: `10:00 UTC 25/11`
  → `01:00 UTC 26/11` (15 h liên tục, T3). Cao điểm kế: `01:00–04:00 UTC 26/11` (T4).
- **Cây**: `main` @ `c33ad729` (bằng chứng Cao điểm 2; mã `a61921b4`), `dirty=0`.
- **Máy**: `mps` (macOS aarch64, 12 core), QEMU 11.1.3, toolchain ghim `1.100.0`.
- **Luật áp**: sub-agent chỉ **T0 + T1**; Lead merge + **T2** gộp lô; không nới ngưỡng; không retry
  lấy xanh; tái dùng bằng chứng §6.3; **không** GitHub Actions (§4.11).

## 1. Bố trí 4 slot (`v2/tools/wt.sh`, D2-245)

| Slot | Task | Nhánh | Worktree | Cache build |
|---:|---|---|---|---|
| 1 | `KT-404` — workspace Cargo thật `v2/user/` + `vifast-kbuild` + `N-7` + 9 crate `forbid(unsafe_code)` | `task/kt404-user-workspace` | `/private/tmp/vifast-wt/1` | `target-task/slot-1` |
| 2 | `KT-408` — trait `KernelStore` + `Machine<S,O>` + ring trace 32 B kernel | `task/kt408-kernelstore-ring` | `/private/tmp/vifast-wt/2` | `target-task/slot-2` |
| 3 | `KT-402b` — harness `aarch64 -dtb` + `x86_64 fw_cfg/table-loader` + ca âm 3 arch | `task/kt402b-bootinfo-harness` | `/private/tmp/vifast-wt/3` | `target-task/slot-3` |
| 4 | `KT-414b` — UEFI `aarch64` qua AAVMF + `BootSource::Uefi` | `task/kt414b-uefi-aarch64` | `/private/tmp/vifast-wt/4` | `target-task/slot-4` |

- **Khoá T1**: `/tmp/vifast-t1-lock.sh` — chỉ **một** `gate --dev` chạy trên máy tại một thời điểm
  (T1 có QEMU smoke + L1 1.000 seed; §9 tối đa 3 QEMU song song, thực tế giữ 1 để số đo không nhiễu).
- **Xung đột tệp đã biết**: `KT-402b` và `KT-414b` cùng chạm `v2/test/boot/run-aarch64.sh` (khác hunk:
  `-dtb` vs `--firmware uefi`) — Lead giải khi merge. Các cặp còn lại 0 chồng tệp theo `KE-HOACH-KIEN-TRUC.md`.
- **Tiền đề `KT-414b`**: `aarch64-unknown-uefi` + `x86_64-unknown-uefi` đã cài cho `1.100.0` (đã kiểm);
  AAVMF `edk2-aarch64-code.fd` có sẵn trong `/opt/homebrew/share/qemu/`.

## 2. Tiến độ

| Mốc (UTC) | Việc | Kết quả |
|---|---|---|
| 10:00 | Cấp 4 slot + khởi chạy 4 sub-agent | 4/4 chạy |
| 10:04–11:20 | 4 sub-agent làm task (T0 + T1 qua khoá) | 4/4 xong, 0 commit (đúng luật: agent không commit) |
| 11:20–11:35 | Lead commit từng nhánh + merge `--no-ff` vào `main` | 4/4 merge sạch (`7b970aea`), 0 xung đột |
| 11:35 | **T2 lần 1** (`gate --fast`) trên ứng viên gộp | **ĐỎ** đúng 1 bước `wake dep-info`: `forbid(unsafe_code)` (KT-404) × `link_section` (KT-408) |
| 11:45 | **Merge fix**: chuyển kho ring `.corespace` `vifast-traceformat` → `vifast-sync` (D2-17) | `f3cf3c97`; `v2 check` PASS; kernel workspace `cargo check` RC=0 |
| 11:55 | **T2 lần 2** trên ứng viên đã vá | **PASS 79/79**, 647,3 s |
| 12:05 | `L1-medium` 10k (KT-408) | xem §3 |
| 01:00 (26/11) | **Dừng** theo lịch | — |

## 3. Bằng chứng

### 3.1 T1 từng nhánh (sub-agent, qua khoá `/tmp/vifast-t1-lock.sh`)

| Task | T1 | Ghi chú |
|---|---|---|
| `KT-404` | **PASS** 705,6 s (`registered=80 ran=66 skipped=14 failed=0`) | chạy trên bản sao sạch (worktree bẩn làm `api boundary`/`vbrev` fail-closed) |
| `KT-408` | 46/47 — đỏ duy nhất `báo cáo R1-v2` | đỏ do bản sao git hỏng; `check_report.py --all` **PASS** trên worktree thật |
| `KT-402b` | 583,7 s — 64 làn OK, 1 đỏ `selftest pre-measure` | đỏ do `refuse 7 tree-dirty` trên worktree bẩn; PASS trên cây sạch |
| `KT-414b` | **PASS** 652,8 s | — |

### 3.2 T2 trên ứng viên gộp

- **Lần 1** (`7b970aea`): **ĐỎ** — `wake theo dep-info` → `cargo check` kernel workspace RC=101:
  `error: usage of the unsafe link_section attribute` (`vifast-traceformat/src/kernel_ring.rs`).
  Nguyên nhân: KT-404 bật `#![forbid(unsafe_code)]` cho `vifast-traceformat`; KT-408 đặt kho ring
  (cần `link_section`) trong chính crate đó. Hai nhánh xanh riêng lẻ, gộp mới lộ.
- **Merge fix** (`f3cf3c97`): `kernel_ring.rs` → `vifast-sync` (crate thấp nhất trong danh sách
  đóng D2-17 mà `vifast-core` được phụ thuộc); `vifast-traceformat` giữ `forbid(unsafe_code)`;
  `vifast-core` thêm dep `vifast-sync`. 0 đổi định dạng 32 B / `size_of::<Machine>()` / `.corespace`.
- **Lần 2** (`f3cf3c97`): **PASS 79/79**, `step-time-total 647273 ms`; log `/tmp/t2-thapdiem-20261125-r2.log`.

### 3.3 Kiểm phụ sau vá

`cargo check` kernel workspace **RC=0**; `vifast-core` **112/112**, `vifast-sync` **19/19**,
`vifast-traceformat` **9/9**; `v2 check` **PASS** (84,1 s); `check-lockfile-sync` **OK**;
`check_report.py --all` **PASS**.

### 3.4 L1-medium (KT-408)

`bash v2/tools/l1-tier.sh --tier medium` — **PASS**: 4 cấu hình smp × 6 lô = **24 task**, `wall 167 s`,
phủ đủ 10.000 seed/cấu hình (`== l1-tier: PASS (mọi lô xanh, phủ đủ seed) ==`); log
`/tmp/l1-medium-thapdiem-20261125.log`.

---

## 4. Lô 2 (2026-11-25 21:20 UTC → 2026-11-26 00:20 UTC) — KT-409/409b/410/411

| Task | Kết quả | Bằng chứng |
|---|---|---|
| `KT-409` phần 1 | **DONE** (`0fbdaab6`) | bước 84 `step_clippy_kernel` (host `--workspace --exclude` 4 crate + `--lib` 3 target arch); dọn 5+7+9 cảnh báo host + 4+4 target; gỡ dev-dep ngược; `TableMemory` lên `vifast-pte`; phát hiện `#[test]` bị nuốt trong doc comment (`plat-x86_64/tests/apic.rs`) |
| `KT-409b` | **DONE** (`e495365f`) | Sv39 (621 LOC) → `vifast-pte::sv39`; PLIC/trap/timer/tlb/user (2.649 LOC) → crate mới host-testable `vifast-plat-riscv64-logic`; `plat-common` 480→170 dòng RISC-V, 6 module mục tiêu 0 RISC-V; 0 `cfg(target_arch)` mới |
| `KT-410` | **DONE** (`5ffaed96`) | `UntypedStore` + `GENERAL_*` + `BootArena` 617.208 B + `SimHeapStore` vi sai + gỡ `vifast-model`; `StaticStore` chặn TCB 65, `UntypedStore` tạo 512 TCB/1.024 Cap, `Revoke` sạch; T1 tự bắt hồi quy `snaps` 32 KiB trên stack 20 KiB (đã sửa) |
| `KT-411` | **DONE** (`2b770814`) | crate `vifast-ktest` (1.427 LOC, `forbid(unsafe_code)`) + `CoreBridge` chung + TAP 43..52 + 12 nhóm invariant; `ktest_core.rs` 1.090→435; `IrqMcsLock<T,S>` 1.170→594 (−576); vá hồi quy KT-409b (6/14 ảnh ktest không biên dịch) |

- **T2 gộp lô 2**: `registered=80 ran=80 skipped=0 failed=0` — **PASS**, `step-time-total 719459 ms`
  (`2b770814`); log `/tmp/t2-lan2-20261126.log`.
- **L1-medium**: **PASS** — 4 cấu hình × 6 lô = 24 task, `wall 166 s`; log `/tmp/l1-lan2-20261126.log`.
- **Miri** (`vifast-objmem --lib`): **PASS** — 25 test, 6,48 s; log `/tmp/miri-objmem-20261126.log`.
- **Nợ tách**: `KT-411b` (gom 14 `[[bin]]` → 2 mode + `< 16.000` LOC + hợp nhất bộ ca a64/x64 — cần
  quyết `expected.txt`), `KT-412`/`KT-413` mở READY.
- **Merge conflict đã giải**: `plat-common/Cargo.toml` (KT-410 `vifast-objmem` × KT-411 `vifast-sync`)
  — giữ cả hai; `plat-common` cần `vifast-core` là **phụ thuộc thường** (KT-410 `untyped_store` dùng
  `Machine`/`UntypedStore`), không chỉ dev-dep.

---

## 5. Lô 3 (2026-11-26 00:20 UTC → 01:00 UTC) — KT-412/413

| Task | Kết quả | Bằng chứng |
|---|---|---|
| `KT-412` | **DONE** (`34433b26`) | `set_harts(n)` từ `BootInfo` thật + `PerHartState` rv/a64; 4 hart preemptive (0 double-pick/0 starvation, `published` §4.16); **bắt 2 lỗi thật của core**: `set_harts` reset mọi hart + `published` fail-open ngoài `n_harts`; 3 đột biến `mut-smp-*` wired (`declarations` 43→46); boot thật rv 74 ok/0 not ok (smp 1 & 4), a64 39/39 `boot_harts=4` |
| `KT-413` | **DONE** (`e6e9ac76`) | 4 hart x86_64 vào core qua `CoreBridge` + bộ ca SMP (TAP 48/49); đóng `N-3` (watchdog LAPIC vector `0x32`, `# timer: watchdog`, ca âm `no-pit-irq`); boot thật `# core smp: harts=4 picks=[1,1,1,1] distinct=4 double=0`; `run-x86_64` 39/39, 33/33 ca âm |

- **T2 lần 1** (`e6e9ac76`): **ĐỎ** 2 bước — (a) `ktest smp=1`: plan `1..71` trong ảnh riscv64 chưa cập nhật
  lên `1..74` (KT-412 thêm 3 TAP nhưng sót dòng plan); (b) `ca âm git-guard song song (R5)`:
  `số worktree KHÁC trước (5 → 6)` — **flake T-INFRA**, chạy lại `neg-test.sh --selftest-parallel`
  ⇒ **PASS** (RC=0).
- **Vá** (`e66b0e28`): `ktest.rs` plan `1..71` → `1..74`.
- **T2 lần 2** (`e66b0e28`): **PASS 80/80**, `step-time-total 692316 ms`; log `/tmp/t2-lan3-r2-20261126.log`.
- **L1-medium**: **PASS** — 24 task, `wall 163 s`; log `/tmp/l1-lan3-20261126.log`.
- **Merge conflict đã giải**: `vifast-ktest/src/smp_suite.rs` + `tests/smp_suite.rs` (add/add — KT-412 và
  KT-413 cùng tạo). Giữ **hai** bộ: `smp_suite` (rv/a64, TAP 71..73) + `smp_suite_x64` (x64, TAP 48/49);
  `coresmp.rs` dùng `smp_suite_x64`. **Nợ tách `KT-413b`**: hợp nhất thành một bộ 3 arch (tham số hoá TAP đầu).
- **Nợ tách**: `KT-413b` (gỡ `sched.rs` + PCID/ASID + hợp nhất `smp_suite`), `KT-415`/`KT-416` mở READY.

---

## 6. Lô 4 (2026-11-26 01:00 UTC → 02:10 UTC) — KT-415/416/418

| Task | Kết quả | Bằng chứng |
|---|---|---|
| `KT-415` | **DONE** (`fe79074d`) | `Mailbox<N>` tầng 2 trên `IpiMailbox` + `OVERFLOW` (bảo toàn `delivered+overflowed==posted`, 4 luồng × 2.000 message); range TLB shootdown 3 arch (`FlushScope::Range` + `sfence` mỗi trang); bench BKL 4 hart: **0 % → 39,6 % → 93,5 %** tranh chấp (1/2/4 hart), `in_cs=0 overlap=0`; 3 đột biến KILLED (`declarations` 46→49) |
| `KT-416` | **DONE** (`26a24606`) | RFC 8 nhóm `review/agent0/GOI-ABI-KIEN-TRUC.md` + `.json`; 6 phát hiện đo được (17 `EP_*` không phải 16; `check_kernel.py:277` đòi `[limits.plat]`; `Tcb.BindNotification` 13/14 chỉ model API; `MapIo`/`IOSpace` 0 dòng; `[profiles]` chưa có; FPU/SIMD chưa có vùng lưu). **Chưa duyệt** — `KT-417` chờ Agent 0 |
| `KT-418` | **DONE** (`db3e21bb`) | FPU/SIMD eager theo TCB (`FpuArea` 528 B ngoài `TcbCore`, `FpuTracker<65>`, `sstatus.FS` dirty-tracking) — 2 tác vụ U-mode FP trên 4 hart không lẫn thanh ghi (`ok 38`); `SUM=0` rv, `SMEP` x64 (CPUID-gated), `PAN` a64 (`pan_supported()`); fuzz `vifast-net` 20.000 chuỗi + đột biến. **Nợ `KT-418b`**: `virtio-rng` + `SMAP` (`stac`/`clac`) + ca âm `-cpu max` |

- **T2 lần 1** (`db3e21bb`): **ĐỎ** 3 bước — (a) `ktest-guard`/`ktest-user`: evidence regex sai
  (`fault_scause=0xd` nhưng log ghi `13`; `taps=37` nhưng ảnh 38) + thiếu evidence FPU; (b) `gpu-x86_64`:
  `EXPECT_TESTS` 50 nhưng ảnh 51 TAP; (c) `selfbreak` luật 2 — **flake** (pass độc lập).
- **Vá** (`bd8e20bb`): `ktest.sh` evidence (`fault_scause=13`, `taps=38`, thêm `# fpu:`), `gpu-x86_64.sh`
  `EXPECT_TESTS=51`.
- **T2 lần 2** (`bd8e20bb`): **ĐỎ** 2 bước — `selfbreak` (flake lặp) + `gpu-x86_64` (TAP 50 **trùng**:
  SMEP chiếm 50, GPU vẫn 50).
- **Vá** (`dac8023d`): `net.rs` GPU TAP `tap_first+35` → `+36` (50→51); thêm chẩn đoán mã thoát git
  trong `selfbreak.sh` `case_law2` (không đổi kịch bản).
- **T2 lần 3** (`dac8023d`): **PASS 80/80**, `step-time-total 834739 ms`; log `/tmp/t2-lan4-r3-20261126.log`.
- **Nợ tách**: `KT-418b` (virtio-rng + SMAP + ca âm `-cpu max`), `KT-411b`/`KT-413b` đang/đã tách.
