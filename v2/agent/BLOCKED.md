# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-24 08:00 (KT-347b — dọn mục đã đóng, không xoá mục còn hiệu lực)**: **không còn
> task nào bị chặn kỹ thuật**; các mục BLOCKED còn lại là **chờ DAG** (task trước chưa xong), không phải
> chặn kỹ thuật. Bốn nợ ghi ở bản 2026-11-22 nay **đã đóng ba** (`KT-335b` gộp chính sách DHCP/ARP/ICMP/
> UDP/DNS; `KT-340b` hạ `.bss` x86_64 96.320 → 43.072 B; `KT-338b`+`KT-339a` sản phẩm aarch64 có
> MMU/VSpace/U-mode + `product-shell`), chỉ còn **`Mailbox SMP-1`** hoãn sang Chu kỳ 3. Nợ **P20** đóng
> bởi `KT-346` (`VBREV_ALGO` `kt134-v5`, danh tính 3 arch); nợ `.data+.bss` x86_64 đóng bởi `KT-340b`;
> nợ DHCP/ARP/... đóng bởi `KT-335b`.
> Toàn bộ mục lịch sử vòng 1 → vòng 25 đã chuyển **nguyên văn** sang
> [`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) — **đọc-only**, không xoá thông tin.
> Nợ mở của Chu kỳ 2 nằm ở khối trạng thái [`STATE.md`](STATE.md), không phải tệp này.

## 1. Mục BLOCKED đang mở

**Không có** (tính đến 2026-11-24, sau `KT-347b`; `KT-348` chỉ `BLOCKED` **theo DAG**, xem §4).

## 2. Nợ có chủ kế thừa (không phải BLOCKED — không chặn task nào)

| Nợ | Trạng thái hiện tại | Nguồn |
|---|---|---|
| `CNode.Mutate` (3) / `CNode.Rotate` (6) | Có số trong `spec/abi.toml` nhưng **chưa cài** ⇒ trả `NotSupported` (18); có TAP. Muốn cài phải có một D2 mới. | `v2/kernel/vifast-core/src/invoke.rs` (nhánh 761, 822), archive §KT-68 |
| `CNode.Insert` | **Không có** số method trong bảng `CNode` của `spec/abi.toml` (đóng băng 0..6) ⇒ giữ `NotSupported`. | archive §KT-67b/§KT-68 |
| `Tcb.Exec(path)` | **Chưa có** trong ABI; `exec` hiện dùng khoá trong bảng dựng sẵn, chưa nhận `path` từ user. | `STATE.md` mục "Nợ mở"; archive §KT-74 |
| `exec`: ca lỗi-muộn + ca SMP thật | Chưa dựng được ca lỗi-muộn (`ObjKind::Untyped` không được object layer hỗ trợ); chưa có ca nhiều hart. | archive §KT-74 (review D3) |
| `ping`/`ifconfig` trong shell sản phẩm | **Chưa có** lệnh mạng trong `shell_user.rs`; net server U-mode đã có nhưng chưa nối thành lệnh shell. | archive §KT-77a, `JOURNAL/kt281-netsrv.md` |
| Đọc **DTB** trên `aarch64` | `kmain(_dtb)` giữ đúng hợp đồng gọi nhưng **chưa đọc** DTB; thuộc `KT-347` (UEFI/DTB/ACPI) trong Chu kỳ 2. | `v2/kernel/vifast-kbin-aarch64/src/main.rs:635-638` |
| `x86_64`: W^X **toàn ảnh** | **Đã đóng phần `VSpace` động + TLB shootdown** (`KT-340b`/`KT-340d` DONE: `.bss` sản phẩm 43.072 B, `INVLPG` + IPI AP↔BSP có TAP). Phần **W^X toàn ảnh** (mọi trang, không chỉ `.corespace`) **chưa xác nhận** ⇒ **giữ mục** (không xoá khi chưa đo). | `QUEUE.md` dòng `KT-340b`/`KT-340d`; `KT-353a` (Chu kỳ 3) |
| `vifast_sync::Mailbox<N>` chưa nối vào kênh wake/IPI riscv64 | **Hoãn có văn bản**: `IpiMailbox` là **bitmask không bao giờ đầy** còn `Mailbox<N>` là FIFO **đầy ⇒ `Busy`** ⇒ thay thẳng sẽ **mất đánh thức** (D2-68/D2-77). `IpiMailbox` đã phủ 3/3 port (riscv64 KT-336c, aarch64 KT-338c, x86_64 ở `KT-340d`) | `SMP-1` **Chu kỳ 3** (D2-244) |
| `v2 mutants` (đột biến thật) **chưa tồn tại** | `nightly --all` tạm dùng `selfbreak` (KT-80) | **Chu kỳ 3** (`v2.1`, D2-244) |
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

- `KT-345a` (áp `spec/` ABI v1.1) **đã DONE** sau `APPROVED 2026-11-22 Agent 0` cho `KT-344`; mục cũ
  ở đây đã đóng (KT-347b dọn). **5 nợ đã đóng được gỡ khỏi §2 kèm bằng chứng**: P20 (`KT-346`),
  `x86_64 .data+.bss` 96.320→43.072 B (`KT-340b`), aarch64 chưa MMU/U-mode (`KT-339a`), chính sách
  DHCP/ARP/ICMP/UDP/DNS (`KT-335b`), `KT-345a` chờ duyệt — chi tiết trong `QUEUE.md` các dòng DONE.
- **`KT-348`** (Cổng nghiệm thu `v2.0`) đang `BLOCKED` **theo DAG** — chờ `KT-347b` `DONE`; xem
  `QUEUE.md` và `KE-HOACH-CHU-KY-2-2026-11-22.md` §2. Đây là thứ tự phụ thuộc, không phải lỗi kỹ thuật.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
