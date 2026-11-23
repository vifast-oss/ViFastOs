# KT-346 — Tách userland sản phẩm sang `v2/user/` + đóng nợ P20 (3 arch)

**Ngày**: 2026-11-24 (khung 05:30–08:15 JST) · **Nhánh**: `task/kt346b` · **Base**: `main` @ `741cc840`
**Lệnh**: D2-252 (Pha A), D2-239, D2-233, D2-245; AGENTS §6.3 (nợ P20)
**Ngân sách**: 3 h

## 1. Việc làm (theo thứ tự)

### 1.1 Tách cây `v2/user/` (cây độc lập, không phải crate)

`git mv` **7 tệp** (100 % rename, không đổi một byte nội dung):

| Trước | Sau | Ai dùng |
|---|---|---|
| `v2/kernel/vifast-plat-user/lib.rs` | `v2/user/runtime/lib.rs` | shim ABI U-mode (`vifast_user_abi`), biên dịch bởi `build.rs` cả **3** kbin |
| `v2/kernel/vifast-plat-user/programs/{echo,procsrv,shell,shell_cmds}.rs` | `v2/user/programs/` | ELF U-mode dùng chung 3 arch |
| `v2/kernel/vifast-kbin-riscv64/init/hello_user.rs` | `v2/user/programs/hello.rs` | đích `run /hello.elf` (riscv64 `product-exec`) |
| `v2/kernel/vifast-kbin-riscv64/init/netsrv_prod_user.rs` | `v2/user/programs/netsrv_prod.rs` | `netsrv` U-mode (riscv64 `product-net`) |

`v2/kernel/vifast-plat-user/` **không còn tồn tại** (không để façade nửa vời: một cây nguồn
duy nhất, không có bản sao thứ hai). Ba `build.rs` (riscv64/aarch64/x86_64) trỏ thẳng
`../../user/runtime/lib.rs` + `../../user/programs/`; **tên ELF giữ nguyên**
(`shell.elf`/`procsrv.elf`/`echo.elf`/`hello.elf`/`netsrv_prod.elf`) nên **không** phải sửa
`include_bytes!`/`$OUT_DIR/<tên>.elf` trong `src/` của bất kỳ crate nào (đúng khảo sát §2).

**`--remap-path-prefix`**: `vifast-kbin-riscv64/build.rs` đã có `remap_flags()` (KT-92/A1-184);
KT-346 **port** nguyên khuôn đó sang `vifast-kbin-aarch64/build.rs` + `vifast-kbin-x86_64/build.rs`
(đọc `CARGO_ENCODED_RUSTFLAGS`/`RUSTFLAGS`, truyền lại cờ remap cho **mọi** lần gọi `$RUSTC` lồng
nhau: shim rlib, rlib phụ, ELF user). Nhờ vậy đường dẫn tuyệt đối của cây build **không** vào byte
ELF user ⇒ không vào ảnh sản phẩm qua `include_bytes!`.

### 1.2 Dấu vân tay `kt134-v4` → `kt134-v5`

- `v2/test/boot/build-rev.sh`: `VBREV_INPUT_ROOTS="v2/kernel v2/lib v2/user"`, `VBREV_ALGO=kt134-v5`.
- `v2/test/boot/build-rev.algo.pin`: cập nhật cặp (sha256 script, `kt134-v5`).
- `v2/tools/wake_scope.py`: `NON_CRATE_SOURCE_TREES` `v2/kernel/vifast-plat-user` → `v2/user`
  (cây userland vẫn nằm trong phạm vi luật `wake`/macro, không bị "rơi" khỏi scope khi dời).
- `v2/tools/repro-check.sh`: bản sao build thêm `v2/user` + `v2/rust-toolchain.toml`; thêm `--arch`.
- `v2/tools/repro-build.sh`: thêm `--arch riscv64|aarch64|x86_64` (target/bin/package theo arch).

### 1.3 P20 — danh tính bản dựng cho **cả ba** arch

- Port `build_identity` (KT-86/D2-125.3) sang `vifast-kbin-aarch64/build.rs` +
  `vifast-kbin-x86_64/build.rs` (cùng 7 hàm của bản riscv64: `algo_from_script`,
  `inputs_from_script`, `git_head12`, `git_dirty`, `identity_watch_paths`, …).
- `BUILD_LINE` (const `concat!` từ `cargo:rustc-env`) trong `src/main.rs` của hai arch; in **ngay
  sau** `# conf image:` trong `product::boot` (aarch64: sau `image::report_image_digest()`;
  x86_64: sau `writeln!` `# conf image:`), cùng thứ tự khoá với riscv64:
  `# build: algo=<algo> inputs=<16 hex|unknown> dirty=<0|1>`.
- Ba runner cổng đọc được dòng này trên **ảnh sản phẩm** (bằng chứng §2.1).

### 1.4 Ca kiểm mới trong `build-rev.sh --selftest`

- `(l)` sửa `v2/user/programs/shell.rs` rồi commit ⇒ dấu vân tay **PHẢI** đổi (phép kiểm **dương**
  của phạm vi `v2/user`; ĐỎ nếu bỏ `v2/user` khỏi `VBREV_INPUT_ROOTS` — đột biến ii).
- `(m)` **hai thư mục khác nhau, cùng nội dung ⇒ dấu vân tay PHẢI GIỐNG** (bất biến P20: danh tính
  bền không phụ thuộc đường dẫn build; ĐỎ nếu chèn đường dẫn tuyệt đối vào chuỗi băm — đột biến i).

## 2. Bằng chứng

(xem §3 — số liệu đo được, dán nguyên văn)

## 3. Nợ / nói thẳng

(xem §3 khi hoàn tất)
