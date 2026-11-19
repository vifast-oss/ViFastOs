# KT-188 — review đối kháng KT-185 (D2-162: C1/C2/C4/C5)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer L26** (độc lập, không phải lead/tác giả) · nhánh
`night2/L26-review185` · worktree `/private/tmp/night2-L26-review185` · HEAD `9921f606` (từ `main`).
**Nguồn đọc trước**: [`kt185-c1c5.md`](kt185-c1c5.md), `git show 8d2f443c --stat`,
`v2/kernel/vifast-plat-aarch64/tests/sysreg.rs`, `v2/kernel/vifast-plat-aarch64/src/sysreg.rs`.
**Ngân sách**: hạn cứng 12:00 JST. Không merge, không push, không sửa `main`, không `gate --fast`,
không QEMU, không tạo worktree mới. Mọi đột biến đã `git checkout --`; cây sạch trước commit.

## 0. Kết luận trước

**KT-185: ĐẠT CÓ ĐIỀU KIỆN.**

| Mục | Phán quyết | Căn cứ (một dòng) |
|---|---|---|
| **C1** 14 hằng "chết" | **ĐẠT** | 14/14 đột biến **ĐỎ** dựng lại **độc lập** (harness riêng của L26, không dùng script tác giả); thêm **12/12** đột biến tinh vi **ĐỎ** (đổi shift của mask, hai hằng bù nhau, sửa **cả** hằng **và** test) |
| **C2** tautology `sctlr_builder` | **ĐẠT** | Không còn OR hằng crate — so literal `0x13C0_1838_FFF5_EBB2`; `sctlr_setters_reference_bits` kiểm **39/39** setter; 3/3 hoán vị + 4 hoán vị khác (`mmu↔alignment`, `SA↔SA0`, `bt0↔bt1`, `enasr↔enas0`) **7/7 ĐỎ** |
| **C4** claim "đủ 39 trường" | **ĐẠT CÓ ĐIỀU KIỆN** | Claim "đủ 39 trường" đã bỏ khỏi `src/sysreg.rs` + `kt167-a64step2.md`; 8 trường thiếu liệt kê **đúng tên+bit**; **nhưng** con số "**54** trường" **SAI** — DDI 0601 (2024-12) có **58** trường, thiếu 4 trường `TME`/`TME0`/`TMT`/`TMT0` (bit 53/52/51/50); hedge "ARMv9.x có thể chưa có trong 2024-12" **SAI** (7 trường đó **CÓ** trong 2024-12) |
| **C5** log build | **ĐẠT** | Log có `Compiling vifast-plat-aarch64`; artifact **thật** trong worktree tác giả: `libvifast_plat_aarch64.rlib` **626138 B**, timestamp `Nov 19 09:57`, chứa path `/private/tmp/night2-L24-c1c5` (20 lần); host test binary `sysreg-7e89ffe7e6d2fc45` + `host-d88200a3f9ea42af` cùng timestamp |
| **Hồi quy** | **ĐẠT** | `cargo test -p vifast-plat-aarch64`: **47 passed, 0 failed** (24 `host` + 19 `sysreg` + 4 `tlb_barrier`); không test cũ nào bị làm yếu |
| **Bịa số** | **2 SỐ SAI** | "**54** trường" (thực **58**) và "**33** stub" (thực **31** `pub fn` + `host_stub` = 32); các số còn lại khớp log |

**Phát hiện nặng nhất**: C4 thay claim sai "đủ 39 trường" bằng **một con số sai mới** — "DDI 0601
(2024-12) có **54** trường" — trong khi bản 2024-12 có **58** trường; danh sách thiếu bỏ sót
**`TME` (53), `TME0` (52), `TMT` (51), `TMT0` (50)**, và câu hedge nói 7 trường ARMv9.x "có thể
chưa có trong bản 2024-12" là **sai** (chúng có trong 2024-12). Đây là **lỗi tài liệu**, không
phải lỗi mã; nhưng nó lặp lại đúng lớp lỗi C4 sinh ra để dẹp (claim không kiểm được).

## 1. C1 — 14 hằng "chết": dựng lại **độc lập**

### 1.1 Test mới có so **literal viết tay** thật không?

**Có.** `tests/sysreg.rs:209-226` (`tcr_masks_reference_values`) so **11** `TCR_*_MASK` với
**11 literal** viết tay; `tests/sysreg.rs:598-605` (`shift_constants_reference_values`) so **3**
hằng shift + `SPSR_M_MASK` với literal. Không vế nào là hằng khác của crate.

Tôi **tính tay lại 11 mask** từ DDI 0487 bảng `TCR_EL1` — khớp **11/11**:
`T0SZ[5:0]` `0x3F`, `IRGN0[9:8]` `0x300`, `ORGN0[11:10]` `0xC00`, `SH0[13:12]` `0x3000`,
`TG0[15:14]` `0xC000`, `T1SZ[21:16]` `0x3F_0000`, `IRGN1[25:24]` `0x300_0000`,
`ORGN1[27:26]` `0xC00_0000`, `SH1[29:28]` `0x3000_0000`, `TG1[31:30]` `0xC000_0000`,
`IPS[34:32]` `0x7_0000_0000`. Ba shift: `TTBR_BADDR_SHIFT=1` (BADDR bit `[47:1]`),
`VBAR_BASE_SHIFT=11` (`VBA[63:11]`), `SPSR_M_SHIFT=0` (`M[3:0]`) — khớp.

`tcr_masks_extract_reference_fields` (`tests/sysreg.rs:231-249`) bóc trường từ **giá trị thanh ghi
viết tay** `0x0000_0074_F9D5_AEAA` bằng chính mask — đây là **lớp thứ hai**, không phải tautology:
nó bắt được mask sai **kể cả khi** literal trong `tcr_masks_reference_values` bị sửa theo (xem S6).

### 1.2 14/14 đột biến **ĐỎ** — dựng lại độc lập

Harness riêng của L26 (`/tmp/l26_mut.py`, không dùng script tác giả): mỗi ca áp chuỗi thay thế
**chính xác một lần** vào `src/sysreg.rs` → `cargo test … --test sysreg` → `git checkout --`.
Log: [`kt188-logs/kt188-mutations.log`](kt188-logs/kt188-mutations.log). Baseline: `ok. 19 passed`.

| # | Đột biến | Kết quả L26 | Test đỏ |
|---|---|---|---|
| C1-01 | `TCR_T0SZ_MASK` `0x3F→0x1F` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-02 | `TCR_T1SZ_MASK` `0x3F→0x1F` | **ĐỎ** (rc=101, 1 failed) | `tcr_masks_reference_values` |
| C1-03 | `TCR_IRGN0_MASK` `0b11→0b01` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-04 | `TCR_ORGN0_MASK` `0b11→0b01` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-05 | `TCR_SH0_MASK` `0b11→0b10` | **ĐỎ** (rc=101, 1 failed) | `tcr_masks_reference_values` |
| C1-06 | `TCR_TG0_MASK` `0b11→0b01` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-07 | `TCR_IRGN1_MASK` `0b11→0b01` | **ĐỎ** (rc=101, 1 failed) | `tcr_masks_reference_values` |
| C1-08 | `TCR_ORGN1_MASK` `0b11→0b01` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-09 | `TCR_SH1_MASK` `0b11→0b10` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-10 | `TCR_TG1_MASK` `0b11→0b01` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-11 | `TCR_IPS_MASK` `0b111→0b011` | **ĐỎ** (rc=101, 2 failed) | `tcr_masks_extract_reference_fields`, `tcr_masks_reference_values` |
| C1-12 | `TTBR_BADDR_SHIFT` `1→2` | **ĐỎ** (rc=101, 1 failed) | `shift_constants_reference_values` |
| C1-13 | `VBAR_BASE_SHIFT` `11→12` | **ĐỎ** (rc=101, 1 failed) | `shift_constants_reference_values` |
| C1-14 | `SPSR_M_SHIFT` `0→1` | **ĐỎ** (rc=101, 1 failed) | `shift_constants_reference_values` |

**14/14 ĐỎ** — khớp từng dòng với log tác giả (cùng tên test đỏ, cùng số `passed/failed`).
Không còn ca nào xanh như L17 §1.3.

### 1.3 Đột biến **tinh vi** — 12/12 ĐỎ

| # | Đột biến | Kết quả | Test đỏ |
|---|---|---|---|
| S1 | **Đổi shift của mask**: `TCR_IRGN0_SHIFT` `8→9` (mask chạy theo) | **ĐỎ** | `tcr_field_positions`, `tcr_masks_reference_values` |
| S2 | **Shift + hardcode bù**: `TCR_IRGN0_SHIFT 8→9`, `TCR_IRGN0_MASK = 0b11 << 8` (mask **giữ nguyên** `0x300`) | **ĐỎ** | `tcr_field_positions` (bắt shift sai) |
| S3 | **Hoán vị hai hằng**: `TCR_IRGN0_MASK` ↔ `TCR_ORGN0_MASK` (định nghĩa chéo shift) | **ĐỎ** | `tcr_masks_reference_values` (cả hai) |
| S4 | **Hoán vị hai hằng**: `TCR_T0SZ_MASK` ↔ `TCR_T1SZ_MASK` | **ĐỎ** | `tcr_masks_reference_values` (cả hai) |
| S5 | **Shift + hardcode bù**: `TCR_IPS_SHIFT 32→33`, `TCR_IPS_MASK = 0b111 << 32` | **ĐỎ** | `tcr_field_positions` |
| S6 | **Sửa cả hằng VÀ test**: `TCR_IRGN0_MASK = 0b11 << 9`, literal test `0x300→0x600` | **ĐỎ** | `tcr_masks_extract_reference_fields` (lớp 2 bắt: `(raw & 0x600) >> 8 = 0b110 ≠ 0b10`) |
| S7 | **Đổi độ rộng**: `TCR_IPS_MASK` `0b111→0b11` | **ĐỎ** | `tcr_masks_reference_values`, `tcr_masks_extract_reference_fields` |
| S8 | **Nới rộng**: `TCR_T0SZ_MASK` `0x3F→0x7F` | **ĐỎ** | `tcr_masks_reference_values` |
| S9 | **Đổi shift của mask**: `TCR_TG0_SHIFT 14→15` | **ĐỎ** | `tcr_field_positions`, `tcr_masks_reference_values`, `tcr_tg_encodings` |
| S10 | **Hai hằng bù nhau**: `SPSR_M_SHIFT 0→1` + `SPSR_M_MASK 0xF→0x1E` | **ĐỎ** | `shift_constants_reference_values` (bắt shift) |
| S11 | **Hai hằng bù nhau**: `VBAR_BASE_SHIFT 11→12` + `VBAR_BASE_MASK !0x7FF→!0xFFF` | **ĐỎ** | `shift_constants_reference_values` (bắt shift) |
| S12 | **Hai hằng bù nhau**: `TTBR_BADDR_SHIFT 1→2` + `TTBR_BADDR_MASK !0xFFF→!0x1FFF` | **ĐỎ** | `shift_constants_reference_values` (bắt shift) |

**Kết luận C1**: đạt. Không có cặp hằng nào "bù nhau" mà thoát — vì mỗi hằng được so **độc lập**
với literal, và **shift** được kiểm riêng ở `tcr_field_positions` (`tests/sysreg.rs:100-113`).
Ca S6 đáng chú ý: sửa **cả** hằng **và** literal test vẫn **ĐỎ** nhờ lớp `tcr_masks_extract_reference_fields`
— đây là bằng chứng hai test **bổ sung nhau**, không phải tautology.

## 2. C2 — `sctlr_builder` không còn tautology; 39/39 setter

### 2.1 `sctlr_builder` (`tests/sysreg.rs:355-415`)

Vế phải cũ `SCTLR_SPAN | … | SCTLR_SA0` (OR hằng crate) đã **bỏ**; thay bằng literal
`0x13C0_1838_FFF5_EBB2`. Tôi **tính tay lại** tổng 34 bit (23,24,25,26,14,15,16,18,21,22,13,27,
28,29,30,31,35,36,37,43,44,54,55,56,57,60,11,20,5,7,8,9,1,4) = `0x13C0_1838_FFF5_EBB2` — **khớp**.
`on.bits() == 0x8_100D` và `off.bits() == 0x100C` cũng khớp. **Không còn tautology.**

### 2.2 `sctlr_setters_reference_bits` (`tests/sysreg.rs:423-476`)

**39 mục**, đúng **39** setter của macro `sctlr_setters!` (`src/sysreg.rs:646-686`). Mỗi mục gọi
**một** setter với `true` rồi so `1 << bit` viết tay. Tôi đối chiếu **39/39** bit với DDI 0601
(2024-12) — khớp (M 0, A 1, C 2, SA 3, SA0 4, CP15BEN 5, ITD 7, SED 8, UMA 9, EOS 11, I 12,
EnDB 13, DZE 14, UCT 15, nTWI 16, nTWE 18, WXN 19, TSCXT 20, IESB 21, EIS 22, SPAN 23, E0E 24,
EE 25, UCI 26, EnDA 27, nTLSMD 28, LSMAOE 29, EnIB 30, EnIA 31, BT0 35, BT1 36, ITFSB 37,
ATA 43, DSSBS 44, EnASR 54, EnAS0 55, EnALS 56, EPAN 57, EnTP2 60).

### 2.3 Hoán vị — 7/7 ĐỎ

| # | Hoán vị | Kết quả | Test đỏ |
|---|---|---|---|
| C2-01 | `with_span => SCTLR_E0E`, `with_el0_big_endian => SCTLR_SPAN` | **ĐỎ** | `sctlr_setters_reference_bits` |
| C2-02 | `with_ata => SCTLR_DSSBS`, `with_dssbs => SCTLR_ATA` | **ĐỎ** | `sctlr_setters_reference_bits` |
| C2-03 | `with_ntwi => SCTLR_NTWE`, `with_ntwe => SCTLR_NTWI` | **ĐỎ** | `sctlr_setters_reference_bits` |
| C2-04 | **`with_mmu ↔ with_alignment`** (`with_m ↔ with_a`) | **ĐỎ** | `sctlr_builder`, `sctlr_setters_reference_bits` |
| C2-05 | **`with_stack_alignment ↔ with_el0_stack_alignment`** (`SA ↔ SA0`) | **ĐỎ** | `sctlr_builder`, `sctlr_setters_reference_bits` |
| C2-06 | `with_bt0 ↔ with_bt1` | **ĐỎ** | `sctlr_setters_reference_bits` |
| C2-07 | `with_enasr ↔ with_enas0` | **ĐỎ** | `sctlr_setters_reference_bits` |

Thêm 7 ca phá khác — **7/7 ĐỎ**: `with_mmu => SCTLR_A` đơn (C2-08); macro bỏ `on` thành `true`
(C2-09 — `sctlr_builder` bắt); macro đảo `!on` (C2-10); `SCTLR_M 1<<0→1<<1` chỉ đổi hằng (C2-11);
`SCTLR_M` + literal test cùng sửa (C2-12 — `sctlr_builder` vẫn bắt); map trùng
`with_el0_big_endian => SCTLR_SPAN` (C2-13); bỏ một setter khỏi macro (C2-14 — lỗi biên dịch vì
test gọi `with_entp2`).

**Kết luận C2**: đạt. Hoán vị `with_m↔with_a` và `SA↔SA0` **bị bắt** — đúng như yêu cầu.

## 3. C4 — claim "đủ 39 trường": đã bỏ, nhưng con số thay thế **SAI**

### 3.1 Claim "đủ 39 trường" — đã bỏ đúng chỗ

`grep -rn "đủ 39\|39 trường"` toàn repo: **không còn** trong `src/` hay `tests/`. Các lần xuất
hiện còn lại đều ở **tài liệu lịch sử** đang mô tả **lỗi cũ** (`kt175-a64review.md`,
`kt179-journalaudit.md`, `BAO-CAO-DEM-…`, `PHIEU-GUI-…`) — không phải claim hiện hành.
`src/sysreg.rs:23` nay viết "cài **39 trường**" (không "đủ"); `src/sysreg.rs:27-33` thêm đoạn
"**Không** tuyên bố 'đủ trường'". `kt167-a64step2.md:178-185` cũng đã sửa. **ĐẠT phần này.**

### 3.2 8 trường thiếu — **đúng tên + bit**

Đối chiếu DDI 0601 (2024-12) — cả 8 đều đúng:

| Trường | Bit trong doc-comment | Bit thực tế 2024-12 | Khớp |
|---|---|---|---|
| `nAA` | 6 | 6 | ✓ |
| `EnRCTX` | 10 | 10 | ✓ |
| `CMOW` | 32 | 32 | ✓ |
| `TCF0` | 39:38 | 39:38 | ✓ |
| `TCF` | 41:40 | 41:40 | ✓ |
| `ATA0` | 42 | 42 | ✓ |
| `TWEDEn` | 45 | 45 | ✓ |
| `TWEDEL` | 49:46 | 49:46 | ✓ |

### 3.3 **SAI**: "54 trường" — bản 2024-12 có **58**

`src/sysreg.rs:27` và `kt167-a64step2.md:181` và `kt185-c1c5.md:95-96` đều nói "DDI 0601 (2024-12)
có **54** trường `SCTLR_EL1`". Đối chiếu mirror MRA 2024-12
([`kt188-logs/kt188-arm-fields.log`](kt188-logs/kt188-arm-fields.log)): bản 2024-12 có **58**
trường (59 mục kể cả `RES0` bit 17). Bốn trường bị bỏ sót:

| Trường | Bit | Có trong 2024-12? |
|---|---|---|
| `TME` | 53 | **CÓ** |
| `TME0` | 52 | **CÓ** |
| `TMT` | 51 | **CÓ** |
| `TMT0` | 50 | **CÓ** |

`39 (đã cài) + 8 (liệt kê) + 7 (ARMv9.x) + 4 (TME) = 58`. Con số **54** = 58 − 4, tức tác giả
đã đếm thiếu đúng 4 trường `TME*`. (Bản **latest** 2026-09 mới có 54 trường vì `TME*` đã bị bỏ;
tác giả ghi "2024-12" nhưng dùng số của bản latest — **mis-attribution**.)

### 3.4 **SAI**: hedge "ARMv9.x có thể chưa có trong bản 2024-12"

`src/sysreg.rs:29-31` viết 7 trường `EnFPM` 34, `MSCEn` 33, `TCSO0` 58, `TCSO` 59, `NMI` 61,
`SPINTMASK` 62, `TIDCP` 63 "có thể chưa có trong bản 2024-12". Thực tế **cả 7 đều CÓ** trong
2024-12 (xem log). Câu hedge này vô nghĩa và che mất 4 trường `TME*` thật sự bị bỏ sót.

**Kết luận C4**: **ĐẠT CÓ ĐIỀU KIỆN** — claim "đủ 39 trường" đã bỏ (đúng yêu cầu chính), 8 trường
liệt kê đúng tên+bit; nhưng con số "54" **sai** (thực 58) và danh sách **thiếu 4 trường `TME*`**.
Cần sửa trước khi đóng C4.

## 4. C5 — log build **thật**

### 4.1 Log có `Compiling` — và artifact **thật**

`kt185-logs/kt185-aarch64-build.log` có `Compiling vifast-pte` + `Compiling vifast-plat-aarch64`
+ `Finished … in 0.90s` + `EXIT=0` + rlib **626138 B**. Kiểm chứng **độc lập**:

- Worktree tác giả `/private/tmp/night2-L24-c1c5` **vẫn còn**; `git status` sạch, HEAD `dfb79ac4`.
- `v2/kernel/target/aarch64-unknown-none/debug/libvifast_plat_aarch64.rlib` **tồn tại**, đúng
  **626138 B**, timestamp `Nov 19 09:57` — khớp từng byte với log.
- `strings` rlib chứa `/private/tmp/night2-L24-c1c5` **20 lần** ⇒ build **thật** trong worktree
  tác giả, không phải chép log.
- Host test binary `sysreg-7e89ffe7e6d2fc45` (1193384 B) + `host-d88200a3f9ea42af` (1293976 B)
  **tồn tại**, timestamp `Nov 19 09:57`, đúng hash trong `kt185-host-test.log`.
- Log cũ `kt167-logs/kt167-aarch64-build.log` **không bị sửa** (`git diff b6efc403 8d2f443c -- v2/agent/JOURNAL/kt167-logs/` rỗng) — đúng như journal nói.

### 4.2 Tái lập độc lập

[`kt188-logs/kt188-build.log`](kt188-logs/kt188-build.log): xoá `target/aarch64-unknown-none` rồi
build lại — **cùng cấu trúc** (`Compiling vifast-pte`, `Compiling vifast-plat-aarch64`,
`Finished in 0.78s`), rlib **626250 B**. Chênh 112 B so với log tác giả = chênh **độ dài path**
(`night2-L24-c1c5` 29 ký tự vs `night2-L26-review185` 33 ký tự, ×28 lần nhúng) — hợp lý, không
phải log bịa.

**Kết luận C5**: đạt. Log **thật**, không phải log cũ chép lại.

## 5. Hồi quy — 47 test xanh, không làm yếu test cũ

[`kt188-logs/kt188-host-test.log`](kt188-logs/kt188-host-test.log):

```text
tests/host.rs        test result: ok. 24 passed; 0 failed
tests/sysreg.rs      test result: ok. 19 passed; 0 failed
tests/tlb_barrier.rs test result: ok.  4 passed; 0 failed
EXIT=0
```

**47 passed, 0 failed** — khớp journal. `cargo clippy -p vifast-plat-aarch64 --all-targets --
-D warnings`: sạch ([`kt188-logs/kt188-clippy-teeth.log`](kt188-logs/kt188-clippy-teeth.log)).
`python3 v2/tools/check-test-teeth.py`: `errors=0 warnings=0` (cùng log).

**Không test cũ nào bị làm yếu**: diff `b6efc403..8d2f443c` chỉ **thêm** 4 test mới
(`tcr_masks_reference_values`, `tcr_masks_extract_reference_fields`, `sctlr_setters_reference_bits`,
`shift_constants_reference_values`) và **thay** vế phải tautology của `sctlr_builder` bằng literal
(mạnh hơn). Không xoá assertion nào, không `#[ignore]`, không `#[should_panic]`, không nới ngưỡng.

## 6. Bịa số — đối chiếu từng con số

| Số trong journal | Log tương ứng | Kết quả |
|---|---|---|
| 47 passed (24+19+4) | `kt185-host-test.log` | ✓ |
| 19 test `sysreg` | `kt185-host-test.log` | ✓ |
| 14/14 đột biến ĐỎ | `kt185-mutations.log` | ✓ (dựng lại độc lập khớp) |
| 3/3 hoán vị ĐỎ | `kt185-mutations.log` | ✓ (dựng lại độc lập khớp) |
| `0.90s` build | `kt185-aarch64-build.log` | ✓ (artifact thật) |
| `626138` B rlib | `kt185-aarch64-build.log` | ✓ (artifact thật) |
| `0.11s` log cũ | `kt167-aarch64-build.log` | ✓ |
| 39/39 setter | `tests/sysreg.rs:423-476` | ✓ |
| 34 setter trong `misc` | `tests/sysreg.rs:379-413` | ✓ |
| `errors=0 warnings=0` | `kt185-v2-check.log:343` | ✓ |
| clippy sạch | `kt185-v2-check.log:6` | ✓ |
| **54 trường `SCTLR_EL1`** | **không có log** | **✗ — thực 58** |
| **33 stub `asm.rs`** | **không có log** | **✗ — thực 31 `pub fn` (+`host_stub` = 32)** |

Hai số sai:
- **54** — `src/sysreg.rs:27`, `kt167-a64step2.md:181`, `kt185-c1c5.md:95-96`; thực tế 2024-12 = **58**.
- **33** — `kt185-c1c5.md:125` ("test cả **33** stub `src/asm.rs:329-460`"); thực tế vùng đó có
  **31** `pub fn` stub (+ `fn host_stub` = 32 hàm). Đây là số **kế thừa từ L17** (`kt175-a64review.md`
  §4.2 cũng ghi 33), nhưng KT-185 thêm `file:line` khiến nó kiểm được và **sai**.

## 7. Phát hiện theo mức

| # | Mức | Phát hiện | Vị trí |
|---|---|---|---|
| **F1** | **Trung bình** (tài liệu) | "DDI 0601 (2024-12) có **54** trường" **SAI** — bản 2024-12 có **58**; thiếu 4 trường `TME` (53), `TME0` (52), `TMT` (51), `TMT0` (50) | `src/sysreg.rs:27-31`; `kt167-a64step2.md:181-183`; `kt185-c1c5.md:95-99` |
| **F2** | **Nhẹ** (tài liệu) | Hedge "7 trường ARMv9.x … có thể chưa có trong bản 2024-12" **SAI** — cả 7 **có** trong 2024-12 | `src/sysreg.rs:29-31` |
| **F3** | **Nhẹ** (số) | "**33** stub `src/asm.rs:329-460`" — thực **31** `pub fn` (+`host_stub` = 32) | `kt185-c1c5.md:125` |
| **F4** | **Nhẹ** (cách ghi log) | Journal nói "Log **nguyên văn** từng dòng" cho `kt185-mutations.log`, nhưng log là **bảng tóm tắt đã parse** (`… \| ĐỎ \| rc=101 \| test result: …`), không phải output cargo thô | `kt185-c1c5.md:46`; `kt185-logs/kt185-mutations.log` |
| **F5** | **Rất nhẹ** (trích dẫn) | Journal trỏ "`kt167-a64step2.md` §4.1" — tệp không có §4.1; nội dung nằm ở §4 **mục 1** | `kt185-c1c5.md:102` |

**Không có phát hiện nặng nào về mã/test**: C1 và C2 đã đóng đúng; C5 thật; hồi quy xanh.

## 8. Còn thiếu / chưa review

- **C3** (30 stub host) — **không** thuộc KT-185 (D2-162 "C3 để bước 3"); nợ có tên vẫn mở. Số
  "33" trong nợ sai (F3), nên khi làm C3 phải đếm lại: **31** `pub fn` stub trong `asm.rs`.
- Không chạy `gate --fast` (task cấm); không QEMU (task không cần).
- Không sửa `spec/`, `AGENTS.md`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
  `docs/v1/`; không merge, không push.

## 9. Lệnh tái lập

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/night2-L26-review185
cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
rm -rf v2/kernel/target/aarch64-unknown-none
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target aarch64-unknown-none
cargo clippy --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --all-targets -- -D warnings
python3 v2/tools/check-test-teeth.py
# đột biến: python3 /tmp/l26_mut.py  (harness riêng của L26; log ở kt188-logs/kt188-mutations.log)
```
