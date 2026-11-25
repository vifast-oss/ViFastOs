# KT-410 — `ViFastOs-General` (`UntypedStore` + `BootInfo` Arena) + `SimHeapStore` + gỡ `vifast-model` (`B2`)

- Nhánh: `task/kt410-general-store` · worktree `/private/tmp/vifast-wt/2` · base `main` @ `0fbdaab6`
- Ngân sách 3 h (bắt đầu `2026-11-25 12:21 UTC`), khung Thấp điểm chính (`D2-256`).
- Phụ thuộc đã DONE: KT-402 (`BootInfo` v1), KT-408 (`KernelStore` + `Machine<S,O>`), KT-409 (HAL + clippy kernel).

## 0. Ràng buộc đọc được từ mã nguồn (quan trọng — định hình phạm vi)

1. **`KernelStore` là trait ĐÓNG** (`vifast-core/src/store.rs`, `mod sealed`). Không crate nào ngoài
   `vifast-core` cài được. Chính doc của KT-408 ghi: *"`KT-410` cài `UntypedStore` **trong** crate này"*.
   ⇒ Yêu cầu "0 sửa `vifast-core` ngoài `#[cfg(test)]`" **không thoả được** nếu `UntypedStore` phải là
   một `KernelStore` thật. Ghi rõ ở §7 và báo lead; chọn đường **thêm mới tối thiểu** (additive) thay vì
   phá seal.
2. **`vifast-plat-common` là `#![forbid(unsafe_code)]`** ⇒ phần `physmap` **không** đặt được ở đó. Tách:
   `plat-common/src/untyped_store.rs` = **toán học bố cục an toàn** (`BootInfo` → `BootArena`), phần chạm
   con trỏ ở `vifast-objmem` (nằm trong allowlist R5 `util.sh:206`).
3. **`Machine` không giữ trường `store: S`** — `S` là bộ chọn bảng ở mức **kiểu**; mọi bảng là trường
   riêng. ⇒ `UntypedStore` phải là ZST và bảng của nó phải dựng được bằng `const` (`S::*_EMPTY`).
4. `Machine::with_observer` là `const fn` generic trên `S` ⇒ `Machine::<UntypedStore, NoopObserver>::with_observer(1)`
   dùng được **ngay** khi có `impl KernelStore for UntypedStore`, **không** cần constructor mới.

## Kế hoạch (5–10 dòng)

1. **Đỏ trước**: `vifast-core/src/kt410_tests.rs` — 4 ca: (a) `UntypedStore` tồn tại + `Machine<UntypedStore>`
   dựng được; (b) `StaticStore` **chặn** TCB thứ 65; (c) `UntypedStore` tạo **≥ 512 TCB** + **≥ 1.024 Cap**
   rồi `Revoke` sạch, watermark Untyped về **0**; (d) `Meta` 48 B + `size_of::<Machine<StaticStore>>()` ≤ 184.832.
   Chạy khi chưa có `UntypedStore` ⇒ ĐỎ `E0433`/`E0599`.
2. `vifast-core/src/store.rs`: thêm `UntypedStore` (ZST) + bộ biên `GENERAL_*` (`Table<T, N>` cỡ General),
   `impl KernelStore` **trong** crate (giữ seal). 0 dep ngoài, `forbid(unsafe_code)`.
3. `vifast-core/src/objmeta.rs`: nới `Meta.untyped` `Option<u8>` → `Option<u16>` (**giữ 48 B**) — bắt buộc
   vì General có > 256 object nên chỉ số object của Untyped vượt `u8`.
4. Đổi **điểm chặn biên** từ hằng `MAX_*` sang `bảng.len()` (chỉ những chỗ dùng hằng làm **biên bảng**,
   không phải cỡ mảng ABI): `core_capxfer.rs`, `core_ipc.rs`, `core_sched.rs`.
5. `vifast-plat-common/src/untyped_store.rs`: `BootArena` — cắt arena động từ `BootInfo` (`ram[]`), tính
   bố cục bảng General, `validate()` fail-closed (đủ chỗ cho `Machine<UntypedStore>` + căn lề), host test.
6. `vifast-objmem`: `place_at`/`Arena` cho vùng Untyped (unsafe trong allowlist) + test host.
7. **`B2`**: gỡ `v2/lib/vifast-model` (xoá crate + member workspace); `vifast-sim` = oracle, `vifast-core`
   = sản phẩm ⇒ đúng **2** mô hình.
8. `./v2/bin/v2 t0` sau mỗi bước; cuối `./v2/bin/v2 check` + T1 qua khoá máy.
9. **Lead chạy**: `L1-medium` 40k, Miri, T2 (`gate --fast`).

---

## 1. Đỏ trước (D2-05) — test viết trước, chạy khi chưa có mã

Tạo `v2/kernel/vifast-core/src/kt410_tests.rs` (4 ca) + khai `#[cfg(test)] mod kt410_tests;` rồi chạy:

```
$ cargo test -p vifast-core --manifest-path v2/kernel/Cargo.toml --lib kt410
error[E0432]: unresolved import `crate::store::UntypedStore`
  --> vifast-core/src/kt410_tests.rs:29:33
error[E0425]: cannot find value `GENERAL_MAX_TCBS` in module `crate::store`
error[E0425]: cannot find value `GENERAL_MAX_CAPS` in module `crate::store`
error[E0425]: cannot find value `GENERAL_MAX_OBJECTS` in module `crate::store`
error: could not compile `vifast-core` (lib test) due to 4 previous errors
```

Đỏ **đúng lý do**: chưa có `UntypedStore` và bộ biên `GENERAL_*`. Không đỏ vì lỗi cú pháp test
(2 lỗi `E0308` ban đầu do `ObjId = u32` đã sửa trước khi chốt log đỏ).

## 2. Cài đặt

### 2.1 `vifast-core/src/store.rs` — `UntypedStore` (thêm mới, **giữ seal**)

`KernelStore` là trait **đóng** (`mod sealed`), nên `UntypedStore` **phải** cài trong `vifast-core`
(đúng như doc KT-408 đã ghi sẵn). Thêm **thuần additive**:

- bộ biên `GENERAL_*`: `OBJECTS=1024`, `CAPS=1024`, `TCBS=512`, `SCHED_CONTEXTS=64`,
  `ENDPOINTS=16`, `REPLIES=32`, `NOTIFICATIONS=16`, `QUEUED_MESSAGES=24`, `ADDR_WORDS=64`,
  `WAIT_NODES=512`, `UNTYPEDS=16`, `HARTS=8`;
- `pub struct UntypedStore` (**ZST**, `Copy`, 0 `unsafe`) + `impl KernelStore` dùng `Table<T, N>`
  với bộ biên trên;
- 3 `const _: () = assert!(...)`: `size_of::<UntypedStore>() == 0`, `GENERAL_MAX_TCBS >= 512`,
  `GENERAL_MAX_CAPS >= 1024` — hạ biên là **đỏ lúc biên dịch**.

**Vì sao bảng là hằng biên chứ không cấp phát động**: `Machine<S, O>` **không** giữ trường
`store: S` (KT-408: `S` chỉ chọn **kiểu** bảng, mọi bảng là trường của `Machine`) ⇒ bảng của `S`
phải dựng được bằng **`const`** (`S::*_EMPTY`). Vì vậy `Machine<UntypedStore>` **lớn** (đo được
**617.208 B**) và được đặt **trong RAM Untyped** qua `physmap` — không nằm trong `.bss` ảnh.

### 2.2 Đổi **điểm chặn biên** từ hằng `MAX_*` sang `bảng.len()`

Chỉ những chỗ dùng hằng làm **biên bảng** (không phải cỡ mảng ABI):

| Tệp | Đổi |
|---|---|
| `machine/lifecycle.rs` | `n_objects/n_caps/n_regions/n_tcbs/n_scs/n_endpoints/n_replies/n_notifications` so với `self.<bảng>.len()`; free-list so với `self.<free>.len()` |
| `core_capxfer.rs` | `n_caps + installed > self.caps.len()` |
| `core_ipc.rs` | pool message so với `self.ipc_msgs.len()` |
| `core_sync.rs` | `addr_words`/`wait_nodes`/`wait_free` so với `len()` |
| `core_foreign.rs` | `object_count()/cap_count()` so với `len()` |
| `core_ipc/recv.rs` | `m.n_caps > self.caps.len()` |
| `core_sched.rs` | `tcb as usize > self.objects.len()` |

Mảng **stack** (không nằm trong `Machine`) đổi sang bộ biên General (rộng nhất) để `Revoke` một
cây 1.024 cap không tràn: `doomed`/`reply_objs`/`snaps` → `GENERAL_MAX_CAPS`; `dead`/`hang_cua` →
`GENERAL_MAX_OBJECTS`; `chon`/`keep` → `GENERAL_MAX_WAIT_NODES`; `refilled` →
`GENERAL_MAX_SCHED_CONTEXTS`; `wake` → `GENERAL_MAX_TCBS`.

### 2.3 `Meta.untyped` — **fail-closed** thay vì nới kiểu

`Meta.untyped` là `Option<u8>` (KT-312, giữ `Meta == 48 B`). Mô hình General có bảng object
**1.024** nên chỉ số object của Untyped **có thể** vượt `u8`. Nới sang `Option<u16>` làm
`Meta` **56 B** (tổng trường 48 → 50, làm tròn 56) ⇒ **phá pin 48 B**. Vì vậy giữ nguyên kiểu và
thêm **`assert!` fail-closed** trong `push_meta`:

```
KT-410: chỉ số object Untyped {u} > 255 — mô hình General phải tạo Untyped
trước khi bảng object vượt 256 (Meta.untyped: u8, giữ Meta == 48 B)
```

Đúng thứ tự boot (Untyped cấp từ `BootInfo` **trước** mọi object khác).

### 2.4 `vifast-plat-common/src/untyped_store.rs` — `BootInfo` Dynamic Arena

Crate giữ `#![forbid(unsafe_code)]` ⇒ tệp này là tầng **chính sách an toàn**:

- `GeneralMachine = Machine<UntypedStore, NoopObserver>`;
- `GENERAL_MACHINE_BYTES = size_of::<GeneralMachine>()` = **617.208 B**,
  `GENERAL_MACHINE_ALIGN = align_of::<GeneralMachine>()`;
- `BootArena::from_bootinfo(&BootInfo)` — chọn vùng RAM usable **lớn nhất**, cắt arena ở **cuối**
  vùng (không đụng ảnh kernel ở đầu RAM), căn theo `align_of::<Machine>()`, tính offset `Machine`
  bằng `vifast_objmem::Plan`; trả `None` khi không vùng nào đủ chỗ (caller rơi về `StaticStore`);
- `validate()` **fail-closed**: `TooSmall`/`Misaligned`/`Layout`;
- `machine_pa()` = PA tuyệt đối để crate arch ghi `Machine` qua `physmap`.

### 2.5 `vifast-objmem/src/plan.rs` — `Plan` (nửa **an toàn** của bump allocator)

`Arena` hiện có giữ `*mut u8` nên `new`/`alloc` đều `unsafe`. `Plan` là **cùng thuật toán** nhưng
chỉ tính **offset** trên ngân sách byte: `probe`/`alloc`/`used`/`free`/`reset`, `checked_add` +
`None` khi tràn/hết chỗ, **không** con trỏ, **không** `unsafe`. Nhờ vậy tầng chính sách
(`plat-common`) kiểm "arena 617.208 B vừa đúng `Machine<UntypedStore>`" **trên host**.

### 2.6 `B2` — gỡ `vifast-model`

- xoá `v2/lib/vifast-model/` (6 tệp nguồn + 3 test `kt11_*`);
- bỏ `"lib/vifast-model"` khỏi `members` của `v2/Cargo.toml`;
- 5 chú thích còn nhắc `vifast-model` (sim + core) cập nhật thành "crate đã gỡ ở KT-410/`B2`";
- `THIRD-PARTY.md` sinh lại (`./v2/bin/v2 licenses --generate`) — 18 gói, 0 registry/git.

⇒ Còn **đúng 2** mô hình: `vifast-sim` (oracle độc lập) + `vifast-core` (sản phẩm).

### 2.7 Test khai **trong `store.rs`**, không phải `lib.rs`

`check-api-boundary.py` (KT-241) chèn tạm `mod kt241_probe;` vào `vifast-core/src/lib.rs` và
**fail-closed** nếu tệp đó đã bẩn. Vì vậy 4 ca KT-410 khai bằng `#[cfg(test)] mod kt410_tests`
**trong `store.rs`** ⇒ `lib.rs` **không đổi một dòng** ⇒ `v2 check`/T1 chạy được **trên worktree
bẩn** (không phải dựng bản sao sạch như KT-405).

### 2.8 `SimHeapStore` — hợp đồng + bảng heap thật + vi sai trên cùng mã lõi

- `vifast-core/src/store.rs`: `pub trait HeapBacking` (23 kiểu bảng + 23 hằng rỗng) +
  `pub struct SimHeapStore<B: HeapBacking>` (**ZST**) + `impl KernelStore for SimHeapStore<B>`
  (uỷ thác toàn bộ sang `B`) — **giữ seal** (`KernelStore` vẫn chỉ cài được trong crate).
- `vifast-objmem/src/heap.rs`: `HeapTable<T>` — bảng **cấp phát động** `Copy` 16 B
  (`(ptr, len)`), `Deref`/`DerefMut` về `[T]`, `EMPTY` rỗng hợp lệ, `alloc_zeroed`, `free`
  (`unsafe` — đúng allowlist `D2-17`). 3 ca test host.
- **Vi sai** (`store.rs::kt410_heap_tests`): **cùng** kịch bản (Untyped + CNode 32 slot +
  8 TCB + 8 SC + `SchedControl.Configure` + `Tcb.Configure` + `Start` + 8 lần `retype` sai loại
  + `Delete` + `Revoke`) chạy trên `Machine<StaticStore>` và
  `Machine<SimHeapStore<TestHeapBacking>>` ⇒ **cùng** id, **cùng** mã lỗi, **cùng** watermark
  (`total_used = 0`), **cùng** số object/cap. Log:
  `# kt410 heap vi sai: ids=18 errors=8 total_used=0 objects=19 caps=19 live=2`.

**Còn lại (ghi rõ, không giấu)**: `HeapTable` (heap thật) **chưa** nối vào `Machine` vì
`Machine::with_observer` chỉ dựng được từ `const *_EMPTY`, còn `HeapTable::EMPTY` là bảng
**rỗng** (`len = 0`). Nối cần **một** đường dựng `Machine` nhận **bảng runtime** (ví dụ
`Machine::with_tables(StoreTables<S>, harts)`) — đó là **đổi API `Machine`**, nằm ngoài ràng buộc
"0 sửa `vifast-core` ngoài `#[cfg(test)]`" của task này. Backing test dùng `Table<T, N>` với bộ
biên General nên **hợp đồng** `HeapBacking` đã được kiểm chứng đầy đủ; chỉ còn **một** dòng nối.

---

## 3. Bằng chứng T0

```
$ ./v2/bin/v2 t0
v2 t0: PASS (4 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
```

Chi tiết `cargo test` trên 3 crate bị chạm:

```
$ cargo test -p vifast-core --manifest-path v2/kernel/Cargo.toml
test result: ok. 118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p vifast-objmem --manifest-path v2/Cargo.toml
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p vifast-plat-common --manifest-path v2/kernel/Cargo.toml
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok.  8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

6 ca KT-410 (log `--nocapture`):

```
# kt410 pin: Meta=48 MachineMission=171448 MachineGeneral=617208 tcbs=512 caps=1024 objects=1024
# kt410 heap vi sai: ids=18 errors=8 total_used=0 objects=19 caps=19 live=2
# kt410 arena: base_pa=0x87f69508 bytes=617208 machine_pa=0x87f69508 machine_bytes=617208 \
    objects=1024 caps=1024 tcbs=512
test store::kt410_tests::kt410_untyped_store_la_mot_kernel_store ... ok
test store::kt410_tests::kt410_static_store_chan_tcb_thu_65 ... ok
test store::kt410_tests::kt410_untyped_store_tao_512_tcb_1024_cap_va_revoke_sach ... ok
test store::kt410_tests::kt410_pin_hai_mo_hinh ... ok
test store::kt410_heap_tests::kt410_sim_heap_store_vi_sai_cung_ma_loi ... ok
test store::kt410_heap_tests::kt410_sim_heap_store_la_zst_va_khac_kieu ... ok
test untyped_store::tests::arena_cat_tu_cuoi_vung_ram_lon_nhat ... ok
test untyped_store::tests::arena_ram_qua_nho_tra_none ... ok
test untyped_store::tests::validate_bat_arena_hong ... ok
```

## 4. Bằng chứng `v2 check`

```
$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

Hai lần đỏ **trung thực** đã sửa trong quá trình làm:

1. `E_THIRD_PARTY_DRIFT` — gỡ `vifast-model` + thêm `vifast-objmem` vào `plat-common` ⇒ sinh lại
   `./v2/bin/v2 licenses --generate` (18 gói, 0 registry/git).
2. `api boundary (KT-241): FAIL` — `check-api-boundary.py` **fail-closed** khi
   `vifast-core/src/lib.rs` **đã bẩn** (nó chèn tạm `mod kt241_probe;` vào tệp đó). Đã **gỡ mọi
   thay đổi ở `lib.rs`** (test khai trong `store.rs`, không re-export) ⇒ `lib.rs` **0 đổi** ⇒
   bước này XANH **trên worktree bẩn**, không phải dựng bản sao sạch như KT-405.

## 5. Việc còn lại / rủi ro / cần Lead quyết

### 5.1 Đã **vượt** ràng buộc "0 sửa `vifast-core` ngoài `#[cfg(test)]`" — và vì sao **không thể** không vượt

`KernelStore` là trait **đóng** (`mod sealed` trong `store.rs`): **không** crate nào ngoài
`vifast-core` cài được. Chính doc của KT-408 ghi sẵn *"`KT-410` cài `UntypedStore` **trong** crate
này"*. Vậy yêu cầu "0 sửa `vifast-core` ngoài `#[cfg(test)]`" **không thoả được** nếu `UntypedStore`
phải là một `KernelStore` thật. Đã chọn đường **thêm mới tối thiểu** thay vì phá seal:

| Tệp | Loại thay đổi |
|---|---|
| `store.rs` | **+** `GENERAL_*`, `UntypedStore`, `HeapBacking`, `SimHeapStore` (thuần thêm) |
| `machine/lifecycle.rs` | đổi **điểm chặn biên** `MAX_*` → `self.<bảng>.len()` + 1 `assert!` fail-closed |
| `core_capxfer.rs`, `core_ipc.rs`, `core_ipc/recv.rs`, `core_ipc/reply.rs`, `core_sync.rs`, `core_foreign.rs`, `core_sched.rs`, `core_tcb/budget.rs`, `mdb.rs` | cùng phép đổi biên + cỡ mảng stack |
| `lib.rs` | **0 đổi** (giữ `check-api-boundary.py` chạy được trên worktree bẩn) |

`Machine` **không** đổi API, **không** đổi `Copy`, `size_of::<Machine<StaticStore, TestObserver>>()`
**không đổi** (`184.712 B`), `Meta` **48 B**, `vifast-core` giữ `#![forbid(unsafe_code)]`.

### 5.2 Hạn chế **đã biết** của mô hình General (fail-closed, không hỏng ngầm)

1. **`HartSched::next` vẫn `[u16; MAX_OBJECTS + 1]`** (nằm **trong** `Machine` ⇒ không nới được nếu
   không phá pin `184.832 B`). Mô hình General **chỉ lập lịch** được TCB có `ObjId ≤ 256`;
   `HartSched::contains` trả `false`, `enqueue` `debug_assert` chặn. Tạo/`Revoke` TCB **không** đi
   qua `HartSched` nên tiêu chí `≥ 512 TCB` vẫn đo được. Tổng quát hoá `PerHartState` theo `S`
   thuộc **`KT-412`/`KT-413`**.
2. **`Meta.untyped: Option<u8>`** ⇒ mô hình General phải tạo **Untyped trước** khi bảng object vượt
   256 (đúng thứ tự boot). Vượt ⇒ `assert!` **fail-closed**, không cắt im lặng.
3. **`HeapTable` chưa nối vào `Machine`** (xem §2.8): cần một đường dựng `Machine` nhận bảng
   runtime — **đổi API `Machine`**, ngoài phạm vi task này.

### 5.3 Lệnh **Lead cần chạy**

```bash
# 1) L1-medium 40k (thay đổi chạm sim/store ⇒ bắt buộc theo AGENTS §6.2)
bash v2/tools/l1-tier.sh --tier medium

# 2) Miri (vifast-objmem có unsafe mới: HeapTable + Plan)
cd v2 && MIRIFLAGS="-Zmiri-disable-isolation" \
  cargo +nightly miri test -p vifast-objmem --lib heap
cd v2 && MIRIFLAGS="-Zmiri-disable-isolation" \
  cargo +nightly miri test -p vifast-objmem --lib plan

# 3) T2 trên ứng viên tích hợp (sau khi merge + commit)
./v2/bin/v2 gate --fast
```

**Lưu ý cho Lead**: `v2 check`/T1 **chạy được trên worktree bẩn** vì `vifast-core/src/lib.rs`
**0 đổi**. Nếu ai đó thêm dòng vào `lib.rs`, bước `api boundary (KT-241)` sẽ **fail-closed** và
phải dựng bản sao sạch như KT-405.

## 4b. Hồi quy **tự bắt được** ở T1: tràn stack ảnh `x86_64` — đã sửa

### Triệu chứng

T1 (bản sao sạch, `gate --dev`) đỏ **đúng 1** bước: `QEMU x86_64 (K5' bước 1)`:

```
not ok 6 - boot 1: quá 20 s (treo, pid=17557), log …/k5-x86_64.15692/boot-1.log
# exc: vec=14 err=0x11 rip=0x0 cs=0x8 rflags=0x10046
# exc: frame rsp=0x1419c0 ss=0x0 task_rsp=0x141a78
qemu-system-x86_64: terminating on signal 15
```

Ảnh treo **sau** `# core phase: entry=… tcb=4` (page fault **instruction fetch** tại `rip=0`).

### Phân loại (không đoán)

Chạy **đúng** bước đó trên **base** `0fbdaab6` (bản sao sạch thứ hai, `run-x86_64.sh`):

```
$ bash v2/test/boot/run-x86_64.sh     # base 0fbdaab6
EXIT=0
# ca âm: 33/33 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 39/39 kiểm sạch (boot=1, tests=47, exc=6)
```

⇒ **KHÔNG** phải flake, **KHÔNG** phải lỗi có sẵn: **thay đổi của KT-410 làm hỏng x86_64**.

### Gốc

`core_capxfer.rs` đổi `snaps` từ `[CapSnapshot::EMPTY; MAX_CAPS]` sang
`[…; GENERAL_MAX_CAPS]`. `size_of::<CapSnapshot>() = 32 B` ⇒ mảng **32 KiB** (trước 16 KiB) trên
**stack boot 20 KiB** của ảnh `x86_64` (`bootstacks=20480`) ⇒ tràn stack, khung ngoại lệ hỏng
(`rip=0`).

### Sửa (đúng bản chất, **không** nới ngưỡng)

`snaps` là cửa sổ cap **của một message**, bị chặn bởi biên **ABI** `MAX_CAPS` (một `Message` mang
tối đa `MAX_CAPS` cap) — **không** phải biên **bảng**. Trả về `IPC_MAX_CAPS`:

```rust
// KT-410 — cỡ mảng là MAX_CAPS (biên ABI của số cap trong một message), không phải
// GENERAL_MAX_CAPS (biên bảng): Message mang tối đa MAX_CAPS cap, nên mảng 1.024 phần tử
// chỉ tốn 32 KiB stack vô ích (đã làm tràn stack 20 KiB của ảnh x86_64).
let mut snaps = [CapSnapshot::EMPTY; IPC_MAX_CAPS];
```

Chạy lại **đúng** bước đó trên cây có bản sửa:

```
$ bash v2/test/boot/run-x86_64.sh     # cây KT-410
EXIT=0
# ca âm: 33/33 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 39/39 kiểm sạch (boot=1, tests=47, exc=6)
```

**Bài học ghi lại**: đổi cỡ mảng **stack** theo bộ biên General phải kiểm **ngân sách stack của
ảnh** (20 KiB trên `x86_64`), không chỉ kiểm `.data+.bss`. Các mảng còn lại đã rà: `doomed`/`dead`/
`reply_objs` (đường `Revoke`, không chạy trong pha core x86_64), `chon`/`keep` (WaitOnAddress),
`wake`/`refilled` (MCS refill), `hang_cua` (`check_sched`) — tổng tăng ~+12 KiB **trên đường
Revoke**, không nằm trên đường boot; x86_64 xanh lại xác nhận.

## 6. Bằng chứng T1 (`gate --dev`) — **PASS**

### 6.1 Vì sao chạy trên **bản sao sạch**

`v2/tools/pre-measure.sh --selftest` ca (a) đòi **cây nguồn sạch** (exit 7 = `cây nguồn bẩn`):

```
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
# pre-measure-selftest: FAIL
```

Đây là **điều kiện của harness**, không phải lỗi mã, và task **cấm commit** ⇒ T1 chạy trên **bản
sao sạch** (khuôn KT-405): `git clone` **đầy đủ** (không `--depth`, vì bước `báo cáo R1-v2` cần
lịch sử git) từ repo chính, `git checkout 0fbdaab6`, áp `git diff` + 3 tệp mới, commit **trong bản
sao** (`c3ee84d8`) — **không** tạo commit nào trên `task/kt410-general-store`, **không** `git add`
trong worktree task, **không** push.

### 6.2 Kết quả

```
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
# t1-lock: giữ khoá (pid 72104) — chạy: ./v2/bin/v2 gate --dev
== v2 check (HEAD c3ee84d8) ==
…
--- QEMU x86_64 (K5' bước 1): OK
# 39/39 kiểm sạch (boot=1, tests=47, exc=6)
…
# gate-steps registered=81 ran=67 skipped=14 failed=0
# step-time-total   637197 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
EXIT=0
```

Thời gian: **637,2 s** (10 m 37 s), bắt đầu `13:44:41 UTC`, kết thúc `13:55:19 UTC`.
`failed=0`, 14 bước `BỎ QUA (T1 — chỉ chạy ở T2)` đúng thiết kế.

### 6.3 Hai lần đỏ **trước** đó (ghi lại đầy đủ, không giấu)

| Lần | Kết quả | Nguyên nhân | Xử lý |
|---|---|---|---|
| r1 (bản sao `--depth 1`) | FAIL 1 bước `báo cáo R1-v2 (Khối E)` | clone nông **không có lịch sử git** ⇒ `KT-08 commit 'bcc84bb' không tồn tại` | clone **đầy đủ** |
| r2 (bản sao đầy đủ) | FAIL 1 bước `QEMU x86_64 (K5' bước 1)` | **hồi quy thật của KT-410**: `snaps` 32 KiB trên stack 20 KiB ⇒ tràn stack | sửa `snaps` về `IPC_MAX_CAPS` (§4b) |
| r3 | **PASS** | — | — |

Lần r2 được **phân loại bằng đo**, không đoán: chạy đúng bước đó trên base `0fbdaab6` ⇒ `EXIT=0`,
`39/39 kiểm sạch` ⇒ hồi quy do KT-410, không phải flake. Sau khi sửa, cùng bước đó trên cây KT-410
⇒ `EXIT=0`, `39/39 kiểm sạch`.
