# Sửa khoá build ảnh ktest — đóng 6 mutant `ktest-riscv64` "treo" (2026-11-28)

- **Nhánh**: `task/ktest-lock-fix` (worktree slot 1), dựa trên `main @4119dfeb`.
- **Triệu chứng**: `v2 mutants` với 6 mục `suite = "ktest-riscv64"` (`mut-leak-unlock`, `mut-no-irq`,
  `mut-no-lock`, `mut-smp-set-harts-reset-all`, `mut-smp-no-refresh-published`,
  `mut-smp-publish-any-hart`) treo >25 phút/mục, log không có boot nào (`# infra before … boot_done=0`),
  `--timeout 600` chỉ cắt được thành `E_WRONG-REASON`. Tưởng là "ma trận 27 boot chậm" nhưng
  **không phải**.
- **Nguyên nhân đo được** (`bash -x` trong bản sao mutant): `ktest_build_lock()` vòng
  `while ! mkdir "$KTEST_BUILD_LOCK"` quanh `v2/kernel/target/.ktest-build.lock`. Bản sao
  worktree/mutant **không có** `v2/kernel/target/`, nên `mkdir` trả **ENOENT** (không phải EEXIST)
  ⇒ hàm quay vòng đủ **900 s** (`KTEST_BUILD_LOCK_WAIT_DS=9000`), rồi `return 1` khi chưa hề build/boot.
  Mỗi suite lại thử lại ⇒ "treo" nhiều chục phút; không phải sản phẩm, không phải ma trận chậm.
- **Sửa**: tạo thư mục cha của khoá **một lần** trước vòng; không tạo được thì `return 1` ngay
  (fail-closed, không chờ vô ích nhưng cũng không bỏ qua khoá). Không đổi ngưỡng/không đổi ảnh.

## Bằng chứng

| Phép kiểm | Kết quả |
|---|---|
| Probe cô lập: `KTEST_BUILD_LOCK` trong thư mục cha **chưa tồn tại** | `LOCK_OK`, `pid` được ghi, `UNLOCK_OK` (trước vá: quay vòng 900 s) |
| `mut-leak-unlock` trên cây có vá (`--timeout 900`) | **KILLED 161,6 s** (trước: >25 phút không boot) |
| `mut-no-irq` | **KILLED 160,3 s** |
| `mut-no-lock` | **KILLED 158,7 s** |
| `mut-smp-set-harts-reset-all` | **KILLED 160,5 s** |
| `mut-smp-no-refresh-published` | **KILLED 159,3 s** |
| `mut-smp-publish-any-hart` | **KILLED 158,7 s** |

⇒ Đủ **6/6 KILLED**, không còn `E_WRONG-REASON`; nợ `mutants --all` chỉ còn việc chạy lại **đầy đủ
36 mục** trên HEAD cuối để có một lượt `--all` sạch (đang chạy sau khi hợp nhất).

## Ghi chú

- Đây là lỗi **hạ tầng đo** (T-INFRA/F-class), không phải lỗi sản phẩm; 6 đột biến vẫn là "bộ kiểm
  bắt được" — chỉ có runner không chạy nổi trong bản sao không có `target/`.
- Sửa một chỗ dùng chung (`v2/test/boot/lib/ktest-build.sh`) cho **5 runner** (ktest/net/input/gpu/
  soak) — có lợi cho mọi lần chạy trong worktree/mutant mới.
- Cổng: T1 `82/68/14/0` @`a842091d` (646 s); T2 **PASS 81/81** @`f5f39f6d` (679,4 s, 18:06:00–18:17:22 UTC)
  — ma trận `ktest.sh` chạy đầy đủ trên cây hợp nhất. Sau đó `mutants --all` trên `f5f39f6d` ra
  **36/36 KILLED** (84 m 33 s), xác nhận bản vá khoá build đúng ở quy mô đầy đủ.
