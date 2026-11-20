# KT-249 — K4' bước 3, **B3.3**: bật MMU (TCR → MAIR → TTBR0 → SCTLR → ISB) + identity map

**Ngày**: 2026-11-21 · **Sub-agent** `a64` · nhánh `task/kt243-a64` · base `main` @ `43a054e3`
**Hàng QUEUE**: `KT-249` (D2-195) · **Phụ thuộc**: KT-248 (commit `6127bb2b`)
**Phạm vi**: **chỉ B3.3**. Không nửa cao/physmap (B3.4 — D2-195 hoãn Q4), không `VSpace` (B3.5),
không TAP (KT-250). **Không tuyên bố K4' đạt.**

---

## 1. Đỏ trước (D2-05)

Ca kiểm viết **trước** khi `boot_mmu`/`bootmem` tồn tại — log [`kt249-red.log`](kt249-red.log):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test mmu_boot
error[E0432]: unresolved import `vifast_plat_aarch64::boot_mmu`
error[E0432]: unresolved import `vifast_plat_aarch64::bootmem`
error[E0599]: no associated function or constant named `from_bits` found for struct `Sctlr`
error: could not compile `vifast-plat-aarch64` (test "mmu_boot") due to 10 previous errors
```

**Sau khi sửa** — 10 test host:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test mmu_boot
running 10 tests
test boot_memory_exhausts ... ok
test boot_memory_round_trip ... ok
test mair_slot1_is_normal_wb ... ok
test map_identity_aligns_outward ... ok
test map_identity_empty ... ok
test map_identity_range ... ok
test mmu_enable_sequence_order ... ok
test sctlr_sets_mmu_bit ... ok
test tcr_start_level ... ok
test ttbr0_is_level1_table ... ok
test result: ok. 10 passed; 0 failed; 0 ignored
```

## 2. Đã làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/boot_mmu.rs` | **mới** — `MMU_ENABLE_SEQUENCE` (5 bước, **dữ liệu**), `apply_step`, `enable_mmu`, `start_level(t0sz)` (bảng D5-5), `sctlr_for_mmu_on` (giữ bit, chỉ đặt `M`), `mmu_is_on`, `ttbr0_base` (**bảng mức 1**), `map_identity` (trang 4 KiB, làm tròn ra ngoài) |
| `v2/kernel/vifast-plat-aarch64/src/bootmem.rs` | **mới** — arena 64 frame = 256 KiB trong `.bss`, `BootMemory: TableMemory` trên mảng `AtomicU64` căn 4 KiB (**không `unsafe`**) |
| `v2/kernel/vifast-plat-aarch64/src/sysreg.rs` | thêm `Sctlr::from_bits` (giữ nguyên bit khi bật `M`) |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | thêm `read_u64`/`write_u64` (đọc/ghi địa chỉ tuyệt đối cho phép đo "MMU thật sự bật") + stub host panic |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | `mmu_bringup`: dựng bảng trang, identity map ảnh + UART + GIC, map trang probe vào **VA không identity**, bật MMU với marker **trước/sau từng bước**, đo 3 phép, fail-loud |

### Hai quyết định kỹ thuật quan trọng (dễ sai, đã kiểm bằng test)

**(a) `TTBR0_EL1` phải trỏ vào bảng mức 1, không phải gốc mức 0.**
Với granule 4 KiB, mức bắt đầu của walk do `T0SZ` quyết định (DDI 0487 bảng D5-5): `T0SZ = 25`
(39-bit VA) ⇒ bắt đầu ở **mức 1**. `PageTable<Aarch64, _>` dựng cây 4 mức (gốc mức 0 → lá mức 3),
nên đưa gốc mức 0 vào `TTBR0_EL1` sẽ khiến phần cứng đọc nó như bảng mức 1 ⇒ **mọi địa chỉ dịch
sai**. `ttbr0_base()` lấy con của gốc; test `ttbr0_is_level1_table` chốt `Some(FRAME_BASE+0x1000)`
và `!= root`.

**(b) Arena bảng trang không cần `unsafe`.**
`static mut [u8; N]` / `UnsafeCell` đều cần `unsafe`. Arena ở đây là
`#[repr(align(4096))] struct Arena([AtomicU64; N])` — căn trang bằng `repr(align)`, đọc/ghi entry
bằng `load`/`store(Relaxed)`, bộ cấp phát là biến đếm trong struct. Nhờ vậy `unsafe` **vẫn chỉ**
nằm ở `asm.rs` (D2-17), và `BootMemory` test được trên host.

## 3. Đo được (nguyên văn)

Log đầy đủ: [`kt249-mmu.log`](kt249-mmu.log).

```text
$ qemu-system-aarch64 -M virt -cpu cortex-a72 -m 128M -nographic -kernel <elf> -no-reboot
# viFastOs aarch64: boot ok
# vector: base=0x40082800 vbar=0x40082800 slots=16 stride=0x80 align=0x800 aligned=1
# brk: count=1 slot=4 kind=sync-cur-spx off=0x200 ec=0x3c il=1 elr=0x40080038 expect=0x40080038 match=1 spsr=0x600003c5 far=0x0
# trap: other=0
# mmu: arena=0x40089000 frames=64 used=8 root=0x40089000 ttbr0=0x4008a000 level=1
# mmu: map kernel=0x40080000-0x400da000 pages=90 uart=1 gic=32 probe_va=0x50000000 probe_pa=0x40087000
# mmu: tcr=0x00000015b5193519 mair=0x000000000000ff00 sctlr=0x0000000000c50839
# mmu: step 1/5 tcr begin
# mmu: step 1/5 tcr ok
# mmu: step 2/5 mair begin
# mmu: step 2/5 mair ok
# mmu: step 3/5 ttbr0 begin
# mmu: step 3/5 ttbr0 ok
# mmu: step 4/5 sctlr begin
# mmu: step 4/5 sctlr ok
# mmu: step 5/5 isb begin
# mmu: step 5/5 isb ok
# mmu: on=1 sctlr=0x0000000000c50839 m=1
# mmu: marker before=0x5649464153540001 after=0x5649464153540001 same=1
# mmu: probe va=0x50000000 read=0x4d4d550000000001 magic=0x4d4d550000000001 match=1
```

`qg_boot rc=0` (QEMU tự thoát), `marker_count=1`, `mmu_steps_ok=5`, không `PANIC`.

| Phép đo | Kết quả | Ý nghĩa |
|---|---|---|
| `SCTLR_EL1.M` sau `ISB` | `on=1 m=1`, `sctlr=0x…c50839` | MMU bật; giá trị reset `0xc50839` **được giữ** (chỉ thêm bit `M`) |
| marker qua VA | `before == after == 0x5649464153540001`, `same=1` | identity map đúng: đọc lại qua VA sau khi bật không đổi giá trị |
| **probe VA không identity** | `va=0x50000000` → `read=0x4d4d550000000001` = `magic`, `match=1` | **translation thật sự chạy** — không chỉ bit `M` được bật. VA `0x5000_0000` nằm **ngoài** RAM 128 MiB; nếu MMU tắt thì đọc thẳng PA đó và **không** ra magic |
| frame đã dùng | `used=8` / `frames=64` | gốc mức 0 + mức 1 + 2 bảng mức 2 + 4 bảng mức 3 (kernel/UART/GIC/probe) |

## 4. Ba đột biến bắt buộc — log [`kt249-mutation.log`](kt249-mutation.log)

| # | Đột biến | Tầng host | Tầng QEMU |
|---|---|---|---|
| **(i)** | bỏ `ISB` khỏi `MMU_ENABLE_SEQUENCE` | **ĐỎ**: `assertion failed: chuỗi phải đủ 5 bước — left: 4` | **KHÔNG quan sát được** (`rc=0`, mọi phép đo vẫn xanh) |
| **(ii)** | `TCR.T0SZ` 25 → 24 | **ĐỎ**: `tcr_start_level` — `left: 24` | **TREO CÓ KIỂM SOÁT**: `qg_boot rc=124` (timeout 5 s), log dừng đúng ở `# mmu: step 4/5 sctlr begin` |
| **(iii)** | `MAIR` slot 1 `0xFF` → `0x00` | **ĐỎ**: `mair_slot1_is_normal_wb` — `left: 0` | **KHÔNG quan sát được** (`rc=0`, `mair=0x…0000` nhưng mọi phép đo vẫn xanh) |

**Đọc đúng ba dòng trên (không tô hồng):**

- **(ii) là đột biến mạnh nhất**: nó chứng minh cả hai tầng. Trên QEMU, sau `msr sctlr_el1` với
  `T0SZ = 24`, walk bắt đầu ở mức 0 nhưng `TTBR0` trỏ vào bảng mức 1 ⇒ **instruction abort** ngay
  ở lệnh kế tiếp; bảng vector cũng không dịch được ⇒ CPU fault lặp vô hạn ⇒ **treo**. Đây đúng
  là "treo có kiểm soát" mà thiết kế dự phòng: chứng minh bằng **timeout** (`rc=124`) và bằng
  **vị trí dừng** trong log (marker trước/sau từng bước — R5 — chỉ đúng bước 4).
- **(i) và (iii) chỉ ĐỎ ở tầng host.** Lý do kỹ thuật, không phải "test yếu":
  - `ISB` là yêu cầu **context synchronization** của đặc tả; QEMU TCG áp dụng `msr sctlr_el1`
    ngay cho lần truy cập kế tiếp nên bỏ `ISB` không đổi hành vi **trên QEMU**. Máy thật mới lộ.
  - `MAIR` chỉ có hiệu lực khi CPU mô hình hoá thuộc tính bộ nhớ (Device cấm gộp/đoán trước,
    cấm truy cập lệch căn); **TCG không mô hình hoá** ⇒ đổi slot 1 thành Device không đổi kết quả.
  - Vì vậy hai đột biến này được giữ **ĐỎ ở tầng host** (chuỗi là dữ liệu, hằng số có test tuyệt
    đối) và **ghi rõ là KHÔNG chứng minh được trên QEMU** — không được tính là "đã kiểm trên
    phần cứng".

## 5. Còn thiếu / rủi ro

1. **Chưa nửa cao / physmap** (B3.4) — D2-195 hoãn Q4. `TTBR1_EL1` chưa dùng; kernel chạy nửa
   thấp.
2. **Chưa `VSpace`/`unmap`/`TLBI`** (B3.5/B3.6) — `PageTable` chỉ map, chưa unmap trên phần cứng.
3. **Chưa TAP** (KT-250) — các dòng `# mmu:` là bằng chứng dạng `#`, runner sẽ kiểm chúng.
4. **W^X chưa đầy đủ**: vùng ảnh map `KERNEL_RWX` (một khối) — tách `.text` RX / `.data` RW là
   việc của tầng VSpace (B3.5), không thuộc bước 3.
5. **Arena 64 frame là hằng**: nếu bước sau map nhiều hơn, `alloc_frame` trả `None` ⇒ boot
   `PANIC` (fail-loud), không ghi ngoài arena.
6. **`free_frame` là no-op** trên arena boot — ghi rõ trong `bootmem.rs`; `destroy` không được
   gọi trên bảng của kernel.

## 6. Bổ sung sau KT-250 — arena 64 → 32 frame để đạt bất biến **I7** (`.data + .bss ≤ 256 KiB`)

Khi kiểm budget ở KT-250 (bất biến I7 của `kt242-k4-buoc3-design.md` §3: trần K0.5 áp cho **mọi**
bước), ảnh aarch64 **vượt** trần:

```text
$ xcrun llvm-objdump -h <elf> | awk '$2==".data"||$2==".bss"{print $2, $3}'
.data 00000008
.bss  00043000          # 268 KiB — VƯỢT trần 262144 B
```

Nguyên nhân: arena bảng trang **64** frame (256 KiB) + trang probe (4 KiB). Đo được boot chỉ dùng
**8** frame ⇒ giảm arena còn **32** frame (128 KiB, vẫn **4×** headroom cho B3.4):

```text
$ xcrun llvm-objdump -h <elf> | awk '$2==".data"||$2==".bss"||$2==".stack"{print $2, $3}'
.data  00000008
.bss   00023000         # 140 KiB tổng .data+.bss — ĐẠT
.stack 00010000         # NOLOAD, báo riêng (D2-72 Q5) — không cộng vào trần
```

Đo lại sau khi giảm (log [`kt249-mmu.log`](kt249-mmu.log), cập nhật): `frames=32 used=8`,
`pages=58` (ảnh nhỏ hơn), `on=1`, `same=1`, `match=1`, `qg_boot rc=0`, TAP 5/5.

**Không nới ngưỡng**: trần giữ nguyên 262144 B; thứ được sửa là **kích thước arena** (một hằng
của ảnh), không phải ngưỡng của cổng.
