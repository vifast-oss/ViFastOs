# KT-411 — Crate `vifast-ktest` generic đa arch (`B5`) + gom `kbin-riscv64` 14 `[[bin]]` → 2 mode

- **Task**: KT-411 (P0, Chặng 1 / W4, 3 h) — phụ thuộc KT-408, KT-409, KT-409b.
- **Nhánh**: `task/kt411-ktest-generic` · **Worktree**: `/private/tmp/vifast-wt/1`
- **Base**: `main` @ `e495365f` (đã có KT-408/409/409b).
- **Căn cứ**: `D2-259` mục 2 (B5), `D2-260` mục 3, `D2-17` (unsafe chỉ trong danh sách đóng),
  `KE-HOACH-KIEN-TRUC.md` dòng 180 (KT-411).
- **Ngân sách**: 3 h. Bắt đầu 2026-11-25 ~22:51 JST (13:51 UTC, Thấp điểm chính).
- **Cảnh báo song song**: KT-410 đang sửa `vifast-core` + `plat-common` (UntypedStore/BootInfo
  Arena). Ràng buộc tự áp: **0 chạm** `vifast-core/**`, **0 chạm**
  `plat-common/src/untyped_store.rs`, `plat-common/src/bootinfo.rs`; mọi thay đổi ở `plat-common`
  là **tệp mới** + **một dòng** `pub mod` trong `lib.rs` (hunk tách rời để lead merge).

## 0. Kế hoạch ngắn (5–10 dòng)

1. **Đỏ-trước**: tạo khung crate `v2/kernel/vifast-ktest` + `tests/shared_suite.rs` gọi
   `vifast_ktest::core_suite::run` và `vifast_plat_common::core_bridge::CoreBridge` — cả hai
   **chưa tồn tại** ⇒ `cargo test -p vifast-ktest` **không biên dịch được** (đỏ đúng lý do).
2. **Ưu tiên 1a** — rút **hợp đồng** `CoreBridge` + `CoreReport` + hằng số pin/trần vào
   `vifast-plat-common/src/core_bridge.rs` (tệp **mới**, `forbid(unsafe_code)`); 3 crate arch
   `impl CoreBridge` bằng wrapper mỏng trên `core_bridge` sẵn có.
3. **Ưu tiên 1b** — crate `vifast-ktest` (`no_std`, `forbid(unsafe_code)`, host-testable) chứa
   **một** bộ ca dùng chung: `core_suite` (TAP 43..52 — 8/10 ca **thuần** `vifast-core`, 2 ca
   cần object layer qua `CoreBridge`) + `invariants` (bộ `check_*` core/IPC/VM/SMP).
4. **Ưu tiên 1c** — nối `kbin-riscv64/src/ktest_core.rs` gọi bộ chung; xoá bản sao cục bộ.
5. **Ưu tiên 2** — generic hoá `IrqMcsLock<T, S: IrqSave>` (nợ KT-409b, 3 bản 408/367/395 LOC)
   vào `plat-common`; 3 arch còn `IrqState` mỏng.
6. **Ưu tiên 3** — gom `kbin-riscv64` 14 `[[bin]]` → 2 mode; đo LOC trước/sau.
7. `./v2/bin/v2 t0` sau mỗi bước → `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `spec/`; không nới ngưỡng; không `#[ignore]`/`#[should_panic]`;
không `git add/commit`; chỉ T0 + T1; không `cfg(target_arch)` mới (AGENTS §4.8); 0 bỏ ca khỏi
matrix.

## 1. Đo hiện trạng (trước khi sửa)

```text
$ wc -l v2/kernel/vifast-kbin-riscv64/src/*.rs | tail -1
   27869 total
$ grep -c '\[\[bin\]\]' v2/kernel/vifast-kbin-riscv64/Cargo.toml
14
$ wc -l v2/kernel/vifast-plat-{riscv64,x86_64,aarch64}/src/irq_lock.rs
     408 v2/kernel/vifast-plat-riscv64/src/irq_lock.rs
     395 v2/kernel/vifast-plat-x86_64/src/irq_lock.rs
     367 v2/kernel/vifast-plat-aarch64/src/irq_lock.rs
    1170 total
$ wc -l v2/kernel/vifast-plat-{riscv64,x86_64,aarch64}/src/core_bridge.rs
     303 v2/kernel/vifast-plat-riscv64/src/core_bridge.rs
     270 v2/kernel/vifast-plat-x86_64/src/core_bridge.rs
     261 v2/kernel/vifast-plat-aarch64/src/core_bridge.rs
     834 total
$ wc -l v2/kernel/vifast-kbin-{riscv64,aarch64,x86_64}/src/{ktest_core,coretest,coretest}.rs
    1090 v2/kernel/vifast-kbin-riscv64/src/ktest_core.rs
     845 v2/kernel/vifast-kbin-aarch64/src/coretest.rs
    1079 v2/kernel/vifast-kbin-x86_64/src/coretest.rs
    3014 total
```

## 2. Đỏ-trước (bằng chứng, D2-05)

Khung crate `v2/kernel/vifast-ktest` + `tests/shared_suite.rs` được viết **trước** khi có
`core_suite`/`invariants`/`CoreBridge`. Lượt chạy đầu **không biên dịch được** — đỏ đúng lý do
("API chưa tồn tại"), không phải đỏ vì lỗi khác:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest
error[E0583]: file not found for module `core_suite`
  --> vifast-ktest/src/lib.rs:11:1
   |
11 | pub mod core_suite;
   | ^^^^^^^^^^^^^^^^^^^
   = help: to create the module `core_suite`, create file "vifast-ktest/src/core_suite.rs" …
error[E0583]: file not found for module `invariants`
  --> vifast-ktest/src/lib.rs:12:1
error: could not compile `vifast-ktest` (lib) due to 2 previous errors
```

Sau khi hiện thực: **6/6** ca host xanh (`cargo test -p vifast-ktest`), trong đó ca 1 chạy **toàn
bộ** bộ ca core (TAP 43..52) trên một nền tảng **giả** và đòi `fails == 0 && taps == 10`.

## 3. Việc đã làm

### 3.1 Crate `v2/kernel/vifast-ktest` (mới) — `B5`

| Tệp | LOC | Nội dung |
|---|---:|---|
| `src/lib.rs` | 153 | `CoreBridge` trait + `LinkReport`/`FrameLabel`/`ObjVmReport` + hằng pin/trần |
| `src/core_suite.rs` | 857 | **TAP 43..52** dùng chung + `Tap`/`TapLine` + `Rig`/`build_rig` + `state_name`/`err_name`/`task_ready` |
| `src/invariants.rs` | 114 | **12 nhóm** `check_*` (core/IPC/VM/SMP) — một nguồn thay ba bản |
| `tests/shared_suite.rs` | 290 | 6 ca host: chạy bộ chung trên nền tảng giả, nhãn TAP ổn định, 12 nhóm, **có răng** |

`#![no_std]`, `#![forbid(unsafe_code)]`, **0** asm/MMIO ⇒ chạy được cả trên host **và** trong ảnh
kernel (AGENTS §4.3). Vì sao `CoreBridge` ở **đây** mà không ở `plat-common`: `plat-common`
**không** phụ thuộc `vifast-core` (quyết định KT-409b giữ nó arch-neutral) mà hợp đồng này nhắc
tới `vifast_core::Machine`/`Footprint`.

### 3.2 riscv64 dùng bộ chung (xoá bản sao)

- `v2/kernel/vifast-plat-riscv64/src/ktest_bridge.rs` (**mới**, 152 LOC): `impl CoreBridge for
  Riscv64Virt` — adapter mỏng trên `core_bridge`/`obj` sẵn có; `record()` ghi 4 số chỉ `init`
  biết (base/arena/untyped/harts) vào atomic, nên **không** thêm `static mut`/`unsafe` nào.
  Toàn tệp sau `#[cfg(feature = "ktest")]` ⇒ ảnh sản phẩm **0** byte harness (D2-08).
- `v2/kernel/vifast-kbin-riscv64/src/ktest_core.rs`: **1.090 → 415 LOC (−675)**. TAP 43..52 nay
  gọi `vifast_ktest::core_suite::{link_tap, run_locked}`; TAP 53..60 của riêng riscv64 vẫn chạy
  **trong cùng** lượt giữ khoá và dùng lại `Rig` do bộ chung trả về — **đúng thứ tự cũ**, không
  đổi một nhãn TAP nào.

### 3.3 Nợ `KT-409b` — generic hoá `IrqMcsLock<T>`

`v2/kernel/vifast-plat-common/src/irq_lock.rs` (**mới**, 286 LOC) chứa **một** bản
`IrqMcsLock<T, S: IrqSave>` + `IrqMcsGuard` + bộ đếm + hai móc đột biến (`mut-no-lock`,
`mut-leak-unlock`). Mỗi arch chỉ còn `IrqState` + `impl IrqSave` + hai type alias:

| Tệp | Trước | Sau | Cắt |
|---|---:|---:|---:|
| `vifast-plat-riscv64/src/irq_lock.rs` | 408 | 214 | −194 |
| `vifast-plat-x86_64/src/irq_lock.rs` | 395 | 175 | −220 |
| `vifast-plat-aarch64/src/irq_lock.rs` | 367 | 208 | −159 |
| **tổng 3 bản** | **1.170** | **597** | **−573** |
| `vifast-plat-common/src/irq_lock.rs` | 0 | 286 | +286 |

**Bằng chứng ngữ nghĩa không đổi**: test host của x86_64 (`tests/irq_lock.rs`, 6 ca: loại trừ 4
luồng, không reentrant, trần chờ hữu hạn, đếm `contended`, `IrqState` khôi phục đúng) chạy trên
**bản chung** — **6/6 xanh**:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-x86_64 --test irq_lock
running 6 tests
test irq_bounded_tra_none_khi_dang_bi_giu ... ok
test irq_lock_khong_reentrant ... ok
test irq_state_cap_va_tra_ve_dung_cap ... ok
test irq_try_lock_none_while_held_va_lay_lai_sau_drop ... ok
test irq_lock_serialises_four_threads ... ok
test irq_contended_dem_so_lan_phai_cho ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Móc đột biến được **re-export** ở crate arch nên caller trong ảnh kiểm
(`vifast_plat_riscv64::irq_lock::mut_arm_no_lock()`) **không đổi một dòng**. `v2/test/mutants.toml`
**không sửa**: `mutants.py` lật `feature = "mut-*"` theo **chuỗi** trên `v2/kernel/**/*.rs` nên
điểm cfg mới ở `plat-common` vẫn được lật; danh sách `crates` chỉ là mô tả (lint đếm **toàn cục**,
không theo crate) — ghi chú ở §6.

## 4. Hồi quy **có sẵn ở base** `e495365f` (phát hiện, đã vá)

`KT-409b` (`a2532a67`) dời module `trap` khỏi `vifast-plat-common` sang
`vifast-plat-riscv64-logic` nhưng **sót hai tham chiếu**, nên **6/14 ảnh ktest riscv64 không biên
dịch được** trên base (mọi ảnh bật feature `user`):

```text
$ cargo build --target riscv64gc-unknown-none-elf --release --bin vifast-kbin-ktest-user --features ktest,user
error[E0433]: cannot find `trap` in `vifast_plat_common`
   --> vifast-plat-riscv64/src/user/trap.rs:245:25
    |     vifast_plat_common::trap::report_u_fault(&Riscv64Virt, exc, tf)
error[E0433]: cannot find module or crate `abi` in this scope
   --> vifast-plat-riscv64/src/user.rs:555:9
    |         abi::dispatch(m, tcb, 0, nr, args, &mut UserMemAccess, &USER_TASK)
```

Đã vá **đúng hai dòng** (0 đổi ngữ nghĩa, 0 nới ngưỡng): `vifast_plat_common::trap::` →
`vifast_plat_riscv64_logic::trap::`, và thêm `use vifast_plat_riscv64_logic::user as abi;`.
Sau khi vá **14/14** ảnh riscv64 build xanh (xem §5). **Cần lead xác nhận** đây là hunk riêng,
ngoài phạm vi KT-411, để merge tách hoặc gộp vào bản vá KT-409b.

## 5. T0 / check / T1

```text
$ cargo test -p vifast-ktest            → 6 passed; 0 failed
$ cargo test -p vifast-plat-common      → 32 + 15 + 7 passed; 0 failed
$ cargo test -p vifast-plat-x86_64 --test irq_lock → 6 passed; 0 failed
$ cargo clippy -p vifast-ktest --all-targets -- -D warnings            → sạch
$ cargo clippy -p vifast-plat-common --all-targets -- -D warnings       → sạch
$ cargo clippy --target riscv64gc-unknown-none-elf -p vifast-plat-riscv64 --features ktest → sạch
$ cargo build --target riscv64gc-unknown-none-elf --release --bin <14 bin> → 14/14 OK
$ ./v2/bin/v2 check                     → PASS (exit 0, TỔNG check 88,4 s)
```

`./v2/bin/v2 t0` **không** dùng được cho crate kernel: nó chạy `cargo clippy` **không** `--target`
⇒ asm RISC-V/AArch64 không biên dịch trên host (`invalid register a7`). Đây là hạn chế **có sẵn**
của `t0` (không phải do KT-411); T0 cho crate kernel ở đây chạy bằng `cargo clippy --target …`
tương đương (kết quả ở trên).

## 6. T1 lượt 1 — ĐỎ, và bài học luật `wake` (đã vá)

Lượt T1 đầu **ĐỎ** ở đúng hai bước luật `wake`:

```text
--- wake paths + --strict (KT-151/155 — KT-333e: một lượt) (FAIL)
--- wake theo dep-info (KT-217: lớp 2) (FAIL)
E E_STRICT_TEST_ONLY_WAKE   v2/kernel/vifast-ktest/src/core_suite.rs:155:
  gọi `wake(` trong mã sản phẩm — `wake()` là API CHỈ-DÙNG-CHO-TEST (D2-144)
```

**Nguyên nhân**: `core_suite::task_ready` gọi `m.wake(tcb)` (đúng ngữ nghĩa cũ của
`ktest_core.rs`), nhưng `vifast-ktest` là **crate thư viện** nên luật coi là mã sản phẩm.

**Vì sao `#[cfg(feature = "ktest")]` không cứu được** (đã đọc mã nguồn luật, không đoán):
`KT-227 P0-4` **bỏ** miễn trừ theo `#[cfg(feature = "ktest")]` (đường lọt B3b của KT-224:
`#[cfg(feature = "ktest")] mod evil;` + tệp riêng chứa `wake`). Miễn trừ còn lại là **đồ thị
module**: tệp chỉ tới được từ **bin** có `required-features` chứa `ktest`. `vifast-plat-riscv64`
là **lib** (link vào cả ảnh sản phẩm) nên `ktest_bridge.rs` **vẫn** là sản phẩm — lượt vá thử
đặt `task_ready` vào đó cũng ĐỎ (`ktest_bridge.rs:139`), xác nhận đúng cách hiểu luật.

**Cách vá (0 nới ngưỡng, 0 sửa `v2/bin/**`)**: bộ ca chung nhận cửa `Ready` qua **con trỏ hàm**
`vifast_ktest::core_suite::ReadyFn`; cài đặt `task_ready` nằm ở
`vifast-kbin-riscv64/src/ktest_core.rs` — tệp **test-only theo đồ thị module** (bin
`vifast-kbin-ktest` có `required-features = ["ktest"]`) nên `wake()` hợp lệ. Sau khi vá:

```text
$ python3 v2/tools/check-wake-paths.py --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
$ python3 v2/tools/check-wake-depinfo.py
# check-wake-depinfo: errors=0 warnings=0 (40.19 s)
```

**Bài học ghi lại**: rút mã test ra khỏi `vifast-kbin-*` **không** miễn nhiên hợp lệ — luật
`wake` miễn theo **đồ thị module**, không theo tên crate. Mọi lần rút tiếp (a64/x64) phải giữ
`wake()` ở `vifast-kbin-*/src/ktest*.rs` và truyền qua `ReadyFn`.

## 7. T1 lượt 2 — chỉ còn **một** bước đỏ, và nó là **thuộc tính của worktree bẩn**

```text
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
--- selftest pre-measure (KT-95)
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
--- selftest pre-measure (KT-95): FAIL
...
# step-time-total   468249 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**Nguyên nhân (không phải lỗi mã)**: `v2/tools/pre-measure.sh` từ chối **đo trên cây bẩn** —
`[ "$DIRTY" = "0" ] || refuse 7 tree-dirty` (dòng 209), với `DIRTY` tính trên **build-inputs**.
Worktree này có 14 tệp kernel chưa commit (đúng phần KT-411) ⇒ `DIRTY=1` ⇒ ca tự kiểm (a) — vốn
dựng ảnh giả `dirty=0` rồi đòi `PRE-MEASURE: OK` — **phải** đỏ. Đây là thiết kế (chống đo ảnh
không khớp cây), không phải hồi quy.

**Đối chứng A/B (chạy trên repo chính, chỉ đọc)**:

```text
$ cd /Users/mps/dvchd/ViFastOs && git status --porcelain --untracked-files=no
 M v2/agent/QUEUE.md                     # KHÔNG phải build-input ⇒ DIRTY=0
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest: PASS (a/b/c/d/e/f/g)
```

Cùng một script, cùng một commit gốc: **PASS** khi build-inputs sạch, **FAIL (7)** khi bẩn ⇒ kết
luận "bước này chỉ xanh sau khi lead commit" là **đo được**, không phải suy đoán.

**Mọi bước T1 khác đều OK**, gồm đúng hai bước đã đỏ ở lượt 1:
`wake paths + --strict` OK, `wake theo dep-info` OK, `L0 kernel workspace` OK, `L1 (sim 1.000
seed)` OK, `luật cấu trúc R1..R5` OK, `budget` OK, `clippy` OK, `fmt` OK.

**Hệ quả cần lead biết**: vì `do_gate_dev` trả `1` ngay sau `lane_run`, bước **QEMU smoke (id 70)**
— bước duy nhất boot ảnh thật — **không chạy** trong cả hai lượt T1. Vì vậy tôi chạy **thủ công**
ma trận `ktest.sh 1 1` để có bằng chứng TAP 43..52 không đổi (§8).

## 8. Bằng chứng boot thật (bù cho QEMU smoke không chạy được trên cây bẩn)

`ktest.sh 1 1` cũng bị `REV-GATE: REFUSE reason=dirty` (KT-86/D2-125.3) nên **không** dùng được
trên worktree chưa commit. Vì vậy tôi boot **trực tiếp** bằng đúng dòng QEMU của runner
(`-icount shift=6 -machine virt,aia=none,aclint=on -bios opensbi… -cpu rv64 -m 128M -smp 1`) và
đọc TAP thật. Đây là phép đo **độc lập** với harness, không sửa `accept()`/kỳ vọng nào.

### 8.1 riscv64 — TAP 43..52 **nguyên văn**, 0 đỏ

```text
$ qemu-system-riscv64 … -kernel …/vifast-kbin-ktest
ok 43 - core link
ok 44 - core untyped boot
ok 45 - core tcb retype
ok 46 - core configure resume suspend
ok 47 - core sched 2 tcb
ok 48 - core ipc round
ok 49 - core notification signal
ok 50 - core waitaddr
ok 51 - core d2-83 boundary
ok 52 - core invariants
# core: phase fails=0 static=184712 pin=196608 image_cap=262144
```

- **71** dòng TAP, **0** dòng `not ok` trong cả lần boot.
- **11/11** dòng evidence `# core:` giữ **nguyên định dạng** (link/untyped/tcb retype/configure/
  sched/ipc/ntfn/waitaddr/d2-83/invariants/phase) — runner grep theo đúng các chuỗi này.
- Boot kết thúc bằng `PANIC: nested trap (KT-45 …)` **đúng như trước** (TAP 71 là ca cuối, panic
  có chủ đích của ảnh `ktest`).
- TAP 70 (`lock: mcs irq guard`) xanh trên **khoá chung mới**: `deferred=1 delivered=1 overlap=0 ok=1`.

### 8.2 x86_64 — TAP 43 (khoá) xanh trên **bản chung**

```text
$ qemu-system-x86_64 -M q35 -m 128M -nographic -no-reboot -smp 4 -kernel …/vifast-kbin-x86_64
# core lock: irq_before=1 irq_inside=0 irq_after=1 acquired=14 overlap=0 in_cs=0 ok=1
ok 43 - lock: RFLAGS.IF masked in critical section, restored after
```

`irq_inside=0` + `irq_after=1` ⇒ `IrqSave` của x86_64 (`cli`/`sti` qua `IrqState`) vẫn tắt/khôi
phục đúng sau khi mã khoá dời sang `plat-common`. (`not ok 16 - net: mac` trong lần boot này là do
tôi **không** truyền `-netdev/-device virtio-net-pci` — runner mới thêm; không phải hồi quy.)

### 8.3 aarch64 — **chưa** đo được trên bare-metal trong ngân sách

Boot tay dừng ở TAP 35 (hết pha net) vì thiếu bộ thiết bị đầy đủ mà `boot_full` của runner dựng
(`virtio-blk` + `virtio-net` + monitor + TCP server), nên **chưa** tới TAP 41 (`lock: irq`). Đây là
**khoảng trống bằng chứng đã biết**, không phải kết luận xanh:
- `IrqState` của aarch64 **không đổi một dòng** (chỉ thêm `impl IrqSave` 3 hàm uỷ nhiệm);
- mã khoá/guard là **cùng một bản** đã xanh trên riscv64 (TAP 70) và x86_64 (TAP 43);
- crate biên dịch sạch cho `aarch64-unknown-none`.
**Cần lead xác nhận TAP 41 ở T2** (bước QEMU smoke/ma trận aarch64 chạy trên cây đã commit).

## 9. Vì sao **chưa** gom 14 `[[bin]]` → 2 mode (ưu tiên 2) — dừng có lý do, kèm kế hoạch

Đã **không** làm, và đây là lý do đo được, không phải bỏ sót:

1. **`< 16.000` LOC không thể đạt trong 3 h**: `vifast-kbin-riscv64/src` hiện **27.194** LOC. Bộ ca
   core dùng chung chỉ rút được **675** LOC khỏi riscv64 (phần *thật sự* dùng chung). 12.000 LOC
   còn lại là **driver + rig U-mode riêng của từng ảnh** (`ktest_ipc` 1.404, `ktest_vfs` 1.332,
   `ktest_user` 1.254, `ktest_persist` 1.148, `ktest_net` 1.125, …) — rút chúng đòi hỏi bộ trait
   `Platform` **đầy đủ** (map page, nạp ELF, vào U-mode, PCI/MMIO), tức nhiều task.
2. **Gom 14 → 2 phải sửa 8 tệp harness** (không phải 1): `v2/test/boot/ktest.sh` (8 khối build +
   `cp` artefact), `guard_selftest.sh`, `input.sh`, `gpu.sh`, `net.sh`, `persist.sh`,
   `v2/test/parity/run.sh`, `v2/test/conformance/run.sh` — cộng 14 tệp nguồn (1 dispatcher + 13
   `rust_main` → `pub fn run`, bỏ 13 `#[panic_handler]`/`rust_secondary_main`). Mỗi tệp harness
   sai một dòng là **ma trận T2 đỏ**, mà tôi **không** kiểm được T1/T2 trên cây bẩn (§7).
3. **Rủi ro không cân xứng**: giao một refactor 22 tệp **chưa từng chạy cổng** cho lead chạy T2
   nguy hiểm hơn giao phần đã có bằng chứng boot thật (§8).

**Kế hoạch đề nghị cho task kế tiếp** (đúng thứ tự, mỗi bước một commit xanh T1):
1. `src/ktest.rs` → dispatcher `rust_main` chọn `suite-*`; chuyển thân hiện tại sang
   `src/ktest_base.rs` (`pub fn run() -> !`).
2. 13 tệp `ktest_*.rs`: `rust_main` → `pub fn run() -> !`; xoá `rust_secondary_main` +
   `#[panic_handler]` (giữ **một** bản trong `ktest.rs`).
3. `Cargo.toml`: 14 `[[bin]]` → 2 (`vifast-kbin-riscv64`, `vifast-kbin-ktest`); thêm 13 feature
   `suite-*` (mỗi suite một nhãn, vì `ktest-user`/`ktest-proc` và `ktest-vblk`/`ktest-persist`
   dùng **chung** bộ feature nên không thể phân biệt bằng feature sẵn có).
4. `ktest.sh` + 5 runner khác: `--bin vifast-kbin-ktest --features ktest,<suite>` rồi
   `cp target/…/vifast-kbin-ktest target/…/vifast-kbin-ktest-<label>` (giữ **nguyên** tên ảnh để
   `image_path`/`expected.txt` không đổi).
5. Đo lại `budget.sh` cho từng ảnh (gom bin **không** được đổi `.text`/`.data+.bss` của ảnh nào).

## 10. Việc còn lại / rủi ro

| # | Việc | Trạng thái |
|---|---|---|
| 1 | a64/x64 dùng bộ ca chung | **Chặn bởi kỳ vọng**: `coretest.rs` của a64 phát TAP **36..41**, x64 phát **38..47** — khác bộ 43..52 của riscv64. Đổi sang bộ chung là **đổi nhãn TAP** ⇒ đổi `expected.txt` (AGENTS §4.2 cấm). Cần lead quyết: hoặc cho phép cập nhật `expected.txt` (một lần, có kiểm), hoặc giữ 3 bộ và chỉ dùng chung phần `invariants` (đã sẵn sàng). |
| 2 | Gom 14 `[[bin]]` → 2 | **Chưa làm** — lý do + kế hoạch §9. |
| 3 | `< 16.000` LOC | **Không đạt** (27.194). Cần 2–3 task nữa (§9.1). |
| 4 | TAP 41 aarch64 (khoá chung) | **Chưa đo** trên bare-metal (§8.3) — cần T2. |
| 5 | `v2/test/mutants.toml` | **Không sửa**: điểm cfg `mut-no-lock`/`mut-leak-unlock` nay ở `vifast-plat-common/src/irq_lock.rs`; `mutants.py` lật theo **chuỗi** trên `v2/kernel/**/*.rs` nên vẫn hoạt động. Danh sách `crates` chỉ là mô tả (lint đếm toàn cục) — lead có thể cập nhật cho đúng chỗ nếu muốn. |
| 6 | Hồi quy KT-409b (`user` feature) | **Đã vá 2 dòng** (§4) — hunk **ngoài phạm vi KT-411**, cần lead tách khi merge. |

## 11. Số cuối (đo lại sau khi hoàn tất)

| Hạng mục | Trước | Sau | Δ |
|---|---:|---:|---:|
| `vifast-kbin-riscv64/src` (tổng) | 27.869 | 27.214 | **−655** |
| `…/src/ktest_core.rs` | 1.090 | 435 | **−655** |
| `vifast-ktest/src` (mới) | 0 | 1.127 | +1.127 |
| `vifast-ktest/tests` (mới) | 0 | 300 | +300 |
| `vifast-plat-riscv64/src/ktest_bridge.rs` (mới) | 0 | 152 | +152 |
| `irq_lock.rs` × 3 arch | 1.170 | 594 | **−576** |
| `vifast-plat-common/src/irq_lock.rs` (mới) | 0 | 286 | +286 |
| `[[bin]]` của `vifast-kbin-riscv64` | 14 | **14** | 0 (chưa gom — §9) |

**Đọc đúng con số**: 655 LOC rút khỏi riscv64 **không** phải "mất 655 dòng" — nó **chuyển** sang
crate dùng chung (1.127 LOC phục vụ **cả ba** arch, thay vì 3 bản sao). Với `irq_lock`, mức cắt
**thật** là 576 LOC (ba bản → một bản + ba impl mỏng).

## 12. Tự kiểm cuối

```text
$ cargo fmt --all --check                                   → OK
$ ./v2/bin/v2 check                                         → PASS (exit 0, TỔNG check 78,2 s)
$ cargo test -p vifast-ktest                                → 6 passed; 0 failed
$ cargo test -p vifast-plat-common                          → 32 + 15 + 7 passed; 0 failed
$ cargo test -p vifast-plat-x86_64 --test irq_lock          → 6 passed; 0 failed
$ cargo build --target riscv64gc-unknown-none-elf --release --bin <14 bin> → 14/14 OK
$ cargo build --target aarch64-unknown-none  --release -p vifast-kbin-aarch64 --features ktest → OK
$ cargo build --target x86_64-unknown-none   --release -p vifast-kbin-x86_64  --features ktest → OK
$ python3 v2/tools/check-wake-paths.py --strict             → errors=0 warnings=0
$ python3 v2/tools/check-wake-depinfo.py                    → errors=0 warnings=0
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev        → FAIL **chỉ** ở `selftest pre-measure`
                                                              (REFUSE reason=tree-dirty, §7)
```

**Không** chạy: T2 (`gate --fast`), `nightly`, `soak`, `l1-tier medium/full`, `v2 mutants --all`
(đúng giới hạn task). **Không** `git add/commit/push/merge/cherry-pick/stash`. **Không** sửa
`spec/`, `v2/agent/{STATE,QUEUE,DECISIONS,BLOCKED}.md`, `reports/STATUS.md`.

## 13. D2-08 — ảnh **sản phẩm** vẫn 0 harness (đo bằng đồ thị phụ thuộc)

```text
$ cargo tree -p vifast-kbin-riscv64                                  | grep -c vifast-ktest → 0
$ cargo tree -p vifast-kbin-riscv64 --features product-all           | grep -c vifast-ktest → 0
$ cargo tree -p vifast-kbin-riscv64 --features ktest                 | grep -c vifast-ktest → 2
```

`vifast-ktest` là **optional dependency** của cả `vifast-kbin-riscv64` và `vifast-plat-riscv64`,
chỉ bật qua `ktest = ["dep:vifast-ktest"]`. Ảnh sản phẩm (kể cả `product-all` = shell + FS + net +
exec) **không** kéo crate harness nào — đúng D2-08, không phải suy luận.
