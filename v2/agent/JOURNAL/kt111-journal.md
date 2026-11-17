# KT-111 — `persist_host.py`: kiểm **ngữ nghĩa** trước khi kiểm công thức

**Ngày**: 2026-11-18 · **Nguồn**: KT-107 (audit "test dùng chung công thức"), mục NGHI VẤN 3.

## Lỗi

`v2/test/boot/persist_host.py` kiểm pha 7 bằng:

```python
if detail != (single << 32) | double:
    fail(...)
```

Kernel (`v2/kernel/vifast-kbin-riscv64/src/ktest_persist.rs:770`) tính:

```rust
detail = ((raw[10] as i64) << 32) | raw[11] as i64;
```

⇒ **cùng một công thức**. Đổi **cả hai** (ví dụ `<< 16`) thì harness vẫn xanh — đã chứng minh bằng
đột biến trong KT-107.

## Vá: bốn bất biến **không** phụ thuộc con số dịch

| # | Bất biến | Bắt được gì |
|---|---|---|
| i | `single != double` | hai bảng gián tiếp trùng khối |
| ii | `single > 0` và `double > 0` | khối 0 là superblock, không phải bảng gián tiếp |
| iii | `detail > single` và `detail > double` | cách đóng gói làm **mất** một khối |
| iv | `detail >= 2^32` | đổi dịch bit (`<< 16`, `<< 24`, …) làm `detail` tụt dưới `2^32` |

(iv) là bất biến mạnh nhất: nó đúng với **mọi** cách đóng gói hai khối 32 bit, nên không "dùng chung
công thức" với kernel.

## Bằng chứng

```text
# kiểm logic bằng log tổng hợp:
dung  <<32   : PASS
dot bien <<16: FAIL < 2^32
dot bien <<24: FAIL < 2^32
collapse     : FAIL < 2^32

# chạy thật trên ảnh canonical 34ee7a25…:
bash v2/test/boot/persist.sh → persist.sh: PASS (7 pha, distinct_process=1)
```

## Còn lại (chưa làm)

- **`kt18_tcb.rs`** dùng chính bảng `ALLOWED` của `v2/sim/vifast-sim/src/tcb.rs` làm kỳ vọng cho
  một trăm hai mươi mốt cặp. Vá cần **bảng kỳ vọng viết tay độc lập** từ thiết kế §4.4/§4.5 — việc
  lớn, và bảng viết tay cũng có rủi ro riêng. Để Agent 0 quyết cách (bảng tay hay kiểm bất biến).
- Ba mục NGHI VẤN nhỏ hơn (`ktest_con.rs:359`, `ktest_core.rs:531`, `persist_host.py:42`) — chưa vá.
