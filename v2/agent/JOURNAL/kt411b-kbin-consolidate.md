# KT-411b — Gom `kbin-riscv64` 14 `[[bin]]` → 2 mode `product`/`ktest`

Nhánh `task/kt411b-kbin-consolidate`, worktree `/private/tmp/vifast-wt/1`, base `main` @ `fe79074d`.
Ngân sách 3 h. Phạm vi: (1) gom 14 `[[bin]]` → 2 mode; (2) giữ đủ độ phủ matrix & parity;
(3) `< 16.000` LOC nếu làm được. **KHÔNG** đổi `expected.txt`/số TAP (AGENTS §4.2).

## 0. Kế hoạch ngắn (5–10 dòng)

1. **Đỏ-trước**: `/tmp/kt411b-red.sh` khẳng định trạng thái đích — `Cargo.toml` có **đúng 2**
   `[[bin]]`, **13** feature `suite-*`, và **một** bin `vifast-kbin-ktest` dựng được **13 ảnh**
   (`suite-base` … `suite-gpu`). Chạy trên base ⇒ ĐỎ (14 bin, 0 suite).
2. `src/ktest.rs` → **dispatcher** (`rust_main`/`rust_secondary_main`/`#[panic_handler]` **một bản**);
   thân hiện tại dời sang `src/ktest_base.rs` (`pub fn run() -> !`).
3. 12 tệp `ktest_*.rs` còn lại: bỏ `#![no_std]`/`#![no_main]`, bỏ `#[no_mangle]` +
   `rust_secondary_main` + `#[panic_handler]` + `static PANIC`, `rust_main` → `pub fn run() -> !`;
   **hoist** `mod` dùng chung về crate root (một chỗ, có `#[cfg(feature = "suite-*")]`).
4. `Cargo.toml`: 14 `[[bin]]` → **2**; thêm 13 feature `suite-*` (mỗi suite một nhãn, giữ nguyên
   bộ feature cũ của nó).
5. 8 tệp harness: `--bin vifast-kbin-ktest --features ktest,suite-<x>` rồi
   `cp $IMG_DIR/vifast-kbin-ktest $IMG_DIR/vifast-kbin-ktest-<label>` — **giữ nguyên tên ảnh** nên
   `image_path`/`expected.txt`/`budget.sh` không đổi một dòng.
6. Sau mỗi bước: `./v2/bin/v2 t0`. Cuối: `./v2/bin/v2 check` + T1 qua khoá máy.
7. Đo lại LOC + số `[[bin]]`; ghi rõ phần `< 16.000` LOC nếu không đạt (kèm số đo).

## 1. Đo hiện trạng (trước khi sửa)

```text
$ grep -c '^\[\[bin\]\]' v2/kernel/vifast-kbin-riscv64/Cargo.toml   → 14
$ cat v2/kernel/vifast-kbin-riscv64/src/*.rs | wc -l                → 27392
$ grep -c '^suite-' v2/kernel/vifast-kbin-riscv64/Cargo.toml        → 0
```

## 2. Đỏ-trước (bằng chứng, D2-05)

`/tmp/kt411b-red.sh` (kịch bản trạng thái đích; **không** nằm trong repo — chỉ là công cụ đo):

```text
$ bash /tmp/kt411b-red.sh
not ok - số [[bin]]: đo [14] muốn [2]
not ok - số feature suite-*: đo [0] muốn [13]
not ok - không còn bin ktest-* riêng: đo [12] muốn [0]
ok - bin ktest duy nhất (1)
not ok - build suite-base (xem /tmp/kt411b-build-base.log)
… 13/13 suite đỏ …
# kt411b-red: 16 lỗi
$ head -1 /tmp/kt411b-build-base.log
error: the package 'vifast-kbin-riscv64' does not contain this feature: suite-base
```

Đỏ **đúng lý do**: chưa có mode `ktest` gom suite — 13 ảnh kiểm vẫn là 13 crate root riêng.

## 3. Việc đã làm

### 3.1 Nguồn — 14 crate root → 1 dispatcher + 13 module

| Tệp | Trước | Sau |
|---|---:|---:|
| `src/ktest.rs` | 1.062 (crate root của ảnh `ktest`) | **242** — dispatcher duy nhất |
| `src/ktest_base.rs` | — | **1.040** — thân ảnh `ktest` cũ (`pub fn run() -> !`) |
| 12 `src/ktest_*.rs` | mỗi tệp một crate root | module, `pub fn run() -> !` |

Biến đổi **cơ học, không đổi ngữ nghĩa** (`/tmp/kt411b-transform.py`):
bỏ `#![no_std]`/`#![no_main]` (chỉ hợp lệ ở crate root), bỏ `#[no_mangle]` trước `rust_main`,
`rust_main` → `pub fn run`, bỏ `rust_secondary_main` + `#[panic_handler]` + `static PANIC` +
`PanicHandler` khỏi câu `use` (một bản duy nhất ở dispatcher).

**Hoist `mod` về crate root** (một chỗ, có `#[cfg(feature = "suite-*")]`): 11 module của suite
`base` (`ktest_conf`, `ktest_core`, `ktest_core64`, `ktest_fault`, `ktest_fs`, `ktest_ipi`,
`ktest_lock`, `ktest_mem`, `ktest_obj`, `ktest_smp`, `ktest_tlb`), 5 của `user` (`ktest_inv`,
`ktest_capx`, `ktest_capa`, `ktest_ring`, `ktest_vfs`), 2 của `proc` (`proc_abi`, `proc_demo`),
1 của `netsrv` (`net_svc`). Vì sao hoist: các tệp đó gọi nhau bằng **đường dẫn tuyệt đối**
`crate::ktest_ipi::…`, `crate::ktest_smp::…`, `crate::proc_abi::…` — hoist giữ nguyên các đường
dẫn đó (đo được: 0 dòng `crate::` phải sửa trong 5 tệp).

`#[path = "<tên>.rs"]` trên mọi `mod` hoist: module nạp bằng `#[path]` phân giải module con theo
**thư mục của tệp** (đã kiểm bằng thí nghiệm rustc riêng), nên `ktest_vfs` vẫn thấy
`ktest_vfs_once`/`ktest_vfs_rig` và `ktest_ipc` vẫn thấy `rig` như trước.

Hai chỗ **không** hoist (giữ nguyên quan hệ cha–con, chỉ đổi `crate::` → `super::`):
`ktest_ipc_rig.rs` (`rig` là module con của `ktest_ipc`; `use crate::*` → `use super::*`,
`crate::tcb_state_name`/`crate::stop_name`/`crate::user::` → `super::…`) và
`ktest_vfs_once.rs`/`ktest_vfs_rig.rs` (module con của `ktest_vfs`).

`ktest_smp.rs`: `#[no_mangle] pub extern "C" fn rust_secondary_main` → `pub fn secondary_main`
(dispatcher gọi cho suite `base`; 12 suite còn lại park như cũ).

### 3.2 `Cargo.toml` — 14 `[[bin]]` → 2, thêm 13 feature `suite-*`

```toml
[[bin]] name = "vifast-kbin-riscv64"  path = "src/main.rs"          # ảnh sản phẩm (không đổi)
[[bin]] name = "vifast-kbin-ktest"    path = "src/ktest.rs"  required-features = ["ktest"]

suite-base = ["ktest"]              suite-guard = ["ktest"]
suite-user = ["ktest", "user"]      suite-proc = ["ktest", "user"]
suite-vblk = ["ktest", "vblk"]      suite-persist = ["ktest", "vblk"]
suite-con = ["ktest", "user", "con"]  suite-exec = ["ktest", "user", "exec"]
suite-ipc = ["ktest", "user", "ipc"]  suite-net = ["ktest", "net"]
suite-netsrv = ["ktest", "user", "netsrv"]  suite-input = ["ktest", "input"]
suite-gpu = ["ktest", "gpu"]
```

Vì sao cần nhãn riêng: `ktest-user`/`ktest-proc` dùng **chung** `ktest,user` và
`ktest-vblk`/`ktest-persist` dùng **chung** `ktest,vblk` ⇒ không phân biệt được bằng feature sẵn có.
Dispatcher kiểm **đúng một** `suite-*` bằng `const _` (`assert!(n == 1)`) — bật 0 hoặc ≥ 2 là lỗi
biên dịch, không có ảnh lai.

### 3.3 Harness — 8 tệp

| Tệp | Đổi |
|---|---|
| `v2/test/boot/ktest.sh` | 8 khối build (14 bin) → `build_suite` + 9 lời gọi (`--bin vifast-kbin-ktest --features ktest,suite-<x>` rồi `cp` ra tên ảnh cũ); `image_path ktest` → `vifast-kbin-ktest-base` |
| `v2/test/boot/guard_selftest.sh` | ảnh base → `vifast-kbin-ktest-base` |
| `v2/test/boot/input.sh` | `--bin vifast-kbin-ktest --features ktest,suite-input` + `cp` |
| `v2/test/boot/gpu.sh` | `… suite-gpu` + `cp` |
| `v2/test/boot/net.sh` | `… suite-net` + `cp`; `… suite-netsrv` + `cp` |
| `v2/test/boot/persist.sh` | `… suite-persist` + `cp` |
| `v2/test/parity/run.sh` | **không đổi** — chỉ đọc `vifast-kbin-ktest-proc` (do `ktest.sh` dựng) |
| `v2/test/conformance/run.sh` | ảnh base → `vifast-kbin-ktest-base` |

**Vì sao ảnh base đổi tên tệp** (`vifast-kbin-ktest` → `vifast-kbin-ktest-base`): `vifast-kbin-ktest`
nay là **tên tệp build dùng chung** của cả 13 suite nên bị ghi đè liên tục. `guard_selftest.sh`
(làn `sau2`) và `net.sh`/`input.sh`/`gpu.sh` (làn `sau5`/`sau6`) chạy **song song** trong T2 ⇒ để
chung tên thì `guard_selftest.sh` có thể boot nhầm ảnh suite khác hoặc tệp đang ghi dở. Chỉ **3**
chỗ đọc tên đó (`ktest.sh:image_path`, `guard_selftest.sh`, `conformance/run.sh`) — đã sửa cả 3.
`expected.txt` khoá theo **nhãn** (`ktest`), không theo đường dẫn ⇒ **không đổi một dòng**.

## 4. Bằng chứng XANH sau khi cài

### 4.1 Kịch bản đỏ-trước chuyển xanh

```text
$ bash /tmp/kt411b-red.sh
ok - số [[bin]] (2)
ok - số feature suite-* (13)
ok - không còn bin ktest-* riêng (0)
ok - bin ktest duy nhất (1)
ok - build suite-base … ok - build suite-gpu      (13/13)
# kt411b-red: 0 lỗi
```

### 4.2 Boot **thật** 9 ảnh của `ktest.sh` + chấm bằng **chính** validator của harness

Cây bẩn ⇒ `REV-GATE` (KT-86/D2-125.3) **từ chối đo** (đúng như journal KT-411 §7), nên boot tay
từng ảnh rồi để `ktest.sh --check-log <nhãn> <log> <smp>` chấm — **không** nới cổng nào, dùng đúng
`expected.txt` + manifest evidence + mẫu PANIC của harness.

```text
$ SMP=1 bash /tmp/kt411b-boot.sh ktest ktest-guard
ok - ktest smp=1 (74 TAP)
ok - ktest-guard smp=1 (3 TAP)
$ SMP=1 bash /tmp/kt411b-boot.sh ktest-user ktest-proc ktest-vblk ktest-exec ktest-ipc ktest-netsrv
ok - ktest-user smp=1 (37 TAP)
ok - ktest-proc smp=1 (10 TAP)
ok - ktest-vblk smp=1 (6 TAP)
ok - ktest-exec smp=1 (6 TAP)
ok - ktest-ipc smp=1 (11 TAP)
ok - ktest-netsrv smp=1 (3 TAP)
$ SMP=1 bash /tmp/kt411b-boot.sh ktest-con          # kèm virtio-console + chardev pipe
# con host: tx_file=/tmp/kt411b-boot/con-smp1.out ok=1
ok - ktest-con smp=1 (5 TAP)
$ SMP=2 bash /tmp/kt411b-boot.sh ktest
ok - ktest smp=2 (74 TAP)
$ SMP=4 bash /tmp/kt411b-boot.sh ktest
ok - ktest smp=4 (74 TAP)
```

`ktest` ở **smp=2/4** là bằng chứng cho đường **mới** `rust_secondary_main` → `ktest_smp::secondary_main`
(chuỗi pha SMP/barrier/timer/PLIC/IPI/TLB của suite `base`).

### 4.3 `expected.txt` không đổi

```text
$ bash v2/test/boot/ktest.sh --print-expected
# ktest       74      74     74
# ktest-guard 3       3      3
# ktest-user  37      37     37
# ktest-proc  10      10     10
# ktest-vblk  6       6      6
# ktest-con   5       5      5
# ktest-exec  6       6      6
# ktest-ipc   11      11     11
# ktest-netsrv 3      3      3
$ bash v2/test/boot/ktest.sh --selftest-stack
# selftest-stack: 5/5 ca đúng như kỳ vọng
```

## 5. Số cuối

| Hạng mục | Trước | Sau | Δ |
|---|---:|---:|---:|
| `[[bin]]` của `vifast-kbin-riscv64` | **14** | **2** | **−12** |
| feature `suite-*` | 0 | 13 | +13 |
| `src/*.rs` (tổng LOC) | 27.392 | **27.384** | **−8** |
| `src/ktest.rs` | 1.062 | 242 | −820 |
| `src/ktest_base.rs` (mới) | 0 | 1.040 | +1.040 |
| 12 tệp suite (tổng) | 26.330 | 26.102 | −228 |
| tệp harness phải sửa | — | 7 sửa + 1 mới (`lib/ktest-build.sh`); `parity/run.sh` **không** đổi | — |

**`< 16.000` LOC: KHÔNG ĐẠT — 27.384.** Gom bin **không** làm giảm LOC (nó chỉ gom **crate root**:
−228 dòng lặp `#![no_std]`/`rust_main`/`rust_secondary_main`/`#[panic_handler]`, +242 dòng
dispatcher). 11.384 dòng còn lại là **thân bộ ca + rig U-mode riêng của từng ảnh**
(`ktest_ipc` 1.388, `ktest_vfs` 1.332, `ktest_user` 1.223, `ktest_persist` 1.133, `ktest_net` 1.108,
`ktest_netsrv` 1.058, `ktest_tlb` 1.061, `ktest_capa` 1.050, …). Cắt chúng đòi hỏi bộ trait
`Platform` **đầy đủ** (map page, nạp ELF, vào U-mode, PCI/MMIO) — nhiều task, **không** phải 3 h,
và nếu cắt bằng cách bỏ ca thì **mất độ phủ matrix** (trái yêu cầu 2 của task). Đây là cùng kết
luận đã ghi ở journal KT-411 §9.1, nay **đo lại** sau khi gom bin.

## 6. Việc còn lại / rủi ro / cần lead quyết

| # | Việc | Trạng thái |
|---|---|---|
| 1 | **Hợp nhất bộ ca a64/x64** về `vifast-ktest` | **DỪNG đúng luật**: `coretest.rs` của a64 phát TAP **36..41**, x64 phát **38..47**, riscv64 **43..52** ⇒ hợp nhất là **đổi nhãn TAP** ⇒ đổi `expected.txt` (AGENTS §4.2 cấm). **Cần lead quyết**: cho phép cập nhật `expected.txt` một lần (có kiểm) hay giữ 3 bộ. |
| 2 | `< 16.000` LOC | **Không đạt** (27.384) — lý do đo được ở §5. |
| 3 | T2 (`gate --fast`) | **Lead chạy** trên cây đã commit. Cây bẩn ⇒ `REV-GATE` từ chối, nên bằng chứng boot ở §4.2 là boot tay + validator của harness. |
| 4 | `v2/test/mutants.toml` | **Không sửa**: `mut-ktest-driver` vá `[features]` bằng `find = "[features]"` ⇒ vẫn khớp (chèn ngay sau dòng đó). Suite `ktest-riscv64` chạy `bash v2/test/boot/ktest.sh` ⇒ vẫn đi qua đường mới. |
| 5 | Ảnh base đổi **tên tệp** | `vifast-kbin-ktest` → `vifast-kbin-ktest-base` (§3.3). Nhãn `ktest` trong `expected.txt` **không đổi**; lead nên biết vì `ls target/release` sẽ thấy tên mới. |
| 6 | Cảnh báo `dead_code`/`unused` trong crate | Có sẵn từ trước (crate bị `clippy` loại trừ ở `step_clippy_kernel`); **không** thêm cảnh báo mới nào ở tệp `ktest_*.rs` (đã đối chiếu từng dòng `--> vifast-kbin-riscv64/src/…`). |

## 7. T0 / `check` / T1

### 7.1 T0 — `vifast-kbin-riscv64` **không** lint được trên host (có sẵn từ trước, không do task này)

```text
$ ./v2/bin/v2 t0
error: invalid register `t3`: unknown register   (vifast-plat-riscv64/src/lib.rs:867)
… 31 lỗi …
--- T0: test -p vifast-kbin-riscv64: FAIL
v2 t0: FAIL (1 crate; **T0** — KHÔNG phải cổng)
```

Nguyên nhân **đo được**: `t0` suy crate bị chạm = `vifast-kbin-riscv64` rồi chạy
`cargo clippy -p … --all-targets` cho **target host**; crate này là ảnh bare-metal nên kéo theo
`vifast-plat-riscv64` (asm `t3`/`t4`/`t6` chỉ có trên riscv64). Chứng minh **không** do task này:

```text
$ cargo clippy -p vifast-plat-riscv64 --manifest-path v2/kernel/Cargo.toml 2>&1 | grep -c '^error'
32
$ git status --short v2/kernel/vifast-plat-riscv64/     → (rỗng, không sửa một dòng)
```

T0 **thay thế** cho phần đã chạm (đúng thứ mà T0 định kiểm: biên dịch + hành vi):
13/13 suite biên dịch cho `riscv64gc-unknown-none-elf` (§4.1) + 9 ảnh boot thật và được
`ktest.sh --check-log` chấm xanh (§4.2) + `cargo fmt --all --check` của workspace kernel **OK**.

### 7.2 `./v2/bin/v2 check` — PASS

```text
$ ./v2/bin/v2 check
--- workspace v2/kernel: clippy host-testable (KT-409): OK
--- workspace v2/user: clippy 3 arch + host test (KT-404): OK
--- selftest helper runner dùng chung (KT-407): OK
# step-time-total   129685 ms  TỔNG check
EXIT=0
```

### 7.3 T1 — `gate --dev` (kết quả nguyên văn)

```text
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
# t1-lock: giữ khoá (pid 18866) — chạy: ./v2/bin/v2 gate --dev
…
--- selftest pre-measure (KT-95)
# pre-measure-selftest (a) FAIL: mã thoát 7 (mong 0 = OK)
# pre-measure-selftest: FAIL
--- selftest pre-measure (KT-95): FAIL
…
# step-time-total   479362 ms  TỔNG gate --dev (T1)
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
# t1-lock: nhả khoá (rc=1)
```

**65 bước OK, 1 bước FAIL, 14 bước BỎ QUA (T1 — chỉ chạy ở T2), 479 s.**

Bước đỏ **duy nhất** là `selftest pre-measure (KT-95)`, mã thoát **7** = *"cây nguồn bẩn"*
(`v2/tools/pre-measure.sh` §dòng 24: `7 = cây nguồn bẩn`). Đây là **thuộc tính của worktree đang
sửa**, không phải của thay đổi: cùng lớp với `REV-GATE: REFUSE reason=dirty` ở §4.2 — lead commit
rồi chạy T2 trên cây sạch thì bước này xanh. **Không** nới ngưỡng, **không** bỏ bước, **không**
retry để lấy xanh (lượt chạy lại duy nhất ở trên là vì lượt trước bị **khoá máy đo** của một T2
khác chặn ngay từ đầu — `T1: có cổng/soak KHÁC đang giữ khoá máy đo ⇒ nhường ngay`, 0 bước nào chạy).

## 8. Đua **mới** do gom bin, và cách chặn (phát hiện trong lúc tự kiểm)

Gom 14 bin → 1 làm **mọi** suite dựng ra **cùng một tên tệp**
`target/<target>/release/vifast-kbin-ktest` rồi `cp` ra tên ảnh riêng. Trong T2, các làn
`sau` (`ktest.sh`), `sau5` (`net.sh`), `sau6` (`input.sh`/`gpu.sh`) chạy **song song** và cùng ghi
tệp đó ⇒ có khoảng đua giữa "cargo trả về" và "`cp`": làn khác kịp dựng suite của nó thì `cp` của
làn này chép **nhầm ảnh**. Trước KT-411b **không** có đua vì mỗi suite một tên bin riêng. Máy đo
đang `loadavg_1m ≈ 11` (12 core, 4 slot) nên khoảng đua là **thật**, không phải lý thuyết.

Cách chặn — `v2/test/boot/lib/ktest-build.sh` (mới, 5 runner cùng source):

| Hàm | Việc |
|---|---|
| `ktest_build_lock` / `ktest_build_unlock` | khoá `mkdir` (bash 3.2/macOS **không** có `flock`), trần chờ **900 s**; hết hạn ⇒ **ĐỎ**, không treo |
| `ktest_publish` | `cp` **nguyên tử**: ghi `*.tmp.$$` cùng thư mục rồi `mv` (`rename(2)` cùng FS) ⇒ người đọc song song (`guard_selftest.sh`, `conformance/run.sh`) **không** thấy tệp ghi dở |
| `ktest_build_suite` | gói **build + phát hành trong cùng một khoá** |

Không đổi: bộ feature từng suite, tên ảnh phát ra, `expected.txt`, `budget.sh`, ngưỡng nào.
Không mất song song: `cargo` vốn đã tuần tự hoá trên cùng `target/` (khoá riêng của nó), nên khoá
ngoài không làm chậm thêm.

Kiểm lại sau khi thêm khoá (ảnh **cùng byte**, chỉ đổi cách phát hành):

```text
$ bash v2/test/boot/input.sh   → # input.sh: 2 kiểm đạt, 0 lỗi
$ bash v2/test/boot/gpu.sh     → # gpu.sh: 2 kiểm đạt, 0 lỗi
$ bash v2/test/boot/net.sh     → # net.sh: 4 kiểm đạt, 0 lỗi
$ bash v2/test/boot/ktest.sh 1 1 → 9/9 ảnh build + budget + ktest-strings OK, dừng ở REV-GATE (cây bẩn)
$ SMP=1 bash /tmp/kt411b-boot.sh ktest ktest-guard ktest-ipc
ok - ktest smp=1 (74 TAP) / ok - ktest-guard smp=1 (3 TAP) / ok - ktest-ipc smp=1 (11 TAP)
```

## 9. Tự kiểm cuối

```text
$ cargo fmt --all --check --manifest-path v2/kernel/Cargo.toml   → OK
$ ./v2/bin/v2 rules                                              → rules: PASS (v2)
$ ./v2/bin/v2 check                                              → PASS (exit 0, TỔNG check 77,0 s)
$ bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev             → FAIL **chỉ** ở
                                                                   `selftest pre-measure (KT-95)`
                                                                   (exit 7 = cây nguồn bẩn, §7.3)
$ grep -c '^\[\[bin\]\]' v2/kernel/vifast-kbin-riscv64/Cargo.toml → 2
$ grep -c '^suite-'      v2/kernel/vifast-kbin-riscv64/Cargo.toml → 13
$ cat v2/kernel/vifast-kbin-riscv64/src/*.rs | wc -l              → 27384
```

**Không** chạy: T2 (`gate --fast`), `nightly`, `soak`, `l1-tier medium/full`, `v2 mutants --all`
(đúng giới hạn task). **Không** `git add/commit/push/merge/cherry-pick/stash`. **Không** sửa
`spec/`, `v2/agent/{STATE,QUEUE,DECISIONS,BLOCKED}.md`, `reports/STATUS.md`. **Không** `pkill -f
qemu-system`; mọi QEMU đều do script của task này tạo và đã dừng.

### 9.1 Danh sách tệp (để lead stage)

```text
 M v2/kernel/vifast-kbin-riscv64/Cargo.toml
 M v2/kernel/vifast-kbin-riscv64/src/ktest.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_con.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_exec.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_gpu.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_guard.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_input.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_ipc_rig.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_net.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_netsrv.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_proc.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_smp.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_user.rs
 M v2/kernel/vifast-kbin-riscv64/src/ktest_vblk.rs
 M v2/test/boot/gpu.sh
 M v2/test/boot/guard_selftest.sh
 M v2/test/boot/input.sh
 M v2/test/boot/ktest.sh
 M v2/test/boot/net.sh
 M v2/test/boot/persist.sh
 M v2/test/conformance/run.sh
?? v2/agent/JOURNAL/kt411b-kbin-consolidate.md
?? v2/kernel/vifast-kbin-riscv64/src/ktest_base.rs
?? v2/test/boot/lib/ktest-build.sh
```

`git diff --stat`: **23 tệp sửa, 316 dòng thêm, 1.451 dòng bớt** (+ 3 tệp mới).
