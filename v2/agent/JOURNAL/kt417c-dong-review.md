# Đóng review đối kháng `KT-417c` — trạng thái từng phát hiện (2026-11-28)

- **Review gốc**: `/tmp/audits/hotfix-review.md` trên `c73eb77a` (merge `82502f7d`): **0 BLOCKER,
  3 MAJOR, 7 MINOR, 4 NIT**.
- **Đã vá**: hotfix vòng 1 (`c73eb77a`) + vòng 2 (`4d6dd9e1`) + `KT-424 #0` (`0e42e3f0`).
- **Cổng chứng nhận**: T2 PASS `81/81` trên `cc405ead` (674,9 s, `# measure-key commit=cc405ead
  build-inputs=b15fc2f60a7d436b dirty=0`) — cây có đủ cả 3 vòng vá.
- Quy ước: **CLOSED** = có mã + phép kiểm/âm bản chứng minh; **PARTIAL** = đã giảm nhưng còn phần
  chưa làm; **OPEN (hoãn)** = chấp nhận nợ, không ảnh hưởng cổng, ghi để vòng sau.

## MAJOR

| # | Phát hiện | Trạng thái | Bằng chứng |
|---|---|---|---|
| MAJOR-1a | `ug=` là **hằng 0 theo cấu trúc**, in trước `Exit` ⇒ không thể phát hiện X2 | **CLOSED** | Bỏ `LAST_USER_GS`; handler in `gs=` **đo tại cửa vào** (sau `swapgs`), runner so **bằng** `gs_scratch=` dòng `# syscall: msr`; ca âm (jj) đột biến `gs=0xdeadbeef` ⇒ ĐỎ đúng lý do; ca dương (0) đòi log thật XANH |
| MAJOR-1b | Chưa có `syscall` **sau** `Exit` (X2 chỉ chứng minh trước `Exit`) | **CLOSED** | `KT-424 #0`: pha core (chạy sau `# umode: exit`) gọi `Tcb.Suspend(Defer)` bằng `syscall`; runner `check_switched_syscall` đòi dòng `# umode: switched via=syscall … gs=0x…` **sau** `# umode: exit` và `gs` khớp `gs_scratch`; T1 `0e42e3f0` + T2 `cc405ead` xanh, runner `47/47` |
| MAJOR-2 | Nhánh X1 (`resume_rip_offset(via_syscall)`) **chưa từng chạy**; mã `kbin` không có test | **CLOSED** | Cùng thay đổi `KT-424 #0` (Suspend qua `syscall` ⇒ `Switched` từ cửa nhanh); test `core_phase_must_switch_via_syscall` (`include_str!`) chốt `mov r10, {defer}` + `syscall` và **cấm** quay lại `int 0x80`; ca âm (mm) đổi `via=syscall` → `via=int80` ⇒ ĐỎ |
| MAJOR-3 | "Nơi **duy nhất** mọi `iretq`" sai; `gs_is_kernel()` dead code | **CLOSED** | `syscall_dispatch` đòi `gs_is_kernel(hart, GS.base)` cho cửa nhanh, `int 0x80` đòi `GS.base == 0`; lệch ⇒ `halt(6)` có tên; doc `enter_user` nói đúng "ba đường" thay vì "duy nhất" |

## MINOR

| # | Phát hiện | Trạng thái | Ghi chú |
|---|---|---|---|
| MINOR-4 | `user_gs_ok` chỉ kiểm `GS.base`, bỏ `KERNEL_GS_BASE` | **CLOSED** | `user_gs_pair_ok` đọc **cả hai** MSR; `enter_user` `halt(5)` nếu lệch. `KERNEL_GS_BASE = 0` được chấp nhận vì ảnh sản phẩm `product-shell` **không** init cửa `syscall` — đã tái hiện `halt(5)` oan ở T1 lần 1 và sửa đúng chỗ (xem `kt417c-hotfix2.md`) |
| MINOR-5 | `LAST_USER_GS` static toàn cục, không theo hart | **CLOSED** | Xoá hẳn cùng MAJOR-1a; không còn trạng thái toàn cục để lẫn hart |
| MINOR-6 | Phép thay thế X3 gần tautology, giả định `SMEP=1`, bỏ sót 2 vùng mã user | **OPEN (hoãn)** | `user_rip_ok` vẫn là cửa sổ theo `code_va/code_bytes`; runner tự kiểm `rip` trong dải user **đo được** (`check_rip_in_user_window`). Không ảnh hưởng cổng; nợ đặt tên/ý niệm `rip_in_loaded_user_code` + SMEP-independent |
| MINOR-7 | 6 ca âm chỉ đòi `rc != 0`, không đòi **lý do**; thiếu ca dương; thiếu ca cho X1 | **CLOSED** | Mỗi ca âm đòi đúng thông điệp (`grep -qF`); thêm ca dương (0) log thật phải XANH; ca (mm) cho X1; ca (jj) cho X2 đo tại cửa vào |
| MINOR-8 | Const-assert chỉ **đếm** `push`, không kiểm thứ tự/giá trị | **OPEN (hoãn)** | `fast_frame_slots_fit` mới kiểm biên offset; chưa assert chuỗi `push` theo thứ tự. Đã có test offset literal + test `syscall_frame_gs` 9 ca; rủi ro thấp nhưng là nợ thật |
| MINOR-9 | Test `enter_user_must_restore_gs_before_iretq` chấp nhận `swapgs` vô điều kiện | **CLOSED** | Test đòi thân hàm chứa `needs_swap_before_user`/`user_gs_ok` **trước** `swapgs` ⇒ xoá điều kiện là ĐỎ |
| MINOR-10 | `gs_pair_survives_every_way_out` là mô hình tự viết, tên overclaim | **OPEN (hoãn)** | Chỉ đổi tên/thêm nhánh `int 0x80` cho khớp mô hình; không phải lỗi mã. Gộp vào lượt sửa NIT sau (tránh đổi input T2 chỉ vì tên test) |

## NIT

| # | Phát hiện | Trạng thái |
|---|---|---|
| NIT-11 | 6 comment selector còn ngược nghĩa (`0x1B` ↔ `0x23`) sau khi GDT đảo | **OPEN (hoãn)** — doc-only, không chỗ nào so sánh `0x1B` |
| NIT-12 | Doc `readback()` còn di sản tuple 4 ô | **OPEN (hoãn)** — doc-only |
| NIT-13 | `halt(6)` dùng cho hai lỗi khác nhau; thiếu bảng mã halt | **OPEN (hoãn)** — dòng `PANIC:` phân biệt được; thêm bảng mã khi mở `KT-424` |
| NIT-14 | `enter_user` thêm 2 `rdmsr` trên mọi lần vào user | **OPEN (hoãn)** — ghi để biết chỗ nếu hot-path hồi quy; chưa đo |

## Kết luận

- **0 BLOCKER; 3/3 MAJOR đóng bằng bằng chứng chạy** (không chỉ bằng lập luận): X1 (`Switched` từ
  cửa `syscall`), X2 (GS đo tại cửa vào, so `gs_scratch`), X2-sau-Exit (syscall pha core sau
  `# umode: exit`), X3 (rip/CS đo được + 6 ca âm đòi lý do).
- Nợ còn lại là **MINOR-6/8/10 + 4 NIT** — không cái nào ảnh hưởng tính đúng của hotfix; gộp vào
  một lượt sửa doc/test sau (kèm T2 riêng) để không đổi input T2 chỉ vì tên/comment.
