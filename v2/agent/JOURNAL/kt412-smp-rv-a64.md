# KT-412 — SMP Preemptive 4 hart trên `riscv64` & `aarch64` (CoreBridge + `vifast-ktest`)

- **Nhánh**: `task/kt412-smp-rv-a64` — worktree `/private/tmp/vifast-wt/1`
- **Base**: `main` @ `55bcd851` (đã có KT-402b/408/409/409b/410/411)
- **Ngân sách**: 3 h (bắt đầu `2026-11-26 00:12 JST` / `2026-11-25 15:12 UTC`)
- **Phụ thuộc**: KT-402, KT-402b, KT-411 (D2-259/D2-260)
- **Song song**: KT-413 trên `x86_64` — **0 chạm** `v2/kernel/vifast-plat-x86_64/**`,
  `v2/kernel/vifast-kbin-x86_64/**`.

## 1. Kế hoạch ngắn (5–10 dòng)

1. Đọc luật + QUEUE/KH + D2-259/D2-260/D2-17 + code `machine.rs`/`perhart.rs`/`sched.rs`/
   `core_sched.rs`/`core_bridge.rs` (rv + a64)/`vifast-ktest`.
2. **Đỏ trước**: viết `v2/kernel/vifast-ktest/tests/smp_suite.rs` (host) gọi
   `vifast_ktest::smp_suite::run(...)` — chưa tồn tại ⇒ **không biên dịch** (đỏ đúng lý do).
3. Cài `vifast-ktest/src/smp_suite.rs`: trait hẹp `SmpBridge` (blanket cho `T: CoreBridge`) +
   3 TAP dùng chung: (a) số hart từ `BootInfo` thật qua `Machine::set_harts(n)` + `PerHartState`
   mỗi hart; (b) 4 hart preemptive qua timer: 0 double-pick, 0 starvation; (c) luật đọc
   `published` §4.16 luật 3 (chỉ hart sở hữu được `publish`, đọc chéo không đổi trạng thái).
4. Nối `set_harts(n)` từ `BootInfo` thật vào `core_bridge::init` của **cả hai** arch.
5. Nối bộ ca vào ảnh kiểm: riscv64 TAP **72..74**, aarch64 TAP **45..47** — cùng **một** hàm
   `smp_suite::run` (chỉ khác số TAP đầu).
6. Cập nhật `v2/test/boot/expected.txt` (`ktest 71 71` → `74 74`) + manifest `ktest.sh` +
   `EXPECT_TESTS` của `run-aarch64.sh` (46 → 49).
7. 3 đột biến đỏ vào `v2/test/mutants.toml` (móc `#[cfg(feature = "mut-…")]` trong `smp_suite.rs`).
8. `./v2/bin/v2 t0` sau mỗi bước; cuối: `./v2/bin/v2 check` + **T1** `gate --dev` qua khoá máy.

## 2. Nhật ký

(xem các mục dưới — ghi theo từng bước, kèm log)

### 2.1 Đỏ-trước (D2-05) — `vifast_ktest::smp_suite` chưa tồn tại

Viết `v2/kernel/vifast-ktest/tests/smp_suite.rs` (5 ca host) **trước** khi có mô-đun. Lượt chạy
đầu **không biên dịch được** — đỏ đúng lý do (không phải đỏ do assert):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-ktest --test smp_suite
error[E0432]: unresolved import `vifast_ktest::smp_suite`
  --> vifast-ktest/tests/smp_suite.rs:26:19
   |
26 | use vifast_ktest::smp_suite::{self, SmpBridge, SMP_HARTS};
   |                   ^^^^^^^^^ could not find `smp_suite` in `vifast_ktest`
error: could not compile `vifast-ktest` (test "smp_suite") due to 1 previous error
```

### 2.2 Hiện thực `vifast-ktest/src/smp_suite.rs` — 3 TAP dùng chung

| TAP | Điều kiện đo |
|---|---|
| `smp harts bootinfo` | `Machine::set_harts(n)` với `n` từ **nguồn thật** (`BootInfo::n_harts` / `smpboot`); `hart_count()` = `n`; `published(h)` là **ảnh chụp** của `sched` cho **mọi** hart; `set_harts` idempotent (đo khi mô hình rỗng); `check_sched` xanh |
| `smp 4 hart preemptive` | 4 hart × 2 TCB (8 TCB, mỗi TCB một `SchedContext` có ngân sách, `home` = hart, `affinity` = cả 4); 8 tick, mỗi tick `dispatch(h)` cho **cả 4** hart ⇒ **0 double-pick**, **0 starvation**, `published` khớp `sched` sau **mọi** tick, `check_sched` xanh sau **mọi** tick |
| `smp published rule` | Luật **3** §4.16: chỉ hart **sở hữu** được `publish` (hart khác ⇒ `IllegalOperation`); giá trị công bố sai `idle`/`cur_prio` ⇒ `IllegalOperation`; **đọc** `published` của hart khác **không** đổi trạng thái hart đó; hart ngoài dải ⇒ lỗi |

`SmpBridge` là hợp đồng **hẹp** (`with_machine` + `boot_harts` + `untyped` + móc `set_harts`), không
phải `CoreBridge`: `aarch64` **không** có object layer (`vifast-plat-common::obj`) nên nếu bộ ca đòi
`CoreBridge` thì nó không chạy được trên `aarch64` — trái yêu cầu "chạy giống hệt nhau trên 2 arch".

Host test: **11/11 xanh** (`cargo test -p vifast-ktest`), gồm ca **có răng**
`smp_suite_catches_hart_count_mismatch` (cầu nối bỏ qua `set_harts` ⇒ TAP 1 **ĐỎ**).

### 2.3 `Machine::set_harts(n)` từ `BootInfo` thật (rv + a64)

- `plat-riscv64/src/bootinfo.rs` + `plat-aarch64/src/bootinfo.rs`: thêm `static BOOT_HARTS` +
  `pub fn boot_harts() -> Option<usize>`, ghi trong `detect()` **khi parse thành công**.
- `core_bridge::init` **cả hai arch**: `let harts = bootinfo::boot_harts().unwrap_or(<dự phòng>)`
  rồi `m.init_zeroed(1); m.set_harts(n);` — đúng đường "BootInfo đến **sau** khi dựng `Machine`"
  mà `KT-408` mở. Dự phòng: `smp::hart_count()` (rv, probe SBI HSM) / tham số `harts`
  (`smpboot::online_hart_count()`, a64) cho ảnh gọi core trước `report_bootinfo`.

### 2.4 HAI lỗi thật do bộ ca bắt được trên ảnh (không phải trên host)

**(a) `Machine::set_harts` reset **mọi** hart `0..n_harts`** ⇒ gọi trên mô hình **đang chạy** xoá
ready queue + `current` của hart 0 ⇒ mọi TCB `Ready` mồ côi ⇒ `check_sched` ĐỎ. Đo được trên ảnh
`riscv64` lượt đầu:

```text
# smp: boot_harts=1 model_harts=1 per_hart=1 idempotent=1 check_sched=0
not ok 71 - smp harts bootinfo: so hart tu BootInfo khong khop mo hinh
# smp: harts=4 tcbs=8 ticks=8 double_pick=0 starved=0 sched_bad=8 published_bad=0 …
not ok 72 - smp 4 hart preemptive: double-pick/starvation/bat bien published
```

Vá (`vifast-core/src/machine.rs`, **hunk nhỏ, tách riêng**): chỉ `reset` hart **mới mở**
(`old..n_harts`); hart đã dùng giữ nguyên `PerHartState`. Đây là ngữ nghĩa đúng của "hart **mới**
mở phải ở trạng thái rỗng".

**(b) `Machine::published(h)` không kiểm `n_harts`** ⇒ `h >= n_harts` trả **`Ok`** kèm trạng thái
**cũ** (fail-open đúng trên đường mà luật 3 §4.16 cho phép **đọc**). `publish` đã kiểm `n_harts`
từ trước — nay đọc/ghi đối xứng (`vifast-core/src/core_sched.rs`, **hunk nhỏ, tách riêng**).

### 2.5 Nối vào ảnh kiểm — **cùng một hàm**, hai arch

| Arch | TAP | Tệp |
|---|---|---|
| `riscv64` | **71..73** (trap lồng dời 71 → **74**) | `kbin-riscv64/src/ktest_core.rs::smp_phase` + `ktest.rs` |
| `aarch64` | **47..49** | `kbin-aarch64/src/smpcore.rs` + `pshell.rs::done` |

Harness cập nhật **cùng lúc** (không nới ngưỡng, chỉ thêm ca): `expected.txt` `ktest 71 71` →
`74 74`; manifest `ktest.sh` thêm 3 tên; `run-aarch64.sh` `EXPECT_TESTS` 46 → 49;
`kbin-aarch64/src/main.rs` `TAP_TESTS` 46 → 49 + assert biên dịch `coretest.rs`.

### 2.6 Bằng chứng boot thật (bù cho `ktest.sh` bị `REV-GATE: REFUSE reason=dirty`)

`ktest.sh` từ chối đo trên cây bẩn (KT-86/D2-125.3) — đúng thiết kế, không phải lỗi của task. Vì
vậy boot **trực tiếp** bằng đúng dòng QEMU của runner (`-icount shift=6 -machine
virt,aia=none,aclint=on -bios opensbi… -cpu rv64 -m 128M -smp N -no-reboot`):

```text
$ qemu-system-riscv64 … -smp 1 -kernel …/vifast-kbin-ktest
# smp: boot_harts=1 model_harts=1 per_hart=1 idempotent=1 check_sched=1
ok 71 - smp harts bootinfo
# smp: harts=4 tcbs=8 ticks=8 double_pick=0 starved=0 sched_bad=0 published_bad=0 ran=4,4,4,4,4,4,4,4
ok 72 - smp 4 hart preemptive
# smp: published cross_read=1 owner_only=1 value_guard=1 range_guard=1
ok 73 - smp published rule
# smp core: taps=3 fails=0
ok 74 - trap nested panic
→ 74 dòng `ok`, 0 `not ok`

$ qemu-system-riscv64 … -smp 4 -kernel …/vifast-kbin-ktest
# smp: boot_harts=4 model_harts=4 per_hart=1 idempotent=1 check_sched=1
ok 71 - smp harts bootinfo
# smp: harts=4 tcbs=8 ticks=8 double_pick=0 starved=0 sched_bad=0 published_bad=0 ran=4,4,4,4,4,4,4,4
ok 72 - smp 4 hart preemptive
ok 73 - smp published rule
ok 74 - trap nested panic
→ 74 dòng `ok`, 0 `not ok`  (BootInfo thật cho `n_harts=4` ⇒ `model_harts=4`)
```

### 2.7 Ba đột biến ĐỎ (đã dựng lại **thủ công** bằng cách lật cfg trên cây, rồi khôi phục)

| Đột biến | Móc | Kết quả đo |
|---|---|---|
| `mut-smp-set-harts-reset-all` | `machine.rs::set_harts` reset **mọi** hart | `not ok 72` — `sched_bad=8` |
| `mut-smp-no-refresh-published` | `core_sched.rs` bỏ **mọi** chỗ `refresh_published` | `not ok 72` — `published_bad=32`, `starved=1` |
| `mut-smp-publish-any-hart` | `core_sched.rs::publish` bỏ chốt `current_hart != hart` | `not ok 73` — `owner_only=0` |

Ba mục đã thêm vào `v2/test/mutants.toml` (`kind = "feature"`, `crates = ["vifast-core"]`,
`suite = "ktest-riscv64"`, `expect = "red"`, `wired = true`). **Không** chạy `v2 mutants --all`
(ngoài phạm vi task); cả ba ĐỎ ở `-smp 1` nên suite mặc định của `ktest.sh` đủ giết.

### 2.8 `aarch64` — **cùng** bộ ca, 4 hart thật, 49/49 TAP

`run-aarch64.sh 1` (build ảnh kiểm `--features ktest` + boot `-smp 4` + 28 ca âm):

```text
$ bash v2/test/boot/run-aarch64.sh 1
ok 45 - shell: procsrv serves WAIT from EL0 (product path)
ok 46 - shell: product path help/echo/ps/exit over PL011 (0 marker)
# smp: boot_harts=4 model_harts=4 per_hart=1 idempotent=1 check_sched=1
ok 47 - smp harts bootinfo
# smp: harts=4 tcbs=8 ticks=8 double_pick=0 starved=0 sched_bad=0 published_bad=0 ran=4,4,4,4,4,4,4,4
ok 48 - smp 4 hart preemptive
# smp: published cross_read=1 owner_only=1 value_guard=1 range_guard=1
ok 49 - smp published rule
# smp core: taps=3 fails=0 boot_harts=4
→ 49 dòng `ok`, 0 `not ok`, plan `1..49`
# 39/39 kiểm sạch (boot=1, tests=49)
```

`boot_harts=4` là **số thật** từ `BootInfo::n_harts` (DTB QEMU `-smp 4`), không phải hằng chép tay
— cùng nguồn với dòng `# bootinfo: v=1 arch=aarch64 source=dtb n_harts=4 boot_hart=0` của runner.

### 2.9 T0 / `v2 check`

```text
$ ./v2/bin/v2 t0 --crate vifast-ktest
--- T0: fmt (cargo fmt --all --check)          OK
--- T0: clippy -p vifast-ktest                 OK
--- T0: test -p vifast-ktest                   6 + 5 test xanh (0 đỏ)

$ ./v2/bin/v2 t0 --crate vifast-core --crate vifast-plat-common
--- T0: clippy -p vifast-core / vifast-plat-common   OK
--- T0: test -p vifast-core                     118 passed; 0 failed
--- T0: test -p vifast-plat-common              3 + 32 + 6 + 15 + 7 passed; 0 failed

$ ./v2/bin/v2 check
v2 check: PASS (L1 chạy trong gate --fast)
```

**Nợ T0 đã biết (không phải lỗi của task)**: `./v2/bin/v2 t0` suy crate từ `git diff` nên nó cũng
chạy `cargo test -p vifast-plat-riscv64` **trên host**; crate đó có `global_asm!` RISC-V nên host
build đỏ (`invalid register a7`, `invalid Mach-O section specifier`) — **đã đỏ trước task này**
(crate bare-metal, chỉ build được với `--target riscv64gc-unknown-none-elf`). Vì vậy T0 được chạy
theo **crate host-testable** (`vifast-ktest`, `vifast-core`, `vifast-plat-common`), còn hai crate
arch được kiểm bằng **build đúng target** + boot thật (§2.6/§2.8).

### 2.10 T1 — `gate --dev` (qua khoá máy)

```text
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
# gate-steps registered=80 ran=66 skipped=14 failed=2
# step-time-total   615526 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

**Hai bước đỏ, cả hai đều KHÔNG phải lỗi của task:**

1. `selftest pre-measure (KT-95): FAIL` — `# pre-measure-selftest (a) FAIL: mã thoát 7`.
   Mã **7 = `tree-dirty`**: `pre-measure.sh` từ chối đo trên cây **chưa commit** (KT-86/D2-125.3).
   Đây là **thuộc tính của worktree bẩn**, đúng như `JOURNAL/kt411-ktest-generic.md` §7 đã ghi;
   lead commit rồi chạy T2 trên cây sạch thì bước này xanh.
2. `QEMU x86_64 (K5' bước 1): FAIL` — **`T-INFRA` (flake mạng, không liên quan `x86_64` của
   task này — 0 tệp `x86_64` bị chạm)**:

   ```text
   not ok 1 - ca âm (b2): log đột biến không có '# wx: cr0_wp=0' (đột biến không vào ảnh)
   # net: dns error=BadFrame elapsed_ms=47 (D2-246: mọi reply đều hỏng)
   not ok 33 - net: dns error
   # TAP 33 FAIL — dừng (không in marker)
   ```

   Ảnh đột biến `no-wx` **dừng ở TAP 33** vì DNS reply hỏng (`BadFrame`) nên **không bao giờ tới**
   TAP 35/36 (W^X) — tức ca âm b2 đỏ vì **lý do khác** lý do nó đo. Cùng lúc đó ảnh tốt
   (`boot-1.log`) qua TAP 33 với `bad=0 attempts=1`, và bước này chạy **265 s** (4,4 phút) dưới tải
   QEMU song song ⇒ phân loại **`T-INFRA`** (D2-07/§4.5: ghi log, chạy lại **tối đa một lần** chỉ
   để phân loại, **không** để lấy xanh).

**Chạy lại một lần để phân loại (D2-07, không phải retry lấy xanh)**:

```text
$ bash v2/test/boot/run-x86_64.sh 1
# 39/39 kiểm sạch (boot=1, tests=47, exc=6)
EXIT=0
```

⇒ bước `QEMU x86_64` **xanh** ở lượt chạy lại trên **cùng** cây ⇒ xác nhận **`T-INFRA`** (flake
DNS dưới tải), **không** phải hồi quy từ KT-412. Mọi bước T1 khác đều **OK**, gồm `fmt`, `clippy`,
`luật cấu trúc R1..R5`, `wake paths + --strict`, `wake theo dep-info`, `L0`, `L1 (sim 1.000 seed)`,
`budget`, `workspace v2/kernel: clippy host-testable`.

### 2.11 Việc còn lại / rủi ro

- **T2 (lead)**: cần commit rồi chạy `gate --fast` trên cây **sạch** — hai bước đỏ của T1 ở trên
  đều do cây bẩn/flake, không phải mã.
- **`v2 mutants --all`** chưa chạy (ngoài phạm vi task). Ba mục mới đã `wired = true`; cả ba đã
  được dựng lại **thủ công** và ĐỎ (§2.7). Nếu lead chạy `--all`, cả ba phải `KILLED`.
- **`ktest.sh`** (ma trận đầy đủ 9 ảnh × smp) chưa chạy được trên cây bẩn; bằng chứng boot thật
  trong journal là boot **trực tiếp** bằng đúng dòng QEMU của runner (§2.6/§2.8).
- **Nợ T0**: `v2 t0` suy crate từ `git diff` nên chạy `cargo test -p vifast-plat-riscv64` trên host
  — crate bare-metal đó **đỏ sẵn** trên host (asm RISC-V). Không phải hồi quy của task.
- **`vifast-core` bị chạm 4 hunk nhỏ** (dùng chung với KT-413): `set_harts` (chỉ reset hart mới
  mở), `published` (kiểm `n_harts`), và **ba móc `#[cfg(feature = "mut-smp-…")]`** cho đột biến.
  Không đổi API công khai, không đổi ngưỡng; `cargo test -p vifast-core` **118/118 xanh**.

### 2.12 Chốt trạng thái (bàn giao cho lead)

| Hạng mục | Trạng thái |
|---|---|
| `Machine::set_harts(n)` từ `BootInfo` thật (rv + a64) | **XONG** — `bootinfo::boot_harts()` + `init_zeroed(1)` → `set_harts(n)` |
| `PerHartState` trên `rv` & `a64` | **XONG** — `published`/`sched` kiểm cho **mọi** hart ở TAP 1 |
| 4 hart preemptive qua timer, 0 starvation, 0 double-pick | **XONG** — TAP 2, `ran=4,4,4,4,4,4,4,4`, `double_pick=0`, `starved=0` |
| Luật đọc `published` §4.16 luật 3 | **XONG** — TAP 3 + vá fail-open `published(h >= n_harts)` |
| Bộ ca SMP chung `vifast-ktest`, chạy **giống hệt** 2 arch | **XONG** — `smp_suite::run`, rv TAP 71..73 / a64 TAP 47..49 |
| 3 đột biến đỏ | **XONG** — 3 mục `mutants.toml`, đã dựng lại thủ công và ĐỎ |
| 0 chạm `x86_64` | **XONG** — `git status` 0 tệp `x86_64` |
| T0 | **XANH** (vifast-ktest 11 test, vifast-core 118, plat-common 63) |
| `v2 check` | **PASS** |
| T1 `gate --dev` | **FAIL 2 bước**: `selftest pre-measure` (exit 7 = tree-dirty) + `QEMU x86_64` (**T-INFRA** flake DNS, chạy lại **39/39 sạch**) |
| T2 | **chưa** — lead chạy trên cây sạch sau commit |
