# KT-248 — K4' bước 3, **B3.2**: bảng vector 16 ô + `VBAR_EL1` + handler đồng bộ

**Ngày**: 2026-11-21 · **Sub-agent** `a64` · nhánh `task/kt243-a64` · base `main` @ `43a054e3`
**Hàng QUEUE**: `KT-248` (D2-195) · **Phụ thuộc**: KT-247 (commit `5c1eafe8`)
**Phạm vi**: **chỉ B3.2**. Không MMU (KT-249), không TAP (KT-250). **Không tuyên bố K4' đạt.**

---

## 1. Đỏ trước (D2-05)

Ca kiểm viết **trước** khi `vector` tồn tại — log [`kt248-red.log`](kt248-red.log):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vector
error[E0432]: unresolved import `vifast_plat_aarch64::vector`
error[E0599]: no variant or associated item named `Brk64` found for enum `ExceptionClass`
error[E0599]: no associated function named `from_bits` found for struct `Esr`
error: could not compile `vifast-plat-aarch64` (test "vector") due to 7 previous errors
```

Đỏ **đúng lý do**: ba thứ mà ca kiểm đòi (mô-đun `vector`, `EC` của `BRK`, `Esr::from_bits`) chưa có.

**Sau khi sửa** — 8 test host, kỳ vọng là **offset tuyệt đối viết tay** từ DDI 0487:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vector
running 8 tests
test brk_exception_class ... ok
test brk_from_el1_uses_spx_sync_slot ... ok
test esr_from_bits_round_trip ... ok
test vbar_masks_low_bits ... ok
test vector_geometry ... ok
test vector_kinds_in_order ... ok
test vector_offset_out_of_range ... ok
test vector_offsets_are_absolute ... ok
test result: ok. 8 passed; 0 failed; 0 ignored
```

## 2. Đã làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/vector.rs` | **mới** — `VECTOR_COUNT=16`, `VECTOR_STRIDE=0x80`, `VECTOR_ALIGN=0x800`, `VectorKind` (16 loại + `ALL` + `name()`), `vector_offset()`, `install(base)` (ghi `VBAR_EL1` → `ISB` → **đọc lại**), `is_aligned()` |
| `v2/kernel/vifast-plat-aarch64/src/sysreg.rs` | thêm `ExceptionClass::Brk64 = 0x3C` (+ `from_bits`) và `Esr::from_bits(raw)` — **không** đổi bit/field nào đã có |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | thêm `brk_probe()`/`brk_probe_addr()` (probe `brk #0` + địa chỉ instruction) + stub host panic |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | bảng vector `global_asm!` (16 stub × `0x80`, `.org` lấy `VECTOR_STRIDE` qua `const` operand), `vec_common` (khung 176 B, đọc `ESR/FAR/ELR/SPSR`, gọi `vec_handler`, `eret` khi resume), `vec_handler` Rust, probe `brk #0`, in bằng chứng |
| `v2/kernel/vifast-kbin-aarch64/linker.ld` | output section `.text.vectors` (`ALIGN(0x800)`, `KEEP(*(.vectors))`, `__vector_base`) |

### Bất biến: bảng vector **không** nằm trong `.bss`

`.bss` là `NOLOAD` ⇒ bảng nằm trong đó sẽ **không có** trong ảnh đã nạp và `VBAR_EL1` trỏ vào
vùng rác (QEMU có thể tình cờ có RAM 0 nên "chạy được", máy thật thì không). Bảng nằm trong
`.text.vectors` (`"ax"`), và `objdump -h` chứng minh nó là `TEXT`:

```text
Idx Name          Size     VMA              Type
  1 .text         00001210 0000000040080000 TEXT
  2 .text.vectors 00000810 0000000040081800 TEXT      ← bảng vector, KHÔNG phải BSS
  3 .rodata       0000041c 0000000040083000 DATA
  5 .bss          00000030 0000000040085000 BSS
```

### Lỗi thật đã gặp khi dựng (ghi lại vì cùng lớp lỗi có thể tái diễn)

Lần đầu, input section tên `.text.vectors` bị `*(.text .text.*)` của `.text` **nuốt** mất: các
symbol `__vec_*` nằm trong `.text` còn `__vector_base` (linker cấp cho output section riêng) trỏ
vào `.text.vectors` **size 0** ⇒ `VBAR_EL1` trỏ vào `.rodata`. Sửa: đổi **tên input section**
thành `.vectors` (không khớp `*(.text.*)`), giữ output section tên `.text.vectors`. Kiểm bằng
`objdump -t`: `__vec_0` = `__vector_base` = `0x4008_1800`.

## 3. Đo được (nguyên văn)

Log đầy đủ: [`kt248-vector.log`](kt248-vector.log).

### (a) Test host: offset tuyệt đối từng ô + `VBAR_BASE_MASK`

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vector
test result: ok. 8 passed; 0 failed
```

`vector_offsets_are_absolute` khẳng định **từng** giá trị `0x000, 0x080, …, 0x780` (mảng viết tay
trong test, **không** sinh từ `vector_offset`); `vbar_masks_low_bits` khẳng định
`VBAR_BASE_MASK == !0x7FF` và `Vbar::new(0x4008_0FFF).bits() == 0x4008_0800`.

### (b) ELF: từng stub đúng `base + i*0x80`

```text
$ xcrun llvm-objdump -t <elf> | grep __vec_
0000000040081800 g  .text.vectors  __vec_0
0000000040081880 g  .text.vectors  __vec_1
0000000040081900 g  .text.vectors  __vec_2
0000000040081980 g  .text.vectors  __vec_3
0000000040081a00 g  .text.vectors  __vec_4
0000000040081f80 g  .text.vectors  __vec_15
0000000040081800 g  .text.vectors  __vector_base

$ bash /tmp/kt248-elfcheck.sh <elf>
vector-elf: base=0000000040081800 slots=16 bad=0
```

### (c) QEMU: `brk #0` in đúng `EC` của `BRK` và `ELR` trỏ đúng chỗ

```text
$ qemu-system-aarch64 -M virt -cpu cortex-a72 -m 128M -nographic -kernel <elf> -no-reboot
# viFastOs aarch64: boot ok
# vector: base=0x40081800 vbar=0x40081800 slots=16 stride=0x80 align=0x800 aligned=1
# brk: count=1 slot=4 kind=sync-cur-spx off=0x200 ec=0x3c il=1 elr=0x40080038 expect=0x40080038 match=1 spsr=0x600003c5 far=0x0
# trap: other=0
```

Đọc từng trường:

| Trường | Giá trị | Ý nghĩa |
|---|---|---|
| `vbar` | `0x40081800` | **đọc lại** `VBAR_EL1` sau `ISB` = `base` ⇒ thanh ghi thật sự giữ địa chỉ bảng |
| `slot`/`off` | `4` / `0x200` | `brk #0` từ EL1h đi vào ô **Current EL with SPx / Synchronous** — đúng DDI 0487 |
| `ec` | `0x3c` | `EC` của `BRK` (không phải `0x30`/`0x31` của breakpoint phần cứng) |
| `il` | `1` | instruction 32 bit |
| `elr`/`expect`/`match` | `0x40080038` / `0x40080038` / `1` | `ELR_EL1` = **địa chỉ instruction `brk #0`**; handler cộng 4 rồi `eret` ⇒ boot đi tiếp |
| `spsr` | `0x600003c5` | `M[3:0] = 0b0101` = EL1h; `DAIF` = `0b1111` (mask đủ) |
| `other` | `0` | không có trap nào khác |

`qg_boot rc=0` (QEMU **tự** thoát qua PSCI), `marker_count=1`, không `PANIC`.

## 4. Đột biến bắt buộc — **ĐỎ** (log [`kt248-mutation.log`](kt248-mutation.log))

| # | Đột biến | Lớp kiểm | Kết quả |
|---|---|---|---|
| 1 | `vector_offset(index)` lệch `0x80` từ ô thứ 3 (index ≥ 2) | test host `vector_offsets_are_absolute` | **ĐỎ**: `assertion failed: ô thứ 2 — left: Some(384) right: Some(256)` |
| 2 | asm `.org (idx - (idx >= 2)) * 0x80` (ô thứ 3 trở đi lệch `0x80`) | kiểm ELF `base + i*0x80` | **ĐỎ**: `bad=14`, `FAIL slot 2: __vec_2=0x40081980 want=0x40081900` |

Cả hai đột biến **hoàn nguyên** rồi đo lại: test host 8/8 xanh, `vector-elf bad=0`.

**Ghi chú kỹ thuật**: lần thử đột biến #2 đầu tiên dùng `.org (idx + (idx >= 2)) * 0x80` và
**không assemble được** (GAS trả `-1` cho so sánh đúng ⇒ `.org` lùi). Đã ghi lại trong log để
người sau không tưởng đó là "đột biến xanh": đột biến chỉ tính khi **build được** và lớp kiểm
**ĐỎ**.

## 5. Còn thiếu / rủi ro

1. **Chưa có TAP** — ảnh in dòng bằng chứng dạng `#`, TAP là KT-250.
2. **Chưa MMU** — mọi fault hiện dừng máy (`debug_halt(2)`) sau khi in `# trap: …`; KT-249 bật MMU
   và dùng chính đường này để chứng minh đột biến `T0SZ` **ĐỎ** (translation fault in ra thay vì
   treo im lặng).
3. **Chưa xử lý IRQ/FIQ/SError** — 16 ô dùng **chung** một stub; IRQ/FIQ chưa bật (`DAIF` mask
   đủ) nên chưa có đường nào tới đó. Khi bật ngắt (bước sau) phải tách handler theo ô.
4. **`FAR_EL1` của `BRK`** không mang nghĩa (in ra `0x0`) — chỉ in, **không** khẳng định.
5. **Chưa đọc DTB** (như KT-247).
