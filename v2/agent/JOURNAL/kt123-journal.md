# KT-123 — ba đính chính từ review chéo lô hai

**Ngày**: 2026-11-18 · **Nguồn**: review chéo `xreview-batch2`, verdict **DRIFT** ở A và E.

## 1. `ktest_con.rs` — bất biến mới **không** phải thứ bắt (E1)

Reviewer tắt riêng bất biến mới và **giữ** đột biến `seq`: TAP 2 **vẫn đỏ**. Nghĩa là thứ bắt đột
biến là `Ring::check()` (đã đòi `seq == head`) và TAP 1 (đã có `seq == head`), **không** phải bất biến
tôi thêm. Comment cũ mô tả nó là "nguồn độc lập" — **sai**.

**Sửa**: viết lại comment cho đúng — nó là **kiểm nhất quán nội tại** của cùng header, giữ lại vì rẻ
và bắt được header hỏng cục bộ, nhưng **không** được gọi là "độc lập".

## 2. `shell-edge.sh` — ca 3b dùng lại `right.log` của ca 3 (D2)

Nếu ai sửa ca 3 thành một dòng thì 3b sẽ **đỏ oan**. **Sửa**: ca 3b có `right.log` riêng.

## 3. Ảnh có `product-fs` mà FS = `None`: mỗi lệnh đứng **~54 giây** (A2)

Kernel rút request khỏi hàng đợi nhưng **không trả lời**, nên shell quay `FS_REPLY_BUDGET` (hai mươi
nghìn) vòng `Yield` rồi mới bỏ cuộc. Reviewer đo `sync` **53,75 s**, `ls` cũ **50,15 s** — lỗi **có
sẵn từ KT-77c**, không phải KT-114 tạo ra, nhưng KT-114 thêm hai lệnh đi cùng đường nên đáng vá.

**Vá**: kernel trả lời **ngay** một `FS_REPLY` với status mới `ST_NOFS` (3). Shell in
`fs khong san sang` thay vì `fs loi code=0`.

```text
# trước:  # shell: fs loi code=0        (sau ~54 s)
# sau:    # shell: fs khong san sang (image khong co product-fs hoac khong co phuong tien)   (ngay)
```

**Không nới phép kiểm nào**: parity FS vẫn **đỏ** ở ca âm 5(c) vì không có dữ liệu thật — chỉ nhanh
hơn.

## Ghi chú khác từ review (chưa vá)

- **Ảnh test đổi 3 byte** sau KT-114: ba số dòng `panic::Location` trong `.rodata` của `shell.elf`
  nhúng (dòng dịch xuống khi chèn mã). **Hành vi không đổi**, không assertion nào đỏ. Chưa ai ghim
  sha ảnh test nên chưa ảnh hưởng.
- **`ps_real` bị ghi đè** nếu `ps` gọi hai lần — nhưng `ok18` đòi `ps_replies == 1` nên TAP 4 **đỏ**
  chứ không xanh giả. Fail-closed, chấp nhận.
- **`demo.ps_real[n]`** dùng `entries.len()` làm guard thay vì `ps_real.len()` — hai mảng cùng bốn
  phần tử nên đúng, nhưng nếu ai nới `entries` thì `ps_real` tràn. Ghi nhận.
