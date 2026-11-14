# KT-38 — `VSpace` + ASID (8 bit/256/thế hệ) + `generation` + `active_on` (K2)

**Vào**: READY (`agentctl next --project v2` trả KT-38) · HEAD `d240026` ([KT-00] lật KT-37 DONE,
512 test) · `git rev-list --count HEAD..origin/main` = **0** · working tree có `viscript/` của tab
khác (**không chạm**, chỉ stage `v2/`) · baseline `gate --fast` **PASS 512 test/67 suite**, L1
1.000 seed **1463 ms** ([`kt38-baseline-gate-fast.log`](kt38-baseline-gate-fast.log)).

Trong lúc làm, tab ViScript commit thêm `f158085`/`cc7b096` lên `main` (không đụng `v2/`); trước
push vẫn kiểm `HEAD..origin/main = 0`.

**Nguồn ngữ nghĩa**: `review/agent0/K2-PLAN.md` §3 KT-38 + §2 **Q4** (ASID/generation/invalidate)
+ **Q9** (thứ tự B→40, C∥B); **D2-65** (chốt Q1–Q10), **D2-66** (Frame); `v2/docs/k2-vm.md`
§2 (hằng số: `ASID_BITS = 8`, 256/thế hệ) + §11 (PageTable); `v2/sim/vifast-sim/src/pagetable.rs`
(oracle walk) + `kobj.rs` (`HartMask` ≤ 64 bit của affinity, `MAX_HARTS = 8`, `set_hart_online`
tinh thần D2-59); `spec/abi.toml` `VSpace id=9` `reserved`/`unsupported`, method 0 `GetAsid`
(debug); thiết kế §4.10 `:353-366` ("Object: VSpace…", "ASID có generation (hết ASID thì tăng
generation và flush toàn bộ)", "VSpace giữ `active_on: HartMask`").

**Không** làm (đúng brief): TLB/shootdown/ack thế hệ/chống deadlock (KT-40), W^X/protect
(KT-39), VM fault (KT-41), `.scn`/`check_vm`/shared read-only (KT-42); không `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không `unsafe`, không dependency
ngoài workspace, không `#[ignore]`/`#[should_panic]`, **không sửa test cũ**.

## 0. Kế hoạch nhỏ (8 bước, ghi vào STATE)

1. Baseline `gate --fast` + đọc nguồn. 2. Test đỏ **biên dịch** (API mới). 3. `vspace.rs` +
`kobj.rs`. 4. Test xanh + unit âm `check_vspace`/`check_asid`. 5. `k2-vm.md` §12.
6. `check` → `gate --fast`. 7. Miri. 8. Review độc lập → commit `[KT-38]` → push → STATE/JOURNAL.

## 1. Test đỏ trước (D2-05)

- [`kt38-red-compile.log`](kt38-red-compile.log) — `tests/kt38_vspace.rs` viết trước khi có code:
  **E0432** `no AsidState/FlushScope/HartMask/VSpaceInfo/ASID_COUNT in kobj` + **E0599** cho
  `vspace_create`, `vspace_*`, `check_vspace`, `check_asid` (135 dòng E0432/E0599) ⇒
  `could not compile`, exit 101. Tái lập bằng cách tạm `git stash` **chỉ** `src/kobj.rs` +
  `src/lib.rs` về KT-37 với test file đã viết (log ghi đúng trạng thái trước KT-38; đã `stash pop`,
  md5 hai tệp khôi phục nguyên).
- **Không có ca đỏ ngữ nghĩa**: KT-38 **không đổi** ngữ nghĩa API cũ — `Retype(VSpace)` **vẫn**
  `NotSupported` (spec giữ `reserved`); KT-38 chỉ **thêm** model API. Đúng câu chữ brief
  "test đỏ trước: `kt38_vspace.rs` E0599/API".

## 2. Thay đổi (chỉ `v2/`)

| Tệp | Nội dung |
|---|---|
| `v2/sim/vifast-sim/src/vspace.rs` **(mới)** | `HartMask` (≤64 bit: `bits/for_harts/contains/insert/remove/union/intersection/is_subset_of/count`), `VSpaceData` (root/asid/generation/active_on), `VSpaceInfo`, `AsidState`, `FlushScope` (`Asid`/`Generation`), `FlushCmd` (seq/scope/generation/asid/harts), `ASID_COUNT = 1 << vifast_pte::ASID_BITS` (**256**, đọc từ crate), `fresh_asid_free()` (dãy giảm dần 255..0 ⇒ cấp 0,1,2,…) + **3 unit test** (bit/subset; `ASID_COUNT = 2^ASID_BITS = 256`; roundtrip struct) |
| `kobj.rs` | `Kind::VSpace` (mốc body, như Frame/PageTable); `Meta.space`/`ChoDat.space`/`push_meta`; `Machine` thêm `vspaces`, `vspace_objects`, `asid_generation`, `asid_free`, `asid_issued`, `asid_exhaustions`, `hart_online`, `hart_vspace`, `flush_log`; `vspace_create(src)` (**model API**) + `create_vspace`; `asid_alloc` (cạn ⇒ `gen += 1` + **một** lệnh `Generation`) + `push_flush`; API `vspace_count/info/root/get_asid/active_on/activate/deactivate/walk/map/map_obj/unmap`, `set_hart_online/hart_online`, `asid_state`, `flush_log/flush_count/last_flush`; `check_vspace` (5 nhóm) + `check_asid` (3 nhóm) chạy trong `sched_after_op` sau **mỗi** public op; `finalise_vspace` (root tinh khiết, phát lệnh `Asid`, gỡ `hart_vspace`, tombstone); `revoke_children` kiểm VSpace **trước mọi thay đổi** (`HasChildren`, atomic) + finalise VSpace **trước** PageTable; `page_table_attach` từ chối root VSpace làm bảng con; **4 unit test** (3 âm: active_on ngoài online/lệch `hart_vspace`; VSpace chết còn dữ liệu/root bị attach; free-list trùng/ASID trùng giữa 2 VSpace/ASID còn trong free-list; + 1 ca **cạn ASID rẻ** gieo free-list còn 1 slot ⇒ `gen += 1` đúng một lần + đúng một lệnh `Generation` — dùng cho Miri) |
| `v2/sim/vifast-sim/tests/kt38_vspace.rs` **(mới)** | **7 test**: (1) `vspace_create` + root 4 KiB (VSpace 0 byte) + `Retype(VSpace)` vẫn `NotSupported` + walk oracle 3 cỡ (4 KiB/2 MiB/1 GiB) + unmap + atomic khi hết arena; (2) **257 ASID ⇒ generation +1 đúng 1 lần, đúng 1 lệnh flush `Generation`, ASID cũ không tái dùng, teardown 257 VSpace giữ free-list thế hệ mới** (dưới `cfg(miri)` rút còn 8 ASID — D2-47(a); ca cạn ASID dưới Miri là unit test ở trên); (3) ASID không tái dùng trong thế hệ (6 vòng tạo→huỷ→tạo, flush `Asid` harts rỗng); (4) `active_on` activate/deactivate, một hart một VSpace, hart offline không activate; (5) **hart offline ⇒ không nhận shootdown** (flush `Asid` chỉ tới hart online); (6) lỗi/biên (WrongObjectType/InvalidCapability, root không attach được, `HasChildren` atomic); (7) **property 10.000 thao tác** tạo/huỷ VSpace + `check_vspace`/`check_asid` sau mỗi op |
| `v2/sim/vifast-sim/src/lib.rs` | `pub mod vspace;` |
| `v2/docs/k2-vm.md` | **§12** (mới): object/root + model API `vspace_create` (Retype vẫn reserved), ASID free-list/"không tái dùng trong thế hệ", `active_on`, **lệnh flush** cho KT-40, 5+3 nhóm bất biến, `HasChildren`/thứ tự finalise, ranh giới KT-38 |
| `v2/agent/JOURNAL/kt38-*`, `STATE.md` | log baseline/đỏ/check/gate/miri + trạng thái |

## 3. Quyết định có thể gây tranh luận (ghi rõ thay vì ẩn)

1. **VSpace tạo bằng model API `vspace_create(src)`; `Retype(VSpace)` vẫn `NotSupported`.**
   - Lý do: `spec/abi.toml` giữ `VSpace` `reserved`/`unsupported` (K2-PLAN §1.2: conformance
     "reserved ⇒ `NotSupported`" **vẫn xanh**; brief KT-38 ghi "tạo VSpace root (**model API**)");
     test cũ `kt09_untyped.rs` **khẳng định** `Retype(VSpace) ⇒ NotSupported` và **không được
     sửa**.
   - Hệ quả: khác Frame/PageTable (KT-36/37 cài **qua** Retype) — **chênh lệch có chủ đích**, ghi
     ở `k2-vm.md` §12.1. Nếu Agent 0 muốn đồng nhất (Retype) thì phải cho phép sửa 2 dòng
     `kt09_untyped` — đề xuất để K3 thống nhất khi mở ABI.
   - `vspace_create` giữ nguyên khuôn `Retype` (cap con của cap Untyped nguồn ⇒ `Revoke` Untyped
     finalise VSpace + root; trả `CapId` theo D2-35; cần `RIGHT_WRITE`).
2. **ASID đã cấp "về hưu" cho hết thế hệ** (teardown **không** trả free-list): chống "stale TLB
   giả" ở K3 (hart đã rời VSpace vẫn có thể còn entry TLB `(gen, asid)`). Teardown =
   thôi sở hữu + **flush theo asid** + về hưu; chỉ `generation += 1` mới tái lập free-list 256.
3. **`active_on` có bất biến đối xứng với `hart_vspace`** (mỗi hart tối đa một VSpace — như thanh
   ghi ASID của MMU) và `active_on ⊆ hart_online`. `set_hart_online(hart,false)` bỏ hart khỏi
   **mọi** `active_on` + `hart_vspace` ⇒ teardown không gửi shootdown cho hart offline.
4. **Flush là mô hình lệnh** (`flush_log`), **không** có TLB/shootdown (KT-40). Teardown phát
   đúng **một** lệnh `Asid` (kể cả `harts` rỗng — không hart nào từng nạp ASID); cạn ASID phát
   đúng **một** lệnh `Generation` cho **mọi hart online** (kể cả hart đã rời mọi VSpace — entry
   TLB cũ của thế hệ cũ có thể còn). Kind `(asid, va)` cho unmap/protect là KT-40 (Q4).
5. **Property 10.000 chia `SO_EPOCH=40 × MOI_EPOCH=250`** (mỗi epoch một `Machine` mới).
   - Lý do: `check_*` của harness chạy sau **mỗi** public op và cấp phát theo `objects.len()`
     (append-only: object chết vẫn nằm trong bảng). Đo bằng probe tạm (`kt38_probe`, đã xoá):
     1.000 vòng tạo/huỷ trên **một** Machine = **2,45 s** ⇒ 10.000 vòng ≈ **4 phút** (O(n²));
     check_sched/check_pages/check_caps mỗi lần ~0,8/0,7/0,5 ms ở n=2.000 object.
   - Chunk vẫn đủ **10.000 thao tác tạo/huỷ thật**, mỗi op vẫn `check_vspace`+`check_asid`, ASID
     chuỗi `0,1,2,…` không trùng trong epoch; test riêng (2) đã phủ "cạn ASID ⇒ gen +1".
   - `cfg(miri)`: 1 epoch × 2 vòng (D2-47(a)).
6. **VSpace không tiêu byte arena; root tiêu đúng 4 KiB** (`Meta.len = 0` cho VSpace). Như Frame
   (0 byte) vs PageTable (4096 B); `check_bytes` vẫn xanh vì range rỗng.
7. **Thứ tự kiểm/finalise**: `Revoke` Untyped kiểm `HasChildren` (root còn PTE/bảng con) **trước
   mọi thay đổi**; finalise **VSpace trước PageTable** (bước kiểm "root tinh khiết" của VSpace
   còn dữ liệu, rồi root mới tombstone). `page_table_attach` từ chối root VSpace (giữ bất biến
   "root = bảng gốc").

## 4. Cổng và bằng chứng

- [`kt38-baseline-gate-fast.log`](kt38-baseline-gate-fast.log) — baseline PASS **512 test/67
  suite**, L1 1.000 seed **1463 ms**.
- [`kt38-check.log`](kt38-check.log) — `./v2/bin/v2 check` PASS (fmt/clippy `-D warnings`, luật
  R1..R5, ABI/scn/gen/golden/report).
- [`kt38-gate-fast.log`](kt38-gate-fast.log) — `./v2/bin/v2 gate --fast` **PASS 526 test/68
  suite** (baseline 512/67 + 7 `kt38_vspace` + 3 unit `vspace` + 4 unit `kobj`), L1 1.000 seed
  **1261 ms** (chạy lần cuối sau khi artifact đã build — lần trước log ghi 11,5–12,7 s là **gồm
  build test binary vừa sửa**, không phải thời gian L1), fmt/clippy `-D warnings` sạch, luật R1..R5
  OK, gen-check ON. Reviewer tự chạy trước đó: **525/68** (lúc chưa có unit test cạn ASID) —
  chênh đúng **+1 unit test**, không đổi test cũ.
- `cargo test -p vifast-sim --test kt38_vspace` **7/7** (9,04 s); `--lib` **53/53**; L0 workspace
  **525 passed / 0 failed**.
- [`kt38-miri.log`](kt38-miri.log) — Miri **PASS** (mọi mục exit 0): `--test kt38_vspace`
  **7/7** (61,29 s); `--lib` lọc `vspace` **5/5** (3 unit `vspace::tests::*` + 2
  `kobj::tests::check_vspace_*`); `--lib 'kobj::tests::check_vspace'` **2/2** (1,61 s);
  `--lib 'kobj::tests::check_asid'` **2/2** (3,27 s, gồm ca cạn ASID rẻ). Dưới `cfg(miri)`:
  test 257 → **8 ASID**, property 10.000 → **1 epoch × 2 vòng**, vòng ASID còn 6 → **2** (D2-47(a);
  stable giữ 256+1/10.000). Warning `integer-to-pointer cast` là của `vifast-objmem` (D2-17),
  không phải KT-38; `v2/sim/**` 0 unsafe.
  Tối ưu trong lúc chạy Miri: `check_asid` bắt trùng `(gen, asid)` bằng **sort** O(n log n)
  (thay `Vec::contains` O(n²)) + mask 256 bit cho "ASID còn trong free-list" (O(1)/VSpace) —
  không đổi ngữ nghĩa, giữ Miri của ca 257 ở mức chấp nhận được.
- Unit test âm **chứng minh checker bắt lỗi** (không chỉ đếm): `active_on` ngoài online, lệch
  `active_on`/`hart_vspace`, VSpace chết còn dữ liệu, root bị attach, free-list trùng, ASID trùng
  trong thế hệ, ASID còn trong free-list.

## 5. Review độc lập

- Lead spawn `reviewer-kt38` (read-only, nhận cảnh báo phải cho verdict về **quyết định 1**).
  Verdict: **APPROVE** — xác nhận `HartMask` chỉ có **một** định nghĩa, quyết định **VSpace tạo
  bằng model API / `Retype(VSpace)` vẫn `NotSupported`** được chấp nhận (**không sửa test cũ**);
  tự chạy `kt38_vspace` + gate **525 test/68 suite** (L1 < 60 s). Sau APPROVE chỉ tối ưu nội bộ
  `check_asid` (sort + mask) và rút cỡ test 257 dưới `cfg(miri)` — không đổi API/ngữ nghĩa.

## 6. Commit + push

- **Commit `74145d0`** — `[KT-38] VSpace + ASID 8 bit/256 + generation + active_on + lệnh flush
  (model); gate 526 test, Miri 7+5+2+2`; **đã push** `origin/main` (`9483373..74145d0`; trước push
  `git rev-list --count HEAD..origin/main` = **0**, sau push = **0**).
- Chỉ stage **12 tệp `v2/`** — `src/vspace.rs`, `src/kobj.rs`, `src/lib.rs`, `tests/kt38_vspace.rs`, `docs/k2-vm.md`,
`agent/STATE.md`, `agent/JOURNAL/kt38-{journal.md,baseline-gate-fast.log,check.log,gate-fast.log,
miri.log,red-compile.log}`; **không** `viscript/`, **không** `git add -A`. Trước push:
`git rev-list --count HEAD..origin/main`; nếu 0 push thẳng, nếu có commit mới thì `git pull
--rebase` (conflict ngoài `v2/` ⇒ dừng, báo).

## 7. Không đổi

`spec/` (`VSpace`/`PageTable`/`Frame` vẫn `reserved`/`unsupported`; `Retype(VSpace)` vẫn
`NotSupported`), `v2/lib/vifast-model` (oracle), `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`,
`v2/bin/v2`; **không sửa test cũ** (`kt09_untyped` giữ nguyên); không `unsafe`, không dependency
ngoài workspace, không `#[ignore]`/`#[should_panic]`, không nới ngưỡng/seed nào của cổng.
