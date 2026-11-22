# KT-334 — Transport VirtIO-PCI dùng chung 3 arch (`vifast-virtio`) + adapter mỏng

**Chu kỳ 2 — Đợt 1.** Người làm: sub-agent `virtio`. Ngày: 2026-11-22 (giờ máy Asia/Tokyo).

| | |
|---|---|
| Worktree | `/tmp/kt334-s1` (nhánh `task/kt334-virtio`) |
| Base | `main` @ `53716be8` (`[KT-331]`) |
| Máy | macOS, Rust/cargo `1.100.0` (ghim `v2/rust-toolchain.toml`, `v2/kernel/rust-toolchain.toml`) |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`; `cargo --manifest-path v2/Cargo.toml` (lib), `cargo --manifest-path v2/kernel/Cargo.toml` (platform) |
| Nhãn số liệu | số đo lấy từ log cổng/runner trong `/tmp/kt334-*.log`; `wc -l` đo trực tiếp |

---

## 0. TL;DR

| Hạng mục | Trước | Sau |
|---|---:|---:|
| `pci.rs` + `virtio_blk.rs` × 3 arch | **4.407 dòng** | **2.235 dòng** (**−49,29 %**) |
| riscv64 (`pci` + `virtio_blk`) | 1.198 | 481 (**−59,85 %**) |
| aarch64 (`pci` + `virtio_blk`) | 1.466 | 659 (**−55,05 %**) |
| x86_64 (`pci` + `virtio_blk`) | 1.743 | 1.095 (**−37,18 %**) |
| Logic dùng chung mới | 0 | `pcibus` 577 + `blkdev` 880 + `dma` 148 + `mmio` 57 = **1.662 dòng** (có test host) |
| Test host `vifast-virtio` | — | **18/18 xanh**, 3 đột biến ⇒ đỏ |
| `rust-version` (`v2/kernel/Cargo.toml`) | `1.99.1` (lệch workspace) | **`1.100.0`** (khớp `v2/Cargo.toml`) |
| `v2 check` | xanh ở base | **xanh** (log `/tmp/kt334-check3.log`) |
| QEMU riscv64 matrix smp 1/2/4 | — | log `/tmp/kt334-ktest-rv.log` |
| QEMU x86_64 | — | log `/tmp/kt334-x86_64.log` |
| QEMU aarch64 | — | log `/tmp/kt334-aarch64.log` |

**Kết luận**: ba bản sao của cùng một giao thức virtio (status → feature → queue → descriptor →
notify → poll) đã gộp về **một** `Blk` trong crate thuần; ba nền tảng chỉ còn **adapter** cho ba
thứ khác nhau thật: cửa sổ MMIO, cách lấy PA của trang DMA (kèm cache maintenance aarch64), và
nguồn thời gian. TAP/API của ảnh **không đổi** (xem §6).

---

## 1. Vì sao (bối cảnh)

Ba `virtio_blk.rs` ở base là **ba bản sao** của cùng một chuỗi khởi tạo/IO virtio-blk:
`reset → ACK → DRIVER → đọc/đàm phán feature → FEATURES_OK → cấu hình queue → DRIVER_OK →
dựng descriptor/avail/used → notify → poll used → đọc virtio_blk_config`, mỗi bản ~500–700 dòng,
khác nhau **chỉ** ở:

1. đọc/ghi thanh ghi BAR (con trỏ thô vs `asm::mmio_*` vs cổng/MMIO x86);
2. địa chỉ vật lý của trang DMA (identity riscv/x86; alias nửa cao aarch64) và **cache
   maintenance** aarch64 (`dc civac`/`dsb`, KT-277);
3. nguồn thời gian (timebase SBI, `CNTPCT_EL0`, PIT µs truyền vào);
4. `PciReport`/`BlkReport` mỗi bản một bộ trường.

Tương tự với `pci.rs`: quét ECAM, đo BAR bằng mask, chọn BAR chứa capability, kiểm chồng lấn —
riscv và aarch64 gần như **trùng từng dòng** (khác địa chỉ ECAM); x86 khác hẳn vì firmware đã
enumerate (ACPI MCFG + legacy 0xCF8/0xCFC + BAR đã gán). Ba bản ~500–930 dòng.

KT-334 gộp phần **quyết định** vào `v2/lib/vifast-virtio` (thuần, `#![forbid(unsafe_code)]`,
test host được — D2-05) và để platform giữ đúng phần **chạm phần cứng** (D2-17).

---

## 2. Thiết kế sau khi gộp

### 2.1 Trait biên (`v2/lib/vifast-virtio/src/mmio.rs`, `dma.rs`)

```rust
pub trait MmioBus {
    fn read8(&self, addr: u64) -> u8;
    fn read16(&self, addr: u64) -> u16;
    fn read32(&self, addr: u64) -> u32;
    fn write8(&mut self, addr: u64, value: u8);
    fn write16(&mut self, addr: u64, value: u16);
    fn write32(&mut self, addr: u64, value: u32);
    fn write64(&mut self, addr: u64, value: u64);
    fn fence(&self) {}                       // mặc định no-op; platform tự cài
    fn notify(&self, addr: u64, queue: u16) { /* mặc định: write16 */ }
    fn read_bytes(&self, base: u64, out: &mut [u8]) { /* mặc định: từng byte */ }
}

pub trait DmaPool {
    fn base_pa(&self) -> u64;
    fn capacity(&self) -> usize;
    fn bytes(&mut self) -> &mut [u8];
    fn alloc(&mut self, len: usize, align: usize) -> Option<DmaBuf>;
    fn free(&mut self, buf: DmaBuf) -> bool;
    fn sync_for_device(&mut self, off: usize, len: usize); // BẮT BUỘC: bàn giao cho thiết bị
    fn sync_for_cpu(&mut self, off: usize, len: usize);    // BẮT BUỘC: nhận lại từ thiết bị
    fn read_u16(&self, off: usize) -> u16;                 // poll volatile
    fn read_u8(&self, off: usize) -> u8;
    fn zero(&mut self, buf: DmaBuf) { /* mặc định */ }
}
```

`sync_for_device`/`sync_for_cpu` là **hook bắt buộc** (không có mặc định): aarch64 cài bằng
`cache::HANDOFF_SEQUENCE` (bỏ bước `Notify` vì transport tự notify) + `cache::COMPLETE_SEQUENCE`
+ `invalidate_line` mỗi vòng poll; riscv/x86 cài no-op **có chủ đích** (DMA nhất quán). Nhờ vậy
KT-277 không bị mất khi gộp — và mất lời gọi là test host đỏ (§5.2-ii).

### 2.2 Transport PCI (`pcibus.rs`)

- `PciCfg` (đọc/ghi config 8/16/32) + `PciBus` (GAT `type Cfg<'a>: PciCfg`, `cfg(bus,dev,func)`,
  `evidence(bus,dev,func) -> (ecam_addr, ecam_pa, ecam_va)`);
- `PciPolicy { kind, scan: ScanRange, assign: DriverAssign, reserved, extra_conflict }` — riscv
  truyền `Some(mmu_range_conflict)` (walk bảng trang), aarch64/x86 `None`;
- `discover` (driver tự gán BAR) và `probe_assigned` (firmware đã gán — x86) dùng chung
  `probe_bar`/`choose_bar`/`caps_fit`/`enable_command`/`conflict_with_other_bars`;
- **một** `PciReport` hợp nhất (superset): `access, ecam_base, functions_seen, bus/dev/func,
  vendor/device, class: u32, revision, subclass, prog_if, header_type, bar_index, bar_kind,
  bar_size, bar_base, bar_assigned, bar_readback, command, caps, ecam_addr/pa/va, other_bars,
  reserved_ranges, overlap_checked, cap_ranges_ok`.

### 2.3 Driver blk (`blkdev.rs`)

`Transport::from_report(&PciReport)` + `Blk` (26 trường `AtomicU64`/`AtomicBool` để platform khai
`static BLK: Blk = Blk::new()` mà **không** `UnsafeCell`/`unsafe`) + `Clock { now, deadline,
spin_limit }`. Mọi stage lỗi giữ **đúng chuỗi cũ**: `layout, dma_align, reset, version1, features,
features_ok, queues, queue_size, queue_enable, driver_ok, config_generation, config, capacity`.

---

## 3. Bảng khác biệt ba bản **trước khi gộp**

### 3.1 `pci.rs`

| Khối | riscv64 (base, 504 dòng) | aarch64 (base, 610 dòng) | x86_64 (base, 927 dòng) | Sau KT-334 |
|---|---|---|---|---|
| Truy cập config | ECAM `0x3000_0000`, identity, `read_volatile` | ECAM PA `0x40_1000_0000`, **VA riêng** `0x8000_0000` qua `asm` | ACPI `MCFG` → ECAM, fallback legacy `0xCF8/0xCFC` | `Ecam`/`Function` cài `PciCfg`; `EcamBus`/`Access` giữ ở platform |
| Quét | bus 0, dev 0..32, func 0 | 256 bus × 32 dev × 8 func | `scan()` đọc mọi bus/function + RSDP/XSDT/MCFG | `ScanRange` trong `PciPolicy`; riêng x86 giữ `scan()`/`parse_mcfg` |
| BAR | tự ghi `0xffff_ffff`, mask, khôi phục, **gán** trong cửa sổ | như riscv | **firmware đã gán**: đọc lại + `mmio_mapped` | `discover` / `probe_assigned` |
| Chồng lấn | `RESERVED_RANGES` + BAR function khác + `mmu_range_conflict` | `RESERVED_RANGES` + BAR function khác | không (firmware) | `reserved` + `conflict_with_other_bars`; riscv thêm `extra_conflict` |
| Capability | `parse_caps` (đã ở lib từ trước) | same | same | gọi trong `pcibus`, thêm `caps_fit` fail-closed |
| Báo cáo | `PciReport` 25 trường | 27 trường | 25 trường (khác bộ) | **một** `PciReport` hợp nhất |
| `init_for` | ~160 dòng trùng aarch64 | ~150 dòng | ~70 dòng, kiểu khác | 3–6 dòng gọi `discover`/`probe_assigned` |

### 3.2 `virtio_blk.rs`

| Khối | riscv64 (base, 694 dòng) | aarch64 (base, 856 dòng) | x86_64 (base, 816 dòng) | Sau KT-334 |
|---|---|---|---|---|
| MMIO | `read_volatile` + `fence rw,rw` | `asm::mmio_*` + `dsb sy` | `asm::read/write_*` + `mfence` | `MmioBus` (3 cài đặt) |
| DMA | trang `.bss` identity | trang `.bss`, `high_to_pa`, `dc civac`/`dc ivac` + 7 bộ đếm | trang `.bss` identity | `DmaPool`; aarch64 cài cache hooks + đếm |
| Đồng hồ | `Riscv64Virt.now_ticks`, deadline `2*TIMEBASE_HZ` | `CNTPCT_EL0`/`CNTFRQ_EL0`, lùi `POLL_SPIN_LIMIT` | `now: &mut dyn FnMut()->u64` µs | `Clock` (3 cài đặt) |
| Trạng thái | `static mut` | `static mut` + `SAFETY` | `UnsafeCell` + `Sync` thủ công | `static BLK: Blk` toàn atomic, **không** `unsafe` |
| Evidence | `hex16/ascii16/sector0` riêng | riêng | riêng | dùng chung trong `blkdev` |
| Giao thức | ~500 dòng | ~650 dòng | ~600 dòng | **một** `Blk` 880 dòng (đã gồm test-hook + docs) |

---

## 4. Số dòng (đo bằng `wc -l`, không làm tròn %)

| Tệp | Base | Sau | Giảm |
|---|---:|---:|---:|
| `vifast-plat-riscv64/src/pci.rs` | 504 | 235 | −269 |
| `vifast-plat-riscv64/src/virtio_blk.rs` | 694 | 246 | −448 |
| **riscv64 cộng** | **1.198** | **481** | **−717 (−59,85 %)** |
| `vifast-plat-aarch64/src/pci.rs` | 610 | 317 | −293 |
| `vifast-plat-aarch64/src/virtio_blk.rs` | 856 | 342 | −514 |
| **aarch64 cộng** | **1.466** | **659** | **−807 (−55,05 %)** |
| `vifast-plat-x86_64/src/pci.rs` | 927 | 821 | −106 |
| `vifast-plat-x86_64/src/virtio_blk.rs` | 816 | 274 | −542 |
| **x86_64 cộng** | **1.743** | **1.095** | **−648 (−37,18 %)** |
| **TỔNG 6 tệp** | **4.407** | **2.235** | **−2.172 (−49,29 %)** |

DoD là **≥ 45 %** ⇒ đạt (49,29 %). x86_64 chỉ −37,18 % vì `pci.rs` còn **nguyên** bộ phân tích
ACPI (RSDP/XSDT/MCFG/EBDA) + quét legacy: phần đó **không** trùng arch nào khác, gộp vào lib sẽ
đẩy logic ACPI vào crate không liên quan virtio. Không cắt để đạt số.

Logic dùng chung mới: `pcibus.rs` 577, `blkdev.rs` 880, `dma.rs` 148, `mmio.rs` 57 — tổng 1.662
dòng (đều < trần R1 1.500 dòng/tệp), test host `tests/transport.rs` 1.000 dòng.

---

## 5. Test host + chứng minh test có răng (D2-05, KT-163)

Lệnh: `cargo test --manifest-path v2/Cargo.toml -p vifast-virtio --test transport`
⇒ **18 passed; 0 failed**.

### 5.1 Mười tám ca (nhóm theo yêu cầu KT-334)

| # | Tên ca | Chứng minh |
|---:|---|---|
| 1 | `init_sets_status_sequence_and_reports_offsets` | chuỗi status ACK→DRIVER→FEATURES_OK→DRIVER_OK, offset 4 cap, features |
| 2 | `init_rejects_device_without_version1` | thiếu `VIRTIO_F_VERSION_1` ⇒ `version1` |
| 3 | `init_rejects_torn_config_generation` | `config_generation` đổi giữa hai lần đọc ⇒ `config_generation` |
| 4 | `init_driver_features_always_include_version1` | driver **luôn** đặt bit 32 |
| 5 | `queue_setup_rejects_bad_size` | queue_max không luỹ thừa 2 / < 3 ⇒ `queue_size` |
| 6 | `setup_writes_desc_avail_used_layout` | desc/avail/used đúng offset `VqLayout` |
| 7 | `read_sector_completes_and_copies_data` | one-shot read, `used->id` khớp head |
| 8 | `write_sector_passes_data_to_device` | OUT buffer đúng nội dung |
| 9 | `submit_syncs_for_device_before_notify` | log sự kiện: `sync_for_device` **trước** `fence`/`notify` |
| 10 | `used_id_mismatch_is_reported` | thiết bị trả `used->id` sai ⇒ `used_id` |
| 11 | `flush_negotiates_feature_and_submits` | `VIRTIO_BLK_F_FLUSH` + request flush |
| 12 | `flush_without_feature_returns_unsupported` | chưa đàm phán ⇒ `unsupported` |
| 13 | `read_out_of_range_is_rejected_without_submit` | LBA ngoài dải ⇒ lỗi, **không** notify |
| 14 | `deadline_timeout_stops_poll` | `Clock::deadline` hết ⇒ `timeout` |
| 15 | `spin_limit_stops_poll_without_deadline` | `deadline=None` ⇒ trần vòng |
| 16 | `device_status_and_generation_are_live` | đọc sống `device_status`/`config_generation` |
| 17 | `dma_arena_alloc_is_lifo_and_aligned` | alloc/free LIFO, căn lề, hết chỗ |
| 18 | `sector0_evidence_hex_ascii_magic` | magic/hex/ascii của sector 0 |

### 5.2 Ba đột biến (sửa nguồn thật, chạy đúng ca, **khôi phục ngay**)

| # | Đột biến | Ca chạy | Kết quả |
|---:|---|---|---|
| i | `has_magic` bỏ so sánh từng byte (chỉ kiểm độ dài) | `sector0_magic_requires_exact_prefix` | **FAILED** (`test result: FAILED. 0 passed; 1 failed`) |
| ii | `submit` bỏ lời gọi `dma.sync_for_device(...)` | `submit_syncs_for_device_before_notify` | **FAILED** — `panicked ... đọc sector 0: Timeout` (thiết bị mô phỏng chỉ hoàn tất sau handoff ⇒ mạnh hơn đếm) |
| iii | bỏ **cả hai** chỗ kiểm `VIRTIO_F_VERSION_1` (device + `wanted`) | `init_rejects_device_without_version1` | **FAILED** (`0 passed; 1 failed`) |

Sau mỗi lần: `cp` khôi phục từ bản sao; `cargo test` lại 18/18 xanh.

---

## 6. Giữ nguyên TAP/API (không đổi hành vi)

- **riscv64** (`ktest-vblk`, 6 TAP): giữ `pci::{ECAM_BASE, PCI_MMIO_WINDOW_BASE,
  BAR_ASSIGN_BASE_CONSOLE/NET/INPUT/GPU, PciReport, VirtioKind, bar_kind_name, init, init_for,
  mmu_range_conflict, RESERVED_RANGES, SCAN_*}`; `virtio_blk::{init, stats, capacity_sectors()->u32,
  ready, read_sector, write_sector, flush, read_sector_force, device_status, config_generation,
  isr_addr, dma_page_addr}`.
- **aarch64** (TAP 27): giữ `pci::{bar_assign_base, bar_kind_name, ecam_pa, ecam_va, Ecam,
  EcamProbe, PciFail, PciReport, BAR_ASSIGN_BASE/BYTES, ECAM_BASE/MAP_BYTES/VA_BASE,
  PCI_MMIO_WINDOW_BASE/BYTES, RESERVED_RANGES, SCAN_*, probe_ecam, has_cap_list, is_multifunction,
  init, init_for, VirtioKind}`; `virtio_blk::{init(&PciReport, dma_pa), high_to_pa, cache_stats,
  CacheStats, sector0_evidence, hex16, ascii16, has_magic, BlkReport, Sector0, SECTOR0_MAGIC,
  SECTOR_BYTES}`. `# dma: cache-clean ok=1 lines>0 inval_lines>0 lines*line_bytes==bytes` giữ nguyên
  (bộ đếm đếm chính lệnh `dc` adapter phát ra).
- **x86_64** (TAP 30): giữ `pci::{init_for, scan, ScanReport, Access, AccessKind, PciReport,
  PciFail, bar_kind_name, class_name}`; `virtio_blk::{init, read_sector/write_sector/flush/
  read_sector_force với &mut dyn FnMut()->u64, bar_disjoint, sector0_evidence, SECTOR_BYTES,
  BlkReport, POLL_TIMEOUT_US, SECTOR0_MAGIC}`.
- `PciReport.class` đổi `u8` → `u32` (hợp nhất, aarch64 vốn in `class=0x{:06x}`); mọi nơi đọc
  `class` đều in hex 24 bit nên không đổi chuỗi.
- x86 `report.bar_base` vẫn là trường `pub` **sửa được** (kbin đổi sang BAR của net); `AccessKind`
  thêm `code()` (0=ECAM, 1=legacy) chỉ để điền `PciReport.access: u8`.

---

## 7. Kết quả cổng

### 7.1 `./v2/bin/v2 check`

```
v2 check: PASS (L1 chạy trong gate --fast)
# 46 bước OK, 0 bước FAIL — log /tmp/kt334-check4.log
```

### 7.2 QEMU riscv64 — `bash v2/test/boot/ktest.sh 1 matrix` (smp 1/2/4, 9 ảnh = 27 boot)

```
# smp-list: 1 2 4 — NBOOT=1 — timeout=20 s/boot — 9 ảnh/boot (plan đọc từ expected.txt)
REV-GATE: ACCEPT inputs=52d6e811355bd7a1 current_inputs=52d6e811355bd7a1 head_rev=23ff5b5b14a1 count=337
ok - ktest-vblk smp=1 boot 1 (6 TAP, 1s)
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

### 7.3 QEMU x86_64 — `bash v2/test/boot/run-x86_64.sh`

```
ok 1 - elf: entry=_start, .text tại 0x100000, multiboot2 + PVH note đúng
ok 2 - boot 1: TAP 33/33 + 5 dòng '# exc:' + marker 1 lần + halt sạch
# ca âm: 29/29 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 32/32 kiểm sạch (boot=1, tests=33, exc=5)
```

### 7.4 QEMU aarch64 — `bash v2/test/boot/run-aarch64.sh`

```
ok 1 - boot 1: TAP 35/35 + marker 1 lần + '# dma: cache-clean ok=1'
# ca âm: 24/24 ĐỎ đúng như phải
# 27/27 kiểm sạch (boot=1, tests=35)
# infra: guard_log=…/k4-aarch64/qemu-guard.log qemu_own=0
```

### 7.5 `.data+.bss` mỗi ảnh (trần 262.144 B/ảnh, không nới)

| Ảnh | `.data+.bss` | Nguồn |
|---|---:|---|
| riscv64 `ktest-vblk` | 11.064 | dòng `# image=ktest-vblk` của matrix |
| riscv64 sản phẩm (`vifast-kbin-riscv64`) | 196.104 | `bash v2/test/boot/budget.sh <ảnh>` |
| x86_64 sản phẩm (`vifast-kbin-x86_64`) | 176.408 | `llvm-size -A` + luật `.data*/.bss*` (không có script riêng) |
| aarch64 sản phẩm (`vifast-kbin-aarch64`) | 233.704 | `bash v2/test/boot/budget-aarch64.sh <ảnh>` |

### 7.6 `rust-version`

`v2/kernel/Cargo.toml`: `1.99.1` → **`1.100.0`** (khớp `v2/Cargo.toml` 1.100.0 và
`v2/rust-toolchain.toml`; không đổi toolchain thực tế — bản sửa chỉ đồng bộ khai báo).

---

## 8. Thay đổi phụ (do đồng bộ MSRV)

`rust-version` lên `1.100.0` làm clippy bật các lint MSRV-gated trên **mã cũ** của kernel; sửa
cơ học, hành vi không đổi, để `cargo clippy -D warnings` vẫn sạch trên MSRV mới:

| Tệp | Sửa |
|---|---|
| `vifast-plat-aarch64/src/virtio_gpu.rs` | `FB_PHYS_BASE % (2<<20) == 0` → `.is_multiple_of(...)` |
| `vifast-plat-x86_64/src/sched.rs` | `rsp % 8` → `rsp.is_multiple_of(8)` (2 chỗ) |
| `vifast-plat-x86_64/src/virtio_net.rs` | `#[allow(clippy::too_many_arguments)]` cho `udp_frame` (8 tham số, đã có từ trước) |

---

## 9. Ghi chú trung thực / rủi ro còn lại

1. **x86_64 giảm ít nhất** (−37,18 %): `pci.rs` vẫn 821 dòng vì ACPI (RSDP/XSDT/MCFG/EBDA) +
   legacy CF8/CFC + quét đa bus **chỉ có ở x86_64**. Gộp phần này vào lib là trái phạm vi
   (không liên quan virtio) và không giúp arch khác. Không cắt để đạt %.
2. **Ba adapter blk vẫn ~240–330 dòng** mỗi bản: phần còn lại là thứ **không thể** gộp —
   đường MMIO/asm của arch, PA/alias + cache maintenance, nguồn thời gian. Đây là biên D2-17.
3. **Đột biến (ii)** đỏ bằng `Timeout` trước khi tới dòng đếm `device_syncs==1`: thiết bị mô
   phỏng chỉ hoàn tất sau handoff, nên đây là bằng chứng **chức năng** (không chỉ đếm). Ca
   `submit_syncs_for_device_before_notify` vẫn có assert thứ tự `sync < fence < notify` trên log.
4. **Aarch64 cache**: QEMU TCG không mô hình hoá hệ quả của `dc civac` (như ghi chú KT-277);
   bằng chứng trên QEMU là **chuỗi lệnh `dc` đã phát** (`lines>0`, `lines*line_bytes==bytes`) —
   đúng tầng quan sát được, không nói quá.
5. **QEMU rev-gate (KT-86)** đòi cây sạch: các runner chỉ chạy **sau** commit; log runner ghi
   `rev` của commit code. Nếu journal được sửa sau đó, đó là thay đổi **chỉ tài liệu**, không
   thuộc `inputs` của `# build:`.
6. Không sửa `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`; không push/merge; không
   chạm `main`; không nới ngưỡng nào.

---

## 10. Việc chưa làm (ngoài phạm vi KT-334)

- Chưa gộp `virtio_net`/`console`/`input`/`gpu` lên `MmioBus`/`DmaPool` — KT-334 chỉ yêu cầu
  blk + PCI; các driver đó vẫn dùng `vifast-virtio` phần device-agnostic như cũ.
- x86_64 ACPI parser vẫn ở platform (xem §9.1).
