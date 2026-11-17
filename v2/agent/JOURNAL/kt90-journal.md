# KT-90 — soak 1.000 trên **ảnh phát hành** (product-shell + product-fs)

**Ngày**: 2026-11-17 · **Ngân sách**: 1,5 h · **Dùng**: ~25 phút (script do agent viết trước khi phiên đó dừng;
lead sửa một lỗi kiểm và chạy).

## Vì sao có task này

KT-88 báo soak `0 lỗi/1000`, nhưng `v2/test/boot/soak.sh:172` build **không** feature:

```text
cargo build … --bin vifast-kbin-riscv64 --release      ← không product-shell, không product-fs
```

⇒ soak của KT-88 chạy trên ảnh sản phẩm **không shell, không FS** (188.680 B, `# k3 halt`), **khác**
ảnh phát hành `291502d2…` (205.440 B, có shell + FS). Khoảng trống này do lead tự phát hiện khi
kiểm số (A1-189), không phải agent tự khai.

## Cách đo (không sửa harness)

Script `kt90-soak-product-image.sh` (chỉ trong `JOURNAL/`) đọc `soak.sh` để lấy **nguyên văn** bộ cờ
QEMU, rồi boot **đúng ảnh phát hành** `v2/artifacts/291502d2….bin`. Mỗi vòng:

1. chờ `shell>` (timeout 20 s);
2. gửi `help` qua UART ⇒ đòi `# shell: help cmds=`;
3. gửi `exit` ⇒ đòi `# shell: exit code=0` + `# product halt:` + QEMU tự thoát;
4. kiểm danh tính ảnh: đúng một dòng `# build:`, có `# product shell:`, có `# product fs:`, `rx_bytes>0`, 0 `PANIC`;
5. giữ `TAP version 13` / `ok 1 - k3 boot` như `soak.sh`.

**Lỗi lead sửa trước khi chạy**: script kiểm `icount` trên cả dòng chú thích của `soak.sh`
(`# … KHÔNG -icount …`) nên tự từ chối; đã đổi sang chỉ kiểm **phần cờ** (`${LINE%%#*}`).

## Kết quả

```text
# wall=355s tổng=1000/1000 ok=1000 fail=0 (355s)
# ảnh: sha256=291502d2…0353 changed=0 bytes=205440
# infra before: qemu_con_sot_total=0 qemu_ppid1=0
# infra after:  qemu_con_sot_total=0 qemu_ppid1=0
# kt90: 1000/1000 vòng xanh — mỗi vòng có 'shell>' + phản hồi 'help' qua UART + halt sạch
```

Ba dòng chứng cứ mẫu (mỗi vòng đều có):

```text
# boot 1 | # build: rev=cf96eb9b589d dirty=0
# boot 1 | # product shell: harts=1 procsrv=0xd shell=0xf … const:input_mode=serial const:inject=0
# boot 1 | # product fs: medium=ram blocks=96 used=2 total=66 volatile=1 why=khong-co-pci …
# boot 1 | # shell: help cmds=9 | help=9 | rx=10
```

## So sánh với soak của KT-88

| | KT-88 (`soak.sh`) | KT-90 (script này) |
|---|---|---|
| Ảnh | sản phẩm **không** feature (188.680 B) | **phát hành** `291502d2` (205.440 B) |
| Shell | không có | **có**, mỗi vòng gửi `help` + `exit` |
| FS | không có | **có** (`# product fs:` mỗi vòng; volume RAM vì soak không gắn `-drive`) |
| Kết quả | 0 lỗi/1000 | **0 lỗi/1000** |
| Wall | (trong chuỗi KT-88) | 355 s |

## Hạn chế (nói thẳng)

- Soak **không gắn `-drive`** ⇒ FS dùng volume RAM (`medium=ram volatile=1 why=khong-co-pci`).
  Đường **đĩa thật** đã được đo ở KT-88 (hai boot, cùng tệp đĩa, sha đĩa không đổi), không lặp ở đây.
- Đây là bằng chứng **thêm**, **không** thay số của KT-88. Agent 0 quyết định tiêu chí 4 dùng số nào
  (A1-189).
- Script nằm trong `JOURNAL/`, **không** phải harness chính thức; nếu A0 muốn thành cổng thường trực
  thì phải thành task riêng (sửa `soak.sh` = đổi phương pháp đo, cần A0 duyệt).

## Vòng review độc lập (verify-kt90) — APPROVE + 3 ghi chú, đã vá 2

Reviewer kiểm **6/6 mục** và kết luận **APPROVE**: log đúng **1.000** dòng `boot N/1000 ok`, không thiếu/trùng/lạ;
**1000/1000** tệp `boot-N.log` đều có `shell>`, `# shell: help cmds=`, `# shell: exit code=0`, `# product halt:`,
`# build: rev=cf96eb9b589d dirty=0`; sha ảnh **bây giờ** vẫn `291502d2…0353`; pilot tái lập **6/6 ok**;
bản vá `-icount` của lead đúng **cả hai chiều** (dòng thật ⇒ PASS, dòng có `-icount` ⇒ REFUSE).

**Đã vá theo review**:
- **N1** — script **xoá** `kt90-preflight.log` mỗi lần chạy ⇒ pilot của reviewer ghi đè bằng chứng preflight của phép đo
  (reviewer đã phục hồi byte-identical). Nay mỗi lần chạy ghi tệp **có dấu thời gian**; tệp đã commit được giữ nguyên.
- **N2** — `QARGS` là **bản cứng**, không suy ra từ `soak.sh`; header/journal nói "lấy nguyên văn" là **quá lời**.
  Nay sửa lời cho đúng, và thêm **đối chiếu từng cờ** với `soak.sh` (lệch ⇒ **TỪ CHỐI ĐO**).

**Ghi nhận, không vá** (reviewer nêu, không chặn):
- `qemu_con_sot` là **cảnh báo**, không phải cổng (đếm mọi QEMU trên máy nên không thể làm cổng cứng).
- sha ảnh kiểm **đầu/cuối**; giữa chừng chỉ kiểm danh tính qua dòng `# build:` từng vòng.
- Đếm **số lượng** vòng, không kiểm **duy nhất** (thực tế `lane_worker` bước nhảy `LANES` nên tất định; `results.txt` có 1000 id duy nhất).
- Cửa sổ đọc `QEMU_ARGS` của `soak.sh` là **hai dòng**; nếu sau này trải ba dòng thì cờ ở dòng thứ ba sẽ lọt.

**Phạm vi bằng chứng (reviewer xác nhận, giữ nguyên)**: KT-90 đủ cho câu *"1.000 boot trên đúng ảnh phát hành,
mỗi vòng shell thật chạy `help`+`exit` và halt sạch"*; **không** đủ cho câu *"soak 1.000 phủ đường đĩa thật"* —
đường đĩa mới có **hai boot + persist** (KT-88). Muốn bằng chứng đĩa 1.000 vòng thì phải task riêng (gắn `-drive`).
