# KT-336c — vá sót `riscv64` của KT-336a: `IrqMcsLock` bọc `sstatus.SIE`, TAP khoá/ngắt, mục `ObjStore`

**Chu kỳ 2 — Đợt 1 (bổ sung).** Người làm: sub-agent `syncrv`. Ngày: 2026-11-22 (giờ máy Asia/Tokyo).
Quyết định: **D2-243(1)** (mở `KT-336c`), **D2-232(2)** (ranh giới `ObjStore`), **D2-238(2)** (bẫy
`McsLock::lock()` không tắt ngắt + `Mailbox` cho kênh wake/IPI), **D2-05** (đỏ trước).

| | |
|---|---|
| Worktree | `/tmp/kt336c-s1` (nhánh `task/kt336c-riscv`) |
| Base | `main` @ `d0c5817c` (`[KT-340a] sửa chú thích chốt số bước T2`) |
| Commit mã + test + tài liệu | `2d4339d2` — `[KT-336c] riscv64: IrqMcsLock bọc sstatus.SIE (thay LOCKED) + TAP 70 khoá/ngắt + mục ObjStore` (**mọi cổng đo trên đúng commit này**) |
| Commit journal + log | commit kế tiếp, **chỉ** thêm tệp dưới `v2/agent/JOURNAL/` — không nằm trong `VBREV_INPUT_ROOTS` (`v2/kernel v2/lib`) nên `build-inputs` **không đổi** (`fbb2bed135fc4236`) |
| Máy | 12 nhân (`hw.ncpu=12`), macOS, Rust/cargo `1.100.0` (`v2/rust-toolchain.toml`) |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`, chạy trong `/tmp/kt336c-s1` |
| Nhãn số liệu | **`[ĐO KT-336c]`** — đo trực tiếp từ log cổng/QEMU trong worktree này; log lưu ở `v2/agent/JOURNAL/kt336c-logs/` |
| QEMU | mọi lần boot qua `v2/test/boot/qemu_guard.sh` (D2-116); **không** `pkill -f qemu-system` |

---

## 1. Khảo sát (trước khi sửa)

### 1.1 `core_bridge::LOCKED` — ở đâu, ai dùng, có tắt ngắt không

Tệp `v2/kernel/vifast-plat-riscv64/src/core_bridge.rs` **303 dòng** tại base. Trước `KT-336c`:

| Vị trí | Nội dung | Ghi chú |
|---|---|---|
| dòng 72 | `static LOCKED: AtomicBool = AtomicBool::new(false);` | spinlock tự chế, **không** tắt ngắt |
| dòng 128–139 | `fn lock()` (CAS quay vòng `compare_exchange_weak`) + `fn unlock()` (`store(false)`) | thứ tự `Acquire`/`Release` |
| dòng 146–159 | `with_machine` → `lock() … f(m) … unlock()` | cấp `&mut Machine` duy nhất |
| dòng 169–205 | `init` → `lock() … unlock()` quanh zero + `init_zeroed` + `add_untyped` | |

Người dùng `LOCKED`: **chỉ** hai hàm trên, cả hai trong chính tệp (grep `LOCKED` toàn repo =
3 dòng). Không có đường lấy khoá này trong handler ngắt — nhưng cũng **không** có gì cấm handler
gọi `with_machine`, nên bẫy D2-238 vẫn mở: hart đang giữ khoá bị ngắt trên chính nó ⇒ handler
`with_machine` ⇒ tự deadlock. Đây đúng phần sót mà D2-243 nói.

### 1.2 riscv64 đã có gì để lưu/khôi phục `sstatus.SIE`

| Đã có | Ở đâu | Dùng được cho khoá? |
|---|---|---|
| `read_sstatus()` (`csrr sstatus`) | `vifast-plat-riscv64/src/timer.rs:82` | đọc được, nhưng **không** phải "lưu + tắt" nguyên tử |
| `SSTATUS_SIE = 1 << 1` | `timer.rs:50`, `vifast_plat_common::trap::SSTATUS_SIE` | hằng dùng chung |
| `ipi::enable_ipi()` (`csrs sie,SSIE` + `csrs sstatus,SIE`) | `ipi.rs:153` | bật, không lưu |
| `ipi::set_global_irq(on)` (`csrs/csrc sstatus,SIE`) | `ipi.rs:172` | **ghi đè**, không lưu giá trị cũ |
| `ipi::disable_ipi()` (`csrc sie,SSIE`) | `ipi.rs:166` | chỉ nguồn, không cổng |

⇒ **Chưa** có hàm "lưu **và** tắt trong một lệnh". `csrrc rd, sstatus, rs1` làm đúng thế
(đọc giá trị cũ vào `rd`, xoá bit) — chọn nó thay vì `csrr` + `csrc` để không có khe giữa hai
lệnh (xem §2.1).

### 1.3 `Mailbox` hiện ai dùng trên `riscv64`

- `vifast_sync::Mailbox` trong `v2/kernel` riscv64: **0** tham chiếu (grep
  `vifast_sync|Mailbox` trên `vifast-plat-riscv64`/`vifast-kbin-riscv64` trước task = 0).
- Kênh wake/IPI **đang chạy** là `vifast_plat_common::ipi::IPI_MAILBOXES` — mỗi hart một
  `IpiMailbox` dạng **bitmask kind** (`AtomicU64`, 3 bit `TlbFlush/VmKick/Wake`), coalesce theo
  kind. Người dùng: `post`/`unpost` (`ipi.rs:226,237`), `drain` (`ipi.rs:263`), `pending`
  (`ipi.rs:288`), `tlb.rs` (drain khi chờ ack), `park_forever` (drain khi hart phụ park).
- Vì sao **không** thay bằng `Mailbox` trong task này: xem §6 (quyết định hoãn, nói thẳng).

### 1.4 `k3b-core.md` thiếu mục `ObjStore` ở đâu

`v2/docs/k3b-core.md` (2.108 dòng tại base) có 27 mục; **không** có mục nào nói về trait
`ObjStore`, `ObjRef`, `place/destroy`, hay đường `slotmem → vifast-objmem`:

- `slotmem` chỉ được mô tả **gián tiếp** ở §13.2 (`Slot CNode → arena`), không nói ranh giới.
- `ObjStore` (định nghĩa thật: `v2/lib/vifast-objmem/src/store.rs:39`) **không** được nhắc.
- `KT-336b` tách `kobj.rs` xong **không** thêm mục này (D2-232(2) yêu cầu) ⇒ sót.
- `KT-336c` thêm **§28** (mục cuối) — 114 dòng, xem §7.

---

## 2. Thiết kế

### 2.1 Vì sao `csrrc` (một lệnh), không phải `csrr` rồi `csrc`

`csrrc rd, sstatus, rs1` **đọc giá trị cũ vào `rd` và xoá bit `SIE` trong cùng một lệnh**:
trạng thái đã lưu và trạng thái tắt ngắt không thể lệch nhau. Hai lệnh rời (`csrr` rồi `csrc`)
để lại một khe — trên cùng hart thì hẹp, nhưng không có lý do gì để mở nó khi một lệnh là đủ.
(Bên `x86_64` của KT-336a dùng `pushfq`/`cli` vì `IF` không có lệnh "đọc-và-xoá" tương đương;
khác biệt này được ghi trong doc của `irq_lock.rs`.)

### 2.2 API mới — `v2/kernel/vifast-plat-riscv64/src/irq_lock.rs` (**mới**, 410 dòng)

```text
irq_enabled()                 csrr sstatus & SIE != 0   (một nguồn đọc cho cả khoá lẫn ca kiểm)
IrqState::save_and_disable()  csrrc → (was_enabled, SIE = 0)
IrqState::restore()           chỉ csrs SIE nếu was_enabled (khôi phục ĐÚNG giá trị đã lưu)
IrqGuard                      tắt ngắt quanh đoạn không lấy khoá; Drop khôi phục
IrqMcsLock<T>                 McsLock<T> + acquired/contended/in_cs/overlap
  .lock_irqsave()             tắt ngắt → MCS FIFO (không trần) → vùng găng
  .lock_irqsave_bounded(n)    tắt ngắt → quay try_lock tối đa n → None (khôi phục SIE, không treo)
  .try_lock_irqsave()         tắt ngắt → try_lock một lần → None
IrqMcsGuard<'a,T>             Drop: giảm in_cs → nhả MCS → khôi phục SIE
```

Bất biến (giữ nguyên từ khuôn x86_64 — **không** phát minh lại):

1. **Thứ tự cặp**: tắt ngắt **trước** khi chờ khoá; nhả khoá **trước** khi bật lại ngắt.
2. **Khôi phục đúng giá trị đã lưu** — gọi từ handler (`SIE = 0`) thì `Drop` **không** bật `SIE`.
3. **Giảm `in_cs` trước khi nhả khoá** — tránh trừ nhầm lượt tăng của người acquire kế tiếp.
4. `MAX_WAITERS = 16` của `vifast-sync` đủ cho 4 hart + handler lồng; không cấp phát, không
   `unsafe` ngoài CSR.
5. `cfg(target_arch)` **chỉ** trong `vifast-plat-riscv64` (R2 cấm `vifast-core` + `v2/lib`);
   `vifast-sync` **không** thêm cfg nào.

### 2.3 Nối vào `core_bridge` — không đổi ngữ nghĩa

| Chỗ | Trước | Sau |
|---|---|---|
| `static LOCKED` (dòng 72) | `AtomicBool` CAS | `IrqMcsLock<()>` |
| `fn lock()`/`fn unlock()` (128–139) | CAS + store | **xoá** (thay bằng guard RAII) |
| `with_machine` | `lock(); … unlock()` | `let _guard = LOCKED.lock_irqsave(); … f(m)` — guard rơi cuối hàm, **cả đường panic** |
| `init` | `lock(); … unlock()` | `let _guard = LOCKED.lock_irqsave(); … report` |

Ngữ nghĩa **giữ nguyên**: một `&mut Machine` tại một thời điểm, không re-entrant, `debug_assert`
`MACHINE_INIT` giữ nguyên vị trí, `CoreReport` không đổi trường nào. Thứ tự khoá: đây là **khoá
lá** (closure không lấy khoá arch khác; không đường nào lấy `core_bridge` trong handler ngắt)
⇒ không nghịch đảo. `size_of::<Machine>()` **không** đổi (`LK` nằm ngoài `Machine`); `.bss` tăng
phần `McsLock` (pool node) — đo ở §7.

### 2.4 TAP 70 — chứng minh khoá **bọc ngắt** trên riscv64

Hai phép đo độc lập, in hai dòng `# lock:` (khuôn dòng của KT-336a, thêm trường `overlap`):

1. **Loại trừ hai ngữ cảnh thật.** BSP lấy `TEST_LOCK` (guard đã tắt `SIE`), rồi **bật lại `SIE`
   một cách cố ý** trong vùng găng và tự gửi IPI (SBI `send_ipi` cho chính hart) ⇒ ngữ cảnh thứ
   hai là **trap handler thật** (`stvec` → `trap_dispatch` → `ipi::on_ipi_interrupt` →
   hook test-only → `try_lock_irqsave` trên **cùng** khoá). Đúng cặp "luồng thường vs handler
   ngắt" của D2-238. Khoá loại trừ đúng ⇒ `denied=1`, `overlap=0`; sai ⇒ `overlap=1`.
   BSP chờ **biến đếm** `PROBE_DONE` (trần 50M vòng), không chờ thời gian.
2. **Ngắt bị hoãn.** Bật nguồn ngắt mềm (`csrs sie,SSIE`) + `sstatus.SIE`, lấy khoá (guard tắt
   `SIE`), tự gửi IPI, quay `IRQ_WINDOW_SPINS` vòng. Trong vùng găng `SIE = 0` ⇒ `during == before`
   và `if_inside = 0`; nhả khoá (guard khôi phục `SIE = 1`) ⇒ IPI **được giao** ⇒ `after > before`.
   Bộ đếm là `IPI_STATS.get(hart).traps` — bộ đếm **của arch**, không phải biến của phép kiểm.

**Vì sao không phải "đo thời gian"** (`k6-test-invariants.md` §1): "CPU không giao ngắt khi
`SIE = 0`" là **bảo đảm kiến trúc** (privileged spec), không phải "thường đúng"; cửa sổ vòng chỉ
để CPU có cơ hội giao nếu `SIE` sai. "Đã giao sau khi nhả" là **biến đếm** chờ tới trần — chứng
minh IPI đã được xếp thật; hết trần ⇒ TAP ĐỎ, **không treo**.

**Vì sao ngữ cảnh thứ hai là trap handler cùng hart, không phải hart phụ:** hart phụ trên
riscv64 kết thúc pha IPI (TAP 20..24) bằng `park_forever` — vòng lặp `drain_current()` **không**
chạy mã của ca kiểm, và không có cửa nào để kbin cài mã vào nó (kbin không sở hữu `trap_dispatch`).
Cùng hart + trap thật lại đúng lớp hazard D2-238 hơn (self-deadlock chỉ xảy ra khi handler chạy
trên **chính** hart đang giữ khoá). Vì vậy ca này **không** phụ thuộc `-smp`: chạy như nhau ở
`smp 1/2/4` (§4).

Hook test-only: `ipi::set_ipi_test_hook(fn())` + `run_ipi_test_hook()` trong
`on_ipi_interrupt`, **`#[cfg(feature = "ktest")]`** ⇒ ảnh sản phẩm không có nhánh/chuỗi nào
(D2-08 kiểm bằng `strings | grep -c ktest`).

---

## 3. Đỏ-trước (D2-05)

Viết `ktest_lock.rs` + manifest + `expected.txt` **trước**, chưa có `irq_lock.rs`. Lượt build đầu:

```text
error[E0432]: unresolved import `vifast_plat_riscv64::irq_lock`
error: could not compile `vifast-kbin-riscv64` (bin "vifast-kbin-ktest") due to 3 previous errors
```

(`E0599`/`E0599` còn lại là thiếu `use vifast_plat_common::Platform` cho `hart_id` — lỗi của chính
tệp test, sửa ngay sau đó.) Sau khi hiện thực: build sạch, boot xanh (§4).

---

## 4. Log QEMU **nguyên văn** (ảnh `ktest`, `2d4339d2`)

Dòng kế hoạch + TAP cuối (log đầy đủ: `v2/agent/JOURNAL/kt336c-logs/qemu-ktest-smp1.log`):

```text
TAP version 13
1..71
...
# lock: mcs acquired=2 contended=1 denied=1 overlap=0 in_cs=0 ok=1
# lock: irq deferred=1 delivered=1 before=101 during=101 after=102 if_before=1 if_inside=0 overlap=0 ok=1
ok 70 - lock: mcs irq guard
ok 71 - trap nested panic
# nested trap: inject sp trong trap stack + jr trap_entry; kỳ vọng PANIC + halt (P2b)
PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)
```

`bash v2/test/boot/ktest.sh --check-log ktest <log> 1`:

```text
ok - check-log ktest smp=1 (plan=71 ok=71, 109 evidence) log /tmp/kt336c-ktest-smp1.log
# check-log: PASS
```

`smp 2` / `smp 4` (matrix — log `qemu-ktest-smp2.log`, `qemu-ktest-smp4.log`):

```text
# lock: mcs acquired=2 contended=1 denied=1 overlap=0 in_cs=0 ok=1
# lock: irq deferred=1 delivered=1 before=1 during=1 after=2 if_before=1 if_inside=0 overlap=0 ok=1
ok 70 - lock: mcs irq guard
ok 71 - trap nested panic
```

```text
ok - check-log ktest smp=2 (plan=71 ok=71, 109 evidence)
ok - check-log ktest smp=4 (plan=71 ok=71, 109 evidence)
```

(`before/during/after` khác nhau giữa các cấu hình vì số trap IPI **trước** pha này phụ thuộc
`-smp`; bất biến là `during == before` và `after > before` — không phải giá trị tuyệt đối.)

---

## 5. Đột biến — cả **ba** ĐỎ (D2-05: đỏ đúng lý do, không treo)

| # | Feature | Dòng log QEMU (nguyên văn) | Kết quả |
|---|---|---|---|
| (i) | `mut-no-irq` (bỏ `csrc sstatus,SIE`) | `# lock: irq deferred=0 delivered=1 before=101 during=102 after=102 if_before=1 if_inside=1 overlap=0 ok=0` | `not ok 70 - lock: mcs irq guard: …` |
| (ii) | `mut-no-lock` (bỏ phần MCS trong phạm vi ca kiểm) | `# lock: mcs acquired=3 contended=0 denied=0 overlap=1 in_cs=0 ok=0` | `not ok 70`; `overlap=1` = **hai ngữ cảnh cùng vào vùng găng** |
| (iii) | `mut-leak-unlock` (guard bỏ nhánh nhả khoá) | `# lock: BAD acquire (đột biến quên unlock — trần chờ đã bắt, không treo)` · `# lock: mcs acquired=3 contended=2 denied=1 overlap=0 in_cs=1 ok=0` | `not ok 70`; **trần chờ hữu hạn** bắt deadlock, không treo |

Đột biến (i) được runner xác nhận ĐỎ **đúng chỗ**:

```text
not ok - check-log ktest smp=1: thiếu ok-count=70/71 not-ok=1 tap-day-1..71 tap70 ev98, log …mut-no-irq…
#   thiếu TAP #70 [lock: mcs irq guard] (thấy 0 dòng, cần đúng 1)
#   thiếu evidence#98: ^# lock: irq deferred=1 delivered=1 before=[0-9]+ during=[0-9]+ after=[0-9]+ if_before=1 if_inside=0 overlap=0 ok=1$
```

Log: `v2/agent/JOURNAL/kt336c-logs/mut-{no-irq,no-lock,leak-unlock}.log` — **chạy lại trên đúng
cây sạch `2d4339d2`** (sau commit), không phải cây nháp; hành vi lặp y hệt lần đầu.

---

## 6. `vifast_sync::Mailbox` cho kênh wake/IPI riscv64 — **HOÃN** (nói thẳng)

**Trạng thái: KHÔNG nối trong `KT-336c`.** Kênh wake/IPI riscv64 vẫn là
`vifast_plat_common::ipi::IPI_MAILBOXES` (bitmask coalesce theo kind). Lý do kỹ thuật, không phải
"hết giờ":

1. **Hai ngữ nghĩa khác nhau, không thay thế được.** `IpiMailbox` là **bitmask kind** coalesce
   (`AtomicU64`: post trùng kind ⇒ gộp, drain một `swap` giao đủ kind, **không bao giờ đầy**).
   `vifast_sync::Mailbox<N>` là **MPSC FIFO có payload**, đầy ⇒ `Busy` (backpressure). Thay thẳng
   hộp thư ⇒ mất coalesce: bão IPI `Wake` (đúng ca D2-68/D2-77 đã đo) làm đầy hộp thư ⇒
   `post` trả `Busy` ⇒ **mất đánh thức**, hoặc phải giữ song song bitmask làm fallback ⇒ **hai
   nguồn sự thật cho cùng một sự kiện** — đúng "nối nửa vời" mà đề bài cấm.
2. **Chưa có consumer cần payload.** `Msg::Wake { hart, tcb }` mang `tcb`; trên riscv64 hiện
   **không** ai dùng payload đó (scheduler K3b chưa chạy trên ảnh sản phẩm; `IpiKind::Wake` chỉ
   là bit đánh thức). Nối `Mailbox` bây giờ = thêm kênh **không có người nhận**, không chứng minh
   được gì ngoài "đã import".
3. **Rủi ro cao hơn giá trị trong 2 h.** Đổi hộp thư chạm `post`/`drain`/`park_forever`/TLB
   shootdown (`tlb.rs` drain khi chờ ack) — 4 đường đã có TAP 20..29 và soak; một sai sót ở đây
   là **mất ack** (deadlock thật, không phải đỏ có chẩn đoán).

**Kế hoạch tối thiểu khi mở lại** (để không phải phát minh lại; đúng hướng D2-240 trụ cột 1
`SMP-1`/`SMP-2`):

1. Giữ **nguyên** giao thức IPI: SBI `send_ipi` → `sip.SSIP` → `stvec` → `drain` (không đổi).
2. Thêm **một** `vifast_sync::Mailbox<8>` mỗi hart cho **payload** wake (`Msg::Wake{hart,tcb}`);
   bit `IpiKind::Wake` vẫn là **doorbell** (coalesce), mailbox chỉ mang nội dung ⇒ **không** đổi
   ngữ nghĩa đầy/coalesce hiện có.
3. `drain_hart_current` đọc mailbox khi kind `Wake` có mặt; ca kiểm: hart phụ gửi 2 wake liên
   tiếp (một coalesce + một payload khác) ⇒ nhận **đủ** payload theo FIFO; hộp thư đầy ⇒ `Busy`
   được **xử lý** (không nuốt) — và một đột biến "bỏ mailbox drain" phải ĐỎ.
4. Chỉ sau đó mới bàn tới chuyện bỏ bitmask (`IPI_MAILBOXES`) — cần một quyết định D2 mới vì nó
   đổi ngữ nghĩa đã được D2-68/D2-77 chốt.

Việc này **không** thuộc `KT-336c`; đề nghị lead ghi thành task riêng (cùng nhóm `SMP-1`,
D2-240) thay vì coi `KT-336c` đã đóng Audit Q1-A/N7 — **`KT-336c` chỉ đóng phần `IrqMcsLock`
của riscv64**, phần `Mailbox` 3/3 port vẫn mở.

---

## 7. Dòng tệp, ngân sách ảnh (D2-72 Q5 — **không** nới trần)

| Tệp | Trước | Sau |
|---|---:|---:|
| `v2/kernel/vifast-plat-riscv64/src/core_bridge.rs` | 303 | **300** |
| `v2/kernel/vifast-plat-riscv64/src/irq_lock.rs` | — | **410** (mới) |
| `v2/kernel/vifast-kbin-riscv64/src/ktest_lock.rs` | — | **242** (mới) |
| `v2/kernel/vifast-plat-riscv64/src/ipi.rs` | 306 | 340 (+34: hook test-only) |
| `v2/docs/k3b-core.md` | 2.108 | 2.222 (+114: §28) |

R1 (≤ 1.500 dòng/tệp): mọi tệp **dưới** trần; `core_bridge.rs` còn **300** dòng — **không** cần
tách tệp (`KT-320/322` chỉ áp khi sắp vượt). R1 PASS trong `v2 check`.

Ngân sách ảnh (`bash v2/test/boot/budget.sh <img> <nhãn>`, trần 262.144 B):

```text
# image=ktest path=…/vifast-kbin-ktest file=728432 data+bss=239408 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# image=product path=…/vifast-kbin-riscv64 file=193568 data+bss=196416 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

⇒ headroom ảnh test **22.736 B**, ảnh sản phẩm **65.728 B**; **không** đổi trần, không dời mảng.

---

## 8. Cổng (chạy trên cây **sạch** `2d4339d2`, rev-gate đòi `dirty=0`)

| Lệnh | Kết quả |
|---|---|
| `./v2/bin/v2 check` | **EXIT 0** — **564** dòng `ok -`, **0** dòng `not ok` (fmt + clippy + R1..R5 + self-test luật) |
| `bash v2/test/boot/ktest.sh 1 1` | **PASS** — **9/9** boot sạch, `ktest 71/71` |
| `bash v2/test/boot/ktest.sh 1 matrix` | **PASS** — **27/27** boot sạch, `ktest 71/71` ở cả `smp 1/2/4` |
| `bash v2/test/boot/selftest.sh <log> 1` | **PASS** — **31/31** ca (thêm 2 ca âm mới cho hai dòng `# lock:`) |
| Đột biến (i)/(ii)/(iii) | **ĐỎ cả ba** — §5; chạy lại trên **đúng** cây `2d4339d2` |

Cổng danh tính bản dựng (KT-86) — nguyên văn:

```text
REV-STATE: root=/private/tmp/kt336c-s1 head_rev=2d4339d2e729 inputs=fbb2bed135fc4236 count=348 dirty=0 scope=[v2/kernel v2/lib v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml v2/tools/repro-build.sh v2/tools/rustc-canon.rs]
REV-GATE: ACCEPT inputs=fbb2bed135fc4236 current_inputs=fbb2bed135fc4236 head_rev=2d4339d2e729 count=348 log=/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T//k3-ktest/kt86-rev-gate.log
```

`ktest.sh 1 1`:

```text
# image=ktest        smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)  TAP 71/71 PASS
...
# KTEST: 9/9 boot sạch (smp=1, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

`ktest.sh 1 matrix`:

```text
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

`selftest.sh` (cổng harness — ca âm **mới** của KT-336c là #29/#30):

```text
ok 29 - log ktest: xoá dòng '# lock: mcs acquired=' (bằng chứng loại trừ của KT-336c)
#   runner đỏ vì [ev97]: not ok - check-log ktest smp=1: thiếu ev97, log …/log-no-mcs-lock.log
ok 30 - log ktest: 'deferred=1' → 0 (ngắt không bị hoãn — đúng ca đột biến mut-no-irq)
#   runner đỏ vì [ev98]: not ok - check-log ktest smp=1: thiếu ev98, log …/log-irq-not-deferred.log
# selftest: 31/31 ca đúng như kỳ vọng
# selftest.sh: PASS
```

Log cổng lưu trong repo: `v2/agent/JOURNAL/kt336c-logs/gate-{check,ktest-1-1,ktest-matrix,selftest}.log`.

**Không nới ngưỡng nào**: `expected.txt` `ktest 70 70 → 71 71` chỉ là **đếm** TAP (thêm một TAP
thật, trap lồng dời 70 → 71); trần ảnh **262.144 B** giữ nguyên; timeout **20 s/boot**;
`MH_LOCK_SPIN_LIMIT`/`SPIN` không đụng; không `#[ignore]`, không retry (D2-07).

---


## 9. Việc còn lại (ngoài `KT-336c`)

1. **`Mailbox` wake/IPI cho riscv64** — §6, cần task riêng (`SMP-1`, D2-240) vì đổi ngữ nghĩa
   coalesce của D2-68/D2-77. `KT-336c` **không** đóng Audit Q1-A/N7 trọn vẹn: chỉ đóng phần
   `IrqMcsLock`.
2. **`KT-338a`/`KT-340b`** vẫn phải làm `IrqMcsLock` + `Mailbox` cho `aarch64`/`x86_64`
   (`x86_64` đã có `IrqMcsLock` từ `KT-336a`, còn `Mailbox` mở).
3. `MEM-1`/`MEM-2` (D2-240) mới thực thi §28.5; `KT-336c` chỉ **viết** ranh giới.
4. **Nợ tài liệu đã biết**: `v2/docs/k6-virtio-blk.md:241` còn nhắc "70 TAP kernel" (số cũ
   của ảnh chính, nay **71**). Không sửa trong `KT-336c`: tệp `k6-*.md` chịu luật E1
   (`verify-numbers` — mọi số phải nằm trong khối `scope=`), sửa số mà không kèm log/`scope=`
   sẽ làm đỏ cổng; để chủ sở hữu tài liệu k6 cập nhật cùng khối bằng chứng.
