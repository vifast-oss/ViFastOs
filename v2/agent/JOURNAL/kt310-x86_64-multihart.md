# KT-310 — **nhiều hart schedule** trên x86_64: hàng đợi chạy toàn cục + khoá

Trạng thái: **DONE** — nhánh `task/kt310-x64smpsched`, base `main` @ `81b8e210`, 2026-11-22.
Sub-agent `x64smpsched`. Quyết định: **D2-201** (K5' bước 1), **D2-212** (KT-286 preemption),
**D2-216** (KT-297/298), **D2-221** (nhiệm vụ KT-310), **D2-198** (uỷ quyền commit nhánh task),
**D2-05** (test đỏ trước), **D2-07** (không retry), **D2-17** (`unsafe` chỉ trong danh sách đóng).

**Không** tuyên bố K5' đạt.

## 1. Khảo sát trước (ba câu hỏi của task)

### (1) `sched.rs` có runqueue **toàn cục** hay per-hart? — **Không có runqueue nào cả**

`vifast-plat-x86_64::sched` **không** có hàng đợi: nó là **bảng cố định** `TASK_SEQUENCE`
(`[B, A, B, A, B, U, A, U, MAIN]`, 9 bước) + hàm thuần `next_task(current, switches, max)`.
`vifast-kbin-x86_64::sched` giữ **trạng thái toàn cục** (`SCHED_RSP[slot]`, `SCHED_CURRENT`,
`SCHED_SWITCHES`, `SCHED_TICKS`, …) — **một** bản, **không** per-hart, **không** khoá.

Hệ quả: `schedule()` hiện **chỉ đúng khi chỉ một hart gọi**. Hai hart cùng gọi sẽ cùng đọc/ghi
`SCHED_SWITCHES` và `SCHED_RSP` ⇒ hai hart `iretq` vào **cùng** một frame trên **cùng** một stack
— đúng lớp lỗi boot 410/600 của KT-293 mà KT-298 đã sửa ở tầng **bàn giao `rsp`**, nhưng **chưa**
sửa ở tầng **chọn tác vụ**.

### (2) `TASK_SEQUENCE` là bảng cố định — hart phụ đọc nó thế nào? — **Không đọc được**

`exc_dispatch` (`main.rs`) so `LAPIC_ID` với `smp::bsp_lapic_id()`: nhánh BSP mới gọi
`sched::on_tick`; nhánh hart phụ chỉ `smp::note_ap_tick()` + `lapic_eoi()`. Vì vậy hart phụ
**không** vào scheduler, và nếu cho vào **nguyên trạng** thì `SCHED_SWITCHES` (toàn cục) bị hai
hart cùng tăng ⇒ hai hart chọn **cùng** bước của bảng ⇒ chồng ngữ cảnh.

### (3) `TSS.rsp0` per-hart đã có (KT-278) — hart phụ `iretq` xuống ring 3 cần gì thêm?

`tss::init(hart, stack_top, ist1)` đã nạp `RSP0` = đỉnh slot stack **của hart ấy**, và
`gdt::install(hart, tss_base)` nạp **GDT riêng** có đủ `USER_CS`/`USER_SS` (7 entry). Nhưng:

1. `RSP0` của hart phụ đang trỏ **stack `hlt` của chính nó**, **không** phải stack kernel của tác
   vụ ring 3 (`TASK_U_STACK`). Nếu hart phụ `iretq` xuống CPL3 mà không nạp lại `RSP0`, tick timer
   từ CPL3 sẽ đẩy frame vào stack `hlt` ấy ⇒ ghi đè ngữ cảnh hart phụ (đúng lớp lỗi `no-rsp0` của
   KT-304, nhưng ở hart khác).
2. Hart phụ **không** có ngữ cảnh "main" để quay về: nó đang trong `kmain_secondary`'s
   `loop { …; hlt(); }`. Vì vậy pha nhiều hart **không** cần trả hart phụ về `hlt` — nó chỉ cần
   **một** lần chuyển vào tác vụ là đủ chứng minh `schedule()` chạy trên hart ấy.
3. `frame_ok` (KT-298) chỉ nhận bốn slot `MAIN/A/B/U`; frame của hart phụ **không** nằm trong
   stack của slot nào ⇒ phải **không** đưa frame ấy qua `frame_ok` (giữ nguyên lý: frame do CPU
   đẩy tại chỗ ngắt là hợp lệ theo định nghĩa, như `TASK_MAIN`).

## 2. Phương án: **(A) hàng đợi toàn cục + khoá** — chọn, và vì sao

| Phương án | Đánh giá |
|---|---|
| **(A) runqueue toàn cục + khoá** | **Chọn.** Nhỏ nhất, đúng khuyến nghị của task, và **tự nhiên** phát hiện double-pick: hàng đợi nói "rảnh" mà bảng ownership nói "có chủ" ⇒ dừng có chẩn đoán |
| (B) per-hart runqueue + chuyển tác vụ qua IPI | Không chọn: cần thêm một cơ chế **migration** (IPI mang "hãy nhận tác vụ"), tức thêm trạng thái và thêm đường đua — ngoài phạm vi 3 h và ngoài yêu cầu ("**không** migration") |

**Phạm vi đã chọn (hẹp có chủ đích):**

- Pha nhiều hart chạy **sau** chuỗi cố định ⇒ TAP 22/23/24 của KT-286/KT-304 **không** đổi nghĩa,
  không nới ngưỡng nào.
- **Hai** hart tham gia: BSP (hart 0) + hart 1. Hàng đợi là **ba** tác vụ **đã có** (`A`, `B`, `U`)
  ⇒ có **chuyển** tác vụ thật (mỗi hart luôn còn tác vụ khác để chọn), không thêm tác vụ mới.
- **Trần cứng** `MH_HARD_ROUNDS = 48` ⇒ boot **không** treo khi một hart im lặng (khuôn KT-301).

## 3. Cơ chế

| Thành phần | Thay đổi |
|---|---|
| `vifast-plat-x86_64::sched` | `MH_HARTS=2`, `MH_TASKS=[A,B,U]`, `MH_QUEUE_SLOTS=8`, `MH_MAX_ROUNDS=6`, `MH_HARD_ROUNDS=48`, `MH_FREE=u64::MAX`; hàm thuần `task_name`, `mh_all_picked`, `mh_end`, `mh_ok` |
| `vifast-plat-x86_64/tests/runqueue.rs` | **mới** — 6 test host: hằng, tên tác vụ, mô hình FIFO **có khoá** (không hart nào chia tác vụ), mô hình **không khoá** (double-pick **phải** bị phát hiện), `mh_end` (chờ hart kia + trần cứng), `mh_ok` (bốn điều kiện) |
| `vifast-kbin-x86_64::sched` | hàng đợi vòng `MH_QUEUE`/`MH_HEAD`/`MH_TAIL`, bảng `MH_OWNER`, khoá spin `MH_LOCK`, `MH_CURRENT`/`MH_PICKS`/`MH_FIRST`/`MH_FIRST_N`/`MH_ROUNDS`/`MH_DOUBLE`; `mh_lock`/`mh_unlock`/`mh_push`/`mh_pop`/`mh_tick`/`mh_start`/`on_tick_ap`; nhánh `MH_ACTIVE` trong `on_tick`; TAP 25 |
| `vifast-kbin-x86_64::smp` | `hart_of_lapic(id)` — tra `SMP_HART_LAPIC_ID` (dữ liệu **do chính hart phụ** công bố) |
| `vifast-kbin-x86_64::main` | nhánh hart phụ của `exc_dispatch` gọi `sched::on_tick_ap(frame_rsp)`; `TAP_TESTS` 26 → **27** |
| `vifast-kbin-x86_64::net` | blk dời `tap_first + 10` → `+ 11` (TAP 26/27) |
| `Cargo.toml` | feature đột biến `bsp-only-sched`, `no-sched-lock` |

**Bất biến của pha** (mỗi cái là một lớp lỗi thật):

1. **Một chủ** — `MH_OWNER[task]` + `compare_exchange`; hàng đợi chỉ chứa tác vụ **rảnh**.
2. **Không double-pick** — `mh_pop` trả `Err(task)` ⇒ `# sched: BAD double-pick` + halt.
3. **Chờ hart kia** — `mh_end` đòi **mọi** hart đã chọn ≥ 1 trước khi kết thúc (trừ trần cứng).
4. **Trần cứng** — `MH_HARD_ROUNDS` ⇒ kết thúc dù thiếu bằng chứng (TAP đỏ, **không** treo).

**Vì sao dòng của hart 0 in ở tổng kết, không in trong handler:** hart phụ in dòng của nó **trước**
khi tăng `MH_PICKS[1]` ("in trước, đếm sau" — khuôn KT-259/KT-278), và BSP chỉ tới phần tổng kết
**sau** khi pha kết thúc ⇒ **không** có hai dòng xen ký tự trên COM1. Nếu cả hai hart cùng in
trong handler thì hai dòng có thể xen nhau và regex của runner sẽ đỏ **oan**.

## 4. Đo được (log QEMU nguyên văn, `-smp 4`)

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: hart=1 picked=a n=1
# sched: ticks=10 tasks=3
# sched: runs a=1245744 b=1471487 switches=9 ab=1 ba=2 a_rsp=0x12e178 b_rsp=0x132178 u_rsp=0x136180
# sched: umode task ran n=24270228 ring=3 preempted=2 resumed=1
# sched: umode counts first=12915741 last=24232588 cs=0x1b kernel_stack=0x136230
# sched: umode progress first=12915741 last=24232588 delta=11316847
# sched: hart=0 picked=b n=2
# sched: multi-hart harts=2 rounds=7 picks=[3, 4] first=[b, a] double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
ok 26 - blk: device
ok 27 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| hart phụ `schedule()` | `# sched: hart=1 picked=a n=1` — hart 1 chọn **A** ở vòng 1 |
| BSP `schedule()` trên **cùng** hàng đợi | `# sched: hart=0 picked=b n=2` — BSP chọn **B** ở vòng 2 |
| hai hart chọn **khác** tác vụ | `first=[b, a]` |
| chuyển tác vụ thật trong pha | `rounds=7`, `picks=[3, 4]` — mỗi hart chạm **cả ba** tác vụ (kể cả `U` ring 3 trên hart 1) |
| không double-pick | `double=0` |
| TAP | **27/27** xanh |

## 5. Đột biến (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **hart phụ không schedule** | `--features bsp-only-sched` | `# sched: hart=0 picked=a n=1` + `# sched: multi-hart harts=2 rounds=48 picks=[48, 0] first=[a, ?] double=0` ⇒ `not ok 25 - sched: multi-hart` + `# TAP 25 FAIL — dừng (không in marker)` (ĐỎ, **không** treo: `rounds=48` = trần cứng) |
| (ii) **bỏ khoá** | `--features no-sched-lock` (khoá no-op + cửa sổ 30 ms bằng PIT kênh 2) | `# sched: hart=1 picked=a n=1` + `# sched: BAD double-pick hart=0 task=a n=1` ⇒ halt, **không marker** (ĐỎ) |

## 6. Ca âm của runner

Giữ **19** ca cũ (a)..(s), thêm **một** ca:

- **(t) chỉ BSP schedule** — log tốt bị xoá dòng `# sched: hart=1 picked=…` **và** sửa dòng tổng
  hợp thành `picks=[6, 0] first=[a, ?]` (đúng log của ảnh `--features bsp-only-sched`) ⇒ **ĐỎ**.

Tổng **20** ca âm, tất cả ĐỎ.

## 7. Còn thiếu / rủi ro

1. **Hai** hart (BSP + hart 1), **không** phải bốn. Hart 2/3 vẫn chỉ `hlt` — đúng phạm vi task
   ("ưu tiên hart phụ gọi `schedule()` và chọn được tác vụ khác BSP").
2. **Không** load balancing, **không** priority, **không** migration chủ động, **không** sleep/wake.
   Hàng đợi là FIFO **cố định** ba tác vụ, không phải scheduler sản phẩm.
3. Hart phụ **không** quay về `hlt` sau pha: nó ở lại trong tác vụ cuối (tick sau chỉ `resume`).
   Không ảnh hưởng boot (sau TAP 21 hart phụ không còn việc gì), nhưng là **nợ** nếu sau này cần
   hart phụ làm việc khác — khi đó phải thêm slot "ngữ cảnh riêng của hart phụ" và cho `frame_ok`
   nhận slot ấy.
4. Pha nhiều hart chạy **sau** chuỗi cố định, nên nó **không** chứng minh "hai hart cùng chạy tác
   vụ trong lúc tác vụ ring 3 đang chạy" — chỉ chứng minh "hai hart cùng `schedule()` trên một
   hàng đợi". Đó là mốc thật của task, không hơn.
5. **Không** tuyên bố K5' đạt: K5' còn "như K3" (TAP đầy đủ smp 1|2|4 + soak 600 boot) và các mục
   ngoài soak (§12.12 của `v2/docs/k5-x86_64-boot.md`).

## 8. Bằng chứng cổng (commit `68a32a78`, base `81b8e210`)

| Lệnh | Kết quả |
|---|---|
| `cargo test -p vifast-plat-x86_64` (host) | **PASS** — 14 suite, gồm `tests/runqueue.rs` **6/6** và `tests/sched.rs` **27/27** (không hồi quy) |
| `./v2/bin/v2 check` | **PASS** (exit 0) — fmt + clippy + luật R1..R5 (R1: `main.rs` 1499 dòng, `sched.rs` 863) |
| `bash v2/test/boot/run-x86_64.sh` | **PASS** — `19/19 kiểm sạch (boot=1, tests=27, exc=5)` + `20/20 ca âm ĐỎ` |
| `bash v2/test/boot/soak-x86_64.sh --smoke 60 3` | **PASS** — `60/60 lỗi=0`, `rc_124=0`, `qemu_foreign=0`, `verdict=OK` |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **PASS** — `37/37 ca đúng như phải (1 dương + 36 âm)` |

Log QEMU nguyên văn của lượt runner (`boot-1.log`):

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: hart=1 picked=b n=2
# sched: ticks=10 tasks=3
# sched: runs a=582705 b=415115 switches=9 ab=1 ba=2 a_rsp=0x12e178 b_rsp=0x132178 u_rsp=0x136180
# sched: umode task ran n=5365799 ring=3 preempted=2 resumed=1
# sched: umode counts first=1622959 last=4694503 cs=0x1b kernel_stack=0x136230
# sched: umode progress first=1622959 last=4694503 delta=3071544
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=2 rounds=7 picks=[3, 4] first=[a, b] double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
ok 26 - blk: device
ok 27 - blk: sector0
```

**Ghi chú về `soak --smoke 60 3`**: đây là **smoke**, **không** phải bằng chứng cổng K5' (K5' đòi
soak **600**, D2-149). Script tự in `# soak: SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5'`.
Chạy soak **600** là việc của vòng sau (cần máy sạch + ngân sách riêng), **không** phải của task
3 h này.

**Ghi chú về `total` của runner**: `selftest_negative` đánh số ca âm **tiếp** dãy (`total + 1`),
nhưng dòng tổng kết cộng `total + 17` cho **20** ca ⇒ dòng `# 19/19 kiểm sạch` **đếm thiếu 3** so
với số ca âm thực tế (lệch này **có từ trước**: 19 ca nhưng cộng `+16`). Không sửa trong task này
để không đổi một con số đã ghi trong các journal trước; `fails` (nguồn quyết định exit code) vẫn
đúng.
