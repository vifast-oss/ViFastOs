# KT-72 (K6) — virtio-blk **thật** trên QEMU `virt`, **chỉ trong ảnh test `ktest-vblk`**

PCI ECAM → virtqueue → `BlockDev` → FS trên đĩa. Ảnh **sản phẩm không có driver** (feature
`vblk` chỉ bật cho ảnh test; KT-73 mới mở), nên mọi câu "virtio-blk thật" trong tài liệu này
đều hiểu là **trên ảnh test `ktest-vblk`**, không phải trên ảnh sản phẩm.

Trạng thái: **DONE (chờ reviewer)** — Agent 2 dòng OS, 2026-11-16/17.
Nguồn sự thật: code + log trong `v2/agent/JOURNAL/kt72-*` (D2-109C: mọi số trong tài liệu này
phải xuất hiện **nguyên văn** trong một log **đã nằm trong repo**; kiểm bằng
[`kt72-verify-numbers.sh`](../agent/JOURNAL/kt72-verify-numbers.sh)).

## 1. Phạm vi và quyết định

- **Không mở `spec/`, không thêm syscall**: toàn bộ KT-72 nằm trong
  `kernel/vifast-plat-riscv64` (MMIO/PCI, có `unsafe` — D2-17), `lib/vifast-virtio` (logic thuần,
  `#![no_std] #![forbid(unsafe_code)]`, test host) và ảnh test mới `vifast-kbin-ktest-vblk`.
- **`vifast-fs` / `vifast-vfs` KHÔNG sửa** (đúng yêu cầu task): virtio-blk chỉ **thêm một `impl
  BlockDev`** ([`ktest_vblk.rs:59`](../kernel/vifast-kbin-riscv64/src/ktest_vblk.rs)) trên trait
  đã có (`lib/vifast-fs/src/fs.rs`). Pager/page cache/journal dùng lại nguyên vẹn.
- **Chọn ECAM, không legacy I/O port**: RISC-V **không có** lệnh `in/out`; "I/O port" trên `virt`
  chỉ là cửa sổ MMIO `gpex_ioport_window` mà vẫn phải đi qua `0xCF8/0xCFC` hai bước. ECAM là
  **một** phép đọc/ghi 32 bit cho mỗi thanh ghi (`bus<<20 | dev<<15 | func<<12 | reg`), không cần
  khoá tuần tự. Lý do ghi ở đầu [`pci.rs`](../kernel/vifast-plat-riscv64/src/pci.rs).
- **Driver chạy kernel-mode**, `unsafe` chỉ trong `kernel/vifast-plat-*`: `read_volatile`/
  `write_volatile` cho MMIO (`pci.rs`, `virtio_blk.rs`), `asm!("fence rw, rw")`
  ([`virtio_blk.rs:217`](../kernel/vifast-plat-riscv64/src/virtio_blk.rs)) và `UnsafeCell` cho
  trang DMA; **không** lệnh đặc quyền nào ở U-mode (D2-101 không áp cho driver, vì driver không
  nằm ở shim).
- **Chỉ ảnh test**: feature mới `vblk = ["ktest"]` trong `vifast-plat-riscv64`; ảnh **sản phẩm**
  không có driver (KT-73 sẽ mở cho sản phẩm). Hệ quả: `.text` ảnh sản phẩm **không đổi** so với
  KT-71 và 12 boot matrix cũ **không đổi dòng QEMU**. Kiểm bằng `strings`: ảnh sản phẩm có 0 lần
  `virtio`/`vblk`.
- **Vì sao phải tách ảnh (xung đột VA thật)**: TAP 35 của KT-52 map thử 4 KiB tại **cùng VA
  `0x4000_0000`** ([`ktest_obj.rs:31`](../kernel/vifast-kbin-riscv64/src/ktest_obj.rs)) trong khi
  driver map **cửa sổ PCIe 2 MiB tại đúng VA đó** — nếu driver nằm trong ảnh `ktest` thì TAP 35
  đỏ (`AlreadyMapped`). Driver vì vậy nằm ở ảnh riêng `ktest-vblk`.

## 2. Dữ kiện nền tảng QEMU `virt` (đọc từ máy ảo, không phải hằng thiết kế)

`info pci` / `info mtree` của QEMU (CPU đóng băng `-S`, tức **trước** khi kernel chạm BAR) —
log đã chép: `JOURNAL/kt72-qemu-probe.log`:

```
BAR0: I/O (not mapped)
BAR1: 32 bit memory (not mapped)
BAR4: 64 bit prefetchable memory (not mapped)
0000000030000000-000000003fffffff (prio 0, container): alias pcie-ecam @pcie-mmcfg-mmio 0000000000000000-000000000fffffff
0000000040000000-000000007fffffff (prio 0, container): alias pcie-mmio @gpex_mmio_window 0000000040000000-000000007fffffff
```

- ECAM ở `0x3000_0000` (256 MiB, bus<<20); driver **map 2 MiB** đầu (đủ bus 0/1) — hằng
  `ECAM_BASE`, `ECAM_MAP_BYTES` ([`pci.rs:32`](../kernel/vifast-plat-riscv64/src/pci.rs)).
- Cửa sổ MMIO PCIe `0x4000_0000..0x7fff_ffff` (1 GiB); driver gán BAR modern vào **đầu cửa sổ**
  (`BAR_ASSIGN_BASE`, [`pci.rs:42`](../kernel/vifast-plat-riscv64/src/pci.rs)).
- **Không có firmware enumerate**: `info pci` trong lúc boot cho
  `BAR0: I/O (not mapped)`, `BAR1: 32 bit memory (not mapped)`, `BAR4: 64 bit prefetchable memory
  (not mapped)` — nên driver **tự** đo cỡ và gán BAR (§3).

## 3. PCI: quét + đo BAR + gán BAR + capability

1. **Quét**: bus 0, device 0..31, function 0; `vendor_id = 0xffff` ⇒ không có thiết bị. Chấp nhận
   `(0x1af4, 0x1001)` và `(0x1af4, 0x1042)` (modern-only cùng driver). Log
   `# vblk pci: functions=2 … vendor=0x1af4 device=0x1001` (host bridge `1b36:0008` chiếm một
   function).
2. **Đo cỡ BAR** (PCI §6.2.5.1): ghi `0xffff_ffff` vào **cả hai** dword (BAR 64-bit) ⇒ đọc mask
   ⇒ `size = (!mask)+1` tính **trong đúng độ rộng** (32 bit cho BAR I/O/32-bit, 64 bit cho BAR
   64-bit). Cỡ phải là luỹ thừa 2; mask 0/rác ⇒ `None` (**không đoán**). Logic + test host:
   [`vifast-virtio/src/pci.rs`](../../lib/vifast-virtio/src/pci.rs) (`bar_probe`), test
   `bar_mask_thanh_co_va_kieu`.
3. **Chọn** BAR memory lớn nhất ⇒ với `virtio-blk-pci` là **BAR4** (`kind=mem64 size=16384`).
4. **Gán**: ghi địa chỉ `0x4000_0000`, đọc lại **che 4 bit thuộc tính** (bit 3 prefetchable, bit
   2:1 kiểu) — so sánh trên địa chỉ, không trên raw. `readback=0x40000000` khớp `assigned`.
5. **Bật** `command.MEMORY|BUS_MASTER` (DMA cần bus master), đọc lại `command=0x6`.
6. **Kiểm chồng lấn TRƯỚC khi ghi BAR** (CAO-2/D2-110C): (i) đo BAR memory **đã có địa chỉ**
   của mọi function khác trên bus 0 rồi so chồng lấn; (ii) so với **vùng nền tảng** (UART/PLIC/
   ECAM/RAM) và (iii) kiểm **trang đã map** mà không phải leaf cửa sổ PCIe của chính driver ⇒
   **fail-closed** (`err=BarConflict`, stage = `other_bar`/`reserved`/`mmu`), không đè im lặng.
   `readback == assigned` chỉ chứng minh **ghi được**, **không** chứng minh **không đè**.
7. **Capability list** tại `0x34`: `virtio_pci_cap` (id `0x09`): `cfg_type` 1 common, 2 notify,
   3 ISR, 4 device; `bar`/`offset`/`length`; notify có `notify_off_multiplier` ở `cap+16`. Chain
   PCI **không bắt buộc** offset tăng dần ⇒ chống vòng bằng tập đã thăm (bài học đo được: bản
   đầu từ chối `next` lùi ⇒ `CapLoop` sai, log `kt72-red-cap-loop.log`). Kết quả
   `common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4`.

## 4. Virtqueue (split) + feature + request

- **Feature**: đọc `device_features` hai trang (`device_feature_select` 0/1) ⇒
  `device_features=0x10130006e54`; **đàm phán** `VIRTIO_F_VERSION_1` (bit 32) **và**
  `VIRTIO_BLK_F_FLUSH` (bit 9, `0x200`) **khi thiết bị có** ⇒ `driver_features=0x100000200`,
  đọc lại, rồi đặt `FEATURES_OK` và **đọc lại** `status=0xf` (thiết bị giữ bit ⇒ không từ chối).
  **CAO-1**: trước bản vá, driver gửi `T_FLUSH` mà `driver_features` chỉ có bit 32 — vi phạm
  spec §5.2.3 (QEMU tha, thiết bị thật không chắc). Nay `flush()` **từ chối** (`Unsupported`)
  nếu chưa đàm phán; quyết định nằm ở hàm thuần `negotiated_features` (test host). Bằng chứng
  `flush_feature=1`.
- **Queue 0**: `num_queues=1`, `queue_max=256`; driver đặt `queue_size=8` (luỹ thừa 2, ≤ max), ghi
  `queue_desc`/`queue_driver`/`queue_device` = **PA** (trang DMA, `VA == PA` — TAP kiểm bằng
  `walk_sw`), bật `queue_enable=1`, đọc lại; đặt `DRIVER_OK`.
- **Bố cục ring** trong **một trang 4 KiB** `.bss` (căn lề 4096) — tính từ `queue_size`, không
  hardcode (`VqLayout`, [`vifast-virtio/src/queue.rs`](../../lib/vifast-virtio/src/queue.rs)):
  `desc 16*N` @0, `avail` (flags 2 + idx 2 + ring 2N + used_event 2), `used` căn 4 (flags 2 +
  idx 2 + ring 8N + avail_event 2); sau đó mới tới request header (16 B), đệm 512 B và byte
  status. Các offset này được **assert const** lúc biên dịch.
- **Chuỗi descriptor**: `IN`/`OUT` = header (`NEXT`, device-readable) → đệm 512 B (`NEXT` [+
  `WRITE` nếu đọc]) → status (`WRITE`); `FLUSH` = header → status (không đệm).
- **Thứ tự bàn giao** (spec §2.7.13): ghi descriptor → ghi `avail->ring` → **fence** → ghi
  `avail->idx` → **fence** → ghi index vào **cửa thông báo** (`notify + notify_off*multiplier`) →
  poll `used->idx` (volatile) → **fence** → đọc `used->elem` + status.
- **Kiểm `used->id`**: phải bằng head descriptor (0). Lệch ⇒ `id_mismatch` + lỗi, **không im
  lặng**. Dung lượng/LBA: `read_sector`/`write_sector` **kiểm dải trước** (`OutOfRange`, không
  phát request); `read_sector_force` chỉ dùng cho ca âm của TAP.
- Timeout poll = 2 s ảo (`rdtime`) ⇒ `Timeout`; K6 **chưa dùng ngắt** virtio (poll), queue depth
  thực tế 1. `queue_max` **phải ≥ 3** (chain header+data+status) và `used` lấy offset theo
  `queue_size` **thật** — trước bản vá dùng hằng `QUEUE_SIZE` và chỉ đòi ≥ 2 (D2-110 medium).

## 5. FS **thật** trên đĩa virtio

`impl BlockDev for VirtioDev` gọi thẳng driver: `read`→`T_IN`, `write`→`T_OUT`, `flush`→`T_FLUSH`,
`block_count` = capacity **đọc từ thiết bị** (`capacity=8192` sector × 512 B = 4 MiB). Mã lỗi ánh
xạ **một chỗ**: `OutOfRange → FsError::Invalid`, còn lại → `Io`.

Pha TAP 5 trên QEMU:
1. `format_with_options` trên đĩa virtio (ghi thật qua `T_OUT`) rồi `initialize` (mount);
2. tạo/ghi `/kt72.txt` (payload 69 byte), `fsck` sạch, `fsck_repair` **`found=0 fixed=0
   unfixed=0`**;
3. **drop `Fs`** rồi mount **lại từ đĩa** (Fs mới ⇒ mọi block đọc lại qua thiết bị), đọc file
   **so byte** (`content_match=1 read_len=69`), `fsck` sạch;
4. đọc **thô** LBA 0/1 ngoài mọi cache: **driver đọc được LBA 0** với nội dung zero đúng như
   `format` ghi ([`fs.rs:638`](../../lib/vifast-fs/src/fs.rs)) — `lba0_zero=1`; **superblock/magic
   `VF04` nằm ở LBA 1** ([`fs.rs:645`](../../lib/vifast-fs/src/fs.rs)) —
   `lba1_magic=VF04 lba1_version=4 lba1_total_blocks=8192`. TAP đọc **cả hai** và in cả hai.
   (Mô tả task ban đầu ghi "sector 0 đúng magic của FS" là **không đúng layout**: LBA 0 là zero
   có chủ đích, magic ở LBA 1.)

**Kiểm độc lập phía HOST** (`JOURNAL/kt72-host-fsck.log`): ảnh đĩa QEMU để lại được công cụ host
đọc: `vifast-fsutil fsck` exit 0, `ls /` thấy `kt72.txt`, `get /kt72.txt` trả **đúng** payload
kernel đã ghi. Đây là bằng chứng dữ liệu **thật sự** ra thiết bị, không chỉ nằm trong RAM của
guest.

## 6. TAP (ảnh `ktest-vblk`, 6 test)

| # | Tên | Bằng chứng (log trong repo) |
|---|---|---|
| 1 | `vblk pci` | `functions=2 … vendor=0x1af4 device=0x1001 bar=4 kind=mem64 size=16384 assigned=0x40000000 readback=0x40000000 command=0x6`; `common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4 … cap_ranges_ok=1 overlap_checked=1 other_bars=0 reserved_ranges=5` |
| 2 | `vblk queue` | `capacity=8192 blk_size=512 size_max=0 seg_max=254 num_queues=1 queue_max=256 queue_size=8 device_features=0x10130006e54 driver_features=0x100000200 flush_feature=1 status=0xf`; `identity=1 version1=1` |
| 3 | `vblk ghi doc lai` | 2 lượt × 8 sector: `writes=16 reads=16 compared=8192 (=reads(16)*512) mismatched=0` |
| 4 | `vblk flush` | `calls=2 flushes=2 completions=2 device_status=0xf driver_ok=1 errors=0 timeouts=0` |
| 5 | `vblk fs tren dia` | `phase=write formatted=1 wrote=69 fsck_clean=1 found=0 fixed=0 unfixed=0 const:payload_len=69`; `phase=remount read_len=69 content_match=1 fsck_clean=1`; `lba0_zero=1 lba1_magic=VF04 lba1_version=4 lba1_total_blocks=8192` |
| 6 | `vblk loi ngoai dai` | `refused=OutOfRange refused_no_request=1 (=requests_after(131)-requests_before(131)) forced=Io forced_requests=1 … recovered=1 … errors_before=0 errors_after=1 errors_delta=1 (=after-before)`; **`id_mismatch=0 timeouts=0` chỉ là quan sát, KHÔNG nằm trong điều kiện `ok`** — `const:id_timeout_proven=0` (chưa có ca âm, D2-108B) |

Ca âm có **hai** lớp: (a) driver từ chối **trước** khi phát request (không treo, `requests`
không tăng); (b) **ép** phát request vượt dung lượng ⇒ thiết bị trả `IOERR` thật
(`forced=Io`), bộ đếm `errors` **tăng đúng 1** (D2-108B: bộ đếm lỗi có ca dương, không phải hằng
0); sau lỗi thiết bị vẫn phục vụ đọc hợp lệ (`recovered=1`).

Boot kết thúc bằng **một** dòng `PANIC: KT-72 vblk xong (6 TAP, 0 fail)` có chủ đích (ảnh này
không fault; `ktest.sh` đòi đúng 1 PANIC mỗi boot) rồi `halt(0)`.

## 7. Đỏ trước + đột biến (mutation)

- **Host (D2-05)**: chạy test mới trên cây chưa có crate ⇒ **không biên dịch**
  (`E0432 … could not find features/pci/queue`), `JOURNAL/kt72-red-host.log`.
- **QEMU (thiết kế sai, đo được)**: bản đầu từ chối capability `next` lùi ⇒ `err=CapLoop`;
  sau khi sửa, lỗi **so BAR raw** (chưa che bit thuộc tính) ⇒ `err=BarUnassigned stage=readback`
  — raw boot log đầy đủ `JOURNAL/kt72-red-boot-cap-loop.log` + `kt72-red-boot-bar-readback.log`
  (đều có `TAP version 13`); `kt72-red-cap-loop.log` là **bản trích** (ghi rõ ở đầu tệp).
- **Mutation host**: đổi `avail_ring_off` lệch 2 byte ⇒ 2 test host **đỏ**
  (`left: 130 / right: 132`), `JOURNAL/kt72-mutation-host.log`.
- **Mutation QEMU**: bỏ kiểm dải trong `read_sector` ⇒ TAP 6 **`not ok`**
  (`refused=Io refused_no_request=0`), `JOURNAL/kt72-mutation-qemu.log` — chứng minh ca âm
  TAP 6 **không tautology**.

## 8. Cổng và ngân sách

- `v2 check` PASS; `gate --fast` **PASS 718 test/99 suite** (703/96 của KT-71 + 15 test host
  mới), `L1: OK (900 ms)`; L1 **thuần** 897/885/1107 ms (`kt72-l1-thuan.log`, D2-100: số cổng
  ≠ số thuần, ghi cả hai).
- **Matrix 15/15 boot sạch** = 4 ảnh cũ (12/12 như KT-71) + ảnh mới `ktest-vblk` × smp 1/2/4
  (`kt72-ktest-matrix.log`); conformance **61/61 dòng + 5/5 ca** (`kt72-conformance.log`);
  clippy host (trong gate) + kernel **6 ảnh `EXIT=0`** (`kt72-clippy-kernel.log`);
  **soak 0/1000** trên **ảnh sản phẩm** (`kt72-soak.log`).
- Ảnh `ktest` **242840 B** — **không đổi** so với KT-71 (driver không nằm trong ảnh này);
  ảnh mới `ktest-vblk` **10976 B**; trần **262144 B** không nới. Ảnh sản phẩm `data+bss`
  **200184 B**, `.text` digest `02ed5a1810ba59ce text=49854` = KT-71 (ELF sha256 đổi chỉ vì
  metadata build, `.text` y hệt — D2-108A: hash lấy từ **build sạch không feature**).

### 8.1 Dòng log **nguyên văn** tương ứng (mọi số ở trên lấy từ đây)

```
# kt72-budget-ktest.log
# image=ktest … data+bss=242840 budget=262144 stack=65536
# kt72-budget-vblk.log
# image=ktest-vblk … data+bss=10976 budget=262144 stack=65536
# kt72-budget-product.log
# image=product … data+bss=200184 budget=262144 stack=65536
# kt72-gate-summary.log
TESTS=718
SUITES=99
# kt72-gate.log
# L1: OK (900 ms, đo bởi cổng)
# kt72-l1-thuan.log
# L1 thuan lan 1: 897 ms
# L1 thuan lan 2: 885 ms
# L1 thuan lan 3: 1107 ms
# kt72-ktest-matrix.log
KTEST: 15/15 boot sạch
# kt72-conformance.log
trace-compare: 61/61 dòng khớp, 5/5 ca
# kt72-soak.log
# soak: 0/1000 lỗi (PASS)
# soak: BUILD_ID=02ed5a1810ba59ce sha256=2e9550bdb6b4948399b76628de680f744869e344245aac9a88449f96575572d0
# kt72-core-static.log
# user core: static=188872 pin=196608 image_cap=262144
# kt72-baseline-summary.log
TESTS=703
SUITES=96
# kt72-qemu-probe.log
BAR4: 64 bit prefetchable memory (not mapped)
alias pcie-mmio @gpex_mmio_window
```

## 9. Nợ / giới hạn (ghi rõ, không giấu)

1. **Poll, chưa ngắt**: chưa dùng MSI-X/INTx; queue depth thực tế 1. Đủ cho FS tuần tự.
2. **`flush` ở mức thiết bị**: request `FLUSH` được thiết bị hoàn tất (`status=OK`,
   `flushes=2`), nhưng hiệu ứng `fsync` trên **host** không quan sát được từ guest. KT-75
   (`test-persist` 7 pha qua reboot) là nơi kiểm bền vững thật; KT-72 chỉ chứng minh request
   thật + dữ liệu đọc lại được **bằng công cụ host**.
3. **Bộ đếm `id_mismatch`/`timeouts` chưa có ca âm** (D2-108B) ⇒ "chưa chứng minh". Chúng **đã
   bị bỏ khỏi điều kiện `ok` của TAP 6** (evidence in `const:id_timeout_proven=0`) để không ai
   đọc `ok 6` thành bằng chứng cho hai bộ đếm này. Bộ đếm `errors` **có** ca dương (ca ép +
   mutation bỏ kiểm dải).
4. **Một bus/một function**, BAR gán cố định đầu cửa sổ, map leaf 2 MiB — đủ cho `virt`, chưa
   phải PCI enumerator tổng quát (không cấp phát động, không xử lý bridge).
5. Ảnh đĩa 4 MiB do runner tạo mới mỗi boot; capacity vẫn đọc từ thiết bị, nhưng driver chưa hỗ
   trợ sector ≠ 512 (`decode_config` từ chối).
6. Ảnh **sản phẩm chưa có driver** (feature `vblk` chỉ ảnh test) — KT-73 mới mở; vì vậy soak
   KT-72 không đi qua virtio (đúng yêu cầu "`-drive` không làm hỏng soak"). Kiểm: ảnh sản phẩm
   có 0 lần `virtio`/`vblk` trong `strings`.
7. Ảnh `ktest-vblk` có `# vblk` evidence nhưng **không** có `# conf image:` digest (không chạy
   `check_trace` của ảnh `ktest`) — nếu KT-73/75 cần neo ảnh thì thêm cùng lúc với TAP.
8. **Coverage tích hợp KHÔNG có**: không boot nào chạy **driver + 70 TAP kernel**, **driver +
   U-mode**, hay **driver + conformance** cùng lúc. Tách ảnh giữ nguyên coverage **cũ** (12 boot
   matrix như trước) nhưng **không** thêm coverage tổ hợp — đó là việc của KT-73/75.
9. **Kiểm chồng lấn BAR chưa đầy đủ**: đã kiểm BAR function khác + vùng nền tảng + trang đã map,
   nhưng **không** đọc bản đồ thiết bị từ DT/`ranges` và **không** kiểm vùng chưa map có thật sự
   trống (chỉ tin cửa sổ PCIe là của PCI). Nếu QEMU/machine đổi layout, driver phải đổi hằng
   `PCI_MMIO_WINDOW_BASE`/`BAR_ASSIGN_BASE`; đây là **nợ thật** cho KT-73 (parse DT).
