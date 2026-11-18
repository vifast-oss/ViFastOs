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

---

# KT-81 §A5a — nối `StrictBlk` vào **mã dựng request của driver thật**

**Ngày**: 2026-11-18 · **Agent 1** (Agent 0 phiếu lượt 5 §A5a)

## 4. Việc Agent 0 giao (nguyên văn §A5a)

> a) StrictBlk nối driver thật: cho mã dựng request của driver virtio-blk thật (cùng mã biên dịch
>    vào ảnh, không chép lại) chạy qua StrictBlk trong test host; phủ đọc / ghi / flush / nhiều
>    sector / sector cuối. Đột biến: tái tạo lỗi v1 #1 và #2 trong mã driver ⇒ StrictBlk báo đúng
>    FlushNotNegotiated / LengthMismatch. Không nối được mà không đổi driver ⇒ DỪNG, ghi vào hàng
>    chờ Agent 0.

## 5. Hiện trạng trước khi sửa — vì sao §3 ở trên **không đủ**

`v2/lib/vifast-virtio/tests/strict.rs` tự dựng `ReqHeader::new(...)` bằng tay rồi đưa cho
`StrictBlk`. Nghĩa là nó chứng minh **mô hình**, **không** chứng minh **driver**: một driver gửi
request sai vẫn qua, vì test không bao giờ chạy mã của driver.

Driver thật (`v2/kernel/vifast-plat-riscv64/src/virtio_blk.rs::submit`) tự làm **ba** việc rời nhau:

```rust
let header = ReqHeader::new(kind, sector).encode();          // (1) header
let status_index = if kind == T_FLUSH { 1u16 } else { 2u16 }; // (2) hình dạng chuỗi descriptor
… len: SECTOR_BYTES as u32 …                                  // (3) độ dài đệm
```

Ba chỗ có thể lệch nhau, và test host chỉ chạm được chỗ (1).

## 6. Sửa gì

| tệp | sửa |
|---|---|
| `v2/lib/vifast-virtio/src/blk.rs` | **MỚI** `BuiltRequest` + `build_request(kind, sector, sectors)` — trả **một** giá trị gồm header đã mã hoá, `sectors`, `data_len`, `status_index`. `data_len` **tính từ số sector**, không nhận từ người gọi (chính chỗ khai tay là nơi lỗi v1 #2 sinh ra). |
| `v2/kernel/vifast-plat-riscv64/src/virtio_blk.rs` | `submit` gọi `build_request(kind, sector, 1)`; header, `status_index`, và `len` của descriptor dữ liệu đều lấy từ **cùng** giá trị ấy. Không còn `ReqHeader::new` hay `SECTOR_BYTES` thứ hai trong đường dựng request. |
| `v2/lib/vifast-virtio/tests/driver_request.rs` | **MỚI** 11 ca: đọc / ghi / flush / **nhiều sector** / **sector cuối** / ngoài mép / chưa đàm phán FLUSH / độ dài lệch (ba chiều) / loại lạ / hình dạng request / **cấu trúc** (driver phải gọi `build_request`). |

**Vì sao `build_request` nằm trong `vifast-virtio` chứ không trong crate kernel**: `vifast-virtio`
là crate **thuần**, biên dịch **cả** vào ảnh (driver gọi nó) **và** vào test host. Đó đúng là điều
kiện "cùng mã biên dịch vào ảnh, không chép lại". `vifast-plat-riscv64` có inline asm nên **không**
build được cho host — đặt mã dựng request ở đó thì test host không chạm tới được.

## 7. Bằng chứng — ca kiểm chạy thật

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-virtio --test driver_request
test doc_mot_sector ... ok
test doc_nhieu_sector ... ok
test doc_qua_mep_bi_tu_choi ... ok
test do_dai_lech_bi_tu_choi ... ok
test driver_that_dung_build_request ... ok
test flush_khi_chua_dam_phan_bi_tu_choi ... ok
test flush_sau_dam_phan ... ok
test ghi_mot_sector ... ok
test hinh_dang_request_dung ... ok
test loai_la_bi_tu_choi ... ok
test sector_cuoi ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 8. Đột biến — tái tạo **hai lỗi v1 trong mã driver** ⇒ ĐỎ đúng lý do

### Đột biến #1 — lỗi v1 #1: driver **không** đàm phán `VIRTIO_BLK_F_FLUSH` mà vẫn gửi `T_FLUSH`

Sửa `negotiated_features` để bỏ nhánh `FLUSH_MASK`:

```text
test flush_sau_dam_phan ... FAILED
thread 'flush_sau_dam_phan' panicked at …/driver_request.rs:74:5:
  left: Err(FlushNotNegotiated)
test do_dai_lech_bi_tu_choi ... FAILED
thread 'do_dai_lech_bi_tu_choi' panicked at …/driver_request.rs:148:5:
  left: Err(FlushNotNegotiated)
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

`StrictBlk` báo **đúng** `FlushNotNegotiated`. (`do_dai_lech_bi_tu_choi` cũng đỏ vì thứ tự kiểm cố
định: đàm phán FLUSH được kiểm **trước** độ dài — đúng thiết kế.)

### Đột biến #2 — lỗi v1 #2: driver khai **độ dài đệm sai**

Sửa `build_request` để `data_len = 0` cho mọi loại:

```text
test doc_mot_sector ... FAILED
  left: Err(LengthMismatch { want: 512, got: 0 })
test ghi_mot_sector ... FAILED
  left: Err(LengthMismatch { want: 512, got: 0 })
test doc_nhieu_sector ... FAILED
  left: Err(LengthMismatch { want: 2048, got: 0 })
test sector_cuoi ... FAILED
test doc_qua_mep_bi_tu_choi ... FAILED
test hinh_dang_request_dung ... FAILED
test result: FAILED. 5 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

`StrictBlk` báo **đúng** `LengthMismatch`. Khôi phục cả hai ⇒ `11 passed`.

## 9. Cái **không** làm

- **Không** đổi ngữ nghĩa driver: vẫn một request một lúc, vẫn `SECTOR_BYTES = 512`, vẫn poll `used`.
  Chỉ **gom** ba chỗ dựng request về một hàm.
- **Không** thêm `unsafe`, không đổi ABI, không đổi `spec/`.
- **Không** chứng minh driver tuân **mọi** điều kiện của `StrictBlk` (ví dụ thứ tự fence, `used->id`).
  Việc đó cần chạy `StrictBlk` **trong** đường thật — vẫn là nợ, ghi ở §10.
- **Không** sửa test `smp` đỏ sẵn — xem `kt81-quet-bang-tai-nguyen.md` §6.

## 10. Nợ còn lại của KT-81

`StrictBlk` nay kiểm **mã dựng request** của driver thật, nhưng **không** nằm trong đường chạy sản
phẩm: nó không kiểm được thứ tự fence, `used->id`, hay việc driver có thật sự gửi đúng cái nó dựng.
Muốn đóng nốt thì phải cho driver gọi `StrictBlk` ở chế độ kiểm (feature `ktest`) — đó là thay đổi
đường chạy, cần Agent 0 quyết.
