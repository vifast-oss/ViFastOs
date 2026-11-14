# KT-36 — `Frame` 3 cỡ (4 KiB / 2 MiB / 1 GiB) + ownership/Untyped + `GetAddress` + bộ đếm W^X

**Vào**: READY (`agentctl next --project v2` trả KT-36) · HEAD lúc vào `c987fe1` ([KT-00] lật KT-35
DONE, 477 test, D2-65); trước commit HEAD `9b70618` (D2-66 của Agent 0, xem §4.2) · `git rev-list --count HEAD..origin/main` = **0** · working tree có thay đổi
`viscript/` của tab khác (**không chạm**, chỉ stage `v2/`).

**Nguồn ngữ nghĩa**: `review/agent0/K2-PLAN.md` §1.1 mục 1 + §2 Q2/Q3/Q7 + §3 KT-36; **D2-65**
(chốt Q1–Q10); `v2/docs/k2-vm.md` (hằng số `[TK]` của KT-35, thêm §10 của KT-36); crate
`v2/lib/vifast-pte` (`FrameSize`, `PAGE_SIZE_BITS` — **đọc từ crate, không chép giá trị**);
`v2/sim/vifast-sim/src/kobj.rs` (`ObjType::Frame` reserved 11, `KError` 19 mã, retype/revoke/
finalise KT-09..KT-25); `spec/abi.toml` `Frame id=11` `reserved`/`unsupported`, method 2 =
`GetAddress`; v1 `crates/vifast-kernel/src/object.rs:310` (`Frame { pa, size, mapped_w, mapped_x }`)
+ `lib.rs:548-572` (W^X theo bộ đếm) để giữ đúng tinh thần "bộ đếm giữ từ v1".

**Không** làm (đúng brief): không map/unmap PageTable/VSpace (KT-37/38), không rights/attrs khi
map, không TLB/shootdown (KT-40), không luật W^X (`WxViolation` — KT-39), không `spec/`,
`vifast-model`, `.scn`, QUEUE/DECISIONS/AGENTS, `viscript/`; không `unsafe`, không dependency
mới ngoài crate `vifast-pte` nội bộ, không `#[ignore]`/`#[should_panic]`, không sửa test cũ.

## 0. Kế hoạch nhỏ (8 bước, ghi vào STATE)

1. Baseline `gate --fast` (477 test) + đọc nguồn. 2. Test đỏ **ngữ nghĩa** (API cũ). 3. Test đỏ
**biên dịch** (API mới). 4. Code `frame.rs` + `kobj.rs` + `Cargo.toml` + `lib.rs`. 5. Test xanh +
unit test âm cho `check_frames`. 6. `check` → `gate --fast`. 7. Miri `kt36_frame` + lib. 8. Review
độc lập → commit `[KT-36]` → push → STATE/JOURNAL.

## 1. Test đỏ trước (D2-05)

- [`kt36-red-semantics.log`](kt36-red-semantics.log) — bản **đầu** của `tests/kt36_frame.rs` chỉ
  dùng API **cũ**: `retype(Untyped, Frame, 13)` trả `NotSupported` (kỳ vọng `InvalidArgument`) và
  ba cỡ `{12,21,30}` đều `NotSupported` (kỳ vọng `Ok`) ⇒ **0 passed / 2 failed**, đỏ đúng lý do
  "Frame còn reserved".
- [`kt36-red-compile.log`](kt36-red-compile.log) — bản **đầy đủ** (12 test + property) viết trước
  khi có code: **E0432** `no FrameMeta/UNTYPED_PA_WINDOW in kobj` + **E0599** `no method
  check_frames/frame_meta/frame_address/frame_get_address/frame_note_map/frame_note_unmap/
  frame_count/untyped_frames/untyped_pa_window` ⇒ **78 lỗi**, `could not compile`.
- [`kt36-baseline-gate-fast.log`](kt36-baseline-gate-fast.log) — baseline **PASS 477 test/65
  suite**, L1 1.000 seed **832 ms**, fmt/clippy/luật/gen-check OK.

## 2. Thay đổi (chỉ `v2/`)

| Tệp | Nội dung |
|---|---|
| `v2/sim/vifast-sim/src/frame.rs` **(mới)** | `FrameMeta { pa, size_bits, live, maps, mapped_w, mapped_x }` + `size_bytes`/`counters_zero`/`has_mapping`; `FrameArea { base, watermark }` + `carve` (Alignment/NotEnoughMemory, **không** đệm căn lề) + `window`; `UNTYPED_PA_WINDOW = 1 << 30` `[TK]` |
| `v2/sim/vifast-sim/src/kobj.rs` | `Kind::Frame` (mốc body); `Meta.frame`/`Meta.frame_area`; `ChoDat` + `push_meta`; `add_untyped(base, size)` **dùng `base`** làm đáy cửa sổ PA; `retype` rẽ nhánh `Frame` → `retype_frame` (4 lỗi atomic); `frame_pa_overlap` (chặn chồng pa, O(#Untyped) nhờ `frame_windows`); `frame_get_address`/`frame_address`/`frame_meta`/`frame_count`/`untyped_frames`/`untyped_pa_window`/`frame_note_map`/`frame_note_unmap`; `check_frames` (**7 nhóm** bất biến, khớp D2-66) chạy cả trong `sched_after_op` ⇒ **sau mỗi public op**; `finalise_frame`; `revoke_children` từ chối `HasChildren` khi Frame còn mapping + reset watermark cửa sổ; `check_bytes`/`untyped_allocated` **bỏ qua** Frame (không tiêu byte arena) |
| `v2/sim/vifast-sim/src/lib.rs` | `pub mod frame;` |
| `v2/sim/vifast-sim/Cargo.toml` | dependency nội bộ `vifast-pte` (D2-65 Q1: đọc cỡ từ crate hình PTE) |
| `v2/sim/vifast-sim/tests/kt36_frame.rs` **(mới)** | 12 test tích hợp: cỡ lạ, 3 cỡ × GetAddress/meta/bộ đếm, biên `0/2^12/2^21/2^30`, pa lệch 1, 1 GiB không ăn byte, trộn cỡ giảm dần, frame chết sạch, revoke bị từ chối `HasChildren`, bất biến bộ đếm, ownership/chồng pa, **10.000 retype + huỷ**, property **10.000 seed** |
| `v2/sim/vifast-sim/src/kobj.rs` (unit test) | 5 test **âm**: `check_frames` bắt Frame mồ côi, frame chết còn mapping, `pa` lệch căn/ngoài cửa sổ, watermark sai, hai Frame chồng `pa`, metadata thừa; 1 test `FrameArea::carve` biên căn lề/đầy/tràn `u64` |
| `v2/docs/k2-vm.md` | **§10** (mới): `FrameMeta` 6 trường, `UNTYPED_PA_WINDOW`, thứ tự kiểm của `Retype`, `GetAddress`, finalise + `check_frames`, ranh giới "không làm" |
| `v2/agent/JOURNAL/kt36-*`, `v2/agent/STATE.md` | log đỏ/baseline/check/gate/miri + trạng thái |

## 3. Quyết định có thể gây tranh luận (ghi rõ thay vì ẩn)

1. **`pa` là nhãn ảo của mô hình, không phải vùng nhớ thật.** Frame cấp từ **cửa sổ PA 1 GiB**
   của Untyped (`UNTYPED_PA_WINDOW`, `[TK]`), `pa = base + watermark`, **không** tiêu byte arena
   ⇒ 1 GiB Frame không ăn RSS (DoD + rủi ro (a)). Hệ quả: `base` của `add_untyped` (trước đây bị
   bỏ qua) trở thành **đáy cửa sổ PA**; Untyped không chồng cửa sổ nếu cùng cấp Frame
   (`AlreadyMapped`). Lý do không dùng thẳng `size` của Untyped: 1 GiB Untyped sẽ cấp 1 GiB host
   hoặc phải đổi arena — ngoài phạm vi KT-36 và phá bất biến byte của KT-09/KT-16.
2. **Không căn lề ngầm.** `pa` lệch căn ⇒ `Alignment` (đúng DoD "pa lệch 1 ⇒ Alignment"); muốn
   trộn cỡ thì cấp **giảm dần** (1 GiB → 2 MiB → 4 KiB luôn đúng căn). Đệm ngầm sẽ làm `pa` phụ
   thuộc thứ tự cấp mà không kiểm được; v1 (`untyped_take`) cũng chỉ nhận block **đã** đúng căn.
3. **Revoke Untyped khi Frame còn mapping ⇒ `HasChildren`, atomic.** K2 chưa có ngược
   Frame→PageTable để tự gỡ mapping (KT-37/39), nên "không im lặng bỏ mapping" = từ chối. Frame
   chết vì thế **luôn** có ba bộ đếm 0; `check_frames` kiểm lại như lưới thứ hai.
4. **Bất biến bộ đếm của KT-36 là `w ≤ maps && x ≤ maps`**, chưa phải `w + x ≤ maps`: luật "một
   mapping không vừa W vừa X" là W^X của **KT-39** (Q3: "KT-39 toàn bộ luật"). Ghi rõ trong
   `k2-vm.md` §10.1 + doc `FrameMeta`.
5. **`check_frames` chạy trong `sched_after_op`** (sau **mỗi** public op), không chỉ trong test.
   Chi phí O(#object) mỗi op chấp nhận được (cùng chỗ với 7 checker K1b/K1c sẵn có).
6. **Test 10.000 retype + huỷ làm theo 100 vòng × 100 Frame, mỗi vòng một `Machine`**: bất biến
   sau mỗi op của mô hình có chi phí O(số object **đã từng** cấp), nên dồn 10.000 Frame vào một
   mô hình là O(n²) (~56 s đo được) mà **không** kiểm thêm gì; 100 vòng vẫn là **10.000** lần
   retype + 10.000 lần huỷ thật. Miri giảm còn 2 × 32 bằng `cfg(miri)` (giữ nguyên 10.000 ngoài
   Miri — D2-47(a)).
7. **Property 10.000 seed** dùng sổ mapping phụ `(frame, w, x)` để `unmap` truyền **đúng** bộ cờ
   của lần `map` — mô hình không tự nhớ quyền của mapping (KT-37 sẽ truyền quyền thật). Trong
   lúc làm, `check_frames` đã **bắt** đúng một trạng thái `maps = 0` mà `w = x = 1` do fuzz
   unmap sai cờ — bằng chứng checker không rỗng (và là lý do thêm sổ mapping).

## 4. Cổng và bằng chứng

- [`kt36-check.log`](kt36-check.log) — `./v2/bin/v2 check` **PASS** (fmt/clippy `-D warnings`, luật
  R1..R5, ABI/scn/gen/golden/report). Hai lần đỏ trong lúc làm: clippy `unnecessary_map_or`
  (`map_or(false, …)` → `is_some_and`) và `manual_is_multiple_of` trong test — sửa code, **không**
  `#[allow]`.
- [`kt36-gate-fast.log`](kt36-gate-fast.log) — `./v2/bin/v2 gate --fast` **PASS 494 test/66 suite (65 suite cũ + `kt36_frame`)**
  (baseline 477 + 12 `kt36_frame` + 5 unit test âm), L1 1.000 seed **769 ms** (baseline 832 ms —
  không hồi quy), fmt/clippy/luật/gen-check OK.
- [`kt36-miri.log`](kt36-miri.log) — Miri `--test kt36_frame` **PASS 12/12** (36,10 s; seed giảm
  bằng `cfg(miri)`: 10.000 → 2×32 Frame, property 10.000 seed → 5×4) + Miri `--lib frame`
  **PASS 5/5** (2,50 s). 2 warning `integer-to-pointer cast` là của `vifast-objmem::obj` (crate
  được phép `unsafe`, D2-17) — **không** phải code KT-36.
- Test xanh: `cargo test -p vifast-sim --test kt36_frame` **12/12**; lib **39/39** (34 cũ + 5 âm).

### 4.1 Hồi quy cổng đã tự bắt trong lúc làm (ghi lại vì quan trọng)

Bản đầu tiên gọi `check_frames` trong `sched_after_op` mà **không** có cửa sổ nhanh: L1 1.000 seed
từ **832 ms** phồng lên **54.513 ms** (đo bởi cổng — sát ngưỡng 60 s của QD §5.2). Nguyên nhân:
mỗi public op quét + cấp phát `vec![0; #object]` dù mô hình **chưa từng** có Frame. Sửa bằng
`frame_objects` (số Frame đã từng tạo) + `if self.frame_objects == 0 { return Ok(()) }` ở đầu
`check_frames`; đo lại: **769 ms** (baseline 832 ms), tức về đúng mức trước KT-36. Không nới seed/
ngưỡng nào; bất biến Frame vẫn chạy đầy đủ sau mỗi op **khi** mô hình có Frame.

## 4.2 Review độc lập (trước commit) — **APPROVE**

`reviewer-kt36` (lead spawn) verdict **APPROVE code**: 12/12 + Miri 12/12 (reviewer tự chạy), PA
ảo/cửa sổ 1 GiB đúng, retype atomic, `HasChildren`, `check_frames`, 10k = 100×100 đạt, scope đúng.
Điều kiện/nit đã xử lý trước commit:
- **B1 (Agent 0)**: **D2-66** đã thêm + push (`9b70618`) — chốt `UNTYPED_PA_WINDOW = 1 GiB`,
  `add_untyped(base)` là đáy cửa sổ PA, `FrameMeta.pa` là nhãn ảo, bất biến bộ đếm KT-36. Code
  **không** phải sửa; HEAD trước commit là `9b70618` (đã `pull`, 0 commit ngoài `v2/` bị conflict).
- **B2 (hành chính)**: `STATE.md` + `kt36-miri.log` **đã có** trong cây (reviewer đọc trước khi
  Agent 2 ghi xong); dòng "lib 39/39" là **đúng** (34 test lib cũ + **5** unit test âm KT-36).
- **Nit 1**: `add_untyped` **từ chối ngay** khi `base + 1 GiB` tràn `u64` (assert rõ, như
  `size > 0`); `FrameArea::carve` giữ `InvalidArgument` làm lớp thứ hai cho `FrameArea` dựng trực tiếp.
- **Nit 2**: `k2-vm.md` §10.2 sửa thành "**Frame sống** không chồng `pa`" (không phải "Untyped
  không chồng cửa sổ") — đúng D2-66.
- **Nit 3**: thống nhất **7 nhóm** bất biến ở `check_frames` doc + `k2-vm.md` §10.5 + journal/STATE
  (khớp D2-66).

## 5. Không đổi

`spec/` (Frame vẫn `reserved`/`unsupported`), `v2/lib/vifast-model` (oracle), `.scn`,
QUEUE/DECISIONS/AGENTS, `viscript/`, `v2/bin/v2`; không `unsafe`, không dependency ngoài workspace,
không `#[ignore]`/`#[should_panic]`, không nới ngưỡng/seed nào của cổng.
