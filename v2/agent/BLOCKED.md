# BLOCKED — ViFastOs v2

Người ghi: **Agent 2** dòng OS. Mỗi mục: task, lý do, chặn bởi ai/cái gì, cần gì để mở.
Quy tắc 2.5: kẹt **hai lần** cùng một ý ⇒ ghi vào đây, **đổi nhánh**, không thử lần ba.

> **Cập nhật 2026-11-22 22:20 (sau `KT-345a`, `KT-333c`, `KT-335`, `KT-338c`)**: **không còn task nào
> bị chặn kỹ thuật**; các mục BLOCKED còn lại là **chờ DAG** (task trước chưa xong), không phải
> chặn kỹ thuật. Bốn nợ mới được ghi ở §2 (dòng `KT-335b`, `Mailbox SMP-1`, `.bss` x86_64, sản
> phẩm aarch64).
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
| `x86_64`: `VSpace` động + W^X toàn ảnh + TLB shootdown | **ĐÃ ĐÓNG** `KT-340b/340c/340d` (`.data+.bss` 43.072 B, TLB shootdown + `IpiMailbox`); W^X `.corespace` còn nợ `KT-353a` (Chu kỳ 3). Ghi cũ: `KT-340a/340b` theo DAG). | `QUEUE.md` dòng `KT-340a/340b` |
| Nợ **P20** (sha ảnh phụ thuộc thư mục build) | **ĐÃ ĐÓNG** `KT-346` (`VBREV_ALGO=kt134-v5`, `v2/user` trong `VBREV_INPUT_ROOTS`, `build_identity` 3 arch; bằng chứng hai thư mục ⇒ cùng vân tay). Còn `OD-K6-3` (máy thứ hai + Windows) ở `v2.1`. Ghi cũ: `KT-346` (Chu kỳ 2) xử lý cùng `VBREV_ALGO`. | `STATE.md` mục "Nợ mở"; `v2/docs/k6-repro.md` |
| **Chính sách `DHCP`/`ARP`/`ICMP`/`UDP`/`DNS`** còn ở 3 tệp platform | `KT-335` mới gộp **transport** + **TCP client**; phần chính sách **chưa** gộp vì ba bản khác nhau về **hợp đồng ba trần thời gian** (`DHCP_TOTAL`/`ATTEMPT`/`MAX_ATTEMPTS`) — gộp cơ học sẽ đổi hành vi timeout TAP x86_64 | **`KT-335b`** (READY) |
| `vifast_sync::Mailbox<N>` chưa nối vào kênh wake/IPI riscv64 | **Hoãn có văn bản**: `IpiMailbox` là **bitmask không bao giờ đầy** còn `Mailbox<N>` là FIFO **đầy ⇒ `Busy`** ⇒ thay thẳng sẽ **mất đánh thức** (D2-68/D2-77). `IpiMailbox` đã phủ 3/3 port (riscv64 KT-336c, aarch64 KT-338c, x86_64 ở `KT-340d`) | `SMP-1` **Chu kỳ 3** (D2-244) |
| `x86_64` `.data+.bss` sản phẩm **96.320 B > 64 KiB** | **ĐÃ ĐÓNG** `KT-340b`: nay **43.072 B** (`product`) / **51.176 B** (`product-shell`) — nhãn tách theo D2-250. Ghi cũ: `KT-340b`: dời bảng trang boot/DMA/stack khỏi `.bss` như aarch64 đã làm ở `KT-338a` | **`KT-340b`** (READY) |
| Ảnh sản phẩm `aarch64` **0 B** `.data+.bss` nhưng **chưa có** MMU/VSpace/U-mode | **ĐÃ ĐÓNG** `KT-339a`: `product-shell` có MMU + `VSpace` + `Machine` (NOLOAD) + EL0 shell, `.data+.bss` **696 B**, TAP **46**. Ghi cũ: Phạm vi `KT-338b` (nối `vifast-core` + IPC EL0); hiện chỉ boot + pl011 + marker + halt | **`KT-338b`** (BLOCKED theo DAG) |
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

- `KT-345a` (áp `spec/` ABI v1.1) **chỉ Agent 0 mở** sau `APPROVED <ngày> Agent 0` cho `KT-344`.
- `KT-348` là task **còn `BLOCKED`** duy nhất theo DAG (chờ `KT-347`+`KT-347b`) — `KT-340a..c`, `KT-338a/b`, `KT-345b`, `KT-342b`, `KT-346` đã `DONE`; xem `QUEUE.md`
  và `KE-HOACH-CHU-KY-2-2026-11-22.md` §2; đây là thứ tự phụ thuộc, không phải lỗi kỹ thuật.

## 5. Lịch sử

[`archive/BLOCKED-chu-ky-1.md`](archive/BLOCKED-chu-ky-1.md) giữ nguyên văn: bảng REVIEW vòng 1; bảng
`BLOCKED` theo `PATCH-BRIEF` §1; `YC-01..YC-18`; dòng ViScript; `KT-67`; `KT-74`; `KT-75`; `KT-76`/S1;
`KT-78`; `KT-77a`; ghi chú phối hợp `KT-80`; vá `KT-77a` sau review.
