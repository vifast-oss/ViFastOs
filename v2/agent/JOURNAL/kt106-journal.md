# KT-106 — soak một nghìn trên ảnh canonical **mới nhất** `34ee7a25…`

**Ngày**: 2026-11-18 · **Ngân sách**: 30 phút · **Dùng**: ~12 phút (wall đo 397 s).

## Vì sao

Ảnh `34ee7a25…` là ảnh **đầu tiên** chứa cả ba bản vá shell của đêm (KT-103 dòng dài, KT-104
`cat`/`write` thiếu tham số, KT-105 `ps` trạng thái). Soak gần nhất (`dd9f7050…`) chạy **trước** ba
bản vá đó.

## Kết quả

```text
# wall=397s tổng=1000/1000 ok=1000 fail=0 (397s)
# ảnh: sha256=34ee7a25da5fa9f325a9c291360333855d7da25472929588b8a714e9ffa01ac3 changed=0 bytes=205440
# infra before: qemu_con_sot_total=0 qemu_ppid1=0
# infra after:  qemu_con_sot_total=0 qemu_ppid1=0
# kt90: 1000/1000 vòng xanh — mỗi vòng có 'shell>' + phản hồi 'help' qua UART + halt sạch
```

Mỗi vòng: `shell>` → `help` (phản hồi `cmds=9`) → `exit` → `# product halt: shell_exit=0`. Ảnh
**không đổi** trong suốt phép đo (`changed=0`).

## Ý nghĩa

Ba bản vá shell **không** làm hỏng đường boot/exit ở quy mô một nghìn vòng. Cộng với ma trận
hai mươi mốt trên hai mươi mốt và persist PASS (đã chạy trên cùng ảnh), ảnh `34ee7a25…` có **đủ**
bộ bằng chứng: parity, ma trận, persist, soak.

## Hạn chế (giữ nguyên)

Soak **không** gắn `-drive` ⇒ FS dùng volume RAM; đường **đĩa thật** chỉ có persist (bảy pha).
