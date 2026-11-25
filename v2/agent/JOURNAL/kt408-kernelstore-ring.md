# KT-408 — Trait `KernelStore` + `Machine<S=StaticStore, O>` + Ring Trace 32 B kernel

- Nhánh: `task/kt408-kernelstore-ring` · worktree `/private/tmp/vifast-wt/2` · base `main` @ `19f0a4c3`
- Ngân sách 3 h (bắt đầu `2026-11-25 10:04 UTC`), khung Thấp điểm chính (`D2-256`).

## Kế hoạch (5–10 dòng)

1. **Đỏ trước**: test `kt408_store_phu_100_phan_tram_bang` (đòi `store.rs` + `KernelStore` + `StaticStore`
   + `Machine<StaticStore, O>` + `set_harts`) và test `kt408_ring_trace_32_b` (đòi ring 32 B ghi được,
   byte khớp `spec/abi.toml [trace]`) — chạy khi chưa có mã ⇒ ĐỎ đúng lý do (E0433/E0599).
2. `vifast-core/src/store.rs`: `Table<T, N>` (Deref/DerefMut/Index/IndexMut/Copy, `#[repr(transparent)]`)
   + trait **đóng** `KernelStore` (23 bảng: `Region`, `Meta`, `Cap`, `SlotMem`, 5 thân ABI, `QueuedMsg`,
   `WaitNode`/`AddrWord`/`wait_buckets`, `PerHart`) + `StaticStore` (mọi `MAX_*` **giữ nguyên** ở
   `machine.rs`). 0 dep ngoài, `forbid(unsafe_code)`.
3. `Machine<S: KernelStore = StaticStore, O: CoreObserver = TestObserver>`: bảng chuyển vào `store: S`,
   `Machine` giữ **nguyên** các trường vô hướng ⇒ `scalars = 311`, `size_of::<Machine>()` **không đổi**
   (pin harness 184712), `Copy` giữ nguyên. Thêm `Machine::set_harts(n)`.
4. `vifast-core/src/trace.rs`: ring 32 B `VFT1` **luôn bật** (`D2-09`), `TraceRing` là **handle ZST**
   trong `PerHartState` (0 B ⇒ 0 tăng `size_of::<Machine>()`), kho `static [AtomicU64]` đặt
   `.corespace` NOLOAD trên target `none` (`cfg_attr(target_os = "none", link_section)`) ⇒
   **0 tăng `.data+.bss`**; hook `dispatch`/`invoke`/IPC.
5. Bằng chứng: (a) test host in **hex thô** 32 B/bản ghi → `vifast-trace.py` giải mã; (b) QEMU
   `gva2gpa` + `pmemsave` vùng ring → `vifast-trace.py` giải mã.
6. 3 đột biến đỏ (tay, ghi log): (i) `KernelStore` trả bảng rỗng ⇒ test phủ bảng đỏ; (ii) ring ghi
   sai offset `kind` ⇒ test byte đỏ; (iii) `set_harts` không cập nhật `n_harts` ⇒ test đỏ.
7. `./v2/bin/v2 t0` sau mỗi bước; cuối: `./v2/bin/v2 check` + T1 qua khoá máy.
8. **Lead chạy**: `L1-medium` 10k, `v2 mutants`, T2 (`gate --fast`).

---

## 1. Đỏ trước (D2-05) — test viết trước, chạy khi chưa có mã

Tạo `v2/kernel/vifast-core/src/kt408_tests.rs` (4 ca) + khai `#[cfg(test)] mod kt408_tests;` rồi chạy:

```
$ cargo test -p vifast-core --lib kt408
error[E0432]: unresolved import `crate::store`
  --> vifast-core/src/kt408_tests.rs:21:12
error[E0432]: unresolved import `crate::trace`
  --> vifast-core/src/kt408_tests.rs:22:12
error[E0433]: cannot find `trace` in the crate root
   --> vifast-core/src/kt408_tests.rs:142:12
error[E0433]: cannot find `trace` in the crate root
   --> vifast-core/src/kt408_tests.rs:149:23
error[E0433]: cannot find `trace` in the crate root
   --> vifast-core/src/kt408_tests.rs:179:20
error[E0433]: cannot find `trace` in the crate root
   --> vifast-core/src/kt408_tests.rs:184:23
error[E0433]: cannot find `trace` in the crate root
   --> vifast-core/src/kt408_tests.rs:192:12
error[E0107]: struct takes at most 1 generic argument but 2 generic arguments were supplied
  --> vifast-core/src/kt408_tests.rs:83:19
  --> vifast-core/src/machine.rs:83:12
error[E0107]: struct takes at most 1 generic argument but 2 generic arguments were supplied
  --> vifast-core/src/kt408_tests.rs:84:19
  --> vifast-core/src/machine.rs:83:12
```

Đỏ **đúng lý do**: chưa có `store.rs`/`trace.rs`, `Machine` chỉ nhận **1** tham số kiểu. Không đỏ vì
lỗi cú pháp test.

## 2. Cài đặt

### 2.1 `vifast-core/src/store.rs` — trait đóng `KernelStore` + `StaticStore`

- `Table<T, const N: usize>` — `#[repr(transparent)]` trên `[T; N]`, `Deref/DerefMut → [T]`.
  Nhờ vậy **0 sửa** ~250 điểm truy cập cũ (`self.objects[i]`, `self.caps.get_mut(..)`,
  `self.harts.iter_mut()`, `&self.ipc_msgs` ép `&[QueuedMsg]`) và `size_of` **không đổi**.
- `trait KernelStore: sealed::Sealed + Copy + Debug` — **23 bảng** (đúng bảng `Machine` khai trước
  KT-408): `Regions`, `Objects`, `Caps`, `Slots`, `Harness`, `Tcbs`, `TcbFree`, `Scs`, `ScFree`,
  `Endpoints`, `EndpointFree`, `Replies`, `ReplyFree`, `Notifications`, `NotificationFree`,
  `IpcMsgs`, `IpcMsgFree`, `AddrWords`, `WaitNodes`, `WaitFree`, `WaitBuckets`, `WaitBucketsTail`,
  `Harts` + 23 hằng `*_EMPTY` (đường dựng **`const fn`** cho `.bss`).
- Hai trait phụ cho bảng **đơn**: `SlotWindow` (`get`/`set`/`fill_zero`) và `HarnessArena`
  (`probe`/`alloc`) — `KT-410` cấp cửa sổ slot từ `BootInfo` Arena mà `Machine` không đổi dòng nào.
- `StaticStore` — **ZST 0 B**, `const EMPTY`; `const` assert `size_of::<StaticStore>() == 0` và
  `size_of::<Table<Meta, 256>>() == size_of::<[Meta; 256]>()`.
- **Đóng** (`sealed`): chỉ `vifast-core` cài được ⇒ không ai thêm mô hình thứ ba làm lệch ngữ nghĩa.
- **0 dep ngoài**; `#![forbid(unsafe_code)]` của crate áp nguyên.

### 2.2 `Machine<S: KernelStore = StaticStore, O: CoreObserver = TestObserver>`

- Bảng **vẫn là trường của `Machine`**, chỉ đổi **kiểu** sang kiểu liên kết của `S`
  (`objects: S::Objects` = `Table<Meta, 256>` với `StaticStore`) ⇒ `Machine` **không** mang thêm
  trường `store` nào, `Copy` giữ nguyên, `size_of::<Machine>()` **giữ đúng 184712**.
- `MAX_*` **giữ nguyên** ở `machine.rs` (`check_kernel.py` đọc theo **tên** `machine.rs#MAX_TCBS`…).
- Thêm `Machine::set_harts(n)` + `n_harts()`: đổi số hart **tại chỗ** (không dựng lại sổ cái), reset
  `PerHartState` của hart mới mở, kẹp `current_hart`; panic ngoài `1..=MAX_HARTS` như `with_harts`.
- `MissionMachine` = `Machine<StaticStore, NoopObserver>` (171.448 B) — dựng được, nhỏ hơn bản test.

### 2.3 Ring trace 32 B luôn bật (`D2-09`)

- `v2/lib/vifast-traceformat/src/kernel_ring.rs` (**mới**): kho `static [AtomicU64]` 8 KiB +
  `HEADS`/`COUNTS`, `#[cfg_attr(target_os = "none", link_section = ".corespace")]` ⇒ section
  **NOLOAD `.corespace`** (cùng section với `Machine`) ⇒ **0 tăng `.data+.bss`**.
- **Vì sao kho ring ở crate `vifast-traceformat`, không ở `vifast-core`**: `vifast-core` là
  `#![forbid(unsafe_code)]` (`D2-17`) và từ Rust 1.82 `link_section` là **thuộc tính unsafe** ⇒
  `forbid` **từ chối** (đo được: `error: usage of the unsafe link_section attribute`). Crate
  `vifast-traceformat` **sở hữu** định dạng 32 B từ `KT-12`/`KT-403` (`D2-38`/`D2-42`) nên là chỗ
  đúng; `vifast-core` chỉ gọi API an toàn ⇒ **giữ nguyên** `forbid(unsafe_code)`.
- `vifast-core/src/trace.rs`: `TraceKind` (1..7 theo `spec/abi.toml [trace].kind`), tái xuất
  `TraceRecord = vifast_traceformat::Record`, `TraceRing` — **handle ZST 0 B** trong
  `PerHartState` ⇒ `size_of::<Machine>()` **không đổi** (pin harness 184712 **không** phải sửa).
- Hook **luôn bật**: `Machine::dispatch` (kind `sched`), `invoke::dispatch` (kind `syscall`),
  `Machine::send`/`recv` (kind `ipc`); `init_zeroed` zero ring tường minh (NOLOAD không được loader
  zero).
- `const` assert: `size_of::<TraceRecord>() == 32`, `size_of::<TraceRing>() == 0`,
  `kernel_ring::MAX_HARTS == vifast_core::MAX_HARTS`.

### 2.4 Vá race **có sẵn** của bộ test host (không nới ngưỡng)

`cargo test -p vifast-core --lib` lặp 25 lượt: **1 lượt ĐỎ**
`resume_tests::resume_inactive_chuyen_running_va_giao_entry` (`stats().resume == base + 1`). Nguyên
nhân: 4 mô-đun test mỗi nơi một `Mutex` riêng nhưng **cùng** đọc bộ đếm `static` toàn cục
`invoke::stats()` ⇒ hai mô-đun vẫn chạy song song. Thêm `src/test_lock.rs` (`STATS_LOCK` dùng chung,
`#[cfg(test)]`) và trỏ 4 khoá cũ về nó + thêm guard cho ca `cnode_ca_am_ma_loi` (khẳng định **đúng**
`denied0 + 2`). **Giữ nguyên** mọi khẳng định `==` (không nới ngưỡng). Đo lại: **25/25 XANH**.

## 3. Bằng chứng đo được

### 3.1 T0 — `cargo test -p vifast-core --lib`

```
$ cargo test -p vifast-core --lib
test result: ok. 112 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

4 ca KT-408 mới:
```
test kt408_tests::kt408_machine_giu_co_copy_va_meta_48_b ... ok
test kt408_tests::kt408_ring_trace_32_b ... ok
test kt408_tests::kt408_set_harts ... ok
test kt408_tests::kt408_store_phu_100_phan_tram_bang ... ok
```

`kt408_machine_giu_co_copy_va_meta_48_b` khẳng định **`size_of::<Machine<StaticStore, TestObserver>>() == 184712`**
⇒ pin harness `D2-84`/`D2-252` **không phải sửa một dòng nào** (khác KT-405 phải sửa 6 dòng ghim).

`check_kernel.py` (python3.12): `kernel-check: PASS (37 giá trị đối chiếu theo tên hằng)` ⇒ `MAX_*`
giữ nguyên.

`vifast-trace.py --selftest`: `== SELFTEST vifast-trace: OK — 9/9 (bản ghi 32 B, header VFT1) ==`.

### 3.2 3 đột biến ĐỎ (chạy trên **bản sao sạch** `/tmp/kt408-t1`, không sửa worktree task)

| # | Đột biến | Kết quả |
|---:|---|---|
| M1 | `Table::deref` trả **slice rỗng** (`&self.0[..0]`) ⇒ store không phủ bảng | **ĐỎ** `kt408_store_phu_100_phan_tram_bang` — `assertion left == right failed: regions` |
| M2 | ring ghi **sai offset** `kind` (`<<16` → `<<24`) | **ĐỎ** `kt408_ring_trace_32_b` — `kind@10 = run (6)`: `left: 1536 right: 6` |
| M3 | `set_harts` **không** cập nhật `n_harts` | **ĐỎ** `kt408_set_harts` — `set_harts(4) phải đổi n_harts`: `left: 1 right: 4` |

### 3.3 T1 — `gate --dev` trên **bản sao sạch** (bắt buộc: `check-api-boundary` fail-closed từ chối
chạy khi `vifast-core/src/lib.rs` **bẩn** — cùng tình huống `KT-405` §6.1)

Bản sao: `git clone --depth 1 --branch task/kt408-kernelstore-ring file:///private/tmp/vifast-wt/2
/tmp/kt408-t1` @ `19f0a4c3` + `git apply /tmp/kt408.patch` + 6 tệp mới + commit **trong bản sao**
(`1203771`, **không** tạo commit nào trên nhánh task, **không** `git add` trong worktree task).

```
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
# gate-steps registered=47 ran=47 skipped=0 failed=1
# step-time-total   233579 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**46/47 bước XANH, 0 bước đỏ vì mã.** Bước đỏ **duy nhất**:

```
--- báo cáo R1-v2 (Khối E): FAIL
not ok - R1-v2.json: KT-09 status 'BLOCKED' ≠ QUEUE 'DONE' (thiếu/sai trường 'queue')
…
not ok - R1-v2.json: thiếu task trong báo cáo: ['KT-100', … 'KT-99']
# 43 mục sai
```

**Đây là ĐỎ CÓ SẴN trên `main` @ `19f0a4c3`, không liên quan KT-408** — chứng minh bằng cách chạy
đúng bước đó trên **cây base sạch** (`git worktree add /tmp/kt408-base 19f0a4c3`):

```
$ python3.12 v2/test/report/check_report.py --all      # trên 19f0a4c3 NGUYÊN BẢN
not ok - R1-v2.json: KT-422 (BLOCKED) không xuất hiện trong blocked[]
not ok - R1-v2.json: KT-423 (BLOCKED) không xuất hiện trong blocked[]
not ok - R1-v2.json: KT-424 (BLOCKED) không xuất hiện trong blocked[]
# 43 mục sai
```

`v2/agent/reports/R1-v2.json` cập nhật lần cuối ở `c6487a95` (**thời KT-05**), không chứa `KT-408`
(`grep -c KT-408` = 0), trong khi `QUEUE.md` đã đi tới `19f0a4c3`. Patch KT-408 **không chạm**
`QUEUE.md` lẫn `reports/` ⇒ bước này đỏ y nguyên trước và sau KT-408.

## 4. Việc còn lại / bàn giao cho Lead

1. **`báo cáo R1-v2 (Khối E)` đỏ có sẵn trên base** — cần Lead/Agent 0 quyết: sinh lại
   `v2/agent/reports/R1-v2.json` (43 mục lệch) hoặc tách task riêng. **Không** phải lỗi KT-408;
   T1 của KT-408 xanh 46/47 bước còn lại.
2. **Lead chạy** (theo phân chia): `bash v2/tools/l1-tier.sh --tier medium` (10k seed),
   `./v2/bin/v2 mutants` (không `--all`), rồi T2 `./v2/bin/v2 gate --fast`.
3. **Bằng chứng `pmemsave` + `vifast-trace`**: ring nằm trong `.corespace` NOLOAD; địa chỉ lấy từ
   `llvm-nm` ký hiệu `vifast_traceformat::kernel_ring::RING`, dịch VA→PA bằng `gva2gpa` của QEMU
   monitor rồi `pmemsave <pa> <n> /tmp/ring.bin` + `python3 v2/tools/vifast-trace.py /tmp/ring.bin`.
   **Chưa chạy được trong ngân sách 3 h** (T1 chiếm 233 s + dựng bản sao sạch); byte-level đã được
   chứng minh ở test host `kt408_ring_trace_32_b` (đúng offset `spec/abi.toml [trace]`).
4. **`v2/test/mutants.toml`**: **không** thêm mục — 3 đột biến ở §3.2 là bằng chứng tay; thêm mục
   sẽ đổi `declarations = 43` (lint `E_MANIFEST` đòi `covered == declarations`) nên cần Lead quyết.
5. **Nợ chuyển tiếp**: `KT-410` cài `UntypedStore` **chỉ cần** cài `KernelStore` + `SlotWindow` +
   `HarnessArena` (không sửa `Machine`); `KT-412` dùng `Machine::set_harts(n)` từ `BootInfo`.
