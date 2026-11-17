# KT-99 — soak 1.000 trên **ảnh canonical mới** `dd9f7050…`

**Ngày**: 2026-11-17 · **Ngân sách**: 30 phút · **Dùng**: ~15 phút (wall đo 533 s).

## Vì sao

Ảnh `291502d2…` (dùng cho KT-88/KT-90) build **trước** KT-89/92/93. Ảnh `dd9f7050…` (KT-94) build từ
HEAD hiện tại nhưng mới chỉ được kiểm **parity 6/6** — chưa soak.

## Kết quả

```text
# wall=533s tổng=1000/1000 ok=1000 fail=0 (533s)
# ảnh: sha256=dd9f7050c1a0a6f63edca611544ff144b40246cc3ec29289fc08b25f5f2feadc changed=0 bytes=205440
# infra before: qemu_con_sot_total=0 qemu_ppid1=0
# infra after:  qemu_con_sot_total=0 qemu_ppid1=0
# kt90: 1000/1000 vòng xanh — mỗi vòng có 'shell>' + phản hồi 'help' qua UART + halt sạch
```

Mỗi vòng: `shell>` → `help` (phản hồi `cmds=9`) → `exit` → `# product halt: shell_exit=0`. Ảnh
**không đổi** trong suốt phép đo (`changed=0`).

## Thay đổi kèm theo

`v2/agent/JOURNAL/kt90-soak-product-image.sh` nhận `--image=<path>`: sha, số byte và dòng danh tính
**suy ra từ chính ảnh** (không đoán). Mặc định (không cờ) vẫn là ảnh `291502d2…` của phép đo KT-88.
Khi ảnh dùng định dạng `inputs=` (không có `rev`), phần `git diff <rev>..HEAD` được bỏ qua và ghi rõ.

## Ý nghĩa cho K6

Ảnh **khớp HEAD** giờ có đủ: parity 6/6 (KT-94), soak **một nghìn trên một nghìn** (đây), ma trận
21/21 + persist (KT-97). Nếu Agent 0 chọn phương án (b) ở `K6-CLOSURE-DRAFT.md` §2, bằng chứng đã
sẵn trên ảnh mới — không phải đo lại.

## Hạn chế (giữ nguyên từ KT-90)

Soak **không** gắn `-drive` ⇒ FS dùng volume RAM. Đường **đĩa thật** mới có hai boot + persist
(KT-88 §2b, KT-97).
