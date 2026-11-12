# KT-16 — tích hợp `vifast-objmem` vào sim (2026-11-13)

- **HEAD vào**: `b15d480` ([KT-00] lật KT-15 DONE, mở KT-16); KT-16 = READY.
- **Commit**: `3db7ac7` `[KT-16] objmem vào sim: Store + body Machine trong ObjMem
  (227 test, parity 10k, Miri)` — đã push `origin/main` (fast-forward `aae08d4..3db7ac7`).
- **Review độc lập**: `reviewer-kt16` **APPROVE** (không DRIFT/UNSAFE); tự chạy gate 227 test,
  L1 1000/999, Miri kt16 4/4, bare-metal, xác nhận test cũ 0 dòng sửa, body thật trong objmem.
  Hai follow-up không chặn commit đã ghi ở §6 (chuyển KT-17).
- **Ngân sách**: bắt đầu 05:14, kết thúc mã ~06:00 (≈46′ < 3 h) ⇒ **không** phải tách
  `KT-16a`/`KT-16b` (D2-47c cho phép nhưng không cần).
- **Phạm vi**: thay **hai** store giả bằng `vifast-objmem`:
  1. `vifast-sim::Store` (bump dùng bởi `Prog::Alloc`);
  2. ruột `kobj::Machine` — object body nằm trong `ObjMem`, watermark = `Arena::used()`.
     Bảng `Cap` **vẫn** là `Vec<Cap>` (D2-47d); `vifast-model` **không** đụng.
- **Không chạm**: `spec/`, `QUEUE`, `DECISIONS`, `AGENTS*`, `viscript/`, `vifast-model`.
  Không thêm dependency ngoài; `v2/sim/**` **không** có `unsafe` (R5 xanh).

## 1. Thiết kế (D2-14, D2-47d, D2-48, KT-15 reviewer)

1. **Vùng nhớ do objmem sở hữu** (`vifast-objmem/src/owned.rs`, feature `heap`, chỉ bật
   cho sim — bare-metal build không thấy module này):
   - `OwnedArena`: cấp thô bằng `alloc::alloc::alloc(Layout)` căn **4096**, **không**
     `Vec<u8>` (QD §3.5); tầng trên chỉ thấy `base()`/`used()`/`capacity()`/
     `alloc_offset(size, align)`.
   - `OwnedMem`: vùng căn lề + `ObjMem`; gói `place`/`destroy`/`reset` (unsafe trong
     objmem) thành API an toàn: `alloc_place`, `alloc_place_span`, `free`, `reset` — nên
     `v2/sim/**` không cần một dòng `unsafe` nào.
   - `ObjMem::new_with_id(base, len, id_base)` + `OwnedMem::with_id_base`: `id` **duy nhất
     toàn cụm** khi một `Machine` có nhiều store (mỗi Untyped một store, `id_base = (i+1)<<40`).
2. **`Machine`**: mỗi Untyped một `OwnedMem` (cấp đúng `size` của `add_untyped`, hoặc đúng
   `len` của Untyped con); object Retype nằm **ngay trong** arena của Untyped chứa nó.
   Arena kernel (`OwnedMem` 1 MiB [ƯL], `Machine::with_arena_bytes`) chứa object harness
   cấp + body Untyped gốc. `Meta` (mô hình: `ty/alive/untyped/offset/len/region/slots`) +
   `ObjRef<Kind>` thay `Vec<Object>`; object chết vẫn giữ metadata vì `object_type`/
   `object_bytes` của `kt11_diff_sim` băm **cả** object đã Revoke.
3. **Watermark = `Arena::used()`**: `retype` cấp block đúng `(len, align)` của spec qua
   `alloc_place_span`, nên `offset`/`untyped_used`/`gap` của mô hình giữ **nguyên byte**
   (không còn `align_up` viết tay); `Revoke` Untyped finalise body rồi `reset` arena của
   **cả cây** (con/cháu), nên `lam tiep duoc` đúng thiết kế §4.2.
4. **Body phải vừa object nhỏ nhất 64 B**: `Kind` gom mọi kiểu object, các variant lớn
   (`TcbData`, `SchedData`, `EndpointState`, `ReplyData`) nằm sau `Box`; assert
   `size_of::<Kind>() + 16 ≤ 64` **lúc biên dịch** (nếu không, block 64 B sẽ tràn).
5. **Guard `chua_header` cho cả `with`/`with_mut`** (reviewer KT-15 yêu cầu): ref có `pa`
   ngoài arena trả `Dead` **trước khi** đọc header — hai store độc lập có thể trùng `id`
   (cùng đếm từ 1) nên guard là bắt buộc; test đỏ trước ở §2.
6. **Bảng `kind → ctor` giữ nguyên, đo cỡ = 8.192 B (= 8 KiB, trần 8 KiB)** — test
   `bang_kind_giu_nguyen_va_khong_phinh_qua_8_kib`.
7. **`vifast-sim::Store`**: bump/watermark/căn lề do `OwnedArena` của objmem; địa chỉ trả
   về vẫn là **địa chỉ logic** `0x1000 + offset` để trace/golden tất định (vùng thật do
   objmem cấp). `arena_base()`/`arena_used()` mới để test chứng minh tích hợp.

## 2. Test đỏ trước (D2-05)

- **`tests/kt16_objmem.rs` (mới, 4 test)** — viết trước khi đổi ruột:
  `cargo test -p vifast-sim --test kt16_objmem` **đỏ** với **10 × E0599**
  (thiếu `Machine::with_arena_bytes`/`arena_capacity`/`untyped_arena`/`object_addr`/
  `with_object`, `Store::arena_base`/`arena_used`) — log `kt16-red-kt16-objmem.log`.
- **Guard `chua_header` cho `with`/`with_mut`**: test `with_va_with_mut_chan_ref_cua_store_khac`
  đỏ khi bỏ guard: `left: Ok(7)` / `right: Err(Dead)` (`kt16-red-guard.log`), xanh sau khi
  khôi phục (`kt16-green-guard.log`).

## 3. Bằng chứng xanh

| Mục | Kết quả | Log |
|---|---|---|
| `./v2/bin/v2 gate --fast` | **PASS** — 40 suite / **227 test**, luật R1..R5 OK, gen-check, `.scn` 21 tệp/11 nhóm, **L1 1.000 seed → 999 trace (622 ms)** | `kt16-gate-fast.log` |
| Test cũ 0 dòng sửa | `git diff --stat v2/sim/vifast-sim/tests/kt09* kt10* kt11*` **rỗng**; 69 test sim xanh | `kt16-test-green.log` |
| Parity `vifast-model` | **10.000 seed / 0 lệch** (chạy trong L0 của gate, `kt11_diff_model_va_sim_10000_seed ... ok`) | `kt16-gate-fast.log` |
| `.scn` | `scn-check: PASS (21 tệp, 11 nhóm = 9 nhóm QD §4 + notification + exit)`, fixtures 5/5 | `kt16-gate-fast.log` |
| Miri `vifast-objmem` | **PASS** — 19 lib + 8 alias + 1 compile-fail + 1 fuzz + 10 contract | `kt16-miri-objmem.log` |
| Miri `kt16_objmem` | **PASS** — 4/4 (0,61 s) | `kt16-miri-sim-kt16.log` |
| Miri `kt09_untyped` + `kt10_cnode` | **PASS** — 8/8 (2,29 s) + 13/13 (3,14 s) | `kt16-miri-sim-kt09-kt10.log` |
| Bare-metal (feature off) | `cargo build -p vifast-objmem --target aarch64-unknown-none` **xanh** | `kt16-aarch64-none.log` |
| Bảng kind | **8.192 B** = 8 KiB (không phình) | test trong gate |

Ghi chú: Miri in cảnh báo *integer-to-pointer cast* (`obj.rs:166/175`) — **có từ KT-14**
(theo thiết kế `PAddr` số nguyên + `ptr::with_exposed_provenance`), không phải lỗi mới;
`v2 miri` không đặt `-Zmiri-strict-provenance`.

## 4. Rủi ro / khác biệt có chủ ý

- `Store::alloc` giới hạn `align ≤ ARENA_ALIGN = 4096` (đáy vùng chỉ bảo đảm căn 4096;
  mô hình dùng 16/32/64). `align` lớn hơn trả `None` — ghi trong doc.
- `Store`/`Harness` **bỏ `Clone`**: vùng nhớ sở hữu + arena không clone được; không test
  nào dùng `Clone`. API công khai còn lại giữ nguyên (thêm `arena_base`/`arena_used`).
- Untyped con có arena **riêng** đúng cỡ vùng cắt; quan hệ cha/con là **logic**
  (offset/cỡ), không phải containment vật lý — chi tiết harness sim, không phải ABI.
- `Meta.slots` (số slot CNode) giữ để `cap_at`/`cnode_slot_count` trên CNode **đã
  finalise** vẫn trả `SlotEmpty` như API cũ (test `kt10_cnode` yêu cầu; body đã
  `drop_in_place` nên không đọc ngược được).
- `Machine` có `Drop`: finalise body còn sống trước khi arena giải phóng — Miri không rò
  `Box`/`Vec` bên trong `Kind`.

## 5. Đề xuất

`KT-16` **DONE** sau khi reviewer độc lập xác nhận; YC mới: không. `KT-17` (cổng Miri
tổng + chốt DoD K1a') giữ nguyên phạm vi.

## 6. Follow-up cho KT-17 (reviewer-kt16 nêu, **không chặn** commit)

1. **`alloc_place_span` chưa chặn `align > ARENA_ALIGN`** (`vifast-objmem/src/owned.rs`):
   `alloc_offset` đã trả `None` khi `align > ARENA_ALIGN` (4096), nhưng đường Retype Untyped
   dùng `alloc_place_span(kind, len, align)` với `align = len`; `size_bits ≥ 13` (len ≥ 8192)
   sẽ xin `Layout` căn lớn hơn căn thật của vùng (4096). Hiện **chưa UB** (Arena chỉ căn
   tương đối theo cursor; offset vẫn ≥ 0 và nằm trong vùng; địa chỉ tuyệt đối có thể không
   đúng căn `len` như hợp đồng `Layout`), nhưng vi phạm hợp đồng API. KT-17: cap/assert
   `align ≤ ARENA_ALIGN` (hoặc cấp vùng con cho Untyped lớn hơn) **hoặc** sửa `Arena::alloc`
   để căn theo địa chỉ tuyệt đối.
2. **`Store::alloc` thu hẹp hợp đồng `align > 4096` → `None`** (Store cũ nhận mọi lũy thừa 2,
   căn từ base 0x1000): ghi rõ hợp đồng mới trong doc (đã ghi) **hoặc** mở rộng
   `ARENA_ALIGN`/cho `OwnedArena` cấp vùng căn lớn hơn ở KT-17. Mô hình hiện chỉ dùng 16/32/64.
