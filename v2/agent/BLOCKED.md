# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-26 (`D2-262`)**: **không còn bất kỳ task nào bị chặn kỹ thuật hay chờ duyệt**;
> toàn bộ gói quyết định `Q1..Q7` sau khép Đợt `W7a` (`KT-417`/`KT-413c`/`KT-408c` `DONE`, T2 PASS 81/81 @ `0f26bebc`)
> đã được Agent 0 **CHỐT DỨT ĐIỂM** (`D2-262`), mở khoá ngay Đợt **`W7b`**: **`KT-417b`**, **`KT-418c`** và
> **`KT-408d`** (`READY`, 3 slot song song 0 % chồng tệp). Đóng gói `Cap` `56→32 B` (−12 KiB) để bảng VM nằm
> hạng nhất trong `KernelStore` (không dùng global `static`, không nới pin), giữ trần 25.000 dòng sản phẩm
> `vifast-core`, tách sạch `KT-417b`/`417d`, `KT-420`/`420b`, `KT-423`/`423b`, và chỉ thị Lead đóng điểm mù
> 153 ca host của `step_l0_kernel` (`-p vifast-core -p vifast-ktest`).
> Toàn bộ chương trình Đại tu Kiến trúc nằm tại [`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md)
> (**46 task** trong dải `KT-400..KT-424`: **32 DONE**, **3 READY**, **11 BLOCKED** thuần theo DAG).
> Toàn bộ mục lịch sử vòng 1 → vòng 25 nằm nguyên văn tại
> [`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) — **đọc-only**.

## 1. Mục BLOCKED kỹ thuật đang mở

**Không có chặn thiết kế hoặc chặn chờ duyệt** (toàn bộ 11 task `BLOCKED` trong `QUEUE.md` chỉ chờ phụ thuộc DAG: `KT-417d`, `KT-417c`, `KT-419`, `KT-419b`, `KT-420`, `KT-420b`, `KT-421`, `KT-422`, `KT-423`, `KT-423b`, `KT-424`).

## 1b. Yêu cầu `Q1..Q7` (2026-11-26) — ĐÃ CHỐT 100 % Ở `D2-262`

| # | Yêu cầu | Phán quyết Agent 0 (`D2-262`) | Task sở hữu |
|---|---|---|---|
| **Q1** | Tách `KT-417b` (6,5–7,5 h > trần 3 h) | **ĐÃ CHỐT**: từ chối mã sai mẫu `KT-417b-1`/`KT-417b-2`; tách đúng regex `^KT-[0-9]+[a-z]?$` thành **`KT-417b`** (≤ 2,5 h, W7b) → **`KT-417d`** (≤ 3 h, W8a) | **`KT-417b`** (`READY`) → **`KT-417d`** |
| **Q2** | Vị trí bảng VM (dư 120 B / trần `184.832 B`) | **ĐÃ CHỐT (Phương án C, `D2-84`/`KT-312`)**: **từ chối** cả (a) global `static` `.corespace` (phá `Copy`/multi-cluster `S3`/`UntypedStore`) lẫn (b) giảm `MAX_MAPPINGS`. Đóng gói `Cap` `56→32 B` (**−12.288 B**) ở `KT-417b`, rồi đặt bảng VM làm **kiểu liên kết hạng nhất của `KernelStore`** ở `KT-417d` (`~176 KiB ≪ 184.832 B`) | **`KT-417b`** → **`KT-417d`** |
| **Q3** | Trần 25.000 LOC `vifast-core` (QD §3.15) | **ĐÃ CHỐT (KHÔNG nâng trần)**: giữ nguyên mục tiêu **25.000 dòng sản phẩm `[ƯL]`** loại trừ `*_tests.rs`/`test_lock.rs`/`#[cfg(test)]` (đúng `ViFastOs-v2-thiet-ke.md:700`; hiện `17.128` dòng sản phẩm, dư > 5.400 dòng); R1 ≤ 1.500/tệp giữ cứng | Nguyên tắc chung (`D2-262`) |
| **Q4** | Gia cố `[boot_caps]` & `spec/abi.toml` (F2/F3/F5/F6) | **ĐÃ CHỐT**: F2a/F2b/F3/F5 đã đóng ở `0f26bebc` (`161ed30`, selftest `19/19`, `boot_caps_layout.rs` 66 B); đóng nốt **F6** (`errors = [...]` cho 13 chữ ký mới + kiểm máy `check_abi.py`) trong cửa sổ `spec/abi.toml` của `KT-417d` | **`KT-417d`** |
| **Q5** | TAP trên ảnh thật cho Notification chết (F1) | **ĐÃ CHỐT**: duyệt thêm 1 TAP QEMU thật chứng minh `CNode.Revoke` trên `Untyped` chủ gỡ `bound_notification` của TCB & tái bind an toàn sau tái dùng `ObjId` | **`KT-419`** |
| **Q6** | Đột biến cho 4 method mới + assert 4 delta `ktest_inv` | **ĐÃ CHỐT**: assert đủ 4 delta `mutated`/`rotated`/`bound`/`unbound` trong `ktest_inv.rs::ok18` + thêm 2 đột biến `core-host` vào `mutants.toml` ⇒ `KILLED` | **`KT-417b`** (`READY`) |
| **Q7** | `persist.sh --crash` không tất định (`records=13`) | **ĐÃ CHỐT**: duyệt mở rộng `PL75` (`MODE_CRASH_MID_WRITE = 3` + `crash_after_writes`) trong `ktest_persist.rs` + `persist_host.py` + `persist.sh` (halt không flush giữa pha 7, bỏ poll `sleep 0.05`, luôn kiểm `partial=1` + replay + dọn tệp dở) | **`KT-408d`** (`READY`) |

## 2. Nợ kiến trúc còn lại (đều đã gán task có chủ trong `D2-259`/`D2-261`/`D2-262`)

| Nợ | Trạng thái hiện tại | Task sở hữu (`D2-262`) |
|---|---|---|
| 2 Object/Cap Layer tách đôi (`vifast-core` vs `plat-common::obj` 2.424 LOC / 33 caller) | `KT-417` DONE; tách sạch 2 bước (`Cap` 56→32 B + dời `vm/` rồi nhập `KernelStore` + xoá `[limits.plat]`/`core_foreign.rs`) | **`KT-417b` (`READY`) → `KT-417d`** |
| Điểm mù `step_l0_kernel` bỏ sót `vifast-core` (137) + `vifast-ktest` (16) = 153 test host | Phát hiện kiểm toán Agent 0 (`v2/bin/lib/steps_gate.sh:8-12`); chạy thực tế 0,18 s | **Lead ngay (`D2-253`/`D2-262`)** |
| Lỗi `-cpu max` `VSpace` isolation (`rflags=0x10202 [TF=1]`) + `hardening-max.sh` + `a64 fpu.rs` host | Mở ngay sau `KT-413c` DONE (0 chồng tệp); **không** nới ratchet `B1.1` | **`KT-418c` (`READY`)** |
| Flake `persist.sh --crash` do poll `sleep 0.05` (`records=13` dưới tải T2) | Duyệt nâng cấp tất định `MODE_CRASH_MID_WRITE` ở `D2-262` | **`KT-408d` (`READY`)** |
| `x86_64` GDT `planned` + `syscall`/`sysret` (giữ `int 0x80` & `7C-r13`) + nợ phủ preempt CPL3 (`KT-413c` §6.1) | Chạy W8b song song `KT-419` (chỉ chạm `spec/kernel.toml`) | **`KT-417c`** |
| Driver & dịch vụ còn trong kernel (`B1`: 3 dep `vifast-net`, 23 tệp driver đóng băng) | Ratchet `B1.1` giữ cứng (`≤ 23`, cấm tăng); `KT-423` đưa về 0; `KT-423b` chứng minh mở rộng NVMe 0 dòng kernel | **`KT-419` → `KT-419b` → `KT-421..KT-423b`** |
| 17 hằng `EP_*` hardcode + 8 `SLOT_*`/`LOOP_*` + 6 `.vidl` + `Tcb.Exec(elf_frame)` + `# build:` a64/x64 | Tách sạch 2 bước ≤ 3 h: `KT-420` (`.vidl`/`namesrv`/`PageSource`, 0 chạm `v2/kernel/` & `abi.toml`) → `KT-420b` (`boot_abi=2` + `Tcb.Exec` + `# build:`) | **`KT-420` → `KT-420b`** |
| Tranh chấp BKL 4 hart (`KT-415`: 0 % → 39,6 % → 93,5 %) + `HartSched::next` (`store.rs:373-380`, `ObjId > 256` General) | **Ngưỡng S3 đã kích hoạt**; tham số hoá `PerHartState`/`HartSched` + khoá Per-Hart + `HartTier2` + `ipc_fastpath` `F1` | **Hậu `KT-424` (#1 `S3`/`F1`)** |
| Nhánh chết `cfg!(feature="product-net")` ở `riscv64/build.rs:206` (`KT-404c`) | Giữ nguyên có chủ đích — biến mất hoàn toàn khi `KT-421` gỡ `net_svc`/`proc_net` | **`KT-421`** |

*(Các nợ cũ ở Chặng 0, Chặng 1 & W5–W7a đã **ĐÓNG 100 %**: dọn kho v1 `KT-400`, `BootInfo` v1 + DTB/ACPI 3 arch `KT-402/402b`, ratchet `B1.1` + `v2 mutants` git-tree thật `KT-403/408c`, workspace `v2/user/` + `vifast-kbuild` + 3 lockfile + `check-boot-abi` + `N-7` `KT-404/404c/404d`, W^X 4 KiB + `.corespace` NX 3 arch `KT-401/401b`, `CoreObserver`/`PerHartState`/Lõi Kép `KernelStore`/`UntypedStore`/gỡ `vifast-model` + `Machine::now` `KT-405/408/408b/408c/410`, UART RX IRQ + `LD_PRELOAD` `KT-406`, tooling `KT-407`, HAL + `clippy` kernel `KT-409/409b`, `vifast-ktest` + gom 14 bin `KT-411/411b`, SMP 4 hart 3 arch + `N-3` + `Tier2Mailbox` + gỡ 2.162 LOC `sched.rs` x64 `KT-412/413/413b/413c/415`, UEFI `x64`/`a64` `KT-414/414b`, `spec/` ABI sạch + `[boot_caps]` + 4 invocation + F1/F2/F3/F5 `KT-417`, FPU/SIMD + `SMEP`/`SMAP`/`PAN`/`SUM` + `EntropyPool` `KT-418/418b`.)*

## 3. Giới hạn có văn bản (không phải chặn)

- **`KT-75`** — ca âm `flush` là **mô hình cache** (QEMU ghi thẳng file ảnh), không phải mất dữ liệu thật;
  `ktest-persist` không nằm trong matrix vì một pha là hai boot trên cùng ảnh đĩa. Chi tiết: archive §KT-75,
  `v2/docs/k6-persist.md`.
- **`KT-78`** — harness QEMU phụ thuộc `perl` cho `setpgrp` (fail-closed nếu thiếu); `loadavg` chỉ là
  lưới phụ, bộ đếm QEMU mới là detector chính. Chi tiết: archive §KT-78, `v2/docs/k6-harness.md` §7.

## 4. Chuyển tiếp theo lịch (không phải chặn)

- Kế hoạch phát hành `KT-348` (`v2.0`) đã được **huỷ bỏ (`superseded`, `D2-259`)** theo chỉ đạo của người điều phối.
- Chương trình Đại tu Kiến trúc ([`KE-HOACH-KIEN-TRUC.md`](../../KE-HOACH-KIEN-TRUC.md), cập nhật `D2-262`): đã **`DONE` 32/46 task** (Chặng 0 xong 100 %, Chặng 1 xong 100 %, Chặng 2 đã xong 11/16 task, Đợt `W7b` mở sẵn 3 slot `READY`: `KT-417b`, `KT-418c`, `KT-408d`). Không còn cửa sổ nào bị treo chờ duyệt.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
