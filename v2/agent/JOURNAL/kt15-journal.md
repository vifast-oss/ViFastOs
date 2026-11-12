# KT-15 — `with_many_mut` + alias từng cặp + fuzz (2026-11-13)

- **HEAD vào**: `d391628` (KT-14 DONE, KT-15 READY); YC-18 đã chốt **D2-48 (a)**.
- **Phạm vi**: bảng tĩnh `kind → constructor` trong `vifast-objmem`, `with_many_mut` thật
  (kiểm alias **từng cặp** trước khi tạo ref), trần `N ≤ 8` **lúc biên dịch**, ma trận
  alias, fuzz tất định, Miri toàn bộ test alias + bản rút gọn fuzz.
- **Không chạm**: `spec/`, `QUEUE`, `DECISIONS`, `AGENTS*`, `viscript/`, `v2/sim/`,
  `vifast-model`.

## 1. Quyết định thiết kế (theo D2-48 (a), giữ `ObjRef` 16 B)

1. **Bảng cố định `kind → constructor`** (`src/many.rs`): `ObjMem.kinds` = 256 ô
   `Option<Entry>` (`no_std`, không cấp phát); `Entry { ctor: unsafe fn(PAddr) -> *mut dyn KObj, size }`.
   `place::<T>` là nơi **duy nhất** biết `T` nên đăng ký `ctor::<T>` + `size_of::<T>()`
   vào ô `v.kind()`. Truy cập tra theo `header.kind` — **không** viết tay vtable: vtable do
   compiler sinh từ `impl KObj for T` khi `*mut T` được unsize thành `*mut dyn KObj`.
   Hai kiểu dùng chung `kind` là lỗi người dùng (ô bị ghi đè), đã ghi trong doc.
2. **`ObjRef::into_dyn()`** (mới, `obj.rs`): `ObjRef<dyn KObj>` không mang vtable nên phép
   chuyển chỉ đổi kiểu tĩnh, vẫn 16 B. Cần thiết vì `ObjRef::new` là `pub(crate)` —
   không có nó thì **người ngoài crate không dựng được** `[ObjRef<dyn KObj>; N]` để gọi
   `with_many_mut`. Có `as_dyn()` đi kèm.
3. **`KObj::as_any_mut()`** (mới, default `None`): closure của `with_many_mut` chỉ nhận
   `[&mut dyn KObj; N]` (chữ ký thiết kế `:207-208`), nên không có đường nào ghi field
   cụ thể mà không mở một hook xuống kiểu cụ thể. Default `None` giữ nguyên mọi impl
   KT-14; object nào cần ghi kiểu cụ thể override `Some(self)` để `downcast_mut::<T>()`.
   Đây là API **mở rộng** trong cùng crate (không đổi layout/ABI, không đổi chữ ký
   `ObjStore`); nếu Agent 0 muốn tên/ý nghĩa khác thì thay tên, cơ chế không đổi.
4. **Thứ tự kiểm trong `with_many_mut`** (mọi bước **trước** khi tạo tham chiếu):
   `const { assert!(N <= 8) }` → ref sống (`header.id == r.id && state == LIVE`) + kind có
   trong bảng → kiểm từng cặp (trùng `pa`, hoặc vùng `[pa, pa + 16 + size_of::<T>())`
   chồng lấn) → mới dựng `[&mut dyn KObj; N]` từ con trỏ thô. Mọi đầu vào không an toàn
   (ref chết, kind lạ, tràn `u64`, chồng vùng) trả **`Err(Alias)` fail-closed**, closure
   **không** được gọi — chữ ký thiết kế chỉ có một kiểu lỗi `Alias`, nên không thêm
   biến thể lỗi (không đổi ABI).
5. **Không cần `UnsafeCell`**: vùng Untyped nằm ngoài `ObjMem`, mọi truy cập đi qua con
   trỏ dựng từ `pa`; `&mut self` đã loại trừ `with`/`with_mut` đồng thời. Ghi rõ trong
   `lib.rs` + `many.rs` (thay ghi chú "sẽ dùng `UnsafeCell`" của KT-14).
6. **Không `#[allow]`, không `#[ignore]`; không nới ngưỡng** (10.000 chuỗi stable,
   200 chuỗi Miri, `N ≤ 8`).

## 2. Test đỏ trước (bằng chứng, trung thực)

- `kt15-red-alias.log` — `tests/alias.rs` viết **trước** khi cài đặt, chạy trên khung
  KT-14: **3 FAILED** (`n1_den_n8_ref_roi_nhau_tra_ok_va_ghi_duoc_tung_object`,
  `ba_kind_khac_nhau_trong_mot_lan_goi_ok`, `goi_lien_tiep_ghi_don_dung_tren_tung_object`:
  `left: Err(Alias)`, `right: Ok(..)`) — đúng vì khung KT-14 luôn fail-closed.
- **Trung thực**: 3 test còn lại (`trung_pa...`, `moi_ref_deu_trung_nhau...`,
  `ref_da_destroy...`) **xanh ngay** trên khung KT-14 vì khung đã kiểm trùng `pa` và luôn
  `Err(Alias)`; chúng vẫn được giữ làm hồi quy, không phải bằng chứng đỏ.
- `kt15-red-n9.log` — chứng minh **chính dòng `const { assert!(N <= 8) }`** là thứ chặn:
  (1) bản sao tạm bỏ đúng dòng assert → `rustc` biên dịch `N = 9` **exit 0**;
  (2) bản hiện tại → `error[E0080]: evaluation panicked: with_many_mut: N <= 8`, exit 1.
  (`--emit=metadata` **không** đủ: lỗi post-mono chỉ lộ khi sinh mã ⇒ dùng `--emit=obj`.)

## 3. Thay đổi so với KT-14 (không nới ngưỡng)

- `src/store.rs`: `with_many_mut` của `ObjMem` → `many::with_many_mut`; `ObjMem` thêm
  trường `kinds`; `place` đăng ký constructor; doc trait nói rõ fail-closed.
- Test KT-14 `with_many_mut_khung_fail_closed_tra_alias` (assert **khung** luôn `Alias`)
  → đổi tên/thân thành `with_many_mut_hai_ref_roi_nhau_tra_ok` (hai ref rời nhau phải
  `Ok`, closure được gọi; cùng `pa` vẫn `Alias`). Đây là assert về **khung tạm** đã bị
  KT-15 thay theo đúng kế hoạch (KT-14 ghi rõ "phần đầy đủ thuộc KT-15"), **không** phải
  sửa test để né đỏ; ma trận đầy đủ hơn nằm ở `tests/alias.rs`.
- `src/obj.rs`: thêm `into_dyn`/`as_dyn`, `KObj::as_any_mut`; test 16 B giữ nguyên.

## 4. Fuzz

- `tests/fuzz.rs`: 10.000 chuỗi stable × 32 thao tác (`place/destroy/with/with_mut/with_many_mut`),
  RNG xorshift64* tự viết (**không** thêm dependency — D2-21). Bất biến sau **mỗi** bước:
  (1) không hai object sống chồng lấn; (2) `arena.used()` khớp mô hình block độc lập
  (tính từ `size_of`/`align_of` + luật căn lề: `align ≤ 16` căn `max(align, 8)` block
  `16 + size`; `align > 16` căn `align` block `align + size`); (3) mọi ref sống `Ok` đúng
  giá trị mô hình; (4) mọi ref đã destroy `Dead`.
- **Không dùng `reset`** trong fuzz: sau `Revoke`/`reset` vùng bị cấp lại nên byte cũ có
  thể *tình cờ* trông như header LIVE (đã gặp đúng ca này khi thử: body `C16 { a=5, b=1 }`
  đọc thành `kind=5, state=0, id=1`). Bất biến "ref destroy ⇒ `Dead`" chỉ đúng khi vùng
  chưa bị dùng lại — đúng như doc `Arena::reset` ("mọi `ObjRef` cũ trở nên vô hiệu").
  `reset` không nằm trong danh sách thao tác DoD; ca reset+id không tái dùng vẫn được
  kiểm riêng ở unit test `many.rs::tests::pa_dung_lai_sau_reset_van_bat_duoc_ref_cu`.
- Miri: **200** chuỗi (rút gọn theo DoD), chạy **191,95 s** < 20 phút ⇒ **giữ 200**,
  không phải giảm thêm; 10.000 chuỗi stable không đổi.

## 5. Bằng chứng (log cùng thư mục)

| Tệp | Nội dung | Kết quả |
|---|---|---|
| `kt15-baseline-gate.log` | `gate --fast` trước khi sửa | PASS (199 test, L1 1.000 seed) |
| `kt15-baseline-miri.log` | `v2 miri -p vifast-objmem` trước khi sửa | PASS 15/15, exit 0 |
| `kt15-red-alias.log` | `tests/alias.rs` viết trước, chạy trên khung KT-14 | **3 FAILED**, 3 passed (lý do ở §2) |
| `kt15-red-n9.log` | N=9: bỏ assert ⇒ build (exit 0); có assert ⇒ E0080 | **đỏ đúng lý do** |
| `kt15-test-green.log` | `cargo test -p vifast-objmem` (stable) | lib **14**, alias **8**, compile_fail **1**, fuzz **1**, contract **10** — 0 failed |
| `kt15-fuzz-stable.log` | riêng fuzz 10.000 chuỗi + thời gian | PASS, 4,51 s (test), 4,63 s wall |
| `kt15-miri.log` | `v2 miri -p vifast-objmem` (toàn crate) | PASS: lib 14, **alias 8**, compile_fail 1 (bỏ spawn có lý do), **fuzz 200 (191,95 s)**, contract 10; exit 0 |
| `kt15-aarch64-none.log` | `cargo build --target aarch64-unknown-none` | PASS (`no_std`) |
| `kt15-gate-fast.log` | `./v2/bin/v2 gate --fast` cuối task | §6 |

- Miri còn **8 cảnh báo `integer-to-pointer cast`** — cùng lớp đã ghi nhận ở KT-14 (do
  `PAddr = u64` theo thiết kế); **không tắt**, không `#[allow]`. Không có lỗi bộ nhớ.
- `compile_fail.rs` dưới Miri **bỏ qua có lý do**: Miri cấm spawn tiến trình, và đây là
  test về **biên dịch** (không phải bộ nhớ); toàn bộ test alias/fuzz vẫn chạy đủ dưới Miri.

## 6. Cổng cuối task

- `./v2/bin/v2 gate --fast`: **PASS** — **218 test**, L1 1.000 seed/999 trace, 596 ms đo cổng;
  gen-check ON, 21 `.scn` + golden không đổi; không retry (D2-07). Log: `kt15-gate-fast.log`.

## 7. Review độc lập (`reviewer-kt15`, lead spawn) — DRIFT nhẹ, **không UNSAFE**

Reviewer tự chạy lại test + gate + Miri alias/lib, soi soundness bảng `kind → ctor`, `Alias`
cho ref chết/kind lạ, compile-fail spawn rustc. Đã sửa **toàn bộ** mục bắt buộc + các mục
"nên làm":

1. **Bỏ claim sai "fail-closed"** ở `store.rs` (`Alias`, `with_many_mut`), `many.rs` đầu tệp:
   `ObjRef` 16 B **không mang identity store** ⇒ ref của store A có thể lọt sang B nếu trùng
   `id`/`kind`/LIVE. Đã đổi thành **hợp đồng** trên `place`/`reset` ("ref chỉ dùng với store
   sinh ra nó; `reset` làm ref cũ vô hiệu") + ghi rõ `with`/`with_mut` trả `Dead`,
   `with_many_mut` trả `Alias` cho **cùng điều kiện**; phần kiểm được giữ nguyên (id lệch,
   state ≠ LIVE, kind lạ, `pa` ngoài arena, trùng `pa`/`id`, chồng vùng).
2. **`STATE.md`**: số 598 ms không có trong log → sửa theo log cuối (**596 ms**, gate 218 test).
3. **Comment sai ở test `pa + 8`**: header tại `pa + 8` **có thể** khớp `id`+LIVE; test cũ bị
   chặn bởi kind chưa đăng ký, không phải `chong_lan`. Đã sửa comment (chỉ khẳng định
   fail-closed) và **thêm test end-to-end** `chong_vung_that_su_bat_boi_kiem_tung_cap`: body
   của `Gia` (16 B, layout y hệt `ObjHeader`) chứa `{kind 33 đã đăng ký, LIVE, id = 777}`,
   ref giả `(pa + 16, 777)` **qua được** kiểm sống + tra bảng ⇒ chỉ `chong_lan` chặn được.
4. **Guard `ObjMem::chua_header` + `Arena::chua`** (mục "nên làm"): `pa` phải nằm trọn header
   16 B trong arena và đúng căn lề **trước** `ptr::read`; `pa` giả/ngoài vùng ⇒ `Alias` thay vì
   đọc ngoài. Kèm test `ref_cua_store_khac_bi_chan_boi_guard_arena` (ref store A sang store B
   bị chặn, store A không bị đụng). Ghi chú: guard **chưa** áp cho `with`/`with_mut` — KT-16.
5. **`KindTable::register` giữ `TypeId` + `debug_assert`** (mục "nên làm"): hai kiểu khác nhau
   dùng chung `kind` panic ở build debug thay vì âm thầm dựng sai vtable.
6. **Thêm test**: trùng `id` khác `pa` với vùng rời ⇒ `Alias` (`kiem_alias`, id không tái
   dùng); `zst_cung_object_thuong_trong_mot_lan_goi_ok` (nhánh `size_of::<T>() == 0`);
   `ba_kind_khac_nhau_ma_co_ref_trung_tra_alias` (mảng trộn 3 kind có ref trùng ⇒ `Alias`).
7. Sau sửa: `cargo test -p vifast-objmem` **34 test** 0 failed; Miri lại toàn crate **PASS**
   (fuzz 200/191,95 s); `gate --fast` **PASS 218 test**; `aarch64-none` xanh; fuzz stable 4,51 s.

## 8. Ghi chú cho KT-16 (không chặn KT-15)

- **Bảng `kind → ctor`**: 256 ô × ~32 B ≈ **8 KiB** trong `ObjMem` (TypeId chỉ để
  `debug_assert`). Spec `[object]` lõi chỉ 1..8 + reserved 9..31, nên KT-16 có thể thu
  nhỏ bảng (ví dụ 32 ô) nếu cần ngân sách; hiện giữ 256 để `header.kind: u8` không bị
  giới hạn ngầm.
- **Guard `chua_header`** mới áp cho `with_many_mut`; `with`/`with_mut` (KT-14) chưa có —
  KT-16 nên áp cùng guard khi thay store của sim.
- **Identity store/cụm**: `ObjRef` 16 B vẫn không mang identity; `with_many_mut` chặn được
  `pa` ngoài arena nhưng ref của store khác nằm trong cùng vùng nhớ là trách nhiệm người
  gọi. KT-16 phải bảo đảm id duy nhất toàn cụm (hoặc thêm identity) trước khi dùng chéo.
- **`KObj::as_any_mut`**: object kernel muốn ghi qua `with_many_mut` phải override
  `Some(self)`; nếu K1b thiết kế API object cụ thể (TCB/Endpoint) thì có thể thay bằng
  phương thức chuyên biệt.

