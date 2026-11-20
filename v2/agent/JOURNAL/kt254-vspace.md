# KT-254 — K4' bước 3, **B3.5**: `VSpace` thật cho aarch64

**Ngày**: 2026-11-21 · **Sub-agent** `a64b` · nhánh `task/kt253-a64b` · base `main` @ `60f082b9`
**Hàng QUEUE**: `KT-254` (D2-200) · **Phụ thuộc**: KT-253 (commit `9497d036`)
**Phạm vi**: **chỉ B3.5**. Không TLBI shootdown đa hart (B3.6), không U-mode (B3.7).
**Không tuyên bố K4' đạt.**

---

## 1. Đỏ trước (D2-05)

Ca kiểm viết **trước** khi `vspace`/`fault` tồn tại — log [`kt254-red.log`](kt254-red.log):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vspace --test fault
error[E0432]: unresolved import `vifast_plat_aarch64::fault`
error[E0432]: unresolved import `vifast_plat_aarch64::vspace`
error: could not compile `vifast-plat-aarch64` (test "fault") due to 1 previous error
error: could not compile `vifast-plat-aarch64` (test "vspace") due to 1 previous error
```

**Sau khi sửa** — 18 test host mới (10 `vspace` + 7 `fault` + 1 `CPACR`; log
[`kt254-vspace.log`](kt254-vspace.log) §1):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --test vspace --test fault
running 10 tests   (vspace)
test create_has_two_roots_and_no_ttbr ... ok
test map_kernel_high_alias_walks ... ok
test map_obj_rejects_bad_inputs ... ok
test map_obj_walk_returns_pa ... ok
test map_physmap_window_walks ... ok
test ttbr_values_are_level1_and_ttbr_encoded ... ok
test unmap_does_not_affect_other_va ... ok
test unmap_then_walk_not_mapped ... ok
test walk_steps_uses_low_root ... ok
test two_vspaces_are_independent ... ok
running 7 tests    (fault)
test access_codes_match_abi ... ok
test data_abort_read ... ok
test data_abort_write ... ok
test ec_extraction_ignores_iss ... ok
test instruction_abort_execute ... ok
test lower_el_aborts_are_classified ... ok
test non_abort_is_none ... ok
```

Toàn crate: **112 ca host** (trước KT-254: 94) — 7 `fault` + 24 `host` + 10 `mmu_boot` +
13 `pl011` + 16 `split_mmu` + 20 `sysreg` + 4 `tlb_barrier` + 8 `vector` + 10 `vspace`.

## 2. Đã làm

| Tệp | Nội dung |
|---|---|
| `v2/kernel/vifast-plat-aarch64/src/vspace.rs` | **mới** — `VSpaceId`, `Frame` (bất biến căn 4 KiB), `VSpace` = `SplitMmu` + ASID: `create`/`map_obj`/`map_obj_with`/`unmap`/`walk`/`walk_steps`/`map_kernel_high`/`map_physmap`/`ttbr0`/`ttbr1`/`ttbr0_value`/`ttbr1_value`/`activate`; `impl Mmu` |
| `v2/kernel/vifast-plat-aarch64/src/fault.rs` | **mới** — giải mã `ESR_EL1`/`FAR_EL1` thành `Abort { access, va, ec }`; `Access` mã **khớp** `FaultAccess` ABI (0/1/2) |
| `v2/kernel/vifast-plat-aarch64/src/sysreg.rs` | `CPACR_EL1_FP_SIMD` (FPEN=0b11, ZEN=0b11) — **bắt buộc** (xem §4) |
| `v2/kernel/vifast-plat-aarch64/src/bootmem.rs` | `shared_boot_memory_at(base)` — PA arena tường minh (sau khi nhảy nửa cao, `addr_of!` là alias) |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | khai báo + re-export `fault`, `vspace` |
| `v2/kernel/vifast-plat-aarch64/tests/vspace.rs` | **mới** — 10 ca host |
| `v2/kernel/vifast-plat-aarch64/tests/fault.rs` | **mới** — 7 ca host |
| `v2/kernel/vifast-plat-aarch64/tests/sysreg.rs` | +1 ca `cpacr_enables_fp_simd` |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | `_start` bật `CPACR_EL1`; `vec_handler` phân loại abort ⇒ `FaultKind::Vm` + resume khi là fault **được chờ**; `high_entry` dựng `VSpace`, map Frame 4 KiB, `walk`, `activate`, `unmap` + TLBI, đọc lại ⇒ fault; TAP 8/9 |
| `v2/kernel/vifast-kbin-aarch64/Cargo.toml` | +`vifast-plat-common` (chỉ **ảnh** — lấy `FaultKind` ABI, không định nghĩa lại) |
| `v2/test/boot/run-aarch64.sh` | `EXPECT_TESTS` 7 → **9** |

### Bốn quyết định kỹ thuật (hai cái là **phát hiện thật** khi đo)

**(a) `VSpace` giữ `SplitMmu` (KT-253), không giữ `PageTable` trần.** Nhờ vậy `map_obj` định
tuyến theo VA (nửa thấp user / nửa cao kernel) mà **không** lặp một dòng walk/map nào; `activate`
lấy **hai** bảng mức 1 từ wrapper và mã hoá bằng `Ttbr` (`BADDR` + `ASID`).

**(b) `FaultKind` ABI dựng ở **ảnh**, không ở crate arch.** `FaultKind` nằm ở
`vifast-plat-common::fault` (D2-56a) — crate đó kéo `vifast-core`/`vifast-alloc` và `PteFacts`
của nó đọc **bit PTE riscv** (nợ M17). Crate arch chỉ **giải mã** `ESR`/`FAR` (arch-specific,
host-testable); ảnh dựng `FaultKind::Vm { vspace, va, access, reason }` bằng
`FaultAccess::from_code(abort.access.code())` + `vm_reason(access, false, None)`. **Không** có
`FaultKind` thứ hai nào được định nghĩa.

**(c) PHÁT HIỆN 1 — `CPACR_EL1.FPEN = 0` ⇒ `Undefined Instruction` (`EC = 0x07`).** Lần chạy đầu
tiên của `VSpace` treo ở `PageTable::unmap`: trình biên dịch phát **NEON** (`movi v0.2d, #0`,
`stp q0, q0`) để zero buffer 176 B trên stack, mà FP/SIMD bị trap ở EL1. QEMU `-d int` chỉ đúng
`ELR = 0xffffffc08000065c` = `movi v0.2d`. Sửa: `_start` ghi `CPACR_EL1 = 0x0033_0000`
(FPEN=0b11, ZEN=0b11) **trước mọi mã Rust**; hằng `CPACR_EL1_FP_SIMD` có test host tuyệt đối.

**(d) PHÁT HIỆN 2 — ảnh liên kết ở PA ⇒ `core::fmt` vtable là địa chỉ **nửa thấp**.** Sau
`activate` (TTBR0 = bảng VSpace), `writeln!` gọi `&dyn Display` qua vtable ⇒ `blr` tới địa chỉ
**liên kết** (nửa thấp) ⇒ prefetch abort `FAR = 0x4008_0ab0` (đo được bằng `-d int`). Sửa:
VSpace map **identity ảnh kernel** ở bảng nửa thấp với `user = false` (EL0 **không** đọc được —
đúng cách ly). Đây là lý do kỹ thuật **bắt buộc**, không phải tuỳ chọn; liên kết toàn ảnh ở VMA
nửa cao (LMA thấp) sẽ bỏ được nó — việc của bước riêng.

## 3. Đo được (nguyên văn)

Log đầy đủ: [`kt254-vspace.log`](kt254-vspace.log). Serial của ảnh (phần KT-254):

```text
# vspace: map va=0x60000000 pa=0x4008d000 walk=0x4008d000 match=1 pages_high=64 pages_physmap=64 pages_image=64 pages_mmio=33
# vspace: activate begin
# vspace: activate ok
# vspace: activate ttbr0=0x400a4000 ttbr1=0x4009f000 asid=1
# vspace: active read=0x5653504143450001 magic=0x5653504143450001 match=1
ok 8 - vspace: map Frame 4 KiB + walk + activate + read
# vspace: unmap pa=0x4008d000 walk=Err(NotMapped) match=1
# fault: kind=Vm va=0x60000000 access=Read reason=NotMapped esr=0x0000000096000006 far=0x60000000 elr=0xffffffc080002158 match=1
ok 9 - vspace: read after unmap faults with FaultKind::Vm
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
```

`qg_boot rc=0`, `marker_count=1`, không `PANIC`. Runner **7/7** (`TAP 9/9`).

| Phép đo | Kết quả | Ý nghĩa |
|---|---|---|
| **map + walk** | `va=0x6000_0000 pa=0x4008_d000 walk=0x4008_d000 match=1` | map **một Frame 4 KiB** vào VSpace, `walk` trả **đúng PA** |
| **activate** | `ttbr0=0x400a4000 ttbr1=0x4009f000 asid=1` | `TTBR0`/`TTBR1` = bảng **mức 1** của VSpace, ASID 1 |
| **đọc qua VA sau activate** | `read=0x5653504143450001 match=1` | `TTBR0` **thật sự** đổi sang bảng VSpace (VA `0x6000_0000` ngoài RAM, không identity) |
| **unmap + walk** | `pa=0x4008d000 walk=Err(NotMapped) match=1` | `unmap` trả đúng PA, `walk` sau đó **lỗi** |
| **fault sau unmap** | `kind=Vm va=0x60000000 access=Read reason=NotMapped esr=0x96000006 far=0x60000000` | `ESR` thật: `EC=0x25` (Data Abort same EL), `WnR=0` (đọc), `DFSC=0x06` (translation fault) ⇒ **đúng `FaultKind::Vm`** |
| **frame đã dùng** | VSpace: 64+64+64+33 trang map, ~14 frame bảng | arena 32 frame: 14 (kernel) + 14 (VSpace) = 28 ≤ 32 |

## 4. Còn thiếu so với B3.5 / rủi ro

1. **Nợ M17 — object layer `vifast-plat-common/src/obj/vspace.rs` vẫn gắn Sv39** (`walk_shape`,
   `PTE_LEVELS`, bit PTE riscv). **Không** sửa trong task này (đúng yêu cầu). Hệ quả: `VSpace`
   aarch64 **chưa** có cap/Untyped/W^X toàn cục/`protect` của object layer — nó là tầng **dữ
   liệu** bảng trang. Tổng quát hoá object layer qua `Mmu` là việc của M17.
2. **Chưa TLBI shootdown đa hart** (B3.6): `VSpace::unmap` **không** đụng TLB (ghi rõ trong
   doc); ảnh tự gọi `tlb::vae1` + `dsb ish` + `isb` cho hart hiện tại. Đa hart là B3.6.
3. **Ảnh vẫn liên kết ở PA** ⇒ VSpace phải identity-map ảnh kernel (phát hiện 2). Liên kết VMA
   nửa cao + LMA thấp sẽ bỏ được mapping này và làm "kernel chạy nửa cao" đầy đủ hơn.
4. **Chưa U-mode** (B3.7 — chờ `spec/abi.toml [calling.aarch64]`, Q2 vẫn hoãn): `FaultKind::Vm`
   hiện chỉ sinh từ **EL1** (`EC=0x25`); nhánh `EC=0x24` (từ EL0) đã có trong classifier + test
   host nhưng chưa đo trên phần cứng.
5. **Arena 32 frame**: đo dùng **28** ⇒ còn **4** frame. VSpace thứ hai sẽ `PANIC` (fail-loud).
   Nếu B3.6/B3.7 cần thêm bảng, phải tăng arena — nhưng trần I7 (`.data+.bss ≤ 256 KiB`) chỉ còn
   ~106 KiB headroom ⇒ tăng arena lên 48 frame (192 KiB) vẫn ĐẠT; ghi lại để bước sau quyết.
6. **`Frame` chưa có chủ sở hữu/cap**: `map_obj` nhận PA trần (đúng phạm vi B3.5 — chưa có
   object layer aarch64). Không được coi là "cấp phát an toàn".

## 5. Bất biến đã giữ

| # | Bất biến | Cách kiểm |
|---|---|---|
| I1 | Không `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`/`vifast-pte` | `v2 check` xanh; `git diff` ba crate đó **rỗng** |
| I2 | `unsafe` chỉ trong `vifast-plat-aarch64::asm` | `#![deny(unsafe_code)]`; `vspace`/`fault` **không** `unsafe` |
| I3 | Đường riscv64 không đổi | không chạm tệp riscv64; `v2 check` xanh |
| I4 | `PageTableFormat`/`Mmu` **không** đổi chữ ký (D2-199) | `git diff v2/lib/vifast-pte` **rỗng** |
| I7 | `.data + .bss ≤ 256 KiB` | `.data 0x20` + `.bss 0x26000` = **155.680 B** < 262.144 B |
| I9 | Mỗi bước có đột biến làm đỏ | KT-253 §4 (3 đột biến); KT-254 dùng lại đường fault thật của TAP 9 |
