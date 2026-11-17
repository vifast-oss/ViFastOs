# KT-97 — kiểm lại toàn bộ sau các thay đổi đêm 17/11

**Ngày**: 2026-11-17 · **Mục đích**: chứng minh các thay đổi KT-92/93/96 **không** làm hỏng đường
chạy thật (không chỉ `v2 check` xanh).

## Vì sao phải chạy lại

- **KT-92** đổi cách `build.rs` gọi `$RUSTC` (thêm `--remap-path-prefix`) — ảnh hưởng **mọi** ELF
  U-mode (shell, procsrv, chương trình test).
- **KT-93** đổi danh sách build-inputs ⇒ dấu vân tay đổi.
- **KT-96** đổi `soak.sh` (thêm `--image`, chốt an toàn).

`v2 check` chỉ chạy L0 + luật; nó **không** chứng minh kernel boot đúng.

## Kết quả (HEAD `fccc9242`, cây sạch)

```text
bash v2/test/boot/ktest.sh 1 matrix
# KTEST: 21/21 boot sạch (smp=matrix, NBOOT=1, 7 ảnh × smp 1/2/4)
# ktest.sh: PASS

bash v2/test/boot/persist.sh
# phase_reboot: phase=7 … distinct_process=1
# persist.sh: PASS

bash v2/test/parity/run.sh --mode serial --image v2/artifacts/dd9f7050….bin
# parity: total=28 ok=6 deferred=22 active=6
# parity: not_ok=0 timeout=0 khong_co_shell=0
# parity: KET_LUAN=ok (exit 0)
```

## Đọc kết quả

- **Ma trận 21/21** trên HEAD sau KT-92 ⇒ bản vá remap **không** làm hỏng ELF U-mode. Đây là điều
  phải chứng minh: remap đổi byte ảnh, nên nếu đường dẫn bị cắt sai thì shell/procsrv sẽ không nạp.
- **Persist PASS** ⇒ đường FS + virtio-blk còn nguyên sau khi `build.rs` đổi.
- **Parity 6/6** trên ảnh canonical **mới** `dd9f7050…` ⇒ ảnh mới dùng được.

## Chưa làm

- **Soak 1.000** trên ảnh mới: `soak.sh` từ chối ảnh có shell (KT-96), script KT-90 đang gắn cứng
  ảnh cũ. Muốn soak ảnh mới phải parameterize script KT-90 — để lượt riêng.
- **Conformance** (`run.sh --boot`) chưa chạy lại.
