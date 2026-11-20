# KT-247 — K4' bước 3, **B3.1**: crate ảnh `vifast-kbin-aarch64` + linker + `_start` + PL011

**Ngày**: 2026-11-21 · **Sub-agent** `a64` · nhánh `task/kt243-a64` · base `main` @ `43a054e3`
**Hàng QUEUE**: `KT-247` (D2-195 mở K4' bước 3, phạm vi B3.1–B3.3 + B3.8)
**Phạm vi task này**: **chỉ B3.1**. Không MMU, không bảng vector, không U-mode, không TAP.
**Không tuyên bố K4' đạt** — tiêu chí K4' là "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000), bước sau.

---

## 1. Đỏ trước (D2-05)

Ca kiểm thể hiện yêu cầu được viết **trước** khi `pl011`/`resources` tồn tại:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test pl011
error[E0432]: unresolved import `vifast_plat_aarch64::pl011`
  --> vifast-plat-aarch64/tests/pl011.rs:20:26
error[E0432]: unresolved import `vifast_plat_aarch64::resources`
  --> vifast-plat-aarch64/tests/pl011.rs:25:26
error: could not compile `vifast-plat-aarch64` (test "pl011") due to 2 previous errors
```

Log: [`kt247-red.log`](kt247-red.log). Đỏ **đúng lý do**: hai mô-đun mà ca kiểm đòi chưa có.

**Sau khi sửa** (13 test host, giá trị kỳ vọng viết tay từ DDI 0183 + memory map QEMU `virt`):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test pl011
running 13 tests
test control_register_bits ... ok
test divisors_other_rates ... ok
test divisors_qemu_virt_115200 ... ok
test divisors_rejects_impossible ... ok
test flag_register_bits ... ok
test init_sequence_order ... ok
test init_step_values ... ok
test interrupt_bits ... ok
test line_control_bits ... ok
test qemu_virt_addresses ... ok
test register_offsets_match_table ... ok
test register_offsets ... ok
test resources_do_not_overlap ... ok
test result: ok. 13 passed; 0 failed; 0 ignored
```

## 2. Đã làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/resources.rs` | **mới** — bảng tài nguyên QEMU `virt` aarch64 (UART0 `0x0900_0000`, GICD `0x0800_0000`, GICC `0x0801_0000`, RAM `0x4000_0000`) + `first_overlap` (khuôn KT-81) |
| `v2/kernel/vifast-plat-aarch64/src/pl011.rs` | **mới** — offset/bit thanh ghi PL011, `divisors()` (const fn, làm tròn nửa lên bằng số nguyên), `INIT_SEQUENCE` (dữ liệu ⇒ test host khẳng định **thứ tự**), `write_byte` chờ `FR.TXFF` với **trần** `TX_WAIT_LIMIT` |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | thêm `mmio_read_u32`/`mmio_write_u32` (MMIO, `unsafe` **vẫn chỉ ở tệp này**), `psci_system_off` (`hvc #0`, `x0 = 0x8400_0008`), `wfi_forever`; stub host **panic** cho cả ba |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | khai báo `pl011`/`resources`, thêm `debug_halt` (PSCI → dự phòng `wfi`) |
| `v2/kernel/vifast-plat-aarch64/tests/pl011.rs` | **mới** — 13 test host (ca kiểm đỏ-trước) |
| `v2/kernel/vifast-kbin-aarch64/` | **mới** — `Cargo.toml`, `build.rs` (`-T linker.ld`, `--gc-sections`), `linker.ld` (nạp `0x4008_0000`, `.stack` NOLOAD 64 KiB + guard 4 KiB), `src/main.rs` (`_start` bằng `global_asm!`, zero `.bss`, `kmain`, `#[panic_handler]`) |
| `v2/kernel/Cargo.toml` | thêm **một** dòng member `vifast-kbin-aarch64` |

**Không** `unsafe` ngoài `vifast-plat-aarch64::asm` (luật R5): `_start` dùng `global_asm!` (không cần
`unsafe`), driver PL011 gọi qua `asm::mmio_*`.

## 3. Đo được (nguyên văn)

Log đầy đủ: [`kt247-boot.log`](kt247-boot.log).

### (a) Build xanh

```text
$ cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-aarch64 \
      --target aarch64-unknown-none --release
    Finished `release` profile [optimized] target(s) in 1.05s
```

### (b) `.text` bắt đầu đúng `0x4008_0000`

```text
$ xcrun llvm-objdump -h v2/kernel/target/aarch64-unknown-none/release/vifast-kbin-aarch64
Idx Name          Size     VMA              Type
  1 .text         00000294 0000000040080000 TEXT
  2 .rodata       00000064 0000000040081000 DATA
  3 .bss          00000000 0000000040082000 BSS
  4 .stack_guard  00001000 0000000040082000 BSS
  5 .stack        00010000 0000000040083000 BSS

$ xcrun llvm-objdump -f <elf>
architecture: aarch64
start address: 0x0000000040080000
```

### (c) QEMU in **đúng một** dòng marker rồi halt sạch

```text
$ qemu-system-aarch64 -M virt -cpu cortex-a72 -m 128M -nographic -kernel <elf> -no-reboot
# viFastOs aarch64: boot ok
```

- 3/3 lần boot: `qg_boot rc=0`, `lines=1`, nội dung `# viFastOs aarch64: boot ok|`
- marker xuất hiện **đúng 1 lần** mỗi log (`grep -c` = 1)
- **không** treo: `qemu_guard.sh` (process group + timeout 5 s) trả 0 = QEMU **tự** thoát
- `sha256 4852009c246c6b8aa81de3c0c895c892ef1832d5408e3d02a43ebf57a046a18d` (nợ P20: sha phụ
  thuộc thư mục build — danh tính bền là commit + build-inputs)

**Vì sao halt sạch**: `debug_halt` gọi `PSCI SYSTEM_OFF` (`hvc #0`, `x0 = 0x8400_0008`). QEMU
`virt` đặt PSCI conduit = HVC khi khách ở EL1 (đúng đường `-kernel`), nên `SYSTEM_OFF` làm QEMU
thoát mã 0. Nếu môi trường không có PSCI, hàm **không** im lặng: nó rơi vào `wfi_forever` và
runner phân biệt bằng marker đã in trước đó.

## 4. Còn thiếu so với thiết kế (ghi rõ, không giấu)

1. **Chưa đọc DTB** (R4 của `kt242-k4-buoc3-design.md` §5 đề nghị lấy địa chỉ từ DTB). Bước này
   dùng hằng của `resources.rs` (memory map QEMU `virt`), **chưa** có FDT parser. `_start` vẫn
   nhận DTB trong `x0` và chuyển vào `kmain` — KT-250 sẽ in `magic` để chứng minh con trỏ là thật.
2. **Chưa có `Platform` impl cho aarch64** — `PanicHandler` dùng chung của `vifast-plat-common`
   đòi cả `now_ticks`/`hart_id`/`set_trap_vector`/`enable_irq`… (thuộc bước sau). Ảnh dùng
   `#[panic_handler]` tối giản tại chỗ, vẫn in dòng `PANIC` mà runner coi là đỏ.
3. **Chưa có TAP** — ảnh in **một** dòng marker (đúng tiêu chí B3.1). TAP là KT-250.
4. **`.bss` hiện rỗng** (0 byte) vì ảnh chưa có `static` nào; KT-249 sẽ có arena bảng trang.

## 5. Rủi ro / bài học cho bước sau

- **R5 (bật MMU sai thứ tự ⇒ treo im lặng)**: KT-249 phải in marker **trước và sau** từng bước
  của chuỗi TCR→MAIR→TTBR0→SCTLR→ISB. Ảnh hiện chưa có bảng vector nên **mọi** fault ở EL1 lúc
  bật MMU sẽ treo (không in gì) — đó là lý do KT-248 (bảng vector) phải **xong trước** KT-249.
- **`perl -e 'alarm N; exec @ARGV'` không giết được QEMU** (lead đã kiểm): mọi lần chạy QEMU
  trong task này đi qua `v2/test/boot/qemu_guard.sh` (process group + timeout + kill group).
- **`cargo fmt --all` trên workspace kernel**: đã kiểm `cargo fmt --manifest-path v2/kernel/Cargo.toml
  --all --check` = sạch, **không** chạm tệp ngoài phạm vi (git status chỉ có tệp của KT-247).
