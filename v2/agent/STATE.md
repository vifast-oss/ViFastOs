# STATE — ViFastOs v2 (vòng 1)

Người ghi: **Agent 2** dòng OS. Cập nhật **sau mỗi bước** (để nén ngữ cảnh bất cứ lúc nào).
Khởi tạo: 2026-11-12 bởi người điều phối (vòng vá sau review). Cập nhật lần đầu: 2026-11-12 bởi Agent 2.

```
HEAD          : (không ghi hash ở đây để khỏi lệch sau commit — xem `git log -1`; lịch sử dưới ghi hash từng task)
BUILD_ID      : —
Task          : KT-21 (xong mã — **MCS donation/passive server/MCP**; chờ review độc lập); KT-18 DONE (5cccb72), KT-19 DONE (07c2e0b), KT-20 DONE (ae92d39)
Bước          : 8/8 (đỏ → mã → 13 test → invariant 1.000 seed → **review APPROVE mã + 3 điều kiện đã xử lý** → gate → Miri → tài liệu; chờ Agent 0 lật sổ)
Nhánh         : main
Cổng gần nhất : ./v2/bin/v2 gate --fast — PASS (**298 test/45 suite**, 0 failed; KT-20 là 284/44 ⇒ **+14 test** = 13 integration + 1 unit `mcs`; **L1 1.000 seed/999 trace** 550 ms; gen-check ON, `scn: OK`) — không retry
Việc dở       : commit `eae876b` **đã push** origin/main; **review độc lập** (lead spawn `reviewer-kt21`); **KT-22..KT-23 chờ**; không lật QUEUE (Agent 0)
```

## KT-21 — MCS donation: passive server, `Call`/`Reply`, MCP (K1b, D2-49(c), D2-50, D2-16)

**Vào**: READY (agentctl `next --project v2` trả KT-21) · HEAD `d547bbf` · KT-18/19/20 DONE · baseline
`gate --fast` PASS 284 test/44 suite ([`JOURNAL/kt21-baseline-gate-fast.log`](JOURNAL/kt21-baseline-gate-fast.log)).
**Phạm vi**: donation của **mô hình sim**; **không** IPC 4 cap/badge/`ReplyRecv` (K1c), hộp thư
(KT-22), SMP/push migration, Notification đầy đủ, refill sporadic/timeout fault (sau K6 — D2-16);
không `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`;
không `#[allow]`/`#[ignore]`. Working tree có thay đổi `viscript/` của tab khác — **không chạm**, chỉ
stage `v2/`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt21-red-compile.log`](JOURNAL/kt21-red-compile.log) — `tests/kt21_donation.rs` **không
  biên dịch**: **42 × `E0599`** cho API chưa có (`sc_of`, `sched_donated_to`, `sched_donor_of`,
  `donated_sc`, `effective_priority`) + **5 × `E0609`** (`ScInfo.donated_to/donor`,
  `TcbInfo.donated_sc`).
- [`JOURNAL/kt21-red-charge.log`](JOURNAL/kt21-red-charge.log) — tệp dò **tạm (không commit)**, dùng
  API **đang có**: sau `Call`, server passive `Ready` nhưng `tcb_has_budget == false`,
  `sched_pick() == None`, `dispatch == None` ⇒ `Call` **không bao giờ được phục vụ**; `2 tick` sau,
  `SC client consumed == 0` (kỳ vọng 2) ⇒ đúng 2 lỗi KT-21 (server không chạy + client không bị trừ).
  Tệp dò đã xoá sau khi lấy log.

### Sản phẩm

- `v2/sim/vifast-sim/src/mcs.rs`: `ScInfo` thêm `donated_to`/`donor` (model-only `[TK]`);
  `donated_priority(donor_priority, donor_mcp, donee_mcp) = min(...)` + **1 unit test** (kẹp MCP,
  quét 7×5×4 tổ hợp bất biến `p_eff ≤ cả ba`).
- `v2/sim/vifast-sim/src/kobj.rs`:
  - `TcbData.donated_sc: Option<ObjId>`, `SchedData.donated_to/donor: Option<ObjId>` — liên kết
    **hai chiều** (tra O(1), kiểm bất biến); `TcbInfo.donated_sc`.
  - Model API `[TK]`: `sc_of`, `donated_sc`, `sched_donated_to`, `sched_donor_of`,
    `effective_priority`.
  - **Donation tự động** đúng hai chỗ giao message của `Call`: `call` (server đang `Recv`) và
    `recv` (server lấy `Call` đang xếp hàng); `reply` trả SC; `donate_sc` chỉ nhận donor có SC
    **đã `Configure`**, SC **không** bị donate, **không** gắn cho TCB khác ("một SC một TCB" khi
    donate — KT-20 chưa cấm chia SC), donee **không** có SC riêng (D2-49(c)), donor không là donee
    (không chuỗi >1). **Không** từ chối `Call/Recv` vì thiếu SC (giữ KT-20 DoD 4).
  - `charge_tick`/`process_refills` dùng `funding_sc` (SC riêng **hoặc** SC được donate) ⇒ thời
    gian server chạy trừ đúng ngân sách client, hết budget ⇒ **server** `Sleeping{Budget}`, refill
    đánh thức server.
  - `sched_sync`/`check_sched` dùng **priority hiệu lực**; `check_sched` thêm bất biến donation
    (hai chiều khớp, một SC ↔ một donee, donee không SC riêng/không donor, donor
    `BlockedOnReply`, donor ≠ donee, không cùng `Running`, `p_eff ≤ MCP` cả hai bên).
  - Huỷ donation: `transition_tcb` (rời `BlockedOnReply` bằng đường khác `Reply`), `tcb_suspend`,
    `exit`, `finalise_tcb` → `release_donation_of` (donee ⇒ trả SC + đánh thức donor qua
    `cancel_reply_of`; donor ⇒ thu hồi SC, donee `Running` ⇒ `Sleeping{Budget}` ngay);
    `set_sched_params`/`tcb_set_priority` **từ chối** đổi SC/priority của donor đang donate
    (`IllegalOperation`/`Busy`); `purge_dead_refs` dọn liên kết khi một bên chết.
- `v2/sim/vifast-sim/tests/kt21_donation.rs` (**mới, 13 test**): server passive chỉ chạy khi được
  donate (trước `Call` `pick` không chọn dù `Ready`); charge vào SC client/`Sleeping{Budget}` +
  refill + `Reply` không mất; server có SC riêng giữ SC riêng; MCP 3 ca (kế thừa/kẹp 40/không nâng
  20); suspend/exit/**finalise** server; abort/finalise client; `SchedContext.Bind` không tạo
  donation; donation cần SC đã `Configure` + các ca `set_sched_params` bị từ chối; **SC dùng chung
  >1 TCB ⇒ không donate**; **không chuỗi 2 tầng qua guard `donated_sc`** (donee `Call` vào endpoint
  có server `Recv`); **`Tcb.SetPriority` từ chối donor, chấp nhận donee đổi MCP** (priority hiệu
  lực/bucket đổi theo); **Reply nhầm Reply object ⇒ không trả donation** (cảnh báo giới hạn K1c,
  [`JOURNAL/kt21-reply-owner.md`](JOURNAL/kt21-reply-owner.md)); **invariant 1.000 seed × 64 op**
  (stable; `#[cfg(miri)]` 3×8) — mỗi bước `check_sched`, không TCB `Running` nào thiếu SC cấp/thiếu
  budget, donation hai chiều, **14.609 bước có donation** / 64.000 bước (ngưỡng test `≥ 12.000`,
  log tái lập [`JOURNAL/kt21-invariant-donation.log`](JOURNAL/kt21-invariant-donation.log)), cùng
  seed cùng trace.

### Quyết định đã chốt (model-only `[TK]`, ghi rõ để review)

- **Donation tự động, không chuỗi >1** (D2-49(c); K1B-PLAN §5 KT-21 rủi ro (a)); server có SC
  riêng **giữ** SC riêng, không nhận donation.
- **Priority khi donate** `p_eff = min(client.priority, client.mcp, server.mcp)`. KT-18 buộc
  `priority ≤ mcp` cho **mọi** TCB (`kobj.rs:1397`) nên chặn thực tế là **MCP của server**: client
  mức cao hơn MCP server ⇒ **kẹp** xuống `server.mcp` (không chạy vượt MCP); client mức thấp hơn ⇒
  server chạy thấp hơn priority gốc (không được nâng). Priority **gốc** của server không đổi — chỉ
  bucket ready queue/`cur_prio` dùng `p_eff`.
- **Huỷ donation** khi server suspend/exit/finalise ⇒ SC về client + client được **đánh thức**
  (`Reply.caller = None`; mã `Cancelled` đầy đủ thuộc K1c); khi client suspend/exit/finalise ⇒ thu
  hồi SC, server mất ngân sách ⇒ `Sleeping{Budget}` (D2-50).
- **"Một SC một TCB" khi donate**: SC đang gắn cho >1 TCB (KT-20 chưa cấm chia SC) **không** được
  donate — nếu donate, TCB kia đang `Running` sẽ mất ngân sách giữa chừng. SC dùng chung vẫn chạy
  bình thường theo KT-20.
- **K1b không mô hình hoá** server phục vụ nhiều client cùng lúc / `ReplyRecv` / badge — K1c.
- **`Reply` trên Reply object không kiểm chủ sở hữu cap** (model thiếu thông tin): `reply` **chỉ**
  trả donation khi `caller` đúng là **donor**; Reply nhầm ⇒ giữ donation, không đánh thức nhầm
  (test `reply_nham_reply_object_khong_tra_donation`; chi tiết + follow-up K1c:
  [`JOURNAL/kt21-reply-owner.md`](JOURNAL/kt21-reply-owner.md)).
- **Đề xuất (chưa thành D2)**: giữ cách **kẹp** `p_eff = min(client.priority, client.mcp,
  server.mcp)` cho K1b, **model-only `[TK]`**; **K1c** chốt **strict reject** nếu cần. **D2-51
  chưa tồn tại** — đề nghị Agent 0 ghi khi lật sổ; KT-21 **không** tự tạo quyết định.

### Cổng + Miri (không retry — D2-07)

- [`JOURNAL/kt21-gate-fast.log`](JOURNAL/kt21-gate-fast.log) — `gate --fast` **PASS 298 test / 0
  failed** (**45 suite**; KT-20: 284/44 ⇒ **+14 test** = 13 integration + 1 unit `mcs`), **L1 1.000
  seed/999 trace** (550 ms), gen-check ON, `.scn` 21 tệp OK, **không retry**; `kt21_donation` native
  ~4,5 s.
- [`JOURNAL/kt21-miri.log`](JOURNAL/kt21-miri.log) — `v2 miri -p vifast-sim --test kt21_donation`
  **PASS 13/13** (0 failed, 0 ignored, **128,57 s**); 2 cảnh báo `integer-to-pointer cast` **có
  sẵn** từ objmem (`v2/lib/vifast-objmem/src/obj.rs`, D2-14/D2-17), không phát sinh từ KT-21;
  `v2/sim/**` **0 unsafe**. Invariant test dưới Miri giảm còn 3 seed × 8 bước (`#[cfg(miri)]`;
  stable giữ **1.000 × 64**) — không `#[ignore]`, không nới `gate --fast`/L1.
- [`JOURNAL/kt21-invariant-donation.log`](JOURNAL/kt21-invariant-donation.log) — số bước có
  donation in bằng `--nocapture`: **14.609 / 64.000** bước; test assert `≥ 12.000` (stable),
  Miri `> 0`.

### Phạm vi / không chạm

Chỉ `v2/sim/vifast-sim/src/{mcs.rs,kobj.rs}`, `v2/sim/vifast-sim/tests/kt21_donation.rs` (mới),
`v2/agent/{STATE.md,JOURNAL/}`. Không sửa `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS,
`viscript/`; không dependency/`unsafe`; không `#[allow]`/`#[ignore]`.

### Review độc lập + đề xuất

Review độc lập: **lead spawn** (`reviewer-kt21`) vì depth 1/1 — **APPROVE phần mã** (reviewer tự
chạy gate 294/45, L1 1000/999, `kt21_donation` 9/9) kèm **3 điều kiện đã xử lý** ở commit follow-up:
(1) thêm 3 test thật (SC dùng chung >1 TCB ⇒ không donate; guard chuỗi 2 tầng qua `dinfo.donated_sc`;
`tcb_set_priority` từ chối donor) — **thêm cả** test Reply nhầm Reply object; (2) sửa STATE
"D2-51 chưa tồn tại — đề xuất clamp, chờ Agent 0 ghi"; (3) `reply` chỉ trả donation khi
`caller == donor` + test cảnh báo + [`JOURNAL/kt21-reply-owner.md`](JOURNAL/kt21-reply-owner.md);
(4) in/assert số bước donation ra log. **Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (không tự
DONE, không lật QUEUE — D2-37). **MCP (đề xuất, chưa thành D2)**: cách hiểu **MCP khi donate** =
**kẹp** `p_eff = min(client.priority, client.mcp, server.mcp)` cho K1b (**model-only `[TK]`**; KT-18
buộc `priority ≤ mcp` nên chặn thực tế là MCP **server**); K1c chốt **strict reject** nếu cần.
**D2-51 chưa tồn tại** — đề nghị Agent 0 ghi khi lật sổ. **YC/Follow-up**: (1) D2-51 (Agent 0 ghi
khi lật sổ, không phải YC mới); (2) KT-22 hộp thư; (3) KT-23 thêm `kt21_donation` vào
`nightly --only-miri` (129 s).

## KT-20 — MCS budget: timer ảo `tick`, consumed/refill, `Sleeping{Budget}` (K1b, D2-49(b,e,h))

**Vào**: READY (agentctl `next --project v2` trả KT-20) · HEAD `54af1a8` · KT-18/19 DONE · baseline
`gate --fast` PASS 263 test/43 suite ([`JOURNAL/kt20-baseline-gate-fast.log`](JOURNAL/kt20-baseline-gate-fast.log)).
**Phạm vi**: ngân sách MCS của **mô hình sim**; **không** donation (KT-21), hộp thư/push migration
(KT-22/K1c), refill sporadic/timeout fault (**hoãn sau K6** — D2-16, D2-49(e)); không `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`; không
`#[allow]`/`#[ignore]`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt20-red-compile.log`](JOURNAL/kt20-red-compile.log) — `tests/kt20_budget.rs` **không biên dịch**:
  **58 × `E0599`** cho API chưa có (`tick`, `virtual_now`, `sched_info`, `sched_consumed`,
  `sched_remaining`, `tcb_has_budget`) + **`E0432`** (`ScInfo`) + `E0308`.
- [`JOURNAL/kt20-red-mutation-bo-charge.log`](JOURNAL/kt20-red-mutation-bo-charge.log) — đột biến tay
  **bỏ dòng `consumed += 1`** trong `ScBudget::charge` (không commit): test invariant **1.000 seed**
  đỏ đúng lý do (`SC 0 cửa sổ 0: 1+1 > budget 1`) ⇒ bộ kiểm bắt được "quên trừ budget" (DoD 10).

### Sản phẩm

- `v2/sim/vifast-sim/src/mcs.rs` (**mới**): `ScBudget { budget, period, consumed, refill, charged }`
  + `ScInfo`; `configure(now, budget, period)` đặt `refill` = **biên thẳng hàng toàn cục kế tiếp**
  (`next_boundary`, bão hoà `u64`, không wrap); `charge()`, `refill_due(now)`, `has_budget()`,
  `remaining()`, `check(now)`; **8 unit test** (biên toàn cục, overflow ở trần, configure, charge
  đúng biên, refill nhảy nhiều biên, trần `u64` không lặp, `check` bắt budget/period/consumed sai,
  **SC chưa cấu hình ⇒ không có budget**).
- `v2/sim/vifast-sim/src/sched.rs`: `HartSched::pick_where(ok)` — `pick` có điều kiện, vẫn mức cao
  nhất + FIFO, O(1) khi phần tử đầu hợp lệ; **+1 unit test**.
- `v2/sim/vifast-sim/src/kobj.rs`: `SchedData.budget: ScBudget` (thay `budget/period` thô);
  `Machine.now: u64`; `tick(n)` (charge TCB đang `Running` cho `[now, now+1)` → `now += 1` → xử lý
  biên refill: **charge trước, refill sau**), `charge_tick` (hết budget ⇒ `Running → Sleeping{Budget}`
  qua **một cửa** `transition_tcb`), `process_refills` (refill ⇒ `Sleeping{Budget} + !suspended →
  Ready` + enqueue), `enforce_budget` (gắn/gỡ SC khiến TCB `Running` không còn budget ⇒ ngủ ngay);
  `SchedControl.Configure` đặt `consumed = 0` + `refill` biên mới (giữ kiểm `budget > 0`,
  `period > 0`, `budget ≤ period`); model API `virtual_now/tick/sched_info/sched_consumed/
  sched_remaining/tcb_has_budget`; `sched_pick_on` lọc budget; `check_sched` thêm bất biến ngân
  sách (mọi SC sống: `budget`/`period` dương, `budget ≤ period`, `consumed ≤ budget`,
  `refill > now`; `Running` ⇒ còn budget).
- `v2/sim/vifast-sim/tests/kt20_budget.rs` (**mới, 12 test**): tick tất định cùng seed cùng mốc
  refill; Configure 0/oversize bị từ chối, không đổi trạng thái; charge **chỉ** khi `Running` qua
  đường IPC thật (Call/Recv/Reply); hết budget ⇒ `Sleeping{Budget}` đúng biên (không `budget + 1`),
  gỡ khỏi ready queue, timer biên kế; không charge quá budget; refill ⇒ `Ready`, `+suspended` giữ
  `Sleeping{Budget}`, `Resume` không phát lại; `pick`/`dispatch` bỏ qua TCB `Ready` hết budget rồi
  nhận lại sau refill; **không SC / SC chưa `Configure` ⇒ không có budget ⇒ không chạy**, sau
  `Configure` mới chạy, gỡ SC khi `Running` ⇒ ngủ ngay; yield không mất budget/thời gian; gắn SC cạn
  cho TCB `Running` ⇒ ngủ ngay + refill đánh thức nhiều TCB; gỡ SC khi `Sleeping{Budget}` không còn
  biên refill; invariant **1.000 seed × 64 bước ngẫu nhiên** với cửa sổ thẳng hàng.

### Quyết định đã chốt (model-only `[TK]`, ghi rõ)

- **Chu kỳ thẳng hàng toàn cục** (D2-49(e)): `refill` = bội số kế tiếp của `period`; **không** cửa
  sổ trượt/sporadic refill/timeout fault (D2-16 — hoãn sau K6).
- **TCB không SC / SC chưa `SchedControl.Configure` = KHÔNG có budget ⇒ `pick`/`dispatch` không
  chọn** (ngữ nghĩa §4.5 + K1B-PLAN §5 KT-20 DoD 4). TCB muốn chạy phải có SC **đã `Configure`**;
  passive server chạy bằng donation ở KT-21. **Đây là sửa theo verdict `reviewer-kt20` (DRIFT mục
  1)**, không phải "không giới hạn"; Agent 0 sẽ ghi **D2-50** khi lật sổ.
- **Charge trước, refill sau**; cấu hình lại giữa chu kỳ reset `consumed` + tính lại biên (quy ước
  5 trong `mcs.rs`); không dồn budget.

### Sửa theo review `reviewer-kt20` (DRIFT → đã sửa, không UNSAFE)

- **Mục 1 — no-SC ≠ unlimited (đã sửa đúng ngữ nghĩa)**: `ScBudget::has_budget` =
  `is_configured() && consumed < budget`; `ScBudget::remaining` = 0 khi chưa cấu hình;
  `Machine::tcb_has_budget` = **false** khi không SC hoặc SC chưa `Configure` ⇒ `sched_pick_on`
  (và `dispatch_step`) bỏ qua; `enforce_budget` đưa TCB `Running` về `Sleeping{Budget}` khi gỡ SC /
  gắn SC cạn / SC chưa `Configure`; `charge_tick` cũng đưa `Running` không SC/chưa cấu hình về
  `Sleeping{Budget}`. **Không** đổi `call/recv/reply`.
- **Rig test cập nhật (test-only, không đổi kỳ vọng cũ)**: `kt19_sched.rs` — `Rig` thêm
  `SchedControl` (slot 6) + `SchedControl.Configure` SC slot 4 với `SC_MAC_DINH = 64` rồi gắn cho 3
  TCB qua helper `gan_sc_mac_dinh`; `pick_du_256` thêm SchedControl/SchedContext (slot 256/257) và
  gắn cho **256** TCB; TCB "D" của test dispatch và 3 TCB của `RandRig` (1.000 seed) cũng được gắn.
  KT-18 không cần (không `dispatch`; chuyển trạng thái trực tiếp là model API, suite vẫn xanh).
- **Mục 2 — Miri**: reviewer đọc `kt20-miri.log` **giữa lúc tệp đang ghi** nên thấy 11/12 và chưa có
  `test result`; log hoàn chỉnh của lần chạy trước đã có `test result: ok. 12 passed`. Sau khi sửa
  review đã **chạy lại** (số dưới đây) ⇒ PASS đủ 12/12.

### Cổng + Miri (không retry — D2-07)

- [`JOURNAL/kt20-gate-fast.log`](JOURNAL/kt20-gate-fast.log) — `gate --fast` **PASS 284 test / 0 failed**
  (**44 suite**; KT-19: 263/43 ⇒ **+21 test** = 12 integration + 8 unit `mcs` + 1 unit `pick_where`),
  **L1 1.000 seed/999 trace**, gen-check ON, `.scn` 21 tệp OK, **không retry**; `kt20_budget` native
  2,8 s.
- [`JOURNAL/kt20-miri.log`](JOURNAL/kt20-miri.log) — chạy lại sau sửa review:
  `v2 miri -p vifast-sim --test kt20_budget` **PASS 12/12** (0 failed, 0 ignored, **117,12 s**);
  2 cảnh báo `integer-to-pointer cast` **có sẵn** từ objmem (`v2/lib/vifast-objmem/src/obj.rs`,
  D2-14/D2-17), không phát sinh từ KT-20; `v2/sim/**` **0 unsafe**. Invariant test dưới Miri vẫn
  chạy nhưng giảm `N_SEED = 5`, `N_STEP = 16` (`#[cfg(miri)]`; stable giữ **1.000 × 64**) — không
  `#[ignore]`, không nới `gate --fast`/L1.

### Phạm vi / không chạm

Chỉ `v2/sim/vifast-sim/src/{mcs.rs,sched.rs,kobj.rs,lib.rs}` (`lib.rs` +1 dòng `pub mod mcs;`),
`v2/sim/vifast-sim/tests/{kt20_budget.rs,kt19_sched.rs}` (rig test-only gắn SC mặc định),
`v2/agent/{STATE.md,JOURNAL/}`. Không sửa `spec/`, `vifast-model`, `.scn`,
QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`; không `#[allow]`/`#[ignore]`.

### Review độc lập + đề xuất

`reviewer-kt20` (lead spawn vì depth 1/1): **DRIFT** (không UNSAFE) — (1) "TCB không SC = không
giới hạn" trái §4.5/K1B-PLAN DoD 4 ⇒ **đã sửa** sang "không SC/chưa `Configure` = không có budget,
không chạy" + rig test-only gắn SC mặc định + test no-SC/chưa-Configure/cạn budget; (2) Miri "chưa
xong" do đọc log giữa lúc ghi ⇒ log hoàn chỉnh + chạy lại **PASS 12/12, 117,12 s**. Không phát hiện
UNSAFE; `v2/sim/**` 0 unsafe.

**Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (không tự DONE, không lật QUEUE — D2-37).
**YC/Follow-up**: (1) **Agent 0 ghi D2-50** "no SC ≠ unlimited" khi lật sổ (lead đã xác nhận sẽ
ghi); (2) KT-21 donation/passive server (SC đi theo Call/Reply — SC-less server chỉ chạy khi được
donate); (3) KT-23 thêm `kt20_budget` vào `nightly --only-miri` (117 s) và cân nhắc thời lượng Miri.

## KT-19 — Scheduler 1 hart: 256 mức + bitmap, `Ready/Running`, preempt mọi bước (K1b, D2-49(a))

**Vào**: READY (agentctl `next --project v2` trả KT-19) · HEAD `26643ba` · KT-18 DONE · baseline
`gate --fast` PASS 251 test/42 suite ([`JOURNAL/kt19-baseline-gate-fast.log`](JOURNAL/kt19-baseline-gate-fast.log)).
**Phạm vi**: scheduler **1 hart** cho **mô hình TCB** (không phải scheduler harness KT-08 `lib.rs`);
**không** budget/refill (KT-20), donation (KT-21), hộp thư/push migration (KT-22, K1c); không
`spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không thêm dependency/`unsafe`;
không `#[allow]`/`#[ignore]`. Working tree: `viscript/` + tệp ngoài `v2/` có thay đổi của tab khác —
**không chạm**, chỉ stage `v2/`.

### Đỏ trước (D2-05)

- [`JOURNAL/kt19-red-compile.log`](JOURNAL/kt19-red-compile.log) — `tests/kt19_sched.rs` không biên dịch:
  **93 × `E0599`** cho toàn bộ API còn thiếu (`ready_len`, `sched_pick`, `current`, `dispatch`,
  `check_sched`, `block_current`, `wake`, `yield_current`, `set_preempt_every_step`, `sched_idle`,
  `sched_cur_prio`, `ready_contains`).
- [`JOURNAL/kt19-red-queue.log`](JOURNAL/kt19-red-queue.log) — **sau khung** (đã có `HartSched` +
  `check_sched`/`dispatch` nhưng **chưa** nối `transition_tcb` với ready queue): **0/10 test pass**;
  thông điệp đúng dự kiến K1B-PLAN §5 KT-19 #2: `check_sched sau transition Ready: TCB 7 Ready nhưng
  không nằm trong ready queue`; test dispatch đỏ vì chưa TCB nào vào hàng ⇒ `current(0) == None`.
- Test red trước then chốt ở 2 log trên; sau khi nối một cửa `transition_tcb → sched_sync`, toàn bộ xanh.

### Sản phẩm

- `v2/sim/vifast-sim/src/sched.rs` (**mới**, ~300 dòng + 2 unit test): `HartSched {
  queues: [VecDeque<ObjId>; 256], bitmap: [u64; 4], len, current, idle, cur_prio }`;
  `highest_priority`/`pick` **O(1)** (tối đa 4 word, `leading_zeros`, không quét 256 hàng), cùng mức
  FIFO (`VecDeque::front`); `enqueue/remove_at/remove_all/set_current/clear_current`; `check_local`
  (bitmap ⇔ hàng khác rỗng + `len`) và `for_each_queued` (chỉ đi bit bật — bản quét 256 hàng/kiểm
  làm kt19 chậm 15 µs/kiểm, đã tối ưu còn 0,19 µs). Không `HashMap`/thời gian thực ⇒ tất định theo seed.
- `v2/sim/vifast-sim/src/kobj.rs`: `Machine.sched: Vec<HartSched>` (KT-19: `HART_COUNT = 1`, cấu trúc
  theo hart để K1c mở rộng) + `preempt_every_step: bool` (mặc định **tắt** — không đổi ngữ nghĩa KT-18);
  **một cửa**: `transition_tcb` preempt TCB cũ về `Ready` khi vào `Running{hart}` rồi `sched_sync`
  (gỡ khỏi bucket theo `priority`, nhả `current`, đặt `Ready`+`!suspended` vào hàng / `Running` thành
  `current`); hook `set_tcb_suspended`, `tcb_resume` (chỉ enqueue khi thực sự gỡ cờ `suspended`),
  `tcb_set_priority`/`tcb_set_affinity`/`tcb_set_home` (gỡ bucket cũ trước khi đổi). Model API `[TK]`:
  `ready_len`, `ready_len_at`, `ready_contains`, `current`, `sched_idle`, `sched_cur_prio`, `sched_pick`,
  `sched_pick_on`, `dispatch`, `block_current`, `wake`, `yield_current`, `check_sched`,
  `set_preempt_every_step`/`preempt_every_step`, `hart_count`; `TcbInfo` có `snapshot()` dùng chung.
  `sched_after_op(preempt)` gọi sau **mỗi** public op TCB/scheduler (`call/recv/reply/exit/suspend/
  resume/wait_notification/wait_address/set_priority/set_affinity/set_home/delete/revoke/retype` =
  preempt+kiểm; `finalise_tcb/configure/set_fault_ep/set_sched_params/bind_notification` = kiểm).
- `v2/sim/vifast-sim/tests/kt19_sched.rs` (**mới**, **10 test**, 0 ignored): (1) `pick` đủ **256 mức**
  0..=255, mức cao nhất trước, FIFO cùng mức, rút hết bằng `dispatch`+`Suspend(Abort)` (assert đúng
  từng mức); (2) dispatch `Running→Ready→Running`, ≤1 `Running`/hart, Running không trong hàng, tôn
  trọng priority + preempt khi có mức cao hơn; (3) `check_sched` sau **mỗi** public op (tour đủ
  Configure/SetFaultEp/set_sched_params/bind_notification/affinity/home/Ready/Call/Recv/Reply/Suspend/
  Resume/Wait notification/WaitAddr/Exit/finalise); (4) preempt ép **mọi bước**: 2 TCB cùng mức xen kẽ
  40 bước, **không đói** (≥19/40 mỗi TCB); (5) **1.000 seed × 64 op** ngẫu nhiên
  (call/recv/reply/suspend/resume/exit/retype): cùng seed ⇒ cùng trace, bất biến sau **mỗi** op,
  1.000/1.000 trace khác nhau; (6) wake TCB `suspended` **không** enqueue, `Resume` enqueue đúng
  priority; (7) `block_current`/`yield_current` đổi `current`, `block_current(Ready)` bị từ chối;
  (8) finalise TCB đang chạy + Suspend(Abort) + Revoke Untyped ⇒ `ready_len` giảm đúng, `current` được
  thay, không ObjId chết trong hàng; (9) affinity/home mặc định (1/0) + rebucket khi đổi priority;
  (10) phủ "một cửa"/curr.

### Bằng chứng đo (DoD 1-7)

- **256 mức**: test tạo đủ 256 TCB priority 0..=255, `sched_pick()` = mức 255; vòng rút hạ dần assert
  `pick == mức p` và `sched_cur_prio == p` cho **mọi** p (255→0) — O(1) theo 4 word bitmap, FIFO cùng mức.
- **≤1 Running/hart**: mỗi bước `dem_running(...) == 1`; `Running` không nằm trong hàng (`ready_contains`).
- **`Ready` ⇔ hàng + `!suspended`**: `check_sched` bắt mọi lệch (gồm cả `Ready+suspended` trong hàng);
  gọi sau mỗi public op + sau mỗi preempt; test gọi thêm sau từng op trong 1.000 seed (kể cả op lỗi).
- **Preempt/không đói**: 40 bước preempt trên 2 TCB cùng mức cho 20/20 lượt; 1.000 seed × 64 op =
  **64.000 op** (đo phân bố tạm thời: call 5.077 ok/4.137 lỗi, recv 5.476/3.714, reply 1.781/7.223,
  suspend 5.436/3.812, resume 8.936/0, exit 9.340/0, retype 9.068/0) — mọi op đều kiểm bất biến.
- **Gỡ hàng**: `Suspend(Abort)`/`finalise_tcb`/`Revoke Untyped` giảm `ready_len` đúng và thay `current`;
  `check_sched` xác nhận không còn ObjId chết/không Ready trong hàng (revoke 2 TCB ⇒ hàng 2→0).

### Cổng + Miri (không retry — D2-07)

- [`JOURNAL/kt19-gate-fast.log`](JOURNAL/kt19-gate-fast.log) — `gate --fast` **PASS 263 test / 0 failed**
  (43 suite; KT-18 là 251/42 ⇒ **+12 test**: 10 integration + 2 unit `HartSched`), **L1 1.000 seed/999
  trace**, gen-check ON, `.scn` 21 tệp OK, **không retry**; KT-19 test native 2,8 s.
- [`JOURNAL/kt19-miri.log`](JOURNAL/kt19-miri.log) — `v2 miri -p vifast-sim --test kt19_sched` **PASS
  10/10** (0 ignored, 4 m 32 s); 2 cảnh báo `integer-to-pointer cast` **có sẵn** từ objmem
  (`v2/lib/vifast-objmem/src/obj.rs:166,175`, D2-14/D2-17), không phát sinh từ KT-19; `v2/sim/**` **0 unsafe**.
- **Miri cần giảm phạm vi (ghi rõ, chỉ dưới `cfg(miri)`; stable giữ nguyên DoD)**: `check_sched` sau
  **mỗi** op quét toàn bộ TCB sống nên Miri chậm hơn ~1000 lần — đo thật 1.000 seed sẽ >4 h. Trên
  stable vẫn **1.000 seed × 64 op** và **đủ 256 mức**; dưới Miri: **5 seed × 64 op** và **64 mức**
  (`#[cfg(miri)] N_SEED = 5`, `N_PRIO = 64`; log 20 seed = 5 m 46 s trước khi giảm:
  [`JOURNAL/kt19-miri-1000seed-aborted.log`](JOURNAL/kt19-miri-1000seed-aborted.log)). Không `#[ignore]`,
  không nới ngưỡng `gate --fast`/L1.

### Phạm vi / không chạm

Chỉ `v2/sim/vifast-sim/src/{sched.rs,kobj.rs,lib.rs}` (`lib.rs` +1 dòng `pub mod sched;`),
`v2/sim/vifast-sim/tests/kt19_sched.rs`, `v2/agent/{STATE.md,JOURNAL/}`. Không sửa `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không dependency/`unsafe`;
không `#[allow]`/`#[ignore]`.

### Review độc lập + đề xuất

Đã báo lead spawn reviewer trước commit (depth 1/1) — lead spawn `reviewer-kt19` (review-only, đọc
diff uncommitted + tự chạy gate/Miri). **Verdict `reviewer-kt19`: APPROVE** (không DRIFT/UNSAFE);
reviewer tự chạy `gate --fast` **263 test/43 suite**, L1 1000/999, `kt19_sched` **10/10**; xác nhận
`pick`/bitmap/FIFO, bất biến Ready⇔queue, ≤1 Running, preempt mặc định tắt, scope sạch.

**Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (không tự DONE, không lật QUEUE — D2-37).
**Follow-up**: KT-20 thêm điều kiện "chỉ pick TCB có SC còn budget" (bổ sung vào `dispatch_step`,
không đổi API KT-19); KT-22/K1c push migration đọc `idle`/`cur_prio` đã công bố; KT-23 thêm
`kt19_sched` vào `nightly --only-miri` và cân nhắc thời lượng Miri (hiện 4 m 32 s với 5 seed/64 mức).

## KT-18 — TCB đầy đủ, bảng `ALLOWED` 11×11, `suspended` (xong mã, chờ verdict reviewer)

**Vào**: READY · HEAD `ecb0e08` · K1a' DONE, K1b mở (D2-49) · baseline `gate --fast` PASS
([`JOURNAL/kt18-baseline-gate-fast.log`](JOURNAL/kt18-baseline-gate-fast.log)).
**Phạm vi**: TCB + vòng đời của **mô hình sim**; **không** scheduler (KT-19), budget (KT-20), donation
(KT-21), hộp thư (KT-22); không `spec/`, `vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`.
Bảng `ALLOWED` đặt ở `v2/docs/k1b-tcb-transition.md` (model-only `[TK]`) theo **D2-49(d)**.

### Đỏ trước (D2-05)

- [`JOURNAL/kt18-red-compile.log`](JOURNAL/kt18-red-compile.log) — `tests/kt18_tcb.rs` **không biên dịch**:
  `E0599` (chưa có `transition_tcb`/`tcb_suspend`/`tcb_resume`/`tcb_set_priority`/`tcb_set_affinity`/
  `tcb_set_home`/`set_sched_params`/`tcb_info`/`tcb_bind_notification`/`pair_hit_count`/
  `transition_coverage`/`finalise_tcb`/`endpoint_send_len`/`reply_caller`/`notification_waiter`/
  `tcb_wait_notification`/`tcb_wait_address`), `E0308`/`E0609` (`tcb_read_state` trả `TcbState`, không
  phải `TcbSnapshot` 4 giá trị), `E0559` (`BlockedOnRecv` thiếu `reply`), `E0432` (`ALLOWED`/`TcbSnapshot`/
  `SuspendMode`/`BlockedOnKind` chưa có).
- [`JOURNAL/kt18-red-transition-gate.log`](JOURNAL/kt18-red-transition-gate.log) — đột biến **bỏ cổng**
  (`set_tcb_state` cũ nhận mọi cặp) ⇒ test ma trận 11×11 đỏ đúng lý do: cặp cấm `(0,0)` nhận `Ok(())`
  thay vì `IllegalOperation` (**14 passed, 1 failed**).
- [`JOURNAL/kt18-red-abort-unlink.log`](JOURNAL/kt18-red-abort-unlink.log) — đột biến **Abort không gỡ
  hàng chờ** ⇒ **5 test đỏ**: hàng gửi `Ok(1)` (mong `Ok(0)`), `Reply.caller Ok(Some(7))` (mong `Ok(None)`),
  hàng nhận `Ok(Some(8))`, notification waiter `Ok(Some(7))`, message Exit chưa huỷ (**10 passed, 5 failed**).

### Sản phẩm

- `v2/sim/vifast-sim/src/tcb.rs` (**mới**): `TcbState` (11 tên, `index()` khớp `vifast_abi::TCB_STATES`),
  `TcbSnapshot` (4 giá trị của `Tcb.ReadState`), `BlockedOnKind`, `SuspendMode`, `FaultKind`,
  `SleepReason`, **`pub const ALLOWED: [[bool; 11]; 11]`** (48 cặp ALLOWED/121 ô) + `allowed_count()` +
  `coverage()`; 2 unit test (không cặp danh tính nào ngoài `BlockedOnSend`; chỉ số/`TCB_STATES`).
- `v2/sim/vifast-sim/src/kobj.rs`: `TcbData` đủ **11 trường** (thêm `suspended`, `priority`,
  `max_controlled_priority`, `affinity: u64`, `home: u16`, `sched_context: Option<ObjId>`,
  `bound_notification: Option<ObjId>`); **một cửa `Machine::transition_tcb`** (cặp cấm ⇒
  `IllegalOperation`, **không** đổi state; `pair_hits: [u32; 121]` + `transition_coverage()`);
  `tcb_read_state → TcbSnapshot`; `tcb_suspend`/`tcb_resume`; `finalise_tcb` gọi **trước** `destroy`
  trong `revoke_children` (state `Dead`, gỡ endpoint/Reply/notification/address, trả `sched_context`
  qua hook + `finalise_log`); model API `set_sched_params` (D2-49(b)), `tcb_set_priority`/`tcb_set_affinity`/
  `tcb_set_home`/`tcb_bind_notification`/`tcb_info` (D2-49(c)); `Notification` có waiter + bucket
  WaitOnAddress tối giản (`tcb_wait_notification`/`tcb_wait_address`) chỉ để chứng minh Abort/finalise
  gỡ đúng hàng (Signal/WakeAddr thuộc K1c).
- **Drift D2-33 đã sửa**: `BlockedOnRecv { ep, reply: Option<ObjId> }`, `BlockedOnAddress.offset: u32`,
  `ReadState` trả 4 giá trị.
- **Bảng 48 cặp = 46 cặp suy ra từ thiết kế + 2 cặp `Stopping` reserved** cho KT-19/K1c (không có
  nguồn nhập `Stopping` ở K1b 1 hart); coverage **của bảng qua cổng** ở test ma trận, coverage
  **thao tác** ở test `moi_duong_dung_mot_cua_transition_tcb` — không dùng `(48,48)` làm coverage thao tác.
- `v2/sim/vifast-sim/tests/kt18_tcb.rs` (**mới**, **18 test**, 0 ignored): ma trận 11×11 (mỗi cặp một TCB
  trên **một** `Machine` ⇒ `transition_coverage() == (48, 48)`, cặp cấm `IllegalOperation` + state giữ
  nguyên + hit `0`), Abort gỡ hàng gửi/nhận/Reply/notification/address (đếm hàng giảm đúng 1) và giữ
  `exit_code`, `Defer ⇒ InvalidArgument` (state không đổi, không hit), `Resume` không phát lại
  message/queue + xoá `exit_code`, `Exit` giữ D2-25 (kể cả Exit khi đang chờ gửi ⇒ cặp danh tính 3→3),
  finalise 6 TCB khi Revoke Untyped (mọi hàng chờ rỗng, log 6 dòng, `sched_context` trả lại), và test
  "một cửa" (đường thật `call/recv/reply/exit/suspend/finalise` chỉ đi qua `transition_tcb`).
- `v2/docs/k1b-tcb-transition.md` (**mới**, model-only `[TK]`, D2-49(d)): bảng 11×11 + lý do từng nhóm
  cặp từ thiết kế §4.4/§4.5 + D2-25/D2-33, quy ước mô hình (kích hoạt ngầm, không cặp danh tính,
  `Defer`, `reply` bị bỏ khi `caller = None`), bảng drift đã sửa, việc của KT-19/20/21/K1c/K3.

### Cổng + Miri

- [`JOURNAL/kt18-gate-fast.log`](JOURNAL/kt18-gate-fast.log) — `gate --fast` **PASS**, **251 test / 0 failed**
  (42 suite; KT-17: 231/41 ⇒ +20 test), **L1 1.000 seed/999 trace**, gen-check ON, **không retry** (D2-07).
- [`JOURNAL/kt18-miri.log`](JOURNAL/kt18-miri.log) — `./v2/bin/v2 miri -p vifast-sim --test kt18_tcb`
  **PASS 18/18** (0 failed, 0 ignored); 2 cảnh báo `integer-to-pointer cast` **có sẵn** từ
  `v2/lib/vifast-objmem/src/obj.rs:175` (arena objmem, D2-14/D2-17), không phải code KT-18; `v2/sim/**` 0 `unsafe`.
- [`JOURNAL/kt18-check.log`](JOURNAL/kt18-check.log) — `v2 check` PASS (fmt/clippy/R1..R5/ABI/scn/report/gen-check).

### Review độc lập (`reviewer-kt18`)

`reviewer-kt18` (review-only) **DRIFT nhẹ, không UNSAFE**: tự chạy `gate --fast` **248 test/42 suite**,
L1 1000/999, Miri `kt18_tcb` **15/15**, xác nhận 48/73 cặp, một cửa `transition_tcb`, phạm vi sạch.
**Đã sửa đủ 4 mục:**
1. **Thứ tự `exit()`**: giờ đọc + `require_cap_type(fault_ep)` **trước** khi ghi `exit_code`/gỡ hàng
   chờ ⇒ cap `fault_ep` đã Delete/Revoke trả `InvalidCapability`, TCB **không** đổi trạng thái/hàng
   chờ (test mới `exit_cap_fault_ep_chet_khong_doi_trang_thai`).
2. **Bảng tự chứng minh vòng tròn**: thêm test `cap_cam_theo_thiet_ke_bi_tu_choi` (5 ca cấm viết từ
   lý do thiết kế: `Inactive→Running`, `Sleeping→Running`, `BlockedOnReply→BlockedOnRecv`,
   `Dead→Ready`, `Ready→Sleeping`); phát biểu lại DoD **46 cặp + 2 reserved**, `(48,48)` chỉ là
   coverage **của bảng qua cổng**, không phải coverage thao tác.
3. **Thiếu test `Suspend(Abort)` trên `Sleeping`**: thêm `suspend_abort_tren_sleeping_giu_trang_thai`
   (vẫn `Sleeping`, `suspended`, giữ `exit_code`, không hàng chờ, không hit cặp danh tính).
4. **Ghi chú doc**: `Exit` từ mọi trạng thái sống là quy ước model-only; `Send(reply)` lần hai im
   lặng thành công (reply bị bỏ) — đã ghi trong `k1b-tcb-transition.md` §4; fixture `12-…scn:13`
   `Defer` để **KT-23** (không sửa `.scn` ở KT-18).

### Đề xuất + follow-up

- **Đề xuất: REVIEW → DONE sau khi Agent 0 lật** (review độc lập DRIFT nhẹ đã sửa đủ; không tự DONE,
  không lật QUEUE — D2-37).
- **Follow-up cho KT-23**: `.scn` `12-suspend-abort-while-reply-wait.scn:13` ghi `Tcb.Suspend mode=Defer => ok`
  trong khi thiết kế §4.4 `:282` nói `Defer` **không** dùng cho IPC ⇒ KT-18 trả `InvalidArgument` (DoD 6);
  fixture mới chỉ kiểm tĩnh, khi KT-23 chạy `.scn` trên mô hình phải chốt lại (sửa fixture, không nới ngữ nghĩa).
- `Stopping` chưa có nguồn nhập trong K1b 1 hart (D2-49(a)) — cặp `Ready/Running → Stopping`,
  `Stopping → Dead` có sẵn cho KT-19/K1c; notification `Signal`/`WakeAddr` thuộc K1c; ready queue/refill
  queue hook thuộc KT-19/KT-20.

## DoD K1a' (chốt ở KT-17)

Định nghĩa xong của K1a' (thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:872-873`): *thay store giả
bằng objmem (API closure) + Miri*; các ca Untyped/CNode của KT-04c chạy trên mô hình.
**Đủ 2 dòng, mỗi dòng có bằng chứng đo được:**

| # | Điều kiện DoD | Bằng chứng |
|---|---|---|
| a | Thay store giả bằng objmem + **Miri từ commit đầu** | `vifast-objmem` Miri **40 test** (20 lib + 8 alias + 1 compile-fail + 1 fuzz 200 chuỗi/197,30 s + 10 contract); `./v2/bin/v2 nightly --only-miri` **PASS 365 s, exit 0** (sim `--lib` 13/13, kt16 4/4, kt09_untyped 8/8, kt09_revoke 7/7, kt10_cnode 13/13, kt17_align 3/3); `v2 miri` cũ vẫn xanh 13/13 — [`JOURNAL/kt17-nightly-only-miri.log`](JOURNAL/kt17-nightly-only-miri.log), [`JOURNAL/kt17-miri-old-cmd.log`](JOURNAL/kt17-miri-old-cmd.log) |
| b | Ca Untyped/CNode của KT-04c chạy trên mô hình, không hồi quy | `.scn` Untyped/CNode **9/9** bước khớp, 0 lệch (3 bước setup Frame để K2); **parity 10.000 seed/240.000 thao tác/9.065 trace khác nhau/0 lệch**; `gate --fast` **231 test/41 suite**, L1 1.000 seed/999 trace, `.scn` 21 tệp OK — [`JOURNAL/kt17-kt04c-scn-parity.log`](JOURNAL/kt17-kt04c-scn-parity.log), [`JOURNAL/kt17-gate-fast.log`](JOURNAL/kt17-gate-fast.log) |

> **Cập nhật 06:40 bởi Agent 2**: **KT-17 xong mã — K1a' đủ DoD** (task cuối phase K1a'):
> (1) đóng **2 follow-up KT-16** — `alloc_place_span` cap `align ≤ ARENA_ALIGN` (fail-closed,
> test đỏ `kt17-red-align.log` → xanh), `Machine::retype` Untyped `size_bits ≥ 13` trả
> `Alignment` rõ; `Store::alloc` chốt hợp đồng `0/không lũy thừa 2/> 4096 ⇒ None` + test;
> (2) **`v2 nightly --only-miri`** (D2-47(a), không vào `gate --fast`, không L1 100k/mutation/
> soak; dùng chung `require_miri_toolchain` với `v2 miri` nên thiếu toolchain fail rõ);
> (3) Miri tổng **PASS 365 s/exit 0** (objmem 40 test + sim 48 test); (4) `.scn` Untyped/CNode
> **9/9** + parity **10.000 seed/0 lệch**; (5) `gate --fast` **PASS 231 test/41 suite**
> (KT-16: 227/40; +4 test mới), L1 999 trace, **không retry**. Reviewer `reviewer-kt17`:
> **DRIFT nhẹ (chỉ tài liệu/DoD)** — đã sửa đủ 4 mục. Chi tiết + log: `JOURNAL/kt17-journal.md`,
> `JOURNAL/kt17-*.log`.
>
> **Cập nhật 06:00 bởi Agent 2**: **KT-16 xong mã** — thay **hai** store giả bằng
> `vifast-objmem` (D2-14, D2-47d): (1) `vifast-sim::Store` dùng `OwnedArena` (vùng căn 4096
> do objmem cấp, địa chỉ logic `0x1000+offset` giữ trace/golden tất định); (2) ruột
> `kobj::Machine` — body `Kind` nằm trong `ObjMem`, **mỗi Untyped một arena riêng** nên
> watermark = `Arena::used()` (bỏ `Vec<Object>` + `align_up` viết tay), Revoke Untyped
> `reset` cả cây; `Meta` giữ metadata cho `object_type`/`object_bytes` của object đã chết
> (parity hash cả object chết), `Cap` vẫn `Vec<Cap>`. Thêm `OwnedArena`/`OwnedMem` (feature
> `heap`) vào objmem — **toàn bộ `unsafe` nằm trong objmem**, `v2/sim/**` 0 `unsafe` (R5
> xanh); guard `chua_header` áp cho **cả** `with`/`with_mut` (reviewer KT-15, test đỏ trước
> `Ok(7)` vs `Dead`); bảng kind → ctor **đo 8.192 B = 8 KiB** (không phình); `ObjMem` có
> `id_base` để `id` duy nhất giữa các vùng. Test đỏ trước `kt16_objmem.rs` **10 × E0599**.
> Bằng chứng: **227 test/40 suite**, old test `git diff --stat kt09*/kt10*/kt11*` **rỗng**;
> parity 10.000 seed 0 lệch; **Miri PASS** (objmem 39 test; kt16 4/4; kt09_untyped 8/8;
> kt10_cnode 13/13); `aarch64-unknown-none` xanh (feature off); `gate --fast` PASS không
> retry. Reviewer `reviewer-kt16`: **APPROVE** (không DRIFT/UNSAFE). Chi tiết + log:
> `JOURNAL/kt16-journal.md`, `JOURNAL/kt16-*.log`.
>
> **Follow-up KT-17 (reviewer-kt16) — ĐÃ ĐÓNG ở KT-17**: (1) `alloc_place_span` cap
> `align > ARENA_ALIGN` (fail-closed `None`, không đốt byte) + `Machine::retype` Untyped
> `size_bits ≥ 13` ⇒ `Alignment` rõ ràng (trước: trả `pa` có thể sai căn); (2) `Store::alloc`
> chốt hợp đồng `align 0/không lũy thừa 2/> 4096 ⇒ None` + test đối chứng, doc ghi rõ.
> Không còn việc phải làm cho 2 follow-up này.

> **Cập nhật 04:48 bởi Agent 2**: **KT-15 xong mã** — `with_many_mut` thật theo **D2-48 (a)**:
> bảng cố định `kind → constructor` (256 ô trong `ObjMem`, `place::<T>` đăng ký), giữ `ObjRef` 16 B
> (thêm `into_dyn()` — vẫn 16 B, không vtable) + `KObj::as_any_mut()` (default `None`) để closure
> `[&mut dyn KObj; N]` ghi được kiểu cụ thể. Alias kiểm **từng cặp** trước khi tạo ref (trùng `pa`,
> chồng vùng, ref chết, kind lạ ⇒ `Alias` fail-closed, closure không gọi); `N = 9` **không build**
> (`const { assert!(N <= 8) }`, log `kt15-red-n9.log`). Crate **34 test** (lib 14, alias 8,
> compile_fail 1, fuzz 1, contract 10); **Miri PASS** (alias 8/8; fuzz **200 chuỗi/191,95 s** < 20 phút
> ⇒ giữ 200); fuzz stable **10.000 chuỗi × 32 thao tác** PASS (4,51 s); build `aarch64-unknown-none`
> xanh; `gate --fast` **PASS 218 test** không retry. Reviewer `reviewer-kt15` **DRIFT (nhẹ), không
> UNSAFE** — đã sửa: bỏ claim sai "fail-closed" (ref không mang identity store → thành hợp đồng
> `place`/`reset`), guard `chua_header`/`Arena::chua` (ref store khác/`pa` ngoài arena ⇒ `Alias`),
> `TypeId` + `debug_assert` chống va chạm `kind`, test end-to-end chồng vùng (`Gia`), trùng `id`,
> ZST, mảng trộn 3 kind, sửa số đo STATE. Chi tiết + bảng log: `JOURNAL/kt15-journal.md`.

> **Cập nhật 04:22 bởi Agent 2**: **KT-14 xong mã** — crate `v2/lib/vifast-objmem` (15 test: header 16 B,
> arena thô căn lề 8/16/64 + hết chỗ `None`, `place/with/with_mut/destroy`, `Dead`, id không tái dùng,
> object align 64); Miri 15/15, build `aarch64-unknown-none`, `gate --fast` 199 test/L1 1000 (không retry).
> Reviewer `reviewer-kt14` **DRIFT (nhẹ)** — đã sửa: test alias **fail-closed** (hết tautology), STATE,
> **YC-18** (`ObjRef<dyn KObj>` 16 B thiếu vtable → Agent 0 chốt trước KT-15), ghi chú `UnsafeCell`/
> store identity/cluster id. Chờ Agent 0 lật sổ.

> **Cập nhật 18:20 bởi người điều phối** theo `review/agent0/DUYET-YC-02-05.md`: YC-04/05 **đã đóng** (D2-32/D2-33), xem **Khối F** trong `QUEUE.md`. Không mở lại YC-04/05.

> **Cập nhật 10:10 bởi Agent 0**: KT-21 DONE (ef101b9, 298 test, Miri 13/13, donation 14.609 bước, D2-51); KT-22 READY.

> **Cập nhật 03:35 bởi Agent 2**: **KT-13 xong mã** — thêm `v2 miri` (D2-47(a): lệnh rời, **không** trong
> `gate --fast`); Miri PoC 13 test PASS; `check` + `gate --fast` PASS (184 test, không retry). Chờ review
> độc lập + Agent 0 lật sổ (D2-37).
> **Cập nhật 03:41 bởi Agent 2**: `reviewer-kt13` **APPROVE** (tự chạy PoC 13/13 + gate 184 test/L1
> 1000/999); đã sửa 3 nit (`D2-47(a)` citation, timestamp 03:35, câu chữ `nightly`); gate lại PASS
> ([`JOURNAL/kt13-gate-fast-final.log`](JOURNAL/kt13-gate-fast-final.log)). Đề xuất DONE.

## Ghi chú

- **L1 đã bật từ KT-08**: `gate --fast` chạy thêm `# L1 OK: 1000 seed` (sim `vifast-sim`, cùng seed → cùng trace). `gen-check` **đã bật** (KT-05, chạy trong `v2 check`/`gate --fast`).
- `./v2/bin/v2 check` / `gate --fast` **đã có** từ KT-03; `gate` đầy đủ và `nightly` **từ chối** (exit 2) cho tới khi có L2/L3 và sim — không núp bóng cổng rỗng.
- Task `REVIEW` (KT-04d) **không** được coi là đã duyệt; chỉ mở khi có dòng `APPROVED <YYYY-MM-DD> Agent 0` trong `QUEUE.md`. **Không** ghi APPROVED.
- Tag v1: `v1-archive` trên `ca2eaf5` (KT-01). **Không** có `v1-final`; v1 không soak lại.
- Nhánh làm việc: `main`. Mỗi commit `[KT-##]`; chạm `kernel/`, `lib/` phải xanh `--fast` (D2-05). **Push sau mỗi commit xanh**: `git push origin main`.
- Toolchain: `cargo`/`rustc` 1.99.1 ở `~/.cargo/bin` (**không** có trên PATH mặc định). `v2/bin/v2` **tự tìm** cargo (PATH → `~/.cargo/bin` → `/usr/local/cargo/bin` → `/opt/homebrew/bin`) và luôn dùng `--manifest-path v2/Cargo.toml` ⇒ **không bao giờ** chạy workspace v1.
- **D2-27**: KT-04c/KT-04d = **21** kịch bản (20 theo QD §4 + ca "Exit khi procsrv chưa Recv"). Q-01 đóng.
- **D2-28**: KT-02 chỉ port tối đa `vifast-fs`, `vifast-fsutil`, `vifast-elf`.
- **Thứ tự điều phối**: `KT-03` (xong) → **`KT-07`** (K0.5 boot riscv64) → `KT-04a`. KT-07 tạo `v2/kernel/` + `v2/boot/`; **không** tạo `v2/platform/` (cây chuẩn §8 là `kernel/vifast-plat-{riscv64,aarch64,x86_64}`).
- **QUEUE row chưa lật** `KT-02 DONE` / `KT-03 READY`: `vifast-agentctl` chưa tồn tại (KT-06) và Agent 2 **không** được sửa QUEUE — chờ Agent 0. Thực chất phụ thuộc đã DONE và đã kiểm lại.

## Cổng v2 (KT-03) — bộ luật R1..R5

`v2/bin/v2` = wrapper bash; `v2/test/rules/run.sh` = self-test 6 case (5 âm + 1 dương).

| Luật | Nội dung | Nguồn | Case âm |
|---|---|---|---|
| **R1** | Tệp trong `kernel/`, `lib/` ≤ **1.500** dòng | thiết kế §8 (cổng cứng, D2-06) | `case-r1-long` (1.501 dòng) |
| **R2** | Cấm `cfg(target_arch)` trong `kernel/vifast-core/**` + `lib/**` | thiết kế **:701** | `case-r2-cfg-arch` |
| **R3** | SPDX theo loại tệp: `//` cho `.rs/.s/.asm`, `#` cho `.toml/.vis/.vish/.vidl/.sh/.py`, sidecar `.license` cho `.json/.bin/.txt` | QD §9.3 | `case-r3-no-spdx` |
| **R4** | Cấm path-dependency sang `crates/` (v1 đóng băng) | D2-26 | `case-r4-path-dep` |
| **R5** | `unsafe` chỉ trong danh sách đóng: `vifast-sync`, `vifast-objmem`, `kernel/vifast-plat-*`, `boot/` | D2-17, thiết kế §8 | `case-r5-unsafe` |

Ghi chú: bộ **5 luật này là R1..R5** (Agent 2 chọn; không phải 6 luật của thiết kế §8 — gen-check là luật thứ 6, thuộc KT-05). R1 làm ở KT-03 thay vì "giao T05" như `review/C-handoff.md` §315 đề nghị: **chặt hơn**, không nới ngưỡng.

## Hiện trạng đo được (sau KT-03)

| Hạng mục | Hiện trạng | Dẫn chứng |
|---|---|---|
| `HEAD` | xem `git log -1`: KT-04a ở `1766145` (+ commit STATE `a8bb272`); KT-07 ở `bf61901`+`e01a81f` | `git log -1 --oneline` |
| Cổng | `./v2/bin/v2 check` PASS (gồm `abi spec`, `abi reserved`, `scn`); `gate --fast` PASS (thời gian: xem log, không ghi tay — Q12); `gate`/`nightly` exit 2 | log cổng; `v2/bin/v2:255` |
| Luật | self-test **6/6** (`ok 1..6`); R1..R5 có case âm đúng mã luật | `bash v2/test/rules/run.sh` |
| Build/test | `cargo test --manifest-path v2/Cargo.toml --workspace` xanh: fs **55** (52 unit + 3 T26) + elf **20** = **100**, 0 failed; clippy `-D warnings` 0 warning; fmt sạch | trong `gate --fast` |
| Luật 1.500 dòng | tệp lớn nhất `vifast-fs/src/fs.rs` = **1.248** dòng | `wc -l` |
| `spec/` | **có** `spec/abi.toml`: 16 syscall (0..14 + **15 DebugGetInfo** D2-29), dải reserved **16..31**, 19 mã lỗi, object **1..8** (SchedControl 8 = lõi, D2-32) + reserved **9..16** (`[object_reserved]` 17..31); `status = "REVIEW"` | `./v2/bin/v2 check` |
| `v2/bin/v2` | **có** (wrapper), `.gitkeep` giữ thư mục | `ls v2/bin` |

## KT-13 — Bootstrap Miri + `v2 miri` (xong mã, review APPROVE, chờ lật sổ, 2026-11-13 03:35)

**Vào**: READY · HEAD `5ad86e5` · baseline `gate --fast` PASS (L1 1000 seed/999 trace, 556 ms).
**Phạm vi**: chỉ `v2/bin/v2` + `STATE`/`JOURNAL`; **không** đụng 5 luật R1..R5, `nightly` (vẫn stub),
`v2/agent/{QUEUE,DECISIONS}.md`, `spec/`, `AGENTS*`, `viscript/`.

**Đỏ trước (D2-05)**: [`JOURNAL/kt13-red-wrapper.log`](JOURNAL/kt13-red-wrapper.log) — `./v2/bin/v2 miri`
→ `v2: lệnh không biết: miri`, **exit 2**; [`JOURNAL/kt13-red-stable.log`](JOURNAL/kt13-red-stable.log) —
`cargo miri --version` trên stable → *"the 'miri' component … is not available for the
'stable-aarch64-apple-darwin' toolchain"*, exit 1.

**Hiện thực**: `find_rustup` (PATH → `~/.cargo/bin` → `/usr/local/cargo/bin` → `/opt/homebrew/bin`);
`do_miri` kiểm nightly + `miri` + `rust-src` + `cargo +nightly miri --version`, thiếu thì **exit 2**
(thiếu rustup **127**) và **in đúng câu bootstrap** `rustup toolchain install nightly` +
`rustup component add miri rust-src --toolchain nightly`; chạy
`cargo +nightly miri <sub> --manifest-path v2/Cargo.toml` (mặc định `sub=test`, nhận
`test/run/nextest/setup/clean`); usage thêm dòng `miri` ghi rõ D2-47 (lệnh rời, **không** trong
`gate --fast`). Exit code các lệnh cũ giữ nguyên (`gate`/`nightly`/lệnh lạ vẫn exit 2).

**Thiếu toolchain (giả lập bằng PATH + rustup/cargo giả)**: [`JOURNAL/kt13-red-toolchain.log`](JOURNAL/kt13-red-toolchain.log)
— 5 ca: thiếu rustup (127), thiếu nightly (2), thiếu `miri` (2), thiếu `rust-src` (2), `cargo-miri`
hỏng (2); cả 5 đều in câu bootstrap, không im lặng bỏ qua.

**PoC**: [`JOURNAL/kt13-miri-poc.log`](JOURNAL/kt13-miri-poc.log) —
`./v2/bin/v2 miri -p vifast-sim --lib` → **13 passed; 0 failed** (7 unit `lib.rs` + 6 unit `kobj.rs`;
Miri 33,17 s, tổng 50,28 s, EXIT=0), không `#[ignore]`, không cần `-Zmiri-disable-isolation`.

**Toolchain**: [`JOURNAL/kt13-toolchain.log`](JOURNAL/kt13-toolchain.log) — nightly `1.102.0-nightly
(3ef0c22cd)`, `miri 0.1.0`; component `miri` + `rust-src` có.

**Cổng**: [`JOURNAL/kt13-check.log`](JOURNAL/kt13-check.log) `check` **PASS**;
[`JOURNAL/kt13-gate-fast.log`](JOURNAL/kt13-gate-fast.log) `gate --fast` **PASS 184 test / 0 failed**,
L1 **1000 seed / 999 trace** (600 ms), 36,665 s, **không retry** (D2-07); sau 3 nit tài liệu chạy lại trên
cây cuối: [`JOURNAL/kt13-gate-fast-final.log`](JOURNAL/kt13-gate-fast-final.log) **PASS 184 test / 0 failed**,
L1 **1000 seed / 999 trace** (603 ms), 36,559 s.

**Review**: `reviewer-kt13` (độc lập, review-only) **APPROVE** — tự chạy PoC Miri **13/13**, `gate --fast`
**184 test / L1 1000/999**; xác nhận wrapper/exit code/phạm vi đúng. 3 nit tài liệu đã sửa (`D2-47(a)`
citation, timestamp 03:35, câu chữ `nightly`).

**Đề xuất**: **DONE** (review độc lập APPROVE, commit + push xong). Không lật QUEUE (D2-37).

## KT-09 — Untyped trên sim + Retype/Revoke + end-to-end (xong mã, chờ lật sổ)

- **Đỏ trước (D2-05)**: `JOURNAL/kt09-red-runtime.log` — 4 test target không biên dịch (chưa có
  `vifast_sim::kobj`/`scenario`); `JOURNAL/kt09-red-fixtures.log` — fixture `bad-state-value` bị bắt nhưng
  **sai thông điệp** (máy kiểm chỉ kiểm tên trường, chưa kiểm giá trị).
- **Sản phẩm**: `v2/sim/vifast-sim/src/kobj.rs` (Untyped watermark, CNode Revoke con/cháu, TCB/Endpoint/Reply/
  SchedContext; mã lỗi lấy từ `vifast_abi::ERRORS`), `src/scenario.rs` (C5: `expect_states` là hậu điều kiện
  **sau mỗi bước**, `exit` kiểm ngay sau halt trước procsrv `Recv`; scheduler chen giữa bước theo seed),
  4 tệp `tests/kt09_*.rs` (**27 test mới**); `check_scn.py` kiểm **giá trị** `expect_states`;
  `run_fixtures.sh` kiểm **đúng thông điệp** (5 fixture).
- **Cổng**: `gate --fast` PASS; L1 **1000 seed/999 trace** không đổi; golden trace không đổi; 21 `.scn` cũ PASS.
- **Không làm (đúng phạm vi)**: property test cây MDB (KT-10), `vifast-model` (KT-11), Frame/VSpace (K2).
- **KT-09b (sửa review Agent 0)**: `TcbState` đủ **11 variant** D2-33 + unit test so `vifast_abi::TCB_STATES`;
  `check_bytes` siết `allocated ≤ watermark`, từng range `≤ watermark`, `allocated + gap + free == size`
  (+ `untyped_allocated`/`untyped_gap`, test gap căn lề); revoke Untyped **lồng nhau** finalise cả cây con
  (+ test), sửa bug Untyped con `size=0`. Tổng **29 test** KT-09; gate --fast PASS, L1 1000 seed/999 trace.
- **Đề xuất**: Agent 0 lật `KT-09 = DONE` (không có YC mới).

## KT-10 — CNode/MDB test-first (xong mã, 2026-11-12 23:15)

**Vào**: READY, phụ thuộc KT-09 DONE (`3d578c0` + `da7d30f`) · HEAD `f638bd6` · baseline `gate --fast` PASS.

**Phạm vi**: mô hình tối giản trong sim/test; property test cây; Revoke. **Không** làm MDB đầy đủ
production (không `mdb_prev/mdb_next` 48 B), **không** `vifast-model` (KT-11), **không** Frame/VSpace/K2.

**Đỏ trước (D2-05)** (đính chính theo review độc lập F1/F2):
- [`JOURNAL/kt10-red-compile.log`](JOURNAL/kt10-red-compile.log) — 2 test target không biên dịch
  (chưa có `mint`/`cap_info`/`cap_table`/`cap_children`/`live_cap_count`/`check_caps`).
- [`JOURNAL/kt10-red-semantic.log`](JOURNAL/kt10-red-semantic.log) — **4/7** test đỏ đúng ngữ nghĩa:
  `delete` để con **mồ côi** (cha chết vẫn là parent) — 2 test `kt10_cnode` (`delete_*`) + 2 property
  (`cung_seed_cung_cay_tat_dinh`, `property_cay_cap_tat_dinh_giu_bat_bien`). **3/7** còn lại
  (`finalise_cnode_long_nhau_*`, `revoke_untyped_finalise_*`, `property_untyped_revoke_*`) panic ở
  `check_caps` với "cha cùng object" — **rule quá chặt cho con của `Retype`**, không phải lỗi finalise
  (rule đó đã bỏ). Vì vậy: đỏ-trước **chỉ chứng minh lỗi `delete`**; finalise CNode có test xanh
  nhưng **không có log đỏ riêng** — ghi nhận là hạn chế bằng chứng, không nhận "tái lập byte-for-byte"
  (log đỏ sinh trước khi chỉnh test: bỏ rule `check_caps`, kỳ vọng `revoke(None)`, `div_ceil`, unit checker).

**Sản phẩm**:
- `src/kobj.rs`: quyền `RIGHT_*`/`RIGHTS_ALL`, `CapInfo` + `cap_info`/`cap_table`/`live_cap_count`/
  `cap_children`/`cnode_slot_count`, `Machine::mint` (quyền ⊆ nguồn ⇒ `InsufficientRights`; badge chỉ
  Endpoint/Notification; `derive` kiểm trước khi cấp ⇒ lỗi không rò cap), `check_caps` (bất biến cây
  MDB), **`delete` reparent con cho ông** (seL4 `cteDelete`), **finalise CNode** khi Untyped revoke
  giết CNode (xoá cap trong slot + cả cây con, dọn slot) — không còn cap mồ côi/trỏ object chết.
- `tests/kt10_cnode.rs` (**13 test**): copy/mint/revoke, cha–con, cap chết, slot trống/đầy,
  `RangeError`/`SlotOccupied`/`InvalidCapability`/`InsufficientRights`/`InvalidArgument`, phạm vi revoke,
  delete reparent, finalise CNode (kể cả CNode lồng nhau).
- `tests/kt10_property.rs` (**5 test**): generator LCG tự viết + oracle cây trong test;
  **128 seed × 48 bước = 6.144 thao tác** (copy 1.102, mint 1.109, delete 802, revoke 1.141 xoá 1.093 cap,
  lỗi 762) — kiểm bất biến sau **mỗi** thao tác; 64 seed `Untyped.Revoke` finalise; 2 ca chuỗi sâu;
  tái lập cùng seed.
- `src/kobj.rs` unit: **2 test** chứng minh `check_caps` không rỗng (bắt cap mồ côi, slot không đối xứng,
  cap chết còn giữ slot). Tổng **20 test mới**, 0 ignored.

**F3 (ngoài DoD, chuyển KT-11/K1a)**: quyền mới chỉ cưỡng chế ở `Mint` (⊆ quyền nguồn); `Copy`/`Retype`/
`Delete`/`Revoke` **chưa** kiểm quyền; `mint(Some(0))` giữ badge nguồn (không xoá badge). Không sửa code
trong KT-10.

**Cổng**: [`JOURNAL/kt10-gate-fast.log`](JOURNAL/kt10-gate-fast.log) — `gate --fast` PASS,
**161 test / 0 failed**, L1 **1000 seed / 999 trace** (582 ms, đo bởi cổng); golden + 21 `.scn` không đổi.

**Review**: review độc lập `reviewer-kt10` **APPROVE** (source + test; tự chạy gate PASS 161 test,
property 128×48=6144, staged chỉ `v2/`); yêu cầu sửa tài liệu F1/F2 + ghi F3 — đã sửa.

**Đề xuất**: Agent 0 lật `KT-10 = DONE` sau review commit cuối (không có YC mới; KT-11 mở sau khi DONE).

## KT-11 — `vifast-model` Untyped/CNode, F3 quyền, so sim 10.000 seed (xong mã, 2026-11-13 00:50)

**Vào**: READY, phụ thuộc KT-10 DONE (`c78e814`) · HEAD `f98e1e7` · baseline `gate --fast` PASS
161 test / L1 1000 seed ([`JOURNAL/kt11-baseline-gate-fast.log`](JOURNAL/kt11-baseline-gate-fast.log)).

**Phạm vi**: crate **`v2/lib/vifast-model`** (thêm workspace) — mô hình Untyped/CNode **tái sử dụng**,
không test-only; chạy 4 ca `.scn` Untyped/CNode của KT-04c; F3 (review KT-10); parity test với
`vifast-sim` **10.000 seed**. Không MDB 48 B, không Frame/VSpace/K2; không sửa `spec/`/`QUEUE`/
`DECISIONS`/`AGENTS`; không `#[ignore]`, không nới ngưỡng. Review `reviewer-kt11` **DRIFT** → sửa đủ;
Agent 0 ban hành **D2-44..46**.

**Đỏ trước (D2-05)**:
- [`JOURNAL/kt11-red-model-api.log`](JOURNAL/kt11-red-model-api.log) — 3 test target của crate chưa
  biên dịch (`unresolved imports vifast_model::{Machine, KError, SlotRef, ...}`).
- [`JOURNAL/kt11-red-f3-sem.log`](JOURNAL/kt11-red-f3-sem.log) — **3/3 FAILED** trên code KT-10:
  `mint(Some(0))` giữ `Some(7)` (mong `None`); `Retype` qua cap chỉ READ trả `Ok(3)` (mong
  `InsufficientRights`); cap con `Retype` có `rights = 15` (mong `3`).
- [`JOURNAL/kt11-red-f3-copy.log`](JOURNAL/kt11-red-f3-copy.log) — `copy_rights` chưa có (`E0599`).

**Sản phẩm**:
- `v2/lib/vifast-model/src/{object,error,cap,machine}.rs` — `ObjType`/19 `KError` ánh xạ `vifast-abi`;
  Untyped watermark; CNode `copy`/`copy_rights`/`mint`/`move_cap`/`delete`/`revoke`; `retype`;
  finalise Untyped lồng nhau + CNode; `check_bytes`/`check_caps`; lớp `invoke_*` kiểm quyền cap CNode.
- **F3 (D2-45/46)**: `Copy` có `rights?` (⊆ nguồn); `Retype` đòi **WRITE** + cap con **kế thừa quyền**;
  `Mint(Some(0))` **xoá badge**, badge ≠ 0 chỉ Endpoint/Notification; `Delete`/`Revoke` qua cap CNode
  cần **WRITE** (`[TK]`, K1a rà lại). Sim `kobj.rs` sửa 3 điểm: `retype` WRITE/kế thừa,
  `mint Some(0)` xoá badge, thêm `copy_rights`.
- **D2-44 `CNode.Move`** `[TK]`: `dst` đã có cap thì xoá cap đích rồi chuyển (overwrite) — test kiểm
  cap đích chết, cháu reparent cho ông, không mồ côi; `Copy`/`Mint` vẫn `SlotOccupied`.
- `.scn` runner tối giản trong test (`tests/kt11_scn.rs`) cho `01/02/03/16`: **9/9** bước khớp,
  3 setup `Retype Frame` chưa mô hình (K2). **D2-46** sửa fixture `02-cnode-mint-badge.scn`
  (`02:11` badge trên Untyped → `src=ep0`; `02:12` hai Mint cùng `dst` → thêm `c2`, `dst=c2`) —
  sửa dữ liệu test, không đổi ABI, không hard-code 7/9.
- **Parity test** (không re-export `kobj` của sim): **10.000 seed / 240.000 thao tác, 0 lệch**
  (9065 trace khác nhau, 10,5 s), assert `check_caps` + `check_all_bytes` **sau mỗi bước**:
  [`JOURNAL/kt11-diff-10000.log`](JOURNAL/kt11-diff-10000.log). `Move` không có trong sim nên test riêng.
- 19 test model mới + 4 test sim F3 mới, 0 ignored.

**Cổng**: [`JOURNAL/kt11-gate-fast.log`](JOURNAL/kt11-gate-fast.log) — `gate --fast` PASS,
**184 test / 0 failed**, L1 **1000 seed/999 trace** (505 ms); fmt/clippy/R1..R5 sạch.

**Đề xuất**: **REVIEW** (reviewer `reviewer-kt11` DRIFT → đã sửa đủ 5 mục; chờ xác nhận lần cuối;
không tự nhận DONE, không lật QUEUE). **Không còn YC**: D2-44/45/46 đã chốt.

## Việc đang làm — KT-T26 (3 h theo QUEUE/PATCH-BRIEF/thiết kế)

DoD đủ 5 mệnh đề (khôi phục theo review #7 — không được tự rút): (1) host test `vifast-fs`;
(2) **block giả + cache**; (3) `MAX_IO = 256`; (4) đọc **RAW** kiểm read-your-own-write;
(5) đánh **"ai chen giữa"** theo D-03.

**Đã làm**: `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs` (3 test, 1.000 seed, `CachedDev` đang ship,
ca âm **mất ghi im lặng** đã đỏ khi đảo kỳ vọng) + `v2/docs/kt-t26-analysis.md` (bảng kẻ chen giữa).
**Chưa làm được trong crate**: tái hiện đầy đủ T26 (`-smp 2`, 50 boot) — **YC-06**.

## KT-04c — `.scn` xong phần viết được; arity BLOCKED (YC-05)

- `v2/docs/scn-format.md` (`[TK]`, `version = 1`, chiếu §9.3), `v2/test/scn/check_scn.py` + 3 fixture âm,
  **21 tệp** `.scn`, gắn vào `v2/bin/v2 check` (bước "scn" + "scn fixtures").
- `18-schedcontext-donation.scn` **đã mở** (SchedControl lên lõi ở KT-04b): `SchedControl.Configure` + `SchedContext.Bind`.
- **Arity + trạng thái**: xong ở KT-04c bằng draft `v2/docs/abi-call-signatures.toml` (D2-33); `check_scn.py`
  kiểm đủ tham số/không lạ/variants và `expect_states` có trường; 21/21, 0 blocked.

## KT-04b — BLOCKED chờ YC-04 (không ghi `spec/`)

- Sản phẩm hợp lệ đã làm: draft `v2/docs/abi-reserved-objects.toml` (9 object reserved + SchedControl, id 8..16,
  `unsupported = true`) + `v2/test/abi/check_reserved.py` gắn vào `v2/bin/v2 check`.
- **YC-04** (BLOCKED): xin Agent 0 (a) cho KT-04b ghi `spec/abi.toml`, hay (b) Agent 0 tự chép draft; kèm câu hỏi
  "VSpace khách = object riêng hay method" và đề nghị lật `QUEUE:102/103/104`.
- Máy kiểm đã vá 2 lỗ (trùng id, ngoài dải) + gỡ bom `status == "REVIEW"`.

## Đã xong — KT-04a (`spec/abi.toml` lõi + bảng ánh xạ)

- `spec/` ở **gốc repo** (AGENTS.md §1/§4.1 — bậc cao hơn cây §8 của thiết kế): `version/abi_version/min_compat = 1`,
  `status = "REVIEW"`, `[frozen]` đủ 5 thứ, `[message]` 64 MR + 4 cap, `[message_info]` 35 bit,
  `[calling.riscv64]` lấy từ code v1 (`a7` số hiệu, `a0/a1` tham số, `a2` MessageInfo, `a3..a6` 4 MR),
  `[object]` lõi **1..8** (SchedControl 8 = lõi theo D2-32), `[object_reserved]` 9..16 + 17..31, `[error]` 19 mã, `[syscall]` 0..15, reserved 16..31.
- Bảng ánh xạ: `v2/docs/abi-v1-v2-mapping.md` (25 crate; 58 hàng = 12→9 số syscall + 27 hàng method (23–24 đích, chưa chuẩn) + 19 bỏ; 22 biến thể KError → 0 + 19).
- Máy kiểm: `v2/test/abi/check_abi.py` (Python ≥3.11 `tomllib`), **gắn vào `v2/bin/v2 check`** — `check` nay có 6 bước.
- Đã đóng: **YC-02** (D2-29: DebugGetInfo = syscall 15), **YC-03** (D2-30/D2-31: đánh số lại; `spec/` ở gốc repo).

## Lịch sử trạng thái (mới nhất ở trên)

| Ngày | Task | Bước | HEAD | BUILD_ID | Cổng | Ghi chú |
|---|---|---|---|---|---|---|
| 2026-11-13 | **KT-21** | 8/8 | `d547bbf` | — | `gate --fast` PASS (**298 test/45 suite**, L1 1000 seed/999 trace, 550 ms) | donation tự động Call/Reply + passive server + MCP (model-only `[TK]`); **13 test** (invariant **1.000 seed × 64 op**, **14.609** bước có donation), Miri **13/13** (129 s); commit `eae876b` + follow-up `[KT-21]` (3 điều kiện `reviewer-kt21`) đã push; **chờ Agent 0 ghi D2-51 + lật sổ** |
| 2026-11-13 | **KT-18** | 8/8 | `5cccb72` | — | `gate --fast` PASS (**251 test/42 suite**, L1 1000 seed/999 trace) | **commit `5cccb72` đã push**; TCB 11 trường + **một cửa** `transition_tcb` + bảng `ALLOWED` 48 = **46+2 reserved**, 18 test (ma trận 121 cặp, coverage bảng 100%), Miri kt18 **18/18**; drift D2-33 sửa; reviewer DRIFT nhẹ đã sửa; chờ Agent 0 lật sổ |
| 2026-11-13 | **KT-15** | 8/8 | `c0d3163` | — | `gate --fast` PASS (**218 test**, L1 1000 seed/999 trace, 596 ms) | **commit `c0d3163` đã push — reviewer DRIFT nhẹ đã sửa, chờ Agent 0 lật sổ**; `with_many_mut` D2-48(a): bảng `kind→ctor`, alias từng cặp, N=9 không build; Miri PASS (alias 8/8, fuzz 200/191,95 s); fuzz 10.000 chuỗi/4,51 s; `aarch64-none` xanh; guard `chua_header` + `TypeId` |
| 2026-11-13 | **KT-14** | 8/8 | `e1921ab` | — | `gate --fast` PASS (199 test, L1 1000 seed/999 trace, 433 ms) | **DONE** `36c51bc`; `vifast-objmem` 15 test, Miri 15/15, build `aarch64-unknown-none`; reviewer DRIFT nhẹ đã sửa; **YC-18** chốt **D2-48** |
| 2026-11-13 | **KT-13** | 8/8 | `5ad86e5` | — | `check` + `gate --fast` PASS (184 test, L1 1000 seed/999 trace, 603 ms) | **reviewer-kt13 APPROVE — chờ Agent 0 lật sổ**; `v2 miri` (D2-47(a): lệnh rời, không trong `gate --fast`); Miri PoC 13 test PASS; nhánh thiếu toolchain fail rõ |
| 2026-11-13 | **KT-11** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace, **184 test**) | **DONE** `035584b`; `vifast-model` Untyped/CNode + F3; parity sim **10.000 seed/0 lệch**; `.scn` **9/9** (D2-44..46) |
| 2026-11-12 | **KT-10** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace, 161 test) | **xong mã — chờ Agent 0**; CNode/MDB test-first: `mint`, `check_caps`, delete reparent, finalise CNode; **20 test mới**, property 6.144 thao tác |
| 2026-11-12 | **KT-09b** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace) | sửa review Agent 0: `TcbState` đủ 11 variant (test spec), `check_bytes` siết allocated/gap, revoke Untyped lồng nhau; **29 test** |
| 2026-11-12 | **KT-09** | 8/8 | — | — | `gate --fast` PASS (L1 1000 seed/999 trace) | **xong mã — chờ Agent 0 lật sổ** (D2-37); Untyped/Revoke/e2e + C5; **27 test**; fixture âm kiểm thông điệp |
| 2026-11-12 | **KT-12b** | 8/8 | — | — | `gate --fast` PASS (có bước no_std) | **xong mã — chờ Agent 0 lật sổ** (D2-37); no_std + std gating đúng D2-42; gen sinh TRACE_* từ `[trace]` |
| 2026-11-12 | **KT-12** | 8/8 | — | — | `check`+`gate --fast` PASS | **DONE**: `vifast-traceformat` (Record 32 B D2-38, ring, decode_all) + golden nhị phân 200×32 B + cross-check text↔nhị phân |
| 2026-11-12 | **KT-05** | 8/8 | — | — | `check`+`gate --fast` PASS (gen-check ON) | **DONE**: `[method_signature]` stable-draft vào spec; `vifast-gen` sinh code+`docs/v2/abi.md`; gen-check so byte + 3 fixture âm |
| 2026-11-12 | **KT-R1** | 8/8 | — | — | `check` PASS (báo cáo) | **DONE**: `R1-v2.md` 6 mục + `R1-v2.json` 11 khoá (project v2); `check_report.py` nối vào `v2 check`; ghi cả hai trạng thái khi sổ lệch |
| 2026-11-12 | **KT-08** | 8/8 | — | — | `gate --fast` PASS (**L1 1000 seed**) | **DONE**: `v2/sim/vifast-sim` (store giả + scheduler 1 hart, trace băm FNV-1a); `v2 sim` CLI; **L1 1.000 seed bật trong gate --fast**; 7 test unit + 1 test L1 |
| 2026-11-12 | **KT-06** | 8/8 | — | — | `check` PASS (agentctl 9/9 test) | **DONE** (sửa theo review #10): `vifast-agentctl next/list/report/start/done/block` (đọc mặc định; `--write` mới ghi; `start` chỉ READY); parser nhận **KT- và ST-**, chuẩn hoá `**DONE**`, `set_status` giữ escape; `v2 agentctl` nối đúng dispatch; **13/13** test |
| 2026-11-12 | **KT-T26** | 8/8 | — | — | `check` PASS + 3 test T26 | **DONE đèn host** (1.000 seed, RAW, ca âm mất ghi im lặng); **tái hiện đầy đủ BLOCKED** (YC-06: crate không có VFS/smp) |
| 2026-11-12 | **KT-04c** | 8/8 | — | — | `check` PASS (scn 21 tệp) | **DONE đủ**: 21 `.scn` 0 blocked, arity theo draft D2-33, 4 fixture âm, ca 18 mở |
| 2026-11-12 | **KT-04b** | 8/8 | — | — | `check` PASS | **BLOCKED** (YC-04): không được ghi `spec/`; draft id 8..16 + `check_reserved.py` gắn cổng; vá 2 lỗ máy kiểm (trùng id/ngoài dải), sửa 2 sai số KT-04a |
| 2026-11-12 | KT-04b | 1–7/8 | `a8bb272` | — | — | review#5 **DRIFT** → dừng trước `spec/`, chuyển draft + YC-04; đỏ trước: 9 object reserved thiếu |
| 2026-11-12 | **KT-04a** | 8/8 | `545f3b4` | — | `gate --fast` PASS | **DONE**: `spec/abi.toml` (REVIEW) + 3 bảng ánh xạ + `check_abi.py` gắn cổng; 4 điều kiện review #4 xử lý |
| 2026-11-12 | KT-04a | 1–7/8 | `545f3b4` | — | — | review#4 ON-TRACK; đỏ trước `abi-check: KHÔNG có spec/abi.toml`; quy ước gọi lấy từ code v1 (4 MR) |
| 2026-11-12 | **KT-07** | 8/8 | `c9cdc9e` | — | boot 50/50 + `gate --fast` | **DONE**: K0.5 boot riscv64 qua OpenSBI; TAP 1 test; `image 14.512 B`, `.data+.bss = 0 B` (trần 256 KiB), `.stack 64 KiB` NOLOAD báo riêng |
| 2026-11-12 | KT-07 | 1–7/8 | `c9cdc9e` | — | — | review#3 ON-TRACK (3 điều kiện) → đỏ trước (`chưa có crate kbin`); `vifast-plat-riscv64` (unsafe, UART/SBI) + `vifast-kbin-riscv64` (safe, linker 0x80200000); kernel workspace tách khỏi v2 |
| 2026-11-12 | **KT-03** | 8/8 | `785996a` | — | `gate --fast` PASS | **DONE**: wrapper `v2/bin/v2` (check/gate --fast/rules; gate đầy đủ + nightly từ chối), 5 luật R1..R5 + self-test 6/6, `check` PASS, `gate --fast` 4,1 s |
| 2026-11-12 | KT-03 | 1–7/8 | `785996a` | — | — | review#2 ON-TRACK (5 điều kiện) → đỏ trước 6/6 (`chưa có v2/bin/v2`); viết fixture + run.sh; sửa R2/R3/`--manifest-path`; `JOURNAL/review-2026-11-12-2.md` |
| 2026-11-12 | **KT-02** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: workspace `v2/` + `bin/` + port `vifast-fs`(10 tệp, max 1.248 dòng)/`vifast-fsutil`/`vifast-elf`; commit `b4a8693`, `054c3e2` |
| 2026-11-12 | KT-02 | 2–7/8 | `329177e` | — | cổng chưa có | tách 4.543 dòng v1 → 10 tệp; `cargo fix` gỡ 13 import; D2-27/D2-28 nhận trong lúc làm |
| 2026-11-12 | KT-02 | 1/8 | `329177e` | — | cổng chưa có | lập kế hoạch nhỏ 4 bước; review#1 ON-TRACK |
| 2026-11-12 | **KT-01** | 8/8 | `329177e` | — | cổng chưa có | **DONE**: tag annotated `v1-archive` trên `ca2eaf5` (subject "T33a mở"); 7/7 mục KNOWN-ISSUES khớp QD §6; test đỏ **N/A** — commit `19659ee` |
| 2026-11-12 | KT-01 | 3–7/8 | `329177e` | — | cổng chưa có | đỏ trước: `git tag -l v1-archive` rỗng; tạo tag; dán review#1; sửa câu chữ Exit + mở Q-01 |
| 2026-11-12 | KT-01 | 1/8 | `329177e` | — | cổng chưa có | mở vòng 1: đọc 7 tệp + STATE + review subagent trước task đầu |
| 2026-11-12 | — | khởi tạo | `ca2eaf5` | — | — | bàn giao vòng 1: 17 dòng task, 5 READY (KT-01, KT-02, KT-04a, KT-T26, KT-R1), KT-04d REVIEW |
