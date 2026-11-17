# KT-117 — `kt18_tcb`: bảng `ALLOWED` không còn tự chứng minh vòng tròn

**Ngày**: 2026-11-18 · **Nguồn**: KT-107 mục NGHI VẤN 1, reviewer độc lập xác nhận.

## Lỗi

`v2/sim/vifast-sim/tests/kt18_tcb.rs` — `bang_allowed_11x11_du_cap_va_cam` lặp **chính** `ALLOWED`
để dựng kỳ vọng:

```rust
for (from, row) in ALLOWED.iter().enumerate() {
    for (to, is_allowed) in row.iter().enumerate() {
        ...
        if *is_allowed { assert!(result.is_ok(), ...) }
```

⇒ đảo một ô trong bảng thì test **vẫn xanh**. Reviewer đã dựng lại: hoán vị
`BlockedOnRecv→Inactive` (đúng → sai) và `BlockedOnRecv→BlockedOnReply` (sai → đúng) ⇒ **18/18 vẫn
PASS**. Chỉ `kt23_k1b` bắt được một phần (`forbidden_attempts=45`).

## Vá: hai nguồn độc lập

### 1. `bang_allowed_khop_tai_lieu` — so với **tài liệu**

`v2/docs/k1b-tcb-transition.md` §2 có **bảng markdown đầy đủ** mười một hàng (nguồn khác với mã).
Test đọc **nguyên văn** bảng đó bằng `include_str!` rồi so **từng ô** với `ALLOWED`.

### 2. `bang_allowed_bat_bien_thiet_ke` — tám bất biến **suy từ thiết kế**

Viết từ §4.4/§4.5, **không** đọc bảng:

| # | Bất biến | Nguồn |
|---|---|---|
| 1 | `Dead` không ra được | §4.4 "Dead: không ra được" |
| 2 | Chỉ `Ready` vào `Running` | Running = đã được scheduler chọn từ ready queue |
| 3 | Chỉ `Running` vào `Sleeping` | §4.5 hết budget/timeout khi đang chạy |
| 4 | `Stopping` chỉ từ `Ready`/`Running`, chỉ ra `Dead` | §4.4, K1c |
| 5 | Mọi trạng thái trừ `Dead` vào được `Dead` | finalise |
| 6 | Mọi `BlockedOn*` vào được `Ready` và `Inactive` | wake / cancel |
| 7 | `BlockedOnReply` chỉ từ `Ready`/`Running`/`BlockedOnSend` | chỉ `Call` mới tạo Reply |
| 8 | Không cặp danh tính nào trừ `BlockedOnSend→BlockedOnSend` | fault khi đang chờ gửi |

Bất biến (2) là mạnh nhất: nó **không** phụ thuộc bảng, nên sửa cả mã **và** tài liệu vẫn bị bắt.

## Bằng chứng

```text
# không đột biến (target dir sạch):
test result: ok. 20 passed; 0 failed

# đột biến đúng như reviewer (BlockedOnRecv: →Inactive true→false, →BlockedOnReply false→true):
test bang_allowed_bat_bien_thiet_ke ... FAILED
test bang_allowed_khop_tai_lieu ... FAILED
test bang_allowed_11x11_du_cap_va_cam ... ok      ← test cũ vẫn xanh: đúng là nó không bắt được
test result: FAILED. 18 passed; 2 failed
```

## Bài học phụ (đáng ghi)

Lần đầu tôi chạy test trong repo nhưng dùng **chung** `CARGO_TARGET_DIR` với bản clone đã bị đột
biến ⇒ cargo dùng lại **bản build cũ** và báo hai test đỏ **sai**. Đây đúng là lý do `task-target.sh`
(KT-84) tồn tại: mỗi task một thư mục target. Khi nghi ngờ, xoá target dir rồi chạy lại.
