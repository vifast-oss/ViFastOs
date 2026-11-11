# 05. Bản port Rust: cấu trúc, build, kiểm thử

> Cập nhật 2026-11-11 (HEAD `3008601`). Thay hoàn toàn mô tả cũ về
> `vifast-sim` / `manifests/` / 27 test — những phần đó đã bị bỏ khỏi cây.

## 1. Toolchain

[`rust-toolchain.toml`](../rust-toolchain.toml): `stable` + `rustfmt`, `clippy`,
targets `x86_64-unknown-none`, `aarch64-unknown-none`, `riscv64gc-unknown-none-elf`.
QEMU: `qemu-system-{riscv64,aarch64,x86_64}`; x86 cần OVMF
(`scripts/test-qemu.py` tự tìm hoặc dùng `OVMF_CODE`).

## 2. Cấu trúc workspace

```
crates/
  vifast-abi/          wire ABI: 58 syscall, KError, Rights, Message, layout user
  vifast-kernel/       object · cap · ipc · sched · vm · ramfs · invariants
  vifast-runtime/      boot, syscall dispatch, timer, Platform trait
  vifast-arch/         trait Arch + BootInfo
  vifast-arch-{riscv64,aarch64,x86_64}/       UART/clock/IRQ/shutdown
  vifast-platform-{riscv64,aarch64,x86_64}/   trap · MMU · timer · IRQ · SMP
  vifast-kernel-{riscv64,aarch64,x86_64-uefi}/ entry asm + linker + ảnh user
  vifast-pte/ vifast-elf/ vifast-acpi/ vifast-maint/ vifast-fs/ vifast-virtio/
  vifast-fsutil/      tool host cho ảnh FS
  vifast-user/        wrapper syscall
  vifast-root/ vifast-server/ vifast-client/ vifast-bin/
scripts/              build.sh gate.sh check-all.sh test-qemu.py soak.py …
docs/                 xem docs/README.md (phân loại sống / tham chiếu / lịch sử)
```

Cây `python-model/` và `legacy/` chỉ còn để tham chiếu lịch sử, **không** tham
gia build/test.

## 3. Build

```bash
./scripts/build.sh
```

Script làm tuần tự: host test 12 crate → build root/server/client + ảnh
`/bin/vf` cho 3 target (objcopy strip, kiểm ngân sách trang client) → build
kernel 3 arch + biến thể feature (`test-destroy-smp`, `test-boot-handoff`,
`test-panic-hart1`, `test-shell-linger`…) → sinh `target/vifast-build-id.txt`
và `target/vifast-build.log` (log này được `gen-docs.py` dùng lại).

Build lẻ một arch:

```bash
cargo build -p vifast-kernel-riscv64 --target riscv64gc-unknown-none-elf --release
cargo build -p vifast-kernel-aarch64 --target aarch64-unknown-none --release
cargo build -p vifast-kernel-x86_64-uefi --target x86_64-unknown-uefi --release
```

## 4. Kiểm thử

**Host test** (`cargo test -p …`, chạy trong `build.sh`):

| Crate | Test | Ghi chú |
|---|---|---|
| `vifast-kernel` | 261 (+2 `#[ignore]`) | object/cap/IPC/sched/VM/ramfs |
| `vifast-fs` | 52 | định dạng đĩa, journal, sparse, fsck |
| `vifast-elf` | 20 | parse/W^X/overlap/args |
| `vifast-abi` | 16 | layout, pager label, VA window |
| `vifast-pte` | 12 | encode/decode 3 arch |
| `vifast-virtio` | 11 | checksum TCP/IP + keymap bàn phím |
| `vifast-acpi` | 8 | RSDP/MADT |
| `vifast-arch` | 8 | primitive |
| `vifast-runtime` / `vifast-maint` | 4 / 7 | timer + mailbox |
| **Tổng bảng** | **388** | khớp `docs/11-hien-tai/09-testing.md` |

**Cổng QEMU** (3 arch):

```bash
python3 scripts/test-qemu.py --repeat 3
python3 scripts/test-qemu.py --feature test-destroy-smp --repeat 5
python3 scripts/test-qemu.py --tcp-check          # TCP echo qua hostfwd
python3 scripts/test-qemu.py --input-check        # gõ phím vào shell
python3 scripts/test-qemu.py --gpu-check          # screendump, kiểm pixel
python3 scripts/test-qemu.py --interactive        # gõ lệnh qua socket
python3 scripts/test-persist.py                   # 7 phase × 3 arch
```

**Case âm / tài liệu:**

```bash
python3 scripts/test-harness-spec.py     # 677 case âm
python3 scripts/gen-test-counts.py --check
python3 scripts/gen-shell-table.py --check
python3 scripts/gen-marker-table.py --check
python3 scripts/gen-docs.py --check
python3 scripts/check-docs.py
cargo fmt --all --check
```

**Toàn bộ:** `./scripts/check-all.sh` (build + QEMU + persist + harness + docs).

## 5. Quy ước khi sửa

- Thêm/bớt marker guest → cập nhật `scripts/harness_spec.py` **và**
  `docs/11-hien-tai/09-testing.md` (check-docs sẽ báo nếu thiếu).
- Đổi số test → chạy `gen-test-counts.py` để sinh lại bảng.
- Đổi lệnh shell → chạy `gen-shell-table.py`.
- Commit mới → chạy `gen-docs.py` để cập nhật summary (check-docs so khớp).
