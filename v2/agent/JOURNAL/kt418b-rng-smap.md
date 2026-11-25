# KT-418b — `virtio-rng` + `SMAP` `stac`/`clac` + ca âm `-cpu max` (phần 2 của KT-418)

- **Task**: KT-418b (P1, Chặng 2 / W7) — phụ thuộc KT-418 (DONE `db3e21bb`).
- **Nhánh**: `task/kt418b-rng-smap` · **Worktree**: `/private/tmp/vifast-wt/2`
- **Base**: `main` @ `88d7473b` (đã có KT-418/415/411b).
- **Ngân sách**: **2 h** (lead rút từ 3 h). Bắt đầu 2026-11-26 06:19 JST (21:19 UTC — Thấp điểm chính).
- **Căn cứ**: `QUEUE.md` dòng 471, `KE-HOACH-KIEN-TRUC.md` dòng KT-418b, `JOURNAL/kt418-sec1.md` §7.3
  (nợ `virtio-rng`) + §8.4 (bài học `PAN`/`SMEP`).

## 0. Kế hoạch ngắn

1. **Đo hiện trạng**: `grep -rn "rng" v2/lib/vifast-virtio/src/` rỗng (không có mã); `vifast-core`
   không có API entropy; `SMAP` đã **có** đường bật (`asm::fpu::enable_smep_smap`) nhưng **cố ý tắt**
   (`smap_deferred`) vì kernel đọc/ghi trang `US=1` từ CPL0 ở nhiều đường.
2. **Đỏ-trước (1)**: `v2/lib/vifast-virtio/tests/rng.rs` gọi `vifast_virtio::rng`/`rngdev` (chưa có)
   ⇒ **không biên dịch được** (đỏ đúng lý do: module chưa tồn tại).
3. **Đỏ-trước (2)**: `v2/kernel/vifast-core/src/entropy_tests.rs` gọi `crate::entropy` (chưa có).
4. **Cài (a)**: `vifast-virtio/src/rng.rs` — device id (`0x1005`/`0x1044`), feature, queue 0, cổng
   **chất lượng entropy** (`entropy_ok`/`distinct_bytes`/`longest_run`), mã lỗi có tên (D2-102).
5. **Cài (b)**: `vifast-virtio/src/rngdev.rs` — driver ring **dùng chung 3 arch** trên
   `MmioBus`+`DmaPool` (khuôn `netdev`/`blkdev` của KT-334/335): `init` → `fill` → `report`,
   fail-closed (thiếu thiết bị / timeout / entropy rác ⇒ `Err`, không PANIC).
6. **Cài (c)**: `vifast-core/src/entropy.rs` — `EntropyPool` (trạng thái 256 bit + bộ đếm, trộn ARX
   kiểu `SipRound`, `no_std`, `forbid(unsafe_code)`), `seed` fail-closed, `fill` **chỉ** khi đã seed.
7. **Cài (d)**: x86_64 — `VirtioKind::Rng` + `virtio_rng.rs` adapter + probe trong ảnh (in bằng chứng
   `# rng: ...`); **không** thêm TAP vào ma trận cổng (không sửa `expected.txt`/`run-x86_64.sh` —
   luật task), bằng chứng dương tính chạy tay bằng QEMU riêng có `-device virtio-rng-pci`.
8. **Ưu tiên 2 — `SMAP`**: kiểm **toàn bộ** đường kernel chạm trang `US=1`, bọc `stac`/`clac` rồi bật.
9. **Ưu tiên 3 — ca âm `-cpu max`**: **KHÔNG** đổi baseline cổng; ghi kế hoạch + bằng chứng chạy tay.
10. `./v2/bin/v2 t0` sau mỗi bước → `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `spec/`; không nới ngưỡng; không `#[ignore]`/`#[should_panic]`; không
`git add/commit`; chỉ T0 + T1; không sửa `expected.txt`/`run-*.sh`/`accept()`; 1 QEMU của tôi.

## 1. Đo hiện trạng (trước khi sửa)

```text
$ grep -rn "rng\|Rng" --include=*.rs v2/lib/vifast-virtio/src/ | wc -l
0
$ grep -rn "entropy\|Entropy" --include=*.rs v2/kernel/vifast-core/src/ | wc -l
0
$ grep -n "smap_deferred" -A 3 v2/kernel/vifast-plat-x86_64/src/asm/fpu.rs
    pub const fn smap_deferred(&self) -> bool { self.cpuid_smap && !self.smap }
```

Kết luận: `virtio-rng` **chưa có một dòng mã**; `vifast-core` **chưa có** API entropy; `SMAP` có
đường bật nhưng **cố ý tắt** (nợ có tên từ KT-418 phần 1).

## 2. Đỏ-trước (1) — `vifast_virtio::rng`/`rngdev` chưa tồn tại

```text
$ cargo test --manifest-path v2/Cargo.toml -p vifast-virtio --test rng
   Compiling vifast-virtio v0.1.0 (/private/tmp/vifast-wt/2/v2/lib/vifast-virtio)
error[E0432]: unresolved import `vifast_virtio::rng`
  --> lib/vifast-virtio/tests/rng.rs:32:20
   |
32 | use vifast_virtio::rng::{
   |                    ^^^ could not find `rng` in `vifast_virtio`

error[E0432]: unresolved import `vifast_virtio::rngdev`
  --> lib/vifast-virtio/tests/rng.rs:37:41
   |
37 | use vifast_virtio::rngdev::{Clock, Rng, RNG_BUF_BYTES, RNG_BUF_OFF, RNG_RING_OFF};
   |                                     ^^^^^^ could not find `rngdev` in `vifast_virtio`

error: could not compile `vifast-virtio` (test "rng") due to 2 previous errors
```

Đỏ **đúng lý do**: hai module chưa tồn tại (không phải test sai kỳ vọng).

## 3. Đỏ-trước (2) — `vifast_core::entropy` chưa tồn tại

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core --lib
error[E0583]: file not found for module `entropy`
error: could not compile `vifast-core` (lib test) due to 1 previous error
```

Đỏ **đúng lý do**: tệp module chưa có.

## 4. Cài đặt — `virtio-rng`

### 4.1 `vifast-virtio/src/rng.rs` (thuần) + `rngdev.rs` (driver dùng chung 3 arch)

- **Device id**: `VIRTIO_RNG_MODERN = 0x1005` (transitional), `VIRTIO_RNG_MODERN_ONLY = 0x1044`.
- **Hàng đợi**: `Q_REQUEST = 0`, `QUEUES_REQUIRED = 1`, `QUEUE_SIZE = 8` (luỹ thừa 2, ≥ 2).
- **Request**: một descriptor **device-writable** (`DescFlags::WRITE`) trỏ đệm `RNG_BUF_OFF` trong
  trang DMA; `used->len` = số byte thiết bị đã ghi (spec §5.4.2 — không có header request).
- **Cổng chất lượng** (`entropy_ok`): `len ≥ 16`, `distinct_bytes ≥ 8`, `longest_run ≤ 8`. Đây là
  cổng **thô** chặn đúng lớp "thiết bị trả toàn 0" — không phải kiểm thống kê.
- **Fail-closed**: `NotReady` (chưa init), `BadLen` (**trước** khi chạm ring), `Timeout` (+ đếm),
  `Io` (`used->id`/`len` vô lý), `LowEntropy` (mẫu rác — `bytes` **không** tăng), `Version1`,
  `Features`, `FeaturesOk`, `Queues`, `QueueSize`, `QueueEnable`, `DriverOk`, `NoDevice`.
- **Thứ tự spec §2.7**: ghi descriptor → `push_avail` → `fence` → `sync_for_device` → notify →
  poll `used->idx` → `fence` → `sync_for_cpu` → đọc `used->elem`.

### 4.2 `vifast-core/src/entropy.rs` — API entropy của core

- `EntropyPool`: trạng thái 256 bit (4 word) + bộ đếm khối; mỗi khối 32 B chạy 4 vòng `SipRound`
  (ARX) sau khi trộn bộ đếm vào `state[0]`.
- `seed(bytes)`: `< 32 B` ⇒ `SeedTooShort`; **mọi byte giống nhau** ⇒ `SeedDegenerate` (lớp thứ hai
  sau cổng của driver — API core **không** tin người gọi); seed sau cùng **thay thế** seed trước.
- `fill(out)`: chưa seed ⇒ `NotSeeded` và `out` **không** đổi một byte.
- **Nợ có tên**: hàm trộn **không** phải CSPRNG đã được đánh giá; ba bất biến đo được (không lặp
  giữa hai lần gọi, 4 KiB không lặp khối 8 B, 64 KiB phủ đủ 256 giá trị byte) có test host. Khi cần
  entropy **mật mã**, thay hàm trộn bằng ChaCha20 — **không** đổi hợp đồng `seed`/`fill`.

### 4.3 Nối vào ảnh x86_64 — **đã viết, đo được, rồi GỠ theo quyết định lead**

- `vifast-plat-x86_64/src/virtio_rng.rs` — adapter MMIO + **một trang DMA riêng** (4 KiB `.bss`,
  identity map) + đồng hồ PIT do ảnh cấp; toàn bộ giao thức ở crate thuần.
- `vifast-kbin-x86_64/src/rng.rs` — probe: tìm thiết bị → init → lấy 64 B → `EntropyPool::seed` →
  `fill` 32 B → in `# rng: ...`. **Không** phát TAP (runner không gắn `virtio-rng-pci`; luật task
  không sửa runner/`expected.txt`) ⇒ trên cổng đây là **ca âm fail-closed** `# rng: no device`.
- `VirtioKind::Rng` thêm vào `vifast-virtio/src/pci.rs`; hai nền tảng còn lại thêm nhánh
  `bar_assign_base` (`riscv64`: leaf 6 `BAR_ASSIGN_BASE_RNG`; `aarch64`: leaf 5) — **bắt buộc**,
  nếu không `cargo check --workspace --target riscv64gc-unknown-none-elf` **ĐỎ** (E0004
  non-exhaustive). Đây là bước `wake theo dep-info` của `v2 check` bắt được (xem §7.1).

> **GỠ 2026-11-26 (lead quyết định (b))**: bước `api boundary (KT-241)` ĐỎ vì tệp
> `v2/kernel/vifast-plat-x86_64/src/virtio_rng.rs` khớp mẫu `virtio_*` của ratchet B1.1
> (`driver_files=24, đóng băng 23, mới 1`). Ratchet **chỉ được giảm**; thêm tệp driver phải qua
> Agent 0. Lead chọn **không** sửa baseline và **không** đổi tên tệp để né mẫu ⇒ adapter + probe
> **đã gỡ khỏi cây**; bản đã đo được lưu ở `/tmp/kt418b-virtio_rng.rs.removed` +
> `/tmp/kt418b-kbin-rng.rs.removed` (và nội dung đầy đủ ở §5 — log QEMU thật). **Đề xuất task
> riêng**: xin Agent 0 duyệt thêm **một** dòng vào `driver_files` của
> `v2/test/baseline/b1.1-ratchet.toml` rồi dựng lại adapter (≈ 20 phút, mã đã có, đã đo xanh).
> Phần **giữ lại**: crate thuần `vifast-virtio::{rng,rngdev}` + `vifast-core::entropy` +
> `VirtioKind::Rng` + nhánh `bar_assign_base` 2 nền tảng + toàn bộ SMAP.

### 4.4 **Sai số đo được**: device id **`0x1005`**, không phải `0x1004`

Bản đầu của tôi suy `0x1004` từ "device id 4" của spec. Boot thật in `# rng: no device` dù QEMU
**có** `-device virtio-rng-pci`. Đo lại bằng monitor QEMU 11.1.3:

```text
$ printf 'info pci\nquit\n' | qemu-system-x86_64 -M q35 -m 64M -display none \
    -device virtio-rng-pci -monitor stdio | grep 1af4
    Class 0255: PCI device 1af4:1005          # transitional
$ ... -device virtio-rng-pci,disable-legacy=on ...
    Class 0255: PCI device 1af4:1044          # modern-only
```

Bảng id PCI **legacy** của virtio **không** cộng đều `0x1000 + device_id`: `0x1004` là
`virtio-scsi`, rng là **`0x1005`**. Đã sửa hằng số + test (thêm `0x1004` vào danh sách "không được
nhận nhầm") + ghi bằng chứng đo ngay tại chỗ khai báo. **Bài học**: hằng số "suy từ số hiệu thiết
bị" phải đối chiếu **thiết bị thật** trước khi tin.

## 5. Bằng chứng `virtio-rng` chạy thật (QEMU, ảnh ktest x86_64)

Lệnh (một QEMU của tôi, có `-device virtio-rng-pci`; đĩa 8 sector đúng chữ ký `VIFASTOS-KT288-S0`
để các TAP blk không đỏ trước):

```bash
IMG=$CARGO_TARGET_DIR/x86_64-unknown-none/release/vifast-kbin-x86_64
qemu-system-x86_64 -M q35 -cpu qemu64 -m 128M -nographic -no-reboot -smp 4 \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
  -drive file=/tmp/kt418b-disk.img,if=none,id=d0,format=raw -device virtio-blk-pci,drive=d0 \
  -device virtio-rng-pci -kernel $IMG
```

```text
# rng: device=0x1005 version1=1 queue=8 bytes=64 distinct=60 run=1 entropy_ok=1 notifies=1 timeouts=0 errors=0 seeded=1 derived=1 pool_bytes=32 pool_blocks=1
...
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

Đọc: 64 B **từ thiết bị** (không phải hằng), **60** giá trị byte khác nhau, run dài nhất **1** ⇒
qua cổng chất lượng; **1** doorbell, **0** timeout, **0** lỗi; `EntropyPool` seed được và cấp 32 B
(`pool_blocks=1`). Boot **hoàn tất** (marker + halt) ⇒ thiết bị tuỳ chọn **không** làm hỏng boot.

**Ca âm trên cổng** (không có thiết bị — đúng cấu hình `run-x86_64.sh`): `# rng: no device`, boot
tiếp tới marker (xem log T1 §8.3).

## 6. Cài đặt — `SMAP` x86_64 (`stac`/`clac`)

### 6.1 Kiểm **toàn bộ** đường kernel chạm trang `US=1`

| Đường | Truy cập | Kết luận |
|---|---|---|
| `umode.rs` phép đo `INVLPG` (2 lần đọc `VSPACE_PROBE_VA`) | **VA user** (`US=1`) | **phải bọc** |
| `syscall_env::read_user_words` (message/`mr` từ vùng user) | **VA user** (`US=1`) | **phải bọc** |
| loader ELF / ghi frame user (`userload.rs`, `shellpath.rs`, `umode.rs:233`) | **PA** trên identity map (`US=0`) | **không** cần — SMAP chỉ chặn trang `US=1` |
| frame syscall trên stack kernel (`umode.rs:528`, `shellpath.rs:1040`) | stack kernel (`US=0`) | **không** cần |
| `sec_probe::probe_read8` | VA user, `AC=0` | **cố ý không bọc** — đây là **ca âm** của SMAP |

### 6.2 Cài

- `asm/smap.rs`: `stac()`/`clac()` (không `preserves_flags` — hai lệnh **đổi** `RFLAGS.AC`) +
  guard `#[must_use] UserAccess::begin()` tự `clac` khi drop (một chỗ quên `clac` là `AC=1` rò
  sang đường sau và SMAP thành vô nghĩa).
- Bọc 3 điểm: `umode.rs` (2) + `syscall_env.rs` (1).
- `enable_smep_smap`: bật `CR4.SMAP` **chỉ khi** `CPUID.07H:EBX` báo có (model `qemu64` của cổng
  **không** có ⇒ ghi mù sẽ `#GP`); `hardened()` nay gồm **cả** SMAP.

### 6.3 Bằng chứng `-cpu max` (CPU **có** SMAP)

```text
# vspace: remap va=0x1000004000 before=0x1111222233334444 after=0x5555666677778888 expect=(0x1111222233334444,0x5555666677778888) invlpg_ok=1
# smep/smap: va=0x1000000000 cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 smap_deferred=0 cr4=0x0000000000300020 smap_err=0x1 smap_cr2=0x1000000000 smap_ok=1 smep_err=0x11 smep_cr2=0x1000000000 smep_ok=1 hardened=1
ok 50 - smep/smap: bat va ca am
```

- `smap=1`, `smap_deferred=0`, `cr4=0x300020` (bit 20 SMEP + bit 21 SMAP) — **đã bật thật**.
- `smap_err=0x1` (P=1, **đọc**, CPL0) + `smap_cr2=0x1000000000` ⇒ ca âm **thật**: đọc trang `US=1`
  từ CPL0 với `AC=0` **fault** đúng địa chỉ. Trước KT-418b `smap_ok` chỉ đúng vì SMAP **tắt**
  (`smap_err=0x0`) — tức phép kiểm cũ **không** chứng minh gì.
- `invlpg_ok=1` ⇒ phép đo `INVLPG` (đường **đã bọc**) vẫn chạy đúng dưới SMAP.

**Đường syscall dưới SMAP** — đo bằng ảnh **sản phẩm-shell** trên `-cpu max` (ảnh ktest chết ở
một lỗi **có sẵn** trước đó, xem §6.4):

```text
$ cargo build ... --features product-shell,script-input   # rồi boot -cpu max
# shell: exit code=0 state=0 puts=57 bytes=469 hash=0x88bc7924cb0d9ae4 prompt=1 help=1 echo=1 exit_line=1 ps=1 wait=1 kill=1 syscalls=88 yields=8 user_cs=1 switches=8 ...
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

**88 syscall** (mỗi `DebugPutStr` đọc vùng user qua `read_user_words` — đường **đã bọc**) chạy
xong, boot hoàn tất ⇒ đường syscall **được kiểm thật** dưới SMAP, không chỉ đọc mã.

### 6.4 `-cpu max` — lỗi **có sẵn**, không phải hồi quy của KT-418b

Ảnh **ktest** trên `-cpu max` dừng **sau** TAP 50, tại phép kiểm cô lập `VSpace`:

```text
# exc: vec=14 err=0x6 rip=0x1000000027 cs=0x1b rflags=0x10202
# exc: cr2=0x1000003000
# exc: frame rsp=0x14cf50 ss=0x23 task_rsp=0x1000002000
```

Trên `qemu64` **cùng** fault đó được xử lý như **ca mong đợi**:
`# vspace: cross fault vec=14 err=0x6 cr2=0x1000003000 cs=0x1b rip=0x1000000027 user=1` rồi
`ok 12 - umode: ring3 entered`. Trên `-cpu max` khung ngắt in bằng đường **chung** (`# exc:`) và
boot dừng — tức hook "fault mong đợi" **không** khớp. Khác biệt đáng chú ý: `rflags=0x10202`
(**TF=1**) trong khung `-cpu max`.

Đây là hành vi **trước** KT-418b: mã của tôi (probe `rng`, `stac`/`clac`) **không** được chạm tới
trong lần boot đó (fault xảy ra ở phép kiểm `VSpace`, trước `net::run`), và log `-cpu max` **trước**
khi tôi sửa SMAP có **cùng** dãy dòng cuối. Vì vậy:

- **KHÔNG** đổi baseline cổng (đúng luật task) — và nay có **bằng chứng cụ thể** rằng đổi baseline
  sang `-cpu max` **không** phải một dòng: boot hiện **chết** ở phép kiểm cô lập `VSpace` vì lý do
  chưa điều tra (nghi `RFLAGS.TF` bị đặt trong đường `iretq`/`#DB`). **Cần lead quyết** + một task
  riêng điều tra `-cpu max` trước khi bàn đổi baseline.

## 7. T0 / `v2 check` / T1

### 7.1 `v2 check` — lần đầu **ĐỎ** 3 bước (đã sửa, ghi lại vì là bài học)

```text
v2 check: 3 bước FAIL
--- wake theo dep-info (KT-217: lớp 2): FAIL
E E_DEPINFO_BUILD   đơn vị `kernel`: cargo check RC=101
--- api boundary (KT-241): FAIL
--- workspace v2/kernel: clippy host-testable (KT-409): FAIL
```

Nguyên nhân **thật** (tái lập bằng tay):

```text
$ cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf
error[E0004]: non-exhaustive patterns: `VirtioKind::Rng` not covered
   --> vifast-plat-aarch64/src/pci.rs:110:22
```

Thêm `VirtioKind::Rng` vào enum của crate thuần ⇒ **mọi** `match` trên nó ở 3 nền tảng phải có
nhánh mới. Đã thêm (`riscv64`: `BAR_ASSIGN_BASE_RNG` leaf 6; `aarch64`: leaf 5). Đây đúng bài học
KT-418 §8.4(3): bước `wake theo dep-info` build **workspace kernel cho target riscv64** nên nó bắt
được lỗi của crate arch khác.

### 7.2 T0 — `./v2/bin/v2 t0 vifast-virtio vifast-core`

```text
--- T0: fmt (cargo fmt --all --check)
--- T0: clippy -p vifast-virtio (v2/lib/vifast-virtio/Cargo.toml)
--- T0: test -p vifast-virtio
--- T0: clippy -p vifast-core (v2/kernel/vifast-core/Cargo.toml)
--- T0: test -p vifast-core
v2 t0: PASS (2 crate; **T0** — KHÔNG phải cổng; cổng = gate --dev (T1) / gate --fast (T2))
```

Lần chạy **trước** đó T0 **ĐỎ** đúng một lỗi clippy trong test mới:
`error: this assertion has a constant value ... clippy::assertions_on_constants` ⇒ đổi hai
`assert!` hằng thành `const _: () = assert!(...)` (kiểm ở **compile time**). Không nới lint.

Test mới: `vifast-virtio --test rng` **10 ca**; `vifast-core --lib` **126 ca** (gồm 8 ca
`entropy_tests`).

### 7.3 `v2 check` — bước `api boundary` ĐỎ vì **cây bẩn** (đúng thiết kế, không phải hồi quy)

Lần chạy **cuối** (sau mọi thay đổi): `v2 check: 1 bước FAIL` — đúng bước `api boundary (KT-241)`,
`# step-time-total 64213 ms`. 47 bước còn lại **OK**.

```text
$ ./v2/bin/v2 check
# B1.1 ratchet: dep_edges=3 (đóng băng 3, mới 0) driver_files=23 (đóng băng 23, mới 0) stale=0
# B1.1: XANH (0 vi phạm, ratchet một chiều)
== KT-241 api boundary (compile-fail) — /private/tmp/vifast-wt/2 ==
check-api-boundary: TỪ CHỐI — tệp sẽ chạm đã bẩn — từ chối chạy (fail-closed), không ghi đè:
M v2/kernel/vifast-core/src/lib.rs
v2 check: 1 bước FAIL
```

Nguyên nhân: harness compile-fail **nối** module probe vào `vifast-core/src/lib.rs` rồi khôi phục;
nó **từ chối** khi tệp đó **bẩn** (`check-api-boundary.py:657`). Worktree của agent **luôn** bẩn
(lead commit sau) và KT-418b **phải** sửa `vifast-core/src/lib.rs` (thêm `pub mod entropy;`) ⇒ bước
này chỉ xanh **sau khi lead commit**. **Không** nới ngưỡng, **không** tắt bước, **không** retry để
lấy xanh (D2-07). Cùng lớp với `selftest pre-measure (KT-95)` của KT-418 phần 1 (`REFUSE
reason=tree-dirty`).

### 7.4 T1 — `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev`

```text
--- api boundary (KT-241)
check-api-boundary: TỪ CHỐI — tệp sẽ chạm đã bẩn — từ chối chạy (fail-closed), không ghi đè:
M v2/kernel/vifast-core/src/lib.rs
# B1.1 ratchet: dep_edges=3 (đóng băng 3, mới 0) driver_files=23 (đóng băng 23, mới 0) stale=0
# B1.1: XANH (0 vi phạm, ratchet một chiều)
--- api boundary (KT-241): FAIL
# step-time-total    66414 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**48 bước OK, đúng 1 bước FAIL** — và bước đó **đỏ vì cây bẩn, đúng thiết kế** (§7.3), không phải
hồi quy. Vì nó nằm ở **làn main**, cổng dừng các bước sau (L0 test / L1 / QEMU smoke) — khác
KT-418 phần 1 (bước đỏ của họ ở làn `sau` nên cổng chạy hết). Bù lại, các bước đó được đo **tay**:

- **QEMU x86_64 smoke** (thay bước 59): boot ảnh ktest `qemu64` + net + đĩa ⇒ **50 TAP `ok`**,
  `# viFastOs x86_64: boot ok` + `halt` (log `/tmp/kt418b-final-qemu64.log`).
- **L1 smoke** (thay bước L1): xem §7.5.
- **QEMU aarch64**: **chưa** chạy lại sau thay đổi (thay đổi aarch64 chỉ là **một nhánh `match`**
  trong `bar_assign_base`, đã xanh ở `cargo check --workspace --target riscv64gc-unknown-none-elf`
  và ở bước `workspace v2/kernel: clippy host-testable`); lead chạy T2 sẽ phủ.

### 7.5 L1 smoke (đo tay, thay bước L1 của T1 bị dừng)

```text
$ bash v2/tools/l1-tier.sh --tier smoke
 smp     seed  ipc_min   ipc_sent  delivered   dropped     map   walk  unmap  fault  shoot             digest
   1     1000        4      12243       2316      6127    2614    700   1342   2349   1342 3675a4a2ef394150
== wall 5 s (0 m 5 s), 1 cấu hình smp × 22 lô = 22 task ==
== l1-tier: PASS (mọi lô xanh, phủ đủ seed) ==
```

### 7.6 **Sai số thứ hai đo được**: `stac`/`clac` vô điều kiện ⇒ `#UD` trên `qemu64`

Bản đầu của guard gọi `stac` **vô điều kiện**. `stac`/`clac` là lệnh của **SMAP**: trên CPU
**không** có SMAP (model `qemu64` của cổng) chúng `#UD`. Boot `qemu64` chết ngay phép đo `INVLPG`:

```text
# vspace: leaves a_code=0x0000000000143065 ... iso_ok=1
# exc: vec=6 err=0x0 rip=0x110a10 cs=0x8 rflags=0x10097      # #UD tại `stac`
# exc: frame rsp=0x1699d0 ss=0x10 task_rsp=0x169a80
```

Đã sửa: `smap::SMAP_ON` (cờ `AtomicBool`) do `enable_smep_smap` ghi **sau** khi ghi `CR4`; guard
chỉ phát `stac`/`clac` khi cờ bật, ngược lại là **no-op**. Đo lại **cả hai** model:

```text
# qemu64 (không SMAP): 50 TAP ok, boot ok + halt
# smep/smap: va=0x1000000000 cpuid_smep=0 cpuid_smap=0 smep=0 smap=0 smap_deferred=0 cr4=0x20 ... hardened=1

# -cpu max (có SMAP): TAP 50 ok
# vspace: remap va=0x1000004000 ... invlpg_ok=1
# smep/smap: va=0x1000000000 cpuid_smep=1 cpuid_smap=1 smep=1 smap=1 smap_deferred=0 cr4=0x0000000000300020 smap_err=0x1 smap_cr2=0x1000000000 smap_ok=1 smep_err=0x11 smep_cr2=0x1000000000 smep_ok=1 hardened=1
ok 50 - smep/smap: bat va ca am
```

**Bài học**: một lệnh "chỉ có trên CPU mới" phải **gate bằng CPUID/CR4 đọc lại**, không được giả
định model CPU. Đây là lần thứ hai trong KT-418 (lần đầu: `PAN` trên `cortex-a72`, §8.4(1) của
journal phần 1) — cùng một lớp lỗi, và cả hai lần đều do **boot thật** bắt, không phải test host.

## 8. Trạng thái bàn giao

- Nhánh `task/kt418b-rng-smap`, worktree `/private/tmp/vifast-wt/2`, base `main` @ `88d7473b`.
- **Không** commit (lead commit). `git status --short`: xem §9.
- **0 thay đổi `spec/`**; **0** `#[ignore]`/`#[should_panic]`; **0** nới ngưỡng; **0** sửa
  `accept()`/`expected.txt`/`run-*.sh`; **0** sửa baseline ratchet B1.1.
- **Xong**:
  - **Ưu tiên 1** — `virtio-rng`: crate thuần `vifast-virtio::{rng,rngdev}` (device id **`0x1005`**
    đo được, ring một hàng đợi, cổng chất lượng entropy, 10 test host) + API
    `vifast-core::entropy::EntropyPool` (8 test host) + `VirtioKind::Rng` + nhánh `bar_assign_base`
    2 nền tảng. **Adapter x86_64 + probe đã viết và đo được bằng QEMU thật** (`# rng: device=0x1005
    ... entropy_ok=1`, boot ok/halt) nhưng **đã gỡ** theo quyết định (b) của lead vì ratchet B1.1
    (§4.3) — mã lưu ở `/tmp/kt418b-virtio_rng.rs.removed` + `/tmp/kt418b-kbin-rng.rs.removed`.
  - **Ưu tiên 2** — `SMAP` x86_64: `stac`/`clac` + guard `UserAccess` (gate bằng cờ `SMAP_ON`),
    bọc **đủ 3** đường chạm trang `US=1`, bật `CR4.SMAP` khi CPUID báo có; ca âm **thật**
    (`smap_err=0x1 smap_ok=1`), `invlpg_ok=1`, ảnh product-shell chạy **88 syscall** dưới SMAP rồi
    `boot ok`; `qemu64` (không SMAP) vẫn 50 TAP + marker.
  - **Ưu tiên 3** — ca âm `-cpu max`: **KHÔNG** đổi baseline (đúng luật); có **bằng chứng cụ thể**
    rằng đổi baseline hiện **không** khả thi: boot ktest `-cpu max` dừng ở phép kiểm cô lập `VSpace`
    (§6.4) — lỗi **có sẵn**, không phải hồi quy (log trước khi sửa SMAP đã cùng dãy dòng).
- **Chưa xong / nợ**:
  1. **Adapter + probe `virtio-rng`** — chờ Agent 0 duyệt **một** dòng vào `driver_files` của
     `v2/test/baseline/b1.1-ratchet.toml` (đề xuất task riêng, ≈ 20 phút, mã đã có + đã đo xanh).
  2. **TAP `virtio-rng` trên 3 arch** — cần runner gắn `-device virtio-rng-pci` (sửa `run-*.sh` +
     `expected.txt` = **ngoài quyền task**).
  3. **Hàm trộn `EntropyPool`** — không phải CSPRNG đã đánh giá; thay bằng ChaCha20 khi cần entropy
     mật mã (hợp đồng `seed`/`fill` không đổi).
- **Cần lead quyết**:
  (a) duyệt baseline B1.1 để dựng lại adapter `virtio-rng` (ưu tiên cao — phần lớn giá trị của task);
  (b) thêm `-device virtio-rng-pci` vào runner để có TAP thật trên cổng;
  (c) điều tra lỗi `-cpu max` (§6.4) **trước khi** bàn đổi baseline cổng;
  (d) thay hàm trộn `EntropyPool` bằng ChaCha20 khi cần entropy mật mã.

## 9. `git status --short`

```text
 M v2/kernel/vifast-core/src/lib.rs
 M v2/kernel/vifast-kbin-x86_64/src/umode.rs
 M v2/kernel/vifast-plat-aarch64/src/pci.rs
 M v2/kernel/vifast-plat-riscv64/src/pci.rs
 M v2/kernel/vifast-plat-x86_64/src/asm.rs
 M v2/kernel/vifast-plat-x86_64/src/asm/fpu.rs
 M v2/kernel/vifast-plat-x86_64/src/syscall_env.rs
 M v2/lib/vifast-virtio/src/lib.rs
 M v2/lib/vifast-virtio/src/pci.rs
?? v2/agent/JOURNAL/kt418b-rng-smap.md
?? v2/kernel/vifast-core/src/entropy.rs
?? v2/kernel/vifast-core/src/entropy_tests.rs
?? v2/kernel/vifast-plat-x86_64/src/asm/smap.rs
?? v2/lib/vifast-virtio/src/rng.rs
?? v2/lib/vifast-virtio/src/rngdev.rs
?? v2/lib/vifast-virtio/tests/rng.rs
```

`git diff --stat`: **9 tệp sửa** (+91/−20) + **7 tệp mới** (gồm journal).
