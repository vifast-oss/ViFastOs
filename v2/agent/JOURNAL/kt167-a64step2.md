# KT-167 — K4' bước 2: `vifast-plat-aarch64` thanh ghi hệ thống + asm nguyên thủy

**Ngày**: 2026-11-19 · **Agent 2 (L10)** · nhánh `night2/L10-a64step2` · HEAD trước task: `25306b3f`
**Hàng QUEUE**: `KT-167` · **Nợ đóng**: N3 của KT-160 (thiếu `TTBR*`/`MAIR`/`TCR`/`TLBI`/asm)
**Phạm vi**: encoding thanh ghi hệ thống + asm nguyên thủy; **không** boot, **không** linker
script, **không** QEMU, **không** tuyên bố K4' đạt.

## 0. Việc được giao (nguyên văn, rút gọn)

> **KT-167 — K4' bước 2: `vifast-plat-aarch64` thanh ghi hệ thống + asm nguyên thủy.** Bước 2
> của K4' là **thanh ghi hệ thống + asm nguyên thủy** (chưa boot, chưa linker script).
> 1. `src/sysreg.rs` — encoding thuần dữ liệu, host-testable: `TCR_EL1` (T0SZ, TG0, T1SZ, TG1,
>    IPS, EPD0/1, SH0/1, ORGN0/1, IRGN0/1, AS, TBI0/1), `MAIR_EL1` (8 slot), `SCTLR_EL1` (M, C,
>    I, SA, WXN, UXN…), `TTBR0/1_EL1` (ASID + BADDR), `VBAR_EL1`, `ESR_EL1` (EC, ISS), `FAR_EL1`,
>    `ELR_EL1`, `SPSR_EL1`, `DAIF`; builder `Tcr::new`/`Mair::new`; test khẳng định **giá trị
>    tuyệt đối** viết tay từ ARM DDI 0487.
> 2. `src/tlb.rs` — mã thao tác `TLBI` (`vmalle1`, `vae1`, `aside1`, `vmalle1is`, `vae1is`) dạng
>    hằng encoding host-testable + hàm asm.
> 3. `src/barrier.rs` — `dsb ish`, `dmb ish`, `isb` (asm).
> 4. asm sau `#[cfg(target_arch = "aarch64")]` + stub host; `forbid(unsafe_code)` ở phần dữ
>    liệu, tách `mod asm` nếu cần `unsafe` (D2-17).
> 5. Đột biến bắt buộc: sai shift `TCR_EL1` ⇒ ĐỎ; sai thứ tự/giá trị `MAIR` slot ⇒ ĐỎ; sai
>    opcode `TLBI vae1` ⇒ ĐỎ.
> 6. Giữ xanh host test + build `--target aarch64-unknown-none`.

## 1. Kết luận trước (đọc nhanh)

- **ĐẠT** `src/sysreg.rs`: encoding **thuần dữ liệu** cho 10 thanh ghi + builder const
  (`Tcr`, `Mair`, `Sctlr`, `Ttbr`, `Vbar`, `Esr`, `Spsr`, `Daif`, `FarEl1`, `ElrEl1`); không
  `unsafe`, không `cfg`, không asm.
- **ĐẠT** `src/tlb.rs`: 6 mã `TLBI` (`vmalle1`, `vmalle1is`, `vae1`, `vae1is`, `aside1`,
  `aside1is`) dạng hằng encoding + 6 hàm asm mỏng.
- **ĐẠT** `src/barrier.rs`: 13 encoding (`dsb`/`dmb` × 6 domain + `isb`) + 5 hàm asm.
- **ĐẠT** `src/asm.rs`: `msr`/`mrs` cho 10 thanh ghi + `TLBI`/barrier thật — **mô-đun duy nhất**
  có `unsafe`; stub host **panic** (không no-op).
- **ĐẠT host**: `cargo test -p vifast-plat-aarch64` — **43 passed, 0 failed** (24 test cũ của
  KT-160 + **15 test mới** `tests/sysreg.rs` + **4 test mới** `tests/tlb_barrier.rs`).
- **ĐẠT aarch64**: `cargo build -p vifast-plat-aarch64 --target aarch64-unknown-none` —
  `Finished` (§3.2).
- **ĐẠT đột biến 3/3**: sai shift `TCR_EL1` ⇒ 2 test ĐỎ; đảo thứ tự slot `MAIR` ⇒ 2 test ĐỎ;
  sai opcode `TLBI vae1` ⇒ 1 test ĐỎ (§3.3–§3.5). Khôi phục ⇒ xanh lại (§3.6).
- **ĐẠT cổng**: `./v2/bin/v2 check` — `v2 check: PASS`, `EXIT=0` (§3.7).
- **ĐẠT đối chiếu chéo**: object `aarch64-unknown-none` chứa **đúng** 11 mã đã test host
  (`llvm-objdump`, §3.8) — hằng encoding và instruction phát ra là **một** nguồn.
- **KHÔNG tuyên bố K4' đạt**: chưa boot, chưa linker script, chưa nối `TTBR*`/`TCR`/`MAIR` vào
  đường boot, chưa có vector table/`eret`, chưa dựng `VSpace` thật (§5).

## 2. Tệp mới/sửa và API

| Tệp | Nội dung |
|---|---|
| `src/sysreg.rs` (mới, 1122 dòng) | `TCR_EL1` (17 trường + `Granule`/`TcrCache`/`Ips` + `Tcr` builder + `TCR_EL1_KERNEL`), `MAIR_EL1` (8 slot + `Mair` + `MAIR_EL1_KERNEL`), `SCTLR_EL1` (39 bit + `Sctlr` builder), `TTBR0/1_EL1` (`Ttbr`), `VBAR_EL1` (`Vbar`), `ESR_EL1` (`Esr` + `ExceptionClass`), `FAR_EL1` (`FarEl1`), `ELR_EL1` (`ElrEl1`), `SPSR_EL1` (`Spsr` + `SpsrMode`), `DAIF` (`Daif`) |
| `src/tlb.rs` (mới, 90 dòng) | `TLBI_SYS_BASE`, `tlbi_encoding`, 6 hằng opcode, `tlbi_va_arg`, 6 hàm asm |
| `src/barrier.rs` (mới, 80 dòng) | 13 hằng encoding, 5 hàm asm (`dsb_sy`, `dsb_ish`, `dmb_sy`, `dmb_ish`, `isb`) |
| `src/asm.rs` (mới, 462 dòng) | `#![allow(unsafe_code)]`; bản bare-metal aarch64: 6 `TLBI` + 5 barrier + 9 `msr` + 11 `mrs`; bản host: stub panic |
| `src/lib.rs` (sửa) | `#![forbid(unsafe_code)]` → `#![deny(unsafe_code)]`; thêm 4 module; re-export; tài liệu bước 2 |
| `tests/sysreg.rs` (mới, 548 dòng) | 15 test, kỳ vọng **tuyệt đối viết tay** |
| `tests/tlb_barrier.rs` (mới, 96 dòng) | 4 test (3 encoding + 1 stub host fail-loud) |

**Vì sao `forbid` → `deny`**: `forbid` **không thể** bị `allow` ở module con, mà `asm!` buộc
phải nằm trong `unsafe`. Gốc crate `deny`, **chỉ** `asm.rs` `#![allow(unsafe_code)]` — mọi
module dữ liệu vẫn bị `deny` chặn; `unsafe` vẫn nằm trong danh sách đóng D2-17
(`kernel/vifast-plat-*`). Không nới luật nào khác.

## 3. Bằng chứng (nguyên văn dòng log)

### 3.1 Host test — `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64`

Log đầy đủ: `v2/agent/JOURNAL/kt167-logs/kt167-host-test.log` (`EXIT=0`).

```text
running 24 tests
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

15 test mới của `tests/sysreg.rs`: `tcr_kernel_value`, `tcr_field_positions`,
`tcr_tg_encodings`, `tcr_ips_encodings`, `tcr_sz_masked_to_six_bits`, `mair_slots`,
`mair_kernel_value`, `sctlr_bit_positions`, `sctlr_builder`, `ttbr_fields`, `vbar_fields`,
`esr_fields`, `spsr_fields`, `daif_fields`, `far_elr_roundtrip`.
4 test mới của `tests/tlb_barrier.rs`: `tlbi_encodings`, `tlbi_va_arg_values`,
`barrier_encodings`, `host_stub_is_fail_loud`.

### 3.2 Build aarch64 — `cargo build -p vifast-plat-aarch64 --target aarch64-unknown-none`

Log: `v2/agent/JOURNAL/kt167-logs/kt167-aarch64-build.log` (`EXIT=0`).

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
```

### 3.3 Đột biến 1 — sai shift `TCR_EL1` (`TCR_IRGN0_SHIFT` 8 → 9)

Log: `v2/agent/JOURNAL/kt167-logs/kt167-mut1-tcr-shift.log` (`EXIT=101`).

```text
test tcr_field_positions ... FAILED
test tcr_kernel_value ... FAILED
test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 3.4 Đột biến 2 — đảo thứ tự slot `MAIR` (`8*i` → `8*(7-i)`)

Log: `v2/agent/JOURNAL/kt167-logs/kt167-mut2-mair-slot.log` (`EXIT=101`).

```text
test mair_slots ... FAILED
test mair_kernel_value ... FAILED
test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 3.5 Đột biến 3 — sai opcode `TLBI vae1` (`op2` 1 → 0, tức opcode `vmalle1`)

Log: `v2/agent/JOURNAL/kt167-logs/kt167-mut3-tlbi-opcode.log` (`EXIT=101`).

```text
test tlbi_encodings ... FAILED
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 3.6 Khôi phục — xanh lại

```text
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 3.7 Cổng — `./v2/bin/v2 check`

Log: `v2/agent/JOURNAL/kt167-logs/kt167-v2-check.log` (`EXIT=0`).

```text
--- fmt: OK
--- clippy: OK
--- luật cấu trúc: OK
--- test có răng (KT-163): OK
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

### 3.8 Đối chiếu chéo instruction phát ra (object `aarch64-unknown-none`)

Log: `v2/agent/JOURNAL/kt167-logs/kt167-aarch64-objdump.log`. Object do
`cargo rustc -p vifast-plat-aarch64 --target aarch64-unknown-none -- --emit=obj` sinh, rồi
`llvm-objdump -d`:

```text
       0: d5033b9f     	dsb	ish
       0: d5033bbf     	dmb	ish
       0: d5033f9f     	dsb	sy
       0: d5033fbf     	dmb	sy
       0: d5033fdf     	isb
       0: d508831f     	tlbi	vmalle1is
       8: d5088320     	tlbi	vae1is, x0
       8: d5088340     	tlbi	aside1is, x0
       0: d508871f     	tlbi	vmalle1
       8: d5088720     	tlbi	vae1, x0
       8: d5088740     	tlbi	aside1, x0
```

Đây là **cùng** 11 giá trị mà `tests/tlb_barrier.rs` khẳng định tuyệt đối ⇒ không có đường thứ
hai để hằng encoding và instruction lệch nhau (asm dùng `.inst <hằng>`).

## 4. Phát hiện đặc tả và quyết định kỹ thuật

1. **`SCTLR_EL1` KHÔNG có `UXN`/`PXN`.** Đề bài liệt kê "SCTLR_EL1 (M, C, I, SA, WXN, UXN…)"
   nhưng theo Arm Architecture Registers (DDI 0601, bản 2024-12) `SCTLR_EL1` không có hai
   trường này — `UXN`/`PXN` là bit **descriptor** (bit 54/53 của entry bảng trang), đã cài ở
   `format.rs` (`DESC_UXN`/`DESC_PXN`) từ KT-160. Tôi **không bịa** trường không tồn tại; thay
   vào đó cài đủ 39 trường có thật của `SCTLR_EL1` (kể cả `WXN`, `SPAN`, `E0E`, `EE`, `UCI`,
   `DZE`, `UCT`, `nTWI`, `nTWE`, `IESB`, `EIS`, `EnDB`, `EnDA`, `nTLSMD`, `LSMAOE`, `EnIB`,
   `EnIA`, `BT0`, `BT1`, `ITFSB`, `ATA`, `DSSBS`, `EnASR`, `EnAS0`, `EnALS`, `EPAN`, `EnTP2`).
   Ghi chú này nằm ngay đầu `src/sysreg.rs` và trong test `sctlr_bit_positions`.
2. **Máy host là Apple Silicon (`aarch64-apple-darwin`)** ⇒ `#[cfg(target_arch = "aarch64")]`
   một mình sẽ biên dịch asm **đặc quyền** (`TLBI`, `msr`) vào test host và chạy ở EL0 ⇒ fault.
   Điều kiện đúng là **aarch64 bare-metal**: `#[cfg(all(target_arch = "aarch64", target_os =
   "none"))]`; mọi target khác (kể cả aarch64 macOS) dùng stub panic. `aarch64-unknown-none`
   thoả `target_os = "none"` nên build thật vẫn dùng asm. Test `host_stub_is_fail_loud` chứng
   minh stub **panic**, không no-op.
3. **Mã `TLBI` đối chiếu bằng hợp dịch thật, không bằng trí nhớ.** Bảng thực tế:
   `CRm = 7` cho bản non-IS, `CRm = 3` cho bản IS; `op2` phân biệt thao tác
   (`vmalle1` 0, `vae1` 1, `aside1` 2). Ví dụ `tlbi vae1, x0` = `0xD508_8720` (không phải
   `0xD508_8120` như một số tài liệu cũ dễ gây nhầm). Đã kiểm bằng
   `clang --target=aarch64-none-elf` + `llvm-objdump` trước khi viết hằng.
4. **`TCR_EL1_KERNEL`** = `0x0000_0015_B519_3519`: T0SZ = T1SZ = 25 (39-bit VA, D2-147),
   TG0 = TG1 = 4 KiB, IPS = 48 bit, SH = inner, IRGN/ORGN = WBWA, ASID 16 bit, cả TTBR0/TTBR1
   bật. Bước 3 nối `TTBR1_EL1`; bước 2 chỉ chốt encoding.
5. **`MAIR_EL1_KERNEL`** = `0x0000_0000_0000_FF00` (slot 0 Device-nGnRnE, slot 1 Normal WB) —
   khớp `format::MAIR_EL1`; test `mair_kernel_value` so **cả hai** định nghĩa độc lập.

## 5. Còn thiếu (bước 3 của K4')

- **Chưa boot**: không `_start`, không linker script, không vector table, không `eret`.
- **Chưa nối thanh ghi vào đường boot**: chưa gọi `write_tcr_el1`/`write_mair_el1`/
  `write_ttbr0_el1`/`write_sctlr_el1` theo trình tự chuẩn (TCR → MAIR → TTBR → SCTLR → ISB).
- **Chưa dựng `VSpace` thật**: `PageTable` (bước 1) và `Ttbr` (bước 2) chưa được nối; chưa có
  `TLBI` sau `unmap` (hiện `PageTable::unmap` ghi chú "bước 2 mới có TLBI" — bước 3 sẽ gọi
  `tlb::vae1`).
- **Chưa kiểm trên QEMU/máy thật**: task này **không cần** QEMU; mọi bằng chứng là host test +
  build target + objdump.
- **K4' chưa đạt** — không tuyên bố khác.

## 6. Lệnh tái lập

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test  --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target aarch64-unknown-none
./v2/bin/v2 check
```

Đột biến (áp rồi khôi phục): sửa `TCR_IRGN0_SHIFT` 8→9; đảo `Mair::new` thành
`8*(MAIR_SLOTS-1-i)`; đổi `TLBI_VAE1` thành `tlbi_encoding(7, 0, 0)` — mỗi lần
`cargo test -p vifast-plat-aarch64` phải `FAILED` (log ở §3.3–§3.5).
