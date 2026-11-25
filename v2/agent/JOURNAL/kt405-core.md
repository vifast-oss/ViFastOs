# KT-405 — Tiền đề Lõi Thuần: `CoreObserver` + `PerHartState` + Ready Queue xâm nhập + `TcbCore`/`EndpointCore` + dọn clippy `vifast-core`

**Nhánh**: `task/kt405-core` (worktree `/private/tmp/vifast-wt/4`, base `6d44167e`)
**Phạm vi ghi**: chỉ `v2/kernel/vifast-core/src/` + tệp journal này. **0** chạm `spec/`, `docs/`, `v2/user/`, 3 crate arch.
**Trạng thái**: mã xong. `clippy -D warnings` **0 cảnh báo**; T0 PASS; 108/108 unit test; parity sim
**30/30**; `vifast-plat-common` 13 suite xanh; ảnh 3 arch build xanh; **T1 PASS** (`gate --dev`,
`registered=76 ran=62 skipped=14 failed=0`, 621,6 s) — T1 chạy trên **bản sao sạch** của chính nhánh
này vì `check-api-boundary` fail-closed trên cây bẩn (§6.1), **cộng 6 dòng ghim harness** phải áp
(§6.3, ngoài phạm vi ghi). L1 digest **trùng** `3675a4a2ef394150`.
**Không commit** (Lead commit).

---

## 1. Vì sao có task này (bối cảnh)

`v2/Cargo.toml` **loại** `kernel` khỏi workspace host (`exclude = ["kernel"]`) ⇒ `cargo clippy` của cổng
chưa bao giờ chạy trên `vifast-core`. D2-260 mục 3 ghi nhận **15 cảnh báo `clippy -D warnings` có sẵn**
trong crate này và giao dọn ở KT-405 (dọn `plat-*` + nối bước clippy kernel vào `steps_check.sh` là KT-409).

Đồng thời KT-405 là **tiền đề dữ liệu** cho `KT-408` (`KernelStore` + `Machine<S, O>`) và `KT-410`
(`UntypedStore` đặt thân ABI **tại chỗ** trong `Untyped`): phải tách dữ liệu **oracle/test** ra khỏi
sổ cái sản phẩm, tách trạng thái **theo hart** khỏi sổ cái object, và chứng minh thân ABI vừa
`spec/abi.toml:159` (`tcb = 512`, `endpoint = 64`).

---

## 2. Kết quả đo (trước → sau)

Đo bằng `cargo test -p vifast-core -- --nocapture kt405_than_abi` (in trong test, không viết tay):

| Hạng mục | Trước KT-405 | Sau KT-405 | Ghi chú |
|---|---:|---:|---|
| `size_of::<Machine>()` | **184.792 B** | **184.712 B** | **−80 B**; pin D2-84 `196.608` **không nới**, trần KT-312 `184.832` vẫn dưới |
| `size_of::<Machine<NoopObserver>>()` | — (không có) | **171.448 B** | **−13.264 B** = đúng `size_of::<TestObserver>()` ⇒ observer sản phẩm **0 B** |
| `size_of::<TestObserver>()` | — (dữ liệu nằm trong `Machine`) | **13.264 B** | `pair_hits[121]` 484 + `forbidden_hits[121]` 484 + `finalise_log[256]` 12.288 + `n_finalise` 2 (+2 đệm) |
| `size_of::<Meta>()` | 48 B | **48 B** | **không đổi** (KT-312 giữ nguyên) |
| `size_of::<TcbCore>()` | — (không có) | **280 B** | `const` assert `≤ 496` (spec `tcb = 512` − 16 B header) |
| `size_of::<TcbData>()` | 824 B | **824 B** | `TcbCore` 280 + `TcbModel` 544 — **không đổi** tổng |
| `size_of::<EndpointCore>()` | — (không có) | **48 B** | `const` assert `≤ 48` (spec `endpoint = 64` − 16 B header) |
| `size_of::<EndpointData>()` | 80 B | **64 B** | `EndpointCore` 48 + `EndpointModel` 16 |
| `size_of::<ScData>()` | 64 B | 64 B | `≤ 240` (spec `sched_context = 256`) |
| `size_of::<NotificationData>()` | 32 B | 32 B | spec `notification = 64` |
| `size_of::<ReplyData>()` | 8 B | 8 B | spec `reply = 64` |
| `Footprint::scalars` | 311 | **311** | **không đổi** — pin `kt60_core_parity` phía sim |
| `Footprint::padding` | 1 | **1** | `≤ 8` (D2-85c) |
| `Footprint::FOOTPRINT_PARTS` | 29 | **26** | 4 bảng rời `sched`/`mail`/`published`/`tlb_pending` → 1 `perhart[8]` |
| L1 smoke digest (1.000 seed) | `3675a4a2ef394150` | `3675a4a2ef394150` | **TRÙNG** — 0 đổi ngữ nghĩa |

**Đại số `size_of::<Machine>()` (đại số có dấu, không nới pin)** — `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb`:

```
PIN_CU (KT-74)                188.872
+ KT-282  Option<IpcBufferBinding>  24 × 64 = +1.536
+ KT-309  Option<u16> mỗi TCB         8 × 64 = +512
+ KT-309  Message lớn thêm 128 B    128 × 24 = +3.072
+ KT-309  hai bộ đếm u64                        +16
+ KT-311  TcbData.entry u64            8 × 64 = +512
− KT-312  Meta 88 → 48 B              40 × 256 = −10.240
− KT-405  (a) gộp PerHartState (6 B đệm × 8 hart)      +48
          (b) HartSched.len usize → u16 (8 B × 8)      −64
          (c) EndpointData 80 → 64 B (16 × 16)        −256
          (d) QueuedMsg.next u16 (8 B × 24)           +192
          ⇒ KT-405 giảm ròng                            −80
= 184.712 B  (khớp tuyệt đối, test in `khop=1`)
```

---

## 3. Việc đã làm (theo 6 ưu tiên của task)

### 3.1 Dọn sạch clippy `-D warnings` (ưu tiên 1)

```
cargo clippy --manifest-path v2/kernel/Cargo.toml -p vifast-core --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.89s
EXIT=0
```

**0 cảnh báo.** 15 cảnh báo có sẵn đã dọn (đúng danh sách D2-260 mục 3):

| # | Lint | Tệp | Cách dọn |
|---:|---|---|---|
| 1–2 | `needless_range_loop` | `check.rs` (`check_bytes` ×2) | `for (i, o) in self.objects[..n].iter().enumerate()` |
| 3–5 | `needless_range_loop` | `check_ipc.rs` (`reply_object`/`notification_object`/`endpoint_object`) | `iter().enumerate()` |
| 6 | `needless_range_loop` | `core_sched.rs` (`check_migrate`) | `self.harts[..n].iter().map(\|h\| &h.published).enumerate()` |
| 7 | `needless_range_loop` | `core_sync.rs` (`addr_word`) | `for w in &self.addr_words[..n]` |
| 8 | `needless_range_loop` | `core_sync.rs` (`set_addr_word`) | `for w in &mut self.addr_words[..n]` |
| 9 | `needless_range_loop` | `core_finalise.rs` (`clear_dead_reply_in_waiters`) | `for e in &mut self.endpoints[..n]` |
| 10 | `needless_range_loop` | `mailbox.rs` (`retain`) | `for slot in self.buf.iter_mut().take(CAP).skip(w)` |
| 11–13 | `needless_range_loop` | `tlb.rs` (`tlb_shootdown_begin` ×2, `tlb_wait`) | `self.harts[..n].iter().map(\|h\| &h.tlb_pending).enumerate()` |
| 14 | `derivable_impls` | `core_tcb.rs` (`VspaceUpdate`) | `#[derive(Default)]` + `#[default]` trên `Keep` |
| 15 | `dead_code` | `suspend_tests.rs` | xoá trường `caller_slot` không dùng |

### 3.2 `trait CoreObserver` + `NoopObserver` 0 B (ưu tiên 2)

Tệp mới **`observer.rs`**:

- `trait CoreObserver: Copy` — 11 phương thức: `pair_hit`/`pair_hit_count`/`forbidden_hit`/
  `forbidden_attempt_count`/`coverage`/`design_coverage`/`finalise_is_full`/`finalise_push`/
  `finalise_log`/`footprint_parts` + `const EMPTY` (cần cho `Machine::with_observer` là `const fn`).
- `NoopObserver` — **ZST**, mọi thao tác no-op; `finalise_is_full()` luôn `false` ⇒ sản phẩm **không
  bao giờ** từ chối `finalise_tcb` vì nhật ký (sản phẩm không có nhật ký).
- `TestObserver` — giữ **nguyên** ba bảng cũ (`pair_hits[121]`, `forbidden_hits[121]`,
  `finalise_log[256]`) + `n_finalise: u16` (thay `usize` để `TestObserver` align 4, không đẩy
  `Machine` lên đệm 8 B).
- `const _: () = assert!(size_of::<NoopObserver>() == 0)` — chạy **cả khi build bare-metal**.
- Test `kt405_noop_observer_0_b_va_machine_san_pham_nho_hon` chứng minh
  `size_of::<Machine<TestObserver>>() − size_of::<Machine<NoopObserver>>() == size_of::<TestObserver>()`.

**Quyết định có ghi**: `finalise_overflow: u32` **KHÔNG** chuyển vào observer — nó là bộ đếm chẩn đoán
mà **sản phẩm cũng có** (luôn 0), không phải dữ liệu oracle. Giữ nó trong `Machine` (đặt ngay sau
`current_hart: u16` để **lấp** 4 trong 6 byte đệm sẵn có) ⇒ `Footprint::scalars` giữ **311**, khớp pin
`kt60_pin_dau_chan_d2_84` của sim. Nếu chuyển vào observer thì `scalars = 307` và **L0 đỏ** ở tệp
`v2/sim/vifast-sim/tests/kt60_core_parity.rs` (ngoài phạm vi ghi của task).

### 3.3 `PerHartState` (ưu tiên 3)

Tệp mới **`perhart.rs`**: gom `sched`/`published`/`mail`/`tlb_pending` thành **một** khối
`[PerHartState; MAX_HARTS]` (`Machine.harts`), thay bốn bảng rời. `PerHartState::new()` là `const fn`
(dùng cho `.bss`), `reset()` khởi tạo tại chỗ (KT-61). Đây là **ranh giới dữ liệu** để KT-412/413 hạ
khoá từ "toàn `Machine`" xuống "một hart" — KT-405 **chưa** đổi khoá (vẫn `IrqMcsLock<Machine>` ở
`plat-*`), đúng phạm vi "0 sửa API 3 arch".

### 3.4 Ready queue xâm nhập + `TcbCore`/`EndpointCore` (ưu tiên 4)

- **`IpcQueue`** (`objstate.rs`): bỏ `slots: [u16; 8]` (16 B), thay bằng `head`/`tail` (`u16`) và mắt
  xích **xâm nhập** `QueuedMsg.next: u16` (`QUEUE_NONE = u16::MAX`). `IpcQueue` 48 → **32 B**.
  `try_push`/`pop_front`/`iter`/`kept`/`invariant` nhận thêm `pool: &[QueuedMsg]`; `invariant` còn
  kiểm **chuỗi mắt xích không đứt/không vòng và đếm đúng `len`** (mạnh hơn bản cũ).
- **`EndpointCore`** = `recv` + `send` (**48 B**), **`EndpointModel`** = `last_mr` (16 B),
  `EndpointData` = `core` + `model` (**64 B**) với `Deref`/`DerefMut` sang `EndpointCore` ⇒ mọi truy
  cập `ep.recv`/`ep.send` **giữ nguyên**, `last_mr` đọc qua `ep.model.last_mr`.
- **`TcbCore`** = mọi trường **trừ** `IpcArea` (**280 B**), **`TcbModel`** = `ipc: IpcArea` (544 B),
  `TcbData` = `core` + `model` (**824 B**, không đổi tổng) với `Deref`/`DerefMut` sang `TcbCore`.
- `const` assert: `size_of::<TcbCore>() <= 496`, `size_of::<EndpointCore>() <= 48` — **chạy cả khi
  build bare-metal**, không chỉ `cargo test`.

### 3.5 Gom truy cập `Machine` về accessor (ưu tiên 5)

`Machine` thành `Machine<O: CoreObserver = TestObserver>`; **mọi** khối `impl Machine` (21 tệp) thành
`impl<O: CoreObserver> Machine<O>`. Bốn bảng rời chỉ còn truy cập qua `self.harts[h].{sched,published,
mail,tlb_pending}`; ba bảng oracle chỉ qua `self.obs.*`. Các iterator công khai (`QueuedIter`, `MsgIds`,
`SendItems`, `TcbIds`, `Waiters`) nhận thêm tham số `O` với **mặc định** `TestObserver` ⇒ chữ ký cũ
vẫn dùng được.

**Hàm dựng không cần chú thích kiểu** (bài học quan trọng): tham số kiểu **mặc định** chỉ áp ở **vị trí
kiểu**, **không** áp ở **vị trí biểu thức** ⇒ `Machine::new()` cho `E0283`. Đã tách đúng khuôn
`HashMap::new()` của std:

- `impl Machine<TestObserver> { new(), with_harts(n), footprint() }` — dùng cho **mọi** nơi gọi cũ
  (sim parity, `plat-common` test, `core_bridge` của ảnh sản phẩm) ⇒ **0 sửa** ngoài crate;
- `impl<O: CoreObserver> Machine<O> { with_observer(n), footprint_of() }` — đường **generic** cho
  `Machine::<NoopObserver>`.

### 3.6 Giữ `Meta == 48 B` & L1 digest 100 % (ưu tiên 6)

`Meta` **48 B** (assert trong `footprint::tests` + `tls_tests`). L1 smoke digest **trùng** (§2).

---

## 4. Tệp đã sửa

**Mới (2)**: `observer.rs`, `perhart.rs`.

**Sửa (37)**: `lib.rs`, `machine.rs`, `machine/init.rs`, `machine/lifecycle.rs`, `machine_slot.rs`,
`objstate.rs`, `tcbdata.rs`, `footprint.rs`, `sched.rs`, `mailbox.rs`, `mdb.rs`, `tlb.rs`, `check.rs`,
`check_ipc.rs`, `core_capxfer.rs`, `core_finalise.rs`, `core_foreign.rs`, `core_ipc.rs`,
`core_ipc/{queue,send,recv,reply}.rs`, `core_sched.rs`, `core_sync.rs`, `core_tcb.rs`,
`core_tcb/{budget,configure}.rs`, `invoke.rs`, `state_write.rs`, `syscall.rs`, `capalloc_tests.rs`,
`configure_tests.rs`, `kt345b_tests.rs`, `nb_queue_tests.rs`, `resume_tests.rs`, `suspend_tests.rs`,
`tls_tests.rs`.

**0 tệp ngoài `v2/kernel/vifast-core/src/`** (trừ tệp journal này).

---

## 5. Bằng chứng T0

```
$ ./v2/bin/v2 t0
test result: ok. 108 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
v2 t0: PASS (1 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
EXIT=0
```

Parity với sim (oracle) — **30/30 xanh**:

```
$ cargo test -p vifast-sim --test kt58_core_parity --test kt59_core_parity --test kt60_core_parity
kt58: test result: ok.  4 passed; 0 failed
kt59: test result: ok.  8 passed; 0 failed
kt60: test result: ok. 18 passed; 0 failed   (gồm kt60_pin_dau_chan_d2_84 — pin scalars=311)
```

`vifast-plat-common` (host test, dùng `Machine::with_harts`): **13 suite, 0 đỏ**.

Ảnh sản phẩm **3 arch build xanh** với thay đổi này:
`vifast-kbin-riscv64` (release, `riscv64gc-unknown-none-elf`) 10,33 s;
`vifast-kbin-aarch64` 100.344 B; `vifast-kbin-x86_64` 94.960 B.

---

## 6. Bằng chứng T1 (`gate --dev`)

### 6.1 Vì sao T1 **không thể** chạy trên worktree bẩn

`v2/tools/check-api-boundary.py` (KT-241) **chèn tạm** `mod kt241_probe;` vào
`v2/kernel/vifast-core/src/lib.rs` rồi `cargo check` để chứng minh ranh giới API bằng compile-fail.
Nó **fail-closed** nếu tệp đó đã bẩn:

```
check-api-boundary: TỪ CHỐI — tệp sẽ chạm đã bẩn — từ chối chạy (fail-closed), không ghi đè:
M v2/kernel/vifast-core/src/lib.rs
--- api boundary (KT-241): FAIL
```

KT-405 **bắt buộc** sửa `lib.rs` (khai báo `mod observer;`/`mod perhart;`) ⇒ trên cây **chưa commit**
bước này luôn đỏ. Đây là **điều kiện của harness**, không phải lỗi mã. Luật của task là **không commit**
(Lead commit) ⇒ T1 được chạy trên **bản sao sạch** (clone `--depth 1` của chính nhánh này tại
`6d44167e`, áp đúng patch `git diff` + 2 tệp mới, commit **trong bản sao** — **không** tạo commit nào
trên `task/kt405-core`, **không** `git add` trong worktree task, **không** push).

### 6.2 Kết quả — **PASS** sau khi cập nhật 6 dòng ghim harness (ngoài phạm vi ghi của task)

```
$ ./v2/bin/v2 gate --dev
--- QEMU aarch64 (K4' bước 3): OK
--- QEMU x86_64 (K5' bước 1): OK
--- QEMU smoke (T1: 7 ảnh × 1 boot): OK
# gate-steps registered=76 ran=62 skipped=14 failed=0
# step-time-total   621567 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
EXIT=0
```

Đáng chú ý: **`api boundary (KT-241)` XANH** trên cây sạch —
`vifast-core: ca âm (20 ca) OK · ca dương OK`, `vifast-sim: ca âm (20 ca) OK · ca dương OK`
⇒ việc `Machine` thành generic **không** mở đường lọt `TcbState::Ready` vào cửa chuyển trạng thái.

### 6.3 Sáu dòng ghim harness **phải** cập nhật (ngoài `v2/kernel/vifast-core/src/`)

KT-405 đổi `size_of::<Machine>()` `184.792 → 184.712` (−80 B) và `FOOTPRINT_PARTS` `29 → 26`.
Harness **ghim cứng** hai giá trị này ở 6 dòng ⇒ T1/T2 đỏ **oan** cho tới khi cập nhật:

| # | Tệp:dòng | Cũ | Mới |
|---:|---|---|---|
| 1 | `v2/test/boot/lib/budget-common.sh:54` | `budget_common_MACHINE=184792` | `=184712` |
| 2 | `v2/test/boot/lib/budget-common.sh:303` | `budget_common_corespace_gate 184792 && ! … 184791` | `184712` / `184711` |
| 3 | `v2/test/boot/run-aarch64.sh:2246,2247` | `size=184792` | `size=184712` |
| 4 | `v2/test/boot/run-x86_64.sh:1684,1685` | `size=184792` | `size=184712` |
| 5 | `v2/test/boot/ktest.sh:600` | `bytes=184792 … parts=29 sum=184792` | `bytes=184712 … parts=26 sum=184712` |
| 6 | `v2/test/boot/ktest.sh:613,645,829` | `static=184792` | `static=184712` |

Ghi chú: `soak-aarch64.sh:876` / `soak-x86_64.sh:815` là **dòng log mẫu trong chú thích** (không
assert) — nên cập nhật cho đúng tài liệu, không ảnh hưởng cổng. Bốn chỗ `184792` còn lại trong
`budget-common.sh` (280/321/322/330) là **dữ liệu self-test** (`corespace=184792 ≥ 184712` vẫn XANH)
— không cần đổi.

**Bằng chứng T1 ở trên lấy từ bản sao sạch đã áp 6 dòng ghim này** (xem §6.1). Trên worktree task
(chưa commit, chưa áp ghim) T1 đỏ đúng 2 bước QEMU vì lý do ghim, **không** vì mã.

---

## 7. Phần còn nợ (bàn giao)

0. **BẮT BUỘC trước khi T2 xanh**: áp 6 dòng ghim harness ở §6.3 (`184792 → 184712`,
   `parts=29 → parts=26`). Không áp thì T1/T2 đỏ ở `QEMU aarch64`, `QEMU x86_64`, `QEMU smoke` —
   **không** phải lỗi mã. Đã kiểm chứng: áp 6 dòng đó ⇒ **T1 PASS 76 bước, 0 đỏ**.
1. **Nối `Machine<NoopObserver>` vào ảnh sản phẩm** — `v2/kernel/vifast-plat-riscv64/src/core_bridge.rs`
   hiện dùng `Machine` (= `Machine<TestObserver>`) nên ảnh sản phẩm **vẫn trả 13.264 B** cho dữ liệu
   oracle. Đổi `static mut MACHINE: MaybeUninit<Machine<NoopObserver>>` + `Machine::<NoopObserver>::with_observer(n)`
   là **1 dòng** nhưng nằm ngoài phạm vi ghi của KT-405 (`v2/kernel/vifast-core/src/`). Đề nghị giao
   **KT-409** (đã chạm `plat-*`) hoặc **KT-410**. Bằng chứng sẵn: `size_of::<Machine<NoopObserver>>() = 171.448 B`.
2. **`EndpointStats`** trong nguyên văn QUEUE: **không tồn tại** trong `vifast-core` (grep 0 hit) — chỉ
   có `EndpointInfo` (model API) và `EndpointModel.last_mr`. Đã tách `last_mr` sang `EndpointModel`
   (phần mô phỏng) ⇒ tinh thần DoD "rút phần mô phỏng Endpoint ra khỏi thân ABI" **đã đạt**; tên gọi
   trong QUEUE là tên cũ/khác nguồn.
3. **`IpcArea` chưa vào `CoreObserver`** — đã tách khỏi `TcbCore` sang `TcbModel` (đạt mục tiêu "thân
   ABI ≤ 496 B"), nhưng vẫn nằm trong `Machine` (544 B × 64 TCB = 34.816 B). Đưa hẳn vào observer/store
   là việc của **KT-408/KT-410** (thiết kế đã ghi trong doc-comment `TcbModel`).
4. **`run-aarch64.sh`/`run-x86_64.sh` không tôn trọng `CARGO_TARGET_DIR`** (hardcode
   `$V2_ROOT/kernel/target`) trong khi `product-{aarch64,x86_64}.sh` **có** tôn trọng. Hệ quả: T1/T2
   chạy trong worktree có `CARGO_TARGET_DIR` (đúng quy ước `wt.sh`) sẽ **đỏ oan** ở 2 bước QEMU
   aarch64/x86_64 ("không thấy ảnh sản phẩm") dù ảnh **đã build xanh**. Vá 2 dòng `IMG_DIR` như
   `product-*.sh` — thuộc **KT-407** (đại tu tooling/runner).
5. **`v2/Cargo.toml` vẫn `exclude = ["kernel"]`** ⇒ clippy của cổng **chưa** phủ `vifast-core`. Nối
   bước clippy kernel vào `steps_check.sh` là **KT-409** (theo D2-260 mục 3, chỉ sau khi KT-405 sạch —
   **đã sạch**).
