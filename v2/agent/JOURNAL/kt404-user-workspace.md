# KT-404 — Workspace Cargo thật `v2/user/` (`B4`) + Crate host `vifast-kbuild` + Đóng `N-7` P20 + 9 crate `forbid(unsafe_code)`

**Nhánh**: `task/kt404-user-workspace` (worktree `/private/tmp/vifast-wt/1`, base `main` @ `19f0a4c3`)
**Ngân sách**: 3 h (bắt đầu `2026-11-25 19:04 JST` = `10:04 UTC`, hạn `22:04 JST` = `13:04 UTC`)
**Không commit** (Lead commit). Chỉ tạo **một** tệp mới trong `v2/agent/`: chính journal này.

---

## 0. Kế hoạch ngắn (5–10 dòng)

1. **Đỏ trước (P1)**: cài một vi phạm `clippy` **có thật** vào `v2/user/programs/` rồi chạy `./v2/bin/v2 check`
   ⇒ **PASS** (chứng minh lỗ hổng: `v2/user/**` không nằm trong bất kỳ workspace cargo nào nên
   `clippy -D warnings` **không bao giờ** chạy trên nó). Ghi log.
2. **P1**: tạo `v2/user/Cargo.toml` (workspace độc lập) + 3 package `vifast-urt` (shim ABI,
   `[lib] name = "vifast_user_abi"`), `vifast-rt`, `vifast-programs` (`[[bin]]` cho từng chương trình).
   Không dời tệp nguồn (dùng `[lib] path`/`[[bin]] path`) ⇒ 0 vỡ tham chiếu của 3 `build.rs`,
   `kt347-u1a.sh`, `steps_check.sh`.
3. **P1**: thêm bước `83` vào `v2/bin/lib/steps_check.sh` (`step_user_workspace`): `cargo clippy`
   cho **3 target bare-metal** (`riscv64gc/aarch64/x86_64-unknown-none`) + 8 ca host của lõi bump;
   tăng `CHECK_STEPS_EXPECTED` 47 → 48.
4. **P1**: chạy lại `v2 check` ⇒ **ĐỎ** đúng vi phạm đã cài (bước mới có răng), gỡ vi phạm ⇒ xanh.
5. **P2**: crate host `v2/kernel/vifast-kbuild` (`#![forbid(unsafe_code)]`, 0 dep ngoài) gom
   `build_identity` + `remap_flags` + `compile_rlib`/`compile_user_elf`/`write_linker_script`;
   3 `build.rs` chỉ còn khai báo danh sách artefact.
6. **P3**: điều tra `N-7` (BUILD_ID = FNV-1a `.text` lệch xuyên thư mục) — đo 2 thư mục cùng commit.
7. **P4**: `#![forbid(unsafe_code)]` cho 9 crate 0-`unsafe`.
8. `./v2/bin/v2 check` + **T1** qua khoá máy (`bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev`).

---

## 1. Bối cảnh đo được trước khi sửa

| Sự kiện | Bằng chứng |
|---|---|
| `v2/user/` **không** có `Cargo.toml` | `ls v2/user/` → `README.md programs rt runtime` |
| `v2/Cargo.toml` `exclude = ["kernel"]`, members **không** có `user` | `v2/Cargo.toml` |
| `v2/kernel/Cargo.toml` members **không** có `user` | `v2/kernel/Cargo.toml` |
| 3 `build.rs` = `547 + 857 + 492 = 1.896` LOC | `find v2 -name build.rs \| xargs wc -l` |
| `build_identity` lặp **3 lần** (~230 LOC/lần) | `diff` 3 khối: chỉ khác **3 dòng** (tên crate arch + `v2/user` trong pathspec `git status`) |
| 9 crate 0-`unsafe` **chưa** có `forbid(unsafe_code)` | quét `v2/lib/*/src/{lib,main}.rs`, `v2/sim/*`, `v2/tools/*`, `v2/kernel/*/src/main.rs` |

---

## 2. ĐỎ-TRƯỚC (P1) — lỗ hổng `clippy` không phủ `v2/user/`

### 2.1 Đỏ #1 — workspace không tồn tại

```text
$ cargo clippy --manifest-path v2/user/Cargo.toml
error: manifest path `v2/user/Cargo.toml` does not exist
```

### 2.2 Đỏ #2 — cổng **PASS** dù `v2/user/` có vi phạm `clippy` thật

Cài **cố ý** một vi phạm `clippy::identity_op` (lint warn-by-default, chắc chắn bị `-D warnings`
bắt) vào `v2/user/programs/hello.rs`:

```rust
    init_ipc_buffer();
    // KT-404 ĐỎ-TRƯỚC: vi phạm clippy::identity_op cố ý — phải bị bắt sau khi nối workspace.
    let _zero = 0u32 + 0;
```

Chạy cổng **trước** khi sửa gì:

```text
$ ./v2/bin/v2 check
...
# step-time-total    96559 ms  TỔNG check
$ echo $?
0
```

⇒ **`v2 check` PASS (rc=0) trong khi `v2/user/programs/hello.rs` có vi phạm `clippy` hiển nhiên.**
Đây đúng là lỗ hổng `KE-HOACH-KIEN-TRUC.md` nút thắt 3 mô tả: *"không chạy `clippy`/`cargo test`
dù `runtime/lib.rs` có `unsafe`"*.

### 2.3 Xanh có răng — sau khi thêm bước 83

```text
$ ./v2/bin/v2 check
--- workspace v2/user: clippy 3 arch + host test (KT-404)
error: this operation has no effect
  --> programs/hello.rs:23:17
23 |     let _zero = 0u32 + 0;
   |                 ^^^^^^^^ help: consider reducing it to: `0`
   = note: `-D clippy::identity-op` implied by `-D warnings`
--- workspace v2/user: clippy 3 arch + host test (KT-404): FAIL
v2 check: 2 bước FAIL
$ echo $?
1
```

Gỡ vi phạm ⇒ `hello.rs` khôi phục **nguyên vẹn** (`diff` với bản sao lưu: rỗng).

### 2.4 Bước mới bắt được **4 cảnh báo `-D warnings` có sẵn** trong `v2/user/` (đã sửa)

| # | Tệp:dòng | Lint | Sửa |
|---|---|---|---|
| 1 | `v2/user/rt/lib.rs:106` | `clippy::write_with_newline` | `write!(…"{} \n")` → `writeln!(…"{} ")` — **cùng chuỗi ra** |
| 2 | `v2/user/programs/procsrv.rs:24` | `unused_imports` (`SYS_EXIT`) | bỏ khỏi danh sách `use` |
| 3 | `v2/user/programs/shell.rs:48` | `unused_imports` (`SYS_EXIT`) | bỏ khỏi danh sách `use` |
| 4 | `v2/user/programs/shell.rs:693` | `clippy::unnecessary_cast` | `put_hex(VA_CON_RING as u64)` → `put_hex(VA_CON_RING)` |

Cả 4 sửa **không đổi hành vi** (chuỗi in ra giống, import không dùng, cast vô nghĩa).

---

## 3. P1 — `v2/user/` là workspace Cargo thật

### 3.1 Cấu trúc (0 dời tệp nguồn)

| Tệp mới | Nội dung |
|---|---|
| `v2/user/Cargo.toml` | `[workspace] members = ["runtime","rt","programs"]` + `[workspace.package]` + `[profile.{dev,release}] panic="abort" opt-level=1 debug=0` (trùng cờ `$RUSTC` của 3 `build.rs`) |
| `v2/user/runtime/Cargo.toml` | package **`vifast-urt`**, `[lib] name = "vifast_user_abi"`, `path = "lib.rs"` |
| `v2/user/rt/Cargo.toml` | package **`vifast-rt`**, `[lib] name = "vifast_rt"`, `path = "lib.rs"`, dep `vifast-urt` |
| `v2/user/programs/Cargo.toml` | package **`vifast-programs`**, **6 `[[bin]]`** (`shell`, `procsrv`, `echo`, `hello`, `netsrv_prod`, `u1a`) trỏ `path = "<tên>.rs"` |
| `v2/user/Cargo.lock` | sinh bởi cargo; **0 gói ngoài** (chỉ 5 package nội bộ) |

**Vì sao `[lib] name` giữ nguyên `vifast_user_abi`/`vifast_rt`**: ba `build.rs` truyền
`--extern vifast_user_abi=<rlib>` và mọi chương trình có `extern crate vifast_user_abi;`. Đổi tên
crate ⇒ đổi symbol đã mangle ⇒ đổi byte ELF ⇒ đổi ảnh. `[lib] path`/`[[bin]] path` trỏ thẳng tệp
nguồn hiện có nên **0 dời tệp**, 0 vỡ `build.rs`/`kt347-u1a.sh`/`steps_check.sh`.

**`[lints.rust] unexpected_cfgs`** khai báo **một chỗ cho mỗi package** (`rt_alloc`, `shell_fs`,
`shell_net`, `shell_exec`, `product_net`, `mut_*`, `test`) — nếu không, `-D warnings` biến
`unexpected_cfgs` thành lỗi. (Đặt ở `[workspace.lints]` thì cargo cảnh báo
`missing_lints_inheritance` cho hai path-dep `vifast-ring`/`vifast-virtio` thuộc workspace khác ⇒
đặt ở từng package, 0 cảnh báo.)

### 3.2 Bước `83` trong `v2/bin/lib/steps_check.sh`

```bash
step_user_workspace() {
    local user_manifest="$V2_ROOT/user/Cargo.toml" t
    local target_dir="${CARGO_TARGET_DIR:-$V2_ROOT/target}/user-workspace"
    local flags="--cfg rt_alloc --cfg shell_fs --cfg shell_net --cfg shell_exec --cfg product_net"
    for t in riscv64gc-unknown-none-elf aarch64-unknown-none x86_64-unknown-none; do
        RUSTFLAGS="$flags" CARGO_TARGET_DIR="$target_dir" \
            "$CARGO" clippy --manifest-path "$user_manifest" --workspace --target "$t" \
            -- -D warnings || return 1
    done
    local rustc_bin
    rustc_bin="$(dirname "$CARGO")/rustc"
    [ -x "$rustc_bin" ] || rustc_bin="$(command -v rustc)"
    "$rustc_bin" --edition 2021 --test "$V2_ROOT/user/runtime/bump.rs" \
        -o "$target_dir/bump-tests" || return 1
    "$target_dir/bump-tests" || return 1
}
```

* Lint **cả 3 target bare-metal** với đúng bộ `--cfg` của **ảnh sản phẩm** ⇒ lint đúng biến thể
  chạy thật (kể cả `shell_cmds.rs`/`shell_fs.rs`/`shell_net.rs` chỉ tồn tại khi `shell_fs`/
  `shell_net` bật), không lint bản test.
* `CARGO_TARGET_DIR` **riêng** (`<target>/user-workspace`) ⇒ không đụng target của workspace host.
* Làn `main` (bước này **ghi** target dir) — cùng làn với `clippy`/`wake dep-info`.
* `CHECK_STEPS_EXPECTED` 47 → **48**; `gate --fast` 78 → **79** bước.
* `step_fmt` đổi vòng `rustfmt --check` liệt kê tay ba thư mục thành
  `cargo fmt --all --check --manifest-path v2/user/Cargo.toml` (phủ đúng tập tệp của workspace).

**Vì sao 8 ca host của lõi bump chạy bằng `rustc --test` chứ không `cargo test`**: `bump` chỉ được
biên dịch trong lib khi `--cfg rt_alloc`, nên một test target của cargo phải `#[path]` trỏ vào
`../bump.rs` — và `check-wake-paths.py` (luật `E_STRICT_WAKE_INCLUDE`, KT-171 B6/B7) **fail-closed**
với mọi `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim`:

```text
E E_STRICT_WAKE_INCLUDE   v2/user/runtime/tests/bump.rs:12: `#[path]` trỏ ra ngoài
                          `v2/kernel`+`v2/sim` (`../bump.rs`) — không quét được đích (fail-closed)
```

Đã thử `v2/user/runtime/tests/bump.rs` + `#[path]` ⇒ **ĐỎ** đúng luật đó ⇒ gỡ tệp test, dùng
**đúng lệnh** của `v2/test/boot/kt347-u1a.sh` (một nguồn ca, không chép lại). Kết quả:

```text
running 8 tests
test bump::tests::align_khong_hop_le_fail_closed ... ok
... (8/8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 4. P2 — crate host `v2/kernel/vifast-kbuild`

### 4.1 Số dòng (đo bằng `wc -l`)

| Tệp | Trước | Sau |
|---|---:|---:|
| `vifast-kbin-riscv64/build.rs` | 857 | **406** |
| `vifast-kbin-aarch64/build.rs` | 547 | **161** |
| `vifast-kbin-x86_64/build.rs` | 492 | **125** |
| **tổng 3 `build.rs`** | **1.896** | **692** (−63,5 %) |
| `vifast-kbuild/src/lib.rs` | — | 43 |
| `vifast-kbuild/src/identity.rs` | — | 267 |
| `vifast-kbuild/src/compile.rs` | — | 257 |
| **tổng crate mới** | — | **567** |

Ba bản `build_identity` (~230 LOC × 3) + `remap_flags` (30 × 3) + `compile_rlib` (70 × 3) +
`compile_user_elf` (80 × 3) + `write_linker_script` (25 × 2) ≈ **1.100 LOC lặp** ⇒ **567 LOC một
bản**. Net **−637 LOC** toàn cây.

**Chưa đạt `~450`** như dòng QUEUE: phần còn lại của `riscv64/build.rs` là **danh sách artefact**
(~150 LOC có chú thích nguồn gốc từng ELF) + **danh sách `rerun-if-changed`** (~55 dòng, gồm cả
`mod` con và tệp `.S` của `global_asm!`). Nén hai danh sách này thành bảng `const` sẽ đạt ~450
nhưng **mất chú thích lịch sử** của từng artefact và không giảm rủi ro thật ⇒ **cố ý không nén**;
ghi lại đây để Lead quyết.

### 4.2 API (một bản, ba arch dùng)

| Hàm | Thay cho |
|---|---|
| `identity::build_identity(manifest_dir, arch_crate)` | 3 bản `build_identity` + `git_*` + `identity_watch_paths` + `is_algo`/`is_inputs16`/`normalize_rev` |
| `compile::remap_flags()` | 3 bản `remap_flags` |
| `compile::compile_rlib(target, crate_name, source, externs, extra)` | `build_rlib`/`build_rlib_at`/`build_rlib_extra`/`compile_rlib` |
| `compile::compile_user_elf(target, cwd, source, out_name, script, externs, extra, link_args)` | `build_user_elf*`/`compile_user_elf` |
| `compile::write_linker_script(path, base, &LinkerScript{…})` | 2 bản `write_linker_script` (khác nhau đúng 3 chỗ: `OUTPUT_ARCH`, `.rodata` thêm, `/DISCARD/`) |
| `compile::export_user_elf(name)` | `export_user_elf` |

**Bug thật đã đóng nhờ gộp**: bản `riscv64` tính `dirty` trên `v2/kernel v2/lib` **thiếu
`v2/user`**, trong khi `v2/user` **nằm trong** `VBREV_INPUT_ROOTS` của `build-rev.sh` (KT-346) ⇒
sửa `v2/user` làm dấu vân tay `inputs` đổi nhưng ảnh riscv64 vẫn khai `dirty=0` (fail-open). Từ
`KT-404` cả 3 arch dùng **một** `DIRTY_SCOPE = ["v2/kernel","v2/lib","v2/user"]` — **siết**, không nới.

### 4.3 `vifast-kbuild` **không** là thành viên workspace bare-metal

`cargo check --workspace --target riscv64gc-unknown-none-elf` (bước `wake theo dep-info`, KT-217)
cố build **mọi** member cho riscv64 ⇒ crate host dùng `std` sẽ ĐỎ:

```text
E E_DEPINFO_BUILD   đơn vị `kernel`: cargo check RC=101 (build ĐỎ …)
```

Cách cargo diễn đạt "host-only helper": `exclude = ["vifast-kbuild"]` trong `v2/kernel/Cargo.toml`
+ `[workspace]` riêng trong `vifast-kbuild/Cargo.toml` (giá trị `version`/`edition`/`rust-version`/
`license` viết thẳng, không `*.workspace = true`). Build-dependency **luôn** được biên dịch cho
host nên 3 `vifast-kbin-*` vẫn dùng được. Sau khi sửa:

```text
$ cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s
```

### 4.4 Ba arch build xanh (ảnh thật, không chỉ `check`)

```text
riscv64 product      : Finished `release` in 9.81s   (KT-89 … inputs=b090cfd83fe82db3 dirty=1)
riscv64 ktest,user   : Finished `release` in 21.05s  (KT-404 init.elf 12648 B … 20 ELF)
riscv64 product-all  : Finished `release`            (shell.elf 32928 B — nhánh shell_fs+net+exec)
aarch64 ktest        : Finished `release` in 17.35s  (procsrv 16736 / shell 13752 / echo 13000)
x86_64  ktest        : Finished `release` in 14.06s  (shell 13800 / procsrv 16736)
```

---

## 5. P3 — `N-7` (P20): `.text` digest lệch xuyên thư mục

### 5.1 Tái hiện (2 thư mục, **cùng nội dung**, `dirty=0`)

Hai bản sao y hệt nội dung cây làm việc, mỗi bản một repo git riêng, **cùng tree OID**
`f03f592a7edf8987f73fee361e57dfc9ea95e5ab`:

```text
/tmp/n7a  inputs=c2c848f83878ca41 dirty=0 rev=19a88b66f64d
/tmp/n7b  inputs=c2c848f83878ca41 dirty=0 rev=9d2911ba850d
```

Build **đường thường** (`cargo build --release`, không cờ repro):

| | `/tmp/n7a` | `/tmp/n7b` |
|---|---|---|
| `inputs` | `c2c848f83878ca41` | `c2c848f83878ca41` ✅ **khớp** |
| `.text` **len** | 60768 | 60768 ✅ khớp |
| `.text` **digest** (D2-80) | `0225a8dd3b4a9cb2` | `a090597d30f9f687` ❌ **lệch** |
| `sha256` ảnh | `35336d70…` | `0f45ab3a…` ❌ lệch |

⇒ **`inputs` ĐÃ khớp xuyên thư mục** (nửa đầu `N-7` đóng sẵn từ KT-346/D2-127); cái lệch là
`.text` digest.

### 5.2 Cô lập nguyên nhân (3 phép đo)

| Phép đo | Kết quả | Kết luận |
|---|---|---|
| Cùng **thư mục nguồn** `/tmp/n7a`, **hai `CARGO_TARGET_DIR`** khác nhau | `build=0225a8dd3b4a9cb2` **×2** | `.text` **không** phụ thuộc target dir |
| Hai thư mục nguồn, `RUSTFLAGS="-C metadata=0123456789abcdef"` | `351a2325346fb931` vs `5dc4f7af69f18b31` | vẫn lệch ⇒ **không** phải `-C metadata` của cargo (cargo ghi đè `-C metadata` sau `RUSTFLAGS`) |
| Hai thư mục nguồn, build qua **`v2/tools/repro-build.sh`** | xem §5.3 | **khớp tuyệt đối** |

So `.symtab` hai ảnh: **572 tên symbol chỉ có ở A, 572 chỉ có ở B** — khác nhau ở **hash
disambiguator** (`Cslypea8pne9v_18vifast_plat_common` vs `CsgH8kOqLwrqV_18vifast_plat_common`).
`.strtab` 67160 vs 67166 B. Địa chỉ symbol lệch **không đều** (−80, +16, −32, −6, +12, −8, −4,
−24, +2, +4 …) ⇒ **thứ tự phát sinh hàm trong `.text` đổi**, không phải dịch đều.

### 5.3 Nguyên nhân gốc (đo được) & cách đóng

`v2/tools/repro-build.sh` đã ghi đúng hai nguồn trong header của nó, và **đường build thường
không truyền cả hai**:

1. **`--remap-path-prefix=<gốc-cây>=/vifast`** — crate **ngoài** workspace (`v2/lib/*`) được cargo
   truyền đường dẫn **tuyệt đối**; `panic!`/bounds-check nhúng nguyên văn đường dẫn vào `.rodata`
   ⇒ `.rodata` đổi **cỡ** theo độ dài chỗ checkout ⇒ mọi tham chiếu `auipc/addi` tới `.rodata`
   trong `.text` đổi theo.
2. **`-C strip=symbols`** — metadata hash của crate ngoài workspace phụ thuộc đường dẫn tuyệt đối
   ⇒ tên symbol mang hash đường dẫn ⇒ `.strtab` đổi; strip cắt hẳn nguồn đó.

Build **qua `repro-build.sh`** trên đúng hai thư mục đó:

```text
/tmp/n7a: REPRO sha256=5fd4fa608b2a27f3137da916eb04d3ec45dadf101bc4e0dbc7f3609693446fb8 size=95232 build=b48c39d4b751fc04 text=60768 inputs=c2c848f83878ca41 dirty=0
/tmp/n7b: REPRO sha256=5fd4fa608b2a27f3137da916eb04d3ec45dadf101bc4e0dbc7f3609693446fb8 size=95232 build=b48c39d4b751fc04 text=60768 inputs=c2c848f83878ca41 dirty=0
```

⇒ **`sha256`, `.text` digest (`build=`), `inputs`, `size` KHỚP TUYỆT ĐỐI** xuyên hai thư mục khác
tên. `rev` khác (`19a88b66f64d` vs `9d2911ba850d`) là **đúng**: hai commit khác nhau cùng một tree.

**Kết luận `N-7`**: `.text` digest **không** phải bất biến của nội dung nguồn — nó là bất biến của
*(nội dung nguồn + cờ build)*. Đường **canonical** (`repro-build.sh`, dùng bởi `repro-check.sh` và
ảnh phát hành) **đã** cho danh tính khớp xuyên thư mục; đường build **thường** (`run.sh`/`ktest.sh`)
thì không, và **không thể** làm khớp trên cargo stable vì `--remap-path-prefix` phải mang **đường
dẫn tuyệt đối của chính cây đang build** (không đặt được trong `.cargo/config.toml` tĩnh) và
`-Zremap-cwd-prefix` là nightly. Vì vậy:

* **Danh tính xuyên thư mục = `inputs`** (đã khớp, đã là thứ `vbrev_gate` dùng) — **không** dùng
  `BUILD_ID`/`sha256` làm danh tính xuyên thư mục (đúng `AGENTS.md` §6.3 nợ P20).
* **`BUILD_ID` = danh tính của chính artefact đó** (neo ảnh ↔ log của cùng một lượt đo) — đúng
  cách `run.sh`/`soak*.sh` đang dùng.
* Bằng chứng 3 arch: xem §5.4.

### 5.4 `repro-check.sh` 3 arch (2 thư mục ngắn/dài, mỗi arch 2 build lạnh)

(xem §7 — kết quả chạy)

---

## 6. P4 — `#![forbid(unsafe_code)]` cho 9 crate 0-`unsafe`

| # | Crate | Ghi chú |
|---:|---|---|
| 1 | `vifast-abi` | hằng ABI sinh từ `spec/` |
| 2 | `vifast-idl` | stub sinh từ `.vidl` |
| 3 | `vifast-ring` | ring 1P/1C Acquire/Release |
| 4 | `vifast-traceformat` | dòng trace chuẩn hoá |
| 5 | `vifast-fsutil` | binary host seed đĩa FS |
| 6 | `vifast-sim` | oracle mô phỏng |
| 7 | `vifast-agentctl` | CLI quy trình (`lib.rs` + `main.rs`) |
| 8 | `vifast-rt` | runtime `no_std + alloc` U-mode (`v2/user/rt/`) |
| 9 | `vifast-kbuild` | crate host mới của task này |

**Vì sao KHÔNG bật cho 3 `vifast-kbin-*`** (dù chúng 0 `unsafe`): `#[no_mangle]` **là thuộc tính
`unsafe`** dưới `forbid(unsafe_code)` — đo được:

```text
error: usage of the unsafe `no_mangle` attribute
 --> t.rs:2:3
  |
2 | #[unsafe(no_mangle)]
  |   ^^^^^^^^^^^^^^^^^
```

(`#[unsafe(no_mangle)]` cũng bị bắt.) Ba crate ảnh nào cũng có `#[no_mangle] pub extern "C" fn
_start` ⇒ **không thể** `forbid` mà không `#[allow(unsafe_code)]` cục bộ (đúng thứ `forbid` sinh ra
để chặn). Ghi lại để Lead quyết: muốn phủ 3 crate ảnh thì phải đổi luật R5/D2-17 sang
`deny(unsafe_code)` + `#[allow]` có kiểm soát (khuôn `vifast-plat-x86_64` của D2-201).

---

## 7. Cổng

### 7.1 `./v2/bin/v2 check` (T0, trên worktree bẩn)

```text
$ ./v2/bin/v2 check
--- fmt: OK
--- clippy: OK
--- workspace v2/user: clippy 3 arch + host test (KT-404): OK
...
--- api boundary (KT-241): FAIL
v2 check: 1 bước FAIL
```

`api boundary` **ĐỎ trên cây bẩn là ĐÚNG thiết kế** — nó tự từ chối khi tệp nó sẽ ghi đã bẩn:

```text
check-api-boundary: TỪ CHỐI — tệp sẽ chạm đã bẩn — từ chối chạy (fail-closed), không ghi đè:
M v2/sim/vifast-sim/src/lib.rs
```

47/48 bước OK; bước duy nhất đỏ là bước **không thể** xanh trên cây bẩn (đúng như
`JOURNAL/kt405-core.md` đã ghi). Trên **bản sao sạch** nó xanh — xem §7.2.

### 7.2 T1 — `gate --dev` trên **bản sao sạch** (bắt buộc: `vbrev_gate` + `api boundary` fail-closed trên cây bẩn)

Bản sao: `git clone --shared` của worktree → `rsync` toàn bộ thay đổi → `git add -A && git commit`
**trong bản sao** (không phải worktree; worktree vẫn 0 commit). `git status --short` trong bản sao
= **rỗng** ⇒ `dirty=0`, `inputs` thật.

```text
$ cd /tmp/kt404-t1 && CARGO_TARGET_DIR=… bash /tmp/vifast-t1-lock.sh ./v2/bin/v2 gate --dev
...
--- luật máy K6 (KT-80: 1,2,3,7,8): OK
--- wake theo dep-info (KT-217: lớp 2): OK
--- api boundary (KT-241): OK
--- L0 test: OK
--- L0 kernel workspace (KT-148): OK
--- L1 (sim 1.000 seed): OK
--- QEMU smoke (T1: 7 ảnh × 1 boot): OK
--- workspace v2/user: clippy 3 arch + host test (KT-404): OK
# gate-steps registered=80 ran=66 skipped=14 failed=0
# gate-wall 705558 ms (705.6 s)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
```

**Lần chạy T1 đầu tiên ĐỎ đúng một bước** — `luật máy K6` (law 1) bắt **số đo viết tay** trong
`v2/docs/k6-build-rev.md` §10 mà tôi vừa thêm:

```text
not ok - luat 1: v2/docs/k6-build-rev.md:208: E1_NUMBER_ASSIGNED [tracked] số 'dirty=0' ngoài khối `scope=` (số đo gán trong prose)
not ok - luat 1: v2/docs/k6-build-rev.md:217: E1_NUMBER_OUTSIDE [tracked] số '60768' ngoài khối `scope=` (phải là số đo trích từ log)
...
# verify-numbers: ok=4 fail=1
--- luật máy K6 (KT-80: 1,2,3,7,8): FAIL
v2 gate --dev: FAIL (T1 — kiểm tích hợp nhanh)
```

Đây là **AGENTS §4.10** ("đưa số đo viết tay vào tài liệu") — luật có răng, đúng như thiết kế.
Sửa: §10 của `k6-build-rev.md` **bỏ hết số đo**, chỉ giữ **định danh hex** + trỏ sang journal này
(nơi `v2/agent/**` được miễn). Kiểm lại:

```text
$ python3.12 v2/tools/verify-numbers.py --root .
# verify-numbers: ok=5 fail=0 wall_ms=29708 law_wall_ms={1:1664,2:9407,3:160,7:30,8:18083}
```

### 7.3 `repro-check.sh` 3 arch (đóng `N-7`, §5.4)

```text
riscv64: PASS: build tái lập byte — 2 đường dẫn tuyệt đối khác nhau, cùng
         sha256=e0614f9a5cf47be17e386365859415c8454ae325d94586cc357abd54fc3f43bc size=95232
         (build=b48c39d4b751fc04 text=60768)
aarch64: PASS: … cùng sha256=76c02e50e7337b389707651c059311ef676d4ddaf528eed2ebe215ddcb24fe67
         size=90832 (build=0215b89210ded56c text=16228)
x86_64 : PASS: … cùng sha256=b83db555c26d5f105816f282fd22483238ef5111d517d6da118b07c27f1751d2
         size=86880 (build=809ccc9d7955182a text=69632)
```

---

## 8. Việc còn lại / rủi ro / cần Lead quyết

1. **`build.rs` 692 LOC, chưa đạt `~450`** như dòng QUEUE (§4.1). Phần còn lại là **danh sách
   artefact + `rerun-if-changed`** của riêng arch — nén thành bảng `const` sẽ đạt ~450 nhưng **mất
   chú thích lịch sử** từng ELF và không giảm rủi ro thật. **Lead quyết** có muốn nén không.
2. **`v2/user/Cargo.lock` chưa được `check-lockfile-sync.py` phủ** (`LOCKFILES` chỉ có 2 tệp
   `v2/Cargo.lock` + `v2/kernel/Cargo.lock`). Workspace mới **0 gói ngoài** nên rủi ro trôi phiên
   bản hiện bằng 0, nhưng `vifast-ring`/`vifast-virtio` là gói **dùng chung** với `v2/` ⇒ nên mở
   `KT-407`/task sau để so **ba** lockfile. `check-licenses.py` cũng chỉ đọc `v2/Cargo.lock`.
3. **3 `vifast-kbin-*` không thể `#![forbid(unsafe_code)]`** vì `#[no_mangle]` là thuộc tính
   `unsafe` (§6). Muốn phủ thì phải đổi luật R5/D2-17 sang `deny(unsafe_code)` + `#[allow]` có
   kiểm soát (khuôn `vifast-plat-x86_64` của D2-201) — **cần Agent 0 quyết**, không tự làm.
4. **Đường build thường vẫn phụ thuộc thư mục** (`N-7` chỉ đóng ở đường **canonical**
   `repro-build.sh`). Muốn đóng cả đường thường thì phải truyền `--remap-path-prefix` +
   `-C strip=symbols` từ **mọi** runner (`v2/test/boot/**`) hoặc chuyển `v2/lib/*` thành thành
   viên workspace kernel — cả hai **ngoài phạm vi** 3 h và chạm `v2/bin/**`/`v2/test/boot/**`.
5. **`v2/bin/lib/{check,consts,steps_check}.sh` bị sửa** — D2-253 nói "chỉ lead được sửa
   `v2/bin/**`". Task này **yêu cầu tường minh** nối bước vào `steps_check.sh` nên tôi làm, và
   báo lại đây đúng tinh thần D2-253 (không giấu). Ba tệp: `steps_check.sh` (bước 83 + `step_fmt`),
   `check.sh` (đăng ký bước 83 + đếm `lane_failed 83`), `consts.sh` (`CHECK_STEPS_EXPECTED` 47→48,
   ghi chú `gate --fast` 78→79).
6. **`v2/docs/k6-build-rev.md` §10 mới** — tài liệu hoá nguyên nhân gốc `N-7` **không kèm số đo**
   (luật §4.10); số đo nằm ở journal này.
7. **`v2/user/README.md` viết lại** — bản cũ ghi "Không có `Cargo.toml`" (đã sai sau task này).
