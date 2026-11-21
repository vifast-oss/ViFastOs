# KT-297 — sửa **phép kiểm timer** x86_64 (periodic + cửa sổ PIT + divisor)

Trạng thái: **DONE (phép kiểm) / soak 600 chưa 0 lỗi** — nhánh `task/kt297-x64timer`, base `main`
@ `ed721b9e`, 2026-11-21. Sub-agent `x64timer`. Quyết định: **D2-201** (mở K5'), **D2-216**
(vòng 15 — KT-297), **D2-05** (test đỏ trước), **D2-07** (không retry), **D2-198** (uỷ quyền
commit nhánh task), **D2-24** (n ≥ 3/p), **D2-149** (K5' N = 600), **D2-72 Q10** (20 s/boot),
**D2-74 (c)** (không `-icount`), **D2-80** (hash ảnh khớp ELF).

**Không** tuyên bố K5' đạt: K5' = "**như K3**" (TAP đầy đủ smp 1|2|4 **+ soak 600 boot**). KT-297
chỉ sửa **phép kiểm timer**; soak 600 còn boot đỏ vì **lỗi khác** (SMP IPI, preemption `#GP`,
virtio-blk) — xem §5.

## 1. Bối cảnh — 10 boot đỏ của KT-293

KT-293 soak 600 x86_64: **590/600**, 10 boot đỏ. Phân tích lại log thật
(`/tmp/kt293-soak/run-20261121-171325-19822/`) cho thấy **6** boot đỏ ở TAP 10 `timer: ticks`
(journal KT-293 ghi 9; hai boot 250/350 thực ra **qua** TAP 10 rồi timeout ở bước sau, boot 373
đỏ ở TAP 20 SMP):

| boot | `# timer: lapic` | `# timer: hz` | nguyên nhân |
|---|---|---|---|
| 98 | `count0=317000 count1=803001 runs=0` | `hz=84 ticks=3` | `runs=0` **giả** — LAPIC periodic quay vòng |
| 163 | `count0=233000 count1=718001 runs=0` | `hz=222 ticks=3` | `runs=0` giả — quay vòng |
| 197 | `count0=969000 count1=975002 runs=0` | `hz=151 ticks=3` | `runs=0` giả — quay vòng |
| 229 | `count0=18003 count1=472004 runs=0` | `hz=37 ticks=3` | `runs=0` giả — quay vòng |
| 299 | `count0=993000 count1=828000 runs=1` | `hz=0 ticks=0 window_pit=0` | cửa sổ PIT **0** — mốc tick đã chờ sẵn |
| 396 | `count0=993000 count1=496000 runs=1` | `hz=0 ticks=0 window_pit=0` | cửa sổ PIT **0** |

Hai lớp lỗi **của phép đo**, không phải timer hỏng:

1. **`runs`**: `LVT_TIMER` đặt **periodic** (`lvt_timer_value(VEC_TIMER, true)`) nên
   `LAPIC_TIMER_CURRENT_COUNT` **nạp lại**; phép kiểm `count1 < count0` (`main.rs:1038` cũ) giả
   định **one-shot** ⇒ quay vòng cho `runs=0` giả.
2. **`hz`/`ticks_ok`**: đo ba mốc tick bằng busy-wait/`hlt`; dưới TCG QEMU giao tick không đều
   (đo được 9..55 ms cho chu kỳ danh định 10 ms) ⇒ `hz` trải **0..24.350**; mốc tick đã chờ sẵn
   thì vòng chờ thoát trước lần đọc PIT đầu ⇒ `window_pit=0`; một mốc nhảy xa > `MAX_TICK_DELTA`
   ⇒ `ticks_ok=false`.

**Không nới ngưỡng**: TAP 10 vẫn đỏ khi timer hỏng thật; chỉ **sửa phép đo** cho đúng bản chất
periodic + đo trên cửa sổ cố định.

## 2. Thay đổi

| Tệp | Thay đổi |
|---|---|
| `v2/kernel/vifast-plat-x86_64/src/apic.rs` | **mới** `lapic_timer_runs` (periodic: counter **đổi giá trị**, không đòi giảm), `pit_divisor_ok` (đọc lại divisor từ counter PIT), `HZ_MIN`/`HZ_MAX`/`hz_ok` |
| `v2/kernel/vifast-plat-x86_64/tests/apic.rs` | **mới** 4 ca host: `lapic_timer_runs_accepts_periodic_wrap` (4 cặp số **nguyên văn** log đỏ KT-293), `lapic_timer_runs_rejects_stopped_or_garbage`, `hz_ok_literal`, `pit_divisor_ok_literal` |
| `v2/kernel/vifast-kbin-x86_64/src/main.rs` | `timer_init`: chờ counter LAPIC **đổi giá trị** (trần 100.000 vòng) + retry đọc divisor PIT (4 lần, lấy max); `measure_timer`: **cửa sổ cố định 500 ms** bằng `PitClock` (PIT kênh 2) + `hlt`; `eoi_ok` đọc **sau** `cli` (bỏ race); TAP 10 thêm `pit_divisor_ok`; log `# timer: lapic … div_ok=…` |
| `v2/kernel/vifast-kbin-x86_64/Cargo.toml` | **mới** feature đột biến `no-lapic-timer`, `wrong-hz` |

### 2.1 `runs` — periodic

`lapic_timer_runs(count0, count1, initial)` = `count0 != count1 && count0 <= initial &&
count1 <= initial`. Ảnh **chờ** counter đổi giá trị (không đọc hai lần liền nhau), nên
`count1 > count0` (quay vòng) là hợp lệ; `0` (ảnh chụp hiếm) hợp lệ nếu ảnh kia khác;
`0xFFFF_FFFF` (không có APIC) và `> initial` ⇒ fail-closed.

### 2.2 `hz` — cửa sổ cố định 500 ms

`measure_timer` lập trình PIT **kênh 2** mode 2 divisor `0xFFFF`, đọc mốc bằng `PitClock`
(cộng dồn qua mỗi lần counter nạp lại), rồi chờ tới khi cửa sổ đạt **596.591 count = 500 ms**.
Mỗi vòng đọc PIT rồi `hlt` — **bắt buộc**: vòng busy-wait (kể cả đọc PIT thưa) giữ BQL của
QEMU, làm callback PIT của main loop trễ ⇒ đo được **1..7 tick/1 s** thay vì ~40. `hlt` nhường
vCPU, QEMU phục vụ timer rồi đánh thức bằng chính ngắt tick.

`hz = tick_delta × 1.193.182 / window_pit`; `ticks_ok` = cửa sổ **đủ dài** + tick **tăng** +
`hz_ok(hz)` (20..500 Hz). Số tick tuyệt đối không phải hợp đồng.

### 2.3 Vì sao vẫn cần kiểm divisor PIT

Dưới TCG, tốc độ tick **giao được** bị chặn trên bởi độ trễ đánh thức `hlt` (~20 ms) nên `hz`
**bão hoà** ~30..55 Hz cho cả nguồn 100 Hz lẫn 1.000 Hz. Vì vậy TAP 10 còn đòi
`pit_divisor_ok(pit_counter0, PIT_DIVISOR_100HZ)` — counter PIT đọc **ngay sau** khi nạp mode 2
phải còn ≈ divisor (dải `[divisor/4, divisor]`). Đây là **kiểm đúng đắn** (chặt hơn `hz > 0`),
không phải nới ngưỡng.

## 3. Test đỏ trước (D2-05)

Thêm 4 ca host vào `tests/apic.rs` **trước** khi hiện thực ⇒ lượt chạy đầu **không biên dịch
được** (`unresolved imports … hz_ok, lapic_timer_runs, HZ_MAX, HZ_MIN`; sau đó
`pit_divisor_ok`). Sau hiện thực: **23/23 ca** `--test apic` xanh (trước KT-297: 19 ca).

## 4. Đột biến — bộ kiểm có răng

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) `no-lapic-timer` | **không** nạp `LAPIC_TIMER_INITIAL_COUNT` | `runs=0` **thật** ⇒ `not ok 10 - timer: ticks` (ĐỎ) |
| (ii) `wrong-hz` | nạp PIT divisor `PIT_DIVISOR_100HZ / 10` (1.193 ⇒ 1.000 Hz danh định) | `div_ok=0` ⇒ `not ok 10 - timer: ticks` (ĐỎ) |

Log nguyên văn:

```text
# no-lapic-timer
# timer: lapic count0=0 count1=0 runs=0 ioapic_route=1 div_ok=1 nominal_hz=100 pit=0x2e76->0x2b50
# timer: hz=43 ticks=22 window_pit=609191
not ok 10 - timer: ticks

# wrong-hz
# timer: lapic count0=968000 count1=933000 runs=1 ioapic_route=1 div_ok=0 nominal_hz=100 pit=0x04a8->0x02a3
# timer: hz=40 ticks=21 window_pit=619785
not ok 10 - timer: ticks
```

## 5. Soak 600 — kết quả

### 5.1 Lần 1 (commit `8ac57674`) — **595/600**, 5 đỏ

```text
# soak: xong wall=1009s; ok=595 fail=5 tổng=600/600
#   fail 27 timeout-20s      → not ok 20 - smp: ipi delivered (KT-278)
#   fail 127 timeout-20s     → qua TAP 23, treo ở bước sau (KT-286)
#   fail 313 timeout-20s     → qua TAP 22, treo ở bước sau (KT-286)
#   fail 459 timeout-20s     → not ok 25 - blk: sector0 (KT-288)
#   fail 504 timeout-20s     → not ok 10 - timer: ticks, div_ok=0 (ĐỎ **giả** của kiểm divisor)
```

Boot 504: `pit=0x0e2c->0x089f` — counter đọc **trễ ~7 ms** (3.628 thay vì ~11.900) do host
deschedule giữa lúc nạp và lúc latch ⇒ `div_ok=0` **giả**. Đã sửa: **retry** nạp+đọc tối đa 4
lần, lấy giá trị lớn nhất; nới cận dưới `divisor/2 → divisor/4` (commit `01d310d5`).

### 5.2 Lần 2 (commit `01d310d5`) — **599/600**, 1 đỏ **không phải timer**

```text
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=10.96 ncpu=12 verdict=OK
# guard: boot_end=601 rc_124=1 qemu_own=0
# soak: xong wall=967s; ok=599 fail=1 tổng=600/600
#   fail 478 timeout-20s → qua TAP 21, treo ở preemption (KT-286 `#GP`)
# soak: BUILD_ID=bd9bdc1bcfcb25cd sha256=8b2ab234… git=01d310d50ffd7ba6026f38d1cc51b9e861a24d22
# soak: 599/600 lỗi=1
```

**TAP 10 `timer: ticks`: 600/600 xanh, 0 đỏ.** Phân bố 600 boot:

| Phép đo | Kết quả |
|---|---|
| `runs=1` / `ioapic_route=1` / `div_ok=1` | **600/600** mỗi trường |
| `ok 10 - timer: ticks` | **600/600** (`not ok 10` = **0**) |
| `hz` | min **26**, p50 **37**, max **53** (danh định 100; TCG bão hoà — §2.3) |
| `ticks` | 14..27 |
| `window_pit` | ~604.800..623.000 count (500 ms + một tick overshoot) |
| `pit_counter0` | min **3.892**, p1 11.132, p50 11.897, max 11.925; **1/600** dưới 5.966 nhưng **retry** lấy max ⇒ `div_ok=1` |
| boot đỏ | **1** — boot 478, `#GP` storm ở preemption (KT-286), **không** phải timer |

Boot 478 (`#GP` storm, ngoài phạm vi KT-297):

```text
# timer: lapic count0=980000 count1=950000 runs=1 ioapic_route=1 div_ok=1 nominal_hz=100 pit=0x2e7d->0x29b2
# timer: hz=40 ticks=21 window_pit=624460
ok 10 - timer: ticks
# timer: eoi=21 ticks=21 spurious=0
ok 20 - smp: ipi delivered
ok 21 - smp: all harts report
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: ticks=7 tasks=2
# sched: runs a=# exc: vec=13 err=0x0 rip=0x100422 cs=0x8 rflags=0x10046
```

## 6. Còn gì chưa xong / rủi ro

1. **K5' CHƯA ĐẠT**: soak 600 lần 2 **599/600** — boot 478 đỏ vì **preemption `#GP`** (KT-286),
   **không** phải timer; lần 1 còn 4 boot đỏ khác (SMP IPI, preemption, virtio-blk). Các lỗi đó
   **ngoài phạm vi KT-297** và cần task riêng. Không kết luận "ảnh x86_64 ổn định".
2. **`hlt` treo nếu PIT hỏng hoàn toàn**: `measure_timer` chờ tick bằng `hlt`; nếu nguồn tick
   chết hẳn thì boot treo tới hết ngân sách runner (20 s) ⇒ **ĐỎ** (timeout), không phải xanh.
   Đột biến `no-lapic-timer` **không** chạm PIT nên vẫn chạy hết và in `runs=0`.
3. **`hz` giao được bão hoà ~30..55 Hz** dưới TCG (không phải 100 Hz danh định) — đó là số
   **thật** của QEMU TCG, đã ghi rõ; kiểm divisor PIT bù cho phần "sai nhịp".
4. **Không** sửa `v2/bin/v2`; lead nối `soak-x86_64.sh --selftest` vào cổng (đã có từ KT-296).

## 7. Xác nhận

- **Không** push, **không** merge, **không** chạm `main`.
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng cổng (TAP 10 vẫn đỏ khi timer hỏng thật; thêm kiểm divisor + `hz_ok` là
  **chặt hơn**), **không** retry soak (D2-07).
- Commit bằng `VIFAST_ROLE=lead git commit` (uỷ quyền D2-198); cây sạch sau commit.
