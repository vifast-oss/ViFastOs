# KT-81 — thiết bị giả **nghiêm ngặt** + bảng tài nguyên một nguồn

**Việc Agent 0 giao (phiếu lượt 4, mục "Thứ tự", việc 4, nguyên văn):**

> 4. KT-81 → K7 (write thiếu nội dung + nhóm ưu tiên trước, KT-137 cd/pwd cuối)

Và trong `reports/QUYET-DINH-A0-K6-2026-11-17.md` (OD-K6-5, luật 5 và 6):

> | **KT-81** | Agent 2 | Thiết bị giả nghiêm ngặt + bảng tài nguyên (luật 5, 6) | 2 lỗi driver cũ
> (FLUSH chưa đàm phán, đòi độ dài) bị bắt ở mô hình |

Đây là **hai luật cuối** trong tám luật máy của OD-K6-5; sáu luật kia đã cài ở KT-80.

## 1. Luật 5 — thiết bị giả nghiêm ngặt

**Vấn đề.** Hai lỗi driver của v1 sống lâu vì **mô hình** (thiết bị giả trong test) chấp nhận mọi
thứ: không feature, không dung lượng, không cỡ dữ liệu — driver gửi gì cũng "chạy". Một thiết bị
thật thì không như vậy. Mô hình dễ dãi **tệ hơn** không có mô hình, vì nó tạo cảm giác đã kiểm.

**Đã thêm.** `v2/lib/vifast-virtio/src/strict.rs` — `StrictBlk`, mô hình hoá bốn điều kiện mà một
`virtio-blk` thật luôn kiểm:

| Điều kiện | Lỗi v1 | `StrictError` |
|---|---|---|
| Gửi `T_FLUSH` khi **chưa** đàm phán `VIRTIO_BLK_F_FLUSH` | **#1** | `FlushNotNegotiated` |
| Độ dài dữ liệu không khớp loại request + số sector | **#2** | `LengthMismatch { want, got }` |
| Sector/dải sector ngoài dung lượng | — | `OutOfRange { sector, sectors, capacity }` |
| Loại request lạ | — | `Unsupported(kind)` |
| Chưa `DRIVER_OK` | — | `NotReady` |

Hai chi tiết **cố ý**:

- **Kiểm cả hai phía** của `FLUSH`: driver khai đã đàm phán **và** thiết bị báo có bit. Driver tự
  cho mình quyền gửi `FLUSH` trong khi thiết bị chưa hứa hỗ trợ cũng là sai.
- **Thứ tự kiểm cố định**: sẵn sàng → loại → đàm phán FLUSH → độ dài → dải sector. Nếu không cố
  định, cùng một request sai cho hai lý do khác nhau ở hai lần chạy, log đỏ không tái lập được và
  người đọc phải đoán. Có ca test riêng cho việc này.

**Test.** `v2/lib/vifast-virtio/tests/strict.rs` — 10 ca, mỗi lỗi v1 có **ca âm** (request sai ⇒
đúng `StrictError`) **và ca dương** (cùng request nhưng đúng ⇒ `S_OK`). Ca dương không thừa: một
`submit` luôn trả lỗi cũng qua được toàn bộ ca âm.

**Đột biến** (chạy thật, không phải suy đoán):

| Đột biến | Kết quả |
|---|---|
| bỏ nhánh `FlushNotNegotiated` | `FAILED. 7 passed; 3 failed` — đỏ đúng ba ca nói về FLUSH |
| bỏ nhánh `LengthMismatch` | `FAILED. 8 passed; 2 failed` — đỏ đúng hai ca nói về độ dài |

## 2. Luật 6 — bảng tài nguyên một nguồn + kiểm chồng lấn

**Vấn đề.** Địa chỉ UART/PLIC/CLINT/trang test/ECAM nằm rải ở nhiều tệp dưới dạng hằng số viết
tay. Hai hậu quả, cả hai **im lặng**: (1) **hai nguồn** — một chỗ sửa, chỗ khác không ⇒ MMU map một
địa chỉ, driver đọc một địa chỉ khác, không lỗi biên dịch, không lỗi lúc chạy; (2) **chồng lấn** —
hai vùng gán đè nhau ⇒ ghi vùng này xoá vùng kia, không ai kiểm vì mỗi hằng số khai riêng lẻ.

**Đã thêm.** `v2/kernel/vifast-plat-common/src/resources.rs`:

- `Region { name, base, size }` + `REGIONS` — **một** bảng cho sáu vùng MMIO nền tảng (uart0,
  plic, clint, test-page, pci-ecam, pci-mmio32).
- `overlaps(a, b)` — nửa mở `[base, base+size)`, kích thước 0 không chồng.
- `first_overlap(regions)` — trả **cặp** vùng đè nhau (không chỉ `bool`): khi đỏ, người đọc cần biết
  **hai vùng nào**.
- `first_invalid(regions)` — bắt vùng kích thước 0 hoặc tràn địa chỉ (`end()` bão hoà, không quay
  vòng thành địa chỉ nhỏ).

**Một nguồn thật, không phải lời khai.** `v2/kernel/vifast-plat-riscv64/src/lib.rs` nay **dẫn xuất**
`UART0_BASE`/`PLIC_BASE`/`CLINT_BASE`/`TEST_PAGE_BASE` từ bảng (`resources::UART0.base as usize`, …)
— không còn hằng số viết tay thứ hai để lệch.

**Test.** `v2/kernel/vifast-plat-common/tests/resources.rs` — 7 ca: bảng thật không chồng lấn, mọi
vùng hợp lệ và căn trang, bảng đủ sáu vùng bắt buộc, chồng lấn bị bắt (đè một phần / chứa nhau /
trùng một byte ở biên), kích thước 0 bị bắt, tràn địa chỉ bị bắt, bảng rỗng không báo động.

**Đột biến:** tăng `PLIC.size` thêm một trang để nó đè `UART0` ⇒ `bang_that_khong_chong_lan`
`FAILED`; khôi phục ⇒ `7 passed`.

## 3. Cái **không** có trong task này

- **Không** sửa driver thật (`virtio_blk.rs`). `StrictBlk` là **mô hình để test**, không phải đường
  chạy sản phẩm — nó không được biên dịch vào ảnh.
- **Không** đổi ABI, không đổi `spec/`, không nới ngưỡng.
- **Không** thay linker script: bảng chỉ chứa vùng MMIO nền tảng; RAM/stack/NOLOAD vẫn do linker
  script quyết định.
- **Không** chứng minh driver thật **tuân** mọi điều kiện của `StrictBlk`. Việc đó cần nối mô hình
  vào đường chạy thật (một bước nữa, chưa làm). Hiện tại giá trị của task là: **hai lỗi v1 ấy giờ
  có một mô hình bắt được**, và bảng tài nguyên có phép kiểm chồng lấn thật.
