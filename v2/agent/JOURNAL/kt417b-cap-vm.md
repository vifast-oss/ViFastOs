# KT-417b — Hợp nhất 1 Cây Cap/MDB bước 1: `Cap` **56 → 32 B** (−12 KiB) + dời `plat-common::obj` (2.424 LOC) → `vifast-core/src/vm/` + đóng `Q6`

**Nhánh**: `task/kt417b-cap-vm` (worktree `/private/tmp/vifast-wt/1`, base `main` @ `297b89dc`)
**Phạm vi ghi**: `v2/kernel/vifast-core/src/**`, `v2/kernel/vifast-core/Cargo.toml`,
`v2/kernel/vifast-plat-common/src/obj/**`, `v2/kernel/vifast-kbin-riscv64/src/ktest_inv.rs`,
`v2/test/mutants.toml`, 4 tệp ghim cỡ (`v2/test/boot/{ktest,run-aarch64,run-x86_64}.sh`,
`v2/test/boot/lib/budget-common.sh`), **đúng 5 dòng** `source` của `[limits.plat]` trong
`spec/kernel.toml`, và tệp nhật ký này. **0** chạm `spec/abi.toml`, **0** chạm `v2/bin/**`,
**0** chạm `run-*.sh`/`soak-*.sh` ngoài 2 runner QEMU đã được lead phê duyệt riêng (xem §8),
**0** chạm `soak-*.sh`.
**Nguồn sự thật của vòng**: `D2-262` mục 2 (`Q2(c)`) + mục 4 (`Q6`), dòng `KT-417b` trong
`QUEUE.md`, mục W7b của `KE-HOACH-KIEN-TRUC.md`, `D2-84`/`KT-312` (khuôn "vượt pin ⇒ tối ưu
cấu trúc, **không** nâng pin").

---

## 1. Vì sao có task này

`D2-262` từ chối cả hai phương án sai cho bảng VM (Q2(a): global `static` `.corespace` — phá
`Machine: Copy`, khoá `UntypedStore`, chặn đa cụm `S3`; Q2(b): giảm `MAX_MAPPINGS` — suy giảm
năng lực) và chọn **phương án C** đúng khuôn `KT-312`: **tối ưu cấu trúc** `Cap` `56 → 32 B`
để lấy lại 12 KiB trước khi bảng VM thành kiểu liên kết hạng nhất của `KernelStore` ở `KT-417d`.
Vì sao `Cap` phình tới 56 B: ba trường `Option` **không có niche** trong khi mọi trường đều
`pub(crate)` (thay đổi nội bộ, 0 đổi ABI):

| Trường cũ | Cỡ | Trường mới | Cỡ |
|---|---:|---|---:|
| `parent: Option<CapId>` | 8 | `parent_raw: u32` (`0 == None`, `CapId` 1-based) | 4 |
| `slot: Option<SlotRef>` | **24** (`cnode` 8 + `slot` 8 + cờ 8) | `slot_cnode: u32` + `slot_idx: u16` | 6 |
| `badge: Option<u64>` | **16** | `badge_val: u64` + bit `CAP_HAS_BADGE` | 8 + bit |
| `alive: bool` | 1 | bit `CAP_ALIVE` trong `flags: u8` | bit |

## 2. Ca ĐỎ-TRƯỚC (bắt buộc, có log)

Thêm `footprint::tests::kt417b_cap_dong_goi_32_b` **trước** khi sửa mã, chạy trên mã cũ:

```
# kt417b cap: Cap=56 caps[512]=28672 Machine=184712 (truoc: 56 / 28672 / 184712)
thread 'footprint::tests::kt417b_cap_dong_goi_32_b' panicked at vifast-core/src/footprint.rs:326:9:
assertion `left == right` failed: KT-417b: size_of::<Cap>() phải 32 B (đo 56)
  left: 56
 right: 32
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 137 filtered out
```
**Log**: `v2/agent/JOURNAL/kt417b-cap-vm/red-first.log` (sao y `cargo test ... --lib kt417b`).

Ca đỏ-trước **thứ hai** (bắt buộc bởi §4.5 AGENTS — "đỏ trước" cho từng thay đổi hành vi):
T1 trên **cây bẩn** đỏ ở 2 bước *hạ tầng* `pre-measure --selftest` (`exit 7 tree-dirty`) và
`api boundary` ("tệp sẽ chạm đã bẩn") — bằng chứng đó chứng minh hai bước này **thật sự** đọc
trạng thái git, và chúng xanh sau khi commit (§6). **Log**: `kt417b-cap-vm/t1-gate-dev.log`.

## 3. Việc đã làm

### 3.1 `Cap` 56 → 32 B (`vifast-core/src/objmeta.rs`)

`pub(crate)` **giữ nguyên mọi trường**, `Copy` 100 %, **0** `unsafe`, **0** global `static`;
thêm hai hằng bit `CAP_ALIVE`/`CAP_HAS_BADGE` và **cùng** API ngữ nghĩa:
`parent()/set_parent()`, `slot()/set_slot()`, `badge()/set_badge()`, `alive()/set_alive()`
— **một** nguồn sự thật cho ý nghĩa `Some`/`None` (không chỗ nào tự so `== 0`).
`const _: () = assert!(crate::machine::MAX_SLOT_WORDS <= u16::MAX as usize);` chốt miền giá trị
của `slot_idx` **ngay khi biên dịch** (kể cả ảnh bare-metal).

19 tệp gọi (`captable.rs`, `check.rs`, `check_ipc.rs`, `core_capxfer.rs`, `core_finalise.rs`,
`core_foreign.rs`, `core_ipc.rs`, `core_sync.rs`, `machine/{init,lifecycle}.rs`,
`machine_slot.rs`, `mdb.rs`, `invoke.rs`, + 5 tệp test) đổi sang API ngữ nghĩa — **0** đổi hành
vi (139/139 test host xanh, gồm toàn bộ `capalloc_tests`/`invoke417_tests`).

### 3.2 Pin + ca biên (`footprint.rs`)

- `kt417b_cap_dong_goi_32_b`: `size_of::<Cap>() == 32`, `caps[512] == 16.384`,
  `Machine ≤ 184.832 − 12.288` (trần KT-312 **không** nới, thu hồi phải **thật**).
- `kt417b_cap_ca_bien_slot_va_badge`: `slot = MAX_SLOT_WORDS − 1 = 4095` (đúng `u16`, không
  tràn), `slot 0` là giá trị **hợp lệ** (khác `None`), `badge = Some(0)` **khác** `None`,
  `badge = Some(u64::MAX)` không mất bit, `parent_raw = 0 ⇔ None` + `Some(1)`/`Some(u32::MAX)`,
  `alive` **độc lập** với `has_badge` (xoá bit này không đụng bit kia).
- Cập nhật đại số có dấu `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb` với số hạng mới
  `−24 B × MAX_CAPS` (KT-417b) ⇒ `khop=1`.

### 3.3 Dời object layer → `vifast-core/src/vm/` (bước 1/2 của `Q2(c)`)

- `git mv` `v2/kernel/vifast-plat-common/src/obj/{mod,vspace,check,arena}.rs` (2.424 LOC)
  → `v2/kernel/vifast-core/src/vm/`; `lib.rs` thêm `pub mod vm;`.
- `vifast-core/Cargo.toml` thêm **2 phụ thuộc thuần acyclic**: `vifast-pte` + `vifast-alloc`
  (cả hai `no_std`, **0** dependency ngoài ⇒ D2-21; **không** phụ thuộc `vifast-core` ⇒ không
  vòng lặp, giữ chiều `D2-83` plat → core). `#![forbid(unsafe_code)]` **giữ nguyên**:
  object layer vốn 100 % an toàn (mọi truy cập bộ nhớ qua trait `PhysMem`).
- `vifast-plat-common/src/obj/mod.rs` còn **shim mỏng** `pub use vifast_core::vm::*;`
  ⇒ **33 caller** (`v2/kernel/**`, `v2/sim/**`, `tests/obj.rs`) **0 phải sửa**, hành vi 0 đổi.
  `KT-417d` (W8a) mới là bước **xoá** shim + hợp nhất bảng VM vào `KernelStore` + `ArchVmOps`.
- **Đúng 5 dòng** `source` của `[limits.plat]` trong `spec/kernel.toml` đổi sang
  `v2/kernel/vifast-core/src/vm/mod.rs#MAX_*`. Vì sao **bắt buộc**: `check_kernel.py` đòi
  **khai báo `const` + biểu thức số ngay trong tệp `source`** (không nhận `pub use` của shim)
  ⇒ không sửa thì `kernel-check` ĐỎ (`E_SOURCE_*`/không thấy hằng), tức cổng fail-closed chứ
  không phải "nới ngưỡng". Đây là **cửa sổ `spec/` TẠM THỜI có phê duyệt** (`D2-262` mục 2 +
  DoD `QUEUE` + lead duyệt trực tiếp), **`KT-417d` sẽ xoá cả `[limits.plat]`**; `spec/abi.toml`
  **0** chạm (đã kiểm `check_abi.py` PASS).

### 3.4 Đóng `Q6`

- `ktest_inv.rs::ok18` (ảnh QEMU riscv64, pha invocation `KT-67a`): thêm **assert** đủ 4 delta
  `mutated == 0 && rotated == 0 && bound == 0 && unbound == 0` (trước đây chỉ ghi chú; D6
  reviewer KT-74 cùng khuôn). Chỉ **siết**, không nới; `denied == 3 && allowed == 3 &&
  set_tls == 0 && sys_err(0) == 18` giữ nguyên.
- `v2/test/mutants.toml`: suite host **mới** `core-invoke417-host`
  (`cargo test -p vifast-core --lib invoke417`) + **2 đột biến**:
  `mut-cnode-mutate-rotate-noop` (bỏ `Machine::{mutate,rotate}`) và
  `mut-notif-bind-unbind-noop` (bỏ `Machine::{tcb_bind_notification,tcb_unbind_notification}`),
  mỗi móc `#[cfg(feature = "…")]` đặt **ngay tại** điểm dispatch `invoke::cnode`/
  `invoke::{bind,unbind}_notification` ⇒ `declarations` 53 → **55**.
  Móc **chỉ** bỏ hiệu ứng, **vẫn** đếm `mutated`/`rotated`/`bound`/`unbound` — nhờ vậy đột biến
  không thể "sống" nhờ ca chỉ kiểm bộ đếm.

## 4. Số đo (máy sinh — chỉ chép từ log)

Lệnh: `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core --lib -- --nocapture footprint::tests tls_tests::pin`
**Log**: `v2/agent/JOURNAL/kt417b-cap-vm/footprint.log`

```
# kt405 abi: TcbCore=280 EndpointCore=48 ScData=64 NotificationData=32 ReplyData=8 Meta=48 \
             Machine=172424 TcbData=824 TcbModel=544 IpcArea=544 EndpointData=64 \
             EndpointModel=16 MachineNoop=159160 TestObserver=13264
# kt417b cap: Cap=32 caps[512]=16384 Machine=172424 (truoc: 56 / 28672 / 184712)
# tls pin: size_of_machine=172424 pin_cu=188872 delta=-16448 size_of_tcbdata=824 \
           size_of_message=816 max_tcbs=64 8xmax=512 kt282=1536 kt309_tcb=512 \
           kt309_msg=3072 kt309_scalar=16 kt311_tcb=512 kt312_giam=22608 tinh=-16448 \
           khop=1 duoi_pin=1
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 132 filtered out
```

| Hạng mục | Trước | Sau | Δ |
|---|---:|---:|---:|
| `size_of::<Cap>()` | **56 B** | **32 B** | **−24 B** |
| `caps[512]` (`MAX_CAPS`) | **28.672 B** | **16.384 B** | **−12.288 B** (12,0 KiB) |
| `size_of::<Machine>()` | **184.712 B** | **172.424 B** | **−12.288 B** (đúng đại số, `khop=1`) |
| trần KT-312 `184.832` / pin D2-84 `196.608` | — | **không nới** | headroom 12.408 B dưới trần |
| `size_of::<Meta>()` | 48 B | 48 B | 0 (KT-312 giữ) |
| `Footprint::scalars` / `parts` | 311 / 26 | **311 / 26** | 0 (pin `kt60_core_parity` phía sim giữ) |

`UntypedStore` (`GENERAL_MAX_CAPS = 1.024`) thu hồi `24 × 1.024 = 24.576 B` (**24,0 KiB**) như
`D2-262` dự phóng.

## 5. Bất biến đã giữ

- **0** `unsafe` mới (D2-17); **0** global `static` (D2-262 cấm cho bảng VM); **100 %** `Copy`.
- **0** đổi `spec/abi.toml`; `spec/kernel.toml` **đúng 5 dòng** `source=`; **0** nới trần/pin
  (`184.832`, `196.608`, `KERNEL_STATIC_BUDGET`, `image_cap=262144` đều nguyên).
- **0** `#[ignore]`/`#[should_panic]`/tắt test; **0** sửa `accept()`/kỳ vọng/snapshot harness để
  test xanh — 4 tệp ghim cỡ chỉ đổi **chữ số của giá trị đo mới** (xem §7).
- **R1** ≤ 1.500 dòng/tệp: tệp lớn nhất mới `vifast-core/src/vm/mod.rs` **1.268** dòng
  (`1.259` + 9 dòng đầu mục KT-417b) — dưới trần.
- `vifast-core` LOC sản phẩm: 17.128 → **19.552** (dưới mục tiêu 25.000 `[ƯL]`, `D2-262` mục 3).

## 6. Cổng

- **T0** (trong lúc sửa):
  - `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core --lib` ⇒ **139 passed, 0 failed**
    (gồm 2 ca mới `kt417b_*`, `kt312_*`, `kt405_*`, `kt408_*`, `invoke417_tests`, `capalloc_tests`).
  - `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common` ⇒ **8 suite / 76 ca
    passed, 0 failed** (gồm `tests/obj.rs` **32** ca chạy qua shim ⇒ shim giữ đúng ngữ nghĩa).
  - `cargo clippy -p vifast-core -p vifast-plat-common --all-targets -- -D warnings` ⇒ **sạch**.
  - `cargo check -p vifast-core --target riscv64gc-unknown-none-elf` ⇒ OK;
    `cargo check -p vifast-kbin-riscv64 --bin vifast-kbin-ktest --features suite-user
    --target riscv64gc-unknown-none-elf` (chứa `ktest_inv.rs` mới) ⇒ OK.
  - `check_kernel.py` ⇒ **PASS 54 giá trị**; `check_abi.py` ⇒ **PASS**; `vifast-gen --check` ⇒ PASS.
  - `mutants --lint` ⇒ `mutants=36 feature=35 declarations=55 covered=55 leftover=0 suites=12
    problems=0`.
  - **Hai đột biến mới KILLED (tự chứng minh trước khi chạy `mutants --all`)**:
    `RUSTFLAGS='--cfg feature="mut-cnode-mutate-rotate-noop"'` ⇒ `invoke417_tests`
    `{mutate_doi_quyen_va_badge, mutate_kiem_quyen_va_arity, rotate_hoan_vi_hai_slot}` **FAILED**;
    `RUSTFLAGS='--cfg feature="mut-notif-bind-unbind-noop"'` ⇒
    `{bind_unbind_notification, bind_busy_khi_notification_da_gan_tcb_khac,
    notification_chet_thi_go_lien_ket_tcb}` **FAILED**. (Không chạy `mutants --all` — việc của
    Lead ở giờ Cao điểm, `D2-256`.)
- **T1** `./v2/bin/v2 gate --dev`: xem §9 (bảng kết quả cuối).

## 7. Tệp ghim cỡ đã hạ theo giá trị đo mới (lead phê duyệt riêng — "re-pin **chặt hơn**")

| Tệp | Thay đổi | Vì sao bắt buộc |
|---|---|---|
| `v2/test/boot/ktest.sh` (4 dòng) | `bytes/static=184712` → **`172424`** | regex ghim **đúng** cỡ ảnh, sai số ⇒ T1 đỏ |
| `v2/test/boot/run-aarch64.sh` (2) | `size=184712` → **`172424`** | như trên (ảnh aarch64) |
| `v2/test/boot/run-x86_64.sh` (2) | `size=184712` → **`172424`** | như trên (ảnh x86_64) |
| `v2/test/boot/lib/budget-common.sh` (2 + 1 nhãn) | `budget_common_MACHINE=172424`; ca biên gate `.corespace` **172424 nhận / 172423, 0 từ chối**; nhãn in `≥172.424` | hằng pin + ca biên phải theo cỡ mới; nhãn cũ ghi "≥184.792" trong khi test 184712 — **sai sẵn**, nay đúng |

Điều kiện lead đặt đã giữ: vẫn so **khớp chính xác** (không đổi sang `[0-9]+`), giữ điều kiện
`in_image=1`/`healthy=1`, `pin=196608` + `image_cap=262144` + trần `184.832` **không** đụng.

`grep -rn "184712\|184_712" v2/ --include=*.sh --include=*.py --include=*.rs | grep -v soak-`
sau khi sửa chỉ còn **2** dòng **số lịch sử cố ý** (không phải ghim sống):
`vifast-core/src/footprint.rs:324` (dòng in "truoc: 56 / 28672 / 184712") và
`vifast-core/src/kt408_tests.rs:9` (doc "pin harness 184712 → 172424 ở KT-417b").
`soak-*.sh` **có** `184712`/`184792` trong **fixture log tổng hợp** của `--selftest`
(`soak-x86_64.sh:803`, `soak-aarch64.sh:846`) — chủ sở hữu là **slot 3 (`KT-408e`)**, pattern
thật của soak dùng `size=[0-9]+` nên **không** đỏ; **không** chạm (đúng ranh giới 0 % chồng tệp).

## 8. Nợ / rủi ro (cho Agent 0 & Lead)

1. **Cửa sổ `spec/kernel.toml` (5 dòng) là TẠM** — `KT-417d` (W8a) **phải xoá cả
   `[limits.plat]`** cùng `check_kernel.py:277`; nếu không, `spec` sẽ trỏ vào module `vm` đã bị
   hợp nhất vào `KernelStore`. Đây là lý do duy nhất task chạm `spec/` và **không** phải ABI.
2. **Shim `plat-common::obj`** (`pub use vifast_core::vm::*`) chỉ để 33 caller khỏi sửa; phải
   **xoá** ở `KT-417d` (nếu để lâu sẽ thành nguồn sự thật thứ hai — đúng thứ `Q2` cấm).
3. `.corespace` NOLOAD **tự co** theo `Machine`: đo trên ảnh ktest ⇒ **180.744 B** (trước là
   **184.792 B**), vẫn `≥ Machine 172.424` nên gate `.corespace ≥ size_of::<Machine>()` xanh
   (smoke T1: `corespace=180744`/`8320` tuỳ ảnh). **Không** sửa linker/pin `.corespace` ở task
   này (đó là pin của `KT-401`/`KT-339a`); nếu ai đó siết gate thành **bằng** thì phải cập nhật
   cùng lúc.
4. Fixture log tổng hợp trong `soak-*.sh` còn số cũ (xem §7) — **vô hại** cho selftest (fixture
   và pattern cùng tệp), nhưng là số liệu lịch sử; chủ sở hữu: slot 3.
5. `v2/Cargo.lock` + `v2/kernel/Cargo.lock` đổi vì `vifast-core` nhận 2 path-dep mới
   (`check-lockfile-sync` PASS cả 3 lockfile).
6. **Giới hạn quyền**: sub-agent **không** được commit (hook `vifast-guard` fail-closed khi
   `VIFAST_ROLE` rỗng) ⇒ thay đổi đã **stage** và **Lead** chạy commit (§9). T1 **chỉ** xanh
   được trên **cây sạch** (`pre-measure` `exit 7 tree-dirty`, `api boundary` từ chối tệp bẩn).

## 9. Cổng T1 `./v2/bin/v2 gate --dev`

- **T1 lượt 1** (cây **bẩn**, trước commit `0a247faa`): **FAIL** — **chỉ** 2 bước do cây bẩn,
  không liên quan mã: `selftest pre-measure (KT-95)` `exit 7 tree-dirty`
  (`vbrev_dirty` quét build-inputs) và `api boundary (KT-241)` "tệp sẽ chạm đã bẩn — từ chối
  ghi đè (fail-closed): `M v2/kernel/vifast-core/src/lib.rs`". **Mọi** bước khác OK.
  Log: `v2/agent/JOURNAL/kt417b-cap-vm/t1-gate-dev.log` (597.108 ms).
- **T1 lượt 2** (cây **sạch** @ `0a247faa`, không đổi một byte mã): **PASS** —
  `v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate =
  'gate --fast')`, `# gate-steps registered=82 ran=68 skipped=14 failed=0`,
  `# step-time-total 902247 ms TỔNG gate --dev (T1)`.
  Log: `v2/agent/JOURNAL/kt417b-cap-vm/t1-clean.log`.
  - 2 bước từng đỏ ở lượt 1 nay **xanh**: `pre-measure-selftest: PASS (a/b/c/d/e/f/g)`,
    `api boundary (KT-241): OK` (B1.1 ratchet XANH, `dep_edges=3 → 0 mới`, `driver_files=23 → 0 mới`).
  - `L0 test: OK`; `L0 kernel workspace (KT-148): OK` (gồm 139 test `vifast-core` + 16 `vifast-ktest`);
    `L1 OK: 1000 seed, 999 trace khác nhau`; `spec/kernel.toml ↔ mã (D2-242 nhóm 1)` OK (54 giá trị).
  - **QEMU smoke (7 ảnh × 1 boot): OK** — `expected[ktest] plan=74 ok=74 ten-TAP=74`,
    `guard 4/4`, `user 38/38` (chứa `ktest_inv` + `ok 18 - invocation ngoai danh sach` của `Q6`),
    `proc 10/10`, `vblk 6/6`, `con 5/5`, `exec 6/6`, `ipc 11/11`, `netsrv 3/3`;
    ảnh `ktest` `data+bss=55192 ≤ 262144`, `corespace=180744 ≥ Machine 172424`.
- **Không retry lấy xanh (D2-07)**: hai lượt chạy trên **hai trạng thái cây khác nhau**
  (bẩn ⇒ fail-closed ở 2 bước git; sạch ⇒ PASS); mã/khoá đầu vào **không** đổi giữa hai lượt
  (`git rev-parse HEAD` = `0a247faa` ở lượt 2, nội dung 35 tệp như nhau).
- **T2 (`gate --fast`) KHÔNG chạy** — việc của Lead ở giờ Cao điểm (D2-256), đúng ranh giới task.

