# KT-253 — K4' bước 3, **B3.4**: nửa cao (`TTBR1_EL1`) + physmap + wrapper `Mmu` định tuyến

**Ngày**: 2026-11-21 · **Sub-agent** `a64b` · nhánh `task/kt253-a64b` · base `main` @ `60f082b9`
**Hàng QUEUE**: `KT-253` (D2-200) · **Quyết định**: **D2-199** chốt Q4 = **(c)** — nửa cao/physmap
làm **trong crate arch**, **không** đổi trait `PageTableFormat`/`Mmu` của `vifast-pte`.
**Phạm vi**: **chỉ B3.4**. Không `VSpace` (B3.5 — KT-254), không TLBI shootdown (B3.6), không
U-mode (B3.7). **Không tuyên bố K4' đạt.**

---

## 1. Đỏ trước (D2-05)

Ca kiểm viết **trước** khi `split_mmu` tồn tại — log [`kt253-red.log`](kt253-red.log):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test split_mmu
error[E0432]: unresolved import `vifast_plat_aarch64::split_mmu`
  --> vifast-plat-aarch64/tests/split_mmu.rs:33:26
   |
33 | use vifast_plat_aarch64::split_mmu::{
   |                          ^^^^^^^^^ could not find `split_mmu` in `vifast_plat_aarch64`
error: could not compile `vifast-plat-aarch64` (test "split_mmu") due to 1 previous error
```

**Sau khi sửa** — 16 test host mới (log [`kt253-mmu.log`](kt253-mmu.log) §1):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test split_mmu
running 16 tests
test duplicate_map_is_already_mapped ... ok
test high_alias_maps_image_at_high_base ... ok
test high_alias_va_math ... ok
test high_va_routes_to_high_table ... ok
test low_va_routes_to_low_table ... ok
test non_canonical_va_is_rejected ... ok
test physmap_lives_in_high_table ... ok
test physmap_maps_any_pa_in_window ... ok
test physmap_unmap_and_va_math ... ok
test roots_are_separate_and_ttbr_bases_are_level1 ... ok
test split_mmu_implements_mmu_trait ... ok
test translate_returns_pa_for_both_halves ... ok
test unmap_frees_only_own_half ... ok
test unmap_one_half_does_not_affect_other ... ok
test va_layout_constants_are_canonical ... ok
test walk_routes_by_half ... ok
test result: ok. 16 passed; 0 failed; 0 ignored
```

Toàn crate: **94 ca host** (trước KT-253: 78) — 24 `host` + 10 `mmu_boot` + 13 `pl011` +
**16 `split_mmu`** + 19 `sysreg` + 4 `tlb_barrier` + 8 `vector`.

## 2. Đã làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/split_mmu.rs` | **mới** — `HIGH_HALF_BASE`/`PHYSMAP_BASE`/`KERNEL_HIGH_BASE`, `Half`, `half_of` (định tuyến **thuần**), `physmap_va`, `high_alias_va`, `map_physmap`, `map_high_alias`, **`SplitMmu`** (hai `PageTable` + `impl Mmu` định tuyến) |
| `v2/kernel/vifast-plat-aarch64/src/boot_mmu.rs` | `MmuStep::WriteTtbr1` + chuỗi **6 bước** (`TCR→MAIR→TTBR0→TTBR1→SCTLR→ISB`); `level1_base(table, va)` dùng chung; `ttbr1_base`; `apply_step`/`enable_mmu` nhận thêm `ttbr1` |
| `v2/kernel/vifast-plat-aarch64/src/bootmem.rs` | **`SharedBootMemory`** + `shared_boot_memory()` — hai handle **chia sẻ** một arena + một con trỏ bump (`static SHARED_NEXT: AtomicUsize`); không `unsafe` |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | `jump_high(entry, sp)` (đặt `sp` rồi `br`, thanh ghi cố định `x9`/`x10`) + `current_pc()` (`adr x, .`) + stub host **panic** |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | khai báo `split_mmu` + re-export |
| `v2/kernel/vifast-plat-aarch64/tests/split_mmu.rs` | **mới** — 16 ca host (ca kiểm đỏ-trước) |
| `v2/kernel/vifast-plat-aarch64/tests/mmu_boot.rs` | `mmu_enable_sequence_order` 5 → **6** bước (thêm `WriteTtbr1` trước `SCTLR`) |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | `mmu_bringup` dựng **hai** bảng (identity nửa thấp + alias/physmap nửa cao), bật MMU 6 bước, chuẩn bị bước nhảy; **`high_entry`** chạy ở VA nửa cao: đo PC thật + physmap, cài lại `VBAR_EL1` bằng VA nửa cao, phát TAP 6/7 |
| `v2/test/boot/run-aarch64.sh` | `EXPECT_TESTS` mặc định 5 → **7**; ca âm (d) dùng `$EXPECT_TESTS` thay vì hard-code `1..5` |

### Bốn quyết định kỹ thuật (dễ sai, đã kiểm bằng test)

**(a) Hai `PageTable` phải **chia sẻ** một bộ đếm frame.** `PageTable::new` cấp frame gốc từ
`TableMemory`; nếu hai bảng có hai bộ đếm riêng thì **cả hai** bắt đầu từ frame 0 và ghi đè lên
nhau — bảng trang hỏng ngay sau frame đầu. `SharedBootMemory` giữ `next: &'static AtomicUsize`
(một `static`), host test dùng `Rc<Cell<usize>>` — cùng một hợp đồng.

**(b) `TTBR1_EL1` cũng phải trỏ **bảng mức 1**, không phải gốc mức 0.** `T1SZ = 25` ⇒ walk nửa
cao bắt đầu ở mức 1 (bảng D5-5), y như `T0SZ`. `level1_base(table, va)` đọc entry mức 0 tại chỉ
số của `va` — `0` cho nửa thấp, `0x1FF` cho nửa cao — nên **một** hàm phục vụ cả hai nửa, không
có bản thứ hai của logic.

**(c) Ranh giới định tuyến là `0xFFFF_FFC0_0000_0000`, không phải `1 << 63`.** Với VA 39-bit
canonical, mọi VA canonical đều `< 2^38` hoặc `>= 0xFFFF_FFC0_…`; dùng `HIGH_HALF_BASE` phân
nửa **chính xác** và không có VA canonical nào rơi vào lỗ giữa hai nửa (test
`va_layout_constants_are_canonical` chốt `half_of(HIGH_HALF_BASE - 0x1000) == None`).

**(d) Sau khi nhảy, `adrp` là PC-relative ⇒ `addr_of!(symbol)` trả **alias nửa cao**, không phải
PA liên kết.** Đây là phát hiện thật khi đo lần đầu: `# high: kmain … match=0` vì
`addr_of!(__text_start)` trả `0xffffffc080000000` (alias) thay vì `0x40080000` (PA). Sửa: đọc PA
liên kết **trước** khi nhảy, lưu vào `static TEXT_START_PA`/`VECTOR_BASE_PA`, `high_entry` đọc
lại. Hệ quả **tốt**: truy cập `static` trong `high_entry` đi qua `TTBR1` (PC-relative ⇒ VA nửa
cao) — nên nửa cao không chỉ phục vụ instruction fetch mà cả dữ liệu tĩnh.

## 3. Đo được (nguyên văn)

Log đầy đủ: [`kt253-mmu.log`](kt253-mmu.log). Serial của ảnh:

```text
# mmu: arena=0x4008a000 frames=32 used=14 root_low=0x4008a000 root_high=0x4008b000 ttbr0=0x4008c000 ttbr1=0x40093000 level=1
# mmu: high alias=0x40080000-0x400bb000 base=0xffffffc080000000 pages=59 physmap_base=0xffffffc000000000 pages_physmap=59
# mmu: step 4/6 ttbr1 begin
# mmu: step 4/6 ttbr1 ok
# high: kmain va=0xffffffc080000ae8 pa=0x40080ae8 match=1 translated=0x40080000
# high: entry va=0xffffffc0800007d8 pa=0x400807d8 translated=0x40080000 sp=0xffffffc08003b000
# physmap: probe pa=0x40087000 va=0xffffffc040087000 magic=0x504859534d415001
# high: jump begin entry=0xffffffc0800007d8 sp=0xffffffc08003b000
# high: jump ok
# high: va=0xffffffc08000085c pa=0x4008085c match=1 expect_va=0xffffffc0800007d8 expect_pa=0x400807d8 translated=0x40080000 same_page=1
# physmap: pa=0x40087000 read=0x504859534d415001 match=1 va=0xffffffc040087000 expect=0x504859534d415001 base=0xffffffc000000000
# high: vbar=0xffffffc080003000 high=0xffffffc080003000 match=1
ok 6 - high: PC at high VA, translate matches PA
ok 7 - physmap: read PA via physmap window
# viFastOs aarch64: boot ok
```

`qg_boot rc=0`, `marker_count=1`, không `PANIC`, `# trap: other=0`.

| Phép đo | Kết quả | Ý nghĩa |
|---|---|---|
| **PC thật ở nửa cao** | `va=0xffffffc08000085c` (đọc bằng `adr`), `same_page=1` | instruction fetch **thật sự** đi qua `TTBR1` — không phải in hằng số |
| **PA của PC** | `pa=0x4008085c` = `text_start + (pc - KERNEL_HIGH_BASE)` | alias nửa cao trỏ đúng PA của ảnh |
| **walk phần mềm** | `translated=0x40080000` = đáy trang của `expect_pa` | bảng nửa cao dịch đúng (so **đáy trang** vì descriptor chỉ giữ bit [47:12]) |
| **physmap đọc PA** | `pa=0x40087000 read=0x504859534d415001 match=1` | đọc PA đã ghi trước đó **qua cửa sổ physmap**, không cần identity |
| **VBAR nửa cao** | `vbar=0xffffffc080003000 match=1` | từ đây cả exception vector chạy ở nửa cao |
| **frame đã dùng** | `used=14` / `frames=32` | nửa thấp 8 + nửa cao 6 (gốc, mức 1, 2 bảng mức 2, 2 bảng mức 3) |
| **TAP** | 7/7, runner **7/7** | không nới ngưỡng: `EXPECT_TESTS` tăng vì ảnh **thêm** 2 ca thật |

## 4. Ba đột biến bắt buộc — log [`kt253-mutation.log`](kt253-mutation.log)

| # | Đột biến | Tầng host | Tầng QEMU |
|---|---|---|---|
| **(i)** | `TCR.T1SZ` 25 → 24 | **ĐỎ**: `tcr_kernel_value` (`left: 24`) | **TREO CÓ KIỂM SOÁT**: `rc=124`, log dừng đúng sau `# high: jump begin` |
| **(ii)** | `TTBR1_EL1` ← bảng nửa thấp | không quan sát được (đột biến trong ảnh) | **TREO CÓ KIỂM SOÁT**: `rc=124`, cùng vị trí |
| **(iii)** | bỏ map physmap | không quan sát được (đột biến trong ảnh) | **ĐỎ quan sát được**: `# trap: ec=0x25 far=0xffffffc040087000`, runner **6/7 EXIT=1** |

**Đọc đúng ba dòng trên (không tô hồng):**

- **(i) và (ii) treo, không in `# trap:`** — cơ chế: VA nửa cao không dịch được ⇒ instruction
  abort tại đích nhảy; `vec_common` đẩy khung 176 B lên **SP nửa cao**, mà SP ấy cũng không dịch
  được ⇒ data abort lồng nhau ⇒ vòng fault vô hạn. Đây là **giới hạn thật của đường boot hiện
  tại** (chưa có stack riêng cho handler), không phải "test yếu": vị trí dừng trong log chỉ đúng
  bước và runner phân biệt được (thiếu TAP 6/7 + marker).
- **(iii) in được `# trap:`** vì fault xảy ra **sau** khi PC/SP nửa cao đã chạy đúng và `VBAR_EL1`
  vẫn là VA thấp (identity) — handler chạy được, in `ec=0x25` (Data Abort from same EL) và
  `far` = đúng VA physmap. Runner đỏ `6/7 EXIT=1`.
- **(ii) không có tầng host** vì đột biến nằm trong **ảnh** (`main.rs`), không trong crate; ghi rõ
  thay vì tạo test giả.

## 5. Còn thiếu so với B3.4 / rủi ro

1. **Chưa liên kết toàn ảnh ở VMA nửa cao.** Ảnh vẫn liên kết ở PA `0x4008_0000`; sau khi nhảy,
   **instruction fetch + stack + `static`** đi qua `TTBR1` (PC-relative), nhưng **MMIO** (UART/GIC
   dùng hằng tuyệt đối `0x0900_0000`/`0x0800_0000`) vẫn đi qua `TTBR0` identity. Liên kết
   VMA nửa cao + LMA thấp (trampoline `_start` vị trí độc lập) là bước riêng — **không** thuộc
   tiêu chí B3.4 ("kernel chạy nửa cao" = PC/SP/vector), nhưng phải ghi rõ.
2. **`TTBR0_EL1` chưa "để trống cho user"** — vẫn giữ identity map cho MMIO + đường lui của
   handler. Bỏ nó cần chuyển MMIO sang nửa cao trước (cùng việc với (1)).
3. **Chưa `TLBI`** sau `unmap` (B3.6) — `SplitMmu::unmap` chỉ sửa bảng, không xoá TLB. Trên QEMU
   TCG điều này chưa lộ vì chưa có `unmap` nào trên đường boot.
4. **Handler chưa có stack riêng** ⇒ fault khi SP hiện tại không map là treo (mục 4). Đề xuất:
   B3.6/B3.8 thêm stack handler ở `.bss` (VA thấp, identity) để mọi fault in được `# trap:`.
5. **Arena 32 frame**: đo dùng **14**; còn 18 frame headroom. `VSpace` (KT-254) cấp thêm bảng
   ⇒ phải theo dõi `used`; cạn ⇒ `alloc_frame` trả `None` ⇒ boot `PANIC` (fail-loud).
6. **`free_frame` vẫn no-op** trên arena boot — `SplitMmu::unmap` "giải phóng" bảng trung gian
   nhưng arena không thu hồi; ghi rõ, không tính là rò rỉ ngầm.

## 6. Bất biến đã giữ

| # | Bất biến | Cách kiểm |
|---|---|---|
| I1 | Không `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`/`vifast-pte` | `v2 check` xanh (luật cấu trúc) |
| I2 | `unsafe` chỉ trong `vifast-plat-aarch64::asm` | `#![deny(unsafe_code)]` ở gốc crate; `jump_high`/`current_pc` nằm trong `asm` |
| I3 | Đường riscv64 không đổi | không chạm tệp riscv64; `v2 check` xanh |
| I4 | `PageTableFormat`/`Mmu` **không** đổi chữ ký (D2-199) | `git diff v2/lib/vifast-pte` **rỗng** |
| I7 | `.data + .bss ≤ 256 KiB` | arena 32 frame giữ nguyên; ảnh mới `.bss` 140 KiB (đo lại ở KT-254) |
| I9 | Mỗi bước có đột biến làm đỏ | mục 4 |
