# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-22 (KT-332, sau `KT-330`)**: **không còn task nào bị chặn kỹ thuật**.
> Toàn bộ mục lịch sử vòng 1 → vòng 25 đã chuyển **nguyên văn** sang
> [`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) — **đọc-only**, không xoá thông tin.
> Nợ mở của Chu kỳ 2 nằm ở khối trạng thái [`STATE.md`](STATE.md), không phải tệp này.

## 1. Mục BLOCKED đang mở

**Không có** (tính đến 2026-11-22, sau `KT-330`).

## 2. Nợ có chủ kế thừa (không phải BLOCKED — không chặn task nào)

| Nợ | Trạng thái hiện tại | Nguồn |
|---|---|---|
| `CNode.Mutate` (3) / `CNode.Rotate` (6) | Có số trong `spec/abi.toml` nhưng **chưa cài** ⇒ trả `NotSupported` (18); có TAP. Muốn cài phải có một D2 mới. | `v2/kernel/vifast-core/src/invoke.rs` (nhánh 761, 822), archive §KT-68 |
| `CNode.Insert` | **Không có** số method trong bảng `CNode` của `spec/abi.toml` (đóng băng 0..6) ⇒ giữ `NotSupported`. | archive §KT-67b/§KT-68 |
| `Tcb.Exec(path)` | **Chưa có** trong ABI; `exec` hiện dùng khoá trong bảng dựng sẵn, chưa nhận `path` từ user. | `STATE.md` mục "Nợ mở"; archive §KT-74 |
| `exec`: ca lỗi-muộn + ca SMP thật | Chưa dựng được ca lỗi-muộn (`ObjKind::Untyped` không được object layer hỗ trợ); chưa có ca nhiều hart. | archive §KT-74 (review D3) |
| `ping`/`ifconfig` trong shell sản phẩm | **Chưa có** lệnh mạng trong `shell_user.rs`; net server U-mode đã có nhưng chưa nối thành lệnh shell. | archive §KT-77a, `JOURNAL/kt281-netsrv.md` |
| Đọc **DTB** trên `aarch64` | `kmain(_dtb)` giữ đúng hợp đồng gọi nhưng **chưa đọc** DTB; thuộc `KT-347` (UEFI/DTB/ACPI) trong Chu kỳ 2. | `v2/kernel/vifast-kbin-aarch64/src/main.rs:635-638` |
| `x86_64`: `VSpace` động + W^X toàn ảnh + TLB shootdown | **Chưa có**; `KT-340a/340b` (Chu kỳ 2, `BLOCKED` theo DAG). | `QUEUE.md` dòng `KT-340a/340b` |
| Nợ **P20** (sha ảnh phụ thuộc thư mục build) | **Còn mở**; `KT-346` (Chu kỳ 2) xử lý cùng `VBREV_ALGO`. | `STATE.md` mục "Nợ mở"; `v2/docs/k6-repro.md` |
| Số ca TAP mặc định hard-code trong `run-aarch64.sh` / `soak-aarch64.sh` | Đã có biến `EXPECT_TESTS` để ghi đè, nhưng giá trị mặc định vẫn nằm trong hai script (nợ quy trình mức thấp). | `v2/test/boot/run-aarch64.sh:38,186`; `soak-aarch64.sh` |

## 3. Giới hạn có văn bản (không phải chặn)

- **`KT-75`** — ca âm `flush` là **mô hình cache** (QEMU ghi thẳng file ảnh), không phải mất dữ liệu thật;
  `ktest-persist` không nằm trong matrix vì một pha là hai boot trên cùng ảnh đĩa. Chi tiết: archive §KT-75,
  `v2/docs/k6-persist.md`.
- **`KT-78`** — harness QEMU phụ thuộc `perl` cho `setpgrp` (fail-closed nếu thiếu); `loadavg` chỉ là
  lưới phụ, bộ đếm QEMU mới là detector chính; `qemu_own` không đếm QEMU trong lane soak.
  Chi tiết: archive §KT-78, `v2/docs/k6-harness.md` §7.
- **`K6-CLOSURE.md`** — ảnh K6 ghim `a0287e4b…`; tái lập đúng ảnh cần `git checkout f4d1dd5f` vì hai commit
  sau đổi `build-rev.sh` (chuỗi dấu vân tay nhúng trong ảnh đổi, mã sản phẩm không đổi).

## 4. Chuyển tiếp theo lịch (không phải chặn)

- `KT-345a` (áp `spec/` ABI v1.1) **chỉ Agent 0 mở** sau `APPROVED <ngày> Agent 0` cho `KT-344`.
- `KT-340a/340b/340c`, `KT-338a/338b`, `KT-345b`, `KT-348` đang `BLOCKED` **theo DAG** — xem `QUEUE.md`
  và `KE-HOACH-CHU-KY-2-2026-11-22.md` §2; đây là thứ tự phụ thuộc, không phải lỗi kỹ thuật.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
