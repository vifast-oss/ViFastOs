# KT-126 — `truncate` nuốt kích thước ≥ 2³² rồi báo thành công

**Ngày**: 2026-11-18 · **Nguồn**: review chéo `xreview-kt125`, phát hiện **trung bình** #1.

## Lỗi

`head` chỉ dành **ba mươi hai** bit cho tham số (`arg = (head >> 8) & 0xffff_ffff` ở kernel), nên
`size << 8` với `size ≥ 2^32` bị **cắt âm thầm**:

```text
# shell: truncate path=/README size=4294967296     ← báo thành công
# shell: stat path=/README kind=1 size=0 ino=2     ← tệp về KHÔNG byte
```

Đây là dạng **nói dối**: lệnh in ra "đã truncate tới 4294967296" trong khi thực tế về không.

## Vá

Từ chối sớm thay vì cắt âm thầm:

```rust
if size > 0xFFFF_FFFF {
    put(b"# shell: kich thuoc qua lon (toi da 4294967295)\n");
    return;
}
```

## Bằng chứng

```text
# shell: kich thuoc qua lon (toi da 4294967295)
# shell: stat path=/README kind=1 size=79 ino=2     ← tệp KHÔNG đổi
# shell: truncate path=/README size=5
# shell: stat path=/README kind=1 size=5 ino=2      ← đường bình thường vẫn đúng
```

## Chưa làm (reviewer chỉ ra, ghi lại)

- **Hai lệnh mới chưa có ca tự động** trong `shell-edge.sh`/`ktest.sh`. Đây đúng dạng "xanh giả" mà
  đợt này đang chống: journal dẫn "shell-edge tám trên tám PASS" là **thật**, nhưng **không phủ** mã
  mới. Cần thêm ca — chưa làm trong ngân sách này.
- **`spec/v1-parity.toml` hết đúng** ở mục `truncate` và `jobs` (vẫn ghi "chưa có"). `spec/` **không**
  thuộc quyền của tôi (AGENTS §4) ⇒ **cần Agent 0**.
- **`truncate` lỗi vẫn để lại tác dụng phụ**: `truncate /README 999999` trả `code=3` nhưng tệp đã
  phình từ 79 lên 32768 B (đường grow commit từng block trước khi báo lỗi). Ghi nhận, chưa vá.
- **`jobs` không có gì kiểm**: `ps` được TAP 4 đối chiếu `ps_real`, còn `jobs` chép cùng công thức
  giải mã mà **không** có nguồn đối chiếu. Nếu công thức sai, không ai bắt.
