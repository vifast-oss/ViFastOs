# KT-336a — Nối `vifast-sync` vào kernel x86_64: `McsLock` **bọc lưu/tắt ngắt** (D2-232, D2-238)

- **Task**: `KT-336a` (Chu kỳ 2, Đợt 1) — bước 1 của việc đóng Audit **Q1-A**/`N7`.
- **Nhánh**: `task/kt336a-sync`, base `main` @ `53716be8`.
- **Ngày**: 2026-11-22 (JST).
- **Kết quả**: TAP x86_64 **34/34** (thêm TAP 34), runner `32/32` (thêm ca âm `dd`/`ee`), soak smoke
  **60/60** boot 0 lỗi; `./v2/bin/v2 check` PASS. `vifast-sync` từ **0 consumer** trong
  `v2/kernel` ⇒ **1 consumer** (`vifast-plat-x86_64`).

---

## 1. Khảo sát (trước khi sửa)

### 1.1 `vifast-sync` có gì

| Tệp | Dòng | Nội dung |
|---|---:|---|
| `v2/lib/vifast-sync/src/mcs.rs` | 494 | `McsLock<T>` — MCS queue lock, pool node cố định `MAX_WAITERS = 16` (node **không** nằm trên stack người gọi), `lock`/`try_lock`/`Guard`, FIFO theo `tail.swap`, không reentrant. `Ordering::SeqCst` cho `tail/head/next/locked`. |
| `v2/lib/vifast-sync/src/mailbox.rs` | 309 | `Mailbox<N>` — MPSC cố định kiểu Vyukov, 7 loại `Msg` (`Wake/TlbFlush/Reschedule/Migrate/Stop/VmKick/PowerReq`), đầy ⇒ `Busy` (backpressure), ack theo thế hệ. |
| `v2/lib/vifast-sync/tests/kt30_sync.rs` | 234 | 10 test single-thread + `Mailbox`. |
| `v2/lib/vifast-sync/tests/kt30_stress.rs` | 267 | 3 test đa luồng (loại trừ 4×500, FIFO có điều khiển, `try_lock`). |

**Trước KT-336a**: grep `McsLock|vifast_sync` trong `v2/kernel` = **0** consumer (chỉ có trong
`v2/sim` và chính crate). Đúng căn cứ D2-232.

### 1.2 Spinlock tự chế trên x86_64 ở đâu

| Vị trí | Dòng (trước sửa) | Cơ chế |
|---|---:|---|
| `v2/kernel/vifast-plat-x86_64/src/com.rs` | 86–183 | `ComLock`: `AtomicBool` CAS quay vòng + `cli`/`sti` thủ công (`enter_critical`/`leave_critical`), bộ đếm `acquired/contended/overlap/in_cs`; `COM1` là khoá một dòng log (KT-316). |
| `v2/kernel/vifast-kbin-x86_64/src/sched.rs` | 221 | `MH_LOCK: AtomicBool`: CAS quay vòng cho pha nhiều hart (KT-310), trần `MH_LOCK_SPIN_LIMIT = 100_000_000` vòng ⇒ `bad_lock` (đỏ có trần, không treo). |

Ngoài phạm vi x86_64 (không chạm trong task này): `core_bridge::LOCKED` ở
`v2/kernel/vifast-plat-riscv64/src/core_bridge.rs:72` — thuộc `KT-338a`/`KT-340b` theo D2-238(2).

### 1.3 Cơ chế tắt/bật ngắt hiện có

- `v2/kernel/vifast-plat-x86_64/src/asm.rs`: `sti()` (:286), `cli()` (:307),
  `interrupts_enabled()` (:321, `pushfq`/`pop` đọc `RFLAGS.IF`).
- `com.rs` đã có khuôn "lưu `IF` trước `cli`, khôi phục đúng giá trị sau khi nhả" — nhưng nằm
  **trong** `ComLock`, không tái dùng được cho khoá khác.

### 1.4 Audit Q1-A (`N7`) nói gì — **nguyên văn**

`docs/v1/KNOWN-ISSUES.md` dòng 7 bảng:

> **Audit Q1-A** — kênh wake lệch 3 port `[V1:DOC]` | Trong `Mailbox`, kênh đánh thức chưa thống
> nhất 3 port: riscv64 có doorbell riêng cho reschedule, arm/x86 dùng mailbox chung. Nợ từ r293 |
> §15.8: thống nhất sau r293–r298; đóng khi một cơ chế chung cho cả 3 port | **`vifast-sync` dùng
> chung cho mọi arch** (QD §6); v2 chỉ riscv64 tới K6 nên ca kiểm chạy ở sim + riscv64 trước |
> **Mang sang v2** (thiết kế lại hộp thư theo hart)

`docs/QUYET-DINH-sau-review.md:223` chốt hướng: `Audit Q1-A kênh wake lệch 3 port → vifast-sync
dùng chung cho mọi arch`.

---

## 2. Bẫy D2-238 và thiết kế

### 2.1 Bẫy

`McsLock::lock()` **không** tắt ngắt. Dùng nguyên trạng cho tài nguyên dùng chung giữa luồng thường
và handler ngắt ⇒ hart đang giữ khoá bị ngắt **trên chính nó**, handler lấy cùng khoá ⇒ **tự
deadlock**. Đây đúng lớp lỗi KT-316 đã gặp với COM1, và là lý do D2-238(2) bắt buộc bọc guard
lưu/tắt IRQ **phía platform**.

### 2.2 Thiết kế — `v2/kernel/vifast-plat-x86_64/src/irq_lock.rs` (mới)

```text
IrqState::save_and_disable()   lưu RFLAGS.IF → cli
IrqState::restore()            chỉ sti nếu trước đó IF = 1
IrqGuard                       tắt ngắt quanh đoạn không lấy khoá; Drop khôi phục IF
IrqMcsLock<T>                  McsLock<T> + acquired/contended/in_cs/overlap
  .lock_irqsave()              cli → MCS lock (FIFO, không trần) → vùng găng
  .lock_irqsave_bounded(n)     cli → quay try_lock tối đa n vòng → None (hết trần, khôi phục IF)
  .try_lock_irqsave()          cli → try_lock một lần → None
IrqMcsGuard<'a,T>              Drop: giảm in_cs → nhả MCS → khôi phục IF
```

Bất biến:

1. **Thứ tự cặp**: tắt ngắt **trước** khi chờ khoá; nhả khoá **trước** khi bật lại ngắt.
2. **Khôi phục đúng giá trị đã lưu** — gọi từ handler (`IF = 0`) thì `Drop` **không** `sti`.
3. **Giảm `in_cs` trước khi nhả khoá** — tránh trừ nhầm lượt tăng của người acquire kế tiếp.
4. `MAX_WAITERS = 16` đủ cho 4 hart + handler lồng (không cấp phát, không `unsafe` mới).
5. `cfg(target_arch)` **chỉ** nằm trong `vifast-plat-x86_64` (R2 chỉ cấm `vifast-core` + `v2/lib`;
   `vifast-sync` **không** thêm cfg nào).

### 2.3 Nối vào kernel

| Chỗ | Trước | Sau |
|---|---|---|
| `COM1` (`com.rs`) | `ComLock` CAS + `cli` tự chế | newtype mỏng trên `IrqMcsLock<()>`; API `lock/acquired/contended/overlap/ok` **giữ nguyên** ⇒ TAP 28 + 3 test host COM1 không đổi. |
| `MH_LOCK` (scheduler x86_64) | `AtomicBool` CAS + `MH_LOCK_SPIN_LIMIT` | `IrqMcsLock<()>`, `mh_lock(hart) -> Option<IrqMcsGuard>` + `lock_irqsave_bounded(MH_LOCK_SPIN_LIMIT)`; guard RAII sống tới hết `mh_tick`, `drop` tường minh **trước** `frame_ok` để giữ đúng ranh giới vùng tới hạn KT-310. `no-sched-lock` vẫn là no-op (đột biến cũ). |
| Thứ tự khoá | (không ghi) | `MH_LOCK → COM1` (in dòng `# sched: hart … picked` khi đang giữ `MH_LOCK`); không có đường `COM1 → MH_LOCK` ⇒ không nghịch đảo. |

**Ngữ nghĩa scheduler không đổi**: cùng trần `MH_LOCK_SPIN_LIMIT`, cùng `bad_lock`/`bad_double`,
cùng ranh giới quyết định/chuyển ngữ cảnh. Chỉ cơ chế khoá đổi.

### 2.4 `Mailbox` — **cố ý để lại**

`Mailbox` **chưa** cần cho bước 1 x86_64 (kênh IPI hiện tại là `ICR` + `SMP_CMD`, không phải wake
MPSC; `vifast_sync::Mailbox` cần cho kênh wake/IPI của `riscv64` theo D2-238(2) — hoàn tất ở
`KT-338a`/`KT-340b`). Không nối nửa vời: **0** tham chiếu `Mailbox` mới trong kernel sau task này.

---

## 3. Test đỏ trước (D2-05) và test mới

### 3.1 Đỏ-trước

- Viết `v2/kernel/vifast-plat-x86_64/tests/irq_lock.rs` **trước** khi có `irq_lock.rs`.
  Lượt chạy đầu (log giữ ở `/tmp`):
  `error[E0432]: unresolved import vifast_plat_x86_64::irq_lock` ⇒ đỏ đúng lý do.
- Sau hiện thực: **6/6** test host mới xanh; 3 test `com.rs` cũ vẫn xanh.

### 3.2 TAP 34 — hai phép đo độc lập (`src/lock_report.rs`)

1. **Loại trừ hai ngữ cảnh thật**: BSP giữ `TEST_LOCK`, gửi IPI tới **hart phụ 1**; handler IPI
   của hart 1 gọi `try_lock_irqsave` trên cùng khoá ⇒ **bị từ chối**. BSP chờ **biến đếm**
   `PROBE_DONE` (trần 50M vòng), không chờ thời gian.
2. **Ngắt bị hoãn thật**: `sti` → lấy khoá (guard `cli`) → gửi **IPI cho chính BSP** → quay
   2M vòng: `during == before` (CPU che ngắt khi `IF = 0`). Nhả khoá (guard khôi phục `IF = 1`) ⇒
   IPI đang chờ **được giao** (`after > before`, chờ bằng biến đếm).

Log QEMU **nguyên văn** (boot sạch, `run-x86_64.sh`):

```text
# lock: mcs acquired=2 contended=1 denied=1 overlap=0 in_cs=0 ok=1
# lock: irq deferred=1 delivered=1 before=2 during=2 after=3 if_before=1 if_inside=0 ok=1
ok 34 - lock: mcs irq guard
```

(`# smp: ipi recv=2` = IPI phép thử tới hart 1; `# smp: ipi recv=3` = IPI tự gửi sau khi nhả khoá —
hai dòng này là hệ quả của phép đo, không phải kiểm mới.)

### 3.3 Runner / soak

- `EXPECT_TESTS` 33 → **34**; `check_log` kiểm **cả hai** dòng `# lock:` + `ok 34`.
- Ca âm mới: `(dd)` xoá dòng `# lock: irq` ⇒ ĐỎ; `(ee)` `deferred=1 → 0` ⇒ ĐỎ. Cả hai **tự khẳng
  định đã phá** (`grep -c` trước/sau), theo luật 2 `k6-test-invariants.md`.
- `soak-x86_64.sh`: `TAP_TESTS=34`, mục `(10f)` kiểm hai dòng + TAP 34, log tổng hợp
  `--selftest` thêm 2 ca âm ⇒ **49/49 PASS** (2 dương + 47 âm).

---

## 4. Ba đột biến — **cả ba ĐỎ** (TAP 34)

| # | Feature | Log QEMU (trích) | Kết quả |
|---|---|---|---|
| (i) | `mut-no-irq` (bỏ `cli` trong `IrqState`) | `# lock: irq deferred=0 delivered=1 … if_before=1 if_inside=1 ok=0` | `not ok 34` → halt 3, **không marker** |
| (ii) | `mut-leak-unlock` (bỏ nhánh nhả khoá ở `IrqMcsGuard::drop`, có arm) | `# lock: BAD acquire (đột biến quên unlock — trần chờ đã bắt, không treo)` · `# lock: mcs acquired=3 contended=2 denied=1 overlap=0 in_cs=1 ok=0` | `not ok 34`; **trần chờ hữu hạn** bắt deadlock, không treo im lặng |
| (iii) | `mut-no-lock` (bỏ phần MCS, chỉ trong phạm vi ca kiểm) | `# lock: mcs acquired=3 contended=0 denied=0 overlap=1 in_cs=0 ok=0` | `not ok 34`; `overlap=1` = **hai ngữ cảnh cùng vào vùng găng** |

Đột biến (iii) lần đầu arm **toàn cục** làm scheduler chết sớm ở TAP 25 (`double-pick`) — đúng
hành vi "mất khoá", nhưng không đo đúng TAP 34. Bản cuối **arm theo phạm vi ca kiểm** để TAP 34 là
ca đỏ, `overlap` đo đúng "hai ngữ cảnh cùng vào".

---

## 5. Test host

- `vifast-plat-x86_64 --test irq_lock`: **6/6** — loại trừ 4 luồng×100, `try_lock` khi đang giữ,
  **trần chờ** (`bounded` trả `None`, không treo), không reentrant, `contended ≥ 1`, cặp
  `IrqState`/guard.
- `vifast-plat-x86_64 --test com`: **3/3** (hồi quy KT-316 — API `ComLock` không đổi).
- `vifast-sync`: **19/19** (4 unit + 3 stress + 10 `kt30_sync` + 2 khác) — đã có sẵn loại trừ,
  FIFO, `try_lock`, không reentrant; **không** phải viết lại.

---

## 6. Cổng (chạy trên cây cuối)

| Lệnh | Kết quả |
|---|---|
| `./v2/bin/v2 check` | **PASS** (fmt + clippy + R1..R5 + self-test) |
| `bash v2/test/boot/run-x86_64.sh` | **PASS** `32/32 kiểm sạch (boot=1, tests=34, exc=5)`, `31/31 ca âm ĐỎ` |
| `bash v2/test/boot/soak-x86_64.sh --smoke 60 3` | **60/60 lỗi=0**, `qemu_foreign=0`, `guard: rc_124=0`; script exit 1 **có chủ đích** vì smoke `N=60 < 600` không phải bằng chứng cổng K5' (D2-24/D2-149). |
| `cargo test -p vifast-sync` / `--test irq_lock` / `--test com` | **19/19**, **6/6**, **3/3** |

**Không** nới ngưỡng nào: `.data+.bss` không đổi ngưỡng, `TAP_TESTS` chỉ tăng 33 → 34, trần
`MH_LOCK_SPIN_LIMIT` giữ nguyên, thời gian boot giữ nguyên.

---

## 7. Ghi chú / bẫy gặp phải

- **D2-08**: module đặt tên `locktest` làm ảnh chứa **6** chuỗi `"ktest"` (chuỗi con của
  `locktest`) ⇒ soak từ chối ảnh sản phẩm. Đổi tên `lock_report` ⇒ `strings | grep -c ktest = 0`.
  Bài học: tên module trong ảnh phải tránh **bất kỳ** chuỗi con `ktest`.
- `#[cfg(feature = "mut-no-irq")]` dùng `cfg!` trong thân hàm để **một** đường mã cho cả hai
  build; ba feature đột biến **không** bật trong ảnh sản phẩm.
- `vifast-core`: **0** thay đổi, **0** `cfg(target_arch)` mới (R2 PASS). Toàn bộ phần phụ thuộc
  kiến trúc nằm trong `vifast-plat-x86_64` — `vifast-sync` vẫn `no_std`, không cfg arch.

## 8. Việc còn lại (ngoài phạm vi bước 1)

- `Mailbox` cho kênh wake/IPI: `KT-338a` (riscv64) + `KT-340b` (x86_64) theo D2-238(2).
- `core_bridge::LOCKED` (riscv64) chưa đổi — chờ `KT-338a`.
