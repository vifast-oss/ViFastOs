# KT-112 — vá ba lỗ xanh-giả của harness `shell-edge.sh` (review chéo phát hiện)

**Ngày**: 2026-11-18 · **Nguồn**: review chéo độc lập KT-108, verdict **DRIFT**.

## Ba lỗ (reviewer dựng lại được, không phải suy đoán)

### 1. `LOGDIR` cố định ⇒ hai lượt chạy **song song** dùng chung log

`LOGDIR="${SHELL_EDGE_LOGDIR:-/tmp/kt108-shell-edge}"` — reviewer dựng một ảnh giả **không boot được**
rồi chạy **đồng thời** với lượt chạy ảnh canonical: lượt ảnh giả báo `wait-khong-treo: PASS` và
`exit: PASS` **từ log của lượt kia**. Đây là xanh giả thật.

**Vá**: mỗi lượt một thư mục `mktemp -d`; dọn khi thoát. `SHELL_EDGE_LOGDIR` vẫn cho chỉ định.

### 2. `check_ps_no_unknown` chỉ đòi **≥ một** dòng pid

`ps` thoái hoá còn một tiến trình vẫn xanh. **Vá**: đối chiếu số dòng pid với `n` trong dòng tổng kết
`# shell: ps n=<n>` — hai nguồn trong cùng log.

### 3. Thông báo lỗi hứa thời gian **sai**

`sleep 0,05` trên macOS thực tế tốn ~0,2 s (reviewer đo 200 lần = 41,6 s) ⇒ `BOOT_POLLS=400` là ~80 s
chứ không phải "20 s", `CASE_POLLS=200` là ~42 s chứ không phải "10 s". **Vá**: in **số vòng poll**
thay vì giây.

## Bằng chứng sau vá

```text
bash v2/test/boot/shell-edge.sh --selftest
# shell-edge-selftest: PASS (8/8 ca, 19/19 kiểm: 11 sai→ĐỎ + 8 đúng→XANH)

bash v2/test/boot/shell-edge.sh --image v2/artifacts/34ee7a25….bin
# shell-edge: 8/8 ca PASS · # shell-edge.sh: PASS

# hai lượt SONG SONG trên cùng ảnh:
A: # shell-edge.sh: PASS
B: # shell-edge.sh: PASS
# thư mục log còn lại: 0 · QEMU còn lại: 0

# ảnh giả KHÔNG boot, chạy SONG SONG với ảnh thật:
FAKE: shell-edge.sh: ảnh thiếu bản vá KT-103 … (exit 1)
# trước vá: FAKE báo PASS nhờ log của lượt kia
```

## Chưa vá (reviewer nêu, cần quyết)

- **Selftest không phủ dây nối**: phá `case_begin` thì selftest vẫn 19/19 PASS trong khi chạy thật ĐỎ.
  Muốn phủ phải thêm ca kiểm cho `run_case`/`case_begin`/`wait_re` — việc riêng.
- **Ca 1 và ca 2 khẳng định cùng một chuỗi** `# shell: thieu duong dan` ⇒ không phân biệt `cat` với
  `write`. Muốn phân biệt phải cho shell in tên lệnh trong thông báo — **đổi hành vi shell**, cần A0.
- **`preflight_image` đòi chuỗi `# shell: thieu duong dan`** nhưng chuỗi này có từ KT-77c, nên nó
  **không** xác nhận được KT-104. Ghi nhận, chưa sửa (cần chuỗi riêng của KT-104).
