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

## Bổ sung: ca tự động cho hai lệnh mới (đóng phát hiện #2 của reviewer)

Reviewer chỉ ra đúng: "shell-edge tám trên tám PASS" là **thật** nhưng **không phủ** mã mới. Đã thêm:

- **`check_truncate_jobs`** — kiểm **ba** vế độc lập: (a) `truncate /README 5` in đúng dòng xác nhận;
  (b) `stat /README` sau đó in `size=5` (chứng minh truncate **có hiệu lực**, không chỉ in); (c) `jobs`
  in đủ bốn số và **tổng** bằng `n` của `ps`.
- **Ca selftest 3c** — log sai (stat vẫn `size=79`) phải **ĐỎ**; log đúng phải **XANH**. Selftest lên
  **hai mươi ba** kiểm (mười lăm sai→ĐỎ + tám đúng→XANH).
- **Ca chạy thật** đặt **ngay trước** `exit` (vì `truncate` **đổi** `/README`, đặt sau sẽ ảnh hưởng ca
  khác). Không đánh số ca mới nên `mark_remaining_fail` và bảng tổng kết không phải sửa.

```text
# truncate-jobs: PASS (# shell: stat path=/README kind=1 size=5 ino=2)
# shell-edge: 8/8 ca PASS
# shell-edge-selftest: PASS (8/8 ca, 23/23 kiểm: 15 sai→ĐỎ + 8 đúng→XANH)
```

## Ảnh canonical cuối (sau KT-126)

```text
REPRO sha256=a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13
      size=205440  inputs=8b5d76c4fdeaf7bb  dirty=0  rev=65d3edfa3862
      build=301720ec292c4f89  text=130072
# đã ghim: v2/artifacts/a0287e4b….bin

bash v2/test/boot/ktest.sh 1 matrix → # KTEST: 21/21 boot sạch · ktest.sh: PASS
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/a0287e4b….bin → KET_LUAN=ok
bash v2/test/boot/shell-edge.sh --image v2/artifacts/a0287e4b….bin → 8/8 ca PASS + truncate-jobs PASS
bash v2/test/boot/persist.sh → persist.sh: PASS
./v2/bin/v2 check → PASS
```
