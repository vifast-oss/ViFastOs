# KT-96 — `soak.sh --image <ảnh>`: soak ảnh có sẵn, không build lại

**Ngày**: 2026-11-17 · **Ngân sách**: 45 phút · **Dùng**: ~25 phút.

## Vì sao

`soak.sh` luôn **tự build** ảnh vào `v2/kernel/target/…` rồi soak ảnh đó. Hệ quả:

- không soak được **ảnh đã ghim** (`v2/artifacts/<sha256>.bin`) — tức không soak được đúng ảnh đã đo;
- build của soak **không** có feature ⇒ soak đo ảnh **không shell, không FS** (đây là gốc của khoảng
  trống KT-90 phát hiện: "0 lỗi/1000" không phải ảnh phát hành).

## Thay đổi

Thêm `--image <ảnh>`: dùng **đúng** ảnh được chỉ định, **không** build, **không** ghi `target/`.
Mặc định (không cờ) **giữ nguyên** hành vi cũ — không đổi phương pháp đo của cổng.

## Hai chốt an toàn (quan trọng hơn tính năng)

1. **Ảnh có shell ⇒ TỪ CHỐI** (mã thoát hai). `soak.sh` đo "boot tới **halt**"; ảnh có shell không
   tự thoát ⇒ mọi boot `timeout` ⇒ `0/N` là **giả**. Script nói thẳng và trỏ sang
   `v2/agent/JOURNAL/kt90-soak-product-image.sh` (script gửi `exit` qua UART).
2. **Lần dò danh tính chấp nhận timeout** khi ảnh cho trước **đã in** dòng `# build:` — vì mục đích
   của lần dò là **đọc dòng đó**, không phải đòi thoát sạch. `vbrev_gate` ngay dưới vẫn kiểm byte ảnh.

## Bằng chứng

```text
# ảnh không shell, build mới (inputs=3caf5e7d5f0a3321 dirty=0):
bash v2/test/boot/soak.sh --image /tmp/kt96-plain/…/vifast-kbin-riscv64 --smoke 3 1
# soak: 3/3 lỗi=0

# ảnh phát hành (có shell):
bash v2/test/boot/soak.sh --image v2/artifacts/dd9f7050….bin --smoke 3 1
# soak.sh: ảnh … CÓ shell (không tự thoát) — soak.sh không đo được ảnh này.   (mã thoát hai)

# ảnh cũ định dạng rev= (276e68c8…):
# REV-GATE: REFUSE reason=malformed   (đúng: cổng mới chỉ nhận `inputs=`)
```

## Chưa làm

- **Chưa** làm `soak.sh` soak được ảnh **có shell** (cần gửi `exit` qua UART = thêm đường vào ⇒ đổi
  phương pháp đo, cần Agent 0). Hiện dùng script KT-90.
- **Chưa** nối `pre-measure.sh` vào soak (cùng lý do).
