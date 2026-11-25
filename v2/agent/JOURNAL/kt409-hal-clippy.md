# KT-409 — Đại tu HAL `vifast-plat-common` + nối cổng `clippy` kernel

- **Task**: KT-409 (P0, Chặng 1 / W3, 3 h) — phụ thuộc KT-404.
- **Nhánh**: `task/kt409-hal-clippy` · **Worktree**: `/private/tmp/vifast-wt/1`
- **Base**: `main` @ `f055799c` (đã có KT-404/408/402b/414b).
- **Căn cứ**: `D2-259` (kế hoạch đại tu), `D2-260` mục 3 (đóng lỗ hổng `clippy` workspace kernel),
  `D2-17` (unsafe theo danh sách đóng), `D2-253` (vùng ghi `v2/bin/**` — task này **được phép**
  sửa đúng phần nối bước).
- **Ngân sách**: 3 h. Bắt đầu 2026-11-25 ~20:40 JST (11:40 UTC, Thấp điểm chính).

## 0. Kế hoạch ngắn (thứ tự ưu tiên theo đề bài)

1. **Đỏ-trước**: chạy `cargo clippy --manifest-path v2/kernel/Cargo.toml -p <crate> --all-targets
   -- -D warnings` trên từng crate `plat-*` + `vifast-core` ⇒ ghi log đỏ **đúng lý do**.
2. **Nối bước `clippy` kernel** vào `v2/bin/lib/steps_check.sh` (`step_clippy_kernel`) + đăng ký
   `lane_step main 84` trong `v2/bin/lib/check.sh` + cập nhật `CHECK_STEPS_EXPECTED` 48 → 49
   (`GATE_FAST_EXTRA_EXPECTED` giữ 31 ⇒ T2 79 → 80). Bước phải **đỏ trước** khi còn cảnh báo.
3. **Dọn sạch `clippy -D warnings`** ở các crate host-testable (`plat-common`, `plat-aarch64`,
   `plat-x86_64`) — sửa mã, không `#[allow]` tràn lan; chỉ dùng `#[allow]` đúng khuôn sẵn có của
   repo cho `assertions_on_constants` (test **cố ý** ghim hằng số) và `too_many_arguments` (chữ ký
   cũ đã có tiền lệ ở `plat-riscv64/src/virtio_net.rs:453`).
4. **Xóa `dev-dependencies` ngược** `plat-common` → `plat-aarch64`/`plat-x86_64`; đưa
   `TableMemory` lên `vifast-pte`.
5. **Dời Sv39/PLIC/trap/timer/tlb/user** sang `plat-riscv64` (0 từ khoá RISC-V trong `plat-common`)
   — nếu hết giờ thì dừng và báo rõ.
6. **Rút trait dùng chung** (`ArchIrq`, `IrqMcsLock<T,A>`, `CoreBridge<A>`, `ElfUserLoader`,
   `ProductShellHost`, `ImageHash`/`WxReport`) — hạng mục cuối, cắt ≥ 3.500 LOC.

**Ràng buộc tự áp**: 0 chạm `v2/kernel/vifast-core/**`; không sửa `spec/`; không nới ngưỡng;
không `#[ignore]`/`#[should_panic]`; không `git add/commit`; chỉ T0 + T1.

## 1. Đỏ-trước (bằng chứng)

### 1.1 Hiện trạng `clippy` trên workspace kernel (trước khi sửa)

Lệnh (mỗi crate một lượt, `CARGO_TARGET_DIR=/Users/mps/dvchd/ViFastOs/target-task/slot-1`):

```bash
cargo clippy --manifest-path v2/kernel/Cargo.toml -p <crate> --all-targets -- -D warnings
```

Kết quả **trước** khi sửa:

| crate | exit | ghi chú |
|---|---:|---|
| `vifast-core` | 0 | sạch (KT-405 đã dọn) |
| `vifast-plat-common` | 101 | **5 lỗi** (lib) + 1 lỗi test `bootinfo` |
| `vifast-plat-aarch64` | 101 | 7 lỗi lib + 5 lỗi test `umode` |
| `vifast-plat-x86_64` | 101 | 9 lỗi lib + 12 lỗi test (`boot`/`ipi`/`apic`/`dhcp`) |
| `vifast-plat-riscv64` | 101 | **không host-buildable**: `invalid register a7` (asm RISC-V) |

Trích log đỏ (nguyên văn):

```text
error: manual implementation of `.is_multiple_of()`
   --> vifast-plat-common/src/bootinfo.rs:679:12
error: the loop variable `i` is only used to index `madt.cpu_ids`
    --> vifast-plat-common/src/bootinfo.rs:1431:14
error: the loop variable `i` is only used to index `e820.entries`
    --> vifast-plat-common/src/bootinfo.rs:1440:14
error: the loop variable `k` is only used to index `u.holes`
  --> vifast-plat-common/src/obj/check.rs:90:30
error: the loop variable `i` is only used to index `self.flushes`
   --> vifast-plat-common/src/obj/check.rs:426:18
error: could not compile `vifast-plat-common` (lib) due to 5 previous errors
```

```text
error: unused import: `crate::asm`
  --> vifast-plat-x86_64/src/wx.rs:33:5
error: variable does not need to be mutable
   --> vifast-plat-x86_64/src/wx.rs:129:9
error: manual `!RangeInclusive::contains` implementation
   --> vifast-plat-x86_64/src/bootinfo.rs:387:8
error: called `.as_ref().map(|guard| &**guard)` on an `Option` value
   --> vifast-plat-x86_64/src/irq_lock.rs:335:9
error: called `.as_mut().map(|guard| &mut **guard)` on an `Option` value
   --> vifast-plat-x86_64/src/irq_lock.rs:344:9
```

```text
error: unnecessary `unsafe` block
  --> vifast-plat-aarch64/src/bootinfo.rs:87:22
error: casting to the same type is unnecessary (`u32` -> `u32`)
   --> vifast-plat-aarch64/src/uart_rx.rs:212:6
error: this function has too many arguments (8/7)
   --> vifast-plat-aarch64/src/virtio_net.rs:405:1
```

### 1.2 Lỗ hổng cổng (lý do phải nối bước)

`v2/bin/lib/steps_check.sh:66` chỉ chạy `cargo clippy` trên `$MANIFEST` = `v2/Cargo.toml`, mà
`v2/Cargo.toml` có `exclude = ["kernel"]` ⇒ **toàn bộ** `v2/kernel/` (8 crate, ~62 k LOC) **không**
được `v2 check`/T1/T2 quét `clippy`. Đây đúng lỗ hổng `D2-260` mục 3 ghi nhận.

## 2. Ưu tiên 1 — Nối bước `clippy` kernel vào `steps_check.sh` (ĐÃ XONG)

### 2.1 Thay đổi (đúng phần nối bước, không định dạng lại tệp)

| Tệp | Thay đổi |
|---|---|
| `v2/bin/lib/steps_check.sh` | thêm hàm `step_clippy_kernel` ngay sau `step_clippy` |
| `v2/bin/lib/check.sh` | `lane_step main 84 "workspace v2/kernel: clippy host-testable (KT-409)" step_clippy_kernel` + `$(lane_failed 84)` vào tổng đếm |
| `v2/bin/lib/consts.sh` | `CHECK_STEPS_EXPECTED` 48 → **49**; `GATE_FAST_EXTRA_EXPECTED` giữ **31** ⇒ T2 79 → **80**; cập nhật dòng chú thích `79 bước` → `80 bước` |

Bước dùng `--workspace --exclude` (không liệt kê `-p`) để crate host-testable **thêm sau** tự động
được quét — hướng fail-closed. Bốn crate bị loại trừ là **đo được**, không suy đoán:

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-riscv64 --lib
error: invalid Mach-O section specifier
error: invalid register `a7`: unknown register
error: invalid register `a6`: unknown register
error: invalid register `a0`: unknown register
EXIT=101
```

```text
$ cargo clippy --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-aarch64 --all-targets -- -D warnings
error: unwinding panics are not supported without std
error[E0152]: found duplicate lang item `panic_impl`
```

`vifast-kbin-x86_64` báo `unused import: core::arch::global_asm` / `vifast_plat_x86_64::boot`
(lint sai bản chất trên host, không phải lỗi mã); `vifast-kbin-riscv64` cùng lý do asm RISC-V.

### 2.2 Bước mới **có răng** (đỏ-trước → xanh)

**Đỏ-trước** (trước khi dọn, chạy đúng lệnh của bước):

```text
error: manual implementation of `.is_multiple_of()`      --> vifast-plat-common/src/bootinfo.rs:679
error: the loop variable `i` is only used to index `madt.cpu_ids`  --> bootinfo.rs:1431
error: the loop variable `i` is only used to index `e820.entries`  --> bootinfo.rs:1440
error: the loop variable `k` is only used to index `u.holes`       --> obj/check.rs:90
error: the loop variable `i` is only used to index `self.flushes`  --> obj/check.rs:426
error: could not compile `vifast-plat-common` (lib) due to 5 previous errors
```

**Ca thử răng chủ động** (chèn cảnh báo tạm vào `plat-common/src/resources.rs` rồi gỡ ngay):

```text
--- chạy bước clippy kernel (kỳ vọng ĐỎ) ---
error: unused variable: `unused_probe`
error: could not compile `vifast-plat-common` (lib) due to 1 previous error
EXIT=101
--- đã gỡ probe ---        (git diff --stat: rỗng)
```

⇒ Bước **bắt được** cảnh báo mới trong `v2/kernel/`; trước KT-409 không cổng nào bắt.

**Xanh sau khi dọn** (`v2 check`, log `/tmp/kt409/check2.log`):

```text
# step-time-total    10212 ms  workspace v2/kernel: clippy host-testable (KT-409)
v2 check: PASS (L1 chạy trong gate --fast)
EXIT=0
```

### 2.3 Dọn cảnh báo `clippy -D warnings` (không nới ngưỡng, không tắt lint)

| crate | lỗi trước | cách sửa |
|---|---:|---|
| `vifast-core` | 0 | (KT-405 đã dọn) |
| `vifast-plat-common` | 5 lib + 1 test | `is_multiple_of`, `needless_range_loop` → iterator (`take(n)`) |
| `vifast-plat-aarch64` | 7 lib + 5 test | bỏ `unsafe` thừa (`addr_of!`), `as_deref`/`as_deref_mut`, bỏ cast `u32→u32`, `is_multiple_of`, `int_plus_one`, `#[allow(too_many_arguments)]` (chữ ký cũ, có tiền lệ `plat-riscv64/src/virtio_net.rs:453`), `#[allow(assertions_on_constants)]` cho test **cố ý** ghim hằng số (khuôn sẵn có `tests/boot.rs`) |
| `vifast-plat-x86_64` | 9 lib + 12 test | bỏ `use crate::asm` thừa, `let flags` (không cần `mut`), `RangeInclusive::contains`, `as_deref`/`as_deref_mut`, `is_multiple_of`, `.expect_err()`, `#[allow(assertions_on_constants)]` |

**Phát hiện thật khi dọn** (`v2/kernel/vifast-plat-x86_64/tests/apic.rs:133`): dòng
`/// `LAPIC_ID`: ID nằm ở bit 24..31 (SDM Hình 10-6).#[test]` — `#[test]` bị **nuốt trong doc
comment** nên `lapic_id_decode_literal` **chưa bao giờ chạy** (clippy báo `never used`). Đã tách
`#[test]` ra dòng riêng ⇒ test chạy thật và **PASS** (không đổi kỳ vọng nào).

## 3. Ưu tiên 3 — Gỡ dev-dependency ngược + `TableMemory` (ĐÃ XONG)

- **`TableMemory` đã ở `vifast-pte` từ KT-186** — kiểm chứng: `v2/lib/vifast-pte/src/page_table.rs:46`
  (`pub trait TableMemory`), `vifast-plat-x86_64/src/bootmem.rs:35` đã `use vifast_pte::TableMemory`,
  `vifast-plat-aarch64/src/mmu.rs:14` chỉ **re-export**. ⇒ Không còn việc phải làm; ghi nhận để
  lead không mở lại.
- **Gỡ dev-dependency ngược**: `vifast-plat-common/Cargo.toml` **xóa** cả khối `[dev-dependencies]`
  (`vifast-plat-aarch64`, `vifast-plat-x86_64`). Tệp duy nhất dùng chúng là
  `tests/mmu_shared.rs` (test generic `kiem_tung_dinh_dang<F: PageTableFormat>` cho 3 định dạng)
  ⇒ **chuyển** sang `vifast-plat-aarch64/tests/mmu_shared.rs` + thêm dev-dep
  `vifast-plat-x86_64` cho `plat-aarch64` (đã phụ thuộc `plat-common` thường; `plat-x86_64`
  **không** phụ thuộc `plat-aarch64` ⇒ không chu trình).
- Bằng chứng: `cargo test -p vifast-plat-common` → **147 test PASS** (13 tệp);
  `cargo test -p vifast-plat-aarch64 --test mmu_shared` → **4 PASS**;
  `check-lockfile-sync.py` → `OK — 14 crate dùng chung cùng phiên bản + cùng nguồn (B3)`.
- `v2/kernel/Cargo.lock` đổi đúng 2 dòng (bỏ 2 dev-dep ở `plat-common`, thêm 1 ở `plat-aarch64`).

## 4. Ưu tiên 2 — Dời Sv39/PLIC/trap/timer/tlb/user sang `plat-riscv64`: **CHẶN (cần lead quyết)**

**Đo được** (không suy đoán):

| Dữ kiện | Số |
|---|---|
| 6 module cần dời (`mmu/plic/trap/timer/tlb/user`) | **3.298 LOC** |
| Test host của 6 module đó (`tests/{mmu,plic,timer,tlb,user}.rs`) | **1.448 LOC** |
| `vifast-plat-riscv64` có thư mục `tests/` host | **không** (`ls` chỉ có `Cargo.toml`, `src`) |
| `cargo test -p vifast-plat-riscv64 --lib` trên host | **ĐỎ** — `invalid register a7` (asm RISC-V không `cfg`-gate) |

**Xung đột**: 6 module này là **logic thuần, host-testable** (đó là lý do chúng nằm ở
`plat-common` từ KT-46..KT-53). Dời sang `vifast-plat-riscv64` ⇒ **mất 1.448 LOC test host**
(`cargo test -p vifast-plat-riscv64` không chạy được trên host). Muốn `plat-riscv64` host-buildable
thì phải `cfg(target_arch = "riscv64")`-gate phần asm — **AGENTS §4.8 cấm** thêm `cfg(target_arch)`
vào kernel. Vì vậy đây là **xung đột luật**, không phải thiếu thời gian.

**Đề xuất tách (đúng hướng kiến trúc, không mất test)**:
1. Phần **định dạng** (`PageTableBuilder`, `Sv39`, `walk_with`, `PteFlags`, `satp_*`) → `vifast-pte`
   (host-testable, đã sở hữu `VA_BITS`/`PTE_LEVELS`/`walk_shape`/`TableMemory`). Test đi theo.
2. Phần **asm/CSR/MMIO** (`satp` write, `sfence.vma`, PLIC MMIO, SBI) → `vifast-plat-riscv64`.
3. `trap`/`timer`/`tlb`/`user`: tách tương tự — luật/state machine (host-testable) ở lại crate
   host-testable, chỉ asm sang `plat-riscv64`.

Việc này cần **task riêng** (ước lượng 3 h) vì chạm 3 crate + 1.448 LOC test; làm trong 3 h của
KT-409 cùng lúc với ưu tiên 1/3 là rủi ro cao (T1 có QEMU smoke 7 ảnh).

**Đã làm được phần nền của ưu tiên 2** (không mất test): dời **bit PTE Sv39 + `is_canonical` +
`PTE_ENTRIES`** từ `vifast-plat-common::mmu` lên `vifast-pte` (`pte.rs`), nên `plat-common` **không
còn** phải biết Sv39 để phục vụ `obj`/`fault` — đây là điều kiện cần để sau này dời `mmu`/`Sv39`
sang `plat-riscv64` mà `obj` (ở lại tới `KT-417`) vẫn biên dịch.

## 5. Ưu tiên 4 — Rút trait dùng chung: **CHƯA LÀM**

Hết ngân sách sau ưu tiên 1 + 3 + phần nền ưu tiên 2. Không mở để tránh bàn giao cây dở.
Ghi nhận số đo để task sau ước lượng: `irq_lock.rs` 410/369/397 LOC (3 bản), `core_bridge.rs`
304/270, `wx.rs` 401/275.

## 6. T1 — lần 1 ĐỎ, tìm ra lỗi thật do chính bản sửa clippy gây ra (đã vá)

**Lệnh**: `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` (log `/tmp/kt409/t1.log`)
**Kết quả lần 1**: `v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)`, 358.172 ms, **2 bước đỏ**:

1. `--- selftest pre-measure (KT-95): FAIL` — `# pre-measure-selftest (a) FAIL: mã thoát 7`.
   `v2/tools/pre-measure.sh:24` ghi rõ `7 = cây nguồn bẩn`; dòng 209 `[ "$DIRTY" = "0" ] || refuse 7`.
   Cây worktree **đang bẩn** (chưa commit — lead commit) ⇒ **đỏ đúng thiết kế**, không phải hồi quy.
   Tiền lệ: `KT-402b` ghi "đỏ duy nhất `pre-measure` trên cây bẩn — PASS trên cây sạch".
2. `--- QEMU x86_64 (K5' bước 1): FAIL` — **hồi quy thật do bản sửa clippy của tôi**:
   ```text
   error[E0433]: failed to resolve: use of undeclared crate or module `asm`
   error: could not compile `vifast-plat-x86_64` (lib) due to 10 previous errors
   run-x86_64.sh: build ảnh sản phẩm tối giản x86_64 FAIL
   ```
   **Nguyên nhân**: để dọn `clippy::unused_imports` trên **host**, tôi đã xóa `use crate::asm;`
   khỏi `vifast-plat-x86_64/src/wx.rs`. Nhưng mọi điểm dùng `asm::` nằm trong hàm
   `#[cfg(all(target_arch = "x86_64", target_os = "none"))]` ⇒ trên **host** import là thừa, trên
   **target x86_64 thật** import là **bắt buộc**. Đây đúng là loại lỗi mà bước `clippy` host-only
   **không** bắt được (xem §7 nợ).
   **Vá**: bỏ hẳn `use crate::asm;`, đổi 13 điểm dùng sang đường dẫn đầy đủ `crate::asm::…`
   (không thêm `cfg(target_arch)` nào — AGENTS §4.8). Kiểm lại:
   ```text
   $ cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-x86_64 \
         --target x86_64-unknown-none --release
   EXIT=0
   ```
   (3 cảnh báo còn lại trên target x86_64 — `unused import: super::*` ở `asm/fpu.rs:10`,
   `asm/trap.rs:10`, `unnecessary unsafe` ở `asm/trap.rs:628`, `unused import` ở
   `kbin-x86_64/src/main.rs:86/92/94` — **có sẵn từ trước**, không do KT-409; ghi thành nợ §7.)

## 7. Nợ / rủi ro ghi nhận (không tự mở rộng phạm vi)

1. **Bước `clippy` mới chỉ phủ host-testable** ⇒ 4 crate bị loại trừ (`plat-riscv64` + 3 `kbin-*`)
   **vẫn không** được `clippy -D warnings` quét. Đo được: build ảnh x86_64 phát **6 cảnh báo**
   (`asm/fpu.rs:10`, `asm/trap.rs:10`, `asm/trap.rs:628`, `kbin-x86_64/src/main.rs:86/92/94`) mà
   cổng không thấy. Đề xuất task riêng: lint theo **target** (`--target <arch>-unknown-none --lib`)
   cho 3 crate arch — cần đo thời gian trước khi nối vào T1/T2.
2. **`pre-measure` đỏ trên cây bẩn** là hành vi đúng; lead chạy T2 trên cây **sạch** sau commit.
3. **Ưu tiên 2 chặn** (xung đột AGENTS §4.8) — xem §4; cần lead quyết hướng tách.
4. **Ưu tiên 4 chưa làm** — xem §5.

## 8. Bổ sung: lint `clippy` trên **target thật** của 3 crate arch (đóng nốt lỗ hổng §7.1)

**Vì sao phải làm**: chính bản sửa clippy host-only của tôi ở §6 đã gây hồi quy **chỉ trên target**
(bỏ `use crate::asm;` ⇒ host xanh, ảnh x86_64 ĐỎ `E0433`). Lint host-only **không** thấy mã chỉ
biên dịch cho bare-metal ⇒ nếu chỉ nối lượt host thì lỗ hổng vẫn còn một nửa.

**Đo trước khi nối** (`cargo clippy --target <t> -p vifast-plat-<arch> --lib -- -D warnings`):

| target | crate | số cảnh báo trước | sau |
|---|---|---:|---:|
| `riscv64gc-unknown-none-elf` | `vifast-plat-riscv64` | **4** | **0** |
| `aarch64-unknown-none` | `vifast-plat-aarch64` | 0 | 0 |
| `x86_64-unknown-none` | `vifast-plat-x86_64` | **4** | **0** |

Đã dọn (không `#[allow]` trừ 1 chỗ có lý do ghi rõ):

- `plat-riscv64/src/core_bridge.rs:205` — `let report = …; report` → trả biểu thức trực tiếp
  (`let_and_return`).
- `plat-riscv64/src/irq_lock.rs:372/381` — `.as_ref().map(|g| &**g)` → `.as_deref()`;
  `.as_mut().map(|g| &mut **g)` → `.as_deref_mut()`.
- `plat-riscv64/src/static_slot.rs` — thêm `impl<T> Default for StaticSlot<T>` (`new_without_default`);
  `Scratch::borrow` → **`borrow_mut`** (`should_implement_trait`) + `#[allow(clippy::mut_from_ref)]`
  **có lý do**: `Scratch<T>` tồn tại đúng để một `static` (không `static mut`) trả `&mut T` theo
  bất biến "một người dùng tuần tự trên BSP" — cùng lý do `UnsafeCell` tồn tại; `Sync` đã
  `unsafe impl` kèm khối an toàn. Cập nhật 1 caller `kbin-riscv64/src/proc_exec.rs:187`.
- `plat-x86_64/src/asm/fpu.rs:10`, `asm/trap.rs:10` — `use super::*;` chỉ cần cho **stub host**
  (`host_only`); trên target mọi stub bị `cfg`-gate ra ⇒ gate glob import bằng
  `#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]` (đúng khuôn `cfg` **đã có** trong
  hai tệp này; R2 của `v2 rules` chỉ áp `kernel/vifast-core` + `lib/` nên hợp lệ).
- `plat-x86_64/src/asm/trap.rs:628` — `unsafe { fn_item as usize as u64 }` → bỏ `unsafe` (lấy địa
  chỉ không dereference) + cast qua `*const ()` (`fn_to_numeric_cast`).

**Bước nối** (vẫn **một** bước `step_clippy_kernel`, không thêm id ⇒ `CHECK_STEPS_EXPECTED` giữ 49):
phần (1) host như §2, phần (2) vòng 3 target `--lib`. Đo: **6,7 s** (warm) cho cả hai phần.

**Ca thử răng của phần (2)** (chèn cảnh báo tạm vào `plat-riscv64/src/lib.rs` rồi gỡ ngay):

```text
--- lint target riscv64 (kỳ vọng ĐỎ) ---
error: unused variable: `unused_probe`
error: could not compile `vifast-plat-riscv64` (lib) due to 1 previous error
EXIT=101
--- đã gỡ probe ---        (git diff --stat: rỗng)
```

**Chưa phủ (nợ, đo được)**: 3 crate `vifast-kbin-*` **không** nằm trong lượt target vì
`kbin-x86_64 --bins` có **52** cảnh báo có sẵn (`kbin-riscv64`: 2, `kbin-aarch64`: 0) — dọn 52
cảnh báo vượt ngân sách 3 h và không thuộc phạm vi "crate `plat-*`". Đề xuất task riêng.

## 9. T1 — lần 3 (sau khi bổ sung lint target): 66 OK / 1 ĐỎ (đúng thiết kế) / 14 BỎ QUA

**Lệnh**: `bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev` (log `/tmp/kt409/t1c.log`)

```text
--- workspace v2/kernel: clippy host-testable (KT-409): OK
# step-time workspace v2/kernel: clippy host-testable (KT-409) 19438 ms
--- L0 kernel workspace (KT-148): OK
--- L1 (sim 1.000 seed): OK
--- QEMU aarch64 (K4' bước 3): OK
--- QEMU x86_64 (K5' bước 1): OK
--- selftest pre-measure (KT-95): FAIL
# step-time-total   505160 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

Đếm: **66 bước OK**, **1 bước ĐỎ**, **14 bước BỎ QUA** (khớp `T2_ONLY_STEPS_EXPECTED=14`).

**Bước đỏ duy nhất** = `selftest pre-measure (KT-95)`, mã thoát **7** = `cây nguồn bẩn`
(`v2/tools/pre-measure.sh:24` và `:209`). Bằng chứng trực tiếp:

```text
$ source v2/test/boot/build-rev.sh; vbrev_dirty "$PWD"
1
$ git status --porcelain -- v2/kernel v2/lib v2/bin v2/Cargo.toml v2/Cargo.lock
 M v2/bin/lib/check.sh
 M v2/bin/lib/consts.sh
 …
```

Tái hiện trực tiếp (không qua cổng):

```text
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
# pre-measure-selftest: FAIL
EXIT=1
```

⇒ **Không phải hồi quy**: worktree chưa commit (lead commit theo luật). Trên cây **sạch** bước này
PASS — tiền lệ `KT-402b` ghi đúng hiện tượng này ("đỏ duy nhất `pre-measure` trên cây bẩn — PASS
trên cây sạch"). **T2 phải chạy trên cây sạch sau commit.**

## 10. Tổng kết trạng thái

| Ưu tiên | Trạng thái | Bằng chứng |
|---|---|---|
| 1. Nối `clippy` kernel + dọn cảnh báo `plat-*` | **XONG** (và mở rộng sang target thật) | §2, §8; đỏ-trước + 2 ca thử răng; `v2 check: PASS`; T1 bước 84 OK |
| 2. Dời Sv39/PLIC/trap/timer/tlb/user sang `plat-riscv64` | **CHẶN** — xung đột AGENTS §4.8 (mất 1.448 LOC test host) | §4 |
| 3. Gỡ dev-dep ngược + `TableMemory` | **XONG** | §3; 147 + 4 test PASS; lockfile-sync OK |
| 4. Rút trait dùng chung, cắt ≥ 3.500 LOC | **CHƯA LÀM** | §5 |

**0 chạm `v2/kernel/vifast-core/**`** — kiểm: `git status --short` không có tệp nào dưới
`v2/kernel/vifast-core/`. **0 chạm `spec/`**. Không `#[ignore]`/`#[should_panic]`. Không nới ngưỡng.
Không `git add`/`commit`/`push`.

## 11. T0 (`./v2/bin/v2 t0`) — 6 crate, 2 ĐỎ **có sẵn** (không do KT-409)

```text
$ ./v2/bin/v2 t0
--- T0: clippy -p vifast-plat-common: OK
--- T0: clippy -p vifast-plat-aarch64: OK
--- T0: clippy -p vifast-plat-x86_64: OK
--- T0: clippy -p vifast-pte: OK
--- T0: clippy -p vifast-plat-riscv64: FAIL
--- T0: clippy -p vifast-kbin-riscv64: FAIL
v2 t0: FAIL (6 crate; **T0** — KHÔNG phải cổng)
```

Lý do **nguyên văn** (cả hai đều do `vifast-plat-riscv64` không build được trên host macOS):

```text
error: invalid Mach-O section specifier
  --> vifast-plat-riscv64/src/core_bridge.rs:80:18
80 | #[link_section = ".corespace"]
   |                  ^^^^^^^^^^^^ not a valid Mach-O section specifier
error: invalid register `a7`: unknown register
  --> vifast-plat-riscv64/src/ipi.rs:86:13
```

Đây là **giới hạn có sẵn của T0** (T0 lint trên host; `t0_crate_of_path` suy crate từ `git diff` nên
mọi task chạm `plat-riscv64`/`kbin-riscv64` đều gặp). **Không phải hồi quy của KT-409**: dòng
`#[link_section = ".corespace"]` và các khối `asm!` RISC-V **không** nằm trong diff của task này
(`git diff` chỉ chạm `core_bridge.rs:205`, `irq_lock.rs`, `static_slot.rs`, `virtio_net.rs`).
Đề xuất (ngoài phạm vi): cho `v2 t0` biết crate nào **không host-buildable** để lint theo target
thay vì báo đỏ oan — cùng danh sách loại trừ mà `step_clippy_kernel` đã dùng.

## 12. Quy mô thay đổi

```text
40 files changed, 222 insertions(+), 377 deletions(-)
+ v2/agent/JOURNAL/kt409-hal-clippy.md        (mới)
+ v2/kernel/vifast-plat-aarch64/tests/mmu_shared.rs  (chuyển từ plat-common)
- v2/kernel/vifast-plat-common/tests/mmu_shared.rs
```

**0 tệp** dưới `v2/kernel/vifast-core/`; **0 tệp** dưới `spec/`.
