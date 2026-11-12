# KT-17 — Cổng Miri tổng `nightly --only-miri` + KT-04c trên objmem + chốt DoD K1a'

Người ghi: **Agent 2** dòng OS. Ngày: 2026-11-13 (Asia/Tokyo). HEAD vào: `64bb83a`
(KT-16 DONE `3db7ac7`; KT-17 READY). Working tree: `viscript/` có thay đổi của tab khác —
**không chạm**, chỉ stage `v2/`; trước push `git rev-list --count HEAD..origin/main` = 0.

Kết quả: **PASS** toàn bộ; reviewer `reviewer-kt17` verdict **DRIFT nhẹ — chỉ tài liệu/DoD**
(code + 2 follow-up đạt, không UNSAFE); đã sửa đủ 4 mục (STATE, JOURNAL, stage file, nit log đỏ).

## 1. Phạm vi (K1A-PLAN §4/KT-17 + QUEUE dòng KT-17)

1. Đóng **2 follow-up KT-16** (reviewer-kt16): `alloc_place_span` cap `align ≤ ARENA_ALIGN`;
   `Store::alloc` chốt hợp đồng `align > 4096`.
2. `./v2/bin/v2 nightly --only-miri` (mới): **chỉ** Miri trên tập chọn, không L1 100k/
   mutation/soak; không vào `gate --fast`; thiếu toolchain fail rõ như `v2 miri`.
3. Cổng Miri tổng chạy thật + log.
4. KT-04c trên objmem: `.scn` Untyped/CNode **9/9** + parity **10.000 seed**; xác nhận không đổi.
5. STATE: bảng DoD K1a' **2 dòng** + đánh dấu 2 follow-up đã đóng.

Không sửa: `vifast-model`, `spec/`, `QUEUE`, `DECISIONS`, `AGENTS*`, `viscript/`; không
`#[allow]`/`#[ignore]`; không nới ngưỡng; không chạy nightly đầy đủ.

## 2. Test đỏ trước (D2-05)

| Log | Nội dung |
|---|---|
| [`kt17-red-nightly.log`](kt17-red-nightly.log) | `./v2/bin/v2 nightly` cũ = stub: *"nightly CHƯA CÓ …"*, **exit 2** ⇒ chưa có cổng Miri tổng |
| [`kt17-red-align.log`](kt17-red-align.log) | **follow-up #1**: unit `owned::tests::alloc_place_span_tu_choi_align_lon_hon_arena_align` FAILED (`Some`, dòng 353); `kt17_align` FAILED: `retype(Untyped, size_bits=13)` trả **`Ok(2)`** thay vì từ chối (nguồn 8192 B đủ chỗ ⇒ trước KT-17 trả `pa` **có thể sai căn 8192**, vi phạm hợp đồng `Layout`) |

> Nit theo reviewer-kt17: unit test của `alloc_place_span` **không có log đỏ riêng** — nó nằm
> chung `kt17-red-align.log` (cùng lần chạy với test sim), đủ chứng minh hành vi đỏ.

## 3. Hiện thực

### 3.1 Follow-up #1 — cap căn lề (fail-closed)

- [`v2/lib/vifast-objmem/src/owned.rs`](../../lib/vifast-objmem/src/owned.rs):
  `OwnedMem::alloc_place_span` tính `align` hiệu dụng (max với căn body/header) rồi
  **`if align > ARENA_ALIGN { return None; }`** — không đốt byte, không trả `pa` sai căn;
  doc ghi rõ đây là hợp đồng của arena mô hình, không phải đổi ABI.
  Unit test `alloc_place_span_tu_choi_align_lon_hon_arena_align` (align 8192 ⇒ `None`,
  `used() == 0`; align **đúng biên 4096** ⇒ `Ok`, `pa % ARENA_ALIGN == 0`).
- [`v2/lib/vifast-objmem/src/arena.rs`](../../lib/vifast-objmem/src/arena.rs): doc `Arena::alloc`
  ghi rõ căn tính **tương đối theo `base`**; caller phải bảo đảm `base` căn ≥ `align`
  (`OwnedArena`/`OwnedMem` cấp vùng căn `ARENA_ALIGN` và chặn `> ARENA_ALIGN`).
- [`v2/sim/vifast-sim/src/kobj.rs`](../../sim/vifast-sim/src/kobj.rs): `Machine::retype` Untyped
  con có `len (= căn của block) > ARENA_ALIGN` (**`size_bits ≥ 13`**, 8192 B) ⇒
  **`Err(KError::Alignment)`** rõ ràng, trước khi cấp byte. Thứ tự kiểm giữ nguyên KT-09:
  nguồn 4096 B + `size_bits 13` vẫn `NotEnoughMemory` (test cũ không đổi).
- Test: [`v2/sim/vifast-sim/tests/kt17_align.rs`](../../sim/vifast-sim/tests/kt17_align.rs)
  - `retype_untyped_size_bits_13_bi_tu_choi_ro_va_khong_dot_byte` — 8192 ⇒ `Alignment`;
    1 MiB + `size_bits 20` (đủ chỗ, vẫn căn > 4096) ⇒ `Alignment`; 4096 + 13/30 ⇒
    `NotEnoughMemory`; `check_bytes` + `untyped_used == 0`.
  - `retype_untyped_size_bits_12_dung_bien_van_can_le_tuyet_doi` — **đối chứng**: 4096 B
    được nhận, `addr % 4096 == 0` (tuyệt đối và tương đối), watermark 4096, bất biến byte.

### 3.2 Follow-up #2 — hợp đồng `Store::alloc`

- [`v2/sim/vifast-sim/src/lib.rs`](../../sim/vifast-sim/src/lib.rs): doc **chốt hợp đồng**:
  `align` không phải lũy thừa 2 / `align = 0` / `align > ARENA_ALIGN` ⇒ `None` (không đốt
  byte, không trả địa chỉ sai căn); `pa` là địa chỉ **logic** `0x1000 + offset` giữ trace/golden.
- Test `store_alloc_chot_hop_dong_align_lon_hon_arena_align` (cùng tệp `kt17_align.rs`):
  `alloc(64, 8192)`, `alloc(64, 0)`, `alloc(64, 3)` ⇒ `None` + `used() == 0`; biên
  `alloc(64, 4096)` ⇒ `Some(0x1000)` căn 4096; sau khi cấp vẫn từ chối `8192`; `invariant()`.

### 3.3 `v2 nightly --only-miri` (D2-47(a))

- [`v2/bin/v2`](../../bin/v2): tách `require_miri_toolchain` dùng chung với `v2 miri`
  (nightly + `miri` + `rust-src` + `cargo +nightly miri --version`; thiếu ⇒ exit 127/2 +
  câu lệnh bootstrap — **fail rõ**, không im lặng); thêm `run_miri_test` và
  `do_nightly_only_miri`, dừng ngay bước đỏ (không retry, D2-07).
- Tập chọn: `vifast-objmem` (toàn bộ) + sim `--lib` + `kt16_objmem` + `kt09_untyped` +
  `kt09_revoke` (plan §4) + `kt10_cnode` + `kt17_align`. **Không** chạy `l1`/L1 100k,
  mutation, soak. `nightly` (không cờ) **vẫn exit 2**; `--only-miri` không nhận tham số thừa.
- Usage ghi rõ phần nào có/không có: nightly đầy đủ vẫn **CHƯA CÓ**.
- Không đưa vào `gate --fast` (Miri 365 s > ngân sách `--fast`).

### 3.4 Tệp thay đổi

`v2/bin/v2`, `v2/lib/vifast-objmem/src/{arena,owned}.rs`, `v2/sim/vifast-sim/src/{kobj,lib}.rs`,
`v2/sim/vifast-sim/tests/kt17_align.rs` (mới), `v2/agent/{STATE.md,JOURNAL/kt17-*}`.
**Không** đụng `viscript/**` (thay đổi của tab khác), `spec/`, `vifast-model`.

## 4. Bằng chứng xanh

| Hạng mục | Kết quả | Log |
|---|---|---|
| `nightly --only-miri` | **PASS 365 s, exit 0** — objmem 20+8+1+**1 fuzz 197,30 s**+10+0; sim `--lib` 13/13 (33,78 s); kt16 4/4; kt09_untyped 8/8; kt09_revoke 7/7; kt10_cnode 13/13; kt17_align 3/3 | [`kt17-nightly-only-miri.log`](kt17-nightly-only-miri.log) |
| `nightly` đầy đủ | vẫn **exit 2** (chỉ có `--only-miri`) | [`kt17-nightly-full-still-absent.log`](kt17-nightly-full-still-absent.log) |
| `.scn` Untyped/CNode (KT-04c) | **9/9** bước khớp; 3 bước setup chưa mô hình (Frame/K2); **0 lệch** | [`kt17-kt04c-scn-parity.log`](kt17-kt04c-scn-parity.log) |
| Parity 10.000 seed | **10.000 seed, 240.000 thao tác, 9.065 trace khác nhau, 0 lệch** (14.792 ms) | [`kt17-kt04c-scn-parity.log`](kt17-kt04c-scn-parity.log) |
| `gate --fast` | **PASS — 231 test/41 suite**, L1 1.000 seed/999 trace (517 ms), `scn: OK`, gen-check ON; **không retry** | [`kt17-gate-fast.log`](kt17-gate-fast.log) |
| `v2 miri` cũ | **PASS** 13/13 (`-p vifast-sim --lib`, 33,78 s) sau refactor | [`kt17-miri-old-cmd.log`](kt17-miri-old-cmd.log) |
| Test xanh follow-up | 1 unit objmem + 3 sim `kt17_align` | [`kt17-green-align.log`](kt17-green-align.log) |

`gate --fast` KT-16 là **227 test/40 suite**; KT-17 thêm **+4** test (3 `kt17_align` + 1 unit
objmem) ⇒ **231 test/41 suite** (số đo từ chính log cổng, không viết tay).

R1..R5/SPDX: `check`/`gate` xanh (rules PASS); `nightly --only-miri` **không** thêm crate:
`vifast-objmem` vẫn **0 dependency ngoài**, `v2/sim/**` **0 `unsafe`** (R5).

## 5. Hai follow-up KT-16 — trạng thái

| # | Nội dung reviewer-kt16 | Cách đóng | Bằng chứng |
|---|---|---|---|
| 1 | `alloc_place_span` chưa chặn `align > ARENA_ALIGN` ⇒ Untyped `size_bits ≥ 13` xin `Layout` căn > căn thật của vùng | cap/assert trong `alloc_place_span` (`None`, fail-closed) + `Machine::retype` từ chối rõ `Alignment` trước khi cấp; đối chứng biên 4096 vẫn căn đúng | unit objmem + `retype_untyped_size_bits_13…` / `…12_dung_bien…` (đỏ → xanh) |
| 2 | `Store::alloc` thu hẹp hợp đồng `align > 4096 → None` nhưng chưa có test/đối chứng | doc chốt hợp đồng + test đối chứng (0/không-lũy-thừa-2/>4096 ⇒ `None`, biên 4096 nhận) | `store_alloc_chot_hop_dong_align_lon_hon_arena_align` |

**Cả hai đã đóng** — không còn việc phải làm trong STATE.

## 6. Hạn chế / rủi ro đã biết (không giấu)

- **Model vs sim (có chủ ý)**: `vifast-model` (oracle) vẫn nhận Untyped con `size_bits 12..=30`;
  sim **cap 4096** ⇒ `size_bits ≥ 13` trả `Alignment`. Parity 10.000 seed chỉ sinh
  `size_bits = 12` (generator `kt11_diff_sim.rs`), nên **0 lệch**. Nếu K1b cần Untyped con
  > 4 KiB, cần task riêng: cấp vùng arena căn theo cỡ vùng (hoặc căn tuyệt đối) — **không**
  làm trong KT-17 (giữ trace/watermark tất định và phạm vi review).
- **Đường `OwnedMem::alloc_place<T>` (`object_layout`)**: chưa cap `align_of::<T>() >
  ARENA_ALIGN`; mô hình hiện **không có** kiểu như vậy (`Kind` căn 8) nên không kích hoạt.
  Đã thử thêm cap + test `#[repr(align(8192))]` nhưng **revert** để diff khớp đúng phạm vi
  reviewer đã duyệt; ghi ở đây làm follow-up tiềm năng nếu thêm kiểu căn lớn.
- **Miri warnings**: `integer-to-pointer cast` (2 warning/lần chạy) là **có sẵn từ KT-15/16**,
  không phải UB; không đặt `-Zmiri-strict-provenance` (KT-16 đã ghi).
- **`nightly --only-miri` 365 s** (fuzz objmem 197 s dưới Miri) ⇒ **không** chạy song song
  build/cổng khác (AGENTS §6), **không** vào `gate --fast`.
- `nightly` đầy đủ (L1 100k/mutation/soak) **vẫn chưa có** — đúng phạm vi K1a'.

## 7. Đề xuất

`KT-17` **DONE** sau verdict reviewer (DRIFT nhẹ đã sửa) + Agent 0 lật QUEUE.
**K1a' DONE** nếu Agent 0 chấp nhận bảng DoD 2 dòng trong STATE (Miri từ commit đầu + KT-04c
trên mô hình), không có YC mới. Không đề xuất mở K1b trong task này.
