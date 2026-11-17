# KT-105 — `ps` in `state=Unknown` cho **mọi** tiến trình (lỗi thật, không test nào bắt)

**Ngày**: 2026-11-18 · **Ngân sách**: 30 phút · **Dùng**: ~20 phút.

## Lỗi

Kernel đóng gói mỗi dòng `ps` là:

```text
entry = (id << 5) | (state.index() << 1) | suspended
```

⇒ trạng thái chiếm **bốn** bit (1..4), pid bắt đầu ở bit 5.

Shell giải mã:

```rust
let state = (entry >> 1) & 0x1F;   // ← NĂM bit
```

Với `id = 13`: `entry >> 1 = 208 + state`; `208 & 0x1F = 16` ⇒ `16 + state` ⇒ **luôn** > 10 ⇒
`state_name` trả `Unknown`. Tức `ps` **chưa bao giờ** in đúng trạng thái.

## Vá

`& 0x0F` (bốn bit) — khớp cách kernel đóng gói.

## Bằng chứng

```text
# trước:  pid=13 state=Unknown · pid=15 state=Unknown · pid=17 state=Unknown suspended
# sau:    pid=13 state=Ready   · pid=15 state=Ready   · pid=17 state=Ready
```

## Vì sao không test nào bắt

- `spec/v1-parity.toml` chỉ đòi `^# shell: ps n=[0-9]+$` — **không** kiểm tên trạng thái.
- TAP của `ktest_proc.rs` kiểm `ps` trả **số dòng** và pid, không kiểm `state=`.
- ⇒ Lỗi nằm đúng giữa hai phép kiểm. Đây là **khoảng trống phủ**, không phải "không ai thử".

## Đề xuất (cần Agent 0 vì đụng kỳ vọng)

Thêm một ca kiểm khẳng định `ps` **không** in `state=Unknown` cho tiến trình đang `Ready`. Chỗ đúng
là `spec/v1-parity.toml` (thêm `transcript_expect`) hoặc TAP trong `ktest_proc.rs` — cả hai đều là
kỳ vọng harness, nên **chưa** làm.

## Ghi chú: `kill 999` trả `code=0` — **không** phải lỗi

`procsrv_user.rs:162` luôn trả `code=0` cho `KILL_REQ`. Đã đối chiếu `spec/v1-parity.toml:110`:
v1 **cũng** trả `code=0` cho pid không tồn tại (`expect = '^# shell: kill pid=999 code=0$'`).
Giữ nguyên là **đúng parity**; đổi sẽ làm đỏ ca `kill`.

## Phát hiện quan trọng hơn: **test dùng chung lỗi với shell**

Sau khi vá shell, **ma trận 18/21 ĐỎ** (`ktest-proc` cả ba cấu hình hart): `snapshot_match=0`.

Nguyên nhân: `v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:783` dựng **dòng kỳ vọng** bằng
**cùng** mask sai:

```rust
let state = ((entry >> 1) & 0x1F) as u8;   // ← cùng lỗi với shell
```

⇒ Test so **shell in gì** với **chuỗi test tự dựng bằng cùng công thức sai** ⇒ hai bên "khớp nhau"
mà **cả hai đều sai**. Đây là dạng lỗi nguy hiểm nhất của test: **test không độc lập với đối tượng
nó kiểm**.

Đã sửa cả hai về `& 0x0F`. Ma trận lại **21/21**.

**Bài học**: nếu chỉ đọc code mà không chạy ma trận, tôi đã "vá" shell và làm đỏ cổng — hoặc tệ hơn,
nếu chỉ sửa test cho xanh thì đã **che** lỗi. Cách đúng là sửa **cả hai** về cùng một định nghĩa
đúng (bốn bit), và ghi lại vì sao.

## Kiểm lại sau khi sửa cả hai

```text
bash v2/test/boot/ktest.sh 1 matrix  → # KTEST: 21/21 boot sạch · ktest.sh: PASS
```

## Ảnh canonical mới nhất (sau khi sửa cả shell và test)

```text
REPRO sha256=34ee7a25da5fa9f325a9c291360333855d7da25472929588b8a714e9ffa01ac3
      size=205440  inputs=be9689a5c032cf69  dirty=0  rev=244f06d11777
      build=907912139bffcff0  text=130062
# đã ghim: v2/artifacts/34ee7a25….bin

bash v2/test/parity/run.sh --mode serial --image v2/artifacts/34ee7a25….bin → KET_LUAN=ok
bash v2/test/boot/persist.sh → persist.sh: PASS (7 pha, distinct_process=1)
```
