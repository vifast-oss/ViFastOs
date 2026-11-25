# KT-413 — SMP Preemptive 4 hart `x86_64` + bộ ca SMP chung `vifast-ktest` + PCID/ASID + đóng `N-3`

- **Task**: KT-413 (P0, Chặng 2 / W5, 3 h) — phụ thuộc KT-402, KT-402b, KT-411.
- **Nhánh**: `task/kt413-smp-x64` · **Worktree**: `/private/tmp/vifast-wt/2`
- **Base**: `main` @ `55bcd851` (đã có KT-402b/408/409/409b/410/411).
- **Căn cứ**: `D2-259` mục 2 (B5 + SMP 3 arch), `D2-260`, `D2-17` (unsafe chỉ trong danh sách đóng),
  `KE-HOACH-KIEN-TRUC.md` dòng 189, `QUEUE.md` dòng 465, nợ `N-3` của `KT-348a`
  (`JOURNAL/kt348a-x64hang.md` §7).
- **Ngân sách**: 3 h. Bắt đầu 2026-11-26 00:09 JST (15:09 UTC, Thấp điểm chính).
- **Cảnh báo song song**: KT-412 chạy trên `riscv64` + `aarch64`. Ràng buộc tự áp: **0 chạm**
  `vifast-plat-riscv64/**`, `vifast-plat-riscv64-logic/**`, `vifast-plat-aarch64/**`,
  `vifast-kbin-riscv64/**`, `vifast-kbin-aarch64/**`. Chạm `vifast-core` chỉ khi bắt buộc, hunk tách rời.

## 0. Kế hoạch ngắn (5–10 dòng)

1. **Đo hiện trạng**: `sched.rs` harness = `kbin-x86_64/src/sched.rs` **1.376** +
   `plat-x86_64/src/sched.rs` **786** = **2.162** dòng (khớp con số QUEUE). `vifast-ktest` mới có
   `core_suite` (TAP 43..52) + `invariants`; **chưa** có bộ ca SMP; `kbin-x86_64` **chưa** phụ thuộc
   `vifast-ktest`.
2. **Đỏ-trước**: `vifast-ktest/tests/smp_suite.rs` gọi `vifast_ktest::smp_suite::run` (chưa có) ⇒
   không biên dịch được; test host `plat-x86_64` cho hằng watchdog `VEC_WATCHDOG`/`STUB_WATCHDOG`
   (chưa có) ⇒ không biên dịch được.
3. **Ưu tiên 1** — bộ ca SMP chung `vifast_ktest::smp_suite` (4 hart: `dispatch` mỗi hart, priority,
   sleep/wake, migration, bất biến) + nối `kbin-x86_64`: `core_bridge::init` **sau** `smp::run` với
   `harts=4` **đo được**, cả 4 hart vào core dưới khoá chung, TAP 49/50.
4. **Ưu tiên 2** — gỡ harness `sched.rs`: **không** làm trong khung này (xem §6 — cần đổi 32 ca âm
   `# sched:` của runner, tức đổi kỳ vọng harness ⇒ phải có D2 riêng, đúng ghi chú `KT-340c` §6.2).
5. **Ưu tiên 3** — PCID/ASID: **không** làm (xem §6 — runner đang **đòi** `cr4_pcid=0`; bật PCID là
   đổi kỳ vọng harness).
6. **Ưu tiên 4** — đóng `N-3`: vector IDT riêng `0x32` + stub + handler đếm `WATCHDOG_WAKES` + arm
   LAPIC timer one-shot trong `measure_timer` ⇒ `hlt` **không** ngủ vô hạn khi mất IRQ0; ca âm
   `--features no-pit-irq` (mask IRQ0) ⇒ boot **vẫn thoát**, TAP 10 ĐỎ.
7. `./v2/bin/v2 t0` sau mỗi bước → `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `spec/`; không nới ngưỡng; không `#[ignore]`/`#[should_panic]`;
không `git add/commit`; chỉ T0 + T1; không `cfg(target_arch)` mới; 0 bỏ ca khỏi matrix.

## 1. Đo hiện trạng (trước khi sửa)

```text
$ wc -l v2/kernel/vifast-kbin-x86_64/src/sched.rs v2/kernel/vifast-plat-x86_64/src/sched.rs
    1376 v2/kernel/vifast-kbin-x86_64/src/sched.rs
     786 v2/kernel/vifast-plat-x86_64/src/sched.rs
    2162 total
$ wc -l v2/kernel/vifast-ktest/src/*.rs
     860 core_suite.rs
     114 invariants.rs
     153 lib.rs
$ grep -c 'sched:' v2/test/boot/run-x86_64.sh
69
$ grep -n 'vifast-ktest' v2/kernel/vifast-kbin-x86_64/Cargo.toml
(không có — ảnh x86_64 chưa dùng bộ ca chung)
```

## 2. Đỏ-trước (bằng chứng, D2-05)

### 2.1 Bộ ca SMP chung — API chưa tồn tại

`v2/kernel/vifast-ktest/tests/smp_suite.rs` được viết **trước** khi có `smp_suite`:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest --test smp_suite
error[E0432]: unresolved import `vifast_ktest::smp_suite`
  --> vifast-ktest/tests/smp_suite.rs:25:19
   |
25 | use vifast_ktest::smp_suite::{self, SMP_HARTS};
   |                   ^^^^^^^^^ could not find `smp_suite` in `vifast_ktest`
error: could not compile `vifast-ktest` (test "smp_suite") due to 1 previous error
```

Đỏ **đúng lý do** ("API chưa có"), không phải đỏ vì lỗi khác. Sau khi hiện thực: **4/4** ca host
xanh (`cargo test -p vifast-ktest`), gồm ca "Machine 1 hart ⇒ TAP 48 **ĐỎ**" (bộ ca có răng).

### 2.2 Nợ `N-3` — `hlt` ngủ vĩnh viễn khi mất IRQ0 (đo được, không suy đoán)

Ca âm `--features no-pit-irq` (mask redirection IRQ0 của IOAPIC ngay trước cửa sổ đo timer).
**Bản KHÔNG có watchdog** (tạm thời `let watchdog_armed = false;`, đã revert):

```text
$ cargo build -p vifast-kbin-x86_64 --target x86_64-unknown-none --release --features ktest,no-pit-irq
$ qemu-system-x86_64 -M q35 -cpu qemu64 -m 128M -nographic -no-reboot -smp 4 -kernel <img>
ok 9 - apic: ioapic
# timer: lapic count0=985000 count1=972000 runs=1 ioapic_route=1 div_ok=1 nominal_hz=100 pit=0x2e5c->0x2cb8
# timer: lapic-window lvt_masked=1 masked_before_unmask=1 dest_bsp=1 order=dest_first
<QEMU còn sống sau 12 s — TREO, không dòng nào sau `# timer: lapic-window`>
$ grep -c 'timer: watchdog' log
0
```

Đây **đúng** triệu chứng nợ `N-3`: `hlt` không có nguồn đánh thức ⇒ treo im lặng. **Bản CÓ
watchdog** (cùng feature):

```text
# timer: hz=0 ticks=0 window_pit=658499
# timer: watchdog vector=0x32 armed=1 wakes=24 masked_after=1 pit_dead=0 ok=1
not ok 10 - timer: ticks
# TAP 10 FAIL — dừng (không in marker)
```

⇒ boot **vẫn thoát** (không treo) và TAP 10 **ĐỎ** — đúng điều kiện ĐẠT của `N-3`. Bản xanh
(không feature): `# timer: watchdog vector=0x32 armed=1 wakes=26 masked_after=1 pit_dead=0 ok=1`
+ `ok 10 - timer: ticks`.

## 3. Việc đã làm

### 3.1 Bộ ca SMP dùng chung `vifast_ktest::smp_suite` (ưu tiên 1)

- `v2/kernel/vifast-ktest/src/smp_suite.rs` (**mới**, ~430 dòng): `SMP_HARTS = 4`, `SMP_TCBS = 5`
  (một TCB/hart + một TCB mức cao trên hart 0 để đo priority), `SMP_HARTS`/`PRIO`/`HOME` là
  **dữ liệu** có test host. Bốn bất biến: (1) **cả bốn** hart `dispatch` ≥ 1; (2) **không**
  double-pick (`current(h)` đôi một khác nhau); (3) priority tĩnh (hart 0 chọn TCB mức 5 trước);
  (4) `block_current` + `wake` + `dispatch` lại đúng. Cộng `push_migration` + bốn bất biến
  `check_sched`/`check_sc_refs`/`check_budget`/`check_mailboxes`.
- `v2/kernel/vifast-ktest/src/lib.rs`: thêm `pub mod smp_suite` + trait **hẹp** `SmpBridge`
  (`with_machine` + `link`) với **blanket impl** cho mọi `CoreBridge`. Vì sao tách: `CoreBridge`
  đòi object layer (`obj_alloc_frame`/`obj_vm_selftest`) mà kernel `x86_64` **chưa** có
  (`KT-340c` chỉ nối `Machine`) — cài ba method **giả** sẽ là bằng chứng sai (`D2-83`/`D2-102`).
- `v2/kernel/vifast-ktest/tests/smp_suite.rs` (**mới**, 4 ca host): chạy bộ ca trên nền tảng giả
  4 hart (0 đỏ, đủ 2 TAP 48/49), nhãn TAP ổn định, **cả bốn** hart được dùng, và ca **có răng**
  (`Machine` 1 hart ⇒ `fails > 0`).

### 3.2 Bốn hart `x86_64` vào `vifast-core` (ưu tiên 1)

- `v2/kernel/vifast-kbin-x86_64/src/coresmp.rs` (**mới**): `ImagePlatform` (`Platform` trên COM1 +
  `TICK_COUNT` + `LAPIC_ID`) + `ImageBridge` (`SmpBridge` trên `core_bridge::with_machine`).
  `run()` dựng **lại** model với `harts = smp::online_count()` **đo được** (`D2-88(a)`) — đóng nợ
  "`harts=1` vì init trước SMP" của `KT-340c` — rồi chạy bộ ca chung.
- `v2/kernel/vifast-kbin-x86_64/src/smp.rs`: thêm `online_mask()`/`online_count()` (đọc
  `SMP_ONLINE_MASK`).
- `v2/kernel/vifast-kbin-x86_64/src/net.rs`: gọi `coresmp::run` **ngay sau** `smp::run` (cả bốn
  hart đã online, chưa hart nào vào pha preemption).
- `Cargo.toml`: `vifast-ktest` + `vifast-plat-common` là dep **optional**, chỉ bật bởi feature
  `ktest` ⇒ ảnh sản phẩm **không** link (D2-08). `mod coresmp` cũng `#[cfg(feature = "ktest")]`.

Bằng chứng boot thật (QEMU 11.1.3, `-smp 4`):

```text
# core smp: online mask=0xf harts=4 expect=4
# core smp: init addr=0x237000 size=184712 pin=196608 harts=4 untyped=1 base=0x20a000 bytes=16384 healthy=1
# core smp: harts=4 picks=[1, 1, 1, 1] first=[t4, t1, t2, t3] distinct=4 double=0 prio_ok=1 sleep_ok=1 migrate=0 invariants_ok=1
ok 48 - core smp 4 hart
ok 49 - core smp sleep migrate
```

### 3.3 Đóng nợ `N-3` — watchdog LAPIC vector riêng `0x32` (ưu tiên 4)

- `vifast-plat-x86_64/src/exc.rs`: `VEC_WATCHDOG = 0x32`, `STUB_WATCHDOG = 20`, `STUB_COUNT = 21`,
  `stub_slot(VEC_WATCHDOG)`.
- `vifast-plat-x86_64/src/asm/trap.rs`: stub `vifast_isr_watchdog` (push 0 + vector + `jmp` stub
  chung) + `watchdog_vec` const operand + `stub_addr` arm.
- `vifast-kbin-x86_64/src/trap.rs`: nhánh `VEC_WATCHDOG` **chỉ** tăng `WATCHDOG_WAKES` + `EOI`
  (không đụng `TICK_COUNT`/`EOI_COUNT`, không in — vector **riêng** là điều kiện để không giả tick).
- `vifast-kbin-x86_64/src/main.rs`: `watchdog_arm()`/`watchdog_disarm()` (đọc lại `LVT_TIMER` để
  chứng minh, không tin hằng số) + `measure_timer` arm **trước** vòng `hlt`, mask **ngay sau**;
  trần `WATCHDOG_MAX_WAKES` cho ca bộ đếm PIT chết (`pit_dead`). Dòng bằng chứng mới
  `# timer: watchdog vector=0x32 armed=… wakes=… masked_after=… pit_dead=… ok=…` + TAP 10 đòi
  `armed && wakes > 0 && masked_after && !pit_dead`.
- Ca âm `--features no-pit-irq` (mask IRQ0) — xem §2.2.
- Runner: `run-x86_64.sh` + `soak-x86_64.sh` đòi **đúng một** dòng
  `# timer: watchdog vector=0x32 armed=1 wakes=[1-9]… masked_after=1 pit_dead=0 ok=1`.

### 3.4 Số hiệu TAP — vì sao 48/49 (và `gpu` dời 48 → 50)

Shell sản phẩm (`KT-341b`) chiếm **44..47**; `gpu` chiếm **48**. Bộ ca SMP lấy **48/49** (số kế
tiếp tự do) nên `gpu` dời sang **50** (`tap_first + 35`), `gpu-x86_64.sh` `EXPECT_TESTS=50`.
`TAP_TESTS = 49 + gpu`; `run-x86_64.sh` `EXPECT_TESTS=49`. TAP 1..47 **không** đổi nghĩa.

## 4. Cổng đã chạy

| Cổng | Kết quả |
|---|---|
| `cargo test -p vifast-ktest` (host) | **10/10** ca xanh (6 `shared_suite` + 4 `smp_suite`) |
| `cargo test -p vifast-plat-x86_64` (host) | xanh — `apic` 24/24, `exc` 27/27, `smp` 21/21 (cập nhật `STUB_COUNT` 20 → 21 + `VEC_WATCHDOG`) |
| `./v2/bin/v2 t0 vifast-ktest vifast-plat-x86_64` | **PASS** (2 crate) |
| `bash v2/test/boot/run-x86_64.sh` | **39/39 kiểm sạch**, `tests=49`, `exc=6`, **33/33 ca âm ĐỎ**, `qemu_own=0` |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **58/58** (2 dương + 56 âm) — PASS |
| `bash v2/test/boot/budget-x86_64.sh` (ktest) | `data+bss=67152 ≤ 262144` (headroom **194.992 B**) |
| `python3 v2/tools/check-wake-paths.py --combined` | `strict=1 errors=0 warnings=0` |
| `./v2/bin/v2 check` | **PASS** (`TỔNG check 176.344 ms`) |
| `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` (T1) | `gate-steps registered=80 ran=66 skipped=14 failed=1` — **1 bước đỏ: `selftest pre-measure (KT-95)`**, xem §4.1 |

### 4.1 Vì sao T1 còn **một** bước đỏ (không phải hồi quy)

```text
--- selftest pre-measure (KT-95)
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
--- selftest pre-measure (KT-95): FAIL
# gate-steps registered=80 ran=66 skipped=14 failed=1
# step-time-total   518934 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

`v2/tools/pre-measure.sh` **từ chối đo trên cây bẩn** (`[ "$DIRTY" = "0" ] || refuse 7 tree-dirty`,
dòng 209) với `DIRTY` tính trên **build-inputs**. Worktree này có **15** tệp build-input chưa
commit (đúng phần KT-413) ⇒ `DIRTY=1` ⇒ ca tự kiểm (a) — vốn dựng ảnh giả `dirty=0` rồi đòi
`PRE-MEASURE: OK` — **phải** đỏ. Đây là **thiết kế** (chống đo ảnh không khớp cây), không phải
hồi quy; **cùng** hiện tượng đã ghi ở `JOURNAL/kt411-ktest-generic.md` §T1 (KT-411): bước này chỉ
xanh **sau khi lead commit**. Mọi bước T1 khác đều **OK** (kể cả `wake paths + --strict` và
`wake theo dep-info` — hai bước đã đỏ ở lượt T1 đầu vì `smp_suite` gọi `wake()`; đã sửa bằng
đường sản phẩm `notification_wait`/`notification_signal`).

## 5. Việc **chưa** làm (và vì sao)

### 5.1 Ưu tiên 2 — gỡ 2.162 dòng harness `sched.rs` (`KT-340e`): **KHÔNG** làm

Con số 2.162 = `kbin-x86_64/src/sched.rs` **1.376** + `plat-x86_64/src/sched.rs` **786** (khớp
QUEUE). Gỡ nó **không** phải việc cơ học: 69 dòng `# sched:` trong `run-x86_64.sh` (32 ca âm)
bám **nguyên văn** các dòng bằng chứng của mô hình tự viết (`TASK_SEQUENCE`, `prio_pick`,
`sleep_ok`, `MH_*`). Thay "ai chạy tiếp" bằng `Machine::dispatch(hart)` **đổi giá trị** các dòng
ấy (scheduler của core chọn theo ưu tiên thật) ⇒ phải sửa **kỳ vọng** runner — đúng việc
`KT-340c` §6.2 đã ghi là **cần Agent 0 duyệt** (D2 mới), và đúng luật tuyệt đối của task này
("KHÔNG sửa kỳ vọng harness"). Ngoài ra đột biến `no-sched-lock` (đòi `# sched: BAD double-pick`)
mất nghĩa khi hàng đợi tự viết bị xoá — phải thay bằng đột biến mới của `dispatch` trong
`v2/test/mutants.toml` (tệp harness). **Đề xuất**: task riêng có D2 duyệt đổi hợp đồng 32 ca âm
`# sched:` sang `tcb=<ObjId>` (đúng §6.3 bước 2 của `KT-340c`).

### 5.2 Ưu tiên 3 — PCID/ASID: **KHÔNG** làm

Runner `run-x86_64.sh` đang **đòi** `# ipi: pcid … cr4_pcid=0 cr4_pge=0 full_flush=1
reason=no-pcid-per-va-invlpg` (dòng 1101-1106) — tức hợp đồng hiện hành là **tắt PCID có kiểm
soát** (nhánh đóng lỗi v1 **N5**). Bật `CR4.PCIDE` làm `cr4_pcid=1` ⇒ runner **ĐỎ** trừ khi sửa
kỳ vọng — lại là việc cần D2 duyệt. Thêm nữa, bật PCID mà **không** cấp PCID riêng cho từng
`VSpace` thì `invlpg`/`mov cr3` vẫn chỉ chạm PCID 0 ⇒ **không** có lợi ích thật; muốn có lợi phải
làm `VSpace` mang PCID + shootdown theo PCID (`INVPCID`) — việc của một task riêng.

## 6. Rủi ro / cần lead quyết

1. **`gpu` dời TAP 48 → 50**: `gpu-x86_64.sh` `EXPECT_TESTS=48 → 50` và `net.rs`
   `tap_first + 33 → + 35`. Đã build `--features ktest,gpu` xanh nhưng **chưa** chạy
   `gpu-x86_64.sh` (cần `-device virtio-gpu-pci` + screendump; T2 mới chạy). Nếu lead muốn giữ
   `gpu` ở 48 thì phải đổi số TAP của bộ ca SMP — **không** còn số trống nào khác (44..47 shell).
2. **`TAP_TESTS` 47 → 49** và `EXPECT_TESTS` 47 → 49: thêm **hai** ca TAP (48/49) — thay đổi
   **hợp đồng** runner theo hướng **siết** (thêm ca), không nới ngưỡng nào.
3. **`# timer: watchdog`** là dòng bằng chứng **mới** mà `run-x86_64.sh` **và** `soak-x86_64.sh`
   **đòi** — ảnh cũ (trước KT-413) sẽ ĐỎ ở cả hai runner. Đúng khuôn `KT-348a` (thêm dòng bất biến
   mới + đòi nó), nhưng lead cần biết đây là **thay đổi hợp đồng ảnh**.
4. **`vifast-ktest` là dep của `vifast-kbin-x86_64`** (optional, chỉ `ktest`): ảnh sản phẩm
   **không** link (đã đo `data+bss=43424` không đổi). `Cargo.lock` kernel đổi 2 dòng.
5. **`migrate=0`** trong dòng bằng chứng: `push_migration` trả 0 vì 5 TCB đều `home` cố định và
   không TCB nào `Ready` ở hart khác home. Đây là **số đo thật**, không phải lỗi; bộ ca chỉ đòi
   `migrate=[0-9]+`.
