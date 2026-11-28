# KT-417c hotfix vòng 2 — bằng chứng X1/X2 theo review đối kháng (`c73eb77a`)

- **Nhánh**: `task/kt417c-evidence` (worktree slot 3), dựa trên `main @82502f7d`.
- **Vì sao**: `review-hotfix-x123` (`/tmp/audits/hotfix-review.md`) kết luận 0 BLOCKER, nhưng
  3 MAJOR là **khoảng trống bằng chứng**, không phải mã sai:
  - **MAJOR-1**: `# syscall: ran … ug=0x0` là **hằng theo cấu trúc** — `enter_user` chỉ ghi 0 hoặc
    `halt(5)`, nên `ug` không đo gì; boot còn in nó **trước** lần `Exit` đầu tiên
    (`boot-1-hotfix.log:69` vs `:72`) ⇒ nếu X2 quay lại, dòng vẫn `ug=0x0`.
  - **MAJOR-3**: "nơi **duy nhất** mọi `iretq`" là sai (còn `asm/syscall.rs:84`,
    `asm/trap.rs:299`, `sysretq:237`), và `gs_is_kernel()` — hàm duy nhất kiểm được phía kernel —
    là **dead code**, nên cửa vào `syscall` không có phép kiểm nào trước `gs:[8]`.
- **Không** hứa đóng MAJOR-2 (nhánh `Switched` từ cửa `syscall` chưa có đường chạy thật): vẫn là
  nợ, ghi ở journal gốc + đề xuất gắn `KT-424` #0.

## Thay đổi

| Tệp | Nội dung |
|---|---|
| `vifast-plat-x86_64/src/syscall_fast.rs` | `user_gs_pair_ok()` (đòi `GS.base == 0` **và** `KERNEL_GS_BASE ∈ {0, gs_scratch(hart)}`) + nhân thuần `user_gs_pair_ok_with()` để test host. `0` là trạng thái boot của ảnh **không** khởi tạo cửa `syscall` (product-shell) — xem mục "T1 lần 1" |
| `vifast-plat-x86_64/src/asm/trap.rs` | `enter_user` đọc **cả hai** MSR sau chuẩn hoá, `halt(5)` nếu cặp sai; bỏ `LAST_USER_GS`/`last_user_gs` (nguồn của `ug` tautology); doc nói đúng "ba đường" thay vì "duy nhất" |
| `vifast-kbin-x86_64/src/umode.rs` | tại cửa vào: `syscall` đòi `gs_is_kernel(hart, GS.base)` (hết dead code), `int 0x80` đòi `GS.base == 0`; lệch ⇒ in `# gs: lech cap … via=…` + `halt(6)`; dòng `# syscall: ran` in `gs=<GS.base đo tại cửa vào>` thay `ug` |
| `vifast-plat-x86_64/tests/syscall_frame_gs.rs` | +2 test: cặp MSR (4 chiều sai), **chốt chống revert** ở tầng mã `kbin` (`include_str!`: phải có `resume_rip_offset(via_syscall)`, `gs_is_kernel(hart_in, gs_base_in)`, `enter_user` phải gọi `user_gs_pair_ok`) |
| `v2/test/boot/run-x86_64.sh` | `check_syscall` so **bằng** `gs=` với `gs_scratch=` (không còn `ug`); ca âm (jj) đổi thành `gs=0xdeadbeef`; **thêm ca dương (0)** log tốt phải XANH; 6 ca âm đòi **đúng lý do** (`grep -qF` thông điệp) — đóng MINOR-7; tổng 37→38 |

## Bằng chứng

| Phép kiểm | Kết quả | Log |
|---|---|---|
| T0 `cargo test -p vifast-plat-x86_64` (host, toàn bộ tệp) | **PASS**, `syscall_frame_gs` **8/8** (thêm 2 test) | `/tmp/kt417c-hotfix2-t0.log` |
| T0 `cargo check -p vifast-kbin-x86_64 --target x86_64-unknown-none --release --features ktest` | PASS (11 cảnh báo có sẵn, không đổi) | cùng log |
| Test chống revert **có răng** | thay `gs_is_kernel(hart_in, gs_base_in)` bằng `false` ⇒ test `kbin_must_use_…` **ĐỎ**; khôi phục ⇒ xanh | `/tmp/kt417c-hotfix2-guard-red.log` |
| **T1 lần 1** `gate --dev` @`7b525ccb` | **ĐỎ 1/81**: `QEMU x86_64 (K5' bước 1)` — `not ok 3 - sản phẩm: boot quá 20 s`; log dừng ngay sau `# shell: enter cpl=3`. Nguyên nhân: `user_gs_pair_ok` đòi `KERNEL_GS_BASE == gs_scratch(hart)` nhưng ảnh **sản phẩm** không gọi `syscall_fast::init` ⇒ `KERNEL_GS_BASE = 0` ⇒ `enter_user` `halt(5)` **oan**. Vá: `0` là trạng thái boot hợp lệ; scratch của hart **khác** vẫn ĐỎ. | `/tmp/t1-hotfix2-7b525ccb.log` |
| **T1 lần 2** `gate --dev` @`4d6dd9e1` | **PASS** `registered=82 ran=68 skipped=14 failed=0`, 649 s (15:08:38–15:19:27 UTC); QEMU x86_64 (bước 59) XANH, `run-x86_64.sh --selftest` **38 ca** + boot `# syscall: ran … gs=0x1501a0` (khớp `gs_scratch=0x1501a0`). | `/tmp/t1-hotfix2b-4d6dd9e1.log` |

## Nợ còn lại (trung thực)

1. **MAJOR-2** (nhánh `Switched` từ cửa `syscall` chưa từng chạy) — cần một chương trình ring 3
   pha core dùng `syscall` trước điểm an toàn `Tcb.Suspend(Defer)`; chưa làm trong hotfix này.
2. **MAJOR-1 phần (b)** (một `syscall` **sau** `Exit`) — pha core/shell hiện chỉ dùng `int 0x80`;
   phép đo GS tại cửa vào đã là **đo thật**, nhưng chưa có lần `syscall` sau `Exit`.

## T2 hợp nhất (sau khi merge 3 nhánh) — `5f48f92a`

- **T2 PASS 81/81**, `# gate-wall 662554 ms` (15:20:07–15:31:13 UTC), `JOURNAL/t2-20261128/`.
- Runner x86_64 (`QEMU x86_64 (K5' bước 1)`) XANH: `# ca âm: 40/40 ĐỎ đúng như phải (+ 1 ca dương
  KT-413c)`, `# 46/46 kiểm sạch`; ca dương (0) đòi toàn bộ log thật XANH nên `gs=` **khớp**
  `gs_scratch=0x1501a0` (ca âm (jj) in đúng giá trị này khi bị đột biến thành `0xdeadbeef`).
- Đây là lần chạy **thật** đầu tiên của runner 38-ca + `gs=` trên cây hợp nhất; là bằng chứng
  T2 cho toàn bộ đợt (hotfix v1+v2 + hạ tầng đo + test-gap).
