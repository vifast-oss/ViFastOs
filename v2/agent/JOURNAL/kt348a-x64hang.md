# KT-348a — hậu kiểm boot "treo sau LAPIC timer" của soak x86_64 (1/600)

**Ngày**: 2026-11-24 (khung 12:40–15:00 JST) · **Nhánh**: `task/kt348a` · **Base**: `main` @ `e5555e5b`
(D2-257). Nửa đầu của lượt chạy trên `4d4cbb62` (đo lại bằng chứng + vá harness); nửa sau merge
`--ff-only` lên `e5555e5b` rồi vá kernel theo D2-257 và **đo lại toàn bộ** trên cây cuối.
**Lệnh**: KT-348 (hậu kiểm nghiệm thu), D2-116, D2-07, D2-05, D2-243 (measure-lock), D2-256
**Ngân sách**: 3 h · **Máy đo**: macOS `mps`, 12 nhân, QEMU 11.1.3, `loadavg_1m` 5,2–6,9 lúc đo

## 0. Kết luận trước (nói thẳng)

- **Boot đó KHÔNG treo vì kernel.** Nó bị **một tiến trình NGOÀI harness gửi `SIGTERM`** và chết ở
  giây **1,1** — trong khi ngưỡng guard là **20 s**. Đo lại log guard của chính lượt nghiệm thu:
  **`rc_124=0`**, 601/601 dòng `boot_end` có **`rc=0`**, `secs` lớn nhất **5,1 s**. Guard **không hề**
  hết hạn ⇒ dòng `qemu-system-x86_64: terminating on signal 15 from pid 73084` **không phải** guard
  giết (guard giết thì `rc=124`).
- **Lỗi thật nằm ở harness, không ở kernel**: `qg_boot` **không đọc mã thoát** của QEMU ⇒ một cái
  chết vì tín hiệu **từ ngoài** bị trả về **`rc=0`** (như boot sạch), rồi soak chấm nó là **“boot đỏ
  của kernel”**. Một `pkill`/OOM/agent khác trên máy đo biến thành “lỗi kernel không tất định” và
  **đốt cả một vòng nghiệm thu 28 phút**.
- **Bản sửa**: phân loại **ba** đường kết thúc (`0` tự thoát / `124` guard tự kill = boot treo thật /
  **`126` tín hiệu từ ngoài**), ghi `ext_sig=`+`killer=` vào log guard, và soak **TỪ CHỐI ĐO**
  (`exit 3`, D2-116) thay vì chấm điểm. **Không nới ngưỡng nào** (20 s, N, p, trần ảnh giữ nguyên).
- **Ca đỏ-trước có răng**: `guard_extsig_selftest.sh --mode old` (đúng đoạn mã cũ) ⇒ `rc=0` — **ĐỎ**;
  `--mode guard` ⇒ `rc=126`, `ext_sig=15` — **XANH**. Tái hiện được **bằng máy**, không cần ảnh khách.
- **Theo D2-257 (A0 khoanh vùng) tôi sửa thêm hai lỗi thật trong kernel** (§4b): (1) `timer_init`
  **mở mask** redirection IRQ0 **trước** khi đặt `Destination` (và `ioapic_init` **không** bao giờ
  xoá `Destination`) ⇒ cửa sổ ngắt đi sai đích; (2) `LVT_TIMER` arm **không mask** trong cửa sổ probe
  trên **cùng vector 32** ⇒ ngắt probe chen vào lúc đo. Cả hai **đo được** bằng bất biến mới
  `# timer: lapic-window …` (đọc lại **thanh ghi**), **bắt buộc** trong `check_boot_log` và trong
  **TAP 10** ⇒ ảnh **cũ** (không có dòng đó) là **ĐỎ**, ảnh mới **XANH**. Điểm (3) của D2-257
  (`hlt` ngủ vĩnh viễn khi mất ngắt) **không sửa** — lý do + thiết kế đề xuất ở §7 (nợ N-3).

## 1. Bằng chứng gốc đo lại (không đoán)

Lượt nghiệm thu: `target-task/nightly/nightly-20261124-022901-4d4cbb62/soak-x86_64/`.

| Đại lượng | Số **đo** | Nguồn |
|---|---|---|
| Boot đỏ | `fail 7` — “có 9 dòng 'ok', cần 47” | `run-20261124-115017-77229/results.txt` |
| Dòng cuối log boot 7 | `qemu-system-x86_64: terminating on signal 15 from pid 73084 (<unknown process>)` | `run-…/boot-7.log:37` |
| `boot_end` của boot 7 | `rc=0 secs=1.1 isolated=1` | `qemu-guard.log` (seq=4, pid=83055) |
| `boot_end` toàn lượt | **601** dòng, **601 `rc=0`**, **0 `rc=124`** | `qemu-guard.log` |
| Phân bố thời gian boot | n=601 · min **1,1 s** · p50 **3,3 s** · p99 **4,4 s** · max **5,1 s** · ngưỡng **20 s** | `qemu-guard.log` |
| Dòng tổng kết của chính soak | `# guard: boot_end=601 rc_124=0 qemu_own=0` | `step-3-soak.log` |
| Số QEMU lạ / mồ côi | `qemu_foreign=0 qemu_ppid1=0` (mẫu “trước”) | `step-3-soak.log:6` |
| Tổng lượt | `# soak: xong wall=1698s; ok=599 fail=1 tổng=600/600` | `step-3-soak.log:740` |

**Hai loại bỏ bằng số (đây là chỗ brief gốc suy sai):**

1. **“guard timeout 20 s quá ngắn khi máy tải” — SAI.** p99 **4,4 s**, max **5,1 s** ⇒ còn **≈ 4×**
   headroom; `rc_124=0` trên **601** boot. Guard chưa bao giờ chạm ngưỡng.
2. **`pid 73084` không thuộc phép đo của chúng ta.** Registry guard có **602 pid phân biệt** (601
   boot + probe) — **không có** 73084; tiến trình soak là `77229`, các làn 80–98k. `73084` nằm trong
   dải pid sinh lúc **02:34 UTC** (đầu bước soak aarch64 của cùng nightly, script pid **73050**) và
   còn sống tới **02:50:2x** ⇒ **tiến trình ngoài** gửi tín hiệu, không phải guard, không phải soak.
3. **“tranh chấp measure-lock” — SAI (đã kiểm)**: hai dòng
   `# CẢNH BÁO (measure-lock): release bị bỏ qua: khoá của pid=83671 …` ở cuối step-2/step-3 **là
   hành vi đúng của chế độ uỷ quyền**: `83671` = tiến trình cha `v2 nightly` (nó giữ khoá và
   `export V2_MEASURE_LOCK_DELEGATED_BY` ở `v2/bin/lib/nightly.sh:657`); con **không** được xoá khoá
   của cha. Không có “kẻ cướp khoá”, không có hai phép đo chồng nhau.

## 2. Chuỗi TAP 9 → TAP 10: bước nào chờ gì

`v2/kernel/vifast-kbin-x86_64/src/main.rs`:

| Bước | Dòng | Việc | Chờ gì |
|---|---|---|---|
| TAP 9 `ok 9 - apic: ioapic` | 941–955 | `ioapic_init()`: đọc ID/VER, **mask toàn bộ** 24 redirection entry | MMIO IOAPIC — **không** chờ ngắt |
| `timer_init()` | 958 → 520–607 | (1) LAPIC: `TIMER_DIVIDE=1`, `LVT_TIMER` periodic → nạp `INITIAL_COUNT=1e6` → chờ `CURRENT_COUNT` **đổi giá trị** (`LAPIC_RUNS_SPIN_BOUND=100_000`) → **mask** LVT; (2) **PIT kênh 0 mode 2** divisor `11_932` = **100 Hz**, đọc lại counter (tối đa 4 lần, `pit_divisor_ok`); (3) IOAPIC: **mở đúng** redirection IRQ0 → **vector 32**, dest 0 | vòng spin, **không** chờ ngắt (IF=0) |
| in `# timer: lapic …` | 963–974 | bằng chứng cấu hình | — |
| **`sti`** | **975** | ngắt bật **lần đầu** trong boot | từ đây `PIT IRQ0 → IOAPIC redir 0 → LAPIC vector 32` mới được giao |
| `measure_timer()` | 976 → 644–680 | nạp **PIT kênh 2** divisor `0xFFFF`, rồi vòng: `clock.update(pit2_latch_read()); t1=TICK_COUNT; nếu counts ≥ HZ_WINDOW_COUNTS(596_591 ≈ 500 ms) hoặc spins ≥ 500_000 thì thoát; spins+=1; **`hlt`**` | **ngắt tick** (mỗi `hlt` chỉ thức khi có ngắt) — `hlt` **không** có timeout trong khách |
| in `# timer: hz=…` + `ok 10` | 981–995 | `ticks_ok` = cửa sổ đủ dài ∧ `tick_delta ≥ 1` ∧ `hz_ok` | — |

Handler tick: `v2/kernel/vifast-kbin-x86_64/src/trap.rs:193–207` — `if id == bsp_lapic_id()`
⇒ `TICK_COUNT += 1`, `lapic_eoi()`, `EOI_COUNT += 1`; spurious (`VEC_SPURIOUS`) chỉ đếm, **không** EOI
(đúng SDM 3A §10.9). `TPR=0`, PIC bị che, `LINT0/1` bị mask (`main.rs:406–436`).

⇒ Nếu **thật sự** mất ngắt thì boot **đứng trong `hlt` vô hạn** ⇒ guard phải kill ở **20 s** ⇒
`rc=124`. Đo được: **không có `rc=124` nào** ⇒ nhánh này **không xảy ra** trong lượt đó.

## 3. Đo thời gian thật từng dòng serial (số quyết định)

8 boot liên tiếp, 1 QEMU/lượt, **cùng ảnh ktest của lượt nghiệm thu**
(`sha256 a079b57e…`, BUILD_ID `6de36ef7341dbdfd`), mốc thời gian do `perl Time::HiRes` bám stdout QEMU
(sai số ≈ 20 ms do fork; log: `/tmp/kt348a/ts-*.log`):

| boot | `# timer: lapic` | `# timer: hz` / `ok 10` | thoát | `ok` |
|---|---|---|---|---|
| 1 | 0,358 s | 2,530 s | 9,351 s | 47 |
| 2 | 0,269 s | 2,319 s | 9,788 s | 47 |
| 3 | 0,350 s | 2,611 s | 6,322 s | 47 |
| 4 | 0,149 s | 2,743 s | 5,963 s | 47 |
| 5 | 0,179 s | 2,162 s | 6,059 s | 47 |
| 6 | 0,205 s | 1,932 s | 4,897 s | 47 |
| 7 | 0,154 s | 2,533 s | 5,490 s | 47 |
| 8 | 0,121 s | 1,906 s | 4,855 s | 47 |

⇒ **Cửa sổ `hlt` chiếm 1,7–2,2 s** và **bắt đầu ở 0,12–0,36 s**. Boot 7 của lượt nghiệm thu bị giết ở
**1,1 s** — tức **ngay giữa cửa sổ đo bình thường**: chỗ log dừng lại **không mang tín hiệu lỗi
kernel nào**. (Nếu là treo thật, nó phải sống tới 20 s.)

## 4. Bản sửa (harness — không nới ngưỡng)

Triết lý: **phân loại đúng nguyên nhân kết thúc** thay vì đổi ngưỡng thời gian. Ba đường tách bạch:

| Mã thoát `qg_boot` | Nghĩa | Soak chấm |
|---|---|---|
| `0` | QEMU tự thoát (khách `halt`) | `ok` nếu log đạt bất biến |
| `124` | **guard** tự kill vì quá `tds` (20 s) | `fail … timeout-20s` — **boot treo thật** của khách |
| **`126`** (mới) | **tín hiệu từ tiến trình NGOÀI**, trước hạn | **TỪ CHỐI ĐO** (exit 3, D2-116) — **không** chấm kernel |

| Tệp | Dòng | Nội dung |
|---|---|---|
| `v2/test/boot/qemu_guard.sh` | 25–36, 44–49, 60–62, 132–152 | hợp đồng mới `126`; `qg_parse_killer()`/`qg_parse_sig()` đọc `terminating on signal N from pid P` **từ log QEMU** (D2-102: giá trị đo, không phải hằng) |
| | 328–347 | `qg_boot`: **đọc mã thoát** `wait`; `128 ≤ st ≤ 191` ⇒ `QG_LAST_SIG=st-128`, `QG_LAST_KILLER`, `rc=126` |
| | 401–410 | `qg_guard_note boot_end` thêm `ext_sig=`/`killer=` — **bằng chứng phân loại nằm trong log** |
| `v2/test/boot/soak-x86_64.sh` | 1360–1366 | `INVALID_MARK`/`INVALID_REASON` (dấu “phép đo không hợp lệ”) |
| | 1374–1396 | `boot_one`: làn khác đã đánh dấu ⇒ dừng; `rc=126` ⇒ ghi `invalid <i> ext-signal sig=… killer=…`, **không** ghi `fail` |
| | 1418–1436 | `rc_126=` trong dòng `# guard:`; `soak_x64_refuse_check` in lý do + pid kẻ giết |
| | 1453 | `SOAK_REFUSE_CHECK=soak_x64_refuse_check` |
| `v2/test/boot/lib/soak-common.sh` | 105, 111–117 | hook `SOAK_REFUSE_CHECK` chạy **trước mọi** nhánh kết luận ⇒ `rc=3`; không thể vô tình PASS/FAIL |
| | 303–328 | 2 ca selftest mới (hook xanh giữ PASS; hook đỏ ⇒ `rc=3` trước nhánh FAIL) ⇒ **13/13** |
| `v2/test/boot/guard_extsig_selftest.sh` | mới (220 dòng) | ca kiểm phân loại: parse killer/sig từ **log thật KT-348**; `--mode old` **ĐỎ** (rc=0); `--mode guard` ⇒ 126 + `ext_sig=15` + `killer=`; log sạch ⇒ `killer` rỗng (không bịa pid) |

**Không** đổi: `TIMEOUT_DS=200` (20 s), N=600, `p=5/1000`, trần ảnh, TAP, `check_boot_log`, `expected.txt`.

## 4b. Bản sửa **kernel** theo D2-257 (A0 khoanh vùng, base `e5555e5b`/`da75f5b`)

D2-257 chỉ đúng **ba** điểm trong `v2/kernel/vifast-kbin-x86_64/src/main.rs`. Kết quả **đo lại độc
lập** của tôi trên base `da75f5b`:

| Điểm | Hiện trạng trên `da75f5b` | Phán quyết của tôi |
|---|---|---|
| **(1)** `timer_init()` mở mask **trước** khi đặt `Destination` | `ioapic_write(reg, VEC_TIMER)` **rồi** `ioapic_write(reg + 1, 0)` — đúng như A0 nói; `ioapic_init()` **chỉ** mask word thấp, **không** xoá word cao ⇒ `Destination` giữ giá trị firmware | **ĐÚNG là lỗi thật** (thứ tự nguy hiểm trên phần cứng thật). Trên QEMU hệ quả **quan sát được = 0** vì reset/SeaBIOS để `Destination = 0` = đúng BSP LAPIC ID; nên **không** thể là nguyên nhân của 1/600 đỏ (xem §1: boot đó bị giết từ ngoài). Vẫn **phải sửa** — và bất biến mới bắt được nó **bằng máy**. |
| **(2)** cửa sổ probe `LVT_TIMER` **không mask** trên **cùng vector 32** | `lvt_timer_value(VEC_TIMER, true)` không kèm `LVT_MASKED`; chỉ mask **sau** khi đo counter | **ĐÚNG là lỗ hổng thật** (ngắt probe có thể chen vào cửa sổ đo; nếu bắn trước `sti` thì bit IRR treo tới `sti`). Sửa **không mất gì**: mask **không** dừng bộ đếm (SDM 3A §10.5.4) nên phép kiểm “timer chạy” giữ nguyên giá trị. |
| **(3)** `measure_timer()` gọi `hlt` — mất ngắt ⇒ ngủ vĩnh viễn | `asm::hlt()` trong vòng chờ tick | **Đúng về cơ chế**, nhưng **không sửa được an toàn trong 3 h**: `hlt` chỉ thức khi **có** ngắt; nguồn ngắt duy nhất là PIT (100 Hz) qua chính IOAPIC/LAPIC đang nghi ngờ. Không có nguồn đánh thức thứ hai thì mọi “hlt có hạn” đều vô nghĩa; bỏ `hlt` thì đo được **1..7 tick/s** (KT-297 ghi) ⇒ TAP 10 đỏ **mọi** boot. Xem nợ §7. |

**Đã sửa (kernel, `main.rs`):**

| Dòng | Nội dung |
|---|---|
| `ioapic_init()` | mask word thấp **rồi xoá luôn `Destination`** (`ioapic_write(reg + 1, 0)`) trong lúc entry còn mask |
| `timer_init()` LVT | arm `LVT_TIMER` **kèm `LVT_MASKED`** ngay từ đầu; đọc lại thanh ghi ⇒ `lvt_probe_masked` |
| `timer_init()` route | **đảo thứ tự**: ghi `Destination` = **BSP LAPIC ID đo được** (`smp::bsp_lapic_id()`, không hardcode `0`) **trước**, rồi mới ghi word thấp để mở mask; đo tiền điều kiện `masked_before` |
| **dòng bằng chứng mới** | `# timer: lapic-window lvt_masked=… masked_before_unmask=… dest_bsp=… order=dest_first` (dòng `#` riêng ⇒ **không** đổi regex `$` của `# timer: lapic …`) |
| TAP 10 | thêm **3** điều kiện: `lvt_probe_masked ∧ ioapic_masked_before ∧ ioapic_dest_ok` ⇒ lỗi thứ tự làm **TAP 10 ĐỎ**, không chỉ đỏ một dòng phụ |
| `soak-x86_64.sh` `check_boot_log` | **bắt buộc** đúng 1 dòng `lapic-window` với **cả ba** trường `=1`; thiếu/sai ⇒ soak ĐỎ |
| `soak-x86_64.sh --selftest` | **+4 ca âm** (đột biến từng trường `lvt_masked`/`masked_before_unmask`/`dest_bsp` + xoá dòng) ⇒ **58/58** |

**Ca đỏ-trước của D2-257 (có răng, đo được):** trên ảnh kernel **trước** bản sửa, dòng
`# timer: lapic-window …` **không tồn tại** ⇒ `check_boot_log` mới báo
`# dòng '# timer: lapic-window …' xuất hiện 0 lần, cần đúng 1` ⇒ soak **ĐỎ**. Ảnh cũ dựng lại bằng
`git stash`/base `e5555e5b` (không cần chạy 600 boot: chỉ cần **1** boot + `check_boot_log`).


## 5. Ca đỏ-trước (tái hiện được, không cần ảnh khách)

`bash v2/test/boot/guard_extsig_selftest.sh --mode old|guard|both`

- `--mode old` = **đúng đoạn mã trước KT-348a** (`wait` rồi trả `0`, không đọc mã thoát) ⇒ QEMU bị
  `SIGTERM` từ ngoài **vẫn** trả `rc=0` ⇒ soak chấm “boot đỏ của kernel” — **đây chính là lỗi KT-348**.
- `--mode guard` = `qemu_guard.sh` hiện tại ⇒ `rc=126`, `# boot_end … rc=126 ext_sig=15 killer=none`.

## 6. Kiểm chứng (đo trong lượt này, trên **cùng** cây `4d4cbb62` + bản sửa)

### 6.1 `soak-x86_64.sh 600 3` — **PASS, nguyên văn**

```text
# git HEAD=4d4cbb6223c8845ec42f886c201fb9ec9912e14c dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=6.89 … verdict=OK
# measure-lock: ĐÃ GIỮ khoá máy đo (pid=41108 host=mps nhãn="soak x86_64 N=600 lanes=3")
# image=product harness-strings=0 (ktest=65 ⇒ bộ quét có răng)
# image=x86_64-product-shell … file=224384 arch=x86_64(e_machine=62) data+bss=51176 budget=262144 label_budget=262144
# image=x86_64-ktest … file=333704 arch=x86_64(e_machine=62) data+bss=66840 budget=262144 label_budget=262144
# probe: '# conf image:' khớp ELF (build=92285e183ba72d86 text=200704)
# probe: log đạt mọi bất biến của một boot sạch (TAP 47/47 + marker + halt)
# soak: bắt đầu N=600 lanes=3 04:14:49Z
# guard: boot_end=601 rc_124=0 rc_126=0 qemu_own=0
# soak: xong wall=1701s (0 boot/s); ok=600 fail=0 tổng=600/600
# soak: BUILD_ID=92285e183ba72d86 sha256=8c9d03f422db79b32e08f251e4ab5c016e7f2782cb0cc90539db61f5e6cfde32 git=4d4cbb6223c8845ec42f886c201fb9ec9912e14c
# soak: 600/600 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/600 lỗi (PASS)
# soak-x86_64.sh: PASS
# measure-lock: đã nhả khoá (pid=41108)
```

- **TAP 47/47** và **budget 2 nhãn xanh** ở trên đều do chính soak in (probe + `budget-x86_64.sh`).
- **Phân bố thời gian boot của lượt này**: n=**601** · min **1,7 s** · p50 **3,2 s** · p99 **4,2 s** ·
  max **4,8 s** — **cùng hình dạng** với lượt nghiệm thu hỏng (p50 3,3 / max 5,1) ⇒ boot hôm nay
  **không** chậm hơn, nên nếu ai đó định “nới ngưỡng 20 s” thì số đo nói **không cần**.
- `rc_124=0` **và** `rc_126=0`: không boot nào bị guard kill, không boot nào bị tín hiệu ngoài.

### 6.1b `soak-x86_64.sh 600 3` trên **kernel đã sửa D2-257** (cây cuối)

Cùng lệnh, log `/tmp/kt348a/soak-final2.log`, ảnh `BUILD_ID=83c6a2fd0e2cb8f5`, patch kernel
`sha256=a8258c6d…`, `dirty-kernel-paths=1` (chưa commit lúc đo — commit `[KT-348a]` chứa đúng patch đó):

```text
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=3.96 … verdict=OK
# measure-lock: ĐÃ GIỮ khoá máy đo (pid=18669 host=mps nhãn="soak x86_64 N=600 lanes=3")
# image=product harness-strings=0 (ktest=65 ⇒ bộ quét có răng)
# image=x86_64-product-shell … file=224384 data+bss=51176 budget=262144 label_budget=262144
# image=x86_64-ktest … file=333704 data+bss=66840 budget=262144 label_budget=262144
# probe: '# conf image:' khớp ELF (build=83c6a2fd0e2cb8f5 text=200704)
# probe: log đạt mọi bất biến của một boot sạch (TAP 47/47 + marker + halt)
# soak: bắt đầu N=600 lanes=3 05:04:21Z
# guard: boot_end=601 rc_124=0 rc_126=0 qemu_own=0
# soak: xong wall=2066s (0 boot/s); ok=600 fail=0 tổng=600/600
# soak: BUILD_ID=83c6a2fd0e2cb8f5 sha256=c3ff54ced92a73c786b141ee682972863bda60f0235860c77bd16b6f9a9011a8 git=e5555e5bda05424aecbd4cba7a34559c32d71a9a
# soak: 600/600 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/600 lỗi (PASS)
# soak-x86_64.sh: PASS
# measure-lock: đã nhả khoá (pid=18669)
```

**§6.1c — kết quả cuối**: **600/600, 0 lỗi** trên cây cuối (`e5555e5b` + patch `[KT-348a]`), phân bố
thời gian boot n=**601** · min **2,1 s** · p50 **3,5 s** · p99 **5,3 s** · max **6,1 s** (ngưỡng 20 s
⇒ còn **3,3×**). `rc_124=0` **và** `rc_126=0`. **600/600** boot in đúng dòng bất biến
`lapic-window lvt_masked=1 masked_before_unmask=1 dest_bsp=1` — tức phép kiểm D2-257 chạy trên
**mọi** boot chứ không phải một lần.

**Probe đã xanh với bất biến MỚI**: `check_boot_log` bắt buộc dòng
`# timer: lapic-window lvt_masked=1 masked_before_unmask=1 dest_bsp=1 order=dest_first`, nên
“TAP 47/47” ở trên là **đã bao gồm** phép kiểm thứ tự D2-257 (không phải chỉ TAP cũ).

### 6.2 Đỏ-trước mức **soak** (tái hiện sự cố bằng tay)

Kịch bản: `soak-x86_64.sh --smoke 4 2` + `/tmp/kt348a/kill_one.sh` (tiến trình **ngoài** đọc
`qemu-guard.log`, lấy `pid=` của boot đang chạy, `kill -TERM` sau 1,0 s) — tái hiện **đúng** dòng
`signal 15 from pid <ngoài>` của sự cố.

- Lượt **đầu** bị **`measure-lock` TỪ CHỐI** vì agent `kt348b` đang giữ khoá (`soak aarch64 N=1000
  lanes=3`, pid 87624) — đúng fail-closed, **không** chạy chồng:
  `# TỪ CHỐI ĐO (measure-lock): máy đo đang bận — KHÔNG chạy chồng cổng/soak.` (`EXIT=2`).
- Lượt đo lại sau khi khoá nhả chưa kịp chạy trong khung này (agent `kt348b` giữ khoá liên tục
  cho `soak aarch64 N=1000`), nên phần **phân loại `126`** được chứng minh ở **hai** tầng rẻ hơn:
  (i) `guard_extsig_selftest.sh` — tiến trình **ngoài** `kill -TERM` đúng QEMU ⇒ `qg_boot` trả
    **126**, `# boot_end … rc=126 ext_sig=15` (ca 4–6) và `--mode old` **trả 0** (ca 3, đỏ-trước);
  (ii) `soak-common.sh --selftest` ca 13 — kết quả có dòng `invalid … ext-signal` + hook
    `SOAK_REFUSE_CHECK` ⇒ **rc=3 TỪ CHỐI ĐO**, **trước** nhánh FAIL.
  Đường `boot_one` → `invalid` → `refuse` (8 dòng trong `soak-x86_64.sh`) **chưa** có ca
  end-to-end; ghi nợ §7 (cần một lượt `--smoke` khi khoá rảnh).

### 6.3 `run-x86_64.sh` (ảnh **sản phẩm** tối giản, 1 QEMU/lượt)

```text
# 37/37 kiểm sạch (boot=1, tests=47, exc=6)
# infra: cpu=qemu64 guard_log=/tmp/kt348a/run-x64c/qemu-guard.log qemu_own=0
EXIT=0
```

Đây là lượt trên **kernel đã sửa D2-257** (lượt đầu, trên kernel cũ, cũng 37/37 — không đổi).
`tests=47` là TAP 47/47 của ảnh kiểm, `exc=6` là sáu dòng `# exc:`.

### 6.4 Selftest (không QEMU) — đã chạy trong lượt này

| Lệnh | Kết quả |
|---|---|
| `bash v2/test/boot/lib/soak-common.sh --selftest` | **13/13 PASS** (thêm 2 ca `SOAK_REFUSE_CHECK`) |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **54/54 PASS** (2 dương + 52 âm; `check_boot_log` không đổi hành vi) |
| `bash v2/test/boot/guard_extsig_selftest.sh` (mode both) | **7/7 PASS** — gồm ca **ĐỎ-TRƯỚC** `--mode old` |
| `bash v2/bin/v2 rules` | **PASS** (`E1 tên tệp NTFS: PASS (2891 tên tracked)`) |
| `bash -n` trên 3 tệp sửa | OK |


## 7. Nợ / điều **chưa** chứng minh (ghi rõ, không giấu)

- **N-1 — chủ của `pid 73084` chưa xác định** (không có log của tiến trình đó). Đã chứng minh **không
  thuộc** phép đo này (registry guard: 602 pid, không có 73084) và **không phải** guard (guard giết ⇒
  `rc=124`; đây `rc=0`). Dải pid: lớn hơn script soak aarch64 `73050` và nhỏ hơn boot đầu tiên của nó
  `73354` ⇒ sinh trong **cửa sổ khởi tạo của bước soak aarch64** (02:34 UTC) và **còn sống** tới
  02:50:26 UTC (≈16 min). Cơ chế nó chọn đúng pgid `83055` **chưa chứng minh được** — **[GT]**:
  một `kill`/`qg_kill_group` còn sót của bước trước trúng **pid được cấp lại** (máy đã quay vòng pid
  trong lượt: pid boot cuối `67–68k` **nhỏ hơn** pid đầu `80448`). Bản sửa này làm lần sau **tự khai
  báo** (`killer=` trong log guard + TỪ CHỐI ĐO thay vì đỏ oan).
- **N-2 — `qg_boot_kill`** (`persist --crash`) **không** đọc mã thoát: nếu bị giết từ ngoài vẫn trả
  `0`. Chưa cần cho soak; cùng khuôn sửa 5 dòng.
- **N-3 — `hlt` vô hạn khi mất ngắt (điểm (3) của D2-257)**: **không sửa** trong 3 h. Lý do đo được:
  `hlt` chỉ thức khi **có** ngắt, nguồn duy nhất là PIT 100 Hz qua đúng IOAPIC/LAPIC đang nghi ngờ;
  bỏ `hlt` ⇒ đo được **1..7 tick/s** (KT-297 ghi) ⇒ TAP 10 **đỏ mọi boot**. **Thiết kế đề xuất**
  (task riêng, cần re-soak): thêm **một vector IDT riêng** (ví dụ `0x31`) + stub + handler chỉ
  `WATCHDOG_WAKES += 1` rồi `EOI`, arm **LAPIC timer one-shot 20 ms** **chỉ trong** `measure_timer`;
  vòng đo đổi thành `if WATCHDOG_WAKES` tăng mà `TICK_COUNT` không tăng quá K lần ⇒ **break** với
  `ticks_ok=false` (TAP 10 **đỏ** kèm lý do) thay vì ngủ vĩnh viễn. Điều kiện ĐẠT: **cùng** số tick
  đo được trên 100 boot với bản hiện tại + ca âm (ngắt PIT bị mask ⇒ boot **vẫn thoát**, TAP 10 đỏ).
- **N-4 — ba runner còn lại** (`soak.sh`, `soak-aarch64.sh`, `ktest.sh`) nhận `126` nhưng chỉ in
  “timeout/không thoát” (không phân loại TỪ CHỐI ĐO); cùng khuôn, copy ~10 dòng mỗi tệp.
- **N-5 — `guard_extsig_selftest.sh` chưa vào `gate --dev`** (bước “selftest qemu guard” nằm trong
  `v2/bin/v2` — **cấm sửa** theo D2-253/254). Lead nối 1 dòng hoặc gọi tay trong T2.
- **N-6 — ca end-to-end `boot_one → invalid → refuse`** chưa chạy được trong khung này: agent
  `kt348b` giữ `measure-lock` cho `soak aarch64 N=1000` suốt 15 min cuối, runner `soak-x86_64.sh`
  đúng luật **TỪ CHỐI ĐO** khi khoá bận. Đã phủ bằng hai tầng rẻ (guard 126 + hook refuse rc=3).
- **N-7 — P20 rộng hơn đã ghi**: hai lần build **cùng commit, dirty=0** nhưng ở hai thư mục khác nhau
  cho `BUILD_ID` **khác** (`6de36ef7341dbdfd` của lượt nghiệm thu so với `92285e183ba72d86` của lượt
  đo lại) — không chỉ `sha256` phụ thuộc thư mục build mà **cả digest `.text`**. Quan sát, **chưa**
  kết luận nguyên nhân; cần điều tra trước mọi tuyên bố “tái lập byte”. Không ảnh hưởng kết luận
  KT-348a (mỗi lượt đo đều neo BUILD_ID của **chính** ảnh nó dùng).
