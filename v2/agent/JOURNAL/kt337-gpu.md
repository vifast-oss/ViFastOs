# KT-337 — `virtio_gpu` + `virtio_input` trên transport chung, bật **GPU 2D x86_64**

- **Chu kỳ 2, Đợt 1** — nhánh `task/kt337-gpu`, gốc `main` @ `9895b596`.
- Phạm vi: (a) gộp logic giao thức GPU/input của riscv64 + aarch64 vào
  `v2/lib/vifast-virtio`, adapter nền tảng mỏng; (b) **mở virtio-gpu 2D trên x86_64** với bằng chứng
  `screendump` PPM thật; (c) hai đột biến đỏ.
- Luật đã đọc: `AGENTS.md` §4/§6, `v2/agent/DECISIONS.md` (D2-17, D2-231, D2-239, D2-235(5)),
  `KE-HOACH-CHU-KY-2-2026-11-22.md` KT-337, `v2/docs/k6-test-invariants.md`,
  `v2/agent/JOURNAL/kt334-virtio.md`. Không sửa `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
  `v2/bin/v2`; không nới ngưỡng; không `#[ignore]`; `git add` theo đường dẫn cụ thể.

## 1. Khảo sát **trước** khi sửa

### 1.1 Kích thước bốn driver cũ

| Tệp | Dòng |
|---|---:|
| `vifast-plat-riscv64/src/virtio_gpu.rs` | 771 |
| `vifast-plat-aarch64/src/virtio_gpu.rs` | 905 |
| `vifast-plat-riscv64/src/virtio_input.rs` | 702 |
| `vifast-plat-aarch64/src/virtio_input.rs` | 904 |
| **Tổng** | **3.282** |

### 1.2 Trùng lặp theo **chức năng** (không chỉ theo dòng)

Hai bản GPU gửi **cùng sáu lệnh** với **cùng hằng số layout**: `GET_DISPLAY_INFO`,
`RESOURCE_CREATE_2D`, `RESOURCE_ATTACH_BACKING` (19 mảnh 64 KiB, mảnh cuối ngắn), `SET_SCANOUT`,
`TRANSFER_TO_HOST_2D` (offset = `Rect::byte_offset(stride)`), `RESOURCE_FLUSH`; cùng chuỗi
status/feature (`VERSION_1`), cùng split virtqueue 16 descriptor, cùng vòng `submit → avail →
notify → poll used → id/len`. Khác nhau **chỉ**: địa chỉ MMIO, PA framebuffer, hàm fence/cache, cách
đọc đồng hồ. So khớp thân hàm (chuẩn hoá khoảng trắng) giữa hai bản cũ:

| Nhóm | Trùng **tên + thân** | Trùng tên, khác thân (khác địa chỉ/cache/đồng hồ) |
|---|---|---|
| GPU riscv64 ↔ aarch64 | 11 hàm: `queue_with`, `resp_type`, `check_nodata`, `get_display_info`, `create_resource`, `attach_backing`, `set_scanout`, `set_scanout_raw`, `probe_bogus_scanout`, `resource_flush`, `flush_rect` | 18 hàm: `init`, `submit`, `transfer_to_host_2d`, `stats`, `page`, `read*`, `write*`, `fence`, `framebuffer`, `ready`, `isr_addr`, `device_status`, `published` |
| Input riscv64 ↔ aarch64 | 4 hàm: `name`, `select_config`, `read_config_payload`, `queue_with` | 20 hàm: `init`, `arm_one`, `arm_all`, `poll_event`, `poll_event_wait`, `stats`, `page`, `read*`, `write*`, `fence`, `ready`, `isr_addr`, `device_status`, `config_generation`, `published` |

Ngoài ra mỗi bản còn **chép tay** bảng mã lệnh/`Rect`/layout descriptor; `vifast-virtio` đã có
`gpu::{Rect, CtrlHdr, ResourceAttachBacking, SetScanout, TransferToHost2d, …}` và
`input::{KeyState, key_to_ascii, EVENT_BUFS}` nhưng **chưa** có driver dùng transport — đó là phần
bị lặp thật sự (không phải cấu trúc dữ liệu).

### 1.3 `gpu.sh` (riscv64) lái QEMU/PPM thế nào

1. Build `vifast-kbin-ktest-gpu --features ktest,gpu`; kiểm `.data+.bss` bằng
   `budget.sh` (trần 262.144 B), chạy `qemu_guard.sh` (process group + timeout).
2. QEMU `-M virt -device virtio-gpu-pci -display none` + **HMP monitor trên unix socket**.
3. Chờ dòng **marker của khách** `# gpu: console done lines=3` (không `sleep` mù), gửi
   `screendump <ppm>` qua HMP, đọc prompt.
4. So **ảnh chụp** bằng `gpu_ppm.py`: 640×480; hình chữ nhật đỏ `(40,40,200,120)` (4 điểm trong,
   6 điểm ngoài, biên chính xác); `"ViFast"` trắng tại `(40,200)` = 122 pixel; ba dòng console
   `ViFastOs`/`gpu console`/`0123456789` tại `(8,8)` so **đủ 64 pixel/ký tự** với bảng font
   `FONT_8X8` **đọc từ mã nguồn kernel** (không chép bản thứ hai); ca "không lem".
5. Ca âm: bỏ `-device virtio-gpu-pci` ⇒ ảnh in `# gpu: no device` + TAP 1 đỏ, không PASS giả.
6. So TAP `1..6`, đủ `ok 1..6`, 0 `not ok`, marker/halt đúng một lần.

`input.sh` tương tự (bàn phím, `sendkey a`/`shift-a`/`f1`, `# input name: …`), `run-aarch64.sh`
ghép cả hai vào một phiên monitor (PPM + `sendkey`) và đòi bộ đếm cache `clean_lines>0`,
`inval_lines>0`, `fb_clean_lines>0`.

### 1.4 x86_64 thiếu gì để có GPU 2D

| Thiếu | Cách xử lý |
|---|---|
| Không có `virtio_gpu` adapter trên `vifast-plat-x86_64` | viết mới (283 dòng) dùng `asm::read*/write*`, `fence(SeqCst)` |
| Không có pha GPU trong `vifast-kbin-x86_64` (34 TAP) | thêm `src/gpu.rs` + feature `gpu` ⇒ TAP 35, ảnh thường **vẫn 34** |
| Đồng hồ: pha cuối chạy sau `cli`, không có tick LAPIC | truyền closure PIT kênh 2 (`PitClock`) như `virtio_blk`, mọi hàm nhận `now: &mut dyn FnMut() -> u64` |
| BAR do **firmware** gán (khác riscv/aarch64 tự đặt) | không ghi đè BAR; đọc `assigned`, kiểm `readback == assigned`, kiểm rời BAR net |
| `screendump` HMP trên `-M q35` trả **720×400** (console VGA mặc định, không phải virtio-gpu) | thêm `-vga none` ⇒ console duy nhất là virtio-gpu, PPM 640×480 |
| Framebuffer phải **identity** để thiết bị DMA đọc đúng dải PA | đặt `FB_PHYS_BASE = 0x0200_0000` (32 MiB, trong 1 GiB identity, trên ảnh/arena, dưới RAM 128 MiB); hằng `const _: () = assert!(…)` |

## 2. Thiết kế **sau**

- `v2/lib/vifast-virtio/src/gpudev.rs` (812 dòng) — driver GPU 2D **thuần**, `#![forbid(unsafe_code)]`,
  viết trên trait `MmioBus`/`DmaPool`/`Clock`, trạng thái `AtomicU64/U16` (khuôn `Blk`) nên adapter
  **không** cần `UnsafeCell` cho state. `transfer_to_host_2d` tính offset **một chỗ**:
  `rect.byte_offset(FB_STRIDE)`; `submit` gọi `sync_for_device` **trước** `fence`+`notify`.
- `v2/lib/vifast-virtio/src/inputdev.rs` (800 dòng) — driver input **thuần**, cùng trait; tham số
  `with_status_queue` (riscv `false`, aarch64 `true`) để giữ nguyên hợp đồng TAP từng nền tảng;
  `arm_all` bàn giao **một lần** cho 8 đệm.
- `v2/lib/vifast-virtio/tests/devices.rs` (880 dòng) — thiết bị giả host + **12 test**: feature/queue,
  `used_id` lệch, deadline, attach 19 mảnh, offset, thứ tự `sync_for_device` trước `notify` (GPU +
  input), console/status queue. Không `unsafe` (R5).
- Adapter mỏng còn lại (chỉ phần nền tảng):

| Tệp | Dòng sau | Nội dung còn lại |
|---|---:|---|
| `vifast-plat-riscv64/src/virtio_gpu.rs` | 270 | MMIO volatile, `fence rw,rw`, physmap framebuffer, `Tick` |
| `vifast-plat-aarch64/src/virtio_gpu.rs` | 409 | `dc civac`/`dc ivac` + bộ đếm, `CNTPCT_EL0`, alias nửa cao → PA, `gpu_framebuffer` |
| `vifast-plat-riscv64/src/virtio_input.rs` | 226 | MMIO, DMA identity, `Tick` |
| `vifast-plat-aarch64/src/virtio_input.rs` | 362 | cache maintenance + bộ đếm, `CNTPCT_EL0`, `high_to_pa` |
| `vifast-plat-x86_64/src/virtio_gpu.rs` | **283 (mới)** | `asm::read*/write*`, `fence(SeqCst)`, DMA identity, `PitClock` |
| `vifast-kbin-x86_64/src/gpu.rs` | **444 (mới)** | pha GPU 2D (TAP 35), giữ khung 500 ms cho `screendump` |
| `v2/test/boot/gpu-x86_64.sh` | **364 (mới)** | runner PPM x86_64 |

## 3. Đo lường

| Hạng mục | Trước | Sau | Thay đổi |
|---|---:|---:|---:|
| Tổng 4 driver riscv+aarch64 | 3.282 | 1.267 | **−2.015 (−61,4 %)** |
| Trong đó logic dùng chung ở lib | 0 | 1.612 | +1.612 (một bản duy nhất) |
| Adapter nền tảng riscv+aarch64 | 3.282 | 1.267 | −61,4 % |
| Adapter x86_64 GPU (mới) | 0 | 283 | nền tảng **mới**, không phải chép |
| Test host `devices.rs` | 0 | 880 (12 test) | mới |
| `.data+.bss` ảnh x86_64 **mặc định** (34 TAP) | 177.440 (dựng lại đúng `9895b596` bằng `git worktree`, đo `budget.sh`) | 177.440 | **0 B** |
| `.data+.bss` ảnh x86_64 `--features gpu` (35 TAP) | — | **186.080** | +8.640 B so với mặc định, **dưới trần 262.144** |

Tính theo tệp: GPU riscv 771→270 (−65,0 %), GPU aarch64 905→409 (−54,8 %), input riscv 702→226
(−67,8 %), input aarch64 904→362 (−60,0 %). Dòng "trước" lấy từ `git show 9895b596:<tệp> | wc -l`.

## 4. Bằng chứng QEMU (nguyên văn)

### 4.1 riscv64 — `bash v2/test/boot/gpu.sh` (rc=0)

```text
ok 1 - ca dương: 6/6 TAP + PPM đúng màu tại toạ độ đã vẽ + 29/29 ký tự console
ok 2 - ca âm: ảnh in 'gpu: no device' + TAP 1 đỏ, không PASS giả
# gpu_ppm: 23 kiểm, 0 lỗi (…/kt263-positive.ppm)
```

Các dòng bằng chứng trong log khách:

```text
# gpu dev: num_queues=2 queue_max=64 queue_size=16 … version1=1 device_status=0xf num_scanouts=1 num_capsets=0
# gpu display: w=1280 h=800 enabled=1 scanouts=1
# gpu resource: id=1 format=1 w=640 h=480 entries=19 chunk=65536
# gpu scanout: id=0 resource=1 w=640 h=480 … bogus=InvalidResourceId
# gpu draw: rect=40,40,200,120 color=0xff0000 text=ViFast at=40,200 …
# gpu console scroll: lines=60 scrolls=1 row0=1 row_last=1 ok=1
```

### 4.2 aarch64 — `bash v2/test/boot/run-aarch64.sh` (rc=0)

```text
# 27/27 kiểm sạch (boot=1, tests=35)
```

Gồm PPM 23/23 và các ca cache: `fb_clean_lines`/`clean_lines`/`inval_lines` dương, `matched=29/29`.
24 ca âm đều ĐỎ đúng (`# ca âm: 24/24 ĐỎ đúng như phải`).

### 4.3 x86_64 GPU — `bash v2/test/boot/gpu-x86_64.sh` (rc=0)

```text
# gpu pci: bus=0 dev=3 func=0 vendor=0x1af4 device=0x1050 bar=4 kind=mem64 size=0x4000 assigned=0xfebfc000 readback=0xfebfc000 command=0x0107
# gpu dev: num_queues=2 queue_max=64 queue_size=16 … version1=1 device_status=0x0f num_scanouts=1 num_capsets=0
# gpu dma: desc=0x150000 avail=0x150100 used=0x150128 cmd=0x150200 resp=0x150600 dma=0x150000 notify_off=0 notify_addr=0xfebff000 queue=0
# gpu fb: pa=0x2000000 identity=1 bytes=1228800 pages=300 stride=2560 attach_entries=19 hold_ms=500
# gpu display: w=1280 h=800 enabled=1 scanouts=1
# gpu resource: id=1 format=1 w=640 h=480 entries=19 chunk=65536
# gpu scanout: id=0 resource=1 w=640 h=480 host_w=1280 host_h=800 enabled=1 bogus=InvalidResourceId
# gpu draw: rect=40,40,200,120 color=0xff0000 text=ViFast at=40,200 fg=0xffffff bg=0x000000 stride=2560
# gpu: drawn
# gpu console: chars=29 newlines=3 scrolls=0 dirty=8,8,88,24 flushes=1 commands=2 matched=29/29 text=ViFastOs|gpu console|0123456789
# gpu stats: commands=10 notifies=10 errors=0 timeouts=0 id_mismatch=0 rejects=1 published=10 queue_size=16 device_status=0x0f
# gpu: console done lines=3
ok 35 - gpu: 2d ppm
ok - gpu_ppm.py PASS (23 kiểm)
```

PPM 640×480: đỏ `(40,40,200,120)`, `"ViFast"` 122 pixel trắng tại `(40,200)`, console 29/29 ký tự
khớp bảng font. Runner kiểm đủ TAP `1..35`, marker/halt đúng một lần, 12 dòng `# gpu*` đúng một
lần, `.data+.bss` qua `budget.sh`.

### 4.4 Không hồi quy

```text
bash v2/test/boot/input.sh      → ok 1/ok 2 (5/5 TAP + 'a'/'A'/mã lạ + bộ đếm sạch; ca âm đỏ đúng)
bash v2/test/boot/run-x86_64.sh → 32/32 kiểm sạch (boot=1, tests=34, exc=5); 31 ca âm đỏ đúng
./v2/bin/v2 check               → rc=0 (fmt, clippy, R1..R5, toàn bộ lane)
```

## 5. Hai đột biến (đỏ **trước**, khôi phục sau)

### (i) `TRANSFER_TO_HOST_2D.offset = 0`

Sửa **một chỗ** trong `gpudev::transfer_to_host_2d` thành `offset = 0`, build lại ảnh x86_64
`--features gpu`, chạy runner:

```text
not ok - ppm: console: ký tự 0 dòng 2 '0' tại (8,24) không khớp bitmap 8×8
…
not ok - ppm: console: chỉ 0/29 ký tự khớp bitmap 8×8
# gpu_ppm: 22 kiểm, 30 lỗi (…/kt337-gpu.ppm)
# gpu-x86_64.sh: 1 lỗi
```

Điểm mấu chốt: **TAP 35 vẫn `ok`** (`ok 35 - gpu: 2d ppm`) vì TAP đọc lại framebuffer trong RAM;
chỉ **PPM** (thiết bị đọc DMA) thấy console lệch — chứng minh phép kiểm PPM có răng và đúng lớp lỗi
KT-271. Test host tương ứng cũng đỏ: `left: Some(0), right: Some(20512)`.

### (ii) Bỏ `sync_for_device` trước `notify`

- **Host**: bỏ lệnh trong `gpudev::submit` + `inputdev::handoff_and_notify` ⇒
  `gpu_submit_syncs_for_device_before_notify` đỏ (`create: "timeout"`) và
  `input_submit_syncs_for_device_before_notify` đỏ (`sync_for_device phải chạy TRƯỚC notify`).
- **QEMU aarch64**: thay thân `Dma::sync_for_device` của adapter input bằng rỗng ⇒ runner đỏ:

```text
not ok 2 - boot 1: # dòng '# input stats:' có clean_lines=0 — không có lệnh 'dc civac' nào chạy trên vùng DMA
```

- **x86_64**: cache nhất quán, `sync_for_device` là no-op **có chủ đích** — đột biến này **không
  quan sát được** trên x86_64; đó là lý do bằng chứng đỏ lấy từ host test + adapter aarch64 (bộ đếm
  `dc`), không phải PPM x86_64. Đã khôi phục nguyên trạng và chạy lại xanh.

## 6. Cổng / kiểm đã chạy

| Lệnh | Kết quả |
|---|---|
| `cargo test -p vifast-virtio --test devices` | 12/12 xanh (đỏ-trước: `/tmp/kt337-red-host.log`) |
| `cargo test -p vifast-plat-aarch64 --test virtio_gpu --test virtio_input` | 7/7 + 6/6 |
| `./v2/bin/v2 check` | **rc=0** (fmt, clippy, R1..R5, test-teeth, toàn bộ lane) |
| `bash v2/test/boot/gpu.sh` | **rc=0** (6/6 TAP + PPM 23/23 + ca âm) |
| `bash v2/test/boot/input.sh` | **rc=0** |
| `bash v2/test/boot/run-x86_64.sh` | **rc=0** (32/32, tests=34) |
| `bash v2/test/boot/run-aarch64.sh` | **rc=0** (27/27, PPM + cache) |
| `bash v2/test/boot/gpu-x86_64.sh` | **rc=0** (TAP 35, PPM 23/23, `-vga none`) |

Mỗi lần một QEMU, qua `qemu_guard.sh`; không `pkill -f qemu-system`.

## 7. Giới hạn / chưa làm

- Chưa có runner **ca âm "không GPU"** riêng cho x86_64 (PPM dương + ảnh gốc 34 TAP đã phủ "không
  bật feature thì không đổi"); ca âm không-GPU đã có ở riscv64/aarch64.
- x86_64 `sync_for_device` là no-op (cache nhất quán) — đột biến (ii) chỉ quan sát được ở aarch64 +
  host test, đã ghi rõ ở §5.
- Không đụng `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`; không push/merge.
