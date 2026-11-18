# KT-160 — K4' bước 1: `vifast-plat-aarch64` phần host-testable

**Ngày**: 2026-11-19 · **Agent 2 (L4)** · nhánh `night2/L4-aarch64` · HEAD trước task: `6b2fdce7`
**Quyết định**: D2-147 (Agent 1 quyết thay Agent 0) · hàng QUEUE: `KT-160`
**Phạm vi**: **chỉ bước 1** — crate + `PageTableFormat`/`Mmu` **thuần dữ liệu** + test host.
**KHÔNG** boot aarch64, **KHÔNG** QEMU, **KHÔNG** tuyên bố K4' đạt.

## 0. Việc được giao (D2-147, nguyên văn)

> **D2-147** **K4' bước 1: `vifast-plat-aarch64` chỉ phần host-testable** (Agent 1 quyết thay
> Agent 0). K4' là milestone lớn; đêm nay chỉ mở **bước 1** có tiêu chí đo được: crate
> `v2/kernel/vifast-plat-aarch64` với `PageTableFormat` 4 mức 4 KiB (39-bit VA, 48-bit PA), `Mmu`
> **thuần dữ liệu** (không asm), test host cho map/unmap/quyền/không rò rỉ frame; **cấm** thêm
> `cfg(target_arch)` vào `vifast-core`/runtime (AGENTS §4.8) — crate plat chọn theo arch như
> `vifast-plat-riscv64`. **Không** boot aarch64 đêm nay, **không** tuyên bố K4' đạt. Ngân sách 3 h.
> Task **KT-160**.

## 1. Kết luận trước (đọc nhanh)

- **ĐẠT bước 1**: crate `v2/kernel/vifast-plat-aarch64` (3 tệp `src/` + 1 tệp test, 1.882 dòng,
  `#![no_std]`, `#![forbid(unsafe_code)]`, **0 dependency**) cài `PageTableFormat` cho AArch64
  4 KiB granule 4 mức (39-bit VA canonical, 48-bit PA) và `Mmu` dữ liệu map/unmap/translate/walk/
  destroy trên nguồn frame trừu tượng.
- **ĐẠT test host**: `22 passed; 0 failed` — descriptor từng bit, walk **đủ 4 mức**, quyền
  R/W/X + user/kernel, 7 ca âm, **không rò rỉ frame** (`alloc − free` = 1 sau unmap hết, = 0 sau
  `destroy`), format thứ hai 3 mức chạy cùng `Mmu`, format 48-bit VA dùng lại đúng mã hoá.
- **ĐẠT đột biến**: bỏ `AF` ⇒ 2 test đỏ; `unmap` không `free_frame` ⇒ 4 test đỏ; đảo `AP[2]` ⇒
  3 test đỏ (log §4.2–§4.4). Khôi phục ⇒ xanh lại (§4.1).
- **ĐẠT L0**: `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common` vẫn xanh
  (115 passed, 0 failed — §4.5).
- **ĐẠT cổng nhánh**: `./v2/bin/v2 check` **PASS**, `EXIT=0` (§4.6).
- **Cross-check**: `aarch64-unknown-none` **có** trong `rustup target list --installed`; crate
  build xanh cho target thật (§4.7) — nhưng **không** boot, **không** chạy.
- **KHÔNG tuyên bố K4' đạt**: chưa có `TTBR*`/`MAIR`/`TCR`/`TLBI`/asm, chưa boot, chưa QEMU.
- **Lệch đề bài (ghi rõ)**: trait `PageTableFormat`/`Mmu` **không tồn tại** trong
  `vifast-plat-common`/`vifast-pte` tại `6b2fdce7` — xem §2. Đã định nghĩa **cục bộ** trong crate
  mới; **không** sửa `vifast-pte`/`vifast-plat-common` để không đụng L0.

## 2. Phát hiện lệch đề bài: trait `PageTableFormat`/`Mmu` **không tồn tại**

Đề bài viết "Đọc `vifast-plat-common/src/mmu.rs` (trait `PageTableFormat`/`Mmu`)". Đo trên cây
`6b2fdce7`:

```text
$ grep -rn "PageTableFormat" --include=*.rs v2/
(không có kết quả)
$ grep -rn "trait Mmu\|trait PageTable" --include=*.rs v2/kernel/
(không có kết quả)
```

- `v2/kernel/vifast-plat-common/src/mmu.rs` (470 dòng) là `PageTableBuilder` **Sv39 cụ thể**
  (dùng `vifast_pte::{walk_shape, Pte, PAGE_SIZE_BITS, PTE_LEVELS, VA_BITS}`), không có trait.
- `v2/lib/vifast-pte` (402 dòng) là hình **Sv39 3 mức** (`PTE_LEVELS = 3`, `VA_BITS = 39`),
  không có trait.
- `PageTableFormat` chỉ xuất hiện trong **tài liệu thiết kế** (§183, §358, §1101) với tư cách
  *kế hoạch*: "Logic copy nửa cao nằm trong `PageTableFormat` của vifast-pte, dùng chung".

**Quyết định của L4** (không hỏi được lead trong hạn, không phải chỗ cần dừng theo luật task —
luật chỉ bắt dừng khi cần `unsafe`):

1. Định nghĩa trait `PageTableFormat` + trait `Mmu` **tại crate mới** (`src/format.rs`,
   `src/mmu.rs`), đúng chữ ký tinh thần của thiết kế §183 (`type Fmt: PageTableFormat`).
2. **Không** sửa `vifast-pte`/`vifast-plat-common`: thêm trait vào đó là thay đổi crate dùng
   chung, ngoài phạm vi D2-147 và có nguy cơ đụng L0 — đúng luật "không phá L0".
3. Ghi **nợ có tên** (§6, N1): bước 2 chuyển trait lên `vifast-pte` theo thiết kế §358/§1101,
   crate này chỉ còn impl.

Hệ quả: "cài `PageTableFormat`" ở bước 1 là **định nghĩa + cài** trong cùng crate; giá trị đo
được không nằm ở chỗ "có trait" mà ở **test**: descriptor đúng bit, walk đủ 4 mức, quyền, đếm
frame, và **format thứ hai** (3 mức, tự chế) chạy qua **cùng** `Mmu` — chứng minh `Mmu` không
hardcode AArch64.

## 3. Crate mới: cấu trúc + quyết định thiết kế

```text
v2/kernel/vifast-plat-aarch64/
├── Cargo.toml            (SPDX, license MIT OR Apache-2.0, 0 dependency)
├── src/lib.rs            (46 dòng — `#![no_std]`, `#![forbid(unsafe_code)]`, re-export)
├── src/format.rs         (485 dòng — PageTableFormat, Aarch64, descriptor, MAIR, Perms)
├── src/mmu.rs            (510 dòng — TableMemory, PageTable, Mmu, MapError, Walk)
└── tests/host.rs         (841 dòng — 22 test host)
```

`v2/kernel/Cargo.toml`: thêm `"vifast-plat-aarch64"` vào `members` (giữ nguyên thứ tự alphabet
với các member khác); `v2/kernel/Cargo.lock` thêm đúng một package.

### 3.1 Hình học (D2-147)

| Tham số | Giá trị | Ghi chú |
|---|---|---|
| granule | 4 KiB (12 bit offset) | `AARCH64_PAGE_BITS = 12` |
| số mức | 4 (0 = gốc, 3 = lá) | `AARCH64_LEVELS = 4` |
| entry/bảng | 512 × 8 B = 4 KiB | `AARCH64_ENTRIES = 512` |
| VA | **39 bit canonical** | `AARCH64_VA_BITS = 39` (D2-147) |
| VA tối đa của format | 48 bit | `AARCH64_MAX_VA_BITS = 48` (thiết kế §541) |
| PA | 48 bit | `AARCH64_PA_BITS = 48` |

### 3.2 Vì sao 39-bit VA mà vẫn **4 mức** (chỗ dễ sai nhất)

AArch64 4 KiB granule 4 mức là **48-bit VA** đầy đủ; thiết kế §541 ghi "4 KiB granule, 48 bit,
TTBR0/1". D2-147 chốt kernel dùng **39-bit canonical** (giống hình Sv39 của riscv64, và giống
cách x86_64 dùng 4 cấp với VA canonical 39-bit ở K5'). Hai cách hiểu:

- (a) `TCR_EL1.T0SZ = 25` ⇒ phần cứng **bắt đầu walk ở mức 1**, mức 0 không dùng;
- (b) `T0SZ = 16` (48-bit) nhưng kernel chỉ cấp **VA canonical 39-bit** ⇒ walk vẫn đi **đủ 4
  mức**, chỉ số mức 0 chỉ có hai giá trị `0` (nửa thấp) hoặc `0x1FF` (nửa cao).

Chọn **(b)**: giữ đúng chữ "4 mức" của D2-147, giữ khả năng 48-bit của format (test
`format_48_bit_va`), và không mất mức nào trong walk. Hệ quả đo được: `map(0x1000)` đi
`root[0] → L1[0] → L2[0] → L3[1]`; `map(0xFFFF_FFC0_0000_0000)` đi `root[511] → L1[256] → L2[0]
→ L3[0]`; `map(0xFFFF_FFFF_FFFF_F000)` đi `[511, 511, 511, 511]` — test chốt từng chỉ số.
`TTBR0/TTBR1` thật (hai gốc riêng) là việc **bước 2**; bước 1 mô hình **một** gốc, nửa cao dùng
`root[511]` — ghi rõ để không ai đọc nhầm là đã có TTBR1.

### 3.3 Descriptor 64-bit (ARMv8-A, stage 1)

Bit đã cài + test từng bit (`descriptor_bits_aarch64`):

| Bit | Tên | Cách đặt |
|---|---|---|
| 0 | `V` | luôn 1 với entry hợp lệ |
| 1 | page/table | 1 với table (mức 0..2) và page (mức 3); 0 = block |
| 4:2 | `AttrIndx` | `MemAttr::Device = 0`, `MemAttr::Normal = 1`; `MAIR_EL1 = 0x00 \| 0xFF << 8` |
| 5 | `NS` | 0 (bước 1 không dùng Secure world) |
| 6 | `AP[1]` | 1 ⇔ leaf user (EL0) |
| 7 | `AP[2]` | 1 ⇔ chỉ đọc |
| 9:8 | `SH[1:0]` | `NonShareable = 0b00`, `Outer = 0b10`, `Inner = 0b11`; `0b01` reserved |
| 10 | `AF` | **luôn 1** với leaf (thiếu ⇒ fault mọi truy cập) |
| 11 | `nG` | 1 ⇔ leaf user (bài học v1 P0-1/r289: thiếu `nG` thì `TLBI ASIDE1` không xoá) |
| 47:12 | PA | 48 bit |
| 53 | `PXN` | 1 trừ khi (exec && kernel) |
| 54 | `UXN` | 1 trừ khi (exec && user) |

`Perms` là kiểu **độc lập arch** (`read/write/exec/user`). AArch64 **không có** bit `R` riêng
(`AP[2]` chọn RW/RO, `AP[1]` chọn EL0/EL1) ⇒ `read = false` **không biểu diễn được**;
`Perms::is_valid()` đòi `read = true`, map trả `InvalidPerms` thay vì lặng lẽ cấp quyền đọc.

### 3.4 `Mmu` dữ liệu + `TableMemory`

- `TableMemory` là **ranh giới duy nhất** với phần cứng: `alloc_frame`/`free_frame`/
  `read_entry`/`write_entry`/`zero_frame`. Host test cài bằng `Vec` frame 4 KiB; bước 2 cài bằng
  physmap — **không đổi một dòng logic**.
- `PageTable<F: PageTableFormat, M: TableMemory>`: `new` (cấp + zero gốc), `map`/`map_with`,
  `unmap`, `translate`, `walk` (chẩn đoán từng bước), `destroy` (trả lại `M` để kiểm đếm).
- `Mmu` trait (`type Fmt: PageTableFormat`) cài cho `PageTable`; test gọi **qua trait** với format
  thứ hai.

### 3.5 Không rò rỉ frame — bất biến và cách đo

- `unmap` xoá lá rồi đi **từ dưới lên**, giải phóng bảng rỗng, **dừng** ở bảng còn entry; gốc
  không bao giờ bị `unmap` giải phóng.
- `destroy` giải phóng **cả cây** kể cả gốc, trả lại nguồn frame.
- Test đếm: sau `new` = 1 (gốc); map 1 lá = 4 (gốc + 3 bảng); `unmap` = 1; `destroy` = 0;
  `alloc − free` sau unmap hết = 1, sau destroy = 0; lặp `n = 1..=64` (§4.1).
- **Frame cấp lại phải zero**: `TestMemory::free_frame` bơm `0xDEAD_BEEF_DEAD_BEEF` vào frame;
  `alloc_frame` **không** zero (đúng hợp đồng); test `stale_frame_is_zeroed` map lại trên frame
  rác ⇒ nếu `Mmu` quên `zero_frame` thì đỏ.
- **Không cấp phát khi từ chối**: mọi kiểm tra (căn lề, canonical, dải PA, quyền) chạy trước
  walk; 6 ca âm so `(allocs, frees)` trước/sau ⇒ không đổi.

## 4. Bằng chứng (dán nguyên văn dòng log)

Log đầy đủ ở `/tmp/kt160-logs/` (không commit — máy tạm). Lệnh chạy trong
`/tmp/night2-L4-aarch64`, `export PATH="$HOME/.cargo/bin:$PATH"`.

### 4.1 Test host — XANH

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 22 tests
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

22 test: `descriptor_bits_aarch64`, `mair_index_matches_programmed_mair`,
`permissions_roundtrip`, `map_translate_4_levels`, `map_high_half_uses_root_511`,
`format_48_bit_va`, `map_unaligned_rejected`, `map_non_canonical_rejected`,
`map_pa_out_of_range_rejected`, `map_overlap_rejected`, `map_invalid_perms_rejected`,
`unmap_unmapped_rejected`, `block_descriptor_rejected`, `unmap_frees_frames_no_leak`,
`unmap_512_pages_frees_all_tables`, `frame_accounting_alloc_eq_free_after_unmap`,
`destroy_frees_everything`, `stale_frame_is_zeroed`, `out_of_frames`,
`mmu_generic_over_second_format`, `walk_reports_invalid_and_canonical`,
`translation_is_copy_and_eq`.

### 4.2 Đột biến 1 — bỏ `AF` khỏi `encode_leaf` ⇒ **ĐỎ**

Sửa: `let mut d = DESC_VALID | DESC_TABLE_OR_PAGE | DESC_AF;` → bỏ `| DESC_AF`.

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
test descriptor_bits_aarch64 ... FAILED
test permissions_roundtrip ... FAILED
test result: FAILED. 20 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### 4.3 Đột biến 2 — `unmap` **không** giải phóng frame ⇒ **ĐỎ**

Sửa: bỏ dòng `self.memory.free_frame(child);` trong vòng giải phóng bảng rỗng của `unmap`.

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
test frame_accounting_alloc_eq_free_after_unmap ... FAILED
test mmu_generic_over_second_format ... FAILED
test unmap_frees_frames_no_leak ... FAILED
test unmap_512_pages_frees_all_tables ... FAILED
test result: FAILED. 18 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

### 4.4 Đột biến 3 (thêm) — đảo `AP[2]` (RW ↔ RO) ⇒ **ĐỎ**

Sửa: `if !perms.write { d |= DESC_AP_RO; }` → `if perms.write { ... }`.

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
test descriptor_bits_aarch64 ... FAILED
test map_translate_4_levels ... FAILED
test permissions_roundtrip ... FAILED
test result: FAILED. 19 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

Cả ba đột biến đã **khôi phục** bằng `cp` từ bản gốc; chạy lại §4.1 xanh (không retry để lấy
xanh — đột biến chạy **một lần**, khôi phục rồi chạy lại là kiểm "đã trả về đúng", không phải
retry cổng).

### 4.5 L0 `vifast-plat-common` — vẫn XANH

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 14 tests
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 1 test
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Tổng: **115 passed, 0 failed** (12 test binary; dòng cuối là doc-test `PanicHandler` — 1 ignored
có sẵn từ trước, không phải test của task này).

### 4.6 `./v2/bin/v2 check` — PASS

```text
$ ./v2/bin/v2 check
--- clippy: OK
--- luật cấu trúc R1..R5
--- luật cấu trúc: OK
...
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
EXIT=0
```

### 4.7 Cross-check `aarch64-unknown-none` — build XANH (không boot)

```text
$ rustup target list --installed | grep aarch64-unknown-none
aarch64-unknown-none
aarch64-unknown-none-softfloat
$ cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target aarch64-unknown-none
   Compiling vifast-plat-aarch64 v0.1.0 (.../v2/kernel/vifast-plat-aarch64)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.20s
```

Target **có sẵn** trên máy (không cài mạng). Đây chỉ là **biên dịch** `no_std` cho target thật —
**không** boot, **không** chạy, **không** phải bằng chứng K4'.

## 5. Tệp thay đổi

| Tệp | Loại |
|---|---|
| `v2/kernel/vifast-plat-aarch64/Cargo.toml` | mới |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | mới |
| `v2/kernel/vifast-plat-aarch64/src/format.rs` | mới |
| `v2/kernel/vifast-plat-aarch64/src/mmu.rs` | mới |
| `v2/kernel/vifast-plat-aarch64/tests/host.rs` | mới |
| `v2/kernel/Cargo.toml` | +1 dòng `members` |
| `v2/kernel/Cargo.lock` | +4 dòng package mới |
| `v2/agent/JOURNAL/kt160-aarch64.md` | mới (tệp này) |

Không chạm `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
`docs/v1/`, `vifast-core`, `vifast-plat-common`, `vifast-pte`, `v2/bin/v2`.

## 6. Còn thiếu / nợ có tên (bước 2 của K4' phải đóng)

- **N1 — trait chưa ở `vifast-pte`**: `PageTableFormat`/`Mmu` đang định nghĩa cục bộ trong crate
  này; thiết kế §358/§1101 đặt chúng ở `vifast-pte` "dùng chung". Bước 2 chuyển lên, crate này
  chỉ còn impl; khi đó riscv64 cũng cài lại `PageTableBuilder` Sv39 theo trait để chứng minh
  "dùng chung" thật.
- **N2 — test crate mới chưa nằm trong `gate --fast`**: L0 kernel hiện chỉ chạy
  `cargo test -p vifast-plat-common` (KT-148). Crate mới là member nhưng test **không** được cổng
  gọi ⇒ hồi quy trong crate sẽ không bị bắt. Đề xuất lead nối `-p vifast-plat-aarch64` vào L0
  (một dòng trong `v2/bin/v2`) — L4 **không tự sửa cổng** (ngoài phạm vi D2-147/D2-148).
- **N3 — chưa có phần cứng**: không `TTBR0_EL1`/`TTBR1_EL1`, không `MAIR_EL1`/`TCR_EL1`, không
  `TLBI`/`dsb`/`isb`, không `SCTLR_EL1.M`, không boot, không QEMU, không linker script aarch64.
  Bước 1 **cố ý** dừng ở dữ liệu.
- **N4 — chưa có block 2 MiB/1 GiB**: `decode` nhận diện block (và test chốt `BlockUnsupported`/
  `AlreadyMapped`/`Reserved`), nhưng `map` chỉ tạo lá 4 KiB. Physmap aarch64 sẽ cần block — bước 2.
- **N5 — chưa có W^X cứng**: `Perms::KERNEL_RWX`/`USER_RW` vẫn map được (giống riscv64 hiện tại);
  chính sách W^X đầy đủ là việc của tầng object/VSpace, không phải format.
- **N6 — chưa có ASID/nG ở tầng TLB**: `nG` đã đặt đúng cho leaf user (test), nhưng `TLBI ASIDE1`
  và ASID allocator là bước 2.
- **N7 — chưa đo `.data+.bss`**: crate chỉ dữ liệu, không có `static` lớn; trần 256 KiB đo ở bước
  boot.

## 7. Việc **không** làm (đúng luật)

- Không boot aarch64, không QEMU, không `pkill -f qemu-system` (task không cần QEMU).
- Không `gate --fast` (D2-148: chỉ lead chạy sau merge).
- Không merge, không push, không tạo worktree mới.
- Không `cfg(target_arch)` ở bất kỳ đâu; không `unsafe` (crate `#![forbid(unsafe_code)]`).
- Không `#[ignore]`/`#[should_panic]`; không nới ngưỡng; không retry để lấy xanh.
- Không sửa `spec/`, ba tài liệu cấm, `docs/v1/`.
