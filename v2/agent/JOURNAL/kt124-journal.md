# KT-124 — ảnh canonical cuối cùng của đêm (sau KT-122/123)

**Ngày**: 2026-11-18 · **Ngân sách**: 20 phút · **Dùng**: ~10 phút.

## Vì sao build lại

KT-122 (TAP 36 + plan) và KT-123 (`ST_NOFS` + trả lời ngay khi FS = `None`) đều đổi **kernel**.

## Kết quả

```text
REPRO sha256=64db17ea5b6c70833c50482372c5bd73abd2839dd7942fc70528dc71d2a07c06
      size=205440  inputs=8d5d423fb4cb7763  dirty=0  rev=484b6f986ed0
      build=6778ed91c8e63c33  text=130072
# đã ghim: v2/artifacts/64db17ea….bin

bash v2/test/boot/ktest.sh 1 matrix  → # KTEST: 21/21 boot sạch · ktest.sh: PASS
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/64db17ea….bin → KET_LUAN=ok
bash v2/test/boot/shell-edge.sh --image v2/artifacts/64db17ea….bin → 8/8 ca PASS
bash v2/test/boot/persist.sh → persist.sh: PASS (7 pha, distinct_process=1)
./v2/bin/v2 check → PASS
```

## Ca âm đáng ghi: ảnh có `product-fs` mà FS = `None`

Dựng bằng đĩa `virtio-blk` **read-only** (mount hỏng ⇒ `medium=none why=mount-loi`):

```text
# shell: fs khong san sang (image khong co product-fs hoac khong co phuong tien)
# shell: exit code=0
# product halt: shell_exit=0 …
```

**Ngay lập tức** — trước KT-123 mỗi lệnh đứng **~54 giây** (kernel rút request mà không trả lời, shell
quay hai mươi nghìn vòng `Yield`). Parity FS vẫn **đỏ** ở ca âm 5(c) như cũ: chỉ nhanh hơn, không nới
phép kiểm.
