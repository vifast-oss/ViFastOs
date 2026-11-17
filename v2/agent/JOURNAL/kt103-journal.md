# KT-103 — shell: cảnh báo "dòng quá dài" in **một lần** mỗi dòng

**Ngày**: 2026-11-18 · **Ngân sách**: 30 phút · **Dùng**: ~15 phút.

## Lỗi

`v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` — vòng đọc UART:

```rust
} else {
    // Dòng quá dài: bỏ phần dư, không tràn buffer (báo để không im lặng).
    put(b"# shell: dong qua dai\n");
}
```

Nhánh này chạy cho **mỗi byte** vượt `MAX_LINE` (64 với ảnh có FS, 32 với ảnh không FS). Một dòng
dán vào dài một nghìn byte ⇒ gần **một nghìn** dòng cảnh báo ⇒ ngập log, chậm UART, và che mất các
dòng bằng chứng khác.

## Vá

Thêm cờ `overflowed`: báo **một lần** cho mỗi dòng, đặt lại khi gặp `\n`.

## Bằng chứng

```text
# ảnh build từ HEAD sau vá, gửi: help → echo + 200 ký tự 'A' → exit
count_dong_qua_dai = 1        (trước: một dòng cảnh báo cho mỗi byte vượt 64 ⇒ ~141 dòng)
shell> xuất hiện 3 lần        (sau help, sau dòng dài, sau exit)
```

## Ghi chú

- Hành vi **không** đổi: dòng vẫn bị cắt ở `MAX_LINE` và vẫn được chạy (đã cắt) — chỉ **số dòng
  cảnh báo** đổi.
- Không đụng `CMDS` ⇒ `# shell: help cmds=` giữ nguyên, harness không phải sửa.

## Kiểm lại sau vá (kernel đổi ⇒ phải đo lại)

```text
REPRO sha256=f6f3d0c6106263af9fa83f80c519531beadfea9b926038550c300f87d2e0fd4c
      size=205440  inputs=044599ac17964d22  dirty=0  rev=535227cc95eb
      build=907912139bffcff0  text=130062
# đã ghim: v2/artifacts/f6f3d0c6….bin

bash v2/test/boot/ktest.sh 1 matrix   → # KTEST: 21/21 boot sạch · ktest.sh: PASS
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/f6f3d0c6….bin
                                      → parity: not_ok=0 · KET_LUAN=ok (exit 0)
bash v2/test/boot/persist.sh          → persist.sh: PASS (7 pha, distinct_process=1)
```

Ảnh canonical mới nhất: **`f6f3d0c6…`** (205.440 B, `inputs=044599ac17964d22 dirty=0`).
