# KT-305 — `translate`/`unmap` cho **block descriptor** (aarch64)

Trạng thái: **DONE** — nhánh `task/kt305-blkunmap`, base `main` @ `58a1f867`, 2026-11-21.
Sub-agent `blkunmap`. Quyết định: **D2-218** (vòng 17 — KT-302 block descriptor), **D2-195**
(mở K4' bước 3), **D2-05** (test đỏ trước), **D2-198** (uỷ quyền commit nhánh task), **D2-17**
(`unsafe` chỉ trong crate arch), **D2-199** (không đổi `Mmu` cho nửa cao — KT-305 **không** đổi
`Mmu`, chỉ thêm hàm **có mặc định** vào `PageTableFormat`).

**Không** tuyên bố K4' đạt: K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000). KT-305 chỉ đóng
nửa còn lại của block descriptor (KT-302 mới map **một chiều**).

## 1. Khảo sát trước (bắt buộc — ghi lại *trước* khi sửa)

### 1.1 `translate`/`unmap` gặp block trả gì (đo tại `58a1f867`)

| Hàm | Gặp block | Vị trí |
|---|---|---|
| `PageTable::translate` | `Err(MapError::BlockUnsupported)` | `vifast-pte/src/page_table.rs:456` |
| `PageTable::unmap` (mức giữa) | `Err(MapError::BlockUnsupported)` | `page_table.rs:386` |
| `PageTable::unmap` (mức lá) | `Err(MapError::BlockUnsupported)` | `page_table.rs:407` |
| `PageTable::walk` (chẩn đoán) | `WalkOutcome::Block { pa, level }` | `page_table.rs:500` |

Vậy mapping **một chiều**: `map_block` (KT-302) tạo được block, `walk` thấy được, nhưng
`translate`/`unmap` **không** dùng được. Ảnh aarch64 vì thế phải đo block bằng `walk_steps`
(`main.rs:942` ghi rõ "translate cố ý từ chối block").

### 1.2 Test nào chốt hành vi cũ

| Test | Tệp | Chốt điều gì |
|---|---|---|
| `block_descriptor_rejected` | `vifast-plat-aarch64/tests/host.rs:677` | translate/unmap trên block mức 1 tự chế ⇒ `BlockUnsupported`; block mức 0 ⇒ `CorruptTable` |
| `map_block_2m_covers_256mib_with_two_frames` | `host.rs:709` | 128 block ⇒ 2 frame; translate/unmap ⇒ `BlockUnsupported` |
| `block_and_reserved_entries_are_rejected` | `vifast-plat-x86_64/tests/host.rs` | block `PS` của x86_64 ⇒ translate/unmap `BlockUnsupported` |

### 1.3 Chọn phương án: **(A) hỗ trợ đầy đủ** — có **cổng năng lực** theo format

Lý do chọn (A) (và vì sao ≤ 2 h):

1. **Walk đã giải mã block từ KT-302** (`Descriptor::Block`), nên phần thiếu chỉ là hai nhánh
   `translate`/`unmap` + một hàm chia nhỏ — không phải viết lại walk.
2. (B) để lại mapping **một chiều** cho MMIO: muốn bỏ **một** trang ECAM phải `destroy` cả cây
   (mất toàn bộ mapping khác) — đó là giới hạn thật, không phải "phong cách".
3. **Cổng năng lực** là điều kiện để làm (A) trong crate **dùng chung** mà không đổi hành vi
   x86_64/Sv39: thêm `PageTableFormat::block_span(level) -> Option<u64>` **mặc định `None`**;
   AArch64 khai mức 1/2. Format không khai ⇒ `translate`/`unmap` vẫn trả `BlockUnsupported`
   **y như trước**, không phải sửa một dòng nào (test x86_64 `block_and_reserved_entries_are_rejected`
   vẫn xanh nguyên văn).

**Test cũ: ĐỔI, không xoá.** `block_descriptor_rejected` → `block_descriptor_translate_and_unmap`
(chuyển sang `tests/block.rs`): nay chốt `translate` trả PA + `level = 1` + quyền giải mã, và
`unmap` **chia nhỏ** (1 GiB → 2 bảng) rồi bỏ một lá; phần "block mức 0 ⇒ `CorruptTable`" **giữ
nguyên**. Hai dòng `BlockUnsupported` trong `map_block_2m_covers_256mib_with_two_frames` đổi
thành `translate` trả `pa = PA_BASE + 3·BLOCK`, `level = 2`. Ca âm "format **không** hỗ trợ
block" được bù bằng test **mới** `block_translate_unmap_unsupported_format_rejected` (format
giải mã block nhưng `block_span` mặc định `None`).

## 2. Thay đổi

| Tệp | Thay đổi |
|---|---|
| `v2/lib/vifast-pte/src/mmu.rs` | `Descriptor::Block` **thêm `perms`** (block mang cùng bộ bit quyền với lá); trait `PageTableFormat` thêm **`block_span(level) -> Option<u64>`** (mặc định `None`) và **`split_block(entry, level, index) -> Option<u64>`** (mặc định `None`) |
| `v2/lib/vifast-pte/src/page_table.rs` | `translate` gặp block ⇒ `Translation { pa: đáy + (va & (span−1)), perms, level }` (kiểm căn lề block, lệch ⇒ `CorruptTable`); `unmap` gặp block ⇒ **`split_block_once`** chia **một** mức rồi đi tiếp (1 GiB → 2 MiB → lá), bỏ **một** lá; `map_block` lấy cỡ/căn từ `block_span` (**một** nguồn hình học); `walk` chỉ đổi pattern `{ .. }` |
| `v2/kernel/vifast-plat-aarch64/src/format.rs` | `Aarch64::block_span` (mức 1 = 1 GiB, mức 2 = 2 MiB); `Aarch64::split_block` (mức 1 → 512 block 2 MiB, mức 2 → 512 lá 4 KiB; **giải mã** quyền/attr/share từ descriptor rồi mã hoá lại, không chép bit lạ); `decode` trả `Block { pa, level, perms }` |
| `v2/kernel/vifast-plat-x86_64/src/format.rs` | `decode` trả `Block { pa, level, perms }` (cơ học, do enum dùng chung đổi) — **không** khai `block_span` ⇒ hành vi giữ nguyên |
| `v2/kernel/vifast-plat-common/src/mmu.rs` | `decode` Sv39 trả `Block { pa, level, perms }` (cơ học) — **không** khai `block_span` ⇒ hành vi giữ nguyên |
| `v2/kernel/vifast-kbin-aarch64/src/ecam.rs` | **mới** — tự kiểm `translate`/`unmap` block trên bảng trang **thật** của ảnh (block **cuối** dải ECAM để bằng chứng block đầu của KT-302 vẫn đúng) + in `# pci: unmap … blocks_split=1 …` |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | gọi `ecam::probe` sau dòng `# pci: map` (giữ `frames_used=33` của dòng ấy); `mod ecam;` |
| `v2/kernel/vifast-plat-aarch64/tests/block.rs` | **mới** — 15 ca: 5 ca KT-302 (chuyển nguyên) + 10 ca KT-305 (9 mới + 1 đổi từ `block_descriptor_rejected`) |
| `v2/kernel/vifast-plat-aarch64/tests/common/mod.rs` | **mới** — `TestMemory` + hằng số dùng chung cho `host.rs`/`block.rs` (R1: không tệp nào > 1.500 dòng; **một** nguồn, không bản sao) |
| `v2/kernel/vifast-plat-aarch64/tests/host.rs` | bỏ phần block (sang `block.rs`), dùng `mod common;` — 23 ca còn lại |
| `v2/kernel/vifast-plat-x86_64/tests/host.rs` | hai `assert_eq!` của `descriptor_ps_bit_by_level` thêm `perms: Perms::KERNEL_RX` (enum đổi) |
| `v2/test/boot/run-aarch64.sh` | `check_pci` đòi dòng `# pci: unmap … level=2 translate_ok=1 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1`; **+1 ca âm** (x) `blocks_split=0` (23 → **24 ca âm**) |
| `v2/test/boot/soak-aarch64.sh` | `check_boot_log` đòi cùng dòng; fixture thêm dòng `# pci: unmap …`; `--selftest` **+1 ca âm** `blocks_split=0` (39 → **40 ca**) |
| `v2/docs/k4-aarch64-boot.md` §10.2/§10.3 | viết lại đoạn "translate/unmap giữ nguyên `BlockUnsupported`" (đã lỗi thời) + thêm dòng log mới |

**Không** đổi: `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, trần I7, arena
(`BOOT_ARENA_FRAMES = 44`), TAP (35), `Mmu` trait (không thêm thao tác — D2-199 giữ nguyên).

## 3. Đo

### 3.1 Test host — **38 ca** (23 `host.rs` + 15 `block.rs`), 0 đỏ

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
tests/host.rs   ... 23 passed
tests/block.rs  ... 15 passed
(+ cache 7, fault 7, gic 11, image 5, mmu_boot 10, pci 10, pl011 13, shootdown 19, wx 8, ...)
```

9 ca **mới** của KT-305 (đỏ trước khi sửa — D2-05, đo được: 6 ca đầu FAILED vì
`BlockUnsupported`):

| Ca | Chốt điều gì |
|---|---|
| `translate_block_level2_returns_pa_with_offset` | block 2 MiB: PA = đáy + offset (0, 4 KiB, 2 MiB−4 KiB), `level = 2`, quyền `KERNEL_RW`; block kế ⇒ `NotMapped` |
| `translate_block_level1_returns_pa_with_offset` | block 1 GiB: PA + offset, `level = 1`, quyền `KERNEL_RX` |
| `unmap_inside_block_splits_and_keeps_neighbours` | unmap **một** trang ⇒ chia nhỏ (+1 bảng mức 3), trang vừa bỏ `NotMapped`, **ba trang kế cận còn map**, map lại được |
| `unmap_all_pages_of_block_frees_split_table` | unmap **cả** block (512 lá) ⇒ giải phóng bảng chia nhỏ + mức 2 + mức 1, còn đúng gốc |
| `unmap_level1_block_splits_twice` | 1 GiB → 512 block 2 MiB → 512 lá (+2 bảng); block 2 MiB kế tiếp **vẫn** ở mức 2 |
| `unmap_block_split_out_of_frames_keeps_block` | hết frame khi chia nhỏ ⇒ `OutOfFrames`, block **còn nguyên** (fail-closed) |
| `block_span_matches_encode_block` | `block_span` ⇔ `encode_block` ở **mọi** mức; cỡ = `span(level)` = hình học 4 KiB/9 bit |
| `split_block_entries_keep_attrs_and_pa` | entry con giữ quyền/`AttrIndx`/share, PA = đáy + index × cỡ con; mức 1 → block, mức 2 → lá |
| `block_translate_unmap_unsupported_format_rejected` | format **không** khai `block_span` ⇒ translate/unmap vẫn `BlockUnsupported` (hành vi cũ của x86_64/Sv39) |

### 3.2 Log QEMU (nguyên văn, `boot-1.log` của runner)

```text
# pci: map ecam_pa=0x4010000000 ecam_va=0x80000000 ecam_blocks=128 ecam_block_level=2 ecam_block_pa=0x4010000000 ecam_block_ok=1 bar_va=0x10000000 bar_pages=512 net_bar_va=0x10400000 net_bar_pages=512 gpu_bar_va=0x10800000 gpu_bar_pages=512 input_bar_va=0x10600000 input_bar_pages=512 fb_pa=0x42000000 fb_pages=300 fb_walk=0x42000000 input_dma_va=0xffffffc08005b000 input_dma_walk=0x400db000 frames_used=33
# pci: unmap va=0x8fe01000 pa=0x401fe01000 unmap_pa=0x401fe01000 level=2 translate_ok=1 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1
# pci: ecam pa=0x4010000000 va=0x80000000 host=1b36:0008 class=0x060000 rev=0x00 empty=0xffff functions=5 buses=256
ok 19 - pci: ecam
```

`# pci: unmap` là kết quả **đo trên bảng trang thật** (`ecam.rs`): `translate` một VA giữa block
cuối dải ECAM trả `pa = 0x401fe01000`, `level = 2`; `unmap` trang ấy **chia nhỏ** block (trang
kế cận `0x8fe00000` vẫn dịch được), rồi map lại. `blocks_split=1` là số block bị chia trong phép
đo, không phải hằng trang trí.

### 3.3 Runner + TAP

```text
# 27/27 kiểm sạch (boot=1, tests=35)
# ca âm: 24/24 ĐỎ đúng như phải
```

- TAP **35/35** — **không** thêm ca TAP (yêu cầu "TAP giữ nguyên" của KT-305).
- 24 ca âm: 23 ca cũ **giữ nguyên** + 1 ca mới (x) `blocks_split=0`.
- `soak-aarch64.sh --selftest`: **40/40** (1 dương + 39 âm; +1 ca `blocks_split=0`).

### 3.4 Ngân sách (cùng máy, cùng toolchain 1.100.0)

| Chỉ số | Trước KT-305 | Sau KT-305 |
|---|---:|---:|
| `.data + .bss` | **233.704 B** (89,1 % trần 262.144) | **233.704 B** (89,1 %) — **không đổi** |
| `.text` | 103.676 B | **105.300 B** (+1.624 B mã) |
| `frames_used` (dòng `# pci: map`) | 33 / arena 44 | **33** / arena 44 (phép đo chạy **sau** dòng ấy; nó chia nhỏ 1 block ⇒ +1 frame **sau** khi in) |
| ECAM | 128 block 2 MiB | 128 block 2 MiB; **1** block cuối bị chia nhỏ bởi phép đo (511 lá + 1 lá map lại) |

**Không nới trần I7**, arena giữ 44 frame.

## 4. Đột biến (đều chạy thật, không mô phỏng log)

### (i) `unmap` block **không chia nhỏ** (xoá cả block) ⇒ ĐỎ

Sửa nhánh `Descriptor::Block` trong `PageTable::unmap` thành ghi 0 vào entry rồi trả PA (xoá cả
block). Test host:

```text
$ cargo test -p vifast-plat-aarch64 --test block unmap_inside_block_splits_and_keeps_neighbours
thread 'unmap_inside_block_splits_and_keeps_neighbours' panicked at tests/block.rs:356:
trang kế cận còn map: NotMapped
```

Đúng lý do: **mất mapping kế cận**. Chạy **runner đầy đủ** với ảnh đột biến:

```text
# pci: unmap va=0x8fe01000 pa=0x401fe01000 unmap_pa=0x401fe01000 level=2 translate_ok=1 split=1 neighbor_ok=0 unmapped_ok=1 remap_ok=1 blocks_split=1
PANIC: pci: probe block lỗi CorruptTable
not ok 2 - boot 1: server TCP không có đủ bằng chứng hai chiều (…)
# 24/26 kiểm sạch (boot=1, tests=35)
exit=1
```

Đã khôi phục `split_block_once`.

### (ii) `translate` trả **đáy** block thay vì PA đã cộng offset ⇒ ĐỎ

Sửa `pa: pa + (va & (span - 1))` thành `pa: pa`. Test host:

```text
$ cargo test -p vifast-plat-aarch64 --test block
test translate_block_level2_returns_pa_with_offset ... FAILED
  assertion `left == right` failed: PA = đáy block + offset trong block
  left: 275146342400   right: 275146346496
test translate_block_level1_returns_pa_with_offset ... FAILED
  left: 1073741824     right: 1379160064
```

Runner đầy đủ với ảnh đột biến:

```text
# pci: unmap va=0x8fe01000 pa=0x401fe01000 unmap_pa=0x401fe01000 level=2 translate_ok=0 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1
PANIC: pci: probe block lỗi CorruptTable
# 24/26 kiểm sạch (boot=1, tests=35)
exit=1
```

Đã khôi phục `pa + (va & (span - 1))`.

## 5. Còn thiếu / rủi ro

1. **Không** tuyên bố K4' đạt — K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000). KT-305 chỉ
   đóng `translate`/`unmap` cho block.
2. **Không** làm (đúng phạm vi): block 1 GiB cho **user**, `protect`, `remap`, **gộp** block
   (un-split/merge), `unmap` theo **dải** (chỉ có `unmap(va)` một trang).
3. **`unmap` block giờ cấp một frame** (chia nhỏ). Hết frame ⇒ `OutOfFrames` và block còn nguyên
   — hành vi **mới** so với trước (trước trả `BlockUnsupported` ngay). Đã có test
   `unmap_block_split_out_of_frames_keeps_block`.
4. **TLB**: `PageTable::unmap` vẫn **không** đụng TLB (đúng tầng dữ liệu). Đường sản phẩm phải
   dùng `VSpace::unmap_shootdown` (KT-262) — phép đo trong ảnh chỉ đọc bảng trang nên không cần
   `TLBI`; trang bị bỏ rồi map lại **cùng** PA nên entry TLB cũ (nếu có) vẫn đúng.
5. **`Descriptor::Block` thêm `perms`** là đổi enum dùng chung: x86_64/Sv39 phải sửa `decode`
   (cơ học) và test x86_64 `descriptor_ps_bit_by_level` thêm `perms`. **Không** đổi hành vi của
   hai format ấy (chúng không khai `block_span`).
6. **Soak aarch64 không chạy trong task này** (3 h): ảnh **đổi** (thêm `ecam.rs`, `.text`
   +1.624 B) ⇒ bằng chứng soak của KT-303 **không còn áp dụng** (AGENTS §6.3). Cần soak 1.000 ở
   vòng sau; runner 27/27 + 24/24 ca âm là cổng của KT-305.
7. **Rủi ro thấp**: phép đo chia nhỏ **một** block ECAM (block cuối) ⇒ 1 frame arena + 511 lá
   thay 1 block. Bằng chứng `ecam_block_ok=1` của KT-302 đo block **đầu** nên vẫn đúng; dòng
   `# pci: map` in **trước** phép đo nên `frames_used=33` vẫn khớp fixture soak.

## 6. Xác nhận

- **Không** push, **không** merge, **không** chạm `main`.
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `spec/`.
- **Không** nới ngưỡng, **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common`.
- `cargo fmt` đã chạy; `./v2/bin/v2 check` **xanh** (exit 0); `bash v2/test/boot/run-aarch64.sh`
  **xanh** (27/27, 24/24 ca âm); `soak-aarch64.sh --selftest` **xanh** (40/40).
- Commit `[KT-305]` trên nhánh `task/kt305-blkunmap` (uỷ quyền D2-198); cây sạch sau commit.
