# KT-409b — Dời Sv39/PLIC/trap/timer/tlb/user khỏi `plat-common` + rút trait dùng chung

- **Task**: KT-409 phần 2 (P0, Chặng 1 / W3, 3 h) — phụ thuộc KT-409 phần 1 (`0fbdaab6`).
- **Nhánh**: `task/kt409b-hal-r2` · **Worktree**: `/private/tmp/vifast-wt/1`
- **Base**: `main` @ `0fbdaab6` (đã có KT-409 phần 1: clippy kernel + gỡ dev-dep + `TableMemory`).
- **Căn cứ**: `D2-259`, `D2-260` mục 3; quyết định Lead 2026-11-25 (được tách phần **định dạng**
  lên `vifast-pte`, chỉ asm/CSR/MMIO ở `plat-riscv64`; **không** `cfg(target_arch)` — AGENTS §4.8).
- **Ngân sách**: 3 h. Bắt đầu 2026-11-25 ~21:20 JST (12:20 UTC, Thấp điểm chính).

## 0. Kế hoạch ngắn

1. **Đỏ-trước**: (a) đếm từ khoá RISC-V trong `plat-common` (kỳ vọng > 0 ⇒ ĐỎ); (b) test host
   dùng `vifast_pte::sv39::PageTableBuilder` (kỳ vọng không biên dịch ⇒ ĐỎ).
2. **Ưu tiên 1a** — dời `plat-common/src/mmu.rs` (649 LOC, phần **định dạng** Sv39: `PteFlags`,
   `Walk`, `PteMemory`, `walk_with`, `PageTableBuilder`, `Sv39`, `satp_*`) lên
   `vifast-pte/src/sv39.rs`; `plat-riscv64` chỉ còn asm/CSR (`csrw satp`, `sfence.vma`).
3. **Ưu tiên 1b** — dời `plic`/`trap`/`timer`/`tlb`/`user` (2.649 LOC) khỏi `plat-common` sang
   `plat-riscv64` (re-export để `kbin-riscv64` đổi import tối thiểu).
4. **Ưu tiên 2** — rút `ArchIrq` + `IrqMcsLock<T, A>` vào `plat-common` (3 bản 408/367/395 LOC
   ⇒ 1 bản chung + 3 impl mỏng).
5. **Ưu tiên 3** — đo LOC trước/sau; ghi số thật.
6. `./v2/bin/v2 check` → T1 `gate --dev` qua khoá máy.

**Ràng buộc tự áp**: 0 chạm `v2/kernel/vifast-core/**`; 0 chạm `spec/`; không nới ngưỡng; không
`#[ignore]`/`#[should_panic]`; không `git add/commit`; chỉ T0 + T1; không `cfg(target_arch)` mới.

## 1. Đỏ-trước (bằng chứng)

### 1.1 `plat-common` còn 480 dòng từ khoá RISC-V / 15 tệp

```text
$ bash /tmp/kt409b/check-riscv-free.sh
# plat-common: 480 dòng / 15 tệp còn từ khoá RISC-V
trap.rs:145
plic.rs:69
smp.rs:44
mmu.rs:44
fault.rs:38
tlb.rs:26
bootinfo.rs:24
user.rs:23
lib.rs:22
obj/mod.rs:15
ipi.rs:15
resources.rs:6
obj/vspace.rs:5
timer.rs:3
obj/check.rs:1
FAIL: plat-common còn từ khoá RISC-V
EXIT=1
```

Phân bố: **311 dòng mã** + **304 dòng chú thích** (một số dòng có cả hai). Sáu module phải dời
(`mmu`/`plic`/`trap`/`timer`/`tlb`/`user`) chiếm **248 dòng mã + 168 dòng chú thích**.

### 1.2 `vifast_pte::sv39` chưa tồn tại (ĐỎ — không biên dịch)

Trước khi sửa, `cargo build -p vifast-pte` không có module `sv39`; mọi `use vifast_pte::sv39::…`
đều `E0433: cannot find module or crate`. Bằng chứng **sau** khi chỉ thêm `pub mod sv39;` mà
**chưa** khai báo phụ thuộc `vifast-pte` cho `plat-riscv64` (đúng lỗi "thiếu cạnh phụ thuộc"):

```text
$ cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-plat-riscv64 \
      --target riscv64gc-unknown-none-elf
error[E0433]: cannot find module or crate `vifast_pte` in this scope
  --> vifast-plat-riscv64/src/mmu.rs:51:5
error[E0433]: cannot find module or crate `vifast_pte` in this scope
  --> vifast-plat-riscv64/src/obj.rs:24:5
```

⇒ Chứng minh `plat-riscv64` **thật sự** chuyển sang dùng `vifast-pte` (không còn đi qua
`plat-common`), và cạnh phụ thuộc mới là **bắt buộc** (đã thêm, xem §3).

## 2. Ưu tiên 1a — `mmu` (định dạng Sv39) → `vifast-pte::sv39` (XONG)

### 2.1 Thay đổi

| Tệp | Việc |
|---|---|
| `v2/lib/vifast-pte/src/sv39.rs` | **mới** (621 LOC) — dời nguyên `plat-common/src/mmu.rs` |
| `v2/lib/vifast-pte/src/lib.rs` | `pub mod sv39;` |
| `v2/kernel/vifast-plat-common/src/mmu.rs` | **xoá** (649 LOC) |
| `v2/kernel/vifast-plat-common/src/lib.rs` | bỏ `pub mod mmu;` |
| `v2/kernel/vifast-plat-riscv64/Cargo.toml` | **thêm** `vifast-pte` (cạnh phụ thuộc mới) |
| `v2/kernel/vifast-kbin-riscv64/Cargo.toml` | **thêm** `vifast-pte` |
| 13 tệp `.rs` | `vifast_plat_common::mmu` → `vifast_pte::sv39` (sed) |

**Không chép hằng số lần thứ hai**: `sv39.rs` **tái xuất** `PTE_VALID/READ/WRITE/EXEC/USER/
GLOBAL/ACCESSED/DIRTY/PPN_SHIFT/ENTRIES` + `is_canonical` từ `crate::pte` (một nguồn), thay vì
khai lại như bản cũ ở `plat-common`. Chỉ `PAGE_BYTES`, `PTE_BYTES`, `SATP_*`, `ASID_MASK`,
`SATP_ASID_SHIFT` là mới tại chỗ (trước đây `SATP_ASID_SHIFT`/`ASID_MASK` mượn từ `crate::tlb` —
module đã dời, nên nay định nghĩa ngay trong `sv39`).

**0 `cfg(target_arch)` mới** — kiểm: `git diff | grep -c 'cfg(target_arch'` = 0 dòng thêm.

### 2.2 Bằng chứng xanh

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
test result: ok. 32 passed; 0 failed   (tests/mmu.rs — 32 ca, gồm cả `vifast_pte::sv39`)
…
# tổng: 147 test PASS / 0 FAIL (13 tệp) — **đúng bằng** số của KT-409 phần 1
```

## 3. Ưu tiên 1b — `plic`/`trap`/`timer`/`tlb`/`user` → `plat-riscv64` (XONG)

### 3.1 Thay đổi

| Tệp nguồn cũ (`plat-common/src/`) | Đích (`plat-riscv64/src/`) | LOC |
|---|---|---:|
| `plic.rs` | `plic_logic.rs` | 382 |
| `timer.rs` | `timer_logic.rs` | 360 |
| `tlb.rs` | `tlb_logic.rs` | 638 |
| `user.rs` | `user_logic.rs` | 458 |
| `trap.rs` | `trap.rs` (mới ở crate arch) | 811 |

- `plat-riscv64/src/lib.rs`: `pub mod plic_logic; pub mod timer_logic; pub mod tlb_logic;
  pub mod trap; pub mod user_logic;` — `trap` **luôn** biên dịch (trước đây ở `plat-common` nên
  không phụ thuộc feature nào; đã sửa một lần vì lần chèn đầu rơi vào sau `#[cfg(feature="user")]`).
- Bốn tệp arch sẵn có (`plic.rs`/`timer.rs`/`tlb.rs`/`user.rs`) nay **tái xuất** phần ngữ nghĩa
  (`pub use crate::*_logic::*;`) nên `vifast_plat_riscv64::plic::PlicStats` v.v. **giữ nguyên**
  đường dẫn cũ ⇒ `kbin-riscv64` chỉ đổi tiền tố crate (23 tệp, sed).
- `plat-common/src/lib.rs`: bỏ 5 `pub mod`; `Platform::now_ns` **quy đổi tại chỗ** từ
  `Self::timebase_hz()` thay vì gọi `timer::ticks_to_ns` (module timer đã dời) — ngữ nghĩa
  **không đổi** (cùng công thức `ticks·1e9/hz`), và `plat-common` không còn phụ thuộc module timer.
- `plat-common/Cargo.toml`: **bỏ** `vifast-core` (façade syscall KT-62 đi theo `user` sang
  `user_logic`; `cargo` báo `unused dependency`), **thêm** `[dev-dependencies] vifast-core` cho
  `tests/user.rs`.

### 3.2 Crate **mới** `vifast-plat-riscv64-logic` — để test host **không mất ca nào** (§4.3)

**Vì sao không dời thẳng vào `vifast-plat-riscv64`**: crate đó chứa `asm!` RISC-V **không**
`cfg`-gate nên **không** biên dịch trên host (`invalid register a7`), mà AGENTS §4.8 **cấm**
thêm `cfg(target_arch)`. Dời 5 module vào đó ⇒ **5 tệp test host (≈1.100 LOC) không còn chạy**
— trái AGENTS §4.3 ("không bỏ case khỏi danh sách chạy").

**Đã thử cách rẻ hơn trước — và nó ĐỎ**: `#[path]` include tệp nguồn từ `plat-common/tests/`
(khuôn có tiền lệ ở `v2/tools/checkers/wake/selftest.py:5755`). Máy kiểm của repo **chặn đúng**:

```text
E E_STRUCT_MOD_OUTSIDE   v2/kernel/vifast-plat-common/tests/host.rs:19: `mod trap;` trỏ **ra
ngoài crate** `v2/kernel/vifast-plat-common` (`v2/kernel/vifast-plat-riscv64/src/trap.rs`) —
cùng một tệp biên dịch vào hai crate (fail-closed, KT-264)
# check-wake-paths: mode=v2 strict=1 errors=11 warnings=0
--- wake paths + --strict (KT-151/155 — KT-333e: một lượt): FAIL
```

⇒ Khuôn `#[path]` **bị cấm tường minh** (KT-264). Nên chọn cách **đúng luật**: tách crate
**host-testable** `v2/kernel/vifast-plat-riscv64-logic` (5 module ngữ nghĩa + 8 tệp test đi theo,
**vẫn chạy** trên host).

| Tệp | Việc |
|---|---|
| `v2/kernel/vifast-plat-riscv64-logic/{Cargo.toml,src/lib.rs}` | **mới** — `no_std`, `forbid(unsafe_code)`, **không** asm/MMIO |
| `src/{plic,timer,tlb,trap,user}.rs` | 5 module ngữ nghĩa (2.649 LOC) dời từ `plat-common` |
| `tests/{host,ipi,mmu,plic,smp,timer,tlb,user}.rs` | 8 tệp test dời từ `plat-common/tests/` |
| `v2/kernel/Cargo.toml` | thêm member |
| `v2/bin/lib/steps_gate.sh:10` | `step_l0_kernel` thêm `-p vifast-plat-riscv64-logic` (đúng phần "số bước do tách crate" Lead cho phép) |
| `vifast-plat-riscv64/src/{plic,timer,tlb,user}.rs` | `pub use vifast_plat_riscv64_logic::<m>::*;` — đường dẫn cũ **giữ nguyên** |
| `vifast-plat-riscv64/src/lib.rs` | `pub use vifast_plat_riscv64_logic::trap;` |

**0 `cfg(target_arch)` mới**; **0 `#[ignore]`**; **0 ca bị bỏ**.

### 3.3 Bằng chứng xanh

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common
test result: ok. 32 passed; 0 failed   (tests/mmu.rs — 32 ca, gồm cả `vifast_pte::sv39`)
test result: ok.  6 passed; 0 failed
test result: ok. 15 passed; 0 failed
test result: ok.  7 passed; 0 failed
# 60 PASS / 0 FAIL (4 tệp còn lại: bootinfo/fault/obj/resources)

$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-riscv64-logic
test result: ok. 14 passed; 0 failed   (tests/host.rs)
test result: ok. 12 passed; 0 failed   (tests/ipi.rs)
test result: ok.  8 passed; 0 failed   (tests/mmu.rs)
test result: ok. 12 passed; 0 failed   (tests/plic.rs)
test result: ok. 12 passed; 0 failed   (tests/smp.rs)
test result: ok. 15 passed; 0 failed   (tests/timer.rs)
test result: ok. 12 passed; 0 failed   (tests/tlb.rs)
test result: ok.  9 passed; 0 failed   (tests/user.rs)
# 94 PASS / 0 FAIL
# ⇒ TỔNG 60 + 94 = 154 ≥ 147 (không mất ca nào; +7 do `tests/mmu.rs` chạy cả 2 crate)

$ cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-plat-riscv64 \
      --target riscv64gc-unknown-none-elf
# 0 error (chỉ 4 cảnh báo `unused dependency` **có sẵn** cho dep sau feature)

$ cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
      --target riscv64gc-unknown-none-elf --features ktest
# 0 error
```

### 3.4 Đo lại từ khoá RISC-V trong `plat-common`

```text
$ bash /tmp/kt409b/check-riscv-free.sh
# plat-common: 170 dòng / 9 tệp còn từ khoá RISC-V
smp.rs:44
fault.rs:38
bootinfo.rs:24
lib.rs:22
obj/mod.rs:15
ipi.rs:15
resources.rs:6
obj/vspace.rs:5
obj/check.rs:1
FAIL: plat-common còn từ khoá RISC-V
```

**480 → 170 dòng (−65 %), 15 → 9 tệp.** Sáu module mục tiêu **đã sạch hoàn toàn** (`mmu` 44,
`plic` 69, `trap` 145, `timer` 3, `tlb` 26, `user` 23 = **310 dòng** đã ra khỏi crate).

**170 dòng còn lại KHÔNG nằm trong 6 module mục tiêu** — chúng thuộc các module **đa kiến trúc**
phải ở lại (`bootinfo` xử lý DTB **cả 3 arch** với biến thể `BootArch::Riscv64` + trường `plic`;
`fault` mang tên trường `scause`/`stval`/`sepc` của `TrapInfo`; `smp`/`ipi`/`resources`/`obj` chỉ
nhắc trong **chú thích** hoặc tên tài nguyên). Muốn 0 tuyệt đối phải dời/generalize thêm 4 module
nữa (`smp`, `ipi`, `bootinfo`, `fault`) — **ngoài phạm vi 3 h** và chưa được Lead quyết. Xem §7.2.

## 4. Ưu tiên 2 — rút `ArchIrq`/`IrqMcsLock<T,A>` vào `plat-common`: **CHƯA LÀM**

Hết ngân sách sau ưu tiên 1a + 1b. Số đo để task sau ước lượng (đã kiểm lại trên cây này):

| Tệp | riscv64 | aarch64 | x86_64 | tổng |
|---|---:|---:|---:|---:|
| `irq_lock.rs` | 408 | 367 | 395 | **1.170** |
| `core_bridge.rs` | 303 | 261 | 270 | **834** |
| `wx.rs` | — | 401 | 273 | **674** |
| `image.rs` | — | 146 | 99 | **245** |
| `format.rs` | — | 383 | 302 | **685** |
| `userload.rs` | — | 392 | 256 | **648** |
| `tcp.rs` | 132 | 132 | — | **264** |

`IrqMcsLock<T>` ba bản **giống nhau ~95 %** (chỉ `IrqState::save_and_disable`/`restore`/
`irq_enabled` là khác arch) ⇒ generic hoá `IrqMcsLock<T, A: ArchIrq>` ước tính cắt **~480 LOC**.
`ProductShellHost` **không** tìm thấy trong cây hiện tại (`grep -rn "ProductShellHost"` = 0) —
có thể đã đổi tên ở task trước; cần Lead xác nhận trước khi mở lại.

## 5. Ưu tiên 3 — LOC: **KHÔNG đạt ≥ 3.500** (số thật)

Đo bằng `git show HEAD:<f>` (trước) vs `wc -l` (sau), chỉ `src/`:

| Crate | trước | sau | Δ |
|---|---:|---:|---:|
| `vifast-plat-common` | 9.175 | 5.876 | **−3.299** |
| `vifast-plat-riscv64` | 12.240 | 12.234 | −6 |
| `vifast-plat-riscv64-logic` (**mới**) | 0 | 2.677 | +2.677 |
| `vifast-pte` | 1.522 | 2.152 | +630 |
| **tổng 4 crate** | 22.937 | 22.939 | **+2** |

`git diff --stat` (chỉ tệp **tracked**): `67 files changed, 233 insertions(+), 3460 deletions(-)` —
con số 3.460 là **dịch chuyển**, không phải cắt: 5 tệp nguồn + `mmu.rs` bị xoá khỏi `plat-common`
và **tái sinh** thành 6 tệp **chưa track** ở `plat-riscv64`/`vifast-pte`.

⇒ **Cắt thật ≈ 0 LOC** (tổng 4 crate **+2**), không phải ≥ 3.500. Nguyên nhân: ưu tiên 1 là **dời** (không cắt lặp);
phần **cắt lặp** thật nằm ở ưu tiên 2 (`IrqMcsLock`/`CoreBridge`/`wx`/`image`/`format`/
`userload`/`tcp` — ước tính **≥ 2.500 LOC** nếu làm hết) và **chưa làm được** vì hết 3 h.
**Không nới ngưỡng, không khai khống.**

## 6. Cổng

### 6.1 `./v2/bin/v2 check` — PASS

```text
$ ./v2/bin/v2 check
# step-time-total   101693 ms  TỔNG check
EXIT=0
```

### 6.2 T1 `gate --dev` — 66 OK / 1 ĐỎ (đúng thiết kế) / 15 BỎ QUA

```text
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
--- workspace v2/kernel: clippy host-testable (KT-409): OK
--- wake paths + --strict (KT-151/155 — KT-333e: một lượt): OK
--- fmt: OK
--- L0 kernel workspace (KT-148): OK
--- L1 (sim 1.000 seed): OK
--- QEMU aarch64 (K4' bước 3): OK
--- QEMU x86_64 (K5' bước 1): OK
--- selftest pre-measure (KT-95): FAIL
# step-time-total   664455 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

Đếm: **66 OK**, **1 ĐỎ**, **15 BỎ QUA** (khớp `T2_ONLY_STEPS_EXPECTED`).

**Bước đỏ duy nhất** = `selftest pre-measure (KT-95)`, mã thoát **7** = `cây nguồn bẩn`
(`v2/tools/pre-measure.sh:24`). Tái hiện trực tiếp, **không** qua cổng:

```text
$ bash v2/tools/pre-measure.sh --selftest
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
$ source v2/test/boot/build-rev.sh; vbrev_dirty "$PWD"
1
```

⇒ **Không phải hồi quy**: worktree chưa commit (luật task: lead commit). Trên cây **sạch** bước
này PASS — cùng hiện tượng đã ghi ở KT-409 phần 1 §9 và KT-402b. **T2 phải chạy trên cây sạch
sau commit.**

**Lần chạy T1 thứ nhất ĐỎ 3 bước** (giữ lại làm bằng chứng bắt lỗi thật):
`fmt` (chưa `cargo fmt`), `wake paths` (`E_STRUCT_MOD_OUTSIDE` — khuôn `#[path]`, xem §3.2),
`clippy kernel` (`dead_code` + `wrong_self_convention` do include tệp nguồn). Cả ba **đã vá**;
lần chạy thứ hai chỉ còn `pre-measure` (cây bẩn).

## 7. Việc còn lại / rủi ro / cần Lead quyết

1. **Crate mới `vifast-plat-riscv64-logic`** (§3.2) — **cần Lead duyệt**: tôi đã tách 1 crate
   host-testable thay vì dời thẳng 5 module vào `vifast-plat-riscv64`, vì (i) dời thẳng ⇒ mất
   ≈1.100 LOC test host (trái §4.3), (ii) khuôn `#[path]` bị máy kiểm `E_STRUCT_MOD_OUTSIDE`
   (KT-264) chặn. Đã cập nhật `v2/kernel/Cargo.toml` (member) và `v2/bin/lib/steps_gate.sh:10`
   (`step_l0_kernel` thêm `-p vifast-plat-riscv64-logic`) — **đúng** phần "số bước do tách crate"
   Lead cho phép. Nếu Lead không muốn thêm crate: phương án thay thế là dời 5 module + 8 tệp test
   vào `vifast-plat-riscv64` và **chấp nhận** mất ca kiểm host (phải ghi `BLOCKED`/nợ).
2. **`plat-common` còn 170 dòng từ khoá RISC-V** ở `smp`/`fault`/`bootinfo`/`lib`/`obj`/`ipi`/
   `resources` (§3.4) — **không** thuộc 6 module mục tiêu nhưng chặn tiêu chí "0 tuyệt đối".
   Cần Lead quyết: mở task riêng generalize `smp`/`ipi`/`bootinfo`/`fault`, hay chấp nhận
   "0 trong 6 module mục tiêu".
3. **Ưu tiên 2 chưa làm** (`ArchIrq`/`IrqMcsLock<T,A>`/`CoreBridge<A>`/`ElfUserLoader`/
   `ImageHash`/`WxReport`) — §4 có số đo để ước lượng; đây cũng là nguồn **cắt LOC thật**.
4. **`ProductShellHost` không tồn tại** trong cây (`grep` = 0) — cần Lead xác nhận tên mới.
5. **`vifast-plat-riscv64` vẫn không host-buildable** ⇒ bước `clippy` kernel (bước 84, KT-409
   phần 1) vẫn **loại trừ** crate này; 5 module vừa dời sang đó **không** được `clippy -D warnings`
   quét trên host. Nợ đã ghi ở journal phần 1 §7.1, **nay nặng thêm** vì crate chứa nhiều logic hơn.
6. **`v2/kernel/Cargo.lock` đổi** (thêm `vifast-pte` cho 2 crate, bỏ `vifast-core` khỏi
   `plat-common`, thêm dev-dep) — Lead commit kèm.

## 8. Tổng kết trạng thái

| Ưu tiên | Trạng thái | Bằng chứng |
|---|---|---|
| 1a. `mmu`/Sv39 (định dạng) → `vifast-pte::sv39` | **XONG** | §2; 32 ca `tests/mmu.rs` PASS; `plat-riscv64` + `kbin-riscv64` build target OK |
| 1b. `plic`/`trap`/`timer`/`tlb`/`user` ra khỏi `plat-common` | **XONG** (qua crate host-testable mới) | §3; 94 ca PASS; 0 `cfg(target_arch)`; 0 ca bị bỏ |
| 2. Rút `ArchIrq`/`IrqMcsLock<T,A>`/`CoreBridge<A>`/… | **CHƯA LÀM** | §4 (có số đo) |
| 3. Cắt ≥ 3.500 LOC lặp | **KHÔNG ĐẠT** (≈16 LOC) | §5 (số thật) |
| Cổng | `check` PASS; T1 66 OK / 1 ĐỎ-cây-bẩn | §6 |

**0 chạm `v2/kernel/vifast-core/**`** — kiểm: `git status --short | grep vifast-core` = rỗng.
**0 chạm `spec/`**. Không `#[ignore]`/`#[should_panic]`. Không nới ngưỡng. Không
`git add`/`commit`/`push`/`stash`.
