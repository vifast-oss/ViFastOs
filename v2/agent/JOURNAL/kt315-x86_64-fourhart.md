# KT-315 — **bốn** hart schedule trên x86_64: tác vụ thứ tư + cổng khởi động/giữ

Trạng thái: **DONE** — nhánh `task/kt315-x64fourhart`, base `main` @ `a9f90972`, 2026-11-22.
Sub-agent `x64fourhart`. Quyết định: **D2-221** (nhiệm vụ KT-315), **D2-212** (KT-286 preemption),
**D2-216** (KT-297/298), **D2-198** (uỷ quyền commit nhánh task), **D2-05** (test đỏ trước),
**D2-07** (không retry), **D2-17** (`unsafe` chỉ trong danh sách đóng).

**Không** tuyên bố K5' đạt.

## 1. Khảo sát trước (ba câu hỏi của task)

### (1) KT-310 dùng `MH_OWNER` + `MH_HARD_ROUNDS = 48` — mở lên 4 hart cần gì?

**Cần thêm một tác vụ, và cần hai cổng đồng bộ.** Ba phát hiện, mỗi cái đo được:

**(1a) Ba tác vụ + bốn hart ⇒ hart thứ tư đói vĩnh viễn.** Hàng đợi là FIFO **toàn cục**; mỗi
hart trả tác vụ cũ rồi lấy tác vụ mới **trong cùng vùng tới hạn**. Với `N` tác vụ và `N` hart,
mỗi hart luôn lấy lại **chính nó** (hàng đợi chỉ có một phần tử khi nó nhìn vào). Với **ba** tác
vụ và **bốn** hart: ba hart đầu giữ ba tác vụ, hàng đợi rỗng; hart thứ tư `mh_pop` trả
`Ok(None)` **mãi mãi** ⇒ `picks[3] = 0` ⇒ TAP 25 đỏ. Test host
`four_harts_need_four_tasks` mô hình hoá đúng điều kiện này (ba tác vụ ⇒ `picks[3] = 0`; bốn tác
vụ ⇒ cả bốn hart chọn được). **Kết luận: thêm tác vụ `C`** (`TASK_C = 4`), không nới ngưỡng, không
hạ phạm vi.

**(1b) Nhịp BSP chậm hơn hart phụ 10× ⇒ cần cổng khởi động.** PIT của BSP là 100 Hz (10 ms);
LAPIC timer của hart phụ ~1 ms (`AP_TIMER_INITIAL_COUNT = 100_000`, `AP_TIMER_DIVIDE = 1`). Không
có cổng thì ba hart phụ hút sạch hàng đợi (và trả/lấy lại) trước tick đầu của BSP ⇒ BSP có thể
**không** chọn được tác vụ nào trong vài chục vòng đầu, và `first[0]` phụ thuộc **thời điểm**
host lập lịch. Cổng: hart phụ chờ `MH_PICKS[0] != 0`.

**(1c) Cổng khởi động **không đủ** — cần cổng **giữ** của BSP.** Đo được ngay lượt runner đầu
(boot 4/5): hart 1 bị host deschedule, BSP đã **trả** `a` ở tick 2 ⇒ hart 1 chọn `a` ⇒
`first=[a, a, b, u]`. Đó là hành vi **hợp lệ** của hàng đợi nhưng làm phép kiểm "hart phụ chọn
khác BSP" đỏ **oan**. Vì vậy BSP **giữ** tác vụ đầu của nó cho tới khi **mọi** hart đã chọn lần
đầu, tối đa `MH_HOLD_TICKS = 4` tick của chính nó (≈ 40 ms). Hết hạn thì trả và chọn lại mỗi tick
⇒ `MH_ROUNDS` tăng đều ⇒ trần cứng chắc chắn chạm ⇒ **không treo** kể cả khi không hart phụ nào
chọn.

### (2) `TSS.rsp0` per-hart (KT-278) đã đủ cho 4 hart `iretq` xuống ring 3?

**Đủ, không phải sửa gì.** `tss::init(hart, …)` nạp `RSP0` = đỉnh slot stack **của hart ấy**;
`gdt::install(hart, …)` nạp GDT riêng có `USER_CS`/`USER_SS`. KT-310 đã thêm bước **nạp lại**
`RSP0` khi hart ấy chọn `TASK_U` (`tss::set_rsp0(hart, MH_U_KERNEL_TOP)`), và bước ấy **đã** chạy
cho **mọi** hart (`hart` là tham số, không phải hằng 1). Đo được: hart 2 chọn `u` ở boot của lượt
runner (`# sched: hart=2 picked=u n=3`) và boot **không** có `# exc:` nào ⇒ `iretq` xuống CPL3
trên hart 2 chạy đúng, tick từ CPL3 đẩy frame vào `TASK_U_STACK` (không phải stack `hlt`).

### (3) `frame_ok` chỉ nhận 4 slot `MAIN/A/B/U` — 4 hart cùng chọn có tranh slot không?

**Không tranh slot, nhưng thiếu slot cho tác vụ thứ tư.** `frame_ok(rsp, slot)` kiểm **vị trí**
frame trong stack **của chính slot ấy** (`slot_stack`) + **nội dung** frame
(`psched::switch_frame_ok`). Frame được lưu **theo tác vụ** (`SCHED_RSP[slot]`), **không** theo
hart — nên hai hart chạy **cùng** một tác vụ ở hai thời điểm là hợp lệ (ownership bảo đảm không
đồng thời). Vì vậy **không** có tranh slot giữa các hart. Cái thiếu là **slot thứ năm** cho tác
vụ `C`: `frame_ok` từ chối slot lạ, `slot_stack` trả `None` ⇒ phải thêm `TASK_C` vào **cả hai**
(đã làm).

## 2. Phương án: giữ **(A) hàng đợi toàn cục + khoá**, thêm tác vụ + hai cổng

| Phương án | Đánh giá |
|---|---|
| **(A) hàng đợi toàn cục + khoá, thêm `TASK_C`, thêm cổng khởi động + cổng giữ** | **Chọn.** Giữ nguyên cơ chế KT-310 (đã có test host + hai đột biến), chỉ sửa đúng ba chỗ **số học** đã đo được ở §1 |
| (B) per-hart runqueue + IPI chuyển tác vụ | Không chọn: cần **migration** — ngoài phạm vi ("**không** migration"), và thêm trạng thái/đường đua |
| (C) hạ phạm vi xuống **3 hart** | Không chọn: 3 hart + 3 tác vụ **chạy được** nhưng bỏ mất tiêu chí K5' ("như K3" = **bốn** hart). Chỉ dùng khi 4 hart **không** thể — ở đây thể, với **một** tác vụ thêm |

**Phạm vi giữ hẹp có chủ đích:** pha nhiều hart vẫn chạy **sau** chuỗi cố định ⇒ TAP 22/23/24 của
KT-286/KT-304 **không** đổi nghĩa; `TASK_SEQUENCE` **không** đổi; **không** load balancing,
**không** priority, **không** migration chủ động, **không** sleep/wake.

## 3. Cơ chế

| Thành phần | Thay đổi |
|---|---|
| `vifast-plat-x86_64::sched` | `TASK_COUNT` 3 → **4**, `TASK_C = 4`, `TASK_SLOTS` 4 → **5**; `MH_HARTS` 2 → **4**; `MH_TASKS` 3 → **4** (`[A, B, U, C]`); `MH_QUEUE_SLOTS` 8 → **16** (> 4 + 4); `MH_MAX_ROUNDS` 6 → **8** (2 lượt/hart); `MH_HARD_ROUNDS` **giữ 48**; `MH_HOLD_TICKS = 4`; hàm thuần mới `mh_harts_picked`, `mh_hold`; `mh_ok` đổi điều kiện 4 thành `first[0] != first[i]` với **mọi** `i ≥ 1`; `task_name` thêm `"c"` |
| `vifast-plat-x86_64/tests/runqueue.rs` | 9 test host: hằng, tên tác vụ, **bốn hart cần bốn tác vụ** (mới), FIFO có khoá (4 hart), FIFO không khoá (double-pick), `mh_end`, `mh_ok` (4 hart), `mh_harts_picked` (mới), `mh_hold` (mới) |
| `vifast-plat-x86_64/tests/sched.rs` | `TASK_COUNT = 4`, `TASK_SLOTS = 5`, `TASK_C = 4`, `next_task(TASK_C, …) = TASK_A` |
| `vifast-kbin-x86_64::sched` | `TASK_C_STACK` (16 KiB) + `TASK_C_RUNS` + `task_c`; `slot_stack`/`frame_ok` nhận `TASK_C`; `MH_BSP_TICKS`; `mh_picks_now`/`mh_finish`; cổng khởi động + cổng giữ trong `mh_tick`; dòng tổng hợp `harts=<số đo> … picks=[4] first=[4]`; `tasks=4`; `runs … c=… c_rsp=…` |
| `vifast-kbin-x86_64/Cargo.toml` | feature đột biến mới `two-hart-sched` |
| `v2/test/boot/run-x86_64.sh` | mẫu 4 hart (4 dòng `picked=`, `harts=4`, 4 `picks`, `first[0] != first[i]`), `tasks=4`, ca âm (t) cập nhật, **ca âm (u) mới** "chỉ 2 hart" ⇒ **21** ca âm |
| `v2/test/boot/soak-x86_64.sh` | cùng mẫu 4 hart + `tasks=4`; log mẫu cập nhật; **+2** ca âm (chỉ 3 hart, chỉ 2 hart) ⇒ **38** ca selftest |

**Bốn bất biến của pha** (giữ nguyên từ KT-310, mạnh hơn ở điều 3):

1. **Một chủ** — `MH_OWNER[task]` + `compare_exchange`; hàng đợi chỉ chứa tác vụ **rảnh**.
2. **Không double-pick** — `mh_pop` trả `Err(task)` ⇒ `# sched: BAD double-pick` + halt.
3. **Chờ **mọi** hart** — `mh_end` đòi **cả bốn** hart đã chọn ≥ 1 trước khi kết thúc (trừ trần
   cứng). KT-310 đòi **hai**; KT-315 đòi **bốn**.
4. **Trần cứng** — `MH_HARD_ROUNDS = 48` ⇒ kết thúc dù thiếu bằng chứng (TAP đỏ, **không** treo).

**Vì sao `harts=` là số **đo**, không phải hằng `MH_HARTS`:** đột biến "chỉ 2 hart" phải in
`harts=2` để runner **đỏ**. Nếu in hằng thì đột biến ấy không thể bị bắt — phép kiểm sẽ là dấu
cao su. `mh_harts_picked` đếm số hart có `picks ≥ 1`.

**Vì sao `first[0] != first[i]` (không phải "bốn giá trị đôi một khác"):** đo được
`first=[a, u, b, b]` (boot 3/8 của lượt kiểm) — hai hart phụ **cùng** chọn `b` ở lần đầu là **hợp
lệ** (hart 1 trả `b` rồi lấy `u`, hart 3 lấy `b`). Đòi đôi một khác là **giả định sai** (đúng lớp
lỗi KT-308). Bất biến thật: **BSP** khác **mọi** hart phụ ở lần đầu — và hai cổng ở §1 làm nó
**tất định**.

## 4. Đo được (log QEMU nguyên văn, `-smp 4`)

Lượt runner (`boot-1.log`):

```text
# sched: switch a→b n=1
# sched: switch b→a n=2
# sched: hart=3 picked=b n=2
# sched: hart=2 picked=u n=3
# sched: hart=1 picked=c n=4
# sched: ticks=10 tasks=4
# sched: runs a=235434 b=201113 c=19372 switches=9 ab=1 ba=2 a_rsp=0x12e198 b_rsp=0x132198 u_rsp=0x13a1a0 c_rsp=0x136198
# sched: umode task ran n=8444832 ring=3 preempted=2 resumed=1
# sched: umode counts first=2844727 last=6984179 cs=0x1b kernel_stack=0x13a250
# sched: umode progress first=2844727 last=6984179 delta=4139452
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=8 picks=[2, 2, 2, 2] first=[a, c, u, b] double=0
ok 22 - sched: two tasks ran
ok 23 - sched: preempted
ok 24 - sched: umode task
ok 25 - sched: multi-hart
ok 26 - blk: device
ok 27 - blk: sector0
```

| Phép đo | Kết quả |
|---|---|
| **bốn** hart `schedule()` | bốn dòng `# sched: hart=0|1|2|3 picked=…` — mỗi hart chọn **một** tác vụ ở lần đầu |
| BSP chọn **khác** mọi hart phụ | `first=[a, c, u, b]` — `a` chỉ thuộc BSP |
| mọi hart chọn ≥ 1 | `picks=[2, 2, 2, 2]`, `harts=4` |
| tác vụ thứ tư **chạy thật** | `c=19372` vòng (bộ đếm do chính tác vụ C tăng) |
| không double-pick | `double=0` |
| TAP | **27/27** xanh; runner 20/20 kiểm sạch + **21 ca âm** ĐỎ |

Lượt runner **thứ hai** trên **cây đã commit** (`842bf356`, cùng ảnh — chỉ khác chú thích):

```text
# sched: hart=1 picked=b n=2
# sched: hart=2 picked=u n=3
# sched: hart=3 picked=u n=6
# sched: ticks=10 tasks=4
# sched: runs a=393541 b=401395 c=814 switches=9 ab=1 ba=2 a_rsp=0x12e198 b_rsp=0x132198 u_rsp=0x13a1a0 c_rsp=0x136198
# sched: hart=0 picked=a n=1
# sched: multi-hart harts=4 rounds=8 picks=[2, 2, 2, 2] first=[a, b, u, u] double=0
```

Hai hart phụ **cùng** chọn `u` ở lần đầu — **hợp lệ** (hart 2 trả `u` rồi lấy tác vụ khác, hart 3
lấy `u`); `first[0] = a` vẫn **khác** mọi hart phụ. Đây đúng là lý do bất biến là
`first[0] != first[i]` chứ **không** phải "bốn giá trị đôi một khác".

Tám boot liên tiếp (cùng ảnh, `-smp 4`) — `first` **không** tất định giữa các hart phụ (đúng như
thiết kế), nhưng `first[0] = a` và `first[i] != a` **luôn** đúng:

```text
# sched: multi-hart harts=4 rounds=8  picks=[3, 1, 2, 2] first=[a, c, b, u] double=0
# sched: multi-hart harts=4 rounds=13 picks=[3, 4, 3, 3] first=[a, b, c, u] double=0
# sched: multi-hart harts=4 rounds=8  picks=[2, 3, 1, 2] first=[a, u, b, b] double=0
# sched: multi-hart harts=4 rounds=10 picks=[4, 2, 2, 2] first=[a, c, b, u] double=0
# sched: multi-hart harts=4 rounds=11 picks=[3, 3, 3, 2] first=[a, u, b, c] double=0
# sched: multi-hart harts=4 rounds=9  picks=[3, 2, 2, 2] first=[a, c, u, b] double=0
# sched: multi-hart harts=4 rounds=11 picks=[3, 2, 3, 3] first=[a, u, b, c] double=0
# sched: multi-hart harts=4 rounds=8  picks=[2, 2, 2, 2] first=[a, b, c, u] double=0
```

## 5. Đột biến (đều ĐỎ)

| Đột biến | Cách làm | Kết quả |
|---|---|---|
| (i) **chỉ 2 hart** | `--features two-hart-sched` (hart 2/3 vẫn `hlt`) | `# sched: hart=1 picked=b n=2` + `# sched: hart=0 picked=a n=1` + `# sched: multi-hart harts=2 rounds=48 picks=[21, 27, 0, 0] first=[a, b, ?, ?] double=0` ⇒ `not ok 25 - sched: multi-hart` + `# TAP 25 FAIL — dừng (không in marker)` (ĐỎ, **không** treo: `rounds=48` = trần cứng) |
| (ii) **bỏ khoá** | `--features no-sched-lock` (khoá no-op + cửa sổ 30 ms bằng PIT kênh 2) | `# sched: BAD double-pick hart=2 task=b n=…` + `# sched: BAD double-pick hart=3 task=c n=4` + `# sched: multi-hart harts=2 rounds=48 picks=[46, 0, 0, 2] first=[a, ?, ?, b] double=3` ⇒ `not ok 25` + không marker (ĐỎ) |

Đột biến (i) là đột biến **trực tiếp** của tiêu chí "cả **bốn** hart vào pha": nó in `harts=2`
(số **đo**) và runner đỏ. Đột biến (ii) chứng minh khoá + ownership vẫn là **bắt buộc** khi có
bốn hart (ba hart phụ cùng vào vùng tới hạn ⇒ ba double-pick).

## 6. Ca âm của runner

Giữ **20** ca cũ (a)..(t) — ca (t) cập nhật sang mẫu 4 hart — thêm **một** ca:

- **(u) chỉ 2 hart schedule** — log tốt bị xoá hai dòng `# sched: hart=2|3 picked=…` **và** sửa
  dòng tổng hợp thành `harts=2 rounds=6 picks=[6, 6, 0, 0] first=[a, b, ?, ?]` (đúng log của ảnh
  `--features two-hart-sched`) ⇒ **ĐỎ**.

Tổng **21** ca âm, tất cả ĐỎ. Soak `--selftest` thêm **hai** ca (chỉ 3 hart, chỉ 2 hart) ⇒
**38/38** (1 dương + 37 âm).

## 7. Bằng chứng cổng

| Lệnh | Kết quả |
|---|---|
| `cargo test -p vifast-plat-x86_64` (host) | **PASS** — 14 suite, gồm `tests/runqueue.rs` **9/9** và `tests/sched.rs` **27/27** |
| `./v2/bin/v2 check` | **PASS** (exit 0) — fmt + clippy + luật R1..R5 |
| `bash v2/test/boot/run-x86_64.sh` | **PASS** — `20/20 kiểm sạch (boot=1, tests=27, exc=5)` + `21/21 ca âm ĐỎ` |
| `bash v2/test/boot/soak-x86_64.sh --smoke 60 3` | **60/60 lỗi=0**, `rc_124=0`, `qemu_own=0` — `--smoke` **luôn** exit 1 (không phải bằng chứng cổng); `qemu_foreign=1..4` ⇒ **T-INFRA** (ba QEMU của task khác trên máy) — xem §8 |
| `bash v2/test/boot/soak-x86_64.sh --selftest` | **PASS** — `38/38 ca đúng như phải (1 dương + 37 âm)` |

## 8. Soak smoke

```text
# soak: bắt đầu N=60 lanes=3 17:36:39Z
# guard: boot_end=61 rc_124=0 qemu_own=0
# soak: xong wall=100s (0 boot/s); ok=60 fail=0 tổng=60/60
# soak: BUILD_ID=599c74330476615a sha256=1b64d80c95e6ca7f2db4322d5359d6c37701d72decc454011892106584fd5a7e git=a9f9097294247f0c7ec9b8961ee9aec084de2457
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5'
```

**60/60 boot lỗi=0**, `rc_124=0`, `qemu_own=0`, `fail=0`. `--smoke` **luôn** exit 1 (dòng 933 của
script: "SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5'"), nên exit code **không** phải phán
quyết của lượt này.

**T-INFRA (ghi trung thực):** `qemu_foreign=1` (before) / `4` (after) ⇒ `verdict=KHONG_HOP_LE` +
`MEASUREMENT_INVALID tag=before|after`. Máy đang chạy **ba** QEMU của **task khác**
(`/private/tmp/kt316-s1`, `/private/tmp/kt313-s1` — ba sub-agent song song trên cùng máy), nên
**số thời gian** của lượt này không dùng để kết luận (D2-116/OD-K6-6). Số **đúng/sai** (60/60) là
phép kiểm nội dung từng log, không phải số thời gian ⇒ vẫn dùng được. **Không** chạy lại để lấy
`qemu_foreign=0` (D2-07: không retry để lấy xanh; máy còn ba QEMU của task khác nên chạy lại cũng
không sạch hơn).

## 9. Còn thiếu / rủi ro

1. **Bốn** hart schedule **trong pha nhiều hart**, **không** phải "bốn hart cùng chạy tác vụ
   trong lúc tác vụ ring 3 đang chạy": pha vẫn chạy **sau** chuỗi cố định (TAP 22/23/24 không đổi
   nghĩa). Đó là mốc thật của task, không hơn.
2. **Không** load balancing, **không** priority, **không** migration chủ động, **không**
   sleep/wake. Hàng đợi là FIFO **cố định** bốn tác vụ, **không** phải scheduler sản phẩm.
3. Hart phụ **không** quay về `hlt` sau pha (nợ KT-310, giữ nguyên): nó ở lại trong tác vụ cuối.
   Với bốn hart, **ba** hart phụ ở lại trong tác vụ — không ảnh hưởng boot (sau TAP 21 không còn
   việc gì), nhưng là nợ nếu sau này cần hart phụ làm việc khác.
4. **Cổng giữ** dựa trên **tick của BSP** (PIT 100 Hz). Nếu host deschedule hart phụ > 40 ms thì
   BSP trả tác vụ trước khi hart ấy chọn ⇒ `first[0] == first[i]` **có thể** xảy ra ⇒ TAP 25 đỏ
   (fail-closed, **không** xanh oan). Đây là **biên** đã biết, không phải lỗi logic; soak 60 boot
   chưa gặp.
5. **Không** tuyên bố K5' đạt: K5' còn "như K3" (TAP đầy đủ smp 1|2|4 + soak **600** boot, D2-149)
   và các mục ngoài soak (§12.12 của `v2/docs/k5-x86_64-boot.md`). `--smoke 60` **không** phải
   bằng chứng cổng K5'.
