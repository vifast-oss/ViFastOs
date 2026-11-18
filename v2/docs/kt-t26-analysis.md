<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-T26 — "ai chen giữa" (D-03) và đèn host cho read-your-own-write

Trạng thái: **REVIEW** (chưa duyệt). Máy đo: `v2/lib/vifast-fs/tests/t26_read_your_own_write.rs`.
Triệu chứng gốc: `docs/v1/KNOWN-ISSUES.md` #2 (T26).

## 1. Con số đúng về `MAX_IO` (sửa theo review #7)

- **`MAX_IO` của v1 = 128**, đặt ở `crates/vifast-abi/src/lib.rs` (r262 **hạ 256 → 128** vì bảng đo
  `256: 12/49 hỏng`, `128: 0/32`). Nó **không** được port sang `v2/lib/vifast-fs` (D2-28) và **không tồn tại**
  trong crate đó.
- `USER_COPY_MAX = 256` (`crates/vifast-abi/src/lib.rs`) là **hằng số khác** (số byte copy tối đa một syscall).
- Vì vậy **256** ở KT-T26 là **cấu hình tái hiện T26**, không phải giá trị sản phẩm: giữ đúng 256 để tái hiện
  `10/50 = 20 %` boot đỏ của v1 (D-04 "giữ cấu hình tái hiện"), **không** hạ về 128 cho xanh, **không** nâng
  hằng số sản phẩm lên 256.
- Trong test: `const MAX_IO: usize = 256;` **cục bộ**, kèm `const _: () = assert!(MAX_IO == 256);`.

## 2. Danh sách "kẻ chen giữa" (D-03: đánh *ai/cái gì* chen giữa)

| # | Kẻ chen giữa | Nếu fs phụ thuộc vào nó thì hỏng thế nào | Trong đèn |
|---|---|---|---|
| 1 | **Writer khác ghi** block trong lúc ta vừa ghi | Cache bị đuổi/ghi đè ⇒ đọc lại thấy bản cũ (đúng triệu chứng T26) | `interleave()` ghi block lạ (vùng ≥ `DATA_START+200`) **sau từng** write, rồi đọc lại ngay |
| 2 | **Xoá cache** (`clear`) giữa hai thao tác | Nếu fs chỉ đúng trong cache thì mất | `interleave()` chọn ngẫu nhiên `clear()` |
| 3 | **Đọc RAW** (driver khác) | Đĩa cũ hơn cache | Đọc RAW sau `fsync` bằng **mount lại** từ ảnh đĩa |
| 4 | **Mất ghi im lặng** (write trả `Ok`, không ghi) | Đây chính là **kênh T26** (không phải lỗi I/O) | `SilentLossDisk` — **ca âm** bắt buộc |
| 5 | Journal replay / torn write | Không thuộc T26 (đã có unit test journal) | ngoài phạm vi KT-T26; nêu để không hở phạm vi |

## 3. Thiết bị và đèn

- Dùng **`CachedDev<D>` đang ship** (không tự chế cache): nó giữ bất biến write-ahead và có sẵn đèn
  `lost_writes()` ("số lần ghi-xong-mà-đọc-lại-khác (T26)"), `slot_state()`, `evict_stats()`.
- Tầng dưới là `RawDisk` (đĩa thô trong bộ nhớ); ca âm bọc `SilentLossDisk`.
- **Vì sao không dùng cache tự chế**: `commit()` của fs gọi `dev.flush()` ngay trong đường ghi
  (`tx.rs:81`), nên sau mỗi thao tác chỉ còn **một** khối bẩn (commit record LBA 29). Cache tự chế flush
  theo thứ tự LBA sẽ phá đúng bất biến write-ahead mà `CachedDev` giữ.

## 4. Phép đo (tất định, host, không QEMU)

| Test | Nội dung | Seed | Kết quả |
|---|---|---|---|
| `ryow_1000_seeds` | ghi `3 × 256` byte, kẻ thứ ba ghi xen sau **từng** khúc, đọc lại ngay; `lost_writes() == 0` | 1.000 | PASS |
| `ryow_across_remount_1000_seeds` | `fsync` → mount lại từ **ảnh RAW** → `lookup` + đọc phải khớp | 1.000 | PASS |
| `harness_catches_silent_write_loss` (ca âm) | thiết bị **mất ghi im lặng** ⇒ phép đo phải trả lỗi | — | PASS (bắt được) |

Bằng chứng đỏ của chính đèn: khi tạm đảo kỳ vọng của ca âm, test **đỏ ngay** (xem log trong
`v2/agent/JOURNAL/2026-11-12.md`, mục KT-T26) — nghĩa là đèn thật sự bắt được mất ghi im lặng.

## 5. Kết luận trung thực

- Bản port v2 **không** tái hiện T26 ở 1.000 seed: fs ghi qua `commit()` + `flush()` ngay, nên giữa hai
  thao tác không còn khối bẩn; mọi kẻ chen giữa (1)–(3) vô hại.
- Điều đó **chưa** chứng minh v1 hết lỗi: T26 ở v1 nằm ở tầng VFS/client (đọc `dir` qua `MAX_IO`), **không**
  trong crate `vifast-fs`; v2 chưa port tầng đó.
- Vì vậy KT-T26 hiện là **đèn**: nó chứng minh (a) read-your-own-write đúng ở cấu hình 256, (b) đèn bắt được
  mất ghi im lặng. Việc tái hiện đầy đủ `-smp 2` + 50 boot thuộc **YC-06** (cần sim KT-08 hoặc VFS K3.5).

## 6. Việc chưa làm

- Chưa có tầng VFS/`read_dir` trong v2 ⇒ phép đo dừng ở `create` + `lookup` + `read` của fs.
- ~~Chưa chạy trên sim/QEMU (`-smp 2`, 50 boot)~~ — **đã chạy ở KT-149** (xem §7).
- Không nới `MAX_IO`; không đổi hằng số sản phẩm.

## 7. KT-149 (D2-129) — đo lại bốn điều kiện N2

- **(ii) 50 boot ảnh sản phẩm v2** (`product-shell,product-fs`, `-smp 2`) với kịch bản ghi rồi đọc
  lại **ngay**, so **từng byte**: **0 đỏ** — harness `v2/test/boot/t26.sh`, log
  `v2/agent/JOURNAL/kt149-logs/v2-50boots.log`.
- **(iii) test host** `t26_read_your_own_write` ở `MAX_IO=256`, 1.000 seed: **xanh** (3 test).
- **(iv) đột biến** bỏ `flush()` thứ hai trong `commit()` ⇒ (iii) **đỏ** (2 test đỏ ở seed 1);
  khôi phục ⇒ xanh.
- **(i) vế 1 trên tag `v1-archive`**: **0/50 T-FS** — tag đã chứa bản vá r296b (`9cd233f3`), nên
  phép đo đúng chữ **không** tái hiện T26. Trên **mã trước vá** (`5fc7b3ff`, cha của bản vá):
  **5/50 T-FS**, đúng chữ ký `V2MK open=1` / `V2RD end n=06` / `V2FAIL v2=1 fsck=0` / `FS V2 BAD`.
  Cần lead/Agent 0 chốt vế 1 dùng tag (đã vá) hay commit trước vá.

Số đo nguyên văn (mỗi khối trích từ đúng log ghi kèm):

```text scope=v2/agent/JOURNAL/kt149-logs/v2-50boots.log
# t26: boots=50 green=50 red=0
```

```text scope=v2/agent/JOURNAL/kt149-logs/v1-50boots-tag.log
SUMMARY red=2 green=48 total=50
```

```text scope=v2/agent/JOURNAL/kt149-logs/v1-50boots-prefix.log
SUMMARY red=7 green=43 total=50
```
