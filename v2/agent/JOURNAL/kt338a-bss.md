# KT-338a — `aarch64`: giải phóng `.bss` (arena bảng trang + 4 trang DMA ra RAM ngoài ảnh)

**Chu kỳ 2 — Đợt 2.** Người làm: sub-agent `bss`. Ngày: 2026-11-22 (giờ máy Asia/Tokyo).
Quyết định: **D2-236** (giải nghẽn `.data+.bss` trước khi nối `vifast-core`), **D2-238** (tách
`KT-338a/b`; mọi task ≤ 3 h), **D2-17** (`unsafe` chỉ trong crate `kernel/vifast-plat-*`, ở đây
chỉ `asm`), **D2-05** (test đỏ trước), **D2-07** (không retry để lấy xanh).
Kế hoạch: `KE-HOACH-CHU-KY-2-2026-11-22.md` mục **KT-338a** (phần `.bss`; phần tách
sản phẩm/`ktest` + `Mailbox` **không** thuộc phạm vi sub-task này — xem §9).

| | |
|---|---|
| Worktree | `/tmp/kt338a-s1` (nhánh `task/kt338a-bss`) |
| Base | `main` @ `99ee94df` (`[KT-338a] Đợt 2: mở KT-338a (aarch64 .bss) + KT-340a (x86_64 VSpace/W^X)`) |
| Máy | macOS Apple Silicon, 12 nhân, Rust/cargo `1.100.0` (ghim `v2/rust-toolchain.toml`) |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`, chạy trong `/tmp/kt338a-s1` |
| Nhãn số liệu | **`[ĐO KT-338a]`** — đo trực tiếp từ log cổng trong worktree này |

Mục tiêu đo được (D2-236): hạ `.data+.bss` của ảnh **aarch64** xuống **< 64 KiB**; **không** nới
trần **262.144 B**; giữ **35/35 TAP**, runner **27/27** + **24 ca âm**, cache maintenance
`dc civac`/`dc ivac` (KT-277) và **W^X** không đổi ngữ nghĩa.

---

## 1. Khảo sát `.bss` ảnh aarch64 (trước khi sửa)

### 1.1 Số đo `budget-aarch64.sh` (nguyên văn)

```text
# image=baseline path=v2/kernel/target/aarch64-unknown-none/release/vifast-kbin-aarch64 file=267384 arch=aarch64(e_machine=183) data+bss=233704 budget=262144 stack=0 guard=0 smp_stacks=557056 noload=557056
# sections-seen: .text=107300 .text.vectors=2076 .el0=52 .rodata=25128 .data=232 .bss=233472 .smp_stacks=557056 
# data+bss-sections: .data=232 .bss=233472 
```

`233.704 / 262.144 = 89,16 %` trần I7 — đúng con số D2-236 ghi (89,2 %).

`budget-aarch64.sh` đo gì (đọc mã): cộng mọi section khớp tiền tố `^\.(data|sdata|bss|sbss)` từ
`llvm-size -A` (nên không bỏ sót `.data.rel.ro`, `.sbss.*`), **trừ** NOLOAD khỏi trần và báo
**riêng** (`.smp_stacks=557056` là arena stack per-hart của KT-259); kiểm `e_machine=183`
(AArch64) **trước** khi đo (trỏ nhầm ảnh arch khác là từ chối, fail-closed); trần giữ nguyên
`256*1024`; vượt ⇒ `exit 1`, không nới.

### 1.2 Từng vùng `.bss` lớn (`llvm-nm --size-sort -S`)

| # | Symbol | Cỡ (B) | %.bss | Ai dùng |
|---:|---|---:|---:|---|
| 1 | `vifast_plat_aarch64::bootmem::ARENA` (`static [AtomicU64; 44×512]`) | **180.224** | 77,2 | Nguồn frame của `PageTable` (`SharedBootMemory`) trên toàn đường boot: `mmu_bringup` (bảng boot), `VSpace::create`/`map_*` (mọi mapping), `umode`/`smpboot` (mapping sau `activate`) |
| 2 | `asm::imp::DMA_PAGE` (virtio-blk, KT-273) | 4.096 | 1,8 | `virtio_blk::Dma::bytes()`, PA cho descriptor |
| 3 | `asm::imp::DMA_PAGE_NET` (KT-275) | 4.096 | 1,8 | `virtio_net` |
| 4 | `asm::imp::DMA_PAGE_GPU` (KT-279) | 4.096 | 1,8 | `virtio_gpu` |
| 5 | `asm::imp::DMA_PAGE_INPUT` (KT-283) | 4.096 | 1,8 | `virtio_input` |
| 6 | `PROBE_PAGE` (KT-249) | 4.096 | 1,8 | Phép đo "MMU thật sự bật": đọc qua VA không identity |
| 7 | `PHYS_PROBE` (KT-253) | 4.096 | 1,8 | Phép đo cửa sổ physmap |
| 8 | `VSPACE_PROBE` (KT-254) | 4.096 | 1,8 | Trang probe của `VSpace` + TLBI |
| 9 | `umode::EL0_STACK` (KT-269) | 4.096 | 1,8 | Stack U-mode |
| 10 | statics nhỏ (`virtio_*::{BLK,INPUT,GPU}` ≤ 0xC0, `faults::*`, `smpboot::*`, cờ `Atomic*`, …) + `.data` | 2.042 | 0,9 | Đếm/bằng chứng TAP |
| — | **padding căn 4 KiB** giữa các static căn trang (arena + 8 trang) | **18.670** | 8,0 | Không phải dữ liệu — hệ quả của `#[repr(align(4096))]` |
| | **Tổng `.data + .bss`** | **233.704** | 100 | |

Kiểm phép cộng: `180.224 + 4×4.096 + 4×4.096 + 2.042 + 18.670 = 233.704` ✔. (Tổng `nm` các
symbol `b/d` = 215.034; phần còn lại là padding — `nm` không tính khe căn lề.)

**Ai dùng `BootMemory::ARENA`**: chỉ `bootmem.rs`; hai cửa `shared_boot_memory()` (gọi ở
`mmu_bringup`, lúc PC còn nửa thấp) và `shared_boot_memory_at(base)` (gọi ở `high_entry` sau khi
nhảy nửa cao, vì `addr_of!(ARENA.0)` lúc ấy trả alias nửa cao, không phải PA). Kiểu
`SharedBootMemory` được đưa vào `SplitMmu`/`VSpace` và đi tiếp vào `wxprobe::map_vspace_image`,
`umode::run`, `smpboot::tlbi_shootdown_test` — tất cả **không** đụng địa chỉ arena trực tiếp, chỉ
gọi `TableMemory::read/write_entry` qua lát `&'static [AtomicU64]`.

**4 trang DMA tĩnh ở đâu**: `v2/kernel/vifast-plat-aarch64/src/asm.rs`, `mod imp` (bare-metal),
mỗi trang `#[repr(align(4096))] struct DmaPage*(UnsafeCell<[u8; 4096]>)` + `static`. Địa chỉ
được driver lấy bằng `asm::dma_page*_addr()`, đổi alias nửa cao → PA bằng
`virtio_*::high_to_pa`; `DmaPool` đọc/cache-maintenance bằng **PA** (vì trang trong `.bss` được
map **identity** trong `VSpace` — điều kiện để `dc civac` trên PA đúng).

---

## 2. Cách giải phóng — chọn và vì sao

### 2.1 Arena → RAM thật ngay sau ảnh, map **identity**

- **Vị trí**: `ARENA_PA = align_up(__kernel_end, 4 KiB)` — biên do **linker** quyết định lúc chạy,
  không phải hằng viết tay. `__kernel_end` là biên đầu tiên chắc chắn trống: linker đã đặt cả
  `.smp_stacks` (NOLOAD, 544 KiB) **trong** `[__text_start, __kernel_end)`; QEMU `virt -m 128M`
  có RAM phẳng `0x4000_0000..0x4800_0000`, ảnh nạp ở `0x4008_0000` ⇒ vùng 256 KiB sau ảnh nằm
  trọn trong RAM (đo được: arena `0x4013_5000–0x4017_5000` < `0x4020_0000`).
- **Không `static`**: arena **không** còn section ELF nào. `bootmem` giữ `static ARENA_BASE:
  AtomicU64` (8 B, PA do `publish_arena` công bố lúc boot) + `SHARED_NEXT`; lát
  `&'static [AtomicU64]` do `asm::arena_words(va)` dựng (unsafe nằm trong `asm`, D2-17), phần
  đọc/ghi entry vẫn là atomics an toàn.
- **Identity-map**: `VA == PA` vì `TableMemory` chỉ có một lát địa chỉ cho **mọi** pha:
  1. **MMU tắt**: PA == VA (như cũ);
  2. **sau khi bật MMU, trước `activate`**: `TTBR0` vẫn là bảng boot — `main.rs` map identity
     `[arena, arena+BOOT_RAM_BYTES)` vào **bảng nửa thấp** (`bootram::map_boot_table`) trước
     `SCTLR_EL1.M=1`; mọi lần ghi entry bảng trang (kể cả `map_high`/`map_phys`) đi qua VA thấp;
  3. **sau `VSpace::activate`**: `TTBR0` đổi sang bảng `VSpace`, nên `bootram::map_vspace` map
     identity **cùng dải** vào bảng nửa thấp của `VSpace` **trước** `activate` — nhờ đó
     `umode`/`smpboot` (cấp bảng trang **sau** `activate`) vẫn ghi được, và `DmaPool` đọc
     PA/cache bằng PA vẫn đúng.
  Không cần alias nửa cao riêng cho arena (khác `.bss`): không đường nào đọc arena qua VA nửa cao.
  **Khác gợi ý trong kế hoạch** ("qua physmap như KT-279"): `DmaPool` của KT-334 nhận **PA** và
  phát `dc civac` trên chính số ấy, nên trang DMA **buộc** phải `VA == PA` tại thời điểm driver
  chạy; một alias physmap (`PHYSMAP_BASE + PA`) sẽ làm `dc` sai địa chỉ. Identity-map là đường
  duy nhất đúng với hợp đồng `DmaPool` hiện có, và nó cũng là đường `input::map_window` walk để
  in `identity=1`.
- **Sạch**: `.bss` được `_start` zero hộ, RAM sau ảnh thì **không** — `bootram::prepare` zero cả
  vùng 256 KiB trước khi công bố PA; ngoài ra `PageTable` vẫn zero từng frame khi cấp (bất biến
  cũ) nên không phụ thuộc một đường zero duy nhất.
- **64 frame** thay 44: con số 44 bị **chặn bởi trần `.bss`** (chính tài liệu cũ ghi "64 frame
  không dùng được vì `.bss` vượt trần"), không phải nhu cầu thật. Arena ra khỏi `.bss` ⇒ bỏ
  chặn: `44` (nhu cầu đã đo của KT-273) + `4` (trang DMA động) + `16` dự phòng cho bảng trang
  của **KT-338b** (`vifast-core` + IPC EL0) = **64 frame = 256 KiB, ngoài ảnh**.

### 2.2 Bốn trang DMA tĩnh → **frame động cấp từ chính arena**

- `bootmem::alloc_boot_frame()` cấp **một** frame 4 KiB từ arena (bộ đếm `SHARED_NEXT`
  `fetch_add` — driver chạy sau khi 4 hart online, nên phải nguyên tử; các lần cấp bảng trang
  khác vẫn `load/store` trên **cùng** bộ đếm và chỉ chạy trên hart 0, hai đường nối tiếp nhau).
- `asm::dma_page_pa(index)` cấp **một lần** cho mỗi driver (blk/net/gpu/input, index 0..3), lưu
  PA vào `static [AtomicU64; 4]`, **zero trang ngay sau khi cấp** (`zero_boot_ram`) rồi trả PA.
  `asm::dma_page*()` trả lát `&'static mut [u8; 4096]` tại **identity VA** (== PA).
- Hàm `dma_page*_addr()` **đổi ngữ nghĩa** từ "VA (alias nửa cao)" → **PA**; bốn call site trong
  `blk/net/gpu/input` bỏ `high_to_pa(...)` và dùng thẳng PA (descriptor thiết bị luôn cần PA).
  `virtio_*::high_to_pa` **giữ nguyên** cho địa chỉ **trong ảnh** (U-mode/host test).
- `DmaPool::sync_for_device` (`dc civac`) / `sync_for_cpu` (`dc ivac`) **không đổi một dòng**:
  chúng nhận `base: u64` là PA từ `init`, và trang DMA vẫn identity-map nên `dc` trên PA đúng
  như trước. Tương tự `Dma::read_u16` đọc thẳng PA.

### 2.3 Phương án bị loại (ghi để người sau không thử lại)

| Phương án | Vì sao **không** |
|---|---|
| Đổi tên section (`ARENA` sang `.bootarena`) để `budget.sh` không cộng | Đánh lừa **phép đo**, không giải phóng byte nào; vi phạm tinh thần D2-236 ("RAM ngoài `.bss`") và I7 (trần đo theo section khai báo) |
| Đặt arena vào slot `.smp_stacks` chưa dùng (4 slot cuối, 272 KiB) | Trộn hai bất biến (stack per-hart vs arena bảng trang); `smp::HartStacks` kiểm bố cục lúc chạy, mọi phép đo `# smp: arena … expect=557056` sẽ sai nghĩa |
| Giữ DMA `static` nhưng trong section NOLOAD riêng | Vẫn chiếm địa chỉ trong ảnh + vẫn là "static" — đúng thứ D2-236 yêu cầu bỏ; và không có lý do kỹ thuật để giữ |
| Lấy RAM từ bảng memory map của DTB/UEFI | QEMU `-kernel` ở đây **không** có UEFI; đọc DTB chỉ để tìm RAM là thêm parser + đường lỗi mới cho một vùng 256 KiB trong khi `__kernel_end` đã là biên chắc chắn. **Nợ cho UEFI thật** (K4'/K5'): lúc có memory map thì lấy vùng theo map, không dựa `__kernel_end` |

### 2.4 Thay đổi mã (9 tệp + 1 tệp mới)

| Tệp | Thay đổi |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/bootmem.rs` | bỏ `struct Arena`/`static ARENA`/`boot_memory()`; `BOOT_ARENA_FRAMES 44→64`; thêm `BOOT_RAM_BYTES`, `BOOT_DMA_FRAMES`, `publish_arena`, `arena_base`, `boot_ram_bytes`, `alloc_boot_frame`; `shared_boot_memory()` dựng từ PA đã công bố; `shared_boot_memory_at(words, base)` nhận lát ngoài |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | bỏ 4 `DmaPage*` static; thêm `arena_words`, `zero_boot_ram`, `dma_page_pa` (cấp động + zero + nhớ PA); 4 cặp `dma_page*_addr()/dma_page*()` đổi sang PA/identity VA; stub host cho 2 hàm mới |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | cập nhật re-export |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | `mmu_bringup`: `bootram::prepare(__kernel_end)` + `bootram::map_boot_table`; `high_entry`: `bootram::map_vspace`; log thêm `boot_ram=…`, `boot_ram_pages=…`, `pages_boot_ram=…`; bỏ `ARENA_PA` |
| `v2/kernel/vifast-kbin-aarch64/src/bootram.rs` (**mới**, 97 dòng) | `prepare`/`map_boot_table`/`map_vspace` (tách để giữ R1; `main.rs` 1.491 dòng) |
| `v2/kernel/vifast-kbin-aarch64/src/ecam.rs` | `probe_block()` — chuyển phép đo walk ECAM khỏi `main.rs` (giữ R1; hành vi nguyên văn) |
| `v2/kernel/vifast-kbin-aarch64/src/{blk,net,gpu,input}.rs` | dùng thẳng PA từ `asm::dma_page*_addr()` (bỏ `high_to_pa` cho DMA) |
| `v2/kernel/vifast-plat-aarch64/src/{cache,virtio_blk,virtio_net,virtio_gpu,virtio_input,resources}.rs` | **chỉ chú thích**: "trang DMA trong `.bss`" → "frame động identity-map ngoài ảnh"; số frame arena 44→64 |
| `v2/kernel/vifast-plat-aarch64/tests/mmu_boot.rs` | **ca đỏ trước**: `boot_arena_has_room_for_dynamic_dma_pages` (`44 + 4` frame, phép tính có ghi) |

---

## 3. Đo sau khi sửa (nguyên văn)

```text
# image=final path=v2/kernel/target/aarch64-unknown-none/release/vifast-kbin-aarch64 file=267288 arch=aarch64(e_machine=183) data+bss=30632 budget=262144 stack=0 guard=0 smp_stacks=557056 noload=557056
# sections-seen: .text=108068 .text.vectors=2076 .el0=52 .rodata=25368 .data=232 .bss=30400 .smp_stacks=557056 
# data+bss-sections: .data=232 .bss=30400 
```

| | Trước | Sau | Chênh |
|---|---:|---:|---:|
| `.data` | 232 B | 232 B | 0 |
| `.bss` | 233.472 B | **30.400 B** | **−203.072 B (−86,98 %)** |
| `.data+.bss` | 233.704 B (89,16 % trần) | **30.632 B (11,68 % trần)** | **−203.072 B** |
| Trần còn lại | 28.440 B | **231.512 B** | +203.072 B |
| `.text` | 107.300 B | 108.068 B | +768 B (mã mới + panic message) |
| `.rodata` | 25.128 B | 25.368 B | +240 B |

Mục tiêu D2-236 `< 64 KiB` **đạt** (30.632 B). Trần **262.144 B không nới**.
`.text` tăng 1.008 B là mã của đường cấp phát/mapping mới — chấp nhận được, không phải nới gì.

### 3.1 Bố cục thật lúc chạy (log boot, `[ĐO KT-338a]`)

```text
# mmu: arena=0x40135000 frames=64 used=14 root_low=0x40135000 root_high=0x40136000 ttbr0=0x40137000 ttbr1=0x4013e000 level=1
# mmu: map kernel=0x40080000-0x40135000 pages=173 boot_ram=0x40135000-0x40175000 boot_ram_pages=64 uart=1 gic=32 probe_va=0x50000000 probe_pa=0x400a7000
# vspace: map va=0x60000000 pa=0x400a9000 walk=0x400a9000 match=1 pages_high=173 pages_physmap=173 pages_image=173 pages_boot_ram=64 pages_mmio=289
# pci: map … input_dma_va=0x40156000 input_dma_walk=0x40156000 frames_used=34
# blk: sector0 … dma=0x4015b000 desc=0x4015b000 …
# net: device … dma=0x4015c000
# gpu dma: desc=0x4015d000 avail=0x4015d100 used=0x4015d128 cmd=0x4015d200 resp=0x4015d600 dma=0x4015d000 …
# input dma: desc=0x40156000 … dma=0x40156000 walk=0x40156000 identity=1 armed=8 …
```

Bốn trang DMA **động** nằm trong arena `0x4013_5000–0x4017_5000`; `identity=1` của virtio-input
là kết quả `vs.walk(dma_pa).pa == dma_pa` trên bảng trang thật. `frames_used=34` lúc dòng
`# pci: map` (đã gồm frame DMA của input cấp trước `activate`); driver còn lại cấp tiếp, tổng
vẫn ≪ 64.

### 3.2 Chứng minh **không** ảnh hưởng `riscv64`/`x86_64`

Xây **cùng** hai ảnh từ `main` @ `99ee94df` (worktree tạm `/tmp/kt338a-base`) và từ cây sau sửa;
so từng section:

| Ảnh | Trước (`main`) | Sau (cây sửa) | Kết luận |
|---|---:|---:|---|
| `riscv64` `.data+.bss` | 196.104 B | 196.104 B | **không đổi** |
| `x86_64` `.data+.bss` | 177.440 B | 177.440 B | **không đổi** |

Section-by-section cũng y hệt (`riscv64`: `.text=50918 .rodata=14920 .data=7824 .bss=188280`,
file 191.320 B; `x86_64`: `.text=135168 .rodata=10348 .data=4408 .user=95 .bss=173032`, file
189.120 B). Hợp lý: diff chỉ chạm `vifast-plat-aarch64` + `vifast-kbin-aarch64` + test của crate
aarch64; không file dùng chung nào bị sửa.

---

## 4. Cache maintenance (KT-277) và W^X (KT-277) — **không đổi ngữ nghĩa**

- `v2/kernel/vifast-plat-aarch64/src/cache.rs` **chỉ đổi chú thích** (2 dòng mô tả vị trí vùng
  DMA); `HandoffStep`/`HANDOFF_SEQUENCE`/`apply_handoff`/`apply_poll_inval` **không đổi một dòng
  mã** — `git diff cache.rs` chỉ có hunk `//!`.
- `DmaPool::sync_for_device` (`dc civac` từng dòng + `dsb`) và `sync_for_cpu` (`dc ivac`) **nguyên
  văn**; chỉ **địa chỉ** của vùng DMA đổi (từ `.bss` sang frame động), vẫn **identity-map** nên
  `dc … , PA` hợp lệ như trước — lý do identity-map của vùng boot là **bắt buộc**, không phải
  trang trí.
- Bằng chứng đo (log boot, runner xanh):
  - `# dma: cache-clean ok=1 lines=64 bytes=4096 ops=1 inval_lines=64 inval_bytes=4096 inval_ops=1 …`
    → `ok 27 - dma: cache maintenance before/after DMA`;
  - `# input stats: … clean_lines=128 clean_bytes=8192 clean_ops=2 inval_lines=64 inval_ops=1` > 0;
  - `# gpu stats: … fb_clean_lines=… fb_clean_ops=2`.
- W^X: `wx.rs` **không** sửa; arena map `Perms::KERNEL_RW` (RW, NX) đúng như vùng `.data/.bss`
  cũ; `# wx: text=RX data=RW ok=1 … fault_ec=0x25 fault_access=Write … text_unchanged=1 data_rw=1
  data_in_rw=1` → `ok 26`. `pages_kernel=173` vẫn bằng `(text_bytes + data_bytes - guard_bytes)/4096`
  (runner `check_wx` đối chiếu) — mapping vùng boot **không** bị cộng vào `pages=` của vùng ảnh.

---

## 5. Đỏ trước (D2-05) và hai đột biến

### 5.1 Test đỏ trước

Ca `boot_arena_has_room_for_dynamic_dma_pages` (host, `tests/mmu_boot.rs`) đòi
`BOOT_ARENA_FRAMES ≥ 44 + 4` — vế phải là **phép tính có ghi** (44 frame đã đo của KT-273 + 4
trang DMA của 4 driver), không phải số chép tay. Trên base `44 < 48` ⇒ **ĐỎ**:
log nguyên văn `v2/agent/JOURNAL/kt338a-red.log`. Sau sửa: `ok` (64 ≥ 48).

### 5.2 Đột biến (i) — `ARENA` trở lại `.bss` ⇒ **budget ĐỎ**

Cách làm: tạm thời cho `shared_boot_memory()` dựng trên `static MutArena([AtomicU64;
64×512])` trong `.bss` (đúng kích thước arena mới), giữ nguyên phần còn lại. Build xanh, **boot
vẫn được** (bảng trang vẫn hợp lệ) — nghĩa là nếu chỉ có test chức năng thì sẽ **lọt**; cổng ngân
sách phải bắt.

```text
# image=mutation-arena-bss … file=267496 arch=aarch64(e_machine=183) data+bss=295192 budget=262144 …
# sections-seen: .text=108012 .text.vectors=2076 .el0=52 .rodata=25368 .data=232 .bss=294960 .smp_stacks=557056 
# data+bss-sections: .data=232 .bss=294960 
budget-aarch64: VƯỢT 295192 > 262144 byte — D2-72 Q5 cấm nới trần (cắt mảng tĩnh sang arena/RAM boot)
```

`exit 1` ⇒ **ĐỎ đúng lý do** ("trần có răng"). Đột biến đã được revert (cây cuối không còn
`MutArena`).

### 5.3 Đột biến (ii) — một trang **không** identity-map ⇒ **boot/TAP ĐỎ**

Cách làm: tạm thời, sau khi map identity vùng boot trong `VSpace`, cấp sớm trang DMA của
`virtio-input` rồi `vs.unmap(asm::dma_page_input_addr())` — đúng **một trang** (frame DMA của
input) mất mapping identity. Build xanh; boot:

```text
PANIC: input: map BAR/trang DMA lỗi dma_walk
```

và runner: `not ok 2 - boot 1: … không đủ bằng chứng`, `# ca âm: 20/24`, `# 24/26 kiểm sạch`
(exit 1). Không im lặng: `input::map_window` walk bảng trang `VSpace` và từ chối ngay khi
`walk(dma_pa)` không trả đúng PA; nếu đi xa hơn thì `DmaPool` đọc PA/`dc civac` trên PA cũng
fault. Đột biến đã được revert.

---

## 6. Giữ xanh (cổng)

| Cổng | Kết quả |
|---|---|
| `cargo test -p vifast-plat-aarch64` (host, gồm ca mới) | PASS — `mmu_boot` 11/11, toàn crate 0 failed |
| `cargo test -p vifast-plat-common -p vifast-plat-aarch64 -p vifast-plat-x86_64` (bộ `step_l0_kernel` của `v2 check`) | PASS |
| `bash v2/test/boot/run-aarch64.sh 1` | **RC=0**; `# 27/27 kiểm sạch (boot=1, tests=35)`; `# ca âm: 24/24 ĐỎ đúng như phải` |
| `bash v2/test/boot/budget-aarch64.sh <ảnh>` | PASS 30.632 ≤ 262.144 |
| `bash v2/test/boot/soak-aarch64.sh --smoke 60 3` | **60/60 boot xanh, 0 lỗi** (hai lần chạy); exit 1 vì `MEASUREMENT_INVALID` — xem §6.1 |
| `./v2/bin/v2 check` | **PASS** (134 s) |
| `./v2/bin/v2 gate --fast` (T2, bắt buộc cho thay đổi boot/page-table) | xem §6.2 |
| `gpu.sh`/`input.sh` | **không dùng aarch64** (riscv64/x86_64); đường GPU/bàn phím aarch64 nằm trong `run-aarch64.sh` (TAP 28–32, xanh) |

### 6.1 Soak aarch64 `--smoke 60 3` và `v2 check`

**Soak** (`bash v2/test/boot/soak-aarch64.sh --smoke 60 3`, log `/tmp/kt338a-soak.txt` +
`/tmp/kt338a-soak2.txt`):

```text
# soak: xong wall=89s (0 boot/s); ok=60 fail=0 tổng=60/60
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 1000 — KHÔNG phải bằng chứng cổng K4' (D2-24), luôn exit 1
```

**60/60 boot xanh, 0 lỗi** — nhưng `exit 1` vì `MEASUREMENT_INVALID`: `loadavg_1m=16,17` (lần 1)
và `14,36` (lần 2) `> ncpu=12` ở phép đo `[after]`, trong khi `qemu_total=0 qemu_ppid1=0
qemu_own=0 qemu_foreign=0` — tải đến từ tiến trình **khác** trên máy (các sub-agent song song
đang build/QEMU), không phải QEMU của task này. Theo **D2-07/AGENTS §6.3**: `T-INFRA`, số **thời
gian** không dùng để kết luận; đã chạy lại **đúng một lần** chỉ để phân loại (cùng kết quả 60/60).
`BUILD_ID=6e698b6cfec1bd29` của soak khớp ảnh đã đo ở §3.

**`v2 check`**: `v2 check: PASS` (134.000 ms, log `/tmp/kt338a-check.txt`; chạy trên cây làm việc
tại HEAD `99ee94df`): fmt OK, clippy OK, luật R1..R5 + tên tệp PASS, L0 kernel PASS.

### 6.2 `gate --fast` (T2 — bắt buộc cho thay đổi boot/page-table)

`./v2/bin/v2 gate --fast` = **PASS** — `v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000
seed)`, tổng **689.951 ms**, log `/tmp/kt338a-gate-fast5.txt`, chạy trên cây **sạch** tại commit
`5e9c0ac8`. Các lần chạy trước đỏ vì **hạ tầng**, không phải sản phẩm (D2-07/D2-116 — ghi log,
không retry mù):

| Lần | Kết quả | Nguyên nhân (T-INFRA, đã xử lý) |
|---:|---|---|
| 1 | FAIL (7 bước) | đĩa máy đầy — `No space left on device`, còn **438 MiB**; kèm `qemu_foreign=3` |
| 2 | FAIL (7 bước) | đĩa đầy tiếp (build riscv64 + 9 ảnh `ktest` ăn hết 1,9 GiB) |
| 3 | FAIL (4 bước) | cây **dirty** (chưa commit) ⇒ `REV-GATE: REFUSE reason=dirty` (KT-86/D2-125.3) |
| 4 | FAIL (1 bước) | `selftest qemu guard`: baseline `qemu_total=1` — QEMU của agent khác trên máy |
| 5 | **PASS** | cây sạch `5e9c0ac8`, `qemu=0`, đã giải phóng ~12 GiB đĩa (dọn `target/` của các worktree task đã `DONE`: KT-333a/334/337 + worktree chu kỳ trước) |

Sau commit chỉ **thêm** tệp journal này (không đụng mã, không đụng input của rev-gate) nên bằng
chứng T2 được giữ nguyên theo AGENTS §6.3.

**Bước T2 đã chạy trong gate:** L0 + L0 kernel + L1 1.000 seed, 9 ảnh `ktest` + rev-gate,
QEMU **aarch64** (`run-aarch64.sh`), QEMU virtio-net, QEMU x86_64, virtio-input, virtio-gpu,
persist đỏ-trước + cắt điện giữa lúc ghi, git-guard (3 ca âm), selfbreak 5 luật K6, guard QEMU,
cùng toàn bộ selftest harness.

---

## 7. Rủi ro / ghi chú

1. **Tăng arena 44→64 frame**: ngoài trần `.bss` nên không ảnh hưởng I7; 16 frame dư là cho
   KT-338b. Nếu KT-338b cần hơn, tăng `BOOT_ARENA_FRAMES` (vùng sau ảnh còn >100 MiB RAM) —
   **không** đụng trần.
2. **`__kernel_end` phụ thuộc linker**: nếu sau này thêm section NOLOAD **sau** `.smp_stacks`,
   `kernel_end` tự dời theo — vẫn đúng. Nếu chuyển sang boot UEFI (K5'), phải lấy vùng theo
   memory map của firmware thay vì giả định RAM phẳng từ `0x4008_0000` — ghi nợ ở §2.3.
3. **`input_dma_va=` giữ nhãn cũ** trong dòng `# pci: map …` dù nay là **PA** (identity). Không
   đổi nhãn để không phải sửa soak fixture/README log mẫu; journal ghi rõ ngữ nghĩa mới.
4. **`run-aarch64.sh` ca âm (l)/(m)/(p)/(q)** dưới đột biến (ii) đỏ vì boot PANIC trước driver —
   đúng (các ca ấy đòi log "no device" của một boot sạch). Đột biến đã revert; cây cuối xanh
   27/27 + 24/24.

## 8. Việc **không** thuộc sub-task này (báo lead)

`KT-338a` trong kế hoạch còn hai phần: (a) tách **ảnh sản phẩm** vs `ktest-aarch64` (D2-08,
`harness-scan` + ca âm), (b) nối `vifast_sync::Mailbox` cho SGI/shootdown (bước 2 đóng Audit
Q1-A). Sub-task `bss` **chỉ** làm phần `.data+.bss`; hai phần trên cần task/sub-agent khác.
