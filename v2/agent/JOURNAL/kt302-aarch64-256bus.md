# KT-302 — aarch64 quét **256 bus** (block descriptor 2 MiB cho ECAM)

Trạng thái: **DONE** — nhánh `task/kt302-a64bus`, base `main` @ `a78e67a9`, 2026-11-21.
Sub-agent `a64bus`. Quyết định: **D2-218** (vòng 17 — KT-302), **D2-208** (KT-273 PCI ECAM),
**D2-195** (mở K4' bước 3), **D2-05** (test đỏ trước), **D2-198** (uỷ quyền commit nhánh task),
**D2-17** (`unsafe` chỉ trong crate arch), **D2-199** (không đổi trait `PageTableFormat`/`Mmu`
cho nửa cao — nhưng KT-302 **thêm một hàm** `map_block` **có mặc định** vào trait, xem §2).

**Không** tuyên bố K4' đạt: K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000). KT-302 chỉ mở
rộng **dải quét PCI** từ 16 lên 256 bus.

## 1. Khảo sát trước (bắt buộc — ghi lại *trước* khi sửa)

### 1.1 `split_mmu.rs` / `vspace.rs` có block descriptor chưa?

**Chưa.** Đo bằng `grep` trên cây tại `a78e67a9`:

```text
$ grep -rn "encode_block\|map_block" v2/ --include=*.rs
(không có kết quả)
$ grep -n "Block" v2/lib/vifast-pte/src/page_table.rs
257:    /// `BlockUnsupported`; bảng hỏng ⇒ `CorruptTable`.
282:                Descriptor::Block { .. } => return Err(MapError::BlockUnsupported),
303:                Descriptor::Block { .. } => return Err(MapError::BlockUnsupported),
328:    /// Dịch `va`; `NotMapped` nếu trống, `BlockUnsupported` nếu gặp block.
352:                Descriptor::Block { .. } => return Err(MapError::BlockUnsupported),
```

Tình trạng chính xác:

| Tầng | Block descriptor |
|---|---|
| `vifast-pte::PageTableFormat` | **không** có `encode_block` — chỉ `encode_table`/`encode_leaf` |
| `vifast-pte::PageTable::map_with` | chỉ tạo **lá 4 KiB** ở mức 3; gặp block trên đường đi ⇒ `AlreadyMapped` |
| `vifast-pte::PageTable::translate`/`unmap` | gặp block ⇒ `BlockUnsupported` (cố ý — test `block_descriptor_rejected` chốt) |
| `vifast-plat-aarch64::format::Aarch64::decode` | **nhận diện** block (mức 1/2) — walker không nói dối |
| `split_mmu.rs`/`vspace.rs` | chỉ map 4 KiB (`map_with`/`map_obj`); **không** có đường block |

Vậy phải **thêm tối thiểu một hàm map block 2 MiB**. Chọn chỗ đặt: `vifast-pte::PageTable`
(crate dùng chung) vì walker/map nằm ở đó — thêm vào crate arch sẽ là **bản sao thứ hai** của
walk (đúng thứ D2-199 muốn tránh). Thêm **một** phương thức trait `encode_block` **có cài đặt
mặc định trả `None`** (ba impl hiện có — Sv39, AArch64, x86_64 — không phải sửa), và **một**
phương thức `PageTable::map_block`. `translate`/`unmap` **giữ nguyên** ngữ nghĩa
`BlockUnsupported` (không đổi test `block_descriptor_rejected`).

### 1.2 Vì sao 256 MiB ECAM không map được bằng trang 4 KiB

- ECAM QEMU `virt` aarch64: PA `0x40_1000_0000`, **256 MiB** (`resources::PCI_ECAM`).
- Ảnh chỉ map trang 4 KiB ⇒ mỗi 2 MiB cần **một** bảng mức 3: 256 MiB / 2 MiB = **128** bảng
  mức 3 + 1 bảng mức 2 = **129 frame** = 516 KiB.
- Arena boot `BOOT_ARENA_FRAMES = 44` (176 KiB); trần `.data+.bss` **262.144 B** (I7) — đo
  baseline: `.data+.bss = 233.704 B` (89,1 %). Nâng arena lên 129 frame là **không thể** nếu
  không nới I7 (cấm).
- Block 2 MiB ở **mức 2**: 256 MiB = **128 block**, tất cả trong **một** bảng mức 2 (512 entry)
  ⇒ chỉ tốn **2 frame** (một bảng mức 1 + một bảng mức 2; gốc mức 0 đã có). **1 frame** nếu
  bảng mức 1/mức 2 đã tồn tại — ở ảnh này ECAM VA `0x8000_0000` rơi vào level-1 index 2 (mới)
  nên cần cả hai.

### 1.3 Ràng buộc kỹ thuật đã kiểm

- `ECAM_VA_BASE = 0x8000_0000` **căn 2 MiB** (0x8000_0000 / 0x20_0000 = 1024).
- `ECAM_BASE = 0x40_1000_0000` **căn 2 MiB** (0x40_1000_0000 / 0x20_0000 = 131.200).
- 256 MiB ECAM phủ VA `0x8000_0000..0x9000_0000` — vẫn `< 2^38` (canonical nửa thấp) và không
  đè dãy VA đã dùng (`0x5000_0000` probe, `0x6000_0000` VSpace, `0x7000_0000` EL0).
- `SCAN_BUSES` hiện là `u8`; 256 **không** vừa `u8` ⇒ đổi sang `u16` (bus PCI vẫn 0..=255).
- `EcamProbe.buses_scanned` đổi `u8` → `u16`; `Ecam.bus` giữ `u8` (số bus PCI là 8 bit).

## 2. Thay đổi

| Tệp | Thay đổi |
|---|---|
| `v2/lib/vifast-pte/src/mmu.rs` | **mới** `PageTableFormat::encode_block(level, pa, perms, attr, share) -> Option<u64>` — **cài đặt mặc định trả `None`** (ba impl Sv39/AArch64/x86_64 **không** phải sửa) |
| `v2/lib/vifast-pte/src/page_table.rs` | **mới** `PageTable::map_block(va, pa, level, perms, attr, share)` — kiểm cấp/căn lề/canonical/dải PA/quyền/**năng lực format** trước khi walk; walk tới `level`, cấp bảng trung gian, ghi descriptor block. `translate`/`unmap`/`destroy` **không đổi** |
| `v2/kernel/vifast-plat-aarch64/src/format.rs` | `Aarch64::encode_attrs` (bit chung leaf/block); `Aarch64::encode_block` (mức 1 = 1 GiB, mức 2 = 2 MiB; bit 1 = 0); `impl PageTableFormat for Aarch64` cài `encode_block` |
| `v2/kernel/vifast-plat-aarch64/src/split_mmu.rs` | **mới** `SplitMmu::map_block` — định tuyến theo VA (nửa thấp/nửa cao), gọi `PageTable::map_block` |
| `v2/kernel/vifast-plat-aarch64/src/vspace.rs` | **mới** `VSpace::map_block` — wrapper `Shareability::Inner` |
| `v2/kernel/vifast-kbin-aarch64/src/mapwin.rs` | **mới** `map_window_blocks_into` — map `[pa, pa+bytes)` bằng block 2 MiB; đòi `bytes` bội 2 MiB + VA/PA căn 2 MiB; trả **số block** |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | ECAM dùng `map_window_blocks_into`; **walk bảng trang thật** tại `ECAM_VA_BASE`, đòi `WalkOutcome::Block { pa: ECAM_BASE, level: 2 }` (sai ⇒ PANIC); in `ecam_blocks=128 ecam_block_level=2 ecam_block_pa=… ecam_block_ok=1` |
| `v2/kernel/vifast-plat-aarch64/src/pci.rs` | `ECAM_MAP_BYTES = 256 << 20`; `SCAN_BUSES: u16 = 256`; `EcamProbe.buses_scanned: u16`; ba vòng quét ép `bus as u8` sau biên; tài liệu đầu tệp viết lại |
| `v2/kernel/vifast-plat-aarch64/tests/host.rs` | **+5 ca**: `map_block_2m_covers_256mib_with_two_frames` (128 block ⇒ **2 frame**, walk thấy `Block{pa,level:2}`, `translate`/`unmap` vẫn `BlockUnsupported`, map 4 KiB trong block ⇒ `AlreadyMapped`), `map_block_unaligned_rejected_without_allocating`, `map_block_level_out_of_range_rejected`, `map_block_overlap_rejected`, `map_block_unsupported_format_rejected` |
| `v2/kernel/vifast-plat-aarch64/tests/pci.rs` | **+1 ca** `scan_covers_all_256_buses_with_one_level2_table` (256 MiB, 256 bus, 128 block ≤ 512 entry, VA/PA căn 2 MiB) |
| `v2/test/boot/run-aarch64.sh` | `check_pci` đòi `buses=256` + `ecam_blocks=128` + `ecam_block_level=2` + `ecam_block_ok=1`; **+2 ca âm** (v) `buses=16`, (w) `ecam_block_ok=0` (21 → **23 ca âm**) |
| `v2/test/boot/soak-aarch64.sh` | `check_boot_log` đòi `buses=256`; fixture cập nhật dòng `# pci: map`/`# pci: ecam` |
| `v2/docs/k4-aarch64-boot.md` §10 | §10.2/§10.3/§10.5 viết lại theo 256 bus + block descriptor |

**Không** đổi: `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, trần I7, arena
(`BOOT_ARENA_FRAMES = 44`), `translate`/`unmap` (vẫn `BlockUnsupported` với block).

## 3. Đo

### 3.1 Log QEMU (nguyên văn, `boot-1.log` của runner)

```text
# pci: map ecam_pa=0x4010000000 ecam_va=0x80000000 ecam_blocks=128 ecam_block_level=2 ecam_block_pa=0x4010000000 ecam_block_ok=1 bar_va=0x10000000 bar_pages=512 net_bar_va=0x10400000 net_bar_pages=512 gpu_bar_va=0x10800000 gpu_bar_pages=512 input_bar_va=0x10600000 input_bar_pages=512 fb_pa=0x42000000 fb_pages=300 fb_walk=0x42000000 input_dma_va=0xffffffc08005b000 input_dma_walk=0x400db000 frames_used=33
# pci: ecam pa=0x4010000000 va=0x80000000 host=1b36:0008 class=0x060000 rev=0x00 empty=0xffff functions=5 buses=256
ok 19 - pci: ecam
# pci: virtio-blk found bus=0 dev=2 func=0 vendor=0x1af4 device=0x1001 class=0x010000 rev=0x00 bar=4 kind=mem64 size=0x4000 assigned=0x10000000 readback=0x10000000 command=0x0006 caps=1 other_bars=0 reserved=6
ok 20 - pci: virtio-blk found
# blk: sector0 hex=5649464153544f532d4b543237332d53 ascii="VIFASTOS-KT273-S" magic=1 capacity=8 status=0x0f dma=0x400dc000 desc=0x400dc000 avail=0x400dc080 used=0x400dc098
ok 21 - blk: sector0
```

`ecam_block_ok=1` là kết quả **walk bảng trang thật** (`walk_steps(ECAM_VA_BASE)`), không phải
lời khai: kết cục `WalkOutcome::Block { pa: 0x4010000000, level: 2 }` khớp `ECAM_BASE` và mức 2.

### 3.2 TAP + runner

```text
# 26/26 kiểm sạch (boot=1, tests=35)
# ca âm: 23/23 ĐỎ đúng như phải
```

- TAP **35/35** (không thêm ca TAP — `buses=256` là **điều kiện của dòng bằng chứng** `# pci:`,
  không phải ca TAP mới; yêu cầu "TAP giữ nguyên" của KT-302).
- 23 ca âm: 21 ca cũ **giữ nguyên** + 2 ca mới (v)/(w). Không ca nào bị xoá/nới.

### 3.3 Ngân sách trước/sau (cùng máy, cùng toolchain 1.100.0)

| Chỉ số | Trước KT-302 (đo bằng `git stash` + build lại) | Sau KT-302 |
|---|---:|---:|
| `.data + .bss` | **233.704 B** (89,1 % trần 262.144) | **233.704 B** (89,1 %) — **không đổi** |
| `.text` | 101.920 B | 103.676 B (+1.756 B mã) |
| `frames_used` (sau PCI) | **41** / arena 44 | **33** / arena 44 (**−8 frame**) |
| ECAM | 4096 trang 4 KiB (16 MiB, 16 bus) | 128 block 2 MiB (256 MiB, 256 bus) |
| `# pci: ecam` | `buses=16` | `buses=256` |

**Không nới trần I7**: `.data+.bss` y nguyên; arena giữ 44 frame. Block descriptor **giảm** 8
frame (10 frame cho ECAM 16 MiB → 2 frame cho ECAM 256 MiB).

## 4. Đột biến (đều chạy thật, không mô phỏng log)

### (i) Map thiếu bus ⇒ `buses=16` (ĐỎ)

Sửa `ECAM_MAP_BYTES = 16 << 20` (hành vi KT-273), build lại, chạy **runner đầy đủ**:

```text
not ok 2 - boot 1: # dòng '# pci: ecam' không có 'buses=256' (KT-302: phải quét đủ 256 bus) # dòng '# pci: map' không có 'ecam_blocks=128' (256 MiB / 2 MiB) , log …/boot-1.log
# 25/26 kiểm sạch (boot=1, tests=35)
exit=1
```

Log ảnh đột biến: `# pci: ecam … buses=16`, `# pci: map … ecam_blocks=8`. Đã khôi phục
`ECAM_MAP_BYTES = 256 << 20`.

### (ii) Block descriptor sai alignment ⇒ PANIC `Unaligned` (ĐỎ)

Sửa `main.rs` truyền `pci::ECAM_BASE + 0x1000` (PA lệch 4 KiB, không căn 2 MiB), build lại, chạy
**runner đầy đủ**:

```text
PANIC: pci: map ECAM lỗi Unaligned
not ok 2 - boot 1: server TCP không có đủ bằng chứng hai chiều (…)
# 23/25 kiểm sạch (boot=1, tests=35)
exit=1
```

`PageTable::map_block` từ chối **trước** khi cấp frame/walk (test host
`map_block_unaligned_rejected_without_allocating` chốt `outstanding()` không đổi). Đã khôi phục
`pci::ECAM_BASE`.

### (iii) Đột biến phụ — map thiếu **một** block cuối ⇒ **data abort thật** ở bus 254

Sửa `map_window_blocks_into` map `blocks - 1` (127 block = bus 0..253), build lại, boot:

```text
# trap: slot=4 off=0x200 ec=0x25 il=1 far=0x8fe00000 elr=0xffffffc0800046b0 spsr=0x800003c5
```

`far=0x8fe00000` = `ECAM_VA_BASE + 254 MiB` — đúng **bus đầu tiên** chưa map (mỗi block 2 MiB
phủ **2** bus). Đây là bằng chứng **quét thật tới bus 254/255**, không phải chỉ in `buses=256`:
thiếu một block là fault ngay. Đã khôi phục `while index < blocks`.

## 5. Còn thiếu / rủi ro

1. **Không** tuyên bố K4' đạt — K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000). KT-302 chỉ
   mở dải quét PCI.
2. **Không** làm: MSI-X, hotplug, ECAM > 256 bus (QEMU `virt` không có), `translate`/`unmap` cho
   block (vẫn `BlockUnsupported` — block là mapping một chiều cho MMIO; muốn bỏ phải `destroy`).
3. **Rủi ro thấp**: `SCAN_BUSES` đổi `u8` → `u16`; mọi nơi dùng đã ép `bus as u8` sau biên
   `0..SCAN_BUSES` (256 giá trị 0..=255 vừa `u8`). Test host `scan_covers_all_256_buses…` chốt
   bus cuối 255 vẫn nằm trong `PCI_ECAM`.
4. **Thời gian quét**: 256 bus × 32 × 8 = 65.536 lần đọc config cho mỗi lượt quét (3 lượt:
   `probe_ecam`, `init_for`, `collect_other_bars`); đo trên QEMU TCG: boot đầy đủ ~0,7 s (không
   chậm hơn đáng kể so với 16 bus) — không cần tối ưu trong phạm vi task.
5. **Soak aarch64** không chạy trong task này (3 h); runner xanh 26/26 là cổng của KT-302. Soak
   1.000 là việc của vòng sau nếu ảnh đổi (AGENTS §6.3).
