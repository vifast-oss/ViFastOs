# KT-336b — Tách `v2/sim/vifast-sim/src/kobj.rs` theo miền + mở R1 cho `sim/src/` + đồng bộ checker `wake`

**Chu kỳ 2 — Đợt 1.** Người làm: sub-agent `kobj`. Ngày: 2026-11-22 (giờ máy Asia/Tokyo).
Quyết định: **D2-232** (tách `kobj.rs` ≤ 1.500 dòng/tệp + mở R1) và **D2-238** (đồng bộ
`SIM_KOBJ` trong checker `wake`, 6 chỗ). Kế hoạch: `KE-HOACH-CHU-KY-2-2026-11-22.md` mục **KT-336b**.

| | |
|---|---|
| Worktree | `/tmp/kt336b-s1` (nhánh `task/kt336b-kobj`) |
| Base | `main` @ `9895b596` (`[KT-335] QUEUE: KT-334/336a DONE; mở KT-335/336b/337`) |
| Máy | 12 nhân (`hw.ncpu=12`), macOS, Rust/cargo `1.100.0` (ghim `v2/rust-toolchain.toml`) |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`, chạy trong `/tmp/kt336b-s1` |
| Nhãn số liệu | **`[ĐO KT-336b]`** — đo trực tiếp từ log cổng trong worktree này |

**Số dòng thực đo tại base:** `wc -l v2/sim/vifast-sim/src/kobj.rs` = **11.964** (§1). D2-232 ghi
**11.443** — số đó đo ở thời điểm soạn quyết định; từ đó `kobj.rs` đã tăng qua các task K1c/K2
(KT-237…KT-335). **Không** nới ngưỡng: mục tiêu vẫn là **≤ 1.500 dòng/tệp**.

---

## 1. Khảo sát

### 1.1 Cấu trúc `kobj.rs` trước khi tách

Tệp gồm **một** khối `impl Machine` khổng lồ (dòng 1015–10918, **367 method**, ~9.900 dòng) +
phần kiểu dữ liệu ở đầu (dòng 1–1014) + `mod tests` inline (dòng 10920–11964, 1.045 dòng).
Không có free function nghiệp vụ; toàn bộ là method inherent trên `Machine` (và impl nhỏ cho
`ObjType`, `KError`, `SlotRef`, `TcbData`, `KObj for Kind`, `Default`/`Drop`).

| Miền chức năng | Dòng gốc (khối method) | `struct`/`impl` chính | Ai gọi |
|---|---:|---|---|
| **Kiểu lõi + `Machine`** | 1–1014 | `ObjType`, `KError`, `SlotRef`, `CapInfo`, `EndpointInfo`, `QueuedView`, `NotificationValue`, `TcbInfo`, `FinaliseEntry`, `Kind`, `TcbData`, `SchedData`, `ReplyData`, `Meta`, `Cap`, `Machine` + `Default`/`Drop` | mọi miền dưới; 45 tệp test của sim; `vifast-model/tests/kt11_diff_sim.rs` |
| **object/arena** (Retype, watermark, objmem, finalise) | 1.016–1.310 + 3.560–3.720 + 10.043–10.275 | `Machine`, `Kind`, `Meta`, `Loc`, `ObjRef` (objmem) | `retype`, `caps`, `vspace`, `pagetable`, `tlb` |
| **CSpace/cap** (MDB cây dẫn xuất, copy/mint/delete/revoke) | 1.179–1.310 + 3.887–4.007 + 4.462–4.678 + 10.202–10.275 + 10.406–10.689 | `Cap`, `Machine::caps`, `collect_subtree` | IPC (chuyển cap), TCB (bind/revoke), finalise |
| **Retype** (Untyped → object, Frame/PageTable/VSpace, ASID) | 4.016–4.457 | `Machine::retype`, `FrameSize` (vifast-pte) | harness KT-36/37/38, `kobj/tcb` |
| **TCB** (configure, prio/affinity/home, suspend/resume, hàng chờ) | 4.662–5.260 + 10.280–10.404 + 10.755–10.916 | `TcbData`, `TcbInfo`, `TcbState` | scheduler, IPC, state_write, 45 tệp test |
| **Scheduler + hộp thư + migration** | 5.372–6.380 + 7.082–7.439 | `Machine::sched`, `mail`, `published`; `HartSched`, `HartMailbox`, `Published` (crate) | `state_write` (5 cửa `Ready`), IPC wake, L1 |
| **MCS/budget/donation** | 4.697–4.808 + 4.916–4.946 + 7.451–7.785 + 8.048–8.091 + 8.113–8.440 | `SchedData`, `SchedContext`, `K1bReport` | KT-20/KT-21 test, `state_write` |
| **Clock + tick ngân sách** | 7.798–8.045 | `MonotonicClock` (crate), `Machine::now` | L1, KT-152 test |
| **Frame + PageTable** (map/walk/unmap/protect, W^X) | 1.319–2.467 | `FrameMeta`, `PageTableData`, `WxRecount` | VM test KT-36/37/39/42 |
| **VSpace/ASID** (activate, flush, check_vm) | 2.478–2.855 + 3.347–3.556 + 3.735–3.873 + 6.694–6.708 | `VSpaceData`, `AsidState`, `FlushCmd`, `HartMask` | KT-38/40/42, L1 VM op |
| **TLB + shootdown** | 2.865–3.331 | `HartTlb`, `TlbEntry`, `PendingShootdown`, `TlbShootdownInfo` | `vspace` unmap/protect, L1 |
| **IPC** (Endpoint/Reply, transfer cap, Send/Recv/Call/Reply) | 6.392–7.070 + 8.932–9.933 + 10.707–10.750 | `EndpointState`, `QueuedMessage`, `Message`/`IpcArea` (crate `ipc`) | 20+ test IPC, L1, `state_write` |
| **Notification + WaitOnAddress** | 6.725–6.927 + 8.447–8.925 | `NotificationState`, `address_waiters`, `addr_words` | KT-27/28 test, L1 |
| **Fault** (Exit/Inject, lỗi cuối) | 6.587–6.672 + 9.949–10.035 | `FinaliseEntry`, `FaultKind` | KT-26/41 test, `procsrv` |
| **Tests nội bộ** | 10.920–11.964 | `mod tests` (67 test) | `cargo test -p vifast-sim --lib` |

### 1.2 Sáu chỗ `SIM_KOBJ` trong `check-wake-paths.py` (trước khi sửa)

| # | Dòng | Dùng để làm gì | Sau khi sửa |
|---:|---:|---|---|
| 1 | 267 | Hằng `SIM_KOBJ = "v2/sim/vifast-sim/src/kobj.rs"` | `SIM_KOBJ_ROOT` (root module) + **`SIM_KOBJ_FILES`** = 14 tệp sản phẩm (`kobj.rs` + 13 miền; **không** gồm `state_write.rs` — nơi `transition_tcb` **phải** nằm) |
| 2 | 339 | `LEGACY_TRANSITION_FILES = (KERNEL_TCB, SIM_KOBJ)` — kiểm **không** tệp nào còn `fn transition_tcb(` | `(KERNEL_TCB, *SIM_KOBJ_FILES)` — kiểm **mọi** tệp miền |
| 3 | 3746 | `bc_files = (sw, SIM_KOBJ)` — mỏ neo `block_current` phải là `NonReadyTarget` ở tệp **định nghĩa** API | `(sw, *SIM_KOBJ_FILES)` — `block_current` nay ở `kobj/sched.rs` |
| 4 | 4250 | `anchors = { SIM_KOBJ: ("fn wake(", ()) }` — bắt buộc tệp có `fn wake(` | Bỏ khỏi `anchors`; kiểm **hợp** cả `SIM_KOBJ_FILES` (đúng một tệp có `fn wake(` là đủ, nay ở `kobj/sched.rs`; không tệp nào ⇒ `E_ANCHOR_MISSING`) |
| 5 | 4952 | `_fixture_v2`: `_write(base / SIM_KOBJ, …)` dựng cây giả cho selftest | `base / SIM_KOBJ_ROOT` |
| 6 | 10188 | Selftest `in_test_only_wake_scope("v2/sim/vifast-sim/src/kobj.rs")` — kiểm tệp thuộc phạm vi bảo vệ | `.../kobj/sched.rs` (chứng minh **tệp miền mới** cũng thuộc phạm vi token; phạm vi tính theo crate) |

Cả 6 chỗ đều là **so path/đọc text** — không chỗ nào parse vị trí dòng, nên đổi sang danh sách tệp
không làm mất phép kiểm nào. `check-wake-depinfo.py` **không** ghim đường dẫn `kobj.rs` (nó dùng
`cargo metadata` + dep-info); chỉ một dòng chú thích nhắc `SIM_KOBJ` — đã cập nhật thành
`SIM_KOBJ_FILES`. `CHECKER-WAKE-CONTRACT.md` cập nhật hàng mỏ neo #7 + ghi chú KT-336b.

---

## 2. Tách theo miền (kết quả)

`kobj.rs` giữ vai trò **root module** (kiểu dữ liệu lõi + `Machine` + `mod` khai báo + re-export
cũ), các miền là **module con** `kobj/<miền>.rs` — cùng mẫu đã có với `kobj/state_write.rs`
(KT-241). Mỗi tệp `use super::*;` + `impl Machine { … }`; hàm nội bộ đổi `fn` → `pub(super) fn`
(phạm vi nhìn thấy **không đổi**: `kobj` + hậu duệ, như cũ). Thân hàm **nguyên văn**.

| Miền | Tệp mới | Dòng | Hàm |
|---|---|---:|---:|
| object/arena/store/finalise | `kobj/object.rs` | 661 | 39 |
| CSpace/cap | `kobj/caps.rs` | 815 | 28 |
| Retype | `kobj/retype.rs` | 460 | 8 |
| TCB | `kobj/tcb.rs` | 737 | 27 |
| Scheduler/hộp thư/migration | `kobj/sched.rs` | **1.402** | 47 |
| MCS/budget/donation | `kobj/budget.rs` | 898 | 30 |
| Clock/tick | `kobj/clock.rs` | 270 | 14 |
| Frame + PageTable | `kobj/pagetable.rs` | 1.167 | 29 |
| VSpace/ASID | `kobj/vspace.rs` | 814 | 27 |
| TLB/shootdown | `kobj/tlb.rs` | 486 | 26 |
| IPC | `kobj/ipc_ops.rs` | 1.351 | 61 |
| Notification/WaitOnAddress | `kobj/notification.rs` | 629 | 25 |
| Fault | `kobj/fault.rs` | 376 | 6 |
| Test nội bộ | `kobj/tests.rs` (**`#[cfg(test)] mod tests;`**) | 1.045 | — |
| Root (kiểu + `mod`) | `kobj.rs` | **1.030** | — |
| (đã có từ KT-241) | `kobj/state_write.rs` | 330 | — |

- **`kobj.rs`: 11.964 → 1.030 dòng** (giảm 91,4 %). Mọi tệp **≤ 1.402 dòng** (< 1.500).
- **Tổng tệp `v2/sim/vifast-sim/src/`**: 16 → **30** (15 tệp phẳng + 15 tệp trong `kobj/`).
- Tổng dòng sau tách: 12.471 (+177 dòng do 13 header SPDX/`//!` + `use super::*;` + `impl` mở/đóng
  — **không** dòng logic nào bị đổi).
- `mod tests` inline (1.045 dòng) chuyển thành `kobj/tests.rs` để root ≤ 1.500; `#[cfg(test)]` giữ
  nguyên nên checker vẫn nhận là **test-only** (đồ thị module), và 67 test vẫn chạy dưới tên
  `tests::…`.
- **Không** đổi API công khai: kiểu/const/re-export vẫn ở root `kobj`; method inherent dù khai báo
  trong module con vẫn là method của `Machine`. Không thêm/bớt method nào (367 → 367).

---

## 3. Mở R1 phủ `v2/sim/vifast-sim/src/`

`v2/bin/v2`: thêm `R1_DIRS="kernel lib sim/vifast-sim/src"` và vòng R1 dùng `$R1_DIRS` thay
`kernel lib`; cập nhật dòng help. **Giữ nguyên ngưỡng 1.500**; `-name target -prune` giữ nguyên.
Phạm vi **có chủ ý** là `src/` (không phải toàn bộ `v2/sim`): `v2/sim/vifast-sim/tests/` là
harness kiểm, ngoài phạm vi luật cấu trúc sản phẩm (cùng lý do `kernel/*/tests/` ngoài R1).

Kiểm không làm cổng đỏ vì tệp khác: quét `sim/vifast-sim/src` tại base chỉ có `kobj.rs` > 1.500;
các tệp còn lại đều < 1.200 dòng. Sau tách: **không tệp nào** > 1.500. `v2 rules` PASS.

---

## 4. Bằng chứng hành vi **không đổi** — L1 1.000 seed (bắt buộc)

Chạy **trước** khi sửa (base `9895b596`, cây sạch) và **sau** khi tách:

```text
$ bash v2/tools/l1-tier.sh --tier smoke          # TRƯỚC (log: /tmp/kt336b-logs/l1-truoc.log)
== gộp ==
 smp     seed  ipc_min   ipc_sent  delivered   dropped     map   walk  unmap  fault  shoot             digest
   1     1000        4      12245       2143      8042    2614    700   1342   2349   1342 237f3fa69d2bf1ea
== wall 7 s (0 m 7 s), 1 cấu hình smp × 22 lô = 22 task ==
== l1-tier: PASS (mọi lô xanh, phủ đủ seed) ==
```

```text
$ bash v2/tools/l1-tier.sh --tier smoke          # SAU (log: /tmp/kt336b-logs/l1-sau.log)
== gộp ==
 smp     seed  ipc_min   ipc_sent  delivered   dropped     map   walk  unmap  fault  shoot             digest
   1     1000        4      12245       2143      8042    2614    700   1342   2349   1342 237f3fa69d2bf1ea
== wall 6 s (0 m 6 s), 1 cấu hình smp × 22 lô = 22 task ==
== l1-tier: PASS (mọi lô xanh, phủ đủ seed) ==
```

**Cùng 1.000 seed, cùng `trace_hash` từng seed ⇒ digest XOR `237f3fa69d2bf1ea` y hệt**, và mọi bộ
đếm tổng (`ipc_sent=12245`, `delivered=2143`, `dropped=8042`, `map=2614`, `walk=700`,
`unmap=1342`, `fault=2349`, `shoot=1342`) trùng khít. Đây là **bằng chứng bắt buộc** rằng tách
module không đổi hành vi mô phỏng.

Chạy thêm `./v2/bin/v2 nightly --l1 1000` (single-process, driver chạy **2 lần/seed**):
`--- nightly 1: PASS (79s; …step-1-l1.log)` — cùng 1.000 seed, "cùng seed ⇒ cùng trace" PASS.

---

## 5. Hai đột biến (chứng minh cổng có răng)

### (i) Module vượt 1.500 dòng ⇒ `v2 check` ĐỎ (R1 có răng trên `sim/src/`)

Thêm 1.400 dòng comment vào `kobj/clock.rs` (270 → 1.672 dòng), chạy `./v2/bin/v2 check`:

```text
R1 v2/sim/vifast-sim/src/kobj/clock.rs: 1672 dòng > 1500
v2 check: 1 bước FAIL
```

`rc=1`. Khôi phục nguyên văn (sha256 khớp bản trước đột biến), `v2 rules` xanh lại.

### (ii) Đổi một hằng số mô phỏng ⇒ `l1-tier smoke` ĐỎ (L1 có răng)

Đổi `DEFAULT_ARENA_BYTES` (hằng số mô hình, `kobj.rs`) từ `1 << 20` → `1` rồi chạy L1 smoke:

```text
thread 'driver_smp_cung_seed_cung_trace' panicked at sim/vifast-sim/src/kobj/object.rs:539:14:
arena kernel còn chỗ (tăng bằng Machine::with_arena_bytes)
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; …
```

```text
== l1-tier: FAIL (22 lỗi) ==
   - smp1-lo0cua22.log: không có 'test result: ok'
   …
   - smp 1: phủ 0/1000 seed — sót hoặc trùng lô
```

`rc=1`. Khôi phục `kobj.rs` (sha256 khớp bản trước đột biến); KHÔNG commit trạng thái đột biến.

---

## 6. Cổng đã chạy

| Cổng | Kết quả | Ghi chú |
|---|---|---|
| `cargo clippy --manifest-path v2/Cargo.toml --workspace --all-targets -- -D warnings` | **PASS** (rc 0) | 12,20 s |
| `cargo test --manifest-path v2/Cargo.toml -p vifast-sim` | **PASS** (rc 0) | toàn bộ test lib + integration; gồm `l1_1000_seed_tat_dinh`, `driver_smp_cung_seed_cung_trace` (29,37 s) |
| `cargo test --manifest-path v2/Cargo.toml -p vifast-model` | **PASS** (rc 0) | `kt11_diff_sim` (parity `vifast-model` ↔ sim) PASS |
| `./v2/bin/v2 check` | **PASS** (rc 0) | `v2 check: PASS (L1 chạy trong gate --fast)`; log `/tmp/kt336b-logs/check2.log` |
| `python3 v2/tools/check-wake-paths.py` | **PASS** (`errors=0 warnings=0`) | log thấy `kênh Wake … kobj/sched.rs:316/338/352` |
| `python3 v2/tools/check-wake-paths.py --strict` | **PASS** (`errors=0 warnings=0`) | |
| `python3 v2/tools/check-wake-paths.py --selftest` | **PASS** (`452/452 ok`) | |
| `python3 v2/tools/check-wake-depinfo.py --root <wt>` | **PASS** (`errors=0 warnings=0`, 32,67 s) | 243 tệp |
| `python3 v2/tools/check-wake-depinfo.py --selftest` | **PASS** (`70/70 ok`) | |
| `bash v2/test/rules/run.sh` | **PASS** (6/6) | fixture R1..R5 |
| `./v2/bin/v2 rules` | **PASS** | E1 NTFS PASS, R1..R5 sạch |
| `./v2/bin/v2 nightly --l1 1000` | **PASS** (79 s) | 1 bước |
| `bash v2/tools/l1-tier.sh --tier smoke` (trước/sau) | **PASS** cả hai, **cùng digest** | §4 |

**Ghi chú `v2 check` lần đầu:** bản nháp ghi chú trong `CHECKER-WAKE-CONTRACT.md` chứa số đo viết
tay (`11.964`, `1.500`, `14`) ⇒ bước `luật máy K6` (verify-numbers) **ĐỎ** `E1_NUMBER_OUTSIDE`.
Đã sửa ghi chú (bỏ số đo, dùng "trần R1"/"mọi tệp miền") ⇒ `v2 check` PASS. Đây là cổng **bắt
đúng**, không phải nới ngưỡng.

---

## 7. Ghi chú kỹ thuật / rủi ro đã kiểm

- **Vì sao `use super::*` đủ:** module con của `kobj` thấy item private của `kobj` (kể cả field
  `Machine` và các `use` nội bộ như `ipc`), đúng mẫu `state_write.rs` đã dùng từ KT-241. Hàm nội
  bộ cần gọi chéo miền đổi `pub(super)` ⇒ phạm vi nhìn thấy **y như** `fn` private cũ (chỉ `kobj`
  + hậu duệ), không lộ ra crate ngoài.
- **Tên module:** miền IPC đặt `ipc_ops.rs` (không `ipc.rs`) vì root đã có `use crate::ipc::{self,
  …}`; trùng tên sẽ là lỗi "defined multiple times".
- **`kobj/state_write.rs` không đổi** và **không** nằm trong `SIM_KOBJ_FILES` (nó là nơi
  `transition_tcb` **phải** nằm); mọi hàm/mỏ neo của nó vẫn qua checker như cũ.
- **Không** chạm `spec/`, `v2/kernel/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`. `STATUS.md` do
  `v2 nightly` sinh đã `git checkout` hoàn nguyên (không commit).
- **Không** push/merge; commit trên nhánh `task/kt336b-kobj` theo D2-235(1).

## 8. Tệp đã đổi / thêm

- Sửa: `v2/sim/vifast-sim/src/kobj.rs`, `v2/bin/v2`, `v2/tools/check-wake-paths.py`,
  `v2/tools/check-wake-depinfo.py`, `v2/tools/wake_scope.py`,
  `v2/agent/reports/CHECKER-WAKE-CONTRACT.md`.
- Thêm: 13 tệp miền `v2/sim/vifast-sim/src/kobj/{object,caps,retype,tcb,sched,budget,clock,pagetable,vspace,tlb,ipc_ops,notification,fault}.rs`,
  `v2/sim/vifast-sim/src/kobj/tests.rs`, và journal này.
