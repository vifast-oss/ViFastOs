# KT-340a — `x86_64`: `budget-x86_64.sh`, tách ảnh sản phẩm vs `ktest`, W^X kernel

**Task**: KT-340a (Chu kỳ 2, Đợt 2), sub-agent `vspace`. Worktree `/tmp/kt340a-s1`,
nhánh `task/kt340a-vspace`, base `main` @ `99ee94df`.
**Ngày**: 2026-11-22 (JST). **Ngân sách**: 3 h.
**Nguồn luật**: `AGENTS.md` §4/§6, D2-08, D2-17, D2-80, D2-236, D2-238, `v2/docs/k6-test-invariants.md`.

> Mọi số dưới đây là **đo được** trên máy này (QEMU 11.1.3, rustc 1.100.0, macOS aarch64 host),
> không phải số viết tay. Log thô nằm trong `/tmp/kt340a-*` của phiên chạy; phần trích dẫn trong
> tệp này là **nguyên văn** dòng log.

---

## 1. Khảo sát (bước 1) — hiện trạng `x86_64` trước KT-340a

### 1.1 Bản đồ bộ nhớ: **identity tĩnh, 2 MiB/lá, 3 cấp cho kernel**

`vifast-kbin-x86_64/src/main.rs` (`global_asm!`) dựng bảng trang **tĩnh trong `.bss`** ngay ở
`_start` (32-bit, trước long mode):

| Cấp | Ô | Trỏ tới | Cờ |
|---|---|---|---|
| `__pml4[0]` | 0 | `__pdpt` | `P|RW|US` (US ở gốc để cây con user truy cập được) |
| `__pml4[224]` | 224 | 0 | **không map** (`0x7000_0000_0000` ⇒ `#PF`, dùng cho TAP 5) |
| `__pdpt[0]` | 0 | `__pd` (512 × 2 MiB) | `P|RW` |
| `__pdpt[2]` | 2 | `__pd_mmio_low` | `P|RW`, 2..3 GiB (**ECAM q35** `0xB000_0000`, PCD) |
| `__pdpt[3]` | 3 | `__pd_mmio` | `P|RW`, 3..4 GiB (LAPIC/IOAPIC, PCD) |
| `__pd[i]` | 0..511 | `i*2 MiB | 0x83` | `P|RW|PS` — **identity 0..1 GiB, RWX toàn bộ** |
| `__pdpt[256]` | 256 | `__pd_user` → `__pt_user` | cây con **user** riêng cho ring 3 |

Kết luận khảo sát:
- **Identity map tĩnh** (không `VSpace` động cho tác vụ): mọi thứ chạy trên một bảng trang duy
  nhất; ring 3 chỉ có **một** cây con user tĩnh (`__pd_user`/`__pt_user`, một trang mã + một
  stack 4 KiB).
- Kernel dùng **3 cấp** PML4→PDPT→PD với **lá lớn 2 MiB**; cây user mới đi tới cấp 4 (PT) — vì
  vậy bảng boot **không thể** tách quyền `.text`/`.rodata`/`.data` (cả ảnh nằm trong **một** lá
  `PD[0]`).
- Địa chỉ: image `0x100000..~0x1db000` (text/rodata/data/user/bss/smp_stacks), còn lại của lá
  `PD[0]` là RAM thấp (BIOS/IVT + trampoline AP `0x8000` + stack 16-bit `0xFC00`).

### 1.2 W^X: **chưa có**

- Mọi lá `PD[i]` = `0x83` ⇒ **RWX**; `CR0.WP` **không** bật (log boot trước KT-340a:
  `cr0=0x0000000080000011` — bit 16 = 0). Ring 0 ghi được cả `.text`/`.rodata`.
- Không có TAP nào kiểm quyền trang kernel; `vifast-plat-riscv64` chỉ có W^X cho **trang user**
  (`user.rs`/`init_pages.rs`: `.text` `U|R|X`, `.rodata` `U|R|NX`, `.data` `U|R|W|NX`, guard
  không map) — bản thân image kernel riscv64 vẫn là **một leaf RWX** (`mmu.rs` ghi rõ "W^X đầy đủ
  là KT-56"). Vì vậy KT-340a dùng **khuôn đo của riscv64** (đọc lại cờ lá từ bảng trang + ca
  ghi thật phải fault), nhưng **cơ chế** phải là của x86_64: tách `PD[0]` thành PT 4 KiB +
  `CR0.WP` (không bịa cơ chế mới; đúng SDM Vol 3A §4.5).

### 1.3 Ảnh sản phẩm vs ktest: **một** binary, 34 TAP **nằm trong ảnh sản phẩm**

Trước KT-340a, `vifast-kbin-x86_64` là **một** ảnh duy nhất: `kmain` in `TAP version 13` +
`1..34`, chạy 34 ca, rồi marker + halt. **Không** có `vifast-kbin-ktest-x86_64`, **không** có
feature `ktest`.

`strings | grep -c ktest` trên ảnh cũ = **0** — nhưng đó **không** phải bằng chứng D2-08: chuỗi
`ktest` chưa bao giờ xuất hiện trong ảnh này; harness thật là **TAP**. Quét đúng marker harness
trên ảnh cũ:

```text
'TAP version 13' = 1      'not ok' = 1      '# TAP ' = 1      'ok 1 - ' = 34
'# smp:' = 16   '# net:' = 6   '# umode:' = 6   '# exc:' = 2   ...
```

⇒ Ảnh sản phẩm cũ **chứa toàn bộ harness** (đúng lỗi D2-08 mà D2-233/D2-238 nêu).

### 1.4 Ngân sách `.data+.bss`: **177.440 B** / 262.144 B

```text
section                size         addr
.text               135168    1048576
.rodata              10348    1183744
.note.Xen               20    1194092
.data                 4408    1196032
.user                   95    1204224
.bss                173032    1208320
.smp_stacks         557056    1384448     (NOLOAD — per-hart, báo riêng)
```

`.data 4.408 + .bss 173.032 = 177.440` ⇒ còn 84.704 B dư, nhưng `KT-340c` sẽ thêm
`vifast-core::Machine` (~184 KiB) ⇒ **bắt buộc** tách 64 KiB `TaskStack` test khỏi ảnh sản phẩm
(đúng D2-236).

---

## 2. `v2/test/boot/budget-x86_64.sh` (bước 2) — XONG

Khuôn `budget-aarch64.sh` (KT-265) + khác biệt cho x86_64:

1. **Cổng kiến trúc trước khi đo**: đọc `e_machine` tại offset `0x12`; phải = **62**; ảnh
   riscv64 (243)/aarch64 (183)/không-ELF ⇒ **TỪ CHỐI ĐO** (fail-closed).
2. Báo **riêng** `.smp_stacks` (NOLOAD per-hart, KT-276) và `.user` (khối mã ring 3, không phải
   writable data) — **không** cộng vào trần.
3. Cộng theo **tiền tố** `^.(data|sdata|bss|sbss)` (bắt `.data.rel.ro`, `.sbss.*`).
4. `--selftest` **10 ca**: fixture đúng hình dạng ảnh x86_64 (`.note.Xen`, `.user`,
   `.smp_stacks`), tên biến thể, ca âm **300 KiB `.bss` ⇒ đỏ**, ca âm `.smp_stacks` 4 MiB không
   kéo tổng, `e_machine` đọc từ **ELF thật** (dựng 20 byte bằng `printf`), ca âm 183/243/rỗng,
   ca âm tệp không phải ELF.

Kết quả thật:

```text
$ bash v2/test/boot/budget-x86_64.sh --selftest
ok 1 - cộng .data+.bss của ảnh x86_64 = 177440 (dưới trần 262144)
ok 2 - .smp_stacks báo riêng = 557056 (không cộng vào trần)
ok 3 - .user báo riêng = 95 (khối mã, không phải writable data)
ok 4 - cộng tên biến thể (.data.rel.ro/.sdata/.sbss) = 128
ok 5 - .bss 300 KiB ⇒ 307200 > 262144 (cổng sẽ FAIL)
ok 6 - .smp_stacks 4 MiB không kéo tổng: data+bss=1032 ≤ 262144
ok 7 - e_machine đọc từ ELF thật = 62 (x86-64) ⇒ nhận
ok 8 - machine_ok(62) ⇒ nhận
ok 9 - e_machine 183/243/rỗng ⇒ TỪ CHỐI (không đo nhầm ảnh arch khác)
ok 10 - tệp không phải ELF ⇒ TỪ CHỐI
# budget-x86_64 --selftest: PASS
```

Đo ảnh thật (lúc đó còn một ảnh):

```text
# image=x86_64-product path=.../vifast-kbin-x86_64 file=189120 arch=x86_64(e_machine=62)
  data+bss=177440 budget=262144 stack=0 guard=0 smp_stacks=557056 user=95 noload=557056
# data+bss-sections: .data=4408 .bss=173032
```

Đã nối vào cổng: `v2/bin/v2` có `step_x64_budget` = `budget-x86_64.sh --selftest`, đăng ký
`lane_step D 76`, `GATE_FAST_EXTRA_EXPECTED` 28 → **29** (fail-closed nếu quên). `run-x86_64.sh`
và `soak-x86_64.sh` đo **cả hai** ảnh; `gpu-x86_64.sh` chuyển từ `budget.sh` chung sang
`budget-x86_64.sh` (có cổng `e_machine`).

---

## 3. Tách ảnh sản phẩm vs `ktest-x86_64` (bước 3) — XONG

### 3.1 Thiết kế (khuôn `vifast-kbin-riscv64`, thích ứng R1)

- Thêm feature **`ktest`** cho `vifast-kbin-x86_64` (**mặc định tắt** = ảnh sản phẩm).
- `kmain` rẽ vào **`product::boot`** (module mới `src/product.rs`) **trước** dòng TAP đầu tiên.
  `product::boot` trả `!` ⇒ toàn bộ harness phía sau là **unreachable**; thân kiểm còn được bọc
  `#[cfg(feature = "ktest")] { … }` nên bị **gỡ trước name-resolution** — mô-đun kiểm (`smp`,
  `net`, `sched`, `umode`, `blk`, `lock_report`, `wx_probe`) cũng `#[cfg(feature = "ktest")]`
  ⇒ **không còn gốc tham chiếu** trong ảnh sản phẩm.
- Hai handler `#[no_mangle]` của harness (`exc_dispatch` đầy đủ, `syscall_dispatch`) có **bản
  sản phẩm tối giản** trong `product.rs` (chẩn đoán `# exc:` rồi halt) để stub asm của crate
  plat vẫn liên kết được — ảnh sản phẩm **không** nuôi đường kiểm.
- `__pt_kernel`/`__trampoline` (xem §4): ảnh sản phẩm có **trang trampoline rỗng** (`hlt` + NOP)
  để `ASSERT` bố cục của `linker.ld` vẫn đúng — không phải mã SMP.

**Vì sao không thêm `[[bin]]` thứ hai**: feature của Cargo là **package-wide**; hai bin trong
cùng package sẽ cùng nhận `ktest`, nên "bin sản phẩm" build kèm feature sẽ in TAP. Vì vậy dùng
**một bin + hai lần build khác feature**, runner copy artefact ktest ra tên riêng
`vifast-kbin-ktest-x86_64` (ghi rõ trong `Cargo.toml` + đầu `run-x86_64.sh`).

### 3.2 Kiểm **bằng máy** (D2-08) — bộ quét + ca âm

`run-x86_64.sh` (mục KT-340a, ca P1..P4) và `soak-x86_64.sh`:

- **P1** quét 14 marker harness bằng `strings | grep -Fc`
  (`TAP version 13`, `not ok`, `# TAP `, `ok 1 - `, `ktest`, `# smp:`, `# net:`, `# umode:`,
  `# sched:`, `# blk:`, `# lock:`, `# apic:`, `# timer:`, `# wx:`) — ảnh sản phẩm **phải 0**.
  **Ca âm**: cùng bộ quét trên ảnh ktest **phải > 0** (P1 in `ktest=77 ⇒ bộ quét có răng`).
- **P2** boot ảnh sản phẩm thật: marker **đúng 1 lần**, dòng halt, `# conf image:` **khớp hash
  ELF** (D2-80), **0** dòng TAP.
- **P3** đưa log sản phẩm cho `check_log` (bộ kiểm TAP) ⇒ **phải ĐỎ**.
- **P4** đưa log ktest cho `check_product_log` (bộ kiểm sản phẩm) ⇒ **phải ĐỎ**.

Kết quả đo:

| chỉ số | trước KT-340a | sau KT-340a (sản phẩm) | sau (ktest) |
|---|---:|---:|---:|
| `strings \| grep -c ktest` | 0 | **0** | 0 |
| marker harness (14 mẫu) | **74** (một ảnh) | **0** | 77 |
| TAP version / `ok N` | 1 / 34 | **0** | 1 / **36** |
| `.data+.bss` | 177.440 B | **96.320 B** | 181.592 B |
| kích thước ảnh | 189.120 B | **93.272 B** | 189.600 B |
| TAP | 34 (trong ảnh sản phẩm) | 0 | 34 + 2 (W^X) |

Ảnh sản phẩm **nhỏ hơn 95.848 B** so với trước (rơi 64 KiB `TaskStack` test + `.bss` harness).
Trần **giữ nguyên 262.144 B**, không nới.

---

## 4. W^X kernel x86_64 (bước 4) — XONG

### 4.1 Cơ chế (theo SDM, khuôn đo của riscv64)

Module mới `vifast-plat-x86_64/src/wx.rs` (+ primitive trong `asm.rs`):

1. `_start` giữ nguyên bảng boot; `kmain` gọi `product::wx_init()` **một lần** (ảnh sản phẩm
   **và** ảnh kiểm), sau `_start` và trước mọi pha khác.
2. Dựng `__pt_kernel` (4 KiB, trong `.bss`) = **512 lá 4 KiB** cho `0..2 MiB`, quyền theo
   **đoạn linker** (symbol `__text_start/__text_end/__data_start/__kernel_end`, không hardcode):

   | đoạn | quyền |
   |---|---|
   | `[text_start, text_end)` `.text` | `P` + **X** (không W) |
   | `[text_end, data_start)` `.rodata` + note | `P|NX` |
   | `[data_start, kernel_end)` `.data/.user/.bss/.smp_stacks` | `P|RW|NX` |
   | `0x8000` (chỉ ảnh ktest) | `P|RW|X` — **ngoại lệ duy nhất**: BSP ghi boot block + AP chạy trampoline trong cùng trang |
   | còn lại của 2 MiB (RAM thấp/BIOS) | `P|RW|NX` |
3. `PD[0] = __pt_kernel | P|RW` (thay lá lớn 2 MiB; `US=0` — identity map vẫn supervisor).
4. Xả TLB: `mov cr3, cr3` (`asm::write_cr3`).
5. Bật **`CR0.WP`** (`asm::write_cr0(read_cr0() | 1<<16)`) — thiếu nó thì ring 0 vẫn ghi được
   trang RO và "W^X" chỉ là nhãn.
6. `WxReport` **đọc lại** từ bảng trang (`walk_leaf`) — TAP dùng đúng giá trị này.

### 4.2 TAP 35/36 (ảnh ktest) — log QEMU **nguyên văn**

```text
# wx: cr0_wp=1 text_end=0x121000 rodata_end=0x124000 data_end=0x1db000 text=0x0000000000100021 rodata=0x8000000000121021 data=0x8000000000124063 low=0x0000000000008063 wx_pages=1 pt=0x12b000
ok 35 - wx: kernel sections
# exc: vec=14 err=0x3 rip=0x100460 cs=0x8 rflags=0x10006
# exc: cr2=0x100040
# wx: text write addr=0x100040 err=0x3 cr2=0x100040 unchanged=1
# wx: data write addr=0x124100 written=1 restored=1
ok 36 - wx: text write faults
```

Đọc **từ bảng trang** (không phải hằng): `text=…0021` = `P|X`; `rodata=…8021` = `P|NX`;
`data=…8063` = `P|RW|NX`; `wx_pages=1` (đúng trang trampoline ktest). Ghi vào `.text` từ CPL0 ⇒
`#PF` `err=0x3` (`P=1, W=1`), `cr2` đúng địa chỉ, byte **không đổi**; cùng stub ghi `.data`
thành công rồi khôi phục. Ảnh sản phẩm **bật** W^X nhưng không đo (đo là harness — D2-08).

### 4.3 Đột biến (chứng minh TAP có răng)

Feature `no-wx`: `protect=false` ⇒ `.text` RWX + `CR0.WP=0`. Runner ca **(b2)**:
build `--features ktest,no-wx` → boot → **phải ĐỎ** và log **phải có** `# wx: cr0_wp=0`
(chứng minh đột biến thật vào ảnh). Feature `wx-data-exec`: bỏ `NX` trên `.data/.bss` ⇒
`wx_pages` tăng ⇒ TAP 35 ĐỎ (đột biến thứ hai, cùng cơ chế).

---

## 5. `VSpace` động cho tác vụ ring 3 (bước 5) — **CHƯA LÀM**

Theo đúng thứ tự ưu tiên của task, bước này nằm sau W^X và **chưa triển khai** trong ngân sách
này. Hiện trạng để `KT-340b`/`KT-340c` tiếp tục:

- Kernel vẫn **một** bảng trang identity tĩnh; ring 3 dùng **một** cây con user tĩnh
  (`__pd_user`/`__pt_user`, một trang mã + một stack 4 KiB) — chưa có `VSpace` cấp phát động,
  chưa map theo tác vụ.
- W^X của KT-340a là **kernel image** W^X (đo được); W^X **trang user** khi nạp ELF là việc của
  `KT-340c` (khi nối `vifast-core` + IPC ring 3), theo khuôn `user.rs` của riscv64.
- Ngoại lệ có chủ đích đã ghi trong mã: trang `0x8000` (trampoline AP) là `RWX` **chỉ** ở ảnh
  ktest; `wx_pages` vì vậy = 1 ở ảnh ktest và = 0 ở ảnh sản phẩm.

---

## 6. Đột biến / kiểm-red-trước (D2-05) và các ca âm

| Ca | Cơ chế | Kỳ vọng | Trạng thái |
|---|---|---|---|
| budget `--selftest` ca 5 | fixture `.bss` 300 KiB | cổng **ĐỎ** | XONG (10/10) |
| D2-08 P1 | quét marker ảnh sản phẩm | 0 marker; ktest > 0 | XONG |
| D2-08 P3 | log sản phẩm vs bộ kiểm TAP | **ĐỎ** | XONG |
| D2-08 P4 | log ktest vs bộ kiểm sản phẩm | **ĐỎ** | XONG |
| W^X (b2) | `--features ktest,no-wx` | TAP 35/36 **ĐỎ** + `cr0_wp=0` | XONG |
| soak `--selftest` | 49 ca (2 dương/47 âm) | PASS | XONG |

---

## 7. Cổng chạy

```text
./v2/bin/v2 check                     → (xem §7.1)
bash v2/test/boot/run-x86_64.sh 1     → (xem §7.2)
bash v2/test/boot/soak-x86_64.sh --smoke 60 3 → (xem §7.3)
```

### 7.1 `v2 check` — **RC=0, PASS** (183.261 s → 116.703 s lượt chạy lại sau khi sửa)

```text
--- wake theo dep-info (KT-217: lớp 2): OK
...
# step-time-total   116703 ms  TỔNG check
```

**Một lần đỏ thật đã gặp và sửa**: lượt `v2 check` đầu, bước `wake theo dep-info` ĐỎ
`E_DEPINFO_BUILD` vì `cargo check --workspace --target riscv64gc-unknown-none-elf` của
**workspace kernel** RC=101 — `wx::enable`/`wx::report` chỉ có `#[cfg(x86_64)]` nên target
riscv64 thiếu hàm. Sửa: thêm **stub kiểm kiểu** cho target khác (trả `WxReport::default()`) —
đúng khuôn `asm.rs` đã dùng cho `read_cr0`… Không nới luật nào. Sau sửa: `rules: PASS`,
`v2 check` PASS.

### 7.2 `run-x86_64.sh` — **RC=0, 36/36 kiểm sạch**

```text
ok 1 - D2-08: ảnh sản phẩm 0 harness marker (ktest=77 ⇒ bộ quét có răng)
ok 2 - sản phẩm: mark 1 lần + halt + 0 TAP + '# conf image' khớp ELF (D2-80)
ok 3 - ca âm: log sản phẩm bị bộ kiểm TAP từ chối (không TAP nào)
ok 4 - elf: entry=_start, .text tại 0x100000, multiboot2 + PVH note đúng
ok 5 - boot 1: TAP 36/36 + 6 dòng '# exc:' + marker 1 lần + halt sạch
ok 6 - ca âm: log ktest bị bộ kiểm sản phẩm từ chối (có TAP)
ok 7 - ca âm (a): marker sai ⇒ ĐỎ
ok 8 - ca âm (b): ảnh marker sai ⇒ ĐỎ
ok 9 - ca âm (b2): no-wx ⇒ ĐỎ + '# wx: cr0_wp=0' (# có 34 dòng 'ok', cần 36)
...
ok 39 - ca dương (KT-313): picks=[2, 4, 1, 1] first=[a, a, u, c] diff=1 ⇒ XANH
# ca âm: 32/32 ĐỎ đúng như phải (+ 1 ca dương KT-313)
# 36/36 kiểm sạch (boot=1, tests=36, exc=6)
# infra: cpu=qemu64 guard_log=… qemu_own=0
```

Ảnh **sản phẩm** boot thật (log nguyên văn):

```text
# conf image: build=0a1a704b34d21fdb text=69632
# viFastOs x86_64: long mode cr0=0x0000000080010011 cr4=0x0000000000000020 cr3=0x0000000000115000 efer=0x0000000000000d00 lma=1
# viFastOs x86_64: boot eax=0x00000000 ebx=0x00001580 pvh_magic=0x336ec578 pvh_ok=1
# viFastOs x86_64: serial com1=0x3f8 divisor=1 lcr=0x03
# viFastOs x86_64: identity map 0x0..0x40000000 pages=512 pml4=0x115000
# viFastOs x86_64: product gdt/idt ok=1
# viFastOs x86_64: boot ok
# viFastOs x86_64: halt
```

`cr0=…8001_0011` — **bit 16 = 1** (`CR0.WP`) ngay cả ở ảnh sản phẩm; **0** dòng TAP.

### 7.3 `soak-x86_64.sh --smoke 60 3` — **60/60 boot xanh, 0 lỗi** (RC=1 theo **hợp đồng
smoke**, không phải đỏ)

```text
# soak: xong wall=209s (0 boot/s); ok=60 fail=0 tổng=60/60
# soak: BUILD_ID=10ab0f802f977468 sha256=6d41ea103333c6a7a2530de78c62da5bfc4dbf84f98e2df3a73453120fdf38e6 git=99ee94dfd90f3a9a9ecb2bb489bbb225d63fb2d4
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 600 — KHÔNG phải bằng chứng cổng K5' (D2-24), luôn exit 1
```

**T-INFRA ghi lại (không retry)**: `# MEASUREMENT_INVALID tag=after ly_do=loadavg_1m=12.41>ncpu=12`
— máy dùng chung với agent khác đang chạy QEMU aarch64 (đúng cảnh báo đầu task) nên phép đo
soak bị đánh dấu **infra**, không phải lỗi sản phẩm; 60/60 boot vẫn sạch. Không chạy lại để lấy
xanh (D2-07); bằng chứng nghiệm thu soak đầy đủ là việc của `KT-348`.

### 7.4 Đối chứng đột biến W^X (log nguyên văn)

```text
# wx: cr0_wp=0 … text=0x0000000000100023 … wx_pages=512 pt=0x12b000
not ok 35 - wx: kernel sections
```

(`0x23` = `P|RW|X`; `wx_pages=512` — cả 2 MiB RWX; TAP 35 **ĐỎ** ⇒ phép đo có răng.)

---

## 8. Tệp đã chạm

| Tệp | Thay đổi |
|---|---|
| `v2/test/boot/budget-x86_64.sh` | **mới** — cổng ngân sách x86_64 (e_machine=62, `.smp_stacks` riêng, 10 ca selftest) |
| `v2/bin/v2` | `step_x64_budget` + `lane_step D 76` + `GATE_FAST_EXTRA_EXPECTED=29` |
| `v2/kernel/vifast-kbin-x86_64/Cargo.toml` | feature `ktest`, `no-wx`, `wx-data-exec`; doc tách ảnh |
| `v2/kernel/vifast-kbin-x86_64/src/main.rs` | `__pt_kernel`; `mod product`/`mod wx_probe`; `cfg(ktest)` cho khối kiểm + mô-đun; `wx_init` |
| `v2/kernel/vifast-kbin-x86_64/src/product.rs` | **mới** — nhánh sản phẩm + handler tối giản + `wx_init`/`wx_report` |
| `v2/kernel/vifast-kbin-x86_64/src/wx_probe.rs` | **mới** — TAP 35/36 |
| `v2/kernel/vifast-kbin-x86_64/src/net.rs` | gọi `wx_probe` (TAP 35/36); gpu dời 35→37 |
| `v2/kernel/vifast-kbin-x86_64/linker.ld` | symbol `__data_start` (ranh giới W^X) |
| `v2/kernel/vifast-plat-x86_64/src/wx.rs` | **mới** — dựng PT/đổi `PD[0]`/`CR0.WP`/`walk_leaf`/`WxReport` |
| `v2/kernel/vifast-plat-x86_64/src/asm.rs` | `write_cr0/write_cr3/invlpg/probe_write8` (+ stub host) |
| `v2/kernel/vifast-plat-x86_64/src/lib.rs` | đăng ký `pub mod wx` |
| `v2/test/boot/run-x86_64.sh` | build 2 ảnh, P1..P4, ca (b2) no-wx, `EXPECT_TESTS=36`, `EXPECT_EXC=6` |
| `v2/test/boot/soak-x86_64.sh` | build 2 ảnh, quét marker + ca âm, `TAP_TESTS=36`, `EXPECT_EXC=6`, fixture selftest |
| `v2/test/boot/gpu-x86_64.sh` | `--features ktest,gpu`, `EXPECT_TESTS=37`, budget-x86_64 |
| `v2/agent/JOURNAL/kt340a-vspace.md` | tệp này |

**Không** chạm: `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2` (chỉ thêm bước cổng).
