<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# `v2/user/` — userland sản phẩm (nguồn độc lập, không phải crate)

**KT-346** (D2-252 Pha A) tách toàn bộ nguồn userland **sản phẩm** ra khỏi `v2/kernel/` để nó là
**một** cây nguồn duy nhất, không bị kẹp trong `build.rs` của một ảnh cụ thể.

| Đường dẫn | Nội dung | Ai biên dịch |
|---|---|---|
| `runtime/lib.rs` | shim ABI U-mode (`vifast_user_abi`): `ecall` (riscv64), `svc #0` (aarch64), `int 0x80` (x86_64), header IPC buffer | `build.rs` của **cả ba** `vifast-kbin-*`, thành `$OUT_DIR/libvifast_user_abi.rlib` |
| `programs/shell.rs` + `shell_cmds.rs` | shell sản phẩm (24–28 lệnh theo `--cfg shell_fs`/`shell_net`/`shell_exec`) | cả ba arch → `shell.elf` |
| `programs/procsrv.rs` | `wait`/`kill` qua `EP_PROC` | cả ba arch → `procsrv.elf` |
| `programs/echo.rs` | phép kiểm buffer IPC của TAP 44 (aarch64/x86_64) | `aarch64`/`x86_64` → `echo.elf` |
| `programs/hello.rs` | đích của `run /hello.elf` (riscv64 `product-exec`) | riscv64 → `hello.elf` |
| `programs/netsrv_prod.rs` | `netsrv` U-mode của ảnh sản phẩm (riscv64 `product-net`) | riscv64 → `netsrv_prod.elf` |

## Luật của cây này

1. **Không** có `Cargo.toml`: ba `build.rs` (`vifast-kbin-{riscv64,aarch64,x86_64}`) gọi `$RUSTC`
   trực tiếp với `--target` của mình. Chương trình U-mode **không** được là thành viên workspace
   (chúng là `no_std` cho target `*-unknown-none`, không có host test).
2. **Tên ELF là hợp đồng**: `shell.elf`/`procsrv.elf`/`echo.elf`/`hello.elf`/`netsrv_prod.elf` được
   `include_bytes!(concat!(env!("OUT_DIR"), "/<tên>.elf"))` ở `src/shellpath.rs`/`k5user.rs`/
   `proc_exec.rs`/… ⇒ **đổi tên tệp nguồn thì phải giữ nguyên tên ELF**.
3. **Đường dẫn tuyệt đối không được vào ảnh**: mọi lần gọi `$RUSTC` lồng nhau phải truyền
   `--remap-path-prefix` mà cargo đã cấp (`remap_flags()` trong từng `build.rs`, KT-92/A1-184).
4. Cây này nằm trong **dấu vân tay build-inputs** (`v2/test/boot/build-rev.sh`:
   `VBREV_INPUT_ROOTS="v2/kernel v2/lib v2/user"`, `VBREV_ALGO=kt134-v5`) ⇒ sửa một dòng ở đây làm
   dấu vân tay đổi (ca `(l)` của `build-rev.sh --selftest`).
5. Ngoài phạm vi `unsafe` (R5/D2-17) **chỉ** ở `runtime/lib.rs` (shim ABI) — `programs/*.rs` không
   có `unsafe`.
