# KT-120 — bản vá KT-112 của tôi là **kiểm rỗng** (review toàn dự án bắt)

**Ngày**: 2026-11-18 · **Nguồn**: KT-115 phát hiện #3.

## Lỗi (của chính tôi)

KT-112 thêm vào `shell-edge.sh`:

```bash
n_rows="$(grep -c -E '^  pid=…' "$log")"          # đếm dòng pid
n_declared="$(sed -n 's/^# shell: ps n=\([0-9]*\).*/\1/p' "$log")"   # đọc n
if [ "$n_rows" -ne "$n_declared" ]; then fail; fi
```

Nhưng **cả hai** đều sinh từ **một** biến `count` của shell (`cmd_ps` in `n` rồi in đúng `n` dòng).
⇒ `n_rows == n_declared` **luôn** đúng. `ps` thoái hoá còn **một** tiến trình vẫn xanh — đúng ca mà
KT-112 tuyên bố đã bắt.

Đây là **cùng một lỗi** mà KT-107 đi tìm (test dùng chung nguồn với code), chỉ khác là tôi tự mắc.

## Vá: thêm một **sàn độc lập**

```bash
PS_MIN_ROWS=3   # ảnh sản phẩm luôn có ít nhất procsrv + shell + victim
if [ "$n_rows" -lt "$PS_MIN_ROWS" ]; then fail; fi
```

Sàn này **không** đọc từ log ⇒ shell in sai số vẫn bị bắt.

## Bằng chứng

```text
# selftest mới (ca 3b): log '# shell: ps n=1' + một dòng pid
# selftest 3b ps-mot-dong: log SAI -> ĐỎ đúng (dưới sàn 3)
# shell-edge-selftest: PASS (8/8 ca, 21/21 kiểm: 13 sai→ĐỎ + 8 đúng→XANH)

# chạy thật trên ảnh canonical:
# shell-edge: 8/8 ca PASS
```

## Ghi nhận

Reviewer cũng chỉ ra **#2**: TAP 4 `shell ps tu core` (`proc_demo.rs`) vẫn là xanh giả cho `state`
sau KT-105 — đã vá riêng ở **KT-119** (ghi trạng thái **thật** bằng `m.tcb_state(id)` lúc `PS_REQ`
vào `ps_real`, không giải mã lại `entry`).
