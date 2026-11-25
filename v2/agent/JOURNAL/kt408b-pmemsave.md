# KT-408b — Nợ của KT-408: `pmemsave` đầu-cuối + nối 3 đột biến `mut-ring-*`

- Nhánh: `task/kt408b-pmemsave` · worktree `/private/tmp/vifast-wt/1` · base `main` @ `13c396ab`
- Ngân sách 90 phút (bắt đầu `2026-11-26 22:13 UTC`), khung Thấp điểm chính (`D2-256`).
- Nợ nguồn: `v2/agent/JOURNAL/kt408-kernelstore-ring.md` §4 mục 3 (pmemsave) + mục 4 (3 đột biến
  chưa nối `mutants.toml`, `declarations` 49).

## 0. Kế hoạch (cập nhật sau khi đo hiện trạng)

1. **Đo hiện trạng**: `v2 mutants --lint` (phải xanh trước khi thêm) — đã xanh:
   `mutants=30 feature=29 declarations=49 covered=49 leftover=0 suites=10 problems=0`.
2. **pmemsave đầu-cuối** (rủi ro thấp hơn theo lệnh lead, nhưng hoá ra là chỗ **lộ lỗi thật**):
   - Dựng ảnh `vifast-kbin-ktest` (`--features suite-base`) cho `riscv64gc-unknown-none-elf`;
     đọc ký hiệu `vifast_sync::kernel_ring::{RING,HEADS,COUNTS}` bằng `llvm-nm` của rustc;
   - boot QEMU (`-machine virt,aia=none,aclin=on -bios OpenSBI -kernel ảnh`) **có monitor HMP**
     trên unix socket, chờ mốc serial rồi gửi `pmemsave <pa> <n> <tệp>` (kernel riscv64
     identity-map `0x8020_0000` ⇒ VA = PA);
   - kiểm tệp dump: `RING`/`HEADS`/`COUNTS` **nằm trong `.corespace`**, bản ghi 32 B giải mã
     được bằng `v2/tools/vifast-trace.py`, `COUNTS[0] = min(HEADS[0], PER_HART)`;
   - lưu **log + tệp mẫu** thành bằng chứng thường trực (không phải `/tmp`).
3. **3 đột biến `mut-ring-*`**: thêm 3 mục `kind = "build"` + `patches` vào
   `v2/test/mutants.toml` (suite host `core-ring-host` = `cargo test -p vifast-core --lib kt408`),
   `declarations` 49 → 52; chứng minh từng cái **KILLED** bằng `v2 mutants <tên>`.
4. `./v2/bin/v2 check` + T1 qua khoá (`/tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev`).

## 1. Hiện trạng đo được (trước khi sửa)

### 1.1 `v2 mutants --lint` — XANH (điều kiện tiên quyết)

```
# mutants lint: mutants=30 feature=29 declarations=49 covered=49 leftover=0 suites=10 problems=0
```

3 móc `mut-ring-*` **chưa có** trong `v2/test/mutants.toml`; 3 đột biến của KT-408 chỉ là bằng
chứng tay (journal §3.2). Đột biến `mut-smp-*`/`mut-tier2-*` là `kind = "feature"` (lật cfg trong
`v2/kernel/**/*.rs`), nhưng `apply_feature` **chỉ quét `v2/kernel`** ⇒ kho ring ở `v2/lib` không
lật được bằng cfg; cả 3 đột biến của KT-408 là **bản vá mã nguồn** ⇒ đúng khuôn `kind = "build"`
+ `patches` (như `mut-info-x1`, `mut-net-one-caller`).

### 1.2 **Lỗi thật lộ ra khi đo ký hiệu ảnh** — `RING` bị LTO xoá khỏi ảnh

Ảnh `vifast-kbin-ktest` (`--features suite-base`, release `lto = true`, `codegen-units = 1`,
`opt-level = "s"`) — `llvm-nm`:

```
0000000080289188 b _RNvNtCsbRlnHb5AnnX_11vifast_sync11kernel_ring5HEADS
00000000802891c8 b _RNvNtCsbRlnHb5AnnX_11vifast_sync11kernel_ring6COUNTS
```

**Không có `RING`.** `.corespace` = `0x2d208` = `size_of::<Machine>()` (`0x2d188`, 184.712 B)
+ `0x80` (HEADS 64 B + COUNTS 64 B) ⇒ 8 KiB kho bản ghi **không** có trong ảnh.

Nguyên nhân: trong ảnh bare-metal **không ai đọc** `RING` (`len_hart`/`read_record`/`dump_hart`
chỉ có trong `kt408_tests` — `#[cfg(test)]`), `RING` là `static` **nội bộ không thoát** ⇒ LTO +
DSE xoá sạch store của ring; `HEADS`/`COUNTS` sống sót vì mang `link_section = ".corespace"`
(nên linker giữ). Hệ quả: `pmemsave` **không có gì để đọc** — đúng lý do nợ này phải chạy
đầu-cuối; "byte-level đã chứng minh ở test host" không chứng minh được ring **có trong RAM ảnh**.

**Vá tối thiểu (không nới ngưỡng, 0 đổi `.data+.bss`, 0 đổi `size_of::<Machine>()`)**:
`#[used]` cho cả ba static ⇒ `llvm.used` giữ ký hiệu + địa chỉ thoát ⇒ DSE không xoá được store;
`.corespace` (NOLOAD) tăng 8 KiB nhưng cổng ngân sách chỉ đòi `.corespace ≥ 184.792 B` ⇒ vẫn xanh.

## 2. Vá `#[used]` — đo trước/sau (cùng ảnh, cùng lệnh `llvm-nm`)

```
# TRƯỚC (base 13c396ab): chỉ HEADS/COUNTS, .corespace = 0x2d208 = Machine + 0x80
0000000080289188 b …kernel_ring5HEADS
00000000802891c8 b …kernel_ring6COUNTS
  5 .corespace        0002d208 …
# SAU (#[used]): có RING 8 KiB, .corespace = 0x2f208 = Machine + 0x2000 + 0x80
0000000080289188 B …kernel_ring4RING
000000008028b188 B …kernel_ring5HEADS
000000008028b1c8 B …kernel_ring6COUNTS
  5 .corespace        0002f208 …
```

`#[used]` **không** đổi `size_of::<Machine>()` (184.712) lẫn `.data+.bss` (section NOLOAD) ⇒ không
nới ngưỡng nào; `.corespace` 184.712 → 193.032 B, cổng `budget*` chỉ đòi `≥ 184.792 B`.

## 3. `pmemsave` đầu-cuối — PASS

Runner mới (thường trực, không phải log tạm):

| Tệp | Vai trò |
|---|---|
| [`v2/test/boot/pmemsave-ring.sh`](../../v2/test/boot/pmemsave-ring.sh) | runner: build ảnh → `llvm-nm` → QEMU qua `qemu_guard.sh` → kiểm tệp dump → lưu mẫu |
| [`v2/test/boot/pmemsave_ring.py`](../../v2/test/boot/pmemsave_ring.py) | chờ mốc serial, `stop` VM, **một** `pmemsave` vùng liền nhau, `cont` |
| [`v2/test/trace/pmemsave-riscv64.log`](../../v2/test/trace/pmemsave-riscv64.log) | log bằng chứng (4,0 KiB) |
| [`v2/test/trace/pmemsave-riscv64-ring.bin`](../../v2/test/trace/pmemsave-riscv64-ring.bin) | **tệp mẫu** QEMU ghi ra: 8.192 B `RING` |
| [`v2/test/trace/pmemsave-riscv64-heads-counts.bin`](../../v2/test/trace/pmemsave-riscv64-heads-counts.bin) | 128 B = 8×u64 `HEADS` + 8×u64 `COUNTS` |

Lệnh: `bash v2/test/boot/pmemsave-ring.sh` (worktree, `CARGO_TARGET_DIR=…/slot-1`) → exit 0.
QEMU 11.1.3 + OpenSBI `fw_dynamic`; ảnh `vifast-kbin-ktest` (`--features suite-base`, release
`lto = true`), `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -icount shift=6`,
monitor HMP `unix:<sock>,server=on,wait=off`. Kernel riscv64 **identity-map** `0x8020_0000`
(`plat-riscv64/src/mmu.rs`) ⇒ `PA == VA`.

Số đo trong log (trích nguyên văn):

```
# sym: RING=0x0000000080289188 HEADS=0x000000008028b188 COUNTS=0x000000008028b1c8
# sym: .corespace=[0x000000008025c000, 0x000000008028b208) = 193032 B
# pmemsave: tag=a … heads=[14 0 0 0 0 0 0 0] counts=[14 0 0 0 0 0 0 0]
# pmemsave: tag=b … heads=[20 0 0 0 0 0 0 0] counts=[20 0 0 0 0 0 0 0]
# liveness: head 14 → 20 (+6 bản ghi giữa hai mốc)
# records: n=20 hart=0 kinds=[2, 3] t_ns=[0..0]
# == pmemsave-ring: PASS (RING/HEADS/COUNTS trong .corespace; 2 ảnh chụp head 0→20 bản ghi;
#    20 bản ghi 32 B giải mã bằng vifast-trace.py) ==
t=0 hart=0 kind=sched tcb=3 a=0 b=0
t=0 hart=0 kind=ipc tcb=4 a=0 b=0
…
```

Kiểm được: (i) ba ký hiệu **nằm trong** `[__corespace_start, __corespace_end)`; (ii) `HEADS`/`COUNTS`
**liền nhau** (0x8028b188 + 0x40 = 0x8028b1c8) nên **một** `pmemsave` 8.320 B là ảnh chụp nguyên tử;
(iii) `count = min(head, 32)` ở **cả hai** mốc, `head` **tăng** 14 → 20 (ring sống, không phải ảnh
tĩnh); (iv) 20 bản ghi 32 B giải mã **bằng công cụ chính thức** `v2/tools/vifast-trace.py`, `kind`
∈ {2 ipc, 3 sched}, `hart = 0`, `t_nb` không lùi, `tcb` là TCB id thật (3, 4, 13, 14, 15, …).

**Bài học đo được** (đáng ghi vào nợ): `pmemsave` **không** nguyên tử nếu đọc ba vùng bằng ba lệnh —
lần chạy đầu đo được `heads=9, counts=14` (guest chạy giữa hai lệnh). Runner vì vậy `stop` VM và
dump **một** blob liền nhau. Ngoài ra **trước** vá, tệp dump toàn 0 và `HEADS`/`COUNTS` đứng yên:
`RING` bị LTO xoá; đây là lý do nợ này **không thể** đóng bằng test host.

## 4. Ba đột biến `mut-ring-*` — nối `mutants.toml`, cả ba KILLED

`v2/test/mutants.toml`: `declarations` **49 → 52**, thêm suite host `core-ring-host`
(`cargo test -p vifast-core --lib kt408`) + 3 mục `kind = "build"` + `patches`:

| id | Vá | Ca phải đỏ | Kết quả | Thời gian |
|---|---|---|---|---:|
| `mut-ring-store-empty` | `Table::deref`: `&self.0` → `&self.0[..0]` | `kt408_store_phu_100_phan_tram_bang` | **KILLED** | 6,6 s |
| `mut-ring-kind-offset` | `push`: `kind << 16` → `<< 24` | `kt408_ring_trace_32_b` | **KILLED** | 6,8 s |
| `mut-ring-set-harts-n` | `set_harts`: bỏ `self.n_harts = harts as usize` | `kt408_set_harts` | **KILLED** | 10,4 s |

Vì sao `kind = "build"` + `patches` (không phải `kind = "feature"`): `apply_feature` **chỉ quét
`v2/kernel/**/*.rs`**, còn kho ring ở `v2/lib/vifast-sync`; cả ba đột biến của KT-408 vốn là **bản vá
mã nguồn** (journal §3.2) nên khuôn `build`+`patches` là đúng (như `mut-info-x1`, `mut-net-one-caller`).
`marker` **riêng từng mục** = tên ca ⇒ đỏ do hạ tầng/compile không bị tính là KILLED.

Lý do đỏ (trích log `--keep` của công cụ):

```
ok - mut-ring-store-empty: KILLED (6.6 s) patches=1 rc=101 marker=ok
  panicked at vifast-core/src/kt408_tests.rs:41:5: assertion `left == right` failed: regions / left: 0
ok - mut-ring-kind-offset: KILLED (6.8 s) patches=1 rc=101 marker=ok
  panicked at vifast-core/src/kt408_tests.rs:174:5: … kind@10 = run (6) / left: 1536
ok - mut-ring-set-harts-n: KILLED (10.4 s) patches=1 rc=101 marker=ok
  panicked at vifast-core/src/kt408_tests.rs:129:5: … set_harts(4) phải đổi n_harts / left: 1
```

`v2 mutants --lint` trước/sau:

```
# TRƯỚC: mutants=30 feature=29 declarations=49 covered=49 leftover=0 suites=10 problems=0
# SAU:   mutants=33 feature=32 declarations=52 covered=52 leftover=0 suites=11 problems=0
```

## 5. T0 / T1 / kiểm

### 5.1 T0 — host test sau khi vá (chạy trên worktree)

```
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core --lib kt408
running 4 tests
test kt408_tests::kt408_store_phu_100_phan_tram_bang ... ok
test kt408_tests::kt408_machine_giu_co_copy_va_meta_48_b ... ok
test kt408_tests::kt408_ring_trace_32_b ... ok
test kt408_tests::kt408_set_harts ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 122 filtered out; finished in 0.00s
                                            (4,7 s kể cả build)

$ cargo test --manifest-path v2/Cargo.toml -p vifast-sync
5 test binary: 4+3+10+5+3 = 25 passed; 0 failed; 0 ignored
```

`v2 mutants --lint` sau khi thêm 3 mục: `mutants=33 … declarations=52 covered=52 leftover=0
suites=11 problems=0` (xem §4).

### 5.2 `./v2/bin/v2 check` — PASS

```
$ ./v2/bin/v2 check          # rc=0
# step-time-total   178751 ms  TỔNG check            (179 s)
# check-wake-depinfo: errors=0 warnings=0 (87.04 s)
# check-test-teeth: errors=0 warnings=0
```

### 5.3 T1 — `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` (qua khoá máy)

```
# gate-steps registered=80 ran=66 skipped=14 failed=1
# step-time-total   598784 ms  TỔNG gate --dev (T1)   (9 min 59 s)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**65/66 bước đã chạy là XANH; 1 bước đỏ duy nhất `selftest pre-measure (KT-95)` — đỏ theo thiết kế
trên CÂY BẨN** (worktree chưa commit, đúng luật "lead commit"):

```
--- selftest pre-measure (KT-95): FAIL
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)    # rc 7 = "tree-dirty"
$ grep -n "refuse 7" v2/tools/pre-measure.sh
209:    [ "$DIRTY" = "0" ] || refuse 7 tree-dirty
```

Chứng minh **không phải** do thay đổi của KT-408b — cùng bước đó trên **bản sao sạch**:

```
$ git clone --depth 1 file:///private/tmp/vifast-wt/1 /tmp/kt408b-clean   # HEAD 96120f2, sạch
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest: PASS (a/b/c/d/e/f/g)                # KHÔNG có thay đổi KT-408b
```

rồi **áp đúng thay đổi của KT-408b** vào bản sao và commit trong bản sao (`c42f7f9`, không tạo
commit nào trên nhánh task):

```
$ git -C /tmp/kt408b-clean log --oneline -1
c42f7f9 [KT-408b] ring .corespace #[used] + pmemsave đầu-cuối + 3 mut-ring-*
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest: PASS (a/b/c/d/e/f/g)
$ python3.12 v2/tools/mutants.py --root . --lint
# mutants lint: mutants=33 feature=32 declarations=52 covered=52 leftover=0 suites=11 problems=0
```

⇒ Đỏ của T1 là **điều kiện "cây bẩn"** có sẵn của harness (cùng hiện tượng đã ghi ở `KT-402b`
trong `QUEUE.md`: "đỏ duy nhất `pre-measure` trên cây bẩn — PASS trên cây sạch"), **không** phải
lỗi của nhánh. Sau khi Lead commit, bước này xanh (đã chứng minh bằng bản sao sạch có đúng thay đổi).

## 6. Việc còn lại / bàn giao Lead

1. **Commit** giúp nhánh `task/kt408b-pmemsave` (agent không commit): 3 tệp sửa (`kernel_ring.rs`,
   `mutants.toml`) + tệp mới (`pmemsave-ring.sh`, `pmemsave_ring.py`, journal, 2 mẫu `.bin` +
   2 `.license`, 1 `.log`).
2. **T2 `gate --fast` trên ứng viên tích hợp**: bắt buộc vì KT-408b chạm `lib/vifast-sync` +
   `v2/test/mutants.toml`; T2 cũng chạy 14 bước T1 bỏ qua (selftest runner ktest, QEMU
   net/input/gpu, persist/crash, selfbreak, git-guard…). `pre-measure` sẽ xanh sau commit.
3. **Quyết định (không tự làm)**: có nên nối `v2/test/boot/pmemsave-ring.sh` thành **bước T2**
   thường trực không. Hiện nó là runner độc lập (chạy 1 lần/≈6 s + build ảnh); nối vào gate đổi
   `gate-steps registered` và thời gian T2 ⇒ vượt phạm vi KT-408b. Nếu nối, nên là bước T2-only
   cạnh `QEMU aarch64`.
4. **Nợ mới ghi nhận (không sửa trong task)**: `Machine::now` **luôn 0** trên ảnh riscv64 ⇒
   `t_ns` của mọi bản ghi trace = 0 (bản ghi vẫn hợp lệ theo `spec/abi.toml [trace]`, nhưng mất
   trục thời gian). Cần task riêng cấp tick cho `Machine` (`D2-49(h)` nói tick ảo); test host
   `kt408_ring_trace_32_b` đã chấp nhận `t_ns <= 1` nên **không** đỏ.
5. **Quan sát thêm**: ring chỉ được ghi từ pha core (`ok 43`..`ok 59`, `head` 0 → 20); trước đó
   `Machine` chưa chạy nên ring rỗng — đúng thiết kế, nhưng nếu sau này muốn trace cả giai đoạn
   boot thì cần hook ở `plat-*::init` (ngoài phạm vi).

## 7. Bổ sung — ảnh **sản phẩm** riscv64: 0 tăng `.data+.bss`, `.corespace` đúng +8.320 B

Đo sau vá bằng `v2/test/boot/budget.sh` trên ảnh `vifast-kbin-riscv64` (release, `lto = true`):

```
# image=product … file=229336 arch=riscv64(e_machine=243) data+bss=11624 budget=262144
#   label_budget=65536 … corespace=193032 … noload=1143304
# sections-seen: .text=61290 .rodata=17376 .data=8136 .bss=3488 .corespace=193032 …
$ llvm-nm --defined-only vifast-kbin-riscv64 | grep kernel_ring
0000000080244188 B …kernel_ring4RING
0000000080246188 B …kernel_ring5HEADS
00000000802461c8 B …kernel_ring6COUNTS
```

`corespace = 193.032 = 184.712 (Machine) + 8.192 (RING) + 128 (HEADS+COUNTS)` — đúng số học, và
`.data+.bss = 11.624 B` **không đổi** (ring ở section NOLOAD) ⇒ **0 nới ngưỡng** (trần 262.144 B,
`label_budget` 65.536 B giữ nguyên).

Ghi chú thêm: hàng đợi T1 ở lượt này do **slot khác** (`/private/tmp/vifast-wt/2`) giữ khoá liên
tục >14 phút; một lượt T1 **đầy đủ trên bản sao sạch** đã được xếp hàng nhưng **không chạy** trong
ngân sách (đã huỷ trước khi lấy khoá, không đụng phép đo của slot khác) — thay bằng chứng minh
từng bước ở §5.3 (`pre-measure` PASS trên bản sao sạch **có** thay đổi KT-408b).
