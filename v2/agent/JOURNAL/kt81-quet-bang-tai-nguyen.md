# KT-81 §A5b — quét **toàn repo** tìm địa chỉ/kích thước viết tay của 6 vùng

**Ngày**: 2026-11-18 · **Agent 1** · HEAD trước task: `c6a97351`

## 0. Việc Agent 0 giao (nguyên văn §A5b)

> b) Luật 2 cho bảng tài nguyên: quét toàn repo (kernel, lib, linker script, DTB, script QEMU) tìm
> địa chỉ/kích thước viết tay của 6 vùng. Mỗi chỗ: dẫn xuất từ REGIONS, hoặc thêm phép kiểm khớp
> REGIONS, hoặc ghi lý do không làm được. Kết quả quét vào JOURNAL.

Sáu vùng trong [`v2/kernel/vifast-plat-common/src/resources.rs`](../../kernel/vifast-plat-common/src/resources.rs):
`UART0` `0x1000_0000`/`0x1000` · `PLIC` `0x0c00_0000`/`0x0400_0000` · `CLINT` `0x0200_0000`/`0x1_0000` ·
`TEST_PAGE` `0x0010_0000`/`0x1000` · `PCI_ECAM` `0x3000_0000`/`0x1000_0000` · `PCI_MMIO32` `0x4000_0000`/`0x4000_0000`.

## 1. Cách quét

```text
$ grep -rn --include=*.rs --include=*.S --include=*.ld --include=*.dts --include=*.dtsi \
    --include=*.sh --include=*.toml --include=*.py \
    -E "0x(1000_0000|10000000|0c00_0000|0c000000|0200_0000|02000000|0010_0000|00100000|3000_0000|30000000|4000_0000|40000000)\b" \
    v2/kernel v2/lib v2/test v2/tools spec | grep -v "/target/"
```

Phạm vi: `v2/kernel`, `v2/lib`, `v2/test`, `v2/tools`, `spec/` — tức **kernel, lib, linker script,
script QEMU/harness, spec**. Không có tệp `.dts`/`.dtsi` nào trong repo (QEMU `virt` sinh DTB động;
không có DTB viết tay để quét — ghi rõ ở §4).

## 2. Bảng kết quả — phân loại từng chỗ

| # | chỗ | giá trị | là vùng nào | xử lý |
|---|---|---|---|---|
| 1 | `v2/kernel/vifast-plat-riscv64/src/pci.rs:37` `ECAM_BASE` | `0x3000_0000` | **PCI_ECAM** | **ĐÃ DẪN XUẤT** từ `resources::PCI_ECAM.base` |
| 2 | `v2/kernel/vifast-plat-riscv64/src/pci.rs:41` `PCI_MMIO_WINDOW_BASE` | `0x4000_0000` | **PCI_MMIO32** | **ĐÃ DẪN XUẤT** từ `resources::PCI_MMIO32.base` |
| 3 | `v2/kernel/vifast-plat-riscv64/src/pci.rs:43` `PCI_MMIO_WINDOW_BYTES` | `1 << 30` | **PCI_MMIO32** (cỡ) | **ĐÃ DẪN XUẤT** từ `resources::PCI_MMIO32.size` |
| 4 | `v2/kernel/vifast-plat-riscv64/src/pci.rs:76` `RESERVED_RANGES[0]` | `(UART0_BASE, 0x1000)` | **UART0** (cỡ) | **ĐÃ DẪN XUẤT** từ `resources::UART0.base`/`.size` |
| 5 | `v2/kernel/vifast-plat-common/tests/ipi.rs:31-32` | `0x0200_0000` | **CLINT** | **ĐÃ DẪN XUẤT** từ `resources::CLINT.base` (ca kiểm đổi theo bảng) |
| 6 | `v2/kernel/vifast-plat-riscv64/src/user.rs:81` `USER_BASE` | `0x4000_0000` | **KHÔNG PHẢI** PCI_MMIO32 | **KHÔNG SỬA** — xem §3 |
| 7 | `v2/kernel/vifast-kbin-riscv64/init/init.ld:10` `. = 0x40000000` | `0x4000_0000` | **KHÔNG PHẢI** PCI_MMIO32 | **KHÔNG SỬA** — VA user, xem §3 |
| 8 | `v2/kernel/vifast-kbin-riscv64/init/inv_user.S:35,46,58,116` | `0x40000000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA user |
| 9 | `v2/kernel/vifast-kbin-riscv64/src/ktest_obj.rs:31` `MAP_VA` | `0x4000_0000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA test |
| 10 | `v2/kernel/vifast-kbin-riscv64/src/ktest_user.rs:778,779,781,1055,1069,1083` | `0x4000_0000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA user |
| 11 | `v2/kernel/vifast-kbin-riscv64/src/ktest_capa.rs:340` | `0x4000_0000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA user |
| 12 | `v2/kernel/vifast-kbin-riscv64/init/pager_common.rs:11,21,23` | `0x4000_0000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA user |
| 13 | `v2/kernel/vifast-plat-common/tests/user.rs:29` `BASE` | `0x1000_0000` | **KHÔNG PHẢI** UART0 | **KHÔNG SỬA** — xem §3 |
| 14 | `v2/kernel/vifast-plat-common/tests/user.rs:149` | `0x4000_0000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA user |
| 15 | `v2/kernel/vifast-plat-common/tests/host.rs:282,301,310` `stval` | `0x0010_0000` | **KHÔNG PHẢI** TEST_PAGE | **KHÔNG SỬA** — xem §3 |
| 16 | `v2/kernel/vifast-plat-riscv64/src/timer.rs:8` (chú thích) | `0x0200_0000` | CLINT | **KHÔNG SỬA** — chú thích, không phải hằng |
| 17 | `v2/kernel/vifast-plat-riscv64/src/mmu.rs:503` (chú thích) | `0x4000_0000` | PCI_MMIO32 | **KHÔNG SỬA** — chú thích |
| 18 | `v2/kernel/vifast-plat-riscv64/src/pci.rs:8,16` (chú thích) | `0x3000_0000`, `0x4000_0000` | PCI_ECAM/MMIO32 | **KHÔNG SỬA** — chú thích |
| 19 | `v2/kernel/vifast-kbin-riscv64/src/ktest.rs:600` (chú thích) | `0x0c00_0000` | PLIC | **KHÔNG SỬA** — chú thích |
| 20 | `v2/kernel/vifast-plat-riscv64/src/obj.rs:161,195` (chú thích) | `0x4000_0000` | **KHÔNG PHẢI** | **KHÔNG SỬA** — VA user |

**Kết luận quét**: chỉ **năm** chỗ là hằng số **thật** của sáu vùng (dòng 1–5); cả năm nay **dẫn
xuất** từ `REGIONS`. Mọi chỗ còn lại hoặc là **chú thích**, hoặc là **địa chỉ ảo của cửa sổ user**
trùng **giá trị** với `PCI_MMIO32` nhưng **khác khái niệm**.

## 3. Vì sao KHÔNG dẫn xuất những chỗ "trùng số"

`USER_BASE = 0x4000_0000` và `PCI_MMIO32.base = 0x4000_0000` **trùng giá trị nhưng không cùng
khái niệm**: một bên là **địa chỉ ảo** của cửa sổ user (Sv39 root index 1), một bên là **địa chỉ
vật lý** của cửa sổ MMIO PCIe. Dẫn xuất chỗ này từ chỗ kia sẽ tạo **phụ thuộc giả**: đổi cửa sổ MMIO
PCIe (hợp lệ) sẽ **âm thầm** dời cửa sổ user, và ngược lại. Đó đúng là lớp lỗi "hai nguồn" mà §A5b
đang chống, chỉ khác chiều.

Cùng lý do cho `tests/user.rs:29` (`BASE` là địa chỉ ảo dùng trong ca kiểm user) và
`tests/host.rs` (`stval` là **giá trị ngẫu nhiên** của một lỗi trang, không phải trang test).

## 4. DTB và linker script

- **DTB**: repo **không có** tệp `.dts`/`.dtsi`/`.dtb` nào. QEMU `virt` sinh DTB động và OpenSBI
  chuyển cho kernel; kernel đọc `hart` từ đó. Không có địa chỉ viết tay để quét ⇒ **không áp dụng**.
- **Linker script**: `v2/kernel/vifast-kbin-riscv64/linker.ld` chỉ đặt `. = 0x80200000` (RAM, không
  thuộc sáu vùng) và `ALIGN(2M)` cho vùng hart. `init/init.ld` đặt `. = 0x40000000` — **VA user**,
  xem §3. Không có địa chỉ MMIO nào trong linker script.
- **Script QEMU**: `v2/test/boot/*.sh` không chứa địa chỉ của sáu vùng (chỉ `-machine virt` và
  đường dẫn ảnh). `ktest.sh:1415` chỉ **nhắc** `0x4000_0000` trong chú thích.

## 5. Phép kiểm **khớp bảng** thêm vào (Agent 0 cho phép "hoặc thêm phép kiểm khớp REGIONS")

Trong `pci.rs`, ba `const _: () = assert!(…)` **biên dịch lúc build**:

```rust
const _: () = assert!(ECAM_MAP_BYTES <= vifast_plat_common::resources::PCI_ECAM.size);
const _: () = assert!(PCI_MMIO_WINDOW_BYTES == vifast_plat_common::resources::PCI_MMIO32.size);
const _: () = assert!(PCI_MMIO_WINDOW_BASE == vifast_plat_common::resources::PCI_MMIO32.base);
```

Vì sao cần **cả** dẫn xuất **và** phép kiểm: dẫn xuất chặn "hai nguồn", nhưng không chặn "driver map
nhiều hơn vùng đã khai". Ba dòng trên chặn đúng cái thứ hai — và chặn **lúc biên dịch**, không phải
lúc chạy.

## 6. Phát hiện **ngoài phạm vi** (ghi lại, KHÔNG sửa)

Khi chạy L0 của workspace kernel, một test **đỏ sẵn từ trước**:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test smp
---- bo_cuc_per_hart_khong_chong_va_can_trang stdout ----
thread '…' panicked at vifast-plat-common/tests/smp.rs:23:5:
assertion `left == right` failed
  left: 86016
 right: 118784
test result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

- `86016` = `HART_GUARD_BYTES + HART_STACK_BYTES + HART_TRAP_STACK_BYTES` (thiếu **syscall stack**).
- `118784` = `HART_STRIDE`, đã gồm `HART_SYSCALL_STACK_BYTES` (thêm ở KT-66).
- Tức **test và chú thích đầu `smp.rs` cũ** so với `HART_STRIDE` hiện hành; `smp.rs` **không** bị
  sửa trong task này (`git status` không có tệp ấy) ⇒ **đỏ sẵn từ trước**.
- **Không cổng nào chạy nó**: `v2/Cargo.toml` `exclude = ["kernel"]`, nên `gate --fast` (L0 trên
  `v2/Cargo.toml`) **không** chạm `vifast-plat-common`. Đây là lớp lỗi "test đỏ mà không ai thấy".
- **Không sửa**: sửa kỳ vọng của test là việc Agent 0 phải quyết (B1: Agent 1 được *từ chối* đổi kỳ
  vọng, không được tự đổi). Đã ghi vào [`v2/agent/BLOCKED-CHO-A0.md`](../BLOCKED-CHO-A0.md).
