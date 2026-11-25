<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# `v2/user/` — userland sản phẩm (**workspace Cargo độc lập**)

**KT-346** (D2-252 Pha A) tách toàn bộ nguồn userland **sản phẩm** ra khỏi `v2/kernel/` để nó là
**một** cây nguồn duy nhất, không bị kẹp trong `build.rs` của một ảnh cụ thể.
**KT-404** (`B4`, D2-259) biến cây này thành **workspace Cargo thật** — trước đó nó **không** là
crate cargo nào nên `clippy -D warnings` của cổng **chưa bao giờ** chạy trên nó (kể cả
`runtime/lib.rs` có `unsafe`).

## Ba package

| Package | Nguồn | `[lib] name` / `[[bin]]` | Ai biên dịch **ảnh** |
|---|---|---|---|
| **`vifast-urt`** | `runtime/lib.rs` (+ `runtime/bump.rs`) | `vifast_user_abi` | `build.rs` của **cả ba** `vifast-kbin-*` → `$OUT_DIR/libvifast_user_abi.rlib` |
| **`vifast-rt`** | `rt/lib.rs` | `vifast_rt` | riscv64 `product-exec` → `libvifast_rt.rlib` |
| **`vifast-programs`** | `programs/*.rs` | 6 `[[bin]]`: `shell`, `procsrv`, `echo`, `hello`, `netsrv_prod`, `u1a` | cả ba arch → `<tên>.elf` |

`[lib] name` **cố ý** giữ `vifast_user_abi`/`vifast_rt`: ba `build.rs` truyền
`--extern vifast_user_abi=<rlib>` và mọi chương trình có `extern crate vifast_user_abi;`. Đổi tên
crate ⇒ đổi symbol đã mangle ⇒ đổi byte ELF ⇒ đổi ảnh.

`[lib] path`/`[[bin]] path` trỏ **thẳng** tệp nguồn hiện có ⇒ **0 dời tệp**, 0 vỡ tham chiếu của
`build.rs`/`kt347-u1a.sh`/`steps_check.sh`.

## Luật của cây này

1. **Workspace này chỉ để lint/test**, **không** thay đường build ảnh: ba `build.rs` vẫn gọi
   `$RUSTC` trực tiếp với `--target` + cờ riêng của từng ảnh (qua crate host
   [`vifast-kbuild`](../kernel/vifast-kbuild/src/lib.rs)). Lý do: chương trình U-mode là `no_std`
   cho target `*-unknown-none`, mỗi ảnh cần bộ `--cfg` và linker script riêng.
2. **Bước `83` của `v2 check`** (`step_user_workspace` trong
   [`v2/bin/lib/steps_check.sh`](../bin/lib/steps_check.sh)) chạy `cargo clippy -D warnings` cho
   **cả 3 target bare-metal** với đúng bộ `--cfg` của **ảnh sản phẩm** (`rt_alloc`, `shell_fs`,
   `shell_net`, `shell_exec`, `product_net`) + 8 ca host của lõi bump. `cargo fmt --all --check`
   của workspace này nằm trong bước `fmt`.
3. **Tên ELF là hợp đồng**: `shell.elf`/`procsrv.elf`/`echo.elf`/`hello.elf`/`netsrv_prod.elf` được
   `include_bytes!(concat!(env!("OUT_DIR"), "/<tên>.elf"))` ở `src/shellpath.rs`/`k5user.rs`/
   `proc_exec.rs`/… ⇒ **đổi tên tệp nguồn thì phải giữ nguyên tên ELF**.
4. **Đường dẫn tuyệt đối không được vào ảnh**: mọi lần gọi `$RUSTC` lồng nhau phải truyền
   `--remap-path-prefix` mà cargo đã cấp (`vifast_kbuild::compile::remap_flags`, KT-92/A1-184).
5. Cây này nằm trong **dấu vân tay build-inputs** (`v2/test/boot/build-rev.sh`:
   `VBREV_INPUT_ROOTS="v2/kernel v2/lib v2/user"`, `VBREV_ALGO=kt134-v5`) ⇒ sửa một dòng ở đây làm
   dấu vân tay đổi (ca `(l)` của `build-rev.sh --selftest`).
6. Ngoài phạm vi `unsafe` (R5/D2-17) **chỉ** ở `runtime/lib.rs` (shim ABI, `mod heap` dưới
   `--cfg rt_alloc`) — `programs/*.rs` và `rt/lib.rs` **không** có `unsafe` (`vifast-rt` đã
   `#![forbid(unsafe_code)]` từ KT-404).
7. **Không** thêm `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim`: `check-wake-paths.py` luật
   `E_STRICT_WAKE_INCLUDE` (KT-171 B6/B7) **fail-closed**. Vì vậy 8 ca host của `runtime/bump.rs`
   chạy bằng `rustc --test` (đúng lệnh của `v2/test/boot/kt347-u1a.sh`), không qua `cargo test`.
