# KT-175 — review đối kháng KT-167 (K4' bước 2: thanh ghi hệ thống + asm aarch64)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer L17** (độc lập, không phải lead/tác giả) · nhánh
`night2/L17-a64review` · worktree `/private/tmp/night2-L17-a64review` · HEAD `2dc03fed` (từ `main`).
**Nguồn đọc trước**: [`kt167-a64step2.md`](kt167-a64step2.md), `src/{sysreg,tlb,barrier,asm}.rs`,
`tests/{sysreg,tlb_barrier}.rs`, `kt167-logs/`.
**Ngân sách**: hạn cứng 08:00 JST. Không merge, không push, không sửa `main`, không `gate --fast`,
không tạo worktree mới, không QEMU. Mọi đột biến đã `git checkout --`; cây sạch trước khi commit.

## 0. Kết luận trước

**KT-167: ĐẠT CÓ ĐIỀU KIỆN.**

| Mục thử phá | Phán quyết | Căn cứ (một dòng) |
|---|---|---|
| **1. Tautology** TCR/MAIR | **Lõi ĐẠT — HỞ 2 lớp** | 17/17 đột biến shift `TCR_EL1` + 8/8 đột biến `MAIR_EL1` (slot 0/1/2/3 + đảo thứ tự + shift/mask/modulo) đều **ĐỎ**; nhưng **14/14 hằng "chết"** (`TCR_*_MASK`, `TTBR_BADDR_SHIFT`, `VBAR_BASE_SHIFT`, `SPSR_M_SHIFT`) đột biến vẫn **43/43 xanh**, và **3/3 hoán vị setter `SCTLR`** vẫn xanh (`sctlr_builder` so hằng của chính crate). |
| **2. `SCTLR_EL1` UXN/PXN** | **ĐÚNG** (điểm chính) / **SAI** (claim "đủ") | `SCTLR_EL1` **không** có UXN/PXN — đúng, hai bit đó là descriptor 54/53. Nhưng "cài **đủ 39 trường** có thật" là **sai**: thiếu ≥8 trường có thật (nAA, EnRCTX, CMOW, TCF0, TCF, ATA0, TWEDEn, TWEDEL). |
| **3. `TLBI` encoding** | **ĐẠT** | 6/6 mã khớp **độc lập** `clang --target=aarch64-none-elf` + `llvm-objdump`; 13/13 barrier cũng khớp. |
| **4. asm host stub** | **ĐẠT một phần** | Trên `aarch64-apple-darwin` stub **panic thật**; `host_stub_is_fail_loud` chứng minh (đột biến cfg ⇒ **ĐỎ** tại `tlb_barrier.rs:91`). Nhưng chỉ **3/33** stub được kiểm. |
| **5. Build `--target aarch64-unknown-none`** | **ĐẠT (tái lập)** — log đã commit **yếu** | Tái lập: `Compiling vifast-plat-aarch64` + rlib **625 242 B** + objdump có asm thật. Log `kt167-aarch64-build.log` chỉ có `Finished … in 0.11s`, **không** `Compiling`, **không** rlib ⇒ một mình nó không chứng minh không no-op. |
| **6. `forbid`→`deny` + `allow`** | **ĐẠT, hơi rộng** | `deny` chặn `unsafe` ngoài `asm.rs` (đột biến ⇒ lỗi biên dịch); `forbid` + `allow` ⇒ `E0453` (đúng lý do tác giả). `asm.rs` là nơi **duy nhất** có `unsafe`. Nhưng `#![allow(unsafe_code)]` phủ **cả tệp** (kể cả stub host), không chỉ `mod imp`. |

**Phát hiện nặng nhất**: **14 hằng công khai "chết"** — 11 `TCR_*_MASK` (`src/sysreg.rs:42,50,54,58,62,66,78,82,86,90,94`)
+ `TTBR_BADDR_SHIFT` (`:689`), `VBAR_BASE_SHIFT` (`:742`), `SPSR_M_SHIFT` (`:1014`) — được
`pub use` ở `src/lib.rs:77-82` nhưng **không code nào dùng, không test nào chạm**. Đột biến
`TCR_IRGN0_MASK` `0b11→0b01`, `TCR_IPS_MASK` `0b111→0b011`, … vẫn **43/43 xanh**. Bước 3 sẽ dùng
mask để bóc/ghép trường; một mask sai sẽ **im lặng** sửa nhầm bit — đúng lớp lỗi KT-160
(`DESC_ATTR_SHIFT 2→3` vẫn 22/22 xanh) mà task này sinh ra để chặn. Kèm theo:
`tests/sysreg.rs:360-396` so builder với **OR hằng của chính crate** ⇒ hoán vị hai setter vẫn xanh.

## 1. Tautology — đột biến từng trường `TCR_EL1` và từng slot `MAIR_EL1`

Log đầy đủ: [`kt175-logs/kt175-mutations.log`](kt175-logs/kt175-mutations.log) — **64 phép đột biến**
(65 dòng gồm baseline; **21 XANH** = lỗ hổng, **43 ĐỎ**), mỗi dòng áp → `cargo test -p
vifast-plat-aarch64 --test sysreg --test tlb_barrier` → `git checkout --`.
Baseline: **43 passed** (24 `host.rs` + 15 `sysreg.rs` + 4 `tlb_barrier.rs`).

### 1.1 `TCR_EL1` — 17/17 trường bị bắt (ĐẠT)

Mỗi đột biến là **shift của chính trường** (không đụng test):

| Trường | Đột biến | Kết quả | Test đỏ |
|---|---|---|---|
| T0SZ | `TCR_T0SZ_SHIFT` 0→1 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value`, `tcr_sz_masked_to_six_bits` |
| TG0 | `TCR_TG0_SHIFT` 14→15 | **ĐỎ** | `tcr_kernel_value`, `tcr_tg_encodings`, `tcr_field_positions` |
| IPS | `TCR_IPS_SHIFT` 32→33 | **ĐỎ** | `tcr_field_positions`, `tcr_ips_encodings`, `tcr_kernel_value` |
| EPD0 | `TCR_EPD0_SHIFT` 7→6 | **ĐỎ** | `tcr_field_positions` |
| SH0 | `TCR_SH0_SHIFT` 12→13 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value` |
| ORGN0 | `TCR_ORGN0_SHIFT` 10→11 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value` |
| IRGN0 | `TCR_IRGN0_SHIFT` 8→9 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value` |
| AS | `TCR_AS_SHIFT` 36→35 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value` |
| TBI0 | `TCR_TBI0_SHIFT` 37→38 | **ĐỎ** | `tcr_field_positions` |
| T1SZ | `TCR_T1SZ_SHIFT` 16→17 | **ĐỎ** | `tcr_kernel_value`, `tcr_field_positions` |
| TG1 | `TCR_TG1_SHIFT` 30→31 | **ĐỎ** | `tcr_field_positions`, `tcr_ips_encodings`, `tcr_kernel_value`, `tcr_tg_encodings` |
| EPD1 | `TCR_EPD1_SHIFT` 23→22 | **ĐỎ** | `tcr_field_positions` |
| A1 | `TCR_A1_SHIFT` 22→21 | **ĐỎ** | `tcr_field_positions` |
| SH1 | `TCR_SH1_SHIFT` 28→29 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value` |
| ORGN1 | `TCR_ORGN1_SHIFT` 26→27 | **ĐỎ** | `tcr_field_positions`, `tcr_kernel_value` |
| IRGN1 | `TCR_IRGN1_SHIFT` 24→25 | **ĐỎ** | `tcr_kernel_value`, `tcr_field_positions` |
| TBI1 | `TCR_TBI1_SHIFT` 38→39 | **ĐỎ** | `tcr_field_positions` |

Vì sao bắt được: `tests/sysreg.rs:78` khẳng định **giá trị tuyệt đối** `0x0000_0074_F9D5_AEAA`
(tôi tính tay lại: khớp), `:97-113` khẳng định **vị trí bit tuyệt đối** của từng trường, `:36`
khẳng định `TCR_EL1_KERNEL.bits() == 0x0000_0015_B519_3519` (tôi tính tay lại: khớp). Đột biến
`TCR_EL1_KERNEL` (T0SZ 25→24, IPS 48→44, SH0 Inner→Outer, AS 1→0) cũng **4/4 ĐỎ** (`tcr_kernel_value`).

### 1.2 `MAIR_EL1` — 8/8 đột biến bị bắt (ĐẠT)

| Đột biến | Kết quả | Test đỏ |
|---|---|---|
| `Mair::new` slot `i` → `7-i` (đảo thứ tự) | **ĐỎ** | `mair_kernel_value`, `mair_slots` |
| `MAIR_EL1_KERNEL` slot 0 `0x00→0x04` | **ĐỎ** | `mair_kernel_value` |
| `MAIR_EL1_KERNEL` slot 1 `0xFF→0xBB` | **ĐỎ** | `mair_kernel_value` |
| `MAIR_EL1_KERNEL` slot 2 `0x00→0x11` | **ĐỎ** | `mair_kernel_value` |
| `MAIR_EL1_KERNEL` slot 3 `0x00→0x22` | **ĐỎ** | `mair_kernel_value` |
| `MAIR_SLOT_SHIFT` 8→4 | **ĐỎ** | `mair_slots`, `mair_kernel_value` |
| `MAIR_SLOT_MASK` 0xFF→0x7F | **ĐỎ** | `mair_kernel_value`, `mair_slots` |
| `Mair::slot` bỏ `% MAIR_SLOTS` (slot(8) panic) | **ĐỎ** | `mair_slots` |

Slot 4..7 phủ bởi vòng `for i in 2..8 { assert_eq!(slot(i), 0) }` (`tests/sysreg.rs:237-239`);
slot 0/1 phủ bởi giá trị tuyệt đối `0x0000_0000_0000_FF00` (`:234`). `mair_kernel_value` còn
so chéo `format::MAIR_EL1` (`:241-244`) — nhưng **không** phải tautology thuần vì `:234` đã
khẳng định tuyệt đối trước đó.

### 1.3 LỖ HỔNG 1 — 14 hằng công khai "chết", 14/14 đột biến vẫn XANH

| Hằng | Dòng | Đột biến | Kết quả |
|---|---|---|---|
| `TCR_T0SZ_MASK` | `src/sysreg.rs:42` | `0x3F→0x1F` | **XANH** |
| `TCR_T1SZ_MASK` | `:66` | `0x3F→0x1F` | **XANH** |
| `TCR_IRGN0_MASK` | `:50` | `0b11→0b01` | **XANH** |
| `TCR_ORGN0_MASK` | `:54` | `0b11→0b01` | **XANH** |
| `TCR_SH0_MASK` | `:58` | `0b11→0b10` | **XANH** |
| `TCR_TG0_MASK` | `:62` | `0b11→0b01` | **XANH** |
| `TCR_IRGN1_MASK` | `:78` | `0b11→0b01` | **XANH** |
| `TCR_ORGN1_MASK` | `:82` | `0b11→0b01` | **XANH** |
| `TCR_SH1_MASK` | `:86` | `0b11→0b10` | **XANH** |
| `TCR_TG1_MASK` | `:90` | `0b11→0b01` | **XANH** |
| `TCR_IPS_MASK` | `:94` | `0b111→0b011` | **XANH** |
| `TTBR_BADDR_SHIFT` | `:689` | `1→2` | **XANH** |
| `VBAR_BASE_SHIFT` | `:742` | `11→12` | **XANH** |
| `SPSR_M_SHIFT` | `:1014` | `0→1` | **XANH** |

Nguyên nhân: builder/getter dùng **literal** (`& 0x3F`, `0b11 << shift`, `!0xFFF`, `!0x7FF`,
`0xF`) chứ không dùng các hằng `*_MASK`/`*_SHIFT` này. `grep -rn --include=*.rs` toàn repo: 11 `TCR_*_MASK` chỉ
xuất hiện ở định nghĩa + `pub use` (`src/lib.rs:77-82`); 3 hằng còn lại cũng vậy. Đây là **API
công khai** (đã export), nên không thể coi là "chi tiết nội bộ": bước 3 dùng
`TCR_IRGN0_MASK` để bóc trường là chuyện tự nhiên, và một mask sai sẽ **im lặng**.

### 1.4 LỖ HỔNG 2 — `sctlr_builder` so hằng của chính crate (3/3 hoán vị XANH)

`tests/sysreg.rs:360-396` khẳng định `misc.bits() == SCTLR_SPAN | SCTLR_E0E | … | SCTLR_SA0`
— **OR của chính các hằng crate**. Vì phép OR giao hoán, hoán vị hai setter không đổi kết quả:

| Đột biến | Kết quả |
|---|---|
| `with_span => SCTLR_SPAN` ↔ `with_el0_big_endian => SCTLR_E0E` | **XANH** (15/15) |
| `with_ata => SCTLR_ATA` ↔ `with_dssbs => SCTLR_DSSBS` | **XANH** (15/15) |
| `with_ntwi => SCTLR_NTWI` ↔ `with_ntwe => SCTLR_NTWE` | **XANH** (15/15) |

Phần `on`/`off` của cùng test (`:304-323`) thì **tuyệt đối** (`0x8_100D`, `0x100C`) nên 5 setter
`M/C/I/SA/WXN` được bảo vệ; 34 setter còn lại **không**. `sctlr_bit_positions` (`:255-295`) khẳng
định 39 hằng tuyệt đối nên **vị trí bit** đúng — lỗ hổng chỉ ở **dây nối setter ↔ hằng**.

### 1.5 Những gì KHÔNG tautology (để công bằng)

`tcr_kernel_value` (`:36`), `tcr_field_positions` (`:78`, `:97-113`), `tcr_tg_encodings`
(`:120-161`), `tcr_ips_encodings` (`:167-186`), `mair_slots` (`:205`), `sctlr_bit_positions`
(`:255-295`), `ttbr_fields` (`:405`), `vbar_fields` (`:431`), `esr_fields` (`:448`), `spsr_fields`
(`:484`), `daif_fields` (`:516-520`), `tlbi_encodings` (`tlb_barrier.rs:29-34`), `barrier_encodings`
(`:66-78`) — **đều là literal viết tay**. Tôi tính tay lại `TCR_EL1_KERNEL = 0x15_B519_3519` và
`TCR_EL1` của `tcr_field_positions = 0x74_F9D5_AEAA`: **khớp**.

## 2. `SCTLR_EL1` — UXN/PXN: tác giả ĐÚNG, nhưng claim "đủ 39 trường" SAI

### 2.1 UXN/PXN — ĐÚNG

`SCTLR_EL1` **không** có `UXN`/`PXN`. Hai bit đó là bit **descriptor** của entry bảng trang:
`DESC_PXN = 1 << 53`, `DESC_UXN = 1 << 54` (`src/format.rs:99-101`). Đối chiếu độc lập:
Linux `arch/arm64/tools/sysreg` (bản master, tải từ `git.kernel.org`) liệt kê `SCTLR_EL1` **không**
có UXN/PXN; FreeBSD `sys/arm64/include/armreg.h:2727-2790` cũng vậy. **Tác giả không bịa trường
không tồn tại** — đúng.

### 2.2 Claim "cài đủ 39 trường có thật" — SAI

`src/sysreg.rs:17-25` và `kt167-a64step2.md:174-176` viết "cài **đủ 39 trường** có thật của
`SCTLR_EL1`". Linux `sysreg` liệt kê **54** trường; tác giả thiếu **15**, trong đó **≥8 trường
có thật từ ARMv8.x** (chắc chắn có trong DDI 0601 bản 2024-12):

| Trường thiếu | Bit | Có từ |
|---|---|---|
| `nAA` | 6 | ARMv8.0 |
| `EnRCTX` | 10 | ARMv8.4 |
| `CMOW` | 32 | ARMv8.7 |
| `TCF0` | 39:38 | ARMv8.5 (MTE) |
| `TCF` | 41:40 | ARMv8.5 (MTE) |
| `ATA0` | 42 | ARMv8.5 (MTE) |
| `TWEDEn` | 45 | ARMv8.6 |
| `TWEDEL` | 49:46 | ARMv8.6 |

(Linux master còn liệt kê `TIDCP` 63, `SPINTMASK` 62, `NMI` 61, `TCSO` 59, `TCSO0` 58, `EnFPM` 34,
`MSCEn` 33 — các trường ARMv9.x, có thể chưa có trong bản 2024-12; không tính vào "≥8".)

**Ảnh hưởng**: **không** có hại cho K4' bước 2 (không dùng MTE/TWED/CMOW/RCTX), và 39 trường đã
cài đều **đúng vị trí** (đối chiếu từng bit với Linux + FreeBSD: khớp 39/39). Nhưng claim "đủ" là
**sai sự thật**, và **không test nào khẳng định tính đầy đủ** — `sctlr_bit_positions` chỉ kiểm 39
hằng đã có. Sửa: hạ claim thành "39 trường đã cài; còn thiếu …" hoặc bổ sung trường.

## 3. `TLBI` encoding — 6/6 ĐÚNG (đối chiếu độc lập)

Log: [`kt175-logs/kt175-clang-tlbi.log`](kt175-logs/kt175-clang-tlbi.log). Hợp dịch **độc lập**
(`clang --target=aarch64-none-elf -c` + `xcrun llvm-objdump -d`, Apple LLVM 21.0.0):

```text
       0: d508871f     	tlbi	vmalle1
       4: d508831f     	tlbi	vmalle1is
       8: d5088720     	tlbi	vae1, x0
       c: d5088320     	tlbi	vae1is, x0
      10: d5088740     	tlbi	aside1, x0
      14: d5088340     	tlbi	aside1is, x0
```

Khớp **từng mã** với `src/tlb.rs:43-53` và `tests/tlb_barrier.rs:29-34`. 13/13 barrier cũng khớp
(`dsb/dmb` × 6 domain + `isb`). Đột biến `TLBI_VAE1` op2 1→0, `TLBI_ASIDE1` op2 2→1,
`TLBI_VMALLE1IS` CRm 3→7, `TLBI_SYS_BASE`, mask CRm, `tlbi_va_arg` — **6/6 ĐỎ**.

Ghi chú nhỏ: `tlbi_va_arg` (`src/tlb.rs:58-60`) **không** cắt còn 44 bit; test
`tlbi_va_arg_values` (`tlb_barrier.rs:57`) khẳng định giá trị **không cắt** `0x000F_FFFF_FFFF_FFFF`.
Với VA 39-bit canonical (D2-147) thì vô hại (27 bit), nhưng là cạnh sắc nếu sau này dùng VA > 56 bit.

## 4. asm host stub — panic thật, nhưng chỉ 3/33 stub được kiểm

### 4.1 Trên `aarch64-apple-darwin` stub **có** panic — ĐẠT

`src/asm.rs:38` chọn asm khi `all(target_arch = "aarch64", target_os = "none")`; `:329` chọn stub
khi `not(...)`. Host là `aarch64-apple-darwin` ⇒ `target_os = "macos"` ⇒ **stub**. Test
`host_stub_is_fail_loud` (`tests/tlb_barrier.rs:89-95`) `catch_unwind` 3 hàm và khẳng định
`is_err()` — **pass** trên máy này. Đột biến cfg bỏ `target_os = "none"` ⇒ asm thật biên dịch vào
host ⇒ test **ĐỎ** tại `tlb_barrier.rs:91` (`assert!(barrier.is_err())`): chứng minh test **phân
biệt được** stub với asm, không phải xanh hình thức.

### 4.2 LỖ HỔNG 3 — 30/33 stub không được kiểm

`host_stub_is_fail_loud` chỉ gọi `barrier::isb`, `tlb::vmalle1`, `asm::read_tcr_el1`. Đột biến
3 stub khác thành no-op:

| Đột biến | Kết quả |
|---|---|
| `read_mair_el1` → `0` | **XANH** (43/43) |
| `write_daif` → `let _ = _value;` | **XANH** (43/43) |
| `tlbi_aside1is` → `let _ = _arg;` | **XANH** (43/43) |

Đây đúng là kịch bản test sinh ra để chặn ("một no-op sẽ khiến test xanh trong khi chẳng có
barrier/TLBI nào chạy" — `tlb_barrier.rs:86-88`), nhưng chỉ 3/33 hàm được phủ. Sửa: lặp qua **cả
33** hàm (hoặc sinh test từ danh sách tên).

## 5. Build `--target aarch64-unknown-none` — thật, nhưng log đã commit không chứng minh

### 5.1 Log đã commit **yếu**

`v2/agent/JOURNAL/kt167-logs/kt167-aarch64-build.log` chỉ có **một dòng**:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
```

Không `Compiling vifast-plat-aarch64`, không liệt kê rlib. `0.11s` + không `Compiling` là dấu hiệu
**no-op** (crate đã build sẵn). Một mình log này **không** chứng minh crate được biên dịch cho
target bare-metal.

### 5.2 Tái lập — ĐẠT

Log: [`kt175-logs/kt175-build.log`](kt175-logs/kt175-build.log). Xoá sạch
`v2/kernel/target/aarch64-unknown-none` rồi build:

```text
   Compiling vifast-pte v0.1.0 (.../v2/lib/vifast-pte)
   Compiling vifast-plat-aarch64 v0.1.0 (.../v2/kernel/vifast-plat-aarch64)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.79s
EXIT=0
-rw-r--r--  625242  .../aarch64-unknown-none/debug/libvifast_plat_aarch64.rlib
```

Object `--emit=obj` rồi `llvm-objdump` ([`kt175-logs/kt175-objdump.log`](kt175-logs/kt175-objdump.log)):
**6 `tlbi` + 5 barrier + 9 `msr` + 11 `mrs`** — tức nhánh `asm::imp` **thật**, không phải stub.
Vậy build là thật; chỉ **bằng chứng đã commit** là thiếu.

## 6. `forbid(unsafe_code)` → `deny` + `allow` cho `asm`

- `src/lib.rs:44` `#![deny(unsafe_code)]`; `src/asm.rs:36` `#![allow(unsafe_code)]`.
- `grep` toàn crate: **chỉ** `asm.rs` có từ khóa `unsafe` (31 khối `unsafe { … }` + 1 `allow`);
  `barrier.rs`, `format.rs`, `mmu.rs`, `sysreg.rs`, `tlb.rs` chỉ **nhắc** `unsafe` trong doc comment.
  ⇒ `asm.rs` là nơi duy nhất — **ĐẠT**.
- Đột biến `unsafe fn sneaky() {}` vào `sysreg.rs` ⇒ **lỗi biên dịch** (`error: declaration of an
  `unsafe` function`, trỏ `#![deny(unsafe_code)]`) ⇒ `deny` **có tác dụng**.
- Đột biến `deny` → `forbid` ⇒ **`error[E0453]: allow(unsafe_code) incompatible with previous
  forbid`** ⇒ lý do tác giả đổi `forbid`→`deny` là **đúng**, không phải nới tuỳ tiện.
- **Hơi rộng**: `#![allow(unsafe_code)]` ở đầu tệp phủ **cả** `mod imp` (asm) **và** `mod imp`
  (stub host). Đột biến thêm `unsafe fn` vào `asm.rs` (ngoài `mod imp`) ⇒ **biên dịch được**
  (XANH). Cách hẹp hơn: `#[allow(unsafe_code)] mod imp { … }` chỉ trên nhánh asm. Trong phạm vi
  D2-17 (crate `kernel/vifast-plat-*` nằm trong danh sách đóng) nên **không vi phạm**, nhưng câu
  "không nới luật nào khác" trong journal là **chưa chính xác**: `deny` yếu hơn `forbid` ở gốc
  crate — một `#[allow(unsafe_code)]` tương lai ở bất kỳ đâu sẽ được chấp nhận. Lưu ý: checker
  R5 (`v2/bin/v2:199-203`) **miễn toàn bộ** `kernel/vifast-plat-*` khỏi quét `unsafe`, nên hàng
  rào thực tế là danh sách đóng D2-17, không phải lint.

## 7. Điều kiện để KT-167 ĐẠT hẳn (C1..C5)

| # | Điều kiện | `file:dòng` | Vì sao |
|---|---|---|---|
| **C1** | Test **hoặc** xoá 14 hằng chết: 11 `TCR_*_MASK` + `TTBR_BADDR_SHIFT` + `VBAR_BASE_SHIFT` + `SPSR_M_SHIFT` | `src/sysreg.rs:42,50,54,58,62,66,78,82,86,90,94,689,742,1014`; `src/lib.rs:77-82` | Đột biến 14/14 vẫn 43/43 xanh; là API công khai, bước 3 sẽ dùng |
| **C2** | Bỏ tautology `sctlr_builder`: khẳng định **từng** setter (`Sctlr::new().with_x(true).bits() == SCTLR_X`) thay vì OR cả cụm | `tests/sysreg.rs:360-396` | 3/3 hoán vị setter vẫn xanh |
| **C3** | Test **cả 33** stub host (không chỉ 3) | `tests/tlb_barrier.rs:89-95`; `src/asm.rs:329-460` | 3/3 stub no-op vẫn xanh |
| **C4** | Sửa claim "đủ 39 trường" → "39 trường đã cài; thiếu nAA/EnRCTX/CMOW/TCF0/TCF/ATA0/TWEDEn/TWEDEL…" (hoặc bổ sung) | `src/sysreg.rs:17-25`; `kt167-a64step2.md:174-176` | Claim sai sự thật; không test nào kiểm tính đầy đủ |
| **C5** | Chụp lại log build có `Compiling vifast-plat-aarch64` + rlib (hoặc `cargo build -v`) | `kt167-logs/kt167-aarch64-build.log` | Log hiện tại chỉ "Finished in 0.11s" — không chứng minh không no-op |

C1–C3 là **lỗ hổng bằng chứng thật** (đột biến sống); C4 là **sai tài liệu**; C5 là **bằng chứng
yếu**. Không điều kiện nào đòi đổi thiết kế hay nới ngưỡng.

## 8. Những gì đã kiểm và ĐẠT (để cân bằng)

- **Encoding đúng**: `TCR_EL1` 17 trường (T0SZ/T1SZ/TG0/TG1/IPS/EPD0/EPD1/SH0/SH1/ORGN0/ORGN1/
  IRGN0/IRGN1/A1/AS/TBI0/TBI1) — đối chiếu DDI 0601 + Linux + FreeBSD: **khớp**; `MAIR_EL1` 8 slot
  (Device-nGnRnE 0x00, Device-nGnRE 0x04, Normal NC 0x44, WT 0xBB, WB 0xFF) — **khớp**;
  `SCTLR_EL1` 39 bit — **khớp 39/39**; `TTBR` (ASID[63:48], BADDR[47:12], CnP[0]), `VBAR`
  (VBA[63:11]), `ESR` (EC[31:26], IL[25], ISS[24:0]), `SPSR` (NZCV[31:28], DAIF[9:6], M[3:0]),
  `DAIF` (D9/A8/I7/F6) — **khớp**.
- **`TCR_EL1_KERNEL` = `0x0000_0015_B519_3519`** — tôi tính tay lại từng trường: **khớp**
  (T0SZ=T1SZ=25, TG0=0b00, TG1=0b10, IPS=0b101, SH=0b11, IRGN/ORGN=0b01, AS=1).
- **`MAIR_EL1_KERNEL` = `0x0000_0000_0000_FF00`** — khớp `format::MAIR_EL1`.
- **6/6 `TLBI` + 13/13 barrier** — khớp hợp dịch độc lập.
- **Build bare-metal thật** — `Compiling` + rlib 625 242 B + objdump có asm.
- **`deny(unsafe_code)` có tác dụng**; `asm.rs` là nơi duy nhất có `unsafe`.
- **Không sửa mã sản phẩm**: mọi đột biến đã `git checkout --`; `git status --porcelain` sạch
  trước khi commit (chỉ thêm `v2/agent/JOURNAL/kt175-*`).

## 9. Lệnh tái lập

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/night2-L17-a64review
# baseline 43 passed
cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
# build that (xoa artifact truoc)
rm -rf v2/kernel/target/aarch64-unknown-none
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target aarch64-unknown-none
# doi chieu doc lap 6 ma TLBI
clang --target=aarch64-none-elf -c /tmp/l17-tlbi.s -o /tmp/l17-tlbi.o
xcrun llvm-objdump -d /tmp/l17-tlbi.o
# chien dich dot bien (tu ap lai tung dong trong log)
cat v2/agent/JOURNAL/kt175-logs/kt175-mutations.log
```

## 10. Chưa review / ngoài phạm vi

- **Không** chạy `./v2/bin/v2 check` hay `gate --fast` (task cấm `gate --fast`; `check` không cần
  cho phán quyết encoding). Log `kt167-v2-check.log` của tác giả **không** được kiểm lại.
- **Không** chạy QEMU (task không cần).
- **Không** review lại `format.rs`/`mmu.rs` (phạm vi KT-160/164, đã có KT-168 review).
- **Không** kiểm `host.rs` 24 test (phạm vi bước 1) — chỉ chạy như baseline.
- **Không** kiểm tính đúng của `ExceptionClass` ngoài 10 mã đã cài (kernel chỉ cần 10 mã đó).
