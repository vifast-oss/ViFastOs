# K6 — BẢN NHÁP văn bản đóng (chờ Agent 0)

> **Trạng thái: NHÁP.** Tài liệu này **không** tuyên bố K6 đạt. Nó chuẩn bị hai phương án để Agent 0
> chọn, kèm đúng ba trạng thái (đóng / hoãn / mở) mà D2-119 yêu cầu. Khi Agent 0 chọn, văn bản chính
> thức là bản này **trừ** phần chưa chọn.

## 1. Phần đã đo được (không phụ thuộc lựa chọn nào)

Sáu tiêu chí của D2-123.3 đã đo trên ảnh phát hành `291502d2…` tại HEAD `cf96eb9b589d`, cây sạch.
Chi tiết từng dòng log: [`K6-EVIDENCE-INDEX.md`](K6-EVIDENCE-INDEX.md).

| Nhóm | Trạng thái | Ghi chú |
|---|---|---|
| Boot ảnh phát hành ở ba cấu hình hart | **đóng** | ba boot, cổng rev `ACCEPT` cả ba |
| Parity sáu lệnh `active` | **đóng** | hai cấu hình hart, `not_ok` bằng không |
| Ca âm (không shell / không FS) | **đóng** | hai ca, mã thoát riêng |
| Ma trận ảnh test | **đóng** | bảy ảnh × ba cấu hình |
| Ba ngưỡng kích thước | **đóng** | ảnh, `.data+.bss`, `Machine` |
| Harness trên ảnh phát hành | **đóng** | sản phẩm không hit, ca âm có hit |
| **Soak** | **CHỜ CHỌN** | xem §2 |

## 2. Điểm phải chọn: soak đo ảnh nào

**Sự thật đo được** (không suy đoán):

- `v2/test/boot/soak.sh` build **không** feature ⇒ soak của KT-88 chạy trên ảnh **không shell, không
  FS** (`v2/test/boot/soak.sh:172`). Đó **không** phải ảnh phát hành.
- **KT-90** soak **đúng** ảnh phát hành, mỗi vòng có `shell>` + `help` + `exit` sạch — log
  `kt90-logs/kt90-soak-1000.log:1016-1020`, journal `kt90-journal.md`.

### Phương án (a) — tiêu chí bốn định nghĩa là "kernel ổn định trên ảnh **mặc định**"

- Dùng số của KT-88 (ảnh không feature).
- Shell/FS đã được chứng minh bằng **đường khác**: parity sáu lệnh, ba boot ảnh phát hành, KT-90.
- **Ưu**: không phải sửa harness; số cũ hợp lệ.
- **Nhược**: câu "soak ảnh phát hành" **không** đúng theo nghĩa hẹp; phải ghi rõ trong văn bản.

### Phương án (b) — tiêu chí bốn định nghĩa là "soak **ảnh phát hành**"

- Dùng số của KT-90 (một nghìn trên một nghìn, ảnh có shell + FS).
- **Ưu**: đúng nghĩa "sản phẩm"; mạnh hơn cho K6.
- **Nhược**: KT-90 là script trong `JOURNAL/`, **không** phải harness chính thức; muốn thành cổng
  thường trực thì phải sửa `soak.sh` (thêm đường gửi `exit`) = **đổi phương pháp đo**, cần Agent 0.

**Đề xuất của lead**: chọn **(b)**, và ghi rõ trong văn bản rằng bằng chứng đến từ script KT-90
(đã review độc lập, APPROVE), đồng thời mở một task riêng để đưa năng lực đó vào `soak.sh` chính
thức. Lý do: (a) đòi người đọc hiểu "soak không phải soak ảnh sản phẩm" — đúng loại mơ hồ đã gây ra
năm lần lỗi D2-108A.

## 3. Ba trạng thái (theo D2-119)

### 3.1 ĐÓNG

Sáu tiêu chí ở §1 (trừ soak, chờ §2), cộng **hai** lỗi v1 đã đóng: **N4**, **N6**.

### 3.2 HOÃN

**N5** — hoãn sang **K5′** theo D2-119. K7 **không** nhận N5.

### 3.3 MỞ

**N1**, **N2**, **N3**, **N7** — bốn lỗi v1 còn mở, chuyển sang K7 ("Đóng lỗi v1").

Bảng chi tiết `file:dòng` đã xong: [`v2/agent/JOURNAL/kt83-table.md`](../JOURNAL/kt83-table.md).
Tóm tắt trạng thái bằng chứng (từ bảng đó):

| Lỗi | Bằng chứng v1 | Ca kiểm v2 | Vì sao còn mở |
|---|---|---|---|
| N1 | có | **một phần** — chỉ một hart, không migrate | thiếu ca "đơn điệu sau migrate" + chéo hart |
| N2 | có | **đủ** (có ca âm) | thiếu phép đo "năm mươi boot, không đỏ" |
| N3 | có | **một phần** — hai client, không có checker "đúng một lần" | thiếu checker + ca bốn client + đột biến phát lại |
| N7 | có | **chưa có** ca kiểm đúng tiêu chí | chưa có test "một cơ chế wake chung cho ba port" |

**Đính chính kèm theo** (KT-83): `K6-DOD.md` §4 có ba chỗ sai đã ghi đính chính ngay đầu mục —
hai dẫn `file:dòng` lệch, và **N5** phải đọc là **HOÃN K5′**, không phải "N/A".

## 4. Nợ P20 (tái lập byte) — caveat bắt buộc

- Tái lập byte **đạt** trên **pipeline canonical** (`v2/tools/repro-build.sh`), hai máy × hai đường
  dẫn (D2-122.1).
- **Chưa** đạt cho **ảnh phát hành** theo nghĩa "ảnh phát hành = ảnh canonical": phần chênh là
  **bảng symbol** (D2-124: `.symtab` + `.strtab` + `.shstrtab`), không phải mã chạy.
- Nợ chuyển **v2.1** với tiêu chí "ảnh phát hành = ảnh canonical".

## 5. Điều kiện kỹ thuật đã đạt trước khi ký

| Điều kiện | Trạng thái |
|---|---|
| Luật một không còn miễn trừ nào (`pre_existing` bằng không) | đạt — KT-82a |
| Cổng so **nội dung** build-inputs, không so lịch sử git | đạt — KT-89 |
| Ảnh tự khai nguồn, harness từ chối ảnh lệch | đạt — KT-86/89 |
| Tái lập byte **cả** phần `build.rs` tự dựng | đạt — KT-92 |
| Công cụ ghim ảnh + kiểm trước khi đo | đạt — KT-84/95 |

## 6. Việc phải làm sau khi Agent 0 chọn

- KT-83 — bảng bảy lỗi v1 với `file:dòng`.
- Văn bản chính thức (bản này, bỏ nhánh không chọn).
- Nếu chọn (b): task đưa năng lực gửi `exit` vào `soak.sh`.
