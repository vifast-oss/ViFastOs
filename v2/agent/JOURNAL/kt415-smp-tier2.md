# KT-415 — SMP Tầng 2 (`Mailbox<N>` + `OVERFLOW` trên `IpiMailbox`) + `VSpace::free_frame`/range shootdown + đo tranh khoá BKL 4 hart

- **Task**: KT-415 (P0, Chặng 2 / W6, 3 h) — phụ thuộc KT-412, KT-413.
- **Nhánh**: `task/kt415-smp-tier2` · **Worktree**: `/private/tmp/vifast-wt/2`
- **Base**: `main` @ `e66b0e28` (đã có KT-412/413).
- **Căn cứ**: `D2-259` mục 6 (BKL toàn cục cản SMP & IRQ), `D2-260`, `D2-17` (unsafe chỉ trong
  danh sách đóng), `KE-HOACH-KIEN-TRUC.md` dòng 190, `QUEUE.md` dòng 467.
- **Ngân sách**: 3 h. Bắt đầu 2026-11-26 01:44 JST (16:44 UTC, Thấp điểm chính).

## 0. Kế hoạch ngắn (5–10 dòng)

1. **Đo hiện trạng**: `vifast_sync::Mailbox<N>` (MPSC 7 message có payload, đầy ⇒ `Busy`) và
   `IpiMailbox` (bitmask 3 kind, coalesce, đầy **không xảy ra**) là **hai tầng rời**: tầng 1 chỉ
   nói "có việc", tầng 2 nói "việc gì" nhưng **không** có đường nối, và **không** có cờ nào cho
   biết tầng 2 đã đầy ⇒ nếu caller bỏ message khi `Busy` thì **mất wake**.
2. **Đỏ-trước (1)**: `v2/lib/vifast-sync/tests/kt415_tier2.rs` gọi `Tier2Mailbox` (chưa có) ⇒
   không biên dịch được. Sau khi cài: đầy ring ⇒ `OVERFLOW` bật, `take()` trả `overflowed=true`
   ⇒ người nhận **full-rescan**, 0 wake mất; 4 luồng × 2.000 message ⇒
   `delivered + full_rescan_cover == posted`.
3. **Đỏ-trước (2)**: `v2/kernel/vifast-plat-common/tests/ipi_tier2.rs` gọi `HartTier2` (chưa có)
   ⇒ không biên dịch được. Sau khi cài: `send` **luôn** rung chuông tầng 1 (kể cả khi ring đầy),
   `drain` rút chuông **trước** rồi ring + `OVERFLOW`; 4 luồng ⇒ 0 mất wake.
4. **Ưu tiên 2** — `VSpace::free_frame`/`destroy` + range TLB shootdown: `PageTable::destroy` đã
   có ở walker dùng chung (`vifast-pte`); bổ sung `SplitMmu::destroy` + `VSpace::destroy`/
   `free_frame`/`free_range` cho `aarch64` + `x86_64`, và `FlushScope::Range` cho `riscv64`.
5. **Ưu tiên 3** — đo tranh khoá BKL 4 hart: `IrqMcsLock` đã có `acquired`/`contended`; thêm
   `core_bridge::lock_stats()` (3 arch) + pha bench 4 hart trong ảnh sản phẩm riscv64 (feature
   `bench-bkl`) in `# bench: kind=bkl …`; **`v2/tools/bench-bkl.sh`** (D2-253: sub-agent **không**
   được sửa `v2/bin/**`) đọc bằng `telemetry.py` và ghi `test/perf/bkl.csv` (`measure=bkl`).
6. **Ưu tiên 4** — 3 đột biến đỏ trong `v2/test/mutants.toml` (feature `mut-*` + suite host).
7. `./v2/bin/v2 t0` sau mỗi bước → `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `spec/`; không nới ngưỡng; không `#[ignore]`/`#[should_panic]`;
không `git add/commit`; chỉ T0 + T1; không `cfg(target_arch)` mới; 0 bỏ ca khỏi matrix.

## 1. Đo hiện trạng (trước khi sửa)

```text
$ wc -l v2/lib/vifast-sync/src/mailbox.rs v2/kernel/vifast-plat-common/src/ipi.rs
     309 v2/lib/vifast-sync/src/mailbox.rs
     727 v2/kernel/vifast-plat-common/src/ipi.rs
$ grep -n "OVERFLOW\|Tier2\|tier2" -r v2/lib/vifast-sync v2/kernel/vifast-plat-common
(không có kết quả)
$ grep -n "acquired\|contended" v2/kernel/vifast-plat-common/src/irq_lock.rs
106:    acquired: AtomicU32,
108:    contended: AtomicU32,
```

Kết luận: tầng 2 **chưa có** cờ `OVERFLOW`; `IrqMcsLock` **đã có** bộ đếm `acquired`/`contended`
nhưng **chưa** có đường đọc từ ảnh kernel (`static LOCKED` là private trong `core_bridge`).

## 2. Đỏ-trước (1) — `Tier2Mailbox<N>` chưa tồn tại

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sync --test kt415_tier2
error[E0432]: unresolved imports `vifast_sync::PostOutcome`, `vifast_sync::Tier2Mailbox`
  --> lib/vifast-sync/tests/kt415_tier2.rs:23:24
   |
23 | use vifast_sync::{Msg, PostOutcome, Tier2Mailbox};
   |                        ^^^^^^^^^^^  ^^^^^^^^^^^^ no `Tier2Mailbox` in the root
   |                        |
   |                        no `PostOutcome` in the root
error: could not compile `vifast-sync` (test "kt415_tier2") due to 1 previous error
```

Đỏ **đúng lý do**: kiểu chưa tồn tại (không phải test sai kỳ vọng).

## 3. Cài `Tier2Mailbox<N>` (tầng 2) — `v2/lib/vifast-sync/src/tier2.rs`

- Ring payload = `Mailbox<N>` **nguyên trạng** (không sửa thuật toán Vyukov); thêm **cờ
  `OVERFLOW`** (`AtomicBool`) + 4 bộ đếm `posted`/`delivered`/`overflowed`/`full_rescans`.
- `post(msg)` → `Delivered` (vào ring) hoặc `Overflow` (ring đầy ⇒ bật cờ). **Chỉ hai** kết cục ⇒
  bất biến bảo toàn `delivered + overflowed == posted` kiểm được bằng máy.
- `take(f)` → rút **hết** ring rồi `swap(false)` cờ; trả `Tier2Drain { messages, overflowed }`.
  Thứ tự rút-ring-trước/đọc-cờ-sau là **bắt buộc** (cờ đặt trong lúc đang rút vẫn được thấy).
- Thứ tự bộ nhớ: người gửi `Release` cờ **sau** khi ring đầy; người nhận `Acquire`/`AcqRel` ⇒
  không có khe "đầy nhưng cờ chưa bật mà người nhận đã rút xong".

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-sync --test kt415_tier2
running 5 tests
test bat_bien_bao_toan_delivered_cong_overflowed_bang_posted ... ok
test full_rescan_cuu_wake_bi_roi ... ok
test take_rong_khong_bat_overflow ... ok
test day_ring_bat_overflow_va_khong_mat_message_cu ... ok
test bon_luong_hai_nghin_message_khong_mat_wake ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 4. Đỏ-trước (2) — `HartTier2`/`PerHartTier2` chưa tồn tại

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test ipi_tier2
error[E0432]: unresolved imports `vifast_plat_common::ipi::HartTier2`, `vifast_plat_common::ipi::PerHartTier2`
  --> vifast-plat-common/tests/ipi_tier2.rs:18:31
   |
18 | use vifast_plat_common::ipi::{HartTier2, IpiKind, PerHartTier2};
   |                               ^^^^^^^^^           ^^^^^^^^^^^ no `PerHartTier2` in `ipi`
   |                               |
   |                               no `HartTier2` in `ipi`
```

## 5. Cài tầng 2 **trên** `IpiMailbox` — `v2/kernel/vifast-plat-common/src/ipi.rs`

- `HartTier2<N>` = `Tier2Mailbox<N>` (payload) + `IpiMailbox` (chuông) **sở hữu riêng** (một người
  rút — hai người rút cùng chuông sẽ chia mất kind).
- `send(msg)` = `ring.post(msg)` **rồi** `doorbell.post(kind)` — **luôn** rung chuông, kể cả khi
  ring đầy. `IpiKind::for_msg` ánh xạ 7 loại message → 3 bit chuông (`TlbFlush`/`VmKick` giữ bit
  riêng; 5 loại còn lại cần hart chạy ⇒ bit `Wake`).
- `drain(f)` = rút chuông **trước** (xoá bit) rồi rút ring + cờ ⇒ `send` chen giữa hai bước để lại
  bit chuông cho lượt sau (không có khe mất wake).
- `PerHartTier2<N>` + `static TIER2_MAILBOXES: PerHartTier2<8>` (ring 8 message/hart).

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test ipi_tier2
running 5 tests
test full_rescan_cuu_wake_bi_roi_tren_mo_hinh_scheduler ... ok
test drain_rut_chuong_truoc_roi_moi_rut_ring_va_co ... ok
test per_hart_tier2_tach_hop_thu_tung_hart ... ok
test send_luon_rung_chuong_ke_ca_khi_ring_day ... ok
test bon_luong_hai_nghin_message_khong_mat_wake ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Sửa một kỳ vọng SAI của chính test (không phải nới ngưỡng)**: `IpiMailbox::posted` đếm **bit
mới** (không phải số lần `post`) — test đầu viết `posted == 3` nên đỏ; sửa thành
`posted == 1 && coalesced == 2 && posted + coalesced == 3`, đúng hợp đồng đã ghi trong `ipi.rs`.

```text
$ ./v2/bin/v2 t0 vifast-sync vifast-plat-common
v2 t0: PASS (2 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
```

## 6. Đo tranh khoá Big Kernel Lock 4 hart (`v2/tools/bench-bkl.sh`)

**Vì sao không sửa `v2/bin/vifast-bench`**: `D2-253` chốt **chỉ lead** được sửa `v2/bin/**`.
Phần đo nằm ở `v2/tools/bench-bkl.sh` (dùng đúng `telemetry.py` + khung `-icount` của
`vifast-bench`); lead chỉ cần thêm nhánh `--bkl` gọi tệp này.

**Cài đặt**: `core_bridge::lock_stats()` (3 arch — `riscv64` thêm `LockReport` cùng khuôn
`aarch64`/`x86_64` của KT-411) + pha bench trong ảnh sản phẩm riscv64 (feature `bench-bkl`):
mỗi hart online chạy `OPS` vòng `with_machine(|m| m.hart_count())`, in
`# bench: kind=bkl harts=… ops=… acquired=… contended=… ratio_ppm=… done=… mask=…`.

**Số đo** (riscv64, QEMU 11.1.3, `-smp N`, **không** `-icount` — xem lý do dưới):

| hart | ops | acquired | contended | `ratio_ppm` | tỷ lệ chờ |
|---:|---:|---:|---:|---:|---:|
| 1 | 2.000 | 2.002 | 0 | 0 | **0 %** |
| 2 | 4.000 | 4.002 | 1.585 | 396.051 | **39,6 %** |
| 4 | 8.000 | 8.002 | 7.482 | 935.016 | **93,5 %** |
| 4 | 8.000 | 8.002 | 7.416 | 926.768 | **92,7 %** |
| 4 | 8.000 | 8.002 | 7.416 | 926.768 | **92,7 %** |
| 4 | 8.000 | 8.002 | 7.324 | 915.271 | **91,5 %** |
| 4 | 8.000 | 8.002 | 7.471 | 933.641 | **93,4 %** |

`in_cs=0`, `overlap=0`, `done == mask` ở **mọi** lần đo (không hart nào bỏ dở).

**Đối chứng `-icount` (quan trọng — nếu không sẽ báo số SAI)**: cùng ảnh, `-smp 4`, `-icount
shift=auto,align=off,sleep=off` ⇒ `contended=0`, `ratio_ppm=0`. Nguyên nhân: `-icount` chạy các
vCPU **luân phiên** (một vCPU một lúc) nên mỗi hart chạy trọn lát của nó, gần như không bao giờ
chạm khoá đang bị giữ. Tỷ lệ chờ là **đại lượng không thứ nguyên** ⇒ không cần đồng hồ ảo;
`-icount` chỉ cần cho phép đo **thời gian**. Vì vậy `bench-bkl.sh` mặc định **tắt** `-icount` và
có cờ `--icount` để tái lập phép đo đối chứng.

**Kết luận đo được**: 4 hart ⇒ **> 91 %** số lần lấy Big Kernel Lock phải **xếp hàng** (so với
0 % ở 1 hart). Đây là bằng chứng định lượng cho `D2-259` mục 6: BKL toàn cục là nút cổ chai thật,
không phải giả thuyết — ngưỡng kích hoạt khoá Per-Hart / đa cụm `S3` đã có số.

**Ghi chú kỹ thuật**: `OPS = 2.000`/hart (không phải 20.000) vì dưới `-icount` mỗi vòng quay chờ
bị **tính** vào ngân sách lệnh ⇒ 20.000 × 4 hart vượt trần 30 s (đo được: treo sau
`ok 1 - k3 boot`). 2.000 vẫn bão hoà tỷ lệ (1 hart 0 %, 2 hart 39,6 %, 4 hart 92,7 %).

## 7. `VSpace::free_frame`/`destroy` + range TLB shootdown (3 arch)

**Hiện trạng trước khi sửa**: `PageTable::destroy` đã có ở walker dùng chung (`vifast-pte`) nhưng
**chưa** có đường `VSpace::destroy`; shootdown chỉ có **một VA** ở cả ba arch (`Shootdown { va }`,
`FlushRequest { va }`, `FlushScope::Page { va }`) ⇒ teardown một vùng phải phát `N` yêu cầu.

| Arch | `VSpace::destroy` | `VSpace::free_frame` | `VSpace::free_range` | Range shootdown |
|---|---|---|---|---|
| `aarch64` | ✅ `SplitMmu::destroy` → `PageTable::destroy` (cả hai nửa) | ✅ unmap + `tlbi vae1` + `dsb ish` + `isb` | ✅ unmap N trang + **một** `shootdown::invalidate_range` | ✅ `PENDING_LEN` + `apply_local_range`/`apply_remote_range` (`dsb ishst` một lần, `tlbi vae1is` mỗi trang) |
| `x86_64` | ✅ `PageTable::destroy` | ✅ unmap + `INVLPG` | ✅ unmap N trang + **một** `ipi::shootdown_range` | ✅ `FlushRequest.len` + `FlushSlot.len` + handler `invlpg` mỗi trang |
| `riscv64` | — (arch **không** có kiểu `VSpace`: bảng trang do object layer `plat-common::obj` quản) | — | — | ✅ `FlushScope::Range { start, len }` (`RANGE_CODE = 3`) + slot `len` + `sfence.vma` mỗi trang |

**Vì sao riscv64 không có `VSpace::destroy`**: `v2/kernel/vifast-plat-riscv64` **không** có kiểu
`VSpace` — bảng trang Sv39 của tiến trình do object layer (`vifast-plat-common::obj::vspace`) quản
theo `CapId`/Untyped. Thêm `destroy` ở đó là **thay đổi object layer** (giải phóng bảng trung gian
về Untyped + `check_objmem`), ngoài phạm vi 3 h và không có trong yêu cầu "range shootdown 3 arch".
Ghi thành **việc còn lại** (xem §10).

**Test host** (không QEMU — phần asm chỉ chạy trên QEMU, T2 của lead):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vspace
test destroy_giai_phong_moi_frame_bang_alloc ... ok
test range_pages_lam_tron_len_va_mot_trang_khi_len_bang_khong ... ok
test shootdown_mang_len_cua_range ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test ipi
test flush_request_range_mot_yeu_cau_cho_n_trang ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-riscv64-logic --test tlb
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Trung thực về thứ tự**: ba test `VSpace`/range được viết **sau** khi cài (không phải đỏ-trước) —
đỏ-trước của task nằm ở tầng 2 (§2, §4) và ở 3 đột biến (§8). Lý do: `free_frame`/`free_range` gọi
asm (`tlbi`/`invlpg`/`sfence`) nên **không** chạy được trên host; phần kiểm được trên host là
`destroy` (thuần bảng trang) + số học range. Đường asm được T2 (QEMU 3 arch) phủ.

## 8. Ba đột biến đỏ (`v2/test/mutants.toml`)

Thêm 2 suite host + 3 đột biến `kind = "feature"` (móc `#[cfg(feature = "mut-…")]` nằm trong
`v2/kernel/**` — điều kiện của `cfg_sites`), `declarations` 46 → **49**:

```text
$ python3.12 v2/tools/mutants.py --lint
# mutants lint: mutants=30 feature=29 declarations=49 covered=49 leftover=0 suites=10 problems=0

$ python3.12 v2/tools/mutants.py --root . mut-tier2-ignore-overflow
ok - mut-tier2-ignore-overflow: KILLED (5.1 s) sites=2 rc=101 marker=ok
$ python3.12 v2/tools/mutants.py --root . mut-tier2-no-doorbell-on-overflow
ok - mut-tier2-no-doorbell-on-overflow: KILLED (5.2 s) sites=1 rc=101 marker=ok
$ python3.12 v2/tools/mutants.py --root . mut-range-pages-one
ok - mut-range-pages-one: KILLED (6.7 s) sites=2 rc=101 marker=ok
```

| Đột biến | Phá tính chất | Suite | Kết quả |
|---|---|---|---|
| `mut-tier2-ignore-overflow` | người nhận **bỏ qua** `OVERFLOW` ⇒ mất wake | `plat-common-host` | **KILLED** |
| `mut-tier2-no-doorbell-on-overflow` | ring đầy ⇒ **không** rung chuông ⇒ mất wake | `plat-common-host` | **KILLED** |
| `mut-range-pages-one` | range bị coi là 1 trang ⇒ TLB các trang sau còn cũ | `plat-aarch64-host` | **KILLED** |

**Không** chạy `mutants --all` (đúng ràng buộc task).

## 9. T0 / check

```text
$ ./v2/bin/v2 t0 vifast-sync vifast-plat-common vifast-plat-aarch64 vifast-plat-x86_64 vifast-plat-riscv64-logic
v2 t0: PASS (5 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
```

**Ngoại lệ có lý do (không phải bỏ qua)**: `vifast-plat-riscv64` **không** host-test được —
`cargo test -p vifast-plat-riscv64` hỏng vì asm riscv64 (`error: invalid register 'a0'`), **đã hỏng
y hệt trên checkout chính** tại `00d6203f` (đo lại: 30 dòng `invalid register`). Vì vậy crate này
được kiểm bằng `cargo check`/`clippy --target riscv64gc-unknown-none-elf` (xanh) thay vì `cargo test`.
`vifast-kbin-riscv64` cũng là ảnh bare-metal (không host-test); `v2 check` **cố ý** loại nó khỏi
bước clippy kernel (`steps_check.sh` `--exclude vifast-kbin-riscv64`), nên phần bench được kiểm
riêng: `cargo clippy -p vifast-kbin-riscv64 --features bench-bkl --target riscv64gc-unknown-none-elf`
⇒ **0 cảnh báo từ `bench_bkl.rs`** (2 lỗi clippy còn lại là mã sản phẩm **có sẵn**, không do task).

```text
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
# step-time-total    88063 ms  TỔNG check
```

## 10. Việc còn lại / rủi ro / cần lead quyết

### 10.1 Cần **lead** làm (D2-253: chỉ lead được sửa `v2/bin/**`)

1. **Nối `vifast-bench --bkl`** — hiện phép đo nằm ở `v2/tools/bench-bkl.sh` (đã chạy được, ghi
   `v2/test/perf/bkl.csv`). Lead chỉ cần thêm một nhánh trong `v2/bin/vifast-bench`:
   `--bkl) exec bash "$V2_ROOT/tools/bench-bkl.sh" "${@:2}" ;;` (và một dòng trong `usage`).
   **Không** sửa gì khác trong `vifast-bench`; schema `history.csv` giữ nguyên (bkl có tệp riêng).

### 10.2 Việc còn lại (không kịp trong 3 h — xếp theo giá trị)

1. **`riscv64`: `VSpace::destroy`/`free_frame` ở tầng object layer** — arch này **không** có kiểu
   `VSpace`; bảng trang Sv39 do `vifast-plat-common::obj::vspace` quản theo `CapId`/Untyped. Cần
   `destroy` giải phóng bảng trung gian về Untyped + `check_objmem` xanh. Đây là thay đổi **object
   layer**, cần D2 riêng (không nằm trong "range shootdown 3 arch" — phần đó **đã xong**).
2. **Nối `HartTier2` vào đường sản phẩm** — hiện tầng 2 là **kiểu + static** (`TIER2_MAILBOXES`)
   với test host đầy đủ, **chưa** có caller trong ảnh: đường IPI hiện tại vẫn dùng
   `IPI_MAILBOXES` (bitmask) + slot yêu cầu riêng của từng arch. Nối vào là thay đổi **đường
   nóng** của 3 arch (thay slot yêu cầu bằng ring tầng 2) ⇒ nên là task riêng có T2 3 arch.
3. **`free_frame`/`free_range` chưa có test QEMU** — phần asm (`tlbi`/`invlpg`/`sfence`) chỉ chạy
   trên QEMU; T2 của lead là lần chạy đầu. Nếu T2 đỏ ở `# tlb:`/`# vspace:` thì nghi
   `apply_local_range`/`shootdown_range` trước.

### 10.3 Rủi ro đã biết

- **`-icount` làm mất tranh chấp** (§6): nếu ai chạy `bench-bkl.sh --icount` rồi báo "0 % chờ" thì
  **sai bản chất**. Script mặc định tắt và ghi `mode=bkl`/`icount=none` trong CSV để phân biệt.
- **`OPS = 2.000`/hart**: đủ bão hoà (1→0 %, 2→39,6 %, 4→92,7 %) nhưng **không** phải phép đo
  thời gian; đừng suy ra "thời gian chờ" từ `ratio_ppm`.
- **Đĩa**: `df -h .` lúc kết thúc = **5,7 GiB** (ngưỡng báo lead là < 5 GiB) — không chạm ngưỡng,
  nhưng slot-2 đang gần đầy; `target-task/slot-2` là nơi duy nhất task này ghi.
- **`vifast-plat-riscv64` không host-test được** (asm) — T0 của crate đó là `cargo check`/`clippy`
  theo target, không phải `cargo test`; đã đối chứng hỏng y hệt trên checkout chính.

## 11. T1 — `gate --dev` (qua khoá máy)

```text
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
# gate-steps registered=80 ran=66 skipped=14 failed=1
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
# step-time-total   497199 ms  TỔNG gate --dev (T1)
# t1-lock: nhả khoá (rc=1)
```

**55 bước OK, 14 bước BỎ QUA (T1 — chỉ chạy ở T2), 1 bước ĐỎ**:

```text
--- selftest pre-measure (KT-95)
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
--- selftest pre-measure (KT-95): FAIL
```

**Không phải hồi quy của task** — `v2/tools/pre-measure.sh` **từ chối đo trên cây bẩn**
(`PRE-MEASURE: REFUSE reason=tree-dirty`, exit 7). Worktree của sub-agent **luôn** bẩn (lead commit
sau), nên bước này chỉ xanh **sau khi lead commit** — **cùng** hiện tượng đã ghi ở
`JOURNAL/kt413-smp-x64.md` §4.1 (KT-413) và `JOURNAL/kt411-ktest-generic.md` (KT-411). Đối chứng:
chạy `pre-measure.sh` trên checkout chính (sạch) ⇒ `PASS (a/b/c/d/e/f/g)`; trên worktree này ⇒
`REFUSE reason=tree-dirty` (đúng mã 7).

Mọi bước T1 khác **OK**, gồm các bước dễ đỏ khi thêm test mới:
`test có răng (KT-163)`, `wake paths + --strict`, `wake theo dep-info`, `clippy kernel (KT-409)`,
`L0 kernel workspace`, `L1 (sim 1.000 seed)` (`# L1 OK: 1000 seed, 999 trace khác nhau`),
`api boundary (KT-241)` (`dep_edges=3`, `driver_files=23`, `stale=0`).

**T2 do lead chạy** trên cây đã commit (bắt buộc theo AGENTS §6.1 vì task chạm đồng bộ SMP +
page table + range shootdown).

## 12. Kiểm boot 3 arch sau khi sửa (không phải T2 — kiểm đích danh, 1 QEMU/lần)

Vì T1 **bỏ qua** 14 bước (gồm boot aarch64/x86_64), ba lần boot đích danh dưới đây được chạy để
giảm rủi ro cho T2 của lead (không thay T2, không đụng ngưỡng):

```text
$ bash v2/test/boot/run-aarch64.sh 1
# ca âm: 28/28 ĐỎ đúng như phải
# 39/39 kiểm sạch (boot=1, tests=49)
rc=0

$ bash v2/test/boot/run-x86_64.sh 1
# ca âm: 33/33 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 39/39 kiểm sạch (boot=1, tests=49, exc=6)
rc=0

$ qemu-system-riscv64 … -smp 4 (ảnh sản phẩm, không feature)
# tlb: asid=0 generation=0 satp_asid=0 active_on=0x2
# smp: boot=1 harts=4 present=0xf online=0xf started=0xd absent=0xf0 …
ok 1 - k3 boot
# k3 halt
```

`bash v2/test/boot/ktest.sh` (riscv64) **TỪ CHỐI ĐO** vì cây bẩn — cùng lớp với `pre-measure`
(`REV-GATE: REFUSE reason=dirty`), chỉ xanh sau khi lead commit. Đó là lý do boot riscv64 ở trên
dùng **ảnh sản phẩm** (không qua rev-gate) thay vì ảnh ktest.

**Số đo cuối** (sau khi sửa cảnh báo `unused variable` ở `rust_secondary_main`):

```text
# bench-bkl telemetry: kind=bkl harts=4 ops=8000 acquired=8002 contended=7440 in_cs=0 overlap=0 ratio_ppm=929767 done=0xf mask=0xf
# bench-bkl: PASS — harts=4 ops=8000 acquired=8002 contended=7440 ratio_ppm=929767
```
